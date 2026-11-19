// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! [`VirtualDevice`]: a virtual bench — a [`Device`] with transmit and receive whose
//! transmit output is looped back to its receive input through a configurable model of
//! real converter hardware, so calibration, synchronisation and health procedures can be
//! exercised end to end in CI without hardware.
//!
//! (The module is declared as `hw::virtual_bench` because `virtual` is a reserved word in
//! Rust; the file is `src/hw/virtual.rs`.)
//!
//! # Signal path (per sample, deterministic)
//!
//! ```text
//! write_tx ─▶ underrun gaps ─▶ DAC quantise/clip ─▶ reconstruction FIR ─▶ device clock
//!   (ppm, lock loss, jitter, PPS error, latency) ─▶ gain error ─▶ crosstalk ─▶
//!   anti-alias FIR ─▶ additive noise ─▶ ADC quantise/clip ─▶ read_rx
//! ```
//!
//! * **Time.** Virtual time starts at 0 s, which is a PPS edge of the *true* (reference)
//!   timeline. The receive side is modelled as an **ideal reference digitiser**: its sample
//!   `m` is taken at true time `t_rx0 + m / fs` exactly, as on a bench where the capture is
//!   locked to the reference 10 MHz and the true PPS. Everything the device under test does
//!   wrong (latency, PPS alignment error, clock offset, jitter, underrun gaps) therefore shows
//!   up in the capture, as it would on a time-interval counter or reference digitiser.
//! * **Transmit timing.** Device output sample `j` (counted from the transmit start, gaps
//!   included) is at true time `t_tx0 + Σ_{i<j} T_i` with `t_tx0 = start edge + PPS error +
//!   latency`. `T_i = 1 / (fs·(1 + ppm·10⁻⁶))`, where `ppm` is `locked_residual_ppm` while the
//!   device is locked to an external 10 MHz ([`RefSource::External`]) and `clock_offset_ppm`
//!   when it runs on its internal reference or inside a [`VirtualEvent::RefLockLoss`] window
//!   (holdover: the phase error accumulates and is not removed when the lock returns).
//! * **Reconstruction.** The analogue output between device samples is the band-limited
//!   (Kaiser-windowed sinc, ±`interp_half` taps, unit-DC-normalised) interpolation of the
//!   reconstruction-filtered DAC samples. The linear-phase reconstruction and anti-alias
//!   FIRs are delay-compensated by their exact group delay, so `latency_ns` is the *total*
//!   transmit-to-receive delay.
//! * **Jitter** is applied as a white Gaussian timing perturbation of the reconstructed
//!   waveform at each capture instant (first-order equivalent of DAC clock jitter).
//! * **Noise and jitter** are counter-based (a hash of seed, channel and sample index), so
//!   results do not depend on block sizes.
//!
//! Receive semantics: while the transmit stream is running, `read_rx` returns only samples
//! whose transmit input is complete and errors if none is (write more first); before the
//! transmit start and after `stop_tx` the output is idle (zero) and receive is unlimited.
//! Transmit history older than `history_samples` is discarded.
//!
//! Evidence: **MODELLED** — the impairments are standard first-order converter models
//! checked by internal tests (`calibration_recovers_latency_through_virtual_device`, etc.);
//! they are not fitted to any real device. This device is not real time
//! ([`Device::realtime`] is false): it runs in virtual time.

use super::fir::{kaiser, lowpass_taps, FirState};
use super::{Device, DeviceConfig, DeviceStatus, RefSource, StartTime};
use crate::scene::SampleSink;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::f64::consts::PI;

/// An event injected into the virtual device.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VirtualEvent {
    /// The transmit FIFO runs dry once `at_sample` data samples have been written: the
    /// output holds zero for `gap_samples` device samples, then continues with the (now
    /// delayed) data. Counted in [`DeviceStatus::underruns`].
    Underrun { at_sample: u64, gap_samples: u64 },
    /// The external 10 MHz reference is lost from device output sample `from_sample` to
    /// `to_sample` (counted from the transmit start): the sample clock free-runs at
    /// `clock_offset_ppm` and [`DeviceStatus::ref_locked`] reads `false` meanwhile.
    RefLockLoss { from_sample: u64, to_sample: u64 },
    /// A late-command (time) error is reported when `at_sample` data samples have been
    /// written. Counted in [`DeviceStatus::late`]; the data are not changed.
    Late { at_sample: u64 },
}

