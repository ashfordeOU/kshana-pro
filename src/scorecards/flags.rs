// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Spoof-flag and integrity-flag efficacy scorecard (research backlog B1301, E13)
//!
//! Receivers raise their own spoofing, jamming and integrity indications, and users rely on
//! them. Whether those flags fire, under which conditions, and how often they fire when
//! nothing is wrong is seldom measured: a low-cost timing module's spoofing flag that never
//! fired during a spoofing test, or an integrity flag that read normal while the position was
//! wrong, are reported field experiences. This module measures flag behaviour per receiver
//! model and firmware from logs recorded under test-kit scenarios
//! (`kshana-testkit-scenario/1`, [`crate::testkit::spec`]):
//!
//! * **Indicators** are declared before scoring, per model, from the vendor's own
//!   documentation ([`Indicator`]: name, meaning, raise threshold, where it is documented, and
//!   the preconditions the vendor states). The list doubles as an index of the documented
//!   flags of each model.
//! * **Detection rate** per indicator and threat class (`jamming-cw`, `jamming-broadband`,
//!   `jamming-chirp`, `jamming-pulsed`, `spoofing-<technique>`, `degradation-<type>`):
//!   runs where the indicator was raised within `detect_within_s` of onset, over runs of that
//!   class, with a Wilson 95 % interval and the median latency.
//! * **Wrong-class indications**: an indicator raised under a class it should not respond to
//!   (a CW-jamming indicator during broadband jamming and the reverse, a spoofing flag under
//!   jamming): the CW-vs-broadband indicator truth test.
//! * **False alarms**: raised epochs in clean runs and before the first onset of threat runs.
//! * **Preconditions**: each run states its conditions (for example `constellations = single`
//!   or `multi`, `start = genuine-first` or `spoof-first`). Where an indicator fired under one
//!   set of conditions and not under another, the scorecard says so and checks whether the
//!   vendor's stated preconditions disclose it.
//! * **Never fired**: an indicator expected to respond that did not fire in any run of its
//!   classes is reported as such.
//! * **Flag normal while wrong** ([`flag_normal_while_wrong`]): episodes where the output was
//!   valid, every declared indicator read normal, and the position or time was wrong beyond
//!   the bar. The detector is generic and reads any log in the flag-log form.
//!
//! Logs are the flag log ([`LOG_FORMAT`], JSON, any named indicators) or the PICS device log
//! (`kshana-pics-device-log/1`, whose `jamming`, `spoofing` and `integrity` become indicators of
//! those names). Truth is the scenario's trajectory at each epoch time.
//!
//! The bars are stated before scoring and hashed into the scorecard. Results are measured
//! rates under the stated scenarios and bars: test evidence, never a statement that a receiver
//! meets a standard. Evidence: **MODELLED** until a campaign on real receivers is run; the
//! scorer is checked on synthetic logs (`tests/receiver_flag_scorecards.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::pics::log::DeviceLog;
use crate::testkit::spec::{Spec, ThreatType};

/// The request format.
pub const REQUEST_FORMAT: &str = "kshana-pro-flag-efficacy-request/1";
/// The flag log format.
pub const LOG_FORMAT: &str = "kshana-flag-log/1";
/// The scorecard format.
pub const FORMAT: &str = "kshana-pro-flag-efficacy/1";
/// The scoring method; the rules in this module are constants of this version.
pub const METHOD_ID: &str = "kshana-pro/flag-efficacy/v1";
/// The scorecard file in a signed directory.
pub const RESULT_FILE: &str = "flag-efficacy.json";
pub const EVIDENCE: &str = "MODELLED: the scorer is checked on synthetic logs; no real \
receiver campaign has been scored in this release. Rates are measured on the logs supplied.";
pub const WORDING: &str = "Measured flag behaviour under the scenarios and bars stated before \
scoring. Test evidence for the runs supplied; not a statement that any receiver meets a \
standard, and not a certification.";
/// The 95 % normal quantile used for the Wilson interval.
pub const Z95: f64 = 1.959_963_985;

/// Round to 9 significant digits (every float that feeds a hash or a signature).
pub fn sig9(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    format!("{x:.8e}").parse().unwrap_or(x)
}

fn sig9o(x: Option<f64>) -> Option<f64> {
    x.map(sig9)
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

// ---------------------------------------------------------------------------------------
// The flag log
// ---------------------------------------------------------------------------------------

/// The unit under test.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    pub model: String,
    #[serde(default)]
    pub firmware: Option<String>,
    #[serde(default)]
    pub serial: Option<String>,
    /// `module`, `board`, `handset`, `timing-module`, `receiver` (free text).
    #[serde(default)]
    pub class: Option<String>,
}

/// One indicator value: a flag, a level, or a word.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IndValue {
    Bool(bool),
    Num(f64),
    Text(String),
}

/// One epoch of a flag log.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlagEpoch {
    /// Seconds from the scenario start.
    pub t_s: f64,
    #[serde(default)]
    pub fix_valid: Option<bool>,
    #[serde(default)]
    pub lat_deg: Option<f64>,
    #[serde(default)]
    pub lon_deg: Option<f64>,
    /// Device time minus the reference, ns (measured outside the device).
    #[serde(default)]
    pub time_error_ns: Option<f64>,
    /// The receiver's indicators at this epoch, by name.
    #[serde(default)]
    pub ind: BTreeMap<String, IndValue>,
}

/// A flag log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlagLog {
    pub format: String,
    pub unit: Unit,
    /// The test-kit scenario (pack `<pack>/<id>`, bare id, or a path).
    pub scenario: String,
    /// The run's conditions (`constellations = "single"`, `start = "genuine-first"`, ...).
    #[serde(default)]
    pub preconditions: BTreeMap<String, String>,
    #[serde(default)]
    pub source: Option<String>,
    pub epochs: Vec<FlagEpoch>,
}

