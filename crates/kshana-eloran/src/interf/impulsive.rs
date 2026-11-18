// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Impulsive man-made noise (MODELLED): Poisson-timed decaying oscillations.
//!
//! Each impulse is `A · exp(−t/τ) · sin(2π f_r t + φ)` for `t ≥ 0` (cut at 12 τ), the
//! response of the coupling path / antenna resonance to a fast switching edge or arc. Impulse
//! times form a Poisson process of rate `rate_hz`; amplitudes are log-normal (median
//! `median_uv_m`, spread `sigma_db`, the usual description of man-made impulsive noise
//! amplitude distributions, cf. ITU-R P.372 §5 and Middleton 1977); `φ` uniform.
//!
//! Optional **bursts** (a Neyman–Scott cluster process) model pantograph arcing on electric
//! railways: bursts start at Poisson times (rate `bursts.rate_hz`) inside the passage
//! window, last an exponentially distributed time (mean `mean_duration_ms`), contain
//! impulses at `impulse_rate_hz` with `gain_db` more amplitude, and are tapered by
//! `sin²(π (t−from)/(to−from))` over the window (train approaching and receding).
//!
//! Presets (representative values chosen for a source a few tens of metres from the
//! antenna; not measured; scale with `gain_db`):
//!
//! | preset             | rate     | median  | σ    | ring    | decay  | bursts                               |
//! |--------------------|----------|---------|------|---------|--------|--------------------------------------|
//! | `smps`             | 2 kHz    | 200 µV/m| 4 dB | 150 kHz | 5 µs   | —                                    |
//! | `motor`            | 5 kHz    | 100 µV/m| 8 dB | 250 kHz | 2 µs   | —                                    |
//! | `railway_traction` | 600 Hz   | 300 µV/m| 6 dB | 90 kHz  | 20 µs  | 3/s, 20 ms, 20 k impulses/s, +20 dB  |
//!
//! (600 Hz = 12-pulse converter commutation at 50 Hz.) `custom` needs `rate_hz`,
//! `median_uv_m`, `ring_hz` and `decay_us`.
//!
//! State (RNG streams, next event times, ringing impulses) persists across blocks and the
//! two processes use separate RNG streams, so the output is independent of the block size.

use super::{chan_seed, sample_index, yes, Interferer, PerChannel, NOISE_BW_HZ, RX_CENTRE_HZ};
use crate::time::Ps;
use crate::timebase::gauss;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::f64::consts::{LN_10, PI, TAU};

/// Impulse length in decay time constants.
pub const CUT_TAU: f64 = 12.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpulsePreset {
    #[default]
    Custom,
    Smps,
    Motor,
    RailwayTraction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurstConfig {
    pub rate_hz: f64,
    pub mean_duration_ms: f64,
    pub impulse_rate_hz: f64,
    /// Amplitude gain of burst impulses over the background median (dB).
    pub gain_db: f64,
    #[serde(default)]
    pub from_s: Option<f64>,
    #[serde(default)]
    pub to_s: Option<f64>,
    #[serde(default = "yes")]
    pub taper: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImpulsiveConfig {
    #[serde(default)]
    pub preset: ImpulsePreset,
    #[serde(default)]
    pub rate_hz: Option<f64>,
    #[serde(default)]
    pub median_uv_m: Option<f64>,
    #[serde(default)]
    pub sigma_db: Option<f64>,
    #[serde(default)]
    pub ring_hz: Option<f64>,
    #[serde(default)]
    pub decay_us: Option<f64>,
    /// Overall amplitude scaling (dB).
    #[serde(default)]
    pub gain_db: f64,
    /// Window of the background impulses.
    #[serde(default)]
    pub from_s: Option<f64>,
    #[serde(default)]
    pub to_s: Option<f64>,
    #[serde(default)]
    pub bursts: Option<BurstConfig>,
    /// Train passage window (sets the burst window; required by `railway_traction`
    /// unless `bursts` gives one).
    #[serde(default)]
    pub passage_from_s: Option<f64>,
    #[serde(default)]
    pub passage_to_s: Option<f64>,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "yes")]
    pub coherent: bool,
}

impl ImpulsiveConfig {
    /// A preset with all other fields at their defaults.
    pub fn preset(p: ImpulsePreset) -> ImpulsiveConfig {
        ImpulsiveConfig {
            preset: p,
            rate_hz: None,
            median_uv_m: None,
            sigma_db: None,
            ring_hz: None,
            decay_us: None,
            gain_db: 0.0,
            from_s: None,
            to_s: None,
            bursts: None,
            passage_from_s: None,
            passage_to_s: None,
            seed: 0,
            coherent: true,
        }
    }
}

