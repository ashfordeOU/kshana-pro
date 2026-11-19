// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! A15 — coherent LF + L-band record and replay on one timeline (gap 9; NAVISP-EL1-132
//! record & replay).
//!
//! # Capture-set format
//!
//! A capture set is one JSON file `<set_id>.capset.json` ([`CaptureSet`], format
//! [`CAPSET_FORMAT`]) plus one SigMF pair per stream:
//!
//! * `<set_id>_lf.sigmf-data/-meta`: the eLoran channel(s), real `rf32_le` or `ri16_le`
//!   (one channel per antenna output of the scenario);
//! * `<set_id>_gnss.sigmf-data/-meta`: the GNSS complex baseband, `ci16_le` around the
//!   stated centre frequency (1575.42 MHz for GPS L1 / Galileo E1).
//!
//! The shared time annotation, per stream ([`StreamEntry`]):
//!
//! * `start_tai_ps`: the instant the digitiser took sample 0, on **one integer-picosecond
//!   timeline** (TAI picoseconds since 1958-01-01, [`crate::time::AbsTime`], written as a
//!   decimal string so no JSON reader rounds it), plus the same instant as UTC ISO text
//!   (informative);
//! * `fs_hz`, and the set's `rate_ratios` (exact reduced fractions of integer sample rates);
//! * `clock_source` / `clock_id`: what disciplined the digitiser (e.g. "GPSDO 10 MHz + 1PPS",
//!   one id per physical clock: two streams on one clock share it);
//! * `cable_delay_ps` (and its standard uncertainty): the delay from the common reference
//!   plane (antenna feed or calibration plane) to the digitiser input.
//!
//! Sample `n` of a stream holds the reference-plane signal of instant
//! `start_tai_ps + n·10¹²/fs − cable_delay_ps` ([`StreamEntry::reference_time_ps`]). Each
//! SigMF metadata file repeats the annotation under `global."kshana:capset"`, gives the UTC
//! start as `captures[0]."core:datetime"`, and lists every marker as an annotation
//! (`core:sample_start`, `core:label`, `kshana:sample_frac`).
//!
//! # Recorder model
//!
//! [`record_capture_set`] writes both streams from one scenario: the eLoran stream by this
//! crate's scene (started at the reference-plane instant of the LF digitiser's sample 0,
//! so the picosecond scene timeline carries the start offset and cable delay exactly) and
//! the GNSS stream by any [`BasebandGenerator`] (the trait seam: kshana-pro implements it
//! with its software GPS L1 C/A + Galileo E1 generator; this crate stays independent of
//! kshana-pro and ships [`SyntheticBaseband`], a tone-plus-noise stand-in for tests).
//! A marker of known shape is injected into both streams at the same reference-plane
//! instants (by default every whole second, a PPS; or given offsets): a Gaussian pulse
//! (τ = 10 µs, spectral null at 100 kHz so the eLoran pulses do not bias it) on the LF
//! stream, a Gaussian-enveloped complex chirp on the baseband stream (both evaluated
//! analytically at each sample instant, so fractional delays are exact).
//!
//! # Replayer check
//!
//! [`replay_check`] reads the set, checks every data file against its SHA-256, and
//! [`check_alignment`] finds each marker in each stream: coarse search of the correlation
//! with the analytic marker over ± `search_ps` around its declared position, then
//! golden-section refinement of the fractional delay (maximum likelihood for a known
//! shape in white noise). After the declared start times and cable delays, the marker must
//! appear at the same reference-plane instant in both streams: `|t_lf − t_gnss| ≤
//! tolerance_ps` (default 5 ns) and each within the tolerance of the declared instant. A
//! wrong start stamp, cable delay or sample rate shows up as a failed marker. The report
//! carries a Cramér–Rao estimate of each marker time's standard deviation from the noise
//! measured beside the marker (conservative on the LF stream: the eLoran signal counts as
//! noise).
//!
//! # Evidence
//!
//! MODELLED: round trip and misalignment detection verified on simulated streams (tests).
//! No coherent LF + L-band hardware capture has been made yet; with hardware the marker is
//! an injected PPS-locked pulse whose own injection delay must be calibrated.

use crate::io::{read_sigmf, Format, SigmfWriter};
use crate::scenario::Scenario;
use crate::scene::{MemSink, SampleSink};
use crate::time::{AbsTime, Ps, PS_PER_S};
use crate::timebase::gauss;
use crate::valid::inverse::golden_section;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Format tag of a capture-set file.
pub const CAPSET_FORMAT: &str = "kshana-capset";
pub const CAPSET_VERSION: u32 = 1;
/// Timeline of every `*_tai_ps` field.
pub const TIMELINE: &str = "TAI, integer picoseconds since 1958-01-01T00:00:00 TAI";

const PS: f64 = PS_PER_S as f64;

// ───────────────────────────── generator seam ─────────────────────────────

/// A complex-baseband generator driven by a scenario (the GNSS half of a hybrid scene).
pub trait BasebandGenerator {
    /// Description written into the metadata.
    fn label(&self) -> String;
    fn fs_hz(&self) -> f64;
    fn center_hz(&self) -> f64;
    /// `n` complex samples `[I, Q]`; sample `k` is the reference-plane signal at instant
    /// `start + k / fs` (the scenario gives trajectory, time references and seed).
    fn generate(
        &mut self,
        sc: &Scenario,
        start: AbsTime,
        n: usize,
    ) -> Result<Vec<[f64; 2]>, String>;
}

/// A deterministic stand-in baseband (tests, plumbing checks; not a GNSS signal): complex
/// tones whose phase is referred to the absolute timeline, plus seeded white noise of
/// `noise_rms` per component.
#[derive(Clone, Debug, PartialEq)]
pub struct SyntheticBaseband {
    pub fs_hz: f64,
    pub center_hz: f64,
    /// (frequency offset Hz, amplitude).
    pub tones: Vec<(f64, f64)>,
    pub noise_rms: f64,
    pub seed: u64,
}

