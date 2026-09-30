// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The open engine's outputs, read as evidence.
//!
//! Everything here is read from files the open Kshana engine wrote; nothing is
//! recomputed. Per run:
//!
//! - `<name>.result.json` — the result document: the fields a criterion reads, its
//!   `units` block, and where the engine stamps them, `scenario_hash`, `seed`,
//!   `engine_version`, `schema_version`, the run-level `label` and `figure_tiers`.
//! - `<name>.report.json` (optional but recommended) — `kind`,
//!   `reproducibility.{engine_version, seed, scenario_sha256, result_scenario_hash,
//!   result_sha256, platform}` and `capabilities.rows[]` (the verification-matrix rows
//!   the run exercised, each with its VALIDATED / MODELLED / PARTNER label).
//! - `<name>.toml` (optional) — the scenario file, hashed and compared with the
//!   report's `scenario_sha256`, and read for the systems the SysML export describes.
//!
//! And once per programme:
//!
//! - the field-units schema (`docs/field-units-schema.json` in the open repository):
//!   `kinds.<kind>.fields.<path>.{unit, provenance, evidence_tier}`;
//! - the verification matrix ledger (`web/data/verification-matrix.json`):
//!   `rows[].{requirement, status}`.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::criterion::UnitLookup;
use kshana::api::KshanaError;

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// SHA-256 of bytes, lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn read(path: &Path) -> Result<Vec<u8>, KshanaError> {
    std::fs::read(path).map_err(|e| err(format!("could not read {}: {e}", path.display())))
}

fn read_json(path: &Path) -> Result<(serde_json::Value, Vec<u8>), KshanaError> {
    let bytes = read(path)?;
    let v = serde_json::from_slice(&bytes)
        .map_err(|e| err(format!("{} is not JSON: {e}", path.display())))?;
    Ok((v, bytes))
}

/// A source file and its hash, for the reproducibility record.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SourceFile {
    pub path: String,
    pub sha256: String,
}

/// The open engine's field-units schema.
#[derive(Debug, Clone)]
pub struct FieldUnitsSchema {
    pub doc: serde_json::Value,
    pub source: SourceFile,
}

/// One field-units entry, as the schema or a result's `units` block states it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UnitEntry {
    pub unit: String,
    pub provenance: Option<String>,
    pub evidence_tier: Option<String>,
    /// The key that matched (may be the legacy unsuffixed or wildcard spelling).
    pub key: String,
    /// Where the entry was read.
    pub source: String,
}

impl FieldUnitsSchema {
    pub fn load(path: &Path, display: &str) -> Result<Self, KshanaError> {
        let (doc, bytes) = read_json(path)?;
        if doc.get("schema").and_then(|v| v.as_str()) != Some("kshana-field-units") {
            return Err(err(format!(
                "{} is not a Kshana field-units schema (no \"schema\": \"kshana-field-units\")",
                path.display()
            )));
        }
        Ok(FieldUnitsSchema {
            doc,
            source: SourceFile {
                path: display.to_string(),
                sha256: sha256_hex(&bytes),
            },
        })
    }

    /// Whether the schema describes `kind`.
    pub fn has_kind(&self, kind: &str) -> bool {
        self.doc.get("kinds").and_then(|k| k.get(kind)).is_some()
    }

    /// The entry describing `path` of `kind`, through the open engine's own path
    /// grammar (`kshana::field_schema::lookup`).
    pub fn entry(&self, kind: &str, path: &str) -> Option<UnitEntry> {
        let fields = self
            .doc
            .get("kinds")?
            .get(kind)?
            .get("fields")?
            .as_object()?;
        let (key, v) = kshana::field_schema::lookup(fields, path)?;
        Some(UnitEntry {
            unit: v.get("unit")?.as_str()?.to_string(),
            provenance: v
                .get("provenance")
                .and_then(|x| x.as_str())
                .map(str::to_string),
            evidence_tier: v
                .get("evidence_tier")
                .and_then(|x| x.as_str())
                .map(str::to_string),
            key: key.to_string(),
            source: format!(
                "field-units schema ({}): kinds.{kind}.fields.\"{key}\"",
                self.source.path
            ),
        })
    }
}

