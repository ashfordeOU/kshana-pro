// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Traceability and the verification cross-reference matrix (VCRM).
//!
//! A trace file (TOML) names the requirements, the open-engine runs and the links
//! between them:
//!
//! ```toml
//! title = "Timing and integrity programme"
//! requirements = "requirements.csv"            # .csv, .reqif / .xml, or .sysml
//! field_units_schema = "engine/field-units-schema.json"    # optional
//! verification_matrix = "engine/verification-matrix.json"  # optional
//!
//! [[run]]
//! id = "RUN-CLOCK"
//! scenario = "runs/clock-holdover/clock-holdover.toml"          # optional
//! result = "runs/clock-holdover/clock-holdover.result.json"
//! report = "runs/clock-holdover/clock-holdover.report.json"     # optional
//!
//! [[link]]                     # optional; a requirement's own `runs` also link
//! requirement = "CLK-001"
//! runs = ["RUN-CLOCK"]
//!
//! [[sign_off]]                 # optional; rows of the evidence pack's sign-off table
//! role = "Prepared by"
//! name = ""
//! ```
//!
//! Paths are relative to the trace file. Every criterion is evaluated against every
//! linked run's real `result.json`; the matrix records the verdict, the evidence (the
//! run's `scenario_hash`, `seed`, `engine_version`, and each field path and value
//! read) and the capability labels the verdict rests on, so a MODELLED basis is flagged
//! next to the verdict.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::criterion::{self, Evaluation, Observation, Verdict};
use super::evidence::{
    basis_for, Basis, FieldUnitsSchema, Run, RunPaths, RunUnits, SourceFile, Stamp,
    VerificationMatrix,
};
use super::requirements::{self, Method, Requirement};
use kshana::api::KshanaError;

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraceFile {
    title: String,
    #[serde(default)]
    programme: Option<String>,
    requirements: String,
    #[serde(default)]
    field_units_schema: Option<String>,
    #[serde(default)]
    verification_matrix: Option<String>,
    #[serde(rename = "run", default)]
    runs: Vec<RunSpec>,
    #[serde(rename = "link", default)]
    links: Vec<LinkSpec>,
    #[serde(default)]
    sign_off: Vec<SignOff>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunSpec {
    id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    scenario: Option<String>,
    result: String,
    #[serde(default)]
    report: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkSpec {
    requirement: String,
    runs: Vec<String>,
}

/// A row of the sign-off table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SignOff {
    pub role: String,
    #[serde(default)]
    pub name: String,
}

/// A loaded trace: requirements, runs and links, with the engine artefacts.
#[derive(Debug, Clone)]
pub struct Trace {
    pub title: String,
    pub programme: Option<String>,
    pub requirements: Vec<Requirement>,
    pub requirements_file: SourceFile,
    pub runs: Vec<Run>,
    pub schema: Option<FieldUnitsSchema>,
    pub matrix: Option<VerificationMatrix>,
    /// Requirement id → run ids, in first-seen order.
    pub links: BTreeMap<String, Vec<String>>,
    pub sign_off: Vec<SignOff>,
    pub trace_file: SourceFile,
}

/// Import a requirements file by its extension.
pub fn import_requirements(path: &Path) -> Result<Vec<Requirement>, KshanaError> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| err(format!("could not read {}: {e}", path.display())))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "csv" => requirements::from_csv(&src),
        "reqif" | "xml" => super::reqif::from_reqif(&src),
        "sysml" => super::sysml::requirements_from_sysml(&src).map(|(r, _)| r),
        _ => Err(err(format!(
            "{}: unknown requirements format (use .csv, .reqif or .sysml)",
            path.display()
        ))),
    }
}