impl BasebandGenerator for SyntheticBaseband {
    fn label(&self) -> String {
        format!(
            "synthetic baseband ({} tones, noise {} rms; not a GNSS signal)",
            self.tones.len(),
            self.noise_rms
        )
    }
    fn fs_hz(&self) -> f64 {
        self.fs_hz
    }
    fn center_hz(&self) -> f64 {
        self.center_hz
    }
    fn generate(
        &mut self,
        _sc: &Scenario,
        start: AbsTime,
        n: usize,
    ) -> Result<Vec<[f64; 2]>, String> {
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ (start.0 as u64));
        // phase referred to the timeline (seconds of day keep f64 precision)
        let t0 = start.0.rem_euclid(86_400 * PS_PER_S as i128) as f64 / PS;
        Ok((0..n)
            .map(|k| {
                let t = t0 + k as f64 / self.fs_hz;
                let mut v = [0.0, 0.0];
                for (f, a) in &self.tones {
                    let ph = std::f64::consts::TAU * (f * t).fract();
                    v[0] += a * ph.cos();
                    v[1] += a * ph.sin();
                }
                v[0] += self.noise_rms * gauss(&mut rng);
                v[1] += self.noise_rms * gauss(&mut rng);
                v
            })
            .collect())
    }
}

// ───────────────────────────── format ─────────────────────────────

/// What a stream carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    /// Real LF samples (eLoran band).
    EloranLf,
    /// Complex baseband (GNSS L-band).
    GnssBaseband,
}

/// Analytic marker: `A·exp(−t²/2τ²)·exp(jπ·κ·t²)`, `κ = sweep/(4τ)` (a Gaussian pulse
/// for `sweep_hz = 0`); a real stream carries the real part.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarkerShape {
    pub tau_ps: i64,
    pub sweep_hz: f64,
    /// Amplitude in sample units (before any integer scaling).
    pub amplitude: f64,
}

impl MarkerShape {
    /// Complex value at `t` seconds from the marker instant.
    pub fn at(&self, t: f64) -> [f64; 2] {
        let tau = self.tau_ps as f64 / PS;
        let env = self.amplitude * (-0.5 * (t / tau).powi(2)).exp();
        if self.sweep_hz == 0.0 {
            return [env, 0.0];
        }
        let k = self.sweep_hz / (4.0 * tau);
        let ph = std::f64::consts::PI * k * t * t;
        [env * ph.cos(), env * ph.sin()]
    }
    /// Half-length of the marker support used for injection and detection (5 τ), s.
    pub fn half_span_s(&self) -> f64 {
        5.0 * self.tau_ps as f64 / PS
    }
}

/// One stream of a capture set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamEntry {
    pub name: String,
    pub kind: StreamKind,
    /// SigMF base relative to the capture-set file's directory.
    pub base: String,
    /// SigMF datatype: `rf32_le`, `ri16_le` (real) or `ci16_le` (complex).
    pub datatype: String,
    pub num_channels: usize,
    pub fs_hz: f64,
    pub center_hz: f64,
    /// Integer sample value per unit for integer complex data (1 for real data).
    pub scale: f64,
    /// Instant of sample 0 at the digitiser, TAI ps since 1958 (decimal string).
    pub start_tai_ps: String,
    /// The same instant in UTC (informative).
    pub start_utc: String,
    pub clock_source: String,
    pub clock_id: String,
    /// Reference plane → digitiser input delay (ps) and its standard uncertainty.
    pub cable_delay_ps: i64,
    pub cable_delay_u_ps: i64,
    pub samples: u64,
    pub sha256: String,
    pub marker: MarkerShape,
    /// What produced the samples.
    pub source: String,
}

impl StreamEntry {
    pub fn start(&self) -> Result<AbsTime, String> {
        self.start_tai_ps
            .parse::<i128>()
            .map(AbsTime)
            .map_err(|e| format!("stream {}: start_tai_ps: {e}", self.name))
    }
    /// Reference-plane instant of sample `n` (fractional allowed), as ps relative to
    /// `origin` (f64, exact while |result| < 2⁵³ ps ≈ 2.5 h).
    pub fn reference_time_ps(&self, n: f64, origin: AbsTime) -> Result<f64, String> {
        let base = (self.start()?.0 - self.cable_delay_ps as i128 - origin.0) as f64;
        Ok(base + n * PS / self.fs_hz)
    }
    /// Fractional sample index holding the reference-plane instant `t`.
    pub fn sample_at(&self, t: AbsTime) -> Result<f64, String> {
        let d = (t.0 - (self.start()?.0 - self.cable_delay_ps as i128)) as f64;
        Ok(d / PS * self.fs_hz)
    }
    fn is_complex(&self) -> bool {
        self.datatype.starts_with('c')
    }
}

/// An exact sample-rate ratio between two streams.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RateRatio {
    pub a: String,
    pub b: String,
    /// `fs_a / fs_b = num / den` (reduced); `None` when a rate is not a whole number of Hz.
    pub num: Option<u64>,
    pub den: Option<u64>,
    pub value: f64,
}

/// A declared marker (reference-plane instant).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarkerDecl {
    pub label: String,
    pub tai_ps: String,
}

impl MarkerDecl {
    pub fn instant(&self) -> Result<AbsTime, String> {
        self.tai_ps
            .parse::<i128>()
            .map(AbsTime)
            .map_err(|e| format!("marker {}: {e}", self.label))
    }
}

/// The capture-set file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureSet {
    pub format: String,
    pub version: u32,
    pub set_id: String,
    pub timeline: String,
    pub streams: Vec<StreamEntry>,
    pub rate_ratios: Vec<RateRatio>,
    pub markers: Vec<MarkerDecl>,
    /// Stated cross-stream alignment tolerance (ps).
    pub tolerance_ps: i64,
    /// SHA-256 of the scenario the set was recorded from (empty for a hardware capture).
    pub scenario_sha256: String,
}

