// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Discount-scheme device scorecard and A-PNT verification (research backlog B1303, E13)
//!
//! Some marine insurers and mutuals offer a premium discount for fitted anti-spoofing or
//! alternative-PNT (A-PNT) equipment. The discount rests on what the kit is said to do. This
//! module checks each such claim against signed test evidence, under a published method, and
//! writes a neutral device scorecard an insurer can read:
//!
//! * **Claims** ([`Claim`]) are stated before checking, as the scheme or the vendor words them,
//!   each with a kind and a threshold: `spoof_detection` and `jam_detection` (detection rate at
//!   least `min_rate`, from a flag-efficacy scorecard, B1301), `no_misleading_output` (no valid,
//!   unflagged wrong output: flag-efficacy "flag normal while wrong" episodes, unit-scorecard
//!   `misleading_epochs`, receiver-scorecard totals), `recovery_within_s` and
//!   `residual_within_ns` (timing recovery, B1304, or unit-scorecard `recovery_s`), and
//!   `apnt_holdover` (A-PNT verification: the named alternative source kept time within `value`
//!   ns of its baseline throughout a GNSS attack, from a timing-recovery report).
//! * **Evidence** ([`EvidenceRef`]) is a signed scorecard directory (verified with the signer
//!   keys given; evidence whose signature fails, or whose signer is not trusted, is set aside)
//!   or a bare JSON file (recorded as unsigned and never enough for `supported`).
//! * Each claim is `supported`, `not supported` or `insufficient evidence` (too few runs, no
//!   matching evidence, only unsigned evidence).
//! * **Publication rule**: the request states who paid for the testing. A scorecard of testing
//!   paid for by the device's maker or seller is marked not publishable as a neutral scorecard
//!   (it remains a valid record for the parties); so is one resting on unsigned or set-aside
//!   evidence.
//!
//! The method is [`METHOD`], published with every scorecard. Wording: a check of stated claims
//! against the evidence supplied; not underwriting advice, a premium recommendation, or a
//! certification of the device. Evidence: **MODELLED** (checked on synthetic scorecards,
//! `tests/receiver_flag_scorecards.rs`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::SourceCheck;
use crate::scorecards::flags::{self, sig9, wilson95};
use crate::scorecards::units;
use crate::timing::recovery;

pub const REQUEST_FORMAT: &str = "kshana-pro-scheme-device-request/1";
pub const FORMAT: &str = "kshana-pro-scheme-device-scorecard/1";
pub const METHOD_ID: &str = "kshana-pro/scheme-device-scorecard/v1";
pub const RESULT_FILE: &str = "scheme-device-scorecard.json";
pub const METHOD: &str = "Each claim is checked against signed evidence only. Detection claims: \
pooled detection runs and hits of every indicator of the claimed meaning over every class of the \
threat (flag-efficacy scorecard, matching model and, when given, firmware); supported when the \
pooled rate is at least min_rate and at least min_runs runs were judged, not supported when the \
rate is below, insufficient otherwise; the Wilson 95 % interval is reported. No misleading \
output: not supported on any flag-normal-while-wrong episode, any unit not meeting a \
misleading_epochs criterion, or any misleading unwarned epoch of a receiver scorecard; supported \
when at least one such source was judged and none failed. Recovery and residual: every matching \
receiver recovered within the value (timing recovery) or every unit met recovery_s at or below \
the value (unit scorecard). A-PNT holdover: the named receiver of a timing-recovery report kept \
|TE - baseline| within the value throughout the attack. Publication: not publishable as a neutral \
scorecard when the testing was paid for by the device's maker or seller, or when any evidence \
was unsigned or set aside.";
pub const EVIDENCE_LABEL: &str = "MODELLED: the method is checked on synthetic scorecards; no \
fielded device has been assessed in this release.";
pub const STATEMENT: &str = "A check of the stated claims against the evidence supplied, under \
the method stated. Not underwriting advice, a premium recommendation, or a certification of \
any device.";

/// The device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub maker: String,
    pub model: String,
    #[serde(default)]
    pub firmware: Option<String>,
}

/// Who paid for the testing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Funding {
    /// `independent`, `insurer`, `operator`, `subject` (the maker) or `seller`.
    pub paid_by: String,
    #[serde(default)]
    pub note: Option<String>,
}

