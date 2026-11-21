// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Independent verification evidence: requirements coverage and verification status
//!
//! An independent verification and validation (ISVV) team, or a supplier preparing for one,
//! needs one answer from three records it already has: the project's **requirements list**,
//! the **traceability map** from requirements to tests, and the **test results**. This module
//! turns them into a signed report:
//!
//! * per requirement: the traced tests, which were found in the results, how many passed,
//!   failed or were skipped, and a status (`passed`, `failed`, `partial`, `not-run`,
//!   `untraced`, or `other-method` when the requirement is verified by analysis, inspection or
//!   review rather than test);
//! * **orphan tests**: results that no trace entry claims;
//! * **trace errors**: trace entries naming a requirement that is not in the list;
//! * **change since the last baseline**: given a previous report (or the signed pack holding
//!   it), the requirements whose status changed, new and removed requirements, and tests that
//!   regressed, were fixed, appeared or disappeared. A baseline given as a signed pack
//!   directory has its signature checked.
//!
//! Inputs: the requirements as CSV (`id` column, optional `text`, `method` and `tests`
//! columns, quoted fields allowed); the trace map as CSV (`requirement,test`, a test pattern
//! may use `*`); results as JUnit XML (`testsuite`/`testcase` with `failure`, `error`,
//! `skipped`) or as CSV (`test,outcome`). A test is named `classname.name`.
//!
//! Generic: no project's requirement text is shipped. **Evidence label: MODELLED**: a tally of
//! the supplied records; no test is re-run and whether a requirement is met is for the
//! project's verification authority to decide.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::sha256_hex;

/// Pro feature.
pub const FEATURE: &str = "pro-isvv-evidence";
/// Report format.
pub const FORMAT: &str = "kshana-pro-isvv-coverage/1";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED";

/// The request (`request.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// Requirements CSV.
    pub requirements: String,
    /// Trace map CSV (`requirement,test`); optional when the requirements carry a `tests`
    /// column.
    #[serde(default)]
    pub trace: Option<String>,
    /// Test result files (JUnit XML, or CSV `test,outcome`).
    pub results: Vec<String>,
    /// A previous report (`report.json`) or the signed pack directory holding it.
    #[serde(default)]
    pub baseline: Option<String>,
}

/// One requirement read from the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Requirement {
    pub id: String,
    pub text: String,
    pub method: String,
}

/// The outcome of one test case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Passed,
    Skipped,
    Failed,
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Passed => "passed",
            Outcome::Skipped => "skipped",
            Outcome::Failed => "failed",
            Outcome::Error => "error",
        }
    }
    fn parse(s: &str) -> Option<Outcome> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pass" | "passed" | "ok" | "success" => Some(Outcome::Passed),
            "skip" | "skipped" | "ignored" | "disabled" => Some(Outcome::Skipped),
            "fail" | "failed" | "failure" => Some(Outcome::Failed),
            "error" | "errored" => Some(Outcome::Error),
            _ => None,
        }
    }
}

/// One test case read from a result file.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub id: String,
    pub outcome: Outcome,
    pub message: String,
    pub file: String,
}

/// One input file and its hash.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputFile {
    pub role: String,
    pub path: String,
    pub sha256: String,
}

/// One trace pattern of a requirement and the tests it matched.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub pattern: String,
    pub matched: Vec<String>,
}

/// One row of the coverage table.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReqRow {
    pub id: String,
    pub text: String,
    pub method: String,
    pub status: String,
    pub links: Vec<Link>,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    /// Trace patterns that matched no test in the results.
    pub not_found: Vec<String>,
}

/// One test (all runs of one test id merged; the worst outcome stands).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestRow {
    pub id: String,
    pub outcome: Outcome,
    pub runs: usize,
    pub files: Vec<String>,
    pub message: String,
    pub requirements: Vec<String>,
}

/// Counts.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    pub requirements: usize,
    pub verified_by_test: usize,
    pub passed: usize,
    pub failed: usize,
    pub partial: usize,
    pub not_run: usize,
    pub untraced: usize,
    pub other_method: usize,
    pub tests: usize,
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub tests_skipped: usize,
    pub orphan_tests: usize,
    /// Passed requirements over requirements verified by test.
    pub passed_share: Option<f64>,
}

/// A change of one item between the baseline and now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub from: String,
    pub to: String,
}

