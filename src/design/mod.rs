// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Design optimiser: the Pareto front of a design space
//!
//! Open Kshana scores one scenario. This module answers "which design should we fly":
//! a study states a design space (variables written into a base scenario of any open
//! kind), constraints and two or three objectives, and gets back the Pareto front, its
//! knee, and where a reference design sits against it.
//!
//! * Every evaluation is one run of the open engine through
//!   [`kshana::api::run_scenario`]; an objective is a number the engine wrote or the
//!   value of a design variable. There is no cost model and nothing is invented.
//! * The front and the knee come from the open kernel [`kshana::mcda::pareto`].
//! * The search is exhaustive when the feasible space fits the budget and a seeded
//!   NSGA-II otherwise; a front that is not exhaustive is labelled "best found in N
//!   runs" and is never called the best possible.
//! * The same study and seed give the same bytes for any thread count.
//! * Every design on the front is exported as a plain open scenario.
//!
//! See `docs/DESIGN-OPTIMISER.md`.

pub mod cli;
pub mod evaluate;
pub mod front;
pub mod render;
pub mod search;
pub mod space;

use std::collections::BTreeMap;
use std::sync::OnceLock;

use kshana::api::KshanaError;
use kshana::mcda::Objective;
use kshana::verification::{verification_matrix, VerificationStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use evaluate::{Member, RunFacts, Status};
use front::{Bounds, Fitness};
use space::{
    canonical_study, invalid, parse_path, sha256_hex, ConstraintSpec, Op, ResultConstraint, Seg,
    Space, VariableSpec,
};

/// The scenario kind of a design study.
pub const KIND: &str = "pro-design-optimise";
/// Version of the result document's layout.
pub const SCHEMA_VERSION: &str = "1.0";

fn default_seed() -> u64 {
    1
}
fn default_method() -> String {
    "auto".into()
}
fn default_budget() -> usize {
    2000
}
fn default_population() -> usize {
    40
}
fn default_generations() -> usize {
    50
}

/// One `[[objective]]`: a name, a sense, and where the number comes from.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveSpec {
    pub name: String,
    /// `min` or `max`.
    pub sense: String,
    /// A path into the run's result document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The name of a design variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    /// Optional unit, used when the engine's result does not state one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// The `[search]` table.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchSpec {
    /// `exhaustive`, `nsga2` or `auto`.
    #[serde(default = "default_method")]
    pub method: String,
    /// The most engine runs the search may make.
    #[serde(default = "default_budget")]
    pub budget: usize,
    #[serde(default = "default_population")]
    pub population: usize,
    #[serde(default = "default_generations")]
    pub generations: usize,
    /// Worker threads. It changes the run time, never the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads: Option<usize>,
}

impl Default for SearchSpec {
    fn default() -> Self {
        SearchSpec {
            method: default_method(),
            budget: default_budget(),
            population: default_population(),
            generations: default_generations(),
            threads: None,
        }
    }
}

/// The `[reference]` table: a design to place against the front.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSpec {
    pub label: String,
    /// The reference as one value per variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<toml::value::Table>,
    /// The reference as a complete open scenario (a design from outside the space).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario: Option<String>,
}

/// A design study, as written in a study file.
///
/// A key the study file format does not have is an error, at every level. A misspelt
/// table (`[[constraints]]` for `[[constraint]]`) would otherwise be ignored and the study
/// would return a front as if the constraint held.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    /// The `kind` line of the file. It may be left out; when present it must be
    /// [`KIND`]. It is not part of the study hash, which states the kind itself.
    #[serde(default, skip_serializing)]
    pub kind: Option<String>,
    pub title: String,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// A complete open scenario, of any open kind, as a TOML string.
    pub base_scenario: String,
    #[serde(default, rename = "variable")]
    pub variables: Vec<VariableSpec>,
    #[serde(default, rename = "constraint")]
    pub constraints: Vec<ConstraintSpec>,
    #[serde(default, rename = "objective")]
    pub objectives: Vec<ObjectiveSpec>,
    #[serde(default)]
    pub search: SearchSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<ReferenceSpec>,
}

/// Where an objective's number comes from.
#[derive(Debug, Clone)]
pub enum Source {
    /// A path into the run's result document.
    Result { path: Vec<Seg>, path_text: String },
    /// A design variable, by position.
    Variable(usize),
}

/// An objective checked against the study.
#[derive(Debug, Clone)]
pub struct ObjectiveDef {
    pub name: String,
    pub sense: Objective,
    pub source: Source,
    pub unit: Option<String>,
}

/// A study checked against its base scenario and ready to run.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub study: Study,
    pub base: toml::Value,
    /// The `kind` of the base scenario.
    pub base_kind: String,
    pub space: Space,
    pub objectives: Vec<ObjectiveDef>,
    pub result_constraints: Vec<ResultConstraint>,
    /// SHA-256 of the canonical form of the study.
    pub scenario_hash: String,
}

impl Study {
    /// Parse a study file.
    pub fn parse(src: &str) -> Result<Study, KshanaError> {
        toml::from_str::<Study>(src).map_err(|e| invalid(format!("invalid {KIND} study: {e}")))
    }

