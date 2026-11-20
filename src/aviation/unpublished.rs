// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Unpublished-interference-area and short-burst detector from fleet data (B1104)
//!
//! Interference that no NOTAM or advisory announces reaches crews unprepared, and short
//! domestic bursts (a few minutes, a dozen aircraft) go unrecorded. From a fleet's ADS-B
//! position reports (the documented CSV) and, optionally, the unified events of its FDM
//! detector runs, this module finds:
//!
//! * **affected aircraft per cell and window**: an aircraft that reported good accuracy
//!   somewhere in the data (the open map's baseline guard) is affected in a cell during a
//!   `window_s` window when it reported NIC below `nic_min` or NACp below `nacp_min` there, a
//!   spoofing-like position jump (at least `jump_km` at more than `jump_kn`, located at the
//!   position before the jump, or after it for a jump back), or an FDM event whose last clean position is in the cell;
//! * **events**: windows with at least `min_aircraft` distinct affected aircraft, merged per
//!   cell when closer than `merge_gap_s`; an event no longer than `short_burst_max_s` is a
//!   **short burst**;
//! * **coverage**: an event is `COVERED` when an advisory (NOTAM text or normalised reports
//!   with a position and radius, or areas stated in the request) intersects its cell and its
//!   validity overlaps the event; otherwise it is `UNPUBLISHED`;
//! * **alerts to operations** for unpublished events, in the alert policy engine's raw table
//!   (`kshana-pro alerts route` routes them), and the event cells as GeoJSON.
//!
//! **Minimum evidence and granularity.** Cells are the open engine's grid at no finer than
//! its pre-registered 0.5° ([`PREREGISTERED_CELL_DEG`]), and no event is published below the
//! open engine's publication minimum of distinct aircraft ([`PUBLICATION_MIN_DISTINCT`]);
//! smaller groups are counted, never located. An event's area is where aircraft were
//! affected, never an emitter position, and aircraft identities are counted, not listed.
//! MODELLED; decision support.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kshana::interference_map::grid::{haversine_m, CellId, Grid, PREREGISTERED_CELL_DEG};
use kshana::interference_map::PUBLICATION_MIN_DISTINCT;
use serde::{Deserialize, Serialize};

use super::*;
use crate::forensics::inputs::{iso, parse_tracks, PositionReport};

pub const FORMAT: &str = "kshana-pro-unpublished-areas/1";
pub const METHOD: &str = "kshana-pro/unpublished-areas/v1";
pub const RESULT_FILE: &str = "unpublished-areas.json";
pub const EVIDENCE: &str = "MODELLED: rules exercised on synthetic fleet traffic with known \
bursts and advisories; not validated against labelled interference events. Areas are where \
aircraft were affected, at 0.5° cells or coarser; no emitter is located.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub cell_deg: f64,
    pub nic_min: u8,
    pub nacp_min: u8,
    pub window_s: f64,
    pub min_aircraft: usize,
    pub merge_gap_s: f64,
    pub short_burst_max_s: f64,
    pub jump_km: f64,
    pub jump_kn: f64,
    /// Minimum altitude (ft) of reports used: low and surface reports lose accuracy for
    /// other reasons (the open map's guard).
    pub min_alt_ft: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            cell_deg: PREREGISTERED_CELL_DEG,
            nic_min: 7,
            nacp_min: 8,
            window_s: 300.0,
            min_aircraft: PUBLICATION_MIN_DISTINCT,
            merge_gap_s: 600.0,
            short_burst_max_s: 900.0,
            jump_km: 5.0,
            jump_kn: 900.0,
            min_alt_ft: 1_000.0,
        }
    }
}

impl Params {
    pub fn check(&self) -> Result<(), String> {
        if !(self.cell_deg.is_finite()
            && self.cell_deg >= PREREGISTERED_CELL_DEG
            && self.cell_deg <= 10.0)
        {
            return Err(format!(
                "params: cell_deg must be at least {PREREGISTERED_CELL_DEG}° (areas are never finer than the open grid) and at most 10°"
            ));
        }
        if self.min_aircraft < PUBLICATION_MIN_DISTINCT {
            return Err(format!(
                "params: min_aircraft must be at least {PUBLICATION_MIN_DISTINCT} (the open engine's publication minimum)"
            ));
        }
        for (n, v) in [
            ("window_s", self.window_s),
            ("jump_km", self.jump_km),
            ("jump_kn", self.jump_kn),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("params: {n} must be above 0"));
            }
        }
        Ok(())
    }
}