/// Change since the baseline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delta {
    pub baseline_title: String,
    pub baseline_sha256: String,
    /// `verified (signer ...)`, `does not verify: ...`, or `not checked (...)`.
    pub baseline_signature: String,
    pub status_changes: Vec<Change>,
    pub new_requirements: Vec<String>,
    pub removed_requirements: Vec<String>,
    pub test_regressions: Vec<Change>,
    pub test_fixes: Vec<Change>,
    pub new_tests: Vec<String>,
    pub removed_tests: Vec<String>,
}

/// The report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub format: String,
    pub title: String,
    pub evidence: String,
    pub verdict: String,
    pub inputs: Vec<InputFile>,
    pub summary: Summary,
    pub requirements: Vec<ReqRow>,
    pub tests: Vec<TestRow>,
    pub orphan_tests: Vec<String>,
    pub trace_errors: Vec<String>,
    pub baseline: Option<Delta>,
    pub caveats: Vec<String>,
}

// ---------------------------------------------------------------------------------------
// Readers.

/// Parse CSV with a header row; fields may be double-quoted (`""` is a quote inside one).
/// Blank lines and lines starting with `#` are skipped.
pub fn parse_csv(text: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut records: Vec<Vec<String>> = Vec::new();
    let mut rec: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut at_start = true;
    let mut chars = text.chars().peekable();
    let mut line_start = true;
    while let Some(c) = chars.next() {
        if line_start && !quoted && c == '#' {
            for d in chars.by_ref() {
                if d == '\n' {
                    break;
                }
            }
            continue;
        }
        line_start = false;
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if at_start => {
                quoted = true;
                at_start = false;
            }
            ',' => {
                rec.push(field.trim().to_string());
                field.clear();
                at_start = true;
            }
            '\r' => {}
            '\n' => {
                rec.push(field.trim().to_string());
                field.clear();
                if !(rec.len() == 1 && rec[0].is_empty()) {
                    records.push(std::mem::take(&mut rec));
                }
                rec.clear();
                at_start = true;
                line_start = true;
            }
            ' ' | '\t' if at_start => {}
            _ => {
                field.push(c);
                at_start = false;
            }
        }
    }
    if quoted {
        return Err("unterminated quoted field".into());
    }
    if !field.is_empty() || !rec.is_empty() {
        rec.push(field.trim().to_string());
        if !(rec.len() == 1 && rec[0].is_empty()) {
            records.push(rec);
        }
    }
    let mut it = records.into_iter();
    let head: Vec<String> = it
        .next()
        .ok_or("empty table: no header row")?
        .into_iter()
        .map(|h| h.to_ascii_lowercase())
        .collect();
    let mut rows = Vec::new();
    for (i, r) in it.enumerate() {
        if r.len() != head.len() {
            return Err(format!(
                "record {}: {} fields, header has {}",
                i + 2,
                r.len(),
                head.len()
            ));
        }
        rows.push(r);
    }
    Ok((head, rows))
}

fn col(head: &[String], name: &str) -> Option<usize> {
    head.iter().position(|h| h == name)
}

/// Whether a verification method means "by test" (empty, `T` or `test`).
pub fn by_test(method: &str) -> bool {
    matches!(
        method.trim().to_ascii_lowercase().as_str(),
        "" | "t" | "test" | "testing"
    )
}

/// Trace pairs `(requirement, test pattern)`.
pub type TracePairs = Vec<(String, String)>;

/// Read the requirements list; returns the requirements and any trace pairs from a `tests`
/// column (patterns separated by `;`).
pub fn read_requirements(text: &str) -> Result<(Vec<Requirement>, TracePairs), String> {
    let (head, rows) = parse_csv(text)?;
    let id = col(&head, "id").ok_or("requirements: no 'id' column")?;
    let tx = col(&head, "text");
    let me = col(&head, "method");
    let te = col(&head, "tests");
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut pairs = Vec::new();
    for r in rows {
        let rid = r[id].clone();
        if rid.is_empty() {
            return Err("requirements: a row has an empty id".into());
        }
        if !seen.insert(rid.clone()) {
            return Err(format!("requirements: '{rid}' twice"));
        }
        if let Some(t) = te {
            for p in r[t].split(';').map(str::trim).filter(|p| !p.is_empty()) {
                pairs.push((rid.clone(), p.to_string()));
            }
        }
        out.push(Requirement {
            id: rid,
            text: tx.map(|c| r[c].clone()).unwrap_or_default(),
            method: me.map(|c| r[c].clone()).unwrap_or_default(),
        });
    }
    Ok((out, pairs))
}

