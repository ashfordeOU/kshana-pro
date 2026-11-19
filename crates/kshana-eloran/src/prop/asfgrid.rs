// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! ASF grids: generate per-station ASF (with ECD and field strength) on a lat/lon grid with
//! any ground model, export them (CSV, GeoJSON points, GeoJSON contours), and import a grid
//! back as a correction table — as a propagation override (`[propagation.ground]
//! model = "asf_grid"`) or as receiver ASF corrections ([`AsfGrid::asf_correction_ns`]).
//!
//! # Quantities
//!
//! At each node and for each station the scene's ground model is evaluated at scene time 0
//! (temporal terms at the scene epoch) and compared with the seawater smooth-earth path
//! (`smooth_earth`, σ = 5 S/m, the SF reference, same `k_factor`):
//!
//! * `asf_ns` — the ASF of the model (its `PathEffect::asf_ns`);
//! * `ecd_us`, `field_dbuv_m` — the propagation ECD and the field (dB µV/m, at the
//!   station's configured power);
//! * `dfield_db`, `decd_ns` — field and ECD relative to the seawater path.
//!
//! An imported grid supplies `asf_ns` and, when present, `dfield_db` and `decd_ns`; the
//! override model rebuilds the path exactly as the smooth-earth/Millington models do from a
//! correction relative to seawater ([`super::terrain::path_from_correction`]), so a model
//! exported and re-imported reproduces its ASF, ECD and field at the nodes (to the printed
//! precision) and interpolates bilinearly in between.
//!
//! # CSV format (v1)
//!
//! ```text
//! # kshana-eloran ASF grid v1; model: …
//! station,st_lat_deg,st_lon_deg,lat_deg,lon_deg,asf_ns,ecd_us,field_dbuv_m,dfield_db,decd_ns
//! M,55.0,0.0,54.0,1.0,123.4567,-0.31234,78.12345,-3.21000,-12.3456
//! ```
//!
//! On import the columns may be in any order; `station`, `lat_deg`, `lon_deg`, `asf_ns`
//! are required, the station position (when given) is checked to within 1 km of the
//! scene's station with that id, and the nodes of each station must form a regular grid
//! (missing nodes are NaN: the four-node bilinear stencil then renormalises over the nodes
//! present). Outside its grid a station gets `outside = "zero"` (ASF 0, i.e. seawater —
//! the default) or `"clamp"` (the nearest edge value). A station absent from the file gets
//! ASF 0. The override's label states that ASF comes from a table.

use super::conductivity::{ConductivityConfig, ConductivityMap};
use super::ground::{ground_curve, Correction, Groundwave, K_LORAN};
use super::smooth_earth::{GroundCurve, SEAWATER};
use super::terrain::path_from_correction;
use super::{PathEffect, PathKind, PropagationModel};
use crate::chain::Station;
use crate::geo::{inverse, Geo};
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::sync::Arc;

/// A lat/lon box and resolution.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridSpec {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
    pub res_deg: f64,
}

impl GridSpec {
    pub fn dims(&self) -> Result<(usize, usize), String> {
        if !(self.res_deg > 0.0) || self.lat_max < self.lat_min || self.lon_max < self.lon_min {
            return Err("ASF grid: res_deg > 0 and min ≤ max".into());
        }
        let n = |a: f64, b: f64| ((b - a) / self.res_deg + 1e-9).floor() as usize + 1;
        let (nlat, nlon) = (n(self.lat_min, self.lat_max), n(self.lon_min, self.lon_max));
        if nlat * nlon > 4_000_000 {
            return Err(format!("ASF grid: {nlat}×{nlon} nodes is too many"));
        }
        Ok((nlat, nlon))
    }
}

/// Per-node values of one station.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AsfValues {
    pub asf_ns: f64,
    pub ecd_us: f64,
    pub field_dbuv_m: f64,
    pub dfield_db: f64,
    pub decd_ns: f64,
}

/// One station's grid (node `(i, j)` at `(lat0 + i·dlat, lon0 + j·dlon)`).
#[derive(Clone, Debug, PartialEq)]
pub struct StationGrid {
    pub station: String,
    pub position: Option<Geo>,
    pub lat0: f64,
    pub lon0: f64,
    pub dlat: f64,
    pub dlon: f64,
    pub nlat: usize,
    pub nlon: usize,
    /// Row-major, south row first; NaN fields = missing.
    pub values: Vec<AsfValues>,
}

