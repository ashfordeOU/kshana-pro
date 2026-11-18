// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Groundwave propagation: Primary, Secondary and Additional Secondary Factors, field
//! strength and propagation-induced ECD for LORAN/eLoran (90–110 kHz, carrier 100 kHz).
//!
//! Models (`[propagation.ground] model = …`):
//!
//! * `simple`: first-order (PF only, inverse-distance field with mild attenuation); smoke
//!   runs only.
//! * `smooth_earth`: homogeneous smooth spherical earth of conductivity σ and relative
//!   permittivity ε_r ([`super::smooth_earth`]: Sommerfeld–Norton flat earth with Wait's
//!   curvature correction at short range, Wait/Bremmer residue series beyond, smoothly
//!   joined).
//! * `millington`: mixed paths over a [`ConductivityMap`] (uniform, polygons or raster) by
//!   Millington's method ([`super::millington`]) on amplitude, phase and group delay.
//!
//! # Delay decomposition (RTCM / Loran-C convention)
//!
//! ```text
//! delay = d/c  +  PF  +  SF  +  ASF  (+ temporal ΔASF)
//! PF  = (n − 1)·d/c,  n = 1.000338                     (excess over vacuum)
//! SF  = phase delay of the groundwave over all-seawater (σ = 5 S/m, ε_r = 80) beyond c/n,
//!       including the induction (near-field) terms     (smooth_earth eq. 3)
//! ASF = SF_mixed − SF_seawater                         (Millington on the actual path)
//! ```
//!
//! For `smooth_earth` the "mixed" path is the homogeneous ground itself, so
//! `ASF = SF_ground − SF_sea` (zero for seawater).
//!
//! # ECD
//!
//! The groundwave attenuation function is dispersive. The engine delays the whole pulse by
//! the phase delay (carrier timing) and shifts the envelope by `PathEffect::ecd_s`, so
//!
//! ```text
//! ECD_prop = τ_g − τ_p,   τ_p = −arg(N·W)/ω₀,   τ_g = −Δarg(N·W)/Δω over 90–110 kHz
//! ```
//!
//! (first-order, linear-phase approximation of the pulse distortion across the 20 kHz
//! band; the same Millington construction is applied to τ_g on mixed paths). **Sign, as
//! computed**: beyond a few hundred km the diffraction (residue-series) phase lag grows
//! only like `ω^{1/3}` (`x ∝ α ∝ ω^{1/3}`), so `τ_p` *decreases* with frequency and
//! `τ_g < τ_p`: the envelope arrives **earlier** relative to the carrier than it left, i.e.
//! the received ECD becomes negative with distance (≈ −1.0 µs at 1000 km over sea and
//! −1.2 µs over σ = 3 mS/m land with `k = K_LORAN`, ≈ −0.05 to −0.3 µs within 300 km;
//! the induction terms add a small negative ECD inside ~20 km). The magnitude matches the
//! published LORAN range (tenths of a µs to ~1–2 µs over ~1000 km, larger over land than
//! over sea); the sign follows from the theory rather than from a fitted value. MODELLED.
//!
//! # Temporal variability
//!
//! `temporal` adds a deterministic diurnal and seasonal term to ASF,
//! `ΔASF(t) = L·[A_d cos(2π(h − h_d)/24) + A_s cos(2π(D − D_s)/365.2425)]`, with `h` the UTC
//! hour and `D` the day of year at scene time `t` (from the scene start given to [`build`]
//! by its [`BuildContext`], or from an explicit `epoch_day_of_year` / `epoch_hour_utc`
//! override in the `temporal` table), and `L` the
//! land fraction of the path (`scale_by_land_fraction`, default true, so an all-sea path
//! keeps ASF = 0). `sigma_seasonal_frac` scales every land conductivity (σ < 1 S/m) by
//! `1 + f·cos(2π(D₀ − D_σ)/365.2425)` evaluated at the scene epoch `D₀` (frozen for the run:
//! scenes last ≪ a season). MODELLED.
//!
//! # Performance
//!
//! Homogeneous-ground curves are tabulated once per (σ, ε_r, k) in a process-wide cache.
//! For maps, the ASF/amplitude/group-delay corrections are computed by Millington at the
//! nodes of a lat/lon grid (`asf_grid_deg`, default 0.005°) around the receiver, memoised
//! per station, and bilinearly interpolated, so a moving receiver sees a continuous delay
//! and Millington runs only when the receiver enters a new cell. Distance, PF, SF and the
//! near-field terms are evaluated exactly at every call.
//!
//! # TODO
//!
//! Integral-equation groundwave over an elevation profile (Hufford 1952; Ott's WAGNER
//! program) is not implemented: it needs a terrain source and an external reference to
//! test against. Until then terrain enters only through the effective conductivity map.

use super::conductivity::{ConductivityConfig, ConductivityMap, GroundConst};
use super::millington::{millington, sections_from_samples};
use super::smooth_earth::{near_field, Ground, GroundCurve, GROUP_HALF_BAND_HZ, SEAWATER};
use super::{BuildContext, PathEffect, PathKind, PropagationModel, C};
use crate::chain::Station;
use crate::geo::{direct, inverse, path_points, Geo};
use crate::time::{AbsTime, Ps};
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

/// Atmospheric refractive index used for the Primary Factor (LORAN-C convention).
pub const N_ATM: f64 = 1.000_338;

/// LORAN carrier frequency (Hz).
pub const F0_HZ: f64 = 100_000.0;