/// One row of the verification matrix.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatrixRow {
    pub requirement: String,
    pub capability: String,
    pub status: String,
    pub module: String,
}

/// The open engine's verification matrix ledger.
#[derive(Debug, Clone)]
pub struct VerificationMatrix {
    pub rows: Vec<MatrixRow>,
    pub source: SourceFile,
}

impl VerificationMatrix {
    pub fn load(path: &Path, display: &str) -> Result<Self, KshanaError> {
        let (doc, bytes) = read_json(path)?;
        let rows = doc
            .get("rows")
            .and_then(|r| r.as_array())
            .ok_or_else(|| err(format!("{} has no `rows` array", path.display())))?;
        let s = |r: &serde_json::Value, k: &str| {
            r.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        Ok(VerificationMatrix {
            rows: rows
                .iter()
                .map(|r| MatrixRow {
                    requirement: s(r, "requirement"),
                    capability: s(r, "capability"),
                    status: s(r, "status"),
                    module: s(r, "module"),
                })
                .collect(),
            source: SourceFile {
                path: display.to_string(),
                sha256: sha256_hex(&bytes),
            },
        })
    }

    /// The label (VALIDATED / MODELLED / PARTNER) of a matrix row, by its requirement.
    pub fn label(&self, requirement: &str) -> Option<&str> {
        self.rows
            .iter()
            .find(|r| r.requirement == requirement)
            .map(|r| r.status.as_str())
    }
}

/// A verification-matrix row a run's report says it exercised or relied on.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CapabilityRow {
    pub requirement: String,
    pub label: String,
    pub role: String,
    pub used_by: Vec<String>,
}

/// A figure's tier as the result document states it (`figure_tiers.figures[]`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FigureTier {
    pub path: String,
    pub requirement: Option<String>,
    pub tier: String,
}

/// The reproducibility stamp of one run, with where each value was read.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Stamp {
    pub scenario_hash: Option<String>,
    pub scenario_hash_source: String,
    pub seed: Option<u64>,
    pub seed_source: String,
    pub engine_version: Option<String>,
    pub engine_version_source: String,
    pub schema_version: Option<String>,
    /// `reproducibility.scenario_sha256` from the report.
    pub scenario_sha256_recorded: Option<String>,
    /// SHA-256 of the scenario file as it is on disk now.
    pub scenario_sha256_actual: Option<String>,
    /// `reproducibility.result_sha256` from the report.
    pub result_sha256_recorded: Option<String>,
    /// SHA-256 of the result file as it is on disk now.
    pub result_sha256_actual: String,
    pub platform: Option<String>,
    pub command: Option<String>,
}

/// One run of the open engine, loaded as evidence.
#[derive(Debug, Clone)]
pub struct Run {
    pub id: String,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub kind_source: String,
    pub scenario_file: Option<SourceFile>,
    pub scenario_src: Option<String>,
    pub result_file: SourceFile,
    pub report_file: Option<SourceFile>,
    pub result: serde_json::Value,
    pub report: Option<serde_json::Value>,
    pub stamp: Stamp,
    pub capability_rows: Vec<CapabilityRow>,
    pub figure_tiers: Vec<FigureTier>,
    /// The result document's run-level `label`, when it has one.
    pub run_label: Option<String>,
    /// Integrity failures: a hash that does not match. Any entry makes every verdict
    /// on this run `not verified`.
    pub integrity_problems: Vec<String>,
}

/// Where a run's files are.
#[derive(Debug, Clone)]
pub struct RunPaths {
    pub id: String,
    pub description: Option<String>,
    pub scenario: Option<PathBuf>,
    pub result: PathBuf,
    pub report: Option<PathBuf>,
    /// The paths as the trace file spells them, for display.
    pub scenario_display: Option<String>,
    pub result_display: String,
    pub report_display: Option<String>,
}

