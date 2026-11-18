// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Unattended LF recording station (D1).
//!
//! A [`Recorder`] runs a loop over the receive side of any [`Device`]: it starts one
//! receive stream (on the next PPS edge when the station has a 1PPS reference), cuts the
//! stream into fixed-length **segments**, and for every segment
//!
//! 1. writes a SigMF pair (`segments/<site>_<UTC>_<seq>.sigmf-data/-meta`) through a
//!    `partial/` directory, so a crash never leaves a half-written file among the finished
//!    segments;
//! 2. timestamps it: `core:datetime` of the first sample = stream start (device time on the
//!    PPS, or the host clock when the device has no PPS/time) + samples before / fs;
//! 3. runs **chain detection**: the software receiver's acquisition ([`crate::rx`]) over
//!    the first `analyse_s` seconds against the candidate chains (by default every
//!    *complete* chain of the GRI database, [`crate::gridb`]), then tracking of the found
//!    stations for an SNR estimate; detected chains become SigMF annotations;
//! 4. runs **health checks**: clipping fraction, noise floor (10th percentile of the 1 ms
//!    window RMS, dBFS), dead input, missing PPS / time, reference unlocked, overflows
//!    (sample drops), disk quota; failed checks become SigMF annotations and state events;
//! 5. moves the segment into `segments/`, hashes its files (SHA-256), updates the
//!    **resumable state file** (`state.json`, written atomically) and the **upload
//!    manifest** (`upload_manifest.json`: every segment not yet uploaded with path, size and
//!    SHA-256 per file);
//! 6. applies **retention** (maximum segment count and/or byte quota; segments not yet
//!    uploaded are kept unless `keep_unuploaded = false`) and, every `every_segments`
//!    segments, runs the optional **upload command hook** (argv with `{manifest}` and
//!    `{dir}` placeholders). Exit status 0 marks the segments listed in that manifest as
//!    uploaded. No network upload is implemented here.
//!
//! On start, [`Recorder::open`] loads `state.json` (sequence numbers continue, earlier
//! segments keep their upload status), removes leftovers in `partial/` (recorded as an
//! `incomplete_segment_discarded` event) and drops state entries whose files are gone.
//!
//! # Configuration (TOML)
//!
//! ```toml
//! site_id = "lfs-01"
//! output_dir = "/data/lfs"
//! sample_rate_hz = 400000
//! channels = 2
//! segment_s = 60.0
//! format = "ri16"
//! gain_db = [20.0, 20.0]
//! position = { lat_deg = 52.37, lon_deg = 4.89, height_m = 2.0 }
//! antenna = { type = "h_loop_pair", orientation_deg = [0.0, 90.0], model = "…" }
//! [reference]
//! clock = "external"          # 10 MHz
//! pps = "external"            # 1PPS
//! device_time = "unix_utc"    # device time set to POSIX UTC on the PPS
//! description = "GNSSDO 10 MHz + 1PPS"
//! [retention]
//! max_segments = 1440
//! quota_bytes = 50_000_000_000
//! [detection]
//! analyse_s = 2.0
//! [upload]
//! command = ["/usr/local/bin/push-lfs", "{manifest}"]
//! ```
//!
//! # Needs (field deployment)
//!
//! * Run against a real SDR at a site: PPS start alignment, device-time scale, overflow
//!   reporting and reference-lock reporting are as each driver implements them.
//! * Free disk space is **not** queried (no OS-specific dependency); the byte quota and
//!   write errors are what the health check sees. Set the quota below the volume size.
//! * The health thresholds are placeholders to be set from the site's own noise survey.
//! * Detection uses the receiver's batch acquisition; weak or distant chains may need a
//!   longer `analyse_s` and a lower threshold. Terminated chains in the database are
//!   candidates by default (`active_only = true` restricts to chains active on the date).

use crate::chain::Chain;
use crate::geo::Geo;
use crate::gridb::GriDb;
use crate::hw::{Device, DeviceConfig, PpsSource, RefSource, StartTime};
use crate::io::{Format, SigmfWriter};
use crate::replay::reader::patch_meta;
use crate::rx::{self, Antenna, LoopMode, RxConfig, RxInput};
use crate::scene::SampleSink;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    abs_from_unix_ns, iso_from_unix_ns, sha256_file, unix_ns_from_iso, write_atomic,
};

/// Format tag of `state.json`.
pub const STATE_FORMAT: &str = "kshana-eloran-lf-station-state";
/// Format tag of `upload_manifest.json`.
pub const MANIFEST_FORMAT: &str = "kshana-eloran-lf-upload-manifest";
/// Read block (samples per channel).
const BLOCK: usize = 16_384;

/// Site position (WGS-84).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SitePosition {
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub height_m: f64,
}

/// Antenna kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntennaKind {
    /// Vertical E-field (whip/monopole), channel 0.
    #[default]
    EField,
    /// Two crossed H-field loops on channels 0 and 1.
    HLoopPair,
    /// One H-field loop.
    HLoop,
    Other,
}

/// Antenna description.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AntennaInfo {
    #[serde(rename = "type", default)]
    pub kind: AntennaKind,
    /// Axis azimuth of each loop / channel (degrees from true north), when meaningful.
    #[serde(default)]
    pub orientation_deg: Vec<f64>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub notes: String,
}

/// Scale of the device time returned by [`Device::start_rx`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceTimeScale {
    /// POSIX UTC nanoseconds (the device time was set from the PPS to UTC).
    UnixUtc,
    /// Unknown epoch: timestamps come from the host clock (flagged).
    #[default]
    Unknown,
}

/// Frequency/time reference of the station.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReferenceInfo {
    #[serde(default)]
    pub clock: RefSource,
    #[serde(default)]
    pub pps: PpsSource,
    #[serde(default)]
    pub device_time: DeviceTimeScale,
    #[serde(default)]
    pub description: String,
}

/// Rotation and retention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Retention {
    #[serde(default)]
    pub max_segments: Option<u64>,
    /// Byte quota for the finished segments (data + metadata).
    #[serde(default)]
    pub quota_bytes: Option<u64>,
    /// Never delete a segment that has not been uploaded (default true).
    #[serde(default = "yes")]
    pub keep_unuploaded: bool,
}

impl Default for Retention {
    fn default() -> Self {
        Retention {
            max_segments: None,
            quota_bytes: None,
            keep_unuploaded: true,
        }
    }
}

