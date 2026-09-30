// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Uncertainty and sensitivity at scale, over any scenario kind
//!
//! The open engine answers "what does this scenario give" (one run) and "how much does
//! it scatter over random seeds" (`campaign [monte_carlo]`). This module answers the
//! two programme questions that come next: **how sure are we**, when the inputs
//! themselves are uncertain, and **what drives it**.
//!
//! A study names a complete open scenario of any kind, the inputs that are uncertain
//! (each with a distribution and the source of that uncertainty), and the outputs to
//! watch. It runs the open engine over a seeded sample of the inputs and reports, per
//! output, the mean, the spread, the 5th, 50th and 95th percentiles, a bootstrap
//! interval on the mean, the running estimate as runs accumulate, the probability of
//! meeting a limit, the percentile band of a time series, and a ranking of the inputs
//! by how much of the output variance each one explains.
//!
//! The engine is used only through [`kshana::api::run_scenario`]; nothing here
//! re-implements or alters a model. The numbers are therefore exactly as trustworthy
//! as the base scenario kind, and the result carries the base kind's label and tiers
//! and never raises them.
//!
//! Layout: [`dist`] (distributions), [`sampling`] (designs), [`exec`] (parallel
//! evaluation in realisation order), [`stats`], [`sobol`], [`src_rank`], [`bands`]
//! (the estimators), [`checkpoint`] (stop and resume), [`render`] (charts and the
//! report page), [`cli`] (the `kshana-pro uncertainty` command). The user guide is
//! `docs/UNCERTAINTY.md`.

pub mod bands;
pub mod checkpoint;
pub mod cli;
pub mod dist;
pub mod exec;
pub mod paths;
pub mod render;
pub mod sampling;
pub mod sobol;
pub mod src_rank;
pub mod stats;

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use dist::Dist;
use exec::{Evaluation, Evaluator, Job, RunRecord};
use sampling::{Block, Matrix, Method};
use stats::LimitOp;

/// The scenario kind of a study file.
pub const KIND: &str = "pro-uncertainty";
/// Version of the result document's layout.
pub const SCHEMA_VERSION: &str = "1.0";
/// The licence feature that gates the capability.
pub const FEATURE: &str = "pro-uncertainty";
/// The most engine runs one study may make: every finished realisation is held in
/// memory until the result is written.
pub const MAX_RUNS: usize = 10_000_000;

// ─── The study file ────────────────────────────────────────────────────────────────

/// A study file, as written (`kind = "pro-uncertainty"`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StudySpec {
    /// `"pro-uncertainty"`; read by the dispatcher.
    #[serde(default)]
    pub kind: Option<String>,
    pub title: String,
    /// Seed of the sampling design and of every bootstrap.
    pub seed: u64,
    /// A complete open scenario, of any kind, as TOML text.
    pub base_scenario: String,
    #[serde(default, rename = "input")]
    pub inputs: Vec<InputSpec>,
    #[serde(default, rename = "output")]
    pub outputs: Vec<OutputSpec>,
    pub sampling: SamplingSpec,
    #[serde(default)]
    pub sensitivity: SensitivitySpec,
    #[serde(default)]
    pub run: RunSpec,
}

/// One uncertain input.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputSpec {
    pub name: String,
    /// Dotted key into the base scenario.
    pub key: String,
    /// `normal`, `uniform`, `lognormal`, `triangular` or `discrete`.
    pub dist: String,
    pub mean: Option<f64>,
    pub std: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mode: Option<f64>,
    pub median: Option<f64>,
    pub ln_std: Option<f64>,
    pub values: Option<Vec<f64>>,
    pub weights: Option<Vec<f64>>,
    /// Where the uncertainty comes from. Required.
    pub source: Option<String>,
    /// The input's unit, for display (the engine does not report units of inputs).
    pub unit: Option<String>,
}

/// A limit on an output.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LimitSpec {
    pub op: String,
    pub value: f64,
}

/// One watched output: a scalar (`path`) or a series (`series` and `time`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSpec {
    pub name: String,
    pub path: Option<String>,
    pub series: Option<String>,
    pub time: Option<String>,
    pub limit: Option<LimitSpec>,
    /// The unit, only for a path the base kind's units block does not describe.
    pub unit: Option<String>,
}

fn default_method() -> String {
    "lhs".to_string()
}
fn default_seed_key() -> String {
    "seed".to_string()
}
fn default_true() -> bool {
    true
}
fn default_none() -> String {
    "none".to_string()
}

/// How the inputs are sampled.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingSpec {
    #[serde(default = "default_method")]
    pub method: String,
    pub n: usize,
    /// Realisation `k` runs with the engine seed `base + k` (the open campaign's rule).
    #[serde(default)]
    pub vary_seed: bool,
    #[serde(default = "default_seed_key")]
    pub seed_key: String,
    /// Keep every realisation's value of every scalar output in the result.
    #[serde(default = "default_true")]
    pub keep_samples: bool,
}

/// Which sensitivity method runs.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SensitivitySpec {
    #[serde(default = "default_none")]
    pub method: String,
}

impl Default for SensitivitySpec {
    fn default() -> Self {
        SensitivitySpec {
            method: default_none(),
        }
    }
}

/// Execution settings. They cannot change a result, so they are not part of the
/// study's identity (its hash).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunSpec {
    pub threads: Option<usize>,
}

// ─── The validated plan ────────────────────────────────────────────────────────────

/// One uncertain input, validated.
#[derive(Debug, Clone)]
pub struct Input {
    pub name: String,
    pub key: String,
    pub dist: Dist,
    /// Where the uncertainty comes from, as the study states it.
    pub source: String,
    /// The source says the distribution is an assumption: printed as ASSUMED.
    pub assumed: bool,
    pub unit: Option<String>,
}

/// One scalar output.
#[derive(Debug, Clone)]
pub struct ScalarOutput {
    pub name: String,
    pub path: String,
    pub limit: Option<(LimitOp, f64)>,
    pub unit: Option<String>,
}

/// One series output.
#[derive(Debug, Clone)]
pub struct SeriesOutput {
    pub name: String,
    pub series: String,
    pub time: String,
    pub limit: Option<(LimitOp, f64)>,
    pub unit: Option<String>,
}

/// The sensitivity method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    None,
    /// Standardised regression coefficients, from the plain sample.
    Src,
    /// Sobol' indices, from a Saltelli design of `n * (k + 2)` runs.
    Sobol,
}

impl Sensitivity {
    /// The study file's spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Sensitivity::None => "none",
            Sensitivity::Src => "src",
            Sensitivity::Sobol => "sobol",
        }
    }
}

/// A validated study: everything except the base scenario itself.
#[derive(Debug, Clone)]
pub struct Plan {
    pub title: String,
    pub seed: u64,
    pub inputs: Vec<Input>,
    pub scalars: Vec<ScalarOutput>,
    pub series: Vec<SeriesOutput>,
    /// Output names in the order the study declares them.
    pub output_order: Vec<String>,
    pub method: Method,
    pub n: usize,
    pub vary_seed: bool,
    pub seed_key: String,
    pub keep_samples: bool,
    pub sensitivity: Sensitivity,
}

fn check_name(what: &str, name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!(
            "{what} name `{name}` must be non-empty letters, digits, `_` or `-`"
        ));
    }
    Ok(())
}

/// Whether a source text declares the distribution an assumption: it contains the
/// word "assumed" (any case).
pub fn is_assumed(source: &str) -> bool {
    source
        .split(|c: char| !c.is_ascii_alphabetic())
        .any(|w| w.eq_ignore_ascii_case("assumed"))
}

fn build_dist(i: &InputSpec) -> Result<Dist, String> {
    let need =
        |what: &str, v: Option<f64>| v.ok_or_else(|| format!("dist \"{}\" needs `{what}`", i.dist));
    // A parameter that the named distribution does not use is a mistake in the file
    // (a copied line, a misremembered name), so it is refused rather than ignored.
    let given: Vec<&str> = [
        ("mean", i.mean.is_some()),
        ("std", i.std.is_some()),
        ("min", i.min.is_some()),
        ("max", i.max.is_some()),
        ("mode", i.mode.is_some()),
        ("median", i.median.is_some()),
        ("ln_std", i.ln_std.is_some()),
        ("values", i.values.is_some()),
        ("weights", i.weights.is_some()),
    ]
    .iter()
    .filter(|(_, present)| *present)
    .map(|(name, _)| *name)
    .collect();
    let allow = |allowed: &[&str]| -> Result<(), String> {
        match given.iter().find(|g| !allowed.contains(g)) {
            Some(extra) => Err(format!(
                "dist \"{}\" does not take `{extra}` (it takes {})",
                i.dist,
                allowed.join(", ")
            )),
            None => Ok(()),
        }
    };
    let d = match i.dist.as_str() {
        "normal" => {
            allow(&["mean", "std"])?;
            Dist::Normal {
                mean: need("mean", i.mean)?,
                std: need("std", i.std)?,
            }
        }
        "uniform" => {
            allow(&["min", "max"])?;
            Dist::Uniform {
                min: need("min", i.min)?,
                max: need("max", i.max)?,
            }
        }
        "lognormal" => {
            allow(&["median", "ln_std"])?;
            Dist::Lognormal {
                median: need("median", i.median)?,
                ln_std: need("ln_std", i.ln_std)?,
            }
        }
        "triangular" => {
            allow(&["min", "mode", "max"])?;
            Dist::Triangular {
                min: need("min", i.min)?,
                mode: need("mode", i.mode)?,
                max: need("max", i.max)?,
            }
        }
        "discrete" => {
            allow(&["values", "weights"])?;
            let values = i.values.clone().ok_or("dist \"discrete\" needs `values`")?;
            let weights = i.weights.clone().unwrap_or_else(|| vec![1.0; values.len()]);
            Dist::Discrete { values, weights }
        }
        other => {
            return Err(format!(
                "unknown dist \"{other}\" (normal, uniform, lognormal, triangular, discrete)"
            ))
        }
    };
    d.validate()?;
    Ok(d)
}

fn build_limit(l: &Option<LimitSpec>) -> Result<Option<(LimitOp, f64)>, String> {
    match l {
        None => Ok(None),
        Some(l) => {
            if !l.value.is_finite() {
                return Err(format!("limit `value` must be finite (got {})", l.value));
            }
            Ok(Some((LimitOp::parse(&l.op)?, l.value)))
        }
    }
}

