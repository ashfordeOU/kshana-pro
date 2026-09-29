// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Machine-checkable acceptance criteria over an open-engine result document.
//!
//! A criterion is a small expression over the fields of a `result.json` the open
//! Kshana engine wrote, with units. It is evaluated against the real document and
//! unit-checked against the engine's own field-units schema. The grammar:
//!
//! ```text
//! criterion  := clause ( "and" clause )*
//! clause     := operand CMP operand [ "for" ("all" | "any") [NAME] ]
//! CMP        := "<" | "<=" | ">" | ">=" | "==" | "!="
//! operand    := NUMBER [UNIT] | "true" | "false" | PATH
//!             | FUNC "(" operand ")"
//!             | ("fraction" | "count") "(" operand CMP operand ")"
//! FUNC       := max | min | mean | rms | abs_max | p95 | sum | count | len
//!             | first | last | abs | fraction
//! PATH       := SEG ( "." SEG )*        SEG := NAME | NAME "[]" | NAME "[" INDEX "]"
//! ```
//!
//! Paths follow the open engine's own path grammar (`kshana::field_schema`): an array
//! contributes one segment, and its `[]` suffix is optional, so `epochs.hpl_m` and
//! `epochs[].hpl_m` both name the `hpl_m` column of every row of `epochs`. A path
//! whose last segment is an aggregate name and that does not exist as written is read
//! as that aggregate over its parent, so `classical.series.error_ns.max` is
//! `max(classical.series[].error_ns)`.
//!
//! An operand that names an array must be reduced by an aggregate or quantified with
//! `for all` / `for any`; comparing a whole array to a number without saying which is a
//! criterion error, reported as *not verified*, never silently read as one element.
//! `for all epochs` also checks that an operand really iterates `epochs`.
//!
//! **Units.** A literal's unit is converted into the field's unit before comparing; a
//! literal with no unit is accepted only against a dimensionless or `count` field. Two
//! fields are compared only when their units are commensurable. A field with no unit
//! in the schema or the document, or whose unit is not a unit (`mixed - see note`), is
//! *not verified*. Null elements of an array are skipped and counted.

use serde::Serialize;

use super::units::{parse_unit, Unit};

/// A comparison operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl CmpOp {
    fn symbol(self) -> &'static str {
        match self {
            CmpOp::Lt => "<",
            CmpOp::Le => "<=",
            CmpOp::Gt => ">",
            CmpOp::Ge => ">=",
            CmpOp::Eq => "==",
            CmpOp::Ne => "!=",
        }
    }
    fn holds(self, a: f64, b: f64) -> bool {
        match self {
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
            // Exact comparison is what the requirement author wrote; tolerance, if
            // wanted, is written as a pair of inequalities.
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
        }
    }
}

/// An aggregate or element-wise function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Func {
    Max,
    Min,
    Mean,
    Rms,
    AbsMax,
    P95,
    Sum,
    Count,
    Len,
    First,
    Last,
    Abs,
    Fraction,
}

impl Func {
    fn parse(s: &str) -> Option<Func> {
        Some(match s {
            "max" => Func::Max,
            "min" => Func::Min,
            "mean" => Func::Mean,
            "rms" => Func::Rms,
            "abs_max" => Func::AbsMax,
            "p95" => Func::P95,
            "sum" => Func::Sum,
            "count" => Func::Count,
            "len" => Func::Len,
            "first" => Func::First,
            "last" => Func::Last,
            "abs" => Func::Abs,
            "fraction" => Func::Fraction,
            _ => return None,
        })
    }
    fn name(self) -> &'static str {
        match self {
            Func::Max => "max",
            Func::Min => "min",
            Func::Mean => "mean",
            Func::Rms => "rms",
            Func::AbsMax => "abs_max",
            Func::P95 => "p95",
            Func::Sum => "sum",
            Func::Count => "count",
            Func::Len => "len",
            Func::First => "first",
            Func::Last => "last",
            Func::Abs => "abs",
            Func::Fraction => "fraction",
        }
    }
    /// Aggregates that may be written as a trailing path segment.
    fn is_suffix_aggregate(s: &str) -> bool {
        matches!(
            s,
            "max" | "min" | "mean" | "rms" | "abs_max" | "p95" | "sum" | "count" | "first" | "last"
        )
    }
}

/// One operand of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Operand {
    Number { value: f64, unit: Option<String> },
    Bool(bool),
    Path(String),
    Call { func: Func, arg: Box<Operand> },
    Predicate { func: Func, cmp: Box<Comparison> },
}

/// `lhs op rhs`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    pub lhs: Operand,
    pub op: CmpOp,
    pub rhs: Operand,
}

/// A quantifier over array operands.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Quantifier {
    pub all: bool,
    pub over: Option<String>,
}

/// One clause of a criterion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Clause {
    pub cmp: Comparison,
    pub quantifier: Option<Quantifier>,
}

/// A parsed criterion.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Criterion {
    pub source: String,
    pub clauses: Vec<Clause>,
}

