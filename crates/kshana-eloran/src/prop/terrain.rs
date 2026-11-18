// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Terrain groundwave: the attenuation function over an irregular terrain profile with
//! varying ground constants, by Hufford's integral equation (the method of Ott's WAGNER
//! program and of the LF/MF integral-equation codes), combined with the smooth-earth
//! machinery of [`super::ground`] so that `ASF_terrain = SF_model − SF_seawater`.
//!
//! `[propagation.ground] model = "terrain"` ([`TerrainConfig`]).
//!
//! # The integral equation (time dependence `e^{+iωt}`, as in [`super::smooth_earth`])
//!
//! For a profile of heights `y(x)` and normalised surface impedance `Δ(x)` (eq. 4 of
//! [`super::smooth_earth`], from the local σ and ε_r) between a transmitter at `x = 0` and
//! an observation point at `x = d`, both on the ground, the attenuation function obeys the
//! Volterra equation (Hufford 1952, "An integral equation approach to the problem of wave
//! propagation over an irregular surface", Q. Appl. Math. 9; Ott & Berry 1970, Ott 1971,
//! "An alternative integral equation for propagation over irregular terrain", Radio
//! Science 5 and 6; the planar form with earth curvature carried by the bulge):
//!
//! ```text
//! W(d) = 1 − √(i d/λ) ∫₀^d W(x) [Δ(x) + y_e'(x) − (y_e(d) − y_e(x))/(d − x)]
//!                              · exp(−i k ρ(x, d)) dx / √(x (d − x))                (T1)
//! y_e(x) = y(x) + x (d − x)/(2 a_e)                (height above the chord, plus bulge)
//! ρ(x, d) = g² d / (2 x (d − x)),   g = y_e(x) − [y(0) + (y(d) − y(0)) x/d]           (T2)
//! ```
//!
//! `ρ` is the parabolic path-length excess of the ray transmitter → surface point →
//! observer over the direct chord; the bracket is the impedance plus the normal derivative
//! of the Green function (it vanishes on a plane, so a tilted plane gives the flat-earth
//! result). The normalisation and the sign of the slope term were fixed against two
//! independent limits of the existing smooth-earth code: (i) a flat homogeneous ground
//! (first order in `√p` reproduces Norton's `F(p) ≈ 1 − i√(πp)`), and (ii) a perfectly
//! conducting sphere (the bulge reproduces Fock's `(√π/4) e^{−3iπ/4} x^{3/2}` term).
//!
//! **Chord reference.** The earth-flattened equation measures phase along the chord
//! `2 a_e sin(d/2a_e) ≈ d − d³/(24 a_e²)` rather than along the arc `d`, so the solution is
//! multiplied by `exp(+i k d³/(24 a_e²))` (2.2 ns at 100 km, 60 ns at 300 km, a pure
//! geometric term). With it a flat profile over homogeneous ground reproduces the
//! smooth-earth attenuation function (residue series) within 0.5 ns and 0.005 dB to 400 km
//! and 1.5 ns / 0.02 dB to 1000 km (1 km steps, all grounds tested); see the tests.
//! Beyond ≈ 1200 km over poor ground `|W|` is small and the cancellation in (T1) grows the
//! error (−45 ns at 1500 km, σ = 1 mS/m), hence the `ratio` method below.
//!
//! # Numerics
//!
//! The profile is sampled every `step_m` (default 500 m, λ/6 at 100 kHz) on the WGS-84
//! geodesic. With `x = d sin²θ` the weight `dx/√(x(d − x)) = 2 dθ` is regular, and the
//! integrand (`W ∝ √x` near the start) is smooth in θ, so (T1) is integrated by the
//! trapezoidal rule in θ on the profile nodes ("product integration"); the last node
//! enters implicitly and is solved for. One solution marches out to `d` (O(N²)); three
//! are made (f₀ and f₀ ± 10 kHz) for the group delay / ECD, in parallel threads.
//!
//! Stability: Hufford's equation is ill-behaved for steep, rough profiles sampled finer
//! than the wavelength resolves. The heights are smoothed with a running mean over
//! `smooth_m` (default 1000 m ≈ λ/3) and the slopes are clipped to ±`max_slope` (default
//! 0.3); this removes features the 3 km wavelength does not see. Antenna heights are zero
//! (both terminals on the local ground).
//!
//! # Methods
//!
//! * `direct`: the solution of (T1) **is** the attenuation function; the ASF is its phase
//!   delay minus the seawater SF ([`super::ground`] convention), the field and ECD
//!   likewise. On a mixed-conductivity path this is the integral-equation mixed-path
//!   result (no Millington approximation).
//! * `ratio`: Millington over the conductivity map (exactly the `millington` model) times
//!   the terrain factor `W_IE(profile)/W_IE(flat profile)` over the same grounds. Errors of
//!   the long-range solution largely cancel in the ratio, and a flat profile reproduces
//!   Millington exactly.
//! * `auto` (default): `direct` up to `direct_max_km` (1000 km), `ratio` beyond.
//!
//! # Caching
//!
//! As for Millington maps: corrections relative to seawater are computed on a lat/lon grid
//! (`asf_grid_deg`, default 0.01°) around the receiver, memoised per station, and
//! bilinearly interpolated; distance, PF, SF and the induction terms are exact per call.
//!
//! # Evidence and limits (MODELLED)
//!
//! Checked internally: flat profile = smooth-earth (above); flat profile with the `ratio`
//! method = Millington exactly; mixed land/sea path vs Millington (agreement within the
//! tens of ns expected of Millington's approximation); a hill on the path adds delay and
//! reduces the field behind it, more for a higher hill; reciprocity is **not** exact (the
//! equation is solved from the transmitter, as WAGNER does). No external reference (a
//! published WAGNER/GRWAVE/LFMF terrain case or a measured ASF profile) is reproduced, so
//! the terrain effect is MODELLED, not VALIDATED. Not modelled: antenna heights, forests
//! and buildings (see [`super::urban`]), backscatter (the equation is the forward-scatter
//! approximation), lateral (off-great-circle) terrain, ducting.

