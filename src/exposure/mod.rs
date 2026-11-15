// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Insurer scorecard: periodic exposure summary for a fleet or route (roadmap 0.42 E7)
//!
//! For a period and an insured fleet or route, this module summarises the **signed evidence**
//! the insured or its monitoring provider supplies:
//!
//! * the incident forensics reports (E1) whose incident window overlaps the period: per incident
//!   the jamming and spoofing verdicts with their confidence, findings by category, the time
//!   under findings, and whether the report's signature verifies;
//! * one AIS and position integrity scoring (E5) for the period: the fleet's or route area's
//!   integrity score with its exact 95 % interval and the identities in each class.
//!
//! The summary is signed as a whole (`crate::signed_dossier`) and verifies for free. It adds no
//! detection of its own: every figure is copied or counted from the evidence, which keeps its own
//! validation status (E1: roadmap V2; E5: roadmap V3). It is an evidence summary for the
//! underwriter, not an underwriting recommendation, a rating, a premium advice or a finding of
//! fact.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The licence feature of `kshana-pro exposure run`.
pub const FEATURE: &str = "pro-insurer-scorecard";
/// The summary's format tag.
pub const FORMAT: &str = "kshana-pro-exposure-summary/1";
/// The file a summary writes.
pub const RESULT_FILE: &str = "exposure-summary.json";

/// What every summary says about itself.
pub const STATEMENT: &str = "A summary of the signed evidence supplied for the period, copied and \
counted without new detection. Each source keeps its own label and validation status. Not an \
underwriting recommendation, a rating, premium advice or a finding of fact; classes and findings \
describe the consistency of data, never intent.";

/// A summary request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExposureRequest {
    pub title: String,
    /// The insured fleet's or route's name as the policy names it.
    pub insured: String,
    /// `fleet` or `route`.
    pub subject: String,
    /// ISO-8601 UTC.
    pub period_start_utc: String,
    pub period_end_utc: String,
    /// Signed incident forensics report directories.
    #[serde(default)]
    pub forensics: Vec<String>,
    /// A signed AIS integrity scoring directory.
    #[serde(default)]
    pub integrity: Option<String>,
    /// The group of that scoring to report: a fleet name, an area name (a route), or `all`.
    #[serde(default)]
    pub integrity_group: Option<String>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl ExposureRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut s: Self =
            toml::from_str(&src).map_err(|e| format!("invalid exposure request: {e}"))?;
        s.base_dir = path.parent().map(Path::to_path_buf);
        Ok((s, src))
    }

    fn path(&self, p: &str) -> PathBuf {
        match &self.base_dir {
            Some(d) if Path::new(p).is_relative() => d.join(p),
            _ => PathBuf::from(p),
        }
    }
}

/// Signature state of one source directory.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SourceCheck {
    pub path: String,
    pub signer: Option<String>,
    pub signature_valid: bool,
    pub signer_trusted: Option<bool>,
    pub problems: Vec<String>,
}

/// One incident in the period.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IncidentRow {
    pub incident_id: String,
    pub title: String,
    pub subject_id: Option<String>,
    pub start_utc: String,
    pub end_utc: String,
    pub variant: String,
    pub status: String,
    pub jamming: String,
    pub jamming_confidence: Option<String>,
    pub spoofing: String,
    pub spoofing_confidence: Option<String>,
    pub findings: usize,
    pub findings_supported: usize,
    pub finding_categories: std::collections::BTreeMap<String, usize>,
    /// Sum of the findings' durations, s (findings may overlap).
    pub time_under_findings_s: f64,
    pub validation_status: String,
    pub source: SourceCheck,
}

/// The integrity part.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct IntegrityPart {
    pub group: String,
    pub vessels: usize,
    pub counts: serde_json::Value,
    pub integrity_score: Option<f64>,
    pub integrity_score_ci95: serde_json::Value,
    pub rf_fixes_no_matching_ais: Option<u64>,
    pub validation_status: String,
    pub source: SourceCheck,
}

/// Totals over the period.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Totals {
    pub incidents: usize,
    pub jamming_indicated: usize,
    pub spoofing_indicated: usize,
    pub high_confidence_findings: usize,
    pub time_under_findings_h: f64,
    /// Sources left out because their signature did not verify (or the signer is not trusted).
    pub sources_rejected: usize,
}

/// An exposure summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExposureSummary {
    pub format: &'static str,
    pub title: String,
    pub insured: String,
    pub subject: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub statement: &'static str,
    pub totals: Totals,
    pub incidents: Vec<IncidentRow>,
    pub integrity: Option<IntegrityPart>,
    pub rejected: Vec<SourceCheck>,
}

