// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Aviation exposure: per-tail ledger, low-altitude map, flight briefings and OCC alerts
//! (B0902)
//!
//! * **Tail ledger** ([`tail_ledger`]): from ADS-B tracks (the documented CSV:
//!   `timestamp,aircraft_id,lat,lon[,alt_baro_ft,nic,nacp]`) over a period (90 days by
//!   default), per tail: flights, time observed, exposure time by altitude band (below
//!   `low_altitude_ft`, to 10,000 ft, above, unknown), time the aircraft's own ADS-B reported a
//!   navigation integrity category below `nic_degraded_below`, events encountered, first and
//!   last exposure, and a `review` flag for maintenance planning. It complements the
//!   report-based ledger (`kshana-pro reports ledger`): this one works from tracks and layers.
//! * **Low-altitude map** ([`low_altitude_map`]): per cell (at least 0.5°) and altitude band
//!   (below and above `ceiling_ft`), the share of reports with a degraded NIC, published only
//!   where at least [`kshana::interference_map::PUBLICATION_MIN_DISTINCT`] distinct aircraft
//!   were seen, with no identifiers in the output.
//! * **Flight briefings and OCC alerts** ([`occ`]): for planned flights (waypoints with
//!   altitude), the layers and events of a look-back window, the route exposure, events near
//!   the route, an alert level (`advisory` or `information`), a stated confidence and the data
//!   latency (`as_of` minus the latest observation the alert rests on; the open observatory
//!   publishes a day no earlier than 24 h after it ends). A single flight is a pre-flight
//!   briefing.
//!
//! MODELLED. A planning and awareness aid; not an airworthiness determination, an operational
//! instruction or a NOTAM.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::{haversine_m, CellId, Grid};
use serde::{Deserialize, Serialize};

use super::layers::{self, CellState, Layer};
use super::transit::{event_reaches, load_tracks, split, Fix};
use super::{iso, round, sig9, Confidence, InputRef};
use crate::ievents::Event;

pub const LEDGER_FORMAT: &str = "kshana-pro-tail-exposure-ledger/1";
pub const LEDGER_FILE: &str = "tail-ledger.json";
pub const LOWALT_FORMAT: &str = "kshana-pro-low-altitude-map/1";
pub const LOWALT_FILE: &str = "low-altitude-map.geojson";
pub const OCC_FORMAT: &str = "kshana-pro-flight-exposure-alerts/1";
pub const OCC_FILE: &str = "flight-alerts.json";

/// The open observatory's minimum release delay (h).
pub const OBSERVATORY_DELAY_H: f64 = kshana::interference_map::observatory::MIN_DELAY_HOURS;

/// The altitude band of a report.
pub fn band(alt_ft: Option<f64>, low_ft: f64) -> &'static str {
    match alt_ft {
        None => "unknown",
        Some(a) if a < low_ft => "low",
        Some(a) if a < 10_000.0 => "mid",
        Some(_) => "high",
    }
}

/// The ledger request (`tails.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerRequest {
    pub title: String,
    pub tracks: Vec<String>,
    #[serde(default)]
    pub layers: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    /// ICAO 24-bit address (or call sign) → registration.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    pub period_end_utc: String,
    #[serde(default = "ninety")]
    pub period_days: f64,
    #[serde(default = "thirty")]
    pub flight_gap_min: f64,
    #[serde(default = "ten")]
    pub reach_km: f64,
    #[serde(default = "seven")]
    pub nic_degraded_below: u8,
    #[serde(default = "low_ft")]
    pub low_altitude_ft: f64,
    #[serde(default = "sixty")]
    pub flag_exposure_minutes: f64,
    #[serde(default = "ten")]
    pub flag_own_degraded_minutes: f64,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn ninety() -> f64 {
    90.0
}
fn thirty() -> f64 {
    30.0
}
fn ten() -> f64 {
    10.0
}
fn seven() -> u8 {
    7
}
fn low_ft() -> f64 {
    2000.0
}
fn sixty() -> f64 {
    60.0
}