impl FlagLog {
    /// Parse a flag log or a PICS device log (by its `format`).
    pub fn parse(src: &str) -> Result<FlagLog, String> {
        let v: serde_json::Value =
            serde_json::from_str(src).map_err(|e| format!("flag log: {e}"))?;
        let l = match v["format"].as_str() {
            Some(LOG_FORMAT) => {
                serde_json::from_value::<FlagLog>(v).map_err(|e| format!("flag log: {e}"))?
            }
            Some(crate::pics::log::FORMAT) => FlagLog::from_device_log(&DeviceLog::parse(src)?),
            other => {
                return Err(format!(
                    "flag log: format {other:?} (expected {LOG_FORMAT} or {})",
                    crate::pics::log::FORMAT
                ))
            }
        };
        l.validate()?;
        Ok(l)
    }

    /// A PICS device log as a flag log: `jamming`, `spoofing` and `integrity` become
    /// indicators of those names; the device name is the model.
    pub fn from_device_log(d: &DeviceLog) -> FlagLog {
        let epochs = d
            .epochs
            .iter()
            .map(|e| {
                let mut ind = BTreeMap::new();
                if let Some(j) = e.jamming {
                    ind.insert("jamming".into(), IndValue::Bool(j));
                }
                if let Some(s) = e.spoofing {
                    ind.insert("spoofing".into(), IndValue::Bool(s));
                }
                if let Some(i) = &e.integrity {
                    ind.insert("integrity".into(), IndValue::Text(i.clone()));
                }
                FlagEpoch {
                    t_s: e.t_s,
                    fix_valid: e.fix_valid,
                    lat_deg: e.lat_deg,
                    lon_deg: e.lon_deg,
                    time_error_ns: e.time_error_ns,
                    ind,
                }
            })
            .collect();
        FlagLog {
            format: LOG_FORMAT.into(),
            unit: Unit {
                model: d.device.name.clone(),
                firmware: d.device.firmware.clone(),
                serial: d.device.serial.clone(),
                class: None,
            },
            scenario: d.scenario.clone(),
            preconditions: BTreeMap::new(),
            source: Some(format!(
                "pics:{}",
                d.source.clone().unwrap_or_else(|| "json".into())
            )),
            epochs,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.epochs.is_empty() {
            return Err("flag log: no epochs".into());
        }
        if self.unit.model.trim().is_empty() {
            return Err("flag log: unit.model is needed".into());
        }
        let mut last = f64::NEG_INFINITY;
        for e in &self.epochs {
            if !e.t_s.is_finite() || e.t_s < last {
                return Err(format!(
                    "flag log: epoch times must be finite and non-decreasing (at t = {})",
                    e.t_s
                ));
            }
            last = e.t_s;
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("flag log serialises")
    }
}

// ---------------------------------------------------------------------------------------
// The request
// ---------------------------------------------------------------------------------------

/// What an indicator is documented to mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Meaning {
    Spoofing,
    Jamming,
    JammingCw,
    JammingBroadband,
    Integrity,
}

impl Meaning {
    pub fn as_str(self) -> &'static str {
        match self {
            Meaning::Spoofing => "spoofing",
            Meaning::Jamming => "jamming",
            Meaning::JammingCw => "jamming_cw",
            Meaning::JammingBroadband => "jamming_broadband",
            Meaning::Integrity => "integrity",
        }
    }

    /// Whether an indicator of this meaning should respond to a threat class: `Some(true)`
    /// expected, `Some(false)` it should stay quiet, `None` either is acceptable.
    pub fn expects(self, class: &str) -> Option<bool> {
        let jam = class.starts_with("jamming-");
        let spoof = class.starts_with("spoofing-");
        match self {
            _ if class == CLEAN => Some(false),
            Meaning::Spoofing => Some(spoof),
            Meaning::Jamming if jam => Some(true),
            Meaning::Jamming if spoof => None,
            Meaning::Jamming => Some(false),
            Meaning::JammingCw => match class {
                "jamming-cw" => Some(true),
                "jamming-broadband" => Some(false),
                _ if spoof => Some(false),
                _ => None,
            },
            Meaning::JammingBroadband => match class {
                "jamming-broadband" => Some(true),
                "jamming-cw" => Some(false),
                _ if spoof => Some(false),
                _ => None,
            },
            Meaning::Integrity if spoof => Some(true),
            Meaning::Integrity => None,
        }
    }
}

/// The class of a clean run (no threat).
pub const CLEAN: &str = "clean";

/// A vendor-documented indicator of one model, declared before scoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Indicator {
    pub model: String,
    /// The indicator's name in the log's `ind` map.
    pub name: String,
    pub meaning: Meaning,
    /// A numeric indicator is raised at or above this value (a flag: when `true`).
    #[serde(default)]
    pub raise_at: Option<f64>,
    /// A word indicator is raised at these values (default `caution`, `unsafe`, `failed`,
    /// `spoofed`, `jammed`).
    #[serde(default)]
    pub raise_values: Vec<String>,
    /// Where the vendor documents the indicator (document and section).
    #[serde(default)]
    pub documented_in: Option<String>,
    /// The preconditions the vendor states for it, as `key=value`
    /// (`constellations=multi`, `start=genuine-first`).
    #[serde(default)]
    pub preconditions: Vec<String>,
}

const DEFAULT_RAISED_WORDS: [&str; 5] = ["caution", "unsafe", "failed", "spoofed", "jammed"];

