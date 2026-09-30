// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! SysML v2 (Systems Modeling Language, version 2, OMG) textual notation: export of a
//! trace, a checker for the grammar subset the export uses, and import of requirement
//! definitions and verify relationships written in that subset.
//!
//! **The subset.** The productions below are transcribed from the SysML v2 textual
//! notation (OMG SysML v2.0, clause 8.2, and the KerML lexical structure of clause 8.2
//! of KerML v1.0) and cover exactly what the export writes:
//!
//! ```text
//! Model            := PackageMember*
//! PackageMember    := Visibility? ( Package | Import | Doc | Comment | Definition
//!                                 | Usage | Satisfy )
//! Package          := 'package' Identification ( ';' | '{' PackageMember* '}' )
//! Import           := 'import' QualifiedName ( '::' ( '*' | '**' ) )? ';'
//! Identification   := ( '<' Name '>' )? Name?
//! Definition       := ( 'part' | 'requirement' | 'verification' | 'attribute' ) 'def'
//!                     Identification ( ':>' QualifiedName )? Body
//! Usage            := AttributeUsage | PartUsage | RequirementUsage | VerificationUsage
//! AttributeUsage   := 'attribute' ( ':>>' Name | Identification ) ( ':' QualifiedName )?
//!                     ( '=' Expression )? Body
//! PartUsage        := 'part' Identification ( ':' QualifiedName )? Body
//! RequirementUsage := 'requirement' Identification ( ':' QualifiedName )? Body
//! VerificationUsage:= 'verification' Identification ( ':' QualifiedName )? Body
//! Body             := ';' | '{' BodyItem* '}'
//! BodyItem         := Doc | Comment | Usage | Subject | Objective
//! Subject          := 'subject' Name ( ':' QualifiedName )? ';'
//! Objective        := 'objective' Name? '{' ( Doc | Comment | Verify )* '}'
//! Verify           := 'verify' 'requirement'? QualifiedName ( ';' | '{' ( Doc | Comment )* '}' )
//! Satisfy          := 'satisfy' 'requirement'? QualifiedName 'by' QualifiedName
//!                     ( ';' | '{' ( Doc | Comment )* '}' )
//! Doc              := 'doc' REGULAR_COMMENT
//! Comment          := REGULAR_COMMENT
//! Expression       := '-'? ( DECIMAL_VALUE | RealValue ) ( '[' UnitExpr ']' )?
//!                   | STRING_VALUE | 'true' | 'false'
//! UnitExpr         := QualifiedName ( ( '*' | '/' ) QualifiedName )*
//! QualifiedName    := Name ( '::' Name )*
//! Name             := ID | UNRESTRICTED_NAME          (an ID may not be a keyword)
//! RealValue        := DECIMAL_VALUE? '.' ( DECIMAL_VALUE | EXP_VALUE ) | EXP_VALUE
//! ```
//!
//! Beyond the grammar the checker resolves every qualified name used for typing,
//! satisfy, verify and units: to an element declared in the file, or to one of the
//! standard-library names the export uses (`ScalarValues::Real`, `String`, `Integer`,
//! `Boolean`; `ISQ::DurationValue`, `LengthValue`, `FrequencyValue`, `SpeedValue`;
//! `SI::s`, `SI::m`, `SI::Hz`). It is not the OMG pilot implementation and does not
//! check the whole language.

use std::collections::BTreeMap;

use super::evidence::Run;
use super::requirements::{Method, Requirement};
use super::trace::{Trace, Vcrm};
use super::units::parse_unit;
use kshana::api::KshanaError;

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(format!("SysML: {}", msg.into()))
}

/// Reserved words of the SysML v2 textual notation. An ID that is one of these must
/// be written as an unrestricted (quoted) name.
const KEYWORDS: &[&str] = &[
    "about",
    "abstract",
    "accept",
    "action",
    "actor",
    "after",
    "alias",
    "all",
    "allocate",
    "allocation",
    "analysis",
    "and",
    "as",
    "assert",
    "assign",
    "assume",
    "at",
    "attribute",
    "bind",
    "binding",
    "by",
    "calc",
    "case",
    "chains",
    "comment",
    "concern",
    "connect",
    "connection",
    "constant",
    "constraint",
    "crosses",
    "decide",
    "def",
    "default",
    "defined",
    "dependency",
    "derived",
    "do",
    "doc",
    "else",
    "end",
    "entry",
    "enum",
    "event",
    "exhibit",
    "exit",
    "expose",
    "false",
    "filter",
    "first",
    "flow",
    "for",
    "fork",
    "frame",
    "from",
    "hastype",
    "if",
    "implies",
    "import",
    "in",
    "include",
    "individual",
    "inout",
    "interface",
    "istype",
    "item",
    "join",
    "language",
    "library",
    "locale",
    "loop",
    "merge",
    "message",
    "meta",
    "metadata",
    "new",
    "nonunique",
    "not",
    "null",
    "objective",
    "occurrence",
    "of",
    "or",
    "ordered",
    "out",
    "package",
    "parallel",
    "part",
    "perform",
    "port",
    "private",
    "protected",
    "public",
    "redefines",
    "ref",
    "references",
    "render",
    "rendering",
    "rep",
    "require",
    "requirement",
    "return",
    "satisfy",
    "send",
    "snapshot",
    "specializes",
    "stakeholder",
    "standard",
    "state",
    "subject",
    "subsets",
    "succession",
    "terminate",
    "then",
    "timeslice",
    "transition",
    "true",
    "until",
    "use",
    "variant",
    "variation",
    "verification",
    "verify",
    "via",
    "view",
    "viewpoint",
    "when",
    "while",
    "xor",
];

/// Standard-library names the export refers to.
const LIBRARY: &[&str] = &[
    "ScalarValues::Real",
    "ScalarValues::String",
    "ScalarValues::Integer",
    "ScalarValues::Boolean",
    "ISQ::DurationValue",
    "ISQ::LengthValue",
    "ISQ::FrequencyValue",
    "ISQ::SpeedValue",
    "SI::s",
    "SI::m",
    "SI::Hz",
];

// ------------------------------------------------------------------ writing helpers

/// A name as SysML writes it: a basic ID when it is one and not a keyword, else an
/// unrestricted name in single quotes.
pub fn name(s: &str) -> String {
    let basic = s
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if basic && !KEYWORDS.contains(&s) {
        s.to_string()
    } else {
        let mut o = String::from("'");
        for c in s.chars() {
            match c {
                '\'' => o.push_str("\\'"),
                '\\' => o.push_str("\\\\"),
                '\n' => o.push_str("\\n"),
                _ => o.push(c),
            }
        }
        o.push('\'');
        o
    }
}

