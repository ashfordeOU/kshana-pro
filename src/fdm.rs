// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # FDM GNSS jamming and spoofing event detector (W1119 C5; research 02 gap 9)
//!
//! EASA Safety Information Bulletin **SIB 2022-02R4** (3 Jul 2026, research 02 [A3]) lists,
//! among its recommendations to operators, the use of the flight data monitoring (FDM)
//! programme to identify and assess GNSS jamming and spoofing events, and the reporting of
//! suspected spoofing to manufacturers (ORO.GEN.160(b) as cited there). No standard method
//! exists. This module is one: it reads a flight-data CSV export and finds
//!
//! * **jamming**: GNSS integrity loss: NIC or NACp falling to the low bars, HFOM above a bar,
//!   the GNSS status flag reporting loss, or the GNSS position missing, with no spoofing
//!   indicator;
//! * **spoofing**: a GNSS position jump (implied speed far above the aircraft's), a GNSS time
//!   jump, GNSS altitude departing from barometric altitude (against the flight's own offset),
//!   GNSS ground speed departing from inertial ground speed, and the **circle pattern** (the
//!   GNSS-derived course turning through 360° while the inertial or FMS track does not).
//!
//! Output: events in the unified schema ([`crate::ievents`]) and a summary oriented to the
//! SIB ([`SibSummary`]): it names the SIB and maps each event to its themes as summarised in
//! Kshana's research; it does not reproduce or paraphrase SIB text beyond that, and every
//! mapping is "illustrative until confirmed against the SIB".
//!
//! Input columns (header names, case-insensitive; aliases in [`COLUMNS`]): `time` (ISO-8601 UTC
//! or seconds), `lat`, `lon`, `gnss_alt_ft`, `baro_alt_ft`, `gs_kt` (GNSS ground speed),
//! `track_deg` (inertial/FMS track), and where present `ins_gs_kt`, `nic`, `nacp`, `hfom_m`,
//! `gnss_status` (1/OK = normal, 0/FAIL/LOST = loss) and `gnss_utc` (time the GNSS states).
//!
//! Evidence: **MODELLED**. Rules exercised on synthetic flights with known truth (tests in
//! `tests/fdm_detector.rs`); not validated on labelled operator FDM data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc, round, sha256_hex};
use crate::ievents;
use kshana::interference_map::grid::haversine_m;

pub const FORMAT: &str = "kshana-pro-fdm-gnss-events/1";
pub const METHOD_ID: &str = "kshana-pro/fdm-gnss-detector/v1";
pub const EVIDENCE: &str = "MODELLED: rules checked on synthetic flights with known truth; not \
validated against labelled operator FDM data.";
pub const SIB: &str = "EASA Safety Information Bulletin 2022-02R4, Global Navigation Satellite \
System Outage and Alterations Leading to Communication / Navigation / Surveillance Degradation \
(revision of 3 Jul 2026)";
pub const SIB_NOTE: &str = "Themes as summarised in Kshana research 02 §2.2; illustrative until \
confirmed against the SIB text. The detector is a tool for an operator's FDM programme, not a \
means of compliance and not a substitute for the crew's or the manufacturer's procedures.";

/// Column names and aliases.
pub const COLUMNS: &[(&str, &[&str])] = &[
    ("time", &["time", "timestamp", "utc", "t"]),
    ("lat", &["lat", "latitude", "gnss_lat"]),
    ("lon", &["lon", "longitude", "gnss_lon"]),
    ("gnss_alt_ft", &["gnss_alt_ft", "gps_alt_ft", "geo_alt_ft"]),
    (
        "baro_alt_ft",
        &["baro_alt_ft", "pressure_alt_ft", "alt_baro_ft"],
    ),
    ("gs_kt", &["gs_kt", "gnss_gs_kt", "ground_speed_kt"]),
    (
        "track_deg",
        &["track_deg", "ins_track_deg", "true_track_deg"],
    ),
    ("ins_gs_kt", &["ins_gs_kt", "ir_gs_kt", "irs_gs_kt"]),
    ("nic", &["nic"]),
    ("nacp", &["nacp"]),
    ("hfom_m", &["hfom_m", "hfom"]),
    ("gnss_status", &["gnss_status", "gps_status"]),
    ("gnss_utc", &["gnss_utc", "gps_utc", "gnss_time"]),
];

