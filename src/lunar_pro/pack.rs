// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D8. Lunar mission-review evidence pack (gap 24)
//!
//! A signed pack of navigation evidence for a mission navigation review (PDR, CDR, FRR): the
//! disclosure that CGPM 2026 Resolution 6 asks of anyone who measures a lunar time realisation
//! against UTC — "describe in full the model used to determine the relativistic effects, the
//! input ephemerides, and the associated uncertainties" (research note 03 §0 item 2, source [7]) —
//! plus the mission's navigation performance with uncertainty and the conformance verdicts.
//!
//! The pack is a directory:
//! * `pack.json`: everything below, machine-readable;
//! * `pack.txt`: the same for a reviewer;
//! * `time_budget.json`: the D1 conversion UTC → TL or TCL at the reference epoch, every term
//!   with its value, uncertainty, type and source, and the secular rates;
//! * `attachments/`: copies of attached evidence (for example a D6 score or a D7 SLA report,
//!   whose verdicts are imported into the conformance table);
//! * `signature.json`: the Ed25519 dossier signature over every file ([`crate::signed_dossier`]).
//!
//! **Contents.** (1) The time scale (TL or TCL), the rate source, the `W0_L` candidate, the
//! relativistic model (D1's terms) and its uncertainty; (2) the ephemerides and frames, each with
//! a version and a SHA-256 (computed from the file when one is given; a declared hash that does
//! not match the file is refused; an entry with neither is marked unverified), plus the open
//! engine's built-in models and the engine version; (3) navigation performance results, each
//! `value ± k·u` with its evidence label; (4) conformance verdicts (**Meets**, **Breach**,
//! **Inconclusive**: the whole `k·u` interval against the requirement); (5) a disclosure
//! checklist, each item *provided* or *missing*, derived from the content, not typed in.
//!
//! **Verification** ([`verify_pack`]): the signature (and the pinned key), every attachment's hash
//! against `pack.json`, the checklist recomputed from the content, and the time conversion
//! recomputed from the declared model, so a pack edited and re-signed under another key with
//! changed numbers is also caught.
//!
//! **Evidence label: MODELLED.** The pack format is a Kshana proposal; no agency has adopted it,
//! and the checklist's mapping to CGPM Resolution 6 is **illustrative until confirmed against the
//! resolution text**. A pack carries whatever evidence label its inputs have.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::lugre::sha256_hex;
use super::time;

/// The pack format tag.
pub const FORMAT: &str = "kshana-lunar-review-pack/1";
/// The open engine this build uses (kept in step with `Cargo.toml`, checked by a test).
pub const ENGINE: &str = "kshana 0.41.0 (crates.io; Cargo.toml pin =0.41.0)";
/// The statement every pack carries.
pub const STATUS: &str = "Kshana Pro D8 lunar mission-review evidence pack (format proposal, not \
    an agency standard); the CGPM Res. 6 checklist mapping is illustrative until confirmed \
    against the resolution text";

/// The engine's built-in models a pack built here relies on.
pub const BUILTIN_MODELS: [(&str, &str); 6] = [
    ("Moon position", "Montenbruck & Gill low-precision lunar series (kshana::ephem), ~0.05 deg vs JPL Horizons"),
    ("Sun position", "Montenbruck & Gill low-precision solar series (kshana::ephem)"),
    ("Lunar orientation", "IAU 2015 mean-Earth series + DE421 ME->PA rotation (kshana::lunar_frame)"),
    ("Precession", "IAU 2006 Fukushima-Williams (kshana::precession)"),
    ("Leap seconds", "engine table (kshana::timescales); a leap second announced after the build is not included"),
    ("TDB - TT", "USNO Circular 179 eq. 2.6 (Kshana Pro D1)"),
];

// ------------------------------------------------------------------------------ spec

