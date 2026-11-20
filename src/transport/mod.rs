// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Road, rail, automotive, eCall and 3GPP evidence (research 2026-11-20 gaps, epic E30)
//!
//! Evidence tools for land transport and cellular positioning, each over the user's own
//! records or stated parameters:
//!
//! * [`telematics`] (B3002): GNSS anti-spoof and jamming checks over telematics and
//!   road-charging on-board-unit streams (position jumps, GNSS against wheel speed, frozen
//!   position, outages while moving, C/N0 and AGC, receiver time, network position), episodes,
//!   per-trip trust for pay-as-you-drive, an on-board-unit fraud evaluation against recorded
//!   attack windows with the distance lost to charging (EETS evidence), and a jammer
//!   fingerprint library that classifies an episode from receiver-side observables only.
//!   Privacy rule: vehicle identifiers are pseudonymised with a salt and positions leave only
//!   as coarse cells.
//! * [`ecall`] (B3003): the GNSS tests of Commission Delegated Regulation (EU) 2017/79
//!   Annex VI (the eCall GNSS requirements UN Regulation No 144 also carries), a mapping of
//!   every Annex VI requirement to its test, scoring of laboratory results against the
//!   stated limits; emergency-location (AML/E112, PSAP) reliability statistics; duty-cycling
//!   detection and an IoT tracker battery trade-off model.
//! * [`integrity`]: protection-level statistics (Stanford diagram counts, exact binomial
//!   bounds on misleading-information rates with an effective sample size) and a Gaussian
//!   overbound check, shared by the automotive and rail tools.
//! * [`auto`] (B3001, first slice): a SOTIF triggering-condition catalogue for GNSS, PL
//!   validation per triggering condition with coverage, V2X position and time checks over a
//!   decoded message log, and a data-level fault-injection library for recorded output.
//! * [`nr`] (B3004, first slice): 5G TDD frame-timing failure under GNSS loss (on the
//!   holdover evidence model), a PRS timing-accuracy model against bandwidth and geometry,
//!   and Rel-18-style integrity vectors with a checker of a device's reported levels;
//!   [`tolerance`] and [`agnss`] (B3004): a shared-risk test-tolerance calculator and an A-GNSS
//!   vector-set scaffold with placeholders and scoring, on values the user transcribes (no
//!   TS 37.571-1 value is shipped).
//! * [`rail`] (B3005, first slice): a tolerable-hazard-rate position-error model including
//!   jamming episodes, a virtual-balise passage error test with an overbound check, and the
//!   EN 50129 evidence catalogue (`regpack assess --catalogue rail-gnss-sil4`).
//!
//! **Evidence label: MODELLED** unless a function says otherwise. Outputs say "checked
//! against <document> §x", "pre-assessment" or "evidence for"; whether a product meets a
//! regulation or standard is for its assessor and authority to decide. No output locates an
//! emitter, controls an antenna, fuses sensors for navigation or generates a signal.

pub mod agnss;
pub mod auto;
pub mod ecall;
pub mod integrity;
pub mod nr;
pub mod rail;
pub mod telematics;
pub mod tolerance;

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

pub use crate::timing::grid::rounded_json;
pub(crate) use crate::timing::grid::{page, write_pack};

/// `x > 0` and not NaN.
pub(crate) fn pos(x: f64) -> bool {
    x > 0.0
}

/// `x >= 0` and not NaN.
pub(crate) fn nonneg(x: f64) -> bool {
    x >= 0.0
}

/// Mean Earth radius for short-baseline distances (m).
pub const R_EARTH_M: f64 = 6_371_008.8;

/// Great-circle distance between two points in degrees (haversine), metres.
pub fn dist_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * R_EARTH_M * a.sqrt().min(1.0).asin()
}

/// East and north offsets (m) of point 2 from point 1 (local tangent plane, short baselines).
pub fn en_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, f64) {
    let n = (lat2 - lat1).to_radians() * R_EARTH_M;
    let e = (lon2 - lon1).to_radians() * R_EARTH_M * lat1.to_radians().cos();
    (e, n)
}

/// A point `(e, n)` metres from `(lat, lon)`.
pub fn offset_deg(lat: f64, lon: f64, e: f64, n: f64) -> (f64, f64) {
    (
        lat + (n / R_EARTH_M).to_degrees(),
        lon + (e / (R_EARTH_M * lat.to_radians().cos())).to_degrees(),
    )
}

