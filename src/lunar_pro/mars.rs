// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D9. Mars coordinate time and MSD/MTC (gap 9)
//!
//! Two parts, deliberately kept apart because they answer different questions (research note 03
//! §3.3: Mars24's MTC and MSD are **solar-time** constructs; there is no adopted Mars coordinate
//! time analogous to TCL):
//!
//! ## 1. Mars Sol Date and Coordinated Mars Time (Allison and McEwen 2000, Mars24)
//! [`mars24`] follows NASA GISS "Mars24 Sunclock — Algorithm and Worked Examples" (updated
//! 2025-01-07, <https://www.giss.nasa.gov/tools/mars24/help/algorithm.html>) step for step:
//! A-2 to A-6 (JD, TT − UTC from the leap-second table, or the page's polynomial before 1972,
//! days since J2000 TT), B-1 to B-5 (mean anomaly, fictitious mean sun, the seven perturbation
//! terms PBS, equation of centre, areocentric solar longitude Ls), C-1 to C-5 (equation of time,
//! MST = MTC, local mean and true solar time, subsolar longitude), and MSD from C-2's bracket. The
//! tests reproduce both of the page's worked examples to its printed precision.
//!
//! ## 2. Mars coordinate time TCM and areoid time TM, with uncertainty
//! The analogue of D1's TCL and TL (names are Kshana's; nothing is adopted). To first order in
//! `1/c²`:
//! * `dTCM/dTCB = 1 − L_CM`, `L_CM = ⟨U_⊙ + v²/2⟩/c² + ⟨U_planets⟩/c²` at Mars's centre. For a
//!   Kepler orbit `⟨1/r⟩ = 1/a` and `⟨v²⟩ = GM/a` exactly, so `⟨U_⊙ + v²/2⟩ = (3/2)·GM_⊙/a`, with
//!   `a` from the engine's Standish elements ([`kshana::ephem::standish_elements`]); the other
//!   planets' mean potential is estimated as `Σ GM_p/a_p` (outer planets, coplanar circular) with a
//!   30 % rectangular half-width.
//! * The periodic part from Mars's eccentricity: `TCM − TCB ⊃ −(2/c²)·√(GM_⊙ a)·e·sin E` (the Mars
//!   analogue of the 1.66 ms annual term of TDB − TT), ≈ 11.4 ms amplitude, referred to zero at
//!   the 1977 origin `T0` (as TCG, TCB and D1's TCL are).
//! * `dTM/dTCM = 1 − L_M`, `L_M = W0_M/c²` with `W0_M` the areoid potential: degree-2 equatorial
//!   value `GM/R·(1 + J2/2) + ω²R²/2` from the engine's [`kshana::body::Body::mars`] (Konopliv
//!   MRO110 J2), uncertainty half its difference from the point-mass mean-radius value.
//! * The mean rate of an areoid clock against TT: `L_B − L_CM − L_M`, with `L_B` the IAU 2006 B3
//!   defining constant. Kshana: **487.7 µs/day**; Ashby and Patla (arXiv:2507.21388, 2025) give
//!   **477 µs/day** and a 226 µs/day variation over a Martian year. That value is **not in the
//!   research note** (which gives no Mars rate); it was taken from the arXiv abstract in this pass.
//!   The ≈10 µs/day (2 %) difference is an **unresolved cross-model difference** (it is far larger
//!   than either model's stated uncertainty here) and is disclosed, not tuned away.
//!
//! **Evidence label: MODELLED.** MSD/MTC reproduce the published Mars24 worked examples (a check
//! against another implementation of the same algorithm, not against Mars clocks). The TCM rate is
//! a first-principles model cross-checked against one published model value with a disclosed
//! difference. There is no Mars clock data.

use serde::Serialize;

use super::time::{self, Scale, C2, L_B, T0_JD};
use super::{Budget, Term, TermKind};

/// The statement every Mars output carries.
pub const STATUS: &str = "MODELLED (Kshana Pro D9): MSD/MTC per Allison & McEwen (2000) / Mars24, \
    reproducing its worked examples; TCM/TM are Kshana's first-principles analogues of TCL/TL, \
    not adopted scales";