/// Effective-earth-radius factor that reproduces the LORAN standard seawater SF (the
/// Harris polynomial of the Loran-C User Handbook, itself a fit to the Johler–Keiller–
/// Walters NBS Circular 573 computations) with this crate's smooth-earth theory: within
/// 15 ns from 50 to 2000 km (tests). It lies between the geometric earth (k = 1, +17 % SF
/// at 1000 km) and the ITU-R P.368 surface standard atmosphere (k = 4/3, −6 %), consistent
/// with an LF groundwave sampling a height range of several km over which the refractivity
/// gradient of an exponential atmosphere is weaker than at the surface. A one-parameter
/// calibration, stated as such; `k_factor = 1.3333` gives the ITU convention.
pub const K_LORAN: f64 = 1.24;

/// Field of a short vertical monopole over perfect ground at 1 km per √kW (µV/m):
/// 300 mV/m·√(P/1 kW) at 1 km.
pub const E1_UV_M_PER_SQRT_KW_AT_1KM: f64 = 300_000.0;

/// Curve table extent and resolution (intervals uniform in √d).
const CURVE_D_MAX_M: f64 = 6.0e6;
const CURVE_N: usize = 3000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum GroundConfig {
    /// First-order model: PF only (no SF/ASF), field `E = E1·√P / d` with a mild
    /// exponential attenuation. For tests and smoke runs; not a fidelity model.
    Simple {
        #[serde(default = "d_att")]
        atten_db_per_1000km: f64,
    },
    /// Homogeneous smooth spherical earth.
    SmoothEarth {
        #[serde(default = "d_sea_sigma")]
        sigma_s_m: f64,
        /// Default from σ (ITU-R P.527 categories; 80 for σ ≥ 1 S/m).
        #[serde(default)]
        eps_r: Option<f64>,
        #[serde(default = "d_k")]
        k_factor: f64,
        #[serde(default)]
        temporal: Option<TemporalConfig>,
    },
    /// Mixed paths over a conductivity map, Millington's method.
    Millington {
        #[serde(default)]
        conductivity: ConductivityConfig,
        #[serde(default = "d_k")]
        k_factor: f64,
        /// Spacing of the conductivity samples along the geodesic (km).
        #[serde(default = "d_sample")]
        sample_km: f64,
        /// Grid (degrees) on which the ASF correction is memoised and interpolated.
        #[serde(default = "d_grid")]
        asf_grid_deg: f64,
        #[serde(default)]
        temporal: Option<TemporalConfig>,
    },
}

fn d_att() -> f64 {
    3.0
}
fn d_sea_sigma() -> f64 {
    5.0
}
fn d_k() -> f64 {
    K_LORAN
}
fn d_sample() -> f64 {
    2.0
}
fn d_grid() -> f64 {
    0.005
}
fn d_true() -> bool {
    true
}

impl Default for GroundConfig {
    fn default() -> Self {
        GroundConfig::Simple {
            atten_db_per_1000km: d_att(),
        }
    }
}

/// Deterministic temporal variability of ASF (see the module documentation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TemporalConfig {
    /// Override of the day of year (1.0 = 1 January 00:00 UTC) and UTC hour at scene time
    /// 0. When both are absent they are derived from the scene start ([`build`] with its
    /// [`BuildContext`]); when only one is given the other takes its former default (day
    /// 1.0, hour 0.0), so older scenario files keep their meaning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch_day_of_year: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch_hour_utc: Option<f64>,
    /// Diurnal amplitude (ns) and UTC hour of its maximum.
    #[serde(default)]
    pub diurnal_amp_ns: f64,
    #[serde(default)]
    pub diurnal_peak_hour_utc: f64,
    /// Seasonal amplitude (ns) and day of year of its maximum.
    #[serde(default)]
    pub seasonal_amp_ns: f64,
    #[serde(default)]
    pub seasonal_peak_day: f64,
    /// Scale the ΔASF terms by the land fraction of the path.
    #[serde(default = "d_true")]
    pub scale_by_land_fraction: bool,
    /// Seasonal relative change of land conductivity, and the day of its maximum.
    #[serde(default)]
    pub sigma_seasonal_frac: f64,
    #[serde(default)]
    pub sigma_peak_day: f64,
}

const YEAR_D: f64 = 365.2425;

impl TemporalConfig {
    /// Epoch (day of year, UTC hour) at scene time 0 used by the formulas: the explicit
    /// override, else day 1.0 / hour 0.0 (see [`TemporalConfig::resolved`]).
    pub fn epoch(&self) -> (f64, f64) {
        (
            self.epoch_day_of_year.unwrap_or(1.0),
            self.epoch_hour_utc.unwrap_or(0.0),
        )
    }

    /// A copy whose epoch is fixed: kept if either epoch field is set, otherwise taken
    /// from the scene start `start` (day of year without its fractional part, and the
    /// UTC hour).
    pub fn resolved(&self, start: AbsTime) -> TemporalConfig {
        let mut t = self.clone();
        if t.epoch_day_of_year.is_none() && t.epoch_hour_utc.is_none() {
            let u = start.utc_day_hour();
            t.epoch_day_of_year = Some(u.day_of_year.floor());
            t.epoch_hour_utc = Some(u.hour_utc);
        }
        let (d, h) = t.epoch();
        t.epoch_day_of_year = Some(d);
        t.epoch_hour_utc = Some(h);
        t
    }

