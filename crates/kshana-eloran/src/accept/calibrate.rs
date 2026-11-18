// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Calibration wizards (A4/C3) as plain functions over traits, so a GUI or CLI can drive
//! them step by step and mocks can stand in for the bench.
//!
//! # 1. Simulator channel delay, gain and linearity — [`calibrate_channels`]
//!
//! 1. Emit the calibration pulse train ([`crate::hw::calib::calibration_pulse_train`]: one
//!    standard LORAN-C pulse per PPS, time origin on the PPS, standard zero crossing at
//!    PPS + 30 µs, COMDTINST M16562.4A) on every channel at each commanded level, through a
//!    [`LoopbackRig`]: the simulator's own receive path ([`DeviceLoopback`]) or an
//!    oscilloscope triggered on the PPS ([`ScopeLoopback`]).
//! 2. Per channel and level: the delay with
//!    [`crate::hw::calib::characterise_from_capture`] (matched filter + standard zero
//!    crossing), minus the known capture-path delay; the received amplitude by least
//!    squares against the analytic pulse at that delay.
//! 3. Gain = slope of received amplitude against commanded amplitude (least squares
//!    through the origin); linearity = the largest deviation of any level from that line
//!    (dB); amplitude-dependent delay = spread of the per-level delays.
//! 4. Write a [`CalibrationFile`] (delay at the highest level, its uncertainty, full-scale
//!    power from the gain and the capture units).
//! 5. Re-verify: emit the train through a [`CalibratedSink`] built from that file and
//!    measure again; the residual delay of every channel must be within the tolerance.
//!
//! # 2. Receiver timing calibration — [`calibrate_receiver`]
//!
//! Start the simulator on a scenario ([`SimulatorControl`]), arm the time-interval counter
//! on the PPS (start = reference PPS, stop = receiver 1PPS output), discard settling
//! readings, collect readings, subtract the simulated propagation delay (computed from the
//! scenario's truth record, [`propagation_delay_ns`]) where the receiver does not remove it
//! and the simulator channel delay (from the calibration file), and evaluate the GUM budget
//! with [`crate::hw::calib::receiver_delay`]. The result is a certificate-style JSON report
//! ([`ReceiverCalReport`]) with result ± U (k = 2), conditions and **traceability fields to
//! be filled by the laboratory**.
//!
//! # Evidence label
//!
//! **MODELLED**: verified against in-process loopback mocks with known delays, gains and
//! compression (this module's tests). The reports are **not accredited calibration
//! certificates**; real delays and uncertainties need the real bench (simulator,
//! cables, a time-interval counter or oscilloscope with their own calibrations).

use crate::hw::calib::{
    calibration_pulse_train, characterise_from_capture, receiver_delay, CalibratedSink,
    CalibrationFile, ChannelCal, CharacteriseOptions, Instrument, ReceiverDelayInputs,
    ReceiverDelayReport, ReferencePoint, UncertaintyComponent, CALIBRATION_VERSION,
};
use crate::hw::{Device, StartTime, TxSession};
use crate::instr::{TicSource, Waveform, WaveformSource};
use crate::scene::{MemSink, SampleSink};
use crate::TruthRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

// ---------------------------------------------------------------------------------------
// Rigs
// ---------------------------------------------------------------------------------------

/// Emits a sample stream on the simulator and captures the result.
pub trait LoopbackRig {
    fn name(&self) -> String;
    /// Emit `tx` (`tx[c][n]` at `fs_hz`, sample 0 on a PPS edge, PPS period `pps_period_s`)
    /// and return one capture per entry of `channels`. Each capture's `t0_s` is the time of
    /// its first sample relative to the PPS edge of `tx` sample 0, so the pulse emitted at
    /// PPS edge `k` sits at `k·period − t0_s` in the capture.
    fn emit_and_capture(
        &mut self,
        tx: &[Vec<f64>],
        fs_hz: f64,
        pps_period_s: f64,
        channels: &[usize],
    ) -> Result<Vec<Waveform>, String>;
}

/// Loopback through the simulator device's own receive channels (TX output cabled to an
/// RX input). Transmit and receive are started on the same PPS and run interleaved block
/// by block from one thread, which suits devices that buffer both directions; the
/// measured delay includes the receive path, so set
/// [`ChannelCalOptions::capture_path_delay_ns`] to its known delay. **UNTESTED ON
/// HARDWARE.** The device must already be configured with TX and RX channels.
pub struct DeviceLoopback<'a> {
    pub device: &'a mut dyn Device,
    /// Samples per block.
    pub block: usize,
    /// RX channel cabled to TX channel `i` (default: the same index).
    pub rx_channel_of: Vec<usize>,
}