// ------------------------------------------------------------------ lexer

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64, Option<String>),
    Word(String),
    Cmp(CmpOp),
    LParen,
    RParen,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let is_word_start = |c: char| c.is_ascii_alphabetic() || c == '_';
    let is_word =
        |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']';
    let is_unit_start = |c: char| c.is_alphabetic() || c == '%' || c == 'µ' || c == '°';
    let is_unit = |c: char| {
        c.is_alphanumeric() || matches!(c, '%' | 'µ' | 'μ' | '°' | '/' | '^' | '*' | '-' | '_')
    };
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '<' | '>' | '=' | '!' => {
                let two = chars.get(i + 1) == Some(&'=');
                let op = match (c, two) {
                    ('<', true) => CmpOp::Le,
                    ('<', false) => CmpOp::Lt,
                    ('>', true) => CmpOp::Ge,
                    ('>', false) => CmpOp::Gt,
                    ('=', true) => CmpOp::Eq,
                    ('!', true) => CmpOp::Ne,
                    _ => return Err(format!("unexpected '{c}' at column {}", i + 1)),
                };
                out.push(Tok::Cmp(op));
                i += if two { 2 } else { 1 };
            }
            _ if c.is_ascii_digit()
                || ((c == '-' || c == '+' || c == '.')
                    && chars
                        .get(i + 1)
                        .is_some_and(|d| d.is_ascii_digit() || *d == '.')) =>
            {
                let start = i;
                i += 1;
                while i < chars.len()
                    && (chars[i].is_ascii_digit()
                        || chars[i] == '.'
                        || ((chars[i] == 'e' || chars[i] == 'E')
                            && chars
                                .get(i + 1)
                                .is_some_and(|d| d.is_ascii_digit() || *d == '-' || *d == '+'))
                        || ((chars[i] == '-' || chars[i] == '+')
                            && (chars[i - 1] == 'e' || chars[i - 1] == 'E')))
                {
                    i += 1;
                }
                let raw: String = chars[start..i].iter().collect();
                let value: f64 = raw
                    .parse()
                    .map_err(|_| format!("'{raw}' is not a number"))?;
                // An optional unit follows after whitespace.
                let mut j = i;
                while j < chars.len() && chars[j] == ' ' {
                    j += 1;
                }
                let mut unit = None;
                if j < chars.len() && is_unit_start(chars[j]) {
                    let us = j;
                    while j < chars.len() && is_unit(chars[j]) {
                        j += 1;
                    }
                    let u: String = chars[us..j].iter().collect();
                    if u != "for" && u != "and" {
                        unit = Some(u);
                        i = j;
                    }
                }
                out.push(Tok::Num(value, unit));
            }
            _ if is_word_start(c) => {
                let start = i;
                while i < chars.len() && is_word(chars[i]) {
                    i += 1;
                }
                out.push(Tok::Word(chars[start..i].iter().collect()));
            }
            _ => return Err(format!("unexpected '{c}' at column {}", i + 1)),
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ parser

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }
    fn word_is(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(x)) if x == w)
    }

    fn operand(&mut self) -> Result<Operand, String> {
        match self.next() {
            Some(Tok::Num(v, u)) => Ok(Operand::Number { value: v, unit: u }),
            Some(Tok::Word(w)) if w == "true" => Ok(Operand::Bool(true)),
            Some(Tok::Word(w)) if w == "false" => Ok(Operand::Bool(false)),
            Some(Tok::Word(w)) => {
                if self.peek() == Some(&Tok::LParen) {
                    let func = Func::parse(&w).ok_or_else(|| format!("unknown function '{w}'"))?;
                    self.next();
                    let inner = self.operand()?;
                    let out = if let Some(Tok::Cmp(op)) = self.peek().cloned() {
                        if !matches!(func, Func::Fraction | Func::Count) {
                            return Err(format!(
                                "'{}' takes a value, not a comparison",
                                func.name()
                            ));
                        }
                        self.next();
                        let rhs = self.operand()?;
                        Operand::Predicate {
                            func,
                            cmp: Box::new(Comparison {
                                lhs: inner,
                                op,
                                rhs,
                            }),
                        }
                    } else {
                        if func == Func::Fraction {
                            return Err(
                                "'fraction' takes a comparison, e.g. fraction(x == true)".into()
                            );
                        }
                        Operand::Call {
                            func,
                            arg: Box::new(inner),
                        }
                    };
                    match self.next() {
                        Some(Tok::RParen) => Ok(out),
                        _ => Err(format!("missing ')' after {}(...)", func.name())),
                    }
                } else {
                    validate_path(&w)?;
                    Ok(Operand::Path(w))
                }
            }
            Some(t) => Err(format!("expected a value, found {t:?}")),
            None => Err("expected a value, found the end of the criterion".into()),
        }
    }

    fn clause(&mut self) -> Result<Clause, String> {
        let lhs = self.operand()?;
        let op = match self.next() {
            Some(Tok::Cmp(op)) => op,
            other => return Err(format!("expected a comparison operator, found {other:?}")),
        };
        let rhs = self.operand()?;
        let mut quantifier = None;
        if self.word_is("for") {
            self.next();
            let all = match self.next() {
                Some(Tok::Word(w)) if w == "all" => true,
                Some(Tok::Word(w)) if w == "any" => false,
                other => {
                    return Err(format!(
                        "expected 'all' or 'any' after 'for', found {other:?}"
                    ))
                }
            };
            let over = match self.peek() {
                Some(Tok::Word(w)) if w != "and" => {
                    let w = w.clone();
                    self.next();
                    Some(w)
                }
                _ => None,
            };
            quantifier = Some(Quantifier { all, over });
        }
        Ok(Clause {
            cmp: Comparison { lhs, op, rhs },
            quantifier,
        })
    }
}

fn validate_path(p: &str) -> Result<(), String> {
    for seg in p.split('.') {
        let (name, rest) = match seg.find('[') {
            Some(i) => (&seg[..i], &seg[i..]),
            None => (seg, ""),
        };
        if name.is_empty() {
            return Err(format!("empty segment in path '{p}'"));
        }
        if !rest.is_empty() {
            let inner = rest
                .strip_prefix('[')
                .and_then(|r| r.strip_suffix(']'))
                .ok_or_else(|| format!("bad index in path segment '{seg}'"))?;
            if !inner.is_empty() && inner.parse::<usize>().is_err() {
                return Err(format!("bad index in path segment '{seg}'"));
            }
        }
    }
    Ok(())
}

