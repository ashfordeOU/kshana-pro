// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! **CI run orchestration** — drive a [`ScenarioPack`] through the open engine,
//! collect figures of merit plus provenance, and (optionally) gate the result
//! against a stored baseline, producing a [`CiReport`].
//!
//! This is the orchestrator that ties the earlier Pack C pieces together: the
//! versioned [`manifest`](crate::pack_c::manifest), the [`metrics`](crate::pack_c::metrics)
//! extractor, and the regression [`gate`](crate::pack_c::gate). For each pack
//! entry it reads the scenario file, runs it **only** through
//! [`crate::registry::run`] (the overlay never forks the open core), flattens the
//! result JSON into a [`MetricSet`], and records the engine version + scenario
//! hashes for reproducibility. The report renderer and the CLI are later concerns
//! and are *not* built here.
//!
//! Determinism: this library reads **no** clock and **no** randomness. Every
//! timestamp is caller-supplied via [`CiContext`]; the engine scenarios are
//! themselves deterministic given their seed, and the [`BTreeMap`]-backed
//! [`MetricSet`] keeps serialisation byte-stable.
//!
//! [`BTreeMap`]: std::collections::BTreeMap

use crate::pack_c::gate::{self, GateVerdict, Thresholds};
use crate::pack_c::manifest::ScenarioPack;
use crate::pack_c::metrics::{self, MetricSet};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Caller-supplied context for a CI run: the wall-clock stamp (the library reads
/// no clock), an optional customer label, and the regression-gate thresholds.
#[derive(Clone, Debug, Default)]
pub struct CiContext {
    /// Caller-supplied UTC timestamp to stamp into the report (e.g. RFC 3339).
    pub generated_utc: Option<String>,
    /// Optional customer/tenant label carried into the report.
    pub customer: Option<String>,
    /// Thresholds used when a baseline is provided to gate against.
    pub thresholds: Thresholds,
}

/// A reproducible, scored report for one CI run of a scenario pack.
///
/// Pins the pack identity + content hash, the engine version, and the per-scenario
/// hashes so a run can be reproduced exactly, alongside the flattened metrics and
/// (when a baseline was supplied) the regression-gate [`verdict`](CiReport::verdict).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CiReport {
    /// The pack's stable identifier.
    pub pack_id: String,
    /// The pack's semantic version.
    pub pack_version: String,
    /// The pack's deterministic content digest.
    pub pack_hash: String,
    /// The engine version that produced the run (first non-empty seen).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_version: Option<String>,
    /// Per-scenario content hashes, sorted and deduplicated.
    pub scenario_hashes: Vec<String>,
    /// Caller-supplied UTC timestamp, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_utc: Option<String>,
    /// Caller-supplied customer label, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    /// The flattened figures of merit collected across every scenario.
    pub metrics: MetricSet,
    /// The regression-gate verdict, present only when a baseline was supplied.
    #[serde(default)]
    pub verdict: Option<GateVerdict>,
}

impl CiReport {
    /// Serialise to deterministic pretty JSON. The [`MetricSet`] is `BTreeMap`-backed
    /// so the output is byte-stable for equal reports.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Parse a [`CiReport`] from JSON, mapping any serde error to a `String`.
    pub fn from_json(s: &str) -> Result<CiReport, String> {
        serde_json::from_str(s).map_err(|e| format!("invalid CI report: {e}"))
    }
}

/// Run every scenario in `pack` (in declaration order) through the open engine,
/// returning the merged [`MetricSet`], the sorted-deduplicated scenario hashes, and
/// the engine version (the first non-empty one seen across the runs).
///
/// Scenario files are read relative to `base_dir`. Any IO error, engine error, or
/// metric-extraction error is mapped to a descriptive `Err(String)` naming the pack
/// and the offending scenario path.
pub fn run_pack(
    pack: &ScenarioPack,
    base_dir: &Path,
) -> Result<(MetricSet, Vec<String>, Option<String>), String> {
    let mut metrics = MetricSet::default();
    let mut scenario_hashes: Vec<String> = Vec::new();
    let mut engine_version: Option<String> = None;

    for entry in &pack.entries {
        let path = base_dir.join(&entry.path);
        let src = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "pack '{}': cannot read scenario '{}': {e}",
                pack.id, entry.path
            )
        })?;

        let out = crate::registry::run(&src)
            .map_err(|e| format!("pack '{}': scenario '{}' failed: {e}", pack.id, entry.path))?;

        let extracted = metrics::extract(&entry.label(), &out.json)?;
        metrics.merge(extracted);

        // Pull top-level provenance from the result JSON (parsed once).
        let value: serde_json::Value = serde_json::from_str(&out.json).map_err(|e| {
            format!(
                "pack '{}': scenario '{}' produced invalid result JSON: {e}",
                pack.id, entry.path
            )
        })?;
        if let Some(hash) = value.get("scenario_hash").and_then(|v| v.as_str()) {
            if !hash.is_empty() {
                scenario_hashes.push(hash.to_string());
            }
        }
        if engine_version.is_none() {
            if let Some(ver) = value.get("engine_version").and_then(|v| v.as_str()) {
                if !ver.is_empty() {
                    engine_version = Some(ver.to_string());
                }
            }
        }
    }

    scenario_hashes.sort();
    scenario_hashes.dedup();

    Ok((metrics, scenario_hashes, engine_version))
}

