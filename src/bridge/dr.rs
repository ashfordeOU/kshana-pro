// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Dead-reckoning (DR) quality during a GNSS outage (research 07 B02/B03, 07b M11).
//!
//! A ship that loses GNSS keeps an estimated position from gyro heading and log speed. How
//! fast that estimate degrades decides how long the bridge can rely on it before the
//! uncertainty is larger than the water it has to work with. This module states the error
//! budget in closed form for a straight leg at constant speed, along and across the track:
//!
//! ```text
//! along^2(T) = s0^2 + (b_v^2 + (v k_v)^2 + c^2) T^2 + q_v T
//! cross^2(T) = s0^2 + v^2 (b_h^2 T^2 + d_h^2 T^4 / 4) + c^2 T^2 + q_v T
//! ```
//!
//! with `s0` the initial position error per axis, `b_v` the log bias, `k_v` its scale
//! error, `b_h` the gyro bias (radians), `d_h` the gyro drift rate (radians per second),
//! `c` the unknown current per axis (zero while the log tracks the bottom) and `q_v` the
//! white-noise density of the log speed (random walk of position). Each term is an
//! independent zero-mean error with the stated standard deviation. The 95 % horizontal
//! radius integrates the elliptical normal exactly (one-dimensional quadrature, bisection
//! on the radius), and [`DrModel::monte_carlo`] draws the same errors and integrates a DR
//! track second by second to check the closed form.
//!
//! **Bottom track.** A Doppler log tracks the bottom only down to its depth limit (often
//! about 200 m); deeper, it falls back to the water track and the unknown current enters.
//! [`DrModel::effective`] applies that rule from the stated depth.
//!
//! MODELLED: the closed form is checked against its own Monte Carlo; the default sensor
//! figures are typical class values, not those of any named equipment.

use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use super::{sig9, KN_MPS};

/// The error budget of a DR estimate.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct DrModel {
    /// Speed, knots.
    pub speed_kn: f64,
    /// Initial position error at the start of the outage, metres (1 sigma per axis).
    pub initial_m: f64,
    /// Log speed bias, knots (1 sigma).
    pub log_bias_kn: f64,
    /// Log scale error, fraction of speed (1 sigma).
    pub log_scale: f64,
    /// Log white noise, knots at 1 Hz (1 sigma).
    pub log_noise_kn: f64,
    /// Gyro heading bias, degrees (1 sigma).
    pub gyro_bias_deg: f64,
    /// Gyro drift, degrees per hour (1 sigma).
    pub gyro_drift_deg_h: f64,
    /// Unknown current per axis, knots (1 sigma), when the log is on the water track.
    pub current_kn: f64,
    /// Whether the log tracks the bottom (ground speeds); `None` decides from the depth.
    pub bottom_track: Option<bool>,
    /// Depth of water, metres, for the bottom-track rule.
    pub depth_m: Option<f64>,
    /// Depth limit of the log's bottom track, metres.
    pub bottom_track_max_depth_m: f64,
}

impl Default for DrModel {
    fn default() -> Self {
        DrModel {
            speed_kn: 12.0,
            initial_m: 10.0,
            log_bias_kn: 0.1,
            log_scale: 0.01,
            log_noise_kn: 0.05,
            gyro_bias_deg: 0.5,
            gyro_drift_deg_h: 0.1,
            current_kn: 0.5,
            bottom_track: None,
            depth_m: None,
            bottom_track_max_depth_m: 200.0,
        }
    }
}

/// One row of the error-growth table.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrRow {
    pub t_s: f64,
    pub along_m: f64,
    pub cross_m: f64,
    pub r95_m: f64,
}

/// The empirical figures of a Monte Carlo run at one time.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrMonteCarlo {
    pub t_s: f64,
    pub runs: usize,
    pub seed: u64,
    pub along_m: f64,
    pub cross_m: f64,
    pub r95_m: f64,
}

/// A DR quality report.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrReport {
    pub method: String,
    pub label: String,
    pub advisory: String,
    pub model: DrModel,
    /// Whether the current term applies (water track).
    pub water_track: bool,
    pub table: Vec<DrRow>,
    /// The limit asked about, metres, and the time the 95 % radius reaches it.
    pub limit_m: Option<f64>,
    pub time_to_limit_s: Option<f64>,
    pub monte_carlo: Option<DrMonteCarlo>,
}

/// The method tag of the DR report.
pub const METHOD: &str = "kshana-pro/dr-quality/v1";

