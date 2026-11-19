// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Ground-conductivity maps for mixed-path groundwave: uniform, polygons on lat/lon, or a
//! regular lat/lon raster (inline, CSV or JSON).
//!
//! TOML (inside `[propagation.ground]`, model `millington`):
//!
//! ```toml
//! [propagation.ground.conductivity]
//! type = "polygons"
//! background = { sigma_s_m = 5.0, eps_r = 80.0 }       # outside every polygon (sea)
//! [[propagation.ground.conductivity.polygons]]
//! name = "island"
//! sigma_s_m = 0.003
//! eps_r = 15.0                                          # optional, default from σ
//! vertices = [[54.0, -6.0], [56.0, -6.0], [56.0, -2.0], [54.0, -2.0]]   # [lat, lon]
//! ```
//!
//! ```toml
//! [propagation.ground.conductivity]
//! type = "raster"
//! lat0_deg = 50.0   # south edge of row 0
//! lon0_deg = -5.0   # west edge of column 0
//! dlat_deg = 0.5
//! dlon_deg = 0.5
//! nlat = 4
//! nlon = 6
//! sigma_s_m = [ ... nlat·nlon values, row 0 (southernmost) first, west to east ... ]
//! # or: file = "path/to/grid.csv"  (nlat lines of nlon comma-separated σ, south row first;
//! #     '#' comments) or file = "grid.json" ({"sigma_s_m": [...], "eps_r": [...]?})
//! background = { sigma_s_m = 5.0, eps_r = 80.0 }   # outside the grid
//! ```
//!
//! Polygons: point-in-polygon by ray casting in (lon, lat) degrees, with longitudes
//! unwrapped around the query point (so a polygon may straddle ±180°); edges are straight
//! in lat/lon (fine for coastline-scale polygons). Where polygons overlap, the **last**
//! listed wins (list a lake after the land around it).

use crate::geo::Geo;
use serde::{Deserialize, Serialize};

/// Electrical constants of a ground type.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundConst {
    /// Conductivity (S/m).
    pub sigma_s_m: f64,
    /// Relative permittivity; default from σ ([`eps_for_sigma`]).
    #[serde(default)]
    pub eps_r: Option<f64>,
}

impl GroundConst {
    pub fn new(sigma_s_m: f64, eps_r: f64) -> GroundConst {
        GroundConst {
            sigma_s_m,
            eps_r: Some(eps_r),
        }
    }
    pub fn eps(&self) -> f64 {
        self.eps_r.unwrap_or_else(|| eps_for_sigma(self.sigma_s_m))
    }
    pub fn seawater() -> GroundConst {
        GroundConst::new(5.0, 80.0)
    }
    fn validate(&self) -> Result<(), String> {
        if !(self.sigma_s_m > 0.0 && self.sigma_s_m.is_finite()) {
            return Err(format!(
                "conductivity {} S/m must be positive",
                self.sigma_s_m
            ));
        }
        if !(self.eps() >= 1.0 && self.eps().is_finite()) {
            return Err(format!("relative permittivity {} must be ≥ 1", self.eps()));
        }
        Ok(())
    }
}

/// Representative relative permittivity for a conductivity, after the ground categories of
/// ITU-R P.527 (sea 70–80, wet ground 30, medium-dry ground 15, dry ground 7, very dry 3).
pub fn eps_for_sigma(sigma: f64) -> f64 {
    if sigma >= 1.0 {
        80.0
    } else if sigma >= 0.01 {
        30.0
    } else if sigma >= 0.001 {
        15.0
    } else if sigma >= 1e-4 {
        7.0
    } else {
        3.0
    }
}

/// A polygon of one ground type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolygonConfig {
    #[serde(default)]
    pub name: String,
    pub sigma_s_m: f64,
    #[serde(default)]
    pub eps_r: Option<f64>,
    /// Vertices `[lat_deg, lon_deg]` (closed implicitly).
    pub vertices: Vec<[f64; 2]>,
}

