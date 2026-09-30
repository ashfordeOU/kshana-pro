// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Mission dossier: requirements to runs to verification matrix to a branded PDF
//!
//! `kshana-pro mission build <programme.toml> --out <dir>` takes a programme (a
//! requirements file and a list of scenarios of any kind) and delivers one dossier:
//!
//! 1. every scenario is run ([`runner`]) and its files are written under `runs/<id>/`
//!    exactly as the open engine's own command line writes them, report included;
//! 2. a trace file is written and the requirements tooling ([`crate::mbse`]) builds the
//!    verification cross-reference matrix and the evidence pack from those files;
//! 3. the dossier is composed ([`compose`]): identification, verdict summary, the
//!    matrix with the values read, one sheet per run ([`figures`]), the trade section,
//!    the open items, the reproducibility record and the sign-off table;
//! 4. it is written as `dossier.json`, `dossier.html` ([`html`]) and `dossier.pdf`
//!    ([`pdf`], with charts from [`pdf_chart`]), with a manifest of every file's
//!    SHA-256 and a hash-chained ledger ([`crate::study_ledger`]).
//!
//! `kshana-pro mission check <dir> [--rerun]` ([`check`]) re-hashes a delivered dossier
//! and, with `--rerun`, repeats every run and compares.
//!
//! Nothing here computes a result of its own: every number is read from a document the
//! open engine (or, for a Pro kind, a Pro pack) wrote, and carries that document's
//! VALIDATED, MODELLED or PARTNER label. See `docs/MISSION-DOSSIER.md`.

pub mod check;
pub mod cli;
pub mod compose;
pub mod figures;
pub mod html;
pub mod pdf;
pub mod pdf_chart;
pub mod programme;
pub mod runner;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::license::Entitlement;
use crate::mbse::evidence::{sha256_hex, SourceFile};
use crate::mbse::trace::{SignOff, Trace};
use crate::study_ledger::{StudyLedger, StudyRecord};
use kshana::api::KshanaError;

/// The licence feature the command needs.
pub const FEATURE: &str = "pro-mission-dossier";
/// Identifies `manifest.json`.
pub const MANIFEST_SCHEMA: &str = "kshana-pro-mission-dossier-manifest";

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// Gate the mission dossier: succeed when `ent` permits the `pro-mission-dossier`
/// feature at `now_unix` (the Enterprise tier carries it; a licence can also grant it by
/// name), else return an `Unsupported` error naming the tier.
pub fn ensure_entitled(ent: &Entitlement, now_unix: i64) -> Result<(), KshanaError> {
    if ent.allows(FEATURE, now_unix) {
        Ok(())
    } else {
        Err(KshanaError::Unsupported(format!(
            "the mission dossier requires the '{}' tier (or the '{FEATURE}' feature on the licence)",
            crate::license::feature_min_tier(FEATURE).as_str()
        )))
    }
}

/// How a dossier is built.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// The issue stamp. It is shown on every face and kept outside the dossier hash.
    pub issued_utc: Option<String>,
    pub entitlement: Entitlement,
    /// The time the entitlement is checked at. The library reads no clock.
    pub now_unix: i64,
}

/// A built dossier and where it was written.
#[derive(Debug, Clone)]
pub struct Built {
    pub dossier: compose::Dossier,
    pub out_dir: PathBuf,
    /// Every file written, relative to `out_dir`, with its SHA-256 (the ledger aside).
    pub files: Vec<SourceFile>,
    pub pdf_pages: usize,
}

/// `manifest.json`: every delivered file with its SHA-256.
#[derive(Debug, Clone, Serialize)]
struct Manifest {
    schema: String,
    dossier_sha256: String,
    files: Vec<SourceFile>,
}

#[derive(Serialize)]
struct TraceRun {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    scenario: String,
    result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    report: Option<String>,
}

#[derive(Serialize)]
struct TraceLink {
    requirement: String,
    runs: Vec<String>,
}

#[derive(Serialize)]
struct TraceOut {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    programme: Option<String>,
    requirements: String,
    run: Vec<TraceRun>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    link: Vec<TraceLink>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    sign_off: Vec<SignOff>,
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), KshanaError> {
    std::fs::write(path, bytes).map_err(|e| err(format!("could not write {}: {e}", path.display())))
}

/// The trace file of a programme's runs, in the form [`Trace::load`] reads.
fn trace_toml(
    programme: &programme::Programme,
    runs: &[runner::RunArtefacts],
) -> Result<String, KshanaError> {
    let t = TraceOut {
        title: programme.title.clone(),
        programme: Some(if programme.reference.is_empty() {
            format!("Issue {}", programme.issue)
        } else {
            format!("{}, issue {}", programme.reference, programme.issue)
        }),
        requirements: programme.requirements_file.path.clone(),
        run: runs
            .iter()
            .map(|r| TraceRun {
                id: r.id.clone(),
                description: r.description.clone(),
                scenario: r.scenario.path.clone(),
                result: r.result.path.clone(),
                report: r.report_json.as_ref().map(|f| f.path.clone()),
            })
            .collect(),
        link: programme
            .links
            .iter()
            .map(|l| TraceLink {
                requirement: l.requirement.clone(),
                runs: l.runs.clone(),
            })
            .collect(),
        sign_off: programme.sign_off.clone(),
    };
    let body = toml::to_string(&t).map_err(|e| err(format!("could not write the trace: {e}")))?;
    Ok(format!(
        "# Written by `kshana-pro mission build` from {}. Read it with `kshana-pro mbse`.\n{body}",
        programme.file.path
    ))
}