/// Pre-registered thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// NIC at or below this marks integrity loss.
    pub nic_low: u8,
    /// NACp at or below this marks integrity loss.
    pub nacp_low: u8,
    /// HFOM above this (m) marks integrity loss.
    pub hfom_max_m: f64,
    /// Implied speed above max(GNSS GS, inertial GS) × factor + margin is a position jump.
    pub jump_factor: f64,
    pub jump_margin_kt: f64,
    /// |Δ(GNSS UTC − record time)| above this (s) is a time jump.
    pub time_jump_s: f64,
    /// |GNSS alt − baro alt − flight offset| above this (ft).
    pub alt_disagree_ft: f64,
    /// |GNSS GS − inertial GS| above this (kt).
    pub gs_disagree_kt: f64,
    /// Circle pattern: GNSS course turns ≥ 360° within this window (s) ...
    pub circle_window_s: f64,
    /// ... while the inertial/FMS track turns less than this (deg).
    pub circle_track_max_deg: f64,
    /// Seconds at the start used for the GNSS-baro offset (clean rows only).
    pub calib_s: f64,
    /// Flagged rows closer than this (s) form one event.
    pub merge_gap_s: f64,
    /// Shortest event kept (s).
    pub min_event_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            nic_low: 2,
            nacp_low: 3,
            hfom_max_m: 1852.0,
            jump_factor: 1.5,
            jump_margin_kt: 150.0,
            time_jump_s: 2.0,
            alt_disagree_ft: 800.0,
            gs_disagree_kt: 50.0,
            circle_window_s: 300.0,
            circle_track_max_deg: 90.0,
            calib_s: 300.0,
            merge_gap_s: 30.0,
            min_event_s: 5.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Row {
    t: f64,
    pos: Option<(f64, f64)>,
    gnss_alt: Option<f64>,
    baro_alt: Option<f64>,
    gs: Option<f64>,
    track: Option<f64>,
    ins_gs: Option<f64>,
    nic: Option<f64>,
    nacp: Option<f64>,
    hfom: Option<f64>,
    status_ok: Option<bool>,
    gnss_utc: Option<f64>,
}

/// One detected indicator count and worst value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Indicator {
    pub rows: usize,
    pub worst: f64,
}

/// One event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FdmEvent {
    pub n: usize,
    pub kind: ievents::Kind,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    /// Bound on onset: previous clean row to first flagged row (s).
    pub onset_uncertainty_s: f64,
    pub phase: String,
    pub baro_alt_ft_at_onset: Option<f64>,
    pub last_clean_position: Option<[f64; 2]>,
    /// Indicator name → rows and worst value (units in the name).
    pub indicators: BTreeMap<String, Indicator>,
    pub confidence: f64,
}

/// The SIB-oriented summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SibSummary {
    pub bulletin: String,
    pub note: String,
    pub jamming_events: usize,
    pub spoofing_events: usize,
    pub time_under_jamming_s: f64,
    pub time_under_spoofing_s: f64,
    pub events_below_10000_ft: usize,
    /// Theme → what this flight's data show for it.
    pub themes: Vec<(String, String)>,
    /// Suggested follow-up for the operator (not SIB text).
    pub follow_up: Vec<String>,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FdmReport {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub flight: String,
    pub input_sha256: String,
    pub rows: usize,
    pub rows_refused: usize,
    pub columns_present: Vec<String>,
    pub params: Params,
    pub gnss_baro_offset_ft: Option<f64>,
    pub events: Vec<FdmEvent>,
    pub unified_events: Vec<ievents::Event>,
    pub sib: SibSummary,
    pub limits: Vec<String>,
}

fn col(h: &[String], name: &str) -> Option<usize> {
    let al = COLUMNS.iter().find(|(n, _)| *n == name)?.1;
    h.iter()
        .position(|c| al.iter().any(|a| c.eq_ignore_ascii_case(a)))
}

fn parse_t(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().or_else(|| parse_utc(s))
}