fn string_lit(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            _ => o.push(c),
        }
    }
    o.push('"');
    o
}

fn doc(s: &str, indent: &str) -> String {
    // A comment body may not contain its own terminator.
    let body = s.replace("*/", "* /");
    format!("{indent}doc /* {body} */\n")
}

/// A real literal in the SysML lexical form (always with a '.', exponent `E`).
pub fn real(v: f64) -> String {
    if !v.is_finite() {
        return "0.0".into();
    }
    if v != 0.0 && (v.abs() < 1e-4 || v.abs() >= 1e15) {
        let s = format!("{v:E}");
        let (m, e) = s.split_once('E').expect("exponent form");
        let m = if m.contains('.') {
            m.to_string()
        } else {
            format!("{m}.0")
        };
        format!("{m}E{e}")
    } else {
        let s = format!("{v}");
        if s.contains('.') || s.contains('E') || s.contains('e') {
            s
        } else {
            format!("{s}.0")
        }
    }
}

// ------------------------------------------------------------------ systems

/// A system part extracted from a scenario.
#[derive(Debug, Clone)]
struct SystemPart {
    def: &'static str,
    instance: String,
    attrs: Vec<SysAttr>,
}

#[derive(Debug, Clone)]
struct SysAttr {
    name: String,
    value: AttrValue,
    unit: Option<String>,
    unit_source: String,
}

#[derive(Debug, Clone)]
enum AttrValue {
    Num(f64),
    Int(i64),
    Str(String),
    Bool(bool),
}

fn classify(path: &str) -> Option<&'static str> {
    let last = path
        .rsplit('.')
        .next()
        .unwrap_or(path)
        .split('[')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let p = last.as_str();
    if p.contains("clock") {
        Some("Clock")
    } else if p.contains("jammer") || p.contains("jamming") || p.contains("interferer") {
        Some("Jammer")
    } else if p.contains("spoof") {
        Some("Spoofer")
    } else if p.contains("constellation")
        || p.contains("shell")
        || p.contains("satellite")
        || p == "gnss"
        || p == "system"
    {
        Some("Constellation")
    } else if p.contains("receiver") || p == "user" || p.contains("antenna") || p == "site" {
        Some("Receiver")
    } else {
        None
    }
}

fn suffix_unit(key: &str) -> Option<&'static str> {
    let k = key.to_ascii_lowercase();
    let table: &[(&str, &str)] = &[
        ("_dbhz", "dB-Hz"),
        ("_dbw", "dBW"),
        ("_dbm", "dBm"),
        ("_dbi", "dBi"),
        ("_db", "dB"),
        ("_ns", "ns"),
        ("_us", "us"),
        ("_ms", "ms"),
        ("_min", "min"),
        ("_km", "km"),
        ("_mm", "mm"),
        ("_cm", "cm"),
        ("_ghz", "GHz"),
        ("_mhz", "MHz"),
        ("_khz", "kHz"),
        ("_hz", "Hz"),
        ("_deg", "deg"),
        ("_rad", "rad"),
        ("_kg", "kg"),
        ("_m", "m"),
        ("_s", "s"),
        ("_h", "h"),
        ("_w", "W"),
        ("_k", "K"),
    ];
    table
        .iter()
        .find(|(suf, _)| k.ends_with(suf))
        .map(|(_, u)| *u)
}

/// The unit of a scenario key: the field-units schema entry whose last path segment is
/// the key, when the schema gives it one unit; else the key's unit suffix.
fn unit_for_key(run: &Run, trace: &Trace, key: &str) -> (Option<String>, String) {
    if let (Some(schema), Some(kind)) = (&trace.schema, &run.kind) {
        if let Some(fields) = schema
            .doc
            .get("kinds")
            .and_then(|k| k.get(kind))
            .and_then(|k| k.get("fields"))
            .and_then(|f| f.as_object())
        {
            let mut found: Vec<(&String, &str)> = fields
                .iter()
                .filter(|(p, v)| {
                    p.rsplit('.').next() == Some(key)
                        && matches!(
                            v.get("provenance").and_then(|x| x.as_str()),
                            Some("input" | "modelled-input")
                        )
                })
                .filter_map(|(p, v)| v.get("unit").and_then(|u| u.as_str()).map(|u| (p, u)))
                .collect();
            found.sort();
            let first = found.first().map(|(_, u)| *u);
            if let Some(u) = first {
                if found.iter().all(|(_, x)| *x == u) {
                    return (
                        Some(u.to_string()),
                        format!("field-units schema kinds.{kind}.fields.\"{}\"", found[0].0),
                    );
                }
            }
        }
    }
    // A composite run (a campaign) nests scenarios of other kinds: look the key up
    // across every kind, and use it when the schema gives it one unit everywhere.
    if let Some(kinds) = trace
        .schema
        .as_ref()
        .and_then(|s| s.doc.get("kinds"))
        .and_then(|k| k.as_object())
    {
        let mut found: Vec<(String, String)> = Vec::new();
        for (kind, kv) in kinds {
            if let Some(fields) = kv.get("fields").and_then(|f| f.as_object()) {
                for (p, v) in fields {
                    if p.rsplit('.').next() == Some(key)
                        && matches!(
                            v.get("provenance").and_then(|x| x.as_str()),
                            Some("input" | "modelled-input")
                        )
                    {
                        if let Some(u) = v.get("unit").and_then(|u| u.as_str()) {
                            found.push((format!("kinds.{kind}.fields.\"{p}\""), u.to_string()));
                        }
                    }
                }
            }
        }
        found.sort();
        if let Some((src, u)) = found.first() {
            if found.iter().all(|(_, x)| x == u) {
                return (Some(u.clone()), format!("field-units schema {src}"));
            }
        }
    }
    match suffix_unit(key) {
        Some(u) => (
            Some(u.to_string()),
            format!(
                "the key's `_{}` suffix",
                key.rsplit('_').next().unwrap_or("")
            ),
        ),
        None => (
            None,
            "no unit stated (dimensionless or not given)".to_string(),
        ),
    }
}

