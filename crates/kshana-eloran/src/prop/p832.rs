// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Ground-conductivity atlas import: rasters and GeoJSON polygons of σ / ε_r or of ground
//! categories, turned into the crate's [`ConductivityMap`] (usable by the `millington`,
//! `terrain` and uncertainty models), with a per-class uncertainty.
//!
//! # Ground categories
//!
//! [`CLASSES`] lists the ground categories of ITU-R P.527 ("Electrical characteristics of
//! the surface of the Earth") with the (σ, ε_r) pairs of the ITU-R P.368 groundwave curves
//! (as recalled from P.368-9/P.527-3 — NEEDS CONFIRMATION against the current editions):
//! sea water of average salinity 5 S/m (ε_r = 70 in P.368; **80 here**, the LORAN seawater
//! reference of [`super::smooth_earth::SEAWATER`], so all-sea paths keep ASF = 0 — the
//! difference is immaterial at 100 kHz, |ε_c| ≈ 9·10⁵), sea water of low salinity
//! 1 S/m / 80, fresh water 3 mS/m / 80, land 30 mS/m / 40, wet ground 10 mS/m / 30, land
//! 3 mS/m / 22, medium-dry ground 1 mS/m / 15, dry ground 0.3 mS/m / 7, very dry ground
//! 0.1 mS/m / 3, fresh-water ice at −1 °C 30 µS/m / 3 and at −10 °C 10 µS/m / 3. ITU-R P.832
//! ("World Atlas of Ground Conductivities") maps zones of such σ values; its digitised
//! data are not bundled (licensing) and must be supplied as a file.
//!
//! The category **numbers** 1–11 of [`CLASSES`] are Kshana's own (P.832 has no code list);
//! a raster or GeoJSON can use them, the class names, or its own `categories` table.
//!
//! The per-class uncertainties (`sigma_log10_sd`: standard deviation of log₁₀ σ, i.e. 0.3 ≈
//! a factor of 2; `eps_rel_sd`: relative standard deviation of ε_r) are **engineering
//! defaults** (MODELLED), motivated by the factor-of-2–3 spread usually quoted for atlas
//! land conductivities; they are not taken from P.527/P.832. A `categories` entry can
//! override them.
//!
//! # Formats
//!
//! * Raster `.asc` (ESRI ASCII grid in degrees: the cells are the conductivity cells,
//!   `xllcorner`/`yllcorner` their south-west edge) or `.csv` (a header line naming `lat`,
//!   `lon` and one of `sigma_s_m`, `sigma_ms`, `category`, optionally `eps_r`; one line per
//!   cell centre on a regular grid; missing cells read as the background). Values ≤ 0 or
//!   NODATA read as the background. `values = "sigma_s_m" | "sigma_ms" | "category"` says
//!   what an `.asc` grid holds.
//! * GeoJSON (`FeatureCollection`, `Feature` or a bare geometry) of `Polygon` /
//!   `MultiPolygon` features, coordinates `[lon, lat]`, properties `sigma` or `sigma_s_m`
//!   (S/m), or `sigma_ms` (mS/m), or `category` (number or class name), and optionally
//!   `epsilon` / `eps_r`; `name` is kept. Later features take precedence (the polygon-map
//!   rule). Interior rings (holes) become polygons of the **background** ground listed
//!   right after their outer ring (so a feature listed later still covers a hole); edges
//!   are straight in lat/lon. A minimal parser on `serde_json::Value` (no GIS crate).

use super::conductivity::{ConductivityConfig, ConductivityMap, GroundConst, PolygonConfig};
use super::ground::{GroundConfig, TemporalConfig, K_LORAN};
use super::BuildContext;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A ground category with its electrical constants and uncertainty.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundClass {
    pub code: u32,
    pub name: String,
    pub sigma_s_m: f64,
    pub eps_r: f64,
    /// Standard deviation of log₁₀ σ.
    pub sigma_log10_sd: f64,
    /// Relative standard deviation of ε_r.
    pub eps_rel_sd: f64,
}

