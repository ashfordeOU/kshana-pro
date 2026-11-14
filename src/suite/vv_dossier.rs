// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Agency V&V credibility dossier (the capstone an ESA/NASA reviewer reads)
//!
//! Composes the whole assurance stack into one **honest model-credibility dossier**,
//! shaped against the two standards an agency M&S reviewer applies:
//!
//! * **NASA-STD-7009B** — the *Credibility Assessment Scale* (CAS): the overlay's
//!   modelling & simulation scored across the eight standard factors (Verification,
//!   Validation, Input Pedigree, Results Uncertainty, Results Robustness, Use History,
//!   M&S Management, People Qualification), each on the 0–4 level, with an evidence
//!   string and an achieved-vs-not rationale.
//! * **ASME V&V 40** — a *Context-of-Use* statement: the quantity of interest, the
//!   decision it informs, the model risk (influence × consequence), and the
//!   credibility **required vs achieved** with the gap stated plainly.
//!
//! It pulls the concrete evidence already produced elsewhere in the suite — the
//! fail-closed [assurance gate][super::gate] verdict, the W3C-PROV + Merkle root
//! ([provenance][super::provenance]), the count of gate checks closed, the open core's
//! [`kshana::verification`] Validated/Modelled split, and the L1 twin validation
//! reports — and binds them into structured data plus a markdown dossier.
//!
//! ## Honesty firewall (this artifact is *about* honesty — it must itself be honest)
//!
//! * **The dossier is Modelled.** It is a credibility-assessment *instrument*, not a
//!   measured result; nothing here is Validated.
//! * **Validation is firewall-capped.** The open kernels (ADEV / ThéoH / AHP / WSM)
//!   are externally Validated *in the open core*, but every composed L1/L3 capability
//!   is Modelled — there is **no flight or measured-device data** behind the composite
//!   quantity of interest. So the CAS *Validation* factor cannot exceed
//!   [`FIREWALL_VALIDATION_CAP`] (level 1: referent exists for components only); it is
//!   level 0 when no twin reproduction evidence is even supplied (fail-closed).
//! * **Use History is honestly low.** This is a new system with CI/demonstrator runs
//!   only and no operational or flight history → capped at
//!   [`USE_HISTORY_CAP`] (level 1).
//! * **People Qualification is not assessed.** No personnel-competency records are
//!   claimed; the factor is recorded as *not assessed* (`None`), never invented.
//! * **The overall credibility never exceeds the weakest assessed factor** — it is the
//!   minimum across the assessed factors, by construction.
//! * The standards are cited **structurally**. The 0–4 levels here are this artifact's
//!   honest, evidence-based assessment; they are **not** a certified score, a formal
//!   accreditation, or a NASA/ASME-endorsed rating, and no certified score thresholds
//!   are fabricated.

use kshana::verification::{summarize, verification_matrix, MatrixSummary};

use super::gate::{assurance_gate, GateOutcome, GateVerdict};
use super::provenance::build_provenance;
use super::TradeReport;
use crate::twin::ValidationReport;

/// The hard firewall cap on the CAS **Validation** factor. The composed L1/L3
/// capability has no measured-device or flight referent, so its model-level
/// validation cannot exceed *level 1* ("referent data exists for components only"),
/// no matter how strong the rest of the evidence is.
pub const FIREWALL_VALIDATION_CAP: u8 = 1;

/// The honest cap on the CAS **Use History** factor: a new system with
/// CI/demonstrator runs only and no operational or flight history → *level 1*.
pub const USE_HISTORY_CAP: u8 = 1;

/// The CAS level (representative) this context of use would require for the model to
/// be the *sole* quantitative basis of the architecture decision. Representative
/// engineering judgement, consistent with how the rest of the overlay treats
/// thresholds — **not** a certified NASA/ASME threshold.
pub const COU_CREDIBILITY_REQUIRED: u8 = 3;

/// The eight NASA-STD-7009B Credibility Assessment Scale factors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CasFactor {
    Verification,
    Validation,
    InputPedigree,
    ResultsUncertainty,
    ResultsRobustness,
    UseHistory,
    MsManagement,
    PeopleQualification,
}

impl CasFactor {
    /// The factor's name as it appears in NASA-STD-7009B.
    pub fn label(self) -> &'static str {
        match self {
            CasFactor::Verification => "Verification",
            CasFactor::Validation => "Validation",
            CasFactor::InputPedigree => "Input Pedigree",
            CasFactor::ResultsUncertainty => "Results Uncertainty",
            CasFactor::ResultsRobustness => "Results Robustness",
            CasFactor::UseHistory => "Use History",
            CasFactor::MsManagement => "M&S Management",
            CasFactor::PeopleQualification => "People Qualification",
        }
    }

    /// The question the factor answers (plain-language summary of the standard's intent).
    pub fn question(self) -> &'static str {
        match self {
            CasFactor::Verification => "Were the M&S equations implemented and solved correctly?",
            CasFactor::Validation => "Do the M&S results agree with the real-world referent?",
            CasFactor::InputPedigree => "How authoritative and traceable are the inputs?",
            CasFactor::ResultsUncertainty => "Is the uncertainty in the results quantified?",
            CasFactor::ResultsRobustness => "How sensitive are the results to assumptions?",
            CasFactor::UseHistory => "Has the M&S been used successfully before?",
            CasFactor::MsManagement => {
                "Is the M&S under configuration control and process discipline?"
            }
            CasFactor::PeopleQualification => "Are the people who produced the M&S qualified?",
        }
    }

    /// The NASA-STD-7009B CAS category this factor belongs to.
    pub fn category(self) -> CasCategory {
        match self {
            CasFactor::Verification | CasFactor::Validation => CasCategory::Development,
            CasFactor::InputPedigree
            | CasFactor::ResultsUncertainty
            | CasFactor::ResultsRobustness => CasCategory::Operations,
            CasFactor::UseHistory | CasFactor::MsManagement | CasFactor::PeopleQualification => {
                CasCategory::SupportingEvidence
            }
        }
    }
}

