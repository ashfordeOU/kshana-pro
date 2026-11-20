// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Simulation fidelity and lab tooling (research 2026-11-20 gaps, epic E32)
//!
//! Tools for the people who run GNSS simulators and record-and-replay benches, next to the
//! test-as-code kit ([`crate::testkit`], whose replay-fidelity statement these feed):
//!
//! * [`chain`]: a **recorder-chain register** (B3201, gap 08:A6): the stages of a recording
//!   chain (antenna/LNA, filters, cables, amplifiers, ADC, AGC, reference) as a versioned
//!   file, the cascade (Friis noise figure, gain, narrowest bandwidth, group delay, bit depth),
//!   findings, the change since the last registration, a `capture.toml` for
//!   `testkit fidelity`, and entries in the hash-chained calibration register of
//!   `kshana_eloran::calreg`.
//! * [`sync`]: a **synchronous multi-sensor replay check** (B3201, gap 08:A7): GNSS I/Q,
//!   IMU, CAN and camera streams on their own clocks, aligned through shared events, with
//!   per-stream offset, drift, jitter, gaps and residual against the tolerance, and a replay
//!   manifest (common window, per-stream start and correction).
//! * [`selftest`]: a **simulator self-test and replay-rig check** (B3203, gaps 08:A31 and
//!   09:D11): reference receivers' C/N0, fixes and PPS against what the simulator was set to
//!   produce (or against the station observations a replay rig reproduces), with the
//!   deviation attributed to the simulator only when every reference receiver sees it.
//! * [`labreport`]: a **comparable lab test-report format** (B3206, gaps 08:D4-D6): lab
//!   results in a common vocabulary through a column mapping, an EN 16803 topic index, and a
//!   comparison across labs with the En number of ISO 13528.
//! * [`tco`]: a **simulator total-cost-of-ownership calculator** (B3203, gap 08b:Q2) over
//!   the user's own quotes (free).
//!
//! Licence: [`FEATURE_REPLAY`] (chain, sync), [`FEATURE_SELFTEST`] (selftest) and
//! [`FEATURE_REPORT`] (report, compare), all Pro. `tco`, `synth` and `check` are free.
//!
//! **Evidence label: MODELLED.** Rules of thumb and textbook formulas, stated per check; no
//! bench, simulator or lab has been assessed with these tools in this build. Outputs never
//! say a simulator, receiver, bench or lab is calibrated, compliant, conformant, accredited
//! or certified. Safety: these tools read files and write files; they drive no instrument and
//! emit no signal.

pub mod chain;
pub mod labreport;
pub mod selftest;
pub mod sync;
pub mod synth;
pub mod tco;

use std::path::Path;

use crate::aviation::Inputs;

/// Licence feature of `simlab chain` and `simlab sync`.
pub const FEATURE_REPLAY: &str = "pro-simlab-replay";
/// Licence feature of `simlab selftest`.
pub const FEATURE_SELFTEST: &str = "pro-simlab-selftest";
/// Licence feature of `simlab report` and `simlab compare`.
pub const FEATURE_REPORT: &str = "pro-simlab-report";

/// Evidence class of every output.
pub const EVIDENCE: &str = "MODELLED";

/// The label of every result document.
pub const LABEL: &str = "MODELLED: computed from the files supplied with the rules of thumb and \
formulas stated in docs/SIMLAB.md. Not a calibration of any instrument, not an accreditation of \
any lab, and not a statement that any simulator, receiver or bench meets a standard.";

/// Read a request file and record it as an input.
pub fn read_request(path: &Path, role: &str) -> Result<(String, Inputs), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut inputs = Inputs::default();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "request.toml".into());
    inputs.add_text(&name, role, &src);
    Ok((src, inputs))
}

/// The directory relative input paths are read against.
pub fn base_of(path: &Path) -> &Path {
    path.parent().unwrap_or(Path::new("."))
}

/// Parse a numeric CSV: a header line (or `#` comments) skipped, rows of `ncols` numbers.
/// Returns the rows and the number of rows skipped as malformed.
pub fn numeric_csv(text: &str, ncols: usize) -> (Vec<Vec<f64>>, usize) {
    let mut rows = Vec::new();
    let mut bad = 0;
    for (i, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let v: Result<Vec<f64>, _> = l
            .split(',')
            .take(ncols)
            .map(|x| x.trim().parse::<f64>())
            .collect();
        match v {
            Ok(v) if v.len() == ncols && v.iter().all(|x| x.is_finite()) => rows.push(v),
            _ if i == 0 => {}
            _ => bad += 1,
        }
    }
    (rows, bad)
}

/// The plain-text summary of a result.
pub fn summary(cmd: &str, headline: &str) -> String {
    format!("[simlab {cmd}] {headline}\nEvidence class: {EVIDENCE}.\n{LABEL}\n")
}

pub(crate) fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.iter().sum::<f64>() / v.len() as f64
}

pub(crate) fn std_dev(v: &[f64]) -> f64 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
}

pub(crate) fn median(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

/// Round to `d` decimals for reports.
pub(crate) fn r(x: f64, d: i32) -> f64 {
    let k = 10f64.powi(d);
    (x * k).round() / k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_skips_header_and_counts_bad_rows() {
        let (rows, bad) = numeric_csv("t,x\n0,1\n1,oops\n# c\n2,3\n", 2);
        assert_eq!(rows, vec![vec![0.0, 1.0], vec![2.0, 3.0]]);
        assert_eq!(bad, 1);
        assert_eq!(median(&[3.0, 1.0, 2.0, 10.0]), 2.5);
        assert!((std_dev(&[1.0, 2.0, 3.0]) - 1.0).abs() < 1e-12);
    }
}
