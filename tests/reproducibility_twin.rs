// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Reproducibility goldens + CI metric gate for the NAV-DT twin (A17).
//!
//! These pin our **own deterministic outputs** — which is reproducibility, **NOT**
//! external validation. A passing gate here is not ESA acceptance or TRL maturation;
//! it only proves a reviewer can independently reproduce every headline number.
//!
//! - `scenario_hash` is the open engine's SHA-256 over the canonical scenario
//!   *inputs* — platform-independent, so it is pinned as a cross-platform golden.
//! - The report fingerprint and FoM values are checked for **in-process
//!   determinism**; their byte-identical cross-platform reproduction rests on the
//!   open core's reproducibility invariant (its own `cross_platform_golden` suite).

// Arksa-exclusive (docs/PROTECTION.md): runs only in a build with the `arksa` feature.
#![cfg(feature = "arksa")]

use kshana_pro::connectors::spacestream::{golden_messages, SpaceStreamMessage};
use kshana_pro::report::{build_report, GateCheck};
use kshana_pro::twin::navdt::run_navdt_bundle;

use kshana::api::run_scenario; // open engine, for GateOp re-export proximity
use kshana_pro::connectors::GateOp;

/// The fixed golden scenario for the twin reproducibility vectors.
fn golden_scenario() -> String {
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

/// The pinned cross-platform golden scenario hash (SHA-256 over the scenario inputs).
/// Stable across platforms because it hashes inputs, not float outputs.
const GOLDEN_SCENARIO_HASH: &str =
    "c12a1d2454af057176b6fa15c26f70509b6abb1c85f198bf03ddecfb6cc52f31";

#[test]
fn twin_bundle_scenario_hash_matches_cross_platform_golden() {
    let b = run_navdt_bundle(&golden_scenario(), "quantum").unwrap();
    let hash = b.scenario_hash.expect("bundle carries a scenario hash");
    if std::env::var("KSHANA_PRINT_GOLDEN").is_ok() {
        eprintln!("GOLDEN_SCENARIO_HASH = {hash}");
    }
    assert_eq!(hash.len(), 64, "scenario hash is a 64-hex SHA-256");
    assert_eq!(
        hash, GOLDEN_SCENARIO_HASH,
        "scenario hash drifted from the cross-platform golden (inputs changed?)"
    );
}

#[test]
fn twin_bundle_and_report_are_deterministic_in_process() {
    let scn = golden_scenario();
    let a = run_navdt_bundle(&scn, "quantum").unwrap();
    let b = run_navdt_bundle(&scn, "quantum").unwrap();
    assert_eq!(a.to_json(), b.to_json(), "bundle must be reproducible");

    let ra = build_report("twin golden", &scn, "quantum").unwrap();
    let rb = build_report("twin golden", &scn, "quantum").unwrap();
    assert_eq!(
        ra.fingerprint(),
        rb.fingerprint(),
        "consolidated report fingerprint must be reproducible"
    );
    assert_eq!(ra.to_evidence_json(), rb.to_evidence_json());
}

#[test]
fn spacestream_goldens_are_reproducible_and_aggregated_only() {
    for (name, msg) in golden_messages() {
        let json = msg.to_json();
        let back = SpaceStreamMessage::parse(&json).unwrap();
        assert_eq!(back, msg, "{name} drifted across the codec");
        // The aggregated-only guarantee holds on the wire form.
        assert!(!json.contains("\"iq\"") && !json.contains("\"samples\""));
    }
}

#[test]
fn metric_gate_reproduces_a_pass_and_a_fail() {
    let report = build_report("gate golden", &golden_scenario(), "quantum").unwrap();
    // The optical clock holds the full outage → availability is high.
    let pass = report.gate(&[GateCheck::new("availability", GateOp::Ge, 0.5)]);
    assert!(pass.passed, "expected gate pass, got {:?}", pass.failures);
    // An impossible threshold reproduces a deterministic fail.
    let fail = report.gate(&[GateCheck::new("availability", GateOp::Gt, 1.5)]);
    assert!(!fail.passed);
    // The gate's honesty note is on the outcome.
    assert!(pass.note.contains("NOT external validation"));
}

#[test]
fn open_engine_still_runs_the_golden_scenario() {
    // Sanity: the open engine produces a result for the golden scenario (the spine
    // the whole bundle composes).
    assert!(run_scenario(&golden_scenario()).is_ok());
}
