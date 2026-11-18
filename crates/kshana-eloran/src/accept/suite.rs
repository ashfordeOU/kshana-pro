// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Hardware acceptance / performance test suite (A5): tests as data, a runner and the
//! requirements verification matrix (RVM).
//!
//! # Tests as data
//!
//! A [`TestCase`] holds an id, the requirement text and ids it verifies, the procedure, the
//! measurement ([`TestKind`], with its parameters) and the [`Limit`]. Running it gives a
//! [`TestResult`] with the measured value, the verdict and evidence file references. Cases
//! serialise to JSON/TOML so a laboratory edits limits and parameters without code.
//! [`default_suite`] lists the standard tests; **its limits are placeholders to be set
//! from the procurement specification**, except the eLoran–GNSS synchronisation limit of
//! 10 ns, which is the stated requirement.
//!
//! # Two classes of test
//!
//! * **Software / signal-level** (run fully in CI from memory scenes): ToA, ECD and ASF
//!   10 ns resolution sweeps (scenes stepped by 10 ns, each step measured on the samples
//!   with [`crate::hw::calib::zero_crossing`] or an envelope matched filter), SNR range
//!   (noise measured on the samples against the pulse amplitude), skywave delay range
//!   (skywave isolated by differencing and timed with the calibration matched filter).
//!   These verify the **synthesis** at the sample level — MODELLED, internally verified.
//! * **Instrument** tests (run against the bench; in CI against mocks): output power
//!   linearity and dynamic range, spurious/harmonics, channel isolation, PPS jitter, delay
//!   stability, reference lock, eLoran–GNSS synchronisation. Their results mean something
//!   only on the real bench with calibrated instruments.
//!
//! # RVM
//!
//! [`parse_requirements_csv`] reads the user's requirements (columns `id`, `text`, optional
//! `method` and `tests`); [`rvm`] maps each requirement to the tests that verify it
//! (from the CSV `tests` column and from each case's `requirement_ids`) and to their
//! verdicts; [`rvm_csv`] / [`rvm_json`] write the matrix.

use crate::hw::calib::{
    calibration_pulse_train, characterise_from_capture, receiver_delay, zero_crossing,
    CharacteriseOptions, ReceiverDelayInputs,
};
use crate::hw::{Device, DeviceStatus, StartTime, TxSession};
use crate::instr::{FrequencySource, PowerSource, SpectrumSource, TicSource};
use crate::scene::{MemSink, SampleSink};
use crate::signal::PulseSignal;
use crate::TruthRecord;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------------------

/// Acceptance limit: `min ≤ measured ≤ max` (either side optional).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    pub unit: String,
}

impl Limit {
    pub fn max(v: f64, unit: &str) -> Limit {
        Limit {
            min: None,
            max: Some(v),
            unit: unit.into(),
        }
    }
    pub fn min(v: f64, unit: &str) -> Limit {
        Limit {
            min: Some(v),
            max: None,
            unit: unit.into(),
        }
    }
    pub fn check(&self, x: f64) -> bool {
        x.is_finite() && self.min.is_none_or(|m| x >= m) && self.max.is_none_or(|m| x <= m)
    }
    pub fn describe(&self) -> String {
        match (self.min, self.max) {
            (Some(a), Some(b)) => format!("{a} ≤ x ≤ {b} {}", self.unit),
            (Some(a), None) => format!("≥ {a} {}", self.unit),
            (None, Some(b)) => format!("≤ {b} {}", self.unit),
            (None, None) => "no limit".into(),
        }
    }
}

/// What a test measures, with its parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TestKind {
    /// CW at each level (dBFS), power meter reading; measured = largest deviation (dB) from
    /// a 1 dB/dB line fitted through all points.
    PowerLinearity {
        channel: usize,
        freq_hz: f64,
        levels_dbfs: Vec<f64>,
    },
    /// CW stepped down from the top level, analyser band power; measured = span (dB) from
    /// the top level down to the lowest level still within `tol_db` of the 1 dB/dB line.
    DynamicRange {
        channel: usize,
        freq_hz: f64,
        levels_dbfs: Vec<f64>,
        band_hz: [f64; 2],
        rbw_hz: f64,
        tol_db: f64,
    },
    /// CW at `amp`; measured = worst of the harmonic band powers (2…N) and the highest
    /// other bin in the sweep, relative to the carrier band power (dBc).
    SpuriousHarmonics {
        channel: usize,
        freq_hz: f64,
        amp: f64,
        max_harmonic: u32,
        band_hz: f64,
        sweep_hz: [f64; 2],
        rbw_hz: f64,
    },
    /// CW on `driven`; measured = band power on `driven` − band power on `victim` (dB).
    ChannelIsolation {
        driven: usize,
        victim: usize,
        freq_hz: f64,
        amp: f64,
        band_hz: [f64; 2],
        rbw_hz: f64,
    },
    /// TIC reference PPS → simulator PPS; measured = standard deviation (ns).
    PpsJitter { n: usize },
    /// TIC readings split into `blocks`; measured = max − min of the block means (ns).
    DelayStability { n: usize, blocks: usize },
    /// Device reference-lock flag and, with a frequency counter, the fractional frequency
    /// offset of the 10 MHz output; measured = |Δf/f| (dimensionless).
    ReferenceLock { nominal_hz: f64, gate_s: f64 },
    /// Software: scenes whose time-reference offset steps by `step_ns`; measured = largest
    /// error (ns) of the zero-crossing shift against the commanded shift.
    ToaResolution {
        step_ns: f64,
        steps: usize,
        fs_hz: f64,
    },
    /// Software: transmitted ECD stepped by `step_ns`; measured = largest error (ns) of the
    /// envelope shift (matched filter) against the commanded ECD step.
    EcdResolution {
        step_ns: f64,
        steps: usize,
        fs_hz: f64,
    },
    /// Software: ASF (temporal ΔASF of a smooth-earth path) stepped by `step_ns`;
    /// measured = largest zero-crossing shift error (ns).
    AsfResolution {
        step_ns: f64,
        steps: usize,
        fs_hz: f64,
    },
    /// Software: WGN set for each target SNR (dB in 20 kHz); measured = largest error (dB)
    /// of the SNR measured on the samples.
    SnrRange { snr_db: Vec<f64>, fs_hz: f64 },
    /// Software: fixed skywave at each delay (µs); measured = largest error (ns) of the
    /// skywave delay measured on the samples.
    SkywaveDelayRange { delays_us: Vec<f64>, fs_hz: f64 },
    /// TIC eLoran reference point (start) → GNSS simulator 1PPS (stop); measured =
    /// |mean − expected| + 2·u_A (ns): the offset plus a k = 2 guard band for the Type A
    /// uncertainty (autocorrelation-aware, as [`crate::hw::calib::receiver_delay`]).
    EloranGnssSync { n: usize, expected_offset_ns: f64 },
}

impl TestKind {
    /// Whether the test runs without instruments.
    pub fn software_only(&self) -> bool {
        matches!(
            self,
            TestKind::ToaResolution { .. }
                | TestKind::EcdResolution { .. }
                | TestKind::AsfResolution { .. }
                | TestKind::SnrRange { .. }
                | TestKind::SkywaveDelayRange { .. }
        )
    }
}

/// One acceptance test.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    /// Requirement ids verified (matched against the requirements CSV).
    #[serde(default)]
    pub requirement_ids: Vec<String>,
    pub requirement: String,
    pub procedure: String,
    pub test: TestKind,
    pub limit: Limit,
}

/// Verdict of a test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail,
    /// Not run (instrument missing or not selected).
    NotRun,
    /// The procedure failed (instrument error, …).
    Error,
}

/// Result of one test.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub requirement_ids: Vec<String>,
    pub requirement: String,
    pub procedure: String,
    pub measured: Option<f64>,
    pub unit: String,
    pub limit: Limit,
    pub verdict: Verdict,
    /// Raw data and intermediate values.
    pub details: Value,
    pub message: String,
    /// Evidence files (result JSON, measurement logs).
    pub evidence: Vec<String>,
}

/// Result of a suite run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuiteReport {
    pub results: Vec<TestResult>,
    pub passed: usize,
    pub failed: usize,
    pub not_run: usize,
    pub errors: usize,
}