fn parse_rows(text: &str) -> Result<(Vec<Row>, usize, Vec<String>), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let h: Vec<String> = lines
        .next()
        .ok_or("empty CSV")?
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let need = |n: &str| col(&h, n).ok_or(format!("CSV has no '{n}' column"));
    let (ct, cla, clo) = (need("time")?, need("lat")?, need("lon")?);
    let names = [
        "gnss_alt_ft",
        "baro_alt_ft",
        "gs_kt",
        "track_deg",
        "ins_gs_kt",
        "nic",
        "nacp",
        "hfom_m",
        "gnss_status",
        "gnss_utc",
    ];
    let idx: Vec<Option<usize>> = names.iter().map(|n| col(&h, n)).collect();
    let present: Vec<String> = ["time", "lat", "lon"]
        .iter()
        .map(|s| s.to_string())
        .chain(
            names
                .iter()
                .zip(&idx)
                .filter(|(_, i)| i.is_some())
                .map(|(n, _)| n.to_string()),
        )
        .collect();
    let (mut rows, mut refused) = (vec![], 0);
    for l in lines {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let g = |i: Option<usize>| i.and_then(|i| f.get(i).copied()).filter(|s| !s.is_empty());
        let num = |k: usize| {
            g(idx[k])
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        let Some(t) = g(Some(ct)).and_then(parse_t) else {
            refused += 1;
            continue;
        };
        let pos = match (
            g(Some(cla)).and_then(|s| s.parse::<f64>().ok()),
            g(Some(clo)).and_then(|s| s.parse::<f64>().ok()),
        ) {
            (Some(a), Some(b)) if (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b) => {
                Some((a, b))
            }
            _ => None,
        };
        let status_ok = g(idx[8]).map(|s| {
            let s = s.to_ascii_uppercase();
            !(s == "0" || s.starts_with("FAIL") || s.starts_with("LOST") || s == "INVALID")
        });
        rows.push(Row {
            t,
            pos,
            gnss_alt: num(0),
            baro_alt: num(1),
            gs: num(2),
            track: num(3),
            ins_gs: num(4),
            nic: num(5),
            nacp: num(6),
            hfom: num(7),
            status_ok,
            gnss_utc: g(idx[9]).and_then(parse_t),
        });
    }
    rows.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((rows, refused, present))
}

#[derive(Clone, Debug, Default)]
struct RowFlags {
    integrity: Option<f64>, // severity: lowest NIC or 0
    jump: Option<f64>,      // implied kt
    time_jump: Option<f64>, // s
    alt: Option<f64>,       // ft
    gs: Option<f64>,        // kt
    circle: Option<f64>,    // deg of GNSS course rotation
}

impl RowFlags {
    fn spoof(&self) -> bool {
        self.jump.is_some()
            || self.time_jump.is_some()
            || self.alt.is_some()
            || self.gs.is_some()
            || self.circle.is_some()
    }
    fn any(&self) -> bool {
        self.spoof() || self.integrity.is_some()
    }
}

