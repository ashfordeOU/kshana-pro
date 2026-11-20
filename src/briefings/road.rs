// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Road and logistics corridor exposure index and heat map (B0904)
//!
//! From telematics jammer-detection events (CSV `timestamp,device_id,lat,lon[,duration_s]`,
//! one row per detection by a tracker or telematics unit), named corridors (polylines with a
//! half-width) are cut into segments of `segment_km`. Per segment over the period:
//! detections, distinct device-days, the detection rate per 100 km per day, and an exposure
//! index `100 × (1 − exp(−rate / reference_rate))`. A heat map counts detections per cell of
//! at least 0.5°.
//!
//! **Privacy rule (research rule V22: no published output identifies an individual jammer
//! user or vehicle).** Device identifiers are used only in memory to count distinct devices and
//! never written. A segment or cell is published only when at least `min_distinct` (at least
//! [`kshana::interference_map::PUBLICATION_MIN_DISTINCT`]) distinct devices reported in it;
//! otherwise it is `withheld`, with no counts. Times are kept to the date and the hour of day,
//! positions to the segment or cell. A pattern that suggests an individual belongs with the
//! competent authority, not in a published index; the output says so.
//!
//! **Reproducible.** No generation time and no salted hashes are written; inputs are ordered as
//! named; floats are rounded to 9 significant digits.
//!
//! MODELLED: the index counts the detections supplied; detections are as good as the
//! detectors that made them and say nothing of intent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::{haversine_m, CellId, Grid};
use serde::{Deserialize, Serialize};

use super::{round, sig9, InputRef};

pub const FORMAT: &str = "kshana-pro-road-exposure-index/1";
pub const RESULT_FILE: &str = "road-exposure.json";

/// The privacy rule as every output states it.
pub const PRIVACY_RULE: &str = "No output identifies an individual jammer user or vehicle: \
device identifiers are counted in memory and never written; a segment or cell is published only \
with at least min_distinct distinct devices, otherwise withheld without counts; times are kept \
to the date and hour of day and positions to the segment or cell. Patterns that suggest an \
individual are for the competent authority, not for publication.";

/// A corridor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corridor {
    pub id: String,
    pub name: String,
    /// `[lat, lon]`.
    pub waypoints: Vec<[f64; 2]>,
    #[serde(default = "five")]
    pub half_width_km: f64,
}

fn five() -> f64 {
    5.0
}

/// The request (`road.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoadRequest {
    pub title: String,
    pub detections: Vec<String>,
    pub corridors: Vec<Corridor>,
    pub period_from_utc: String,
    pub period_to_utc: String,
    #[serde(default = "twenty_five")]
    pub segment_km: f64,
    #[serde(default = "half")]
    pub cell_deg: f64,
    #[serde(default = "min_k")]
    pub min_distinct: usize,
    /// Distinct device-days per 100 km per day that gives an index of 63.
    #[serde(default = "one")]
    pub reference_rate: f64,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn twenty_five() -> f64 {
    25.0
}
fn half() -> f64 {
    0.5
}
fn min_k() -> usize {
    kshana::interference_map::PUBLICATION_MIN_DISTINCT
}
fn one() -> f64 {
    1.0
}

impl RoadRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// One segment's line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentIndex {
    pub corridor_id: String,
    pub segment: usize,
    pub from_km: f64,
    pub to_km: f64,
    /// `[lat, lon]` of the segment's start and end.
    pub start: [f64; 2],
    pub end: [f64; 2],
    /// `published` or `withheld`.
    pub status: String,
    #[serde(default)]
    pub detections: Option<usize>,
    #[serde(default)]
    pub device_days: Option<usize>,
    #[serde(default)]
    pub days_with_detections: Option<usize>,
    #[serde(default)]
    pub rate_per_100km_day: Option<f64>,
    #[serde(default)]
    pub index: Option<f64>,
    /// Detections by hour of day (UTC), 24 values.
    #[serde(default)]
    pub by_hour_utc: Option<Vec<usize>>,
}

/// One corridor's summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CorridorIndex {
    pub corridor_id: String,
    pub name: String,
    pub length_km: f64,
    pub segments_published: usize,
    pub segments_withheld: usize,
    /// Length-weighted mean index over published segments.
    #[serde(default)]
    pub index: Option<f64>,
    #[serde(default)]
    pub peak_segment: Option<usize>,
}

