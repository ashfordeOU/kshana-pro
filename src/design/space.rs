// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The design space of a study: its variables, its design constraints, the enumeration
//! of the feasible designs and the study hash.
//!
//! Every variable is a finite list of **levels** (a real or integer range with a step,
//! or an explicit list of choices), so a design is a vector of level indices, one per
//! variable. The grid of all index vectors is the design space; a Walker constraint
//! removes the vectors whose plane count does not divide the satellite count or whose
//! phasing is outside `0..planes`. Those are never turned into a scenario and never run.

use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::Study;

/// The most grid points a space may have and still be enumerated to count its feasible
/// designs. Beyond this the feasible count is reported as unknown and the search is
/// NSGA-II.
pub const ENUMERATION_CAP: u128 = 2_000_000;

/// The most levels one variable may have.
pub const MAX_LEVELS: usize = 1_000_000;

pub(crate) fn invalid(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

// ─── Paths ──────────────────────────────────────────────────────────────────────

/// One step of a dotted path: a table key or an array index (`shell[0]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

/// Parse a dotted path with `[i]` for array elements, for example
/// `constellation[0].shell[0].inclination_deg` or `global.pdop.p95`. A bare number
/// between dots (`samples.0.value`) is also an index.
pub fn parse_path(path: &str) -> Result<Vec<Seg>, String> {
    if path.trim().is_empty() {
        return Err("the path is empty".into());
    }
    let mut out = Vec::new();
    for part in path.split('.') {
        if part.is_empty() {
            return Err(format!("'{path}' has an empty step"));
        }
        let (name, mut rest) = match part.find('[') {
            Some(i) => (&part[..i], &part[i..]),
            None => (part, ""),
        };
        if !name.is_empty() {
            match name.parse::<usize>() {
                Ok(i) => out.push(Seg::Index(i)),
                Err(_) => out.push(Seg::Key(name.to_string())),
            }
        } else if rest.is_empty() {
            return Err(format!("'{path}' has an empty step"));
        }
        while !rest.is_empty() {
            let close = rest
                .find(']')
                .ok_or_else(|| format!("'{path}': '[' without ']'"))?;
            if !rest.starts_with('[') {
                return Err(format!("'{path}': unexpected text after ']'"));
            }
            let idx = rest[1..close]
                .trim()
                .parse::<usize>()
                .map_err(|_| format!("'{path}': '{}' is not an array index", &rest[1..close]))?;
            out.push(Seg::Index(idx));
            rest = &rest[close + 1..];
        }
    }
    Ok(out)
}

fn describe(path: &[Seg]) -> String {
    let mut s = String::new();
    for seg in path {
        match seg {
            Seg::Key(k) => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(k);
            }
            Seg::Index(i) => s.push_str(&format!("[{i}]")),
        }
    }
    if s.is_empty() {
        "the top level".into()
    } else {
        format!("'{s}'")
    }
}

/// Read the value at `path` in a scenario.
pub fn toml_get<'a>(root: &'a toml::Value, path: &[Seg]) -> Result<&'a toml::Value, String> {
    let mut cur = root;
    for (n, seg) in path.iter().enumerate() {
        let next = match seg {
            Seg::Key(k) => cur.as_table().and_then(|t| t.get(k)),
            Seg::Index(i) => cur.as_array().and_then(|a| a.get(*i)),
        };
        cur = next.ok_or_else(|| {
            let what = match seg {
                Seg::Key(k) => format!("no key '{k}'"),
                Seg::Index(i) => format!("no element [{i}]"),
            };
            format!("{what} under {}", describe(&path[..n]))
        })?;
    }
    Ok(cur)
}

/// Replace the value at `path` in a scenario. The path must already exist: a study
/// changes keys of the base scenario, it never adds any.
pub fn toml_set(root: &mut toml::Value, path: &[Seg], value: toml::Value) -> Result<(), String> {
    let mut cur = root;
    for (n, seg) in path.iter().enumerate() {
        let next = match seg {
            Seg::Key(k) => cur.as_table_mut().and_then(|t| t.get_mut(k)),
            Seg::Index(i) => cur.as_array_mut().and_then(|a| a.get_mut(*i)),
        };
        cur = next.ok_or_else(|| format!("nothing at step {} of {}", n + 1, describe(path)))?;
    }
    *cur = value;
    Ok(())
}

