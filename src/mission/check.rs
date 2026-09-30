// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Check a delivered dossier.
//!
//! `kshana-pro mission check <dir>` reads nothing but the directory:
//!
//! 1. the body of `dossier.json` must hash to its `dossier_sha256`;
//! 2. every file the body lists (the trace, the requirements copy, everything under
//!    `runs/`) must hash to the value the body records;
//! 3. every file `manifest.json` lists (the three faces of the dossier and the evidence
//!    pack as well) must hash to the value the manifest records, and the manifest must
//!    name the same dossier hash;
//! 4. `ledger.json` must verify, and its last record must carry the dossier hash.
//!
//! With `--rerun`, every stored scenario is run again on the embedded engine and the
//! SHA-256 of the result is compared with the dossier's. When a re-run differs, the
//! report lists which fields differ and by how much.
//!
//! A failed check names each file or run it failed on.

use std::path::{Component, Path};

use serde_json::Value;

use crate::license::Entitlement;
use crate::mbse::evidence::sha256_hex;
use crate::study_ledger::StudyLedger;
use kshana::api::KshanaError;

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// One re-run and how it compares.
#[derive(Debug, Clone, PartialEq)]
pub struct Rerun {
    pub run: String,
    pub matches: bool,
    pub recorded_sha256: String,
    pub rerun_sha256: Option<String>,
    /// What differs, when the hashes do.
    pub differences: Vec<String>,
}

/// The outcome of a check.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CheckReport {
    pub dossier_sha256: String,
    /// Files whose hash was compared.
    pub files_checked: usize,
    /// Every problem found, each naming its file or run. Empty means the check passed.
    pub problems: Vec<String>,
    pub reruns: Vec<Rerun>,
}

impl CheckReport {
    pub fn passed(&self) -> bool {
        self.problems.is_empty()
    }
}