fn str_at<'a>(v: &'a serde_json::Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_str()
}

impl Run {
    pub fn load(p: &RunPaths) -> Result<Run, KshanaError> {
        let (result, rbytes) = read_json(&p.result)?;
        let result_file = SourceFile {
            path: p.result_display.clone(),
            sha256: sha256_hex(&rbytes),
        };
        let (report, report_file) = match &p.report {
            Some(rp) => {
                let (v, b) = read_json(rp)?;
                (
                    Some(v),
                    Some(SourceFile {
                        path: p.report_display.clone().unwrap_or_default(),
                        sha256: sha256_hex(&b),
                    }),
                )
            }
            None => (None, None),
        };
        let (scenario_src, scenario_file) = match &p.scenario {
            Some(sp) => {
                let b = read(sp)?;
                let src = String::from_utf8(b.clone())
                    .map_err(|_| err(format!("{} is not UTF-8", sp.display())))?;
                (
                    Some(src),
                    Some(SourceFile {
                        path: p.scenario_display.clone().unwrap_or_default(),
                        sha256: sha256_hex(&b),
                    }),
                )
            }
            None => (None, None),
        };

        // Kind: the report states it; else the result; else the scenario's `kind`
        // key; a scenario without one runs as the engine's default kind, `clock`.
        let scenario_kind = scenario_src.as_deref().and_then(kshana_pro_peek_kind);
        let (kind, kind_source) =
            if let Some(k) = report.as_ref().and_then(|r| str_at(r, &["kind"])) {
                (Some(k.to_string()), "report.json `kind`".to_string())
            } else if let Some(k) = str_at(&result, &["kind"]) {
                (Some(k.to_string()), "result.json `kind`".to_string())
            } else if let Some(k) = scenario_kind.clone() {
                (Some(k), "scenario `kind`".to_string())
            } else if scenario_src.is_some() {
                (
                    Some("clock".to_string()),
                    "scenario has no `kind`; the engine's default kind is `clock`".to_string(),
                )
            } else {
                (
                    None,
                    "unknown: no report, no `kind` in the result, no scenario".to_string(),
                )
            };

        let repro = report.as_ref().and_then(|r| r.get("reproducibility"));
        let rstr = |k: &str| {
            repro
                .and_then(|x| x.get(k))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let (scenario_hash, scenario_hash_source) = if let Some(h) =
            str_at(&result, &["scenario_hash"])
        {
            (
                Some(h.to_string()),
                "result.json `scenario_hash`".to_string(),
            )
        } else if let Some(h) = rstr("result_scenario_hash") {
            (
                Some(h),
                "report.json `reproducibility.result_scenario_hash`".to_string(),
            )
        } else {
            (
                    None,
                    "not stamped: this kind's result carries no `scenario_hash`; the report's `scenario_sha256` identifies the input instead".to_string(),
                )
        };
        let (seed, seed_source) = if let Some(s) = result.get("seed").and_then(|v| v.as_u64()) {
            (Some(s), "result.json `seed`".to_string())
        } else if let Some(s) = repro.and_then(|x| x.get("seed")).and_then(|v| v.as_u64()) {
            (Some(s), "report.json `reproducibility.seed`".to_string())
        } else {
            (None, "not recorded".to_string())
        };
        let (engine_version, engine_version_source) =
            if let Some(v) = str_at(&result, &["engine_version"]) {
                (
                    Some(v.to_string()),
                    "result.json `engine_version`".to_string(),
                )
            } else if let Some(v) = rstr("engine_version") {
                (
                    Some(v),
                    "report.json `reproducibility.engine_version`".to_string(),
                )
            } else {
                (None, "not recorded".to_string())
            };
        let platform = repro.and_then(|x| x.get("platform")).map(|p| {
            format!(
                "{}/{}",
                p.get("os").and_then(|v| v.as_str()).unwrap_or("?"),
                p.get("arch").and_then(|v| v.as_str()).unwrap_or("?")
            )
        });
        let stamp = Stamp {
            scenario_hash,
            scenario_hash_source,
            seed,
            seed_source,
            engine_version,
            engine_version_source,
            schema_version: str_at(&result, &["schema_version"]).map(str::to_string),
            scenario_sha256_recorded: rstr("scenario_sha256"),
            scenario_sha256_actual: scenario_file.as_ref().map(|f| f.sha256.clone()),
            result_sha256_recorded: rstr("result_sha256"),
            result_sha256_actual: result_file.sha256.clone(),
            platform,
            command: rstr("command"),
        };

        let mut integrity_problems = Vec::new();
        if let (Some(rec), Some(act)) = (
            &stamp.scenario_sha256_recorded,
            &stamp.scenario_sha256_actual,
        ) {
            if rec != act {
                integrity_problems.push(format!(
                    "the scenario file changed after the run: report records SHA-256 {rec}, the file now hashes to {act}; re-run the engine"
                ));
            }
        }
        // Without a report nothing hashes the scenario against the run, so check what
        // the scenario itself states against what the run stamped: a scenario file of
        // another kind, or with another seed, is not the input of this run.
        if let Some(src) = scenario_src.as_deref() {
            let stated = report
                .as_ref()
                .and_then(|r| str_at(r, &["kind"]))
                .or_else(|| str_at(&result, &["kind"]));
            let sk = scenario_kind.clone().unwrap_or_else(|| "clock".to_string());
            if let Some(rk) = stated {
                if rk != sk {
                    integrity_problems.push(format!(
                        "the scenario file is not this run's input: it is of kind '{sk}', the run is of kind '{rk}'"
                    ));
                }
            }
            if let (Some(ss), Some(rs)) = (kshana_pro_peek_seed(src), stamp.seed) {
                if ss != rs {
                    integrity_problems.push(format!(
                        "the scenario file is not this run's input: it sets seed {ss}, the run stamped seed {rs}"
                    ));
                }
            }
        }
        if let Some(rec) = &stamp.result_sha256_recorded {
            if rec != &stamp.result_sha256_actual {
                integrity_problems.push(format!(
                    "the result file is not the one the report describes: report records SHA-256 {rec}, the file hashes to {}",
                    stamp.result_sha256_actual
                ));
            }
        }
        if let (Some(rv), Some(ev)) = (str_at(&result, &["engine_version"]), rstr("engine_version"))
        {
            if rv != ev {
                integrity_problems.push(format!(
                    "result and report disagree on the engine version ({rv} vs {ev})"
                ));
            }
        }

        let capability_rows = report
            .as_ref()
            .and_then(|r| r.get("capabilities"))
            .and_then(|c| c.get("rows"))
            .and_then(|r| r.as_array())
            .map(|rows| {
                rows.iter()
                    .map(|r| CapabilityRow {
                        requirement: str_at(r, &["requirement"]).unwrap_or_default().to_string(),
                        label: str_at(r, &["label"]).unwrap_or_default().to_string(),
                        role: str_at(r, &["role"]).unwrap_or_default().to_string(),
                        used_by: r
                            .get("used_by")
                            .and_then(|u| u.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let tiers_src = result
            .get("figure_tiers")
            .and_then(|f| f.get("figures"))
            .or_else(|| {
                report
                    .as_ref()
                    .and_then(|r| r.get("capabilities"))
                    .and_then(|c| c.get("figure_tiers"))
            });
        let figure_tiers = tiers_src
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .map(|f| FigureTier {
                        path: str_at(f, &["path"]).unwrap_or_default().to_string(),
                        requirement: str_at(f, &["requirement"]).map(str::to_string),
                        tier: str_at(f, &["tier"]).unwrap_or_default().to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let run_label = str_at(&result, &["label"]).map(str::to_string);

        Ok(Run {
            id: p.id.clone(),
            description: p.description.clone(),
            kind,
            kind_source,
            scenario_file,
            scenario_src,
            result_file,
            report_file,
            result,
            report,
            stamp,
            capability_rows,
            figure_tiers,
            run_label,
            integrity_problems,
        })
    }

    /// The unit entry for a canonical path: the field-units schema first, then the
    /// result document's own `units` block.
    pub fn unit_entry(&self, schema: Option<&FieldUnitsSchema>, path: &str) -> Option<UnitEntry> {
        if let (Some(s), Some(k)) = (schema, &self.kind) {
            if let Some(e) = s.entry(k, path) {
                return Some(e);
            }
        }
        let units = self.result.get("units")?.as_object()?;
        let (key, v) = kshana::field_schema::lookup(units, path)?;
        Some(UnitEntry {
            unit: v.get("unit")?.as_str()?.to_string(),
            provenance: v
                .get("provenance")
                .and_then(|x| x.as_str())
                .map(str::to_string),
            evidence_tier: None,
            key: key.to_string(),
            source: format!(
                "result units block ({}): units.\"{key}\"",
                self.result_file.path
            ),
        })
    }
}

/// The top-level `seed` key of a scenario TOML, if it has a non-negative integer one.
fn kshana_pro_peek_seed(src: &str) -> Option<u64> {
    let v: toml::Value = toml::from_str(src).ok()?;
    u64::try_from(v.get("seed")?.as_integer()?).ok()
}

/// The `kind` key of a scenario TOML, if it has one.
fn kshana_pro_peek_kind(src: &str) -> Option<String> {
    let v: toml::Value = toml::from_str(src).ok()?;
    v.get("kind")?.as_str().map(str::to_string)
}

/// A [`UnitLookup`] over one run and the programme's schema.
pub struct RunUnits<'a> {
    pub run: &'a Run,
    pub schema: Option<&'a FieldUnitsSchema>,
}

impl UnitLookup for RunUnits<'_> {
    fn unit_for(&self, canonical_path: &str) -> Option<(String, String)> {
        self.run
            .unit_entry(self.schema, canonical_path)
            .map(|e| (e.unit, e.source))
    }
}

/// The evidence label a verdict rests on.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Basis {
    /// VALIDATED, MODELLED or PARTNER: the weakest label among the contributions.
    pub label: String,
    /// True when the basis is not VALIDATED: the verdict rests on a modelled (or
    /// partner-owned) capability, and a reader must see that next to it.
    pub modelled_basis: bool,
    /// Each contribution: what was read and the label it carries.
    pub contributions: Vec<BasisItem>,
}

/// One contribution to a basis.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BasisItem {
    pub label: String,
    pub capability: Option<String>,
    pub source: String,
}

fn rank(label: &str) -> u8 {
    match label {
        "VALIDATED" => 0,
        "MODELLED" | "PARTIAL" => 1,
        _ => 2, // PARTNER or anything unrecognised: weakest
    }
}

/// The run-level basis: the run's own label if it states one, else the matrix rows it
/// exercised for its own kind (the row every run shares, reproducibility, is not a
/// statement about any figure and is left out).
pub fn run_basis(run: &Run, matrix: Option<&VerificationMatrix>) -> Vec<BasisItem> {
    let mut out = Vec::new();
    if let Some(l) = &run.run_label {
        if l.trim_start().starts_with("MODELLED") {
            out.push(BasisItem {
                label: "MODELLED".into(),
                capability: None,
                source: format!(
                    "result.json `label` of {}: \"{}\"",
                    run.id,
                    truncate(l, 120)
                ),
            });
        }
    }
    let kind = run.kind.clone().unwrap_or_default();
    let specific: Vec<&CapabilityRow> = run
        .capability_rows
        .iter()
        .filter(|r| !r.used_by.iter().any(|u| u == "every run"))
        .collect();
    // Rows the run's own kind uses; a composite kind (a campaign) lists its members'
    // kinds instead, and then every row it lists counts.
    let own: Vec<&CapabilityRow> = specific
        .iter()
        .copied()
        .filter(|r| r.used_by.contains(&kind))
        .collect();
    let mut chosen = if own.is_empty() { specific } else { own };
    // A partner-owned capability the run relies on is always shown: whatever figure the
    // criterion reads, the hardware behind it is not something Kshana provides.
    for r in &run.capability_rows {
        if r.role.contains("relied") && !chosen.iter().any(|c| c.requirement == r.requirement) {
            chosen.push(r);
        }
    }
    for row in chosen {
        let relied = row.role.contains("relied");
        let label = matrix
            .and_then(|m| m.label(&row.requirement))
            .unwrap_or(&row.label)
            .to_string();
        out.push(BasisItem {
            label: if relied { "PARTNER".into() } else { label },
            capability: Some(row.requirement.clone()),
            source: format!(
                "report.json capabilities of {} (role: {})",
                run.id, row.role
            ),
        });
    }
    if out.is_empty() {
        out.push(BasisItem {
            label: "MODELLED".into(),
            capability: None,
            source: format!(
                "{} states no capability rows; with nothing to read, the basis is taken as MODELLED",
                run.id
            ),
        });
    }
    out
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// The basis of a verdict that read `paths` from `run`, for a requirement that may pin
/// its capability rows.
pub fn basis_for(
    run: &Run,
    paths: &[String],
    pinned: &[String],
    schema: Option<&FieldUnitsSchema>,
    matrix: Option<&VerificationMatrix>,
) -> Basis {
    let mut items: Vec<BasisItem> = Vec::new();
    if !pinned.is_empty() {
        for cap in pinned {
            let from_matrix = matrix.and_then(|m| m.label(cap));
            let from_run = run
                .capability_rows
                .iter()
                .find(|r| &r.requirement == cap)
                .map(|r| r.label.clone());
            let (label, source) = match (from_matrix, from_run) {
                (Some(l), _) => (
                    l.to_string(),
                    format!(
                        "verification matrix ({})",
                        matrix.map(|m| m.source.path.as_str()).unwrap_or("")
                    ),
                ),
                (None, Some(l)) => (l, format!("report.json capabilities of {}", run.id)),
                (None, None) => (
                    "MODELLED".into(),
                    "pinned capability not found in the matrix or the run; taken as MODELLED"
                        .into(),
                ),
            };
            items.push(BasisItem {
                label,
                capability: Some(cap.clone()),
                source,
            });
        }
    } else {
        let mut need_run = false;
        for p in paths {
            if let Some(ft) = run.figure_tiers.iter().find(|f| &f.path == p) {
                items.push(BasisItem {
                    label: ft.tier.clone(),
                    capability: ft.requirement.clone(),
                    source: format!("result.json figure_tiers for `{p}`"),
                });
                continue;
            }
            let tier = run
                .unit_entry(schema, p)
                .and_then(|e| e.evidence_tier.map(|t| (t, e.source)));
            match tier {
                Some((t, src)) if t == "modelled" => items.push(BasisItem {
                    label: "MODELLED".into(),
                    capability: None,
                    source: format!("evidence tier `modelled` of `{p}` in the {src}"),
                }),
                Some((t, _)) if t == "illustrative" || t == "depends-on-input" => {
                    // An echoed input is what the run was asked to do, not evidence of
                    // what it achieved; it neither raises nor lowers the basis.
                }
                Some((t, src)) if t == "validated" => items.push(BasisItem {
                    label: "VALIDATED".into(),
                    capability: None,
                    source: format!("evidence tier `validated` of `{p}` in the {src}"),
                }),
                _ => need_run = true,
            }
        }
        if need_run || items.is_empty() {
            items.extend(run_basis(run, matrix));
        }
    }
    let label = items
        .iter()
        .max_by_key(|i| rank(&i.label))
        .map(|i| match rank(&i.label) {
            0 => "VALIDATED",
            1 => "MODELLED",
            _ => "PARTNER",
        })
        .unwrap_or("MODELLED")
        .to_string();
    Basis {
        modelled_basis: label != "VALIDATED",
        label,
        contributions: items,
    }
}