impl Plan {
    /// Validate a study file's content (everything that does not need the base
    /// scenario or a run).
    pub fn from_spec(spec: &StudySpec) -> Result<Plan, String> {
        if let Some(k) = &spec.kind {
            if k != KIND {
                return Err(format!("`kind` is \"{k}\", expected \"{KIND}\""));
            }
        }
        if spec.title.trim().is_empty() {
            return Err("`title` must not be empty".into());
        }
        if i64::try_from(spec.seed).is_err() {
            return Err(format!("`seed` {} does not fit a TOML integer", spec.seed));
        }

        let mut inputs = Vec::with_capacity(spec.inputs.len());
        let mut keys: Vec<&str> = Vec::new();
        let mut names: Vec<&str> = Vec::new();
        for i in &spec.inputs {
            check_name("input", &i.name)?;
            let at = |e: String| format!("input `{}`: {e}", i.name);
            if names.contains(&i.name.as_str()) {
                return Err(format!("input `{}` is declared twice", i.name));
            }
            if keys.contains(&i.key.as_str()) {
                return Err(at(format!(
                    "key `{}` is already varied by another input",
                    i.key
                )));
            }
            names.push(&i.name);
            keys.push(&i.key);
            let source = i.source.as_deref().map(str::trim).unwrap_or("");
            if source.is_empty() {
                return Err(at(
                    "`source` is required: say where this uncertainty comes from (a \
                     datasheet, a measurement, a requirement), or write \"assumed\""
                        .to_string(),
                ));
            }
            inputs.push(Input {
                name: i.name.clone(),
                key: i.key.clone(),
                dist: build_dist(i).map_err(at)?,
                source: source.to_string(),
                assumed: is_assumed(source),
                unit: i.unit.clone(),
            });
        }

        if spec.outputs.is_empty() {
            return Err("at least one [[output]] is needed".into());
        }
        let (mut scalars, mut series, mut order) = (Vec::new(), Vec::new(), Vec::new());
        for o in &spec.outputs {
            check_name("output", &o.name)?;
            let at = |e: String| format!("output `{}`: {e}", o.name);
            if order.contains(&o.name) {
                return Err(format!("output `{}` is declared twice", o.name));
            }
            order.push(o.name.clone());
            let limit = build_limit(&o.limit).map_err(at)?;
            match (&o.path, &o.series, &o.time) {
                (Some(path), None, None) => {
                    if paths::is_series(path) {
                        return Err(at(format!(
                            "`path` `{path}` names rows; a scalar output needs one value \
                             (use `series` and `time` for a band)"
                        )));
                    }
                    scalars.push(ScalarOutput {
                        name: o.name.clone(),
                        path: path.clone(),
                        limit,
                        unit: o.unit.clone(),
                    });
                }
                (None, Some(s), Some(t)) => {
                    for p in [s, t] {
                        if !paths::is_series(p) {
                            return Err(at(format!(
                                "`{p}` does not name a series (it needs one `[]` segment)"
                            )));
                        }
                    }
                    series.push(SeriesOutput {
                        name: o.name.clone(),
                        series: s.clone(),
                        time: t.clone(),
                        limit,
                        unit: o.unit.clone(),
                    });
                }
                _ => {
                    return Err(at(
                        "give either `path` (a scalar) or both `series` and `time` (a band)"
                            .to_string(),
                    ))
                }
            }
        }

        let method = Method::parse(&spec.sampling.method)?;
        if spec.sampling.n < 2 {
            return Err(format!(
                "sampling `n` must be at least 2 (got {})",
                spec.sampling.n
            ));
        }
        let sensitivity = match spec.sensitivity.method.as_str() {
            "none" => Sensitivity::None,
            "src" => Sensitivity::Src,
            "sobol" => Sensitivity::Sobol,
            other => {
                return Err(format!(
                    "sensitivity method must be \"none\", \"src\" or \"sobol\" (got \"{other}\")"
                ))
            }
        };
        if sensitivity != Sensitivity::None && inputs.is_empty() {
            return Err(format!(
                "sensitivity method \"{}\" needs at least one [[input]]",
                sensitivity.as_str()
            ));
        }
        if sensitivity != Sensitivity::None && scalars.is_empty() {
            return Err(
                "sensitivity is computed for scalar outputs; declare an [[output]] with `path`"
                    .into(),
            );
        }
        if inputs.is_empty() && !spec.sampling.vary_seed {
            return Err(
                "nothing varies: declare at least one [[input]] or set sampling.vary_seed = true"
                    .into(),
            );
        }
        if let Some(0) = spec.run.threads {
            return Err("run `threads` must be at least 1".into());
        }
        let runs = match sensitivity {
            Sensitivity::Sobol => spec.sampling.n.checked_mul(inputs.len() + 2),
            _ => Some(spec.sampling.n),
        };
        if runs.is_none_or(|r| r > MAX_RUNS) {
            return Err(format!(
                "the study needs {} engine runs; one study may make at most {MAX_RUNS} (every \
                 finished realisation is held in memory until the result is written)",
                runs.map_or_else(|| "more than 2^64".to_string(), |r| r.to_string())
            ));
        }
        Ok(Plan {
            title: spec.title.clone(),
            seed: spec.seed,
            inputs,
            scalars,
            series,
            output_order: order,
            method,
            n: spec.sampling.n,
            vary_seed: spec.sampling.vary_seed,
            seed_key: spec.sampling.seed_key.clone(),
            keep_samples: spec.sampling.keep_samples,
            sensitivity,
        })
    }

    /// How many engine runs the study needs (the base run is not counted).
    pub fn runs_total(&self) -> usize {
        match self.sensitivity {
            Sensitivity::Sobol => self.n * (self.inputs.len() + 2),
            _ => self.n,
        }
    }

    /// Every realisation of the design, in order. `base_seed` is the base scenario's
    /// seed when the study varies it.
    ///
    /// Plain designs: realisation `k` gets seed `base + k`. Sobol' designs: row `j` of
    /// `A` and of every `A_B^(i)` gets `base + j`, and row `j` of `B` gets
    /// `base + n + j`; the seed thus behaves as one more column that is always taken
    /// from `A`, so a first-order or total index never mixes in seed noise from a
    /// changed seed.
    pub fn jobs(&self, base_seed: Option<u64>) -> Result<Vec<Job>, String> {
        let k = self.inputs.len();
        let unit_rows = match self.sensitivity {
            Sensitivity::Sobol => sampling::saltelli_unit_rows(self.method, self.n, k, self.seed),
            _ => sampling::unit_sample(self.method, self.n, k, self.seed, Matrix::A),
        };
        unit_rows
            .into_iter()
            .enumerate()
            .map(|(index, u)| {
                let seed = match base_seed {
                    None => None,
                    Some(base) => {
                        let offset = match self.sensitivity {
                            Sensitivity::Sobol => match sampling::sobol_block(self.n, k, index) {
                                Some((Block::B, row)) => self.n + row,
                                Some((_, row)) => row,
                                None => index,
                            },
                            _ => index,
                        };
                        Some(
                            base.checked_add(offset as u64)
                                .filter(|s| i64::try_from(*s).is_ok())
                                .ok_or("the engine seed overflows over the ensemble")?,
                        )
                    }
                };
                // A distribution whose tail overflows (a lognormal with a very large
                // `ln_std`, a normal with a huge `std`) would hand the engine an infinity;
                // the study is refused before anything runs.
                let inputs = self
                    .inputs
                    .iter()
                    .zip(&u)
                    .map(|(inp, u)| {
                        let v = inp.dist.inverse_cdf(*u);
                        if v.is_finite() {
                            Ok(v)
                        } else {
                            Err(format!(
                                "input `{}`: its distribution gives a value that is not a \
                                 finite number ({v}) at realisation {index}; narrow the \
                                 distribution",
                                inp.name
                            ))
                        }
                    })
                    .collect::<Result<Vec<f64>, String>>()?;
                Ok(Job {
                    index,
                    inputs,
                    seed,
                })
            })
            .collect()
    }
}

// ─── What is known about the base (the unperturbed) run ────────────────────────────

/// The tier the base run grants one figure.
#[derive(Debug, Clone, Default)]
pub struct BaseTier {
    /// The tier in the base run's own `figure_tiers` block, when it lists the path.
    pub tier: Option<String>,
    /// The verification-matrix row that tier is read from.
    pub requirement: Option<String>,
}

/// What the base run says about the scenario and each output.
#[derive(Debug, Clone, Default)]
pub struct BaseInfo {
    /// The base scenario's kind.
    pub kind: String,
    pub engine_version: String,
    /// The base run's own label, verbatim.
    pub label: Option<String>,
    pub scenario_hash: Option<String>,
    pub result_sha256: Option<String>,
    /// The base scenario's seed, when the study varies it.
    pub base_seed: Option<u64>,
    /// Each input's value in the base scenario.
    pub input_nominal: Vec<Option<f64>>,
    pub scalar_nominal: Vec<Option<f64>>,
    pub scalar_units: Vec<String>,
    pub scalar_tiers: Vec<BaseTier>,
    pub series_nominal: Vec<Option<Vec<f64>>>,
    pub series_time: Vec<Vec<f64>>,
    /// Per series output: (unit of the values, unit of the time axis).
    pub series_units: Vec<(String, String)>,
    pub series_tiers: Vec<BaseTier>,
}

/// The tier an ensemble statistic carries, given the tier the base kind grants the
/// figure for one run. It is never above MODELLED: the input distributions are the
/// user's statement, so no number of runs turns the statistic into a validated one. A
/// figure the base kind marks as partner-owned stays partner-owned.
pub fn study_tier(base: Option<&str>) -> &'static str {
    match base {
        Some("PARTNER") => "PARTNER",
        _ => "MODELLED",
    }
}

// ─── Hashing ───────────────────────────────────────────────────────────────────────

