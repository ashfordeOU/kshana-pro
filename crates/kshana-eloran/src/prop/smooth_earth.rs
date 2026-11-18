// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Groundwave over a homogeneous, smooth, spherical earth: the attenuation function
//! `W(d)` from the classical theory, and tabulated phase/amplitude/group-delay curves
//! ([`GroundCurve`]) for the Millington mixed-path method.
//!
//! # Conventions
//!
//! Time dependence `e^{+iωt}` (Wait's engineering convention). The vertical electric field
//! at the ground of a short vertical monopole radiating `P` over an imperfect spherical
//! earth is
//!
//! ```text
//! E(d) = E₁·√(P/1 kW) · (1 km / d) · √(θ/sin θ) · |N(kd)| · |W(d)|          (1)
//! N(kd) = 1 − i/(kd) − 1/(kd)²                                               (2)
//! ```
//!
//! with `E₁ = 300 mV/m` ([`super::ground::E1_UV_M_PER_SQRT_KW_AT_1KM`]), `k = n·ω/c` the
//! wavenumber in the (refracting, `n = 1.000338`) air, `θ = d/a_e` and `N` the induction
//! (near-field) terms of the vertical dipole (Stratton; any antenna text). The phase of
//! `N·W` is a lag relative to free-space propagation at `c/n`, so the **Secondary Factor**
//! (phase delay beyond the Primary Factor) is
//!
//! ```text
//! SF(d) = −arg[N(kd)·W(d)] / ω                                               (3)
//! ```
//!
//! # Surface impedance and numerical distance
//!
//! ```text
//! ε_c = ε_r − i·σ/(ω ε₀),    Δ = √(ε_c − 1) / ε_c      (vertical polarisation)    (4)
//! p   = −i·k·d·Δ²/2          (Sommerfeld–Norton numerical distance)             (5)
//! ```
//!
//! # Short range: flat earth plus first-order curvature (Sommerfeld–Norton–Wait)
//!
//! ```text
//! F(p) = 1 − i·√(πp)·e^{−p}·erfc(i√p) = 1 − i√(πp)·w(−√p)                    (6)
//! W ≈ F(p) + [1 − i√(πp) − (1 + 2p)·F(p)] / (4δ³) + i·(7/60)·x³,   δ³ = q³    (7)
//! ```
//!
//! (6) is Norton's flat-earth attenuation function in Wait's form (Wait 1998, eq. 13;
//! Sommerfeld 1909, Norton 1937); `w` is the Faddeeva function. The second term of (7) is
//! Wait's first-order curvature correction (Wait 1964 "Electromagnetic surface waves",
//! Advances in Radio Research 1; Wait 1998), written with Fock's parameters below; with
//! `p = i q² x` it is a power series in `√p` times `x^{3/2}`, summed directly here so it
//! stays regular at `q → 0`, where it reduces to Fock's perfectly conducting term
//! `(√π/4)·e^{−3iπ/4}·x^{3/2}`. The last term is Fock's next (second-order) term for small
//! `q`. The overall phase/sign of both terms in this `e^{+iωt}` convention was **fixed
//! numerically against the residue series (9)** (which independently tends to `F(p)` as
//! `x → 0`): with them, (7) and (9) agree to < 2e-3 in `W` up to `x = 0.3` for
//! `|q| ≤ 1.5` (a few hundredths of a dB, < 0.5 ns), see the tests.
//!
//! # Long range: the Wait/Bremmer residue (mode) series
//!
//! ```text
//! α = (k a_e / 2)^{1/3},   x = α·d/a_e,   q = −i·α·Δ                           (8)
//! W(x, q) = √(πx)·e^{−iπ/4} · Σ_s e^{−i x t_s} / (t_s − q²)                   (9)
//! w₁'(t_s) − q·w₁(t_s) = 0,   w₁(t) = √π·[Bi(t) − i·Ai(t)] = 2√π e^{−iπ/6} Ai(t e^{−2πi/3})  (10)
//! ```
//!
//! (Bremmer 1949 "Terrestrial radio waves", ch. 4; Wait 1956/1998; ITU-R P.368 and the
//! ITU GRWAVE program use the same series.) The roots `t_s` are **computed**: starting at
//! the zeros of `w₁'` (`t = |a'_s| e^{−iπ/3}`, `a'_s` from the asymptotic formula of
//! DLMF 9.9.7), each root is continued from `q = 0` to the actual `q` along the Riccati
//! equation `dt/dq = 1/(t − q²)` (Wait 1998, eq. 36) as predictor, with Newton's method on
//! (10) as corrector. The series is summed until the terms fall below 1e-13 of the sum.
//!
//! The two regimes are joined by a smooth (C¹) blend of `ln W` between `x = 0.15` and
//! `x = 0.25` (≈ 60–100 km at 100 kHz), both scaled by `1/√|q|` for `|q| > 1` (poor
//! ground), where the neglected `q`-dependence of the `x³` term of (7) is larger. A test
//! bounds the difference of the two forms over the window: < 1 ns in SF, < 0.03 dB.
//!
//! # Effective earth radius
//!
//! `a_e = k·6 371 km`, `k` configurable. The default is the LORAN calibration
//! [`super::ground::K_LORAN`] = 1.24, which reproduces the LORAN standard seawater SF (the
//! Harris polynomial of the Loran-C User Handbook, a fit to the Johler–Keiller–Walters
//! computations of NBS Circular 573) to within 15 ns from 50 to 2000 km. The ITU-R
//! P.368/GRWAVE standard atmosphere (`k = 4/3`) gives SF 5–7 % lower than Harris beyond
//! 300 km, the geometric earth (`k = 1`) 17 % higher. The PF uses the full path at `c/n`.
//!
//! # Evidence
//!
//! * VALIDATED (with the one-parameter `k` calibration above): seawater SF vs the Harris
//!   polynomial (Loran-C User Handbook COMDTPUB P16562.6, 1992, as published by Lo et al.,
//!   ILA 2009, eq. 5), see tests in [`super::ground`]; the inverse-distance field of a
//!   1 kW short monopole at short range. Internally checked: Airy zeros, Faddeeva values,
//!   residue series → flat earth `F(p)` as `x → 0`, short/long-range agreement.
//! * MODELLED: field strength beyond ~500 km (no machine-readable P.368 values were
//!   available to check against; the theory is the one P.368/GRWAVE implements), land SF,
//!   ECD.