/// The time-scale part of the spec.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSpec {
    /// `tl` or `tcl`.
    pub option: String,
    #[serde(default = "d_w0")]
    pub w0: String,
    #[serde(default = "d_rs")]
    pub rate_source: String,
    /// The review's reference epoch (UTC, `YYYY-MM-DDTHH:MM:SS`).
    pub reference_epoch: String,
    /// How the mission's lunar time is traced to UTC (route, laboratory, link).
    #[serde(default)]
    pub utc_traceability: String,
}
fn d_w0() -> String {
    "kshana-equatorial-j2".into()
}
fn d_rs() -> String {
    "lte440".into()
}

/// A frame used by the mission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSpec {
    pub name: String,
    pub realisation: String,
    #[serde(default)]
    pub source: String,
}

/// An ephemeris or other model input (kernel, SP3, gravity field).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EphemerisSpec {
    pub name: String,
    pub version: String,
    /// A local file to hash (relative to the spec); not copied into the pack.
    #[serde(default)]
    pub file: Option<String>,
    /// A declared SHA-256 (must match the file when both are given).
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub source: String,
}

/// A navigation performance result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceSpec {
    pub id: String,
    pub quantity: String,
    pub value: f64,
    /// Standard uncertainty (same unit).
    pub u: f64,
    #[serde(default = "d_k")]
    pub k: f64,
    pub unit: String,
    /// `le <x>` or `ge <x>` (optional).
    #[serde(default)]
    pub requirement: Option<String>,
    /// `MODELLED` or `VALIDATED`.
    pub evidence_label: String,
    #[serde(default)]
    pub source: String,
}
fn d_k() -> f64 {
    2.0
}

/// An attachment copied into the pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentSpec {
    pub path: String,
    pub role: String,
}

/// The pack spec (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSpec {
    pub mission: String,
    /// `PDR`, `CDR`, `FRR` (or another review name).
    pub review: String,
    pub baseline: String,
    pub prepared_by: String,
    pub time: TimeSpec,
    #[serde(default)]
    pub frame: Vec<FrameSpec>,
    #[serde(default)]
    pub ephemeris: Vec<EphemerisSpec>,
    #[serde(default)]
    pub performance: Vec<PerformanceSpec>,
    #[serde(default)]
    pub attachment: Vec<AttachmentSpec>,
}

// ------------------------------------------------------------------------------ pack content

/// An ephemeris entry with its hash status.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EphemerisEntry {
    pub name: String,
    pub version: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    /// `computed` (from the file), `declared` (no file at hand) or `unverified` (neither).
    pub hash_status: String,
    pub source: String,
}

/// A conformance verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conformance {
    pub id: String,
    pub requirement: String,
    pub measured: String,
    pub verdict: String,
    pub evidence_label: String,
    pub source: String,
}

/// A checklist item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckItem {
    pub id: String,
    pub item: String,
    pub basis: String,
    pub provided: bool,
    pub detail: String,
}

/// An attachment in the pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachmentEntry {
    pub file: String,
    pub role: String,
    pub sha256: String,
    pub kind: String,
}

/// The time disclosure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeDisclosure {
    pub scale: String,
    pub cgpm_option: String,
    pub rate_source: String,
    pub w0_name: String,
    pub w0_m2_s2: f64,
    pub w0_u_m2_s2: f64,
    pub w0_source: String,
    pub reference_epoch_utc: String,
    /// Lunar time − UTC at the reference epoch (s), u_c and U(k=2) (s).
    pub offset_s: f64,
    pub u_c_s: f64,
    pub expanded_k2_s: f64,
    /// d(scale − TT)/dt (µs/day) and its u (ns/day).
    pub rate_vs_tt_us_day: f64,
    pub rate_u_ns_day: f64,
    pub model_terms: Vec<String>,
    pub utc_traceability: String,
}

/// The pack (`pack.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pack {
    pub format: String,
    pub status: String,
    pub mission: String,
    pub review: String,
    pub baseline: String,
    pub prepared_by: String,
    pub engine: String,
    pub kshana_pro: String,
    pub time: TimeDisclosure,
    pub frames: Vec<FrameSpec>,
    pub ephemerides: Vec<EphemerisEntry>,
    pub builtin_models: Vec<(String, String)>,
    pub performance: Vec<PerformanceSpec>,
    pub conformance: Vec<Conformance>,
    pub attachments: Vec<AttachmentEntry>,
    pub checklist: Vec<CheckItem>,
    pub complete: bool,
}