impl LoopbackRig for DeviceLoopback<'_> {
    fn name(&self) -> String {
        format!("{} (RX loopback)", self.device.name())
    }
    fn emit_and_capture(
        &mut self,
        tx: &[Vec<f64>],
        _fs_hz: f64,
        _pps_period_s: f64,
        channels: &[usize],
    ) -> Result<Vec<Waveform>, String> {
        let n = tx.first().map(|c| c.len()).unwrap_or(0);
        let block = self.block.max(1);
        self.device.start_rx(StartTime::NextPps)?;
        self.device.start_tx(StartTime::NextPps)?;
        let mut rx: Vec<Vec<f64>> = Vec::new();
        let push = |r: Vec<Vec<f64>>, rx: &mut Vec<Vec<f64>>| {
            if rx.is_empty() {
                *rx = vec![Vec::new(); r.len()];
            }
            for (d, s) in rx.iter_mut().zip(r) {
                d.extend(s);
            }
        };
        let mut k = 0;
        while k < n {
            let e = (k + block).min(n);
            let b: Vec<Vec<f64>> = tx.iter().map(|c| c[k..e].to_vec()).collect();
            self.device.write_tx(&b)?;
            let r = self.device.read_rx(e - k)?;
            push(r, &mut rx);
            k = e;
        }
        self.device.stop_tx()?;
        for _ in 0..1000 {
            if rx.first().map(|c| c.len()).unwrap_or(0) >= n {
                break;
            }
            let r = self.device.read_rx(block)?;
            if r.is_empty() || r[0].is_empty() {
                break;
            }
            push(r, &mut rx);
        }
        self.device.stop_rx()?;
        let fs = self.device.sample_rate_hz().ok_or("device not configured")?;
        channels
            .iter()
            .map(|&c| {
                let rc = self.rx_channel_of.get(c).copied().unwrap_or(c);
                let mut s = rx
                    .get(rc)
                    .cloned()
                    .ok_or_else(|| format!("no RX channel {rc}"))?;
                s.truncate(n);
                Ok(Waveform {
                    fs_hz: fs,
                    t0_s: 0.0,
                    samples: s,
                    unit: "FS".into(),
                })
            })
            .collect()
    }
}

/// Producer that writes a fixed multichannel buffer in blocks.
fn write_buffer(sink: &mut dyn SampleSink, tx: &[Vec<f64>], block: usize) -> Result<(), String> {
    let n = tx.first().map(|c| c.len()).unwrap_or(0);
    let mut k = 0;
    while k < n {
        let e = (k + block).min(n);
        sink.write_block(&tx.iter().map(|c| c[k..e].to_vec()).collect::<Vec<_>>())?;
        k = e;
    }
    sink.finish()
}

/// Oscilloscope capture: for each channel in turn, the stream is started in a
/// [`TxSession`], the scope is armed after `arm_delay` (so it triggers on a PPS edge that
/// carries a pulse — the train has none on edge 0) and the waveform is fetched when the
/// stream has ended. The scope's record must contain the pulse after its trigger (a few
/// hundred µs). Since every pulse of the train is identical, the capture is reported
/// relative to edge 1. **UNTESTED ON HARDWARE.**
pub struct ScopeLoopback<'a> {
    /// The simulator device (configured for TX); taken out while streaming.
    pub device: Option<Box<dyn Device>>,
    pub scope: &'a mut dyn WaveformSource,
    pub block: usize,
    /// Wait between the stream start and arming the scope (≥ one PPS period on hardware).
    pub arm_delay: Duration,
}

impl LoopbackRig for ScopeLoopback<'_> {
    fn name(&self) -> String {
        format!("oscilloscope {}", self.scope.id())
    }
    fn emit_and_capture(
        &mut self,
        tx: &[Vec<f64>],
        _fs_hz: f64,
        pps_period_s: f64,
        channels: &[usize],
    ) -> Result<Vec<Waveform>, String> {
        let mut out = Vec::new();
        for &c in channels {
            let dev = self.device.take().ok_or("scope rig has no device")?;
            let buf = tx.to_vec();
            let block = self.block.max(1);
            let s = TxSession::start(dev, StartTime::NextPps, 8, move |sink| {
                write_buffer(sink, &buf, block)
            })?;
            std::thread::sleep(self.arm_delay);
            let armed = self.scope.arm(c);
            let rep = s.wait()?;
            self.device = Some(rep.device);
            armed?;
            let mut w = self.scope.fetch(c)?;
            // the trigger edge is edge 1 of the train: refer t0 to edge 0
            w.t0_s += pps_period_s;
            out.push(w);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------------------
// Channel calibration
// ---------------------------------------------------------------------------------------

/// Unit of the captured samples, for the full-scale power.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureUnits {
    /// Volts across `load_ohm` (oscilloscope with a terminated input).
    Volts { load_ohm: f64 },
    /// Arbitrary units whose sine amplitude 1.0 corresponds to `dbm_at_unit_amplitude`
    /// (e.g. a calibrated SDR receive path).
    Relative { dbm_at_unit_amplitude: f64 },
}

/// Options of [`calibrate_channels`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelCalOptions {
    /// Device identification written to the calibration file.
    pub device: String,
    /// Calibration date (ISO 8601).
    pub date: String,
    pub n_channels: usize,
    pub fs_hz: f64,
    /// PPS period of the train (1 s on hardware; shorter periods only on loopback mocks).
    pub pps_period_s: f64,
    /// Pulses per level (on PPS edges 1..=n; edge 0 is left empty for the calibrated
    /// sink's look-ahead).
    pub n_pulses: usize,
    /// Commanded pulse amplitudes (fraction of full scale), at least two.
    pub levels: Vec<f64>,
    pub search: CharacteriseOptions,
    /// Known delay of the capture path per channel (ns), subtracted (missing = 0).
    pub capture_path_delay_ns: Vec<f64>,
    /// Standard uncertainty of the capture-path delay (ns).
    pub capture_path_u_ns: f64,
    /// Further Type B standard uncertainty of the delay (trigger level, reference
    /// distribution, cable), ns.
    pub type_b_u_ns: f64,
    pub units: CaptureUnits,
    /// Residual delay tolerance of the re-verification (ns).
    pub verify_tol_ns: f64,
    #[serde(default)]
    pub instruments: Vec<Instrument>,
    #[serde(default)]
    pub reference_point: ReferencePoint,
    #[serde(default)]
    pub notes: String,
}

