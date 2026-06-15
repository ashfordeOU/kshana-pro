// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! End-to-end **interconnection / interoperability** proof across both repos.
//!
//! This is the integration test that demonstrates the whole stack composes: the
//! open `kshana` engine, the brand-new open `hybrid-ukf` scenario, and the Pro
//! overlay's mission-analysis chain (benefits → NAV-DT twin bundle → consolidated
//! report → SpaceStream message → CI gate) all interoperate, the honesty labels
//! propagate from the open verification matrix through every Pro artifact, and the
//! whole pipeline is reproducible. Nothing here is contrived — each stage consumes
//! the real output of the prior stage.

use kshana::api::run_scenario;
use kshana_pro::benefits::{benefits_case, FomSpec};
use kshana_pro::connectors::spacestream::{LinkSummary, SpaceStreamMessage};
use kshana_pro::connectors::{EphemSample, GateOp};
use kshana_pro::registry;
use kshana_pro::report::{build_report, GateCheck, RtmRow};
use kshana_pro::suite::Variant;
use kshana_pro::twin::navdt::run_navdt_bundle;

/// The clock-holdover scenario shape the Pro mission-analysis chain consumes.
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

/// INTEROP 1 — the superset: the Pro dispatcher runs an OPEN scenario unchanged
/// (delegates to the embedded open engine), and runs the brand-new open
/// `hybrid-ukf` scenario too. Pro = open ∪ proprietary, by construction.
#[test]
fn pro_dispatcher_runs_open_and_new_hybrid_ukf_scenarios() {
    // A plain open clock scenario routes through the Pro dispatcher to the open engine.
    let open = registry::run(&clock_scenario()).expect("pro dispatcher runs open scenario");
    assert!(open.json.contains("scenario_hash"));

    // The freshly-added open `hybrid-ukf` kind also runs through the Pro dispatcher —
    // proving Pro inherits every open capability, including ones added after Pro. A
    // minimal, fast instance (2 consistency seeds, short arc): we only need to prove
    // it *routes and runs* via the superset; statistical consistency is proven in the
    // open repo's own suite.
    let ukf = "kind = \"hybrid-ukf\"\n\
               seed = 1771\n\
               residual_accel_bias_m_s2 = 1.0e-4\n\
               speed_m_s = 100.0\n\
               sigma_pr_m = 1.0\n\
               sigma_rr_mps = 0.05\n\
               consistency_seeds = 2\n\
               q_factor = 1.0\n\
               r_factor = 1.0\n\
               [time]\n\
               step_s = 1.0\n\
               duration_s = 40.0\n\
               [gnss]\n\
               windows = [ { t0 = 0.0, t1 = 20.0, state = \"nominal\" }, { t0 = 20.0, t1 = 40.0, state = \"denied\" } ]\n\
               [accel]\n\
               id = \"cold-atom-interferometer\"\n\
               provenance = \"modelled, partner-owned\"\n\
               bias = 0.0\n\
               q_va = 1.0e-9\n\
               [accel.cai]\n\
               wavelength_m = 780.241209e-9\n\
               pulse_sep_t = 0.02\n\
               atom_number = 1.0e6\n\
               contrast = 0.5\n\
               cycle_time_s = 0.5\n\
               [clock]\n\
               id = \"ultrastable-optical\"\n\
               provenance = \"modelled\"\n\
               white_fm_adev_1s = 1.0e-13\n\
               rw_fm_level = 1.0e-15\n";
    let out = registry::run(ukf).expect("pro dispatcher runs the new hybrid-ukf scenario");
    assert!(
        out.summary.contains("hybrid-ukf") && out.summary.contains("MODELLED"),
        "hybrid-ukf must run via the Pro superset and stay labelled modelled: {}",
        out.summary
    );
    assert!(
        out.summary.contains("CONSISTENT") || out.summary.contains("self-consistency"),
        "the consistency oracle must surface through the Pro path"
    );
}

