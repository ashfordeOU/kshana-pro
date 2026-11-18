// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The per-clock holdover model (roadmap 0.42 T1), exactly as pre-registered for V5
//! (`docs/evidence/v5-holdover-time-to-breach/2026-11-15_PREREGISTRATION.md`, section 2).
//!
//! From a phase record `x(t)` of a clock against a better reference, before the handover time
//! `t_h` only:
//!
//! 1. least squares `x = a + b v + c v^2 / 2`, `v = t - t_h`, over the last `W` seconds; residual
//!    variance and the parameter covariance `C`;
//! 2. the overlapping Allan variance of the residuals at octave averaging factors up to `N / 8`,
//!    fitted to `3 R / tau^2 + q_wf / tau + q_rw tau / 3` with non-negative terms;
//! 3. the holdover time error `TE(u) = x(t_h + u) - (a + b u)` is predicted normal with
//!    `m(u) = c u^2 / 2` and
//!    `s(u)^2 = C_aa + (C_bb + q_wf / W + q_rw W / 3) u^2 + C_cc u^4 / 4 + q_wf u + q_rw u^3 / 3 + R`.
//!
//! The future-noise term is the open engine's van Loan coast variance
//! ([`kshana::holdover::coast_phase_variance`]); the Allan variance is the open engine's
//! ([`kshana::allan::overlapping_adev`]). A clock whose record is a closed-loop servo offset
//! (linuxptp, chrony, ntpd, PPS, White Rabbit) does not show its oscillator's free-running
//! noise; for it the noise comes from a stated prior (a clock class or datasheet Allan points)
//! and the record gives only the present offset ([`HoldoverModel::from_prior`]).

use serde::Serialize;

/// Two-sided 95 % normal quantile used for the prediction interval.
pub const Z95: f64 = 1.959_963_984_540_054;

/// The noise of a clock in the three-term Allan model `3 R / tau^2 + q_wf / tau + q_rw tau / 3`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Noise {
    /// White phase noise variance (s^2).
    pub r_s2: f64,
    /// White frequency noise level (s^2/s).
    pub q_wf: f64,
    /// Random-walk frequency noise level (1/s).
    pub q_rw: f64,
}

impl Noise {
    /// The model Allan variance at `tau`.
    pub fn avar(&self, tau: f64) -> f64 {
        3.0 * self.r_s2 / (tau * tau) + self.q_wf / tau + self.q_rw * tau / 3.0
    }
}

/// Solve a small dense linear system `M p = y` by Gaussian elimination with partial pivoting.
#[allow(clippy::needless_range_loop)]
fn solve(mut m: Vec<Vec<f64>>, mut y: Vec<f64>) -> Option<Vec<f64>> {
    let n = y.len();
    for col in 0..n {
        let piv = (col..n).max_by(|&i, &j| m[i][col].abs().total_cmp(&m[j][col].abs()))?;
        if m[piv][col].abs() < 1e-300 {
            return None;
        }
        m.swap(col, piv);
        y.swap(col, piv);
        for r in col + 1..n {
            let f = m[r][col] / m[col][col];
            for k in col..n {
                m[r][k] -= f * m[col][k];
            }
            y[r] -= f * y[col];
        }
    }
    let mut p = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| m[r][k] * p[k]).sum();
        p[r] = (y[r] - s) / m[r][r];
    }
    Some(p)
}

/// Invert a small symmetric positive matrix (by solving for each unit vector).
fn invert(m: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let mut inv = vec![vec![0.0; n]; n];
    for j in 0..n {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        let col = solve(m.to_vec(), e)?;
        for i in 0..n {
            inv[i][j] = col[i];
        }
    }
    Some(inv)
}

/// Fit `3 R / tau^2 + q_wf / tau + q_rw tau / 3` to Allan variances `(tau, avar)` by least squares
/// on relative residuals, every term non-negative: each non-empty subset of the three terms is
/// solved unconstrained, the subsets with a negative term are dropped, and the lowest cost wins.
pub fn fit_noise(points: &[(f64, f64)]) -> Noise {
    fit_noise_with(points, |_| 0.0)
}

