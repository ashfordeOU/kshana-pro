// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Special functions of complex argument for the groundwave theory: the Airy functions
//! `Ai(z)`, `Ai'(z)` and the Faddeeva function `w(z) = e^{−z²} erfc(−iz)`.
//!
//! * **Airy**: Maclaurin series (DLMF 9.4.1, `Ai = c₁f − c₂g`) for `|z| < 6.5`; for larger
//!   `|z|` the Poincaré expansions DLMF 9.7.5–9.7.6 in `|arg z| < 2π/3` and the
//!   oscillatory (connection) forms DLMF 9.7.9–9.7.10 near the negative real axis, each
//!   summed to its smallest term. Relative accuracy ≈ 1e-9 or better in the sector
//!   `|arg z| ≥ π/3`, the only one the mode equation evaluates (ζ = t·e^{−2πi/3} lies near
//!   the negative real axis); in `|arg z| < π/3, |z| < 6.5` the series cancels and loses
//!   up to `e^{(4/3)|z|^{3/2}}` relative accuracy, so it must not be used there (tested
//!   against the published zeros `a₁ = −2.338107410`, `a'₁ = −1.018792972` and
//!   `Ai(0)`, `Ai'(0)`).
//! * **Faddeeva** (upper half-plane only, which is all the flat-earth attenuation function
//!   needs): for `|z| < 2`, and for `|z| < 4` within 45° of the real axis, from the
//!   Maclaurin series of `erf` evaluated at `iz` (for the arguments that arise, `z² = p` the
//!   numerical distance lies in the fourth quadrant near the real axis, where that series
//!   has no destructive cancellation); elsewhere from Laplace's continued fraction `w(z) = (i/√π)/(z − ½/(z − 1/(z − 3/2/(z − …))))`
//!   (Abramowitz & Stegun 7.1.15, Gautschi 1970).

use super::complex::{Cx, I, ONE, ZERO};
use std::f64::consts::PI;

/// `Ai(0)` and `−Ai'(0)` (DLMF 9.2.3, 9.2.4).
const AI0: f64 = 0.355_028_053_887_817_2;
const MAI0P: f64 = 0.258_819_403_792_806_8;
const SQRT_PI: f64 = 1.772_453_850_905_516;

/// `(Ai(z), Ai'(z))` for complex `z`.
pub fn airy_ai(z: Cx) -> (Cx, Cx) {
    if z.abs() < 6.5 {
        airy_series(z)
    } else {
        airy_asymptotic(z)
    }
}

fn airy_series(z: Cx) -> (Cx, Cx) {
    let z3 = z * z * z;
    // f = Σ a_k, g = Σ b_k and their derivatives (recurrences derived from DLMF 9.4.1).
    let mut a = ONE;
    let mut b = z;
    let mut fp_term = z * z / 2.0; // f' term k = 1
    let mut gp_term = ONE; // g' term k = 0
    let mut f = ONE;
    let mut g = z;
    let mut fp = fp_term;
    let mut gp = gp_term;
    for k in 1..200 {
        let kf = k as f64;
        a = a * z3 / ((3.0 * kf - 1.0) * (3.0 * kf));
        b = b * z3 / ((3.0 * kf) * (3.0 * kf + 1.0));
        gp_term = gp_term * z3 / ((3.0 * kf - 2.0) * (3.0 * kf));
        f += a;
        g += b;
        gp += gp_term;
        if k >= 2 {
            fp_term = fp_term * z3 / ((3.0 * kf - 3.0) * (3.0 * kf - 1.0));
            fp += fp_term;
        }
        let m = a.abs() + b.abs() + fp_term.abs() + gp_term.abs();
        if m < 1e-17 * (f.abs() + g.abs() + fp.abs() + gp.abs()) {
            break;
        }
    }
    (f * AI0 - g * MAI0P, fp * AI0 - gp * MAI0P)
}

