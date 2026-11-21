// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Ground-station and teleport timing and pass-quality assurance (E19 B1907)
//!
//! Research 06 (H3, H4, H5, I7) and 06b (AG6, AG7, AG30, LP4): TT&C networks and teleports sell
//! contacts by SLA, often booked through a federated provider, and their pass schedules, time
//! tags and ranging all hang on the station clock. From a station's pass log and its clock
//! log this module gives:
//!
//! * **per pass** a verdict (`delivered`, `degraded` with the criteria missed, `missed`,
//!   `excluded` for a pass cancelled by the customer): contact fraction (actual over scheduled
//!   duration), acquisition delay, data delivered fraction and frame error rate against the
//!   SLA, and the station's **time error during the pass** (largest |offset| of the clock log in
//!   the pass window) against the timing limit;
//! * the **timing limit** derived from the antenna (MODELLED): a time error `Δt` points the
//!   antenna `ω·Δt` behind the satellite, `ω = v/h` the largest angular rate (zenith pass of a
//!   circular orbit at altitude `h`, `v = sqrt(μ/(R+h))`, Earth rotation neglected), so the
//!   limit is `pointing_fraction · beamwidth / ω`; or a stated `limit_s` (ranging, time tags);
//! * **SLA aggregates** per station, provider (federated booking), customer and satellite: the
//!   success rate against the target, and the passes behind every miss;
//! * a signed evidence directory (`report.json`, `report.html`, `passes.csv`, inputs) for the
//!   operator's customer reports and its periodic reporting to a regulator.
//!
//! **Label: MODELLED** for the timing limit; COMPUTED for the SLA figures from the logs as
//! supplied. Tested on a synthetic station.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::selfcheck::{self, Reading, SourceSpec};
use super::sig9;
use super::timebomb::Label;

pub const FEATURE: &str = "pro-timing-station";
pub const FORMAT: &str = "kshana-pro-station-assurance/1";
pub const LABEL: &str = "COMPUTED from the pass and clock logs as supplied (SLA figures); MODELLED for the timing limit derived from the antenna beamwidth and the orbit. Tested on a synthetic station; not an audit of the station.";
/// Earth's gravitational parameter, m³/s².
const MU: f64 = 3.986_004_418e14;
/// Earth's mean radius, m.
const RE: f64 = 6_371_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Antenna {
    /// Half-power beamwidth, degrees.
    pub beamwidth_deg: f64,
    /// Share of the beamwidth the time error may take (default 0.1).
    #[serde(default = "d_frac")]
    pub pointing_fraction: f64,
    /// Lowest circular-orbit altitude served, km (default 400).
    #[serde(default = "d_alt")]
    pub altitude_km: f64,
}

fn d_frac() -> f64 {
    0.1
}
fn d_alt() -> f64 {
    400.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sla {
    #[serde(default = "d_contact")]
    pub min_contact_fraction: f64,
    #[serde(default = "d_aos")]
    pub max_aos_delay_s: f64,
    #[serde(default = "d_data")]
    pub min_data_fraction: f64,
    #[serde(default = "d_fer")]
    pub max_frame_error_rate: f64,
    /// Share of the counted passes that must be delivered.
    #[serde(default = "d_target")]
    pub target_success_rate: f64,
}

impl Default for Sla {
    fn default() -> Self {
        Sla {
            min_contact_fraction: d_contact(),
            max_aos_delay_s: d_aos(),
            min_data_fraction: d_data(),
            max_frame_error_rate: d_fer(),
            target_success_rate: d_target(),
        }
    }
}

fn d_contact() -> f64 {
    0.95
}
fn d_aos() -> f64 {
    30.0
}
fn d_data() -> f64 {
    0.99
}
fn d_fer() -> f64 {
    1e-4
}
fn d_target() -> f64 {
    0.98
}

/// The station clock log (station minus UTC).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockLog {
    pub format: String,
    pub path: String,
    #[serde(default)]
    pub sign: Option<f64>,
    #[serde(default)]
    pub select: Option<String>,
    #[serde(default)]
    pub time_offset_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationSpec {
    pub station: String,
    #[serde(default)]
    pub operator: Option<String>,
    /// CSV with a header naming the columns (see the module documentation).
    pub passes: String,
    #[serde(default)]
    pub clock: Option<ClockLog>,
    #[serde(default)]
    pub antenna: Option<Antenna>,
    /// A stated timing limit, s (wins over the antenna's).
    #[serde(default)]
    pub limit_s: Option<f64>,
    #[serde(default)]
    pub sla: Sla,
    /// Only passes scheduled in [from, to) (`YYYY-MM-DD`).
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
}

/// One pass of the log.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Pass {
    pub id: String,
    pub satellite: String,
    pub antenna: String,
    pub provider: String,
    pub customer: String,
    pub scheduled_aos: f64,
    pub scheduled_los: f64,
    pub actual_aos: Option<f64>,
    pub actual_los: Option<f64>,
    pub bytes_planned: Option<f64>,
    pub bytes_delivered: Option<f64>,
    pub frames_bad: Option<f64>,
    pub frames_total: Option<f64>,
    /// `ok`, `failed`, `cancelled-customer`, `cancelled-station`, ...
    pub status: String,
}