/// The three NASA-STD-7009B CAS factor groupings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CasCategory {
    Development,
    Operations,
    SupportingEvidence,
}

impl CasCategory {
    pub fn label(self) -> &'static str {
        match self {
            CasCategory::Development => "M&S Development",
            CasCategory::Operations => "M&S Operations",
            CasCategory::SupportingEvidence => "Supporting Evidence",
        }
    }
}

/// Plain-language summary of a 0–4 CAS level. **Not** the certified NASA rubric — a
/// short gloss of the scale's intent, so the rendered table is readable.
pub fn cas_level_name(level: u8) -> &'static str {
    match level {
        0 => "insufficient / not evidenced",
        1 => "minimal",
        2 => "moderate (partial)",
        3 => "substantial",
        _ => "strong / fully demonstrated",
    }
}

/// One factor's scored assessment.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CasAssessment {
    pub factor: CasFactor,
    pub category: CasCategory,
    /// Achieved level 0–4, or `None` when the factor is **not assessed** (never
    /// invented — e.g. People Qualification).
    pub level: Option<u8>,
    /// The maximum level justifiable on the available evidence (the honest ceiling).
    pub cap: u8,
    /// What backs the level.
    pub evidence: String,
    /// Achieved-vs-not rationale, including why the cap applies.
    pub rationale: String,
    /// `true` when the honesty firewall (not merely thin evidence) sets the cap.
    pub firewall_capped: bool,
}

/// Risk band for the ASME V&V 40 context of use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum RiskLevel {
    Low,
    Moderate,
    High,
}

impl RiskLevel {
    pub fn label(self) -> &'static str {
        match self {
            RiskLevel::Low => "Low",
            RiskLevel::Moderate => "Moderate",
            RiskLevel::High => "High",
        }
    }
}

/// Conservative V&V 40-style model-risk matrix: model risk rises with both how much
/// the decision relies on the model (influence) and how severe a wrong decision is
/// (consequence). Biased upward (any High → at least High/Moderate) so the artifact
/// demands *more* credibility than it claims, never less.
fn risk_matrix(influence: RiskLevel, consequence: RiskLevel) -> RiskLevel {
    use RiskLevel::*;
    match (influence, consequence) {
        (High, High) | (High, Moderate) | (Moderate, High) => High,
        (High, Low) | (Moderate, Moderate) | (Low, High) => Moderate,
        (Moderate, Low) | (Low, Moderate) | (Low, Low) => Low,
    }
}

/// ASME V&V 40-2018 context-of-use statement: the quantity of interest, the decision
/// it informs, the model risk, and the credibility required vs achieved.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextOfUse {
    /// The model output that informs the decision.
    pub quantity_of_interest: String,
    /// How the model output is used in the decision (and the scope it is *not* used for).
    pub intended_use: String,
    /// How much the decision relies on the model.
    pub model_influence: RiskLevel,
    /// Severity if the decision is wrong.
    pub decision_consequence: RiskLevel,
    /// Derived model risk (influence × consequence).
    pub model_risk: RiskLevel,
    /// CAS level this context would need for *sole* reliance on the model (representative).
    pub credibility_required: u8,
    /// CAS overall level actually achieved.
    pub credibility_achieved: u8,
    /// The honest gap statement.
    pub gap: String,
}

/// The L1 twin reproduction evidence folded into the dossier (Input Pedigree +
/// Validation signals). Built from a [`ValidationReport`] plus whether the device's
/// input curve is datasheet-cited (vs representative-class).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TwinValidationEvidence {
    pub device: String,
    pub max_rel_err: f64,
    pub tolerance: f64,
    pub passed: bool,
    /// `true` when the device's published ADEV came from a cited datasheet, `false`
    /// when it is a representative-class curve (Input Pedigree discriminator).
    pub datasheet_cited: bool,
}

impl TwinValidationEvidence {
    /// Build from a twin [`ValidationReport`]; `datasheet_cited` records whether the
    /// input curve is datasheet-traceable (CSAC) or representative-class.
    pub fn from_report(
        device: impl Into<String>,
        report: &ValidationReport,
        datasheet_cited: bool,
    ) -> Self {
        Self {
            device: device.into(),
            max_rel_err: report.max_rel_err,
            tolerance: report.tolerance,
            passed: report.passed,
            datasheet_cited,
        }
    }
}