/// Impairments of a [`VirtualDevice`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VirtualConfig {
    /// Total transmit-to-receive delay (ns), filters included.
    pub latency_ns: f64,
    /// Free-running sample-clock error (ppm, positive = fast).
    pub clock_offset_ppm: f64,
    /// Residual sample-clock error while locked to an external reference (ppm).
    pub locked_residual_ppm: f64,
    /// RMS sample-clock jitter (ps).
    pub jitter_ps_rms: f64,
    /// DAC resolution (bits, 0 = unquantised). Output is clipped at ±1.0 either way.
    pub dac_bits: u32,
    /// ADC resolution (bits, 0 = unquantised). Input is clipped at ±1.0 either way.
    pub adc_bits: u32,
    /// Reconstruction filter cut-off (cycles/sample), `None` = no filter.
    pub reconstruction_cutoff: Option<f64>,
    /// Anti-alias filter cut-off (cycles/sample), `None` = no filter.
    pub anti_alias_cutoff: Option<f64>,
    /// Taps of each filter (made odd).
    pub filter_taps: usize,
    /// Output gain error per transmit channel (dB); missing entries = 0 dB.
    pub gain_error_db: Vec<f64>,
    /// Additive white Gaussian noise at the ADC (RMS, full-scale units).
    pub noise_rms: f64,
    /// Crosstalk from every transmit channel into every other receive channel (dB,
    /// amplitude, e.g. −60). `None` = none.
    pub crosstalk_db: Option<f64>,
    /// PPS alignment error of the device time (ns): added to every PPS-referenced start.
    pub pps_error_ns: f64,
    /// Injected events.
    pub events: Vec<VirtualEvent>,
    /// Board temperature reported by [`VirtualDevice::temperature_c`].
    pub temperature_c: Option<f64>,
    /// Seed of the noise and jitter.
    pub seed: u64,
    /// Half-length of the band-limited interpolator (taps each side).
    pub interp_half: usize,
    /// Transmit history kept for the receive side (device samples per channel).
    pub history_samples: usize,
}

impl Default for VirtualConfig {
    fn default() -> Self {
        VirtualConfig {
            latency_ns: 0.0,
            clock_offset_ppm: 0.0,
            locked_residual_ppm: 0.0,
            jitter_ps_rms: 0.0,
            dac_bits: 0,
            adc_bits: 0,
            reconstruction_cutoff: None,
            anti_alias_cutoff: None,
            filter_taps: 63,
            gain_error_db: vec![],
            noise_rms: 0.0,
            crosstalk_db: None,
            pps_error_ns: 0.0,
            events: vec![],
            temperature_c: None,
            seed: 1,
            interp_half: 24,
            history_samples: 1 << 23,
        }
    }
}

