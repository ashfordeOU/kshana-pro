// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Evaluate a design: write its values into the base scenario, run the scenario on the
//! embedded open engine through [`kshana::api::run_scenario`], and read the objectives
//! and constraint values out of the result document.
//!
//! Nothing is computed here. An objective is a number the open engine wrote, or the
//! value of a design variable. A run the engine rejects is kept as a failed member with
//! its error; it is never dropped.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use kshana::api::KshanaError;

use super::space::{invalid, json_get, sha256_hex, toml_get, toml_set, units_key, Seg};
use super::{Resolved, Source};

/// What happened to one design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The engine ran it, every objective was read and every result constraint holds.
    Ok,
    /// The engine ran it and every objective was read, but a result constraint is
    /// broken. It stays in the evaluation log and is kept off the front.
    ConstraintViolated,
    /// The engine ran it but wrote `null` for an objective (for example a dilution of
    /// precision when no fix ever exists), so the design cannot be placed.
    NoValue,
    /// The engine rejected the scenario.
    Failed,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::ConstraintViolated => "constraint-violated",
            Status::NoValue => "no-value",
            Status::Failed => "failed",
        }
    }
}

/// One result constraint as checked on one run.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintCheck {
    /// The number read at the constraint's path; `None` when the engine wrote `null`.
    pub value: Option<f64>,
    pub ok: bool,
}

/// What a run says about itself, kept for the units block and the labels.
#[derive(Debug, Clone, PartialEq)]
pub struct RunFacts {
    /// The `kind` the result document states.
    pub kind: Option<String>,
    /// The run-level `label` of the result document.
    pub label: Option<String>,
    /// The `engine_version` of the result document, for kinds that state it.
    pub engine_version: Option<String>,
    /// Per objective: the unit and note the result's own `units` block states.
    pub objective_units: Vec<Option<(String, Option<String>)>>,
    /// Per objective: the tier and requirement the result's own `figure_tiers` states.
    pub objective_tiers: Vec<Option<(String, String)>>,
    /// Per variable: a unit the result's `units` block states for an input of that name.
    pub variable_units: Vec<Option<String>>,
}

/// One evaluated design (or the reference design).
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// Level indices; `None` for a reference design given as a full scenario.
    pub design: Option<Vec<usize>>,
    /// SHA-256 of the scenario text that was run.
    pub scenario_sha256: String,
    /// SHA-256 of the result document the engine returned; `None` for a failed run.
    pub result_sha256: Option<String>,
    pub status: Status,
    /// Objective values in study order; `None` for a failed run.
    pub objectives: Option<Vec<f64>>,
    pub constraints: Vec<ConstraintCheck>,
    pub error: Option<String>,
    pub facts: Option<RunFacts>,
}

impl Member {
    /// True when the design can be on the front.
    pub fn scored(&self) -> bool {
        self.status == Status::Ok
    }
    /// The number of broken result constraints.
    pub fn violations(&self) -> u32 {
        self.constraints.iter().filter(|c| !c.ok).count() as u32
    }
    /// The token this member contributes to the run digest.
    pub fn digest_token(&self) -> String {
        match &self.result_sha256 {
            Some(h) => h.clone(),
            None => format!(
                "failed:{}",
                sha256_hex(self.error.as_deref().unwrap_or("").as_bytes())
            ),
        }
    }
}

/// The scenario text of a design: the base scenario with each variable's level written
/// at its key, serialised as TOML. This exact text is what is run, hashed and exported.
pub fn scenario_text(r: &Resolved, design: &[usize]) -> Result<String, KshanaError> {
    let mut doc = r.base.clone();
    for (v, &i) in r.space.variables.iter().zip(design) {
        toml_set(&mut doc, &v.path, v.levels[i].clone())
            .map_err(|e| invalid(format!("variable '{}': {e}", v.name)))?;
    }
    toml::to_string(&doc).map_err(|e| invalid(format!("the scenario cannot be written: {e}")))
}

/// What is found at a result path.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup {
    Number(f64),
    /// The path exists and holds `null`: the engine reports no value for this run.
    Null,
    /// The path is not in the document.
    Missing,
    /// The path holds something that is not a number.
    NotNumeric(String),
}

/// Read a number at `path` of a result document.
pub fn lookup(result: &serde_json::Value, path: &[Seg]) -> Lookup {
    match json_get(result, path) {
        None => Lookup::Missing,
        Some(serde_json::Value::Null) => Lookup::Null,
        Some(v) => match v.as_f64() {
            Some(x) if x.is_finite() => Lookup::Number(x),
            _ => Lookup::NotNumeric(match v {
                serde_json::Value::String(_) => "a string".into(),
                serde_json::Value::Bool(_) => "a boolean".into(),
                serde_json::Value::Array(_) => "an array".into(),
                serde_json::Value::Object(_) => "an object".into(),
                _ => "not a finite number".into(),
            }),
        },
    }
}