/// Behaviour outside a station's grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outside {
    #[default]
    Zero,
    Clamp,
}

impl StationGrid {
    fn at(&self, i: usize, j: usize) -> &AsfValues {
        &self.values[i * self.nlon + j]
    }

    /// Bilinear values at `p` (`None` outside unless `clamp`).
    pub fn interp(&self, p: Geo, clamp: bool) -> Option<AsfValues> {
        let mut fi = (p.lat_deg - self.lat0) / self.dlat;
        let mut fj = ((p.lon_deg - self.lon0 + 540.0).rem_euclid(360.0) - 180.0) / self.dlon;
        let (imax, jmax) = ((self.nlat - 1) as f64, (self.nlon - 1) as f64);
        let e = 1e-9;
        if (fi < -e || fj < -e || fi > imax + e || fj > jmax + e) && !clamp {
            return None;
        }
        fi = fi.clamp(0.0, imax);
        fj = fj.clamp(0.0, jmax);
        let i0 = (fi.floor() as usize).min(self.nlat.saturating_sub(2));
        let j0 = (fj.floor() as usize).min(self.nlon.saturating_sub(2));
        let (u, v) = (fi - i0 as f64, fj - j0 as f64);
        let mut acc = [0.0f64; 5];
        let mut wsum = [0.0f64; 5];
        let i1 = (i0 + 1).min(self.nlat - 1);
        let j1 = (j0 + 1).min(self.nlon - 1);
        for (i, j, w) in [
            (i0, j0, (1.0 - u) * (1.0 - v)),
            (i1, j0, u * (1.0 - v)),
            (i0, j1, (1.0 - u) * v),
            (i1, j1, u * v),
        ] {
            let a = self.at(i, j);
            for (k, x) in [a.asf_ns, a.ecd_us, a.field_dbuv_m, a.dfield_db, a.decd_ns]
                .into_iter()
                .enumerate()
            {
                if x.is_finite() && w > 0.0 {
                    acc[k] += w * x;
                    wsum[k] += w;
                }
            }
        }
        if wsum[0] <= 1e-12 {
            return None;
        }
        let f = |k: usize, dflt: f64| {
            if wsum[k] > 1e-12 {
                acc[k] / wsum[k]
            } else {
                dflt
            }
        };
        Some(AsfValues {
            asf_ns: f(0, 0.0),
            ecd_us: f(1, f64::NAN),
            field_dbuv_m: f(2, f64::NAN),
            dfield_db: f(3, 0.0),
            decd_ns: f(4, 0.0),
        })
    }
}

/// ASF grids of a set of stations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AsfGrid {
    pub model: String,
    pub stations: Vec<StationGrid>,
}

/// The seawater smooth-earth reference model.
fn sea_model(k_factor: f64) -> Result<Groundwave, String> {
    let map = ConductivityMap::build(&ConductivityConfig::Uniform {
        sigma_s_m: SEAWATER.sigma,
        eps_r: Some(SEAWATER.eps_r),
    })?;
    Groundwave::new(map, k_factor, 2.0, 0.005, None, "seawater")
}

/// Compute the grid for `stations` with `model` (its ground path), seawater reference with
/// `k_factor` (use the model's own k).
pub fn generate(
    model: &dyn PropagationModel,
    stations: &[Station],
    spec: &GridSpec,
    k_factor: f64,
) -> Result<AsfGrid, String> {
    let (nlat, nlon) = spec.dims()?;
    let sea = sea_model(k_factor)?;
    let mut out = AsfGrid {
        model: model.name(),
        stations: Vec::new(),
    };
    for st in stations {
        let mut values = Vec::with_capacity(nlat * nlon);
        for i in 0..nlat {
            for j in 0..nlon {
                let p = Geo::new(
                    spec.lat_min + i as f64 * spec.res_deg,
                    spec.lon_min + j as f64 * spec.res_deg,
                );
                let rx = RxState {
                    t_s: 0.0,
                    pos: p,
                    heading_rad: 0.0,
                    speed_mps: 0.0,
                };
                let g = model
                    .paths(st, &rx)
                    .into_iter()
                    .find(|q| q.kind == PathKind::Ground)
                    .ok_or_else(|| {
                        format!("ASF grid: model {} gives no groundwave", model.name())
                    })?;
                let s = sea.evaluate(st.position, st.power_kw, p, 0.0);
                values.push(AsfValues {
                    asf_ns: g.asf_ns,
                    ecd_us: g.ecd_s * 1e6,
                    field_dbuv_m: 20.0 * g.field_uv_m.max(1e-30).log10(),
                    dfield_db: 20.0 * (g.field_uv_m / s.field_uv_m).log10(),
                    decd_ns: (g.ecd_s - s.ecd_s) * 1e9,
                });
            }
        }
        out.stations.push(StationGrid {
            station: st.id.clone(),
            position: Some(st.position),
            lat0: spec.lat_min,
            lon0: spec.lon_min,
            dlat: spec.res_deg,
            dlon: spec.res_deg,
            nlat,
            nlon,
            values,
        });
    }
    Ok(out)
}

