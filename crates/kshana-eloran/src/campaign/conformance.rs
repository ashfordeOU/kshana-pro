// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Conformance test library (D6): receiver test cases as data, run on the simulator and
//! scored on receiver outputs.
//!
//! # Status of these tests — read first
//!
//! The cases are **modelled on** the kinds of tests found in LORAN-C / eLoran receiver
//! performance standards — e.g. RTCM SC-127 *Minimum Performance Standards for eLoran
//! receivers* and IEC 61075 *Loran-C receivers for ships* — **but the texts of those
//! standards were not available when this library was written**. No test procedure,
//! stimulus level or pass limit here is taken from a standard. Every limit is
//! **ILLUSTRATIVE** (`illustrative = true` is enforced on load) and each case lists, in
//! `to_confirm`, what must be checked against the governing standard before a result can
//! be called a conformance result. A PASS here means "the software receiver meets the
//! illustrative bar on this simulated stimulus", nothing more.
//!
//! # Case files
//!
//! A case is a scenario file (`scenarios/conformance/*.toml`) with an extra
//! `[conformance]` table (ignored by [`Scenario`]):
//!
//! ```toml
//! [conformance]
//! id = "CONF-ACQ-01"
//! title = "Acquisition sensitivity"
//! category = "acquisition_sensitivity"
//! intent = "…"
//! modelled_on = ["RTCM SC-127 eLoran MPS (acquisition) — text not consulted"]
//! to_confirm = ["stimulus definition", "noise bandwidth", "limits"]
//! targets = ["conf-A/M", "conf-A/X"]          # default: every station not in `interferers`
//! interferers = []                             # chain names that are stimulus only
//! [conformance.receiver]                       # software receiver options
//! track_gris = 4
//! [[conformance.variations]]
//! label = "snr-high"
//! stimulus = { noise_uv_m = 10.0 }
//! set = { "interference[0].rms_uv_m_20khz" = 10.0 }   # TOML path overrides
//! [[conformance.criteria]]
//! metric = "acquired_fraction"
//! op = "ge"
//! limit = 1.0
//! ```
//!
//! Override paths address the scenario TOML tree: `a.b`, `a[0].b`, e.g.
//! `chains[0].stations[1].ecd_us`, `propagation.sky` (a whole table), `scene.duration_s`.
//!
//! # Metrics
//!
//! Software receiver ([`evaluate_software`]), per variation; `metric@chain/station` is the
//! per-target value, the bare name the worst over targets (sum for counts):
//!
//! | metric | meaning |
//! |---|---|
//! | `acquired_fraction` | targets acquired / targets |
//! | `tracked_fraction` | targets with ≥ 1 measurement / targets |
//! | `toa_rms_ns`, `toa_bias_ns`, `toa_std_ns` | ToA error vs truth (worst; bias as \|mean\|) |
//! | `ecd_rms_us` | ECD error RMS |
//! | `cycle_errors` | windows with a wrong cycle (sum) |
//! | `cycle_slips` | windows flagged as cycle slips (sum) |
//! | `snr_error_db` | \|measured − truth\| SNR (worst) |
//! | `snr_truth_db` | truth SNR (stimulus, the first target; informative) |
//! | `position_error_m` | all-in-view range-domain fix vs mean truth position |
//! | `tdoa_position_error_m` | hyperbolic fix vs mean truth position |
//! | `time_error_ns` | time solution at the known position vs truth (worst \|error\|) |
//! | `acq_time_s` | shortest record prefix (multiples of 2 GRI of the slowest target chain) at which every target is acquired (NaN = never) |
//! | `blink_detection_rate`, `blink_false_alarm_rate` | per-group blink decisions (pulses 1–2 amplitude vs 3–8 from [`crate::rx::Receiver::pulse_timing`]) vs the scenario's blink faults |
//! | `ldc_messages`, `ldc_frames_failed`, `ldc_auth_failures` | data channel decoding (sum) |
//!
//! External receivers ([`evaluate_external`]) are scored from imported
//! [`ObsRecord`]s against the same truth: `acquired_fraction`, `toa_rms_ns`,
//! `toa_bias_ns`, `td_rms_ns`, `ecd_rms_us`, `snr_error_db`, `cycle_errors` (|ToA/TD error|
//! > 5 µs), `position_error_m` (95th percentile of fix errors), `time_error_ns` (95th
//! percentile of |timing_ns|, the 1PPS − reference reading), blink rates from the
//! records' `blink` flags.