/// One CSV table with a header row; `#` lines and blank lines are skipped.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub cols: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// Parse comma-separated text with a header row.
    pub fn parse(text: &str) -> Result<Table, String> {
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'));
        let head = lines.next().ok_or("empty table: no header row")?;
        let cols: Vec<String> = head.split(',').map(|c| c.trim().to_string()).collect();
        let mut rows = Vec::new();
        for (i, l) in lines.enumerate() {
            let r: Vec<String> = l.split(',').map(|c| c.trim().to_string()).collect();
            if r.len() != cols.len() {
                return Err(format!(
                    "row {}: {} fields, header has {}",
                    i + 2,
                    r.len(),
                    cols.len()
                ));
            }
            rows.push(r);
        }
        Ok(Table { cols, rows })
    }

    /// Read and parse a file.
    pub fn read(path: &Path) -> Result<Table, String> {
        let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Table::parse(&t).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Column index.
    pub fn col(&self, name: &str) -> Option<usize> {
        self.cols.iter().position(|c| c == name)
    }

    /// Column index, or an error naming the column.
    pub fn need(&self, name: &str) -> Result<usize, String> {
        self.col(name)
            .ok_or_else(|| format!("missing column '{name}'"))
    }

    /// The number in `row` at column `c` (`None` when the column is absent or the cell empty).
    pub fn num(&self, row: usize, c: Option<usize>) -> Result<Option<f64>, String> {
        let Some(c) = c else { return Ok(None) };
        let s = self.rows[row][c].as_str();
        if s.is_empty() || s.eq_ignore_ascii_case("nan") {
            return Ok(None);
        }
        s.parse::<f64>().map(Some).map_err(|_| {
            format!(
                "row {}: '{}' is not a number ({})",
                row + 2,
                s,
                self.cols[c]
            )
        })
    }

    /// The text in `row` at column `c` (empty when absent).
    pub fn text(&self, row: usize, c: Option<usize>) -> &str {
        c.map(|c| self.rows[row][c].as_str()).unwrap_or("")
    }
}

/// Read a TOML request.
pub fn parse_toml<T: serde::de::DeserializeOwned>(src: &str, what: &str) -> Result<T, String> {
    toml::from_str(src).map_err(|e| format!("{what}: {e}"))
}

/// Pseudonym of an identifier: the first 12 hex characters of SHA-256(salt ‖ 0x1f ‖ id).
pub fn pseudonym(salt: &str, id: &str) -> String {
    let mut h = Sha256::new();
    h.update(salt.as_bytes());
    h.update([0x1f]);
    h.update(id.as_bytes());
    hex::encode(h.finalize())[..12].to_string()
}

/// A coarse cell label of a position: the south-west corner on a `step_deg` grid.
pub fn cell(lat: f64, lon: f64, step_deg: f64) -> String {
    let f = |x: f64| (x / step_deg).floor() * step_deg;
    format!("{:.2},{:.2}", f(lat), f(lon))
}

/// The `q` quantile (0..=1) of `v` by linear interpolation; `None` when empty.
pub fn quantile(v: &[f64], q: f64) -> Option<f64> {
    let mut s: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    if s.is_empty() {
        return None;
    }
    s.sort_by(f64::total_cmp);
    let pos = q.clamp(0.0, 1.0) * (s.len() - 1) as f64;
    let (i, f) = (pos.floor() as usize, pos.fract());
    Some(if i + 1 < s.len() {
        s[i] * (1.0 - f) + s[i + 1] * f
    } else {
        s[i]
    })
}

/// Mean; `None` when empty.
pub fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// Exact (Clopper-Pearson) two-sided 95 % interval of `k` events in `n` trials.
pub fn cp95(k: usize, n: usize) -> (f64, f64) {
    kshana::m4_logic::clopper_pearson(k, n)
}

/// Wilson 95 % interval of `k` in `n`; `None` when `n == 0`.
pub fn wilson95(k: usize, n: usize) -> Option<(f64, f64)> {
    crate::aviation::wilson95(k, n)
}

/// Standard normal cumulative distribution.
pub fn norm_cdf(z: f64) -> f64 {
    crate::timing::model::norm_cdf(z)
}

/// Inverse standard normal cumulative distribution.
pub fn norm_quantile(p: f64) -> f64 {
    kshana::raim::normal_quantile(p)
}

/// Count of each string, sorted.
pub fn counts<'a>(it: impl Iterator<Item = &'a str>) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for s in it {
        *m.entry(s.to_string()).or_insert(0) += 1;
    }
    m
}

/// `x` with 3 significant digits for pages.
pub fn f3(x: f64) -> String {
    if !x.is_finite() {
        return format!("{x}");
    }
    if x == 0.0 {
        return "0".into();
    }
    let mag = x.abs().log10().floor() as i32;
    if !(-3..6).contains(&mag) {
        format!("{x:.2e}")
    } else {
        let d = (2 - mag).max(0) as usize;
        format!("{x:.d$}")
    }
}

/// An optional number for pages.
pub fn fo(x: Option<f64>) -> String {
    x.map(f3).unwrap_or_else(|| "-".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_and_offsets_agree() {
        let (lat, lon) = offset_deg(59.437, 24.7536, 300.0, -400.0);
        let d = dist_m(59.437, 24.7536, lat, lon);
        assert!((d - 500.0).abs() < 0.5, "{d}");
        let (e, n) = en_m(59.437, 24.7536, lat, lon);
        assert!((e - 300.0).abs() < 0.5 && (n + 400.0).abs() < 0.5);
    }

    #[test]
    fn table_quantile_and_privacy() {
        let t = Table::parse("# c\na,b\n1,\n2,x\n").unwrap();
        assert_eq!(t.num(0, t.col("a")).unwrap(), Some(1.0));
        assert_eq!(t.num(0, t.col("b")).unwrap(), None);
        assert!(t.num(1, t.col("b")).is_err());
        assert!(Table::parse("a,b\n1\n").is_err());
        assert_eq!(quantile(&[1.0, 2.0, 3.0, 4.0, 5.0], 0.5), Some(3.0));
        assert_eq!(pseudonym("s", "car-1").len(), 12);
        assert_ne!(pseudonym("s", "car-1"), pseudonym("t", "car-1"));
        assert_eq!(cell(59.437, 24.7536, 0.1), "59.40,24.70");
    }
}