use super::complex::{Cx, ONE};
use super::conductivity::{ConductivityConfig, ConductivityMap};
use super::dem::{profile, Dem, DemConfig};
use super::ground::{
    ground_curve, Correction, Groundwave, TemporalConfig, E1_UV_M_PER_SQRT_KW_AT_1KM, F0_HZ,
    K_LORAN, N_ATM,
};
use super::smooth_earth::{near_field, Ground, GroundCurve, GROUP_HALF_BAND_HZ, R_EARTH, SEAWATER};
use super::{BuildContext, PathEffect, PathKind, PropagationModel, C};
use crate::chain::Station;
use crate::geo::{inverse, Geo};
use crate::time::Ps;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

/// Solve the integral equation (T1) on a uniformly sampled profile.
///
/// `h` heights (m) at `x_m = m·step` (m = 0..N), `delta` the surface impedance at each
/// node at the wavenumber `k` (rad/m), `a_e` the effective earth radius, `max_slope` the
/// slope clip. Returns `W` at every node (`W₀ = 1`), referred to the chord (multiply by
/// [`chord_correction`] for the arc reference).
pub fn solve_ie(h: &[f64], step: f64, delta: &[Cx], k: f64, a_e: f64, max_slope: f64) -> Vec<Cx> {
    let n_pts = h.len();
    let mut w = vec![ONE; n_pts];
    if n_pts < 2 {
        return w;
    }
    let mut hp = vec![0.0; n_pts];
    for (m, d) in hp.iter_mut().enumerate() {
        let (a, b) = (m.saturating_sub(1), (m + 1).min(n_pts - 1));
        *d = ((h[b] - h[a]) / ((b - a) as f64 * step)).clamp(-max_slope, max_slope);
    }
    let sqrt_i = Cx::cis(PI / 4.0);
    let mut f = vec![Cx::default(); n_pts];
    let mut th = vec![0.0; n_pts];
    for n in 1..n_pts {
        let d = n as f64 * step;
        let c = sqrt_i * (d * k / (2.0 * PI)).sqrt();
        let (h0, hn) = (h[0], h[n]);
        let inv_n = 1.0 / n as f64;
        for m in 0..n {
            let x = m as f64 * step;
            let b = x * (d - x) / (2.0 * a_e);
            let ye = h[m] + b;
            let yep = hp[m] + (d - 2.0 * x) / (2.0 * a_e);
            let kern = delta[m] + Cx::real(yep - (hn - ye) / (d - x));
            let rho = if m == 0 {
                0.0
            } else {
                let g = ye - (h0 + (hn - h0) * m as f64 * inv_n);
                g * g * d / (2.0 * x * (d - x))
            };
            f[m] = w[m] * kern * Cx::cis(-k * rho);
            th[m] = (m as f64 * inv_n).sqrt().asin();
        }
        let mut s = Cx::default();
        for m in 1..n {
            s += (f[m - 1] + f[m]) * (th[m] - th[m - 1]);
        }
        let dth = PI / 2.0 - th[n - 1];
        s += f[n - 1] * dth;
        w[n] = (ONE - c * s) / (ONE + c * delta[n] * dth);
    }
    w
}

