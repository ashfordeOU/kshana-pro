// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Skywave propagation (stream S6).
//!
//! Two models behind [`SkyConfig`]:
//!
//! * `fixed`: one skywave a fixed delay and ratio after a simple groundwave estimate
//!   (kept for tests and for scenes that want an exact, geometry-free skywave);
//! * `ionospheric`: one-hop (and optionally two-hop) reflection from an ionospheric layer
//!   on a spherical earth, with an ITU-R P.684-style LF field-strength estimate, a
//!   day/night/sunrise transition from the solar zenith angle at the path midpoint, the
//!   ionospheric reflection phase, a propagation-induced skywave ECD and slow
//!   Rayleigh/Rice fading (see [`super::sky_fading`]).
//!
//! # Geometry (MODELLED, standard)
//!
//! Earth: sphere of radius [`R_EARTH_M`]. A hop of ground central angle `θ` reflects at
//! virtual height `h` above the hop midpoint. Each half-hop is the chord
//!
//! ```text
//! s = √(R² + (R+h)² − 2 R (R+h) cos(θ/2)),   L_n = 2 n s(θ = d/(nR), h)
//! ```
//!
//! and the elevation angle at the ground is `ψ = atan2((R+h)cos(θ/2) − R, (R+h) sin(θ/2))`,
//! the angle of incidence at the layer `i` with `sin i = R cos ψ / (R+h)`. The skywave
//! **envelope (group) delay after the groundwave** is `(L_n − d)/c` with `d` the geodesic
//! ground distance. The ground path's Secondary/Additional Secondary Factors are ignored
//! in this difference (they are a few µs at most against 30–200 µs): the computed skywave
//! delay is absolute, `d·n/c` (the PF-only ground delay) plus `(L_n − d)/c`, whatever the
//! ground model. Over 250–2,000 km and `h` = 60–100 km the excess delay spans about
//! 36–240 µs (see the tests).
//!
//! # Stated values are relative to the actual groundwave
//!
//! `delay_us` (one-hop envelope delay after the groundwave) and `ratio_db` (one-hop
//! sky/ground ratio), and both values of the `fixed` model, are relative to the
//! groundwave **actually computed by the scene's ground model**: the composite model
//! hands the ground paths to [`PropagationModel::paths_with_ground`], so a stated +6 dB at
//! 40 µs is +6 dB at 40 µs in the truth record over `smooth_earth` or `millington` ground
//! as over `simple`. Only when the model is called on its own ([`PropagationModel::paths`],
//! no ground paths) is a simple groundwave estimate ([`super::ground::Simple`], with
//! `ground_ref_atten_db_per_1000km`) the reference.
//!
//! Group vs phase height: the carrier is reflected slightly below the virtual (group)
//! height (`phase_height_offset_km`, MODELLED default 1 km). The path's `delay` is the
//! **phase** delay `L(h − Δh)/c` and its `ecd_s` is `(L(h) − L(h − Δh))/c`, so the envelope
//! arrives at the group delay and the carrier at the phase delay: the propagation-induced
//! ECD of the skywave (positive: envelope late against the carrier, as for any dispersive
//! plasma where group velocity < phase velocity). Typical values 0.5–5 µs.
//!
//! # Field strength (MODELLED, after ITU-R P.684-6 §2 "wave-hop" method)
//!
//! ```text
//! E_n = 2 · E0·√P · ‖R‖ⁿ · Rgⁿ⁻¹ · D · Ft · cos ψ / L_n      (µV/m, L in km)
//! ```
//!
//! * `E0 = 300 mV/m` at 1 km for 1 kW (short monopole over perfect ground), `P` in kW;
//!   the factor 2 is the ground reflection at the receiving end; `cos ψ` the short-monopole
//!   pattern;
//! * `‖R‖` effective ionospheric reflection coefficient. P.684 gives it as curves against
//!   `f·cos i` for day and night; here an **approximate fit** `‖R‖ = R0 · exp(−k cos i)`
//!   is used with defaults chosen to reproduce the usual 100 kHz behaviour (night ‖R‖ ≈
//!   0.5 at grazing, 0.13 at vertical incidence; day roughly 10 dB lower). Both fits are
//!   configurable;
//! * `Rg` ground reflection coefficient between hops (two-hop only, default 0.9);
//! * `D` focusing factor of the curved ionosphere and earth, computed from the ray-tube
//!   geometry `D² = L² cos ψ / (R² sin θ |dθ/dψ| sin ψ)` (= 1 on a flat earth), capped at
//!   [`D_MAX`] near grazing;
//! * `Ft` terminal factor for finite ground conductivity, `terminal_loss_db` (default 0 dB,
//!   i.e. sea paths; P.684 curves give a few dB of loss at low elevation over land).
//!
//! Limits: one hop to ≈ 2,000 km (ψ → 0 beyond; ψ is clamped to [`PSI_MIN_DEG`]), two hops
//! to ≈ 4,000 km; no polar-cap, sporadic-E, geomagnetic or sunspot-cycle dependence; the
//! ‖R‖ fit is not read from the P.684 curves and is **not validated**. The `ratio_db`
//! override replaces this estimate with a fixed sky/ground ratio (requirement range
//! −30 to +20 dB is covered; −60 to +40 dB is accepted).
//!
//! # Day, night and the sunrise/sunset transition (MODELLED)
//!
//! The solar zenith angle `χ` at the path midpoint is computed at scene time from the
//! scene start (the [`super::BuildContext`] given to [`build`], i.e. `[scene] start_utc`;
//! an explicit `start_utc` in the sky table overrides it) (low-precision solar ephemeris of the Astronomical Almanac, ≈ 0.01°;
//! TAI−UTC is ignored, < 0.2°). The day weight `w` rises smoothly (smoothstep) from 0 at
//! `χ ≥ 99°` (sun 9° below the horizon: D region gone) to 1 at `χ ≤ 85°`. `ln ‖R‖`, the
//! virtual height and the reflection phase are interpolated between their night and day
//! values with `w`. `daylight = "day"` / `"night"` forces the state.
//!
//! References: ITU-R P.684-6 (2012) "Prediction of field strength at frequencies below
//! about 150 kHz"; K. Davies, *Ionospheric Radio* (1990) ch. 6 (group/phase height);
//! J.R. Wait, *Electromagnetic Waves in Stratified Media* (focusing).

