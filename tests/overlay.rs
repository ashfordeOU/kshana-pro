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

/// Enumerated strict-superset guard. The dispatcher routes only `pro-*` kinds to Pro
/// packs and delegates everything else to the embedded open engine, so the superset
/// property reduces to: *no open kind name carries the Pro prefix*. This asserts that
/// over the **entire** open catalog (not a sampled subset), so a future open kind that
/// accidentally started with `pro-` — the one way an open scenario could be shadowed
/// or gated by the overlay — would fail the build.
#[test]
fn every_open_kind_is_free_of_the_pro_prefix_so_none_can_be_shadowed() {
    use kshana_pro::registry::PRO_KIND_PREFIX;
    let kinds = kshana::api::list_scenario_kinds();
    assert!(
        kinds.len() >= 34,
        "expected the open catalog to expose at least 34 kinds, found {}",
        kinds.len()
    );
    for meta in &kinds {
        assert!(
            !meta.name.starts_with(PRO_KIND_PREFIX),
            "open kind '{}' collides with the Pro prefix '{PRO_KIND_PREFIX}' — it could be \
             shadowed by a Pro pack, breaking the strict-superset guarantee",
            meta.name
        );
    }
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

/// A non-Pro unknown kind is delegated to the open engine (which maps it to its
/// historical clock default) — it is NOT a Pro-routing concern.
#[test]
fn non_pro_unknown_kind_delegates_to_open_engine() {
    let src = "kind = \"definitely-not-a-pro-kind\"\n";
    // No `pro-` prefix → open engine. It must not produce a Pro-routing error.
    if let Err(e) = registry::run(src) {
        let msg = format!("{e}");
        assert!(
            !msg.contains("not a registered Pro pack") && !msg.contains("has no constructor"),
            "non-pro kind must not be routed to the Pro registry: {msg}"
        );
    }
}

/// THE SAFETY PROPERTY (regression test for the silent-mis-route bug): a `pro-`
/// prefixed kind that is NOT a registered Pro pack must fail closed with an honest
/// error — it must NEVER silently fall through and run as an open clock scenario.
/// This is the case that goes live the moment a future L1/L2/L3 kind appears in a
/// scenario file before its pack is registered.
#[test]
fn pro_prefixed_but_unregistered_kind_fails_closed() {
    let src = "kind = \"pro-not-registered\"\n";
    let err =
        registry::run(src).expect_err("unregistered pro- kind must error, not run as a clock");
    assert!(
        format!("{err}").contains("not a registered Pro pack"),
        "must get an explicit routing error, got: {err}"
    );
}