impl Trace {
    /// Load a trace file and everything it names.
    pub fn load(path: &Path) -> Result<Trace, KshanaError> {
        let bytes = std::fs::read(path)
            .map_err(|e| err(format!("could not read {}: {e}", path.display())))?;
        let src = String::from_utf8(bytes.clone())
            .map_err(|_| err(format!("{} is not UTF-8", path.display())))?;
        let tf: TraceFile = toml::from_str(&src)
            .map_err(|e| err(format!("{}: invalid trace file: {e}", path.display())))?;
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let at = |p: &str| -> PathBuf { base.join(p) };

        let req_path = at(&tf.requirements);
        let requirements = import_requirements(&req_path)?;
        let requirements_file = SourceFile {
            path: tf.requirements.clone(),
            sha256: super::evidence::sha256_hex(
                &std::fs::read(&req_path).map_err(|e| err(e.to_string()))?,
            ),
        };
        let schema = match &tf.field_units_schema {
            Some(p) => Some(FieldUnitsSchema::load(&at(p), p)?),
            None => None,
        };
        let matrix = match &tf.verification_matrix {
            Some(p) => Some(VerificationMatrix::load(&at(p), p)?),
            None => None,
        };
        let mut runs = Vec::new();
        for r in &tf.runs {
            if runs.iter().any(|x: &Run| x.id == r.id) {
                return Err(err(format!("run id '{}' appears twice", r.id)));
            }
            runs.push(Run::load(&RunPaths {
                id: r.id.clone(),
                description: r.description.clone(),
                scenario: r.scenario.as_deref().map(at),
                result: at(&r.result),
                report: r.report.as_deref().map(at),
                scenario_display: r.scenario.clone(),
                result_display: r.result.clone(),
                report_display: r.report.clone(),
            })?);
        }
        let mut links: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut add = |req: &str, run: &str| -> Result<(), KshanaError> {
            if !requirements.iter().any(|r| r.id == req) {
                return Err(err(format!(
                    "link names requirement '{req}', which is not in {}",
                    tf.requirements
                )));
            }
            if !runs.iter().any(|r| r.id == run) {
                return Err(err(format!(
                    "requirement '{req}' links run '{run}', which the trace does not define"
                )));
            }
            let v = links.entry(req.to_string()).or_default();
            if !v.iter().any(|x| x == run) {
                v.push(run.to_string());
            }
            Ok(())
        };
        for r in &requirements {
            for run in &r.runs {
                add(&r.id, run)?;
            }
        }
        for l in &tf.links {
            for run in &l.runs {
                add(&l.requirement, run)?;
            }
        }
        Ok(Trace {
            title: tf.title,
            programme: tf.programme,
            requirements,
            requirements_file,
            runs,
            schema,
            matrix,
            links,
            sign_off: tf.sign_off,
            trace_file: SourceFile {
                path: path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default(),
                sha256: super::evidence::sha256_hex(&bytes),
            },
        })
    }

    pub fn run(&self, id: &str) -> Option<&Run> {
        self.runs.iter().find(|r| r.id == id)
    }

    /// The runs linked to a requirement.
    pub fn runs_for(&self, req: &str) -> Vec<&Run> {
        self.links
            .get(req)
            .map(|ids| ids.iter().filter_map(|i| self.run(i)).collect())
            .unwrap_or_default()
    }
}

/// The evidence behind one verdict.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Evidence {
    pub scenario_hash: Option<String>,
    pub seed: Option<u64>,
    pub engine_version: Option<String>,
    pub schema_version: Option<String>,
    pub observations: Vec<Observation>,
}

/// One cell of the matrix: a requirement against one run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cell {
    pub run: String,
    pub verdict: Verdict,
    pub reason: Option<String>,
    pub evidence: Evidence,
    pub basis: Basis,
}

/// One row of the matrix.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Row {
    pub requirement: String,
    pub text: String,
    pub method: Method,
    pub criterion: Option<String>,
    /// Met only if every linked run meets it; not met if any run does not; otherwise
    /// not verified.
    pub verdict: Verdict,
    /// True when the overall verdict rests on a basis that is not VALIDATED.
    pub modelled_basis: bool,
    pub cells: Vec<Cell>,
}

/// Something a reviewer must close before sign-off.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OpenItem {
    pub requirement: Option<String>,
    pub run: Option<String>,
    pub category: String,
    pub text: String,
}

/// The reproducibility record of one run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RunRecord {
    pub id: String,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub kind_source: String,
    pub scenario_file: Option<SourceFile>,
    pub result_file: SourceFile,
    pub report_file: Option<SourceFile>,
    pub stamp: Stamp,
    pub capability_rows: Vec<super::evidence::CapabilityRow>,
    pub integrity_problems: Vec<String>,
    pub rerun_command: Option<String>,
}