/// Parse a criterion. The error names what is wrong in plain words.
pub fn parse(src: &str) -> Result<Criterion, String> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err("empty criterion".into());
    }
    let mut p = Parser { toks, pos: 0 };
    let mut clauses = vec![p.clause()?];
    while p.word_is("and") {
        p.next();
        clauses.push(p.clause()?);
    }
    if let Some(t) = p.peek() {
        return Err(format!("unexpected {t:?} after the criterion"));
    }
    Ok(Criterion {
        source: src.trim().to_string(),
        clauses,
    })
}

/// Every field path a criterion reads, as written.
pub fn paths_of(c: &Criterion) -> Vec<String> {
    fn walk(o: &Operand, out: &mut Vec<String>) {
        match o {
            Operand::Path(p) => out.push(p.clone()),
            Operand::Call { arg, .. } => walk(arg, out),
            Operand::Predicate { cmp, .. } => {
                walk(&cmp.lhs, out);
                walk(&cmp.rhs, out);
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cl in &c.clauses {
        walk(&cl.cmp.lhs, &mut out);
        walk(&cl.cmp.rhs, &mut out);
    }
    out
}

// ------------------------------------------------------------------ evaluation

/// Where a field's unit comes from. Implemented by the evidence layer over the
/// field-units schema and the result document's own `units` block.
pub trait UnitLookup {
    /// `(unit string, where it was read)` for a canonical (`[]`-suffixed) path.
    fn unit_for(&self, canonical_path: &str) -> Option<(String, String)>;
}

/// The verdict of a criterion against one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Verdict {
    #[serde(rename = "met")]
    Met,
    #[serde(rename = "not met")]
    NotMet,
    #[serde(rename = "not verified")]
    NotVerified,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Met => "met",
            Verdict::NotMet => "not met",
            Verdict::NotVerified => "not verified",
        }
    }
}

/// One field read while evaluating a criterion: the evidence a reviewer checks.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Observation {
    /// The canonical field path, `[]` for each array level.
    pub path: String,
    /// The value compared: a number, a boolean, or for an array the worst element.
    pub value: serde_json::Value,
    /// The field's unit as the engine states it.
    pub unit: Option<String>,
    /// Where that unit was read.
    pub unit_source: Option<String>,
    /// What was compared, in words.
    pub detail: String,
}

/// The outcome of evaluating one criterion against one result document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Evaluation {
    pub verdict: Verdict,
    /// Why the verdict is `not verified` (or which clause failed).
    pub reason: Option<String>,
    pub observations: Vec<Observation>,
}

#[derive(Debug, Clone)]
struct UnitInfo {
    text: Option<String>,
    parsed: Option<Unit>,
    source: Option<String>,
    from_literal: bool,
}

#[derive(Debug, Clone)]
enum Val {
    Num(f64, UnitInfo),
    Bool(bool),
    NumArr(Vec<Option<f64>>, UnitInfo),
    BoolArr(Vec<Option<bool>>),
}

/// A value plus the evidence gathered producing it.
struct Evald {
    val: Val,
    /// Canonical path of the (first) field behind the value.
    path: Option<String>,
    /// Canonical array names this operand iterates.
    iterates: Vec<String>,
}

/// An error that makes the verdict `not verified`.
struct NotVerified(String);

/// Resolve `path` against `doc` following the engine's path grammar. Returns the leaf
/// values (one per array element, `None` where an element lacks the field), whether any
/// array was expanded, the canonical path, and the array names iterated.
#[allow(clippy::type_complexity)]
fn resolve<'a>(
    doc: &'a serde_json::Value,
    path: &str,
) -> Option<(
    Vec<Option<&'a serde_json::Value>>,
    bool,
    String,
    Vec<String>,
)> {
    let segs: Vec<&str> = path.split('.').collect();
    let mut cur: Vec<Option<&serde_json::Value>> = vec![Some(doc)];
    let mut arrayed = false;
    let mut canon: Vec<String> = Vec::new();
    let mut iterates = Vec::new();
    let mut found_any = false;
    for (si, seg) in segs.iter().enumerate() {
        let (name, index) = match seg.find('[') {
            Some(i) => (&seg[..i], Some(&seg[i + 1..seg.len() - 1])),
            None => (*seg, None),
        };
        let last = si + 1 == segs.len();
        let mut next: Vec<Option<&serde_json::Value>> = Vec::new();
        let mut seg_canon = name.to_string();
        let mut expanded = false;
        found_any = false;
        for v in &cur {
            let x = v.and_then(|v| v.get(name));
            if x.is_some() {
                found_any = true;
            }
            match (x, index) {
                (Some(serde_json::Value::Array(a)), Some(ix)) if !ix.is_empty() => {
                    let i: usize = ix.parse().ok()?;
                    // One element of the column: the unit entry is the column's.
                    seg_canon = format!("{name}[]");
                    next.push(a.get(i));
                }
                (Some(serde_json::Value::Array(a)), _) => {
                    // Expand (and keep expanding nested arrays of arrays).
                    let mut level: Vec<&serde_json::Value> = a.iter().collect();
                    let mut depth = 1;
                    while !level.is_empty() && level.iter().all(|e| e.is_array()) && !last {
                        level = level
                            .iter()
                            .flat_map(|e| e.as_array().into_iter().flatten())
                            .collect();
                        depth += 1;
                    }
                    if last {
                        while !level.is_empty() && level.iter().all(|e| e.is_array()) {
                            level = level
                                .iter()
                                .flat_map(|e| e.as_array().into_iter().flatten())
                                .collect();
                            depth += 1;
                        }
                    }
                    expanded = true;
                    seg_canon = format!("{name}{}", "[]".repeat(depth));
                    next.extend(level.into_iter().map(Some));
                }
                (Some(other), Some(ix)) if !ix.is_empty() => {
                    let _ = other;
                    return None;
                }
                (x, _) => next.push(x),
            }
        }
        if !found_any {
            return None;
        }
        if expanded {
            arrayed = true;
            iterates.push(name.to_string());
        }
        canon.push(seg_canon);
        cur = next;
    }
    if !found_any {
        return None;
    }
    Some((cur, arrayed, canon.join("."), iterates))
}