fn time(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(x) = s.parse::<f64>() {
        return x.is_finite().then_some(x);
    }
    Label::parse_iso(s).ok().map(|l| l.seconds() as f64)
}

/// An optional cell: empty is `None`; anything else must be a finite number or a UTC time
/// (a NaN or a typo would otherwise skip that pass's check, or pass it, without a word).
fn optional(
    row: usize,
    name: &str,
    v: &str,
    parse: fn(&str) -> Option<f64>,
) -> Result<Option<f64>, String> {
    if v.trim().is_empty() {
        return Ok(None);
    }
    parse(v)
        .map(Some)
        .ok_or_else(|| format!("passes row {row}: {name} '{v}' is not a finite number or time"))
}

fn finite_num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// Read the pass CSV (header row; columns `pass_id, satellite, scheduled_aos,
/// scheduled_los` required; `antenna, provider, customer, actual_aos, actual_los,
/// bytes_planned, bytes_delivered, frames_bad, frames_total, status` optional; times ISO 8601
/// or Unix seconds).
pub fn read_passes(text: &str) -> Result<Vec<Pass>, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("passes: empty")?
        .split(',')
        .map(|x| x.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| head.iter().position(|h| h == n);
    for req in ["pass_id", "satellite", "scheduled_aos", "scheduled_los"] {
        if col(req).is_none() {
            return Err(format!("passes: column '{req}' missing"));
        }
    }
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        let c: Vec<&str> = l.split(',').map(str::trim).collect();
        let get = |n: &str| col(n).and_then(|k| c.get(k).copied()).unwrap_or("");
        let row = i + 2;
        let num = |n: &str| optional(row, n, get(n), finite_num);
        let tim = |n: &str| optional(row, n, get(n), time);
        let (Some(a), Some(b)) = (time(get("scheduled_aos")), time(get("scheduled_los"))) else {
            return Err(format!(
                "passes row {}: scheduled_aos and scheduled_los",
                i + 2
            ));
        };
        if b <= a {
            return Err(format!(
                "passes row {}: scheduled_los before scheduled_aos",
                i + 2
            ));
        }
        let st = get("status");
        out.push(Pass {
            id: get("pass_id").into(),
            satellite: get("satellite").into(),
            antenna: get("antenna").into(),
            provider: get("provider").into(),
            customer: get("customer").into(),
            scheduled_aos: a,
            scheduled_los: b,
            actual_aos: tim("actual_aos")?,
            actual_los: tim("actual_los")?,
            bytes_planned: num("bytes_planned")?,
            bytes_delivered: num("bytes_delivered")?,
            frames_bad: num("frames_bad")?,
            frames_total: num("frames_total")?,
            status: if st.is_empty() {
                "ok".into()
            } else {
                st.to_ascii_lowercase()
            },
        });
    }
    Ok(out)
}

/// The largest angular rate seen from the ground for a circular orbit at `alt_km`, rad/s.
pub fn max_angular_rate(alt_km: f64) -> f64 {
    let h = alt_km * 1000.0;
    (MU / (RE + h)).sqrt() / h
}

