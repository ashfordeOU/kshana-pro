// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Every Pro kind's result is labelled MODELLED.
//!
//! The public Kshana Pro page says every Pro output is labelled MODELLED. The open engine
//! carries that label as a top-level `label` string that starts with `MODELLED`; a Pro
//! result carries it in the same field. This test runs one example scenario of EVERY
//! registered Pro kind through the dispatcher and fails if any result has no such label,
//! or if a new Pro kind is registered without an example here.

use kshana_pro::registry;

/// One small example scenario per Pro kind. Kinds whose every field has a default run
/// from `kind` alone; the rest use the repository's own example scenarios.
fn examples() -> Vec<(&'static str, String)> {
    let bare = |k: &'static str| (k, format!("kind = \"{k}\"\n"));
    // The Arksa-exclusive kinds are not in this crate (docs/PROTECTION.md); their label
    // checks live in the server-side crate with their code.
    vec![
        bare("pro-seam-check"),
        (
            "pro-twin",
            include_str!("../scenarios/twin-csac.toml").to_string(),
        ),
        (
            "pro-uncertainty",
            include_str!("../scenarios/pro-power/uncertainty/uq-clock-holdover-seeds.toml")
                .to_string(),
        ),
        (
            "pro-trade-study",
            include_str!("../scenarios/trade-clock.toml").to_string(),
        ),
        (
            "pro-batch",
            include_str!("../scenarios/batch-clock-mc.toml").to_string(),
        ),
        (
            "pro-quantum-systematics",
            include_str!("../scenarios/quantum-systematics-sr.toml").to_string(),
        ),
        (
            "pro-optimism-gap-audit",
            "kind = \"pro-optimism-gap-audit\"\nn_per_class = 100\nseverities = [0.3, 0.6]\n\
             seeds = [1, 2]\nn_perms = 50\n"
                .to_string(),
        ),
        bare("pro-conflict-resilience-study"),
        (
            "pro-timing-holdover",
            include_str!("../scenarios/timing-holdover-inline.toml").to_string(),
        ),
        (
            "pro-threat-scenario",
            "kind = \"pro-threat-scenario\"\nscenario = \"texbat-ds4-matched-power-position-push\"\n"
                .to_string(),
        ),
        (
            "pro-receiver-scorecard",
            "kind = \"pro-receiver-scorecard\"\ntitle = \"t\"\n[receiver]\nmaker = \"m\"\nmodel = \"x\"\n\
             [campaign]\nid = \"c\"\n[[case]]\nid = \"a\"\nlibrary = \"jammertest-stepped-power-jamming\"\n"
                .to_string(),
        ),
        (
            "pro-timing-scorecard",
            "kind = \"pro-timing-scorecard\"\ntitle = \"t\"\nfleet = \"examples/timing-fleet/fleet.toml\"\n\
             grandmasters = [\"gm-fra-1\"]\nscenarios = [\"baltic-timing-holdover-under-jamming\"]\n"
                .to_string(),
        ),
        (
            "pro-multi-source-trust",
            format!(
                "kind = \"pro-multi-source-trust\"\ninput = \"examples/multi-source/sources.csv\"\n{}",
                include_str!("../examples/multi-source/config.toml")
            ),
        ),
        (
            "pro-area-picture",
            "kind = \"pro-area-picture\"\nconsole = \"examples/area-console/console.toml\"\n".to_string(),
        ),
        (
            "pro-planner-front",
            include_str!("../examples/planner-front/front.toml").to_string(),
        ),
    ]
}

/// The label check the open engine's own tests apply, made strict: the top-level `label`
/// is a string that STARTS with MODELLED, so no Pro result can lead with a higher tier.
fn assert_modelled(kind: &str, json: &str) {
    let v: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{kind}: result is not JSON: {e}"));
    let label = v["label"]
        .as_str()
        .unwrap_or_else(|| panic!("{kind}: result has no top-level `label` string"));
    assert!(
        label.starts_with("MODELLED"),
        "{kind}: label does not start with MODELLED: {label}"
    );
    assert_eq!(v["kind"], kind, "{kind}: result names another kind");
}