fn unit_of(lookup: &dyn UnitLookup, canon: &str) -> UnitInfo {
    match lookup.unit_for(canon) {
        Some((text, source)) => UnitInfo {
            parsed: parse_unit(&text),
            text: Some(text),
            source: Some(source),
            from_literal: false,
        },
        None => UnitInfo {
            text: None,
            parsed: None,
            source: None,
            from_literal: false,
        },
    }
}

fn eval_path(
    doc: &serde_json::Value,
    lookup: &dyn UnitLookup,
    path: &str,
) -> Result<Evald, NotVerified> {
    // As written first; then as `<parent>.<aggregate>`.
    if let Some((vals, arrayed, canon, iterates)) = resolve(doc, path) {
        return leaf_values(vals, arrayed, canon, iterates, lookup);
    }
    if let Some((parent, agg)) = path.rsplit_once('.') {
        if Func::is_suffix_aggregate(agg) {
            if let Some((vals, arrayed, canon, iterates)) = resolve(doc, parent) {
                let inner = leaf_values(vals, arrayed, canon, iterates, lookup)?;
                let func = Func::parse(agg).expect("suffix aggregate is a function");
                return apply(func, inner);
            }
        }
    }
    Err(NotVerified(format!(
        "field '{path}' is not in the result document"
    )))
}

fn leaf_values(
    vals: Vec<Option<&serde_json::Value>>,
    arrayed: bool,
    canon: String,
    iterates: Vec<String>,
    lookup: &dyn UnitLookup,
) -> Result<Evald, NotVerified> {
    let all_bool = vals.iter().all(|v| {
        matches!(
            v,
            Some(serde_json::Value::Bool(_)) | None | Some(serde_json::Value::Null)
        )
    });
    let any_bool = vals
        .iter()
        .any(|v| matches!(v, Some(serde_json::Value::Bool(_))));
    if any_bool && all_bool {
        let b: Vec<Option<bool>> = vals.iter().map(|v| v.and_then(|x| x.as_bool())).collect();
        let val = if arrayed {
            Val::BoolArr(b)
        } else {
            Val::Bool(b[0].ok_or_else(|| NotVerified(format!("field '{canon}' is null")))?)
        };
        return Ok(Evald {
            val,
            path: Some(canon),
            iterates,
        });
    }
    let mut nums = Vec::with_capacity(vals.len());
    for v in &vals {
        match v {
            Some(serde_json::Value::Number(n)) => nums.push(n.as_f64()),
            None | Some(serde_json::Value::Null) => nums.push(None),
            Some(other) => {
                return Err(NotVerified(format!(
                    "field '{canon}' is not numeric (found {})",
                    kind_name(other)
                )))
            }
        }
    }
    let unit = unit_of(lookup, &canon);
    let val = if arrayed {
        Val::NumArr(nums, unit)
    } else {
        Val::Num(
            nums[0].ok_or_else(|| NotVerified(format!("field '{canon}' is null")))?,
            unit,
        )
    };
    Ok(Evald {
        val,
        path: Some(canon),
        iterates,
    })
}

fn kind_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

fn count_unit() -> UnitInfo {
    UnitInfo {
        text: Some("count".into()),
        parsed: parse_unit("count"),
        source: Some("the count of elements".into()),
        from_literal: false,
    }
}

fn apply(func: Func, e: Evald) -> Result<Evald, NotVerified> {
    let name = func.name();
    let (arr, unit) = match e.val {
        Val::NumArr(a, u) => (a, u),
        Val::Num(v, u) => match func {
            Func::Abs => {
                return Ok(Evald {
                    val: Val::Num(v.abs(), u),
                    ..e
                })
            }
            _ => (vec![Some(v)], u),
        },
        Val::BoolArr(b) if matches!(func, Func::Count | Func::Len) => {
            let n = if func == Func::Len {
                b.len()
            } else {
                b.iter().flatten().count()
            };
            return Ok(Evald {
                val: Val::Num(n as f64, count_unit()),
                path: e.path,
                iterates: vec![],
            });
        }
        _ => return Err(NotVerified(format!("'{name}' needs numeric values"))),
    };
    let present: Vec<f64> = arr.iter().flatten().copied().collect();
    let scalar = |v: Option<f64>| -> Result<f64, NotVerified> {
        v.ok_or_else(|| NotVerified(format!("'{name}' over no values")))
    };
    let v = match func {
        Func::Abs => {
            return Ok(Evald {
                val: Val::NumArr(arr.iter().map(|x| x.map(f64::abs)).collect(), unit),
                ..e
            })
        }
        Func::Max => scalar(present.iter().copied().reduce(f64::max))?,
        Func::Min => scalar(present.iter().copied().reduce(f64::min))?,
        Func::AbsMax => scalar(present.iter().map(|x| x.abs()).reduce(f64::max))?,
        Func::Mean => {
            if present.is_empty() {
                return Err(NotVerified(format!("'{name}' over no values")));
            }
            present.iter().sum::<f64>() / present.len() as f64
        }
        Func::Rms => {
            if present.is_empty() {
                return Err(NotVerified(format!("'{name}' over no values")));
            }
            (present.iter().map(|x| x * x).sum::<f64>() / present.len() as f64).sqrt()
        }
        Func::P95 => {
            if present.is_empty() {
                return Err(NotVerified(format!("'{name}' over no values")));
            }
            let mut s = present.clone();
            s.sort_by(|a, b| a.total_cmp(b));
            // Nearest-rank, the same definition the engine's timing_p95_ns uses.
            let rank = ((0.95 * s.len() as f64).ceil() as usize).clamp(1, s.len());
            s[rank - 1]
        }
        Func::Sum => present.iter().sum(),
        Func::First => scalar(arr.first().copied().flatten())?,
        Func::Last => scalar(arr.last().copied().flatten())?,
        Func::Count => {
            return Ok(Evald {
                val: Val::Num(present.len() as f64, count_unit()),
                path: e.path,
                iterates: vec![],
            })
        }
        Func::Len => {
            return Ok(Evald {
                val: Val::Num(arr.len() as f64, count_unit()),
                path: e.path,
                iterates: vec![],
            })
        }
        Func::Fraction => unreachable!("fraction is a predicate"),
    };
    Ok(Evald {
        val: Val::Num(v, unit),
        path: e.path,
        iterates: vec![],
    })
}

