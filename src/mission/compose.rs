// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The dossier body: everything the three faces (JSON, HTML, PDF) show.
//!
//! The body is composed from what already exists and nothing is recomputed here: the
//! verification cross-reference matrix, the open items and the run records come from
//! [`crate::mbse`]; the figures and chart columns are read from each run's result
//! document ([`super::figures`]); the capability labels and the not-modelled list are
//! read from each run's `report.json`, the open engine's own report.
//!
//! `dossier_sha256` is the SHA-256 of the body in canonical JSON (object keys sorted, no
//! insignificant white space). The `--utc` issue stamp is kept outside the body, so the
//! same inputs always give the same hash.

use serde::Serialize;
use serde_json::Value;

use super::figures::{chart_of, figures_of, ChartData, FigureRow, SeriesRow};
use super::programme::Programme;
use super::runner::RunArtefacts;
use crate::mbse::criterion::Verdict;
use crate::mbse::evidence::{sha256_hex, SourceFile};
use crate::mbse::pack::Pack;
use crate::mbse::trace::{OpenItem, SignOff, Trace};
use kshana::api::KshanaError;

/// Identifies the JSON document.
pub const DOSSIER_SCHEMA: &str = "kshana-pro-mission-dossier";
/// Version of the JSON document's shape.
pub const DOSSIER_SCHEMA_VERSION: u32 = 1;

/// The banner every face carries. ASCII only, and with no parentheses, so the phrase
/// "not a certification" stays one literal string in the PDF content stream.
pub const BANNER_LINES: [&str; 3] = [
    "MODELLED RESULTS - every number in this dossier is simulation output of the open",
    "Kshana engine, labelled VALIDATED, MODELLED or PARTNER as the engine labels it.",
    "This is an engineering study, not a certification and not acceptance by any authority.",
];

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// What the dossier is and what it was made from.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Identification {
    pub title: String,
    pub reference: String,
    pub issue: String,
    pub classification: String,
    pub overlay_version: String,
    /// Every engine version the runs stamped (one, unless the results disagree).
    pub engine_versions: Vec<String>,
    /// Scenario kinds the embedded open engine reports.
    pub open_engine_kinds: usize,
    /// `release` or `development`: how the Kshana Pro binary that ran was built.
    pub build_profile: String,
    /// SHA-256 over the programme hash, the requirements hash and every scenario hash.
    pub request_sha256: String,
    /// `pack_sha256` of the evidence pack written beside the dossier.
    pub evidence_pack_sha256: String,
    /// Every input file with its SHA-256: the programme file, the requirements file and
    /// each scenario file.
    pub inputs: Vec<SourceFile>,
    /// Files the build wrote that the body's hash covers, with their SHA-256: the trace
    /// file, the copy of the requirements file and every file under `runs/`.
    pub files: Vec<SourceFile>,
}

/// How many verdicts rest on each label.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct BasisCounts {
    pub validated: usize,
    pub modelled: usize,
    pub partner: usize,
}

/// The verdict summary.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VerdictSummary {
    pub requirements: usize,
    pub met: usize,
    pub not_met: usize,
    pub not_verified: usize,
    /// Requirements met on a basis that is not VALIDATED.
    pub met_on_modelled_basis: usize,
    /// Requirements no run is traced to.
    pub without_evidence: usize,
    /// Verdicts: one per requirement and linked run.
    pub verdicts: usize,
    /// The label each of those verdicts rests on.
    pub verdicts_by_basis: BasisCounts,
    /// Scenario runs, the trade study aside.
    pub runs: usize,
    /// 1 when the programme has a trade study, else 0.
    pub trade_studies: usize,
    pub open_items: usize,
}

/// One value a verdict read.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ValueRead {
    pub path: String,
    pub value: Value,
    pub unit: Option<String>,
    pub detail: String,
}

/// One verdict: a requirement against one run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatrixCell {
    pub run: String,
    pub verdict: String,
    /// VALIDATED, MODELLED or PARTNER.
    pub basis: String,
    /// True when the basis is not VALIDATED.
    pub flagged: bool,
    pub values: Vec<ValueRead>,
    pub reason: Option<String>,
}

/// One row of the matrix.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatrixRow {
    pub requirement: String,
    pub text: String,
    pub method: String,
    pub criterion: Option<String>,
    pub verdict: String,
    pub flagged: bool,
    pub cells: Vec<MatrixCell>,
}

/// A capability the run's report says it exercised or relied on, with its label. The
/// name is the row's `requirement` in the open verification matrix; what the engine
/// does for it, its oracle and its tests are in the run's own report.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CapabilityLabel {
    pub requirement: String,
    pub label: String,
    pub role: String,
}