impl LedgerRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// One tail's line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TailEntry {
    pub tail: String,
    /// The ids merged under the tail.
    pub ids: Vec<String>,
    pub flights: usize,
    pub days_flown: usize,
    pub observed_minutes: f64,
    pub exposure_minutes: f64,
    /// `low`, `mid`, `high`, `unknown`.
    pub exposure_minutes_by_band: BTreeMap<String, f64>,
    pub own_nic_degraded_minutes: f64,
    pub events_encountered: Vec<String>,
    #[serde(default)]
    pub first_exposure_utc: Option<String>,
    #[serde(default)]
    pub last_exposure_utc: Option<String>,
    /// `review` or `none`.
    pub flag: String,
    pub flag_reason: String,
}

/// The ledger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TailLedger {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub title: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub period_days: f64,
    pub rules: Vec<String>,
    pub entries: Vec<TailEntry>,
    pub inputs: Vec<InputRef>,
    pub limits: Vec<String>,
}

/// Build the per-tail ledger.
pub fn tail_ledger(r: &LedgerRequest) -> Result<TailLedger, String> {
    if r.period_days <= 0.0 {
        return Err("period_days must be positive".into());
    }
    let end = super::utc("period_end_utc", &r.period_end_utc)?;
    let start = end - r.period_days * 86_400.0;
    let mut inputs = vec![];
    let tracks = load_tracks(&r.base_dir, &r.tracks, "aircraft_id", &mut inputs)?;
    let events = super::load_events(&r.base_dir, &r.events, &mut inputs)?;
    let lay = layers::load_layers(&r.base_dir, &r.layers, &mut inputs)?;
    // Merge ids under their registration.
    let mut by_tail: BTreeMap<String, (Vec<String>, Vec<Fix>)> = BTreeMap::new();
    for (id, fixes) in tracks {
        let tail = r.aliases.get(&id).cloned().unwrap_or_else(|| id.clone());
        let e = by_tail.entry(tail).or_default();
        e.0.push(id);
        e.1.extend(fixes.into_iter().filter(|f| f.t >= start && f.t <= end));
    }
    let reach = r.reach_km * 1000.0;
    let mut entries = vec![];
    for (tail, (ids, mut fixes)) in by_tail {
        fixes.sort_by(|a, b| a.t.total_cmp(&b.t));
        if fixes.is_empty() {
            continue;
        }
        let flights = split(&fixes, r.flight_gap_min * 60.0);
        let mut days: BTreeSet<i64> = BTreeSet::new();
        let (mut obs, mut exp, mut own) = (0.0, 0.0, 0.0);
        let mut by_band: BTreeMap<String, f64> = BTreeMap::new();
        let mut evs: BTreeSet<String> = BTreeSet::new();
        let (mut first, mut last): (Option<f64>, Option<f64>) = (None, None);
        for fl in &flights {
            for (k, f) in fl.iter().enumerate() {
                days.insert((f.t / 86_400.0).floor() as i64);
                let hold = fl
                    .get(k + 1)
                    .map(|n| (n.t - f.t).clamp(0.0, super::transit::MAX_HOLD_S))
                    .unwrap_or(0.0);
                obs += hold;
                let deg =
                    layers::state_at_time(&lay, f.t, f.lat, f.lon) == Some(CellState::Degraded);
                let hit: Vec<&Event> = events
                    .iter()
                    .filter(|e| event_reaches(e, f.t, f.lat, f.lon, reach))
                    .collect();
                let nic = f.nic;
                if nic.is_some_and(|n| n < r.nic_degraded_below) {
                    own += hold;
                }
                if deg || !hit.is_empty() {
                    exp += hold;
                    *by_band
                        .entry(band(f.alt_ft, r.low_altitude_ft).into())
                        .or_default() += hold / 60.0;
                    first = Some(first.map_or(f.t, |x: f64| x.min(f.t)));
                    last = Some(last.map_or(f.t, |x: f64| x.max(f.t)));
                    for e in hit {
                        evs.insert(e.id.clone());
                    }
                }
            }
        }
        for b in ["low", "mid", "high", "unknown"] {
            by_band.entry(b.into()).or_default();
        }
        for v in by_band.values_mut() {
            *v = sig9(round(*v, 3));
        }
        let (em, om) = (exp / 60.0, own / 60.0);
        let mut reasons = vec![];
        if em >= r.flag_exposure_minutes {
            reasons.push(format!(
                "{em:.0} exposure minutes (flag at {:.0})",
                r.flag_exposure_minutes
            ));
        }
        if om >= r.flag_own_degraded_minutes {
            reasons.push(format!(
                "{om:.0} minutes with own NIC below {} (flag at {:.0})",
                r.nic_degraded_below, r.flag_own_degraded_minutes
            ));
        }
        entries.push(TailEntry {
            tail,
            ids,
            flights: flights.iter().filter(|f| f.len() >= 2).count(),
            days_flown: days.len(),
            observed_minutes: sig9(round(obs / 60.0, 3)),
            exposure_minutes: sig9(round(em, 3)),
            exposure_minutes_by_band: by_band,
            own_nic_degraded_minutes: sig9(round(om, 3)),
            events_encountered: evs.into_iter().collect(),
            first_exposure_utc: first.map(iso),
            last_exposure_utc: last.map(iso),
            flag: if reasons.is_empty() { "none" } else { "review" }.into(),
            flag_reason: if reasons.is_empty() {
                "below both flag thresholds".into()
            } else {
                reasons.join("; ")
            },
        });
    }
    let mut limits = vec![
        "Exposure is what the layers and events supplied say about where and when the aircraft flew; the own-NIC minutes are what the aircraft itself reported over ADS-B. Neither is a test of the installed equipment.".to_string(),
        "A planning aid for maintenance; the manufacturer's and operator's procedures decide any action on an aircraft.".into(),
        "ADS-B coverage below 2,000 ft is sparse: low-band minutes cover only the low-altitude flying that receivers saw.".into(),
    ];
    if r.period_days < 90.0 {
        limits.push(format!(
            "The period is {:.0} days, shorter than the 90 days the ledger is designed for.",
            r.period_days
        ));
    }
    Ok(TailLedger {
        format: LEDGER_FORMAT.into(),
        evidence: super::EVIDENCE.into(),
        statement: "A per-tail exposure ledger for maintenance planning. Not an airworthiness determination and not an instruction.".into(),
        title: r.title.clone(),
        period_start_utc: iso(start),
        period_end_utc: iso(end),
        period_days: r.period_days,
        rules: vec![
            format!("Flights split at gaps over {:.0} min; each report holds until the next, at most 30 min.", r.flight_gap_min),
            format!("Exposure: in a degraded layer cell that day, or within {:.0} km plus radius of an active event.", r.reach_km),
            format!("Bands: low below {:.0} ft, mid to 10,000 ft, high above, unknown without altitude.", r.low_altitude_ft),
            format!("Flag review at {:.0} exposure minutes or {:.0} minutes with own NIC below {}.", r.flag_exposure_minutes, r.flag_own_degraded_minutes, r.nic_degraded_below),
        ],
        entries,
        inputs,
        limits,
    })
}

