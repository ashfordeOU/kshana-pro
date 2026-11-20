// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Regression diff between two builds' verdict directories.
//!
//! Per scenario: the verdict change (`regression`: pass → fail, or pass → incomplete/not-run;
//! `fixed`: fail or incomplete → pass; `now-run`: not run before; `worse` / `better` for the
//! other moves; `unchanged`; `new`; `removed`), whether the scenario itself changed
//! between the builds (its SHA-256: a changed scenario makes the comparison one of different
//! tests, and is flagged), and per criterion the measured values and their delta. A metric
//! that got worse while still within its bar is reported as `worse`, not as a regression.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::score::ScenarioVerdict;
use super::spec::Op;
use crate::license::KEY_LEN;
use crate::scorecards::levels::Verdict;

pub const DIFF_FORMAT: &str = "kshana-testkit-diff/1";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CriterionDelta {
    pub id: String,
    pub from: Option<f64>,
    pub to: Option<f64>,
    pub delta: Option<f64>,
    pub from_verdict: Option<Verdict>,
    pub to_verdict: Option<Verdict>,
    /// `better`, `worse`, `same` or `n/a`.
    pub trend: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenarioDelta {
    pub scenario: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub change: String,
    pub scenario_changed: bool,
    pub criteria: Vec<CriterionDelta>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diff {
    pub format: &'static str,
    pub label: &'static str,
    pub from_build: String,
    pub to_build: String,
    pub regressions: usize,
    pub fixed: usize,
    pub scenarios: Vec<ScenarioDelta>,
    /// Signature problems found when trust keys were given.
    pub signature_problems: Vec<String>,
}

/// Every scenario verdict of a build directory, by id.
pub fn load_build(dir: &Path) -> Result<(String, BTreeMap<String, ScenarioVerdict>), String> {
    let s: super::run::Summary = serde_json::from_str(
        &std::fs::read_to_string(dir.join("summary.json"))
            .map_err(|e| format!("{}: {e}", dir.display()))?,
    )
    .map_err(|e| format!("{}/summary.json: {e}", dir.display()))?;
    let mut m = BTreeMap::new();
    for r in &s.scenarios {
        let p = dir.join(&r.scenario).join("verdict.json");
        let v: ScenarioVerdict = serde_json::from_str(
            &std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
        )
        .map_err(|e| format!("{}: {e}", p.display()))?;
        m.insert(r.scenario.clone(), v);
    }
    Ok((s.build, m))
}

fn rank(v: &str) -> u8 {
    match v {
        "pass" => 3,
        "incomplete" => 2,
        "fail" => 1,
        _ => 0,
    }
}

/// Compare build `a` (before) with build `b` (after). With `trust`, both directories'
/// signatures (build and every scenario) are verified first and problems listed.
pub fn diff(a: &Path, b: &Path, trust: &[[u8; KEY_LEN]]) -> Result<Diff, String> {
    let mut sig = Vec::new();
    if !trust.is_empty() {
        for d in [a, b] {
            let mut dirs = vec![d.to_path_buf()];
            if let Ok(rd) = std::fs::read_dir(d) {
                dirs.extend(
                    rd.filter_map(|e| e.ok().map(|e| e.path()))
                        .filter(|p| p.is_dir()),
                );
            }
            for x in dirs {
                match crate::signed_dossier::verify(&x, trust) {
                    Ok(v) if v.passed() => {}
                    Ok(v) => sig.push(format!(
                        "{}: {}",
                        x.display(),
                        if v.problems.is_empty() {
                            "signature not trusted".into()
                        } else {
                            v.problems.join("; ")
                        }
                    )),
                    Err(e) => sig.push(format!("{}: {e}", x.display())),
                }
            }
        }
    }
    let (fa, ma) = load_build(a)?;
    let (fb, mb) = load_build(b)?;
    let mut ids: Vec<&String> = ma.keys().chain(mb.keys()).collect();
    ids.sort();
    ids.dedup();
    let mut out = Vec::new();
    for id in ids {
        let (va, vb) = (ma.get(id), mb.get(id));
        let change = match (va, vb) {
            (None, Some(_)) => "new",
            (Some(_), None) => "removed",
            (Some(x), Some(y)) => {
                let (rx, ry) = (rank(&x.verdict), rank(&y.verdict));
                if x.verdict == "not-run" && y.verdict != "not-run" {
                    "now-run"
                } else if x.verdict == "pass" && ry < rx {
                    "regression"
                } else if ry > rx && y.verdict == "pass" {
                    "fixed"
                } else if x.verdict == y.verdict {
                    "unchanged"
                } else if ry < rx {
                    "worse"
                } else {
                    "better"
                }
            }
            (None, None) => unreachable!(),
        };
        let mut criteria = Vec::new();
        if let (Some(x), Some(y)) = (va, vb) {
            let mut cids: Vec<&String> = x
                .criteria
                .iter()
                .map(|c| &c.id)
                .chain(y.criteria.iter().map(|c| &c.id))
                .collect();
            cids.sort();
            cids.dedup();
            for cid in cids {
                let cx = x.criteria.iter().find(|c| &c.id == cid);
                let cy = y.criteria.iter().find(|c| &c.id == cid);
                let (mx, my) = (cx.and_then(|c| c.measured), cy.and_then(|c| c.measured));
                let delta = match (mx, my) {
                    (Some(p), Some(q)) => Some(q - p),
                    _ => None,
                };
                let op = cy
                    .or(cx)
                    .map(|c| if c.op == ">=" { Op::Ge } else { Op::Le });
                let trend = match (delta, op) {
                    (Some(d), _) if d.abs() < 1e-9 => "same",
                    (Some(d), Some(Op::Le)) => {
                        if d < 0.0 {
                            "better"
                        } else {
                            "worse"
                        }
                    }
                    (Some(d), Some(Op::Ge)) => {
                        if d > 0.0 {
                            "better"
                        } else {
                            "worse"
                        }
                    }
                    _ => "n/a",
                };
                criteria.push(CriterionDelta {
                    id: cid.clone(),
                    from: mx,
                    to: my,
                    delta,
                    from_verdict: cx.map(|c| c.verdict),
                    to_verdict: cy.map(|c| c.verdict),
                    trend: trend.into(),
                });
            }
        }
        out.push(ScenarioDelta {
            scenario: id.clone(),
            from: va.map(|v| v.verdict.clone()),
            to: vb.map(|v| v.verdict.clone()),
            change: change.into(),
            scenario_changed: matches!((va, vb), (Some(x), Some(y)) if x.scenario_sha256 != y.scenario_sha256),
            criteria,
        });
    }
    Ok(Diff {
        format: DIFF_FORMAT,
        label: super::LABEL,
        from_build: fa,
        to_build: fb,
        regressions: out.iter().filter(|s| s.change == "regression").count(),
        fixed: out.iter().filter(|s| s.change == "fixed").count(),
        scenarios: out,
        signature_problems: sig,
    })
}

/// A short Markdown table of a diff (for a CI job summary).
pub fn markdown(d: &Diff) -> String {
    let mut s = format!(
        "## Test kit: {} → {}\n\n{} regression(s), {} fixed. MODELLED.\n\n| scenario | before | after | change | scenario changed |\n|---|---|---|---|---|\n",
        d.from_build, d.to_build, d.regressions, d.fixed
    );
    for x in &d.scenarios {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            x.scenario,
            x.from.as_deref().unwrap_or("-"),
            x.to.as_deref().unwrap_or("-"),
            x.change,
            if x.scenario_changed { "yes" } else { "no" }
        ));
    }
    for p in &d.signature_problems {
        s.push_str(&format!("\nSignature problem: {p}\n"));
    }
    s
}