/// Read the value at `path` in a result document. `None` when any step is absent.
pub fn json_get<'a>(root: &'a serde_json::Value, path: &[Seg]) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for seg in path {
        cur = match seg {
            Seg::Key(k) => cur.get(k.as_str())?,
            Seg::Index(i) => cur.get(*i)?,
        };
    }
    Some(cur)
}

/// A result path in the spelling of the open engine's `units` block: array indices are
/// dropped (`constellations[0].satellites` → `constellations[].satellites`).
pub fn units_key(path: &str) -> String {
    let mut out = String::new();
    let mut in_bracket = false;
    for c in path.chars() {
        match c {
            '[' => {
                in_bracket = true;
                out.push('[');
            }
            ']' => {
                in_bracket = false;
                out.push(']');
            }
            _ if in_bracket => {}
            _ => out.push(c),
        }
    }
    out
}

// ─── Study file: variables and constraints ──────────────────────────────────────

/// One `[[variable]]` of a study file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VariableSpec {
    /// The name used by constraints, objectives, the reference design and the outputs.
    pub name: String,
    /// Dotted path of the key in the base scenario that this variable sets.
    pub key: String,
    /// `real`, `integer` or `choice`.
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// The explicit levels of a `choice` variable (numbers, strings or booleans).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<toml::Value>>,
    /// Optional unit, used in the outputs when the engine's result does not state one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// `walker = { total, planes, phasing }`: the three named variables must form a Walker
/// pattern (planes divides total, and phasing is in `0..planes`).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WalkerSpec {
    pub total: String,
    pub planes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phasing: Option<String>,
}

/// `result = { path, op, value }`: a condition on a number of the run's result.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultConstraintSpec {
    pub path: String,
    /// One of `<=`, `<`, `>=`, `>`, `==`, `!=`.
    pub op: String,
    pub value: f64,
}

/// One `[[constraint]]`: exactly one of `walker` or `result`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walker: Option<WalkerSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultConstraintSpec>,
}

// ─── Resolved space ─────────────────────────────────────────────────────────────

/// A variable checked against the base scenario, with its levels worked out.
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub key: String,
    pub path: Vec<Seg>,
    /// `real`, `integer` or `choice`.
    pub ty: String,
    /// The value written into the scenario at each level, already in the type of the
    /// base scenario's key (a float key stays a float, an integer key an integer).
    pub levels: Vec<toml::Value>,
    /// The levels as numbers, when every level is a number.
    pub numeric: Option<Vec<f64>>,
    /// True for a range (neighbouring levels are close in value); false for a `choice`
    /// list, whose order carries no meaning for the search.
    pub ordered: bool,
    pub unit: Option<String>,
}

/// A Walker constraint with its variables resolved to indices.
#[derive(Debug, Clone)]
pub struct Walker {
    pub total: usize,
    pub planes: usize,
    pub phasing: Option<usize>,
}

/// The comparison of a result constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Le,
    Lt,
    Ge,
    Gt,
    Eq,
    Ne,
}

impl Op {
    pub fn parse(s: &str) -> Option<Op> {
        Some(match s.trim() {
            "<=" => Op::Le,
            "<" => Op::Lt,
            ">=" => Op::Ge,
            ">" => Op::Gt,
            "==" | "=" => Op::Eq,
            "!=" => Op::Ne,
            _ => return None,
        })
    }
    pub fn holds(self, a: f64, b: f64) -> bool {
        match self {
            Op::Le => a <= b,
            Op::Lt => a < b,
            Op::Ge => a >= b,
            Op::Gt => a > b,
            Op::Eq => a == b,
            Op::Ne => a != b,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Le => "<=",
            Op::Lt => "<",
            Op::Ge => ">=",
            Op::Gt => ">",
            Op::Eq => "==",
            Op::Ne => "!=",
        }
    }
}

/// A result constraint with its path parsed.
#[derive(Debug, Clone)]
pub struct ResultConstraint {
    pub path_text: String,
    pub path: Vec<Seg>,
    pub op: Op,
    pub value: f64,
}

/// The design space of a study.
#[derive(Debug, Clone)]
pub struct Space {
    pub variables: Vec<Variable>,
    pub walkers: Vec<Walker>,
    /// Number of index vectors in the grid (the product of the level counts).
    pub grid_size: u128,
}

