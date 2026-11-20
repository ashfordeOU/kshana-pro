// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Spillover footprint of declared EW/test areas and NOTAM geometry (B0905)
//!
//! * **NOTAM to geometry** ([`parse_notams`], free): the Q) line circle, the E) item polygon
//!   (`6930N 02900E - 7010N 02900E - ...`) or circle (`RADIUS 30NM CENTRED ON 6930N03000E`),
//!   B)/C) times with `EST` and `PERM`, the D) schedule in the forms `HHMM-HHMM` and
//!   `DAILY HHMM-HHMM`, and F)/G) limits, with the harmonised start and end criteria
//!   ([`HARMONISED_CRITERIA`]) applied the same way to every NOTAM. Output as JSON and GeoJSON.
//! * **Footprint** ([`footprint`], Pro): from the **declared** area (the NOTAM's own geometry,
//!   or a declared test area), never from observations, the cells (at least 0.5°) a surface
//!   source anywhere inside the area could reach by line of sight at each receiver altitude:
//!   radio horizon `4.12 × (√h_source + √h_receiver)` km (4/3 Earth), an inner ring with the
//!   lower assumed source height and an outer ring with the higher, capped at `max_range_km`
//!   (the power is unknown, so the cap is an assumption stated in the output). Terrain is not
//!   modelled (smooth Earth), which can only overstate the footprint.
//! * **Affected users**: the aircraft and vessels in the supplied tracks whose cell lay in the
//!   footprint for their altitude while the area was active, with time inside.
//! * **Agreement**: per layer day in the active period, the footprint at the agreement altitude
//!   against the layer's degraded cells: degraded inside, degraded outside (within twice the
//!   outer range), observed footprint cells not degraded, footprint cells not observed.
//!
//! Export-control boundary: no step estimates where an emitter is. The footprint starts from the
//! area an authority declared and is published only as cells of at least 0.5° or as that
//! declared area. MODELLED.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::{CellId, Grid};
use serde::{Deserialize, Serialize};

use super::areas::point_in_polygon;
use super::layers::{CellState, Layer};
use super::road::point_segment;
use super::{iso, round, sig9, InputRef};
use crate::ireports::text;

pub const NOTAM_FORMAT: &str = "kshana-pro-notam-geometry/1";
pub const FOOTPRINT_FORMAT: &str = "kshana-pro-spillover-footprint/1";
pub const FOOTPRINT_FILE: &str = "footprint.json";

/// How start and end are read, for every NOTAM alike.
pub const HARMONISED_CRITERIA: &str = "Start: the B) item. End: the C) item; an EST end is kept \
and flagged estimated; PERM has no end and is flagged. Active periods: the D) schedule when it \
is in a form read here (HHMM-HHMM, DAILY HHMM-HHMM, with or without H24), one window per day \
from start to end; otherwise the whole B) to C) period is active and the schedule text is kept \
for the reader. Times are UTC. Vertical limits: F) and G) when given, else the Q) line's lower \
and upper flight levels.";

/// A NOTAM's geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Geometry {
    /// `polygon` or `circle`.
    pub kind: String,
    /// `[lat, lon]` vertices (polygon) or a 32-point ring around the circle.
    pub polygon: Vec<[f64; 2]>,
    #[serde(default)]
    pub center: Option<[f64; 2]>,
    #[serde(default)]
    pub radius_km: Option<f64>,
    /// `E) item` or `Q) line`.
    pub source: String,
}

/// One NOTAM read to geometry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotamGeometry {
    pub id: String,
    #[serde(default)]
    pub location: Option<String>,
    /// `gnss-interference` when the E) item names GNSS/GPS jamming, interference or
    /// unreliability; `other` otherwise.
    pub kind: String,
    pub start_utc: String,
    #[serde(default)]
    pub end_utc: Option<String>,
    pub end_estimated: bool,
    pub permanent: bool,
    #[serde(default)]
    pub schedule: Option<String>,
    /// `[start, end]` UTC.
    pub active_windows: Vec<[String; 2]>,
    #[serde(default)]
    pub lower_ft: Option<f64>,
    #[serde(default)]
    pub upper_ft: Option<f64>,
    pub geometry: Geometry,
    pub text: String,
    pub notes: Vec<String>,
}

/// The converter's document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotamDoc {
    pub format: String,
    pub harmonised_criteria: String,
    pub notams: Vec<NotamGeometry>,
    pub refused: Vec<String>,
}

