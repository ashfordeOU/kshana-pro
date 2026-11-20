// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # MASS Code PNT evidence: override independence and the Operational Envelope template
//! (E29 B2902; research 07b R01, R02, R04)
//!
//! The IMO MASS Code (MSC.595(111), adopted 21 May 2026, non-mandatory) has no GNSS text; it
//! sets goals. Two of them carry PNT evidence (research 07b, the Code read in full):
//!
//! * the override of an autonomous function must be independent of the systems it controls
//!   (16.5 EP1, as cited in the research), so the override path must not depend, directly or
//!   through another component, on GNSS or on a controlled system;
//! * the navigation system keeps its function under the expected conditions of the
//!   Operational Envelope (OE) and can be operated by alternative means after a failure
//!   (chapter 16; 16.3.2 EP4 as cited in the research).
//!
//! The check walks a **declared** dependency graph (the owner's declaration; Kshana does not
//! inspect the ship) from every component of the override path and reports each dependency
//! chain that reaches a GNSS component or a controlled system. The OE template lists the
//! owner's OE conditions with PNT content and the Kshana evidence formats named for each.
//! Experience-building data collection is the `imo-mass-code` catalogue's row MASS-EBP.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{esc, page_head, product_name, sha256_hex, write_json};

/// The check's format tag.
pub const FORMAT: &str = "kshana-pro-mass-override-check/1";
/// The file the check writes.
pub const RESULT_FILE: &str = "mass-check.json";
/// The method id.
pub const METHOD: &str = "kshana-pro/mass-override-independence/v1";
/// Component kinds.
pub const KINDS: [&str; 9] = [
    "gnss",
    "sensor",
    "processor",
    "controller",
    "actuator",
    "link",
    "operator",
    "power",
    "other",
];

/// One component of the declared architecture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

/// One Operational Envelope condition with PNT content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OeCondition {
    pub id: String,
    pub description: String,
    /// The PNT condition the OE expects the ship to meet, in the owner's words.
    pub pnt_condition: String,
    /// The expected behaviour.
    pub expected: String,
    /// Kshana evidence format tags that speak to it.
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// The owner's declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MassDecl {
    pub vessel: String,
    #[serde(rename = "component")]
    pub components: Vec<Component>,
    pub override_path: Vec<String>,
    /// The systems the override acts on.
    pub controlled: Vec<String>,
    #[serde(rename = "oe_condition", default)]
    pub oe: Vec<OeCondition>,
}

impl MassDecl {
    pub fn parse(src: &str) -> Result<MassDecl, String> {
        let d: MassDecl = toml::from_str(src).map_err(|e| format!("MASS declaration: {e}"))?;
        d.validate()?;
        Ok(d)
    }

    pub fn validate(&self) -> Result<(), String> {
        let ids: BTreeSet<&str> = self.components.iter().map(|c| c.id.as_str()).collect();
        if ids.len() != self.components.len() {
            return Err("MASS declaration: duplicate component id".into());
        }
        for c in &self.components {
            if !KINDS.contains(&c.kind.as_str()) {
                return Err(format!(
                    "component {}: kind '{}' is not one of {}",
                    c.id,
                    c.kind,
                    KINDS.join(", ")
                ));
            }
            for d in &c.depends_on {
                if !ids.contains(d.as_str()) {
                    return Err(format!("component {}: unknown dependency '{d}'", c.id));
                }
            }
        }
        if self.override_path.is_empty() {
            return Err("MASS declaration: override_path is empty".into());
        }
        for x in self.override_path.iter().chain(&self.controlled) {
            if !ids.contains(x.as_str()) {
                return Err(format!("MASS declaration: unknown component '{x}'"));
            }
        }
        for o in &self.oe {
            for f in &o.evidence {
                if product_name(f).is_none() {
                    return Err(format!(
                        "OE condition {}: unknown evidence format {f}",
                        o.id
                    ));
                }
            }
        }
        Ok(())
    }
}

/// A dependency chain from an override component to an offending component.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Chain {
    pub from: String,
    /// `gnss` or `controlled system`.
    pub reaches: &'static str,
    pub path: Vec<String>,
}

/// The check's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MassCheck {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub vessel: String,
    pub declaration_sha256: String,
    pub statement: &'static str,
    pub override_path: Vec<String>,
    pub controlled: Vec<String>,
    /// No chain from the override path reaches a GNSS component.
    pub override_gnss_independent: bool,
    /// No chain from the override path reaches a controlled system.
    pub override_independent_of_controlled: bool,
    pub chains: Vec<Chain>,
    pub oe_template: Vec<OeCondition>,
    /// OE conditions with no evidence format named.
    pub oe_without_evidence: Vec<String>,
}