    /// ΔASF (s) at scene time `t_s` for a path with land fraction `land`.
    pub fn delta_asf_s(&self, t_s: f64, land: f64) -> f64 {
        let (d0, h0) = self.epoch();
        let hours = h0 + t_s / 3600.0;
        let day = d0 + hours / 24.0;
        let h = hours.rem_euclid(24.0);
        let v = self.diurnal_amp_ns * (2.0 * PI * (h - self.diurnal_peak_hour_utc) / 24.0).cos()
            + self.seasonal_amp_ns * (2.0 * PI * (day - self.seasonal_peak_day) / YEAR_D).cos();
        let l = if self.scale_by_land_fraction {
            land
        } else {
            1.0
        };
        l * v * 1e-9
    }
    /// Land-conductivity scale at the epoch.
    pub fn sigma_scale(&self) -> f64 {
        let (d0, h0) = self.epoch();
        1.0 + self.sigma_seasonal_frac
            * (2.0 * PI * (d0 + h0 / 24.0 - self.sigma_peak_day) / YEAR_D).cos()
    }
}

pub struct Simple {
    pub atten_db_per_1000km: f64,
}

impl PropagationModel for Simple {
    fn name(&self) -> String {
        "groundwave-simple".into()
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        let d = inverse(st.position, rx.pos).distance_m.max(1.0);
        let geom = d / C;
        let pf_excess = (N_ATM - 1.0) * geom;
        let d_km = d / 1000.0;
        let field = E1_UV_M_PER_SQRT_KW_AT_1KM * st.power_kw.sqrt() / d_km.max(1e-3)
            * 10f64.powf(-self.atten_db_per_1000km * d_km / 1000.0 / 20.0);
        vec![PathEffect {
            kind: PathKind::Ground,
            delay: Ps::from_s(geom + pf_excess),
            field_uv_m: field,
            ecd_s: 0.0,
            phase_rad: 0.0,
            geom_ns: geom * 1e9,
            pf_ns: pf_excess * 1e9,
            sf_ns: 0.0,
            asf_ns: 0.0,
        }]
    }
}

/// Process-wide cache of homogeneous-ground curves, keyed by (σ, ε_r, k) bit patterns.
type CurveCache = Mutex<HashMap<(u64, u64, u64), Arc<GroundCurve>>>;

fn curve_cache() -> &'static CurveCache {
    static C: OnceLock<CurveCache> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The tabulated 100 kHz curve of a homogeneous ground (cached).
pub fn ground_curve(g: Ground, k_factor: f64) -> Arc<GroundCurve> {
    let key = (g.sigma.to_bits(), g.eps_r.to_bits(), k_factor.to_bits());
    if let Some(c) = curve_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
    {
        return c.clone();
    }
    // computed outside the lock (deterministic; a concurrent duplicate is harmless)
    let c = Arc::new(GroundCurve::new(
        g,
        F0_HZ,
        k_factor,
        N_ATM,
        CURVE_D_MAX_M,
        CURVE_N,
    ));
    curve_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(key)
        .or_insert(c)
        .clone()
}

/// Groundwave corrections relative to all-seawater at one receiver node.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Correction {
    /// ASF: phase delay difference (s).
    pub phase_s: f64,
    /// ln-amplitude difference (nepers).
    pub ln_amp: f64,
    /// Group-delay difference (s).
    pub group_s: f64,
    /// Fraction of the path over land (σ < 1 S/m).
    pub land: f64,
}

/// Full-fidelity groundwave: homogeneous or Millington over a map.
pub struct Groundwave {
    pub k_factor: f64,
    pub map: ConductivityMap,
    pub sample_m: f64,
    pub grid_deg: f64,
    pub temporal: Option<TemporalConfig>,
    sigma_scale: f64,
    sea: Arc<GroundCurve>,
    /// Uniform map: its curve.
    uniform: Option<(Arc<GroundCurve>, f64)>,
    cache: Mutex<HashMap<(u64, u64, i64, i64), Correction>>,
    label: String,
}

impl Groundwave {
    pub fn new(
        map: ConductivityMap,
        k_factor: f64,
        sample_km: f64,
        grid_deg: f64,
        temporal: Option<TemporalConfig>,
        label: &str,
    ) -> Result<Groundwave, String> {
        if !(k_factor > 0.5 && k_factor < 10.0) {
            return Err(format!("k_factor {k_factor} out of range (0.5, 10)"));
        }
        if sample_km.is_nan() || sample_km <= 0.0 || grid_deg.is_nan() || grid_deg <= 0.0 {
            return Err("sample_km and asf_grid_deg must be positive".into());
        }
        let sigma_scale = temporal.as_ref().map(|t| t.sigma_scale()).unwrap_or(1.0);
        if sigma_scale.is_nan() || sigma_scale <= 0.0 {
            return Err("sigma_seasonal_frac makes a conductivity non-positive".into());
        }
        let sea = ground_curve(SEAWATER, k_factor);
        let mut gw = Groundwave {
            k_factor,
            map,
            sample_m: sample_km * 1000.0,
            grid_deg,
            temporal,
            sigma_scale,
            sea,
            uniform: None,
            cache: Mutex::new(HashMap::new()),
            label: label.into(),
        };
        if let Some(g) = gw.map.is_uniform() {
            let land = if g.sigma_s_m < 1.0 { 1.0 } else { 0.0 };
            gw.uniform = Some((ground_curve(gw.ground(g), k_factor), land));
        }
        Ok(gw)
    }

    /// Electrical constants used for a map ground (with the seasonal land scaling).
    fn ground(&self, g: GroundConst) -> Ground {
        let s = if g.sigma_s_m < 1.0 {
            g.sigma_s_m * self.sigma_scale
        } else {
            g.sigma_s_m
        };
        Ground {
            sigma: s,
            eps_r: g.eps(),
        }
    }