/// Verify the source directory `dir`; `named` is its path as the request names it, which is what
/// the summary records (never the resolved absolute path, so a summary is the same on every machine
/// and does not disclose local directories).
fn check(dir: &Path, named: &str, trust: &[[u8; 32]]) -> SourceCheck {
    match crate::signed_dossier::verify(dir, trust) {
        Ok(v) => SourceCheck {
            path: named.to_string(),
            signer: Some(v.signer.clone()),
            signature_valid: v.signature_ok && v.problems.is_empty(),
            signer_trusted: v.trusted,
            problems: v.problems.clone(),
        },
        Err(e) => SourceCheck {
            path: named.to_string(),
            signer: None,
            signature_valid: false,
            signer_trusted: None,
            problems: vec![e.to_string()],
        },
    }
}

fn usable(c: &SourceCheck) -> bool {
    c.signature_valid && c.signer_trusted != Some(false)
}

fn s(v: &serde_json::Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn so(v: &serde_json::Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// Build a summary. `trust`: the signer keys to pin (empty: signatures checked, signers not
/// pinned).
pub fn build(r: &ExposureRequest, trust: &[[u8; 32]]) -> Result<ExposureSummary, String> {
    if !matches!(r.subject.as_str(), "fleet" | "route") {
        return Err("subject must be fleet or route".into());
    }
    let p0 = crate::forensics::inputs::parse_utc(&r.period_start_utc)
        .ok_or("period_start_utc is not ISO-8601 UTC")?;
    let p1 = crate::forensics::inputs::parse_utc(&r.period_end_utc)
        .ok_or("period_end_utc is not ISO-8601 UTC")?;
    if p1 <= p0 {
        return Err("the period is empty".into());
    }
    let mut incidents = Vec::new();
    let mut rejected = Vec::new();
    for f in &r.forensics {
        let dir = r.path(f);
        let c = check(&dir, f, trust);
        let text = std::fs::read_to_string(dir.join("report.json"))
            .map_err(|e| format!("{}: {e}", dir.display()))?;
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))?;
        if v["format"] != crate::forensics::FORMAT {
            return Err(format!(
                "{}: not an incident forensics report",
                dir.display()
            ));
        }
        if !usable(&c) {
            rejected.push(c);
            continue;
        }
        let inc = &v["incident"];
        let (a, b) = (
            crate::forensics::inputs::parse_utc(inc["start_utc"].as_str().unwrap_or("")),
            crate::forensics::inputs::parse_utc(inc["end_utc"].as_str().unwrap_or("")),
        );
        let (Some(a), Some(b)) = (a, b) else {
            return Err(format!(
                "{}: the incident window is not readable",
                dir.display()
            ));
        };
        if b < p0 || a > p1 {
            continue;
        }
        let findings = v["findings"].as_array().cloned().unwrap_or_default();
        let mut cats = std::collections::BTreeMap::new();
        for x in &findings {
            *cats.entry(s(&x["category"])).or_insert(0) += 1;
        }
        incidents.push(IncidentRow {
            incident_id: s(&inc["id"]),
            title: s(&inc["title"]),
            subject_id: so(&inc["subject_id"]),
            start_utc: s(&inc["start_utc"]),
            end_utc: s(&inc["end_utc"]),
            variant: s(&v["variant"]),
            status: s(&v["status"]),
            jamming: s(&v["verdict"]["jamming"]["status"]),
            jamming_confidence: so(&v["verdict"]["jamming"]["confidence"]),
            spoofing: s(&v["verdict"]["spoofing"]["status"]),
            spoofing_confidence: so(&v["verdict"]["spoofing"]["confidence"]),
            findings: findings.len(),
            findings_supported: findings
                .iter()
                .filter(|x| x["supported"].as_bool() == Some(true))
                .count(),
            finding_categories: cats,
            time_under_findings_s: findings
                .iter()
                .filter_map(|x| x["duration_s"].as_f64())
                .sum(),
            validation_status: s(&v["validation_status"]),
            source: c,
        });
    }
    incidents.sort_by(|a, b| {
        a.start_utc
            .cmp(&b.start_utc)
            .then(a.incident_id.cmp(&b.incident_id))
    });
    let integrity = match &r.integrity {
        Some(d) => {
            let dir = r.path(d);
            let c = check(&dir, d, trust);
            let text = std::fs::read_to_string(dir.join(crate::integrity::RESULT_FILE))
                .map_err(|e| format!("{}: {e}", dir.display()))?;
            let v: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", dir.display()))?;
            if v["format"] != crate::integrity::FORMAT {
                return Err(format!("{}: not an AIS integrity scoring", dir.display()));
            }
            if usable(&c) {
                let want = r.integrity_group.clone().unwrap_or_else(|| "all".into());
                let g = v["groups"]
                    .as_array()
                    .and_then(|gs| {
                        gs.iter().find(|g| {
                            (want == "all" && g["group"] == "all") || g["name"] == want.as_str()
                        })
                    })
                    .ok_or_else(|| format!("the integrity scoring has no group '{want}'"))?;
                Some(IntegrityPart {
                    group: want,
                    vessels: g["vessels"].as_u64().unwrap_or(0) as usize,
                    counts: g["counts"].clone(),
                    integrity_score: g["integrity_score"].as_f64(),
                    integrity_score_ci95: g["integrity_score_ci95"].clone(),
                    rf_fixes_no_matching_ais: g["rf_fixes_no_matching_ais"].as_u64(),
                    validation_status: s(&v["validation_status"]),
                    source: c,
                })
            } else {
                rejected.push(c);
                None
            }
        }
        None => None,
    };
    let totals = Totals {
        incidents: incidents.len(),
        jamming_indicated: incidents
            .iter()
            .filter(|i| i.jamming == "indicated")
            .count(),
        spoofing_indicated: incidents
            .iter()
            .filter(|i| i.spoofing == "indicated")
            .count(),
        high_confidence_findings: incidents.iter().map(|i| i.findings_supported).sum(),
        time_under_findings_h: incidents
            .iter()
            .map(|i| i.time_under_findings_s)
            .sum::<f64>()
            / 3600.0,
        sources_rejected: rejected.len(),
    };
    Ok(ExposureSummary {
        format: FORMAT,
        title: r.title.clone(),
        insured: r.insured.clone(),
        subject: r.subject.clone(),
        period_start_utc: r.period_start_utc.clone(),
        period_end_utc: r.period_end_utc.clone(),
        statement: STATEMENT,
        totals,
        incidents,
        integrity,
        rejected,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The summary's page.
pub fn render_html(x: &ExposureSummary) -> String {
    let t = &x.totals;
    let mut h = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:2em;max-width:70em}}table{{border-collapse:collapse}}\
         td,th{{border:1px solid #bbb;padding:.25em .5em;text-align:left}}.l{{color:#555}}</style></head><body>\
         <h1>{}</h1><p><b>Insured {}:</b> {}. <b>Period:</b> {} to {}.</p><p class=\"l\">{}</p>\
         <h2>Summary</h2><ul><li>Incidents with signed forensic reports: {}</li><li>Jamming indicated: {}</li>\
         <li>Spoofing indicated: {}</li><li>Findings meeting the claim-grade bar (high confidence, corroborated): {}</li>\
         <li>Time under findings: {:.2} h (findings may overlap)</li><li>Sources rejected (signature): {}</li></ul>",
        esc(&x.title),
        esc(&x.title),
        esc(&x.subject),
        esc(&x.insured),
        esc(&x.period_start_utc),
        esc(&x.period_end_utc),
        esc(x.statement),
        t.incidents,
        t.jamming_indicated,
        t.spoofing_indicated,
        t.high_confidence_findings,
        t.time_under_findings_h,
        t.sources_rejected
    );
    if let Some(i) = &x.integrity {
        h.push_str(&format!(
            "<h2>AIS and position integrity ({})</h2><p>{} identities; integrity score {} (95 % interval {}); counts {}; RF fixes with no matching AIS: {}.</p><p class=\"l\">{}</p>",
            esc(&i.group),
            i.vessels,
            i.integrity_score.map(|s| format!("{s:.4}")).unwrap_or_else(|| "n/a".into()),
            esc(&i.integrity_score_ci95.to_string()),
            esc(&i.counts.to_string()),
            i.rf_fixes_no_matching_ais.map(|d| d.to_string()).unwrap_or_else(|| "n/a".into()),
            esc(&i.validation_status)
        ));
    }
    h.push_str("<h2>Incidents</h2><table><tr><th>Incident</th><th>Subject</th><th>Window (UTC)</th><th>Jamming</th><th>Spoofing</th><th>Findings</th><th>Time under findings</th><th>Report</th></tr>");
    for i in &x.incidents {
        h.push_str(&format!(
            "<tr><td>{} {}</td><td>{}</td><td>{} to {}</td><td>{}{}</td><td>{}{}</td><td>{} ({} claim-grade)</td><td>{:.0} s</td><td>{}, {}</td></tr>",
            esc(&i.incident_id),
            esc(&i.title),
            esc(i.subject_id.as_deref().unwrap_or("")),
            esc(&i.start_utc),
            esc(&i.end_utc),
            esc(&i.jamming),
            i.jamming_confidence.as_ref().map(|c| format!(" ({})", esc(c))).unwrap_or_default(),
            esc(&i.spoofing),
            i.spoofing_confidence.as_ref().map(|c| format!(" ({})", esc(c))).unwrap_or_default(),
            i.findings,
            i.findings_supported,
            i.time_under_findings_s,
            esc(&i.variant),
            esc(&i.status)
        ));
    }
    h.push_str("</table></body></html>\n");
    h
}

/// Write a summary directory: `exposure-summary.json` and `report.html`.
pub fn write_dir(out: &Path, x: &ExposureSummary, request_src: &str) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let w = |n: &str, b: &[u8]| std::fs::write(out.join(n), b).map_err(|e| format!("{n}: {e}"));
    w(
        RESULT_FILE,
        (serde_json::to_string_pretty(x).expect("plain data") + "\n").as_bytes(),
    )?;
    w("report.html", render_html(x).as_bytes())?;
    w("request.toml", request_src.as_bytes())
}
