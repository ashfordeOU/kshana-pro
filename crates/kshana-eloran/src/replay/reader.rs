// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Streaming SigMF reader (block by block, seekable) and metadata patching.

use crate::io::Format;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Reads a real interleaved SigMF pair (`ri16_le` / `rf32_le`) block by block.
pub struct SigmfReader {
    data: BufReader<File>,
    pub base: PathBuf,
    pub format: Format,
    pub fs_hz: f64,
    pub channels: usize,
    /// Total samples per channel in the file.
    pub total: u64,
    /// Next sample index to read.
    pub pos: u64,
    pub meta: serde_json::Value,
}

/// Read and parse `<base>.sigmf-meta`.
pub fn read_meta(base: &Path) -> Result<serde_json::Value, String> {
    let p = base.with_extension("sigmf-meta");
    serde_json::from_str(&std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// Read-modify-write `<base>.sigmf-meta`.
pub fn patch_meta(base: &Path, f: impl FnOnce(&mut serde_json::Value)) -> Result<(), String> {
    let mut m = read_meta(base)?;
    f(&mut m);
    let p = base.with_extension("sigmf-meta");
    std::fs::write(&p, serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?)
        .map_err(|e| format!("{}: {e}", p.display()))
}

impl SigmfReader {
    pub fn open(base: &Path) -> Result<SigmfReader, String> {
        let meta = read_meta(base)?;
        let g = &meta["global"];
        let format = Format::parse(g["core:datatype"].as_str().unwrap_or(""))?;
        let fs_hz = g["core:sample_rate"].as_f64().ok_or("missing core:sample_rate")?;
        let channels = g["core:num_channels"].as_u64().unwrap_or(1).max(1) as usize;
        let dp = base.with_extension("sigmf-data");
        let f = File::open(&dp).map_err(|e| format!("{}: {e}", dp.display()))?;
        let len = f.metadata().map_err(|e| e.to_string())?.len();
        let total = len / (channels * format.bytes()) as u64;
        Ok(SigmfReader { data: BufReader::new(f), base: base.to_path_buf(), format, fs_hz, channels, total, pos: 0, meta })
    }

    pub fn duration_s(&self) -> f64 {
        self.total as f64 / self.fs_hz
    }

    /// Move to sample `n` (clamped to the end).
    pub fn seek(&mut self, n: u64) -> Result<(), String> {
        let n = n.min(self.total);
        let frame = (self.channels * self.format.bytes()) as u64;
        self.data.seek(SeekFrom::Start(n * frame)).map_err(|e| e.to_string())?;
        self.pos = n;
        Ok(())
    }

    /// Read up to `n` samples per channel (empty at the end).
    pub fn read(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        let n = (n as u64).min(self.total - self.pos) as usize;
        let b = self.format.bytes();
        let frame = self.channels * b;
        let mut raw = vec![0u8; n * frame];
        self.data.read_exact(&mut raw).map_err(|e| e.to_string())?;
        let mut out = vec![Vec::with_capacity(n); self.channels];
        for i in 0..n {
            for (c, ch) in out.iter_mut().enumerate() {
                let o = i * frame + c * b;
                ch.push(match self.format {
                    Format::Ri16 => i16::from_le_bytes([raw[o], raw[o + 1]]) as f64 / 32767.0,
                    Format::Rf32 => f32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]) as f64,
                });
            }
        }
        self.pos += n as u64;
        if n == 0 {
            return Ok(vec![Vec::new(); self.channels]);
        }
        Ok(out)
    }
}
