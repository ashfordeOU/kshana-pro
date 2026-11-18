// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Windowed-sinc FIR filters (Kaiser window): fractional delay for calibration and
//! low-pass anti-alias filters for decimation, plus a streaming FIR state.
//!
//! The LF eLoran band (90–110 kHz) sits at 0.045–0.055 cycles/sample at 2 MS/s, far below
//! Nyquist, where a Kaiser-windowed sinc of ±32 taps with β = 12 has a delay error of about
//! 0.002 ns at 2 MS/s across the band.

use std::f64::consts::PI;

/// Modified Bessel function of the first kind, order 0 (power series).
pub fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..200 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// Kaiser window evaluated at `x` ∈ [−1, 1] (0 outside).
pub fn kaiser(x: f64, beta: f64) -> f64 {
    if x.abs() > 1.0 {
        return 0.0;
    }
    bessel_i0(beta * (1.0 - x * x).sqrt()) / bessel_i0(beta)
}

/// Normalised sinc `sin(πx)/(πx)`.
pub fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

/// Fractional-delay taps: `2·half + 1` taps delaying by `half + frac` samples
/// (`frac` ∈ [0, 1)). The window is centred on the fractional point, the taps are
/// normalised to unit DC gain.
pub fn fractional_delay_taps(frac: f64, half: usize, beta: f64) -> Vec<f64> {
    let n = 2 * half + 1;
    let centre = half as f64 + frac;
    let span = half as f64 + 1.0;
    let mut h: Vec<f64> = (0..n)
        .map(|k| {
            let x = k as f64 - centre;
            sinc(x) * kaiser(x / span, beta)
        })
        .collect();
    let s: f64 = h.iter().sum();
    for v in h.iter_mut() {
        *v /= s;
    }
    h
}

/// Linear-phase low-pass taps (`n` odd), cut-off `cutoff` in cycles/sample (0 < cutoff < 0.5),
/// unit DC gain.
pub fn lowpass_taps(cutoff: f64, n: usize, beta: f64) -> Vec<f64> {
    let n = n | 1;
    let m = (n / 2) as f64;
    let mut h: Vec<f64> = (0..n)
        .map(|k| {
            let x = k as f64 - m;
            2.0 * cutoff * sinc(2.0 * cutoff * x) * kaiser(x / (m + 1.0), beta)
        })
        .collect();
    let s: f64 = h.iter().sum();
    for v in h.iter_mut() {
        *v /= s;
    }
    h
}

/// A streaming FIR with an extra whole-sample delay `pre`:
/// `y[n] = Σ_k h[k] · x[n − pre − k]`, history kept across blocks.
#[derive(Clone, Debug)]
pub struct FirState {
    taps: Vec<f64>,
    pre: usize,
    hist: Vec<f64>,
}

impl FirState {
    pub fn new(taps: Vec<f64>, pre: usize) -> FirState {
        let h = pre + taps.len().saturating_sub(1);
        FirState {
            taps,
            pre,
            hist: vec![0.0; h],
        }
    }
    pub fn taps(&self) -> &[f64] {
        &self.taps
    }
    /// Filter one block (same length out).
    pub fn process(&mut self, x: &[f64]) -> Vec<f64> {
        let h = self.hist.len();
        let mut buf = Vec::with_capacity(h + x.len());
        buf.extend_from_slice(&self.hist);
        buf.extend_from_slice(x);
        let mut y = vec![0.0; x.len()];
        for (i, out) in y.iter_mut().enumerate() {
            let base = h + i - self.pre; // index of x[n − pre]
            let mut acc = 0.0;
            for (k, t) in self.taps.iter().enumerate() {
                acc += t * buf[base - k];
            }
            *out = acc;
        }
        let keep = buf.len() - h;
        self.hist.copy_from_slice(&buf[keep..]);
        y
    }
}

/// Apply a delay of `delay_samples` (≥ `half`, may be fractional) to a whole signal,
/// keeping its length (the start is zero-filled). Convenience for tests and offline use.
pub fn delay_signal(x: &[f64], delay_samples: f64, half: usize, beta: f64) -> Vec<f64> {
    assert!(
        delay_samples >= half as f64,
        "delay must be at least the FIR half length"
    );
    let i = delay_samples.floor();
    let frac = delay_samples - i;
    let pre = i as usize - half;
    FirState::new(fractional_delay_taps(frac, half, beta), pre).process(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_delay_of_lf_tone_is_accurate() {
        let fs = 2e6;
        let f = 100e3;
        let x: Vec<f64> = (0..4000)
            .map(|n| (2.0 * PI * f * n as f64 / fs).sin())
            .collect();
        let d = 40.3721; // samples
        let y = delay_signal(&x, d, 32, 12.0);
        // project onto the expected sine and cosine: phase error → timing error
        let (mut a, mut b, mut ss, mut cc) = (0.0, 0.0, 0.0, 0.0);
        for (n, yn) in y.iter().enumerate().take(3800).skip(200) {
            let ph = 2.0 * PI * f * (n as f64 - d) / fs;
            a += yn * ph.sin();
            b += yn * ph.cos();
            ss += ph.sin() * ph.sin();
            cc += ph.cos() * ph.cos();
        }
        let (a, b) = (a / ss, b / cc);
        let timing_err_ns = b.atan2(a) / (2.0 * PI * f) * 1e9;
        assert!(
            timing_err_ns.abs() < 0.005,
            "timing error {timing_err_ns} ns"
        );
        assert!(((a * a + b * b).sqrt() - 1.0).abs() < 1e-4, "gain {a} {b}");
    }

    #[test]
    fn lowpass_passes_and_stops() {
        let h = lowpass_taps(0.2, 101, 8.0);
        let resp = |f: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (k, t) in h.iter().enumerate() {
                re += t * (2.0 * PI * f * k as f64).cos();
                im -= t * (2.0 * PI * f * k as f64).sin();
            }
            (re * re + im * im).sqrt()
        };
        assert!((resp(0.05) - 1.0).abs() < 1e-3);
        assert!(resp(0.3) < 1e-3);
    }

    #[test]
    fn streaming_matches_one_shot() {
        let x: Vec<f64> = (0..1000)
            .map(|n| ((n * 7919) % 101) as f64 - 50.0)
            .collect();
        let taps = fractional_delay_taps(0.25, 8, 8.0);
        let whole = FirState::new(taps.clone(), 3).process(&x);
        let mut st = FirState::new(taps, 3);
        let mut parts = Vec::new();
        for c in x.chunks(37) {
            parts.extend(st.process(c));
        }
        for (a, b) in whole.iter().zip(&parts) {
            assert!((a - b).abs() < 1e-12);
        }
    }
}