/// SHA-256 of `bytes`, in hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// The canonical JSON form of a parsed study: the validated content with every default
/// filled in, the base scenario as parsed (so its comments and layout do not count),
/// keys in sorted order, and without the execution settings (`[run]`), which cannot
/// change a result.
pub fn canonical_study(plan: &Plan, base: &toml::Value) -> Result<String, String> {
    let limit = |l: &Option<(LimitOp, f64)>| match l {
        Some((op, v)) => json!({"op": op.as_str(), "value": v}),
        None => Value::Null,
    };
    let doc = json!({
        "kind": KIND,
        "title": plan.title,
        "seed": plan.seed,
        "base_scenario": serde_json::to_value(base).map_err(|e| e.to_string())?,
        "inputs": plan.inputs.iter().map(|i| json!({
            "name": i.name, "key": i.key, "dist": i.dist.to_json(),
            "source": i.source, "unit": i.unit,
        })).collect::<Vec<_>>(),
        "outputs": plan.output_order.iter().map(|name| {
            if let Some(s) = plan.scalars.iter().find(|s| &s.name == name) {
                json!({"name": s.name, "path": s.path, "limit": limit(&s.limit), "unit": s.unit})
            } else {
                let s = plan.series.iter().find(|s| &s.name == name)
                    .expect("every declared output is a scalar or a series");
                json!({"name": s.name, "series": s.series, "time": s.time,
                       "limit": limit(&s.limit), "unit": s.unit})
            }
        }).collect::<Vec<_>>(),
        "sampling": {
            "method": plan.method.as_str(), "n": plan.n, "vary_seed": plan.vary_seed,
            "seed_key": plan.seed_key, "keep_samples": plan.keep_samples,
        },
        "sensitivity": {"method": plan.sensitivity.as_str()},
    });
    serde_json::to_string(&doc).map_err(|e| e.to_string())
}

// ─── The open engine as an evaluator ───────────────────────────────────────────────

/// The TOML value to write at a key that currently holds `current`.
fn toml_number(current: &toml::Value, value: f64) -> Result<toml::Value, String> {
    match current {
        toml::Value::Float(_) => Ok(toml::Value::Float(value)),
        toml::Value::Integer(_) => {
            if value.fract() == 0.0 && value.abs() < 9.0e15 {
                Ok(toml::Value::Integer(value as i64))
            } else {
                Err(format!(
                    "the key holds a whole number, and {value} is not one (use dist \
                     \"discrete\" with whole-number values)"
                ))
            }
        }
        other => Err(format!(
            "the key holds a {}, not a number",
            other.type_str()
        )),
    }
}

/// Runs the open engine for one realisation and reads the watched outputs.
struct EngineEvaluator<'a> {
    base: &'a toml::Value,
    plan: &'a Plan,
    /// Length of each series output in the base run; a realisation whose series has
    /// another length does not contribute to the band.
    series_len: Vec<usize>,
}

fn run_engine(scenario: &toml::Value) -> Result<(String, Value), String> {
    let src = toml::to_string(scenario).map_err(|e| format!("serialise the scenario: {e}"))?;
    let out = kshana::api::run_scenario(&src).map_err(|e| format!("{e}"))?;
    let doc: Value = serde_json::from_str(&out.json)
        .map_err(|e| format!("the result document did not parse: {e}"))?;
    Ok((out.json, doc))
}

impl Evaluator for EngineEvaluator<'_> {
    fn evaluate(&self, inputs: &[f64], seed: Option<u64>) -> Evaluation {
        let mut scn = self.base.clone();
        for (inp, value) in self.plan.inputs.iter().zip(inputs) {
            let set = paths::get_key(&scn, &inp.key)
                .and_then(|cur| toml_number(cur, *value))
                .and_then(|v| paths::set_key(&mut scn, &inp.key, v));
            if let Err(e) = set {
                return Evaluation::failed(format!("input `{}`: {e}", inp.name));
            }
        }
        if let Some(seed) = seed {
            let v = toml::Value::Integer(seed as i64);
            if let Err(e) = paths::set_key(&mut scn, &self.plan.seed_key, v) {
                return Evaluation::failed(e);
            }
        }
        let (json, doc) = match run_engine(&scn) {
            Ok(x) => x,
            Err(e) => return Evaluation::failed(e),
        };
        Evaluation {
            scalars: self
                .plan
                .scalars
                .iter()
                .map(|o| paths::scalar(&doc, &o.path).ok().flatten())
                .collect(),
            series: self
                .plan
                .series
                .iter()
                .zip(&self.series_len)
                .map(|(o, len)| {
                    paths::series(&doc, &o.series)
                        .ok()
                        .filter(|s| s.len() == *len)
                })
                .collect(),
            result_sha256: Some(sha256_hex(json.as_bytes())),
            error: None,
        }
    }
}

/// The engine's version: from the base run when its kind reports it, otherwise from
/// the smallest open run that does.
fn engine_version(base_doc: &Value) -> String {
    if let Some(v) = base_doc.get("engine_version").and_then(Value::as_str) {
        return v.to_string();
    }
    let probe = "kind = \"clock\"\nseed = 1\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 20.0\n[gnss]\nwindows = [ { t0 = 0.0, t1 = 20.0, state = \"nominal\" } ]\n[clock_quantum]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n[clock_classical]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n";
    kshana::api::run_scenario(probe)
        .ok()
        .and_then(|o| serde_json::from_str::<Value>(&o.json).ok())
        .and_then(|d| {
            d.get("engine_version")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| "(unreported)".to_string())
}

fn base_tier(base_doc: &Value, path: &str) -> BaseTier {
    let canonical = paths::canonical(path);
    base_doc
        .get("figure_tiers")
        .and_then(|f| f.get("figures"))
        .and_then(Value::as_array)
        .and_then(|figs| {
            figs.iter().find(|f| {
                f.get("path")
                    .and_then(Value::as_str)
                    .is_some_and(|p| p == path || p == canonical)
            })
        })
        .map(|f| BaseTier {
            tier: f.get("tier").and_then(Value::as_str).map(str::to_string),
            requirement: f
                .get("requirement")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
        .unwrap_or_default()
}

/// A study ready to run: the plan, the parsed base scenario, its hash, the base run's
/// facts and the full design.
pub struct Prepared {
    pub plan: Plan,
    pub base: toml::Value,
    /// SHA-256 of [`canonical_study`].
    pub study_hash: String,
    pub info: BaseInfo,
    pub jobs: Vec<Job>,
    /// Threads the study file asks for.
    pub threads: Option<usize>,
}

/// Parse and validate a study, run the base scenario once, and lay out the design.
pub fn prepare(study_toml: &str) -> Result<Prepared, String> {
    let spec: StudySpec =
        toml::from_str(study_toml).map_err(|e| format!("invalid {KIND} study: {e}"))?;
    prepare_spec(&spec)
}

/// [`prepare`] for a study that is already parsed.
pub fn prepare_spec(spec: &StudySpec) -> Result<Prepared, String> {
    let plan = Plan::from_spec(spec)?;
    let base: toml::Value = toml::from_str(&spec.base_scenario)
        .map_err(|e| format!("`base_scenario` is not valid TOML: {e}"))?;
    let kind = base
        .get("kind")
        .and_then(|k| k.as_str())
        .unwrap_or("clock")
        .to_string();
    if kind.starts_with(crate::registry::PRO_KIND_PREFIX) {
        return Err(format!(
            "`base_scenario` is of kind \"{kind}\"; the base of a study must be an open \
             scenario kind"
        ));
    }

    // Every input key must exist and hold a number the distribution can replace.
    let mut input_nominal = Vec::with_capacity(plan.inputs.len());
    for inp in &plan.inputs {
        let at = |e: String| format!("input `{}`: {e}", inp.name);
        let cur = paths::get_key(&base, &inp.key).map_err(at)?;
        match cur {
            toml::Value::Float(f) => input_nominal.push(Some(*f)),
            toml::Value::Integer(i) => {
                let whole = match &inp.dist {
                    Dist::Discrete { values, .. } => values.iter().all(|v| v.fract() == 0.0),
                    _ => false,
                };
                if !whole {
                    return Err(at(format!(
                        "key `{}` holds a whole number; vary it with dist \"discrete\" and \
                         whole-number values",
                        inp.key
                    )));
                }
                input_nominal.push(Some(*i as f64));
            }
            other => {
                return Err(at(format!(
                    "key `{}` holds a {}, not a number",
                    inp.key,
                    other.type_str()
                )))
            }
        }
    }

    let base_seed = if plan.vary_seed {
        let v = paths::get_key(&base, &plan.seed_key).map_err(|e| {
            format!("sampling.vary_seed: {e} (set sampling.seed_key to the scenario's seed field)")
        })?;
        Some(
            v.as_integer()
                .and_then(|i| u64::try_from(i).ok())
                .ok_or_else(|| {
                    format!(
                        "sampling.vary_seed: seed key `{}` is not a non-negative whole number",
                        plan.seed_key
                    )
                })?,
        )
    } else {
        None
    };

    // The base run: the scenario exactly as written. It proves the scenario runs, and
    // it is where every output's unit, tier and nominal value are read from.
    let (base_json, base_doc) =
        run_engine(&base).map_err(|e| format!("the base scenario does not run: {e}"))?;

    let mut info = BaseInfo {
        kind,
        engine_version: engine_version(&base_doc),
        label: base_doc
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string),
        scenario_hash: base_doc
            .get("scenario_hash")
            .and_then(Value::as_str)
            .map(str::to_string),
        result_sha256: Some(sha256_hex(base_json.as_bytes())),
        base_seed,
        input_nominal,
        ..BaseInfo::default()
    };
    // The unit of an output is the base kind's own. A declared `unit` fills the gap only
    // where the base kind's units block says nothing; one that disagrees with the base
    // kind is refused, so a result can never carry a unit the engine did not report.
    let unit_of = |declared: &Option<String>, path: &str, name: &str| -> Result<String, String> {
        match (declared, paths::unit_in_doc(&base_doc, path)) {
            (Some(d), Some(engine)) if *d != engine => Err(format!(
                "output `{name}`: `unit` is \"{d}\", but the base kind reports `{path}` in \
                 \"{engine}\"; remove `unit` (it is read from the base run) or correct it"
            )),
            (_, Some(engine)) => Ok(engine),
            (Some(d), None) => Ok(d.clone()),
            (None, None) => Err(format!(
                "output `{name}`: path `{path}` has no entry in the base kind's units block; \
                 declare its `unit` on the [[output]]"
            )),
        }
    };
    for o in &plan.scalars {
        let nominal =
            paths::scalar(&base_doc, &o.path).map_err(|e| format!("output `{}`: {e}", o.name))?;
        info.scalar_nominal.push(nominal);
        info.scalar_units.push(unit_of(&o.unit, &o.path, &o.name)?);
        info.scalar_tiers.push(base_tier(&base_doc, &o.path));
    }
    for o in &plan.series {
        let at = |e: String| format!("output `{}`: {e}", o.name);
        let values = paths::series(&base_doc, &o.series).map_err(at)?;
        let time = paths::series(&base_doc, &o.time).map_err(at)?;
        if values.len() != time.len() {
            return Err(at(format!(
                "`series` has {} rows but `time` has {}",
                values.len(),
                time.len()
            )));
        }
        if values.is_empty() {
            return Err(at("the series is empty in the base run".to_string()));
        }
        let time_unit = paths::unit_in_doc(&base_doc, &o.time).ok_or_else(|| {
            at(format!(
                "time path `{}` has no entry in the base kind's units block",
                o.time
            ))
        })?;
        info.series_units
            .push((unit_of(&o.unit, &o.series, &o.name)?, time_unit));
        info.series_tiers.push(base_tier(&base_doc, &o.series));
        info.series_nominal.push(Some(values));
        info.series_time.push(time);
    }

    let study_hash = sha256_hex(canonical_study(&plan, &base)?.as_bytes());
    let jobs = plan.jobs(base_seed)?;
    Ok(Prepared {
        plan,
        base,
        study_hash,
        info,
        jobs,
        threads: spec.run.threads,
    })
}

// ─── Running ───────────────────────────────────────────────────────────────────────

/// How a study is run.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Worker threads; `None` takes the study file's `[run] threads`, else the
    /// machine's parallelism (at most eight).
    pub threads: Option<usize>,
    /// The checkpoint file (`runs.jsonl`); `None` runs in memory.
    pub checkpoint: Option<PathBuf>,
    /// Continue an existing checkpoint file.
    pub resume: bool,
    /// Stop cleanly after this many new runs.
    pub max_runs: Option<usize>,
}

/// A finished study.
#[derive(Debug, Clone)]
pub struct StudyResult {
    /// The result document.
    pub doc: Value,
    /// The result document, pretty-printed: the content of `uq.result.json`.
    pub json: String,
    /// One paragraph for a terminal.
    pub summary: String,
}

/// What a call to [`run_study`] did.
#[derive(Debug, Clone)]
pub struct StudyRun {
    pub runs_total: usize,
    /// Realisations finished so far (this call and earlier ones).
    pub runs_done: usize,
    /// Realisations run by this call.
    pub new_runs: usize,
    /// Things the caller should tell the user (a repaired checkpoint, a clean stop).
    pub notes: Vec<String>,
    /// The result, once every realisation is finished.
    pub result: Option<StudyResult>,
    pub study_hash: String,
}

fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8)
}

