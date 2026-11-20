// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! LEO-PNT evaluation (research 2026-11-20 gaps E21: B2101, B2107, B2108) on SYNTHETIC inputs:
//! multi-simulator scorecard under a registered procedure; SISRE weights checked against the
//! published GPS values and SISRE against hand cases; DCB jumps; dependency ratings; claim
//! templates carry no value and an uncited value is refused.

use kshana_pro::leo_eval::{claims, od, sims};

const PROC: &str = r#"format = "kshana-pro-leo-sim-procedure/1"
id = "leo-synth"
title = "SYNTHETIC LEO-PNT procedure"

[[step]]
id = "open-sky"
what = "steady state, open sky"
metric = "h_rms_m"
bar = 5.0
bar_source = "SYNTHETIC bar"

[[step]]
id = "cold"
what = "cold start"
metric = "ttff_s"
"#;

#[test]
fn scorecard_keeps_order_and_marks_the_simulator_that_differs() {
    let h = sims::procedure_hash(PROC);
    let mut runs = String::from("simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix\n");
    for (sim, scale, ttff) in [
        ("SimC", 1.0, 30),
        ("SimA", 1.1, 32),
        ("SimB", 0.9, 31),
        ("SimD", 6.0, 29),
    ] {
        for k in 0..100 {
            runs.push_str(&format!(
                "{sim},open-sky,{k},{},0,0,0,1\n",
                scale * if k % 2 == 0 { 3.0 } else { -3.0 }
            ));
            runs.push_str(&format!("{sim},cold,{k},1,1,1,0,{}\n", (k >= ttff) as u8));
        }
    }
    let s = sims::score(PROC, &h, &runs).unwrap();
    assert_eq!(
        s.simulators,
        vec!["SimC", "SimA", "SimB", "SimD"],
        "input order, not ranked"
    );
    let sky = &s.steps[0];
    let d: Vec<&str> = sky
        .cells
        .iter()
        .filter(|c| c.differs_from_others)
        .map(|c| c.simulator.as_str())
        .collect();
    assert_eq!(d, vec!["SimD"]);
    assert_eq!(sky.cells[0].meets_bar, Some(true));
    assert_eq!(sky.cells[3].meets_bar, Some(false));
    assert_eq!(s.steps[1].cells[0].value, Some(30.0));
    assert!(
        sims::score(&PROC.replace("5.0", "50.0"), &h, &runs).is_err(),
        "changed after registration"
    );
}

#[test]
fn sisre_weights_and_values() {
    // Published GPS orbit-only weights: w_R about 0.98, w_A,C about 1/7 (w² about 1/49).
    let g = od::weights(20_200.0, 0.0).unwrap();
    assert!((g.e_uz2.sqrt() - 0.98).abs() < 0.01, "{g:?}");
    assert!((g.e_transverse - 1.0 / 49.0).abs() < 0.003, "{g:?}");
    let l = od::weights(1_000.0, 5.0).unwrap();
    assert!(l.e_uz < g.e_uz && l.e_transverse > g.e_transverse, "{l:?}");
    let mut csv = String::from("t,sat,dR_m,dA_m,dC_m,dclk_m,sigma_m\n");
    for k in 0..20 {
        csv.push_str(&format!("{k},CLK,0,0,0,0.5,1\n{k},RAD,1,0,0,0,1\n"));
    }
    let s = od::sisre(&csv, &l).unwrap();
    let clk = s.iter().find(|x| x.sat == "CLK").unwrap();
    assert!((clk.rms_m - 0.5).abs() < 1e-12 && clk.orbit_only_rms_m == 0.0);
    let rad = s.iter().find(|x| x.sat == "RAD").unwrap();
    assert!((rad.rms_m - l.e_uz2.sqrt()).abs() < 1e-12);
    assert!(clk.overbound.as_ref().unwrap().bounded);
    assert!(od::weights(-1.0, 0.0).is_err());
}

#[test]
fn dcb_jumps_and_dependency_ratings() {
    let csv = "date,sat,dcb_ns,sigma_ns\n2026-01-01,L1,2.0,0.1\n2026-01-02,L1,2.05,0.1\n2026-01-03,L1,3.0,0.1\n2026-01-04,L1,3.02,0.1\n";
    let d = od::dcb(csv, 3.0).unwrap();
    assert_eq!(d[0].jumps.len(), 1);
    assert_eq!(d[0].jumps[0].0, "2026-01-03");
    let deps: Vec<od::Dependency> = toml::from_str::<std::collections::BTreeMap<String, Vec<od::Dependency>>>(
        r#"dependency = [
  { name = "corrections", kind = "correction-service", single_source = true, outage_impact = "convergence-lost" },
  { name = "orbits", kind = "orbit-product", single_source = true, fallback = "broadcast", outage_impact = "degraded" },
  { name = "frame", kind = "reference-frame", single_source = true, outage_impact = "none" },
]"#,
    )
    .unwrap()
    .remove("dependency")
    .unwrap();
    let r = od::deps(&deps).unwrap();
    assert_eq!(
        r.iter().map(|x| x.rating).collect::<Vec<_>>(),
        vec!["critical", "moderate", "low"]
    );
}

#[test]
fn claim_templates_cite_nothing_and_uncited_values_are_refused() {
    let t = claims::templates();
    assert_eq!(
        t.iter()
            .map(|d| d.constellation.as_str())
            .collect::<Vec<_>>(),
        vec!["Celeste", "CentiSpace", "Geespace"]
    );
    for d in &t {
        assert_eq!(claims::check(d).unwrap(), d.claims.len());
        assert!(d
            .claims
            .iter()
            .all(|c| c.value.is_none() && c.source_url.is_none()));
    }
    let mut d = t[0].clone();
    d.claims[0].value = Some(1.0);
    assert!(claims::check(&d).is_err(), "a value without a source");
    d.claims[0].source_url = Some("https://example.org/SYNTHETIC".into());
    d.claims[0].source_date = Some("2026-01-01".into());
    d.claims[0].quote = Some("SYNTHETIC quote".into());
    assert_eq!(claims::check(&d).unwrap(), d.claims.len() - 1);
    let c = claims::compare(&d, &[("horizontal-accuracy".into(), 1.4)]).unwrap();
    assert_eq!(c.len(), 1);
    assert!(!c[0].reproduced);
}
