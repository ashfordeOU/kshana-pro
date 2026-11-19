// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Spec twins: a [`Device`] that behaves like a catalogue device as far as its published
//! specification goes, so a bench can be exercised, and its expected performance predicted,
//! before the hardware is bought or plugged in.
//!
//! A [`SpecTwin`] wraps the [`VirtualDevice`] (DAC quantisation, reconstruction filter,
//! clock, jitter, loopback to an ideal reference digitiser) configured from the catalogue:
//!
//! * DAC resolution = `tx.dac_bits`;
//! * reconstruction filter at the published analogue bandwidth (when it is inside the
//!   simulated Nyquist band);
//! * RMS clock jitter = `timing.jitter_rms_s` when published;
//! * an AC-coupled output gets a first-order high-pass at `tx.hp_corner_hz`;
//! * configuring a sample rate above `tx.max_sample_rate_hz`, or using a device whose
//!   published lower frequency limit excludes 90–110 kHz, is refused with the reason.
//!
//! [`expected_performance`] runs the calibration pulse train through the twin at several
//! levels and reports the timing and envelope errors and the waveform SNR the published
//! converter alone would allow. Evidence: **MODELLED from datasheet values**; analogue
//! effects the datasheet does not state (output filter ripple, temperature drift,
//! nonlinearity) are not in the twin. Measured hardware replaces these numbers via the
//! acceptance suite.

use super::catalog::HwSpec;
use crate::accept::suite::envelope_ecd_us;
use crate::hw::calib::{
    calibration_pulse_train, characterise_from_capture, write_calibration_train,
    CharacteriseOptions,
};
use crate::hw::virtual_bench::{read_exact, DeviceSink, VirtualConfig, VirtualDevice};
use crate::hw::{Device, DeviceConfig, DeviceStatus, RefSource, StartTime};
use serde::{Deserialize, Serialize};

/// A device emulated from its published specification.
pub struct SpecTwin {
    spec: HwSpec,
    inner: VirtualDevice,
    /// First-order high-pass coefficient and state per channel (AC-coupled outputs).
    hp: Option<(f64, f64)>,
    hp_state: Vec<(f64, f64)>,
    /// Assumptions made where the datasheet is silent.
    pub assumptions: Vec<String>,
}

impl SpecTwin {
    /// Build the twin for simulation at `fs_hz`.
    pub fn from_spec(spec: &HwSpec, fs_hz: f64) -> Result<SpecTwin, String> {
        let tx = spec
            .tx
            .as_ref()
            .ok_or_else(|| format!("{}: no signal output in the catalogue", spec.id))?;
        if spec.lf_direct() == Some(false) {
            return Err(format!(
                "{} cannot output 90–110 kHz directly (lowest frequency {} Hz, {}-coupled){}",
                spec.model,
                tx.freq_min_hz
                    .map(|f| format!("{f:.0}"))
                    .unwrap_or("?".into()),
                tx.coupling.as_deref().unwrap_or("?"),
                tx.lf_capable_note
                    .as_deref()
                    .map(|n| format!(": {n}"))
                    .unwrap_or_default()
            ));
        }
        if let Some(max) = tx.max_sample_rate_hz {
            if fs_hz > max * (1.0 + 1e-9) {
                return Err(format!(
                    "{}: {fs_hz} S/s is above its published maximum {max} S/s",
                    spec.model
                ));
            }
        }
        let mut assumptions = vec![];
        let dac_bits = match tx.dac_bits {
            Some(b) => b,
            None => {
                assumptions.push("DAC resolution not published: modelled as 12 bits".into());
                12
            }
        };
        let reconstruction_cutoff = tx.analog_bw_hz.and_then(|bw| {
            let c = bw / fs_hz;
            (c < 0.45).then_some(c.max(0.06))
        });
        if tx.analog_bw_hz.is_none() {
            assumptions.push("analogue bandwidth not published: no reconstruction filter".into());
        }
        let jitter_ps_rms = spec
            .timing
            .as_ref()
            .and_then(|t| t.jitter_rms_s)
            .map(|j| j * 1e12)
            .unwrap_or_else(|| {
                assumptions.push("output clock jitter not published: modelled as 0".into());
                0.0
            });
        assumptions.push(
            "latency, gain error and filter ripple are not datasheet values: \
             latency 0 (it is calibrated out by the loopback procedure)"
                .into(),
        );
        let vc = VirtualConfig {
            dac_bits,
            adc_bits: 0,
            reconstruction_cutoff,
            jitter_ps_rms,
            noise_rms: 0.0,
            ..VirtualConfig::default()
        };
        let hp = match (tx.coupling.as_deref(), tx.hp_corner_hz) {
            (Some("ac"), Some(fc)) if fc > 0.0 => {
                let rc = 1.0 / (2.0 * std::f64::consts::PI * fc);
                let dt = 1.0 / fs_hz;
                Some((rc / (rc + dt), fc))
            }
            (Some("ac"), None) => {
                assumptions.push(
                    "AC-coupled output with no published corner: high-pass not modelled".into(),
                );
                None
            }
            _ => None,
        };
        Ok(SpecTwin {
            spec: spec.clone(),
            inner: VirtualDevice::new(vc)?,
            hp,
            hp_state: vec![],
            assumptions,
        })
    }

