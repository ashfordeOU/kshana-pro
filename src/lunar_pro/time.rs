// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D1. Lunar time with GUM uncertainty: TCB, TCG, TT, TDB, TCL, TL and UTC
//!
//! Transforms between the barycentric, geocentric and terrestrial time scales, Lunar Coordinate
//! Time (TCL, IAU 2024 Resolution II) and a scaled lunar time TL on a lunar equipotential
//! `W0_L`, and UTC, with a GUM uncertainty budget on every term. Both options that CGPM 2026
//! Resolution 6 leaves open (an unscaled TCL, or a scaled TL) can be selected
//! ([`LunarOption`]); [`w0_sensitivity`] gives the ns/day consequence of each candidate `W0_L`
//! (gap 8: "a lunar W0 is not defined", Defraigne, Meynadier and Bourgoin 2025).
//!
//! ## The relations (all rates as `d(X)/d(TT) − 1`, all offsets `X − TT` at one TT instant)
//!
//! * `TAI = TT − 32.184 s`; `UTC = TAI − ΔAT` with ΔAT from the open engine's leap-second table
//!   ([`kshana::timescales::tai_minus_utc`]). Exact.
//! * `TT = TCG − L_G (JD_TCG − T0)·86400` (IAU 2000 B1.9), `L_G = 6.969290134e-10` defining
//!   ([`kshana::lunar_time::L_G`]). Exact.
//! * `TDB − TT`: the seven-term series of USNO Circular 179 (Kaplan 2005) eq. 2.6, stated there
//!   to be good to about 10 µs (1600–2200): a rectangular term of half-width 10 µs. (The open
//!   engine's two-term [`kshana::timescales::tdb_minus_tt`] is kept as a cross-check.)
//! * `TDB = TCB − L_B (JD_TCB − T0)·86400 + TDB0` (IAU 2006 B3), `L_B = 1.550519768e-8`,
//!   `TDB0 = −6.55e-5 s`, defining. Exact.
//! * TCL: `dTCL/dTCB = 1 − L_CL`. Two sources ([`RateSource`]):
//!   - **LTE440** (Lu, Yang and Xie, Purple Mountain Observatory): `L_CL = 1.48253621667e-8`,
//!     with the stated `±1e-17` of its preliminary least-squares fit taken as a standard
//!     uncertainty;
//!   - **first principles**: `L_CL = L_C + ΔL` with `L_C = 1.48082686741e-8 ± 2e-17` (IERS
//!     Conventions 2010, Table 1.1) and
//!     `ΔL = [ (GM_E − GM_M)⟨1/r⟩ + (μ_E − μ_M)(⟨v²⟩/2 + ⟨U_tide,⊙⟩) ] / c²`, the long-term
//!     average of the difference of the external potential and kinetic terms at the Moon and
//!     at the Earth, from the open engine's analytic Moon and Sun series
//!     ([`kshana::ephem`], [`kshana::lunar_time::moon_geocentric_velocity_m_s`]) or a JPL
//!     kernel ([`kshana::ephem_provider::KernelEphemeris`]). The cross terms
//!     `v_EMB·v_r + ∇U_⊙·r` are an exact time derivative (`d(v_EMB·r)/dt`) and so average to
//!     zero; they give the periodic term below.
//! * Periodic part of `TCL − TT` (selenocentre against geocentre, simultaneous in TCB):
//!   `−v_EMB·r/c²` (the ≈127 µs monthly term; LTE440 quotes ≈126 µs) and the lunar
//!   eccentricity term `−2e√(G(M_E+M_M)a)/c² · sin l` (≈0.48 µs). Origin convention as
//!   implemented: `TCL − TT = 0` at `T0` (illustrative until confirmed against the IAU 2024
//!   Resolution II text).
//! * TL: `dTL/dTCL = 1 − L_L`, `L_L = W0_L/c²`, `TL = TCL` at `T0`.
//!
//! ## Rates to keep straight (research note §2.2)
//! TCL runs ahead of TT by ≈58.74 µs/day (LTE440; OSTP quotes 58.7). A clock on the lunar
//! equipotential, i.e. TL, runs ahead of TT by ≈56.02 µs/day (Ashby and Patla 2024). The
//! difference, ≈2.71 µs/day, is `L_L` (Turyshev 2025: 2.712 µs/day at the selenoid). The
//! "≈56 µs/day" figure is therefore the TL rate, not the TCL rate.
//!
//! ## Uncertainty
//! Every term of every offset is a [`Term`] with its source; secular terms grow with
//! `|t − T0|` because a rate uncertainty does. Same-named terms are the same input, so a TCL to
//! TL conversion cancels the TCL terms and keeps only the `W0_L` term. The ephemeris term of
//! the first-principles rate is the difference between two ephemeris sources where the caller
//! supplies them (for example DE440 and DE421 kernels); without kernels it is the analytic
//! series against the JPL Horizons Moon vectors vendored in `tests/fixtures/lunar_llr`. The
//! open engine exposes DE421 and DE440 only for the lunar *orientation*
//! ([`kshana::lunar_frame`]), which does not enter a time-scale rate, so no DE421-vs-DE440
//! position difference is claimed without kernels.
//!
//! Evidence label: **MODELLED**. The secular rates are cross-checked against published model
//! values; nothing is validated against lunar clock data (none exists yet).

