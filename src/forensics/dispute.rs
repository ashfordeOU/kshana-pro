// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Commercial dispute evidence (research 2026-11-20 gaps, item B0606)
//!
//! Owners and charterers argue over GNSS-caused time: off-hire, delay against the laycan,
//! DP downtime on an offshore charter, a deviation, a master's refusal to sail into a
//! spoofing area. The question is the same each time: **when was position or timing integrity
//! lost, for how long, what does the evidence attribute it to, and which independent sources
//! agree?** This module assembles that from evidence Kshana Pro already produces and maps it to
//! the questions a clause raises:
//!
//! * the maritime actor-vs-victim pack ([`super::maritime`], W1119 C3), run from its request or
//!   read from a pack directory (signature checked);
//! * the incident forensics bundle ([`super`], docs/INCIDENT-FORENSICS.md), read from its
//!   directory (signature checked);
//! * the log time-integrity check ([`super::logtime`], B0603), run from its request: the time
//!   base the durations rest on, and timing-integrity intervals of their own;
//! * operational status logs (DP position references, DP status, operations), REPORTED;
//! * the claimed periods of each party, REPORTED;
//! * clause templates ([`TEMPLATES`]): off-hire, delay against the laycan, DP downtime,
//!   deviation and safe port, each a set of questions answered from the evidence with the
//!   pack and finding IDs cited.
//!
//! The report gives per-source intervals, the merged periods of lost integrity with inner and
//! outer durations from the onset and end bounds, which sources agree in each period, the
//! overlap of every claimed period, and the clause questions. The output directory is signed
//! with [`crate::signed_dossier`] (verify free with `kshana-pro dossier verify`).
//!
//! **Label: MODELLED evidence. Not a legal finding.** The clause templates are generic
//! engineering questions, not the wording or the construction of any contract; whether a
//! clause applies, and what follows, is for the parties, their lawyers and the tribunal.
//! Tested on synthetic scenarios with known truth only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::inputs::sha256_hex;
use super::logtime::{self, iso_ms, parse_time, sig9};
use super::maritime::{self, Conclusion, EventType};
use crate::license::KEY_LEN;

/// The command feature (`kshana-pro forensics dispute`).
pub const FEATURE: &str = "pro-dispute-evidence";
/// The report format tag.
pub const FORMAT: &str = "kshana-pro-dispute-evidence/1";
/// The method.
pub const METHOD_ID: &str = "kshana-pro/dispute-evidence/v1";
/// The report's main file.
pub const RESULT_FILE: &str = "dispute-evidence.json";
/// The label every report carries.
pub const LABEL: &str = "MODELLED evidence: an engineering reconstruction from the data and \
Kshana Pro evidence supplied, under thresholds fixed before the run; tested on synthetic \
scenarios only. NOT a legal finding, not a finding of fact, not an opinion on liability, on \
off-hire or on the construction of any contract clause.";
/// What every report says about itself.
pub const STATEMENT: &str = "This report states when GNSS position or timing integrity was lost \
in the window, for how long (with the bounds the evidence allows), what the cited evidence \
attributes it to, and which independent sources agree. MEASURED = observables as logged; DERIVED \
= computed by a Kshana Pro method; REPORTED = statements, status logs and claims as given. The \
clause templates are generic questions mapped to that evidence; they are not legal advice.";

/// The clause templates: (id, title, what it asks).
pub const TEMPLATES: &[(&str, &str, &str)] = &[
    (
        "off-hire",
        "Off-hire / loss of time",
        "Time lost to loss of GNSS integrity in the window, its attributed cause (external interference, equipment, undetermined), the longest continuous period against a minimum, and the overlap with each claimed off-hire period.",
    ),
    (
        "delay-laycan",
        "Delay against the laycan or a berth slot",
        "Integrity loss before the laycan end (or slot time), the part of each claimed delay that coincides with it, the unexplained remainder, and whether the interference was area-wide.",
    ),
    (
        "dp-downtime",
        "DP downtime",
        "DP downtime from the status logs, the part coinciding with GNSS integrity loss, the remainder, and which position references were lost or stayed available.",
    ),
    (
        "deviation",
        "Deviation",
        "Whether a spoofing episode (false position) or an AIS-versus-own-position discrepancy coincides with the claimed deviation period.",
    ),
    (
        "safe-port",
        "Safe port / refusal to sail",
        "Whether interference in the window was area-wide (neighbours or a Station saw it), how often it recurred, and whether it was present before a stated decision time.",
    ),
];

fn d_merge() -> f64 {
    60.0
}

/// A status log: CSV `utc,source,state[,note]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusLogSpec {
    pub name: String,
    /// `dp` (DP status and position references) or `ops`.
    #[serde(default)]
    pub kind: Option<String>,
    pub path: String,
}

/// A party's claimed period (REPORTED).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimedPeriod {
    pub label: String,
    #[serde(default)]
    pub party: Option<String>,
    pub start_utc: String,
    pub end_utc: String,
}

/// A clause to map evidence to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClauseSpec {
    /// One of [`TEMPLATES`].
    pub template: String,
    /// The clause's reference in the contract (e.g. "Clause 17").
    pub id: String,
    /// The clause text as given (REPORTED; not interpreted).
    #[serde(default)]
    pub text: Option<String>,
    /// `off-hire`: the minimum continuous period that counts (h).
    #[serde(default)]
    pub min_period_h: Option<f64>,
    /// `delay-laycan`: the laycan end (or berth slot) time.
    #[serde(default)]
    pub laycan_end_utc: Option<String>,
    /// `safe-port`: the time of the decision (e.g. the master's refusal to sail).
    #[serde(default)]
    pub decision_utc: Option<String>,
    /// `dp-downtime`: the status sources that mean DP downtime (default: every source of
    /// every `dp` status log whose name contains "DP").
    #[serde(default)]
    pub dp_sources: Vec<String>,
    /// Which claimed periods (by label) this clause is about (default: all).
    #[serde(default)]
    pub claimed: Vec<String>,
}

/// A request (`kshana-pro forensics dispute <request.toml>`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    #[serde(default)]
    pub case_ref: Option<String>,
    #[serde(default)]
    pub vessel_id: Option<String>,
    pub window_start_utc: String,
    pub window_end_utc: String,
    /// A maritime evidence pack request (run here).
    #[serde(default)]
    pub maritime: Option<String>,
    /// A maritime evidence pack directory made earlier.
    #[serde(default)]
    pub maritime_pack: Option<String>,
    /// An incident forensics bundle directory.
    #[serde(default)]
    pub forensics_bundle: Option<String>,
    /// A log time-integrity request (run here).
    #[serde(default)]
    pub log_time: Option<String>,
    #[serde(default)]
    pub status_log: Vec<StatusLogSpec>,
    #[serde(default)]
    pub claimed: Vec<ClaimedPeriod>,
    #[serde(default)]
    pub clauses: Vec<ClauseSpec>,
    /// Spans closer than this merge into one period (s).
    #[serde(default = "d_merge")]
    pub merge_gap_s: f64,
}

impl Request {
    pub fn parse(text: &str) -> Result<Request, String> {
        let r: Request = toml::from_str(text).map_err(|e| format!("invalid request: {e}"))?;
        for c in &r.clauses {
            if !TEMPLATES.iter().any(|t| t.0 == c.template) {
                return Err(format!(
                    "clause {}: unknown template '{}' ({})",
                    c.id,
                    c.template,
                    TEMPLATES.iter().map(|t| t.0).collect::<Vec<_>>().join(", ")
                ));
            }
        }
        if r.maritime.is_none()
            && r.maritime_pack.is_none()
            && r.forensics_bundle.is_none()
            && r.log_time.is_none()
        {
            return Err(
                "the request names no evidence: maritime, maritime_pack, forensics_bundle or log_time"
                    .into(),
            );
        }
        Ok(r)
    }
}