/// `u_k` and `v_k` of DLMF 9.7.2 (u₀ = v₀ = 1).
fn uv(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut u = vec![1.0];
    let mut v = vec![1.0];
    for k in 1..n {
        let kf = k as f64;
        let uk = u[k - 1] * (6.0 * kf - 5.0) * (6.0 * kf - 3.0) * (6.0 * kf - 1.0)
            / ((2.0 * kf - 1.0) * 216.0 * kf);
        u.push(uk);
        v.push(-(6.0 * kf + 1.0) / (6.0 * kf - 1.0) * uk);
    }
    (u, v)
}

fn airy_asymptotic(z: Cx) -> (Cx, Cx) {
    let (u, v) = uv(40);
    if z.arg().abs() < 2.0 * PI / 3.0 {
        // DLMF 9.7.5, 9.7.6
        let xi = z.powf(1.5) * (2.0 / 3.0);
        let ixi = xi.recip();
        let (mut su, mut sv) = (ZERO, ZERO);
        let mut p = ONE;
        let mut last = f64::INFINITY;
        for k in 0..u.len() {
            let tu = p * u[k];
            let tv = p * v[k];
            let m = tu.abs();
            if m > last {
                break;
            }
            su += tu;
            sv += tv;
            last = m;
            if m < 1e-17 {
                break;
            }
            p = -(p * ixi);
        }
        let e = (-xi).exp() / (2.0 * SQRT_PI);
        let z14 = z.powf(0.25);
        (e / z14 * su, -(e * z14) * sv)
    } else {
        // DLMF 9.7.9, 9.7.10 with w = −z, |arg w| ≤ π/3
        let w = -z;
        let xi = w.powf(1.5) * (2.0 / 3.0);
        let ixi = xi.recip();
        let (mut pp, mut qq, mut rr, mut ss) = (ZERO, ZERO, ZERO, ZERO);
        let mut p = ONE; // ξ^{-k}
        let mut last = f64::INFINITY;
        for k in 0..u.len() {
            let tu = p * u[k];
            let tv = p * v[k];
            let m = tu.abs();
            if m > last {
                break;
            }
            last = m;
            // sign (−1)^{⌊k/2⌋}
            let sgn = if (k / 2) % 2 == 0 { 1.0 } else { -1.0 };
            if k % 2 == 0 {
                pp += tu * sgn;
                rr += tv * sgn;
            } else {
                qq += tu * sgn;
                ss += tv * sgn;
            }
            if m < 1e-17 {
                break;
            }
            p *= ixi;
        }
        let ph = xi - PI / 4.0;
        // complex cos/sin
        let eip = (I * ph).exp();
        let emip = (-(I * ph)).exp();
        let c = (eip + emip) / 2.0;
        let s = (eip - emip) / Cx::new(0.0, 2.0);
        let w14 = w.powf(0.25);
        let ai = (c * pp + s * qq) / (w14 * SQRT_PI);
        let aip = w14 * (s * rr - c * ss) / SQRT_PI;
        (ai, aip)
    }
}

