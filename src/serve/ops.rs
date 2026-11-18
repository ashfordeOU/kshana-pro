// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The operation table
//!
//! A job names an operation and carries a JavaScript Object Notation (JSON) request.
//! Each [`Operation`] has a name, the entitlement feature it needs, a `prepare` step
//! that checks the request and the entitlement at submission (nothing is queued when it
//! fails), and a `run` step that turns the request into a result document plus named
//! artefacts. Every scenario runs through [`crate::registry::run_gated`] or a function
//! built on the overlay's own dispatcher, so the open engine is reached only through its
//! public items.
//!
//! Shipped operations:
//!
//! | Name | Request | Result | Artefacts |
//! |---|---|---|---|
//! | `run` | `{"scenario": "<TOML>"}` (any open or Pro kind) | the run's result document, byte for byte | `chart.svg`, `table.csv` when the run gives them |
//! | `dossier` | `{"scenario": "<TOML>", "issued_utc"?: "<stamp>"}` | the study dossier JSON | `dossier.json`, `dossier.html` |
//! | `eloran-run` | `{"scenario": "<eLoran TOML>", "format"?: "ri16"|"rf32"|"none"}` (feature `pro-eloran`) | the run record with a truth preview | `run.json`, `truth.csv`, `recording.sigmf-meta` (the binary recording stays in the job's `inputs/`) |
//! | `ci` | `{"manifest": "<pack TOML>", "scenarios": {"<file>": "<TOML>"}, "baseline"?: <ci.json>, "threshold"?: <relative>, "generated_utc"?: "<stamp>"}` | the Pack C report JSON | `ci.json`, `report.html` |
//!
//! ## Adding an operation
//!
//! 1. Write a `prepare` function: parse the request, return [`OpError::Invalid`] for a
//!    malformed one and [`OpError::Forbidden`] when the entitlement does not allow it.
//!    It must be cheap: it runs on the connection thread.
//! 2. Write a `run` function returning an [`OpOutput`]: the result document (a JSON
//!    string) and the artefacts as `(file name, text)` pairs. Names must pass
//!    [`super::jobs::is_safe_name`].
//! 3. Add one [`Operation`] entry to [`OPERATIONS`].
//!
//! Nothing else changes: the job routes, the store, the workers, the ledger and the
//! OpenAPI document pick the new entry up from this table. A new Pro scenario kind needs
//! no new operation at all: it already runs through `run` once it is registered.

use std::path::Path;

use serde::Deserialize;

use crate::license::{self, Entitlement};

/// Why a request was refused at submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpError {
    /// The request is malformed (HTTP 422).
    Invalid(String),
    /// The entitlement does not allow it (HTTP 403). Never names the licensee.
    Forbidden(String),
}

/// What `prepare` learnt about an accepted request.
#[derive(Debug, Clone, Default)]
pub struct Prepared {
    /// The scenario kind, when the operation runs one scenario.
    pub kind: Option<String>,
}

/// What a worker hands an operation's `run` step.
pub struct RunContext<'a> {
    pub entitlement: &'a Entitlement,
    pub now_unix: i64,
    /// A private directory for input files this job needs on disk.
    pub work_dir: &'a Path,
}

/// The output of a successful operation.
#[derive(Debug, Clone, Default)]
pub struct OpOutput {
    /// The result document (JSON text), stored as `result.json`.
    pub result: String,
    /// Named artefacts, stored under `artefacts/`.
    pub artefacts: Vec<(String, String)>,
}

