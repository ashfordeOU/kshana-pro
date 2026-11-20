// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Regulator, type-approval and framework packs (research 2026-11-20 gaps, epic E29)
//!
//! Evidence packs that show, requirement by requirement, which piece of signed Kshana
//! evidence speaks to it and what is missing. A pack is a **pre-assessment**: an aid for
//! preparing a submission to a notified body, a class society, a flag state, a regulator or
//! a buyer. It never says that a product or an operator meets a regulation; that judgement
//! belongs to the assessor and the authority.
//!
//! * [`Catalogue`] (`report-templates/regpacks/<id>.json`): one per document. Each row
//!   names the document, its version and the clause (or exactly [`CLAUSE_UNCONFIRMED`] when
//!   the clause was not read in the research), the basis for the citation, a short
//!   paraphrase of the requirement, the Kshana evidence formats that speak to it and the gap.
//! * [`assess`]: reads the operator's signed evidence directories (and, optionally, a report
//!   of the open engine's standards-pack runner), verifies every signature, and states per row
//!   `evidence supplied`, `no evidence supplied` or `not assessed by Kshana`.
//! * [`bam`]: a bridge alert mapping check of PNT-integrity alerts, replayed over the
//!   PNT data processing reference ([`crate::pntdp`]) output.
//! * [`mass`]: the MASS Code override-independence check over a declared dependency graph.
//! * [`complaint`]: a harmful-interference report pack shaped after ITU Radio Regulations
//!   Appendix 10, filled from interference episodes and observations; areas only, at
//!   [`complaint::CELL_DEG`] granularity, never an emitter position.
//! * [`links`]: link-security evidence from a declared space link and command configuration.
//! * [`bid`]: a signed bill-of-materials origin attestation with the test evidence chain for
//!   a bid.
//!
//! Licence features (explicit tiers in `license::explicit_tier`): [`FEATURE`] (Pro) for
//! assess, bam, mass and links; [`FEATURE_COMPLAINT`] (Pro) for the interference report
//! pack; [`FEATURE_BID`] (Pro) for the bid attestation. Listing, showing, examples and
//! verification are free.
//!
//! **Evidence label: MODELLED.** The catalogues are paraphrases of the documents named, read
//! as each catalogue's `source.read` says; the checks run on the data supplied under rules
//! written into every output. Nothing here has been reviewed by a notified body, class
//! society or authority.

pub mod bam;
pub mod bid;
pub mod complaint;
pub mod links;
pub mod mass;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The licence feature of assess, bam, mass and links.
pub const FEATURE: &str = "pro-regulator-pack";
/// The licence feature of the interference report pack.
pub const FEATURE_COMPLAINT: &str = "pro-interference-complaint";
/// The licence feature of the bid attestation.
pub const FEATURE_BID: &str = "pro-bid-attestation";

/// The catalogue format.
pub const CATALOGUE_FORMAT: &str = "kshana-pro-regulator-catalogue/1";
/// The pre-assessment format.
pub const FORMAT: &str = "kshana-pro-pre-assessment/1";
/// The file a pre-assessment writes.
pub const RESULT_FILE: &str = "pre-assessment.json";
/// The clause text of a row whose clause the research did not read.
pub const CLAUSE_UNCONFIRMED: &str = "clause to be confirmed";
/// The open engine's standards-pack report schema.
pub const STANDARDS_REPORT: &str = "kshana.standards-pack-report/1";

/// The evidence label of every output of this module.
pub const LABEL: &str = "MODELLED: a pre-assessment of the evidence supplied against the \
requirements as paraphrased in the catalogue. Not reviewed by any notified body, class \
society or authority; it does not state that any product or operator meets the document.";

/// Every catalogue, by id.
pub const CATALOGUES: &[(&str, &str)] = &[
    (
        "imo-msc401-432",
        include_str!("../../report-templates/regpacks/imo-msc401-432.json"),
    ),
    (
        "imo-circ-1575",
        include_str!("../../report-templates/regpacks/imo-circ-1575.json"),
    ),
    (
        "bam-pnt-alerts",
        include_str!("../../report-templates/regpacks/bam-pnt-alerts.json"),
    ),
    (
        "iacs-ur-e27",
        include_str!("../../report-templates/regpacks/iacs-ur-e27.json"),
    ),
    (
        "etsi-en-303-413",
        include_str!("../../report-templates/regpacks/etsi-en-303-413.json"),
    ),
    (
        "imo-mass-code",
        include_str!("../../report-templates/regpacks/imo-mass-code.json"),
    ),
    (
        "nist-csf2-pnt",
        include_str!("../../report-templates/regpacks/nist-csf2-pnt.json"),
    ),
    (
        "eu-space-act-cyber",
        include_str!("../../report-templates/regpacks/eu-space-act-cyber.json"),
    ),
];