/// Read a trace map (`requirement,test`).
pub fn read_trace(text: &str) -> Result<TracePairs, String> {
    let (head, rows) = parse_csv(text)?;
    let rq = col(&head, "requirement").ok_or("trace: no 'requirement' column")?;
    let te = col(&head, "test").ok_or("trace: no 'test' column")?;
    Ok(rows
        .into_iter()
        .filter(|r| !r[rq].is_empty() && !r[te].is_empty())
        .map(|r| (r[rq].clone(), r[te].clone()))
        .collect())
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(j) = tail.find(';') else {
            out.push_str(tail);
            return out;
        };
        let ent = &tail[1..j];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16)
                .ok()
                .and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => out.push(c),
            None => out.push_str(&tail[..=j]),
        }
        rest = &tail[j + 1..];
    }
    out.push_str(rest);
    out
}

/// A start or end tag: name, attributes, end tag, self-closing.
type Tag = (String, BTreeMap<String, String>, bool, bool);

fn parse_tag(inner: &str) -> Result<Tag, String> {
    let (end, inner) = match inner.strip_prefix('/') {
        Some(r) => (true, r),
        None => (false, inner),
    };
    let (selfc, inner) = match inner.strip_suffix('/') {
        Some(r) => (true, r),
        None => (false, inner),
    };
    let inner = inner.trim();
    let name_end = inner
        .find(|c: char| c.is_whitespace())
        .unwrap_or(inner.len());
    let name = inner[..name_end].to_string();
    let mut attrs = BTreeMap::new();
    let mut rest = inner[name_end..].trim_start();
    while !rest.is_empty() {
        let eq = rest
            .find('=')
            .ok_or_else(|| format!("<{name}>: attribute without a value"))?;
        let key = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let q = after
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'')
            .ok_or_else(|| format!("<{name}>: attribute '{key}' is not quoted"))?;
        let close = after[1..]
            .find(q)
            .ok_or_else(|| format!("<{name}>: attribute '{key}' is not closed"))?;
        attrs.insert(key, unescape(&after[1..1 + close]));
        rest = after[close + 2..].trim_start();
    }
    Ok((name, attrs, end, selfc))
}

/// Read JUnit XML: every `testcase` (its `classname` attribute, else the enclosing
/// `testsuite` name, joined to `name` by `.`), failed on `failure`, errored on `error`,
/// skipped on `skipped`.
pub fn read_junit(text: &str, file: &str) -> Result<Vec<Case>, String> {
    let mut out = Vec::new();
    let mut suites: Vec<String> = Vec::new();
    let mut cur: Option<Case> = None;
    let mut rest = text;
    let err = |m: String| format!("{file}: {m}");
    while let Some(i) = rest.find('<') {
        rest = &rest[i..];
        let skip_to = |rest: &str, open: &str, close: &str| -> Result<usize, String> {
            rest.find(close)
                .map(|j| j + close.len())
                .ok_or_else(|| format!("{file}: '{open}' is not closed"))
        };
        if rest.starts_with("<!--") {
            rest = &rest[skip_to(rest, "<!--", "-->")?..];
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            rest = &rest[skip_to(rest, "<![CDATA[", "]]>")?..];
            continue;
        }
        if rest.starts_with("<?") {
            rest = &rest[skip_to(rest, "<?", "?>")?..];
            continue;
        }
        if rest.starts_with("<!") {
            rest = &rest[skip_to(rest, "<!", ">")?..];
            continue;
        }
        let j = rest
            .find('>')
            .ok_or_else(|| err("a tag is not closed".into()))?;
        let (name, attrs, end, selfc) = parse_tag(&rest[1..j]).map_err(err)?;
        rest = &rest[j + 1..];
        match (name.as_str(), end) {
            ("testsuite", false) if !selfc => {
                suites.push(attrs.get("name").cloned().unwrap_or_default());
            }
            ("testsuite", true) => {
                suites.pop();
            }
            ("testcase", false) => {
                if cur.is_some() {
                    return Err(err("a testcase inside a testcase".into()));
                }
                let n = attrs
                    .get("name")
                    .cloned()
                    .ok_or_else(|| err("a testcase without a name".into()))?;
                let cls = attrs
                    .get("classname")
                    .cloned()
                    .or_else(|| suites.last().cloned())
                    .unwrap_or_default();
                let c = Case {
                    id: if cls.is_empty() {
                        n
                    } else {
                        format!("{cls}.{n}")
                    },
                    outcome: Outcome::Passed,
                    message: String::new(),
                    file: file.to_string(),
                };
                if selfc {
                    out.push(c);
                } else {
                    cur = Some(c);
                }
            }
            ("testcase", true) => {
                out.push(
                    cur.take()
                        .ok_or_else(|| err("a stray </testcase>".into()))?,
                );
            }
            ("failure" | "error" | "skipped", false) => {
                if let Some(c) = cur.as_mut() {
                    let o = match name.as_str() {
                        "failure" => Outcome::Failed,
                        "error" => Outcome::Error,
                        _ => Outcome::Skipped,
                    };
                    if o > c.outcome {
                        c.outcome = o;
                        c.message = attrs
                            .get("message")
                            .or_else(|| attrs.get("type"))
                            .cloned()
                            .unwrap_or_default();
                    }
                }
            }
            _ => {}
        }
    }
    if cur.is_some() {
        return Err(err("a testcase is not closed".into()));
    }
    Ok(out)
}