use crate::chain::{Chain, StationFault, StationStatus};
use crate::field::importers::{compare_records, flags, obs_from_truth, percentile, ObsRecord};
use crate::field::unix_s_from_abs;
use crate::rx::{self, Antenna, LoopMode, RxConfig, RxInput, RxOutput};
use crate::scenario::Scenario;
use crate::time::AbsTime;
use crate::TruthRecord;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Metrics of one evaluation: name (optionally `name@chain/station`) → value.
pub type Metrics = BTreeMap<String, f64>;

/// The library directory shipped with the crate.
pub fn library_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scenarios/conformance")
}

/// Comparison operator of a criterion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Le,
    Lt,
    Ge,
    Gt,
    Eq,
}

impl Op {
    pub fn holds(self, v: f64, limit: f64) -> bool {
        match self {
            Op::Le => v <= limit,
            Op::Lt => v < limit,
            Op::Ge => v >= limit,
            Op::Gt => v > limit,
            Op::Eq => (v - limit).abs() <= 1e-9 * limit.abs().max(1.0),
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            Op::Le => "<=",
            Op::Lt => "<",
            Op::Ge => ">=",
            Op::Gt => ">",
            Op::Eq => "==",
        }
    }
}

/// A pass criterion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    pub metric: String,
    /// `chain/station` for a per-target metric.
    #[serde(default)]
    pub target: Option<String>,
    pub op: Op,
    pub limit: f64,
    /// Variation labels this applies to (empty = all).
    #[serde(default)]
    pub variations: Vec<String>,
    /// Always true in this library: the limit is not taken from a standard.
    #[serde(default = "yes")]
    pub illustrative: bool,
    #[serde(default)]
    pub note: String,
}

fn yes() -> bool {
    true
}

impl Criterion {
    pub fn key(&self) -> String {
        match &self.target {
            Some(t) => format!("{}@{t}", self.metric),
            None => self.metric.clone(),
        }
    }
    pub fn applies_to(&self, variation: &str) -> bool {
        self.variations.is_empty() || self.variations.iter().any(|v| v == variation)
    }
}

/// A stimulus variation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variation {
    pub label: String,
    /// What is varied, for reports (name → value).
    #[serde(default)]
    pub stimulus: BTreeMap<String, f64>,
    /// Scenario TOML path → value.
    #[serde(default)]
    pub set: BTreeMap<String, toml::Value>,
}

/// Software receiver options of a case.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RxOptions {
    #[serde(default)]
    pub track_gris: Option<usize>,
    #[serde(default)]
    pub cri_blank: Option<bool>,
    /// Give the receiver its initial position (narrows acquisition). Default true.
    #[serde(default)]
    pub approx_position: Option<bool>,
    #[serde(default)]
    pub fit_window_us: Option<(f64, f64)>,
    #[serde(default)]
    pub acq_threshold_db: Option<f64>,
    #[serde(default)]
    pub propagation_ecd_us: Option<f64>,
    /// Decode the data channel (the chains' LDC configurations).
    #[serde(default)]
    pub ldc: bool,
}

/// A conformance case (the `[conformance]` table).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConformanceCase {
    pub id: String,
    pub title: String,
    pub category: String,
    pub intent: String,
    /// Standards the case is modelled on (texts not consulted).
    pub modelled_on: Vec<String>,
    /// What must be confirmed against the standard.
    pub to_confirm: Vec<String>,
    #[serde(default)]
    pub requirements: Vec<String>,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub interferers: Vec<String>,
    #[serde(default)]
    pub receiver: RxOptions,
    #[serde(default)]
    pub variations: Vec<Variation>,
    #[serde(default)]
    pub criteria: Vec<Criterion>,
    /// Extra metrics to compute even if no criterion uses them.
    #[serde(default)]
    pub report: Vec<String>,
}

/// Categories the library is expected to cover.
pub const CATEGORIES: &[&str] = &[
    "acquisition_sensitivity",
    "acquisition_time",
    "dynamic_range",
    "cri_rejection",
    "cw_rejection",
    "skywave_rejection",
    "ecd_tolerance",
    "cycle_identification",
    "blink_detection",
    "data_channel",
    "dynamics",
    "timing_accuracy",
    "position_accuracy",
];

/// A case file: the case and the scenario source it lives in.
#[derive(Clone, Debug)]
pub struct ConformanceFile {
    pub case: ConformanceCase,
    /// The scenario tree without the `[conformance]` table.
    pub scenario: toml::Table,
    pub path: Option<PathBuf>,
}

