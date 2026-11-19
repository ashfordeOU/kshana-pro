// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Hardware output and capture (stream S13).
//!
//! # Output policy
//!
//! **Hardware output is for conducted use only.** Connect a device's output to cables,
//! attenuators, couplers or an antenna simulator (H-field loop simulator, E-field
//! injection network), **never to a radiating antenna**. LF transmission over the air
//! without authorisation is unlawful in most jurisdictions and can disturb navigation and
//! timing services. Every hardware device prints [`OUTPUT_POLICY`] when it is opened.
//!
//! # Model
//!
//! * [`Device`]: one SDR or file back end — configure (sample rate, channels, gains,
//!   10 MHz reference source, 1PPS time source), start at the next PPS edge or at a device
//!   time, stream real samples per channel out, read samples in, report counters.
//! * [`TxSession`]: non-blocking streaming. A **producer thread** runs any sample source
//!   (normally [`crate::scene::Scene::run`]) into a [`RealtimeSink`], which pushes blocks into
//!   a bounded FIFO with backpressure; a **pump thread** pulls blocks out and writes them to
//!   the device. The caller keeps control and can poll [`TxSession::status`] or stop.
//! * Back ends: [`file::FileDevice`] (SigMF, optional real-time pacing; tests and dry
//!   runs) and, behind the `soapy` feature on Unix, `soapy::SoapyDevice` (SoapySDR 0.8
//!   through `dlopen`).
//! * [`calib`]: calibration files, the [`calib::CalibratedSink`] (per-channel fractional
//!   delay and gain) and calibration procedures.
//!
//! Sample values are the scene's output units: ±1.0 is device full scale.

pub mod awg;
pub mod calib;
#[cfg(all(feature = "digilent", unix))]
pub mod digilent;
#[cfg(all(any(feature = "uhd", feature = "digilent"), unix))]
pub mod dl;
pub mod file;
pub mod fir;
pub mod health;
#[cfg(feature = "redpitaya")]
pub mod redpitaya;
pub mod scpi_awg;
#[cfg(all(feature = "soapy", unix))]
pub mod soapy;
#[cfg(all(feature = "uhd", unix))]
pub mod uhd;
/// The virtual bench device (`src/hw/virtual.rs`; `virtual` is a reserved word in Rust).
#[path = "virtual.rs"]
pub mod virtual_bench;

use crate::scene::SampleSink;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;

/// The conducted-output policy, printed whenever a hardware device is opened.
pub const OUTPUT_POLICY: &str = "OUTPUT POLICY: conducted use only. Connect this output to \
cables, attenuators or an antenna simulator, never to a radiating antenna. Over-the-air LF \
transmission is not permitted.";

/// Print the output policy for `device` on standard error.
pub fn announce_output_policy(device: &str) {
    eprintln!("[kshana-eloran] {device}: {OUTPUT_POLICY}");
}

/// Source of the 10 MHz frequency reference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefSource {
    #[default]
    Internal,
    /// External 10 MHz (SoapySDR clock source "external").
    External,
}

/// Source of the device time (1PPS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PpsSource {
    #[default]
    Internal,
    /// External 1PPS (SoapySDR time source "external").
    External,
}

/// Device configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Sample rate (Hz); must match the scene's `fs_hz`.
    pub sample_rate_hz: f64,
    /// Number of transmit (output) channels.
    pub tx_channels: usize,
    /// Number of receive (capture) channels.
    #[serde(default)]
    pub rx_channels: usize,
    /// Transmit gain per channel (dB, device-specific meaning); missing entries = 0 dB.
    #[serde(default)]
    pub tx_gain_db: Vec<f64>,
    #[serde(default)]
    pub rx_gain_db: Vec<f64>,
    /// RF centre frequency (Hz). 0 for a DC-coupled LF daughterboard carrying the real
    /// LF signal on I directly.
    #[serde(default)]
    pub center_freq_hz: f64,
    #[serde(default)]
    pub clock_source: RefSource,
    #[serde(default)]
    pub time_source: PpsSource,
}

