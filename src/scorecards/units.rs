// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Low-cost receiver, board and handset scorecards with firmware regression (research
//! backlog B1302, E13)
//!
//! Low-cost boards, modules and handsets are bought and fielded in numbers, and their
//! behaviour changes with firmware. Users report that two units of one model behave
//! differently, and that a firmware update made a unit worse at something it used to do. This
//! module scores bought units under test-kit scenarios with a published method and compares
//! firmware versions of one model:
//!
//! * **Units** ([`UnitSpec`]) are declared before scoring: model, class (`board`, `module`,
//!   `handset`, `timing-module`), firmware, serial.
//! * **Runs** pair a unit with a test-kit scenario (`kshana-testkit-scenario/1`, a pack id or a
//!   file) and the unit's log (the PICS device log, or NMEA 0183). Each run is scored by the
//!   test kit's own scorer ([`crate::testkit::score::score`], method `kshana-pro/testkit/v1`):
//!   the same criteria, windows and not-assessed rules.
//! * **Model scorecard** per model and firmware: per scenario and criterion the measured values
//!   over the units (median, minimum, maximum, spread) and how many units met the criterion.
//!   Where units of one model and firmware disagree on a verdict, the scorecard reports the
//!   unit-to-unit variability.
//! * **Firmware regression** ([`Compare`]): for one model, firmware `from` against `to`, per
//!   scenario and criterion: the change of the median across units, judged against a tolerance
//!   stated before scoring, `max(absolute floor, relative × |before|, k × unit spread)`, so a
//!   difference within the spread between units is not called a regression. A criterion whose
//!   share of units meeting it fell is a verdict regression. Each regression is written as an
//!   alert line (`regression-alerts.md`) for subscribers of that model.
//!
//! Method [`METHOD_ID`] is stated in [`METHOD`] and published with every scorecard. Results are
//! measured on the units and runs supplied, never a statement that a model meets a standard.
//! Evidence: **MODELLED** until real units are scored; checked on synthetic logs
//! (`tests/receiver_flag_scorecards.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::flags::{load_scenario, sig9};
use crate::pics::log::DeviceLog;
use crate::scorecards::levels::Verdict;
use crate::testkit::score::{self as tk, ScenarioVerdict};
use crate::testkit::spec::Spec;

pub const REQUEST_FORMAT: &str = "kshana-pro-unit-scorecard-request/1";
pub const FORMAT: &str = "kshana-pro-unit-scorecard/1";
pub const METHOD_ID: &str = "kshana-pro/unit-scorecard/v1";
pub const RESULT_FILE: &str = "unit-scorecard.json";
pub const METHOD: &str = "Each run is scored by the test kit's scorer (kshana-pro/testkit/v1) \
against its scenario's criteria; a metric whose data the log lacks is not assessed. Per model and \
firmware, per scenario and criterion: median, minimum, maximum and spread (max - min) of the \
measured values over units, and the number of units meeting the criterion. Firmware regression: \
the median after minus the median before, signed so that positive is worse (the criterion's \
direction), is a regression when it exceeds max(absolute floor of the metric, relative tolerance \
x |median before|, k x the larger unit spread); a fall in the share of units meeting a criterion \
is a verdict regression. Tolerances are stated before scoring and hashed into the scorecard.";
pub const EVIDENCE: &str = "MODELLED: the method is checked on synthetic logs; no bought unit has \
been scored in this release.";
pub const WORDING: &str = "Measured results of the units and runs supplied, under criteria and \
tolerances stated before scoring. Not a statement that any model meets a standard, and not a \
certification.";

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// A unit under test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitSpec {
    pub id: String,
    pub model: String,
    /// `board`, `module`, `handset`, `timing-module`, `receiver`.
    #[serde(default)]
    pub class: Option<String>,
    pub firmware: String,
    #[serde(default)]
    pub serial: Option<String>,
    /// Where and when it was bought (independent purchase, as the tester states it).
    #[serde(default)]
    pub provenance: Option<String>,
}