/// One interval from one source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub id: String,
    /// `vessel-gnss`, `forensics:<receiver>`, `log-time:<clock>`, `status:<log>/<source>`.
    pub source: String,
    /// `gnss-jamming`, `gnss-spoofing`, `ship-side-fault`, `ais-manipulation`,
    /// `gnss-degradation`, `timing-integrity-lost`, `status:<state>`.
    pub kind: String,
    /// `MEASURED`, `DERIVED` or `REPORTED`.
    pub label: String,
    /// Counts as loss of position or timing integrity (status logs and AIS-only findings do not).
    pub integrity: bool,
    pub start_utc: String,
    pub end_utc: String,
    /// The bounds the evidence allows (earliest start, latest end).
    pub outer_start_utc: String,
    pub outer_end_utc: String,
    pub duration_s: f64,
    /// `external-interference`, `localised-to-vessel`, `equipment`, `ais-manipulation`,
    /// `undetermined`, `timing-fault`, `reported`.
    pub cause: String,
    pub corroborated_by: Vec<String>,
    /// The evidence it rests on: `KS:<method>#<item>@<sha256 prefix>` or
    /// `REPORTED:<file>@<sha256 prefix>#row-<n>`.
    pub cite: String,
    #[serde(skip)]
    t: (f64, f64),
    #[serde(skip)]
    outer: (f64, f64),
}

/// A merged period of lost integrity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Period {
    pub n: usize,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    /// Union of the inner bounds (the least the evidence supports), s.
    pub duration_min_s: f64,
    /// Union of the outer bounds (the most the evidence allows), s.
    pub duration_max_s: f64,
    pub kinds: Vec<String>,
    pub causes: Vec<String>,
    /// The sources with a span in the period and the independent corroborators they name.
    pub sources: Vec<String>,
    pub independent_sources: usize,
    /// `corroborated` (two or more independent sources) or `single-source`.
    pub agreement: String,
    pub spans: Vec<String>,
    #[serde(skip)]
    t: (f64, f64),
}

/// How a claimed period meets the evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimCheck {
    pub label: String,
    pub party: Option<String>,
    pub start_utc: String,
    pub end_utc: String,
    pub claimed_s: f64,
    /// Overlap with lost integrity (point estimate, inner and outer bounds), s.
    pub with_integrity_loss_s: f64,
    pub with_integrity_loss_min_s: f64,
    pub with_integrity_loss_max_s: f64,
    /// Overlap with reported DP downtime, s.
    pub with_dp_downtime_s: f64,
    /// Claimed time with no integrity loss in the evidence, s.
    pub not_explained_s: f64,
    pub fraction_explained: f64,
}

/// One answered clause question.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub question: String,
    /// `indicated`, `not-indicated`, `undetermined` or `value`.
    pub answer: String,
    pub value: Option<f64>,
    pub unit: Option<String>,
    pub basis: String,
    pub cites: Vec<String>,
}

/// A clause with its answered questions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClauseEvidence {
    pub template: String,
    pub title: String,
    pub id: String,
    /// The clause text as given (REPORTED).
    pub text: Option<String>,
    pub answers: Vec<Answer>,
}

/// A cited evidence source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cited {
    /// `maritime-pack`, `forensics-bundle`, `log-time`, `status-log`.
    pub kind: String,
    pub method: String,
    pub file: String,
    pub sha256: String,
    /// `run here`, or the signature check of a directory made earlier.
    pub provenance: String,
    pub summary: String,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub format: String,
    pub method: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub case_ref: Option<String>,
    pub vessel_id: Option<String>,
    pub window_start_utc: String,
    pub window_end_utc: String,
    pub cited: Vec<Cited>,
    /// The time base of the evidence (from the log time-integrity check), or why it is open.
    pub time_base: String,
    /// Extra uncertainty on every time from the time base (s).
    pub time_base_u_s: f64,
    pub spans: Vec<Span>,
    pub periods: Vec<Period>,
    pub integrity_lost_s: f64,
    pub integrity_lost_min_s: f64,
    pub integrity_lost_max_s: f64,
    pub dp_downtime_s: f64,
    /// Per source: seconds of lost integrity it reports in the window.
    pub per_source_s: BTreeMap<String, f64>,
    pub claims: Vec<ClaimCheck>,
    pub clauses: Vec<ClauseEvidence>,
    pub limits: Vec<String>,
}

// ---------------------------------------------------------------------------------------

fn t_of(s: &str) -> Result<f64, String> {
    parse_time(s)
        .map(|x| x.t)
        .ok_or_else(|| format!("invalid time '{s}'"))
}