    pub fn spec(&self) -> &HwSpec {
        &self.spec
    }

    /// Peak output (V into 50 Ω) at full scale, when published.
    pub fn full_scale_vpeak(&self) -> Option<f64> {
        self.spec
            .tx
            .as_ref()
            .and_then(|t| t.max_vpp_into_50ohm)
            .map(|v| v / 2.0)
    }
}

impl Device for SpecTwin {
    fn name(&self) -> String {
        format!("spec twin of {} {}", self.spec.vendor, self.spec.model)
    }
    fn realtime(&self) -> bool {
        false
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        let tx = self.spec.tx.clone().unwrap_or_default();
        if let Some(max) = tx.max_sample_rate_hz {
            if cfg.sample_rate_hz > max * (1.0 + 1e-9) {
                return Err(format!(
                    "{}: {} S/s is above its published maximum {max} S/s",
                    self.spec.model, cfg.sample_rate_hz
                ));
            }
        }
        if let Some(ch) = tx.channels {
            if cfg.tx_channels > ch as usize {
                return Err(format!(
                    "{}: {} output channels requested, {ch} published",
                    self.spec.model, cfg.tx_channels
                ));
            }
        }
        if cfg.clock_source == RefSource::External && self.spec.takes_10mhz() == Some(false) {
            return Err(format!(
                "{}: no external 10 MHz input published",
                self.spec.model
            ));
        }
        if let Some((a, _)) = self.hp {
            // Re-derive for the configured rate.
            let fc = self.hp.map(|h| h.1).unwrap_or(0.0);
            let rc = 1.0 / (2.0 * std::f64::consts::PI * fc);
            let dt = 1.0 / cfg.sample_rate_hz;
            let _ = a;
            self.hp = Some((rc / (rc + dt), fc));
        }
        self.hp_state = vec![(0.0, 0.0); cfg.tx_channels];
        self.inner.configure(cfg)
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.inner.sample_rate_hz()
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        self.hp_state.iter_mut().for_each(|s| *s = (0.0, 0.0));
        self.inner.start_tx(at)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        match self.hp {
            None => self.inner.write_tx(block),
            Some((a, _)) => {
                let mut b = block.to_vec();
                for (c, ch) in b.iter_mut().enumerate() {
                    if c >= self.hp_state.len() {
                        self.hp_state.resize(c + 1, (0.0, 0.0));
                    }
                    let (mut x1, mut y1) = self.hp_state[c];
                    for v in ch.iter_mut() {
                        let y = a * (y1 + *v - x1);
                        x1 = *v;
                        y1 = y;
                        *v = y;
                    }
                    self.hp_state[c] = (x1, y1);
                }
                self.inner.write_tx(&b)
            }
        }
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        self.inner.stop_tx()
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        self.inner.start_rx(at)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        self.inner.read_rx(n)
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        self.inner.stop_rx()
    }
    fn status(&self) -> DeviceStatus {
        let mut s = self.inner.status();
        s.message = format!("{} | {}", self.name(), s.message);
        s
    }
}

/// Predicted performance at one output level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelResult {
    /// Pulse peak relative to full scale (dB).
    pub level_dbfs: f64,
    /// Peak output voltage into 50 Ω at this level, when the full scale is published.
    pub vpeak_50ohm: Option<f64>,
    /// Mean zero-crossing delay error (ns) against the ideal pulse (bias the converter adds).
    pub delay_bias_ns: f64,
    /// Pulse-to-pulse standard deviation of the zero crossing (ns).
    pub delay_std_ns: f64,
    /// Envelope-to-cycle discrepancy measured on the output (µs; 0 transmitted).
    pub ecd_us: f64,
    /// Waveform SNR of the output against the ideal pulse train (dB).
    pub snr_db: f64,
}