/// Chord-to-arc phase factor `exp(+i k d³/(24 a_e²))` (see the module documentation).
pub fn chord_correction(k: f64, d: f64, a_e: f64) -> Cx {
    Cx::cis(k * d.powi(3) / (24.0 * a_e * a_e))
}

/// Running mean over `win` samples (truncated at the ends).
pub fn smooth(h: &[f64], win: usize) -> Vec<f64> {
    let half = win / 2;
    if half == 0 || h.len() < 3 {
        return h.to_vec();
    }
    let mut pre = vec![0.0; h.len() + 1];
    for (i, v) in h.iter().enumerate() {
        pre[i + 1] = pre[i] + v;
    }
    (0..h.len())
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half).min(h.len() - 1);
            (pre[b + 1] - pre[a]) / (b + 1 - a) as f64
        })
        .collect()
}

/// Phase delay, group delay (s, arc reference) and ln-amplitude at the last node of
/// profile solutions at `f0 − 10 kHz`, `f0`, `f0 + 10 kHz`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IeResult {
    pub phase_delay_s: f64,
    pub group_delay_s: f64,
    pub ln_amp: f64,
}

/// Unwrapped argument of the last element of a continuous sequence.
fn unwrapped_last(w: &[Cx]) -> f64 {
    let mut u = 0.0f64;
    for z in w {
        let mut a = z.arg();
        while a - u > PI {
            a -= 2.0 * PI;
        }
        while a - u < -PI {
            a += 2.0 * PI;
        }
        u = a;
    }
    u
}

/// Solve the three frequencies over a profile of heights `h` (spacing `step`) and grounds.
pub fn solve_profile(
    h: &[f64],
    step: f64,
    grounds: &[Ground],
    k_factor: f64,
    max_slope: f64,
) -> IeResult {
    let a_e = k_factor * R_EARTH;
    let d = step * (h.len().saturating_sub(1)) as f64;
    let run = |f: f64| -> (f64, f64) {
        let k = N_ATM * 2.0 * PI * f / C;
        let delta: Vec<Cx> = grounds.iter().map(|g| g.delta(f)).collect();
        let w = solve_ie(h, step, &delta, k, a_e, max_slope);
        let last = *w.last().unwrap_or(&ONE);
        // arg with the chord correction (a pure phase)
        let arg = unwrapped_last(&w) + k * d.powi(3) / (24.0 * a_e * a_e);
        (arg, last.abs().ln())
    };
    let ((am, _), (a0, ln0), (ap, _)) = std::thread::scope(|s| {
        let hm = s.spawn(|| run(F0_HZ - GROUP_HALF_BAND_HZ));
        let hp = s.spawn(|| run(F0_HZ + GROUP_HALF_BAND_HZ));
        let r0 = run(F0_HZ);
        (
            hm.join().expect("IE thread"),
            r0,
            hp.join().expect("IE thread"),
        )
    });
    IeResult {
        phase_delay_s: -a0 / (2.0 * PI * F0_HZ),
        group_delay_s: -(ap - am) / (2.0 * PI * 2.0 * GROUP_HALF_BAND_HZ),
        ln_amp: ln0,
    }
}

/// Integral-equation method.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainMethod {
    /// `direct` up to `direct_max_km`, `ratio` beyond.
    #[default]
    Auto,
    Direct,
    Ratio,
}