impl CaptureSet {
    pub fn stream(&self, name: &str) -> Option<&StreamEntry> {
        self.streams.iter().find(|s| s.name == name)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
    pub fn from_json(s: &str) -> Result<CaptureSet, String> {
        let c: CaptureSet = serde_json::from_str(s).map_err(|e| format!("capture set: {e}"))?;
        if c.format != CAPSET_FORMAT {
            return Err(format!("not a capture set (format {:?})", c.format));
        }
        Ok(c)
    }
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The ratio `fs_a / fs_b`, exact when both are whole numbers of Hz.
pub fn rate_ratio(a: &StreamEntry, b: &StreamEntry) -> RateRatio {
    let whole = |f: f64| (f > 0.0 && f.fract() == 0.0 && f < 1e15).then_some(f as u64);
    let (num, den) = match (whole(a.fs_hz), whole(b.fs_hz)) {
        (Some(x), Some(y)) => {
            let g = gcd(x, y);
            (Some(x / g), Some(y / g))
        }
        _ => (None, None),
    };
    RateRatio {
        a: a.name.clone(),
        b: b.name.clone(),
        num,
        den,
        value: a.fs_hz / b.fs_hz,
    }
}

// ───────────────────────────── recorder ─────────────────────────────

/// Digitiser set-up of one stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamSetup {
    pub clock_source: String,
    pub clock_id: String,
    pub cable_delay_ps: i64,
    #[serde(default)]
    pub cable_delay_u_ps: i64,
    /// Digitiser start relative to the scenario start (ps, may be negative).
    #[serde(default)]
    pub start_offset_ps: i64,
    /// Fault injection for tests: the stamped start differs from the true start by this
    /// (a mis-stamped recorder). 0 for a correct recorder.
    #[serde(default)]
    pub stamp_error_ps: i64,
}

impl StreamSetup {
    pub fn new(clock_source: &str, clock_id: &str, cable_delay_ps: i64) -> StreamSetup {
        StreamSetup {
            clock_source: clock_source.into(),
            clock_id: clock_id.into(),
            cable_delay_ps,
            cable_delay_u_ps: 0,
            start_offset_ps: 0,
            stamp_error_ps: 0,
        }
    }
}

/// Where the markers go.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerPlan {
    /// Every whole second (UTC and TAI seconds coincide) inside both streams: a PPS.
    Pps,
    /// Offsets from the scenario start (ps).
    Offsets(Vec<i64>),
}

/// Recorder plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordPlan {
    /// File-name stem (letters, digits, `-`, `_`).
    pub set_id: String,
    pub lf: StreamSetup,
    pub gnss: StreamSetup,
    pub lf_format: String,
    /// Integer value per unit of the `ci16_le` baseband.
    pub gnss_scale: f64,
    pub markers: MarkerPlan,
    pub lf_marker: MarkerShape,
    /// Baseband marker; a `sweep_hz` of 0 here means "half the sample rate".
    pub gnss_marker: MarkerShape,
    pub tolerance_ps: i64,
}

impl RecordPlan {
    /// Defaults: rf32 LF, PPS markers, LF Gaussian τ = 10 µs amplitude 0.5, baseband chirp
    /// τ = 40 µs amplitude 80 (noise of unit power per sample: ≈ 1 ns marker time σ at
    /// 2 MHz) sweeping fs/2, scale 300, tolerance 5 ns.
    pub fn new(set_id: &str, lf: StreamSetup, gnss: StreamSetup) -> RecordPlan {
        RecordPlan {
            set_id: set_id.into(),
            lf,
            gnss,
            lf_format: "rf32".into(),
            gnss_scale: 300.0,
            markers: MarkerPlan::Pps,
            lf_marker: MarkerShape {
                tau_ps: 10_000_000,
                sweep_hz: 0.0,
                amplitude: 0.5,
            },
            gnss_marker: MarkerShape {
                tau_ps: 40_000_000,
                sweep_hz: 0.0,
                amplitude: 80.0,
            },
            tolerance_ps: 5_000,
        }
    }
}

fn add_marker(samples: &mut [[f64; 2]], ref0_minus_m_ps: i128, fs: f64, shape: &MarkerShape) {
    // sample n is at (ref0 − m) + n/fs from the marker
    let off = ref0_minus_m_ps as f64 / PS;
    let half = shape.half_span_s();
    let n_lo = ((-half - off) * fs).floor().max(0.0) as usize;
    let n_hi = (((half - off) * fs).ceil().max(0.0) as usize).min(samples.len());
    for (n, s) in samples.iter_mut().enumerate().take(n_hi).skip(n_lo) {
        let v = shape.at(off + n as f64 / fs);
        s[0] += v[0];
        s[1] += v[1];
    }
}

fn file_sha256(p: &Path) -> Result<String, String> {
    let b = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(hex::encode(Sha256::digest(&b)))
}

fn sibling(dir: &Path, base: &str, ext: &str) -> PathBuf {
    dir.join(format!("{base}.{ext}"))
}

/// Write `<dir>/<base>.sigmf-*` complex `ci16_le`.
fn write_ci16(
    dir: &Path,
    base: &str,
    samples: &[[f64; 2]],
    scale: f64,
) -> Result<(PathBuf, u64), String> {
    let p = sibling(dir, base, "sigmf-data");
    let mut buf = Vec::with_capacity(samples.len() * 4);
    let mut clipped = 0u64;
    for s in samples {
        for v in s {
            let x = (v * scale).round();
            if x.abs() > 32767.0 {
                clipped += 1;
            }
            buf.extend_from_slice(&(x.clamp(-32767.0, 32767.0) as i16).to_le_bytes());
        }
    }
    std::fs::write(&p, &buf).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok((p, clipped))
}