/// [`fit_noise`] with a known, fixed term `offset(tau)` added to the model (the successor
/// predictor's drift term `D^2 tau^2 / 2`).
pub fn fit_noise_with(points: &[(f64, f64)], offset: impl Fn(f64) -> f64) -> Noise {
    let pts: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|(t, v)| *t > 0.0 && v.is_finite() && *v > 0.0)
        .collect();
    if pts.is_empty() {
        return Noise::default();
    }
    let basis = |tau: f64| [3.0 / (tau * tau), 1.0 / tau, tau / 3.0];
    let cost = |p: &[f64; 3]| -> f64 {
        pts.iter()
            .map(|(t, v)| {
                let b = basis(*t);
                let model = b[0] * p[0] + b[1] * p[1] + b[2] * p[2];
                ((model + offset(*t) - v) / v).powi(2)
            })
            .sum()
    };
    let mut best = ([0.0; 3], pts.len() as f64);
    for mask in 1u8..8 {
        let idx: Vec<usize> = (0..3).filter(|i| mask & (1 << i) != 0).collect();
        if idx.len() > pts.len() {
            continue;
        }
        // Normal equations of the weighted problem: rows b_j(tau_i) / v_i, target
        // (v_i - offset_i) / v_i (exactly 1 without an offset).
        let k = idx.len();
        let mut ata = vec![vec![0.0; k]; k];
        let mut aty = vec![0.0; k];
        for (t, v) in &pts {
            let b = basis(*t);
            for (r, &i) in idx.iter().enumerate() {
                let ai = b[i] / v;
                aty[r] += ai * ((v - offset(*t)) / v);
                for (c, &j) in idx.iter().enumerate() {
                    ata[r][c] += ai * b[j] / v;
                }
            }
        }
        let Some(sol) = solve(ata, aty) else { continue };
        if sol.iter().any(|x| !x.is_finite() || *x < 0.0) {
            continue;
        }
        let mut p = [0.0; 3];
        for (r, &i) in idx.iter().enumerate() {
            p[i] = sol[r];
        }
        let c = cost(&p);
        if c < best.1 {
            best = (p, c);
        }
    }
    Noise {
        r_s2: best.0[0],
        q_wf: best.0[1],
        q_rw: best.0[2],
    }
}

/// Where a model's noise came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoiseSource {
    /// Fitted to the clock's own free-running record.
    Measured,
    /// A stated prior (clock class or datasheet points); the record is a servo offset.
    Prior,
}

/// A fitted holdover model of one clock at one handover time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HoldoverModel {
    /// Phase at the handover (s), relative to the record's own reference.
    pub phase_s: f64,
    /// Fractional frequency at the handover (the frozen state).
    pub freq: f64,
    /// Frequency drift (1/s).
    pub drift_per_s: f64,
    /// Parameter variances `C_aa` (s^2), `C_bb` (1), `C_cc` (1/s^2).
    pub var_phase: f64,
    pub var_freq: f64,
    pub var_drift: f64,
    /// Training window length (s).
    pub window_s: f64,
    /// Samples in the window.
    pub n_window: usize,
    /// RMS of the window's residuals (s).
    pub residual_rms_s: f64,
    /// The noise used for the prediction.
    pub noise: Noise,
    pub noise_source: NoiseSource,
    /// The Allan points the noise was fitted to (`tau` s, deviation); empty for a prior.
    pub adev_points: Vec<(f64, f64)>,
    /// `quadratic` (the V5 predictor), `kalman` (the successor, V5b) or `prior`.
    pub method: &'static str,
    /// The full state covariance at the handover (phase, frequency, drift), for `kalman`.
    pub cov: Option<[[f64; 3]; 3]>,
    /// The orbit-period harmonics and the flicker term of `harmonic` (V5c); absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub harmonic: Option<Harmonic>,
}

/// GPS orbital period used by the V5c harmonics, s (half a sidereal day; a constant, never fitted).
pub const GPS_ORBIT_PERIOD_S: f64 = 43_082.0;

/// The periodic part and the flicker term of a V5c model (`kshana-pro/holdover/v3`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Harmonic {
    /// Period `P`, s.
    pub period_s: f64,
    /// Time of the window's first sample relative to the handover, s (negative): the phase origin.
    pub v0_s: f64,
    /// `(alpha_1, beta_1, alpha_2, beta_2)`, s.
    pub coef: [f64; 4],
    /// Their covariance, the least-squares covariance scaled by `kappa`.
    pub cov: [[f64; 4]; 4],
    /// `N / N_eff`.
    pub kappa: f64,
    /// First lag at which the least-squares residuals' autocorrelation falls below `1/e`, s.
    pub tau_c_s: f64,
    /// The flicker floor `F` of the Allan model (dimensionless, `sigma_y^2`).
    pub flicker: f64,
}

impl Harmonic {
    fn basis(&self, v: f64) -> [f64; 4] {
        let w = 2.0 * std::f64::consts::PI / self.period_s;
        let ph = v - self.v0_s;
        [
            (w * ph).cos(),
            (w * ph).sin(),
            (2.0 * w * ph).cos(),
            (2.0 * w * ph).sin(),
        ]
    }

    /// `h` at `v` (s from the handover).
    pub fn h(&self, v: f64) -> f64 {
        let b = self.basis(v);
        (0..4).map(|i| self.coef[i] * b[i]).sum()
    }

    /// The gradient of `h(u) - h(0)` with respect to the coefficients.
    fn grad(&self, u: f64) -> [f64; 4] {
        let a = self.basis(u);
        let b = self.basis(0.0);
        [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]]
    }
}

