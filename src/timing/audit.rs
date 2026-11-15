// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Timing audit export (roadmap 0.42 T3): a report directory that maps a fleet assessment and
//! its evidence history to a framework template (MiFID II RTS 25 traceability, or the NIST IR
//! 8323 PNT profile), then is signed as a whole with the dossier signature
//! ([`crate::signed_dossier`]). Building and signing need the `pro-timing-audit` feature;
//! verifying needs nothing (`kshana-pro dossier verify <dir> --trust <key>`, or
//! `scripts/verify-dossier-signature.py`), so the recipient checks it with the free edition.
//!
//! Wording rule: each item is **mapped to** a provision; the report never says a system is
//! compliant, conformant or certified.

use std::path::Path;

use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};

use super::evidence::{self, Entry};
use super::fleet::{human_duration, human_time, FleetAssessment, FleetSpec};

/// The templates this build carries.
pub const TEMPLATES: [(&str, &str); 2] = [
    (
        "rts25-traceability",
        include_str!("../../report-templates/timing/rts25-traceability.json"),
    ),
    (
        "nist-ir-8323-profile",
        include_str!("../../report-templates/timing/nist-ir-8323-profile.json"),
    ),
];

/// The format tag of a report.
pub const REPORT_FORMAT: &str = "kshana-pro-timing-audit/1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateItem {
    #[serde(rename = "ref")]
    pub reference: String,
    pub topic: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub operator: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Template {
    pub format: String,
    pub id: String,
    pub title: String,
    pub framework: String,
    pub wording: String,
    pub items: Vec<TemplateItem>,
}

/// The evidence keys a template may name.
pub const EVIDENCE_KEYS: [&str; 11] = [
    "fleet.inventory",
    "fleet.limits",
    "clock.records",
    "clock.divergence",
    "clock.holdover",
    "evidence.history",
    "report.signature",
    "operator.utc_source",
    "operator.traceability_chain",
    "operator.timestamp_point",
    "operator.review",
];

/// Words a report must never use about the systems it describes.
pub const FORBIDDEN_WORDS: [&str; 5] = [
    "compliant",
    "compliance",
    "conform",
    "conformant",
    "certified",
];

pub fn template(id: &str) -> Result<Template, KshanaError> {
    let (_, text) = TEMPLATES.iter().find(|(k, _)| *k == id).ok_or_else(|| {
        KshanaError::InvalidInput(format!(
            "unknown audit profile '{id}' (expected {})",
            TEMPLATES.map(|t| t.0).join(" or ")
        ))
    })?;
    let t: Template = serde_json::from_str(text)
        .map_err(|e| KshanaError::InvalidInput(format!("template {id}: {e}")))?;
    for it in &t.items {
        for k in &it.evidence {
            if !EVIDENCE_KEYS.contains(&k.as_str()) {
                return Err(KshanaError::InvalidInput(format!(
                    "template {id}: unknown evidence key '{k}'"
                )));
            }
        }
    }
    Ok(t)
}

/// What the operator states (each optional; a missing statement is shown as to be completed).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorStatement {
    #[serde(default)]
    pub organisation: Option<String>,
    #[serde(default)]
    pub utc_source: Option<String>,
    #[serde(default)]
    pub traceability_chain: Option<String>,
    #[serde(default)]
    pub timestamp_point: Option<String>,
    #[serde(default)]
    pub review: Option<String>,
    /// RTS 25 activity of the clocks (`kshana::rts25::ACTIVITY_NAMES`), default `high-frequency`.
    #[serde(default)]
    pub activity: Option<String>,
}

/// One evidence block of the report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EvidenceBlock {
    pub key: String,
    pub title: String,
    /// Column names and rows (a table), or a single row of one cell (a statement).
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReportItem {
    #[serde(rename = "ref")]
    pub reference: String,
    pub topic: String,
    pub summary: String,
    pub mapped_evidence: Vec<String>,
    pub operator_action: String,
}

/// The report document (`report.json`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub label: String,
    pub profile: String,
    pub title: String,
    pub framework: String,
    pub wording: String,
    pub utc: String,
    pub fleet: String,
    pub organisation: Option<String>,
    pub items: Vec<ReportItem>,
    pub evidence: Vec<EvidenceBlock>,
}

fn todo(v: &Option<String>) -> String {
    v.clone()
        .unwrap_or_else(|| "To be completed by the operator.".into())
}

fn block(key: &str, title: &str, cols: &[&str], rows: Vec<Vec<String>>) -> EvidenceBlock {
    EvidenceBlock {
        key: key.into(),
        title: title.into(),
        columns: cols.iter().map(|c| c.to_string()).collect(),
        rows,
    }
}