/// Build the dossier of a programme into `out_dir`.
pub fn build(
    programme_path: &Path,
    out_dir: &Path,
    opts: &BuildOptions,
) -> Result<Built, KshanaError> {
    ensure_entitled(&opts.entitlement, opts.now_unix)?;
    let programme = programme::Programme::load(programme_path)?;
    runner::ensure_kinds_entitled(&programme, &opts.entitlement, opts.now_unix)?;

    // A ledger already in the directory must verify before anything is added to it.
    let ledger_path = out_dir.join("ledger.json");
    let mut ledger = match std::fs::read_to_string(&ledger_path) {
        Ok(text) => {
            let l = StudyLedger::from_json(&text).map_err(err)?;
            if !l.verify() {
                return Err(err(format!(
                    "{} does not verify: a record was altered; nothing was written",
                    ledger_path.display()
                )));
            }
            l
        }
        Err(_) => StudyLedger::new(),
    };

    std::fs::create_dir_all(out_dir)
        .map_err(|e| err(format!("could not create {}: {e}", out_dir.display())))?;
    let runs = runner::run_all(&programme, out_dir, &opts.entitlement, opts.now_unix)?;

    write(
        &out_dir.join(&programme.requirements_file.path),
        &programme.requirements_bytes,
    )?;
    let trace_text = trace_toml(&programme, &runs)?;
    let trace_path = out_dir.join("trace.toml");
    write(&trace_path, trace_text.as_bytes())?;
    let trace = Trace::load(&trace_path)?;
    let pack = crate::mbse::pack::build(&trace, opts.issued_utc.clone());
    let pack_html = pack.to_html();
    let pack_json = pack.to_json() + "\n";
    write(&out_dir.join("evidence-pack.html"), pack_html.as_bytes())?;
    write(&out_dir.join("evidence-pack.json"), pack_json.as_bytes())?;

    let dossier = compose::compose(&programme, &runs, &trace, &pack, opts.issued_utc.clone())?;
    let json = dossier.to_json() + "\n";
    let charts: BTreeMap<String, String> = runs
        .iter()
        .map(|r| (r.id.clone(), r.chart_svg.clone()))
        .collect();
    let html = dossier.to_html(&charts);
    let pdf = pdf::render(&dossier).map_err(err)?;
    write(&out_dir.join("dossier.json"), json.as_bytes())?;
    write(&out_dir.join("dossier.html"), html.as_bytes())?;
    write(&out_dir.join("dossier.pdf"), &pdf.bytes)?;

    let mut files = vec![
        SourceFile {
            path: "dossier.json".into(),
            sha256: sha256_hex(json.as_bytes()),
        },
        SourceFile {
            path: "dossier.html".into(),
            sha256: sha256_hex(html.as_bytes()),
        },
        SourceFile {
            path: "dossier.pdf".into(),
            sha256: sha256_hex(&pdf.bytes),
        },
        SourceFile {
            path: "evidence-pack.html".into(),
            sha256: sha256_hex(pack_html.as_bytes()),
        },
        SourceFile {
            path: "evidence-pack.json".into(),
            sha256: sha256_hex(pack_json.as_bytes()),
        },
    ];
    files.extend(dossier.body.identification.files.iter().cloned());
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.to_string(),
        dossier_sha256: dossier.dossier_sha256.clone(),
        files: files.clone(),
    };
    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| err(format!("manifest: {e}")))? + "\n";
    write(&out_dir.join("manifest.json"), manifest_json.as_bytes())?;

    // Ledger: the request, the runs, the delivery. Each record chains to the one before.
    let study_id = format!("MD-{}", &programme.request_sha256[..16]);
    let mut digest = String::new();
    for r in &runs {
        digest.push_str(&format!("{} {}\n", r.id, r.result.sha256));
    }
    let record = |run_hash: Option<String>, delivered_utc: Option<String>| StudyRecord {
        study_id: study_id.clone(),
        request_hash: programme.request_sha256.clone(),
        run_hash,
        delivered_utc,
        prev_hash: String::new(),
        record_hash: String::new(),
    };
    ledger.append(record(None, None));
    ledger.append(record(Some(sha256_hex(digest.as_bytes())), None));
    ledger.append(record(
        Some(dossier.dossier_sha256.clone()),
        opts.issued_utc.clone(),
    ));
    write(&ledger_path, (ledger.to_json() + "\n").as_bytes())?;

    Ok(Built {
        dossier,
        out_dir: out_dir.to_path_buf(),
        files,
        pdf_pages: pdf.pages,
    })
}