/// The composed evidence the dossier rests on, pulled from the rest of the suite.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ComposedEvidence {
    /// The fail-closed L3 assurance-gate verdict.
    pub gate: GateVerdict,
    /// 64-hex W3C-PROV Merkle root over the decision.
    pub merkle_root: String,
    /// `true` iff the root is 64-hex and recomputes identically (tamper-evidence holds).
    pub merkle_stable: bool,
    /// Number of assurance-gate checks.
    pub checks_total: usize,
    /// Gate checks that passed.
    pub checks_closed: usize,
    /// `true` iff there is at least one check and every one passed.
    pub checks_all_closed: bool,
    /// Open-core verification-matrix Validated/Modelled/Partner split.
    pub matrix: MatrixSummary,
    /// L1 twin reproduction evidence.
    pub twin_validations: Vec<TwinValidationEvidence>,
}

/// The full credibility dossier: structured CAS + ASME V&V 40 + composed evidence.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CredibilityDossier {
    pub title: String,
    pub generated_by: String,
    pub cas_factors: Vec<CasAssessment>,
    /// Overall CAS level = minimum across the **assessed** factors.
    pub overall_level: u8,
    /// The factor(s) that bind the overall level (the weakest relevant factors).
    pub binding_factors: Vec<CasFactor>,
    pub context_of_use: ContextOfUse,
    pub evidence: ComposedEvidence,
    /// `true` only when every load-bearing piece of evidence is present and passing
    /// (gate PASS, decision present, Merkle stable, every gate check closed, a passing twin). Any
    /// gap flips this false — fail-closed.
    pub evidence_complete: bool,
    /// The overall credibility statement (never exceeds the weakest assessed factor).
    pub overall_statement: String,
}