/// GM of the Sun (m³/s², IAU 2009 TDB-compatible, as the engine).
pub const GM_SUN: f64 = kshana::forces::MU_SUN;
/// Ashby and Patla (arXiv:2507.21388, 2025): areoid clocks vs the Earth geoid, mean (µs/day).
pub const ASHBY_PATLA_MARS_US_DAY: f64 = 477.0;
/// Their variation over a Martian year (µs/day).
pub const ASHBY_PATLA_MARS_VARIATION_US_DAY: f64 = 226.0;
/// Mars24's sol-to-day ratio (C-2).
pub const SOL_DAYS: f64 = 1.027_491_251_7;

const PBS: [(f64, f64, f64); 7] = [
    (0.0071, 2.2353, 49.409),
    (0.0057, 2.7543, 168.173),
    (0.0039, 1.1177, 191.837),
    (0.0037, 15.7866, 21.736),
    (0.0021, 2.1354, 15.704),
    (0.0020, 2.4694, 95.528),
    (0.0018, 32.8493, 49.095),
];

fn wrap(x: f64, m: f64) -> f64 {
    x.rem_euclid(m)
}

/// Every step of the Mars24 algorithm.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Mars24 {
    pub jd_ut: f64,
    pub tt_minus_utc_s: f64,
    pub jd_tt: f64,
    pub dt_j2000: f64,
    /// B-1 mean anomaly (deg, unwrapped as printed by the page before reduction).
    pub m_deg: f64,
    /// B-2 fictitious mean sun (deg).
    pub alpha_fms_deg: f64,
    pub pbs_deg: f64,
    /// B-4 equation of centre ν − M (deg).
    pub nu_minus_m_deg: f64,
    /// B-5 areocentric solar longitude (deg, 0–360).
    pub ls_deg: f64,
    /// C-1 equation of time (deg).
    pub eot_deg: f64,
    /// Mars Sol Date.
    pub msd: f64,
    /// C-2 MST = Coordinated Mars Time (h, 0–24).
    pub mtc_h: f64,
    pub lon_west_deg: f64,
    /// C-3 local mean solar time (h).
    pub lmst_h: f64,
    /// C-4 local true solar time (h, 0–24).
    pub ltst_h: f64,
    /// C-5 subsolar longitude (deg west, 0–360).
    pub subsolar_lon_deg: f64,
}

/// TT − UTC (s) as Mars24 A-4: the leap-second table from 1972, the page's polynomial before.
pub fn tt_minus_utc(jd_ut: f64) -> f64 {
    if jd_ut >= 2_441_317.5 {
        kshana::timescales::tai_minus_utc(jd_ut) + 32.184
    } else {
        let t = (jd_ut - 2_451_545.0) / 36_525.0;
        64.184 + 59.0 * t - 51.2 * t * t - 67.1 * t.powi(3) - 16.4 * t.powi(4)
    }
}

/// The Mars24 algorithm at a UTC Julian date, for a site `lon_west_deg` (planetographic, west).
pub fn mars24(jd_ut: f64, lon_west_deg: f64) -> Mars24 {
    let tt_utc = tt_minus_utc(jd_ut);
    let jd_tt = jd_ut + tt_utc / 86_400.0;
    let dt = jd_tt - 2_451_545.0;
    let m = 19.3871 + 0.524_020_73 * dt;
    let afms = 270.3871 + 0.524_038_496 * dt;
    let pbs: f64 = PBS
        .iter()
        .map(|(a, tau, phi)| a * ((0.985_626 * dt / tau + phi).to_radians()).cos())
        .sum();
    let mr = m.to_radians();
    let nu_m = (10.691 + 3.0e-7 * dt) * mr.sin()
        + 0.623 * (2.0 * mr).sin()
        + 0.050 * (3.0 * mr).sin()
        + 0.005 * (4.0 * mr).sin()
        + 0.0005 * (5.0 * mr).sin()
        + pbs;
    let ls = afms + nu_m;
    let lr = ls.to_radians();
    let eot = 2.861 * (2.0 * lr).sin() - 0.071 * (4.0 * lr).sin() + 0.002 * (6.0 * lr).sin() - nu_m;
    let msd = (jd_tt - 2_451_549.5) / SOL_DAYS + 44_796.0 - 0.000_962_6;
    let mtc = wrap(24.0 * msd, 24.0);
    let lmst = wrap(mtc - lon_west_deg / 15.0, 24.0);
    let ltst = wrap(lmst + eot / 15.0, 24.0);
    Mars24 {
        jd_ut,
        tt_minus_utc_s: tt_utc,
        jd_tt,
        dt_j2000: dt,
        m_deg: m,
        alpha_fms_deg: afms,
        pbs_deg: pbs,
        nu_minus_m_deg: nu_m,
        ls_deg: wrap(ls, 360.0),
        eot_deg: eot,
        msd,
        mtc_h: mtc,
        lon_west_deg,
        lmst_h: lmst,
        ltst_h: ltst,
        subsolar_lon_deg: wrap(mtc * 15.0 + eot + 180.0, 360.0),
    }
}

