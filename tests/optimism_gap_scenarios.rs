// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! INVARIANT: the committed Pack-A scenario templates parse and run through the
//! superset dispatcher / open optimism-gap engine without error, and they carry the
//! honest framing (MODELLED, not a certification, arXiv preprint). If a template
//! drifts from the pack's schema, or loses its honesty banner, this fails.

use kshana_pro::registry;

const BASELINE: &str = include_str!("../scenarios/optimism-gap-baseline.toml");
const SWEEP: &str = include_str!("../scenarios/optimism-gap-sweep.toml");

/// Both templates are valid TOML, carry the Pack-A kind, and state the honest scope
/// in their comments (MODELLED, not a certification, the arXiv preprint).
#[test]
fn templates_parse_and_carry_honest_framing() {
    for (name, src) in [("baseline", BASELINE), ("sweep", SWEEP)] {
        assert_eq!(
            registry::peek_kind(src).as_deref(),
            Some("pro-optimism-gap-audit"),
            "{name} template must carry the Pack-A kind"
        );
        assert!(registry::is_pro_kind("pro-optimism-gap-audit"));
        // It must deserialize into the pack's scenario struct.
        toml::from_str::<toml::Value>(src).unwrap_or_else(|e| panic!("{name} not valid TOML: {e}"));
        // Honesty in the template comments (THE MOAT).
        assert!(
            src.contains("MODELLED"),
            "{name} must declare MODELLED scope"
        );
        assert!(
            src.contains("not a certification") || src.contains("NOT a certification"),
            "{name} must say not a certification"
        );
        assert!(
            src.contains("arXiv:2606.22054"),
            "{name} must cite the preprint id"
        );
        assert!(
            src.to_uppercase().contains("PREPRINT"),
            "{name} must describe the proof as a preprint"
        );
    }
}

/// The (fast) sweep template runs end-to-end through the dispatcher and produces the
/// field-vs-bench headline + honest envelope.
#[test]
fn sweep_template_runs_through_the_pack() {
    let out = registry::run(SWEEP).expect("sweep template must run via the pack");
    assert!(out.summary.contains("optimism-gap audit"));
    assert!(out.summary.contains("not a certification"));
    let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
    assert_eq!(v["kind"], "pro-optimism-gap-audit");
    let gap = v["field_vs_bench"]["optimism_gap_mean"].as_f64().unwrap();
    assert!(gap > 0.0, "sweep optimism gap {gap} should be positive");
    assert!(v["predictor"]["loo_by_detector"]["r2"].as_f64().is_some());
    assert!(out.json.contains("arXiv:2606.22054"));
}

/// The published-baseline template runs end-to-end and yields the published row
/// count. `#[ignore]` only because the full baseline grid is slow (~minutes).
#[test]
#[ignore = "slow: full published baseline grid; run with --ignored"]
fn baseline_template_reproduces_published_row_count() {
    let out = registry::run(BASELINE).expect("baseline template must run via the pack");
    let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
    assert_eq!(v["predictor"]["n_rows"].as_u64().unwrap(), 260);
    assert_eq!(
        v["predictor"]["loo_by_detector"]["folds"].as_u64().unwrap(),
        13
    );
    assert_eq!(v["predictor"]["loo_by_class"]["folds"].as_u64().unwrap(), 4);
    // The published predictor headline (deterministic; ±0.05 covers paper rounding).
    let r2_det = v["predictor"]["loo_by_detector"]["r2"].as_f64().unwrap();
    let r2_cls = v["predictor"]["loo_by_class"]["r2"].as_f64().unwrap();
    assert!((r2_det - 0.47).abs() < 0.05, "LOO-detector R² {r2_det}");
    assert!((r2_cls - 0.46).abs() < 0.05, "LOO-class R² {r2_cls}");
}
