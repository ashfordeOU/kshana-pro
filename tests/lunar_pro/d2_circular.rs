// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D2: "Circular T for the Moon" on synthetic comparisons with known truth (part of the
// tests/lunar_pro.rs test binary).

use kshana_pro::lunar_pro::circular::*;
use kshana_pro::lunar_pro::time::{LunarOption, Model, RateSource, W0Candidate};

fn cfg(option: LunarOption) -> Config {
    Config {
        option,
        issue: 7,
        year: 2026,
        month: 11,
        window_days: 2.5,
    }
}

fn model() -> Model {
    Model::new(RateSource::Lte440, W0Candidate::conventional(2.821e6))
}

#[test]
fn d2_dates_are_the_circular_t_mjds_of_the_month() {
    let d = circular_dates(2026, 11).unwrap();
    // November 2026: MJD 61345 = 2026-11-01.
    assert_eq!(d, vec![61349, 61354, 61359, 61364, 61369, 61374]);
    assert!(circular_dates(2026, 13).is_err());
}

#[test]
fn d2_recovers_the_known_truth_within_its_statistical_uncertainty() {
    let m = model();
    let plan = SyntheticPlan::example();
    let (cmp, earth, mjd0) = synthetic(&m, LunarOption::Tl, &plan).unwrap();
    let c = compute(&m, &cfg(LunarOption::Tl), &cmp, &earth).unwrap();
    assert_eq!(c.rows.len(), 2);
    for (row, clk) in c.rows.iter().zip(&plan.clocks) {
        assert_eq!(row.lunar_clock, clk.code);
        for v in &row.values {
            let dev = v.dev_ns.unwrap();
            let ua = v.u_a_ns.unwrap();
            let truth = clk.truth(v.mjd as f64, mjd0);
            assert!(
                (dev - truth).abs() <= 4.0 * ua,
                "{} {}: {dev} vs {truth} (uA {ua})",
                row.lunar_clock,
                v.mjd
            );
            // uA is about σ/√n for a centred line fit.
            assert!(ua > 0.05 && ua < 0.3, "{ua}");
            // uB carries the link calibration and the Earth laboratory's Circular T u.
            assert!(v.u_b_ns.unwrap() >= (1.0f64 + 4.0).sqrt() - 1e-9);
            assert!((v.u_ns.unwrap().powi(2) - ua * ua - v.u_b_ns.unwrap().powi(2)).abs() < 1e-9);
            // TL(k) − UTC = model − dev.
            let model_s = model_offset(&m, LunarOption::Tl, v.mjd as f64).value();
            assert!((v.realisation_minus_utc_s.unwrap() - (model_s - dev * 1e-9)).abs() < 1e-15);
        }
    }
}

#[test]
fn d2_statistical_uncertainty_has_the_stated_coverage() {
    // Monte Carlo over seeds: |dev − truth| ≤ 1.96 uA in about 95 % of cases.
    let m = model();
    let mut inside = 0usize;
    let mut total = 0usize;
    for seed in 0..60 {
        let plan = SyntheticPlan {
            seed,
            ..SyntheticPlan::example()
        };
        let (cmp, earth, mjd0) = synthetic(&m, LunarOption::Tl, &plan).unwrap();
        let c = compute(&m, &cfg(LunarOption::Tl), &cmp, &earth).unwrap();
        for (row, clk) in c.rows.iter().zip(&plan.clocks) {
            for v in &row.values {
                total += 1;
                if (v.dev_ns.unwrap() - clk.truth(v.mjd as f64, mjd0)).abs()
                    <= 1.96 * v.u_a_ns.unwrap()
                {
                    inside += 1;
                }
            }
        }
    }
    let frac = inside as f64 / total as f64;
    assert!(
        (0.91..=0.99).contains(&frac),
        "coverage {frac} over {total}"
    );
}

#[test]
fn d2_the_open_engine_circular_t_parser_reads_the_table_back() {
    let m = model();
    let mut plan = SyntheticPlan::example();
    // A clock with no comparisons for the second half of the month prints "-".
    plan.clocks.push(SyntheticClock {
        code: "LTL3".into(),
        earth_lab: "PTB".into(),
        a_ns: 1.0,
        b_ns_per_day: 0.0,
    });
    let (mut cmp, earth, mjd0) = synthetic(&m, LunarOption::Tl, &plan).unwrap();
    cmp.retain(|c| !(c.lunar_clock == "LTL3" && c.mjd_utc > mjd0 + 12.0));
    let c = compute(&m, &cfg(LunarOption::Tl), &cmp, &earth).unwrap();
    let text = c.to_text();
    assert!(text.contains(DISCLAIMER));
    let sec = kshana::realdata::clk::parse_circular_t_section1(&text).unwrap();
    assert_eq!(sec.issue, Some(7));
    assert_eq!(sec.mjds, c.mjds);
    assert_eq!(sec.rows.len(), 3);
    for (pr, row) in sec.rows.iter().zip(&c.rows) {
        assert_eq!(pr.code, row.lunar_clock);
        for (pv, v) in pr.values.iter().zip(&row.values) {
            match (pv, v.dev_ns) {
                (Some(a), Some(b)) => assert!((a - b).abs() <= 0.005 + 1e-9),
                (None, None) => {}
                other => panic!("mismatch {other:?}"),
            }
        }
        assert!((pr.u.unwrap() - row.u_ns.unwrap()).abs() <= 0.005 + 1e-9);
    }
    let ltl3 = &c.rows[2];
    assert!(ltl3.values.last().unwrap().dev_ns.is_none());
    assert!(ltl3.values[0].dev_ns.is_some());
}