/// Read results as CSV (`test,outcome[,message]`).
pub fn read_result_csv(text: &str, file: &str) -> Result<Vec<Case>, String> {
    let (head, rows) = parse_csv(text).map_err(|e| format!("{file}: {e}"))?;
    let te = col(&head, "test").ok_or_else(|| format!("{file}: no 'test' column"))?;
    let oc = col(&head, "outcome").ok_or_else(|| format!("{file}: no 'outcome' column"))?;
    let me = col(&head, "message");
    rows.into_iter()
        .map(|r| {
            Ok(Case {
                id: r[te].clone(),
                outcome: Outcome::parse(&r[oc]).ok_or_else(|| {
                    format!("{file}: test '{}': unknown outcome '{}'", r[te], r[oc])
                })?,
                message: me.map(|c| r[c].clone()).unwrap_or_default(),
                file: file.to_string(),
            })
        })
        .collect()
}

/// `*` matches any run of characters; everything else literally.
pub fn glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == s;
    }
    let mut pos = 0;
    for (i, p) in parts.iter().enumerate() {
        if i == 0 {
            if !s.starts_with(p) {
                return false;
            }
            pos = p.len();
        } else if i == parts.len() - 1 {
            return s.len() >= pos + p.len() && s[pos..].ends_with(p);
        } else if let Some(k) = s[pos..].find(p) {
            pos += k + p.len();
        } else {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------------------
// Coverage.

/// Merge all runs of each test id; the worst outcome stands.
fn merge(cases: &[Case]) -> BTreeMap<String, TestRow> {
    let mut m: BTreeMap<String, TestRow> = BTreeMap::new();
    for c in cases {
        let e = m.entry(c.id.clone()).or_insert_with(|| TestRow {
            id: c.id.clone(),
            outcome: c.outcome,
            runs: 0,
            files: Vec::new(),
            message: c.message.clone(),
            requirements: Vec::new(),
        });
        e.runs += 1;
        if !e.files.contains(&c.file) {
            e.files.push(c.file.clone());
        }
        if c.outcome > e.outcome {
            e.outcome = c.outcome;
            e.message = c.message.clone();
        }
    }
    m
}

/// The coverage report from records already read (no files, no baseline).
pub fn coverage(
    title: &str,
    reqs: &[Requirement],
    trace: &[(String, String)],
    cases: &[Case],
) -> Report {
    let mut tests = merge(cases);
    let known: BTreeSet<&str> = reqs.iter().map(|r| r.id.as_str()).collect();
    let mut trace_errors = Vec::new();
    let mut by_req: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (r, t) in trace {
        if !known.contains(r.as_str()) {
            trace_errors.push(format!(
                "trace entry {r} -> {t}: requirement '{r}' is not in the list"
            ));
            continue;
        }
        let v = by_req.entry(r.as_str()).or_default();
        if !v.contains(&t.as_str()) {
            v.push(t.as_str());
        }
    }
    let mut rows = Vec::new();
    for r in reqs {
        let pats = by_req.get(r.id.as_str()).cloned().unwrap_or_default();
        let mut links = Vec::new();
        let mut not_found = Vec::new();
        let mut matched_all: BTreeSet<String> = BTreeSet::new();
        for p in &pats {
            let matched: Vec<String> = tests.keys().filter(|id| glob(p, id)).cloned().collect();
            if matched.is_empty() {
                not_found.push(p.to_string());
            }
            matched_all.extend(matched.iter().cloned());
            links.push(Link {
                pattern: p.to_string(),
                matched,
            });
        }
        let (mut passed, mut failed, mut skipped) = (0, 0, 0);
        for id in &matched_all {
            let t = tests.get_mut(id).expect("matched from the keys");
            t.requirements.push(r.id.clone());
            match t.outcome {
                Outcome::Passed => passed += 1,
                Outcome::Skipped => skipped += 1,
                Outcome::Failed | Outcome::Error => failed += 1,
            }
        }
        let status = if !by_test(&r.method) && pats.is_empty() {
            "other-method"
        } else if pats.is_empty() {
            "untraced"
        } else if failed > 0 {
            "failed"
        } else if passed == 0 {
            "not-run"
        } else if !not_found.is_empty() || skipped > 0 {
            "partial"
        } else {
            "passed"
        };
        rows.push(ReqRow {
            id: r.id.clone(),
            text: r.text.clone(),
            method: r.method.clone(),
            status: status.into(),
            links,
            passed,
            failed,
            skipped,
            not_found,
        });
    }
    let tests: Vec<TestRow> = tests.into_values().collect();
    let orphan_tests: Vec<String> = tests
        .iter()
        .filter(|t| t.requirements.is_empty())
        .map(|t| t.id.clone())
        .collect();
    let count = |s: &str| rows.iter().filter(|r| r.status == s).count();
    let tcount = |f: &dyn Fn(Outcome) -> bool| tests.iter().filter(|t| f(t.outcome)).count();
    let other_method = count("other-method");
    let verified_by_test = rows.len() - other_method;
    let passed = count("passed");
    let summary = Summary {
        requirements: rows.len(),
        verified_by_test,
        passed,
        failed: count("failed"),
        partial: count("partial"),
        not_run: count("not-run"),
        untraced: count("untraced"),
        other_method,
        tests: tests.len(),
        tests_passed: tcount(&|o| o == Outcome::Passed),
        tests_failed: tcount(&|o| matches!(o, Outcome::Failed | Outcome::Error)),
        tests_skipped: tcount(&|o| o == Outcome::Skipped),
        orphan_tests: orphan_tests.len(),
        passed_share: (verified_by_test > 0).then(|| passed as f64 / verified_by_test as f64),
    };
    let verdict = format!(
        "{} of {} requirements verified by test have passed; {} failed, {} partial, {} not run, {} untraced; {} orphan tests",
        summary.passed,
        summary.verified_by_test,
        summary.failed,
        summary.partial,
        summary.not_run,
        summary.untraced,
        summary.orphan_tests
    );
    Report {
        format: FORMAT.into(),
        title: title.into(),
        evidence: EVIDENCE.into(),
        verdict,
        inputs: Vec::new(),
        summary,
        requirements: rows,
        tests,
        orphan_tests,
        trace_errors,
        baseline: None,
        caveats: vec![
            "A tally of the supplied requirements, trace map and test results; no test is re-run.".into(),
            "A requirement counts as passed when every traced test pattern matched a test in the results, none failed and none was skipped. Whether that meets the requirement is for the project's verification authority to decide.".into(),
            "Requirements verified by analysis, inspection or review (method other than test) are listed, not scored, unless tests are traced to them.".into(),
        ],
    }
}

/// Change from `base` to `now`.
pub fn delta(base: &Report, now: &Report, sha: &str, signature: &str) -> Delta {
    let b: BTreeMap<&str, &str> = base
        .requirements
        .iter()
        .map(|r| (r.id.as_str(), r.status.as_str()))
        .collect();
    let n: BTreeMap<&str, &str> = now
        .requirements
        .iter()
        .map(|r| (r.id.as_str(), r.status.as_str()))
        .collect();
    let bt: BTreeMap<&str, Outcome> = base
        .tests
        .iter()
        .map(|t| (t.id.as_str(), t.outcome))
        .collect();
    let nt: BTreeMap<&str, Outcome> = now
        .tests
        .iter()
        .map(|t| (t.id.as_str(), t.outcome))
        .collect();
    let ch = |id: &str, f: &str, t: &str| Change {
        id: id.into(),
        from: f.into(),
        to: t.into(),
    };
    let bad = |o: Outcome| matches!(o, Outcome::Failed | Outcome::Error);
    Delta {
        baseline_title: base.title.clone(),
        baseline_sha256: sha.into(),
        baseline_signature: signature.into(),
        status_changes: n
            .iter()
            .filter_map(|(id, s)| b.get(id).filter(|f| *f != s).map(|f| ch(id, f, s)))
            .collect(),
        new_requirements: n
            .keys()
            .filter(|k| !b.contains_key(*k))
            .map(|k| k.to_string())
            .collect(),
        removed_requirements: b
            .keys()
            .filter(|k| !n.contains_key(*k))
            .map(|k| k.to_string())
            .collect(),
        test_regressions: nt
            .iter()
            .filter_map(|(id, o)| {
                bt.get(id)
                    .filter(|f| !bad(**f) && bad(*o))
                    .map(|f| ch(id, f.as_str(), o.as_str()))
            })
            .collect(),
        test_fixes: nt
            .iter()
            .filter_map(|(id, o)| {
                bt.get(id)
                    .filter(|f| bad(**f) && *o == Outcome::Passed)
                    .map(|f| ch(id, f.as_str(), o.as_str()))
            })
            .collect(),
        new_tests: nt
            .keys()
            .filter(|k| !bt.contains_key(*k))
            .map(|k| k.to_string())
            .collect(),
        removed_tests: bt
            .keys()
            .filter(|k| !nt.contains_key(*k))
            .map(|k| k.to_string())
            .collect(),
    }
}

fn read(base: &Path, p: &str) -> Result<(PathBuf, Vec<u8>), String> {
    let path = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        base.join(p)
    };
    let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, b))
}