impl Indicator {
    /// Whether this indicator is raised at an epoch; `None` when the epoch does not carry it.
    pub fn raised(&self, e: &FlagEpoch) -> Option<bool> {
        Some(match e.ind.get(&self.name)? {
            IndValue::Bool(b) => *b,
            IndValue::Num(x) => *x >= self.raise_at.unwrap_or(1.0),
            IndValue::Text(t) => {
                let t = t.to_ascii_lowercase();
                if self.raise_values.is_empty() {
                    DEFAULT_RAISED_WORDS.contains(&t.as_str())
                } else {
                    self.raise_values.iter().any(|v| v.eq_ignore_ascii_case(&t))
                }
            }
        })
    }
}

/// One run: a log under a scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSpec {
    #[serde(default)]
    pub id: Option<String>,
    /// The log file (flag log or PICS device log).
    pub log: String,
    /// Overrides the log's scenario.
    #[serde(default)]
    pub scenario: Option<String>,
    /// Merged over the log's preconditions.
    #[serde(default)]
    pub preconditions: BTreeMap<String, String>,
}

/// The bars, stated before scoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bars {
    /// When the bars were written (ISO-8601 UTC), as the campaign owner states it.
    pub fixed_utc: Option<String>,
    /// An indicator counts as detecting a threat when raised within this of onset, s.
    pub detect_within_s: f64,
    /// Epochs before this (scenario time) are not counted for false alarms, s.
    pub false_alarm_skip_s: f64,
    /// Wrong output: a valid fix farther than this from truth, m (scenario's own when unset).
    pub misleading_h_error_m: Option<f64>,
    /// Wrong output: a time error beyond this, ns (scenario's own when unset).
    pub misleading_time_error_ns: Option<f64>,
    /// A rate is stated only with at least this many runs; fewer is "not assessed".
    pub min_runs: usize,
    /// Two precondition sets differ when their detection rates differ by at least this.
    pub precondition_gap: f64,
}

impl Default for Bars {
    fn default() -> Self {
        Bars {
            fixed_utc: None,
            detect_within_s: 60.0,
            false_alarm_skip_s: 0.0,
            misleading_h_error_m: None,
            misleading_time_error_ns: None,
            min_runs: 3,
            precondition_gap: 0.5,
        }
    }
}

impl Bars {
    fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !ok(self.detect_within_s) {
            return Err("bars: detect_within_s must be > 0".into());
        }
        if !(self.false_alarm_skip_s.is_finite() && self.false_alarm_skip_s >= 0.0) {
            return Err("bars: false_alarm_skip_s must be >= 0".into());
        }
        if self.misleading_h_error_m.is_some_and(|v| !ok(v))
            || self.misleading_time_error_ns.is_some_and(|v| !ok(v))
        {
            return Err("bars: misleading thresholds must be > 0".into());
        }
        if self.min_runs == 0 {
            return Err("bars: min_runs must be at least 1".into());
        }
        if !(self.precondition_gap > 0.0 && self.precondition_gap <= 1.0) {
            return Err("bars: precondition_gap is in (0, 1]".into());
        }
        Ok(())
    }
}

/// A flag-efficacy request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub format: String,
    pub title: String,
    #[serde(default)]
    pub bars: Bars,
    #[serde(rename = "indicator")]
    pub indicators: Vec<Indicator>,
    #[serde(rename = "run")]
    pub runs: Vec<RunSpec>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl Request {
    pub fn parse(src: &str) -> Result<Request, String> {
        let r: Request =
            toml::from_str(src).map_err(|e| format!("invalid flag-efficacy request: {e}"))?;
        r.validate()?;
        Ok(r)
    }

    pub fn from_file(path: &Path) -> Result<(Request, String), String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r = Request::parse(&src)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != REQUEST_FORMAT {
            return Err(format!("format must be '{REQUEST_FORMAT}'"));
        }
        self.bars.validate()?;
        if self.indicators.is_empty() {
            return Err("declare at least one [[indicator]] before scoring".into());
        }
        let mut seen = BTreeSet::new();
        for i in &self.indicators {
            if i.model.trim().is_empty() || i.name.trim().is_empty() {
                return Err("indicator: model and name are needed".into());
            }
            if !seen.insert((i.model.clone(), i.name.clone())) {
                return Err(format!(
                    "indicator '{}' of model '{}' is declared twice",
                    i.name, i.model
                ));
            }
            if i.raise_at.is_some_and(|v| !v.is_finite()) {
                return Err(format!("indicator '{}': raise_at must be finite", i.name));
            }
            for p in &i.preconditions {
                if !p.contains('=') {
                    return Err(format!(
                        "indicator '{}': precondition '{p}' must be key=value",
                        i.name
                    ));
                }
            }
        }
        if self.runs.is_empty() {
            return Err("a request has at least one [[run]]".into());
        }
        Ok(())
    }

    /// A path relative to the request's folder.
    pub fn resolve(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        match (&self.base_dir, pb.is_absolute()) {
            (Some(b), false) => b.join(pb),
            _ => pb,
        }
    }
}

/// Load one test-kit scenario by pack id (`<pack>/<id>`, `pack:<pack>/<id>` or bare id) or by
/// path (relative to `base`), resolved.
pub fn load_scenario(name: &str, base: Option<&Path>) -> Result<Spec, String> {
    let n = name.strip_prefix("pack:").unwrap_or(name);
    let as_path = match base {
        Some(b) if !Path::new(n).is_absolute() => b.join(n),
        _ => PathBuf::from(n),
    };
    let spec = if as_path.is_file() {
        Spec::load(&as_path)?
    } else if let Some((_, src)) = crate::testkit::packs::find(n) {
        Spec::parse(src)?
    } else {
        return Err(format!(
            "scenario '{name}': not a file or a test-kit pack scenario"
        ));
    };
    spec.resolve()
}

