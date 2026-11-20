// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The architecture failure-mode pack (`kshana-pro altpnt failure-modes`).
//!
//! An alternative-PNT architecture is a set of **sources** (GNSS, a LEO service, eLoran, a
//! terrestrial radio, an inertial unit, a clock, ...), each depending on **resources** it may
//! share with others (an antenna, a power bus, a reference clock, a processor). The supplier
//! declares **failure modes**, each against one source or one resource, with its detection,
//! mitigation and the fallback sources expected to carry on, and lists the **tests** run on
//! each mode. The pack reads only these declarations and the signed test evidence they cite;
//! it does not run, switch or combine anything.
//!
//! Findings, each a fixed rule:
//!
//! * a mode that names no detection, or no mitigation;
//! * a fallback that the same failure also takes out (it depends on the failed source or the
//!   failed resource): a common-cause failure; a mode left with no unaffected fallback;
//! * a resource every source depends on (a single point of failure), and a resource shared by
//!   two or more sources with no failure mode declared for it;
//! * a source with no declared failure mode;
//! * a failed test, a test whose cited evidence does not verify, a mode with no test.
//!
//! Every mode gets the evidence label of its counted tests ([`super::evidence_label`]): only
//! twin or simulator tests leave it MODELLED, an architectural target.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{evidence_label, Environment, Inputs, EVIDENCE, SCOPE};
use crate::license::KEY_LEN;

/// The result format.
pub const FORMAT: &str = "kshana-pro-altpnt-failure-modes/1";

/// One source of the architecture.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    /// gnss, leo, eloran, terrestrial-radio, inertial, clock, vision, baro, other.
    pub kind: String,
    /// Resources this source depends on.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// One declared failure mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailureMode {
    pub id: String,
    /// The failing source (or give `resource`).
    #[serde(default)]
    pub source: Option<String>,
    /// The failing resource: every source depending on it is affected.
    #[serde(default)]
    pub resource: Option<String>,
    pub mode: String,
    #[serde(default)]
    pub effect: String,
    #[serde(default)]
    pub detection: String,
    #[serde(default)]
    pub mitigation: String,
    /// Sources expected to carry on.
    #[serde(default)]
    pub fallback: Vec<String>,
}

/// One test run on a failure mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub id: String,
    pub failure_mode: String,
    pub environment: Environment,
    /// pass, fail or inconclusive.
    pub outcome: String,
    /// A signed result directory (verified against the `--trust` keys).
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub note: String,
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub system: String,
    #[serde(rename = "source")]
    pub sources: Vec<Source>,
    #[serde(rename = "failure_mode", default)]
    pub modes: Vec<FailureMode>,
    #[serde(rename = "test", default)]
    pub tests: Vec<Test>,
}

/// A test as counted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestRow {
    pub id: String,
    pub environment: Environment,
    pub outcome: String,
    /// `none`, `verified`, `signature-ok-untrusted` (no `--trust` key given), or `failed: ...`.
    pub evidence_check: String,
    /// The signer key ID of the cited evidence.
    pub evidence_key_id: Option<String>,
    /// Counted towards the label (its evidence, if cited, verified).
    pub counted: bool,
}

/// One mode's row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModeRow {
    pub id: String,
    pub mode: String,
    pub target: String,
    pub affected_sources: Vec<String>,
    pub detection_declared: bool,
    pub mitigation_declared: bool,
    pub fallback: Vec<String>,
    /// Fallbacks the same failure takes out.
    pub fallback_affected: Vec<String>,
    pub independent_fallback: bool,
    pub tests: Vec<TestRow>,
    /// The evidence label of the counted tests.
    pub label: &'static str,
    /// TESTED, TEST-FAILED, MODELLED-ONLY or UNTESTED.
    pub status: &'static str,
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// The rule (stable identifier).
    pub rule: &'static str,
    pub subject: String,
    pub detail: String,
}

