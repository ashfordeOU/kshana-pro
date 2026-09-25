// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Study dossier — the reproducible evidence pack a paying study delivers
//!
//! `kshana-pro study --dossier <scenario.toml> --out <dir>` turns ONE scenario into a
//! reproducible evidence dossier, laid out like a European Space Agency (ESA)
//! technical note: identification, scope, applicable and reference documents,
//! method, results, limitations. It is emitted as JSON (the machine record) and as
//! a self-contained HTML page (the human record). There is no PDF face: the
//! existing PDF path ([`crate::report_pdf`]) renders the consolidated trade report,
//! not this document, and a second PDF layout is not built here.
//!
//! ## What the dossier holds
//!
//! 1. **Inputs and versions** — the SHA-256 of the scenario file exactly as given,
//!    the scenario actually run (the file minus its `[dossier]` block), the open
//!    engine version, the Pro overlay version, and the engine's own `scenario_hash`.
//! 2. **Per-figure tier split** — every reported figure of merit labelled
//!    VALIDATED or MODELLED through the open core's [`kshana::fom_label::tier_for`],
//!    which reads the tier back from the open verification matrix. A figure with no
//!    owning matrix row is labelled MODELLED, never promoted.
//! 3. **One-at-a-time sensitivity sweep** — each key parameter is perturbed alone,
//!    the scenario re-run on the same seed, and every figure's change and elasticity
//!    reported.
//! 4. **Timing and holdover** (first-class for `clock` scenarios) — the Pro
//!    holdover uncertainty band ([`crate::twin::holdover_uq`]) against the scenario's
//!    own time-error budget and the published clock-class limits already carried by
//!    [`HoldoverMask`], with the maximum time interval error (MTIE) check that module
//!    already performs. No new mask values are introduced here.
//! 5. **Framework mapping** — a reading against the four resilience levels of the
//!    US Department of Homeland Security (DHS) Resilient PNT Conformance Framework
//!    (RPCF) v2.0, and core-function readings offered for later mapping to IEEE
//!    P1952. Both are labelled **tentative, simulation-derived, not a certified
//!    level** in the data and on the page.
//! 6. **Assurance artefacts** — a Goal Structuring Notation (GSN) argument, an
//!    ECSS-style Verification Control Document (VCD) and a W3C-PROV provenance record
//!    with a Merkle root, all built from the existing L3 types
//!    ([`crate::suite::gsn`], [`crate::suite::vcd`], the open core's
//!    [`kshana::assurance::provenance::ProvRecord`]). For a `pro-trade-study`
//!    scenario the trade's own gate, GSN, VCD and provenance are appended unchanged.
//!
//! ## Reproducibility
//!
//! Every figure is a deterministic function of the scenario (seeded runs), so the
//! dossier carries a `result_hash`: the SHA-256 of its canonical JSON body. The
//! optional issue stamp (`--utc`) is kept outside the hashed body, so two runs of
//! the same scenario produce the same hash and byte-identical files unless a stamp
//! is supplied.
//!
//! ## Honesty
//!
//! The dossier composes existing capability; it measures nothing new. Everything a
//! simulation cannot establish (equipment-design requirements, "no degradation"
//! relative to an industry nominal, source diversity of real hardware) is recorded as
//! not assessed, never assumed.

use kshana::api::KshanaError;
use kshana::assurance::provenance::ProvRecord;
use kshana::fom_label::{tier_for, ValidationTier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::suite::gsn::{GsnArgument, GsnNodeKind};
use crate::suite::provenance::DecisionProvenance;
use crate::suite::vcd::{Vcd, VcdItem, VcdStatus, VerificationMethod};
use crate::twin::holdover_uq::{HoldoverInputs, HoldoverMask, MtieMask};
use crate::util::dot_path_number;

/// The label every framework reading carries, in the data and on the page.
pub const FRAMEWORK_LABEL: &str = "TENTATIVE, SIMULATION-DERIVED, NOT A CERTIFIED LEVEL";

/// Largest number of samples the MTIE check is allowed to use. The band bound is
/// quadratic in the sample count, so a long coast is resampled onto a coarser grid.
const MTIE_MAX_SAMPLES: f64 = 4096.0;

/// Default one-at-a-time perturbation factors.
const DEFAULT_FACTORS: [f64; 2] = [0.5, 2.0];

// ─── Dossier configuration (the optional `[dossier]` block in the scenario) ───────

/// The optional `[dossier]` block. Every field has a default, so any scenario can
/// be dossiered as-is.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DossierSpec {
    /// Document title. Default: derived from the scenario kind.
    #[serde(default)]
    pub title: Option<String>,
    /// Document reference. Default: `KSH-PRO-TN-` + the first 12 hex digits of the
    /// input file's SHA-256.
    #[serde(default)]
    pub reference: Option<String>,
    /// For `clock` scenarios: which side is the subject (`quantum` or `classical`).
    /// Default `quantum`.
    #[serde(default)]
    pub subject: Option<String>,
    /// Dot-paths of the figures to report. Default: every numeric field under a
    /// `fom` object (restricted to the subject side for `clock` scenarios).
    #[serde(default)]
    pub figures: Vec<String>,
    /// One-at-a-time sweep. Default for `clock` scenarios: the subject clock's
    /// noise and residual parameters plus the timing budget.
    #[serde(default)]
    pub sweep: Vec<SweepSpec>,
    /// Holdover uncertainty settings (timing scenarios).
    #[serde(default)]
    pub holdover: Option<HoldoverSpec>,
}

/// One swept parameter: a dot-path into the scenario TOML, perturbed by
/// multiplicative `factors` and/or set to absolute `values`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SweepSpec {
    pub param: String,
    #[serde(default)]
    pub factors: Vec<f64>,
    #[serde(default)]
    pub values: Vec<f64>,
    /// Why the absolute values were chosen (shown in the dossier).
    #[serde(default)]
    pub note: Option<String>,
}

/// Holdover settings for the timing section.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldoverSpec {
    /// Time error at coast start, nanoseconds. Default 0.
    #[serde(default)]
    pub x0_ns: f64,
    /// 1-sigma uncertainty on the residual fractional-frequency offset. Default 0.
    #[serde(default)]
    pub sigma_y0: f64,
    /// Coverage factor of the band. Default 2.
    #[serde(default = "default_k")]
    pub k: f64,
    /// Interval the clock must hold over, seconds. Default: the longest
    /// GNSS-denied window in the scenario.
    #[serde(default)]
    pub reacquisition_s: Option<f64>,
}

fn default_k() -> f64 {
    2.0
}

impl Default for HoldoverSpec {
    fn default() -> Self {
        Self {
            x0_ns: 0.0,
            sigma_y0: 0.0,
            k: default_k(),
            reacquisition_s: None,
        }
    }
}

/// Caller options that are NOT part of the hashed body.
#[derive(Debug, Clone, Default)]
pub struct DossierOptions {
    /// Issue stamp (e.g. `2026-09-25T10:00:00Z`). Kept outside the hash.
    pub issued_utc: Option<String>,
}

// ─── Dossier output ────────────────────────────────────────────────────────────

/// Section 1 — identification.
#[derive(Debug, Clone, Serialize)]
pub struct Identification {
    pub title: String,
    pub reference: String,
    pub issue: String,
    pub document_type: String,
    pub scenario_kind: String,
    pub engine_version: String,
    pub pro_version: String,
    /// SHA-256 of the scenario file bytes exactly as supplied.
    pub input_sha256: String,
    /// SHA-256 of the scenario text actually run (the file minus `[dossier]`).
    pub run_input_sha256: String,
    /// The engine's own reproducible scenario hash, when the kind reports one.
    pub scenario_hash: Option<String>,
    pub seed: Option<i64>,
    pub classification: String,
}

/// One reference document.
#[derive(Debug, Clone, Serialize)]
pub struct RefDoc {
    pub id: String,
    pub title: String,
    pub issuer: String,
    pub source: String,
    pub used_for: String,
}

/// One reported figure with its honest tier.
#[derive(Debug, Clone, Serialize)]
pub struct FigureRow {
    pub path: String,
    pub value: f64,
    pub unit: Option<String>,
    /// `VALIDATED`, `MODELLED` or `PARTIAL`.
    pub tier: String,
    pub tier_basis: String,
}

/// Count of figures per tier, plus the open matrix totals for context.
#[derive(Debug, Clone, Serialize)]
pub struct TierSplit {
    pub validated: usize,
    pub modelled: usize,
    pub partial: usize,
    pub total: usize,
    pub open_matrix_validated: usize,
    pub open_matrix_modelled: usize,
    pub open_matrix_partner_owned: usize,
    pub open_matrix_total: usize,
}

/// A figure's response to one perturbation.
#[derive(Debug, Clone, Serialize)]
pub struct FigureDelta {
    pub path: String,
    pub base: f64,
    pub value: Option<f64>,
    pub delta: Option<f64>,
    /// `delta / |base|`, when the base is non-zero.
    pub rel_delta: Option<f64>,
    /// `(Δf/f) / (Δp/p)`, when both relative changes are defined.
    pub elasticity: Option<f64>,
}

/// One sweep case.
#[derive(Debug, Clone, Serialize)]
pub struct SweepCase {
    pub param: String,
    pub base_value: f64,
    pub factor: Option<f64>,
    pub value: f64,
    pub note: Option<String>,
    pub scenario_hash: Option<String>,
    pub error: Option<String>,
    pub deltas: Vec<FigureDelta>,
}

/// The most influential parameter for one figure.
#[derive(Debug, Clone, Serialize)]
pub struct Driver {
    pub figure: String,
    pub param: Option<String>,
    pub max_abs_elasticity: Option<f64>,
}

/// Section 5.2 — the one-at-a-time sweep.
#[derive(Debug, Clone, Serialize)]
pub struct Sensitivity {
    pub method: String,
    pub cases: Vec<SweepCase>,
    pub drivers: Vec<Driver>,
    pub skipped: Vec<String>,
}

