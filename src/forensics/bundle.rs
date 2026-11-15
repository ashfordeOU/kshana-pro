// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The signed report bundle, the expert review, and verification.
//!
//! ```text
//! <bundle>/
//!   report.json  report.html  report.pdf        the report (JSON is the record)
//!   findings.csv  timeline.csv  timeline.svg     tables and the timeline figure
//!   area.geojson  tracks.geojson                 the affected area (MODELLED) and the tracks
//!   recipe.toml                                  the incident with paths into inputs/
//!   inputs/<file>                                every input, byte for byte
//!   evidence/<n>-<receiver>/                     an open-engine evidence pack per receiver log
//!   review/expert-review.json                    claim-grade only, added by the reviewer
//!   signature.json                               Ed25519 over every file's SHA-256
//! ```
//!
//! Verification ([`verify_bundle`]) needs no licence: it checks the bundle signature, that
//! every input the report names is in the bundle with the hash the report states, that each
//! evidence pack verifies under the bundle's signing key, and the expert review's signature
//! and the report hash it was given for.

use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};

use super::detect::{analyse_with_inputs, settle_status, ForensicReport, MethodParams};
use super::inputs::{sha256_hex, InputRole};
use super::render;
use super::IncidentScenario;
use crate::license::{key_id, public_key_for, KEY_LEN};
use crate::signed_dossier;

/// The review file inside a bundle.
pub const REVIEW_FILE: &str = "review/expert-review.json";
/// Its format tag.
pub const REVIEW_FORMAT: &str = "kshana-pro-forensic-review/1";

fn io(p: &Path, e: impl std::fmt::Display) -> KshanaError {
    KshanaError::InvalidInput(format!("{}: {e}", p.display()))
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), KshanaError> {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    std::fs::write(&p, bytes).map_err(|e| io(&p, e))
}

fn slug(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').chars().take(40).collect()
}

/// What [`build_bundle`] was asked to do.
pub struct BundleRequest<'a> {
    pub scenario: &'a IncidentScenario,
    pub out: &'a Path,
    /// The report's time stamp and the signature's time (ISO-8601 UTC).
    pub utc: &'a str,
    /// The issuer's Ed25519 seed.
    pub seed: &'a [u8; KEY_LEN],
    /// The issuer's name in the signature.
    pub signer: &'a str,
}

/// What was written.
#[derive(Debug)]
pub struct BundleOutcome {
    pub dir: PathBuf,
    pub report: ForensicReport,
    pub signature: PathBuf,
}

/// The recipe: the incident with every path pointing into `inputs/`.
pub fn recipe_toml(scn: &IncidentScenario, report: &ForensicReport) -> Result<String, KshanaError> {
    let file_of = |role: InputRole, label: &str| -> String {
        report
            .inputs
            .iter()
            .find(|i| i.role == role && i.label == label)
            .map(|i| format!("inputs/{}", i.file))
            .unwrap_or_default()
    };
    let mut r = scn.clone();
    for x in &mut r.receivers {
        x.path = file_of(InputRole::ReceiverLog, &x.label);
        if x.nav.is_some() {
            x.nav = Some(file_of(
                InputRole::RinexNav,
                &format!("{} (navigation)", x.label),
            ));
        }
    }
    for x in &mut r.ais {
        x.path = file_of(InputRole::Ais, &x.label);
    }
    for x in &mut r.adsb {
        x.path = file_of(InputRole::Adsb, &x.label);
    }
    r.base_dir = None;
    let body =
        toml::to_string(&r).map_err(|e| KshanaError::InvalidInput(format!("recipe: {e}")))?;
    Ok(format!(
        "# Recipe of forensic report {} ({}). Reproduce from this folder:\n#   {}\n{}",
        report.incident.id, report.method_id, report.reproduce.command, body
    ))
}

