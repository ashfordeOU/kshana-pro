// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The requirements model and its CSV form.
//!
//! A requirement carries an identifier, its text, a verification method (test,
//! analysis, inspection or demonstration, the four methods of ECSS-E-ST-10-02C, the
//! European Cooperation for Space Standardization verification standard) and an
//! acceptance criterion written in the criterion language of
//! [`super::criterion`]. It may also name the Kshana runs that verify it and the
//! verification-matrix capability rows its evidence rests on.
//!
//! CSV (comma-separated values, RFC 4180): a header row, then one requirement per row.
//! Columns, matched without regard to case: `id`, `text`, `method` (or `verification`,
//! `verification_method`), `criterion` (or `acceptance`, `acceptance_criterion`),
//! optional `runs` and `capability`, each a `;`-separated list. Other columns are
//! ignored.

use serde::{Deserialize, Serialize};

use super::criterion;
use kshana::api::KshanaError;

/// A verification method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Test,
    Analysis,
    Inspection,
    Demonstration,
}

impl Method {
    pub fn parse(s: &str) -> Option<Method> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "test" | "t" => Method::Test,
            "analysis" | "a" => Method::Analysis,
            "inspection" | "i" => Method::Inspection,
            "demonstration" | "d" | "demo" => Method::Demonstration,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Test => "test",
            Method::Analysis => "analysis",
            Method::Inspection => "inspection",
            Method::Demonstration => "demonstration",
        }
    }
    /// ECSS-E-ST-10-02C counts verification by simulation as analysis. For any other
    /// method a Kshana run supports the criterion but does not discharge the method.
    pub fn discharged_by_simulation(self) -> bool {
        self == Method::Analysis
    }
}

/// One requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    pub method: Method,
    /// Machine-checkable acceptance criterion; `None` when the requirement has none.
    #[serde(default)]
    pub criterion: Option<String>,
    /// Run identifiers that verify this requirement.
    #[serde(default)]
    pub runs: Vec<String>,
    /// Verification-matrix capability rows (their `requirement` names) the evidence
    /// rests on, when the author pins them.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// Check a requirements set: non-empty, unique identifiers, criteria that parse.
pub fn validate(reqs: &[Requirement]) -> Result<(), KshanaError> {
    if reqs.is_empty() {
        return Err(err("no requirements supplied"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for r in reqs {
        if r.id.trim().is_empty() {
            return Err(err("a requirement has an empty id"));
        }
        if !seen.insert(r.id.clone()) {
            return Err(err(format!("requirement id '{}' appears twice", r.id)));
        }
        if let Some(c) = &r.criterion {
            criterion::parse(c).map_err(|e| {
                err(format!(
                    "requirement '{}': criterion `{c}` does not parse: {e}",
                    r.id
                ))
            })?;
        }
    }
    Ok(())
}

/// Split a `;`-separated list, dropping empty items.
pub fn split_list(s: &str) -> Vec<String> {
    s.split(';')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Parse RFC 4180 CSV into rows of fields.
pub fn parse_csv_rows(src: &str) -> Result<Vec<Vec<String>>, KshanaError> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = src.trim_start_matches('\u{feff}').chars().peekable();
    let mut line = 1;
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                '\n' => {
                    line += 1;
                    field.push(c)
                }
                _ => field.push(c),
            }
        } else {
            match c {
                '"' if field.is_empty() => in_quotes = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    line += 1;
                    row.push(std::mem::take(&mut field));
                    if !(row.len() == 1 && row[0].is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    } else {
                        row.clear();
                    }
                }
                _ => field.push(c),
            }
        }
    }
    if in_quotes {
        return Err(err(format!(
            "CSV: unterminated quoted field before line {line}"
        )));
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

/// Import requirements from CSV.
pub fn from_csv(src: &str) -> Result<Vec<Requirement>, KshanaError> {
    let rows = parse_csv_rows(src)?;
    let header = rows.first().ok_or_else(|| err("CSV: no header row"))?;
    let col = |names: &[&str]| -> Option<usize> {
        header
            .iter()
            .position(|h| names.contains(&h.trim().to_ascii_lowercase().as_str()))
    };
    let id_c = col(&["id"]).ok_or_else(|| err("CSV: no `id` column"))?;
    let text_c = col(&["text", "requirement"]).ok_or_else(|| err("CSV: no `text` column"))?;
    let method_c = col(&["method", "verification", "verification_method"])
        .ok_or_else(|| err("CSV: no `method` column"))?;
    let crit_c = col(&["criterion", "acceptance", "acceptance_criterion"]);
    let runs_c = col(&["runs", "run"]);
    let cap_c = col(&["capability", "capabilities"]);
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate().skip(1) {
        let get = |c: usize| row.get(c).map(|s| s.trim().to_string()).unwrap_or_default();
        let m = get(method_c);
        let method = Method::parse(&m).ok_or_else(|| {
            err(format!(
                "CSV row {}: method '{m}' is not test, analysis, inspection or demonstration",
                i + 1
            ))
        })?;
        let crit = crit_c.map(get).filter(|s| !s.is_empty());
        out.push(Requirement {
            id: get(id_c),
            text: get(text_c),
            method,
            criterion: crit,
            runs: runs_c.map(|c| split_list(&get(c))).unwrap_or_default(),
            capabilities: cap_c.map(|c| split_list(&get(c))).unwrap_or_default(),
        });
    }
    validate(&out)?;
    Ok(out)
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Export requirements to CSV, in the column order [`from_csv`] reads.
pub fn to_csv(reqs: &[Requirement]) -> String {
    let mut s = String::from("id,text,method,criterion,runs,capability\n");
    for r in reqs {
        s.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_field(&r.id),
            csv_field(&r.text),
            r.method.as_str(),
            csv_field(r.criterion.as_deref().unwrap_or("")),
            csv_field(&r.runs.join(";")),
            csv_field(&r.capabilities.join(";")),
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_round_trip_with_quotes_and_commas() {
        let src = "ID,Text,Method,Criterion,Runs\n\
                   R1,\"Holdover, classical\",Analysis,classical.fom.holdover_s >= 30 min,RUN-A;RUN-B\n\
                   R2,\"Says \"\"hello\"\"\",inspection,,\n";
        let reqs = from_csv(src).unwrap();
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].text, "Holdover, classical");
        assert_eq!(reqs[0].runs, vec!["RUN-A", "RUN-B"]);
        assert_eq!(reqs[1].text, "Says \"hello\"");
        assert_eq!(reqs[1].criterion, None);
        assert_eq!(from_csv(&to_csv(&reqs)).unwrap(), reqs);
    }

    #[test]
    fn csv_rejects_bad_method_duplicate_and_bad_criterion() {
        assert!(from_csv("id,text,method\nR1,x,guess\n").is_err());
        assert!(from_csv("id,text,method\nR1,x,test\nR1,y,test\n").is_err());
        let e = from_csv("id,text,method,criterion\nR1,x,test,a <\n").unwrap_err();
        assert!(e.to_string().contains("does not parse"));
        assert!(from_csv("text,method\nx,test\n").is_err());
    }
}