/// Run a scenario text on the embedded open engine. A panic inside the engine is
/// reported as an error for this run, so one bad design cannot take the study down.
pub fn run_engine(text: &str) -> Result<String, String> {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        kshana::api::run_scenario(text)
    }));
    match caught {
        Ok(Ok(out)) => Ok(out.json),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("the engine stopped on an internal error for this scenario".into()),
    }
}

fn facts(r: &Resolved, result: &serde_json::Value) -> RunFacts {
    let units = result.get("units").and_then(|u| u.as_object());
    let tiers = result
        .get("figure_tiers")
        .and_then(|t| t.get("figures"))
        .and_then(|f| f.as_array());
    let objective_units = r
        .objectives
        .iter()
        .map(|o| match &o.source {
            Source::Result { path_text, .. } => units
                .and_then(|u| {
                    u.get(path_text.as_str())
                        .or_else(|| u.get(units_key(path_text).as_str()))
                })
                .and_then(|e| {
                    Some((
                        e.get("unit")?.as_str()?.to_string(),
                        e.get("note").and_then(|n| n.as_str()).map(str::to_string),
                    ))
                }),
            Source::Variable(_) => None,
        })
        .collect();
    let objective_tiers = r
        .objectives
        .iter()
        .map(|o| match &o.source {
            Source::Result { path_text, .. } => tiers.and_then(|figs| {
                figs.iter().find_map(|f| {
                    (f.get("path")?.as_str()? == path_text).then_some(())?;
                    Some((
                        f.get("tier")?.as_str()?.to_string(),
                        f.get("requirement")?.as_str()?.to_string(),
                    ))
                })
            }),
            Source::Variable(_) => None,
        })
        .collect();
    // A variable's unit: the engine states units by result path, not by scenario key,
    // so take it only when every `input` entry whose last step is the key's last step
    // agrees on one unit.
    let variable_units = r
        .space
        .variables
        .iter()
        .map(|v| {
            let leaf = match v.path.last() {
                Some(Seg::Key(k)) => k.as_str(),
                _ => return None,
            };
            let mut found: Option<String> = None;
            for (path, e) in units? {
                let is_input = e.get("provenance").and_then(|p| p.as_str()) == Some("input");
                let last = path.rsplit('.').next().unwrap_or(path);
                if is_input && last == leaf {
                    let unit = e.get("unit")?.as_str()?.to_string();
                    match &found {
                        Some(u) if *u != unit => return None,
                        _ => found = Some(unit),
                    }
                }
            }
            found
        })
        .collect();
    RunFacts {
        kind: result
            .get("kind")
            .and_then(|k| k.as_str())
            .map(str::to_string),
        label: result
            .get("label")
            .and_then(|k| k.as_str())
            .map(str::to_string),
        engine_version: result
            .get("engine_version")
            .and_then(|k| k.as_str())
            .map(str::to_string),
        objective_units,
        objective_tiers,
        variable_units,
    }
}

/// Run `text` and score it. `variable_value(i)` gives the numeric value of variable
/// `i` for an objective that is a design variable.
///
/// `Err` is a fault of the study itself (a path that is not in the result, or not a
/// number), which stops the study. A scenario the engine rejects is `Ok` with
/// [`Status::Failed`].
fn score(
    r: &Resolved,
    design: Option<Vec<usize>>,
    text: &str,
    variable_value: &dyn Fn(usize) -> Result<f64, KshanaError>,
) -> Result<Member, KshanaError> {
    let scenario_sha256 = sha256_hex(text.as_bytes());
    let unscored = |status: Status,
                    error: String,
                    result_sha256: Option<String>,
                    facts: Option<RunFacts>| Member {
        design: design.clone(),
        scenario_sha256: scenario_sha256.clone(),
        result_sha256,
        status,
        objectives: None,
        constraints: Vec::new(),
        error: Some(error),
        facts,
    };
    let json = match run_engine(text) {
        Ok(j) => j,
        Err(e) => return Ok(unscored(Status::Failed, e, None, None)),
    };
    let result_sha256 = sha256_hex(json.as_bytes());
    let result: serde_json::Value = serde_json::from_str(&json)
        .map_err(|e| invalid(format!("the engine's result is not JSON: {e}")))?;
    let kind = result
        .get("kind")
        .and_then(|k| k.as_str())
        .unwrap_or(&r.base_kind)
        .to_string();
    let run_facts = facts(r, &result);

    let mut objectives = Vec::with_capacity(r.objectives.len());
    for o in &r.objectives {
        match &o.source {
            Source::Variable(i) => objectives.push(variable_value(*i)?),
            Source::Result { path, path_text } => match lookup(&result, path) {
                Lookup::Number(x) => objectives.push(x),
                Lookup::Null => {
                    return Ok(unscored(
                        Status::NoValue,
                        format!(
                            "objective '{}': '{path_text}' is null in the result (the engine reports no value for this design)",
                            o.name
                        ),
                        Some(result_sha256),
                        Some(run_facts),
                    ))
                }
                Lookup::Missing => {
                    return Err(invalid(format!(
                        "objective '{}': path '{path_text}' is not in the result of a `{kind}` run",
                        o.name
                    )))
                }
                Lookup::NotNumeric(what) => {
                    return Err(invalid(format!(
                        "objective '{}': path '{path_text}' holds {what}, not a number, in the result of a `{kind}` run",
                        o.name
                    )))
                }
            },
        }
    }
    let mut constraints = Vec::with_capacity(r.result_constraints.len());
    for c in &r.result_constraints {
        match lookup(&result, &c.path) {
            Lookup::Number(x) => constraints.push(ConstraintCheck {
                value: Some(x),
                ok: c.op.holds(x, c.value),
            }),
            Lookup::Null => constraints.push(ConstraintCheck {
                value: None,
                ok: false,
            }),
            Lookup::Missing => {
                return Err(invalid(format!(
                    "constraint: path '{}' is not in the result of a `{kind}` run",
                    c.path_text
                )))
            }
            Lookup::NotNumeric(what) => {
                return Err(invalid(format!(
                "constraint: path '{}' holds {what}, not a number, in the result of a `{kind}` run",
                c.path_text
            )))
            }
        }
    }
    let status = if constraints.iter().all(|c| c.ok) {
        Status::Ok
    } else {
        Status::ConstraintViolated
    };
    Ok(Member {
        design,
        scenario_sha256,
        result_sha256: Some(result_sha256),
        status,
        objectives: Some(objectives),
        constraints,
        error: None,
        facts: Some(run_facts),
    })
}