impl DeviceConfig {
    pub fn tx(sample_rate_hz: f64, tx_channels: usize) -> DeviceConfig {
        DeviceConfig {
            sample_rate_hz,
            tx_channels,
            rx_channels: 0,
            tx_gain_db: vec![],
            rx_gain_db: vec![],
            center_freq_hz: 0.0,
            clock_source: RefSource::Internal,
            time_source: PpsSource::Internal,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.sample_rate_hz.is_nan() || self.sample_rate_hz <= 0.0 {
            return Err("sample_rate_hz must be positive".into());
        }
        if self.tx_channels == 0 && self.rx_channels == 0 {
            return Err("at least one tx or rx channel".into());
        }
        Ok(())
    }
}

/// When a stream starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartTime {
    /// As soon as possible (not PPS aligned).
    Now,
    /// On the next 1PPS edge of the device time source.
    NextPps,
    /// At a device time (ns, on the device clock as set from the PPS).
    AtDeviceTimeNs(i64),
}

/// Device status snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub tx_streaming: bool,
    pub rx_streaming: bool,
    pub samples_tx: u64,
    pub samples_rx: u64,
    /// Transmit underruns reported by the device (or pacing falling behind).
    pub underruns: u64,
    /// Receive overflows (samples dropped by the device).
    pub overflows: u64,
    /// Late-command / time errors.
    pub late: u64,
    /// Device time at the stream start (ns), when known.
    pub start_time_ns: Option<i64>,
    /// Whether the start was aligned to a PPS edge.
    pub pps_aligned: bool,
    /// Reference lock, when the device reports it.
    pub ref_locked: Option<bool>,
    pub message: String,
}

/// One output/capture back end. Blocks are `channels[c][n]`, real values, ±1.0 = full scale.
pub trait Device: Send {
    /// Human-readable device name.
    fn name(&self) -> String;
    /// Whether writes are paced by a real clock (underruns are then meaningful).
    fn realtime(&self) -> bool;
    /// Apply a configuration (before starting a stream).
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String>;
    /// Configured sample rate (Hz), once configured.
    fn sample_rate_hz(&self) -> Option<f64>;
    /// Start the transmit stream; returns the device start time (ns) when known.
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String>;
    /// Write one block (blocks until the device accepted it).
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String>;
    /// End the transmit stream (flush, end of burst).
    fn stop_tx(&mut self) -> Result<(), String>;
    /// Start the receive stream; returns the device start time (ns) when known.
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String>;
    /// Read up to `n` samples per channel; an empty result means end of stream.
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String>;
    fn stop_rx(&mut self) -> Result<(), String>;
    fn status(&self) -> DeviceStatus;
}

/// Counters shared by a [`TxSession`]'s threads.
#[derive(Debug, Default)]
pub struct StreamCounters {
    pub blocks_queued: AtomicU64,
    pub blocks_written: AtomicU64,
    pub samples_written: AtomicU64,
    /// Producer found the FIFO full and waited (backpressure, normal).
    pub backpressure_waits: AtomicU64,
    /// Pump found the FIFO empty while the device was streaming (real-time devices only).
    pub fifo_underruns: AtomicU64,
    pub producer_done: AtomicBool,
    pub stop: AtomicBool,
}

/// Snapshot of a session.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionStatus {
    pub blocks_queued: u64,
    pub blocks_written: u64,
    pub samples_written: u64,
    pub backpressure_waits: u64,
    pub fifo_underruns: u64,
    pub producer_done: bool,
}

impl StreamCounters {
    pub fn snapshot(&self) -> SessionStatus {
        SessionStatus {
            blocks_queued: self.blocks_queued.load(Ordering::Relaxed),
            blocks_written: self.blocks_written.load(Ordering::Relaxed),
            samples_written: self.samples_written.load(Ordering::Relaxed),
            backpressure_waits: self.backpressure_waits.load(Ordering::Relaxed),
            fifo_underruns: self.fifo_underruns.load(Ordering::Relaxed),
            producer_done: self.producer_done.load(Ordering::Relaxed),
        }
    }
}

enum Msg {
    Block(Vec<Vec<f64>>),
    End,
}

/// A [`SampleSink`] that pushes blocks into a device FIFO, waiting while it is full.
pub struct RealtimeSink {
    tx: SyncSender<Msg>,
    counters: Arc<StreamCounters>,
}

impl SampleSink for RealtimeSink {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        if self.counters.stop.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }
        let msg = Msg::Block(channels.to_vec());
        let msg = match self.tx.try_send(msg) {
            Ok(()) => None,
            Err(TrySendError::Full(m)) => {
                self.counters
                    .backpressure_waits
                    .fetch_add(1, Ordering::Relaxed);
                Some(m)
            }
            Err(TrySendError::Disconnected(_)) => return Err("device stream closed".into()),
        };
        if let Some(m) = msg {
            self.tx
                .send(m)
                .map_err(|_| "device stream closed".to_string())?;
        }
        self.counters.blocks_queued.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn finish(&mut self) -> Result<(), String> {
        let _ = self.tx.send(Msg::End);
        Ok(())
    }
}