/// The pack.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pack {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub system: String,
    pub sources: Vec<Source>,
    /// Resource to the sources depending on it.
    pub resources: BTreeMap<String, Vec<String>>,
    pub single_points_of_failure: Vec<String>,
    pub modes: Vec<ModeRow>,
    pub findings: Vec<Finding>,
    /// Counts: modes, with detection, with an independent fallback, by label, by status.
    pub coverage: BTreeMap<String, usize>,
    pub scope: &'static str,
}

fn check(req: &Request) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for s in &req.sources {
        if !ids.insert(&s.id) {
            return Err(format!("source '{}' listed twice", s.id));
        }
    }
    if req.sources.is_empty() {
        return Err("the architecture needs at least one [[source]]".into());
    }
    let mut mids = BTreeSet::new();
    for m in &req.modes {
        if !mids.insert(&m.id) {
            return Err(format!("failure mode '{}' listed twice", m.id));
        }
        match (&m.source, &m.resource) {
            (Some(s), None) if ids.contains(s) => {}
            (None, Some(_)) => {}
            (Some(s), None) => return Err(format!("{}: unknown source '{s}'", m.id)),
            _ => return Err(format!("{}: give exactly one of source or resource", m.id)),
        }
        if let Some(f) = m.fallback.iter().find(|f| !ids.contains(f)) {
            return Err(format!("{}: unknown fallback source '{f}'", m.id));
        }
    }
    for t in &req.tests {
        if !mids.contains(&t.failure_mode) {
            return Err(format!(
                "test {}: unknown failure mode '{}'",
                t.id, t.failure_mode
            ));
        }
        if !matches!(t.outcome.as_str(), "pass" | "fail" | "inconclusive") {
            return Err(format!(
                "test {}: outcome must be pass, fail or inconclusive",
                t.id
            ));
        }
    }
    Ok(())
}

/// Verify cited evidence: `(check, key id, counted)`.
fn evidence_check(
    base: &Path,
    ev: &Option<String>,
    trust: &[[u8; KEY_LEN]],
) -> (String, Option<String>, bool) {
    let Some(dir) = ev else {
        return ("none".into(), None, true);
    };
    let p = if Path::new(dir).is_absolute() {
        Path::new(dir).to_path_buf()
    } else {
        base.join(dir)
    };
    match crate::signed_dossier::verify(&p, trust) {
        Err(e) => (format!("failed: {e}"), None, false),
        Ok(v) if !v.signature_ok || !v.problems.is_empty() => (
            format!("failed: {}", {
                let mut p = v.problems.clone();
                if !v.signature_ok {
                    p.insert(0, "signature does not hold".into());
                }
                p.join("; ")
            }),
            Some(v.key_id),
            false,
        ),
        Ok(v) => match v.trusted {
            Some(true) => ("verified".into(), Some(v.key_id), true),
            Some(false) => (
                "failed: signer is not one of the --trust keys".into(),
                Some(v.key_id),
                false,
            ),
            None => ("signature-ok-untrusted".into(), Some(v.key_id), true),
        },
    }
}