impl VirtualConfig {
    /// A realistic bench: 16-bit DAC, 14-bit ADC, linear-phase filters at 0.2 cycles/sample,
    /// low noise, the given latency.
    pub fn bench(latency_ns: f64) -> VirtualConfig {
        VirtualConfig {
            latency_ns,
            dac_bits: 16,
            adc_bits: 14,
            reconstruction_cutoff: Some(0.2),
            anti_alias_cutoff: Some(0.2),
            noise_rms: 1e-4,
            jitter_ps_rms: 1.0,
            ..VirtualConfig::default()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        for (n, c) in [
            ("reconstruction_cutoff", self.reconstruction_cutoff),
            ("anti_alias_cutoff", self.anti_alias_cutoff),
        ] {
            if let Some(c) = c {
                if !(c > 0.0 && c < 0.5) {
                    return Err(format!("{n} must be in (0, 0.5) cycles/sample"));
                }
            }
        }
        if self.dac_bits > 32 || self.adc_bits > 32 || self.dac_bits == 1 || self.adc_bits == 1 {
            return Err("converter bits must be 0 (ideal) or 2..=32".into());
        }
        if !(self.noise_rms >= 0.0) || !(self.jitter_ps_rms >= 0.0) {
            return Err("noise_rms and jitter_ps_rms must be ≥ 0".into());
        }
        if self.interp_half < 4 {
            return Err("interp_half must be ≥ 4".into());
        }
        if self.clock_offset_ppm.abs() > 1e4 || self.locked_residual_ppm.abs() > 1e4 {
            return Err("clock offsets must be within ±10⁴ ppm".into());
        }
        for e in &self.events {
            if let VirtualEvent::RefLockLoss {
                from_sample,
                to_sample,
            } = e
            {
                if to_sample <= from_sample {
                    return Err("RefLockLoss: to_sample must exceed from_sample".into());
                }
            }
        }
        Ok(())
    }
}

/// Quantise `x` to a signed converter of `bits` (full scale ±1.0); returns (value, clipped).
pub fn quantise(x: f64, bits: u32) -> (f64, bool) {
    let clipped = x.abs() > 1.0;
    let x = x.clamp(-1.0, 1.0);
    if bits == 0 {
        return (x, clipped);
    }
    let l = ((1u64 << (bits - 1)) - 1) as f64;
    ((x * l).round() / l, clipped)
}

fn splitmix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Counter-based standard normal deviate for (seed, stream, index).
fn gauss_at(seed: u64, stream: u64, idx: u64) -> f64 {
    let h = splitmix(seed ^ splitmix(stream.wrapping_mul(0xD1B5_4A32_D192_ED03) ^ idx));
    let h2 = splitmix(h);
    let u1 = ((h >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let u2 = (h2 >> 11) as f64 / (1u64 << 53) as f64;
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

const STREAM_NOISE: u64 = 1;
const STREAM_JITTER: u64 = 2;
const KAISER_BETA: f64 = 10.0;
const KAISER_LUT: usize = 4096;

/// The virtual bench device.
pub struct VirtualDevice {
    vc: VirtualConfig,
    cfg: Option<DeviceConfig>,
    status: DeviceStatus,
    kaiser_lut: Vec<f64>,
    // transmit
    out: Vec<Vec<f64>>,
    out_base: u64,
    rec: Vec<FirState>,
    rec_delay: f64,
    tx_edge_s: f64,
    tx_t0_s: Option<f64>,
    tx_dev_samples: u64,
    data_written: u64,
    tx_active: bool,
    underruns: Vec<(u64, u64)>,
    lates: Vec<u64>,
    loss: Vec<(u64, u64)>,
    // receive
    rx_t0_s: Option<f64>,
    rx_pre_next: u64,
    rx_out: u64,
    aa: Vec<FirState>,
    aa_delay: usize,
    aa_dropped: usize,
    rx_ready: Vec<VecDeque<f64>>,
    now_s: f64,
    /// DAC samples clipped at ±1.0.
    pub dac_clipped: u64,
    /// ADC samples clipped at ±1.0.
    pub adc_clipped: u64,
}

impl VirtualDevice {
    pub fn new(vc: VirtualConfig) -> Result<VirtualDevice, String> {
        vc.validate()?;
        let kaiser_lut = (0..=KAISER_LUT)
            .map(|i| kaiser(i as f64 / KAISER_LUT as f64, KAISER_BETA))
            .collect();
        let mut underruns: Vec<(u64, u64)> = vec![];
        let mut lates = vec![];
        let mut loss = vec![];
        for e in &vc.events {
            match *e {
                VirtualEvent::Underrun {
                    at_sample,
                    gap_samples,
                } => underruns.push((at_sample, gap_samples)),
                VirtualEvent::Late { at_sample } => lates.push(at_sample),
                VirtualEvent::RefLockLoss {
                    from_sample,
                    to_sample,
                } => loss.push((from_sample, to_sample)),
            }
        }
        underruns.sort();
        lates.sort();
        loss.sort();
        // merge overlapping lock-loss windows
        let mut merged: Vec<(u64, u64)> = vec![];
        for (a, b) in loss {
            match merged.last_mut() {
                Some(l) if a <= l.1 => l.1 = l.1.max(b),
                _ => merged.push((a, b)),
            }
        }
        Ok(VirtualDevice {
            vc,
            cfg: None,
            status: DeviceStatus::default(),
            kaiser_lut,
            out: vec![],
            out_base: 0,
            rec: vec![],
            rec_delay: 0.0,
            tx_edge_s: 0.0,
            tx_t0_s: None,
            tx_dev_samples: 0,
            data_written: 0,
            tx_active: false,
            underruns,
            lates,
            loss: merged,
            rx_t0_s: None,
            rx_pre_next: 0,
            rx_out: 0,
            aa: vec![],
            aa_delay: 0,
            aa_dropped: 0,
            rx_ready: vec![],
            now_s: 0.0,
            dac_clipped: 0,
            adc_clipped: 0,
        })
    }

    pub fn virtual_config(&self) -> &VirtualConfig {
        &self.vc
    }
    /// Board temperature (the configured value).
    pub fn temperature_c(&self) -> Option<f64> {
        self.vc.temperature_c
    }
    /// The configured PPS alignment error (ns) — what a time-interval counter between the
    /// reference PPS and the device's PPS-aligned output would read, less the latency.
    pub fn pps_error_ns(&self) -> f64 {
        self.vc.pps_error_ns
    }
    /// Current virtual time (s).
    pub fn now_s(&self) -> f64 {
        self.now_s
    }

    fn fs(&self) -> Result<f64, String> {
        self.cfg
            .as_ref()
            .map(|c| c.sample_rate_hz)
            .ok_or_else(|| "virtual device not configured".to_string())
    }

    fn external(&self) -> bool {
        self.cfg
            .as_ref()
            .map(|c| c.clock_source == RefSource::External)
            .unwrap_or(false)
    }

    fn locked_at(&self, j: u64) -> Option<bool> {
        if !self.external() {
            return None;
        }
        Some(!self.loss.iter().any(|&(a, b)| j >= a && j < b))
    }

    fn periods(&self, fs: f64) -> (f64, f64) {
        let t_free = 1.0 / (fs * (1.0 + self.vc.clock_offset_ppm * 1e-6));
        if self.external() {
            (
                1.0 / (fs * (1.0 + self.vc.locked_residual_ppm * 1e-6)),
                t_free,
            )
        } else {
            (t_free, t_free)
        }
    }

    /// Fractional device-output index at true time offset `d` (s) after `t_tx0`.
    fn index_at(&self, d: f64, fs: f64) -> f64 {
        let (tb, tf) = self.periods(fs);
        if !self.external() || self.loss.is_empty() {
            return d / tb;
        }
        let (mut pj, mut pt) = (0.0f64, 0.0f64);
        for &(a, b) in &self.loss {
            let (a, b) = (a as f64, b as f64);
            let dt = (a - pj) * tb;
            if d < pt + dt {
                return pj + (d - pt) / tb;
            }
            pt += dt;
            pj = a;
            let dt = (b - a) * tf;
            if d < pt + dt {
                return pj + (d - pt) / tf;
            }
            pt += dt;
            pj = b;
        }
        pj + (d - pt) / tb
    }

    fn start_edge(&self, at: StartTime) -> (f64, bool, f64) {
        // (edge s, pps aligned, device-time error s)
        match at {
            StartTime::Now => (self.now_s, false, 0.0),
            StartTime::NextPps => (
                (self.now_s - 1e-12).ceil().max(0.0),
                true,
                self.vc.pps_error_ns * 1e-9,
            ),
            StartTime::AtDeviceTimeNs(t) => (t as f64 * 1e-9, false, self.vc.pps_error_ns * 1e-9),
        }
    }

    fn kaiser_w(&self, x: f64) -> f64 {
        let p = x.abs() * KAISER_LUT as f64;
        if p >= KAISER_LUT as f64 {
            return 0.0;
        }
        let i = p as usize;
        let f = p - i as f64;
        self.kaiser_lut[i] * (1.0 - f) + self.kaiser_lut[i + 1] * f
    }

    /// Band-limited value of transmit channel `ch` at fractional filtered-stream index `v`;
    /// `None` when the samples needed are not written yet.
    fn interp(&self, ch: usize, v: f64) -> Option<f64> {
        let h = self.vc.interp_half as i64;
        let buf = &self.out[ch];
        let end = self.out_base as i64 + buf.len() as i64;
        let i0 = v.floor() as i64;
        if self.tx_active && i0 + h >= end {
            return None;
        }
        let frac = v - i0 as f64;
        let rel = |k: i64| -> f64 {
            let r = k - self.out_base as i64;
            if r < 0 || r >= buf.len() as i64 {
                0.0
            } else {
                buf[r as usize]
            }
        };
        if frac < 1e-12 {
            return Some(rel(i0));
        }
        let s = (PI * frac).sin();
        let span = (h + 1) as f64;
        let (mut acc, mut wsum) = (0.0, 0.0);
        for k in (i0 - h + 1)..=(i0 + h) {
            let x = v - k as f64;
            let m = i0 - k; // sin(π x) = (−1)^m sin(π frac)
            let sx = if m.rem_euclid(2) == 0 { s } else { -s };
            let w = sx / (PI * x) * self.kaiser_w(x / span);
            acc += w * rel(k);
            wsum += w;
        }
        Some(if wsum != 0.0 { acc / wsum } else { 0.0 })
    }

    /// Pre-anti-alias receive sample `m` (all receive channels), or `None` if not available.
    fn pre_sample(&self, m: u64, fs: f64, rx_ch: usize) -> Option<Vec<f64>> {
        let tau = self.rx_t0_s.unwrap_or(0.0) + m as f64 / fs;
        let ntx = self.out.len();
        let mut v = vec![0.0; ntx];
        if let Some(t0) = self.tx_t0_s {
            let jit = if self.vc.jitter_ps_rms > 0.0 {
                self.vc.jitter_ps_rms * 1e-12 * gauss_at(self.vc.seed, STREAM_JITTER, m)
            } else {
                0.0
            };
            let u = self.index_at(tau - jit - t0, fs) + self.rec_delay;
            for (d, x) in v.iter_mut().enumerate() {
                let g = self.gain(d);
                *x = g * self.interp(d, u)?;
            }
        }
        let xt = self.vc.crosstalk_db.map(|db| 10f64.powf(db / 20.0));
        let total: f64 = v.iter().sum();
        let rx_gain = |c: usize| -> f64 {
            let g = self
                .cfg
                .as_ref()
                .and_then(|c2| c2.rx_gain_db.get(c).copied())
                .unwrap_or(0.0);
            10f64.powf(g / 20.0)
        };
        Some(
            (0..rx_ch)
                .map(|c| {
                    let own = v.get(c).copied().unwrap_or(0.0);
                    let leak = xt.map(|k| k * (total - own)).unwrap_or(0.0);
                    rx_gain(c) * (own + leak)
                })
                .collect(),
        )
    }

    fn gain(&self, ch: usize) -> f64 {
        let e = self.vc.gain_error_db.get(ch).copied().unwrap_or(0.0);
        let g = self
            .cfg
            .as_ref()
            .and_then(|c| c.tx_gain_db.get(ch).copied())
            .unwrap_or(0.0);
        10f64.powf((e + g) / 20.0)
    }

    fn append_out(&mut self, staged: Vec<Vec<f64>>) {
        for (c, x) in staged.into_iter().enumerate() {
            let y = match self.rec.get_mut(c) {
                Some(f) => f.process(&x),
                None => x,
            };
            self.out[c].extend_from_slice(&y);
        }
        let lim = self.vc.history_samples.max(1 << 16);
        if self.out.first().map(|b| b.len()).unwrap_or(0) > 2 * lim {
            let drop = self.out[0].len() - lim;
            for b in self.out.iter_mut() {
                b.drain(..drop);
            }
            self.out_base += drop as u64;
        }
    }

    fn filter_taps(&self, cutoff: f64) -> Vec<f64> {
        lowpass_taps(cutoff, self.vc.filter_taps.max(3), 10.0)
    }
}

impl Device for VirtualDevice {
    fn name(&self) -> String {
        "virtual-bench".into()
    }
    fn realtime(&self) -> bool {
        false
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        cfg.validate()?;
        self.cfg = Some(cfg.clone());
        self.status.message = format!(
            "virtual bench: latency {} ns, clock {} ppm, ref {:?}",
            self.vc.latency_ns, self.vc.clock_offset_ppm, cfg.clock_source
        );
        Ok(())
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.cfg.as_ref().map(|c| c.sample_rate_hz)
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let cfg = self.cfg.clone().ok_or("virtual device not configured")?;
        if cfg.tx_channels == 0 {
            return Err("no tx channels configured".into());
        }
        let (edge, pps, err) = self.start_edge(at);
        self.tx_edge_s = edge;
        self.tx_t0_s = Some(edge + err + self.vc.latency_ns * 1e-9);
        self.out = vec![Vec::new(); cfg.tx_channels];
        self.out_base = 0;
        self.rec = match self.vc.reconstruction_cutoff {
            Some(fc) => (0..cfg.tx_channels)
                .map(|_| FirState::new(self.filter_taps(fc), 0))
                .collect(),
            None => vec![],
        };
        self.rec_delay = self
            .rec
            .first()
            .map(|f| ((f.taps().len() - 1) / 2) as f64)
            .unwrap_or(0.0);
        self.tx_dev_samples = 0;
        self.data_written = 0;
        self.tx_active = true;
        self.status.tx_streaming = true;
        self.status.samples_tx = 0;
        self.status.start_time_ns = Some((edge * 1e9).round() as i64);
        self.status.pps_aligned = pps;
        Ok(self.status.start_time_ns)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        if !self.tx_active {
            return Err("transmit stream not started".into());
        }
        let nch = self.out.len();
        if block.len() != nch {
            return Err(format!(
                "block has {} channels, device has {nch}",
                block.len()
            ));
        }
        let n = block.first().map(|c| c.len()).unwrap_or(0);
        if block.iter().any(|c| c.len() != n) {
            return Err("channels of a block differ in length".into());
        }
        let mut staged: Vec<Vec<f64>> = vec![Vec::with_capacity(n); nch];
        for i in 0..n {
            let k = self.data_written + i as u64;
            while let Some(&(at, gap)) = self.underruns.first() {
                if at > k {
                    break;
                }
                self.underruns.remove(0);
                self.status.underruns += 1;
                for s in staged.iter_mut() {
                    s.extend(std::iter::repeat_n(0.0, gap as usize));
                }
                self.tx_dev_samples += gap;
            }
            while let Some(&at) = self.lates.first() {
                if at > k {
                    break;
                }
                self.lates.remove(0);
                self.status.late += 1;
            }
            for (c, s) in staged.iter_mut().enumerate() {
                let (q, clip) = quantise(block[c][i], self.vc.dac_bits);
                if clip {
                    self.dac_clipped += 1;
                }
                s.push(q);
            }
            self.tx_dev_samples += 1;
        }
        self.data_written += n as u64;
        self.status.samples_tx += n as u64;
        self.append_out(staged);
        let fs = self.fs()?;
        self.now_s = self
            .now_s
            .max(self.tx_edge_s + self.tx_dev_samples as f64 / fs);
        Ok(())
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        if self.tx_active && !self.rec.is_empty() {
            let tail = self.rec[0].taps().len();
            let z = vec![vec![0.0; tail]; self.out.len()];
            self.append_out(z);
        }
        self.tx_active = false;
        self.status.tx_streaming = false;
        Ok(())
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let cfg = self.cfg.clone().ok_or("virtual device not configured")?;
        if cfg.rx_channels == 0 {
            return Err("no rx channels configured".into());
        }
        // The receive side is the ideal reference digitiser: no PPS error of its own.
        let (edge, pps, _) = self.start_edge(at);
        self.rx_t0_s = Some(edge);
        self.rx_pre_next = 0;
        self.rx_out = 0;
        self.aa = match self.vc.anti_alias_cutoff {
            Some(fc) => (0..cfg.rx_channels)
                .map(|_| FirState::new(self.filter_taps(fc), 0))
                .collect(),
            None => vec![],
        };
        self.aa_delay = self
            .aa
            .first()
            .map(|f| (f.taps().len() - 1) / 2)
            .unwrap_or(0);
        self.aa_dropped = 0;
        self.rx_ready = vec![VecDeque::new(); cfg.rx_channels];
        self.status.rx_streaming = true;
        self.status.samples_rx = 0;
        if !self.status.tx_streaming {
            self.status.start_time_ns = Some((edge * 1e9).round() as i64);
            self.status.pps_aligned = pps;
        }
        Ok(Some((edge * 1e9).round() as i64))
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        if self.rx_t0_s.is_none() || !self.status.rx_streaming {
            return Err("receive stream not started".into());
        }
        let fs = self.fs()?;
        let rx_ch = self.rx_ready.len();
        let have = self.rx_ready[0].len();
        if have < n {
            let need = (n - have) + (self.aa_delay - self.aa_dropped);
            let mut pre: Vec<Vec<f64>> = vec![Vec::with_capacity(need); rx_ch];
            for _ in 0..need {
                match self.pre_sample(self.rx_pre_next, fs, rx_ch) {
                    Some(v) => {
                        for (c, x) in v.into_iter().enumerate() {
                            pre[c].push(x);
                        }
                        self.rx_pre_next += 1;
                    }
                    None => break,
                }
            }
            let got = pre[0].len();
            let drop = (self.aa_delay - self.aa_dropped).min(got);
            for (c, x) in pre.into_iter().enumerate() {
                let y = match self.aa.get_mut(c) {
                    Some(f) => f.process(&x),
                    None => x,
                };
                self.rx_ready[c].extend(y.into_iter().skip(drop));
            }
            self.aa_dropped += drop;
        }
        let k = n.min(self.rx_ready[0].len());
        if k == 0 && n > 0 {
            return Err(
                "virtual loopback: receive is ahead of transmit; write more transmit samples first"
                    .into(),
            );
        }
        let mut out = vec![Vec::with_capacity(k); rx_ch];
        for (c, o) in out.iter_mut().enumerate() {
            for i in 0..k {
                let mut x = self.rx_ready[c].pop_front().unwrap_or(0.0);
                if self.vc.noise_rms > 0.0 {
                    let idx = self.rx_out + i as u64;
                    x += self.vc.noise_rms
                        * gauss_at(self.vc.seed, STREAM_NOISE + 16 * (c as u64 + 1), idx);
                }
                let (q, clip) = quantise(x, self.vc.adc_bits);
                if clip {
                    self.adc_clipped += 1;
                }
                o.push(q);
            }
        }
        self.rx_out += k as u64;
        self.status.samples_rx += k as u64;
        self.now_s = self
            .now_s
            .max(self.rx_t0_s.unwrap_or(0.0) + self.rx_out as f64 / fs);
        Ok(out)
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        self.status.rx_streaming = false;
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        let mut s = self.status.clone();
        s.ref_locked = self.locked_at(self.tx_dev_samples);
        s
    }
}

/// A [`SampleSink`] writing into a device's transmit stream (for producers such as
/// [`super::calib::write_calibration_train`] driven synchronously, without a
/// [`super::TxSession`]).
pub struct DeviceSink<'a> {
    pub device: &'a mut dyn Device,
}

impl SampleSink for DeviceSink<'_> {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        self.device.write_tx(channels)
    }
}

/// Read `n` samples per channel from a device's running receive stream.
pub fn read_exact(device: &mut dyn Device, n: usize) -> Result<Vec<Vec<f64>>, String> {
    let mut acc: Vec<Vec<f64>> = vec![];
    let mut got = 0;
    while got < n {
        let b = device.read_rx((n - got).min(65_536))?;
        let k = b.first().map(|c| c.len()).unwrap_or(0);
        if k == 0 {
            break;
        }
        if acc.is_empty() {
            acc = vec![Vec::with_capacity(n); b.len()];
        }
        for (a, x) in acc.iter_mut().zip(b) {
            a.extend(x);
        }
        got += k;
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::super::calib::{characterise_from_capture, write_calibration_train};
    use super::*;

    const FS: f64 = 2e6;

    fn loop_cfg(tx: usize, rx: usize, clock: RefSource) -> DeviceConfig {
        let mut c = DeviceConfig::tx(FS, tx);
        c.rx_channels = rx;
        c.clock_source = clock;
        c
    }

    /// Transmit start (device time) of the calibration runs: 1 ms after the receive start,
    /// so the first pulse lies wholly inside the capture.
    const T0: f64 = 1e-3;

    fn pps(n: usize, period: f64) -> Vec<f64> {
        (0..n).map(|k| T0 + k as f64 * period).collect()
    }

    /// Run the calibration train (1 pulse per `period` s, `pulses` pulses) through `dev`
    /// and return the channel-0 capture.
    fn run_train(dev: &mut VirtualDevice, channels: usize, pulses: usize, period: f64) -> Vec<f64> {
        dev.start_rx(StartTime::NextPps).unwrap();
        dev.start_tx(StartTime::AtDeviceTimeNs((T0 * 1e9) as i64))
            .unwrap();
        let dur = pulses as f64 * period;
        write_calibration_train(
            &mut DeviceSink { device: dev },
            FS,
            dur,
            period,
            channels,
            0.5,
        )
        .unwrap();
        dev.stop_tx().unwrap();
        let n = ((T0 + dur + 1e-3) * FS) as usize;
        let cap = read_exact(dev, n).unwrap();
        assert_eq!(cap[0].len(), n);
        cap.into_iter().next().unwrap()
    }

    #[test]
    fn calibration_recovers_latency_through_virtual_device() {
        for &lat in &[1234.567, 37.21] {
            let mut d = VirtualDevice::new(VirtualConfig::bench(lat)).unwrap();
            d.configure(&loop_cfg(1, 1, RefSource::External)).unwrap();
            let cap = run_train(&mut d, 1, 20, 1e-3);
            let e =
                characterise_from_capture(&cap, FS, &pps(20, 1e-3), &Default::default()).unwrap();
            assert_eq!(e.n, 20);
            assert!(
                (e.delay_ns - lat).abs() < 0.2,
                "latency {lat}: got {} ns",
                e.delay_ns
            );
            assert_eq!(d.status().ref_locked, Some(true));
        }
    }

    #[test]
    fn pps_error_adds_to_the_measured_delay() {
        let mut vc = VirtualConfig::bench(500.0);
        vc.pps_error_ns = -42.5;
        let mut d = VirtualDevice::new(vc).unwrap();
        d.configure(&loop_cfg(1, 1, RefSource::External)).unwrap();
        let cap = run_train(&mut d, 1, 8, 1e-3);
        let e = characterise_from_capture(&cap, FS, &pps(8, 1e-3), &Default::default()).unwrap();
        assert!((e.delay_ns - 457.5).abs() < 0.2, "got {}", e.delay_ns);
        assert_eq!(d.status().start_time_ns, Some(1_000_000));
    }

    #[test]
    fn free_running_clock_and_lock_loss_show_as_drift() {
        // internal reference, +10 ppm: pulse k (k ms after the start) arrives k·10 ns early
        let mut vc = VirtualConfig::default();
        vc.clock_offset_ppm = 10.0;
        let mut d = VirtualDevice::new(vc.clone()).unwrap();
        d.configure(&loop_cfg(1, 1, RefSource::Internal)).unwrap();
        let cap = run_train(&mut d, 1, 10, 1e-3);
        let e = characterise_from_capture(&cap, FS, &pps(10, 1e-3), &Default::default()).unwrap();
        // (each pulse is also compressed by 10 ppm, which biases every estimate by the same
        // ≈ −0.8 ns, so the drift is checked pulse to pulse)
        assert!(e.per_pps_ns[0].abs() < 1.0);
        for (k, v) in e.per_pps_ns.iter().enumerate() {
            let want = -(k as f64 * 1e-3) * 10e-6 / (1.0 + 10e-6) * 1e9;
            let got = v - e.per_pps_ns[0];
            assert!((got - want).abs() < 0.05, "pps {k}: {got} vs {want}");
        }
        assert_eq!(d.status().ref_locked, None);
        // external reference with a lock loss over device samples 4000..8000 (2 ms of
        // holdover, between pulses 2 and 4)
        vc.events = vec![VirtualEvent::RefLockLoss {
            from_sample: 4000,
            to_sample: 8000,
        }];
        let mut d = VirtualDevice::new(vc).unwrap();
        d.configure(&loop_cfg(1, 1, RefSource::External)).unwrap();
        d.start_tx(StartTime::NextPps).unwrap();
        d.write_tx(&[vec![0.0; 5000]]).unwrap();
        assert_eq!(d.status().ref_locked, Some(false));
        d.stop_tx().unwrap();
        let mut d = VirtualDevice::new(d.virtual_config().clone()).unwrap();
        d.configure(&loop_cfg(1, 1, RefSource::External)).unwrap();
        let cap = run_train(&mut d, 1, 10, 1e-3);
        assert_eq!(d.status().ref_locked, Some(true));
        let e = characterise_from_capture(&cap, FS, &pps(10, 1e-3), &Default::default()).unwrap();
        let dt = 4000.0 / FS * 1e9 * (1.0 - 1.0 / (1.0 + 10e-6)); // ≈ 20 ns accumulated
        assert!(e.per_pps_ns[0].abs() < 0.05 && e.per_pps_ns[1].abs() < 0.05);
        for k in 4..10 {
            assert!(
                (e.per_pps_ns[k] + dt).abs() < 0.05,
                "pps {k}: {}",
                e.per_pps_ns[k]
            );
        }
    }

    fn rms(x: &[f64]) -> f64 {
        (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
    }

    #[test]
    fn crosstalk_and_gain_error_show_up_as_configured() {
        let mut vc = VirtualConfig::default();
        vc.gain_error_db = vec![1.5, -0.7];
        vc.crosstalk_db = Some(-40.0);
        vc.latency_ns = 250.0;
        let mut d = VirtualDevice::new(vc).unwrap();
        d.configure(&loop_cfg(2, 2, RefSource::Internal)).unwrap();
        d.start_rx(StartTime::NextPps).unwrap();
        d.start_tx(StartTime::NextPps).unwrap();
        let n = 20_000;
        let x: Vec<f64> = (0..n)
            .map(|i| 0.4 * (2.0 * PI * 100e3 * i as f64 / FS).sin())
            .collect();
        d.write_tx(&[x.clone(), vec![0.0; n]]).unwrap();
        d.stop_tx().unwrap();
        let cap = read_exact(&mut d, n).unwrap();
        let (a, b) = (2000, n - 2000);
        let tx = rms(&x[a..b]);
        let g0 = rms(&cap[0][a..b]) / tx;
        let xt = rms(&cap[1][a..b]) / rms(&cap[0][a..b]);
        assert!((20.0 * g0.log10() - 1.5).abs() < 0.01, "gain {g0}");
        assert!((20.0 * xt.log10() + 40.0).abs() < 0.01, "crosstalk {xt}");
    }

    #[test]
    fn underrun_and_late_counters_and_gap() {
        let mut vc = VirtualConfig::default();
        vc.events = vec![
            VirtualEvent::Underrun {
                at_sample: 3000,
                gap_samples: 500,
            },
            VirtualEvent::Underrun {
                at_sample: 9000,
                gap_samples: 10,
            },
            VirtualEvent::Late { at_sample: 100 },
        ];
        let mut d = VirtualDevice::new(vc).unwrap();
        d.configure(&loop_cfg(1, 1, RefSource::Internal)).unwrap();
        d.start_rx(StartTime::NextPps).unwrap();
        d.start_tx(StartTime::NextPps).unwrap();
        for _ in 0..5 {
            d.write_tx(&[vec![1e-3; 1000]]).unwrap();
        }
        let s = d.status();
        assert_eq!(s.underruns, 1);
        assert_eq!(s.late, 1);
        assert_eq!(s.samples_tx, 5000);
        // the gap: device samples 3000..3500 are zero, data resumes after
        let cap = read_exact(&mut d, 4000).unwrap();
        assert!((cap[0][2900] - 1e-3).abs() < 1e-12);
        assert!(cap[0][3100..3400].iter().all(|v| v.abs() < 1e-12));
        assert!((cap[0][3600] - 1e-3).abs() < 1e-12);
        for _ in 0..5 {
            d.write_tx(&[vec![1e-3; 1000]]).unwrap();
        }
        assert_eq!(d.status().underruns, 2);
        // receive beyond the transmitted data while streaming is refused, not invented
        let _ = read_exact(&mut d, 6000).unwrap();
        let part = d.read_rx(1000).unwrap();
        assert!(!part[0].is_empty() && part[0].len() < 1000);
        assert!(d.read_rx(1000).is_err());
        d.stop_tx().unwrap();
        assert_eq!(d.read_rx(1000).unwrap()[0].len(), 1000);
    }

    #[test]
    fn quantisation_and_noise_levels() {
        let (q, c) = quantise(0.123456, 8);
        assert!((q - (0.123456f64 * 127.0).round() / 127.0).abs() < 1e-15 && !c);
        assert_eq!(quantise(1.5, 12), (1.0, true));
        let mut vc = VirtualConfig::default();
        vc.noise_rms = 1e-3;
        vc.adc_bits = 16;
        let mut d = VirtualDevice::new(vc).unwrap();
        d.configure(&loop_cfg(1, 1, RefSource::Internal)).unwrap();
        d.start_rx(StartTime::Now).unwrap();
        let a = read_exact(&mut d, 50_000).unwrap();
        let r = rms(&a[0]);
        assert!((r - 1e-3).abs() < 2e-5, "noise rms {r}");
        // deterministic and independent of block size
        let mut d2 = VirtualDevice::new(d.virtual_config().clone()).unwrap();
        d2.configure(&loop_cfg(1, 1, RefSource::Internal)).unwrap();
        d2.start_rx(StartTime::Now).unwrap();
        let mut b = vec![];
        while b.len() < 50_000 {
            b.extend(d2.read_rx(777.min(50_000 - b.len())).unwrap().remove(0));
        }
        assert_eq!(a[0], b);
    }
}