/// The low-altitude map request (`lowalt.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LowAltRequest {
    pub title: String,
    pub tracks: Vec<String>,
    #[serde(default = "half")]
    pub cell_deg: f64,
    #[serde(default = "low_ft")]
    pub ceiling_ft: f64,
    #[serde(default = "seven")]
    pub nic_degraded_below: u8,
    /// Share of a cell's reports with degraded NIC that marks it degraded.
    #[serde(default = "quarter")]
    pub degraded_share: f64,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn half() -> f64 {
    0.5
}
fn quarter() -> f64 {
    0.25
}

impl LowAltRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// Build the low-altitude map (GeoJSON, no identifiers).
pub fn low_altitude_map(r: &LowAltRequest) -> Result<serde_json::Value, String> {
    if r.cell_deg < super::MIN_CELL_DEG - 1e-9 {
        return Err(format!(
            "cell_deg must be at least {} degrees",
            super::MIN_CELL_DEG
        ));
    }
    let grid = Grid::new(r.cell_deg).ok_or("invalid cell_deg")?;
    let mut inputs = vec![];
    // (cell, band) -> (distinct ids, reports, degraded reports)
    type Acc = (BTreeSet<String>, usize, usize);
    let mut acc: BTreeMap<(CellId, &'static str), Acc> = BTreeMap::new();
    for n in &r.tracks {
        let (b, rf) = super::read_input(&r.base_dir, n, "tracks")?;
        inputs.push(rf);
        let t = crate::forensics::inputs::parse_tracks(&String::from_utf8_lossy(&b), "aircraft_id")
            .map_err(|e| format!("{n}: {e}"))?;
        for (id, v) in t.by_id {
            for x in v {
                let (Some(alt), Some(nic)) = (x.alt_ft, x.nic) else {
                    continue;
                };
                let bnd = if alt < r.ceiling_ft { "below" } else { "above" };
                let e = acc.entry((grid.cell_of(x.lat, x.lon), bnd)).or_default();
                e.0.insert(id.clone());
                e.1 += 1;
                if nic < r.nic_degraded_below {
                    e.2 += 1;
                }
            }
        }
    }
    let min = kshana::interference_map::PUBLICATION_MIN_DISTINCT;
    let mut features = vec![];
    for ((c, bnd), (ids, n, d)) in &acc {
        if ids.len() < min {
            continue;
        }
        let share = *d as f64 / *n as f64;
        let (s, w, nn, e) = grid.bounds(*c);
        features.push(serde_json::json!({
            "type": "Feature",
            "properties": {
                "cell_i": c.i, "cell_j": c.j, "band": bnd,
                "aircraft_observed": ids.len(), "reports": n, "reports_nic_degraded": d,
                "share_nic_degraded": sig9(round(share, 4)),
                "status": if share >= r.degraded_share { "degraded" } else { "not_degraded" },
            },
            "geometry": {"type": "Polygon", "coordinates": [[[w, s], [e, s], [e, nn], [w, nn], [w, s]]]}
        }));
    }
    Ok(serde_json::json!({
        "type": "FeatureCollection",
        "kshana_pro_low_altitude_map": {
            "format": LOWALT_FORMAT,
            "title": r.title,
            "evidence": super::EVIDENCE,
            "method": format!(
                "Reports with altitude and NIC binned by {} degree cell and band (below / above {:.0} ft); a cell-band is published only with at least {min} distinct aircraft; degraded when at least {:.0} % of its reports have NIC below {}.",
                r.cell_deg, r.ceiling_ft, 100.0 * r.degraded_share, r.nic_degraded_below
            ),
            "notice": "Aggregate only: no aircraft identifiers. A cell-band missing from the map was not observed by enough aircraft; that is not evidence of a clear cell. Degraded NIC does not identify the cause.",
            "inputs": inputs,
        },
        "features": features
    }))
}

/// A planned flight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlightPlan {
    pub id: String,
    #[serde(default)]
    pub tail: Option<String>,
    pub departure_utc: String,
    /// `[lat, lon, alt_ft]`.
    pub waypoints: Vec<[f64; 3]>,
    /// `hems`, `bizav`, `airline`, `other` (wording only).
    #[serde(default)]
    pub operation: Option<String>,
}

