// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Validation against reality (stream D-core): the tools that turn recordings, measurements
//! and independent reference data into evidence for (or against) the crate's models.
//!
//! * [`fidelity`] (D2): the automatic fidelity comparator. A recording plus its capture
//!   metadata is compared with a simulation of the same interval (scenario built from a
//!   given scenario or from the GRI database), both run through the software receiver, at
//!   three levels: signal (averaged pulse envelope, spectrum, noise floor), observable (ToA,
//!   ECD, SNR, field strength per station) and solution (position, time). Output: a
//!   [`fidelity::FidelityReport`] (serde JSON) with per-metric pass/fail and a score.
//! * [`inverse`] (D3): inverse modelling. Per-station ASF from ToA at known positions,
//!   transmitter timing offsets from multi-site data, effective ground conductivity of a
//!   path segment (golden-section fit of Millington ASF), skywave delay and ratio (two-path
//!   fit of averaged pulses). Levenberg–Marquardt / Gauss–Newton and golden-section searches
//!   are implemented there; every estimate carries an uncertainty.
//! * [`crosscheck`] (D5): independent cross-check harnesses. GRWAVE output tables and an
//!   optional GRWAVE runner, digitised ITU-R P.368 / P.684 curves, and published ASF/ECD
//!   survey points, each compared with the model under stated tolerances.
//! * [`selfcal`] (A7, gap 26): the self-calibrating simulator loop. Generate a scenario,
//!   compare reference-receiver observations with the simulator's prediction, update
//!   propagation parameters (σ per path section, ASF bias, skywave height) by inverse
//!   modelling with uncertainty, regenerate, repeat; every iteration is a hash-chained
//!   calibration-register entry and the lineage is signed (HMAC-SHA256).
//!
//! # Evidence labels
//!
//! Every comparison report can produce a validation-ledger row ([`LedgerRow`], markdown in
//! the column layout of `docs/VALIDATION.md`). The label is decided by [`decide_label`]: a
//! model becomes **VALIDATED** only when the comparison passes **and** the reference is
//! independent (published reference values, an independent program's output, or measured
//! data the model was not tuned to) **and** there are enough points. Synthetic data (all the
//! data shipped with this module's tests) and data the model was tuned to never give
//! VALIDATED: the row stays MODELLED and says why.
//!
//! The comparison machinery itself is MODELLED (internally verified: the tests feed it
//! synthetic data with known perturbations and check that it recovers them).

pub mod crosscheck;
pub mod fidelity;
pub mod inverse;
#[cfg(feature = "core-methods")]
pub mod selfcal;

use serde::{Deserialize, Serialize};

/// Summary statistics of a sample of values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub n: usize,
    pub mean: f64,
    /// Sample standard deviation (n − 1), 0 for n < 2.
    pub std: f64,
    pub rms: f64,
    pub max_abs: f64,
}

/// Statistics of `v` (non-finite values are skipped).
pub fn stats(v: &[f64]) -> Stats {
    let x: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
    let n = x.len();
    if n == 0 {
        return Stats::default();
    }
    let nf = n as f64;
    let mean = x.iter().sum::<f64>() / nf;
    let std = if n > 1 {
        (x.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / (nf - 1.0)).sqrt()
    } else {
        0.0
    };
    Stats {
        n,
        mean,
        std,
        rms: (x.iter().map(|a| a * a).sum::<f64>() / nf).sqrt(),
        max_abs: x.iter().fold(0.0, |a, b| a.max(b.abs())),
    }
}

/// Deserialise an `f64` written by serde_json, where non-finite values become `null`:
/// `null` reads back as NaN.
pub(crate) fn de_f64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
}

/// Evidence label of a ledger row (see `docs/VALIDATION.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Label {
    #[serde(rename = "VALIDATED")]
    Validated,
    #[serde(rename = "MODELLED")]
    Modelled,
}

impl Label {
    pub fn as_str(self) -> &'static str {
        match self {
            Label::Validated => "VALIDATED",
            Label::Modelled => "MODELLED",
        }
    }
}

/// What a comparison's reference data is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    /// Published reference values (specification, ITU curve, published survey).
    IndependentPublished,
    /// Output of an independent program (e.g. the ITU GRWAVE program).
    IndependentProgram,
    /// Measured data (a recording, a survey) the model was not tuned to.
    IndependentMeasured,
    /// Data the model was calibrated/tuned to: agreement is a calibration, not validation.
    TunedTo,
    /// Synthetic data (generated by this crate or invented for a test).
    Synthetic,
}

impl ReferenceKind {
    pub fn is_independent(self) -> bool {
        matches!(
            self,
            ReferenceKind::IndependentPublished
                | ReferenceKind::IndependentProgram
                | ReferenceKind::IndependentMeasured
        )
    }
}