/// The evidence formats a row may name: format tag and product name.
pub const EVIDENCE_FORMATS: &[(&str, &str)] = &[
    (STANDARDS_REPORT, "Open-engine standards-pack report"),
    ("kshana-testkit-build/1", "Test-kit build verdicts"),
    ("kshana-testkit-verdict/1", "Test-kit scenario verdict"),
    ("kshana-pics-scorecard/1", "PICS interference scorecard"),
    (
        "kshana-pro-receiver-scorecard/1",
        "Receiver resilience scorecard",
    ),
    ("kshana-pro-timing-scorecard/1", "Timing scorecard"),
    ("kshana-pro-timing-audit/1", "Timing audit report"),
    ("kshana-pro-trust-evidence/1", "Multi-source trust evidence"),
    ("kshana-pro-forensic-report/1", "Incident forensics report"),
    (
        "kshana-pro-maritime-evidence-pack/1",
        "Maritime incident evidence pack",
    ),
    (
        "kshana-pro-interference-episodes/1",
        "Interference episodes",
    ),
    (
        "kshana-pro-report-verification/1",
        "Interference report verification",
    ),
    ("kshana-pro-ais-integrity/1", "AIS and position integrity"),
    (
        "kshana-pro-pnt-verification/1",
        "PNT service verification study",
    ),
    ("kshana-pro-area-picture/1", "Area trust picture"),
    ("kshana-pro-exposure-summary/1", "Exposure summary"),
    (bam::FORMAT, "BAM alert mapping check"),
    (mass::FORMAT, "MASS override-independence check"),
    (links::FORMAT, "Link-security evidence"),
    (complaint::FORMAT, "Interference report pack"),
    (bid::FORMAT, "Bid evidence attestation"),
];

/// The product name of a format tag.
pub fn product_name(format: &str) -> Option<&'static str> {
    EVIDENCE_FORMATS.iter().find(|p| p.0 == format).map(|p| p.1)
}

/// A catalogue's source document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub title: String,
    pub version: String,
    pub url: String,
    /// What was read, and when; or that the text was not read.
    pub read: String,
}

/// One requirement row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    /// The clause, or exactly [`CLAUSE_UNCONFIRMED`].
    pub clause: String,
    /// Where the clause citation (or its absence) comes from.
    pub basis: String,
    /// Kshana's paraphrase of the requirement; never a quotation of more than a few words.
    pub topic: String,
    /// Evidence format tags that speak to the row; empty when Kshana has none.
    pub evidence: Vec<String>,
    pub gap: String,
}

/// A catalogue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub format: String,
    pub id: String,
    pub title: String,
    /// `type-approval`, `class`, `code`, `framework` or `regulation`.
    pub kind: String,
    pub status: String,
    pub source: Source,
    pub rows: Vec<Row>,
    pub wording: String,
}

impl Catalogue {
    /// Structural problems: format, unique row ids, known evidence formats, a basis for every
    /// cited clause, non-empty paraphrase and gap.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.format != CATALOGUE_FORMAT {
            p.push(format!("{}: format '{}'", self.id, self.format));
        }
        if self.rows.is_empty() {
            p.push(format!("{}: no rows", self.id));
        }
        let mut seen = BTreeSet::new();
        for r in &self.rows {
            if !seen.insert(r.id.as_str()) {
                p.push(format!("{}: duplicate row {}", self.id, r.id));
            }
            for f in &r.evidence {
                if product_name(f).is_none() {
                    p.push(format!("{} {}: unknown evidence format {f}", self.id, r.id));
                }
            }
            if r.clause.trim().is_empty() || r.basis.trim().is_empty() {
                p.push(format!(
                    "{} {}: clause and basis are required",
                    self.id, r.id
                ));
            }
            if r.topic.trim().is_empty() || r.gap.trim().is_empty() {
                p.push(format!("{} {}: topic and gap are required", self.id, r.id));
            }
        }
        p
    }
}

/// Every catalogue, parsed.
pub fn catalogues() -> Result<Vec<Catalogue>, String> {
    CATALOGUES
        .iter()
        .map(|(id, t)| serde_json::from_str(t).map_err(|e| format!("catalogue {id}: {e}")))
        .collect()
}

