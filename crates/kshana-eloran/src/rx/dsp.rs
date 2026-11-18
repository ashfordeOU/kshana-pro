// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receiver DSP primitives: a minimal complex type, an iterative radix-2 FFT, the
//! front-end band-pass FIR design and zero-phase (group-delay-compensated) FFT filtering.
//!
//! # Front-end filter
//!
//! The band-pass is designed as a modulated windowed-sinc low-pass:
//!
//! ```text
//! lp[k] = (2 fc / fs) · sinc(2 fc k / fs) · w_Blackman(k),   k = −M..M,  Σ lp = 1
//! h[k]  = 2 · lp[k] · cos(2π f0 k / fs)
//! ```
//!
//! with `f0` the band centre (100 kHz) and `fc = (f_hi − f_lo)/2 + transition/2`, so the
//! response is flat over `f_lo..f_hi` (90–110 kHz) and −6 dB at `f_lo − transition/2` and
//! `f_hi + transition/2`. The length `N = 2M + 1 ≈ 5.5 fs / transition` gives the Blackman
//! window's ≈ −74 dB stop band. The filter is real, symmetric (linear phase) and its
//! magnitude is symmetric about `f0`, so it **does not move the carrier phase** of a
//! LORAN pulse; it only smooths the envelope (the receiver's templates are built through
//! the same taps, see [`crate::rx::track::Template`]). The filter is applied centred
//! (`y[n] = Σ_{k=−M}^{M} h[k] x[n−k]`), i.e. the causal FIR's group delay of exactly `M`
//! samples is removed by indexing, so filtered samples stay on the scene time axis.

use std::f64::consts::PI;
use std::ops::{Add, Mul, Sub};

/// A complex number (just enough for the receiver).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const ZERO: C64 = C64 { re: 0.0, im: 0.0 };
    #[inline]
    pub fn new(re: f64, im: f64) -> C64 {
        C64 { re, im }
    }
    /// `e^{jφ}`.
    #[inline]
    pub fn cis(phi: f64) -> C64 {
        let (s, c) = phi.sin_cos();
        C64 { re: c, im: s }
    }
    #[inline]
    pub fn conj(self) -> C64 {
        C64 { re: self.re, im: -self.im }
    }
    #[inline]
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    #[inline]
    pub fn abs(self) -> f64 {
        self.norm_sqr().sqrt()
    }
    #[inline]
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }
    #[inline]
    pub fn scale(self, k: f64) -> C64 {
        C64 { re: self.re * k, im: self.im * k }
    }
}

impl Add for C64 {
    type Output = C64;
    #[inline]
    fn add(self, o: C64) -> C64 {
        C64 { re: self.re + o.re, im: self.im + o.im }
    }
}
impl Sub for C64 {
    type Output = C64;
    #[inline]
    fn sub(self, o: C64) -> C64 {
        C64 { re: self.re - o.re, im: self.im - o.im }
    }
}
impl Mul for C64 {
    type Output = C64;
    #[inline]
    fn mul(self, o: C64) -> C64 {
        C64 { re: self.re * o.re - self.im * o.im, im: self.re * o.im + self.im * o.re }
    }
}
impl std::ops::AddAssign for C64 {
    #[inline]
    fn add_assign(&mut self, o: C64) {
        self.re += o.re;
        self.im += o.im;
    }
}

/// In-place iterative radix-2 FFT (`inverse` = true computes the unscaled inverse).
/// `x.len()` must be a power of two.
pub fn fft(x: &mut [C64], inverse: bool) {
    let n = x.len();
    assert!(n.is_power_of_two(), "fft length must be a power of two");
    if n < 2 {
        return;
    }
    // bit reversal
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            x.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        // twiddles for this stage, computed directly (no recurrence error build-up)
        let tw: Vec<C64> = (0..half).map(|k| C64::cis(sign * 2.0 * PI * k as f64 / len as f64)).collect();
        for start in (0..n).step_by(len) {
            for k in 0..half {
                let a = x[start + k];
                let b = x[start + k + half] * tw[k];
                x[start + k] = a + b;
                x[start + k + half] = a - b;
            }
        }
        len <<= 1;
    }
}

/// Front-end filter specification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandPassSpec {
    /// Lower and upper edges of the flat pass band (Hz).
    pub lo_hz: f64,
    pub hi_hz: f64,
    /// Transition width (Hz) on each side.
    pub transition_hz: f64,
}

impl Default for BandPassSpec {
    fn default() -> Self {
        BandPassSpec { lo_hz: 90_000.0, hi_hz: 110_000.0, transition_hz: 20_000.0 }
    }
}