/// The RTS 25 divergence of each servo record against the activity's limit, through the open
/// engine's [`kshana::rts25::assess`]. A free-running record is against a reference, not UTC.
fn divergence(
    spec: &FleetSpec,
    a: &FleetAssessment,
    activity: kshana::rts25::Activity,
) -> Result<Vec<Vec<String>>, KshanaError> {
    let mut rows = Vec::new();
    for c in &spec.clocks {
        let Some(ca) = a.clock(&c.id) else { continue };
        if ca.record != "disciplined" {
            rows.push(vec![
                c.id.clone(),
                "free-running record against a reference".into(),
                "—".into(),
                "—".into(),
                "not assessed".into(),
            ]);
            continue;
        }
        let mut inp = c.source.clone();
        if inp.gaps.is_none() {
            inp.gaps = Some("longest".into());
        }
        let rec = kshana::clock_log::read_input(&inp)
            .map_err(|e| KshanaError::InvalidInput(format!("clock '{}': {e}", c.id)))?;
        let r = kshana::rts25::assess(activity, &rec.time_s, &rec.phase_s, 3.0 * rec.tau0_s);
        rows.push(vec![
            c.id.clone(),
            format!("offset as recorded by {}", c.source.format),
            human_time(r.max_abs_divergence_s),
            human_time(r.limit_s),
            r.outcome.to_string(),
        ]);
    }
    Ok(rows)
}

fn history_rows(history: &[(String, Vec<Entry>)]) -> Vec<Vec<String>> {
    history
        .iter()
        .map(|(id, h)| {
            let p = evidence::verify_chain(h);
            vec![
                id.clone(),
                h.len().to_string(),
                h.first()
                    .map(|e| e.utc.clone())
                    .unwrap_or_else(|| "—".into()),
                h.last()
                    .map(|e| e.utc.clone())
                    .unwrap_or_else(|| "—".into()),
                if h.is_empty() {
                    "no history".into()
                } else if p.is_empty() {
                    "chain intact".into()
                } else {
                    format!("BROKEN: {}", p.join("; "))
                },
            ]
        })
        .collect()
}