/// One catalogue by id.
pub fn catalogue(id: &str) -> Result<Catalogue, String> {
    catalogues()?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| {
            format!(
                "unknown catalogue '{id}'; known: {}",
                CATALOGUES
                    .iter()
                    .map(|t| t.0)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// SHA-256 of bytes, hex.
pub fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// `x` rounded to 9 significant digits (every float that feeds a hash goes through this).
pub fn sig9(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let d = 9 - 1 - x.abs().log10().floor() as i32;
    if !(-300..=300).contains(&d) {
        return x;
    }
    let m = 10f64.powi(d);
    (x * m).round() / m
}

/// HTML escape.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The page head shared by every page of this module.
pub fn page_head(title: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:2em;max-width:74em}}table{{border-collapse:collapse}}\
         td,th{{border:1px solid #bbb;padding:.25em .5em;text-align:left;vertical-align:top}}.l{{color:#555}}</style></head><body>",
        esc(title)
    )
}

/// Write `value` as pretty JSON to `dir/file`.
pub fn write_json<T: Serialize>(dir: &Path, file: &str, value: &T) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let p = dir.join(file);
    std::fs::write(
        &p,
        serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| format!("{}: {e}", p.display()))
}

// ---------------------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------------------

/// One evidence item as read.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvidenceItem {
    pub path: String,
    /// Every known format found in the directory's top-level JSON files.
    pub formats: Vec<String>,
    pub products: Vec<String>,
    pub signer: Option<String>,
    pub key_id: Option<String>,
    /// `signed`, or `unsigned open-engine report`.
    pub kind: String,
    pub signature_valid: bool,
    /// `true` pinned and trusted, `false` pinned and not trusted, `None` not pinned.
    pub signer_trusted: Option<bool>,
    /// SHA-256 of `signature.json` (signed) or of the report file (unsigned).
    pub sha256: String,
    pub problems: Vec<String>,
}

impl EvidenceItem {
    /// Whether the item may be put against a row.
    pub fn usable(&self) -> bool {
        self.signature_valid && self.signer_trusted != Some(false)
    }
}

fn formats_in(dir: &Path) -> Result<Vec<String>, String> {
    let mut found = BTreeSet::new();
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    files.sort();
    for p in files {
        if p.file_name().and_then(|n| n.to_str()) == Some(crate::signed_dossier::SIGNATURE_FILE) {
            continue;
        }
        let Ok(t) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) else {
            continue;
        };
        for key in ["format", "schema"] {
            if let Some(f) = v[key].as_str() {
                if product_name(f).is_some() {
                    found.insert(f.to_string());
                }
            }
        }
    }
    Ok(found.into_iter().collect())
}

/// Read and verify one signed evidence directory.
pub fn read_evidence(dir: &Path, trust: &[[u8; 32]]) -> Result<EvidenceItem, String> {
    let formats = formats_in(dir)?;
    if formats.is_empty() {
        return Err(format!(
            "{}: no Kshana evidence format found in its JSON files",
            dir.display()
        ));
    }
    let sig = std::fs::read(dir.join(crate::signed_dossier::SIGNATURE_FILE)).unwrap_or_default();
    let (signer, key_id, ok, trusted, problems) = match crate::signed_dossier::verify(dir, trust) {
        Ok(r) => (
            Some(r.signer.clone()),
            Some(r.key_id.clone()),
            r.signature_ok && r.problems.is_empty(),
            r.trusted,
            r.problems,
        ),
        Err(e) => (None, None, false, None, vec![e.to_string()]),
    };
    Ok(EvidenceItem {
        path: dir.display().to_string(),
        products: formats
            .iter()
            .map(|f| product_name(f).unwrap_or("").to_string())
            .collect(),
        formats,
        signer,
        key_id,
        kind: "signed".into(),
        signature_valid: ok,
        signer_trusted: trusted,
        sha256: sha256_hex(&sig),
        problems,
    })
}

/// Read a report of the open engine's standards-pack runner (`kshana standards-pack run
/// <id> --json`). It carries no signature: it is listed as unsigned with its hash, and the
/// pre-assessment that cites it is signed.
pub fn read_standards_report(path: &Path) -> Result<EvidenceItem, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    if v["schema"].as_str() != Some(STANDARDS_REPORT) {
        return Err(format!(
            "{}: not a standards-pack report (schema {STANDARDS_REPORT})",
            path.display()
        ));
    }
    let mut problems = Vec::new();
    if v["evidence_tier"].as_str() != Some("MODELLED") {
        problems.push("report is not labelled MODELLED".to_string());
    }
    Ok(EvidenceItem {
        path: format!(
            "{} (pack {}, engine {})",
            path.display(),
            v["pack"].as_str().unwrap_or("?"),
            v["engine_version"].as_str().unwrap_or("?")
        ),
        formats: vec![STANDARDS_REPORT.into()],
        products: vec![product_name(STANDARDS_REPORT).unwrap_or("").into()],
        signer: None,
        key_id: None,
        kind: "unsigned open-engine report".into(),
        signature_valid: problems.is_empty(),
        signer_trusted: None,
        sha256: sha256_hex(&bytes),
        problems,
    })
}