/// The index document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoadIndex {
    pub format: String,
    pub evidence: String,
    pub privacy_rule: String,
    pub method: String,
    pub title: String,
    pub period_from_utc: String,
    pub period_to_utc: String,
    pub min_distinct: usize,
    pub detections_read: usize,
    pub detections_refused: usize,
    pub detections_in_corridors: usize,
    pub corridors: Vec<CorridorIndex>,
    pub segments: Vec<SegmentIndex>,
    pub inputs: Vec<InputRef>,
    pub limits: Vec<String>,
}

struct Det {
    t: f64,
    dev: String,
    lat: f64,
    lon: f64,
}

fn parse_detections(text: &str, name: &str) -> Result<(Vec<Det>, usize), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or(format!("{name}: empty CSV"))?
        .split(',')
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| {
        head.iter()
            .position(|c| c == n)
            .ok_or(format!("{name}: no '{n}' column"))
    };
    let (ct, cd, cla, clo) = (
        col("timestamp")?,
        col("device_id")?,
        col("lat")?,
        col("lon")?,
    );
    let (mut out, mut bad) = (vec![], 0);
    for l in lines {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let g = |k: usize| f.get(k).copied().unwrap_or("");
        match (
            crate::forensics::inputs::parse_utc(g(ct)),
            g(cla).parse::<f64>(),
            g(clo).parse::<f64>(),
        ) {
            (Some(t), Ok(lat), Ok(lon))
                if !g(cd).is_empty()
                    && (-90.0..=90.0).contains(&lat)
                    && (-180.0..=180.0).contains(&lon) =>
            {
                out.push(Det {
                    t,
                    dev: g(cd).to_string(),
                    lat,
                    lon,
                })
            }
            _ => bad += 1,
        }
    }
    Ok((out, bad))
}

/// Distance (m) from a point to a segment, in a local equirectangular frame, and the fraction
/// along the segment of the nearest point.
pub fn point_segment(lat: f64, lon: f64, a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let k = (lat.to_radians()).cos();
    let (ax, ay) = (a[1] * k, a[0]);
    let (bx, by) = (b[1] * k, b[0]);
    let (px, py) = (lon * k, lat);
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let f = if l2 > 0.0 {
        (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (qa, qo) = (a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1]));
    (haversine_m(lat, lon, qa, qo), f)
}

/// One segment: `(from_km, to_km, start, end)`.
type Seg = (f64, f64, [f64; 2], [f64; 2]);

/// The segments of a corridor.
fn segments(c: &Corridor, seg_km: f64) -> Vec<Seg> {
    // Points every seg_km along the polyline.
    let mut cum = vec![0.0];
    for w in c.waypoints.windows(2) {
        let d = haversine_m(w[0][0], w[0][1], w[1][0], w[1][1]) / 1000.0;
        cum.push(cum.last().copied().unwrap_or(0.0) + d);
    }
    let total = *cum.last().unwrap_or(&0.0);
    let at = |km: f64| -> [f64; 2] {
        for i in 0..c.waypoints.len().saturating_sub(1) {
            if km <= cum[i + 1] || i + 2 == c.waypoints.len() {
                let span = cum[i + 1] - cum[i];
                let f = if span > 0.0 {
                    ((km - cum[i]) / span).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (a, b) = (c.waypoints[i], c.waypoints[i + 1]);
                return [a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1])];
            }
        }
        c.waypoints[0]
    };
    let n = (total / seg_km).ceil().max(1.0) as usize;
    (0..n)
        .map(|k| {
            let (a, b) = (k as f64 * seg_km, ((k + 1) as f64 * seg_km).min(total));
            (a, b, at(a), at(b))
        })
        .collect()
}

#[derive(Default)]
struct Acc {
    det: usize,
    devday: BTreeSet<(String, i64)>,
    devs: BTreeSet<String>,
    days: BTreeSet<i64>,
    hours: [usize; 24],
}

