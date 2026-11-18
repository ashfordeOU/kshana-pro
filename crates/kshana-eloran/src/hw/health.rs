// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Hardware health and status aggregation with alarm thresholds and a JSON snapshot.
//!
//! * [`MonitoredDevice`] wraps any [`Device`] and publishes its status, an output-level
//!   estimate measured from the transmitted stream and optional sensor readings
//!   (temperature, PPS offset) through a shared [`HealthProbe`] — so the health of a device
//!   that has been moved into a [`super::TxSession`] pump thread can still be read live.
//! * [`HealthMonitor`] turns a probe (or a [`DeviceStatus`] directly) plus the
//!   [`SessionStatus`] of the transmit session into a [`HealthSnapshot`]: reference lock,
//!   underruns (device and FIFO), receive overflows, late bursts, temperature, output RMS and
//!   peak level (dBFS), clipping, PPS alignment — each checked against [`HealthThresholds`]
//!   and raised as an [`Alarm`] with a [`Severity`]. Counter alarms fire on the *increase*
//!   since the previous snapshot, so a past event does not alarm forever; the totals stay
//!   in the snapshot.
//!
//! Level convention: dBFS is `20·log10(x)` with 1.0 = device full scale (a full-scale sine
//! therefore has an RMS of −3.01 dBFS).
//!
//! Evidence: **MODELLED** — internal bookkeeping checked with the virtual bench
//! ([`super::virtual_bench`]); the thresholds are configuration, not measured limits.

use super::{Device, DeviceConfig, DeviceStatus, SessionStatus, StartTime};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Alarm severity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Ok,
    Warning,
    Alarm,
}

/// One raised alarm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alarm {
    /// Stable machine-readable code, e.g. `ref_unlocked`, `underrun`.
    pub code: String,
    pub severity: Severity,
    pub message: String,
}

/// Alarm thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HealthThresholds {
    /// Alarm when the device reports an unlocked reference.
    pub require_ref_lock: bool,
    /// Device + FIFO underruns allowed per snapshot interval before an alarm.
    pub max_underruns: u64,
    /// Receive overflows allowed per interval.
    pub max_overflows: u64,
    /// Late bursts / time errors allowed per interval.
    pub max_late: u64,
    pub temp_warn_c: f64,
    pub temp_alarm_c: f64,
    /// Warn when the output RMS level is below this (dBFS) — output missing or attenuated.
    pub min_output_rms_dbfs: f64,
    /// Warn when the output peak exceeds this (dBFS) — close to clipping.
    pub max_output_peak_dbfs: f64,
    /// Alarm when the measured PPS alignment error exceeds this (ns).
    pub max_pps_offset_ns: f64,
    /// Warn when a stream was not started on a PPS edge.
    pub require_pps_aligned: bool,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        HealthThresholds {
            require_ref_lock: true,
            max_underruns: 0,
            max_overflows: 0,
            max_late: 0,
            temp_warn_c: 70.0,
            temp_alarm_c: 85.0,
            min_output_rms_dbfs: -60.0,
            max_output_peak_dbfs: -0.1,
            max_pps_offset_ns: 50.0,
            require_pps_aligned: true,
        }
    }
}

/// Output level accumulated over one interval.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LevelMeter {
    pub samples: u64,
    pub sum_sq: f64,
    pub peak: f64,
    /// Samples with |x| > 1.0.
    pub clipped: u64,
}

impl LevelMeter {
    pub fn feed(&mut self, x: &[f64]) {
        for &v in x {
            let a = v.abs();
            self.sum_sq += v * v;
            if a > self.peak {
                self.peak = a;
            }
            if a > 1.0 {
                self.clipped += 1;
            }
        }
        self.samples += x.len() as u64;
    }
    pub fn rms(&self) -> Option<f64> {
        (self.samples > 0).then(|| (self.sum_sq / self.samples as f64).sqrt())
    }
}