impl Default for ChannelCalOptions {
    fn default() -> Self {
        ChannelCalOptions {
            device: "simulator".into(),
            date: crate::instr::log::utc_today(),
            n_channels: 1,
            fs_hz: 1e6,
            pps_period_s: 1.0,
            n_pulses: 4,
            levels: vec![0.1, 0.3, 0.6, 0.9],
            search: CharacteriseOptions::default(),
            capture_path_delay_ns: vec![],
            capture_path_u_ns: 0.0,
            type_b_u_ns: 0.5,
            units: CaptureUnits::Volts { load_ohm: 50.0 },
            verify_tol_ns: 1.0,
            instruments: vec![],
            reference_point: ReferencePoint::default(),
            notes: String::new(),
        }
    }
}

/// One level of one channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelPoint {
    pub commanded_fs: f64,
    pub delay_ns: f64,
    pub delay_std_ns: f64,
    pub delay_u_ns: f64,
    /// Received sine amplitude (capture units).
    pub received_amplitude: f64,
    /// Deviation from the fitted linear gain (dB).
    pub deviation_db: f64,
}

/// Per-channel details.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelCalDetail {
    pub channel: usize,
    pub levels: Vec<LevelPoint>,
    /// Received amplitude per unit of full scale (capture units / FS).
    pub gain: f64,
    pub gain_db: f64,
    pub linearity_max_dev_db: f64,
    /// max − min of the per-level delays (ns).
    pub delay_spread_ns: f64,
}

/// Re-verification result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalVerification {
    pub residual_ns: Vec<f64>,
    pub residual_u_ns: Vec<f64>,
    pub tol_ns: f64,
    pub pass: bool,
}

/// Result of [`calibrate_channels`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelCalReport {
    pub rig: String,
    pub calibration: CalibrationFile,
    pub channels: Vec<ChannelCalDetail>,
    pub verification: CalVerification,
}

fn train(o: &ChannelCalOptions, amp: f64) -> (Vec<f64>, Vec<f64>) {
    let n = ((o.n_pulses as f64 + 1.0) * o.pps_period_s * o.fs_hz).round() as usize;
    let pps: Vec<f64> = (1..=o.n_pulses)
        .map(|k| k as f64 * o.pps_period_s)
        .collect();
    (calibration_pulse_train(o.fs_hz, n, &pps, 0.0, amp), pps)
}

/// Pulse instants (s from the first sample) of edges 1..=n inside a capture.
fn capture_pps(w: &Waveform, o: &ChannelCalOptions) -> Vec<f64> {
    let len = w.samples.len() as f64 / w.fs_hz;
    (1..=o.n_pulses)
        .map(|k| k as f64 * o.pps_period_s - w.t0_s)
        .filter(|t| *t >= 0.0 && *t < len)
        .collect()
}

/// Least-squares amplitude of the analytic pulse train at `delay_ns` in `x`.
pub fn fit_amplitude(x: &[f64], fs_hz: f64, pps_s: &[f64], delay_ns: f64) -> f64 {
    let t = calibration_pulse_train(fs_hz, x.len(), pps_s, delay_ns, 1.0);
    let tt: f64 = t.iter().map(|v| v * v).sum();
    if tt == 0.0 {
        return 0.0;
    }
    t.iter().zip(x).map(|(a, b)| a * b).sum::<f64>() / tt
}

fn measure(
    w: &Waveform,
    o: &ChannelCalOptions,
    path_ns: f64,
) -> Result<(crate::hw::calib::DelayEstimate, f64), String> {
    let pps = capture_pps(w, o);
    let mut search = o.search.clone();
    search.expected_delay_ns += path_ns;
    let e = characterise_from_capture(&w.samples, w.fs_hz, &pps, &search)?;
    let a = fit_amplitude(&w.samples, w.fs_hz, &pps, e.delay_ns);
    Ok((e, a))
}

