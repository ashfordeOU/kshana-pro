// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! LEO-PNT evaluation (research 2026-11-20 gaps E21: B2101, B2107, B2108) on SYNTHETIC inputs:
//! published procedures parse and hash stably; the scorecard keeps input order and states
//! whether the method hash was registered; one receiver on two simulators agrees within the
//! stated tolerance, and the self-test vocabulary attributes deviations; Doppler bounds; SISRE
//! weights against the published GPS values; DCB, dependencies; the T-RAIM study with injected
//! single and dual faults; the due-diligence template cites every verdict.

use kshana_pro::leo_eval::{claims, od, sims, traim};

fn proc_with_tol() -> String {
    let mut p = sims::PUBLISHED
        .iter()
        .find(|x| x.0 == "leo-acquisition")
        .unwrap()
        .1
        .to_string();
    p = p.replace(
        "metric = \"h_rms_m\"",
        "metric = \"h_rms_m\"\nbar = 5.0\nbar_source = \"SYNTHETIC bar\"\nequivalence_tolerance = 0.5",
    );
    p.replace(
        "metric = \"ttff_s\"\n\n[[step]]\nid = \"warm-start\"",
        "metric = \"ttff_s\"\nequivalence_tolerance = 3.0\n\n[[step]]\nid = \"warm-start\"",
    )
}

fn runs(sims_: &[(&str, &str, f64, usize)]) -> String {
    let mut r =
        String::from("simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix,receiver\n");
    for (sim, rx, scale, ttff) in sims_ {
        for k in 0..100 {
            r.push_str(&format!(
                "{sim},steady-state,{k},{},0,0,0,1,{rx}\n",
                scale * if k % 2 == 0 { 3.0 } else { -3.0 }
            ));
            r.push_str(&format!(
                "{sim},cold-start,{k},1,1,1,0,{},{rx}\n",
                (k >= *ttff) as u8
            ));
        }
    }
    r
}

#[test]
fn published_procedures_are_frozen_and_hash_stably() {
    assert_eq!(sims::PUBLISHED.len(), 3);
    for (id, t) in sims::PUBLISHED {
        let h1 = sims::procedure_hash(t).unwrap();
        let h2 =
            sims::procedure_hash(&t.replace("title = ", "title = \"x\"\n#")).unwrap_or_default();
        assert_eq!(h1.len(), 64, "{id}");
        let _ = h2;
        let p: sims::Procedure = toml::from_str(t).unwrap();
        assert_eq!(p.version, "1.0");
    }
    // Wording outside the method part does not change the hash; a step does.
    let t = sims::PUBLISHED[0].1;
    let reworded = t.replacen(
        "title = \"LEO-PNT receiver",
        "title = \"LEO-PNT receiver (reworded)",
        1,
    );
    assert_eq!(
        sims::procedure_hash(t).unwrap(),
        sims::procedure_hash(&reworded).unwrap()
    );
    assert_ne!(
        sims::procedure_hash(t).unwrap(),
        sims::procedure_hash(&t.replace("h_p95_m", "h_rms_m")).unwrap()
    );
}

#[test]
fn scorecard_keeps_order_and_states_registration() {
    let p = proc_with_tol();
    let h = sims::procedure_hash(&p).unwrap();
    let r = runs(&[
        ("SimC", "rx1", 1.0, 30),
        ("SimA", "rx1", 1.1, 32),
        ("SimB", "rx1", 0.9, 31),
        ("SimD", "rx1", 6.0, 29),
    ]);
    let s = sims::score(&p, Some(&h), &r).unwrap();
    assert!(s.prereg_matches);
    assert_eq!(
        s.simulators,
        vec!["SimC", "SimA", "SimB", "SimD"],
        "input order, never ranked"
    );
    let st = s.steps.iter().find(|x| x.step == "steady-state").unwrap();
    let d: Vec<&str> = st
        .cells
        .iter()
        .filter(|c| c.differs_from_others)
        .map(|c| c.simulator.as_str())
        .collect();
    assert_eq!(d, vec!["SimD"]);
    assert_eq!(st.cells[3].meets_bar, Some(false));
    let s2 = sims::score(&p, Some("00"), &r).unwrap();
    assert!(!s2.prereg_matches && s2.label.contains("not registered"));
}