fn items(block: &str) -> (Option<String>, BTreeMap<char, String>) {
    let first = block.lines().next().unwrap_or("");
    let id = first
        .contains("NOTAM")
        .then(|| first.split_whitespace().next().map(String::from))
        .flatten();
    let flat = block.replace('\n', " ");
    let mut marks: Vec<(usize, char)> = vec![];
    for c in ['Q', 'A', 'B', 'C', 'D', 'E', 'F', 'G'] {
        let pat = format!("{c})");
        let mut from = 0;
        while let Some(p) = flat[from..].find(&pat) {
            let at = from + p;
            if at == 0 || flat.as_bytes()[at - 1] == b' ' {
                marks.push((at, c));
                break;
            }
            from = at + pat.len();
        }
    }
    marks.sort();
    let mut m = BTreeMap::new();
    for (i, (p, c)) in marks.iter().enumerate() {
        let end = marks.get(i + 1).map_or(flat.len(), |x| x.0);
        m.insert(*c, flat[p + 2..end].trim().to_string());
    }
    (id, m)
}

/// `YYMMDDhhmm` to Unix seconds.
pub fn notam_time(s: &str) -> Option<f64> {
    let d: String = s
        .split_whitespace()
        .next()?
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if d.len() != 10 {
        return None;
    }
    let y: i64 = 2000 + d[0..2].parse::<i64>().ok()?;
    let date = text::parse_date(&format!("{y:04}-{}-{}", &d[2..4], &d[4..6]))?;
    let sod = text::parse_time_of_day(&d[6..10])?;
    Some(text::at(date, sod))
}

/// Compact ICAO coordinate pairs in text, in order: `6930N 02900E`, `6930N02900E`,
/// `693000N0290000E`.
pub fn coordinate_pairs(s: &str) -> Vec<[f64; 2]> {
    let u = s.to_ascii_uppercase();
    let b = u.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    let digits = |from: usize| {
        let mut k = from;
        while k < b.len() && (b[k].is_ascii_digit() || b[k] == b'.') {
            k += 1;
        }
        k
    };
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let e1 = digits(i);
            if e1 < b.len() && (b[e1] == b'N' || b[e1] == b'S') && (4..=9).contains(&(e1 - i)) {
                let mut j = e1 + 1;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                let e2 = digits(j);
                if e2 > j
                    && e2 < b.len()
                    && (b[e2] == b'E' || b[e2] == b'W')
                    && (5..=10).contains(&(e2 - j))
                {
                    let txt = format!(
                        "{}{} {}{}",
                        &u[i..e1],
                        b[e1] as char,
                        &u[j..e2],
                        b[e2] as char
                    );
                    if let Some((la, lo)) = text::parse_coords(&txt) {
                        out.push([la, lo]);
                    }
                    i = e2 + 1;
                    continue;
                }
            }
            i = e1.max(i + 1);
            continue;
        }
        i += 1;
    }
    out
}

/// A radius in the text (`30NM`, `30 NM`, `55KM`, `RADIUS 30NM`) in km.
fn radius_km(s: &str) -> Option<f64> {
    let u = s.to_ascii_uppercase();
    if !u.contains("RADIUS") && !u.contains("CIRCLE") {
        return None;
    }
    let w: Vec<&str> = u.split_whitespace().collect();
    for (k, t) in w.iter().enumerate() {
        let (num, unit) = match t.find(|c: char| c.is_ascii_alphabetic()) {
            Some(p) => (&t[..p], &t[p..]),
            None => (*t, w.get(k + 1).copied().unwrap_or("")),
        };
        if let Ok(v) = num.parse::<f64>() {
            let unit = unit.trim_end_matches(['.', ',']);
            match unit {
                "NM" => return Some(v * 1.852),
                "KM" => return Some(v),
                _ => {}
            }
        }
    }
    None
}

/// A ring of `n` points around a circle.
pub fn circle_ring(lat: f64, lon: f64, r_km: f64, n: usize) -> Vec<[f64; 2]> {
    let dlat = r_km / 111.195;
    let k = lat.to_radians().cos().max(1e-6);
    (0..n)
        .map(|i| {
            let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
            [
                sig9(round(lat + dlat * a.cos(), 5)),
                sig9(round(lon + dlat / k * a.sin(), 5)),
            ]
        })
        .collect()
}

