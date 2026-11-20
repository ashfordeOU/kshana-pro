// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Maritime AIS integrity evidence (research 2026-11-20 gaps, epic E07, Pro parts)
//!
//! The open engine carries the AIS integrity detectors. This module builds the paid evidence
//! on the user's own decoded AIS positions (and, where supplied, static data and independent
//! sources), each written as a signed evidence directory:
//!
//! * [`consistency`]: cross-source consistency of AIS against independent position sources
//!   the user holds (radar or VTS tracks, LRIT reports, satellite AIS, imagery detections),
//!   each with its declared accuracy: per association a residual against a stated gate,
//!   episodes of inconsistency per vessel, and independent-source targets with no AIS
//!   within the gate.
//! * [`packs`]: three evidence packs over the same track engine:
//!   * **sanctions** (dark activity review): reception gaps, ship-to-ship encounter
//!     candidates, identity changes in static data, implied-speed jumps and simultaneous use
//!     of one MMSI, for vessels the user names. Kshana bundles no sanctions list and makes no
//!     sanctions determination; any screening is the user's.
//!   * **IUU** (fishing review): time inside user-declared zones, time at a declared
//!     fishing-like speed band and with navigational status "engaged in fishing" inside them,
//!     gaps that start or end near a zone, and encounter (transshipment) candidates.
//!   * **subsea asset** (cable and pipeline review): presence, dwell and slow speed inside a
//!     declared corridor, "at anchor" or "moored" status while moving inside it (an
//!     anchor-drag indicator), gaps inside it, and the vessels present around a reported
//!     fault time and position. Presence is not attribution.
//!
//! Track engine ([`Tracks`]): one CSV of decoded AIS position reports
//! (`timestamp,mmsi,lat,lon[,sog_kn,cog_deg,heading_deg,nav_status,source]`), an optional
//! static-data CSV (`timestamp,mmsi[,imo,name,callsign,flag,ship_type]`), reception gaps,
//! implied-speed jumps, identity changes, MMSI collisions and encounters on a time grid.
//!
//! **Evidence label: MODELLED.** Rules are exercised on synthetic scenarios with known truth
//! ([`synth`], `tests/ais_evidence.rs`); thresholds are pre-registered in each request and
//! written into each pack; nothing here has been validated against labelled real cases. A
//! reception gap is "no report received": it can be coverage, reception or a transponder
//! switched off, and the pack says so. Positions are the vessels' own broadcasts or the
//! user's independent sources: no output locates a radio emitter, and nothing transmits.

pub mod consistency;
pub mod packs;
pub mod synth;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};
use crate::transport::{dist_m, en_m, Table};

/// Evidence label written into every report.
pub const EVIDENCE: &str = "MODELLED: rules exercised on synthetic scenarios with known truth; \
thresholds pre-registered in the request; not validated against labelled real cases.";

/// What every pack says about a reception gap.
pub const GAP_NOTE: &str = "A gap is a period with no AIS report received from the vessel. It \
can come from receiver coverage, reception conditions, the data supplier, or a transponder \
switched off or silenced; this pack does not decide which.";

/// What every pack says about itself.
pub const STATEMENT: &str =
    "An evidence pack: an engineering reconstruction of the data supplied, \
under thresholds fixed in the request before the run. REPORTED = AIS broadcasts and \
third-party data as supplied; DERIVED = computed by this method. It is not a legal opinion, a \
finding of fact or a determination of intent, and it locates no radio emitter.";

/// m/s per knot.
pub const KN: f64 = 1852.0 / 3600.0;

/// One decoded AIS position report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    /// Unix seconds (UTC).
    pub t: f64,
    pub mmsi: String,
    pub lat: f64,
    pub lon: f64,
    /// Speed over ground (kn); `None` when absent or "not available" (102.3).
    pub sog_kn: Option<f64>,
    /// Course over ground (deg).
    pub cog_deg: Option<f64>,
    /// Navigational status code as broadcast (0 under way using engine, 1 at anchor,
    /// 5 moored, 7 engaged in fishing, ...); `None` when absent or 15 "not defined".
    pub nav_status: Option<u8>,
    /// Receiving source as labelled by the user (e.g. `terrestrial`, `satellite`).
    pub source: String,
}

