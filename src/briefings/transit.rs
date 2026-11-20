// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Per-transit exposure history and the fleet view (B0901)
//!
//! From AIS tracks (the documented CSV: `timestamp,vessel_id,lat,lon[,sog_kn]`), each vessel's
//! track is split into transits at gaps longer than `gap_h`. For each transit, every report
//! holds until the next one (at most [`MAX_HOLD_S`]) and that time is attributed to:
//!
//! * the named areas the report lies in (built-in chokepoints and regions, and the user's
//!   areas, for example listed areas), with entry and exit times;
//! * the interference-map layer of that day: degraded, observed and not observed time;
//! * the interference events active at that time whose place lies within `reach_km` plus the
//!   event's radius.
//!
//! Exposure time is the time in a degraded cell or within reach of an active event. Each
//! transit gets a record id (the SHA-256 of its content, floats rounded to 9 significant
//! digits), so the same tracks give the same ids. Transits through `kind = "listed"` areas
//! also give one listed-area record each, naming the area's source.
//!
//! Evidence: MODELLED. Exposure is what the layers and events supplied say about the places
//! and times the vessel was; it is not a measurement of the vessel's receivers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::areas::{self, Area};
use super::layers::{self, CellState, Layer};
use super::{iso, round, sig9, InputRef};
use crate::ievents::Event;

pub const FORMAT: &str = "kshana-pro-transit-history/1";
pub const RESULT_FILE: &str = "transit-history.json";
/// The longest time one report holds for (s).
pub const MAX_HOLD_S: f64 = 1800.0;

pub const METHOD: &str = "Tracks split into transits at gaps longer than gap_h. Each report \
holds until the next (at most 30 min). Held time is attributed to the named areas containing \
the report, to the state of the report's cell in that day's interference-map layers (degraded \
if any layer says so), and to every event active at that time whose place is within reach_km \
plus its radius. Exposure time = time in a degraded cell or within reach of an active event.";

/// The request (`transits.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitRequest {
    pub title: String,
    /// AIS track CSV files.
    pub tracks: Vec<String>,
    #[serde(default)]
    pub events: Vec<String>,
    /// Layer files or directories.
    #[serde(default)]
    pub layers: Vec<String>,
    /// An area file (TOML) with the user's areas (for example listed areas).
    #[serde(default)]
    pub area_file: Option<String>,
    #[serde(default)]
    pub no_builtin_areas: bool,
    #[serde(default = "six")]
    pub gap_h: f64,
    #[serde(default = "ten")]
    pub reach_km: f64,
    /// Vessel display names by id (MMSI or IMO).
    #[serde(default)]
    pub names: BTreeMap<String, String>,
    #[serde(default)]
    pub period_start_utc: Option<String>,
    #[serde(default)]
    pub period_end_utc: Option<String>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn six() -> f64 {
    6.0
}

fn ten() -> f64 {
    10.0
}

impl TransitRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// Time in one area during a transit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaTime {
    pub area_id: String,
    pub name: String,
    pub kind: String,
    pub entered_utc: String,
    pub exited_utc: String,
    pub minutes: f64,
    pub exposure_minutes: f64,
}

/// Time within reach of one event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventTime {
    pub event_id: String,
    pub kind: crate::ievents::Kind,
    pub confidence: f64,
    pub corroborated: bool,
    pub minutes: f64,
}

/// One transit's exposure record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransitRecord {
    pub record_id: String,
    pub vessel_id: String,
    pub vessel_name: String,
    pub start_utc: String,
    pub end_utc: String,
    pub reports: usize,
    pub distance_km: f64,
    pub minutes: f64,
    pub exposure_minutes: f64,
    pub degraded_minutes: f64,
    pub observed_minutes: f64,
    pub not_observed_minutes: f64,
    pub event_minutes: f64,
    pub areas: Vec<AreaTime>,
    pub events: Vec<EventTime>,
}

/// A transit through a listed area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListedAreaRecord {
    pub record_id: String,
    pub transit_record_id: String,
    pub vessel_id: String,
    pub area_id: String,
    pub area_name: String,
    pub area_source: String,
    pub entered_utc: String,
    pub exited_utc: String,
    pub minutes: f64,
    pub exposure_minutes: f64,
}