/// One holdover mask evaluated against the coast.
#[derive(Debug, Clone, Serialize)]
pub struct MaskResult {
    pub name: String,
    pub threshold_s: f64,
    pub provenance: String,
    /// In-spec coast (s); `None` when the envelope never reaches the threshold.
    pub in_spec_coast_s: Option<f64>,
    pub holds_over_interval: bool,
    pub envelope_at_interval_s: f64,
    pub margin_at_interval_s: f64,
    pub mtie_compliant: bool,
    pub mtie_first_violation_tau_s: Option<f64>,
    pub mtie_min_margin_s: f64,
    pub mtie_points: usize,
}

/// Section 5.3 — timing and holdover (clock scenarios).
#[derive(Debug, Clone, Serialize)]
pub struct TimingSection {
    pub clock_id: String,
    pub clock_provenance: String,
    pub q_wf: f64,
    pub q_rw: f64,
    pub y0: f64,
    pub drift: f64,
    pub x0_s: f64,
    pub sigma_y0: f64,
    pub k: f64,
    pub longest_denial_s: Option<f64>,
    pub interval_s: Option<f64>,
    pub mtie_tau0_s: f64,
    pub engine_holdover_s: Option<f64>,
    pub budget: MaskResult,
    pub clock_classes: Vec<MaskResult>,
    pub tier: String,
    pub note: String,
}

/// One RPCF requirement and what this simulation can say about it.
#[derive(Debug, Clone, Serialize)]
pub struct RequirementReading {
    pub number: u8,
    pub text: String,
    pub reading: String,
}

/// One RPCF resilience level.
#[derive(Debug, Clone, Serialize)]
pub struct LevelReading {
    pub level: u8,
    pub definition: String,
    pub behaviour: String,
    pub requirements: Vec<RequirementReading>,
}

/// One core-function reading (Prevent / Respond / Recover).
#[derive(Debug, Clone, Serialize)]
pub struct CoreFunctionReading {
    pub function: String,
    pub driver: String,
    pub value: Option<f64>,
    pub reading: String,
}

/// Section 5.4 — framework mapping.
#[derive(Debug, Clone, Serialize)]
pub struct FrameworkMapping {
    pub label: String,
    pub rpcf_levels: Vec<LevelReading>,
    pub tentative_behavioural_level: Option<u8>,
    pub basis: String,
    pub core_functions: Vec<CoreFunctionReading>,
    pub p1952_note: String,
}

/// Material appended unchanged from a `pro-trade-study` run.
#[derive(Debug, Clone, Serialize)]
pub struct TradeAppendix {
    pub winner: Option<String>,
    pub gate_markdown: String,
    pub gsn_markdown: String,
    pub vcd_markdown: String,
    pub provenance_markdown: String,
    pub merkle_root: String,
}

/// Section 5.5 — assurance artefacts.
#[derive(Debug, Clone, Serialize)]
pub struct Assurance {
    pub determinism_runs: usize,
    pub determinism_identical: bool,
    pub gsn: GsnArgument,
    pub vcd: Vcd,
    pub provenance: DecisionProvenance,
    pub trade: Option<TradeAppendix>,
}

/// Everything that is hashed.
#[derive(Debug, Clone, Serialize)]
pub struct DossierBody {
    pub identification: Identification,
    pub scope: Vec<String>,
    pub reference_documents: Vec<RefDoc>,
    pub method: Vec<String>,
    pub figures: Vec<FigureRow>,
    pub tier_split: TierSplit,
    pub sensitivity: Sensitivity,
    pub timing: Option<TimingSection>,
    pub framework_mapping: FrameworkMapping,
    pub assurance: Assurance,
    pub limitations: Vec<String>,
    pub run_scenario_toml: String,
}

/// The complete dossier: the hashed body, its hash, and the unhashed stamp.
#[derive(Debug, Clone, Serialize)]
pub struct StudyDossier {
    #[serde(flatten)]
    pub body: DossierBody,
    /// SHA-256 of the canonical JSON of [`DossierBody`].
    pub result_hash: String,
    /// Optional issue stamp; deliberately outside the hash.
    pub issued_utc: Option<String>,
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

fn sha_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn invalid(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// Split a scenario into (the text to run, the parsed `[dossier]` block).
///
/// With no `[dossier]` block the file is run verbatim, so its `scenario_hash` is the
/// one `kshana-pro run` reports. With a block, the block is removed and the rest
/// re-serialised deterministically; that exact text is recorded in the dossier.
pub fn split_scenario(src: &str) -> Result<(String, DossierSpec), KshanaError> {
    let mut value: toml::Value =
        toml::from_str(src).map_err(|e| invalid(format!("invalid scenario TOML: {e}")))?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| invalid("scenario TOML is not a table"))?;
    match table.remove("dossier") {
        None => Ok((src.to_string(), DossierSpec::default())),
        Some(block) => {
            let spec: DossierSpec = block
                .try_into()
                .map_err(|e| invalid(format!("invalid [dossier] block: {e}")))?;
            let run = toml::to_string(&value)
                .map_err(|e| invalid(format!("could not re-serialise scenario: {e}")))?;
            Ok((run, spec))
        }
    }
}

fn toml_get<'a>(v: &'a toml::Value, path: &str) -> Option<&'a toml::Value> {
    let mut cur = v;
    for key in path.split('.') {
        cur = cur.as_table()?.get(key)?;
    }
    Some(cur)
}