/// Run the channel delay/gain/linearity calibration and its re-verification.
pub fn calibrate_channels(
    rig: &mut dyn LoopbackRig,
    o: &ChannelCalOptions,
) -> Result<ChannelCalReport, String> {
    if o.levels.len() < 2 || o.levels.iter().any(|l| !(*l > 0.0 && *l <= 1.0)) {
        return Err("need at least two levels in (0, 1]".into());
    }
    if o.n_channels == 0 || o.n_pulses == 0 {
        return Err("need at least one channel and one pulse".into());
    }
    let chans: Vec<usize> = (0..o.n_channels).collect();
    let path = |c: usize| o.capture_path_delay_ns.get(c).copied().unwrap_or(0.0);
    let mut per: Vec<Vec<LevelPoint>> = vec![Vec::new(); o.n_channels];
    for &amp in &o.levels {
        let (x, _) = train(o, amp);
        let tx = vec![x; o.n_channels];
        let caps = rig.emit_and_capture(&tx, o.fs_hz, o.pps_period_s, &chans)?;
        for (c, w) in caps.iter().enumerate() {
            let (e, a) = measure(w, o, path(c)).map_err(|err| format!("channel {c}: {err}"))?;
            per[c].push(LevelPoint {
                commanded_fs: amp,
                delay_ns: e.delay_ns - path(c),
                delay_std_ns: e.std_ns,
                delay_u_ns: e.u_ns,
                received_amplitude: a,
                deviation_db: 0.0,
            });
        }
    }
    let mut details = Vec::new();
    let mut cals = Vec::new();
    for (c, mut lv) in per.into_iter().enumerate() {
        let sxy: f64 = lv.iter().map(|p| p.commanded_fs * p.received_amplitude).sum();
        let sxx: f64 = lv.iter().map(|p| p.commanded_fs.powi(2)).sum();
        let gain = sxy / sxx;
        if gain.is_nan() || gain <= 0.0 {
            return Err(format!("channel {c}: no signal received (gain {gain})"));
        }
        for p in lv.iter_mut() {
            p.deviation_db = 20.0 * (p.received_amplitude / (gain * p.commanded_fs)).abs().log10();
        }
        let lin = lv.iter().map(|p| p.deviation_db.abs()).fold(0.0, f64::max);
        let dmax = lv.iter().map(|p| p.delay_ns).fold(f64::MIN, f64::max);
        let dmin = lv.iter().map(|p| p.delay_ns).fold(f64::MAX, f64::min);
        let top = lv
            .iter()
            .max_by(|a, b| a.commanded_fs.total_cmp(&b.commanded_fs))
            .expect("levels");
        let full_scale_dbm = match &o.units {
            CaptureUnits::Volts { load_ohm } => {
                10.0 * ((gain * gain / 2.0) / load_ohm * 1e3).log10()
            }
            CaptureUnits::Relative {
                dbm_at_unit_amplitude,
            } => dbm_at_unit_amplitude + 20.0 * gain.log10(),
        };
        cals.push(ChannelCal {
            channel: c,
            delay_ns: top.delay_ns,
            delay_u_ns: (top.delay_u_ns.powi(2) + o.capture_path_u_ns.powi(2) + o.type_b_u_ns.powi(2))
                .sqrt(),
            full_scale_dbm,
            full_scale_u_db: lin,
            load_ohm: match &o.units {
                CaptureUnits::Volts { load_ohm } => Some(*load_ohm),
                _ => None,
            },
            notes: format!(
                "delay at {:.3} FS; amplitude-dependent delay spread {:.3} ns; linearity {:.3} dB",
                top.commanded_fs,
                dmax - dmin,
                lin
            ),
        });
        details.push(ChannelCalDetail {
            channel: c,
            levels: lv,
            gain,
            gain_db: 20.0 * gain.log10(),
            linearity_max_dev_db: lin,
            delay_spread_ns: dmax - dmin,
        });
    }
    let cal = CalibrationFile {
        version: CALIBRATION_VERSION,
        date: o.date.clone(),
        device: o.device.clone(),
        sample_rate_hz: o.fs_hz,
        reference_point: o.reference_point.clone(),
        instruments: o.instruments.clone(),
        channels: cals,
        gnss_offset: None,
        notes: format!(
            "kshana-eloran calibrate_channels via {}; levels {:?} FS; {}",
            rig.name(),
            o.levels,
            o.notes
        ),
    };
    cal.validate()?;

    // re-verify through the calibrated sink at the highest level
    let top = o.levels.iter().cloned().fold(0.0, f64::max);
    let (x, _) = train(o, top);
    let mut cs = CalibratedSink::from_calibration(MemSink::default(), o.fs_hz, &cal, None, 0.0)?;
    let block = (0.01 * o.fs_hz) as usize;
    for k in (0..x.len()).step_by(block.max(1)) {
        let e = (k + block).min(x.len());
        cs.write_block(&vec![x[k..e].to_vec(); o.n_channels])?;
    }
    cs.finish()?;
    let tx = cs.into_inner().channels;
    let caps = rig.emit_and_capture(&tx, o.fs_hz, o.pps_period_s, &chans)?;
    let mut res = Vec::new();
    let mut res_u = Vec::new();
    for (c, w) in caps.iter().enumerate() {
        let mut opt = o.clone();
        opt.search.expected_delay_ns = 0.0;
        let (e, _) = measure(w, &opt, path(c)).map_err(|err| format!("verify channel {c}: {err}"))?;
        res.push(e.delay_ns - path(c));
        res_u.push(e.u_ns);
    }
    let pass = res.iter().all(|r| r.abs() <= o.verify_tol_ns);
    Ok(ChannelCalReport {
        rig: rig.name(),
        calibration: cal,
        channels: details,
        verification: CalVerification {
            residual_ns: res,
            residual_u_ns: res_u,
            tol_ns: o.verify_tol_ns,
            pass,
        },
    })
}

// ---------------------------------------------------------------------------------------
// Receiver timing calibration
// ---------------------------------------------------------------------------------------

/// Simulator control used by the receiver calibration.
pub trait SimulatorControl {
    fn id(&self) -> String;
    /// Start generating the scenario (TOML).
    fn start(&mut self, scenario_toml: &str) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}

struct DynSink<'a>(&'a mut dyn SampleSink);

impl SampleSink for DynSink<'_> {
    fn write_block(&mut self, c: &[Vec<f64>]) -> Result<(), String> {
        self.0.write_block(c)
    }
    fn finish(&mut self) -> Result<(), String> {
        self.0.finish()
    }
}

/// [`SimulatorControl`] streaming scenes to a [`Device`] (configured beforehand) through
/// an optional [`CalibratedSink`]. **UNTESTED ON HARDWARE.**
pub struct DeviceSimulator {
    device: Option<Box<dyn Device>>,
    session: Option<TxSession<()>>,
    pub calibration: Option<CalibrationFile>,
    pub fifo_blocks: usize,
}

impl DeviceSimulator {
    pub fn new(device: Box<dyn Device>, calibration: Option<CalibrationFile>) -> DeviceSimulator {
        DeviceSimulator {
            device: Some(device),
            session: None,
            calibration,
            fifo_blocks: 8,
        }
    }
    /// The device back (after [`SimulatorControl::stop`]).
    pub fn into_device(self) -> Option<Box<dyn Device>> {
        self.device
    }
}

