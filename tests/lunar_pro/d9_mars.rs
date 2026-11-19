// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
// D9: Mars coordinate time and MSD/MTC (part of the tests/lunar_pro.rs test binary).
//
// Reference values: NASA GISS, "Mars24 Sunclock — Algorithm and Worked Examples" (updated
// 2025-01-07), https://www.giss.nasa.gov/tools/mars24/help/algorithm.html, worked examples 1 and
// 2, printed to five decimals.

use kshana_pro::lunar_pro::mars::*;

fn jd(s: &str) -> f64 {
    kshana_pro::lunar_pro::time::parse_iso(s).unwrap().to_f64()
}

/// One unit in the page's fifth decimal plus its rounding (the page rounds each printed step
/// from unrounded values, so a printed sum can differ from the sum of printed terms by 1e-5).
const T: f64 = 1.5e-5;

fn close(got: f64, want: f64, tol: f64, what: &str) {
    assert!(
        (got - want).abs() <= tol,
        "{what}: {got} vs published {want}"
    );
}

#[test]
fn d9_mars24_worked_example_1_near_coincident_times() {
    // 2000-01-06 00:00:00 UTC, 0° W.
    let m = mars24(jd("2000-01-06T00:00:00"), 0.0);
    close(m.jd_ut, 2_451_549.5, 1e-9, "JD_UT");
    close(m.tt_minus_utc_s, 64.184, 1e-9, "TT-UTC");
    close(m.jd_tt, 2_451_549.500_74, T, "JD_TT");
    close(m.dt_j2000, 4.500_74, T, "dt");
    close(m.m_deg, 21.745_58, T, "M");
    close(m.alpha_fms_deg, 272.745_66, T, "alpha_FMS");
    close(m.pbs_deg, 0.001_42, T, "PBS");
    close(m.nu_minus_m_deg, 4.441_93, T, "nu-M");
    close(m.ls_deg, 277.187_58, T, "Ls");
    close(m.eot_deg, -5.187_74, T, "EOT");
    close(m.mtc_h, 23.994_25, T, "MST");
    close(m.lmst_h, 23.994_25, T, "LMST");
    close(m.ltst_h, 23.648_40, T, "LTST");
    close(m.subsolar_lon_deg, 174.726_00, 6e-5, "subsolar longitude");
    assert_eq!(hms(m.mtc_h), "23:59:39");
    assert_eq!(hms(m.ltst_h), "23:38:54");
    // MST = 24 h × MSD (mod 24): the page's bracket, 1075103.99425 h.
    close(24.0 * m.msd, 1_075_103.994_25, 2e-4, "24 MSD");
}

#[test]
fn d9_mars24_worked_example_2_spirit_landing() {
    // MER-A Spirit landing: 2004-01-03 13:46:31 UTC, 184.702° W; local true midnight.
    let m = mars24(jd("2004-01-03T13:46:31"), 184.702);
    close(m.jd_ut, 2_453_008.073_97, T, "JD_UT");
    close(m.dt_j2000, 1_463.074_71, T, "dt");
    close(m.m_deg, 786.068_58, T, "M");
    close(m.alpha_fms_deg, 1_037.094_57, T, "alpha_FMS");
    close(m.pbs_deg, 0.016_14, T, "PBS");
    close(m.nu_minus_m_deg, 10.229_59, T, "nu-M");
    close(m.ls_deg, 327.324_16, T, "Ls");
    close(m.eot_deg, -12.775_53, T, "EOT");
    close(m.mtc_h, 13.165_37, T, "MST");
    close(m.lmst_h, 0.851_90, T, "LMST");
    // The page prints 0.00025 h although its own printed LMST + EOT/15 = 0.00020 h: we agree with
    // the sum of its printed steps; 5e-5 h (0.18 s) is within the page's rounding of LTST.
    close(m.ltst_h, 0.000_25, 6e-5, "LTST");
    close(m.subsolar_lon_deg, 4.705_00, 6e-5, "subsolar longitude");
    close(24.0 * m.msd, 1_109_173.165_37, 2e-4, "24 MSD");
}