/// One vessel's line in the fleet view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VesselSummary {
    pub vessel_id: String,
    pub vessel_name: String,
    pub transits: usize,
    pub minutes: f64,
    pub exposure_minutes: f64,
    pub exposure_share: f64,
    pub listed_area_transits: usize,
    pub events_encountered: usize,
    #[serde(default)]
    pub last_exposure_utc: Option<String>,
    /// Area ids with the most exposure time, at most three.
    pub top_areas: Vec<String>,
}

/// The transit history document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransitHistory {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub method: String,
    pub title: String,
    #[serde(default)]
    pub period_start_utc: Option<String>,
    #[serde(default)]
    pub period_end_utc: Option<String>,
    pub transits: Vec<TransitRecord>,
    pub listed_area_records: Vec<ListedAreaRecord>,
    pub fleet: Vec<VesselSummary>,
    pub inputs: Vec<InputRef>,
    pub limits: Vec<String>,
}

/// Whether event `e` is active at `t` and its place is within `reach_m` of `(lat, lon)`.
pub fn event_reaches(e: &Event, t: f64, lat: f64, lon: f64, reach_m: f64) -> bool {
    let (Some(pl), Some(a), Some(b)) = (
        &e.location,
        crate::forensics::inputs::parse_utc(&e.start_utc),
        crate::forensics::inputs::parse_utc(&e.end_utc),
    ) else {
        return false;
    };
    t >= a && t <= b && haversine_m(lat, lon, pl.lat_deg, pl.lon_deg) <= pl.radius_m + reach_m
}