/// [`fit_noise_with`] with the flicker floor `F` as a fourth term:
/// `sigma_y^2(tau) = 3 R / tau^2 + q_wf / tau + F + q_rw tau / 3` (+ the fixed offset), relative
/// least squares, every term non-negative, exhaustive over the subsets of the four terms.
pub fn fit_noise_flicker(points: &[(f64, f64)], offset: impl Fn(f64) -> f64) -> (Noise, f64) {
    let pts: Vec<(f64, f64)> = points
        .iter()
        .copied()
        .filter(|(t, v)| *t > 0.0 && v.is_finite() && *v > 0.0)
        .collect();
    if pts.is_empty() {
        return (Noise::default(), 0.0);
    }
    let basis = |tau: f64| [3.0 / (tau * tau), 1.0 / tau, 1.0, tau / 3.0];
    let cost = |p: &[f64; 4]| -> f64 {
        pts.iter()
            .map(|(t, v)| {
                let b = basis(*t);
                let model: f64 = (0..4).map(|i| b[i] * p[i]).sum();
                ((model + offset(*t) - v) / v).powi(2)
            })
            .sum()
    };
    let mut best = ([0.0; 4], cost(&[0.0; 4]));
    for mask in 1u8..16 {
        let idx: Vec<usize> = (0..4).filter(|i| mask & (1 << i) != 0).collect();
        if idx.len() > pts.len() {
            continue;
        }
        let k = idx.len();
        let mut ata = vec![vec![0.0; k]; k];
        let mut aty = vec![0.0; k];
        for (t, v) in &pts {
            let b = basis(*t);
            for (r, &i) in idx.iter().enumerate() {
                let ai = b[i] / v;
                aty[r] += ai * ((v - offset(*t)) / v);
                for (c, &j) in idx.iter().enumerate() {
                    ata[r][c] += ai * b[j] / v;
                }
            }
        }
        let Some(sol) = solve(ata, aty) else { continue };
        if sol.iter().any(|x| !x.is_finite() || *x < 0.0) {
            continue;
        }
        let mut p = [0.0; 4];
        for (r, &i) in idx.iter().enumerate() {
            p[i] = sol[r];
        }
        let c = cost(&p);
        if c < best.1 {
            best = (p, c);
        }
    }
    (
        Noise {
            r_s2: best.0[0],
            q_wf: best.0[1],
            q_rw: best.0[3],
        },
        best.0[2],
    )
}

/// The phase-measurement variance floor of the successor predictor: the rounding of a phase
/// printed to 1e-13 s.
pub const R_FLOOR_S2: f64 = 1e-26 / 12.0;

/// One point of a prediction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Prediction {
    pub u_s: f64,
    pub mean_s: f64,
    pub sigma_s: f64,
}

/// The predicted times to breach one limit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Breach {
    pub limit_s: f64,
    /// First grid time with breach probability >= 0.05 (`None`: beyond the horizon).
    pub t_warn_s: Option<f64>,
    /// First grid time with breach probability >= 0.5 (`None`: beyond the horizon).
    pub t_50_s: Option<f64>,
    pub horizon_s: f64,
}

/// The standard normal cumulative distribution, through the complementary error function.
pub fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

fn erfc(x: f64) -> f64 {
    // The Taylor series of erf below 2 (at most two digits lost to cancellation), and the
    // continued fraction of erfc (modified Lentz) from 2 up, where it converges quickly.
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 2.0 {
        // erf series: 2/sqrt(pi) * sum (-1)^n x^(2n+1) / (n! (2n+1))
        let mut sum: f64 = 0.0;
        let mut term = x;
        let x2 = x * x;
        let mut n = 0.0;
        while term.abs() > 1e-17 * sum.abs().max(1e-300) {
            sum += term / (2.0 * n + 1.0);
            n += 1.0;
            term *= -x2 / n;
            if n > 200.0 {
                break;
            }
        }
        return 1.0 - 2.0 / std::f64::consts::PI.sqrt() * sum;
    }
    // Continued fraction (Lentz): erfc(x) = exp(-x^2)/sqrt(pi) * 1/(x + 1/2/(x + 1/(x + 3/2/(x + ...
    let tiny = 1e-300;
    let mut f = x;
    if f == 0.0 {
        f = tiny;
    }
    let mut c = f;
    let mut d = 0.0;
    for i in 1..300 {
        let a = i as f64 / 2.0;
        d = x + a * d;
        if d == 0.0 {
            d = tiny;
        }
        c = x + a / c;
        if c == 0.0 {
            c = tiny;
        }
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x * x).exp() / (std::f64::consts::PI.sqrt() * f)
}