/// Build the report (no files written).
pub fn build(
    spec: &FleetSpec,
    a: &FleetAssessment,
    history: &[(String, Vec<Entry>)],
    profile: &str,
    op: &OperatorStatement,
    utc: &str,
) -> Result<Report, KshanaError> {
    let t = template(profile)?;
    let act_name = op.activity.as_deref().unwrap_or("high-frequency");
    let activity = kshana::rts25::Activity::from_name(act_name).ok_or_else(|| {
        KshanaError::InvalidInput(format!(
            "unknown activity '{act_name}' (expected one of {})",
            kshana::rts25::ACTIVITY_NAMES.join(", ")
        ))
    })?;
    let mut needed: Vec<&str> = Vec::new();
    for it in &t.items {
        for k in &it.evidence {
            if !needed.contains(&k.as_str()) {
                needed.push(k);
            }
        }
    }
    let mut ev = Vec::new();
    for k in needed {
        let b = match k {
            "fleet.inventory" => block(
                k,
                "Clock inventory",
                &["Clock", "Site", "Vendor", "Model", "Role", "Source"],
                a.clocks
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone(),
                            c.site.clone().unwrap_or_default(),
                            c.vendor.clone().unwrap_or_default(),
                            c.model.clone().unwrap_or_default(),
                            c.role.clone().unwrap_or_default(),
                            c.source_format.clone(),
                        ]
                    })
                    .collect(),
            ),
            "fleet.limits" => block(
                k,
                "Limits set by the operator",
                &["Limit", "Value"],
                a.limits
                    .iter()
                    .map(|l| vec![l.name.clone(), human_time(l.value_s)])
                    .collect(),
            ),
            "clock.records" => block(
                k,
                "Measured records",
                &["Clock", "Format", "Record", "Samples", "Interval", "Span", "SHA-256 of the samples"],
                a.clocks
                    .iter()
                    .map(|c| {
                        vec![
                            c.id.clone(),
                            c.source_format.clone(),
                            c.record.clone(),
                            c.n_used.to_string(),
                            format!("{} s", c.tau0_s),
                            human_duration(Some(c.span_s)),
                            c.record_sha256.clone(),
                        ]
                    })
                    .collect(),
            ),
            "clock.divergence" => block(
                k,
                &format!("Recorded divergence against the RTS 25 limit for activity '{act_name}'"),
                &["Clock", "What was recorded", "Largest divergence", "Limit", "Outcome"],
                divergence(spec, a, activity)?,
            ),
            "clock.holdover" => {
                let mut rows = Vec::new();
                for c in &a.clocks {
                    for b in &c.breaches {
                        rows.push(vec![
                            c.id.clone(),
                            b.name.clone(),
                            human_time(b.value_s),
                            human_duration(b.t_warn_s),
                            human_duration(b.t_50_s),
                            match c.noise_source {
                                super::model::NoiseSource::Measured => "measured".into(),
                                super::model::NoiseSource::Prior => {
                                    format!("prior ({})", c.prior.clone().unwrap_or_default())
                                }
                            },
                        ]);
                    }
                }
                block(
                    k,
                    &format!(
                        "Holdover time to breach (horizon {}; {})",
                        human_duration(Some(a.horizon_s)),
                        super::v5_status()
                    ),
                    &["Clock", "Limit", "Value", "Warning (5 %)", "Median (50 %)", "Noise"],
                    rows,
                )
            }
            "evidence.history" => block(
                k,
                "Per-clock evidence history",
                &["Clock", "Entries", "First", "Last", "Chain"],
                history_rows(history),
            ),
            "report.signature" => block(
                k,
                "Signature",
                &["Statement"],
                vec![vec![
                    "Every file of this report is listed with its SHA-256 in signature.json and signed with Ed25519; check it with `kshana-pro dossier verify <dir> --trust <public key>` (free, offline) or scripts/verify-dossier-signature.py.".into(),
                ]],
            ),
            "operator.utc_source" => block(k, "UTC source (operator)", &["Statement"], vec![vec![todo(&op.utc_source)]]),
            "operator.traceability_chain" => block(
                k,
                "Traceability chain (operator)",
                &["Statement"],
                vec![vec![todo(&op.traceability_chain)]],
            ),
            "operator.timestamp_point" => block(
                k,
                "Point where the timestamp is applied (operator)",
                &["Statement"],
                vec![vec![todo(&op.timestamp_point)]],
            ),
            "operator.review" => block(k, "Last yearly review (operator)", &["Statement"], vec![vec![todo(&op.review)]]),
            _ => unreachable!("template keys are checked on load"),
        };
        ev.push(b);
    }
    Ok(Report {
        format: REPORT_FORMAT,
        label: super::label(),
        profile: t.id.clone(),
        title: t.title.clone(),
        framework: t.framework.clone(),
        wording: t.wording.clone(),
        utc: utc.into(),
        fleet: a.fleet.clone(),
        organisation: op.organisation.clone(),
        items: t
            .items
            .iter()
            .map(|i| ReportItem {
                reference: i.reference.clone(),
                topic: i.topic.clone(),
                summary: i.summary.clone(),
                mapped_evidence: i.evidence.clone(),
                operator_action: i.operator.clone(),
            })
            .collect(),
        evidence: ev,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Report {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a report is plain data") + "\n"
    }

    pub fn to_html(&self) -> String {
        let title_of = |k: &str| {
            self.evidence
                .iter()
                .find(|b| b.key == k)
                .map(|b| b.title.clone())
                .unwrap_or_else(|| k.into())
        };
        let mut items = String::new();
        for i in &self.items {
            let ev: String = i
                .mapped_evidence
                .iter()
                .map(|k| format!("<li><a href=\"#{}\">{}</a></li>", esc(k), esc(&title_of(k))))
                .collect();
            items.push_str(&format!(
                "<tr><td><b>{}</b><br>{}</td><td>{}</td><td>mapped to:<ul>{ev}</ul></td><td>{}</td></tr>",
                esc(&i.reference),
                esc(&i.topic),
                esc(&i.summary),
                esc(&i.operator_action)
            ));
        }
        let mut blocks = String::new();
        for b in &self.evidence {
            let head: String = b
                .columns
                .iter()
                .map(|c| format!("<th>{}</th>", esc(c)))
                .collect();
            let rows: String = b
                .rows
                .iter()
                .map(|r| {
                    format!(
                        "<tr>{}</tr>",
                        r.iter()
                            .map(|c| format!("<td>{}</td>", esc(c)))
                            .collect::<String>()
                    )
                })
                .collect();
            blocks.push_str(&format!(
                "<section id=\"{}\"><h3>{}</h3><div class=\"wrap\"><table><thead><tr>{head}</tr></thead><tbody>{rows}</tbody></table></div></section>",
                esc(&b.key),
                esc(&b.title)
            ));
        }
        format!(
            r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Timing audit report</title>
<style>
:root {{ color-scheme: light dark; --bg:#f6f7f9; --surface:#fff; --ink:#14171c; --muted:#5b6472; --line:#dde1e7; --accent:#0b57a8; }}
@media (prefers-color-scheme: dark) {{ :root:not([data-theme="light"]) {{ --bg:#0f1216; --surface:#171b21; --ink:#e7eaee; --muted:#9aa3af; --line:#2a3039; --accent:#8cc4f5; }} }}
:root[data-theme="dark"] {{ --bg:#0f1216; --surface:#171b21; --ink:#e7eaee; --muted:#9aa3af; --line:#2a3039; --accent:#8cc4f5; }}
body {{ margin:0; background:var(--bg); color:var(--ink); font:15px/1.55 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }}
main {{ max-width:1100px; margin:0 auto; padding:24px 16px 48px; }}
h1 {{ font-size:22px; margin:0 0 4px; }} h2 {{ font-size:17px; margin:28px 0 8px; }} h3 {{ font-size:15px; margin:20px 0 6px; }}
.sub {{ color:var(--muted); }} a {{ color:var(--accent); }}
.box {{ background:var(--surface); border:1px solid var(--line); border-radius:8px; padding:12px 16px; }}
.wrap {{ background:var(--surface); border:1px solid var(--line); border-radius:8px; overflow-x:auto; }}
table {{ width:100%; border-collapse:collapse; min-width:640px; }}
th,td {{ text-align:left; padding:8px 10px; border-bottom:1px solid var(--line); vertical-align:top; font-size:14px; overflow-wrap:anywhere; }}
th {{ font-size:12px; text-transform:uppercase; letter-spacing:.04em; color:var(--muted); }}
ul {{ margin:4px 0 0 18px; padding:0; }}
</style></head><body><main>
<h1>{title}</h1>
<p class="sub">{framework}<br>Fleet: {fleet}{org} · generated {utc} · profile <code>{profile}</code></p>
<div class="box"><p>{wording}</p><p class="sub">{label}</p></div>
<h2>Mapping</h2>
<div class="wrap"><table><thead><tr><th>Provision</th><th>What it asks (summary)</th><th>Evidence in this report</th><th>Operator to add</th></tr></thead><tbody>{items}</tbody></table></div>
<h2>Evidence</h2>
{blocks}
</main></body></html>
"#,
            title = esc(&self.title),
            framework = esc(&self.framework),
            fleet = esc(&self.fleet),
            org = self
                .organisation
                .as_ref()
                .map(|o| format!(" · {}", esc(o)))
                .unwrap_or_default(),
            utc = esc(&self.utc),
            profile = esc(&self.profile),
            wording = esc(&self.wording),
            label = esc(&self.label),
        )
    }
}

/// Write the report directory: `report.json`, `report.html`, `fleet-assessment.json`,
/// `template.json`, `history/<clock>.jsonl` and `README.txt`. Signing is a separate step
/// ([`crate::signed_dossier::sign`]) so that the directory is complete before it is signed.
pub fn write_dir(
    out: &Path,
    report: &Report,
    a: &FleetAssessment,
    history: &[(String, Vec<Entry>)],
) -> Result<(), KshanaError> {
    let io = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", out.display()));
    if out.exists() && std::fs::read_dir(out).map_err(io)?.next().is_some() {
        return Err(KshanaError::InvalidInput(format!(
            "{}: the report directory must be new or empty",
            out.display()
        )));
    }
    std::fs::create_dir_all(out.join("history")).map_err(io)?;
    std::fs::write(out.join("report.json"), report.to_json()).map_err(io)?;
    std::fs::write(out.join("report.html"), report.to_html()).map_err(io)?;
    std::fs::write(out.join("fleet-assessment.json"), a.to_json() + "\n").map_err(io)?;
    let (_, t) = TEMPLATES
        .iter()
        .find(|(k, _)| *k == report.profile)
        .expect("the report's profile is a known template");
    std::fs::write(out.join("template.json"), t).map_err(io)?;
    for (id, h) in history {
        let text: String = h
            .iter()
            .map(|e| serde_json::to_string(e).expect("an entry is plain data") + "\n")
            .collect();
        std::fs::write(out.join("history").join(format!("{id}.jsonl")), text).map_err(io)?;
    }
    std::fs::write(
        out.join("README.txt"),
        format!(
            "Timing audit report ({profile}), fleet {fleet}, generated {utc}.\n\n\
             report.html            the report to read\n\
             report.json            the same, as data\n\
             fleet-assessment.json  the holdover assessment of every clock\n\
             history/               each clock's hash-chained evidence history\n\
             template.json          the mapping template used\n\
             signature.json         SHA-256 of every file above, signed with Ed25519\n\n\
             Check it, offline and without a licence:\n\
               kshana-pro dossier verify <this directory> --trust <the signer's public key>\n\
             or: python3 scripts/verify-dossier-signature.py <this directory> --trust <key>\n\n\
             Each item is mapped to a provision; the report is not a statement that any\n\
             system meets the framework.\n",
            profile = report.profile,
            fleet = report.fleet,
            utc = report.utc
        ),
    )
    .map_err(io)?;
    Ok(())
}