/// One entry of the operation table.
pub struct Operation {
    /// The operation name used in `POST /jobs`.
    pub name: &'static str,
    /// One line for the OpenAPI document and the guide.
    pub summary: &'static str,
    /// The entitlement feature the operation itself needs (`None`: only the scenario
    /// kind is gated, as for `run`).
    pub feature: Option<&'static str>,
    /// Check the request and the entitlement at submission.
    pub prepare: fn(&serde_json::Value, &Entitlement, i64) -> Result<Prepared, OpError>,
    /// Execute the request on a worker thread.
    pub run: fn(&serde_json::Value, &RunContext<'_>) -> Result<OpOutput, String>,
}

/// The operation table. See the module documentation for how to add an entry.
pub static OPERATIONS: &[Operation] = &[
    Operation {
        name: "run",
        summary: "Run one scenario of any open or Pro kind",
        feature: None,
        prepare: prepare_run,
        run: run_run,
    },
    Operation {
        name: "dossier",
        summary: "Build the reproducible study dossier for one scenario",
        feature: Some("pro-study-dossier"),
        prepare: prepare_dossier,
        run: run_dossier,
    },
    Operation {
        name: "ci",
        summary: "Run a Pack C scenario pack (scenario texts inline) and gate it against an optional baseline",
        feature: Some("pro-scenario-ci"),
        prepare: prepare_ci,
        run: run_ci,
    },
    Operation {
        name: super::eloran::OP,
        summary: "Run an eLoran/GNSS simulator scenario (request.scenario: TOML; request.format: ri16, rf32 or none) to a SigMF recording, truth table and run record",
        feature: Some(crate::eloran_service::FEATURE),
        prepare: super::eloran::prepare_op,
        run: super::eloran::run_op,
    },
];

/// Look an operation up by name.
pub fn find(name: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|op| op.name == name)
}

/// The names of every operation, in table order.
pub fn names() -> Vec<&'static str> {
    OPERATIONS.iter().map(|op| op.name).collect()
}

// ─── Shared checks ──────────────────────────────────────────────────────────────

/// A required, non-empty string field of the request object.
fn required_str<'a>(req: &'a serde_json::Value, field: &str) -> Result<&'a str, OpError> {
    match req.get(field) {
        Some(serde_json::Value::String(s)) if !s.trim().is_empty() => Ok(s),
        Some(serde_json::Value::String(_)) => {
            Err(OpError::Invalid(format!("request.{field} is empty")))
        }
        Some(_) => Err(OpError::Invalid(format!(
            "request.{field} must be a string"
        ))),
        None => Err(OpError::Invalid(format!("request.{field} is required"))),
    }
}

/// Refuse a Pro scenario kind the entitlement does not allow. The message names the
/// kind and the tiers, never the licensee.
fn check_kind(src: &str, ent: &Entitlement, now: i64) -> Result<Option<String>, OpError> {
    let kind = crate::registry::peek_kind(src);
    if let Some(k) = &kind {
        if k.starts_with(crate::registry::PRO_KIND_PREFIX) && !ent.allows(k, now) {
            return Err(OpError::Forbidden(format!(
                "'{k}' requires the '{}' tier; current license tier is '{}'{}",
                license::feature_min_tier(k).as_str(),
                ent.tier.as_str(),
                if ent.is_expired(now) {
                    " (and it has expired)"
                } else {
                    ""
                },
            )));
        }
    }
    Ok(kind)
}

// ─── run ────────────────────────────────────────────────────────────────────────

fn prepare_run(req: &serde_json::Value, ent: &Entitlement, now: i64) -> Result<Prepared, OpError> {
    let src = required_str(req, "scenario")?;
    let kind = check_kind(src, ent, now)?;
    Ok(Prepared { kind })
}

fn run_run(req: &serde_json::Value, ctx: &RunContext<'_>) -> Result<OpOutput, String> {
    let src = required_str(req, "scenario").map_err(op_error_text)?;
    let out = crate::registry::run_gated(src, ctx.entitlement, ctx.now_unix)
        .map_err(|e| e.to_string())?;
    let mut artefacts = Vec::new();
    if !out.svg.trim().is_empty() {
        artefacts.push(("chart.svg".to_string(), out.svg));
    }
    if let Some(csv) = out.csv {
        artefacts.push(("table.csv".to_string(), csv));
    }
    Ok(OpOutput {
        result: out.json,
        artefacts,
    })
}

