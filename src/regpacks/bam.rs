// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Bridge alert mapping check of PNT-integrity alerts (E29 B2901; research 09b C69)
//!
//! A resilient PNT box or PNT data processor raises integrity conditions (integrity level
//! lowered, a source excluded, sources in disagreement). On the bridge each must arrive as an
//! alert with an identifier, a priority and a category under bridge alert management (IMO
//! MSC.302(87) and IEC 62923-1/-2; clause numbers to be confirmed, the standards were not read
//! in the research). This module checks a maker's **alert map** (condition to alert) against
//! rules written into the output, then replays the map over the output of the PNT data
//! processing reference ([`crate::pntdp`], `kshana-pro pntdp run --json`) to show which
//! alerts would be raised and cleared, and which conditions occurred with no alert.
//!
//! The rules are Kshana's pre-assessment rules, configurable in the map's `[policy]`; they are
//! not the text of the standards. The output is evidence against the alert-mapping rows of
//! the `bam-pnt-alerts` catalogue; whether an alert interface meets the standards is decided
//! by the type examination, not here.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{esc, page_head, sha256_hex, sig9, write_json};

/// The check's format tag.
pub const FORMAT: &str = "kshana-pro-bam-alert-check/1";
/// The file the check writes.
pub const RESULT_FILE: &str = "bam-check.json";
/// The method id.
pub const METHOD: &str = "kshana-pro/bam-pnt-alert-map/v1";

/// Priorities in increasing order.
pub const PRIORITIES: [&str; 4] = ["caution", "warning", "alarm", "emergency-alarm"];
/// Categories.
pub const CATEGORIES: [&str; 3] = ["A", "B", "C"];
/// Source flags of the PNT data processing reference.
pub const FLAGS: [&str; 7] = [
    "incomplete",
    "invalid",
    "stale",
    "frozen",
    "jump",
    "inconsistent",
    "disagreement",
];

fn default_required() -> Vec<String> {
    let mut v: Vec<String> = vec![
        "integrity:none".into(),
        "integrity:low".into(),
        "status:raised".into(),
        "no-solution".into(),
    ];
    v.extend(FLAGS.iter().map(|f| format!("flag:{f}")));
    v
}

fn default_allowed() -> Vec<String> {
    vec!["caution".into(), "warning".into(), "alarm".into()]
}

fn default_min_none() -> String {
    "warning".into()
}

/// The pre-assessment rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Conditions that must be mapped.
    #[serde(default = "default_required")]
    pub required: Vec<String>,
    /// Priorities a PNT-integrity alert may take.
    #[serde(default = "default_allowed")]
    pub allowed_priorities: Vec<String>,
    /// The lowest priority for the loss of integrity (`integrity:none`).
    #[serde(default = "default_min_none")]
    pub min_priority_integrity_none: String,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            required: default_required(),
            allowed_priorities: default_allowed(),
            min_priority_integrity_none: default_min_none(),
        }
    }
}

/// One mapping from a condition to an alert.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlertDef {
    pub condition: String,
    pub alert_id: String,
    #[serde(default)]
    pub instance: Option<u32>,
    pub priority: String,
    pub category: String,
    #[serde(default)]
    pub text: String,
}

/// A maker's alert map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlertMap {
    #[serde(default)]
    pub equipment: String,
    #[serde(default)]
    pub policy: Policy,
    #[serde(rename = "alert", default)]
    pub alerts: Vec<AlertDef>,
}

impl AlertMap {
    pub fn parse(src: &str) -> Result<AlertMap, String> {
        toml::from_str(src).map_err(|e| format!("alert map: {e}"))
    }
}

/// Whether `c` is a condition this check knows.
pub fn known_condition(c: &str) -> bool {
    match c.split_once(':') {
        Some(("integrity", l)) => ["none", "low", "medium"].contains(&l),
        Some(("status", "raised")) => true,
        Some(("flag", f)) => FLAGS.contains(&f),
        None => c == "no-solution",
        _ => false,
    }
}

fn rank(p: &str) -> Option<usize> {
    PRIORITIES.iter().position(|x| *x == p)
}

/// One finding of the map check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub rule: &'static str,
    /// `gap` or `note`.
    pub severity: &'static str,
    pub text: String,
}