    /// Correction (mixed − seawater) for a path from `tx` to `rx` by Millington.
    pub fn correction_exact(&self, tx: Geo, rx: Geo) -> Correction {
        let d = inverse(tx, rx).distance_m;
        if let Some((c, land)) = &self.uniform {
            let a = c.at(d);
            let s = self.sea.at(d);
            return Correction {
                phase_s: a.phase_delay_s - s.phase_delay_s,
                ln_amp: a.ln_amp - s.ln_amp,
                group_s: a.group_delay_s - s.group_delay_s,
                land: *land,
            };
        }
        let n = ((d / self.sample_m).ceil() as usize).clamp(2, 20_000);
        let pts = path_points(tx, rx, n);
        let mut kinds: Vec<GroundConst> = Vec::new();
        let idx: Vec<usize> = pts
            .iter()
            .map(|(_, p)| {
                let g = self.map.at(*p);
                match kinds.iter().position(|k| *k == g) {
                    Some(i) => i,
                    None => {
                        kinds.push(g);
                        kinds.len() - 1
                    }
                }
            })
            .collect();
        let curves: Vec<Arc<GroundCurve>> = kinds
            .iter()
            .map(|g| ground_curve(self.ground(*g), self.k_factor))
            .collect();
        // boundaries: refine by bisection along the geodesic (to ~sample/4096) so the
        // result varies continuously with the receiver position
        let mut secs = sections_from_samples(&idx, d);
        let az = inverse(tx, rx).az1_rad;
        let step = d / n as f64;
        for k in 0..secs.len().saturating_sub(1) {
            let left = kinds[secs[k].ground];
            let mut lo = (secs[k].end_m - 0.5 * step).max(0.0);
            let mut hi = (secs[k].end_m + 0.5 * step).min(d);
            for _ in 0..12 {
                let mid = 0.5 * (lo + hi);
                if self.map.at(direct(tx, az, mid)) == left {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            secs[k].end_m = 0.5 * (lo + hi);
        }
        let mut land = 0.0;
        let mut start = 0.0;
        for s in &secs {
            if kinds[s.ground].sigma_s_m < 1.0 {
                land += s.end_m - start;
            }
            start = s.end_m;
        }
        let m = millington(&secs, |g, x| curves[g].at(x));
        let s = self.sea.at(d);
        Correction {
            phase_s: m.phase_delay_s - s.phase_delay_s,
            ln_amp: m.ln_amp - s.ln_amp,
            group_s: m.group_delay_s - s.group_delay_s,
            land: if d > 0.0 { land / d } else { 0.0 },
        }
    }

    /// Correction at a receiver position: exact for uniform maps, otherwise bilinear
    /// between memoised grid nodes.
    pub fn correction(&self, tx: Geo, rx: Geo) -> Correction {
        if self.uniform.is_some() {
            return self.correction_exact(tx, rx);
        }
        let q = self.grid_deg;
        let fi = rx.lat_deg / q;
        let fj = rx.lon_deg / q;
        let (i0, j0) = (fi.floor() as i64, fj.floor() as i64);
        let (u, v) = (fi - i0 as f64, fj - j0 as f64);
        let node = |i: i64, j: i64| -> Correction {
            let key = (tx.lat_deg.to_bits(), tx.lon_deg.to_bits(), i, j);
            if let Some(c) = self
                .cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&key)
            {
                return *c;
            }
            let c = self.correction_exact(tx, Geo::new(i as f64 * q, j as f64 * q));
            self.cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key, c);
            c
        };
        let c00 = node(i0, j0);
        let c10 = node(i0 + 1, j0);
        let c01 = node(i0, j0 + 1);
        let c11 = node(i0 + 1, j0 + 1);
        let bl = |f: fn(&Correction) -> f64| -> f64 {
            (1.0 - u) * (1.0 - v) * f(&c00)
                + u * (1.0 - v) * f(&c10)
                + (1.0 - u) * v * f(&c01)
                + u * v * f(&c11)
        };
        Correction {
            phase_s: bl(|c| c.phase_s),
            ln_amp: bl(|c| c.ln_amp),
            group_s: bl(|c| c.group_s),
            land: bl(|c| c.land),
        }
    }

    /// The groundwave path from a transmitter at `tx` radiating `power_kw` to `rx` at
    /// scene time `t_s`.
    pub fn evaluate(&self, tx: Geo, power_kw: f64, rx: Geo, t_s: f64) -> PathEffect {
        let d = inverse(tx, rx).distance_m.max(100.0);
        let geom = d / C;
        let pf = (N_ATM - 1.0) * geom;
        let sea = self.sea.at(d);
        let corr = self.correction(tx, rx);
        // induction terms at f0 and f0 ± 10 kHz
        let kd = |f: f64| N_ATM * 2.0 * PI * f / C * d;
        let n0 = near_field(kd(F0_HZ));
        let nm = near_field(kd(F0_HZ - GROUP_HALF_BAND_HZ));
        let np = near_field(kd(F0_HZ + GROUP_HALF_BAND_HZ));
        let tp_n = -n0.arg() / (2.0 * PI * F0_HZ);
        let tg_n = -(np.arg() - nm.arg()) / (2.0 * PI * 2.0 * GROUP_HALF_BAND_HZ);
        let sf = sea.phase_delay_s + tp_n;
        let temporal = self
            .temporal
            .as_ref()
            .map(|t| t.delta_asf_s(t_s, corr.land))
            .unwrap_or(0.0);
        let asf = corr.phase_s + temporal;
        let tau_p = sf + corr.phase_s;
        let tau_g = sea.group_delay_s + corr.group_s + tg_n;
        let spread = {
            let th = d / (self.k_factor * super::smooth_earth::R_EARTH);
            if th < 1e-6 {
                1.0
            } else {
                (th / th.sin()).sqrt()
            }
        };
        let field = E1_UV_M_PER_SQRT_KW_AT_1KM * power_kw.max(0.0).sqrt() / (d / 1000.0)
            * spread
            * n0.abs()
            * (sea.ln_amp + corr.ln_amp).exp();
        PathEffect {
            kind: PathKind::Ground,
            delay: Ps::from_s(geom + pf + sf + asf),
            field_uv_m: field,
            ecd_s: tau_g - tau_p,
            phase_rad: 0.0,
            geom_ns: geom * 1e9,
            pf_ns: pf * 1e9,
            sf_ns: sf * 1e9,
            asf_ns: asf * 1e9,
        }
    }
}