/// A designed zero-phase band-pass FIR.
#[derive(Clone, Debug)]
pub struct Fir {
    /// Taps `h[k]`, `k = −M..M` stored at index `k + M`.
    pub taps: Vec<f64>,
    /// Half length `M` (the causal filter's group delay, samples).
    pub half: usize,
    /// Band centre (Hz).
    pub f0_hz: f64,
    /// Sample rate (Hz).
    pub fs_hz: f64,
    /// `Σ h²`: white-noise power gain.
    pub noise_gain: f64,
}

impl Fir {
    /// Design the band-pass for sample rate `fs_hz`.
    pub fn design(spec: BandPassSpec, fs_hz: f64) -> Result<Fir, String> {
        if !(spec.hi_hz > spec.lo_hz && spec.lo_hz > 0.0 && spec.transition_hz > 0.0) {
            return Err("band-pass: need 0 < lo < hi and a positive transition width".into());
        }
        let f0 = 0.5 * (spec.lo_hz + spec.hi_hz);
        let fc = 0.5 * (spec.hi_hz - spec.lo_hz) + 0.5 * spec.transition_hz;
        if f0 + fc >= fs_hz / 2.0 {
            return Err(format!("band-pass: upper edge {} Hz is above Nyquist for fs = {fs_hz} Hz", f0 + fc));
        }
        let n = ((5.5 * fs_hz / spec.transition_hz).ceil() as usize) | 1;
        let m = n / 2;
        let mut lp: Vec<f64> = (0..n)
            .map(|i| {
                let k = i as f64 - m as f64;
                let x = 2.0 * fc * k / fs_hz;
                let sinc = if k == 0.0 { 1.0 } else { (PI * x).sin() / (PI * x) };
                let w = 0.42 + 0.5 * (2.0 * PI * i as f64 / (n - 1) as f64 - PI).cos()
                    + 0.08 * (4.0 * PI * i as f64 / (n - 1) as f64 - 2.0 * PI).cos();
                sinc * w
            })
            .collect();
        let s: f64 = lp.iter().sum();
        for v in lp.iter_mut() {
            *v /= s;
        }
        let taps: Vec<f64> = lp
            .iter()
            .enumerate()
            .map(|(i, l)| 2.0 * l * (2.0 * PI * f0 * (i as f64 - m as f64) / fs_hz).cos())
            .collect();
        let noise_gain = taps.iter().map(|h| h * h).sum();
        Ok(Fir { taps, half: m, f0_hz: f0, fs_hz, noise_gain })
    }

    /// Filter real signals, zero phase (output aligned with input), by FFT overlap-save.
    /// Channels are processed two at a time packed as real/imaginary parts.
    pub fn filter_many(&self, xs: &[&[f64]]) -> Vec<Vec<f64>> {
        let mut out = Vec::with_capacity(xs.len());
        let mut i = 0;
        while i < xs.len() {
            let a = xs[i];
            let b = xs.get(i + 1).copied();
            let (ya, yb) = self.filter_pair(a, b);
            out.push(ya);
            if let Some(yb) = yb {
                out.push(yb);
            }
            i += 2;
        }
        out
    }

    /// Filter one real signal (zero phase).
    pub fn filter(&self, x: &[f64]) -> Vec<f64> {
        self.filter_pair(x, None).0
    }

    fn filter_pair(&self, a: &[f64], b: Option<&[f64]>) -> (Vec<f64>, Option<Vec<f64>>) {
        let nh = self.taps.len();
        let len = a.len().max(b.map(|v| v.len()).unwrap_or(0));
        let nf = (4 * nh).next_power_of_two().max(4096);
        let step = nf - nh + 1;
        let mut hf = vec![C64::ZERO; nf];
        for (k, h) in self.taps.iter().enumerate() {
            hf[k] = C64::new(*h, 0.0);
        }
        fft(&mut hf, false);
        let m = self.half as isize;
        let mut ya = vec![0.0; len];
        let mut yb = b.map(|_| vec![0.0; len]);
        let get = |x: &[f64], i: isize| -> f64 {
            if i >= 0 && (i as usize) < x.len() {
                x[i as usize]
            } else {
                0.0
            }
        };
        let mut buf = vec![C64::ZERO; nf];
        // causal outputs y_c[n] for n ∈ [0, len + M); centred y[n] = y_c[n + M]
        let total = len + self.half;
        let mut s = 0usize;
        let scale = 1.0 / nf as f64;
        while s < total {
            for (j, v) in buf.iter_mut().enumerate() {
                let idx = s as isize - (nh as isize - 1) + j as isize;
                *v = C64::new(get(a, idx), b.map(|bb| get(bb, idx)).unwrap_or(0.0));
            }
            fft(&mut buf, false);
            for (v, h) in buf.iter_mut().zip(&hf) {
                *v = *v * *h;
            }
            fft(&mut buf, true);
            for j in 0..step {
                let n_c = s + j;
                let n = n_c as isize - m;
                if n < 0 {
                    continue;
                }
                if n as usize >= len {
                    break;
                }
                let v = buf[nh - 1 + j].scale(scale);
                ya[n as usize] = v.re;
                if let Some(yb) = yb.as_mut() {
                    yb[n as usize] = v.im;
                }
            }
            s += step;
        }
        (ya, yb)
    }
}