/// The decimal places needed to write `x` exactly (at most 12).
fn decimals(x: f64) -> u32 {
    for d in 0..=12u32 {
        let p = 10f64.powi(d as i32);
        if ((x * p).round() / p - x).abs() <= f64::EPSILON * x.abs().max(1.0) * 4.0 {
            return d;
        }
    }
    12
}

fn range_levels(name: &str, min: f64, max: f64, step: f64) -> Result<Vec<f64>, KshanaError> {
    for (label, v) in [("min", min), ("max", max), ("step", step)] {
        if !v.is_finite() {
            return Err(invalid(format!(
                "variable '{name}': {label} is not a finite number ({v})"
            )));
        }
    }
    if step <= 0.0 {
        return Err(invalid(format!(
            "variable '{name}': step must be greater than zero (got {step})"
        )));
    }
    if max < min {
        return Err(invalid(format!(
            "variable '{name}': max ({max}) is below min ({min})"
        )));
    }
    let n = ((max - min) / step + 1e-9).floor();
    if n + 1.0 > MAX_LEVELS as f64 {
        return Err(invalid(format!(
            "variable '{name}': {} levels is more than the {MAX_LEVELS} allowed; use a larger step",
            n + 1.0
        )));
    }
    let d = decimals(min).max(decimals(step));
    let p = 10f64.powi(d as i32);
    Ok((0..=(n as usize))
        .map(|k| ((min + k as f64 * step) * p).round() / p)
        .collect())
}

/// Put a number into the type the base scenario's key already has.
fn coerce(name: &str, base: &toml::Value, x: f64) -> Result<toml::Value, KshanaError> {
    match base {
        toml::Value::Float(_) => Ok(toml::Value::Float(x)),
        toml::Value::Integer(_) => {
            if x.fract() != 0.0 || x.abs() > 9.0e15 {
                Err(invalid(format!(
                    "variable '{name}': the base scenario holds an integer at its key, but the level {x} is not a whole number"
                )))
            } else {
                Ok(toml::Value::Integer(x as i64))
            }
        }
        other => Err(invalid(format!(
            "variable '{name}': the base scenario holds a {} at its key, which a number cannot replace; use type = \"choice\"",
            other.type_str()
        ))),
    }
}