impl HoldoverModel {
    /// Fit the model to the phase samples `x` (s) at spacing `tau0` (s), the last sample being
    /// the handover. Uses the last `window_s` seconds (all of `x` if shorter). Needs at least 16
    /// samples in the window.
    pub fn fit(x: &[f64], tau0: f64, window_s: f64) -> Result<HoldoverModel, String> {
        if tau0.is_nan() || tau0 <= 0.0 {
            return Err("the sample interval must be positive".into());
        }
        let want = (window_s / tau0).round() as usize + 1;
        let n = want.min(x.len());
        if n < 16 {
            return Err(format!(
                "a holdover model needs at least 16 samples in its window; the record has {n}"
            ));
        }
        let w = &x[x.len() - n..];
        if w.iter().any(|v| !v.is_finite()) {
            return Err("the window has a missing or non-finite sample".into());
        }
        let span = (n - 1) as f64 * tau0;
        let ref_x = w[n - 1];
        // Scaled abscissa s = v / span in [-1, 0]; columns [1, s, s^2 / 2].
        let mut ata = vec![vec![0.0; 3]; 3];
        let mut aty = vec![0.0; 3];
        for (i, xi) in w.iter().enumerate() {
            let s = (i as f64 - (n - 1) as f64) / (n - 1) as f64;
            let row = [1.0, s, s * s / 2.0];
            for r in 0..3 {
                aty[r] += row[r] * (xi - ref_x);
                for c in 0..3 {
                    ata[r][c] += row[r] * row[c];
                }
            }
        }
        let p = solve(ata.clone(), aty).ok_or("the window's fit is singular")?;
        let inv = invert(&ata).ok_or("the window's fit is singular")?;
        let mut resid = Vec::with_capacity(n);
        for (i, xi) in w.iter().enumerate() {
            let s = (i as f64 - (n - 1) as f64) / (n - 1) as f64;
            resid.push((xi - ref_x) - (p[0] + p[1] * s + p[2] * s * s / 2.0));
        }
        let sr2 = resid.iter().map(|r| r * r).sum::<f64>() / (n as f64 - 3.0);
        let mut adev_points = Vec::new();
        let mut m = 1usize;
        while m <= n / 8 && n > 2 * m {
            let dev = kshana::allan::overlapping_adev(&resid, tau0, m);
            adev_points.push((m as f64 * tau0, dev));
            m *= 2;
        }
        let noise = fit_noise(
            &adev_points
                .iter()
                .map(|(t, d)| (*t, d * d))
                .collect::<Vec<_>>(),
        );
        Ok(HoldoverModel {
            phase_s: ref_x + p[0],
            freq: p[1] / span,
            drift_per_s: p[2] / (span * span),
            var_phase: sr2 * inv[0][0],
            var_freq: sr2 * inv[1][1] / (span * span),
            var_drift: sr2 * inv[2][2] / span.powi(4),
            window_s: span,
            n_window: n,
            residual_rms_s: sr2.sqrt(),
            noise,
            noise_source: NoiseSource::Measured,
            adev_points,
            method: "quadratic",
            cov: None,
            harmonic: None,
        })
    }

    /// The successor predictor (V5b pre-registration, section 2): the drift from the window's
    /// quadratic, the noise from the Allan variance of the raw window phase with the drift term
    /// held fixed, and the handover state and its full covariance from the open engine's
    /// three-state clock Kalman filter run over the window.
    pub fn fit_kalman(x: &[f64], tau0: f64, window_s: f64) -> Result<HoldoverModel, String> {
        let quad = HoldoverModel::fit(x, tau0, window_s)?;
        let n = quad.n_window;
        let w = &x[x.len() - n..];
        let x0 = w[0];
        let rel: Vec<f64> = w.iter().map(|v| v - x0).collect();
        let d = quad.drift_per_s;
        let mut pts = Vec::new();
        let mut m = 1usize;
        while m <= n / 8 && n > 2 * m {
            let dev = kshana::allan::overlapping_adev(&rel, tau0, m);
            pts.push((m as f64 * tau0, dev));
            m *= 2;
        }
        let avar: Vec<(f64, f64)> = pts.iter().map(|(t, a)| (*t, a * a)).collect();
        let fitted = fit_noise_with(&avar, |tau| d * d * tau * tau / 2.0);
        let r_eff = fitted.r_s2.max(R_FLOOR_S2);
        // Start: a line over the first quarter.
        let q = (n / 4).max(2);
        let (mut st, mut stt, mut sx, mut stx) = (0.0, 0.0, 0.0, 0.0);
        for (i, v) in rel[..q].iter().enumerate() {
            let t = i as f64 * tau0;
            st += t;
            stt += t * t;
            sx += v;
            stx += t * v;
        }
        let qf = q as f64;
        let den = qf * stt - st * st;
        let y0 = if den != 0.0 {
            (qf * stx - st * sx) / den
        } else {
            0.0
        };
        let b0 = (sx - y0 * st) / qf;
        let sdq = (rel[..q]
            .iter()
            .enumerate()
            .map(|(i, v)| (v - b0 - y0 * i as f64 * tau0).powi(2))
            .sum::<f64>()
            / (qf - 2.0).max(1.0))
        .sqrt();
        let s_x = 10.0 * sdq + r_eff.sqrt();
        let mut kf = kshana::clock_state::ClockState3::new(fitted.q_wf, fitted.q_rw, 0.0)
            .with_initial_cov(s_x * s_x, 1e-16, 1e-24);
        kf.x = [0.0, y0, 0.0];
        kf.update_phase(rel[0], r_eff);
        for z in &rel[1..] {
            kf.predict(tau0);
            kf.update_phase(*z, r_eff);
        }
        let p = kf.covariance();
        Ok(HoldoverModel {
            phase_s: x0 + kf.x[0],
            freq: kf.x[1],
            drift_per_s: kf.x[2],
            var_phase: p[0][0],
            var_freq: p[1][1],
            var_drift: p[2][2],
            window_s: quad.window_s,
            n_window: n,
            residual_rms_s: quad.residual_rms_s,
            noise: Noise {
                r_s2: r_eff,
                ..fitted
            },
            noise_source: NoiseSource::Measured,
            adev_points: pts,
            method: "kalman",
            cov: Some(p),
            harmonic: None,
        })
    }