fn collect_parts(run: &Run, trace: &Trace) -> Vec<SystemPart> {
    let Some(src) = &run.scenario_src else {
        return vec![];
    };
    let Ok(root) = toml::from_str::<toml::Value>(src) else {
        return vec![];
    };
    let mut out = Vec::new();
    fn walk(v: &toml::Value, path: String, out: &mut Vec<(String, toml::value::Table)>) {
        match v {
            toml::Value::Table(t) => {
                if !path.is_empty() {
                    out.push((path.clone(), t.clone()));
                }
                for (k, x) in t {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    walk(x, p, out);
                }
            }
            toml::Value::Array(a) if a.iter().all(|x| x.is_table()) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, format!("{path}[{i}]"), out);
                }
            }
            _ => {}
        }
    }
    let mut tables = Vec::new();
    walk(&root, String::new(), &mut tables);
    for (path, t) in tables {
        let Some(def) = classify(&path) else {
            continue;
        };
        let mut attrs = Vec::new();
        for (k, v) in &t {
            let value = match v {
                // SysML v2 has no literal for NaN or infinity; writing one as a number
                // would state a value the scenario does not have, so it is left out.
                toml::Value::Float(f) if f.is_finite() => AttrValue::Num(*f),
                toml::Value::Integer(i) => AttrValue::Int(*i),
                toml::Value::String(s) => AttrValue::Str(s.clone()),
                toml::Value::Boolean(b) => AttrValue::Bool(*b),
                _ => continue,
            };
            let (unit, unit_source) = match value {
                AttrValue::Num(_) | AttrValue::Int(_) => unit_for_key(run, trace, k),
                _ => (None, String::new()),
            };
            attrs.push(SysAttr {
                name: k.clone(),
                value,
                unit,
                unit_source,
            });
        }
        if attrs.is_empty() {
            continue;
        }
        out.push(SystemPart {
            def,
            instance: path,
            attrs,
        });
    }
    out
}

/// The SysML type and SI unit for a unit string: an ISQ quantity for the four
/// dimensions the export maps, otherwise `Real`.
fn isq(unit: Option<&str>) -> Option<(&'static str, &'static str, crate::mbse::units::Unit)> {
    // Only a plain time, length, frequency or speed spelling maps to an ISQ quantity; a
    // compound that merely reduces to one (a noise intensity in s^2/s) stays Real.
    const SIMPLE: &[&str] = &[
        "s", "ms", "us", "ns", "ps", "min", "h", "m", "km", "cm", "mm", "Hz", "kHz", "MHz", "GHz",
        "m/s", "km/s",
    ];
    let raw = unit?;
    if !SIMPLE.contains(&raw) {
        return None;
    }
    let u = parse_unit(raw)?;
    let dims = u.dimension_string();
    let (ty, si, base) = match dims.as_str() {
        "s" => ("ISQ::DurationValue", "SI::s", "s"),
        "m" => ("ISQ::LengthValue", "SI::m", "m"),
        "1/s" => ("ISQ::FrequencyValue", "SI::Hz", "Hz"),
        "m/s" => ("ISQ::SpeedValue", "SI::m / SI::s", "m/s"),
        _ => return None,
    };
    Some((ty, si, parse_unit(base).expect("base unit parses")))
}

// ------------------------------------------------------------------ export