impl ConformanceFile {
    pub fn from_toml(src: &str) -> Result<ConformanceFile, String> {
        let mut t: toml::Table = toml::from_str(src).map_err(|e| format!("case: {e}"))?;
        let c = t
            .remove("conformance")
            .ok_or("case file has no [conformance] table")?;
        let case: ConformanceCase = c
            .try_into()
            .map_err(|e: toml::de::Error| format!("[conformance]: {e}"))?;
        let f = ConformanceFile {
            case,
            scenario: t,
            path: None,
        };
        f.validate()?;
        Ok(f)
    }

    pub fn load(path: &Path) -> Result<ConformanceFile, String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut f = Self::from_toml(&src).map_err(|e| format!("{}: {e}", path.display()))?;
        f.path = Some(path.to_path_buf());
        Ok(f)
    }

    /// Structural checks: id, illustrative limits, variation labels, scenario builds.
    pub fn validate(&self) -> Result<(), String> {
        let c = &self.case;
        if c.id.is_empty() {
            return Err("case id is empty".into());
        }
        if c.modelled_on.is_empty() || c.to_confirm.is_empty() {
            return Err(format!(
                "{}: modelled_on and to_confirm must be stated",
                c.id
            ));
        }
        let mut labels = BTreeSet::new();
        for v in &c.variations {
            if !labels.insert(v.label.as_str()) {
                return Err(format!("{}: duplicate variation {:?}", c.id, v.label));
            }
        }
        for k in &c.criteria {
            if !k.illustrative {
                return Err(format!(
                    "{}: criterion {} is marked non-illustrative; limits in this library are \
                     not taken from a standard",
                    c.id,
                    k.key()
                ));
            }
            for v in &k.variations {
                if !labels.contains(v.as_str()) {
                    return Err(format!("{}: criterion {} names unknown variation {v:?}", c.id, k.key()));
                }
            }
        }
        for v in self.variation_labels() {
            let s = self.scenario_text(v.as_deref(), &BTreeMap::new())?;
            Scenario::from_toml(&s)
                .and_then(|sc| sc.build().map(|_| ()))
                .map_err(|e| format!("{} variation {v:?}: {e}", c.id))?;
        }
        Ok(())
    }

    /// Variation labels (`None` = the base scenario when there are no variations).
    pub fn variation_labels(&self) -> Vec<Option<String>> {
        if self.case.variations.is_empty() {
            vec![None]
        } else {
            self.case
                .variations
                .iter()
                .map(|v| Some(v.label.clone()))
                .collect()
        }
    }

    /// Scenario TOML for a variation plus extra overrides (applied after the variation).
    pub fn scenario_text(
        &self,
        variation: Option<&str>,
        extra: &BTreeMap<String, toml::Value>,
    ) -> Result<String, String> {
        let mut t = toml::Value::Table(self.scenario.clone());
        if let Some(l) = variation {
            let v = self
                .case
                .variations
                .iter()
                .find(|v| v.label == l)
                .ok_or_else(|| format!("{}: no variation {l:?}", self.case.id))?;
            for (p, x) in &v.set {
                set_path(&mut t, p, x.clone())?;
            }
        }
        for (p, x) in extra {
            set_path(&mut t, p, x.clone())?;
        }
        toml::to_string(&t).map_err(|e| e.to_string())
    }
}

/// Load every `*.toml` case of a directory, sorted by id.
pub fn load_library(dir: &Path) -> Result<Vec<ConformanceFile>, String> {
    let mut out = Vec::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    for p in paths {
        out.push(ConformanceFile::load(&p)?);
    }
    out.sort_by(|a, b| a.case.id.cmp(&b.case.id));
    let mut ids = BTreeSet::new();
    for f in &out {
        if !ids.insert(f.case.id.clone()) {
            return Err(format!("duplicate case id {}", f.case.id));
        }
    }
    Ok(out)
}