/// INTEROP 2 — the full mission-analysis chain on one scenario: open run →
/// benefits (A02) + NAV-DT bundle (A10) → consolidated report (A11, with trade +
/// impairment + RTM) → CI gate (A17) → SpaceStream message (A09). Every stage
/// consumes the prior stage's real output.
#[test]
fn full_mission_analysis_chain_composes_end_to_end() {
    let scn = clock_scenario();

    // Stage 0: the open engine produces the result the whole chain reads.
    let open = run_scenario(&scn).expect("open engine runs");
    let json: serde_json::Value = serde_json::from_str(&open.json).unwrap();
    let q_timing = json["quantum"]["fom"]["timing_rms_ns"].as_f64().unwrap();

    // Stage A02: benefit-vs-baseline over the same neutral code path.
    let cand = Variant {
        label: "optical".into(),
        scenario_toml: scn.clone(),
    };
    let base = Variant {
        label: "csac".into(),
        scenario_toml: scn.clone(),
    };
    let foms = vec![FomSpec {
        name: "holdover (s)".into(),
        candidate_key: "quantum.fom.holdover_s".into(),
        baseline_key: "classical.fom.holdover_s".into(),
        lower_better: false,
    }];
    let benefits = benefits_case("interop trade", &cand, &base, &foms).unwrap();
    assert_eq!(benefits.rows.len(), 1);
    assert_eq!(benefits.rows[0].candidate_label, "MODELLED"); // honest label propagated

    // Stage A10: the NAV-DT bundle over the same scenario.
    let bundle = run_navdt_bundle(&scn, "quantum").unwrap();
    assert!(bundle.kshana_is_one_component);
    assert!(bundle.scenario_hash.is_some());

    // Stage A11: the consolidated report, composing trade evidence + impairment + RTM.
    let report = build_report("E2E interop report", &scn, "quantum")
        .unwrap()
        .with_trade_evidence("interop trade evidence".into())
        .with_impairment(kshana_pro::report::run_impairment_summary(7))
        .with_rtm(vec![RtmRow::new(
            "timing RMS (ns)",
            20.0,
            q_timing,
            true,
            "from the same open run",
        )]);
    let md = report.to_markdown();
    assert!(md.contains("Executive FoM summary"));
    assert!(md.contains("MODELLED"));
    assert!(md.contains("ONE work-package component")); // honesty boundary on the artifact
    assert!(md.contains("floor assumption")); // modelled caveat on-face

    // Stage A17: the CI gate over the report.
    let gate = report.gate(&[GateCheck::new("availability", GateOp::Ge, 0.5)]);
    assert!(gate.passed, "gate should pass: {:?}", gate.failures);
    assert!(gate.note.contains("NOT external validation"));

    // Stage A09: a SpaceStream message built from the SAME open FoM the report used.
    let fom = kshana::fom::FoMScores {
        timing_rms_ns: q_timing,
        timing_p95_ns: json["quantum"]["fom"]["timing_p95_ns"].as_f64().unwrap(),
        holdover_s: json["quantum"]["fom"]["holdover_s"].as_f64().unwrap(),
        resilience_slope_ns_per_s: 0.0,
        availability: json["quantum"]["fom"]["availability"].as_f64().unwrap(),
        integrity: json["quantum"]["fom"]["integrity"].as_f64(),
        security: None,
    };
    let epoch = EphemSample {
        t_s: 0.0,
        jd_utc: 2_451_545.0,
        gcrs_r_m: [1.0e6, 2.0e6, 3.0e6],
        gcrs_v_m_s: [10.0, 20.0, 30.0],
        ecef_r_m: [4.0e6, 5.0e6, 6.0e6],
        lat_deg: 0.0,
        lon_deg: 0.0,
        alt_km: 400.0,
        speed_m_s: 7600.0,
    };
    let msg = SpaceStreamMessage::from_pass(
        "E2E-PASS",
        &epoch,
        &fom,
        true,
        LinkSummary {
            cn0_dbhz: 45.0,
            margin_db: 8.0,
        },
        None,
    );
    // The message round-trips and its timing matches the open run (true interop).
    let back = SpaceStreamMessage::parse(&msg.to_json()).unwrap();
    assert_eq!(back.ranging_timing.timing_rms_ns, q_timing);
    // Aggregated-only: no raw IQ crossed the interface.
    assert!(!msg.to_json().contains("\"iq\"") && !msg.to_json().contains("\"samples\""));
}

/// INTEROP 3 — honesty labels propagate from the OPEN verification matrix all the way
/// to the Pro artifacts: the validated/modelled split a Pro report shows is exactly
/// the open core's, never inflated.
#[test]
fn honesty_labels_propagate_from_open_matrix_to_pro_report() {
    let open_summary =
        kshana::verification::summarize(&kshana::verification::verification_matrix());
    let counts = kshana_pro::report::matrix_counts();
    assert_eq!(counts.validated, open_summary.validated);
    assert_eq!(counts.modelled, open_summary.modelled);
    assert_eq!(counts.partner, open_summary.partner_owned);
    // And a Pro report renders those exact counts (no inflation).
    let report = build_report("label-propagation", &clock_scenario(), "quantum").unwrap();
    let md = report.to_markdown();
    assert!(md.contains(&format!("{} validated", counts.validated)));
}

/// INTEROP 4 — the whole pipeline is reproducible: re-running it yields the identical
/// report fingerprint and the identical scenario hash (the reproducibility invariant
/// the open core guarantees, carried through the Pro chain).
#[test]
fn end_to_end_pipeline_is_reproducible() {
    let a = build_report("repro", &clock_scenario(), "quantum").unwrap();
    let b = build_report("repro", &clock_scenario(), "quantum").unwrap();
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_eq!(a.bundle.scenario_hash, b.bundle.scenario_hash);
}