/// An advisory stated in the request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advisory {
    pub name: String,
    #[serde(default)]
    pub center: Option<[f64; 2]>,
    #[serde(default)]
    pub radius_nm: Option<f64>,
    #[serde(default, rename = "box")]
    pub area: Option<[f64; 4]>,
    #[serde(default)]
    pub start_utc: Option<String>,
    #[serde(default)]
    pub end_utc: Option<String>,
}

/// An unpublished-areas request (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub adsb: Vec<String>,
    /// Unified events JSON from `forensics fdm` (or any unified events with a location).
    #[serde(default)]
    pub fdm_events: Vec<String>,
    /// NOTAM text, or normalised reports JSON (`reports normalise`).
    #[serde(default)]
    pub notams: Vec<String>,
    #[serde(default)]
    pub advisories: Vec<Advisory>,
    #[serde(default)]
    pub params: Params,
}

#[derive(Clone, Debug, PartialEq)]
enum Shape {
    Circle(f64, f64, f64), // lat, lon, radius m
    Box(GeoBox),
}

#[derive(Clone, Debug, PartialEq)]
struct Adv {
    name: String,
    shape: Shape,
    t0: f64,
    t1: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaEvent {
    pub id: String,
    /// `[south, west, north, east]` of the cell.
    pub cell: [f64; 4],
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    pub short_burst: bool,
    /// `jamming`, `spoofing` or `mixed`.
    pub kind: String,
    pub distinct_aircraft: usize,
    pub integrity_reports: usize,
    pub position_jumps: usize,
    pub fdm_events: usize,
    /// `UNPUBLISHED` or `COVERED`.
    pub status: String,
    pub covered_by: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreasResult {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub statement: String,
    pub title: String,
    pub cell_deg: f64,
    pub publication_minimum_aircraft: usize,
    pub aircraft: usize,
    pub aircraft_with_baseline: usize,
    pub advisories: usize,
    pub advisories_without_area: usize,
    pub events: Vec<AreaEvent>,
    pub unpublished: usize,
    pub short_bursts: usize,
    /// Cell-windows with affected aircraft below the publication minimum (counted, not located).
    pub windows_below_minimum: usize,
    pub rules: Vec<String>,
    pub limits: Vec<String>,
    pub params: Params,
    pub inputs: Vec<InputRef>,
}

fn intersects(s: &Shape, b: (f64, f64, f64, f64)) -> bool {
    let (south, west, north, east) = b;
    match s {
        Shape::Box(g) => g.south <= north && g.north >= south && g.west <= east && g.east >= west,
        Shape::Circle(lat, lon, r) => {
            let (cl, co) = (lat.clamp(south, north), lon.clamp(west, east));
            haversine_m(*lat, *lon, cl, co) <= *r
        }
    }
}

#[derive(Default)]
struct Acc {
    aircraft: BTreeSet<String>,
    integrity: usize,
    jumps: usize,
    fdm: usize,
    fdm_spoofing: usize,
    t0: f64,
    t1: f64,
}

/// Analyse ADS-B texts, unified-event texts and NOTAM texts (each `(name, text)`).
pub fn analyse(
    req: &Request,
    adsb: &[(String, String)],
    fdm_events: &[(String, String)],
    notams: &[(String, String)],
    inputs: &Inputs,
) -> Result<AreasResult, String> {
    let p = &req.params;
    p.check()?;
    let grid = Grid::new(p.cell_deg).ok_or("params: invalid cell_deg")?;
    // Advisories.
    let mut advs: Vec<Adv> = vec![];
    let mut without_area = 0usize;
    let span = |a: Option<&str>, b: Option<&str>| -> Result<(f64, f64), String> {
        Ok((
            a.map(utc).transpose()?.unwrap_or(f64::NEG_INFINITY),
            b.map(utc).transpose()?.unwrap_or(f64::INFINITY),
        ))
    };
    for a in &req.advisories {
        let (t0, t1) = span(a.start_utc.as_deref(), a.end_utc.as_deref())?;
        let shape = match (a.center, a.radius_nm, a.area) {
            (_, _, Some(b)) => Shape::Box(GeoBox::from_array(b)?),
            (Some(c), Some(r), None) => Shape::Circle(c[0], c[1], r * 1852.0),
            _ => {
                return Err(format!(
                    "advisory {}: needs box, or center and radius_nm",
                    a.name
                ))
            }
        };
        advs.push(Adv {
            name: a.name.clone(),
            shape,
            t0,
            t1,
        });
    }
    for (name, text) in notams {
        let reports = if text.trim_start().starts_with('{') {
            crate::ireports::ReportsDoc::from_json(text)
                .map_err(|e| format!("{name}: {e}"))?
                .reports
        } else {
            crate::ireports::normalise::normalise(
                text,
                &crate::ireports::safe_name(name),
                Some(crate::ireports::SourceFormat::Notam),
            )
            .map_err(|e| format!("{name}: {e}"))?
        };
        for r in reports {
            let label = r
                .subject_id
                .clone()
                .unwrap_or_else(|| format!("{} #{}", r.source.file, r.source.record));
            match &r.position {
                Some(pos) => {
                    let t0 = r.start_t().unwrap_or(f64::NEG_INFINITY);
                    let t1 = match &r.occurred_end_utc {
                        Some(e) => utc(e).unwrap_or(f64::INFINITY),
                        None => f64::INFINITY,
                    };
                    advs.push(Adv {
                        name: label,
                        shape: Shape::Circle(pos.lat_deg, pos.lon_deg, pos.radius_m.unwrap_or(0.0)),
                        t0,
                        t1,
                    });
                }
                None => without_area += 1,
            }
        }
    }

    // Tracks.
    let mut tracks: BTreeMap<String, Vec<PositionReport>> = BTreeMap::new();
    for (name, text) in adsb {
        let t = parse_tracks(text, "aircraft_id").map_err(|e| format!("{name}: {e}"))?;
        for (id, v) in t.by_id {
            tracks.entry(id).or_default().extend(v);
        }
    }
    for v in tracks.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    let degraded = |r: &PositionReport| match (r.nic, r.nacp) {
        (None, None) => None,
        (a, b) => Some(a.is_some_and(|v| v < p.nic_min) || b.is_some_and(|v| v < p.nacp_min)),
    };
    let usable = |r: &PositionReport| r.alt_ft.is_none_or(|a| a >= p.min_alt_ft);
    let mut baseline = 0usize;
    // (cell, window) -> accumulator
    let mut acc: BTreeMap<(CellId, i64), Acc> = BTreeMap::new();
    let win = |t: f64| (t / p.window_s).floor() as i64;
    let mut add = |c: CellId, t: f64, id: &str, what: u8| {
        let a = acc.entry((c, win(t))).or_insert_with(|| Acc {
            t0: t,
            t1: t,
            ..Default::default()
        });
        a.aircraft.insert(id.to_string());
        a.t0 = a.t0.min(t);
        a.t1 = a.t1.max(t);
        match what {
            0 => a.integrity += 1,
            1 => a.jumps += 1,
            2 => a.fdm += 1,
            3 => {
                a.fdm += 1;
                a.fdm_spoofing += 1
            }
            _ => {}
        }
    };
    for (id, v) in &tracks {
        if !v.iter().any(|r| usable(r) && degraded(r) == Some(false)) {
            continue;
        }
        baseline += 1;
        for r in v.iter().filter(|r| usable(r)) {
            if degraded(r) == Some(true) {
                add(grid.cell_of(r.lat, r.lon), r.t, id, 0);
            }
        }
        // A jump is located at the position before it; a jump back within the merge gap of
        // an earlier one leaves the false position, so it is located at the position after it.
        let mut onset: Option<f64> = None;
        for w in v.windows(2) {
            let d = haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
            let dt = (w[1].t - w[0].t).max(1e-3);
            if d >= p.jump_km * 1000.0 && d / dt / 0.514_444 > p.jump_kn && usable(&w[0]) {
                let back = onset.is_some_and(|t0| w[1].t - t0 <= p.merge_gap_s);
                let at = if back { &w[1] } else { &w[0] };
                add(grid.cell_of(at.lat, at.lon), w[1].t, id, 1);
                onset = if back { None } else { Some(w[1].t) };
            }
        }
    }
    for (name, text) in fdm_events {
        let v: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("{name}: {e}"))?;
        let arr = match &v {
            serde_json::Value::Array(a) => a.clone(),
            o => o
                .get("unified_events")
                .and_then(|e| e.as_array())
                .cloned()
                .ok_or(format!("{name}: no unified events array"))?,
        };
        for e in arr {
            let ev: crate::ievents::Event =
                serde_json::from_value(e).map_err(|e| format!("{name}: {e}"))?;
            // The source estimate, if any, is never read (no emitter positions).
            let Some(loc) = &ev.location else { continue };
            let (t0, t1) = (utc(&ev.start_utc)?, utc(&ev.end_utc)?);
            let who = ev
                .observers
                .first()
                .cloned()
                .unwrap_or_else(|| ev.id.clone());
            let c = grid.cell_of(loc.lat_deg, loc.lon_deg);
            let mut t = t0;
            // The event counts once; later windows only carry the aircraft.
            let mut what = match ev.kind {
                crate::ievents::Kind::Spoofing | crate::ievents::Kind::Meaconing => 3,
                _ => 2,
            };
            loop {
                add(c, t.min(t1), &who, what);
                what = 9;
                if t >= t1 {
                    break;
                }
                t += p.window_s;
            }
        }
    }

    // Windows -> events per cell.
    let mut below = 0usize;
    let mut per_cell: BTreeMap<CellId, Vec<(i64, &Acc)>> = BTreeMap::new();
    for ((c, w), a) in &acc {
        if a.aircraft.len() >= p.min_aircraft {
            per_cell.entry(*c).or_default().push((*w, a));
        } else {
            below += 1;
        }
    }
    let gap_w = (p.merge_gap_s / p.window_s).floor() as i64 + 1;
    let mut events = vec![];
    for (c, ws) in per_cell {
        let mut groups: Vec<Vec<&Acc>> = vec![];
        let mut last: Option<i64> = None;
        for (w, a) in ws {
            match (last, groups.last_mut()) {
                (Some(l), Some(g)) if w - l <= gap_w => g.push(a),
                _ => groups.push(vec![a]),
            }
            last = Some(w);
        }
        let (s, w0, n, e) = grid.bounds(c);
        for g in groups {
            let mut ids: BTreeSet<&String> = BTreeSet::new();
            let (mut integ, mut jumps, mut fdm, mut fdm_s) = (0, 0, 0, 0);
            let (mut t0, mut t1) = (f64::INFINITY, f64::NEG_INFINITY);
            for a in &g {
                ids.extend(a.aircraft.iter());
                integ += a.integrity;
                jumps += a.jumps;
                fdm += a.fdm;
                fdm_s += a.fdm_spoofing;
                t0 = t0.min(a.t0);
                t1 = t1.max(a.t1);
            }
            let kind = match (integ > 0 || fdm > fdm_s, jumps > 0 || fdm_s > 0) {
                (true, true) => "mixed",
                (false, true) => "spoofing",
                _ => "jamming",
            };
            let covered_by: Vec<String> = advs
                .iter()
                .filter(|a| intersects(&a.shape, (s, w0, n, e)) && a.t0 <= t1 && a.t1 >= t0)
                .map(|a| a.name.clone())
                .collect();
            events.push(AreaEvent {
                id: String::new(),
                cell: [rnd(s, 4), rnd(w0, 4), rnd(n, 4), rnd(e, 4)],
                start_utc: iso(t0),
                end_utc: iso(t1),
                duration_s: rnd(t1 - t0, 1),
                short_burst: t1 - t0 <= p.short_burst_max_s,
                kind: kind.into(),
                distinct_aircraft: ids.len(),
                integrity_reports: integ,
                position_jumps: jumps,
                fdm_events: fdm,
                status: if covered_by.is_empty() {
                    "UNPUBLISHED"
                } else {
                    "COVERED"
                }
                .into(),
                covered_by,
            });
        }
    }
    events.sort_by(|a, b| {
        a.start_utc
            .cmp(&b.start_utc)
            .then(a.cell[0].total_cmp(&b.cell[0]))
            .then(a.cell[1].total_cmp(&b.cell[1]))
    });
    for (k, e) in events.iter_mut().enumerate() {
        e.id = format!("area-{}", k + 1);
    }
    let rules = vec![
        format!("Affected: NIC < {} or NACp < {} at or above {} ft, a jump of at least {} km at more than {} kn, or an FDM event located in the cell; aircraft without a good report are left out.", p.nic_min, p.nacp_min, p.min_alt_ft, p.jump_km, p.jump_kn),
        format!("Event: {}-s windows with at least {} distinct affected aircraft in a {}° cell, merged when within {} s; short burst when at most {} s long.", p.window_s, p.min_aircraft, p.cell_deg, p.merge_gap_s, p.short_burst_max_s),
        "Coverage: COVERED when an advisory's area intersects the cell and its validity overlaps the event; otherwise UNPUBLISHED.".into(),
    ];
    let limits = vec![
        "A NOTAM without a readable position and radius cannot cover anything; such advisories are counted in advisories_without_area.".to_string(),
        "Degraded ADS-B positions are themselves uncertain; at 0.5° cells this moves an event by at most a neighbouring cell.".into(),
        "Equipment faults on several aircraft at once, or a regional cause, look the same; the event says where aircraft were affected, not why.".into(),
    ];
    let unpublished = events.iter().filter(|e| e.status == "UNPUBLISHED").count();
    let short = events.iter().filter(|e| e.short_burst).count();
    Ok(AreasResult {
        format: FORMAT.into(),
        method: METHOD.into(),
        evidence: EVIDENCE.into(),
        statement: DECISION_SUPPORT.into(),
        title: req.title.clone(),
        cell_deg: p.cell_deg,
        publication_minimum_aircraft: PUBLICATION_MIN_DISTINCT,
        aircraft: tracks.len(),
        aircraft_with_baseline: baseline,
        advisories: advs.len(),
        advisories_without_area: without_area,
        events,
        unpublished,
        short_bursts: short,
        windows_below_minimum: below,
        rules,
        limits,
        params: p.clone(),
        inputs: inputs.refs.clone(),
    })
}

/// Alerts to operations for unpublished events (the alert policy engine's raw table).
pub fn alerts_table(r: &AreasResult) -> String {
    let alerts: Vec<crate::alerts::Alert> = r
        .events
        .iter()
        .filter(|e| e.status == "UNPUBLISHED")
        .map(|e| crate::alerts::Alert {
            t_unix: utc(&e.start_utc).unwrap_or(0.0) as i64,
            id: e.id.clone(),
            source: "kshana-pro-aviation-areas".into(),
            kind: format!("unpublished-{}", e.kind),
            severity: if e.kind == "jamming" {
                crate::alerts::Severity::Warning
            } else {
                crate::alerts::Severity::Critical
            },
            area: format!(
                "cell {}/{} to {}/{}",
                e.cell[0], e.cell[1], e.cell[2], e.cell[3]
            ),
            text: format!(
                "{}{} affecting {} aircraft from {} to {} ({} s) with no covering advisory (MODELLED; decision support)",
                if e.short_burst { "Short burst: " } else { "" },
                e.kind,
                e.distinct_aircraft,
                e.start_utc,
                e.end_utc,
                e.duration_s
            ),
        })
        .collect();
    crate::alerts::raw_log(&alerts)
}

/// The event cells as GeoJSON.
pub fn geojson(r: &AreasResult) -> String {
    let features: Vec<serde_json::Value> = r
        .events
        .iter()
        .map(|e| {
            let [s, w, n, x] = e.cell;
            serde_json::json!({
                "type": "Feature",
                "geometry": {"type": "Polygon", "coordinates": [[[w, s], [x, s], [x, n], [w, n], [w, s]]]},
                "properties": {
                    "id": e.id, "status": e.status, "kind": e.kind, "short_burst": e.short_burst,
                    "start_utc": e.start_utc, "end_utc": e.end_utc, "distinct_aircraft": e.distinct_aircraft,
                    "evidence": "MODELLED"
                }
            })
        })
        .collect();
    serde_json::to_string_pretty(
        &serde_json::json!({"type": "FeatureCollection", "features": features}),
    )
    .unwrap_or_default()
        + "\n"
}

/// Read a request file and its inputs, and analyse.
pub fn run(path: &Path) -> Result<(AreasResult, Inputs), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut inp = Inputs::default();
    inp.add_text("request.toml", "request", &text);
    let mut adsb = vec![];
    for a in &req.adsb {
        adsb.push((a.clone(), inp.read(base, a, "adsb")?));
    }
    let mut ev = vec![];
    for a in &req.fdm_events {
        ev.push((a.clone(), inp.read(base, a, "fdm-events")?));
    }
    let mut no = vec![];
    for a in &req.notams {
        no.push((a.clone(), inp.read(base, a, "notam")?));
    }
    let r = analyse(&req, &adsb, &ev, &no, &inp)?;
    Ok((r, inp))
}

/// The plain-text summary.
pub fn summary(r: &AreasResult) -> String {
    let mut s = format!(
        "Unpublished interference areas: {}\n{}\nEvidence: {}\n\n{} event(s), {} unpublished, {} short burst(s); {} cell-window(s) below the publication minimum of {} aircraft\n",
        r.title, r.statement, r.evidence, r.events.len(), r.unpublished, r.short_bursts, r.windows_below_minimum, r.publication_minimum_aircraft
    );
    for e in &r.events {
        s += &format!(
            "  {} {} {} {} to {} ({} s{}), {} aircraft, cell {:?}{}\n",
            e.id,
            e.status,
            e.kind,
            e.start_utc,
            e.end_utc,
            e.duration_s,
            if e.short_burst { ", short burst" } else { "" },
            e.distinct_aircraft,
            e.cell,
            if e.covered_by.is_empty() {
                String::new()
            } else {
                format!(", covered by {}", e.covered_by.join(", "))
            }
        );
    }
    s
}