/// The timing limit from the antenna, s.
pub fn antenna_limit_s(a: &Antenna) -> f64 {
    a.pointing_fraction * a.beamwidth_deg.to_radians() / max_angular_rate(a.altitude_km)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PassResult {
    pub id: String,
    pub satellite: String,
    pub antenna: String,
    pub provider: String,
    pub customer: String,
    pub scheduled_aos: String,
    /// `delivered`, `degraded`, `missed`, `excluded`.
    pub verdict: String,
    pub missed_criteria: Vec<String>,
    pub contact_fraction: Option<f64>,
    pub aos_delay_s: Option<f64>,
    pub data_fraction: Option<f64>,
    pub frame_error_rate: Option<f64>,
    pub max_time_error_s: Option<f64>,
    /// `ok`, `exceeded`, `no-data`, `not-checked`.
    pub timing: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Aggregate {
    pub by: String,
    pub key: String,
    pub counted: usize,
    pub delivered: usize,
    pub degraded: usize,
    pub missed: usize,
    pub excluded: usize,
    pub success_rate: Option<f64>,
    pub meets_target: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StationReport {
    pub format: &'static str,
    pub label: &'static str,
    pub station: String,
    pub operator: Option<String>,
    pub utc: String,
    pub period: Option<(String, String)>,
    pub sla: Sla,
    pub timing_limit_s: Option<f64>,
    pub timing_limit_basis: String,
    pub passes: Vec<PassResult>,
    pub aggregates: Vec<Aggregate>,
    pub timing_exceeded: usize,
    pub verdict: String,
}

/// Assess the station. `base` resolves the request's relative paths.
pub fn assess(spec: &StationSpec, base: &Path, utc: &str) -> Result<StationReport, String> {
    let pp = base.join(&spec.passes);
    let passes =
        read_passes(&std::fs::read_to_string(&pp).map_err(|e| format!("{}: {e}", pp.display()))?)?;
    let from = spec
        .from
        .as_deref()
        .map(time)
        .map(|x| x.ok_or("from: YYYY-MM-DD"))
        .transpose()?;
    let to = spec
        .to
        .as_deref()
        .map(time)
        .map(|x| x.ok_or("to: YYYY-MM-DD"))
        .transpose()?;
    let clock: Option<Vec<Reading>> = match &spec.clock {
        None => None,
        Some(c) => {
            let p = base.join(&c.path);
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            Some(selfcheck::read_source(
                &SourceSpec {
                    name: "station-clock".into(),
                    kind: None,
                    format: c.format.clone(),
                    path: c.path.clone(),
                    u_s: 1.0,
                    disciplining: false,
                    sign: c.sign,
                    time_offset_s: c.time_offset_s,
                    select: c.select.clone(),
                },
                &text,
            )?)
        }
    };
    let (limit, basis) = match (spec.limit_s, &spec.antenna) {
        (Some(l), _) => (Some(l), "stated".to_string()),
        (None, Some(a)) => {
            let w = max_angular_rate(a.altitude_km);
            (
                Some(antenna_limit_s(a)),
                format!(
                    "MODELLED: {} × {}° beamwidth / {:.3}°/s (zenith pass at {} km)",
                    a.pointing_fraction,
                    a.beamwidth_deg,
                    w.to_degrees(),
                    a.altitude_km
                ),
            )
        }
        (None, None) => (None, "no limit: timing not checked".into()),
    };
    let sla = &spec.sla;
    let mut out = Vec::new();
    for p in &passes {
        if from.is_some_and(|f| p.scheduled_aos < f) || to.is_some_and(|t| p.scheduled_aos >= t) {
            continue;
        }
        let sched = p.scheduled_los - p.scheduled_aos;
        let contact = match (p.actual_aos, p.actual_los) {
            (Some(a), Some(b)) if b > a => {
                Some((b.min(p.scheduled_los) - a.max(p.scheduled_aos)).max(0.0) / sched)
            }
            _ => None,
        };
        let aos_delay = p.actual_aos.map(|a| a - p.scheduled_aos);
        let data = match (p.bytes_planned, p.bytes_delivered) {
            (Some(pl), Some(d)) if pl > 0.0 => Some(d / pl),
            _ => None,
        };
        let fer = match (p.frames_bad, p.frames_total) {
            (Some(b), Some(t)) if t > 0.0 => Some(b / t),
            _ => None,
        };
        let (max_te, timing) = match (&clock, limit) {
            (Some(c), Some(l)) => {
                let w0 = p.actual_aos.unwrap_or(p.scheduled_aos);
                let w1 = p.actual_los.unwrap_or(p.scheduled_los);
                let m = c
                    .iter()
                    .filter(|r| r.t >= w0 && r.t <= w1)
                    .map(|r| r.offset.abs())
                    .fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.max(x))));
                match m {
                    Some(m) if m > l => (Some(m), "exceeded"),
                    Some(m) => (Some(m), "ok"),
                    None => (None, "no-data"),
                }
            }
            _ => (None, "not-checked"),
        };
        let mut missed = Vec::new();
        let excluded = p.status == "cancelled-customer";
        let no_contact =
            p.actual_aos.is_none() || p.status == "failed" || p.status == "cancelled-station";
        if !excluded && !no_contact {
            if contact.is_some_and(|c| c < sla.min_contact_fraction) {
                missed.push(format!(
                    "contact fraction {:.3} < {}",
                    contact.unwrap_or(0.0),
                    sla.min_contact_fraction
                ));
            }
            if aos_delay.is_some_and(|d| d > sla.max_aos_delay_s) {
                missed.push(format!(
                    "AOS delay {:.0} s > {} s",
                    aos_delay.unwrap_or(0.0),
                    sla.max_aos_delay_s
                ));
            }
            if data.is_some_and(|d| d < sla.min_data_fraction) {
                missed.push(format!(
                    "data delivered {:.4} < {}",
                    data.unwrap_or(0.0),
                    sla.min_data_fraction
                ));
            }
            if fer.is_some_and(|f| f > sla.max_frame_error_rate) {
                missed.push(format!(
                    "frame error rate {:.2e} > {:e}",
                    fer.unwrap_or(0.0),
                    sla.max_frame_error_rate
                ));
            }
            if timing == "exceeded" {
                missed.push(format!(
                    "station time error {} > limit {}",
                    super::fleet::human_time(max_te.unwrap_or(0.0)),
                    super::fleet::human_time(limit.unwrap_or(0.0))
                ));
            }
        }
        let verdict = if excluded {
            "excluded"
        } else if no_contact {
            "missed"
        } else if missed.is_empty() {
            "delivered"
        } else {
            "degraded"
        };
        out.push(PassResult {
            id: p.id.clone(),
            satellite: p.satellite.clone(),
            antenna: p.antenna.clone(),
            provider: p.provider.clone(),
            customer: p.customer.clone(),
            scheduled_aos: Label::from_unix(p.scheduled_aos.floor() as i64).iso(),
            verdict: verdict.into(),
            missed_criteria: missed,
            contact_fraction: contact.map(sig9),
            aos_delay_s: aos_delay.map(sig9),
            data_fraction: data.map(sig9),
            frame_error_rate: fer.map(sig9),
            max_time_error_s: max_te.map(sig9),
            timing: timing.into(),
        });
    }
    if out.is_empty() {
        return Err("no pass in the period".into());
    }
    let mut aggregates = Vec::new();
    let mut agg = |by: &str, key: &dyn Fn(&PassResult) -> String| {
        let mut m: BTreeMap<String, [usize; 4]> = BTreeMap::new();
        for p in &out {
            let e = m.entry(key(p)).or_default();
            match p.verdict.as_str() {
                "delivered" => e[0] += 1,
                "degraded" => e[1] += 1,
                "missed" => e[2] += 1,
                _ => e[3] += 1,
            }
        }
        for (k, c) in m {
            let counted = c[0] + c[1] + c[2];
            let rate = (counted > 0).then(|| c[0] as f64 / counted as f64);
            aggregates.push(Aggregate {
                by: by.into(),
                key: if k.is_empty() { "(none)".into() } else { k },
                counted,
                delivered: c[0],
                degraded: c[1],
                missed: c[2],
                excluded: c[3],
                success_rate: rate.map(sig9),
                meets_target: rate.map(|r| r >= sla.target_success_rate),
            });
        }
    };
    agg("station", &|_| spec.station.clone());
    agg("provider", &|p| p.provider.clone());
    agg("customer", &|p| p.customer.clone());
    agg("satellite", &|p| p.satellite.clone());
    agg("antenna", &|p| p.antenna.clone());
    let te = out.iter().filter(|p| p.timing == "exceeded").count();
    let st = &aggregates[0];
    let verdict = format!(
        "{}: {} of {} counted passes delivered ({}), target {:.1} % {}; {} pass(es) with the station time error over the limit",
        spec.station,
        st.delivered,
        st.counted,
        st.success_rate
            .map(|r| format!("{:.2} %", 100.0 * r))
            .unwrap_or_else(|| "n/a".into()),
        100.0 * sla.target_success_rate,
        match st.meets_target {
            Some(true) => "met",
            Some(false) => "not met",
            None => "not assessed",
        },
        te
    );
    Ok(StationReport {
        format: FORMAT,
        label: LABEL,
        station: spec.station.clone(),
        operator: spec.operator.clone(),
        utc: utc.into(),
        period: match (&spec.from, &spec.to) {
            (Some(a), Some(b)) => Some((a.clone(), b.clone())),
            _ => None,
        },
        sla: sla.clone(),
        timing_limit_s: limit.map(sig9),
        timing_limit_basis: basis,
        passes: out,
        aggregates,
        timing_exceeded: te,
        verdict,
    })
}