fn dbfs(x: f64) -> f64 {
    if x > 0.0 {
        20.0 * x.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// Sensor readings a device wrapper can supply.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SensorReadings {
    pub temperature_c: Option<f64>,
    /// Measured PPS alignment error (ns): device PPS-aligned output minus reference PPS,
    /// less the calibrated delay — e.g. from a time-interval counter.
    pub pps_offset_ns: Option<f64>,
}

#[derive(Debug, Default)]
struct ProbeState {
    name: String,
    status: DeviceStatus,
    sensors: SensorReadings,
    levels: Vec<LevelMeter>,
    write_errors: u64,
    last_error: Option<String>,
}

/// Shared, thread-safe view of a [`MonitoredDevice`].
#[derive(Clone, Debug, Default)]
pub struct HealthProbe {
    state: Arc<Mutex<ProbeState>>,
}

/// What a probe holds at one instant (levels are taken and reset).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeReading {
    pub name: String,
    pub status: DeviceStatus,
    pub sensors: SensorReadings,
    pub levels: Vec<LevelMeter>,
    pub write_errors: u64,
    pub last_error: Option<String>,
}

impl HealthProbe {
    /// Current status, sensors and the output levels since the previous call (reset).
    pub fn take(&self) -> ProbeReading {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let nch = s.levels.len();
        let levels = std::mem::replace(&mut s.levels, vec![LevelMeter::default(); nch]);
        ProbeReading {
            name: s.name.clone(),
            status: s.status.clone(),
            sensors: s.sensors.clone(),
            levels,
            write_errors: s.write_errors,
            last_error: s.last_error.clone(),
        }
    }
    /// Current device status without touching the level meters.
    pub fn device_status(&self) -> DeviceStatus {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status
            .clone()
    }
}

type SensorFn<D> = fn(&D) -> SensorReadings;

/// A [`Device`] wrapper publishing status, output levels and sensors to a [`HealthProbe`].
pub struct MonitoredDevice<D: Device> {
    inner: D,
    probe: HealthProbe,
    sensors: Option<SensorFn<D>>,
}

impl<D: Device> MonitoredDevice<D> {
    pub fn new(inner: D) -> (MonitoredDevice<D>, HealthProbe) {
        let probe = HealthProbe::default();
        let m = MonitoredDevice {
            inner,
            probe: probe.clone(),
            sensors: None,
        };
        m.publish();
        (m, probe)
    }
    /// Read sensors from the wrapped device after every call (e.g.
    /// `|d: &VirtualDevice| SensorReadings { temperature_c: d.temperature_c(), .. }`).
    pub fn with_sensors(mut self, f: SensorFn<D>) -> MonitoredDevice<D> {
        self.sensors = Some(f);
        self.publish();
        self
    }
    pub fn inner(&self) -> &D {
        &self.inner
    }
    pub fn into_inner(self) -> D {
        self.inner
    }
    fn publish(&self) {
        let mut s = self.probe.state.lock().unwrap_or_else(|e| e.into_inner());
        s.name = self.inner.name();
        s.status = self.inner.status();
        if let Some(f) = self.sensors {
            s.sensors = f(&self.inner);
        }
    }
    fn track<T>(&self, r: Result<T, String>) -> Result<T, String> {
        if let Err(e) = &r {
            let mut s = self.probe.state.lock().unwrap_or_else(|e| e.into_inner());
            s.write_errors += 1;
            s.last_error = Some(e.clone());
        }
        self.publish();
        r
    }
}

impl<D: Device> Device for MonitoredDevice<D> {
    fn name(&self) -> String {
        self.inner.name()
    }
    fn realtime(&self) -> bool {
        self.inner.realtime()
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        {
            let mut s = self.probe.state.lock().unwrap_or_else(|e| e.into_inner());
            s.levels = vec![LevelMeter::default(); cfg.tx_channels];
        }
        let r = self.inner.configure(cfg);
        self.track(r)
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.inner.sample_rate_hz()
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let r = self.inner.start_tx(at);
        self.track(r)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        {
            let mut s = self.probe.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.levels.len() < block.len() {
                s.levels.resize(block.len(), LevelMeter::default());
            }
            for (m, x) in s.levels.iter_mut().zip(block) {
                m.feed(x);
            }
        }
        let r = self.inner.write_tx(block);
        self.track(r)
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        let r = self.inner.stop_tx();
        self.track(r)
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let r = self.inner.start_rx(at);
        self.track(r)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        let r = self.inner.read_rx(n);
        self.track(r)
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        let r = self.inner.stop_rx();
        self.track(r)
    }
    fn status(&self) -> DeviceStatus {
        self.inner.status()
    }
}

/// Per-channel output level in a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelLevel {
    pub channel: usize,
    pub samples: u64,
    /// `null` in JSON when no samples were seen (−∞).
    pub rms_dbfs: Option<f64>,
    pub peak_dbfs: Option<f64>,
    pub clipped: u64,
}