/// Whether two floats are the same value (a not-a-number equals itself here).
fn same_f64(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || a == b
}

/// Run a study through the open engine. With a checkpoint it can be stopped
/// ([`RunOptions::max_runs`]) and resumed ([`RunOptions::resume`]).
pub fn run_study(study_toml: &str, opts: &RunOptions) -> Result<StudyRun, String> {
    let prepared = prepare(study_toml)?;
    run_prepared(&prepared, opts)
}

/// [`run_study`] for a study that is already prepared.
pub fn run_prepared(p: &Prepared, opts: &RunOptions) -> Result<StudyRun, String> {
    let runs_total = p.jobs.len();
    let mut notes = Vec::new();
    let mut done: BTreeMap<usize, RunRecord> = BTreeMap::new();
    let mut writer = None;

    if let Some(path) = &opts.checkpoint {
        let header = checkpoint::Header {
            study_hash: p.study_hash.clone(),
            runs_total,
            overlay_version: crate::version().to_string(),
            engine_version: p.info.engine_version.clone(),
        };
        if path.exists() {
            if !opts.resume {
                return Err(format!(
                    "{} already exists; pass --resume to continue that run, or choose \
                     another output directory",
                    path.display()
                ));
            }
            let loaded = checkpoint::load(path, &p.study_hash, runs_total)?;
            if loaded.header.engine_version != header.engine_version {
                return Err(format!(
                    "refused: {} was started with engine {} and this build embeds engine {}; \
                     runs of two engine versions are never mixed in one study",
                    path.display(),
                    loaded.header.engine_version,
                    header.engine_version
                ));
            }
            if let Some(note) = loaded.dropped_truncated_line {
                notes.push(note);
            }
            // A stored record must be the realisation this design defines.
            for (index, r) in &loaded.records {
                let job = &p.jobs[*index];
                let same_inputs = r.inputs.len() == job.inputs.len()
                    && r.inputs
                        .iter()
                        .zip(&job.inputs)
                        .all(|(a, b)| same_f64(*a, *b));
                if !same_inputs || r.seed != job.seed {
                    return Err(format!(
                        "refused: realisation {index} in {} does not match this study's \
                         design (its inputs or seed differ); the checkpoint was edited",
                        path.display()
                    ));
                }
            }
            done = loaded.records;
            writer = Some(checkpoint::Writer::append(path)?);
        } else {
            if opts.resume {
                notes.push(format!(
                    "--resume: {} does not exist yet; starting from the first realisation",
                    path.display()
                ));
            }
            writer = Some(checkpoint::Writer::create(path, &header)?);
        }
    }

    let mut pending: Vec<Job> = p
        .jobs
        .iter()
        .filter(|j| !done.contains_key(&j.index))
        .cloned()
        .collect();
    if let Some(max) = opts.max_runs {
        pending.truncate(max);
    }
    let evaluator = EngineEvaluator {
        base: &p.base,
        plan: &p.plan,
        series_len: p.info.series_time.iter().map(Vec::len).collect(),
    };
    let threads = opts.threads.or(p.threads).unwrap_or_else(default_threads);
    let new_runs = pending.len();
    exec::run_ordered(&pending, &evaluator, threads, &mut |record| {
        if let Some(w) = writer.as_mut() {
            w.write(&record)?;
        }
        done.insert(record.index, record);
        Ok(())
    })?;
    drop(writer);

    let runs_done = done.len();
    let result = if runs_done == runs_total {
        let records: Vec<RunRecord> = done.into_values().collect();
        Some(analyse(&p.plan, &p.info, &p.study_hash, &records))
    } else {
        notes.push(format!(
            "stopped after {new_runs} new run(s): {runs_done} of {runs_total} realisations are \
             finished. Run again with --resume to continue"
        ));
        None
    };
    Ok(StudyRun {
        runs_total,
        runs_done,
        new_runs,
        notes,
        result,
        study_hash: p.study_hash.clone(),
    })
}

/// Run a plan over any [`Evaluator`] (a closure, a surrogate), in memory. There is no
/// base scenario: `info` states what is known about the evaluator's outputs.
pub fn run_with_evaluator(
    plan: &Plan,
    info: &BaseInfo,
    evaluator: &dyn Evaluator,
    threads: usize,
) -> Result<StudyResult, String> {
    let jobs = plan.jobs(info.base_seed)?;
    let mut records = Vec::with_capacity(jobs.len());
    exec::run_ordered(&jobs, evaluator, threads, &mut |r| {
        records.push(r);
        Ok(())
    })?;
    let hash =
        sha256_hex(canonical_study(plan, &toml::Value::Table(Default::default()))?.as_bytes());
    Ok(analyse(plan, info, &hash, &records))
}

// ─── Analysis: records → the result document ───────────────────────────────────────

/// Accumulates the `units` block.
struct Units(Map<String, Value>);

impl Units {
    fn add(&mut self, path: &str, unit: &str, provenance: &str, note: &str) {
        let mut e = Map::new();
        e.insert("unit".into(), json!(unit));
        e.insert("provenance".into(), json!(provenance));
        if !note.is_empty() {
            e.insert("note".into(), json!(note));
        }
        self.0.insert(path.to_string(), Value::Object(e));
    }
}

const SIBLING_UNIT: &str = "see the sibling `unit` field";

fn num(v: f64) -> Value {
    if v.is_finite() {
        json!(v)
    } else {
        Value::Null
    }
}

fn opt(v: Option<f64>) -> Value {
    v.map(num).unwrap_or(Value::Null)
}

fn squared(unit: &str) -> String {
    if unit == "1" {
        "1".to_string()
    } else if unit.chars().all(|c| c.is_ascii_alphanumeric()) {
        format!("{unit}^2")
    } else {
        format!("({unit})^2")
    }
}

/// The realisations the statistics are taken over: the first `n` (the plain sample, or
/// matrix `A` of a Sobol' design).
fn stat_block<'a>(plan: &Plan, records: &'a [RunRecord]) -> &'a [RunRecord] {
    &records[..plan.n.min(records.len())]
}

