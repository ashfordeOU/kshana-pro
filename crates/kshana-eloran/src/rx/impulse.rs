// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Impulsive-noise mitigation ahead of the receiver, and the experiment that shows when it
//! matters (research 01 gap 10: LF atmospheric noise is impulsive, simulators and receiver
//! tests commonly treat it as Gaussian).
//!
//! * [`mitigate`]: per block of samples, a robust noise scale σ̂ = 1.4826 · median|x| (the
//!   MAD of a zero-mean signal); samples beyond `k · σ̂` are **clipped** to ±k·σ̂ or
//!   **blanked** (set to zero). The pulses themselves sit below the threshold at the SNRs
//!   where mitigation matters; at high SNR choose a larger `k` or leave it off.
//! * [`experiment`]: the same pulse train in Gaussian noise and in Middleton Class A noise
//!   ([`crate::interf::atmos`]) **at equal RMS power**, timed by the calibration matched
//!   filter, with and without mitigation. It quantifies the receiver-relevant difference that
//!   a power-only (Gaussian) noise model hides.
//!
//! Evidence: **MODELLED** (Class A statistics per Middleton 1977 as implemented in
//! `interf::atmos`; outcome checked by internal tests). The ranking (clipping helps a
//! linear receiver in impulsive noise, does not in Gaussian noise) is the textbook result of
//! locally optimum detection in Class A noise; the size of the gain depends on A, Γ and the
//! receiver bandwidth and must be validated against recorded LF noise.

use crate::hw::calib::{calibration_pulse_train, characterise_from_capture, CharacteriseOptions};
use crate::interf::atmos::{AtmosConfig, Atmospheric};
use crate::interf::Interferer;
use crate::time::Ps;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// Mitigation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Off,
    Clip,
    Blank,
}

/// Mitigation settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mitigation {
    pub mode: Mode,
    /// Threshold in robust noise scales.
    pub k: f64,
    /// Block length for the robust scale (samples).
    pub block: usize,
}

impl Default for Mitigation {
    fn default() -> Self {
        Mitigation {
            mode: Mode::Clip,
            k: 3.0,
            block: 4096,
        }
    }
}

/// Apply the mitigation in place; returns the fraction of samples changed.
pub fn mitigate(x: &mut [f64], m: &Mitigation) -> f64 {
    if m.mode == Mode::Off || x.is_empty() {
        return 0.0;
    }
    let mut changed = 0usize;
    for chunk in x.chunks_mut(m.block.max(64)) {
        let mut a: Vec<f64> = chunk.iter().map(|v| v.abs()).collect();
        let mid = a.len() / 2;
        a.select_nth_unstable_by(mid, |p, q| p.partial_cmp(q).unwrap());
        let sigma = 1.4826 * a[mid];
        if sigma <= 0.0 {
            continue;
        }
        let th = m.k * sigma;
        for v in chunk.iter_mut() {
            if v.abs() > th {
                changed += 1;
                *v = match m.mode {
                    Mode::Clip => th * v.signum(),
                    Mode::Blank => 0.0,
                    Mode::Off => *v,
                };
            }
        }
    }
    changed as f64 / x.len() as f64
}

/// Experiment settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExperimentConfig {
    pub fs_hz: f64,
    /// Pulses integrated (coherently averaged) per timing estimate, as a receiver integrates.
    pub pulses: usize,
    /// Independent timing estimates (trials) per cell.
    pub trials: usize,
    /// Pulse peak amplitude.
    pub amp: f64,
    /// Noise RMS (same for both noise models).
    pub noise_rms: f64,
    /// Class A parameters.
    pub a_index: f64,
    pub gamma: f64,
    pub mitigation: Mitigation,
    pub seed: u64,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        ExperimentConfig {
            fs_hz: 1e6,
            pulses: 64,
            trials: 40,
            amp: 0.02,
            noise_rms: 0.05,
            a_index: 0.25,
            gamma: 0.02,
            mitigation: Mitigation::default(),
            seed: 7,
        }
    }
}

/// RMS timing error (ns) for one combination.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub noise: String,
    pub mitigation: Mode,
    pub toa_rms_ns: f64,
    pub changed_fraction: f64,
}

/// Experiment result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExperimentReport {
    pub cells: Vec<Cell>,
    /// Class A excess kurtosis of the generated noise (Gaussian = 0).
    pub class_a_excess_kurtosis: f64,
    pub evidence: String,
}

fn kurt(x: &[f64]) -> f64 {
    let n = x.len() as f64;
    let m2 = x.iter().map(|v| v * v).sum::<f64>() / n;
    let m4 = x.iter().map(|v| v.powi(4)).sum::<f64>() / n;
    m4 / (m2 * m2) - 3.0
}