/// A health snapshot (serialisable to JSON).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HealthSnapshot {
    /// Caller-supplied timestamp (s); the monitor never reads a clock itself.
    pub t_s: f64,
    pub device: String,
    pub overall: Severity,
    pub tx_streaming: bool,
    pub rx_streaming: bool,
    pub ref_locked: Option<bool>,
    pub underruns_total: u64,
    pub fifo_underruns_total: u64,
    pub overflows_total: u64,
    pub late_total: u64,
    pub underruns_new: u64,
    pub overflows_new: u64,
    pub late_new: u64,
    pub samples_tx: u64,
    pub samples_rx: u64,
    pub temperature_c: Option<f64>,
    pub pps_aligned: bool,
    pub start_time_ns: Option<i64>,
    pub pps_offset_ns: Option<f64>,
    pub output: Vec<ChannelLevel>,
    pub write_errors: u64,
    pub last_error: Option<String>,
    pub alarms: Vec<Alarm>,
}

impl HealthSnapshot {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    pub fn from_json(s: &str) -> Result<HealthSnapshot, String> {
        serde_json::from_str(s).map_err(|e| format!("health snapshot: {e}"))
    }
    pub fn has(&self, code: &str) -> bool {
        self.alarms.iter().any(|a| a.code == code)
    }
}

/// Aggregates device and session state into snapshots with alarms.
#[derive(Clone, Debug, Default)]
pub struct HealthMonitor {
    pub thresholds: HealthThresholds,
    last: Option<(u64, u64, u64)>, // underruns (device+fifo), overflows, late
}

/// Inputs of one evaluation without a probe.
#[derive(Clone, Debug, Default)]
pub struct HealthInputs<'a> {
    pub device: String,
    pub status: DeviceStatus,
    pub session: Option<&'a SessionStatus>,
    pub sensors: SensorReadings,
    pub levels: Vec<LevelMeter>,
    pub write_errors: u64,
    pub last_error: Option<String>,
}

impl HealthMonitor {
    pub fn new(thresholds: HealthThresholds) -> HealthMonitor {
        HealthMonitor {
            thresholds,
            last: None,
        }
    }

    /// Evaluate a probe reading (taking and resetting its level meters).
    pub fn evaluate_probe(
        &mut self,
        probe: &HealthProbe,
        session: Option<&SessionStatus>,
        t_s: f64,
    ) -> HealthSnapshot {
        let r = probe.take();
        self.evaluate(
            &HealthInputs {
                device: r.name,
                status: r.status,
                session,
                sensors: r.sensors,
                levels: r.levels,
                write_errors: r.write_errors,
                last_error: r.last_error,
            },
            t_s,
        )
    }

    /// Evaluate any device directly (no level measurement; for devices not wrapped).
    pub fn evaluate_device(
        &mut self,
        device: &dyn Device,
        session: Option<&SessionStatus>,
        sensors: SensorReadings,
        t_s: f64,
    ) -> HealthSnapshot {
        self.evaluate(
            &HealthInputs {
                device: device.name(),
                status: device.status(),
                session,
                sensors,
                ..Default::default()
            },
            t_s,
        )
    }

