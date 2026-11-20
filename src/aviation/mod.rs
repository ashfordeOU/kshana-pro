// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Aviation operational analytics (research backlog E11: B1101 to B1104)
//!
//! Four analyses an operator, an airport or an ANSP runs on data it already holds, built on
//! the FDM detector ([`crate::fdm`]), the documented ADS-B CSV format and the open engine's
//! interference map (grid, NIC/NACp bounds, route exposure, publication minimum):
//!
//! * [`approach`] (B1101): per-airport GNSS approach availability loss, go-around exposure
//!   and a capacity-impact model from ADS-B in the terminal area, with optional oceanic
//!   RNP support evidence after spoofing (`kshana-pro aviation approach`, feature
//!   [`FEATURE_APPROACH`]).
//! * [`residual`] (B1102): post-exposure residual effects across consecutive flights of one
//!   aircraft (degradation persisting after leaving the area, a GNSS clock that stays wrong,
//!   a shifted GNSS-baro offset, TAWS alerts with no concurrent interference) and a
//!   clean-or-reset recommendation (`aviation residual`, [`FEATURE_RESIDUAL`]).
//! * [`nuisance`] (B1103): nuisance-alert and alert-fatigue statistics per route and phase
//!   with intervals, a workload estimate and a training extract (`aviation nuisance`,
//!   [`FEATURE_NUISANCE`]); the alert classes cover TAWS/EGPWS, clock and bridge GNSS/BNWAS
//!   alarms.
//! * [`unpublished`] (B1104): interference areas found in fleet data with no covering NOTAM,
//!   and short-burst events, published only above a minimum-evidence threshold, at the open
//!   engine's 0.5° cell granularity (`aviation areas`, [`FEATURE_AREAS`]).
//!
//! Every result is **decision support**, never an operational approval, an airworthiness
//! determination or a means of compliance ([`DECISION_SUPPORT`]), and is labelled MODELLED:
//! the rules are exercised on synthetic fixtures with known truth, not validated against
//! labelled operational data. Nothing here locates an emitter: areas are the cells where
//! aircraft were affected, never a source position.

pub mod approach;
pub mod nuisance;
pub mod residual;
pub mod synth;
pub mod unpublished;

use std::path::Path;

use serde::{Deserialize, Serialize};

/// `aviation approach` (Pro).
pub const FEATURE_APPROACH: &str = "pro-approach-availability";
/// `aviation residual` (Pro).
pub const FEATURE_RESIDUAL: &str = "pro-residual-effects";
/// `aviation nuisance` (Pro).
pub const FEATURE_NUISANCE: &str = "pro-nuisance-alerts";
/// `aviation areas` (Enterprise: fleet-wide).
pub const FEATURE_AREAS: &str = "pro-unpublished-areas";

/// What every result says about itself.
pub const DECISION_SUPPORT: &str = "Decision support only. Not an operational approval, not \
an airworthiness or continuing-airworthiness determination, not a means of compliance and not \
a substitute for the operator's, the ANSP's or the manufacturer's procedures. Findings describe \
the consistency of the data supplied, never intent or cause.";

/// Round to 9 significant digits (every float that feeds a signed hash goes through this,
/// so the bytes do not depend on the last bits of a platform's arithmetic).
pub fn sig9(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return if x == 0.0 { 0.0 } else { x };
    }
    format!("{x:.8e}").parse().unwrap_or(x)
}

/// Round to `d` decimals, then to 9 significant digits.
pub fn rnd(x: f64, d: i32) -> f64 {
    sig9(crate::forensics::inputs::round(x, d))
}

/// Wilson score interval (95 %) for `k` successes in `n` trials, as `(lower, upper)`.
pub fn wilson95(k: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let (k, n) = (k as f64, n as f64);
    let z = 1.959_963_985;
    let p = k / n;
    let den = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / den;
    let h = z * ((p * (1.0 - p) + z * z / (4.0 * n)) / n).sqrt() / den;
    Some((rnd((c - h).max(0.0), 4), rnd((c + h).min(1.0), 4)))
}

/// Approximate 95 % interval for a Poisson count `n` (Byar's approximation), as counts.
pub fn poisson95(n: usize) -> (f64, f64) {
    let z = 1.959_963_985;
    let lo = if n == 0 {
        0.0
    } else {
        let x = n as f64;
        x * (1.0 - 1.0 / (9.0 * x) - z / (3.0 * x.sqrt())).powi(3)
    };
    let x = n as f64 + 1.0;
    let hi = x * (1.0 - 1.0 / (9.0 * x) + z / (3.0 * x.sqrt())).powi(3);
    (lo.max(0.0), hi)
}

/// One input file as a result records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRef {
    pub role: String,
    /// The file name inside the result directory (`inputs/<name>`).
    pub file: String,
    pub sha256: String,
}

/// Inputs read once: the record and the bytes to copy into the result.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub refs: Vec<InputRef>,
    pub copies: Vec<(String, Vec<u8>)>,
}