fn scale_to(x: &mut [f64], rms: f64) {
    let r = (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
    if r > 0.0 {
        x.iter_mut().for_each(|v| *v *= rms / r);
    }
}

/// Run the equal-power Gaussian vs Class A experiment: per trial, `pulses` pulses in noise,
/// mitigated (or not), coherently averaged over the 1 ms period, timed by the matched filter.
pub fn experiment(c: &ExperimentConfig) -> Result<ExperimentReport, String> {
    let period_n = (1e-3 * c.fs_hz).round() as usize;
    let lead = period_n / 4;
    let n = c.pulses * period_n;
    // One pulse per period, time origin `lead` samples into each period.
    let t_org = lead as f64 / c.fs_hz;
    let one = calibration_pulse_train(c.fs_hz, period_n, &[t_org], 0.0, c.amp);
    let mut rng = ChaCha8Rng::seed_from_u64(c.seed);
    let mut ac = AtmosConfig::with_fam(60.0);
    ac.a_index = c.a_index;
    ac.gamma = c.gamma;
    ac.dwell_us = 1e6 / c.fs_hz; // one Class A state per sample: impulses at the sample scale
    let mut gen = Atmospheric::new(&ac, c.fs_hz, c.seed)?;
    let mut kurts = vec![];
    let modes = [Mode::Off, Mode::Clip, Mode::Blank];
    let mut err: Vec<Vec<f64>> = vec![vec![]; 6];
    let mut changed = [0.0; 6];
    for trial in 0..c.trials {
        let mut gauss: Vec<f64> = (0..n)
            .map(|_| {
                use rand::Rng;
                let (u1, u2): (f64, f64) = (rng.gen::<f64>().max(1e-300), rng.gen());
                (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
            })
            .collect();
        scale_to(&mut gauss, c.noise_rms);
        let mut classa = vec![0.0; n];
        gen.add(
            Ps::from_s(trial as f64 * n as f64 / c.fs_hz),
            c.fs_hz,
            0,
            &mut classa,
        );
        scale_to(&mut classa, c.noise_rms);
        kurts.push(kurt(&classa));
        for (ni, noise) in [&gauss, &classa].into_iter().enumerate() {
            for (mi, mode) in modes.iter().enumerate() {
                let mut x: Vec<f64> = (0..n).map(|i| one[i % period_n] + noise[i]).collect();
                changed[ni * 3 + mi] += mitigate(
                    &mut x,
                    &Mitigation {
                        mode: *mode,
                        ..c.mitigation.clone()
                    },
                );
                let mut avg = vec![0.0; period_n];
                for (i, v) in x.iter().enumerate() {
                    avg[i % period_n] += v / c.pulses as f64;
                }
                let est = characterise_from_capture(
                    &avg,
                    c.fs_hz,
                    &[t_org],
                    &CharacteriseOptions {
                        expected_delay_ns: 0.0,
                        search_ns: 4000.0,
                    },
                )?;
                err[ni * 3 + mi].push(est.delay_ns);
            }
        }
    }
    let mut cells = vec![];
    for (ni, name) in ["gaussian", "class_a"].iter().enumerate() {
        for (mi, mode) in modes.iter().enumerate() {
            let e = &err[ni * 3 + mi];
            let rms = (e.iter().map(|d| d * d).sum::<f64>() / e.len() as f64).sqrt();
            cells.push(Cell {
                noise: (*name).into(),
                mitigation: *mode,
                toa_rms_ns: rms,
                changed_fraction: changed[ni * 3 + mi] / c.trials as f64,
            });
        }
    }
    Ok(ExperimentReport {
        cells,
        class_a_excess_kurtosis: kurts.iter().sum::<f64>() / kurts.len().max(1) as f64,
        evidence: "MODELLED: equal-power Gaussian vs Middleton Class A, coherent pulse averaging and matched-filter timing; validate the gain against recorded LF noise".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mitigation_clips_and_blanks_only_outliers() {
        let mut x = vec![0.1, -0.1, 0.12, -0.09, 5.0, 0.11, -0.1, -4.0];
        x.extend(std::iter::repeat_n([0.1, -0.1], 100).flatten());
        let mut y = x.clone();
        let f = mitigate(
            &mut y,
            &Mitigation {
                mode: Mode::Clip,
                k: 3.0,
                block: 4096,
            },
        );
        assert!(f > 0.0 && f < 0.02, "{f}");
        assert!(y[4] < 1.0 && y[4] > 0.3 && y[7] < -0.3);
        assert_eq!(y[0], x[0]);
        let mut z = x.clone();
        mitigate(
            &mut z,
            &Mitigation {
                mode: Mode::Blank,
                k: 3.0,
                block: 4096,
            },
        );
        assert_eq!(z[4], 0.0);
    }

    #[test]
    fn class_a_hurts_a_linear_receiver_and_clipping_recovers_it() {
        let r = experiment(&ExperimentConfig::default()).unwrap();
        let get = |n: &str, m: Mode| {
            r.cells
                .iter()
                .find(|c| c.noise == n && c.mitigation == m)
                .unwrap()
                .toa_rms_ns
        };
        assert!(
            r.class_a_excess_kurtosis > 3.0,
            "kurtosis {}",
            r.class_a_excess_kurtosis
        );
        let (g_off, g_clip) = (get("gaussian", Mode::Off), get("gaussian", Mode::Clip));
        let (a_off, a_clip) = (get("class_a", Mode::Off), get("class_a", Mode::Clip));
        // Equal power: a linear matched filter sees about the same error in both models.
        assert!(
            (a_off / g_off) > 0.6 && (a_off / g_off) < 1.6,
            "linear: gauss {g_off} vs class A {a_off}"
        );
        // Clipping recovers a large part of the impulsive case, and costs little in Gaussian.
        assert!(a_clip < 0.7 * a_off, "class A: off {a_off} clip {a_clip}");
        assert!(g_clip < 1.3 * g_off, "gaussian: off {g_off} clip {g_clip}");
    }
}