impl GroundClass {
    pub fn ground(&self) -> GroundConst {
        GroundConst::new(self.sigma_s_m, self.eps_r)
    }
}

/// (code, name, σ S/m, ε_r, sd log₁₀ σ, rel. sd ε_r) — see the module documentation.
pub const CLASSES: &[(u32, &str, f64, f64, f64, f64)] = &[
    (1, "sea", 5.0, 80.0, 0.05, 0.1),
    (2, "sea_low_salinity", 1.0, 80.0, 0.1, 0.1),
    (3, "fresh_water", 3e-3, 80.0, 0.3, 0.1),
    (4, "good_ground", 3e-2, 40.0, 0.3, 0.3),
    (5, "wet_ground", 1e-2, 30.0, 0.3, 0.3),
    (6, "land", 3e-3, 22.0, 0.3, 0.3),
    (7, "medium_dry_ground", 1e-3, 15.0, 0.3, 0.3),
    (8, "dry_ground", 3e-4, 7.0, 0.4, 0.4),
    (9, "very_dry_ground", 1e-4, 3.0, 0.5, 0.5),
    (10, "ice_minus_1c", 3e-5, 3.0, 0.5, 0.3),
    (11, "ice_minus_10c", 1e-5, 3.0, 0.5, 0.3),
];

/// The built-in classes.
pub fn builtin_classes() -> Vec<GroundClass> {
    CLASSES
        .iter()
        .map(|&(code, name, s, e, sd, esd)| GroundClass {
            code,
            name: name.into(),
            sigma_s_m: s,
            eps_r: e,
            sigma_log10_sd: sd,
            eps_rel_sd: esd,
        })
        .collect()
}

/// A user category definition (overrides or adds a class).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryDef {
    pub sigma_s_m: f64,
    #[serde(default)]
    pub eps_r: Option<f64>,
    #[serde(default)]
    pub sigma_log10_sd: Option<f64>,
    #[serde(default)]
    pub eps_rel_sd: Option<f64>,
}

/// What raster values mean.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RasterValues {
    #[default]
    SigmaSM,
    SigmaMs,
    Category,
}

/// Atlas import configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AtlasConfig {
    Raster {
        file: String,
        #[serde(default)]
        values: RasterValues,
        #[serde(default = "GroundConst::seawater")]
        background: GroundConst,
        #[serde(default)]
        categories: BTreeMap<String, CategoryDef>,
    },
    Geojson {
        file: String,
        #[serde(default = "GroundConst::seawater")]
        background: GroundConst,
        #[serde(default)]
        categories: BTreeMap<String, CategoryDef>,
    },
}

/// An imported atlas: the map, its equivalent inline configuration and its classes.
#[derive(Clone, Debug)]
pub struct Atlas {
    pub map: ConductivityMap,
    pub config: ConductivityConfig,
    pub classes: Vec<GroundClass>,
}

impl Atlas {
    /// The class nearest to a ground (in log σ, then log ε_r) — its uncertainty applies.
    pub fn class_of(&self, g: GroundConst) -> &GroundClass {
        nearest_class(&self.classes, g)
    }
}

/// Nearest class to `g` among `classes` (distance in log₁₀ σ plus ¼ of log₁₀ ε_r).
pub fn nearest_class(classes: &[GroundClass], g: GroundConst) -> &GroundClass {
    let ls = g.sigma_s_m.max(1e-12).log10();
    let le = g.eps().max(1.0).log10();
    classes
        .iter()
        .min_by(|a, b| {
            let da = (a.sigma_s_m.log10() - ls).abs() + 0.25 * (a.eps_r.log10() - le).abs();
            let db = (b.sigma_s_m.log10() - ls).abs() + 0.25 * (b.eps_r.log10() - le).abs();
            da.total_cmp(&db)
        })
        .expect("at least one class")
}