/// Build the agency V&V credibility dossier for a completed trade study, folding in
/// the L1 twin validation evidence. Composes the gate, provenance, the gate-check count and the
/// open-core verification matrix; scores the NASA-STD-7009B CAS honestly (firewall
/// caps enforced); and states the ASME V&V 40 context of use with the credibility gap.
///
/// Fail-closed: missing or failing evidence lowers the affected factors (it is never
/// assumed to pass), `evidence_complete` flips false, and the overall level is the
/// minimum across the assessed factors.
pub fn build_dossier(
    report: &TradeReport,
    twin_validations: &[TwinValidationEvidence],
) -> CredibilityDossier {
    // ── Compose the existing evidence ──────────────────────────────────────────
    let gate = assurance_gate(report);
    let gate_passed = gate.outcome == GateOutcome::Pass;

    // Provenance / Merkle: recompute twice to prove the root is stable (tamper-evident).
    let p1 = build_provenance(report);
    let p2 = build_provenance(report);
    let merkle_root = p1.merkle_root.clone();
    let merkle_stable = merkle_root.len() == 64
        && merkle_root.chars().all(|c| c.is_ascii_hexdigit())
        && merkle_root == p2.merkle_root;

    let checks_total = gate.criteria.len();
    let checks_closed = gate.criteria.iter().filter(|c| c.passed).count();
    let checks_all_closed = checks_total > 0 && checks_closed == checks_total;

    let matrix = summarize(&verification_matrix());

    let has_decision = report.decision.is_some();
    let smaa_present = report
        .decision
        .as_ref()
        .map(|dr| !dr.smaa.is_empty())
        .unwrap_or(false);
    let sensitivity_present = report
        .decision
        .as_ref()
        .map(|dr| !dr.tornado.is_empty() || dr.min_flip.is_some())
        .unwrap_or(false);
    let twin_passed = twin_validations.iter().any(|t| t.passed);
    let datasheet_input = twin_validations.iter().any(|t| t.datasheet_cited);

    let evidence = ComposedEvidence {
        gate,
        merkle_root,
        merkle_stable,
        checks_total,
        checks_closed,
        checks_all_closed,
        matrix,
        twin_validations: twin_validations.to_vec(),
    };

    // ── NASA-STD-7009B Credibility Assessment Scale (honest, fail-closed) ───────
    let mut cas_factors: Vec<CasAssessment> = Vec::new();

    // Verification — equations implemented & solved correctly.
    let verification_level = if !merkle_stable {
        1 // integrity of the reproducible record not established → fail-closed
    } else if matrix.validated >= 1 {
        3 // referenced numerical kernels externally checked + composition reproducible
    } else {
        2
    };
    cas_factors.push(CasAssessment {
        factor: CasFactor::Verification,
        category: CasFactor::Verification.category(),
        level: Some(verification_level),
        cap: 3,
        evidence: format!(
            "Composed on open-core kernels of which {} are externally Validated to stated \
             tolerances (e.g. ADEV vs Stable32/NBS14, AHP eigenvector vs LAPACK, holdover \
             coast-variance vs SciPy Van-Loan); the composition is reproducible and \
             tamper-evident (Merkle root {}; stable: {}).",
            matrix.validated,
            &evidence.merkle_root[..evidence.merkle_root.len().min(16)],
            merkle_stable
        ),
        rationale: "Achieved: the underlying equations are implemented and cross-checked, and the \
             composed pipeline is deterministically reproducible. Capped at 3 (not 4): the \
             composed L1/L3 layers carry no independent formal SQA audit or certified \
             numerical-error bounds."
            .to_string(),
        firewall_capped: false,
    });

    // Validation — agreement with a real-world referent. FIREWALL-CAPPED.
    let validation_raw = if twin_passed { 1 } else { 0 };
    let validation_level = validation_raw.min(FIREWALL_VALIDATION_CAP);
    cas_factors.push(CasAssessment {
        factor: CasFactor::Validation,
        category: CasFactor::Validation.category(),
        level: Some(validation_level),
        cap: FIREWALL_VALIDATION_CAP,
        evidence: if twin_passed {
            format!(
                "L1 twin reproduces published device ADEV curve(s) within tolerance \
                 ({} reproduction report(s) supplied; best is a calibration fit, Modelled). \
                 Component kernels are externally Validated in the open core, but the composed \
                 quantity of interest has NO measured-device or flight comparison.",
                evidence.twin_validations.len()
            )
        } else {
            "No twin reproduction evidence supplied and no measured-device/flight referent \
             exists for the composed quantity of interest — fail-closed to level 0."
                .to_string()
        },
        rationale: format!(
            "FIREWALL CAP: the open kernels (ADEV/ThéoH/AHP/WSM) are externally Validated, but \
             every composed L1/L3 capability is Modelled with no flight or measured-device data, \
             so model-level Validation cannot exceed level {FIREWALL_VALIDATION_CAP} (referent \
             exists for components only). The gap to higher levels is measured-device and flight \
             referent data."
        ),
        firewall_capped: true,
    });

    // Input Pedigree — authority/traceability of inputs.
    let pedigree_level = if datasheet_input { 2 } else { 1 };
    cas_factors.push(CasAssessment {
        factor: CasFactor::InputPedigree,
        category: CasFactor::InputPedigree.category(),
        level: Some(pedigree_level),
        cap: 2,
        evidence: format!(
            "Mixed pedigree: {} input curve(s) datasheet-cited, the remainder \
             representative-class; the decision thresholds (TRL floor, SMAA/flip floors) are \
             representative engineering choices carried from the gate, not certified values.",
            evidence
                .twin_validations
                .iter()
                .filter(|t| t.datasheet_cited)
                .count()
        ),
        rationale:
            "Achieved up to 2 (some inputs traceable to a published datasheet). Capped at 2: \
             not every input is traceable to an authoritative source and the thresholds are \
             representative — neither is controlled to an agency configuration baseline."
                .to_string(),
        firewall_capped: false,
    });

    // Results Uncertainty — is the uncertainty quantified.
    let uncertainty_level = if has_decision && smaa_present && sensitivity_present {
        3
    } else if has_decision {
        2
    } else {
        1 // fail-closed: no decision-level uncertainty quantification present
    };
    cas_factors.push(CasAssessment {
        factor: CasFactor::ResultsUncertainty,
        category: CasFactor::ResultsUncertainty.category(),
        level: Some(uncertainty_level),
        cap: 3,
        evidence: "SMAA rank-1 acceptability under Dirichlet weight sampling, single-criterion \
             weight-flip distance, and a tornado sensitivity ranking quantify the decision's \
             uncertainty; the L1 holdover pack adds banded (UQ) coast predictions."
            .to_string(),
        rationale:
            "Achieved when the multi-criteria decision carries SMAA + sensitivity. Capped at 3: \
             input uncertainties are characterised over weights/assumptions, not propagated \
             from measured input covariances."
                .to_string(),
        firewall_capped: false,
    });

    // Results Robustness — sensitivity to assumptions.
    let robust_check_passed = evidence
        .gate
        .criteria
        .iter()
        .find(|c| c.name.contains("rank-stability"))
        .map(|c| c.passed)
        .unwrap_or(false);
    let robustness_level = if has_decision && gate_passed && robust_check_passed {
        3
    } else if has_decision {
        2
    } else {
        1 // fail-closed
    };
    cas_factors.push(CasAssessment {
        factor: CasFactor::ResultsRobustness,
        category: CasFactor::ResultsRobustness.category(),
        level: Some(robustness_level),
        cap: 3,
        evidence: format!(
            "Fail-closed assurance gate enforces winner rank-stability, Pareto-optimality and \
             a SMAA floor; gate verdict: {}.",
            match evidence.gate.outcome {
                GateOutcome::Pass => "PASS",
                GateOutcome::Fail => "FAIL",
            }
        ),
        rationale: "Achieved when the decision is present and the gate's robustness checks pass. \
             Capped at 3: robustness is assessed against weight/assumption perturbations only, \
             not against a measured operating envelope."
            .to_string(),
        firewall_capped: false,
    });

    // Use History — prior successful use. HONESTLY LOW (new system).
    cas_factors.push(CasAssessment {
        factor: CasFactor::UseHistory,
        category: CasFactor::UseHistory.category(),
        level: Some(USE_HISTORY_CAP),
        cap: USE_HISTORY_CAP,
        evidence: "Developmental/CI use only: the framework runs reproducibly in the overlay test \
             suite. No operational deployment, mission use, or flight history."
            .to_string(),
        rationale: format!(
            "HONEST CAP: a new system has no operational or flight use history, so this factor \
             cannot exceed level {USE_HISTORY_CAP}. Use history accrues only with operational \
             missions."
        ),
        firewall_capped: false,
    });

    // M&S Management — configuration control & process discipline.
    let management_level = if merkle_stable && checks_total > 0 {
        3
    } else if merkle_stable {
        2
    } else {
        1 // fail-closed
    };
    cas_factors.push(CasAssessment {
        factor: CasFactor::MsManagement,
        category: CasFactor::MsManagement.category(),
        level: Some(management_level),
        cap: 3,
        evidence: format!(
            "Tamper-evident W3C-PROV record with a SHA-256 Merkle root, reproducible open-core \
             scenario hashes, a version-pinned dependency graph (Cargo.lock), a fail-closed \
             assurance gate ({} of {} checks closed).",
            checks_closed, checks_total
        ),
        rationale:
            "Achieved: the M&S is under reproducible configuration control with tamper-evident \
             provenance. Capped at 3: no formal organisational QMS/ISO-9001 accreditation is \
             claimed for the development process."
                .to_string(),
        firewall_capped: false,
    });

    // People Qualification — NOT ASSESSED (never invented).
    cas_factors.push(CasAssessment {
        factor: CasFactor::PeopleQualification,
        category: CasFactor::PeopleQualification.category(),
        level: None,
        cap: 0,
        evidence: "No personnel-competency or qualification records are claimed by this artifact."
            .to_string(),
        rationale:
            "NOT ASSESSED: assessing this factor requires an organisational competency record \
             outside the software's scope. It is recorded as not-assessed (never invented); its \
             absence further limits any accreditation claim."
                .to_string(),
        firewall_capped: false,
    });

    // ── Overall: minimum across the ASSESSED factors (never exceeds the weakest) ─
    let assessed: Vec<(CasFactor, u8)> = cas_factors
        .iter()
        .filter_map(|f| f.level.map(|l| (f.factor, l)))
        .collect();
    let overall_level = assessed.iter().map(|(_, l)| *l).min().unwrap_or(0);
    let binding_factors: Vec<CasFactor> = assessed
        .iter()
        .filter(|(_, l)| *l == overall_level)
        .map(|(f, _)| *f)
        .collect();

    let evidence_complete =
        has_decision && gate_passed && merkle_stable && checks_all_closed && twin_passed;

    // ── ASME V&V 40 context of use ─────────────────────────────────────────────
    let qoi = if has_decision {
        format!(
            "The ranked architecture selection ('{}') and its weighted-sum score for: {}",
            report.winner.as_deref().unwrap_or("(no winner)"),
            report.title
        )
    } else {
        format!(
            "The single figure-of-merit ranking ('{}') for: {}",
            report.metric, report.title
        )
    };
    let model_influence = RiskLevel::High; // the trade study IS the quantitative basis of the down-select
    let decision_consequence = RiskLevel::Moderate; // Phase 0/A down-select: consequential but reversible
    let model_risk = risk_matrix(model_influence, decision_consequence);
    let gap = if overall_level >= COU_CREDIBILITY_REQUIRED {
        format!(
            "Achieved CAS level {overall_level} meets the representative required level \
             {COU_CREDIBILITY_REQUIRED} for this context of use."
        )
    } else {
        format!(
            "CREDIBILITY GAP: achieved CAS level {overall_level} < required level \
             {COU_CREDIBILITY_REQUIRED} for a {} model-risk context. The model is therefore NOT \
             sufficient on its own to drive the architecture decision; it is admissible as ONE \
             quantitative input among others at an early (Phase 0/A), reversible down-select, \
             with all Modelled gaps disclosed. Closing the gap to operational/flight decision \
             authority requires (1) measured-device and flight referent data (Validation), \
             (2) operational use history, and (3) independent V&V and personnel-qualification \
             records.",
            model_risk.label()
        )
    };
    let context_of_use = ContextOfUse {
        quantity_of_interest: qoi,
        intended_use:
            "Informs an early-phase (ECSS Phase 0/A) architecture down-select / feasibility \
             trade. It is NOT used for flight qualification, an operational go/no-go, or any \
             safety-of-life decision."
                .to_string(),
        model_influence,
        decision_consequence,
        model_risk,
        credibility_required: COU_CREDIBILITY_REQUIRED,
        credibility_achieved: overall_level,
        gap,
    };

    // ── Overall credibility statement (bounded by the weakest assessed factor) ──
    let binding_names: Vec<&str> = binding_factors.iter().map(|f| f.label()).collect();
    let overall_statement = format!(
        "Overall model credibility is bounded at NASA-STD-7009B CAS level {overall_level} \
         ({}), set by the weakest assessed factor(s): {}. People Qualification is not assessed. \
         This is feasibility-grade (Phase 0/A) credibility — admissible as one disclosed-gap \
         input to an architecture down-select, and explicitly NOT a flight qualification or a \
         formal accreditation. Evidence complete: {}. The gap to higher credibility is \
         measured-device/flight referent data (Validation) and operational use history.",
        cas_level_name(overall_level),
        binding_names.join(", "),
        evidence_complete,
    );

    CredibilityDossier {
        title: format!("Agency V&V credibility dossier — {}", report.title),
        generated_by: format!(
            "Kshana Pro v{} over the open Kshana engine (this dossier is Modelled)",
            crate::version()
        ),
        cas_factors,
        overall_level,
        binding_factors,
        context_of_use,
        evidence,
        evidence_complete,
        overall_statement,
    }
}