/// Daily windows from a schedule text, between `start` and `end`; `None` if not read.
fn windows(sched: &str, start: f64, end: f64) -> Option<Vec<(f64, f64)>> {
    let u = sched.to_ascii_uppercase();
    if u.contains("H24") {
        return Some(vec![(start, end)]);
    }
    let parts: Vec<&str> = u.split_whitespace().collect();
    let mut spans = vec![];
    for p in &parts {
        if let Some((a, b)) = p.split_once('-') {
            if a.len() == 4 && b.len() == 4 {
                spans.push((text::parse_time_of_day(a)?, text::parse_time_of_day(b)?));
            }
        }
    }
    let other = parts
        .iter()
        .any(|p| !p.contains('-') && *p != "DAILY" && *p != "AND" && *p != ",");
    if spans.is_empty() || other {
        return None;
    }
    let mut out = vec![];
    let mut day = (start / 86_400.0).floor() * 86_400.0;
    while day < end {
        for (a, b) in &spans {
            let (s, e) = (day + a, if b > a { day + b } else { day + b + 86_400.0 });
            let (s, e) = (s.max(start), e.min(end));
            if e > s {
                out.push((s, e));
            }
        }
        day += 86_400.0;
    }
    Some(out)
}

fn classify(e: &str) -> &'static str {
    let u = e.to_ascii_uppercase();
    let gnss = ["GNSS", "GPS", "GALILEO", "GLONASS", "BEIDOU", "RNAV", "RNP"]
        .iter()
        .any(|w| u.contains(w));
    let effect = [
        "JAM",
        "INTERFERENCE",
        "UNRELIABLE",
        "UNAVBL",
        "OUTAGE",
        "RFI",
        "SPOOF",
    ]
    .iter()
    .any(|w| u.contains(w));
    if gnss && effect {
        "gnss-interference"
    } else {
        "other"
    }
}

/// Read one NOTAM block.
pub fn parse_notam(block: &str, fallback_id: &str) -> Result<NotamGeometry, String> {
    let (id, it) = items(block);
    let id = id.unwrap_or_else(|| fallback_id.to_string());
    let e = it.get(&'E').cloned().unwrap_or_default();
    let mut notes = vec![];
    let start = it
        .get(&'B')
        .and_then(|b| notam_time(b))
        .ok_or(format!("{id}: no readable B) start time"))?;
    let c = it.get(&'C').cloned().unwrap_or_default();
    let permanent = c.contains("PERM");
    let end_estimated = c.contains("EST");
    let end = if permanent { None } else { notam_time(&c) };
    if permanent {
        notes.push("PERM: no end; the active period is open".into());
    } else if end.is_none() {
        return Err(format!("{id}: no readable C) end time"));
    }
    if end_estimated {
        notes.push("EST: the end is estimated and may be revised".into());
    }
    if let Some(e2) = end {
        if e2 <= start {
            return Err(format!("{id}: C) is not after B)"));
        }
    }
    let q = it.get(&'Q').cloned().unwrap_or_default();
    let qf: Vec<&str> = q.split('/').map(str::trim).collect();
    let pts = coordinate_pairs(&e);
    let geometry = if pts.len() >= 3 {
        let mut p = pts.clone();
        if p.len() > 3 && p.first() == p.last() {
            p.pop();
        }
        Geometry {
            kind: "polygon".into(),
            polygon: p,
            center: None,
            radius_km: None,
            source: "E) item".into(),
        }
    } else if let (1, Some(rk)) = (pts.len(), radius_km(&e)) {
        Geometry {
            kind: "circle".into(),
            polygon: circle_ring(pts[0][0], pts[0][1], rk, 32),
            center: Some(pts[0]),
            radius_km: Some(sig9(round(rk, 3))),
            source: "E) item".into(),
        }
    } else if let Some((la, lo, rm)) = qf.last().and_then(|p| text::parse_q_position(p)) {
        notes.push("geometry from the Q) line circle (whole-NM radius around a rounded centre): coarser than an E) item area".into());
        Geometry {
            kind: "circle".into(),
            polygon: circle_ring(la, lo, rm / 1000.0, 32),
            center: Some([la, lo]),
            radius_km: Some(sig9(round(rm / 1000.0, 3))),
            source: "Q) line".into(),
        }
    } else {
        return Err(format!("{id}: no area in the E) item or the Q) line"));
    };
    let (lower_ft, upper_ft) = match (it.get(&'F'), it.get(&'G')) {
        (Some(f), Some(g)) => (
            text::parse_altitude(f).and_then(|x| x.0).or(
                if f.to_ascii_uppercase().contains("SFC") || f.to_ascii_uppercase().contains("GND")
                {
                    Some(0.0)
                } else {
                    None
                },
            ),
            text::parse_altitude(g).and_then(|x| x.1.or(x.0)),
        ),
        _ => {
            let fl = |s: Option<&&str>| s.and_then(|v| v.parse::<f64>().ok()).map(|v| v * 100.0);
            (fl(qf.get(5)), fl(qf.get(6)))
        }
    };
    let schedule = it.get(&'D').cloned();
    let span_end = end.unwrap_or(start + 30.0 * 86_400.0);
    if permanent {
        notes.push("active windows of a PERM NOTAM are listed for 30 days from B)".into());
    }
    let wins = match &schedule {
        Some(s) => match windows(s, start, span_end) {
            Some(w) => w,
            None => {
                notes.push("schedule not in a form read here: the whole period is treated as active; read the D) item".into());
                vec![(start, span_end)]
            }
        },
        None => vec![(start, span_end)],
    };
    Ok(NotamGeometry {
        id,
        location: it.get(&'A').cloned(),
        kind: classify(&e).into(),
        start_utc: iso(start),
        end_utc: end.map(iso),
        end_estimated,
        permanent,
        schedule,
        active_windows: wins.iter().map(|(a, b)| [iso(*a), iso(*b)]).collect(),
        lower_ft,
        upper_ft,
        geometry,
        text: e,
        notes,
    })
}