/// Built-in classes with the user's category definitions merged in (a key that is a
/// built-in code or name replaces it, any other key adds a class).
pub fn merged_classes(cats: &BTreeMap<String, CategoryDef>) -> Result<Vec<GroundClass>, String> {
    let mut v = builtin_classes();
    let mut next = 1000;
    for (k, c) in cats {
        if !(c.sigma_s_m > 0.0) {
            return Err(format!("atlas category {k:?}: sigma_s_m must be positive"));
        }
        let gc = GroundConst {
            sigma_s_m: c.sigma_s_m,
            eps_r: c.eps_r,
        };
        let pos = v
            .iter()
            .position(|x| x.name == *k || k.parse::<u32>().ok() == Some(x.code));
        let base_sd = (0.3, 0.3);
        let code = k.parse::<u32>().unwrap_or_else(|_| {
            next += 1;
            next
        });
        let cls = GroundClass {
            code,
            name: k.clone(),
            sigma_s_m: c.sigma_s_m,
            eps_r: gc.eps(),
            sigma_log10_sd: c.sigma_log10_sd.unwrap_or(base_sd.0),
            eps_rel_sd: c.eps_rel_sd.unwrap_or(base_sd.1),
        };
        match pos {
            Some(i) => {
                let name = v[i].name.clone();
                v[i] = GroundClass {
                    name,
                    code: v[i].code,
                    ..cls
                };
            }
            None => v.push(cls),
        }
    }
    Ok(v)
}

fn class_by_key<'a>(classes: &'a [GroundClass], key: &str) -> Option<&'a GroundClass> {
    let key = key.trim();
    if let Ok(c) = key.parse::<f64>() {
        if c.fract() == 0.0 && c >= 0.0 {
            return classes.iter().find(|x| x.code == c as u32);
        }
    }
    classes.iter().find(|x| x.name.eq_ignore_ascii_case(key))
}

/// Ground of one raster value (`None` = background).
fn ground_of_value(
    v: f64,
    eps: Option<f64>,
    kind: RasterValues,
    classes: &[GroundClass],
) -> Result<Option<GroundConst>, String> {
    if !v.is_finite() || v <= 0.0 {
        return Ok(None);
    }
    Ok(Some(match kind {
        RasterValues::SigmaSM => GroundConst {
            sigma_s_m: v,
            eps_r: eps,
        },
        RasterValues::SigmaMs => GroundConst {
            sigma_s_m: v * 1e-3,
            eps_r: eps,
        },
        RasterValues::Category => {
            let c = class_by_key(classes, &format!("{v}"))
                .ok_or_else(|| format!("atlas: unknown category {v}"))?;
            GroundConst {
                sigma_s_m: c.sigma_s_m,
                eps_r: Some(eps.unwrap_or(c.eps_r)),
            }
        }
    }))
}

/// A raster from an ESRI ASCII grid (cells = conductivity cells).
pub fn raster_from_asc(
    text: &str,
    kind: RasterValues,
    background: GroundConst,
    classes: &[GroundClass],
) -> Result<ConductivityConfig, String> {
    // the DEM parser gives node = cell centre, rows south first
    let g = super::dem::parse_asc(text)?;
    let mut sig = Vec::with_capacity(g.nlat * g.nlon);
    let mut eps = Vec::with_capacity(g.nlat * g.nlon);
    for i in 0..g.nlat {
        for j in 0..g.nlon {
            let p = crate::geo::Geo::new(g.lat0 + i as f64 * g.dlat, g.lon0 + j as f64 * g.dlon);
            let v = g.sample(p).unwrap_or(f64::NAN);
            let gc = ground_of_value(v, None, kind, classes)?.unwrap_or(background);
            sig.push(gc.sigma_s_m);
            eps.push(gc.eps());
        }
    }
    Ok(ConductivityConfig::Raster {
        lat0_deg: g.lat0 - g.dlat / 2.0,
        lon0_deg: g.lon0 - g.dlon / 2.0,
        dlat_deg: g.dlat,
        dlon_deg: g.dlon,
        nlat: g.nlat,
        nlon: g.nlon,
        sigma_s_m: sig,
        eps_r: eps,
        file: None,
        background,
    })
}