/// Resolved burst parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBurst {
    pub rate_hz: f64,
    pub mean_dur_s: f64,
    pub impulse_rate_hz: f64,
    pub gain: f64,
    pub from: f64,
    pub to: f64,
    pub taper: bool,
}

/// Fully resolved parameters (preset + overrides).
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub rate_hz: f64,
    /// Median amplitude including `gain_db` (µV/m).
    pub median: f64,
    pub sigma_ln: f64,
    pub ring_hz: f64,
    pub tau_s: f64,
    pub from: f64,
    pub to: f64,
    pub bursts: Option<ResolvedBurst>,
    pub seed: u64,
    pub coherent: bool,
}

impl Resolved {
    pub fn from_config(c: &ImpulsiveConfig) -> Result<Resolved, String> {
        // (rate, median, sigma, ring, decay_us)
        let p: Option<(f64, f64, f64, f64, f64)> = match c.preset {
            ImpulsePreset::Custom => None,
            ImpulsePreset::Smps => Some((2000.0, 200.0, 4.0, 150e3, 5.0)),
            ImpulsePreset::Motor => Some((5000.0, 100.0, 8.0, 250e3, 2.0)),
            ImpulsePreset::RailwayTraction => Some((600.0, 300.0, 6.0, 90e3, 20.0)),
        };
        let need = |o: Option<f64>, i: usize, n: &str| -> Result<f64, String> {
            o.or(p.map(|t| [t.0, t.1, t.2, t.3, t.4][i]))
                .ok_or_else(|| format!("impulsive: custom preset needs {n}"))
        };
        let rate = need(c.rate_hz, 0, "rate_hz")?;
        let median = need(c.median_uv_m, 1, "median_uv_m")?;
        let sigma_db = c.sigma_db.or(p.map(|t| t.2)).unwrap_or(6.0);
        let ring = need(c.ring_hz, 3, "ring_hz")?;
        let decay_us = need(c.decay_us, 4, "decay_us")?;
        if !(rate >= 0.0 && median >= 0.0 && sigma_db >= 0.0 && ring > 0.0 && decay_us > 0.0) {
            return Err("impulsive: rate, median, sigma ≥ 0; ring_hz, decay_us > 0".into());
        }
        let mut bursts = c.bursts.clone();
        if bursts.is_none() && c.preset == ImpulsePreset::RailwayTraction {
            bursts = Some(BurstConfig {
                rate_hz: 3.0,
                mean_duration_ms: 20.0,
                impulse_rate_hz: 20_000.0,
                gain_db: 20.0,
                from_s: None,
                to_s: None,
                taper: true,
            });
        }
        let bursts = match bursts {
            None => None,
            Some(b) => {
                let from = c.passage_from_s.or(b.from_s);
                let to = c.passage_to_s.or(b.to_s);
                if c.preset == ImpulsePreset::RailwayTraction && (from.is_none() || to.is_none()) {
                    return Err("impulsive: railway_traction needs passage_from_s and passage_to_s".into());
                }
                let (from, to) = (from.unwrap_or(0.0).max(0.0), to.unwrap_or(f64::INFINITY));
                if to.is_nan() || to <= from {
                    return Err("impulsive: burst/passage window must have to > from".into());
                }
                if !(b.rate_hz > 0.0 && b.mean_duration_ms > 0.0 && b.impulse_rate_hz > 0.0) {
                    return Err("impulsive: burst rate, duration and impulse rate must be positive".into());
                }
                Some(ResolvedBurst {
                    rate_hz: b.rate_hz,
                    mean_dur_s: b.mean_duration_ms * 1e-3,
                    impulse_rate_hz: b.impulse_rate_hz,
                    gain: 10f64.powf(b.gain_db / 20.0),
                    from,
                    to,
                    taper: b.taper && to.is_finite(),
                })
            }
        };
        Ok(Resolved {
            rate_hz: rate,
            median: median * 10f64.powf(c.gain_db / 20.0),
            sigma_ln: sigma_db * LN_10 / 20.0,
            ring_hz: ring,
            tau_s: decay_us * 1e-6,
            from: c.from_s.unwrap_or(0.0).max(0.0),
            to: c.to_s.unwrap_or(f64::INFINITY),
            bursts,
            seed: c.seed,
            coherent: c.coherent,
        })
    }

    /// Fraction of one impulse's energy in `[f1, f2]` (one-sided; random phase): the
    /// Lorentzian lines at ±f_r of half-width `γ = 1/(2πτ)`, folded to positive frequency.
    pub fn band_fraction(&self, f1: f64, f2: f64) -> f64 {
        let g = 1.0 / (TAU * self.tau_s);
        let fr = self.ring_hz;
        let a = |f: f64| ((f - fr) / g).atan() + ((f + fr) / g).atan();
        (a(f2) - a(f1)) / PI
    }