use super::sky_fading::{FadingConfig, FadingProcess};
use super::{BuildContext, PathEffect, PathKind, PropagationModel, C};
use crate::chain::Station;
use crate::geo::{direct, inverse, Geo};
use crate::time::{AbsTime, Ps, PS_PER_S};
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Mean earth radius used for skywave geometry (m).
pub const R_EARTH_M: f64 = 6_371_000.0;
/// Cap on the focusing factor near grazing incidence.
pub const D_MAX: f64 = 3.0;
/// Lowest elevation angle used in the field formula (deg).
pub const PSI_MIN_DEG: f64 = 0.5;
/// Free-space field at 1 km for 1 kW (µV/m), ITU-R P.684 `E0`.
pub const E0_UV_M: f64 = 300_000.0;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "snake_case")]
pub enum SkyConfig {
    /// One skywave arriving `delay_us` after the groundwave with amplitude ratio
    /// `ratio_db` (sky/ground), both relative to the scene's actual groundwave (a simple
    /// groundwave estimate when the model is used without ground paths).
    Fixed {
        delay_us: f64,
        ratio_db: f64,
        #[serde(default)]
        phase_deg: f64,
    },
    /// Geometric ionospheric skywave with P.684-style field strength, day/night,
    /// reflection phase, skywave ECD and fading.
    Ionospheric(IonoConfig),
}

/// Forced or automatic illumination state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Daylight {
    /// From the solar zenith angle at the path midpoint (needs the scene start, or
    /// `start_utc`).
    #[default]
    Auto,
    Day,
    Night,
}

/// Fit `‖R‖ = r0 · exp(−k cos i)` of the effective reflection coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReflFit {
    pub r0: f64,
    pub k: f64,
}

impl ReflFit {
    pub fn at(&self, cos_i: f64) -> f64 {
        (self.r0 * (-self.k * cos_i).exp()).clamp(1e-6, 1.0)
    }
}