// ---------------------------------------------------------------------------------------
// Threat classes and the open detector
// ---------------------------------------------------------------------------------------

/// The class of each threat of a scenario, with its window: `(class, from_s, to_s)`.
pub fn threat_classes(spec: &Spec) -> Vec<(String, f64, f64)> {
    use kshana_eloran::sync::degrade::JamKind;
    spec.threats
        .iter()
        .filter_map(|t| {
            let c = match t.kind? {
                ThreatType::Jamming => match t.waveform.unwrap_or_default() {
                    JamKind::Cw => "jamming-cw".to_string(),
                    JamKind::Wideband => "jamming-broadband".into(),
                    JamKind::Chirp => "jamming-chirp".into(),
                    JamKind::Pulsed => "jamming-pulsed".into(),
                },
                ThreatType::Spoofing => format!(
                    "spoofing-{}",
                    t.technique.map(|x| x.as_str()).unwrap_or("unknown")
                ),
                ThreatType::Cn0Offset => "degradation-cn0_offset".into(),
                ThreatType::PowerStep => "degradation-power_step".into(),
                ThreatType::Outage => "degradation-outage".into(),
                ThreatType::SatMask => "degradation-sat_mask".into(),
            };
            Some((c, t.start_s, t.end_s()))
        })
        .collect()
}

/// One epoch as the open detector sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrongInput {
    pub t_s: f64,
    /// The receiver calls its output valid.
    pub valid: bool,
    /// Every indicator the receiver carries reads normal.
    pub all_normal: bool,
    pub h_error_m: Option<f64>,
    pub time_error_ns: Option<f64>,
}

/// An episode of wrong output while every flag read normal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WrongEpisode {
    pub from_s: f64,
    pub to_s: f64,
    pub epochs: usize,
    pub max_h_error_m: Option<f64>,
    pub max_time_error_ns: Option<f64>,
}

/// The open "flag normal while wrong" detector: epochs whose output is valid, whose flags all
/// read normal, and whose position error exceeds `h_bar_m` or time error exceeds `t_bar_ns`,
/// grouped into episodes (a gap longer than `max_gap_s` between such epochs, or a correct
/// epoch, ends an episode).
pub fn flag_normal_while_wrong(
    eps: &[WrongInput],
    h_bar_m: f64,
    t_bar_ns: f64,
    max_gap_s: f64,
) -> Vec<WrongEpisode> {
    let mut out: Vec<WrongEpisode> = vec![];
    let mut open = false;
    for e in eps {
        let wrong = e.valid
            && e.all_normal
            && (e.h_error_m.is_some_and(|h| h > h_bar_m)
                || e.time_error_ns.is_some_and(|t| t.abs() > t_bar_ns));
        if !wrong {
            open = false;
            continue;
        }
        let hm = e.h_error_m.filter(|h| *h > h_bar_m);
        let tm = e.time_error_ns.map(f64::abs).filter(|t| *t > t_bar_ns);
        match out.last_mut() {
            Some(w) if open && e.t_s - w.to_s <= max_gap_s => {
                w.to_s = e.t_s;
                w.epochs += 1;
                w.max_h_error_m = max_opt(w.max_h_error_m, hm);
                w.max_time_error_ns = max_opt(w.max_time_error_ns, tm);
            }
            _ => out.push(WrongEpisode {
                from_s: e.t_s,
                to_s: e.t_s,
                epochs: 1,
                max_h_error_m: hm,
                max_time_error_ns: tm,
            }),
        }
        open = true;
    }
    out
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Wilson score interval for `k` successes of `n` at 95 %.
pub fn wilson95(k: usize, n: usize) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let (n, p) = (n as f64, k as f64 / n as f64);
    let z2 = Z95 * Z95;
    let d = 1.0 + z2 / n;
    let c = (p + z2 / (2.0 * n)) / d;
    let h = Z95 * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / d;
    Some(((c - h).max(0.0), (c + h).min(1.0)))
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

// ---------------------------------------------------------------------------------------
// The scorecard
// ---------------------------------------------------------------------------------------

/// How one indicator did in one run, per threat of the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunIndicator {
    pub indicator: String,
    pub class: String,
    /// Expected to respond (`true`), to stay quiet (`false`), either (`null`).
    pub expected: Option<bool>,
    /// Raised within the detection bar (expected) or anywhere in the window (otherwise);
    /// `null` when no epoch of the window carries the indicator.
    pub raised: Option<bool>,
    pub latency_s: Option<f64>,
}

/// One scored run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunResult {
    pub id: String,
    pub unit: Unit,
    pub scenario: String,
    pub scenario_sha256: String,
    pub log_sha256: String,
    pub preconditions: BTreeMap<String, String>,
    pub classes: Vec<String>,
    pub epochs: usize,
    pub indicators: Vec<RunIndicator>,
    /// Baseline epochs (clean run, or before the first onset) and raised epochs per indicator.
    pub baseline_epochs: usize,
    pub false_alarm_epochs: BTreeMap<String, usize>,
    pub normal_while_wrong: Vec<WrongEpisode>,
}

/// A rate with its interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rate {
    pub runs: usize,
    pub hits: usize,
    /// `hits / runs` when `runs >= min_runs`, else `null` (not assessed).
    pub rate: Option<f64>,
    pub ci95: Option<(f64, f64)>,
}

fn rate(hits: usize, runs: usize, min_runs: usize) -> Rate {
    let assessed = runs >= min_runs;
    Rate {
        runs,
        hits,
        rate: assessed.then(|| sig9(hits as f64 / runs as f64)),
        ci95: if assessed {
            wilson95(hits, runs).map(|(a, b)| (sig9(a), sig9(b)))
        } else {
            None
        },
    }
}