/// One run: a unit's log under a scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSpec {
    pub unit: String,
    pub scenario: String,
    /// A PICS device log (`.json`) or an NMEA 0183 log (`.nmea`).
    pub log: String,
    /// Scenario time of an NMEA log's first epoch, s.
    #[serde(default)]
    pub t0_s: f64,
}

/// A firmware comparison of one model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compare {
    pub model: String,
    pub from: String,
    pub to: String,
}

/// Tolerances, stated before scoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bars {
    pub fixed_utc: Option<String>,
    /// Relative tolerance on the median before.
    pub relative: f64,
    /// Multiple of the unit-to-unit spread.
    pub spread_k: f64,
    /// Absolute floor per metric (defaults in [`default_floors`]; entries here override).
    pub absolute: BTreeMap<String, f64>,
}

impl Default for Bars {
    fn default() -> Self {
        Bars {
            fixed_utc: None,
            relative: 0.2,
            spread_k: 1.0,
            absolute: BTreeMap::new(),
        }
    }
}

/// The absolute floor of each test-kit metric.
pub fn default_floors() -> BTreeMap<&'static str, f64> {
    BTreeMap::from([
        ("detection_latency_s", 1.0),
        ("recovery_s", 2.0),
        ("misleading_epochs", 0.5),
        ("false_alarms", 0.5),
        ("availability", 0.01),
        ("max_h_error_m", 1.0),
        ("p95_h_error_m", 1.0),
        ("max_unflagged_h_error_m", 1.0),
        ("max_time_error_ns", 10.0),
        ("max_unflagged_time_error_ns", 10.0),
    ])
}

impl Bars {
    pub fn floor(&self, metric: &str) -> f64 {
        self.absolute
            .get(metric)
            .copied()
            .or_else(|| default_floors().get(metric).copied())
            .unwrap_or(0.0)
    }
    fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        if !ok(self.relative) || !ok(self.spread_k) {
            return Err("bars: relative and spread_k must be >= 0".into());
        }
        if self
            .absolute
            .values()
            .any(|v| !(v.is_finite() && *v >= 0.0))
        {
            return Err("bars: absolute floors must be >= 0".into());
        }
        Ok(())
    }
}

/// A unit-scorecard request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub format: String,
    pub title: String,
    #[serde(default)]
    pub bars: Bars,
    #[serde(rename = "unit")]
    pub units: Vec<UnitSpec>,
    #[serde(rename = "run")]
    pub runs: Vec<RunSpec>,
    #[serde(default, rename = "compare")]
    pub compares: Vec<Compare>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl Request {
    pub fn parse(src: &str) -> Result<Request, String> {
        let r: Request =
            toml::from_str(src).map_err(|e| format!("invalid unit-scorecard request: {e}"))?;
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
        let mut ids = BTreeSet::new();
        for u in &self.units {
            if u.id.trim().is_empty() || !ids.insert(u.id.as_str()) {
                return Err(format!("unit id '{}' is empty or repeated", u.id));
            }
            if u.model.trim().is_empty() || u.firmware.trim().is_empty() {
                return Err(format!("unit '{}': model and firmware are needed", u.id));
            }
        }
        if self.runs.is_empty() {
            return Err("a request has at least one [[run]]".into());
        }
        let mut pairs = BTreeSet::new();
        for r in &self.runs {
            if !ids.contains(r.unit.as_str()) {
                return Err(format!("run of unknown unit '{}'", r.unit));
            }
            if !pairs.insert((r.unit.as_str(), r.scenario.as_str())) {
                return Err(format!(
                    "unit '{}' has two runs of scenario '{}'",
                    r.unit, r.scenario
                ));
            }
        }
        for c in &self.compares {
            for fw in [&c.from, &c.to] {
                if !self
                    .units
                    .iter()
                    .any(|u| u.model == c.model && &u.firmware == fw)
                {
                    return Err(format!(
                        "compare: no unit of model '{}' runs firmware '{fw}'",
                        c.model
                    ));
                }
            }
            if c.from == c.to {
                return Err("compare: from and to are the same firmware".into());
            }
        }
        Ok(())
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        match (&self.base_dir, pb.is_absolute()) {
            (Some(b), false) => b.join(pb),
            _ => pb,
        }
    }
}