fn utf8(p: &Path, b: Vec<u8>) -> Result<String, String> {
    String::from_utf8(b).map_err(|_| format!("{}: not UTF-8 text", p.display()))
}

/// The input files (role, published name, bytes) of a run, for the pack.
pub type Inputs = Vec<(String, String, Vec<u8>)>;

/// Run the request: read every file under `base`, tally, compare with the baseline. A
/// baseline given as a signed pack directory is verified against `trust` (any key when empty).
pub fn run(
    q: &Request,
    base: &Path,
    trust: &[[u8; crate::license::KEY_LEN]],
) -> Result<(Report, Inputs), String> {
    if q.results.is_empty() {
        return Err("results: no result file".into());
    }
    let mut inputs: Inputs = Vec::new();
    let (rp, rb) = read(base, &q.requirements)?;
    let rtext = utf8(&rp, rb.clone())?;
    inputs.push(("requirements".into(), "requirements.csv".into(), rb));
    let (reqs, mut trace) =
        read_requirements(&rtext).map_err(|e| format!("{}: {e}", rp.display()))?;
    if let Some(t) = &q.trace {
        let (tp, tb) = read(base, t)?;
        let ttext = utf8(&tp, tb.clone())?;
        inputs.push(("trace".into(), "trace.csv".into(), tb));
        trace.extend(read_trace(&ttext).map_err(|e| format!("{}: {e}", tp.display()))?);
    } else if trace.is_empty() {
        return Err("no trace map: give 'trace' or a 'tests' column in the requirements".into());
    }
    let mut cases = Vec::new();
    for (i, f) in q.results.iter().enumerate() {
        let (p, b) = read(base, f)?;
        let text = utf8(&p, b.clone())?;
        let name = Path::new(f)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.clone());
        let is_xml = text.trim_start().starts_with('<');
        cases.extend(if is_xml {
            read_junit(&text, &name)?
        } else {
            read_result_csv(&text, &name)?
        });
        inputs.push(("results".into(), format!("results/{:02}-{name}", i + 1), b));
    }
    let mut rep = coverage(&q.title, &reqs, &trace, &cases);
    if let Some(bl) = &q.baseline {
        let p = if Path::new(bl).is_absolute() {
            PathBuf::from(bl)
        } else {
            base.join(bl)
        };
        let (file, signature) = if p.is_dir() {
            let s = match crate::signed_dossier::verify(&p, trust) {
                Ok(v) if v.problems.is_empty() => format!(
                    "verified (signer {}, key {}{})",
                    v.signer,
                    v.key_id,
                    if trust.is_empty() {
                        ", no trusted key given"
                    } else {
                        ", trusted"
                    }
                ),
                Ok(v) => format!("does not verify: {}", v.problems.join("; ")),
                Err(e) => format!("does not verify: {e}"),
            };
            (p.join("report.json"), s)
        } else {
            (
                p.clone(),
                "not checked (a report file, not a signed pack)".into(),
            )
        };
        let b = std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let prev: Report = serde_json::from_slice(&b)
            .map_err(|e| format!("{}: not an ISVV coverage report: {e}", file.display()))?;
        if prev.format != FORMAT {
            return Err(format!(
                "{}: format '{}', expected {FORMAT}",
                file.display(),
                prev.format
            ));
        }
        rep.baseline = Some(delta(&prev, &rep, &sha256_hex(&b), &signature));
        inputs.push(("baseline".into(), "baseline-report.json".into(), b));
    }
    rep.inputs = inputs
        .iter()
        .map(|(role, name, b)| InputFile {
            role: role.clone(),
            path: format!("inputs/{name}"),
            sha256: sha256_hex(b),
        })
        .collect();
    Ok((rep, inputs))
}