    pub fn evaluate(&mut self, inp: &HealthInputs<'_>, t_s: f64) -> HealthSnapshot {
        let th = &self.thresholds;
        let st = &inp.status;
        let fifo = inp.session.map(|s| s.fifo_underruns).unwrap_or(0);
        let und = st.underruns + fifo;
        let (lu, lo, ll) = self.last.unwrap_or((0, 0, 0));
        let (du, dov, dl) = (
            und.saturating_sub(lu),
            st.overflows.saturating_sub(lo),
            st.late.saturating_sub(ll),
        );
        self.last = Some((und, st.overflows, st.late));
        let mut alarms = vec![];
        let mut raise = |code: &str, sev: Severity, msg: String| {
            alarms.push(Alarm {
                code: code.into(),
                severity: sev,
                message: msg,
            })
        };
        if th.require_ref_lock && st.ref_locked == Some(false) {
            raise(
                "ref_unlocked",
                Severity::Alarm,
                "10 MHz reference not locked".into(),
            );
        }
        if du > th.max_underruns {
            raise(
                "underrun",
                Severity::Alarm,
                format!("{du} new transmit underrun(s) (device {}, FIFO {fifo} total)", st.underruns),
            );
        }
        if dov > th.max_overflows {
            raise(
                "overflow",
                Severity::Alarm,
                format!("{dov} new receive overflow(s)"),
            );
        }
        if dl > th.max_late {
            raise(
                "late",
                Severity::Alarm,
                format!("{dl} new late burst / time error(s)"),
            );
        }
        if let Some(t) = inp.sensors.temperature_c {
            if t >= th.temp_alarm_c {
                raise(
                    "temperature",
                    Severity::Alarm,
                    format!("temperature {t:.1} °C ≥ {:.1} °C", th.temp_alarm_c),
                );
            } else if t >= th.temp_warn_c {
                raise(
                    "temperature",
                    Severity::Warning,
                    format!("temperature {t:.1} °C ≥ {:.1} °C", th.temp_warn_c),
                );
            }
        }
        if (st.tx_streaming || st.rx_streaming) && th.require_pps_aligned && !st.pps_aligned {
            raise(
                "pps_not_aligned",
                Severity::Warning,
                "stream not started on a PPS edge".into(),
            );
        }
        if let Some(p) = inp.sensors.pps_offset_ns {
            if p.abs() > th.max_pps_offset_ns {
                raise(
                    "pps_offset",
                    Severity::Alarm,
                    format!(
                        "PPS alignment error {p:.2} ns exceeds ±{:.2} ns",
                        th.max_pps_offset_ns
                    ),
                );
            }
        }
        let mut output = vec![];
        for (c, m) in inp.levels.iter().enumerate() {
            let rms = m.rms().map(dbfs);
            let peak = (m.samples > 0).then(|| dbfs(m.peak));
            if m.samples > 0 {
                if rms.unwrap_or(f64::NEG_INFINITY) < th.min_output_rms_dbfs {
                    raise(
                        "output_low",
                        Severity::Warning,
                        format!(
                            "channel {c}: output RMS {} dBFS below {:.1} dBFS",
                            fmt_db(rms),
                            th.min_output_rms_dbfs
                        ),
                    );
                }
                if m.clipped > 0 {
                    raise(
                        "clipping",
                        Severity::Alarm,
                        format!("channel {c}: {} sample(s) beyond full scale", m.clipped),
                    );
                } else if peak.unwrap_or(f64::NEG_INFINITY) > th.max_output_peak_dbfs {
                    raise(
                        "near_clipping",
                        Severity::Warning,
                        format!("channel {c}: peak {} dBFS", fmt_db(peak)),
                    );
                }
            }
            output.push(ChannelLevel {
                channel: c,
                samples: m.samples,
                rms_dbfs: rms.filter(|v| v.is_finite()),
                peak_dbfs: peak.filter(|v| v.is_finite()),
                clipped: m.clipped,
            });
        }
        if inp.write_errors > 0 {
            raise(
                "device_error",
                Severity::Alarm,
                format!(
                    "{} device call error(s); last: {}",
                    inp.write_errors,
                    inp.last_error.clone().unwrap_or_default()
                ),
            );
        }
        let overall = alarms
            .iter()
            .map(|a| a.severity)
            .max()
            .unwrap_or(Severity::Ok);
        HealthSnapshot {
            t_s,
            device: inp.device.clone(),
            overall,
            tx_streaming: st.tx_streaming,
            rx_streaming: st.rx_streaming,
            ref_locked: st.ref_locked,
            underruns_total: st.underruns,
            fifo_underruns_total: fifo,
            overflows_total: st.overflows,
            late_total: st.late,
            underruns_new: du,
            overflows_new: dov,
            late_new: dl,
            samples_tx: st.samples_tx,
            samples_rx: st.samples_rx,
            temperature_c: inp.sensors.temperature_c,
            pps_aligned: st.pps_aligned,
            start_time_ns: st.start_time_ns,
            pps_offset_ns: inp.sensors.pps_offset_ns,
            output,
            write_errors: inp.write_errors,
            last_error: inp.last_error.clone(),
            alarms,
        }
    }
}