/// Build the pack. `base` resolves relative evidence paths; `trust` pins evidence signers.
pub fn analyse(req: &Request, base: &Path, trust: &[[u8; KEY_LEN]]) -> Result<Pack, String> {
    check(req)?;
    let mut findings = Vec::new();
    let mut resources: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in &req.sources {
        for r in &s.depends_on {
            resources.entry(r.clone()).or_default().push(s.id.clone());
        }
    }
    let n_src = req.sources.len();
    let spof: Vec<String> = resources
        .iter()
        .filter(|(_, v)| n_src > 1 && v.len() == n_src)
        .map(|(k, _)| k.clone())
        .collect();
    for r in &spof {
        findings.push(Finding {
            rule: "single-point-of-failure",
            subject: r.clone(),
            detail: format!("every one of the {n_src} sources depends on '{r}'"),
        });
    }
    let declared_res: BTreeSet<&String> = req
        .modes
        .iter()
        .filter_map(|m| m.resource.as_ref())
        .collect();
    for (r, v) in &resources {
        if v.len() >= 2 && !declared_res.contains(r) {
            findings.push(Finding {
                rule: "shared-resource-undeclared",
                subject: r.clone(),
                detail: format!(
                    "shared by {} with no failure mode declared for it",
                    v.join(", ")
                ),
            });
        }
    }
    let covered: BTreeSet<&String> = req.modes.iter().filter_map(|m| m.source.as_ref()).collect();
    for s in &req.sources {
        if !covered.contains(&s.id) {
            findings.push(Finding {
                rule: "source-undocumented",
                subject: s.id.clone(),
                detail: "no failure mode is declared against this source".into(),
            });
        }
    }

    let mut rows = Vec::new();
    for m in &req.modes {
        let (target, affected): (String, Vec<String>) = match (&m.source, &m.resource) {
            (Some(s), _) => (format!("source {s}"), vec![s.clone()]),
            (_, Some(r)) => (
                format!("resource {r}"),
                resources.get(r).cloned().unwrap_or_default(),
            ),
            _ => unreachable!("checked"),
        };
        let fb_aff: Vec<String> = m
            .fallback
            .iter()
            .filter(|f| affected.contains(f))
            .cloned()
            .collect();
        let independent = m.fallback.iter().any(|f| !affected.contains(f));
        if m.detection.trim().is_empty() {
            findings.push(Finding {
                rule: "no-detection",
                subject: m.id.clone(),
                detail: "no detection is declared for this mode".into(),
            });
        }
        if m.mitigation.trim().is_empty() {
            findings.push(Finding {
                rule: "no-mitigation",
                subject: m.id.clone(),
                detail: "no mitigation is declared for this mode".into(),
            });
        }
        for f in &fb_aff {
            findings.push(Finding {
                rule: "common-cause-fallback",
                subject: m.id.clone(),
                detail: format!("fallback '{f}' is taken out by the same failure ({target})"),
            });
        }
        if !independent {
            findings.push(Finding {
                rule: "no-independent-fallback",
                subject: m.id.clone(),
                detail: "no declared fallback survives this failure".into(),
            });
        }
        let mut tests = Vec::new();
        for t in req.tests.iter().filter(|t| t.failure_mode == m.id) {
            let (ck, kid, counted) = evidence_check(base, &t.evidence, trust);
            if !counted {
                findings.push(Finding {
                    rule: "evidence-not-verified",
                    subject: t.id.clone(),
                    detail: format!(
                        "cited evidence {}: {ck}",
                        t.evidence.clone().unwrap_or_default()
                    ),
                });
            }
            if t.outcome == "fail" && counted {
                findings.push(Finding {
                    rule: "test-failed",
                    subject: t.id.clone(),
                    detail: format!("{} test of {} failed", t.environment.name(), m.id),
                });
            }
            tests.push(TestRow {
                id: t.id.clone(),
                environment: t.environment,
                outcome: t.outcome.clone(),
                evidence_check: ck,
                evidence_key_id: kid,
                counted,
            });
        }
        let envs: Vec<Environment> = tests
            .iter()
            .filter(|t| t.counted && t.outcome != "inconclusive")
            .map(|t| t.environment)
            .collect();
        let label = evidence_label(&envs);
        let failed = tests.iter().any(|t| t.counted && t.outcome == "fail");
        let status = if failed {
            "TEST-FAILED"
        } else if envs.is_empty() {
            "UNTESTED"
        } else if label == "MODELLED" {
            "MODELLED-ONLY"
        } else {
            "TESTED"
        };
        if envs.is_empty() {
            findings.push(Finding {
                rule: "untested",
                subject: m.id.clone(),
                detail: "no counted test of this mode".into(),
            });
        }
        rows.push(ModeRow {
            id: m.id.clone(),
            mode: m.mode.clone(),
            target,
            affected_sources: affected,
            detection_declared: !m.detection.trim().is_empty(),
            mitigation_declared: !m.mitigation.trim().is_empty(),
            fallback: m.fallback.clone(),
            fallback_affected: fb_aff,
            independent_fallback: independent,
            tests,
            label,
            status,
        });
    }
    let mut coverage = BTreeMap::new();
    coverage.insert("modes".into(), rows.len());
    coverage.insert(
        "with_detection".into(),
        rows.iter().filter(|r| r.detection_declared).count(),
    );
    coverage.insert(
        "with_independent_fallback".into(),
        rows.iter().filter(|r| r.independent_fallback).count(),
    );
    for r in &rows {
        *coverage.entry(format!("label:{}", r.label)).or_insert(0) += 1;
        *coverage.entry(format!("status:{}", r.status)).or_insert(0) += 1;
    }
    Ok(Pack {
        format: FORMAT,
        evidence: EVIDENCE,
        title: req.title.clone(),
        system: req.system.clone(),
        sources: req.sources.clone(),
        resources,
        single_points_of_failure: spof,
        modes: rows,
        findings,
        coverage,
        scope: SCOPE,
    })
}