impl CredibilityDossier {
    /// `true` iff the overall credibility does not exceed any assessed factor (the
    /// invariant the dossier is built to guarantee).
    pub fn overall_within_factors(&self) -> bool {
        self.cas_factors
            .iter()
            .filter_map(|f| f.level)
            .all(|l| self.overall_level <= l)
    }

    /// Render the full markdown dossier — the document an agency M&S reviewer reads.
    pub fn markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", self.title));
        md.push_str(&format!("_{}._\n\n", self.generated_by));
        md.push_str(
            "> **Honesty banner.** This dossier is a credibility-assessment *instrument* — it \
             is **Modelled**, not a measured result, and is **not** a certified score or a \
             formal accreditation. It is structured per **NASA-STD-7009B** (Standard for \
             Models and Simulations — Credibility Assessment Scale) and **ASME V&V 40-2018** \
             (Assessing Credibility of Computational Models); the 0–4 levels are this \
             artifact's honest, evidence-based assessment, not a NASA/ASME-endorsed rating, \
             and no certified score thresholds are fabricated.\n\n",
        );

        // ASME V&V 40 — context of use.
        let cou = &self.context_of_use;
        md.push_str("## Context of use (ASME V&V 40-2018)\n\n");
        md.push_str(&format!(
            "- **Quantity of interest:** {}\n",
            cou.quantity_of_interest
        ));
        md.push_str(&format!("- **Intended use:** {}\n", cou.intended_use));
        md.push_str(&format!(
            "- **Model influence:** {} — **Decision consequence:** {} → **Model risk:** {} \
             (conservative; representative).\n",
            cou.model_influence.label(),
            cou.decision_consequence.label(),
            cou.model_risk.label()
        ));
        md.push_str(&format!(
            "- **Credibility required (representative, for sole reliance):** CAS level {} — \
             **achieved:** CAS level {}.\n",
            cou.credibility_required, cou.credibility_achieved
        ));
        md.push_str(&format!("- **Gap:** {}\n\n", cou.gap));

