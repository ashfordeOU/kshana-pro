// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Lunar and cislunar PNT evidence (research backlog E24)
//!
//! Independent verification evidence for the people who build, buy, insure or review lunar
//! navigation: lunar navigation service providers (LNSPs), receiver makers, ground-station
//! operators and landing teams. It sits beside the D-family lunar layer ([`crate::lunar_pro`],
//! whose D7 checks a service's monthly SLA) and reuses the open engine through the pinned
//! `kshana` dependency: the AFS reference generator (`kshana::lunar_afs`), the Allan-family
//! estimators (`kshana::allan`), the named lunar clock classes (`kshana::clock_specs`), the
//! lunar frame rotations (`kshana::lunar_frame`) and the Helmert datum Jacobian
//! (`kshana::lunar_datum`).
//!
//! | Backlog item | Command | Module | What it shows |
//! |---|---|---|---|
//! | B2401 URA/SISA overbounding | `ura` | [`service`] | whether broadcast URA overbounds the signal-in-space error at each threshold, with the inflation that would make it |
//! | B2401 time consistency | `time-consistency` | [`service`] | which node's clock departs from the constellation ensemble, when, and as a step or a ramp |
//! | AFS receiver verification (Pro view of B2403) | `afs-rx` | [`afs`] | receiver replica codes against the open reference generator, frame CRC and TOI continuity, PRN collisions |
//! | B2405 lunar clock characterisation | `clock` | [`clocks`] | measured ADEV with 95 % intervals against a named clock class or a mask |
//! | B2405 station delay calibration | `station-delay` | [`clocks`] | GUM budget of a ground-station delay, expanded uncertainty, En against the previous calibration |
//! | B2405 radiometric forensics and track quality | `radiometric` | [`clocks`] | per-pass outliers, steps, ramps, biases and gaps, and a track-quality score |
//! | B2404 FDIR verification | `fdir` | [`surface`] | landing-sensor fault-handling results against a pre-registered plan (hash-locked) |
//! | B2404 configuration-state attestation | `config` | [`surface`] | the flight configuration as loaded against the reviewed one |
//! | B2404 map-to-frame datum tie | `datum` | [`surface`] | a map's offset from reference control, and whether it matches the mean-Earth/principal-axis rotation |
//! | B2404 tip-over probability | `tipover` | [`surface`] | MODELLED probability of touching down on a slope beyond the static tip-over angle |
//!
//! Every result is a signed directory (`crate::signed_dossier`, Ed25519), labelled
//! **MODELLED**: checked on synthetic inputs with known faults. No lunar navigation service,
//! lunar clock record or landing campaign has been scored here, so nothing is VALIDATED. Where a
//! public document defines a quantity, the source is named; where its section has not been
//! read here it says "to confirm". Nothing here certifies, qualifies or approves anything.

pub mod afs;
pub mod clocks;
pub mod service;
pub mod surface;
pub mod synth;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::aviation::Inputs;

/// `ura`, `time-consistency`, `afs-rx` (Pro).
pub const FEATURE_SERVICE: &str = "pro-lunar-service-verification";
/// `clock`, `station-delay`, `radiometric` (Pro).
pub const FEATURE_CLOCK: &str = "pro-lunar-clock-evidence";
/// `fdir`, `config`, `datum`, `tipover` (Pro).
pub const FEATURE_SURFACE: &str = "pro-lunar-surface-evidence";

/// Evidence class of every result.
pub const EVIDENCE: &str = "MODELLED";

/// What every result says about itself.
pub const STATEMENT: &str = "MODELLED evidence for engineering review: checked on synthetic \
inputs with known faults, not validated against measured lunar data. It is not a certification, \
qualification or approval, it does not show that anything meets a standard, and it does not replace the provider's, the \
operator's or the reviewing agency's own verification.";

/// A request file: one section per command (only the section of the command run is needed).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub ura: Option<service::UraReq>,
    #[serde(default)]
    pub time_consistency: Option<service::TimeReq>,
    #[serde(default)]
    pub afs_rx: Option<afs::AfsReq>,
    #[serde(default)]
    pub clock: Option<clocks::ClockReq>,
    #[serde(default)]
    pub station_delay: Option<clocks::DelayReq>,
    #[serde(default)]
    pub radiometric: Option<clocks::RadioReq>,
    #[serde(default)]
    pub fdir: Option<surface::FdirReq>,
    #[serde(default)]
    pub config: Option<surface::ConfigReq>,
    #[serde(default)]
    pub datum: Option<surface::DatumReq>,
    #[serde(default)]
    pub tipover: Option<surface::TipReq>,
}

/// A request and the files it names, read once.
pub struct Loaded {
    pub req: Request,
    pub base: std::path::PathBuf,
    pub inputs: Inputs,
}

impl Loaded {
    /// Read a file the request names (relative to the request) and record it as an input.
    pub fn read(&mut self, path: &str, role: &str) -> Result<String, String> {
        let base = self.base.clone();
        self.inputs.read(&base, path, role)
    }
}

/// Read a request file.
pub fn load(path: &Path) -> Result<Loaded, String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&src).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut inputs = Inputs::default();
    inputs.add_text("request.toml", "request", &src);
    Ok(Loaded {
        req,
        base: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
        inputs,
    })
}

/// The plain-text summary of a result.
pub fn summary(kind: &str, headline: &str) -> String {
    format!("[lunar-evidence {kind}] {headline}\n{STATEMENT}\nEvidence class: {EVIDENCE}.\n")
}

/// Parse a CSV with a header row: the rows as string fields, checked against `cols`.
pub(crate) fn csv_rows(text: &str, cols: &[&str]) -> Result<Vec<Vec<String>>, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("empty CSV")?
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    if head.len() < cols.len() || head.iter().zip(cols).any(|(h, c)| h != c) {
        return Err(format!(
            "CSV header must start with {}, got {}",
            cols.join(","),
            head.join(",")
        ));
    }
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        let f: Vec<String> = l.split(',').map(|s| s.trim().to_string()).collect();
        if f.len() < cols.len() {
            return Err(format!(
                "CSV row {}: {} fields, need {}",
                i + 2,
                f.len(),
                cols.len()
            ));
        }
        out.push(f);
    }
    Ok(out)
}

/// A field as a finite number.
pub(crate) fn num(s: &str, what: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("{what}: '{s}' is not a finite number"))
}

/// Two-sided standard normal tail `P(|Z| > k) = erfc(k/√2)`.
pub(crate) fn two_sided_tail(k: f64) -> f64 {
    kshana::lambda::erfc_accurate(k / std::f64::consts::SQRT_2)
}

/// Mean and sample standard deviation.
pub(crate) fn mean_std(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    if v.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let m = v.iter().sum::<f64>() / n;
    let s = if v.len() > 1 {
        (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    (m, s)
}

/// Median of a slice (NaN when empty).
pub(crate) fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// Three-way verdict of an interval against an upper limit.
pub(crate) fn upper_verdict(lo: f64, hi: f64, limit: f64, ok: &str, bad: &str) -> String {
    if hi <= limit {
        ok.into()
    } else if lo > limit {
        bad.into()
    } else {
        "INCONCLUSIVE".into()
    }
}

pub use crate::aviation::{rnd, wilson95};
