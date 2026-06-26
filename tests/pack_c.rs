// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! End-to-end integration for the Pack C CI orchestrator: parse a pack manifest,
//! run its scenarios through the **real** open engine, and assert the reproducible
//! scored report plus the regression-gate path.
//!
//! Fixtures live under `tests/fixtures/pack-c/`. They are verbatim copies of two
//! fast open scenarios so the test never reaches outside the crate at runtime:
//!   * `araim.toml` — a measurement-domain `gnss-sim` ARAIM run (~0.4 s) that emits
//!     a top-level `engine_version` + `scenario_hash` and known-direction FoMs
//!     (`raim_availability` higher-is-better, `mean_hpl_m` lower-is-better), so it
//!     drives the gate; and
//!   * `impairment.toml` — a fast `impairment-eval` run (~30 ms) that adds a second
//!     `scenario_hash` (no `engine_version`), proving the orchestrator picks the
//!     first non-empty engine version across differing scenarios.

use kshana_pro::pack_c::{parse_pack, run_ci, CiContext, MetricSet, Thresholds};
use std::collections::BTreeMap;
use std::path::Path;

/// The directory holding the pack manifest and its scenario fixtures, relative to
/// the crate root (the cwd for integration tests).
const FIXTURE_DIR: &str = "tests/fixtures/pack-c";

/// The metric key the gate tests degrade/improve — known higher-is-better.
const AVAIL_KEY: &str = "araim::fom.raim_availability";

fn read_pack() -> kshana_pro::pack_c::ScenarioPack {
    let manifest = std::fs::read_to_string(format!("{FIXTURE_DIR}/pack.toml"))
        .expect("fixture pack manifest must be readable");
    parse_pack(&manifest).expect("fixture pack manifest must parse")
}

fn ctx() -> CiContext {
    CiContext {
        generated_utc: Some("2026-01-01T00:00:00Z".to_string()),
        customer: Some("Integration Test Customer".to_string()),
        thresholds: Thresholds::new(0.05),
    }
}

/// Test A: a baseline-free run produces a populated, ungated report.
#[test]
fn run_ci_without_baseline_produces_scored_report() {
    let pack = read_pack();
    let report = run_ci(&pack, Path::new(FIXTURE_DIR), None, &ctx())
        .expect("baseline-free run must succeed");

    assert_eq!(report.pack_id, "resilience-baseline-test");
    assert_eq!(report.pack_version, "1.0.0");
    assert_eq!(report.pack_hash, pack.pack_hash());

    assert!(!report.metrics.is_empty(), "metrics must be collected");
    assert!(
        report.engine_version.is_some(),
        "at least one scenario emits an engine_version"
    );
    assert!(
        !report.scenario_hashes.is_empty(),
        "scenario hashes must be recorded"
    );
    // The two fixtures emit distinct hashes.
    assert_eq!(
        report.scenario_hashes.len(),
        2,
        "two distinct scenario hashes"
    );
    assert!(report.verdict.is_none(), "no baseline => no verdict");

    // The known-direction FoM is present (so the gate tests below are meaningful).
    assert!(
        report.metrics.get(AVAIL_KEY).is_some(),
        "ARAIM availability FoM must be extracted under its label"
    );
}

/// Test B: gating against a baseline produces a verdict reflecting the comparison.
#[test]
fn run_ci_with_baseline_gates_end_to_end() {
    let pack = read_pack();
    let current =
        run_ci(&pack, Path::new(FIXTURE_DIR), None, &ctx()).expect("initial run must succeed");
    let avail = current
        .metrics
        .get(AVAIL_KEY)
        .expect("availability FoM present");

    // Baseline that is BETTER than the current run on a higher-is-better metric:
    // the current run is then a regression, so the gate must fail.
    let mut worse_for_current = current.clone();
    set_metric(&mut worse_for_current.metrics, AVAIL_KEY, avail + 0.2);
    let regressed = run_ci(
        &pack,
        Path::new(FIXTURE_DIR),
        Some(&worse_for_current),
        &ctx(),
    )
    .expect("gated run must succeed");
    let verdict = regressed
        .verdict
        .expect("baseline supplied => verdict present");
    assert!(
        !verdict.passed,
        "current is worse than baseline => gate fails"
    );
    assert!(
        verdict.regressions.iter().any(|r| r.key == AVAIL_KEY),
        "the degraded availability metric must be flagged"
    );

    // Baseline that is WORSE than the current run: the current run is an
    // improvement, so the gate must pass.
    let mut better_for_current = current.clone();
    set_metric(
        &mut better_for_current.metrics,
        AVAIL_KEY,
        (avail - 0.2).max(0.0),
    );
    let improved = run_ci(
        &pack,
        Path::new(FIXTURE_DIR),
        Some(&better_for_current),
        &ctx(),
    )
    .expect("gated run must succeed");
    let verdict = improved
        .verdict
        .expect("baseline supplied => verdict present");
    assert!(
        verdict.passed,
        "current is better than baseline => gate passes"
    );
}

/// Test C: two runs with the same context serialise byte-identically (determinism).
#[test]
fn run_ci_is_deterministic_across_runs() {
    let pack = read_pack();
    let a = run_ci(&pack, Path::new(FIXTURE_DIR), None, &ctx()).expect("first run must succeed");
    let b = run_ci(&pack, Path::new(FIXTURE_DIR), None, &ctx()).expect("second run must succeed");
    assert_eq!(
        a.to_json(),
        b.to_json(),
        "two runs with identical context must be byte-identical"
    );
}

/// Overwrite a single metric value in a [`MetricSet`] for the gate fixtures.
fn set_metric(metrics: &mut MetricSet, key: &str, value: f64) {
    let mut overlay = BTreeMap::new();
    overlay.insert(key.to_string(), value);
    metrics.merge(MetricSet(overlay));
}