fn parse_requirement(r: &str) -> Result<(String, f64), String> {
    let mut it = r.split_whitespace();
    match (
        it.next(),
        it.next().and_then(|v| v.parse::<f64>().ok()),
        it.next(),
    ) {
        (Some(op @ ("le" | "ge")), Some(v), None) if v.is_finite() => Ok((op.into(), v)),
        _ => Err(format!("requirement '{r}' is not 'le <x>' or 'ge <x>'")),
    }
}

/// Verdict of `value ± k·u` against a requirement.
pub fn verdict(value: f64, u: f64, k: f64, op: &str, limit: f64) -> &'static str {
    let (lo, hi) = (value - k * u, value + k * u);
    let (inside, outside) = if op == "le" {
        (hi <= limit, lo > limit)
    } else {
        (lo >= limit, hi < limit)
    };
    if inside {
        "Meets"
    } else if outside {
        "Breach"
    } else {
        "Inconclusive"
    }
}

fn model_of(t: &TimeSpec) -> Result<(time::Model, time::LunarOption), String> {
    let opt = time::LunarOption::parse(&t.option)?;
    let m = time::Model::new(
        time::RateSource::parse(&t.rate_source)?,
        time::W0Candidate::parse(&t.w0)?,
    );
    Ok((m, opt))
}

/// The time disclosure and the full D1 budget for a time spec.
pub fn time_disclosure(t: &TimeSpec) -> Result<(TimeDisclosure, serde_json::Value), String> {
    let (m, opt) = model_of(t)?;
    let jd = time::parse_iso(&t.reference_epoch)?;
    let off = m.lunar_minus_utc(opt, jd);
    let rate = m.secular_rate(opt.scale(), time::Scale::Tt);
    let conv = m.convert(time::Scale::Utc, jd, opt.scale());
    let d = TimeDisclosure {
        scale: opt.scale().as_str().into(),
        cgpm_option: match opt.scale() {
            time::Scale::Tl => {
                "TL: scaled to the lunar equipotential W0_L (the NASA/JAXA option)".into()
            }
            _ => "TCL: unscaled lunar coordinate time (the ESA/CCTF option)".into(),
        },
        rate_source: t.rate_source.clone(),
        w0_name: m.w0.name.clone(),
        w0_m2_s2: m.w0.w0_m2_s2,
        w0_u_m2_s2: m.w0.u_m2_s2,
        w0_source: m.w0.source.clone(),
        reference_epoch_utc: t.reference_epoch.clone(),
        offset_s: off.value(),
        u_c_s: off.u_c(),
        expanded_k2_s: off.expanded(2.0),
        rate_vs_tt_us_day: rate.value() * 86_400e6,
        rate_u_ns_day: rate.u_c() * 86_400e9,
        model_terms: off
            .terms
            .iter()
            .map(|x| {
                format!(
                    "{} = {:.12e} s (u {:.3e} s, {:?}; {})",
                    x.name,
                    x.value,
                    x.u.abs(),
                    x.kind,
                    x.source
                )
            })
            .collect(),
        utc_traceability: t.utc_traceability.clone(),
    };
    let budget = serde_json::json!({
        "label": super::LABEL,
        "conversion": conv,
        "secular_rate_vs_tt": rate,
        "w0": m.w0,
    });
    Ok((d, budget))
}