#[test]
fn d9_msd_advances_one_per_sol_and_pre_1972_uses_the_polynomial() {
    let a = mars24(2_460_000.5, 0.0);
    let b = mars24(2_460_000.5 + SOL_DAYS, 0.0);
    close(b.msd - a.msd, 1.0, 1e-9, "one sol");
    close(b.mtc_h, a.mtc_h, 1e-6, "MTC repeats after a sol");
    // Before 1972 the page's polynomial: at J2000 itself it would give 64.184 s.
    close(
        tt_minus_utc(2_441_000.5),
        64.184 + 59.0 * ((2_441_000.5 - 2_451_545.0) / 36_525.0)
            - 51.2 * ((2_441_000.5f64 - 2_451_545.0) / 36_525.0).powi(2)
            - 67.1 * ((2_441_000.5f64 - 2_451_545.0) / 36_525.0).powi(3)
            - 16.4 * ((2_441_000.5f64 - 2_451_545.0) / 36_525.0).powi(4),
        1e-12,
        "pre-1972",
    );
    // After 2017: 37 leap seconds → 69.184 s.
    close(
        tt_minus_utc(jd("2026-11-19T00:00:00")),
        69.184,
        1e-9,
        "2026",
    );
    // Local time: 15° west is one hour earlier.
    let w = mars24(2_460_000.5, 15.0);
    close(
        (a.lmst_h - w.lmst_h).rem_euclid(24.0),
        1.0,
        1e-9,
        "longitude",
    );
}

#[test]
fn d9_l_cm_follows_kepler_and_matches_the_earth_analogue() {
    // The same formula applied to the Earth gives IAU L_C (TCG vs TCB) to within the Moon's and
    // planets' share: (3/2)GM/(1 AU c2) = 1.4806e-8 vs L_C = 1.48083e-8.
    let c2 = kshana_pro::lunar_pro::time::C2;
    let earth = 1.5 * GM_SUN / (kshana::ephem::AU_M * c2);
    assert!(
        (earth / kshana_pro::lunar_pro::time::L_C - 1.0).abs() < 5e-4,
        "{earth:e}"
    );
    // Mars: (3/2)GM/(1.5237 AU c2) ≈ 9.717e-9, the planets adding ~2.6e-12.
    let l = l_cm();
    assert!((l.value() - 9.72e-9).abs() < 0.01e-9, "{:e}", l.value());
    assert!(l.u_c() > 1e-13 && l.u_c() < 1e-12, "{:e}", l.u_c());
    // Areoid W0 ≈ 12.65e6 m2/s2 (12.2 µs/day).
    let (w0, u, _) = areoid_w0();
    assert!((w0 - 12.652e6).abs() < 3e3, "{w0}");
    assert!(u > 1e3 && u < 2e4, "{u}");
    close(l_m().value() * 86_400e6, 12.16, 0.02, "L_M us/day");
}