    /// The V5c predictor (`kshana-pro/holdover/v3`, pre-registration
    /// `2026-11-16_V5c_PREREGISTRATION.md`, section 2): V5b's Kalman handover run on the phase
    /// with its orbit-period harmonics removed, a flicker floor in the Allan model, and the
    /// harmonics carried into the prediction. The `F u^2` term is the `x_rms(u) ~ u sigma_y(u)`
    /// approximation for flicker FM (an approximation).
    pub fn fit_harmonic(x: &[f64], tau0: f64, window_s: f64) -> Result<HoldoverModel, String> {
        if tau0.is_nan() || tau0 <= 0.0 {
            return Err("the sample interval must be positive".into());
        }
        let want = (window_s / tau0).round() as usize + 1;
        let n = want.min(x.len());
        if n < 16 {
            return Err(format!(
                "a holdover model needs at least 16 samples in its window; the record has {n}"
            ));
        }
        let w = &x[x.len() - n..];
        if w.iter().any(|v| !v.is_finite()) {
            return Err("the window has a missing or non-finite sample".into());
        }
        let span = (n - 1) as f64 * tau0;
        let v0 = -span;
        let p_s = GPS_ORBIT_PERIOD_S;
        let om = 2.0 * std::f64::consts::PI / p_s;
        let ref_x = w[n - 1];
        // Step 1: least squares of [1, s, s^2/2, cos, sin, cos2, sin2], s = v / span.
        let row = |i: usize| -> [f64; 7] {
            let v = (i as f64 - (n - 1) as f64) * tau0;
            let s = v / span;
            let ph = v - v0;
            [
                1.0,
                s,
                s * s / 2.0,
                (om * ph).cos(),
                (om * ph).sin(),
                (2.0 * om * ph).cos(),
                (2.0 * om * ph).sin(),
            ]
        };
        let mut ata = vec![vec![0.0; 7]; 7];
        let mut aty = vec![0.0; 7];
        for (i, xi) in w.iter().enumerate() {
            let r = row(i);
            for a in 0..7 {
                aty[a] += r[a] * (xi - ref_x);
                for b in 0..7 {
                    ata[a][b] += r[a] * r[b];
                }
            }
        }
        let p = solve(ata.clone(), aty).ok_or("the window's harmonic fit is singular")?;
        let inv = invert(&ata).ok_or("the window's harmonic fit is singular")?;
        let resid: Vec<f64> = w
            .iter()
            .enumerate()
            .map(|(i, xi)| {
                let r = row(i);
                (xi - ref_x) - (0..7).map(|a| r[a] * p[a]).sum::<f64>()
            })
            .collect();
        let sr2 = resid.iter().map(|r| r * r).sum::<f64>() / (n as f64 - 7.0);
        // tau_c: the first lag at which the residuals' autocorrelation falls below 1/e.
        let mean_r = resid.iter().sum::<f64>() / n as f64;
        let c0: f64 = resid.iter().map(|r| (r - mean_r).powi(2)).sum();
        let mut lag = 1usize;
        while lag < n {
            let c: f64 = (0..n - lag)
                .map(|i| (resid[i] - mean_r) * (resid[i + lag] - mean_r))
                .sum();
            if c0 <= 0.0 || c / c0 < (-1.0f64).exp() {
                break;
            }
            lag += 1;
        }
        let tau_c = (lag as f64 * tau0).max(tau0);
        let n_eff = (span / tau_c).max(1.0);
        let kappa = n as f64 / n_eff;
        let mut cov = [[0.0; 4]; 4];
        for (a, row_c) in cov.iter_mut().enumerate() {
            for (b, c) in row_c.iter_mut().enumerate() {
                *c = kappa * sr2 * inv[3 + a][3 + b];
            }
        }
        let harm = Harmonic {
            period_s: p_s,
            v0_s: v0,
            coef: [p[3], p[4], p[5], p[6]],
            cov,
            kappa,
            tau_c_s: tau_c,
            flicker: 0.0,
        };
        // Steps 2 and 3 of V5b on x - h.
        let xd: Vec<f64> = w
            .iter()
            .enumerate()
            .map(|(i, xi)| xi - harm.h((i as f64 - (n - 1) as f64) * tau0))
            .collect();
        let quad = HoldoverModel::fit(&xd, tau0, window_s)?;
        let x0 = xd[0];
        let rel: Vec<f64> = xd.iter().map(|v| v - x0).collect();
        let d = quad.drift_per_s;
        let mut pts = Vec::new();
        let mut m = 1usize;
        while m <= n / 8 && n > 2 * m {
            let dev = kshana::allan::overlapping_adev(&rel, tau0, m);
            pts.push((m as f64 * tau0, dev));
            m *= 2;
        }
        let avar: Vec<(f64, f64)> = pts.iter().map(|(t, a)| (*t, a * a)).collect();
        let (fitted, flicker) = fit_noise_flicker(&avar, |tau| d * d * tau * tau / 2.0);
        let r_eff = fitted.r_s2.max(R_FLOOR_S2);
        let q = (n / 4).max(2);
        let (mut st, mut stt, mut sx, mut stx) = (0.0, 0.0, 0.0, 0.0);
        for (i, v) in rel[..q].iter().enumerate() {
            let t = i as f64 * tau0;
            st += t;
            stt += t * t;
            sx += v;
            stx += t * v;
        }
        let qf = q as f64;
        let den = qf * stt - st * st;
        let y0 = if den != 0.0 {
            (qf * stx - st * sx) / den
        } else {
            0.0
        };
        let b0 = (sx - y0 * st) / qf;
        let sdq = (rel[..q]
            .iter()
            .enumerate()
            .map(|(i, v)| (v - b0 - y0 * i as f64 * tau0).powi(2))
            .sum::<f64>()
            / (qf - 2.0).max(1.0))
        .sqrt();
        let s_x = 10.0 * sdq + r_eff.sqrt();
        let mut kf = kshana::clock_state::ClockState3::new(fitted.q_wf, fitted.q_rw, 0.0)
            .with_initial_cov(s_x * s_x, 1e-16, 1e-24);
        kf.x = [0.0, y0, 0.0];
        kf.update_phase(rel[0], r_eff);
        for z in &rel[1..] {
            kf.predict(tau0);
            kf.update_phase(*z, r_eff);
        }
        let pc = kf.covariance();
        Ok(HoldoverModel {
            // The frozen state: the detrended phase estimate plus h at the handover.
            phase_s: x0 + kf.x[0] + harm.h(0.0),
            freq: kf.x[1],
            drift_per_s: kf.x[2],
            var_phase: pc[0][0],
            var_freq: pc[1][1],
            var_drift: pc[2][2],
            window_s: quad.window_s,
            n_window: n,
            residual_rms_s: quad.residual_rms_s,
            noise: Noise {
                r_s2: r_eff,
                ..fitted
            },
            noise_source: NoiseSource::Measured,
            adev_points: pts,
            method: "harmonic",
            cov: Some(pc),
            harmonic: Some(Harmonic { flicker, ..harm }),
        })
    }

