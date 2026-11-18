// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Record and replay (stream S12).
//!
//! * [`capture`]: record from a [`Device`]'s receive side to SigMF, with a timestamped
//!   `captures` entry (PPS-aligned start, reference source, antenna, gain);
//! * [`inventory`]: list the SigMF recordings in a folder (duration, rate, channels);
//! * [`extract_segment`]: copy a time segment into a new recording;
//! * [`decimate`]: integer-factor decimation with a Kaiser-windowed-sinc anti-alias FIR;
//! * [`replay`] / [`replay_to_device`]: play a recording into any [`SampleSink`] (looped,
//!   with a gain from a calibration file), or through a [`Device`] in a [`TxSession`].
//!
//! Hardware replay obeys the output policy of [`crate::hw`]: cables or antenna simulators
//! only, never over the air.

pub mod reader;

pub use reader::SigmfReader;

use crate::hw::calib::CalibrationFile;
use crate::hw::fir::{lowpass_taps, FirState};
use crate::hw::{Device, StartTime, TxSession};
use crate::io::{Format, SigmfWriter};
use crate::scene::SampleSink;
use reader::patch_meta;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Block length used when streaming files (samples per channel).
pub const BLOCK: usize = 65_536;

/// Description of a capture, stored in the SigMF `captures` entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CaptureInfo {
    /// UTC of the first sample (ISO 8601), when known (e.g. the PPS the capture started on).
    pub datetime_utc: Option<String>,
    /// Frequency reference / time source, e.g. "external 10 MHz + 1PPS from GNSSDO".
    pub reference_source: String,
    /// Antenna or input, e.g. "H-loop pair NS/EW" or "loopback from TX0 via 30 dB pad".
    pub antenna: String,
    /// Receive gain per channel (dB).
    pub gain_db: Vec<f64>,
    #[serde(default)]
    pub notes: String,
}

/// Summary of a capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureSummary {
    pub samples: u64,
    pub channels: usize,
    pub start_time_ns: Option<i64>,
    pub pps_aligned: bool,
    pub overflows: u64,
}

/// Capture `n_samples` per channel from `device` (configured, with rx channels) into
/// `<base>.sigmf-*`, starting at `at` (use [`StartTime::NextPps`] for a PPS-aligned start).
pub fn capture(device: &mut dyn Device, at: StartTime, n_samples: u64, base: &Path, format: Format, info: &CaptureInfo) -> Result<CaptureSummary, String> {
    let t = device.start_rx(at)?;
    let mut writer: Option<SigmfWriter> = None;
    let mut got = 0u64;
    let mut fs = 0.0;
    let res = (|| -> Result<(), String> {
        while got < n_samples {
            let b = device.read_rx(BLOCK.min((n_samples - got) as usize))?;
            let k = b.first().map(|c| c.len()).unwrap_or(0);
            if k == 0 {
                break;
            }
            if writer.is_none() {
                fs = device_rate(device)?;
                writer = Some(SigmfWriter::create(base, format, b.len(), fs, json!({ "capture_device": device.name() }))?);
            }
            writer.as_mut().unwrap().write_block(&b)?;
            got += k as u64;
        }
        Ok(())
    })();
    device.stop_rx()?;
    res?;
    let mut w = writer.ok_or("capture returned no samples")?;
    w.finish()?;
    let st = device.status();
    let channels = w_channels(base)?;
    patch_meta(base, |m| {
        m["global"]["core:description"] = json!("Kshana Pro eLoran capture (real samples, LF band)");
        let c = &mut m["captures"][0];
        c["core:frequency"] = json!(100000.0);
        if let Some(d) = &info.datetime_utc {
            c["core:datetime"] = json!(d);
        }
        c["kshana:pps_aligned"] = json!(st.pps_aligned);
        c["kshana:start_time_ns"] = json!(t);
        c["kshana:reference_source"] = json!(info.reference_source);
        c["kshana:antenna"] = json!(info.antenna);
        c["kshana:gain_db"] = json!(info.gain_db);
        c["kshana:overflows"] = json!(st.overflows);
        if !info.notes.is_empty() {
            c["kshana:notes"] = json!(info.notes);
        }
    })?;
    Ok(CaptureSummary { samples: got, channels, start_time_ns: t, pps_aligned: st.pps_aligned, overflows: st.overflows })
}

