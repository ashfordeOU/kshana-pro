// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Digital elevation models (DEM) for the terrain groundwave model ([`super::terrain`]):
//! readers, bilinear sampling and terrain profiles along geodesics.
//!
//! Sources (`[propagation.ground.dem] type = …`):
//!
//! * `flat`: constant height (default 0 m) — the smooth-earth reference.
//! * `srtm`: a directory of SRTM `.hgt` tiles (1″: 3601×3601, 3″: 1201×1201 samples,
//!   detected from the file size). Big-endian `i16` metres, rows from north to south, the
//!   tile `N45E006.hgt` spanning 45–46° N, 6–7° E with its edge rows/columns on the
//!   integer degrees (node-registered, edges shared with the neighbours). The void value
//!   −32768 is treated as missing. Tiles are loaded on first use and kept in a small
//!   cache (`max_tiles`, default 16, oldest evicted). A missing tile (SRTM has no tiles
//!   over the open sea) reads as `void_fill_m` (default 0 m = sea level).
//! * `asc`: an ESRI ASCII grid (`ncols`, `nrows`, `xllcorner|xllcenter`,
//!   `yllcorner|yllcenter`, `cellsize`, optional `NODATA_value`, then `nrows` rows from
//!   north to south) in degrees of longitude/latitude.
//! * `csv_grid`: a regular grid given by its geometry in the configuration (`lat0_deg`,
//!   `lon0_deg` of node (0, 0), `dlat_deg`, `dlon_deg`) and a CSV file of `nlat` lines of
//!   `nlon` heights, **south row first** (the layout of the conductivity raster); `#`
//!   comments; empty fields or `nan` are voids.
//! * `hills`: analytic Gaussian hills on a base height (synthetic terrain for tests and
//!   what-if studies): `h = base + Σ Hᵢ·exp(−(rᵢ/Rᵢ)²)`, `rᵢ` the geodesic distance to the
//!   hill centre.
//!
//! Sampling is bilinear between the four surrounding nodes; void nodes are left out and the
//! remaining weights renormalised; four voids give `None` (the caller fills with
//! `void_fill_m`). Profiles ([`profile`]) sample the DEM at equally spaced points on the
//! WGS-84 geodesic. Heights are taken as given (SRTM: EGM96 geoid heights); the geoid–
//! ellipsoid separation is irrelevant to the groundwave, which only sees height
//! differences along the path.

use crate::geo::{inverse, path_points, Geo};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A source of terrain heights.
pub trait ElevationSource: Send + Sync {
    /// Height (m) at `p`, `None` where the source has no data (void or outside).
    fn height_m(&self, p: Geo) -> Option<f64>;
    /// Short description for model names and logs.
    fn describe(&self) -> String;
}

/// Storage of a regular grid.
#[derive(Clone, Debug)]
enum Heights {
    /// SRTM samples, void = −32768.
    I16(Vec<i16>),
    /// Heights, NaN = void.
    F32(Vec<f32>),
}

/// A node-registered regular lat/lon grid: node `(i, j)` at
/// `(lat0 + i·dlat, lon0 + j·dlon)`, row `i = 0` the southernmost.
#[derive(Clone, Debug)]
pub struct Grid {
    pub lat0: f64,
    pub lon0: f64,
    pub dlat: f64,
    pub dlon: f64,
    pub nlat: usize,
    pub nlon: usize,
    data: Heights,
}

impl Grid {
    /// A grid from heights (row 0 = south, west to east); NaN marks a void.
    pub fn from_f32(
        lat0: f64,
        lon0: f64,
        dlat: f64,
        dlon: f64,
        nlat: usize,
        nlon: usize,
        data: Vec<f32>,
    ) -> Result<Grid, String> {
        if !(dlat > 0.0 && dlon > 0.0) || nlat < 2 || nlon < 2 {
            return Err("DEM grid: dlat/dlon must be positive and the grid at least 2×2".into());
        }
        if data.len() != nlat * nlon {
            return Err(format!(
                "DEM grid: {} values for {nlat}×{nlon} nodes",
                data.len()
            ));
        }
        Ok(Grid {
            lat0,
            lon0,
            dlat,
            dlon,
            nlat,
            nlon,
            data: Heights::F32(data),
        })
    }