/// One static-data record.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StaticRecord {
    pub t: f64,
    pub mmsi: String,
    pub imo: String,
    pub name: String,
    pub callsign: String,
    pub flag: String,
    pub ship_type: String,
}

/// Decoded AIS: reports by MMSI (sorted by time), static records by MMSI, refused rows.
#[derive(Clone, Debug, Default)]
pub struct Tracks {
    pub by_mmsi: BTreeMap<String, Vec<Report>>,
    pub statics: BTreeMap<String, Vec<StaticRecord>>,
    pub rows: usize,
    pub refused: usize,
}

fn time_of(s: &str) -> Option<f64> {
    parse_utc(s).or_else(|| s.parse::<f64>().ok())
}

impl Tracks {
    /// Parse the position CSV (required columns `timestamp,mmsi,lat,lon`).
    pub fn parse(text: &str) -> Result<Tracks, String> {
        let t = Table::parse(text)?;
        let (ct, cm, clat, clon) = (
            t.need("timestamp")?,
            t.need("mmsi")?,
            t.need("lat")?,
            t.need("lon")?,
        );
        let (csog, ccog, cnav, csrc) = (
            t.col("sog_kn"),
            t.col("cog_deg"),
            t.col("nav_status"),
            t.col("source"),
        );
        let mut out = Tracks {
            rows: t.rows.len(),
            ..Tracks::default()
        };
        for i in 0..t.rows.len() {
            let (Some(ts), Ok(lat), Ok(lon)) = (
                time_of(t.text(i, Some(ct))),
                t.text(i, Some(clat)).parse::<f64>(),
                t.text(i, Some(clon)).parse::<f64>(),
            ) else {
                out.refused += 1;
                continue;
            };
            // 91 / 181 are the AIS "not available" position values.
            if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                out.refused += 1;
                continue;
            }
            let mmsi = t.text(i, Some(cm)).to_string();
            if mmsi.is_empty() {
                out.refused += 1;
                continue;
            }
            let sog = t
                .num(i, csog)
                .ok()
                .flatten()
                .filter(|v| (0.0..102.25).contains(v));
            let cog = t
                .num(i, ccog)
                .ok()
                .flatten()
                .filter(|v| (0.0..360.0).contains(v));
            let nav = t
                .text(i, cnav)
                .parse::<u8>()
                .ok()
                .filter(|v| *v != 15 && *v < 16);
            let source = match t.text(i, csrc) {
                "" => "ais".to_string(),
                s => s.to_string(),
            };
            out.by_mmsi.entry(mmsi.clone()).or_default().push(Report {
                t: ts,
                mmsi,
                lat,
                lon,
                sog_kn: sog,
                cog_deg: cog,
                nav_status: nav,
                source,
            });
        }
        for v in out.by_mmsi.values_mut() {
            v.sort_by(|a, b| a.t.total_cmp(&b.t));
        }
        Ok(out)
    }

    /// Add the static-data CSV (required columns `timestamp,mmsi`).
    pub fn add_static(&mut self, text: &str) -> Result<(), String> {
        let t = Table::parse(text)?;
        let (ct, cm) = (t.need("timestamp")?, t.need("mmsi")?);
        let c = |n: &str| t.col(n);
        let (ci, cn, cc, cf, cs) = (
            c("imo"),
            c("name"),
            c("callsign"),
            c("flag"),
            c("ship_type"),
        );
        for i in 0..t.rows.len() {
            let Some(ts) = time_of(t.text(i, Some(ct))) else {
                self.refused += 1;
                continue;
            };
            let mmsi = t.text(i, Some(cm)).to_string();
            let norm = |s: &str| s.trim().to_ascii_uppercase();
            self.statics
                .entry(mmsi.clone())
                .or_default()
                .push(StaticRecord {
                    t: ts,
                    mmsi,
                    imo: norm(t.text(i, ci)),
                    name: norm(t.text(i, cn)),
                    callsign: norm(t.text(i, cc)),
                    flag: norm(t.text(i, cf)),
                    ship_type: norm(t.text(i, cs)),
                });
        }
        for v in self.statics.values_mut() {
            v.sort_by(|a, b| a.t.total_cmp(&b.t));
        }
        Ok(())
    }

    /// The interpolated position of `mmsi` at `t`, from bracketing reports no more than
    /// `max_s` apart (or a report within `max_s / 2` of `t` at either end).
    pub fn at(&self, mmsi: &str, t: f64, max_s: f64) -> Option<(f64, f64, Option<f64>)> {
        interp(self.by_mmsi.get(mmsi)?, t, max_s)
    }
}