/// Run the method and write the signed bundle into `req.out` (which must be empty or absent).
pub fn build_bundle(req: &BundleRequest<'_>) -> Result<BundleOutcome, KshanaError> {
    let out = req.out;
    if out.exists()
        && std::fs::read_dir(out)
            .map_err(|e| io(out, e))?
            .next()
            .is_some()
    {
        return Err(io(out, "the output folder is not empty"));
    }
    std::fs::create_dir_all(out).map_err(|e| io(out, e))?;
    let (inputs, mut report) = analyse_with_inputs(req.scenario, Some(req.utc))?;

    // Inputs, byte for byte.
    for i in &inputs {
        write(out, &format!("inputs/{}", i.record.file), &i.data)?;
    }
    // An open-engine evidence pack per receiver log, for the incident window.
    let p = MethodParams::V1;
    let mut packs_ok = true;
    let mut pack_notes = Vec::new();
    for (n, r) in req.scenario.receivers.iter().enumerate() {
        let log = inputs
            .iter()
            .find(|i| i.record.role == InputRole::ReceiverLog && i.record.label == r.label);
        let nav = inputs.iter().find(|i| {
            i.record.role == InputRole::RinexNav
                && i.record.label == format!("{} (navigation)", r.label)
        });
        let Some(log) = log else { continue };
        let scn = super::detect::trust_scenario_for(r, &p)?;
        let title = format!("{}: {}", report.incident.id, r.label);
        let pr = kshana::evidence::PackRequest {
            scenario: &scn,
            log_bytes: &log.data,
            nav_bytes: nav.map(|x| x.data.as_slice()),
            imu_bytes: None,
            log_file_name: &log.record.file,
            from: &report.incident.start_utc,
            to: &report.incident.end_utc,
            title: &title,
            created_utc: None,
        };
        let dir = format!("evidence/{}-{}", n + 1, slug(&r.label));
        match kshana::evidence::build_receiver_trust_pack(&pr, req.seed, None) {
            Ok(pack) => {
                for (name, bytes) in &pack.files {
                    write(out, &format!("{dir}/{name}"), bytes)?;
                }
                pack_notes.push(format!("{dir} ({} epochs)", pack.epochs_in_window));
            }
            Err(e) => {
                packs_ok = false;
                pack_notes.push(format!("{}: not made ({e})", r.label));
            }
        }
    }
    if let Some(c) = report.checklist.iter_mut().find(|c| c.id == "C9") {
        if req.scenario.receivers.is_empty() {
            c.result = "pass";
            c.detail = "no receiver log in this incident".into();
        } else {
            c.result = if packs_ok { "pass" } else { "fail" };
            c.detail = pack_notes.join("; ");
        }
    }
    settle_status(&mut report);

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| KshanaError::InvalidInput(format!("report: {e}")))?
        + "\n";
    write(out, "report.json", json.as_bytes())?;
    write(out, "report.html", render::report_html(&report).as_bytes())?;
    write(
        out,
        "report.pdf",
        &render::report_pdf(&report).map_err(KshanaError::InvalidInput)?,
    )?;
    write(
        out,
        "findings.csv",
        render::findings_csv(&report).as_bytes(),
    )?;
    write(
        out,
        "timeline.csv",
        render::timeline_csv(&report).as_bytes(),
    )?;
    write(
        out,
        "timeline.svg",
        render::timeline_svg(&report).as_bytes(),
    )?;
    write(
        out,
        "area.geojson",
        render::area_geojson(&report).as_bytes(),
    )?;
    write(
        out,
        "tracks.geojson",
        render::tracks_geojson(&report).as_bytes(),
    )?;
    write(
        out,
        "recipe.toml",
        recipe_toml(req.scenario, &report)?.as_bytes(),
    )?;
    let signature = signed_dossier::sign(out, req.seed, req.signer, req.utc)?;
    Ok(BundleOutcome {
        dir: out.to_path_buf(),
        report,
        signature,
    })
}

// ─── Expert review (claim-grade) ──────────────────────────────────────────────────────

/// The reviewer's statement, as signed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewStatement {
    pub format: String,
    pub case_id: String,
    /// SHA-256 of the bundle's `report.json` the review was given for.
    pub report_sha256: String,
    pub reviewer_name: String,
    pub qualification: String,
    pub organisation: String,
    pub independence_statement: String,
    /// `concur`, `concur-with-reservations` or `do-not-concur`.
    pub decision: String,
    pub statement: String,
    pub reviewed_utc: String,
    pub reviewer_public_key: String,
    pub reviewer_key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReviewFile {
    #[serde(flatten)]
    statement: ReviewStatement,
    signature: String,
}

/// What a reviewer gives.
pub struct ReviewInput<'a> {
    pub reviewer_name: &'a str,
    pub qualification: &'a str,
    pub organisation: &'a str,
    pub independence_statement: &'a str,
    pub decision: &'a str,
    pub statement: &'a str,
    pub reviewed_utc: &'a str,
    pub seed: &'a [u8; KEY_LEN],
}

