// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # NAV-DT performance/resilience twin pipeline (A10)
//!
//! Runs the **open-core spine** end-to-end for one scenario and assembles a single,
//! hash-stamped, label-honest bundle that is the canonical *NAV-DT performance-
//! prediction* output: ranging/timing accuracy, availability, integrity, and a
//! resilience-vs-time figure of merit, each tagged with its `MODELLED`/`VALIDATED`
//! provenance pulled from the open verification matrix.
//!
//! Reproducibility comes from the open engine's `report::hash_scenario` stamp
//! (surfaced as `scenario_hash` in the result), so the same scenario reproduces the
//! same bundle bit-for-bit.
//!
//! ## Honesty guard (enforced by tests)
//! - **Kshana is ONE COMPONENT of the twin, never the whole twin** —
//!   [`NavDtBundle::kshana_is_one_component`] is always `true` and the note says so.
//! - Every modelled FoM stays tagged `MODELLED`; none is upgraded to `VALIDATED`
//!   without a real external oracle (the label is pulled from
//!   [`crate::benefits::fom_provenance`], the same honest source the trade engine uses).
//! - No flight / in-orbit-validated claims.

use kshana::api::{run_scenario, KshanaError};
use serde::Serialize;

use crate::benefits::fom_provenance;
use crate::util::dot_path_number;

/// Bundle schema version.
pub const BUNDLE_VERSION: &str = "1.0.0";

/// One labelled figure of merit in the bundle.
#[derive(Debug, Clone, Serialize)]
pub struct LabeledFom {
    pub name: String,
    pub key: String,
    pub value: Option<f64>,
    /// `MODELLED` / `VALIDATED` / `PARTNER`, pulled live from the verification matrix.
    pub label: String,
    pub oracle: String,
}

/// The canonical NAV-DT performance-prediction bundle for one scenario.
#[derive(Debug, Clone, Serialize)]
pub struct NavDtBundle {
    pub bundle_version: String,
    pub engine_version: String,
    /// The open engine's reproducible scenario hash (`None` if the run produced none).
    pub scenario_hash: Option<String>,
    /// Always `true` — Kshana is one work-package component of a partner-led NAV-DT.
    pub kshana_is_one_component: bool,
    /// Ranging/timing accuracy FoMs.
    pub ranging_timing: Vec<LabeledFom>,
    /// Service availability.
    pub availability: LabeledFom,
    /// Estimator integrity (filter self-consistency — NOT aviation HPL/VPL).
    pub integrity: LabeledFom,
    /// Resilience-vs-time FoMs (holdover + degradation slope).
    pub resilience: Vec<LabeledFom>,
    pub note: String,
}

/// Build one labelled FoM from the result JSON and the honest verification label.
fn labeled(json: &serde_json::Value, name: &str, key: &str) -> LabeledFom {
    let (status, oracle) = fom_provenance(key);
    LabeledFom {
        name: name.into(),
        key: key.into(),
        value: dot_path_number(json, key),
        label: status.tag().into(),
        oracle: format!("{oracle:?}"),
    }
}

/// Run the open-core spine for `scenario_toml` and assemble the NAV-DT bundle. The
/// `subject` selects which result branch to read (e.g. `"quantum"` or `"classical"`
/// for a clock-holdover scenario).
pub fn run_navdt_bundle(scenario_toml: &str, subject: &str) -> Result<NavDtBundle, KshanaError> {
    if subject.trim().is_empty() {
        return Err(KshanaError::InvalidInput(
            "NAV-DT bundle needs a non-empty result subject (e.g. \"quantum\")".into(),
        ));
    }
    let out = run_scenario(scenario_toml)?;
    let json: serde_json::Value = serde_json::from_str(&out.json)
        .map_err(|e| KshanaError::InvalidInput(format!("result JSON did not parse: {e}")))?;
    Ok(bundle_from_json(&json, subject))
}