    /// Mean energy of one background impulse (µV²/m²·s): `E[A²] τ/4`.
    pub fn impulse_energy(&self) -> f64 {
        self.median.powi(2) * (2.0 * self.sigma_ln.powi(2)).exp() * self.tau_s / 4.0
    }

    /// Long-run mean-square field (µV²/m²) in the receiver band (100 kHz ± 10 kHz): the
    /// background while active plus the burst process averaged over its window (with the
    /// sin² amplitude taper, power ∝ sin⁴, mean 3/8).
    pub fn mean_power_rx_band(&self) -> f64 {
        let frac = self.band_fraction(RX_CENTRE_HZ - NOISE_BW_HZ / 2.0, RX_CENTRE_HZ + NOISE_BW_HZ / 2.0);
        let e = self.impulse_energy() * frac;
        let mut p = self.rate_hz * e;
        if let Some(b) = &self.bursts {
            let duty = b.mean_dur_s / (b.mean_dur_s + 1.0 / b.rate_hz);
            let taper = if b.taper { 3.0 / 8.0 } else { 1.0 };
            p += b.impulse_rate_hz * duty * b.gain.powi(2) * taper * e;
        }
        p
    }
}

#[derive(Clone, Copy, Debug)]
struct Imp {
    t: f64,
    amp: f64,
    phase: f64,
}

struct State {
    bg: ChaCha8Rng,
    br: ChaCha8Rng,
    next_bg: f64,
    next_start: f64,
    burst_end: f64,
    next_imp: f64,
    active: Vec<Imp>,
}

fn expo(rng: &mut ChaCha8Rng, rate: f64) -> f64 {
    let u: f64 = 1.0 - rng.gen::<f64>(); // (0, 1]
    -u.ln() / rate
}

pub struct Impulsive {
    pub p: Resolved,
    seed: u64,
    states: PerChannel<State>,
}

impl Impulsive {
    pub fn new(c: &ImpulsiveConfig, scene_seed: u64) -> Result<Impulsive, String> {
        let p = Resolved::from_config(c)?;
        Ok(Impulsive { seed: p.seed ^ scene_seed, p, states: PerChannel::new() })
    }

    fn make_state(&self, ch: usize) -> State {
        let s = chan_seed(self.seed, ch, self.p.coherent);
        let mut bg = ChaCha8Rng::seed_from_u64(s ^ 0xB6);
        let mut br = ChaCha8Rng::seed_from_u64(s ^ 0xB0057);
        let next_bg = if self.p.rate_hz > 0.0 { self.p.from + expo(&mut bg, self.p.rate_hz) } else { f64::INFINITY };
        let next_start = match &self.p.bursts {
            Some(b) => b.from + expo(&mut br, b.rate_hz),
            None => f64::INFINITY,
        };
        State { bg, br, next_bg, next_start, burst_end: f64::NEG_INFINITY, next_imp: f64::INFINITY, active: vec![] }
    }

    fn amplitude(&self, rng: &mut ChaCha8Rng) -> (f64, f64) {
        let a = self.p.median * (self.p.sigma_ln * gauss(rng)).exp();
        let ph = rng.gen::<f64>() * TAU;
        (a, ph)
    }
}