/// Write the pack: report, page, request and inputs (sign afterwards).
pub fn write(out: &Path, rep: &Report, request_src: &str, inputs: &Inputs) -> Result<(), String> {
    crate::transport::write_pack(out, rep, &html(rep), "request.toml", request_src)?;
    for (_, name, b) in inputs {
        let p = out.join("inputs").join(name);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        std::fs::write(&p, b).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

/// The evidence page.
pub fn html(r: &Report) -> String {
    let s = &r.summary;
    let intro = vec![
        format!(
            "{} requirements ({} verified by test, {} by another method); {} tests ({} passed, {} failed, {} skipped).",
            s.requirements, s.verified_by_test, s.other_method, s.tests, s.tests_passed, s.tests_failed, s.tests_skipped
        ),
        r.caveats.join(" "),
    ];
    let reqs = r
        .requirements
        .iter()
        .map(|q| {
            vec![
                q.id.clone(),
                q.text.clone(),
                if q.method.is_empty() {
                    "test".into()
                } else {
                    q.method.clone()
                },
                q.status.clone(),
                format!("{} / {} / {}", q.passed, q.failed, q.skipped),
                q.not_found.join(", "),
            ]
        })
        .collect();
    let bad = r
        .tests
        .iter()
        .filter(|t| t.outcome != Outcome::Passed)
        .map(|t| {
            vec![
                t.id.clone(),
                t.outcome.as_str().into(),
                t.message.clone(),
                t.requirements.join(", "),
            ]
        })
        .collect();
    let orphans = r.orphan_tests.iter().map(|t| vec![t.clone()]).collect();
    let errs = r.trace_errors.iter().map(|t| vec![t.clone()]).collect();
    let mut sections = vec![
        (
            "Requirements".to_string(),
            vec![
                "id",
                "text",
                "method",
                "status",
                "passed / failed / skipped",
                "traced but not found",
            ],
            reqs,
        ),
        (
            "Tests not passed".to_string(),
            vec!["test", "outcome", "message", "requirements"],
            bad,
        ),
        (
            "Orphan tests (no requirement)".to_string(),
            vec!["test"],
            orphans,
        ),
        ("Trace errors".to_string(), vec!["entry"], errs),
    ];
    if let Some(d) = &r.baseline {
        let mut rows: Vec<Vec<String>> = vec![vec![
            "baseline".into(),
            format!(
                "{} ({})",
                d.baseline_title,
                &d.baseline_sha256[..12.min(d.baseline_sha256.len())]
            ),
            d.baseline_signature.clone(),
        ]];
        let fmt = |v: &[Change]| {
            v.iter()
                .map(|c| format!("{}: {} → {}", c.id, c.from, c.to))
                .collect::<Vec<_>>()
                .join("; ")
        };
        rows.push(vec![
            "requirement status changes".into(),
            d.status_changes.len().to_string(),
            fmt(&d.status_changes),
        ]);
        rows.push(vec![
            "new requirements".into(),
            d.new_requirements.len().to_string(),
            d.new_requirements.join(", "),
        ]);
        rows.push(vec![
            "removed requirements".into(),
            d.removed_requirements.len().to_string(),
            d.removed_requirements.join(", "),
        ]);
        rows.push(vec![
            "test regressions".into(),
            d.test_regressions.len().to_string(),
            fmt(&d.test_regressions),
        ]);
        rows.push(vec![
            "test fixes".into(),
            d.test_fixes.len().to_string(),
            fmt(&d.test_fixes),
        ]);
        rows.push(vec![
            "new tests".into(),
            d.new_tests.len().to_string(),
            d.new_tests.join(", "),
        ]);
        rows.push(vec![
            "removed tests".into(),
            d.removed_tests.len().to_string(),
            d.removed_tests.join(", "),
        ]);
        sections.push((
            "Change since baseline".to_string(),
            vec!["item", "count", "detail"],
            rows,
        ));
    }
    crate::transport::page(&r.title, &r.evidence, &r.verdict, &intro, &sections)
}

// ---------------------------------------------------------------------------------------
// Synthetic example.

/// SYNTHETIC records: requirements, trace map, a baseline JUnit file and a current one.
/// Returns `(requirements.csv, trace.csv, junit-baseline.xml, junit-current.xml)`.
pub fn synth() -> (String, String, String, String) {
    let reqs = "id,text,method\n\
        SYN-001,\"The unit shall start within 5 s, cold\",T\n\
        SYN-002,The unit shall log every command,T\n\
        SYN-003,The unit shall reject a malformed telecommand,T\n\
        SYN-004,The unit shall report its software version,T\n\
        SYN-005,The unit shall recover from a watchdog reset,T\n\
        SYN-006,The memory budget shall stay below 80 %,A\n\
        SYN-007,The unit shall time-stamp telemetry,T\n\
        SYN-008,The unit shall store parameters across resets,T\n";
    let trace = "requirement,test\n\
        SYN-001,boot.Startup.cold_start\n\
        SYN-002,log.Commands.*\n\
        SYN-003,tc.Parser.rejects_bad_crc\n\
        SYN-003,tc.Parser.rejects_short_frame\n\
        SYN-004,hk.Version.reports_version\n\
        SYN-005,fdir.Watchdog.recovers\n\
        SYN-007,tm.Clock.stamps_packets\n\
        SYN-099,tm.Clock.stamps_packets\n";
    let case = |cls: &str, name: &str, body: &str| {
        if body.is_empty() {
            format!("    <testcase classname=\"{cls}\" name=\"{name}\" time=\"0.01\"/>\n")
        } else {
            format!("    <testcase classname=\"{cls}\" name=\"{name}\" time=\"0.01\">{body}</testcase>\n")
        }
    };
    let junit = |cases: &[(&str, &str, &str)]| {
        let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- SYNTHETIC test results -->\n<testsuites>\n  <testsuite name=\"synthetic\">\n");
        for (c, n, b) in cases {
            s.push_str(&case(c, n, b));
        }
        s.push_str("  </testsuite>\n</testsuites>\n");
        s
    };
    let fail = "<failure message=\"expected 0x1F, got 0x00\"/>";
    let baseline = junit(&[
        ("boot.Startup", "cold_start", ""),
        ("log.Commands", "logs_nominal", ""),
        ("log.Commands", "logs_burst", ""),
        ("tc.Parser", "rejects_bad_crc", ""),
        ("tc.Parser", "rejects_short_frame", ""),
        ("hk.Version", "reports_version", fail),
        ("tm.Clock", "stamps_packets", ""),
    ]);
    let current = junit(&[
        ("boot.Startup", "cold_start", ""),
        ("log.Commands", "logs_nominal", ""),
        ("log.Commands", "logs_burst", fail),
        ("tc.Parser", "rejects_bad_crc", ""),
        ("hk.Version", "reports_version", ""),
        (
            "tm.Clock",
            "stamps_packets",
            "<skipped message=\"bench clock unavailable\"/>",
        ),
        ("util.Strings", "trims &amp; splits", ""),
    ]);
    (reqs.into(), trace.into(), baseline, current)
}