/// Interpolated `(lat, lon, sog)` of a sorted track at `t`.
pub fn interp(v: &[Report], t: f64, max_s: f64) -> Option<(f64, f64, Option<f64>)> {
    if v.is_empty() {
        return None;
    }
    let i = v.partition_point(|r| r.t <= t);
    let near = |r: &Report| ((t - r.t).abs() <= max_s / 2.0).then_some((r.lat, r.lon, r.sog_kn));
    match (i.checked_sub(1).map(|j| &v[j]), v.get(i)) {
        (Some(a), Some(b)) if b.t - a.t <= max_s => {
            let f = if b.t > a.t {
                (t - a.t) / (b.t - a.t)
            } else {
                0.0
            };
            Some((
                a.lat + f * (b.lat - a.lat),
                a.lon + f * (b.lon - a.lon),
                a.sog_kn.or(b.sog_kn),
            ))
        }
        (Some(a), Some(b)) => near(a).or_else(|| near(b)),
        (Some(a), None) => near(a),
        (None, Some(b)) => near(b),
        (None, None) => None,
    }
}

/// Pre-registered thresholds of the track engine (written into every pack).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// A gap is a period with no report longer than this (s).
    pub gap_s: f64,
    /// Fastest credible vessel speed (kn): an implied speed above it between consecutive
    /// reports is a jump.
    pub max_speed_kn: f64,
    /// Encounter: both vessels within this distance (m)...
    pub encounter_dist_m: f64,
    /// ... both at or below this speed (kn)...
    pub encounter_max_sog_kn: f64,
    /// ... for at least this long (s).
    pub encounter_min_s: f64,
    /// Time grid step for encounters and dwell (s).
    pub grid_s: f64,
    /// Longest interval between reports bridged by interpolation (s).
    pub interp_max_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            gap_s: 3.0 * 3600.0,
            max_speed_kn: 50.0,
            encounter_dist_m: 500.0,
            encounter_max_sog_kn: 3.0,
            encounter_min_s: 2.0 * 3600.0,
            grid_s: 600.0,
            interp_max_s: 1800.0,
        }
    }
}

impl Params {
    /// Every threshold positive.
    pub fn validate(&self) -> Result<(), String> {
        let all = [
            ("gap_s", self.gap_s),
            ("max_speed_kn", self.max_speed_kn),
            ("encounter_dist_m", self.encounter_dist_m),
            ("encounter_max_sog_kn", self.encounter_max_sog_kn),
            ("encounter_min_s", self.encounter_min_s),
            ("grid_s", self.grid_s),
            ("interp_max_s", self.interp_max_s),
        ];
        for (n, v) in all {
            if !(v > 0.0 && v.is_finite()) {
                return Err(format!("params.{n} must be positive"));
            }
        }
        Ok(())
    }
}

/// A reception gap.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gap {
    pub mmsi: String,
    pub start_utc: String,
    pub end_utc: String,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
    pub duration_h: f64,
    pub start: [f64; 2],
    pub end: [f64; 2],
    /// Straight-line distance between the last and next report (km).
    pub displacement_km: f64,
    /// Average speed the displacement implies (kn).
    pub implied_speed_kn: f64,
}