impl SuiteReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    pub fn get(&self, id: &str) -> Option<&TestResult> {
        self.results.iter().find(|r| r.id == id)
    }
}

// ---------------------------------------------------------------------------------------
// Stimulus
// ---------------------------------------------------------------------------------------

/// A stimulus the simulator emits while an instrument measures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StimulusSpec {
    /// Continuous sine of amplitude `amp` (fraction of full scale) on one channel.
    Cw {
        channel: usize,
        amp: f64,
        freq_hz: f64,
    },
    /// The calibration pulse train (one pulse per PPS) on one channel.
    PulseTrain { channel: usize, amp: f64 },
}

/// The simulator as a stimulus source.
pub trait Stimulus {
    /// Start emitting `spec` (replacing any running stimulus); returns once emitting.
    fn apply(&mut self, spec: &StimulusSpec) -> Result<(), String>;
    /// Stop emitting.
    fn off(&mut self) -> Result<(), String>;
    /// Device status (reference lock), when known.
    fn status(&self) -> Option<DeviceStatus> {
        None
    }
}

/// [`Stimulus`] over a [`Device`]: each stimulus streams for `duration_s` in a
/// [`TxSession`] (background threads) while the instruments measure. **UNTESTED ON
/// HARDWARE.** The device must be configured for `n_channels` TX channels at `fs_hz`.
pub struct DeviceStimulus {
    device: Option<Box<dyn Device>>,
    session: Option<TxSession<()>>,
    pub fs_hz: f64,
    pub n_channels: usize,
    pub duration_s: f64,
    pub pps_period_s: f64,
    last_status: Option<DeviceStatus>,
    /// Whether the running session's device is paced by a real clock.
    session_realtime: bool,
}

impl DeviceStimulus {
    pub fn new(device: Box<dyn Device>, fs_hz: f64, n_channels: usize, duration_s: f64) -> Self {
        DeviceStimulus {
            last_status: Some(device.status()),
            device: Some(device),
            session: None,
            fs_hz,
            n_channels,
            duration_s,
            pps_period_s: 1.0,
            session_realtime: false,
        }
    }
    pub fn into_device(mut self) -> Result<Option<Box<dyn Device>>, String> {
        self.off()?;
        Ok(self.device.take())
    }
}

fn stimulus_block(
    spec: &StimulusSpec,
    fs: f64,
    n_ch: usize,
    n0: usize,
    n: usize,
    period: f64,
) -> Vec<Vec<f64>> {
    let mut out = vec![vec![0.0; n]; n_ch];
    match spec {
        StimulusSpec::Cw {
            channel,
            amp,
            freq_hz,
        } => {
            if let Some(c) = out.get_mut(*channel) {
                for (i, v) in c.iter_mut().enumerate() {
                    let t = (n0 + i) as f64 / fs;
                    *v = amp * (2.0 * std::f64::consts::PI * freq_hz * t).sin();
                }
            }
        }
        StimulusSpec::PulseTrain { channel, amp } => {
            let t_a = n0 as f64 / fs;
            let k0 = ((t_a - 400e-6) / period).floor().max(0.0) as i64;
            let k1 = ((t_a + n as f64 / fs) / period).ceil() as i64;
            let pps: Vec<f64> = (k0..=k1).map(|k| k as f64 * period - t_a).collect();
            if let Some(c) = out.get_mut(*channel) {
                *c = calibration_pulse_train(fs, n, &pps, 0.0, *amp);
            }
        }
    }
    out
}

impl Stimulus for DeviceStimulus {
    fn apply(&mut self, spec: &StimulusSpec) -> Result<(), String> {
        self.off()?;
        let dev = self.device.take().ok_or("stimulus device unavailable")?;
        self.session_realtime = dev.realtime();
        let (fs, n_ch, period) = (self.fs_hz, self.n_channels, self.pps_period_s);
        let total = (self.duration_s * fs).round() as usize;
        let block = ((0.01 * fs) as usize).max(1);
        let spec = spec.clone();
        let s = TxSession::start(dev, StartTime::NextPps, 8, move |sink: &mut dyn SampleSink| {
            let mut n0 = 0;
            while n0 < total {
                let n = block.min(total - n0);
                match sink.write_block(&stimulus_block(&spec, fs, n_ch, n0, n, period)) {
                    Err(e) if e.contains("stopped") => return Ok(()),
                    r => r?,
                }
                n0 += n;
            }
            sink.finish()
        })?;
        self.session = Some(s);
        Ok(())
    }
    fn off(&mut self) -> Result<(), String> {
        if let Some(s) = self.session.take() {
            // A real-time device carries a continuous stimulus until switched off; a
            // non-real-time device (file) writes the finite stimulus to completion.
            if self.session_realtime {
                s.stop();
            }
            let r = s.wait()?;
            self.last_status = Some(r.device_status.clone());
            self.device = Some(r.device);
        }
        Ok(())
    }
    fn status(&self) -> Option<DeviceStatus> {
        self.device
            .as_ref()
            .map(|d| d.status())
            .or_else(|| self.last_status.clone())
    }
}

// ---------------------------------------------------------------------------------------
// Bench and runner
// ---------------------------------------------------------------------------------------

/// What the runner may use. Missing items make the tests that need them `NotRun`.
#[derive(Default)]
pub struct Bench<'a> {
    pub stimulus: Option<&'a mut dyn Stimulus>,
    pub power: Option<&'a mut dyn PowerSource>,
    pub spectrum: Option<&'a mut dyn SpectrumSource>,
    /// TIC: start = reference PPS, stop = simulator PPS / reference-point output.
    pub tic: Option<&'a mut dyn TicSource>,
    /// TIC: start = eLoran reference point, stop = GNSS simulator 1PPS.
    pub sync_tic: Option<&'a mut dyn TicSource>,
    pub freq: Option<&'a mut dyn FrequencySource>,
    /// Directory for per-test result JSON (evidence).
    pub evidence_dir: Option<PathBuf>,
    /// Extra evidence referenced by every instrument test (e.g. measurement log paths).
    pub evidence_refs: Vec<String>,
}

struct Outcome {
    measured: Option<f64>,
    details: Value,
    /// Verdict forced regardless of the limit (e.g. lock lost).
    force: Option<(Verdict, String)>,
}

fn out(measured: f64, details: Value) -> Result<Outcome, String> {
    Ok(Outcome {
        measured: Some(measured),
        details,
        force: None,
    })
}

const NOT_RUN: &str = "\u{0}notrun:";

fn missing(what: &str) -> String {
    format!("{NOT_RUN}{what} not provided")
}

fn with_stimulus<T>(
    b: &mut Bench,
    spec: &StimulusSpec,
    f: impl FnOnce(&mut Bench) -> Result<T, String>,
) -> Result<T, String> {
    b.stimulus
        .as_deref_mut()
        .ok_or_else(|| missing("stimulus"))?
        .apply(spec)?;
    let r = f(b);
    let off = b.stimulus.as_deref_mut().expect("checked").off();
    let v = r?;
    off?;
    Ok(v)
}