/// Export a trace (and its evaluated matrix, for the verdicts) as SysML v2 text.
pub fn export(trace: &Trace, vcrm: &Vcrm) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "// Generated by Kshana Pro {} (MBSE) from {} (SHA-256 {}).\n// Units are read from the open engine's field-units schema; values from the scenario files the runs used.\n",
        crate::version(),
        trace.trace_file.path,
        trace.trace_file.sha256
    ));
    s.push_str(&format!("package {} {{\n", name(&trace.title)));
    s.push_str(&doc(
        "Requirements, the systems each Kshana run simulated, and the satisfy and verify relationships between them. Acceptance criteria are machine-checkable expressions over open Kshana engine result documents.",
        "    ",
    ));

    // Systems: one part def per category, the union of its attributes.
    let per_run: Vec<(&Run, Vec<SystemPart>)> = trace
        .runs
        .iter()
        .map(|r| (r, collect_parts(r, trace)))
        .collect();
    let mut defs: BTreeMap<&'static str, BTreeMap<String, (String, String)>> = BTreeMap::new();
    for (_, parts) in &per_run {
        for p in parts {
            let d = defs.entry(p.def).or_default();
            for a in &p.attrs {
                let (ty, note) = match &a.value {
                    AttrValue::Str(_) => ("ScalarValues::String".to_string(), String::new()),
                    AttrValue::Bool(_) => ("ScalarValues::Boolean".to_string(), String::new()),
                    AttrValue::Int(_) if a.unit.is_none() || a.unit.as_deref() == Some("count") => {
                        ("ScalarValues::Integer".to_string(), String::new())
                    }
                    AttrValue::Num(_) | AttrValue::Int(_) => match isq(a.unit.as_deref()) {
                        Some((ty, _, _)) => (
                            ty.to_string(),
                            format!(
                                "unit {} ({}); values are written in coherent SI units",
                                a.unit.clone().unwrap_or_default(),
                                a.unit_source
                            ),
                        ),
                        None => (
                            "ScalarValues::Real".to_string(),
                            match &a.unit {
                                Some(u) => format!("unit {u} ({})", a.unit_source),
                                None => a.unit_source.clone(),
                            },
                        ),
                    },
                };
                // First writer wins; a later clash in type falls back to Real.
                d.entry(a.name.clone())
                    .and_modify(|e| {
                        if e.0 != ty {
                            *e = (
                                "ScalarValues::Real".into(),
                                "mixed units across scenarios".into(),
                            );
                        }
                    })
                    .or_insert((ty, note));
            }
        }
    }
    s.push_str("    package Systems {\n");
    for (def, attrs) in &defs {
        s.push_str(&format!("        part def {def} {{\n"));
        for (an, (ty, note)) in attrs {
            if note.is_empty() {
                s.push_str(&format!("            attribute {} : {ty};\n", name(an)));
            } else {
                s.push_str(&format!("            attribute {} : {ty} {{\n", name(an)));
                s.push_str(&doc(note, "                "));
                s.push_str("            }\n");
            }
        }
        s.push_str("        }\n");
    }
    s.push_str("    }\n");

    // Scenarios: one part per run, holding the system instances it simulated.
    s.push_str("    package Scenarios {\n");
    for (run, parts) in &per_run {
        s.push_str(&format!("        part {} {{\n", name(&run.id)));
        s.push_str(&doc(
            &format!(
                "Kshana run {}: kind {}, scenario {}, scenario_hash {}, seed {}, engine {}.",
                run.id,
                run.kind.clone().unwrap_or_else(|| "unknown".into()),
                run.scenario_file
                    .as_ref()
                    .map(|f| f.path.as_str())
                    .unwrap_or("not given"),
                run.stamp
                    .scenario_hash
                    .clone()
                    .unwrap_or_else(|| "not stamped".into()),
                run.stamp
                    .seed
                    .map(|x| x.to_string())
                    .unwrap_or_else(|| "not recorded".into()),
                run.stamp
                    .engine_version
                    .clone()
                    .unwrap_or_else(|| "not recorded".into())
            ),
            "            ",
        ));
        for p in parts {
            let ty_attrs = defs.get(p.def);
            s.push_str(&format!(
                "            part {} : Systems::{} {{\n",
                name(&p.instance),
                p.def
            ));
            for a in &p.attrs {
                let ty = ty_attrs
                    .and_then(|m| m.get(&a.name))
                    .map(|x| x.0.as_str())
                    .unwrap_or("ScalarValues::Real");
                let numeric = match a.value {
                    AttrValue::Int(i) => Some(i as f64),
                    AttrValue::Num(f) => Some(f),
                    _ => None,
                };
                let v = match (&a.value, numeric) {
                    (AttrValue::Str(x), _) => string_lit(x),
                    (AttrValue::Bool(b), _) => b.to_string(),
                    (_, Some(raw)) if ty.starts_with("ISQ::") => match isq(a.unit.as_deref()) {
                        Some((_, si, base)) => {
                            let from = parse_unit(a.unit.as_deref().unwrap_or("1"))
                                .expect("isq() parsed it");
                            let v = from.convert(raw, &base).unwrap_or(raw);
                            format!("{} [{si}]", real(v))
                        }
                        None => real(raw),
                    },
                    (AttrValue::Int(i), _) if ty == "ScalarValues::Integer" => i.to_string(),
                    (_, Some(raw)) => real(raw),
                    (_, None) => {
                        unreachable!("every attribute value is a string, boolean or number")
                    }
                };
                s.push_str(&format!(
                    "                attribute :>> {} = {v};\n",
                    name(&a.name)
                ));
            }
            s.push_str("            }\n");
        }
        s.push_str("        }\n");
    }
    s.push_str("    }\n");

    // Requirement definitions and usages.
    s.push_str("    package RequirementDefinitions {\n");
    for r in &trace.requirements {
        s.push_str(&format!(
            "        requirement def <{}> {} {{\n",
            name(&r.id),
            name(&r.id)
        ));
        s.push_str(&doc(&r.text, "            "));
        s.push_str(&format!(
            "            attribute verificationMethod : ScalarValues::String = {};\n",
            string_lit(r.method.as_str())
        ));
        if let Some(c) = &r.criterion {
            s.push_str(&format!(
                "            attribute acceptanceCriterion : ScalarValues::String = {};\n",
                string_lit(c)
            ));
        }
        if !r.capabilities.is_empty() {
            s.push_str(&format!(
                "            attribute kshanaCapabilities : ScalarValues::String = {};\n",
                string_lit(&r.capabilities.join(";"))
            ));
        }
        s.push_str("        }\n");
    }
    s.push_str("    }\n    package Requirements {\n");
    for r in &trace.requirements {
        s.push_str(&format!(
            "        requirement {} : RequirementDefinitions::{};\n",
            name(&r.id),
            name(&r.id)
        ));
    }
    s.push_str("    }\n");

    // Verification: one verification case per run, verifying its linked requirements.
    s.push_str("    package Verification {\n");
    s.push_str("        verification def KshanaRun {\n");
    s.push_str(&doc(
        "A run of the open Kshana engine, identified by its reproducibility stamp.",
        "            ",
    ));
    for (an, ty) in [
        ("scenarioHash", "ScalarValues::String"),
        ("seed", "ScalarValues::Integer"),
        ("engineVersion", "ScalarValues::String"),
        ("schemaVersion", "ScalarValues::String"),
        ("scenarioSha256", "ScalarValues::String"),
    ] {
        s.push_str(&format!("            attribute {an} : {ty};\n"));
    }
    s.push_str("        }\n");
    for run in &trace.runs {
        s.push_str(&format!(
            "        verification {} : KshanaRun {{\n",
            name(&run.id)
        ));
        let st = &run.stamp;
        if let Some(h) = &st.scenario_hash {
            s.push_str(&format!(
                "            attribute :>> scenarioHash = {};\n",
                string_lit(h)
            ));
        }
        if let Some(x) = st.seed {
            s.push_str(&format!("            attribute :>> seed = {x};\n"));
        }
        if let Some(x) = &st.engine_version {
            s.push_str(&format!(
                "            attribute :>> engineVersion = {};\n",
                string_lit(x)
            ));
        }
        if let Some(x) = &st.schema_version {
            s.push_str(&format!(
                "            attribute :>> schemaVersion = {};\n",
                string_lit(x)
            ));
        }
        if let Some(x) = &st.scenario_sha256_actual {
            s.push_str(&format!(
                "            attribute :>> scenarioSha256 = {};\n",
                string_lit(x)
            ));
        }
        let verified: Vec<(&str, String)> = vcrm
            .rows
            .iter()
            .flat_map(|row| {
                row.cells.iter().filter(|c| c.run == run.id).map(move |c| {
                    (
                        row.requirement.as_str(),
                        format!(
                            "Kshana verdict: {} on a {} basis.{}",
                            c.verdict.as_str(),
                            c.basis.label,
                            c.reason
                                .as_ref()
                                .map(|r| format!(" {r}"))
                                .unwrap_or_default()
                        ),
                    )
                })
            })
            .collect();
        if !verified.is_empty() {
            s.push_str("            objective {\n");
            for (req, note) in verified {
                s.push_str(&format!(
                    "                verify Requirements::{} {{\n",
                    name(req)
                ));
                s.push_str(&doc(&note, "                    "));
                s.push_str("                }\n");
            }
            s.push_str("            }\n");
        }
        s.push_str("        }\n");
    }
    s.push_str("    }\n");

    // Satisfy: the system a run simulated satisfies the requirement, asserted only
    // where the run met it; otherwise a comment says why not.
    for row in &vcrm.rows {
        for c in &row.cells {
            if c.verdict == super::criterion::Verdict::Met {
                s.push_str(&format!(
                    "    satisfy Requirements::{} by Scenarios::{};\n",
                    name(&row.requirement),
                    name(&c.run)
                ));
            } else {
                s.push_str(&format!(
                    "    /* No satisfy relationship: {} is {} by {}. */\n",
                    row.requirement.replace("*/", "* /"),
                    c.verdict.as_str(),
                    c.run.replace("*/", "* /")
                ));
            }
        }
    }
    s.push_str("}\n");
    s
}