/// A raster from a CSV of cell centres (`lat,lon,<value>[,eps_r]` with a header line).
pub fn raster_from_csv(
    text: &str,
    background: GroundConst,
    classes: &[GroundClass],
) -> Result<ConductivityConfig, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines
        .next()
        .ok_or("atlas CSV: empty")?
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
    let ilat = col(&["lat", "lat_deg", "latitude"]).ok_or("atlas CSV: no lat column")?;
    let ilon = col(&["lon", "lon_deg", "longitude"]).ok_or("atlas CSV: no lon column")?;
    let (ival, kind) = if let Some(i) = col(&["sigma_s_m", "sigma"]) {
        (i, RasterValues::SigmaSM)
    } else if let Some(i) = col(&["sigma_ms", "sigma_ms_m"]) {
        (i, RasterValues::SigmaMs)
    } else if let Some(i) = col(&["category", "class"]) {
        (i, RasterValues::Category)
    } else {
        return Err("atlas CSV: needs a sigma_s_m, sigma_ms or category column".into());
    };
    let ieps = col(&["eps_r", "epsilon", "epsilon_r"]);
    let mut pts: Vec<(f64, f64, GroundConst)> = Vec::new();
    for (n, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let num = |i: usize| -> Result<f64, String> {
            f.get(i)
                .ok_or_else(|| format!("atlas CSV row {}: missing field", n + 1))?
                .parse::<f64>()
                .map_err(|e| format!("atlas CSV row {}: {e}", n + 1))
        };
        let (lat, lon) = (num(ilat)?, num(ilon)?);
        let eps = match ieps {
            Some(i) if f.get(i).is_some_and(|s| !s.is_empty()) => Some(num(i)?),
            _ => None,
        };
        let g = if kind == RasterValues::Category {
            let key = f.get(ival).copied().unwrap_or("");
            let c = class_by_key(classes, key)
                .ok_or_else(|| format!("atlas CSV row {}: unknown category {key:?}", n + 1))?;
            Some(GroundConst {
                sigma_s_m: c.sigma_s_m,
                eps_r: Some(eps.unwrap_or(c.eps_r)),
            })
        } else {
            ground_of_value(num(ival)?, eps, kind, classes)?
        };
        if let Some(g) = g {
            pts.push((lat, lon, g));
        }
    }
    if pts.is_empty() {
        return Err("atlas CSV: no cells".into());
    }
    let axis = |vals: Vec<f64>| -> Result<(f64, f64, usize), String> {
        let mut u = vals;
        u.sort_by(f64::total_cmp);
        u.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        if u.len() == 1 {
            return Ok((u[0], 1.0, 1));
        }
        let step = (1..u.len())
            .map(|i| u[i] - u[i - 1])
            .fold(f64::INFINITY, f64::min);
        let n = ((u[u.len() - 1] - u[0]) / step).round() as usize + 1;
        for v in &u {
            let k = (v - u[0]) / step;
            if (k - k.round()).abs() > 1e-6 {
                return Err("atlas CSV: cells are not on a regular grid".into());
            }
        }
        Ok((u[0], step, n))
    };
    let (lat_min, dlat, nlat) = axis(pts.iter().map(|p| p.0).collect())?;
    let (lon_min, dlon, nlon) = axis(pts.iter().map(|p| p.1).collect())?;
    let mut sig = vec![background.sigma_s_m; nlat * nlon];
    let mut eps = vec![background.eps(); nlat * nlon];
    for (lat, lon, g) in pts {
        let i = ((lat - lat_min) / dlat).round() as usize;
        let j = ((lon - lon_min) / dlon).round() as usize;
        sig[i * nlon + j] = g.sigma_s_m;
        eps[i * nlon + j] = g.eps();
    }
    Ok(ConductivityConfig::Raster {
        lat0_deg: lat_min - dlat / 2.0,
        lon0_deg: lon_min - dlon / 2.0,
        dlat_deg: dlat,
        dlon_deg: dlon,
        nlat,
        nlon,
        sigma_s_m: sig,
        eps_r: eps,
        file: None,
        background,
    })
}