#[test]
fn every_registered_pro_kind_has_an_example_here() {
    let have: Vec<&str> = examples().iter().map(|(k, _)| *k).collect();
    for m in registry::pro_kinds() {
        assert!(
            have.contains(&m.name),
            "Pro kind `{}` has no example in tests/modelled_label.rs: add one so its label is checked",
            m.name
        );
    }
    assert_eq!(have.len(), registry::pro_kinds().len(), "stale example");
}

#[test]
fn every_pro_kind_result_is_labelled_modelled() {
    for (kind, src) in examples() {
        let out = registry::run(&src).unwrap_or_else(|e| panic!("{kind}: run failed: {e}"));
        assert_modelled(kind, &out.json);
    }
}

// ------------------------------------------------------------------ command outputs
//
// Every command that writes a result document writes the same top-level `label`. Each
// test below builds that document through the library entry point the command uses.

/// The label check for a command's JSON output (no `kind` field to compare).
fn assert_label(what: &str, json: &str) {
    let v: serde_json::Value =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("{what}: not JSON: {e}"));
    let label = v["label"]
        .as_str()
        .unwrap_or_else(|| panic!("{what}: no top-level `label` string"));
    assert!(
        label.starts_with("MODELLED"),
        "{what}: label does not start with MODELLED: {label}"
    );
}

/// `ci`: the CI report JSON, the history record it appends, the trend JSON over that
/// history and `ci diff` JSON; and an old report without a label still reads back.
#[test]
fn ci_report_history_trend_and_diff_are_labelled_modelled() {
    use kshana_pro::pack_c::{diff, history, parse_pack, run_ci, trend, CiContext, CiReport};
    let dir = std::path::Path::new("tests/fixtures/pack-c");
    let pack = parse_pack(&std::fs::read_to_string(dir.join("pack.toml")).unwrap()).unwrap();
    let report = run_ci(&pack, dir, None, &CiContext::default()).expect("ci runs");
    let json = report.to_json();
    assert_label("ci report", &json);

    let (chained, line) = history::chain(&[], history::record_from_report(&report));
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_label("ci history record", &v["record"].to_string());
    assert_label("ci trend", &trend::render_json(&[chained], "t"));
    assert_label("ci diff", &diff::to_json(&diff::diff(&report, &report)));

    // A report written before the label existed still parses as a baseline.
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old.as_object_mut().unwrap().remove("label");
    let back = CiReport::from_json(&old.to_string()).expect("an unlabelled report reads back");
    assert_eq!(back.label, report.label);
}

/// `report --format json`: the consolidated report's evidence JSON.
#[test]
fn report_json_is_labelled_modelled() {
    let src = "seed = 42\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 7200.0\n\
        [gnss]\nwindows = [ { t0 = 0.0, t1 = 600.0, state = \"nominal\" }, \
        { t0 = 600.0, t1 = 7200.0, state = \"denied\" } ]\n\
        [clock_quantum]\nid = \"optical-sr-lattice\"\nprovenance = \"demo\"\ny0 = 5.0e-17\nq_wf = 1.0e-30\nq_rw = 0.0\n\
        [clock_classical]\nid = \"csac-sa45s\"\nprovenance = \"demo\"\ny0 = 5.0e-10\nq_wf = 9.0e-20\nq_rw = 0.0\n";
    let r = kshana_pro::report::build_report("t", src, "quantum").expect("report builds");
    assert_label("report json", &r.to_evidence_json());
}

/// `study --dossier`: the study dossier JSON.
#[test]
fn study_dossier_json_is_labelled_modelled() {
    let src = include_str!("../scenarios/dossier-holdover-csac.toml");
    let d = kshana_pro::study_dossier::build(src, &Default::default()).expect("dossier builds");
    assert_label("study dossier", &d.to_json());
}