/// Set `path` (`a.b[0].c`) in a TOML tree, creating tables as needed.
pub fn set_path(root: &mut toml::Value, path: &str, value: toml::Value) -> Result<(), String> {
    enum Seg {
        Key(String),
        Idx(usize),
    }
    let mut segs = Vec::new();
    for part in path.split('.') {
        let (name, rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if !name.is_empty() {
            segs.push(Seg::Key(name.to_string()));
        }
        let mut r = rest;
        while let Some(s) = r.strip_prefix('[') {
            let e = s.find(']').ok_or_else(|| format!("bad path {path:?}"))?;
            segs.push(Seg::Idx(
                s[..e].parse().map_err(|_| format!("bad index in {path:?}"))?,
            ));
            r = &s[e + 1..];
        }
    }
    let mut cur = root;
    for (i, s) in segs.iter().enumerate() {
        let last = i + 1 == segs.len();
        match s {
            Seg::Key(k) => {
                let t = cur
                    .as_table_mut()
                    .ok_or_else(|| format!("{path}: {k:?} is not inside a table"))?;
                if last {
                    t.insert(k.clone(), value);
                    return Ok(());
                }
                cur = t
                    .entry(k.clone())
                    .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            }
            Seg::Idx(n) => {
                let a = cur
                    .as_array_mut()
                    .ok_or_else(|| format!("{path}: index {n} into a non-array"))?;
                if *n == a.len() && last {
                    a.push(value);
                    return Ok(());
                }
                let len = a.len();
                let slot = a
                    .get_mut(*n)
                    .ok_or_else(|| format!("{path}: index {n} out of range (len {len})"))?;
                if last {
                    *slot = value;
                    return Ok(());
                }
                cur = slot;
            }
        }
    }
    Err(format!("empty path {path:?}"))
}

// ---------------------------------------------------------------------------------------
// Simulation and evaluation

/// One simulated stimulus.
#[derive(Clone, Debug)]
pub struct SimRun {
    pub scenario: Scenario,
    pub channels: Vec<Vec<f64>>,
    pub truth: Vec<TruthRecord>,
    pub start: AbsTime,
}

impl SimRun {
    pub fn chains(&self) -> Vec<Chain> {
        self.scenario.chains.iter().map(|c| c.chain.clone()).collect()
    }
}

/// Run a scenario text in memory.
pub fn simulate(text: &str) -> Result<SimRun, String> {
    let scenario = Scenario::from_toml(text)?;
    let start = scenario.start()?;
    let (sink, truth, _) = crate::run_to_memory(text)?;
    Ok(SimRun {
        scenario,
        channels: sink.channels,
        truth,
        start,
    })
}

/// The case's targets resolved to (chain index, station index, "chain/station").
pub fn targets(case: &ConformanceCase, chains: &[Chain]) -> Result<Vec<(usize, usize, String)>, String> {
    let mut out = Vec::new();
    if case.targets.is_empty() {
        for (ci, ch) in chains.iter().enumerate() {
            if case.interferers.contains(&ch.name) {
                continue;
            }
            for (si, st) in ch.stations.iter().enumerate() {
                if st.status == StationStatus::On {
                    out.push((ci, si, format!("{}/{}", ch.name, st.id)));
                }
            }
        }
    } else {
        for t in &case.targets {
            let (c, s) = t
                .split_once('/')
                .ok_or_else(|| format!("target {t:?}: expected chain/station"))?;
            let ci = chains
                .iter()
                .position(|ch| ch.name == c)
                .ok_or_else(|| format!("target {t:?}: no chain {c:?}"))?;
            let si = chains[ci]
                .stations
                .iter()
                .position(|st| st.id == s)
                .ok_or_else(|| format!("target {t:?}: no station {s:?}"))?;
            out.push((ci, si, t.clone()));
        }
    }
    if out.is_empty() {
        return Err(format!("{}: no targets", case.id));
    }
    Ok(out)
}

/// The receiver configuration for a case on a scenario.
pub fn rx_config(case: &ConformanceCase, sc: &Scenario) -> Result<RxConfig, String> {
    let o = &case.receiver;
    let mut cfg = RxConfig {
        output_scale_uv_m: sc.scene.params.output_scale_uv_m,
        ..Default::default()
    };
    let loops: Vec<usize> = sc
        .outputs
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, crate::antenna::OutputChannel::HLoop { .. }))
        .map(|(i, _)| i)
        .collect();
    cfg.antenna = if loops.len() >= 2 {
        Antenna::CrossedLoop {
            a: loops[0],
            b: loops[1],
            mode: LoopMode::BeamSteer,
        }
    } else {
        Antenna::EField { channel: 0 }
    };
    if let Some(v) = o.track_gris {
        cfg.track_gris = v;
    }
    if let Some(v) = o.cri_blank {
        cfg.cri.blank = v;
    }
    if o.approx_position.unwrap_or(true) {
        cfg.approx_position = Some(sc.receiver.state(0.0).pos);
    }
    if let Some(v) = o.fit_window_us {
        cfg.fit_window_us = v;
    }
    if let Some(v) = o.acq_threshold_db {
        cfg.acq_threshold_db = v;
    }
    if let Some(v) = o.propagation_ecd_us {
        cfg.propagation_ecd_us = v;
    }
    if o.ldc {
        cfg.ldc = sc.ldc_configs()?;
    }
    Ok(cfg)
}

fn wants(case: &ConformanceCase, criteria: &[Criterion], m: &str) -> bool {
    criteria.iter().any(|c| c.metric == m) || case.report.iter().any(|r| r == m)
}

