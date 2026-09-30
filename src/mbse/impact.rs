// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Change impact: which requirements need re-verification.
//!
//! Two forms:
//!
//! - **Two traces** (for example the same programme run on two engine versions, or
//!   after a scenario change): a requirement needs re-verification when a run linked to
//!   it has a different `scenario_hash` (or, for a kind whose result carries none, a
//!   different scenario SHA-256), a different `engine_version`, or when a capability
//!   label its evidence rests on changed between the two verification matrices (or the
//!   two runs' reports). A changed criterion, method or link set also counts. The two
//!   matrices are evaluated and verdict changes are listed.
//! - **Two scenario files**: the files are compared by content (parsed, so comments and
//!   layout do not count) and the changed keys listed; every requirement linked to a
//!   run of the old file needs re-verification. The new `scenario_hash` itself only
//!   exists once the engine has run the new file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;

use super::evidence::{sha256_hex, Run, VerificationMatrix};
use super::trace::{vcrm, Trace};
use kshana::api::KshanaError;

/// A requirement that needs re-verification, and why.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Reverify {
    pub requirement: String,
    pub reasons: Vec<String>,
    pub old_verdict: Option<String>,
    pub new_verdict: Option<String>,
}

/// What changed in one run between the two traces.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RunChange {
    pub run: String,
    pub changes: Vec<String>,
}

/// The impact report.
/// The honesty label every change-impact report carries, in the open engine's `label` form.
pub const IMPACT_LABEL: &str = "MODELLED - a change-impact list over a trace of open-engine simulation runs: it names the requirements to verify again after a change. It verifies nothing itself, and every verdict it refers to rests on modelled runs.";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Impact {
    /// The honesty label, [`IMPACT_LABEL`].
    pub label: &'static str,
    pub old: String,
    pub new: String,
    pub reverify: Vec<Reverify>,
    pub unaffected: Vec<String>,
    pub run_changes: Vec<RunChange>,
    pub added_requirements: Vec<String>,
    pub removed_requirements: Vec<String>,
    /// For the scenario form: the keys whose values differ.
    pub changed_scenario_keys: Vec<String>,
}

fn scenario_identity(r: &Run) -> (Option<String>, &'static str) {
    if let Some(h) = &r.stamp.scenario_hash {
        return (Some(h.clone()), "scenario_hash");
    }
    (
        r.stamp
            .scenario_sha256_recorded
            .clone()
            .or_else(|| r.stamp.scenario_sha256_actual.clone()),
        "scenario SHA-256 (this kind's result carries no scenario_hash)",
    )
}

/// Labels a run's evidence can rest on: each capability row it lists, read through the
/// trace's matrix when there is one.
fn labels(run: &Run, matrix: Option<&VerificationMatrix>) -> BTreeMap<String, String> {
    run.capability_rows
        .iter()
        .map(|r| {
            (
                r.requirement.clone(),
                matrix
                    .and_then(|m| m.label(&r.requirement))
                    .unwrap_or(&r.label)
                    .to_string(),
            )
        })
        .collect()
}

fn run_changes(
    old: &Run,
    new: &Run,
    om: Option<&VerificationMatrix>,
    nm: Option<&VerificationMatrix>,
) -> Vec<String> {
    let mut out = Vec::new();
    let (oh, what) = scenario_identity(old);
    let (nh, _) = scenario_identity(new);
    if oh != nh {
        out.push(format!(
            "{what} changed: {} → {}",
            oh.unwrap_or_else(|| "none".into()),
            nh.unwrap_or_else(|| "none".into())
        ));
    }
    if old.stamp.engine_version != new.stamp.engine_version {
        out.push(format!(
            "engine_version changed: {} → {}",
            old.stamp
                .engine_version
                .clone()
                .unwrap_or_else(|| "none".into()),
            new.stamp
                .engine_version
                .clone()
                .unwrap_or_else(|| "none".into())
        ));
    }
    if old.stamp.seed != new.stamp.seed {
        out.push(format!(
            "seed changed: {} → {}",
            old.stamp
                .seed
                .map(|s| s.to_string())
                .unwrap_or_else(|| "none".into()),
            new.stamp
                .seed
                .map(|s| s.to_string())
                .unwrap_or_else(|| "none".into())
        ));
    }
    // No stamp told the runs apart (a kind with no scenario_hash and a trace with no
    // report or scenario file, or identical stamps): the result file itself is the
    // evidence, so a different result file is a change.
    if out.is_empty() && old.stamp.result_sha256_actual != new.stamp.result_sha256_actual {
        out.push(format!(
            "result file changed although no stamp differs: SHA-256 {} → {}",
            old.stamp.result_sha256_actual, new.stamp.result_sha256_actual
        ));
    }
    let ol = labels(old, om);
    let nl = labels(new, nm);
    for (cap, l) in &nl {
        match ol.get(cap) {
            Some(o) if o != l => out.push(format!("capability '{cap}' label changed: {o} → {l}")),
            None => out.push(format!("capability '{cap}' is newly exercised ({l})")),
            _ => {}
        }
    }
    for (cap, o) in &ol {
        if !nl.contains_key(cap) {
            out.push(format!("capability '{cap}' ({o}) is no longer exercised"));
        }
    }
    out
}