/// `HH:MM:SS` of hours.
pub fn hms(h: f64) -> String {
    let s = (wrap(h, 24.0) * 3600.0).round() as u64 % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

// ------------------------------------------------------------------------------ TCM and TM

/// Mars's Standish (Table 1) orbital elements at a TDB Julian date: (a m, e, mean anomaly rad).
pub fn mars_elements(jd_tdb: f64) -> (f64, f64, f64) {
    let t = (jd_tdb - 2_451_545.0) / 36_525.0;
    let el = kshana::ephem::standish_elements(
        kshana::ephem::Planet::Mars,
        t,
        kshana::ephem::StandishTable::Table1,
    )
    .expect("Mars has Standish elements");
    (el.a_m, el.e, el.mean_anomaly_rad)
}

/// Outer planets' GM (m³/s²) and semi-major axes (AU): Jupiter, Saturn, Uranus, Neptune
/// (DE440-era GM values; mean distances).
const OUTER: [(f64, f64); 4] = [
    (1.266_865_34e17, 5.2026),
    (3.793_118_7e16, 9.5549),
    (5.793_939e15, 19.2184),
    (6.836_529e15, 30.1104),
];

/// `L_CM` with its budget.
pub fn l_cm() -> Budget {
    let (a, _, _) = mars_elements(T0_JD);
    let sun = 1.5 * GM_SUN / (a * C2);
    let planets: f64 = OUTER
        .iter()
        .map(|(gm, au)| gm / (au * kshana::ephem::AU_M * C2))
        .sum();
    Budget::new(vec![
        Term::normal(
            "L_CM solar: (3/2) GM_sun / (a_Mars c2)",
            sun,
            sun * 1e-5,
            TermKind::B,
            "Kepler orbit average; a from the engine's Standish elements (u: 1e-5 relative, mean vs osculating a)",
        ),
        Term::rectangular(
            "L_CM planets: sum GM_p / (a_p c2), outer planets",
            planets,
            0.3 * planets,
            "coplanar circular estimate, 30 % half-width",
        ),
    ])
}

/// The areoid potential (m²/s²) and its standard uncertainty.
pub fn areoid_w0() -> (f64, f64, String) {
    let b = kshana::body::Body::mars();
    let j2 = b.zonals[0];
    let eq = b.mu / b.re * (1.0 + j2 / 2.0) + 0.5 * (b.rotation_rate * b.re).powi(2);
    let pm = b.mu / 3_389_500.0;
    (
        eq,
        (eq - pm).abs() / 2.0,
        "degree-2 equatorial GM/R(1+J2/2) + w2R2/2 from kshana::body::Body::mars (MRO110 J2); u: half the difference from GM/3389.5 km".into(),
    )
}

/// `L_M = W0_M/c²` with its budget.
pub fn l_m() -> Budget {
    let (w0, u, src) = areoid_w0();
    Budget::new(vec![Term::normal(
        "L_M = W0_Mars / c2",
        w0 / C2,
        u / C2,
        TermKind::B,
        &src,
    )])
}

/// Mean rate of an areoid clock against TT, `d(TM − TT)/dt = L_B − L_CM − L_M`, with its budget.
pub fn rate_tm_minus_tt() -> Budget {
    let mut b = Budget::new(vec![Term::exact(
        "L_B (IAU 2006 B3)",
        L_B,
        "defining constant",
    )]);
    for t in l_cm().terms.iter().chain(l_m().terms.iter()) {
        b.push(t.scaled(-1.0));
    }
    b
}

/// Instantaneous deviation of the TCM rate from its mean at `jd_tdb` (dimensionless):
/// `−2·GM_⊙·(1/r − 1/a)/c²`.
pub fn rate_deviation(jd_tdb: f64) -> f64 {
    let (a, e, m) = mars_elements(jd_tdb);
    let ea = kshana::ephem::solve_kepler(m, e);
    let r = a * (1.0 - e * ea.cos());
    -2.0 * GM_SUN * (1.0 / r - 1.0 / a) / C2
}

/// The periodic term of TCM − TCB (s): `−(2/c²)·√(GM a)·e·sin E`, minus its value at T0.
pub fn periodic_tcm_minus_tcb(jd_tdb: f64) -> f64 {
    let p = |jd: f64| {
        let (a, e, m) = mars_elements(jd);
        let ea = kshana::ephem::solve_kepler(m, e);
        -2.0 * (GM_SUN * a).sqrt() * e * ea.sin() / C2
    };
    p(jd_tdb) - p(T0_JD)
}

/// TCM and TM at an instant.
#[derive(Clone, Debug, Serialize)]
pub struct MarsCoordinateTime {
    pub utc: String,
    /// TCM − TCB (s) and its budget; TM − TCB likewise; TM − TT likewise.
    pub tcm_minus_tcb: Budget,
    pub tm_minus_tcb: Budget,
    pub tm_minus_tt: Budget,
    pub rate_tm_minus_tt_us_day: f64,
    pub rate_u_ns_day: f64,
    pub w0_m2_s2: f64,
    pub note: String,
}

/// TCM, TM and TM − TT at a UTC instant (TCB and TT from the D1 chain).
pub fn coordinate_time(utc_iso: &str) -> Result<MarsCoordinateTime, String> {
    let jd = time::parse_iso(utc_iso)?;
    let m = time::Model::default_model();
    let tcb = m.convert(Scale::Utc, jd, Scale::Tcb);
    let tt = m.convert(Scale::Utc, jd, Scale::Tt);
    let since = tcb
        .jd_out
        .diff_seconds(kshana::timescales::TwoPartJd::from_f64(T0_JD));
    let periodic = periodic_tcm_minus_tcb(tt.jd_out.to_f64());
    // TCM − TCB = −L_CM·(TCB − T0) + periodic.
    let mut tcm = l_cm().scaled(-since);
    tcm.push(Term::normal(
        "periodic: -(2/c2) sqrt(GM a) e sin E (Mars eccentricity)",
        periodic,
        // Standish Table 1 elements (~arcsec-level) and the omitted planetary periodic terms.
        1e-6,
        TermKind::B,
        "Kepler term from Standish elements; u: omitted planetary periodic terms (estimate)",
    ));
    // TM − TCB = (TCM − TCB) − L_M·(TCM − T0).
    let mut tm = tcm.clone();
    for t in l_m().scaled(-since).terms {
        tm.push(t);
    }
    // TM − TT = (TM − TCB) − (TT − TCB).
    let tt_minus_tcb = tt.offset.minus(&tcb.offset);
    let tm_tt = tm.minus(&tt_minus_tcb);
    let r = rate_tm_minus_tt();
    Ok(MarsCoordinateTime {
        utc: utc_iso.into(),
        tcm_minus_tcb: tcm,
        tm_minus_tcb: tm,
        tm_minus_tt: tm_tt,
        rate_tm_minus_tt_us_day: r.value() * 86_400e6,
        rate_u_ns_day: r.u_c() * 86_400e9,
        w0_m2_s2: areoid_w0().0,
        note: "TCM/TM: Kshana first-principles analogues of TCL/TL; origin TCM = TCB = TM at T0 (1977-01-01 TAI) by analogy, not adopted".into(),
    })
}
