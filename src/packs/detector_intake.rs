// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Customer-detector **intake** for Pack A — the documented format a customer uses
//! to submit THEIR detector's scored outputs so we compute their field-vs-bench
//! (ID-vs-OOD) optimism gap.
//!
//! A customer reduces their detector to one scalar score per labelled sample, tagged
//! with the distribution the sample came from (`id` = the in-distribution / bench
//! regime, `ood` = the out-of-distribution / field regime). We then compute the AUC
//! over each subset and report the gap `auc_id − auc_ood`.
//!
//! The AUC is **not** re-derived here — it is the open engine's exact Mann–Whitney
//! routine ([`kshana::impairment_eval::auc`]), the same one the published study and
//! the [`crate::packs::optimism_gap_audit`] baseline use, so a customer's number is
//! computed identically to ours.
//!
//! ## Honest scope
//! This computes operating characteristics over the customer's own labelled data; it
//! is a MODELLED engineering measurement, **not a certification**. The schema is
//! documented in `docs/packs/DETECTOR-INTAKE-FORMAT.md`.

use kshana::impairment_eval::auc;
use serde::{Deserialize, Serialize};

/// Which distribution a submitted sample was drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    /// In-distribution / bench regime (the tuned, nominal conditions).
    Id,
    /// Out-of-distribution / field regime (the shifted, subtler conditions).
    Ood,
}

/// One scored, labelled sample from a customer's detector.
///
/// `score` must be oriented so **higher means more likely impairment-present** (the
/// convention the open [`auc`] routine assumes). `label = true` means the impairment
/// is genuinely present (a positive); `false` is a clean/negative sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredSample {
    /// The detector's scalar decision statistic (higher ⇒ more impaired).
    pub score: f64,
    /// Ground truth: `true` = impairment present (positive), `false` = clean (negative).
    pub label: bool,
    /// Which distribution this sample came from (`"id"` or `"ood"`).
    pub distribution: Distribution,
}

/// The computed field-vs-bench gap for a customer's submitted samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetectorGap {
    /// AUC over the in-distribution (bench) subset.
    pub auc_id: f64,
    /// AUC over the out-of-distribution (field) subset.
    pub auc_ood: f64,
    /// The optimism gap `auc_id − auc_ood` (positive ⇒ the bench number is optimistic).
    pub optimism_gap: f64,
    /// Number of in-distribution samples used.
    pub n_id: usize,
    /// Number of out-of-distribution samples used.
    pub n_ood: usize,
}

/// The top-level intake document a customer submits (a list of scored samples).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectorIntake {
    /// The scored, labelled samples.
    #[serde(default)]
    pub samples: Vec<ScoredSample>,
}

impl DetectorIntake {
    /// Compute the field-vs-bench gap from this intake document.
    pub fn gap(&self) -> Result<DetectorGap, String> {
        gap_from_samples(&self.samples)
    }
}

/// Compute one distribution subset's AUC (positives vs negatives), validating that
/// both classes are present and scores are finite. Reuses the open engine's [`auc`].
fn subset_auc(
    samples: &[ScoredSample],
    dist: Distribution,
    which: &str,
) -> Result<(f64, usize), String> {
    let subset: Vec<&ScoredSample> = samples.iter().filter(|s| s.distribution == dist).collect();
    if subset.is_empty() {
        return Err(format!("no {which} samples submitted"));
    }
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for s in &subset {
        if !s.score.is_finite() {
            return Err(format!("{which} subset contains a non-finite score"));
        }
        if s.label {
            pos.push(s.score);
        } else {
            neg.push(s.score);
        }
    }
    if pos.is_empty() || neg.is_empty() {
        return Err(format!(
            "{which} subset is degenerate (all one class): {} positive(s), {} negative(s) — AUC needs both",
            pos.len(),
            neg.len()
        ));
    }
    let a = auc(&pos, &neg);
    if !a.is_finite() {
        return Err(format!("{which} subset produced a non-finite AUC"));
    }
    Ok((a, subset.len()))
}