fn prop_f64(p: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| p.get(*k).and_then(Value::as_f64))
}

fn feature_ground(
    props: &Value,
    classes: &[GroundClass],
    idx: usize,
) -> Result<GroundConst, String> {
    let eps = prop_f64(props, &["epsilon", "eps_r", "epsilon_r"]);
    if let Some(s) = prop_f64(props, &["sigma", "sigma_s_m"]) {
        return Ok(GroundConst {
            sigma_s_m: s,
            eps_r: eps,
        });
    }
    if let Some(s) = prop_f64(props, &["sigma_ms", "sigma_mS_m"]) {
        return Ok(GroundConst {
            sigma_s_m: s * 1e-3,
            eps_r: eps,
        });
    }
    if let Some(c) = props.get("category") {
        let key = match c {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        let cls = class_by_key(classes, &key)
            .ok_or_else(|| format!("GeoJSON feature {idx}: unknown category {key:?}"))?;
        return Ok(GroundConst {
            sigma_s_m: cls.sigma_s_m,
            eps_r: Some(eps.unwrap_or(cls.eps_r)),
        });
    }
    Err(format!(
        "GeoJSON feature {idx}: needs a sigma, sigma_s_m, sigma_ms or category property"
    ))
}

fn ring(v: &Value, idx: usize) -> Result<Vec<[f64; 2]>, String> {
    let pts = v
        .as_array()
        .ok_or_else(|| format!("GeoJSON feature {idx}: ring is not an array"))?;
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for p in pts {
        let a = p
            .as_array()
            .filter(|a| a.len() >= 2)
            .ok_or_else(|| format!("GeoJSON feature {idx}: a position must be [lon, lat]"))?;
        let lon = a[0].as_f64().ok_or("GeoJSON: non-numeric longitude")?;
        let lat = a[1].as_f64().ok_or("GeoJSON: non-numeric latitude")?;
        out.push([lat, lon]);
    }
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    if out.len() < 3 {
        return Err(format!(
            "GeoJSON feature {idx}: a ring needs at least 3 positions"
        ));
    }
    Ok(out)
}

/// Polygons from a GeoJSON text (see the module documentation).
pub fn polygons_from_geojson(
    text: &str,
    background: GroundConst,
    classes: &[GroundClass],
) -> Result<ConductivityConfig, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("GeoJSON: {e}"))?;
    let features: Vec<Value> = match root.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => root
            .get("features")
            .and_then(Value::as_array)
            .cloned()
            .ok_or("GeoJSON: FeatureCollection without features")?,
        Some("Feature") => vec![root.clone()],
        Some("Polygon") | Some("MultiPolygon") => {
            vec![serde_json::json!({"type": "Feature", "geometry": root, "properties": {}})]
        }
        t => return Err(format!("GeoJSON: unsupported root type {t:?}")),
    };
    let mut polys = Vec::new();
    for (idx, f) in features.iter().enumerate() {
        let props = f.get("properties").cloned().unwrap_or(Value::Null);
        let geom = match f.get("geometry") {
            Some(g) if !g.is_null() => g,
            _ => continue,
        };
        let name = props
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let gtype = geom.get("type").and_then(Value::as_str).unwrap_or("");
        let coords = geom
            .get("coordinates")
            .ok_or_else(|| format!("GeoJSON feature {idx}: no coordinates"))?;
        let polygons: Vec<&Value> = match gtype {
            "Polygon" => vec![coords],
            "MultiPolygon" => coords
                .as_array()
                .ok_or_else(|| format!("GeoJSON feature {idx}: bad MultiPolygon"))?
                .iter()
                .collect(),
            _ => continue, // points/lines carry no area
        };
        let g = feature_ground(&props, classes, idx)?;
        for p in polygons {
            let rings = p
                .as_array()
                .ok_or_else(|| format!("GeoJSON feature {idx}: bad Polygon"))?;
            for (ri, r) in rings.iter().enumerate() {
                let (ground, nm) = if ri == 0 {
                    (g, name.clone())
                } else {
                    (background, format!("{name} (hole {ri})"))
                };
                polys.push(PolygonConfig {
                    name: nm,
                    sigma_s_m: ground.sigma_s_m,
                    eps_r: ground.eps_r,
                    vertices: ring(r, idx)?,
                });
            }
        }
    }
    Ok(ConductivityConfig::Polygons {
        background,
        polygons: polys,
    })
}