fn w_channels(base: &Path) -> Result<usize, String> {
    Ok(reader::read_meta(base)?["global"]["core:num_channels"].as_u64().unwrap_or(1) as usize)
}

fn device_rate(device: &dyn Device) -> Result<f64, String> {
    device
        .sample_rate_hz()
        .ok_or_else(|| format!("{}: sample rate unknown (configure the device before capture)", device.name()))
}

/// One SigMF recording in an inventory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DatasetEntry {
    /// Base path (without extension).
    pub base: PathBuf,
    pub datatype: String,
    pub sample_rate_hz: f64,
    pub channels: usize,
    pub samples: u64,
    pub duration_s: f64,
    pub datetime_utc: Option<String>,
    pub description: String,
}

/// List the SigMF recordings (`*.sigmf-meta` with a matching data file) in `dir`, sorted.
/// Recordings in formats this crate cannot read are listed with their metadata.
pub fn inventory(dir: &Path) -> Result<Vec<DatasetEntry>, String> {
    let mut out = Vec::new();
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("sigmf-meta") {
            continue;
        }
        let base = p.with_extension("");
        let data = base.with_extension("sigmf-data");
        let Ok(meta) = reader::read_meta(&base) else { continue };
        let g = &meta["global"];
        let datatype = g["core:datatype"].as_str().unwrap_or("").to_string();
        let fs = g["core:sample_rate"].as_f64().unwrap_or(0.0);
        let nch = g["core:num_channels"].as_u64().unwrap_or(1).max(1) as usize;
        let bytes_per = sigmf_sample_bytes(&datatype);
        let len = std::fs::metadata(&data).map(|m| m.len()).unwrap_or(0);
        let samples = if bytes_per > 0 { len / (bytes_per * nch) as u64 } else { 0 };
        out.push(DatasetEntry {
            base,
            datatype,
            sample_rate_hz: fs,
            channels: nch,
            samples,
            duration_s: if fs > 0.0 { samples as f64 / fs } else { 0.0 },
            datetime_utc: meta["captures"][0]["core:datetime"].as_str().map(String::from),
            description: g["core:description"].as_str().unwrap_or("").to_string(),
        });
    }
    out.sort_by(|a, b| a.base.cmp(&b.base));
    Ok(out)
}

/// Bytes per sample (per channel) of a SigMF datatype (complex counts both parts), 0 if unknown.
fn sigmf_sample_bytes(dt: &str) -> usize {
    let (cplx, rest) = match dt.chars().next() {
        Some('c') => (2, &dt[1..]),
        Some('r') => (1, &dt[1..]),
        _ => return 0,
    };
    let bits: String = rest.chars().skip(1).take_while(|c| c.is_ascii_digit()).collect();
    bits.parse::<usize>().map(|b| cplx * b / 8).unwrap_or(0)
}

/// Copy `[start_s, start_s + duration_s)` of `src` into `dst` (same format). The new
/// capture's datetime is shifted by `start_s` when the source has one (only the
/// fractional-second arithmetic of ISO stamps with a `Z` suffix is supported).
pub fn extract_segment(src: &Path, dst: &Path, start_s: f64, duration_s: f64) -> Result<u64, String> {
    let mut r = SigmfReader::open(src)?;
    let n0 = (start_s * r.fs_hz).round().max(0.0) as u64;
    if n0 >= r.total {
        return Err(format!("segment start {start_s} s is beyond the end ({:.3} s)", r.duration_s()));
    }
    let n = ((duration_s * r.fs_hz).round() as u64).min(r.total - n0);
    r.seek(n0)?;
    let extra = json!({ "segment_of": src.display().to_string(), "segment_start_sample": n0 });
    let mut w = SigmfWriter::create(dst, r.format, r.channels, r.fs_hz, extra)?;
    let mut left = n;
    while left > 0 {
        let b = r.read(BLOCK.min(left as usize))?;
        let k = b[0].len() as u64;
        if k == 0 {
            break;
        }
        w.write_block(&b)?;
        left -= k;
    }
    w.finish()?;
    copy_capture_fields(&r.meta, dst, n0 as f64 / r.fs_hz)?;
    Ok(n - left)
}