#[test]
fn d2_csv_and_circular_t_inputs_give_the_same_result() {
    let m = model();
    let plan = SyntheticPlan::example();
    let (cmp, earth, _) = synthetic(&m, LunarOption::Tl, &plan).unwrap();
    let a = compute(&m, &cfg(LunarOption::Tl), &cmp, &earth).unwrap();
    // Through the CSV writers and readers.
    let cmp2 = parse_comparisons(&comparisons_csv(&cmp)).unwrap();
    let earth2 = EarthSeries::from_csv(&earth_csv(&earth)).unwrap();
    let b = compute(&m, &cfg(LunarOption::Tl), &cmp2, &earth2).unwrap();
    // Through a BIPM-layout Circular T Section 1 for the Earth laboratories.
    let mut ct = String::from(
        "CIRCULAR T 999\n\n1 - Difference between UTC and its local realizations UTC(k)\n\n",
    );
    let mjds: Vec<f64> = earth.labs["PTB"].iter().map(|p| p.0).collect();
    ct.push_str("MJD");
    for m in &mjds {
        ct.push_str(&format!(" {}", *m as i64));
    }
    ct.push_str("\nLaboratory k   [UTC-UTC(k)]/ns   uA uB u\n");
    for (lab, v) in &earth.labs {
        ct.push_str(&format!("{lab} (Somewhere)"));
        for p in v {
            ct.push_str(&format!(" {:.4}", p.1));
        }
        ct.push_str(" 0.3 1.9 2.0\n");
    }
    ct.push_str("\n2 - next section\n");
    let earth3 = EarthSeries::from_circular_t(&[ct]).unwrap();
    let c3 = compute(&m, &cfg(LunarOption::Tl), &cmp, &earth3).unwrap();
    for ((ra, rb), rc) in a.rows.iter().zip(&b.rows).zip(&c3.rows) {
        for ((va, vb), vc) in ra.values.iter().zip(&rb.values).zip(&rc.values) {
            assert!((va.dev_ns.unwrap() - vb.dev_ns.unwrap()).abs() < 1e-3);
            assert!((va.dev_ns.unwrap() - vc.dev_ns.unwrap()).abs() < 1e-3);
        }
    }
}

#[test]
fn d2_tcl_option_is_selectable_and_restores_its_own_offset() {
    let m = model();
    let plan = SyntheticPlan::example();
    let (cmp, earth, mjd0) = synthetic(&m, LunarOption::Tcl, &plan).unwrap();
    let c = compute(&m, &cfg(LunarOption::Tcl), &cmp, &earth).unwrap();
    assert_eq!(c.scale, "TCL");
    let v = &c.rows[0].values[0];
    assert!((v.dev_ns.unwrap() - plan.clocks[0].truth(v.mjd as f64, mjd0)).abs() < 1.0);
    // TCL(k) − UTC exceeds TL(k) − UTC by L_L × elapsed since T0 (≈ 49 ms in 2026).
    let tl = model_offset(&m, LunarOption::Tl, v.mjd as f64).value();
    let d = v.realisation_minus_utc_s.unwrap() - tl;
    assert!((d - 0.0494).abs() < 1e-3, "{d}");
    assert!(c.to_text().contains("[TCL-TCL(k)]"));
}

#[test]
fn d2_refuses_to_extrapolate_the_earth_series_and_bad_input() {
    let e =
        EarthSeries::from_csv("mjd,lab,utc_minus_utck_ns,u_ns\n61344,PTB,1.0,2\n61349,PTB,2.0,2\n")
            .unwrap();
    assert!((e.at("PTB", 61346.5).unwrap().0 - 1.5).abs() < 1e-12);
    assert!(e.at("PTB", 61350.0).is_err());
    assert!(e.at("NPL", 61346.0).is_err());
    assert!(parse_comparisons("61349,LTL1,PTB,70.2,0.5\n").is_err());
    assert!(parse_comparisons("61349,LTL1,PTB,70.2,0,1\n").is_err());
}