    #[inline]
    fn node(&self, i: usize, j: usize) -> Option<f64> {
        let k = i * self.nlon + j;
        match &self.data {
            Heights::I16(v) => {
                let x = v[k];
                if x == SRTM_VOID {
                    None
                } else {
                    Some(x as f64)
                }
            }
            Heights::F32(v) => {
                let x = v[k];
                if x.is_nan() {
                    None
                } else {
                    Some(x as f64)
                }
            }
        }
    }

    /// Bilinear height at `p` (void nodes left out), `None` outside the grid or when all
    /// four nodes are void.
    pub fn sample(&self, p: Geo) -> Option<f64> {
        let fi = (p.lat_deg - self.lat0) / self.dlat;
        let dl = (p.lon_deg - self.lon0 + 540.0).rem_euclid(360.0) - 180.0;
        let fj = dl / self.dlon;
        let (imax, jmax) = ((self.nlat - 1) as f64, (self.nlon - 1) as f64);
        let eps = 1e-9;
        if fi < -eps || fj < -eps || fi > imax + eps || fj > jmax + eps {
            return None;
        }
        let fi = fi.clamp(0.0, imax);
        let fj = fj.clamp(0.0, jmax);
        let i0 = (fi.floor() as usize).min(self.nlat - 2);
        let j0 = (fj.floor() as usize).min(self.nlon - 2);
        let (u, v) = (fi - i0 as f64, fj - j0 as f64);
        let mut acc = 0.0;
        let mut wsum = 0.0;
        for (di, dj, w) in [
            (0, 0, (1.0 - u) * (1.0 - v)),
            (1, 0, u * (1.0 - v)),
            (0, 1, (1.0 - u) * v),
            (1, 1, u * v),
        ] {
            if w <= 0.0 {
                continue;
            }
            if let Some(h) = self.node(i0 + di, j0 + dj) {
                acc += w * h;
                wsum += w;
            }
        }
        if wsum > 1e-12 {
            Some(acc / wsum)
        } else {
            // exactly on a node whose neighbours carry zero weight
            let (ii, jj) = (fi.round() as usize, fj.round() as usize);
            self.node(ii, jj)
        }
    }
}

/// SRTM void value.
pub const SRTM_VOID: i16 = -32768;

impl ElevationSource for Grid {
    fn height_m(&self, p: Geo) -> Option<f64> {
        self.sample(p)
    }
    fn describe(&self) -> String {
        format!(
            "grid {}×{} from ({}, {}) step ({}, {})°",
            self.nlat, self.nlon, self.lat0, self.lon0, self.dlat, self.dlon
        )
    }
}

/// Parse one SRTM `.hgt` tile whose south-west corner is at (`lat_sw`, `lon_sw`) degrees.
pub fn parse_hgt(bytes: &[u8], lat_sw: i32, lon_sw: i32) -> Result<Grid, String> {
    let n = match bytes.len() {
        l if l == 3601 * 3601 * 2 => 3601,
        l if l == 1201 * 1201 * 2 => 1201,
        l => {
            // any square tile (tests use small synthetic ones)
            let s = ((l / 2) as f64).sqrt().round() as usize;
            if s >= 2 && s * s * 2 == l {
                s
            } else {
                return Err(format!(
                    "SRTM tile: {l} bytes is not a square grid of i16 samples"
                ));
            }
        }
    };
    let mut v = vec![0i16; n * n];
    // file rows run north → south; grid row 0 is south
    for r in 0..n {
        let i = n - 1 - r;
        for j in 0..n {
            let k = 2 * (r * n + j);
            v[i * n + j] = i16::from_be_bytes([bytes[k], bytes[k + 1]]);
        }
    }
    let step = 1.0 / (n - 1) as f64;
    Ok(Grid {
        lat0: lat_sw as f64,
        lon0: lon_sw as f64,
        dlat: step,
        dlon: step,
        nlat: n,
        nlon: n,
        data: Heights::I16(v),
    })
}

/// SRTM tile name for the tile containing (`lat`, `lon`): e.g. `N45E006`.
pub fn hgt_name(lat_sw: i32, lon_sw: i32) -> String {
    format!(
        "{}{:02}{}{:03}",
        if lat_sw < 0 { 'S' } else { 'N' },
        lat_sw.unsigned_abs(),
        if lon_sw < 0 { 'W' } else { 'E' },
        lon_sw.unsigned_abs()
    )
}

type TileMap = HashMap<(i32, i32), Option<Arc<Grid>>>;