/// Split text into NOTAM blocks (blank line, or a line starting `<id> NOTAM`) and read each.
pub fn parse_notams(content: &str) -> NotamDoc {
    let mut blocks: Vec<String> = vec![];
    let mut cur = String::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        let new =
            t.split_whitespace().next().is_some_and(|w| w.contains('/')) && t.contains("NOTAM");
        if (t.is_empty() || new) && !cur.trim().is_empty() {
            blocks.push(std::mem::take(&mut cur));
        }
        if !t.is_empty() {
            cur.push_str(t);
            cur.push('\n');
        }
    }
    if !cur.trim().is_empty() {
        blocks.push(cur);
    }
    let (mut ok, mut bad) = (vec![], vec![]);
    for (k, b) in blocks.iter().enumerate() {
        if !(b.contains("Q)") || b.contains("E)")) {
            continue;
        }
        match parse_notam(b, &format!("notam-{}", k + 1)) {
            Ok(n) => ok.push(n),
            Err(e) => bad.push(e),
        }
    }
    NotamDoc {
        format: NOTAM_FORMAT.into(),
        harmonised_criteria: HARMONISED_CRITERIA.into(),
        notams: ok,
        refused: bad,
    }
}

/// GeoJSON of the NOTAM areas.
pub fn notams_geojson(d: &NotamDoc) -> serde_json::Value {
    let f: Vec<serde_json::Value> = d
        .notams
        .iter()
        .map(|n| {
            let mut ring: Vec<[f64; 2]> = n.geometry.polygon.iter().map(|p| [p[1], p[0]]).collect();
            if let Some(f) = ring.first().copied() {
                ring.push(f);
            }
            serde_json::json!({
                "type": "Feature",
                "properties": {
                    "id": n.id, "kind": n.kind, "start_utc": n.start_utc, "end_utc": n.end_utc,
                    "end_estimated": n.end_estimated, "permanent": n.permanent,
                    "lower_ft": n.lower_ft, "upper_ft": n.upper_ft, "geometry_source": n.geometry.source,
                    "active_windows": n.active_windows,
                },
                "geometry": {"type": "Polygon", "coordinates": [ring]}
            })
        })
        .collect();
    serde_json::json!({"type": "FeatureCollection", "kshana_pro_notam_geometry": {"format": NOTAM_FORMAT, "harmonised_criteria": HARMONISED_CRITERIA}, "features": f})
}

/// A declared test area given directly (not as NOTAM text).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declared {
    pub id: String,
    /// `[lat, lon]` vertices.
    pub polygon: Vec<[f64; 2]>,
    pub start_utc: String,
    pub end_utc: String,
    #[serde(default)]
    pub lower_ft: Option<f64>,
    #[serde(default)]
    pub upper_ft: Option<f64>,
}

