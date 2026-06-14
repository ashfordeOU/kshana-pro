// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! Integration tests for the superset dispatcher: a single entry point runs both
//! Pro packs and (delegated, unchanged) open scenarios. This is the end-to-end
//! proof of the open-core overlay architecture (PR1/PR2).

use kshana_pro::registry;

/// A `pro-*` scenario routes to the Pro overlay and runs.
#[test]
fn dispatches_pro_pack() {
    let src = "kind = \"pro-seam-check\"\nlabel = \"integration\"\n";
    let out = registry::run(src).expect("pro pack must run");
    assert!(out.summary.contains("seam check OK"));
    assert!(out.summary.contains("integration"));
}

/// A real open scenario is delegated **unchanged** to the embedded open engine and
/// produces the open engine's own result — proving Pro is a superset, not a fork.
#[test]
fn delegates_open_scenario_unchanged() {
    // A real open scenario: propagate the ISS for a couple of steps via SGP4. Kept
    // short so the test is fast, but it exercises a genuine open-engine run path
    // (the open engine's own validation requires a `tle` or `orbit`).
    let src = "kind = \"ephemeris\"\n\
        tle = \"\"\"\n\
        1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
        2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
        \"\"\"\n\
        step_s = 60.0\n\
        duration_s = 120.0\n";
    let out = registry::run(src).expect("open scenario must run via delegation");
    // The result is the open engine's own ephemeris output (SGP4 propagation),
    // proving Pro delegated to — rather than reimplemented — the open engine.
    let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
    assert!(
        v["scenario_hash"].as_str().is_some(),
        "delegated open run must carry the open engine's scenario hash"
    );
    assert!(
        v["n_samples"].as_u64().unwrap_or(0) >= 2,
        "delegated ephemeris run must have propagated multiple samples"
    );
    assert!(
        v["samples"].is_array(),
        "delegated open run must carry its sample series"
    );
}

/// An unknown / no kind falls through to the open engine (its historical default),
/// never to the Pro overlay.
#[test]
fn unknown_kind_falls_through_to_open_engine() {
    // No `kind` field → open engine's default (clock) path; we only assert it does
    // not error out of the dispatcher with a Pro-routing failure.
    let src = "kind = \"definitely-not-a-pro-kind\"\n";
    // The open engine treats unknown as its default pack; result may be Ok or a
    // structured open-engine error, but it must NOT be the Pro registry-desync error.
    if let Err(e) = registry::run(src) {
        assert!(
            !format!("{e}").contains("registered but has no constructor"),
            "unknown kind must not be routed to the Pro registry"
        );
    }
}
