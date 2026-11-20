// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # PNT dependency graph and audit, with sector templates (research 2026-11-20 gaps, E08)
//!
//! An operator lists what it runs (an **inventory**: a ship, a port, an aircraft type, a
//! water utility, a pipeline, an emergency-call chain, a TETRA network, a rescue service)
//! as nodes: PNT **sources** (GNSS receivers, gyro, log, eLoran, PTP grandmasters, atomic
//! clocks, network time, manual input) and **systems** that need position, navigation or
//! time from them, each need listing its providers in priority order, its fallbacks and
//! the independent sources it is cross-checked against. Every GNSS source depends on one
//! shared node, the GNSS signal in space ([`GNSS_SIS`]), so a jammer or spoofer that takes
//! one receiver takes them all: the common mode that inventories most often miss.
//!
//! The audit ([`audit`]) evaluates the graph under:
//!
//! * **GNSS denied** (the signal in space down): which critical functions stop, which carry
//!   on degraded on a fallback, which do not depend on GNSS at all, and how long each lasts
//!   on the holdover of GNSS-disciplined sources;
//! * **GNSS spoofed** (sources up but wrong): which critical functions would run on
//!   GNSS-derived data with no independent cross-check (silently wrong);
//! * **single node loss**, for every node: the single points of failure of a critical
//!   function;
//!
//! and checks the inventory against its **sector template** ([`templates`]): the functions
//! a sector usually has that the inventory does not list, and systems fed by GNSS that
//! cannot be isolated from it. The result is a list of findings, a graph export (GraphML
//! and JSON), and a signed audit directory.
//!
//! The templates are structured after the asset-inventory and dependency-mapping ideas of
//! the documents they name (for ships IACS UR E26 and UK MCA MGN 719); a finding is measured
//! against them, never a statement that an operator meets them. MODELLED: the evaluation is
//! exact over the graph as listed; whether the listing matches the real installation is the
//! operator's statement.

pub mod register;
pub mod templates;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The licence feature of `kshana-pro dependency audit` and `questionnaire`.
pub const FEATURE_AUDIT: &str = "pro-dependency-audit";
/// The licence feature of `kshana-pro dependency register`.
pub const FEATURE_REGISTER: &str = "pro-dependency-register";
/// The inventory's format tag.
pub const INVENTORY_FORMAT: &str = "kshana-pro-pnt-inventory/1";
/// The audit's format tag.
pub const AUDIT_FORMAT: &str = "kshana-pro-pnt-dependency-audit/1";
/// The method tag.
pub const METHOD: &str = "kshana-pro/pnt-dependency/v1";
/// The audit's main file.
pub const RESULT_FILE: &str = "audit.json";
/// The implicit node every GNSS source depends on.
pub const GNSS_SIS: &str = "gnss-signal-in-space";

/// What every audit says about itself.
pub const STATEMENT: &str = "A dependency audit of the inventory as the operator listed it: \
exact over the listed graph, MODELLED as a description of the real installation. Findings are \
measured against the sector template named; they are not a statement that the operator meets \
any rule, and not an instruction to change any system.";

/// The kinds of PNT a node needs or provides.
pub const PNT_KINDS: [&str; 4] = ["position", "velocity", "heading", "time"];

/// The node types that are sources. Every other type is a system.
pub const SOURCE_TYPES: [&str; 15] = [
    "gnss",
    "gnss-compass",
    "gyro",
    "log",
    "radar",
    "eloran",
    "inertial",
    "ptp-grandmaster",
    "atomic-clock",
    "network-time",
    "mobile-network",
    "terrestrial-radio",
    "manual",
    "antenna",
    "power",
];

/// Source types that take their PNT from GNSS.
pub fn gnss_type(t: &str) -> bool {
    matches!(t, "gnss" | "gnss-compass")
}

/// One need of a system.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Need {
    /// `position`, `velocity`, `heading` or `time`.
    pub pnt: String,
    /// Providers in priority order; the first one up is used.
    pub from: Vec<String>,
    /// Used when no provider is up (the function then runs degraded).
    #[serde(default)]
    pub fallback: Vec<String>,
    /// Independent sources the function's output is compared with.
    #[serde(default)]
    pub cross_check: Vec<String>,
}

/// One node of the inventory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// A source type ([`SOURCE_TYPES`]) or `system`.
    #[serde(rename = "type")]
    pub kind: String,
    /// PNT kinds this node provides onward.
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default)]
    pub needs: Vec<Need>,
    /// Other nodes this one cannot run without (an antenna, a power supply, a network).
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// A function whose loss matters to safety or service (audited for single points).
    #[serde(default)]
    pub critical: bool,
    /// For a GNSS-disciplined source: how long it keeps within its limit without GNSS, s.
    #[serde(default)]
    pub holdover_s: f64,
    /// Whether the node's GNSS input can be switched off (isolated) on its own.
    #[serde(default)]
    pub isolation_switch: Option<bool>,
    /// The template item this node answers, when its id differs.
    #[serde(default)]
    pub template_item: Option<String>,
}