/// Reception gaps of one track.
pub fn gaps(v: &[Report], p: &Params) -> Vec<Gap> {
    v.windows(2)
        .filter(|w| w[1].t - w[0].t > p.gap_s)
        .map(|w| {
            let (a, b) = (&w[0], &w[1]);
            let d = dist_m(a.lat, a.lon, b.lat, b.lon);
            let dt = b.t - a.t;
            Gap {
                mmsi: a.mmsi.clone(),
                start_utc: iso(a.t),
                end_utc: iso(b.t),
                t0: a.t,
                t1: b.t,
                duration_h: dt / 3600.0,
                start: [a.lat, a.lon],
                end: [b.lat, b.lon],
                displacement_km: d / 1000.0,
                implied_speed_kn: d / dt / KN,
            }
        })
        .collect()
}

/// An implied-speed jump between consecutive reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Jump {
    pub mmsi: String,
    pub utc: String,
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub distance_km: f64,
    pub implied_speed_kn: f64,
}

/// Jumps of one track (consecutive reports at least 1 s apart).
pub fn jumps(v: &[Report], p: &Params) -> Vec<Jump> {
    v.windows(2)
        .filter_map(|w| {
            let (a, b) = (&w[0], &w[1]);
            let dt = (b.t - a.t).max(1.0);
            let d = dist_m(a.lat, a.lon, b.lat, b.lon);
            let s = d / dt / KN;
            (s > p.max_speed_kn).then(|| Jump {
                mmsi: a.mmsi.clone(),
                utc: iso(b.t),
                from: [a.lat, a.lon],
                to: [b.lat, b.lon],
                distance_km: d / 1000.0,
                implied_speed_kn: s,
            })
        })
        .collect()
}

/// Simultaneous use of one MMSI: a share of jumps that alternate back to the earlier
/// position, as two transmitters on one identity would produce.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Collision {
    pub mmsi: String,
    /// Reports that jump away and back (A-B-A pattern).
    pub alternations: usize,
    pub reports: usize,
    pub first_utc: String,
}

/// MMSI collision indicator of one track: A-B-A alternations where both legs are jumps.
pub fn collision(v: &[Report], p: &Params) -> Option<Collision> {
    let fast = |a: &Report, b: &Report| {
        dist_m(a.lat, a.lon, b.lat, b.lon) / (b.t - a.t).max(1.0) / KN > p.max_speed_kn
    };
    let mut n = 0;
    let mut first = None;
    for w in v.windows(3) {
        let (a, b, c) = (&w[0], &w[1], &w[2]);
        if fast(a, b)
            && fast(b, c)
            && dist_m(a.lat, a.lon, c.lat, c.lon) < 0.2 * dist_m(a.lat, a.lon, b.lat, b.lon)
        {
            n += 1;
            first.get_or_insert(b.t);
        }
    }
    (n >= 2).then(|| Collision {
        mmsi: v[0].mmsi.clone(),
        alternations: n,
        reports: v.len(),
        first_utc: iso(first.unwrap_or(v[0].t)),
    })
}

/// One change of a static field.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IdentityChange {
    pub mmsi: String,
    pub field: String,
    pub from: String,
    pub to: String,
    pub utc: String,
}

/// Changes of IMO, name, call sign, flag or ship type in one MMSI's static records
/// (empty values are ignored).
pub fn identity_changes(v: &[StaticRecord]) -> Vec<IdentityChange> {
    let mut out = Vec::new();
    let mut last: BTreeMap<&str, (&str, f64)> = BTreeMap::new();
    for r in v {
        for (f, val) in [
            ("imo", r.imo.as_str()),
            ("name", r.name.as_str()),
            ("callsign", r.callsign.as_str()),
            ("flag", r.flag.as_str()),
            ("ship_type", r.ship_type.as_str()),
        ] {
            if val.is_empty() {
                continue;
            }
            if let Some((prev, _)) = last.get(f) {
                if *prev != val {
                    out.push(IdentityChange {
                        mmsi: r.mmsi.clone(),
                        field: f.into(),
                        from: (*prev).into(),
                        to: val.into(),
                        utc: iso(r.t),
                    });
                }
            }
            last.insert(f, (val, r.t));
        }
    }
    out
}