fn h12(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

fn union(mut v: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    v.retain(|x| x.1 > x.0);
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for x in v {
        match out.last_mut() {
            Some(l) if x.0 <= l.1 => l.1 = l.1.max(x.1),
            _ => out.push(x),
        }
    }
    out
}

fn length(v: &[(f64, f64)]) -> f64 {
    v.iter().map(|x| x.1 - x.0).sum::<f64>() + 0.0
}

/// Length of the intersection of two unions.
fn overlap(a: &[(f64, f64)], b: &[(f64, f64)]) -> f64 {
    let mut s = 0.0;
    for x in a {
        for y in b {
            let lo = x.0.max(y.0);
            let hi = x.1.min(y.1);
            if hi > lo {
                s += hi - lo;
            }
        }
    }
    s
}

fn clip(x: (f64, f64), w: (f64, f64)) -> (f64, f64) {
    (x.0.max(w.0), x.1.min(w.1))
}

#[allow(clippy::too_many_arguments)]
fn span(
    id: String,
    source: String,
    kind: &str,
    label: &str,
    integrity: bool,
    t: (f64, f64),
    outer: (f64, f64),
    cause: &str,
    corroborated_by: Vec<String>,
    cite: String,
) -> Span {
    Span {
        id,
        source,
        kind: kind.into(),
        label: label.into(),
        integrity,
        start_utc: iso_ms(t.0),
        end_utc: iso_ms(t.1),
        outer_start_utc: iso_ms(outer.0),
        outer_end_utc: iso_ms(outer.1),
        duration_s: sig9(t.1 - t.0),
        cause: cause.into(),
        corroborated_by,
        cite,
        t,
        outer,
    }
}

fn maritime_spans(
    p: &maritime::EvidencePack,
    sha: &str,
    out: &mut Vec<Span>,
) -> Result<(), String> {
    for e in &p.episodes {
        let t = (t_of(&e.start_utc)?, t_of(&e.end_utc)?);
        let outer = (t_of(&e.onset.earliest_utc)?, t_of(&e.end.latest_utc)?);
        let kind = match e.event_type {
            EventType::Jamming => "gnss-jamming",
            EventType::Spoofing => "gnss-spoofing",
            EventType::ShipSideFault => "ship-side-fault",
            EventType::AisManipulation => "ais-manipulation",
        };
        let cause = match e.conclusion {
            Conclusion::VictimOfExternalInterference => "external-interference",
            Conclusion::LocalisedToVessel => "localised-to-vessel",
            Conclusion::ShipSideFault => "equipment",
            Conclusion::AisManipulation => "ais-manipulation",
            Conclusion::NoEvent | Conclusion::Inconclusive => "undetermined",
        };
        let mut corr: Vec<String> = e
            .neighbours_corroborating
            .iter()
            .map(|n| format!("ais-neighbour:{n}"))
            .collect();
        if e.station_corroborates {
            corr.push("station".into());
        }
        out.push(span(
            format!("M{}", e.n),
            "vessel-gnss".into(),
            kind,
            "DERIVED",
            e.event_type != EventType::AisManipulation,
            t,
            (outer.0.min(t.0), outer.1.max(t.1)),
            cause,
            corr,
            format!("KS:{}#episode-{}@{}", maritime::METHOD_ID, e.n, h12(sha)),
        ));
    }
    Ok(())
}

fn forensics_spans(v: &serde_json::Value, sha: &str, out: &mut Vec<Span>) -> Result<(), String> {
    let method = v
        .get("method_id")
        .and_then(|x| x.as_str())
        .unwrap_or(super::METHOD_ID)
        .to_string();
    let empty = vec![];
    let findings = v
        .get("findings")
        .and_then(|x| x.as_array())
        .unwrap_or(&empty);
    for f in findings {
        let s = |path: &[&str]| -> Option<String> {
            let mut x = f;
            for k in path {
                x = x.get(*k)?;
            }
            x.as_str().map(String::from)
        };
        let (Some(id), Some(cat)) = (s(&["id"]), s(&["category"])) else {
            continue;
        };
        let req = |path: &[&str]| -> Result<f64, String> {
            s(path)
                .ok_or_else(|| format!("forensics finding {id}: missing {}", path.join(".")))
                .and_then(|x| t_of(&x))
        };
        let on_e = req(&["onset", "earliest_utc"])?;
        let on_l = req(&["onset", "latest_utc"])?;
        let en_e = req(&["end", "earliest_utc"])?;
        let en_l = req(&["end", "latest_utc"])?;
        let corr: Vec<String> = f
            .get("corroborated_by")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let kind = match cat.as_str() {
            "jamming" => "gnss-jamming",
            "spoofing" => "gnss-spoofing",
            "jamming-and-spoofing" => "gnss-jamming-and-spoofing",
            _ => "gnss-degradation",
        };
        let cause = if cat == "degradation-not-attributed" {
            "undetermined"
        } else if !corr.is_empty() {
            "external-interference"
        } else {
            "undetermined"
        };
        let t = (on_l.min(en_e), en_e.max(on_l));
        out.push(span(
            format!("F-{id}"),
            format!("forensics:{}", s(&["source"]).unwrap_or_default()),
            kind,
            "DERIVED",
            true,
            t,
            (on_e, en_l),
            cause,
            corr,
            format!("KS:{method}#{id}@{}", h12(sha)),
        ));
    }
    Ok(())
}

fn logtime_spans(r: &logtime::Report, sha: &str, out: &mut Vec<Span>) {
    for (k, i) in r.integrity_intervals.iter().enumerate() {
        let u = r
            .clocks
            .iter()
            .find(|c| c.clock == i.clock)
            .and_then(|c| c.corrected_u_s)
            .unwrap_or(0.0);
        out.push(span(
            format!("T{}", k + 1),
            format!("log-time:{}", i.clock),
            "timing-integrity-lost",
            "DERIVED",
            true,
            (i.t0, i.t1),
            (i.t0 - u, i.t1 + u),
            "timing-fault",
            vec![],
            format!(
                "KS:{}#{}/interval-{}@{}",
                logtime::METHOD_ID,
                i.clock,
                k + 1,
                h12(sha)
            ),
        ));
    }
}

const OK_STATES: &[&str] = &[
    "ok",
    "online",
    "available",
    "normal",
    "in-use",
    "selected",
    "green",
];

fn status_spans(
    spec: &StatusLogSpec,
    text: &str,
    sha: &str,
    w: (f64, f64),
    out: &mut Vec<Span>,
) -> Result<(), String> {
    // Per source: (t, state, row).
    let mut rows: BTreeMap<String, Vec<(f64, String, usize)>> = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() < 3 {
            continue;
        }
        let Some(t) = parse_time(f[0]) else {
            if n == 0 {
                continue; // header
            }
            return Err(format!(
                "{}: row {}: invalid time '{}'",
                spec.path,
                n + 1,
                f[0]
            ));
        };
        rows.entry(f[1].to_string())
            .or_default()
            .push((t.t, f[2].to_ascii_lowercase(), n + 1));
    }
    let file = Path::new(&spec.path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    for (src, mut v) in rows {
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        for k in 0..v.len() {
            let (t, ref state, row) = v[k];
            if OK_STATES.contains(&state.as_str()) {
                continue;
            }
            // Until the source's next row with another state.
            let end = v[k + 1..]
                .iter()
                .find(|x| x.1 != *state)
                .map(|x| x.0)
                .unwrap_or(w.1);
            if k > 0 && v[k - 1].1 == *state {
                continue;
            }
            let x = clip((t, end), w);
            if x.1 <= x.0 {
                continue;
            }
            out.push(span(
                format!("S-{}-{row}", spec.name),
                format!("status:{}/{src}", spec.name),
                &format!("status:{state}"),
                "REPORTED",
                false,
                x,
                x,
                "reported",
                vec![],
                format!("REPORTED:{file}@{}#row-{row}", h12(sha)),
            ));
        }
    }
    Ok(())
}

/// The independence group of a source: one per forensics receiver, one per log of the log
/// time check (its clocks and their cross-log disagreements are one source), else the source.
fn source_group(s: &str) -> String {
    match s.split_once(':') {
        Some(("log-time", rest)) => {
            let rest = rest.strip_prefix("cross-log:").unwrap_or(rest);
            format!("log-time:{}", rest.split(':').next().unwrap_or(rest))
        }
        _ => s.to_string(),
    }
}

/// Verify a signed directory when it carries a signature.
fn provenance(dir: &Path, trust: &[[u8; KEY_LEN]]) -> String {
    if !dir.join(crate::signed_dossier::SIGNATURE_FILE).exists() {
        return "unsigned directory: its integrity rests on the hash recorded here only".into();
    }
    match crate::signed_dossier::verify(dir, trust) {
        Ok(v) => format!(
            "signed by {} (key {}) at {}: signature {}, {}{}",
            v.signer,
            v.key_id,
            v.utc,
            if v.signature_ok { "valid" } else { "INVALID" },
            match v.trusted {
                Some(true) => "key trusted (pinned)",
                Some(false) => "key NOT trusted",
                None => "key not pinned (pass --trust)",
            },
            if v.problems.is_empty() {
                String::new()
            } else {
                format!("; problems: {}", v.problems.join("; "))
            }
        ),
        Err(e) => format!("signature check failed: {e}"),
    }
}

/// (file name, bytes).
pub type NamedFile = (String, Vec<u8>);
/// A log time-integrity run: the report, the request text and the logs.
pub type LogTimeOutput = (logtime::Report, String, Vec<NamedFile>);
/// Time intervals (start, end), Unix seconds.
type Intervals = Vec<(f64, f64)>;

/// Everything the evidence step produced, for writing the directory.
#[derive(Default)]
pub struct Assembled {
    pub maritime: Option<(maritime::EvidencePack, maritime::MaritimeInputs)>,
    pub log_time: Option<LogTimeOutput>,
    /// (file name, bytes) copied into `cited/` or `inputs/`.
    pub cited_files: Vec<(String, Vec<u8>)>,
    pub input_files: Vec<(String, Vec<u8>)>,
}

fn resolve(base: &Path, p: &str) -> PathBuf {
    if Path::new(p).is_relative() {
        base.join(p)
    } else {
        PathBuf::from(p)
    }
}

fn read(p: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

/// Assemble the evidence and answer the clauses. Paths are relative to `base`; `trust` pins
/// the keys of pack and bundle directories made earlier.
pub fn assess(
    req: &Request,
    base: &Path,
    trust: &[[u8; KEY_LEN]],
) -> Result<(Report, Assembled), String> {
    let w = (t_of(&req.window_start_utc)?, t_of(&req.window_end_utc)?);
    if w.1 <= w.0 {
        return Err("window_end_utc must be after window_start_utc".into());
    }
    let mut asm = Assembled::default();
    let mut cited = Vec::new();
    let mut spans: Vec<Span> = Vec::new();

    // Maritime actor-vs-victim pack.
    if let Some(m) = &req.maritime {
        let mreq = maritime::MaritimeRequest::from_file(&resolve(base, m))?;
        let inp = mreq.load()?;
        let pack = maritime::analyse(&inp)?;
        let text = json(&pack);
        let sha = sha256_hex(text.as_bytes());
        maritime_spans(&pack, &sha, &mut spans)?;
        cited.push(Cited {
            kind: "maritime-pack".into(),
            method: maritime::METHOD_ID.into(),
            file: format!("maritime/{}", maritime::RESULT_FILE),
            sha256: sha,
            provenance: "run here from the request; written under maritime/ and covered by this report's signature".into(),
            summary: format!(
                "{} episode(s); conclusion {:?}",
                pack.episodes.len(),
                pack.verdict.conclusion
            ),
        });
        asm.maritime = Some((pack, inp));
    }
    if let Some(d) = &req.maritime_pack {
        let dir = resolve(base, d);
        let bytes = read(&dir.join(maritime::RESULT_FILE))?;
        let pack: maritime::EvidencePack = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}: {e}", dir.join(maritime::RESULT_FILE).display()))?;
        if pack.format != maritime::FORMAT {
            return Err(format!(
                "{d}: not a maritime evidence pack ({})",
                pack.format
            ));
        }
        let sha = sha256_hex(&bytes);
        maritime_spans(&pack, &sha, &mut spans)?;
        cited.push(Cited {
            kind: "maritime-pack".into(),
            method: pack.method.clone(),
            file: format!("cited/maritime-{}", maritime::RESULT_FILE),
            sha256: sha,
            provenance: provenance(&dir, trust),
            summary: format!(
                "{} episode(s); conclusion {:?}",
                pack.episodes.len(),
                pack.verdict.conclusion
            ),
        });
        asm.cited_files
            .push((format!("maritime-{}", maritime::RESULT_FILE), bytes));
    }
    // Incident forensics bundle.
    if let Some(d) = &req.forensics_bundle {
        let dir = resolve(base, d);
        let bytes = read(&dir.join("report.json"))?;
        let v: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}: {e}", dir.join("report.json").display()))?;
        if v.get("format").and_then(|x| x.as_str()) != Some(super::FORMAT) {
            return Err(format!(
                "{d}: report.json is not a forensic report ({})",
                super::FORMAT
            ));
        }
        let sha = sha256_hex(&bytes);
        forensics_spans(&v, &sha, &mut spans)?;
        cited.push(Cited {
            kind: "forensics-bundle".into(),
            method: v
                .get("method_id")
                .and_then(|x| x.as_str())
                .unwrap_or(super::METHOD_ID)
                .into(),
            file: "cited/forensics-report.json".into(),
            sha256: sha,
            provenance: provenance(&dir, trust),
            summary: v
                .pointer("/verdict/sentence")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        });
        asm.cited_files
            .push(("forensics-report.json".into(), bytes));
    }
    // Log time integrity.
    let mut time_base =
        "No log time-integrity check was supplied: the times are taken as logged (unverified)."
            .to_string();
    let mut time_base_u = 0.0;
    if let Some(lt) = &req.log_time {
        let path = resolve(base, lt);
        let text = String::from_utf8_lossy(&read(&path)?).to_string();
        let lreq = logtime::Request::parse(&text)?;
        let lbase = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let r = logtime::check(&lreq, &lbase)?;
        let files: Vec<(String, Vec<u8>)> = lreq
            .logs
            .iter()
            .map(|l| {
                let p = resolve(&lbase, &l.path);
                read(&p).map(|b| {
                    (
                        p.file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        b,
                    )
                })
            })
            .collect::<Result<_, _>>()?;
        let rj = json(&r);
        let sha = sha256_hex(rj.as_bytes());
        logtime_spans(&r, &sha, &mut spans);
        time_base_u = r
            .clocks
            .iter()
            .filter_map(|c| c.corrected_u_s)
            .fold(0.0, f64::max);
        time_base = format!(
            "{} (KS:{}@{}); times carry up to {:.3} s standard uncertainty from the corrected timelines.",
            r.verdict,
            logtime::METHOD_ID,
            h12(&sha),
            time_base_u
        );
        cited.push(Cited {
            kind: "log-time".into(),
            method: logtime::METHOD_ID.into(),
            file: format!("log-time/{}", logtime::RESULT_FILE),
            sha256: sha,
            provenance: "run here from the request; written under log-time/ and covered by this report's signature".into(),
            summary: r.verdict.clone(),
        });
        asm.log_time = Some((r, text, files));
    }
    // Status logs.
    for s in &req.status_log {
        let bytes = read(&resolve(base, &s.path))?;
        let sha = sha256_hex(&bytes);
        status_spans(s, &String::from_utf8_lossy(&bytes), &sha, w, &mut spans)?;
        let file = Path::new(&s.path)
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_default();
        cited.push(Cited {
            kind: "status-log".into(),
            method: "REPORTED".into(),
            file: format!("inputs/{file}"),
            sha256: sha,
            provenance: "as supplied (REPORTED)".into(),
            summary: format!(
                "status log '{}' ({})",
                s.name,
                s.kind.as_deref().unwrap_or("ops")
            ),
        });
        asm.input_files.push((file, bytes));
    }

    // Clip to the window; drop spans outside it.
    for s in &mut spans {
        s.t = clip(s.t, w);
        s.outer = clip(s.outer, w);
        s.start_utc = iso_ms(s.t.0);
        s.end_utc = iso_ms(s.t.1);
        s.outer_start_utc = iso_ms(s.outer.0);
        s.outer_end_utc = iso_ms(s.outer.1);
        s.duration_s = sig9((s.t.1 - s.t.0).max(0.0));
    }
    spans.retain(|s| s.t.1 > s.t.0 || s.outer.1 > s.outer.0);
    spans.sort_by(|a, b| a.t.0.total_cmp(&b.t.0).then(a.id.cmp(&b.id)));

    // Merged periods of lost integrity.
    let integ: Vec<&Span> = spans.iter().filter(|s| s.integrity).collect();
    let mut periods: Vec<Period> = Vec::new();
    let mut members: Vec<Vec<&Span>> = Vec::new();
    for s in integ.iter().copied() {
        let lo = s.t.0.min(s.outer.0);
        match members.last_mut() {
            Some(m)
                if lo
                    <= m.iter()
                        .map(|x| x.t.1.max(x.outer.1))
                        .fold(f64::MIN, f64::max)
                        + req.merge_gap_s =>
            {
                m.push(s)
            }
            _ => members.push(vec![s]),
        }
    }
    for (k, m) in members.iter().enumerate() {
        let point = union(m.iter().map(|s| s.t).collect());
        let outer = union(m.iter().map(|s| s.outer).collect());
        let t = (
            point.first().map(|x| x.0).unwrap_or(0.0),
            point.last().map(|x| x.1).unwrap_or(0.0),
        );
        let mut srcs: BTreeSet<String> = BTreeSet::new();
        let mut groups: BTreeSet<String> = BTreeSet::new();
        for s in m {
            srcs.insert(s.source.clone());
            groups.insert(source_group(&s.source));
            for c in &s.corroborated_by {
                srcs.insert(c.clone());
                groups.insert(c.clone());
            }
        }
        let kinds: BTreeSet<String> = m.iter().map(|s| s.kind.clone()).collect();
        let causes: BTreeSet<String> = m.iter().map(|s| s.cause.clone()).collect();
        // The inner duration: the point estimates shrunk by the time-base uncertainty.
        let inner = length(&point) - 2.0 * time_base_u * point.len() as f64;
        periods.push(Period {
            n: k + 1,
            start_utc: iso_ms(t.0),
            end_utc: iso_ms(t.1),
            duration_s: sig9(t.1 - t.0),
            duration_min_s: sig9(inner.max(0.0)),
            duration_max_s: sig9(length(&outer) + 2.0 * time_base_u * outer.len() as f64),
            kinds: kinds.into_iter().collect(),
            causes: causes.into_iter().collect(),
            independent_sources: groups.len(),
            agreement: if groups.len() >= 2 {
                "corroborated".into()
            } else {
                "single-source".into()
            },
            sources: srcs.into_iter().collect(),
            spans: m.iter().map(|s| s.id.clone()).collect(),
            t,
        });
    }
    let lost_point = union(integ.iter().map(|s| s.t).collect());
    let lost_outer = union(integ.iter().map(|s| s.outer).collect());
    let integrity_lost_s = length(&lost_point);
    let integrity_lost_min_s = periods.iter().map(|p| p.duration_min_s).sum::<f64>();
    let integrity_lost_max_s = periods.iter().map(|p| p.duration_max_s).sum::<f64>();
    let mut per_source: BTreeMap<String, f64> = BTreeMap::new();
    for s in integ.iter() {
        *per_source.entry(s.source.clone()).or_default() += s.t.1 - s.t.0;
    }
    for v in per_source.values_mut() {
        *v = sig9(*v);
    }

    // DP downtime.
    let dp_sources_default: Vec<String> = spans
        .iter()
        .filter(|s| s.source.starts_with("status:"))
        .filter(|s| {
            let log = s.source["status:".len()..].split('/').next().unwrap_or("");
            req.status_log
                .iter()
                .any(|l| l.name == log && l.kind.as_deref() == Some("dp"))
        })
        .filter(|s| {
            s.source
                .rsplit('/')
                .next()
                .is_some_and(|x| x.to_ascii_uppercase().contains("DP"))
        })
        .map(|s| s.source.clone())
        .collect();
    let dp_union = |names: &[String]| -> Vec<(f64, f64)> {
        union(
            spans
                .iter()
                .filter(|s| {
                    if names.is_empty() {
                        dp_sources_default.contains(&s.source)
                    } else {
                        names.iter().any(|n| s.source.ends_with(&format!("/{n}")))
                    }
                })
                .map(|s| s.t)
                .collect(),
        )
    };
    let dp_all = dp_union(&[]);
    let dp_downtime_s = length(&dp_all);

    // Claimed periods.
    let mut claims = Vec::new();
    for c in &req.claimed {
        let x = clip((t_of(&c.start_utc)?, t_of(&c.end_utc)?), w);
        let cl = x.1 - x.0;
        let ov = overlap(&[x], &lost_point);
        let ov_in: f64 = periods
            .iter()
            .map(|p| {
                let shrink = (p.duration_s - p.duration_min_s).max(0.0) / 2.0;
                overlap(&[x], &[(p.t.0 + shrink, p.t.1 - shrink)])
            })
            .sum();
        let ov_out = overlap(&[x], &lost_outer);
        claims.push(ClaimCheck {
            label: c.label.clone(),
            party: c.party.clone(),
            start_utc: iso_ms(x.0),
            end_utc: iso_ms(x.1),
            claimed_s: sig9(cl.max(0.0)),
            with_integrity_loss_s: sig9(ov),
            with_integrity_loss_min_s: sig9(ov_in.min(ov)),
            with_integrity_loss_max_s: sig9(ov_out.min(cl.max(0.0))),
            with_dp_downtime_s: sig9(overlap(&[x], &dp_all)),
            not_explained_s: sig9((cl - ov).max(0.0)),
            fraction_explained: if cl > 0.0 { sig9(ov / cl) } else { 0.0 },
        });
    }

    let ctx = ClauseCtx {
        spans: &spans,
        periods: &periods,
        claims: &claims,
        lost: &lost_point,
        w,
        time_base: &time_base,
        cites_tb: cited
            .iter()
            .filter(|c| c.kind == "log-time")
            .map(|c| format!("KS:{}@{}", c.method, h12(&c.sha256)))
            .collect(),
    };
    let mut clauses = Vec::new();
    for c in &req.clauses {
        clauses.push(answer_clause(c, &ctx, &dp_union, &req.claimed)?);
    }

    let report = Report {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        label: LABEL.into(),
        statement: STATEMENT.into(),
        title: req.title.clone(),
        case_ref: req.case_ref.clone(),
        vessel_id: req.vessel_id.clone(),
        window_start_utc: iso_ms(w.0),
        window_end_utc: iso_ms(w.1),
        cited,
        time_base,
        time_base_u_s: sig9(time_base_u),
        spans,
        periods,
        integrity_lost_s: sig9(integrity_lost_s),
        integrity_lost_min_s: sig9(integrity_lost_min_s),
        integrity_lost_max_s: sig9(integrity_lost_max_s),
        dp_downtime_s: sig9(dp_downtime_s),
        per_source_s: per_source,
        claims,
        clauses,
        limits: vec![
            "MODELLED evidence: every span is the output of a Kshana Pro method or a status log as supplied; the methods are tested on synthetic scenarios only and carry their own limits (cited packs).".into(),
            "Not a legal finding: the clause templates ask generic engineering questions; whether a clause applies, its construction, causation in law and the allocation of time are for the parties and the tribunal.".into(),
            "A period is 'corroborated' when two or more independent sources (the vessel's own GNSS, AIS neighbours, a Station, a forensics receiver, a log clock) show it; agreement is not proof of cause.".into(),
            "Inner and outer durations follow the onset and end bounds of the cited evidence and the time base's uncertainty; times outside the window are clipped.".into(),
            "Status logs and claimed periods are REPORTED as supplied and are not verified here.".into(),
        ],
    };
    Ok((report, asm))
}