    /// Check the study against its base scenario.
    pub fn resolve(&self) -> Result<Resolved, KshanaError> {
        if let Some(kind) = &self.kind {
            if kind != KIND {
                return Err(invalid(format!(
                    "the file is a `{kind}` scenario, not a design study (kind = \"{KIND}\")"
                )));
            }
        }
        let base: toml::Value = toml::from_str(&self.base_scenario)
            .map_err(|e| invalid(format!("base_scenario is not valid TOML: {e}")))?;
        let base_kind = base
            .get("kind")
            .and_then(|k| k.as_str())
            .ok_or_else(|| invalid("base_scenario has no `kind`"))?
            .to_string();
        if base_kind.starts_with(crate::registry::PRO_KIND_PREFIX) {
            return Err(invalid(format!(
                "base_scenario is a `{base_kind}` scenario; a design study runs an open kind"
            )));
        }
        let space = Space::resolve(self, &base)?;

        if self.objectives.len() < 2 || self.objectives.len() > 3 {
            return Err(invalid(format!(
                "a study needs two or three [[objective]] entries (this one has {})",
                self.objectives.len()
            )));
        }
        let mut objectives = Vec::new();
        for o in &self.objectives {
            if o.name.trim().is_empty() {
                return Err(invalid("an [[objective]] has an empty name"));
            }
            if objectives.iter().any(|d: &ObjectiveDef| d.name == o.name) {
                return Err(invalid(format!(
                    "objective '{}' is defined more than once",
                    o.name
                )));
            }
            let sense = match o.sense.as_str() {
                "min" => Objective::Min,
                "max" => Objective::Max,
                other => {
                    return Err(invalid(format!(
                        "objective '{}': sense '{other}' is not min or max",
                        o.name
                    )))
                }
            };
            let (source, unit) = match (&o.path, &o.variable) {
                (Some(p), None) => (
                    Source::Result {
                        path: parse_path(p)
                            .map_err(|e| invalid(format!("objective '{}': path {e}", o.name)))?,
                        path_text: p.clone(),
                    },
                    o.unit.clone(),
                ),
                (None, Some(name)) => {
                    let i = space
                        .variables
                        .iter()
                        .position(|v| &v.name == name)
                        .ok_or_else(|| {
                            invalid(format!(
                                "objective '{}': '{name}' is not a variable of this study",
                                o.name
                            ))
                        })?;
                    if space.variables[i].numeric.is_none() {
                        return Err(invalid(format!(
                            "objective '{}': variable '{name}' is not a number",
                            o.name
                        )));
                    }
                    (
                        Source::Variable(i),
                        o.unit.clone().or_else(|| space.variables[i].unit.clone()),
                    )
                }
                _ => {
                    return Err(invalid(format!(
                        "objective '{}': give exactly one of path (a result path) or variable (a design variable)",
                        o.name
                    )))
                }
            };
            objectives.push(ObjectiveDef {
                name: o.name.clone(),
                sense,
                source,
                unit,
            });
        }

        let mut result_constraints = Vec::new();
        for c in &self.constraints {
            if let Some(rc) = &c.result {
                if !rc.value.is_finite() {
                    return Err(invalid(format!(
                        "constraint on '{}': value is not a finite number ({})",
                        rc.path, rc.value
                    )));
                }
                result_constraints.push(ResultConstraint {
                    path_text: rc.path.clone(),
                    path: parse_path(&rc.path)
                        .map_err(|e| invalid(format!("constraint: path {e}")))?,
                    op: Op::parse(&rc.op).ok_or_else(|| {
                        invalid(format!(
                            "constraint on '{}': op '{}' is not one of <=, <, >=, >, ==, !=",
                            rc.path, rc.op
                        ))
                    })?,
                    value: rc.value,
                });
            }
        }

        match self.search.method.as_str() {
            "auto" | "exhaustive" | "nsga2" => {}
            other => {
                return Err(invalid(format!(
                    "search: method '{other}' is not exhaustive, nsga2 or auto"
                )))
            }
        }
        if self.search.budget == 0 {
            return Err(invalid("search: budget must be at least 1"));
        }
        if self.search.population < 2 {
            return Err(invalid("search: population must be at least 2"));
        }
        if let Some(reference) = &self.reference {
            match (&reference.values, &reference.scenario) {
                (Some(_), None) | (None, Some(_)) => {}
                _ => {
                    return Err(invalid(
                        "reference: give exactly one of values = {...} or scenario = \"...\"",
                    ))
                }
            }
        }
        let scenario_hash = sha256_hex(canonical_study(self, &base)?.as_bytes());
        Ok(Resolved {
            study: self.clone(),
            base,
            base_kind,
            space,
            objectives,
            result_constraints,
            scenario_hash,
        })
    }
}

impl Resolved {
    pub fn senses(&self) -> Vec<Objective> {
        self.objectives.iter().map(|o| o.sense).collect()
    }
}

/// Run-time options that do not change the result.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Worker threads; overrides `[search] threads`.
    pub threads: Option<usize>,
}

/// Everything a study produces.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// `design.result.json`.
    pub json: String,
    /// `design.pareto.svg`.
    pub svg: String,
    /// `design.report.html`.
    pub html: String,
    /// `front/index.csv`.
    pub index_csv: String,
    /// `front/NNN.toml`: file name and scenario text, in front order.
    pub front_files: Vec<(String, String)>,
    /// One line for the terminal.
    pub summary: String,
}