/// Configuration of the ionospheric skywave model (all fields have defaults).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IonoConfig {
    /// Virtual reflection height by day / night (km). D region by day, E region by night.
    #[serde(default = "d_h_day")]
    pub h_day_km: f64,
    #[serde(default = "d_h_night")]
    pub h_night_km: f64,
    /// Number of hops modelled: 1, or 2 (adds a second path).
    #[serde(default = "d_hops")]
    pub hops: u8,
    /// Optional override of the UTC instant of scene time 0 for the sun's position. By
    /// default the scene start is used ([`super::BuildContext`]); set it only to evaluate
    /// the ionosphere at a different epoch than the scene's.
    #[serde(default)]
    pub start_utc: Option<String>,
    #[serde(default)]
    pub daylight: Daylight,
    /// Override of the one-hop envelope delay after the groundwave (µs).
    #[serde(default)]
    pub delay_us: Option<f64>,
    /// Override of the one-hop mean sky/ground ratio (dB) against the scene's actual
    /// groundwave (replaces the P.684 field and the day/night change of field).
    #[serde(default)]
    pub ratio_db: Option<f64>,
    /// Attenuation of the simple groundwave estimate used as the reference when the model
    /// is evaluated without the scene's ground paths (dB/1000 km).
    #[serde(default = "d_att")]
    pub ground_ref_atten_db_per_1000km: f64,
    #[serde(default = "d_r_day")]
    pub refl_day: ReflFit,
    #[serde(default = "d_r_night")]
    pub refl_night: ReflFit,
    /// Ground reflection coefficient between hops.
    #[serde(default = "d_rg")]
    pub ground_reflection: f64,
    /// Terminal (antenna/ground) loss `Ft` (dB, ≥ 0).
    #[serde(default)]
    pub terminal_loss_db: f64,
    /// Reflection phase by day / night (deg), per hop.
    #[serde(default = "d_ph_day")]
    pub refl_phase_day_deg: f64,
    #[serde(default = "d_ph_night")]
    pub refl_phase_night_deg: f64,
    /// Virtual (group) height minus carrier-phase reflection height (km); sets the ECD.
    #[serde(default = "d_dh")]
    pub phase_height_offset_km: f64,
    /// Slow amplitude/phase fading; none if absent.
    #[serde(default)]
    pub fading: Option<FadingConfig>,
    /// Ionosonde and space-weather data modifying height, ‖R‖ and fading
    /// ([`super::iono_data`], MODELLED heuristic); none if absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<super::iono_data::IonoDataConfig>,
}

fn d_h_day() -> f64 {
    70.0
}
fn d_h_night() -> f64 {
    90.0
}
fn d_hops() -> u8 {
    1
}
fn d_att() -> f64 {
    3.0
}
fn d_r_day() -> ReflFit {
    ReflFit { r0: 0.15, k: 2.5 }
}
fn d_r_night() -> ReflFit {
    ReflFit { r0: 0.6, k: 1.5 }
}
fn d_rg() -> f64 {
    0.9
}
fn d_ph_day() -> f64 {
    160.0
}
fn d_ph_night() -> f64 {
    180.0
}
fn d_dh() -> f64 {
    1.0
}

impl Default for IonoConfig {
    fn default() -> Self {
        IonoConfig {
            h_day_km: d_h_day(),
            h_night_km: d_h_night(),
            hops: 1,
            start_utc: None,
            daylight: Daylight::Night,
            delay_us: None,
            ratio_db: None,
            ground_ref_atten_db_per_1000km: d_att(),
            refl_day: d_r_day(),
            refl_night: d_r_night(),
            ground_reflection: d_rg(),
            terminal_loss_db: 0.0,
            refl_phase_day_deg: d_ph_day(),
            refl_phase_night_deg: d_ph_night(),
            phase_height_offset_km: d_dh(),
            fading: None,
            data: None,
        }
    }
}

pub struct Fixed {
    pub delay_us: f64,
    pub ratio_db: f64,
    pub phase_deg: f64,
}

impl PropagationModel for Fixed {
    fn name(&self) -> String {
        "skywave-fixed".into()
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        self.paths_with_ground(st, rx, &[])
    }
    fn paths_with_ground(
        &self,
        st: &Station,
        rx: &RxState,
        ground: &[PathEffect],
    ) -> Vec<PathEffect> {
        let g = reference_ground(st, rx, ground, 3.0);
        vec![PathEffect {
            kind: PathKind::Sky { hop: 1 },
            delay: g.delay + Ps::from_us(self.delay_us),
            field_uv_m: g.field_uv_m * 10f64.powf(self.ratio_db / 20.0),
            ecd_s: 0.0,
            phase_rad: self.phase_deg.to_radians(),
            geom_ns: g.geom_ns,
            pf_ns: 0.0,
            sf_ns: 0.0,
            asf_ns: 0.0,
        }]
    }
}

/// The groundwave a stated skywave delay/ratio refers to: the first ground path of
/// `ground` (the scene's ground model), else a simple estimate with the given attenuation.
pub fn reference_ground(
    st: &Station,
    rx: &RxState,
    ground: &[PathEffect],
    atten_db_per_1000km: f64,
) -> PathEffect {
    ground
        .iter()
        .find(|p| p.kind == PathKind::Ground)
        .copied()
        .unwrap_or_else(|| {
            super::ground::Simple {
                atten_db_per_1000km,
            }
            .paths(st, rx)[0]
        })
}

/// Geometry of an `n`-hop skywave over ground distance `d_m` at virtual height `h_m`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HopGeometry {
    /// Total slant path length (m).
    pub length_m: f64,
    /// Elevation angle at the ground (rad); negative beyond the hop's horizon range.
    pub psi_rad: f64,
    /// Cosine of the angle of incidence on the layer.
    pub cos_i: f64,
    /// Focusing factor (capped).
    pub focusing: f64,
}