/// Add a signed expert review to a claim-grade bundle. The bundle's own signature must hold
/// first (the reviewer reviews what the issuer signed). Afterwards the issuer re-signs the
/// bundle (`kshana-pro dossier sign`), so the bundle signature covers the review too.
pub fn add_review(dir: &Path, r: &ReviewInput<'_>) -> Result<PathBuf, KshanaError> {
    let v = signed_dossier::verify(dir, &[])?;
    if !v.signature_ok || !v.problems.is_empty() {
        return Err(KshanaError::InvalidInput(format!(
            "the bundle does not verify; review refused: {}",
            v.problems.join("; ")
        )));
    }
    let report_bytes = std::fs::read(dir.join("report.json")).map_err(|e| io(dir, e))?;
    let report: serde_json::Value = serde_json::from_slice(&report_bytes)
        .map_err(|e| KshanaError::InvalidInput(format!("report.json: {e}")))?;
    if report["variant"] != "claim-grade" {
        return Err(KshanaError::InvalidInput(
            "only a claim-grade report takes an expert review".into(),
        ));
    }
    if report["status"] != "awaiting-expert-review" {
        return Err(KshanaError::InvalidInput(format!(
            "the report's status is '{}': it does not meet the claim-grade checklist, so it cannot be reviewed as claim-grade",
            report["status"].as_str().unwrap_or("")
        )));
    }
    if dir.join(REVIEW_FILE).exists() {
        return Err(KshanaError::InvalidInput(
            "the bundle already has a review".into(),
        ));
    }
    for (name, v) in [
        ("reviewer name", r.reviewer_name),
        ("qualification", r.qualification),
        ("organisation", r.organisation),
        ("independence statement", r.independence_statement),
        ("statement", r.statement),
    ] {
        if v.trim().is_empty() {
            return Err(KshanaError::InvalidInput(format!(
                "the review needs a {name}"
            )));
        }
    }
    if !["concur", "concur-with-reservations", "do-not-concur"].contains(&r.decision) {
        return Err(KshanaError::InvalidInput(
            "decision must be concur, concur-with-reservations or do-not-concur".into(),
        ));
    }
    let pk = public_key_for(r.seed);
    let st = ReviewStatement {
        format: REVIEW_FORMAT.into(),
        case_id: report["incident"]["id"].as_str().unwrap_or("").into(),
        report_sha256: sha256_hex(&report_bytes),
        reviewer_name: r.reviewer_name.trim().into(),
        qualification: r.qualification.trim().into(),
        organisation: r.organisation.trim().into(),
        independence_statement: r.independence_statement.trim().into(),
        decision: r.decision.into(),
        statement: r.statement.trim().into(),
        reviewed_utc: r.reviewed_utc.into(),
        reviewer_public_key: hex::encode(pk),
        reviewer_key_id: key_id(&pk),
    };
    let msg = serde_json::to_vec(&st).map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    let sig = SigningKey::from_bytes(r.seed).sign(&msg);
    let file = ReviewFile {
        statement: st,
        signature: hex::encode(sig.to_bytes()),
    };
    let text = serde_json::to_string_pretty(&file)
        .map_err(|e| KshanaError::InvalidInput(e.to_string()))?
        + "\n";
    write(dir, REVIEW_FILE, text.as_bytes())?;
    Ok(dir.join(REVIEW_FILE))
}

// ─── Verification (free) ──────────────────────────────────────────────────────────────

/// The outcome of [`verify_bundle`].
#[derive(Debug, Clone, Default)]
pub struct BundleVerification {
    pub signer: String,
    pub signed_utc: String,
    pub key_id: String,
    pub files: usize,
    pub signature_ok: bool,
    /// The signing key is pinned with `--trust` (`None`: nothing pinned).
    pub trusted: Option<bool>,
    pub case_id: String,
    pub variant: String,
    pub status: String,
    pub inputs_checked: usize,
    pub evidence_packs_ok: usize,
    /// `None`: no review; `Some(true)`: signed, for this report, and (when pinned) trusted.
    pub review_ok: Option<bool>,
    pub reviewer: Option<String>,
    pub review_decision: Option<String>,
    pub problems: Vec<String>,
}

impl BundleVerification {
    /// Every check holds and the issuer key is pinned and trusted.
    pub fn passed(&self) -> bool {
        self.signature_ok
            && self.trusted == Some(true)
            && self.problems.is_empty()
            && self.review_ok != Some(false)
    }
    /// A claim-grade report with a valid signed review, in a bundle that passed.
    pub fn claim_grade_complete(&self) -> bool {
        self.passed() && self.variant == "claim-grade" && self.review_ok == Some(true)
    }
}

fn read_files(dir: &Path) -> Result<kshana::evidence::Files, KshanaError> {
    let mut out = kshana::evidence::Files::new();
    for e in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
        let e = e.map_err(|e| io(dir, e))?;
        if e.file_type().map_err(|x| io(dir, x))?.is_file() {
            let name = e.file_name().to_string_lossy().to_string();
            out.insert(name, std::fs::read(e.path()).map_err(|x| io(&e.path(), x))?);
        }
    }
    Ok(out)
}