use kshana::timescales::{self, TwoPartJd, JD_J2000, SECONDS_PER_DAY};
use serde::{Deserialize, Serialize};

use super::budget::{Budget, Term, TermKind};

/// `c²` (m²/s²), from the open engine.
pub const C2: f64 = kshana::lunar_time::C2_M2_S2;
/// `T0 = 1977-01-01T00:00:32.184 TT` as a Julian date (IAU 1991 A4, IAU 2000 B1.9, IAU 2006 B3).
pub const T0_JD: f64 = 2_443_144.500_372_5;
/// `L_G` (IAU 2000 B1.9), defining; the open engine's constant.
pub const L_G: f64 = kshana::lunar_time::L_G;
/// `L_B` (IAU 2006 B3), defining.
pub const L_B: f64 = 1.550_519_768e-8;
/// `TDB0` (IAU 2006 B3), defining (s).
pub const TDB0_S: f64 = -6.55e-5;
/// `L_C` (IERS Conventions 2010, Table 1.1).
pub const L_C: f64 = 1.480_826_867_41e-8;
/// Standard uncertainty of `L_C` (IERS Conventions 2010, Table 1.1).
pub const U_L_C: f64 = 2e-17;
/// LTE440: `dTCL/dTCB = 1 − L_CL`.
pub const LTE440_L_CL: f64 = 1.482_536_216_67e-8;
/// LTE440's stated precision of its preliminary secular fit, taken as a standard uncertainty.
pub const LTE440_U_L_CL: f64 = 1e-17;
/// LTE440: `dTDB/dTCL = 1 − 6.798355238e-10` (a second, independent statement of the same fit).
pub const LTE440_TCL_TDB: f64 = 6.798_355_238e-10;
/// Ashby and Patla (AJ 168:112, 2024): a lunar-surface clock against a geoid clock, µs/day.
pub const ASHBY_PATLA_TL_TT_US_DAY: f64 = 56.02;
/// The OSTP memorandum (2 Apr 2024) figure, µs/day.
pub const OSTP_US_DAY: f64 = 58.7;
/// Turyshev (arXiv 2507.22145): TCL − TL at the selenoid, µs/day.
pub const TURYSHEV_SELENOID_US_DAY: f64 = 2.712;
/// Turyshev: at the lunar South Pole, µs/day.
pub const TURYSHEV_SOUTH_POLE_US_DAY: f64 = 2.711;
/// Turyshev: `L_L` is known only to about this (dimensionless).
pub const TURYSHEV_U_L_L: f64 = 2e-15;
/// Lunar orbit eccentricity and semi-major axis used by the periodic eccentricity term.
pub const MOON_E: f64 = 0.0549;
pub const MOON_A_M: f64 = 3.847_48e8;
/// Half-width of the periodic-series truncation (evection, variation and the weighting of the
/// eccentricity term are left out), s.
pub const PERIODIC_TRUNC_HALF_WIDTH_S: f64 = 0.25e-6;
/// Half-width of the Circular 179 TDB − TT series, s.
pub const TDB_SERIES_HALF_WIDTH_S: f64 = 10e-6;

const GM_E: f64 = kshana::forces::MU_EARTH;
const GM_M: f64 = kshana::forces::MU_MOON;
const GM_S: f64 = kshana::forces::MU_SUN;

/// The time scales.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    Utc,
    Tai,
    Tt,
    Tcg,
    Tdb,
    Tcb,
    Tcl,
    Tl,
}

impl Scale {
    pub fn parse(s: &str) -> Result<Scale, String> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "utc" => Scale::Utc,
            "tai" => Scale::Tai,
            "tt" => Scale::Tt,
            "tcg" => Scale::Tcg,
            "tdb" => Scale::Tdb,
            "tcb" => Scale::Tcb,
            "tcl" => Scale::Tcl,
            "tl" => Scale::Tl,
            o => {
                return Err(format!(
                    "unknown time scale '{o}' (utc|tai|tt|tcg|tdb|tcb|tcl|tl)"
                ))
            }
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Scale::Utc => "UTC",
            Scale::Tai => "TAI",
            Scale::Tt => "TT",
            Scale::Tcg => "TCG",
            Scale::Tdb => "TDB",
            Scale::Tcb => "TCB",
            Scale::Tcl => "TCL",
            Scale::Tl => "TL",
        }
    }
}

/// The two options CGPM 2026 Resolution 6 leaves open for the lunar reference time scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LunarOption {
    /// Unscaled Lunar Coordinate Time (preferred by ESA and the CCTF scientific community).
    Tcl,
    /// Scaled on a lunar equipotential `W0_L` (preferred by NASA and JAXA).
    Tl,
}

impl LunarOption {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "tcl" => Ok(LunarOption::Tcl),
            "tl" => Ok(LunarOption::Tl),
            o => Err(format!("unknown lunar option '{o}' (tcl|tl)")),
        }
    }
    pub fn scale(self) -> Scale {
        match self {
            LunarOption::Tcl => Scale::Tcl,
            LunarOption::Tl => Scale::Tl,
        }
    }
}