/// Skywave path geometry on the sphere (see the module doc).
pub fn hop_geometry(d_m: f64, h_m: f64, hops: u8) -> HopGeometry {
    let n = hops.max(1) as f64;
    let r = R_EARTH_M;
    let rh = r + h_m;
    let theta_hop = d_m / (n * r);
    let beta = theta_hop / 2.0;
    let s = (r * r + rh * rh - 2.0 * r * rh * beta.cos())
        .max(0.0)
        .sqrt();
    let length_m = 2.0 * n * s;
    let psi = (rh * beta.cos() - r).atan2(rh * beta.sin());
    let psi_c = psi.max(PSI_MIN_DEG.to_radians());
    let sin_i = (r * psi_c.cos() / rh).min(1.0);
    let cos_i = (1.0 - sin_i * sin_i).max(0.0).sqrt();
    // focusing: D² = L² cos ψ / (R² sin θ_tot · n |dθ_hop/dψ| · sin ψ)
    let dtheta_dpsi = 2.0 * (1.0 - r * psi_c.sin() / (rh * cos_i.max(1e-12))).abs();
    let theta_tot = d_m / r;
    let denom = r * r * theta_tot.sin().max(1e-12) * n * dtheta_dpsi * psi_c.sin();
    let d2 = length_m * length_m * psi_c.cos() / denom.max(1e-30);
    let focusing = if d2.is_finite() {
        d2.sqrt().clamp(0.1, D_MAX)
    } else {
        D_MAX
    };
    HopGeometry {
        length_m,
        psi_rad: psi,
        cos_i,
        focusing,
    }
}

/// Envelope (group) delay of the `n`-hop skywave after the groundwave (s): `(L − d)/c`.
pub fn sky_excess_delay_s(d_m: f64, h_m: f64, hops: u8) -> f64 {
    (hop_geometry(d_m, h_m, hops).length_m - d_m) / C
}