fn eval_operand(
    o: &Operand,
    doc: &serde_json::Value,
    lookup: &dyn UnitLookup,
    obs: &mut Vec<Observation>,
) -> Result<Evald, NotVerified> {
    match o {
        Operand::Number { value, unit } => {
            let info = match unit {
                Some(u) => UnitInfo {
                    parsed: Some(parse_unit(u).ok_or_else(|| {
                        NotVerified(format!("'{u}' in the criterion is not a unit"))
                    })?),
                    text: Some(u.clone()),
                    source: Some("criterion literal".into()),
                    from_literal: true,
                },
                None => UnitInfo {
                    text: None,
                    parsed: None,
                    source: None,
                    from_literal: true,
                },
            };
            Ok(Evald {
                val: Val::Num(*value, info),
                path: None,
                iterates: vec![],
            })
        }
        Operand::Bool(b) => Ok(Evald {
            val: Val::Bool(*b),
            path: None,
            iterates: vec![],
        }),
        Operand::Path(p) => eval_path(doc, lookup, p),
        // count/len of a path counts its elements whatever they hold (objects too).
        Operand::Call {
            func: func @ (Func::Count | Func::Len),
            arg,
        } if matches!(**arg, Operand::Path(_)) => {
            let Operand::Path(p) = &**arg else {
                unreachable!("guarded above")
            };
            let (vals, arrayed, canon, _) = resolve(doc, p)
                .ok_or_else(|| NotVerified(format!("field '{p}' is not in the result document")))?;
            if !arrayed {
                return Err(NotVerified(format!(
                    "'{}' needs an array; '{canon}' is a single value",
                    func.name()
                )));
            }
            let n = if *func == Func::Len {
                vals.len()
            } else {
                vals.iter()
                    .filter(|v| !matches!(v, None | Some(serde_json::Value::Null)))
                    .count()
            };
            Ok(Evald {
                val: Val::Num(n as f64, count_unit()),
                path: Some(canon),
                iterates: vec![],
            })
        }
        Operand::Call { func, arg } => {
            let inner = eval_operand(arg, doc, lookup, obs)?;
            apply(*func, inner)
        }
        Operand::Predicate { func, cmp } => {
            let (flags, path) = compare_elementwise(cmp, doc, lookup, obs)?;
            let present: Vec<bool> = flags.iter().flatten().copied().collect();
            if present.is_empty() {
                return Err(NotVerified(format!("'{}' over no values", func.name())));
            }
            let n_true = present.iter().filter(|b| **b).count();
            let (v, unit) = if *func == Func::Count {
                (n_true as f64, count_unit())
            } else {
                (
                    n_true as f64 / present.len() as f64,
                    UnitInfo {
                        text: Some("1".into()),
                        parsed: Some(Unit::one()),
                        source: Some("a fraction of elements".into()),
                        from_literal: false,
                    },
                )
            };
            obs.push(Observation {
                path: path.clone().unwrap_or_default(),
                value: serde_json::json!(v),
                unit: unit.text.clone(),
                unit_source: unit.source.clone(),
                detail: format!(
                    "{}: {n_true} of {} elements satisfy `{} {} {}`",
                    func.name(),
                    present.len(),
                    show(&cmp.lhs),
                    cmp.op.symbol(),
                    show(&cmp.rhs)
                ),
            });
            Ok(Evald {
                val: Val::Num(v, unit),
                path,
                iterates: vec![],
            })
        }
    }
}

fn show(o: &Operand) -> String {
    match o {
        Operand::Number { value, unit } => {
            let v = if *value != 0.0 && (value.abs() < 1e-3 || value.abs() >= 1e6) {
                format!("{value:e}")
            } else {
                format!("{value}")
            };
            match unit {
                Some(u) => format!("{v} {u}"),
                None => v,
            }
        }
        Operand::Bool(b) => b.to_string(),
        Operand::Path(p) => p.clone(),
        Operand::Call { func, arg } => format!("{}({})", func.name(), show(arg)),
        Operand::Predicate { func, cmp } => format!(
            "{}({} {} {})",
            func.name(),
            show(&cmp.lhs),
            cmp.op.symbol(),
            show(&cmp.rhs)
        ),
    }
}

/// Converts a right-hand value into the left-hand side's unit.
#[derive(Clone)]
struct Conv(Option<(Unit, Unit)>);

impl Conv {
    fn apply(&self, v: f64) -> f64 {
        match &self.0 {
            Some((from, to)) => from.convert(v, to).unwrap_or(f64::NAN),
            None => v,
        }
    }
}

