// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Pulsed interference: a periodic gate (period, width or duty cycle, offset, start/stop
//! window) applied to a carrier or to wideband Gaussian noise. Representative of pulsed
//! radars/beacons, other pulsed navaids and gated jammers. The gate is a function of the
//! global sample time, so the output does not depend on the block size.

use super::{chan_seed, sample_index, Interferer, PerChannel, NOISE_BW_HZ};
use crate::time::Ps;
use crate::timebase::gauss;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::f64::consts::{SQRT_2, TAU};

/// Pulse width from `width_s` or `duty` (exactly one).
pub fn width(period_s: f64, width_s: Option<f64>, duty: Option<f64>) -> Result<f64, String> {
    if period_s.is_nan() || period_s <= 0.0 {
        return Err("pulsed: period_s must be positive".into());
    }
    let w = match (width_s, duty) {
        (Some(w), None) => w,
        (None, Some(d)) => d * period_s,
        _ => return Err("pulsed: give exactly one of width_s or duty".into()),
    };
    if !(w > 0.0 && w <= period_s) {
        return Err(format!("pulsed: width {w} s must be in (0, period]"));
    }
    Ok(w)
}

pub struct Pulsed {
    pub period: f64,
    pub width: f64,
    pub offset: f64,
    /// Carrier (Hz) or wideband if `None`.
    pub freq_hz: Option<f64>,
    /// Peak carrier amplitude or full-band noise σ.
    pub amp: f64,
    pub phase: f64,
    pub from: f64,
    pub to: f64,
    seed: u64,
    coherent: bool,
    rngs: PerChannel<ChaCha8Rng>,
}

impl Pulsed {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        period: f64,
        width: f64,
        offset: f64,
        freq_hz: Option<f64>,
        rms_uv_m: f64,
        phase: f64,
        from: f64,
        to: f64,
        fs_hz: f64,
        seed: u64,
        coherent: bool,
    ) -> Result<Pulsed, String> {
        if rms_uv_m.is_nan() || rms_uv_m < 0.0 {
            return Err("pulsed: rms_uv_m must be ≥ 0".into());
        }
        if let Some(f) = freq_hz {
            if !(f > 0.0 && f < fs_hz / 2.0) {
                return Err(format!("pulsed: freq_hz {f} must be in (0, fs/2)"));
            }
        }
        let amp = match freq_hz {
            Some(_) => rms_uv_m * SQRT_2,
            None => rms_uv_m * ((fs_hz / 2.0) / NOISE_BW_HZ).sqrt(),
        };
        Ok(Pulsed { period, width, offset, freq_hz, amp, phase, from, to, seed, coherent, rngs: PerChannel::new() })
    }

    /// True while the gate is on at time `t` (s).
    pub fn on(&self, t: f64) -> bool {
        t >= self.from && t < self.to && (t - self.offset).rem_euclid(self.period) < self.width
    }
}

impl Interferer for Pulsed {
    fn name(&self) -> String {
        match self.freq_hz {
            Some(f) => format!("pulsed carrier {f:.0} Hz"),
            None => "pulsed wideband".into(),
        }
    }
    fn add(&mut self, t0: Ps, fs: f64, ch: usize, out: &mut [f64]) {
        let g0 = sample_index(t0, fs);
        let (seed, coherent) = (self.seed, self.coherent);
        for (n, v) in out.iter_mut().enumerate() {
            let t = (g0 + n as i64) as f64 / fs;
            if !self.on(t) {
                continue;
            }
            *v += match self.freq_hz {
                Some(f) => self.amp * (TAU * f * t + self.phase).sin(),
                None => {
                    let rng = self.rngs.get(ch, |k| ChaCha8Rng::seed_from_u64(chan_seed(seed, k, coherent)));
                    self.amp * gauss(rng)
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interf::dsp::band_rms;

    #[test]
    fn gate_and_levels() {
        let fs = 400_000.0;
        // 1 ms period, 25 % duty, carrier 100 kHz at 10 µV/m RMS while on
        let mut p = Pulsed::new(1e-3, 0.25e-3, 0.0, Some(100e3), 10.0, 0.0, 0.0, f64::INFINITY, fs, 1, true).unwrap();
        let mut x = vec![0.0; 40_000];
        p.add(Ps::ZERO, fs, 0, &mut x);
        let on = x.iter().filter(|v| **v != 0.0).count() as f64 / x.len() as f64;
        assert!((on - 0.25).abs() < 0.01, "{on}");
        let ms: f64 = x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64;
        assert!((ms - 25.0).abs() < 0.5, "{ms}"); // 100 · 0.25
        // wideband: in-band RMS while on equals the configured 20 kHz RMS
        let mut w = Pulsed::new(1e-3, 0.5e-3, 0.0, None, 5.0, 0.0, 0.0, f64::INFINITY, fs, 1, true).unwrap();
        let mut y = vec![0.0; 400_000];
        w.add(Ps::ZERO, fs, 0, &mut y);
        let r = band_rms(&y, fs, 90e3, 110e3, 400);
        let expect = 5.0 * 0.5f64.sqrt();
        assert!((r / expect - 1.0).abs() < 0.05, "{r} vs {expect}");
        // window
        let mut q = Pulsed::new(1e-3, 0.5e-3, 0.0, Some(50e3), 1.0, 0.0, 0.01, 0.02, fs, 1, true).unwrap();
        let mut z = vec![0.0; 12_000];
        q.add(Ps::ZERO, fs, 0, &mut z);
        assert!(z[..4000].iter().all(|v| *v == 0.0) && z[8000..].iter().all(|v| *v == 0.0));
        assert!(z[4000..8000].iter().any(|v| *v != 0.0));
        assert!(width(1.0, Some(0.1), Some(0.1)).is_err());
    }
}