impl SimulatorControl for DeviceSimulator {
    fn id(&self) -> String {
        self.device
            .as_ref()
            .map(|d| d.name())
            .unwrap_or_else(|| "device (streaming)".into())
    }
    fn start(&mut self, scenario_toml: &str) -> Result<(), String> {
        if self.session.is_some() {
            return Err("simulator already running".into());
        }
        let sc = crate::Scenario::from_toml(scenario_toml)?;
        let mut scene = sc.build()?;
        let fs = sc.scene.params.fs_hz;
        let dev = self.device.take().ok_or("no device")?;
        let cal = self.calibration.clone();
        let s = TxSession::start(dev, StartTime::NextPps, self.fifo_blocks, move |sink| {
            let mut truth = Vec::new();
            let r = match &cal {
                Some(cal) => {
                    let mut cs = CalibratedSink::from_calibration(DynSink(sink), fs, cal, None, 0.0)?;
                    scene.run(&mut cs, &mut truth)
                }
                None => scene.run(sink, &mut truth),
            };
            match r {
                Ok(_) => Ok(()),
                Err(e) if e.contains("stopped") => Ok(()),
                Err(e) => Err(e),
            }
        })?;
        self.session = Some(s);
        Ok(())
    }
    fn stop(&mut self) -> Result<(), String> {
        let Some(s) = self.session.take() else {
            return Ok(());
        };
        s.stop();
        let rep = s.wait()?;
        self.device = Some(rep.device);
        Ok(())
    }
}

/// The groundwave propagation delay (ns) of a station in a scenario, from the scene's own
/// truth record: the scenario is run to memory for just over one GRI (its duration is
/// shortened). `station` = `(chain, station id)`; `None` takes the first emission.
pub fn propagation_delay_ns(
    scenario_toml: &str,
    station: Option<(&str, &str)>,
) -> Result<(f64, TruthRecord), String> {
    let mut sc = crate::Scenario::from_toml(scenario_toml)?;
    let gri = sc
        .chains
        .iter()
        .map(|c| c.chain.gri_us)
        .max()
        .ok_or("scenario has no chain")? as f64
        * 1e-6;
    sc.scene.params.duration_s = sc.scene.params.duration_s.min(2.0 * gri + 0.02);
    sc.interference.clear();
    let mut scene = sc.build()?;
    let mut truth = Vec::new();
    scene.run(&mut MemSink::default(), &mut truth)?;
    let r = truth
        .into_iter()
        .find(|t| station.is_none_or(|(c, s)| t.chain == c && t.station == s))
        .ok_or("station not found in the scenario truth")?;
    Ok((r.delay_ns, r))
}

/// Receiver identification.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReceiverIdentity {
    pub id: String,
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    pub firmware: String,
    /// Antenna input used (E-field / H-field) and the 1PPS output measured.
    pub interface: String,
}

/// Environmental and set-up conditions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Conditions {
    pub date: String,
    pub location: String,
    pub temperature_c: Option<f64>,
    pub humidity_pct: Option<f64>,
    /// Common time reference (e.g. "UTC(k) 1PPS via distribution amplifier").
    pub reference: String,
    pub warm_up: String,
    pub notes: String,
}

/// One link of the traceability chain; fields the laboratory must fill are "TO BE FILLED".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceabilityLink {
    pub item: String,
    pub identification: String,
    pub calibrated_by: String,
    pub certificate_no: String,
    pub calibration_date: String,
    pub due_date: String,
}

impl TraceabilityLink {
    pub fn to_fill(item: &str) -> TraceabilityLink {
        let t = "TO BE FILLED".to_string();
        TraceabilityLink {
            item: item.into(),
            identification: t.clone(),
            calibrated_by: t.clone(),
            certificate_no: t.clone(),
            calibration_date: t.clone(),
            due_date: t,
        }
    }
}

/// The default traceability chain (to fill in).
pub fn default_traceability() -> Vec<TraceabilityLink> {
    [
        "National time scale UTC(k) / national metrology institute",
        "Laboratory time reference (1PPS, 10 MHz)",
        "Time-interval counter",
        "Simulator channel calibration (calibration file)",
        "Cables and distribution amplifier",
    ]
    .iter()
    .map(|s| TraceabilityLink::to_fill(s))
    .collect()
}

/// Whether the receiver removes the simulated propagation delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropagationHandling {
    /// Timing receiver configured with the simulated position (and ASF): nothing to
    /// subtract.
    RemovedByReceiver,
    /// Raw time of arrival: subtract the simulated propagation delay.
    Subtract,
}

/// Set-up of a receiver calibration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceiverCalSetup {
    pub report_id: String,
    pub operator: String,
    pub receiver: ReceiverIdentity,
    pub conditions: Conditions,
    /// Scenario run by the simulator (TOML). Time-reference offsets should be zero so the
    /// emission is on the reference PPS.
    pub scenario_toml: String,
    /// Station whose delay is used: `(chain, station)`; `None` = first emission.
    pub station: Option<(String, String)>,
    pub propagation: PropagationHandling,
    /// Standard uncertainty of the simulated propagation delay (ns).
    pub propagation_u_ns: f64,
    /// Simulator calibration and channel used.
    pub calibration: Option<CalibrationFile>,
    pub channel: usize,
    /// True when the simulator output already compensates the channel delay (calibrated
    /// sink): the residual is then 0 with the calibration's uncertainty.
    pub calibration_applied: bool,
    pub tic_offset_ns: f64,
    pub type_b: Vec<UncertaintyComponent>,
    pub k: f64,
    pub n_readings: usize,
    /// Readings discarded first (receiver settling).
    pub settle_readings: usize,
    pub traceability: Vec<TraceabilityLink>,
    /// Evidence files (measurement logs, captures).
    pub evidence: Vec<String>,
}