/// Something the run leaves out or assumes, as its report states it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    pub text: String,
    pub source: String,
}

/// A requirement's verdict on one run, for the run's sheet.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SheetVerdict {
    pub requirement: String,
    pub verdict: String,
    pub basis: String,
}

/// The sheet of one run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RunSheet {
    pub id: String,
    pub description: Option<String>,
    pub kind: String,
    pub pro_kind: bool,
    /// The title the engine's report gives the run (else the scenario file name).
    pub title: String,
    /// The run-level honesty label of the result document, when it has one.
    pub label: Option<String>,
    /// The engine's one-line summary of the run.
    pub summary: String,
    pub scenario: SourceFile,
    pub result: SourceFile,
    pub chart_file: SourceFile,
    pub report_json: Option<SourceFile>,
    pub report_html: Option<SourceFile>,
    pub scenario_hash: Option<String>,
    pub seed: Option<u64>,
    pub engine_version: Option<String>,
    pub verdicts: Vec<SheetVerdict>,
    /// The figures the sheet leads with.
    pub headline: Vec<FigureRow>,
    /// Every computed scalar of the result.
    pub figures: Vec<FigureRow>,
    pub other_outputs: Vec<FigureRow>,
    pub inputs: Vec<FigureRow>,
    pub series: Vec<SeriesRow>,
    pub unlabelled_numeric_fields: usize,
    /// True when the result has no `units` block with provenance, so nothing can be
    /// called a figure.
    pub no_provenance: bool,
    pub charts: Vec<ChartData>,
    pub capabilities: Vec<CapabilityLabel>,
    pub capability_labels: BasisCounts,
    pub not_modelled: Vec<Note>,
    /// What the run's integrity rests on.
    pub integrity: String,
}

/// One variant of the trade study.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TradeVariant {
    pub label: String,
    pub metric_value: Option<f64>,
    pub rank: Option<u64>,
    pub scenario_hash: Option<String>,
    pub error: Option<String>,
}

/// The trade section, read from the result of the programme's `pro-trade-study` run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TradeSection {
    pub run: String,
    pub title: String,
    pub metric: String,
    pub lower_better: bool,
    pub variants: Vec<TradeVariant>,
    pub winner: Option<String>,
    pub recommendation: Option<String>,
    pub gate_outcome: Option<String>,
    pub gate_summary: Option<String>,
    /// Merkle root of the trade's own provenance record.
    pub merkle_root: Option<String>,
    pub scenario: SourceFile,
    pub result: SourceFile,
}

/// The reproducibility record of one run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReproRecord {
    pub run: String,
    pub kind: String,
    /// The command that repeats the run.
    pub command: String,
    /// The directory of the dossier the command is run in (a copy of it).
    pub directory: String,
    pub scenario: SourceFile,
    pub result: SourceFile,
    pub report: Option<SourceFile>,
    pub scenario_hash: Option<String>,
    pub seed: Option<u64>,
    pub engine_version: Option<String>,
    pub platform: Option<String>,
    /// True when the free open engine runs this kind.
    pub free_engine: bool,
    pub statement: String,
}

/// Everything the dossier hash covers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DossierBody {
    pub schema: String,
    pub schema_version: u32,
    pub generated_by: String,
    pub identification: Identification,
    pub banner: String,
    pub summary: VerdictSummary,
    pub matrix: Vec<MatrixRow>,
    pub sheets: Vec<RunSheet>,
    pub trade: Option<TradeSection>,
    pub open_items: Vec<OpenItem>,
    pub reproducibility: Vec<ReproRecord>,
    /// What holds for every run of the record: platform, build and how to re-check.
    pub reproducibility_note: String,
    pub sign_off: Vec<SignOff>,
    pub limitations: Vec<String>,
}

/// The mission dossier.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Dossier {
    pub body: DossierBody,
    pub dossier_sha256: String,
    pub issued_utc: Option<String>,
}

/// The canonical JSON of a body: object keys sorted, no insignificant white space.
pub fn canonical_json<T: Serialize>(body: &T) -> Result<String, KshanaError> {
    let v = serde_json::to_value(body).map_err(|e| err(format!("dossier body: {e}")))?;
    serde_json::to_string(&v).map_err(|e| err(format!("dossier body: {e}")))
}

fn str_at(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_str().map(str::to_string)
}

/// `development` for a debug build, `release` otherwise.
pub fn build_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "development"
    } else {
        "release"
    }
}