/// A directory of SRTM tiles with an on-demand cache.
pub struct SrtmTiles {
    pub dir: PathBuf,
    pub max_tiles: usize,
    cache: Mutex<(TileMap, VecDeque<(i32, i32)>)>,
}

impl SrtmTiles {
    pub fn new(dir: impl AsRef<Path>, max_tiles: usize) -> SrtmTiles {
        SrtmTiles {
            dir: dir.as_ref().to_path_buf(),
            max_tiles: max_tiles.max(1),
            cache: Mutex::new((HashMap::new(), VecDeque::new())),
        }
    }

    fn load(&self, key: (i32, i32)) -> Option<Arc<Grid>> {
        let name = hgt_name(key.0, key.1);
        for cand in [
            format!("{name}.hgt"),
            format!("{}.hgt", name.to_lowercase()),
        ] {
            let p = self.dir.join(cand);
            if let Ok(b) = std::fs::read(&p) {
                return parse_hgt(&b, key.0, key.1).ok().map(Arc::new);
            }
        }
        None
    }

    /// The tile with south-west corner `key` (cached; `None` if absent).
    pub fn tile(&self, key: (i32, i32)) -> Option<Arc<Grid>> {
        {
            let c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(t) = c.0.get(&key) {
                return t.clone();
            }
        }
        let t = self.load(key);
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let std::collections::hash_map::Entry::Vacant(e) = c.0.entry(key) {
            e.insert(t.clone());
            c.1.push_back(key);
            while c.1.len() > self.max_tiles {
                if let Some(old) = c.1.pop_front() {
                    c.0.remove(&old);
                }
            }
        }
        t
    }

    /// Number of tiles (present or known absent) in the cache.
    pub fn cached(&self) -> usize {
        self.cache.lock().unwrap_or_else(|e| e.into_inner()).0.len()
    }
}

impl ElevationSource for SrtmTiles {
    fn height_m(&self, p: Geo) -> Option<f64> {
        let lon = (p.lon_deg + 540.0).rem_euclid(360.0) - 180.0;
        let (fl, fo) = (p.lat_deg.floor(), lon.floor());
        let q = Geo::new(p.lat_deg, lon);
        // a point on a tile's north/east edge is also on the next tile's south/west edge
        let lat_keys: &[f64] = if p.lat_deg == fl {
            &[fl, fl - 1.0]
        } else {
            &[fl]
        };
        let lon_keys: &[f64] = if lon == fo { &[fo, fo - 1.0] } else { &[fo] };
        for la in lat_keys {
            for lo in lon_keys {
                if let Some(h) = self
                    .tile((*la as i32, *lo as i32))
                    .and_then(|t| t.sample(q))
                {
                    return Some(h);
                }
            }
        }
        None
    }
    fn describe(&self) -> String {
        format!("SRTM tiles in {}", self.dir.display())
    }
}

/// Parse an ESRI ASCII grid (degrees). Returns a node grid whose nodes are the cell
/// centres.
pub fn parse_asc(text: &str) -> Result<Grid, String> {
    let mut hdr: HashMap<String, f64> = HashMap::new();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.peek() {
        let t = l.trim();
        if t.is_empty() {
            lines.next();
            continue;
        }
        let mut it = t.split_whitespace();
        let key = it.next().unwrap_or("").to_ascii_lowercase();
        if key.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            let v: f64 = it
                .next()
                .ok_or_else(|| format!("ASC header {key}: missing value"))?
                .parse()
                .map_err(|e| format!("ASC header {key}: {e}"))?;
            hdr.insert(key, v);
            lines.next();
        } else {
            break;
        }
    }
    let get = |k: &str| {
        hdr.get(k)
            .copied()
            .ok_or_else(|| format!("ASC header: {k} missing"))
    };
    let ncols = get("ncols")? as usize;
    let nrows = get("nrows")? as usize;
    let cs = get("cellsize")?;
    let nodata = hdr.get("nodata_value").copied();
    let (lon_c, lat_c) = match (hdr.get("xllcenter"), hdr.get("yllcenter")) {
        (Some(x), Some(y)) => (*x, *y),
        _ => (get("xllcorner")? + cs / 2.0, get("yllcorner")? + cs / 2.0),
    };
    let mut vals: Vec<f32> = Vec::with_capacity(ncols * nrows);
    for l in lines {
        for f in l.split_whitespace() {
            let v: f64 = f.parse().map_err(|e| format!("ASC value {f:?}: {e}"))?;
            let void = nodata.is_some_and(|n| (v - n).abs() < 1e-6) || !v.is_finite();
            vals.push(if void { f32::NAN } else { v as f32 });
        }
    }
    if vals.len() != ncols * nrows {
        return Err(format!(
            "ASC grid: {} values for {nrows}×{ncols}",
            vals.len()
        ));
    }
    // rows north → south in the file
    let mut data = vec![f32::NAN; ncols * nrows];
    for r in 0..nrows {
        let i = nrows - 1 - r;
        data[i * ncols..(i + 1) * ncols].copy_from_slice(&vals[r * ncols..(r + 1) * ncols]);
    }
    Grid::from_f32(lat_c, lon_c, cs, cs, nrows, ncols, data)
}

