// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! A small complex-number type for the groundwave theory (no external crate).
//!
//! Only what the attenuation-function code needs: field arithmetic, `exp`, `ln`
//! (principal branch), `sqrt` (principal branch, `Re ≥ 0`), integer powers and polar form.

use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

/// A complex number `re + i·im`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Cx {
    pub re: f64,
    pub im: f64,
}

/// The imaginary unit.
pub const I: Cx = Cx { re: 0.0, im: 1.0 };
pub const ONE: Cx = Cx { re: 1.0, im: 0.0 };
pub const ZERO: Cx = Cx { re: 0.0, im: 0.0 };

impl Cx {
    #[inline]
    pub const fn new(re: f64, im: f64) -> Cx {
        Cx { re, im }
    }
    #[inline]
    pub const fn real(re: f64) -> Cx {
        Cx { re, im: 0.0 }
    }
    /// `r·e^{iθ}`.
    #[inline]
    pub fn polar(r: f64, theta: f64) -> Cx {
        let (s, c) = theta.sin_cos();
        Cx { re: r * c, im: r * s }
    }
    /// `e^{iθ}`.
    #[inline]
    pub fn cis(theta: f64) -> Cx {
        Cx::polar(1.0, theta)
    }
    #[inline]
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
    #[inline]
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    /// Principal argument in (−π, π].
    #[inline]
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    #[inline]
    pub fn conj(self) -> Cx {
        Cx { re: self.re, im: -self.im }
    }
    #[inline]
    pub fn scale(self, k: f64) -> Cx {
        Cx { re: self.re * k, im: self.im * k }
    }
    #[inline]
    pub fn exp(self) -> Cx {
        Cx::polar(self.re.exp(), self.im)
    }
    /// Principal logarithm.
    #[inline]
    pub fn ln(self) -> Cx {
        Cx { re: self.abs().ln(), im: self.arg() }
    }
    /// Principal square root (branch cut on the negative real axis, `Re ≥ 0`).
    pub fn sqrt(self) -> Cx {
        let r = self.abs();
        if r == 0.0 {
            return ZERO;
        }
        let re = ((r + self.re) / 2.0).sqrt();
        let im = ((r - self.re) / 2.0).sqrt();
        Cx { re, im: if self.im < 0.0 { -im } else { im } }
    }
    /// Principal real power `z^a = e^{a·ln z}`.
    pub fn powf(self, a: f64) -> Cx {
        if self.re == 0.0 && self.im == 0.0 {
            return ZERO;
        }
        let r = self.abs().powf(a);
        Cx::polar(r, a * self.arg())
    }
    pub fn powi(self, n: i32) -> Cx {
        let mut r = ONE;
        let mut b = if n < 0 { ONE / self } else { self };
        let mut e = n.unsigned_abs();
        while e > 0 {
            if e & 1 == 1 {
                r *= b;
            }
            b = b * b;
            e >>= 1;
        }
        r
    }
    pub fn recip(self) -> Cx {
        let d = self.norm_sqr();
        Cx { re: self.re / d, im: -self.im / d }
    }
    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

impl Add for Cx {
    type Output = Cx;
    #[inline]
    fn add(self, o: Cx) -> Cx {
        Cx { re: self.re + o.re, im: self.im + o.im }
    }
}
impl Sub for Cx {
    type Output = Cx;
    #[inline]
    fn sub(self, o: Cx) -> Cx {
        Cx { re: self.re - o.re, im: self.im - o.im }
    }
}
impl Mul for Cx {
    type Output = Cx;
    #[inline]
    fn mul(self, o: Cx) -> Cx {
        Cx { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }
}
impl Div for Cx {
    type Output = Cx;
    #[inline]
    fn div(self, o: Cx) -> Cx {
        // Smith's algorithm (avoids overflow for large components).
        if o.re.abs() >= o.im.abs() {
            let r = o.im / o.re;
            let d = o.re + o.im * r;
            Cx { re: (self.re + self.im * r) / d, im: (self.im - self.re * r) / d }
        } else {
            let r = o.re / o.im;
            let d = o.re * r + o.im;
            Cx { re: (self.re * r + self.im) / d, im: (self.im * r - self.re) / d }
        }
    }
}
impl Neg for Cx {
    type Output = Cx;
    #[inline]
    fn neg(self) -> Cx {
        Cx { re: -self.re, im: -self.im }
    }
}
impl Add<f64> for Cx {
    type Output = Cx;
    #[inline]
    fn add(self, o: f64) -> Cx {
        Cx { re: self.re + o, im: self.im }
    }
}
impl Sub<f64> for Cx {
    type Output = Cx;
    #[inline]
    fn sub(self, o: f64) -> Cx {
        Cx { re: self.re - o, im: self.im }
    }
}
impl Mul<f64> for Cx {
    type Output = Cx;
    #[inline]
    fn mul(self, o: f64) -> Cx {
        self.scale(o)
    }
}
impl Div<f64> for Cx {
    type Output = Cx;
    #[inline]
    fn div(self, o: f64) -> Cx {
        Cx { re: self.re / o, im: self.im / o }
    }
}
impl Mul<Cx> for f64 {
    type Output = Cx;
    #[inline]
    fn mul(self, o: Cx) -> Cx {
        o.scale(self)
    }
}
impl Add<Cx> for f64 {
    type Output = Cx;
    #[inline]
    fn add(self, o: Cx) -> Cx {
        Cx { re: self + o.re, im: o.im }
    }
}
impl Sub<Cx> for f64 {
    type Output = Cx;
    #[inline]
    fn sub(self, o: Cx) -> Cx {
        Cx { re: self - o.re, im: -o.im }
    }
}
impl AddAssign for Cx {
    #[inline]
    fn add_assign(&mut self, o: Cx) {
        self.re += o.re;
        self.im += o.im;
    }
}
impl SubAssign for Cx {
    #[inline]
    fn sub_assign(&mut self, o: Cx) {
        self.re -= o.re;
        self.im -= o.im;
    }
}
impl MulAssign for Cx {
    #[inline]
    fn mul_assign(&mut self, o: Cx) {
        *self = *self * o;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Cx, b: Cx, tol: f64) -> bool {
        (a - b).abs() <= tol * (1.0 + b.abs())
    }

    #[test]
    fn arithmetic_and_functions() {
        let a = Cx::new(1.5, -2.0);
        let b = Cx::new(-0.3, 0.7);
        assert!(close(a * b / b, a, 1e-15));
        assert!(close(a.sqrt() * a.sqrt(), a, 1e-15));
        assert!(a.sqrt().re >= 0.0);
        assert!(close(a.ln().exp(), a, 1e-15));
        assert!(close(a.powi(3), a * a * a, 1e-15));
        assert!(close(a.powi(-2), ONE / (a * a), 1e-15));
        assert!(close(a.powf(1.5), a.sqrt() * a, 1e-14));
        assert!(close(Cx::new(-4.0, 0.0).sqrt(), Cx::new(0.0, 2.0), 1e-15));
        assert!(close(Cx::new(-4.0, -0.0).sqrt(), Cx::new(0.0, 2.0), 1e-15));
        assert!(close(I * I, Cx::real(-1.0), 0.0));
    }
}