fn basis_slot<'a>(c: &'a mut BasisCounts, label: &str) -> &'a mut usize {
    match label {
        "VALIDATED" => &mut c.validated,
        "PARTNER" => &mut c.partner,
        _ => &mut c.modelled,
    }
}

fn sheet_of(r: &RunArtefacts, programme: &Programme, pack: &Pack) -> Result<RunSheet, KshanaError> {
    let scenario = programme
        .scenarios
        .iter()
        .find(|s| s.id == r.id)
        .ok_or_else(|| err(format!("run '{}' is not in the programme", r.id)))?;
    let figs = figures_of(&r.result_doc, &r.figures_named)
        .map_err(|e| err(format!("scenario '{}': figure {e}", r.id)))?;
    let mut charts = Vec::new();
    for spec in &scenario.charts {
        charts.push(
            chart_of(&r.result_doc, spec)
                .map_err(|e| err(format!("scenario '{}': chart '{}': {e}", r.id, spec.title)))?,
        );
    }
    let report = r.report_doc.as_ref();
    let rows = report
        .and_then(|d| d.get("capabilities"))
        .and_then(|c| c.get("rows"))
        .and_then(Value::as_array);
    let capabilities: Vec<CapabilityLabel> = rows
        .map(|rows| {
            rows.iter()
                .map(|x| CapabilityLabel {
                    requirement: str_at(x, &["requirement"]).unwrap_or_default(),
                    label: str_at(x, &["label"]).unwrap_or_default(),
                    role: str_at(x, &["role"]).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut capability_labels = BasisCounts::default();
    for c in &capabilities {
        *basis_slot(&mut capability_labels, &c.label) += 1;
    }
    let not_modelled: Vec<Note> = report
        .and_then(|d| d.get("not_modelled"))
        .and_then(|n| n.get("items"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|x| Note {
                    text: str_at(x, &["text"]).unwrap_or_default(),
                    source: str_at(x, &["source"]).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    let record = pack.body.vcrm.runs.iter().find(|x| x.id == r.id);
    let verdicts = pack
        .body
        .vcrm
        .rows
        .iter()
        .flat_map(|row| {
            row.cells
                .iter()
                .filter(|c| c.run == r.id)
                .map(move |c| SheetVerdict {
                    requirement: row.requirement.clone(),
                    verdict: c.verdict.as_str().to_string(),
                    basis: c.basis.label.clone(),
                })
        })
        .collect();
    let integrity = match (&r.report_json, &r.report_note) {
        (Some(_), _) => "The run's report.json records the SHA-256 of the scenario file and of the result file; both are checked against the files whenever the trace is loaded.".to_string(),
        (None, Some(note)) => note.clone(),
        (None, None) => "No report: integrity rests on the result hash alone.".to_string(),
    };
    Ok(RunSheet {
        id: r.id.clone(),
        description: r.description.clone(),
        kind: r.kind.clone(),
        pro_kind: r.pro_kind,
        title: report
            .and_then(|d| str_at(d, &["title"]))
            .or_else(|| str_at(&r.result_doc, &["title"]))
            .unwrap_or_else(|| r.stem.clone()),
        label: str_at(&r.result_doc, &["label"]),
        summary: r.summary.clone(),
        scenario: r.scenario.clone(),
        result: r.result.clone(),
        chart_file: r.chart.clone(),
        report_json: r.report_json.clone(),
        report_html: r.report_html.clone(),
        scenario_hash: record.and_then(|x| x.stamp.scenario_hash.clone()),
        seed: record.and_then(|x| x.stamp.seed),
        engine_version: record.and_then(|x| x.stamp.engine_version.clone()),
        verdicts,
        headline: figs.headline,
        figures: figs.figures,
        other_outputs: figs.other_outputs,
        inputs: figs.inputs,
        series: figs.series,
        unlabelled_numeric_fields: figs.unlabelled_numeric_fields,
        no_provenance: figs.no_provenance,
        charts,
        capabilities,
        capability_labels,
        not_modelled,
        integrity,
    })
}

fn trade_of(r: &RunArtefacts) -> TradeSection {
    let d = &r.result_doc;
    let variants = d
        .get("variants")
        .and_then(Value::as_array)
        .map(|vs| {
            vs.iter()
                .map(|v| TradeVariant {
                    label: str_at(v, &["label"]).unwrap_or_default(),
                    metric_value: v.get("metric_value").and_then(Value::as_f64),
                    rank: v.get("rank").and_then(Value::as_u64),
                    scenario_hash: str_at(v, &["scenario_hash"]),
                    error: str_at(v, &["error"]),
                })
                .collect()
        })
        .unwrap_or_default();
    TradeSection {
        run: r.id.clone(),
        title: str_at(d, &["title"]).unwrap_or_else(|| r.stem.clone()),
        metric: str_at(d, &["metric"]).unwrap_or_default(),
        lower_better: d
            .get("lower_better")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        variants,
        winner: str_at(d, &["winner"]),
        recommendation: str_at(d, &["recommendation"]),
        gate_outcome: str_at(d, &["gate", "outcome"]),
        gate_summary: str_at(d, &["gate", "summary"]),
        merkle_root: str_at(d, &["provenance", "merkle_root"]),
        scenario: r.scenario.clone(),
        result: r.result.clone(),
    }
}

fn repro_of(r: &RunArtefacts, pack: &Pack) -> ReproRecord {
    let record = pack.body.vcrm.runs.iter().find(|x| x.id == r.id);
    let engine_version = record.and_then(|x| x.stamp.engine_version.clone());
    let platform = record.and_then(|x| x.stamp.platform.clone());
    let file = r
        .scenario
        .path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string();
    let (command, statement) = if r.pro_kind {
        (
            format!("kshana-pro run {file} --license <token>"),
            format!(
                "'{}' is a Kshana Pro kind: the free engine does not run it. The command repeats it under a licence that carries the kind, and `kshana-pro mission check <dir> --rerun` compares the result's SHA-256. The open engine writes no report for a Pro kind, so this run's integrity rests on the result hash alone.",
                r.kind
            ),
        )
    } else {
        let engine = engine_version.clone().unwrap_or_else(|| "?".into());
        let on = platform
            .clone()
            .map(|p| format!(" on {p}"))
            .unwrap_or_default();
        let s = format!(
            "The free open Kshana engine repeats this run: in a directory that holds only a copy of {file}, the command writes {stem}.result.json, and with a release build of engine {engine}{on} that file has the SHA-256 recorded here.",
            stem = r.stem
        );
        (format!("kshana {file}"), s)
    };
    ReproRecord {
        run: r.id.clone(),
        kind: r.kind.clone(),
        command,
        directory: r.dir.clone(),
        scenario: r.scenario.clone(),
        result: r.result.clone(),
        report: r.report_json.clone(),
        scenario_hash: record.and_then(|x| x.stamp.scenario_hash.clone()),
        seed: record.and_then(|x| x.stamp.seed),
        engine_version,
        platform,
        free_engine: !r.pro_kind,
        statement,
    }
}

/// What holds for every run of the reproducibility record.
fn reproducibility_note() -> String {
    let mut s = String::from("The engine pins floating-point results per platform and per build: another operating system or processor architecture, or an unoptimised build of the engine, can give slightly different floating-point values in some fields. Then the hash differs, and a field-by-field comparison of the two result files shows by how much. `kshana-pro mission check <dir> --rerun` repeats every run on the embedded engine and compares the hashes.");
    if build_profile() == "development" {
        s.push_str(" This dossier was produced by a development build of Kshana Pro, which compiles the embedded engine at the release optimisation level; a dossier for delivery is produced by a release build.");
    }
    s
}

/// The limitations every dossier states.
fn limitations() -> Vec<String> {
    vec![
        "Simulation evidence counts as analysis (ECSS-E-ST-10-02C, the European Cooperation for Space Standardization verification standard). For a requirement whose method is test, inspection or demonstration, a met criterion supports the requirement but does not replace the test, inspection or demonstration; it stays an open item.".into(),
        "A label grades the capability as the open engine's verification matrix records it, not this programme's scenario inputs: a VALIDATED capability does not make an illustrative input measured.".into(),
        "A figure is a scalar the result's own units block marks `computed`. A numeric field with no units entry is counted and not shown.".into(),
        "Charts are drawn only for the paths the programme names. The chart the engine drew for each run is kept beside its result.".into(),
        "A scenario is run from its own text. A data file a scenario names by relative path is not copied into the dossier.".into(),
        "The PDF is identical between two builds except for the trailer identifier, which the PDF library fills with random characters.".into(),
        "The dossier hash, the manifest and the ledger are not signed: they show that the files agree with each other, not who issued them. Pass the dossier hash to the recipient by a separate route when origin matters.".into(),
        "Nothing in this dossier is a certification, a qualification or an acceptance by any authority.".into(),
    ]
}

/// Compose the dossier from the programme, its runs, the loaded trace and the evidence
/// pack built from it.
pub fn compose(
    programme: &Programme,
    runs: &[RunArtefacts],
    trace: &Trace,
    pack: &Pack,
    issued_utc: Option<String>,
) -> Result<Dossier, KshanaError> {
    let v = &pack.body.vcrm;

    let mut by_basis = BasisCounts::default();
    let mut verdicts = 0;
    let mut matrix = Vec::new();
    for row in &v.rows {
        let cells = row
            .cells
            .iter()
            .map(|c| {
                verdicts += 1;
                *basis_slot(&mut by_basis, &c.basis.label) += 1;
                MatrixCell {
                    run: c.run.clone(),
                    verdict: c.verdict.as_str().to_string(),
                    basis: c.basis.label.clone(),
                    flagged: c.basis.modelled_basis,
                    values: c
                        .evidence
                        .observations
                        .iter()
                        .map(|o| ValueRead {
                            path: o.path.clone(),
                            value: o.value.clone(),
                            unit: o.unit.clone(),
                            detail: o.detail.clone(),
                        })
                        .collect(),
                    reason: c.reason.clone().filter(|_| c.verdict != Verdict::Met),
                }
            })
            .collect();
        matrix.push(MatrixRow {
            requirement: row.requirement.clone(),
            text: row.text.clone(),
            method: row.method.as_str().to_string(),
            criterion: row.criterion.clone(),
            verdict: row.verdict.as_str().to_string(),
            flagged: row.modelled_basis,
            cells,
        });
    }
    let summary = VerdictSummary {
        requirements: v.summary.requirements,
        met: v.summary.met,
        not_met: v.summary.not_met,
        not_verified: v.summary.not_verified,
        met_on_modelled_basis: v.summary.met_on_modelled_basis,
        without_evidence: v.rows.iter().filter(|r| r.cells.is_empty()).count(),
        verdicts,
        verdicts_by_basis: by_basis,
        runs: runs.iter().filter(|r| !r.is_trade).count(),
        trade_studies: runs.iter().filter(|r| r.is_trade).count(),
        open_items: v.open_items.len(),
    };

    let mut sheets = Vec::new();
    let mut trade = None;
    for r in runs {
        if r.is_trade {
            trade = Some(trade_of(r));
        } else {
            sheets.push(sheet_of(r, programme, pack)?);
        }
    }

    let mut engine_versions: Vec<String> = v
        .runs
        .iter()
        .filter_map(|r| r.stamp.engine_version.clone())
        .collect();
    engine_versions.sort();
    engine_versions.dedup();

    let mut inputs = vec![programme.file.clone(), programme.requirements_file.clone()];
    for s in &programme.scenarios {
        inputs.push(SourceFile {
            path: s.path_display.clone(),
            sha256: s.sha256.clone(),
        });
    }
    let mut files = vec![
        trace.trace_file.clone(),
        programme.requirements_file.clone(),
    ];
    for r in runs {
        files.push(r.scenario.clone());
        files.push(r.result.clone());
        files.push(r.chart.clone());
        files.extend(r.report_json.clone());
        files.extend(r.report_html.clone());
    }

    let sign_off = if programme.sign_off.is_empty() {
        ["Prepared by", "Checked by", "Approved by"]
            .iter()
            .map(|r| SignOff {
                role: r.to_string(),
                name: String::new(),
            })
            .collect()
    } else {
        programme.sign_off.clone()
    };

    let body = DossierBody {
        schema: DOSSIER_SCHEMA.to_string(),
        schema_version: DOSSIER_SCHEMA_VERSION,
        generated_by: format!("Kshana Pro {} (mission dossier)", crate::version()),
        identification: Identification {
            title: programme.title.clone(),
            reference: programme.reference.clone(),
            issue: programme.issue.clone(),
            classification: programme.classification.clone(),
            overlay_version: crate::version().to_string(),
            engine_versions,
            open_engine_kinds: crate::open_engine_kinds(),
            build_profile: build_profile().to_string(),
            request_sha256: programme.request_sha256.clone(),
            evidence_pack_sha256: pack.pack_sha256.clone(),
            inputs,
            files,
        },
        banner: BANNER_LINES.join(" "),
        summary,
        matrix,
        sheets,
        trade,
        open_items: v.open_items.clone(),
        reproducibility: runs.iter().map(|r| repro_of(r, pack)).collect(),
        reproducibility_note: reproducibility_note(),
        sign_off,
        limitations: limitations(),
    };
    let dossier_sha256 = sha256_hex(canonical_json(&body)?.as_bytes());
    Ok(Dossier {
        body,
        dossier_sha256,
        issued_utc,
    })
}

impl Dossier {
    /// The dossier as pretty-printed JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the dossier serialises")
    }
}
