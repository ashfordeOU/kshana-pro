// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Slow skywave fading (MODELLED): a seeded, time-correlated complex Gaussian process
//! (Rayleigh) with an optional steady component (Rice).
//!
//! The diffuse component is a **sum of sinusoids** (Clarke/Jakes-type, here with a
//! Gaussian Doppler spectrum): `I(t) = √(1/N) Σ cos(2π fᵢ t + φᵢ)` (same for `Q`, independent
//! draws), `fᵢ ~ N(0, σ_f)`, `φᵢ ~ U(0, 2π)`, `σ_f = 1/(π√2 · T_c)`. The ensemble
//! autocorrelation of `I` is `½ exp(−τ²/T_c²)`, i.e. it falls to 1/e at the configured
//! correlation time `T_c`. With Rice factor `K` the complex gain is
//! `h = √(K/(K+1)) + √(1/(K+1)) (I + jQ)`, `E|h|² = 1`, so the configured skywave field is
//! the RMS (mean-power) field.
//!
//! The process is a pure function of `(seed, station, hop, t)`: deterministic, independent
//! of block size and evaluation order, and needs no state in the (shared, immutable)
//! propagation model. Each station and hop fades independently.
//!
//! Reference: W.C. Jakes, *Microwave Mobile Communications* (1974) ch. 1; skywave fading
//! at LF is slow (correlation times of tens of seconds to minutes, e.g. ITU-R P.684 §4
//! notes on temporal variability), which motivates the default `T_c` = 30 s. Not validated
//! against measured LF fading statistics.

use crate::timebase::gauss;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FadingConfig {
    /// 1/e correlation time of the complex gain (s).
    #[serde(default = "d_tc")]
    pub corr_time_s: f64,
    /// Rice factor (dB, steady/diffuse power); absent = Rayleigh.
    #[serde(default)]
    pub k_factor_db: Option<f64>,
    #[serde(default)]
    pub seed: u64,
    /// Number of sinusoids per quadrature.
    #[serde(default = "d_n")]
    pub n_sinusoids: usize,
}

fn d_tc() -> f64 {
    30.0
}
fn d_n() -> usize {
    64
}

impl Default for FadingConfig {
    fn default() -> Self {
        FadingConfig { corr_time_s: d_tc(), k_factor_db: None, seed: 0, n_sinusoids: d_n() }
    }
}

/// The fading process.
#[derive(Clone, Debug)]
pub struct FadingProcess {
    pub sigma_f_hz: f64,
    /// Steady and diffuse amplitude weights.
    pub los: f64,
    pub diffuse: f64,
    pub seed: u64,
    pub n: usize,
}

impl FadingProcess {
    pub fn new(c: &FadingConfig) -> Result<FadingProcess, String> {
        if c.corr_time_s.is_nan() || c.corr_time_s <= 0.0 {
            return Err("fading: corr_time_s must be positive".into());
        }
        if !(4..=1024).contains(&c.n_sinusoids) {
            return Err("fading: n_sinusoids in 4..1024".into());
        }
        let (los, diffuse) = match c.k_factor_db {
            None => (0.0, 1.0),
            Some(kdb) => {
                let k = 10f64.powf(kdb / 10.0);
                ((k / (k + 1.0)).sqrt(), (1.0 / (k + 1.0)).sqrt())
            }
        };
        Ok(FadingProcess {
            sigma_f_hz: 1.0 / (std::f64::consts::PI * std::f64::consts::SQRT_2 * c.corr_time_s),
            los,
            diffuse,
            seed: c.seed,
            n: c.n_sinusoids,
        })
    }

    /// Complex gain `(I, Q)` of stream `key` at time `t_s` (diffuse part, E[I²+Q²] = 1).
    pub fn iq(&self, key: u64, t_s: f64) -> (f64, f64) {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ key.rotate_left(17) ^ 0x5EED_FAD1_0000_0001);
        let norm = (1.0 / self.n as f64).sqrt();
        let mut acc = [0.0f64; 2];
        for a in acc.iter_mut() {
            for _ in 0..self.n {
                let f = self.sigma_f_hz * gauss(&mut rng);
                let ph: f64 = rng.gen::<f64>() * TAU;
                *a += (TAU * f * t_s + ph).cos();
            }
            *a *= norm;
        }
        (acc[0], acc[1])
    }

    /// Amplitude factor and phase (rad) of stream `key` at `t_s`.
    pub fn gain(&self, key: u64, t_s: f64) -> (f64, f64) {
        let (i, q) = self.iq(key, t_s);
        let re = self.los + self.diffuse * i;
        let im = self.diffuse * q;
        ((re * re + im * im).sqrt(), im.atan2(re))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rayleigh_statistics_and_correlation() {
        let p = FadingProcess::new(&FadingConfig { corr_time_s: 20.0, seed: 3, ..Default::default() }).unwrap();
        // average over several independent streams and a long window
        let mut pw = 0.0;
        let mut below = 0usize;
        let mut cnt = 0usize;
        let (mut c0, mut c1, mut c3) = (0.0, 0.0, 0.0);
        for key in 0..20u64 {
            for k in 0..400 {
                let t = k as f64 * 5.0; // 2000 s = 100 Tc
                let (i, q) = p.iq(key, t);
                let a2 = i * i + q * q;
                pw += a2;
                if a2 < 1.0 {
                    below += 1;
                }
                cnt += 1;
                let (i1, _) = p.iq(key, t + 2.0);
                let (i3, _) = p.iq(key, t + 60.0);
                c0 += i * i;
                c1 += i * i1;
                c3 += i * i3;
            }
        }
        let mean = pw / cnt as f64;
        assert!((mean - 1.0).abs() < 0.1, "mean power {mean}");
        // Rayleigh: P(|h|² < mean) = 1 − 1/e ≈ 0.632
        let frac = below as f64 / cnt as f64;
        assert!((frac - 0.632).abs() < 0.06, "{frac}");
        // correlation: ≈ exp(−(2/20)²) ≈ 0.99 at 2 s, ≈ 0 at 3 Tc
        assert!(c1 / c0 > 0.9, "{}", c1 / c0);
        assert!((c3 / c0).abs() < 0.15, "{}", c3 / c0);
    }

    #[test]
    fn deterministic_and_seeded() {
        let a = FadingProcess::new(&FadingConfig { seed: 1, ..Default::default() }).unwrap();
        let b = FadingProcess::new(&FadingConfig { seed: 2, ..Default::default() }).unwrap();
        assert_eq!(a.gain(7, 12.5), a.gain(7, 12.5));
        assert_ne!(a.gain(7, 12.5), b.gain(7, 12.5));
        assert_ne!(a.gain(7, 12.5), a.gain(8, 12.5));
    }

    #[test]
    fn rice_has_less_spread() {
        let p = FadingProcess::new(&FadingConfig { k_factor_db: Some(10.0), ..Default::default() }).unwrap();
        let mut deep = 0usize;
        let mut pw = 0.0;
        for k in 0..2000 {
            let (a, _) = p.gain(1, k as f64 * 3.0);
            if a < 0.5 {
                deep += 1;
            }
            pw += a * a;
        }
        // Rayleigh would be below 0.5 (−6 dB) 22 % of the time; Rice K = 10 dB rarely
        assert!(deep < 100, "{deep}");
        assert!((pw / 2000.0 - 1.0).abs() < 0.1);
    }
}