impl DrModel {
    pub fn validate(&self) -> Result<(), String> {
        let v = [
            self.speed_kn,
            self.initial_m,
            self.log_bias_kn,
            self.log_scale,
            self.log_noise_kn,
            self.gyro_bias_deg,
            self.gyro_drift_deg_h,
            self.current_kn,
            self.bottom_track_max_depth_m,
        ];
        if v.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return Err("every DR figure must be finite and not negative".into());
        }
        Ok(())
    }

    /// Whether the unknown current enters (the log is on the water track).
    pub fn water_track(&self) -> bool {
        match (self.bottom_track, self.depth_m) {
            (Some(b), _) => !b,
            (None, Some(d)) => d > self.bottom_track_max_depth_m,
            (None, None) => true,
        }
    }

    /// The model with the current term set by [`Self::water_track`].
    pub fn effective(&self) -> DrModel {
        let mut m = self.clone();
        if !self.water_track() {
            m.current_kn = 0.0;
        }
        m
    }

    /// Along- and across-track 1-sigma errors after `t` seconds, metres.
    pub fn sigmas(&self, t: f64) -> (f64, f64) {
        let m = self.effective();
        let v = m.speed_kn * KN_MPS;
        let s0 = m.initial_m.powi(2);
        let bv = (m.log_bias_kn * KN_MPS).powi(2);
        let kv = (v * m.log_scale).powi(2);
        let c = (m.current_kn * KN_MPS).powi(2);
        let q = (m.log_noise_kn * KN_MPS).powi(2);
        let bh = m.gyro_bias_deg.to_radians().powi(2);
        let dh = (m.gyro_drift_deg_h.to_radians() / 3600.0).powi(2);
        let along = s0 + (bv + kv + c) * t * t + q * t;
        let cross = s0 + v * v * (bh * t * t + dh * t.powi(4) / 4.0) + c * t * t + q * t;
        (along.sqrt(), cross.sqrt())
    }

    /// The 95 % horizontal radius after `t` seconds, metres.
    pub fn r95(&self, t: f64) -> f64 {
        let (a, b) = self.sigmas(t);
        radius_for(a, b, 0.95)
    }

    /// The error-growth table at `times`.
    pub fn table(&self, times: &[f64]) -> Vec<DrRow> {
        times
            .iter()
            .map(|&t| {
                let (a, c) = self.sigmas(t);
                DrRow {
                    t_s: t,
                    along_m: sig9(a),
                    cross_m: sig9(c),
                    r95_m: sig9(radius_for(a, c, 0.95)),
                }
            })
            .collect()
    }

    /// The time the 95 % radius reaches `limit_m`, within 72 hours; `None` if never.
    pub fn time_to_limit(&self, limit_m: f64) -> Option<f64> {
        let hi = 72.0 * 3600.0;
        if self.r95(0.0) >= limit_m {
            return Some(0.0);
        }
        if self.r95(hi) < limit_m {
            return None;
        }
        let (mut lo, mut hi) = (0.0, hi);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if self.r95(mid) < limit_m {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(sig9(hi))
    }

    /// Draw the errors `runs` times and integrate a DR track second by second for `t`
    /// seconds on a straight leg; the empirical along, cross and 95 % radius.
    pub fn monte_carlo(&self, t: f64, runs: usize, seed: u64) -> DrMonteCarlo {
        let m = self.effective();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        let mut gauss = move || -> f64 {
            // Box-Muller.
            let u1: f64 = rng.gen_range(f64::EPSILON..1.0);
            let u2: f64 = rng.gen::<f64>();
            (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
        };
        let v = m.speed_kn * KN_MPS;
        let steps = t.max(0.0).round() as usize;
        let mut along = Vec::with_capacity(runs);
        let mut cross = Vec::with_capacity(runs);
        let mut radius = Vec::with_capacity(runs);
        for _ in 0..runs {
            let mut x = m.initial_m * gauss();
            let mut y = m.initial_m * gauss();
            let bv = m.log_bias_kn * KN_MPS * gauss();
            let kv = m.log_scale * gauss();
            let bh = m.gyro_bias_deg.to_radians() * gauss();
            let dh = m.gyro_drift_deg_h.to_radians() / 3600.0 * gauss();
            let (cx, cy) = (
                m.current_kn * KN_MPS * gauss(),
                m.current_kn * KN_MPS * gauss(),
            );
            for k in 0..steps {
                // The heading error at the middle of the step (exact for a linear drift).
                let h = bh + dh * (k as f64 + 0.5);
                let speed = v * (1.0 + kv) + bv + m.log_noise_kn * KN_MPS * gauss();
                // Along-track axis x, across-track y; the true track runs along x at v.
                x += speed * h.cos() - v + cx;
                y += speed * h.sin() + cy;
            }
            along.push(x);
            cross.push(y);
            radius.push(x.hypot(y));
        }
        let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
        radius.sort_by(|a, b| a.total_cmp(b));
        let i = ((0.95 * runs as f64).ceil() as usize).clamp(1, runs) - 1;
        DrMonteCarlo {
            t_s: t,
            runs,
            seed,
            along_m: sig9(rms(&along)),
            cross_m: sig9(rms(&cross)),
            r95_m: sig9(radius[i]),
        }
    }

    /// The report at `times`, with the time to `limit_m` and an optional Monte Carlo check
    /// at the last time.
    pub fn report(
        &self,
        times: &[f64],
        limit_m: Option<f64>,
        mc: Option<(usize, u64)>,
    ) -> DrReport {
        DrReport {
            method: METHOD.into(),
            label: super::LABEL.into(),
            advisory: super::ADVISORY.into(),
            model: self.clone(),
            water_track: self.water_track(),
            table: self.table(times),
            limit_m,
            time_to_limit_s: limit_m.and_then(|l| self.time_to_limit(l)),
            monte_carlo: mc.and_then(|(n, seed)| {
                times
                    .last()
                    .filter(|_| n > 0)
                    .map(|&t| self.monte_carlo(t, n, seed))
            }),
        }
    }
}

/// The standard normal cumulative distribution (Abramowitz and Stegun 7.1.26 for erf,
/// absolute error below 1.5e-7).
pub fn norm_cdf(x: f64) -> f64 {
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * z);
    let poly = t
        * (0.254_829_592
            + t * (-0.284_496_736
                + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let erf = 1.0 - poly * (-z * z).exp();
    if x >= 0.0 {
        0.5 * (1.0 + erf)
    } else {
        0.5 * (1.0 - erf)
    }
}

/// The probability that a zero-mean normal error with axis sigmas `a` and `b` lies within
/// `r` of the origin: the integral over the larger axis of its density times the
/// probability that the other axis lies within the chord, by composite Simpson.
pub fn prob_within(a: f64, b: f64, r: f64) -> f64 {
    let (a, b) = (a.max(b).max(1e-12), a.min(b).max(1e-12));
    if r <= 0.0 {
        return 0.0;
    }
    let n = 2000;
    let h = 2.0 * r / n as f64;
    let f = |x: f64| {
        let chord = (r * r - x * x).max(0.0).sqrt();
        let dens = (-0.5 * (x / a).powi(2)).exp() / (a * (std::f64::consts::TAU).sqrt());
        dens * (2.0 * norm_cdf(chord / b) - 1.0)
    };
    let mut sum = f(-r) + f(r);
    for i in 1..n {
        sum += f(-r + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    (sum * h / 3.0).clamp(0.0, 1.0)
}

/// The radius holding probability `p` of a zero-mean normal error with axis sigmas `a`, `b`.
pub fn radius_for(a: f64, b: f64, p: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 10.0 * a.max(b).max(1e-9));
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if prob_within(a, b, mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

/// Seconds of a DR quality table: every `step_s` up to `outage_s`.
pub fn times(outage_s: f64, step_s: f64) -> Vec<f64> {
    let step = if step_s > 0.0 { step_s } else { 600.0 };
    let mut v = vec![0.0];
    let mut t = step;
    while t < outage_s - 1e-9 {
        v.push(t);
        t += step;
    }
    if outage_s > 0.0 {
        v.push(outage_s);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_of_a_circular_normal_matches_rayleigh() {
        // Circular: r95 = sigma * sqrt(-2 ln 0.05) = 2.4477 sigma.
        let r = radius_for(10.0, 10.0, 0.95);
        assert!((r - 24.477).abs() < 0.01, "{r}");
        // One-dimensional limit: 1.96 sigma.
        let r = radius_for(10.0, 1e-6, 0.95);
        assert!((r - 19.6).abs() < 0.05, "{r}");
    }

    #[test]
    fn bottom_track_removes_the_current_until_the_depth_limit() {
        let mut m = DrModel {
            depth_m: Some(50.0),
            ..Default::default()
        };
        assert!(!m.water_track());
        let shallow = m.r95(3600.0);
        m.depth_m = Some(450.0);
        assert!(m.water_track());
        assert!(m.r95(3600.0) > shallow * 2.0);
    }

    #[test]
    fn closed_form_matches_monte_carlo() {
        let m = DrModel::default();
        for t in [600.0, 3600.0] {
            let (a, c) = m.sigmas(t);
            let mc = m.monte_carlo(t, 4000, 7);
            assert!(
                (mc.along_m / a - 1.0).abs() < 0.06,
                "{t}: {} vs {a}",
                mc.along_m
            );
            assert!(
                (mc.cross_m / c - 1.0).abs() < 0.06,
                "{t}: {} vs {c}",
                mc.cross_m
            );
            assert!((mc.r95_m / m.r95(t) - 1.0).abs() < 0.08);
        }
    }

    #[test]
    fn time_to_limit_is_where_the_radius_reaches_it() {
        let m = DrModel::default();
        let t = m.time_to_limit(1852.0).unwrap();
        assert!((m.r95(t) - 1852.0).abs() < 1.0);
        assert_eq!(times(1800.0, 600.0), vec![0.0, 600.0, 1200.0, 1800.0]);
    }
}