/// Where the TCL secular rate comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RateSource {
    /// The published LTE440 secular fit.
    Lte440,
    /// Kshana's first-principles average (see the module docs).
    FirstPrinciples,
}

impl RateSource {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "lte440" => Ok(RateSource::Lte440),
            "first-principles" => Ok(RateSource::FirstPrinciples),
            o => Err(format!(
                "unknown rate source '{o}' (lte440|first-principles)"
            )),
        }
    }
}

/// A candidate lunar equipotential `W0_L` for TL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct W0Candidate {
    pub name: String,
    /// `W0_L` (m²/s², positive).
    pub w0_m2_s2: f64,
    /// Standard uncertainty of `W0_L` as a realisation of the named surface (m²/s²); zero for
    /// a conventional (adopted) value.
    pub u_m2_s2: f64,
    pub source: String,
}

impl W0Candidate {
    /// `L_L = W0_L / c²`.
    pub fn l_l(&self) -> f64 {
        self.w0_m2_s2 / C2
    }
    /// The open engine's lunar surface potential (degree-2 zonal plus rotation, on the
    /// equator; [`kshana::lunar_gauge::LunarSurfacePotential::KSHANA`]).
    pub fn kshana_equatorial() -> Self {
        let w0 = kshana::lunar_gauge::LunarSurfacePotential::KSHANA.equatorial_potential_m2_s2();
        W0Candidate {
            name: "kshana-equatorial-j2".into(),
            w0_m2_s2: w0,
            u_m2_s2: TURYSHEV_U_L_L * C2,
            source: "open engine lunar_gauge::LunarSurfacePotential::KSHANA (GM, 1738.14 km, J2, \
                     rotation; Ashby and Patla 2024 Eq. 10 truncated at degree 2); u: Turyshev's \
                     2e-15 knowledge of L_L"
                .into(),
        }
    }
    /// A point mass at the open engine's mean lunar radius (no J2, no rotation).
    pub fn point_mass_mean_radius() -> Self {
        let w0 = GM_M / kshana::lunar_time::RE_MOON_M;
        let eq = Self::kshana_equatorial().w0_m2_s2;
        W0Candidate {
            name: "point-mass-mean-radius".into(),
            w0_m2_s2: w0,
            u_m2_s2: (w0 - eq).abs(),
            source: "GM_Moon / 1737.4 km (open engine lunar_time::RE_MOON_M); u: its difference \
                     from the equatorial J2 surface"
                .into(),
        }
    }
    /// From a published TCL − TL rate in µs/day, rounded to `resolution_us_day`.
    pub fn from_rate(name: &str, us_day: f64, resolution_us_day: f64, source: &str) -> Self {
        let to_w0 = |r: f64| r * 1e-6 / SECONDS_PER_DAY * C2;
        W0Candidate {
            name: name.into(),
            w0_m2_s2: to_w0(us_day),
            u_m2_s2: to_w0(resolution_us_day / 2.0) / 3f64.sqrt(),
            source: source.into(),
        }
    }
    pub fn turyshev_selenoid() -> Self {
        Self::from_rate(
            "turyshev-selenoid",
            TURYSHEV_SELENOID_US_DAY,
            0.001,
            "Turyshev, arXiv 2507.22145: 2.712 µs/day at the selenoid (u: quoted resolution)",
        )
    }
    pub fn turyshev_south_pole() -> Self {
        Self::from_rate(
            "turyshev-south-pole",
            TURYSHEV_SOUTH_POLE_US_DAY,
            0.001,
            "Turyshev, arXiv 2507.22145: 2.711 µs/day at the South Pole (u: quoted resolution)",
        )
    }
    /// A conventional value (as if adopted by IAU/IAG): exact by definition.
    pub fn conventional(w0_m2_s2: f64) -> Self {
        W0Candidate {
            name: "conventional".into(),
            w0_m2_s2,
            u_m2_s2: 0.0,
            source: "a conventional (adopted) value: exact by definition".into(),
        }
    }
    /// The built-in candidates.
    pub fn all() -> Vec<W0Candidate> {
        vec![
            Self::kshana_equatorial(),
            Self::point_mass_mean_radius(),
            Self::turyshev_selenoid(),
            Self::turyshev_south_pole(),
        ]
    }
    /// A built-in candidate by name, or `conventional:<W0>`.
    pub fn parse(s: &str) -> Result<Self, String> {
        if let Some(v) = s.strip_prefix("conventional:") {
            let w0: f64 = v.parse().map_err(|_| format!("bad W0 value '{v}'"))?;
            if !(w0.is_finite() && w0 > 0.0) {
                return Err("W0 must be positive".into());
            }
            return Ok(Self::conventional(w0));
        }
        Self::all()
            .into_iter()
            .find(|c| c.name == s)
            .ok_or_else(|| {
                format!(
                    "unknown W0 candidate '{s}' (kshana-equatorial-j2|point-mass-mean-radius|\
                 turyshev-selenoid|turyshev-south-pole|conventional:<m2/s2>)"
                )
            })
    }
}