fn mean_std(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let s = if v.len() > 1 {
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    (m, s)
}

fn amp_of(dbfs: f64) -> f64 {
    10f64.powf(dbfs / 20.0)
}

fn run_kind(kind: &TestKind, b: &mut Bench) -> Result<Outcome, String> {
    match kind {
        TestKind::PowerLinearity {
            channel,
            freq_hz,
            levels_dbfs,
        } => {
            if b.power.is_none() {
                return Err(missing("power meter"));
            }
            let mut p = Vec::new();
            for &l in levels_dbfs {
                let spec = StimulusSpec::Cw {
                    channel: *channel,
                    amp: amp_of(l),
                    freq_hz: *freq_hz,
                };
                p.push(with_stimulus(b, &spec, |b| {
                    b.power.as_deref_mut().expect("checked").read_dbm(*freq_hz)
                })?);
            }
            if p.is_empty() {
                return Err("no levels".into());
            }
            let off = p.iter().zip(levels_dbfs).map(|(a, l)| a - l).sum::<f64>() / p.len() as f64;
            let dev: Vec<f64> = p.iter().zip(levels_dbfs).map(|(a, l)| a - l - off).collect();
            let m = dev.iter().map(|d| d.abs()).fold(0.0, f64::max);
            out(
                m,
                json!({"levels_dbfs": levels_dbfs, "power_dbm": p, "deviation_db": dev,
                       "full_scale_dbm": off}),
            )
        }
        TestKind::DynamicRange {
            channel,
            freq_hz,
            levels_dbfs,
            band_hz,
            rbw_hz,
            tol_db,
        } => {
            if b.spectrum.is_none() {
                return Err(missing("spectrum analyser"));
            }
            let mut lv = levels_dbfs.clone();
            lv.sort_by(|a, b| b.total_cmp(a));
            let mut pts = Vec::new();
            for &l in &lv {
                let spec = StimulusSpec::Cw {
                    channel: *channel,
                    amp: amp_of(l),
                    freq_hz: *freq_hz,
                };
                let p = with_stimulus(b, &spec, |b| {
                    let sa = b.spectrum.as_deref_mut().expect("checked");
                    sa.select_input(*channel)?;
                    let t = sa.sweep(band_hz[0], band_hz[1], *rbw_hz)?;
                    t.band_power_dbm(band_hz[0], band_hz[1])
                        .ok_or_else(|| "empty band".to_string())
                })?;
                pts.push((l, p));
            }
            let Some(&(l0, p0)) = pts.first() else {
                return Err("no levels".into());
            };
            let mut lowest = l0;
            for &(l, p) in &pts {
                if (p - l - (p0 - l0)).abs() <= *tol_db {
                    lowest = l;
                } else {
                    break;
                }
            }
            out(l0 - lowest, json!({"points": pts, "tol_db": tol_db}))
        }
        TestKind::SpuriousHarmonics {
            channel,
            freq_hz,
            amp,
            max_harmonic,
            band_hz,
            sweep_hz,
            rbw_hz,
        } => {
            if b.spectrum.is_none() {
                return Err(missing("spectrum analyser"));
            }
            let spec = StimulusSpec::Cw {
                channel: *channel,
                amp: *amp,
                freq_hz: *freq_hz,
            };
            let t = with_stimulus(b, &spec, |b| {
                let sa = b.spectrum.as_deref_mut().expect("checked");
                sa.select_input(*channel)?;
                sa.sweep(sweep_hz[0], sweep_hz[1], *rbw_hz)
            })?;
            let half = band_hz / 2.0;
            let pc = t
                .band_power_dbm(freq_hz - half, freq_hz + half)
                .ok_or("carrier outside the sweep")?;
            let mut harm = Vec::new();
            for h in 2..=*max_harmonic {
                let f = h as f64 * freq_hz;
                if let Some(p) = t.band_power_dbm(f - half, f + half) {
                    harm.push((h, p - pc));
                }
            }
            // highest bin outside the carrier and harmonic bands
            let excluded = |f: f64| {
                (1..=*max_harmonic).any(|h| (f - h as f64 * freq_hz).abs() <= half)
            };
            let spur = t
                .freqs_hz
                .iter()
                .zip(&t.dbm)
                .filter(|(f, _)| !excluded(**f))
                .map(|(f, p)| (*f, *p))
                .fold(None, |a: Option<(f64, f64)>, x| match a {
                    Some(y) if y.1 >= x.1 => Some(y),
                    _ => Some(x),
                });
            // a single bin is compared with the carrier's integrated power: conservative
            // for tones, which put all their power in about one RBW
            let spur_dbc = spur.map(|s| s.1 - pc);
            let worst = harm
                .iter()
                .map(|h| h.1)
                .chain(spur_dbc)
                .fold(f64::MIN, f64::max);
            out(
                worst,
                json!({"carrier_dbm": pc, "harmonics_dbc": harm, "worst_spur": spur,
                       "worst_spur_dbc": spur_dbc}),
            )
        }
        TestKind::ChannelIsolation {
            driven,
            victim,
            freq_hz,
            amp,
            band_hz,
            rbw_hz,
        } => {
            if b.spectrum.is_none() {
                return Err(missing("spectrum analyser"));
            }
            let spec = StimulusSpec::Cw {
                channel: *driven,
                amp: *amp,
                freq_hz: *freq_hz,
            };
            let (pd, pv) = with_stimulus(b, &spec, |b| {
                let sa = b.spectrum.as_deref_mut().expect("checked");
                let mut bp = |input: usize| -> Result<f64, String> {
                    sa.select_input(input)?;
                    sa.sweep(band_hz[0], band_hz[1], *rbw_hz)?
                        .band_power_dbm(band_hz[0], band_hz[1])
                        .ok_or_else(|| "empty band".to_string())
                };
                Ok((bp(*driven)?, bp(*victim)?))
            })?;
            out(pd - pv, json!({"driven_dbm": pd, "victim_dbm": pv}))
        }
        TestKind::PpsJitter { n } => {
            let tic = b.tic.as_deref_mut().ok_or_else(|| missing("TIC"))?;
            tic.arm_pps()?;
            let r = tic.read_n(*n)?;
            let (m, s) = mean_std(&r);
            let pp = r.iter().cloned().fold(f64::MIN, f64::max)
                - r.iter().cloned().fold(f64::MAX, f64::min);
            out(s, json!({"tic": tic.id(), "mean_ns": m, "std_ns": s, "pp_ns": pp, "readings_ns": r}))
        }
        TestKind::DelayStability { n, blocks } => {
            let tic = b.tic.as_deref_mut().ok_or_else(|| missing("TIC"))?;
            let blocks = (*blocks).max(1);
            if *n < blocks {
                return Err("n must be ≥ blocks".into());
            }
            tic.arm_pps()?;
            let r = tic.read_n(*n)?;
            let per = r.len() / blocks;
            let means: Vec<f64> = (0..blocks)
                .map(|k| mean_std(&r[k * per..(k + 1) * per]).0)
                .collect();
            let pp = means.iter().cloned().fold(f64::MIN, f64::max)
                - means.iter().cloned().fold(f64::MAX, f64::min);
            // linear drift over the readings (ns per reading)
            let nn = r.len() as f64;
            let mx = (nn - 1.0) / 2.0;
            let my = r.iter().sum::<f64>() / nn;
            let sxx: f64 = (0..r.len()).map(|i| (i as f64 - mx).powi(2)).sum();
            let sxy: f64 = r.iter().enumerate().map(|(i, y)| (i as f64 - mx) * (y - my)).sum();
            let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
            out(
                pp,
                json!({"tic": tic.id(), "block_means_ns": means, "drift_ns_per_reading": slope,
                       "readings_ns": r}),
            )
        }
        TestKind::ReferenceLock { nominal_hz, gate_s } => {
            let lock = b.stimulus.as_deref().and_then(|s| s.status()).and_then(|s| s.ref_locked);
            let f = match b.freq.as_deref_mut() {
                Some(fc) => Some(fc.read_hz(*gate_s)?),
                None => None,
            };
            let frac = f.map(|f| (f - nominal_hz).abs() / nominal_hz);
            let details = json!({"ref_locked": lock, "frequency_hz": f, "fractional_offset": frac});
            let force = match (lock, frac) {
                (Some(false), _) => Some((Verdict::Fail, "device reports reference unlocked".into())),
                (None, None) => {
                    return Err(missing("frequency counter and device lock status"));
                }
                (Some(true), None) => Some((
                    Verdict::Pass,
                    "lock flag only (no frequency counter): offset not measured".into(),
                )),
                _ => None,
            };
            Ok(Outcome {
                measured: frac,
                details,
                force,
            })
        }
        TestKind::ToaResolution {
            step_ns,
            steps,
            fs_hz,
        } => resolution(Quantity::Toa, *step_ns, *steps, *fs_hz),
        TestKind::EcdResolution {
            step_ns,
            steps,
            fs_hz,
        } => resolution(Quantity::Ecd, *step_ns, *steps, *fs_hz),
        TestKind::AsfResolution {
            step_ns,
            steps,
            fs_hz,
        } => resolution(Quantity::Asf, *step_ns, *steps, *fs_hz),
        TestKind::SnrRange { snr_db, fs_hz } => snr_range(snr_db, *fs_hz),
        TestKind::SkywaveDelayRange { delays_us, fs_hz } => skywave_range(delays_us, *fs_hz),
        TestKind::EloranGnssSync {
            n,
            expected_offset_ns,
        } => {
            let tic = b.sync_tic.as_deref_mut().ok_or_else(|| missing("sync TIC"))?;
            tic.arm_pps()?;
            let r = tic.read_n(*n)?;
            let rep = receiver_delay(
                &r,
                &ReceiverDelayInputs {
                    channel_delay_ns: 0.0,
                    channel_delay_u_ns: 0.0,
                    propagation_delay_ns: 0.0,
                    propagation_delay_u_ns: 0.0,
                    tic_offset_ns: 0.0,
                    type_b: vec![],
                    k: 2.0,
                },
            )?;
            let off = rep.mean_tic_ns - expected_offset_ns;
            out(
                off.abs() + 2.0 * rep.u_a_ns,
                json!({"tic": tic.id(), "mean_ns": rep.mean_tic_ns, "offset_ns": off,
                       "std_ns": rep.std_tic_ns, "u_a_ns": rep.u_a_ns,
                       "lag1_autocorrelation": rep.lag1_autocorrelation, "readings_ns": r}),
            )
        }
    }
}

/// Run the selected cases (all when `select` is `None`) in order.
pub fn run_suite(cases: &[TestCase], select: Option<&[String]>, b: &mut Bench) -> SuiteReport {
    let mut results = Vec::new();
    for c in cases {
        let selected = select.is_none_or(|s| s.iter().any(|x| *x == c.id));
        let (measured, details, verdict, message) = if !selected {
            (None, Value::Null, Verdict::NotRun, "not selected".to_string())
        } else {
            match run_kind(&c.test, b) {
                Ok(o) => match o.force {
                    Some((v, m)) => (o.measured, o.details, v, m),
                    None => match o.measured {
                        Some(x) if c.limit.check(x) => {
                            (o.measured, o.details, Verdict::Pass, String::new())
                        }
                        Some(_) => (
                            o.measured,
                            o.details,
                            Verdict::Fail,
                            format!("outside limit {}", c.limit.describe()),
                        ),
                        None => (None, o.details, Verdict::Error, "no measured value".into()),
                    },
                },
                Err(e) => match e.strip_prefix(NOT_RUN) {
                    Some(m) => (None, Value::Null, Verdict::NotRun, m.to_string()),
                    None => (None, Value::Null, Verdict::Error, e),
                },
            }
        };
        let mut r = TestResult {
            id: c.id.clone(),
            requirement_ids: c.requirement_ids.clone(),
            requirement: c.requirement.clone(),
            procedure: c.procedure.clone(),
            measured,
            unit: c.limit.unit.clone(),
            limit: c.limit.clone(),
            verdict,
            details,
            message,
            evidence: vec![],
        };
        if selected && !c.test.software_only() {
            r.evidence.extend(b.evidence_refs.iter().cloned());
        }
        if let (Some(dir), true) = (&b.evidence_dir, selected) {
            let p = dir.join(format!("{}.json", sanitize(&c.id)));
            r.evidence.insert(0, p.display().to_string());
            if let Err(e) = std::fs::create_dir_all(dir)
                .and_then(|_| std::fs::write(&p, serde_json::to_string_pretty(&r).expect("json")))
            {
                r.message = format!("{} (evidence not written: {e})", r.message);
            }
        }
        results.push(r);
    }
    let count = |v: Verdict| results.iter().filter(|r| r.verdict == v).count();
    SuiteReport {
        passed: count(Verdict::Pass),
        failed: count(Verdict::Fail),
        not_run: count(Verdict::NotRun),
        errors: count(Verdict::Error),
        results,
    }
}

fn sanitize(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Software (signal-level) tests on memory scenes
// ---------------------------------------------------------------------------------------

/// Parameters of a one-station test scene (synthetic geometry: receiver 55° N 0° E,
/// station 56° N 0° E, ≈ 111 km, GRI 40 000 µs).
#[derive(Clone, Debug, Default)]
struct SceneSpec {
    fs_hz: f64,
    time_ref_offset_ns: f64,
    ecd_us: f64,
    asf_ns: Option<f64>,
    sky: Option<f64>,
    noise_uv_m: Option<f64>,
}

/// Field (µV/m) that maps to sample value 1.0 in the test scenes.
const TEST_SCALE_UV_M: f64 = 10_000.0;

fn scene_toml(s: &SceneSpec) -> String {
    let mut t = format!(
        r#"[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.045
fs_hz = {fs}
seed = 3
output_scale_uv_m = {TEST_SCALE_UV_M}
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 0.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "REF"
offset_ns = {off}
"#,
        fs = s.fs_hz,
        off = s.time_ref_offset_ns
    );
    if let Some(a) = s.asf_ns {
        t += &format!(
            "[propagation.ground]\nmodel = \"smooth_earth\"\nsigma_s_m = 5.0\n[propagation.ground.temporal]\n\
             diurnal_amp_ns = {a}\ndiurnal_peak_hour_utc = 0.0\nscale_by_land_fraction = false\n"
        );
    }
    if let Some(d) = s.sky {
        t += &format!("[propagation.sky]\nmodel = \"fixed\"\ndelay_us = {d}\nratio_db = 0.0\n");
    }
    if let Some(n) = s.noise_uv_m {
        t += &format!("[[interference]]\ntype = \"wgn\"\nrms_uv_m_20khz = {n}\nseed = 5\n");
    }
    t += &format!(
        r#"[[chains]]
name = "T"
gri_us = 40000
[[chains.stations]]
id = "M"
role = "master"
position = {{ lat_deg = 56.0, lon_deg = 0.0 }}
time_ref = "REF"
ecd_us = {ecd}
"#,
        ecd = s.ecd_us
    );
    t
}

/// Run a scene to memory: channel 0 samples and the first truth record whose pulse lies
/// fully inside the scene.
fn run_scene(s: &SceneSpec) -> Result<(Vec<f64>, TruthRecord), String> {
    let (mut sink, truth, _) = crate::run_to_memory(&scene_toml(s))?;
    let x = sink.channels.swap_remove(0);
    let end = x.len() as f64 / s.fs_hz;
    let r = truth
        .into_iter()
        .filter(|t| t.toa_s > 1e-3 && t.toa_s + 3e-3 < end)
        .min_by(|a, b| a.toa_s.total_cmp(&b.toa_s))
        .ok_or("no complete pulse in the test scene")?;
    Ok((x, r))
}

/// ECD (µs) of the pulse whose time origin is `t_origin_s`: the envelope shift that
/// maximises the matched-filter output `Σ x·env(t − t₀ − e)·sin(2πf(t − t₀))` (grid search
/// over ±5 µs, then Newton). The carrier is fixed by `t_origin_s`, so only the envelope
/// moves — the definition of ECD (COMDTINST M16562.4A).
pub fn envelope_ecd_us(x: &[f64], fs_hz: f64, t_origin_s: f64) -> Option<f64> {
    let sig = PulseSignal::loran_c();
    let ext = sig.envelope.extent_s() + 10e-6;
    let a = ((t_origin_s - 10e-6) * fs_hz).floor().max(0.0) as usize;
    let b = (((t_origin_s + ext) * fs_hz).ceil() as usize).min(x.len());
    if a >= b {
        return None;
    }
    let c = |e_s: f64| -> f64 {
        (a..b)
            .map(|i| x[i] * sig.pulse(i as f64 / fs_hz - t_origin_s, e_s, 0.0))
            .sum()
    };
    let mut best = (f64::MIN, 0.0);
    for k in -20..=20 {
        let e = k as f64 * 0.25e-6;
        let v = c(e);
        if v > best.0 {
            best = (v, e);
        }
    }
    let mut e = best.1;
    let h = 20e-9;
    for _ in 0..20 {
        let (cm, c0, cp) = (c(e - h), c(e), c(e + h));
        let d1 = (cp - cm) / (2.0 * h);
        let d2 = (cp - 2.0 * c0 + cm) / (h * h);
        if d2.is_nan() || d2 >= 0.0 {
            break;
        }
        let step = (-d1 / d2).clamp(-0.25e-6, 0.25e-6);
        e += step;
        if step.abs() < 1e-15 {
            break;
        }
    }
    Some(e * 1e6)
}

#[derive(Clone, Copy, Debug)]
enum Quantity {
    Toa,
    Ecd,
    Asf,
}

fn resolution(q: Quantity, step_ns: f64, steps: usize, fs_hz: f64) -> Result<Outcome, String> {
    if steps == 0 {
        return Err("steps must be ≥ 1".into());
    }
    let spec = |k: usize| {
        let v = k as f64 * step_ns;
        let mut s = SceneSpec {
            fs_hz,
            ..SceneSpec::default()
        };
        match q {
            // a reference running behind UTC by v emits v later
            Quantity::Toa => s.time_ref_offset_ns = -v,
            Quantity::Ecd => s.ecd_us = v * 1e-3,
            Quantity::Asf => s.asf_ns = Some(v),
        }
        s
    };
    let (x0, r0) = run_scene(&spec(0))?;
    let guess = r0.toa_s + 30e-6;
    let feature = |x: &[f64]| -> Result<f64, String> {
        let zc = zero_crossing(x, fs_hz, guess).ok_or("no standard zero crossing found")?;
        Ok(match q {
            Quantity::Toa | Quantity::Asf => zc * 1e9,
            Quantity::Ecd => {
                envelope_ecd_us(x, fs_hz, zc - 30e-6).ok_or("ECD estimate failed")? * 1e3
            }
        })
    };
    let f0 = feature(&x0)?;
    let mut rows = Vec::new();
    let mut worst: f64 = 0.0;
    for k in 1..=steps {
        let (x, r) = run_scene(&spec(k))?;
        let d = feature(&x)? - f0;
        let want = k as f64 * step_ns;
        worst = worst.max((d - want).abs());
        let truth = match q {
            Quantity::Toa => (r.toa_s - r0.toa_s) * 1e9,
            Quantity::Ecd => (r.ecd_us - r0.ecd_us) * 1e3,
            Quantity::Asf => r.asf_ns - r0.asf_ns,
        };
        rows.push(json!({"commanded_ns": want, "measured_ns": d, "truth_ns": truth,
                         "error_ns": d - want}));
    }
    out(
        worst,
        json!({"quantity": format!("{q:?}"), "fs_hz": fs_hz, "steps": rows,
               "method": match q {
                   Quantity::Ecd => "envelope matched filter (carrier fixed at the standard zero crossing)",
                   _ => "standard zero crossing, 4-point Lagrange cubic (hw::calib::zero_crossing)",
               }}),
    )
}

/// Sine amplitude at the eLoran carrier fitted by least squares over `[t_a, t_b]`.
fn carrier_amplitude(x: &[f64], fs: f64, t_a: f64, t_b: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * PulseSignal::loran_c().carrier_hz;
    let (a, b) = ((t_a * fs).ceil() as usize, ((t_b * fs).floor() as usize).min(x.len()));
    let (mut ss, mut cc, mut sc, mut xs, mut xc) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (i, v) in x.iter().enumerate().take(b).skip(a) {
        let (s, c) = (w * i as f64 / fs).sin_cos();
        ss += s * s;
        cc += c * c;
        sc += s * c;
        xs += v * s;
        xc += v * c;
    }
    let det = ss * cc - sc * sc;
    if det == 0.0 {
        return 0.0;
    }
    let p = (xs * cc - xc * sc) / det;
    let q = (xc * ss - xs * sc) / det;
    (p * p + q * q).sqrt()
}

fn snr_range(targets: &[f64], fs_hz: f64) -> Result<Outcome, String> {
    let base = SceneSpec {
        fs_hz,
        ..SceneSpec::default()
    };
    let (x0, r0) = run_scene(&base)?;
    // pulse peak at 65 µs after the time origin: fit the carrier over ±5 µs around it
    let amp = carrier_amplitude(&x0, fs_hz, r0.toa_s + 60e-6, r0.toa_s + 70e-6) * TEST_SCALE_UV_M;
    let sig_rms = amp / 2f64.sqrt();
    let mut rows = Vec::new();
    let mut worst: f64 = 0.0;
    for &snr in targets {
        let noise = r0.field_uv_m / 10f64.powf(snr / 20.0);
        let (x, r) = run_scene(&SceneSpec {
            noise_uv_m: Some(noise),
            ..base.clone()
        })?;
        let ms: f64 = x.iter().zip(&x0).map(|(a, b)| (a - b).powi(2)).sum::<f64>() / x.len() as f64;
        let n20 = ms.sqrt() * TEST_SCALE_UV_M * (20e3 / (fs_hz / 2.0)).sqrt();
        let meas = 20.0 * (sig_rms / n20).log10();
        worst = worst.max((meas - snr).abs());
        rows.push(json!({"target_db": snr, "measured_db": meas, "truth_db": r.snr_db,
                         "noise_rms_uv_m_20khz": n20}));
    }
    out(
        worst,
        json!({"signal_rms_uv_m": sig_rms, "truth_field_uv_m": r0.field_uv_m, "points": rows,
               "method": "noise = noisy − noiseless scene (same seed), RMS scaled to 20 kHz; \
                          signal = carrier LS fit at the pulse peak"}),
    )
}

fn skywave_range(delays_us: &[f64], fs_hz: f64) -> Result<Outcome, String> {
    let base = SceneSpec {
        fs_hz,
        ..SceneSpec::default()
    };
    let (xg, rg) = run_scene(&base)?;
    let mut rows = Vec::new();
    let mut worst: f64 = 0.0;
    for &d in delays_us {
        let (xs, rs) = run_scene(&SceneSpec {
            sky: Some(d),
            ..base.clone()
        })?;
        let resid: Vec<f64> = xs.iter().zip(&xg).map(|(a, b)| a - b).collect();
        let e = characterise_from_capture(
            &resid,
            fs_hz,
            &[rg.toa_s],
            &CharacteriseOptions {
                expected_delay_ns: d * 1e3,
                search_ns: 3000.0,
            },
        )?;
        let err = e.delay_ns - d * 1e3;
        worst = worst.max(err.abs());
        rows.push(json!({"commanded_us": d, "measured_us": e.delay_ns * 1e-3,
                         "truth_us": rs.sky_delay_us, "error_ns": err}));
    }
    out(
        worst,
        json!({"points": rows, "method": "skywave isolated as (ground+sky) − ground scene, \
               timed with the calibration matched filter (hw::calib::characterise_from_capture)"}),
    )
}

// ---------------------------------------------------------------------------------------
// Default suite
// ---------------------------------------------------------------------------------------

/// The standard acceptance suite. Limits marked "placeholder" must be replaced with the
/// procurement specification's values.
pub fn default_suite() -> Vec<TestCase> {
    let ph = " (placeholder limit: set from the specification)";
    let tc = |id: &str, req: &str, proc_: &str, test: TestKind, limit: Limit| TestCase {
        id: id.into(),
        requirement_ids: vec![],
        requirement: req.into(),
        procedure: proc_.into(),
        test,
        limit,
    };
    vec![
        tc(
            "AT-PWR-LIN",
            &format!("Output power shall be linear within 0.5 dB over the output range{ph}"),
            "CW 100 kHz at 0, −10, −20, −30, −40 dBFS on channel 0; power meter at the connector; fit a 1 dB/dB line",
            TestKind::PowerLinearity { channel: 0, freq_hz: 100e3, levels_dbfs: vec![0.0, -10.0, -20.0, -30.0, -40.0] },
            Limit::max(0.5, "dB"),
        ),
        tc(
            "AT-PWR-DR",
            &format!("Output dynamic range shall be at least 60 dB{ph}"),
            "CW 100 kHz stepped down in 10 dB steps from 0 dBFS; analyser band power 99–101 kHz; range while within 1 dB of the line",
            TestKind::DynamicRange { channel: 0, freq_hz: 100e3, levels_dbfs: (0..=10).map(|k| -10.0 * k as f64).collect(), band_hz: [99e3, 101e3], rbw_hz: 100.0, tol_db: 1.0 },
            Limit::min(60.0, "dB"),
        ),
        tc(
            "AT-SPUR",
            &format!("Harmonics and spurious outputs shall be below −40 dBc{ph}"),
            "CW 100 kHz at −6 dBFS; sweep 10 kHz–1.05 MHz; harmonic band powers 2…10 and highest other bin relative to the carrier",
            TestKind::SpuriousHarmonics { channel: 0, freq_hz: 100e3, amp: 0.5, max_harmonic: 10, band_hz: 2e3, sweep_hz: [10e3, 1.05e6], rbw_hz: 300.0 },
            Limit::max(-40.0, "dBc"),
        ),
        tc(
            "AT-ISO",
            &format!("Isolation between output channels shall be at least 60 dB{ph}"),
            "CW 100 kHz at −6 dBFS on channel 0 only; band power 90–110 kHz on channel 0 and channel 1",
            TestKind::ChannelIsolation { driven: 0, victim: 1, freq_hz: 100e3, amp: 0.5, band_hz: [90e3, 110e3], rbw_hz: 300.0 },
            Limit::min(60.0, "dB"),
        ),
        tc(
            "AT-PPS-JIT",
            &format!("1PPS output jitter shall be below 1 ns RMS{ph}"),
            "TIC start = reference 1PPS, stop = simulator 1PPS; 100 readings; standard deviation",
            TestKind::PpsJitter { n: 100 },
            Limit::max(1.0, "ns"),
        ),
        tc(
            "AT-DLY-STAB",
            &format!("Output delay shall be stable within 2 ns peak-to-peak over the test duration{ph}"),
            "TIC reference 1PPS → reference-point output, 600 readings (10 min at 1 PPS) in 10 blocks; max − min of block means",
            TestKind::DelayStability { n: 600, blocks: 10 },
            Limit::max(2.0, "ns"),
        ),
        tc(
            "AT-REF-LOCK",
            &format!("The simulator shall lock to the external 10 MHz reference (|Δf/f| ≤ 1e-11){ph}"),
            "Device lock status; frequency counter on the 10 MHz output, 10 s gate",
            TestKind::ReferenceLock { nominal_hz: 10e6, gate_s: 10.0 },
            Limit::max(1e-11, "1"),
        ),
        tc(
            "AT-RES-TOA",
            "ToA shall be settable in steps of 10 ns",
            "Software: scenes with the station's time reference stepped by 10 ns (5 steps); standard zero crossing on the samples",
            TestKind::ToaResolution { step_ns: 10.0, steps: 5, fs_hz: 1e6 },
            Limit::max(1.0, "ns"),
        ),
        tc(
            "AT-RES-ECD",
            "ECD shall be settable in steps of 10 ns",
            "Software: transmitted ECD stepped by 10 ns (5 steps); envelope matched filter on the samples",
            TestKind::EcdResolution { step_ns: 10.0, steps: 5, fs_hz: 1e6 },
            Limit::max(1.0, "ns"),
        ),
        tc(
            "AT-RES-ASF",
            "ASF shall be settable in steps of 10 ns",
            "Software: ΔASF of a smooth-earth path stepped by 10 ns (5 steps); standard zero crossing on the samples",
            TestKind::AsfResolution { step_ns: 10.0, steps: 5, fs_hz: 1e6 },
            Limit::max(1.0, "ns"),
        ),
        tc(
            "AT-SNR-RNG",
            "SNR shall be settable from −10 dB to +60 dB (20 kHz bandwidth)",
            "Software: WGN set for −10, 0, 20, 40, 60 dB SNR; SNR measured on the samples",
            TestKind::SnrRange { snr_db: vec![-10.0, 0.0, 20.0, 40.0, 60.0], fs_hz: 1e6 },
            Limit::max(1.0, "dB"),
        ),
        tc(
            "AT-SKY-RNG",
            "Skywave delay shall be settable from 30 µs to 2000 µs after the groundwave",
            "Software: fixed skywave (0 dB ratio) at 30, 100, 500, 1000, 2000 µs; delay measured on the differenced samples",
            TestKind::SkywaveDelayRange { delays_us: vec![30.0, 100.0, 500.0, 1000.0, 2000.0], fs_hz: 1e6 },
            Limit::max(1.0, "ns"),
        ),
        tc(
            "AT-SYNC-GNSS",
            "eLoran and GNSS simulator outputs shall be synchronised within 10 ns",
            "TIC start = eLoran reference-point output, stop = GNSS simulator 1PPS; 100 readings; |mean − expected| + 2·u_A",
            TestKind::EloranGnssSync { n: 100, expected_offset_ns: 0.0 },
            Limit::max(10.0, "ns"),
        ),
    ]
}

// ---------------------------------------------------------------------------------------
// Requirements verification matrix
// ---------------------------------------------------------------------------------------

/// A requirement from the user's CSV.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    /// Verification method (Test / Analysis / Inspection / Demonstration).
    pub method: String,
    /// Test ids named in the CSV.
    pub tests: Vec<String>,
}

/// Split one CSV line (RFC 4180 quoting with `""` escapes).
pub fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match (c, q) {
            ('"', true) if it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            ('"', _) => q = !q,
            (',', false) => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

/// Parse a requirements CSV with a header row naming `id`, `text` (or `requirement`) and
/// optionally `method` and `tests` (test ids separated by `;` or spaces). `#` lines are
/// comments.
pub fn parse_requirements_csv(text: &str) -> Result<Vec<Requirement>, String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let header: Vec<String> = split_csv_line(lines.next().ok_or("empty requirements CSV")?)
        .into_iter()
        .map(|h| h.to_ascii_lowercase())
        .collect();
    let col = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
    let id = col(&["id", "req_id", "requirement_id"]).ok_or("requirements CSV needs an id column")?;
    let tx = col(&["text", "requirement", "description"])
        .ok_or("requirements CSV needs a text column")?;
    let me = col(&["method", "verification_method"]);
    let ts = col(&["tests", "test_ids"]);
    let mut v = Vec::new();
    for (i, l) in lines.enumerate() {
        let f = split_csv_line(l);
        let get = |k: Option<usize>| k.and_then(|k| f.get(k)).cloned().unwrap_or_default();
        let rid = get(Some(id));
        if rid.is_empty() {
            return Err(format!("requirements CSV row {}: empty id", i + 2));
        }
        v.push(Requirement {
            id: rid,
            text: get(Some(tx)),
            method: get(me),
            tests: get(ts)
                .split([';', ' '])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }
    Ok(v)
}

/// RVM status of a requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RvmStatus {
    Verified,
    Failed,
    /// Some mapped tests not run, errored or missing from the report.
    Incomplete,
    /// No test maps to the requirement.
    NotCovered,
}

/// One RVM row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RvmRow {
    pub requirement_id: String,
    pub requirement_text: String,
    pub method: String,
    pub test_ids: Vec<String>,
    /// `test_id: verdict (measured unit)` per mapped test.
    pub results: Vec<String>,
    pub evidence: Vec<String>,
    pub status: RvmStatus,
}

/// Build the matrix. Tests map to a requirement through the CSV `tests` column and the
/// cases' `requirement_ids`.
pub fn rvm(reqs: &[Requirement], cases: &[TestCase], report: Option<&SuiteReport>) -> Vec<RvmRow> {
    reqs.iter()
        .map(|r| {
            let mut ids: Vec<String> = r.tests.clone();
            for c in cases {
                if c.requirement_ids.contains(&r.id) && !ids.contains(&c.id) {
                    ids.push(c.id.clone());
                }
            }
            let mut results = Vec::new();
            let mut evidence = Vec::new();
            let mut verdicts = Vec::new();
            for t in &ids {
                match report.and_then(|rep| rep.get(t)) {
                    Some(res) => {
                        verdicts.push(Some(res.verdict));
                        let m = res
                            .measured
                            .map(|m| format!(" ({m:.4} {})", res.unit))
                            .unwrap_or_default();
                        results.push(format!("{t}: {:?}{m}", res.verdict));
                        evidence.extend(res.evidence.iter().cloned());
                    }
                    None => {
                        verdicts.push(None);
                        results.push(format!("{t}: no result"));
                    }
                }
            }
            let status = if ids.is_empty() {
                RvmStatus::NotCovered
            } else if verdicts.contains(&Some(Verdict::Fail)) {
                RvmStatus::Failed
            } else if verdicts.iter().all(|v| *v == Some(Verdict::Pass)) {
                RvmStatus::Verified
            } else {
                RvmStatus::Incomplete
            };
            RvmRow {
                requirement_id: r.id.clone(),
                requirement_text: r.text.clone(),
                method: r.method.clone(),
                test_ids: ids,
                results,
                evidence,
                status,
            }
        })
        .collect()
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// RVM as CSV.
pub fn rvm_csv(rows: &[RvmRow]) -> String {
    let mut s = String::from("requirement_id,requirement_text,method,test_ids,results,status,evidence\n");
    for r in rows {
        let f = [
            r.requirement_id.clone(),
            r.requirement_text.clone(),
            r.method.clone(),
            r.test_ids.join(";"),
            r.results.join("; "),
            serde_json::to_value(r.status)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            r.evidence.join(";"),
        ];
        s.push_str(&f.iter().map(|x| csv_field(x)).collect::<Vec<_>>().join(","));
        s.push('\n');
    }
    s
}

/// RVM as JSON.
pub fn rvm_json(rows: &[RvmRow]) -> String {
    serde_json::to_string_pretty(rows).expect("serialisable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instr::Trace;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn software_tests_run_in_ci_and_pass() {
        let cases: Vec<TestCase> = default_suite()
            .into_iter()
            .filter(|c| c.test.software_only())
            .collect();
        assert_eq!(cases.len(), 5);
        let rep = run_suite(&cases, None, &mut Bench::default());
        for r in &rep.results {
            assert_eq!(r.verdict, Verdict::Pass, "{} {:?} {}", r.id, r.measured, r.details);
        }
        // the sample-level measurements are far inside the 1 ns / 1 dB limits
        // At 1 MS/s (10 samples per carrier cycle) the 4-point zero-crossing interpolation
        // contributes ~0.1 ns; measured 0.135 ns, far inside the 1 ns limit.
        assert!(rep.get("AT-RES-TOA").unwrap().measured.unwrap() < 0.25);
        assert!(rep.get("AT-RES-ASF").unwrap().measured.unwrap() < 0.05);
        assert!(rep.get("AT-RES-ECD").unwrap().measured.unwrap() < 0.1);
        assert!(rep.get("AT-SKY-RNG").unwrap().measured.unwrap() < 0.2);
        assert!(rep.get("AT-SNR-RNG").unwrap().measured.unwrap() < 0.5);
    }

    #[test]
    fn a_wrong_limit_fails_and_missing_instruments_are_not_run() {
        let mut c = default_suite();
        let toa = c.iter_mut().find(|c| c.id == "AT-RES-TOA").unwrap();
        toa.test = TestKind::ToaResolution {
            step_ns: 10.0,
            steps: 1,
            fs_hz: 1e6,
        };
        toa.limit = Limit::max(-1.0, "ns"); // impossible
        let sel = vec!["AT-RES-TOA".to_string(), "AT-PPS-JIT".to_string()];
        let rep = run_suite(&c, Some(&sel), &mut Bench::default());
        assert_eq!(rep.get("AT-RES-TOA").unwrap().verdict, Verdict::Fail);
        let j = rep.get("AT-PPS-JIT").unwrap();
        assert_eq!(j.verdict, Verdict::NotRun);
        assert!(j.message.contains("TIC not provided"));
        assert_eq!(rep.get("AT-SPUR").unwrap().message, "not selected");
        assert_eq!((rep.failed, rep.passed), (1, 0));
    }

    // ---- instrument mocks sharing the stimulus state ----

    #[derive(Default)]
    struct State {
        spec: Option<StimulusSpec>,
        input: usize,
    }

    struct MockStim(Rc<RefCell<State>>, bool);
    impl Stimulus for MockStim {
        fn apply(&mut self, s: &StimulusSpec) -> Result<(), String> {
            self.0.borrow_mut().spec = Some(s.clone());
            Ok(())
        }
        fn off(&mut self) -> Result<(), String> {
            self.0.borrow_mut().spec = None;
            Ok(())
        }
        fn status(&self) -> Option<DeviceStatus> {
            Some(DeviceStatus {
                ref_locked: Some(self.1),
                ..DeviceStatus::default()
            })
        }
    }

    /// Output at the connector: +10 dBm at full scale, compressing above −3 dBFS, noise
    /// floor −110 dBm; 2nd harmonic −55 dBc; −75 dB crosstalk to other channels.
    fn conn_dbm(st: &State, input: usize) -> Option<(f64, f64)> {
        match st.spec.as_ref()? {
            StimulusSpec::Cw {
                channel,
                amp,
                freq_hz,
            } => {
                let l = 20.0 * amp.log10();
                let mut p = 10.0 + l - if l > -3.0 { (l + 3.0) * 0.1 } else { 0.0 };
                if *channel != input {
                    p -= 75.0;
                }
                Some((*freq_hz, p))
            }
            _ => None,
        }
    }

    struct MockPm(Rc<RefCell<State>>);
    impl PowerSource for MockPm {
        fn id(&self) -> String {
            "pm".into()
        }
        fn read_dbm(&mut self, _f: f64) -> Result<f64, String> {
            let st = self.0.borrow();
            let p = conn_dbm(&st, 0).map(|x| x.1).unwrap_or(-110.0);
            Ok(10.0 * (10f64.powf(p / 10.0) + 10f64.powf(-11.0)).log10())
        }
    }

    struct MockSa(Rc<RefCell<State>>);
    impl SpectrumSource for MockSa {
        fn id(&self) -> String {
            "sa".into()
        }
        fn select_input(&mut self, input: usize) -> Result<(), String> {
            self.0.borrow_mut().input = input;
            Ok(())
        }
        fn sweep(&mut self, a: f64, b: f64, rbw: f64) -> Result<Trace, String> {
            let st = self.0.borrow();
            let n = ((b - a) / (rbw / 2.0)).round() as usize + 1;
            let mut t = Trace::uniform(a, b, vec![-110.0; n], rbw, 1.0);
            if let Some((f, p)) = conn_dbm(&st, st.input) {
                for (h, dbc) in [(1.0, 0.0), (2.0, -55.0), (3.0, -70.0)] {
                    // tone: put its power in the nearest bin, scaled for the band sum
                    let fh = f * h;
                    // nearest bin (the sweep grid need not contain the tone frequency)
                    let nearest = t
                        .freqs_hz
                        .iter()
                        .enumerate()
                        .min_by(|a, b| (a.1 - fh).abs().total_cmp(&(b.1 - fh).abs()))
                        .filter(|(_, x)| (*x - fh).abs() <= rbw)
                        .map(|(i, _)| i);
                    if let Some(i) = nearest {
                        // band_power sums bins × spacing/rbw = ½: put ×2 in one bin
                        let tone = p + dbc + 10.0 * 2f64.log10();
                        t.dbm[i] = 10.0
                            * (10f64.powf(tone / 10.0) + 10f64.powf(t.dbm[i] / 10.0)).log10();
                    }
                }
            }
            Ok(t)
        }
        fn marker_peak(&mut self) -> Result<(f64, f64), String> {
            Err("unused".into())
        }
    }

    struct MockTic {
        k: usize,
        base: f64,
        drift: f64,
    }
    impl TicSource for MockTic {
        fn id(&self) -> String {
            "tic".into()
        }
        fn arm_pps(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn read_n(&mut self, n: usize) -> Result<Vec<f64>, String> {
            Ok((0..n)
                .map(|_| {
                    self.k += 1;
                    let w = [0.3, -0.2, 0.1, -0.4, 0.2][self.k % 5];
                    self.base + w + self.drift * self.k as f64
                })
                .collect())
        }
    }

    struct MockFc(f64);
    impl FrequencySource for MockFc {
        fn id(&self) -> String {
            "fc".into()
        }
        fn read_hz(&mut self, _g: f64) -> Result<f64, String> {
            Ok(self.0)
        }
    }

    #[test]
    fn instrument_tests_against_mocks_and_rvm() {
        let st = Rc::new(RefCell::new(State::default()));
        let mut stim = MockStim(st.clone(), true);
        let mut pm = MockPm(st.clone());
        let mut sa = MockSa(st.clone());
        let mut tic = MockTic {
            k: 0,
            base: 50.0,
            drift: 0.0,
        };
        let mut stic = MockTic {
            k: 0,
            base: 3.0,
            drift: 0.0,
        };
        let mut fc = MockFc(10e6 * (1.0 + 2e-12));
        let dir = std::env::temp_dir().join(format!("kshana-suite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cases: Vec<TestCase> = default_suite()
            .into_iter()
            .filter(|c| !c.test.software_only())
            .collect();
        // shorten the TIC tests
        for c in cases.iter_mut() {
            match &mut c.test {
                TestKind::DelayStability { n, .. } => *n = 100,
                TestKind::PpsJitter { n } | TestKind::EloranGnssSync { n, .. } => *n = 50,
                _ => {}
            }
        }
        cases[0].requirement_ids = vec!["R-1".into()];
        let rep = {
            let mut b = Bench {
                stimulus: Some(&mut stim),
                power: Some(&mut pm),
                spectrum: Some(&mut sa),
                tic: Some(&mut tic),
                sync_tic: Some(&mut stic),
                freq: Some(&mut fc),
                evidence_dir: Some(dir.clone()),
                evidence_refs: vec!["instruments.jsonl".into()],
            };
            run_suite(&cases, None, &mut b)
        };
        let get = |id: &str| rep.get(id).unwrap();
        // linearity: compression of 0.3 dB at 0 dBFS, spread over the fitted line
        let lin = get("AT-PWR-LIN");
        assert_eq!(lin.verdict, Verdict::Pass, "{:?}", lin);
        assert!(lin.measured.unwrap() > 0.1 && lin.measured.unwrap() < 0.5);
        // dynamic range: 0 dBFS compresses 0.3 dB (within 1 dB); floor −110 dBm limits
        // the low end: 10 + L ≫ −110 + band noise
        let dr = get("AT-PWR-DR");
        assert_eq!(dr.verdict, Verdict::Pass, "{dr:?}");
        assert!(dr.measured.unwrap() >= 90.0, "{dr:?}");
        let sp = get("AT-SPUR");
        assert!((sp.measured.unwrap() + 55.0).abs() < 0.5, "{sp:?}");
        assert_eq!(sp.verdict, Verdict::Pass);
        let iso = get("AT-ISO");
        assert!((iso.measured.unwrap() - 75.0).abs() < 0.5, "{iso:?}");
        let j = get("AT-PPS-JIT");
        assert_eq!(j.verdict, Verdict::Pass);
        // pattern (−0.2, +0.1, −0.4, +0.2, +0.3) ns repeated over 100 readings: s = √(0.068·100/99) ≈ 0.2634 ns
        assert!((j.measured.unwrap() - 0.2634).abs() < 0.002, "{j:?}");
        assert_eq!(get("AT-DLY-STAB").verdict, Verdict::Pass);
        assert_eq!(get("AT-REF-LOCK").verdict, Verdict::Pass);
        let s = get("AT-SYNC-GNSS");
        assert_eq!(s.verdict, Verdict::Pass, "{s:?}");
        assert!(s.measured.unwrap() > 3.0 && s.measured.unwrap() < 3.5);
        assert!(st.borrow().spec.is_none(), "stimulus left on");
        // evidence written and referenced
        assert!(lin.evidence[0].ends_with("AT-PWR-LIN.json"));
        assert!(lin.evidence.contains(&"instruments.jsonl".to_string()));
        assert!(std::path::Path::new(&lin.evidence[0]).exists());

        // drifting delay and an unlocked reference fail
        let mut tic = MockTic {
            k: 0,
            base: 50.0,
            drift: 0.1,
        };
        // 12 ns offset: outside the 10 ns sync limit
        let mut stic = MockTic {
            k: 0,
            base: 12.0,
            drift: 0.0,
        };
        let mut stim = MockStim(st.clone(), false);
        let rep2 = {
            let mut b = Bench {
                stimulus: Some(&mut stim),
                tic: Some(&mut tic),
                sync_tic: Some(&mut stic),
                ..Bench::default()
            };
            run_suite(&cases, None, &mut b)
        };
        assert_eq!(rep2.get("AT-DLY-STAB").unwrap().verdict, Verdict::Fail);
        assert_eq!(rep2.get("AT-REF-LOCK").unwrap().verdict, Verdict::Fail);
        assert_eq!(rep2.get("AT-SYNC-GNSS").unwrap().verdict, Verdict::Fail);
        assert_eq!(rep2.get("AT-PWR-LIN").unwrap().verdict, Verdict::NotRun);

        // RVM
        let reqs = parse_requirements_csv(
            "# lab requirements\nID,Text,Method,Tests\nR-1,\"Power linear, 0.5 dB\",Test,\n\
             R-2,Sync 10 ns,Test,AT-SYNC-GNSS\nR-3,Isolation,Test,AT-ISO;AT-SPUR\nR-4,GUI,Inspection,\n\
             R-5,PPS,Test,AT-PPS-JIT AT-NOPE\n",
        )
        .unwrap();
        assert_eq!(reqs[0].text, "Power linear, 0.5 dB");
        let m = rvm(&reqs, &cases, Some(&rep));
        assert_eq!(m[0].test_ids, vec!["AT-PWR-LIN"]);
        assert_eq!(m[0].status, RvmStatus::Verified);
        assert_eq!(m[2].status, RvmStatus::Verified);
        assert_eq!(m[3].status, RvmStatus::NotCovered);
        assert_eq!(m[4].status, RvmStatus::Incomplete);
        assert_eq!(rvm(&reqs, &cases, Some(&rep2))[1].status, RvmStatus::Failed);
        let csv = rvm_csv(&m);
        assert!(csv.starts_with("requirement_id,"));
        assert!(csv.contains("\"Power linear, 0.5 dB\""));
        assert!(csv.contains("not_covered"));
        let back: Vec<RvmRow> = serde_json::from_str(&rvm_json(&m)).unwrap();
        assert_eq!(back, m);
        assert!(parse_requirements_csv("foo,bar\n1,2").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn suite_definition_round_trips_and_device_stimulus_streams() {
        let s = default_suite();
        let j = serde_json::to_string(&s).unwrap();
        let back: Vec<TestCase> = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
        assert!(s
            .iter()
            .any(|c| c.id == "AT-SYNC-GNSS" && c.limit.max == Some(10.0)));
        assert_eq!(split_csv_line(r#"a,"b ""q"", c",d"#), vec!["a", "b \"q\", c", "d"]);

        // DeviceStimulus over a file device (non-real-time)
        let d = std::env::temp_dir().join(format!("kshana-stim-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let mut dev = crate::hw::file::FileDevice::tx(&d.join("stim"), crate::io::Format::Rf32);
        dev.configure(&crate::hw::DeviceConfig::tx(1e6, 2)).unwrap();
        let mut stim = DeviceStimulus::new(Box::new(dev), 1e6, 2, 0.02);
        stim.apply(&StimulusSpec::Cw {
            channel: 1,
            amp: 0.5,
            freq_hz: 100e3,
        })
        .unwrap();
        stim.off().unwrap();
        stim.pps_period_s = 5e-3;
        stim.apply(&StimulusSpec::PulseTrain { channel: 0, amp: 0.5 })
            .unwrap();
        let dev = stim.into_device().unwrap().unwrap();
        assert!(dev.status().samples_tx > 0);
        let blk = stimulus_block(
            &StimulusSpec::Cw {
                channel: 1,
                amp: 0.5,
                freq_hz: 100e3,
            },
            1e6,
            2,
            0,
            10,
            1.0,
        );
        assert!(blk[0].iter().all(|v| *v == 0.0) && (blk[1][1] - 0.5 * (0.2 * std::f64::consts::PI).sin()).abs() < 1e-12);
        std::fs::remove_dir_all(&d).ok();
    }
}