/// Certificate-style receiver calibration report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReceiverCalReport {
    pub document_type: String,
    pub report_id: String,
    pub operator: String,
    pub receiver: ReceiverIdentity,
    pub simulator: String,
    pub tic: String,
    pub conditions: Conditions,
    pub method: String,
    pub scenario_sha256: String,
    pub calibration_file_sha256: Option<String>,
    pub propagation: PropagationHandling,
    pub propagation_delay_ns: f64,
    pub channel_delay_ns: f64,
    pub readings_ns: Vec<f64>,
    pub budget: ReceiverDelayReport,
    /// "d_rx = X ns ± U ns (k = 2)".
    pub result_statement: String,
    pub receiver_delay_ns: f64,
    pub expanded_u_ns: f64,
    pub k: f64,
    pub traceability: Vec<TraceabilityLink>,
    pub evidence: Vec<String>,
    pub disclaimer: String,
    /// SHA-256 of this report's JSON with this field empty.
    pub report_sha256: String,
}

impl ReceiverCalReport {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    /// Recompute the self-hash (with `report_sha256` empty).
    pub fn compute_hash(&self) -> String {
        let mut c = self.clone();
        c.report_sha256 = String::new();
        hex::encode(Sha256::digest(
            serde_json::to_string(&c).expect("serialisable").as_bytes(),
        ))
    }
}