impl Space {
    /// Check the variables and design constraints of `study` against its base scenario.
    pub fn resolve(study: &Study, base: &toml::Value) -> Result<Space, KshanaError> {
        if study.variables.is_empty() {
            return Err(invalid(
                "the study has no [[variable]]: a design space needs at least one variable",
            ));
        }
        let mut variables: Vec<Variable> = Vec::new();
        for v in &study.variables {
            if v.name.trim().is_empty() {
                return Err(invalid("a [[variable]] has an empty name"));
            }
            if variables.iter().any(|o| o.name == v.name) {
                return Err(invalid(format!(
                    "variable '{}' is defined more than once",
                    v.name
                )));
            }
            let path = parse_path(&v.key)
                .map_err(|e| invalid(format!("variable '{}': key {e}", v.name)))?;
            let base_value = toml_get(base, &path).map_err(|e| {
                invalid(format!(
                    "variable '{}': key '{}' does not exist in the base scenario ({e})",
                    v.name, v.key
                ))
            })?;
            let (levels, numeric, ordered) = match v.ty.as_str() {
                "real" | "integer" => {
                    if v.values.is_some() {
                        return Err(invalid(format!(
                            "variable '{}': a {} variable takes min, max and step; use type = \"choice\" for a list of values",
                            v.name, v.ty
                        )));
                    }
                    let min = v
                        .min
                        .ok_or_else(|| invalid(format!("variable '{}': min is missing", v.name)))?;
                    let max = v
                        .max
                        .ok_or_else(|| invalid(format!("variable '{}': max is missing", v.name)))?;
                    let step = match (v.step, v.ty.as_str()) {
                        (Some(s), _) => s,
                        (None, "integer") => 1.0,
                        (None, _) => {
                            return Err(invalid(format!(
                                "variable '{}': step is missing (a real variable is searched on a grid)",
                                v.name
                            )))
                        }
                    };
                    let nums = range_levels(&v.name, min, max, step)?;
                    if v.ty == "integer" && nums.iter().any(|x| x.fract() != 0.0) {
                        return Err(invalid(format!(
                            "variable '{}': an integer variable needs whole-number min and step",
                            v.name
                        )));
                    }
                    let levels = nums
                        .iter()
                        .map(|&x| coerce(&v.name, base_value, x))
                        .collect::<Result<Vec<_>, _>>()?;
                    (levels, Some(nums), true)
                }
                "choice" => {
                    let vals = v.values.as_ref().ok_or_else(|| {
                        invalid(format!(
                            "variable '{}': a choice variable needs values = [...]",
                            v.name
                        ))
                    })?;
                    if vals.is_empty() {
                        return Err(invalid(format!("variable '{}': values is empty", v.name)));
                    }
                    if vals.len() > MAX_LEVELS {
                        return Err(invalid(format!(
                            "variable '{}': more than {MAX_LEVELS} values",
                            v.name
                        )));
                    }
                    let mut levels = Vec::new();
                    let mut nums = Vec::new();
                    for val in vals {
                        match val {
                            toml::Value::Integer(i) => {
                                levels.push(coerce(&v.name, base_value, *i as f64)?);
                                nums.push(*i as f64);
                            }
                            toml::Value::Float(f) => {
                                if !f.is_finite() {
                                    return Err(invalid(format!(
                                        "variable '{}': a value is not a finite number ({f})",
                                        v.name
                                    )));
                                }
                                levels.push(coerce(&v.name, base_value, *f)?);
                                nums.push(*f);
                            }
                            toml::Value::String(_) | toml::Value::Boolean(_) => {
                                if !base_value.same_type(val) {
                                    return Err(invalid(format!(
                                        "variable '{}': the base scenario holds a {} at its key, but a value is a {}",
                                        v.name,
                                        base_value.type_str(),
                                        val.type_str()
                                    )));
                                }
                                levels.push(val.clone());
                            }
                            other => {
                                return Err(invalid(format!(
                                "variable '{}': a value is a {}; use numbers, strings or booleans",
                                v.name,
                                other.type_str()
                            )))
                            }
                        }
                    }
                    for (i, a) in levels.iter().enumerate() {
                        if levels[..i].contains(a) {
                            return Err(invalid(format!(
                                "variable '{}': the value {a} is listed twice",
                                v.name
                            )));
                        }
                    }
                    let numeric = (nums.len() == levels.len()).then_some(nums);
                    (levels, numeric, false)
                }
                other => {
                    return Err(invalid(format!(
                        "variable '{}': type '{other}' is not real, integer or choice",
                        v.name
                    )))
                }
            };
            variables.push(Variable {
                name: v.name.clone(),
                key: v.key.clone(),
                path,
                ty: v.ty.clone(),
                levels,
                numeric,
                ordered,
                unit: v.unit.clone(),
            });
        }

        let index_of = |name: &str, role: &str| -> Result<usize, KshanaError> {
            let i = variables
                .iter()
                .position(|v| v.name == name)
                .ok_or_else(|| {
                    invalid(format!(
                        "walker constraint: {role} = \"{name}\" is not a variable of this study"
                    ))
                })?;
            let whole = variables[i]
                .numeric
                .as_ref()
                .is_some_and(|n| n.iter().all(|x| x.fract() == 0.0 && *x >= 0.0));
            if !whole {
                return Err(invalid(format!(
                    "walker constraint: variable '{name}' ({role}) must take whole numbers, zero or more"
                )));
            }
            Ok(i)
        };
        let mut walkers = Vec::new();
        for c in &study.constraints {
            match (&c.walker, &c.result) {
                (Some(w), None) => walkers.push(Walker {
                    total: index_of(&w.total, "total")?,
                    planes: index_of(&w.planes, "planes")?,
                    phasing: match &w.phasing {
                        Some(p) => Some(index_of(p, "phasing")?),
                        None => None,
                    },
                }),
                (None, Some(_)) => {}
                (Some(_), Some(_)) => {
                    return Err(invalid(
                        "a [[constraint]] has both walker and result; write two constraints",
                    ))
                }
                (None, None) => {
                    return Err(invalid(
                        "a [[constraint]] has neither walker = {...} nor result = {...}",
                    ))
                }
            }
        }
        let grid_size = variables
            .iter()
            .fold(1u128, |acc, v| acc.saturating_mul(v.levels.len() as u128));
        Ok(Space {
            variables,
            walkers,
            grid_size,
        })
    }

    /// The number of levels of each variable.
    pub fn level_counts(&self) -> Vec<usize> {
        self.variables.iter().map(|v| v.levels.len()).collect()
    }