/// A path from a dossier must stay inside the dossier directory.
fn safe(rel: &str) -> bool {
    let p = Path::new(rel);
    !rel.is_empty()
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn listed(v: Option<&Value>) -> Vec<(String, String)> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|f| {
                    Some((
                        f.get("path")?.as_str()?.to_string(),
                        f.get("sha256")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn fmt(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 40 {
        format!("{}...", s.chars().take(37).collect::<String>())
    } else {
        s
    }
}

/// Every leaf at which two JSON documents differ: the number of them and the first
/// `limit`, each as `path: a vs b` (with the relative difference for two numbers).
pub fn differences(a: &Value, b: &Value, limit: usize) -> (usize, Vec<String>) {
    fn walk(a: &Value, b: &Value, path: &str, n: &mut usize, out: &mut Vec<String>, limit: usize) {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
                keys.sort();
                keys.dedup();
                for k in keys {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    walk(
                        x.get(k).unwrap_or(&Value::Null),
                        y.get(k).unwrap_or(&Value::Null),
                        &p,
                        n,
                        out,
                        limit,
                    );
                }
            }
            (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
                for (i, (u, v)) in x.iter().zip(y).enumerate() {
                    walk(u, v, &format!("{path}[{i}]"), n, out, limit);
                }
            }
            _ if a == b => {}
            _ => {
                *n += 1;
                if out.len() < limit {
                    let rel = match (a.as_f64(), b.as_f64()) {
                        (Some(x), Some(y)) if x != 0.0 || y != 0.0 => format!(
                            " (relative difference {:.1e})",
                            (x - y).abs() / x.abs().max(y.abs())
                        ),
                        _ => String::new(),
                    };
                    out.push(format!("{path}: {} vs {}{rel}", fmt(a), fmt(b)));
                }
            }
        }
    }
    let mut n = 0;
    let mut out = Vec::new();
    walk(a, b, "", &mut n, &mut out, limit);
    (n, out)
}

fn check_files(dir: &Path, files: &[(String, String)], source: &str, report: &mut CheckReport) {
    for (path, recorded) in files {
        if !safe(path) {
            report.problems.push(format!(
                "{path}: {source} names a path outside the dossier directory"
            ));
            continue;
        }
        match std::fs::read(dir.join(path)) {
            Err(_) => report
                .problems
                .push(format!("{path}: missing ({source} lists it)")),
            Ok(bytes) => {
                report.files_checked += 1;
                let actual = sha256_hex(&bytes);
                if &actual != recorded {
                    report.problems.push(format!(
                        "{path}: SHA-256 is {actual}, {source} records {recorded}"
                    ));
                }
            }
        }
    }
}

/// Check the dossier in `dir`. With `rerun`, every stored scenario is run again under
/// that entitlement and compared.
pub fn check(dir: &Path, rerun: Option<(&Entitlement, i64)>) -> Result<CheckReport, KshanaError> {
    let dossier_path = dir.join("dossier.json");
    let text = std::fs::read_to_string(&dossier_path)
        .map_err(|e| err(format!("could not read {}: {e}", dossier_path.display())))?;
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| err(format!("{} is not JSON: {e}", dossier_path.display())))?;
    let body = doc
        .get("body")
        .ok_or_else(|| err("dossier.json has no `body`: it is not a mission dossier"))?;
    if body.get("schema").and_then(Value::as_str) != Some(super::compose::DOSSIER_SCHEMA) {
        return Err(err(format!(
            "dossier.json is not a mission dossier (its schema is not `{}`)",
            super::compose::DOSSIER_SCHEMA
        )));
    }
    let recorded = doc
        .get("dossier_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut report = CheckReport {
        dossier_sha256: recorded.clone(),
        ..Default::default()
    };

    // 1. The body against its own hash.
    let canonical = serde_json::to_string(body).map_err(|e| err(e.to_string()))?;
    let actual = sha256_hex(canonical.as_bytes());
    if actual != recorded {
        report.problems.push(format!(
            "dossier.json: the body hashes to {actual}, but dossier_sha256 says {recorded}"
        ));
    }

    // 2. The files the body lists.
    let body_files = listed(body.get("identification").and_then(|i| i.get("files")));
    if body_files.is_empty() {
        report
            .problems
            .push("dossier.json: the body lists no files".into());
    }
    check_files(dir, &body_files, "dossier.json", &mut report);

    // 3. The manifest.
    match std::fs::read_to_string(dir.join("manifest.json")) {
        Err(_) => report.problems.push("manifest.json: missing".into()),
        Ok(t) => match serde_json::from_str::<Value>(&t) {
            Err(e) => report
                .problems
                .push(format!("manifest.json: not JSON ({e})")),
            Ok(m) => {
                if m.get("dossier_sha256").and_then(Value::as_str) != Some(recorded.as_str()) {
                    report
                        .problems
                        .push("manifest.json: names another dossier hash than dossier.json".into());
                }
                let files = listed(m.get("files"));
                for (p, h) in &body_files {
                    if !files.iter().any(|(mp, mh)| mp == p && mh == h) {
                        report.problems.push(format!(
                            "manifest.json: does not list {p} with the hash dossier.json records"
                        ));
                    }
                }
                for face in ["dossier.json", "dossier.html", "dossier.pdf"] {
                    if !files.iter().any(|(p, _)| p == face) {
                        report
                            .problems
                            .push(format!("manifest.json: does not list {face}"));
                    }
                }
                check_files(dir, &files, "manifest.json", &mut report);
            }
        },
    }

    // 4. The ledger.
    match std::fs::read_to_string(dir.join("ledger.json")) {
        Err(_) => report.problems.push("ledger.json: missing".into()),
        Ok(t) => match StudyLedger::from_json(&t) {
            Err(e) => report.problems.push(format!("ledger.json: {e}")),
            Ok(l) => {
                if !l.verify() {
                    report.problems.push(
                        "ledger.json: the hash chain does not verify; a record was altered".into(),
                    );
                }
                match l.records().last() {
                    Some(r) if r.run_hash.as_deref() == Some(recorded.as_str()) => {}
                    Some(_) => report.problems.push(
                        "ledger.json: the last record does not carry this dossier's hash".into(),
                    ),
                    None => report.problems.push("ledger.json: no records".into()),
                }
            }
        },
    }

    // 5. Re-run every stored scenario.
    if let Some((ent, now)) = rerun {
        let records = body
            .get("reproducibility")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if records.is_empty() {
            report
                .problems
                .push("dossier.json: no reproducibility record to re-run".into());
        }
        for r in &records {
            let run = r
                .get("run")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let at = |k: &str, f: &str| {
                r.get(k)
                    .and_then(|x| x.get(f))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            };
            let (scenario, result) = (at("scenario", "path"), at("result", "path"));
            let recorded_sha = at("result", "sha256");
            let mut rr = Rerun {
                run: run.clone(),
                matches: false,
                recorded_sha256: recorded_sha.clone(),
                rerun_sha256: None,
                differences: vec![],
            };
            if !safe(&scenario) || !safe(&result) {
                report.problems.push(format!(
                    "{run}: the record names a path outside the dossier directory"
                ));
                report.reruns.push(rr);
                continue;
            }
            let outcome = std::fs::read_to_string(dir.join(&scenario))
                .map_err(|e| format!("could not read {scenario}: {e}"))
                .and_then(|src| {
                    crate::registry::run_gated(&src, ent, now).map_err(|e| e.to_string())
                });
            match outcome {
                Err(e) => report.problems.push(format!("{run}: re-run failed: {e}")),
                Ok(out) => {
                    let sha = sha256_hex(out.json.as_bytes());
                    rr.matches = sha == recorded_sha;
                    rr.rerun_sha256 = Some(sha.clone());
                    if !rr.matches {
                        let stored: Option<Value> = std::fs::read(dir.join(&result))
                            .ok()
                            .and_then(|b| serde_json::from_slice(&b).ok());
                        let fresh: Option<Value> = serde_json::from_str(&out.json).ok();
                        if let (Some(a), Some(b)) = (stored, fresh) {
                            let (n, first) = differences(&a, &b, 8);
                            rr.differences = first;
                            if n > rr.differences.len() {
                                rr.differences
                                    .push(format!("and {} more", n - rr.differences.len()));
                            }
                        }
                        report.problems.push(format!(
                            "{run}: the re-run result hashes to {sha}, the dossier records {recorded_sha}{}",
                            if rr.differences.is_empty() {
                                String::new()
                            } else {
                                format!(" (stored vs re-run: {})", rr.differences.join("; "))
                            }
                        ));
                    }
                }
            }
            report.reruns.push(rr);
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_must_stay_inside_the_directory() {
        assert!(safe("runs/RUN-A/a.result.json"));
        assert!(safe("trace.toml"));
        for bad in ["", "/etc/passwd", "../x", "runs/../../x"] {
            assert!(!safe(bad), "{bad:?}");
        }
    }

    #[test]
    fn differences_name_the_field_and_the_size() {
        let a = json!({"x": {"y": [1.0, 2.0]}, "k": "a", "same": 3});
        let b = json!({"x": {"y": [1.0, 2.000002]}, "k": "b", "same": 3});
        let (n, d) = differences(&a, &b, 10);
        assert_eq!(n, 2);
        assert!(
            d.iter()
                .any(|s| s.starts_with("x.y[1]: 2.0 vs 2.000002") && s.contains("1.0e-6")),
            "{d:?}"
        );
        assert!(
            d.iter().any(|s| s.starts_with("k: \"a\" vs \"b\"")),
            "{d:?}"
        );
        assert_eq!(differences(&a, &a, 10).0, 0);
        assert_eq!(differences(&a, &b, 1).1.len(), 1);
    }
}