use super::complex::{Cx, I, ONE, ZERO};
use super::special::{airy_ai, faddeeva};
use super::C;
use std::f64::consts::PI;

/// Permittivity of free space (F/m).
pub const EPS0: f64 = 8.854_187_812_8e-12;
/// Mean earth radius (m) used for the effective radius `a_e = k·R_E`.
pub const R_EARTH: f64 = 6_371_000.0;
/// Lower and upper `x` of the blend between (7) and (9) for `|q| ≤ 1`; both are scaled by
/// `1/√|q|` for `|q| > 1` (the error of (7) grows with `|q|`).
pub const X_BLEND_LO: f64 = 0.15;
pub const X_BLEND_HI: f64 = 0.25;

/// Number of modes for a 1e-13 truncation of (9) at `x_lo`: the `s`-th term decays as
/// `exp(−(√3/2)|a_s| x)`, `|a_s| ≈ (3π(4s−1)/8)^{2/3}`.
fn modes_needed(x_lo: f64) -> usize {
    let a = 30.0 / (0.866 * x_lo);
    ((a.powf(1.5) * 8.0 / (3.0 * PI) + 1.0) / 4.0).ceil() as usize + 10
}

/// Electrical constants of a homogeneous ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ground {
    /// Conductivity (S/m).
    pub sigma: f64,
    /// Relative permittivity.
    pub eps_r: f64,
}

/// Seawater as used for the LORAN SF reference (σ = 5 S/m, ε_r = 80).
pub const SEAWATER: Ground = Ground { sigma: 5.0, eps_r: 80.0 };