fn read_ci16(dir: &Path, base: &str, scale: f64) -> Result<Vec<Vec<f64>>, String> {
    let p = sibling(dir, base, "sigmf-data");
    let raw = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let n = raw.len() / 4;
    let (mut i, mut q) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for k in 0..n {
        let o = 4 * k;
        i.push(i16::from_le_bytes([raw[o], raw[o + 1]]) as f64 / scale);
        q.push(i16::from_le_bytes([raw[o + 2], raw[o + 3]]) as f64 / scale);
    }
    Ok(vec![i, q])
}

fn annotate_meta(
    dir: &Path,
    entry: &StreamEntry,
    set_id: &str,
    markers: &[MarkerDecl],
    extra_global: serde_json::Value,
) -> Result<(), String> {
    let p = sibling(dir, &entry.base, "sigmf-meta");
    let mut meta: serde_json::Value = match std::fs::read_to_string(&p) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", p.display()))?,
        Err(_) => serde_json::json!({ "global": {}, "captures": [], "annotations": [] }),
    };
    let g = meta["global"].as_object_mut().ok_or("SigMF: no global")?;
    g.insert("core:datatype".into(), entry.datatype.clone().into());
    g.insert("core:sample_rate".into(), entry.fs_hz.into());
    g.insert("core:num_channels".into(), entry.num_channels.into());
    g.insert("core:version".into(), "1.0.0".into());
    g.entry("core:description")
        .or_insert_with(|| entry.source.clone().into());
    g.insert(
        "kshana:capset".into(),
        serde_json::json!({
            "set_id": set_id,
            "stream": entry.name,
            "timeline": TIMELINE,
            "start_tai_ps": entry.start_tai_ps,
            "clock_source": entry.clock_source,
            "clock_id": entry.clock_id,
            "cable_delay_ps": entry.cable_delay_ps,
            "cable_delay_u_ps": entry.cable_delay_u_ps,
            "scale": entry.scale,
            "marker": entry.marker,
        }),
    );
    if let serde_json::Value::Object(x) = extra_global {
        for (k, v) in x {
            g.insert(k, v);
        }
    }
    meta["captures"] = serde_json::json!([{
        "core:sample_start": 0,
        "core:frequency": entry.center_hz,
        "core:datetime": entry.start_utc,
    }]);
    let mut ann = Vec::new();
    for m in markers {
        let n = entry.sample_at(m.instant()?)?;
        if n >= 0.0 && n < entry.samples as f64 {
            ann.push(serde_json::json!({
                "core:sample_start": n.floor() as u64,
                "core:sample_count": 1,
                "core:label": m.label,
                "kshana:sample_frac": n - n.floor(),
                "kshana:tai_ps": m.tai_ps,
            }));
        }
    }
    meta["annotations"] = serde_json::Value::Array(ann);
    std::fs::write(
        &p,
        serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("{}: {e}", p.display()))
}

