// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Post-exposure residual-effect detector and clean-or-reset recommendation (B1102)
//!
//! Crews report effects that outlast the interference: TAWS/EGPWS alerts hours later or on
//! the next flight, a GNSS-derived clock that stays wrong, a receiver that does not recover
//! after the aircraft has left the area. This module reads the flight-data exports of one
//! aircraft's consecutive flights (the [`crate::fdm`] CSV format), finds the exposures with
//! the FDM detector, and looks for these **residual indicators** after them:
//!
//! * `clock_offset_after_exposure`: the median of GNSS UTC − recorder time on clean rows
//!   (outside every detected event) at least `settle_s` after the last event of a flight, or
//!   on any later flight, differs from the aircraft's reference (the earliest clean value
//!   before any exposure) by more than `clock_tol_s`;
//! * `degradation_after_area_exit`: with interference areas supplied (boxes, e.g. from
//!   `aviation areas` or NOTAMs), an event continues more than `exit_persist_s` after the
//!   aircraft left every area. The position during the event is carried coarsely from the
//!   last clean fix with the inertial ground speed and track (area membership only);
//! * `degraded_from_flight_start`: a later flight has an event within `start_window_s` of
//!   its first record, outside every area supplied (the receiver did not come back clean);
//! * `alert_without_interference`: a TAWS/EGPWS or clock alert (alerts CSV) after an
//!   exposure, at a time no detected event covers (± `alert_margin_s`).
//!
//! Recorded resets (CSV `reset_utc[,action]`) split the history: an indicator after a reset
//! that followed the exposure means the effect persisted through it.
//!
//! **Outcome** (a recommendation, MODELLED): `RESET RECOMMENDED` (an indicator after the last
//! exposure, no reset since), `LRU CHECK RECOMMENDED` (an indicator persisted through a
//! recorded reset), `NO RESIDUAL EFFECT FOUND`, `NO EXPOSURE FOUND` or `NOT ASSESSABLE`. The
//! manufacturer's and operator's procedures decide; the position-against-reference check is
//! `kshana-pro reports reset-check`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::*;
use crate::fdm;
use crate::forensics::inputs::iso;
use crate::ievents::Kind;

pub const FORMAT: &str = "kshana-pro-residual-effects/1";
pub const METHOD: &str = "kshana-pro/residual-effects/v1";
pub const RESULT_FILE: &str = "residual-effects.json";
pub const EVIDENCE: &str = "MODELLED: indicators exercised on synthetic consecutive flights with \
known residual effects; not validated against labelled operator FDM data or manufacturers' reset \
criteria. A recommendation only; the manufacturer's and operator's procedures decide.";

pub const RESET_RECOMMENDED: &str = "RESET RECOMMENDED";
pub const LRU_CHECK: &str = "LRU CHECK RECOMMENDED";
pub const NO_RESIDUAL: &str = "NO RESIDUAL EFFECT FOUND";
pub const NO_EXPOSURE: &str = "NO EXPOSURE FOUND";
pub const NOT_ASSESSABLE: &str = "NOT ASSESSABLE";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub settle_s: f64,
    pub clock_tol_s: f64,
    pub exit_persist_s: f64,
    pub start_window_s: f64,
    pub alert_margin_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            settle_s: 600.0,
            clock_tol_s: 1.0,
            exit_persist_s: 120.0,
            start_window_s: 120.0,
            alert_margin_s: 60.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightRef {
    pub id: String,
    pub csv: String,
}