/// The footprint request (`spillover.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FootprintRequest {
    pub title: String,
    #[serde(default)]
    pub notams: Vec<String>,
    #[serde(default)]
    pub declared: Vec<Declared>,
    /// Assumed source antenna height range (m above the surface): inner and outer ring.
    #[serde(default = "heights")]
    pub source_height_m: [f64; 2],
    /// A stated cap on range (km): the source power is unknown.
    #[serde(default = "cap")]
    pub max_range_km: f64,
    #[serde(default = "alts")]
    pub receiver_altitudes_ft: Vec<f64>,
    #[serde(default = "half")]
    pub cell_deg: f64,
    #[serde(default)]
    pub tracks_adsb: Vec<String>,
    #[serde(default)]
    pub tracks_ais: Vec<String>,
    #[serde(default)]
    pub layers: Vec<String>,
    #[serde(default = "cruise")]
    pub agreement_altitude_ft: f64,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn heights() -> [f64; 2] {
    [2.0, 30.0]
}
fn cap() -> f64 {
    400.0
}
fn alts() -> Vec<f64> {
    vec![0.0, 500.0, 2000.0, 10_000.0, 30_000.0]
}
fn half() -> f64 {
    0.5
}
fn cruise() -> f64 {
    30_000.0
}

impl FootprintRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// Radio horizon (km) between heights in metres, 4/3 Earth.
pub fn horizon_km(h1_m: f64, h2_m: f64) -> f64 {
    4.12 * (h1_m.max(0.0).sqrt() + h2_m.max(0.0).sqrt())
}

/// Distance (m) from a point to a polygon (0 inside).
pub fn dist_to_area(poly: &[[f64; 2]], lat: f64, lon: f64) -> f64 {
    if point_in_polygon(poly, lat, lon) {
        return 0.0;
    }
    (0..poly.len())
        .map(|i| point_segment(lat, lon, poly[i], poly[(i + 1) % poly.len()]).0)
        .fold(f64::INFINITY, f64::min)
}

/// Inner and outer ranges (km) at a receiver altitude.
pub fn ranges_km(r: &FootprintRequest, alt_ft: f64) -> (f64, f64) {
    let h = alt_ft.max(0.0) * 0.3048;
    let (lo, hi) = (
        r.source_height_m[0].min(r.source_height_m[1]),
        r.source_height_m[0].max(r.source_height_m[1]),
    );
    (
        horizon_km(lo, h).min(r.max_range_km),
        horizon_km(hi, h).min(r.max_range_km),
    )
}

/// The ring of a cell (`inner`, `outer`) at an altitude, or `None`.
fn ring_of(
    poly: &[[f64; 2]],
    grid: &Grid,
    c: CellId,
    inner: f64,
    outer: f64,
) -> Option<&'static str> {
    let (la, lo) = grid.center(c);
    let d = dist_to_area(poly, la, lo) / 1000.0;
    if d <= inner {
        Some("inner")
    } else if d <= outer {
        Some("outer")
    } else {
        None
    }
}

/// The footprint cells of an area at one altitude.
pub fn footprint_cells(
    poly: &[[f64; 2]],
    grid: &Grid,
    inner_km: f64,
    outer_km: f64,
) -> BTreeMap<CellId, &'static str> {
    let (mut s, mut w, mut n, mut e) = (90.0_f64, 180.0_f64, -90.0_f64, -180.0_f64);
    for p in poly {
        s = s.min(p[0]);
        n = n.max(p[0]);
        w = w.min(p[1]);
        e = e.max(p[1]);
    }
    let dlat = outer_km / 111.0 + grid.cell_deg;
    let k = ((s.abs().max(n.abs()) + dlat).min(89.0)).to_radians().cos();
    let dlon = outer_km / (111.0 * k) + grid.cell_deg;
    let (a, b) = (
        grid.cell_of((s - dlat).max(-90.0), (w - dlon).max(-180.0)),
        grid.cell_of((n + dlat).min(90.0), (e + dlon).min(179.999)),
    );
    let mut out = BTreeMap::new();
    for i in a.i..=b.i {
        for j in a.j..=b.j {
            let c = CellId { i, j };
            if let Some(r) = ring_of(poly, grid, c, inner_km, outer_km) {
                out.insert(c, r);
            }
        }
    }
    out
}

/// One altitude's footprint.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AltitudeFootprint {
    pub altitude_ft: f64,
    pub inner_range_km: f64,
    pub outer_range_km: f64,
    pub inner_cells: usize,
    pub outer_cells: usize,
}

