// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Small DSP tools for the interference sources and their tests: a windowed-sinc FIR
//! band-pass design, a Goertzel single-bin DFT and an in-band power estimate.

use std::f64::consts::{PI, TAU};

/// Linear-phase band-pass FIR (`taps` odd) passing `f1..f2` Hz at sample rate `fs`:
/// ideal band-pass impulse response × Blackman window. Stop-band ≈ −74 dB; transition
/// width ≈ 5.5·fs/taps.
pub fn fir_bandpass(f1: f64, f2: f64, fs: f64, taps: usize) -> Vec<f64> {
    assert!(taps % 2 == 1 && taps >= 3, "taps must be odd and ≥ 3");
    let m = (taps - 1) as f64 / 2.0;
    let (a, b) = (f1 / fs, f2 / fs);
    let sinc = |x: f64| if x == 0.0 { 1.0 } else { (PI * x).sin() / (PI * x) };
    (0..taps)
        .map(|n| {
            let k = n as f64 - m;
            let ideal = 2.0 * b * sinc(2.0 * b * k) - 2.0 * a * sinc(2.0 * a * k);
            let w = 0.42 - 0.5 * (TAU * n as f64 / (taps - 1) as f64).cos()
                + 0.08 * (2.0 * TAU * n as f64 / (taps - 1) as f64).cos();
            ideal * w
        })
        .collect()
}

/// Goertzel: `|X(f)|²` of `x` at frequency `f` (DFT sum, not normalised).
pub fn goertzel(x: &[f64], fs: f64, f: f64) -> f64 {
    let w = TAU * f / fs;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0, 0.0);
    for v in x {
        let s0 = v + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

/// Mean-square power of `x` in `[f1, f2]` Hz (one-sided), from DFT bins of contiguous
/// segments of `seg` samples (rectangular window, bin spacing `fs/seg`; bins on the band
/// edges count half). For white noise of variance σ² this returns `σ²·(f2−f1)/(fs/2)`;
/// for a tone of RMS `a` on a bin inside the band, `a²`.
pub fn band_power(x: &[f64], fs: f64, f1: f64, f2: f64, seg: usize) -> f64 {
    let df = fs / seg as f64;
    let k1 = (f1 / df).round() as usize;
    let k2 = (f2 / df).round() as usize;
    let nseg = x.len() / seg;
    assert!(nseg > 0 && k2 > k1 && 2 * k2 < seg, "band_power: bad segment or band");
    let mut acc = 0.0;
    for s in 0..nseg {
        let y = &x[s * seg..(s + 1) * seg];
        for k in k1..=k2 {
            let w = if k == k1 || k == k2 { 0.5 } else { 1.0 };
            acc += w * 2.0 * goertzel(y, fs, k as f64 * df) / (seg * seg) as f64;
        }
    }
    acc / nseg as f64
}

/// RMS of `x` in `[f1, f2]` Hz (see [`band_power`]).
pub fn band_rms(x: &[f64], fs: f64, f1: f64, f2: f64, seg: usize) -> f64 {
    band_power(x, fs, f1, f2, seg).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_power_on_bin() {
        let fs = 400_000.0;
        let x: Vec<f64> = (0..40_000).map(|n| 3.0 * (TAU * 100_000.0 * n as f64 / fs).sin()).collect();
        let p = band_power(&x, fs, 90_000.0, 110_000.0, 400);
        assert!((p - 4.5).abs() < 1e-6, "{p}");
        // out of band: nothing
        assert!(band_power(&x, fs, 20_000.0, 40_000.0, 400) < 1e-12);
    }

    #[test]
    fn fir_passes_band_and_rejects_outside() {
        let fs = 1e6;
        let h = fir_bandpass(90e3, 110e3, fs, 255);
        let resp = |f: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (n, c) in h.iter().enumerate() {
                re += c * (TAU * f * n as f64 / fs).cos();
                im -= c * (TAU * f * n as f64 / fs).sin();
            }
            (re * re + im * im).sqrt()
        };
        assert!((resp(100e3) - 1.0).abs() < 0.01);
        assert!(resp(50e3) < 1e-3 && resp(160e3) < 1e-3);
    }
}