impl Inputs {
    /// Read `path` (relative paths against `base`), record it under `role`, return its text.
    pub fn read(&mut self, base: &Path, path: &str, role: &str) -> Result<String, String> {
        let p = if Path::new(path).is_absolute() {
            Path::new(path).to_path_buf()
        } else {
            base.join(path)
        };
        let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let mut name = crate::ireports::safe_name(path);
        let mut k = 2;
        while self.copies.iter().any(|(n, _)| *n == name) {
            name = format!("{k}-{}", crate::ireports::safe_name(path));
            k += 1;
        }
        self.refs.push(InputRef {
            role: role.into(),
            file: name.clone(),
            sha256: crate::forensics::inputs::sha256_hex(&b),
        });
        let text = String::from_utf8_lossy(&b).to_string();
        self.copies.push((name, b));
        Ok(text)
    }

    /// Record text given directly (tests and library callers).
    pub fn add_text(&mut self, name: &str, role: &str, text: &str) {
        self.refs.push(InputRef {
            role: role.into(),
            file: name.into(),
            sha256: crate::forensics::inputs::sha256_hex(text.as_bytes()),
        });
        self.copies.push((name.into(), text.as_bytes().to_vec()));
    }
}

/// Write `result` (pretty JSON) as `file`, a plain-text `summary.txt`, extra files, and the
/// inputs under `inputs/`. The CLI signs the directory afterwards.
pub fn write_result<T: Serialize>(
    dir: &Path,
    file: &str,
    result: &T,
    summary: &str,
    extra: &[(&str, String)],
    inputs: &Inputs,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("inputs")).map_err(io)?;
    let json = serde_json::to_string_pretty(result).map_err(|e| e.to_string())? + "\n";
    std::fs::write(dir.join(file), json).map_err(io)?;
    std::fs::write(dir.join("summary.txt"), summary).map_err(io)?;
    for (name, text) in extra {
        std::fs::write(dir.join(name), text).map_err(io)?;
    }
    for (name, bytes) in &inputs.copies {
        std::fs::write(dir.join("inputs").join(name), bytes).map_err(io)?;
    }
    Ok(())
}

/// A box `[south, west, north, east]` in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeoBox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl GeoBox {
    pub fn from_array(a: [f64; 4]) -> Result<Self, String> {
        let b = GeoBox {
            south: a[0],
            west: a[1],
            north: a[2],
            east: a[3],
        };
        if !(a.iter().all(|v| v.is_finite())
            && (-90.0..=90.0).contains(&b.south)
            && (-90.0..=90.0).contains(&b.north)
            && b.south < b.north
            && (-180.0..=180.0).contains(&b.west)
            && (-180.0..=180.0).contains(&b.east)
            && b.west < b.east)
        {
            return Err(format!(
                "box {a:?}: needs [south, west, north, east] with south < north and west < east"
            ));
        }
        Ok(b)
    }

    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.south && lat <= self.north && lon >= self.west && lon <= self.east
    }
}

/// Unix seconds of an ISO-8601 time (or Unix seconds as text).
pub fn utc(s: &str) -> Result<f64, String> {
    crate::forensics::inputs::parse_utc(s).ok_or(format!("'{s}' is not an ISO-8601 UTC time"))
}

/// Classify an alert text: `taws`, `clock`, `gnss`, `bnwas`, `ais` or `other`.
pub fn alert_class(text: &str) -> &'static str {
    let t = text.to_ascii_uppercase();
    let has = |w: &[&str]| w.iter().any(|x| t.contains(x));
    if has(&[
        "TERRAIN",
        "PULL UP",
        "EGPWS",
        "TAWS",
        "GPWS",
        "OBSTACLE",
        "SINK RATE",
        "GLIDESLOPE",
    ]) {
        "taws"
    } else if has(&["CLOCK", "TIME", "UTC"]) {
        "clock"
    } else if has(&["BNWAS", "WATCH ALARM"]) {
        "bnwas"
    } else if has(&["AIS"]) {
        "ais"
    } else if has(&[
        "GPS",
        "GNSS",
        "POSITION",
        "NAV ",
        "RAIM",
        "UNABLE RNP",
        "FMS POS",
    ]) {
        "gnss"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sig9_rounds_to_nine_significant_digits() {
        assert_eq!(sig9(1.234_567_891_23), 1.234_567_89);
        assert_eq!(sig9(123_456_789_123.0), 123_456_789_000.0);
        assert_eq!(sig9(0.0), 0.0);
        assert_eq!(sig9(-2.5), -2.5);
    }

    #[test]
    fn wilson_and_poisson_intervals() {
        let (lo, hi) = wilson95(5, 10).unwrap();
        assert!((lo - 0.2366).abs() < 1e-3 && (hi - 0.7634).abs() < 1e-3);
        assert!(wilson95(0, 0).is_none());
        let (lo, hi) = poisson95(10);
        assert!(
            (lo - 4.80).abs() < 0.05 && (hi - 18.39).abs() < 0.1,
            "{lo} {hi}"
        );
        assert_eq!(poisson95(0).0, 0.0);
    }

    #[test]
    fn box_checks_order() {
        assert!(GeoBox::from_array([1.0, 2.0, 0.0, 3.0]).is_err());
        let b = GeoBox::from_array([0.0, 0.0, 1.0, 1.0]).unwrap();
        assert!(b.contains(0.5, 0.5) && !b.contains(1.5, 0.5));
    }
}