impl Ground {
    /// Normalised surface impedance Δ (4), vertical polarisation, at `f_hz`.
    pub fn delta(&self, f_hz: f64) -> Cx {
        let w = 2.0 * PI * f_hz;
        let ec = Cx::new(self.eps_r, -self.sigma / (w * EPS0));
        (ec - 1.0).sqrt() / ec
    }
}

/// Induction terms `N(kd)` (2).
pub fn near_field(kd: f64) -> Cx {
    Cx::new(1.0 - 1.0 / (kd * kd), -1.0 / kd)
}

/// Norton's flat-earth attenuation function `F(p)` (6).
pub fn flat_earth(p: Cx) -> Cx {
    let s = p.sqrt();
    ONE - I * (PI * p).sqrt() * faddeeva(-s)
}

/// `c_m = √π (−i)^m / Γ((m+1)/2)` for `m = 0..M`: `F(p) = 1 + Σ_{m≥1} c_m p^{m/2}`.
fn f_coeffs(m_max: usize) -> Vec<Cx> {
    let sqrt_pi = PI.sqrt();
    let mut gamma_half = vec![0.0; m_max + 3]; // Γ((m+1)/2) for m = 0..
    gamma_half[0] = sqrt_pi; // Γ(1/2)
    gamma_half[1] = 1.0; // Γ(1)
    for m in 2..gamma_half.len() {
        // Γ((m+1)/2) = ((m−1)/2)·Γ((m−1)/2)
        gamma_half[m] = (m as f64 - 1.0) / 2.0 * gamma_half[m - 2];
    }
    let mut out = Vec::with_capacity(m_max + 1);
    let mut mi = ONE;
    for g in gamma_half.iter().take(m_max + 1) {
        out.push(mi * (sqrt_pi / g));
        mi *= Cx::new(0.0, -1.0);
    }
    out
}

/// Curvature-corrected flat-earth attenuation function (7) at Fock distance `x`
/// for Fock impedance parameter `q` (`p = i q² x`).
pub fn flat_curved(x: f64, q: Cx) -> Cx {
    let p = I * q * q * x;
    let f = flat_earth(p);
    let s = p.sqrt();
    // correction = N(p)/(4q³) = −e^{i3π/4} x^{3/2}/4 · Σ_{m≥3} (c_m + 2c_{m−2}) s^{m−3}
    let corr = if s.abs() < 3.0 {
        let c = f_coeffs(90);
        let mut sum = ZERO;
        let mut sp = ONE;
        for m in 3..c.len() {
            let t = (c[m] + c[m - 2] * 2.0) * sp;
            sum += t;
            if m > 6 && t.abs() < 1e-17 * sum.abs() {
                break;
            }
            sp *= s;
        }
        -(sum * Cx::cis(3.0 * PI / 4.0) * x.powf(1.5) / 4.0)
    } else {
        let n = ONE - I * (PI * p).sqrt() - (ONE + p * 2.0) * f;
        n / (q * q * q * 4.0)
    };
    f + corr + Cx::new(0.0, 7.0 / 60.0 * x * x * x)
}