impl PropagationModel for Groundwave {
    fn name(&self) -> String {
        self.label.clone()
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        vec![self.evaluate(st.position, st.power_kw, rx.pos, rx.t_s)]
    }
}

/// Build a groundwave model; the temporal terms take their epoch from `ctx.start` unless
/// the configuration overrides it (see [`TemporalConfig::resolved`]).
pub fn build(cfg: &GroundConfig, ctx: &BuildContext) -> Result<Box<dyn PropagationModel>, String> {
    let resolve = |t: &Option<TemporalConfig>| t.as_ref().map(|t| t.resolved(ctx.start));
    match cfg {
        GroundConfig::Simple {
            atten_db_per_1000km,
        } => Ok(Box::new(Simple {
            atten_db_per_1000km: *atten_db_per_1000km,
        })),
        GroundConfig::SmoothEarth {
            sigma_s_m,
            eps_r,
            k_factor,
            temporal,
        } => {
            let map = ConductivityMap::build(&ConductivityConfig::Uniform {
                sigma_s_m: *sigma_s_m,
                eps_r: *eps_r,
            })?;
            let label = format!("groundwave-smooth-earth(σ={sigma_s_m} S/m, k={k_factor})");
            Ok(Box::new(Groundwave::new(
                map,
                *k_factor,
                d_sample(),
                d_grid(),
                resolve(temporal),
                &label,
            )?))
        }
        GroundConfig::Millington {
            conductivity,
            k_factor,
            sample_km,
            asf_grid_deg,
            temporal,
        } => {
            let map = ConductivityMap::build(conductivity)?;
            let label = format!("groundwave-millington(k={k_factor})");
            Ok(Box::new(Groundwave::new(
                map,
                *k_factor,
                *sample_km,
                *asf_grid_deg,
                resolve(temporal),
                &label,
            )?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::smooth_earth::SmoothEarth;
    use super::*;

    /// Seawater SF of the Loran-C User Handbook (Harris polynomial), in the continuous
    /// "modified" form published by Lo, Leathem, Offermans, Gunther et al., "Defining
    /// Primary, Secondary, Additional Secondary Factors for RTCM Minimum Performance
    /// Specifications", ILA 2009, eq. 5 (d in m, result ns).
    fn harris_ns(d: f64) -> f64 {
        if d <= 188_866.0 {
            -11.42 + 0.001_124_62 * d + 821_543.0 / d
        } else {
            -407.58 + 0.002_154_77 * d + 38_897_093.0 / d
        }
    }

    fn uniform(sigma: f64, eps: f64, k: f64) -> Groundwave {
        let m = ConductivityMap::build(&ConductivityConfig::Uniform {
            sigma_s_m: sigma,
            eps_r: Some(eps),
        })
        .unwrap();
        Groundwave::new(m, k, 2.0, 0.005, None, "t").unwrap()
    }

    fn tx() -> Geo {
        Geo::new(0.0, 0.0)
    }
    /// A point due north of `tx` at `km`.
    fn at_km(km: f64) -> Geo {
        crate::geo::direct(tx(), 0.0, km * 1000.0)
    }

    #[test]
    fn seawater_sf_matches_loran_harris_polynomial() {
        // VALIDATED: |SF − Harris| ≤ 20 ns from 50 to 2000 km with k = K_LORAN. 20 ns is
        // the spread between the two published LORAN seawater SF standards (Harris
        // polynomial vs Brunavs, up to 22 ns, Lo et al. 2009).
        let g = uniform(5.0, 80.0, K_LORAN);
        let mut worst: f64 = 0.0;
        for km in (1..=40).map(|i| i as f64 * 50.0) {
            let p = g.evaluate(tx(), 1.0, at_km(km), 0.0);
            let h = harris_ns(km * 1000.0);
            worst = worst.max((p.sf_ns - h).abs());
            assert!(
                (p.sf_ns - h).abs() <= 20.0,
                "{km} km: SF {} vs Harris {h}",
                p.sf_ns
            );
            assert!(p.asf_ns.abs() < 1e-9, "ASF over all-sea must be 0");
        }
        assert!(worst > 0.0);
        // published checkpoint: 1.01 µs at 390 statute miles (628 km), Loran-C SF chart
        let p = g.evaluate(tx(), 1.0, at_km(627.6), 0.0);
        assert!((p.sf_ns - 1010.0).abs() < 30.0, "{}", p.sf_ns);
        // ITU k = 4/3: still within 7 % beyond 200 km
        let g43 = uniform(5.0, 80.0, 4.0 / 3.0);
        for km in [300.0, 1000.0, 1500.0] {
            let p = g43.evaluate(tx(), 1.0, at_km(km), 0.0);
            let h = harris_ns(km * 1000.0);
            assert!(
                ((p.sf_ns - h) / h).abs() < 0.07,
                "k=4/3 {km} km: {} vs {h}",
                p.sf_ns
            );
        }
    }

    #[test]
    fn seawater_field_strength() {
        let g = uniform(5.0, 80.0, K_LORAN);
        // VALIDATED (definition of E1, ITU-R P.368 reference 300 V cymomotive force): at
        // short range over sea the 1 kW field is 300 mV/m·(1 km/d) within 0.5 dB
        for km in [5.0, 10.0, 30.0, 60.0] {
            let p = g.evaluate(tx(), 1.0, at_km(km), 0.0);
            let ratio_db = 20.0 * (p.field_uv_m / (300_000.0 / km)).log10();
            assert!(ratio_db.abs() < 0.5, "{km} km: {ratio_db} dB");
        }
        // MODELLED: attenuation below inverse distance becomes significant beyond ~500 km
        let att = |km: f64| {
            20.0 * (g.evaluate(tx(), 1.0, at_km(km), 0.0).field_uv_m / (300_000.0 / km)).log10()
        };
        assert!(att(300.0) > -3.0);
        assert!(att(500.0) < -2.0 && att(500.0) > -6.0, "{}", att(500.0));
        assert!(att(1000.0) < -6.0 && att(1000.0) > -15.0, "{}", att(1000.0));
        // monotone decrease of field and increase of SF with distance
        let mut last = (f64::INFINITY, -1.0);
        for km in (1..=60).map(|i| i as f64 * 50.0) {
            let p = g.evaluate(tx(), 400.0, at_km(km), 0.0);
            assert!(p.field_uv_m < last.0 && p.sf_ns > last.1, "{km}");
            last = (p.field_uv_m, p.sf_ns);
        }
        // power scaling: √P
        let a = g.evaluate(tx(), 1.0, at_km(400.0), 0.0).field_uv_m;
        let b = g.evaluate(tx(), 400.0, at_km(400.0), 0.0).field_uv_m;
        assert!((b / a - 20.0).abs() < 1e-9);
    }

    #[test]
    fn land_asf_and_attenuation() {
        // Poor land (σ = 0.001–0.003 S/m): ASF of several µs at long range; monotone in σ.
        let poor = uniform(0.001, 15.0, K_LORAN);
        let medium = uniform(0.003, 15.0, K_LORAN);
        let good = uniform(0.03, 30.0, K_LORAN);
        let sea = uniform(5.0, 80.0, K_LORAN);
        for km in [100.0, 300.0, 600.0, 1000.0] {
            let p = poor.evaluate(tx(), 1.0, at_km(km), 0.0);
            let m = medium.evaluate(tx(), 1.0, at_km(km), 0.0);
            let g = good.evaluate(tx(), 1.0, at_km(km), 0.0);
            let s = sea.evaluate(tx(), 1.0, at_km(km), 0.0);
            assert!(
                p.asf_ns > m.asf_ns && m.asf_ns > g.asf_ns && g.asf_ns > 0.0,
                "{km}"
            );
            // (good ground with an inductive impedance may slightly exceed sea far out)
            assert!(
                p.field_uv_m < m.field_uv_m
                    && m.field_uv_m < g.field_uv_m
                    && g.field_uv_m < s.field_uv_m * 1.05,
                "{km}: {} {} {} {}",
                p.field_uv_m,
                m.field_uv_m,
                g.field_uv_m,
                s.field_uv_m
            );
            // SF is the seawater SF whatever the ground (ASF carries the difference)
            assert!((p.sf_ns - s.sf_ns).abs() < 1e-9);
        }
        let p1000 = poor.evaluate(tx(), 1.0, at_km(1000.0), 0.0);
        assert!(
            p1000.asf_ns > 3000.0 && p1000.asf_ns < 15_000.0,
            "{}",
            p1000.asf_ns
        );
        // the Loran-C SF chart quoted for "terrain type 1" (200–310 statute miles): 3.2–4.3 µs
        // total; σ = 0.001 S/m gives the same order (MODELLED, terrain types not mapped to σ)
        let t = poor.evaluate(tx(), 1.0, at_km(400.0), 0.0);
        assert!(
            t.sf_ns + t.asf_ns > 2500.0 && t.sf_ns + t.asf_ns < 8000.0,
            "{}",
            t.sf_ns + t.asf_ns
        );
        // ASF grows monotonically with distance over homogeneous land
        let mut last = 0.0;
        for km in (1..=40).map(|i| i as f64 * 50.0) {
            let a = medium.evaluate(tx(), 1.0, at_km(km), 0.0).asf_ns;
            assert!(a > last, "{km}");
            last = a;
        }
    }

    #[test]
    fn ecd_from_dispersion() {
        let sea = uniform(5.0, 80.0, K_LORAN);
        let land = uniform(0.003, 15.0, K_LORAN);
        let e_sea = sea.evaluate(tx(), 1.0, at_km(1000.0), 0.0).ecd_s * 1e6;
        let e_land = land.evaluate(tx(), 1.0, at_km(1000.0), 0.0).ecd_s * 1e6;
        for km in [10.0, 100.0, 300.0, 1000.0, 2000.0] {
            for (n, g) in [("sea", &sea), ("land", &land)] {
                println!(
                    "ECD {n} {km} km: {:.3} µs",
                    g.evaluate(tx(), 1.0, at_km(km), 0.0).ecd_s * 1e6
                );
            }
        }
        // dispersion makes the envelope lead the carrier more and more with distance
        // (see the module documentation); magnitude tenths of a µs to ~2 µs over 1000 km,
        // larger over land than over sea (MODELLED against the published LORAN range)
        assert!(e_land < -0.1 && e_land > -3.0, "land ECD {e_land} µs");
        assert!(e_sea < 0.0 && e_sea > e_land, "sea ECD {e_sea} µs");
        let mut last = 0.0;
        for km in [150.0, 300.0, 600.0, 1000.0, 1500.0] {
            let e = land.evaluate(tx(), 1.0, at_km(km), 0.0).ecd_s;
            assert!(e < last, "{km}");
            last = e;
        }
    }

    #[test]
    fn table_matches_direct_evaluation() {
        for g in [
            SEAWATER,
            Ground {
                sigma: 0.002,
                eps_r: 15.0,
            },
        ] {
            let c = ground_curve(g, K_LORAN);
            let e = SmoothEarth::new(g, F0_HZ, K_LORAN, N_ATM);
            for km in [3.3, 47.0, 81.0, 333.3, 1234.5, 2500.0] {
                let w = e.w(km * 1000.0);
                let t = c.at(km * 1000.0);
                let mut ph = -w.arg() / (2.0 * PI * F0_HZ);
                let period = 1.0 / F0_HZ;
                while ph - t.phase_delay_s > period / 2.0 {
                    ph -= period;
                }
                while ph - t.phase_delay_s < -period / 2.0 {
                    ph += period;
                }
                assert!(
                    (ph - t.phase_delay_s).abs() < 0.05e-9,
                    "{g:?} {km}: {} vs {}",
                    ph,
                    t.phase_delay_s
                );
                assert!((w.abs().ln() - t.ln_amp).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn continuity_across_transition() {
        // no step in SF/ASF/field across the short/long-range blend (≈ 50–110 km)
        for g in [uniform(5.0, 80.0, K_LORAN), uniform(0.002, 15.0, K_LORAN)] {
            let mut prev: Option<PathEffect> = None;
            for i in 0..=400 {
                let km = 40.0 + i as f64 * 0.2;
                let p = g.evaluate(tx(), 1.0, at_km(km), 0.0);
                if let Some(q) = prev {
                    let dt = (p.sf_ns + p.asf_ns) - (q.sf_ns + q.asf_ns);
                    assert!(dt > -0.5 && dt < 3.0, "{km} km step {dt} ns");
                    let ddb = 20.0 * (p.field_uv_m / q.field_uv_m).log10();
                    assert!(ddb < 0.0 && ddb > -0.1, "{km} km field step {ddb} dB");
                }
                prev = Some(p);
            }
        }
    }

    fn coast_map() -> ConductivityMap {
        // land north of 51° N between 10° W and 10° E, sea elsewhere
        ConductivityMap::build(&ConductivityConfig::Polygons {
            background: GroundConst::seawater(),
            polygons: vec![super::super::conductivity::PolygonConfig {
                name: "land".into(),
                sigma_s_m: 0.003,
                eps_r: Some(15.0),
                vertices: vec![[51.0, -10.0], [60.0, -10.0], [60.0, 10.0], [51.0, 10.0]],
            }],
        })
        .unwrap()
    }

    #[test]
    fn millington_reciprocity_all_sea_and_recovery() {
        let gw = Groundwave::new(coast_map(), K_LORAN, 1.0, 0.005, None, "t").unwrap();
        let a = Geo::new(49.0, 0.0); // sea
        let b = Geo::new(53.0, 0.0); // land
        let ab = gw.correction_exact(a, b);
        let ba = gw.correction_exact(b, a);
        assert!(
            (ab.phase_s - ba.phase_s).abs() < 1e-12,
            "{} {}",
            ab.phase_s,
            ba.phase_s
        );
        assert!((ab.ln_amp - ba.ln_amp).abs() < 1e-9);
        assert!(ab.phase_s > 0.0 && ab.land > 0.4 && ab.land < 0.6);
        // all-sea path: ASF = 0
        let s = gw.correction_exact(Geo::new(45.0, 0.0), Geo::new(50.0, 2.0));
        assert!(s.phase_s.abs() < 1e-15 && s.land == 0.0);
        // mixed path ASF lies between all-sea (0) and all-land of the same length
        let land = uniform(0.003, 15.0, K_LORAN).correction_exact(a, b);
        assert!(ab.phase_s < land.phase_s);
        // field recovery after the coast (Millington): extending a land-launched path
        // further over sea raises the field relative to the all-sea field
        let t = Geo::new(55.0, 0.0);
        let c1 = gw.correction_exact(t, Geo::new(50.5, 0.0));
        let c2 = gw.correction_exact(t, Geo::new(48.0, 0.0));
        assert!(
            c2.ln_amp > c1.ln_amp - 1e-9,
            "no field recovery: {} {}",
            c1.ln_amp,
            c2.ln_amp
        );
    }

    #[test]
    fn grid_interpolation_is_continuous_and_close() {
        let gw = Groundwave::new(coast_map(), K_LORAN, 1.0, 0.005, None, "t").unwrap();
        let t = Geo::new(55.0, 2.0);
        let mut prev = None;
        for i in 0..200 {
            let rx = Geo::new(49.3 + i as f64 * 0.0003, 0.4);
            let c = gw.correction(t, rx);
            let e = gw.correction_exact(t, rx);
            assert!(
                (c.phase_s - e.phase_s).abs() < 2e-9,
                "interp error {} ns",
                (c.phase_s - e.phase_s) * 1e9
            );
            if let Some(p) = prev {
                let p: Correction = p;
                assert!((c.phase_s - p.phase_s).abs() < 0.5e-9);
            }
            prev = Some(c);
        }
    }

    #[test]
    fn temporal_terms() {
        let t = TemporalConfig {
            epoch_day_of_year: Some(1.0),
            epoch_hour_utc: Some(0.0),
            diurnal_amp_ns: 10.0,
            diurnal_peak_hour_utc: 6.0,
            seasonal_amp_ns: 30.0,
            seasonal_peak_day: 1.0,
            scale_by_land_fraction: true,
            sigma_seasonal_frac: 0.2,
            sigma_peak_day: 1.0,
        };
        assert!(
            (t.delta_asf_s(6.0 * 3600.0, 1.0) * 1e9
                - (10.0 + 30.0 * (2.0 * PI * 0.25 / YEAR_D).cos()))
            .abs()
                < 1e-9
        );
        assert_eq!(t.delta_asf_s(1234.0, 0.0), 0.0);
        assert!((t.sigma_scale() - 1.2).abs() < 1e-12);
        // deterministic and applied through the model
        let m = ConductivityMap::build(&ConductivityConfig::Uniform {
            sigma_s_m: 0.003,
            eps_r: Some(15.0),
        })
        .unwrap();
        let gw = Groundwave::new(m, K_LORAN, 2.0, 0.005, Some(t.clone()), "t").unwrap();
        let a = gw.evaluate(tx(), 1.0, at_km(300.0), 0.0);
        let b = gw.evaluate(tx(), 1.0, at_km(300.0), 6.0 * 3600.0);
        assert!(
            ((b.asf_ns - a.asf_ns) - 10.0).abs() < 0.1,
            "{}",
            b.asf_ns - a.asf_ns
        );
        // higher σ (season) → smaller ASF than the unscaled ground
        let base = uniform(0.003, 15.0, K_LORAN).evaluate(tx(), 1.0, at_km(300.0), 0.0);
        assert!(a.asf_ns - t.delta_asf_s(0.0, 1.0) * 1e9 < base.asf_ns);
    }

    #[test]
    fn config_toml_round_trip() {
        let c: GroundConfig = toml::from_str(
            r#"
model = "millington"
k_factor = 1.24
sample_km = 1.0
[conductivity]
type = "uniform"
sigma_s_m = 0.005
[temporal]
diurnal_amp_ns = 5.0
"#,
        )
        .unwrap();
        assert!(matches!(c, GroundConfig::Millington { .. }));
        build(&c, &ctx("2026-11-18T00:00:00Z")).unwrap();
        let s: GroundConfig = toml::from_str("model = \"smooth_earth\"").unwrap();
        assert_eq!(
            s,
            GroundConfig::SmoothEarth {
                sigma_s_m: 5.0,
                eps_r: None,
                k_factor: K_LORAN,
                temporal: None
            }
        );
    }

    fn ctx(utc: &str) -> BuildContext {
        BuildContext::new(AbsTime::from_utc_iso(utc).unwrap(), 1.0, 0)
    }

    #[test]
    fn temporal_epoch_comes_from_the_scene_start_unless_overridden() {
        let t: TemporalConfig =
            toml::from_str("diurnal_amp_ns = 10.0\ndiurnal_peak_hour_utc = 6.0").unwrap();
        assert_eq!(t.epoch_day_of_year, None);
        let r = t.resolved(AbsTime::from_utc_iso("2026-11-18T06:00:00Z").unwrap());
        assert_eq!(
            (r.epoch_day_of_year, r.epoch_hour_utc),
            (Some(322.0), Some(6.0))
        );
        // at the diurnal peak hour: ΔASF = +A on an all-land path
        assert!((r.delta_asf_s(0.0, 1.0) * 1e9 - 10.0).abs() < 1e-9);
        // through the model: a scene starting at 06:00 sees the peak, one at 18:00 the trough
        let cfg = |toml_t: &str| -> GroundConfig {
            toml::from_str(&format!(
                "model = \"smooth_earth\"\nsigma_s_m = 0.003\n[temporal]\n{toml_t}"
            ))
            .unwrap()
        };
        let c = cfg("diurnal_amp_ns = 10.0\ndiurnal_peak_hour_utc = 6.0");
        let st = Station {
            id: "M".into(),
            name: "M".into(),
            role: crate::signal::Role::Master,
            position: tx(),
            power_kw: 1.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        };
        let rx = RxState {
            t_s: 0.0,
            pos: at_km(300.0),
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        let a = build(&c, &ctx("2026-11-18T06:00:00Z"))
            .unwrap()
            .paths(&st, &rx)[0];
        let b = build(&c, &ctx("2026-11-18T18:00:00Z"))
            .unwrap()
            .paths(&st, &rx)[0];
        assert!(
            ((a.asf_ns - b.asf_ns) - 20.0).abs() < 1e-6,
            "{}",
            a.asf_ns - b.asf_ns
        );
        // an explicit epoch overrides the scene start (backward-compatible files)
        let o = cfg("diurnal_amp_ns = 10.0\ndiurnal_peak_hour_utc = 6.0\nepoch_day_of_year = 322.0\nepoch_hour_utc = 6.0");
        let c2 = build(&o, &ctx("2026-11-18T18:00:00Z"))
            .unwrap()
            .paths(&st, &rx)[0];
        assert_eq!(c2.asf_ns, a.asf_ns);
    }
}