fn pinned_label(cap: &str, m: Option<&VerificationMatrix>, runs: &[&Run]) -> Option<String> {
    m.and_then(|m| m.label(cap).map(str::to_string))
        .or_else(|| {
            runs.iter()
                .flat_map(|r| r.capability_rows.iter())
                .find(|r| r.requirement == cap)
                .map(|r| r.label.clone())
        })
}

/// Compare two traces.
pub fn compare(old: &Trace, new: &Trace) -> Impact {
    let ov = vcrm(old);
    let nv = vcrm(new);
    let verdict = |v: &super::trace::Vcrm, id: &str| {
        v.rows
            .iter()
            .find(|r| r.requirement == id)
            .map(|r| r.verdict.as_str().to_string())
    };
    let mut per_run: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut run_change_list = Vec::new();
    for nr in &new.runs {
        match old.run(&nr.id) {
            Some(or) => {
                let ch = run_changes(or, nr, old.matrix.as_ref(), new.matrix.as_ref());
                if !ch.is_empty() {
                    run_change_list.push(RunChange {
                        run: nr.id.clone(),
                        changes: ch.clone(),
                    });
                }
                per_run.insert(nr.id.clone(), ch);
            }
            None => {
                let ch = vec!["run is new in this trace".to_string()];
                run_change_list.push(RunChange {
                    run: nr.id.clone(),
                    changes: ch.clone(),
                });
                per_run.insert(nr.id.clone(), ch);
            }
        }
    }
    let mut reverify = Vec::new();
    let mut unaffected = Vec::new();
    for req in &new.requirements {
        let mut reasons = Vec::new();
        let old_req = old.requirements.iter().find(|r| r.id == req.id);
        if let Some(o) = old_req {
            if o.criterion != req.criterion {
                reasons.push(format!(
                    "criterion changed: `{}` → `{}`",
                    o.criterion.clone().unwrap_or_default(),
                    req.criterion.clone().unwrap_or_default()
                ));
            }
            if o.method != req.method {
                reasons.push(format!(
                    "method changed: {} → {}",
                    o.method.as_str(),
                    req.method.as_str()
                ));
            }
        }
        let old_links: BTreeSet<&String> = old.links.get(&req.id).into_iter().flatten().collect();
        let new_links: BTreeSet<&String> = new.links.get(&req.id).into_iter().flatten().collect();
        for r in new_links.difference(&old_links) {
            reasons.push(format!("run {r} newly linked"));
        }
        for r in old_links.difference(&new_links) {
            reasons.push(format!("run {r} no longer linked"));
        }
        for run_id in &new_links {
            for c in per_run.get(*run_id).into_iter().flatten() {
                reasons.push(format!("run {run_id}: {c}"));
            }
        }
        let old_runs = old.runs_for(&req.id);
        let new_runs = new.runs_for(&req.id);
        for cap in &req.capabilities {
            let o = pinned_label(cap, old.matrix.as_ref(), &old_runs);
            let n = pinned_label(cap, new.matrix.as_ref(), &new_runs);
            if o != n {
                reasons.push(format!(
                    "pinned capability '{cap}' label changed: {} → {}",
                    o.unwrap_or_else(|| "absent".into()),
                    n.unwrap_or_else(|| "absent".into())
                ));
            }
        }
        let (ov_, nv_) = (verdict(&ov, &req.id), verdict(&nv, &req.id));
        if reasons.is_empty() {
            unaffected.push(req.id.clone());
        } else {
            reverify.push(Reverify {
                requirement: req.id.clone(),
                reasons,
                old_verdict: ov_,
                new_verdict: nv_,
            });
        }
    }
    let added: Vec<String> = new
        .requirements
        .iter()
        .filter(|r| !old.requirements.iter().any(|o| o.id == r.id))
        .map(|r| r.id.clone())
        .collect();
    for a in &added {
        if let Some(x) = reverify.iter_mut().find(|x| &x.requirement == a) {
            x.reasons.insert(0, "requirement is new".into());
        } else {
            unaffected.retain(|u| u != a);
            reverify.push(Reverify {
                requirement: a.clone(),
                reasons: vec!["requirement is new".into()],
                old_verdict: None,
                new_verdict: verdict(&nv, a),
            });
        }
    }
    Impact {
        label: IMPACT_LABEL,
        old: old.title.clone(),
        new: new.title.clone(),
        reverify,
        unaffected,
        run_changes: run_change_list,
        added_requirements: added,
        removed_requirements: old
            .requirements
            .iter()
            .filter(|r| !new.requirements.iter().any(|n| n.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        changed_scenario_keys: vec![],
    }
}

fn flatten(v: &toml::Value, prefix: String, out: &mut BTreeMap<String, String>) {
    match v {
        toml::Value::Table(t) => {
            for (k, x) in t {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(x, p, out);
            }
        }
        toml::Value::Array(a) if a.iter().any(|x| x.is_table()) => {
            for (i, x) in a.iter().enumerate() {
                flatten(x, format!("{prefix}[{i}]"), out);
            }
        }
        other => {
            out.insert(prefix, other.to_string());
        }
    }
}

/// Compare two scenario files and list the requirements in `trace` that need
/// re-verification: those linked to a run of the old file.
pub fn compare_scenarios(trace: &Trace, old: &Path, new: &Path) -> Result<Impact, KshanaError> {
    let read = |p: &Path| {
        std::fs::read(p)
            .map_err(|e| KshanaError::InvalidInput(format!("could not read {}: {e}", p.display())))
    };
    let (ob, nb) = (read(old)?, read(new)?);
    let parse = |b: &[u8], p: &Path| -> Result<toml::Value, KshanaError> {
        let s = std::str::from_utf8(b)
            .map_err(|_| KshanaError::InvalidInput(format!("{} is not UTF-8", p.display())))?;
        toml::from_str(s).map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", p.display())))
    };
    let (ov, nv) = (parse(&ob, old)?, parse(&nb, new)?);
    let (mut of, mut nf) = (BTreeMap::new(), BTreeMap::new());
    flatten(&ov, String::new(), &mut of);
    flatten(&nv, String::new(), &mut nf);
    let mut changed = Vec::new();
    for (k, v) in &nf {
        match of.get(k) {
            Some(o) if o != v => changed.push(format!("{k}: {o} → {v}")),
            None => changed.push(format!("{k}: added ({v})")),
            _ => {}
        }
    }
    for (k, o) in &of {
        if !nf.contains_key(k) {
            changed.push(format!("{k}: removed (was {o})"));
        }
    }
    let old_sha = sha256_hex(&ob);
    let old_name = old.file_name().map(|f| f.to_string_lossy().to_string());
    let runs: Vec<&Run> = trace
        .runs
        .iter()
        .filter(|r| {
            r.scenario_file.as_ref().is_some_and(|f| {
                f.sha256 == old_sha
                    || r.stamp.scenario_sha256_recorded.as_deref() == Some(old_sha.as_str())
                    || Path::new(&f.path)
                        .file_name()
                        .map(|x| x.to_string_lossy().to_string())
                        == old_name
            })
        })
        .collect();
    let mut reverify = Vec::new();
    let mut unaffected = Vec::new();
    let v = vcrm(trace);
    for req in &trace.requirements {
        let hit: Vec<&str> = trace
            .runs_for(&req.id)
            .iter()
            .filter(|r| runs.iter().any(|x| x.id == r.id))
            .map(|r| r.id.as_str())
            .collect();
        if !changed.is_empty() && !hit.is_empty() {
            reverify.push(Reverify {
                requirement: req.id.clone(),
                reasons: hit
                    .iter()
                    .map(|r| format!(
                        "run {r} used {}, whose content changed in {} key(s); re-run the engine to get the new scenario_hash and re-evaluate",
                        old.display(),
                        changed.len()
                    ))
                    .collect(),
                old_verdict: v
                    .rows
                    .iter()
                    .find(|x| x.requirement == req.id)
                    .map(|x| x.verdict.as_str().to_string()),
                new_verdict: None,
            });
        } else {
            unaffected.push(req.id.clone());
        }
    }
    Ok(Impact {
        label: IMPACT_LABEL,
        old: old.display().to_string(),
        new: new.display().to_string(),
        reverify,
        unaffected,
        run_changes: runs
            .iter()
            .map(|r| RunChange {
                run: r.id.clone(),
                changes: if changed.is_empty() {
                    vec!["scenario content unchanged (comments or layout only)".into()]
                } else {
                    vec![format!(
                        "scenario content changed in {} key(s)",
                        changed.len()
                    )]
                },
            })
            .collect(),
        added_requirements: vec![],
        removed_requirements: vec![],
        changed_scenario_keys: changed,
    })
}

impl Impact {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("impact serialises")
    }

    pub fn to_markdown(&self) -> String {
        let mut s = format!(
            "# Change impact\n\nOld: {}\nNew: {}\n\n",
            self.old, self.new
        );
        if !self.changed_scenario_keys.is_empty() {
            s.push_str("## Changed scenario keys\n\n");
            for k in &self.changed_scenario_keys {
                s.push_str(&format!("- `{k}`\n"));
            }
            s.push('\n');
        }
        s.push_str(&format!(
            "## Requirements to re-verify ({})\n\n",
            self.reverify.len()
        ));
        for r in &self.reverify {
            s.push_str(&format!(
                "- **{}** (verdict before: {}, after: {})\n",
                r.requirement,
                r.old_verdict.clone().unwrap_or_else(|| "—".into()),
                r.new_verdict
                    .clone()
                    .unwrap_or_else(|| "not yet run".into())
            ));
            for why in &r.reasons {
                s.push_str(&format!("  - {why}\n"));
            }
        }
        s.push_str(&format!(
            "\n## Unaffected ({})\n\n{}\n",
            self.unaffected.len(),
            if self.unaffected.is_empty() {
                "none".to_string()
            } else {
                self.unaffected.join(", ")
            }
        ));
        s
    }
}