/// Roots `t_s` of the mode equation (10) for impedance `q`, `s = 1..=n`.
pub fn solve_modes(q: Cx, n: usize) -> Vec<Cx> {
    let e = Cx::cis(-2.0 * PI / 3.0);
    let rot = Cx::cis(-PI / 3.0);
    let newton = |mut t: Cx, q: Cx| -> Cx {
        for _ in 0..60 {
            let (ai, aip) = airy_ai(e * t);
            let f = e * aip - q * ai;
            let fp = t * ai - q * e * aip;
            let step = f / fp;
            t -= step;
            if step.abs() < 1e-14 * t.abs().max(1.0) {
                break;
            }
        }
        t
    };
    let mut out = Vec::with_capacity(n);
    for s in 1..=n {
        // |a'_s| ≈ U(3π(4s−3)/8), U(t) = t^{2/3}(1 − 7/48 t^{−2} + 35/288 t^{−4}) (DLMF 9.9.7)
        let tt = 3.0 * PI * (4.0 * s as f64 - 3.0) / 8.0;
        let u = tt.powf(2.0 / 3.0) * (1.0 - 7.0 / 48.0 / (tt * tt) + 35.0 / 288.0 / tt.powi(4));
        let mut t = newton(rot * u, ZERO);
        let steps = (4.0 + 24.0 * q.abs() / t.abs().sqrt()).ceil().min(200.0) as usize;
        let mut qp = ZERO;
        for j in 1..=steps {
            let qj = q * (j as f64 / steps as f64);
            // RK2 predictor on dt/dq = 1/(t − q²)
            let h = qj - qp;
            let k1 = ONE / (t - qp * qp);
            let tm = t + h * k1 * 0.5;
            let qm = qp + h * 0.5;
            let k2 = ONE / (tm - qm * qm);
            t += h * k2;
            t = newton(t, qj);
            qp = qj;
        }
        out.push(t);
    }
    out
}

/// One frequency of the smooth-earth groundwave for one ground.
#[derive(Clone, Debug)]
pub struct SmoothEarth {
    pub ground: Ground,
    pub f_hz: f64,
    /// Wavenumber in air `n·ω/c` (rad/m).
    pub k: f64,
    /// Effective earth radius (m).
    pub a_e: f64,
    pub alpha: f64,
    pub q: Cx,
    pub modes: Vec<Cx>,
    /// Blend window in `x`.
    pub x_lo: f64,
    pub x_hi: f64,
}

impl SmoothEarth {
    pub fn new(ground: Ground, f_hz: f64, k_factor: f64, n_atm: f64) -> SmoothEarth {
        let k = n_atm * 2.0 * PI * f_hz / C;
        let a_e = k_factor * R_EARTH;
        let alpha = (k * a_e / 2.0).cbrt();
        let delta = ground.delta(f_hz);
        let q = Cx::new(0.0, -alpha) * delta;
        let scale = 1.0 / q.abs().max(1.0).sqrt();
        let (x_lo, x_hi) = (X_BLEND_LO * scale, X_BLEND_HI * scale);
        let modes = solve_modes(q, modes_needed(x_lo));
        SmoothEarth { ground, f_hz, k, a_e, alpha, q, modes, x_lo, x_hi }
    }

    /// Fock distance `x` of a ground distance `d_m`.
    pub fn x_of(&self, d_m: f64) -> f64 {
        self.alpha * d_m / self.a_e
    }

    /// Residue series (9) at Fock distance `x`.
    pub fn residue(&self, x: f64) -> Cx {
        let q2 = self.q * self.q;
        let mut sum = ZERO;
        let mut small = 0;
        for &t in &self.modes {
            let term = (Cx::new(0.0, -x) * t).exp() / (t - q2);
            sum += term;
            if term.abs() < 1e-13 * sum.abs() {
                small += 1;
                if small >= 3 {
                    break;
                }
            } else {
                small = 0;
            }
        }
        sum * (PI * x).sqrt() * Cx::cis(-PI / 4.0)
    }

    /// Ground attenuation function `W(d)` (without the induction terms), joining (7) and
    /// (9) smoothly. `ln W` is blended, the caller unwraps the phase.
    pub fn w(&self, d_m: f64) -> Cx {
        let x = self.x_of(d_m);
        if x <= 0.0 {
            return ONE;
        }
        if x <= self.x_lo {
            flat_curved(x, self.q)
        } else if x >= self.x_hi {
            self.residue(x)
        } else {
            let a = flat_curved(x, self.q);
            let b = self.residue(x);
            let u = (x - self.x_lo) / (self.x_hi - self.x_lo);
            let wgt = u * u * (3.0 - 2.0 * u);
            // blend ln W with the phase difference taken on the short arc
            let la = a.ln();
            let mut lb = b.ln();
            while lb.im - la.im > PI {
                lb.im -= 2.0 * PI;
            }
            while lb.im - la.im < -PI {
                lb.im += 2.0 * PI;
            }
            (la * (1.0 - wgt) + lb * wgt).exp()
        }
    }