fn toml_number(v: &toml::Value, path: &str) -> Option<f64> {
    match toml_get(v, path)? {
        toml::Value::Float(f) => Some(*f),
        toml::Value::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

/// Return the scenario with the numeric parameter at `path` set to `value`.
fn with_param(run_src: &str, path: &str, value: f64) -> Result<String, KshanaError> {
    let mut root: toml::Value =
        toml::from_str(run_src).map_err(|e| invalid(format!("invalid scenario TOML: {e}")))?;
    let mut keys: Vec<&str> = path.split('.').collect();
    let last = keys
        .pop()
        .ok_or_else(|| invalid("empty sweep parameter path"))?;
    let mut cur = &mut root;
    for k in keys {
        cur = cur
            .as_table_mut()
            .and_then(|t| t.get_mut(k))
            .ok_or_else(|| invalid(format!("sweep parameter '{path}' not found")))?;
    }
    let table = cur
        .as_table_mut()
        .ok_or_else(|| invalid(format!("sweep parameter '{path}' is not in a table")))?;
    match table.get(last) {
        Some(toml::Value::Float(_)) | Some(toml::Value::Integer(_)) => {}
        _ => {
            return Err(invalid(format!(
                "sweep parameter '{path}' is not a numeric field of the scenario"
            )))
        }
    }
    table.insert(last.to_string(), toml::Value::Float(value));
    toml::to_string(&root).map_err(|e| invalid(format!("could not re-serialise scenario: {e}")))
}

/// Run a scenario through the superset dispatcher (open ∪ Pro) and parse its JSON.
fn run_json(src: &str) -> Result<serde_json::Value, KshanaError> {
    let out = crate::registry::run(src)?;
    serde_json::from_str(&out.json).map_err(|e| invalid(format!("result is not JSON: {e}")))
}

fn result_hash(v: &serde_json::Value) -> Option<String> {
    v.get("scenario_hash")
        .and_then(|h| h.as_str())
        .map(String::from)
}

/// Collect dot-paths of numeric leaves under any object key named `fom` (arrays are
/// not entered). `prefix` restricts the search to one subtree.
fn fom_paths(v: &serde_json::Value, prefix: &str) -> Vec<String> {
    fn walk(v: &serde_json::Value, path: &str, under_fom: bool, out: &mut Vec<String>) {
        if let Some(obj) = v.as_object() {
            for (k, child) in obj {
                if k == "units" {
                    continue;
                }
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                let fom = under_fom || k == "fom";
                if fom && child.as_f64().is_some_and(f64::is_finite) {
                    out.push(p);
                } else {
                    walk(child, &p, fom, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    let root = if prefix.is_empty() {
        Some(v)
    } else {
        v.get(prefix)
    };
    if let Some(r) = root {
        walk(r, prefix, false, &mut out);
    }
    out
}

/// Fallback figure set for kinds with no `fom` object: numeric leaves at depth ≤ 2
/// outside `units`, `seed` and arrays.
fn shallow_numeric_paths(v: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(obj) = v.as_object() {
        for (k, child) in obj {
            if k == "units" || k == "seed" {
                continue;
            }
            if child.as_f64().is_some_and(f64::is_finite) {
                out.push(k.clone());
            } else if let Some(inner) = child.as_object() {
                for (k2, c2) in inner {
                    if c2.as_f64().is_some_and(f64::is_finite) {
                        out.push(format!("{k}.{k2}"));
                    }
                }
            }
        }
    }
    out
}

fn tier_of(path: &str) -> (String, String) {
    let leaf = path.rsplit('.').next().unwrap_or(path);
    match tier_for(leaf) {
        Some(t) => (
            t.tag().to_string(),
            format!("open verification matrix, via kshana::fom_label::tier_for(\"{leaf}\")"),
        ),
        None => (
            ValidationTier::Modelled.tag().to_string(),
            "no owning row in the open verification matrix; conservative tier".to_string(),
        ),
    }
}

fn unit_of(result: &serde_json::Value, path: &str) -> Option<String> {
    result
        .get("units")
        .and_then(|u| u.get(path))
        .and_then(|e| e.get("unit"))
        .and_then(|s| s.as_str())
        .map(String::from)
}

/// The longest `denied` window in a `[gnss] windows` list, seconds.
fn longest_denial_s(scn: &toml::Value) -> Option<f64> {
    let windows = toml_get(scn, "gnss.windows")?.as_array()?;
    windows
        .iter()
        .filter(|w| w.get("state").and_then(|s| s.as_str()) == Some("denied"))
        .filter_map(|w| {
            let t0 = w
                .get("t0")
                .and_then(|x| x.as_float().or(x.as_integer().map(|i| i as f64)))?;
            let t1 = w
                .get("t1")
                .and_then(|x| x.as_float().or(x.as_integer().map(|i| i as f64)))?;
            Some(t1 - t0)
        })
        .filter(|d| *d > 0.0)
        .fold(None, |acc: Option<f64>, d| {
            Some(acc.map_or(d, |a| a.max(d)))
        })
}

fn eval_mask(
    h: &HoldoverInputs,
    mask: &HoldoverMask,
    interval_s: f64,
    tau0: f64,
) -> Result<MaskResult, KshanaError> {
    let coast = h.in_spec_coast_s(mask)?;
    let verdict = h.holds_over_interval(mask, interval_s)?;
    let horizon = interval_s.max(2.0 * tau0);
    let mtie = h.check_mtie_mask(tau0, horizon, &MtieMask::from_holdover_mask(mask), true)?;
    Ok(MaskResult {
        name: mask.name.clone(),
        threshold_s: mask.threshold_s,
        provenance: mask.provenance.clone(),
        in_spec_coast_s: coast.is_finite().then_some(coast),
        holds_over_interval: verdict.holds,
        envelope_at_interval_s: verdict.envelope_s,
        margin_at_interval_s: verdict.margin_s,
        mtie_compliant: mtie.compliant,
        mtie_first_violation_tau_s: mtie.first_violation_tau_s,
        mtie_min_margin_s: mtie.min_margin_s,
        mtie_points: mtie.points.len(),
    })
}

// ─── Section builders ──────────────────────────────────────────────────────────

fn reference_documents(has_timing: bool, has_trade: bool) -> Vec<RefDoc> {
    let mut docs = vec![
        RefDoc {
            id: "RD-1".into(),
            title: "Resilient Positioning, Navigation, and Timing (PNT) Conformance Framework, Version 2.0 (26 April 2022)".into(),
            issuer: "US Department of Homeland Security, Science and Technology Directorate, with the Cybersecurity and Infrastructure Security Agency".into(),
            source: "https://www.dhs.gov/sites/default/files/2022-05/22_0531_st_resilient_pnt_conformance_framework_v2.0.pdf".into(),
            used_for: "Resilience levels 0 to 4 (section 5.3); minimum requirements 1 to 8 (section 5.5, Table 1); core functions Prevent, Respond, Recover (section 5.2)".into(),
        },
        RefDoc {
            id: "RD-2".into(),
            title: "Resilient Positioning, Navigation, and Timing (PNT) Reference Architecture, Version 1.0".into(),
            issuer: "US Department of Homeland Security, Science and Technology Directorate".into(),
            source: "https://www.dhs.gov/sites/default/files/2022-06/22_0609_st_resilient_pnt_ra.pdf".into(),
            used_for: "Context only: the seven resilience technique categories (section 3.3); not scored in this dossier".into(),
        },
        RefDoc {
            id: "RD-3".into(),
            title: "IEEE P1952, Standard for Resilient Positioning, Navigation and Timing (PNT) User Equipment (active Project Authorization Request, approved 2021-05-21)".into(),
            issuer: "IEEE Standards Association".into(),
            source: "https://standards.ieee.org/ieee/1952/10606/".into(),
            used_for: "Scope only. The draft standard and its sub-score definitions are not publicly available".into(),
        },
        RefDoc {
            id: "RD-4".into(),
            title: "ECSS-E-ST-10-02C Rev.1, Space engineering: Verification (1 February 2018)".into(),
            issuer: "European Cooperation for Space Standardization".into(),
            source: "https://ecss.nl/standard/ecss-e-st-10-02c-rev-1-verification-1-february-2018/".into(),
            used_for: "Structure of the Verification Control Document (methods: test, analysis, review of design, inspection)".into(),
        },
        RefDoc {
            id: "RD-5".into(),
            title: "Goal Structuring Notation Community Standard, Version 3".into(),
            issuer: "Safety-Critical Systems Club, Assurance Case Working Group (SCSC-141C)".into(),
            source: "https://scsc.uk/gsn".into(),
            used_for: "Structure of the assurance argument (goal, strategy, solution, context, assumption, justification)".into(),
        },
        RefDoc {
            id: "RD-6".into(),
            title: "PROV-DM: The PROV Data Model, W3C Recommendation (30 April 2013)".into(),
            issuer: "World Wide Web Consortium".into(),
            source: "https://www.w3.org/TR/prov-dm/".into(),
            used_for: "Shape of the provenance record whose Merkle root binds every run in this dossier".into(),
        },
    ];
    if has_timing {
        docs.push(RefDoc {
            id: "RD-7".into(),
            title: "ITU-T G.8272, Timing characteristics of primary reference time clocks".into(),
            issuer:
                "International Telecommunication Union, Telecommunication Standardization Sector"
                    .into(),
            source: "https://www.itu.int/rec/T-REC-G.8272".into(),
            used_for:
                "PRTC-A 100 ns time-error class limit, as already carried by HoldoverMask::prtc_a"
                    .into(),
        });
        docs.push(RefDoc {
            id: "RD-8".into(),
            title:
                "ITU-T G.8272.1, Timing characteristics of enhanced primary reference time clocks"
                    .into(),
            issuer:
                "International Telecommunication Union, Telecommunication Standardization Sector"
                    .into(),
            source: "https://www.itu.int/rec/T-REC-G.8272.1".into(),
            used_for:
                "ePRTC 30 ns time-error class limit, as already carried by HoldoverMask::eprtc"
                    .into(),
        });
    }
    if has_trade {
        docs.push(RefDoc {
            id: "RD-9".into(),
            title: "ECSS-E-ST-10C, Space engineering: System engineering general requirements (Annex L, trade-off report)".into(),
            issuer: "European Cooperation for Space Standardization".into(),
            source: "https://ecss.nl/".into(),
            used_for: "Structure of the appended trade-study material".into(),
        });
    }
    docs
}

fn build_framework(timing: Option<&TimingSection>, figures: &[FigureRow]) -> FrameworkMapping {
    let not_sim = "not assessed: an equipment-design requirement that a simulation cannot verify; it needs inspection or test of the unit";
    let fig = |leaf: &str| -> Option<(String, f64)> {
        figures
            .iter()
            .find(|f| f.path.rsplit('.').next() == Some(leaf))
            .map(|f| (f.path.clone(), f.value))
    };

    let threat = timing.and_then(|t| t.longest_denial_s).is_some();
    let bounded = timing.map(|t| {
        let uq_ok = t.budget.holds_over_interval;
        let eng_ok = match (t.engine_holdover_s, t.longest_denial_s) {
            (Some(h), Some(d)) => h >= d,
            _ => false,
        };
        (uq_ok, eng_ok)
    });

    let l2_behaviour = if !threat {
        "not exercised: the scenario has no GNSS-denied window".to_string()
    } else {
        "consistent: the modelled clock keeps outputting a coasted time solution through the denied window (RPCF allows unbounded degradation at this level)".to_string()
    };
    let (l3_behaviour, l3_ok) = match (threat, bounded) {
        (false, _) | (_, None) => (
            "not exercised: no timing threat window to test boundedness against".to_string(),
            false,
        ),
        (true, Some((true, true))) => (
            "consistent: both the conservative holdover band and the engine's simulated coast stay inside the scenario's time-error budget for the whole denied window".to_string(),
            true,
        ),
        (true, Some((uq, eng))) => (
            format!(
                "not supported: holdover band within budget over the window = {uq}; engine coast covers the window = {eng}. Degradation is not shown to be bounded by the scenario budget"
            ),
            false,
        ),
    };

    let rpcf_levels = vec![
        LevelReading {
            level: 1,
            definition: "Ensures recoverability after removal of the threat.".into(),
            behaviour: "not assessable: recovery after the threat is an equipment behaviour; the engine does not model reset, reload or firmware".into(),
            requirements: vec![
                RequirementReading { number: 1, text: "Verify that stored data from external inputs adheres to values and formats of established standards.".into(), reading: not_sim.into() },
                RequirementReading { number: 2, text: "Support full system recovery by manual means and return to the defined performance after removal of the threat.".into(), reading: not_sim.into() },
                RequirementReading { number: 3, text: "Securely reload or update firmware.".into(), reading: not_sim.into() },
            ],
        },
        LevelReading {
            level: 2,
            definition: "Provides a solution (possibly with unbounded degradation) during threat.".into(),
            behaviour: l2_behaviour,
            requirements: vec![
                RequirementReading { number: 4, text: "Identify compromised PNT sources and prevent them from contributing to erroneous PNT solutions.".into(), reading: "not assessed: the scenario models denial, not source compromise; no detector is exercised".into() },
                RequirementReading { number: 5, text: "Support automatic recovery of individual PNT sources and system, without disrupting system PNT output.".into(), reading: not_sim.into() },
            ],
        },
        LevelReading {
            level: 3,
            definition: "Provides a solution (with bounded degradation) during threat.".into(),
            behaviour: l3_behaviour,
            requirements: vec![
                RequirementReading { number: 6, text: "Ensure that corrupted data from one PNT source cannot corrupt data from another PNT source.".into(), reading: not_sim.into() },
                RequirementReading { number: 7, text: "Cross-verify between PNT solutions from all PNT sources.".into(), reading: not_sim.into() },
            ],
        },
        LevelReading {
            level: 4,
            definition: "Provides a solution without degradation during threat.".into(),
            behaviour: "not assessable: 'no degradation' is defined relative to nominal operation set by industry or standards bodies (RPCF section 5.3), which a simulation cannot establish".into(),
            requirements: vec![
                RequirementReading { number: 8, text: "Have diversity of PNT source technology to mitigate common mode threats.".into(), reading: "not assessed: needs a declared architecture of real sources; a single-scenario timing run has none".into() },
            ],
        },
    ];

    let level = if l3_ok {
        Some(3)
    } else if threat {
        Some(2)
    } else {
        None
    };
    let basis = match level {
        Some(l) => format!(
            "Highest RPCF level whose during-threat behaviour this simulation is consistent with: {l}. \
             Behavioural reading only. RPCF levels are cumulative and requirements 1 to 8 are \
             equipment requirements this simulation does not verify, so this is not a level of \
             the equipment. {FRAMEWORK_LABEL}."
        ),
        None => format!(
            "No behavioural reading: the scenario exercises no threat window the levels can be \
             read against. {FRAMEWORK_LABEL}."
        ),
    };

    let prevent = fig("security");
    let respond = fig("availability");
    let recover = timing.and_then(|t| match (t.budget.in_spec_coast_s, t.longest_denial_s) {
        (Some(c), Some(d)) if d > 0.0 => Some((c / d).min(1.0)),
        (None, Some(_)) => Some(1.0),
        _ => None,
    });
    let core_functions = vec![
        CoreFunctionReading {
            function: "Prevent".into(),
            driver: prevent
                .as_ref()
                .map(|(p, _)| {
                    format!("{p} (analytic spoof-detectability bound; not a detector test)")
                })
                .unwrap_or_else(|| "no driver in this scenario".into()),
            value: prevent.map(|(_, v)| v),
            reading: "MODELLED. Read per function; never combine into one score.".into(),
        },
        CoreFunctionReading {
            function: "Respond".into(),
            driver: respond
                .as_ref()
                .map(|(p, _)| format!("{p} (fraction of the run inside the time-error budget)"))
                .unwrap_or_else(|| "no driver in this scenario".into()),
            value: respond.map(|(_, v)| v),
            reading: "MODELLED. Read per function; never combine into one score.".into(),
        },
        CoreFunctionReading {
            function: "Recover".into(),
            driver: if recover.is_some() {
                "in-spec coast of the conservative holdover band divided by the longest denied window, capped at 1".into()
            } else {
                "no driver in this scenario".into()
            },
            value: recover,
            reading: "MODELLED. Read per function; never combine into one score.".into(),
        },
    ];

    FrameworkMapping {
        label: FRAMEWORK_LABEL.to_string(),
        rpcf_levels,
        tentative_behavioural_level: level,
        basis,
        core_functions,
        p1952_note: "IEEE P1952 is an active Project Authorization Request (approved 2021-05-21); its draft text and any sub-score definitions are not publicly available. The Prevent / Respond / Recover readings above are keyed to the RPCF core functions (RD-1 section 5.2) and are offered for later mapping to P1952 once it is published. They are not P1952 scores.".into(),
    }
}

fn build_timing(
    scn: &toml::Value,
    subject: &str,
    result: &serde_json::Value,
    spec: &HoldoverSpec,
) -> Result<Option<TimingSection>, KshanaError> {
    let section = format!("clock_{subject}");
    if toml_get(scn, &section).is_none() {
        return Ok(None);
    }
    let num = |k: &str| toml_number(scn, &format!("{section}.{k}")).unwrap_or(0.0);
    let text = |k: &str| {
        toml_get(scn, &format!("{section}.{k}"))
            .and_then(|v| v.as_str())
            .unwrap_or("(not stated)")
            .to_string()
    };
    let x0_s = spec.x0_ns * 1e-9;
    let h = HoldoverInputs {
        q_wf: num("q_wf"),
        q_rw: num("q_rw"),
        q_drift: 0.0,
        x0: x0_s,
        y0: num("y0"),
        drift: num("drift"),
        sigma_y0: spec.sigma_y0,
        sigma_drift: 0.0,
        k: spec.k,
    };
    let denial = longest_denial_s(scn);
    let interval = spec.reacquisition_s.or(denial);
    let step = toml_number(scn, "time.step_s").unwrap_or(1.0).max(1e-3);
    let span = interval.unwrap_or(step * 2.0).max(step * 2.0);
    let tau0 = step.max(span / MTIE_MAX_SAMPLES);
    let budget_ns = toml_number(scn, "threshold_ns")
        .ok_or_else(|| invalid("clock scenario has no threshold_ns budget"))?;
    let budget_mask = HoldoverMask::custom(
        "scenario time-error budget",
        budget_ns * 1e-9,
        None,
        "the scenario's own threshold_ns input",
    );
    let iv = interval.unwrap_or(0.0);
    let budget = eval_mask(&h, &budget_mask, iv, tau0)?;
    let clock_classes = [
        HoldoverMask::eprtc(),
        HoldoverMask::prtc_a(),
        HoldoverMask::telecom_1_5us_24h(),
    ]
    .iter()
    .map(|m| {
        let iv_m = m.window_s.unwrap_or(iv);
        eval_mask(
            &h,
            m,
            iv_m,
            tau0.max(iv_m.max(2.0 * step) / MTIE_MAX_SAMPLES),
        )
    })
    .collect::<Result<Vec<_>, _>>()?;
    let engine_holdover_s = dot_path_number(result, &format!("{subject}.fom.holdover_s"));
    Ok(Some(TimingSection {
        clock_id: text("id"),
        clock_provenance: text("provenance"),
        q_wf: h.q_wf,
        q_rw: h.q_rw,
        y0: h.y0,
        drift: h.drift,
        x0_s,
        sigma_y0: h.sigma_y0,
        k: h.k,
        longest_denial_s: denial,
        interval_s: interval,
        mtie_tau0_s: tau0,
        engine_holdover_s,
        budget,
        clock_classes,
        tier: "MODELLED".into(),
        note: "The band is the open core's closed-form coast model (white and random-walk frequency noise plus deterministic residuals, no flicker term) wrapped by the Pro holdover uncertainty pack. The MTIE estimator (kshana::allan::mtie) is validated in the open core; the class limits are the cited published values; the composition is MODELLED. The 1.5 µs / 24 h row is tested over its own 24 h window, not the scenario window.".into(),
    }))
}

fn default_sweep(scn: &toml::Value, kind: &str, subject: &str) -> Vec<SweepSpec> {
    let mut out = Vec::new();
    if kind != "clock" {
        return out;
    }
    let section = format!("clock_{subject}");
    for (k, always) in [
        ("q_wf", true),
        ("q_rw", false),
        ("y0", false),
        ("drift", false),
    ] {
        let p = format!("{section}.{k}");
        if let Some(v) = toml_number(scn, &p) {
            if always || v != 0.0 {
                out.push(SweepSpec {
                    param: p,
                    factors: DEFAULT_FACTORS.to_vec(),
                    values: vec![],
                    note: None,
                });
            }
        }
    }
    if toml_number(scn, "threshold_ns").is_some() {
        out.push(SweepSpec {
            param: "threshold_ns".into(),
            factors: DEFAULT_FACTORS.to_vec(),
            values: vec![],
            note: None,
        });
    }
    out
}

fn run_sweep(
    run_src: &str,
    scn: &toml::Value,
    specs: &[SweepSpec],
    figures: &[FigureRow],
) -> Result<Sensitivity, KshanaError> {
    let mut cases = Vec::new();
    let mut skipped = Vec::new();
    for s in specs {
        let Some(base) = toml_number(scn, &s.param) else {
            return Err(invalid(format!(
                "sweep parameter '{}' is not a numeric field of the scenario",
                s.param
            )));
        };
        let mut points: Vec<(Option<f64>, f64)> = Vec::new();
        for &f in &s.factors {
            if base == 0.0 {
                skipped.push(format!(
                    "{} × {f}: base value is 0, a multiplicative perturbation is a no-op (declare absolute `values`)",
                    s.param
                ));
            } else {
                points.push((Some(f), base * f));
            }
        }
        for &v in &s.values {
            points.push((None, v));
        }
        for (factor, value) in points {
            let perturbed = with_param(run_src, &s.param, value)?;
            let (hash, err, deltas) = match run_json(&perturbed) {
                Ok(r) => {
                    let rel_p = (base != 0.0).then(|| (value - base) / base.abs());
                    let deltas = figures
                        .iter()
                        .map(|f| {
                            let v = dot_path_number(&r, &f.path);
                            let delta = v.map(|x| x - f.value);
                            let rel = delta.filter(|_| f.value != 0.0).map(|d| d / f.value.abs());
                            let elasticity = match (rel, rel_p) {
                                (Some(rf), Some(rp)) if rp != 0.0 => Some(rf / rp),
                                _ => None,
                            };
                            FigureDelta {
                                path: f.path.clone(),
                                base: f.value,
                                value: v,
                                delta,
                                rel_delta: rel,
                                elasticity,
                            }
                        })
                        .collect();
                    (result_hash(&r), None, deltas)
                }
                Err(e) => (None, Some(format!("run failed: {e}")), Vec::new()),
            };
            cases.push(SweepCase {
                param: s.param.clone(),
                base_value: base,
                factor,
                value,
                note: s.note.clone(),
                scenario_hash: hash,
                error: err,
                deltas,
            });
        }
    }
    let drivers = figures
        .iter()
        .map(|f| {
            let mut best: Option<(String, f64)> = None;
            for c in &cases {
                for d in c.deltas.iter().filter(|d| d.path == f.path) {
                    if let Some(e) = d.elasticity {
                        if best.as_ref().map_or(true, |(_, b)| e.abs() > *b) {
                            best = Some((c.param.clone(), e.abs()));
                        }
                    }
                }
            }
            Driver {
                figure: f.path.clone(),
                param: best.as_ref().map(|(p, _)| p.clone()),
                max_abs_elasticity: best.map(|(_, e)| e),
            }
        })
        .collect();
    Ok(Sensitivity {
        method: "One-at-a-time: each parameter is changed alone, the scenario re-run on the same seed (common random numbers), and each figure's change reported. Elasticity = (relative change in the figure) / (relative change in the parameter). Interactions between parameters are not explored.".into(),
        cases,
        drivers,
        skipped,
    })
}

fn build_assurance(
    ident: &Identification,
    figures: &[FigureRow],
    split: &TierSplit,
    sens: &Sensitivity,
    timing: Option<&TimingSection>,
    determinism_identical: bool,
    trade: Option<TradeAppendix>,
) -> Assurance {
    let mut g = GsnArgument {
        nodes: Vec::new(),
        links: Vec::new(),
    };
    g.node(
        "G1",
        GsnNodeKind::Goal,
        format!(
            "The figures reported in '{}' are reproducible, honestly tiered, and their sensitivity to the key inputs is stated.",
            ident.title
        ),
    );
    g.node(
        "C1",
        GsnNodeKind::Context,
        format!(
            "Scenario kind '{}', open engine {}, Kshana Pro {}, input SHA-256 {}.",
            ident.scenario_kind, ident.engine_version, ident.pro_version, ident.input_sha256
        ),
    );
    g.in_context_of("G1", "C1");
    g.node(
        "A1",
        GsnNodeKind::Assumption,
        "The scenario inputs represent the system under study; no measured-device or field data is used.".into(),
    );
    g.in_context_of("G1", "A1");
    g.node(
        "S1",
        GsnNodeKind::Strategy,
        "Argue over reproducibility, tier labelling, sensitivity and (for timing scenarios) the time-error budget.".into(),
    );
    g.supported_by("G1", "S1");
    g.node(
        "G2",
        GsnNodeKind::Goal,
        "The baseline result is reproducible from scenario, seed and engine version.".into(),
    );
    g.supported_by("S1", "G2");
    g.node(
        "Sn1",
        GsnNodeKind::Solution,
        format!(
            "Baseline run twice in process: identical = {determinism_identical}; engine scenario hash {}.",
            ident.scenario_hash.as_deref().unwrap_or("(not reported by this kind)")
        ),
    );
    g.supported_by("G2", "Sn1");
    g.node(
        "G3",
        GsnNodeKind::Goal,
        "Every reported figure carries the tier the open verification matrix grants it.".into(),
    );
    g.supported_by("S1", "G3");
    g.node(
        "Sn2",
        GsnNodeKind::Solution,
        format!(
            "Tier table: {} VALIDATED, {} MODELLED, {} PARTIAL of {} figures.",
            split.validated, split.modelled, split.partial, split.total
        ),
    );
    g.supported_by("G3", "Sn2");
    g.node(
        "G4",
        GsnNodeKind::Goal,
        "The sensitivity of each figure to the key inputs is stated.".into(),
    );
    g.supported_by("S1", "G4");
    if sens.cases.is_empty() {
        g.node(
            "A2",
            GsnNodeKind::Assumption,
            "No sweep parameters were declared or defaulted for this kind; sensitivity is not established.".into(),
        );
        g.in_context_of("G4", "A2");
    } else {
        g.node(
            "Sn3",
            GsnNodeKind::Solution,
            format!(
                "One-at-a-time sweep: {} cases, {} failed.",
                sens.cases.len(),
                sens.cases.iter().filter(|c| c.error.is_some()).count()
            ),
        );
        g.supported_by("G4", "Sn3");
    }
    if let Some(t) = timing {
        g.node(
            "G5",
            GsnNodeKind::Goal,
            "The clock's coast is compared with the scenario time-error budget over the denied window.".into(),
        );
        g.supported_by("S1", "G5");
        g.node(
            "Sn4",
            GsnNodeKind::Solution,
            format!(
                "Holdover band (k = {}) within budget over the interval: {}; MTIE within budget: {}.",
                t.k, t.budget.holds_over_interval, t.budget.mtie_compliant
            ),
        );
        g.supported_by("G5", "Sn4");
    }
    g.node(
        "J1",
        GsnNodeKind::Justification,
        format!("Framework readings are behavioural and per function: {FRAMEWORK_LABEL}."),
    );
    g.in_context_of("S1", "J1");

    let mut items = vec![
        VcdItem {
            req_id: "REQ-DOS-010".into(),
            requirement: "The baseline result shall be reproducible from scenario, seed and engine version.".into(),
            method: VerificationMethod::Test,
            evidence: "baseline run twice in process, results compared".into(),
            status: if determinism_identical { VcdStatus::Pass } else { VcdStatus::Fail },
            note: format!("identical = {determinism_identical}"),
        },
        VcdItem {
            req_id: "REQ-DOS-020".into(),
            requirement: "Every reported figure shall carry a VALIDATED or MODELLED tier from the open verification matrix.".into(),
            method: VerificationMethod::Inspection,
            evidence: format!("{} of {} figures tiered", figures.len(), split.total),
            status: if figures.is_empty() { VcdStatus::Open } else { VcdStatus::Pass },
            note: "a figure with no owning row is labelled MODELLED".into(),
        },
        VcdItem {
            req_id: "REQ-DOS-030".into(),
            requirement: "The one-at-a-time sensitivity sweep shall complete without failed runs.".into(),
            method: VerificationMethod::Analysis,
            evidence: format!("{} cases", sens.cases.len()),
            status: if sens.cases.is_empty() {
                VcdStatus::Open
            } else if sens.cases.iter().any(|c| c.error.is_some()) {
                VcdStatus::Fail
            } else {
                VcdStatus::Pass
            },
            note: if sens.cases.is_empty() { "no sweep declared".into() } else { "same seed for every case".into() },
        },
    ];
    items.push(match timing {
        Some(t) => VcdItem {
            req_id: "REQ-DOS-040".into(),
            requirement: "The clock shall hold the scenario time-error budget over the denied window (conservative band).".into(),
            method: VerificationMethod::Analysis,
            evidence: format!(
                "envelope {:.3e} s vs budget {:.3e} s",
                t.budget.envelope_at_interval_s, t.budget.threshold_s
            ),
            status: if t.interval_s.is_none() {
                VcdStatus::NotApplicable
            } else if t.budget.holds_over_interval {
                VcdStatus::Pass
            } else {
                VcdStatus::Fail
            },
            note: "MODELLED holdover band".into(),
        },
        None => VcdItem {
            req_id: "REQ-DOS-040".into(),
            requirement: "The clock shall hold the scenario time-error budget over the denied window (conservative band).".into(),
            method: VerificationMethod::Analysis,
            evidence: "not a clock scenario".into(),
            status: VcdStatus::NotApplicable,
            note: "no timing section".into(),
        },
    });
    items.push(VcdItem {
        req_id: "REQ-DOS-050".into(),
        requirement: "Every framework reading shall be labelled tentative, simulation-derived and not a certified level.".into(),
        method: VerificationMethod::Inspection,
        evidence: FRAMEWORK_LABEL.into(),
        status: VcdStatus::Pass,
        note: "carried in the data and on the page".into(),
    });

    let mut input_hashes = vec![ident.input_sha256.clone(), ident.run_input_sha256.clone()];
    input_hashes.push(
        ident
            .scenario_hash
            .clone()
            .unwrap_or_else(|| "∅".to_string()),
    );
    for c in &sens.cases {
        let mut s = format!("{}={:e};", c.param, c.value);
        s.push_str(c.scenario_hash.as_deref().unwrap_or("∅"));
        for d in &c.deltas {
            s.push_str(&format!(
                ";{}={}",
                d.path,
                d.value
                    .map(|v| format!("{v:e}"))
                    .unwrap_or_else(|| "∅".into())
            ));
        }
        if let Some(e) = &c.error {
            s.push_str(";err=");
            s.push_str(e);
        }
        input_hashes.push(sha_hex(s.as_bytes()));
    }
    let record = ProvRecord {
        engine_commit: format!(
            "kshana-pro/{} on open kshana {}",
            ident.pro_version, ident.engine_version
        ),
        input_hashes,
        seed: ident.seed.and_then(|s| u64::try_from(s).ok()),
        nondeterminism_notes: vec![
            "Every run is seeded; sweep cases reuse the baseline seed.".into(),
            "The issue stamp is excluded from the hashed body.".into(),
        ],
        lockfile_hash: String::new(),
        timestamp: None,
        tolerance: Some("figures hashed at f64 shortest round-trip precision".into()),
    };
    let merkle_root = record.merkle_root();

    Assurance {
        determinism_runs: 2,
        determinism_identical,
        gsn: g,
        vcd: Vcd { items },
        provenance: DecisionProvenance {
            record,
            merkle_root,
        },
        trade,
    }
}

fn trade_appendix(run_src: &str) -> Result<Option<TradeAppendix>, KshanaError> {
    use crate::packs::trade_study_pack::TradeStudyScenario;
    use crate::suite::{gate::assurance_gate, gsn::build_argument, provenance::build_provenance};
    let scenario: TradeStudyScenario = toml::from_str(run_src)
        .map_err(|e| invalid(format!("invalid pro-trade-study scenario: {e}")))?;
    let report = crate::suite::run_trade_study(&scenario.trade_space())?;
    let prov = build_provenance(&report);
    Ok(Some(TradeAppendix {
        winner: report.winner.clone(),
        gate_markdown: assurance_gate(&report).markdown(),
        gsn_markdown: build_argument(&report, report.gate.min_trl).markdown(),
        vcd_markdown: crate::suite::vcd::build_vcd(&report).markdown(),
        provenance_markdown: prov.markdown(),
        merkle_root: prov.merkle_root,
    }))
}

fn engine_version_of(result: &serde_json::Value) -> Result<String, KshanaError> {
    if let Some(v) = result.get("engine_version").and_then(|v| v.as_str()) {
        return Ok(v.to_string());
    }
    // Kinds that do not echo the engine version: ask the embedded engine with the
    // smallest open run that does.
    let probe = "kind = \"clock\"\nseed = 1\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 20.0\n[gnss]\nwindows = [ { t0 = 0.0, t1 = 20.0, state = \"nominal\" } ]\n[clock_quantum]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n[clock_classical]\nid = \"probe\"\nprovenance = \"version probe\"\ny0 = 0.0\nq_wf = 1.0e-20\nq_rw = 0.0\n";
    let r = run_json(probe)?;
    Ok(r.get("engine_version")
        .and_then(|v| v.as_str())
        .unwrap_or("(unreported)")
        .to_string())
}

// ─── Entry point ───────────────────────────────────────────────────────────────

/// Build the study dossier for one scenario source.
pub fn build(src: &str, opts: &DossierOptions) -> Result<StudyDossier, KshanaError> {
    let (run_src, spec) = split_scenario(src)?;
    let scn: toml::Value =
        toml::from_str(&run_src).map_err(|e| invalid(format!("invalid scenario TOML: {e}")))?;
    let kind = crate::registry::peek_kind(&run_src).unwrap_or_else(|| "clock".to_string());
    let subject = spec
        .subject
        .clone()
        .unwrap_or_else(|| "quantum".to_string());
    if kind == "clock" && subject != "quantum" && subject != "classical" {
        return Err(invalid(format!(
            "[dossier] subject must be 'quantum' or 'classical' for a clock scenario (got '{subject}')"
        )));
    }

    // Baseline, twice: the determinism evidence.
    let result = run_json(&run_src)?;
    let again = run_json(&run_src)?;
    let determinism_identical = result == again;

    // Figures.
    let paths = if !spec.figures.is_empty() {
        spec.figures.clone()
    } else {
        let prefix = if kind == "clock" {
            subject.as_str()
        } else {
            ""
        };
        let p = fom_paths(&result, prefix);
        if p.is_empty() {
            shallow_numeric_paths(&result)
        } else {
            p
        }
    };
    if paths.is_empty() {
        return Err(invalid(
            "no numeric figures found in the result; declare them in [dossier] figures",
        ));
    }
    let mut figures = Vec::new();
    for p in &paths {
        let value = dot_path_number(&result, p)
            .ok_or_else(|| invalid(format!("figure '{p}' is missing or not a finite number")))?;
        let (tier, tier_basis) = tier_of(p);
        figures.push(FigureRow {
            path: p.clone(),
            value,
            unit: unit_of(&result, p),
            tier,
            tier_basis,
        });
    }
    let count = |t: ValidationTier| figures.iter().filter(|f| f.tier == t.tag()).count();
    let matrix = kshana::verification::summarize(&kshana::verification::verification_matrix());
    let tier_split = TierSplit {
        validated: count(ValidationTier::Validated),
        modelled: count(ValidationTier::Modelled),
        partial: count(ValidationTier::Partial),
        total: figures.len(),
        open_matrix_validated: matrix.validated,
        open_matrix_modelled: matrix.modelled,
        open_matrix_partner_owned: matrix.partner_owned,
        open_matrix_total: matrix.total,
    };

    // Sensitivity.
    let sweep_specs = if spec.sweep.is_empty() {
        default_sweep(&scn, &kind, &subject)
    } else {
        spec.sweep.clone()
    };
    let sensitivity = run_sweep(&run_src, &scn, &sweep_specs, &figures)?;

    // Timing.
    let timing = if kind == "clock" {
        build_timing(
            &scn,
            &subject,
            &result,
            &spec.holdover.clone().unwrap_or_default(),
        )?
    } else {
        None
    };

    let framework_mapping = build_framework(timing.as_ref(), &figures);

    let input_sha256 = sha_hex(src.as_bytes());
    let identification = Identification {
        title: spec
            .title
            .clone()
            .unwrap_or_else(|| format!("Study dossier — {kind} scenario")),
        reference: spec
            .reference
            .clone()
            .unwrap_or_else(|| format!("KSH-PRO-TN-{}", &input_sha256[..12])),
        issue: "1".into(),
        document_type: "Technical note (study dossier)".into(),
        scenario_kind: kind.clone(),
        engine_version: engine_version_of(&result)?,
        pro_version: crate::version().to_string(),
        input_sha256,
        run_input_sha256: sha_hex(run_src.as_bytes()),
        scenario_hash: result_hash(&result),
        seed: result.get("seed").and_then(|s| s.as_i64()),
        classification: "Kshana Pro — commercial in confidence. Not for public distribution."
            .into(),
    };

    let trade = if kind == crate::packs::trade_study_pack::TradeStudyScenario::KIND {
        trade_appendix(&run_src)?
    } else {
        None
    };
    let has_trade = trade.is_some();
    let assurance = build_assurance(
        &identification,
        &figures,
        &tier_split,
        &sensitivity,
        timing.as_ref(),
        determinism_identical,
        trade,
    );

    let scope = vec![
        format!(
            "This note records the evidence for one scenario of kind '{kind}' run on the open Kshana engine with the Kshana Pro overlay."
        ),
        "It states what was run, which figures resulted, which of them are VALIDATED against an external reference and which are MODELLED, how sensitive they are to the key inputs, and how the behaviour reads against published resilience frameworks.".into(),
        "It is an engineering study. It is not a certification, an accreditation, or a statement of conformance to any framework or standard, and it carries no endorsement from any agency or standards body.".into(),
    ];
    let mut method = vec![
        "M1. The scenario is run through the Kshana Pro superset dispatcher (every open scenario kind and every registered Pro pack). A [dossier] block, if present, is removed first; the text actually run is recorded in full (run_scenario_toml) with its SHA-256.".to_string(),
        "M2. The baseline is run twice in the same process and the two results compared, as the reproducibility test.".into(),
        "M3. Each figure's tier is read from the open verification matrix through kshana::fom_label::tier_for. A figure with no owning row is labelled MODELLED.".into(),
        "M4. One-at-a-time sensitivity sweep, as described in section 5.2.".into(),
        "M5. Framework mapping: the during-threat behaviour of the run is compared with the level definitions of RD-1 Table 1. Requirements a simulation cannot verify are recorded as not assessed.".into(),
        "M6. Assurance artefacts: a GSN argument (RD-5), a VCD (RD-4) and a W3C-PROV record (RD-6) whose Merkle root binds the input, the baseline and every sweep case.".into(),
    ];
    if timing.is_some() {
        method.push("M7. Timing: the subject clock's parameters feed the Pro holdover uncertainty band x(t) = x0 + y0·t + ½·D·t² ± k·σ(t). The in-spec coast, the margin over the denied window, and the MTIE of the band (checked against each limit as a constant ceiling) are reported for the scenario budget and for the published clock classes.".into());
    }
    if has_trade {
        method.push("M8. The trade study's own assurance gate, GSN, VCD and provenance are appended unchanged (Appendix A).".into());
    }

    let mut limitations = vec![
        "All figures come from a simulation with modelled inputs. No measured-device, field or flight data is used.".to_string(),
        "The sensitivity sweep is one-at-a-time around one baseline; it does not explore interactions or the full input space.".into(),
        format!("Framework readings are {FRAMEWORK_LABEL}. Requirements 1 to 8 of RD-1 are equipment requirements this dossier does not verify."),
        "IEEE P1952 sub-score definitions are not public; the core-function readings are not P1952 scores.".into(),
        "The document date is not part of the reproducibility hash; the hash covers content only.".into(),
    ];
    if timing.is_some() {
        limitations.push("The closed-form coast model has no flicker-frequency term; a flicker-dominated clock is under-modelled in the holdover band.".into());
        limitations.push("MTIE is checked against each class's constant time-error limit; the observation-interval-dependent ITU-T MTIE masks are not applied.".into());
    }

    let body = DossierBody {
        identification,
        scope,
        reference_documents: reference_documents(timing.is_some(), has_trade),
        method,
        figures,
        tier_split,
        sensitivity,
        timing,
        framework_mapping,
        assurance,
        limitations,
        run_scenario_toml: run_src,
    };
    let canonical = serde_json::to_string(&body)
        .map_err(|e| invalid(format!("dossier does not serialise: {e}")))?;
    Ok(StudyDossier {
        body,
        result_hash: sha_hex(canonical.as_bytes()),
        issued_utc: opts.issued_utc.clone(),
    })
}

// ─── Rendering ─────────────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(x: f64) -> String {
    if x == 0.0 {
        "0".into()
    } else if x.abs() >= 1e-3 && x.abs() < 1e6 {
        let s = format!("{x:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        format!("{x:.4e}")
    }
}

fn opt(x: Option<f64>) -> String {
    x.map(num).unwrap_or_else(|| "—".into())
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

impl StudyDossier {
    /// Pretty JSON of the whole dossier (body, hash and stamp).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("dossier serialises")
    }

    /// Self-contained HTML (no external requests), laid out as a technical note.
    pub fn to_html(&self) -> String {
        let b = &self.body;
        let id = &b.identification;
        let mut h = String::new();
        h.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\"/>\n");
        h.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\n");
        h.push_str(&format!("<title>{}</title>\n", esc(&id.title)));
        h.push_str("<style>\n:root{--bg:#f6f4f0;--ink:#212121;--muted:#5b5446;--rule:#c7aa66;--panel:#ffffff;--navy:#0e1726;--ok:#2f6b3a;--bad:#9a2e22}\n@media (prefers-color-scheme: dark){:root:not([data-theme=\"light\"]){--bg:#0e1726;--ink:#ece6d8;--muted:#b6985a;--rule:#c7aa66;--panel:#131923;--navy:#ece6d8;--ok:#8fc79a;--bad:#e59488}}\n:root[data-theme=\"dark\"]{--bg:#0e1726;--ink:#ece6d8;--muted:#b6985a;--rule:#c7aa66;--panel:#131923;--navy:#ece6d8;--ok:#8fc79a;--bad:#e59488}\nbody{background:var(--bg);color:var(--ink);font-family:Georgia,'Times New Roman',serif;line-height:1.5;margin:0;padding:0 16px}\nmain{max-width:980px;margin:0 auto;padding:24px 0 48px}\nh1{font-size:1.8rem;margin:.2rem 0}\nh2{font-size:1.25rem;border-bottom:2px solid var(--rule);padding-bottom:4px;margin-top:2rem}\nh3{font-size:1.05rem;margin-top:1.4rem}\n.banner{border:2px solid var(--rule);padding:10px 14px;font-weight:700;margin:14px 0}\n.table-wrap{overflow-x:auto}\ntable{border-collapse:collapse;width:100%;background:var(--panel);font-size:.9rem;margin:8px 0}\nth,td{border:1px solid color-mix(in srgb,var(--rule) 55%,transparent);padding:4px 7px;text-align:left;vertical-align:top}\nth{background:color-mix(in srgb,var(--rule) 18%,transparent)}\ncode,pre{font-family:ui-monospace,Menlo,Consolas,monospace;font-size:.82rem;word-break:break-all}\npre{white-space:pre-wrap;background:var(--panel);padding:8px;border:1px solid var(--rule)}\n.muted{color:var(--muted)}\n.VALIDATED{color:var(--ok);font-weight:700}\n.MODELLED,.PARTIAL{color:var(--muted);font-weight:700}\n.PASS{color:var(--ok);font-weight:700}\n.FAIL{color:var(--bad);font-weight:700}\n</style>\n</head>\n<body>\n<main>\n");

        h.push_str(&format!(
            "<p class=\"muted\">Ashforde OÜ · Kshana Pro · {}</p>\n<h1>{}</h1>\n<p class=\"muted\">{} · Reference {} · Issue {}{}</p>\n",
            esc(&id.document_type),
            esc(&id.title),
            esc(&id.classification),
            esc(&id.reference),
            esc(&id.issue),
            self.issued_utc
                .as_deref()
                .map(|u| format!(" · Issued {}", esc(u)))
                .unwrap_or_default(),
        ));
        h.push_str("<div class=\"banner\">MODELLED RESULTS. This is an engineering study, not a certification and not a statement of conformance. Framework readings are tentative, simulation-derived and not a certified level.</div>\n");

        // 1 Identification
        h.push_str("<h2>1. Identification</h2>\n<div class=\"table-wrap\"><table>\n");
        let rows: Vec<(&str, String)> = vec![
            ("Title", esc(&id.title)),
            ("Reference", esc(&id.reference)),
            (
                "Scenario kind",
                format!("<code>{}</code>", esc(&id.scenario_kind)),
            ),
            ("Open engine version", esc(&id.engine_version)),
            ("Kshana Pro version", esc(&id.pro_version)),
            (
                "Input file SHA-256",
                format!("<code>{}</code>", esc(&id.input_sha256)),
            ),
            (
                "Run input SHA-256",
                format!("<code>{}</code>", esc(&id.run_input_sha256)),
            ),
            (
                "Engine scenario hash",
                format!(
                    "<code>{}</code>",
                    esc(id
                        .scenario_hash
                        .as_deref()
                        .unwrap_or("(not reported by this kind)"))
                ),
            ),
            (
                "Seed",
                id.seed.map(|s| s.to_string()).unwrap_or_else(|| "—".into()),
            ),
            (
                "Dossier result hash (SHA-256)",
                format!("<code>{}</code>", esc(&self.result_hash)),
            ),
            (
                "Provenance Merkle root",
                format!("<code>{}</code>", esc(&b.assurance.provenance.merkle_root)),
            ),
        ];
        for (k, v) in rows {
            h.push_str(&format!("<tr><th>{k}</th><td>{v}</td></tr>\n"));
        }
        h.push_str("</table></div>\n");

        // 2 Scope
        h.push_str("<h2>2. Scope</h2>\n");
        for p in &b.scope {
            h.push_str(&format!("<p>{}</p>\n", esc(p)));
        }

        // 3 Documents
        h.push_str("<h2>3. Applicable and reference documents</h2>\n<p>Applicable documents: none. This note is not issued against a contract.</p>\n<div class=\"table-wrap\"><table>\n<tr><th>ID</th><th>Document</th><th>Issuer</th><th>Used for</th></tr>\n");
        for d in &b.reference_documents {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}<br/><code>{}</code></td><td>{}</td><td>{}</td></tr>\n",
                esc(&d.id),
                esc(&d.title),
                esc(&d.source),
                esc(&d.issuer),
                esc(&d.used_for)
            ));
        }
        h.push_str("</table></div>\n");

        // 4 Method
        h.push_str("<h2>4. Method</h2>\n<ul>\n");
        for m in &b.method {
            h.push_str(&format!("<li>{}</li>\n", esc(m)));
        }
        h.push_str("</ul>\n");

        // 5 Results
        h.push_str("<h2>5. Results</h2>\n<h3>5.1 Figures and their tier</h3>\n");
        let s = &b.tier_split;
        h.push_str(&format!(
            "<p>{} VALIDATED · {} MODELLED · {} PARTIAL of {} figures. For context, the open engine's verification matrix holds {} VALIDATED, {} MODELLED and {} partner-owned rows of {}.</p>\n",
            s.validated, s.modelled, s.partial, s.total,
            s.open_matrix_validated, s.open_matrix_modelled, s.open_matrix_partner_owned, s.open_matrix_total
        ));
        h.push_str("<div class=\"table-wrap\"><table>\n<tr><th>Figure</th><th>Value</th><th>Unit</th><th>Tier</th><th>Tier basis</th></tr>\n");
        for f in &b.figures {
            h.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td>{}</td></tr>\n",
                esc(&f.path),
                num(f.value),
                esc(f.unit.as_deref().unwrap_or("—")),
                esc(&f.tier),
                esc(&f.tier),
                esc(&f.tier_basis)
            ));
        }
        h.push_str("</table></div>\n");

        h.push_str("<h3>5.2 One-at-a-time sensitivity</h3>\n");
        h.push_str(&format!("<p>{}</p>\n", esc(&b.sensitivity.method)));
        if b.sensitivity.cases.is_empty() {
            h.push_str("<p>No sweep parameters were declared or defaulted for this scenario kind. Declare <code>[[dossier.sweep]]</code> entries to add them.</p>\n");
        } else {
            h.push_str("<div class=\"table-wrap\"><table>\n<tr><th>Parameter</th><th>Base</th><th>Case</th><th>Figure</th><th>Value</th><th>Change</th><th>Elasticity</th></tr>\n");
            for c in &b.sensitivity.cases {
                let case = match c.factor {
                    Some(f) => format!("× {}", num(f)),
                    None => format!("= {}", num(c.value)),
                };
                if let Some(e) = &c.error {
                    h.push_str(&format!(
                        "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td colspan=\"4\" class=\"FAIL\">{}</td></tr>\n",
                        esc(&c.param), num(c.base_value), case, esc(e)
                    ));
                    continue;
                }
                for d in &c.deltas {
                    h.push_str(&format!(
                        "<tr><td><code>{}</code></td><td>{}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                        esc(&c.param),
                        num(c.base_value),
                        case,
                        esc(&d.path),
                        opt(d.value),
                        opt(d.delta),
                        opt(d.elasticity)
                    ));
                }
            }
            h.push_str("</table></div>\n<p><strong>Most influential input per figure</strong> (largest absolute elasticity):</p>\n<ul>\n");
            for d in &b.sensitivity.drivers {
                h.push_str(&format!(
                    "<li><code>{}</code>: {}</li>\n",
                    esc(&d.figure),
                    match (&d.param, d.max_abs_elasticity) {
                        (Some(p), Some(e)) =>
                            format!("<code>{}</code> (|elasticity| {})", esc(p), num(e)),
                        _ => "no defined elasticity".into(),
                    }
                ));
            }
            h.push_str("</ul>\n");
        }
        for sk in &b.sensitivity.skipped {
            h.push_str(&format!("<p class=\"muted\">Skipped: {}</p>\n", esc(sk)));
        }

        h.push_str("<h3>5.3 Timing and holdover</h3>\n");
        match &b.timing {
            None => h.push_str("<p>Not a clock scenario; no holdover analysis.</p>\n"),
            Some(t) => {
                h.push_str(&format!(
                    "<p>Subject clock <code>{}</code> ({}). White-frequency noise q_wf = {}, random-walk frequency noise q_rw = {}, residual frequency offset y0 = {}, aging D = {}, start error x0 = {} s, σ(y0) = {}, coverage factor k = {}. Longest GNSS-denied window: {} s. Interval tested: {} s. MTIE sampling step: {} s. Engine-simulated holdover (one seeded realisation): {} s. Tier: <span class=\"MODELLED\">{}</span>.</p>\n",
                    esc(&t.clock_id), esc(&t.clock_provenance), num(t.q_wf), num(t.q_rw), num(t.y0), num(t.drift), num(t.x0_s), num(t.sigma_y0), num(t.k),
                    opt(t.longest_denial_s), opt(t.interval_s), num(t.mtie_tau0_s), opt(t.engine_holdover_s), esc(&t.tier)
                ));
                h.push_str("<div class=\"table-wrap\"><table>\n<tr><th>Limit</th><th>Threshold (s)</th><th>In-spec coast (s)</th><th>Holds over interval</th><th>Envelope at interval (s)</th><th>Margin (s)</th><th>MTIE within limit</th><th>Source</th></tr>\n");
                for m in std::iter::once(&t.budget).chain(t.clock_classes.iter()) {
                    h.push_str(&format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td>{}</td></tr>\n",
                        esc(&m.name),
                        num(m.threshold_s),
                        m.in_spec_coast_s.map(num).unwrap_or_else(|| "unbounded".into()),
                        if m.holds_over_interval { "PASS" } else { "FAIL" },
                        yes_no(m.holds_over_interval),
                        num(m.envelope_at_interval_s),
                        num(m.margin_at_interval_s),
                        if m.mtie_compliant { "PASS" } else { "FAIL" },
                        yes_no(m.mtie_compliant),
                        esc(&m.provenance)
                    ));
                }
                h.push_str("</table></div>\n");
                h.push_str(&format!("<p class=\"muted\">{}</p>\n", esc(&t.note)));
            }
        }

        let fm = &b.framework_mapping;
        h.push_str("<h3>5.4 Framework mapping</h3>\n");
        h.push_str(&format!("<div class=\"banner\">{}</div>\n", esc(&fm.label)));
        h.push_str(&format!("<p>{}</p>\n", esc(&fm.basis)));
        h.push_str("<div class=\"table-wrap\"><table>\n<tr><th>RPCF level (RD-1 Table 1)</th><th>During-threat behaviour in this run</th><th>Minimum requirements</th></tr>\n");
        for l in &fm.rpcf_levels {
            let reqs: Vec<String> = l
                .requirements
                .iter()
                .map(|r| {
                    format!(
                        "<strong>{}.</strong> {} <em>{}</em>",
                        r.number,
                        esc(&r.text),
                        esc(&r.reading)
                    )
                })
                .collect();
            h.push_str(&format!(
                "<tr><td><strong>Level {}</strong>: {}</td><td>{}</td><td>{}</td></tr>\n",
                l.level,
                esc(&l.definition),
                esc(&l.behaviour),
                reqs.join("<br/>")
            ));
        }
        h.push_str("</table></div>\n<p><strong>Core-function readings</strong> (RD-1 section 5.2), read per function, never combined:</p>\n<div class=\"table-wrap\"><table>\n<tr><th>Function</th><th>Driver</th><th>Value</th><th>Reading</th></tr>\n");
        for c in &fm.core_functions {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(&c.function),
                esc(&c.driver),
                opt(c.value),
                esc(&c.reading)
            ));
        }
        h.push_str("</table></div>\n");
        h.push_str(&format!("<p class=\"muted\">{}</p>\n", esc(&fm.p1952_note)));

        let a = &b.assurance;
        h.push_str("<h3>5.5 Assurance artefacts</h3>\n<p><strong>Goal Structuring Notation argument</strong> (RD-5)</p>\n<div class=\"table-wrap\"><table>\n<tr><th>Node</th><th>Type</th><th>Statement</th><th>Supported by</th><th>In context of</th></tr>\n");
        for n in &a.gsn.nodes {
            let links = |kind: crate::suite::gsn::GsnLinkKind| -> String {
                a.gsn
                    .links
                    .iter()
                    .filter(|l| l.from == n.id && l.kind == kind)
                    .map(|l| l.to.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(&n.id),
                n.kind.tag(),
                esc(&n.statement),
                links(crate::suite::gsn::GsnLinkKind::SupportedBy),
                links(crate::suite::gsn::GsnLinkKind::InContextOf)
            ));
        }
        h.push_str("</table></div>\n<p><strong>Verification Control Document</strong> (RD-4 structure; requirement identifiers are generated locally for this dossier)</p>\n<div class=\"table-wrap\"><table>\n<tr><th>Requirement</th><th>Text</th><th>Method</th><th>Evidence</th><th>Status</th><th>Note</th></tr>\n");
        for it in &a.vcd.items {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td>{}</td></tr>\n",
                esc(&it.req_id),
                esc(&it.requirement),
                it.method.label(),
                esc(&it.evidence),
                it.status.label().replace('/', ""),
                it.status.label(),
                esc(&it.note)
            ));
        }
        h.push_str("</table></div>\n<p><strong>W3C-PROV provenance</strong> (RD-6)</p>\n");
        h.push_str(&format!(
            "<p>Engine: <code>{}</code>. {} content hashes bound. Merkle root <code>{}</code>.</p>\n",
            esc(&a.provenance.record.engine_commit),
            a.provenance.record.input_hashes.len(),
            esc(&a.provenance.merkle_root)
        ));

        // 6 Limitations
        h.push_str("<h2>6. Limitations</h2>\n<ul>\n");
        for l in &b.limitations {
            h.push_str(&format!("<li>{}</li>\n", esc(l)));
        }
        h.push_str("</ul>\n");

        if let Some(t) = &a.trade {
            h.push_str("<h2>Appendix A. Trade-study assurance material</h2>\n");
            h.push_str(&format!(
                "<p>Winner: {}. Trade provenance Merkle root <code>{}</code>.</p>\n",
                esc(t.winner.as_deref().unwrap_or("(none)")),
                esc(&t.merkle_root)
            ));
            for md in [
                &t.gate_markdown,
                &t.gsn_markdown,
                &t.vcd_markdown,
                &t.provenance_markdown,
            ] {
                h.push_str(&format!("<pre>{}</pre>\n", esc(md)));
            }
        }

        h.push_str("<h2>Appendix B. Scenario as run</h2>\n");
        h.push_str(&format!("<pre>{}</pre>\n", esc(&b.run_scenario_toml)));

        h.push_str("<h2>Appendix C. Abbreviations</h2>\n<ul>\n");
        for (k, v) in [
            (
                "CISA",
                "Cybersecurity and Infrastructure Security Agency (US)",
            ),
            ("DHS", "Department of Homeland Security (US)"),
            ("ECSS", "European Cooperation for Space Standardization"),
            ("ePRTC", "enhanced primary reference time clock"),
            ("ESA", "European Space Agency"),
            ("GNSS", "Global Navigation Satellite System"),
            ("GSN", "Goal Structuring Notation"),
            ("IEEE", "Institute of Electrical and Electronics Engineers"),
            (
                "ITU-T",
                "International Telecommunication Union, Telecommunication Standardization Sector",
            ),
            ("JSON", "JavaScript Object Notation"),
            ("MTIE", "maximum time interval error"),
            ("PNT", "positioning, navigation and timing"),
            ("PROV", "the W3C provenance data model"),
            ("PRTC", "primary reference time clock"),
            ("RPCF", "Resilient PNT Conformance Framework"),
            ("SHA-256", "Secure Hash Algorithm, 256-bit"),
            (
                "TOML",
                "Tom's Obvious Minimal Language (the scenario file format)",
            ),
            ("VCD", "Verification Control Document"),
            ("W3C", "World Wide Web Consortium"),
        ] {
            h.push_str(&format!("<li><strong>{k}</strong>: {}</li>\n", esc(v)));
        }
        h.push_str("</ul>\n</main>\n</body>\n</html>\n");
        h
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const CLOCK: &str = r#"kind = "clock"
seed = 42
threshold_ns = 20.0
[time]
step_s = 10.0
duration_s = 1200.0
[gnss]
windows = [
  { t0 = 0.0,   t1 = 600.0,  state = "nominal" },
  { t0 = 600.0, t1 = 1200.0, state = "denied" },
]
[clock_quantum]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
[clock_classical]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
"#;

    #[test]
    fn scenario_without_a_dossier_block_is_run_verbatim() {
        let (run, spec) = split_scenario(CLOCK).unwrap();
        assert_eq!(run, CLOCK);
        assert!(spec.figures.is_empty() && spec.sweep.is_empty());
    }

    #[test]
    fn dossier_block_is_removed_before_the_run() {
        let src = format!("{CLOCK}\n[dossier]\ntitle = \"T\"\nsubject = \"classical\"\n");
        let (run, spec) = split_scenario(&src).unwrap();
        assert!(!run.contains("[dossier]"));
        assert_eq!(spec.title.as_deref(), Some("T"));
        assert_eq!(spec.subject.as_deref(), Some("classical"));
    }

    #[test]
    fn unknown_dossier_keys_are_rejected() {
        let src = format!("{CLOCK}\n[dossier]\nsubjekt = \"classical\"\n");
        assert!(split_scenario(&src).is_err());
    }

    #[test]
    fn with_param_changes_only_the_named_number() {
        let out = with_param(CLOCK, "clock_quantum.q_wf", 1.0e-19).unwrap();
        let v: toml::Value = toml::from_str(&out).unwrap();
        assert_eq!(toml_number(&v, "clock_quantum.q_wf"), Some(1.0e-19));
        assert_eq!(toml_number(&v, "clock_classical.q_wf"), Some(9.0e-20));
        assert!(with_param(CLOCK, "clock_quantum.id", 1.0).is_err());
        assert!(with_param(CLOCK, "nope.q_wf", 1.0).is_err());
    }

    #[test]
    fn longest_denial_reads_the_gnss_windows() {
        let v: toml::Value = toml::from_str(CLOCK).unwrap();
        assert_eq!(longest_denial_s(&v), Some(600.0));
    }

    #[test]
    fn fom_figures_default_to_the_subject_side() {
        let (run, _) = split_scenario(CLOCK).unwrap();
        let r = run_json(&run).unwrap();
        let q = fom_paths(&r, "quantum");
        assert!(!q.is_empty());
        assert!(q.iter().all(|p| p.starts_with("quantum.fom.")));
        assert!(q.contains(&"quantum.fom.holdover_s".to_string()));
    }

    #[test]
    fn unowned_figures_are_modelled_never_validated() {
        let (tier, basis) = tier_of("some.custom.figure");
        assert_eq!(tier, "MODELLED");
        assert!(basis.contains("conservative"));
    }

    #[test]
    fn dossier_hash_is_deterministic_and_excludes_the_stamp() {
        let a = build(CLOCK, &DossierOptions::default()).unwrap();
        let b = build(
            CLOCK,
            &DossierOptions {
                issued_utc: Some("2026-09-25T00:00:00Z".into()),
            },
        )
        .unwrap();
        assert_eq!(a.result_hash, b.result_hash);
        assert_eq!(a.result_hash.len(), 64);
        assert!(a.body.assurance.determinism_identical);
    }

    #[test]
    fn framework_mapping_is_always_labelled_tentative() {
        let d = build(CLOCK, &DossierOptions::default()).unwrap();
        let fm = &d.body.framework_mapping;
        assert_eq!(fm.label, FRAMEWORK_LABEL);
        assert!(fm.basis.contains(FRAMEWORK_LABEL));
        assert_eq!(fm.rpcf_levels.len(), 4);
        // Requirements 1..=8 all appear, none claimed as verified.
        let reqs: Vec<u8> = fm
            .rpcf_levels
            .iter()
            .flat_map(|l| l.requirements.iter().map(|r| r.number))
            .collect();
        assert_eq!(reqs, (1..=8).collect::<Vec<u8>>());
        assert!(fm
            .rpcf_levels
            .iter()
            .flat_map(|l| &l.requirements)
            .all(|r| r.reading.starts_with("not assessed")));
        // Level 4 is never read from a simulation.
        assert!(fm.tentative_behavioural_level.unwrap_or(0) <= 3);
        let html = d.to_html();
        assert!(html.contains(FRAMEWORK_LABEL));
    }

    #[test]
    fn a_blown_budget_is_not_read_as_bounded() {
        // q_wf = 9e-20 over a 600 s coast gives a 1-sigma of about 7.3 ns; with k = 2
        // plus a 30 ns start error the band leaves the 20 ns budget, so the RPCF
        // level-3 behaviour must NOT be claimed.
        let src = format!("{CLOCK}\n[dossier]\n[dossier.holdover]\nx0_ns = 30.0\n");
        let d = build(&src, &DossierOptions::default()).unwrap();
        let t = d.body.timing.as_ref().expect("clock scenario has timing");
        assert!(!t.budget.holds_over_interval);
        assert_eq!(
            d.body.framework_mapping.tentative_behavioural_level,
            Some(2)
        );
    }

    #[test]
    fn a_quiet_clock_inside_budget_reads_bounded() {
        let quiet = CLOCK.replace("q_wf = 9.0e-20", "q_wf = 1.0e-24");
        let d = build(&quiet, &DossierOptions::default()).unwrap();
        let t = d.body.timing.as_ref().unwrap();
        assert!(t.budget.holds_over_interval, "{:?}", t.budget);
        assert_eq!(
            d.body.framework_mapping.tentative_behavioural_level,
            Some(3)
        );
    }

    #[test]
    fn a_zero_base_parameter_needs_absolute_values() {
        let src = format!(
            "{CLOCK}\n[dossier]\n[[dossier.sweep]]\nparam = \"clock_quantum.y0\"\nfactors = [2.0]\nvalues = [1.0e-12]\n"
        );
        let d = build(&src, &DossierOptions::default()).unwrap();
        assert_eq!(d.body.sensitivity.cases.len(), 1);
        assert_eq!(d.body.sensitivity.skipped.len(), 1);
        assert_eq!(d.body.sensitivity.cases[0].value, 1.0e-12);
    }
}
