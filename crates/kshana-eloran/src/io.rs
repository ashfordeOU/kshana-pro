// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Sample files: interleaved real samples (`ri16_le` or `rf32_le`) with a SigMF metadata
//! file, and readers for them (record and replay, software receiver input).

use crate::scene::SampleSink;
use serde_json::json;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// Sample format on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Interleaved real 16-bit little-endian, full scale ±32767 ↔ ±1.0.
    Ri16,
    /// Interleaved real 32-bit float little-endian.
    Rf32,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s {
            "ri16" | "ri16_le" => Ok(Format::Ri16),
            "rf32" | "rf32_le" => Ok(Format::Rf32),
            _ => Err(format!("unknown sample format {s} (ri16 or rf32)")),
        }
    }
    pub fn sigmf(self) -> &'static str {
        match self {
            Format::Ri16 => "ri16_le",
            Format::Rf32 => "rf32_le",
        }
    }
    pub fn bytes(self) -> usize {
        match self {
            Format::Ri16 => 2,
            Format::Rf32 => 4,
        }
    }
}

/// Writes `<base>.sigmf-data` (interleaved channels) and `<base>.sigmf-meta`.
pub struct SigmfWriter {
    data: BufWriter<File>,
    meta_path: PathBuf,
    format: Format,
    channels: usize,
    fs_hz: f64,
    pub samples: u64,
    pub clipped: u64,
    meta_extra: serde_json::Value,
}

impl SigmfWriter {
    pub fn create(
        base: &Path,
        format: Format,
        channels: usize,
        fs_hz: f64,
        extra: serde_json::Value,
    ) -> Result<SigmfWriter, String> {
        let data_path = base.with_extension("sigmf-data");
        let meta_path = base.with_extension("sigmf-meta");
        let f = File::create(&data_path).map_err(|e| format!("{}: {e}", data_path.display()))?;
        Ok(SigmfWriter {
            data: BufWriter::new(f),
            meta_path,
            format,
            channels,
            fs_hz,
            samples: 0,
            clipped: 0,
            meta_extra: extra,
        })
    }
}

impl SampleSink for SigmfWriter {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        if channels.len() != self.channels {
            return Err("channel count changed".into());
        }
        let n = channels.first().map(|c| c.len()).unwrap_or(0);
        let mut buf = Vec::with_capacity(n * self.channels * self.format.bytes());
        for i in 0..n {
            for c in channels {
                let v = c[i];
                match self.format {
                    Format::Ri16 => {
                        let x = (v * 32767.0).round();
                        if x.abs() > 32767.0 {
                            self.clipped += 1;
                        }
                        buf.extend_from_slice(&(x.clamp(-32767.0, 32767.0) as i16).to_le_bytes());
                    }
                    Format::Rf32 => buf.extend_from_slice(&(v as f32).to_le_bytes()),
                }
            }
        }
        self.data.write_all(&buf).map_err(|e| e.to_string())?;
        self.samples += n as u64;
        Ok(())
    }
    fn finish(&mut self) -> Result<(), String> {
        self.data.flush().map_err(|e| e.to_string())?;
        let meta = json!({
            "global": {
                "core:datatype": self.format.sigmf(),
                "core:sample_rate": self.fs_hz,
                "core:num_channels": self.channels,
                "core:version": "1.0.0",
                "core:description": "Kshana Pro eLoran simulator output (real samples, LF band, field strength scaled to full scale)",
                "core:recorder": "kshana-eloran",
                "kshana:eloran": self.meta_extra,
            },
            "captures": [{ "core:sample_start": 0, "core:frequency": 100000.0 }],
            "annotations": []
        });
        std::fs::write(
            &self.meta_path,
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .map_err(|e| format!("{}: {e}", self.meta_path.display()))
    }
}

/// A recording read back: per-channel samples, sample rate and the SigMF metadata.
#[derive(Clone, Debug)]
pub struct Recording {
    pub fs_hz: f64,
    pub channels: Vec<Vec<f64>>,
    pub meta: serde_json::Value,
}

/// Read a SigMF recording written by [`SigmfWriter`] (or any `ri16_le`/`rf32_le` real,
/// interleaved SigMF pair). `max_samples` limits the samples read per channel.
pub fn read_sigmf(base: &Path, max_samples: Option<u64>) -> Result<Recording, String> {
    let meta_path = base.with_extension("sigmf-meta");
    let data_path = base.with_extension("sigmf-data");
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&meta_path)
            .map_err(|e| format!("{}: {e}", meta_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", meta_path.display()))?;
    let g = &meta["global"];
    let fmt = Format::parse(g["core:datatype"].as_str().unwrap_or(""))?;
    let fs_hz = g["core:sample_rate"]
        .as_f64()
        .ok_or("missing core:sample_rate")?;
    let nch = g["core:num_channels"].as_u64().unwrap_or(1) as usize;
    let mut r = BufReader::new(
        File::open(&data_path).map_err(|e| format!("{}: {e}", data_path.display()))?,
    );
    let mut raw = Vec::new();
    r.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let frame = nch * fmt.bytes();
    let mut n = raw.len() / frame;
    if let Some(m) = max_samples {
        n = n.min(m as usize);
    }
    let mut channels = vec![Vec::with_capacity(n); nch];
    for i in 0..n {
        for (c, ch) in channels.iter_mut().enumerate() {
            let o = i * frame + c * fmt.bytes();
            let v = match fmt {
                Format::Ri16 => i16::from_le_bytes([raw[o], raw[o + 1]]) as f64 / 32767.0,
                Format::Rf32 => {
                    f32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]) as f64
                }
            };
            ch.push(v);
        }
    }
    Ok(Recording {
        fs_hz,
        channels,
        meta,
    })
}
