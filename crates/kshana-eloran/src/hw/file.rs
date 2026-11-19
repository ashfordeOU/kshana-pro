// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! [`FileDevice`]: a [`Device`] that writes its transmit stream to a SigMF recording
//! (through [`crate::io::SigmfWriter`]) and reads its receive stream from one. With
//! pacing on it behaves like real hardware in time: writes are held back to the sample
//! clock against the host's wall clock, `NextPps` waits for the next whole UTC second
//! of the host clock, and falling behind counts as an underrun. Used for tests, dry runs
//! and as a loopback stand-in for a capture.

use super::{Device, DeviceConfig, DeviceStatus, StartTime};
use crate::io::{Format, SigmfWriter};
use crate::replay::reader::{patch_meta, SigmfReader};
use crate::scene::SampleSink;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A SigMF-backed device.
pub struct FileDevice {
    tx_base: Option<PathBuf>,
    rx_base: Option<PathBuf>,
    format: Format,
    pacing: bool,
    cfg: Option<DeviceConfig>,
    writer: Option<SigmfWriter>,
    reader: Option<SigmfReader>,
    t0: Option<Instant>,
    status: DeviceStatus,
    /// Extra metadata stored under `kshana:eloran` in the transmit recording.
    pub meta: serde_json::Value,
}

impl FileDevice {
    /// A transmit-to-file device writing `<base>.sigmf-{data,meta}`.
    pub fn tx(base: &Path, format: Format) -> FileDevice {
        FileDevice {
            tx_base: Some(base.to_path_buf()),
            rx_base: None,
            format,
            pacing: false,
            cfg: None,
            writer: None,
            reader: None,
            t0: None,
            status: DeviceStatus::default(),
            meta: json!({}),
        }
    }
    /// A receive-from-file device reading `<base>.sigmf-{data,meta}`.
    pub fn rx(base: &Path) -> FileDevice {
        let mut d = FileDevice::tx(base, Format::Rf32);
        d.tx_base = None;
        d.rx_base = Some(base.to_path_buf());
        d
    }
    /// Add a receive source to a transmit device.
    pub fn with_rx_source(mut self, base: &Path) -> FileDevice {
        self.rx_base = Some(base.to_path_buf());
        self
    }
    /// Pace writes and reads to the sample clock (real-time dry run).
    pub fn with_pacing(mut self, on: bool) -> FileDevice {
        self.pacing = on;
        self
    }

    fn fs(&self) -> Result<f64, String> {
        self.cfg
            .as_ref()
            .map(|c| c.sample_rate_hz)
            .ok_or_else(|| "device not configured".to_string())
    }

    /// Wait for the start instant; returns (start time ns, pps aligned).
    fn wait_start(&self, at: StartTime) -> (Option<i64>, bool) {
        match at {
            StartTime::Now => (Some(0), false),
            StartTime::AtDeviceTimeNs(t) => (Some(t), false),
            StartTime::NextPps => {
                if self.pacing {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default();
                    let wait =
                        Duration::from_secs(1) - Duration::from_nanos(now.subsec_nanos() as u64);
                    std::thread::sleep(wait);
                    (Some((now.as_secs() as i64 + 1) * 1_000_000_000), true)
                } else {
                    // Without pacing the file's sample 0 *is* the PPS edge.
                    (Some(0), true)
                }
            }
        }
    }

    fn pace(&mut self, samples: u64) -> Result<(), String> {
        if !self.pacing {
            return Ok(());
        }
        let fs = self.fs()?;
        let t0 = *self.t0.get_or_insert_with(Instant::now);
        let due = Duration::from_secs_f64(samples as f64 / fs);
        let el = t0.elapsed();
        if due > el {
            std::thread::sleep(due - el);
        } else if el - due > Duration::from_millis(50) {
            self.status.underruns += 1;
        }
        Ok(())
    }
}

impl Device for FileDevice {
    fn name(&self) -> String {
        match (&self.tx_base, &self.rx_base) {
            (Some(t), _) => format!("file:{}", t.display()),
            (None, Some(r)) => format!("file-rx:{}", r.display()),
            _ => "file".into(),
        }
    }
    fn realtime(&self) -> bool {
        self.pacing
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        cfg.validate()?;
        self.cfg = Some(cfg.clone());
        Ok(())
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.cfg.as_ref().map(|c| c.sample_rate_hz)
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let cfg = self.cfg.clone().ok_or("device not configured")?;
        let base = self
            .tx_base
            .clone()
            .ok_or("this file device has no transmit file")?;
        if cfg.tx_channels == 0 {
            return Err("no tx channels configured".into());
        }
        let mut extra = self.meta.clone();
        extra["device"] = json!(self.name());
        extra["output_policy"] = json!(super::OUTPUT_POLICY);
        extra["device_config"] = serde_json::to_value(&cfg).map_err(|e| e.to_string())?;
        self.writer = Some(SigmfWriter::create(
            &base,
            self.format,
            cfg.tx_channels,
            cfg.sample_rate_hz,
            extra,
        )?);
        let (t, pps) = self.wait_start(at);
        self.t0 = Some(Instant::now());
        self.status.tx_streaming = true;
        self.status.start_time_ns = t;
        self.status.pps_aligned = pps;
        Ok(t)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        let w = self.writer.as_mut().ok_or("transmit stream not started")?;
        w.write_block(block)?;
        let n = block.first().map(|c| c.len()).unwrap_or(0) as u64;
        self.status.samples_tx += n;
        let s = self.status.samples_tx;
        self.pace(s)
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        if let Some(mut w) = self.writer.take() {
            w.finish()?;
            let base = self.tx_base.clone().unwrap_or_default();
            let st = self.status.clone();
            patch_meta(&base, |m| {
                if let Some(c) = m["captures"].get_mut(0) {
                    c["kshana:pps_aligned"] = json!(st.pps_aligned);
                    c["kshana:start_time_ns"] = json!(st.start_time_ns);
                    c["kshana:clipped_samples"] = json!(w.clipped);
                }
            })?;
        }
        self.status.tx_streaming = false;
        Ok(())
    }
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let base = self
            .rx_base
            .clone()
            .ok_or("this file device has no receive source")?;
        let r = SigmfReader::open(&base)?;
        if let Some(c) = &self.cfg {
            if (c.sample_rate_hz - r.fs_hz).abs() > 1e-6 {
                return Err(format!(
                    "receive source is {} Hz, device configured for {} Hz",
                    r.fs_hz, c.sample_rate_hz
                ));
            }
        } else {
            let mut c = DeviceConfig::tx(r.fs_hz, 0);
            c.rx_channels = r.channels;
            self.cfg = Some(c);
        }
        self.reader = Some(r);
        let (t, pps) = self.wait_start(at);
        self.t0 = Some(Instant::now());
        self.status.rx_streaming = true;
        self.status.start_time_ns = t;
        self.status.pps_aligned = pps;
        Ok(t)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        let r = self.reader.as_mut().ok_or("receive stream not started")?;
        let mut b = r.read(n)?;
        let want = self.cfg.as_ref().map(|c| c.rx_channels).unwrap_or(0);
        if want > 0 && want < b.len() {
            b.truncate(want);
        }
        let k = b.first().map(|c| c.len()).unwrap_or(0) as u64;
        self.status.samples_rx += k;
        let s = self.status.samples_rx;
        self.pace(s)?;
        Ok(b)
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        self.reader = None;
        self.status.rx_streaming = false;
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        self.status.clone()
    }
}