/// The statement every check carries.
pub const STATEMENT: &str = "Computed over the dependency graph the owner declared; Kshana did \
not inspect the ship. A chain listed is evidence against the override-independence goal of the \
MASS Code as the owner declared the architecture; an empty list means no such chain was \
declared.";

fn chains_from(
    start: &str,
    graph: &BTreeMap<&str, &Component>,
    controlled: &BTreeSet<&str>,
) -> Vec<Chain> {
    // Depth-first over simple paths; each offending node is reported once per start, by its
    // first chain found depth-first (dependencies in declared order).
    let mut out = Vec::new();
    let mut reported = BTreeSet::new();
    let mut stack: Vec<Vec<&str>> = vec![vec![start]];
    while let Some(path) = stack.pop() {
        let last = *path.last().expect("non-empty");
        let c = graph[last];
        let reaches = if c.kind == "gnss" {
            Some("gnss")
        } else if controlled.contains(last) {
            Some("controlled system")
        } else {
            None
        };
        if let Some(r) = reaches {
            if reported.insert((last, r)) {
                out.push(Chain {
                    from: start.to_string(),
                    reaches: r,
                    path: path.iter().map(|s| s.to_string()).collect(),
                });
            }
        }
        for d in c.depends_on.iter().rev() {
            if !path.contains(&d.as_str()) {
                let mut p = path.clone();
                p.push(d.as_str());
                stack.push(p);
            }
        }
    }
    out
}

/// Run the check.
pub fn run(d: &MassDecl, src: &str) -> MassCheck {
    let graph: BTreeMap<&str, &Component> =
        d.components.iter().map(|c| (c.id.as_str(), c)).collect();
    let controlled: BTreeSet<&str> = d.controlled.iter().map(String::as_str).collect();
    let mut chains = Vec::new();
    for s in &d.override_path {
        chains.extend(chains_from(s, &graph, &controlled));
    }
    MassCheck {
        format: FORMAT,
        method: METHOD,
        label: super::LABEL,
        vessel: d.vessel.clone(),
        declaration_sha256: sha256_hex(src.as_bytes()),
        statement: STATEMENT,
        override_path: d.override_path.clone(),
        controlled: d.controlled.clone(),
        override_gnss_independent: !chains.iter().any(|c| c.reaches == "gnss"),
        override_independent_of_controlled: !chains
            .iter()
            .any(|c| c.reaches == "controlled system"),
        chains,
        oe_without_evidence: d
            .oe
            .iter()
            .filter(|o| o.evidence.is_empty())
            .map(|o| o.id.clone())
            .collect(),
        oe_template: d.oe.clone(),
    }
}

/// The check's page.
pub fn render_html(m: &MassCheck) -> String {
    let mut h = page_head("MASS override independence");
    h.push_str(&format!(
        "<h1>MASS override independence and OE PNT template: {}</h1><p class=\"l\">{}</p><p>{}</p>\
         <p>Override path: {}. GNSS-independent as declared: <b>{}</b>. Independent of the controlled systems as declared: <b>{}</b>.</p>\
         <h2>Dependency chains</h2><table><tr><th>From</th><th>Reaches</th><th>Chain</th></tr>",
        esc(&m.vessel),
        esc(m.label),
        esc(m.statement),
        esc(&m.override_path.join(", ")),
        if m.override_gnss_independent { "yes" } else { "NO" },
        if m.override_independent_of_controlled { "yes" } else { "NO" },
    ));
    for c in &m.chains {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&c.from),
            c.reaches,
            esc(&c.path.join(" → "))
        ));
    }
    h.push_str("</table><h2>Operational Envelope PNT template</h2><table><tr><th>OE condition</th><th>PNT condition</th><th>Expected</th><th>Evidence</th></tr>");
    for o in &m.oe_template {
        h.push_str(&format!(
            "<tr><td>{}: {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&o.id),
            esc(&o.description),
            esc(&o.pnt_condition),
            esc(&o.expected),
            if o.evidence.is_empty() {
                "none named (gap)".to_string()
            } else {
                esc(&o
                    .evidence
                    .iter()
                    .map(|f| product_name(f).unwrap_or(f))
                    .collect::<Vec<_>>()
                    .join("; "))
            }
        ));
    }
    h.push_str("</table></body></html>\n");
    h
}

/// Write the check's directory.
pub fn write_dir(out: &std::path::Path, m: &MassCheck) -> Result<(), String> {
    write_json(out, RESULT_FILE, m)?;
    std::fs::write(out.join("report.html"), render_html(m)).map_err(|e| e.to_string())
}