/// Verify a bundle offline. `trust`: the issuer keys pinned; `reviewer_trust`: reviewer keys
/// pinned (empty: the review's own key is reported, not trusted).
pub fn verify_bundle(
    dir: &Path,
    trust: &[[u8; KEY_LEN]],
    reviewer_trust: &[[u8; KEY_LEN]],
) -> Result<BundleVerification, KshanaError> {
    let d = signed_dossier::verify(dir, trust)?;
    let mut v = BundleVerification {
        signer: d.signer,
        signed_utc: d.utc,
        key_id: d.key_id,
        files: d.files,
        signature_ok: d.signature_ok,
        trusted: d.trusted,
        problems: d.problems,
        ..Default::default()
    };
    let sig: serde_json::Value = serde_json::from_slice(
        &std::fs::read(dir.join(signed_dossier::SIGNATURE_FILE)).map_err(|e| io(dir, e))?,
    )
    .map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
    let issuer = sig["public_key"]
        .as_str()
        .and_then(|k| crate::license::parse_key_hex(k).ok());
    let report_bytes = match std::fs::read(dir.join("report.json")) {
        Ok(b) => b,
        Err(_) => {
            v.problems.push("report.json is missing".into());
            return Ok(v);
        }
    };
    let report: serde_json::Value = match serde_json::from_slice(&report_bytes) {
        Ok(r) => r,
        Err(e) => {
            v.problems.push(format!("report.json does not parse: {e}"));
            return Ok(v);
        }
    };
    if report["format"] != super::FORMAT {
        v.problems
            .push(format!("report.json is not a {} report", super::FORMAT));
    }
    v.case_id = report["incident"]["id"].as_str().unwrap_or("").into();
    v.variant = report["variant"].as_str().unwrap_or("").into();
    v.status = report["status"].as_str().unwrap_or("").into();
    for i in report["inputs"].as_array().into_iter().flatten() {
        let (Some(file), Some(want)) = (i["file"].as_str(), i["sha256"].as_str()) else {
            v.problems
                .push("an input record lacks its file or hash".into());
            continue;
        };
        match std::fs::read(dir.join("inputs").join(file)) {
            Ok(b) if sha256_hex(&b) == want => v.inputs_checked += 1,
            Ok(_) => v
                .problems
                .push(format!("inputs/{file}: hash differs from the report")),
            Err(_) => v
                .problems
                .push(format!("inputs/{file}: missing from the bundle")),
        }
    }
    let ev = dir.join("evidence");
    if ev.is_dir() {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&ev)
            .map_err(|e| io(&ev, e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort();
        for p in dirs {
            let files = read_files(&p)?;
            let rep = kshana::evidence::verify_bundle(
                &files,
                &kshana::evidence::VerifyOptions {
                    expected_public_key: issuer,
                    full_log: None,
                    require_timestamp: false,
                    tsa_anchors: None,
                },
            );
            if rep.ok {
                v.evidence_packs_ok += 1;
            } else {
                v.problems.push(format!(
                    "{}: open evidence pack does not verify ({:?})",
                    p.strip_prefix(dir).unwrap_or(&p).display(),
                    rep.failures
                ));
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string(dir.join(REVIEW_FILE)) {
        let ok = (|| -> Result<(bool, ReviewStatement), String> {
            let f: ReviewFile = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            let st = f.statement;
            let pk = crate::license::parse_key_hex(&st.reviewer_public_key)
                .map_err(|e| e.to_string())?;
            let vk = VerifyingKey::from_bytes(&pk).map_err(|e| e.to_string())?;
            let sb: [u8; 64] = hex::decode(&f.signature)
                .map_err(|e| e.to_string())?
                .try_into()
                .map_err(|_| "signature length".to_string())?;
            let msg = serde_json::to_vec(&st).map_err(|e| e.to_string())?;
            let sig_ok = vk.verify(&msg, &Signature::from_bytes(&sb)).is_ok();
            let mut ok = sig_ok && st.format == REVIEW_FORMAT;
            if !sig_ok {
                return Err("the review signature is invalid".into());
            }
            if st.report_sha256 != sha256_hex(&report_bytes) {
                return Err("the review was given for another report.json".into());
            }
            if !reviewer_trust.is_empty() && !reviewer_trust.contains(&pk) {
                ok = false;
            }
            Ok((ok, st))
        })();
        match ok {
            Ok((good, st)) => {
                if !good {
                    v.problems
                        .push("the reviewer key is not one of the pinned reviewer keys".into());
                }
                v.review_ok = Some(good);
                v.reviewer = Some(format!(
                    "{} ({}, key {})",
                    st.reviewer_name, st.organisation, st.reviewer_key_id
                ));
                v.review_decision = Some(st.decision);
            }
            Err(e) => {
                v.review_ok = Some(false);
                v.problems.push(format!("{REVIEW_FILE}: {e}"));
            }
        }
    }
    Ok(v)
}