/// The long-term averages behind the first-principles TCL rate.
#[derive(Clone, Debug, Serialize)]
pub struct OrbitAverages {
    /// `⟨1/r⟩` (1/m).
    pub inv_r: f64,
    /// `⟨v_r²⟩` (m²/s²), geocentric Moon velocity.
    pub v2: f64,
    /// `⟨U_tide,⊙⟩` (m²/s²): the Sun's quadrupole tidal potential at the Moon relative to Earth.
    pub tide: f64,
    /// `ΔL` from the full window, and from each half (for the averaging-window term).
    pub delta_l: f64,
    pub delta_l_halves: [f64; 2],
    pub source: String,
}

/// `ΔL` from the three averages.
pub fn delta_l_from(inv_r: f64, v2: f64, tide: f64) -> f64 {
    let mu_e = GM_E / (GM_E + GM_M);
    let mu_m = GM_M / (GM_E + GM_M);
    ((GM_E - GM_M) * inv_r + (mu_e - mu_m) * (v2 / 2.0 + tide)) / C2
}

fn tide_potential(r: [f64; 3], s: [f64; 3]) -> f64 {
    let rr = norm(r);
    let ss = norm(s);
    let cos = dot(r, s) / (rr * ss);
    GM_S * rr * rr * (3.0 * cos * cos - 1.0) / (2.0 * ss.powi(3))
}

/// The averages over `days` daily samples from `start_jd_tt`, from a state function giving
/// `(moon r, moon v, sun r)` geocentric.
pub fn orbit_averages_with<F>(
    start_jd_tt: f64,
    days: usize,
    source: &str,
    mut f: F,
) -> Result<OrbitAverages, String>
where
    F: FnMut(f64) -> Result<([f64; 3], [f64; 3], [f64; 3]), String>,
{
    if days < 60 {
        return Err("the averaging window must be at least 60 days".into());
    }
    let mut acc = [[0.0f64; 3]; 2];
    let mut n = [0usize; 2];
    for k in 0..days {
        let jd = start_jd_tt + k as f64;
        let (r, v, s) = f(jd)?;
        let h = usize::from(k >= days / 2);
        acc[h][0] += 1.0 / norm(r);
        acc[h][1] += dot(v, v);
        acc[h][2] += tide_potential(r, s);
        n[h] += 1;
    }
    let avg = |h: usize, i: usize| acc[h][i] / n[h] as f64;
    let tot = |i: usize| (acc[0][i] + acc[1][i]) / (n[0] + n[1]) as f64;
    Ok(OrbitAverages {
        inv_r: tot(0),
        v2: tot(1),
        tide: tot(2),
        delta_l: delta_l_from(tot(0), tot(1), tot(2)),
        delta_l_halves: [
            delta_l_from(avg(0, 0), avg(0, 1), avg(0, 2)),
            delta_l_from(avg(1, 0), avg(1, 1), avg(1, 2)),
        ],
        source: source.into(),
    })
}

/// The 18.6-year nodal cycle in days, the default averaging window.
pub const NODAL_DAYS: usize = 6_798;

/// Averages from the open engine's analytic Moon and Sun series, from J2000 over the nodal
/// cycle.
pub fn orbit_averages_analytic() -> OrbitAverages {
    orbit_averages_with(JD_J2000, NODAL_DAYS, "open engine analytic series", |jd| {
        let t = (jd - JD_J2000) / 36_525.0;
        Ok((
            kshana::ephem::moon_position(t),
            kshana::lunar_time::moon_geocentric_velocity_m_s(t),
            kshana::ephem::sun_position(t),
        ))
    })
    .expect("analytic series cannot fail")
}

/// Averages from a JPL kernel (Moon and Sun geocentric), over `days` from `start_jd_tt`; the
/// velocity is a ±60 s central difference.
pub fn orbit_averages_kernel(
    k: &kshana::ephem_provider::KernelEphemeris,
    start_jd_tt: f64,
    days: usize,
) -> Result<OrbitAverages, String> {
    use kshana::jd2::Jd2;
    let src = format!("JPL kernel {} (sha256 {})", k.source(), k.kernel_sha256());
    orbit_averages_with(start_jd_tt, days, &src, |jd| {
        let at = |dt: f64| k.moon_geocentric_tt(Jd2::new(jd).add_seconds(dt));
        let r = at(0.0)?;
        let p = at(60.0)?;
        let m = at(-60.0)?;
        let v = [
            (p[0] - m[0]) / 120.0,
            (p[1] - m[1]) / 120.0,
            (p[2] - m[2]) / 120.0,
        ];
        let s = k.sun_geocentric_tt(Jd2::new(jd))?;
        Ok((r, v, s))
    })
}

/// The JPL Horizons geocentric Moon vectors (twelve weekly epochs in 2015) vendored with the
/// lunar laser-ranging fixture (`tests/fixtures/lunar_llr/NOTICE.md` §4).
const HORIZONS_MOON: &str =
    include_str!("../../tests/fixtures/lunar_llr/horizons_moon_geocentric_2015.csv");

