// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Atmospheric (lightning) noise for the LF band (MODELLED).
//!
//! # Level: ITU-R P.372
//!
//! The level is set by the median external noise factor `Fam` (dB above kT₀b) and
//! converted to a field strength in the receiver bandwidth with the ITU-R P.372 relation
//! (§ "Relation between noise factor and field strength"):
//!
//! ```text
//! En = Fa + 20 log10(f_MHz) + 10 log10(b_Hz) − 95.5     dB(µV/m)
//! ```
//!
//! so at 100 kHz in 20 kHz, `En = Fam − 72.5 dB(µV/m)`. Give `fam_db` directly,
//! `rms_uv_m_20khz` directly, or a `season` and local `time_block` to look up
//! [`FAM_100KHZ_MIDLAT`]: an **approximate** table of Fam at 100 kHz for a northern
//! mid-latitude (≈ 50° N, Europe) site, built by hand from the shape of the P.372 1 MHz
//! seasonal maps and the frequency-dependence curves. It is indicative only (MODELLED,
//! not read point-by-point from P.372, which should be used for real studies, giving
//! `fam_db`); `fam_offset_db` shifts it for other sites. The Fam value is the median
//! of the hourly values; upper/lower deciles (Du, Dl ≈ ±6–10 dB) are not modelled.
//!
//! # Statistics: Middleton Class A
//!
//! Atmospheric noise is impulsive. The samples follow Middleton's canonical Class A model
//! (Middleton 1977, *IEEE Trans. EMC* 19(3) "Statistical-physical models of
//! electromagnetic interference"): the noise is Gaussian with a variance that depends on
//! the number `m` of simultaneously active impulses, `m ~ Poisson(A)`:
//!
//! ```text
//! σ_m² = σ² (m/A + Γ) / (1 + Γ),    p(x) = Σ_m e^{−A} A^m/m! · N(x; 0, σ_m²)
//! ```
//!
//! with impulsive index `A` (small = more impulsive) and Gaussian-to-impulsive power
//! ratio `Γ`. Here `m` is redrawn every `dwell_us` (default 50 µs ≈ 1/20 kHz, the time
//! scale on which the receiver resolves impulses), so the impulsive structure survives
//! the receiver's band-limiting; within a dwell the samples are white Gaussian. The total
//! variance is σ² whatever A and Γ, so the RMS in 20 kHz is exactly the configured level.
//! The kurtosis of the full-band samples is `3 (1/A + 1 + 2Γ + Γ²)/(1+Γ)²`. Defaults
//! `A = 0.25`, `Γ = 0.02` are within the range Middleton reports for LF atmospherics;
//! they are not fitted to measurements.

use super::{chan_seed, sample_index, Interferer, PerChannel, NOISE_BW_HZ};
use crate::time::Ps;
use crate::timebase::gauss;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

/// Approximate median Fam (dB) at 100 kHz, northern mid-latitude (≈ 50° N), by season
/// (rows: winter, spring, summer, autumn) and 4-hour local-time block (columns: 00–04,
/// 04–08, 08–12, 12–16, 16–20, 20–24). MODELLED, indicative only (see module doc).
pub const FAM_100KHZ_MIDLAT: [[f64; 6]; 4] = [
    [88.0, 84.0, 74.0, 76.0, 86.0, 90.0],
    [94.0, 90.0, 78.0, 80.0, 90.0, 96.0],
    [100.0, 96.0, 84.0, 88.0, 98.0, 102.0],
    [96.0, 92.0, 80.0, 82.0, 92.0, 98.0],
];

/// Look up [`FAM_100KHZ_MIDLAT`]; `block` is "00-04", "04-08", …, "20-24".
pub fn fam_table(season: Season, block: &str) -> Result<f64, String> {
    let blocks = ["00-04", "04-08", "08-12", "12-16", "16-20", "20-24"];
    let i = blocks
        .iter()
        .position(|b| *b == block.trim())
        .ok_or_else(|| format!("atmospheric: time_block {block:?} must be one of {blocks:?}"))?;
    let r = match season {
        Season::Winter => 0,
        Season::Spring => 1,
        Season::Summer => 2,
        Season::Autumn => 3,
    };
    Ok(FAM_100KHZ_MIDLAT[r][i])
}