/// Reconcile units of two numeric sides: returns the conversion of the RHS into the
/// LHS's unit, and the unit text the comparison is made in.
fn reconcile(l: &UnitInfo, r: &UnitInfo) -> Result<(Conv, Option<String>), NotVerified> {
    // A field side must state a parsable unit.
    for side in [l, r] {
        if !side.from_literal {
            match (&side.text, &side.parsed) {
                (None, _) => return Err(NotVerified(
                    "the field has no unit in the field-units schema or the result's units block"
                        .into(),
                )),
                (Some(t), None) => {
                    return Err(NotVerified(format!(
                        "the field's unit '{t}' is not a unit that can be checked"
                    )))
                }
                _ => {}
            }
        }
    }
    let is_bare_literal = |u: &UnitInfo| u.from_literal && u.text.is_none();
    match (is_bare_literal(l), is_bare_literal(r)) {
        (true, true) => Ok((Conv(None), None)),
        (false, true) | (true, false) => {
            let field = if is_bare_literal(r) { l } else { r };
            let pu = field.parsed.as_ref().expect("checked above");
            if pu.is_dimensionless() || pu.dims.keys().all(|k| k == "count") {
                Ok((Conv(None), field.text.clone()))
            } else {
                Err(NotVerified(format!(
                    "unit mismatch: the criterion's number has no unit but the field is in '{}'",
                    field.text.clone().unwrap_or_default()
                )))
            }
        }
        (false, false) => {
            let lu = l.parsed.as_ref().expect("parsed");
            let ru = r.parsed.as_ref().expect("parsed");
            if !lu.commensurable(ru) {
                return Err(NotVerified(format!(
                    "unit mismatch: '{}' ({}) cannot be compared with '{}' ({})",
                    l.text.clone().unwrap_or_default(),
                    lu.dimension_string(),
                    r.text.clone().unwrap_or_default(),
                    ru.dimension_string()
                )));
            }
            Ok((Conv(Some((ru.clone(), lu.clone()))), l.text.clone()))
        }
    }
}

/// Evaluate `cmp` element-wise; scalars broadcast. Returns per-element truth (None for
/// a null element) and the canonical path of the first field involved.
#[allow(clippy::type_complexity)]
fn compare_elementwise(
    cmp: &Comparison,
    doc: &serde_json::Value,
    lookup: &dyn UnitLookup,
    obs: &mut Vec<Observation>,
) -> Result<(Vec<Option<bool>>, Option<String>), NotVerified> {
    let (flags, path, _, _) = compare_full(cmp, doc, lookup, obs)?;
    Ok((flags, path))
}

/// Element-wise comparison with the numbers used, for evidence.
#[allow(clippy::type_complexity)]
fn compare_full(
    cmp: &Comparison,
    doc: &serde_json::Value,
    lookup: &dyn UnitLookup,
    obs: &mut Vec<Observation>,
) -> Result<
    (
        Vec<Option<bool>>,
        Option<String>,
        Vec<(Option<f64>, Option<f64>)>,
        (Evald, Evald, Option<String>),
    ),
    NotVerified,
> {
    let l = eval_operand(&cmp.lhs, doc, lookup, obs)?;
    let r = eval_operand(&cmp.rhs, doc, lookup, obs)?;
    let path = l.path.clone().or_else(|| r.path.clone());
    match (&l.val, &r.val) {
        (Val::Bool(_) | Val::BoolArr(_), Val::Bool(_) | Val::BoolArr(_)) => {
            if !matches!(cmp.op, CmpOp::Eq | CmpOp::Ne) {
                return Err(NotVerified("booleans compare only with == or !=".into()));
            }
            let la = bools(&l.val);
            let ra = bools(&r.val);
            let n = broadcast_len(la.len(), ra.len())?;
            let flags = (0..n)
                .map(|i| {
                    let a = la[if la.len() == 1 { 0 } else { i }];
                    let b = ra[if ra.len() == 1 { 0 } else { i }];
                    match (a, b) {
                        (Some(a), Some(b)) => Some((a == b) == (cmp.op == CmpOp::Eq)),
                        _ => None,
                    }
                })
                .collect();
            Ok((flags, path, vec![], (l, r, None)))
        }
        (Val::Num(_, lu) | Val::NumArr(_, lu), Val::Num(_, ru) | Val::NumArr(_, ru)) => {
            let (conv, unit) = reconcile(lu, ru)?;
            let la = nums(&l.val);
            let ra: Vec<Option<f64>> = nums(&r.val)
                .iter()
                .map(|x| x.map(|v| conv.apply(v)))
                .collect();
            let n = broadcast_len(la.len(), ra.len())?;
            let mut pairs = Vec::with_capacity(n);
            let flags = (0..n)
                .map(|i| {
                    let a = la[if la.len() == 1 { 0 } else { i }];
                    let b = ra[if ra.len() == 1 { 0 } else { i }];
                    pairs.push((a, b));
                    match (a, b) {
                        (Some(a), Some(b)) => Some(cmp.op.holds(a, b)),
                        _ => None,
                    }
                })
                .collect();
            Ok((flags, path, pairs, (l, r, unit)))
        }
        _ => Err(NotVerified(
            "a number cannot be compared with a boolean".into(),
        )),
    }
}

fn bools(v: &Val) -> Vec<Option<bool>> {
    match v {
        Val::Bool(b) => vec![Some(*b)],
        Val::BoolArr(a) => a.clone(),
        _ => vec![],
    }
}

fn nums(v: &Val) -> Vec<Option<f64>> {
    match v {
        Val::Num(x, _) => vec![Some(*x)],
        Val::NumArr(a, _) => a.clone(),
        _ => vec![],
    }
}

fn is_array(v: &Val) -> bool {
    matches!(v, Val::NumArr(..) | Val::BoolArr(_))
}

fn broadcast_len(a: usize, b: usize) -> Result<usize, NotVerified> {
    if a == b || b == 1 {
        Ok(a)
    } else if a == 1 {
        Ok(b)
    } else {
        Err(NotVerified(format!(
            "the two sides have different lengths ({a} and {b})"
        )))
    }
}