/// One affected user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AffectedUser {
    pub id: String,
    /// `aircraft` or `vessel`.
    pub platform: String,
    pub first_utc: String,
    pub last_utc: String,
    pub minutes: f64,
    /// `inner` when any report lay in the inner ring, else `outer`.
    pub ring: String,
}

/// One layer day's agreement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Agreement {
    pub date: String,
    pub source_kind: String,
    pub degraded_inside: usize,
    pub degraded_outside: usize,
    pub footprint_observed_not_degraded: usize,
    pub footprint_not_observed: usize,
    /// degraded_inside / (inside + outside).
    #[serde(default)]
    pub share_of_degraded_explained: Option<f64>,
    /// degraded_inside / observed footprint cells.
    #[serde(default)]
    pub share_of_footprint_degraded: Option<f64>,
}

/// One area's footprint result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaFootprint {
    pub id: String,
    pub kind: String,
    pub active_windows: Vec<[String; 2]>,
    pub declared_polygon: Vec<[f64; 2]>,
    pub altitudes: Vec<AltitudeFootprint>,
    pub affected_users: Vec<AffectedUser>,
    pub agreement: Vec<Agreement>,
}

/// The footprint document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FootprintDoc {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub method: String,
    pub title: String,
    pub assumptions: Vec<String>,
    pub cell_deg: f64,
    pub areas: Vec<AreaFootprint>,
    pub inputs: Vec<InputRef>,
    pub limits: Vec<String>,
}

fn active(wins: &[(f64, f64)], t: f64) -> bool {
    wins.iter().any(|(a, b)| t >= *a && t <= *b)
}

/// An area to model: id, kind, polygon, active windows.
type Subject = (String, String, Vec<[f64; 2]>, Vec<(f64, f64)>);