/// ITU-R P.372 noise-factor to field-strength relation: RMS field (µV/m) in bandwidth `b_hz` for noise factor `fa_db`.
pub fn fa_to_uv_m(fa_db: f64, f_mhz: f64, b_hz: f64) -> f64 {
    10f64.powf((fa_db + 20.0 * f_mhz.log10() + 10.0 * b_hz.log10() - 95.5) / 20.0)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtmosConfig {
    #[serde(default)]
    pub fam_db: Option<f64>,
    #[serde(default)]
    pub rms_uv_m_20khz: Option<f64>,
    #[serde(default)]
    pub season: Option<Season>,
    #[serde(default)]
    pub time_block: Option<String>,
    #[serde(default)]
    pub fam_offset_db: f64,
    /// Frequency at which Fam applies (MHz).
    #[serde(default = "d_f")]
    pub freq_mhz: f64,
    /// Middleton Class A impulsive index A and Gaussian ratio Γ.
    #[serde(default = "d_a")]
    pub a_index: f64,
    #[serde(default = "d_gamma")]
    pub gamma: f64,
    #[serde(default = "d_dwell")]
    pub dwell_us: f64,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub coherent: bool,
}

fn d_f() -> f64 {
    0.1
}
fn d_a() -> f64 {
    0.25
}
fn d_gamma() -> f64 {
    0.02
}
fn d_dwell() -> f64 {
    50.0
}

impl AtmosConfig {
    pub fn with_fam(fam_db: f64) -> AtmosConfig {
        AtmosConfig {
            fam_db: Some(fam_db),
            rms_uv_m_20khz: None,
            season: None,
            time_block: None,
            fam_offset_db: 0.0,
            freq_mhz: d_f(),
            a_index: d_a(),
            gamma: d_gamma(),
            dwell_us: d_dwell(),
            seed: 0,
            coherent: false,
        }
    }

    /// RMS field in 20 kHz (µV/m) from whichever level was given (exactly one).
    pub fn rms_uv_m_20khz(&self) -> Result<f64, String> {
        let table = match (&self.season, &self.time_block) {
            (Some(s), Some(b)) => Some(fam_table(*s, b)? + self.fam_offset_db),
            (None, None) => None,
            _ => return Err("atmospheric: give both season and time_block".into()),
        };
        let n = self.fam_db.is_some() as u8
            + self.rms_uv_m_20khz.is_some() as u8
            + table.is_some() as u8;
        if n != 1 {
            return Err(
                "atmospheric: give exactly one of fam_db, rms_uv_m_20khz or season+time_block"
                    .into(),
            );
        }
        if self.freq_mhz.is_nan() || self.freq_mhz <= 0.0 {
            return Err("atmospheric: freq_mhz must be positive".into());
        }
        Ok(match (self.fam_db.or(table), self.rms_uv_m_20khz) {
            (Some(f), _) => fa_to_uv_m(f, self.freq_mhz, NOISE_BW_HZ),
            (None, Some(r)) => r,
            _ => unreachable!(),
        })
    }
}

struct State {
    rng: ChaCha8Rng,
    dwell: i64,
    sigma: f64,
}

pub struct Atmospheric {
    /// Full-band σ (µV/m).
    pub sigma: f64,
    pub a: f64,
    pub gamma: f64,
    dwell_n: i64,
    seed: u64,
    coherent: bool,
    states: PerChannel<State>,
}

/// Poisson deviate (Knuth; fine for the small means used here).
fn poisson(rng: &mut ChaCha8Rng, mean: f64) -> u32 {
    let l = (-mean).exp();
    let mut k = 0u32;
    let mut p = 1.0;
    loop {
        p *= rng.gen::<f64>();
        if p <= l || k > 10_000 {
            return k;
        }
        k += 1;
    }
}

impl Atmospheric {
    pub fn new(c: &AtmosConfig, fs_hz: f64, scene_seed: u64) -> Result<Atmospheric, String> {
        let rms = c.rms_uv_m_20khz()?;
        if !(c.a_index > 0.0 && c.a_index <= 50.0 && c.gamma >= 0.0 && c.dwell_us > 0.0) {
            return Err("atmospheric: a_index in (0, 50], gamma ≥ 0, dwell_us > 0".into());
        }
        Ok(Atmospheric {
            sigma: rms * ((fs_hz / 2.0) / NOISE_BW_HZ).sqrt(),
            a: c.a_index,
            gamma: c.gamma,
            dwell_n: ((c.dwell_us * 1e-6 * fs_hz).round() as i64).max(1),
            seed: c.seed ^ scene_seed,
            coherent: c.coherent,
            states: PerChannel::new(),
        })
    }

    /// Theoretical kurtosis of the full-band samples.
    pub fn kurtosis(&self) -> f64 {
        let (a, g) = (self.a, self.gamma);
        3.0 * (1.0 / a + 1.0 + 2.0 * g + g * g) / (1.0 + g).powi(2)
    }
}

impl Interferer for Atmospheric {
    fn name(&self) -> String {
        format!("atmospheric class-A (A={}, Γ={})", self.a, self.gamma)
    }
    fn add(&mut self, t0: Ps, fs: f64, ch: usize, out: &mut [f64]) {
        let g0 = sample_index(t0, fs);
        let (seed, coherent) = (self.seed, self.coherent);
        let (sigma, a, gamma, dn) = (self.sigma, self.a, self.gamma, self.dwell_n);
        let s = self.states.get(ch, |k| State {
            rng: ChaCha8Rng::seed_from_u64(chan_seed(seed, k, coherent)),
            dwell: i64::MIN,
            sigma: 0.0,
        });
        for (n, v) in out.iter_mut().enumerate() {
            let d = (g0 + n as i64).div_euclid(dn);
            if d != s.dwell {
                s.dwell = d;
                let m = poisson(&mut s.rng, a) as f64;
                s.sigma = sigma * ((m / a + gamma) / (1.0 + gamma)).sqrt();
            }
            *v += s.sigma * gauss(&mut s.rng);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interf::dsp::band_rms;

    #[test]
    fn p372_conversion() {
        // at 100 kHz in 20 kHz: En = Fam − 72.49 dB(µV/m)
        let e = 20.0 * fa_to_uv_m(100.0, 0.1, 20_000.0).log10();
        assert!((e - 27.51).abs() < 0.01, "{e}");
        let c = AtmosConfig {
            season: Some(Season::Summer),
            time_block: Some("20-24".into()),
            fam_db: None,
            ..AtmosConfig::with_fam(0.0)
        };
        assert!((c.rms_uv_m_20khz().unwrap() - fa_to_uv_m(102.0, 0.1, 20e3)).abs() < 1e-9);
        assert!(AtmosConfig {
            rms_uv_m_20khz: Some(1.0),
            ..AtmosConfig::with_fam(80.0)
        }
        .rms_uv_m_20khz()
        .is_err());
        assert!(fam_table(Season::Winter, "03-07").is_err());
    }

    #[test]
    fn level_and_impulsiveness() {
        let fs = 400_000.0;
        let c = AtmosConfig {
            seed: 5,
            ..AtmosConfig::with_fam(95.0)
        };
        let mut s = Atmospheric::new(&c, fs, 0).unwrap();
        let mut x = vec![0.0; 1_200_000];
        for (i, blk) in x.chunks_mut(4000).enumerate() {
            let t0 = Ps(((i as i128 * 4000 * 1_000_000_000_000) / fs as i128) as i64);
            s.add(t0, fs, 0, blk);
        }
        let want = c.rms_uv_m_20khz().unwrap();
        let got = band_rms(&x, fs, 90e3, 110e3, 400);
        assert!((20.0 * (got / want).log10()).abs() < 0.5, "{got} vs {want}");
        // heavy tails: kurtosis well above Gaussian 3 and near theory (≈ 14.5)
        let m2 = x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64;
        let m4 = x.iter().map(|v| v.powi(4)).sum::<f64>() / x.len() as f64;
        let k = m4 / (m2 * m2);
        assert!(
            k > 8.0 && (k / s.kurtosis() - 1.0).abs() < 0.35,
            "kurtosis {k} theory {}",
            s.kurtosis()
        );
    }

    #[test]
    fn seeds_reproduce_and_channels_independent() {
        let fs = 400_000.0;
        let c = AtmosConfig::with_fam(90.0);
        let gen = |seed: u64, ch: usize| {
            let mut s = Atmospheric::new(&c, fs, seed).unwrap();
            let mut x = vec![0.0; 10_000];
            for k in 0..=ch {
                x.iter_mut().for_each(|v| *v = 0.0);
                s.add(Ps::ZERO, fs, k, &mut x);
            }
            x
        };
        assert_eq!(gen(1, 0), gen(1, 0));
        assert_ne!(gen(1, 0), gen(2, 0));
        assert_ne!(gen(1, 0), gen(1, 1));
    }
}