#[test]
fn one_receiver_on_two_simulators_agrees_within_tolerance_and_deviations_are_attributed() {
    let p = proc_with_tol();
    let h = sims::procedure_hash(&p).unwrap();
    // One receiver, two simulators within the tolerances: consistent on both stated steps.
    let e = sims::equivalence(
        &p,
        Some(&h),
        &runs(&[("SimX", "rx1", 1.0, 30), ("SimY", "rx1", 1.1, 31)]),
    )
    .unwrap();
    assert_eq!(e.steps.len(), 2);
    assert!(
        e.steps.iter().all(|s| s.verdict == "consistent"),
        "{:?}",
        e.steps
    );
    // Two receivers, SimY 1 m worse for both: the simulator deviates.
    let e = sims::equivalence(
        &p,
        Some(&h),
        &runs(&[
            ("SimX", "rx1", 1.0, 30),
            ("SimX", "rx2", 1.0, 30),
            ("SimY", "rx1", 1.4, 30),
            ("SimY", "rx2", 1.4, 30),
        ]),
    )
    .unwrap();
    assert_eq!(
        e.steps
            .iter()
            .find(|s| s.step == "steady-state")
            .unwrap()
            .verdict,
        "simulator-deviation"
    );
    // Only one receiver differs: the receiver deviates.
    let e = sims::equivalence(
        &p,
        Some(&h),
        &runs(&[
            ("SimX", "rx1", 1.0, 30),
            ("SimX", "rx2", 1.0, 30),
            ("SimY", "rx1", 1.0, 30),
            ("SimY", "rx2", 1.4, 30),
        ]),
    )
    .unwrap();
    assert_eq!(
        e.steps
            .iter()
            .find(|s| s.step == "steady-state")
            .unwrap()
            .verdict,
        "receiver-deviation"
    );
    // One receiver beyond the tolerance: unattributed.
    let e = sims::equivalence(
        &p,
        None,
        &runs(&[("SimX", "rx1", 1.0, 30), ("SimY", "rx1", 1.4, 30)]),
    )
    .unwrap();
    assert_eq!(
        e.steps
            .iter()
            .find(|s| s.step == "steady-state")
            .unwrap()
            .verdict,
        "unattributed"
    );
    assert!(!e.prereg_matches);
}

#[test]
fn doppler_bounds_follow_the_closed_form() {
    let (d, r) = sims::doppler_bounds(550.0, 1.5e9).unwrap();
    let v = (3.986_004_418e14_f64 / 6_921_000.0).sqrt();
    assert!((d - 1.5e9 * v * 6_371_000.0 / 6_921_000.0 / 299_792_458.0).abs() < 1e-6);
    assert!((r - 1.5e9 * v * v / 550_000.0 / 299_792_458.0).abs() < 1e-6);
    assert!(d > 30_000.0 && d < 40_000.0, "{d}");
    assert!(sims::doppler_bounds(0.0, 1.0).is_err());
}