/// The rms relative difference of the geocentric distance between the analytic Moon series and
/// JPL Horizons at the vendored epochs (the ephemeris accuracy proxy of the first-principles
/// rate when no kernels are supplied).
pub fn analytic_vs_horizons_distance_rel() -> f64 {
    let mut s = 0.0;
    let mut n = 0.0;
    for l in HORIZONS_MOON.lines() {
        if l.starts_with('#') || l.starts_with("jd_tt") || l.trim().is_empty() {
            continue;
        }
        let f: Vec<f64> = l.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        if f.len() != 4 {
            continue;
        }
        let hz = norm([f[1] * 1e3, f[2] * 1e3, f[3] * 1e3]);
        let an = norm(kshana::ephem::moon_position((f[0] - JD_J2000) / 36_525.0));
        s += ((an - hz) / hz).powi(2);
        n += 1.0;
    }
    (s / n).sqrt()
}

/// The lunar time model: rate source and `W0_L`, with any ephemeris-difference input.
#[derive(Clone, Debug, Serialize)]
pub struct Model {
    pub rate_source: RateSource,
    pub w0: W0Candidate,
    /// First-principles averages (computed when the rate source asks for them).
    pub averages: Option<OrbitAverages>,
    /// Extra ephemeris sources (kernels) whose ΔL differences become the ephemeris term.
    pub ephemeris_alternatives: Vec<OrbitAverages>,
}

impl Model {
    /// A model with the given rate source and `W0_L`.
    pub fn new(rate_source: RateSource, w0: W0Candidate) -> Self {
        let averages = match rate_source {
            RateSource::FirstPrinciples => Some(orbit_averages_analytic()),
            RateSource::Lte440 => None,
        };
        Model {
            rate_source,
            w0,
            averages,
            ephemeris_alternatives: Vec::new(),
        }
    }

    /// The default: LTE440 rate, the open engine's equatorial `W0_L`.
    pub fn default_model() -> Self {
        Self::new(RateSource::Lte440, W0Candidate::kshana_equatorial())
    }

    /// The budget of `L_CL` (dimensionless): its value and uncertainty terms.
    pub fn l_cl(&self) -> Budget {
        match self.rate_source {
            RateSource::Lte440 => Budget::new(vec![Term::normal(
                "L_CL (LTE440 secular fit)",
                LTE440_L_CL,
                LTE440_U_L_CL,
                TermKind::B,
                "LTE440 (Lu, Yang, Xie): 1 - 1.48253621667e-8, preliminary fit ±1e-17",
            )]),
            RateSource::FirstPrinciples => {
                let a = self
                    .averages
                    .clone()
                    .unwrap_or_else(orbit_averages_analytic);
                let mu_e = GM_E / (GM_E + GM_M);
                let mu_m = GM_M / (GM_E + GM_M);
                let pot = (GM_E - GM_M) * a.inv_r / C2;
                let kin = (mu_e - mu_m) * a.v2 / 2.0 / C2;
                let tide = (mu_e - mu_m) * a.tide / C2;
                let mut b = Budget::new(vec![
                    Term::normal(
                        "L_C (IERS 2010)",
                        L_C,
                        U_L_C,
                        TermKind::B,
                        "IERS Conventions 2010 Table 1.1: 1.48082686741e-8 ± 2e-17",
                    ),
                    Term::exact(
                        "dL potential (GM_E - GM_M)<1/r>/c2",
                        pot,
                        "open engine analytic Moon series, nodal-cycle average",
                    ),
                    Term::exact(
                        "dL kinetic (mu_E - mu_M)<v2>/2c2",
                        kin,
                        "open engine lunar_time::moon_geocentric_velocity_m_s",
                    ),
                    Term::exact(
                        "dL solar tide",
                        tide,
                        "Sun quadrupole tide, open engine analytic Sun series",
                    ),
                ]);
                if self.ephemeris_alternatives.is_empty() {
                    let d = analytic_vs_horizons_distance_rel();
                    b.push(Term::normal(
                        "ephemeris: Moon distance",
                        0.0,
                        pot * d,
                        TermKind::B,
                        "rms relative distance difference, analytic series vs JPL Horizons \
                         (tests/fixtures/lunar_llr)",
                    ));
                    b.push(Term::normal(
                        "ephemeris: Moon speed",
                        0.0,
                        kin * 2.0 * d,
                        TermKind::B,
                        "assumed: speed error of the series of the same relative size as its \
                         distance error (2x in v2)",
                    ));
                } else {
                    let spread = self
                        .ephemeris_alternatives
                        .iter()
                        .map(|e| (e.delta_l - a.delta_l).abs())
                        .fold(0.0, f64::max);
                    b.push(Term::rectangular(
                        "ephemeris: source difference",
                        0.0,
                        spread,
                        "largest dL difference between the ephemeris sources supplied",
                    ));
                }
                b.push(Term::rectangular(
                    "averaging window",
                    0.0,
                    (a.delta_l_halves[0] - a.delta_l_halves[1]).abs() / 2.0,
                    "half the dL difference between the two halves of the window",
                ));
                b.push(Term::rectangular(
                    "model truncation (1PN, J2, planets)",
                    0.0,
                    1e-16,
                    "allocation: Earth J2 at the Moon ~3e-18, 2PN differences ~3e-19, planetary \
                     tides <1e-17",
                ));
                b
            }
        }
    }