/// Export a requirements set alone (no runs) as SysML v2 text: the requirement
/// definitions and one requirement usage for each.
pub fn export_requirements(title: &str, reqs: &[Requirement]) -> String {
    let mut s = format!(
        "// Generated by Kshana Pro {} (MBSE): requirements only.\npackage {} {{\n",
        crate::version(),
        name(title)
    );
    s.push_str("    package RequirementDefinitions {\n");
    for r in reqs {
        s.push_str(&format!(
            "        requirement def <{}> {} {{\n",
            name(&r.id),
            name(&r.id)
        ));
        s.push_str(&doc(&r.text, "            "));
        s.push_str(&format!(
            "            attribute verificationMethod : ScalarValues::String = {};\n",
            string_lit(r.method.as_str())
        ));
        if let Some(c) = &r.criterion {
            s.push_str(&format!(
                "            attribute acceptanceCriterion : ScalarValues::String = {};\n",
                string_lit(c)
            ));
        }
        if !r.capabilities.is_empty() {
            s.push_str(&format!(
                "            attribute kshanaCapabilities : ScalarValues::String = {};\n",
                string_lit(&r.capabilities.join(";"))
            ));
        }
        s.push_str("        }\n");
    }
    s.push_str("    }\n    package Requirements {\n");
    for r in reqs {
        s.push_str(&format!(
            "        requirement {} : RequirementDefinitions::{};\n",
            name(&r.id),
            name(&r.id)
        ));
    }
    s.push_str("    }\n}\n");
    s
}

// ------------------------------------------------------------------ lexer

#[derive(Debug, Clone, PartialEq)]
enum T {
    Id(String),
    Quoted(String),
    Str(String),
    Num(String),
    Sym(&'static str),
    Comment(String),
}

fn lex(src: &str) -> Result<Vec<(T, usize)>, KshanaError> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut line = 1;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            let start = i + 2;
            let mut j = start;
            while j + 1 < c.len() && !(c[j] == '*' && c[j + 1] == '/') {
                j += 1;
            }
            if j + 1 >= c.len() {
                return Err(err(format!("line {line}: unterminated comment")));
            }
            let body: String = c[start..j].iter().collect();
            line += body.matches('\n').count();
            out.push((T::Comment(body.trim().to_string()), line));
            i = j + 2;
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let s = i;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push((T::Id(c[s..i].iter().collect()), line));
            continue;
        }
        if ch == '\'' || ch == '"' {
            let q = ch;
            i += 1;
            let mut v = String::new();
            loop {
                let Some(&x) = c.get(i) else {
                    return Err(err(format!(
                        "line {line}: unterminated {}",
                        if q == '"' { "string" } else { "name" }
                    )));
                };
                i += 1;
                match x {
                    '\\' => {
                        let e = c
                            .get(i)
                            .copied()
                            .ok_or_else(|| err(format!("line {line}: bad escape")))?;
                        i += 1;
                        v.push(match e {
                            'b' => '\u{8}',
                            't' => '\t',
                            'n' => '\n',
                            'f' => '\u{c}',
                            'r' => '\r',
                            '"' => '"',
                            '\'' => '\'',
                            '\\' => '\\',
                            _ => return Err(err(format!("line {line}: bad escape \\{e}"))),
                        });
                    }
                    '\n' => return Err(err(format!("line {line}: newline inside a quoted token"))),
                    _ if x == q => break,
                    _ => v.push(x),
                }
            }
            out.push((if q == '"' { T::Str(v) } else { T::Quoted(v) }, line));
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let s = i;
            while i < c.len()
                && (c[i].is_ascii_digit()
                    || c[i] == '.'
                    || c[i] == 'e'
                    || c[i] == 'E'
                    || ((c[i] == '-' || c[i] == '+') && (c[i - 1] == 'e' || c[i - 1] == 'E')))
            {
                i += 1;
            }
            let raw: String = c[s..i].iter().collect();
            if !valid_number(&raw) {
                return Err(err(format!("line {line}: '{raw}' is not a SysML number")));
            }
            out.push((T::Num(raw), line));
            continue;
        }
        let two: String = c[i..(i + 3).min(c.len())].iter().collect();
        let sym = if two.starts_with(":>>") {
            ":>>"
        } else if two.starts_with(":>") {
            ":>"
        } else if two.starts_with("::") {
            "::"
        } else {
            match ch {
                '{' => "{",
                '}' => "}",
                ';' => ";",
                ':' => ":",
                '[' => "[",
                ']' => "]",
                '(' => "(",
                ')' => ")",
                '=' => "=",
                ',' => ",",
                '*' => "*",
                '/' => "/",
                '<' => "<",
                '>' => ">",
                '-' => "-",
                _ => return Err(err(format!("line {line}: unexpected '{ch}'"))),
            }
        };
        i += sym.len();
        out.push((T::Sym(sym), line));
    }
    Ok(out)
}

/// DECIMAL_VALUE, or RealValue := DECIMAL_VALUE? '.' (DECIMAL_VALUE | EXP_VALUE) | EXP_VALUE.
fn valid_number(s: &str) -> bool {
    let dec = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
    let exp = |x: &str| match x.find(['e', 'E']) {
        Some(p) => {
            let (m, e) = (&x[..p], &x[p + 1..]);
            let e = e.strip_prefix(['+', '-']).unwrap_or(e);
            dec(m) && dec(e)
        }
        None => false,
    };
    if dec(s) || exp(s) {
        return true;
    }
    match s.split_once('.') {
        Some((a, b)) => (a.is_empty() || dec(a)) && (dec(b) || exp(b)),
        None => false,
    }
}

// ------------------------------------------------------------------ parser (checker)

/// One element of the parsed model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    /// `package`, `import`, `part def`, `requirement def`, `verification def`,
    /// `attribute def`, `part`, `requirement`, `verification`, `attribute`, `subject`,
    /// `objective`, `verify`, `satisfy`, `doc`, `comment`.
    pub kind: String,
    pub short_name: Option<String>,
    pub name: Option<String>,
    /// `:` typing or `:>` specialisation target.
    pub typed_by: Option<String>,
    /// `:>>` redefined feature name.
    pub redefines: Option<String>,
    /// Literal value as written, and the string value when it is a string.
    pub value: Option<String>,
    pub string_value: Option<String>,
    /// For satisfy / verify: the requirement referenced; for satisfy also `by`.
    pub target: Option<String>,
    pub by: Option<String>,
    /// Doc or comment text.
    pub text: Option<String>,
    pub children: Vec<Node>,
    pub line: usize,
}