fn attachment_kind(text: &str) -> (String, Vec<Conformance>) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        return ("file".into(), Vec::new());
    };
    let mut out = Vec::new();
    if let Some(bars) = v
        .get("bars")
        .and_then(|b| b.as_array())
        .filter(|_| v.get("all_bars_met").is_some())
    {
        for b in bars {
            out.push(Conformance {
                id: format!("D6 {}", b["id"].as_str().unwrap_or("")),
                requirement: format!(
                    "{} {} {} {}",
                    b["signal"].as_str().unwrap_or(""),
                    b["quantity"].as_str().unwrap_or(""),
                    b["op"].as_str().unwrap_or(""),
                    b["value"]
                ),
                measured: format!("{}", b["observed"]),
                verdict: if b["met"].as_bool() == Some(true) {
                    "Meets"
                } else {
                    "Breach"
                }
                .into(),
                evidence_label: "MODELLED".into(),
                source: "attached D6 LuGRE score (pre-registered bar)".into(),
            });
        }
        return ("kshana-d6-lugre-score".into(), out);
    }
    if let Some(ps) = v
        .get("providers")
        .and_then(|p| p.as_array())
        .filter(|_| v.get("month").is_some())
    {
        for p in ps {
            for r in p["results"].as_array().into_iter().flatten() {
                out.push(Conformance {
                    id: format!(
                        "D7 {} {}",
                        p["provider"].as_str().unwrap_or(""),
                        r["sla"].as_str().unwrap_or("")
                    ),
                    requirement: format!(
                        "SLA {} ({})",
                        r["sla"].as_str().unwrap_or(""),
                        r["source"].as_str().unwrap_or("")
                    ),
                    measured: format!(
                        "SISE {} m, availability {}",
                        r["sise"]["value"]
                            .as_f64()
                            .map(|x| format!("{x:.2}"))
                            .unwrap_or("-".into()),
                        r["availability"]
                            .as_f64()
                            .map(|x| format!("{x:.4}"))
                            .unwrap_or("-".into())
                    ),
                    verdict: r["verdict"].as_str().unwrap_or("NotAssessed").into(),
                    evidence_label: "MODELLED".into(),
                    source: format!(
                        "attached D7 SLA report {}",
                        v["month"].as_str().unwrap_or("")
                    ),
                });
            }
        }
        return ("kshana-d7-sla-report".into(), out);
    }
    ("json".into(), out)
}

