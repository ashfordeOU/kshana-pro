// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Per-avionics-type recovery-time scorecard.
//!
//! Operators report that some avionics take a long time to return to a usable GNSS position
//! after a spoofing or jamming episode, and that the time differs by equipment type. This
//! scorecard puts numbers on it from test evidence: it reads the test kit's signed scenario
//! verdicts ([`crate::testkit`], for example the aviation recovery and reset scenarios), takes
//! every `recovery_s` criterion as one run, assigns each run to an avionics type through the
//! fleet file, and reports per type: runs, recovered, not recovered, not assessed, median,
//! 95th percentile and worst recovery time, and the share recovered within the operator's
//! target (a parameter; no target is built in). With aircraft counts per type it also gives
//! the fleet-weighted share.
//!
//! A run whose log ends before a recovery could be seen is **not assessed** and is counted
//! apart, never as recovered.

use serde::{Deserialize, Serialize};

use super::sig9;
use crate::scorecards::levels::Verdict;
use crate::testkit::score::ScenarioVerdict;

/// The format tag of a fleet file.
pub const FLEET_FORMAT: &str = "kshana-avionics-fleet/1";
/// The format tag of the scorecard.
pub const FORMAT: &str = "kshana-avionics-recovery-scorecard/1";

/// One avionics type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvionicsType {
    pub id: String,
    #[serde(default)]
    pub description: String,
    /// The test kit's receiver names that are units of this type.
    pub receivers: Vec<String>,
    /// Aircraft in the fleet carrying this type (for the fleet-weighted share).
    #[serde(default)]
    pub aircraft: Option<u32>,
}

/// A fleet file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fleet {
    pub format: String,
    #[serde(default)]
    pub operator: String,
    #[serde(rename = "type")]
    pub types: Vec<AvionicsType>,
}

impl Fleet {
    pub fn parse(src: &str) -> Result<Fleet, String> {
        let f: Fleet = toml::from_str(src).map_err(|e| format!("fleet: {e}"))?;
        if f.format != FLEET_FORMAT {
            return Err(format!(
                "fleet: format '{}' is not {FLEET_FORMAT}",
                f.format
            ));
        }
        if f.types.is_empty() {
            return Err("fleet: no [[type]]".into());
        }
        let mut seen = std::collections::BTreeMap::new();
        for t in &f.types {
            for r in &t.receivers {
                if let Some(other) = seen.insert(r.clone(), t.id.clone()) {
                    return Err(format!(
                        "fleet: receiver '{r}' is in both '{other}' and '{}'",
                        t.id
                    ));
                }
            }
        }
        Ok(f)
    }
}

/// One recovery run taken from a verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub source: String,
    pub scenario: String,
    pub scenario_sha256: String,
    pub build: String,
    pub receiver: String,
    pub criterion: String,
    /// `recovered`, `not-recovered` or `not-assessed`.
    pub outcome: String,
    pub recovery_s: Option<f64>,
}

/// One avionics type's row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeRow {
    pub id: String,
    pub description: String,
    pub aircraft: Option<u32>,
    pub runs: usize,
    pub recovered: usize,
    pub not_recovered: usize,
    pub not_assessed: usize,
    pub median_s: Option<f64>,
    pub p95_s: Option<f64>,
    pub worst_s: Option<f64>,
    /// Share of assessed runs recovered within the target.
    pub within_target: Option<f64>,
    pub target_verdict: Verdict,
    pub scenarios: Vec<String>,
}

/// The scorecard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub format: String,
    pub label: String,
    pub operator: String,
    pub target_s: Option<f64>,
    pub target_source: String,
    /// Types ordered worst first (most not recovered, then the longest 95th percentile).
    pub types: Vec<TypeRow>,
    pub fleet_weighted_within_target: Option<f64>,
    /// Receivers in the verdicts that no type lists.
    pub unassigned_receivers: Vec<String>,
    pub runs: Vec<Run>,
}