/// Assemble a [`CiReport`] from already-collected run results — **pure**: no IO, no
/// engine, no clock. The verdict is computed only when a `baseline` is supplied, by
/// gating `metrics` against `baseline.metrics` under `ctx.thresholds`.
pub fn assemble_report(
    pack: &ScenarioPack,
    metrics: MetricSet,
    scenario_hashes: Vec<String>,
    engine_version: Option<String>,
    baseline: Option<&CiReport>,
    ctx: &CiContext,
) -> CiReport {
    let verdict = baseline.map(|b| gate::compare(&b.metrics, &metrics, &ctx.thresholds));
    CiReport {
        pack_id: pack.id.clone(),
        pack_version: pack.version.clone(),
        pack_hash: pack.pack_hash(),
        engine_version,
        scenario_hashes,
        generated_utc: ctx.generated_utc.clone(),
        customer: ctx.customer.clone(),
        metrics,
        verdict,
    }
}

/// Run a pack end to end: [`run_pack`] to collect metrics + provenance, then
/// [`assemble_report`] to build the (optionally gated) [`CiReport`].
pub fn run_ci(
    pack: &ScenarioPack,
    base_dir: &Path,
    baseline: Option<&CiReport>,
    ctx: &CiContext,
) -> Result<CiReport, String> {
    let (metrics, scenario_hashes, engine_version) = run_pack(pack, base_dir)?;
    Ok(assemble_report(
        pack,
        metrics,
        scenario_hashes,
        engine_version,
        baseline,
        ctx,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack_c::manifest::PackEntry;
    use std::collections::BTreeMap;

    /// A pack used purely to exercise the pure assembly path (never run).
    fn sample_pack() -> ScenarioPack {
        ScenarioPack {
            id: "resilience-baseline".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            entries: vec![PackEntry {
                path: "araim.toml".to_string(),
                label: Some("araim".to_string()),
            }],
        }
    }

    fn metric_set(pairs: &[(&str, f64)]) -> MetricSet {
        MetricSet(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<BTreeMap<_, _>>(),
        )
    }

    fn ctx_with_stamp() -> CiContext {
        CiContext {
            generated_utc: Some("2026-01-01T00:00:00Z".to_string()),
            customer: Some("ACME".to_string()),
            thresholds: Thresholds::new(0.05),
        }
    }

    #[test]
    fn assemble_without_baseline_has_no_verdict_and_carries_fields() {
        let pack = sample_pack();
        let metrics = metric_set(&[("araim::fom.raim_availability", 0.9)]);
        let ctx = ctx_with_stamp();

        let report = assemble_report(
            &pack,
            metrics.clone(),
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );

        assert!(report.verdict.is_none());
        assert_eq!(report.pack_id, "resilience-baseline");
        assert_eq!(report.pack_version, "1.0.0");
        assert_eq!(report.pack_hash, pack.pack_hash());
        assert_eq!(report.metrics, metrics);
        assert_eq!(
            report.generated_utc.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert_eq!(report.customer.as_deref(), Some("ACME"));
        assert_eq!(report.engine_version.as_deref(), Some("0.20.0"));
        assert_eq!(report.scenario_hashes, vec!["hash-a".to_string()]);
    }

    #[test]
    fn assemble_with_better_baseline_fails_the_gate() {
        let pack = sample_pack();
        // raim_availability is HigherIsBetter: baseline 0.9, current 0.7 => regression.
        let baseline_metrics = metric_set(&[("araim::fom.raim_availability", 0.9)]);
        let current_metrics = metric_set(&[("araim::fom.raim_availability", 0.7)]);
        let ctx = ctx_with_stamp();

        let baseline = assemble_report(
            &pack,
            baseline_metrics,
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );

        let report = assemble_report(
            &pack,
            current_metrics,
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            Some(&baseline),
            &ctx,
        );

        let verdict = report
            .verdict
            .expect("baseline supplied => verdict present");
        assert!(!verdict.passed);
    }

    #[test]
    fn assemble_with_equal_baseline_passes_the_gate() {
        let pack = sample_pack();
        let metrics = metric_set(&[("araim::fom.raim_availability", 0.9)]);
        let ctx = ctx_with_stamp();

        let baseline = assemble_report(
            &pack,
            metrics.clone(),
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );

        let report = assemble_report(
            &pack,
            metrics,
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            Some(&baseline),
            &ctx,
        );

        let verdict = report
            .verdict
            .expect("baseline supplied => verdict present");
        assert!(verdict.passed);
    }

    #[test]
    fn report_round_trips_and_to_json_is_deterministic() {
        let pack = sample_pack();
        let metrics = metric_set(&[
            ("araim::fom.raim_availability", 0.9),
            ("araim::fom.mean_hpl_m", 22.1),
        ]);
        let ctx = ctx_with_stamp();

        let baseline = assemble_report(
            &pack,
            metrics.clone(),
            vec!["hash-a".to_string()],
            Some("0.20.0".to_string()),
            None,
            &ctx,
        );
        // Include a verdict so the round-trip exercises the gated branch too.
        let report = assemble_report(
            &pack,
            metrics,
            vec!["hash-a".to_string(), "hash-b".to_string()],
            Some("0.20.0".to_string()),
            Some(&baseline),
            &ctx,
        );

        let json = report.to_json();
        let parsed = CiReport::from_json(&json).expect("report JSON must parse back");
        assert_eq!(parsed, report);

        // Determinism: two serialisations are byte-identical.
        assert_eq!(report.to_json(), report.to_json());
    }
}