/// Build the footprints. Returns the document and its GeoJSON.
pub fn footprint(r: &FootprintRequest) -> Result<(FootprintDoc, serde_json::Value), String> {
    if r.cell_deg < super::MIN_CELL_DEG - 1e-9 {
        return Err(format!(
            "cell_deg must be at least {} degrees",
            super::MIN_CELL_DEG
        ));
    }
    if r.source_height_m.iter().any(|h| *h < 0.0) || r.max_range_km <= 0.0 {
        return Err("source heights must not be negative and max_range_km positive".into());
    }
    if r.receiver_altitudes_ft.is_empty() {
        return Err("give at least one receiver altitude".into());
    }
    let grid = Grid::new(r.cell_deg).ok_or("invalid cell_deg")?;
    let mut inputs = vec![];
    // (id, kind, polygon, windows)
    let mut subjects: Vec<Subject> = vec![];
    for n in &r.notams {
        let (b, rf) = super::read_input(&r.base_dir, n, "notam")?;
        inputs.push(rf);
        let d = parse_notams(&String::from_utf8_lossy(&b));
        if !d.refused.is_empty() {
            return Err(format!("{n}: {}", d.refused.join("; ")));
        }
        for x in d.notams {
            let w = x
                .active_windows
                .iter()
                .filter_map(|w| {
                    Some((
                        super::utc("window", &w[0]).ok()?,
                        super::utc("window", &w[1]).ok()?,
                    ))
                })
                .collect();
            subjects.push((x.id, x.kind, x.geometry.polygon, w));
        }
    }
    for d in &r.declared {
        if d.polygon.len() < 3 {
            return Err(format!("declared {}: a polygon needs three vertices", d.id));
        }
        let (a, b) = (
            super::utc("start_utc", &d.start_utc)?,
            super::utc("end_utc", &d.end_utc)?,
        );
        subjects.push((
            d.id.clone(),
            "declared".into(),
            d.polygon.clone(),
            vec![(a, b)],
        ));
    }
    if subjects.is_empty() {
        return Err("give at least one NOTAM file or declared area".into());
    }
    let mut tracks: Vec<(&str, BTreeMap<String, Vec<super::transit::Fix>>)> = vec![];
    if !r.tracks_adsb.is_empty() {
        tracks.push((
            "aircraft",
            super::transit::load_tracks(&r.base_dir, &r.tracks_adsb, "aircraft_id", &mut inputs)?,
        ));
    }
    if !r.tracks_ais.is_empty() {
        tracks.push((
            "vessel",
            super::transit::load_tracks(&r.base_dir, &r.tracks_ais, "vessel_id", &mut inputs)?,
        ));
    }
    let layers: Vec<Layer> = super::layers::load_layers(&r.base_dir, &r.layers, &mut inputs)?;
    let mut alts = r.receiver_altitudes_ft.clone();
    alts.sort_by(f64::total_cmp);
    alts.dedup();
    let mut out = vec![];
    let mut features = vec![];
    for (id, kind, poly, wins) in &subjects {
        let mut alt_rows = vec![];
        for a in &alts {
            let (ri, ro) = ranges_km(r, *a);
            let cells = footprint_cells(poly, &grid, ri, ro);
            let inner = cells.values().filter(|x| **x == "inner").count();
            for (c, ring) in &cells {
                let (s, w, n, e) = grid.bounds(*c);
                features.push(serde_json::json!({
                    "type": "Feature",
                    "properties": {"area_id": id, "altitude_ft": a, "ring": ring, "cell_i": c.i, "cell_j": c.j},
                    "geometry": {"type": "Polygon", "coordinates": [[[w, s], [e, s], [e, n], [w, n], [w, s]]]}
                }));
            }
            alt_rows.push(AltitudeFootprint {
                altitude_ft: *a,
                inner_range_km: sig9(round(ri, 2)),
                outer_range_km: sig9(round(ro, 2)),
                inner_cells: inner,
                outer_cells: cells.len() - inner,
            });
        }
        // Affected users.
        let mut users = vec![];
        for (platform, by) in &tracks {
            for (uid, fixes) in by {
                let (mut first, mut last, mut secs, mut ring): (Option<f64>, f64, f64, &str) =
                    (None, 0.0, 0.0, "outer");
                for (k, f) in fixes.iter().enumerate() {
                    if !active(wins, f.t) {
                        continue;
                    }
                    let alt = if *platform == "vessel" {
                        0.0
                    } else {
                        f.alt_ft.unwrap_or(0.0)
                    };
                    let (ri, ro) = ranges_km(r, alt);
                    if let Some(rg) = ring_of(poly, &grid, grid.cell_of(f.lat, f.lon), ri, ro) {
                        let hold = fixes
                            .get(k + 1)
                            .map(|n| (n.t - f.t).clamp(0.0, super::transit::MAX_HOLD_S))
                            .unwrap_or(0.0);
                        secs += hold;
                        first.get_or_insert(f.t);
                        last = f.t;
                        if rg == "inner" {
                            ring = "inner";
                        }
                    }
                }
                if let Some(fs) = first {
                    users.push(AffectedUser {
                        id: uid.clone(),
                        platform: (*platform).into(),
                        first_utc: iso(fs),
                        last_utc: iso(last),
                        minutes: sig9(round(secs / 60.0, 2)),
                        ring: ring.into(),
                    });
                }
            }
        }
        users.sort_by(|a, b| a.first_utc.cmp(&b.first_utc).then(a.id.cmp(&b.id)));
        // Agreement against the layers of the active days.
        let (ri, ro) = ranges_km(r, r.agreement_altitude_ft);
        let mut agreement = vec![];
        for l in &layers {
            let day = (l.day_start, l.day_start + 86_400.0);
            if !wins.iter().any(|(a, b)| *a < day.1 && *b >= day.0) {
                continue;
            }
            let lg = l.grid();
            let fp = footprint_cells(poly, &lg, ri, ro);
            let wide = footprint_cells(poly, &lg, 2.0 * ro, 2.0 * ro);
            let deg: BTreeSet<CellId> = l
                .degraded()
                .into_iter()
                .filter(|c| wide.contains_key(c))
                .collect();
            let inside = deg.iter().filter(|c| fp.contains_key(c)).count();
            let outside = deg.len() - inside;
            let observed_fp = fp
                .keys()
                .filter(|c| {
                    matches!(
                        l.cells.get(c),
                        Some(CellState::Degraded | CellState::NotDegraded)
                    )
                })
                .count();
            let not_obs = fp.keys().filter(|c| !l.cells.contains_key(c)).count();
            agreement.push(Agreement {
                date: l.date.clone(),
                source_kind: l.source_kind.clone(),
                degraded_inside: inside,
                degraded_outside: outside,
                footprint_observed_not_degraded: observed_fp - inside,
                footprint_not_observed: not_obs,
                share_of_degraded_explained: (!deg.is_empty())
                    .then(|| sig9(round(inside as f64 / deg.len() as f64, 4))),
                share_of_footprint_degraded: (observed_fp > 0)
                    .then(|| sig9(round(inside as f64 / observed_fp as f64, 4))),
            });
        }
        out.push(AreaFootprint {
            id: id.clone(),
            kind: kind.clone(),
            active_windows: wins.iter().map(|(a, b)| [iso(*a), iso(*b)]).collect(),
            declared_polygon: poly.clone(),
            altitudes: alt_rows,
            affected_users: users,
            agreement,
        });
    }
    let doc = FootprintDoc {
        format: FOOTPRINT_FORMAT.into(),
        evidence: super::EVIDENCE.into(),
        statement: "A modelled line-of-sight footprint around a declared area, for planning, NOTAM drafting and post-event review. Not an operational instruction, not a measurement and not a location of any emitter.".into(),
        method: "Radio horizon 4.12 x (sqrt(h_source) + sqrt(h_receiver)) km (4/3 Earth), h in metres; the source may be anywhere in the declared area. Inner ring: the lower source height; outer ring: the higher; both capped at max_range_km. A cell is in the footprint when its centre is within range of the area. Affected users: reports in footprint cells, for their altitude, during active windows. Agreement: footprint at the agreement altitude against degraded layer cells within twice the outer range.".into(),
        title: r.title.clone(),
        assumptions: vec![
            format!("Source antenna height {:.0} to {:.0} m above the surface.", r.source_height_m[0].min(r.source_height_m[1]), r.source_height_m[0].max(r.source_height_m[1])),
            format!("Range capped at {:.0} km (the source power is not known).", r.max_range_km),
            "Smooth Earth: terrain shielding is not modelled, so the footprint can only be overstated by it.".into(),
            format!("Agreement is assessed at {:.0} ft.", r.agreement_altitude_ft),
        ],
        cell_deg: r.cell_deg,
        areas: out,
        inputs,
        limits: vec![
            "The footprint models where line of sight allows reception, not where a receiver is affected: that depends on power, antenna, receiver and signal.".into(),
            "Agreement with ADS-B degradation layers describes overlap of cells; other sources in the region also degrade cells.".into(),
            format!("Outputs are cells of at least {} degrees and the declared area; nothing here estimates an emitter position.", super::MIN_CELL_DEG),
        ],
    };
    let geo = serde_json::json!({
        "type": "FeatureCollection",
        "kshana_pro_spillover_footprint": {"format": FOOTPRINT_FORMAT, "title": r.title, "evidence": super::EVIDENCE, "cell_deg": r.cell_deg},
        "features": features
    });
    Ok((doc, geo))
}