fn fmt_db(x: Option<f64>) -> String {
    match x {
        Some(v) if v.is_finite() => format!("{v:.1}"),
        _ => "-inf".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::virtual_bench::{VirtualConfig, VirtualDevice, VirtualEvent};
    use super::super::{RefSource, TxSession};
    use super::*;

    fn sensors(d: &VirtualDevice) -> SensorReadings {
        SensorReadings {
            temperature_c: d.temperature_c(),
            pps_offset_ns: Some(d.pps_error_ns()),
        }
    }

    fn device(vc: VirtualConfig) -> (MonitoredDevice<VirtualDevice>, HealthProbe) {
        let (mut m, p) = MonitoredDevice::new(VirtualDevice::new(vc).unwrap());
        m = m.with_sensors(sensors);
        let mut cfg = DeviceConfig::tx(1e6, 2);
        cfg.clock_source = RefSource::External;
        m.configure(&cfg).unwrap();
        (m, p)
    }

    #[test]
    fn healthy_session_has_no_alarms_and_measures_level() {
        let mut vc = VirtualConfig::default();
        vc.temperature_c = Some(41.0);
        vc.pps_error_ns = 3.0;
        let (m, probe) = device(vc);
        let s = TxSession::start(Box::new(m), StartTime::NextPps, 4, |sink| {
            for _ in 0..10 {
                let x: Vec<f64> = (0..1000)
                    .map(|i| 0.5 * (i as f64 * 0.6283).sin())
                    .collect();
                sink.write_block(&[x.clone(), x])?;
            }
            sink.finish()
        })
        .unwrap();
        let r = s.wait().unwrap();
        let mut mon = HealthMonitor::default();
        let snap = mon.evaluate_probe(&probe, Some(&r.session), 1.0);
        assert_eq!(snap.overall, Severity::Ok, "{:?}", snap.alarms);
        assert_eq!(snap.samples_tx, 10_000);
        assert_eq!(snap.ref_locked, Some(true));
        assert_eq!(snap.temperature_c, Some(41.0));
        let rms = snap.output[0].rms_dbfs.unwrap();
        assert!((rms - 20.0 * (0.5f64 / 2f64.sqrt()).log10()).abs() < 0.05, "{rms}");
        assert!((snap.output[1].peak_dbfs.unwrap() - 20.0 * 0.5f64.log10()).abs() < 0.5);
        let back = HealthSnapshot::from_json(&snap.to_json()).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn faults_raise_alarms_once_per_increase() {
        let mut vc = VirtualConfig::default();
        vc.temperature_c = Some(78.0);
        vc.pps_error_ns = 120.0;
        vc.events = vec![
            VirtualEvent::Underrun {
                at_sample: 1500,
                gap_samples: 100,
            },
            VirtualEvent::Late { at_sample: 10 },
            VirtualEvent::RefLockLoss {
                from_sample: 0,
                to_sample: 1 << 40,
            },
        ];
        let (mut m, probe) = device(vc);
        m.start_tx(StartTime::Now).unwrap();
        m.write_tx(&[vec![1.2; 1000], vec![1e-5; 1000]]).unwrap();
        m.write_tx(&[vec![0.1; 1000], vec![1e-5; 1000]]).unwrap();
        let mut mon = HealthMonitor::default();
        let a = mon.evaluate_probe(&probe, None, 0.0);
        for code in [
            "ref_unlocked",
            "underrun",
            "late",
            "temperature",
            "pps_offset",
            "pps_not_aligned",
            "clipping",
            "output_low",
        ] {
            assert!(a.has(code), "missing {code}: {:?}", a.alarms);
        }
        assert_eq!(a.overall, Severity::Alarm);
        assert_eq!(a.underruns_total, 1);
        assert_eq!(a.output[0].clipped, 1000);
        let json: serde_json::Value = serde_json::from_str(&a.to_json()).unwrap();
        assert_eq!(json["alarms"].as_array().unwrap().len(), a.alarms.len());
        // no new events: counter alarms clear, the totals stay
        let b = mon.evaluate_probe(&probe, None, 1.0);
        assert!(!b.has("underrun") && !b.has("late") && !b.has("clipping"));
        assert!(b.has("ref_unlocked"));
        assert_eq!(b.underruns_total, 1);
        // a failing device call is recorded
        let mut m = m;
        m.stop_tx().unwrap();
        assert!(m.write_tx(&[vec![0.0; 10], vec![0.0; 10]]).is_err());
        let c = mon.evaluate_probe(&probe, None, 2.0);
        assert!(c.has("device_error"));
    }
}