fn copy_capture_fields(src_meta: &serde_json::Value, dst: &Path, offset_s: f64) -> Result<(), String> {
    let c = src_meta["captures"][0].clone();
    patch_meta(dst, |m| {
        if let Some(obj) = c.as_object() {
            for (k, v) in obj {
                if k == "core:sample_start" {
                    continue;
                }
                m["captures"][0][k] = v.clone();
            }
        }
        if let Some(d) = c["core:datetime"].as_str() {
            if let Some(nd) = shift_iso(d, offset_s) {
                m["captures"][0]["core:datetime"] = json!(nd);
            }
        }
        if offset_s != 0.0 {
            m["captures"][0]["kshana:pps_aligned"] = json!(false);
        }
    })
}

/// Shift an ISO-8601 UTC stamp `YYYY-MM-DDTHH:MM:SS[.f]Z` by `dt` seconds (no leap seconds).
pub fn shift_iso(s: &str, dt: f64) -> Option<String> {
    let u = crate::sync::UtcStamp::parse(s).ok()?;
    Some(u.plus(dt).iso())
}

/// Decimate `src` by the integer `factor` into `dst` (format `rf32` keeps the filter's
/// dynamic range). The anti-alias filter is a Kaiser-windowed sinc with its cut-off at 90 % of
/// the output Nyquist frequency (`0.45/factor` cycles per input sample), `taps` long
/// (odd; 0 = automatic: 32·factor + 1), β = 8 (≈ 80 dB stop band). The filter delay is
/// compensated, so the output is time-aligned with the input (sample k ↔ input k·factor).
pub fn decimate(src: &Path, dst: &Path, factor: usize, taps: usize, format: Format) -> Result<u64, String> {
    if factor < 1 {
        return Err("decimation factor must be ≥ 1".into());
    }
    let mut r = SigmfReader::open(src)?;
    let n_taps = if taps == 0 { 32 * factor + 1 } else { taps | 1 };
    let h = lowpass_taps(0.45 / factor as f64, n_taps, 8.0);
    let gd = n_taps / 2; // group delay (input samples)
    let mut firs: Vec<FirState> = (0..r.channels).map(|_| FirState::new(h.clone(), 0)).collect();
    let fs_out = r.fs_hz / factor as f64;
    let extra = json!({ "decimated_from": src.display().to_string(), "decimation_factor": factor, "anti_alias_taps": n_taps });
    let mut w = SigmfWriter::create(dst, format, r.channels, fs_out, extra)?;
    // global input index of the next sample entering the filter, and output count
    let mut n_in: u64 = 0;
    let mut n_out: u64 = 0;
    let total_out = r.total.div_ceil(factor as u64);
    let mut flushing = 0usize;
    loop {
        let mut b = r.read(BLOCK)?;
        if b[0].is_empty() {
            if flushing >= gd {
                break;
            }
            let pad = (gd - flushing).min(BLOCK);
            flushing += pad;
            b = vec![vec![0.0; pad]; r.channels];
        }
        let k = b[0].len();
        let ys: Vec<Vec<f64>> = b.iter().enumerate().map(|(c, x)| firs[c].process(x)).collect();
        let mut out: Vec<Vec<f64>> = vec![Vec::new(); r.channels];
        for (i, _) in ys[0].iter().enumerate() {
            let g = n_in + i as u64; // filter output index g ↔ input sample g − gd
            if g < gd as u64 {
                continue;
            }
            let src_idx = g - gd as u64;
            if src_idx.is_multiple_of(factor as u64) && n_out < total_out {
                for (o, y) in out.iter_mut().zip(&ys) {
                    o.push(y[i]);
                }
                n_out += 1;
            }
        }
        n_in += k as u64;
        if !out[0].is_empty() {
            w.write_block(&out)?;
        }
    }
    w.finish()?;
    copy_capture_fields(&r.meta, dst, 0.0)?;
    Ok(n_out)
}

/// Replay options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplayOptions {
    /// Number of passes through the file (≥ 1).
    pub loops: u32,
    /// Plain gain (dB) applied to every channel.
    pub gain_db: f64,
    /// When set with a calibration, scale so that the recording's full scale comes out at
    /// this level (dBm) at each connector: `gain = target − full_scale_dbm(channel)`.
    pub target_full_scale_dbm: Option<f64>,
    /// Block length (samples per channel).
    pub block: usize,
}

impl Default for ReplayOptions {
    fn default() -> Self {
        ReplayOptions { loops: 1, gain_db: 0.0, target_full_scale_dbm: None, block: 20_000 }
    }
}