/// A loaded run.
pub struct LoadedRun {
    pub unit: String,
    pub spec: Spec,
    pub log: DeviceLog,
}

/// Read every run of a request.
pub fn load_runs(r: &Request) -> Result<Vec<LoadedRun>, String> {
    let mut out = vec![];
    for run in &r.runs {
        let spec = load_scenario(&run.scenario, r.base_dir.as_deref())?;
        let p = r.resolve(&run.log);
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let log = if p.extension().is_some_and(|x| x == "nmea") {
            crate::pics::log::from_nmea(&text, &spec.id, run.t0_s)?
        } else {
            DeviceLog::parse(&text).map_err(|e| format!("{}: {e}", run.log))?
        };
        out.push(LoadedRun {
            unit: run.unit.clone(),
            spec,
            log,
        });
    }
    Ok(out)
}

/// One criterion of one scenario over the units of a model and firmware.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionStats {
    pub scenario: String,
    pub criterion: String,
    pub metric: String,
    pub op: String,
    pub value: f64,
    pub units: usize,
    pub units_met: usize,
    pub units_not_met: usize,
    pub units_not_assessed: usize,
    pub median: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub spread: Option<f64>,
}

/// One model and firmware.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCard {
    pub model: String,
    pub firmware: String,
    pub classes: Vec<String>,
    pub units: Vec<String>,
    pub scenarios: usize,
    pub runs_pass: usize,
    pub runs_fail: usize,
    pub runs_incomplete: usize,
    /// The lowest per-run supported resilience level over the units' runs that grade levels.
    pub lowest_level_supported: Option<u8>,
    pub criteria: Vec<CriterionStats>,
}

/// One criterion's change between firmware versions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionChange {
    pub scenario: String,
    pub criterion: String,
    pub metric: String,
    pub before_median: Option<f64>,
    pub after_median: Option<f64>,
    /// After minus before, signed so that positive is worse.
    pub worse_by: Option<f64>,
    pub tolerance: Option<f64>,
    pub before_met: String,
    pub after_met: String,
    /// `regression`, `improvement`, `within-tolerance`, `verdict-regression`, `not-compared`.
    pub change: String,
}

/// A firmware comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Regression {
    pub model: String,
    pub from: String,
    pub to: String,
    pub regressions: usize,
    pub improvements: usize,
    pub changes: Vec<CriterionChange>,
    pub alerts: Vec<String>,
}

/// A variability note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variability {
    pub model: String,
    pub firmware: String,
    pub scenario: String,
    pub criterion: String,
    pub text: String,
}

/// The unit scorecard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub format: String,
    pub method: String,
    pub method_text: String,
    pub evidence: String,
    pub wording: String,
    pub title: String,
    pub bars: Bars,
    pub bars_sha256: String,
    pub request_sha256: String,
    pub units: Vec<UnitSpec>,
    pub verdicts: Vec<ScenarioVerdict>,
    pub models: Vec<ModelCard>,
    pub variability: Vec<Variability>,
    pub regressions: Vec<Regression>,
    pub limits: Vec<String>,
}

fn stats(v: &mut [f64]) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    if v.is_empty() {
        return (None, None, None, None);
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let med = if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    };
    (
        Some(sig9(med)),
        Some(sig9(v[0])),
        Some(sig9(v[n - 1])),
        Some(sig9(v[n - 1] - v[0])),
    )
}

/// Score a request.
pub fn score(r: &Request, src: &str) -> Result<Scorecard, String> {
    let runs = load_runs(r)?;
    score_loaded(r, src, &runs)
}

