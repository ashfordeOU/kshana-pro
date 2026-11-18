// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! End-to-end validation campaign runner (stream D7).
//!
//! A **campaign plan** (TOML) lists cases. Each case names a conformance case file (or
//! an inline scenario path with criteria), optional scenario overrides, which receivers to
//! evaluate (the software receiver and/or external receiver logs imported with a CSV
//! profile), and **pass bars written before running** (pre-registration: the plan's SHA-256
//! is recorded before any simulation runs, and the bars cannot change afterwards without
//! changing that hash).
//!
//! The runner simulates each case/variation in memory, evaluates the receivers with the
//! conformance library ([`super::conformance`]), scores the metrics against the case
//! criteria plus the campaign bars, and writes a campaign directory:
//!
//! | file | content |
//! |---|---|
//! | `plan.toml` | the plan exactly as given |
//! | `cases/<case>__<variation>__<receiver>.json` | metrics and scored criteria |
//! | `summary.csv` | one row per case/variation/receiver with the verdict |
//! | `rvm.csv` | requirement ↔ case coverage and verdicts |
//! | `campaign.json` | plan SHA-256, per-file SHA-256 in write order and a hash chain over them, overall counts |
//!
//! [`verify_campaign`] recomputes every file hash and the chain, so any modification or
//! deletion of a result file is detected.

use super::conformance::{
    evaluate_external, evaluate_software, overall, score, simulate, ConformanceFile, Criterion,
    Scored, Verdict,
};
use crate::field::importers::CsvProfile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// An external receiver: its log file per variation (or one file for all) and the CSV
/// profile that maps its columns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalReceiver {
    pub name: String,
    /// CSV profile file (TOML) for [`CsvProfile`].
    pub profile: PathBuf,
    /// Log file for every variation, or per variation label.
    #[serde(default)]
    pub log: Option<PathBuf>,
    #[serde(default)]
    pub logs: BTreeMap<String, PathBuf>,
    /// Maximum time difference when matching records to truth (s).
    #[serde(default = "d_dt")]
    pub max_dt_s: f64,
}

fn d_dt() -> f64 {
    0.5
}

/// One campaign case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignCase {
    pub id: String,
    /// Conformance case file (relative to the plan's directory, or absolute).
    pub file: PathBuf,
    /// Variation labels to run (empty = all of the case's variations).
    #[serde(default)]
    pub variations: Vec<String>,
    /// Scenario overrides (TOML path → value) applied to every variation (e.g. shorter
    /// `scene.duration_s`, other seed).
    #[serde(default)]
    pub set: BTreeMap<String, toml::Value>,
    /// Evaluate the built-in software receiver (default true).
    #[serde(default = "yes")]
    pub software_receiver: bool,
    #[serde(default)]
    pub external: Vec<ExternalReceiver>,
    /// Campaign pass bars, added to the case's own criteria (pre-registered).
    #[serde(default)]
    pub bars: Vec<Criterion>,
    /// Requirement ids this case verifies (added to the case file's own).
    #[serde(default)]
    pub requirements: Vec<String>,
}

fn yes() -> bool {
    true
}

/// A campaign plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignPlan {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub cases: Vec<CampaignCase>,
}

impl CampaignPlan {
    pub fn from_toml(src: &str) -> Result<CampaignPlan, String> {
        let p: CampaignPlan = toml::from_str(src).map_err(|e| format!("campaign plan: {e}"))?;
        if p.cases.is_empty() {
            return Err("campaign plan: no cases".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for c in &p.cases {
            if !ids.insert(c.id.clone()) {
                return Err(format!("campaign plan: duplicate case id {}", c.id));
            }
        }
        Ok(p)
    }
}

/// Result of one case/variation/receiver.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    pub case: String,
    pub conformance_id: String,
    pub variation: String,
    pub receiver: String,
    pub metrics: BTreeMap<String, f64>,
    pub scored: Vec<Scored>,
    pub verdict: Verdict,
    pub requirements: Vec<String>,
    /// Set when the case could not be run (the verdict is then NOT_EVALUATED).
    #[serde(default)]
    pub error: Option<String>,
}

/// Summary of a campaign.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignSummary {
    pub name: String,
    pub plan_sha256: String,
    pub results: Vec<CaseResult>,
    pub pass: usize,
    pub fail: usize,
    pub not_evaluated: usize,
}