    /// True when the design satisfies every design (Walker) constraint.
    pub fn feasible(&self, design: &[usize]) -> bool {
        self.walkers.iter().all(|w| {
            let num = |var: usize| -> f64 {
                self.variables[var]
                    .numeric
                    .as_ref()
                    .map(|n| n[design[var]])
                    .unwrap_or(f64::NAN)
            };
            let total = num(w.total) as u64;
            let planes = num(w.planes) as u64;
            if planes == 0 || total == 0 || total % planes != 0 {
                return false;
            }
            match w.phasing {
                Some(p) => (num(p) as u64) < planes,
                None => true,
            }
        })
    }

    /// True when the grid is small enough to enumerate.
    pub fn enumerable(&self) -> bool {
        self.grid_size <= ENUMERATION_CAP
    }

    /// Every feasible design, in design order: the first variable changes slowest, the
    /// last fastest. `None` when the grid is larger than [`ENUMERATION_CAP`].
    pub fn enumerate(&self) -> Option<Vec<Vec<usize>>> {
        if !self.enumerable() {
            return None;
        }
        let counts = self.level_counts();
        let mut out = Vec::new();
        let mut cur = vec![0usize; counts.len()];
        loop {
            if self.feasible(&cur) {
                out.push(cur.clone());
            }
            // Odometer step, last variable fastest.
            let mut i = counts.len();
            loop {
                if i == 0 {
                    return Some(out);
                }
                i -= 1;
                cur[i] += 1;
                if cur[i] < counts[i] {
                    break;
                }
                cur[i] = 0;
            }
        }
    }

    /// The level indices of a design given as `name = value` pairs (the reference
    /// design). Every variable must be given and every value must be one of its levels.
    pub fn design_from_values(
        &self,
        values: &toml::value::Table,
    ) -> Result<Vec<usize>, KshanaError> {
        for k in values.keys() {
            if !self.variables.iter().any(|v| &v.name == k) {
                return Err(invalid(format!(
                    "reference: '{k}' is not a variable of this study"
                )));
            }
        }
        self.variables
            .iter()
            .map(|v| {
                let given = values.get(&v.name).ok_or_else(|| {
                    invalid(format!("reference: no value for variable '{}'", v.name))
                })?;
                let as_num = match given {
                    toml::Value::Integer(i) => Some(*i as f64),
                    toml::Value::Float(f) => Some(*f),
                    _ => None,
                };
                let hit = match (&v.numeric, as_num) {
                    (Some(nums), Some(x)) => nums.iter().position(|n| *n == x),
                    _ => v.levels.iter().position(|l| l == given),
                };
                hit.ok_or_else(|| {
                    invalid(format!(
                        "reference: {given} is not a level of variable '{}'; give the reference as a full scenario to place a design from outside the space",
                        v.name
                    ))
                })
            })
            .collect()
    }

    /// The values of a design as a JSON object, keyed by variable name.
    pub fn values_json(&self, design: &[usize]) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        for (v, &i) in self.variables.iter().zip(design) {
            m.insert(v.name.clone(), toml_to_json(&v.levels[i]));
        }
        serde_json::Value::Object(m)
    }
}

// ─── Study hash ─────────────────────────────────────────────────────────────────

/// A TOML value as JSON. A float that is a whole number keeps its `.0`, so an integer
/// key and a float key never collide.
pub fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::json!(i),
        toml::Value::Float(f) => serde_json::json!(f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(d) => serde_json::Value::String(d.to_string()),
        toml::Value::Array(a) => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => serde_json::Value::Object(
            t.iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect(),
        ),
    }
}

/// Compact JSON with object keys in sorted order at every depth.
pub fn canonical_json(v: &serde_json::Value) -> String {
    fn write(v: &serde_json::Value, out: &mut String) {
        match v {
            serde_json::Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                out.push('{');
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::Value::String((*k).clone()).to_string());
                    out.push(':');
                    write(&m[*k], out);
                }
                out.push('}');
            }
            serde_json::Value::Array(a) => {
                out.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(x, out);
                }
                out.push(']');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut s = String::new();
    write(v, &mut s);
    s
}