/// The map checks; the rules are stated in each finding.
pub fn check_map(m: &AlertMap) -> Vec<Finding> {
    let mut f = Vec::new();
    let gap = |rule, text: String| Finding {
        rule,
        severity: "gap",
        text,
    };
    for p in &m.policy.allowed_priorities {
        if rank(p).is_none() {
            f.push(gap("POLICY", format!("unknown priority '{p}' in policy")));
        }
    }
    let mapped: BTreeSet<&str> = m.alerts.iter().map(|a| a.condition.as_str()).collect();
    for r in &m.policy.required {
        if !mapped.contains(r.as_str()) {
            f.push(gap(
                "MAP-1",
                format!("required condition '{r}' has no alert (every required condition must be mapped)"),
            ));
        }
    }
    let mut by_id: BTreeMap<(String, Option<u32>), Vec<&AlertDef>> = BTreeMap::new();
    let mut seen_cond = BTreeSet::new();
    for a in &m.alerts {
        if !known_condition(&a.condition) {
            f.push(gap("MAP-0", format!("unknown condition '{}'", a.condition)));
        }
        if !seen_cond.insert(a.condition.as_str()) {
            f.push(gap(
                "MAP-0",
                format!("condition '{}' mapped more than once", a.condition),
            ));
        }
        if !m.policy.allowed_priorities.contains(&a.priority) {
            f.push(gap(
                "MAP-2",
                format!(
                    "'{}': priority '{}' is not one of the allowed priorities ({})",
                    a.condition,
                    a.priority,
                    m.policy.allowed_priorities.join(", ")
                ),
            ));
        }
        if !CATEGORIES.contains(&a.category.as_str()) {
            f.push(gap(
                "MAP-2",
                format!(
                    "'{}': category '{}' is not A, B or C",
                    a.condition, a.category
                ),
            ));
        }
        if a.alert_id.trim().is_empty() {
            f.push(gap(
                "MAP-3",
                format!("'{}': no alert identifier", a.condition),
            ));
        } else {
            by_id
                .entry((a.alert_id.trim().to_string(), a.instance))
                .or_default()
                .push(a);
        }
        if a.text.trim().is_empty() {
            f.push(Finding {
                rule: "MAP-5",
                severity: "note",
                text: format!("'{}': no alert text", a.condition),
            });
        }
    }
    for ((id, inst), v) in &by_id {
        let pr: BTreeSet<&str> = v.iter().map(|a| a.priority.as_str()).collect();
        if pr.len() > 1 {
            f.push(gap(
                "MAP-3",
                format!(
                    "alert {id}{} is used for conditions with different priorities ({})",
                    inst.map(|i| format!(" instance {i}")).unwrap_or_default(),
                    pr.into_iter().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
    if let Some(a) = m.alerts.iter().find(|a| a.condition == "integrity:none") {
        if let (Some(have), Some(min)) = (
            rank(&a.priority),
            rank(&m.policy.min_priority_integrity_none),
        ) {
            if have < min {
                f.push(gap(
                    "MAP-4",
                    format!(
                        "loss of integrity is mapped to '{}', below the policy's minimum '{}'",
                        a.priority, m.policy.min_priority_integrity_none
                    ),
                ));
            }
        }
    }
    f
}

/// One epoch of `pntdp run --json`, as read.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Epoch {
    pub t_s: f64,
    #[serde(default)]
    pub selected: Option<String>,
    pub integrity: String,
    #[serde(default)]
    pub flags: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub status: String,
}

/// Read the epochs of a `pntdp run --json` document.
pub fn parse_epochs(text: &str) -> Result<Vec<Epoch>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("pntdp: {e}"))?;
    let e = v
        .get("epochs")
        .cloned()
        .ok_or("pntdp: no 'epochs' (write it with `kshana-pro pntdp run --json`)")?;
    serde_json::from_value(e).map_err(|e| format!("pntdp epochs: {e}"))
}

/// Epochs from the processor's own output.
pub fn epochs_from(out: &[crate::pntdp::Output]) -> Vec<Epoch> {
    out.iter()
        .map(|o| Epoch {
            t_s: o.t_s,
            selected: o.selected.clone(),
            integrity: o.integrity.as_str().to_string(),
            flags: o.flags.clone(),
            status: o.status.to_string(),
        })
        .collect()
}

/// The conditions active at one epoch.
pub fn active(e: &Epoch) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    if e.integrity != "high" {
        s.insert(format!("integrity:{}", e.integrity));
    }
    if e.status == "raised" {
        s.insert("status:raised".into());
    }
    if e.selected.is_none() {
        s.insert("no-solution".into());
    }
    for v in e.flags.values() {
        for f in v {
            s.insert(format!("flag:{f}"));
        }
    }
    s
}

/// One alert event on the replayed timeline.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Event {
    pub t_s: f64,
    pub condition: String,
    /// `raised` or `cleared`.
    pub event: &'static str,
    /// The mapped alert as `id[/instance] priority category`, or `UNMAPPED`.
    pub alert: String,
}