/// Automatic chain detection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DetectionConfig {
    #[serde(default = "yes")]
    pub enabled: bool,
    /// GRI database chain ids to try; empty = every complete chain.
    #[serde(default)]
    pub chains: Vec<String>,
    /// Only chains active on the segment's date.
    #[serde(default)]
    pub active_only: bool,
    /// Seconds from the segment start analysed (≥ 4 GRI recommended).
    #[serde(default = "d_analyse")]
    pub analyse_s: f64,
    /// Acquisition threshold (peak to median, dB).
    #[serde(default = "d_thr")]
    pub acq_threshold_db: f64,
    /// Stations found for a chain to count as detected.
    #[serde(default = "d_min_st")]
    pub min_stations: usize,
    /// Track found stations for an SNR estimate.
    #[serde(default = "yes")]
    pub track: bool,
    /// With tracking, a found station counts only if its tracked SNR (dB in 20 kHz) is at
    /// least this. Guards against false acquisitions on another chain's strong pulses,
    /// which short records do not average out (cross-rate leakage).
    #[serde(default)]
    pub min_snr_db: f64,
    /// Use the site position to narrow the acquisition search.
    #[serde(default = "yes")]
    pub use_position: bool,
    /// Optional user database file merged over the embedded one (GriDb::merge_json).
    #[serde(default)]
    pub user_db: Option<PathBuf>,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        DetectionConfig {
            enabled: true,
            chains: vec![],
            active_only: false,
            analyse_s: d_analyse(),
            acq_threshold_db: d_thr(),
            min_stations: d_min_st(),
            track: true,
            min_snr_db: 0.0,
            use_position: true,
            user_db: None,
        }
    }
}

/// Health thresholds (placeholders; set them from a site survey).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HealthConfig {
    /// |x| at or above which a sample counts as clipped.
    #[serde(default = "d_clip")]
    pub clip_level: f64,
    #[serde(default = "d_clip_frac")]
    pub max_clip_fraction: f64,
    /// Noise floor above this (dBFS) is flagged `noise_floor_high`.
    #[serde(default = "d_nf_max")]
    pub noise_floor_max_dbfs: f64,
    /// Noise floor below this (dBFS) is flagged `input_dead`.
    #[serde(default = "d_nf_min")]
    pub noise_floor_min_dbfs: f64,
    /// Flag `missing_pps` when the stream did not start on a PPS edge.
    #[serde(default = "yes")]
    pub require_pps: bool,
}

impl Default for HealthConfig {
    fn default() -> Self {
        HealthConfig {
            clip_level: d_clip(),
            max_clip_fraction: d_clip_frac(),
            noise_floor_max_dbfs: d_nf_max(),
            noise_floor_min_dbfs: d_nf_min(),
            require_pps: true,
        }
    }
}

/// Upload hook.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UploadConfig {
    /// argv; `{manifest}` and `{dir}` are replaced. Exit 0 = uploaded.
    #[serde(default)]
    pub command: Option<Vec<String>>,
    /// Run the hook every this many segments (default 1).
    #[serde(default = "d_one")]
    pub every_segments: u64,
}

impl Default for UploadConfig {
    fn default() -> Self {
        UploadConfig {
            command: None,
            every_segments: 1,
        }
    }
}

/// Station configuration (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationConfig {
    pub site_id: String,
    pub output_dir: PathBuf,
    pub position: SitePosition,
    #[serde(default)]
    pub antenna: AntennaInfo,
    #[serde(default)]
    pub gain_db: Vec<f64>,
    pub sample_rate_hz: f64,
    #[serde(default = "d_ch")]
    pub channels: usize,
    pub segment_s: f64,
    #[serde(default = "d_fmt")]
    pub format: String,
    #[serde(default)]
    pub reference: ReferenceInfo,
    #[serde(default)]
    pub retention: Retention,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default)]
    pub health: HealthConfig,
    #[serde(default)]
    pub upload: UploadConfig,
}

fn yes() -> bool {
    true
}
fn d_analyse() -> f64 {
    2.0
}
fn d_thr() -> f64 {
    10.0
}
fn d_min_st() -> usize {
    2
}
fn d_clip() -> f64 {
    0.999
}
fn d_clip_frac() -> f64 {
    1e-4
}
fn d_nf_max() -> f64 {
    -10.0
}
fn d_nf_min() -> f64 {
    -100.0
}
fn d_one() -> u64 {
    1
}
fn d_ch() -> usize {
    1
}
fn d_fmt() -> String {
    "ri16".into()
}

impl StationConfig {
    pub fn from_toml(src: &str) -> Result<StationConfig, String> {
        let c: StationConfig = toml::from_str(src).map_err(|e| format!("station config: {e}"))?;
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.site_id.is_empty()
            || !self
                .site_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("site_id must be non-empty [A-Za-z0-9_-]".into());
        }
        if !(self.sample_rate_hz > 0.0) {
            return Err("sample_rate_hz must be positive".into());
        }
        if self.channels == 0 {
            return Err("channels must be ≥ 1".into());
        }
        if !(self.segment_s > 0.0) {
            return Err("segment_s must be positive".into());
        }
        Format::parse(&self.format)?;
        if self.upload.every_segments == 0 {
            return Err("upload.every_segments must be ≥ 1".into());
        }
        Ok(())
    }
    /// Samples per channel per segment.
    pub fn segment_samples(&self) -> u64 {
        (self.segment_s * self.sample_rate_hz).round().max(1.0) as u64
    }
    /// The device configuration this station needs.
    pub fn device_config(&self) -> DeviceConfig {
        DeviceConfig {
            sample_rate_hz: self.sample_rate_hz,
            tx_channels: 0,
            rx_channels: self.channels,
            tx_gain_db: vec![],
            rx_gain_db: self.gain_db.clone(),
            center_freq_hz: 0.0,
            clock_source: self.reference.clock,
            time_source: self.reference.pps,
        }
    }
}

// ---------------------------------------------------------------------------------------
// Detection and health results

/// One station's acquisition result in a detection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationDetection {
    pub id: String,
    pub found: bool,
    pub metric_db: f64,
    /// Mean tracked SNR (dB in 20 kHz) when tracked.
    pub snr_db: Option<f64>,
}

/// A detected chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChainDetection {
    /// Database id (or chain name for user-supplied chains).
    pub id: String,
    pub gri_us: u32,
    pub name: String,
    pub detected: bool,
    pub stations: Vec<StationDetection>,
}