const HEADER: &str =
    "station,st_lat_deg,st_lon_deg,lat_deg,lon_deg,asf_ns,ecd_us,field_dbuv_m,dfield_db,decd_ns";

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

impl AsfGrid {
    /// The grid of station `id` (position, when stored, within 1 km of `pos`).
    pub fn station(&self, id: &str, pos: Geo) -> Option<&StationGrid> {
        self.stations.iter().find(|s| {
            s.station == id
                && s.position
                    .is_none_or(|p| inverse(p, pos).distance_m < 1000.0)
        })
    }

    /// Receiver ASF correction (ns) for `station` at `rx`: the value to subtract from a
    /// measured ToA (or add to a PF+SF prediction). `None` outside the grid or for an
    /// unknown station.
    pub fn asf_correction_ns(&self, station: &Station, rx: Geo) -> Option<f64> {
        self.station(&station.id, station.position)?
            .interp(rx, false)
            .map(|v| v.asf_ns)
    }

    /// CSV export (format v1).
    pub fn to_csv(&self) -> String {
        let mut s = format!(
            "# kshana-eloran ASF grid v1; model: {} (MODELLED values)\n{HEADER}\n",
            self.model.replace('\n', " ")
        );
        for g in &self.stations {
            let (sl, so) = g
                .position
                .map(|p| (format!("{}", p.lat_deg), format!("{}", p.lon_deg)))
                .unwrap_or_default();
            for i in 0..g.nlat {
                for j in 0..g.nlon {
                    let v = g.at(i, j);
                    let _ = writeln!(
                        s,
                        "{},{sl},{so},{:.9},{:.9},{:.4},{:.6},{:.5},{:.6},{:.4}",
                        g.station,
                        g.lat0 + i as f64 * g.dlat,
                        g.lon0 + j as f64 * g.dlon,
                        v.asf_ns,
                        v.ecd_us,
                        v.field_dbuv_m,
                        v.dfield_db,
                        v.decd_ns
                    );
                }
            }
        }
        s
    }

    /// GeoJSON `FeatureCollection` of node `Point`s with the values as properties.
    pub fn to_geojson_points(&self) -> String {
        let mut f = Vec::new();
        for g in &self.stations {
            for i in 0..g.nlat {
                for j in 0..g.nlon {
                    let v = g.at(i, j);
                    let num = |x: f64| {
                        if x.is_finite() {
                            format!("{x:.4}")
                        } else {
                            "null".into()
                        }
                    };
                    f.push(format!(
                        "{{\"type\":\"Feature\",\"geometry\":{{\"type\":\"Point\",\"coordinates\":[{:.9},{:.9}]}},\
                         \"properties\":{{\"station\":{},\"asf_ns\":{},\"ecd_us\":{},\"field_dbuv_m\":{},\"dfield_db\":{}}}}}",
                        g.lon0 + j as f64 * g.dlon,
                        g.lat0 + i as f64 * g.dlat,
                        json_str(&g.station),
                        num(v.asf_ns),
                        num(v.ecd_us),
                        num(v.field_dbuv_m),
                        num(v.dfield_db)
                    ));
                }
            }
        }
        format!(
            "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}",
            f.join(",")
        )
    }