fn worst(v: impl Iterator<Item = f64>) -> f64 {
    v.filter(|x| x.is_finite()).fold(f64::NAN, |a, b| if a.is_nan() { b } else { a.max(b) })
}

/// Blink decision per (target, group) from the pulse amplitudes of a group.
const BLINK_RATIO: f64 = 0.35;

/// Evaluate the software receiver on a simulated stimulus. `criteria` (normally the
/// case's, plus any campaign bars) decides which costly metrics are computed.
pub fn evaluate_software(
    case: &ConformanceCase,
    criteria: &[Criterion],
    run: &SimRun,
) -> Result<(Metrics, RxOutput), String> {
    let chains = run.chains();
    let tg = targets(case, &chains)?;
    let cfg = rx_config(case, &run.scenario)?;
    let fs = run.scenario.scene.params.fs_hz;
    let input = RxInput::new(run.channels.clone(), fs, run.start);
    let (receiver, out) = rx::process(&input, &chains, cfg.clone())?;
    let mut m = Metrics::new();
    let n = tg.len() as f64;
    let acquired = |acq: &[rx::Acquisition], ci: usize, si: usize| {
        acq.iter()
            .any(|a| a.chain_index == ci && a.station_index == si && a.found)
    };
    m.insert(
        "acquired_fraction".into(),
        tg.iter().filter(|t| acquired(&out.acquisitions, t.0, t.1)).count() as f64 / n,
    );
    m.insert(
        "tracked_fraction".into(),
        tg.iter()
            .filter(|t| {
                out.measurements
                    .iter()
                    .any(|x| x.chain_index == t.0 && x.station_index == t.1)
            })
            .count() as f64
            / n,
    );
    let errs = rx::toa_errors(&out.measurements, &run.truth);
    let mut cyc = 0.0;
    let mut slips = 0.0;
    for (ci, si, key) in &tg {
        let (cn, sn) = (&chains[*ci].name, &chains[*ci].stations[*si].id);
        let es: Vec<&rx::StationErrors> = errs.iter().filter(|e| &e.chain == cn && &e.station == sn).collect();
        if let Some(e) = es.first() {
            m.insert(format!("toa_rms_ns@{key}"), e.toa_rms_ns);
            m.insert(format!("toa_bias_ns@{key}"), e.toa_mean_ns.abs());
            m.insert(format!("toa_std_ns@{key}"), e.toa_std_ns);
            m.insert(format!("ecd_rms_us@{key}"), e.ecd_rms_err_us);
            m.insert(format!("cycle_errors@{key}"), e.cycle_errors as f64);
            m.insert(format!("snr_error_db@{key}"), (e.snr_meas_db - e.snr_truth_db).abs());
            m.insert(format!("snr_truth_db@{key}"), e.snr_truth_db);
            cyc += es.iter().map(|e| e.cycle_errors as f64).sum::<f64>();
        }
        let s = out
            .measurements
            .iter()
            .filter(|x| x.chain_index == *ci && x.station_index == *si && x.cycle_slip)
            .count() as f64;
        m.insert(format!("cycle_slips@{key}"), s);
        slips += s;
    }
    for name in ["toa_rms_ns", "toa_bias_ns", "toa_std_ns", "ecd_rms_us", "snr_error_db"] {
        let w = worst(tg.iter().filter_map(|t| m.get(&format!("{name}@{}", t.2)).copied()));
        m.insert(name.into(), w);
    }
    m.insert("cycle_errors".into(), cyc);
    m.insert("cycle_slips".into(), slips);
    if let Some(v) = m.get(&format!("snr_truth_db@{}", tg[0].2)).copied() {
        m.insert("snr_truth_db".into(), v);
    } else if let Some(r) = run
        .truth
        .iter()
        .find(|r| r.chain == chains[tg[0].0].name && r.station == chains[tg[0].0].stations[tg[0].1].id)
    {
        m.insert("snr_truth_db".into(), r.snr_db);
    }
    let corr = rx::Corrections::default();
    if wants(case, criteria, "position_error_m") {
        let v = rx::position_solution(&out.measurements, &chains, run.start, &corr, &rx::PositionConfig {
            initial: cfg.approx_position,
            ..Default::default()
        })
        .ok()
        .filter(|s| s.converged)
        .and_then(|s| rx::position_error_m(s.position, &run.truth))
        .unwrap_or(f64::NAN);
        m.insert("position_error_m".into(), v);
    }
    if wants(case, criteria, "tdoa_position_error_m") {
        let v = rx::tdoa_fix(&out.measurements, &chains, run.start, &corr, cfg.approx_position)
            .ok()
            .filter(|s| s.converged)
            .and_then(|s| rx::position_error_m(s.position, &run.truth))
            .unwrap_or(f64::NAN);
        m.insert("tdoa_position_error_m".into(), v);
    }
    if wants(case, criteria, "time_error_ns") {
        let pos = run.scenario.receiver.state(0.0).pos;
        let ts = rx::time_solution(&out.measurements, &chains, run.start, pos, &corr);
        let ce = rx::clock_errors(&ts, &run.truth);
        let w = worst(
            ce.iter()
                .filter(|c| tg.iter().any(|t| chains[t.0].name == c.chain && chains[t.0].stations[t.1].id == c.station))
                .map(|c| c.error_ns.abs()),
        );
        m.insert("time_error_ns".into(), w);
    }
    if wants(case, criteria, "acq_time_s") {
        m.insert("acq_time_s".into(), acquisition_time(&run.channels, fs, run.start, &chains, &cfg, &tg)?);
    }
    if wants(case, criteria, "blink_detection_rate") || wants(case, criteria, "blink_false_alarm_rate") {
        let (det, fa) = blink_rates(&receiver, &out, &chains, run, &tg);
        m.insert("blink_detection_rate".into(), det);
        m.insert("blink_false_alarm_rate".into(), fa);
    }
    if case.receiver.ldc {
        let mut msgs = 0.0;
        let mut failed = 0.0;
        let mut auth_bad = 0.0;
        for l in &out.ldc {
            if !tg.iter().any(|t| t.0 == l.chain_index && t.1 == l.station_index) {
                continue;
            }
            msgs += l.report.messages.len() as f64;
            failed += l.report.frames_failed as f64;
            auth_bad += l.auth.iter().filter(|a| a.verified == Some(false)).count() as f64;
        }
        m.insert("ldc_messages".into(), msgs);
        m.insert("ldc_frames_failed".into(), failed);
        m.insert("ldc_auth_failures".into(), auth_bad);
    }
    Ok((m, out))
}