#[test]
fn sisre_dcb_and_dependencies() {
    let g = od::weights(20_200.0, 0.0).unwrap();
    assert!((g.e_uz2.sqrt() - 0.98).abs() < 0.01, "{g:?}");
    assert!((g.e_transverse - 1.0 / 49.0).abs() < 0.003, "{g:?}");
    let l = od::weights(1_000.0, 5.0).unwrap();
    let mut csv = String::from("t,sat,dR_m,dA_m,dC_m,dclk_m,sigma_m\n");
    for k in 0..20 {
        csv.push_str(&format!("{k},CLK,0,0,0,0.5,1\n{k},RAD,1,0,0,0,1\n"));
    }
    let s = od::sisre(&csv, &l).unwrap();
    assert!((s.iter().find(|x| x.sat == "CLK").unwrap().rms_m - 0.5).abs() < 1e-12);
    assert!((s.iter().find(|x| x.sat == "RAD").unwrap().rms_m - l.e_uz2.sqrt()).abs() < 1e-12);
    let d = od::dcb("date,sat,dcb_ns,sigma_ns\n2026-01-01,L1,2.0,0.1\n2026-01-02,L1,2.05,0.1\n2026-01-03,L1,3.0,0.1\n", 3.0).unwrap();
    assert_eq!(d[0].jumps.len(), 1);
    let deps = vec![
        od::Dependency {
            name: "c".into(),
            kind: "correction-service".into(),
            single_source: true,
            fallback: String::new(),
            outage_impact: "convergence-lost".into(),
            evidence: String::new(),
        },
        od::Dependency {
            name: "f".into(),
            kind: "reference-frame".into(),
            single_source: true,
            fallback: String::new(),
            outage_impact: "none".into(),
            evidence: String::new(),
        },
    ];
    assert_eq!(
        od::deps(&deps)
            .unwrap()
            .iter()
            .map(|x| x.rating)
            .collect::<Vec<_>>(),
        vec!["critical", "low"]
    );
}

#[test]
fn traim_study_detects_single_faults_and_shows_the_dual_fault_limit() {
    let q: traim::Study = toml::from_str(
        r#"altitude_km = 550
in_view = [6, 10]
sigma_m = 1.0
p_fa = 1e-3
p_md = 1e-3
epochs = 400
seed = 7
[[fault]]
id = "single"
sats = [2]
bias_m = 40.0
start = 100
end = 150
[[fault]]
id = "dual"
sats = [1, 3]
bias_m = 40.0
start = 250
end = 300
"#,
    )
    .unwrap();
    let r = traim::run(&q).unwrap();
    for row in &r.rows {
        assert!(row.false_alarm_rate < 0.02, "{row:?}");
        assert!(row.faults[0].detection_rate > 0.8, "{row:?}");
        assert_eq!(row.faults[0].recovery_epochs, Some(0));
        // A single-satellite exclusion cannot remove a dual fault.
        assert_eq!(row.faults[1].correct_exclusion_rate, 0.0);
    }
    // More redundancy: the single fault is detected and excluded more often.
    let (six, ten) = (&r.rows[0], &r.rows[1]);
    assert!(ten.faults[0].detection_rate >= six.faults[0].detection_rate);
    assert!(
        ten.faults[0].correct_exclusion_rate > six.faults[0].correct_exclusion_rate,
        "{six:?} {ten:?}"
    );
    assert!(ten.faults[0].correct_exclusion_rate > 0.8, "{ten:?}");
    let mut bad = q;
    bad.in_view = vec![4];
    assert!(traim::run(&bad).is_err());
}

#[test]
fn dd_template_cites_every_verdict_and_refuses_uncited_values() {
    let mut d: claims::Dataset = serde_json::from_value(serde_json::json!({
        "format": claims::FORMAT,
        "constellation": "SYNTHETIC-LEO",
        "operator": "SYNTHETIC operator",
        "claims": [
            {"id": "h-acc", "quantity": "horizontal accuracy", "unit": "m", "direction": "max", "value": 1.0,
             "source_url": "https://example.org/SYNTHETIC", "source_date": "2026-01-01", "quote": "SYNTHETIC quote"},
            {"id": "ttff", "quantity": "time to first fix", "unit": "s", "direction": "max", "value": null}
        ]
    }))
    .unwrap();
    let md = claims::dd_report(&d, &[("h-acc".into(), 1.4)], "SYNTHETIC scorecard").unwrap();
    assert!(
        md.contains("NOT REPRODUCED")
            && md.contains("https://example.org/SYNTHETIC")
            && md.contains("MODELLED")
    );
    assert!(md.contains("1 claim(s) of the dataset have no cited value"));
    d.claims[0].quote = None;
    assert!(claims::dd_report(&d, &[], "x").is_err());
}
