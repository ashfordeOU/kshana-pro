// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Timing evidence packs for regulation (W1119 C4; research 02 gaps 15, 16, 22; play P7).
//!
//! One signed annual pack per framework, each a table of **requirement → evidence →
//! verdict**:
//!
//! * `mifid2-rts25`: Commission Delegated Regulation (EU) 2017/574 (MiFID II RTS 25):
//!   traceability to UTC, maximum divergence by activity class (the Annex limits as the open
//!   engine transcribes and pins them, [`kshana::rts25`]), granularity, the yearly review;
//! * `dora-rts-2024-1774`: Commission Delegated Regulation (EU) 2024/1774 Article 12 (DORA
//!   ICT risk-management RTS): clocks synchronised to a documented, reliable reference time
//!   source;
//! * `eu-cer`: Directive (EU) 2022/2557 (critical entities resilience): the PNT/timing
//!   dependency as part of the entity's risk assessment and resilience measures;
//! * `india-ist-draft`: the draft Legal Metrology (Indian Standard Time) Rules, 2025:
//!   traceability to IST through NPL, no unapproved foreign time reference, spoofing.
//!
//! Inputs ([`Inputs`], a TOML file): the time-error record of the business or ICT clock
//! against the local reference (any `kshana::clock_log` format), BIPM Circular T section 1
//! for that reference's laboratory (`kshana::realdata::clk`), the result of an open-engine
//! `holdover-test` run, a spoof test (a Pro timing scorecard, T4, or a PICS scorecard, C1), a
//! timing fleet assessment (T1), and the operator's statements. Every input file is copied
//! into the pack and hashed, and the pack is signed with the dossier signature.
//!
//! **Verdicts** are about the evidence: `supported by the evidence`, `not supported by the
//! evidence`, `insufficient evidence`, `operator statement (not verified)` or `to be completed
//! by the operator`. A pack never says an organisation or a system meets a regulation; that
//! judgement is the operator's, its auditor's and its authority's (the T3 wording rule,
//! [`super::audit::FORBIDDEN_WORDS`]).
//!
//! **Sources and limits.** Each requirement cites its source. A limit is either transcribed
//! (RTS 25 Annex, through `kshana::rts25`; ITU-T budgets, through the open engine's
//! `holdover-test`), or the operator's own, or marked "to confirm with the regulation text".
//! DORA and CER state no numerical timing limit in the research behind this pack; the
//! article numbers of CER and every provision of the India rules (a draft as found) are to
//! confirm with the regulation text.
//!
//! **Evidence label: MODELLED.** Checked by internal tests on synthetic inputs; no operator's
//! records have been packed. The arithmetic (divergence plus the Circular T term) is exact;
//! the mapping of evidence to provisions is Kshana's reading, to be reviewed by the operator.

use std::path::{Path, PathBuf};

use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The pack's format tag.
pub const FORMAT: &str = "kshana-pro-timing-evidence-pack/1";
/// The licence feature (the timing audit export's).
pub const FEATURE: &str = super::AUDIT_FEATURE;
/// The text every unconfirmed limit carries.
pub const TO_CONFIRM: &str = "to confirm with the regulation text";
pub const LABEL: &str = "MODELLED: evidence mapped to regulatory requirements by Kshana's \
reading, from the operator's records as supplied; arithmetic checked by internal tests on \
synthetic inputs. Not a legal opinion and not a finding that any requirement is met.";
pub const WORDING: &str = "Each requirement lists the evidence supplied for it and a verdict \
about that evidence. The pack prepares an audit; whether the requirement is met is for the \
operator, its auditor and its authority to judge.";

/// The frameworks: id, title, source citation.
pub const FRAMEWORKS: [(&str, &str, &str); 4] = [
    (
        "mifid2-rts25",
        "MiFID II RTS 25: business clock synchronisation",
        "Commission Delegated Regulation (EU) 2017/574, OJ L 87, 31.3.2017 (Articles 1 to 4 and the Annex)",
    ),
    (
        "dora-rts-2024-1774",
        "DORA ICT risk-management RTS, Article 12: clock synchronisation",
        "Commission Delegated Regulation (EU) 2024/1774, Article 12 (research 02 source T7)",
    ),
    (
        "eu-cer",
        "EU Critical Entities Resilience Directive: PNT and timing dependency",
        "Directive (EU) 2022/2557 (CER); designations by 17 Jul 2026, obligations about 10 months later (research 02 source T9); article numbers to confirm with the regulation text",
    ),
    (
        "india-ist-draft",
        "India draft Legal Metrology (Indian Standard Time) Rules, 2025",
        "Draft Legal Metrology (Indian Standard Time) Rules, 2025, Department of Consumer Affairs (research 02 source N14); a draft as found, every provision to confirm with the final text",
    ),
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockIn {
    /// What the record is measured against, e.g. `UTC(NPL)`.
    pub reference: String,
    pub source: kshana::clock_log::ClockLogInput,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CircularTIn {
    pub path: String,
    /// The laboratory's Circular T code (`NPL`, `PTB`, `NPLI`, ...).
    pub lab: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIn {
    pub path: String,
}

/// What the operator states. Every field optional; a missing statement is shown as to be
/// completed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    pub utc_source: Option<String>,
    pub traceability_chain: Option<String>,
    /// Date of the last review of the traceability system, `YYYY-MM-DD`.
    pub review_date: Option<String>,
    /// RTS 25 activity (`kshana::rts25::ACTIVITY_NAMES`).
    pub activity: Option<String>,
    /// RTS 25: the timestamp granularity of the operator's records, s.
    pub timestamp_granularity_s: Option<f64>,
    /// DORA: the documented reference time source.
    pub reference_time_source: Option<String>,
    /// DORA: the operator's synchronisation tolerance, s.
    pub sync_tolerance_s: Option<f64>,
    /// The clock inventory (fleet file) covers every in-scope clock.
    pub inventory_complete: Option<bool>,
    /// CER: where the PNT/timing dependency register is kept.
    pub pnt_dependency_register: Option<String>,
    /// India: the primary time reference (e.g. `NPL time service`, `NavIC`, `GPS`).
    pub primary_reference: Option<String>,
    /// India: the approval of a foreign reference, when one is used.
    pub foreign_reference_approval: Option<String>,
    pub navic_receiver: Option<String>,
    pub npl_traceability: Option<String>,
}

/// The pack's inputs (paths relative to the inputs file).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub organisation: Option<String>,
    /// The period the pack covers, e.g. `2026`.
    pub period: Option<String>,
    pub clock: Option<ClockIn>,
    pub circular_t: Option<CircularTIn>,
    /// An open-engine `holdover-test` result JSON.
    pub holdover: Option<FileIn>,
    /// A Pro timing scorecard (`kshana-pro-timing-scorecard/1`) or a PICS scorecard.
    pub spoof: Option<FileIn>,
    /// A timing fleet assessment JSON (`kshana-pro-timing-fleet/1`).
    pub fleet: Option<FileIn>,
    #[serde(default)]
    pub operator: Operator,
}

impl Inputs {
    pub fn parse(src: &str) -> Result<Inputs, String> {
        toml::from_str(src).map_err(|e| format!("evidence pack inputs: {e}"))
    }
}

/// The divergence record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Divergence {
    pub reference: String,
    pub format: String,
    pub samples: usize,
    pub span_s: f64,
    pub max_abs_offset_s: f64,
    pub record_sha256: String,
    #[serde(skip)]
    time_s: Vec<f64>,
    #[serde(skip)]
    offset_s: Vec<f64>,
}