/// Health check outcome of one segment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Health {
    pub clipped_samples: u64,
    pub clip_fraction: f64,
    /// 10th percentile of 1 ms window RMS (dBFS, 0 dBFS = RMS 1.0).
    pub noise_floor_dbfs: f64,
    pub rms_dbfs: f64,
    pub peak_abs: f64,
    pub overflows: u64,
    pub pps_aligned: bool,
    pub ref_locked: Option<bool>,
    /// Failed checks (empty = healthy): clipping, noise_floor_high, input_dead,
    /// missing_pps, reference_unlocked, overflow, disk_quota, truncated, detection_error.
    pub flags: Vec<String>,
}

/// Running statistics while a segment streams in.
struct HealthAcc {
    clip_level: f64,
    clipped: u64,
    n: u64,
    sumsq: f64,
    peak: f64,
    win: usize,
    win_acc: f64,
    win_n: usize,
    win_rms: Vec<f64>,
}

impl HealthAcc {
    fn new(fs: f64, clip_level: f64) -> HealthAcc {
        HealthAcc {
            clip_level,
            clipped: 0,
            n: 0,
            sumsq: 0.0,
            peak: 0.0,
            win: ((fs * 1e-3).round() as usize).max(1),
            win_acc: 0.0,
            win_n: 0,
            win_rms: Vec::new(),
        }
    }
    fn push(&mut self, block: &[Vec<f64>]) {
        let n = block.first().map(|c| c.len()).unwrap_or(0);
        for i in 0..n {
            let mut p = 0.0;
            for c in block {
                let v = c[i];
                if v.abs() >= self.clip_level {
                    self.clipped += 1;
                }
                self.peak = self.peak.max(v.abs());
                p += v * v;
            }
            p /= block.len() as f64;
            self.sumsq += p;
            self.n += 1;
            self.win_acc += p;
            self.win_n += 1;
            if self.win_n == self.win {
                self.win_rms.push((self.win_acc / self.win as f64).sqrt());
                self.win_acc = 0.0;
                self.win_n = 0;
            }
        }
    }
    fn finish(mut self, channels: usize) -> Health {
        let db = |x: f64| 20.0 * x.max(1e-12).log10();
        self.win_rms.sort_by(|a, b| a.total_cmp(b));
        let p10 = if self.win_rms.is_empty() {
            (self.sumsq / self.n.max(1) as f64).sqrt()
        } else {
            self.win_rms[self.win_rms.len() / 10]
        };
        Health {
            clipped_samples: self.clipped,
            clip_fraction: self.clipped as f64 / (self.n.max(1) * channels as u64) as f64,
            noise_floor_dbfs: db(p10),
            rms_dbfs: db((self.sumsq / self.n.max(1) as f64).sqrt()),
            peak_abs: self.peak,
            ..Default::default()
        }
    }
}

/// Candidate chains for detection: (id, chain).
pub fn detection_candidates(cfg: &DetectionConfig, date: &str) -> Result<Vec<(String, Chain)>, String> {
    let mut db = GriDb::embedded()?;
    if let Some(p) = &cfg.user_db {
        let src = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        db.merge_json(&src)?;
    }
    let mut out = Vec::new();
    if cfg.chains.is_empty() {
        for c in &db.chains {
            if !c.is_complete() || (cfg.active_only && !c.active_on(date)) {
                continue;
            }
            if let Ok(ch) = c.to_chain_strict() {
                out.push((c.id.clone(), ch));
            }
        }
    } else {
        for id in &cfg.chains {
            let c = db
                .get(id)
                .ok_or_else(|| format!("detection: unknown GRI database chain {id:?}"))?;
            out.push((c.id.clone(), c.to_chain_strict()?));
        }
    }
    Ok(out)
}

/// Run chain detection on samples (`channels[c][n]`, first sample at `start_unix_ns`).
pub fn detect_chains(
    channels: &[Vec<f64>],
    fs_hz: f64,
    start_unix_ns: i128,
    candidates: &[(String, Chain)],
    cfg: &DetectionConfig,
    antenna: AntennaKind,
    position: Option<Geo>,
) -> Result<Vec<ChainDetection>, String> {
    if candidates.is_empty() {
        return Ok(vec![]);
    }
    let chains: Vec<Chain> = candidates.iter().map(|c| c.1.clone()).collect();
    let input = RxInput::new(channels.to_vec(), fs_hz, abs_from_unix_ns(start_unix_ns));
    let antenna = if antenna == AntennaKind::HLoopPair && channels.len() >= 2 {
        Antenna::CrossedLoop {
            a: 0,
            b: 1,
            mode: LoopMode::BeamSteer,
        }
    } else {
        Antenna::EField { channel: 0 }
    };
    let rcfg = RxConfig {
        antenna,
        acq_threshold_db: cfg.acq_threshold_db,
        approx_position: position,
        ..Default::default()
    };
    let rx = rx::Receiver::new(&input, &chains, rcfg)?;
    let acq = rx.acquire();
    let meas = if cfg.track && acq.iter().any(|a| a.found) {
        rx.track(&acq).1
    } else {
        vec![]
    };
    let mut out = Vec::new();
    for (ci, (id, ch)) in candidates.iter().enumerate() {
        let stations: Vec<StationDetection> = ch
            .stations
            .iter()
            .enumerate()
            .map(|(si, st)| {
                let a = acq
                    .iter()
                    .find(|a| a.chain_index == ci && a.station_index == si);
                let snrs: Vec<f64> = meas
                    .iter()
                    .filter(|m| m.chain_index == ci && m.station_index == si)
                    .map(|m| m.snr_db)
                    .collect();
                let snr_db =
                    (!snrs.is_empty()).then(|| snrs.iter().sum::<f64>() / snrs.len() as f64);
                let snr_ok = !cfg.track || snr_db.is_some_and(|v| v >= cfg.min_snr_db);
                StationDetection {
                    id: st.id.clone(),
                    found: a.is_some_and(|a| a.found) && snr_ok,
                    metric_db: a.map(|a| a.metric_db).unwrap_or(f64::NAN),
                    snr_db,
                }
            })
            .collect();
        let found = stations.iter().filter(|s| s.found).count();
        out.push(ChainDetection {
            id: id.clone(),
            gri_us: ch.gri_us,
            name: ch.name.clone(),
            detected: found >= cfg.min_stations.max(1),
            stations,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// State, manifest

/// One file of a segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Path relative to the station directory.
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// A finished segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegmentRecord {
    pub seq: u64,
    /// Recorder session (one per [`Recorder::run`] stream).
    pub session: u64,
    pub name: String,
    pub start_utc: String,
    pub start_unix_ns: i64,
    /// "device_pps" or "host_clock".
    pub timestamp_source: String,
    pub samples: u64,
    pub fs_hz: f64,
    pub channels: usize,
    pub files: Vec<FileEntry>,
    pub detected: Vec<ChainDetection>,
    pub health: Health,
    pub uploaded: bool,
}

impl SegmentRecord {
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|f| f.bytes).sum()
    }
}

/// An event in the station log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationEvent {
    pub utc: String,
    pub kind: String,
    pub detail: String,
}