/// The reference a comparison was made against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// Citation: document, figure/table, program version, survey, capture id.
    pub citation: String,
    pub kind: ReferenceKind,
    /// Minimum number of compared points for a VALIDATED label (default 5).
    #[serde(default = "d_min_points")]
    pub min_points: usize,
}

fn d_min_points() -> usize {
    5
}

impl Reference {
    pub fn new(citation: &str, kind: ReferenceKind) -> Reference {
        Reference {
            citation: citation.into(),
            kind,
            min_points: d_min_points(),
        }
    }
    /// A synthetic reference (tests, demonstrations).
    pub fn synthetic(what: &str) -> Reference {
        Reference::new(&format!("SYNTHETIC: {what}"), ReferenceKind::Synthetic)
    }
}

/// Label decision: VALIDATED only for a passing comparison against an independent reference
/// with at least `min_points` points. Returns the label and the reason.
pub fn decide_label(pass: bool, n_points: usize, reference: &Reference) -> (Label, String) {
    if !pass {
        return (Label::Modelled, "comparison failed its tolerance".into());
    }
    match reference.kind {
        ReferenceKind::Synthetic => (
            Label::Modelled,
            "synthetic reference: harness check only, not evidence".into(),
        ),
        ReferenceKind::TunedTo => (
            Label::Modelled,
            "the model was tuned to this reference: calibration, not validation".into(),
        ),
        _ if n_points < reference.min_points => (
            Label::Modelled,
            format!(
                "only {n_points} points compared (< {} required)",
                reference.min_points
            ),
        ),
        _ => (
            Label::Validated,
            "independent reference reproduced within tolerance".into(),
        ),
    }
}

/// One generated row of the validation ledger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LedgerRow {
    pub model: String,
    pub label: Label,
    pub reference: String,
    pub test: String,
    pub tolerance: String,
    pub result: String,
    /// Why the label was chosen.
    pub reason: String,
}

impl LedgerRow {
    /// Build a row, deciding the label with [`decide_label`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        model: &str,
        reference: &Reference,
        test: &str,
        tolerance: &str,
        result: &str,
        pass: bool,
        n_points: usize,
    ) -> LedgerRow {
        let (label, reason) = decide_label(pass, n_points, reference);
        LedgerRow {
            model: model.into(),
            label,
            reference: reference.citation.clone(),
            test: test.into(),
            tolerance: tolerance.into(),
            result: result.into(),
            reason,
        }
    }

    /// Markdown table row in the ledger's column order (Model / capability | Label |
    /// Reference | Test | Tolerance | Known limitations); the last column carries the result
    /// and the reason for the label.
    pub fn to_markdown(&self) -> String {
        let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
        format!(
            "| {} | {} | {} | {} | {} | Result: {} ({}) |",
            esc(&self.model),
            self.label.as_str(),
            esc(&self.reference),
            esc(&self.test),
            esc(&self.tolerance),
            esc(&self.result),
            esc(&self.reason)
        )
    }
}

/// Parse a CSV line (comma separated, optional double quotes around fields).
pub(crate) fn csv_fields(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_basic() {
        let s = stats(&[1.0, 2.0, 3.0, f64::NAN]);
        assert_eq!(s.n, 3);
        assert!((s.mean - 2.0).abs() < 1e-12);
        assert!((s.std - 1.0).abs() < 1e-12);
        assert!((s.max_abs - 3.0).abs() < 1e-12);
    }

    #[test]
    fn label_rules_are_honest() {
        let ind = Reference::new("ITU-R P.368 Fig. 1", ReferenceKind::IndependentPublished);
        assert_eq!(decide_label(true, 10, &ind).0, Label::Validated);
        assert_eq!(decide_label(false, 10, &ind).0, Label::Modelled);
        assert_eq!(decide_label(true, 2, &ind).0, Label::Modelled);
        assert_eq!(
            decide_label(true, 100, &Reference::synthetic("x")).0,
            Label::Modelled
        );
        let tuned = Reference::new("Harris polynomial", ReferenceKind::TunedTo);
        assert_eq!(decide_label(true, 100, &tuned).0, Label::Modelled);
        let row = LedgerRow::new("m|x", &ind, "t", "1 dB", "0.3 dB", true, 10);
        let md = row.to_markdown();
        assert!(
            md.starts_with("| m\\|x | VALIDATED | ITU-R P.368 Fig. 1 |"),
            "{md}"
        );
        assert_eq!(md.matches(" | ").count(), 5);
    }

    #[test]
    fn csv_fields_quotes() {
        assert_eq!(csv_fields(r#"a, "b,c" ,"d""e""#), vec!["a", "b,c", "d\"e"]);
    }
}