/// SHA-256 of a byte string, as lower-case hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The canonical form of a study: the parsed study as JSON with sorted keys, the base
/// scenario and a reference scenario replaced by their parsed content, and the thread
/// count left out. Comments, key order, spacing and the number of threads therefore do
/// not change it; any change to a value does.
pub fn canonical_study(study: &Study, base: &toml::Value) -> Result<String, KshanaError> {
    let mut v = serde_json::to_value(study)
        .map_err(|e| invalid(format!("the study cannot be written as JSON: {e}")))?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| invalid("the study is not a table"))?;
    obj.insert("kind".into(), serde_json::json!(super::KIND));
    obj.insert("base_scenario".into(), toml_to_json(base));
    if let Some(search) = obj.get_mut("search").and_then(|s| s.as_object_mut()) {
        search.remove("threads");
    }
    if let Some(reference) = obj.get_mut("reference").and_then(|s| s.as_object_mut()) {
        if let Some(text) = reference
            .get("scenario")
            .and_then(|s| s.as_str())
            .map(str::to_string)
        {
            let parsed: toml::Value = toml::from_str(&text)
                .map_err(|e| invalid(format!("reference: the scenario is not valid TOML: {e}")))?;
            reference.insert("scenario".into(), toml_to_json(&parsed));
        }
    }
    Ok(canonical_json(&v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_parse_keys_and_indices() {
        assert_eq!(
            parse_path("constellation[0].shell[1].inclination_deg").unwrap(),
            vec![
                Seg::Key("constellation".into()),
                Seg::Index(0),
                Seg::Key("shell".into()),
                Seg::Index(1),
                Seg::Key("inclination_deg".into()),
            ]
        );
        assert_eq!(
            parse_path("samples.2.v").unwrap(),
            vec![
                Seg::Key("samples".into()),
                Seg::Index(2),
                Seg::Key("v".into())
            ]
        );
        assert!(parse_path("").is_err());
        assert!(parse_path("a..b").is_err());
        assert!(parse_path("a[x]").is_err());
        assert!(parse_path("a[1").is_err());
    }

    #[test]
    fn units_key_drops_indices() {
        assert_eq!(
            units_key("constellations[0].shells[12].period_min"),
            "constellations[].shells[].period_min"
        );
        assert_eq!(units_key("global.pdop.p95"), "global.pdop.p95");
    }

    #[test]
    fn set_replaces_only_an_existing_key() {
        let mut v: toml::Value = toml::from_str("a = 1\n[[t]]\nx = 1.0\n[[t]]\nx = 2.0\n").unwrap();
        toml_set(
            &mut v,
            &parse_path("t[1].x").unwrap(),
            toml::Value::Float(9.0),
        )
        .unwrap();
        assert_eq!(
            toml_get(&v, &parse_path("t[1].x").unwrap())
                .unwrap()
                .as_float(),
            Some(9.0)
        );
        assert_eq!(
            toml_get(&v, &parse_path("t[0].x").unwrap())
                .unwrap()
                .as_float(),
            Some(1.0)
        );
        assert!(toml_set(
            &mut v,
            &parse_path("t[2].x").unwrap(),
            toml::Value::Float(0.0)
        )
        .is_err());
        assert!(toml_get(&v, &parse_path("b").unwrap())
            .unwrap_err()
            .contains("no key 'b'"));
    }

    #[test]
    fn range_levels_are_exact_decimals() {
        let l = range_levels("x", 0.1, 0.5, 0.1).unwrap();
        assert_eq!(l, vec![0.1, 0.2, 0.3, 0.4, 0.5]);
        let l = range_levels("x", 1500.0, 8000.0, 2500.0).unwrap();
        assert_eq!(l, vec![1500.0, 4000.0, 6500.0]);
        assert!(range_levels("x", f64::NAN, 1.0, 0.1).is_err());
        assert!(range_levels("x", 0.0, f64::INFINITY, 0.1).is_err());
        assert!(range_levels("x", 0.0, 1.0, 0.0).is_err());
        assert!(range_levels("x", 1.0, 0.0, 0.1).is_err());
    }

    #[test]
    fn canonical_json_sorts_keys_at_every_depth() {
        let a: serde_json::Value =
            serde_json::from_str(r#"{"b":{"z":1,"a":[{"y":2,"x":1}]},"a":1.0}"#).unwrap();
        assert_eq!(
            canonical_json(&a),
            r#"{"a":1.0,"b":{"a":[{"x":1,"y":2}],"z":1}}"#
        );
    }
}