struct P {
    t: Vec<(T, usize)>,
    i: usize,
    refs: Vec<(String, usize, &'static str)>,
}

impl P {
    fn peek(&self) -> Option<&T> {
        self.t.get(self.i).map(|x| &x.0)
    }
    fn line(&self) -> usize {
        self.t
            .get(self.i)
            .or_else(|| self.t.last())
            .map(|x| x.1)
            .unwrap_or(0)
    }
    fn fail<X>(&self, what: &str) -> Result<X, KshanaError> {
        Err(err(format!(
            "line {}: expected {what}, found {}",
            self.line(),
            match self.peek() {
                Some(t) => format!("{t:?}"),
                None => "the end of the file".into(),
            }
        )))
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Some(T::Id(x)) if x == k)
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Some(T::Sym(x)) if *x == s)
    }
    fn kw(&mut self, k: &str) -> Result<(), KshanaError> {
        if self.is_kw(k) {
            self.i += 1;
            Ok(())
        } else {
            self.fail(&format!("'{k}'"))
        }
    }
    fn sym(&mut self, s: &str) -> Result<(), KshanaError> {
        if self.is_sym(s) {
            self.i += 1;
            Ok(())
        } else {
            self.fail(&format!("'{s}'"))
        }
    }
    fn name(&mut self) -> Result<String, KshanaError> {
        match self.peek().cloned() {
            Some(T::Id(x)) if !KEYWORDS.contains(&x.as_str()) => {
                self.i += 1;
                Ok(x)
            }
            Some(T::Quoted(x)) => {
                self.i += 1;
                Ok(x)
            }
            Some(T::Id(x)) => Err(err(format!(
                "line {}: '{x}' is a reserved word and must be quoted to be used as a name",
                self.line()
            ))),
            _ => self.fail("a name"),
        }
    }
    fn at_name(&self) -> bool {
        matches!(self.peek(), Some(T::Quoted(_)))
            || matches!(self.peek(), Some(T::Id(x)) if !KEYWORDS.contains(&x.as_str()))
    }
    fn qname(&mut self, role: &'static str) -> Result<String, KshanaError> {
        let line = self.line();
        let mut parts = vec![self.name()?];
        while self.is_sym("::") {
            self.i += 1;
            parts.push(self.name()?);
        }
        let q = parts.join("::");
        self.refs.push((q.clone(), line, role));
        Ok(q)
    }
    fn identification(&mut self, n: &mut Node) -> Result<(), KshanaError> {
        if self.is_sym("<") {
            self.i += 1;
            n.short_name = Some(self.name()?);
            self.sym(">")?;
        }
        if self.at_name() {
            n.name = Some(self.name()?);
        }
        Ok(())
    }
    fn doc_or_comment(&mut self) -> Result<Option<Node>, KshanaError> {
        let line = self.line();
        if self.is_kw("doc") {
            self.i += 1;
            match self.peek().cloned() {
                Some(T::Comment(c)) => {
                    self.i += 1;
                    Ok(Some(Node {
                        kind: "doc".into(),
                        text: Some(c),
                        line,
                        ..Default::default()
                    }))
                }
                _ => self.fail("a /* ... */ body after 'doc'"),
            }
        } else if let Some(T::Comment(c)) = self.peek().cloned() {
            self.i += 1;
            Ok(Some(Node {
                kind: "comment".into(),
                text: Some(c),
                line,
                ..Default::default()
            }))
        } else {
            Ok(None)
        }
    }
    fn expression(&mut self, n: &mut Node) -> Result<(), KshanaError> {
        match self.peek().cloned() {
            Some(T::Str(s)) => {
                self.i += 1;
                n.value = Some(string_lit(&s));
                n.string_value = Some(s);
            }
            Some(T::Id(x)) if x == "true" || x == "false" => {
                self.i += 1;
                n.value = Some(x);
            }
            _ => {
                let mut v = String::new();
                if self.is_sym("-") {
                    self.i += 1;
                    v.push('-');
                }
                match self.peek().cloned() {
                    Some(T::Num(x)) => {
                        self.i += 1;
                        v.push_str(&x);
                    }
                    _ => return self.fail("a literal value"),
                }
                if self.is_sym("[") {
                    self.i += 1;
                    let mut u = self.qname("unit")?;
                    while self.is_sym("*") || self.is_sym("/") {
                        let op = if self.is_sym("*") { "*" } else { "/" };
                        self.i += 1;
                        u = format!("{u} {op} {}", self.qname("unit")?);
                    }
                    self.sym("]")?;
                    v = format!("{v} [{u}]");
                }
                n.value = Some(v);
            }
        }
        Ok(())
    }
    fn body(&mut self, n: &mut Node, allowed: &[&str]) -> Result<(), KshanaError> {
        if self.is_sym(";") {
            self.i += 1;
            return Ok(());
        }
        self.sym("{")?;
        while !self.is_sym("}") {
            if self.peek().is_none() {
                return self.fail("'}'");
            }
            if let Some(d) = self.doc_or_comment()? {
                n.children.push(d);
                continue;
            }
            let line = self.line();
            let kw = match self.peek() {
                Some(T::Id(x)) => x.clone(),
                _ => return self.fail("a body element"),
            };
            if !allowed.contains(&kw.as_str()) {
                return Err(err(format!(
                    "line {line}: '{kw}' is not in the supported subset here"
                )));
            }
            let child = match kw.as_str() {
                "attribute" | "part" | "requirement" | "verification" => self.usage()?,
                "subject" => {
                    self.i += 1;
                    let mut c = Node {
                        kind: "subject".into(),
                        line,
                        ..Default::default()
                    };
                    c.name = Some(self.name()?);
                    if self.is_sym(":") {
                        self.i += 1;
                        c.typed_by = Some(self.qname("type")?);
                    }
                    self.sym(";")?;
                    c
                }
                "objective" => {
                    self.i += 1;
                    let mut c = Node {
                        kind: "objective".into(),
                        line,
                        ..Default::default()
                    };
                    if self.at_name() {
                        c.name = Some(self.name()?);
                    }
                    self.sym("{")?;
                    while !self.is_sym("}") {
                        if let Some(d) = self.doc_or_comment()? {
                            c.children.push(d);
                        } else if self.is_kw("verify") {
                            c.children.push(self.verify()?);
                        } else {
                            return self.fail("'verify' or 'doc' inside an objective");
                        }
                    }
                    self.sym("}")?;
                    c
                }
                _ => return self.fail("a body element"),
            };
            n.children.push(child);
        }
        self.sym("}")
    }
    fn trailing_docs(&mut self, n: &mut Node) -> Result<(), KshanaError> {
        if self.is_sym(";") {
            self.i += 1;
            return Ok(());
        }
        self.sym("{")?;
        while !self.is_sym("}") {
            match self.doc_or_comment()? {
                Some(d) => n.children.push(d),
                None => return self.fail("'doc' or '}'"),
            }
        }
        self.sym("}")
    }
    fn verify(&mut self) -> Result<Node, KshanaError> {
        let line = self.line();
        self.kw("verify")?;
        if self.is_kw("requirement") {
            self.i += 1;
        }
        let mut n = Node {
            kind: "verify".into(),
            line,
            ..Default::default()
        };
        n.target = Some(self.qname("verified requirement")?);
        self.trailing_docs(&mut n)?;
        Ok(n)
    }
    fn usage(&mut self) -> Result<Node, KshanaError> {
        let line = self.line();
        let kw = match self.peek() {
            Some(T::Id(x)) => x.clone(),
            _ => return self.fail("a usage keyword"),
        };
        self.i += 1;
        let mut n = Node {
            kind: kw.clone(),
            line,
            ..Default::default()
        };
        if kw == "attribute" && self.is_sym(":>>") {
            self.i += 1;
            n.redefines = Some(self.name()?);
        } else {
            self.identification(&mut n)?;
        }
        if self.is_sym(":") {
            self.i += 1;
            n.typed_by = Some(self.qname("type")?);
        }
        if kw == "attribute" && self.is_sym("=") {
            self.i += 1;
            self.expression(&mut n)?;
        }
        let allowed: &[&str] = match kw.as_str() {
            "attribute" => &[],
            "part" => &["attribute", "part"],
            "requirement" => &["attribute", "subject"],
            "verification" => &["attribute", "subject", "objective"],
            _ => return self.fail("'attribute', 'part', 'requirement' or 'verification'"),
        };
        self.body(&mut n, allowed)?;
        Ok(n)
    }
    fn member(&mut self) -> Result<Node, KshanaError> {
        if let Some(d) = self.doc_or_comment()? {
            return Ok(d);
        }
        if self.is_kw("private") || self.is_kw("public") || self.is_kw("protected") {
            self.i += 1;
        }
        let line = self.line();
        let kw = match self.peek() {
            Some(T::Id(x)) => x.clone(),
            _ => return self.fail("a package member"),
        };
        match kw.as_str() {
            "package" => {
                self.i += 1;
                let mut n = Node {
                    kind: "package".into(),
                    line,
                    ..Default::default()
                };
                self.identification(&mut n)?;
                if self.is_sym(";") {
                    self.i += 1;
                    return Ok(n);
                }
                self.sym("{")?;
                while !self.is_sym("}") {
                    if self.peek().is_none() {
                        return self.fail("'}'");
                    }
                    n.children.push(self.member()?);
                }
                self.sym("}")?;
                Ok(n)
            }
            "import" => {
                self.i += 1;
                let mut q = self.qname("import")?;
                if self.is_sym("::") {
                    self.i += 1;
                    if self.is_sym("*") {
                        self.i += 1;
                        q.push_str("::*");
                        if self.is_sym("*") {
                            self.i += 1;
                            q.push('*');
                        }
                    } else {
                        return self.fail("'*' or '**'");
                    }
                }
                self.sym(";")?;
                Ok(Node {
                    kind: "import".into(),
                    target: Some(q),
                    line,
                    ..Default::default()
                })
            }
            "satisfy" => {
                self.i += 1;
                if self.is_kw("requirement") {
                    self.i += 1;
                }
                let mut n = Node {
                    kind: "satisfy".into(),
                    line,
                    ..Default::default()
                };
                n.target = Some(self.qname("satisfied requirement")?);
                self.kw("by")?;
                n.by = Some(self.qname("satisfying element")?);
                self.trailing_docs(&mut n)?;
                Ok(n)
            }
            "part" | "requirement" | "verification" | "attribute" if matches!(self.t.get(self.i + 1), Some((T::Id(x), _)) if x == "def") =>
            {
                self.i += 2;
                let mut n = Node {
                    kind: format!("{kw} def"),
                    line,
                    ..Default::default()
                };
                self.identification(&mut n)?;
                if self.is_sym(":>") {
                    self.i += 1;
                    n.typed_by = Some(self.qname("specialisation")?);
                }
                let allowed: &[&str] = match kw.as_str() {
                    "part" => &["attribute", "part"],
                    "requirement" => &["attribute", "subject"],
                    "verification" => &["attribute", "subject", "objective"],
                    _ => &["attribute"],
                };
                self.body(&mut n, allowed)?;
                Ok(n)
            }
            "part" | "requirement" | "verification" | "attribute" => self.usage(),
            _ => Err(err(format!(
                "line {line}: '{kw}' is not in the supported subset"
            ))),
        }
    }
}