/// Load a request: the request and its inputs record.
pub fn load(path: &Path) -> Result<(Request, Inputs), String> {
    super::read_request(path)
}

/// The failure-mode matrix as Markdown.
pub fn markdown(p: &Pack) -> String {
    let mut s = format!(
        "# Architecture failure modes: {}\n\nSystem: {}. Evidence class: {}.\n\n",
        p.title, p.system, p.evidence
    );
    s += &format!(
        "{} declared modes; {} with detection; {} with an independent fallback. \
         Single points of failure: {}.\n\n",
        p.coverage.get("modes").copied().unwrap_or(0),
        p.coverage.get("with_detection").copied().unwrap_or(0),
        p.coverage
            .get("with_independent_fallback")
            .copied()
            .unwrap_or(0),
        if p.single_points_of_failure.is_empty() {
            "none found".to_string()
        } else {
            p.single_points_of_failure.join(", ")
        }
    );
    s += "| Mode | Target | Detection | Mitigation | Independent fallback | Tests | Label | Status |\n";
    s += "|---|---|---|---|---|---|---|---|\n";
    let yn = |b: bool| if b { "yes" } else { "no" };
    for r in &p.modes {
        let tests = r
            .tests
            .iter()
            .map(|t| format!("{} {} {}", t.id, t.environment.name(), t.outcome))
            .collect::<Vec<_>>()
            .join("; ");
        s += &format!(
            "| {} {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.id,
            r.mode,
            r.target,
            yn(r.detection_declared),
            yn(r.mitigation_declared),
            yn(r.independent_fallback),
            if tests.is_empty() { "-".into() } else { tests },
            r.label,
            r.status
        );
    }
    s += "\n## Findings\n\n";
    if p.findings.is_empty() {
        s += "None.\n";
    }
    for f in &p.findings {
        s += &format!("- `{}` {}: {}\n", f.rule, f.subject, f.detail);
    }
    s += &format!(
        "\nLabels: MEASURED-OPERATIONAL (field or flight), MEASURED-LAB (HIL or RF lab), \
         MODELLED (twin or simulator only: an architectural target, not an empirical result), \
         CLAIMED (no test).\n\n{}\n",
        p.scope
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resource_mode_takes_out_its_dependants() {
        let req: Request = toml::from_str(
            r#"title = "t"
system = "s"
[[source]]
id = "a"
kind = "gnss"
depends_on = ["ant", "pwr"]
[[source]]
id = "b"
kind = "leo"
depends_on = ["ant", "pwr"]
[[failure_mode]]
id = "M1"
resource = "ant"
mode = "antenna damage"
detection = "x"
mitigation = "y"
fallback = ["b"]
"#,
        )
        .unwrap();
        let p = analyse(&req, Path::new("."), &[]).unwrap();
        assert_eq!(p.single_points_of_failure, vec!["ant", "pwr"]);
        let m = &p.modes[0];
        assert_eq!(m.affected_sources, vec!["a", "b"]);
        assert!(!m.independent_fallback);
        assert_eq!(m.status, "UNTESTED");
        assert!(p
            .findings
            .iter()
            .any(|f| f.rule == "shared-resource-undeclared" && f.subject == "pwr"));
        assert!(!p
            .findings
            .iter()
            .any(|f| f.rule == "shared-resource-undeclared" && f.subject == "ant"));
        assert_eq!(
            p.findings
                .iter()
                .filter(|f| f.rule == "source-undocumented")
                .count(),
            2
        );
    }
}
