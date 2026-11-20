// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The avionics procedure format (`kshana-avionics-procedure/1`), parameters, the equipment
//! log, and scoring.
//!
//! ```toml
//! format = "kshana-avionics-procedure/1"
//! id = "sbas-udrei-do-not-use"
//! version = 1
//! title = "SBAS: a satellite set to 'do not use' is dropped within the alert time"
//! kind = "sbas-sequence"                 # sbas-sequence | araim | gbas-events | spoof-detection
//! modelled_on = ["RTCA DO-229F, Appendix A (message content)"]
//!
//! [[parameter]]                          # a limit the applicant supplies
//! name = "alert_time_s"
//! unit = "s"
//! meaning = "first message carrying the alarm to the satellite no longer used"
//! source = "applicant: from their copy of the MOPS, for the operation under test"
//!
//! [sequence]                             # kind = sbas-sequence (see sbas_seq)
//! duration_s = 180
//! prns = [1, 3, 6, 9, 12, 14, 17]
//! [[sequence.event]]
//! id = "dnu"
//! kind = "udrei"
//! t_s = 60
//! duration_s = 60
//! prn = 6
//! udrei = 15
//! ```
//!
//! A parameter may carry a `public_value` with its `public_source` only when the value is
//! published openly (for example the ARAIM reference tolerance). Every other limit comes
//! from the applicant's parameter file (`kshana-avionics-params/1`); a check whose limit was
//! not supplied is **not assessed**, never met.
//!
//! The equipment log (`kshana-avionics-dut-log/1`, JSON) holds the equipment's output per
//! second (mode, satellites used, protection levels, alert flag) for SBAS procedures, and
//! per case (protection levels, availability, excluded constellations) for ARAIM procedures.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::araim::{self, AraimSpec, CaseResult};
use super::sbas_seq::{self, ExpectKind, Expectation, SequenceSpec, Stream};
use super::{sha256_hex, sig9};
use crate::scorecards::levels::Verdict;

/// The format tag of a procedure.
pub const FORMAT: &str = "kshana-avionics-procedure/1";
/// The format tag of a parameter file.
pub const PARAMS_FORMAT: &str = "kshana-avionics-params/1";
/// The format tag of an equipment log.
pub const LOG_FORMAT: &str = "kshana-avionics-dut-log/1";
/// The format tag of a verdict.
pub const VERDICT_FORMAT: &str = "kshana-avionics-verdict/1";
/// The method identifier of the scoring.
pub const METHOD_ID: &str = "kshana-pro/avionics/v1";

fn one() -> u32 {
    1
}
fn modelled() -> String {
    "MODELLED".into()
}

/// What a procedure runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    SbasSequence,
    Araim,
    /// A GBAS event timeline ([`super::gbas`]).
    GbasEvents,
    /// Spoofing-detection output verification ([`super::spoof_protocol`]).
    SpoofDetection,
}

/// A limit the procedure needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub unit: String,
    pub meaning: String,
    /// Where the value comes from (normally: the applicant's copy of the document).
    pub source: String,
    /// A value only when it is openly published, with where.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_value: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub public_source: String,
}

/// A procedure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Procedure {
    pub format: String,
    pub id: String,
    #[serde(default = "one")]
    pub version: u32,
    pub title: String,
    #[serde(default)]
    pub purpose: String,
    pub kind: Kind,
    #[serde(default = "modelled")]
    pub evidence: String,
    /// Documents the procedure is modelled on: document, revision, section; never their text.
    #[serde(default)]
    pub modelled_on: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, rename = "parameter")]
    pub parameters: Vec<Parameter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<SequenceSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub araim: Option<AraimSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gbas: Option<super::gbas::GbasSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spoof: Option<super::spoof_protocol::SpoofSpec>,
}

// Built from parts so the source itself never spells the words.
const BANNED: [&str; 5] = [
    concat!("compl", "iant"),
    concat!("compl", "iance"),
    concat!("con", "form"),
    concat!("certi", "fied"),
    concat!("quali", "fied"),
];

impl Procedure {
    pub fn parse(src: &str) -> Result<Procedure, String> {
        let p: Procedure = toml::from_str(src).map_err(|e| format!("procedure: {e}"))?;
        p.validate()?;
        Ok(p)
    }