/// Parse a CSV grid of heights (south row first; empty or `nan` fields are voids).
/// Returns (values, nlat, nlon).
pub fn parse_csv_grid(text: &str) -> Result<(Vec<f32>, usize, usize), String> {
    let mut v = Vec::new();
    let mut nlat = 0;
    let mut nlon = None;
    for (ln, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let row: Vec<f32> = t
            .split([',', ';', '\t'])
            .map(|f| {
                let f = f.trim();
                if f.is_empty() || f.eq_ignore_ascii_case("nan") {
                    Ok(f32::NAN)
                } else {
                    f.parse::<f32>()
                        .map_err(|e| format!("DEM CSV line {}: {e}", ln + 1))
                }
            })
            .collect::<Result<_, _>>()?;
        match nlon {
            None => nlon = Some(row.len()),
            Some(n) if n != row.len() => {
                return Err(format!(
                    "DEM CSV line {}: {} values, expected {n}",
                    ln + 1,
                    row.len()
                ))
            }
            _ => {}
        }
        v.extend(row);
        nlat += 1;
    }
    Ok((v, nlat, nlon.unwrap_or(0)))
}

/// A Gaussian hill.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hill {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
    /// 1/e radius (km).
    pub radius_km: f64,
}

/// Analytic terrain: Gaussian hills on a base height.
#[derive(Clone, Debug, PartialEq)]
pub struct Hills {
    pub base_m: f64,
    pub hills: Vec<Hill>,
}

impl ElevationSource for Hills {
    fn height_m(&self, p: Geo) -> Option<f64> {
        let mut h = self.base_m;
        for hl in &self.hills {
            let r = inverse(Geo::new(hl.lat_deg, hl.lon_deg), p).distance_m / 1000.0;
            let x = r / hl.radius_km.max(1e-6);
            if x < 6.0 {
                h += hl.height_m * (-x * x).exp();
            }
        }
        Some(h)
    }
    fn describe(&self) -> String {
        format!("{} Gaussian hill(s) on {} m", self.hills.len(), self.base_m)
    }
}

/// Constant height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flat(pub f64);

impl ElevationSource for Flat {
    fn height_m(&self, _p: Geo) -> Option<f64> {
        Some(self.0)
    }
    fn describe(&self) -> String {
        format!("flat {} m", self.0)
    }
}

/// DEM configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DemConfig {
    Flat {
        #[serde(default)]
        height_m: f64,
    },
    Srtm {
        dir: String,
        #[serde(default = "d_tiles")]
        max_tiles: usize,
    },
    Asc {
        file: String,
    },
    CsvGrid {
        file: String,
        lat0_deg: f64,
        lon0_deg: f64,
        dlat_deg: f64,
        dlon_deg: f64,
    },
    Hills {
        #[serde(default)]
        base_m: f64,
        hills: Vec<Hill>,
    },
}

fn d_tiles() -> usize {
    16
}

impl Default for DemConfig {
    fn default() -> Self {
        DemConfig::Flat { height_m: 0.0 }
    }
}

/// A DEM with its void fill.
pub struct Dem {
    pub source: Box<dyn ElevationSource>,
    /// Height used where the source has no data (m).
    pub void_fill_m: f64,
}