/// Import an atlas (reads its file).
pub fn load_atlas(cfg: &AtlasConfig) -> Result<Atlas, String> {
    let (config, classes) = match cfg {
        AtlasConfig::Raster {
            file,
            values,
            background,
            categories,
        } => {
            let classes = merged_classes(categories)?;
            let text = std::fs::read_to_string(file).map_err(|e| format!("atlas {file}: {e}"))?;
            let c = if file.to_ascii_lowercase().ends_with(".asc") {
                raster_from_asc(&text, *values, *background, &classes)
            } else {
                raster_from_csv(&text, *background, &classes)
            }
            .map_err(|e| format!("atlas {file}: {e}"))?;
            (c, classes)
        }
        AtlasConfig::Geojson {
            file,
            background,
            categories,
        } => {
            let classes = merged_classes(categories)?;
            let text = std::fs::read_to_string(file).map_err(|e| format!("atlas {file}: {e}"))?;
            let c = polygons_from_geojson(&text, *background, &classes)
                .map_err(|e| format!("atlas {file}: {e}"))?;
            (c, classes)
        }
    };
    let map = ConductivityMap::build(&config)?;
    Ok(Atlas {
        map,
        config,
        classes,
    })
}

/// `model = "millington_atlas"`: Millington over an imported atlas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtlasGroundConfig {
    pub atlas: AtlasConfig,
    #[serde(default = "d_k")]
    pub k_factor: f64,
    #[serde(default = "d_sample")]
    pub sample_km: f64,
    #[serde(default = "d_grid")]
    pub asf_grid_deg: f64,
    #[serde(default)]
    pub temporal: Option<TemporalConfig>,
}

fn d_k() -> f64 {
    K_LORAN
}
fn d_sample() -> f64 {
    2.0
}
fn d_grid() -> f64 {
    0.005
}