    /// Spherical spreading factor `√(θ/sin θ)`.
    pub fn spreading(&self, d_m: f64) -> f64 {
        let th = d_m / self.a_e;
        if th < 1e-6 {
            1.0
        } else {
            (th / th.sin()).sqrt()
        }
    }
}

/// Half-width of the band used for the group delay (Hz): the slope of the phase across
/// 90–110 kHz.
pub const GROUP_HALF_BAND_HZ: f64 = 10_000.0;

/// Tabulated groundwave curves for one ground, for the Millington method.
///
/// Stored on a grid uniform in `√d` (fine near the transmitter, where `W` behaves like
/// `√d`, coarser far out), all **without** the induction terms and spreading (which are
/// ground-independent and cancel in Millington's method): `ln|W|`, the unwrapped phase lag
/// `−arg W` at `f₀` (as a delay, s), and the group delay `−d(arg W)/dω` from the phase
/// difference at `f₀ ± 10 kHz` (s). Catmull–Rom interpolation in `√d`.
#[derive(Clone, Debug)]
pub struct GroundCurve {
    pub ground: Ground,
    pub f0_hz: f64,
    ds: f64,
    ln_amp: Vec<f64>,
    phase_delay_s: Vec<f64>,
    group_delay_s: Vec<f64>,
    /// Maximum tabulated distance (m).
    pub d_max_m: f64,
}

/// Values of a curve at one distance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CurvePoint {
    pub ln_amp: f64,
    pub phase_delay_s: f64,
    pub group_delay_s: f64,
}

impl GroundCurve {
    /// Tabulate to `d_max_m` with `n` intervals in `√d`.
    pub fn new(ground: Ground, f0_hz: f64, k_factor: f64, n_atm: f64, d_max_m: f64, n: usize) -> GroundCurve {
        let e0 = SmoothEarth::new(ground, f0_hz, k_factor, n_atm);
        let em = SmoothEarth::new(ground, f0_hz - GROUP_HALF_BAND_HZ, k_factor, n_atm);
        let ep = SmoothEarth::new(ground, f0_hz + GROUP_HALF_BAND_HZ, k_factor, n_atm);
        let s_max = d_max_m.sqrt();
        let ds = s_max / n as f64;
        let mut ln_amp = Vec::with_capacity(n + 1);
        let mut ph0 = Vec::with_capacity(n + 1);
        let mut gd = Vec::with_capacity(n + 1);
        let (mut u0, mut um, mut up) = (0.0f64, 0.0f64, 0.0f64); // unwrapped arg
        for j in 0..=n {
            let d = (j as f64 * ds).powi(2);
            let w0 = e0.w(d);
            let wm = em.w(d);
            let wp = ep.w(d);
            let unwrap = |prev: f64, z: Cx| -> f64 {
                let mut a = z.arg();
                while a - prev > PI {
                    a -= 2.0 * PI;
                }
                while a - prev < -PI {
                    a += 2.0 * PI;
                }
                a
            };
            u0 = unwrap(u0, w0);
            um = unwrap(um, wm);
            up = unwrap(up, wp);
            ln_amp.push(w0.abs().ln());
            ph0.push(-u0 / (2.0 * PI * f0_hz));
            gd.push(-(up - um) / (2.0 * PI * 2.0 * GROUP_HALF_BAND_HZ));
        }
        GroundCurve { ground, f0_hz, ds, ln_amp, phase_delay_s: ph0, group_delay_s: gd, d_max_m }
    }