/// Build the bundle from an already-parsed open result JSON. Lets a caller that has
/// already run the scenario (e.g. the consolidated report) reuse the result instead
/// of running it twice.
pub fn bundle_from_json(json: &serde_json::Value, subject: &str) -> NavDtBundle {
    let scenario_hash = json
        .get("scenario_hash")
        .and_then(|h| h.as_str())
        .map(String::from);
    // The open engine stamps its own version into the result; surface it (the open
    // engine version, distinct from this overlay crate's version).
    let engine_version = json
        .get("engine_version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let k = |leaf: &str| format!("{subject}.fom.{leaf}");
    NavDtBundle {
        bundle_version: BUNDLE_VERSION.into(),
        engine_version,
        scenario_hash,
        kshana_is_one_component: true,
        ranging_timing: vec![
            labeled(json, "timing RMS (ns)", &k("timing_rms_ns")),
            labeled(json, "timing p95 (ns)", &k("timing_p95_ns")),
        ],
        availability: labeled(json, "availability", &k("availability")),
        integrity: labeled(json, "filter self-consistency", &k("integrity")),
        resilience: vec![
            labeled(json, "holdover (s)", &k("holdover_s")),
            labeled(
                json,
                "degradation slope (ns/s)",
                &k("resilience_slope_ns_per_s"),
            ),
        ],
        note: "NAV-DT performance prediction. Kshana provides ONE component (the \
               performance/resilience spine) of a partner-led Navigation Digital Twin. \
               FoMs labelled MODELLED are simulation results, not flight/in-orbit validated."
            .into(),
    }
}

impl NavDtBundle {
    /// All FoMs in the bundle, flattened (for rendering / coverage checks).
    pub fn all_foms(&self) -> Vec<&LabeledFom> {
        self.ranging_timing
            .iter()
            .chain(std::iter::once(&self.availability))
            .chain(std::iter::once(&self.integrity))
            .chain(self.resilience.iter())
            .collect()
    }

    /// Reproducible JSON artifact.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("NavDtBundle serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock_scenario() -> String {
        "seed = 42\n\
         threshold_ns = 20.0\n\
         [time]\n\
         step_s = 10.0\n\
         duration_s = 7200.0\n\
         [gnss]\n\
         windows = [\n\
           { t0 = 0.0,   t1 = 600.0,  state = \"nominal\" },\n\
           { t0 = 600.0, t1 = 7200.0, state = \"denied\" },\n\
         ]\n\
         [clock_quantum]\n\
         id = \"optical-sr-lattice\"\n\
         provenance = \"demo; q_wf=sigma_y(1s)^2\"\n\
         y0   = 5.0e-17\n\
         q_wf = 1.0e-30\n\
         q_rw = 0.0\n\
         [clock_classical]\n\
         id = \"csac-sa45s\"\n\
         provenance = \"demo; q_wf=sigma_y(1s)^2\"\n\
         y0   = 5.0e-10\n\
         q_wf = 9.0e-20\n\
         q_rw = 0.0\n"
            .into()
    }

    #[test]
    fn bundle_covers_all_four_fom_families() {
        let b = run_navdt_bundle(&clock_scenario(), "quantum").unwrap();
        assert_eq!(b.ranging_timing.len(), 2);
        assert_eq!(b.resilience.len(), 2);
        // Every FoM resolved a value from the open run.
        for fom in b.all_foms() {
            assert!(fom.value.is_some(), "FoM {} had no value", fom.key);
        }
        assert!(
            b.scenario_hash.is_some(),
            "bundle must carry a reproducible hash"
        );
    }

    #[test]
    fn bundle_is_deterministic() {
        let a = run_navdt_bundle(&clock_scenario(), "quantum").unwrap();
        let b = run_navdt_bundle(&clock_scenario(), "quantum").unwrap();
        // Same hash and same FoM values on a re-run (reproducibility invariant).
        assert_eq!(a.scenario_hash, b.scenario_hash);
        assert_eq!(a.to_json(), b.to_json());
    }

    #[test]
    fn every_fom_is_honestly_labelled_and_kshana_is_one_component() {
        let b = run_navdt_bundle(&clock_scenario(), "quantum").unwrap();
        assert!(b.kshana_is_one_component);
        assert!(b.note.contains("ONE component"));
        // For the clock-holdover spine, every FoM is MODELLED (none silently VALIDATED).
        for fom in b.all_foms() {
            assert_eq!(
                fom.label, "MODELLED",
                "FoM {} should be MODELLED for this spine, got {}",
                fom.key, fom.label
            );
        }
    }

    #[test]
    fn integrity_is_self_consistency_modelled_not_aviation_validated() {
        let b = run_navdt_bundle(&clock_scenario(), "quantum").unwrap();
        // The bundle's integrity is the filter self-consistency FoM — MODELLED.
        assert_eq!(b.integrity.label, "MODELLED");
        assert!(b.integrity.name.contains("self-consistency"));
    }

    #[test]
    fn empty_subject_is_an_error() {
        assert!(matches!(
            run_navdt_bundle(&clock_scenario(), ""),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn bad_scenario_propagates_an_error() {
        assert!(run_navdt_bundle("kind = \"not-a-real-kind\"\n", "quantum").is_err());
    }
}