    /// GeoJSON contours of ASF (marching squares; one `MultiLineString` of segments per
    /// station and level, properties `station`, `asf_ns`).
    pub fn to_geojson_contours(&self, levels_ns: &[f64]) -> String {
        let mut f = Vec::new();
        for g in &self.stations {
            for &lv in levels_ns {
                let segs = contour_segments(g, lv);
                if segs.is_empty() {
                    continue;
                }
                let lines: Vec<String> = segs
                    .iter()
                    .map(|(a, b)| format!("[[{:.9},{:.9}],[{:.9},{:.9}]]", a.1, a.0, b.1, b.0))
                    .collect();
                f.push(format!(
                    "{{\"type\":\"Feature\",\"geometry\":{{\"type\":\"MultiLineString\",\"coordinates\":[{}]}},\
                     \"properties\":{{\"station\":{},\"asf_ns\":{lv}}}}}",
                    lines.join(","),
                    json_str(&g.station)
                ));
            }
        }
        format!(
            "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}",
            f.join(",")
        )
    }

    /// Import a CSV grid (see the module documentation).
    pub fn from_csv(text: &str) -> Result<AsfGrid, String> {
        let mut model = String::from("imported ASF grid");
        let mut header: Option<Vec<String>> = None;
        type Row = (String, Option<Geo>, f64, f64, AsfValues);
        let mut rows: Vec<Row> = Vec::new();
        for (ln, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            if let Some(c) = t.strip_prefix('#') {
                if let Some(m) = c.split("model:").nth(1) {
                    model = format!("imported ASF grid ({})", m.trim());
                }
                continue;
            }
            let f: Vec<&str> = t.split(',').map(str::trim).collect();
            let Some(h) = &header else {
                header = Some(f.iter().map(|s| s.to_ascii_lowercase()).collect());
                continue;
            };
            let col = |n: &str| h.iter().position(|x| x == n);
            let num = |n: &str| -> Result<Option<f64>, String> {
                match col(n).and_then(|i| f.get(i)) {
                    Some(s) if !s.is_empty() => s
                        .parse::<f64>()
                        .map(Some)
                        .map_err(|e| format!("ASF CSV line {}: {n}: {e}", ln + 1)),
                    _ => Ok(None),
                }
            };
            let req = |n: &str| -> Result<f64, String> {
                num(n)?.ok_or_else(|| format!("ASF CSV line {}: {n} missing", ln + 1))
            };
            let id = col("station")
                .and_then(|i| f.get(i))
                .ok_or_else(|| format!("ASF CSV line {}: station missing", ln + 1))?
                .to_string();
            let pos = match (num("st_lat_deg")?, num("st_lon_deg")?) {
                (Some(a), Some(b)) => Some(Geo::new(a, b)),
                _ => None,
            };
            rows.push((
                id,
                pos,
                req("lat_deg")?,
                req("lon_deg")?,
                AsfValues {
                    asf_ns: req("asf_ns")?,
                    ecd_us: num("ecd_us")?.unwrap_or(f64::NAN),
                    field_dbuv_m: num("field_dbuv_m")?.unwrap_or(f64::NAN),
                    dfield_db: num("dfield_db")?.unwrap_or(f64::NAN),
                    decd_ns: num("decd_ns")?.unwrap_or(f64::NAN),
                },
            ));
        }
        let mut out = AsfGrid {
            model,
            stations: Vec::new(),
        };
        let mut keys: Vec<(String, Option<Geo>)> = Vec::new();
        for r in &rows {
            if !keys.iter().any(|k| k.0 == r.0 && k.1 == r.1) {
                keys.push((r.0.clone(), r.1));
            }
        }
        for (id, pos) in keys {
            let pts: Vec<&Row> = rows.iter().filter(|r| r.0 == id && r.1 == pos).collect();
            let axis = |v: Vec<f64>| -> Result<(f64, f64, usize), String> {
                let mut u = v;
                u.sort_by(f64::total_cmp);
                u.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
                if u.len() < 2 {
                    return Err(format!(
                        "ASF CSV: station {id}: grid needs ≥ 2 nodes per axis"
                    ));
                }
                let step = (1..u.len())
                    .map(|i| u[i] - u[i - 1])
                    .fold(f64::INFINITY, f64::min);
                let n = ((u[u.len() - 1] - u[0]) / step).round() as usize + 1;
                if u.iter().any(|x| {
                    let k = (x - u[0]) / step;
                    (k - k.round()).abs() > 1e-6
                }) {
                    return Err(format!(
                        "ASF CSV: station {id}: nodes are not on a regular grid"
                    ));
                }
                Ok((u[0], step, n))
            };
            let (lat0, dlat, nlat) = axis(pts.iter().map(|r| r.2).collect())?;
            let (lon0, dlon, nlon) = axis(pts.iter().map(|r| r.3).collect())?;
            let nan = AsfValues {
                asf_ns: f64::NAN,
                ecd_us: f64::NAN,
                field_dbuv_m: f64::NAN,
                dfield_db: f64::NAN,
                decd_ns: f64::NAN,
            };
            let mut values = vec![nan; nlat * nlon];
            for r in pts {
                let i = ((r.2 - lat0) / dlat).round() as usize;
                let j = ((r.3 - lon0) / dlon).round() as usize;
                values[i * nlon + j] = r.4;
            }
            out.stations.push(StationGrid {
                station: id,
                position: pos,
                lat0,
                lon0,
                dlat,
                dlon,
                nlat,
                nlon,
                values,
            });
        }
        Ok(out)
    }
}

