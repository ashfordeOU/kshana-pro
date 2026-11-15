// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Framework evidence packs (roadmap 0.42 E6)
//!
//! One template per framework (`report-templates/frameworks/<id>.json`): EASA SIB 2022-02R4,
//! NIST IR 8323r1, the DHS Resilient PNT Conformance Framework, IMO Circular Letter No. 5037
//! (draft until its text is read), MiFID II RTS 25, the EU Space Act proposal and NIS 2. Each
//! lists the framework's duties (paraphrased, with the reference), the Kshana evidence products
//! **mapped to** each duty, and the gap Kshana does not cover. It extends the open engine's six
//! framework mappings (`kshana::compliance::mapping`) with the Pro evidence products.
//!
//! A pack takes the operator's signed Kshana Pro report directories, verifies each signature,
//! reads each one's format tag, and states per duty which supplied evidence is mapped to it,
//! which mapped products were not supplied, and the gap. Wording: "mapped to" only. A pack never
//! says a framework is met; that judgement is the operator's, its assessor's and its authority's.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The licence feature of `kshana-pro frameworks pack`.
pub const FEATURE: &str = "pro-framework-pack";
/// The pack's format tag.
pub const FORMAT: &str = "kshana-pro-framework-pack/1";
/// The file a pack writes.
pub const RESULT_FILE: &str = "framework-pack.json";

/// Every template, by id.
pub const TEMPLATES: &[(&str, &str)] = &[
    (
        "easa-sib-2022-02r4",
        include_str!("../../report-templates/frameworks/easa-sib-2022-02r4.json"),
    ),
    (
        "nist-ir-8323r1",
        include_str!("../../report-templates/frameworks/nist-ir-8323r1.json"),
    ),
    (
        "dhs-rpcf",
        include_str!("../../report-templates/frameworks/dhs-rpcf.json"),
    ),
    (
        "imo-cl-5037",
        include_str!("../../report-templates/frameworks/imo-cl-5037.json"),
    ),
    (
        "mifid2-rts25",
        include_str!("../../report-templates/frameworks/mifid2-rts25.json"),
    ),
    (
        "eu-space-act",
        include_str!("../../report-templates/frameworks/eu-space-act.json"),
    ),
    (
        "nis2",
        include_str!("../../report-templates/frameworks/nis2.json"),
    ),
];

/// The evidence products a duty can be mapped to: format tag, name, main file.
pub const PRODUCTS: &[(&str, &str, &str)] = &[
    (
        "kshana-pro-forensic-report/1",
        "Incident forensics report (E1)",
        "report.json",
    ),
    (
        "kshana-pro-receiver-scorecard/1",
        "Receiver resilience scorecard (E3)",
        "receiver-scorecard.json",
    ),
    (
        "kshana-pro-timing-scorecard/1",
        "Timing scorecard (T4)",
        "timing-scorecard.json",
    ),
    (
        "kshana-pro-timing-audit/1",
        "Timing audit report (T3)",
        "report.json",
    ),
    (
        "kshana-pro-trust-evidence/1",
        "Multi-source trust evidence (F3)",
        "trust-evidence.json",
    ),
    (
        "kshana-pro-ais-integrity/1",
        "AIS and position integrity scoring (E5)",
        "integrity-score.json",
    ),
    (
        "kshana-pro-pnt-verification/1",
        "PNT service verification study (E4)",
        "verification-study.json",
    ),
    (
        "kshana-pro-area-picture/1",
        "Area trust picture (F1)",
        "area-picture.json",
    ),
    (
        "kshana-pro-exposure-summary/1",
        "Insurer exposure summary (E7)",
        "exposure-summary.json",
    ),
];

/// A framework's source document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub title: String,
    pub version: String,
    pub url: String,
    pub checked: String,
}

/// One duty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Duty {
    pub id: String,
    pub reference: String,
    pub duty: String,
    pub mapped_to: Vec<String>,
    pub gap: String,
}