impl Node {
    pub fn is_source(&self) -> bool {
        SOURCE_TYPES.contains(&self.kind.as_str())
    }
}

/// An inventory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub format: String,
    /// The asset audited (a vessel, a terminal, a type, a network).
    pub asset: String,
    pub operator: String,
    /// The sector template id ([`templates::ids`]).
    pub sector: String,
    /// How long the asset must keep its critical functions without GNSS, s (optional).
    #[serde(default)]
    pub required_autonomy_s: Option<f64>,
    #[serde(rename = "node")]
    pub nodes: Vec<Node>,
}

impl Inventory {
    /// Parse an inventory from TOML or JSON text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let inv: Inventory = if text.trim_start().starts_with('{') {
            serde_json::from_str(text).map_err(|e| format!("invalid inventory: {e}"))?
        } else {
            toml::from_str(text).map_err(|e| format!("invalid inventory: {e}"))?
        };
        inv.validate()?;
        Ok(inv)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != INVENTORY_FORMAT {
            return Err(format!(
                "inventory format must be '{INVENTORY_FORMAT}', found '{}'",
                self.format
            ));
        }
        if self.nodes.is_empty() {
            return Err("the inventory lists no node".into());
        }
        let mut ids = BTreeSet::new();
        for n in &self.nodes {
            if n.id.is_empty() || n.id == GNSS_SIS || !ids.insert(n.id.as_str()) {
                return Err(format!("node id '{}' is empty, reserved or repeated", n.id));
            }
        }
        for n in &self.nodes {
            if !n.is_source() && n.kind != "system" {
                return Err(format!(
                    "node '{}': unknown type '{}' (a source type or 'system')",
                    n.id, n.kind
                ));
            }
            for p in &n.provides {
                if !PNT_KINDS.contains(&p.as_str()) {
                    return Err(format!("node '{}': unknown PNT kind '{p}'", n.id));
                }
            }
            for d in &n.depends_on {
                if !ids.contains(d.as_str()) {
                    return Err(format!("node '{}': depends on unknown node '{d}'", n.id));
                }
            }
            if n.is_source() && !n.needs.is_empty() {
                return Err(format!(
                    "node '{}': a source has no needs (use depends_on)",
                    n.id
                ));
            }
            if !(n.holdover_s.is_finite() && n.holdover_s >= 0.0) {
                return Err(format!("node '{}': holdover_s must not be negative", n.id));
            }
            for need in &n.needs {
                if !PNT_KINDS.contains(&need.pnt.as_str()) {
                    return Err(format!("node '{}': unknown PNT kind '{}'", n.id, need.pnt));
                }
                if need.from.is_empty() {
                    return Err(format!(
                        "node '{}': the {} need lists no provider",
                        n.id, need.pnt
                    ));
                }
                for p in need
                    .from
                    .iter()
                    .chain(&need.fallback)
                    .chain(&need.cross_check)
                {
                    if !ids.contains(p.as_str()) {
                        return Err(format!(
                            "node '{}': the {} need names unknown node '{p}'",
                            n.id, need.pnt
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// A node's state under a scenario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    Down,
    Degraded,
    Up,
}

/// The states of every node, and whether its output is GNSS-derived.
#[derive(Clone, Debug, PartialEq)]
pub struct Eval {
    pub state: BTreeMap<String, State>,
    pub gnss_derived: BTreeMap<String, bool>,
    /// For each system need: the provider in use (`None`: down).
    pub active: BTreeMap<(String, String), Option<String>>,
}

/// Evaluate the graph with the `failed` nodes down (and [`GNSS_SIS`] when listed). GNSS
/// sources with a holdover stay up when the signal in space is down unless `exhausted`
/// lists them.
pub fn evaluate(inv: &Inventory, failed: &BTreeSet<String>, exhausted: &BTreeSet<String>) -> Eval {
    let sis_down = failed.contains(GNSS_SIS);
    let mut state: BTreeMap<String, State> = inv
        .nodes
        .iter()
        .map(|n| (n.id.clone(), State::Down))
        .collect();
    let mut derived: BTreeMap<String, bool> = BTreeMap::new();
    let mut active = BTreeMap::new();
    // Least fixed point: start down, raise until nothing changes. A cycle with no outside
    // provider stays down.
    for _ in 0..=inv.nodes.len() + 1 {
        let mut changed = false;
        for n in &inv.nodes {
            let deps = n
                .depends_on
                .iter()
                .map(|d| state[d])
                .min()
                .unwrap_or(State::Up);
            let s = if failed.contains(&n.id) {
                State::Down
            } else if n.is_source() {
                let sis_ok = !gnss_type(&n.kind)
                    || !sis_down
                    || (n.holdover_s > 0.0 && !exhausted.contains(&n.id));
                if sis_ok {
                    deps
                } else {
                    State::Down
                }
            } else {
                let mut worst = deps;
                let mut der = false;
                for need in &n.needs {
                    let provides = |p: &String| {
                        inv.node(p).is_some_and(|x| {
                            x.provides.contains(&need.pnt)
                                || (x.is_source() && x.provides.is_empty())
                        })
                    };
                    let up = |p: &&String| state[*p] >= State::Degraded && provides(p);
                    let (st, prov) = match need.from.iter().find(up) {
                        Some(p) => (state[p].min(State::Up), Some(p.clone())),
                        None => match need.fallback.iter().find(up) {
                            Some(p) => (State::Degraded, Some(p.clone())),
                            None => (State::Down, None),
                        },
                    };
                    if let Some(p) = &prov {
                        der |= derived.get(p).copied().unwrap_or(false);
                    }
                    active.insert((n.id.clone(), need.pnt.clone()), prov);
                    worst = worst.min(st);
                }
                derived.insert(n.id.clone(), der);
                worst
            };
            if n.is_source() {
                derived.insert(n.id.clone(), gnss_type(&n.kind));
            }
            if state[&n.id] != s {
                state.insert(n.id.clone(), s);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Eval {
        state,
        gnss_derived: derived,
        active,
    }
}

/// A finding.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub id: String,
    /// `high`, `medium` or `info`.
    pub severity: String,
    pub node: Option<String>,
    pub text: String,
}

/// One critical function under GNSS denial.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DenialRow {
    pub node: String,
    pub name: String,
    /// `gnss-independent`, `gnss-with-fallback` or `gnss-dependent`.
    pub class: String,
    /// The state at the onset (holdover sources still up) and once holdover is exhausted.
    pub at_onset: State,
    pub after_holdover: State,
    /// When the state changes, seconds after the onset (from holdover), if it does.
    pub holdover_s: Option<f64>,
    /// Silently wrong under spoofing: runs on GNSS-derived data with no independent
    /// cross-check up.
    pub silent_under_spoofing: bool,
}

/// The audit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Audit {
    pub format: String,
    pub method: String,
    pub statement: String,
    pub label: String,
    pub asset: String,
    pub operator: String,
    pub sector: String,
    pub template_title: String,
    pub inventory_sha256: String,
    pub nodes: usize,
    pub sources: usize,
    pub gnss_sources: usize,
    pub critical: usize,
    pub baseline_down: Vec<String>,
    pub denial: Vec<DenialRow>,
    /// Single points of failure: node -> critical functions that go down with it alone.
    pub single_points: BTreeMap<String, Vec<String>>,
    pub template_missing: Vec<String>,
    pub findings: Vec<Finding>,
    pub status: Status,
}

/// The resilience status summarised from an audit (what a register aggregates).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Status {
    /// `L1` (a critical function stops without GNSS or has a single point of failure),
    /// `L2` (none stops, but some run degraded or would be silently wrong under spoofing),
    /// `L3` (every critical function keeps running without GNSS and is cross-checked).
    pub level: String,
    pub critical: usize,
    pub gnss_dependent: usize,
    pub gnss_with_fallback: usize,
    pub gnss_independent: usize,
    pub silent_under_spoofing: usize,
    pub single_points: usize,
    /// The shortest time a critical function keeps running without GNSS, s (`None`: no
    /// function changes state).
    pub min_autonomy_s: Option<f64>,
    pub required_autonomy_s: Option<f64>,
    pub template_missing: usize,
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn label(inv: &Inventory, id: &str) -> String {
    inv.node(id)
        .map(|n| {
            if n.name.is_empty() {
                n.id.clone()
            } else {
                n.name.clone()
            }
        })
        .unwrap_or_else(|| id.to_string())
}

/// Audit an inventory; `src` is its text (hashed into the audit).
pub fn audit(inv: &Inventory, src: &str) -> Result<Audit, String> {
    inv.validate()?;
    let tpl = templates::find(&inv.sector)?;
    let none = BTreeSet::new();
    let base = evaluate(inv, &none, &none);
    let critical: Vec<&Node> = inv.nodes.iter().filter(|n| n.critical).collect();
    let mut findings = Vec::new();
    let baseline_down: Vec<String> = base
        .state
        .iter()
        .filter(|(_, s)| **s == State::Down)
        .map(|(k, _)| k.clone())
        .collect();
    for d in &baseline_down {
        findings.push(Finding {
            id: format!("F-BASELINE-DOWN-{d}"),
            severity: "medium".into(),
            node: Some(d.clone()),
            text: format!(
                "{} cannot run even with everything up: a need has no provider of that kind, \
                 or it sits in a dependency cycle. Check the inventory.",
                label(inv, d)
            ),
        });
    }

    // ---- GNSS denied, onset and holdover ----
    let mut sis: BTreeSet<String> = BTreeSet::new();
    sis.insert(GNSS_SIS.into());
    let onset = evaluate(inv, &sis, &none);
    let mut holders: Vec<(&Node, f64)> = inv
        .nodes
        .iter()
        .filter(|n| gnss_type(&n.kind) && n.holdover_s > 0.0)
        .map(|n| (n, n.holdover_s))
        .collect();
    holders.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.id.cmp(&b.0.id)));
    let mut exhausted: BTreeSet<String> = BTreeSet::new();
    let mut change_at: BTreeMap<String, f64> = BTreeMap::new();
    let mut prev = onset.clone();
    for (n, h) in &holders {
        exhausted.insert(n.id.clone());
        let e = evaluate(inv, &sis, &exhausted);
        for c in &critical {
            if e.state[&c.id] < prev.state[&c.id] {
                change_at.entry(c.id.clone()).or_insert(*h);
            }
        }
        prev = e;
    }
    let after = prev;

    // ---- GNSS spoofed: sources up, data wrong ----
    let mut denial = Vec::new();
    for c in &critical {
        let b = base.state[&c.id];
        let a = after.state[&c.id];
        let class = if a == State::Up && onset.state[&c.id] == State::Up && b == State::Up {
            "gnss-independent"
        } else if a >= State::Degraded {
            "gnss-with-fallback"
        } else {
            "gnss-dependent"
        };
        // Silent: derived from GNSS in the baseline and no cross-check source that is up and
        // not GNSS-derived.
        let derived = base.gnss_derived.get(&c.id).copied().unwrap_or(false);
        let checked = c.needs.iter().any(|n| {
            n.cross_check.iter().any(|x| {
                base.state[x] >= State::Degraded
                    && !base.gnss_derived.get(x).copied().unwrap_or(false)
            })
        });
        let silent = derived && !checked;
        if class == "gnss-dependent" {
            findings.push(Finding {
                id: format!("F-GNSS-ONLY-{}", c.id),
                severity: "high".into(),
                node: Some(c.id.clone()),
                text: format!(
                    "{} stops without GNSS{}: no fallback of the needed kind is independent of \
                     the GNSS signal in space.",
                    label(inv, &c.id),
                    match change_at.get(&c.id) {
                        Some(h) => format!(" after {h:.0} s of holdover"),
                        None => String::new(),
                    }
                ),
            });
        } else if class == "gnss-with-fallback" && a == State::Degraded {
            findings.push(Finding {
                id: format!("F-GNSS-DEGRADED-{}", c.id),
                severity: "info".into(),
                node: Some(c.id.clone()),
                text: format!(
                    "{} carries on degraded without GNSS, on its fallback.",
                    label(inv, &c.id)
                ),
            });
        }
        if silent {
            findings.push(Finding {
                id: format!("F-SILENT-SPOOF-{}", c.id),
                severity: "high".into(),
                node: Some(c.id.clone()),
                text: format!(
                    "{} runs on GNSS-derived data with no independent cross-check listed: a \
                     spoofed input would not be noticed by the function itself.",
                    label(inv, &c.id)
                ),
            });
        }
        denial.push(DenialRow {
            node: c.id.clone(),
            name: label(inv, &c.id),
            class: class.into(),
            at_onset: onset.state[&c.id],
            after_holdover: a,
            holdover_s: change_at.get(&c.id).copied(),
            silent_under_spoofing: silent,
        });
    }
    if let Some(req) = inv.required_autonomy_s {
        for r in &denial {
            let short = match (r.class.as_str(), r.holdover_s) {
                (_, Some(h)) => h < req,
                ("gnss-dependent", None) => true,
                _ => false,
            };
            if short {
                findings.push(Finding {
                    id: format!("F-AUTONOMY-{}", r.node),
                    severity: "high".into(),
                    node: Some(r.node.clone()),
                    text: format!(
                        "{} keeps running without GNSS for {} s, less than the required {req:.0} s.",
                        r.name,
                        r.holdover_s.map(|h| format!("{h:.0}")).unwrap_or("0".into())
                    ),
                });
            }
        }
    }

    // ---- single points of failure ----
    let mut single_points: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for n in &inv.nodes {
        let mut f = BTreeSet::new();
        f.insert(n.id.clone());
        let e = evaluate(inv, &f, &none);
        let hit: Vec<String> = critical
            .iter()
            .filter(|c| {
                c.id != n.id && base.state[&c.id] > State::Down && e.state[&c.id] == State::Down
            })
            .map(|c| c.id.clone())
            .collect();
        if !hit.is_empty() {
            findings.push(Finding {
                id: format!("F-SPOF-{}", n.id),
                severity: "high".into(),
                node: Some(n.id.clone()),
                text: format!(
                    "Losing {} alone stops {}.",
                    label(inv, &n.id),
                    hit.iter()
                        .map(|h| label(inv, h))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
            single_points.insert(n.id.clone(), hit);
        }
    }
    // The signal in space as a common mode: more than one GNSS source, all lost together.
    let gnss_sources: Vec<&Node> = inv.nodes.iter().filter(|n| gnss_type(&n.kind)).collect();
    if gnss_sources.len() > 1 {
        findings.push(Finding {
            id: "F-COMMON-MODE-GNSS".into(),
            severity: "info".into(),
            node: None,
            text: format!(
                "{} GNSS sources are listed; they share the GNSS signal in space, so one \
                 jammer or spoofer in range affects all of them together: they are redundant \
                 against equipment faults, not against interference.",
                gnss_sources.len()
            ),
        });
    }

    // ---- isolation switches ----
    for n in &inv.nodes {
        let fed = n.needs.iter().any(|need| {
            need.from
                .iter()
                .any(|p| inv.node(p).is_some_and(|x| gnss_type(&x.kind)))
        });
        if fed && n.isolation_switch != Some(true) {
            findings.push(Finding {
                id: format!("F-NO-ISOLATION-{}", n.id),
                severity: if n.critical { "medium" } else { "info" }.into(),
                node: Some(n.id.clone()),
                text: format!(
                    "{} takes GNSS directly and {} switch its GNSS input off on its own.",
                    label(inv, &n.id),
                    if n.isolation_switch == Some(false) {
                        "cannot"
                    } else {
                        "the inventory does not say whether it can"
                    }
                ),
            });
        }
    }

    // ---- template coverage ----
    let have: BTreeSet<String> = inv
        .nodes
        .iter()
        .flat_map(|n| [Some(n.id.clone()), n.template_item.clone()])
        .flatten()
        .collect();
    let mut template_missing = Vec::new();
    for it in tpl.items {
        let present = have.contains(it.id) || it.aliases.iter().any(|a| have.contains(*a));
        if !present {
            template_missing.push(it.id.to_string());
            findings.push(Finding {
                id: format!("F-TEMPLATE-{}", it.id),
                severity: "info".into(),
                node: None,
                text: format!(
                    "The {} template expects '{}' ({}); the inventory does not list it. Add it, \
                     or note that the asset has none.",
                    tpl.id, it.name, it.why
                ),
            });
        }
    }

    let count = |c: &str| denial.iter().filter(|r| r.class == c).count();
    let silent = denial.iter().filter(|r| r.silent_under_spoofing).count();
    let min_autonomy = denial
        .iter()
        .filter_map(|r| {
            r.holdover_s
                .or((r.class == "gnss-dependent").then_some(0.0))
        })
        .min_by(|a, b| a.total_cmp(b));
    let level = if count("gnss-dependent") > 0 || !single_points.is_empty() {
        "L1"
    } else if count("gnss-with-fallback") > 0 || silent > 0 {
        "L2"
    } else {
        "L3"
    };
    let status = Status {
        level: level.into(),
        critical: critical.len(),
        gnss_dependent: count("gnss-dependent"),
        gnss_with_fallback: count("gnss-with-fallback"),
        gnss_independent: count("gnss-independent"),
        silent_under_spoofing: silent,
        single_points: single_points.len(),
        min_autonomy_s: min_autonomy,
        required_autonomy_s: inv.required_autonomy_s,
        template_missing: template_missing.len(),
    };
    let rank = |s: &str| match s {
        "high" => 0,
        "medium" => 1,
        _ => 2,
    };
    findings.sort_by(|a, b| {
        rank(&a.severity)
            .cmp(&rank(&b.severity))
            .then(a.id.cmp(&b.id))
    });
    Ok(Audit {
        format: AUDIT_FORMAT.into(),
        method: METHOD.into(),
        statement: STATEMENT.into(),
        label: "MODELLED".into(),
        asset: inv.asset.clone(),
        operator: inv.operator.clone(),
        sector: inv.sector.clone(),
        template_title: tpl.title.into(),
        inventory_sha256: sha256_hex(src.as_bytes()),
        nodes: inv.nodes.len(),
        sources: inv.nodes.iter().filter(|n| n.is_source()).count(),
        gnss_sources: gnss_sources.len(),
        critical: critical.len(),
        baseline_down,
        denial,
        single_points,
        template_missing,
        findings,
        status,
    })
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The graph's edges: (provider, consumer, label, kind) with kind `primary`, `fallback`,
/// `cross-check` or `depends`.
pub fn edges(inv: &Inventory) -> Vec<(String, String, String, String)> {
    let mut e = Vec::new();
    for n in &inv.nodes {
        if gnss_type(&n.kind) {
            e.push((
                GNSS_SIS.into(),
                n.id.clone(),
                "signal".into(),
                "depends".into(),
            ));
        }
        for d in &n.depends_on {
            e.push((d.clone(), n.id.clone(), "depends".into(), "depends".into()));
        }
        for need in &n.needs {
            for (list, kind) in [
                (&need.from, "primary"),
                (&need.fallback, "fallback"),
                (&need.cross_check, "cross-check"),
            ] {
                for p in list {
                    e.push((p.clone(), n.id.clone(), need.pnt.clone(), kind.into()));
                }
            }
        }
    }
    e
}

/// The graph as GraphML (yEd, Gephi, NetworkX read it).
pub fn graphml(inv: &Inventory) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n\
         <key id=\"name\" for=\"node\" attr.name=\"name\" attr.type=\"string\"/>\n\
         <key id=\"type\" for=\"node\" attr.name=\"type\" attr.type=\"string\"/>\n\
         <key id=\"critical\" for=\"node\" attr.name=\"critical\" attr.type=\"boolean\"/>\n\
         <key id=\"pnt\" for=\"edge\" attr.name=\"pnt\" attr.type=\"string\"/>\n\
         <key id=\"kind\" for=\"edge\" attr.name=\"kind\" attr.type=\"string\"/>\n\
         <graph id=\"pnt-dependency\" edgedefault=\"directed\">\n",
    );
    s.push_str(&format!(
        "<node id=\"{GNSS_SIS}\"><data key=\"name\">GNSS signal in space</data><data key=\"type\">signal</data><data key=\"critical\">false</data></node>\n"
    ));
    for n in &inv.nodes {
        s.push_str(&format!(
            "<node id=\"{}\"><data key=\"name\">{}</data><data key=\"type\">{}</data><data key=\"critical\">{}</data></node>\n",
            xml(&n.id),
            xml(&label(inv, &n.id)),
            xml(&n.kind),
            n.critical
        ));
    }
    for (i, (a, b, p, k)) in edges(inv).iter().enumerate() {
        s.push_str(&format!(
            "<edge id=\"e{i}\" source=\"{}\" target=\"{}\"><data key=\"pnt\">{}</data><data key=\"kind\">{}</data></edge>\n",
            xml(a),
            xml(b),
            xml(p),
            xml(k)
        ));
    }
    s.push_str("</graph>\n</graphml>\n");
    s
}

/// The graph as JSON (`nodes`, `edges`).
pub fn graph_json(inv: &Inventory) -> serde_json::Value {
    let mut nodes = vec![serde_json::json!({
        "id": GNSS_SIS, "name": "GNSS signal in space", "type": "signal", "critical": false
    })];
    nodes.extend(inv.nodes.iter().map(|n| {
        serde_json::json!({
            "id": n.id, "name": label(inv, &n.id), "type": n.kind, "critical": n.critical
        })
    }));
    let edges: Vec<serde_json::Value> = edges(inv)
        .into_iter()
        .map(|(a, b, p, k)| serde_json::json!({"from": a, "to": b, "pnt": p, "kind": k}))
        .collect();
    serde_json::json!({"format": "kshana-pro-pnt-graph/1", "nodes": nodes, "edges": edges})
}

/// The audit as text.
pub fn summary_text(a: &Audit) -> String {
    let mut s = format!(
        "PNT dependency audit: {} ({}), sector template {}.\n{}\n\nStatus {}: {} critical \
         functions; {} stop without GNSS, {} carry on degraded, {} do not depend on GNSS; {} \
         silently wrong under spoofing; {} single points of failure.\n\nFindings:\n",
        a.asset,
        a.operator,
        a.template_title,
        a.statement,
        a.status.level,
        a.status.critical,
        a.status.gnss_dependent,
        a.status.gnss_with_fallback,
        a.status.gnss_independent,
        a.status.silent_under_spoofing,
        a.status.single_points
    );
    for f in &a.findings {
        s.push_str(&format!("  [{}] {}: {}\n", f.severity, f.id, f.text));
    }
    s
}

/// Write the audit directory: audit.json, findings.csv, graph.graphml, graph.json,
/// inventory (copy), summary.txt.
pub fn write_dir(dir: &Path, a: &Audit, inv: &Inventory, src: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let w = |n: &str, t: &str| std::fs::write(dir.join(n), t).map_err(|e| format!("{n}: {e}"));
    w(
        RESULT_FILE,
        &(serde_json::to_string_pretty(a).map_err(|e| e.to_string())? + "\n"),
    )?;
    let mut csv = String::from("id,severity,node,text\n");
    for f in &a.findings {
        csv.push_str(&format!(
            "{},{},{},\"{}\"\n",
            f.id,
            f.severity,
            f.node.clone().unwrap_or_default(),
            f.text.replace('"', "'")
        ));
    }
    w("findings.csv", &csv)?;
    w("graph.graphml", &graphml(inv))?;
    w(
        "graph.json",
        &(serde_json::to_string_pretty(&graph_json(inv)).map_err(|e| e.to_string())? + "\n"),
    )?;
    w("inventory.txt", src)?;
    w("summary.txt", &summary_text(a))
}

/// A questionnaire answer drawn from an audit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Answer {
    pub id: String,
    pub question: String,
    pub answer: String,
    /// `from-audit` or `not-in-inventory`.
    pub basis: String,
}

/// Answer the standard government and regulator questions on GNSS reliance from an audit
/// (research 05 #46, 12 #1730). Each answer cites the audit; a question the inventory
/// cannot answer says so.
pub fn questionnaire(a: &Audit) -> Vec<Answer> {
    let names = |f: &dyn Fn(&DenialRow) -> bool| {
        let v: Vec<String> = a
            .denial
            .iter()
            .filter(|r| f(r))
            .map(|r| r.name.clone())
            .collect();
        if v.is_empty() {
            "none".to_string()
        } else {
            v.join("; ")
        }
    };
    let mut out = Vec::new();
    let mut q = |id: &str, question: &str, answer: String, basis: &str| {
        out.push(Answer {
            id: id.into(),
            question: question.into(),
            answer,
            basis: basis.into(),
        })
    };
    q(
        "Q1",
        "Which critical functions depend on GNSS for position, navigation or time?",
        format!(
            "Stop without GNSS: {}. Carry on degraded: {}. Independent of GNSS: {}.",
            names(&|r| r.class == "gnss-dependent"),
            names(&|r| r.class == "gnss-with-fallback"),
            names(&|r| r.class == "gnss-independent")
        ),
        "from-audit",
    );
    q(
        "Q2",
        "How long can the critical functions operate without GNSS?",
        match a.status.min_autonomy_s {
            Some(t) => format!(
                "The shortest is {t:.0} s{}.",
                a.status
                    .required_autonomy_s
                    .map(|r| format!(" (required: {r:.0} s)"))
                    .unwrap_or_default()
            ),
            None => "No critical function changes state over the holdover listed.".into(),
        },
        "from-audit",
    );
    q(
        "Q3",
        "Are there single points of failure in the PNT supply?",
        if a.single_points.is_empty() {
            "None found in the listed graph.".into()
        } else {
            a.single_points
                .iter()
                .map(|(k, v)| format!("{k} (stops {})", v.join(", ")))
                .collect::<Vec<_>>()
                .join("; ")
        },
        "from-audit",
    );
    q(
        "Q4",
        "How would a spoofed GNSS input be noticed?",
        format!(
            "Functions with no independent cross-check listed: {}.",
            names(&|r| r.silent_under_spoofing)
        ),
        "from-audit",
    );
    let iso: Vec<&Finding> = a
        .findings
        .iter()
        .filter(|f| f.id.starts_with("F-NO-ISOLATION-"))
        .collect();
    q(
        "Q5",
        "Can GNSS inputs be isolated from the systems they feed?",
        if iso.is_empty() {
            "Every system fed by GNSS directly has an isolation switch listed.".into()
        } else {
            format!(
                "Not stated or not possible for: {}.",
                iso.iter()
                    .filter_map(|f| f.node.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
        "from-audit",
    );
    q(
        "Q6",
        "Has the dependency inventory been checked against a sector reference?",
        if a.template_missing.is_empty() {
            format!(
                "Yes, against the {} template; nothing missing.",
                a.template_title
            )
        } else {
            format!(
                "Against the {} template; not listed: {}.",
                a.template_title,
                a.template_missing.join(", ")
            )
        },
        "from-audit",
    );
    q(
        "Q7",
        "Have GNSS interference events affected operations in the last 12 months?",
        "Not in the inventory: answer from incident records (for example signed forensic \
         reports)."
            .into(),
        "not-in-inventory",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIP: &str = r#"
format = "kshana-pro-pnt-inventory/1"
asset = "MV Test"
operator = "Test Shipping"
sector = "ship"
required_autonomy_s = 3600
[[node]]
id = "ant"
type = "antenna"
[[node]]
id = "gnss1"
type = "gnss"
provides = ["position", "velocity", "time"]
depends_on = ["ant"]
[[node]]
id = "gnss2"
type = "gnss"
provides = ["position", "velocity", "time"]
depends_on = ["ant"]
[[node]]
id = "gyro"
type = "gyro"
provides = ["heading"]
[[node]]
id = "log"
type = "log"
provides = ["velocity"]
[[node]]
id = "radar"
type = "radar"
provides = ["position"]
[[node]]
id = "dr"
type = "system"
provides = ["position"]
[[node.needs]]
pnt = "heading"
from = ["gyro"]
[[node.needs]]
pnt = "velocity"
from = ["log"]
[[node]]
id = "ecdis"
type = "system"
critical = true
isolation_switch = true
[[node.needs]]
pnt = "position"
from = ["gnss1", "gnss2"]
fallback = ["dr"]
cross_check = ["radar"]
[[node]]
id = "ais"
type = "system"
critical = true
[[node.needs]]
pnt = "position"
from = ["gnss1"]
"#;

    #[test]
    fn denial_spoofing_spof_and_template() {
        let inv = Inventory::parse(SHIP).unwrap();
        let a = audit(&inv, SHIP).unwrap();
        let row = |id: &str| a.denial.iter().find(|r| r.node == id).unwrap().clone();
        assert_eq!(row("ecdis").class, "gnss-with-fallback");
        assert_eq!(row("ecdis").after_holdover, State::Degraded);
        assert!(!row("ecdis").silent_under_spoofing, "radar cross-check");
        assert_eq!(row("ais").class, "gnss-dependent");
        assert!(row("ais").silent_under_spoofing);
        // The shared antenna stops AIS (ECDIS falls back to DR); gnss1 alone stops AIS.
        assert_eq!(a.single_points["ant"], vec!["ais".to_string()]);
        assert_eq!(a.single_points["gnss1"], vec!["ais".to_string()]);
        assert!(!a.single_points.contains_key("gnss2"));
        assert_eq!(a.status.level, "L1");
        assert!(a.findings.iter().any(|f| f.id == "F-NO-ISOLATION-ais"));
        assert!(!a.findings.iter().any(|f| f.id == "F-NO-ISOLATION-ecdis"));
        assert!(a.findings.iter().any(|f| f.id == "F-COMMON-MODE-GNSS"));
        assert!(a.findings.iter().any(|f| f.id == "F-AUTONOMY-ais"));
        assert!(a.template_missing.contains(&"vdr".to_string()));
        assert!(!a.template_missing.contains(&"ecdis".to_string()));
        assert_eq!(a.findings[0].severity, "high");
        let g = graphml(&inv);
        assert!(g.contains("source=\"gnss-signal-in-space\" target=\"gnss1\""));
        assert!(g.contains("<data key=\"kind\">fallback</data>"));
        let qs = questionnaire(&a);
        assert!(qs[0].answer.contains("Stop without GNSS:"));
        assert_eq!(qs.last().unwrap().basis, "not-in-inventory");
    }

    #[test]
    fn holdover_delays_the_stop() {
        let src = r#"
format = "kshana-pro-pnt-inventory/1"
asset = "Site"
operator = "Op"
sector = "water"
required_autonomy_s = 86400
[[node]]
id = "gm"
type = "gnss"
provides = ["time"]
holdover_s = 14400
[[node]]
id = "scada"
type = "system"
critical = true
[[node.needs]]
pnt = "time"
from = ["gm"]
"#;
        let inv = Inventory::parse(src).unwrap();
        let a = audit(&inv, src).unwrap();
        let r = &a.denial[0];
        assert_eq!(r.at_onset, State::Up);
        assert_eq!(r.after_holdover, State::Down);
        assert_eq!(r.holdover_s, Some(14400.0));
        assert_eq!(a.status.min_autonomy_s, Some(14400.0));
        assert!(a.findings.iter().any(|f| f.id == "F-AUTONOMY-scada"));
    }

    #[test]
    fn bad_inventories_fail() {
        assert!(Inventory::parse(
            "format = \"x\"\nasset=\"a\"\noperator=\"o\"\nsector=\"ship\"\nnode=[]"
        )
        .is_err());
        let bad = SHIP.replace("from = [\"gnss1\"]\n", "from = [\"nope\"]\n");
        assert!(Inventory::parse(&bad).is_err());
        let json = serde_json::to_string(&Inventory::parse(SHIP).unwrap()).unwrap();
        assert!(
            Inventory::parse(&json).is_ok(),
            "JSON inventories are read too"
        );
    }
}