/// One indicator under one threat class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassRow {
    pub class: String,
    pub expected: Option<bool>,
    /// Expected classes: detections. Others: raised (wrong-class or neutral responses).
    pub response: Rate,
    pub median_latency_s: Option<f64>,
    /// Detection rate per precondition set (`key=value,...`).
    pub by_preconditions: BTreeMap<String, Rate>,
}

/// One indicator of one model and firmware.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndicatorRow {
    pub model: String,
    pub firmware: Option<String>,
    pub indicator: String,
    pub meaning: Meaning,
    pub documented_in: Option<String>,
    pub vendor_preconditions: Vec<String>,
    pub classes: Vec<ClassRow>,
    pub false_alarm_epochs: usize,
    pub baseline_epochs: usize,
    pub false_alarm_epoch_rate: Option<f64>,
    pub runs_with_false_alarm: usize,
    pub baseline_runs: usize,
}

/// A finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// `never-fired`, `wrong-class`, `precondition-dependent`, `undisclosed-precondition`,
    /// `false-alarms`, `normal-while-wrong`, `indicator-absent`.
    pub kind: String,
    pub model: String,
    pub firmware: Option<String>,
    pub indicator: Option<String>,
    pub text: String,
}

/// The scorecard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub wording: String,
    pub title: String,
    pub bars: Bars,
    pub bars_sha256: String,
    pub request_sha256: String,
    pub indicators_sha256: String,
    pub runs: Vec<RunResult>,
    pub table: Vec<IndicatorRow>,
    pub findings: Vec<Finding>,
    pub limits: Vec<String>,
}

/// SHA-256 of the bars (canonical JSON).
pub fn bars_sha256(b: &Bars) -> String {
    sha(&serde_json::to_vec(b).unwrap_or_default())
}