/// The resumable state (`state.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationState {
    pub format: String,
    pub version: u32,
    pub site_id: String,
    pub next_seq: u64,
    pub sessions: u64,
    pub total_samples: u64,
    /// Segments currently on disk, oldest first.
    pub segments: Vec<SegmentRecord>,
    /// Segments deleted by retention (seq, name, uploaded).
    pub deleted: Vec<(u64, String, bool)>,
    /// Most recent events (bounded).
    pub events: Vec<StationEvent>,
}

impl StationState {
    fn new(site: &str) -> StationState {
        StationState {
            format: STATE_FORMAT.into(),
            version: 1,
            site_id: site.into(),
            next_seq: 0,
            sessions: 0,
            total_samples: 0,
            segments: vec![],
            deleted: vec![],
            events: vec![],
        }
    }
}

/// The upload manifest (`upload_manifest.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UploadManifest {
    pub format: String,
    pub version: u32,
    pub site_id: String,
    pub generated_utc: String,
    pub position: SitePosition,
    pub antenna: AntennaInfo,
    pub reference: ReferenceInfo,
    pub segments: Vec<ManifestSegment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestSegment {
    pub seq: u64,
    pub name: String,
    pub start_utc: String,
    pub duration_s: f64,
    pub files: Vec<FileEntry>,
    pub detected_chains: Vec<String>,
    pub health_flags: Vec<String>,
}

/// Verify the files of a manifest against the disk (SHA-256 and size). Returns the paths
/// that differ or are missing.
pub fn verify_manifest(dir: &Path, m: &UploadManifest) -> Vec<String> {
    let mut bad = Vec::new();
    for s in &m.segments {
        for f in &s.files {
            let p = dir.join(&f.path);
            let ok = std::fs::metadata(&p).map(|md| md.len() == f.bytes).unwrap_or(false)
                && sha256_file(&p).map(|h| h == f.sha256).unwrap_or(false);
            if !ok {
                bad.push(f.path.clone());
            }
        }
    }
    bad
}

/// Summary of one [`Recorder::run`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    pub session: u64,
    pub segments_written: u64,
    pub samples: u64,
    pub deleted: u64,
    pub uploads_ok: u64,
    pub uploads_failed: u64,
    pub end_of_stream: bool,
}

// ---------------------------------------------------------------------------------------
// Recorder

/// The unattended recorder.
pub struct Recorder {
    pub cfg: StationConfig,
    pub dir: PathBuf,
    pub state: StationState,
    candidates: Option<Vec<(String, Chain)>>,
    /// Host clock source (POSIX ns); replaceable for tests.
    pub host_clock: fn() -> i128,
}

fn system_now_ns() -> i128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}

const MAX_EVENTS: usize = 500;