/// IMO numbers broadcast under more than one MMSI: `imo -> [mmsi]`.
pub fn shared_imo(t: &Tracks) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (mmsi, v) in &t.statics {
        for r in v.iter().filter(|r| !r.imo.is_empty() && r.imo != "0") {
            m.entry(r.imo.clone()).or_default().insert(mmsi.clone());
        }
    }
    m.into_iter()
        .filter(|(_, s)| s.len() > 1)
        .map(|(k, s)| (k, s.into_iter().collect()))
        .collect()
}

/// An encounter candidate: two vessels close together and slow for a sustained time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Encounter {
    pub a: String,
    pub b: String,
    pub start_utc: String,
    pub end_utc: String,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
    pub duration_h: f64,
    /// Closest separation on the grid (m).
    pub min_separation_m: f64,
    /// Mean position of the pair.
    pub position: [f64; 2],
}

/// A slow vessel at one grid step: MMSI, latitude, longitude.
type Slot<'a> = (&'a str, f64, f64);

/// Encounters among `focus` vessels (with any vessel) on the time grid.
pub fn encounters(t: &Tracks, focus: &BTreeSet<String>, p: &Params) -> Vec<Encounter> {
    let (Some(t0), Some(t1)) = (
        t.by_mmsi
            .values()
            .filter_map(|v| v.first())
            .map(|r| r.t)
            .reduce(f64::min),
        t.by_mmsi
            .values()
            .filter_map(|v| v.last())
            .map(|r| r.t)
            .reduce(f64::max),
    ) else {
        return Vec::new();
    };
    // Open runs per pair: (start, last, min sep, sum lat, sum lon, n).
    type Run = (f64, f64, f64, f64, f64, usize);
    let mut open: BTreeMap<(String, String), Run> = BTreeMap::new();
    let mut out = Vec::new();
    let close = |k: &(String, String), r: Run, out: &mut Vec<Encounter>| {
        let dur = r.1 - r.0 + p.grid_s;
        if dur >= p.encounter_min_s {
            out.push(Encounter {
                a: k.0.clone(),
                b: k.1.clone(),
                start_utc: iso(r.0),
                end_utc: iso(r.1),
                t0: r.0,
                t1: r.1,
                duration_h: dur / 3600.0,
                min_separation_m: r.2,
                position: [r.3 / r.5 as f64, r.4 / r.5 as f64],
            });
        }
    };
    let cell_deg = (p.encounter_dist_m / 111_000.0).max(1e-4);
    let mut tk = t0;
    while tk <= t1 {
        // Slow vessels at this step, hashed by cell.
        let mut grid: BTreeMap<(i64, i64), Vec<Slot>> = BTreeMap::new();
        for (m, v) in &t.by_mmsi {
            if let Some((lat, lon, sog)) = interp(v, tk, p.interp_max_s) {
                if sog.is_some_and(|s| s <= p.encounter_max_sog_kn) {
                    let key = (
                        (lat / cell_deg).floor() as i64,
                        (lon / cell_deg).floor() as i64,
                    );
                    grid.entry(key).or_default().push((m.as_str(), lat, lon));
                }
            }
        }
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        for (&(ci, cj), here) in &grid {
            for &(ma, la, lo) in here {
                for di in -1..=1 {
                    for dj in -1..=1 {
                        let Some(nb) = grid.get(&(ci + di, cj + dj)) else {
                            continue;
                        };
                        for &(mb, lb, lob) in nb {
                            if ma >= mb || !(focus.contains(ma) || focus.contains(mb)) {
                                continue;
                            }
                            let d = dist_m(la, lo, lb, lob);
                            if d <= p.encounter_dist_m {
                                let k = (ma.to_string(), mb.to_string());
                                if !seen.insert(k.clone()) {
                                    continue;
                                }
                                let e = open.entry(k).or_insert((tk, tk, d, 0.0, 0.0, 0));
                                e.1 = tk;
                                e.2 = e.2.min(d);
                                e.3 += (la + lb) / 2.0;
                                e.4 += (lo + lob) / 2.0;
                                e.5 += 1;
                            }
                        }
                    }
                }
            }
        }
        let ended: Vec<_> = open
            .keys()
            .filter(|k| !seen.contains(*k))
            .cloned()
            .collect();
        for k in ended {
            if let Some(r) = open.remove(&k) {
                close(&k, r, &mut out);
            }
        }
        tk += p.grid_s;
    }
    for (k, r) in std::mem::take(&mut open) {
        close(&k, r, &mut out);
    }
    out.sort_by(|x, y| x.t0.total_cmp(&y.t0).then(x.a.cmp(&y.a)));
    out
}