// ─── dossier ────────────────────────────────────────────────────────────────────

fn prepare_dossier(
    req: &serde_json::Value,
    ent: &Entitlement,
    now: i64,
) -> Result<Prepared, OpError> {
    license::ensure_dossier_entitled(ent, now).map_err(|e| OpError::Forbidden(e.to_string()))?;
    let src = required_str(req, "scenario")?;
    if let Some(v) = req.get("issued_utc") {
        if !v.is_string() && !v.is_null() {
            return Err(OpError::Invalid(
                "request.issued_utc must be a string".into(),
            ));
        }
    }
    let kind = check_kind(src, ent, now)?;
    Ok(Prepared { kind })
}

fn run_dossier(req: &serde_json::Value, ctx: &RunContext<'_>) -> Result<OpOutput, String> {
    // Re-check at run time: a licence can expire between submission and execution.
    prepare_dossier(req, ctx.entitlement, ctx.now_unix).map_err(op_error_text)?;
    let src = required_str(req, "scenario").map_err(op_error_text)?;
    let opts = crate::study_dossier::DossierOptions {
        issued_utc: req
            .get("issued_utc")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    };
    let dossier = crate::study_dossier::build(src, &opts).map_err(|e| e.to_string())?;
    let json = dossier.to_json();
    Ok(OpOutput {
        result: json.clone(),
        artefacts: vec![
            ("dossier.json".to_string(), json),
            ("dossier.html".to_string(), dossier.to_html()),
        ],
    })
}

// ─── ci ─────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CiRequest {
    manifest: String,
    scenarios: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    baseline: Option<serde_json::Value>,
    #[serde(default)]
    threshold: Option<f64>,
    #[serde(default)]
    generated_utc: Option<String>,
}

/// Parse and check a `ci` request; returns it with the parsed pack.
fn parse_ci(
    req: &serde_json::Value,
    ent: &Entitlement,
    now: i64,
) -> Result<(CiRequest, crate::pack_c::ScenarioPack), OpError> {
    let ci: CiRequest = serde_json::from_value(req.clone())
        .map_err(|e| OpError::Invalid(format!("invalid ci request: {e}")))?;
    let pack = crate::pack_c::parse_pack(&ci.manifest).map_err(OpError::Invalid)?;
    for entry in &pack.entries {
        if !super::jobs::is_safe_name(&entry.path) {
            return Err(OpError::Invalid(format!(
                "pack entry path '{}' must be a plain file name (letters, digits, '.', '_', '-'; no directory)",
                entry.path
            )));
        }
        let Some(src) = ci.scenarios.get(&entry.path) else {
            return Err(OpError::Invalid(format!(
                "pack entry '{}' has no scenario text in request.scenarios",
                entry.path
            )));
        };
        check_kind(src, ent, now)?;
    }
    if let Some(t) = ci.threshold {
        if !(t.is_finite() && t >= 0.0) {
            return Err(OpError::Invalid(
                "request.threshold must be a finite, non-negative relative tolerance".into(),
            ));
        }
    }
    if let Some(b) = &ci.baseline {
        serde_json::from_value::<crate::pack_c::CiReport>(b.clone())
            .map_err(|e| OpError::Invalid(format!("request.baseline is not a CI report: {e}")))?;
    }
    Ok((ci, pack))
}

fn prepare_ci(req: &serde_json::Value, ent: &Entitlement, now: i64) -> Result<Prepared, OpError> {
    license::ensure_ci_entitled(ent, now).map_err(|e| OpError::Forbidden(e.to_string()))?;
    parse_ci(req, ent, now)?;
    Ok(Prepared { kind: None })
}

