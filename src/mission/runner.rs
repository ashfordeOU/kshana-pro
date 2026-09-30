// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Run every scenario of a programme and write its files.
//!
//! Each scenario's exact bytes go through [`crate::registry::run_gated`], so an open
//! kind runs on the embedded open engine unchanged and a Pro kind still needs its
//! entitlement. Under `<out>/runs/<id>/` the run leaves what the open engine's own
//! command line leaves beside a scenario:
//!
//! - `<name>.toml`: the scenario file, byte for byte;
//! - `<name>.result.json` and `<name>.chart.svg`: the run's output, unchanged;
//! - `<name>.report.json` and `<name>.report.html`: the open engine's report of the run
//!   (`kshana::advanced_report::build`), invoked as `kshana <name>.toml` from that
//!   directory, so the report's `scenario_sha256` and `result_sha256` are the hashes of
//!   the two files beside it.
//!
//! The open engine reports only its own kinds. A Pro kind is written without a report,
//! and the dossier says that its integrity rests on the result hash alone.

use std::path::Path;

use serde_json::Value;

use super::programme::{Programme, Scenario};
use crate::license::Entitlement;
use crate::mbse::evidence::{sha256_hex, SourceFile};
use kshana::api::KshanaError;

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// The files of one run, with the documents parsed.
#[derive(Debug, Clone)]
pub struct RunArtefacts {
    pub id: String,
    pub description: Option<String>,
    pub kind: String,
    pub pro_kind: bool,
    pub is_trade: bool,
    /// File name stem shared by the run's files.
    pub stem: String,
    /// The run's directory, relative to the dossier: `runs/<id>`.
    pub dir: String,
    /// Paths are relative to the dossier directory.
    pub scenario: SourceFile,
    pub result: SourceFile,
    pub chart: SourceFile,
    pub report_json: Option<SourceFile>,
    pub report_html: Option<SourceFile>,
    /// Why the run has no report, when it has none.
    pub report_note: Option<String>,
    pub result_doc: Value,
    pub report_doc: Option<Value>,
    /// The engine's one-line summary of the run.
    pub summary: String,
    /// The chart the engine drew.
    pub chart_svg: String,
    pub figures_named: Vec<String>,
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), KshanaError> {
    std::fs::write(path, bytes).map_err(|e| err(format!("could not write {}: {e}", path.display())))
}

/// Refuse the programme before anything runs when it holds a Pro kind the entitlement
/// does not carry. The message never names the licensee.
pub fn ensure_kinds_entitled(
    programme: &Programme,
    ent: &Entitlement,
    now_unix: i64,
) -> Result<(), KshanaError> {
    for s in &programme.scenarios {
        if s.is_pro_kind() && !ent.allows(&s.kind, now_unix) {
            return Err(KshanaError::Unsupported(format!(
                "scenario '{}' is of kind '{}', which requires the '{}' tier; current license tier is '{}'{}",
                s.id,
                s.kind,
                crate::license::feature_min_tier(&s.kind).as_str(),
                ent.tier.as_str(),
                if ent.is_expired(now_unix) {
                    " (and it has expired)"
                } else {
                    ""
                }
            )));
        }
    }
    Ok(())
}

/// Run one scenario and write its files under `<out>/runs/<id>/`.
pub fn run_one(
    s: &Scenario,
    out: &Path,
    ent: &Entitlement,
    now_unix: i64,
) -> Result<RunArtefacts, KshanaError> {
    let run = crate::registry::run_gated(&s.src, ent, now_unix).map_err(|e| match e {
        KshanaError::Unsupported(m) => {
            KshanaError::Unsupported(format!("scenario '{}': {m}", s.id))
        }
        other => err(format!(
            "scenario '{}' ({}) did not run: {other}",
            s.id, s.path_display
        )),
    })?;
    let result_doc: Value = serde_json::from_str(&run.json)
        .map_err(|e| err(format!("scenario '{}': the result is not JSON: {e}", s.id)))?;

    let dir_rel = format!("runs/{}", s.id);
    let dir = out.join("runs").join(&s.id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| err(format!("could not create {}: {e}", dir.display())))?;
    let name = |ext: &str| format!("{}.{ext}", s.stem);
    let rel = |file: &str| format!("{dir_rel}/{file}");

    write(&dir.join(&s.file_name), &s.bytes)?;
    write(&dir.join(name("result.json")), run.json.as_bytes())?;
    write(&dir.join(name("chart.svg")), run.svg.as_bytes())?;

    let (report_json, report_html, report_doc, report_note) = if s.is_pro_kind() {
        (
            None,
            None,
            None,
            Some(format!(
                "'{}' is a Kshana Pro kind; the open engine reports only its own kinds, so this run has no report and its integrity rests on the result hash alone",
                s.kind
            )),
        )
    } else {
        let inv = kshana::advanced_report::Invocation {
            scenario_arg: s.file_name.clone(),
            result_file: Some(name("result.json")),
            chart_file: Some(name("chart.svg")),
            ..Default::default()
        };
        match kshana::advanced_report::build(&run, &s.src, &inv) {
            Ok(report) => {
                let json = report.to_json();
                let html = report.to_html(&run.svg);
                write(&dir.join(name("report.json")), json.as_bytes())?;
                write(&dir.join(name("report.html")), html.as_bytes())?;
                let doc: Value = serde_json::from_str(&json).map_err(|e| {
                    err(format!("scenario '{}': the report is not JSON: {e}", s.id))
                })?;
                (
                    Some(SourceFile {
                        path: rel(&name("report.json")),
                        sha256: sha256_hex(json.as_bytes()),
                    }),
                    Some(SourceFile {
                        path: rel(&name("report.html")),
                        sha256: sha256_hex(html.as_bytes()),
                    }),
                    Some(doc),
                    None,
                )
            }
            Err(e) => (
                None,
                None,
                None,
                Some(format!(
                    "the open engine wrote no report for this run ({e}); its integrity rests on the result hash alone"
                )),
            ),
        }
    };

    Ok(RunArtefacts {
        id: s.id.clone(),
        description: s.description.clone(),
        kind: s.kind.clone(),
        pro_kind: s.is_pro_kind(),
        is_trade: s.is_trade,
        stem: s.stem.clone(),
        dir: dir_rel.clone(),
        scenario: SourceFile {
            path: rel(&s.file_name),
            sha256: s.sha256.clone(),
        },
        result: SourceFile {
            path: rel(&name("result.json")),
            sha256: sha256_hex(run.json.as_bytes()),
        },
        chart: SourceFile {
            path: rel(&name("chart.svg")),
            sha256: sha256_hex(run.svg.as_bytes()),
        },
        report_json,
        report_html,
        report_note,
        result_doc,
        report_doc,
        summary: run.summary,
        chart_svg: run.svg,
        figures_named: s.figures.clone(),
    })
}

/// Run every scenario of the programme, in programme order. Each run is gated by its
/// own kind ([`run_one`]); call [`ensure_kinds_entitled`] first to refuse a programme
/// before anything is written.
pub fn run_all(
    programme: &Programme,
    out: &Path,
    ent: &Entitlement,
    now_unix: i64,
) -> Result<Vec<RunArtefacts>, KshanaError> {
    programme
        .scenarios
        .iter()
        .map(|s| run_one(s, out, ent, now_unix))
        .collect()
}