/// The version the embedded open engine reports.
pub fn engine_version() -> String {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION
        .get_or_init(|| {
            // The smallest open run that states its engine version.
            let probe = "kind = \"clock\"\nseed = 1\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 20.0\n[gnss]\nwindows = [ { t0 = 0.0, t1 = 20.0, state = \"nominal\" } ]\n[clock_quantum]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n[clock_classical]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n";
            kshana::api::run_scenario(probe)
                .ok()
                .and_then(|o| serde_json::from_str::<Value>(&o.json).ok())
                .and_then(|v| {
                    v.get("engine_version")
                        .and_then(|e| e.as_str())
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "(unreported)".to_string())
        })
        .clone()
}

fn tier_rank(tag: &str) -> u8 {
    match tag {
        "VALIDATED" => 0,
        "MODELLED" => 1,
        _ => 2,
    }
}

fn weaker<'a>(a: &'a str, b: &'a str) -> &'a str {
    if tier_rank(b) > tier_rank(a) {
        b
    } else {
        a
    }
}

/// The weakest verification-matrix row that names the kind, as (requirement, label).
/// `None` when no row names it, in which case no tier is claimed.
fn weakest_row(kind: &str) -> Option<(String, &'static str)> {
    let needle = format!("`{kind}`");
    let mut best: Option<(String, &'static str)> = None;
    for item in verification_matrix() {
        if !(item.capability.contains(&needle) || item.requirement.contains(&needle)) {
            continue;
        }
        let tag = match item.status {
            VerificationStatus::Validated => "VALIDATED",
            VerificationStatus::Modelled => "MODELLED",
            VerificationStatus::PartnerOwned => "PARTNER",
        };
        if best
            .as_ref()
            .is_none_or(|(_, t)| tier_rank(tag) > tier_rank(t))
        {
            best = Some((item.requirement.to_string(), tag));
        }
    }
    best
}

const KERNEL_BASIS: &str = "the value is the open engine's; its place on the front is decided by the open Pareto kernel (kshana::mcda::pareto), which the engine documents as a modelled closed form, so the figure is never above MODELLED";

/// The `figure_tiers` block: for each headline figure, the label of the weakest open
/// capability it rests on.
fn figure_tiers(r: &Resolved, facts: Option<&RunFacts>, has_reference: bool) -> Value {
    let mut figures = Vec::new();
    let mut untiered = Vec::new();
    let run_label = facts.and_then(|f| f.label.as_deref()).unwrap_or("");
    for (k, o) in r.objectives.iter().enumerate() {
        let groups: Vec<&str> = if has_reference {
            vec!["knee", "front[]", "evaluated[]", "reference"]
        } else {
            vec!["knee", "front[]", "evaluated[]"]
        };
        let base: Option<(String, String)> = match &o.source {
            Source::Variable(_) => None,
            Source::Result { .. } => facts
                .and_then(|f| f.objective_tiers.get(k).cloned().flatten())
                .map(|(tier, requirement)| (requirement, tier))
                .or_else(|| {
                    weakest_row(&r.base_kind).map(|(requirement, tag)| {
                        let capped = if run_label.starts_with("PARTNER") {
                            weaker(tag, "PARTNER")
                        } else if run_label.starts_with("MODELLED") {
                            weaker(tag, "MODELLED")
                        } else {
                            tag
                        };
                        (requirement, capped.to_string())
                    })
                }),
        };
        for g in groups {
            let path = format!("{g}.objectives.{}", o.name);
            match (&o.source, &base) {
                (Source::Variable(_), _) => untiered.push(json!({
                    "path": path,
                    "reason": "a design variable: an input of the study, not a computed figure",
                })),
                (_, None) => untiered.push(json!({
                    "path": path,
                    "reason": format!(
                        "no verification-matrix row names the `{}` kind, so no tier is claimed",
                        r.base_kind
                    ),
                })),
                (_, Some((requirement, tier))) => {
                    let selected = g == "knee" || g == "front[]";
                    let tier = if selected {
                        weaker(tier, "MODELLED")
                    } else {
                        tier.as_str()
                    };
                    let mut entry = json!({
                        "path": path,
                        "requirement": requirement,
                        "tier": tier,
                    });
                    if selected {
                        entry["basis"] = json!(KERNEL_BASIS);
                    }
                    figures.push(entry);
                }
            }
        }
    }
    untiered.push(json!({
        "path": "hypervolume.value",
        "reason": "computed by the overlay from the front; no verification-matrix row owns it",
    }));
    json!({
        "source": "the open engine's verification matrix (kshana::verification::verification_matrix) and each run's own figure_tiers block",
        "legend": "VALIDATED: checked against an independent external oracle. MODELLED: first-principles or published-formula model, internally tested. PARTNER: a partner's discipline. A figure takes the label of the weakest open capability it rests on and is never raised",
        "figures": figures,
        "untiered": untiered,
    })
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// A value as a table cell: a whole number without a decimal point, any other number
/// in its shortest exact form.
fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => match n.as_f64() {
            Some(x) if x.fract() == 0.0 && x.abs() < 1e15 => format!("{x:.0}"),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn objectives_json(r: &Resolved, values: &[f64]) -> Value {
    let mut m = serde_json::Map::new();
    for (o, x) in r.objectives.iter().zip(values) {
        m.insert(o.name.clone(), json!(x));
    }
    Value::Object(m)
}

fn constraints_json(r: &Resolved, m: &Member) -> Value {
    Value::Array(
        r.result_constraints
            .iter()
            .zip(&m.constraints)
            .map(|(c, check)| {
                json!({
                    "path": c.path_text,
                    "op": c.op.as_str(),
                    "limit": c.value,
                    "value": check.value,
                    "ok": check.ok,
                })
            })
            .collect(),
    )
}

/// Run a study given as the text of a study file.
pub fn run_study(src: &str, opts: &Options) -> Result<Outcome, KshanaError> {
    let study = Study::parse(src)?;
    let resolved = study.resolve()?;
    run_resolved(&resolved, opts)
}

/// Run a checked study.
pub fn run_resolved(r: &Resolved, opts: &Options) -> Result<Outcome, KshanaError> {
    let threads = opts
        .threads
        .or(r.study.search.threads)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        })
        .max(1);
    let senses = r.senses();
    let budget = r.study.search.budget;

    // ── The space ────────────────────────────────────────────────────────────
    let all_feasible = r.space.enumerate();
    let feasible_count = all_feasible.as_ref().map(Vec::len);
    if feasible_count == Some(0) {
        return Err(invalid(
            "the design space has no feasible design: every combination breaks a design constraint",
        ));
    }
    let method = match (r.study.search.method.as_str(), feasible_count) {
        ("exhaustive", None) => {
            return Err(invalid(format!(
                "search: the grid has {} designs, too many to enumerate (the limit is {}); use method = \"nsga2\" or coarser steps",
                r.space.grid_size,
                space::ENUMERATION_CAP
            )))
        }
        ("exhaustive", Some(n)) if n > budget => {
            return Err(invalid(format!(
                "search: exhaustive needs {n} runs but the budget is {budget}; raise the budget or use method = \"nsga2\""
            )))
        }
        ("exhaustive", Some(_)) => "exhaustive",
        ("nsga2", _) => "nsga2",
        (_, Some(n)) if n <= budget => "exhaustive",
        _ => "nsga2",
    };

    // ── The search ───────────────────────────────────────────────────────────
    let mut members: Vec<Member> = Vec::new();
    let mut study_error: Option<KshanaError> = None;
    let mut score_batch = |batch: &[Vec<usize>]| -> Vec<Fitness> {
        if study_error.is_some() {
            return vec![Fitness::Failed; batch.len()];
        }
        match evaluate::evaluate_batch(r, batch, threads) {
            Ok(ms) => {
                let fit = ms
                    .iter()
                    .map(|m| match (&m.objectives, m.status) {
                        (Some(o), Status::Ok | Status::ConstraintViolated) => Fitness::Scored {
                            objectives: o.clone(),
                            violations: m.violations(),
                        },
                        _ => Fitness::Failed,
                    })
                    .collect();
                members.extend(ms);
                fit
            }
            Err(e) => {
                study_error = Some(e);
                vec![Fitness::Failed; batch.len()]
            }
        }
    };
    let feasible_fn = |d: &[usize]| r.space.feasible(d);
    let log = if method == "exhaustive" {
        search::exhaustive(all_feasible.clone().unwrap_or_default(), &mut score_batch)
    } else {
        let problem = search::Problem {
            levels: r.space.level_counts(),
            ordered: r.space.variables.iter().map(|v| v.ordered).collect(),
            senses: senses.clone(),
            feasible: &feasible_fn,
            all_feasible: all_feasible.clone(),
        };
        let cfg = search::Config {
            population: r.study.search.population,
            generations: r.study.search.generations,
            budget,
            seed: r.study.seed,
        };
        search::nsga2(&problem, &cfg, &mut score_batch)
    };
    if let Some(e) = study_error {
        return Err(e);
    }
    debug_assert_eq!(members.len(), log.designs.len());
    let evaluations = members.len();
    let exhaustive = feasible_count == Some(evaluations);
    let infeasible = match (method, feasible_count) {
        ("exhaustive", Some(n)) => (r.space.grid_size - n as u128) as usize,
        _ => log.infeasible,
    };

    // ── The front, from the open kernel ──────────────────────────────────────
    let scored: Vec<usize> = (0..members.len())
        .filter(|&i| members[i].scored())
        .collect();
    if scored.is_empty() {
        let first = members
            .iter()
            .find_map(|m| m.error.clone())
            .unwrap_or_else(|| "every run broke a result constraint".into());
        return Err(invalid(format!(
            "no design could be scored, so there is no front: {evaluations} runs, {} failed, {} gave no value for an objective, {} broke a result constraint; first failure: {first}",
            members.iter().filter(|m| m.status == Status::Failed).count(),
            members.iter().filter(|m| m.status == Status::NoValue).count(),
            members
                .iter()
                .filter(|m| m.status == Status::ConstraintViolated)
                .count(),
        )));
    }
    let points: Vec<Vec<f64>> = scored
        .iter()
        .map(|&i| members[i].objectives.clone().unwrap_or_default())
        .collect();
    let mut front_ids: Vec<usize> = front::kernel_front(&points, &senses)
        .into_iter()
        .map(|p| scored[p])
        .collect();
    let objectives_of = |i: usize| -> &Vec<f64> {
        members[i]
            .objectives
            .as_ref()
            .expect("a scored member has objectives")
    };
    front_ids.sort_by(|&a, &b| {
        let (oa, ob) = (objectives_of(a), objectives_of(b));
        oa.iter()
            .zip(ob)
            .map(|(x, y)| x.total_cmp(y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let knee = front::kernel_knee(&points, &senses);
    let front_points: Vec<Vec<f64>> = front_ids
        .iter()
        .map(|&i| objectives_of(i).clone())
        .collect();
    let bounds = Bounds::of(&points, &senses).expect("at least one scored design");
    let hypervolume = front::hypervolume(&front_points, &senses, &bounds);
    let mut distinct: Vec<&Vec<f64>> = Vec::new();
    for p in &front_points {
        if !distinct.contains(&p) {
            distinct.push(p);
        }
    }
    let width = front_ids.len().to_string().len().max(3);
    let front_name = |pos: usize| format!("{:0width$}", pos + 1, width = width);
    let position_of = |member: usize| front_ids.iter().position(|&i| i == member);

    // ── Convergence of the search (NSGA-II only) ─────────────────────────────
    let mut convergence = Vec::new();
    if method == "nsga2" {
        let mut archive: Vec<Vec<f64>> = Vec::new();
        let mut done = 0usize;
        for g in 0..=log.generations_run {
            let mut added = false;
            for (i, m) in members.iter().enumerate() {
                if log.generation[i] == g {
                    done += 1;
                    if m.scored() {
                        archive.push(objectives_of(i).clone());
                        added = true;
                    }
                }
            }
            if added {
                archive = front::own_front(&archive, &senses)
                    .into_iter()
                    .map(|i| archive[i].clone())
                    .collect();
            }
            convergence.push(json!({
                "generation": g,
                "evaluations": done,
                "front_size": archive.len(),
                "hypervolume": front::hypervolume(&archive, &senses, &bounds),
            }));
        }
    }

    // ── The reference design ─────────────────────────────────────────────────
    let mut cache_hits = log.cache_hits;
    let mut reference_runs = 0usize;
    let mut reference_member: Option<Member> = None;
    let mut reference_json = Value::Null;
    if let Some(spec) = &r.study.reference {
        let (member, variables, in_space) = if let Some(values) = &spec.values {
            let design = r.space.design_from_values(values)?;
            if !r.space.feasible(&design) {
                return Err(invalid(
                    "reference: the values break a design constraint of this study; give the reference as a full scenario to place a design from outside the space",
                ));
            }
            let member = match members
                .iter()
                .find(|m| m.design.as_deref() == Some(&design[..]))
            {
                Some(m) => {
                    cache_hits += 1;
                    m.clone()
                }
                None => {
                    reference_runs += 1;
                    evaluate::run_member(r, &design)?
                }
            };
            (member, r.space.values_json(&design), true)
        } else {
            reference_runs += 1;
            let text = spec.scenario.as_deref().unwrap_or("");
            (evaluate::run_scenario_member(r, text)?, Value::Null, false)
        };
        let mut j = json!({
            "label": spec.label,
            "given_as": if in_space { "variable values" } else { "a full scenario" },
            "in_design_space": in_space,
            "status": member.status.as_str(),
            "scenario_sha256": member.scenario_sha256,
            "result_sha256": member.result_sha256,
        });
        if in_space {
            j["variables"] = variables;
        }
        if let Some(e) = &member.error {
            j["error"] = json!(e);
        }
        if let Some(o) = &member.objectives {
            let placement = front::place(o, &front_points, &senses);
            let names =
                |list: &[usize]| -> Vec<String> { list.iter().map(|&p| front_name(p)).collect() };
            j["objectives"] = objectives_json(r, o);
            j["constraints"] = constraints_json(r, &member);
            j["on_front"] = json!(placement.on_front() && member.status == Status::Ok);
            j["dominated_by"] = json!(names(&placement.dominated_by));
            j["dominates"] = json!(names(&placement.dominates));
            j["equal_to"] = json!(names(&placement.equals));
            j["statement"] = json!(if !placement.dominated_by.is_empty() {
                format!(
                    "{} front design(s) are at least as good on every objective and better on one",
                    placement.dominated_by.len()
                )
            } else if member.status != Status::Ok {
                "no front design dominates it, but it breaks a result constraint of this study"
                    .to_string()
            } else if !placement.dominates.is_empty() {
                "no front design dominates it, and it dominates front designs: it lies outside the stated design space".to_string()
            } else {
                "no front design dominates it".to_string()
            });
        }
        reference_json = j;
        reference_member = Some(member);
    }

    // ── What the runs say about themselves ───────────────────────────────────
    let facts: Option<&RunFacts> = members.iter().find_map(|m| m.facts.as_ref());
    let engine_version = facts
        .and_then(|f| f.engine_version.clone())
        .unwrap_or_else(engine_version);
    let base_label = facts.and_then(|f| f.label.clone());
    // A unit the engine states is never replaced by one written in the study file; the
    // study's unit is used only where the engine states none.
    let objective_engine_unit = |k: usize| -> Option<String> {
        match &r.objectives[k].source {
            Source::Result { .. } => facts
                .and_then(|f| f.objective_units.get(k).cloned().flatten())
                .map(|(u, _)| u),
            Source::Variable(i) => engine_variable_unit(facts, *i),
        }
    };
    let objective_unit = |k: usize| -> String {
        let o = &r.objectives[k];
        // For an objective that is a variable, `o.unit` already falls back to the
        // variable's own `unit`.
        objective_engine_unit(k)
            .or_else(|| o.unit.clone())
            .unwrap_or_else(|| UNSTATED.to_string())
    };

    // ── The result document ──────────────────────────────────────────────────
    let failed = members
        .iter()
        .filter(|m| m.status == Status::Failed)
        .count();
    let no_value = members
        .iter()
        .filter(|m| m.status == Status::NoValue)
        .count();
    let violated = members
        .iter()
        .filter(|m| m.status == Status::ConstraintViolated)
        .count();
    let run_digest = {
        let mut all = String::new();
        for m in &members {
            all.push_str(&m.digest_token());
            all.push('\n');
        }
        sha256_hex(all.as_bytes())
    };
    let algorithm = if method == "exhaustive" {
        "exhaustive enumeration"
    } else {
        "NSGA-II"
    };
    let front_statement = if exhaustive {
        format!(
            "the front of the stated design space: every one of its {evaluations} feasible designs was run"
        )
    } else {
        format!(
            "best found in {evaluations} runs: the search did not run every feasible design, so a better one may exist"
        )
    };
    let label = format!(
        "MODELLED - {}. Each design is scored by one run of the open `{}` kind and the front is only as good as that kind's model{}",
        if exhaustive {
            format!(
                "Pareto front of a stated design space, every one of its {evaluations} feasible designs run ({algorithm})"
            )
        } else {
            format!(
                "Pareto front best found in {evaluations} runs ({algorithm}, seed {}); the search did not run every feasible design, so a better one may exist",
                r.study.seed
            )
        },
        r.base_kind,
        match &base_label {
            Some(l) => format!(". The runs carry the engine's own label: {l}"),
            None => String::new(),
        }
    );

    let evaluated: Vec<Value> = members
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let design = m.design.as_ref().expect("a searched member has a design");
            let mut j = json!({
                "id": i,
                "generation": log.generation[i],
                "variables": r.space.values_json(design),
                "status": m.status.as_str(),
                "objectives": m.objectives.as_ref().map(|o| objectives_json(r, o)),
                "scenario_sha256": m.scenario_sha256,
                "result_sha256": m.result_sha256,
                "on_front": position_of(i).is_some(),
            });
            if !r.result_constraints.is_empty() && m.objectives.is_some() {
                j["constraints"] = constraints_json(r, m);
            }
            if let Some(e) = &m.error {
                j["error"] = json!(e);
            }
            j
        })
        .collect();

    let knee_member = knee.as_ref().map(|k| scored[k.index]);
    // A front of one or two distinct points, or one that is a straight line, has no
    // knee: the kernel then returns an end point at distance 0.
    let knee_exists = distinct.len() > 2 && knee.as_ref().is_some_and(|k| k.distance > 1e-9);
    let mut front_files = Vec::new();
    let mut front_json = Vec::new();
    for (pos, &i) in front_ids.iter().enumerate() {
        let m = &members[i];
        let design = m.design.as_ref().expect("a searched member has a design");
        let text = evaluate::scenario_text(r, design)?;
        let file = format!("front/{}.toml", front_name(pos));
        front_json.push(json!({
            "id": front_name(pos),
            "evaluated_id": i,
            "file": file,
            "knee": knee_exists && knee_member == Some(i),
            "variables": r.space.values_json(design),
            "objectives": objectives_json(r, objectives_of(i)),
            "scenario_sha256": m.scenario_sha256,
            "result_sha256": m.result_sha256,
            "scenario": text,
        }));
        front_files.push((format!("{}.toml", front_name(pos)), text));
    }
    let knee_json = match (&knee, knee_member) {
        (Some(k), Some(i)) => {
            let design = members[i]
                .design
                .as_ref()
                .expect("a searched member has a design");
            json!({
                "front_id": position_of(i).map(&front_name),
                "evaluated_id": i,
                "variables": r.space.values_json(design),
                "objectives": objectives_json(r, objectives_of(i)),
                "distance": k.distance,
                "exists": knee_exists,
                "anchors": [
                    position_of(scored[k.anchors.0]).map(&front_name),
                    position_of(scored[k.anchors.1]).map(&front_name),
                ],
                "method": "the front design farthest from the chord between the two most widely separated front designs, in objective space scaled to the front's own range (Branke et al., 2004), computed by kshana::mcda::pareto::knee_point; when the front has fewer than three distinct points or is a straight line there is no knee: `exists` is false and the design named here is only an end of the front",
            })
        }
        _ => Value::Null,
    };

    let variables_json: Vec<Value> = r
        .space
        .variables
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let mut j = json!({
                "name": v.name,
                "key": v.key,
                "type": v.ty,
                "level_count": v.levels.len(),
                "unit": variable_unit_of(r, facts, i).unwrap_or_else(|| UNSTATED.to_string()),
            });
            if v.levels.len() <= 64 {
                j["values"] = Value::Array(v.levels.iter().map(space::toml_to_json).collect());
            } else {
                j["first"] = space::toml_to_json(&v.levels[0]);
                j["last"] = space::toml_to_json(&v.levels[v.levels.len() - 1]);
            }
            j
        })
        .collect();
    let objectives_meta: Vec<Value> = r
        .objectives
        .iter()
        .enumerate()
        .map(|(k, o)| {
            let mut j = json!({
                "name": o.name,
                "sense": if o.sense == Objective::Min { "min" } else { "max" },
                "unit": objective_unit(k),
            });
            match &o.source {
                Source::Result { path_text, .. } => {
                    j["source"] = json!("result");
                    j["path"] = json!(path_text);
                }
                Source::Variable(i) => {
                    j["source"] = json!("variable");
                    j["variable"] = json!(r.space.variables[*i].name);
                }
            }
            j
        })
        .collect();
    let constraints_meta: Vec<Value> = r
        .study
        .constraints
        .iter()
        .map(|c| match (&c.walker, &c.result) {
            (Some(w), _) => json!({
                "type": "walker",
                "total": w.total,
                "planes": w.planes,
                "phasing": w.phasing,
                "rule": "planes divides total, and phasing is in 0..planes; a design that breaks it is never run",
            }),
            (_, Some(c)) => json!({
                "type": "result",
                "path": c.path,
                "op": c.op,
                "value": c.value,
                "rule": "a run that breaks it stays in evaluated[] and is kept off the front",
            }),
            _ => Value::Null,
        })
        .collect();

    // Units: one entry for every numeric field of this document.
    let mut units: BTreeMap<String, Value> = BTreeMap::new();
    let mut unit = |path: &str, unit: &str, provenance: &str, note: &str| {
        units.insert(
            path.to_string(),
            json!({"unit": unit, "provenance": provenance, "note": note}),
        );
    };
    unit("seed", "1", "input", "seed of the search's random numbers");
    unit(
        "space.grid_size",
        "count",
        "computed",
        "combinations of variable levels, feasible or not",
    );
    unit("space.feasible", "count", "computed", "combinations that satisfy every design constraint; null when the grid is too large to enumerate");
    unit(
        "space.infeasible",
        "count",
        "computed",
        "combinations a design constraint removes; null when the grid is too large to enumerate",
    );
    unit(
        "space.variables[].level_count",
        "count",
        "computed",
        "levels of the variable",
    );
    unit(
        "space.variables[].values[]",
        "mixed - see note",
        "input",
        "the levels of each variable, in that variable's own unit (space.variables[].unit)",
    );
    unit(
        "space.variables[].first",
        "mixed - see note",
        "input",
        "first level of a variable with more than 64 levels, in that variable's own unit",
    );
    unit(
        "space.variables[].last",
        "mixed - see note",
        "input",
        "last level of a variable with more than 64 levels, in that variable's own unit",
    );
    unit(
        "search.budget",
        "count",
        "input",
        "the most engine runs the search may make",
    );
    unit(
        "search.population",
        "count",
        "input",
        "NSGA-II population size",
    );
    unit(
        "search.generations",
        "count",
        "input",
        "NSGA-II generations allowed",
    );
    unit(
        "search.generations_run",
        "count",
        "computed",
        "NSGA-II generations completed",
    );
    unit(
        "work.evaluations",
        "count",
        "computed",
        "designs run on the engine by the search",
    );
    unit(
        "work.cache_hits",
        "count",
        "computed",
        "candidates that repeated an evaluated design and cost no run",
    );
    unit("work.infeasible", "count", "computed", "designs a design constraint rejected without a run: every such grid point for an exhaustive search, the distinct rejected candidates for NSGA-II");
    unit(
        "work.failed",
        "count",
        "computed",
        "runs the engine rejected; each is kept in evaluated[] with its error",
    );
    unit(
        "work.no_value",
        "count",
        "computed",
        "runs for which the engine wrote null for an objective, so the design cannot be placed",
    );
    unit(
        "work.constraint_violated",
        "count",
        "computed",
        "runs that broke a result constraint",
    );
    unit(
        "work.reference_runs",
        "count",
        "computed",
        "engine runs made for the reference design",
    );
    unit(
        "evaluated[].id",
        "1",
        "computed",
        "position in evaluation order",
    );
    unit("evaluated[].generation", "count", "computed", "NSGA-II generation the design was run in (0 for the first population and for an exhaustive search)");
    unit(
        "evaluated[].constraints[].limit",
        "mixed - see note",
        "input",
        "the constraint's value, in the unit of its result path",
    );
    unit(
        "evaluated[].constraints[].value",
        "mixed - see note",
        "computed",
        "the number read at the constraint's result path",
    );
    unit(
        "reference.constraints[].limit",
        "mixed - see note",
        "input",
        "the constraint's value, in the unit of its result path",
    );
    unit(
        "reference.constraints[].value",
        "mixed - see note",
        "computed",
        "the number read at the constraint's result path",
    );
    unit(
        "constraints[].value",
        "mixed - see note",
        "input",
        "the limit of a result constraint, in the unit of its result path",
    );
    unit(
        "front[].evaluated_id",
        "1",
        "computed",
        "position of the design in evaluated[]",
    );
    unit(
        "front_summary.designs",
        "count",
        "computed",
        "designs on the front",
    );
    unit(
        "front_summary.distinct_objective_points",
        "count",
        "computed",
        "distinct objective vectors among the front designs",
    );
    unit(
        "knee.evaluated_id",
        "1",
        "computed",
        "position of the knee design in evaluated[]",
    );
    unit(
        "knee.distance",
        "1",
        "computed",
        "distance of the knee from the chord, in objective space scaled to the front's range",
    );
    unit(
        "hypervolume.value",
        "1",
        "computed",
        "share of the box between the ideal and the nadir that the front dominates",
    );
    unit(
        "convergence[].generation",
        "count",
        "computed",
        "generation number",
    );
    unit(
        "convergence[].evaluations",
        "count",
        "computed",
        "designs run up to and including this generation",
    );
    unit(
        "convergence[].front_size",
        "count",
        "computed",
        "non-dominated designs found so far",
    );
    unit(
        "convergence[].hypervolume",
        "1",
        "computed",
        "hypervolume of the front found so far, in the final box",
    );
    unit(
        "reproducibility.engine_runs",
        "count",
        "computed",
        "engine runs of the study, reference included",
    );
    for (i, v) in r.space.variables.iter().enumerate() {
        if v.numeric.is_none() {
            continue;
        }
        let u = variable_unit_of(r, facts, i).unwrap_or_else(|| UNSTATED.to_string());
        let mut note = format!(
            "design variable, written at '{}' of the base scenario",
            v.key
        );
        if u == UNSTATED {
            note.push_str(NO_UNIT_NOTE);
        }
        note.push_str(&overridden_note(
            v.unit.as_deref(),
            engine_variable_unit(facts, i).as_deref(),
        ));
        for g in ["evaluated[]", "front[]", "knee", "reference"] {
            unit(&format!("{g}.variables.{}", v.name), &u, "input", &note);
        }
    }
    for (k, o) in r.objectives.iter().enumerate() {
        let u = objective_unit(k);
        let (provenance, mut note) = match &o.source {
            Source::Result { path_text, .. } => (
                "computed",
                match facts
                    .and_then(|f| f.objective_units.get(k).cloned().flatten())
                    .and_then(|(_, n)| n)
                {
                    Some(n) => format!(
                        "read at '{path_text}' of each run's result; the engine's note: {n}"
                    ),
                    None => format!("read at '{path_text}' of each run's result"),
                },
            ),
            Source::Variable(i) => (
                "input",
                format!("the design variable '{}'", r.space.variables[*i].name),
            ),
        };
        if u == UNSTATED {
            note.push_str(NO_UNIT_NOTE);
        }
        note.push_str(&overridden_note(
            o.unit.as_deref(),
            objective_engine_unit(k).as_deref(),
        ));
        for g in ["evaluated[]", "front[]", "knee", "reference"] {
            unit(&format!("{g}.objectives.{}", o.name), &u, provenance, &note);
        }
        unit(
            &format!("hypervolume.ideal.{}", o.name),
            &u,
            "computed",
            "best value among the scored designs",
        );
        unit(
            &format!("hypervolume.nadir.{}", o.name),
            &u,
            "computed",
            "worst value among the scored designs",
        );
    }

    let mut not_modelled = vec![
        "cost, mass, schedule and risk: an objective is an engine output or a design variable, and there is no cost model".to_string(),
        format!(
            "anything the open `{}` kind does not model; the front inherits every limit of that kind",
            r.base_kind
        ),
        "designs outside the stated variables, levels and constraints".to_string(),
        "uncertainty of the objectives: each design is one deterministic run".to_string(),
    ];
    if !exhaustive {
        not_modelled.push(
            "completeness: the search did not run every feasible design, so a better design may exist in the stated space".to_string(),
        );
    }

    let engine_runs = evaluations + reference_runs;
    let doc = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": KIND,
        "overlay": "kshana-pro",
        "overlay_version": crate::version(),
        "engine_version": engine_version,
        "title": r.study.title,
        "seed": r.study.seed,
        "scenario_hash": r.scenario_hash,
        "label": label,
        "exhaustive": exhaustive,
        "front_statement": front_statement,
        "base": {
            "kind": r.base_kind,
            "label": base_label,
        },
        "search": {
            "method_requested": r.study.search.method,
            "method_used": method,
            "algorithm": algorithm,
            "budget": budget,
            "population": r.study.search.population,
            "generations": r.study.search.generations,
            "generations_run": log.generations_run,
        },
        "space": {
            "variables": variables_json,
            "grid_size": r.space.grid_size.min(u64::MAX as u128) as u64,
            "feasible": feasible_count,
            "infeasible": feasible_count.map(|n| (r.space.grid_size - n as u128) as u64),
            "enumerated": feasible_count.is_some(),
        },
        "objectives": objectives_meta,
        "constraints": constraints_meta,
        "work": {
            "evaluations": evaluations,
            "cache_hits": cache_hits,
            "infeasible": infeasible,
            "failed": failed,
            "no_value": no_value,
            "constraint_violated": violated,
            "reference_runs": reference_runs,
        },
        "evaluated": evaluated,
        "front": front_json,
        "front_summary": {
            "designs": front_ids.len(),
            "distinct_objective_points": distinct.len(),
        },
        "knee": knee_json,
        "hypervolume": {
            "value": hypervolume,
            "ideal": objectives_json(r, &bounds.ideal),
            "nadir": objectives_json(r, &bounds.nadir),
            "note": "share of the box between the ideal (best value of each objective among the scored designs) and the nadir (worst value of each) that the front dominates; it compares fronts of one study and means nothing across studies",
        },
        "reference": reference_json,
        "convergence": convergence,
        "units": units,
        "figure_tiers": figure_tiers(r, facts, reference_member.is_some()),
        "not_modelled": not_modelled,
        "reproducibility": {
            "engine_runs": engine_runs,
            "run_digest": run_digest,
            "run_digest_rule": "SHA-256 over one line per evaluated design, in evaluated[] order: the SHA-256 of that run's result document, or 'failed:' and the SHA-256 of its error text",
            "deterministic": "the same study file and seed give this document byte for byte, for any thread count",
            "check": "each front[].scenario is a plain open scenario; run it with the open engine and the result document has the SHA-256 in front[].result_sha256",
            "command": "kshana-pro design <study.toml> --out <dir>",
        },
    });
    let json = serde_json::to_string_pretty(&doc)
        .map_err(|e| invalid(format!("the result cannot be written: {e}")))?;

    // ── front/index.csv ──────────────────────────────────────────────────────
    let mut csv = String::from("id,file,knee");
    for v in &r.space.variables {
        csv.push(',');
        csv.push_str(&csv_field(&v.name));
    }
    for o in &r.objectives {
        csv.push(',');
        // An objective may share its name with a variable; keep the headers distinct.
        if r.space.variables.iter().any(|v| v.name == o.name) {
            csv.push_str(&csv_field(&format!("{} (objective)", o.name)));
        } else {
            csv.push_str(&csv_field(&o.name));
        }
    }
    csv.push_str(",scenario_sha256,result_sha256\n");
    for f in &front_json {
        let mut row = vec![
            value_text(&f["id"]),
            format!("{}.toml", value_text(&f["id"])),
            if f["knee"] == json!(true) {
                "yes"
            } else {
                "no"
            }
            .to_string(),
        ];
        for v in &r.space.variables {
            row.push(value_text(&f["variables"][&v.name]));
        }
        for o in &r.objectives {
            row.push(value_text(&f["objectives"][&o.name]));
        }
        row.push(value_text(&f["scenario_sha256"]));
        row.push(value_text(&f["result_sha256"]));
        csv.push_str(
            &row.iter()
                .map(|s| csv_field(s))
                .collect::<Vec<_>>()
                .join(","),
        );
        csv.push('\n');
    }

    let svg = render::pareto_svg(&doc);
    let html = render::report_html(&doc, &svg);
    let summary = format!(
        "Kshana Pro design optimiser | {} | {evaluations} designs run ({algorithm}{}), front of {} designs ({} distinct points){} | {}",
        r.study.title,
        if exhaustive { ", every feasible design" } else { ", best found" },
        front_ids.len(),
        distinct.len(),
        match knee_member {
            Some(i) if knee_exists => format!(
                ", knee {}",
                r.objectives
                    .iter()
                    .zip(objectives_of(i))
                    .map(|(o, x)| format!("{} {x}", o.name))
                    .collect::<Vec<_>>()
                    .join(" / ")
            ),
            _ => String::new(),
        },
        if exhaustive {
            "MODELLED"
        } else {
            "MODELLED, a better design may exist"
        },
    );
    Ok(Outcome {
        json,
        svg,
        html,
        index_csv: csv,
        front_files,
        summary,
    })
}

/// The unit written when neither the study nor the engine's result states one.
pub const UNSTATED: &str = "unstated - see note";
const NO_UNIT_NOTE: &str = "; neither the engine's result nor the study states a unit for it (set `unit` in the study file)";

/// The unit the engine's own result states for variable `i`, when it states one.
fn engine_variable_unit(facts: Option<&RunFacts>, i: usize) -> Option<String> {
    facts.and_then(|f| f.variable_units.get(i).cloned().flatten())
}

/// The unit of variable `i`: the engine's when it states one, otherwise the study's.
fn variable_unit_of(r: &Resolved, facts: Option<&RunFacts>, i: usize) -> Option<String> {
    engine_variable_unit(facts, i).or_else(|| r.space.variables[i].unit.clone())
}

/// What a units note says when the study file writes a unit the engine contradicts.
fn overridden_note(written: Option<&str>, engine: Option<&str>) -> String {
    match (written, engine) {
        (Some(w), Some(e)) if w != e => format!(
            "; the study file states '{w}', but the engine's result states '{e}', which is used"
        ),
        _ => String::new(),
    }
}