struct ClauseCtx<'a> {
    spans: &'a [Span],
    periods: &'a [Period],
    claims: &'a [ClaimCheck],
    lost: &'a [(f64, f64)],
    w: (f64, f64),
    time_base: &'a str,
    cites_tb: Vec<String>,
}

fn ans(
    q: &str,
    a: &str,
    value: Option<f64>,
    unit: Option<&str>,
    basis: String,
    cites: Vec<String>,
) -> Answer {
    Answer {
        question: q.into(),
        answer: a.into(),
        value: value.map(sig9),
        unit: unit.map(String::from),
        basis,
        cites,
    }
}

fn cites_of<'a>(spans: impl Iterator<Item = &'a Span>) -> Vec<String> {
    let mut v: Vec<String> = spans.map(|s| s.cite.clone()).collect();
    v.sort();
    v.dedup();
    v
}

fn answer_clause(
    c: &ClauseSpec,
    cx: &ClauseCtx,
    dp_union: &dyn Fn(&[String]) -> Intervals,
    claimed: &[ClaimedPeriod],
) -> Result<ClauseEvidence, String> {
    let (_, title, _) = TEMPLATES
        .iter()
        .find(|t| t.0 == c.template)
        .copied()
        .ok_or_else(|| format!("unknown template {}", c.template))?;
    let integ = || cx.spans.iter().filter(|s| s.integrity);
    let all_cites = cites_of(integ());
    let lost_s = length(cx.lost);
    let claims: Vec<&ClaimCheck> = cx
        .claims
        .iter()
        .filter(|k| c.claimed.is_empty() || c.claimed.contains(&k.label))
        .collect();
    let mut a = Vec::new();
    let causes = || -> BTreeSet<String> { integ().map(|s| s.cause.clone()).collect() };
    let claim_answers = |a: &mut Vec<Answer>, what: &str| {
        for k in &claims {
            a.push(ans(
                &format!("How much of the claimed {what} '{}' coincides with lost integrity?", k.label),
                "value",
                Some(k.with_integrity_loss_s),
                Some("s"),
                format!(
                    "{:.0} s of {:.0} s claimed ({:.0} %; bounds {:.0}–{:.0} s); {:.0} s of the claim has no integrity loss in the evidence",
                    k.with_integrity_loss_s,
                    k.claimed_s,
                    100.0 * k.fraction_explained,
                    k.with_integrity_loss_min_s,
                    k.with_integrity_loss_max_s,
                    k.not_explained_s
                ),
                all_cites.clone(),
            ));
        }
        if claims.is_empty() && !claimed.is_empty() {
            a.push(ans(
                &format!("Claimed {what} periods"),
                "undetermined",
                None,
                None,
                "the clause names claimed periods that the request does not hold".into(),
                vec![],
            ));
        }
    };
    let area_wide: Vec<&Span> = integ()
        .filter(|s| !s.corroborated_by.is_empty() || s.cause == "external-interference")
        .collect();
    match c.template.as_str() {
        "off-hire" => {
            a.push(ans(
                "Was GNSS position or timing integrity lost in the window?",
                if lost_s > 0.0 {
                    "indicated"
                } else {
                    "not-indicated"
                },
                Some(lost_s),
                Some("s"),
                format!(
                    "{} period(s) totalling {:.0} s ({:.2} h) of lost integrity in the window",
                    cx.periods.len(),
                    lost_s,
                    lost_s / 3600.0
                ),
                all_cites.clone(),
            ));
            let mut cs = causes();
            let timing = cs.remove("timing-fault");
            let (verdict, mut basis) = if cs.is_empty() && timing {
                (
                    "undetermined",
                    "only log-clock (timing) faults: no position-domain evidence of the cause"
                        .to_string(),
                )
            } else if cs.is_empty() {
                ("not-indicated", "no integrity loss".to_string())
            } else if cs.len() == 1 && cs.contains("external-interference") {
                (
                    "indicated",
                    "external interference: the vessel's GNSS anomaly was seen by independent sources (neighbours, Station or another receiver)".into(),
                )
            } else if cs.contains("equipment") {
                (
                    "indicated",
                    format!("the evidence attributes at least part to the ship's own equipment (causes: {})", cs.iter().cloned().collect::<Vec<_>>().join(", ")),
                )
            } else {
                (
                    "undetermined",
                    format!(
                        "causes in the evidence: {}",
                        cs.iter().cloned().collect::<Vec<_>>().join(", ")
                    ),
                )
            };
            if timing && !cs.is_empty() {
                basis.push_str(
                    "; log clocks also show timing faults in the same window (see the time base)",
                );
            }
            a.push(ans(
                "What does the evidence attribute the loss to (external interference vs the ship's equipment)?",
                verdict,
                None,
                None,
                basis,
                all_cites.clone(),
            ));
            let longest = cx.periods.iter().map(|p| p.duration_s).fold(0.0, f64::max);
            if let Some(h) = c.min_period_h {
                a.push(ans(
                    &format!(
                        "Is the longest continuous period at least the clause minimum ({h} h)?"
                    ),
                    if longest >= h * 3600.0 {
                        "indicated"
                    } else {
                        "not-indicated"
                    },
                    Some(longest / 3600.0),
                    Some("h"),
                    format!(
                        "longest period {:.0} s ({:.2} h)",
                        longest,
                        longest / 3600.0
                    ),
                    all_cites.clone(),
                ));
            }
            claim_answers(&mut a, "off-hire period");
        }
        "delay-laycan" => {
            let end = match &c.laycan_end_utc {
                Some(s) => t_of(s)?,
                None => cx.w.1,
            };
            let before = overlap(&[(cx.w.0, end)], cx.lost);
            a.push(ans(
                "How much integrity loss falls before the laycan end (or slot time)?",
                if before > 0.0 {
                    "indicated"
                } else {
                    "not-indicated"
                },
                Some(before),
                Some("s"),
                format!(
                    "{:.0} s between the window start and {}",
                    before,
                    iso_ms(end)
                ),
                all_cites.clone(),
            ));
            claim_answers(&mut a, "delay");
            a.push(ans(
                "Was the interference area-wide (outside the vessel's own control)?",
                if area_wide.is_empty() {
                    if lost_s > 0.0 {
                        "undetermined"
                    } else {
                        "not-indicated"
                    }
                } else {
                    "indicated"
                },
                Some(area_wide.len() as f64),
                Some("spans"),
                if area_wide.is_empty() {
                    "no span is corroborated by an independent source".into()
                } else {
                    format!(
                        "{} span(s) corroborated by independent sources ({})",
                        area_wide.len(),
                        area_wide
                            .iter()
                            .flat_map(|s| s.corroborated_by.iter().cloned())
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
                cites_of(area_wide.iter().copied()),
            ));
        }
        "dp-downtime" => {
            let dp = dp_union(&c.dp_sources);
            let dp_s = length(&dp);
            let co = overlap(&dp, cx.lost);
            let dp_cites = cites_of(cx.spans.iter().filter(|s| {
                s.source.starts_with("status:") && dp.iter().any(|x| s.t.0 < x.1 && s.t.1 > x.0)
            }));
            a.push(ans(
                "How much DP downtime do the status logs report in the window?",
                if dp_s > 0.0 { "value" } else { "not-indicated" },
                Some(dp_s),
                Some("s"),
                format!(
                    "{} downtime interval(s) totalling {:.0} s (REPORTED)",
                    dp.len(),
                    dp_s
                ),
                dp_cites.clone(),
            ));
            a.push(ans(
                "How much of the DP downtime coincides with lost GNSS integrity?",
                if dp_s <= 0.0 {
                    "undetermined"
                } else if co > 0.0 {
                    "indicated"
                } else {
                    "not-indicated"
                },
                Some(co),
                Some("s"),
                format!(
                    "{:.0} s of {:.0} s ({:.0} %); {:.0} s of downtime has no GNSS integrity loss in the evidence",
                    co,
                    dp_s,
                    if dp_s > 0.0 { 100.0 * co / dp_s } else { 0.0 },
                    (dp_s - co).max(0.0)
                ),
                [dp_cites.clone(), all_cites.clone()].concat(),
            ));
            // Position references: per status source, lost time and whether it stayed up.
            let mut refs: BTreeMap<String, f64> = BTreeMap::new();
            for s in cx.spans.iter().filter(|s| s.source.starts_with("status:")) {
                *refs.entry(s.source.clone()).or_default() += s.t.1 - s.t.0;
            }
            let lost_refs: Vec<String> =
                refs.iter().map(|(k, v)| format!("{k} {v:.0} s")).collect();
            a.push(ans(
                "Which position references or DP sources were reported lost, and for how long?",
                if refs.is_empty() {
                    "not-indicated"
                } else {
                    "value"
                },
                Some(refs.len() as f64),
                Some("sources"),
                if refs.is_empty() {
                    "no status source reported a state other than ok".into()
                } else {
                    format!(
                        "{}; sources never reported lost stayed available",
                        lost_refs.join("; ")
                    )
                },
                dp_cites,
            ));
            claim_answers(&mut a, "DP downtime");
        }
        "deviation" => {
            let spoof: Vec<&Span> = integ().filter(|s| s.kind.contains("spoofing")).collect();
            let ais: Vec<&Span> = cx
                .spans
                .iter()
                .filter(|s| s.kind == "ais-manipulation")
                .collect();
            let sp_s: f64 = length(&union(spoof.iter().map(|s| s.t).collect()));
            a.push(ans(
                "Was there a spoofing episode (false own position) in the window?",
                if spoof.is_empty() {
                    "not-indicated"
                } else {
                    "indicated"
                },
                Some(sp_s),
                Some("s"),
                format!("{} spoofing span(s), {:.0} s", spoof.len(), sp_s),
                cites_of(spoof.iter().copied()),
            ));
            a.push(ans(
                "Did the vessel's AIS broadcast depart from its own position (AIS manipulation)?",
                if ais.is_empty() { "not-indicated" } else { "indicated" },
                Some(ais.len() as f64),
                Some("spans"),
                if ais.is_empty() {
                    "no AIS-versus-own-NMEA discrepancy in the cited maritime pack".into()
                } else {
                    "the AIS broadcast departs from the vessel's own clean NMEA (see the maritime pack: by whom the data cannot say)".into()
                },
                cites_of(ais.iter().copied()),
            ));
            claim_answers(&mut a, "deviation period");
        }
        "safe-port" => {
            let aw = union(area_wide.iter().map(|s| s.t).collect());
            a.push(ans(
                "Was interference area-wide in the window (seen by neighbours, a Station or another receiver)?",
                if aw.is_empty() {
                    if lost_s > 0.0 { "undetermined" } else { "not-indicated" }
                } else {
                    "indicated"
                },
                Some(length(&aw)),
                Some("s"),
                format!("{} area-wide period(s) totalling {:.0} s", aw.len(), length(&aw)),
                cites_of(area_wide.iter().copied()),
            ));
            a.push(ans(
                "How often did integrity loss recur in the window?",
                "value",
                Some(cx.periods.len() as f64),
                Some("periods"),
                format!("{} separate period(s)", cx.periods.len()),
                all_cites.clone(),
            ));
            if let Some(d) = &c.decision_utc {
                let td = t_of(d)?;
                let before = overlap(&[(cx.w.0, td)], cx.lost);
                a.push(ans(
                    &format!("Was integrity loss present before the decision at {d}?"),
                    if before > 0.0 { "indicated" } else { "not-indicated" },
                    Some(before),
                    Some("s"),
                    format!("{:.0} s of lost integrity in the window before the decision; reasonableness is not judged here", before),
                    all_cites.clone(),
                ));
            }
        }
        _ => {}
    }
    a.push(ans(
        "Can the logs' time stamps be relied on for these durations?",
        if cx.cites_tb.is_empty() {
            "undetermined"
        } else if cx.time_base.starts_with("INTACT") {
            "indicated"
        } else {
            "value"
        },
        None,
        None,
        cx.time_base.to_string(),
        cx.cites_tb.clone(),
    ));
    Ok(ClauseEvidence {
        template: c.template.clone(),
        title: title.into(),
        id: c.id.clone(),
        text: c.text.clone(),
        answers: a,
    })
}

/// Human-readable summary.
pub fn render_text(r: &Report) -> String {
    let mut s = format!(
        "Dispute evidence: {}\nWindow {} .. {}\n{}\n\n",
        r.title, r.window_start_utc, r.window_end_utc, r.label
    );
    s.push_str(&format!(
        "Integrity lost: {:.0} s (bounds {:.0}–{:.0} s) in {} period(s); DP downtime reported: {:.0} s\nTime base: {}\n\n",
        r.integrity_lost_s,
        r.integrity_lost_min_s,
        r.integrity_lost_max_s,
        r.periods.len(),
        r.dp_downtime_s,
        r.time_base
    ));
    for p in &r.periods {
        s.push_str(&format!(
            "Period {}: {} .. {} ({:.0} s, {:.0}–{:.0} s) {}: {} [{}]\n  sources: {}\n",
            p.n,
            p.start_utc,
            p.end_utc,
            p.duration_s,
            p.duration_min_s,
            p.duration_max_s,
            p.agreement,
            p.kinds.join(", "),
            p.causes.join(", "),
            p.sources.join(", ")
        ));
    }
    for c in &r.claims {
        s.push_str(&format!(
            "Claim '{}'{}: {:.0} s claimed, {:.0} s with lost integrity ({:.0} %), {:.0} s with DP downtime\n",
            c.label,
            c.party.as_deref().map(|p| format!(" ({p})")).unwrap_or_default(),
            c.claimed_s,
            c.with_integrity_loss_s,
            100.0 * c.fraction_explained,
            c.with_dp_downtime_s
        ));
    }
    for c in &r.clauses {
        s.push_str(&format!("\n{} — {} [{}]\n", c.id, c.title, c.template));
        for a in &c.answers {
            s.push_str(&format!(
                "  - {} -> {}{}\n      {}\n      cites: {}\n",
                a.question,
                a.answer,
                a.value
                    .map(|v| format!(" ({v} {})", a.unit.as_deref().unwrap_or("")))
                    .unwrap_or_default(),
                a.basis,
                if a.cites.is_empty() {
                    "-".into()
                } else {
                    a.cites.join(", ")
                }
            ));
        }
    }
    s.push_str(&format!("\n{}\n", r.statement));
    s
}

/// Intervals as CSV.
pub fn spans_csv(r: &Report) -> String {
    let mut s = String::from(
        "id,source,kind,label,integrity,start_utc,end_utc,outer_start_utc,outer_end_utc,duration_s,cause,corroborated_by,cite\n",
    );
    for x in &r.spans {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            x.id,
            x.source.replace(',', ";"),
            x.kind,
            x.label,
            x.integrity,
            x.start_utc,
            x.end_utc,
            x.outer_start_utc,
            x.outer_end_utc,
            x.duration_s,
            x.cause,
            x.corroborated_by.join(";"),
            x.cite
        ));
    }
    s
}