impl Recorder {
    /// Open (or resume) the station directory.
    pub fn open(cfg: StationConfig) -> Result<Recorder, String> {
        cfg.validate()?;
        let dir = cfg.output_dir.clone();
        for d in [dir.clone(), dir.join("segments"), dir.join("partial")] {
            std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let sp = dir.join("state.json");
        let mut state = if sp.exists() {
            let s: StationState = serde_json::from_str(
                &std::fs::read_to_string(&sp).map_err(|e| format!("{}: {e}", sp.display()))?,
            )
            .map_err(|e| format!("{}: {e}", sp.display()))?;
            if s.format != STATE_FORMAT || s.site_id != cfg.site_id {
                return Err(format!(
                    "{}: not a state file of site {:?}",
                    sp.display(),
                    cfg.site_id
                ));
            }
            s
        } else {
            StationState::new(&cfg.site_id)
        };
        let mut rec = Recorder {
            cfg,
            dir,
            state: StationState::new(""),
            candidates: None,
            host_clock: system_now_ns,
        };
        // leftovers of an interrupted segment
        let partial = rec.dir.join("partial");
        let mut leftovers = Vec::new();
        for e in std::fs::read_dir(&partial).map_err(|e| e.to_string())?.flatten() {
            leftovers.push(e.file_name().to_string_lossy().into_owned());
            let _ = std::fs::remove_file(e.path());
        }
        // state entries whose files disappeared
        let before = state.segments.len();
        let dir = rec.dir.clone();
        state
            .segments
            .retain(|s| s.files.iter().all(|f| dir.join(&f.path).exists()));
        let lost = before - state.segments.len();
        rec.state = state;
        if !leftovers.is_empty() {
            rec.event(
                "incomplete_segment_discarded",
                &format!("removed from partial/: {}", leftovers.join(", ")),
            );
        }
        if lost > 0 {
            rec.event(
                "segment_files_missing",
                &format!("{lost} segment(s) dropped from the state"),
            );
        }
        rec.save_state()?;
        Ok(rec)
    }

    fn now_iso(&self) -> String {
        iso_from_unix_ns((self.host_clock)())
    }

    fn event(&mut self, kind: &str, detail: &str) {
        let e = StationEvent {
            utc: self.now_iso(),
            kind: kind.into(),
            detail: detail.into(),
        };
        self.state.events.push(e);
        if self.state.events.len() > MAX_EVENTS {
            let k = self.state.events.len() - MAX_EVENTS;
            self.state.events.drain(..k);
        }
    }

    pub fn save_state(&self) -> Result<(), String> {
        write_atomic(
            &self.dir.join("state.json"),
            serde_json::to_string_pretty(&self.state)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )
    }

    /// The manifest of every segment not yet uploaded.
    pub fn manifest(&self) -> UploadManifest {
        UploadManifest {
            format: MANIFEST_FORMAT.into(),
            version: 1,
            site_id: self.cfg.site_id.clone(),
            generated_utc: self.now_iso(),
            position: self.cfg.position,
            antenna: self.cfg.antenna.clone(),
            reference: self.cfg.reference.clone(),
            segments: self
                .state
                .segments
                .iter()
                .filter(|s| !s.uploaded)
                .map(|s| ManifestSegment {
                    seq: s.seq,
                    name: s.name.clone(),
                    start_utc: s.start_utc.clone(),
                    duration_s: s.samples as f64 / s.fs_hz,
                    files: s.files.clone(),
                    detected_chains: s
                        .detected
                        .iter()
                        .filter(|c| c.detected)
                        .map(|c| c.id.clone())
                        .collect(),
                    health_flags: s.health.flags.clone(),
                })
                .collect(),
        }
    }

    /// Write `upload_manifest.json`; returns its path.
    pub fn write_manifest(&self) -> Result<(PathBuf, UploadManifest), String> {
        let m = self.manifest();
        let p = self.dir.join("upload_manifest.json");
        write_atomic(
            &p,
            serde_json::to_string_pretty(&m)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        )?;
        Ok((p, m))
    }

    fn candidates(&mut self, date: &str) -> Result<Vec<(String, Chain)>, String> {
        if self.candidates.is_none() {
            self.candidates = Some(detection_candidates(&self.cfg.detection, date)?);
        }
        Ok(self.candidates.clone().unwrap_or_default())
    }

    /// Configure the device for this station.
    pub fn configure_device(&self, device: &mut dyn Device) -> Result<(), String> {
        let c = self.cfg.device_config();
        c.validate()?;
        device.configure(&c)
    }

    /// Record until `stop` is set, the device ends its stream, or `max_segments` segments
    /// were written in this run. The device is configured and its receive stream started
    /// here (on the next PPS edge when the station has an external PPS).
    pub fn run(
        &mut self,
        device: &mut dyn Device,
        max_segments: Option<u64>,
        stop: &AtomicBool,
    ) -> Result<RunReport, String> {
        self.configure_device(device)?;
        let fs = self.cfg.sample_rate_hz;
        let at = if self.cfg.reference.pps == PpsSource::External {
            StartTime::NextPps
        } else {
            StartTime::Now
        };
        let host_start = (self.host_clock)();
        let t_dev = device.start_rx(at)?;
        let st0 = device.status();
        let (stream_start_ns, ts_source) = match (self.cfg.reference.device_time, t_dev) {
            (DeviceTimeScale::UnixUtc, Some(t)) => (t as i128, "device_pps"),
            _ => (host_start, "host_clock"),
        };
        self.state.sessions += 1;
        let session = self.state.sessions;
        self.event(
            "session_start",
            &format!(
                "device {} at {} ({ts_source}), pps_aligned {}",
                device.name(),
                iso_from_unix_ns(stream_start_ns),
                st0.pps_aligned
            ),
        );
        let mut report = RunReport {
            session,
            ..Default::default()
        };
        let seg_n = self.cfg.segment_samples();
        let mut stream_pos: u64 = 0;
        let mut overflows_prev = st0.overflows;
        let res = (|| -> Result<(), String> {
            loop {
                if stop.load(Ordering::Relaxed) || max_segments.is_some_and(|m| report.segments_written >= m) {
                    break;
                }
                let seg_start_ns = stream_start_ns + (stream_pos as i128 * 1_000_000_000) / fs as i128;
                let out = self.record_segment(device, seg_n, seg_start_ns, ts_source, session, &mut overflows_prev, stop)?;
                match out {
                    None => {
                        report.end_of_stream = true;
                        break;
                    }
                    Some((samples, eos, del, up_ok, up_fail)) => {
                        stream_pos += samples;
                        report.samples += samples;
                        report.segments_written += 1;
                        report.deleted += del;
                        report.uploads_ok += up_ok;
                        report.uploads_failed += up_fail;
                        if eos {
                            report.end_of_stream = true;
                            break;
                        }
                    }
                }
            }
            Ok(())
        })();
        let stop_res = device.stop_rx();
        self.event(
            "session_end",
            &format!(
                "{} segment(s), {} samples{}",
                report.segments_written,
                report.samples,
                match &res {
                    Err(e) => format!(", error: {e}"),
                    Ok(()) => String::new(),
                }
            ),
        );
        self.save_state()?;
        res?;
        stop_res?;
        Ok(report)
    }

    /// Record one segment. `None` when the stream ended before any sample.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn record_segment(
        &mut self,
        device: &mut dyn Device,
        seg_n: u64,
        start_ns: i128,
        ts_source: &str,
        session: u64,
        overflows_prev: &mut u64,
        stop: &AtomicBool,
    ) -> Result<Option<(u64, bool, u64, u64, u64)>, String> {
        let fs = self.cfg.sample_rate_hz;
        let fmt = Format::parse(&self.cfg.format)?;
        let seq = self.state.next_seq;
        let iso = iso_from_unix_ns(start_ns);
        let compact: String = iso
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == 'T')
            .take(15)
            .collect();
        let name = format!("{}_{compact}Z_{seq:06}", self.cfg.site_id);
        let pbase = self.dir.join("partial").join(&name);
        let mut writer: Option<SigmfWriter> = None;
        let mut acc = HealthAcc::new(fs, self.cfg.health.clip_level);
        let n_analyse = if self.cfg.detection.enabled {
            ((self.cfg.detection.analyse_s * fs).round() as u64).min(seg_n) as usize
        } else {
            0
        };
        let mut keep: Vec<Vec<f64>> = Vec::new();
        let mut got = 0u64;
        let mut eos = false;
        while got < seg_n {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let b = device.read_rx(BLOCK.min((seg_n - got) as usize))?;
            let k = b.first().map(|c| c.len()).unwrap_or(0);
            if k == 0 {
                eos = true;
                break;
            }
            if writer.is_none() {
                writer = Some(SigmfWriter::create(
                    &pbase,
                    fmt,
                    b.len(),
                    fs,
                    json!({ "site_id": self.cfg.site_id, "capture_device": device.name() }),
                )?);
                keep = vec![Vec::with_capacity(n_analyse); b.len()];
            }
            writer
                .as_mut()
                .expect("created")
                .write_block(&b)
                .map_err(|e| format!("disk: {e}"))?;
            acc.push(&b);
            for (c, ch) in b.iter().enumerate() {
                let room = n_analyse.saturating_sub(keep[c].len());
                keep[c].extend_from_slice(&ch[..room.min(ch.len())]);
            }
            got += k as u64;
        }
        let Some(mut w) = writer else {
            return Ok(None);
        };
        w.finish().map_err(|e| format!("disk: {e}"))?;
        let nch = keep.len();
        let st = device.status();
        let mut health = acc.finish(nch);
        health.overflows = st.overflows.saturating_sub(*overflows_prev);
        *overflows_prev = st.overflows;
        health.pps_aligned = st.pps_aligned;
        health.ref_locked = st.ref_locked;
        let hc = &self.cfg.health;
        let mut flags = Vec::new();
        if health.clip_fraction > hc.max_clip_fraction {
            flags.push("clipping".to_string());
        }
        if health.noise_floor_dbfs > hc.noise_floor_max_dbfs {
            flags.push("noise_floor_high".into());
        }
        if health.noise_floor_dbfs < hc.noise_floor_min_dbfs {
            flags.push("input_dead".into());
        }
        if hc.require_pps && (!st.pps_aligned || ts_source != "device_pps") {
            flags.push("missing_pps".into());
        }
        if st.ref_locked == Some(false) {
            flags.push("reference_unlocked".into());
        }
        if health.overflows > 0 {
            flags.push("overflow".into());
        }
        if got < seg_n {
            flags.push("truncated".into());
        }
        // chain detection
        let mut detected = Vec::new();
        if self.cfg.detection.enabled && n_analyse > 0 {
            let date: String = iso.chars().take(10).collect();
            let res = self.candidates(&date).and_then(|cands| {
                let pos = self
                    .cfg
                    .detection
                    .use_position
                    .then(|| Geo::new(self.cfg.position.lat_deg, self.cfg.position.lon_deg));
                detect_chains(
                    &keep,
                    fs,
                    start_ns,
                    &cands,
                    &self.cfg.detection,
                    self.cfg.antenna.kind,
                    pos,
                )
            });
            match res {
                Ok(d) => detected = d.into_iter().filter(|c| c.detected).collect(),
                Err(e) => {
                    flags.push("detection_error".into());
                    self.event("detection_error", &format!("{name}: {e}"));
                }
            }
        }
        health.flags = flags;
        // metadata: captures + annotations
        let cfg = &self.cfg;
        let n_an = n_analyse.min(got as usize) as u64;
        patch_meta(&pbase, |m| {
            m["global"]["core:description"] =
                json!(format!("Kshana Pro LF recording station {} segment {seq}", cfg.site_id));
            m["global"]["core:recorder"] = json!("kshana-eloran field::station");
            m["global"]["core:geolocation"] = json!({
                "type": "Point",
                "coordinates": [cfg.position.lon_deg, cfg.position.lat_deg, cfg.position.height_m]
            });
            m["global"]["kshana:site_id"] = json!(cfg.site_id);
            m["global"]["kshana:antenna"] = json!(cfg.antenna);
            m["global"]["kshana:reference"] = json!(cfg.reference);
            let c = &mut m["captures"][0];
            c["core:datetime"] = json!(iso);
            c["kshana:timestamp_source"] = json!(ts_source);
            c["kshana:pps_aligned"] = json!(st.pps_aligned);
            c["kshana:gain_db"] = json!(cfg.gain_db);
            c["kshana:segment_seq"] = json!(seq);
            c["kshana:session"] = json!(session);
            let mut ann = Vec::new();
            for d in &detected {
                ann.push(json!({
                    "core:sample_start": 0,
                    "core:sample_count": n_an,
                    "core:label": format!("chain {} detected", d.gri_us / 10),
                    "core:comment": format!("{} ({})", d.name, d.id),
                    "kshana:chain_id": d.id,
                    "kshana:gri_us": d.gri_us,
                    "kshana:stations": d.stations,
                }));
            }
            if !health.flags.is_empty() {
                ann.push(json!({
                    "core:sample_start": 0,
                    "core:sample_count": got,
                    "core:label": "health",
                    "core:comment": health.flags.join(","),
                    "kshana:health": health,
                }));
            }
            m["annotations"] = json!(ann);
        })?;
        // finish: move into segments/
        let mut files = Vec::new();
        for ext in ["sigmf-data", "sigmf-meta"] {
            let from = pbase.with_extension(ext);
            let rel = format!("segments/{name}.{ext}");
            let to = self.dir.join(&rel);
            std::fs::rename(&from, &to).map_err(|e| format!("disk: {}: {e}", to.display()))?;
            files.push(FileEntry {
                bytes: std::fs::metadata(&to).map_err(|e| e.to_string())?.len(),
                sha256: sha256_file(&to)?,
                path: rel,
            });
        }
        let rec = SegmentRecord {
            seq,
            session,
            name: name.clone(),
            start_utc: iso,
            start_unix_ns: start_ns as i64,
            timestamp_source: ts_source.into(),
            samples: got,
            fs_hz: fs,
            channels: nch,
            files,
            detected,
            health,
            uploaded: false,
        };
        if !rec.health.flags.is_empty() {
            self.event("health", &format!("{name}: {}", rec.health.flags.join(",")));
        }
        self.state.segments.push(rec);
        self.state.next_seq += 1;
        self.state.total_samples += got;
        let deleted = self.apply_retention()?;
        let (mut up_ok, mut up_fail) = (0, 0);
        self.write_manifest()?;
        if self.cfg.upload.command.is_some()
            && self.state.next_seq.is_multiple_of(self.cfg.upload.every_segments)
        {
            match self.run_upload_hook() {
                Ok(true) => up_ok += 1,
                Ok(false) => up_fail += 1,
                Err(e) => {
                    up_fail += 1;
                    self.event("upload_failed", &e);
                }
            }
            // retention may now delete uploaded segments
            self.apply_retention()?;
            self.write_manifest()?;
        }
        self.save_state()?;
        Ok(Some((got, eos, deleted, up_ok, up_fail)))
    }

    /// Delete the oldest segments beyond the count/byte limits. Returns how many.
    pub fn apply_retention(&mut self) -> Result<u64, String> {
        let r = self.cfg.retention.clone();
        let mut deleted = 0;
        loop {
            let count = self.state.segments.len() as u64;
            let bytes: u64 = self.state.segments.iter().map(|s| s.bytes()).sum();
            let over = r.max_segments.is_some_and(|m| count > m)
                || r.quota_bytes.is_some_and(|q| bytes > q);
            if !over {
                break;
            }
            let Some(i) = self
                .state
                .segments
                .iter()
                .position(|s| s.uploaded || !r.keep_unuploaded)
            else {
                // nothing deletable: flag on the newest segment once
                if let Some(last) = self.state.segments.last_mut() {
                    if !last.health.flags.iter().any(|f| f == "disk_quota") {
                        last.health.flags.push("disk_quota".into());
                    }
                }
                self.event(
                    "disk_quota",
                    &format!("{count} segments / {bytes} bytes over the limit; none uploaded"),
                );
                break;
            };
            let s = self.state.segments.remove(i);
            for f in &s.files {
                let _ = std::fs::remove_file(self.dir.join(&f.path));
            }
            self.state.deleted.push((s.seq, s.name.clone(), s.uploaded));
            self.event("retention_delete", &format!("{} (uploaded {})", s.name, s.uploaded));
            deleted += 1;
        }
        Ok(deleted)
    }

    /// Run the upload hook on the current manifest. `Ok(true)` when it exited 0 (the
    /// listed segments are then marked uploaded).
    pub fn run_upload_hook(&mut self) -> Result<bool, String> {
        let Some(argv) = self.cfg.upload.command.clone() else {
            return Ok(false);
        };
        let (path, m) = self.write_manifest()?;
        if m.segments.is_empty() {
            return Ok(true);
        }
        let subst = |a: &str| {
            a.replace("{manifest}", &path.to_string_lossy())
                .replace("{dir}", &self.dir.to_string_lossy())
        };
        let (prog, args) = argv.split_first().ok_or("upload.command is empty")?;
        let status = std::process::Command::new(subst(prog))
            .args(args.iter().map(|a| subst(a)))
            .current_dir(&self.dir)
            .status()
            .map_err(|e| format!("upload hook {prog}: {e}"))?;
        if status.success() {
            let seqs: Vec<u64> = m.segments.iter().map(|s| s.seq).collect();
            for s in &mut self.state.segments {
                if seqs.contains(&s.seq) {
                    s.uploaded = true;
                }
            }
            self.event("upload_ok", &format!("{} segment(s)", seqs.len()));
            Ok(true)
        } else {
            self.event("upload_failed", &format!("hook exit status {status}"));
            Ok(false)
        }
    }
}