/// Build the index and the heat map.
pub fn build(r: &RoadRequest) -> Result<(RoadIndex, serde_json::Value), String> {
    let t0 = super::utc("period_from_utc", &r.period_from_utc)?;
    let t1 = super::utc("period_to_utc", &r.period_to_utc)?;
    if t1 <= t0 {
        return Err("the period is empty".into());
    }
    if r.min_distinct < kshana::interference_map::PUBLICATION_MIN_DISTINCT {
        return Err(format!(
            "min_distinct must be at least {} (privacy rule)",
            kshana::interference_map::PUBLICATION_MIN_DISTINCT
        ));
    }
    if r.cell_deg < super::MIN_CELL_DEG - 1e-9 {
        return Err(format!("cell_deg must be at least {}", super::MIN_CELL_DEG));
    }
    if r.segment_km < 1.0 || r.reference_rate <= 0.0 {
        return Err("segment_km must be at least 1 and reference_rate positive".into());
    }
    for c in &r.corridors {
        if c.waypoints.len() < 2 || c.half_width_km <= 0.0 {
            return Err(format!(
                "corridor {}: needs two waypoints and a positive half-width",
                c.id
            ));
        }
    }
    let mut inputs = vec![];
    let (mut dets, mut refused) = (vec![], 0);
    for n in &r.detections {
        let (b, rf) = super::read_input(&r.base_dir, n, "detections")?;
        inputs.push(rf);
        let (d, bad) = parse_detections(&String::from_utf8_lossy(&b), n)?;
        refused += bad;
        dets.extend(d);
    }
    let read = dets.len() + refused;
    dets.retain(|d| d.t >= t0 && d.t < t1);
    let days_n = super::days_between(t0, t1) as f64;
    let grid = Grid::new(r.cell_deg).ok_or("invalid cell_deg")?;
    let segs: Vec<Vec<Seg>> = r
        .corridors
        .iter()
        .map(|c| segments(c, r.segment_km))
        .collect();
    let mut acc: BTreeMap<(usize, usize), Acc> = BTreeMap::new();
    let mut cells: BTreeMap<CellId, Acc> = BTreeMap::new();
    let mut in_corr = 0;
    for d in &dets {
        let day = (d.t / 86_400.0).floor() as i64;
        let hour = ((d.t - day as f64 * 86_400.0) / 3600.0).floor() as usize % 24;
        let add = |a: &mut Acc| {
            a.det += 1;
            a.devday.insert((d.dev.clone(), day));
            a.devs.insert(d.dev.clone());
            a.days.insert(day);
            a.hours[hour] += 1;
        };
        add(cells.entry(grid.cell_of(d.lat, d.lon)).or_default());
        // The nearest segment of each corridor within the half-width.
        let mut hit = false;
        for (ci, c) in r.corridors.iter().enumerate() {
            let best = segs[ci]
                .iter()
                .enumerate()
                .map(|(si, s)| (si, point_segment(d.lat, d.lon, s.2, s.3).0))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((si, m)) = best {
                if m <= c.half_width_km * 1000.0 {
                    add(acc.entry((ci, si)).or_default());
                    hit = true;
                }
            }
        }
        if hit {
            in_corr += 1;
        }
    }
    let idx = |rate: f64| 100.0 * (1.0 - (-rate / r.reference_rate).exp());
    let mut segments_out = vec![];
    let mut corridors_out = vec![];
    for (ci, c) in r.corridors.iter().enumerate() {
        let (mut pubn, mut wh, mut wsum, mut lsum) = (0, 0, 0.0, 0.0);
        let mut peak: Option<(usize, f64)> = None;
        for (si, s) in segs[ci].iter().enumerate() {
            let len = (s.1 - s.0).max(1e-9);
            let a = acc.get(&(ci, si));
            let ok = a.is_some_and(|a| a.devs.len() >= r.min_distinct);
            let mut row = SegmentIndex {
                corridor_id: c.id.clone(),
                segment: si + 1,
                from_km: sig9(round(s.0, 3)),
                to_km: sig9(round(s.1, 3)),
                start: [sig9(round(s.2[0], 4)), sig9(round(s.2[1], 4))],
                end: [sig9(round(s.3[0], 4)), sig9(round(s.3[1], 4))],
                status: "withheld".into(),
                detections: None,
                device_days: None,
                days_with_detections: None,
                rate_per_100km_day: None,
                index: None,
                by_hour_utc: None,
            };
            match (ok, a) {
                (true, Some(a)) => {
                    let rate = a.devday.len() as f64 / (len / 100.0) / days_n;
                    let ix = idx(rate);
                    row.status = "published".into();
                    row.detections = Some(a.det);
                    row.device_days = Some(a.devday.len());
                    row.days_with_detections = Some(a.days.len());
                    row.rate_per_100km_day = Some(sig9(round(rate, 4)));
                    row.index = Some(sig9(round(ix, 2)));
                    row.by_hour_utc = Some(a.hours.to_vec());
                    pubn += 1;
                    wsum += ix * len;
                    lsum += len;
                    if peak.is_none_or(|p| ix > p.1) {
                        peak = Some((si + 1, ix));
                    }
                }
                _ => wh += 1,
            }
            segments_out.push(row);
        }
        corridors_out.push(CorridorIndex {
            corridor_id: c.id.clone(),
            name: c.name.clone(),
            length_km: sig9(round(segs[ci].last().map(|s| s.1).unwrap_or(0.0), 3)),
            segments_published: pubn,
            segments_withheld: wh,
            index: (lsum > 0.0).then(|| sig9(round(wsum / lsum, 2))),
            peak_segment: peak.map(|p| p.0),
        });
    }
    let mut features = vec![];
    for (c, a) in &cells {
        if a.devs.len() < r.min_distinct {
            continue;
        }
        let (s, w, n, e) = grid.bounds(*c);
        features.push(serde_json::json!({
            "type": "Feature",
            "properties": {
                "cell_i": c.i, "cell_j": c.j, "detections": a.det,
                "device_days": a.devday.len(), "days_with_detections": a.days.len(),
            },
            "geometry": {"type": "Polygon", "coordinates": [[[sig9(w), sig9(s)], [sig9(e), sig9(s)], [sig9(e), sig9(n)], [sig9(w), sig9(n)], [sig9(w), sig9(s)]]]}
        }));
    }
    let heat = serde_json::json!({
        "type": "FeatureCollection",
        "kshana_pro_road_heat_map": {
            "format": "kshana-pro-road-heat-map/1",
            "title": r.title,
            "evidence": super::EVIDENCE,
            "privacy_rule": PRIVACY_RULE,
            "cell_deg": r.cell_deg,
            "min_distinct": r.min_distinct,
            "period_from_utc": r.period_from_utc,
            "period_to_utc": r.period_to_utc,
            "notice": "Aggregate only. Cells with fewer than min_distinct distinct devices are omitted; an omitted cell is not evidence of no jamming.",
        },
        "features": features
    });
    let ix = RoadIndex {
        format: FORMAT.into(),
        evidence: super::EVIDENCE.into(),
        privacy_rule: PRIVACY_RULE.into(),
        method: format!(
            "Corridors cut into {} km segments; a detection counts for the nearest segment of each corridor within its half-width. Rate = distinct device-days per 100 km per day over {} day(s); index = 100 x (1 - exp(-rate / {})). Heat map cells {} degrees.",
            r.segment_km, days_n, r.reference_rate, r.cell_deg
        ),
        title: r.title.clone(),
        period_from_utc: r.period_from_utc.clone(),
        period_to_utc: r.period_to_utc.clone(),
        min_distinct: r.min_distinct,
        detections_read: read,
        detections_refused: refused,
        detections_in_corridors: in_corr,
        corridors: corridors_out,
        segments: segments_out,
        inputs,
        limits: vec![
            "Detections are as good as the detectors that made them; a detection says nothing of intent or of who operated a device.".into(),
            "Withheld segments and cells have too few distinct reporting devices to publish; withheld is not evidence of no jamming.".into(),
            "The rate is not normalised by traffic: busier roads carry more trackers and so report more.".into(),
        ],
    };
    Ok((ix, heat))
}

/// Write `road-exposure.json`, `road-heat-map.geojson`, `segments.csv` and the request.
pub fn write_dir(
    out: &Path,
    ix: &RoadIndex,
    heat: &serde_json::Value,
    request_src: &str,
) -> Result<(), String> {
    super::write_json(out, RESULT_FILE, ix)?;
    super::write_json(out, "road-heat-map.geojson", heat)?;
    let mut csv = String::from("corridor_id,segment,from_km,to_km,status,detections,device_days,rate_per_100km_day,index\n");
    let o = |v: Option<String>| v.unwrap_or_default();
    for s in &ix.segments {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            s.corridor_id,
            s.segment,
            s.from_km,
            s.to_km,
            s.status,
            o(s.detections.map(|x| x.to_string())),
            o(s.device_days.map(|x| x.to_string())),
            o(s.rate_per_100km_day.map(|x| x.to_string())),
            o(s.index.map(|x| x.to_string()))
        ));
    }
    super::write_text(out, "segments.csv", csv.as_bytes())?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}