/// Result of a finished [`TxSession`].
pub struct TxReport<T> {
    /// What the producer returned (e.g. run statistics and truth).
    pub produced: T,
    /// The device, returned for further use or status.
    pub device: Box<dyn Device>,
    pub session: SessionStatus,
    pub device_status: DeviceStatus,
}

/// A running transmit session (producer thread + pump thread).
pub struct TxSession<T: Send + 'static> {
    counters: Arc<StreamCounters>,
    producer: JoinHandle<Result<T, String>>,
    pump: JoinHandle<Result<Box<dyn Device>, String>>,
}

impl<T: Send + 'static> TxSession<T> {
    /// Start streaming: `producer` writes all its samples into the sink it is given
    /// (calling `finish` at the end, as `Scene::run` does). The device starts at `at`
    /// once the FIFO (`fifo_blocks` deep) is half full or the producer has finished.
    pub fn start<F>(
        mut device: Box<dyn Device>,
        at: StartTime,
        fifo_blocks: usize,
        producer: F,
    ) -> Result<TxSession<T>, String>
    where
        F: FnOnce(&mut dyn SampleSink) -> Result<T, String> + Send + 'static,
    {
        let fifo_blocks = fifo_blocks.max(2);
        let (tx, rx) = sync_channel::<Msg>(fifo_blocks);
        let counters = Arc::new(StreamCounters::default());
        let c_prod = counters.clone();
        let producer = std::thread::Builder::new()
            .name("eloran-producer".into())
            .spawn(move || {
                let mut sink = RealtimeSink {
                    tx,
                    counters: c_prod.clone(),
                };
                let r = producer(&mut sink);
                // make sure the pump ends even if the producer failed before finish()
                let _ = sink.tx.send(Msg::End);
                c_prod.producer_done.store(true, Ordering::Relaxed);
                r
            })
            .map_err(|e| e.to_string())?;
        let c_pump = counters.clone();
        let prime = fifo_blocks / 2;
        let pump = std::thread::Builder::new()
            .name("eloran-pump".into())
            .spawn(move || -> Result<Box<dyn Device>, String> {
                pump(&mut *device, &rx, at, prime, &c_pump)?;
                Ok(device)
            })
            .map_err(|e| e.to_string())?;
        Ok(TxSession {
            counters,
            producer,
            pump,
        })
    }

    /// Current counters.
    pub fn status(&self) -> SessionStatus {
        self.counters.snapshot()
    }

    /// Ask the producer to stop at its next block; [`TxSession::wait`] then returns its error.
    pub fn stop(&self) {
        self.counters.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_finished(&self) -> bool {
        self.producer.is_finished() && self.pump.is_finished()
    }

    /// Wait for both threads; returns the producer's output and the device.
    pub fn wait(self) -> Result<TxReport<T>, String> {
        let p = self
            .producer
            .join()
            .map_err(|_| "producer thread panicked".to_string())?;
        let d = self
            .pump
            .join()
            .map_err(|_| "pump thread panicked".to_string())?;
        let device = d?;
        let produced = p?;
        let device_status = device.status();
        Ok(TxReport {
            produced,
            device,
            session: self.counters.snapshot(),
            device_status,
        })
    }
}

fn pump(
    device: &mut dyn Device,
    rx: &Receiver<Msg>,
    at: StartTime,
    prime: usize,
    c: &StreamCounters,
) -> Result<(), String> {
    // Prime the FIFO before starting so a real-time device does not underrun at once.
    let mut pending: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut ended = false;
    while pending.len() < prime {
        match rx.recv() {
            Ok(Msg::Block(b)) => pending.push(b),
            Ok(Msg::End) | Err(_) => {
                ended = true;
                break;
            }
        }
    }
    device.start_tx(at)?;
    let rt = device.realtime();
    let write = |device: &mut dyn Device, b: Vec<Vec<f64>>| -> Result<(), String> {
        device.write_tx(&b)?;
        c.blocks_written.fetch_add(1, Ordering::Relaxed);
        c.samples_written.fetch_add(
            b.first().map(|x| x.len()).unwrap_or(0) as u64,
            Ordering::Relaxed,
        );
        Ok(())
    };
    for b in pending {
        if let Err(e) = write(device, b) {
            c.stop.store(true, Ordering::Relaxed);
            let _ = device.stop_tx();
            return Err(e);
        }
    }
    while !ended {
        let msg = match rx.try_recv() {
            Ok(m) => Ok(m),
            Err(TryRecvError::Empty) => {
                if rt && !c.producer_done.load(Ordering::Relaxed) {
                    c.fifo_underruns.fetch_add(1, Ordering::Relaxed);
                }
                rx.recv().map_err(|_| ())
            }
            Err(TryRecvError::Disconnected) => Err(()),
        };
        match msg {
            Ok(Msg::Block(b)) => {
                if let Err(e) = write(device, b) {
                    c.stop.store(true, Ordering::Relaxed);
                    let _ = device.stop_tx();
                    return Err(e);
                }
            }
            Ok(Msg::End) | Err(()) => ended = true,
        }
    }
    device.stop_tx()
}

