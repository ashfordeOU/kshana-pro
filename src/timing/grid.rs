// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Power-grid timing evidence (research 07 J01-J04, J07; backlog B3101)
//!
//! A phasor measurement unit (PMU) stamps the grid's voltage and current phasors with a
//! GNSS-disciplined clock. A time error `Δt` becomes a phase-angle error `Δθ = 360°·f0·Δt`
//! (0.0216° per µs at 60 Hz, 0.018° per µs at 50 Hz) and a total vector error of
//! `2·|sin(Δθ/2)|`: the 1 % TVE criterion of IEEE C37.118.1 is used up by about 26.5 µs at
//! 60 Hz and 31.8 µs at 50 Hz with no other error. Five analyses, each over the operator's own
//! records:
//!
//! * **PMU** ([`assess_pmus`]): every PMU's clock log (time error against an independent
//!   reference, read by the self-check's readers [`super::selfcheck::read_source`]) turned
//!   into phase-angle and TVE evidence against a timing budget, joined with the PMU's own
//!   C37.118.2 time flags (read by the open engine's [`kshana::c37118`]): the readings where
//!   the error exceeded the budget while the PMU still reported itself in sync, or exceeded
//!   the time quality it claimed. Per event window, each PMU's error and the angle-difference
//!   error of every PMU pair.
//! * **Gradual spoof** ([`detect_drift`]): a recorded time-error series (or the angle
//!   difference of two PMUs on one bus, one of them on an independent time source) watched by
//!   a level CUSUM against the series' own baseline, a sliding-window rate test against the
//!   clock's plausible rate, and a step test. It reports the first alarm, the estimated onset
//!   and rate, and whether the alarm came before the error used up the budget.
//! * **SCADA time stamps** ([`check_scada`]): device time stamps against the master's receive
//!   time per device: time stamps going backwards, duplicates, device clocks ahead of the
//!   master, clock steps and drift of the latency floor, coarse resolution (wind-farm and
//!   substation SCADA, J07).
//! * **PRC-002 evidence** ([`assess_prc002`]): every recording device's measured time error
//!   against the ±2 ms criterion of NERC PRC-002 (R10, to confirm against the current text)
//!   over a period, with monitoring gaps, and its resilience: how long the device stays within
//!   the criterion after losing its reference, from the holdover evidence model
//!   ([`super::holdover_evidence::assess`]). Signed by the command.
//! * **Event forensics** ([`assess_event`]): was timing a cause of a grid event? Per device in
//!   the event window: measured time error against its budget, a drift alarm in the hours
//!   before, the PMU's own sync flags, the SCADA findings, and recorded sequence-of-events
//!   orderings re-checked after correcting each stamp by its device's measured error.
//!   Signed by the command.
//!
//! **Label: MODELLED.** Arithmetic on the logs as supplied; tested on synthetic records. A
//! finding is evidence for the operator to investigate; whether a standard or regulation is
//! met is for the operator, its auditor and its regulator to decide.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::holdover_evidence::{self, HoldoverSpec, OscillatorSpec};
use super::selfcheck::{read_source, Reading, SourceSpec};

/// The command feature of every `grid` analysis.
pub const FEATURE: &str = "pro-grid-timing";
pub const PMU_FORMAT: &str = "kshana-pro-grid-pmu/1";
pub const DRIFT_FORMAT: &str = "kshana-pro-grid-drift/1";
pub const SCADA_FORMAT: &str = "kshana-pro-grid-scada/1";
pub const PRC002_FORMAT: &str = "kshana-pro-grid-prc002/1";
pub const EVENT_FORMAT: &str = "kshana-pro-grid-event/1";
pub const LABEL: &str = "MODELLED: arithmetic on the operator's records as supplied (clock logs against an independent reference, C37.118.2 captures, SCADA and sequence-of-events logs); tested on synthetic records. Evidence for the operator to investigate, not a regulatory determination: whether a standard or regulation is met is for the operator, its auditor and its regulator to decide.";

/// NERC PRC-002 R10 time-synchronisation criterion for recorded data (±2 ms of UTC).
pub const PRC002_LIMIT_S: f64 = 2e-3;
pub const PRC002_BASIS: &str = "NERC PRC-002 R10: recorded data time-synchronised to within ±2 ms of UTC (to confirm against the current text of the standard)";
/// The steady-state TVE criterion of IEEE C37.118.1.
pub const TVE_CRITERION: f64 = 0.01;
pub const TVE_BASIS: &str = "IEEE C37.118.1 steady-state total vector error 1 % (to confirm against the standard); a pure phase error uses it up at |Δθ| = 2·asin(TVE/2)";

// ---------------------------------------------------------------------------------------
// Shared helpers.

/// Phase-angle error (degrees) of a time error `te_s` at nominal frequency `f0`.
pub fn phase_error_deg(te_s: f64, f0: f64) -> f64 {
    360.0 * f0 * te_s
}

/// Total vector error of a pure phase error of `deg` degrees.
pub fn tve_of_phase(deg: f64) -> f64 {
    2.0 * (deg.to_radians() / 2.0).sin().abs()
}

/// The time error that alone uses up a TVE of `tve` at `f0`.
pub fn te_for_tve(tve: f64, f0: f64) -> f64 {
    2.0 * (tve / 2.0).asin() / (2.0 * std::f64::consts::PI * f0)
}

/// Upper bound of a C37.118.2 PMU time quality code (STAT bits 8-6): `None` for 0 (not
/// used), infinity for 7 (beyond 10 ms or unknown).
pub fn pmu_tq_bound(code: u8) -> Option<f64> {
    match code {
        1 => Some(100e-9),
        2 => Some(1e-6),
        3 => Some(10e-6),
        4 => Some(100e-6),
        5 => Some(1e-3),
        6 => Some(10e-3),
        7 => Some(f64::INFINITY),
        _ => None,
    }
}

/// Upper bound of a C37.118.2 message time quality code (FRACSEC bits 3-0): `None` for 0
/// (locked) and for unused codes, infinity for 0xF (clock fault).
pub fn msg_tq_bound(code: u8) -> Option<f64> {
    match code {
        1..=11 => Some(10f64.powi(i32::from(code) - 10)),
        15 => Some(f64::INFINITY),
        _ => None,
    }
}

/// `x` rounded to 9 significant digits (every float written into a signed pack).
pub fn sig9(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    format!("{x:.8e}").parse().unwrap_or(x)
}

/// Round every number of a JSON value to 9 significant digits.
pub fn round_json(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Number(n) if n.is_f64() => {
            if let Some(x) = n.as_f64() {
                if let Some(m) = serde_json::Number::from_f64(sig9(x)) {
                    *n = m;
                }
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(round_json),
        serde_json::Value::Object(o) => o.values_mut().for_each(round_json),
        _ => {}
    }
}

/// Pretty JSON of `r` with every float rounded to 9 significant digits.
pub fn rounded_json<T: Serialize>(r: &T) -> String {
    let mut v = serde_json::to_value(r).expect("serialises");
    round_json(&mut v);
    serde_json::to_string_pretty(&v).expect("serialises")
}

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// One table of an evidence page: heading, column names, rows.
pub(crate) type Section<'a> = (String, Vec<&'a str>, Vec<Vec<String>>);

/// A plain evidence page: title, label, verdict, then one table per section.
pub(crate) fn page(
    title: &str,
    label: &str,
    verdict: &str,
    intro: &[String],
    sections: &[Section],
) -> String {
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{t}</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:64rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%;margin-bottom:1.5rem}}td,th{{border:1px solid #ccc;padding:.3rem;text-align:left;vertical-align:top}}small{{color:#555}}</style></head><body>\
<h1>{t}</h1><p><small>{l}</small></p><p><strong>{v}</strong></p>",
        t = esc(title),
        l = esc(label),
        v = esc(verdict)
    );
    for i in intro {
        o.push_str(&format!("<p>{}</p>", esc(i)));
    }
    for (h, cols, rows) in sections {
        o.push_str(&format!("<h2>{}</h2><table><tr>", esc(h)));
        for c in cols {
            o.push_str(&format!("<th>{}</th>", esc(c)));
        }
        o.push_str("</tr>");
        for r in rows {
            o.push_str("<tr>");
            for c in r {
                o.push_str(&format!("<td>{}</td>", esc(c)));
            }
            o.push_str("</tr>");
        }
        o.push_str("</table>");
    }
    o.push_str("</body></html>\n");
    o
}

/// Write `report.json` (rounded), `report.html` and the request under `inputs/`.
pub(crate) fn write_pack<T: Serialize>(
    out: &Path,
    report: &T,
    html: &str,
    request_name: &str,
    request: &str,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
    std::fs::write(out.join("report.json"), rounded_json(report)).map_err(io)?;
    std::fs::write(out.join("report.html"), html).map_err(io)?;
    std::fs::write(out.join("inputs").join(request_name), request).map_err(io)?;
    Ok(())
}

fn h(x: f64) -> String {
    super::fleet::human_time(x)
}

/// `x > 0` and not NaN.
pub(crate) fn pos(x: f64) -> bool {
    x > 0.0
}

/// `x >= 0` and not NaN.
pub(crate) fn nonneg(x: f64) -> bool {
    x >= 0.0
}

pub(crate) fn read_text(base: &Path, p: &str) -> Result<String, String> {
    let path = if Path::new(p).is_absolute() {
        p.into()
    } else {
        base.join(p)
    };
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_bytes(base: &Path, p: &str) -> Result<Vec<u8>, String> {
    let path = if Path::new(p).is_absolute() {
        p.into()
    } else {
        base.join(p)
    };
    std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A clock log: the device's time error against an independent reference.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockLogSpec {
    /// Any format of the self-check (`csv` = `unix_time,offset_s[,locked]`, `ptp4l`,
    /// `phc2sys`, `ts2phc`, `chrony-tracking`, `chrony-statistics`, `pps-assert`,
    /// `ntp-peerstats`).
    pub format: String,
    pub path: String,
    /// Multiplier that makes the offset "device minus reference".
    #[serde(default)]
    pub sign: Option<f64>,
    #[serde(default)]
    pub time_offset_s: f64,
    #[serde(default)]
    pub select: Option<String>,
}

/// Read a clock log (path relative to `base`) into readings, device minus reference.
pub fn read_clock(name: &str, c: &ClockLogSpec, base: &Path) -> Result<Vec<Reading>, String> {
    let s = SourceSpec {
        name: name.into(),
        kind: None,
        format: c.format.clone(),
        path: c.path.clone(),
        u_s: 1.0,
        disciplining: false,
        sign: c.sign,
        time_offset_s: c.time_offset_s,
        select: c.select.clone(),
    };
    read_source(&s, &read_text(base, &c.path)?)
}

/// A run of consecutive readings beyond a limit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Interval {
    pub start_unix: f64,
    pub end_unix: f64,
    pub readings: usize,
    pub max_abs_te_s: f64,
}