/// Faddeeva function `w(z) = e^{−z²} erfc(−iz)` for `Im z ≥ 0`.
pub fn faddeeva(z: Cx) -> Cx {
    let r = z.abs();
    if r < 2.0 || (r < 4.0 && z.im <= z.re.abs()) {
        // erf(u) = (2/√π) Σ (−1)^n u^{2n+1} / (n!(2n+1)), u = iz
        let u = I * z;
        let u2 = u * u;
        let mut term = u; // (−1)^n u^{2n+1}/n!
        let mut sum = u;
        for n in 1..400 {
            let nf = n as f64;
            term = -(term * u2) / nf;
            let t = term / (2.0 * nf + 1.0);
            sum += t;
            if t.abs() < 1e-17 * sum.abs().max(1e-300) {
                break;
            }
        }
        let erf = sum * (2.0 / SQRT_PI);
        (-(z * z)).exp() * (ONE + erf)
    } else {
        let n = 200;
        let mut k = ZERO;
        for j in (1..=n).rev() {
            k = Cx::real(j as f64 / 2.0) / (z - k);
        }
        Cx::new(0.0, 1.0 / SQRT_PI) / (z - k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn airy_reference_values() {
        let (a, ap) = airy_ai(Cx::real(0.0));
        assert!((a.re - AI0).abs() < 1e-15 && (ap.re + MAI0P).abs() < 1e-15);
        // DLMF Table 9.9.1: a1 = −2.338107410459767, a'1 = −1.018792971647471,
        // a10 = −12.828776752865757, a'10 = −12.38478837
        assert!(airy_ai(Cx::real(-2.338_107_410_459_767)).0.abs() < 1e-13);
        assert!(airy_ai(Cx::real(-1.018_792_971_647_471)).1.abs() < 1e-13);
        let (a10, _) = airy_ai(Cx::real(-12.828_776_752_865_757));
        assert!(a10.abs() < 1e-10, "{a10:?}");
        // Ai(1) = 0.1352924163128814, Ai'(1) = −0.1591474412967932 (A&S table 10.11)
        let (a1, a1p) = airy_ai(Cx::real(1.0));
        assert!((a1.re - 0.135_292_416_312_881_4).abs() < 1e-14);
        assert!((a1p.re + 0.159_147_441_296_793_2).abs() < 1e-14);
    }

    #[test]
    fn airy_series_and_asymptotic_agree_at_switch() {
        // the sector |arg z| ≥ π/3 is the one the mode equation uses (ζ near the negative
        // real axis); in |arg z| < π/3 the Maclaurin series cancels like e^{(4/3)|z|^{3/2}}
        for k in 0..24 {
            let th = -PI + (k as f64 + 0.5) * (2.0 * PI / 24.0);
            if th.abs() < PI / 3.0 {
                continue;
            }
            for r in [6.5, 7.0] {
                let z = Cx::polar(r, th);
                let (a, ap) = airy_series(z);
                let (b, bp) = airy_asymptotic(z);
                assert!((a - b).abs() < 2e-8 * a.abs().max(1e-3), "Ai r={r} th={th}: {a:?} {b:?}");
                assert!((ap - bp).abs() < 2e-8 * ap.abs().max(1e-3), "Ai' r={r} th={th}");
            }
        }
    }

    #[test]
    fn airy_satisfies_wronskian_like_identity() {
        // Ai'' = z·Ai: check by central differences off the real axis
        let z = Cx::new(-3.0, -2.5);
        let h = 1e-4;
        let (_, a1) = airy_ai(z + h);
        let (_, a0) = airy_ai(z - h);
        let (a, _) = airy_ai(z);
        let d2 = (a1 - a0) / (2.0 * h);
        assert!((d2 - z * a).abs() < 1e-7 * (z * a).abs());
    }

    #[test]
    fn faddeeva_reference_values() {
        assert!((faddeeva(ZERO) - ONE).abs() < 1e-15);
        // w(x) on the real axis: Re = e^{−x²}, Im = (2/√π)·Dawson(x); D(1) = 0.5380795069127684
        let w1 = faddeeva(Cx::real(1.0));
        assert!((w1.re - (-1.0f64).exp()).abs() < 1e-14);
        assert!((w1.im - 2.0 / SQRT_PI * 0.538_079_506_912_768_4).abs() < 1e-14);
        // w(iy) = e^{y²} erfc(y): erfc(1)·e = 0.4275835761558070
        let wi = faddeeva(Cx::new(0.0, 1.0));
        assert!((wi.re - 0.427_583_576_155_807).abs() < 1e-14 && wi.im.abs() < 1e-14);
        // continuity of the two branches across |z| = 4 in the upper half-plane
        for k in 0..=12 {
            let th = PI * k as f64 / 12.0;
            let a = faddeeva(Cx::polar(4.0 - 1e-10, th));
            let b = faddeeva(Cx::polar(4.0 + 1e-10, th));
            assert!((a - b).abs() < 1e-6 * a.abs().max(1e-3), "th={th} {a:?} {b:?}");
            let a = faddeeva(Cx::polar(2.0 - 1e-10, th));
            let b = faddeeva(Cx::polar(2.0 + 1e-10, th));
            assert!((a - b).abs() < 1e-6 * a.abs().max(1e-3), "r=2 th={th} {a:?} {b:?}");
        }
    }
}
