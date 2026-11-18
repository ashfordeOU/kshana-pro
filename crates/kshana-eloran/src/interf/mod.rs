// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Interference and noise added to the output channels (stream S7).
//!
//! Every source is deterministic for a given seed and adds field-strength samples (µV/m)
//! to each block, in block order. Sources:
//!
//! | `type`          | model                                                     | label     |
//! |-----------------|-----------------------------------------------------------|-----------|
//! | `cw`            | continuous tone                                           | exact     |
//! | `wgn`           | white Gaussian noise, RMS stated in 20 kHz                | exact     |
//! | `pulsed`        | periodic pulsed carrier or wideband burst ([`pulsed`])    | exact     |
//! | `impulsive`     | Poisson damped-oscillation impulses, SMPS/motor/railway presets ([`impulsive`]) | MODELLED |
//! | `atmospheric`   | Middleton Class A, level from ITU-R P.372 Fam ([`atmos`]) | MODELLED  |
//! | `band_noise`    | FIR band-limited Gaussian noise ([`band`])                | exact     |
//!
//! "exact" means the output matches its configuration by construction (tested); the
//! MODELLED sources are representative models, not validated against measurements.
//!
//! Sources are listed in the scenario and summed (any combination). Stochastic sources keep
//! their state (RNG streams, filter history, pending impulses) across blocks and index time
//! by the global sample number, so their output does not depend on the block size. A
//! source is either `coherent` (one physical emitter: the same waveform on every output
//! channel) or independent per channel (diffuse noise). [`cri`] has the cross-rate
//! interference overlap statistics of LORAN chains.

pub mod atmos;
pub mod band;
pub mod cri;
pub mod dsp;
pub mod impulsive;
pub mod pulsed;

use crate::time::{Ps, PS_PER_S};
use crate::timebase::gauss;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

pub use atmos::{AtmosConfig, Season};
pub use impulsive::{BurstConfig, ImpulsePreset, ImpulsiveConfig};

/// Receiver noise bandwidth used to state noise levels and SNR (Hz).
pub const NOISE_BW_HZ: f64 = 20_000.0;
/// Centre of the receiver noise bandwidth (the LORAN carrier, Hz).
pub const RX_CENTRE_HZ: f64 = 100_000.0;

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
    /// Periodic pulses: on for `width_s` (or `duty`·period) every `period_s`, starting
    /// at `offset_s`, inside `[from_s, to_s)`. With `freq_hz` a carrier of RMS `rms_uv_m`
    /// while on; without, wideband Gaussian noise of RMS `rms_uv_m` in 20 kHz while on.
    Pulsed {
        period_s: f64,
        #[serde(default)]
        width_s: Option<f64>,
        #[serde(default)]
        duty: Option<f64>,
        #[serde(default)]
        offset_s: f64,
        #[serde(default)]
        freq_hz: Option<f64>,
        rms_uv_m: f64,
        #[serde(default)]
        phase_deg: f64,
        #[serde(default)]
        from_s: Option<f64>,
        #[serde(default)]
        to_s: Option<f64>,
        #[serde(default)]
        seed: u64,
        #[serde(default = "yes")]
        coherent: bool,
    },
    /// Impulsive noise (switch-mode supplies, motors, railway traction).
    Impulsive(ImpulsiveConfig),
    /// Atmospheric noise (ITU-R P.372 level, Middleton Class A statistics).
    Atmospheric(AtmosConfig),
    /// Gaussian noise band-limited to `centre_hz ± bw_hz/2` by a FIR filter, total RMS
    /// `rms_uv_m`.
    BandNoise {
        centre_hz: f64,
        bw_hz: f64,
        rms_uv_m: f64,
        #[serde(default = "d_taps")]
        taps: usize,
        #[serde(default)]
        seed: u64,
        #[serde(default)]
        coherent: bool,
    },
}

pub(crate) fn yes() -> bool {
    true
}
fn d_taps() -> usize {
    255
}

/// Global index of the first sample at or after scene time `t0` (the scene's blocks start
/// on sample instants, so this is the block's first sample number).
pub fn sample_index(t0: Ps, fs_hz: f64) -> i64 {
    let num = t0.0 as i128 * fs_hz as i128;
    let p = PS_PER_S as i128;
    (-((-num).div_euclid(p))) as i64
}

/// Seed of channel `ch`'s stream: the same for all channels when `coherent`.
pub fn chan_seed(seed: u64, ch: usize, coherent: bool) -> u64 {
    let k = if coherent { 0 } else { ch as u64 + 1 };
    let mut z = seed ^ k.wrapping_mul(0xD1B5_4A32_D192_ED03);
    // splitmix64 finaliser
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Per-channel state, created lazily from a constructor.
pub(crate) struct PerChannel<S> {
    states: Vec<S>,
}

impl<S> PerChannel<S> {
    pub fn new() -> Self {
        PerChannel { states: Vec::new() }
    }
    pub fn get(&mut self, ch: usize, mut make: impl FnMut(usize) -> S) -> &mut S {
        while self.states.len() <= ch {
            let k = self.states.len();
            self.states.push(make(k));
        }
        &mut self.states[ch]
    }
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
        Wgn {
            sigma: rms_20k * ((fs_hz / 2.0) / NOISE_BW_HZ).sqrt(),
            seed,
            rngs: vec![],
        }
    }
}