/// Record a capture set of `sc` into `dir` (see the module docs). Returns the set (also
/// written to `<dir>/<set_id>.capset.json`).
pub fn record_capture_set(
    sc: &Scenario,
    gnss: &mut dyn BasebandGenerator,
    plan: &RecordPlan,
    dir: &Path,
) -> Result<CaptureSet, String> {
    if plan.set_id.is_empty()
        || !plan
            .set_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("capture set id: letters, digits, '-' and '_' only".into());
    }
    let fmt = Format::parse(&plan.lf_format)?;
    if !(plan.gnss_scale > 0.0) || plan.tolerance_ps <= 0 {
        return Err("capture set: gnss_scale and tolerance must be positive".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let t_sc = sc.start()?;
    let dur = sc.scene.params.duration_s;
    let fs_lf = sc.scene.params.fs_hz;
    let fs_g = gnss.fs_hz();
    if !(fs_g > 0.0) {
        return Err("baseband generator: sample rate must be positive".into());
    }
    let mut gm = plan.gnss_marker;
    if gm.sweep_hz == 0.0 {
        gm.sweep_hz = 0.5 * fs_g;
    }
    let lm = plan.lf_marker;

    // true digitiser starts and reference-plane starts
    let lf_true = t_sc.add_ps(Ps(plan.lf.start_offset_ps));
    let g_true = t_sc.add_ps(Ps(plan.gnss.start_offset_ps));
    let lf_ref0 = lf_true.add_ps(Ps(-plan.lf.cable_delay_ps));
    let g_ref0 = g_true.add_ps(Ps(-plan.gnss.cable_delay_ps));
    let n_lf = (dur * fs_lf).round() as usize;
    let n_g = (dur * fs_g).round() as usize;

    // markers inside both streams with a margin of the marker span
    let margin = (lm.half_span_s().max(gm.half_span_s()) * PS) as i128;
    let span = |r0: AbsTime, n: usize, fs: f64| {
        (r0.0 + margin, r0.0 + (n as f64 / fs * PS) as i128 - margin)
    };
    let (a0, a1) = span(lf_ref0, n_lf, fs_lf);
    let (b0, b1) = span(g_ref0, n_g, fs_g);
    let (lo, hi) = (a0.max(b0), a1.min(b1));
    let instants: Vec<(String, i128)> = match &plan.markers {
        MarkerPlan::Pps => {
            let ps = PS_PER_S as i128;
            let mut v = Vec::new();
            let mut t = lo.div_euclid(ps) * ps;
            if t < lo {
                t += ps;
            }
            while t <= hi {
                v.push(("PPS".to_string(), t));
                t += ps;
            }
            v
        }
        MarkerPlan::Offsets(o) => o
            .iter()
            .enumerate()
            .map(|(i, x)| (format!("MARK{i}"), t_sc.0 + *x as i128))
            .collect(),
    };
    if instants.is_empty() {
        return Err("capture set: no marker instant inside both streams (lengthen the capture or give marker offsets)".into());
    }
    if let Some((l, _)) = instants.iter().find(|(_, t)| *t < lo || *t > hi) {
        return Err(format!(
            "capture set: marker {l} is not inside both streams with margin"
        ));
    }
    let markers: Vec<MarkerDecl> = instants
        .iter()
        .map(|(l, t)| MarkerDecl {
            label: l.clone(),
            tai_ps: t.to_string(),
        })
        .collect();

    // LF: the scene started at the reference-plane instant of the LF sample 0
    let mut lf_sc = sc.clone();
    lf_sc.scene.start_utc = lf_ref0.to_utc_iso();
    lf_sc.scene.params.duration_s = n_lf as f64 / fs_lf;
    let mut scene = lf_sc.build()?;
    let mut sink = MemSink::default();
    let mut truth = Vec::new();
    scene.run(&mut sink, &mut truth)?;
    let mut lf_ch = sink.channels;
    for ch in lf_ch.iter_mut() {
        ch.truncate(n_lf);
        let mut c: Vec<[f64; 2]> = ch.iter().map(|v| [*v, 0.0]).collect();
        for (_, t) in &instants {
            add_marker(&mut c, lf_ref0.0 - t, fs_lf, &lm);
        }
        *ch = c.into_iter().map(|v| v[0]).collect();
    }
    let lf_base = format!("{}_lf", plan.set_id);
    let mut w = SigmfWriter::create(
        &dir.join(&lf_base),
        fmt,
        lf_ch.len(),
        fs_lf,
        serde_json::json!({ "scenario_sha256": sc.hash() }),
    )?;
    w.write_block(&lf_ch)?;
    w.finish()?;
    let lf_stamp = lf_true.add_ps(Ps(plan.lf.stamp_error_ps));
    let lf_entry = StreamEntry {
        name: "eloran".into(),
        kind: StreamKind::EloranLf,
        base: lf_base.clone(),
        datatype: fmt.sigmf().into(),
        num_channels: lf_ch.len(),
        fs_hz: fs_lf,
        center_hz: 100_000.0,
        scale: 1.0,
        start_tai_ps: lf_stamp.0.to_string(),
        start_utc: lf_stamp.to_utc_iso(),
        clock_source: plan.lf.clock_source.clone(),
        clock_id: plan.lf.clock_id.clone(),
        cable_delay_ps: plan.lf.cable_delay_ps,
        cable_delay_u_ps: plan.lf.cable_delay_u_ps,
        samples: n_lf as u64,
        sha256: file_sha256(&sibling(dir, &lf_base, "sigmf-data"))?,
        marker: lm,
        source: format!(
            "kshana-eloran scene (scenario {}), {} channel(s)",
            sc.hash(),
            lf_ch.len()
        ),
    };

    // GNSS baseband through the generator seam
    let mut g = gnss.generate(sc, g_ref0, n_g)?;
    if g.len() != n_g {
        return Err(format!(
            "baseband generator returned {} samples, {n_g} requested",
            g.len()
        ));
    }
    for (_, t) in &instants {
        add_marker(&mut g, g_ref0.0 - t, fs_g, &gm);
    }
    let g_base = format!("{}_gnss", plan.set_id);
    let (gp, clipped) = write_ci16(dir, &g_base, &g, plan.gnss_scale)?;
    if clipped > 0 {
        return Err(format!(
            "baseband: {clipped} values clipped at scale {} (lower gnss_scale)",
            plan.gnss_scale
        ));
    }
    let g_stamp = g_true.add_ps(Ps(plan.gnss.stamp_error_ps));
    let g_entry = StreamEntry {
        name: "gnss".into(),
        kind: StreamKind::GnssBaseband,
        base: g_base.clone(),
        datatype: "ci16_le".into(),
        num_channels: 1,
        fs_hz: fs_g,
        center_hz: gnss.center_hz(),
        scale: plan.gnss_scale,
        start_tai_ps: g_stamp.0.to_string(),
        start_utc: g_stamp.to_utc_iso(),
        clock_source: plan.gnss.clock_source.clone(),
        clock_id: plan.gnss.clock_id.clone(),
        cable_delay_ps: plan.gnss.cable_delay_ps,
        cable_delay_u_ps: plan.gnss.cable_delay_u_ps,
        samples: n_g as u64,
        sha256: file_sha256(&gp)?,
        marker: gm,
        source: gnss.label(),
    };
    annotate_meta(
        dir,
        &lf_entry,
        &plan.set_id,
        &markers,
        serde_json::json!({}),
    )?;
    annotate_meta(
        dir,
        &g_entry,
        &plan.set_id,
        &markers,
        serde_json::json!({ "kshana:eloran_scenario_sha256": sc.hash() }),
    )?;
    let set = CaptureSet {
        format: CAPSET_FORMAT.into(),
        version: CAPSET_VERSION,
        set_id: plan.set_id.clone(),
        timeline: TIMELINE.into(),
        rate_ratios: vec![rate_ratio(&lf_entry, &g_entry)],
        streams: vec![lf_entry, g_entry],
        markers,
        tolerance_ps: plan.tolerance_ps,
        scenario_sha256: sc.hash(),
    };
    let p = dir.join(format!("{}.capset.json", plan.set_id));
    std::fs::write(&p, set.to_json()).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(set)
}

// ───────────────────────────── replayer ─────────────────────────────

/// Samples of one stream read back: real channels, or `[I, Q]` for complex data.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadedStream {
    pub entry: StreamEntry,
    pub channels: Vec<Vec<f64>>,
}