    pub fn load(path: &std::path::Path) -> Result<Procedure, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Procedure::parse(&s).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// SHA-256 of the canonical TOML.
    pub fn sha256(&self) -> String {
        sha256_hex(self.to_toml().as_bytes())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT {
            return Err(format!("format '{}' is not {FORMAT}", self.format));
        }
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!("id '{}': letters, digits, '-' or '_'", self.id));
        }
        // The wording rule: a procedure never claims that equipment meets a document.
        let text = format!(
            "{} {} {}",
            self.title,
            self.purpose,
            self.modelled_on.join(" ")
        )
        .to_lowercase();
        if let Some(w) = BANNED.iter().find(|w| text.contains(*w)) {
            return Err(format!(
                "procedure '{}': the word '{w}' is not used in procedures (they are test \
                 procedures modelled on a document, not statements that equipment meets it)",
                self.id
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for p in &self.parameters {
            if !names.insert(&p.name) {
                return Err(format!("parameter '{}' declared twice", p.name));
            }
            if p.public_value.is_some() && p.public_source.trim().is_empty() {
                return Err(format!(
                    "parameter '{}': a public_value needs its public_source",
                    p.name
                ));
            }
        }
        match self.kind {
            Kind::SbasSequence => {
                let s = self
                    .sequence
                    .as_ref()
                    .ok_or("kind sbas-sequence needs [sequence]")?;
                if self.araim.is_some() {
                    return Err("kind sbas-sequence takes no [araim]".into());
                }
                s.validate()?;
                // Every limit an expectation names is declared.
                for e in sbas_seq::generate(s)?.expectations {
                    if !e.limit_param.is_empty() && !names.contains(&e.limit_param) {
                        return Err(format!(
                            "expectation '{}' needs parameter '{}', not declared",
                            e.id, e.limit_param
                        ));
                    }
                }
            }
            Kind::Araim => {
                let a = self.araim.as_ref().ok_or("kind araim needs [araim]")?;
                if self.sequence.is_some() {
                    return Err("kind araim takes no [sequence]".into());
                }
                a.validate()?;
            }
            Kind::SpoofDetection => {
                let s = self
                    .spoof
                    .as_ref()
                    .ok_or("kind spoof-detection needs [spoof]")?;
                if self.sequence.is_some() || self.araim.is_some() || self.gbas.is_some() {
                    return Err("kind spoof-detection takes only [spoof]".into());
                }
                s.validate()?;
                for p in ["detection_time_s", "max_false_alert_s"] {
                    if !names.contains(&p.to_string()) {
                        return Err(format!("kind spoof-detection needs parameter '{p}'"));
                    }
                }
            }
            Kind::GbasEvents => {
                let g = self.gbas.as_ref().ok_or("kind gbas-events needs [gbas]")?;
                if self.sequence.is_some() || self.araim.is_some() {
                    return Err("kind gbas-events takes no [sequence] or [araim]".into());
                }
                for e in super::gbas::generate(g)?.expectations {
                    if !e.limit_param.is_empty() && !names.contains(&e.limit_param) {
                        return Err(format!(
                            "expectation '{}' needs parameter '{}', not declared",
                            e.id, e.limit_param
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// The parameter values in force: the applicant's file, then public values.
    pub fn resolve_params(&self, file: Option<&ParamsFile>) -> Params {
        let mut m = BTreeMap::new();
        for p in &self.parameters {
            if let Some(v) = file.and_then(|f| f.values.get(&p.name)) {
                let src = file
                    .and_then(|f| f.sources.get(&p.name).cloned())
                    .unwrap_or_else(|| "applicant-supplied".into());
                m.insert(
                    p.name.clone(),
                    Limit {
                        param: p.name.clone(),
                        value: Some(*v),
                        source: format!(
                            "{} (supplied by {})",
                            src,
                            file.map(|f| f.supplied_by.as_str()).unwrap_or("?")
                        ),
                    },
                );
            } else if let Some(v) = p.public_value {
                m.insert(
                    p.name.clone(),
                    Limit {
                        param: p.name.clone(),
                        value: Some(v),
                        source: format!("public: {}", p.public_source),
                    },
                );
            } else {
                m.insert(
                    p.name.clone(),
                    Limit {
                        param: p.name.clone(),
                        value: None,
                        source: format!("not supplied ({})", p.source),
                    },
                );
            }
        }
        Params(m)
    }
}

/// An applicant's parameter file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamsFile {
    pub format: String,
    pub supplied_by: String,
    #[serde(default)]
    pub values: BTreeMap<String, f64>,
    /// Per parameter: where the applicant took the value (document, revision, section).
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
}

impl ParamsFile {
    pub fn parse(src: &str) -> Result<ParamsFile, String> {
        let p: ParamsFile = toml::from_str(src).map_err(|e| format!("params: {e}"))?;
        if p.format != PARAMS_FORMAT {
            return Err(format!(
                "params: format '{}' is not {PARAMS_FORMAT}",
                p.format
            ));
        }
        if p.supplied_by.trim().is_empty() {
            return Err("params: supplied_by is required".into());
        }
        for (k, v) in &p.values {
            if !v.is_finite() {
                return Err(format!("params: {k} is not a finite number"));
            }
        }
        Ok(p)
    }
}

/// One limit in force.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    pub param: String,
    pub value: Option<f64>,
    pub source: String,
}

/// The limits in force for a run.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Params(pub BTreeMap<String, Limit>);

impl Params {
    pub fn value(&self, name: &str) -> Option<f64> {
        self.0.get(name).and_then(|l| l.value)
    }
    pub fn source(&self, name: &str) -> String {
        self.0
            .get(name)
            .map(|l| l.source.clone())
            .unwrap_or_else(|| "not declared".into())
    }
    pub fn limit(&self, name: &str) -> Limit {
        self.0.get(name).cloned().unwrap_or(Limit {
            param: name.into(),
            value: None,
            source: "not declared".into(),
        })
    }
}

/// The equipment's operating mode, ordered from least to most SBAS-dependent.
/// Equipment modes in rank order; `gbas-gls` (a GBAS approach) ranks above the SBAS modes so
/// that "at most `gnss-only`" means neither augmentation is in use.
pub const MODES: [&str; 6] = [
    "none",
    "gnss-only",
    "sbas-npa",
    "sbas-apv",
    "sbas-pa",
    "gbas-gls",
];

/// The rank of a mode in [`MODES`].
pub fn mode_rank(m: &str) -> Option<usize> {
    MODES.iter().position(|x| *x == m)
}

/// The equipment identity.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
}

/// One second of equipment output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DutEpoch {
    pub t_s: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_prns: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hpl_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vpl_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert: Option<bool>,
}

/// The equipment's output for one ARAIM case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DutAraimCase {
    pub id: String,
    pub vpl_m: Option<f64>,
    pub hpl_m: Option<f64>,
    #[serde(default)]
    pub emt_m: Option<f64>,
    #[serde(default)]
    pub sigma_v_acc_m: Option<f64>,
    #[serde(default)]
    pub available_vertical: Option<bool>,
    #[serde(default)]
    pub available_horizontal: Option<bool>,
    #[serde(default)]
    pub excluded_constellations: Vec<String>,
}

/// An equipment log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DutLog {
    pub format: String,
    pub device: Device,
    pub procedure: String,
    /// Where the log came from (`lab`, `SYNTHETIC`, ...).
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub epochs: Vec<DutEpoch>,
    #[serde(default)]
    pub araim_cases: Vec<DutAraimCase>,
}

impl DutLog {
    pub fn parse(src: &str) -> Result<DutLog, String> {
        let l: DutLog = serde_json::from_str(src).map_err(|e| format!("log: {e}"))?;
        if l.format != LOG_FORMAT {
            return Err(format!("log: format '{}' is not {LOG_FORMAT}", l.format));
        }
        let mut last = f64::NEG_INFINITY;
        for e in &l.epochs {
            if !e.t_s.is_finite() || e.t_s <= last {
                return Err(format!("log: epoch times must increase (at t = {})", e.t_s));
            }
            last = e.t_s;
            if let Some(m) = &e.mode {
                if mode_rank(m).is_none() {
                    return Err(format!(
                        "log: mode '{m}' at t = {} (expected {})",
                        e.t_s,
                        MODES.join(", ")
                    ));
                }
            }
        }
        Ok(l)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// One check's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub kind: String,
    pub subject: String,
    pub limit_param: String,
    pub limit: Option<f64>,
    pub limit_source: String,
    pub measured: Option<f64>,
    pub verdict: Verdict,
    pub detail: String,
}

/// A procedure's verdict for one run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcedureVerdict {
    pub format: String,
    pub method: String,
    pub label: String,
    pub procedure: String,
    pub procedure_version: u32,
    pub procedure_sha256: String,
    pub title: String,
    pub modelled_on: Vec<String>,
    pub device: Device,
    pub log_source: String,
    pub log_sha256: Option<String>,
    pub limits: Vec<Limit>,
    pub checks: Vec<Check>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub araim_reference: Vec<CaseResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stimulus_frames_sha256: Option<String>,
    pub met: usize,
    pub not_met: usize,
    pub not_assessed: usize,
    /// `pass` (every scored check met), `fail` (any not met) or `incomplete` (none not met,
    /// some not assessed).
    pub verdict: String,
}

/// Score `log` (if any) for `proc` with `params`.
pub fn score(
    proc: &Procedure,
    params: &Params,
    log: Option<&DutLog>,
    log_bytes: Option<&[u8]>,
) -> Result<ProcedureVerdict, String> {
    proc.validate()?;
    if let Some(l) = log {
        if l.procedure != proc.id {
            return Err(format!(
                "the log is for procedure '{}', not '{}'",
                l.procedure, proc.id
            ));
        }
    }
    let (checks, refs, frames_sha) = match proc.kind {
        Kind::SbasSequence => {
            let stream = sbas_seq::generate(proc.sequence.as_ref().expect("validated"))?;
            (
                sbas_checks(&stream, params, log),
                vec![],
                Some(stream.frames_sha256.clone()),
            )
        }
        Kind::Araim => {
            let (c, r) = araim::checks(proc.araim.as_ref().expect("validated"), params, log)?;
            (c, r, None)
        }
        Kind::GbasEvents => {
            let stream = super::gbas::generate(proc.gbas.as_ref().expect("validated"))?;
            let mut c = sbas_checks(&stream, params, log);
            // The satellite precondition is SBAS's; GBAS needs the approach mode before the
            // first event instead.
            c.retain(|x| x.id != "precondition-satellites-used");
            if let Some(l) = log {
                let first = stream
                    .expectations
                    .iter()
                    .map(|x| x.t_ref_s)
                    .fold(f64::INFINITY, f64::min);
                let pre: Vec<&DutEpoch> = l.epochs.iter().filter(|e| e.t_s < first).collect();
                let ok = (!pre.is_empty() && pre.iter().any(|e| e.mode.is_some()))
                    .then(|| pre.iter().any(|e| e.mode.as_deref() == Some("gbas-gls")));
                c.insert(
                    0,
                    Check {
                        id: "precondition-gbas-approach".into(),
                        kind: "precondition".into(),
                        subject: "mode gbas-gls before the first event".into(),
                        limit_param: String::new(),
                        limit: None,
                        limit_source: String::new(),
                        measured: None,
                        verdict: Verdict::from_bool(ok),
                        detail: match ok {
                            None => "no mode logged before the first event".into(),
                            Some(true) => "the GBAS approach was in use before the events".into(),
                            Some(false) => "the GBAS approach was never in use before the events; the test cannot show it being left".into(),
                        },
                    },
                );
            }
            (c, vec![], Some(stream.frames_sha256.clone()))
        }
        Kind::SpoofDetection => (
            super::spoof_protocol::checks(proc.spoof.as_ref().expect("validated"), params, log),
            vec![],
            None,
        ),
    };
    let scored = checks.iter().filter(|c| c.kind != "reference");
    let met = scored.clone().filter(|c| c.verdict == Verdict::Met).count();
    let not_met = scored
        .clone()
        .filter(|c| c.verdict == Verdict::NotMet)
        .count();
    let not_assessed = scored.filter(|c| c.verdict == Verdict::NotAssessed).count();
    Ok(ProcedureVerdict {
        format: VERDICT_FORMAT.into(),
        method: METHOD_ID.into(),
        label: super::LABEL.into(),
        procedure: proc.id.clone(),
        procedure_version: proc.version,
        procedure_sha256: proc.sha256(),
        title: proc.title.clone(),
        modelled_on: proc.modelled_on.clone(),
        device: log.map(|l| l.device.clone()).unwrap_or_default(),
        log_source: log.map(|l| l.source.clone()).unwrap_or_default(),
        log_sha256: log_bytes.map(sha256_hex),
        limits: params.0.values().cloned().collect(),
        checks,
        araim_reference: refs,
        stimulus_frames_sha256: frames_sha,
        met,
        not_met,
        not_assessed,
        verdict: if not_met > 0 {
            "fail"
        } else if not_assessed > 0 || met == 0 {
            "incomplete"
        } else {
            "pass"
        }
        .into(),
    })
}

fn used(e: &DutEpoch, prn: u16) -> Option<bool> {
    e.used_prns.as_ref().map(|u| u.contains(&prn))
}

/// The SBAS checks: one per expectation (informational ones excluded), plus a precondition
/// that every masked satellite is used before the first event.
pub fn sbas_checks(stream: &Stream, params: &Params, log: Option<&DutLog>) -> Vec<Check> {
    let mut out = Vec::new();
    let Some(log) = log else {
        for x in stream
            .expectations
            .iter()
            .filter(|x| x.kind != ExpectKind::Informational)
        {
            out.push(not_assessed(x, params, "no equipment log supplied"));
        }
        return out;
    };
    let first_event = stream
        .expectations
        .iter()
        .map(|x| x.t_ref_s)
        .fold(f64::INFINITY, f64::min);
    let masked: Vec<u16> = stream
        .expectations
        .iter()
        .flat_map(|x| x.prns.iter().copied())
        .collect();
    let pre: Vec<&DutEpoch> = log.epochs.iter().filter(|e| e.t_s < first_event).collect();
    let pre_ok = if pre.is_empty() || pre.iter().all(|e| e.used_prns.is_none()) {
        None
    } else {
        Some(
            masked
                .iter()
                .all(|p| pre.iter().any(|e| used(e, *p) == Some(true))),
        )
    };
    out.push(Check {
        id: "precondition-satellites-used".into(),
        kind: "precondition".into(),
        subject: format!("PRNs {masked:?}"),
        limit_param: String::new(),
        limit: None,
        limit_source: String::new(),
        measured: None,
        verdict: Verdict::from_bool(pre_ok),
        detail: match pre_ok {
            None => "no satellite use logged before the first event".into(),
            Some(true) => "every satellite the events affect was used before them".into(),
            Some(false) => "a satellite the events affect was never used before them; the test \
                          cannot show its exclusion"
                .into(),
        },
    });
    let log_end = log.epochs.last().map(|e| e.t_s).unwrap_or(0.0);
    for x in &stream.expectations {
        let limit = params.limit(&x.limit_param);
        let until = x.until_s.unwrap_or(f64::INFINITY);
        let check = match x.kind {
            ExpectKind::Informational => continue,
            ExpectKind::Exclude | ExpectKind::ModeAtMost => {
                // The holding condition per epoch.
                let target = x.mode.as_deref().and_then(mode_rank);
                let holds = |e: &DutEpoch| -> Option<bool> {
                    match x.kind {
                        ExpectKind::Exclude => {
                            let mut all = true;
                            for p in &x.prns {
                                all &= !used(e, *p)?;
                            }
                            Some(all)
                        }
                        _ => Some(mode_rank(e.mode.as_deref()?)? <= target?),
                    }
                };
                let win: Vec<&DutEpoch> = log
                    .epochs
                    .iter()
                    .filter(|e| e.t_s >= x.t_ref_s && e.t_s < until)
                    .collect();
                // First epoch from which the condition holds to the end of the window.
                let mut from: Option<f64> = None;
                let mut unknown = false;
                for e in &win {
                    match holds(e) {
                        None => unknown = true,
                        Some(true) => {
                            if from.is_none() {
                                from = Some(e.t_s);
                            }
                        }
                        Some(false) => from = None,
                    }
                }
                let measured = from.map(|f| f - x.t_ref_s);
                let horizon = until.min(log_end);
                let (verdict, detail) = match (measured, limit.value) {
                    (_, _) if win.is_empty() => (
                        Verdict::NotAssessed,
                        "the log does not cover the window".into(),
                    ),
                    (_, _) if unknown && from.is_none() => (
                        Verdict::NotAssessed,
                        "the log lacks the fields this check reads".into(),
                    ),
                    (Some(m), Some(l)) => (
                        Verdict::from_bool(Some(m <= l)),
                        format!("held from {m:.1} s after the reference (limit {l} s)"),
                    ),
                    (Some(m), None) => (
                        Verdict::NotAssessed,
                        format!("held from {m:.1} s; limit '{}' not supplied", limit.param),
                    ),
                    (None, Some(l)) if horizon - x.t_ref_s > l => (
                        Verdict::NotMet,
                        "never held to the end of the window".to_string(),
                    ),
                    (None, _) => (
                        Verdict::NotAssessed,
                        "not held, and the window ends before the limit".into(),
                    ),
                };
                mk(x, &limit, measured, verdict, detail)
            }
            ExpectKind::Include => {
                let mut first: Option<f64> = None;
                let mut unknown = false;
                for e in log.epochs.iter().filter(|e| e.t_s >= x.t_ref_s) {
                    let mut all = Some(true);
                    for p in &x.prns {
                        all = match (all, used(e, *p)) {
                            (Some(a), Some(u)) => Some(a && u),
                            _ => None,
                        };
                    }
                    match all {
                        None => unknown = true,
                        Some(true) => {
                            first = Some(e.t_s);
                            break;
                        }
                        Some(false) => {}
                    }
                }
                let measured = first.map(|f| f - x.t_ref_s);
                let (verdict, detail) = match (measured, limit.value) {
                    (Some(m), Some(l)) => (
                        Verdict::from_bool(Some(m <= l)),
                        format!("used again {m:.1} s after the event ended (limit {l} s)"),
                    ),
                    (Some(m), None) => (
                        Verdict::NotAssessed,
                        format!(
                            "used again after {m:.1} s; limit '{}' not supplied",
                            limit.param
                        ),
                    ),
                    (None, Some(l)) if !unknown && log_end - x.t_ref_s > l => {
                        (Verdict::NotMet, "not used again before the log ends".into())
                    }
                    (None, _) => (
                        Verdict::NotAssessed,
                        "not seen used again before the log ends".into(),
                    ),
                };
                mk(x, &limit, measured, verdict, detail)
            }
            ExpectKind::RemainUsed => {
                let win: Vec<&DutEpoch> = log
                    .epochs
                    .iter()
                    .filter(|e| e.t_s >= x.t_ref_s && e.t_s < until)
                    .collect();
                let mut res = if win.is_empty() { None } else { Some(true) };
                for e in &win {
                    for p in &x.prns {
                        res = match (res, used(e, *p)) {
                            (Some(a), Some(u)) => Some(a && u),
                            _ => None,
                        };
                    }
                }
                let detail = match res {
                    Some(true) => "kept in use throughout".to_string(),
                    Some(false) => "dropped during the event".to_string(),
                    None => "the log does not show satellite use over the window".to_string(),
                };
                mk(x, &limit, None, Verdict::from_bool(res), detail)
            }
        };
        out.push(check);
    }
    out
}

fn mk(x: &Expectation, limit: &Limit, measured: Option<f64>, v: Verdict, detail: String) -> Check {
    Check {
        id: x.id.clone(),
        kind: match x.kind {
            ExpectKind::Exclude => "exclude",
            ExpectKind::Include => "include",
            ExpectKind::ModeAtMost => "mode_at_most",
            ExpectKind::RemainUsed => "remain_used",
            ExpectKind::Informational => "informational",
        }
        .into(),
        subject: if x.prns.is_empty() {
            format!("mode <= {}", x.mode.clone().unwrap_or_default())
        } else {
            format!("PRNs {:?}", x.prns)
        },
        limit_param: x.limit_param.clone(),
        limit: if x.limit_param.is_empty() {
            None
        } else {
            limit.value
        },
        limit_source: if x.limit_param.is_empty() {
            String::new()
        } else {
            limit.source.clone()
        },
        measured: measured.map(sig9),
        verdict: v,
        detail: format!("{}: {detail}", x.what),
    }
}

fn not_assessed(x: &Expectation, params: &Params, why: &str) -> Check {
    mk(
        x,
        &params.limit(&x.limit_param),
        None,
        Verdict::NotAssessed,
        why.into(),
    )
}

/// A SYNTHETIC equipment log for an SBAS procedure, to try the scoring: `good` follows every
/// expectation one second after its reference time; otherwise the equipment never reacts.
pub fn synth_log(proc: &Procedure, good: bool) -> Result<DutLog, String> {
    let mut log = DutLog {
        format: LOG_FORMAT.into(),
        device: Device {
            name: if good {
                "synthetic-good"
            } else {
                "synthetic-bad"
            }
            .into(),
            firmware: None,
            part_number: None,
        },
        procedure: proc.id.clone(),
        source: "SYNTHETIC".into(),
        epochs: vec![],
        araim_cases: vec![],
    };
    match proc.kind {
        Kind::SbasSequence => {
            let seq = proc.sequence.as_ref().expect("validated");
            let stream = sbas_seq::generate(seq)?;
            for t in 0..seq.duration_s {
                let t = t as f64;
                let mut used: Vec<u16> = seq.prns.clone();
                let mut mode = "sbas-pa";
                if good {
                    for x in &stream.expectations {
                        let until = x.until_s.unwrap_or(f64::INFINITY);
                        let active = t >= x.t_ref_s + 1.0 && t < until;
                        match x.kind {
                            ExpectKind::Exclude if active => {
                                used.retain(|p| !x.prns.contains(p));
                            }
                            ExpectKind::ModeAtMost if active => mode = "gnss-only",
                            _ => {}
                        }
                    }
                }
                log.epochs.push(DutEpoch {
                    t_s: t,
                    mode: Some(mode.into()),
                    used_prns: Some(used),
                    hpl_m: Some(12.0),
                    vpl_m: Some(if mode == "sbas-pa" { 15.0 } else { 0.0 }),
                    alert: Some(false),
                });
            }
        }
        Kind::SpoofDetection => {
            log.epochs =
                super::spoof_protocol::synth_epochs(proc.spoof.as_ref().expect("validated"), good);
        }
        Kind::GbasEvents => {
            let g = proc.gbas.as_ref().expect("validated");
            let stream = super::gbas::generate(g)?;
            for t in 0..g.duration_s {
                let t = t as f64;
                let mut mode = "gbas-gls";
                if good {
                    for x in &stream.expectations {
                        let until = x.until_s.unwrap_or(f64::INFINITY);
                        if x.kind == ExpectKind::ModeAtMost && t >= x.t_ref_s + 1.0 && t < until {
                            mode = "gnss-only";
                        }
                    }
                }
                log.epochs.push(DutEpoch {
                    t_s: t,
                    mode: Some(mode.into()),
                    used_prns: None,
                    hpl_m: Some(8.0),
                    vpl_m: Some(if mode == "gbas-gls" { 10.0 } else { 0.0 }),
                    alert: Some(false),
                });
            }
        }
        Kind::Araim => {
            let a = proc.araim.as_ref().expect("validated");
            if a.check == araim::AraimCheck::Cases {
                let params = proc.resolve_params(None);
                for r in araim::run_cases(a)? {
                    let off = if good { 0.01 } else { 2.5 };
                    log.araim_cases.push(DutAraimCase {
                        id: r.id.clone(),
                        vpl_m: Some(r.vpl_m + off),
                        hpl_m: Some(r.hpl_m + off),
                        emt_m: None,
                        sigma_v_acc_m: None,
                        available_vertical: params.value("val_m").map(|v| r.vpl_m <= v),
                        available_horizontal: params.value("hal_m").map(|v| r.hpl_m <= v),
                        excluded_constellations: if good {
                            r.declared_unusable.clone()
                        } else {
                            vec![]
                        },
                    });
                }
            }
        }
    }
    Ok(log)
}
