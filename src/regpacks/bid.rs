// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Bid evidence attestation: declared export status of a bill of materials and the test
//! evidence chain (E29 B2906; research 07b R10, S20; 08b Q7; 12 #1651 to #1653)
//!
//! Buyers ask bidders for "no ITAR components" and for jamming-resilience test results (the
//! Dstl resilient timing and positioning RFI, for example). This module turns the bidder's
//! **declared** bill of materials (each line's origin, export regime and classification, and
//! who declared it) and its signed Kshana test evidence into one signed attestation:
//!
//! * per line, the declared regime and classification, with lines that declare an ITAR (USML)
//!   classification and lines with no declaration listed separately;
//! * the test evidence, each directory verified, with its signature hash;
//! * a chain hash over the bill of materials and every evidence signature, recomputed by
//!   [`check_dir`].
//!
//! Kshana does not classify items: the attestation states what the bidder declared and which
//! evidence was attached. Export classification is the bidder's and its authority's.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{esc, page_head, sha256_hex, write_json, EvidenceItem};

/// The attestation's format tag.
pub const FORMAT: &str = "kshana-pro-bid-attestation/1";
/// The file the attestation writes.
pub const RESULT_FILE: &str = "bid-attestation.json";
/// The bill of materials as copied into the signed directory.
pub const BOM_FILE: &str = "bom.csv";
/// The method id.
pub const METHOD: &str = "kshana-pro/bid-attestation/v1";
/// The BOM header.
pub const BOM_HEADER: &str =
    "part,description,maker,origin_country,export_regime,classification,declared_by";

/// The bid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bid {
    pub bidder: String,
    pub tender: String,
    /// The buyer's clauses as the bidder cites them (short).
    #[serde(default)]
    pub clauses: Vec<String>,
    /// The product the bill of materials is for.
    pub product: String,
}

impl Bid {
    pub fn parse(src: &str) -> Result<Bid, String> {
        toml::from_str(src).map_err(|e| format!("bid: {e}"))
    }
}

/// One BOM line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Line {
    pub part: String,
    pub description: String,
    pub maker: String,
    pub origin_country: String,
    pub export_regime: String,
    pub classification: String,
    pub declared_by: String,
    /// `itar declared`, `other regime declared`, `not listed (declared)` or `not declared`.
    pub status: &'static str,
}

fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if q && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => q = !q,
            ',' if !q => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

fn status_of(regime: &str, class: &str) -> &'static str {
    let r = regime.to_ascii_lowercase();
    let c = class.to_ascii_lowercase();
    if r.contains("itar") || r.contains("usml") || c.starts_with("usml") {
        "itar declared"
    } else if r.is_empty() {
        "not declared"
    } else if r == "not listed" || r == "none" {
        "not listed (declared)"
    } else {
        "other regime declared"
    }
}

/// Parse the BOM.
pub fn parse_bom(text: &str) -> Result<Vec<Line>, String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let head = lines.next().ok_or("bom: empty")?;
    if split_csv(head).join(",") != BOM_HEADER {
        return Err(format!("bom: header must be `{BOM_HEADER}`"));
    }
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        let f = split_csv(l);
        if f.len() != 7 {
            return Err(format!("bom line {}: {} fields, want 7", i + 2, f.len()));
        }
        if f[0].is_empty() {
            return Err(format!("bom line {}: no part", i + 2));
        }
        out.push(Line {
            status: status_of(&f[4], &f[5]),
            part: f[0].clone(),
            description: f[1].clone(),
            maker: f[2].clone(),
            origin_country: f[3].clone(),
            export_regime: f[4].clone(),
            classification: f[5].clone(),
            declared_by: f[6].clone(),
        });
    }
    if out.is_empty() {
        return Err("bom: no lines".into());
    }
    Ok(out)
}

/// The attestation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attestation {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub statement: &'static str,
    pub bid: Bid,
    pub bom_sha256: String,
    pub lines: Vec<Line>,
    pub itar_declared: Vec<String>,
    pub not_declared: Vec<String>,
    pub origin_missing: Vec<String>,
    pub by_regime: BTreeMap<String, usize>,
    pub by_origin: BTreeMap<String, usize>,
    /// One line summing up the declared status, in the declaration's terms.
    pub summary: String,
    pub evidence: Vec<EvidenceItem>,
    /// SHA-256 over the BOM hash and every evidence signature hash, in order, newline-joined.
    pub chain_sha256: String,
}

/// What every attestation says about itself.
pub const STATEMENT: &str = "Attests the export status the bidder declared for each line of the \
bill of materials and the test evidence attached, each verified and hashed. Kshana does not \
classify items; the classification is the bidder's and its export authority's.";

fn chain(bom_sha: &str, ev: &[EvidenceItem]) -> String {
    let mut s = bom_sha.to_string();
    for e in ev {
        s.push('\n');
        s.push_str(&e.sha256);
    }
    sha256_hex(s.as_bytes())
}