/// Predicted performance of a device as an eLoran source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwinReport {
    pub device: String,
    pub fs_hz: f64,
    pub dac_bits: u32,
    pub levels: Vec<LevelResult>,
    pub assumptions: Vec<String>,
    pub evidence: String,
}

impl TwinReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Run the calibration pulse train through the twin of `spec` at each level (dBFS) and
/// measure what the published converter allows. `pulses` pulses, one per `period_s`.
pub fn expected_performance(
    spec: &HwSpec,
    fs_hz: f64,
    levels_dbfs: &[f64],
    pulses: usize,
) -> Result<TwinReport, String> {
    let mut out = vec![];
    let mut assumptions = vec![];
    let mut dac_bits = 0;
    for &lvl in levels_dbfs {
        let mut tw = SpecTwin::from_spec(spec, fs_hz)?;
        dac_bits = tw.inner.virtual_config().dac_bits;
        if assumptions.is_empty() {
            assumptions = tw.assumptions.clone();
        }
        let vfs = tw.full_scale_vpeak();
        let m = measure_level(
            &mut tw,
            fs_hz,
            lvl,
            pulses,
            spec.takes_10mhz() == Some(true),
            vfs,
        )?;
        out.push(m.result);
    }
    Ok(TwinReport {
        device: spec.id.clone(),
        fs_hz,
        dac_bits,
        levels: out,
        assumptions,
        evidence: "MODELLED from datasheet values (converter only); replace with the acceptance suite on the real device".into(),
    })
}

/// One level measured through a device's own loopback (twin or hardware).
#[derive(Clone, Debug)]
pub struct LevelCapture {
    pub result: LevelResult,
    /// Amplitude of the capture relative to the commanded pulse (dB; 0 = as commanded).
    pub gain_db: f64,
    /// Commanded peak amplitude (full-scale units).
    pub amp: f64,
    /// PPS instants used (s, device time from the receive start).
    pub pps_s: Vec<f64>,
    /// The channel-0 capture (full-scale units).
    pub capture: Vec<f64>,
}

/// Play the calibration pulse train (one pulse per ms, `pulses` pulses) at `level_dbfs`
/// through `dev` (transmit and receive configured on the same device, output looped back to
/// input) and measure: zero-crossing delay and spread, ECD, gain and waveform SNR after
/// removing the measured delay and gain. Works on a [`SpecTwin`] or on hardware.
pub fn measure_level(
    dev: &mut dyn Device,
    fs_hz: f64,
    level_dbfs: f64,
    pulses: usize,
    external_ref: bool,
    vpeak_full_scale: Option<f64>,
) -> Result<LevelCapture, String> {
    const T0: f64 = 1e-3;
    let period = 1e-3;
    let amp = 10f64.powf(level_dbfs / 20.0);
    let mut cfg = DeviceConfig::tx(fs_hz, 1);
    cfg.rx_channels = 1;
    cfg.clock_source = if external_ref {
        RefSource::External
    } else {
        RefSource::Internal
    };
    dev.configure(&cfg)?;
    // Transmit 1 ms after the receive start, on the device clock (the device may have run
    // before: its clock does not restart).
    let rx0 = dev.start_rx(StartTime::NextPps)?.unwrap_or(0);
    dev.start_tx(StartTime::AtDeviceTimeNs(rx0 + (T0 * 1e9) as i64))?;
    let dur = pulses as f64 * period;
    write_calibration_train(&mut DeviceSink { device: dev }, fs_hz, dur, period, 1, amp)?;
    dev.stop_tx()?;
    let n = ((T0 + dur + 1e-3) * fs_hz) as usize;
    let cap = read_exact(dev, n)?
        .into_iter()
        .next()
        .ok_or("the device returned no capture")?;
    dev.stop_rx().ok();
    let pps: Vec<f64> = (0..pulses).map(|k| T0 + k as f64 * period).collect();
    analyse_capture(cap, fs_hz, pps, amp, level_dbfs, vpeak_full_scale)
}