/// `model = "terrain"`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainConfig {
    /// Terrain heights (default flat 0 m).
    #[serde(default)]
    pub dem: DemConfig,
    /// Height used where the DEM has no data (m).
    #[serde(default)]
    pub void_fill_m: f64,
    /// Conductivity map (default uniform seawater) …
    #[serde(default)]
    pub conductivity: ConductivityConfig,
    /// … or an imported conductivity atlas, which then replaces `conductivity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<super::p832::AtlasConfig>,
    #[serde(default = "d_k")]
    pub k_factor: f64,
    /// Profile sample spacing (m).
    #[serde(default = "d_step")]
    pub step_m: f64,
    /// Running-mean smoothing window of the heights (m); 0 = none.
    #[serde(default = "d_smooth")]
    pub smooth_m: f64,
    /// Slope clip.
    #[serde(default = "d_slope")]
    pub max_slope: f64,
    /// Grid (degrees) on which the corrections are memoised.
    #[serde(default = "d_grid")]
    pub asf_grid_deg: f64,
    #[serde(default)]
    pub method: TerrainMethod,
    /// Longest path solved with the `direct` method under `auto` (km).
    #[serde(default = "d_direct_max")]
    pub direct_max_km: f64,
    /// Millington conductivity sample spacing for the `ratio` method (km).
    #[serde(default = "d_sample")]
    pub sample_km: f64,
    #[serde(default)]
    pub temporal: Option<TemporalConfig>,
}

fn d_k() -> f64 {
    K_LORAN
}
fn d_step() -> f64 {
    500.0
}
fn d_smooth() -> f64 {
    1000.0
}
fn d_slope() -> f64 {
    0.3
}
fn d_grid() -> f64 {
    0.01
}
fn d_direct_max() -> f64 {
    1000.0
}
fn d_sample() -> f64 {
    2.0
}

impl Default for TerrainConfig {
    fn default() -> Self {
        TerrainConfig {
            dem: DemConfig::default(),
            void_fill_m: 0.0,
            conductivity: ConductivityConfig::default(),
            atlas: None,
            k_factor: d_k(),
            step_m: d_step(),
            smooth_m: d_smooth(),
            max_slope: d_slope(),
            asf_grid_deg: d_grid(),
            method: TerrainMethod::Auto,
            direct_max_km: d_direct_max(),
            sample_km: d_sample(),
            temporal: None,
        }
    }
}