impl Dem {
    pub fn new(source: Box<dyn ElevationSource>, void_fill_m: f64) -> Dem {
        Dem {
            source,
            void_fill_m,
        }
    }
    pub fn build(cfg: &DemConfig, void_fill_m: f64) -> Result<Dem, String> {
        let source: Box<dyn ElevationSource> = match cfg {
            DemConfig::Flat { height_m } => Box::new(Flat(*height_m)),
            DemConfig::Srtm { dir, max_tiles } => {
                if !Path::new(dir).is_dir() {
                    return Err(format!("DEM: SRTM directory {dir:?} not found"));
                }
                Box::new(SrtmTiles::new(dir, *max_tiles))
            }
            DemConfig::Asc { file } => {
                let t = std::fs::read_to_string(file).map_err(|e| format!("DEM {file}: {e}"))?;
                Box::new(parse_asc(&t).map_err(|e| format!("DEM {file}: {e}"))?)
            }
            DemConfig::CsvGrid {
                file,
                lat0_deg,
                lon0_deg,
                dlat_deg,
                dlon_deg,
            } => {
                let t = std::fs::read_to_string(file).map_err(|e| format!("DEM {file}: {e}"))?;
                let (v, nlat, nlon) = parse_csv_grid(&t).map_err(|e| format!("DEM {file}: {e}"))?;
                Box::new(Grid::from_f32(
                    *lat0_deg, *lon0_deg, *dlat_deg, *dlon_deg, nlat, nlon, v,
                )?)
            }
            DemConfig::Hills { base_m, hills } => {
                for h in hills {
                    if !(h.radius_km > 0.0) {
                        return Err("DEM hills: radius_km must be positive".into());
                    }
                }
                Box::new(Hills {
                    base_m: *base_m,
                    hills: hills.clone(),
                })
            }
        };
        Ok(Dem {
            source,
            void_fill_m,
        })
    }
    /// Height at `p` with voids filled.
    pub fn height_m(&self, p: Geo) -> f64 {
        self.source.height_m(p).unwrap_or(self.void_fill_m)
    }
}