/// The Circular T term.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CircularT {
    pub issue: Option<u32>,
    pub lab: String,
    /// `None` when the laboratory is not in section 1.
    pub values: usize,
    pub max_abs_utc_minus_utck_ns: Option<f64>,
    /// Combined standard uncertainty u, ns.
    pub u_ns: Option<f64>,
    pub mjd_first: Option<i64>,
    pub mjd_last: Option<i64>,
}

impl CircularT {
    /// Bound on |UTC − UTC(k)| over the issue: max |value| + 2u (k = 2), s.
    pub fn bound_s(&self) -> Option<f64> {
        Some((self.max_abs_utc_minus_utck_ns? + 2.0 * self.u_ns?) * 1e-9)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Budget {
    pub name: String,
    pub max_abs_te_ns: f64,
    pub exceeded: bool,
    pub time_to_exceed_s: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Holdover {
    pub envelope_verdict: String,
    pub recovery_verdict: String,
    pub holdover_duration_s: Option<f64>,
    pub budgets: Vec<Budget>,
}

impl Holdover {
    /// The record held every time-error budget through the holdover.
    pub fn held(&self) -> bool {
        !self.budgets.is_empty() && self.budgets.iter().all(|b| !b.exceeded)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Spoof {
    pub format: String,
    pub cases: usize,
    pub detected: usize,
    pub highest_level: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fleet {
    pub clocks: usize,
    pub breached: usize,
    pub act: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputFile {
    pub role: String,
    pub file: String,
    pub sha256: String,
}

/// Everything read from the inputs.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Evidence {
    pub divergence: Option<Divergence>,
    pub circular_t: Option<CircularT>,
    pub holdover: Option<Holdover>,
    pub spoof: Option<Spoof>,
    pub fleet: Option<Fleet>,
    pub files: Vec<InputFile>,
    /// Input problems (a file that could not be read, a laboratory not in Circular T).
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Supported,
    NotSupported,
    Insufficient,
    OperatorStatement,
    ToComplete,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Supported => "supported by the evidence",
            Verdict::NotSupported => "not supported by the evidence",
            Verdict::Insufficient => "insufficient evidence",
            Verdict::OperatorStatement => "operator statement (not verified)",
            Verdict::ToComplete => "to be completed by the operator",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Requirement {
    pub id: String,
    /// The provision, as cited.
    pub source: String,
    /// Kshana's paraphrase of what it asks.
    pub requirement: String,
    /// The limit applied, if any.
    pub limit: Option<String>,
    /// Where the limit comes from, or [`TO_CONFIRM`].
    pub limit_basis: String,
    /// The evidence used, in words with numbers.
    pub evidence: Vec<String>,
    pub verdict: Verdict,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Counts {
    pub supported: usize,
    pub not_supported: usize,
    pub insufficient: usize,
    pub operator_statement: usize,
    pub to_complete: usize,
}

/// The pack document (`evidence-pack.json`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pack {
    pub format: &'static str,
    pub label: &'static str,
    pub wording: &'static str,
    pub framework: String,
    pub title: String,
    pub source: String,
    pub utc: String,
    pub organisation: Option<String>,
    pub period: Option<String>,
    pub requirements: Vec<Requirement>,
    pub counts: Counts,
    pub evidence: Evidence,
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn read_rel(base: &Path, p: &str) -> Result<(PathBuf, Vec<u8>), String> {
    let path = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        base.join(p)
    };
    let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, b))
}

fn json_f(v: &serde_json::Value, ptr: &str) -> Option<f64> {
    v.pointer(ptr).and_then(|x| x.as_f64())
}

fn json_s(v: &serde_json::Value, ptr: &str) -> Option<String> {
    v.pointer(ptr).and_then(|x| x.as_str()).map(str::to_string)
}

/// Read every input named in `inp` (paths relative to `base`).
pub fn gather(inp: &Inputs, base: &Path) -> Evidence {
    let mut ev = Evidence::default();
    let file = |role: &str, p: &str, ev: &mut Evidence| -> Option<Vec<u8>> {
        match read_rel(base, p) {
            Ok((path, b)) => {
                ev.files.push(InputFile {
                    role: role.into(),
                    file: path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    sha256: sha(&b),
                });
                Some(b)
            }
            Err(e) => {
                ev.problems.push(format!("{role}: {e}"));
                None
            }
        }
    };
    if let Some(c) = &inp.clock {
        let mut src = c.source.clone();
        if let Some(p) = &src.path {
            if let Some(b) = file("clock record", p, &mut ev) {
                src.path = None;
                src.text = Some(String::from_utf8_lossy(&b).to_string());
            }
        }
        if src.gaps.is_none() {
            src.gaps = Some("longest".into());
        }
        if src.text.is_some() {
            match kshana::clock_log::read_input(&src) {
                Ok(r) => {
                    let max = r.phase_s.iter().fold(0.0f64, |a, x| a.max(x.abs()));
                    ev.divergence = Some(Divergence {
                        reference: c.reference.clone(),
                        format: c.source.format.clone(),
                        samples: r.phase_s.len(),
                        span_s: r.time_s.last().copied().unwrap_or(0.0),
                        max_abs_offset_s: max,
                        record_sha256: r.phase_sha256(),
                        time_s: r.time_s.clone(),
                        offset_s: r.phase_s.clone(),
                    });
                }
                Err(e) => ev.problems.push(format!("clock record: {e}")),
            }
        }
    }
    if let Some(c) = &inp.circular_t {
        if let Some(b) = file("Circular T", &c.path, &mut ev) {
            match kshana::realdata::clk::parse_circular_t_section1(&String::from_utf8_lossy(&b)) {
                Ok(s) => {
                    let series = s.series(&c.lab);
                    let row = s.rows.iter().find(|r| r.code == c.lab);
                    if row.is_none() {
                        ev.problems.push(format!(
                            "Circular T: laboratory {} is not in section 1",
                            c.lab
                        ));
                    }
                    ev.circular_t = Some(CircularT {
                        issue: s.issue,
                        lab: c.lab.clone(),
                        values: series.len(),
                        max_abs_utc_minus_utck_ns: series
                            .iter()
                            .map(|(_, v)| v.abs())
                            .max_by(f64::total_cmp),
                        u_ns: row.and_then(|r| r.u),
                        mjd_first: series.first().map(|x| x.0),
                        mjd_last: series.last().map(|x| x.0),
                    });
                }
                Err(e) => ev.problems.push(format!("Circular T: {e}")),
            }
        }
    }
    if let Some(h) = &inp.holdover {
        if let Some(b) = file("holdover test", &h.path, &mut ev) {
            match serde_json::from_slice::<serde_json::Value>(&b) {
                Ok(v) if json_s(&v, "/kind").as_deref() == Some("holdover-test") => {
                    let budgets = v
                        .get("budgets")
                        .and_then(|x| x.as_array())
                        .map(|a| {
                            a.iter()
                                .map(|b| Budget {
                                    name: json_s(b, "/name").unwrap_or_default(),
                                    max_abs_te_ns: json_f(b, "/max_abs_te_ns").unwrap_or(f64::NAN),
                                    exceeded: b
                                        .get("exceeded")
                                        .and_then(|x| x.as_bool())
                                        .unwrap_or(true),
                                    time_to_exceed_s: json_f(b, "/time_to_exceed_s"),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    ev.holdover = Some(Holdover {
                        envelope_verdict: json_s(&v, "/verdicts/holdover_envelope")
                            .unwrap_or_else(|| "NOT-EVALUATED".into()),
                        recovery_verdict: v
                            .pointer("/verdicts/recovery")
                            .map(|x| {
                                x.as_str()
                                    .map(str::to_string)
                                    .unwrap_or_else(|| x.to_string())
                            })
                            .unwrap_or_else(|| "NOT-EVALUATED".into()),
                        holdover_duration_s: json_f(&v, "/holdover/duration_s"),
                        budgets,
                    });
                }
                Ok(_) => ev
                    .problems
                    .push("holdover test: not a holdover-test result".into()),
                Err(e) => ev.problems.push(format!("holdover test: {e}")),
            }
        }
    }
    if let Some(s) = &inp.spoof {
        if let Some(b) = file("spoof test", &s.path, &mut ev) {
            match serde_json::from_slice::<serde_json::Value>(&b) {
                Ok(v) => match json_s(&v, "/format").as_deref() {
                    Some(f @ "kshana-pro-timing-scorecard/1") => {
                        let rs = v
                            .get("results")
                            .and_then(|x| x.as_array())
                            .cloned()
                            .unwrap_or_default();
                        ev.spoof = Some(Spoof {
                            format: f.into(),
                            cases: rs.len(),
                            detected: rs
                                .iter()
                                .filter(|r| r.get("detection_s").is_some_and(|d| d.is_number()))
                                .count(),
                            highest_level: v
                                .pointer("/levels/highest_supported")
                                .and_then(|x| x.as_u64()),
                        });
                    }
                    Some(f @ "kshana-pics-scorecard/1") => {
                        let cs: Vec<serde_json::Value> = v
                            .get("cases")
                            .and_then(|x| x.as_array())
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|c| json_s(c, "/class").as_deref() == Some("spoofing"))
                            .collect();
                        ev.spoof = Some(Spoof {
                            format: f.into(),
                            cases: cs.len(),
                            detected: cs
                                .iter()
                                .filter(|c| {
                                    json_s(c, "/detection/verdict").as_deref() == Some("met")
                                })
                                .count(),
                            highest_level: v
                                .pointer("/levels/highest_supported")
                                .and_then(|x| x.as_u64()),
                        });
                    }
                    other => ev.problems.push(format!(
                        "spoof test: format {other:?} (expected a timing or PICS scorecard)"
                    )),
                },
                Err(e) => ev.problems.push(format!("spoof test: {e}")),
            }
        }
    }
    if let Some(f) = &inp.fleet {
        if let Some(b) = file("fleet assessment", &f.path, &mut ev) {
            match serde_json::from_slice::<serde_json::Value>(&b) {
                Ok(v)
                    if json_s(&v, "/format").as_deref()
                        == Some(super::fleet::ASSESSMENT_FORMAT) =>
                {
                    let n = |k: &str| {
                        v.pointer(&format!("/counts/{k}"))
                            .and_then(|x| x.as_u64())
                            .unwrap_or(0) as usize
                    };
                    ev.fleet = Some(Fleet {
                        clocks: n("clocks"),
                        breached: n("breached"),
                        act: n("act"),
                    });
                }
                Ok(_) => ev
                    .problems
                    .push("fleet assessment: not a timing fleet assessment".into()),
                Err(e) => ev.problems.push(format!("fleet assessment: {e}")),
            }
        }
    }
    ev
}

fn t(s: f64) -> String {
    super::fleet::human_time(s)
}

struct Ctx<'a> {
    inp: &'a Inputs,
    ev: &'a Evidence,
    utc: &'a str,
}

#[allow(clippy::too_many_arguments)]
fn req(
    id: &str,
    source: &str,
    requirement: &str,
    limit: Option<String>,
    limit_basis: &str,
    evidence: Vec<String>,
    verdict: Verdict,
    reason: impl Into<String>,
) -> Requirement {
    Requirement {
        id: id.into(),
        source: source.into(),
        requirement: requirement.into(),
        limit,
        limit_basis: limit_basis.into(),
        evidence,
        verdict,
        reason: reason.into(),
    }
}

impl Ctx<'_> {
    fn op(&self) -> &Operator {
        &self.inp.operator
    }

    fn circ_line(&self) -> Option<String> {
        self.ev.circular_t.as_ref().map(|c| {
            format!(
                "Circular T {}: UTC - UTC({}) max |{}| ns over {} values (MJD {} to {}), u = {} ns",
                c.issue.map(|i| i.to_string()).unwrap_or_else(|| "?".into()),
                c.lab,
                c.max_abs_utc_minus_utck_ns
                    .map(|x| format!("{x:.1}"))
                    .unwrap_or_else(|| "n/a".into()),
                c.values,
                c.mjd_first.map(|x| x.to_string()).unwrap_or_default(),
                c.mjd_last.map(|x| x.to_string()).unwrap_or_default(),
                c.u_ns
                    .map(|x| format!("{x:.1}"))
                    .unwrap_or_else(|| "not given".into())
            )
        })
    }

    fn div_line(&self) -> Option<String> {
        self.ev.divergence.as_ref().map(|d| {
            format!(
                "clock record ({}, {} samples over {}): max |offset from {}| = {}",
                d.format,
                d.samples,
                super::fleet::human_duration(Some(d.span_s)),
                d.reference,
                t(d.max_abs_offset_s)
            )
        })
    }

    fn hold_line(&self) -> Option<String> {
        self.ev.holdover.as_ref().map(|h| {
            format!(
                "holdover test: envelope {}, recovery {}, holdover {}; budgets: {}",
                h.envelope_verdict,
                h.recovery_verdict,
                h.holdover_duration_s
                    .map(|d| super::fleet::human_duration(Some(d)))
                    .unwrap_or_else(|| "n/a".into()),
                h.budgets
                    .iter()
                    .map(|b| format!(
                        "{} ({} ns) {}",
                        b.name,
                        b.max_abs_te_ns,
                        if b.exceeded { "exceeded" } else { "held" }
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        })
    }

    fn spoof_line(&self) -> Option<String> {
        self.ev.spoof.as_ref().map(|s| {
            format!(
                "spoof test ({}): {} of {} spoofing cases detected; highest DHS RPCF level supported: {}",
                s.format,
                s.detected,
                s.cases,
                s.highest_level.map(|l| l.to_string()).unwrap_or_else(|| "n/a".into())
            )
        })
    }

    fn traceability(
        &self,
        id: &str,
        source: &str,
        requirement: &str,
        stated: Option<&String>,
        what: &str,
    ) -> Requirement {
        let mut evidence: Vec<String> = Vec::new();
        if let Some(s) = stated {
            evidence.push(format!("operator: {what}: {s}"));
        }
        evidence.extend(self.circ_line());
        let lab_ok = self.ev.circular_t.as_ref().is_some_and(|c| c.values > 0);
        let (v, r) = match (stated.is_some(), lab_ok) {
            (true, true) => (
                Verdict::Supported,
                "the stated reference's laboratory publishes UTC - UTC(k) in Circular T, so the chain to UTC is documented to the laboratory",
            ),
            (true, false) => (
                Verdict::OperatorStatement,
                "stated by the operator; no Circular T entry for the laboratory was supplied",
            ),
            (false, true) => (
                Verdict::Insufficient,
                "the laboratory is in Circular T, but the operator has not stated its source and chain",
            ),
            (false, false) => (Verdict::ToComplete, "no statement and no Circular T entry"),
        };
        req(
            id,
            source,
            requirement,
            None,
            "documentary requirement; no numerical limit",
            evidence,
            v,
            r,
        )
    }

    fn resilience(&self, id: &str, source: &str, requirement: &str, basis: &str) -> Requirement {
        let mut evidence: Vec<String> = Vec::new();
        evidence.extend(self.hold_line());
        evidence.extend(self.spoof_line());
        let hold = self.ev.holdover.as_ref();
        let spoof = self.ev.spoof.as_ref();
        let (v, r) = match (hold, spoof) {
            (None, None) => (
                Verdict::Insufficient,
                "no holdover test and no spoof test supplied".to_string(),
            ),
            (Some(h), _) if !h.held() => (
                Verdict::NotSupported,
                "the holdover record exceeded a time-error budget".to_string(),
            ),
            (_, Some(s)) if s.cases == 0 || s.detected < s.cases => (
                Verdict::NotSupported,
                format!(
                    "{} of {} spoofing cases were not detected",
                    s.cases - s.detected,
                    s.cases
                ),
            ),
            (Some(_), Some(_)) => (
                Verdict::Supported,
                "the reference held its budgets without GNSS and every spoofing case was detected"
                    .to_string(),
            ),
            (None, Some(_)) => (
                Verdict::Insufficient,
                "spoof test only; no holdover test".to_string(),
            ),
            (Some(_), None) => (
                Verdict::Insufficient,
                "holdover test only; no spoof test".to_string(),
            ),
        };
        req(id, source, requirement, None, basis, evidence, v, r)
    }

    fn review(&self, id: &str, source: &str) -> Requirement {
        let op = self.op();
        let mut evidence = Vec::new();
        if let Some(c) = &op.traceability_chain {
            evidence.push(format!("operator: traceability system: {c}"));
        }
        if let Some(d) = &op.review_date {
            evidence.push(format!("operator: last review {d}"));
        }
        let age = op
            .review_date
            .as_deref()
            .and_then(|d| days_between(d, self.utc));
        let (v, r) = match (&op.traceability_chain, age) {
            (None, _) | (_, None) => (Verdict::ToComplete, "the traceability system description or its review date is missing".to_string()),
            (Some(_), Some(a)) if !(0..=366).contains(&a) => (
                Verdict::NotSupported,
                format!("the last review is {a} days before the pack date"),
            ),
            (Some(_), Some(a)) => (
                Verdict::OperatorStatement,
                format!("reviewed {a} days before the pack date (within a year); the review itself is the operator's"),
            ),
        };
        req(
            id,
            source,
            "Establish a system of traceability to UTC, document its design, functioning and specification, and review it at least once a year.",
            Some("review at least yearly".into()),
            "Article 4 of the Regulation, as in the Pro timing-audit template (report-templates/timing/rts25-traceability.json)",
            evidence,
            v,
            r,
        )
    }

    fn mifid(&self) -> Result<Vec<Requirement>, String> {
        let op = self.op();
        let act_name = op.activity.as_deref().unwrap_or("high-frequency");
        let act = kshana::rts25::Activity::from_name(act_name).ok_or_else(|| {
            format!(
                "unknown RTS 25 activity '{act_name}' (expected one of {})",
                kshana::rts25::ACTIVITY_NAMES.join(", ")
            )
        })?;
        let (limit, gran) = act.limits_s();
        let annex =
            "Annex, Tables 1 and 2, as transcribed and pinned by the open engine (kshana::rts25)";
        let mut out = vec![self.traceability(
            "RTS25-1",
            "Commission Delegated Regulation (EU) 2017/574, Article 1",
            "Synchronise business clocks to UTC as issued by a timing centre listed in the BIPM Annual Report on Time Activities, or disseminated by satellite with any offset accounted for.",
            op.utc_source.as_ref(),
            "UTC source",
        )];
        // Divergence.
        let mut evidence: Vec<String> = Vec::new();
        evidence.extend(self.div_line());
        evidence.extend(self.circ_line());
        let (v, r) = match &self.ev.divergence {
            None => (
                Verdict::Insufficient,
                "no clock record supplied".to_string(),
            ),
            Some(d) => {
                let rep = kshana::rts25::assess(
                    act,
                    &d.time_s,
                    &d.offset_s,
                    3.0 * (d.span_s / (d.samples.max(2) - 1) as f64),
                );
                evidence.push(format!(
                    "kshana::rts25 against the {act_name} limit: {} ({} samples beyond, {} gaps)",
                    rep.outcome,
                    rep.samples_beyond,
                    rep.gaps.len()
                ));
                let ct = self.ev.circular_t.as_ref().and_then(CircularT::bound_s);
                match (rep.outcome, ct) {
                    ("beyond limit", _) => (Verdict::NotSupported, "the record exceeds the limit against the local reference alone".to_string()),
                    ("not assessable", _) => (Verdict::Insufficient, "the record has gaps longer than three sample intervals".to_string()),
                    (_, None) => (
                        Verdict::Insufficient,
                        "within the limit against the local reference, but no Circular T term (with its uncertainty) bounds the reference against UTC".to_string(),
                    ),
                    (_, Some(c)) => {
                        let total = d.max_abs_offset_s + c;
                        evidence.push(format!(
                            "bound on |clock - UTC| = max |offset| + max |UTC - UTC(k)| + 2u = {} (limit {})",
                            t(total),
                            t(limit)
                        ));
                        if total <= limit {
                            (Verdict::Supported, format!("bound {} within the {} limit", t(total), t(limit)))
                        } else {
                            (Verdict::NotSupported, format!("bound {} beyond the {} limit", t(total), t(limit)))
                        }
                    }
                }
            }
        };
        out.push(req(
            "RTS25-2",
            "Commission Delegated Regulation (EU) 2017/574, Articles 2 and 3 and the Annex",
            "Keep the maximum divergence of business clocks from UTC within the level for the trading activity.",
            Some(format!("{} ({act_name})", t(limit))),
            annex,
            evidence,
            v,
            r,
        ));
        // Granularity.
        let (v, r, ev) = match op.timestamp_granularity_s {
            None => (Verdict::ToComplete, "the operator has not stated its timestamp granularity".to_string(), vec![]),
            Some(g) if g <= gran => (
                Verdict::OperatorStatement,
                format!("stated granularity {} within {}; a property of the operator's records, not measured here", t(g), t(gran)),
                vec![format!("operator: timestamp granularity {}", t(g))],
            ),
            Some(g) => (
                Verdict::NotSupported,
                format!("stated granularity {} coarser than {}", t(g), t(gran)),
                vec![format!("operator: timestamp granularity {}", t(g))],
            ),
        };
        out.push(req(
            "RTS25-3",
            "Commission Delegated Regulation (EU) 2017/574, Articles 2 and 3 and the Annex",
            "Timestamp granularity at least as fine as the level for the trading activity.",
            Some(format!("{} ({act_name})", t(gran))),
            annex,
            ev,
            v,
            r,
        ));
        out.push(self.review(
            "RTS25-4",
            "Commission Delegated Regulation (EU) 2017/574, Article 4",
        ));
        out.push(self.resilience(
            "RTS25-5",
            "Supporting evidence for Article 4 (traceability kept when GNSS is lost or spoofed)",
            "Traceability to UTC maintained through a loss or spoofing of GNSS; the research summaries say GPS alone is not sufficient evidence of traceability.",
            &format!("not a stated limit of the Regulation; {TO_CONFIRM} (research 02 sources T5, T6)"),
        ));
        Ok(out)
    }

    fn dora(&self) -> Vec<Requirement> {
        let op = self.op();
        let src = "Commission Delegated Regulation (EU) 2024/1774, Article 12";
        let mut out = vec![self.traceability(
            "DORA12-1",
            src,
            "Synchronise the clocks of ICT systems to a documented, reliable reference time source (research paraphrase: logging integrity).",
            op.reference_time_source.as_ref(),
            "reference time source",
        )];
        let mut evidence: Vec<String> = Vec::new();
        evidence.extend(self.div_line());
        if let Some(f) = &self.ev.fleet {
            evidence.push(format!(
                "fleet assessment: {} clocks, {} breached, {} act",
                f.clocks, f.breached, f.act
            ));
        }
        let (v, r) = match (op.sync_tolerance_s, &self.ev.divergence) {
            (None, _) => (
                Verdict::Insufficient,
                "no synchronisation tolerance stated by the operator".to_string(),
            ),
            (_, None) => (
                Verdict::Insufficient,
                "no clock record supplied".to_string(),
            ),
            (Some(tol), Some(d)) => {
                let breached = self.ev.fleet.as_ref().is_some_and(|f| f.breached > 0);
                if d.max_abs_offset_s <= tol && !breached {
                    (
                        Verdict::Supported,
                        format!(
                            "max |offset| {} within the operator's tolerance {}",
                            t(d.max_abs_offset_s),
                            t(tol)
                        ),
                    )
                } else if breached {
                    (
                        Verdict::NotSupported,
                        "the fleet assessment shows a clock beyond a limit".to_string(),
                    )
                } else {
                    (
                        Verdict::NotSupported,
                        format!(
                            "max |offset| {} beyond the operator's tolerance {}",
                            t(d.max_abs_offset_s),
                            t(tol)
                        ),
                    )
                }
            }
        };
        out.push(req(
            "DORA12-2",
            src,
            "The ICT clocks are kept synchronised to that source.",
            op.sync_tolerance_s.map(|x| format!("{} (the operator's tolerance)", t(x))),
            &format!("the Article states no numerical tolerance in the research behind this pack; {TO_CONFIRM}"),
            evidence,
            v,
            r,
        ));
        out.push(self.resilience(
            "DORA12-3",
            src,
            "The reference time source is reliable: it keeps time when GNSS is lost and rejects a spoofed GNSS time.",
            &format!("Kshana's reading of \"reliable\"; {TO_CONFIRM}"),
        ));
        let mut evidence: Vec<String> = Vec::new();
        if let Some(f) = &self.ev.fleet {
            evidence.push(format!("fleet assessment lists {} clocks", f.clocks));
        }
        if let Some(c) = op.inventory_complete {
            evidence.push(format!("operator: inventory complete = {c}"));
        }
        let (v, r) = match (&self.ev.fleet, op.inventory_complete) {
            (Some(_), Some(true)) => (
                Verdict::OperatorStatement,
                "the fleet lists the clocks; completeness is the operator's statement",
            ),
            (Some(_), Some(false)) => (
                Verdict::NotSupported,
                "the operator states the inventory is incomplete",
            ),
            (Some(_), None) => (
                Verdict::ToComplete,
                "the operator has not stated that the inventory is complete",
            ),
            (None, _) => (Verdict::Insufficient, "no fleet assessment supplied"),
        };
        out.push(req(
            "DORA12-4",
            src,
            "All ICT system clocks in scope are covered.",
            None,
            "documentary requirement; no numerical limit",
            evidence,
            v,
            r,
        ));
        out
    }

    fn cer(&self) -> Vec<Requirement> {
        let op = self.op();
        let src = "Directive (EU) 2022/2557 (CER), risk assessment and resilience measures of critical entities (article numbers to confirm with the regulation text)";
        let mut evidence: Vec<String> = Vec::new();
        if let Some(r) = &op.pnt_dependency_register {
            evidence.push(format!("operator: PNT/timing dependency register: {r}"));
        }
        if let Some(f) = &self.ev.fleet {
            evidence.push(format!(
                "fleet assessment: {} timing-dependent clocks inventoried",
                f.clocks
            ));
        }
        let (v, r) = match (&op.pnt_dependency_register, &self.ev.fleet) {
            (Some(_), Some(_)) => (
                Verdict::Supported,
                "a dependency register and an inventory of the timing-dependent clocks",
            ),
            (Some(_), None) => (
                Verdict::OperatorStatement,
                "a register is stated; no inventory supplied",
            ),
            (None, Some(_)) => (
                Verdict::Insufficient,
                "an inventory, but no dependency register stated",
            ),
            (None, None) => (Verdict::ToComplete, "no register and no inventory"),
        };
        let mut out = vec![req(
            "CER-1",
            src,
            "Identify the entity's dependency on GNSS-derived position, navigation and timing as part of its risk assessment.",
            None,
            &format!("no named GNSS rule found in the research (research 02 source T9); {TO_CONFIRM}"),
            evidence,
            v,
            r,
        )];
        let mut evidence: Vec<String> = Vec::new();
        evidence.extend(self.hold_line());
        let (v, r) = match &self.ev.holdover {
            None => (
                Verdict::Insufficient,
                "no holdover test supplied".to_string(),
            ),
            Some(h) => (
                Verdict::Supported,
                format!(
                    "the impact of a GNSS loss is measured: {}",
                    if h.held() {
                        "the record held every budget".to_string()
                    } else {
                        let first = h
                            .budgets
                            .iter()
                            .filter(|b| b.exceeded)
                            .filter_map(|b| b.time_to_exceed_s)
                            .min_by(f64::total_cmp);
                        format!(
                            "a budget was exceeded after {}",
                            first
                                .map(|d| super::fleet::human_duration(Some(d)))
                                .unwrap_or_else(|| "an unstated time".into())
                        )
                    }
                ),
            ),
        };
        out.push(req(
            "CER-2",
            src,
            "Assess the impact of a loss of GNSS timing on the essential service.",
            None,
            &format!("no numerical limit in the Directive per the research; {TO_CONFIRM}"),
            evidence,
            v,
            r,
        ));
        out.push(self.resilience(
            "CER-3",
            src,
            "Take resilience measures against the loss and the spoofing of GNSS timing.",
            &format!("Kshana's reading of the resilience-measure duty for a timing dependency; {TO_CONFIRM}"),
        ));
        out
    }

    fn india(&self) -> Vec<Requirement> {
        let op = self.op();
        let src = "Draft Legal Metrology (Indian Standard Time) Rules, 2025";
        let mut out = vec![self.traceability(
            "IST-1",
            src,
            "Traceability to Indian Standard Time as maintained by CSIR-NPL, the custodian named in the draft.",
            op.npl_traceability.as_ref(),
            "NPL traceability",
        )];
        if let Some(c) = &self.ev.circular_t {
            if c.lab != "NPLI" {
                out[0].evidence.push(format!(
                    "note: the Circular T laboratory given is {}, not NPLI (CSIR-NPL's code)",
                    c.lab
                ));
                if out[0].verdict == Verdict::Supported {
                    out[0].verdict = Verdict::Insufficient;
                    out[0].reason = "the Circular T entry is not NPL India's".into();
                }
            }
        }
        out[0].limit_basis =
            format!("accuracy figures of the draft not in the research; {TO_CONFIRM}");
        let mut evidence = Vec::new();
        if let Some(p) = &op.primary_reference {
            evidence.push(format!("operator: primary reference: {p}"));
        }
        if let Some(a) = &op.foreign_reference_approval {
            evidence.push(format!("operator: approval of a foreign reference: {a}"));
        }
        let (v, r) = match &op.primary_reference {
            None => (Verdict::ToComplete, "no primary reference stated"),
            Some(p) => {
                let l = p.to_ascii_lowercase();
                let foreign = ["gps", "galileo", "glonass", "beidou", "gnss"]
                    .iter()
                    .any(|k| l.contains(k))
                    && !l.contains("navic");
                match (foreign, &op.foreign_reference_approval) {
                    (true, None) => (
                        Verdict::NotSupported,
                        "a foreign GNSS reference with no approval stated",
                    ),
                    (true, Some(_)) => (
                        Verdict::OperatorStatement,
                        "a foreign reference with an approval stated by the operator",
                    ),
                    (false, _) => (
                        Verdict::OperatorStatement,
                        "a domestic reference stated by the operator",
                    ),
                }
            }
        };
        out.push(req(
            "IST-2",
            src,
            "Do not use a foreign time reference (for example GPS) as the source of time unless it is approved.",
            None,
            &format!("draft provision as summarised in the research (source N14); {TO_CONFIRM}"),
            evidence,
            v,
            r,
        ));
        let (v, r, evidence) = match &op.navic_receiver {
            Some(n) => (
                Verdict::OperatorStatement,
                "stated by the operator",
                vec![format!("operator: NavIC receiver: {n}")],
            ),
            None => (Verdict::ToComplete, "no NavIC capability stated", vec![]),
        };
        out.push(req(
            "IST-3",
            src,
            "A domestic satellite time source (NavIC) or NPL dissemination is available to the timing system.",
            None,
            &format!("Kshana's reading of the draft's domestic-reference intent; {TO_CONFIRM}"),
            evidence,
            v,
            r,
        ));
        out.push(self.resilience(
            "IST-4",
            src,
            "Protect time dissemination against spoofing (the draft has spoofing provisions) and keep time on loss of the reference.",
            &format!("draft provision as summarised in the research (source N14); {TO_CONFIRM}"),
        ));
        out
    }
}

/// Days from `date` (`YYYY-MM-DD`) to the date of `utc` (ISO 8601).
pub fn days_between(date: &str, utc: &str) -> Option<i64> {
    let p = |s: &str| -> Option<i64> {
        let d: Vec<i64> = s
            .get(..10)?
            .split('-')
            .map(|x| x.parse().ok())
            .collect::<Option<_>>()?;
        (d.len() == 3).then(|| kshana_eloran::time::days_from_civil(d[0], d[1], d[2]))
    };
    Some(p(utc)? - p(date)?)
}

/// Build a pack (no files written).
pub fn build(framework: &str, inp: &Inputs, base: &Path, utc: &str) -> Result<Pack, KshanaError> {
    let (id, title, source) = FRAMEWORKS
        .iter()
        .find(|f| f.0 == framework)
        .ok_or_else(|| {
            KshanaError::InvalidInput(format!(
                "unknown framework '{framework}' (expected {})",
                FRAMEWORKS.map(|f| f.0).join(", ")
            ))
        })?;
    let ev = gather(inp, base);
    let ctx = Ctx { inp, ev: &ev, utc };
    let requirements = match *id {
        "mifid2-rts25" => ctx.mifid().map_err(KshanaError::InvalidInput)?,
        "dora-rts-2024-1774" => ctx.dora(),
        "eu-cer" => ctx.cer(),
        _ => ctx.india(),
    };
    let c = |v: Verdict| requirements.iter().filter(|r| r.verdict == v).count();
    let counts = Counts {
        supported: c(Verdict::Supported),
        not_supported: c(Verdict::NotSupported),
        insufficient: c(Verdict::Insufficient),
        operator_statement: c(Verdict::OperatorStatement),
        to_complete: c(Verdict::ToComplete),
    };
    Ok(Pack {
        format: FORMAT,
        label: LABEL,
        wording: WORDING,
        framework: id.to_string(),
        title: format!("Timing evidence pack: {title}"),
        source: source.to_string(),
        utc: utc.into(),
        organisation: inp.organisation.clone(),
        period: inp.period.clone(),
        requirements,
        counts,
        evidence: ev,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn requirements_csv(p: &Pack) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut o = String::from("id,source,requirement,limit,limit_basis,evidence,verdict,reason\n");
    for r in &p.requirements {
        o.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            r.id,
            q(&r.source),
            q(&r.requirement),
            q(r.limit.as_deref().unwrap_or("")),
            q(&r.limit_basis),
            q(&r.evidence.join(" | ")),
            q(r.verdict.as_str()),
            q(&r.reason)
        ));
    }
    o
}

pub fn html(p: &Pack) -> String {
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:64rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:.35rem;text-align:left;vertical-align:top}}small{{color:#555}}</style></head><body>\
<h1>{}</h1><p>{}</p><p>{} · {} · pack date {}</p><p>{}</p><p>{}</p>",
        esc(&p.title),
        esc(&p.title),
        esc(&p.source),
        esc(p.organisation.as_deref().unwrap_or("(organisation not stated)")),
        esc(p.period.as_deref().unwrap_or("(period not stated)")),
        esc(&p.utc),
        esc(p.label),
        esc(p.wording)
    );
    o.push_str("<table><tr><th>Requirement</th><th>Evidence</th><th>Verdict</th></tr>");
    for r in &p.requirements {
        o.push_str(&format!(
            "<tr><td><strong>{}</strong> {}<br><small>{}</small><br><small>Limit: {} ({})</small></td><td>{}</td><td><strong>{}</strong><br><small>{}</small></td></tr>",
            esc(&r.id),
            esc(&r.requirement),
            esc(&r.source),
            esc(r.limit.as_deref().unwrap_or("none")),
            esc(&r.limit_basis),
            r.evidence.iter().map(|e| esc(e)).collect::<Vec<_>>().join("<br>"),
            r.verdict.as_str(),
            esc(&r.reason)
        ));
    }
    o.push_str("</table><h2>Input files</h2><ul>");
    for f in &p.evidence.files {
        o.push_str(&format!(
            "<li>{}: {} <code>{}</code></li>",
            esc(&f.role),
            esc(&f.file),
            &f.sha256
        ));
    }
    o.push_str("</ul>");
    if !p.evidence.problems.is_empty() {
        o.push_str("<h2>Input problems</h2><ul>");
        for x in &p.evidence.problems {
            o.push_str(&format!("<li>{}</li>", esc(x)));
        }
        o.push_str("</ul>");
    }
    o.push_str("</body></html>\n");
    o
}

/// Write the pack directory: `evidence-pack.json`, `evidence-pack.html`, `requirements.csv`,
/// the inputs file and every input file under `inputs/`. The caller signs it.
pub fn write_dir(
    out: &Path,
    p: &Pack,
    inputs_src: &str,
    inp: &Inputs,
    base: &Path,
) -> Result<(), KshanaError> {
    let io = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", out.display()));
    std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
    std::fs::write(
        out.join("evidence-pack.json"),
        serde_json::to_string_pretty(p).expect("pack serialises"),
    )
    .map_err(io)?;
    std::fs::write(out.join("evidence-pack.html"), html(p)).map_err(io)?;
    std::fs::write(out.join("requirements.csv"), requirements_csv(p)).map_err(io)?;
    std::fs::write(out.join("inputs").join("inputs.toml"), inputs_src).map_err(io)?;
    let mut paths: Vec<&str> = Vec::new();
    if let Some(c) = &inp.clock {
        paths.extend(c.source.path.as_deref());
    }
    paths.extend(inp.circular_t.as_ref().map(|c| c.path.as_str()));
    paths.extend(inp.holdover.as_ref().map(|c| c.path.as_str()));
    paths.extend(inp.spoof.as_ref().map(|c| c.path.as_str()));
    paths.extend(inp.fleet.as_ref().map(|c| c.path.as_str()));
    for p in paths {
        if let Ok((path, b)) = read_rel(base, p) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name != "inputs.toml" {
                std::fs::write(out.join("inputs").join(name), b).map_err(io)?;
            }
        }
    }
    Ok(())
}