/// Play the recording `src` into `sink` (calling `finish` at the end); returns samples
/// written per channel.
pub fn replay(src: &Path, sink: &mut dyn SampleSink, opt: &ReplayOptions, cal: Option<&CalibrationFile>) -> Result<u64, String> {
    let mut r = SigmfReader::open(src)?;
    let gains: Vec<f64> = (0..r.channels)
        .map(|c| {
            let mut g = opt.gain_db;
            if let (Some(cal), Some(t)) = (cal, opt.target_full_scale_dbm) {
                if let Some(cc) = cal.channels.get(c) {
                    g += t - cc.full_scale_dbm;
                }
            }
            10f64.powf(g / 20.0)
        })
        .collect();
    let mut written = 0;
    for _ in 0..opt.loops.max(1) {
        r.seek(0)?;
        loop {
            let mut b = r.read(opt.block.max(1))?;
            if b[0].is_empty() {
                break;
            }
            for (c, ch) in b.iter_mut().enumerate() {
                for v in ch.iter_mut() {
                    *v *= gains[c];
                }
            }
            written += b[0].len() as u64;
            sink.write_block(&b)?;
        }
    }
    sink.finish()?;
    Ok(written)
}

/// Replay a recording through a configured device (non-blocking session).
pub fn replay_to_device(
    device: Box<dyn Device>,
    src: &Path,
    at: StartTime,
    opt: ReplayOptions,
    cal: Option<CalibrationFile>,
    fifo_blocks: usize,
) -> Result<TxSession<u64>, String> {
    let src = src.to_path_buf();
    TxSession::start(device, at, fifo_blocks, move |sink| replay(&src, sink, &opt, cal.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw::file::FileDevice;
    use crate::hw::DeviceConfig;
    use crate::io::read_sigmf;
    use std::f64::consts::PI;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kshana-eloran-replay-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_tone(base: &Path, fs: f64, n: usize, f: &[f64], fmt: Format, datetime: Option<&str>) {
        let chans: Vec<Vec<f64>> = f.iter().map(|fr| (0..n).map(|i| 0.5 * (2.0 * PI * fr * i as f64 / fs).sin()).collect()).collect();
        let mut w = SigmfWriter::create(base, fmt, f.len(), fs, json!({})).unwrap();
        w.write_block(&chans).unwrap();
        w.finish().unwrap();
        if let Some(d) = datetime {
            patch_meta(base, |m| m["captures"][0]["core:datetime"] = json!(d)).unwrap();
        }
    }

    #[test]
    fn inventory_lists_recordings() {
        let d = tmpdir("inv");
        write_tone(&d.join("a"), 1e6, 2000, &[1e5], Format::Ri16, Some("2026-11-18T00:00:00Z"));
        write_tone(&d.join("b"), 2e6, 1000, &[1e5, 1e5], Format::Rf32, None);
        std::fs::write(d.join("notes.txt"), "x").unwrap();
        let inv = inventory(&d).unwrap();
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].samples, 2000);
        assert_eq!(inv[0].channels, 1);
        assert!((inv[0].duration_s - 0.002).abs() < 1e-12);
        assert_eq!(inv[0].datetime_utc.as_deref(), Some("2026-11-18T00:00:00Z"));
        assert_eq!(inv[1].channels, 2);
        assert_eq!(inv[1].datatype, "rf32_le");
        assert_eq!(sigmf_sample_bytes("cf32_le"), 8);
        assert_eq!(sigmf_sample_bytes("ri16_le"), 2);
    }

    #[test]
    fn segment_extraction_keeps_samples_and_shifts_datetime() {
        let d = tmpdir("seg");
        write_tone(&d.join("src"), 1e6, 100_000, &[1.0e5, 0.9e5], Format::Rf32, Some("2026-11-18T23:59:59.95Z"));
        let n = extract_segment(&d.join("src"), &d.join("seg"), 0.06, 0.01).unwrap();
        assert_eq!(n, 10_000);
        let a = read_sigmf(&d.join("src"), None).unwrap();
        let b = read_sigmf(&d.join("seg"), None).unwrap();
        assert_eq!(&b.channels[1][..], &a.channels[1][60_000..70_000]);
        assert_eq!(b.meta["captures"][0]["core:datetime"], json!("2026-11-19T00:00:00.010000000Z"));
    }

    #[test]
    fn decimation_preserves_inband_and_rejects_aliases() {
        let d = tmpdir("dec");
        let fs = 2e6;
        // ch0: 100 kHz (in band), ch1: 900 kHz (would alias to 100 kHz at 1 MS/s... after /4 = 500 kS/s)
        write_tone(&d.join("src"), fs, 200_000, &[1.0e5, 0.9e6], Format::Rf32, None);
        let n = decimate(&d.join("src"), &d.join("dec"), 4, 0, Format::Rf32).unwrap();
        assert_eq!(n, 50_000);
        let r = read_sigmf(&d.join("dec"), None).unwrap();
        assert_eq!(r.fs_hz, 5e5);
        // time aligned: out[k] == input sample 4k
        let mut err: f64 = 0.0;
        for k in 1000..49_000 {
            let want = 0.5 * (2.0 * PI * 1e5 * (4 * k) as f64 / fs).sin();
            err = err.max((r.channels[0][k] - want).abs());
        }
        assert!(err < 2e-3, "in-band error {err}");
        let leak = r.channels[1][1000..49_000].iter().fold(0.0f64, |m, v| m.max(v.abs()));
        assert!(leak < 0.5 * 1e-3, "alias leak {leak}");
    }

    #[test]
    fn capture_from_file_device_writes_annotated_sigmf() {
        let d = tmpdir("cap");
        write_tone(&d.join("air"), 1e6, 30_000, &[1e5], Format::Ri16, None);
        let mut dev = FileDevice::rx(&d.join("air"));
        let mut cfg = DeviceConfig::tx(1e6, 0);
        cfg.rx_channels = 1;
        dev.configure(&cfg).unwrap();
        let info = CaptureInfo {
            datetime_utc: Some("2026-11-18T10:00:00Z".into()),
            reference_source: "external 10 MHz + 1PPS".into(),
            antenna: "loopback TX0 via 40 dB".into(),
            gain_db: vec![10.0],
            notes: String::new(),
        };
        let s = capture(&mut dev, StartTime::NextPps, 25_000, &d.join("cap"), Format::Rf32, &info).unwrap();
        assert_eq!(s.samples, 25_000);
        assert!(s.pps_aligned);
        let r = read_sigmf(&d.join("cap"), None).unwrap();
        assert_eq!(r.channels[0].len(), 25_000);
        let c = &r.meta["captures"][0];
        assert_eq!(c["core:datetime"], json!("2026-11-18T10:00:00Z"));
        assert_eq!(c["kshana:pps_aligned"], json!(true));
        assert_eq!(c["kshana:antenna"], json!("loopback TX0 via 40 dB"));
        assert_eq!(c["kshana:gain_db"], json!([10.0]));
    }

    #[test]
    fn replay_through_device_loops_with_gain() {
        let d = tmpdir("rep");
        write_tone(&d.join("rec"), 1e6, 12_345, &[1e5], Format::Rf32, None);
        let mut dev = FileDevice::tx(&d.join("out"), Format::Rf32);
        dev.configure(&DeviceConfig::tx(1e6, 1)).unwrap();
        let mut cal = crate::hw::calib::CalibrationFile {
            version: 1,
            date: "2026-11-18".into(),
            device: "file".into(),
            sample_rate_hz: 1e6,
            reference_point: Default::default(),
            instruments: vec![],
            channels: vec![],
            gnss_offset: None,
            notes: String::new(),
        };
        cal.channels.push(crate::hw::calib::ChannelCal { channel: 0, delay_ns: 0.0, delay_u_ns: 0.0, full_scale_dbm: 6.0, full_scale_u_db: 0.0, load_ohm: None, notes: String::new() });
        let opt = ReplayOptions { loops: 3, gain_db: 0.0, target_full_scale_dbm: Some(0.0), block: 4096 };
        let s = replay_to_device(Box::new(dev), &d.join("rec"), StartTime::Now, opt, Some(cal), 4).unwrap();
        let rep = s.wait().unwrap();
        assert_eq!(rep.produced, 3 * 12_345);
        let a = read_sigmf(&d.join("rec"), None).unwrap();
        let b = read_sigmf(&d.join("out"), None).unwrap();
        assert_eq!(b.channels[0].len(), 3 * 12_345);
        let g = 10f64.powf(-6.0 / 20.0);
        for k in [0usize, 1, 2] {
            for i in (0..12_345).step_by(97) {
                assert!((b.channels[0][k * 12_345 + i] - g * a.channels[0][i]).abs() < 1e-6);
            }
        }
    }
}