    /// A model from a stated prior noise, for a clock whose record is a closed-loop servo offset:
    /// the present offset is the mean of the last `window_s` of the record and its uncertainty
    /// the variance of that mean; frequency and drift are taken as zero (the servo removes
    /// them), the frequency error at handover comes from the prior, and no drift is predicted.
    pub fn from_prior(x: &[f64], tau0: f64, window_s: f64, noise: Noise) -> Result<Self, String> {
        let want = (window_s / tau0).round() as usize + 1;
        let n = want.min(x.len());
        if n < 2 {
            return Err("a servo record needs at least two samples".into());
        }
        let w = &x[x.len() - n..];
        if w.iter().any(|v| !v.is_finite()) {
            return Err("the window has a missing or non-finite sample".into());
        }
        let mean = w.iter().sum::<f64>() / n as f64;
        let var = w.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64 - 1.0);
        Ok(HoldoverModel {
            phase_s: mean,
            freq: 0.0,
            drift_per_s: 0.0,
            var_phase: var,
            var_freq: 0.0,
            var_drift: 0.0,
            window_s: (n - 1) as f64 * tau0,
            n_window: n,
            residual_rms_s: var.sqrt(),
            noise: Noise {
                r_s2: noise.r_s2.max(var),
                ..noise
            },
            noise_source: NoiseSource::Prior,
            adev_points: Vec::new(),
            method: "prior",
            cov: None,
            harmonic: None,
        })
    }

    /// The predicted mean holdover time error after `u` s (relative to the frozen state).
    pub fn mean(&self, u: f64) -> f64 {
        if let Some(h) = &self.harmonic {
            return self.drift_per_s * u * u / 2.0 + h.h(u) - h.h(0.0);
        }
        match self.noise_source {
            NoiseSource::Measured => self.drift_per_s * u * u / 2.0,
            // A servo record: the present offset carries on.
            NoiseSource::Prior => self.phase_s,
        }
    }

    /// The predicted 1-sigma of the holdover time error after `u` s.
    pub fn sigma(&self, u: f64) -> f64 {
        if let Some(p) = &self.cov {
            let g = [1.0, u, u * u / 2.0];
            let mut gpg = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    gpg += g[i] * p[i][j] * g[j];
                }
            }
            let nz = &self.noise;
            let mut v =
                gpg + kshana::holdover::coast_phase_variance(nz.q_wf, nz.q_rw, 0.0, u) + nz.r_s2;
            if let Some(h) = &self.harmonic {
                let g = h.grad(u);
                for i in 0..4 {
                    for j in 0..4 {
                        v += g[i] * h.cov[i][j] * g[j];
                    }
                }
                v += h.flicker * u * u;
            }
            return v.max(0.0).sqrt();
        }
        let w = self.window_s.max(f64::MIN_POSITIVE);
        let nz = &self.noise;
        let var_b = self.var_freq + nz.q_wf / w + nz.q_rw * w / 3.0;
        let v = self.var_phase
            + var_b * u * u
            + self.var_drift * u.powi(4) / 4.0
            + kshana::holdover::coast_phase_variance(nz.q_wf, nz.q_rw, 0.0, u)
            + nz.r_s2;
        v.max(0.0).sqrt()
    }

    pub fn predict(&self, u: f64) -> Prediction {
        Prediction {
            u_s: u,
            mean_s: self.mean(u),
            sigma_s: self.sigma(u),
        }
    }

    /// `P(|TE(u)| >= limit)`.
    pub fn breach_probability(&self, u: f64, limit_s: f64) -> f64 {
        let m = self.mean(u);
        let s = self.sigma(u);
        if s <= 0.0 {
            return if m.abs() >= limit_s { 1.0 } else { 0.0 };
        }
        norm_cdf((-limit_s - m) / s) + (1.0 - norm_cdf((limit_s - m) / s))
    }

    /// The warning and median times to breach `limit_s`, searched on the grid `k step`,
    /// `k = 1 .. horizon / step`.
    pub fn time_to_breach(&self, limit_s: f64, step_s: f64, horizon_s: f64) -> Breach {
        let k_max = (horizon_s / step_s).round().max(1.0) as usize;
        let mut t_warn = None;
        let mut t_50 = None;
        for k in 1..=k_max {
            let u = k as f64 * step_s;
            let p = self.breach_probability(u, limit_s);
            if t_warn.is_none() && p >= 0.05 {
                t_warn = Some(u);
            }
            if p >= 0.5 {
                t_50 = Some(u);
                break;
            }
        }
        Breach {
            limit_s,
            t_warn_s: t_warn,
            t_50_s: t_50,
            horizon_s,
        }
    }
}