fn bearing(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (p1, p2) = (a.0.to_radians(), b.0.to_radians());
    let dl = (b.1 - a.1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

fn wrap180(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn integrity_flag(r: &Row, p: &Params) -> Option<f64> {
    let nic = r.nic.filter(|v| *v <= p.nic_low as f64);
    let nacp = r.nacp.filter(|v| *v <= p.nacp_low as f64);
    let hfom = r.hfom.filter(|v| *v > p.hfom_max_m);
    let lost = r.status_ok == Some(false) || r.pos.is_none();
    if nic.is_some() || nacp.is_some() || hfom.is_some() || lost {
        Some(nic.or(nacp).unwrap_or(0.0))
    } else {
        None
    }
}

/// Analyse one flight's CSV.
pub fn analyse(flight: &str, csv: &str, p: &Params) -> Result<FdmReport, String> {
    let (rows, refused, present) = parse_rows(csv)?;
    if rows.len() < 10 {
        return Err(format!("{} rows; at least 10 are needed", rows.len()));
    }
    let t0 = rows[0].t;
    let mut fl = vec![RowFlags::default(); rows.len()];
    for (i, r) in rows.iter().enumerate() {
        fl[i].integrity = integrity_flag(r, p);
    }
    // GNSS-baro offset from the clean calibration rows.
    let offset = median(
        rows.iter()
            .zip(&fl)
            .filter(|(r, f)| r.t - t0 <= p.calib_s && f.integrity.is_none())
            .filter_map(|(r, _)| Some(r.gnss_alt? - r.baro_alt?))
            .collect(),
    );
    let mut last: Option<(f64, (f64, f64), Option<f64>)> = None; // t, pos, gnss_utc - t
    for (i, r) in rows.iter().enumerate() {
        if let (Some(o), Some(g), Some(b)) = (offset, r.gnss_alt, r.baro_alt) {
            let d = (g - b - o).abs();
            if d > p.alt_disagree_ft && r.pos.is_some() {
                fl[i].alt = Some(d);
            }
        }
        if let (Some(g), Some(n)) = (r.gs, r.ins_gs) {
            let d = (g - n).abs();
            if d > p.gs_disagree_kt && r.pos.is_some() {
                fl[i].gs = Some(d);
            }
        }
        if let Some(pos) = r.pos {
            let tdiff = r.gnss_utc.map(|g| g - r.t);
            if let Some((tl, pl, td)) = last {
                let dt = (r.t - tl).max(1e-3);
                let v_kt = haversine_m(pl.0, pl.1, pos.0, pos.1) / dt / (1852.0 / 3600.0);
                let ref_kt = r.gs.unwrap_or(0.0).max(r.ins_gs.unwrap_or(0.0)).max(100.0);
                if v_kt > ref_kt * p.jump_factor + p.jump_margin_kt {
                    fl[i].jump = Some(v_kt);
                }
                if let (Some(a), Some(b)) = (td, tdiff) {
                    if (b - a).abs() > p.time_jump_s {
                        fl[i].time_jump = Some(b - a);
                    }
                }
            }
            last = Some((r.t, pos, tdiff));
        }
    }
    // Circle pattern: the GNSS-derived course turns ≥ 360° in the window while the
    // inertial/FMS track turns less than the bar.
    type Fix = (usize, f64, (f64, f64), Option<f64>);
    let fixes: Vec<Fix> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| Some((i, r.t, r.pos?, r.track)))
        .collect();
    let mut course: Vec<(usize, f64, f64, Option<f64>)> = vec![]; // row, t, gnss course, track
    for w in fixes.windows(2) {
        if haversine_m(w[0].2 .0, w[0].2 .1, w[1].2 .0, w[1].2 .1) > 5.0 {
            course.push((w[1].0, w[1].1, bearing(w[0].2, w[1].2), w[1].3));
        }
    }
    let mut j0 = 0usize;
    let (mut cum_c, mut cum_t) = (vec![0.0f64], vec![0.0f64]);
    for k in 1..course.len() {
        cum_c.push(cum_c[k - 1] + wrap180(course[k].2 - course[k - 1].2));
        let dt = match (course[k].3, course[k - 1].3) {
            (Some(a), Some(b)) => wrap180(a - b),
            _ => f64::NAN,
        };
        cum_t.push(cum_t[k - 1] + dt);
    }
    for k in 0..course.len() {
        while course[k].1 - course[j0].1 > p.circle_window_s {
            j0 += 1;
        }
        let rot = (cum_c[k] - cum_c[j0]).abs();
        let trk = (cum_t[k] - cum_t[j0]).abs();
        if rot >= 360.0 && trk.is_finite() && trk < p.circle_track_max_deg {
            for c in &course[j0..=k] {
                fl[c.0].circle = Some(rot);
            }
        }
    }

    // Episodes.
    let mut eps: Vec<(usize, usize)> = vec![];
    for (i, f) in fl.iter().enumerate() {
        if !f.any() {
            continue;
        }
        match eps.last_mut() {
            Some(e) if rows[i].t - rows[e.1].t <= p.merge_gap_s => e.1 = i,
            _ => eps.push((i, i)),
        }
    }
    eps.retain(|&(a, b)| {
        rows[b].t - rows[a].t >= p.min_event_s
            || (a..=b).any(|i| fl[i].jump.is_some() || fl[i].time_jump.is_some())
    });
    let sha = sha256_hex(csv.as_bytes());
    let mut events = vec![];
    let mut unified = vec![];
    for (k, &(a, b)) in eps.iter().enumerate() {
        let n = k + 1;
        let mut ind: BTreeMap<String, Indicator> = BTreeMap::new();
        let mut add = |name: &str, v: Option<f64>| {
            if let Some(v) = v {
                let e = ind.entry(name.to_string()).or_default();
                e.rows += 1;
                e.worst = round(e.worst.max(v.abs()), 2);
            }
        };
        let mut spoof_rows = 0;
        for f in &fl[a..=b] {
            add("integrity_loss_rows", f.integrity.map(|_| 1.0));
            add("position_jump_implied_kt", f.jump);
            add("gnss_time_jump_s", f.time_jump);
            add("gnss_baro_disagreement_ft", f.alt);
            add("gnss_ins_gs_disagreement_kt", f.gs);
            add("circle_gnss_course_rotation_deg", f.circle);
            if f.spoof() {
                spoof_rows += 1;
            }
        }
        let kind = if spoof_rows > 0 {
            ievents::Kind::Spoofing
        } else {
            ievents::Kind::Jamming
        };
        let prev = a.checked_sub(1).map(|i| rows[i].t).unwrap_or(rows[a].t);
        let clean = rows[..a]
            .iter()
            .rev()
            .zip(fl[..a].iter().rev())
            .find(|(r, f)| r.pos.is_some() && !f.any())
            .and_then(|(r, _)| r.pos);
        let ra = &rows[a];
        let vs = if a >= 10 {
            match (ra.baro_alt, rows[a - 10].baro_alt) {
                (Some(x), Some(y)) => (x - y) / (ra.t - rows[a - 10].t).max(1.0) * 60.0,
                _ => 0.0,
            }
        } else {
            0.0
        };
        let phase = match (ra.baro_alt, ra.ins_gs.or(ra.gs)) {
            (_, Some(g)) if g < 50.0 => "ground",
            (Some(h), _) if h < 10_000.0 && vs < -300.0 => "descent/approach",
            (Some(h), _) if h < 10_000.0 && vs > 300.0 => "climb",
            (Some(h), _) if vs.abs() <= 300.0 && h >= 10_000.0 => "cruise",
            (Some(_), _) if vs > 300.0 => "climb",
            (Some(_), _) if vs < -300.0 => "descent",
            _ => "level (below 10,000 ft)",
        };
        let n_ind = ind.keys().filter(|k| *k != "integrity_loss_rows").count();
        let conf = round(
            1.0 - (1.0
                - if ind.contains_key("integrity_loss_rows") {
                    0.5
                } else {
                    0.0
                })
                * 0.5f64.powi(n_ind as i32),
            3,
        );
        let start = iso(ra.t.floor());
        let end = iso(rows[b].t.ceil());
        let mut metrics = BTreeMap::new();
        for (k, v) in &ind {
            if k != "integrity_loss_rows" {
                metrics.insert(k.clone(), v.worst);
            }
        }
        metrics.insert("duration_s".into(), round(rows[b].t - ra.t, 1));
        if let Some((la, lo)) = clean {
            unified.push(ievents::Event {
                format: ievents::FORMAT.into(),
                id: format!("fdm-{flight}-{n}"),
                start_utc: start.clone(),
                end_utc: end.clone(),
                systems: vec![ievents::System::Gnss { band: "L1".into() }],
                kind,
                location: Some(ievents::Place {
                    lat_deg: la,
                    lon_deg: lo,
                    radius_m: 1852.0,
                }),
                area: None,
                observers: vec![format!("aircraft:{flight}")],
                metrics,
                source_estimate: None,
                confidence: conf,
                basis: METHOD_ID.into(),
                evidence: vec![ievents::EvidenceRef {
                    file: "inputs/flight.csv".into(),
                    sha256: sha.clone(),
                }],
                reporter: "kshana-pro forensics fdm".into(),
                corroborated: false,
            });
        }
        events.push(FdmEvent {
            n,
            kind,
            start_utc: start,
            end_utc: end,
            duration_s: round(rows[b].t - ra.t, 1),
            onset_uncertainty_s: round(ra.t - prev, 2),
            phase: phase.into(),
            baro_alt_ft_at_onset: ra.baro_alt,
            last_clean_position: clean.map(|(a, b)| [round(a, 5), round(b, 5)]),
            indicators: ind,
            confidence: conf,
        });
    }
    let dur = |k: ievents::Kind| {
        events
            .iter()
            .filter(|e| e.kind == k)
            .map(|e| e.duration_s)
            .fold(0.0, |a, d| a + d)
    };
    let nj = events
        .iter()
        .filter(|e| e.kind == ievents::Kind::Jamming)
        .count();
    let ns = events
        .iter()
        .filter(|e| e.kind == ievents::Kind::Spoofing)
        .count();
    let low = events
        .iter()
        .filter(|e| e.baro_alt_ft_at_onset.is_some_and(|h| h < 10_000.0))
        .count();
    let themes = vec![
        (
            "Operators: use the FDM programme to identify and assess GNSS jamming and spoofing events".to_string(),
            format!("{nj} jamming and {ns} spoofing event(s) identified in this flight by method {METHOD_ID}"),
        ),
        (
            "Spoofing is the higher risk; no crew alert distinguishes jamming from spoofing".to_string(),
            "Each event is classed from the data (spoofing when any position, time, altitude, speed or circle indicator is present), not from a crew alert".to_string(),
        ),
        (
            "Monitor GNSS time against non-GNSS time sources".to_string(),
            if present.iter().any(|c| c == "gnss_utc") {
                format!("GNSS time compared with the recorder time; {} time jump row(s)", events.iter().filter_map(|e| e.indicators.get("gnss_time_jump_s")).map(|i| i.rows).sum::<usize>())
            } else {
                "Not assessed: the export carries no GNSS time column".to_string()
            },
        ),
        (
            "Report suspected spoofing to manufacturers (ORO.GEN.160(b) as cited in the SIB)".to_string(),
            if ns > 0 { "Spoofing event(s) found: the unified events file is a ready attachment".to_string() } else { "No spoofing event found".to_string() },
        ),
    ];
    let mut follow_up = vec![];
    if ns > 0 {
        follow_up.push("Review TAWS alerts and FMS position after each spoofing event (post-spoofing recovery is a known concern).".into());
    }
    if low > 0 {
        follow_up.push("Events below 10,000 ft: check the approach type flown and any GNSS-dependent procedure.".into());
    }
    let mut limits = vec![
        "GNSS altitude is compared with barometric altitude against the flight's own offset in the first minutes; large temperature deviations or QNH changes can move that offset.".to_string(),
        "The circle indicator needs a track column independent of GNSS (IRS or FMS); where the recorded track is GNSS-derived it cannot separate a real orbit from a spoofed one.".into(),
        "Integrity loss alone is classed as jamming; an equipment fault or masking during manoeuvres looks the same in these parameters.".into(),
        "Thresholds are pre-registered engineering values (params in the report); MODELLED on synthetic flights, not validated on labelled operator data.".into(),
    ];
    for c in ["ins_gs_kt", "nic", "gnss_utc", "track_deg"] {
        if !present.iter().any(|x| x == c) {
            limits.push(format!(
                "Column {c} absent: the indicators that need it were not assessed."
            ));
        }
    }
    Ok(FdmReport {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        evidence: EVIDENCE.into(),
        flight: flight.into(),
        input_sha256: sha,
        rows: rows.len(),
        rows_refused: refused,
        columns_present: present,
        params: p.clone(),
        gnss_baro_offset_ft: offset.map(|o| round(o, 1)),
        sib: SibSummary {
            bulletin: SIB.into(),
            note: SIB_NOTE.into(),
            jamming_events: nj,
            spoofing_events: ns,
            time_under_jamming_s: round(dur(ievents::Kind::Jamming), 1),
            time_under_spoofing_s: round(dur(ievents::Kind::Spoofing), 1),
            events_below_10000_ft: low,
            themes,
            follow_up,
        },
        events,
        unified_events: unified,
        limits,
    })
}

/// Write the report directory: `fdm-report.json`, `events.json`, `sib-summary.txt`,
/// `inputs/flight.csv`.
pub fn write_dir(dir: &std::path::Path, r: &FdmReport, csv: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("inputs")).map_err(io)?;
    let j = |v: &dyn Fn() -> serde_json::Result<String>| v().unwrap_or_default() + "\n";
    std::fs::write(
        dir.join("fdm-report.json"),
        j(&|| serde_json::to_string_pretty(r)),
    )
    .map_err(io)?;
    std::fs::write(
        dir.join("events.json"),
        j(&|| serde_json::to_string_pretty(&r.unified_events)),
    )
    .map_err(io)?;
    std::fs::write(dir.join("inputs/flight.csv"), csv).map_err(io)?;
    let mut s = format!(
        "GNSS interference events, flight {}\nOriented to: {}\n{}\n\nJamming events: {} ({} s)\nSpoofing events: {} ({} s)\nEvents with onset below 10,000 ft: {}\n\n",
        r.flight, r.sib.bulletin, r.sib.note, r.sib.jamming_events, r.sib.time_under_jamming_s,
        r.sib.spoofing_events, r.sib.time_under_spoofing_s, r.sib.events_below_10000_ft
    );
    for e in &r.events {
        s += &format!(
            "#{} {:?} {} to {} ({} s, {}), indicators: {}\n",
            e.n,
            e.kind,
            e.start_utc,
            e.end_utc,
            e.duration_s,
            e.phase,
            e.indicators.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }
    s += "\nThemes:\n";
    for (t, v) in &r.sib.themes {
        s += &format!("- {t}: {v}\n");
    }
    s += &format!("\nEvidence: {}\n", r.evidence);
    std::fs::write(dir.join("sib-summary.txt"), s).map_err(io)?;
    Ok(())
}

/// Synthetic flights with known truth.
pub mod synth {
    /// Flight classes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Flight {
        Clean,
        Jamming,
        Spoofing,
        /// A real holding pattern (GNSS and inertial track turn together): no event.
        Holding,
        /// Spoofing to a fixed point that the GNSS track circles while inertial track is straight.
        CircleSpoof,
    }

    pub const T0: f64 = 1_780_300_800.0;
    /// Event window (s after T0).
    pub const EV: (f64, f64) = (1200.0, 1500.0);

    fn dest(lat: f64, lon: f64, n: f64, e: f64) -> (f64, f64) {
        let r = 6_371_000.0;
        (
            lat + (n / r).to_degrees(),
            lon + (e / (r * lat.to_radians().cos())).to_degrees(),
        )
    }

    /// A 40-minute descent from FL350 heading 090 at 450 kt, sampled at 1 Hz.
    pub fn make(f: Flight) -> String {
        let mut s = String::from("time,lat,lon,gnss_alt_ft,baro_alt_ft,gs_kt,track_deg,ins_gs_kt,nic,nacp,hfom_m,gnss_status,gnss_utc\n");
        let (mut n, mut e) = (0.0f64, 0.0f64);
        let (lat0, lon0) = (58.0, 22.0);
        let mut hdg: f64 = 90.0;
        for k in 0..2400 {
            let t = k as f64;
            let in_ev = t >= EV.0 && t < EV.1;
            let baro = (35_000.0 - t * 12.0).max(3_000.0);
            let gs = 450.0 - t * 0.08;
            if f == Flight::Holding && in_ev {
                hdg = (hdg + 3.0).rem_euclid(360.0); // standard-rate turn
            }
            let v = gs * 1852.0 / 3600.0;
            n += v * hdg.to_radians().cos();
            e += v * hdg.to_radians().sin();
            let truth = dest(lat0, lon0, n, e);
            let geo_off = 350.0; // GNSS (geometric) − pressure altitude on this day
            let (mut pos, mut galt, mut ggs, mut nic, mut nacp, mut hfom, mut st, mut gutc) =
                (Some(truth), baro + geo_off, gs, 8, 10, 15.0, "1", T0 + t);
            match f {
                Flight::Jamming if in_ev => {
                    pos = None;
                    nic = 0;
                    nacp = 0;
                    hfom = 9999.0;
                    st = "0";
                }
                Flight::Spoofing if in_ev => {
                    // Position jumps 60 km north and drifts; GNSS altitude +2,000 ft; GNSS GS
                    // differs from inertial; GNSS time pushed by 30 s; integrity looks normal.
                    pos = Some(dest(truth.0, truth.1, 60_000.0, -(t - EV.0) * 50.0));
                    galt += 2_000.0;
                    ggs = gs + 120.0;
                    gutc += 30.0;
                }
                Flight::CircleSpoof if in_ev => {
                    let a = (t - EV.0) / 120.0 * std::f64::consts::TAU; // one orbit per 2 min
                    pos = Some(dest(59.0, 24.0, 3000.0 * a.cos(), 3000.0 * a.sin()));
                    ggs = 300.0;
                }
                _ => {}
            }
            let (la, lo) = pos
                .map(|p| (format!("{:.6}", p.0), format!("{:.6}", p.1)))
                .unwrap_or_default();
            s += &format!(
                "{},{la},{lo},{:.0},{:.0},{:.1},{:.1},{:.1},{nic},{nacp},{hfom:.0},{st},{}\n",
                crate::forensics::inputs::iso(T0 + t),
                galt,
                baro,
                ggs,
                hdg,
                gs,
                crate::forensics::inputs::iso(gutc)
            );
        }
        s
    }
}