fn record_id(prefix: &str, parts: &[String]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0x1f]);
    }
    let d = h.finalize();
    format!(
        "{prefix}-{}",
        d.iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// One report's position and time.
#[derive(Clone, Copy, Debug)]
pub struct Fix {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: Option<f64>,
    /// ADS-B navigation integrity category, when reported.
    pub nic: Option<u8>,
}

/// Split a time-sorted track at gaps longer than `gap_s`.
pub fn split(track: &[Fix], gap_s: f64) -> Vec<Vec<Fix>> {
    let mut out: Vec<Vec<Fix>> = vec![];
    for f in track {
        match out.last_mut() {
            Some(cur) if f.t - cur.last().map(|x| x.t).unwrap_or(f.t) <= gap_s => cur.push(*f),
            _ => out.push(vec![*f]),
        }
    }
    out
}

/// Build one transit record.
pub fn transit_record(
    vessel_id: &str,
    vessel_name: &str,
    fixes: &[Fix],
    areas: &[Area],
    layers: &[Layer],
    events: &[Event],
    reach_km: f64,
) -> TransitRecord {
    let reach_m = reach_km * 1000.0;
    let (mut minutes, mut exp, mut deg, mut obs, mut nobs, mut evm, mut dist) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    // area id -> (first t, last t, held s, exposure s)
    let mut at: BTreeMap<usize, (f64, f64, f64, f64)> = BTreeMap::new();
    let mut et: BTreeMap<usize, f64> = BTreeMap::new();
    for (k, f) in fixes.iter().enumerate() {
        let hold = match fixes.get(k + 1) {
            Some(n) => {
                dist += haversine_m(f.lat, f.lon, n.lat, n.lon);
                (n.t - f.t).clamp(0.0, MAX_HOLD_S)
            }
            None => 0.0,
        };
        let st = layers::state_at_time(layers, f.t, f.lat, f.lon);
        let hit: Vec<usize> = events
            .iter()
            .enumerate()
            .filter(|(_, e)| event_reaches(e, f.t, f.lat, f.lon, reach_m))
            .map(|(i, _)| i)
            .collect();
        let exposed = st == Some(CellState::Degraded) || !hit.is_empty();
        minutes += hold;
        match st {
            Some(CellState::Degraded) => {
                deg += hold;
                obs += hold;
            }
            Some(CellState::NotDegraded) => obs += hold,
            _ => nobs += hold,
        }
        if !hit.is_empty() {
            evm += hold;
        }
        if exposed {
            exp += hold;
        }
        for i in hit {
            *et.entry(i).or_default() += hold;
        }
        for (i, a) in areas.iter().enumerate() {
            if a.contains(f.lat, f.lon) {
                let e = at.entry(i).or_insert((f.t, f.t, 0.0, 0.0));
                e.1 = f.t + hold;
                e.2 += hold;
                if exposed {
                    e.3 += hold;
                }
            }
        }
    }
    let m = |s: f64| sig9(round(s / 60.0, 3));
    let start = fixes.first().map(|f| f.t).unwrap_or(0.0);
    let end = fixes.last().map(|f| f.t).unwrap_or(0.0);
    let areas_out: Vec<AreaTime> = at
        .iter()
        .map(|(i, (a, b, h, x))| AreaTime {
            area_id: areas[*i].id.clone(),
            name: areas[*i].name.clone(),
            kind: areas[*i].kind.clone(),
            entered_utc: iso(*a),
            exited_utc: iso(*b),
            minutes: m(*h),
            exposure_minutes: m(*x),
        })
        .collect();
    let events_out: Vec<EventTime> = et
        .iter()
        .map(|(i, s)| EventTime {
            event_id: events[*i].id.clone(),
            kind: events[*i].kind,
            confidence: sig9(events[*i].confidence),
            corroborated: events[*i].corroborated,
            minutes: m(*s),
        })
        .collect();
    let mut r = TransitRecord {
        record_id: String::new(),
        vessel_id: vessel_id.into(),
        vessel_name: vessel_name.into(),
        start_utc: iso(start),
        end_utc: iso(end),
        reports: fixes.len(),
        distance_km: sig9(round(dist / 1000.0, 3)),
        minutes: m(minutes),
        exposure_minutes: m(exp),
        degraded_minutes: m(deg),
        observed_minutes: m(obs),
        not_observed_minutes: m(nobs),
        event_minutes: m(evm),
        areas: areas_out,
        events: events_out,
    };
    let body = serde_json::to_string(&r).unwrap_or_default();
    r.record_id = record_id("T", &[body]);
    r
}

/// Load an area catalogue for a request.
pub fn load_catalogue(
    base: &Option<PathBuf>,
    area_file: &Option<String>,
    builtin_off: bool,
    inputs: &mut Vec<InputRef>,
) -> Result<Vec<Area>, String> {
    let file_areas = match area_file {
        Some(f) => {
            let (b, r) = super::read_input(base, f, "areas")?;
            inputs.push(r);
            areas::parse_areas(&String::from_utf8_lossy(&b))?
        }
        None => vec![],
    };
    Ok(areas::catalogue(file_areas, builtin_off))
}

/// Read AIS (`vessel_id`) or ADS-B (`aircraft_id`) tracks into fixes per id.
pub fn load_tracks(
    base: &Option<PathBuf>,
    named: &[String],
    id_col: &str,
    inputs: &mut Vec<InputRef>,
) -> Result<BTreeMap<String, Vec<Fix>>, String> {
    let mut by: BTreeMap<String, Vec<Fix>> = BTreeMap::new();
    for n in named {
        let (b, r) = super::read_input(base, n, "tracks")?;
        inputs.push(r);
        let t = crate::forensics::inputs::parse_tracks(&String::from_utf8_lossy(&b), id_col)
            .map_err(|e| format!("{n}: {e}"))?;
        for (id, v) in t.by_id {
            by.entry(id).or_default().extend(v.into_iter().map(|p| Fix {
                t: p.t,
                lat: p.lat,
                lon: p.lon,
                alt_ft: p.alt_ft,
                nic: p.nic,
            }));
        }
    }
    for v in by.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    Ok(by)
}

/// Build the history.
pub fn build(r: &TransitRequest) -> Result<TransitHistory, String> {
    if r.gap_h <= 0.0 || r.reach_km < 0.0 {
        return Err("gap_h must be positive and reach_km not negative".into());
    }
    let mut inputs = vec![];
    let tracks = load_tracks(&r.base_dir, &r.tracks, "vessel_id", &mut inputs)?;
    let events = super::load_events(&r.base_dir, &r.events, &mut inputs)?;
    let layers = layers::load_layers(&r.base_dir, &r.layers, &mut inputs)?;
    let areas = load_catalogue(&r.base_dir, &r.area_file, r.no_builtin_areas, &mut inputs)?;
    let p0 = r
        .period_start_utc
        .as_deref()
        .map(|s| super::utc("period_start_utc", s))
        .transpose()?;
    let p1 = r
        .period_end_utc
        .as_deref()
        .map(|s| super::utc("period_end_utc", s))
        .transpose()?;
    let mut transits = vec![];
    let mut listed = vec![];
    for (id, fixes) in &tracks {
        let fixes: Vec<Fix> = fixes
            .iter()
            .filter(|f| p0.is_none_or(|a| f.t >= a) && p1.is_none_or(|b| f.t <= b))
            .copied()
            .collect();
        let name = r.names.get(id).cloned().unwrap_or_else(|| id.clone());
        for seg in split(&fixes, r.gap_h * 3600.0) {
            if seg.len() < 2 {
                continue;
            }
            let rec = transit_record(id, &name, &seg, &areas, &layers, &events, r.reach_km);
            for a in rec.areas.iter().filter(|a| a.kind == "listed") {
                let src = areas
                    .iter()
                    .find(|x| x.id == a.area_id)
                    .map(|x| x.source.clone())
                    .unwrap_or_default();
                listed.push(ListedAreaRecord {
                    record_id: record_id(
                        "L",
                        &[
                            rec.record_id.clone(),
                            a.area_id.clone(),
                            a.entered_utc.clone(),
                        ],
                    ),
                    transit_record_id: rec.record_id.clone(),
                    vessel_id: id.clone(),
                    area_id: a.area_id.clone(),
                    area_name: a.name.clone(),
                    area_source: src,
                    entered_utc: a.entered_utc.clone(),
                    exited_utc: a.exited_utc.clone(),
                    minutes: a.minutes,
                    exposure_minutes: a.exposure_minutes,
                });
            }
            transits.push(rec);
        }
    }
    let fleet = fleet(&transits, &listed);
    let mut limits = vec![
        "Exposure is what the layers and events supplied say about the places and times the vessel was; it is not a measurement of the vessel's receivers.".to_string(),
        "Time in cells the layers did not observe is reported as not observed, never as clear.".into(),
        "Built-in areas are coarse boxes for accounting, not legal, listed or charted boundaries; listed-area records name the source of the user's area.".into(),
        "AIS positions may themselves be affected by interference; a spoofed track places exposure at the reported, not the true, position.".into(),
    ];
    if events.iter().any(|e| e.location.is_none()) {
        limits.push("Events with a named area but no location were not placed.".into());
    }
    if layers.is_empty() {
        limits
            .push("No interference-map layer was supplied: exposure rests on events only.".into());
    }
    Ok(TransitHistory {
        format: FORMAT.into(),
        evidence: super::EVIDENCE.into(),
        statement: super::DECISION_SUPPORT.into(),
        method: METHOD.into(),
        title: r.title.clone(),
        period_start_utc: r.period_start_utc.clone(),
        period_end_utc: r.period_end_utc.clone(),
        transits,
        listed_area_records: listed,
        fleet,
        inputs,
        limits,
    })
}

/// The fleet view from transit records.
pub fn fleet(transits: &[TransitRecord], listed: &[ListedAreaRecord]) -> Vec<VesselSummary> {
    let mut by: BTreeMap<&str, Vec<&TransitRecord>> = BTreeMap::new();
    for t in transits {
        by.entry(t.vessel_id.as_str()).or_default().push(t);
    }
    by.into_iter()
        .map(|(id, ts)| {
            let minutes: f64 = ts.iter().map(|t| t.minutes).sum();
            let exp: f64 = ts.iter().map(|t| t.exposure_minutes).sum();
            let mut ev: Vec<&str> = ts
                .iter()
                .flat_map(|t| t.events.iter().map(|e| e.event_id.as_str()))
                .collect();
            ev.sort();
            ev.dedup();
            let mut by_area: BTreeMap<&str, f64> = BTreeMap::new();
            for t in &ts {
                for a in &t.areas {
                    *by_area.entry(a.area_id.as_str()).or_default() += a.exposure_minutes;
                }
            }
            let mut top: Vec<(&str, f64)> = by_area.into_iter().filter(|x| x.1 > 0.0).collect();
            top.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
            VesselSummary {
                vessel_id: id.into(),
                vessel_name: ts[0].vessel_name.clone(),
                transits: ts.len(),
                minutes: sig9(round(minutes, 3)),
                exposure_minutes: sig9(round(exp, 3)),
                exposure_share: if minutes > 0.0 {
                    sig9(round(exp / minutes, 4))
                } else {
                    0.0
                },
                listed_area_transits: listed.iter().filter(|l| l.vessel_id == id).count(),
                events_encountered: ev.len(),
                last_exposure_utc: ts
                    .iter()
                    .filter(|t| t.exposure_minutes > 0.0)
                    .map(|t| t.end_utc.clone())
                    .max(),
                top_areas: top.into_iter().take(3).map(|x| x.0.to_string()).collect(),
            }
        })
        .collect()
}

/// The fleet page.
pub fn render_html(h: &TransitHistory) -> String {
    use super::esc;
    let mut s = super::html_head(&h.title);
    s.push_str(&format!(
        "<h1>{}</h1><p class=\"l\">{}</p><p class=\"l\">{}</p><h2>Fleet</h2><table><tr><th>Vessel</th><th>Transits</th>\
         <th>Hours</th><th>Exposure hours</th><th>Share</th><th>Listed-area transits</th><th>Events</th><th>Last exposure</th><th>Top areas</th></tr>",
        esc(&h.title),
        esc(&h.evidence),
        esc(&h.statement)
    ));
    for v in &h.fleet {
        s.push_str(&format!(
            "<tr><td>{} ({})</td><td>{}</td><td>{:.1}</td><td>{:.1}</td><td>{:.1} %</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&v.vessel_name),
            esc(&v.vessel_id),
            v.transits,
            v.minutes / 60.0,
            v.exposure_minutes / 60.0,
            v.exposure_share * 100.0,
            v.listed_area_transits,
            v.events_encountered,
            esc(v.last_exposure_utc.as_deref().unwrap_or("-")),
            esc(&v.top_areas.join(", "))
        ));
    }
    s.push_str("</table><h2>Transits</h2><table><tr><th>Record</th><th>Vessel</th><th>From</th><th>To</th><th>km</th><th>Exposure min</th><th>Not observed min</th><th>Areas</th><th>Events</th></tr>");
    for t in &h.transits {
        s.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{:.0}</td><td>{:.0}</td><td>{:.0}</td><td>{}</td><td>{}</td></tr>",
            esc(&t.record_id),
            esc(&t.vessel_name),
            esc(&t.start_utc),
            esc(&t.end_utc),
            t.distance_km,
            t.exposure_minutes,
            t.not_observed_minutes,
            esc(&t.areas.iter().map(|a| format!("{} {:.0} min", a.area_id, a.minutes)).collect::<Vec<_>>().join("; ")),
            esc(&t.events.iter().map(|e| e.event_id.clone()).collect::<Vec<_>>().join(", "))
        ));
    }
    s.push_str("</table>");
    if !h.listed_area_records.is_empty() {
        s.push_str("<h2>Listed-area records</h2><table><tr><th>Record</th><th>Vessel</th><th>Area</th><th>Entered</th><th>Exited</th><th>Minutes</th><th>Area source</th></tr>");
        for l in &h.listed_area_records {
            s.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{:.0}</td><td>{}</td></tr>",
                esc(&l.record_id),
                esc(&l.vessel_id),
                esc(&l.area_name),
                esc(&l.entered_utc),
                esc(&l.exited_utc),
                l.minutes,
                esc(&l.area_source)
            ));
        }
        s.push_str("</table>");
    }
    s.push_str("<h2>Limits</h2><ul>");
    for l in &h.limits {
        s.push_str(&format!("<li>{}</li>", esc(l)));
    }
    s.push_str("</ul><h2>Inputs</h2><ul>");
    for i in &h.inputs {
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

/// Write `transit-history.json`, `transits.csv`, `listed-areas.csv`, `fleet.html` and the
/// request.
pub fn write_dir(out: &Path, h: &TransitHistory, request_src: &str) -> Result<(), String> {
    super::write_json(out, RESULT_FILE, h)?;
    let mut csv = String::from("record_id,vessel_id,start_utc,end_utc,distance_km,minutes,exposure_minutes,degraded_minutes,not_observed_minutes,event_minutes,areas,events\n");
    for t in &h.transits {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            t.record_id,
            t.vessel_id,
            t.start_utc,
            t.end_utc,
            t.distance_km,
            t.minutes,
            t.exposure_minutes,
            t.degraded_minutes,
            t.not_observed_minutes,
            t.event_minutes,
            t.areas
                .iter()
                .map(|a| a.area_id.as_str())
                .collect::<Vec<_>>()
                .join(";"),
            t.events
                .iter()
                .map(|e| e.event_id.as_str())
                .collect::<Vec<_>>()
                .join(";")
        ));
    }
    super::write_text(out, "transits.csv", csv.as_bytes())?;
    let mut l = String::from("record_id,transit_record_id,vessel_id,area_id,entered_utc,exited_utc,minutes,exposure_minutes\n");
    for x in &h.listed_area_records {
        l.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            x.record_id,
            x.transit_record_id,
            x.vessel_id,
            x.area_id,
            x.entered_utc,
            x.exited_utc,
            x.minutes,
            x.exposure_minutes
        ));
    }
    super::write_text(out, "listed-areas.csv", l.as_bytes())?;
    super::write_text(out, "fleet.html", render_html(h).as_bytes())?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}