fn beyond_intervals(rs: &[Reading], limit: f64) -> Vec<Interval> {
    let mut out: Vec<Interval> = Vec::new();
    let mut cur: Option<Interval> = None;
    for r in rs {
        if r.offset.abs() > limit {
            match &mut cur {
                Some(c) => {
                    c.end_unix = r.t;
                    c.readings += 1;
                    c.max_abs_te_s = c.max_abs_te_s.max(r.offset.abs());
                }
                None => {
                    cur = Some(Interval {
                        start_unix: r.t,
                        end_unix: r.t,
                        readings: 1,
                        max_abs_te_s: r.offset.abs(),
                    })
                }
            }
        } else if let Some(c) = cur.take() {
            out.push(c);
        }
    }
    out.extend(cur);
    out
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// The reading nearest `t` within `tol`.
fn nearest(rs: &[Reading], t: f64, tol: f64) -> Option<&Reading> {
    let i = rs.partition_point(|r| r.t < t);
    let mut best: Option<&Reading> = None;
    for j in [i.wrapping_sub(1), i] {
        if let Some(r) = rs.get(j) {
            if (r.t - t).abs() <= tol && best.is_none_or(|b| (r.t - t).abs() < (b.t - t).abs()) {
                best = Some(r);
            }
        }
    }
    best
}

// ---------------------------------------------------------------------------------------
// PMU phase-angle evidence (J01).

fn d_tve() -> f64 {
    TVE_CRITERION
}
fn d_share() -> f64 {
    1.0
}
fn d_tol() -> f64 {
    0.5
}

/// One PMU.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PmuSpec {
    pub name: String,
    /// The PMU's C37.118 IDCODE in the capture (needed when the capture holds several PMUs).
    #[serde(default)]
    pub id: Option<u16>,
    /// A C37.118.2 capture (concatenated frames).
    #[serde(default)]
    pub capture: Option<String>,
    /// The PMU clock's time error against an independent reference.
    #[serde(default)]
    pub clock: Option<ClockLogSpec>,
    /// This PMU's timing budget (s); default from the TVE budget.
    #[serde(default)]
    pub budget_s: Option<f64>,
}

/// An event window.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventWindow {
    pub name: String,
    pub start_unix: f64,
    pub end_unix: f64,
}

/// A PMU evidence request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PmuRequest {
    #[serde(default)]
    pub title: Option<String>,
    /// Nominal grid frequency (50 or 60 Hz).
    pub nominal_hz: f64,
    /// TVE budget (default 0.01).
    #[serde(default = "d_tve")]
    pub tve_budget: f64,
    /// Share of the TVE budget allotted to timing (default 1: timing alone).
    #[serde(default = "d_share")]
    pub timing_share: f64,
    /// Matching tolerance between a clock reading and a capture report (s).
    #[serde(default = "d_tol")]
    pub match_tolerance_s: f64,
    pub pmu: Vec<PmuSpec>,
    #[serde(default)]
    pub event: Vec<EventWindow>,
}