    fn interp(v: &[f64], ds: f64, d_m: f64) -> f64 {
        let s = d_m.max(0.0).sqrt() / ds;
        let n = v.len() - 1;
        if s >= n as f64 {
            // linear extrapolation in √d beyond the table
            return v[n] + (v[n] - v[n - 1]) * (s - n as f64);
        }
        let i = s.floor() as usize;
        let t = s - i as f64;
        let p1 = v[i];
        let p2 = v[i + 1];
        let p0 = if i == 0 { 2.0 * p1 - p2 } else { v[i - 1] };
        let p3 = if i + 2 > n { 2.0 * p2 - p1 } else { v[i + 2] };
        // Catmull–Rom
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t * t * t)
    }

    /// Curve values at ground distance `d_m`.
    pub fn at(&self, d_m: f64) -> CurvePoint {
        CurvePoint {
            ln_amp: Self::interp(&self.ln_amp, self.ds, d_m),
            phase_delay_s: Self::interp(&self.phase_delay_s, self.ds, d_m),
            group_delay_s: Self::interp(&self.group_delay_s, self.ds, d_m),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F0: f64 = 100_000.0;
    const N_ATM: f64 = 1.000_338;

    #[test]
    fn modes_satisfy_equation_and_limits() {
        // q = 0: t_1 = |a'_1| e^{−iπ/3}, a'_1 = −1.018792972 (DLMF Table 9.9.1)
        let m = solve_modes(ZERO, 5);
        let want = Cx::cis(-PI / 3.0) * 1.018_792_971_647_471;
        assert!((m[0] - want).abs() < 1e-10, "{:?}", m[0]);
        // large q → zeros of w1: t_1 → |a_1| e^{−iπ/3} + 1/q
        let q = Cx::new(300.0, -300.0);
        let m = solve_modes(q, 3);
        let want = Cx::cis(-PI / 3.0) * 2.338_107_410_459_767 + ONE / q;
        assert!((m[0] - want).abs() < 1e-4, "{:?} vs {:?}", m[0], want);
        // general q: residual of (10)
        let e = Cx::cis(-2.0 * PI / 3.0);
        for q in [Cx::new(0.5, -0.4), Cx::new(1.1, -1.0), Cx::new(3.4, -2.0)] {
            let m = solve_modes(q, 60);
            for (i, &t) in m.iter().enumerate() {
                let (ai, aip) = airy_ai(e * t);
                let r = (e * aip - q * ai).abs() / (aip.abs() + q.abs() * ai.abs());
                assert!(r < 1e-9, "q={q:?} s={} r={r}", i + 1);
                assert!(t.im < 0.0);
                if i > 0 {
                    // neighbouring roots are ≈ π/√|t| apart
                    let gap = PI / t.abs().sqrt();
                    assert!((t - m[i - 1]).abs() > 0.5 * gap, "duplicate mode q={q:?} s={}", i + 1);
                }
            }
        }
    }

    #[test]
    fn short_and_long_range_forms_join() {
        // At the blend window the curvature-corrected flat-earth form (7) and the residue
        // series (9) must agree: |ΔSF| < 1 ns, |Δ amplitude| < 0.03 dB.
        for g in [
            SEAWATER,
            Ground { sigma: 0.03, eps_r: 15.0 },
            Ground { sigma: 0.005, eps_r: 15.0 },
            Ground { sigma: 0.001, eps_r: 10.0 },
            Ground { sigma: 0.0003, eps_r: 5.0 },
        ] {
            let e = SmoothEarth::new(g, F0, 4.0 / 3.0, N_ATM);
            for x in [e.x_lo, 0.5 * (e.x_lo + e.x_hi), e.x_hi] {
                let a = flat_curved(x, e.q);
                let b = e.residue(x);
                let dphi_ns = (b / a).arg() / (2.0 * PI * F0) * 1e9;
                let ddb = 20.0 * (b.abs() / a.abs()).log10();
                println!("{g:?} |q|={:.2} x={x}: {dphi_ns:.3} ns {ddb:.4} dB", e.q.abs());
                assert!(dphi_ns.abs() < 1.0 && ddb.abs() < 0.03, "{g:?} x={x}: {dphi_ns} ns {ddb} dB");
            }
        }
    }
}