fn nearest_rank(sorted: &[f64], q: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let k = ((q * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    Some(sorted[k - 1])
}

/// Every recovery run in `verdicts` ((source, verdict) pairs).
pub fn runs(verdicts: &[(String, ScenarioVerdict)]) -> Vec<Run> {
    let mut out = Vec::new();
    for (src, v) in verdicts {
        for c in v.criteria.iter().filter(|c| c.metric == "recovery_s") {
            let outcome = match (c.measured, c.verdict) {
                (Some(_), _) => "recovered",
                (None, Verdict::NotMet) => "not-recovered",
                (None, _) => "not-assessed",
            };
            out.push(Run {
                source: src.clone(),
                scenario: v.scenario.clone(),
                scenario_sha256: v.scenario_sha256.clone(),
                build: v.build.clone(),
                receiver: v.receiver.clone(),
                criterion: c.id.clone(),
                outcome: outcome.into(),
                recovery_s: c.measured.map(sig9),
            });
        }
    }
    out
}

/// Build the scorecard.
pub fn scorecard(
    fleet: &Fleet,
    verdicts: &[(String, ScenarioVerdict)],
    target_s: Option<f64>,
    target_source: &str,
) -> Result<Scorecard, String> {
    if let Some(t) = target_s {
        if !t.is_finite() || t <= 0.0 {
            return Err("the recovery target must be a positive number of seconds".into());
        }
    }
    let all = runs(verdicts);
    if all.is_empty() {
        return Err("no recovery_s criterion in the verdicts given".into());
    }
    let mut rows = Vec::new();
    for t in &fleet.types {
        let mine: Vec<&Run> = all
            .iter()
            .filter(|r| t.receivers.contains(&r.receiver))
            .collect();
        let mut rec: Vec<f64> = mine.iter().filter_map(|r| r.recovery_s).collect();
        rec.sort_by(f64::total_cmp);
        let not_rec = mine.iter().filter(|r| r.outcome == "not-recovered").count();
        let not_ass = mine.iter().filter(|r| r.outcome == "not-assessed").count();
        let assessed = rec.len() + not_rec;
        let within = match target_s {
            Some(tg) if assessed > 0 => Some(sig9(
                rec.iter().filter(|x| **x <= tg).count() as f64 / assessed as f64,
            )),
            _ => None,
        };
        let mut scen: Vec<String> = mine.iter().map(|r| r.scenario.clone()).collect();
        scen.sort();
        scen.dedup();
        rows.push(TypeRow {
            id: t.id.clone(),
            description: t.description.clone(),
            aircraft: t.aircraft,
            runs: mine.len(),
            recovered: rec.len(),
            not_recovered: not_rec,
            not_assessed: not_ass,
            median_s: nearest_rank(&rec, 0.5),
            p95_s: nearest_rank(&rec, 0.95),
            worst_s: rec.last().copied(),
            within_target: within,
            target_verdict: match (within, target_s) {
                (Some(w), Some(_)) => Verdict::from_bool(Some(w >= 1.0)),
                _ => Verdict::NotAssessed,
            },
            scenarios: scen,
        });
    }
    rows.sort_by(|a, b| {
        let share = |r: &TypeRow| {
            let n = r.recovered + r.not_recovered;
            if n == 0 {
                0.0
            } else {
                r.not_recovered as f64 / n as f64
            }
        };
        share(b)
            .partial_cmp(&share(a))
            .unwrap()
            .then(
                b.p95_s
                    .unwrap_or(-1.0)
                    .partial_cmp(&a.p95_s.unwrap_or(-1.0))
                    .unwrap(),
            )
            .then(a.id.cmp(&b.id))
    });
    let weighted: Vec<(f64, f64)> = rows
        .iter()
        .filter_map(|r| Some((r.aircraft? as f64, r.within_target?)))
        .collect();
    let total: f64 = weighted.iter().map(|w| w.0).sum();
    let mut unassigned: Vec<String> = all
        .iter()
        .filter(|r| {
            !fleet
                .types
                .iter()
                .any(|t| t.receivers.contains(&r.receiver))
        })
        .map(|r| r.receiver.clone())
        .collect();
    unassigned.sort();
    unassigned.dedup();
    Ok(Scorecard {
        format: FORMAT.into(),
        label: super::LABEL.into(),
        operator: fleet.operator.clone(),
        target_s,
        target_source: if target_s.is_some() {
            target_source.into()
        } else {
            "no target supplied: shares within target not assessed".into()
        },
        types: rows,
        fleet_weighted_within_target: if total > 0.0 {
            Some(sig9(
                weighted.iter().map(|w| w.0 * w.1).sum::<f64>() / total,
            ))
        } else {
            None
        },
        unassigned_receivers: unassigned,
        runs: all,
    })
}

fn opt(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.0}")).unwrap_or_else(|| "-".into())
}

/// The scorecard as Markdown.
pub fn markdown(s: &Scorecard) -> String {
    let mut m = String::from("# Recovery-time scorecard by avionics type\n\n");
    m.push_str(&format!("{}\n\n", s.label));
    if !s.operator.is_empty() {
        m.push_str(&format!("Operator: {}\n\n", s.operator));
    }
    m.push_str(&format!(
        "Target: {} ({})\n\n",
        s.target_s
            .map(|t| format!("{t} s"))
            .unwrap_or_else(|| "none".into()),
        s.target_source
    ));
    m.push_str("| Type | Aircraft | Runs | Recovered | Not recovered | Not assessed | Median (s) | P95 (s) | Worst (s) | Within target |\n|---|---|---|---|---|---|---|---|---|---|\n");
    for r in &s.types {
        m.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.id,
            r.aircraft
                .map(|a| a.to_string())
                .unwrap_or_else(|| "-".into()),
            r.runs,
            r.recovered,
            r.not_recovered,
            r.not_assessed,
            opt(r.median_s),
            opt(r.p95_s),
            opt(r.worst_s),
            r.within_target
                .map(|w| format!("{:.0}%", w * 100.0))
                .unwrap_or_else(|| "not assessed".into()),
        ));
    }
    if let Some(w) = s.fleet_weighted_within_target {
        m.push_str(&format!(
            "\nFleet-weighted share of runs recovered within the target: {:.0}%\n",
            w * 100.0
        ));
    }
    if !s.unassigned_receivers.is_empty() {
        m.push_str(&format!(
            "\nReceivers in the verdicts that no type lists (left out): {}\n",
            s.unassigned_receivers.join(", ")
        ));
    }
    m
}