/// Evaluate one design of the space.
pub fn run_member(r: &Resolved, design: &[usize]) -> Result<Member, KshanaError> {
    let text = scenario_text(r, design)?;
    score(r, Some(design.to_vec()), &text, &|i| {
        let v = &r.space.variables[i];
        v.numeric
            .as_ref()
            .map(|n| n[design[i]])
            .ok_or_else(|| invalid(format!("variable '{}' is not a number", v.name)))
    })
}

/// Evaluate a design given as a full scenario text (a reference design from outside
/// the space). An objective that is a design variable is read at that variable's key
/// in the given scenario.
pub fn run_scenario_member(r: &Resolved, text: &str) -> Result<Member, KshanaError> {
    let doc: toml::Value = toml::from_str(text)
        .map_err(|e| invalid(format!("reference: the scenario is not valid TOML: {e}")))?;
    score(r, None, text, &|i| {
        let v = &r.space.variables[i];
        let at = toml_get(&doc, &v.path).map_err(|e| {
            invalid(format!(
                "reference: an objective is the variable '{}', but the reference scenario has no key '{}' ({e}); use a result path as the objective instead",
                v.name, v.key
            ))
        })?;
        match at {
            toml::Value::Integer(n) => Ok(*n as f64),
            toml::Value::Float(f) if f.is_finite() => Ok(*f),
            other => Err(invalid(format!(
                "reference: key '{}' holds a {}, not a number",
                v.key,
                other.type_str()
            ))),
        }
    })
}

/// Evaluate `designs` on up to `threads` threads. The result is in the order of
/// `designs` whatever the thread count, so the study's output does not depend on it.
pub fn evaluate_batch(
    r: &Resolved,
    designs: &[Vec<usize>],
    threads: usize,
) -> Result<Vec<Member>, KshanaError> {
    parallel_map(designs.len(), threads, |i| run_member(r, &designs[i]))
        .into_iter()
        .collect()
}

/// Apply `f` to `0..n` on up to `threads` scoped threads; results in index order.
pub fn parallel_map<T: Send>(n: usize, threads: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let workers = threads.clamp(1, n.max(1));
    if workers == 1 {
        return (0..n).map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<T>>> = (0..n).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                let out = f(i);
                *slots[i].lock().unwrap_or_else(|p| p.into_inner()) = Some(out);
            });
        }
    });
    slots
        .into_iter()
        .map(|m| {
            m.into_inner()
                .unwrap_or_else(|p| p.into_inner())
                .expect("every index was evaluated")
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_tells_null_from_missing() {
        let v = serde_json::json!({"a": {"b": 2.5, "n": null, "s": "x"}, "l": [1, 2]});
        let p = |s: &str| super::super::space::parse_path(s).unwrap();
        assert_eq!(lookup(&v, &p("a.b")), Lookup::Number(2.5));
        assert_eq!(lookup(&v, &p("l[1]")), Lookup::Number(2.0));
        assert_eq!(lookup(&v, &p("a.n")), Lookup::Null);
        assert_eq!(lookup(&v, &p("a.zz")), Lookup::Missing);
        assert_eq!(lookup(&v, &p("l[5]")), Lookup::Missing);
        assert!(matches!(lookup(&v, &p("a.s")), Lookup::NotNumeric(_)));
        assert!(matches!(lookup(&v, &p("a")), Lookup::NotNumeric(_)));
    }

    #[test]
    fn parallel_map_keeps_index_order() {
        let one: Vec<usize> = parallel_map(37, 1, |i| i * i);
        let four: Vec<usize> = parallel_map(37, 4, |i| i * i);
        assert_eq!(one, four);
        assert_eq!(one[36], 36 * 36);
        let none: Vec<usize> = parallel_map(0, 4, |i| i);
        assert!(none.is_empty());
    }
}