/// Read a capture set and its streams; every data file is checked against its SHA-256.
pub fn read_capture_set(path: &Path) -> Result<(CaptureSet, Vec<LoadedStream>), String> {
    let set = CaptureSet::from_json(
        &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
    )?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut out = Vec::new();
    for e in &set.streams {
        let data = sibling(dir, &e.base, "sigmf-data");
        let h = file_sha256(&data)?;
        if h != e.sha256 {
            return Err(format!(
                "stream {}: data file does not match its SHA-256",
                e.name
            ));
        }
        let channels = if e.is_complex() {
            if e.datatype != "ci16_le" {
                return Err(format!("stream {}: unsupported {}", e.name, e.datatype));
            }
            read_ci16(dir, &e.base, e.scale)?
        } else {
            read_sigmf(&dir.join(&e.base), None)?.channels
        };
        if channels.first().map(|c| c.len() as u64) != Some(e.samples) {
            return Err(format!(
                "stream {}: {} samples on disk, {} declared",
                e.name,
                channels.first().map(|c| c.len()).unwrap_or(0),
                e.samples
            ));
        }
        out.push(LoadedStream {
            entry: e.clone(),
            channels,
        });
    }
    Ok((set, out))
}

/// One marker found in one stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarkerHit {
    pub stream: String,
    pub found: bool,
    /// Normalised correlation at the peak (1 = clean marker).
    pub correlation: f64,
    /// Fractional sample index of the marker.
    pub sample: f64,
    /// Measured reference-plane instant − declared instant (ps).
    pub offset_ps: f64,
    /// Cramér–Rao standard deviation of the marker time (ps) from the local noise.
    pub sigma_ps: f64,
}

/// One marker across the streams.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MarkerAlignment {
    pub label: String,
    pub tai_ps: String,
    pub hits: Vec<MarkerHit>,
    /// First stream − second stream reference-plane instant (ps).
    pub cross_ps: Option<f64>,
    pub pass: bool,
}

/// Result of the alignment check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlignmentReport {
    pub set_id: String,
    pub tolerance_ps: i64,
    pub markers: Vec<MarkerAlignment>,
    pub max_abs_cross_ps: f64,
    pub pass: bool,
    pub problems: Vec<String>,
}

/// Correlation of the marker template placed at fractional sample `pos` with the stream
/// (complex: magnitude; real: value).
fn corr_at(x: &[Vec<f64>], complex: bool, fs: f64, shape: &MarkerShape, pos: f64) -> f64 {
    let half = shape.half_span_s() * fs;
    let lo = (pos - half).floor().max(0.0) as usize;
    let hi = ((pos + half).ceil().max(0.0) as usize).min(x[0].len());
    let (mut re, mut im) = (0.0, 0.0);
    for n in lo..hi {
        let g = shape.at((n as f64 - pos) / fs);
        if complex {
            // x · conj(g)
            re += x[0][n] * g[0] + x[1][n] * g[1];
            im += x[1][n] * g[0] - x[0][n] * g[1];
        } else {
            re += x[0][n] * g[0];
        }
    }
    if complex {
        (re * re + im * im).sqrt()
    } else {
        re
    }
}

fn find_marker(
    st: &LoadedStream,
    m: &MarkerDecl,
    search_ps: i64,
) -> Result<(MarkerHit, f64), String> {
    let e = &st.entry;
    let complex = e.is_complex();
    let fs = e.fs_hz;
    let shape = e.marker;
    let x = &st.channels;
    let expect = e.sample_at(m.instant()?)?;
    let s = search_ps as f64 / PS * fs;
    let len = x[0].len() as f64;
    let half = shape.half_span_s() * fs;
    let k0 = (expect - s).floor().max(half);
    let k1 = (expect + s).ceil().min(len - half - 1.0);
    let miss = MarkerHit {
        stream: e.name.clone(),
        found: false,
        correlation: 0.0,
        sample: f64::NAN,
        offset_ps: f64::NAN,
        sigma_ps: f64::NAN,
    };
    if !(k1 > k0) {
        return Ok((miss, f64::NAN));
    }
    // coarse: integer positions
    let mut best = (f64::NEG_INFINITY, k0);
    let mut k = k0;
    while k <= k1 {
        let c = corr_at(x, complex, fs, &shape, k);
        if c > best.0 {
            best = (c, k);
        }
        k += 1.0;
    }
    // fine: golden section on the fractional position
    let r = golden_section(
        |p| -corr_at(x, complex, fs, &shape, p),
        best.1 - 1.0,
        best.1 + 1.0,
        1e-7,
    );
    let pos = r.x;
    let peak = -r.fx;
    // normalised correlation and noise beside the marker
    let lo = (pos - half).floor().max(0.0) as usize;
    let hi = ((pos + half).ceil() as usize).min(x[0].len());
    let (mut ex, mut eg, mut dg2) = (0.0, 0.0, 0.0);
    let comps = if complex { 2 } else { 1 };
    for n in lo..hi {
        let t = (n as f64 - pos) / fs;
        let g = shape.at(t);
        let h = 0.5 / fs;
        let (gp, gm) = (shape.at(t + h), shape.at(t - h));
        for c in 0..comps {
            ex += x[c][n] * x[c][n];
            eg += g[c] * g[c];
            dg2 += ((gp[c] - gm[c]) / (2.0 * h)).powi(2);
        }
    }
    let rho = if ex > 0.0 && eg > 0.0 {
        peak / (ex * eg).sqrt()
    } else {
        0.0
    };
    // noise: residual after removing the fitted marker over the support
    let a = peak / eg.max(1e-300);
    let mut res = 0.0;
    let mut nres = 0usize;
    for n in lo..hi {
        let g = shape.at((n as f64 - pos) / fs);
        for c in 0..comps {
            res += (x[c][n] - a * g[c]).powi(2);
            nres += 1;
        }
    }
    let var = if nres > 0 { res / nres as f64 } else { 0.0 };
    let sigma_s = (var / (a * a * dg2).max(1e-300)).sqrt();
    let t_ref = e.reference_time_ps(pos, m.instant()?)?;
    Ok((
        MarkerHit {
            stream: e.name.clone(),
            found: rho >= 0.5,
            correlation: rho,
            sample: pos,
            offset_ps: t_ref,
            sigma_ps: sigma_s * PS,
        },
        t_ref,
    ))
}

