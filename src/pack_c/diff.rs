// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **What moved between two runs** — compare two `ci.json` reports figure by figure.
//!
//! The use case is reviewing an engine upgrade or a scenario revision: run the pack on
//! both, then `kshana-pro ci diff old.json new.json`. Every figure whose value differs
//! (bit for bit), or that exists in only one report, is listed with its old and new
//! value, the change, and whether the SHA-256 of the member result it came from changed.
//! Figures that did not move are counted, not listed. Watched figures are listed the same
//! way from the watch outcomes, since a watch can read an aggregate that is not a metric.

use crate::pack_c::run::CiReport;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// One figure that moved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Moved {
    /// The metric key (`label::path`) or watch identity.
    pub key: String,
    /// The old value (absent when the figure is new).
    pub old: Option<f64>,
    /// The new value (absent when the figure disappeared).
    pub new: Option<f64>,
    /// `new - old`.
    pub change: Option<f64>,
    /// `(new - old) / |old|`, when old is not zero.
    pub rel_change: Option<f64>,
    /// The unit, when either report states it.
    pub unit: Option<String>,
    /// Whether the member result this figure came from changed its SHA-256; `None` when
    /// either report carries no member hashes.
    pub result_sha256_changed: Option<bool>,
}

/// One member result compared by hash.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemberDiff {
    /// Entry label.
    pub label: String,
    /// Old result SHA-256.
    pub old: Option<String>,
    /// New result SHA-256.
    pub new: Option<String>,
    /// Whether it changed.
    pub changed: bool,
}

/// Identity of one side of the comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Side {
    /// Pack id and version.
    pub pack: String,
    /// Pack hash.
    pub pack_hash: String,
    /// Engine version.
    pub engine_version: Option<String>,
    /// Report time stamp.
    pub generated_utc: Option<String>,
}

/// The comparison of two reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiffReport {
    /// The old report.
    pub old: Side,
    /// The new report.
    pub new: Side,
    /// Metrics that moved, in key order.
    pub moved: Vec<Moved>,
    /// Number of metrics present in both with identical values.
    pub unchanged: usize,
    /// Watched figures that moved, in watch order of the new report.
    pub watched_moved: Vec<Moved>,
    /// Member results compared by hash (empty when either report has none).
    pub members: Vec<MemberDiff>,
}

fn side(r: &CiReport) -> Side {
    Side {
        pack: format!("{} {}", r.pack_id, r.pack_version),
        pack_hash: r.pack_hash.clone(),
        engine_version: r.engine_version.clone(),
        generated_utc: r.generated_utc.clone(),
    }
}

fn member_hashes(r: &CiReport) -> Option<BTreeMap<String, String>> {
    r.watch.as_ref().map(|w| {
        w.members
            .iter()
            .map(|m| (m.label.clone(), m.result_sha256.clone()))
            .collect()
    })
}

fn moved(
    key: &str,
    old: Option<f64>,
    new: Option<f64>,
    unit: Option<String>,
    sha: Option<bool>,
) -> Moved {
    let change = match (old, new) {
        (Some(o), Some(n)) => Some(n - o),
        _ => None,
    };
    let rel_change = match (old, change) {
        (Some(o), Some(c)) if o != 0.0 => Some(c / o.abs()),
        _ => None,
    };
    Moved {
        key: key.to_string(),
        old,
        new,
        change,
        rel_change,
        unit,
        result_sha256_changed: sha,
    }
}