/// Turn the finished realisations (all of them, in order) into the result document.
pub fn analyse(
    plan: &Plan,
    info: &BaseInfo,
    study_hash: &str,
    records: &[RunRecord],
) -> StudyResult {
    let mut units = Units(Map::new());
    let k = plan.inputs.len();
    let stat = stat_block(plan, records);
    let n_failed_all = records.iter().filter(|r| r.error.is_some()).count();
    let n_failed_stat = stat.iter().filter(|r| r.error.is_some()).count();
    let boot_base = plan.seed ^ info.base_seed.unwrap_or(0) ^ 0xC0FF_EE00;

    // ── inputs ──
    let inputs_json: Vec<Value> = plan
        .inputs
        .iter()
        .enumerate()
        .map(|(i, inp)| {
            let col: Vec<f64> = stat.iter().map(|r| r.inputs[i]).collect();
            let s = stats::sorted(&col);
            json!({
                "name": inp.name,
                "key": inp.key,
                "unit": inp.unit.clone().unwrap_or_else(|| format!("unit of the scenario key `{}`", inp.key)),
                "dist": inp.dist.to_json(),
                "distribution": inp.dist.describe(),
                "source": inp.source,
                "source_class": if inp.assumed { "ASSUMED" } else { "STATED" },
                "nominal": opt(info.input_nominal.get(i).copied().flatten()),
                "sample": {
                    "mean": num(stats::mean(&col)),
                    "std": num(stats::variance_pop(&col).sqrt()),
                    "min": num(s.first().copied().unwrap_or(f64::NAN)),
                    "max": num(s.last().copied().unwrap_or(f64::NAN)),
                },
            })
        })
        .collect();
    for f in ["mean", "std", "min", "max", "mode", "median", "values[]"] {
        units.add(
            &format!("inputs[].dist.{f}"),
            SIBLING_UNIT,
            "input",
            "parameter of the input's stated distribution, in the input's own unit",
        );
    }
    units.add(
        "inputs[].dist.ln_std",
        "1",
        "input",
        "standard deviation of the natural logarithm of a lognormal input",
    );
    units.add(
        "inputs[].dist.probabilities[]",
        "1",
        "input",
        "probability of each value of a discrete input (the weights, normalised)",
    );
    units.add(
        "inputs[].nominal",
        SIBLING_UNIT,
        "input",
        "the input's value in the base scenario",
    );
    for (f, what) in [
        ("mean", "mean"),
        ("std", "standard deviation (divisor n)"),
        ("min", "smallest value"),
        ("max", "largest value"),
    ] {
        units.add(
            &format!("inputs[].sample.{f}"),
            SIBLING_UNIT,
            "computed",
            &format!("{what} of the values actually drawn for this input"),
        );
    }

    // ── scalar outputs ──
    let mut outputs = Map::new();
    for (j, o) in plan.scalars.iter().enumerate() {
        let unit = info
            .scalar_units
            .get(j)
            .cloned()
            .unwrap_or_else(|| "1".into());
        let per_run: Vec<Option<f64>> = stat
            .iter()
            .map(|r| {
                if r.error.is_some() {
                    None
                } else {
                    r.scalars.get(j).copied().flatten()
                }
            })
            .collect();
        let values: Vec<f64> = per_run.iter().flatten().copied().collect();
        let prefix = format!("outputs.{}", o.name);
        let mut e = Map::new();
        e.insert("path".into(), json!(o.path));
        e.insert("unit".into(), json!(unit));
        e.insert(
            "nominal".into(),
            opt(info.scalar_nominal.get(j).copied().flatten()),
        );
        e.insert("n".into(), json!(values.len()));
        match stats::summarise(&values, boot_base ^ j as u64) {
            Some(s) => {
                for (key, v) in [
                    ("mean", s.mean),
                    ("std", s.std),
                    ("std_sample", s.std_sample),
                    ("min", s.min),
                    ("max", s.max),
                    ("p05", s.p05),
                    ("p50", s.p50),
                    ("p95", s.p95),
                    ("ci95_low", s.ci95_low),
                    ("ci95_high", s.ci95_high),
                ] {
                    e.insert(key.into(), num(v));
                }
            }
            None => {
                e.insert(
                    "reason".into(),
                    json!("no realisation gave a value for this output"),
                );
            }
        }
        if let Some((op, limit)) = o.limit {
            let n_met = values.iter().filter(|v| op.met(**v, limit)).count();
            let (lo, hi) = stats::wilson_ci95(n_met, stat.len());
            e.insert(
                "limit".into(),
                json!({
                    "op": op.as_str(),
                    "value": limit,
                    "statement": format!(
                        "probability of meeting the limit ({} {}), given the stated input uncertainty",
                        o.name, op.phrase(limit, &unit)
                    ),
                    "n_met": n_met,
                    "n_evaluated": values.len(),
                    "n_realisations": stat.len(),
                    "probability": num(n_met as f64 / stat.len().max(1) as f64),
                    "probability_ci95_low": num(lo),
                    "probability_ci95_high": num(hi),
                    "probability_among_evaluated": if values.is_empty() { Value::Null }
                        else { num(n_met as f64 / values.len() as f64) },
                    "counting_rule": "a realisation that failed or gave no value is counted as NOT meeting the limit",
                }),
            );
            units.add(
                &format!("{prefix}.limit.value"),
                &unit,
                "input",
                "the limit, in the output's own unit",
            );
            units.add(
                &format!("{prefix}.limit.n_met"),
                "count",
                "computed",
                "realisations whose value meets the limit",
            );
            units.add(
                &format!("{prefix}.limit.n_evaluated"),
                "count",
                "computed",
                "realisations that gave a value",
            );
            units.add(
                &format!("{prefix}.limit.n_realisations"),
                "count",
                "computed",
                "realisations the probability is taken over (failed ones included)",
            );
            units.add(&format!("{prefix}.limit.probability"), "1", "computed", "n_met / n_realisations: probability of meeting the limit, given the stated input uncertainty");
            units.add(
                &format!("{prefix}.limit.probability_ci95_low"),
                "1",
                "computed",
                "lower end of the Wilson score 95% interval on the probability",
            );
            units.add(
                &format!("{prefix}.limit.probability_ci95_high"),
                "1",
                "computed",
                "upper end of the Wilson score 95% interval on the probability",
            );
            units.add(
                &format!("{prefix}.limit.probability_among_evaluated"),
                "1",
                "computed",
                "n_met / n_evaluated: the share among the realisations that gave a value",
            );
        }
        // Running estimate as realisations accumulate (over those that gave a value).
        let conv: Vec<Value> = stats::running(&values)
            .into_iter()
            .map(|p| {
                let mut c = json!({
                    "n": p.n, "mean": num(p.mean), "std": num(p.std),
                    "p05": num(p.p05), "p50": num(p.p50), "p95": num(p.p95),
                });
                if let Some((op, limit)) = o.limit {
                    let met = values[..p.n].iter().filter(|v| op.met(**v, limit)).count();
                    c["probability_among_evaluated"] = num(met as f64 / p.n as f64);
                }
                c
            })
            .collect();
        e.insert("convergence".into(), Value::Array(conv));
        if plan.keep_samples {
            e.insert(
                "samples".into(),
                Value::Array(per_run.iter().map(|v| opt(*v)).collect()),
            );
        }
        outputs.insert(o.name.clone(), Value::Object(e));

        units.add(
            &format!("{prefix}.nominal"),
            &unit,
            "computed",
            "the output in the base run (the scenario as written)",
        );
        units.add(
            &format!("{prefix}.n"),
            "count",
            "computed",
            "realisations that gave a value; every statistic of this output is over these",
        );
        for (f, what) in [
            ("mean", "mean over the realisations"),
            ("std", "standard deviation over the realisations, divisor n (the open campaign's definition)"),
            ("std_sample", "standard deviation over the realisations, divisor n - 1"),
            ("min", "smallest value"),
            ("max", "largest value"),
            ("p05", "5th percentile (nearest rank)"),
            ("p50", "50th percentile, the median (nearest rank)"),
            ("p95", "95th percentile (nearest rank)"),
            ("ci95_low", "lower end of the fixed-seed percentile-bootstrap 95% confidence interval on the mean (2000 resamples)"),
            ("ci95_high", "upper end of the fixed-seed percentile-bootstrap 95% confidence interval on the mean (2000 resamples)"),
        ] {
            units.add(&format!("{prefix}.{f}"), &unit, "computed", what);
        }
        units.add(&format!("{prefix}.samples[]"), &unit, "computed", "the output of each realisation, in realisation order; null where the run failed or gave no value");
        units.add(
            &format!("{prefix}.convergence[].n"),
            "count",
            "computed",
            "the first n realisations that gave a value",
        );
        for f in ["mean", "std", "p05", "p50", "p95"] {
            units.add(
                &format!("{prefix}.convergence[].{f}"),
                &unit,
                "computed",
                "the statistic over the first n realisations that gave a value",
            );
        }
        units.add(
            &format!("{prefix}.convergence[].probability_among_evaluated"),
            "1",
            "computed",
            "share of the first n realisations that gave a value and meet the limit",
        );
    }

    // ── bands ──
    let mut bands_json = Map::new();
    for (j, o) in plan.series.iter().enumerate() {
        let (unit, time_unit) = info
            .series_units
            .get(j)
            .cloned()
            .unwrap_or_else(|| ("1".into(), "1".into()));
        let time = info.series_time.get(j).cloned().unwrap_or_default();
        let contributing: Vec<&[f64]> = stat
            .iter()
            .filter(|r| r.error.is_none())
            .filter_map(|r| r.series.get(j).and_then(|s| s.as_deref()))
            .collect();
        let b = bands::band(&contributing, time.len(), o.limit);
        let prefix = format!("bands.{}", o.name);
        let arr = |xs: &[Option<f64>]| Value::Array(xs.iter().map(|x| opt(*x)).collect());
        let mut e = Map::new();
        e.insert("series_path".into(), json!(o.series));
        e.insert("time_path".into(), json!(o.time));
        e.insert("unit".into(), json!(unit));
        e.insert("time_unit".into(), json!(time_unit));
        e.insert("n".into(), json!(b.n));
        e.insert(
            "t".into(),
            Value::Array(time.iter().map(|t| num(*t)).collect()),
        );
        e.insert("p05".into(), arr(&b.p05));
        e.insert("p50".into(), arr(&b.p50));
        e.insert("p95".into(), arr(&b.p95));
        e.insert("n_at_step".into(), json!(b.n_at_step));
        e.insert(
            "nominal".into(),
            match info.series_nominal.get(j).and_then(|s| s.as_ref()) {
                Some(xs) => Value::Array(xs.iter().map(|x| num(*x)).collect()),
                None => Value::Null,
            },
        );
        if let Some((op, limit)) = o.limit {
            e.insert(
                "limit".into(),
                json!({
                    "op": op.as_str(), "value": limit,
                    "statement": format!(
                        "share of realisations meeting the limit ({} {}) at each time step, given the stated input uncertainty",
                        o.name, op.phrase(limit, &unit)
                    ),
                }),
            );
            e.insert("share_meeting_limit".into(), arr(&b.share_meeting_limit));
            units.add(
                &format!("{prefix}.limit.value"),
                &unit,
                "input",
                "the limit, in the series' own unit",
            );
            units.add(
                &format!("{prefix}.share_meeting_limit[]"),
                "1",
                "computed",
                "share of the realisations with a value at this time step that meet the limit",
            );
        }
        bands_json.insert(o.name.clone(), Value::Object(e));
        units.add(
            &format!("{prefix}.n"),
            "count",
            "computed",
            "realisations that contributed a series",
        );
        units.add(
            &format!("{prefix}.n_at_step[]"),
            "count",
            "computed",
            "realisations with a value at this time step",
        );
        units.add(
            &format!("{prefix}.t[]"),
            &time_unit,
            "computed",
            "time of each step, from the base run",
        );
        units.add(
            &format!("{prefix}.p05[]"),
            &unit,
            "computed",
            "5th percentile (nearest rank) over the realisations at this time step",
        );
        units.add(
            &format!("{prefix}.p50[]"),
            &unit,
            "computed",
            "50th percentile, the median (nearest rank) over the realisations at this time step",
        );
        units.add(
            &format!("{prefix}.p95[]"),
            &unit,
            "computed",
            "95th percentile (nearest rank) over the realisations at this time step",
        );
        units.add(
            &format!("{prefix}.nominal[]"),
            &unit,
            "computed",
            "the series in the base run (the scenario as written)",
        );
    }

    // ── sensitivity ──
    let mut sens_outputs = Map::new();
    let mut top_drivers: Vec<String> = Vec::new();
    for (j, o) in plan.scalars.iter().enumerate() {
        let unit = info
            .scalar_units
            .get(j)
            .cloned()
            .unwrap_or_else(|| "1".into());
        let prefix = format!("sensitivity.outputs.{}", o.name);
        let value_of = |r: &RunRecord| -> Option<f64> {
            if r.error.is_some() {
                None
            } else {
                r.scalars.get(j).copied().flatten()
            }
        };
        match plan.sensitivity {
            Sensitivity::None => {}
            Sensitivity::Src => {
                let rows: Vec<&RunRecord> = stat.iter().filter(|r| value_of(r).is_some()).collect();
                let x: Vec<Vec<f64>> = rows.iter().map(|r| r.inputs.clone()).collect();
                let y: Vec<f64> = rows.iter().filter_map(|r| value_of(r)).collect();
                // Every realisation failed or gave no value: nothing to fit, and each
                // input still gets its entry (null, with the reason).
                let out = if rows.is_empty() {
                    src_rank::no_fit(
                        k,
                        0,
                        "no realisation gave a value for this output, so there is nothing to \
                         fit",
                    )
                } else {
                    src_rank::regress(&x, &y)
                };
                let rank = src_rank::rank_by_magnitude(&out.entries);
                let entries: Vec<Value> = plan
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(i, inp)| match &out.entries[i] {
                        Ok(e) => json!({
                            "input": inp.name, "src": num(e.src),
                            "coefficient": num(e.coefficient), "rank": rank[i],
                        }),
                        Err(why) => json!({
                            "input": inp.name, "src": Value::Null,
                            "coefficient": Value::Null, "rank": Value::Null, "reason": why,
                        }),
                    })
                    .collect();
                if let Some(best) = (0..k).find(|i| rank[*i] == Some(1)) {
                    if let Ok(e) = &out.entries[best] {
                        top_drivers.push(format!(
                            "{}: {} (standardised coefficient {:+.3}{})",
                            o.name,
                            plan.inputs[best].name,
                            e.src,
                            if out.reliable {
                                ""
                            } else {
                                ", ranking NOT reliable"
                            }
                        ));
                    }
                }
                sens_outputs.insert(
                    o.name.clone(),
                    json!({
                        "n_rows": out.n,
                        "r_squared": out.r_squared.map(num).unwrap_or(Value::Null),
                        "sum_src_squared": out.sum_src_squared.map(num).unwrap_or(Value::Null),
                        "reliable": out.reliable,
                        "statement": out.statement,
                        "reason": out.reason,
                        "inputs": entries,
                    }),
                );
                units.add(
                    &format!("{prefix}.n_rows"),
                    "count",
                    "computed",
                    "realisations the regression is fitted to",
                );
                units.add(&format!("{prefix}.r_squared"), "1", "computed", "coefficient of determination of the linear fit: the share of the output variance it explains");
                units.add(&format!("{prefix}.sum_src_squared"), "1", "computed", "sum of the squared standardised coefficients; equals R-squared when the sampled inputs are uncorrelated");
                units.add(&format!("{prefix}.inputs[].src"), "1", "computed", "standardised regression coefficient: output standard deviations per input standard deviation");
                units.add(
                    &format!("{prefix}.inputs[].coefficient"),
                    &format!("{unit} per unit of the input"),
                    "computed",
                    "regression slope: change of the output per unit of the input",
                );
                units.add(&format!("{prefix}.inputs[].rank"), "1", "computed", "ordinal: 1 is the input with the largest standardised coefficient in magnitude");
            }
            Sensitivity::Sobol => {
                let n = plan.n;
                let block = |b: usize| -> Vec<Option<f64>> {
                    records[b * n..(b + 1) * n].iter().map(value_of).collect()
                };
                let (ya, yb) = (block(0), block(1));
                let yab: Vec<Vec<Option<f64>>> = (0..k).map(|i| block(2 + i)).collect();
                let varies: Vec<bool> = (0..k)
                    .map(|i| {
                        let first = records[0].inputs[i];
                        records[..2 * n].iter().any(|r| r.inputs[i] != first)
                    })
                    .collect();
                let out =
                    sobol::estimate(&ya, &yb, &yab, &varies, boot_base ^ 0x50B0_1000 ^ j as u64);
                let rank = sobol::rank_by_total(&out.indices);
                let entries: Vec<Value> = plan
                    .inputs
                    .iter()
                    .enumerate()
                    .map(|(i, inp)| match &out.indices[i] {
                        Ok(x) => json!({
                            "input": inp.name,
                            "first_order": num(x.first_order),
                            "first_order_ci95_low": num(x.first_order_ci95.0),
                            "first_order_ci95_high": num(x.first_order_ci95.1),
                            "total": num(x.total),
                            "total_ci95_low": num(x.total_ci95.0),
                            "total_ci95_high": num(x.total_ci95.1),
                            "rank": rank[i],
                        }),
                        Err(why) => json!({
                            "input": inp.name,
                            "first_order": Value::Null, "first_order_ci95_low": Value::Null,
                            "first_order_ci95_high": Value::Null, "total": Value::Null,
                            "total_ci95_low": Value::Null, "total_ci95_high": Value::Null,
                            "rank": Value::Null, "reason": why,
                        }),
                    })
                    .collect();
                if let Some(best) = (0..k).find(|i| rank[*i] == Some(1)) {
                    if let Ok(x) = &out.indices[best] {
                        top_drivers.push(format!(
                            "{}: {} (total index {:.3}, 95% interval {:.3} to {:.3})",
                            o.name, plan.inputs[best].name, x.total, x.total_ci95.0, x.total_ci95.1
                        ));
                    }
                }
                let conv: Vec<Value> = stats::convergence_sizes(n)
                    .into_iter()
                    .filter_map(|m| {
                        sobol::running_point(&ya, &yb, &yab, m).map(|est| {
                            json!({
                                "n": m,
                                "first_order": est.iter().map(|e| num(e.0)).collect::<Vec<_>>(),
                                "total": est.iter().map(|e| num(e.1)).collect::<Vec<_>>(),
                            })
                        })
                    })
                    .collect();
                sens_outputs.insert(
                    o.name.clone(),
                    json!({
                        "n_rows": out.n_rows,
                        "variance": num(out.variance),
                        "interaction_share": out.interaction_share.map(num).unwrap_or(Value::Null),
                        "interaction_share_meaning": if plan.vary_seed {
                            "1 minus the sum of the first-order indices: the share of the output variance not explained by any single listed input alone (interactions between inputs, and the run-to-run effect of the engine seed)"
                        } else {
                            "1 minus the sum of the first-order indices: the share of the output variance left to interactions between inputs"
                        },
                        "reason": out.reason,
                        "inputs": entries,
                        "convergence": conv,
                    }),
                );
                units.add(
                    &format!("{prefix}.n_rows"),
                    "count",
                    "computed",
                    "rows of the design in which every one of the k + 2 runs gave a value",
                );
                units.add(
                    &format!("{prefix}.variance"),
                    &squared(&unit),
                    "computed",
                    "variance of the output over matrices A and B together",
                );
                units.add(
                    &format!("{prefix}.interaction_share"),
                    "1",
                    "computed",
                    "1 minus the sum of the first-order indices",
                );
                units.add(&format!("{prefix}.inputs[].first_order"), "1", "computed", "first-order Sobol' index (Saltelli and others 2010 estimator): share of the output variance explained by this input alone; not clamped, so an input with no effect may read slightly below zero");
                units.add(&format!("{prefix}.inputs[].first_order_ci95_low"), "1", "computed", "lower end of the percentile-bootstrap 95% interval (1000 resamples of the design rows)");
                units.add(&format!("{prefix}.inputs[].first_order_ci95_high"), "1", "computed", "upper end of the percentile-bootstrap 95% interval (1000 resamples of the design rows)");
                units.add(&format!("{prefix}.inputs[].total"), "1", "computed", "total Sobol' index (Jansen 1999 estimator): share of the output variance that involves this input, interactions included");
                units.add(&format!("{prefix}.inputs[].total_ci95_low"), "1", "computed", "lower end of the percentile-bootstrap 95% interval (1000 resamples of the design rows)");
                units.add(&format!("{prefix}.inputs[].total_ci95_high"), "1", "computed", "upper end of the percentile-bootstrap 95% interval (1000 resamples of the design rows)");
                units.add(
                    &format!("{prefix}.inputs[].rank"),
                    "1",
                    "computed",
                    "ordinal: 1 is the input with the largest total index",
                );
                units.add(
                    &format!("{prefix}.convergence[].n"),
                    "count",
                    "computed",
                    "the first n rows of the design",
                );
                units.add(
                    &format!("{prefix}.convergence[].first_order[]"),
                    "1",
                    "computed",
                    "first-order index of each input, in input order, from the first n rows",
                );
                units.add(
                    &format!("{prefix}.convergence[].total[]"),
                    "1",
                    "computed",
                    "total index of each input, in input order, from the first n rows",
                );
            }
        }
    }
    let sensitivity_json = json!({
        "method": plan.sensitivity.as_str(),
        "runs": plan.runs_total(),
        "cost": match plan.sensitivity {
            Sensitivity::None => "no sensitivity analysis was requested".to_string(),
            Sensitivity::Src => format!("no extra runs: fitted to the same {} realisations", plan.n),
            Sensitivity::Sobol => format!("n * (k + 2) = {} * ({} + 2) = {} runs", plan.n, k, plan.runs_total()),
        },
        "outputs": Value::Object(sens_outputs),
    });
    units.add(
        "sensitivity.runs",
        "count",
        "computed",
        "engine runs the study made (the base run not counted)",
    );

    // ── tiers ──
    let mut figures = Vec::new();
    let mut tier_entry = |name: &str, section: &str, base_path: &str, bt: Option<&BaseTier>| {
        let base = bt.and_then(|b| b.tier.as_deref());
        figures.push(json!({
            "path": format!("{section}.{name}"),
            "output": name,
            "base_path": base_path,
            "tier": study_tier(base),
            "base_tier": base,
            "requirement": bt.and_then(|b| b.requirement.as_deref()),
            "note": match base {
                Some(t) => format!("the base kind marks this figure {t} for one run; a statistic over stated input distributions is never above MODELLED"),
                None => "the base run's figure_tiers block does not list this path, so no tier is claimed for the single run; the statistic is MODELLED".to_string(),
            },
        }));
    };
    for (j, o) in plan.scalars.iter().enumerate() {
        tier_entry(&o.name, "outputs", &o.path, info.scalar_tiers.get(j));
    }
    for (j, o) in plan.series.iter().enumerate() {
        tier_entry(&o.name, "bands", &o.series, info.series_tiers.get(j));
    }

    // ── reproducibility ──
    let mut digest = Sha256::new();
    for r in records {
        match (&r.result_sha256, &r.error) {
            (Some(h), None) => digest.update(h.as_bytes()),
            (_, Some(e)) => digest.update(format!("error:{e}").as_bytes()),
            (None, None) => digest.update(
                format!(
                    "values:{:?}",
                    r.scalars
                        .iter()
                        .map(|v| v.map(f64::to_bits))
                        .collect::<Vec<_>>()
                )
                .as_bytes(),
            ),
        }
        digest.update(b"\n");
    }
    let run_digest = hex::encode(digest.finalize());

    let assumed: Vec<&str> = plan
        .inputs
        .iter()
        .filter(|i| i.assumed)
        .map(|i| i.name.as_str())
        .collect();
    let label = if k == 0 {
        format!(
            "MODELLED: statistics of {} open-engine runs of kind `{}` over engine seeds only \
             (no input is varied). The estimators are checked against closed forms. Every \
             figure carries the base kind's tier, never a higher one.",
            records.len(),
            info.kind
        )
    } else {
        format!(
            "MODELLED: statistics of {} open-engine runs of kind `{}` over the stated input \
             distributions. The estimators are checked against closed forms; the \
             distributions are the study's own statement{}. Every figure is conditional on \
             them and carries the base kind's tier, never a higher one.",
            records.len(),
            info.kind,
            if assumed.is_empty() {
                String::new()
            } else {
                format!(
                    " ({} of {} inputs ASSUMED: {})",
                    assumed.len(),
                    k,
                    assumed.join(", ")
                )
            }
        )
    };

    let failures: Vec<Value> = records
        .iter()
        .filter(|r| r.error.is_some())
        .take(20)
        .map(|r| json!({"index": r.index, "error": r.error}))
        .collect();
    units.add(
        "seed",
        "1",
        "input",
        "study seed: fixes the sampling design and every bootstrap",
    );
    units.add(
        "sampling.n",
        "count",
        "input",
        "realisations of the sample the statistics are taken over",
    );
    units.add(
        "sampling.base_seed",
        "1",
        "input",
        "the base scenario's seed; realisation k runs with base_seed + k",
    );
    units.add(
        "runs.total",
        "count",
        "computed",
        "engine runs of the study (the base run not counted)",
    );
    units.add("runs.ok", "count", "computed", "runs that finished");
    units.add(
        "runs.failed",
        "count",
        "computed",
        "runs the engine refused or failed; recorded, never dropped",
    );
    units.add("runs.statistics_realisations", "count", "computed", "realisations the statistics are taken over (the plain sample, or matrix A of a Sobol' design)");
    units.add(
        "runs.statistics_failed",
        "count",
        "computed",
        "of those, the ones that failed",
    );
    units.add(
        "runs.failures[].index",
        "1",
        "computed",
        "ordinal: the realisation's index in the design",
    );
    units.add(
        "reproducibility.runs_total",
        "count",
        "computed",
        "runs behind run_digest",
    );

    let mut not_modelled = vec![
        json!("Correlation between inputs: every input is sampled independently of the others. Two inputs that move together in reality are treated as unrelated here."),
        json!("The uncertainty of the model itself: the base kind's equations and their validation tier are taken as they are. More runs narrow the sampling error of a statistic; they do not change how well the model matches reality."),
        json!("The correctness of the stated distributions: every figure is conditional on them."),
        json!("Distribution shapes other than normal, uniform, lognormal, triangular and discrete."),
        json!("Sensitivity of series (band) outputs: indices and coefficients are computed for scalar outputs only."),
    ];
    if plan.vary_seed && plan.sensitivity != Sensitivity::None {
        not_modelled.push(json!("The engine seed is not ranked as an input: its run-to-run effect is part of the variance that no listed input explains."));
    }

    let mut doc = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": KIND,
        "overlay": "kshana-pro",
        "overlay_version": crate::version(),
        "engine_version": info.engine_version,
        "title": plan.title,
        "seed": plan.seed,
        "scenario_hash": study_hash,
        "label": label,
        "base": {
            "kind": info.kind,
            "label": info.label,
            "scenario_hash": info.scenario_hash,
            "result_sha256": info.result_sha256,
        },
        "inputs": inputs_json,
        "output_order": plan.output_order,
        "sampling": {
            "method": plan.method.as_str(),
            "n": plan.n,
            "vary_seed": plan.vary_seed,
            "seed_key": if plan.vary_seed { json!(plan.seed_key) } else { Value::Null },
            "base_seed": info.base_seed,
            "seed_rule": if !plan.vary_seed {
                "every realisation runs with the base scenario's own seed"
            } else if plan.sensitivity == Sensitivity::Sobol {
                "row j of matrix A and of every A_B matrix runs with base_seed + j; row j of matrix B with base_seed + n + j"
            } else {
                "realisation k runs with base_seed + k (the open campaign's rule)"
            },
        },
        "runs": {
            "total": records.len(),
            "ok": records.len() - n_failed_all,
            "failed": n_failed_all,
            "statistics_realisations": stat.len(),
            "statistics_failed": n_failed_stat,
            "statistics_over": "successful realisations only; a failed run is recorded and counted, never dropped, and never counted as meeting a limit",
            "failures": failures,
            "failures_listed": failures_listed(n_failed_all),
        },
        "outputs": Value::Object(outputs),
        "bands": Value::Object(bands_json),
        "sensitivity": sensitivity_json,
        "figure_tiers": {
            "figures": figures,
            "legend": "VALIDATED: checked against an independent external oracle. MODELLED: first-principles or published-physics model with tests, not checked against an external oracle. PARTNER: owned by a partner, not modelled by the engine. `base_tier` is what the open engine grants the figure for one run; `tier` is what this study's statistic carries, and it is never above MODELLED.",
            "source": "the base run's own figure_tiers block (open engine verification matrix)",
        },
        "not_modelled": not_modelled,
        "reproducibility": {
            "study_hash": study_hash,
            "runs_total": records.len(),
            "run_digest": run_digest,
            "run_digest_rule": "SHA-256 over, for each realisation in order, the SHA-256 of its engine result document (or its error text), each followed by a newline",
            "base_result_sha256": info.result_sha256,
            "rebuild": "the same study file, overlay version and engine version give the same result document byte for byte, for any thread count and any number of stops and resumes",
        },
    });
    doc["units"] = Value::Object(units.0);

    // ── summary ──
    let mut summary = format!(
        "Kshana Pro uncertainty | {} | {} runs of `{}` ({} sampling, n = {}{})",
        plan.title,
        records.len(),
        info.kind,
        plan.method.as_str(),
        plan.n,
        if plan.vary_seed {
            ", engine seed varied"
        } else {
            ""
        },
    );
    if n_failed_all > 0 {
        summary += &format!(
            " | {n_failed_all} run(s) FAILED and are recorded; every statistic is over the successful realisations only, and a failed run never counts as meeting a limit"
        );
    }
    for (j, o) in plan.scalars.iter().enumerate() {
        let e = &doc["outputs"][&o.name];
        let unit = info.scalar_units.get(j).map(String::as_str).unwrap_or("");
        if let (Some(m), Some(s), Some(lo), Some(hi)) = (
            e["mean"].as_f64(),
            e["std"].as_f64(),
            e["p05"].as_f64(),
            e["p95"].as_f64(),
        ) {
            summary += &format!(
                " | {}: mean {m:.6} std {s:.6} [p05 {lo:.6} .. p95 {hi:.6}] {unit}",
                o.name
            );
        }
        if let Some(p) = e["limit"]["probability"].as_f64() {
            summary += &format!(
                "; probability of meeting the limit ({}) {:.4} [95% {:.4} .. {:.4}], given the stated input uncertainty",
                o.limit.map(|(op, v)| op.phrase(v, unit)).unwrap_or_default(),
                p,
                e["limit"]["probability_ci95_low"].as_f64().unwrap_or(f64::NAN),
                e["limit"]["probability_ci95_high"].as_f64().unwrap_or(f64::NAN),
            );
        }
    }
    if !top_drivers.is_empty() {
        summary += &format!(" | main driver of {}", top_drivers.join("; of "));
    }
    if !assumed.is_empty() {
        summary += &format!(" | ASSUMED inputs: {}", assumed.join(", "));
    }
    summary += " | MODELLED";

    let json = serde_json::to_string_pretty(&doc).expect("the result document is plain data");
    StudyResult { doc, json, summary }
}