/// Shortest prefix (multiples of two GRIs of the slowest target chain) at which every
/// target is acquired; NaN when not within the record.
fn acquisition_time(
    channels: &[Vec<f64>],
    fs: f64,
    start: AbsTime,
    chains: &[Chain],
    cfg: &RxConfig,
    tg: &[(usize, usize, String)],
) -> Result<f64, String> {
    let gri = tg
        .iter()
        .map(|t| chains[t.0].gri_us as f64 * 1e-6)
        .fold(0.0, f64::max);
    let total = channels.first().map(|c| c.len()).unwrap_or(0) as f64 / fs;
    let mut k = 1;
    loop {
        let len_s = 2.0 * gri * k as f64 + 0.012; // + one group span
        if len_s > total {
            return Ok(f64::NAN);
        }
        let n = (len_s * fs) as usize;
        let input = RxInput::new(channels.iter().map(|c| c[..n].to_vec()).collect(), fs, start);
        let r = rx::Receiver::new(&input, chains, cfg.clone())?;
        let acq = r.acquire();
        if tg.iter().all(|t| {
            acq.iter()
                .any(|a| a.chain_index == t.0 && a.station_index == t.1 && a.found)
        }) {
            return Ok(len_s);
        }
        k += 1;
    }
}

/// Blink windows (scene s) of a station from its faults.
fn blink_windows(ch: &Chain, si: usize) -> Vec<(f64, f64)> {
    ch.stations[si]
        .faults
        .iter()
        .filter_map(|f| match f {
            StationFault::Blink { from_s, to_s } => Some((*from_s, *to_s)),
            _ => None,
        })
        .collect()
}