/// Compare `old` with `new`.
pub fn diff(old: &CiReport, new: &CiReport) -> DiffReport {
    let (oh, nh) = (member_hashes(old), member_hashes(new));
    let sha_changed = |label: &str| -> Option<bool> {
        let (o, n) = (oh.as_ref()?, nh.as_ref()?);
        Some(o.get(label) != n.get(label))
    };
    let unit_of = |key: &str| -> Option<String> {
        new.watch
            .as_ref()
            .and_then(|w| w.units.get(key))
            .or_else(|| old.watch.as_ref().and_then(|w| w.units.get(key)))
            .cloned()
    };
    let keys: BTreeSet<&String> = old.metrics.keys().chain(new.metrics.keys()).collect();
    let mut out_moved = Vec::new();
    let mut unchanged = 0;
    for k in keys {
        let (o, n) = (old.metrics.get(k), new.metrics.get(k));
        if o.is_some() && o == n {
            unchanged += 1;
            continue;
        }
        let label = k.split("::").next().unwrap_or("");
        out_moved.push(moved(k, o, n, unit_of(k), sha_changed(label)));
    }
    let mut watched_moved = Vec::new();
    if let Some(nw) = &new.watch {
        for no in &nw.outcomes {
            let oo = old
                .watch
                .as_ref()
                .and_then(|w| w.outcomes.iter().find(|o| o.id == no.id));
            let ov = oo.and_then(|o| o.value);
            if ov.is_some() && ov == no.value {
                continue;
            }
            watched_moved.push(moved(
                &no.id,
                ov,
                no.value,
                no.unit.clone(),
                sha_changed(&no.entry),
            ));
        }
    }
    let mut members = Vec::new();
    if let (Some(o), Some(n)) = (&oh, &nh) {
        let labels: BTreeSet<&String> = o.keys().chain(n.keys()).collect();
        for l in labels {
            members.push(MemberDiff {
                label: l.clone(),
                old: o.get(l).cloned(),
                new: n.get(l).cloned(),
                changed: o.get(l) != n.get(l),
            });
        }
    }
    DiffReport {
        old: side(old),
        new: side(new),
        moved: out_moved,
        unchanged,
        watched_moved,
        members,
    }
}

fn num(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{x}"),
        None => "absent".into(),
    }
}

/// A plain-text rendering, one line per moved figure.
pub fn to_text(d: &DiffReport) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "ci diff: {} (engine {}, {}) -> {} (engine {}, {})\n",
        d.old.pack,
        d.old.engine_version.as_deref().unwrap_or("n/a"),
        d.old.generated_utc.as_deref().unwrap_or("no time stamp"),
        d.new.pack,
        d.new.engine_version.as_deref().unwrap_or("n/a"),
        d.new.generated_utc.as_deref().unwrap_or("no time stamp"),
    ));
    if d.old.pack_hash != d.new.pack_hash {
        s.push_str("pack hash changed: the manifest itself differs\n");
    }
    for m in &d.members {
        s.push_str(&format!(
            "member {}: result SHA-256 {}\n",
            m.label,
            if m.changed { "CHANGED" } else { "unchanged" }
        ));
    }
    s.push_str(&format!(
        "{} figure(s) moved, {} unchanged\n",
        d.moved.len(),
        d.unchanged
    ));
    let line = |m: &Moved| {
        let change = match (m.change, m.rel_change) {
            (Some(c), Some(r)) => format!("  (change {c:+}, {:+.3} %)", r * 100.0),
            (Some(c), None) => format!("  (change {c:+})"),
            _ => String::new(),
        };
        let sha = match m.result_sha256_changed {
            Some(true) => "  [result SHA-256 changed]",
            Some(false) => "  [result SHA-256 unchanged]",
            None => "",
        };
        format!(
            "  {} : {} -> {}{}{change}{sha}\n",
            m.key,
            num(m.old),
            num(m.new),
            m.unit.as_ref().map(|u| format!(" {u}")).unwrap_or_default(),
        )
    };
    for m in &d.moved {
        s.push_str(&line(m));
    }
    if !d.watched_moved.is_empty() {
        s.push_str(&format!(
            "{} watched figure(s) moved\n",
            d.watched_moved.len()
        ));
        for m in &d.watched_moved {
            s.push_str(&line(m));
        }
    }
    s
}

/// Deterministic pretty JSON.
pub fn to_json(d: &DiffReport) -> String {
    serde_json::to_string_pretty(d).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack_c::metrics::MetricSet;

    fn report(pairs: &[(&str, f64)]) -> CiReport {
        CiReport {
            pack_id: "p".into(),
            pack_version: "1".into(),
            pack_hash: "h".into(),
            engine_version: None,
            scenario_hashes: vec![],
            generated_utc: None,
            customer: None,
            metrics: MetricSet(pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()),
            verdict: None,
            watch: None,
        }
    }

    #[test]
    fn lists_only_what_moved() {
        let a = report(&[("e::x", 1.0), ("e::y", 2.0), ("e::gone", 3.0)]);
        let b = report(&[("e::x", 1.0), ("e::y", 2.5), ("e::new", 4.0)]);
        let d = diff(&a, &b);
        let keys: Vec<&str> = d.moved.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, vec!["e::gone", "e::new", "e::y"]);
        assert_eq!(d.unchanged, 1);
        let y = &d.moved[2];
        assert_eq!(y.change, Some(0.5));
        assert_eq!(y.rel_change, Some(0.25));
        assert_eq!(y.result_sha256_changed, None);
        assert!(to_text(&d).contains("3 figure(s) moved, 1 unchanged"));
    }
}
