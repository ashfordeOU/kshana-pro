// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Space link-security evidence (E29 B2905; research 06 E8, F5, I1 to I10)
//!
//! From a mission's **declared** link and command configuration (the operator's declaration;
//! Kshana does not listen to any link), this lists every link in cleartext, every telecommand
//! path accepted without authentication, and every link whose protection can be switched off,
//! and checks the operator's own "no encryption, no fly" policy. The output is evidence against
//! the cybersecurity rows of the `eu-space-act-cyber` catalogue (the Commission proposal; its
//! text not read in this work, so rows are at topic level).

use serde::{Deserialize, Serialize};

use super::{esc, page_head, sha256_hex, write_json};

/// The evidence format tag.
pub const FORMAT: &str = "kshana-pro-link-security/1";
/// The file the evidence writes.
pub const RESULT_FILE: &str = "link-security.json";
/// The method id.
pub const METHOD: &str = "kshana-pro/link-security/v1";
/// Link functions.
pub const FUNCTIONS: [&str; 6] = [
    "telecommand",
    "telemetry",
    "payload-data",
    "ranging",
    "ground-network",
    "user-terminal",
];

/// One declared link.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub id: String,
    pub function: String,
    #[serde(default)]
    pub band: String,
    /// `none`, or the protection's name as declared.
    pub encryption: String,
    /// `none`, or the scheme's name as declared.
    pub authentication: String,
    /// Whether the protection can be switched off in operation.
    #[serde(default)]
    pub crypto_can_be_disabled: bool,
    /// Whether the link carries commands (always true for telecommand).
    #[serde(default)]
    pub carries_commands: bool,
    #[serde(default)]
    pub key_management: String,
    #[serde(default)]
    pub notes: String,
}

impl Link {
    fn commands(&self) -> bool {
        self.function == "telecommand" || self.carries_commands
    }
    fn clear(&self) -> bool {
        self.encryption.trim().eq_ignore_ascii_case("none") || self.encryption.trim().is_empty()
    }
    fn unauth(&self) -> bool {
        self.authentication.trim().eq_ignore_ascii_case("none")
            || self.authentication.trim().is_empty()
    }
}

/// The operator's declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkDecl {
    pub mission: String,
    /// Whether the mission has propulsion (an unprotected command path then also bears on
    /// space safety).
    #[serde(default)]
    pub propulsion: bool,
    #[serde(rename = "link")]
    pub links: Vec<Link>,
}

impl LinkDecl {
    pub fn parse(src: &str) -> Result<LinkDecl, String> {
        let d: LinkDecl = toml::from_str(src).map_err(|e| format!("links: {e}"))?;
        if d.links.is_empty() {
            return Err("links: no [[link]]".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for l in &d.links {
            if !FUNCTIONS.contains(&l.function.as_str()) {
                return Err(format!(
                    "link {}: function '{}' is not one of {}",
                    l.id,
                    l.function,
                    FUNCTIONS.join(", ")
                ));
            }
            if !ids.insert(l.id.as_str()) {
                return Err(format!("links: duplicate id {}", l.id));
            }
        }
        Ok(d)
    }
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub link: String,
    pub rule: &'static str,
    /// `gap` or `note`.
    pub severity: &'static str,
    pub text: String,
}

/// The evidence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LinkSecurity {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub statement: &'static str,
    pub mission: String,
    pub declaration_sha256: String,
    pub links: Vec<Link>,
    pub cleartext_links: Vec<String>,
    pub unauthenticated_command_paths: Vec<String>,
    pub findings: Vec<Finding>,
    /// The operator's policy: every command path encrypted and authenticated, and its
    /// protection cannot be switched off.
    pub no_encryption_no_fly: &'static str,
    pub gaps: usize,
}

/// What every output says about itself.
pub const STATEMENT: &str = "Computed from the link configuration the operator declared; \
Kshana did not observe any link. Evidence of what is declared, against the topic-level rows of \
the catalogue named; not a security assessment of the mission.";

/// Run the checks.
pub fn run(d: &LinkDecl, src: &str) -> LinkSecurity {
    let mut f = Vec::new();
    let mut push = |l: &Link, rule, severity, text: String| {
        f.push(Finding {
            link: l.id.clone(),
            rule,
            severity,
            text,
        })
    };
    for l in &d.links {
        if l.commands() && l.unauth() {
            push(
                l,
                "LNK-1",
                "gap",
                format!(
                    "commands accepted without authentication{}",
                    if d.propulsion {
                        " on a mission with propulsion"
                    } else {
                        ""
                    }
                ),
            );
        }
        if l.commands() && l.clear() {
            push(l, "LNK-2", "gap", "command path in cleartext".into());
        } else if l.clear() {
            push(
                l,
                "LNK-2",
                "note",
                format!("{} link in cleartext", l.function),
            );
        }
        if l.crypto_can_be_disabled && (!l.clear() || !l.unauth()) {
            push(
                l,
                "LNK-3",
                if l.commands() { "gap" } else { "note" },
                "protection can be switched off in operation".into(),
            );
        }
        if (!l.clear() || !l.unauth()) && l.key_management.trim().is_empty() {
            push(
                l,
                "LNK-4",
                "note",
                "protection declared with no key-management description".into(),
            );
        }
    }
    let policy_met = d
        .links
        .iter()
        .filter(|l| l.commands())
        .all(|l| !l.clear() && !l.unauth() && !l.crypto_can_be_disabled)
        && d.links.iter().any(|l| l.commands());
    LinkSecurity {
        format: FORMAT,
        method: METHOD,
        label: super::LABEL,
        statement: STATEMENT,
        mission: d.mission.clone(),
        declaration_sha256: sha256_hex(src.as_bytes()),
        cleartext_links: d
            .links
            .iter()
            .filter(|l| l.clear())
            .map(|l| l.id.clone())
            .collect(),
        unauthenticated_command_paths: d
            .links
            .iter()
            .filter(|l| l.commands() && l.unauth())
            .map(|l| l.id.clone())
            .collect(),
        links: d.links.clone(),
        gaps: f.iter().filter(|x| x.severity == "gap").count(),
        findings: f,
        no_encryption_no_fly: if policy_met {
            "met as declared"
        } else {
            "not met as declared"
        },
    }
}

/// The page.
pub fn render_html(s: &LinkSecurity) -> String {
    let mut h = page_head("Link-security evidence");
    h.push_str(&format!(
        "<h1>Link-security evidence: {}</h1><p class=\"l\">{}</p><p>{}</p>\
         <p>Cleartext links: {}. Command paths without authentication: {}. \"No encryption, no fly\" policy: <b>{}</b>.</p>\
         <table><tr><th>Link</th><th>Rule</th><th>Severity</th><th>Finding</th></tr>",
        esc(&s.mission),
        esc(s.label),
        esc(s.statement),
        esc(&s.cleartext_links.join(", ")),
        esc(&s.unauthenticated_command_paths.join(", ")),
        s.no_encryption_no_fly
    ));
    for x in &s.findings {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&x.link),
            x.rule,
            x.severity,
            esc(&x.text)
        ));
    }
    h.push_str("</table></body></html>\n");
    h
}

/// Write the directory.
pub fn write_dir(out: &std::path::Path, s: &LinkSecurity) -> Result<(), String> {
    write_json(out, RESULT_FILE, s)?;
    std::fs::write(out.join("report.html"), render_html(s)).map_err(|e| e.to_string())
}