/// Per condition over the replay.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConditionSummary {
    pub condition: String,
    pub epochs_active: usize,
    pub onsets: usize,
    pub alert: String,
}

/// The check's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BamCheck {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub equipment: String,
    pub map_sha256: String,
    pub pntdp_sha256: String,
    pub policy: Policy,
    pub alerts: Vec<AlertDef>,
    pub findings: Vec<Finding>,
    pub epochs: usize,
    pub conditions: Vec<ConditionSummary>,
    /// Onsets of conditions that have no alert.
    pub unmapped_onsets: usize,
    pub timeline: Vec<Event>,
    /// Map gaps (findings of severity `gap`).
    pub gaps: usize,
}

fn alert_text(m: &AlertMap, c: &str) -> String {
    m.alerts
        .iter()
        .find(|a| a.condition == c)
        .map(|a| {
            format!(
                "{}{} {} {}",
                a.alert_id,
                a.instance.map(|i| format!("/{i}")).unwrap_or_default(),
                a.priority,
                a.category
            )
        })
        .unwrap_or_else(|| "UNMAPPED".into())
}

/// Check a map and replay it over epochs.
pub fn run(m: &AlertMap, map_text: &str, epochs: &[Epoch], pntdp_text: &str) -> BamCheck {
    let findings = check_map(m);
    let mut timeline = Vec::new();
    let mut prev: BTreeSet<String> = BTreeSet::new();
    let mut stats: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for e in epochs {
        let now = active(e);
        for c in now.difference(&prev) {
            stats.entry(c.clone()).or_default().1 += 1;
            timeline.push(Event {
                t_s: sig9(e.t_s),
                condition: c.clone(),
                event: "raised",
                alert: alert_text(m, c),
            });
        }
        for c in prev.difference(&now) {
            timeline.push(Event {
                t_s: sig9(e.t_s),
                condition: c.clone(),
                event: "cleared",
                alert: alert_text(m, c),
            });
        }
        for c in &now {
            stats.entry(c.clone()).or_default().0 += 1;
        }
        prev = now;
    }
    let conditions: Vec<ConditionSummary> = stats
        .into_iter()
        .map(|(c, (n, on))| ConditionSummary {
            alert: alert_text(m, &c),
            condition: c,
            epochs_active: n,
            onsets: on,
        })
        .collect();
    let unmapped_onsets = conditions
        .iter()
        .filter(|c| c.alert == "UNMAPPED")
        .map(|c| c.onsets)
        .sum();
    BamCheck {
        format: FORMAT,
        method: METHOD,
        label: super::LABEL,
        equipment: m.equipment.clone(),
        map_sha256: sha256_hex(map_text.as_bytes()),
        pntdp_sha256: sha256_hex(pntdp_text.as_bytes()),
        policy: m.policy.clone(),
        alerts: m.alerts.clone(),
        gaps: findings.iter().filter(|f| f.severity == "gap").count(),
        findings,
        epochs: epochs.len(),
        conditions,
        unmapped_onsets,
        timeline,
    }
}

/// The check's page.
pub fn render_html(b: &BamCheck) -> String {
    let mut h = page_head("BAM alert mapping check");
    h.push_str(&format!(
        "<h1>BAM alert mapping check: {}</h1><p class=\"l\">{}</p>\
         <p>{} map gaps; {} epochs replayed; {} onsets of conditions with no alert.</p>\
         <h2>Map findings</h2><table><tr><th>Rule</th><th>Severity</th><th>Finding</th></tr>",
        esc(&b.equipment),
        esc(b.label),
        b.gaps,
        b.epochs,
        b.unmapped_onsets
    ));
    for f in &b.findings {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            f.rule,
            f.severity,
            esc(&f.text)
        ));
    }
    h.push_str("</table><h2>Conditions seen in the replay</h2><table><tr><th>Condition</th><th>Epochs active</th><th>Onsets</th><th>Alert</th></tr>");
    for c in &b.conditions {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&c.condition),
            c.epochs_active,
            c.onsets,
            esc(&c.alert)
        ));
    }
    h.push_str("</table></body></html>\n");
    h
}

/// Write the check's directory.
pub fn write_dir(out: &std::path::Path, b: &BamCheck) -> Result<(), String> {
    write_json(out, RESULT_FILE, b)?;
    std::fs::write(out.join("report.html"), render_html(b)).map_err(|e| e.to_string())
}