/// The result of checking a SysML file.
#[derive(Debug, Clone)]
pub struct Model {
    pub members: Vec<Node>,
}

fn declared(nodes: &[Node], prefix: &[String], out: &mut Vec<Vec<String>>) {
    for n in nodes {
        let mut here = prefix.to_vec();
        let named = n.name.clone().or_else(|| n.redefines.clone());
        for nm in [n.name.clone(), n.short_name.clone()].into_iter().flatten() {
            let mut p = prefix.to_vec();
            p.push(nm);
            out.push(p);
        }
        if let Some(nm) = named {
            here.push(nm);
        }
        declared(&n.children, &here, out);
    }
}

/// Parse and check a SysML text against the subset: grammar, then name resolution.
pub fn check(src: &str) -> Result<Model, KshanaError> {
    let toks = lex(src)?;
    let mut p = P {
        t: toks,
        i: 0,
        refs: Vec::new(),
    };
    let mut members = Vec::new();
    while p.peek().is_some() {
        members.push(p.member()?);
    }
    if members.iter().all(|m| m.kind != "package") {
        return Err(err("the model has no package"));
    }
    let mut decls = Vec::new();
    declared(&members, &[], &mut decls);
    for (q, line, role) in &p.refs {
        if *role == "import" {
            continue;
        }
        let segs: Vec<&str> = q.split("::").collect();
        let lib = LIBRARY.contains(&q.as_str());
        let local = decls.iter().any(|d| {
            d.len() >= segs.len()
                && d[d.len() - segs.len()..]
                    .iter()
                    .zip(&segs)
                    .all(|(a, b)| a == b)
        });
        if !lib && !local {
            return Err(err(format!("line {line}: {role} '{q}' does not resolve")));
        }
    }
    Ok(Model { members })
}

fn walk_all<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in nodes {
        out.push(n);
        walk_all(&n.children, out);
    }
}