/// A residual-effects request (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// The aircraft (registration or fleet number), as the operator names it.
    pub aircraft: String,
    pub flights: Vec<FlightRef>,
    /// CSV `time,flight,alert` (alert text such as `TERRAIN`, `PULL UP`, `CLOCK`).
    #[serde(default)]
    pub alerts: Option<String>,
    /// CSV `reset_utc[,action]`.
    #[serde(default)]
    pub resets: Option<String>,
    /// Interference areas `[south, west, north, east]`.
    #[serde(default)]
    pub areas: Vec<[f64; 4]>,
    #[serde(default)]
    pub params: Params,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Indicator {
    pub indicator: String,
    pub flight: String,
    pub time_utc: String,
    pub value: f64,
    pub reference: Option<f64>,
    pub after_reset: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlightSummary {
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub rows: usize,
    pub jamming_events: usize,
    pub spoofing_events: usize,
    pub exposed: bool,
    pub clock_offset_pre_s: Option<f64>,
    pub clock_offset_post_s: Option<f64>,
    pub fdm_input_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResidualResult {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub statement: String,
    pub title: String,
    pub aircraft: String,
    pub outcome: String,
    pub reasons: Vec<String>,
    pub clock_reference_s: Option<f64>,
    pub flights: Vec<FlightSummary>,
    pub indicators: Vec<Indicator>,
    pub resets_utc: Vec<String>,
    pub alerts_read: usize,
    pub alerts_before_first_exposure: usize,
    pub limits: Vec<String>,
    pub params: Params,
    pub inputs: Vec<InputRef>,
}

#[derive(Clone, Debug, Default)]
struct Row {
    t: f64,
    pos: Option<(f64, f64)>,
    gs: Option<f64>,
    track: Option<f64>,
    clock: Option<f64>,
}

fn col(h: &[String], name: &str) -> Option<usize> {
    let al = fdm::COLUMNS.iter().find(|(n, _)| *n == name)?.1;
    h.iter()
        .position(|c| al.iter().any(|a| c.eq_ignore_ascii_case(a)))
}

fn parse_t(s: &str) -> Option<f64> {
    let s = s.trim();
    s.parse::<f64>()
        .ok()
        .or_else(|| crate::forensics::inputs::parse_utc(s))
}

fn rows(csv: &str) -> Vec<Row> {
    let mut lines = csv
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let Some(h) = lines.next() else {
        return vec![];
    };
    let h: Vec<String> = h.split(',').map(|s| s.trim().to_string()).collect();
    let (ct, cla, clo) = (col(&h, "time"), col(&h, "lat"), col(&h, "lon"));
    let (cgs, cig, ctr, cgu) = (
        col(&h, "gs_kt"),
        col(&h, "ins_gs_kt"),
        col(&h, "track_deg"),
        col(&h, "gnss_utc"),
    );
    let mut out = vec![];
    for l in lines {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let g = |i: Option<usize>| i.and_then(|i| f.get(i).copied()).filter(|s| !s.is_empty());
        let num = |i: Option<usize>| g(i).and_then(|s| s.parse::<f64>().ok());
        let Some(t) = g(ct).and_then(parse_t) else {
            continue;
        };
        out.push(Row {
            t,
            pos: match (num(cla), num(clo)) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            },
            gs: num(cig).or(num(cgs)),
            track: num(ctr),
            clock: g(cgu).and_then(parse_t).map(|u| u - t),
        });
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

fn median(mut v: Vec<f64>) -> Option<f64> {
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

fn dest(lat: f64, lon: f64, dist_m: f64, brg_deg: f64) -> (f64, f64) {
    let r = kshana::interference_map::grid::EARTH_RADIUS_M;
    let b = brg_deg.to_radians();
    (
        lat + (dist_m * b.cos() / r).to_degrees(),
        lon + (dist_m * b.sin() / (r * lat.to_radians().cos().max(1e-6))).to_degrees(),
    )
}

/// The alert classes residual effects look at: TAWS/EGPWS and clock alerts.
fn residual_alert(text: &str) -> bool {
    matches!(alert_class(text), "taws" | "clock")
}

/// A detected event: start, end, kind, last clean position.
type EventSpan = (f64, f64, Kind, Option<(f64, f64)>);

struct Flight {
    id: String,
    rows: Vec<Row>,
    report: fdm::FdmReport,
    events: Vec<EventSpan>,
}

/// Analyse flights given as `(id, csv)`, alerts and resets CSV text.
pub fn analyse(
    req: &Request,
    flights_in: &[(String, String)],
    alerts_csv: Option<&str>,
    resets_csv: Option<&str>,
    inputs: &Inputs,
) -> Result<ResidualResult, String> {
    let p = &req.params;
    let areas: Vec<GeoBox> = req
        .areas
        .iter()
        .map(|a| GeoBox::from_array(*a))
        .collect::<Result<_, _>>()?;
    if flights_in.is_empty() {
        return Err("needs at least one flight".into());
    }
    let mut flights = vec![];
    for (id, csv) in flights_in {
        let report = fdm::analyse(id, csv, &fdm::Params::default())
            .map_err(|e| format!("flight {id}: {e}"))?;
        let events = report
            .events
            .iter()
            .map(|e| {
                (
                    utc(&e.start_utc).unwrap_or(0.0),
                    utc(&e.end_utc).unwrap_or(0.0),
                    e.kind,
                    e.last_clean_position.map(|p| (p[0], p[1])),
                )
            })
            .collect();
        flights.push(Flight {
            id: id.clone(),
            rows: rows(csv),
            report,
            events,
        });
    }
    flights.sort_by(|a, b| {
        let t = |f: &Flight| f.rows.first().map(|r| r.t).unwrap_or(0.0);
        t(a).total_cmp(&t(b))
    });
    let mut resets: Vec<f64> = vec![];
    if let Some(csv) = resets_csv {
        for (i, l) in csv.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') || l.to_ascii_lowercase().starts_with("reset") {
                continue;
            }
            let f = l.split(',').next().unwrap_or("");
            resets.push(parse_t(f).ok_or(format!("resets line {}: bad time '{f}'", i + 1))?);
        }
    }
    resets.sort_by(f64::total_cmp);
    let mut alerts: Vec<(f64, String, String)> = vec![];
    if let Some(csv) = alerts_csv {
        for (i, l) in csv.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') || l.to_ascii_lowercase().starts_with("time") {
                continue;
            }
            let f: Vec<&str> = l.splitn(3, ',').map(str::trim).collect();
            if f.len() < 3 {
                return Err(format!("alerts line {}: needs time,flight,alert", i + 1));
            }
            let t = parse_t(f[0]).ok_or(format!("alerts line {}: bad time", i + 1))?;
            alerts.push((t, f[1].to_string(), f[2].to_string()));
        }
    }
    let first_exposure = flights
        .iter()
        .flat_map(|f| f.events.iter().map(|e| e.0))
        .fold(f64::INFINITY, f64::min);
    let last_exposure = flights
        .iter()
        .flat_map(|f| f.events.iter().map(|e| e.1))
        .fold(f64::NEG_INFINITY, f64::max);
    let in_event =
        |f: &Flight, t: f64, m: f64| f.events.iter().any(|e| t >= e.0 - m && t <= e.1 + m);
    let reset_between = |a: f64, b: f64| resets.iter().any(|r| *r > a && *r <= b);

    // Clock offsets per flight.
    let mut summaries = vec![];
    let mut clock_ref: Option<f64> = None;
    let mut indicators: Vec<Indicator> = vec![];
    let mut exposure_seen: Option<f64> = None; // end time of the latest exposure so far
    for f in &flights {
        let clean = |r: &&Row| r.clock.is_some() && r.pos.is_some() && !in_event(f, r.t, 0.0);
        let first_ev = f.events.iter().map(|e| e.0).fold(f64::INFINITY, f64::min);
        let last_ev = f
            .events
            .iter()
            .map(|e| e.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let pre = median(
            f.rows
                .iter()
                .filter(clean)
                .filter(|r| r.t < first_ev)
                .filter_map(|r| r.clock)
                .collect(),
        );
        let post = if f.events.is_empty() {
            None
        } else {
            median(
                f.rows
                    .iter()
                    .filter(clean)
                    .filter(|r| r.t > last_ev + p.settle_s)
                    .filter_map(|r| r.clock)
                    .collect(),
            )
        };
        if clock_ref.is_none() && exposure_seen.is_none() {
            clock_ref = pre;
        }
        // A later flight's pre-event (or whole-flight) value after an exposure.
        if let (Some(ex), Some(c), Some(rf)) = (exposure_seen, pre, clock_ref) {
            if (c - rf).abs() > p.clock_tol_s {
                let t0 = f.rows.first().map(|r| r.t).unwrap_or(0.0);
                indicators.push(Indicator {
                    indicator: "clock_offset_after_exposure".into(),
                    flight: f.id.clone(),
                    time_utc: iso(t0),
                    value: rnd(c, 3),
                    reference: Some(rnd(rf, 3)),
                    after_reset: reset_between(ex, t0),
                    detail: format!(
                        "GNSS UTC − recorder time {:+.1} s on this flight against {:+.1} s before the exposure",
                        c, rf
                    ),
                });
            }
        }
        if !f.events.is_empty() {
            if let (Some(c), Some(rf)) = (post, clock_ref.or(pre)) {
                if (c - rf).abs() > p.clock_tol_s {
                    indicators.push(Indicator {
                        indicator: "clock_offset_after_exposure".into(),
                        flight: f.id.clone(),
                        time_utc: iso(last_ev + p.settle_s),
                        value: rnd(c, 3),
                        reference: Some(rnd(rf, 3)),
                        after_reset: reset_between(last_ev, last_ev + p.settle_s),
                        detail: format!(
                            "GNSS UTC − recorder time {:+.1} s from {} s after the last event, against {:+.1} s before",
                            c, p.settle_s, rf
                        ),
                    });
                }
            }
        }
        // Degradation from flight start (a later flight).
        if let Some(ex) = exposure_seen {
            let t0 = f.rows.first().map(|r| r.t).unwrap_or(0.0);
            for e in &f.events {
                if e.0 - t0 <= p.start_window_s {
                    let covered =
                        e.3.or(f.rows.iter().find_map(|r| r.pos))
                            .is_some_and(|(la, lo)| areas.iter().any(|a| a.contains(la, lo)));
                    if !covered {
                        indicators.push(Indicator {
                            indicator: "degraded_from_flight_start".into(),
                            flight: f.id.clone(),
                            time_utc: iso(e.0),
                            value: rnd(e.1 - e.0, 1),
                            reference: None,
                            after_reset: reset_between(ex, e.0),
                            detail: format!(
                                "{:?} event from {:.0} s after the first record, outside every area supplied",
                                e.2,
                                e.0 - t0
                            ),
                        });
                    }
                }
            }
        }
        // Degradation after leaving the areas.
        if !areas.is_empty() {
            for e in &f.events {
                let Some(start) = e.3 else { continue };
                let mut pos = start;
                let mut last_t = f
                    .rows
                    .iter()
                    .rev()
                    .find(|r| r.t < e.0 && r.pos.is_some())
                    .map(|r| r.t)
                    .unwrap_or(e.0);
                let mut was_in = areas.iter().any(|a| a.contains(pos.0, pos.1));
                let mut exit: Option<f64> = None;
                let from = last_t;
                for r in f.rows.iter().filter(|r| r.t > from && r.t <= e.1) {
                    if let (Some(g), Some(tr)) = (r.gs, r.track) {
                        pos = dest(pos.0, pos.1, g * 1852.0 / 3600.0 * (r.t - last_t), tr);
                    }
                    last_t = r.t;
                    let inside = areas.iter().any(|a| a.contains(pos.0, pos.1));
                    if inside {
                        was_in = true;
                        exit = None;
                    } else if was_in && exit.is_none() {
                        exit = Some(r.t);
                    }
                }
                if let Some(x) = exit {
                    if e.1 - x > p.exit_persist_s {
                        indicators.push(Indicator {
                            indicator: "degradation_after_area_exit".into(),
                            flight: f.id.clone(),
                            time_utc: iso(x),
                            value: rnd(e.1 - x, 1),
                            reference: Some(p.exit_persist_s),
                            after_reset: false,
                            detail: format!(
                                "{:?} event continued {:.0} s after the aircraft (carried by inertial speed and track) left every area supplied",
                                e.2,
                                e.1 - x
                            ),
                        });
                    }
                }
            }
        }
        // Alerts after an exposure with no concurrent event.
        let flight_exposure = f
            .events
            .iter()
            .map(|e| e.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let since = match (exposure_seen, flight_exposure.is_finite()) {
            (_, true) => Some(flight_exposure.max(exposure_seen.unwrap_or(f64::NEG_INFINITY))),
            (Some(x), false) => Some(x),
            (None, false) => None,
        };
        if let Some(since) = since {
            for (t, fl, text) in &alerts {
                if fl != &f.id || *t <= since || !residual_alert(text) {
                    continue;
                }
                if in_event(f, *t, p.alert_margin_s) {
                    continue;
                }
                indicators.push(Indicator {
                    indicator: "alert_without_interference".into(),
                    flight: f.id.clone(),
                    time_utc: iso(*t),
                    value: rnd((*t - since) / 60.0, 1),
                    reference: None,
                    after_reset: reset_between(since, *t),
                    detail: format!(
                        "{} alert '{}' {:.0} min after the exposure, with no event detected at that time",
                        alert_class(text),
                        text,
                        (*t - since) / 60.0
                    ),
                });
            }
        }
        if flight_exposure.is_finite() {
            exposure_seen = Some(flight_exposure);
        }
        summaries.push(FlightSummary {
            id: f.id.clone(),
            start_utc: f.rows.first().map(|r| iso(r.t)).unwrap_or_default(),
            end_utc: f.rows.last().map(|r| iso(r.t)).unwrap_or_default(),
            rows: f.rows.len(),
            jamming_events: f.report.sib.jamming_events,
            spoofing_events: f.report.sib.spoofing_events,
            exposed: !f.events.is_empty(),
            clock_offset_pre_s: pre.map(|v| rnd(v, 3)),
            clock_offset_post_s: post.map(|v| rnd(v, 3)),
            fdm_input_sha256: f.report.input_sha256.clone(),
        });
    }
    let alerts_before = alerts
        .iter()
        .filter(|(t, _, x)| *t < first_exposure && residual_alert(x))
        .count();

    // Outcome.
    let mut reasons = vec![];
    let outcome = if !first_exposure.is_finite() {
        reasons.push(
            "The FDM detector found no jamming or spoofing event in the flights supplied.".into(),
        );
        NO_EXPOSURE
    } else {
        let after_last: Vec<&Indicator> = indicators
            .iter()
            .filter(|i| {
                utc(&i.time_utc).is_ok_and(|t| t >= last_exposure - 1.0)
                    || i.indicator == "degradation_after_area_exit"
            })
            .collect();
        let through_reset = indicators.iter().any(|i| i.after_reset);
        let last_flight_end = flights
            .iter()
            .filter_map(|f| f.rows.last().map(|r| r.t))
            .fold(f64::NEG_INFINITY, f64::max);
        let reset_since = reset_between(last_exposure, f64::INFINITY);
        for i in &indicators {
            reasons.push(format!("{} on {}: {}", i.indicator, i.flight, i.detail));
        }
        if through_reset {
            reasons.push("At least one indicator appeared after a recorded reset that followed the exposure.".into());
            LRU_CHECK
        } else if !after_last.is_empty() && !reset_since {
            RESET_RECOMMENDED
        } else if !indicators.is_empty() && reset_since {
            reasons.push(
                "A reset is recorded after the last exposure and no indicator follows it.".into(),
            );
            NO_RESIDUAL
        } else if last_flight_end - last_exposure < p.settle_s {
            reasons.push(format!(
                "Less than {} s of data after the last exposure: nothing to assess residual effects on.",
                p.settle_s
            ));
            NOT_ASSESSABLE
        } else {
            reasons.push("No residual indicator after the exposure in the data supplied.".into());
            NO_RESIDUAL
        }
    };
    let mut limits = vec![
        "Indicators show that an effect outlasted the detected events; they do not show its cause. Equipment faults look the same.".to_string(),
        "The clock indicator needs a GNSS time column (gnss_utc); the area-exit indicator needs areas, inertial ground speed and track.".into(),
        "The position during an event is carried from the last clean fix by inertial speed and track, for area membership only (kilometre level).".into(),
        "Alerts are classed by their text; an alert genuine for terrain or traffic counts the same.".into(),
    ];
    if alerts_before > 0 {
        limits.push(format!(
            "{alerts_before} TAWS/clock alert(s) occurred before the first exposure: alerts after it are weaker evidence of a residual effect."
        ));
    }
    Ok(ResidualResult {
        format: FORMAT.into(),
        method: METHOD.into(),
        evidence: EVIDENCE.into(),
        statement: DECISION_SUPPORT.into(),
        title: req.title.clone(),
        aircraft: req.aircraft.clone(),
        outcome: outcome.into(),
        reasons,
        clock_reference_s: clock_ref.map(|v| rnd(v, 3)),
        flights: summaries,
        indicators,
        resets_utc: resets.iter().map(|t| iso(*t)).collect(),
        alerts_read: alerts.len(),
        alerts_before_first_exposure: alerts_before,
        limits,
        params: p.clone(),
        inputs: inputs.refs.clone(),
    })
}

/// Read a request file and its inputs, and analyse.
pub fn run(path: &Path) -> Result<(ResidualResult, Inputs), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut inp = Inputs::default();
    inp.add_text("request.toml", "request", &text);
    let mut flights = vec![];
    for f in &req.flights {
        flights.push((f.id.clone(), inp.read(base, &f.csv, "fdm")?));
    }
    let alerts = match &req.alerts {
        Some(a) => Some(inp.read(base, a, "alerts")?),
        None => None,
    };
    let resets = match &req.resets {
        Some(a) => Some(inp.read(base, a, "resets")?),
        None => None,
    };
    let r = analyse(&req, &flights, alerts.as_deref(), resets.as_deref(), &inp)?;
    Ok((r, inp))
}

/// The plain-text summary.
pub fn summary(r: &ResidualResult) -> String {
    let mut s = format!(
        "Residual effects: aircraft {} ({})\nOutcome: {} (MODELLED, a recommendation only)\n{}\nEvidence: {}\n\n",
        r.aircraft, r.title, r.outcome, r.statement, r.evidence
    );
    for f in &r.flights {
        s += &format!(
            "Flight {} {} to {}: {} jamming, {} spoofing event(s); clock offset pre {:?} s, post {:?} s\n",
            f.id, f.start_utc, f.end_utc, f.jamming_events, f.spoofing_events, f.clock_offset_pre_s, f.clock_offset_post_s
        );
    }
    s += "\nReasons:\n";
    for x in &r.reasons {
        s += &format!("- {x}\n");
    }
    s
}