/// Measure a looped-back capture of the calibration pulse train (PPS instants `pps_s`,
/// commanded amplitude `amp`): delay and spread, ECD, least-squares gain and waveform SNR
/// after removing them. Deterministic: the same capture gives the same numbers, which is
/// what lets [`super::firstplug::verify_evidence`] recompute a recorded result.
pub fn analyse_capture(
    cap: Vec<f64>,
    fs_hz: f64,
    pps: Vec<f64>,
    amp: f64,
    level_dbfs: f64,
    vpeak_full_scale: Option<f64>,
) -> Result<LevelCapture, String> {
    let est = characterise_from_capture(&cap, fs_hz, &pps, &CharacteriseOptions::default())?;
    let ideal = calibration_pulse_train(fs_hz, cap.len(), &pps, est.delay_ns, amp);
    let (mut xy, mut yy) = (0.0, 0.0);
    for (a, b) in cap.iter().zip(&ideal) {
        xy += a * b;
        yy += b * b;
    }
    let g = if yy > 0.0 { xy / yy } else { 0.0 };
    let (mut sig, mut err) = (0.0, 0.0);
    for (a, b) in cap.iter().zip(&ideal) {
        let r = g * b;
        sig += r * r;
        err += (a - r) * (a - r);
    }
    let snr_db = if err > 0.0 {
        10.0 * (sig / err).log10()
    } else {
        f64::INFINITY
    };
    let t_origin = pps[0] + est.delay_ns * 1e-9;
    let ecd = envelope_ecd_us(&cap, fs_hz, t_origin).unwrap_or(f64::NAN);
    Ok(LevelCapture {
        result: LevelResult {
            level_dbfs,
            vpeak_50ohm: vpeak_full_scale.map(|v| v * amp),
            delay_bias_ns: est.delay_ns,
            delay_std_ns: est.std_ns,
            ecd_us: ecd,
            snr_db: (snr_db * 100.0).round() / 100.0,
        },
        gain_db: if g > 0.0 {
            20.0 * g.log10()
        } else {
            f64::NEG_INFINITY
        },
        amp,
        pps_s: pps,
        capture: cap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::catalog::{Catalog, Category, TxSpec};

    fn spec(bits: u32, coupling: &str, hp: Option<f64>, lo: f64) -> HwSpec {
        let mut c = Catalog::default();
        c.merge_json(r#"[{"id":"x","vendor":"V","model":"M","category":"sdr","sources":[{"url":"https://e.org","title":"t"}]}]"#).unwrap();
        let mut s = c.get("x").unwrap().clone();
        s.category = Category::Sdr;
        s.tx = Some(TxSpec {
            channels: Some(2),
            dac_bits: Some(bits),
            max_sample_rate_hz: Some(125e6),
            freq_min_hz: Some(lo),
            freq_max_hz: Some(50e6),
            coupling: Some(coupling.into()),
            hp_corner_hz: hp,
            max_vpp_into_50ohm: Some(2.0),
            ..Default::default()
        });
        s
    }

    #[test]
    fn more_dac_bits_means_less_timing_error() {
        let r14 = expected_performance(&spec(14, "dc", None, 0.0), 2e6, &[-40.0], 8).unwrap();
        let r8 = expected_performance(&spec(8, "dc", None, 0.0), 2e6, &[-40.0], 8).unwrap();
        let (a, b) = (&r14.levels[0], &r8.levels[0]);
        assert!(
            a.snr_db > b.snr_db + 20.0,
            "14-bit {} dB vs 8-bit {} dB",
            a.snr_db,
            b.snr_db
        );
        assert!(
            a.delay_bias_ns.abs() < 1.0,
            "14-bit bias {}",
            a.delay_bias_ns
        );
        assert_eq!(a.vpeak_50ohm.map(|v| (v * 1e4).round()), Some(100.0));
    }

    #[test]
    fn ac_coupling_corner_shifts_the_envelope_and_rf_only_parts_are_refused() {
        let dc = expected_performance(&spec(14, "dc", None, 0.0), 2e6, &[-6.0], 4).unwrap();
        let ac = expected_performance(&spec(14, "ac", Some(5e3), 1e3), 2e6, &[-6.0], 4).unwrap();
        assert!(
            dc.levels[0].ecd_us.abs() < 0.05,
            "dc ecd {}",
            dc.levels[0].ecd_us
        );
        assert!(
            (ac.levels[0].ecd_us - dc.levels[0].ecd_us).abs() > 0.1
                || (ac.levels[0].delay_bias_ns - dc.levels[0].delay_bias_ns).abs() > 50.0,
            "a 5 kHz high-pass must visibly shift the pulse: {:?}",
            ac.levels[0]
        );
        let err = SpecTwin::from_spec(&spec(12, "ac", None, 70e6), 2e6)
            .err()
            .unwrap();
        assert!(err.contains("cannot output 90–110 kHz"), "{err}");
    }
}