fn fmt_num(v: f64) -> String {
    if v == 0.0 || (v.abs() >= 1e-3 && v.abs() < 1e6) {
        let s = format!("{v:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        if s.is_empty() || s == "-" {
            "0".into()
        } else {
            s
        }
    } else {
        format!("{v:.6e}")
    }
}

fn eval_clause(
    cl: &Clause,
    doc: &serde_json::Value,
    lookup: &dyn UnitLookup,
    obs: &mut Vec<Observation>,
) -> Result<bool, NotVerified> {
    let (flags, _path, pairs, (l, r, unit)) = compare_full(&cl.cmp, doc, lookup, obs)?;
    let arrayed = is_array(&l.val) || is_array(&r.val);
    let unit_txt = unit
        .clone()
        .filter(|u| u != "1")
        .map(|u| format!(" {u}"))
        .unwrap_or_default();
    let field = |e: &Evald| -> (Option<String>, Option<String>, Option<String>) {
        match &e.val {
            Val::Num(_, u) | Val::NumArr(_, u) if !u.from_literal => {
                (e.path.clone(), u.text.clone(), u.source.clone())
            }
            _ => (e.path.clone(), None, None),
        }
    };
    let (fpath, funit, fsrc) = if l.path.is_some() {
        field(&l)
    } else {
        field(&r)
    };
    let text = format!(
        "`{} {} {}`",
        show(&cl.cmp.lhs),
        cl.cmp.op.symbol(),
        show(&cl.cmp.rhs)
    );
    match (&cl.quantifier, arrayed) {
        (None, true) => Err(NotVerified(format!(
            "{text} compares an array; reduce it with an aggregate such as max(...) or add `for all` / `for any`"
        ))),
        (Some(_), false) => Err(NotVerified(format!(
            "{text} has a quantifier but no operand is an array"
        ))),
        (None, false) => {
            let ok = flags[0].ok_or_else(|| NotVerified(format!("{text}: a value is null")))?;
            let value = match (&l.val, pairs.first()) {
                (Val::Bool(b), _) => serde_json::json!(b),
                (_, Some((Some(a), _))) => serde_json::json!(a),
                _ => serde_json::Value::Null,
            };
            let detail = match pairs.first() {
                Some((Some(a), Some(b))) => format!(
                    "{text}: {}{unit_txt} {} {}{unit_txt} is {}",
                    fmt_num(*a),
                    cl.cmp.op.symbol(),
                    fmt_num(*b),
                    if ok { "true" } else { "false" }
                ),
                _ => format!("{text} is {}", if ok { "true" } else { "false" }),
            };
            obs.push(Observation {
                path: fpath.unwrap_or_default(),
                value,
                unit: funit,
                unit_source: fsrc,
                detail,
            });
            Ok(ok)
        }
        (Some(q), true) => {
            if let Some(name) = &q.over {
                let iterated: Vec<&String> = l.iterates.iter().chain(r.iterates.iter()).collect();
                if !iterated.contains(&name) {
                    return Err(NotVerified(format!(
                        "{text} is quantified over '{name}' but no operand iterates '{name}'"
                    )));
                }
            }
            let present: Vec<(usize, bool)> = flags
                .iter()
                .enumerate()
                .filter_map(|(i, f)| f.map(|b| (i, b)))
                .collect();
            if present.is_empty() {
                return Err(NotVerified(format!("{text}: every element is null")));
            }
            let nulls = flags.len() - present.len();
            let n_ok = present.iter().filter(|(_, b)| *b).count();
            let ok = if q.all {
                n_ok == present.len()
            } else {
                n_ok > 0
            };
            // Worst element: the first failing one for `all`, the first passing one for
            // `any`; otherwise the element closest to the limit.
            let pick = if q.all {
                present.iter().find(|(_, b)| !*b).map(|(i, _)| *i)
            } else {
                present.iter().find(|(_, b)| *b).map(|(i, _)| *i)
            };
            let idx = pick.or_else(|| {
                pairs
                    .iter()
                    .enumerate()
                    .filter_map(|(i, (a, b))| Some((i, (*a)? - (*b)?)))
                    .min_by(|x, y| {
                        let sx = match cl.cmp.op {
                            CmpOp::Lt | CmpOp::Le => -x.1,
                            _ => x.1,
                        };
                        let sy = match cl.cmp.op {
                            CmpOp::Lt | CmpOp::Le => -y.1,
                            _ => y.1,
                        };
                        sx.total_cmp(&sy)
                    })
                    .map(|(i, _)| i)
            });
            let (value, at) = match idx.and_then(|i| pairs.get(i).map(|p| (i, *p))) {
                Some((i, (Some(a), Some(b)))) => (
                    serde_json::json!(a),
                    format!(
                        "; element {i}: {}{unit_txt} {} {}{unit_txt}",
                        fmt_num(a),
                        cl.cmp.op.symbol(),
                        fmt_num(b)
                    ),
                ),
                _ => (serde_json::json!(n_ok), String::new()),
            };
            let label = if pick.is_some() {
                if q.all {
                    " (first failing)"
                } else {
                    " (first passing)"
                }
            } else {
                " (closest to the limit)"
            };
            obs.push(Observation {
                path: fpath.unwrap_or_default(),
                value,
                unit: funit,
                unit_source: fsrc,
                detail: format!(
                    "{text} for {}: {n_ok} of {} elements satisfy{}{}{}",
                    if q.all { "all" } else { "any" },
                    present.len(),
                    if nulls > 0 {
                        format!(" ({nulls} null elements skipped)")
                    } else {
                        String::new()
                    },
                    if at.is_empty() { "" } else { label },
                    at
                ),
            });
            Ok(ok)
        }
    }
}