/// Counts over the matrix.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Summary {
    pub requirements: usize,
    pub met: usize,
    pub not_met: usize,
    pub not_verified: usize,
    pub met_on_modelled_basis: usize,
    pub cells: usize,
    pub runs: usize,
    pub open_items: usize,
}

/// The honesty label every verification cross-reference matrix and trace carries, in the
/// open engine's `label` form.
pub const VCRM_LABEL: &str = "MODELLED - requirements checked against open-engine simulation runs: each verdict states the evidence basis of the runs it rests on (MODELLED, PARTNER or VALIDATED, exactly as the open engine labels them). A met requirement is met in simulation; it is not verified on hardware and not certified.";

/// The verification cross-reference matrix.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Vcrm {
    /// The honesty label, [`VCRM_LABEL`].
    pub label: &'static str,
    pub title: String,
    pub programme: Option<String>,
    pub overlay_version: String,
    pub rows: Vec<Row>,
    pub runs: Vec<RunRecord>,
    pub open_items: Vec<OpenItem>,
    pub summary: Summary,
    pub inputs: Vec<SourceFile>,
}

fn not_verified(reason: impl Into<String>) -> Evaluation {
    Evaluation {
        verdict: Verdict::NotVerified,
        reason: Some(reason.into()),
        observations: vec![],
    }
}

/// Evaluate one requirement against one run.
pub fn evaluate_cell(trace: &Trace, req: &Requirement, run: &Run) -> Cell {
    let eval = if !run.integrity_problems.is_empty() {
        not_verified(run.integrity_problems.join("; "))
    } else {
        match &req.criterion {
            None => not_verified("the requirement has no machine-checkable criterion"),
            Some(src) => match criterion::parse(src) {
                Err(e) => not_verified(format!("criterion does not parse: {e}")),
                Ok(c) => criterion::evaluate(
                    &c,
                    &run.result,
                    &RunUnits {
                        run,
                        schema: trace.schema.as_ref(),
                    },
                ),
            },
        }
    };
    let paths: Vec<String> = eval
        .observations
        .iter()
        .map(|o| o.path.clone())
        .filter(|p| !p.is_empty())
        .collect();
    let basis = basis_for(
        run,
        &paths,
        &req.capabilities,
        trace.schema.as_ref(),
        trace.matrix.as_ref(),
    );
    Cell {
        run: run.id.clone(),
        verdict: eval.verdict,
        reason: eval.reason,
        evidence: Evidence {
            scenario_hash: run.stamp.scenario_hash.clone(),
            seed: run.stamp.seed,
            engine_version: run.stamp.engine_version.clone(),
            schema_version: run.stamp.schema_version.clone(),
            observations: eval.observations,
        },
        basis,
    }
}