/// Paths of a comma-separated list.
pub fn paths(list: &str) -> Vec<PathBuf> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

// ---------------------------------------------------------------------------------------
// Pre-assessment
// ---------------------------------------------------------------------------------------

/// The equipment or system under pre-assessment, as the submitter declares it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    #[serde(default)]
    pub maker: String,
    #[serde(default)]
    pub product: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub notes: String,
}

impl Subject {
    pub fn parse(src: &str) -> Result<Subject, String> {
        toml::from_str(src).map_err(|e| format!("subject: {e}"))
    }
}

/// Row status: evidence supplied.
pub const SUPPLIED: &str = "evidence supplied";
/// Row status: Kshana has evidence for the row but none was supplied.
pub const NOT_SUPPLIED: &str = "no evidence supplied";
/// Row status: Kshana has no evidence product for the row.
pub const NOT_ASSESSED: &str = "not assessed by Kshana";

/// One row of a pre-assessment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssessedRow {
    pub id: String,
    pub clause: String,
    pub basis: String,
    pub topic: String,
    pub status: &'static str,
    pub evidence: Vec<String>,
    pub missing: Vec<String>,
    pub gap: String,
}

/// A pre-assessment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PreAssessment {
    pub format: &'static str,
    pub label: &'static str,
    pub catalogue: String,
    pub catalogue_sha256: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub source: Source,
    pub wording: String,
    pub subject: Subject,
    pub evidence: Vec<EvidenceItem>,
    pub rows: Vec<AssessedRow>,
    /// Rows with evidence supplied, with no evidence supplied, not assessed by Kshana.
    pub counts: [usize; 3],
}

/// The catalogue text of `id` (for its hash).
fn catalogue_text(id: &str) -> &'static str {
    CATALOGUES
        .iter()
        .find(|c| c.0 == id)
        .map(|c| c.1)
        .unwrap_or("")
}

/// Build a pre-assessment. Only usable evidence (valid signature, signer not refused) is put
/// against a row; the rest is listed with its problems.
pub fn assess(c: &Catalogue, subject: Subject, evidence: Vec<EvidenceItem>) -> PreAssessment {
    let mut supplied: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for e in evidence.iter().filter(|e| e.usable()) {
        for (f, p) in e.formats.iter().zip(&e.products) {
            supplied
                .entry(f.as_str())
                .or_default()
                .push(format!("{p} ({})", e.path));
        }
    }
    let mut counts = [0usize; 3];
    let rows = c
        .rows
        .iter()
        .map(|r| {
            let ev: Vec<String> = r
                .evidence
                .iter()
                .flat_map(|f| supplied.get(f.as_str()).cloned().unwrap_or_default())
                .collect();
            let missing: Vec<String> = r
                .evidence
                .iter()
                .filter(|f| !supplied.contains_key(f.as_str()))
                .map(|f| product_name(f).unwrap_or(f).to_string())
                .collect();
            let status = if r.evidence.is_empty() {
                counts[2] += 1;
                NOT_ASSESSED
            } else if ev.is_empty() {
                counts[1] += 1;
                NOT_SUPPLIED
            } else {
                counts[0] += 1;
                SUPPLIED
            };
            AssessedRow {
                id: r.id.clone(),
                clause: r.clause.clone(),
                basis: r.basis.clone(),
                topic: r.topic.clone(),
                status,
                evidence: ev,
                missing,
                gap: r.gap.clone(),
            }
        })
        .collect();
    PreAssessment {
        format: FORMAT,
        label: LABEL,
        catalogue: c.id.clone(),
        catalogue_sha256: sha256_hex(catalogue_text(&c.id).as_bytes()),
        title: c.title.clone(),
        kind: c.kind.clone(),
        status: c.status.clone(),
        source: c.source.clone(),
        wording: c.wording.clone(),
        subject,
        evidence,
        rows,
        counts,
    }
}