/// Stream a scene to a device: configures nothing (configure the device first), runs the
/// scene in a producer thread, returns the session. The producer returns the run
/// statistics and the truth record.
pub fn stream_scene(
    device: Box<dyn Device>,
    mut scene: crate::scene::Scene,
    at: StartTime,
    fifo_blocks: usize,
) -> Result<TxSession<(crate::scene::RunStats, Vec<crate::scene::TruthRecord>)>, String> {
    TxSession::start(device, at, fifo_blocks, move |sink| {
        let mut truth = Vec::new();
        let st = scene.run(sink, &mut truth)?;
        Ok((st, truth))
    })
}

#[cfg(test)]
mod tests {
    use super::file::FileDevice;
    use super::*;
    use crate::io::{read_sigmf, Format};

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let d =
            std::env::temp_dir().join(format!("kshana-eloran-hw-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn session_streams_all_blocks_in_order_with_backpressure() {
        let d = tmpdir("session");
        let base = d.join("out");
        let mut dev = FileDevice::tx(&base, Format::Rf32);
        dev.configure(&DeviceConfig::tx(1e6, 2)).unwrap();
        let s = TxSession::start(Box::new(dev), StartTime::NextPps, 2, |sink| {
            for k in 0..50 {
                let b: Vec<f64> = (0..100).map(|i| ((k * 100 + i) as f64) * 1e-5).collect();
                sink.write_block(&[b.clone(), b.iter().map(|v| -v).collect()])?;
            }
            sink.finish()?;
            Ok(42u32)
        })
        .unwrap();
        let r = s.wait().unwrap();
        assert_eq!(r.produced, 42);
        assert_eq!(r.session.blocks_written, 50);
        assert_eq!(r.session.samples_written, 5000);
        assert!(r.device_status.pps_aligned);
        let rec = read_sigmf(&base, None).unwrap();
        assert_eq!(rec.channels[0].len(), 5000);
        for (i, v) in rec.channels[0].iter().enumerate() {
            assert!((v - i as f64 * 1e-5).abs() < 1e-6);
            assert!((rec.channels[1][i] + v).abs() < 1e-6);
        }
    }

    #[test]
    fn producer_error_propagates_and_device_stops() {
        let d = tmpdir("err");
        let mut dev = FileDevice::tx(&d.join("e"), Format::Ri16);
        dev.configure(&DeviceConfig::tx(1e6, 1)).unwrap();
        let s = TxSession::start(
            Box::new(dev),
            StartTime::Now,
            4,
            |sink| -> Result<(), String> {
                sink.write_block(&[vec![0.0; 10]])?;
                Err("boom".into())
            },
        )
        .unwrap();
        assert_eq!(s.wait().err().unwrap(), "boom");
    }

    #[test]
    fn stream_scene_runs_a_scene_through_a_file_device() {
        let src = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.05
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
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 55.5, lon_deg = 0.0 }
"#;
        let scene = crate::Scenario::from_toml(src).unwrap().build().unwrap();
        let d = tmpdir("scene");
        let base = d.join("scene");
        let mut dev = FileDevice::tx(&base, Format::Rf32);
        dev.configure(&DeviceConfig::tx(1e6, 1)).unwrap();
        let r = stream_scene(Box::new(dev), scene, StartTime::NextPps, 8)
            .unwrap()
            .wait()
            .unwrap();
        assert_eq!(r.produced.0.samples_per_channel, 50_000);
        assert!(!r.produced.1.is_empty());
        let rec = read_sigmf(&base, None).unwrap();
        assert_eq!(rec.channels[0].len(), 50_000);
        assert!(rec.channels[0].iter().any(|v| v.abs() > 0.0));
    }
}