/// One file of the manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub sha256: String,
    /// SHA-256 over (previous chain value ‖ this file's hash).
    pub chain: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CampaignManifest {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub plan_sha256: String,
    pub files: Vec<ManifestEntry>,
    pub pass: usize,
    pub fail: usize,
    pub not_evaluated: usize,
    pub evidence_label: String,
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn slug(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

/// Run a campaign whose plan text is `plan_src` (paths resolved against `plan_dir`) and
/// write the campaign directory `out_dir` (created; must not already contain a manifest).
pub fn run_campaign(plan_src: &str, plan_dir: &Path, out_dir: &Path) -> Result<CampaignSummary, String> {
    let plan = CampaignPlan::from_toml(plan_src)?;
    let plan_sha = sha(plan_src.as_bytes());
    if out_dir.join("campaign.json").exists() {
        return Err(format!("{}: already holds a campaign; use a new directory", out_dir.display()));
    }
    std::fs::create_dir_all(out_dir.join("cases")).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut files: Vec<(String, Vec<u8>)> = vec![("plan.toml".into(), plan_src.as_bytes().to_vec())];
    let mut results = Vec::new();
    for case in &plan.cases {
        let path = if case.file.is_absolute() { case.file.clone() } else { plan_dir.join(&case.file) };
        let cf = match ConformanceFile::load(&path) {
            Ok(c) => c,
            Err(e) => {
                results.push(CaseResult {
                    case: case.id.clone(),
                    conformance_id: String::new(),
                    variation: String::new(),
                    receiver: "-".into(),
                    metrics: BTreeMap::new(),
                    scored: vec![],
                    verdict: Verdict::NotEvaluated,
                    requirements: case.requirements.clone(),
                    error: Some(e),
                });
                continue;
            }
        };
        let mut criteria = cf.case.criteria.clone();
        criteria.extend(case.bars.iter().cloned());
        let mut reqs = cf.case.requirements.clone();
        reqs.extend(case.requirements.iter().cloned());
        reqs.sort();
        reqs.dedup();
        let labels: Vec<Option<String>> = if case.variations.is_empty() {
            cf.variation_labels()
        } else {
            case.variations.iter().map(|v| Some(v.clone())).collect()
        };
        for label in labels {
            let vname = label.clone().unwrap_or_else(|| "default".into());
            let push_err = |results: &mut Vec<CaseResult>, receiver: &str, e: String| {
                results.push(CaseResult {
                    case: case.id.clone(),
                    conformance_id: cf.case.id.clone(),
                    variation: vname.clone(),
                    receiver: receiver.into(),
                    metrics: BTreeMap::new(),
                    scored: vec![],
                    verdict: Verdict::NotEvaluated,
                    requirements: reqs.clone(),
                    error: Some(e),
                });
            };
            let run = match cf.scenario_text(label.as_deref(), &case.set).and_then(|t| simulate(&t)) {
                Ok(r) => r,
                Err(e) => {
                    push_err(&mut results, "-", e);
                    continue;
                }
            };
            if case.software_receiver {
                match evaluate_software(&cf.case, &criteria, &run) {
                    Ok((m, _)) => {
                        let s = score(&criteria, &vname, &m);
                        results.push(CaseResult {
                            case: case.id.clone(),
                            conformance_id: cf.case.id.clone(),
                            variation: vname.clone(),
                            receiver: "software".into(),
                            verdict: overall(&s),
                            metrics: m,
                            scored: s,
                            requirements: reqs.clone(),
                            error: None,
                        });
                    }
                    Err(e) => push_err(&mut results, "software", e),
                }
            }
            for ext in &case.external {
                let log = ext.logs.get(&vname).or(ext.log.as_ref());
                let outcome = (|| -> Result<BTreeMap<String, f64>, String> {
                    let log = log.ok_or_else(|| format!("no log for variation {vname}"))?;
                    let rp = |p: &PathBuf| if p.is_absolute() { p.clone() } else { plan_dir.join(p) };
                    let prof_txt = std::fs::read_to_string(rp(&ext.profile)).map_err(|e| format!("{}: {e}", ext.profile.display()))?;
                    let prof = CsvProfile::from_toml(&prof_txt)?;
                    let text = std::fs::read_to_string(rp(log)).map_err(|e| format!("{}: {e}", log.display()))?;
                    let (obs, _rep) = prof.import(&text, &ext.name)?;
                    evaluate_external(&cf.case, &run, &obs, ext.max_dt_s)
                })();
                match outcome {
                    Ok(m) => {
                        let s = score(&criteria, &vname, &m);
                        results.push(CaseResult {
                            case: case.id.clone(),
                            conformance_id: cf.case.id.clone(),
                            variation: vname.clone(),
                            receiver: ext.name.clone(),
                            verdict: overall(&s),
                            metrics: m,
                            scored: s,
                            requirements: reqs.clone(),
                            error: None,
                        });
                    }
                    Err(e) => push_err(&mut results, &ext.name, e),
                }
            }
        }
    }
    // per-case files
    for r in &results {
        let name = format!("cases/{}__{}__{}.json", slug(&r.case), slug(&r.variation), slug(&r.receiver));
        files.push((name, serde_json::to_vec_pretty(r).map_err(|e| e.to_string())?));
    }
    // summary
    let mut csv = String::from("case,conformance_id,variation,receiver,verdict,failed_metrics,error\n");
    for r in &results {
        let failed: Vec<String> = r.scored.iter().filter(|s| s.verdict == Verdict::Fail).map(|s| s.metric.clone()).collect();
        csv.push_str(&format!(
            "{},{},{},{},{:?},{},{}\n",
            r.case,
            r.conformance_id,
            r.variation,
            r.receiver,
            r.verdict,
            failed.join(";"),
            r.error.as_deref().unwrap_or("").replace(',', ";").replace('\n', " ")
        ));
    }
    files.push(("summary.csv".into(), csv.into_bytes()));
    files.push(("rvm.csv".into(), rvm_csv(&results).into_bytes()));
    let pass = results.iter().filter(|r| r.verdict == Verdict::Pass).count();
    let fail = results.iter().filter(|r| r.verdict == Verdict::Fail).count();
    let ne = results.len() - pass - fail;
    // write files and the hash-chained manifest
    let mut chain = String::new();
    let mut entries = Vec::new();
    for (p, bytes) in &files {
        let full = out_dir.join(p);
        if let Some(d) = full.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&full, bytes).map_err(|e| format!("{}: {e}", full.display()))?;
        let h = sha(bytes);
        chain = sha(format!("{chain}{h}").as_bytes());
        entries.push(ManifestEntry { path: p.clone(), sha256: h, chain: chain.clone() });
    }
    let manifest = CampaignManifest {
        format: "kshana-eloran-campaign".into(),
        version: 1,
        name: plan.name.clone(),
        plan_sha256: plan_sha.clone(),
        files: entries,
        pass,
        fail,
        not_evaluated: ne,
        evidence_label: "MODELLED".into(),
    };
    std::fs::write(out_dir.join("campaign.json"), serde_json::to_vec_pretty(&manifest).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(CampaignSummary { name: plan.name, plan_sha256: plan_sha, results, pass, fail, not_evaluated: ne })
}

/// Requirement verification matrix: one row per requirement with the cases that cover it and
/// the combined verdict (FAIL if any fail, PASS if all pass, else NOT_EVALUATED).
pub fn rvm_csv(results: &[CaseResult]) -> String {
    let mut by: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for r in results {
        for q in &r.requirements {
            by.entry(q.clone()).or_default().push(r);
        }
    }
    let mut s = String::from("requirement_id,cases,verdict\n");
    for (q, rs) in by {
        let mut cases: Vec<String> = rs.iter().map(|r| format!("{}/{}/{}", r.case, r.variation, r.receiver)).collect();
        cases.dedup();
        let v = if rs.iter().any(|r| r.verdict == Verdict::Fail) {
            "FAIL"
        } else if rs.iter().all(|r| r.verdict == Verdict::Pass) {
            "PASS"
        } else {
            "NOT_EVALUATED"
        };
        s.push_str(&format!("{q},{},{v}\n", cases.join(";")));
    }
    s
}

/// Verify a campaign directory: every file hash and the chain match the manifest, and the
/// plan file matches the recorded plan hash. Returns the manifest.
pub fn verify_campaign(dir: &Path) -> Result<CampaignManifest, String> {
    let m: CampaignManifest = serde_json::from_slice(
        &std::fs::read(dir.join("campaign.json")).map_err(|e| format!("campaign.json: {e}"))?,
    )
    .map_err(|e| format!("campaign.json: {e}"))?;
    let mut chain = String::new();
    for e in &m.files {
        let bytes = std::fs::read(dir.join(&e.path)).map_err(|x| format!("{}: {x}", e.path))?;
        let h = sha(&bytes);
        if h != e.sha256 {
            return Err(format!("{}: content changed (hash mismatch)", e.path));
        }
        chain = sha(format!("{chain}{h}").as_bytes());
        if chain != e.chain {
            return Err(format!("{}: hash chain broken", e.path));
        }
    }
    let plan = std::fs::read(dir.join("plan.toml")).map_err(|e| format!("plan.toml: {e}"))?;
    if sha(&plan) != m.plan_sha256 {
        return Err("plan.toml does not match the recorded plan hash".into());
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib_case() -> PathBuf {
        super::super::conformance::library_dir().join("position-accuracy.toml")
    }

    fn plan(extra_bar_limit: f64) -> String {
        format!(
            r#"
name = "ci-campaign"
description = "small campaign run in CI"
[[cases]]
id = "pos"
file = "{}"
set = {{ "scene.duration_s" = 1.2 }}
requirements = ["REQ-POS-1"]
[[cases.bars]]
metric = "position_error_m"
op = "le"
limit = {extra_bar_limit}
note = "campaign bar"
"#,
            lib_case().display()
        )
    }

    #[test]
    fn campaign_runs_writes_manifest_and_detects_tampering() {
        let src = plan(1000.0);
        let dir = std::env::temp_dir().join(format!("kshana-campaign-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let s = run_campaign(&src, Path::new("."), &dir).unwrap();
        assert!(!s.results.is_empty());
        assert!(s.results.iter().all(|r| r.receiver == "software"));
        assert!(s.results.iter().all(|r| r.error.is_none()), "{:?}", s.results.iter().map(|r| &r.error).collect::<Vec<_>>());
        assert!(s.results.iter().any(|r| r.scored.iter().any(|x| x.note == "campaign bar")));
        assert_eq!(s.pass + s.fail + s.not_evaluated, s.results.len());
        let m = verify_campaign(&dir).unwrap();
        assert_eq!(m.plan_sha256, sha(src.as_bytes()));
        assert!(m.files.iter().any(|f| f.path == "rvm.csv"));
        let rvm = std::fs::read_to_string(dir.join("rvm.csv")).unwrap();
        assert!(rvm.contains("REQ-POS-1"));
        // a second run into the same directory is refused
        assert!(run_campaign(&src, Path::new("."), &dir).is_err());
        // tamper with a result file
        let case_file = m.files.iter().find(|f| f.path.starts_with("cases/")).unwrap();
        let p = dir.join(&case_file.path);
        let mut t = std::fs::read_to_string(&p).unwrap();
        t.push(' ');
        std::fs::write(&p, t).unwrap();
        let e = verify_campaign(&dir).unwrap_err();
        assert!(e.contains("hash mismatch"), "{e}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn plan_validation_and_missing_files_are_reported() {
        assert!(CampaignPlan::from_toml("name = 'x'\ncases = []").is_err());
        let dup = "name='x'\n[[cases]]\nid='a'\nfile='f'\n[[cases]]\nid='a'\nfile='g'\n";
        assert!(CampaignPlan::from_toml(dup).unwrap_err().contains("duplicate"));
        let missing = "name='x'\n[[cases]]\nid='a'\nfile='does-not-exist.toml'\n";
        let dir = std::env::temp_dir().join(format!("kshana-campaign-miss-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let s = run_campaign(missing, Path::new("."), &dir).unwrap();
        assert_eq!(s.not_evaluated, 1);
        assert!(s.results[0].error.is_some());
        verify_campaign(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