/// The pre-assessment page, laid out as a test report: scope, subject, results per
/// requirement, evidence register, deviations (rows without evidence).
pub fn render_html(p: &PreAssessment) -> String {
    let mut h = page_head(&format!("Pre-assessment: {}", p.title));
    h.push_str(&format!(
        "<h1>Pre-assessment: {}</h1><p class=\"l\">{}</p><p class=\"l\">{}</p>\
         <h2>1. Scope</h2><p>Document: {} ({}), <a href=\"{}\">{}</a>. Status: {}. Read: {}</p>\
         <h2>2. Subject</h2><table><tr><th>Maker</th><td>{}</td></tr><tr><th>Product</th><td>{}</td></tr>\
         <tr><th>Version</th><td>{}</td></tr><tr><th>Notes</th><td>{}</td></tr></table>\
         <h2>3. Results per requirement</h2><p>{} rows with evidence supplied, {} with no evidence supplied, {} not assessed by Kshana.</p>\
         <table><tr><th>Row</th><th>Clause</th><th>Requirement (paraphrased)</th><th>Status</th><th>Evidence</th><th>Missing</th><th>Gap</th></tr>",
        esc(&p.title),
        esc(p.label),
        esc(&p.wording),
        esc(&p.source.title),
        esc(&p.source.version),
        esc(&p.source.url),
        esc(&p.source.url),
        esc(&p.status),
        esc(&p.source.read),
        esc(&p.subject.maker),
        esc(&p.subject.product),
        esc(&p.subject.version),
        esc(&p.subject.notes),
        p.counts[0],
        p.counts[1],
        p.counts[2],
    ));
    for r in &p.rows {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}<br><span class=\"l\">{}</span></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&r.id),
            esc(&r.clause),
            esc(&r.basis),
            esc(&r.topic),
            r.status,
            esc(&r.evidence.join("; ")),
            esc(&r.missing.join("; ")),
            esc(&r.gap)
        ));
    }
    h.push_str("</table><h2>4. Evidence register</h2><table><tr><th>Item</th><th>Products</th><th>Signer</th><th>Status</th><th>SHA-256</th><th>Problems</th></tr>");
    for e in &p.evidence {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
            esc(&e.path),
            esc(&e.products.join("; ")),
            esc(e.signer.as_deref().unwrap_or("")),
            match (e.kind.as_str(), e.signature_valid, e.signer_trusted) {
                ("signed", false, _) => "signature INVALID (not used)",
                ("signed", true, Some(false)) => "valid, signer NOT trusted (not used)",
                ("signed", true, Some(true)) => "valid, signer trusted",
                ("signed", true, None) => "valid, signer not pinned",
                (_, true, _) => "unsigned open-engine report (hash recorded)",
                _ => "unsigned report with problems (not used)",
            },
            esc(&e.sha256),
            esc(&e.problems.join("; "))
        ));
    }
    h.push_str("</table><h2>5. Gaps (rows without supplied evidence)</h2><ul>");
    for r in p.rows.iter().filter(|r| r.status != SUPPLIED) {
        h.push_str(&format!(
            "<li>{} ({}): {}. {}</li>",
            esc(&r.id),
            r.status,
            esc(&r.topic),
            esc(&r.gap)
        ));
    }
    h.push_str("</ul></body></html>\n");
    h
}

/// Write a pre-assessment directory: `pre-assessment.json` and `report.html`.
pub fn write_dir(out: &Path, p: &PreAssessment) -> Result<(), String> {
    write_json(out, RESULT_FILE, p)?;
    std::fs::write(out.join("report.html"), render_html(p)).map_err(|e| e.to_string())
}

/// Example inputs, by name: (name, file name, text).
pub const EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "subject",
        "subject.toml",
        include_str!("../../report-templates/regpacks/examples/subject.toml"),
    ),
    (
        "bam",
        "alert-map.toml",
        include_str!("../../report-templates/regpacks/examples/alert-map.toml"),
    ),
    (
        "mass",
        "mass.toml",
        include_str!("../../report-templates/regpacks/examples/mass.toml"),
    ),
    (
        "complaint",
        "complaint.toml",
        include_str!("../../report-templates/regpacks/examples/complaint.toml"),
    ),
    (
        "links",
        "links.toml",
        include_str!("../../report-templates/regpacks/examples/links.toml"),
    ),
    (
        "bid",
        "bid.toml",
        include_str!("../../report-templates/regpacks/examples/bid.toml"),
    ),
    (
        "bom",
        "bom.csv",
        include_str!("../../report-templates/regpacks/examples/bom.csv"),
    ),
];

/// One example by name.
pub fn example(name: &str) -> Option<&'static str> {
    EXAMPLES.iter().find(|e| e.0 == name).map(|e| e.2)
}