    /// The secular rate of `a` against TT, `d(a)/d(TT) − 1`, as a budget (dimensionless).
    pub fn rate_vs_tt(&self, a: Scale) -> Budget {
        match a {
            Scale::Tt | Scale::Tai | Scale::Utc | Scale::Tdb => {
                Budget::new(vec![Term::exact("zero secular rate", 0.0, "by definition")])
            }
            Scale::Tcg => Budget::new(vec![Term::exact(
                "L_G",
                L_G / (1.0 - L_G),
                "IAU 2000 B1.9, defining",
            )]),
            Scale::Tcb => Budget::new(vec![Term::exact(
                "L_B",
                L_B / (1.0 - L_B),
                "IAU 2006 B3, defining",
            )]),
            Scale::Tcl => {
                // dTCL/dTT = (1 − L_CL)/(1 − L_B): rate = (L_B − L_CL)/(1 − L_B).
                let mut b = Budget::new(vec![Term::exact(
                    "L_B",
                    L_B / (1.0 - L_B),
                    "IAU 2006 B3, defining",
                )]);
                for t in self.l_cl().terms {
                    b.push(t.scaled(-1.0 / (1.0 - L_B)));
                }
                b
            }
            Scale::Tl => {
                let tcl = self.rate_vs_tt(Scale::Tcl);
                let r = tcl.value();
                // (1 + r)(1 − L_L) − 1 = r − L_L(1 + r).
                let mut b = tcl;
                b.push(self.w0_term(-(1.0 + r)));
                b
            }
        }
    }

    /// The `W0_L` term with sensitivity `k` on `L_L`.
    fn w0_term(&self, k: f64) -> Term {
        Term::normal(
            &format!("L_L (W0_L: {})", self.w0.name),
            k * self.w0.l_l(),
            k * self.w0.u_m2_s2 / C2,
            if self.w0.u_m2_s2 == 0.0 {
                TermKind::Defining
            } else {
                TermKind::B
            },
            &self.w0.source,
        )
    }

    /// The secular rate of `a` against `b` (dimensionless), correlated terms cancelled.
    pub fn secular_rate(&self, a: Scale, b: Scale) -> Budget {
        self.rate_vs_tt(a).minus(&self.rate_vs_tt(b))
    }

    /// The periodic part `P(t)` of `TCL − TT` (s), with its terms.
    pub fn periodic_tcl(&self, jd_tt: f64) -> Budget {
        let p = periodic_parts(jd_tt);
        let p0 = periodic_parts(T0_JD);
        let cross = p.0 - p0.0;
        let ecc = p.1 - p0.1;
        // The Sun's barycentric motion (~13 m/s against Earth's ~29.8 km/s) is left out of the
        // EMB velocity, and the series carry their own distance error.
        let d = analytic_vs_horizons_distance_rel();
        let rel = ((13.0f64 / 29_780.0).powi(2) + d * d).sqrt();
        Budget::new(vec![
            Term::normal(
                "periodic: -v_EMB.r/c2 (monthly)",
                cross,
                (p.0.abs() + p0.0.abs()) * rel,
                TermKind::B,
                "open engine analytic Sun and Moon series; u: Sun barycentric motion left out \
                 (13 m/s / 29.78 km/s) and series distance error vs JPL Horizons",
            ),
            Term::exact(
                "periodic: lunar eccentricity",
                ecc,
                "-2 e sqrt(G(M_E+M_M) a)/c2 sin l (Keplerian)",
            ),
            Term::rectangular(
                "periodic: series truncation",
                0.0,
                PERIODIC_TRUNC_HALF_WIDTH_S,
                "evection, variation and the mass weighting of the eccentricity term left out",
            ),
        ])
    }

    /// `s − TT` (seconds) at the TT instant `jd_tt`, as a budget.
    pub fn offset_from_tt(&self, s: Scale, jd_tt: TwoPartJd) -> Budget {
        let dt = jd_tt.diff_seconds(TwoPartJd::from_f64(T0_JD));
        let jd = jd_tt.to_f64();
        match s {
            Scale::Tt => Budget::default(),
            Scale::Tai => Budget::new(vec![Term::exact(
                "TT - TAI",
                -timescales::TT_MINUS_TAI,
                "32.184 s, defining",
            )]),
            Scale::Utc => {
                let mut jd_utc = jd - (timescales::TT_MINUS_TAI + 37.0) / SECONDS_PER_DAY;
                let mut dat = 37.0;
                for _ in 0..3 {
                    dat = timescales::tai_minus_utc(jd_utc);
                    jd_utc = jd - (timescales::TT_MINUS_TAI + dat) / SECONDS_PER_DAY;
                }
                Budget::new(vec![
                    Term::exact("TT - TAI", -timescales::TT_MINUS_TAI, "32.184 s, defining"),
                    Term::exact(
                        "TAI - UTC (leap seconds)",
                        -dat,
                        "open engine leap-second table (IERS Bulletin C)",
                    ),
                ])
            }
            Scale::Tcg => Budget::new(vec![Term::exact(
                "TCG - TT (L_G)",
                L_G / (1.0 - L_G) * dt,
                "IAU 2000 B1.9, defining",
            )]),
            Scale::Tdb => Budget::new(vec![tdb_term(jd)]),
            Scale::Tcb => {
                let tdb = tdb_term(jd);
                let d_tdb = dt + tdb.value;
                // TCB − TDB = (L_B·D − TDB0)/(1 − L_B), D in TDB seconds since T0.
                let k = 1.0 / (1.0 - L_B);
                Budget::new(vec![
                    tdb.scaled(k),
                    Term::exact(
                        "TCB - TDB (L_B, TDB0)",
                        (L_B * d_tdb - TDB0_S) * k,
                        "IAU 2006 B3, defining",
                    ),
                ])
            }
            Scale::Tcl => {
                let mut b = self.rate_vs_tt(Scale::Tcl).scaled(dt);
                for t in self.periodic_tcl(jd).terms {
                    b.push(t);
                }
                b
            }
            Scale::Tl => {
                let tcl = self.offset_from_tt(Scale::Tcl, jd_tt);
                // TL − TT = (TCL − TT) − L_L·(Δ + (TCL − TT)).
                let elapsed_tcl = dt + tcl.value();
                let mut b = tcl;
                b.push(self.w0_term(-elapsed_tcl));
                b
            }
        }
    }