fn failures_listed(n_failed: usize) -> String {
    if n_failed > 20 {
        format!("the first 20 of {n_failed}; every failure is in the checkpoint file")
    } else {
        "all".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK: &str = r#"
kind = "pro-uncertainty"
title = "link margin under stated input uncertainty"
seed = 11

base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""

[[input]]
name = "eirp"
key = "eirp_dbw"
dist = "normal"
mean = 55.0
std = 1.0
source = "assumed"

[[input]]
name = "losses"
key = "other_losses_db"
dist = "uniform"
min = 2.0
max = 4.0
source = "link design note, lumped pointing and atmosphere range"

[[output]]
name = "margin"
path = "margin_db"
limit = { op = "ge", value = 91.0 }

[sampling]
method = "lhs"
n = 64
"#;

    fn with(extra: &str) -> String {
        format!("{LINK}\n{extra}")
    }

    #[test]
    fn every_numeric_field_of_the_result_has_a_unit_entry() {
        // The engine's own audit, over a study of each sensitivity method and with a
        // band: every numeric leaf must be described and every entry well formed.
        let clock = r#"
kind = "pro-uncertainty"
title = "clock band"
seed = 3
base_scenario = """
kind = "clock"
seed = 42
threshold_ns = 20.0
[time]
step_s = 10.0
duration_s = 600.0
[gnss]
windows = [ { t0 = 0.0, t1 = 100.0, state = "nominal" }, { t0 = 100.0, t1 = 600.0, state = "denied" } ]
[clock_quantum]
id = "q"
provenance = "test"
y0 = 0.0
q_wf = 1.0e-30
q_rw = 0.0
[clock_classical]
id = "c"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
"""
[[input]]
name = "q_wf"
key = "clock_classical.q_wf"
dist = "lognormal"
median = 9.0e-20
ln_std = 0.3
source = "assumed"
[[input]]
name = "levels"
key = "clock_classical.y0"
dist = "discrete"
values = [0.0, 1.0e-10]
source = "two build standards"
[[output]]
name = "final_error"
path = "classical.series[-1].error_ns"
limit = { op = "abs_le", value = 20.0 }
[[output]]
name = "time_error"
series = "classical.series[].error_ns"
time = "classical.series[].t"
limit = { op = "abs_le", value = 20.0 }
[sampling]
n = 24
vary_seed = true
[sensitivity]
method = "sobol"
"#;
        for study in [
            with(""),
            with("[sensitivity]\nmethod = \"src\"\n"),
            with("[sensitivity]\nmethod = \"sobol\"\n"),
            clock.to_string(),
        ] {
            let run = run_study(&study, &RunOptions::default()).expect("the study runs");
            let doc = run.result.expect("complete").doc;
            let audit = kshana::field_schema::audit_document(&doc);
            assert!(audit.missing.is_empty(), "no unit for: {:?}", audit.missing);
            assert!(
                audit.malformed.is_empty(),
                "malformed: {:?}",
                audit.malformed
            );
            assert!(audit.field_count() > 20);
        }
    }

    #[test]
    fn the_result_carries_the_pro_contract() {
        let run = run_study(
            &with("[sensitivity]\nmethod = \"src\"\n"),
            &RunOptions::default(),
        )
        .expect("the study runs");
        let r = run.result.expect("complete");
        let d = &r.doc;
        assert_eq!(d["schema_version"], SCHEMA_VERSION);
        assert_eq!(d["kind"], KIND);
        assert_eq!(d["overlay"], "kshana-pro");
        assert_eq!(d["overlay_version"], crate::version());
        assert!(d["engine_version"]
            .as_str()
            .is_some_and(|v| v.contains('.')));
        assert_eq!(d["seed"], 11);
        assert_eq!(d["scenario_hash"].as_str().map(str::len), Some(64));
        assert_eq!(d["scenario_hash"], run.study_hash);
        assert!(d["label"]
            .as_str()
            .expect("a label")
            .starts_with("MODELLED"));
        assert_eq!(d["outputs"]["margin"]["unit"], "dB");
        assert_eq!(d["units"]["outputs.margin.mean"]["unit"], "dB");
        assert_eq!(d["figure_tiers"]["figures"][0]["tier"], "MODELLED");
        assert!(d["not_modelled"]
            .as_array()
            .expect("a list")
            .iter()
            .any(|x| x
                .as_str()
                .is_some_and(|s| s.contains("Correlation between inputs"))));
        assert_eq!(d["reproducibility"]["runs_total"], 64);
        assert_eq!(
            d["reproducibility"]["run_digest"].as_str().map(str::len),
            Some(64)
        );
        // ASSUMED is printed wherever the input appears.
        assert_eq!(d["inputs"][0]["source_class"], "ASSUMED");
        assert_eq!(d["inputs"][1]["source_class"], "STATED");
        assert!(d["label"]
            .as_str()
            .expect("a label")
            .contains("ASSUMED: eirp"));
        assert!(r.summary.contains("ASSUMED inputs: eirp"));
        assert!(r.summary.contains("given the stated input uncertainty"));
    }

    #[test]
    fn a_statistic_is_never_above_modelled() {
        assert_eq!(study_tier(Some("VALIDATED")), "MODELLED");
        assert_eq!(study_tier(Some("PARTIAL")), "MODELLED");
        assert_eq!(study_tier(Some("MODELLED")), "MODELLED");
        assert_eq!(study_tier(Some("PARTNER")), "PARTNER");
        assert_eq!(study_tier(None), "MODELLED");
    }

    #[test]
    fn the_study_hash_ignores_layout_and_execution_settings_only() {
        let base = prepare(&with("")).expect("valid").study_hash;
        // Threads and a comment inside the base scenario do not change the study.
        let threads = prepare(&with("[run]\nthreads = 3\n"))
            .expect("valid")
            .study_hash;
        assert_eq!(base, threads);
        let commented = LINK.replace("band = \"x\"", "band = \"x\"   # X band");
        assert_eq!(base, prepare(&commented).expect("valid").study_hash);
        // Stating a default does not change it either.
        let stated = LINK.replace("n = 64", "n = 64\nvary_seed = false");
        assert_eq!(base, prepare(&stated).expect("valid").study_hash);
        // Anything that can change a number does.
        for (from, to) in [
            ("seed = 11", "seed = 12"),
            ("std = 1.0", "std = 1.5"),
            ("n = 64", "n = 65"),
            ("range_km = 2000.0", "range_km = 2001.0"),
            ("value = 91.0", "value = 90.0"),
            ("method = \"lhs\"", "method = \"random\""),
            ("source = \"assumed\"", "source = \"datasheet\""),
        ] {
            assert!(LINK.contains(from), "fixture no longer contains `{from}`");
            let changed = prepare(&LINK.replace(from, to)).expect("valid").study_hash;
            assert_ne!(base, changed, "`{from}` -> `{to}` must change the hash");
        }
    }

    #[test]
    fn assumed_is_recognised_as_a_word() {
        assert!(is_assumed("assumed"));
        assert!(is_assumed("Assumed: +/- 1 dB, typical"));
        assert!(is_assumed("no datasheet yet, ASSUMED"));
        assert!(!is_assumed("datasheet, section 4.2"));
        assert!(!is_assumed("unassumedly precise"));
    }

    #[test]
    fn a_declared_unit_that_disagrees_with_the_engine_is_refused() {
        let wrong = LINK.replace(
            "path = \"margin_db\"\n",
            "path = \"margin_db\"\nunit = \"km\"\n",
        );
        assert_ne!(wrong, LINK);
        let e = prepare(&wrong)
            .err()
            .expect("a unit the engine did not report");
        assert!(
            e.contains("output `margin`") && e.contains("\"km\"") && e.contains("\"dB\""),
            "{e}"
        );
        // The engine's own unit, stated again, is accepted.
        let same = LINK.replace(
            "path = \"margin_db\"\n",
            "path = \"margin_db\"\nunit = \"dB\"\n",
        );
        assert_eq!(prepare(&same).expect("valid").info.scalar_units, vec!["dB"]);
    }

    #[test]
    fn a_distribution_that_overflows_is_refused_before_anything_runs() {
        let wide = LINK.replace("min = 2.0\nmax = 4.0", "min = -1.7e308\nmax = 1.7e308");
        assert_ne!(wide, LINK);
        let e = prepare(&wide)
            .err()
            .expect("a range wider than a finite number");
        assert!(
            e.contains("input `losses`") && e.contains("too wide"),
            "{e}"
        );
        let tail = LINK.replace(
            "dist = \"normal\"\nmean = 55.0\nstd = 1.0",
            "dist = \"lognormal\"\nmedian = 1.0e300\nln_std = 400.0",
        );
        assert_ne!(tail, LINK);
        let e = prepare(&tail).err().expect("a tail that reaches infinity");
        assert!(
            e.contains("input `eirp`") && e.contains("not a finite number"),
            "{e}"
        );
    }

    #[test]
    fn a_study_whose_every_run_fails_reports_null_with_a_reason() {
        // The slant range is drawn below zero in every realisation and the engine refuses
        // each one. Every method must still give a result, never a crash.
        let failing = LINK.replace(
            "key = \"eirp_dbw\"\ndist = \"normal\"\nmean = 55.0\nstd = 1.0",
            "key = \"range_km\"\ndist = \"uniform\"\nmin = -300.0\nmax = -100.0",
        );
        assert_ne!(failing, LINK);
        for method in ["none", "src", "sobol"] {
            let run = run_study(
                &with(&format!("[sensitivity]\nmethod = \"{method}\"\n")).replace(LINK, &failing),
                &RunOptions::default(),
            )
            .unwrap_or_else(|e| panic!("{method}: the study itself runs: {e}"));
            let d = run.result.expect("complete").doc;
            assert_eq!(d["runs"]["failed"], d["runs"]["total"], "{method}");
            let m = &d["outputs"]["margin"];
            assert_eq!(m["n"], 0, "{method}");
            assert!(
                m["mean"].is_null() && m["reason"].is_string(),
                "{method}: {m}"
            );
            assert_eq!(m["limit"]["probability"], 0.0, "{method}");
            if method != "none" {
                let s = &d["sensitivity"]["outputs"]["margin"];
                assert!(s["reason"].is_string(), "{method}: {s}");
                let entries = s["inputs"].as_array().expect("one entry per input");
                assert_eq!(entries.len(), 2, "{method}");
                for e in entries {
                    assert!(
                        e["rank"].is_null() && e["reason"].is_string(),
                        "{method}: {e}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_study_too_large_to_hold_in_memory_is_refused() {
        let plan = |text: &str| {
            let spec: StudySpec = toml::from_str(text).expect("parses");
            Plan::from_spec(&spec)
        };
        let sobol = |n: &str| with("[sensitivity]\nmethod = \"sobol\"\n").replace("n = 64", n);
        // Two inputs: a Sobol' design makes n * 4 runs.
        assert!(plan(&sobol("n = 2500000")).is_ok());
        let e = plan(&sobol("n = 2500001")).expect_err("10,000,004 runs");
        assert!(
            e.contains("10000004") && e.contains("at most 10000000"),
            "{e}"
        );
        let e = plan(&sobol("n = 9223372036854775807")).expect_err("overflows");
        assert!(e.contains("more than 2^64"), "{e}");
        let e = plan(&LINK.replace("n = 64", "n = 4000000000000")).expect_err("too many");
        assert!(e.contains("4000000000000"), "{e}");
    }

    #[test]
    fn sobol_seeds_keep_the_seed_with_matrix_a() {
        let spec: StudySpec =
            toml::from_str(&with("[sensitivity]\nmethod = \"sobol\"\n")).expect("parses");
        let plan = Plan::from_spec(&spec).expect("valid");
        let jobs = plan.jobs(Some(100)).expect("a design");
        let n = 64;
        assert_eq!(jobs.len(), n * 4);
        for j in 0..n {
            assert_eq!(jobs[j].seed, Some(100 + j as u64)); // A
            assert_eq!(jobs[n + j].seed, Some(100 + (n + j) as u64)); // B
            assert_eq!(jobs[2 * n + j].seed, Some(100 + j as u64)); // A_B^(0)
            assert_eq!(jobs[3 * n + j].seed, Some(100 + j as u64)); // A_B^(1)
        }
        assert!(plan.jobs(Some(u64::MAX - 3)).is_err());
    }
}