/// Terrain profile from `a` to `b`: `n + 1` equally spaced points on the geodesic, each
/// `(distance from a (m), height (m), position)`.
pub fn profile(dem: &Dem, a: Geo, b: Geo, n: usize) -> Vec<(f64, f64, Geo)> {
    path_points(a, b, n)
        .into_iter()
        .map(|(d, p)| (d, dem.height_m(p), p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kshana-dem-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A synthetic `n×n` SRTM tile with h = 10·row_from_south + col (void at one node).
    fn synth_hgt(n: usize) -> Vec<u8> {
        let mut b = Vec::with_capacity(n * n * 2);
        for r in 0..n {
            let i = n - 1 - r; // row from the south
            for j in 0..n {
                let v: i16 = if i == 2 && j == 2 {
                    SRTM_VOID
                } else {
                    (10 * i + j) as i16
                };
                b.extend_from_slice(&v.to_be_bytes());
            }
        }
        b
    }

    #[test]
    fn srtm_tiles_orientation_voids_and_cache() {
        assert_eq!(hgt_name(45, 6), "N45E006");
        assert_eq!(hgt_name(-1, -73), "S01W073");
        let dir = tmpdir("srtm");
        // 11×11 tile over 45–46 N, 6–7 E: step 0.1°
        std::fs::write(dir.join("N45E006.hgt"), synth_hgt(11)).unwrap();
        let t = SrtmTiles::new(&dir, 2);
        // node (i=1, j=3): lat 45.1, lon 6.3 → 13
        assert!((t.height_m(Geo::new(45.1, 6.3)).unwrap() - 13.0).abs() < 1e-9);
        // bilinear midway: (45.15, 6.35) → mean of 13, 14, 23, 24 = 18.5
        assert!((t.height_m(Geo::new(45.15, 6.35)).unwrap() - 18.5).abs() < 1e-9);
        // void at node (2,2): the remaining weights are renormalised
        assert!(
            t.height_m(Geo::new(45.2, 6.2)).is_none(),
            "exactly on the void node"
        );
        let h = t.height_m(Geo::new(45.15, 6.15)).unwrap(); // corners 11, 12, 21, void
        assert!((h - (11.0 + 12.0 + 21.0) / 3.0).abs() < 1e-9, "{h}");
        // north-east corner node (shared edge): 10·10 + 10 = 110
        assert!((t.height_m(Geo::new(46.0, 7.0)).unwrap() - 110.0).abs() < 1e-9);
        // missing tile: None → void fill via Dem
        assert!(t.height_m(Geo::new(10.5, 6.5)).is_none());
        assert!(t.cached() <= 2);
        t.height_m(Geo::new(20.5, 6.5));
        assert_eq!(t.cached(), 2, "cache bounded by max_tiles");
        let dem = Dem::new(Box::new(SrtmTiles::new(&dir, 4)), -1.0);
        assert_eq!(dem.height_m(Geo::new(10.5, 6.5)), -1.0);
        // lower-case file names are found as well
        std::fs::write(dir.join("s01w073.hgt"), synth_hgt(3)).unwrap();
        assert!(dem.source.height_m(Geo::new(-0.5, -72.5)).is_some());
        // standard sizes recognised
        assert_eq!(
            parse_hgt(&vec![0u8; 1201 * 1201 * 2], 0, 0).unwrap().nlat,
            1201
        );
        assert!(parse_hgt(&[0u8; 7], 0, 0).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn asc_and_csv_grids() {
        let asc =
            "ncols 3\nnrows 2\nxllcorner 10.0\nyllcorner 50.0\ncellsize 0.5\nNODATA_value -9999\n\
                   1 2 3\n4 5 -9999\n";
        let g = parse_asc(asc).unwrap();
        // cell centres: south row at 50.25 (values 4 5 void), north row at 50.75 (1 2 3)
        assert!((g.sample(Geo::new(50.25, 10.25)).unwrap() - 4.0).abs() < 1e-9);
        assert!((g.sample(Geo::new(50.75, 11.25)).unwrap() - 3.0).abs() < 1e-9);
        assert!(g.sample(Geo::new(50.25, 11.25)).is_none(), "void node");
        assert!((g.sample(Geo::new(50.5, 10.5)).unwrap() - 3.0).abs() < 1e-9);
        assert!(g.sample(Geo::new(49.0, 10.5)).is_none());
        let (v, nlat, nlon) = parse_csv_grid("# south first\n0,10\n20,\n").unwrap();
        assert_eq!((nlat, nlon), (2, 2));
        let g = Grid::from_f32(0.0, 0.0, 1.0, 1.0, nlat, nlon, v).unwrap();
        assert!((g.sample(Geo::new(0.0, 0.5)).unwrap() - 5.0).abs() < 1e-9);
        assert!((g.sample(Geo::new(1.0, 0.5)).unwrap() - 20.0).abs() < 1e-9);
        assert!(parse_csv_grid("1,2\n3\n").is_err());
        // files through the configuration
        let dir = tmpdir("grids");
        let f = dir.join("g.asc");
        std::fs::write(&f, asc).unwrap();
        let d = Dem::build(
            &DemConfig::Asc {
                file: f.to_string_lossy().into(),
            },
            0.0,
        )
        .unwrap();
        assert!((d.height_m(Geo::new(50.75, 10.75)) - 2.0).abs() < 1e-9);
        let c = dir.join("g.csv");
        std::fs::write(&c, "0,10\n20,30\n").unwrap();
        let cfg: DemConfig = toml::from_str(&format!(
            "type = \"csv_grid\"\nfile = {:?}\nlat0_deg = 0.0\nlon0_deg = 0.0\ndlat_deg = 1.0\ndlon_deg = 1.0",
            c.to_string_lossy()
        ))
        .unwrap();
        let d = Dem::build(&cfg, 0.0).unwrap();
        assert!((d.height_m(Geo::new(0.5, 0.5)) - 15.0).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_along_geodesic_with_hill() {
        let dem = Dem::build(
            &DemConfig::Hills {
                base_m: 5.0,
                hills: vec![Hill {
                    lat_deg: 0.5,
                    lon_deg: 0.0,
                    height_m: 300.0,
                    radius_km: 5.0,
                }],
            },
            0.0,
        )
        .unwrap();
        let p = profile(&dem, Geo::new(0.0, 0.0), Geo::new(1.0, 0.0), 100);
        assert_eq!(p.len(), 101);
        assert!((p[50].1 - 305.0).abs() < 1.0, "{}", p[50].1);
        assert!((p[0].1 - 5.0).abs() < 1e-6 && (p[100].1 - 5.0).abs() < 1e-6);
        assert!(
            (p[100].0 - inverse(Geo::new(0.0, 0.0), Geo::new(1.0, 0.0)).distance_m).abs() < 1e-6
        );
    }
}