/// Import requirement definitions (and the run links their verify relationships carry)
/// from a SysML file in the subset. Each `requirement def` becomes a requirement: its
/// id is the short name (else the name), its text the `doc`, and its method, criterion
/// and pinned capabilities the string attributes `verificationMethod`,
/// `acceptanceCriterion` and `kshanaCapabilities`. A `verification` usage whose
/// objective verifies a requirement usage typed by that definition links the run named
/// by the verification usage. Returns the requirements and the links.
#[allow(clippy::type_complexity)]
pub fn requirements_from_sysml(
    src: &str,
) -> Result<(Vec<Requirement>, BTreeMap<String, Vec<String>>), KshanaError> {
    let model = check(src)?;
    let mut all = Vec::new();
    walk_all(&model.members, &mut all);
    let mut reqs = Vec::new();
    // requirement def name/short name → requirement id
    let mut def_ids: Vec<(String, String)> = Vec::new();
    for n in all.iter().filter(|n| n.kind == "requirement def") {
        let id = n
            .short_name
            .clone()
            .or_else(|| n.name.clone())
            .ok_or_else(|| err(format!("line {}: a requirement def has no name", n.line)))?;
        let text = n
            .children
            .iter()
            .find(|c| c.kind == "doc")
            .and_then(|c| c.text.clone())
            .unwrap_or_default();
        let attr = |nm: &str| {
            n.children
                .iter()
                .find(|c| c.kind == "attribute" && c.name.as_deref() == Some(nm))
                .and_then(|c| c.string_value.clone())
        };
        let m = attr("verificationMethod").ok_or_else(|| {
            err(format!(
                "line {}: requirement def '{id}' has no verificationMethod attribute",
                n.line
            ))
        })?;
        let method = Method::parse(&m).ok_or_else(|| {
            err(format!(
                "line {}: method '{m}' is not test, analysis, inspection or demonstration",
                n.line
            ))
        })?;
        for nm in [n.name.clone(), n.short_name.clone()].into_iter().flatten() {
            def_ids.push((nm, id.clone()));
        }
        reqs.push(Requirement {
            id,
            text,
            method,
            criterion: attr("acceptanceCriterion"),
            runs: vec![],
            capabilities: attr("kshanaCapabilities")
                .map(|s| super::requirements::split_list(&s))
                .unwrap_or_default(),
        });
    }
    let last = |q: &str| q.rsplit("::").next().unwrap_or(q).to_string();
    // requirement usage name → requirement id (through its definition)
    let mut usage_ids: Vec<(String, String)> = Vec::new();
    for n in all.iter().filter(|n| n.kind == "requirement") {
        if let (Some(nm), Some(ty)) = (&n.name, &n.typed_by) {
            if let Some((_, id)) = def_ids.iter().find(|(d, _)| *d == last(ty)) {
                usage_ids.push((nm.clone(), id.clone()));
            }
        }
    }
    let mut links: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for v in all.iter().filter(|n| n.kind == "verification") {
        let Some(run) = v.name.clone() else { continue };
        let mut inner = Vec::new();
        walk_all(&v.children, &mut inner);
        for vf in inner.iter().filter(|n| n.kind == "verify") {
            let t = last(vf.target.as_deref().unwrap_or(""));
            let id = usage_ids
                .iter()
                .find(|(u, _)| *u == t)
                .or_else(|| def_ids.iter().find(|(d, _)| *d == t))
                .map(|(_, id)| id.clone());
            if let Some(id) = id {
                let e = links.entry(id).or_default();
                if !e.contains(&run) {
                    e.push(run.clone());
                }
            }
        }
    }
    for r in &mut reqs {
        if let Some(runs) = links.get(&r.id) {
            r.runs = runs.clone();
        }
    }
    super::requirements::validate(&reqs)?;
    Ok((reqs, links))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_quoted_when_needed() {
        assert_eq!(name("Clock"), "Clock");
        assert_eq!(name("CLK-001"), "'CLK-001'");
        assert_eq!(name("part"), "'part'");
        assert_eq!(name("it's"), "'it\\'s'");
    }

    #[test]
    fn reals_are_in_the_lexical_form() {
        for (v, s) in [
            (1e-30, "1.0E-30"),
            (5.5e-10, "5.5E-10"),
            (7200.0, "7200.0"),
            (0.25, "0.25"),
            (-3.0, "-3.0"),
        ] {
            assert_eq!(real(v), s);
        }
        for s in ["1.0E-30", "7200.0", "0.25", ".5", "42", "5E3"] {
            assert!(valid_number(s), "{s}");
        }
        for s in ["1.", "1e", "1.0E"] {
            assert!(!valid_number(s), "{s}");
        }
    }

    const GOOD: &str = r#"
package 'Demo' {
    doc /* a demo */
    package Systems {
        part def Clock {
            attribute y0 : ScalarValues::Real { doc /* unit 1 */ }
            attribute holdover : ISQ::DurationValue;
        }
    }
    package Scenarios {
        part 'RUN-1' {
            part clock_classical : Systems::Clock {
                attribute :>> y0 = 5.0E-10;
                attribute :>> holdover = 2610.0 [SI::s];
            }
        }
    }
    package RequirementDefinitions {
        requirement def <'R-1'> 'R-1' {
            doc /* The holdover shall last. */
            attribute verificationMethod : ScalarValues::String = "analysis";
            attribute acceptanceCriterion : ScalarValues::String = "classical.fom.holdover_s >= 30 min";
        }
    }
    package Requirements {
        requirement 'R-1' : RequirementDefinitions::'R-1';
    }
    package Verification {
        verification def KshanaRun { attribute seed : ScalarValues::Integer; }
        verification 'RUN-1' : KshanaRun {
            attribute :>> seed = 42;
            objective { verify Requirements::'R-1' { doc /* met */ } }
        }
    }
    satisfy Requirements::'R-1' by Scenarios::'RUN-1';
}
"#;

    #[test]
    fn subset_checker_accepts_a_well_formed_model() {
        check(GOOD).expect("valid subset");
        let (reqs, links) = requirements_from_sysml(GOOD).unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].id, "R-1");
        assert_eq!(reqs[0].method, Method::Analysis);
        assert_eq!(links["R-1"], vec!["RUN-1"]);
    }

    #[test]
    fn subset_checker_rejects_errors() {
        // unresolved reference
        assert!(check(&GOOD.replace("by Scenarios::'RUN-1'", "by Scenarios::'RUN-9'")).is_err());
        // unquoted keyword used as a name
        assert!(check("package part { }").is_err());
        // missing semicolon
        assert!(check(&GOOD.replace("= 42;", "= 42")).is_err());
        // a bad number
        assert!(check(&GOOD.replace("5.0E-10", "5.0E")).is_err());
        // unterminated doc
        assert!(check("package A { doc /* x }").is_err());
        // outside the subset
        assert!(check("package A { action def X; }").is_err());
        // unresolved library name
        assert!(check(&GOOD.replace("ISQ::DurationValue", "ISQ::NoSuchValue")).is_err());
    }
}