/// Marching-squares segments ((lat, lon), (lat, lon)) of one level.
fn contour_segments(g: &StationGrid, level: f64) -> Vec<((f64, f64), (f64, f64))> {
    let mut out = Vec::new();
    if g.nlat < 2 || g.nlon < 2 {
        return out;
    }
    let pos = |i: usize, j: usize| (g.lat0 + i as f64 * g.dlat, g.lon0 + j as f64 * g.dlon);
    for i in 0..g.nlat - 1 {
        for j in 0..g.nlon - 1 {
            // corners counter-clockwise from SW
            let c = [(i, j), (i, j + 1), (i + 1, j + 1), (i + 1, j)];
            let v: Vec<f64> = c.iter().map(|&(a, b)| g.at(a, b).asf_ns).collect();
            if v.iter().any(|x| !x.is_finite()) {
                continue;
            }
            let mut pts = Vec::new();
            for e in 0..4 {
                let (a, b) = (e, (e + 1) % 4);
                if (v[a] >= level) != (v[b] >= level) {
                    let t = (level - v[a]) / (v[b] - v[a]);
                    let (pa, pb) = (pos(c[a].0, c[a].1), pos(c[b].0, c[b].1));
                    pts.push((pa.0 + t * (pb.0 - pa.0), pa.1 + t * (pb.1 - pa.1)));
                }
            }
            match pts.len() {
                2 => out.push((pts[0], pts[1])),
                4 => {
                    // saddle: resolve with the cell mean
                    let mean = v.iter().sum::<f64>() / 4.0;
                    if (mean >= level) == (v[0] >= level) {
                        out.push((pts[0], pts[3]));
                        out.push((pts[1], pts[2]));
                    } else {
                        out.push((pts[0], pts[1]));
                        out.push((pts[2], pts[3]));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// `model = "asf_grid"`: groundwave from an imported ASF table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsfGridConfig {
    /// CSV grid file (format v1).
    pub file: String,
    #[serde(default)]
    pub outside: Outside,
    /// Effective-earth-radius factor of the seawater SF reference.
    #[serde(default = "d_k")]
    pub k_factor: f64,
}

fn d_k() -> f64 {
    K_LORAN
}

/// The ASF-table groundwave model.
pub struct AsfGridModel {
    pub grid: AsfGrid,
    pub outside: Outside,
    pub k_factor: f64,
    sea: Arc<GroundCurve>,
}

impl AsfGridModel {
    pub fn new(grid: AsfGrid, outside: Outside, k_factor: f64) -> Result<AsfGridModel, String> {
        if !(k_factor > 0.5 && k_factor < 10.0) {
            return Err(format!(
                "asf_grid: k_factor {k_factor} out of range (0.5, 10)"
            ));
        }
        Ok(AsfGridModel {
            grid,
            outside,
            k_factor,
            sea: ground_curve(SEAWATER, k_factor),
        })
    }

    /// Correction relative to seawater for `st` at `rx`.
    pub fn correction(&self, st: &Station, rx: Geo) -> Correction {
        let v = self
            .grid
            .station(&st.id, st.position)
            .and_then(|g| g.interp(rx, self.outside == Outside::Clamp))
            .unwrap_or_default();
        let fin = |x: f64| if x.is_finite() { x } else { 0.0 };
        let phase = fin(v.asf_ns) * 1e-9;
        Correction {
            phase_s: phase,
            ln_amp: fin(v.dfield_db) * std::f64::consts::LN_10 / 20.0,
            group_s: phase + fin(v.decd_ns) * 1e-9,
            land: 0.0,
        }
    }
}

impl PropagationModel for AsfGridModel {
    fn name(&self) -> String {
        format!("groundwave-asf-table({})", self.grid.model)
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        let c = self.correction(st, rx.pos);
        vec![path_from_correction(
            &self.sea,
            self.k_factor,
            st.position,
            st.power_kw,
            rx.pos,
            c,
            0.0,
        )]
    }
}

/// Build the `asf_grid` model (reads the file).
pub fn build(cfg: &AsfGridConfig) -> Result<AsfGridModel, String> {
    let text =
        std::fs::read_to_string(&cfg.file).map_err(|e| format!("asf_grid {}: {e}", cfg.file))?;
    let grid = AsfGrid::from_csv(&text).map_err(|e| format!("asf_grid {}: {e}", cfg.file))?;
    AsfGridModel::new(grid, cfg.outside, cfg.k_factor)
}

#[cfg(test)]
mod tests {
    use super::super::conductivity::{GroundConst, PolygonConfig};
    use super::*;

    fn station(id: &str, lat: f64, lon: f64) -> Station {
        Station {
            id: id.into(),
            name: id.into(),
            role: crate::signal::Role::Master,
            position: Geo::new(lat, lon),
            power_kw: 250.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        }
    }

    fn coast_model() -> Groundwave {
        let map = ConductivityMap::build(&ConductivityConfig::Polygons {
            background: GroundConst::seawater(),
            polygons: vec![PolygonConfig {
                name: "land".into(),
                sigma_s_m: 0.003,
                eps_r: Some(15.0),
                vertices: vec![[51.0, -10.0], [60.0, -10.0], [60.0, 10.0], [51.0, 10.0]],
            }],
        })
        .unwrap();
        Groundwave::new(map, K_LORAN, 2.0, 0.005, None, "coast").unwrap()
    }

    fn spec() -> GridSpec {
        GridSpec {
            lat_min: 50.0,
            lat_max: 52.0,
            lon_min: 0.0,
            lon_max: 1.0,
            res_deg: 0.25,
        }
    }

    #[test]
    fn generate_export_import_round_trip() {
        let m = coast_model();
        let sts = [station("M", 54.0, 0.0), station("X", 49.0, -1.0)];
        let g = generate(&m, &sts, &spec(), K_LORAN).unwrap();
        assert_eq!(g.stations.len(), 2);
        assert_eq!((g.stations[0].nlat, g.stations[0].nlon), (9, 5));
        let csv = g.to_csv();
        let back = AsfGrid::from_csv(&csv).unwrap();
        assert_eq!(back.stations.len(), 2);
        assert!(back.model.contains("coast"));
        let model = AsfGridModel::new(back.clone(), Outside::Zero, K_LORAN).unwrap();
        for st in &sts {
            for (lat, lon) in [(50.0, 0.0), (51.5, 0.75), (52.0, 1.0)] {
                let rx = RxState {
                    t_s: 0.0,
                    pos: Geo::new(lat, lon),
                    heading_rad: 0.0,
                    speed_mps: 0.0,
                };
                let a = m.paths(st, &rx)[0];
                let b = model.paths(st, &rx)[0];
                assert!((a.asf_ns - b.asf_ns).abs() < 1e-3, "{} {lat} {lon}", st.id);
                assert!((a.delay - b.delay).as_ns().abs() < 2e-3);
                assert!((20.0 * (a.field_uv_m / b.field_uv_m).log10()).abs() < 1e-4);
                assert!((a.ecd_s - b.ecd_s).abs() < 1e-12);
            }
        }
        // between nodes: bilinear, close to the model away from the coast
        let st = &sts[0];
        let p = Geo::new(51.6, 0.4);
        let c = back.asf_correction_ns(st, p).unwrap();
        let e = m.paths(
            st,
            &RxState {
                t_s: 0.0,
                pos: p,
                heading_rad: 0.0,
                speed_mps: 0.0,
            },
        )[0]
        .asf_ns;
        assert!((c - e).abs() < 0.05 * e.abs() + 5.0, "{c} vs {e}");
        // outside / unknown station / moved station
        assert!(back.asf_correction_ns(st, Geo::new(53.0, 0.5)).is_none());
        assert!(back
            .asf_correction_ns(&station("Y", 54.0, 0.0), p)
            .is_none());
        assert!(back
            .asf_correction_ns(&station("M", 56.0, 0.0), p)
            .is_none());
        let rx_out = RxState {
            t_s: 0.0,
            pos: Geo::new(53.0, 0.5),
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        assert_eq!(model.paths(st, &rx_out)[0].asf_ns, 0.0);
        let clamp = AsfGridModel::new(back, Outside::Clamp, K_LORAN).unwrap();
        assert!(clamp.paths(st, &rx_out)[0].asf_ns > 0.0);
    }

    #[test]
    fn geojson_exports_and_contours() {
        let m = coast_model();
        let g = generate(&m, &[station("M", 54.0, 0.0)], &spec(), K_LORAN).unwrap();
        let pts: serde_json::Value = serde_json::from_str(&g.to_geojson_points()).unwrap();
        assert_eq!(pts["features"].as_array().unwrap().len(), 45);
        assert_eq!(pts["features"][0]["properties"]["station"], "M");
        // ASF decreases southward (more sea): a mid-range level crosses the grid
        let v = &g.stations[0].values;
        let (lo, hi) = (v[0].asf_ns, v[v.len() - 1].asf_ns);
        assert!(hi > lo);
        let lv = 0.5 * (lo + hi);
        let c: serde_json::Value =
            serde_json::from_str(&g.to_geojson_contours(&[lv, 1e9])).unwrap();
        let f = c["features"].as_array().unwrap();
        assert_eq!(f.len(), 1, "the out-of-range level has no contour");
        let segs = f[0]["geometry"]["coordinates"].as_array().unwrap();
        assert!(segs.len() >= 4);
        // each contour point lies where the interpolated ASF equals the level
        for s in segs {
            let p = &s[0];
            let q = Geo::new(p[1].as_f64().unwrap(), p[0].as_f64().unwrap());
            let a = g.stations[0].interp(q, false).unwrap().asf_ns;
            assert!((a - lv).abs() < 0.02 * lv.abs() + 1.0, "{a} vs {lv}");
        }
    }

    #[test]
    fn csv_import_checks_and_missing_nodes() {
        let csv = "station,lat_deg,lon_deg,asf_ns\nA,0,0,10\nA,0,1,20\nA,1,0,30\n";
        let g = AsfGrid::from_csv(csv).unwrap();
        let s = &g.stations[0];
        assert!(s.position.is_none());
        // missing (1,1): renormalised over the three present nodes
        let v = s.interp(Geo::new(0.5, 0.5), false).unwrap();
        assert!((v.asf_ns - 20.0).abs() < 1e-9);
        assert!((s.interp(Geo::new(0.0, 0.5), false).unwrap().asf_ns - 15.0).abs() < 1e-9);
        assert!(AsfGrid::from_csv(
            "station,lat_deg,lon_deg,asf_ns\nA,0,0,1\nA,0,0.3,1\nA,0,1,1\nA,1,0,1\n"
        )
        .is_err());
        assert!(AsfGrid::from_csv("station,lat_deg,lon_deg\nA,0,0\n").is_err());
        // through the configuration
        let dir = std::env::temp_dir().join(format!("kshana-asfgrid-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("asf.csv");
        std::fs::write(&f, csv).unwrap();
        let cfg: super::super::ground::GroundConfig = toml::from_str(&format!(
            "model = \"asf_grid\"\nfile = {:?}\noutside = \"clamp\"",
            f.to_string_lossy()
        ))
        .unwrap();
        let ctx = super::super::BuildContext::new(
            crate::time::AbsTime::from_utc_iso("2026-01-01T00:00:00Z").unwrap(),
            1.0,
            0,
        );
        let m = super::super::ground::build(&cfg, &ctx).unwrap();
        let rx = RxState {
            t_s: 0.0,
            pos: Geo::new(0.0, 0.5),
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        assert!((m.paths(&station("A", 5.0, 5.0), &rx)[0].asf_ns - 15.0).abs() < 1e-6);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