/// The OCC request (`occ.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OccRequest {
    pub title: String,
    /// The time the alerts are made for; the look-back window ends here.
    pub as_of_utc: String,
    #[serde(default = "seven_f")]
    pub lookback_days: f64,
    pub flights: Vec<FlightPlan>,
    #[serde(default)]
    pub layers: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "twenty_five")]
    pub corridor_km: f64,
    #[serde(default = "low_ft")]
    pub low_altitude_ft: f64,
    /// A low-altitude map (`briefing lowalt`) that covers low segments, if any.
    #[serde(default)]
    pub low_altitude_map: Option<String>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn seven_f() -> f64 {
    7.0
}
fn twenty_five() -> f64 {
    25.0
}

impl OccRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// One flight's alert.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlightAlert {
    pub flight_id: String,
    #[serde(default)]
    pub tail: Option<String>,
    pub departure_utc: String,
    /// `advisory` (recent degradation or events on or near the route) or `information`.
    pub level: String,
    pub route_km: f64,
    pub low_altitude_km: f64,
    pub layer_days: usize,
    pub days_with_degraded: usize,
    pub mean_share_degraded_of_observed: f64,
    pub mean_share_not_observed: f64,
    pub events_near_route: Vec<String>,
    /// Low-altitude map cells on the route marked degraded (below the ceiling band).
    pub low_altitude_degraded_cells: usize,
    pub summary: String,
    pub confidence: Confidence,
    /// `as_of` minus the latest observation this alert rests on (h).
    #[serde(default)]
    pub data_latency_h: Option<f64>,
}