/// Write the report directory (unsigned): `dispute-evidence.json`, `spans.csv`,
/// `README.txt`, `request.toml`, `inputs/`, `cited/`, and `maritime/` and `log-time/` for
/// evidence run here.
pub fn write_dir(dir: &Path, r: &Report, asm: &Assembled, req_text: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(dir.join(RESULT_FILE), json(r) + "\n").map_err(io)?;
    std::fs::write(dir.join("spans.csv"), spans_csv(r)).map_err(io)?;
    std::fs::write(dir.join("README.txt"), render_text(r)).map_err(io)?;
    std::fs::write(dir.join("request.toml"), req_text).map_err(io)?;
    if !asm.input_files.is_empty() {
        std::fs::create_dir_all(dir.join("inputs")).map_err(io)?;
        for (n, b) in &asm.input_files {
            std::fs::write(dir.join("inputs").join(n), b).map_err(io)?;
        }
    }
    if !asm.cited_files.is_empty() {
        std::fs::create_dir_all(dir.join("cited")).map_err(io)?;
        for (n, b) in &asm.cited_files {
            std::fs::write(dir.join("cited").join(n), b).map_err(io)?;
        }
    }
    if let Some((pack, inp)) = &asm.maritime {
        maritime::write_pack(&dir.join("maritime"), pack, inp)?;
    }
    if let Some((lr, text, files)) = &asm.log_time {
        logtime::write_dir(&dir.join("log-time"), lr, text, files)?;
    }
    Ok(())
}