/// A declared area: a closed polygon of `[lat, lon]` vertices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub name: String,
    /// The user's label (e.g. `closed-area`, `eez`, `port-approach`).
    #[serde(default)]
    pub kind: String,
    pub polygon: Vec<[f64; 2]>,
}

impl Area {
    /// Check the polygon.
    pub fn validate(&self) -> Result<(), String> {
        if self.polygon.len() < 3 {
            return Err(format!("area '{}' needs at least 3 vertices", self.name));
        }
        if self
            .polygon
            .iter()
            .any(|p| !(-90.0..=90.0).contains(&p[0]) || !(-180.0..=180.0).contains(&p[1]))
        {
            return Err(format!("area '{}' has a vertex out of range", self.name));
        }
        Ok(())
    }

    /// Point inside (ray casting in lat/lon; areas must not cross the antimeridian).
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        let p = &self.polygon;
        let mut inside = false;
        let mut j = p.len() - 1;
        for i in 0..p.len() {
            let (yi, xi, yj, xj) = (p[i][0], p[i][1], p[j][0], p[j][1]);
            if (yi > lat) != (yj > lat) && lon < (xj - xi) * (lat - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    /// Distance to the area (m): zero inside, else to the nearest edge.
    pub fn distance_m(&self, lat: f64, lon: f64) -> f64 {
        if self.contains(lat, lon) {
            return 0.0;
        }
        let n = self.polygon.len();
        (0..n)
            .map(|i| seg_dist_m(lat, lon, self.polygon[i], self.polygon[(i + 1) % n]))
            .fold(f64::INFINITY, f64::min)
    }
}

/// Distance (m) from a point to the segment `a`–`b` (local tangent plane at the point).
pub fn seg_dist_m(lat: f64, lon: f64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (ax, ay) = en_m(lat, lon, a[0], a[1]);
    let (bx, by) = en_m(lat, lon, b[0], b[1]);
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let f = if l2 > 0.0 {
        (-(ax * dx + ay * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (px, py) = (ax + f * dx, ay + f * dy);
    (px * px + py * py).sqrt()
}

/// Read a file relative to the request's directory.
pub(crate) fn read_rel(base: &Path, p: &str) -> Result<String, String> {
    crate::timing::grid::read_text(base, p)
}

/// Load positions (and optional static data) named by a request.
pub fn load_tracks(base: &Path, ais: &str, statics: Option<&str>) -> Result<Tracks, String> {
    let mut t = Tracks::parse(&read_rel(base, ais)?).map_err(|e| format!("{ais}: {e}"))?;
    if let Some(s) = statics {
        t.add_static(&read_rel(base, s)?)
            .map_err(|e| format!("{s}: {e}"))?;
    }
    if t.by_mmsi.is_empty() {
        return Err(format!("{ais}: no usable position report"));
    }
    Ok(t)
}

/// SHA-256 of an input, recorded in every pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InputHash {
    pub file: String,
    pub sha256: String,
}

/// Hash each named input.
pub fn hash_inputs(base: &Path, files: &[&str]) -> Result<Vec<InputHash>, String> {
    files
        .iter()
        .map(|f| {
            let b = read_rel(base, f)?;
            Ok(InputHash {
                file: (*f).to_string(),
                sha256: crate::forensics::inputs::sha256_hex(b.as_bytes()),
            })
        })
        .collect()
}

/// Round to `d` decimals.
pub(crate) fn r(x: f64, d: i32) -> f64 {
    crate::forensics::inputs::round(x, d)
}