fn pre_key(p: &BTreeMap<String, String>) -> String {
    if p.is_empty() {
        return "(none stated)".into();
    }
    p.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// A loaded run: the log, its resolved scenario and the log bytes.
pub struct LoadedRun {
    pub id: String,
    pub log: FlagLog,
    pub spec: Spec,
    pub log_bytes: Vec<u8>,
}

/// Read every run of a request.
pub fn load_runs(r: &Request) -> Result<Vec<LoadedRun>, String> {
    let mut out = vec![];
    let mut ids = BTreeSet::new();
    for (i, run) in r.runs.iter().enumerate() {
        let path = r.resolve(&run.log);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut log = FlagLog::parse(&String::from_utf8_lossy(&bytes))
            .map_err(|e| format!("{}: {e}", run.log))?;
        for (k, v) in &run.preconditions {
            log.preconditions.insert(k.clone(), v.clone());
        }
        let sc = run.scenario.clone().unwrap_or_else(|| log.scenario.clone());
        let spec = load_scenario(&sc, r.base_dir.as_deref())?;
        log.scenario = sc;
        let id = run
            .id
            .clone()
            .unwrap_or_else(|| format!("run-{:03}", i + 1));
        if !ids.insert(id.clone()) {
            return Err(format!("run id '{id}' is repeated"));
        }
        out.push(LoadedRun {
            id,
            log,
            spec,
            log_bytes: bytes,
        });
    }
    Ok(out)
}

/// Score one run against the model's declared indicators.
pub fn score_run(run: &LoadedRun, inds: &[&Indicator], bars: &Bars) -> RunResult {
    let spec = &run.spec;
    let log = &run.log;
    let classes = threat_classes(spec);
    let mut indicators = vec![];
    for ind in inds {
        let list: Vec<(String, f64, f64)> = if classes.is_empty() {
            vec![(CLEAN.to_string(), 0.0, spec.scene.duration_s)]
        } else {
            classes.clone()
        };
        for (class, from, to) in list {
            let expected = ind.meaning.expects(&class);
            let until = if expected == Some(true) {
                (from + bars.detect_within_s).min(to)
            } else {
                to
            };
            let win: Vec<&FlagEpoch> = log
                .epochs
                .iter()
                .filter(|e| e.t_s >= from && e.t_s <= until)
                .collect();
            let carried: Vec<(f64, bool)> = win
                .iter()
                .filter_map(|e| ind.raised(e).map(|r| (e.t_s, r)))
                .collect();
            let first = carried.iter().find(|x| x.1).map(|x| x.0);
            indicators.push(RunIndicator {
                indicator: ind.name.clone(),
                class: class.clone(),
                expected,
                raised: if carried.is_empty() {
                    None
                } else {
                    Some(first.is_some())
                },
                latency_s: first.map(|t| sig9(t - from)),
            });
        }
    }
    // Baseline: the whole of a clean run, or before the first onset.
    let onset = classes.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
    let base: Vec<&FlagEpoch> = log
        .epochs
        .iter()
        .filter(|e| e.t_s >= bars.false_alarm_skip_s && e.t_s < onset)
        .collect();
    let mut fa = BTreeMap::new();
    for ind in inds {
        fa.insert(
            ind.name.clone(),
            base.iter().filter(|e| ind.raised(e) == Some(true)).count(),
        );
    }
    // The open detector.
    let h_bar = bars
        .misleading_h_error_m
        .unwrap_or(spec.expected.misleading_h_error_m);
    let t_bar = bars
        .misleading_time_error_ns
        .unwrap_or(spec.expected.misleading_time_error_ns);
    let w: Vec<WrongInput> = log
        .epochs
        .iter()
        .map(|e| {
            let tr = spec.trajectory.at(e.t_s);
            let valid = e.fix_valid != Some(false);
            WrongInput {
                t_s: e.t_s,
                valid,
                all_normal: inds.iter().all(|i| i.raised(e) != Some(true)),
                h_error_m: match (e.lat_deg, e.lon_deg) {
                    (Some(la), Some(lo)) if valid => {
                        Some(haversine_m(tr.lat_deg, tr.lon_deg, la, lo))
                    }
                    _ => None,
                },
                time_error_ns: e.time_error_ns,
            }
        })
        .collect();
    let gap = median(
        log.epochs
            .windows(2)
            .map(|p| p[1].t_s - p[0].t_s)
            .filter(|d| *d > 0.0)
            .collect(),
    )
    .unwrap_or(1.0)
        * 1.5;
    let wrong = flag_normal_while_wrong(&w, h_bar, t_bar, gap)
        .into_iter()
        .map(|x| WrongEpisode {
            from_s: sig9(x.from_s),
            to_s: sig9(x.to_s),
            epochs: x.epochs,
            max_h_error_m: sig9o(x.max_h_error_m),
            max_time_error_ns: sig9o(x.max_time_error_ns),
        })
        .collect();
    RunResult {
        id: run.id.clone(),
        unit: log.unit.clone(),
        scenario: spec.id.clone(),
        scenario_sha256: spec.sha256(),
        log_sha256: sha(&run.log_bytes),
        preconditions: log.preconditions.clone(),
        classes: classes.iter().map(|c| c.0.clone()).collect(),
        epochs: log.epochs.len(),
        indicators,
        baseline_epochs: base.len(),
        false_alarm_epochs: fa,
        normal_while_wrong: wrong,
    }
}

/// Undisclosed when no vendor precondition excludes the failing conditions.
fn disclosed(vendor: &[String], failing: &BTreeMap<String, String>) -> bool {
    vendor.iter().any(|p| {
        let (k, v) = p.split_once('=').unwrap_or((p, ""));
        failing
            .get(k.trim())
            .is_some_and(|fv| !fv.eq_ignore_ascii_case(v.trim()))
    })
}

/// Score a request.
pub fn score(r: &Request, src: &str) -> Result<Scorecard, String> {
    let runs = load_runs(r)?;
    score_loaded(r, src, &runs)
}

/// Score already loaded runs (the CLI and the tests).
pub fn score_loaded(r: &Request, src: &str, runs: &[LoadedRun]) -> Result<Scorecard, String> {
    r.validate()?;
    let bars = &r.bars;
    let mut results = vec![];
    let mut findings = vec![];
    for run in runs {
        let inds: Vec<&Indicator> = r
            .indicators
            .iter()
            .filter(|i| i.model == run.log.unit.model)
            .collect();
        if inds.is_empty() {
            return Err(format!(
                "run '{}': no indicator is declared for model '{}' (declare them before scoring)",
                run.id, run.log.unit.model
            ));
        }
        results.push(score_run(run, &inds, bars));
    }
    // Aggregate per (model, firmware, indicator).
    let mut keys: BTreeSet<(String, Option<String>)> = BTreeSet::new();
    for x in &results {
        keys.insert((x.unit.model.clone(), x.unit.firmware.clone()));
    }
    let mut table = vec![];
    for (model, fw) in &keys {
        let mine: Vec<&RunResult> = results
            .iter()
            .filter(|x| &x.unit.model == model && &x.unit.firmware == fw)
            .collect();
        for ind in r.indicators.iter().filter(|i| &i.model == model) {
            let mut classes: BTreeMap<String, Vec<(&RunResult, &RunIndicator)>> = BTreeMap::new();
            for x in &mine {
                for ri in x.indicators.iter().filter(|ri| ri.indicator == ind.name) {
                    classes.entry(ri.class.clone()).or_default().push((x, ri));
                }
            }
            let mut rows = vec![];
            for (class, v) in &classes {
                let judged: Vec<&(&RunResult, &RunIndicator)> =
                    v.iter().filter(|(_, ri)| ri.raised.is_some()).collect();
                let hits = judged
                    .iter()
                    .filter(|(_, ri)| ri.raised == Some(true))
                    .count();
                let expected = ind.meaning.expects(class);
                let mut by: BTreeMap<String, (usize, usize, BTreeMap<String, String>)> =
                    BTreeMap::new();
                for (x, ri) in &judged {
                    let e = by.entry(pre_key(&x.preconditions)).or_insert((
                        0,
                        0,
                        x.preconditions.clone(),
                    ));
                    e.1 += 1;
                    if ri.raised == Some(true) {
                        e.0 += 1;
                    }
                }
                let by_pre: BTreeMap<String, Rate> = by
                    .iter()
                    .map(|(k, (h, n, _))| (k.clone(), rate(*h, *n, bars.min_runs)))
                    .collect();
                if v.iter().all(|(_, ri)| ri.raised.is_none()) {
                    findings.push(Finding {
                        kind: "indicator-absent".into(),
                        model: model.clone(),
                        firmware: fw.clone(),
                        indicator: Some(ind.name.clone()),
                        text: format!(
                            "No log of {class} runs carries indicator '{}': not assessed.",
                            ind.name
                        ),
                    });
                }
                if expected == Some(true) && !judged.is_empty() {
                    if hits == 0 {
                        findings.push(Finding {
                            kind: "never-fired".into(),
                            model: model.clone(),
                            firmware: fw.clone(),
                            indicator: Some(ind.name.clone()),
                            text: format!(
                                "'{}' ({}) did not fire within {} s of onset in any of {} {class} run(s).",
                                ind.name,
                                ind.meaning.as_str(),
                                bars.detect_within_s,
                                judged.len()
                            ),
                        });
                    }
                    // Precondition dependence: a set that fired and one that did not.
                    let sets: Vec<(&String, f64, &BTreeMap<String, String>)> = by
                        .iter()
                        .map(|(k, (h, n, p))| (k, *h as f64 / *n as f64, p))
                        .collect();
                    if let (Some(best), Some(worst)) = (
                        sets.iter().max_by(|a, b| a.1.total_cmp(&b.1)),
                        sets.iter().min_by(|a, b| a.1.total_cmp(&b.1)),
                    ) {
                        if best.1 - worst.1 >= bars.precondition_gap {
                            findings.push(Finding {
                                kind: "precondition-dependent".into(),
                                model: model.clone(),
                                firmware: fw.clone(),
                                indicator: Some(ind.name.clone()),
                                text: format!(
                                    "'{}' under {class}: detection {:.2} with {} but {:.2} with {}.",
                                    ind.name, best.1, best.0, worst.1, worst.0
                                ),
                            });
                            if !disclosed(&ind.preconditions, worst.2) {
                                findings.push(Finding {
                                    kind: "undisclosed-precondition".into(),
                                    model: model.clone(),
                                    firmware: fw.clone(),
                                    indicator: Some(ind.name.clone()),
                                    text: format!(
                                        "The vendor's stated preconditions for '{}' ({}) do not exclude {}, where it detected {:.2} of {class} runs.",
                                        ind.name,
                                        if ind.preconditions.is_empty() {
                                            "none stated".to_string()
                                        } else {
                                            ind.preconditions.join(", ")
                                        },
                                        worst.0,
                                        worst.1
                                    ),
                                });
                            }
                        }
                    }
                }
                if expected == Some(false) && class != CLEAN && hits > 0 {
                    findings.push(Finding {
                        kind: "wrong-class".into(),
                        model: model.clone(),
                        firmware: fw.clone(),
                        indicator: Some(ind.name.clone()),
                        text: format!(
                            "'{}' ({}) was raised in {hits} of {} {class} run(s), a class it is not documented to indicate.",
                            ind.name,
                            ind.meaning.as_str(),
                            judged.len()
                        ),
                    });
                }
                rows.push(ClassRow {
                    class: class.clone(),
                    expected,
                    response: rate(hits, judged.len(), bars.min_runs),
                    median_latency_s: sig9o(median(
                        judged
                            .iter()
                            .filter_map(|(_, ri)| ri.latency_s.filter(|_| ri.raised == Some(true)))
                            .collect(),
                    )),
                    by_preconditions: by_pre,
                });
            }
            let fa_e: usize = mine
                .iter()
                .map(|x| x.false_alarm_epochs.get(&ind.name).copied().unwrap_or(0))
                .sum();
            let base_e: usize = mine.iter().map(|x| x.baseline_epochs).sum();
            let base_runs = mine.iter().filter(|x| x.baseline_epochs > 0).count();
            let fa_runs = mine
                .iter()
                .filter(|x| x.false_alarm_epochs.get(&ind.name).copied().unwrap_or(0) > 0)
                .count();
            if fa_runs > 0 {
                findings.push(Finding {
                    kind: "false-alarms".into(),
                    model: model.clone(),
                    firmware: fw.clone(),
                    indicator: Some(ind.name.clone()),
                    text: format!(
                        "'{}' was raised in {fa_e} of {base_e} baseline epoch(s), in {fa_runs} of {base_runs} run(s).",
                        ind.name
                    ),
                });
            }
            table.push(IndicatorRow {
                model: model.clone(),
                firmware: fw.clone(),
                indicator: ind.name.clone(),
                meaning: ind.meaning,
                documented_in: ind.documented_in.clone(),
                vendor_preconditions: ind.preconditions.clone(),
                classes: rows,
                false_alarm_epochs: fa_e,
                baseline_epochs: base_e,
                false_alarm_epoch_rate: (base_e > 0).then(|| sig9(fa_e as f64 / base_e as f64)),
                runs_with_false_alarm: fa_runs,
                baseline_runs: base_runs,
            });
        }
        let wrong: Vec<(&String, usize, Option<f64>)> = mine
            .iter()
            .filter(|x| !x.normal_while_wrong.is_empty())
            .map(|x| {
                (
                    &x.id,
                    x.normal_while_wrong.iter().map(|w| w.epochs).sum(),
                    x.normal_while_wrong
                        .iter()
                        .filter_map(|w| w.max_h_error_m)
                        .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b)))),
                )
            })
            .collect();
        for (id, n, h) in wrong {
            findings.push(Finding {
                kind: "normal-while-wrong".into(),
                model: model.clone(),
                firmware: fw.clone(),
                indicator: None,
                text: format!(
                    "Run {id}: {n} epoch(s) of valid output with every declared flag normal while the output was wrong beyond the bar{}.",
                    h.map(|h| format!(" (largest position error {h:.1} m)"))
                        .unwrap_or_default()
                ),
            });
        }
    }
    let mut limits = vec![
        "Rates are measured on the runs supplied, under the scenarios named; they say nothing about conditions not run.".to_string(),
        format!("A rate is stated only with at least {} judged runs; fewer is not assessed. Intervals are Wilson 95 %.", bars.min_runs),
        "Truth is the scenario's trajectory; a log recorded against a different set-up is scored as if it matched.".into(),
        "Indicators and their meanings are as the request declares them from the vendor's documentation; an indicator the log does not carry is not assessed.".into(),
        "MODELLED: the scorer is checked on synthetic logs; no real receiver campaign is scored in this release.".into(),
    ];
    if results
        .iter()
        .all(|x| x.classes.iter().all(|c| !c.starts_with("spoofing-")))
    {
        limits.push("No spoofing run: spoofing indicators not assessed.".into());
    }
    Ok(Scorecard {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        evidence: EVIDENCE.into(),
        wording: WORDING.into(),
        title: r.title.clone(),
        bars: bars.clone(),
        bars_sha256: bars_sha256(bars),
        request_sha256: sha(src.as_bytes()),
        indicators_sha256: sha(&serde_json::to_vec(&r.indicators).unwrap_or_default()),
        runs: results,
        table,
        findings,
        limits,
    })
}