/// Evaluate a parsed criterion against a result document.
pub fn evaluate(c: &Criterion, doc: &serde_json::Value, lookup: &dyn UnitLookup) -> Evaluation {
    let mut obs = Vec::new();
    let mut not_met = None;
    let mut not_verified = None;
    for (i, cl) in c.clauses.iter().enumerate() {
        match eval_clause(cl, doc, lookup, &mut obs) {
            Ok(true) => {}
            Ok(false) => {
                if not_met.is_none() {
                    not_met = Some(format!("clause {} is false", i + 1));
                }
            }
            Err(NotVerified(msg)) => {
                if not_verified.is_none() {
                    not_verified = Some(msg);
                }
            }
        }
    }
    // A clause shown false is a finding even when another clause could not be checked.
    let (verdict, reason) = if let Some(r) = not_met {
        (Verdict::NotMet, Some(r))
    } else if let Some(r) = not_verified {
        (Verdict::NotVerified, Some(r))
    } else {
        (Verdict::Met, None)
    };
    Evaluation {
        verdict,
        reason,
        observations: obs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Units(Vec<(&'static str, &'static str)>);
    impl UnitLookup for Units {
        fn unit_for(&self, p: &str) -> Option<(String, String)> {
            self.0
                .iter()
                .find(|(k, _)| *k == p)
                .map(|(_, u)| (u.to_string(), "test".to_string()))
        }
    }

    fn doc() -> serde_json::Value {
        json!({
            "al_h_m": 40.0,
            "fom": {"p95_ns": 19.7, "ok": true},
            "epochs": [
                {"hpl_m": 10.0, "available": true},
                {"hpl_m": 45.0, "available": false},
                {"hpl_m": 20.0, "available": true},
                {"hpl_m": null, "available": true}
            ],
            "series": [1.0, -3.0, 2.0]
        })
    }

    fn units() -> Units {
        Units(vec![
            ("al_h_m", "m"),
            ("fom.p95_ns", "ns"),
            ("epochs[].hpl_m", "m"),
            ("series[]", "ns"),
        ])
    }

    fn run(src: &str) -> Evaluation {
        evaluate(&parse(src).expect("parses"), &doc(), &units())
    }

    #[test]
    fn scalar_with_unit_conversion() {
        assert_eq!(run("fom.p95_ns <= 20 ns").verdict, Verdict::Met);
        assert_eq!(run("fom.p95_ns <= 0.02 us").verdict, Verdict::Met);
        assert_eq!(run("fom.p95_ns < 19 ns").verdict, Verdict::NotMet);
    }

    #[test]
    fn unit_mismatch_is_not_verified() {
        let e = run("fom.p95_ns <= 20 m");
        assert_eq!(e.verdict, Verdict::NotVerified);
        assert!(e.reason.unwrap().contains("unit mismatch"));
        let e = run("fom.p95_ns <= 20");
        assert_eq!(e.verdict, Verdict::NotVerified);
        let e = run("al_h_m > fom.p95_ns");
        assert!(e.reason.unwrap().contains("unit mismatch"));
    }

    #[test]
    fn quantifier_over_array_with_nulls() {
        let e = run("epochs.hpl_m < al_h_m for all epochs");
        assert_eq!(e.verdict, Verdict::NotMet);
        assert!(
            e.observations[0].detail.contains("2 of 3"),
            "{:?}",
            e.observations
        );
        assert!(e.observations[0].detail.contains("1 null"));
        assert_eq!(e.observations[0].path, "epochs[].hpl_m");
        assert_eq!(run("epochs[].hpl_m < 50 m for all").verdict, Verdict::Met);
        assert_eq!(
            run("epochs.hpl_m > 40 m for any epochs").verdict,
            Verdict::Met
        );
    }

    #[test]
    fn array_without_quantifier_is_not_verified() {
        let e = run("epochs.hpl_m < 50 m");
        assert_eq!(e.verdict, Verdict::NotVerified);
        let e = run("epochs.hpl_m < 50 m for all satellites");
        assert!(e.reason.unwrap().contains("no operand iterates"));
    }

    #[test]
    fn aggregates_and_suffix_form() {
        assert_eq!(run("max(series) <= 2 ns").verdict, Verdict::Met);
        assert_eq!(run("abs_max(series) <= 2 ns").verdict, Verdict::NotMet);
        assert_eq!(run("series.max <= 2 ns").verdict, Verdict::Met);
        assert_eq!(run("count(epochs.hpl_m) == 3").verdict, Verdict::Met);
        assert_eq!(run("len(epochs.hpl_m) == 4").verdict, Verdict::Met);
        assert_eq!(
            run("fraction(epochs.available == true) >= 75 %").verdict,
            Verdict::Met
        );
        assert_eq!(run("abs(series) <= 3 ns for all").verdict, Verdict::Met);
        assert_eq!(run("rms(series) < 2.2 ns").verdict, Verdict::Met);
    }

    #[test]
    fn booleans_and_conjunction() {
        assert_eq!(run("fom.ok == true").verdict, Verdict::Met);
        assert_eq!(
            run("fom.ok == true and fom.p95_ns < 1 ns").verdict,
            Verdict::NotMet
        );
        assert_eq!(run("fom.ok < true").verdict, Verdict::NotVerified);
    }

    #[test]
    fn missing_field_and_missing_unit() {
        assert_eq!(run("fom.nope <= 1 ns").verdict, Verdict::NotVerified);
        let d = json!({"x": 1.0});
        let e = evaluate(&parse("x <= 2 m").unwrap(), &d, &Units(vec![]));
        assert!(e.reason.unwrap().contains("no unit"));
    }

    #[test]
    fn parse_errors_are_plain() {
        assert!(parse("").is_err());
        assert!(parse("x <").is_err());
        assert!(parse("wibble(x) < 1")
            .unwrap_err()
            .contains("unknown function"));
        assert!(parse("x < 1 for some").is_err());
        assert!(parse("fraction(x) > 1").is_err());
    }
}