/// A framework template.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub format: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: Option<String>,
    pub source: Source,
    pub duties: Vec<Duty>,
    pub wording: String,
}

/// Every template, parsed.
pub fn templates() -> Result<Vec<Template>, String> {
    TEMPLATES
        .iter()
        .map(|(id, t)| serde_json::from_str(t).map_err(|e| format!("template {id}: {e}")))
        .collect()
}

/// One template by id.
pub fn template(id: &str) -> Result<Template, String> {
    templates()?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| {
            format!(
                "unknown framework '{id}'; known: {}",
                TEMPLATES.iter().map(|t| t.0).collect::<Vec<_>>().join(", ")
            )
        })
}

/// The product name of a format tag.
pub fn product_name(format: &str) -> Option<&'static str> {
    PRODUCTS.iter().find(|p| p.0 == format).map(|p| p.1)
}

/// One evidence directory as read.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvidenceItem {
    pub path: String,
    pub format: String,
    pub product: String,
    pub main_file: String,
    pub signer: Option<String>,
    pub signature_valid: bool,
    /// `true` pinned and trusted, `false` pinned and not trusted, `None` not pinned.
    pub signer_trusted: Option<bool>,
    pub problems: Vec<String>,
}

/// One duty in a pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DutyRow {
    pub id: String,
    pub reference: String,
    pub duty: String,
    /// `evidence mapped`, `no evidence supplied` or `not covered by Kshana`.
    pub status: &'static str,
    pub mapped_evidence: Vec<String>,
    pub mapped_products_not_supplied: Vec<String>,
    pub gap: String,
}

/// A framework evidence pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub framework: String,
    pub title: String,
    pub template_status: Option<String>,
    pub source: Source,
    pub wording: String,
    pub evidence: Vec<EvidenceItem>,
    pub duties: Vec<DutyRow>,
    pub counts: [usize; 3],
}