        // NASA-STD-7009B — CAS table.
        md.push_str("## Credibility Assessment Scale (NASA-STD-7009B)\n\n");
        md.push_str(
            "| Factor | Category | Level (0–4) | Cap | Evidence | Rationale |\n\
             |--------|----------|-------------|-----|----------|-----------|\n",
        );
        for f in &self.cas_factors {
            let level_s = match f.level {
                Some(l) => format!("{l} ({})", cas_level_name(l)),
                None => "not assessed".to_string(),
            };
            let cap_s = if f.firewall_capped {
                format!("{} (firewall)", f.cap)
            } else if f.level.is_none() {
                "—".to_string()
            } else {
                f.cap.to_string()
            };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                f.factor.label(),
                f.category.label(),
                level_s,
                cap_s,
                f.evidence,
                f.rationale,
            ));
        }
        md.push('\n');
        md.push_str(&format!(
            "**Overall credibility statement.** {}\n\n",
            self.overall_statement
        ));

        // Composed evidence.
        let ev = &self.evidence;
        md.push_str("## Composed evidence\n\n");
        md.push_str(&format!(
            "- **Assurance gate (fail-closed):** {} — {}\n",
            match ev.gate.outcome {
                GateOutcome::Pass => "PASS",
                GateOutcome::Fail => "FAIL",
            },
            ev.gate.summary
        ));
        md.push_str(&format!(
            "- **Provenance (W3C-PROV) Merkle root:** `{}` (stable / tamper-evident: {})\n",
            ev.merkle_root, ev.merkle_stable
        ));
        md.push_str(&format!(
            "- **Gate checks:** {} of {} closed (all closed: {})\n",
            ev.checks_closed, ev.checks_total, ev.checks_all_closed
        ));
        md.push_str(&format!(
            "- **Open-core verification matrix:** {} of {} rows externally Validated, {} \
             Modelled, {} partner-owned\n",
            ev.matrix.validated, ev.matrix.total, ev.matrix.modelled, ev.matrix.partner_owned
        ));
        if ev.twin_validations.is_empty() {
            md.push_str(
                "- **L1 twin reproduction:** none supplied (Validation fail-closed to 0)\n",
            );
        } else {
            md.push_str("- **L1 twin reproduction:**\n");
            for t in &ev.twin_validations {
                md.push_str(&format!(
                    "  - {} — max rel-err {:.4} vs tol {:.4} → {} (input pedigree: {})\n",
                    t.device,
                    t.max_rel_err,
                    t.tolerance,
                    if t.passed {
                        "reproduced"
                    } else {
                        "NOT reproduced"
                    },
                    if t.datasheet_cited {
                        "datasheet-cited"
                    } else {
                        "representative-class"
                    },
                ));
            }
        }
        md.push('\n');

        // Embed the underlying gate table for the reviewer to drill into.
        md.push_str(&ev.gate.markdown());

        md.push_str(
            "\n_This dossier composes the suite's own evidence (gate, W3C-PROV+Merkle) \
             and the open core's verification matrix into a single credibility \
             picture. It is deliberately scrupulous about its own honesty: Validation is \
             firewall-capped because the composed capability has no measured-device/flight \
             data, Use History is low because the system is new, and People Qualification is \
             not assessed. The whole value of the artifact is that it tells an agency the \
             HONEST credibility — including the gaps to flight._\n",
        );
        md
    }
}

