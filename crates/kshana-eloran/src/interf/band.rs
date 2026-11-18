// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Band-limited Gaussian noise: white Gaussian samples through a linear-phase FIR
//! band-pass ([`super::dsp::fir_bandpass`]) centred on `centre_hz` with bandwidth `bw_hz`.
//! The input σ is set so the output variance (σ²·Σh²) equals `rms_uv_m²`. The filter
//! history is pre-filled at construction (stationary from the first sample) and carried
//! across blocks, so the output is independent of the block size.

use super::dsp::fir_bandpass;
use super::{chan_seed, Interferer, PerChannel};
use crate::time::Ps;
use crate::timebase::gauss;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

struct State {
    rng: ChaCha8Rng,
    /// Circular history of input samples.
    hist: Vec<f64>,
    pos: usize,
}

pub struct BandNoise {
    pub centre_hz: f64,
    pub bw_hz: f64,
    pub h: Vec<f64>,
    pub sigma_in: f64,
    seed: u64,
    coherent: bool,
    states: PerChannel<State>,
}

impl BandNoise {
    pub fn new(
        centre_hz: f64,
        bw_hz: f64,
        rms_uv_m: f64,
        taps: usize,
        fs_hz: f64,
        seed: u64,
        coherent: bool,
    ) -> Result<BandNoise, String> {
        let (f1, f2) = (centre_hz - bw_hz / 2.0, centre_hz + bw_hz / 2.0);
        if !(bw_hz > 0.0 && f1 > 0.0 && f2 < fs_hz / 2.0) {
            return Err(format!(
                "band_noise: band {f1}..{f2} Hz must lie inside (0, fs/2)"
            ));
        }
        if taps.is_multiple_of(2) || !(15..=4095).contains(&taps) {
            return Err("band_noise: taps must be odd, 15..4095".into());
        }
        if rms_uv_m.is_nan() || rms_uv_m < 0.0 {
            return Err("band_noise: rms_uv_m must be ≥ 0".into());
        }
        let h = fir_bandpass(f1, f2, fs_hz, taps);
        let e: f64 = h.iter().map(|v| v * v).sum();
        Ok(BandNoise {
            centre_hz,
            bw_hz,
            sigma_in: rms_uv_m / e.sqrt(),
            h,
            seed,
            coherent,
            states: PerChannel::new(),
        })
    }
}

impl Interferer for BandNoise {
    fn name(&self) -> String {
        format!(
            "band noise {:.0} Hz ± {:.0} Hz",
            self.centre_hz,
            self.bw_hz / 2.0
        )
    }
    fn add(&mut self, _t0: Ps, _fs: f64, ch: usize, out: &mut [f64]) {
        let (seed, coherent, sig, nt) = (self.seed, self.coherent, self.sigma_in, self.h.len());
        let s = self.states.get(ch, |k| {
            let mut rng = ChaCha8Rng::seed_from_u64(chan_seed(seed, k, coherent));
            let hist = (0..nt).map(|_| sig * gauss(&mut rng)).collect();
            State { rng, hist, pos: 0 }
        });
        for v in out.iter_mut() {
            s.hist[s.pos] = sig * gauss(&mut s.rng);
            // y[n] = Σ h[k] x[n−k]; x[n] at pos, x[n−k] at pos−k (circular)
            let mut acc = 0.0;
            let (newer, older) = s.hist.split_at(s.pos + 1);
            // k = 0..=pos  → newer reversed; k = pos+1.. → older reversed
            for (k, x) in newer.iter().rev().chain(older.iter().rev()).enumerate() {
                acc += self.h[k] * x;
            }
            *v += acc;
            s.pos = (s.pos + 1) % nt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interf::dsp::{band_power, band_rms};

    #[test]
    fn spectrum_matches_configuration() {
        let fs = 400_000.0;
        let mut b = BandNoise::new(80e3, 20e3, 10.0, 255, fs, 3, false).unwrap();
        let mut x = vec![0.0; 400_000];
        for blk in x.chunks_mut(3333) {
            b.add(Ps::ZERO, fs, 0, blk);
        }
        let total = (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
        assert!((total / 10.0 - 1.0).abs() < 0.03, "total rms {total}");
        let inband = band_rms(&x, fs, 70e3, 90e3, 400);
        assert!((inband / 10.0 - 1.0).abs() < 0.04, "in-band {inband}");
        let outside = band_power(&x, fs, 120e3, 160e3, 400) + band_power(&x, fs, 10e3, 50e3, 400);
        // < −23 dB of the band power (dominated by the rectangular-window leakage of the
        // estimator, not by the filter's −74 dB stop band)
        assert!(outside < 0.5, "leak {outside}");
        // half of the band overlaps the 90–110 kHz receiver band
        let cfg = crate::interf::InterfConfig::BandNoise {
            centre_hz: 90e3,
            bw_hz: 20e3,
            rms_uv_m: 10.0,
            taps: 255,
            seed: 0,
            coherent: false,
        };
        assert!((crate::interf::noise_rms_20k(&[cfg]) - 50f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn block_size_independent() {
        let fs = 400_000.0;
        let gen = |blk: usize| {
            let mut b = BandNoise::new(100e3, 5e3, 1.0, 127, fs, 7, false).unwrap();
            let mut x = vec![0.0; 20_000];
            for c in x.chunks_mut(blk) {
                b.add(Ps::ZERO, fs, 0, c);
            }
            x
        };
        assert_eq!(gen(1000), gen(777));
    }
}