/// Score already loaded runs.
pub fn score_loaded(r: &Request, src: &str, runs: &[LoadedRun]) -> Result<Scorecard, String> {
    r.validate()?;
    let unit = |id: &str| r.units.iter().find(|u| u.id == id);
    let mut verdicts: Vec<(String, ScenarioVerdict)> = vec![];
    for run in runs {
        let u = unit(&run.unit).ok_or_else(|| format!("unknown unit '{}'", run.unit))?;
        let v = tk::score(&run.spec, &run.log, &u.firmware, &u.id)?;
        verdicts.push((u.id.clone(), v));
    }
    // Model cards.
    let groups: BTreeSet<(String, String)> = r
        .units
        .iter()
        .map(|u| (u.model.clone(), u.firmware.clone()))
        .collect();
    let mut models = vec![];
    let mut variability = vec![];
    for (model, fw) in &groups {
        let ids: Vec<&str> = r
            .units
            .iter()
            .filter(|u| &u.model == model && &u.firmware == fw)
            .map(|u| u.id.as_str())
            .collect();
        let mine: Vec<&ScenarioVerdict> = verdicts
            .iter()
            .filter(|(id, _)| ids.contains(&id.as_str()))
            .map(|(_, v)| v)
            .collect();
        let mut keys: BTreeMap<(String, String), Vec<&tk::CriterionResult>> = BTreeMap::new();
        for v in &mine {
            for c in &v.criteria {
                keys.entry((v.scenario.clone(), c.id.clone()))
                    .or_default()
                    .push(c);
            }
        }
        let mut criteria = vec![];
        for ((sc, cid), cs) in &keys {
            let mut vals: Vec<f64> = cs
                .iter()
                .filter(|c| c.verdict != Verdict::NotAssessed)
                .filter_map(|c| c.measured)
                .collect();
            let (median, min, max, spread) = stats(&mut vals);
            let n = |v: Verdict| cs.iter().filter(|c| c.verdict == v).count();
            let st = CriterionStats {
                scenario: sc.clone(),
                criterion: cid.clone(),
                metric: cs[0].metric.clone(),
                op: cs[0].op.clone(),
                value: cs[0].value,
                units: cs.len(),
                units_met: n(Verdict::Met),
                units_not_met: n(Verdict::NotMet),
                units_not_assessed: n(Verdict::NotAssessed),
                median,
                min,
                max,
                spread,
            };
            if st.units_met > 0 && st.units_not_met > 0 {
                variability.push(Variability {
                    model: model.clone(),
                    firmware: fw.clone(),
                    scenario: sc.clone(),
                    criterion: cid.clone(),
                    text: format!(
                        "{} of {} units met '{cid}' on {sc} and {} did not (measured {} to {}).",
                        st.units_met,
                        st.units,
                        st.units_not_met,
                        fmt(st.min),
                        fmt(st.max)
                    ),
                });
            }
            criteria.push(st);
        }
        let classes: BTreeSet<String> = r
            .units
            .iter()
            .filter(|u| ids.contains(&u.id.as_str()))
            .filter_map(|u| u.class.clone())
            .collect();
        let count = |s: &str| mine.iter().filter(|v| v.verdict == s).count();
        let scen: BTreeSet<&str> = mine.iter().map(|v| v.scenario.as_str()).collect();
        models.push(ModelCard {
            model: model.clone(),
            firmware: fw.clone(),
            classes: classes.into_iter().collect(),
            units: ids.iter().map(|s| s.to_string()).collect(),
            scenarios: scen.len(),
            runs_pass: count("pass"),
            runs_fail: count("fail"),
            runs_incomplete: count("incomplete"),
            lowest_level_supported: mine.iter().filter_map(|v| v.level_supported).min(),
            criteria,
        });
    }
    // Firmware regressions.
    let mut regressions = vec![];
    for c in &r.compares {
        let find = |fw: &str| {
            models
                .iter()
                .find(|m| m.model == c.model && m.firmware == fw)
        };
        let (Some(a), Some(b)) = (find(&c.from), find(&c.to)) else {
            continue;
        };
        let mut changes = vec![];
        let mut alerts = vec![];
        let keys: BTreeSet<(String, String)> = a
            .criteria
            .iter()
            .chain(&b.criteria)
            .map(|x| (x.scenario.clone(), x.criterion.clone()))
            .collect();
        for (sc, cid) in keys {
            let ca = a
                .criteria
                .iter()
                .find(|x| x.scenario == sc && x.criterion == cid);
            let cb = b
                .criteria
                .iter()
                .find(|x| x.scenario == sc && x.criterion == cid);
            let (Some(ca), Some(cb)) = (ca, cb) else {
                changes.push(CriterionChange {
                    scenario: sc.clone(),
                    criterion: cid.clone(),
                    metric: ca.or(cb).map(|x| x.metric.clone()).unwrap_or_default(),
                    before_median: ca.and_then(|x| x.median),
                    after_median: cb.and_then(|x| x.median),
                    worse_by: None,
                    tolerance: None,
                    before_met: ca.map(met_share).unwrap_or_else(|| "-".into()),
                    after_met: cb.map(met_share).unwrap_or_else(|| "-".into()),
                    change: "not-compared".into(),
                });
                continue;
            };
            let sign = if ca.op == ">=" { -1.0 } else { 1.0 };
            let worse = match (ca.median, cb.median) {
                (Some(x), Some(y)) => Some(sig9(sign * (y - x))),
                _ => None,
            };
            let tol = ca.median.map(|x| {
                let spread = ca.spread.unwrap_or(0.0).max(cb.spread.unwrap_or(0.0));
                sig9(
                    r.bars
                        .floor(&ca.metric)
                        .max(r.bars.relative * x.abs())
                        .max(r.bars.spread_k * spread),
                )
            });
            let share = |x: &CriterionStats| {
                let judged = x.units_met + x.units_not_met;
                (judged > 0).then(|| x.units_met as f64 / judged as f64)
            };
            let verdict_fell = matches!((share(ca), share(cb)), (Some(p), Some(q)) if q < p);
            let change = match (worse, tol) {
                (Some(w), Some(t)) if w > t => "regression",
                _ if verdict_fell => "verdict-regression",
                (Some(w), Some(t)) if -w > t => "improvement",
                (Some(_), Some(_)) => "within-tolerance",
                _ => "not-compared",
            };
            if change == "regression" || change == "verdict-regression" {
                alerts.push(format!(
                    "{} firmware {} vs {}: '{cid}' ({}) on {sc}: median {} -> {}{}; units meeting it {} -> {}.",
                    c.model,
                    c.to,
                    c.from,
                    ca.metric,
                    fmt(ca.median),
                    fmt(cb.median),
                    match (worse, tol) {
                        (Some(w), Some(t)) => format!(" (worse by {} against a tolerance of {})", fmt(Some(w)), fmt(Some(t))),
                        _ => String::new(),
                    },
                    met_share(ca),
                    met_share(cb),
                ));
            }
            changes.push(CriterionChange {
                scenario: sc,
                criterion: cid,
                metric: ca.metric.clone(),
                before_median: ca.median,
                after_median: cb.median,
                worse_by: worse,
                tolerance: tol,
                before_met: met_share(ca),
                after_met: met_share(cb),
                change: change.into(),
            });
        }
        regressions.push(Regression {
            model: c.model.clone(),
            from: c.from.clone(),
            to: c.to.clone(),
            regressions: changes
                .iter()
                .filter(|x| x.change == "regression" || x.change == "verdict-regression")
                .count(),
            improvements: changes.iter().filter(|x| x.change == "improvement").count(),
            changes,
            alerts,
        });
    }
    let mut limits = vec![
        "Results are measured on the units and runs supplied; a model's other units, firmware and conditions are not covered.".to_string(),
        "A spread over fewer than three units says little about unit-to-unit variability.".into(),
        "Truth is each scenario's trajectory; scoring and not-assessed rules are the test kit's (kshana-pro/testkit/v1).".into(),
        "MODELLED: the method is checked on synthetic logs; no bought unit is scored in this release.".into(),
    ];
    if r.compares.is_empty() {
        limits.push("No firmware comparison requested.".into());
    }
    Ok(Scorecard {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        method_text: METHOD.into(),
        evidence: EVIDENCE.into(),
        wording: WORDING.into(),
        title: r.title.clone(),
        bars: r.bars.clone(),
        bars_sha256: sha(&serde_json::to_vec(&r.bars).unwrap_or_default()),
        request_sha256: sha(src.as_bytes()),
        units: r.units.clone(),
        verdicts: verdicts.into_iter().map(|(_, v)| v).collect(),
        models,
        variability,
        regressions,
        limits,
    })
}