/// The alert list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlightAlerts {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub confidence_method: String,
    pub title: String,
    pub as_of_utc: String,
    pub window_from_utc: String,
    pub latency_note: String,
    pub alerts: Vec<FlightAlert>,
    pub inputs: Vec<InputRef>,
    pub data_licences: Vec<String>,
    pub limits: Vec<String>,
}

fn low_km(w: &[[f64; 3]], low: f64) -> f64 {
    w.windows(2)
        .filter(|p| p[0][2] < low || p[1][2] < low)
        .map(|p| haversine_m(p[0][0], p[0][1], p[1][0], p[1][1]))
        .sum::<f64>()
        / 1000.0
}

/// Build the OCC alert list (one flight: a pre-flight briefing).
pub fn occ(r: &OccRequest) -> Result<FlightAlerts, String> {
    let now = super::utc("as_of_utc", &r.as_of_utc)?;
    if r.lookback_days <= 0.0 {
        return Err("lookback_days must be positive".into());
    }
    let from = now - r.lookback_days * 86_400.0;
    let mut inputs = vec![];
    let all_events = super::load_events(&r.base_dir, &r.events, &mut inputs)?;
    let all_layers = layers::load_layers(&r.base_dir, &r.layers, &mut inputs)?;
    let low_map: Option<serde_json::Value> = match &r.low_altitude_map {
        Some(f) => {
            let (b, rf) = super::read_input(&r.base_dir, f, "low-altitude-map")?;
            inputs.push(rf);
            let v: serde_json::Value =
                serde_json::from_slice(&b).map_err(|e| format!("{f}: {e}"))?;
            if v["kshana_pro_low_altitude_map"]["format"] != LOWALT_FORMAT {
                return Err(format!("{f}: not a low-altitude map"));
            }
            Some(v)
        }
        None => None,
    };
    // Only what was known at as_of: layers whose day ended by then, events started by then.
    let lay: Vec<&Layer> = all_layers
        .iter()
        .filter(|l| l.day_start + 86_400.0 > from && l.day_start + 86_400.0 <= now)
        .collect();
    let events: Vec<Event> = all_events
        .iter()
        .filter(|e| {
            let (Some(a), Some(b)) = (
                crate::forensics::inputs::parse_utc(&e.start_utc),
                crate::forensics::inputs::parse_utc(&e.end_utc),
            ) else {
                return false;
            };
            a <= now && b >= from
        })
        .cloned()
        .collect();
    let latest =
        lay.iter()
            .map(|l| l.day_start + 86_400.0)
            .chain(events.iter().filter_map(|e| {
                crate::forensics::inputs::parse_utc(&e.end_utc).map(|b| b.min(now))
            }))
            .fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t))));
    let days = super::days_between(from, now);
    let mut alerts = vec![];
    for f in &r.flights {
        if f.waypoints.len() < 2 {
            return Err(format!("flight {}: needs at least two waypoints", f.id));
        }
        super::utc("departure_utc", &f.departure_utc)?;
        let w2: Vec<[f64; 2]> = f.waypoints.iter().map(|w| [w[0], w[1]]).collect();
        let rb = super::brief::route_brief(
            &f.id,
            &w2,
            &lay,
            &events,
            &r.as_of_utc,
            r.corridor_km,
            None,
            days,
        )?;
        let lk = low_km(&f.waypoints, r.low_altitude_ft);
        let mut low_deg = 0;
        let mut conf = rb.confidence.clone();
        if lk > 0.0 {
            match &low_map {
                Some(m) => {
                    let mut cells: BTreeSet<(i64, i64)> = BTreeSet::new();
                    for p in f.waypoints.windows(2) {
                        if p[0][2] >= r.low_altitude_ft && p[1][2] >= r.low_altitude_ft {
                            continue;
                        }
                        for k in 0..=20 {
                            let x = k as f64 / 20.0;
                            let (la, lo) = (
                                p[0][0] + x * (p[1][0] - p[0][0]),
                                p[0][1] + x * (p[1][1] - p[0][1]),
                            );
                            for feat in m["features"].as_array().into_iter().flatten() {
                                let pr = &feat["properties"];
                                if pr["band"] != "below" || pr["status"] != "degraded" {
                                    continue;
                                }
                                let ring = &feat["geometry"]["coordinates"][0];
                                let (w, s) = (ring[0][0].as_f64(), ring[0][1].as_f64());
                                let (e, n) = (ring[2][0].as_f64(), ring[2][1].as_f64());
                                if let (Some(w), Some(s), Some(e), Some(n)) = (w, s, e, n) {
                                    if la >= s && la < n && lo >= w && lo < e {
                                        cells.insert((
                                            pr["cell_i"].as_i64().unwrap_or(0),
                                            pr["cell_j"].as_i64().unwrap_or(0),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    low_deg = cells.len();
                }
                None => conf.lower("the flight has low-altitude segments and no low-altitude observations were supplied"),
            }
        }
        let advisory =
            rb.days_with_degraded > 0 || !rb.contributing_events.is_empty() || low_deg > 0;
        let latency = latest.map(|t| round((now - t) / 3600.0, 1));
        let op = f.operation.as_deref().unwrap_or("flight");
        let summary = format!(
            "{} ({op}, {:.0} km, {:.0} km below {:.0} ft): {} In the {:.0} days to {}: degraded on {} of {} layer day(s), {} event(s) within {:.0} km{}. Confidence {}.",
            f.id,
            rb.route_km,
            lk,
            r.low_altitude_ft,
            if advisory { "ADVISORY: past interference recorded on or near the route." } else { "INFORMATION: no interference recorded on or near the route in the data supplied." },
            r.lookback_days,
            r.as_of_utc,
            rb.days_with_degraded,
            rb.days.len(),
            rb.contributing_events.len(),
            r.corridor_km,
            if low_deg > 0 { format!(", {low_deg} degraded low-altitude cell(s)") } else { String::new() },
            conf.level
        );
        alerts.push(FlightAlert {
            flight_id: f.id.clone(),
            tail: f.tail.clone(),
            departure_utc: f.departure_utc.clone(),
            level: if advisory { "advisory" } else { "information" }.into(),
            route_km: rb.route_km,
            low_altitude_km: sig9(round(lk, 1)),
            layer_days: rb.days.len(),
            days_with_degraded: rb.days_with_degraded,
            mean_share_degraded_of_observed: rb.mean_share_degraded_of_observed,
            mean_share_not_observed: rb.mean_share_not_observed,
            events_near_route: rb.contributing_events,
            low_altitude_degraded_cells: low_deg,
            summary,
            confidence: conf,
            data_latency_h: latency,
        });
    }
    alerts.sort_by(|a, b| {
        (a.level != "advisory")
            .cmp(&(b.level != "advisory"))
            .then(a.departure_utc.cmp(&b.departure_utc))
            .then(a.flight_id.cmp(&b.flight_id))
    });
    Ok(FlightAlerts {
        format: OCC_FORMAT.into(),
        evidence: super::EVIDENCE.into(),
        statement: super::DECISION_SUPPORT.into(),
        confidence_method: super::CONFIDENCE_METHOD.into(),
        title: r.title.clone(),
        as_of_utc: r.as_of_utc.clone(),
        window_from_utc: iso(from),
        latency_note: format!(
            "Data latency is as_of minus the latest observation the alert rests on. Open observatory layers are published no earlier than {OBSERVATORY_DELAY_H:.0} h after their day ends, so a layer-based alert is at least that old; events are as recent as their reporters made them. Only layers whose day had ended and events that had started by as_of are used."
        ),
        alerts,
        inputs,
        data_licences: super::brief::licences(&lay),
        limits: vec![
            "Advisory means past interference was recorded on or near the route in the window; it is not a forecast that the flight will be affected, and information does not mean the route is clear.".into(),
            "ADS-B layers mostly reflect aircraft at altitude; interference from a surface source usually reaches less far at low altitude, and low-altitude coverage is sparse.".into(),
            "Waypoints are joined by straight lines in latitude and longitude.".into(),
        ],
    })
}

/// The alert page.
pub fn render_occ_html(a: &FlightAlerts) -> String {
    use super::esc;
    let mut s = super::html_head(&a.title);
    s.push_str(&format!(
        "<h1>{}</h1><p><b>As of</b> {} (look-back from {}).</p><p class=\"l\">{}</p><p class=\"l\">{}</p>\
         <table><tr><th>Flight</th><th>Departure</th><th>Level</th><th>Summary</th><th>Confidence</th><th>Data latency</th></tr>",
        esc(&a.title),
        esc(&a.as_of_utc),
        esc(&a.window_from_utc),
        esc(&a.statement),
        esc(&a.latency_note)
    ));
    for x in &a.alerts {
        s.push_str(&format!(
            "<tr><td>{}{}</td><td>{}</td><td><b>{}</b></td><td>{}</td><td class=\"c-{}\">{}<br><span class=\"l\">{}</span></td><td>{}</td></tr>",
            esc(&x.flight_id),
            x.tail.as_ref().map(|t| format!(" ({})", esc(t))).unwrap_or_default(),
            esc(&x.departure_utc),
            esc(&x.level.to_uppercase()),
            esc(&x.summary),
            esc(&x.confidence.level),
            esc(&x.confidence.level),
            esc(&x.confidence.reasons.join("; ")),
            x.data_latency_h.map(|h| format!("{h:.1} h")).unwrap_or_else(|| "no data".into())
        ));
    }
    s.push_str("</table><ul>");
    for l in &a.limits {
        s.push_str(&format!("<li>{}</li>", esc(l)));
    }
    for i in &a.inputs {
        s.push_str(&format!(
            "<li>{} ({}) sha256 <code>{}</code></li>",
            esc(&i.file),
            esc(&i.role),
            esc(&i.sha256)
        ));
    }
    s.push_str("</ul></body></html>\n");
    s
}

/// Write the alerts: JSON, HTML, PDF (the pre-flight briefing) and the request.
pub fn write_occ(out: &Path, a: &FlightAlerts, request_src: &str) -> Result<(), String> {
    super::write_json(out, OCC_FILE, a)?;
    super::write_text(out, "flight-alerts.html", render_occ_html(a).as_bytes())?;
    let mut sections = vec![(
        "What this is".to_string(),
        vec![
            a.statement.clone(),
            a.evidence.clone(),
            a.latency_note.clone(),
        ],
    )];
    for x in &a.alerts {
        sections.push((
            format!("{} - {}", x.flight_id, x.level.to_uppercase()),
            vec![
                x.summary.clone(),
                format!(
                    "Confidence {}: {}.",
                    x.confidence.level,
                    x.confidence.reasons.join("; ")
                ),
                format!(
                    "Data latency: {}.",
                    x.data_latency_h
                        .map(|h| format!("{h:.1} h"))
                        .unwrap_or_else(|| "no data".into())
                ),
            ],
        ));
    }
    let mut rest: Vec<String> = a.limits.iter().map(|l| format!("- {l}")).collect();
    rest.extend(
        a.inputs
            .iter()
            .map(|i| format!("{} ({}) sha256 {}", i.file, i.role, i.sha256)),
    );
    rest.extend(a.data_licences.iter().cloned());
    sections.push(("Limits and inputs".into(), rest));
    let id = crate::forensics::inputs::sha256_hex(
        serde_json::to_string(a).unwrap_or_default().as_bytes(),
    );
    let pdf = super::simple_pdf(
        &format!("Flight exposure briefing: {}", a.title),
        &format!(
            "Flight exposure briefing - as of {} - MODELLED",
            a.as_of_utc
        ),
        &id[..16],
        &sections,
    )?;
    super::write_text(out, "flight-briefing.pdf", &pdf)?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}

/// Write the ledger: JSON, CSV and the request.
pub fn write_ledger(out: &Path, l: &TailLedger, request_src: &str) -> Result<(), String> {
    super::write_json(out, LEDGER_FILE, l)?;
    let mut csv = String::from("tail,flights,days_flown,observed_minutes,exposure_minutes,low_minutes,mid_minutes,high_minutes,unknown_minutes,own_nic_degraded_minutes,events,first_exposure_utc,last_exposure_utc,flag\n");
    for e in &l.entries {
        let b = |k: &str| e.exposure_minutes_by_band.get(k).copied().unwrap_or(0.0);
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            e.tail,
            e.flights,
            e.days_flown,
            e.observed_minutes,
            e.exposure_minutes,
            b("low"),
            b("mid"),
            b("high"),
            b("unknown"),
            e.own_nic_degraded_minutes,
            e.events_encountered.join(";"),
            e.first_exposure_utc.as_deref().unwrap_or(""),
            e.last_exposure_utc.as_deref().unwrap_or(""),
            e.flag
        ));
    }
    super::write_text(out, "tail-ledger.csv", csv.as_bytes())?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}