fn main_json(dir: &Path) -> Result<(String, serde_json::Value), String> {
    for (_, _, f) in PRODUCTS {
        let p = dir.join(f);
        if let Ok(t) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                if v["format"].as_str().and_then(product_name).is_some() {
                    return Ok((f.to_string(), v));
                }
            }
        }
    }
    Err(format!(
        "{}: no Kshana Pro evidence product found (looked for {})",
        dir.display(),
        PRODUCTS
            .iter()
            .map(|p| p.2)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Read and verify one evidence directory.
pub fn read_evidence(dir: &Path, trust: &[[u8; 32]]) -> Result<EvidenceItem, String> {
    let (main, v) = main_json(dir)?;
    let format = v["format"].as_str().unwrap_or("").to_string();
    let (signer, ok, trusted, problems) = match crate::signed_dossier::verify(dir, trust) {
        Ok(r) => {
            let ok = r.signature_ok && r.problems.is_empty();
            (Some(r.signer.clone()), ok, r.trusted, r.problems)
        }
        Err(e) => (None, false, None, vec![e.to_string()]),
    };
    Ok(EvidenceItem {
        path: dir.display().to_string(),
        product: product_name(&format).unwrap_or("").to_string(),
        format,
        main_file: main,
        signer,
        signature_valid: ok,
        signer_trusted: trusted,
        problems,
    })
}

/// Build a pack. Only evidence whose signature is valid (and, when keys are pinned, trusted)
/// is mapped; the rest is listed with its problems.
pub fn build(t: &Template, evidence: Vec<EvidenceItem>) -> Pack {
    let usable: Vec<&EvidenceItem> = evidence
        .iter()
        .filter(|e| e.signature_valid && e.signer_trusted != Some(false))
        .collect();
    let mut counts = [0usize; 3];
    let duties = t
        .duties
        .iter()
        .map(|d| {
            let mapped: Vec<String> = usable
                .iter()
                .filter(|e| d.mapped_to.contains(&e.format))
                .map(|e| format!("{} ({})", e.product, e.path))
                .collect();
            let supplied: BTreeSet<&str> = usable.iter().map(|e| e.format.as_str()).collect();
            let missing: Vec<String> = d
                .mapped_to
                .iter()
                .filter(|f| !supplied.contains(f.as_str()))
                .map(|f| product_name(f).unwrap_or(f).to_string())
                .collect();
            let status = if d.mapped_to.is_empty() {
                counts[2] += 1;
                "not covered by Kshana"
            } else if mapped.is_empty() {
                counts[1] += 1;
                "no evidence supplied"
            } else {
                counts[0] += 1;
                "evidence mapped"
            };
            DutyRow {
                id: d.id.clone(),
                reference: d.reference.clone(),
                duty: d.duty.clone(),
                status,
                mapped_evidence: mapped,
                mapped_products_not_supplied: missing,
                gap: d.gap.clone(),
            }
        })
        .collect();
    Pack {
        format: FORMAT,
        framework: t.id.clone(),
        title: t.title.clone(),
        template_status: t.status.clone(),
        source: t.source.clone(),
        wording: t.wording.clone(),
        evidence,
        duties,
        counts,
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The pack's page.
pub fn render_html(p: &Pack) -> String {
    let mut h = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:2em;max-width:72em}}table{{border-collapse:collapse}}\
         td,th{{border:1px solid #bbb;padding:.25em .5em;text-align:left;vertical-align:top}}.l{{color:#555}}</style></head><body>\
         <h1>Evidence mapped to {}</h1>{}<p class=\"l\">{}</p>\
         <p>Source: {} ({}), <a href=\"{}\">{}</a>. {}</p>\
         <p>{} duties with evidence mapped, {} with no evidence supplied, {} not covered by Kshana.</p>\
         <h2>Duties</h2><table><tr><th>Reference</th><th>Duty (paraphrased)</th><th>Status</th><th>Evidence mapped</th><th>Mapped products not supplied</th><th>Not provided by Kshana</th></tr>",
        esc(&p.title),
        esc(&p.title),
        p.template_status
            .as_ref()
            .map(|s| format!("<p><b>Template status: {}.</b></p>", esc(s)))
            .unwrap_or_default(),
        esc(&p.wording),
        esc(&p.source.title),
        esc(&p.source.version),
        esc(&p.source.url),
        esc(&p.source.url),
        esc(&p.source.checked),
        p.counts[0],
        p.counts[1],
        p.counts[2],
    );
    for d in &p.duties {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&d.reference),
            esc(&d.duty),
            d.status,
            esc(&d.mapped_evidence.join("; ")),
            esc(&d.mapped_products_not_supplied.join("; ")),
            esc(&d.gap)
        ));
    }
    h.push_str("</table><h2>Evidence supplied</h2><table><tr><th>Directory</th><th>Product</th><th>Signer</th><th>Signature</th><th>Problems</th></tr>");
    for e in &p.evidence {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&e.path),
            esc(&e.product),
            esc(e.signer.as_deref().unwrap_or("")),
            match (e.signature_valid, e.signer_trusted) {
                (false, _) => "INVALID (not mapped)",
                (true, Some(false)) => "valid, signer NOT trusted (not mapped)",
                (true, Some(true)) => "valid, signer trusted",
                (true, None) => "valid, signer not pinned",
            },
            esc(&e.problems.join("; "))
        ));
    }
    h.push_str("</table></body></html>\n");
    h
}

/// Write a pack directory: `framework-pack.json` and `report.html`.
pub fn write_dir(out: &Path, p: &Pack) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    std::fs::write(
        out.join(RESULT_FILE),
        serde_json::to_string_pretty(p).expect("plain data") + "\n",
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(out.join("report.html"), render_html(p)).map_err(|e| e.to_string())
}

/// Paths of a comma-separated list.
pub fn paths(list: &str) -> Vec<PathBuf> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}