/// The table as CSV (one row per model, firmware, indicator and class).
pub fn csv(s: &Scorecard) -> String {
    let mut out = String::from(
        "model,firmware,indicator,meaning,class,expected,runs,responses,rate,ci95_low,ci95_high,median_latency_s\n",
    );
    let f = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
    for r in &s.table {
        for c in &r.classes {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{}\n",
                r.model,
                r.firmware.clone().unwrap_or_default(),
                r.indicator,
                r.meaning.as_str(),
                c.class,
                match c.expected {
                    Some(true) => "respond",
                    Some(false) => "quiet",
                    None => "either",
                },
                c.response.runs,
                c.response.hits,
                f(c.response.rate),
                f(c.response.ci95.map(|x| x.0)),
                f(c.response.ci95.map(|x| x.1)),
                f(c.median_latency_s),
            ));
        }
    }
    out
}

/// Write `flag-efficacy.json`, `flag-efficacy.csv`, `request.toml`, the logs as scored and
/// `README.txt` (sign the directory afterwards).
pub fn write_dir(dir: &Path, s: &Scorecard, src: &str, runs: &[LoadedRun]) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("logs")).map_err(io)?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(s).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("flag-efficacy.csv"), csv(s)).map_err(io)?;
    std::fs::write(dir.join("request.toml"), src).map_err(io)?;
    for r in runs {
        std::fs::write(
            dir.join("logs").join(format!("{}.json", r.id)),
            &r.log_bytes,
        )
        .map_err(io)?;
    }
    let mut t = format!(
        "Flag-efficacy scorecard ({FORMAT}, method {METHOD_ID})\n{}\n\n{WORDING}\n\n",
        s.title
    );
    for f in &s.findings {
        t += &format!(
            "[{}] {} {}: {}\n",
            f.kind,
            f.model,
            f.firmware.clone().unwrap_or_default(),
            f.text
        );
    }
    t += &format!("\nEvidence: {EVIDENCE}\n");
    std::fs::write(dir.join("README.txt"), t).map_err(io)?;
    Ok(())
}