fn blink_rates(
    receiver: &rx::Receiver,
    out: &RxOutput,
    chains: &[Chain],
    run: &SimRun,
    tg: &[(usize, usize, String)],
) -> (f64, f64) {
    let (mut hit, mut pos, mut fa, mut neg) = (0usize, 0usize, 0usize, 0usize);
    for (ci, si, _) in tg {
        let ch = &chains[*ci];
        let wins = blink_windows(ch, *si);
        let groups: Vec<i64> = run
            .truth
            .iter()
            .filter(|r| r.chain == ch.name && r.station == ch.stations[*si].id)
            .map(|r| r.group)
            .collect();
        let (Some(g0), Some(g1)) = (groups.iter().min(), groups.iter().max()) else {
            continue;
        };
        for track in out
            .tracks
            .iter()
            .filter(|t| t.chain_index == *ci && t.station_index == *si)
        {
            for g in *g0..=*g1 {
                let Some(pt) = receiver.pulse_timing(track, g, &[]) else {
                    continue;
                };
                let a: Vec<f64> = pt.pulses.iter().map(|p| p.amplitude_rel).collect();
                if a.len() < 8 {
                    continue;
                }
                let first = (a[0] + a[1]) / 2.0;
                let rest = a[2..8].iter().sum::<f64>() / 6.0;
                let declared = rest > 0.0 && first < BLINK_RATIO * rest;
                let t = rx::nominal_emission_s(ch, *si, g, run.start);
                let truth = wins.iter().any(|(a, b)| t >= *a && t < *b);
                if truth {
                    pos += 1;
                    hit += declared as usize;
                } else {
                    neg += 1;
                    fa += declared as usize;
                }
            }
        }
    }
    (
        if pos > 0 { hit as f64 / pos as f64 } else { f64::NAN },
        if neg > 0 { fa as f64 / neg as f64 } else { f64::NAN },
    )
}

/// Evaluate an external receiver's imported records against the simulation truth.
pub fn evaluate_external(
    case: &ConformanceCase,
    run: &SimRun,
    obs: &[ObsRecord],
    max_dt_s: f64,
) -> Result<Metrics, String> {
    let chains = run.chains();
    let tg = targets(case, &chains)?;
    let reference = obs_from_truth(&run.truth, &chains, run.start);
    let cmp = compare_records(obs, &reference, &chains, max_dt_s);
    let mut m = Metrics::new();
    let key_of = |r: &ObsRecord| {
        crate::field::importers::canonical_key(r.chain.as_deref(), r.station.as_deref(), &chains)
    };
    let seen = tg
        .iter()
        .filter(|t| {
            obs.iter()
                .any(|r| !r.flags.contains(flags::INVALID) && key_of(r) == Some((t.0, t.1)))
        })
        .count();
    m.insert("acquired_fraction".into(), seen as f64 / tg.len() as f64);
    let per = |q: &str, f: &dyn Fn(&crate::field::importers::QuantityStats) -> f64, m: &mut Metrics, name: &str| {
        let mut vals = Vec::new();
        for t in &tg {
            if let Some(s) = cmp.get(&t.2, q) {
                m.insert(format!("{name}@{}", t.2), f(s));
                vals.push(f(s));
            }
        }
        if !vals.is_empty() {
            m.insert(name.into(), worst(vals.into_iter()));
        }
    };
    per("toa_ns", &|s| s.rms, &mut m, "toa_rms_ns");
    per("toa_ns", &|s| s.mean.abs(), &mut m, "toa_bias_ns");
    per("td_ns", &|s| s.rms, &mut m, "td_rms_ns");
    per("ecd_us", &|s| s.rms, &mut m, "ecd_rms_us");
    per("snr_db", &|s| s.mean.abs(), &mut m, "snr_error_db");
    m.insert("cycle_errors".into(), cmp.cycle_errors as f64);
    if let Some(p) = cmp.get("fix", "position_m") {
        m.insert("position_error_m".into(), p.abs95);
    }
    let tim: Vec<f64> = obs.iter().filter_map(|r| r.timing_ns).map(f64::abs).collect();
    if !tim.is_empty() {
        m.insert("time_error_ns".into(), percentile(tim, 0.95));
    }
    // blink flags
    let t0 = unix_s_from_abs(run.start);
    let (mut hit, mut pos, mut fa, mut neg) = (0usize, 0usize, 0usize, 0usize);
    for r in obs {
        let (Some(k), Some(t)) = (key_of(r), r.t_unix_s) else { continue };
        if !tg.iter().any(|x| (x.0, x.1) == k) {
            continue;
        }
        let ts = t - t0;
        let inside = blink_windows(&chains[k.0], k.1).iter().any(|(a, b)| ts >= *a && ts < *b);
        let flagged = r.flags.contains(flags::BLINK);
        if inside {
            pos += 1;
            hit += flagged as usize;
        } else {
            neg += 1;
            fa += flagged as usize;
        }
    }
    if pos > 0 {
        m.insert("blink_detection_rate".into(), hit as f64 / pos as f64);
    }
    if neg > 0 && pos > 0 {
        m.insert("blink_false_alarm_rate".into(), fa as f64 / neg as f64);
    }
    Ok(m)
}

/// Verdict of one criterion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Pass,
    Fail,
    /// The metric was not available (receiver did not report it, or NaN).
    NotEvaluated,
}