impl Interferer for Impulsive {
    fn name(&self) -> String {
        "impulsive".into()
    }
    fn add(&mut self, t0: Ps, fs: f64, ch: usize, out: &mut [f64]) {
        let g0 = sample_index(t0, fs);
        let tb = (g0 + out.len() as i64) as f64 / fs;
        // detach the per-channel states so `self` can be borrowed while they mutate
        let mut states = std::mem::replace(&mut self.states, PerChannel::new());
        {
            let s = states.get(ch, |k| self.make_state(k));
            // background events up to the block end
            let mut fresh = vec![];
            while s.next_bg < tb {
                let t = s.next_bg;
                let (a, ph) = self.amplitude(&mut s.bg);
                if t < self.p.to {
                    fresh.push(Imp { t, amp: a, phase: ph });
                }
                s.next_bg = if t < self.p.to { t + expo(&mut s.bg, self.p.rate_hz) } else { f64::INFINITY };
            }
            // burst process
            if let Some(b) = self.p.bursts.clone() {
                loop {
                    if s.next_imp < s.burst_end {
                        if s.next_imp >= tb {
                            break;
                        }
                        let t = s.next_imp;
                        let (a, ph) = self.amplitude(&mut s.br);
                        let taper = if b.taper {
                            let x = (PI * (t - b.from) / (b.to - b.from)).sin();
                            x * x
                        } else {
                            1.0
                        };
                        fresh.push(Imp { t, amp: a * b.gain * taper, phase: ph });
                        s.next_imp = t + expo(&mut s.br, b.impulse_rate_hz);
                        continue;
                    }
                    if s.next_start < tb && s.next_start < b.to {
                        let t = s.next_start;
                        let dur = expo(&mut s.br, 1.0 / b.mean_dur_s);
                        s.burst_end = (t + dur).min(b.to);
                        s.next_imp = t;
                        s.next_start = t + dur + expo(&mut s.br, b.rate_hz);
                        continue;
                    }
                    break;
                }
            }
            s.active.extend(fresh);
            s.active.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
            // render
            let tau = self.p.tau_s;
            let w = TAU * self.p.ring_hz;
            let len = out.len() as i64;
            for imp in &s.active {
                let n_a = ((imp.t * fs).ceil() as i64 - g0).max(0);
                let n_b = (((imp.t + CUT_TAU * tau) * fs).floor() as i64 - g0 + 1).min(len);
                for n in n_a..n_b {
                    let dt = (g0 + n) as f64 / fs - imp.t;
                    if dt < 0.0 {
                        continue;
                    }
                    out[n as usize] += imp.amp * (-dt / tau).exp() * (w * dt + imp.phase).sin();
                }
            }
            s.active.retain(|i| i.t + CUT_TAU * tau >= tb);
        }
        self.states = states;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interf::dsp::band_power;

    fn run(src: &mut Impulsive, fs: f64, n: usize, block: usize, ch: usize) -> Vec<f64> {
        let mut x = vec![0.0; n];
        let mut i = 0;
        while i < n {
            let m = block.min(n - i);
            let t0 = Ps(((i as i128 * 1_000_000_000_000) / fs as i128) as i64);
            src.add(t0, fs, ch, &mut x[i..i + m]);
            i += m;
        }
        x
    }

    #[test]
    fn in_band_level_matches_analytic() {
        let fs = 400_000.0;
        let mut c = ImpulsiveConfig::preset(ImpulsePreset::Custom);
        c.rate_hz = Some(500.0);
        c.median_uv_m = Some(100.0);
        c.sigma_db = Some(3.0);
        c.ring_hz = Some(100e3);
        c.decay_us = Some(20.0);
        let mut s = Impulsive::new(&c, 9).unwrap();
        let x = run(&mut s, fs, 800_000, 4000, 0);
        let meas = band_power(&x, fs, 90e3, 110e3, 400);
        let pred = s.p.mean_power_rx_band();
        let err_db = 10.0 * (meas / pred).log10();
        assert!(err_db.abs() < 1.0, "measured {meas:.2} predicted {pred:.2} ({err_db:.2} dB)");
        // the spectrum peaks at the ring frequency
        let off = band_power(&x, fs, 40e3, 60e3, 400);
        assert!(meas > 20.0 * off);
    }

    #[test]
    fn block_size_independent_and_seeded() {
        let fs = 400_000.0;
        let mut c = ImpulsiveConfig::preset(ImpulsePreset::RailwayTraction);
        c.passage_from_s = Some(0.05);
        c.passage_to_s = Some(0.35);
        let a = run(&mut Impulsive::new(&c, 1).unwrap(), fs, 160_000, 4000, 0);
        let b = run(&mut Impulsive::new(&c, 1).unwrap(), fs, 160_000, 1237, 0);
        assert_eq!(a, b);
        let d = run(&mut Impulsive::new(&c, 2).unwrap(), fs, 160_000, 4000, 0);
        assert_ne!(a, d);
        // coherent: the same waveform on another channel
        let mut s = Impulsive::new(&c, 1).unwrap();
        let _ = run(&mut s, fs, 160_000, 4000, 0);
        let e = run(&mut s, fs, 160_000, 4000, 1);
        assert_eq!(a, e);
    }

    #[test]
    fn railway_bursts_inside_passage_only() {
        let fs = 400_000.0;
        let mut c = ImpulsiveConfig::preset(ImpulsePreset::RailwayTraction);
        c.passage_from_s = Some(1.0);
        c.passage_to_s = Some(3.0);
        assert!(Resolved::from_config(&c).unwrap().bursts.is_some());
        let mut c2 = c.clone();
        c2.median_uv_m = None;
        c2.rate_hz = Some(0.0);
        let x = run(&mut Impulsive::new(&c2, 4).unwrap(), fs, 1_600_000, 4000, 0);
        let before: f64 = x[..400_000].iter().map(|v| v * v).sum();
        let during: f64 = x[600_000..1_000_000].iter().map(|v| v * v).sum();
        assert_eq!(before, 0.0);
        assert!(during > 0.0);
        // missing passage window is an error
        let mut c3 = ImpulsiveConfig::preset(ImpulsePreset::RailwayTraction);
        c3.passage_to_s = Some(1.0);
        assert!(Resolved::from_config(&c3).is_err());
        assert!(Resolved::from_config(&ImpulsiveConfig::preset(ImpulsePreset::Custom)).is_err());
    }
}