fn checklist(p: &Pack) -> Vec<CheckItem> {
    let t = &p.time;
    let item = |id: &str, item: &str, basis: &str, ok: bool, detail: String| CheckItem {
        id: id.into(),
        item: item.into(),
        basis: basis.into(),
        provided: ok,
        detail,
    };
    let cg = "CGPM 2026 Res. 6 (as cited in research note 03 [7]; illustrative)";
    vec![
        item(
            "C1",
            "Lunar time scale and CGPM option stated (TCL or TL)",
            cg,
            !t.scale.is_empty(),
            t.cgpm_option.clone(),
        ),
        item(
            "C2",
            "Relativistic model described in full (every term, its source)",
            cg,
            !t.model_terms.is_empty(),
            format!(
                "{} terms; rate source {}",
                t.model_terms.len(),
                t.rate_source
            ),
        ),
        item(
            "C3",
            "W0_L stated when TL is used",
            cg,
            t.scale != "TL" || t.w0_m2_s2 > 0.0,
            format!("{} = {} m2/s2 (u {})", t.w0_name, t.w0_m2_s2, t.w0_u_m2_s2),
        ),
        item(
            "C4",
            "Time uncertainties given",
            cg,
            t.u_c_s.is_finite() && t.u_c_s >= 0.0,
            format!("u_c {:.3e} s at {}", t.u_c_s, t.reference_epoch_utc),
        ),
        item(
            "C5",
            "Traceability route to UTC stated",
            cg,
            !t.utc_traceability.trim().is_empty(),
            t.utc_traceability.clone(),
        ),
        item(
            "C6",
            "Input ephemerides named with versions",
            cg,
            !p.ephemerides.is_empty(),
            format!("{} entries", p.ephemerides.len()),
        ),
        item(
            "C7",
            "Every input ephemeris pinned by a computed SHA-256",
            "Kshana evidence practice",
            !p.ephemerides.is_empty() && p.ephemerides.iter().all(|e| e.hash_status == "computed"),
            p.ephemerides
                .iter()
                .map(|e| format!("{}: {}", e.name, e.hash_status))
                .collect::<Vec<_>>()
                .join("; "),
        ),
        item(
            "C8",
            "Reference frames named with their realisation",
            "research note 03 gap 7",
            !p.frames.is_empty(),
            p.frames
                .iter()
                .map(|f| f.name.clone())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        item(
            "C9",
            "Navigation performance results with uncertainty",
            "review practice",
            !p.performance.is_empty() && p.performance.iter().all(|x| x.u.is_finite() && x.u > 0.0),
            format!("{} results", p.performance.len()),
        ),
        item(
            "C10",
            "Every result carries an evidence label (MODELLED or VALIDATED)",
            "Kshana evidence practice",
            p.performance
                .iter()
                .all(|x| x.evidence_label == "MODELLED" || x.evidence_label == "VALIDATED"),
            String::new(),
        ),
        item(
            "C11",
            "Conformance verdicts against requirements",
            "review practice",
            !p.conformance.is_empty(),
            format!("{} verdicts", p.conformance.len()),
        ),
        item(
            "C12",
            "Engine and tool versions and built-in models stated",
            "Kshana evidence practice",
            !p.engine.is_empty() && !p.builtin_models.is_empty(),
            p.engine.clone(),
        ),
    ]
}

/// A built pack: content, the D1 budget, and attachment files (pack path, bytes).
pub type Built = (Pack, serde_json::Value, Vec<(String, Vec<u8>)>);

/// Build the pack content from a spec (paths relative to `base`).
pub fn build(spec: &PackSpec, base: &Path) -> Result<Built, String> {
    for (n, v) in [
        ("mission", &spec.mission),
        ("review", &spec.review),
        ("baseline", &spec.baseline),
        ("prepared_by", &spec.prepared_by),
    ] {
        if v.trim().is_empty() {
            return Err(format!("{n} is empty"));
        }
    }
    let (time, budget) = time_disclosure(&spec.time)?;
    let mut ephemerides = Vec::new();
    for e in &spec.ephemeris {
        let (sha, bytes, status) = match &e.file {
            Some(f) => {
                let p = base.join(f);
                let b = std::fs::read(&p)
                    .map_err(|x| format!("ephemeris {}: {}: {x}", e.name, p.display()))?;
                let h = sha256_hex(&b);
                if let Some(d) = &e.sha256 {
                    if !d.eq_ignore_ascii_case(&h) {
                        return Err(format!(
                            "ephemeris {}: file SHA-256 {h} is not the declared {d}",
                            e.name
                        ));
                    }
                }
                (Some(h), Some(b.len() as u64), "computed")
            }
            None => match &e.sha256 {
                Some(d) => (Some(d.to_ascii_lowercase()), None, "declared"),
                None => (None, None, "unverified"),
            },
        };
        ephemerides.push(EphemerisEntry {
            name: e.name.clone(),
            version: e.version.clone(),
            sha256: sha,
            bytes,
            hash_status: status.into(),
            source: e.source.clone(),
        });
    }
    let mut conformance = Vec::new();
    for x in &spec.performance {
        if !(x.value.is_finite() && x.u.is_finite() && x.u >= 0.0 && x.k > 0.0) {
            return Err(format!(
                "performance {}: value, u >= 0 and k > 0 must be finite",
                x.id
            ));
        }
        if let Some(r) = &x.requirement {
            let (op, lim) = parse_requirement(r)?;
            conformance.push(Conformance {
                id: x.id.clone(),
                requirement: format!("{} {} {} {}", x.quantity, op, lim, x.unit),
                measured: format!("{} ± {} (k={}) {}", x.value, x.k * x.u, x.k, x.unit),
                verdict: verdict(x.value, x.u, x.k, &op, lim).into(),
                evidence_label: x.evidence_label.clone(),
                source: x.source.clone(),
            });
        }
    }
    let mut attachments = Vec::new();
    let mut files = Vec::new();
    for (i, a) in spec.attachment.iter().enumerate() {
        let p = base.join(&a.path);
        let b = std::fs::read(&p).map_err(|x| format!("attachment {}: {x}", p.display()))?;
        let stem = Path::new(&a.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let name = format!("attachments/{:02}-{stem}", i + 1);
        let (kind, verdicts) = attachment_kind(std::str::from_utf8(&b).unwrap_or(""));
        conformance.extend(verdicts);
        attachments.push(AttachmentEntry {
            file: name.clone(),
            role: a.role.clone(),
            sha256: sha256_hex(&b),
            kind,
        });
        files.push((name, b));
    }
    let mut pack = Pack {
        format: FORMAT.into(),
        status: STATUS.into(),
        mission: spec.mission.clone(),
        review: spec.review.clone(),
        baseline: spec.baseline.clone(),
        prepared_by: spec.prepared_by.clone(),
        engine: ENGINE.into(),
        kshana_pro: env!("CARGO_PKG_VERSION").into(),
        time,
        frames: spec.frame.clone(),
        ephemerides,
        builtin_models: BUILTIN_MODELS
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
        performance: spec.performance.clone(),
        conformance,
        attachments,
        checklist: Vec::new(),
        complete: false,
    };
    pack.checklist = checklist(&pack);
    pack.complete = pack.checklist.iter().all(|c| c.provided);
    Ok((pack, budget, files))
}

/// Human-readable pack.
pub fn render_text(p: &Pack) -> String {
    let t = &p.time;
    let mut s = format!(
        "{}\n\nLUNAR MISSION NAVIGATION REVIEW EVIDENCE PACK\nmission {} | review {} | baseline {} | prepared by {}\nengine {} | Kshana Pro {}\n\n",
        p.status, p.mission, p.review, p.baseline, p.prepared_by, p.engine, p.kshana_pro
    );
    s.push_str(&format!(
        "1. TIME SCALE\n  {} ({})\n  rate source {}; W0_L {} = {} m2/s2 (u {}), {}\n  {} - UTC at {} = {:.12} s, u_c {:.3e} s, U(k=2) {:.3e} s\n  d({} - TT)/dt = {:.6} us/day (u {:.4} ns/day)\n  UTC traceability: {}\n  model terms:\n",
        t.scale, t.cgpm_option, t.rate_source, t.w0_name, t.w0_m2_s2, t.w0_u_m2_s2, t.w0_source, t.scale,
        t.reference_epoch_utc, t.offset_s, t.u_c_s, t.expanded_k2_s, t.scale, t.rate_vs_tt_us_day, t.rate_u_ns_day,
        if t.utc_traceability.is_empty() { "(not stated)" } else { &t.utc_traceability }
    ));
    for m in &t.model_terms {
        s.push_str(&format!("    {m}\n"));
    }
    s.push_str("\n2. EPHEMERIDES AND FRAMES\n");
    for e in &p.ephemerides {
        s.push_str(&format!(
            "  {} {} sha256 {} [{}] {}\n",
            e.name,
            e.version,
            e.sha256.as_deref().unwrap_or("-"),
            e.hash_status,
            e.source
        ));
    }
    for f in &p.frames {
        s.push_str(&format!(
            "  frame {}: {} {}\n",
            f.name, f.realisation, f.source
        ));
    }
    for (a, b) in &p.builtin_models {
        s.push_str(&format!("  built-in: {a}: {b}\n"));
    }
    s.push_str("\n3. NAVIGATION PERFORMANCE\n");
    for x in &p.performance {
        s.push_str(&format!(
            "  {} {}: {} ± {} (k={}) {} [{}] {}\n",
            x.id,
            x.quantity,
            x.value,
            x.k * x.u,
            x.k,
            x.unit,
            x.evidence_label,
            x.source
        ));
    }
    s.push_str("\n4. CONFORMANCE\n");
    for c in &p.conformance {
        s.push_str(&format!(
            "  {:<28} {:<14} {} | measured {} [{}]\n",
            c.id, c.verdict, c.requirement, c.measured, c.evidence_label
        ));
    }
    s.push_str("\n5. DISCLOSURE CHECKLIST\n");
    for c in &p.checklist {
        s.push_str(&format!(
            "  [{}] {} {} ({}){}\n",
            if c.provided { "x" } else { " " },
            c.id,
            c.item,
            c.basis,
            if c.detail.is_empty() {
                String::new()
            } else {
                format!(": {}", c.detail)
            }
        ));
    }
    s.push_str(&format!(
        "\npack complete: {}\n",
        if p.complete {
            "yes"
        } else {
            "NO (items missing)"
        }
    ));
    if !p.attachments.is_empty() {
        s.push_str("\nattachments:\n");
        for a in &p.attachments {
            s.push_str(&format!(
                "  {} ({}, {}) sha256 {}\n",
                a.file, a.role, a.kind, a.sha256
            ));
        }
    }
    s
}

/// Write the pack directory (unsigned; sign it afterwards). Refuses a non-empty directory.
pub fn write_pack(out: &Path, spec: &PackSpec, base: &Path) -> Result<Pack, String> {
    if out.exists()
        && std::fs::read_dir(out)
            .map(|mut d| d.next().is_some())
            .unwrap_or(false)
    {
        return Err(format!("{}: not empty", out.display()));
    }
    let (pack, budget, files) = build(spec, base)?;
    std::fs::create_dir_all(out.join("attachments")).map_err(|e| e.to_string())?;
    let w = |n: &str, b: &[u8]| std::fs::write(out.join(n), b).map_err(|e| format!("{n}: {e}"));
    w(
        "pack.json",
        (serde_json::to_string_pretty(&pack).map_err(|e| e.to_string())? + "\n").as_bytes(),
    )?;
    w("pack.txt", render_text(&pack).as_bytes())?;
    w(
        "time_budget.json",
        (serde_json::to_string_pretty(&budget).map_err(|e| e.to_string())? + "\n").as_bytes(),
    )?;
    for (n, b) in &files {
        w(n, b)?;
    }
    Ok(pack)
}

/// Parse a spec file and resolve its base directory.
pub fn read_spec(path: &Path) -> Result<(PackSpec, PathBuf), String> {
    let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let s: PackSpec = toml::from_str(&t).map_err(|e| format!("pack spec: {e}"))?;
    Ok((s, path.parent().unwrap_or(Path::new(".")).to_path_buf()))
}

/// The result of verifying a pack.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct PackVerification {
    pub signature_ok: bool,
    pub trusted: Option<bool>,
    pub signer: String,
    pub complete: bool,
    pub problems: Vec<String>,
}

impl PackVerification {
    pub fn passed(&self) -> bool {
        self.signature_ok && self.trusted == Some(true) && self.problems.is_empty()
    }
}

/// Verify a pack: signature and pinned key, attachment hashes, the checklist and the time
/// conversion recomputed from the content.
pub fn verify_pack(
    dir: &Path,
    trust: &[[u8; crate::license::KEY_LEN]],
) -> Result<PackVerification, String> {
    let sig = crate::signed_dossier::verify(dir, trust).map_err(|e| e.to_string())?;
    let mut v = PackVerification {
        signature_ok: sig.signature_ok,
        trusted: sig.trusted,
        signer: sig.signer.clone(),
        complete: false,
        problems: sig.problems.clone(),
    };
    let text =
        std::fs::read_to_string(dir.join("pack.json")).map_err(|e| format!("pack.json: {e}"))?;
    let p: Pack = serde_json::from_str(&text).map_err(|e| format!("pack.json: {e}"))?;
    if p.format != FORMAT {
        v.problems
            .push(format!("pack.json: format '{}' is not {FORMAT}", p.format));
    }
    for a in &p.attachments {
        match std::fs::read(dir.join(&a.file)) {
            Ok(b) if sha256_hex(&b) == a.sha256 => {}
            Ok(_) => v
                .problems
                .push(format!("{}: SHA-256 differs from pack.json", a.file)),
            Err(e) => v.problems.push(format!("{}: {e}", a.file)),
        }
    }
    if checklist(&p) != p.checklist {
        v.problems
            .push("checklist does not follow from the pack content".into());
    }
    if p.complete != p.checklist.iter().all(|c| c.provided) {
        v.problems
            .push("'complete' does not follow from the checklist".into());
    }
    let ts = TimeSpec {
        option: p.time.scale.to_ascii_lowercase(),
        w0: if p.time.w0_name.starts_with("conventional") {
            format!("conventional:{}", p.time.w0_m2_s2)
        } else {
            p.time.w0_name.clone()
        },
        rate_source: p.time.rate_source.clone(),
        reference_epoch: p.time.reference_epoch_utc.clone(),
        utc_traceability: p.time.utc_traceability.clone(),
    };
    match time_disclosure(&ts) {
        Ok((t, _)) => {
            if (t.offset_s - p.time.offset_s).abs() > 1e-12
                || (t.u_c_s - p.time.u_c_s).abs() > 1e-15
            {
                v.problems.push(format!(
                    "time: recomputed {} - UTC {:.12} s (u {:.3e}) differs from the pack's {:.12} s (u {:.3e})",
                    t.scale, t.offset_s, t.u_c_s, p.time.offset_s, p.time.u_c_s
                ));
            }
        }
        Err(e) => v.problems.push(format!("time: cannot recompute: {e}")),
    }
    for (c, x) in p
        .conformance
        .iter()
        .zip(p.performance.iter().filter(|x| x.requirement.is_some()))
    {
        if let Some(Ok((op, lim))) = x.requirement.as_deref().map(parse_requirement) {
            if c.id == x.id && c.verdict != verdict(x.value, x.u, x.k, &op, lim) {
                v.problems.push(format!(
                    "conformance {}: verdict does not follow from the result",
                    c.id
                ));
            }
        }
    }
    v.complete = p.complete;
    Ok(v)
}

/// A complete example spec (SYNTHETIC numbers) written into `dir` with a small stand-in
/// ephemeris file; returns the spec path.
pub fn write_example(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join("ephemeris-standin.bsp"),
        b"SYNTHETIC stand-in for a SPICE kernel; replace with the mission's file\n",
    )
    .map_err(|e| e.to_string())?;
    let spec = "mission = \"SYNTHETIC lander (example)\"\nreview = \"CDR\"\nbaseline = \"NAV-BL-0.3 (example)\"\nprepared_by = \"Kshana Pro example\"\n\n\
        [time]\noption = \"tl\"\nw0 = \"kshana-equatorial-j2\"\nrate_source = \"lte440\"\nreference_epoch = \"2027-03-01T00:00:00\"\n\
        utc_traceability = \"UTC(k) ground laboratory -> Earth-Moon TWTT link -> lander clock (example)\"\n\n\
        [[frame]]\nname = \"MOON_PA_DE440\"\nrealisation = \"DE440 lunar principal axes\"\nsource = \"NAIF\"\n\n\
        [[ephemeris]]\nname = \"Planetary and lunar ephemeris\"\nversion = \"DE440 (stand-in file)\"\nfile = \"ephemeris-standin.bsp\"\nsource = \"JPL NAIF\"\n\n\
        [[performance]]\nid = \"P1\"\nquantity = \"landing horizontal position error (95%)\"\nvalue = 62.0\nu = 9.0\nunit = \"m\"\nrequirement = \"le 100\"\nevidence_label = \"MODELLED\"\nsource = \"covariance analysis (example)\"\n\n\
        [[performance]]\nid = \"P2\"\nquantity = \"onboard time error vs TL at 24 h holdover\"\nvalue = 40.0\nu = 12.0\nunit = \"ns\"\nrequirement = \"le 50\"\nevidence_label = \"MODELLED\"\nsource = \"D3 holdover calculator (example)\"\n";
    let p = dir.join("pack.toml");
    std::fs::write(&p, spec).map_err(|e| e.to_string())?;
    Ok(p)
}