    /// Convert the instant `jd` read in scale `from` to scale `to`.
    pub fn convert(&self, from: Scale, jd: TwoPartJd, to: Scale) -> Conversion {
        let mut jd_tt = jd;
        if from != Scale::Tt {
            for _ in 0..5 {
                let off = self.offset_from_tt(from, jd_tt).value();
                jd_tt = jd.add_seconds(-off);
            }
        }
        let off_to = self.offset_from_tt(to, jd_tt);
        let off_from = self.offset_from_tt(from, jd_tt);
        let budget = off_to.minus(&off_from);
        let out = jd.add_seconds(budget.value());
        let mut notes = Vec::new();
        if from == Scale::Utc || to == Scale::Utc {
            notes.push(
                "TAI - UTC from the open engine's leap-second table; a leap second announced \
                 after that table was built is not included"
                    .to_string(),
            );
        }
        if matches!(from, Scale::Tcl | Scale::Tl) || matches!(to, Scale::Tcl | Scale::Tl) {
            notes.push(
                "TCL/TL compared at the selenocentre (TL on the W0_L surface) against TT at the \
                 geocentre, simultaneous in TCB; origin TCL = TL = TT at T0 (illustrative until \
                 confirmed against IAU 2024 Res. II)"
                    .to_string(),
            );
        }
        Conversion {
            from,
            to,
            jd_in: jd,
            jd_out: out,
            jd_tt,
            offset: budget,
            notes,
        }
    }

    /// The broadcast-style offset `lunar − UTC` (s) at the UTC instant `jd_utc`.
    pub fn lunar_minus_utc(&self, option: LunarOption, jd_utc: TwoPartJd) -> Budget {
        self.convert(Scale::Utc, jd_utc, option.scale()).offset
    }
}

/// The result of a conversion.
#[derive(Clone, Debug, Serialize)]
pub struct Conversion {
    pub from: Scale,
    pub to: Scale,
    #[serde(serialize_with = "ser_jd")]
    pub jd_in: TwoPartJd,
    #[serde(serialize_with = "ser_jd")]
    pub jd_out: TwoPartJd,
    #[serde(serialize_with = "ser_jd")]
    pub jd_tt: TwoPartJd,
    /// `to − from` (s) with its budget.
    pub offset: Budget,
    pub notes: Vec<String>,
}

fn ser_jd<S: serde::Serializer>(j: &TwoPartJd, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeStruct;
    let mut st = s.serialize_struct("jd", 2)?;
    st.serialize_field("day", &j.day)?;
    st.serialize_field("frac", &j.frac)?;
    st.end()
}

/// `(−v_EMB·r/c², −2e√(GMa)/c² sin l)` at `jd_tt` (s).
fn periodic_parts(jd_tt: f64) -> (f64, f64) {
    let t = (jd_tt - JD_J2000) / 36_525.0;
    let r = kshana::ephem::moon_position(t);
    let vr = kshana::lunar_time::moon_geocentric_velocity_m_s(t);
    // Earth's heliocentric velocity is minus the geocentric Sun's (central difference, ±60 s).
    let h = 60.0 / SECONDS_PER_DAY / 36_525.0;
    let sp = kshana::ephem::sun_position(t + h);
    let sm = kshana::ephem::sun_position(t - h);
    let mu_m = GM_M / (GM_E + GM_M);
    let v_emb = [
        -(sp[0] - sm[0]) / 120.0 + mu_m * vr[0],
        -(sp[1] - sm[1]) / 120.0 + mu_m * vr[1],
        -(sp[2] - sm[2]) / 120.0 + mu_m * vr[2],
    ];
    let cross = -dot(v_emb, r) / C2;
    // Moon's mean anomaly l (Montenbruck and Gill §3.3.2 fundamental argument).
    let l = (134.962_92 + 477_198.867_53 * t).to_radians();
    let amp = 2.0 * MOON_E * ((GM_E + GM_M) * MOON_A_M).sqrt() / C2;
    (cross, -amp * l.sin())
}