/// Find every declared marker in every stream and check cross-stream alignment within
/// the set's tolerance (see the module docs); `search_ps` is the search half-range.
pub fn check_alignment(
    set: &CaptureSet,
    streams: &[LoadedStream],
    search_ps: i64,
) -> Result<AlignmentReport, String> {
    let mut problems = Vec::new();
    let tol = set.tolerance_ps as f64;
    let mut markers = Vec::new();
    let mut max_cross = 0.0f64;
    if streams.len() < 2 {
        problems.push("fewer than two streams".into());
    }
    if set.markers.is_empty() {
        problems.push("no markers declared".into());
    }
    for m in &set.markers {
        let mut hits = Vec::new();
        let mut times = Vec::new();
        for st in streams {
            let (h, t) = find_marker(st, m, search_ps)?;
            if !h.found {
                problems.push(format!("{}: marker {} not found", st.entry.name, m.label));
            } else if h.offset_ps.abs() > tol {
                problems.push(format!(
                    "{}: marker {} at {:+.1} ns from its declared instant",
                    st.entry.name,
                    m.label,
                    h.offset_ps / 1000.0
                ));
            }
            times.push(h.found.then_some(t));
            hits.push(h);
        }
        let cross = match (
            times.first().copied().flatten(),
            times.get(1).copied().flatten(),
        ) {
            (Some(a), Some(b)) => Some(a - b),
            _ => None,
        };
        if let Some(c) = cross {
            max_cross = max_cross.max(c.abs());
            if c.abs() > tol {
                problems.push(format!(
                    "marker {}: streams disagree by {:+.1} ns (tolerance {:.1} ns)",
                    m.label,
                    c / 1000.0,
                    tol / 1000.0
                ));
            }
        }
        let pass = hits.iter().all(|h| h.found && h.offset_ps.abs() <= tol)
            && cross.is_some_and(|c| c.abs() <= tol);
        markers.push(MarkerAlignment {
            label: m.label.clone(),
            tai_ps: m.tai_ps.clone(),
            hits,
            cross_ps: cross,
            pass,
        });
    }
    Ok(AlignmentReport {
        set_id: set.set_id.clone(),
        tolerance_ps: set.tolerance_ps,
        pass: problems.is_empty() && markers.iter().all(|m| m.pass),
        markers,
        max_abs_cross_ps: max_cross,
        problems,
    })
}