/// Build the `millington_atlas` model: the atlas is imported and the result is exactly a
/// `millington` model over the equivalent inline conductivity configuration.
pub fn build(
    cfg: &AtlasGroundConfig,
    ctx: &BuildContext,
) -> Result<Box<dyn super::PropagationModel>, String> {
    let atlas = load_atlas(&cfg.atlas)?;
    super::ground::build(
        &GroundConfig::Millington {
            conductivity: atlas.config,
            k_factor: cfg.k_factor,
            sample_km: cfg.sample_km,
            asf_grid_deg: cfg.asf_grid_deg,
            temporal: cfg.temporal.clone(),
        },
        ctx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::Geo;

    #[test]
    fn classes_and_nearest() {
        let c = builtin_classes();
        assert_eq!(c.len(), 11);
        assert_eq!(
            nearest_class(&c, GroundConst::new(0.0028, 20.0)).name,
            "land"
        );
        assert_eq!(nearest_class(&c, GroundConst::seawater()).name, "sea");
        assert_eq!(
            nearest_class(&c, GroundConst::new(0.003, 80.0)).name,
            "fresh_water"
        );
        let mut cats = BTreeMap::new();
        cats.insert(
            "land".to_string(),
            CategoryDef {
                sigma_s_m: 0.004,
                eps_r: Some(20.0),
                sigma_log10_sd: Some(0.2),
                eps_rel_sd: None,
            },
        );
        cats.insert(
            "42".to_string(),
            CategoryDef {
                sigma_s_m: 0.05,
                eps_r: None,
                sigma_log10_sd: None,
                eps_rel_sd: None,
            },
        );
        let m = merged_classes(&cats).unwrap();
        let land = m.iter().find(|x| x.name == "land").unwrap();
        assert_eq!(
            (land.code, land.sigma_s_m, land.sigma_log10_sd),
            (6, 0.004, 0.2)
        );
        assert_eq!(class_by_key(&m, "42").unwrap().sigma_s_m, 0.05);
        assert_eq!(class_by_key(&m, "7").unwrap().name, "medium_dry_ground");
    }

    #[test]
    fn geojson_polygons_multipolygons_holes_and_properties() {
        let gj = r#"{
 "type": "FeatureCollection",
 "features": [
  {"type": "Feature", "properties": {"name": "island", "sigma": 0.003, "epsilon": 15},
   "geometry": {"type": "Polygon", "coordinates": [
     [[0,50],[4,50],[4,52],[0,52],[0,50]],
     [[1,50.5],[2,50.5],[2,51],[1,51],[1,50.5]]]}},
  {"type": "Feature", "properties": {"category": "dry_ground"},
   "geometry": {"type": "MultiPolygon", "coordinates": [
     [[[10,10],[11,10],[11,11],[10,11]]],
     [[[20,10],[21,10],[21,11],[20,11]]]]}},
  {"type": "Feature", "properties": {"sigma_ms": 10},
   "geometry": {"type": "Polygon", "coordinates": [[[3,51],[3.5,51],[3.5,51.5],[3,51.5]]]}},
  {"type": "Feature", "properties": {}, "geometry": {"type": "Point", "coordinates": [0,0]}}
 ]}"#;
        let cfg = polygons_from_geojson(gj, GroundConst::seawater(), &builtin_classes()).unwrap();
        let m = ConductivityMap::build(&cfg).unwrap();
        assert_eq!(m.at(Geo::new(51.5, 0.5)).sigma_s_m, 0.003);
        assert_eq!(m.at(Geo::new(51.5, 0.5)).eps(), 15.0);
        assert_eq!(
            m.at(Geo::new(50.7, 1.5)).sigma_s_m,
            5.0,
            "hole = background"
        );
        assert_eq!(m.at(Geo::new(49.0, 1.5)).sigma_s_m, 5.0);
        assert_eq!(m.at(Geo::new(10.5, 10.5)).sigma_s_m, 3e-4);
        assert_eq!(m.at(Geo::new(10.5, 20.5)).eps(), 7.0);
        assert!(
            (m.at(Geo::new(51.2, 3.2)).sigma_s_m - 0.01).abs() < 1e-12,
            "later wins"
        );
        // errors
        let bad = r#"{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1]]]}}"#;
        assert!(polygons_from_geojson(bad, GroundConst::seawater(), &builtin_classes()).is_err());
        let bare = r#"{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1]]]}"#;
        assert!(polygons_from_geojson(bare, GroundConst::seawater(), &builtin_classes()).is_err());
        assert!(polygons_from_geojson("{", GroundConst::seawater(), &builtin_classes()).is_err());
    }

    #[test]
    fn rasters_csv_and_asc_with_categories() {
        let classes = builtin_classes();
        let csv = "lat,lon,category\n50.25,0.25,land\n50.25,0.75,7\n50.75,0.25,sea\n";
        let cfg = raster_from_csv(csv, GroundConst::seawater(), &classes).unwrap();
        let m = ConductivityMap::build(&cfg).unwrap();
        assert!((m.at(Geo::new(50.3, 0.3)).sigma_s_m - 0.003).abs() < 2e-4);
        assert!((m.at(Geo::new(50.3, 0.9)).sigma_s_m - 0.001).abs() < 1e-4);
        assert!(
            (m.at(Geo::new(50.6, 0.6)).sigma_s_m - 5.0).abs() < 0.03,
            "missing cell = background (σ quantised to 1/50 decade)"
        );
        assert_eq!(m.at(Geo::new(50.3, 0.3)).eps(), 22.0);
        let csv2 = "lat,lon,sigma_ms,eps_r\n10,20,3,12\n10,20.5,30,\n";
        let m2 = ConductivityMap::build(
            &raster_from_csv(csv2, GroundConst::seawater(), &classes).unwrap(),
        )
        .unwrap();
        assert!((m2.at(Geo::new(10.1, 20.1)).sigma_s_m - 0.003).abs() < 2e-4);
        assert_eq!(m2.at(Geo::new(10.1, 20.1)).eps(), 12.0);
        assert!((m2.at(Geo::new(10.1, 20.6)).sigma_s_m - 0.03).abs() < 2e-3);
        assert!(raster_from_csv(
            "lat,lon,sigma\n0,0,1\n0,0.3,1\n0,1,1\n",
            GroundConst::seawater(),
            &classes
        )
        .is_err());
        let asc =
            "ncols 2\nnrows 2\nxllcorner 0\nyllcorner 50\ncellsize 1\nNODATA_value -1\n6 9\n-1 3\n";
        let m3 = ConductivityMap::build(
            &raster_from_asc(
                asc,
                RasterValues::Category,
                GroundConst::seawater(),
                &classes,
            )
            .unwrap(),
        )
        .unwrap();
        assert!((m3.at(Geo::new(51.5, 0.5)).sigma_s_m - 0.003).abs() < 2e-4); // north row: land
        assert!((m3.at(Geo::new(51.5, 1.5)).sigma_s_m - 1e-4).abs() < 1e-5);
        assert!((m3.at(Geo::new(50.5, 0.5)).sigma_s_m - 5.0).abs() < 0.03); // NODATA
        assert!((m3.at(Geo::new(50.5, 1.5)).sigma_s_m - 0.003).abs() < 2e-4); // fresh water
        assert_eq!(m3.at(Geo::new(50.5, 1.5)).eps(), 80.0);
    }

    #[test]
    fn atlas_files_drive_millington() {
        let dir = std::env::temp_dir().join(format!("kshana-p832-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("land.geojson");
        std::fs::write(
            &f,
            r#"{"type":"Feature","properties":{"category":6},"geometry":{"type":"Polygon","coordinates":[[[-10,51],[10,51],[10,60],[-10,60]]]}}"#,
        )
        .unwrap();
        let toml_src = format!(
            "model = \"millington_atlas\"\n[atlas]\ntype = \"geojson\"\nfile = {:?}\n",
            f.to_string_lossy()
        );
        let cfg: GroundConfig = toml::from_str(&toml_src).unwrap();
        let ctx = BuildContext::new(
            crate::time::AbsTime::from_utc_iso("2026-01-01T00:00:00Z").unwrap(),
            1.0,
            0,
        );
        let m = super::super::ground::build(&cfg, &ctx).unwrap();
        let st = crate::chain::Station {
            id: "M".into(),
            name: "M".into(),
            role: crate::signal::Role::Master,
            position: Geo::new(49.0, 0.0),
            power_kw: 1.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        };
        let rx = crate::traj::RxState {
            t_s: 0.0,
            pos: Geo::new(53.0, 0.0),
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        let p = m.paths(&st, &rx)[0];
        assert!(p.asf_ns > 50.0, "mixed path ASF {}", p.asf_ns);
        let a = load_atlas(&AtlasConfig::Geojson {
            file: f.to_string_lossy().into(),
            background: GroundConst::seawater(),
            categories: BTreeMap::new(),
        })
        .unwrap();
        assert_eq!(
            a.class_of(a.map.at(Geo::new(55.0, 0.0))).sigma_log10_sd,
            0.3
        );
        assert!(load_atlas(&AtlasConfig::Geojson {
            file: "/nonexistent.geojson".into(),
            background: GroundConst::seawater(),
            categories: BTreeMap::new(),
        })
        .is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