/// Solve a small dense linear system `A x = b` (Gauss–Jordan, partial pivoting).
/// Returns `None` if singular.
pub fn solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let mut m: Vec<Vec<f64>> = a.iter().zip(b).map(|(r, bi)| {
        let mut r = r.clone();
        r.push(*bi);
        r
    }).collect();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| m[i][c].abs().partial_cmp(&m[j][c].abs()).unwrap())?;
        if m[p][c].abs() < 1e-300 {
            return None;
        }
        m.swap(c, p);
        let d = m[c][c];
        for v in m[c].iter_mut() {
            *v /= d;
        }
        for r in 0..n {
            if r != c {
                let f = m[r][c];
                if f != 0.0 {
                    let pivot = m[c].clone();
                    for (x, p) in m[r].iter_mut().zip(&pivot).skip(c) {
                        *x -= f * p;
                    }
                }
            }
        }
    }
    Some(m.iter().map(|r| r[n]).collect())
}

/// Inverse of a small dense matrix (`None` if singular).
pub fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut cols = Vec::with_capacity(n);
    for c in 0..n {
        let mut e = vec![0.0; n];
        e[c] = 1.0;
        cols.push(solve(a, &e)?);
    }
    Some((0..n).map(|r| (0..n).map(|c| cols[c][r]).collect()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_round_trip_and_known_bin() {
        let n = 64;
        let mut x: Vec<C64> = (0..n).map(|i| C64::cis(2.0 * PI * 5.0 * i as f64 / n as f64)).collect();
        let orig = x.clone();
        fft(&mut x, false);
        assert!((x[5].re - n as f64).abs() < 1e-9);
        assert!(x[6].abs() < 1e-9);
        fft(&mut x, true);
        for (a, b) in x.iter().zip(&orig) {
            assert!((a.scale(1.0 / n as f64) - *b).abs() < 1e-12);
        }
    }

    #[test]
    fn fft_filter_equals_direct_centred_convolution() {
        let fir = Fir::design(BandPassSpec::default(), 1_000_000.0).unwrap();
        let x: Vec<f64> = (0..20_000).map(|i| ((i * 7919) % 1000) as f64 / 500.0 - 1.0).collect();
        let y = fir.filter(&x);
        let m = fir.half as isize;
        for n in [0usize, 5, 137, 9_999, 19_999] {
            let mut d = 0.0;
            for (i, h) in fir.taps.iter().enumerate() {
                let k = i as isize - m;
                let j = n as isize - k;
                if j >= 0 && (j as usize) < x.len() {
                    d += h * x[j as usize];
                }
            }
            assert!((d - y[n]).abs() < 1e-10, "n={n} {d} {}", y[n]);
        }
    }

    #[test]
    fn band_pass_response_flat_in_band_rejects_out_of_band() {
        let fs = 1_000_000.0;
        let fir = Fir::design(BandPassSpec::default(), fs).unwrap();
        let resp = |f: f64| -> f64 {
            let m = fir.half as f64;
            let mut c = C64::ZERO;
            for (i, h) in fir.taps.iter().enumerate() {
                c += C64::cis(-2.0 * PI * f * (i as f64 - m) / fs).scale(*h);
            }
            // zero phase: the response is real
            assert!(c.im.abs() < 1e-9);
            c.re
        };
        for f in [90e3, 95e3, 100e3, 105e3, 110e3] {
            assert!((resp(f) - 1.0).abs() < 2e-3, "{f}: {}", resp(f));
        }
        assert!(resp(50e3).abs() < 1e-3);
        assert!(resp(160e3).abs() < 1e-3);
    }

    #[test]
    fn small_linear_solve() {
        let a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let x = solve(&a, &[3.0, 5.0]).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12);
        let inv = invert(&a).unwrap();
        assert!((inv[0][0] - 0.6).abs() < 1e-12);
    }
}