/// Compute the customer's field-vs-bench optimism gap from a flat list of scored
/// samples: AUC over the `id` (bench) subset, AUC over the `ood` (field) subset, and
/// `optimism_gap = auc_id − auc_ood`. Rejects empty / single-distribution /
/// single-class / non-finite input with a clear error. The AUC is the open engine's
/// exact Mann–Whitney routine — no math is re-derived.
pub fn gap_from_samples(samples: &[ScoredSample]) -> Result<DetectorGap, String> {
    if samples.is_empty() {
        return Err("no samples submitted (the intake is empty)".into());
    }
    let (auc_id, n_id) = subset_auc(samples, Distribution::Id, "id")?;
    let (auc_ood, n_ood) = subset_auc(samples, Distribution::Ood, "ood")?;
    Ok(DetectorGap {
        auc_id,
        auc_ood,
        optimism_gap: auc_id - auc_ood,
        n_id,
        n_ood,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(score: f64, label: bool, distribution: Distribution) -> ScoredSample {
        ScoredSample {
            score,
            label,
            distribution,
        }
    }

    #[test]
    fn hand_built_samples_yield_a_sane_positive_gap() {
        // ID (bench): cleanly separable → AUC = 1.0.
        // OOD (field): positives and negatives overlap → AUC < 1.0.
        let samples = vec![
            s(9.0, true, Distribution::Id),
            s(8.0, true, Distribution::Id),
            s(2.0, false, Distribution::Id),
            s(1.0, false, Distribution::Id),
            // OOD: one positive drops into the negative range (overlap).
            s(7.0, true, Distribution::Ood),
            s(3.0, true, Distribution::Ood),
            s(4.0, false, Distribution::Ood),
            s(2.0, false, Distribution::Ood),
        ];
        let g = gap_from_samples(&samples).expect("computes");
        assert!(
            (g.auc_id - 1.0).abs() < 1e-12,
            "bench AUC {} should be 1.0",
            g.auc_id
        );
        assert!(
            g.auc_ood < 1.0,
            "field AUC {} should be below bench",
            g.auc_ood
        );
        assert!(
            g.optimism_gap > 0.0,
            "gap {} should be positive",
            g.optimism_gap
        );
        assert!((g.optimism_gap - (g.auc_id - g.auc_ood)).abs() < 1e-12);
        assert_eq!(g.n_id, 4);
        assert_eq!(g.n_ood, 4);
    }

    #[test]
    fn matches_open_engine_auc_exactly() {
        // The intake AUC must equal the open engine's auc() on the same scores —
        // proving we reuse the engine, not a re-derivation.
        let samples = vec![
            s(5.0, true, Distribution::Id),
            s(6.0, true, Distribution::Id),
            s(0.0, false, Distribution::Id),
            s(1.0, false, Distribution::Id),
            s(2.0, false, Distribution::Id),
            s(4.0, true, Distribution::Ood),
            s(3.0, false, Distribution::Ood),
        ];
        let g = gap_from_samples(&samples).unwrap();
        let want_id = auc(&[5.0, 6.0], &[0.0, 1.0, 2.0]);
        let want_ood = auc(&[4.0], &[3.0]);
        assert_eq!(g.auc_id.to_bits(), want_id.to_bits());
        assert_eq!(g.auc_ood.to_bits(), want_ood.to_bits());
    }

    #[test]
    fn empty_input_errors() {
        let err = gap_from_samples(&[]).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn missing_distribution_errors() {
        // Only ID samples → no OOD subset.
        let samples = vec![
            s(9.0, true, Distribution::Id),
            s(1.0, false, Distribution::Id),
        ];
        let err = gap_from_samples(&samples).unwrap_err();
        assert!(err.contains("no ood samples"), "got: {err}");
    }

    #[test]
    fn single_class_subset_errors() {
        // OOD subset has only positives → degenerate AUC.
        let samples = vec![
            s(9.0, true, Distribution::Id),
            s(1.0, false, Distribution::Id),
            s(5.0, true, Distribution::Ood),
            s(6.0, true, Distribution::Ood),
        ];
        let err = gap_from_samples(&samples).unwrap_err();
        assert!(err.contains("degenerate"), "got: {err}");
    }

    #[test]
    fn non_finite_score_errors() {
        let samples = vec![
            s(f64::NAN, true, Distribution::Id),
            s(1.0, false, Distribution::Id),
            s(5.0, true, Distribution::Ood),
            s(2.0, false, Distribution::Ood),
        ];
        let err = gap_from_samples(&samples).unwrap_err();
        assert!(err.contains("non-finite"), "got: {err}");
    }

    #[test]
    fn json_round_trips_with_lowercase_distribution() {
        let doc: DetectorIntake = serde_json::from_str(
            r#"{ "samples": [
                { "score": 9.0, "label": true,  "distribution": "id" },
                { "score": 1.0, "label": false, "distribution": "id" },
                { "score": 5.0, "label": true,  "distribution": "ood" },
                { "score": 2.0, "label": false, "distribution": "ood" }
            ] }"#,
        )
        .expect("intake JSON parses");
        let g = doc.gap().expect("computes");
        assert_eq!(g.n_id, 2);
        assert_eq!(g.n_ood, 2);
        assert!(g.optimism_gap >= 0.0);
    }
}