fn met_share(x: &CriterionStats) -> String {
    format!("{}/{}", x.units_met, x.units_met + x.units_not_met)
}

fn fmt(x: Option<f64>) -> String {
    match x {
        Some(v) => format!("{v}"),
        None => "n/a".into(),
    }
}

/// The model table as CSV.
pub fn csv(s: &Scorecard) -> String {
    let mut out = String::from(
        "model,firmware,scenario,criterion,metric,op,value,units,met,not_met,not_assessed,median,min,max,spread\n",
    );
    let f = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
    for m in &s.models {
        for c in &m.criteria {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                m.model,
                m.firmware,
                c.scenario,
                c.criterion,
                c.metric,
                c.op,
                c.value,
                c.units,
                c.units_met,
                c.units_not_met,
                c.units_not_assessed,
                f(c.median),
                f(c.min),
                f(c.max),
                f(c.spread)
            ));
        }
    }
    out
}

/// The regression alerts as Markdown.
pub fn alerts_markdown(s: &Scorecard) -> String {
    let mut t = format!(
        "# Firmware regression alerts: {}\n\nMODELLED. {WORDING}\n\n",
        s.title
    );
    if s.regressions.is_empty() {
        t.push_str("No firmware comparison in this scorecard.\n");
    }
    for r in &s.regressions {
        t.push_str(&format!(
            "## {} {} -> {}: {} regression(s), {} improvement(s)\n\n",
            r.model, r.from, r.to, r.regressions, r.improvements
        ));
        for a in &r.alerts {
            t.push_str(&format!("- {a}\n"));
        }
        t.push('\n');
    }
    t
}

/// Write `unit-scorecard.json`, `units.csv`, `regression-alerts.md`, `request.toml` and
/// `README.txt` (sign the directory afterwards).
pub fn write_dir(dir: &Path, s: &Scorecard, src: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(s).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("units.csv"), csv(s)).map_err(io)?;
    std::fs::write(dir.join("regression-alerts.md"), alerts_markdown(s)).map_err(io)?;
    std::fs::write(dir.join("request.toml"), src).map_err(io)?;
    let mut t = format!(
        "Unit scorecard ({FORMAT}, method {METHOD_ID})\n{}\n\nMethod: {METHOD}\n\n{WORDING}\n\n",
        s.title
    );
    for m in &s.models {
        t += &format!(
            "{} {} ({} unit(s)): {} pass, {} fail, {} incomplete\n",
            m.model,
            m.firmware,
            m.units.len(),
            m.runs_pass,
            m.runs_fail,
            m.runs_incomplete
        );
    }
    for v in &s.variability {
        t += &format!("Variability: {} {}: {}\n", v.model, v.firmware, v.text);
    }
    t += &format!("\nEvidence: {EVIDENCE}\n");
    std::fs::write(dir.join("README.txt"), t).map_err(io)?;
    Ok(())
}
