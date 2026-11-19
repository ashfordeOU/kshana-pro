// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// Integration tests of the Kshana Pro lunar layer (src/lunar_pro, W1119 D1–D5).

use kshana::timescales::TwoPartJd;
use kshana_pro::lunar_pro::time::*;
use kshana_pro::lunar_pro::Budget;

const US_DAY: f64 = 86_400e6;
const NS_DAY: f64 = 86_400e9;

fn epochs() -> Vec<TwoPartJd> {
    [
        "1985-03-01T06:00:00",
        "2000-01-01T12:00:00",
        "2026-11-19T00:00:00",
        "2031-07-15T18:30:00",
        "2049-12-31T23:00:00",
    ]
    .iter()
    .map(|s| parse_iso(s).unwrap())
    .collect()
}

// ---------------------------------------------------------------- D1: secular rates

#[test]
fn d1_lte440_two_published_statements_agree_through_l_b() {
    // LTE440 states dTCL/dTCB = 1 − 1.48253621667e-8 and dTDB/dTCL = 1 − 6.798355238e-10; with
    // the defining L_B these must agree within the fit's ±1e-17 (each) and the rounding.
    let m = Model::default_model();
    let r = m.secular_rate(Scale::Tcl, Scale::Tdb).value();
    let other = 1.0 / (1.0 - LTE440_TCL_TDB) - 1.0;
    assert!(
        (r - other).abs() < 2.0 * 2f64.sqrt() * 1e-17,
        "{r:e} vs {other:e}"
    );
}

#[test]
fn d1_tcl_minus_tt_rate_is_58_74_us_per_day() {
    let m = Model::default_model();
    let b = m.secular_rate(Scale::Tcl, Scale::Tt);
    let us = b.value() * US_DAY;
    assert!((us - 58.7378).abs() < 1e-3, "{us}");
    // OSTP quotes 58.7 (one decimal): within half a unit of its last digit.
    assert!((us - OSTP_US_DAY).abs() <= 0.05, "{us}");
    // The stated uncertainty is the fit's 1e-17 → 0.86 ps/day.
    assert!((b.u_c() * NS_DAY * 1e3 - 0.864).abs() < 0.01);
}

#[test]
fn d1_first_principles_tcl_rate_agrees_with_lte440_within_its_uncertainty() {
    let lte = Model::default_model().secular_rate(Scale::Tcl, Scale::Tt);
    let fp = Model::new(
        RateSource::FirstPrinciples,
        W0Candidate::kshana_equatorial(),
    )
    .secular_rate(Scale::Tcl, Scale::Tt);
    let d = (fp.value() - lte.value()).abs();
    let u = (fp.u_c().powi(2) + lte.u_c().powi(2)).sqrt();
    assert!(d <= 2.0 * u, "difference {d:e} > 2u = {:e}", 2.0 * u);
    // And the first-principles uncertainty is a real one, not a token (sub-ns/day, > 0).
    assert!(
        fp.u_c() * NS_DAY > 0.05 && fp.u_c() * NS_DAY < 2.0,
        "{}",
        fp.u_c() * NS_DAY
    );
    // Its ΔL (TCL vs TCG) is ≈ 1.477 µs/day.
    let tcg = Model::new(
        RateSource::FirstPrinciples,
        W0Candidate::kshana_equatorial(),
    )
    .secular_rate(Scale::Tcg, Scale::Tcl)
    .value();
    assert!((tcg * US_DAY - 1.4769).abs() < 1e-3, "{}", tcg * US_DAY);
}

#[test]
fn d1_tl_minus_tt_rate_is_56_02_us_per_day_for_every_w0_candidate() {
    // Ashby and Patla (2024) give 56.02 µs/day (two decimals); we accept ±1 in the last quoted
    // digit for every built-in W0_L candidate. (The residual for the default candidate is
    // ≈ 0.006 µs/day: a cross-model difference, disclosed in docs/LUNAR-PRO.md.)
    for c in W0Candidate::all() {
        let m = Model::new(RateSource::Lte440, c.clone());
        let us = m.secular_rate(Scale::Tl, Scale::Tt).value() * US_DAY;
        assert!(
            (us - ASHBY_PATLA_TL_TT_US_DAY).abs() <= 0.01,
            "{}: {us}",
            c.name
        );
    }
    // And TCL − TL equals Turyshev's 2.712 µs/day for his selenoid value.
    let m = Model::new(RateSource::Lte440, W0Candidate::turyshev_selenoid());
    let d = m.secular_rate(Scale::Tcl, Scale::Tl).value() * US_DAY;
    assert!((d - 2.712).abs() < 1e-4, "{d}");
}