/// A synthetic case with known truth: area jamming at sea (the maritime pack's scenario,
/// 08:10:00–08:18:20 UTC on 2026-06-01), a VDR whose second receiver rolled its GPS week over
/// at 08:10:00, DP status logs, two claimed periods and the five clause templates.
pub mod synth {
    use std::path::Path;

    use crate::forensics::logtime::synth as lt;
    use crate::forensics::maritime::synth as ms;

    /// The DP status log.
    pub const STATUS_CSV: &str = "utc,source,state\n\
2026-06-01T08:00:00Z,DGNSS1,ok\n\
2026-06-01T08:00:00Z,DGNSS2,ok\n\
2026-06-01T08:00:00Z,HPR,ok\n\
2026-06-01T08:00:00Z,DP,ok\n\
2026-06-01T08:10:10Z,DGNSS1,rejected\n\
2026-06-01T08:10:12Z,DGNSS2,rejected\n\
2026-06-01T08:10:30Z,DP,downtime\n\
2026-06-01T08:18:40Z,DGNSS1,ok\n\
2026-06-01T08:18:45Z,DGNSS2,ok\n\
2026-06-01T08:25:00Z,DP,ok\n";

    /// The dispute request.
    pub const REQUEST: &str = r#"title = "Synthetic: DP charter, GNSS jamming 2026-06-01"
case_ref = "SYN-0001"
vessel_id = "636000001"
window_start_utc = "2026-06-01T08:00:00Z"
window_end_utc = "2026-06-01T08:30:00Z"
maritime = "maritime/request.toml"
log_time = "log-time/request.toml"

[[status_log]]
name = "dp-log"
kind = "dp"
path = "dp-status.csv"

[[claimed]]
label = "Charterer off-hire"
party = "charterer"
start_utc = "2026-06-01T08:10:00Z"
end_utc = "2026-06-01T08:30:00Z"

[[claimed]]
label = "Owner delay"
party = "owner"
start_utc = "2026-06-01T08:10:00Z"
end_utc = "2026-06-01T08:20:00Z"

[[clauses]]
template = "off-hire"
id = "Clause 17"
min_period_h = 0.1
claimed = ["Charterer off-hire"]

[[clauses]]
template = "delay-laycan"
id = "Laycan"
laycan_end_utc = "2026-06-01T09:00:00Z"
claimed = ["Owner delay"]

[[clauses]]
template = "dp-downtime"
id = "DP performance"

[[clauses]]
template = "deviation"
id = "Deviation"
claimed = ["Owner delay"]

[[clauses]]
template = "safe-port"
id = "Safe port"
decision_utc = "2026-06-01T08:20:00Z"
"#;