fn run_ci(req: &serde_json::Value, ctx: &RunContext<'_>) -> Result<OpOutput, String> {
    license::ensure_ci_entitled(ctx.entitlement, ctx.now_unix).map_err(|e| e.to_string())?;
    let (ci, pack) = parse_ci(req, ctx.entitlement, ctx.now_unix).map_err(op_error_text)?;
    std::fs::create_dir_all(ctx.work_dir).map_err(|e| format!("cannot create inputs: {e}"))?;
    for entry in &pack.entries {
        let src = &ci.scenarios[&entry.path];
        super::jobs::write_atomic(&ctx.work_dir.join(&entry.path), src.as_bytes())
            .map_err(|e| format!("cannot store scenario '{}': {e}", entry.path))?;
    }
    let baseline = match &ci.baseline {
        Some(b) => Some(
            serde_json::from_value::<crate::pack_c::CiReport>(b.clone())
                .map_err(|e| format!("request.baseline is not a CI report: {e}"))?,
        ),
        None => None,
    };
    let cictx = crate::pack_c::CiContext {
        generated_utc: ci.generated_utc.clone(),
        customer: None,
        thresholds: ci
            .threshold
            .map(crate::pack_c::Thresholds::new)
            .unwrap_or_default(),
    };
    let report = crate::pack_c::run_ci(&pack, ctx.work_dir, baseline.as_ref(), &cictx)?;
    let branded = crate::report_pack_c::PackCReport::from_ci(
        format!("Pack C — {} {}", pack.id, pack.version),
        &report,
    );
    let json = report.to_json();
    Ok(OpOutput {
        result: json.clone(),
        artefacts: vec![
            ("ci.json".to_string(), json),
            ("report.html".to_string(), branded.to_html()),
        ],
    })
}

fn op_error_text(e: OpError) -> String {
    match e {
        OpError::Invalid(m) | OpError::Forbidden(m) => m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::Tier;

    fn pro() -> Entitlement {
        Entitlement {
            licensee: "SECRET-LICENSEE".into(),
            tier: Tier::Pro,
            expires_unix: 0,
            features: vec![],
            arksa: false,
        }
    }

    #[test]
    fn table_names_are_unique_and_findable() {
        let names = names();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len());
        for n in names {
            assert_eq!(find(n).unwrap().name, n);
        }
        assert!(find("nope").is_none());
    }

    #[test]
    fn run_refuses_a_pro_kind_at_evaluation_without_the_licensee() {
        let req = serde_json::json!({"scenario": "kind = \"pro-twin\"\n"});
        let e = prepare_run(&req, &Entitlement::evaluation(), 0).unwrap_err();
        match e {
            OpError::Forbidden(m) => assert!(m.contains("tier") && !m.contains("evaluation —")),
            other => panic!("expected Forbidden, got {other:?}"),
        }
    }

    #[test]
    fn dossier_and_ci_need_their_features() {
        let req = serde_json::json!({"scenario": "kind = \"clock\"\n"});
        assert!(matches!(
            prepare_dossier(&req, &Entitlement::evaluation(), 0),
            Err(OpError::Forbidden(_))
        ));
        assert!(prepare_dossier(&req, &pro(), 0).is_ok());
        let ci = serde_json::json!({"manifest": "", "scenarios": {}});
        assert!(matches!(
            prepare_ci(&ci, &Entitlement::evaluation(), 0),
            Err(OpError::Forbidden(_))
        ));
    }

    #[test]
    fn ci_refuses_paths_that_leave_the_inputs_directory() {
        let manifest = "id = \"p\"\nversion = \"1\"\n[[entries]]\npath = \"../x.toml\"\n";
        let req = serde_json::json!({"manifest": manifest, "scenarios": {"../x.toml": "kind = \"clock\"\n"}});
        match prepare_ci(&req, &pro(), 0) {
            Err(OpError::Invalid(m)) => assert!(m.contains("plain file name"), "{m}"),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn missing_or_wrong_typed_fields_are_invalid() {
        for req in [
            serde_json::json!({}),
            serde_json::json!({"scenario": 3}),
            serde_json::json!({"scenario": "  "}),
        ] {
            assert!(matches!(
                prepare_run(&req, &pro(), 0),
                Err(OpError::Invalid(_))
            ));
        }
    }
}