/// Synthetic flag logs for the tests and to try the command (MODELLED fixtures, not models of
/// any receiver).
pub mod synth {
    use super::*;

    /// How the synthetic unit behaves.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Behaviour {
        /// Flags every threat promptly, never outputs the counterfeit position, and stays quiet
        /// otherwise.
        Good,
        /// Its spoofing flag never fires; it follows the spoofer with every flag normal.
        SilentSpoof,
        /// Its spoofing flag fires only in multi-constellation runs.
        MultiOnly,
        /// Its CW indicator also fires under broadband jamming.
        CwConfused,
    }

    /// A flag log of `unit` under `spec` with the given preconditions.
    pub fn log(
        spec: &Spec,
        unit: &Unit,
        b: Behaviour,
        pre: &[(&str, &str)],
        react_s: f64,
    ) -> FlagLog {
        let pre: BTreeMap<String, String> = pre
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let multi = pre.get("constellations").map(String::as_str) != Some("single");
        let classes = threat_classes(spec);
        let (mpd_lat, mpd_lon) = crate::pics::log::metres_per_degree(spec.trajectory.lat_deg);
        let n = spec.scene.duration_s as usize;
        let epochs = (0..=n)
            .map(|i| {
                let t = i as f64;
                let tr = spec.trajectory.at(t);
                let (e, nn, tn) = spec.counterfeit_at(t);
                let active: Vec<&(String, f64, f64)> =
                    classes.iter().filter(|c| t >= c.1 && t <= c.2).collect();
                let since = active.iter().map(|c| t - c.1).fold(f64::NAN, f64::max);
                let reacted = since.is_finite() && since >= react_s;
                let has = |p: &str| active.iter().any(|c| c.0.starts_with(p));
                let spoof_flag = match b {
                    Behaviour::SilentSpoof => false,
                    Behaviour::MultiOnly => multi && reacted && has("spoofing-"),
                    _ => reacted && has("spoofing-"),
                };
                let jam = reacted && has("jamming-");
                let cw = match b {
                    Behaviour::CwConfused => jam,
                    _ => reacted && has("jamming-cw"),
                };
                let follows = has("spoofing-") && !spoof_flag && b != Behaviour::Good;
                let (lat, lon, te) = if follows {
                    (tr.lat_deg + nn / mpd_lat, tr.lon_deg + e / mpd_lon, tn)
                } else {
                    (tr.lat_deg, tr.lon_deg, 0.0)
                };
                let valid = !jam;
                let mut ind = BTreeMap::new();
                ind.insert(
                    "spoof".into(),
                    IndValue::Num(if spoof_flag { 2.0 } else { 0.0 }),
                );
                ind.insert("jam".into(), IndValue::Bool(jam));
                ind.insert("cw".into(), IndValue::Bool(cw));
                ind.insert(
                    "integrity".into(),
                    IndValue::Text(if spoof_flag { "unsafe" } else { "ok" }.into()),
                );
                FlagEpoch {
                    t_s: t,
                    fix_valid: Some(valid && !spoof_flag),
                    lat_deg: Some(lat),
                    lon_deg: Some(lon),
                    time_error_ns: Some(te),
                    ind,
                }
            })
            .collect();
        FlagLog {
            format: LOG_FORMAT.into(),
            unit: unit.clone(),
            scenario: spec.id.clone(),
            preconditions: pre,
            source: Some("synthetic".into()),
            epochs,
        }
    }
}