/// The passes as CSV.
pub fn passes_csv(r: &StationReport) -> String {
    let f = |x: Option<f64>| x.map(|v| format!("{v}")).unwrap_or_default();
    let mut o = String::from("pass_id,satellite,antenna,provider,customer,scheduled_aos,verdict,contact_fraction,aos_delay_s,data_fraction,frame_error_rate,max_time_error_s,timing,missed\n");
    for p in &r.passes {
        o.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},\"{}\"\n",
            p.id,
            p.satellite,
            p.antenna,
            p.provider,
            p.customer,
            p.scheduled_aos,
            p.verdict,
            f(p.contact_fraction),
            f(p.aos_delay_s),
            f(p.data_fraction),
            f(p.frame_error_rate),
            f(p.max_time_error_s),
            p.timing,
            p.missed_criteria.join("; ").replace('"', "'")
        ));
    }
    o
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The report as HTML.
pub fn html(r: &StationReport) -> String {
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Station assurance: {}</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:.3rem;text-align:left}}small{{color:#555}}</style></head><body>\
<h1>Station assurance: {}</h1><p>{}</p><p><strong>{}</strong></p><p>Timing limit: {} ({})</p>",
        esc(&r.station),
        esc(&r.station),
        esc(r.label),
        esc(&r.verdict),
        r.timing_limit_s
            .map(super::fleet::human_time)
            .unwrap_or_else(|| "none".into()),
        esc(&r.timing_limit_basis)
    );
    o.push_str("<h2>SLA</h2><table><tr><th>By</th><th>Key</th><th>Counted</th><th>Delivered</th><th>Degraded</th><th>Missed</th><th>Excluded</th><th>Rate</th><th>Target</th></tr>");
    for a in &r.aggregates {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&a.by),
            esc(&a.key),
            a.counted,
            a.delivered,
            a.degraded,
            a.missed,
            a.excluded,
            a.success_rate.map(|x| format!("{:.2} %", 100.0 * x)).unwrap_or_default(),
            match a.meets_target {
                Some(true) => "met",
                Some(false) => "not met",
                None => "",
            }
        ));
    }
    o.push_str("</table><h2>Passes not delivered</h2><table><tr><th>Pass</th><th>Satellite</th><th>Provider</th><th>Verdict</th><th>Why</th></tr>");
    for p in r.passes.iter().filter(|p| p.verdict != "delivered") {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><small>{}</small></td></tr>",
            esc(&p.id),
            esc(&p.satellite),
            esc(&p.provider),
            esc(&p.verdict),
            esc(&p.missed_criteria.join("; "))
        ));
    }
    o.push_str("</table></body></html>\n");
    o
}

/// Write the evidence directory (sign it afterwards).
pub fn write_dir(
    out: &Path,
    r: &StationReport,
    src: &str,
    spec: &StationSpec,
    base: &Path,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
    std::fs::write(
        out.join("report.json"),
        serde_json::to_string_pretty(r).expect("serialises"),
    )
    .map_err(io)?;
    std::fs::write(out.join("report.html"), html(r)).map_err(io)?;
    std::fs::write(out.join("passes.csv"), passes_csv(r)).map_err(io)?;
    std::fs::write(out.join("inputs").join("station.toml"), src).map_err(io)?;
    let mut extra = vec![spec.passes.as_str()];
    if let Some(c) = &spec.clock {
        extra.push(&c.path);
    }
    for p in extra {
        let name = Path::new(p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("input");
        std::fs::copy(base.join(p), out.join("inputs").join(name))
            .map_err(|e| format!("{p}: {e}"))?;
    }
    Ok(())
}
