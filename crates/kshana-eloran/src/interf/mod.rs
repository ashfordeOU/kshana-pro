// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Interference and noise added to the output channels (stream S7 extends this with
//! pulsed, impulsive, atmospheric noise and combinations).
//!
//! Every source is deterministic for a given seed and adds field-strength samples (µV/m)
//! to each block, in block order.

use crate::time::Ps;
use crate::timebase::gauss;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Receiver noise bandwidth used to state noise levels and SNR (Hz).
pub const NOISE_BW_HZ: f64 = 20_000.0;

/// An additive source on the output channels.
pub trait Interferer: Send {
    fn name(&self) -> String;
    /// Add this source to `out` (channel `ch`), whose first sample is at scene time `t0`.
    fn add(&mut self, t0: Ps, fs_hz: f64, ch: usize, out: &mut [f64]);
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InterfConfig {
    /// Continuous-wave tone of RMS field `rms_uv_m` at `freq_hz`.
    Cw {
        freq_hz: f64,
        rms_uv_m: f64,
        #[serde(default)]
        phase_deg: f64,
        #[serde(default)]
        from_s: Option<f64>,
        #[serde(default)]
        to_s: Option<f64>,
    },
    /// White Gaussian noise whose RMS in a 20 kHz bandwidth is `rms_uv_m_20khz`.
    Wgn {
        rms_uv_m_20khz: f64,
        #[serde(default)]
        seed: u64,
    },
}

pub struct Cw {
    pub freq_hz: f64,
    pub amp: f64,
    pub phase: f64,
    pub from_s: f64,
    pub to_s: f64,
}

impl Interferer for Cw {
    fn name(&self) -> String {
        format!("cw {:.1} Hz", self.freq_hz)
    }
    fn add(&mut self, t0: Ps, fs_hz: f64, _ch: usize, out: &mut [f64]) {
        let t0s = t0.as_s();
        for (n, v) in out.iter_mut().enumerate() {
            let t = t0s + n as f64 / fs_hz;
            if t >= self.from_s && t < self.to_s {
                *v += self.amp * (std::f64::consts::TAU * self.freq_hz * t + self.phase).sin();
            }
        }
    }
}

/// White Gaussian noise, one independent stream per channel.
pub struct Wgn {
    pub sigma: f64,
    pub seed: u64,
    rngs: Vec<ChaCha8Rng>,
}

impl Wgn {
    /// `rms_20k` is the RMS within 20 kHz; the full-band σ at sample rate `fs` is
    /// `rms_20k · √((fs/2) / 20 kHz)`.
    pub fn new(rms_20k: f64, fs_hz: f64, seed: u64) -> Wgn {
        Wgn { sigma: rms_20k * ((fs_hz / 2.0) / NOISE_BW_HZ).sqrt(), seed, rngs: vec![] }
    }
}

impl Interferer for Wgn {
    fn name(&self) -> String {
        "wgn".into()
    }
    fn add(&mut self, _t0: Ps, _fs_hz: f64, ch: usize, out: &mut [f64]) {
        while self.rngs.len() <= ch {
            let k = self.rngs.len() as u64;
            self.rngs.push(ChaCha8Rng::seed_from_u64(self.seed.wrapping_mul(0x9E37_79B9).wrapping_add(k)));
        }
        let rng = &mut self.rngs[ch];
        for v in out.iter_mut() {
            *v += self.sigma * gauss(rng);
        }
    }
}

/// Build a source. `scene_seed` is mixed into stochastic sources so scenes are reproducible.
pub fn build(cfg: &InterfConfig, fs_hz: f64, scene_seed: u64) -> Result<Box<dyn Interferer>, String> {
    Ok(match cfg {
        InterfConfig::Cw { freq_hz, rms_uv_m, phase_deg, from_s, to_s } => Box::new(Cw {
            freq_hz: *freq_hz,
            amp: rms_uv_m * std::f64::consts::SQRT_2,
            phase: phase_deg.to_radians(),
            from_s: from_s.unwrap_or(f64::NEG_INFINITY),
            to_s: to_s.unwrap_or(f64::INFINITY),
        }),
        InterfConfig::Wgn { rms_uv_m_20khz, seed } => {
            Box::new(Wgn::new(*rms_uv_m_20khz, fs_hz, seed ^ scene_seed))
        }
    })
}

/// RMS noise in 20 kHz of the stochastic sources of a configuration (for SNR in the truth).
pub fn noise_rms_20k(cfgs: &[InterfConfig]) -> f64 {
    cfgs.iter()
        .map(|c| match c {
            InterfConfig::Wgn { rms_uv_m_20khz, .. } => rms_uv_m_20khz.powi(2),
            _ => 0.0,
        })
        .sum::<f64>()
        .sqrt()
}