/// TDB − TT from USNO Circular 179 eq. 2.6 (s), `t` in Julian centuries TT.
pub fn tdb_minus_tt_circ179(jd_tt: f64) -> f64 {
    let t = (jd_tt - JD_J2000) / 36_525.0;
    0.001_657 * (628.3076 * t + 6.2401).sin()
        + 0.000_022 * (575.3385 * t + 4.2970).sin()
        + 0.000_014 * (1256.6152 * t + 6.1969).sin()
        + 0.000_005 * (606.9777 * t + 4.0212).sin()
        + 0.000_005 * (52.9691 * t + 0.4444).sin()
        + 0.000_002 * (21.3299 * t + 5.5431).sin()
        + 0.000_010 * t * (628.3076 * t + 4.2490).sin()
}

fn tdb_term(jd_tt: f64) -> Term {
    Term::rectangular(
        "TDB - TT (Circular 179 series)",
        tdb_minus_tt_circ179(jd_tt),
        TDB_SERIES_HALF_WIDTH_S,
        "USNO Circular 179 (Kaplan 2005) eq. 2.6, about 10 µs (1600-2200)",
    )
}

/// One row of the `W0_L` sensitivity table.
#[derive(Clone, Debug, Serialize)]
pub struct W0Row {
    pub name: String,
    pub w0_m2_s2: f64,
    pub l_l: f64,
    /// `TCL − TL` rate (ns/day, positive).
    pub tcl_minus_tl_ns_day: f64,
    /// `TL − TT` rate (µs/day).
    pub tl_minus_tt_us_day: f64,
    /// Rate difference against the reference candidate (ns/day; positive: TL runs faster).
    pub delta_vs_ref_ns_day: f64,
    /// The height on the Moon that gives the same potential difference (m).
    pub height_equiv_m: f64,
    /// Standard uncertainty of the TL rate from this candidate's `W0_L` (ns/day).
    pub u_ns_day: f64,
    pub source: String,
}

/// `d(rate)/d(W0_L)` in ns/day per m²/s² (negative: a deeper potential slows TL).
pub fn d_rate_d_w0_ns_day() -> f64 {
    -SECONDS_PER_DAY * 1e9 / C2
}

/// Lunar surface gravity (m/s²) from the open engine's constants.
pub fn moon_surface_gravity() -> f64 {
    kshana::lunar_gauge::LunarSurfacePotential::KSHANA.effective_gravity_m_s2()
}

/// The ns/day consequence of each candidate `W0_L`, against `reference`.
pub fn w0_sensitivity(
    rate_source: RateSource,
    reference: &W0Candidate,
    candidates: &[W0Candidate],
) -> Vec<W0Row> {
    let tcl = Model::new(rate_source, reference.clone())
        .rate_vs_tt(Scale::Tcl)
        .value();
    let g = moon_surface_gravity();
    candidates
        .iter()
        .map(|c| {
            let tl = tcl - c.l_l() * (1.0 + tcl);
            W0Row {
                name: c.name.clone(),
                w0_m2_s2: c.w0_m2_s2,
                l_l: c.l_l(),
                tcl_minus_tl_ns_day: c.l_l() * (1.0 + tcl) * SECONDS_PER_DAY * 1e9,
                tl_minus_tt_us_day: tl * SECONDS_PER_DAY * 1e6,
                delta_vs_ref_ns_day: (c.w0_m2_s2 - reference.w0_m2_s2) * d_rate_d_w0_ns_day(),
                height_equiv_m: (reference.w0_m2_s2 - c.w0_m2_s2) / g,
                u_ns_day: c.u_m2_s2 * d_rate_d_w0_ns_day().abs(),
                source: c.source.clone(),
            }
        })
        .collect()
}

/// Parse `YYYY-MM-DDTHH:MM:SS[.f]` (no zone; read in whatever scale the caller names) into a
/// two-part JD, using the open engine's calendar routine.
pub fn parse_iso(s: &str) -> Result<TwoPartJd, String> {
    let bad = || format!("bad date-time '{s}' (YYYY-MM-DDTHH:MM:SS)");
    let s = s.trim().trim_end_matches('Z');
    let (d, t) = s.split_once(['T', ' ']).unwrap_or((s, "00:00:00"));
    let dp: Vec<&str> = d.split('-').collect();
    let tp: Vec<&str> = t.split(':').collect();
    if dp.len() != 3 || tp.len() < 2 {
        return Err(bad());
    }
    let y: i32 = dp[0].parse().map_err(|_| bad())?;
    let mo: u32 = dp[1].parse().map_err(|_| bad())?;
    let da: u32 = dp[2].parse().map_err(|_| bad())?;
    let h: u32 = tp[0].parse().map_err(|_| bad())?;
    let mi: u32 = tp[1].parse().map_err(|_| bad())?;
    let se: f64 = tp
        .get(2)
        .map(|x| x.parse())
        .transpose()
        .map_err(|_| bad())?
        .unwrap_or(0.0);
    if !(1..=12).contains(&mo)
        || !(1..=31).contains(&da)
        || h > 23
        || mi > 59
        || !(0.0..61.0).contains(&se)
    {
        return Err(bad());
    }
    let day = timescales::julian_date(y, mo, da, 0, 0, 0.0);
    Ok(TwoPartJd::from_f64(day).add_seconds(h as f64 * 3600.0 + mi as f64 * 60.0 + se))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