#[test]
fn d1_both_cgpm_options_are_selectable_and_differ_by_l_l() {
    let m = Model::default_model();
    let jd = parse_iso("2026-11-19T00:00:00").unwrap();
    let tcl = m.lunar_minus_utc(LunarOption::parse("tcl").unwrap(), jd);
    let tl = m.lunar_minus_utc(LunarOption::parse("TL").unwrap(), jd);
    let elapsed = jd.diff_seconds(TwoPartJd::from_f64(T0_JD));
    let expect = m.w0.l_l() * elapsed;
    assert!(((tcl.value() - tl.value()) - expect).abs() < 1e-6 * expect);
    // ≈ 49 ms by 2026 since the 1977 origin.
    assert!((tcl.value() - tl.value() - 0.0494).abs() < 0.001);
    // TL − UTC ≈ 69.184 s + 56 µs/day since 1977.
    assert!((tl.value() - 70.2048).abs() < 1e-3, "{}", tl.value());
}

// ---------------------------------------------------------------- D1: round trips

#[test]
fn d1_every_conversion_round_trips_to_sub_nanosecond() {
    let scales = [
        Scale::Utc,
        Scale::Tai,
        Scale::Tt,
        Scale::Tcg,
        Scale::Tdb,
        Scale::Tcb,
        Scale::Tcl,
        Scale::Tl,
    ];
    let m = Model::default_model();
    for jd in epochs() {
        for &a in &scales {
            for &b in &scales {
                let ab = m.convert(a, jd, b);
                let ba = m.convert(b, ab.jd_out, a);
                let err = ba.jd_out.diff_seconds(jd);
                assert!(err.abs() < 1e-10, "{a:?}->{b:?}->{a:?}: {err:e} s");
                // The budget of the way back is the negative of the way there.
                assert!((ab.offset.value() + ba.offset.value()).abs() < 1e-10);
                assert!(
                    (ab.offset.u_c() - ba.offset.u_c()).abs() <= 1e-6 * ab.offset.u_c() + 1e-18
                );
            }
        }
    }
}

#[test]
fn d1_tcl_to_tl_cancels_every_tcl_term() {
    let m = Model::default_model();
    let jd = parse_iso("2026-11-19T00:00:00").unwrap();
    let c = m.convert(Scale::Tcl, jd, Scale::Tl);
    let with_u: Vec<_> = c
        .offset
        .terms
        .iter()
        .filter(|t| t.u.abs() > 1e-15)
        .collect();
    assert_eq!(with_u.len(), 1, "{with_u:#?}");
    assert!(with_u[0].name.starts_with("L_L"));
    // A conventional W0 has no uncertainty at all.
    let m = Model::new(RateSource::Lte440, W0Candidate::conventional(2.8e6));
    assert!(m.convert(Scale::Tcl, jd, Scale::Tl).offset.u_c() < 1e-15);
}

// ---------------------------------------------------------------- D1: budget arithmetic

#[test]
fn d1_budget_sums_correctly_analytically_and_by_monte_carlo() {
    let m = Model::new(
        RateSource::FirstPrinciples,
        W0Candidate::turyshev_selenoid(),
    );
    let jd = parse_iso("2026-11-19T00:00:00").unwrap();
    for to in [Scale::Tl, Scale::Tcl, Scale::Tcb] {
        let b = m.convert(Scale::Utc, jd, to).offset;
        let sum: f64 = b.terms.iter().map(|t| t.value).sum();
        assert_eq!(sum, b.value());
        let q: f64 = b.terms.iter().map(|t| t.u * t.u).sum::<f64>().sqrt();
        assert!((q - b.u_c()).abs() <= 1e-15 * q.max(1e-30));
        assert!((b.expanded(2.0) - 2.0 * q).abs() <= 1e-15 * q.max(1e-30));
        let mc = b.monte_carlo_std(200_000, 7);
        assert!(
            (mc / b.u_c() - 1.0).abs() < 0.01,
            "{to:?}: mc {mc:e} vs {:e}",
            b.u_c()
        );
    }
    // Correlated difference: (A + B) − A leaves exactly B's uncertainty.
    let a = Budget::new(vec![kshana_pro::lunar_pro::Term::normal(
        "x",
        1.0,
        0.3,
        kshana_pro::lunar_pro::TermKind::B,
        "",
    )]);
    let mut ab = a.clone();
    ab.push(kshana_pro::lunar_pro::Term::normal(
        "y",
        2.0,
        0.4,
        kshana_pro::lunar_pro::TermKind::A,
        "",
    ));
    let d = ab.minus(&a);
    assert!((d.value() - 2.0).abs() < 1e-15 && (d.u_c() - 0.4).abs() < 1e-15);
}