/// The noise of a stated clock class (the open engine's representative white-FM and random-walk
/// FM levels; the random-run term is left out).
pub fn class_noise(name: &str) -> Option<Noise> {
    use kshana::clock_state::ClockClass;
    let class = match name.to_ascii_lowercase().as_str() {
        "tcxo" => ClockClass::Tcxo,
        "ocxo" => ClockClass::Ocxo,
        "rafs" | "rubidium" | "rb" => ClockClass::Rafs,
        "csac" => ClockClass::Csac,
        "uso" => ClockClass::Uso,
        "dsac" => ClockClass::Dsac,
        _ => return None,
    };
    let (q_wf, q_rw, _) = class.psds();
    Some(Noise {
        r_s2: 0.0,
        q_wf,
        q_rw,
    })
}

/// Class names [`class_noise`] accepts.
pub const CLASS_NAMES: [&str; 6] = ["tcxo", "ocxo", "rafs", "csac", "uso", "dsac"];

/// A small deterministic generator (SplitMix64 with Box-Muller normals), the crate's own, so a
/// synthetic data set is the same on every platform and every dependency version.
#[derive(Clone, Debug)]
pub struct SplitMix {
    state: u64,
    spare: Option<f64>,
}

impl SplitMix {
    pub fn new(seed: u64) -> Self {
        SplitMix {
            state: seed,
            spare: None,
        }
    }
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in (0, 1).
    pub fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.uniform()
    }
    /// Standard normal.
    pub fn normal(&mut self) -> f64 {
        if let Some(s) = self.spare.take() {
            return s;
        }
        let u1 = self.uniform();
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let th = 2.0 * std::f64::consts::PI * u2;
        self.spare = Some(r * th.sin());
        r * th.cos()
    }
}