/// The groundwave path from a correction relative to seawater: exactly the construction of
/// [`Groundwave::evaluate`] (PF, seawater SF with the induction terms, ASF = correction
/// phase + `temporal_s`, field and ECD), shared by the terrain, ASF-grid and uncertainty
/// models.
pub fn path_from_correction(
    sea: &GroundCurve,
    k_factor: f64,
    tx: Geo,
    power_kw: f64,
    rx: Geo,
    corr: Correction,
    temporal_s: f64,
) -> PathEffect {
    let d = inverse(tx, rx).distance_m.max(100.0);
    let geom = d / C;
    let pf = (N_ATM - 1.0) * geom;
    let s = sea.at(d);
    let kd = |f: f64| N_ATM * 2.0 * PI * f / C * d;
    let n0 = near_field(kd(F0_HZ));
    let nm = near_field(kd(F0_HZ - GROUP_HALF_BAND_HZ));
    let np = near_field(kd(F0_HZ + GROUP_HALF_BAND_HZ));
    let tp_n = -n0.arg() / (2.0 * PI * F0_HZ);
    let tg_n = -(np.arg() - nm.arg()) / (2.0 * PI * 2.0 * GROUP_HALF_BAND_HZ);
    let sf = s.phase_delay_s + tp_n;
    let asf = corr.phase_s + temporal_s;
    let tau_p = sf + corr.phase_s;
    let tau_g = s.group_delay_s + corr.group_s + tg_n;
    let th = d / (k_factor * R_EARTH);
    let spread = if th < 1e-6 {
        1.0
    } else {
        (th / th.sin()).sqrt()
    };
    let field = E1_UV_M_PER_SQRT_KW_AT_1KM * power_kw.max(0.0).sqrt() / (d / 1000.0)
        * spread
        * n0.abs()
        * (s.ln_amp + corr.ln_amp).exp();
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

/// Bilinear interpolation of memoised node corrections on a `q`-degree grid.
pub fn grid_bilinear<F: Fn(i64, i64) -> Correction>(q: f64, rx: Geo, node: F) -> Correction {
    let fi = rx.lat_deg / q;
    let fj = rx.lon_deg / q;
    let (i0, j0) = (fi.floor() as i64, fj.floor() as i64);
    let (u, v) = (fi - i0 as f64, fj - j0 as f64);
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

/// The terrain groundwave model.
pub struct TerrainGroundwave {
    pub cfg: TerrainConfig,
    pub dem: Dem,
    pub map: ConductivityMap,
    sigma_scale: f64,
    sea: Arc<GroundCurve>,
    /// Millington over the same map (ratio method).
    mill: Groundwave,
    cache: Mutex<HashMap<(u64, u64, i64, i64), Correction>>,
    label: String,
}

impl TerrainGroundwave {
    pub fn new(
        cfg: TerrainConfig,
        dem: Dem,
        map: ConductivityMap,
    ) -> Result<TerrainGroundwave, String> {
        if !(cfg.k_factor > 0.5 && cfg.k_factor < 10.0) {
            return Err(format!(
                "terrain: k_factor {} out of range (0.5, 10)",
                cfg.k_factor
            ));
        }
        if !(cfg.step_m >= 10.0 && cfg.step_m <= 10_000.0) {
            return Err("terrain: step_m must be within 10–10 000 m".into());
        }
        if cfg.smooth_m.is_nan() || cfg.smooth_m < 0.0 || !(cfg.max_slope > 0.0) {
            return Err("terrain: smooth_m ≥ 0 and max_slope > 0".into());
        }
        if !(cfg.asf_grid_deg > 0.0) || !(cfg.direct_max_km > 0.0) {
            return Err("terrain: asf_grid_deg and direct_max_km must be positive".into());
        }
        let sigma_scale = cfg
            .temporal
            .as_ref()
            .map(|t| t.sigma_scale())
            .unwrap_or(1.0);
        if sigma_scale.is_nan() || sigma_scale <= 0.0 {
            return Err("terrain: sigma_seasonal_frac makes a conductivity non-positive".into());
        }
        // Millington part: same map and seasonal σ (its temporal ΔASF is applied here)
        let mill_t = cfg.temporal.as_ref().map(|t| TemporalConfig {
            diurnal_amp_ns: 0.0,
            seasonal_amp_ns: 0.0,
            ..t.clone()
        });
        let mill = Groundwave::new(
            map.clone(),
            cfg.k_factor,
            cfg.sample_km,
            cfg.asf_grid_deg,
            mill_t,
            "terrain-millington",
        )?;
        let label = format!(
            "groundwave-terrain({}, k={}, {:?})",
            dem.source.describe(),
            cfg.k_factor,
            cfg.method
        );
        Ok(TerrainGroundwave {
            sea: ground_curve(SEAWATER, cfg.k_factor),
            cfg,
            dem,
            map,
            sigma_scale,
            mill,
            cache: Mutex::new(HashMap::new()),
            label,
        })
    }

    fn ground_at(&self, p: Geo) -> Ground {
        let g = self.map.at(p);
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

    /// The smoothed profile from `tx` to `rx`: (heights, spacing, grounds, land fraction).
    pub fn profile(&self, tx: Geo, rx: Geo) -> (Vec<f64>, f64, Vec<Ground>, f64) {
        let d = inverse(tx, rx).distance_m;
        let n = ((d / self.cfg.step_m).ceil() as usize).max(2);
        let step = d / n as f64;
        let p = profile(&self.dem, tx, rx, n);
        let raw: Vec<f64> = p.iter().map(|q| q.1).collect();
        let win = (self.cfg.smooth_m / step).round() as usize;
        let h = smooth(&raw, win);
        let grounds: Vec<Ground> = p.iter().map(|q| self.ground_at(q.2)).collect();
        let land = grounds.iter().filter(|g| g.sigma < 1.0).count() as f64 / grounds.len() as f64;
        (h, step, grounds, land)
    }

    /// Whether a path of `d_m` uses the direct method.
    pub fn uses_direct(&self, d_m: f64) -> bool {
        match self.cfg.method {
            TerrainMethod::Direct => true,
            TerrainMethod::Ratio => false,
            TerrainMethod::Auto => d_m <= self.cfg.direct_max_km * 1000.0,
        }
    }

    /// Correction (terrain model − seawater) for the path `tx` → `rx`, solved exactly.
    pub fn correction_exact(&self, tx: Geo, rx: Geo) -> Correction {
        let d = inverse(tx, rx).distance_m;
        if d < 2.0 * self.cfg.step_m {
            // too short for a profile: Millington (terrain irrelevant at this range)
            return self.mill.correction_exact(tx, rx);
        }
        let (h, step, grounds, land) = self.profile(tx, rx);
        let ie = solve_profile(&h, step, &grounds, self.cfg.k_factor, self.cfg.max_slope);
        if self.uses_direct(d) {
            let s = self.sea.at(d);
            Correction {
                phase_s: ie.phase_delay_s - s.phase_delay_s,
                ln_amp: ie.ln_amp - s.ln_amp,
                group_s: ie.group_delay_s - s.group_delay_s,
                land,
            }
        } else {
            let flat = vec![0.0; h.len()];
            let f = solve_profile(&flat, step, &grounds, self.cfg.k_factor, self.cfg.max_slope);
            let m = self.mill.correction_exact(tx, rx);
            Correction {
                phase_s: m.phase_s + ie.phase_delay_s - f.phase_delay_s,
                ln_amp: m.ln_amp + ie.ln_amp - f.ln_amp,
                group_s: m.group_s + ie.group_delay_s - f.group_delay_s,
                land: m.land,
            }
        }
    }

    /// Correction at a receiver position, bilinear between memoised grid nodes.
    pub fn correction(&self, tx: Geo, rx: Geo) -> Correction {
        let q = self.cfg.asf_grid_deg;
        grid_bilinear(q, rx, |i, j| {
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
        })
    }

    /// Groundwave path at scene time `t_s` (grid-interpolated correction).
    pub fn evaluate(&self, tx: Geo, power_kw: f64, rx: Geo, t_s: f64) -> PathEffect {
        let corr = self.correction(tx, rx);
        self.evaluate_with(tx, power_kw, rx, t_s, corr)
    }

    /// Groundwave path with a given correction.
    pub fn evaluate_with(
        &self,
        tx: Geo,
        power_kw: f64,
        rx: Geo,
        t_s: f64,
        corr: Correction,
    ) -> PathEffect {
        let temporal = self
            .cfg
            .temporal
            .as_ref()
            .map(|t| t.delta_asf_s(t_s, corr.land))
            .unwrap_or(0.0);
        path_from_correction(
            &self.sea,
            self.cfg.k_factor,
            tx,
            power_kw,
            rx,
            corr,
            temporal,
        )
    }
}

impl PropagationModel for TerrainGroundwave {
    fn name(&self) -> String {
        self.label.clone()
    }
    fn paths(&self, st: &Station, rx: &RxState) -> Vec<PathEffect> {
        vec![self.evaluate(st.position, st.power_kw, rx.pos, rx.t_s)]
    }
}

/// Build the terrain model (the temporal epoch from `ctx` unless overridden).
pub fn build(cfg: &TerrainConfig, ctx: &BuildContext) -> Result<TerrainGroundwave, String> {
    let mut c = cfg.clone();
    c.temporal = c.temporal.as_ref().map(|t| t.resolved(ctx.start));
    let dem = Dem::build(&c.dem, c.void_fill_m)?;
    let map = match &c.atlas {
        Some(a) => super::p832::load_atlas(a)?.map,
        None => ConductivityMap::build(&c.conductivity)?,
    };
    TerrainGroundwave::new(c, dem, map)
}

#[cfg(test)]
mod tests {
    use super::super::conductivity::{GroundConst, PolygonConfig};
    use super::super::dem::{Flat, Hill, Hills};
    use super::super::smooth_earth::SmoothEarth;
    use super::*;

    fn tx() -> Geo {
        Geo::new(0.0, 0.0)
    }
    fn at_km(km: f64) -> Geo {
        crate::geo::direct(tx(), 0.0, km * 1000.0)
    }
    fn uniform(sigma: f64, eps: f64) -> ConductivityMap {
        ConductivityMap::build(&ConductivityConfig::Uniform {
            sigma_s_m: sigma,
            eps_r: Some(eps),
        })
        .unwrap()
    }
    fn model(
        dem: Dem,
        map: ConductivityMap,
        step: f64,
        method: TerrainMethod,
    ) -> TerrainGroundwave {
        TerrainGroundwave::new(
            TerrainConfig {
                step_m: step,
                method,
                ..Default::default()
            },
            dem,
            map,
        )
        .unwrap()
    }

    #[test]
    fn flat_profile_reproduces_the_smooth_earth_attenuation_function() {
        // MODELLED (internal consistency): the IE over a flat homogeneous profile equals the
        // residue-series/flat-curved W of smooth_earth: < 1 ns and < 0.01 dB to 300 km
        // (1 km steps; the module documents 1.5 ns / 0.02 dB to 1000 km)
        for g in [
            SEAWATER,
            Ground {
                sigma: 0.003,
                eps_r: 15.0,
            },
            Ground {
                sigma: 0.001,
                eps_r: 10.0,
            },
        ] {
            let se = SmoothEarth::new(g, F0_HZ, K_LORAN, N_ATM);
            let k = N_ATM * 2.0 * PI * F0_HZ / C;
            let step = 1000.0;
            let n = 300;
            let w = solve_ie(
                &vec![0.0; n + 1],
                step,
                &vec![g.delta(F0_HZ); n + 1],
                k,
                se.a_e,
                0.3,
            );
            for km in [20.0, 60.0, 100.0, 200.0, 300.0] {
                let d = km * 1000.0;
                let r = w[(d / step) as usize] * chord_correction(k, d, se.a_e) / se.w(d);
                let dt = -r.arg() / (2.0 * PI * F0_HZ) * 1e9;
                let db = 20.0 * r.abs().log10();
                assert!(
                    dt.abs() < 1.0 && db.abs() < 0.01,
                    "{g:?} {km} km: {dt} ns {db} dB"
                );
            }
        }
    }

    #[test]
    fn flat_terrain_model_matches_smooth_earth_model() {
        // through the full model (delay, field, ECD) vs the smooth_earth ground model
        let map = uniform(0.003, 15.0);
        let t = model(
            Dem::new(Box::new(Flat(0.0)), 0.0),
            map.clone(),
            1000.0,
            TerrainMethod::Direct,
        );
        let se = Groundwave::new(map, K_LORAN, 2.0, 0.005, None, "se").unwrap();
        for km in [30.0, 120.0, 250.0] {
            let c = t.correction_exact(tx(), at_km(km));
            let a = t.evaluate_with(tx(), 400.0, at_km(km), 0.0, c);
            let b = se.evaluate(tx(), 400.0, at_km(km), 0.0);
            assert!(
                (a.asf_ns - b.asf_ns).abs() < 1.5,
                "{km}: {} vs {}",
                a.asf_ns,
                b.asf_ns
            );
            assert!((a.sf_ns - b.sf_ns).abs() < 1e-9);
            assert!((20.0 * (a.field_uv_m / b.field_uv_m).log10()).abs() < 0.02);
            assert!(
                (a.ecd_s - b.ecd_s).abs() < 2e-9,
                "{km}: ECD {} vs {}",
                a.ecd_s,
                b.ecd_s
            );
            assert!((c.land - 1.0).abs() < 1e-12);
        }
        // ratio method with a flat profile = Millington exactly
        let r = model(
            Dem::new(Box::new(Flat(120.0)), 0.0),
            uniform(0.003, 15.0),
            1000.0,
            TerrainMethod::Ratio,
        );
        let c = r.correction_exact(tx(), at_km(150.0));
        let m = r.mill.correction_exact(tx(), at_km(150.0));
        assert!((c.phase_s - m.phase_s).abs() < 1e-15 && (c.ln_amp - m.ln_amp).abs() < 1e-12);
    }

    fn hill_dem(height: f64) -> Dem {
        Dem::new(
            Box::new(Hills {
                base_m: 0.0,
                hills: vec![Hill {
                    lat_deg: at_km(60.0).lat_deg,
                    lon_deg: 0.0,
                    height_m: height,
                    radius_km: 6.0,
                }],
            }),
            0.0,
        )
    }

    #[test]
    fn hill_adds_delay_and_attenuation_behind_it() {
        // MODELLED trend: a hill mid-path delays the groundwave and lowers the field
        // behind it, monotonically with its height; before the hill nothing changes much
        let map = uniform(0.003, 15.0);
        let rx = at_km(110.0);
        let flat = model(hill_dem(0.0), map.clone(), 500.0, TerrainMethod::Direct)
            .correction_exact(tx(), rx);
        let mut last = (0.0, 0.0);
        for h in [150.0, 300.0, 600.0] {
            let c = model(hill_dem(h), map.clone(), 500.0, TerrainMethod::Direct)
                .correction_exact(tx(), rx);
            let dt = (c.phase_s - flat.phase_s) * 1e9;
            let ddb = 20.0 / std::f64::consts::LN_10 * (c.ln_amp - flat.ln_amp);
            println!("hill {h} m: ΔASF {dt:.1} ns, Δfield {ddb:.2} dB");
            assert!(dt > last.0 && ddb < last.1, "{h} m: {dt} ns {ddb} dB");
            last = (dt, ddb);
        }
        assert!(last.0 > 5.0 && last.0 < 2000.0, "600 m hill: {} ns", last.0);
        // before the hill: little effect
        let before_flat = model(hill_dem(0.0), map.clone(), 500.0, TerrainMethod::Direct)
            .correction_exact(tx(), at_km(35.0));
        let before = model(hill_dem(600.0), map, 500.0, TerrainMethod::Direct)
            .correction_exact(tx(), at_km(35.0));
        assert!(((before.phase_s - before_flat.phase_s) * 1e9).abs() < 0.2 * last.0);
    }

    #[test]
    fn mixed_path_agrees_with_millington_within_its_accuracy() {
        // sea for the first 40 km, land beyond: the IE mixed-path result and Millington
        // agree to tens of ns (Millington's documented accuracy)
        let coast = at_km(40.0).lat_deg;
        let map = ConductivityMap::build(&ConductivityConfig::Polygons {
            background: GroundConst::seawater(),
            polygons: vec![PolygonConfig {
                name: "land".into(),
                sigma_s_m: 0.003,
                eps_r: Some(15.0),
                vertices: vec![[coast, -1.0], [5.0, -1.0], [5.0, 1.0], [coast, 1.0]],
            }],
        })
        .unwrap();
        let t = model(
            Dem::new(Box::new(Flat(0.0)), 0.0),
            map,
            1000.0,
            TerrainMethod::Direct,
        );
        for km in [80.0, 150.0] {
            let ie = t.correction_exact(tx(), at_km(km));
            let m = t.mill.correction_exact(tx(), at_km(km));
            let dt = (ie.phase_s - m.phase_s) * 1e9;
            let ddb = 20.0 / std::f64::consts::LN_10 * (ie.ln_amp - m.ln_amp);
            println!(
                "mixed {km} km: IE−Millington {dt:.1} ns {ddb:.2} dB (ASF {:.0} ns)",
                ie.phase_s * 1e9
            );
            assert!(dt.abs() < 50.0 && ddb.abs() < 1.0, "{km}: {dt} ns {ddb} dB");
            assert!(ie.land > 0.3 && ie.land < 0.8);
        }
    }

    #[test]
    fn grid_cache_and_config() {
        let cfg: super::super::ground::GroundConfig = toml::from_str(
            r#"
model = "terrain"
step_m = 2000.0
asf_grid_deg = 0.02
[dem]
type = "hills"
hills = [{ lat_deg = 0.3, lon_deg = 0.0, height_m = 400.0, radius_km = 5.0 }]
[conductivity]
type = "uniform"
sigma_s_m = 0.005
"#,
        )
        .unwrap();
        let ctx = BuildContext::new(
            crate::time::AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap(),
            1.0,
            0,
        );
        let m = super::super::ground::build(&cfg, &ctx).unwrap();
        assert!(m.name().contains("terrain"));
        let st = Station {
            id: "M".into(),
            name: "M".into(),
            role: crate::signal::Role::Master,
            position: tx(),
            power_kw: 100.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        };
        let mut prev: Option<PathEffect> = None;
        for i in 0..5 {
            let rx = RxState {
                t_s: 0.0,
                pos: Geo::new(0.6 + 0.004 * i as f64, 0.01),
                heading_rad: 0.0,
                speed_mps: 0.0,
            };
            let p = m.paths(&st, &rx)[0];
            assert!(p.asf_ns > 0.0 && p.field_uv_m > 0.0);
            if let Some(q) = prev {
                assert!((p.asf_ns - q.asf_ns).abs() < 20.0, "continuous");
            }
            prev = Some(p);
        }
        assert!(TerrainGroundwave::new(
            TerrainConfig {
                step_m: 1.0,
                ..Default::default()
            },
            Dem::new(Box::new(Flat(0.0)), 0.0),
            uniform(5.0, 80.0)
        )
        .is_err());
    }
}