#[cfg(test)]
mod tests {
    use super::super::decision::{TradeCriterion, Weighting};
    use super::super::{run_trade_study, GateThresholds, TradeSpace, TrlLevel, Variant};
    use super::*;
    use crate::twin::{calibrate, fit_report, DeviceSpec};

    fn clock_variant(quantum_id: &str, q_wf: f64) -> String {
        format!(
            "kind = \"clock\"\n\
             seed = 42\n\
             threshold_ns = 20.0\n\
             [time]\n\
             step_s = 10.0\n\
             duration_s = 3600.0\n\
             [gnss]\n\
             windows = [\n\
             {{ t0 = 0.0, t1 = 600.0, state = \"nominal\" }},\n\
             {{ t0 = 600.0, t1 = 3600.0, state = \"denied\" }},\n\
             ]\n\
             [clock_quantum]\n\
             id = \"{quantum_id}\"\n\
             provenance = \"trade variant\"\n\
             y0 = 0.0\n\
             q_wf = {q_wf}\n\
             q_rw = 0.0\n\
             [clock_classical]\n\
             id = \"csac\"\n\
             provenance = \"trade variant\"\n\
             y0 = 0.0\n\
             q_wf = 9.0e-20\n\
             q_rw = 0.0\n"
        )
    }

    /// A passing multi-criteria study whose winner clears the TRL floor.
    fn good_space() -> TradeSpace {
        TradeSpace::multi_criteria(
            "clock architecture trade",
            vec![
                Variant {
                    label: "optical".into(),
                    scenario_toml: clock_variant("optical", 1.0e-30),
                },
                Variant {
                    label: "csac".into(),
                    scenario_toml: clock_variant("csac", 9.0e-20),
                },
            ],
            vec![
                TradeCriterion::benefit("holdover", "quantum.fom.holdover_s", 0.7),
                TradeCriterion::cost("rms_ns", "quantum.fom.timing_rms_ns", 0.3),
            ],
        )
        .with_weighting(Weighting::Direct)
        .with_trl("optical", TrlLevel::new(7))
        .with_trl("csac", TrlLevel::new(8))
        .with_gate(GateThresholds::default())
    }

    /// A passing twin reproduction from a clean white-FM curve (σ_y(τ)=s1·τ^-1/2),
    /// which the 2-parameter fit reproduces to well within tolerance.
    fn twin_evidence(device: &str, datasheet_cited: bool) -> TwinValidationEvidence {
        let s1 = 3.0e-10;
        let spec = DeviceSpec {
            device: device.into(),
            provenance: "white-FM ADEV curve".into(),
            adev_points: vec![(1.0, s1), (4.0, s1 / 2.0), (16.0, s1 / 4.0)],
            drift_per_s: 0.0,
        };
        let twin = calibrate(&spec).unwrap();
        let rep = fit_report(&twin, &spec, 0.10).unwrap();
        assert!(rep.passed, "fixture twin must reproduce its own curve");
        TwinValidationEvidence::from_report(device, &rep, datasheet_cited)
    }

    /// A passing, datasheet-cited twin reproduction.
    fn good_twin() -> TwinValidationEvidence {
        twin_evidence("CSAC (datasheet)", true)
    }

    fn factor(d: &CredibilityDossier, f: CasFactor) -> &CasAssessment {
        d.cas_factors.iter().find(|x| x.factor == f).unwrap()
    }