/// Write `footprint.json`, `footprint.geojson`, `affected-users.csv` and the request.
pub fn write_dir(
    out: &Path,
    d: &FootprintDoc,
    geo: &serde_json::Value,
    request_src: &str,
) -> Result<(), String> {
    super::write_json(out, FOOTPRINT_FILE, d)?;
    super::write_json(out, "footprint.geojson", geo)?;
    let mut csv = String::from("area_id,id,platform,first_utc,last_utc,minutes,ring\n");
    for a in &d.areas {
        for u in &a.affected_users {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                a.id, u.id, u.platform, u.first_utc, u.last_utc, u.minutes, u.ring
            ));
        }
    }
    super::write_text(out, "affected-users.csv", csv.as_bytes())?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_and_radius_are_read() {
        let p = coordinate_pairs("AREA 6930N 02900E - 7010N02900E - 701000N0310000E.");
        assert_eq!(p.len(), 3);
        assert!((p[0][0] - 69.5).abs() < 1e-9 && (p[0][1] - 29.0).abs() < 1e-9);
        assert!((p[2][0] - 70.166_666_666).abs() < 1e-6);
        assert_eq!(
            radius_km("WI RADIUS 30NM CENTRED ON 6930N03000E"),
            Some(30.0 * 1.852)
        );
        assert_eq!(radius_km("CIRCLE 55 KM"), Some(55.0));
        assert!((horizon_km(2.0, 0.0) - 4.12 * 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn schedules_expand_to_daily_windows() {
        let s = notam_time("2606010000").unwrap();
        let e = notam_time("2606030000").unwrap();
        let w = windows("DAILY 0800-1600", s, e).unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].1 - w[0].0, 8.0 * 3600.0);
        assert!(windows("MON WED 0800-1600", s, e).is_none());
        assert_eq!(windows("H24", s, e).unwrap(), vec![(s, e)]);
    }
}