/// Build the attestation.
pub fn build(bid: Bid, bom_text: &str, evidence: Vec<EvidenceItem>) -> Result<Attestation, String> {
    let lines = parse_bom(bom_text)?;
    let pick = |f: &dyn Fn(&Line) -> bool| -> Vec<String> {
        lines
            .iter()
            .filter(|l| f(l))
            .map(|l| l.part.clone())
            .collect()
    };
    let itar = pick(&|l| l.status == "itar declared");
    let nd = pick(&|l| l.status == "not declared");
    let om = pick(&|l| l.origin_country.is_empty());
    let mut by_regime = BTreeMap::new();
    let mut by_origin = BTreeMap::new();
    for l in &lines {
        *by_regime
            .entry(if l.export_regime.is_empty() {
                "(not declared)".to_string()
            } else {
                l.export_regime.clone()
            })
            .or_insert(0) += 1;
        *by_origin
            .entry(if l.origin_country.is_empty() {
                "(not declared)".to_string()
            } else {
                l.origin_country.clone()
            })
            .or_insert(0) += 1;
    }
    let summary = if itar.is_empty() && nd.is_empty() {
        format!(
            "None of the {} lines declares an ITAR (USML) classification, and every line has a declaration.",
            lines.len()
        )
    } else {
        format!(
            "{} of {} lines declare an ITAR (USML) classification; {} lines have no declaration.",
            itar.len(),
            lines.len(),
            nd.len()
        )
    };
    let bom_sha256 = sha256_hex(bom_text.as_bytes());
    Ok(Attestation {
        format: FORMAT,
        method: METHOD,
        label: super::LABEL,
        statement: STATEMENT,
        bid,
        chain_sha256: chain(&bom_sha256, &evidence),
        bom_sha256,
        lines,
        itar_declared: itar,
        not_declared: nd,
        origin_missing: om,
        by_regime,
        by_origin,
        summary,
        evidence,
    })
}

/// The page.
pub fn render_html(a: &Attestation) -> String {
    let mut h = page_head("Bid evidence attestation");
    h.push_str(&format!(
        "<h1>Bid evidence attestation: {}</h1><p class=\"l\">{}</p><p>{}</p>\
         <p>Bidder: {}. Tender: {}. Clauses cited: {}.</p><p><b>{}</b></p>\
         <h2>Bill of materials (as declared)</h2><table><tr><th>Part</th><th>Description</th><th>Maker</th><th>Origin</th><th>Regime</th><th>Classification</th><th>Declared by</th><th>Status</th></tr>",
        esc(&a.bid.product),
        esc(a.label),
        esc(a.statement),
        esc(&a.bid.bidder),
        esc(&a.bid.tender),
        esc(&a.bid.clauses.join("; ")),
        esc(&a.summary)
    ));
    for l in &a.lines {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&l.part),
            esc(&l.description),
            esc(&l.maker),
            esc(&l.origin_country),
            esc(&l.export_regime),
            esc(&l.classification),
            esc(&l.declared_by),
            l.status
        ));
    }
    h.push_str("</table><h2>Test evidence</h2><table><tr><th>Item</th><th>Products</th><th>Signer</th><th>Signature</th><th>SHA-256</th></tr>");
    for e in &a.evidence {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
            esc(&e.path),
            esc(&e.products.join("; ")),
            esc(e.signer.as_deref().unwrap_or("")),
            if e.usable() {
                "valid"
            } else {
                "INVALID or not trusted"
            },
            esc(&e.sha256)
        ));
    }
    h.push_str(&format!(
        "</table><p>BOM SHA-256 <code>{}</code>; chain SHA-256 <code>{}</code>.</p></body></html>\n",
        a.bom_sha256, a.chain_sha256
    ));
    h
}

/// Write the directory (attestation, page, and the BOM itself so the signature covers it).
pub fn write_dir(out: &Path, a: &Attestation, bom_text: &str) -> Result<(), String> {
    write_json(out, RESULT_FILE, a)?;
    std::fs::write(out.join(BOM_FILE), bom_text).map_err(|e| e.to_string())?;
    std::fs::write(out.join("report.html"), render_html(a)).map_err(|e| e.to_string())
}

/// Recompute the BOM hash and the chain of a written attestation; the problems found.
/// (The signature itself is checked by `signed_dossier::verify`.)
pub fn check_dir(dir: &Path) -> Result<Vec<String>, String> {
    let t = std::fs::read_to_string(dir.join(RESULT_FILE))
        .map_err(|e| format!("{}: {e}", dir.join(RESULT_FILE).display()))?;
    let v: serde_json::Value = serde_json::from_str(&t).map_err(|e| e.to_string())?;
    let mut p = Vec::new();
    if v["format"].as_str() != Some(FORMAT) {
        p.push(format!("{RESULT_FILE}: not {FORMAT}"));
        return Ok(p);
    }
    let bom = std::fs::read(dir.join(BOM_FILE)).map_err(|e| format!("{BOM_FILE}: {e}"))?;
    let bom_sha = sha256_hex(&bom);
    if v["bom_sha256"].as_str() != Some(bom_sha.as_str()) {
        p.push("bom.csv does not match bom_sha256".into());
    }
    let mut s = bom_sha;
    for e in v["evidence"].as_array().cloned().unwrap_or_default() {
        s.push('\n');
        s.push_str(e["sha256"].as_str().unwrap_or(""));
    }
    if v["chain_sha256"].as_str() != Some(sha256_hex(s.as_bytes()).as_str()) {
        p.push("chain_sha256 does not match the BOM and evidence hashes".into());
    }
    Ok(p)
}