    #[test]
    fn dossier_has_all_eight_cas_factors() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        assert_eq!(d.cas_factors.len(), 8, "all eight NASA-STD-7009B factors");
        for f in [
            CasFactor::Verification,
            CasFactor::Validation,
            CasFactor::InputPedigree,
            CasFactor::ResultsUncertainty,
            CasFactor::ResultsRobustness,
            CasFactor::UseHistory,
            CasFactor::MsManagement,
            CasFactor::PeopleQualification,
        ] {
            assert!(d.cas_factors.iter().any(|x| x.factor == f), "{:?}", f);
        }
    }

    /// FIREWALL: Validation can never exceed level 1, even on the strongest evidence.
    #[test]
    fn validation_is_firewall_capped_at_one() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        let v = factor(&d, CasFactor::Validation);
        assert!(
            v.firewall_capped,
            "Validation must be flagged firewall-capped"
        );
        assert_eq!(v.cap, FIREWALL_VALIDATION_CAP);
        assert_eq!(FIREWALL_VALIDATION_CAP, 1);
        assert!(
            v.level.unwrap() <= 1,
            "composed-layer Validation cannot exceed level 1 (no flight/measured data)"
        );
        // Even a passing datasheet twin only reaches level 1 — never higher.
        assert_eq!(v.level, Some(1));
    }

    /// FAIL-CLOSED: with no twin reproduction evidence, Validation drops to 0.
    #[test]
    fn validation_fails_closed_to_zero_without_twin_evidence() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[]); // no twin evidence supplied
        let v = factor(&d, CasFactor::Validation);
        assert_eq!(
            v.level,
            Some(0),
            "missing referent evidence must fail closed to level 0, not be assumed"
        );
        assert!(
            !d.evidence_complete,
            "missing twin evidence ⇒ evidence incomplete"
        );
    }

    /// Use History is honestly low for a new system.
    #[test]
    fn use_history_is_honestly_low() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        let u = factor(&d, CasFactor::UseHistory);
        assert!(
            u.level.unwrap() <= USE_HISTORY_CAP,
            "a new system has no operational/flight use history"
        );
        assert_eq!(USE_HISTORY_CAP, 1);
    }

    /// People Qualification is recorded as not-assessed — never invented.
    #[test]
    fn people_qualification_is_not_assessed() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        let p = factor(&d, CasFactor::PeopleQualification);
        assert!(
            p.level.is_none(),
            "People Qualification must be not-assessed (None)"
        );
        assert!(p.rationale.contains("NOT ASSESSED"));
    }

    /// Input Pedigree honestly reflects datasheet vs representative inputs.
    #[test]
    fn input_pedigree_reflects_datasheet_vs_representative() {
        let report = run_trade_study(&good_space()).unwrap();
        // Datasheet-cited input → up to level 2.
        let d_cited = build_dossier(&report, &[good_twin()]);
        assert_eq!(factor(&d_cited, CasFactor::InputPedigree).level, Some(2));
        // Representative-class only → level 1.
        let d_rep = build_dossier(&report, &[twin_evidence("rep", false)]);
        assert_eq!(factor(&d_rep, CasFactor::InputPedigree).level, Some(1));
    }

    /// The overall credibility never exceeds the weakest assessed factor.
    #[test]
    fn overall_never_exceeds_weakest_factor() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        assert!(d.overall_within_factors());
        let min_assessed = d.cas_factors.iter().filter_map(|f| f.level).min().unwrap();
        assert_eq!(d.overall_level, min_assessed);
        // Validation (1) and Use History (1) bind it; overall must be 1 here.
        assert_eq!(d.overall_level, 1);
        assert!(d.binding_factors.contains(&CasFactor::Validation));
        assert!(d.binding_factors.contains(&CasFactor::UseHistory));
    }

    /// FAIL-CLOSED: a single-FoM study (no decision) cannot clear the gate, and the
    /// decision-dependent factors drop — the dossier is honestly degraded.
    #[test]
    fn single_fom_study_fails_closed() {
        let space = TradeSpace::single_fom(
            "single",
            "quantum.fom.holdover_s",
            false,
            vec![
                Variant {
                    label: "optical".into(),
                    scenario_toml: clock_variant("optical", 1.0e-30),
                },
                Variant {
                    label: "csac".into(),
                    scenario_toml: clock_variant("csac", 9.0e-20),
                },
            ],
        );
        let report = run_trade_study(&space).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        assert_eq!(
            d.evidence.gate.outcome,
            GateOutcome::Fail,
            "no decision ⇒ gate fails closed"
        );
        assert!(!d.evidence_complete);
        // Decision-dependent factors fail closed to their floor (level 1).
        assert_eq!(factor(&d, CasFactor::ResultsUncertainty).level, Some(1));
        assert_eq!(factor(&d, CasFactor::ResultsRobustness).level, Some(1));
        // And the overall never claims more than that floor.
        assert!(d.overall_level <= 1);
    }

    /// ASME V&V 40: the context of use states the gap honestly (required > achieved).
    #[test]
    fn context_of_use_states_the_gap() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        let cou = &d.context_of_use;
        assert_eq!(cou.credibility_required, COU_CREDIBILITY_REQUIRED);
        assert_eq!(cou.credibility_achieved, d.overall_level);
        assert!(
            cou.credibility_achieved < cou.credibility_required,
            "feasibility-grade credibility must fall short of sole-reliance requirement"
        );
        assert!(cou.gap.contains("CREDIBILITY GAP"));
        assert_eq!(cou.model_risk, RiskLevel::High);
    }

    /// The markdown composes every evidence source and carries the honesty banner.
    #[test]
    fn markdown_composes_all_evidence_and_is_honest() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        let md = d.markdown();
        // Standards cited structurally.
        assert!(md.contains("NASA-STD-7009B"));
        assert!(md.contains("ASME V&V 40"));
        // Honesty banner present; not Validated / not accreditation.
        assert!(md.contains("Modelled"));
        assert!(md.contains("not") && md.contains("accreditation"));
        // Composed evidence present.
        assert!(md.contains("Merkle root"));
        assert!(md.contains("Gate checks"));
        assert!(!md.contains("GSN") && !md.contains("Verification Control Document"));
        assert!(md.contains("verification matrix"));
        assert!(md.contains("Assurance gate"));
        // The eight factor names all render.
        for f in [
            "Verification",
            "Validation",
            "Input Pedigree",
            "Results Uncertainty",
            "Results Robustness",
            "Use History",
            "M&S Management",
            "People Qualification",
        ] {
            assert!(md.contains(f), "factor {f} must render");
        }
        // The overall statement is bounded by the weakest factor.
        assert!(md.contains("Overall credibility"));
    }

    /// Structured data round-trips through serde (it is an evidence artifact).
    #[test]
    fn dossier_round_trips_through_serde() {
        let report = run_trade_study(&good_space()).unwrap();
        let d = build_dossier(&report, &[good_twin()]);
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("overall_level"));
        assert!(json.contains("Validation"));
        assert!(json.contains(&d.evidence.merkle_root));
    }
}