/// Conductivity map configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConductivityConfig {
    Uniform {
        sigma_s_m: f64,
        #[serde(default)]
        eps_r: Option<f64>,
    },
    Polygons {
        #[serde(default = "GroundConst::seawater")]
        background: GroundConst,
        polygons: Vec<PolygonConfig>,
    },
    Raster {
        lat0_deg: f64,
        lon0_deg: f64,
        dlat_deg: f64,
        dlon_deg: f64,
        nlat: usize,
        nlon: usize,
        /// Inline σ values (row-major, south row first); or use `file`.
        #[serde(default)]
        sigma_s_m: Vec<f64>,
        /// Inline ε_r values (same layout), optional.
        #[serde(default)]
        eps_r: Vec<f64>,
        /// CSV or JSON file with the grid (by extension).
        #[serde(default)]
        file: Option<String>,
        #[serde(default = "GroundConst::seawater")]
        background: GroundConst,
    },
}

impl Default for ConductivityConfig {
    fn default() -> Self {
        ConductivityConfig::Uniform {
            sigma_s_m: 5.0,
            eps_r: Some(80.0),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Poly {
    ground: GroundConst,
    lat: Vec<f64>,
    lon: Vec<f64>,
    bbox: (f64, f64), // lat min/max (lon handled by unwrapping)
}

impl Poly {
    fn contains(&self, p: Geo) -> bool {
        if p.lat_deg < self.bbox.0 || p.lat_deg > self.bbox.1 {
            return false;
        }
        let n = self.lat.len();
        let unwrap =
            |lon: f64| -> f64 { p.lon_deg + (lon - p.lon_deg + 540.0).rem_euclid(360.0) - 180.0 };
        let mut inside = false;
        let mut j = n - 1;
        for i in 0..n {
            let (yi, yj) = (self.lat[i], self.lat[j]);
            let (xi, xj) = (unwrap(self.lon[i]), unwrap(self.lon[j]));
            if (yi > p.lat_deg) != (yj > p.lat_deg) {
                let x = xj + (p.lat_deg - yj) * (xi - xj) / (yi - yj);
                if p.lon_deg < x {
                    inside = !inside;
                }
            }
            j = i;
        }
        inside
    }
}

/// A built conductivity map.
#[derive(Clone, Debug)]
pub enum ConductivityMap {
    Uniform(GroundConst),
    Polygons {
        background: GroundConst,
        polys: Vec<Poly>,
    },
    Raster {
        lat0: f64,
        lon0: f64,
        dlat: f64,
        dlon: f64,
        nlat: usize,
        nlon: usize,
        cells: Vec<GroundConst>,
        background: GroundConst,
    },
}

/// Quantise σ to 1/50 decade (≈ 4.7 %) so a raster yields a bounded set of ground types.
fn quantise_sigma(s: f64) -> f64 {
    10f64.powf((s.log10() * 50.0).round() / 50.0)
}

fn parse_csv(text: &str) -> Result<Vec<f64>, String> {
    let mut v = Vec::new();
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        for f in line.split([',', ';', ' ', '\t']).filter(|f| !f.is_empty()) {
            v.push(
                f.parse::<f64>()
                    .map_err(|e| format!("conductivity CSV line {}: {e}", ln + 1))?,
            );
        }
    }
    Ok(v)
}

#[derive(Deserialize)]
struct JsonGrid {
    sigma_s_m: Vec<f64>,
    #[serde(default)]
    eps_r: Vec<f64>,
}

impl ConductivityMap {
    pub fn build(cfg: &ConductivityConfig) -> Result<ConductivityMap, String> {
        match cfg {
            ConductivityConfig::Uniform { sigma_s_m, eps_r } => {
                let g = GroundConst {
                    sigma_s_m: *sigma_s_m,
                    eps_r: *eps_r,
                };
                g.validate()?;
                Ok(ConductivityMap::Uniform(g))
            }
            ConductivityConfig::Polygons {
                background,
                polygons,
            } => {
                background.validate()?;
                let mut polys = Vec::new();
                for p in polygons {
                    if p.vertices.len() < 3 {
                        return Err(format!(
                            "conductivity polygon {:?}: at least 3 vertices",
                            p.name
                        ));
                    }
                    let ground = GroundConst {
                        sigma_s_m: p.sigma_s_m,
                        eps_r: p.eps_r,
                    };
                    ground
                        .validate()
                        .map_err(|e| format!("polygon {:?}: {e}", p.name))?;
                    let lat: Vec<f64> = p.vertices.iter().map(|v| v[0]).collect();
                    let lon: Vec<f64> = p.vertices.iter().map(|v| v[1]).collect();
                    if lat.iter().any(|l| l.abs() > 90.0) {
                        return Err(format!(
                            "polygon {:?}: latitude out of range (vertices are [lat, lon])",
                            p.name
                        ));
                    }
                    let bbox = (
                        lat.iter().cloned().fold(f64::INFINITY, f64::min),
                        lat.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    );
                    polys.push(Poly {
                        ground,
                        lat,
                        lon,
                        bbox,
                    });
                }
                Ok(ConductivityMap::Polygons {
                    background: *background,
                    polys,
                })
            }
            ConductivityConfig::Raster {
                lat0_deg,
                lon0_deg,
                dlat_deg,
                dlon_deg,
                nlat,
                nlon,
                sigma_s_m,
                eps_r,
                file,
                background,
            } => {
                background.validate()?;
                if !(*dlat_deg > 0.0 && *dlon_deg > 0.0) || *nlat == 0 || *nlon == 0 {
                    return Err(
                        "conductivity raster: dlat/dlon must be positive, nlat/nlon ≥ 1".into(),
                    );
                }
                let (sig, eps) = match file {
                    Some(f) => {
                        let text = std::fs::read_to_string(f)
                            .map_err(|e| format!("conductivity raster {f}: {e}"))?;
                        if f.to_ascii_lowercase().ends_with(".json") {
                            let g: JsonGrid = serde_json::from_str(&text)
                                .map_err(|e| format!("conductivity raster {f}: {e}"))?;
                            (g.sigma_s_m, g.eps_r)
                        } else {
                            (parse_csv(&text)?, Vec::new())
                        }
                    }
                    None => (sigma_s_m.clone(), eps_r.clone()),
                };
                let n = nlat * nlon;
                if sig.len() != n {
                    return Err(format!(
                        "conductivity raster: {} σ values for {nlat}×{nlon} cells",
                        sig.len()
                    ));
                }
                if !eps.is_empty() && eps.len() != n {
                    return Err(format!(
                        "conductivity raster: {} ε_r values for {n} cells",
                        eps.len()
                    ));
                }
                let mut cells = Vec::with_capacity(n);
                for (i, &sv) in sig.iter().enumerate() {
                    let g = GroundConst {
                        sigma_s_m: sv,
                        eps_r: eps.get(i).copied(),
                    };
                    g.validate().map_err(|e| format!("raster cell {i}: {e}"))?;
                    let e = g.eps();
                    cells.push(GroundConst {
                        sigma_s_m: quantise_sigma(g.sigma_s_m),
                        eps_r: Some((e * 2.0).round() / 2.0),
                    });
                }
                Ok(ConductivityMap::Raster {
                    lat0: *lat0_deg,
                    lon0: *lon0_deg,
                    dlat: *dlat_deg,
                    dlon: *dlon_deg,
                    nlat: *nlat,
                    nlon: *nlon,
                    cells,
                    background: *background,
                })
            }
        }
    }

    /// Ground at a point.
    pub fn at(&self, p: Geo) -> GroundConst {
        match self {
            ConductivityMap::Uniform(g) => *g,
            ConductivityMap::Polygons { background, polys } => polys
                .iter()
                .rev()
                .find(|q| q.contains(p))
                .map(|q| q.ground)
                .unwrap_or(*background),
            ConductivityMap::Raster {
                lat0,
                lon0,
                dlat,
                dlon,
                nlat,
                nlon,
                cells,
                background,
            } => {
                let i = ((p.lat_deg - lat0) / dlat).floor();
                let dl = (p.lon_deg - lon0 + 540.0).rem_euclid(360.0) - 180.0;
                let j = (dl / dlon).floor();
                if i < 0.0 || j < 0.0 || i >= *nlat as f64 || j >= *nlon as f64 {
                    *background
                } else {
                    cells[i as usize * nlon + j as usize]
                }
            }
        }
    }

    pub fn is_uniform(&self) -> Option<GroundConst> {
        match self {
            ConductivityMap::Uniform(g) => Some(*g),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygons_point_in_polygon_and_precedence() {
        let cfg: ConductivityConfig = toml::from_str(
            r#"
type = "polygons"
background = { sigma_s_m = 5.0, eps_r = 80.0 }
[[polygons]]
name = "land"
sigma_s_m = 0.003
vertices = [[50.0, 0.0], [52.0, 0.0], [52.0, 4.0], [50.0, 4.0]]
[[polygons]]
name = "lake"
sigma_s_m = 0.01
eps_r = 80.0
vertices = [[50.5, 1.0], [51.0, 1.0], [51.0, 2.0], [50.5, 2.0]]
[[polygons]]
name = "dateline"
sigma_s_m = 0.002
vertices = [[10.0, 179.0], [12.0, 179.0], [12.0, -179.0], [10.0, -179.0]]
"#,
        )
        .unwrap();
        let m = ConductivityMap::build(&cfg).unwrap();
        assert_eq!(m.at(Geo::new(49.0, 1.0)).sigma_s_m, 5.0);
        let g = m.at(Geo::new(51.5, 3.0));
        assert_eq!(g.sigma_s_m, 0.003);
        assert_eq!(g.eps(), 15.0);
        assert_eq!(m.at(Geo::new(50.7, 1.5)).sigma_s_m, 0.01);
        assert_eq!(m.at(Geo::new(11.0, 179.5)).sigma_s_m, 0.002);
        assert_eq!(m.at(Geo::new(11.0, -179.5)).sigma_s_m, 0.002);
        assert_eq!(m.at(Geo::new(11.0, 178.5)).sigma_s_m, 5.0);
    }

    #[test]
    fn raster_inline_csv_and_json() {
        let cfg = ConductivityConfig::Raster {
            lat0_deg: 50.0,
            lon0_deg: 0.0,
            dlat_deg: 1.0,
            dlon_deg: 1.0,
            nlat: 2,
            nlon: 3,
            sigma_s_m: vec![0.001, 0.002, 0.003, 0.01, 0.02, 5.0],
            eps_r: vec![],
            file: None,
            background: GroundConst::seawater(),
        };
        let m = ConductivityMap::build(&cfg).unwrap();
        assert!((m.at(Geo::new(50.5, 1.5)).sigma_s_m - 0.002).abs() < 0.0002);
        assert!((m.at(Geo::new(51.5, 0.5)).sigma_s_m - 0.01).abs() < 0.001);
        assert_eq!(m.at(Geo::new(49.5, 0.5)).sigma_s_m, 5.0);
        assert_eq!(m.at(Geo::new(50.5, 3.5)).sigma_s_m, 5.0);
        // files
        let dir = std::env::temp_dir().join(format!("kshana-eloran-cond-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("g.csv");
        std::fs::write(&csv, "# south row first\n0.001,0.002,0.003\n0.01,0.02,5\n").unwrap();
        let json = dir.join("g.json");
        std::fs::write(
            &json,
            r#"{"sigma_s_m":[0.001,0.002,0.003,0.01,0.02,5],"eps_r":[10,10,10,20,20,80]}"#,
        )
        .unwrap();
        for f in [&csv, &json] {
            let mut c = cfg.clone();
            if let ConductivityConfig::Raster {
                sigma_s_m, file, ..
            } = &mut c
            {
                sigma_s_m.clear();
                *file = Some(f.to_string_lossy().into_owned());
            }
            let m = ConductivityMap::build(&c).unwrap();
            assert!((m.at(Geo::new(51.5, 1.5)).sigma_s_m - 0.02).abs() < 0.002);
        }
        let mj = {
            let mut c = cfg.clone();
            if let ConductivityConfig::Raster {
                sigma_s_m, file, ..
            } = &mut c
            {
                sigma_s_m.clear();
                *file = Some(json.to_string_lossy().into_owned());
            }
            ConductivityMap::build(&c).unwrap()
        };
        assert_eq!(mj.at(Geo::new(51.5, 1.5)).eps(), 20.0);
        let _ = std::fs::remove_dir_all(&dir);
        // wrong size is an error
        let mut bad = cfg.clone();
        if let ConductivityConfig::Raster { sigma_s_m, .. } = &mut bad {
            sigma_s_m.pop();
        }
        assert!(ConductivityMap::build(&bad).is_err());
    }
}