/// A scored criterion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scored {
    pub metric: String,
    pub op: Op,
    pub limit: f64,
    pub value: Option<f64>,
    pub verdict: Verdict,
    pub illustrative: bool,
    pub note: String,
}

/// Score metrics of one variation against criteria.
pub fn score(criteria: &[Criterion], variation: &str, m: &Metrics) -> Vec<Scored> {
    criteria
        .iter()
        .filter(|c| c.applies_to(variation))
        .map(|c| {
            let v = m.get(&c.key()).copied().filter(|x| x.is_finite());
            Scored {
                metric: c.key(),
                op: c.op,
                limit: c.limit,
                value: v,
                verdict: match v {
                    None => Verdict::NotEvaluated,
                    Some(x) if c.op.holds(x, c.limit) => Verdict::Pass,
                    Some(_) => Verdict::Fail,
                },
                illustrative: c.illustrative,
                note: c.note.clone(),
            }
        })
        .collect()
}

/// Overall verdict: FAIL if any failed; PASS if at least one passed and none failed or
/// were not evaluated; otherwise NOT_EVALUATED.
pub fn overall(s: &[Scored]) -> Verdict {
    if s.iter().any(|x| x.verdict == Verdict::Fail) {
        Verdict::Fail
    } else if !s.is_empty() && s.iter().all(|x| x.verdict == Verdict::Pass) {
        Verdict::Pass
    } else {
        Verdict::NotEvaluated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_path_edits_tables_arrays_and_creates_tables() {
        let mut v: toml::Value = toml::from_str(
            "[scene]\nduration_s = 1.0\n[[interference]]\ntype = \"wgn\"\nrms_uv_m_20khz = 1.0\n",
        )
        .unwrap();
        set_path(&mut v, "scene.duration_s", toml::Value::Float(0.5)).unwrap();
        set_path(&mut v, "interference[0].rms_uv_m_20khz", toml::Value::Float(9.0)).unwrap();
        set_path(&mut v, "propagation.sky.model", toml::Value::String("fixed".into())).unwrap();
        set_path(&mut v, "interference[1]", toml::Value::Table(toml::Table::new())).unwrap();
        assert!(set_path(&mut v, "interference[5].x", toml::Value::Integer(1)).is_err());
        assert_eq!(v["scene"]["duration_s"].as_float(), Some(0.5));
        assert_eq!(v["interference"][0]["rms_uv_m_20khz"].as_float(), Some(9.0));
        assert_eq!(v["propagation"]["sky"]["model"].as_str(), Some("fixed"));
        assert_eq!(v["interference"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn score_and_overall() {
        let c = vec![
            Criterion {
                metric: "toa_rms_ns".into(),
                target: None,
                op: Op::Le,
                limit: 10.0,
                variations: vec![],
                illustrative: true,
                note: String::new(),
            },
            Criterion {
                metric: "acquired_fraction".into(),
                target: None,
                op: Op::Ge,
                limit: 1.0,
                variations: vec!["hi".into()],
                illustrative: true,
                note: String::new(),
            },
        ];
        let mut m = Metrics::new();
        m.insert("toa_rms_ns".into(), 5.0);
        let s = score(&c, "lo", &m);
        assert_eq!(s.len(), 1);
        assert_eq!(overall(&s), Verdict::Pass);
        let s = score(&c, "hi", &m);
        assert_eq!(s[1].verdict, Verdict::NotEvaluated);
        assert_eq!(overall(&s), Verdict::NotEvaluated);
        m.insert("acquired_fraction".into(), 0.5);
        assert_eq!(overall(&score(&c, "hi", &m)), Verdict::Fail);
    }

    #[test]
    fn library_loads_covers_categories_and_every_limit_is_illustrative() {
        let lib = load_library(&library_dir()).unwrap();
        assert!(lib.len() >= 12 && lib.len() <= 20, "{} cases", lib.len());
        for cat in CATEGORIES {
            assert!(
                lib.iter().any(|f| f.case.category == *cat),
                "no case for category {cat}"
            );
        }
        for f in &lib {
            assert!(f.case.criteria.iter().all(|c| c.illustrative));
            assert!(f.case.modelled_on.iter().all(|m| m.contains("not consulted")), "{}", f.case.id);
            assert!(!f.case.criteria.is_empty(), "{}", f.case.id);
        }
    }

    #[test]
    fn non_illustrative_limit_is_refused() {
        let src = std::fs::read_to_string(library_dir().join("acq-sensitivity.toml")).unwrap();
        let bad = src.replacen("illustrative = true", "illustrative = false", 1);
        assert!(ConformanceFile::from_toml(&bad).unwrap_err().contains("non-illustrative"));
    }
}