impl Interferer for Wgn {
    fn name(&self) -> String {
        "wgn".into()
    }
    fn add(&mut self, _t0: Ps, _fs_hz: f64, ch: usize, out: &mut [f64]) {
        while self.rngs.len() <= ch {
            let k = self.rngs.len() as u64;
            self.rngs.push(ChaCha8Rng::seed_from_u64(
                self.seed.wrapping_mul(0x9E37_79B9).wrapping_add(k),
            ));
        }
        let rng = &mut self.rngs[ch];
        for v in out.iter_mut() {
            *v += self.sigma * gauss(rng);
        }
    }
}

/// Build a source. `scene_seed` is mixed into stochastic sources so scenes are reproducible.
pub fn build(
    cfg: &InterfConfig,
    fs_hz: f64,
    scene_seed: u64,
) -> Result<Box<dyn Interferer>, String> {
    Ok(match cfg {
        InterfConfig::Cw {
            freq_hz,
            rms_uv_m,
            phase_deg,
            from_s,
            to_s,
        } => Box::new(Cw {
            freq_hz: *freq_hz,
            amp: rms_uv_m * std::f64::consts::SQRT_2,
            phase: phase_deg.to_radians(),
            from_s: from_s.unwrap_or(f64::NEG_INFINITY),
            to_s: to_s.unwrap_or(f64::INFINITY),
        }),
        InterfConfig::Wgn {
            rms_uv_m_20khz,
            seed,
        } => Box::new(Wgn::new(*rms_uv_m_20khz, fs_hz, seed ^ scene_seed)),
        InterfConfig::Pulsed {
            period_s,
            width_s,
            duty,
            offset_s,
            freq_hz,
            rms_uv_m,
            phase_deg,
            from_s,
            to_s,
            seed,
            coherent,
        } => Box::new(pulsed::Pulsed::new(
            *period_s,
            pulsed::width(*period_s, *width_s, *duty)?,
            *offset_s,
            *freq_hz,
            *rms_uv_m,
            phase_deg.to_radians(),
            from_s.unwrap_or(f64::NEG_INFINITY),
            to_s.unwrap_or(f64::INFINITY),
            fs_hz,
            seed ^ scene_seed,
            *coherent,
        )?),
        InterfConfig::Impulsive(c) => Box::new(impulsive::Impulsive::new(c, scene_seed)?),
        InterfConfig::Atmospheric(c) => Box::new(atmos::Atmospheric::new(c, fs_hz, scene_seed)?),
        InterfConfig::BandNoise {
            centre_hz,
            bw_hz,
            rms_uv_m,
            taps,
            seed,
            coherent,
        } => Box::new(band::BandNoise::new(
            *centre_hz,
            *bw_hz,
            *rms_uv_m,
            *taps,
            fs_hz,
            seed ^ scene_seed,
            *coherent,
        )?),
    })
}

/// Fraction of the band `[f1, f2]` that lies in the receiver band (100 kHz ± 10 kHz).
pub fn rx_band_overlap(f1: f64, f2: f64) -> f64 {
    let a = RX_CENTRE_HZ - NOISE_BW_HZ / 2.0;
    let b = RX_CENTRE_HZ + NOISE_BW_HZ / 2.0;
    if f2 <= f1 {
        return 0.0;
    }
    ((f2.min(b) - f1.max(a)).max(0.0)) / (f2 - f1)
}

/// Mean-square noise (µV²/m²) of one source in the 20 kHz receiver band, `None` for
/// deterministic sources (CW, pulsed carrier).
pub fn noise_power_20k(c: &InterfConfig) -> Result<Option<f64>, String> {
    Ok(match c {
        InterfConfig::Wgn { rms_uv_m_20khz, .. } => Some(rms_uv_m_20khz.powi(2)),
        InterfConfig::Pulsed {
            freq_hz: None,
            period_s,
            width_s,
            duty,
            rms_uv_m,
            ..
        } => Some(rms_uv_m.powi(2) * pulsed::width(*period_s, *width_s, *duty)? / period_s),
        InterfConfig::Pulsed { .. } | InterfConfig::Cw { .. } => None,
        InterfConfig::Impulsive(c) => {
            Some(impulsive::Resolved::from_config(c)?.mean_power_rx_band())
        }
        InterfConfig::Atmospheric(c) => Some(c.rms_uv_m_20khz()?.powi(2)),
        InterfConfig::BandNoise {
            centre_hz,
            bw_hz,
            rms_uv_m,
            ..
        } => Some(
            rms_uv_m.powi(2) * rx_band_overlap(centre_hz - bw_hz / 2.0, centre_hz + bw_hz / 2.0),
        ),
    })
}

/// RMS noise in 20 kHz of the stochastic sources of a configuration (for SNR in the truth):
/// WGN, wideband pulsed (time-averaged), impulsive (long-run average, see
/// [`impulsive::Resolved::mean_power_rx_band`]), atmospheric and the in-band part of
/// band-limited noise. Invalid configurations count zero (they fail in [`build`]).
pub fn noise_rms_20k(cfgs: &[InterfConfig]) -> f64 {
    cfgs.iter()
        .map(|c| noise_power_20k(c).ok().flatten().unwrap_or(0.0))
        .sum::<f64>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_index_inverts_scene_block_start() {
        for fs in [400_000i128, 2_000_000, 3_000_001] {
            for n0 in [0i128, 1, 7, 12_345, 99_999_999] {
                let t = Ps(((n0 * PS_PER_S as i128) / fs) as i64);
                assert_eq!(sample_index(t, fs as f64) as i128, n0);
            }
        }
    }

    #[test]
    fn overlap() {
        assert_eq!(rx_band_overlap(90e3, 110e3), 1.0);
        assert_eq!(rx_band_overlap(100e3, 140e3), 0.25);
        assert_eq!(rx_band_overlap(150e3, 160e3), 0.0);
    }
}