/// Build the matrix for a trace.
pub fn vcrm(trace: &Trace) -> Vcrm {
    let mut rows = Vec::new();
    let mut open = Vec::new();
    for req in &trace.requirements {
        let runs = trace.runs_for(&req.id);
        let cells: Vec<Cell> = runs.iter().map(|r| evaluate_cell(trace, req, r)).collect();
        let verdict = if cells.is_empty() {
            Verdict::NotVerified
        } else if cells.iter().any(|c| c.verdict == Verdict::NotMet) {
            Verdict::NotMet
        } else if cells.iter().all(|c| c.verdict == Verdict::Met) {
            Verdict::Met
        } else {
            Verdict::NotVerified
        };
        let modelled_basis = cells.iter().any(|c| c.basis.modelled_basis);
        if cells.is_empty() {
            open.push(OpenItem {
                requirement: Some(req.id.clone()),
                run: None,
                category: "no evidence".into(),
                text: "no run is traced to this requirement".into(),
            });
        }
        if req.criterion.is_none() {
            open.push(OpenItem {
                requirement: Some(req.id.clone()),
                run: None,
                category: "manual verification".into(),
                text: format!(
                    "no machine-checkable criterion: verify by {} and record the result in the sign-off table",
                    req.method.as_str()
                ),
            });
        } else if !req.method.discharged_by_simulation() {
            open.push(OpenItem {
                requirement: Some(req.id.clone()),
                run: None,
                category: "method".into(),
                text: format!(
                    "the declared method is {}; simulation evidence counts as analysis (ECSS-E-ST-10-02C), so the {} itself still needs its own record",
                    req.method.as_str(),
                    req.method.as_str()
                ),
            });
        }
        for c in &cells {
            match c.verdict {
                Verdict::NotVerified => open.push(OpenItem {
                    requirement: Some(req.id.clone()),
                    run: Some(c.run.clone()),
                    category: "not verified".into(),
                    text: c.reason.clone().unwrap_or_default(),
                }),
                Verdict::NotMet => open.push(OpenItem {
                    requirement: Some(req.id.clone()),
                    run: Some(c.run.clone()),
                    category: "not met".into(),
                    text: c
                        .evidence
                        .observations
                        .iter()
                        .map(|o| o.detail.clone())
                        .collect::<Vec<_>>()
                        .join("; "),
                }),
                Verdict::Met if c.basis.modelled_basis => open.push(OpenItem {
                    requirement: Some(req.id.clone()),
                    run: Some(c.run.clone()),
                    category: format!("{} basis", c.basis.label),
                    text: format!(
                        "met on a {} basis: the capability behind this figure is not externally validated in the open verification matrix",
                        c.basis.label
                    ),
                }),
                Verdict::Met => {}
            }
        }
        rows.push(Row {
            requirement: req.id.clone(),
            text: req.text.clone(),
            method: req.method,
            criterion: req.criterion.clone(),
            verdict,
            modelled_basis,
            cells,
        });
    }
    // Label drift: a run's embedded label differs from the programme's matrix.
    if let Some(m) = &trace.matrix {
        for run in &trace.runs {
            for row in &run.capability_rows {
                if let Some(l) = m.label(&row.requirement) {
                    if l != row.label {
                        open.push(OpenItem {
                            requirement: None,
                            run: Some(run.id.clone()),
                            category: "label drift".into(),
                            text: format!(
                                "capability '{}' is {} in the run's report but {} in {}",
                                row.requirement, row.label, l, m.source.path
                            ),
                        });
                    }
                }
            }
        }
    }
    let run_records: Vec<RunRecord> = trace
        .runs
        .iter()
        .map(|r| RunRecord {
            id: r.id.clone(),
            description: r.description.clone(),
            kind: r.kind.clone(),
            kind_source: r.kind_source.clone(),
            scenario_file: r.scenario_file.clone(),
            result_file: r.result_file.clone(),
            report_file: r.report_file.clone(),
            stamp: r.stamp.clone(),
            capability_rows: r.capability_rows.clone(),
            integrity_problems: r.integrity_problems.clone(),
            rerun_command: r.scenario_file.as_ref().map(|s| {
                format!(
                    "kshana {} (engine {})",
                    Path::new(&s.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    r.stamp.engine_version.clone().unwrap_or_else(|| "?".into())
                )
            }),
        })
        .collect();
    for r in &run_records {
        if !r.integrity_problems.is_empty() {
            open.push(OpenItem {
                requirement: None,
                run: Some(r.id.clone()),
                category: "evidence integrity".into(),
                text: r.integrity_problems.join("; "),
            });
        }
    }
    let count = |v: Verdict| rows.iter().filter(|r| r.verdict == v).count();
    let summary = Summary {
        requirements: rows.len(),
        met: count(Verdict::Met),
        not_met: count(Verdict::NotMet),
        not_verified: count(Verdict::NotVerified),
        met_on_modelled_basis: rows
            .iter()
            .filter(|r| r.verdict == Verdict::Met && r.modelled_basis)
            .count(),
        cells: rows.iter().map(|r| r.cells.len()).sum(),
        runs: run_records.len(),
        open_items: open.len(),
    };
    let mut inputs = vec![trace.trace_file.clone(), trace.requirements_file.clone()];
    if let Some(s) = &trace.schema {
        inputs.push(s.source.clone());
    }
    if let Some(m) = &trace.matrix {
        inputs.push(m.source.clone());
    }
    Vcrm {
        label: VCRM_LABEL,
        title: trace.title.clone(),
        programme: trace.programme.clone(),
        overlay_version: crate::version().to_string(),
        rows,
        runs: run_records,
        open_items: open,
        summary,
        inputs,
    }
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn short(h: &Option<String>) -> String {
    h.as_deref()
        .map(|s| s.chars().take(12).collect())
        .unwrap_or_else(|| "—".into())
}

impl Vcrm {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the matrix serialises")
    }

    /// The matrix as a Markdown table, one line per requirement × run.
    pub fn to_markdown(&self) -> String {
        let mut s = format!(
            "# Verification cross-reference matrix (VCRM): {}\n\n",
            md_cell(&self.title)
        );
        s.push_str(&format!(
            "{} requirements: {} met ({} on a MODELLED or PARTNER basis), {} not met, {} not verified. {} runs, {} open items.\n\n",
            self.summary.requirements,
            self.summary.met,
            self.summary.met_on_modelled_basis,
            self.summary.not_met,
            self.summary.not_verified,
            self.summary.runs,
            self.summary.open_items
        ));
        s.push_str("| Requirement | Method | Criterion | Run | Verdict | Basis | Evidence |\n|---|---|---|---|---|---|---|\n");
        for r in &self.rows {
            if r.cells.is_empty() {
                s.push_str(&format!(
                    "| {} | {} | {} | — | not verified | — | no run traced |\n",
                    md_cell(&r.requirement),
                    r.method.as_str(),
                    md_cell(r.criterion.as_deref().unwrap_or("—"))
                ));
            }
            for c in &r.cells {
                let ev = if c.evidence.observations.is_empty() {
                    c.reason.clone().unwrap_or_default()
                } else {
                    c.evidence
                        .observations
                        .iter()
                        .map(|o| o.detail.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                };
                s.push_str(&format!(
                    "| {} | {} | {} | {} (hash {}, seed {}, engine {}) | {} | {}{} | {} |\n",
                    md_cell(&r.requirement),
                    r.method.as_str(),
                    md_cell(r.criterion.as_deref().unwrap_or("—")),
                    md_cell(&c.run),
                    short(&c.evidence.scenario_hash),
                    c.evidence
                        .seed
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "—".into()),
                    c.evidence
                        .engine_version
                        .clone()
                        .unwrap_or_else(|| "—".into()),
                    c.verdict.as_str(),
                    c.basis.label,
                    if c.basis.modelled_basis {
                        " (flagged)"
                    } else {
                        ""
                    },
                    md_cell(&ev)
                ));
            }
        }
        s
    }

    /// The matrix as CSV, one line per requirement × run.
    pub fn to_csv(&self) -> String {
        let q = |s: &str| {
            if s.contains([',', '"', '\n']) {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.to_string()
            }
        };
        let mut s = String::from("requirement,method,criterion,run,verdict,basis,modelled_basis,scenario_hash,seed,engine_version,field_paths,values,reason\n");
        for r in &self.rows {
            for c in &r.cells {
                let paths = c
                    .evidence
                    .observations
                    .iter()
                    .map(|o| o.path.clone())
                    .collect::<Vec<_>>()
                    .join(";");
                let vals = c
                    .evidence
                    .observations
                    .iter()
                    .map(|o| o.value.to_string())
                    .collect::<Vec<_>>()
                    .join(";");
                s.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    q(&r.requirement),
                    r.method.as_str(),
                    q(r.criterion.as_deref().unwrap_or("")),
                    q(&c.run),
                    c.verdict.as_str(),
                    c.basis.label,
                    c.basis.modelled_basis,
                    c.evidence.scenario_hash.clone().unwrap_or_default(),
                    c.evidence.seed.map(|x| x.to_string()).unwrap_or_default(),
                    c.evidence.engine_version.clone().unwrap_or_default(),
                    q(&paths),
                    q(&vals),
                    q(c.reason.as_deref().unwrap_or(""))
                ));
            }
        }
        s
    }

    /// The trace links as JSON: requirement → runs, with the run stamps.
    pub fn links_json(&self) -> serde_json::Value {
        serde_json::json!({
            "title": self.title,
            "label": VCRM_LABEL,
            "links": self.rows.iter().map(|r| serde_json::json!({
                "requirement": r.requirement,
                "runs": r.cells.iter().map(|c| serde_json::json!({
                    "run": c.run,
                    "scenario_hash": c.evidence.scenario_hash,
                    "seed": c.evidence.seed,
                    "engine_version": c.evidence.engine_version,
                    "verdict": c.verdict.as_str(),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
}