/// Parse a segment's `core:datetime` back to POSIX ns (helper for consumers).
pub fn segment_start_ns(meta: &serde_json::Value) -> Option<i128> {
    meta["captures"][0]["core:datetime"]
        .as_str()
        .and_then(|s| unix_ns_from_iso(s).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hw::DeviceStatus;

    /// In-test receive device playing back samples of a simulated scene.
    struct MockRx {
        data: Vec<Vec<f64>>,
        pos: usize,
        fs: Option<f64>,
        start_ns: Option<i64>,
        pps: bool,
        gain: f64,
        overflow_after: Option<usize>,
        status: DeviceStatus,
    }

    impl MockRx {
        fn new(data: Vec<Vec<f64>>, start_ns: Option<i64>) -> MockRx {
            MockRx {
                data,
                pos: 0,
                fs: None,
                start_ns,
                pps: start_ns.is_some(),
                gain: 1.0,
                overflow_after: None,
                status: DeviceStatus::default(),
            }
        }
    }

    impl Device for MockRx {
        fn name(&self) -> String {
            "mock-rx".into()
        }
        fn realtime(&self) -> bool {
            false
        }
        fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
            if cfg.rx_channels != self.data.len() {
                return Err("channel count".into());
            }
            self.fs = Some(cfg.sample_rate_hz);
            Ok(())
        }
        fn sample_rate_hz(&self) -> Option<f64> {
            self.fs
        }
        fn start_tx(&mut self, _: StartTime) -> Result<Option<i64>, String> {
            Err("rx only".into())
        }
        fn write_tx(&mut self, _: &[Vec<f64>]) -> Result<(), String> {
            Err("rx only".into())
        }
        fn stop_tx(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn start_rx(&mut self, _at: StartTime) -> Result<Option<i64>, String> {
            self.status.rx_streaming = true;
            self.status.pps_aligned = self.pps;
            self.status.start_time_ns = self.start_ns;
            self.status.ref_locked = Some(true);
            Ok(self.start_ns)
        }
        fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
            let len = self.data[0].len();
            let e = (self.pos + n).min(len);
            let b: Vec<Vec<f64>> = self
                .data
                .iter()
                .map(|c| c[self.pos..e].iter().map(|v| v * self.gain).collect())
                .collect();
            if let Some(k) = self.overflow_after {
                if self.pos < k && e >= k {
                    self.status.overflows += 1;
                }
            }
            self.pos = e;
            self.status.samples_rx = e as u64;
            Ok(if b[0].is_empty() { vec![] } else { b })
        }
        fn stop_rx(&mut self) -> Result<(), String> {
            self.status.rx_streaming = false;
            Ok(())
        }
        fn status(&self) -> DeviceStatus {
            self.status.clone()
        }
    }

    const START: &str = "2026-11-18T03:00:00Z";
    const FS: f64 = 400_000.0;

    /// Scene: GRI DB chain 8970 received on Lake Erie, light noise, 1.2 s.
    fn scene_samples() -> Vec<Vec<f64>> {
        let snippet = crate::gridb::scenario_snippet(8970).unwrap();
        let src = format!(
            "[scene]\nstart_utc = \"{START}\"\nduration_s = 1.2\nfs_hz = {FS}\nseed = 3\noutput_scale_uv_m = 100000.0\n\
             [receiver]\ntype = \"static\"\nlat_deg = 41.95\nlon_deg = -81.6\n\
             [[outputs]]\ntype = \"e_field\"\n\
             [[time_refs]]\nid = \"UTC(USNO)\"\n\
             [propagation.ground]\nmodel = \"simple\"\n\
             [[interference]]\ntype = \"wgn\"\nrms_uv_m_20khz = 20.0\n{snippet}"
        );
        crate::run_to_memory(&src).unwrap().0.channels
    }

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kshana-field-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn config(dir: &Path) -> StationConfig {
        let src = format!(
            "site_id = \"lfs-test\"\noutput_dir = {:?}\nsample_rate_hz = {FS}\nsegment_s = 0.6\n\
             position = {{ lat_deg = 41.95, lon_deg = -81.6 }}\n\
             antenna = {{ type = \"e_field\", model = \"mock whip\" }}\n\
             [reference]\npps = \"external\"\nclock = \"external\"\ndevice_time = \"unix_utc\"\n\
             [detection]\nchains = [\"8970-great-lakes\", \"7980-southeast-us\", \"5930-canadian-east-coast\"]\nanalyse_s = 0.6\n",
            dir.to_string_lossy()
        );
        StationConfig::from_toml(&src).unwrap()
    }

    fn start_ns() -> i64 {
        unix_ns_from_iso(START).unwrap() as i64
    }

    #[test]
    fn records_segments_detects_chain_writes_manifest_and_resumes() {
        let dir = tmpdir("rec");
        let data = scene_samples();
        let mut dev = MockRx::new(data.clone(), Some(start_ns()));
        let mut rec = Recorder::open(config(&dir)).unwrap();
        let stop = AtomicBool::new(false);
        let rep = rec.run(&mut dev, None, &stop).unwrap();
        assert_eq!(rep.segments_written, 2);
        assert!(rep.end_of_stream);
        assert_eq!(rec.state.segments.len(), 2);
        let s0 = &rec.state.segments[0];
        let s1 = &rec.state.segments[1];
        assert_eq!(s0.start_utc, "2026-11-18T03:00:00.000000Z");
        assert_eq!(s1.start_utc, "2026-11-18T03:00:00.600000Z");
        assert_eq!(s0.timestamp_source, "device_pps");
        for s in [s0, s1] {
            assert!(s.health.flags.is_empty(), "{:?}", s.health);
            let ids: Vec<&str> = s.detected.iter().map(|d| d.id.as_str()).collect();
            assert_eq!(ids, ["8970-great-lakes"], "{:?}", s.detected);
            let d = &s.detected[0];
            assert!(d.stations.iter().filter(|x| x.found).count() >= 3);
            assert!(d.stations.iter().any(|x| x.snr_db.is_some_and(|v| v > 10.0)));
        }
        // SigMF metadata carries the timestamp and annotation
        let meta = crate::replay::reader::read_meta(&dir.join(format!("segments/{}", s1.name))).unwrap();
        assert_eq!(
            segment_start_ns(&meta),
            Some(start_ns() as i128 + 600_000_000)
        );
        assert_eq!(meta["annotations"][0]["kshana:chain_id"], "8970-great-lakes");
        assert_eq!(meta["captures"][0]["kshana:pps_aligned"], true);
        // the recording round-trips
        let r = crate::io::read_sigmf(&dir.join(format!("segments/{}", s0.name)), None).unwrap();
        assert_eq!(r.channels[0].len(), 240_000);
        assert!((r.channels[0][1000] - data[0][1000]).abs() < 1.0 / 32767.0);
        // manifest hashes match the files
        let m: UploadManifest = serde_json::from_str(
            &std::fs::read_to_string(dir.join("upload_manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(m.segments.len(), 2);
        assert!(verify_manifest(&dir, &m).is_empty());
        // tampering is detected
        let p = dir.join(&m.segments[0].files[1].path);
        let mut t = std::fs::read_to_string(&p).unwrap();
        t.push(' ');
        std::fs::write(&p, t).unwrap();
        assert_eq!(verify_manifest(&dir, &m), vec![m.segments[0].files[1].path.clone()]);

        // resume: a leftover partial file is discarded, numbering continues
        std::fs::write(dir.join("partial/lfs-test_x.sigmf-data"), b"junk").unwrap();
        let mut cfg = config(&dir);
        cfg.detection.enabled = false;
        cfg.retention.max_segments = Some(3);
        cfg.upload.command = Some(vec!["true".into()]);
        let mut rec2 = Recorder::open(cfg).unwrap();
        assert_eq!(rec2.state.next_seq, 2);
        assert!(rec2
            .state
            .events
            .iter()
            .any(|e| e.kind == "incomplete_segment_discarded"));
        assert!(!dir.join("partial/lfs-test_x.sigmf-data").exists());
        let mut dev2 = MockRx::new(data, Some(start_ns() + 10_000_000_000));
        let rep2 = rec2.run(&mut dev2, Some(2), &stop).unwrap();
        assert_eq!(rep2.session, 2);
        assert_eq!(rep2.segments_written, 2);
        // hook succeeded → all uploaded; retention kept 3 of 4
        assert!(rep2.uploads_ok >= 1);
        assert_eq!(rec2.state.segments.len(), 3);
        assert_eq!(rec2.state.segments[0].seq, 1);
        assert!(rec2.state.segments.iter().all(|s| s.uploaded));
        assert_eq!(rec2.state.deleted[0].0, 0);
        assert!(!dir.join(&s0.files[0].path).exists());
        assert!(rec2.manifest().segments.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn health_flags_clipping_missing_pps_overflow_and_quota() {
        let dir = tmpdir("health");
        let data: Vec<Vec<f64>> = vec![(0..120_000)
            .map(|i| ((i as f64) * 0.37).sin() * 0.5)
            .collect()];
        let mut dev = MockRx::new(data, None); // no PPS, no device time
        dev.gain = 3.0; // clips
        dev.overflow_after = Some(50_000);
        let mut cfg = config(&dir);
        cfg.segment_s = 0.25;
        cfg.detection.enabled = false;
        cfg.retention.quota_bytes = Some(150_000); // one ri16 segment is 200 kB
        let mut rec = Recorder::open(cfg).unwrap();
        rec.host_clock = || 1_794_960_000_000_000_000; // fixed host time
        let rep = rec.run(&mut dev, None, &AtomicBool::new(false)).unwrap();
        assert_eq!(rep.segments_written, 2); // 100k + 20k (truncated)
        let s0 = &rec.state.segments[0];
        assert_eq!(s0.timestamp_source, "host_clock");
        assert_eq!(s0.start_utc, "2026-11-18T00:00:00.000000Z");
        for f in ["clipping", "missing_pps", "overflow", "disk_quota"] {
            assert!(s0.health.flags.iter().any(|x| x == f), "{f}: {:?}", s0.health);
        }
        let s1 = &rec.state.segments[1];
        assert!(s1.health.flags.iter().any(|x| x == "truncated"));
        assert!(s1.health.flags.iter().any(|x| x == "disk_quota"));
        assert!(!s1.health.flags.iter().any(|x| x == "overflow"));
        // nothing uploaded → nothing deleted despite the quota
        assert_eq!(rep.deleted, 0);
        assert!(rec.state.events.iter().any(|e| e.kind == "disk_quota"));
        assert!(s0.health.clip_fraction > 0.1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_validation() {
        let dir = tmpdir("cfg");
        let mut c = config(&dir);
        c.site_id = "bad id".into();
        assert!(c.validate().is_err());
        let mut c = config(&dir);
        c.format = "cf32".into();
        assert!(c.validate().is_err());
        assert_eq!(config(&dir).segment_samples(), 240_000);
        assert_eq!(config(&dir).device_config().rx_channels, 1);
    }
}