/// Virtual height (m) that gives a one-hop excess delay of `delay_s` over `d_m`
/// (bisection over 1–1,000 km); `None` if outside that range.
pub fn height_for_delay(d_m: f64, delay_s: f64) -> Option<f64> {
    let (mut lo, mut hi) = (1_000.0, 1_000_000.0);
    if sky_excess_delay_s(d_m, lo, 1) > delay_s || sky_excess_delay_s(d_m, hi, 1) < delay_s {
        return None;
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if sky_excess_delay_s(d_m, mid, 1) < delay_s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(0.5 * (lo + hi))
}

/// Julian Date (UTC) of a scene instant, `t_s` seconds after `start`.
/// TAI−UTC is ignored (≤ 37 s; documented).
pub fn julian_date(start: AbsTime, t_s: f64) -> f64 {
    // 1958-01-01T00:00:00 = JD 2436204.5
    let s = start.0 as f64 / PS_PER_S as f64 + t_s;
    2_436_204.5 + s / 86_400.0
}

/// Solar zenith angle (deg) at `pos` at Julian Date `jd` (low-precision almanac formulae,
/// ≈ 0.01° 1950–2050).
pub fn solar_zenith_deg(jd: f64, pos: Geo) -> f64 {
    let n = jd - 2_451_545.0;
    let l = (280.460 + 0.985_647_4 * n).to_radians();
    let g = (357.528 + 0.985_600_3 * n).to_radians();
    let lambda = l + (1.915f64.to_radians()) * g.sin() + (0.020f64.to_radians()) * (2.0 * g).sin();
    let eps = (23.439 - 0.000_000_4 * n).to_radians();
    let ra = (eps.cos() * lambda.sin()).atan2(lambda.cos());
    let dec = (eps.sin() * lambda.sin()).asin();
    let gmst_h = 18.697_374_558 + 24.065_709_824_419_08 * n;
    let lha = (gmst_h * 15.0).to_radians() + pos.lon_deg.to_radians() - ra;
    let phi = pos.lat_deg.to_radians();
    let cz = phi.sin() * dec.sin() + phi.cos() * dec.cos() * lha.cos();
    cz.clamp(-1.0, 1.0).acos().to_degrees()
}

/// Day weight from the solar zenith angle: 0 at χ ≥ 99°, 1 at χ ≤ 85°, smoothstep between.
pub fn day_weight(chi_deg: f64) -> f64 {
    let u = ((99.0 - chi_deg) / (99.0 - 85.0)).clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

/// The ionospheric skywave model.
pub struct Ionospheric {
    pub cfg: IonoConfig,
    start: Option<AbsTime>,
    fading: Option<FadingProcess>,
    data: Option<super::iono_data::IonoData>,
}

impl Ionospheric {
    /// Model without a scene context: `daylight = "auto"` needs `start_utc` in `cfg`.
    pub fn new(cfg: IonoConfig) -> Result<Ionospheric, String> {
        Ionospheric::with_start(cfg, None)
    }

    /// Model in a scene starting at `scene_start` (used for the sun's position unless
    /// `cfg.start_utc` overrides it).
    pub fn with_start(
        cfg: IonoConfig,
        scene_start: Option<AbsTime>,
    ) -> Result<Ionospheric, String> {
        if !(cfg.hops == 1 || cfg.hops == 2) {
            return Err("skywave: hops must be 1 or 2".into());
        }
        for (n, h) in [("h_day_km", cfg.h_day_km), ("h_night_km", cfg.h_night_km)] {
            if !(40.0..=400.0).contains(&h) {
                return Err(format!("skywave: {n} = {h} outside 40–400 km"));
            }
        }
        if let Some(d) = cfg.delay_us {
            if !(0.0..=2000.0).contains(&d) {
                return Err(format!("skywave: delay_us {d} outside 0–2000 µs"));
            }
        }
        if let Some(r) = cfg.ratio_db {
            if !(-60.0..=40.0).contains(&r) {
                return Err(format!("skywave: ratio_db {r} outside −60…+40 dB"));
            }
        }
        if !(0.0..=1.0).contains(&cfg.ground_reflection) || cfg.terminal_loss_db < 0.0 {
            return Err("skywave: ground_reflection in 0..1 and terminal_loss_db ≥ 0".into());
        }
        if !(0.0..20.0).contains(&cfg.phase_height_offset_km) {
            return Err("skywave: phase_height_offset_km in 0..20".into());
        }
        let start = match &cfg.start_utc {
            Some(s) => Some(AbsTime::from_utc_iso(s)?),
            None => scene_start,
        };
        if cfg.daylight == Daylight::Auto && start.is_none() {
            return Err(
                "skywave: daylight = \"auto\" needs the scene start (build the model with a \
                        BuildContext) or start_utc, or set daylight = \"day\" / \"night\""
                    .into(),
            );
        }
        let fading = match &cfg.fading {
            Some(f) => Some(FadingProcess::new(f)?),
            None => None,
        };
        let data = match &cfg.data {
            Some(d) => {
                if start.is_none() {
                    return Err("skywave: data needs the scene start (BuildContext) or start_utc".into());
                }
                Some(super::iono_data::IonoData::build(d)?)
            }
            None => None,
        };
        Ok(Ionospheric {
            cfg,
            start,
            fading,
            data,
        })
    }

    /// Day weight (0 night, 1 day) at the midpoint between `a` and `b` at scene time `t_s`.
    pub fn day_weight_at(&self, a: Geo, b: Geo, t_s: f64) -> f64 {
        match self.cfg.daylight {
            Daylight::Day => 1.0,
            Daylight::Night => 0.0,
            Daylight::Auto => {
                let inv = inverse(a, b);
                let mid = direct(a, inv.az1_rad, inv.distance_m / 2.0);
                let start = self.start.expect("checked in new");
                day_weight(solar_zenith_deg(julian_date(start, t_s), mid))
            }
        }
    }

    /// Mean (unfaded) P.684-style field of the `n`-hop skywave (µV/m).
    pub fn p684_field(&self, power_kw: f64, d_m: f64, h_m: f64, hops: u8, w_day: f64) -> f64 {
        let g = hop_geometry(d_m, h_m, hops);
        let r_n = self.cfg.refl_night.at(g.cos_i).ln();
        let r_d = self.cfg.refl_day.at(g.cos_i).ln();
        let refl = (r_n + w_day * (r_d - r_n)).exp();
        let n = hops as i32;
        let psi = g.psi_rad.max(PSI_MIN_DEG.to_radians());
        let ft = 10f64.powf(-self.cfg.terminal_loss_db / 20.0);
        2.0 * E0_UV_M
            * power_kw.sqrt()
            * refl.powi(n)
            * self.cfg.ground_reflection.powi(n - 1)
            * g.focusing
            * ft
            * psi.cos()
            / (g.length_m / 1000.0)
    }
}

/// Stable identity of a station for fading seeds (FNV-1a over id and position).
pub fn station_key(st: &Station) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for x in b {
            h ^= *x as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(st.id.as_bytes());
    eat(&st.position.lat_deg.to_le_bytes());
    eat(&st.position.lon_deg.to_le_bytes());
    h
}

impl PropagationModel for Ionospheric {
    fn name(&self) -> String {
        format!("skywave-ionospheric ({} hop)", self.cfg.hops)
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        self.paths_with_ground(st, rx, &[])
    }
    /// Computed delays and P.684 fields are absolute (referenced to the PF-only ground
    /// delay); a stated `delay_us` / `ratio_db` refers to the first path of `ground`.
    fn paths_with_ground(
        &self,
        st: &Station,
        rx: &RxState,
        ground: &[PathEffect],
    ) -> Vec<PathEffect> {
        let simple = super::ground::Simple {
            atten_db_per_1000km: self.cfg.ground_ref_atten_db_per_1000km,
        }
        .paths(st, rx)[0];
        let g = reference_ground(st, rx, ground, self.cfg.ground_ref_atten_db_per_1000km);
        let d = inverse(st.position, rx.pos).distance_m.max(1.0);
        let w = self.day_weight_at(st.position, rx.pos, rx.t_s);
        let mut h = 1000.0 * (self.cfg.h_night_km + w * (self.cfg.h_day_km - self.cfg.h_night_km));
        // ionosonde / space-weather data (none: unchanged)
        let mods = match (&self.data, self.start) {
            (Some(data), Some(start)) => {
                let inv = inverse(st.position, rx.pos);
                let mid = direct(st.position, inv.az1_rad, inv.distance_m / 2.0);
                let t = super::iono_data::abs_seconds(start) + rx.t_s;
                let m = data.modifiers(t, w, mid);
                h = m.height(h);
                Some(m)
            }
            _ => None,
        };
        let dh = 1000.0 * self.cfg.phase_height_offset_km;
        let phase_per_hop = (self.cfg.refl_phase_night_deg
            + w * (self.cfg.refl_phase_day_deg - self.cfg.refl_phase_night_deg))
            .to_radians();
        let key = station_key(st);
        let mut out = Vec::with_capacity(self.cfg.hops as usize);
        for hop in 1..=self.cfg.hops {
            let group = sky_excess_delay_s(d, h, hop);
            let ecd = (group - sky_excess_delay_s(d, (h - dh).max(1000.0), hop)).max(0.0);
            // a stated delay is after the actual groundwave, a computed one after the
            // PF-only ground delay (absolute)
            let (base, group) = match (hop, self.cfg.delay_us) {
                (1, Some(us)) => (g, us * 1e-6),
                _ => (simple, group),
            };
            let mut field = match (hop, self.cfg.ratio_db) {
                (1, Some(r)) => g.field_uv_m * 10f64.powf(r / 20.0),
                _ => {
                    self.p684_field(st.power_kw, d, h, hop, w)
                        * mods.map(|m| m.refl_scale.powi(hop as i32)).unwrap_or(1.0)
                }
            };
            let mut phase = phase_per_hop * hop as f64;
            if let Some(f) = &self.fading {
                let (a, p) = f.gain(
                    key.wrapping_add((hop as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)),
                    rx.t_s,
                );
                let depth = mods.map(|m| m.fading_depth).unwrap_or(1.0);
                if depth == 1.0 {
                    field *= a;
                    phase += p;
                } else {
                    field *= a.max(1e-300).powf(depth);
                    phase += p * depth;
                }
            }
            // carrier delay = group delay − ECD
            let excess = group - ecd;
            out.push(PathEffect {
                kind: PathKind::Sky { hop },
                delay: base.delay + Ps::from_s(excess),
                field_uv_m: field,
                ecd_s: ecd,
                phase_rad: phase.rem_euclid(2.0 * PI),
                geom_ns: base.geom_ns + excess * 1e9,
                pf_ns: 0.0,
                sf_ns: 0.0,
                asf_ns: 0.0,
            });
        }
        out
    }
}

/// Build a skywave model in the scene context `ctx` (its start drives the day/night state
/// unless `start_utc` overrides it).
pub fn build(cfg: &SkyConfig, ctx: &BuildContext) -> Result<Box<dyn PropagationModel>, String> {
    match cfg {
        SkyConfig::Fixed {
            delay_us,
            ratio_db,
            phase_deg,
        } => Ok(Box::new(Fixed {
            delay_us: *delay_us,
            ratio_db: *ratio_db,
            phase_deg: *phase_deg,
        })),
        SkyConfig::Ionospheric(c) => Ok(Box::new(Ionospheric::with_start(
            c.clone(),
            Some(ctx.start),
        )?)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::StationStatus;
    use crate::signal::Role;

    fn station(lat: f64, lon: f64) -> Station {
        Station {
            id: "M".into(),
            name: "M".into(),
            role: Role::Master,
            position: Geo::new(lat, lon),
            power_kw: 400.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: StationStatus::On,
            faults: vec![],
        }
    }
    fn rx(lat: f64, lon: f64, t: f64) -> RxState {
        RxState {
            t_s: t,
            pos: Geo::new(lat, lon),
            heading_rad: 0.0,
            speed_mps: 0.0,
        }
    }

    #[test]
    fn flat_limit_geometry() {
        // short path: L ≈ 2√((d/2)² + h²)
        let g = hop_geometry(100_000.0, 70_000.0, 1);
        let flat = 2.0 * (50_000f64.powi(2) + 70_000f64.powi(2)).sqrt();
        assert!(
            (g.length_m - flat).abs() / flat < 2e-3,
            "{} {}",
            g.length_m,
            flat
        );
        assert!((g.focusing - 1.0).abs() < 0.05, "D {}", g.focusing);
        assert!(g.psi_rad > 0.9);
    }

    #[test]
    fn delay_range_30_to_200_us_is_covered() {
        // geometric delays over typical eLoran distances and D/E-region heights
        let mut lo = f64::INFINITY;
        let mut hi = 0.0f64;
        for d_km in [250.0, 400.0, 600.0, 800.0, 1000.0, 1500.0, 2000.0] {
            for h_km in [60.0, 70.0, 80.0, 90.0, 100.0] {
                let t = sky_excess_delay_s(d_km * 1e3, h_km * 1e3, 1) * 1e6;
                lo = lo.min(t);
                hi = hi.max(t);
            }
        }
        // geometry alone spans ≈ 36–240 µs (the shortest delays need long, low hops); the
        // height solver and the `delay_us` override reach any value in 30–200 µs
        assert!(lo < 40.0 && hi > 200.0, "range {lo:.1}..{hi:.1} µs");
        // typical: 1000 km night ≈ 70–80 µs, day ≈ 50 µs
        let n = sky_excess_delay_s(1.0e6, 90e3, 1) * 1e6;
        let dd = sky_excess_delay_s(1.0e6, 70e3, 1) * 1e6;
        assert!(
            (60.0..95.0).contains(&n) && (40.0..60.0).contains(&dd),
            "{n} {dd}"
        );
        // every value 30..200 µs is reachable at 500 km by choosing the height, or by override
        for target in [30.0, 50.0, 100.0, 150.0, 200.0] {
            let h = height_for_delay(500e3, target * 1e-6).unwrap();
            assert!((sky_excess_delay_s(500e3, h, 1) * 1e6 - target).abs() < 1e-6);
        }
        // two hops are later
        assert!(sky_excess_delay_s(1.5e6, 90e3, 2) > sky_excess_delay_s(1.5e6, 90e3, 1));
    }

    #[test]
    fn delay_override_and_ecd() {
        let st = station(55.0, 0.0);
        let r = rx(55.0, 12.0, 0.0); // ≈ 765 km
        for target in [30.0, 75.0, 200.0] {
            let m = Ionospheric::new(IonoConfig {
                delay_us: Some(target),
                ..Default::default()
            })
            .unwrap();
            let p = m.paths(&st, &r);
            let g = crate::prop::ground::Simple {
                atten_db_per_1000km: 3.0,
            }
            .paths(&st, &r)[0];
            let env = (p[0].delay - g.delay).as_us() + p[0].ecd_s * 1e6;
            assert!((env - target).abs() < 1e-5, "{env}");
            assert!(
                p[0].ecd_s > 0.2e-6 && p[0].ecd_s < 5e-6,
                "ecd {}",
                p[0].ecd_s
            );
        }
    }

    #[test]
    fn solar_zenith_sane() {
        // equinox 2026-03-20 12:00 UTC at (0,0): sun nearly overhead; midnight: below
        let noon = AbsTime::from_utc_iso("2026-03-20T12:00:00Z").unwrap();
        let z = solar_zenith_deg(julian_date(noon, 0.0), Geo::new(0.0, 0.0));
        assert!(z < 3.0, "{z}");
        let z2 = solar_zenith_deg(julian_date(noon, 12.0 * 3600.0), Geo::new(0.0, 0.0));
        assert!(z2 > 175.0, "{z2}");
        // June solstice noon at 50°N, lon 0: χ ≈ 50 − 23.44 = 26.6°
        let s = AbsTime::from_utc_iso("2026-06-21T12:00:00Z").unwrap();
        let z3 = solar_zenith_deg(julian_date(s, 0.0), Geo::new(50.0, 0.0));
        assert!((z3 - 26.6).abs() < 0.6, "{z3}");
    }

    #[test]
    fn day_night_ratio_sensible() {
        let st = station(55.0, 0.0);
        let r = rx(55.0, 15.0, 0.0); // ≈ 955 km
        let g = crate::prop::ground::Simple {
            atten_db_per_1000km: 3.0,
        }
        .paths(&st, &r)[0];
        let mk = |utc: &str| {
            Ionospheric::new(IonoConfig {
                start_utc: Some(utc.into()),
                daylight: Daylight::Auto,
                ..Default::default()
            })
            .unwrap()
            .paths(&st, &r)[0]
        };
        let night = mk("2026-12-21T00:00:00Z");
        let day = mk("2026-06-21T11:00:00Z");
        let rn = 20.0 * (night.field_uv_m / g.field_uv_m).log10();
        let rd = 20.0 * (day.field_uv_m / g.field_uv_m).log10();
        // night skywave comparable to the groundwave; day 10–25 dB weaker than night
        assert!((-8.0..8.0).contains(&rn), "night ratio {rn:.1} dB");
        assert!(
            rn - rd > 10.0 && rn - rd < 25.0,
            "night {rn:.1} day {rd:.1}"
        );
        // night delay longer (higher layer)
        assert!(night.delay > day.delay);
    }

    #[test]
    fn sunrise_transition_is_monotonic() {
        // 2026-03-20, path midpoint near (55N, 7.5E): sunrise ≈ 05:30 UTC
        let st = station(55.0, 0.0);
        let r = rx(55.0, 15.0, 0.0);
        let m = Ionospheric::new(IonoConfig {
            start_utc: Some("2026-03-20T03:00:00Z".into()),
            daylight: Daylight::Auto,
            ..Default::default()
        })
        .unwrap();
        let mut prev = f64::INFINITY;
        let mut ws = vec![];
        for k in 0..=24 {
            let t = k as f64 * 600.0; // 03:00 .. 07:00
            let p = m.paths(&st, &RxState { t_s: t, ..r })[0];
            assert!(p.field_uv_m <= prev * 1.0001);
            prev = p.field_uv_m;
            ws.push(m.day_weight_at(st.position, r.pos, t));
        }
        assert_eq!(ws[0], 0.0);
        assert_eq!(*ws.last().unwrap(), 1.0);
    }

    #[test]
    fn auto_without_start_is_an_error() {
        let c = IonoConfig {
            daylight: Daylight::Auto,
            start_utc: None,
            ..Default::default()
        };
        assert!(Ionospheric::new(c).is_err());
        assert!(Ionospheric::new(IonoConfig {
            hops: 3,
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn auto_uses_the_scene_start_and_start_utc_overrides_it() {
        let st = station(55.0, 0.0);
        let r = rx(55.0, 15.0, 0.0);
        let ctx = |utc: &str| BuildContext::new(AbsTime::from_utc_iso(utc).unwrap(), 1.0, 0);
        let auto = IonoConfig {
            daylight: Daylight::Auto,
            ..Default::default()
        };
        let night = build(
            &SkyConfig::Ionospheric(auto.clone()),
            &ctx("2026-12-21T00:00:00Z"),
        )
        .unwrap()
        .paths(&st, &r)[0];
        let day = build(
            &SkyConfig::Ionospheric(auto.clone()),
            &ctx("2026-06-21T11:00:00Z"),
        )
        .unwrap()
        .paths(&st, &r)[0];
        let explicit = Ionospheric::new(IonoConfig {
            start_utc: Some("2026-12-21T00:00:00Z".into()),
            ..auto.clone()
        })
        .unwrap()
        .paths(&st, &r)[0];
        assert_eq!(night, explicit);
        assert!(night.field_uv_m > 3.0 * day.field_uv_m);
        // explicit start_utc wins over the scene start
        let o = IonoConfig {
            start_utc: Some("2026-12-21T00:00:00Z".into()),
            ..auto
        };
        let p = build(&SkyConfig::Ionospheric(o), &ctx("2026-06-21T11:00:00Z"))
            .unwrap()
            .paths(&st, &r)[0];
        assert_eq!(p, night);
    }

    #[test]
    fn ratio_override_and_two_hops() {
        let st = station(55.0, 0.0);
        let r = rx(55.0, 25.0, 0.0); // ≈ 1590 km
        let g = crate::prop::ground::Simple {
            atten_db_per_1000km: 3.0,
        }
        .paths(&st, &r)[0];
        for db in [-30.0, 0.0, 20.0] {
            let p = Ionospheric::new(IonoConfig {
                ratio_db: Some(db),
                hops: 2,
                ..Default::default()
            })
            .unwrap()
            .paths(&st, &r);
            assert_eq!(p.len(), 2);
            assert!((20.0 * (p[0].field_uv_m / g.field_uv_m).log10() - db).abs() < 1e-9);
            assert_eq!(p[1].kind, PathKind::Sky { hop: 2 });
            assert!(p[1].delay > p[0].delay);
            assert!(p[1].field_uv_m > 0.0);
        }
    }
}