    /// Write the case into `dir`.
    pub fn write_demo(dir: &Path) -> std::io::Result<()> {
        let m = ms::make(ms::Case::AreaJamming);
        let md = dir.join("maritime");
        std::fs::create_dir_all(&md)?;
        std::fs::write(md.join("own.nmea"), &m.nmea)?;
        std::fs::write(md.join("ais.csv"), &m.ais_csv)?;
        std::fs::write(
            md.join("request.toml"),
            format!(
                "title = \"Synthetic area jamming\"\nvessel_id = \"{}\"\nnmea = \"own.nmea\"\nais = \"ais.csv\"\n",
                m.vessel_id
            ),
        )?;
        let ld = dir.join("log-time");
        std::fs::create_dir_all(&ld)?;
        std::fs::write(ld.join("vdr.nmea"), lt::vdr(lt::Fault::WeekRollover).0)?;
        std::fs::write(
            ld.join("request.toml"),
            "title = \"VDR clocks\"\nwindow_start_utc = \"2026-06-01T08:00:00Z\"\nwindow_end_utc = \"2026-06-01T08:30:00Z\"\n\n[[logs]]\nname = \"vdr\"\nkind = \"vdr\"\nformat = \"nmea\"\npath = \"vdr.nmea\"\nu_s = 0.2\n",
        )?;
        std::fs::write(dir.join("dp-status.csv"), STATUS_CSV)?;
        std::fs::write(dir.join("request.toml"), REQUEST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_and_overlap() {
        let u = union(vec![(0.0, 10.0), (5.0, 20.0), (30.0, 40.0)]);
        assert_eq!(u, vec![(0.0, 20.0), (30.0, 40.0)]);
        assert_eq!(length(&u), 30.0);
        assert_eq!(overlap(&u, &[(15.0, 35.0)]), 10.0);
    }

    #[test]
    fn status_spans_run_until_ok() {
        let spec = StatusLogSpec {
            name: "dp".into(),
            kind: Some("dp".into()),
            path: "s.csv".into(),
        };
        let mut v = Vec::new();
        status_spans(
            &spec,
            synth::STATUS_CSV,
            "ab",
            (1_780_300_800.0, 1_780_302_600.0),
            &mut v,
        )
        .unwrap();
        let dp: Vec<&Span> = v.iter().filter(|s| s.source == "status:dp/DP").collect();
        assert_eq!(dp.len(), 1);
        assert_eq!(dp[0].start_utc, "2026-06-01T08:10:30.000Z");
        assert_eq!(dp[0].end_utc, "2026-06-01T08:25:00.000Z");
        assert!(!v.iter().any(|s| s.source.ends_with("/HPR")));
    }

    #[test]
    fn source_groups() {
        assert_eq!(source_group("log-time:vdr:GN"), "log-time:vdr");
        assert_eq!(source_group("log-time:cross-log:vdr:GN"), "log-time:vdr");
        assert_eq!(source_group("log-time:console"), "log-time:console");
        assert_eq!(source_group("forensics:bridge"), "forensics:bridge");
        assert_eq!(source_group("vessel-gnss"), "vessel-gnss");
    }

    #[test]
    fn unknown_template_refused() {
        let r = Request::parse(
            "title='x'\nwindow_start_utc='2026-06-01T08:00:00Z'\nwindow_end_utc='2026-06-01T09:00:00Z'\nlog_time='a.toml'\n[[clauses]]\ntemplate='nope'\nid='c'\n",
        );
        assert!(r.unwrap_err().contains("unknown template"));
    }
}