/// A synthetic free-running clock: white FM `q_wf`, random-walk FM `q_rw` (exact van Loan
/// discretisation), a deterministic drift, an initial frequency offset and white phase noise of
/// standard deviation `pm_s`. Returns `n` phase samples at `tau0`. SYNTHETIC: not a measurement.
#[allow(clippy::too_many_arguments)]
pub fn synth_clock(
    rng: &mut SplitMix,
    n: usize,
    tau0: f64,
    q_wf: f64,
    q_rw: f64,
    drift_per_s: f64,
    freq0: f64,
    pm_s: f64,
) -> Vec<f64> {
    // Process covariance over tau0 for state (x, y): [[q_wf t + q_rw t^3/3, q_rw t^2/2],
    // [q_rw t^2/2, q_rw t]].
    let t = tau0;
    let pxx = q_wf * t + q_rw * t.powi(3) / 3.0;
    let pxy = q_rw * t * t / 2.0;
    let pyy = q_rw * t;
    let l11 = pxx.sqrt();
    let l21 = if l11 > 0.0 { pxy / l11 } else { 0.0 };
    let l22 = (pyy - l21 * l21).max(0.0).sqrt();
    let mut x = 0.0;
    let mut y = freq0;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(x + pm_s * rng.normal());
        let e1 = rng.normal();
        let e2 = rng.normal();
        x += y * t + drift_per_s * t * t / 2.0 + l11 * e1;
        y += drift_per_s * t + l21 * e1 + l22 * e2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_cdf_matches_known_values() {
        assert!((norm_cdf(0.0) - 0.5).abs() < 1e-15);
        assert!((norm_cdf(1.959_963_984_540_054) - 0.975).abs() < 1e-12);
        assert!((norm_cdf(-3.0) - 0.001_349_898_031_630_094_5).abs() < 1e-14);
        assert!((norm_cdf(0.3) - 0.617_911_422_188_952_7).abs() < 1e-14);
        assert!(norm_cdf(-40.0) >= 0.0 && norm_cdf(-40.0) < 1e-300);
    }

    #[test]
    fn noise_fit_recovers_its_own_model() {
        let truth = Noise {
            r_s2: 1e-20,
            q_wf: 4e-22,
            q_rw: 1e-30,
        };
        let pts: Vec<(f64, f64)> = (0..12)
            .map(|k| {
                let tau = 10.0 * 2f64.powi(k);
                (tau, truth.avar(tau))
            })
            .collect();
        let got = fit_noise(&pts);
        assert!((got.r_s2 / truth.r_s2 - 1.0).abs() < 1e-6);
        assert!((got.q_wf / truth.q_wf - 1.0).abs() < 1e-6);
        assert!((got.q_rw / truth.q_rw - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_pure_quadratic_is_recovered_and_predicts_its_drift() {
        let tau0 = 10.0;
        let x: Vec<f64> = (0..2000)
            .map(|i| {
                let t = i as f64 * tau0;
                1e-6 + 2e-9 * t + 0.5 * 3e-14 * t * t
            })
            .collect();
        let m = HoldoverModel::fit(&x, tau0, 1e9).unwrap();
        let t_end = 1999.0 * tau0;
        assert!((m.freq - (2e-9 + 3e-14 * t_end)).abs() < 1e-15);
        assert!((m.drift_per_s - 3e-14).abs() < 1e-19);
        assert!((m.mean(3600.0) - 0.5 * 3e-14 * 3600.0 * 3600.0).abs() < 1e-12);
    }

    #[test]
    fn breach_times_are_ordered_and_grow_with_the_limit() {
        let mut rng = SplitMix::new(1);
        let x = synth_clock(&mut rng, 20_000, 10.0, 2.5e-23, 7.5e-27, 1e-16, 0.0, 1e-9);
        let m = HoldoverModel::fit(&x, 10.0, 48.0 * 3600.0).unwrap();
        let a = m.time_to_breach(100e-9, 10.0, 86_400.0);
        let b = m.time_to_breach(1.1e-6, 10.0, 86_400.0);
        let (wa, fa) = (a.t_warn_s.unwrap(), a.t_50_s.unwrap());
        assert!(wa <= fa);
        assert!(b.t_warn_s.unwrap_or(f64::INFINITY) >= wa);
    }

    #[test]
    fn the_kalman_state_tracks_a_random_walk_clock() {
        let mut rng = SplitMix::new(3);
        let (q_wf, q_rw) = (2.5e-23, 7.5e-27);
        let x = synth_clock(&mut rng, 20_000, 10.0, q_wf, q_rw, 0.0, 1e-10, 1e-9);
        let m = HoldoverModel::fit_kalman(&x, 10.0, 48.0 * 3600.0).unwrap();
        assert_eq!(m.method, "kalman");
        // The handover phase is within a few sigma of the last sample, unlike a global fit.
        let last = *x.last().unwrap();
        assert!(
            (m.phase_s - last).abs() < 5.0 * m.sigma(0.0) + 5e-9,
            "{} {}",
            m.phase_s,
            last
        );
        assert!(m.sigma(3600.0) > m.sigma(60.0));
    }

    #[test]
    fn the_generator_is_fixed() {
        let mut r = SplitMix::new(20261115);
        let v: Vec<u64> = (0..3).map(|_| r.next_u64()).collect();
        let mut r2 = SplitMix::new(20261115);
        assert_eq!(v, (0..3).map(|_| r2.next_u64()).collect::<Vec<_>>());
        let mean: f64 = (0..20_000).map(|_| r.normal()).sum::<f64>() / 20_000.0;
        assert!(mean.abs() < 0.03);
    }
}