/// What a PMU's capture said about its time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaptureSummary {
    pub id: u16,
    pub station: Option<String>,
    pub reports: usize,
    pub sync_error_reports: usize,
    pub sync_losses: Vec<kshana::c37118::SyncLoss>,
    pub max_unlocked_code: u8,
    /// Worst time quality the PMU claimed (upper bound of the STAT code, s).
    pub worst_claimed_quality_s: Option<f64>,
    pub clock_fault_reports: usize,
    pub leap_pending_reports: usize,
    pub bad_crc: usize,
    pub data_without_config: usize,
    pub size_mismatch: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PmuEvidence {
    pub name: String,
    pub budget_s: f64,
    pub clock_readings: usize,
    pub first_unix: Option<f64>,
    pub last_unix: Option<f64>,
    pub max_abs_te_s: Option<f64>,
    pub max_abs_phase_error_deg: Option<f64>,
    /// TVE from the timing error alone at its worst.
    pub max_tve_from_timing: Option<f64>,
    pub beyond_budget: Vec<Interval>,
    pub beyond_budget_readings: usize,
    /// Readings beyond the budget while the PMU's own report (within the matching tolerance)
    /// said it was in sync and claimed a time quality better than the error measured.
    pub flags_silent_readings: usize,
    /// Readings whose error exceeded the time quality the PMU claimed at that moment.
    pub quality_claim_exceeded_readings: usize,
    /// Readings with a capture report to compare against.
    pub matched_readings: usize,
    pub capture: Option<CaptureSummary>,
    /// `within-budget`, `beyond-budget`, `beyond-budget-flags-silent`, `flags-only`
    /// (no clock log; the capture reported sync errors), `no-clock-evidence`.
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PmuAtEvent {
    pub name: String,
    pub readings: usize,
    pub median_te_s: Option<f64>,
    pub max_abs_te_s: Option<f64>,
    pub phase_error_deg: Option<f64>,
    pub sync_error_reports: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PairAtEvent {
    pub a: String,
    pub b: String,
    /// Error of the measured angle difference θa − θb from the two clocks' errors, degrees.
    pub angle_difference_error_deg: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventAngles {
    pub name: String,
    pub start_unix: f64,
    pub end_unix: f64,
    pub pmus: Vec<PmuAtEvent>,
    pub pairs: Vec<PairAtEvent>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PmuReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: Option<String>,
    pub nominal_hz: f64,
    pub tve_budget: f64,
    pub timing_share: f64,
    pub timing_budget_s: f64,
    pub budget_basis: String,
    pub pmus: Vec<PmuEvidence>,
    pub events: Vec<EventAngles>,
    pub counts: BTreeMap<String, usize>,
    pub verdict: String,
}

/// The default timing budget: the time error that alone uses `tve · share` of TVE.
pub fn timing_budget(f0: f64, tve: f64, share: f64) -> Result<f64, String> {
    let ok = pos(f0) && pos(tve) && tve < 2.0 && pos(share) && share <= 1.0;
    if !ok {
        return Err(
            "nominal_hz must be > 0, tve_budget in (0, 2) and timing_share in (0, 1]".into(),
        );
    }
    Ok(te_for_tve(tve * share, f0))
}

pub(crate) struct LoadedPmu {
    pub readings: Vec<Reading>,
    pub reports: Vec<kshana::c37118::TimeReport>,
    pub capture: Option<CaptureSummary>,
}

pub(crate) fn load_pmu(
    name: &str,
    id: Option<u16>,
    capture: Option<&str>,
    clock: Option<&ClockLogSpec>,
    base: &Path,
) -> Result<LoadedPmu, String> {
    let readings = match clock {
        Some(c) => read_clock(name, c, base)?,
        None => Vec::new(),
    };
    let (reports, capture) = match capture {
        None => (Vec::new(), None),
        Some(p) => {
            let s = kshana::c37118::read_stream(&read_bytes(base, p)?);
            let ids: Vec<u16> = match &s.config {
                Some(c) => c.pmus.iter().map(|p| p.id).collect(),
                None => {
                    let mut v: Vec<u16> = s.reports.iter().map(|r| r.pmu).collect();
                    v.sort_unstable();
                    v.dedup();
                    v
                }
            };
            let pid = match id {
                Some(i) => i,
                None if ids.len() == 1 => ids[0],
                None => {
                    return Err(format!(
                        "{name}: the capture holds {} PMUs ({ids:?}); give the PMU's id",
                        ids.len()
                    ))
                }
            };
            let station = s
                .config
                .as_ref()
                .and_then(|c| c.pmus.iter().find(|p| p.id == pid))
                .map(|p| p.station.clone());
            let mut reps: Vec<_> = s.reports.iter().filter(|r| r.pmu == pid).cloned().collect();
            reps.sort_by(|a, b| a.t_unix_s.total_cmp(&b.t_unix_s));
            let sum = CaptureSummary {
                id: pid,
                station,
                reports: reps.len(),
                sync_error_reports: reps.iter().filter(|r| r.sync_error).count(),
                sync_losses: kshana::c37118::sync_losses(&reps),
                max_unlocked_code: reps.iter().map(|r| r.unlocked).max().unwrap_or(0),
                worst_claimed_quality_s: reps
                    .iter()
                    .filter_map(|r| pmu_tq_bound(r.pmu_time_quality))
                    .reduce(f64::max),
                clock_fault_reports: reps.iter().filter(|r| r.msg_time_quality == 15).count(),
                leap_pending_reports: reps.iter().filter(|r| r.leap_pending).count(),
                bad_crc: s.bad_crc,
                data_without_config: s.data_without_config,
                size_mismatch: s.size_mismatch,
            };
            (reps, Some(sum))
        }
    };
    if readings.is_empty() && capture.is_none() {
        return Err(format!("{name}: give a clock log, a capture or both"));
    }
    Ok(LoadedPmu {
        readings,
        reports,
        capture,
    })
}

fn nearest_report(
    reps: &[kshana::c37118::TimeReport],
    t: f64,
    tol: f64,
) -> Option<&kshana::c37118::TimeReport> {
    let i = reps.partition_point(|r| r.t_unix_s < t);
    [i.wrapping_sub(1), i]
        .into_iter()
        .filter_map(|j| reps.get(j))
        .filter(|r| (r.t_unix_s - t).abs() <= tol)
        .min_by(|a, b| (a.t_unix_s - t).abs().total_cmp(&(b.t_unix_s - t).abs()))
}

/// The claimed time-quality bound of a report: the PMU code, else the message code, else
/// none (locked, unstated).
fn claimed_bound(r: &kshana::c37118::TimeReport) -> Option<f64> {
    pmu_tq_bound(r.pmu_time_quality).or_else(|| msg_tq_bound(r.msg_time_quality))
}

pub(crate) fn pmu_evidence(
    name: &str,
    l: &LoadedPmu,
    budget: f64,
    f0: f64,
    tol: f64,
) -> PmuEvidence {
    let rs = &l.readings;
    let max_te = rs.iter().map(|r| r.offset.abs()).reduce(f64::max);
    let beyond = beyond_intervals(rs, budget);
    let (mut silent, mut exceeded, mut matched) = (0, 0, 0);
    for r in rs {
        let Some(rep) = nearest_report(&l.reports, r.t, tol) else {
            continue;
        };
        matched += 1;
        let claim = claimed_bound(rep);
        let over_claim = claim.is_some_and(|c| r.offset.abs() > c);
        if over_claim {
            exceeded += 1;
        }
        if r.offset.abs() > budget
            && !rep.sync_error
            && rep.msg_time_quality != 15
            && claim.is_none_or(|c| c < r.offset.abs())
        {
            silent += 1;
        }
    }
    let n_beyond: usize = beyond.iter().map(|i| i.readings).sum();
    let status = if rs.is_empty() {
        if l.capture.as_ref().is_some_and(|c| c.sync_error_reports > 0) {
            "flags-only"
        } else {
            "no-clock-evidence"
        }
    } else if n_beyond == 0 {
        "within-budget"
    } else if silent > 0 {
        "beyond-budget-flags-silent"
    } else {
        "beyond-budget"
    };
    PmuEvidence {
        name: name.into(),
        budget_s: budget,
        clock_readings: rs.len(),
        first_unix: rs.first().map(|r| r.t),
        last_unix: rs.last().map(|r| r.t),
        max_abs_te_s: max_te,
        max_abs_phase_error_deg: max_te.map(|t| phase_error_deg(t, f0)),
        max_tve_from_timing: max_te.map(|t| tve_of_phase(phase_error_deg(t, f0))),
        beyond_budget: beyond,
        beyond_budget_readings: n_beyond,
        flags_silent_readings: silent,
        quality_claim_exceeded_readings: exceeded,
        matched_readings: matched,
        capture: l.capture.clone(),
        status: status.into(),
    }
}

/// Assess every PMU of a request (paths relative to `base`).
pub fn assess_pmus(req: &PmuRequest, base: &Path) -> Result<PmuReport, String> {
    if req.pmu.is_empty() {
        return Err("the request lists no PMU".into());
    }
    let budget = timing_budget(req.nominal_hz, req.tve_budget, req.timing_share)?;
    let f0 = req.nominal_hz;
    let mut loaded = Vec::new();
    let mut pmus = Vec::new();
    for p in &req.pmu {
        let l = load_pmu(&p.name, p.id, p.capture.as_deref(), p.clock.as_ref(), base)?;
        let b = p.budget_s.unwrap_or(budget);
        pmus.push(pmu_evidence(&p.name, &l, b, f0, req.match_tolerance_s));
        loaded.push(l);
    }
    let mut events = Vec::new();
    for e in &req.event {
        if !nonneg(e.end_unix - e.start_unix) {
            return Err(format!("event {}: end_unix before start_unix", e.name));
        }
        let at: Vec<PmuAtEvent> = req
            .pmu
            .iter()
            .zip(&loaded)
            .map(|(p, l)| {
                let mut te: Vec<f64> = l
                    .readings
                    .iter()
                    .filter(|r| r.t >= e.start_unix && r.t <= e.end_unix)
                    .map(|r| r.offset)
                    .collect();
                let n = te.len();
                let mx = te.iter().map(|x| x.abs()).reduce(f64::max);
                let med = median(&mut te);
                PmuAtEvent {
                    name: p.name.clone(),
                    readings: n,
                    median_te_s: med,
                    max_abs_te_s: mx,
                    phase_error_deg: med.map(|m| phase_error_deg(m, f0)),
                    sync_error_reports: l
                        .reports
                        .iter()
                        .filter(|r| r.t_unix_s >= e.start_unix && r.t_unix_s <= e.end_unix)
                        .filter(|r| r.sync_error)
                        .count(),
                }
            })
            .collect();
        let mut pairs = Vec::new();
        for i in 0..at.len() {
            for j in i + 1..at.len() {
                if let (Some(a), Some(b)) = (at[i].median_te_s, at[j].median_te_s) {
                    pairs.push(PairAtEvent {
                        a: at[i].name.clone(),
                        b: at[j].name.clone(),
                        angle_difference_error_deg: phase_error_deg(a - b, f0),
                    });
                }
            }
        }
        events.push(EventAngles {
            name: e.name.clone(),
            start_unix: e.start_unix,
            end_unix: e.end_unix,
            pmus: at,
            pairs,
        });
    }
    let mut counts = BTreeMap::new();
    for p in &pmus {
        *counts.entry(p.status.clone()).or_insert(0) += 1;
    }
    let beyond: Vec<&str> = pmus
        .iter()
        .filter(|p| p.status.starts_with("beyond"))
        .map(|p| p.name.as_str())
        .collect();
    let silent: Vec<&str> = pmus
        .iter()
        .filter(|p| p.status == "beyond-budget-flags-silent")
        .map(|p| p.name.as_str())
        .collect();
    let verdict = if beyond.is_empty() {
        format!(
            "{} PMU(s): no clock reading beyond the timing budget of {} ({:.4}° at {} Hz)",
            pmus.len(),
            h(budget),
            phase_error_deg(budget, f0),
            f0
        )
    } else {
        format!(
            "{} of {} PMU(s) beyond the timing budget of {} ({:.4}° at {} Hz): {}{}",
            beyond.len(),
            pmus.len(),
            h(budget),
            phase_error_deg(budget, f0),
            f0,
            beyond.join(", "),
            if silent.is_empty() {
                String::new()
            } else {
                format!(
                    "; the PMU's own sync flags stayed silent at {}",
                    silent.join(", ")
                )
            }
        )
    };
    Ok(PmuReport {
        format: PMU_FORMAT,
        label: LABEL,
        title: req.title.clone(),
        nominal_hz: f0,
        tve_budget: req.tve_budget,
        timing_share: req.timing_share,
        timing_budget_s: budget,
        budget_basis: format!(
            "{TVE_BASIS}; timing share {} of TVE {}",
            req.timing_share, req.tve_budget
        ),
        pmus,
        events,
        counts,
        verdict,
    })
}

// ---------------------------------------------------------------------------------------
// Gradual-spoof (drift) detection (J02).

fn d_window() -> f64 {
    300.0
}
fn d_noise() -> f64 {
    50e-9
}
fn d_rate() -> f64 {
    1e-9
}
fn d_k5() -> f64 {
    5.0
}
fn d_persist() -> usize {
    3
}
fn d_kappa() -> f64 {
    0.5
}
fn d_hcusum() -> f64 {
    20.0
}
fn d_stepk() -> f64 {
    8.0
}
fn d_baseline() -> f64 {
    600.0
}

/// Detector settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftParams {
    /// Sliding window of the rate test (s).
    #[serde(default = "d_window")]
    pub window_s: f64,
    /// Standard uncertainty of one time-error reading (s).
    #[serde(default = "d_noise")]
    pub noise_s: f64,
    /// The clock's largest plausible rate of time error (s/s) when nothing is wrong.
    #[serde(default = "d_rate")]
    pub allowed_rate: f64,
    /// Coverage factor of the rate test.
    #[serde(default = "d_k5")]
    pub k: f64,
    /// Consecutive rate-test exceedances before an alarm.
    #[serde(default = "d_persist")]
    pub persist: usize,
    /// CUSUM reference value and threshold (in units of `noise_s`).
    #[serde(default = "d_kappa")]
    pub cusum_kappa: f64,
    #[serde(default = "d_hcusum")]
    pub cusum_h: f64,
    /// A single increment beyond `step_k·√2·noise_s` is a step.
    #[serde(default = "d_stepk")]
    pub step_k: f64,
    /// The first seconds of the series set the baseline level.
    #[serde(default = "d_baseline")]
    pub baseline_s: f64,
}

impl Default for DriftParams {
    fn default() -> Self {
        DriftParams {
            window_s: d_window(),
            noise_s: d_noise(),
            allowed_rate: d_rate(),
            k: d_k5(),
            persist: d_persist(),
            cusum_kappa: d_kappa(),
            cusum_h: d_hcusum(),
            step_k: d_stepk(),
            baseline_s: d_baseline(),
        }
    }
}

impl DriftParams {
    fn check(&self) -> Result<(), String> {
        let pos_ = [
            self.window_s,
            self.noise_s,
            self.k,
            self.cusum_h,
            self.step_k,
        ];
        if pos_.iter().any(|x| !pos(*x))
            || !nonneg(self.allowed_rate)
            || !nonneg(self.cusum_kappa)
            || !nonneg(self.baseline_s)
            || self.persist == 0
        {
            return Err("drift settings: window_s, noise_s, k, cusum_h and step_k must be > 0; allowed_rate, cusum_kappa and baseline_s >= 0; persist >= 1".into());
        }
        Ok(())
    }
}

/// Two PMUs on one bus (or electrically very close), one of them on an independent time
/// source: CSV `unix_time,angle_a_deg,angle_b_deg`. The change of `θa − θb` from its
/// baseline is read as a time error of PMU a against PMU b.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnglePairSpec {
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriftRequest {
    #[serde(default)]
    pub name: Option<String>,
    /// Needed for an angle pair and for the phase view.
    #[serde(default)]
    pub nominal_hz: Option<f64>,
    /// The time-error budget whose use is the deadline for detection (s); default from the
    /// TVE criterion when `nominal_hz` is given.
    #[serde(default)]
    pub budget_s: Option<f64>,
    #[serde(default)]
    pub clock: Option<ClockLogSpec>,
    #[serde(default)]
    pub angle_pair: Option<AnglePairSpec>,
    #[serde(default)]
    pub detector: DriftParams,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Step {
    pub t_unix: f64,
    pub size_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DriftReport {
    pub format: &'static str,
    pub label: &'static str,
    pub name: Option<String>,
    /// `clock` or `angle-pair`.
    pub input: String,
    pub samples: usize,
    pub first_unix: f64,
    pub last_unix: f64,
    pub baseline_s: f64,
    pub detector: DriftParams,
    /// `gradual-drift`, `step`, `level-shift` or `none`.
    pub kind: String,
    pub first_alarm_unix: Option<f64>,
    pub cusum_alarm_unix: Option<f64>,
    pub rate_alarm_unix: Option<f64>,
    /// The CUSUM change point: where the statistic last left zero before its alarm.
    pub estimated_onset_unix: Option<f64>,
    /// The rate fitted from the estimated onset to one window past the first alarm (s/s).
    pub rate_after_alarm: Option<f64>,
    pub te_at_alarm_s: Option<f64>,
    pub phase_at_alarm_deg: Option<f64>,
    pub steps: Vec<Step>,
    pub budget_s: Option<f64>,
    pub budget_reached_unix: Option<f64>,
    /// Seconds from the first alarm to the error using up the budget (positive: the alarm
    /// came first).
    pub warning_s: Option<f64>,
    pub max_abs_te_s: f64,
    pub verdict: String,
    #[serde(skip)]
    pub trace: Vec<[f64; 6]>,
}

/// Read an angle-pair CSV into (t, time error of a against b).
pub fn read_angle_pair(text: &str, f0: f64) -> Result<Vec<(f64, f64)>, String> {
    let mut v = Vec::new();
    for l in text.lines() {
        let c: Vec<&str> = l.split([',', ';']).map(str::trim).collect();
        let (Some(t), Some(a), Some(b)) = (
            c.first().and_then(|x| x.parse::<f64>().ok()),
            c.get(1).and_then(|x| x.parse::<f64>().ok()),
            c.get(2).and_then(|x| x.parse::<f64>().ok()),
        ) else {
            continue;
        };
        let mut d = (a - b) % 360.0;
        if d > 180.0 {
            d -= 360.0;
        } else if d <= -180.0 {
            d += 360.0;
        }
        v.push((t, d));
    }
    if v.is_empty() {
        return Err("angle pair: no reading".into());
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Unwrap, then convert to time.
    let mut out = Vec::with_capacity(v.len());
    let mut prev = v[0].1;
    let mut acc = v[0].1;
    for (t, d) in v {
        let mut dd = d - prev;
        if dd > 180.0 {
            dd -= 360.0;
        } else if dd < -180.0 {
            dd += 360.0;
        }
        acc += dd;
        prev = d;
        out.push((t, acc / (360.0 * f0)));
    }
    Ok(out)
}

/// Least-squares slope of `(t, x)` points (`None` below three points).
fn ols_slope(w: &[(f64, f64)]) -> Option<f64> {
    if w.len() < 3 {
        return None;
    }
    let t0 = w[0].0;
    let n = w.len() as f64;
    let tm = w.iter().map(|x| x.0 - t0).sum::<f64>() / n;
    let xm = w.iter().map(|x| x.1).sum::<f64>() / n;
    let sxx: f64 = w.iter().map(|x| (x.0 - t0 - tm).powi(2)).sum();
    let sxy: f64 = w.iter().map(|x| (x.0 - t0 - tm) * (x.1 - xm)).sum();
    (sxx > 0.0).then(|| sxy / sxx)
}

/// Run the detector over `(unix time, time error)` samples (time order).
pub fn detect_drift(series: &[(f64, f64)], p: &DriftParams, budget: Option<f64>) -> DriftReport {
    let n = series.len();
    let t0 = series.first().map(|x| x.0).unwrap_or(0.0);
    let mut base: Vec<f64> = series
        .iter()
        .take_while(|x| x.0 - t0 <= p.baseline_s)
        .map(|x| x.1)
        .collect();
    let b = median(&mut base).unwrap_or(0.0);
    let sig = p.noise_s;
    let (mut sp, mut sm) = (0.0f64, 0.0f64);
    let (mut zero_p, mut zero_m) = (t0, t0);
    let mut cusum_alarm: Option<(f64, f64)> = None;
    let mut rate_alarm: Option<f64> = None;
    let mut rate_at: Option<f64> = None;
    let mut run = 0usize;
    let mut lo = 0usize;
    let mut steps = Vec::new();
    let mut trace = Vec::with_capacity(n);
    for i in 0..n {
        let (t, x) = series[i];
        // Level CUSUM against the baseline.
        let z = (x - b) / sig;
        sp = (sp + z - p.cusum_kappa).max(0.0);
        sm = (sm - z - p.cusum_kappa).max(0.0);
        if sp == 0.0 {
            zero_p = t;
        }
        if sm == 0.0 {
            zero_m = t;
        }
        if cusum_alarm.is_none() && (sp > p.cusum_h || sm > p.cusum_h) {
            cusum_alarm = Some((t, if sp > p.cusum_h { zero_p } else { zero_m }));
        }
        // Step test.
        if i > 0 {
            let (tp, xp) = series[i - 1];
            let dx = x - xp;
            if dx.abs() > p.step_k * std::f64::consts::SQRT_2 * sig + p.allowed_rate * (t - tp) {
                steps.push(Step {
                    t_unix: t,
                    size_s: dx,
                });
            }
        }
        // Sliding-window rate test.
        while series[lo].0 < t - p.window_s {
            lo += 1;
        }
        let w = &series[lo..=i];
        let (mut slope, mut us) = (f64::NAN, f64::NAN);
        if w.len() >= 3 && t - w[0].0 >= p.window_s / 2.0 {
            let tm = w.iter().map(|x| x.0 - t).sum::<f64>() / w.len() as f64;
            let xm = w.iter().map(|x| x.1).sum::<f64>() / w.len() as f64;
            let sxx: f64 = w.iter().map(|x| (x.0 - t - tm).powi(2)).sum();
            let sxy: f64 = w.iter().map(|x| (x.0 - t - tm) * (x.1 - xm)).sum();
            if sxx > 0.0 {
                slope = sxy / sxx;
                let rss: f64 = w
                    .iter()
                    .map(|x| (x.1 - xm - slope * (x.0 - t - tm)).powi(2))
                    .sum();
                let s_res = (rss / (w.len() as f64 - 2.0)).sqrt();
                us = sig.max(s_res) / sxx.sqrt();
                // A step inside the window inflates the residuals: the rate test then waits.
                if slope.abs() > p.allowed_rate + p.k * us {
                    run += 1;
                    if run >= p.persist && rate_alarm.is_none() {
                        rate_alarm = Some(t);
                        rate_at = Some(slope);
                    }
                } else {
                    run = 0;
                }
            }
        }
        let alarm = cusum_alarm.is_some() || rate_alarm.is_some();
        trace.push([t, x, slope, us, sp.max(sm), if alarm { 1.0 } else { 0.0 }]);
    }
    let first = match (cusum_alarm.map(|c| c.0), rate_alarm) {
        (Some(a), Some(r)) => Some(a.min(r)),
        (a, r) => a.or(r),
    };
    let onset = cusum_alarm.map(|c| c.1);
    let step_near = first.and_then(|f| {
        steps
            .iter()
            .find(|s| s.t_unix <= f && s.t_unix >= onset.unwrap_or(f) - p.window_s)
    });
    let kind = match first {
        None => "none",
        Some(_) if step_near.is_some() => "step",
        Some(_) if rate_alarm.is_some() => "gradual-drift",
        Some(_) => "level-shift",
    };
    let at = first.and_then(|f| series.iter().find(|x| x.0 >= f).copied());
    // The rate fitted from the estimated onset (or the alarm) to one window past the alarm.
    let rate_after = first.and_then(|f| {
        let from = onset.unwrap_or(f).min(f);
        let w: Vec<(f64, f64)> = series
            .iter()
            .filter(|x| x.0 >= from && x.0 <= f + p.window_s)
            .copied()
            .collect();
        ols_slope(&w)
    });
    let budget_reached = budget.and_then(|bd| series.iter().find(|x| x.1.abs() > bd).map(|x| x.0));
    let warning = match (first, budget_reached) {
        (Some(f), Some(r)) => Some(r - f),
        _ => None,
    };
    let max_te = series.iter().map(|x| x.1.abs()).fold(0.0, f64::max);
    let verdict = match (first, kind) {
        (None, _) => format!("no alarm over {} samples (max |TE| {})", n, h(max_te)),
        (Some(f), k) => {
            let w = match (warning, budget) {
                (Some(w), Some(bd)) if w >= 0.0 => format!(
                    "; the alarm came {} before |TE| reached the budget of {}",
                    super::fleet::human_duration(Some(w)),
                    h(bd)
                ),
                (Some(w), Some(bd)) => format!(
                    "; |TE| had already reached the budget of {} {} before the alarm",
                    h(bd),
                    super::fleet::human_duration(Some(-w))
                ),
                (None, Some(bd)) => format!("; |TE| never reached the budget of {}", h(bd)),
                _ => String::new(),
            };
            format!(
                "{k} alarm at unix {f:.0} (estimated onset {}), TE at alarm {}{}{w}",
                onset
                    .map(|o| format!("unix {o:.0}"))
                    .unwrap_or_else(|| "unknown".into()),
                at.map(|a| h(a.1)).unwrap_or_default(),
                rate_after
                    .map(|r| format!(", rate {:.3} ns/s", r * 1e9))
                    .unwrap_or_default()
            )
        }
    };
    DriftReport {
        format: DRIFT_FORMAT,
        label: LABEL,
        name: None,
        input: "clock".into(),
        samples: n,
        first_unix: t0,
        last_unix: series.last().map(|x| x.0).unwrap_or(t0),
        baseline_s: p.baseline_s,
        detector: p.clone(),
        kind: kind.into(),
        first_alarm_unix: first,
        cusum_alarm_unix: cusum_alarm.map(|c| c.0),
        rate_alarm_unix: rate_alarm,
        estimated_onset_unix: onset,
        rate_after_alarm: rate_after.or(rate_at),
        te_at_alarm_s: at.map(|a| a.1),
        phase_at_alarm_deg: None,
        steps,
        budget_s: budget,
        budget_reached_unix: budget_reached,
        warning_s: warning,
        max_abs_te_s: max_te,
        verdict,
        trace,
    }
}

/// Run a drift request (paths relative to `base`).
pub fn run_drift(req: &DriftRequest, base: &Path) -> Result<DriftReport, String> {
    req.detector.check()?;
    let (series, input) = match (&req.clock, &req.angle_pair) {
        (Some(c), None) => (
            read_clock("clock", c, base)?
                .into_iter()
                .map(|r| (r.t, r.offset))
                .collect::<Vec<_>>(),
            "clock",
        ),
        (None, Some(a)) => {
            let f0 = req
                .nominal_hz
                .ok_or("an angle pair needs nominal_hz to turn degrees into time")?;
            // The standing angle difference is the grid's, not the clocks': remove it.
            let mut v = read_angle_pair(&read_text(base, &a.path)?, f0)?;
            let t0 = v[0].0;
            let mut b: Vec<f64> = v
                .iter()
                .take_while(|x| x.0 - t0 <= req.detector.baseline_s)
                .map(|x| x.1)
                .collect();
            let b = median(&mut b).unwrap_or(0.0);
            v.iter_mut().for_each(|x| x.1 -= b);
            (v, "angle-pair")
        }
        _ => return Err("give exactly one of clock and angle_pair".into()),
    };
    let budget = match (req.budget_s, req.nominal_hz) {
        (Some(b), _) => Some(b),
        (None, Some(f)) => Some(timing_budget(f, TVE_CRITERION, 1.0)?),
        _ => None,
    };
    let mut r = detect_drift(&series, &req.detector, budget);
    r.name = req.name.clone();
    r.input = input.into();
    if let (Some(f), Some(te)) = (req.nominal_hz, r.te_at_alarm_s) {
        r.phase_at_alarm_deg = Some(phase_error_deg(te, f));
    }
    Ok(r)
}

/// The detector trace as CSV.
pub fn drift_csv(r: &DriftReport) -> String {
    let mut s = String::from("unix_time,te_s,rate,u_rate,cusum,alarm\n");
    for x in &r.trace {
        s.push_str(&format!(
            "{},{:e},{},{},{:.3},{}\n",
            x[0],
            x[1],
            if x[2].is_finite() {
                format!("{:e}", x[2])
            } else {
                String::new()
            },
            if x[3].is_finite() {
                format!("{:e}", x[3])
            } else {
                String::new()
            },
            x[4],
            x[5] as u8
        ));
    }
    s
}

// ---------------------------------------------------------------------------------------
// SCADA time-stamp integrity (J07).

/// The coarsest decimal step (1 s down to 10 µs) every stamp sits on; 1e-6 when finer.
/// An upper bound on the resolution the source stamps with.
pub fn apparent_resolution(stamps: impl Iterator<Item = f64>) -> Option<f64> {
    let fr: Vec<f64> = stamps.map(|x| x - x.floor()).collect();
    if fr.is_empty() {
        return None;
    }
    for q in [1.0f64, 0.1, 0.01, 1e-3, 1e-4, 1e-5] {
        let tol = (0.1 * q).min(5e-7);
        if fr.iter().all(|f| ((f / q).round() * q - f).abs() <= tol) {
            return Some(q);
        }
    }
    Some(1e-6)
}

fn d_ahead() -> f64 {
    1e-3
}
fn d_maxlat() -> f64 {
    10.0
}
fn d_stepsc() -> f64 {
    0.5
}
fn d_win_sc() -> f64 {
    3600.0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScadaSpec {
    /// CSV `device,source_unix,receive_unix[,seq]` (a header line is skipped).
    pub path: String,
    /// A device stamp later than the master's receive time by more than this is "ahead" (s).
    #[serde(default = "d_ahead")]
    pub ahead_tolerance_s: f64,
    /// Latency beyond this is stale (s).
    #[serde(default = "d_maxlat")]
    pub max_latency_s: f64,
    /// A change of the latency floor between windows beyond this is a clock step (s).
    #[serde(default = "d_stepsc")]
    pub step_s: f64,
    /// Window of the latency floor (s).
    #[serde(default = "d_win_sc")]
    pub window_s: f64,
    /// The time-stamp resolution the records need (s), e.g. 0.001 for sequence-of-events.
    #[serde(default)]
    pub required_resolution_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScadaDevice {
    pub device: String,
    pub records: usize,
    pub backwards: usize,
    /// Consecutive records carrying the same device stamp (a stuck or repeated stamp).
    pub duplicates: usize,
    pub ahead: usize,
    pub stale: usize,
    pub median_latency_s: f64,
    pub min_latency_s: f64,
    /// Drift of the latency floor (s per day): the device clock gaining (negative) or losing.
    pub floor_drift_s_per_day: Option<f64>,
    pub floor_steps: Vec<Step>,
    pub apparent_resolution_s: Option<f64>,
    /// `ok` or the findings, e.g. `clock-ahead`, `clock-step`, `clock-drift`, `backwards`,
    /// `duplicates`, `stale`, `coarse-resolution`.
    pub findings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScadaReport {
    pub format: &'static str,
    pub label: &'static str,
    pub spec: ScadaSpec,
    pub devices: Vec<ScadaDevice>,
    pub verdict: String,
}

/// Check a SCADA log (path relative to `base`).
pub fn check_scada(spec: &ScadaSpec, base: &Path) -> Result<ScadaReport, String> {
    check_scada_text(spec, &read_text(base, &spec.path)?)
}

/// Check SCADA records given as text.
pub fn check_scada_text(spec: &ScadaSpec, text: &str) -> Result<ScadaReport, String> {
    if !pos(spec.window_s) || !pos(spec.step_s) || !pos(spec.max_latency_s) {
        return Err("scada: window_s, step_s and max_latency_s must be > 0".into());
    }
    let mut per: BTreeMap<String, Vec<(f64, f64, Option<u64>)>> = BTreeMap::new();
    for l in text.lines() {
        let c: Vec<&str> = l.split([',', ';']).map(str::trim).collect();
        if c.len() < 3 || c[0].is_empty() || c[0].starts_with('#') {
            continue;
        }
        let (Ok(s), Ok(r)) = (c[1].parse::<f64>(), c[2].parse::<f64>()) else {
            continue;
        };
        let seq = c.get(3).and_then(|x| x.parse::<u64>().ok());
        per.entry(c[0].to_string()).or_default().push((s, r, seq));
    }
    if per.is_empty() {
        return Err("scada: no record (expected device,source_unix,receive_unix[,seq])".into());
    }
    let mut devices = Vec::new();
    for (dev, mut v) in per {
        // Arrival order: by sequence number when every record has one, else receive time.
        if v.iter().all(|x| x.2.is_some()) {
            v.sort_by_key(|x| x.2);
        } else {
            v.sort_by(|a, b| a.1.total_cmp(&b.1));
        }
        let mut backwards = 0;
        let mut duplicates = 0;
        for w in v.windows(2) {
            if w[1].0 < w[0].0 {
                backwards += 1;
            } else if w[1].0 == w[0].0 {
                duplicates += 1;
            }
        }
        let lat: Vec<f64> = v.iter().map(|x| x.1 - x.0).collect();
        let ahead = lat.iter().filter(|l| **l < -spec.ahead_tolerance_s).count();
        let stale = lat.iter().filter(|l| **l > spec.max_latency_s).count();
        let mut lv = lat.clone();
        let med = median(&mut lv).unwrap_or(f64::NAN);
        let minl = lat.iter().copied().fold(f64::INFINITY, f64::min);
        // Latency floor per window of receive time.
        let mut by_t: Vec<(f64, f64)> = v.iter().map(|x| (x.1, x.1 - x.0)).collect();
        by_t.sort_by(|a, b| a.0.total_cmp(&b.0));
        let t0 = by_t[0].0;
        let mut floors: BTreeMap<i64, (f64, f64)> = BTreeMap::new();
        for (t, l) in &by_t {
            let k = ((t - t0) / spec.window_s).floor() as i64;
            let e = floors.entry(k).or_insert((*t, f64::INFINITY));
            if *l < e.1 {
                *e = (*t, *l);
            }
        }
        let fl: Vec<(f64, f64)> = floors.values().copied().collect();
        let mut floor_steps = Vec::new();
        for w in fl.windows(2) {
            if (w[1].1 - w[0].1).abs() > spec.step_s {
                floor_steps.push(Step {
                    t_unix: w[1].0,
                    size_s: w[0].1 - w[1].1,
                });
            }
        }
        // Drift: least squares over the floors between steps (first segment long enough).
        let drift = if fl.len() >= 3 && floor_steps.is_empty() {
            let tm = fl.iter().map(|x| x.0 - t0).sum::<f64>() / fl.len() as f64;
            let ym = fl.iter().map(|x| x.1).sum::<f64>() / fl.len() as f64;
            let sxx: f64 = fl.iter().map(|x| (x.0 - t0 - tm).powi(2)).sum();
            let sxy: f64 = fl.iter().map(|x| (x.0 - t0 - tm) * (x.1 - ym)).sum();
            (sxx > 0.0).then(|| sxy / sxx * 86_400.0)
        } else {
            None
        };
        let res = apparent_resolution(v.iter().map(|x| x.0));
        let mut findings = Vec::new();
        if ahead > 0 {
            findings.push("clock-ahead".to_string());
        }
        if !floor_steps.is_empty() {
            findings.push("clock-step".into());
        }
        // A floor moving by more than a step a day is a drifting clock.
        if drift.is_some_and(|d| d.abs() > spec.step_s) {
            findings.push("clock-drift".into());
        }
        if backwards > 0 {
            findings.push("backwards".into());
        }
        if duplicates > 0 {
            findings.push("duplicates".into());
        }
        if stale > 0 {
            findings.push("stale".into());
        }
        if let (Some(req), Some(r)) = (spec.required_resolution_s, res) {
            if r > req * 1.000_001 {
                findings.push("coarse-resolution".into());
            }
        }
        if findings.is_empty() {
            findings.push("ok".into());
        }
        devices.push(ScadaDevice {
            device: dev,
            records: v.len(),
            backwards,
            duplicates,
            ahead,
            stale,
            median_latency_s: med,
            min_latency_s: minl,
            floor_drift_s_per_day: drift,
            floor_steps,
            apparent_resolution_s: res,
            findings,
        });
    }
    let flagged: Vec<String> = devices
        .iter()
        .filter(|d| d.findings != ["ok"])
        .map(|d| format!("{} ({})", d.device, d.findings.join(", ")))
        .collect();
    let verdict = if flagged.is_empty() {
        format!("{} device(s): no time-stamp finding", devices.len())
    } else {
        format!(
            "{} of {} device(s) with time-stamp findings: {}",
            flagged.len(),
            devices.len(),
            flagged.join("; ")
        )
    };
    Ok(ScadaReport {
        format: SCADA_FORMAT,
        label: LABEL,
        spec: spec.clone(),
        devices,
        verdict,
    })
}

// ---------------------------------------------------------------------------------------
// NERC PRC-002 time-synchronisation evidence with resilience (J03).

fn d_gap() -> f64 {
    300.0
}

/// The device's holdover characterisation (the holdover evidence model's inputs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResilienceSpec {
    /// How long the device must hold the criterion without its reference (h).
    pub period_h: f64,
    pub oscillator: OscillatorSpec,
    #[serde(default)]
    pub initial_te_s: f64,
    #[serde(default)]
    pub u_initial_te_s: f64,
    #[serde(default)]
    pub coverage_k: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prc002Device {
    pub name: String,
    /// `ser`, `fr`, `dfr`, `ddr`, `pmu`, `relay` or other (informative).
    #[serde(default)]
    pub kind: Option<String>,
    pub clock: ClockLogSpec,
    #[serde(default)]
    pub resilience: Option<ResilienceSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prc002Request {
    pub site: String,
    #[serde(default)]
    pub title: Option<String>,
    /// The limit (s); default ±2 ms.
    #[serde(default)]
    pub limit_s: Option<f64>,
    /// A gap between readings longer than this is a monitoring gap (s).
    #[serde(default = "d_gap")]
    pub gap_s: f64,
    /// The period of the evidence (default: the span of the logs).
    #[serde(default)]
    pub period_start_unix: Option<f64>,
    #[serde(default)]
    pub period_end_unix: Option<f64>,
    pub device: Vec<Prc002Device>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gap {
    pub start_unix: f64,
    pub end_unix: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResilienceResult {
    pub period_s: f64,
    pub max_supported_holdover_s: Option<f64>,
    pub bound_at_period_s: f64,
    pub supported: bool,
    pub verdict: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Prc002Evidence {
    pub name: String,
    pub kind: Option<String>,
    pub readings: usize,
    pub max_abs_te_s: f64,
    pub share_within: f64,
    pub beyond: Vec<Interval>,
    pub gaps: Vec<Gap>,
    /// Seconds of the period covered by readings no further apart than `gap_s`.
    pub covered_s: f64,
    pub coverage: f64,
    /// `within`, `within-with-gaps`, `beyond`, `no-data`.
    pub status: String,
    pub resilience: Option<ResilienceResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Prc002Report {
    pub format: &'static str,
    pub label: &'static str,
    pub site: String,
    pub title: Option<String>,
    pub utc: String,
    pub limit_s: f64,
    pub limit_basis: String,
    pub period_start_unix: f64,
    pub period_end_unix: f64,
    pub devices: Vec<Prc002Evidence>,
    pub counts: BTreeMap<String, usize>,
    pub verdict: String,
}

/// Assess the request (paths relative to `base`).
pub fn assess_prc002(req: &Prc002Request, base: &Path, utc: &str) -> Result<Prc002Report, String> {
    if req.device.is_empty() {
        return Err("the request lists no device".into());
    }
    let limit = req.limit_s.unwrap_or(PRC002_LIMIT_S);
    if !pos(limit) || !pos(req.gap_s) {
        return Err("limit_s and gap_s must be > 0".into());
    }
    let mut logs = Vec::new();
    for d in &req.device {
        logs.push(read_clock(&d.name, &d.clock, base)?);
    }
    let ps = req.period_start_unix.unwrap_or_else(|| {
        logs.iter()
            .filter_map(|l| l.first().map(|r| r.t))
            .fold(f64::INFINITY, f64::min)
    });
    let pe = req.period_end_unix.unwrap_or_else(|| {
        logs.iter()
            .filter_map(|l| l.last().map(|r| r.t))
            .fold(f64::NEG_INFINITY, f64::max)
    });
    if !pos(pe - ps) {
        return Err("the period is empty (period_end_unix must be after period_start_unix)".into());
    }
    let mut devices = Vec::new();
    for (d, all) in req.device.iter().zip(&logs) {
        let rs: Vec<Reading> = all
            .iter()
            .filter(|r| r.t >= ps && r.t <= pe)
            .copied()
            .collect();
        let resilience = match &d.resilience {
            None => None,
            Some(s) => {
                let hs = HoldoverSpec {
                    site: d.name.clone(),
                    title: None,
                    preset: None,
                    period_h: Some(s.period_h),
                    limit_s: Some(limit),
                    coverage_k: s.coverage_k,
                    equipment_class: None,
                    initial_te_s: s.initial_te_s,
                    u_initial_te_s: s.u_initial_te_s,
                    oscillator: s.oscillator.clone(),
                };
                let r = holdover_evidence::assess(&hs, utc)
                    .map_err(|e| format!("{}: resilience: {e}", d.name))?;
                Some(ResilienceResult {
                    period_s: r.period_s,
                    max_supported_holdover_s: r.max_supported_holdover_s,
                    bound_at_period_s: r.bound_at_period_s,
                    supported: r.supported,
                    verdict: r.verdict,
                })
            }
        };
        if rs.is_empty() {
            devices.push(Prc002Evidence {
                name: d.name.clone(),
                kind: d.kind.clone(),
                readings: 0,
                max_abs_te_s: f64::NAN,
                share_within: 0.0,
                beyond: vec![],
                gaps: vec![Gap {
                    start_unix: ps,
                    end_unix: pe,
                }],
                covered_s: 0.0,
                coverage: 0.0,
                status: "no-data".into(),
                resilience,
            });
            continue;
        }
        let beyond = beyond_intervals(&rs, limit);
        let n_beyond: usize = beyond.iter().map(|i| i.readings).sum();
        let mut gaps = Vec::new();
        let mut covered = 0.0;
        if rs[0].t - ps > req.gap_s {
            gaps.push(Gap {
                start_unix: ps,
                end_unix: rs[0].t,
            });
        }
        for w in rs.windows(2) {
            let dt = w[1].t - w[0].t;
            if dt > req.gap_s {
                gaps.push(Gap {
                    start_unix: w[0].t,
                    end_unix: w[1].t,
                });
            } else {
                covered += dt;
            }
        }
        let last = rs[rs.len() - 1].t;
        if pe - last > req.gap_s {
            gaps.push(Gap {
                start_unix: last,
                end_unix: pe,
            });
        } else {
            covered += pe - last;
        }
        if rs[0].t - ps <= req.gap_s {
            covered += rs[0].t - ps;
        }
        let status = if n_beyond > 0 {
            "beyond"
        } else if gaps.is_empty() {
            "within"
        } else {
            "within-with-gaps"
        };
        devices.push(Prc002Evidence {
            name: d.name.clone(),
            kind: d.kind.clone(),
            readings: rs.len(),
            max_abs_te_s: rs.iter().map(|r| r.offset.abs()).fold(0.0, f64::max),
            share_within: (rs.len() - n_beyond) as f64 / rs.len() as f64,
            beyond,
            gaps,
            covered_s: covered,
            coverage: (covered / (pe - ps)).min(1.0),
            status: status.into(),
            resilience,
        });
    }
    let mut counts = BTreeMap::new();
    for d in &devices {
        *counts.entry(d.status.clone()).or_insert(0) += 1;
        if let Some(r) = &d.resilience {
            *counts
                .entry(if r.supported {
                    "resilience-supported".to_string()
                } else {
                    "resilience-not-supported".to_string()
                })
                .or_insert(0) += 1;
        }
    }
    let within = devices.iter().filter(|d| d.status == "within").count();
    let mut verdict = format!(
        "{within} of {} device(s) measured within ±{} for every reading of the period with no monitoring gap",
        devices.len(),
        h(limit)
    );
    let beyond: Vec<&str> = devices
        .iter()
        .filter(|d| d.status == "beyond")
        .map(|d| d.name.as_str())
        .collect();
    if !beyond.is_empty() {
        verdict.push_str(&format!("; beyond the limit: {}", beyond.join(", ")));
    }
    let gappy: Vec<&str> = devices
        .iter()
        .filter(|d| d.status == "within-with-gaps" || d.status == "no-data")
        .map(|d| d.name.as_str())
        .collect();
    if !gappy.is_empty() {
        verdict.push_str(&format!("; monitoring gaps: {}", gappy.join(", ")));
    }
    let weak: Vec<&str> = devices
        .iter()
        .filter(|d| d.resilience.as_ref().is_some_and(|r| !r.supported))
        .map(|d| d.name.as_str())
        .collect();
    if !weak.is_empty() {
        verdict.push_str(&format!(
            "; holdover not supported by the model for the stated period: {}",
            weak.join(", ")
        ));
    }
    Ok(Prc002Report {
        format: PRC002_FORMAT,
        label: LABEL,
        site: req.site.clone(),
        title: req.title.clone(),
        utc: utc.into(),
        limit_s: limit,
        limit_basis: if req.limit_s.is_some() {
            format!("the operator's limit; context: {PRC002_BASIS}")
        } else {
            PRC002_BASIS.into()
        },
        period_start_unix: ps,
        period_end_unix: pe,
        devices,
        counts,
        verdict,
    })
}

pub fn prc002_html(r: &Prc002Report) -> String {
    let rows = r
        .devices
        .iter()
        .map(|d| {
            vec![
                d.name.clone(),
                d.kind.clone().unwrap_or_default(),
                d.status.clone(),
                d.readings.to_string(),
                if d.max_abs_te_s.is_finite() {
                    h(d.max_abs_te_s)
                } else {
                    "-".into()
                },
                format!("{:.4} %", 100.0 * d.share_within),
                format!("{:.2} %", 100.0 * d.coverage),
                d.gaps.len().to_string(),
                d.resilience
                    .as_ref()
                    .map(|x| x.verdict.clone())
                    .unwrap_or_else(|| "not assessed".into()),
            ]
        })
        .collect();
    page(
        &format!("Recording-device time evidence: {}", r.site),
        r.label,
        &r.verdict,
        &[format!(
            "Limit ±{} ({}); period unix {:.0} to {:.0}; {}",
            h(r.limit_s),
            r.limit_basis,
            r.period_start_unix,
            r.period_end_unix,
            r.utc
        )],
        &[(
            "Devices".into(),
            vec![
                "Device",
                "Kind",
                "Status",
                "Readings",
                "max |TE|",
                "Within",
                "Coverage",
                "Gaps",
                "Holdover (model)",
            ],
            rows,
        )],
    )
}

// ---------------------------------------------------------------------------------------
// Grid event forensics (J04).

fn d_pre() -> f64 {
    3600.0
}
fn d_cov() -> f64 {
    60.0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDevice {
    pub name: String,
    /// `pmu`, `ser`, `dfr`, `relay`, `rtu` or other: a PMU's default budget comes from the
    /// TVE criterion, every other device's from the PRC-002 limit.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub id: Option<u16>,
    #[serde(default)]
    pub capture: Option<String>,
    #[serde(default)]
    pub clock: Option<ClockLogSpec>,
    #[serde(default)]
    pub budget_s: Option<f64>,
    /// The device's name in the SCADA and sequence-of-events logs (default: `name`).
    #[serde(default)]
    pub log_name: Option<String>,
}

/// An expected order of two recorded events.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderRule {
    pub before: String,
    pub after: String,
    /// The physical minimum gap between them (s).
    #[serde(default)]
    pub min_gap_s: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoeSpec {
    /// CSV `device,unix_time,label`.
    pub path: String,
    pub order: Vec<OrderRule>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventRequest {
    pub name: String,
    pub start_unix: f64,
    pub end_unix: f64,
    /// Seconds before the event searched for a drift onset.
    #[serde(default = "d_pre")]
    pub pre_s: f64,
    pub nominal_hz: f64,
    #[serde(default = "d_tve")]
    pub tve_budget: f64,
    /// The clock log must reach within this of both ends of the window (s).
    #[serde(default = "d_cov")]
    pub coverage_gap_s: f64,
    #[serde(default = "d_tol")]
    pub match_tolerance_s: f64,
    #[serde(default)]
    pub detector: DriftParams,
    pub device: Vec<EventDevice>,
    #[serde(default)]
    pub scada: Option<ScadaSpec>,
    #[serde(default)]
    pub soe: Option<SoeSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OrderCheck {
    pub before: String,
    pub after: String,
    pub recorded_gap_s: Option<f64>,
    pub corrected_gap_s: Option<f64>,
    /// `consistent`, `inverted-explained-by-timing`, `inverted-not-explained`,
    /// `inverted-clock-unknown`, `consistent-only-before-correction` or `missing`.
    pub status: String,
    pub devices: Vec<String>,
    /// Each device's measured time error at its stamp (s), when known.
    pub corrections_s: Vec<Option<f64>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DeviceFinding {
    pub name: String,
    pub kind: Option<String>,
    pub budget_s: f64,
    pub clock_covers_window: bool,
    pub max_abs_te_in_window_s: Option<f64>,
    pub phase_error_deg: Option<f64>,
    pub drift_alarm_unix: Option<f64>,
    pub drift_kind: Option<String>,
    pub sync_error_reports_in_window: usize,
    pub scada_findings: Vec<String>,
    pub findings: Vec<String>,
    /// `timing-implicated`, `timing-suspect`, `timing-not-implicated` or `insufficient`.
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventReport {
    pub format: &'static str,
    pub label: &'static str,
    pub name: String,
    pub utc: String,
    pub start_unix: f64,
    pub end_unix: f64,
    pub pre_s: f64,
    pub nominal_hz: f64,
    pub devices: Vec<DeviceFinding>,
    pub order: Vec<OrderCheck>,
    pub counts: BTreeMap<String, usize>,
    pub verdict: String,
}

/// Assess an event (paths relative to `base`).
pub fn assess_event(req: &EventRequest, base: &Path, utc: &str) -> Result<EventReport, String> {
    if req.device.is_empty() {
        return Err("the request lists no device".into());
    }
    if !nonneg(req.end_unix - req.start_unix) || !nonneg(req.pre_s) {
        return Err("end_unix must not be before start_unix; pre_s >= 0".into());
    }
    req.detector.check()?;
    let pmu_budget = timing_budget(req.nominal_hz, req.tve_budget, 1.0)?;
    let f0 = req.nominal_hz;
    let scada = match &req.scada {
        Some(s) => Some(check_scada(s, base)?),
        None => None,
    };
    let (ws, we) = (req.start_unix, req.end_unix);
    let mut loaded = Vec::new();
    let mut devices = Vec::new();
    for d in &req.device {
        let l =
            load_pmu(&d.name, d.id, d.capture.as_deref(), d.clock.as_ref(), base).or_else(|e| {
                // A device with neither log still gets a finding (insufficient).
                if d.capture.is_none() && d.clock.is_none() {
                    Ok(LoadedPmu {
                        readings: vec![],
                        reports: vec![],
                        capture: None,
                    })
                } else {
                    Err(e)
                }
            })?;
        let is_pmu = d.kind.as_deref() == Some("pmu");
        let budget = d
            .budget_s
            .unwrap_or(if is_pmu { pmu_budget } else { PRC002_LIMIT_S });
        let inw: Vec<&Reading> = l
            .readings
            .iter()
            .filter(|r| r.t >= ws && r.t <= we)
            .collect();
        let covers = !l.readings.is_empty()
            && l.readings
                .iter()
                .any(|r| r.t <= ws + req.coverage_gap_s && r.t >= ws - req.coverage_gap_s)
            && l.readings
                .iter()
                .any(|r| r.t >= we - req.coverage_gap_s && r.t <= we + req.coverage_gap_s);
        let max_te = inw.iter().map(|r| r.offset.abs()).reduce(f64::max);
        let pre: Vec<(f64, f64)> = l
            .readings
            .iter()
            .filter(|r| r.t >= ws - req.pre_s && r.t <= we)
            .map(|r| (r.t, r.offset))
            .collect();
        let drift = (pre.len() >= 3).then(|| detect_drift(&pre, &req.detector, Some(budget)));
        let sync_in = l
            .reports
            .iter()
            .filter(|r| r.t_unix_s >= ws && r.t_unix_s <= we && r.sync_error)
            .count();
        let log_name = d.log_name.clone().unwrap_or_else(|| d.name.clone());
        let scada_f: Vec<String> = scada
            .as_ref()
            .and_then(|s| s.devices.iter().find(|x| x.device == log_name))
            .map(|x| x.findings.iter().filter(|f| *f != "ok").cloned().collect())
            .unwrap_or_default();
        let mut findings = Vec::new();
        let mut implicated = false;
        let mut suspect = false;
        if let Some(m) = max_te {
            if m > budget {
                implicated = true;
                findings.push(format!(
                    "measured |TE| {} in the window, beyond the budget {}",
                    h(m),
                    h(budget)
                ));
            }
        }
        if sync_in > 0 {
            implicated = true;
            findings.push(format!(
                "{sync_in} C37.118 report(s) in the window flagged not in sync"
            ));
        }
        if let Some(dr) = &drift {
            if let Some(a) = dr.first_alarm_unix {
                suspect = true;
                findings.push(format!(
                    "{} alarm at unix {a:.0} (estimated onset {})",
                    dr.kind,
                    dr.estimated_onset_unix
                        .map(|o| format!("unix {o:.0}"))
                        .unwrap_or_else(|| "unknown".into())
                ));
            }
        }
        if scada_f
            .iter()
            .any(|f| f.starts_with("clock") || f == "backwards")
        {
            suspect = true;
            findings.push(format!("SCADA time-stamp findings: {}", scada_f.join(", ")));
        }
        let status = if implicated {
            "timing-implicated"
        } else if !covers && sync_in == 0 {
            "insufficient"
        } else if suspect {
            "timing-suspect"
        } else {
            "timing-not-implicated"
        };
        if !covers {
            findings.push(format!(
                "the clock log does not reach within {} s of both ends of the window",
                req.coverage_gap_s
            ));
        }
        devices.push(DeviceFinding {
            name: d.name.clone(),
            kind: d.kind.clone(),
            budget_s: budget,
            clock_covers_window: covers,
            max_abs_te_in_window_s: max_te,
            phase_error_deg: max_te.filter(|_| is_pmu).map(|m| phase_error_deg(m, f0)),
            drift_alarm_unix: drift.as_ref().and_then(|d| d.first_alarm_unix),
            drift_kind: drift
                .as_ref()
                .filter(|d| d.first_alarm_unix.is_some())
                .map(|d| d.kind.clone()),
            sync_error_reports_in_window: sync_in,
            scada_findings: scada_f,
            findings,
            status: status.into(),
        });
        loaded.push((log_name, l));
    }
    // Sequence of events, re-checked with each stamp corrected by its device's error.
    let mut order = Vec::new();
    if let Some(s) = &req.soe {
        let text = read_text(base, &s.path)?;
        let mut recs: Vec<(String, f64, String)> = Vec::new();
        for l in text.lines() {
            let c: Vec<&str> = l.splitn(3, [',', ';']).map(str::trim).collect();
            if c.len() < 3 {
                continue;
            }
            if let Ok(t) = c[1].parse::<f64>() {
                recs.push((c[0].to_string(), t, c[2].to_string()));
            }
        }
        recs.sort_by(|a, b| a.1.total_cmp(&b.1));
        let te_of = |dev: &str, t: f64| -> Option<f64> {
            loaded
                .iter()
                .find(|(n, _)| n == dev)
                .and_then(|(_, l)| nearest(&l.readings, t, req.coverage_gap_s))
                .map(|r| r.offset)
        };
        for rule in &s.order {
            let a = recs.iter().find(|r| r.2 == rule.before);
            let b = recs.iter().find(|r| r.2 == rule.after);
            let (Some(a), Some(b)) = (a, b) else {
                order.push(OrderCheck {
                    before: rule.before.clone(),
                    after: rule.after.clone(),
                    recorded_gap_s: None,
                    corrected_gap_s: None,
                    status: "missing".into(),
                    devices: vec![],
                    corrections_s: vec![],
                });
                continue;
            };
            let rec = b.1 - a.1;
            let (ea, eb) = (te_of(&a.0, a.1), te_of(&b.0, b.1));
            let corr = match (ea, eb) {
                (Some(ea), Some(eb)) => Some((b.1 - eb) - (a.1 - ea)),
                _ => None,
            };
            let ok_rec = rec >= rule.min_gap_s;
            let status = match (ok_rec, corr.map(|c| c >= rule.min_gap_s)) {
                (true, Some(false)) => "consistent-only-before-correction",
                (true, _) => "consistent",
                (false, Some(true)) => "inverted-explained-by-timing",
                (false, Some(false)) => "inverted-not-explained",
                (false, None) => "inverted-clock-unknown",
            };
            order.push(OrderCheck {
                before: rule.before.clone(),
                after: rule.after.clone(),
                recorded_gap_s: Some(rec),
                corrected_gap_s: corr,
                status: status.into(),
                devices: vec![a.0.clone(), b.0.clone()],
                corrections_s: vec![ea, eb],
            });
        }
    }
    // An inversion the clock errors explain implicates the devices involved.
    for o in &order {
        if o.status == "inverted-explained-by-timing" {
            // A device is implicated when its own correction carries at least half the change.
            let change = (o.corrected_gap_s.unwrap_or(0.0) - o.recorded_gap_s.unwrap_or(0.0)).abs();
            for (dname, e) in o.devices.iter().zip(&o.corrections_s) {
                if e.is_none_or(|e| e.abs() < 0.5 * change) {
                    continue;
                }
                if let Some((i, _)) = loaded.iter().enumerate().find(|(_, (n, _))| n == dname) {
                    let d = &mut devices[i];
                    d.findings.push(format!(
                        "recorded order '{}' before '{}' inverted ({:+.6} s) and restored by the measured clock errors ({:+.6} s)",
                        o.before,
                        o.after,
                        o.recorded_gap_s.unwrap_or(f64::NAN),
                        o.corrected_gap_s.unwrap_or(f64::NAN)
                    ));
                    d.status = "timing-implicated".into();
                }
            }
        }
    }
    let mut counts = BTreeMap::new();
    for d in &devices {
        *counts.entry(d.status.clone()).or_insert(0) += 1;
    }
    let names = |s: &str| -> Vec<&str> {
        devices
            .iter()
            .filter(|d| d.status == s)
            .map(|d| d.name.as_str())
            .collect()
    };
    let (imp, sus, ins) = (
        names("timing-implicated"),
        names("timing-suspect"),
        names("insufficient"),
    );
    let mut verdict = if !imp.is_empty() {
        format!(
            "timing is implicated by the evidence at {} of {} device(s): {}",
            imp.len(),
            devices.len(),
            imp.join(", ")
        )
    } else if ins.len() == devices.len() {
        "insufficient evidence: no device's clock log covers the event window".to_string()
    } else if sus.is_empty() && ins.is_empty() {
        format!(
            "timing is not implicated by the evidence supplied: all {} device(s) within budget over the window with no flag",
            devices.len()
        )
    } else {
        "timing is not implicated at any device with evidence covering the window".to_string()
    };
    if !sus.is_empty() {
        verdict.push_str(&format!(
            "; to investigate (drift or time-stamp findings within budget): {}",
            sus.join(", ")
        ));
    }
    if !ins.is_empty() && ins.len() < devices.len() {
        verdict.push_str(&format!("; no covering clock evidence: {}", ins.join(", ")));
    }
    Ok(EventReport {
        format: EVENT_FORMAT,
        label: LABEL,
        name: req.name.clone(),
        utc: utc.into(),
        start_unix: ws,
        end_unix: we,
        pre_s: req.pre_s,
        nominal_hz: f0,
        devices,
        order,
        counts,
        verdict,
    })
}

pub fn event_html(r: &EventReport) -> String {
    let dev = r
        .devices
        .iter()
        .map(|d| {
            vec![
                d.name.clone(),
                d.kind.clone().unwrap_or_default(),
                d.status.clone(),
                d.max_abs_te_in_window_s
                    .map(h)
                    .unwrap_or_else(|| "-".into()),
                h(d.budget_s),
                d.findings.join("; "),
            ]
        })
        .collect();
    let ord = r
        .order
        .iter()
        .map(|o| {
            vec![
                format!("{} → {}", o.before, o.after),
                o.recorded_gap_s
                    .map(|g| format!("{g:+.6} s"))
                    .unwrap_or_else(|| "-".into()),
                o.corrected_gap_s
                    .map(|g| format!("{g:+.6} s"))
                    .unwrap_or_else(|| "-".into()),
                o.status.clone(),
            ]
        })
        .collect();
    page(
        &format!("Grid event timing forensics: {}", r.name),
        r.label,
        &r.verdict,
        &[format!(
            "Window unix {:.3} to {:.3}; drift searched from {:.0} s before; nominal {} Hz; {}",
            r.start_unix, r.end_unix, r.pre_s, r.nominal_hz, r.utc
        )],
        &[
            (
                "Devices".into(),
                vec![
                    "Device",
                    "Kind",
                    "Status",
                    "max |TE| in window",
                    "Budget",
                    "Findings",
                ],
                dev,
            ),
            (
                "Sequence of events".into(),
                vec!["Order", "Recorded gap", "Corrected gap", "Status"],
                ord,
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tve_relations() {
        let t60 = te_for_tve(0.01, 60.0);
        assert!((t60 - 26.526e-6).abs() < 0.01e-6, "{t60}");
        let t50 = te_for_tve(0.01, 50.0);
        assert!((t50 - 31.831e-6).abs() < 0.01e-6, "{t50}");
        assert!((phase_error_deg(1e-6, 60.0) - 0.0216).abs() < 1e-12);
        assert!((tve_of_phase(phase_error_deg(t60, 60.0)) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn quality_codes() {
        assert_eq!(pmu_tq_bound(0), None);
        assert_eq!(pmu_tq_bound(2), Some(1e-6));
        assert_eq!(pmu_tq_bound(7), Some(f64::INFINITY));
        assert!((msg_tq_bound(4).unwrap() - 1e-6).abs() < 1e-20);
        assert_eq!(msg_tq_bound(0), None);
        assert_eq!(msg_tq_bound(15), Some(f64::INFINITY));
    }

    #[test]
    fn sig9_rounds() {
        assert_eq!(sig9(1.234_567_890_123), 1.234_567_89);
        assert_eq!(sig9(0.0), 0.0);
        let mut v = serde_json::json!({"a": [0.1 + 0.2, 3]});
        round_json(&mut v);
        assert_eq!(v["a"][0].as_f64().unwrap(), 0.3);
        assert_eq!(v["a"][1].as_u64().unwrap(), 3);
    }

    fn noise(i: usize) -> f64 {
        // Deterministic pseudo-noise in [-1, 1).
        let x = (i as u64)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }

    #[test]
    fn drift_clean_series_no_alarm() {
        let s: Vec<(f64, f64)> = (0..7200).map(|i| (i as f64, 30e-9 * noise(i))).collect();
        let r = detect_drift(&s, &DriftParams::default(), Some(26.5e-6));
        assert_eq!(r.kind, "none", "{}", r.verdict);
    }

    #[test]
    fn drift_ramp_detected_before_budget() {
        // 10 ns/s drag from t = 3600 s.
        let s: Vec<(f64, f64)> = (0..10_800)
            .map(|i| {
                let t = i as f64;
                (
                    t,
                    30e-9 * noise(i)
                        + if t > 3600.0 {
                            10e-9 * (t - 3600.0)
                        } else {
                            0.0
                        },
                )
            })
            .collect();
        let r = detect_drift(&s, &DriftParams::default(), Some(26.5e-6));
        assert_eq!(r.kind, "gradual-drift", "{}", r.verdict);
        let a = r.first_alarm_unix.unwrap();
        assert!(a > 3600.0 && a < 3900.0, "{a}");
        assert!(r.warning_s.unwrap() > 2000.0);
        let on = r.estimated_onset_unix.unwrap();
        assert!((on - 3600.0).abs() < 120.0, "{on}");
    }

    #[test]
    fn drift_step_classified() {
        let s: Vec<(f64, f64)> = (0..3600)
            .map(|i| {
                (
                    i as f64,
                    30e-9 * noise(i) + if i >= 1800 { 2e-6 } else { 0.0 },
                )
            })
            .collect();
        let r = detect_drift(&s, &DriftParams::default(), None);
        assert_eq!(r.kind, "step", "{}", r.verdict);
        assert_eq!(r.steps.len(), 1);
        assert_eq!(r.steps[0].t_unix, 1800.0);
    }

    #[test]
    fn scada_findings() {
        let mut t = String::from("device,source_unix,receive_unix,seq\n");
        for i in 0..200u64 {
            let r = 1000.0 + i as f64 * 60.0;
            // WTG-1 healthy (0.2 s latency); WTG-2 clock 3 s ahead after record 100.
            t.push_str(&format!("WTG-1,{},{},{}\n", r - 0.2, r, i));
            let off = if i >= 100 { 3.0 } else { 0.0 };
            t.push_str(&format!("WTG-2,{},{},{}\n", r - 0.2 + off, r, i));
        }
        let spec = ScadaSpec {
            path: String::new(),
            ahead_tolerance_s: 1e-3,
            max_latency_s: 10.0,
            step_s: 0.5,
            window_s: 600.0,
            required_resolution_s: None,
        };
        let r = check_scada_text(&spec, &t).unwrap();
        assert_eq!(r.devices[0].findings, vec!["ok"]);
        assert!(r.devices[1].findings.contains(&"clock-ahead".to_string()));
        assert!(r.devices[1].findings.contains(&"clock-step".to_string()));
    }
}