/// Read a capture set (with integrity checks) and check its alignment, searching ±200 µs.
pub fn replay_check(path: &Path) -> Result<AlignmentReport, String> {
    let (set, streams) = read_capture_set(path)?;
    check_alignment(&set, &streams, 200_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCEN: &str = r#"
[scene]
start_utc = "2026-11-18T11:59:59.95Z"
duration_s = 0.1
fs_hz = 1000000
seed = 5
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[propagation.ground]
model = "simple"
[[chains]]
name = "c"
gri_us = 79800
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
"#;

    fn gen() -> SyntheticBaseband {
        SyntheticBaseband {
            fs_hz: 2_046_000.0,
            center_hz: 1_575_420_000.0,
            tones: vec![(250_000.0, 0.3), (-410_000.0, 0.2)],
            noise_rms: 0.7,
            seed: 9,
        }
    }

    fn plan(id: &str) -> RecordPlan {
        let mut lf = StreamSetup::new("GPSDO-A 10 MHz + 1PPS", "gpsdo-a", 152_500);
        lf.start_offset_ps = 0;
        let mut g = StreamSetup::new("GPSDO-A 10 MHz + 1PPS", "gpsdo-a", 37_250);
        g.start_offset_ps = 123_456_789; // the GNSS digitiser starts 123.456789 µs later
        RecordPlan::new(id, lf, g)
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kshana-capset-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn round_trip_keeps_one_timeline_and_markers_align() {
        let sc = Scenario::from_toml(SCEN).unwrap();
        let d = dir("rt");
        let set = record_capture_set(&sc, &mut gen(), &plan("rt1"), &d).unwrap();
        // the PPS at 12:00:00 is inside both streams
        assert_eq!(set.markers.len(), 1);
        let pps = AbsTime::from_utc_iso("2026-11-18T12:00:00Z").unwrap();
        assert_eq!(set.markers[0].instant().unwrap(), pps);
        assert_eq!(set.rate_ratios[0].num, Some(500));
        assert_eq!(set.rate_ratios[0].den, Some(1023));
        // start stamps on the integer-picosecond timeline
        let t0 = AbsTime::from_utc_iso("2026-11-18T11:59:59.95Z").unwrap();
        assert_eq!(set.streams[0].start().unwrap(), t0);
        assert_eq!(set.streams[1].start().unwrap(), t0.add_ps(Ps(123_456_789)));
        // file round trip: JSON, hashes, SigMF annotations
        let p = d.join("rt1.capset.json");
        let (back, streams) = read_capture_set(&p).unwrap();
        assert_eq!(back, set);
        assert_eq!(streams[0].channels[0].len(), 100_000);
        assert_eq!(streams[1].channels.len(), 2);
        let meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("rt1_gnss.sigmf-meta")).unwrap())
                .unwrap();
        assert_eq!(meta["global"]["kshana:capset"]["cable_delay_ps"], 37_250);
        assert_eq!(meta["annotations"][0]["core:label"], "PPS");
        let lfm: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(d.join("rt1_lf.sigmf-meta")).unwrap())
                .unwrap();
        assert_eq!(
            lfm["captures"][0]["core:datetime"],
            set.streams[0].start_utc.as_str()
        );
        // the LF annotation is at the marker's reference-plane sample (50 ms + 152.5 ns)
        let n = lfm["annotations"][0]["core:sample_start"].as_u64().unwrap();
        assert_eq!(n, 50_000);
        // alignment within the stated tolerance (5 ns), the eLoran marker to < 1 ps (noise-free), the baseband one to its noise σ
        let rep = replay_check(&p).unwrap();
        assert!(rep.pass, "{:?}", rep.problems);
        assert!(
            rep.max_abs_cross_ps < 2500.0,
            "{} {:?}",
            rep.max_abs_cross_ps,
            rep.markers
        );
        for h in &rep.markers[0].hits {
            assert!(h.correlation > 0.9, "{h:?}");
            assert!(h.sigma_ps > 0.0 && h.sigma_ps < 1200.0, "{h:?}");
            // within 3σ of the declared instant (σ from the local noise)
            assert!(h.offset_ps.abs() < 3.0 * h.sigma_ps.max(1.0), "{h:?}");
        }
        // the LF stream still carries the eLoran pulses (the scene ran)
        let lf = &streams[0].channels[0];
        let peak = lf[..40_000].iter().fold(0.0f64, |a, b| a.max(b.abs()));
        assert!(peak > 1e-4, "{peak}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn misalignment_is_detected() {
        let sc = Scenario::from_toml(SCEN).unwrap();
        // 1. a recorder that stamps the GNSS start 40 ns late
        let d = dir("mis");
        let mut pl = plan("mis1");
        pl.gnss.stamp_error_ps = 40_000;
        record_capture_set(&sc, &mut gen(), &pl, &d).unwrap();
        let rep = replay_check(&d.join("mis1.capset.json")).unwrap();
        assert!(!rep.pass);
        let c = rep.markers[0].cross_ps.unwrap();
        // the GNSS stream reads 40 ns late: eLoran − GNSS = −40 ns
        assert!((c + 40_000.0).abs() < 2_000.0, "cross {c} ps");
        assert!(
            rep.problems.iter().any(|p| p.contains("disagree")),
            "{:?}",
            rep.problems
        );

        // 2. a correct recording whose declared cable delay is edited by 10 ns
        let d2 = dir("cab");
        let set = record_capture_set(&sc, &mut gen(), &plan("cab1"), &d2).unwrap();
        let mut edited = set.clone();
        edited.streams[0].cable_delay_ps += 10_000;
        let p = d2.join("cab1.capset.json");
        std::fs::write(&p, edited.to_json()).unwrap();
        let rep = replay_check(&p).unwrap();
        assert!(!rep.pass);
        let c = rep.markers[0].cross_ps.unwrap();
        assert!((c + 10_000.0).abs() < 2_000.0, "cross {c} ps");

        // 3. a wrong declared sample rate moves the marker by microseconds
        let mut e2 = set.clone();
        e2.streams[1].fs_hz *= 1.0 + 1e-4;
        std::fs::write(&p, e2.to_json()).unwrap();
        assert!(!replay_check(&p).unwrap().pass);

        // 4. a tampered data file is refused before any alignment
        std::fs::write(&p, set.to_json()).unwrap();
        assert!(replay_check(&p).unwrap().pass);
        let data = d2.join("cab1_gnss.sigmf-data");
        let mut b = std::fs::read(&data).unwrap();
        b[100] ^= 1;
        std::fs::write(&data, b).unwrap();
        assert!(replay_check(&p).unwrap_err().contains("SHA-256"));
        std::fs::remove_dir_all(&d).ok();
        std::fs::remove_dir_all(&d2).ok();
    }

    #[test]
    fn stated_marker_sigma_matches_the_scatter_and_false_alarms_are_rare() {
        // Monte Carlo over 16 noise realisations: the scatter of the baseband marker time
        // agrees with the stated Cramér–Rao σ (within 0.6–1.6×), and a correct recording
        // passes the 5 ns tolerance every time.
        let sc = Scenario::from_toml(SCEN).unwrap();
        let d = dir("mc");
        let (mut off, mut sig) = (Vec::new(), Vec::new());
        let mut fails = 0;
        for seed in 0..16u64 {
            let mut g = gen();
            g.seed = 1000 + seed;
            record_capture_set(&sc, &mut g, &plan("mc"), &d).unwrap();
            let rep = replay_check(&d.join("mc.capset.json")).unwrap();
            if !rep.pass {
                fails += 1;
            }
            let h = &rep.markers[0].hits[1];
            off.push(h.offset_ps);
            sig.push(h.sigma_ps);
        }
        let s = crate::valid::stats(&off);
        let mean_sigma = sig.iter().sum::<f64>() / sig.len() as f64;
        let ratio = s.rms / mean_sigma;
        assert!(
            (0.6..1.6).contains(&ratio),
            "scatter {} ps vs σ {mean_sigma} ps",
            s.rms
        );
        assert!(s.mean.abs() < 3.0 * mean_sigma / 4.0, "bias {} ps", s.mean);
        assert_eq!(fails, 0);
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_marker_outside_the_streams_is_refused_and_sample_mapping_is_exact() {
        let sc = Scenario::from_toml(SCEN).unwrap();
        let mut pl = plan("out1");
        pl.markers = MarkerPlan::Offsets(vec![5_000_000]); // 5 µs: inside the margin
        assert!(record_capture_set(&sc, &mut gen(), &pl, &dir("out")).is_err());
        let e = StreamEntry {
            name: "x".into(),
            kind: StreamKind::EloranLf,
            base: "x".into(),
            datatype: "rf32_le".into(),
            num_channels: 1,
            fs_hz: 1e6,
            center_hz: 1e5,
            scale: 1.0,
            start_tai_ps: "1000000000000".into(),
            start_utc: String::new(),
            clock_source: String::new(),
            clock_id: String::new(),
            cable_delay_ps: 250,
            cable_delay_u_ps: 0,
            samples: 10,
            sha256: String::new(),
            marker: pl.lf_marker,
            source: String::new(),
        };
        // sample 3 holds the reference-plane instant start + 3 µs − 250 ps
        let t = AbsTime(1_000_000_000_000 + 3_000_000 - 250);
        assert!((e.sample_at(t).unwrap() - 3.0).abs() < 1e-12);
        assert!(e.reference_time_ps(3.0, t).unwrap().abs() < 1e-6);
    }
}