/// Evidence of a known kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    pub id: String,
    /// `flag-efficacy`, `unit-scorecard`, `timing-recovery`, `receiver-scorecard`.
    pub kind: String,
    /// A signed directory, or a JSON file (unsigned).
    pub path: String,
}

/// A claim kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    SpoofDetection,
    JamDetection,
    NoMisleadingOutput,
    RecoveryWithinS,
    ResidualWithinNs,
    ApntHoldover,
}

/// A claim, as the scheme or vendor words it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub text: String,
    pub kind: ClaimKind,
    /// Detection claims: the rate claimed (default 0.9).
    #[serde(default)]
    pub min_rate: Option<f64>,
    /// Detection claims: runs needed (default 5).
    #[serde(default)]
    pub min_runs: Option<usize>,
    /// Recovery (s), residual and A-PNT holdover (ns) claims.
    #[serde(default)]
    pub value: Option<f64>,
    /// A-PNT holdover: the receiver id of the alternative source in the timing report.
    #[serde(default)]
    pub receiver: Option<String>,
    /// The evidence ids this claim is checked against (default: all).
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// A scheme device request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub format: String,
    pub title: String,
    /// The scheme, as the insurer publishes it.
    pub scheme: String,
    pub device: Device,
    pub funding: Funding,
    #[serde(rename = "evidence")]
    pub evidence: Vec<EvidenceRef>,
    #[serde(rename = "claim")]
    pub claims: Vec<Claim>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

const KINDS: [&str; 4] = [
    "flag-efficacy",
    "unit-scorecard",
    "timing-recovery",
    "receiver-scorecard",
];
const PAYERS: [&str; 5] = ["independent", "insurer", "operator", "subject", "seller"];

impl Request {
    pub fn parse(src: &str) -> Result<Request, String> {
        let r: Request =
            toml::from_str(src).map_err(|e| format!("invalid scheme device request: {e}"))?;
        r.validate()?;
        Ok(r)
    }