#[test]
fn d1_secular_uncertainty_grows_linearly_from_t0() {
    let m = Model::default_model();
    let u = |s: &str| {
        m.convert(Scale::Tt, parse_iso(s).unwrap(), Scale::Tcl)
            .offset
            .term("L_CL (LTE440 secular fit)")
            .unwrap()
            .u
            .abs()
    };
    let t0 = TwoPartJd::from_f64(T0_JD);
    let a = parse_iso("2001-01-01T00:00:32.184")
        .unwrap()
        .diff_seconds(t0);
    let b = parse_iso("2025-01-01T00:00:32.184")
        .unwrap()
        .diff_seconds(t0);
    let ratio = u("2025-01-01T00:00:32.184") / u("2001-01-01T00:00:32.184");
    assert!((ratio - b / a).abs() < 1e-9);
}

// ---------------------------------------------------------------- D1: periodic terms, TDB

#[test]
fn d1_monthly_periodic_term_has_the_published_amplitude() {
    // LTE440 quotes ≈ 126 µs for the monthly term; −v_EMB·r/c² ≈ 29.8 km/s × 384 400 km / c².
    let m = Model::default_model();
    let start = parse_iso("2026-01-01T00:00:00").unwrap().to_f64();
    let vals: Vec<f64> = (0..300)
        .map(|k| {
            m.periodic_tcl(start + k as f64 * 0.1)
                .term("periodic: -v_EMB.r/c2 (monthly)")
                .unwrap()
                .value
        })
        .collect();
    let amp = (vals.iter().cloned().fold(f64::MIN, f64::max)
        - vals.iter().cloned().fold(f64::MAX, f64::min))
        / 2.0;
    assert!((amp * 1e6 - 126.0).abs() < 6.0, "{} µs", amp * 1e6);
}

#[test]
fn d1_circular_179_tdb_series_agrees_with_the_open_engine_series() {
    // The engine keeps two terms; Circular 179 seven. They must agree to within the sum of the
    // neglected amplitudes (≈ 60 µs) and both carry the 1.657 ms annual term.
    let mut max: f64 = 0.0;
    let mut peak: f64 = 0.0;
    for k in 0..3000 {
        let jd = T0_JD + k as f64 * 9.1;
        let d = tdb_minus_tt_circ179(jd) - kshana::timescales::tdb_minus_tt(jd);
        max = max.max(d.abs());
        peak = peak.max(tdb_minus_tt_circ179(jd).abs());
    }
    assert!(max < 60e-6, "{max:e}");
    assert!((peak - 1.68e-3).abs() < 0.05e-3, "{peak:e}");
}

// ---------------------------------------------------------------- D1: W0 sensitivity

#[test]
fn d1_w0_sensitivity_slope_and_height_equivalent() {
    let slope = d_rate_d_w0_ns_day();
    assert!((slope + 9.6133e-4).abs() < 1e-7, "{slope:e}");
    let r = W0Candidate::kshana_equatorial();
    let g = moon_surface_gravity();
    // A candidate 1 km "higher" (W0 smaller by g·1000 m) runs faster by g·1000/c² per day.
    let hi = W0Candidate::conventional(r.w0_m2_s2 - g * 1000.0);
    let rows = w0_sensitivity(RateSource::Lte440, &r, &[r.clone(), hi]);
    assert!(rows[0].delta_vs_ref_ns_day.abs() < 1e-12);
    assert!((rows[1].height_equiv_m - 1000.0).abs() < 1e-6);
    // The open engine's topographic spread per metre is the same physics.
    let per_m = kshana::lunar_time::topographic_spread_ns_per_day()
        / kshana::lunar_time::LUNAR_TOPO_ELEVATION_SPAN_M;
    assert!((rows[1].delta_vs_ref_ns_day / 1000.0 - per_m).abs() < 0.002 * per_m);
    // The built-in candidates span about 1 ns/day: the TL rate depends on an undefined W0.
    let all = w0_sensitivity(RateSource::Lte440, &r, &W0Candidate::all());
    let span = all
        .iter()
        .map(|x| x.tl_minus_tt_us_day)
        .fold(f64::MIN, f64::max)
        - all
            .iter()
            .map(|x| x.tl_minus_tt_us_day)
            .fold(f64::MAX, f64::min);
    assert!(
        span * 1e3 > 0.3 && span * 1e3 < 5.0,
        "{} ns/day",
        span * 1e3
    );
    for row in &all {
        assert!(
            (row.tcl_minus_tl_ns_day - 2712.0).abs() < 3.0,
            "{}",
            row.name
        );
    }
}

#[path = "lunar_pro/d3_holdover.rs"]
mod d3_holdover;

#[path = "lunar_pro/d2_circular.rs"]
mod d2_circular;

#[path = "lunar_pro/d4_offset_codec.rs"]
mod d4_offset_codec;

#[path = "lunar_pro/d5_lunex.rs"]
mod d5_lunex;

#[path = "lunar_pro/cli.rs"]
mod cli;