#[test]
fn d9_areoid_clock_rate_against_tt_and_the_published_value() {
    let r = rate_tm_minus_tt();
    let us = r.value() * 86_400e6;
    // Kshana first principles: 487.7 µs/day.
    close(us, 487.7, 0.3, "TM - TT rate");
    // Ashby and Patla (arXiv:2507.21388) give 477 µs/day: the ≈10.7 µs/day (2.2 %) difference is
    // a disclosed, unresolved cross-model difference; this test pins its size so a change shows.
    let d = us - ASHBY_PATLA_MARS_US_DAY;
    assert!(d > 9.0 && d < 12.5, "difference {d} µs/day");
    // Our stated uncertainty is sub-µs/day (planet term), so the difference is not covered by it:
    // it is reported, not absorbed.
    let u = r.u_c() * 86_400e6;
    assert!(u > 0.01 && u < 0.5, "{u}");
    // The rate varies over a Martian year by 2GM/(ac2)·2e/(1−e2) peak to peak ≈ 211 µs/day
    // (Mars's eccentricity alone); Ashby and Patla's 226 µs/day also contains the Earth's annual
    // term (±28 µs/day), so agreement within that is all that can be asked.
    let start = jd("2026-01-01T00:00:00");
    let dev: Vec<f64> = (0..700)
        .map(|k| rate_deviation(start + k as f64) * 86_400e6)
        .collect();
    let pp =
        dev.iter().cloned().fold(f64::MIN, f64::max) - dev.iter().cloned().fold(f64::MAX, f64::min);
    assert!((pp - 211.0).abs() < 3.0, "{pp}");
    assert!(
        (pp - ASHBY_PATLA_MARS_VARIATION_US_DAY).abs() < 2.0 * 28.0,
        "{pp}"
    );
    // Its mean over a Martian orbit is zero (the mean rate is L_CM).
    let mean: f64 = (0..687)
        .map(|k| rate_deviation(start + k as f64))
        .sum::<f64>()
        / 687.0;
    assert!(mean.abs() < 2e-12, "{mean:e}");
}

#[test]
fn d9_periodic_term_amplitude_and_its_rate_is_the_rate_deviation() {
    // Amplitude (2/c2)√(GM a)·e ≈ 11.4 ms (Earth's analogue is the 1.66 ms TDB term).
    let start = jd("2026-01-01T00:00:00");
    let p: Vec<f64> = (0..700)
        .map(|k| periodic_tcm_minus_tcb(start + k as f64))
        .collect();
    let amp = (p.iter().cloned().fold(f64::MIN, f64::max)
        - p.iter().cloned().fold(f64::MAX, f64::min))
        / 2.0;
    assert!((amp * 1e3 - 11.4).abs() < 0.2, "{} ms", amp * 1e3);
    // d/dt of the periodic term equals the rate deviation (numerical derivative).
    for k in [10.0, 200.0, 400.0] {
        let t = start + k;
        let h = 0.5;
        let num =
            (periodic_tcm_minus_tcb(t + h) - periodic_tcm_minus_tcb(t - h)) / (2.0 * h * 86_400.0);
        let dev = rate_deviation(t);
        assert!((num - dev).abs() < 0.01 * 2.5e-9, "{num:e} vs {dev:e}");
    }
    close(
        periodic_tcm_minus_tcb(kshana_pro::lunar_pro::time::T0_JD),
        0.0,
        1e-15,
        "zero at T0",
    );
}

#[test]
fn d9_coordinate_time_budget_is_consistent() {
    let a = coordinate_time("2026-11-19T00:00:00").unwrap();
    let b = coordinate_time("2036-11-19T00:00:00").unwrap();
    // Over ten years TM − TT grows by the mean rate times the interval, up to the periodic terms
    // of TM (±11.4 ms) and of TT−TCB (±1.7 ms).
    let days = 3653.0;
    let grow = b.tm_minus_tt.value() - a.tm_minus_tt.value();
    let want = a.rate_tm_minus_tt_us_day * 1e-6 * days;
    assert!((grow - want).abs() < 0.03, "{grow} vs {want}");
    // TM − TCB minus TCM − TCB is exactly the L_M term.
    let lm = a.tm_minus_tcb.value() - a.tcm_minus_tcb.value();
    let lm_term = a.tm_minus_tcb.term("L_M = W0_Mars / c2").unwrap().value;
    assert!((lm - lm_term).abs() < 1e-12 && lm < 0.0);
    // Uncertainty grows with time from T0 (secular terms dominate).
    assert!(b.tm_minus_tt.u_c() > a.tm_minus_tt.u_c());
    // TM runs ahead of TT by ~0.488 ms/day since 1977 (~18 220 days): ≈ 8.9 s, ± periodic terms.
    assert!(
        a.tm_minus_tt.value() > 5.0 && a.tm_minus_tt.value() < 12.0,
        "{}",
        a.tm_minus_tt.value()
    );
    assert!(coordinate_time("not a time").is_err());
}