    pub fn from_file(path: &Path) -> Result<(Request, String), String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r = Request::parse(&src)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != REQUEST_FORMAT {
            return Err(format!("format must be '{REQUEST_FORMAT}'"));
        }
        if self.device.model.trim().is_empty() {
            return Err("device: model is needed".into());
        }
        if !PAYERS.contains(&self.funding.paid_by.as_str()) {
            return Err(format!(
                "funding.paid_by must be one of {}",
                PAYERS.join(", ")
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for e in &self.evidence {
            if !KINDS.contains(&e.kind.as_str()) {
                return Err(format!(
                    "evidence '{}': kind must be one of {}",
                    e.id,
                    KINDS.join(", ")
                ));
            }
            if !ids.insert(e.id.as_str()) {
                return Err(format!("evidence id '{}' is repeated", e.id));
            }
        }
        if self.claims.is_empty() {
            return Err("state at least one [[claim]]".into());
        }
        let mut cids = std::collections::BTreeSet::new();
        for c in &self.claims {
            if !cids.insert(c.id.as_str()) {
                return Err(format!("claim id '{}' is repeated", c.id));
            }
            for e in &c.evidence {
                if !ids.contains(e.as_str()) {
                    return Err(format!("claim '{}': unknown evidence '{e}'", c.id));
                }
            }
            match c.kind {
                ClaimKind::SpoofDetection | ClaimKind::JamDetection => {
                    if c.min_rate.is_some_and(|r| !(0.0..=1.0).contains(&r)) {
                        return Err(format!("claim '{}': min_rate is in [0, 1]", c.id));
                    }
                }
                ClaimKind::NoMisleadingOutput => {}
                _ => {
                    if !c.value.is_some_and(|v| v.is_finite() && v > 0.0) {
                        return Err(format!("claim '{}': needs a positive value", c.id));
                    }
                    if c.kind == ClaimKind::ApntHoldover && c.receiver.is_none() {
                        return Err(format!(
                            "claim '{}': apnt_holdover names the alternative source's receiver id",
                            c.id
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        match (&self.base_dir, pb.is_absolute()) {
            (Some(b), false) => b.join(pb),
            _ => pb,
        }
    }
}

/// Loaded evidence.
#[derive(Debug, Clone)]
pub enum Loaded {
    Flags(Box<flags::Scorecard>),
    Units(Box<units::Scorecard>),
    Timing(Box<recovery::Report>),
    Receiver(serde_json::Value),
}

/// One evidence source as checked.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvidenceCheck {
    pub id: String,
    pub kind: String,
    /// `signed`, `unsigned` or `set aside`.
    pub status: String,
    pub source: Option<SourceCheck>,
}

/// One checked claim.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaimResult {
    pub id: String,
    pub text: String,
    pub kind: ClaimKind,
    /// `supported`, `not supported` or `insufficient evidence`.
    pub verdict: String,
    pub measured: Option<f64>,
    pub ci95: Option<(f64, f64)>,
    pub basis: Vec<String>,
}

/// The device scorecard.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scorecard {
    pub format: String,
    pub method: String,
    pub method_text: String,
    pub evidence_label: String,
    pub statement: String,
    pub title: String,
    pub scheme: String,
    pub device: Device,
    pub funding: Funding,
    pub publishable: bool,
    pub publication_note: String,
    pub evidence: Vec<EvidenceCheck>,
    pub claims: Vec<ClaimResult>,
    pub supported: usize,
    pub not_supported: usize,
    pub insufficient: usize,
    pub limits: Vec<String>,
}

fn file_for(kind: &str) -> &'static str {
    match kind {
        "flag-efficacy" => flags::RESULT_FILE,
        "unit-scorecard" => units::RESULT_FILE,
        "timing-recovery" => recovery::RESULT_FILE,
        _ => "receiver-scorecard.json",
    }
}

/// Parse evidence JSON of a kind.
pub fn parse_evidence(kind: &str, text: &str) -> Result<Loaded, String> {
    let e = |x: serde_json::Error| format!("{kind}: {x}");
    Ok(match kind {
        "flag-efficacy" => {
            let s: flags::Scorecard = serde_json::from_str(text).map_err(e)?;
            if s.format != flags::FORMAT {
                return Err(format!("{kind}: not a {} scorecard", flags::FORMAT));
            }
            Loaded::Flags(Box::new(s))
        }
        "unit-scorecard" => {
            let s: units::Scorecard = serde_json::from_str(text).map_err(e)?;
            if s.format != units::FORMAT {
                return Err(format!("{kind}: not a {} scorecard", units::FORMAT));
            }
            Loaded::Units(Box::new(s))
        }
        "timing-recovery" => {
            let s: recovery::Report = serde_json::from_str(text).map_err(e)?;
            if s.format != recovery::FORMAT {
                return Err(format!("{kind}: not a {} report", recovery::FORMAT));
            }
            Loaded::Timing(Box::new(s))
        }
        _ => {
            let v: serde_json::Value = serde_json::from_str(text).map_err(e)?;
            if v["format"].as_str() != Some(crate::scorecards::FORMAT) {
                return Err(format!(
                    "{kind}: not a {} scorecard",
                    crate::scorecards::FORMAT
                ));
            }
            Loaded::Receiver(v)
        }
    })
}

/// Load and verify every evidence source.
pub fn load(
    r: &Request,
    trust: &[[u8; 32]],
) -> Result<Vec<(EvidenceCheck, Option<Loaded>)>, String> {
    let mut out = vec![];
    for e in &r.evidence {
        let p = r.resolve(&e.path);
        let (file, source) = if p.is_dir() {
            (
                p.join(file_for(&e.kind)),
                Some(super::check(&p, &e.path, trust)),
            )
        } else {
            (p.clone(), None)
        };
        let status = match &source {
            None => "unsigned",
            Some(s) if s.signature_valid && s.signer_trusted != Some(false) => "signed",
            Some(_) => "set aside",
        };
        let text = std::fs::read_to_string(&file)
            .map_err(|x| format!("evidence '{}': {}: {x}", e.id, file.display()))?;
        let loaded = parse_evidence(&e.kind, &text)?;
        out.push((
            EvidenceCheck {
                id: e.id.clone(),
                kind: e.kind.clone(),
                status: status.into(),
                source,
            },
            (status != "set aside").then_some(loaded),
        ));
    }
    Ok(out)
}

fn fw_match(want: &Option<String>, have: &Option<String>) -> bool {
    want.is_none() || have.is_none() || want == have
}

fn check_claim(c: &Claim, dev: &Device, ev: &[(EvidenceCheck, Option<Loaded>)]) -> ClaimResult {
    let use_ev: Vec<&(EvidenceCheck, Option<Loaded>)> = ev
        .iter()
        .filter(|(e, _)| c.evidence.is_empty() || c.evidence.contains(&e.id))
        .collect();
    let mut basis = vec![];
    let mut signed_basis = false;
    let mut ci = None;
    // (judged anything, failed anything)
    let mut judged = false;
    let mut failed = false;
    let mut detection: Option<(usize, usize)> = None;
    let mut worst: Option<f64> = None;
    for (chk, l) in &use_ev {
        let Some(l) = l else {
            basis.push(format!("{}: set aside (signature)", chk.id));
            continue;
        };
        let signed = chk.status == "signed";
        let before = (judged, detection, worst);
        match (c.kind, l) {
            (ClaimKind::SpoofDetection | ClaimKind::JamDetection, Loaded::Flags(s)) => {
                let (meanings, prefix): (&[flags::Meaning], &str) =
                    if c.kind == ClaimKind::SpoofDetection {
                        (&[flags::Meaning::Spoofing], "spoofing-")
                    } else {
                        (
                            &[
                                flags::Meaning::Jamming,
                                flags::Meaning::JammingCw,
                                flags::Meaning::JammingBroadband,
                            ],
                            "jamming-",
                        )
                    };
                // Pool per run: a run detects when any indicator of the meaning fired.
                let names: Vec<&str> = s
                    .table
                    .iter()
                    .filter(|t| t.model == dev.model && fw_match(&dev.firmware, &t.firmware))
                    .filter(|t| meanings.contains(&t.meaning))
                    .map(|t| t.indicator.as_str())
                    .collect();
                let mut runs = 0;
                let mut hits = 0;
                for run in s.runs.iter().filter(|x| {
                    x.unit.model == dev.model && fw_match(&dev.firmware, &x.unit.firmware)
                }) {
                    for class in run.classes.iter().filter(|k| k.starts_with(prefix)) {
                        let rs: Vec<&flags::RunIndicator> = run
                            .indicators
                            .iter()
                            .filter(|ri| {
                                &ri.class == class && names.contains(&ri.indicator.as_str())
                            })
                            .filter(|ri| ri.raised.is_some())
                            .collect();
                        if rs.is_empty() {
                            continue;
                        }
                        runs += 1;
                        if rs.iter().any(|ri| ri.raised == Some(true)) {
                            hits += 1;
                        }
                    }
                }
                if runs > 0 {
                    let d = detection.get_or_insert((0, 0));
                    d.0 += hits;
                    d.1 += runs;
                    basis.push(format!(
                        "{}: {hits} of {runs} {prefix}* run(s) detected by {}",
                        chk.id,
                        names.join(", ")
                    ));
                }
            }
            (ClaimKind::NoMisleadingOutput, Loaded::Flags(s)) => {
                let mut n = 0;
                let mut eps = 0;
                for run in s.runs.iter().filter(|x| {
                    x.unit.model == dev.model && fw_match(&dev.firmware, &x.unit.firmware)
                }) {
                    n += 1;
                    eps += run
                        .normal_while_wrong
                        .iter()
                        .map(|w| w.epochs)
                        .sum::<usize>();
                }
                if n > 0 {
                    judged = true;
                    failed |= eps > 0;
                    basis.push(format!(
                        "{}: {eps} epoch(s) of flag-normal-while-wrong output over {n} run(s)",
                        chk.id
                    ));
                }
            }
            (ClaimKind::NoMisleadingOutput, Loaded::Units(s)) => {
                for m in s.models.iter().filter(|m| {
                    m.model == dev.model && fw_match(&dev.firmware, &Some(m.firmware.clone()))
                }) {
                    for cr in m
                        .criteria
                        .iter()
                        .filter(|x| x.metric == "misleading_epochs")
                    {
                        if cr.units_met + cr.units_not_met > 0 {
                            judged = true;
                            failed |= cr.units_not_met > 0;
                            basis.push(format!(
                                "{}: {} '{}': {} of {} unit(s) met misleading_epochs {} {}",
                                chk.id,
                                cr.scenario,
                                cr.criterion,
                                cr.units_met,
                                cr.units_met + cr.units_not_met,
                                cr.op,
                                cr.value
                            ));
                        }
                    }
                }
            }
            (ClaimKind::NoMisleadingOutput, Loaded::Receiver(v)) => {
                if v["receiver"]["model"].as_str() == Some(dev.model.as_str()) {
                    if let Some(n) = v["totals"]["misleading_unwarned_epochs"].as_u64() {
                        judged = true;
                        failed |= n > 0;
                        basis.push(format!(
                            "{}: receiver scorecard, {n} misleading unwarned epoch(s)",
                            chk.id
                        ));
                    }
                }
            }
            (ClaimKind::RecoveryWithinS | ClaimKind::ResidualWithinNs, Loaded::Timing(t)) => {
                let v = c.value.unwrap_or(0.0);
                for x in t.receivers.iter().filter(|x| {
                    x.id != recovery::ENSEMBLE
                        && x.model.as_deref().is_none_or(|m| m == dev.model)
                        && fw_match(&dev.firmware, &x.firmware)
                }) {
                    let m = if c.kind == ClaimKind::RecoveryWithinS {
                        match x.recovery.as_str() {
                            "recovered" => x.recovery_s,
                            "not recovered" => Some(f64::INFINITY),
                            _ => None,
                        }
                    } else {
                        x.residual_ns.map(f64::abs)
                    };
                    if let Some(m) = m {
                        judged = true;
                        failed |= m > v;
                        worst = Some(worst.map_or(m, |w: f64| w.max(m)));
                        basis.push(format!("{}: receiver {}: {m}", chk.id, x.id));
                    }
                }
            }
            (ClaimKind::RecoveryWithinS, Loaded::Units(s)) => {
                let v = c.value.unwrap_or(0.0);
                for m in s.models.iter().filter(|m| {
                    m.model == dev.model && fw_match(&dev.firmware, &Some(m.firmware.clone()))
                }) {
                    for cr in m.criteria.iter().filter(|x| x.metric == "recovery_s") {
                        if let Some(mx) = cr.max {
                            judged = true;
                            failed |= mx > v;
                            worst = Some(worst.map_or(mx, |w: f64| w.max(mx)));
                            basis.push(format!(
                                "{}: {} '{}': slowest unit {mx} s",
                                chk.id, cr.scenario, cr.criterion
                            ));
                        }
                    }
                }
            }
            (ClaimKind::ApntHoldover, Loaded::Timing(t)) => {
                let v = c.value.unwrap_or(0.0);
                if let Some(x) = t
                    .receivers
                    .iter()
                    .find(|x| Some(&x.id) == c.receiver.as_ref())
                {
                    if let Some(d) = x.attack_max_dev_ns {
                        judged = true;
                        failed |= d > v;
                        worst = Some(worst.map_or(d, |w: f64| w.max(d)));
                        basis.push(format!(
                            "{}: alternative source '{}' deviated at most {d} ns during the attack",
                            chk.id, x.id
                        ));
                    }
                }
            }
            _ => {}
        }
        if (judged, detection, worst) != before && signed {
            signed_basis = true;
        }
    }
    let (verdict, m) = match c.kind {
        ClaimKind::SpoofDetection | ClaimKind::JamDetection => {
            let min_rate = c.min_rate.unwrap_or(0.9);
            let min_runs = c.min_runs.unwrap_or(5);
            match detection {
                Some((h, n)) if n >= min_runs => {
                    let rate = h as f64 / n as f64;
                    ci = wilson95(h, n).map(|(a, b)| (sig9(a), sig9(b)));
                    (
                        if rate >= min_rate {
                            "supported"
                        } else {
                            "not supported"
                        },
                        Some(rate),
                    )
                }
                Some((h, n)) => {
                    basis.push(format!("{n} run(s) judged; {min_runs} needed"));
                    ("insufficient evidence", Some(h as f64 / n as f64))
                }
                None => ("insufficient evidence", None),
            }
        }
        _ => match (judged, failed) {
            (true, true) => ("not supported", worst),
            (true, false) => ("supported", worst),
            _ => ("insufficient evidence", None),
        },
    };
    let measured = m.filter(|x| x.is_finite()).map(sig9);
    let verdict = if verdict == "supported" && !signed_basis {
        basis.push("only unsigned evidence: not enough for supported".into());
        "insufficient evidence"
    } else {
        verdict
    };
    if basis.is_empty() {
        basis.push("no matching evidence".into());
    }
    ClaimResult {
        id: c.id.clone(),
        text: c.text.clone(),
        kind: c.kind,
        verdict: verdict.into(),
        measured,
        ci95: ci,
        basis,
    }
}

/// Check every claim.
pub fn check(r: &Request, src: &str, trust: &[[u8; 32]]) -> Result<Scorecard, String> {
    let ev = load(r, trust)?;
    Ok(check_loaded(r, src, &ev))
}

/// Check every claim against loaded evidence.
pub fn check_loaded(r: &Request, _src: &str, ev: &[(EvidenceCheck, Option<Loaded>)]) -> Scorecard {
    let claims: Vec<ClaimResult> = r
        .claims
        .iter()
        .map(|c| check_claim(c, &r.device, ev))
        .collect();
    let subject_paid = matches!(r.funding.paid_by.as_str(), "subject" | "seller");
    let weak = ev.iter().any(|(e, _)| e.status != "signed");
    let publishable = !subject_paid && !weak;
    let mut note = vec![];
    if subject_paid {
        note.push(format!(
            "Testing paid for by the {}: under the method this scorecard is not published as a neutral scorecard; it remains a record for the parties.",
            if r.funding.paid_by == "subject" {
                "device's maker"
            } else {
                "device's seller"
            }
        ));
    }
    if weak {
        note.push("Some evidence is unsigned or was set aside: not publishable until every source verifies.".into());
    }
    if note.is_empty() {
        note.push("Publishable under the method: independent funding and signed evidence.".into());
    }
    let n = |v: &str| claims.iter().filter(|c| c.verdict == v).count();
    Scorecard {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        method_text: METHOD.into(),
        evidence_label: EVIDENCE_LABEL.into(),
        statement: STATEMENT.into(),
        title: r.title.clone(),
        scheme: r.scheme.clone(),
        device: r.device.clone(),
        funding: r.funding.clone(),
        publishable,
        publication_note: note.join(" "),
        evidence: ev.iter().map(|(e, _)| e.clone()).collect(),
        supported: n("supported"),
        not_supported: n("not supported"),
        insufficient: n("insufficient evidence"),
        claims,
        limits: vec![
            "Claims are checked against the evidence supplied only; a device's behaviour outside the tests run is not covered.".into(),
            "Detection rates pool every run of the claimed threat type; per-class rates are in the flag-efficacy scorecard.".into(),
            "MODELLED: the method is checked on synthetic scorecards; no fielded device is assessed in this release.".into(),
        ],
    }
}

/// Write `scheme-device-scorecard.json`, `request.toml` and `README.txt` (sign afterwards).
pub fn write_dir(dir: &Path, s: &Scorecard, src: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(s).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("request.toml"), src).map_err(io)?;
    let mut t = format!(
        "Discount-scheme device scorecard ({FORMAT}, method {METHOD_ID})\n{}\nScheme: {}\nDevice: {} {} {}\nPublishable: {} ({})\n\n{STATEMENT}\n\nMethod: {METHOD}\n\n",
        s.title,
        s.scheme,
        s.device.maker,
        s.device.model,
        s.device.firmware.clone().unwrap_or_default(),
        if s.publishable { "yes" } else { "no" },
        s.publication_note
    );
    for c in &s.claims {
        t += &format!("{} ({:?}): {} — \"{}\"\n", c.id, c.kind, c.verdict, c.text);
    }
    t += &format!("\nEvidence: {EVIDENCE_LABEL}\n");
    std::fs::write(dir.join("README.txt"), t).map_err(io)?;
    Ok(())
}