/// Run the receiver timing calibration.
pub fn calibrate_receiver(
    sim: &mut dyn SimulatorControl,
    tic: &mut dyn TicSource,
    s: &ReceiverCalSetup,
) -> Result<ReceiverCalReport, String> {
    if s.n_readings < 2 {
        return Err("need at least two readings".into());
    }
    let st = s.station.as_ref().map(|(c, x)| (c.as_str(), x.as_str()));
    let (prop_ns, _) = propagation_delay_ns(&s.scenario_toml, st)?;
    let scenario_sha256 = crate::Scenario::from_toml(&s.scenario_toml)?.hash();
    let (ch_delay, ch_u) = match &s.calibration {
        Some(cal) => {
            let c = cal
                .channels
                .get(s.channel)
                .ok_or_else(|| format!("calibration has no channel {}", s.channel))?;
            (
                if s.calibration_applied { 0.0 } else { c.delay_ns },
                c.delay_u_ns,
            )
        }
        None => (0.0, 0.0),
    };
    sim.start(&s.scenario_toml)?;
    let run = (|| {
        tic.arm_pps()?;
        if s.settle_readings > 0 {
            tic.read_n(s.settle_readings)?;
        }
        tic.read_n(s.n_readings)
    })();
    let stopped = sim.stop();
    let readings = run?;
    stopped?;
    let (p, pu) = match s.propagation {
        PropagationHandling::Subtract => (prop_ns, s.propagation_u_ns),
        PropagationHandling::RemovedByReceiver => (0.0, s.propagation_u_ns),
    };
    let budget = receiver_delay(
        &readings,
        &ReceiverDelayInputs {
            channel_delay_ns: ch_delay,
            channel_delay_u_ns: ch_u,
            propagation_delay_ns: p,
            propagation_delay_u_ns: pu,
            tic_offset_ns: s.tic_offset_ns,
            type_b: s.type_b.clone(),
            k: s.k,
        },
    )?;
    let mut rep = ReceiverCalReport {
        document_type: "eLoran receiver timing calibration report (certificate style; not an \
                        accredited calibration certificate)"
            .into(),
        report_id: s.report_id.clone(),
        operator: s.operator.clone(),
        receiver: s.receiver.clone(),
        simulator: sim.id(),
        tic: tic.id(),
        conditions: s.conditions.clone(),
        method: "TIC start = reference 1PPS, stop = receiver 1PPS; d_rx = mean(TIC) − d_channel − \
                 d_prop − d_tic; GUM (JCGM 100:2008) budget with autocorrelation-aware Type A"
            .into(),
        scenario_sha256,
        calibration_file_sha256: s
            .calibration
            .as_ref()
            .map(|c| hex::encode(Sha256::digest(c.to_json().as_bytes()))),
        propagation: s.propagation,
        propagation_delay_ns: prop_ns,
        channel_delay_ns: ch_delay,
        readings_ns: readings,
        result_statement: format!(
            "d_rx = {:.2} ns ± {:.2} ns (k = {})",
            budget.receiver_delay_ns, budget.expanded_u_ns, budget.k
        ),
        receiver_delay_ns: budget.receiver_delay_ns,
        expanded_u_ns: budget.expanded_u_ns,
        k: budget.k,
        budget,
        traceability: s.traceability.clone(),
        evidence: s.evidence.clone(),
        disclaimer: "Values depend on the bench, the instruments and their calibrations; the \
                     traceability chain must be completed by the laboratory."
            .into(),
        report_sha256: String::new(),
    };
    rep.report_sha256 = rep.compute_hash();
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw::calib::{DELAY_FIR_BETA, DELAY_FIR_HALF};
    use crate::hw::fir::{fractional_delay_taps, FirState};
    use crate::hw::{DeviceConfig, DeviceStatus};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Loopback device: each TX channel comes back on the same RX channel delayed by
    /// `delay_ns`, scaled by `gain` and softly compressed (`tanh`).
    struct LoopDev {
        fs: f64,
        delay_ns: Vec<f64>,
        gain: Vec<f64>,
        compress: f64,
        firs: Vec<FirState>,
        rx: Vec<VecDeque<f64>>,
        record: Arc<Mutex<Vec<Vec<f64>>>>,
        ref_locked: bool,
    }

    impl LoopDev {
        fn new(fs: f64, delay_ns: Vec<f64>, gain: Vec<f64>, compress: f64) -> LoopDev {
            let n = delay_ns.len();
            LoopDev {
                fs,
                delay_ns,
                gain,
                compress,
                firs: vec![],
                rx: vec![VecDeque::new(); n],
                record: Arc::new(Mutex::new(vec![Vec::new(); n])),
                ref_locked: true,
            }
        }
        fn reset(&mut self) {
            self.firs = self
                .delay_ns
                .iter()
                .map(|d| {
                    // a causal FIR needs ≥ DELAY_FIR_HALF samples of bulk latency: the
                    // capture path delay of this mock (PATH_NS)
                    let s = d * 1e-9 * self.fs + DELAY_FIR_HALF as f64;
                    let i = s.floor();
                    FirState::new(
                        fractional_delay_taps(s - i, DELAY_FIR_HALF, DELAY_FIR_BETA),
                        i as usize - DELAY_FIR_HALF,
                    )
                })
                .collect();
            for q in self.rx.iter_mut() {
                q.clear();
            }
            for r in self.record.lock().unwrap().iter_mut() {
                r.clear();
            }
        }
    }

    impl Device for LoopDev {
        fn name(&self) -> String {
            "loopdev".into()
        }
        fn realtime(&self) -> bool {
            false
        }
        fn configure(&mut self, _cfg: &DeviceConfig) -> Result<(), String> {
            Ok(())
        }
        fn sample_rate_hz(&self) -> Option<f64> {
            Some(self.fs)
        }
        fn start_tx(&mut self, _at: StartTime) -> Result<Option<i64>, String> {
            self.reset();
            Ok(Some(0))
        }
        fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
            for (c, x) in block.iter().enumerate() {
                let y = self.firs[c].process(x);
                let k = self.compress;
                let y: Vec<f64> = y
                    .iter()
                    .map(|v| self.gain[c] * if k > 0.0 { (k * v).tanh() / k } else { *v })
                    .collect();
                self.rx[c].extend(y.iter());
                self.record.lock().unwrap()[c].extend(y);
            }
            Ok(())
        }
        fn stop_tx(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn start_rx(&mut self, _at: StartTime) -> Result<Option<i64>, String> {
            Ok(Some(0))
        }
        fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
            Ok(self
                .rx
                .iter_mut()
                .map(|q| {
                    let m = n.min(q.len());
                    q.drain(..m).collect()
                })
                .collect())
        }
        fn stop_rx(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn status(&self) -> DeviceStatus {
            DeviceStatus {
                ref_locked: Some(self.ref_locked),
                ..DeviceStatus::default()
            }
        }
    }

    /// Bulk latency of the mock loopback (32 samples at 1 MS/s).
    const PATH_NS: f64 = 32_000.0;

    fn opts(n: usize) -> ChannelCalOptions {
        ChannelCalOptions {
            device: "loopdev".into(),
            date: "2026-11-18".into(),
            n_channels: n,
            fs_hz: 1e6,
            pps_period_s: 2e-3,
            n_pulses: 3,
            levels: vec![0.1, 0.4, 0.8],
            search: CharacteriseOptions {
                expected_delay_ns: 0.0,
                search_ns: 3000.0,
            },
            capture_path_delay_ns: vec![PATH_NS; n],
            capture_path_u_ns: 0.0,
            type_b_u_ns: 0.2,
            units: CaptureUnits::Volts { load_ohm: 50.0 },
            verify_tol_ns: 0.2,
            ..ChannelCalOptions::default()
        }
    }

    #[test]
    fn channel_wizard_recovers_delay_gain_and_compression_and_verifies() {
        let mut dev = LoopDev::new(1e6, vec![812.37, 1250.9], vec![0.5, 0.25], 0.0);
        let mut rig = DeviceLoopback {
            device: &mut dev,
            block: 4096,
            rx_channel_of: vec![],
        };
        let r = calibrate_channels(&mut rig, &opts(2)).unwrap();
        let c = &r.calibration.channels;
        assert!((c[0].delay_ns - 812.37).abs() < 0.1, "{}", c[0].delay_ns);
        assert!((c[1].delay_ns - 1250.9).abs() < 0.1, "{}", c[1].delay_ns);
        // 0.5 V amplitude into 50 Ω = 2.5 mW = 3.98 dBm
        assert!((c[0].full_scale_dbm - 10.0 * (2.5f64).log10()).abs() < 0.01);
        assert!((r.channels[1].gain - 0.25).abs() < 1e-3);
        assert!(r.channels[0].linearity_max_dev_db < 0.01);
        assert!(r.verification.pass, "{:?}", r.verification);
        assert!(c[0].delay_u_ns >= 0.2);
        // the file is a valid calibration file
        CalibrationFile::from_json(&r.calibration.to_json()).unwrap();

        // compression shows up as non-linearity, a pass/fail the GUI can show
        let mut dev = LoopDev::new(1e6, vec![300.0], vec![1.0], 1.5);
        let mut rig = DeviceLoopback {
            device: &mut dev,
            block: 4096,
            rx_channel_of: vec![],
        };
        let r = calibrate_channels(&mut rig, &opts(1)).unwrap();
        assert!(r.channels[0].linearity_max_dev_db > 0.5, "{:?}", r.channels[0]);
        let lv = &r.channels[0].levels;
        assert!(lv[2].deviation_db < lv[0].deviation_db);
    }

    /// Mock scope reading the loop device's recorded output around PPS edge 1.
    struct MockScope {
        record: Arc<Mutex<Vec<Vec<f64>>>>,
        fs: f64,
        period: f64,
        pre_s: f64,
        len_s: f64,
    }

    impl WaveformSource for MockScope {
        fn id(&self) -> String {
            "mockscope".into()
        }
        fn arm(&mut self, _channel: usize) -> Result<(), String> {
            Ok(())
        }
        fn fetch(&mut self, channel: usize) -> Result<Waveform, String> {
            let r = self.record.lock().unwrap();
            let a = ((self.period - self.pre_s) * self.fs).round() as usize;
            let b = a + (self.len_s * self.fs) as usize;
            Ok(Waveform {
                fs_hz: self.fs,
                t0_s: -self.pre_s,
                samples: r[channel][a..b.min(r[channel].len())].to_vec(),
                unit: "V".into(),
            })
        }
    }

    #[test]
    fn scope_rig_measures_the_same_delay() {
        let dev = LoopDev::new(1e6, vec![640.25], vec![0.8], 0.0);
        let record = dev.record.clone();
        let mut scope = MockScope {
            record,
            fs: 1e6,
            period: 2e-3,
            pre_s: 100e-6,
            len_s: 1.2e-3,
        };
        let mut rig = ScopeLoopback {
            device: Some(Box::new(dev)),
            scope: &mut scope,
            block: 1000,
            arm_delay: Duration::ZERO,
        };
        let mut o = opts(1);
        o.levels = vec![0.5, 1.0];
        let r = calibrate_channels(&mut rig, &o).unwrap();
        assert!((r.calibration.channels[0].delay_ns - 640.25).abs() < 0.1);
        assert!(r.verification.pass, "{:?}", r.verification);
        assert!(r.rig.contains("mockscope"));
    }

    const SCEN: &str = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.1
fs_hz = 1000000
seed = 1
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 0.0
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 40000
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 0.0 }
"#;

    /// TIC mock: receiver 1PPS = reference + channel delay + propagation + receiver delay,
    /// with deterministic ±1 ns alternation.
    struct MockTic {
        base_ns: f64,
        k: usize,
        armed: bool,
    }

    impl TicSource for MockTic {
        fn id(&self) -> String {
            "mocktic".into()
        }
        fn arm_pps(&mut self) -> Result<(), String> {
            self.armed = true;
            Ok(())
        }
        fn read_n(&mut self, n: usize) -> Result<Vec<f64>, String> {
            assert!(self.armed);
            Ok((0..n)
                .map(|_| {
                    self.k += 1;
                    self.base_ns + if self.k % 2 == 0 { 1.0 } else { -1.0 }
                })
                .collect())
        }
    }

    #[test]
    fn receiver_wizard_subtracts_propagation_and_channel_delay() {
        let (prop, rec) = propagation_delay_ns(SCEN, Some(("c", "M"))).unwrap();
        assert!((rec.distance_km - 111.3).abs() < 0.5);
        assert!(prop > 370_000.0 && prop < 372_000.0, "{prop}");
        let cal = {
            let mut dev = LoopDev::new(1e6, vec![500.0], vec![0.5], 0.0);
            let mut rig = DeviceLoopback {
                device: &mut dev,
                block: 4096,
                rx_channel_of: vec![],
            };
            calibrate_channels(&mut rig, &opts(1)).unwrap().calibration
        };
        let rx_delay = 1234.5;
        let mut tic = MockTic {
            base_ns: cal.channels[0].delay_ns + prop + rx_delay,
            k: 0,
            armed: false,
        };
        let mut sim = DeviceSimulator::new(
            Box::new(LoopDev::new(1e6, vec![0.0], vec![1.0], 0.0)),
            None,
        );
        let setup = ReceiverCalSetup {
            report_id: "RC-0001".into(),
            operator: "op".into(),
            receiver: ReceiverIdentity {
                id: "RX-1".into(),
                ..Default::default()
            },
            conditions: Conditions {
                date: "2026-11-18".into(),
                temperature_c: Some(23.0),
                ..Default::default()
            },
            scenario_toml: SCEN.into(),
            station: Some(("c".into(), "M".into())),
            propagation: PropagationHandling::Subtract,
            propagation_u_ns: 1.0,
            calibration: Some(cal.clone()),
            channel: 0,
            calibration_applied: false,
            tic_offset_ns: 0.0,
            type_b: vec![UncertaintyComponent::normal("TIC resolution", 0.1)],
            k: 2.0,
            n_readings: 100,
            settle_readings: 5,
            traceability: default_traceability(),
            evidence: vec!["tic.jsonl".into()],
        };
        let rep = calibrate_receiver(&mut sim, &mut tic, &setup).unwrap();
        assert!((rep.receiver_delay_ns - rx_delay).abs() < 1e-6, "{}", rep.receiver_delay_ns);
        assert!(rep.expanded_u_ns > 2.0 && rep.expanded_u_ns < 3.0, "{}", rep.expanded_u_ns);
        assert_eq!(rep.readings_ns.len(), 100);
        assert!(rep.result_statement.contains("k = 2"));
        assert_eq!(rep.report_sha256, rep.compute_hash());
        let j = rep.to_json();
        assert!(j.contains("TO BE FILLED") && j.contains("not an accredited"));
        // the device came back from the streaming session
        assert!(sim.into_device().is_some());

        // receiver removing propagation and a calibrated output: nothing subtracted
        let mut tic = MockTic {
            base_ns: rx_delay,
            k: 0,
            armed: false,
        };
        let mut sim = DeviceSimulator::new(
            Box::new(LoopDev::new(1e6, vec![0.0], vec![1.0], 0.0)),
            Some(cal),
        );
        let s2 = ReceiverCalSetup {
            propagation: PropagationHandling::RemovedByReceiver,
            calibration_applied: true,
            ..setup
        };
        let rep = calibrate_receiver(&mut sim, &mut tic, &s2).unwrap();
        assert!((rep.receiver_delay_ns - rx_delay).abs() < 1e-6);
    }
}
