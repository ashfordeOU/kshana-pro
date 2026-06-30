// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — ECSS Verification Control Document (VCD)
//!
//! Generates an **ECSS-E-ST-10C / ECSS-E-ST-10-02C-style Verification Control
//! Document**: a traceability matrix mapping each *decision requirement* to a
//! verification **method** (Test / Analysis / Review-of-Design / Inspection), the
//! **evidence** discharging it, and a **status**. The requirements are the claims the
//! selection rests on (a unique winner, satisfied knock-outs, weight consistency,
//! Pareto-optimality, robustness, TRL, reproducibility).
//!
//! Honesty scope: the requirement identifiers here use a **locally generated**
//! `REQ-TS-*` namespace ("TS" = trade study) and are derived from the trade's own
//! claims — they are **not** any external tender's requirement IDs, and the
//! thresholds are representative engineering choices carried from
//! [`GateThresholds`](crate::suite::GateThresholds). Everything here is **Modelled**.

use super::{GateThresholds, TradeReport};

/// ECSS verification method (ECSS-E-ST-10-02C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum VerificationMethod {
    Test,
    Analysis,
    ReviewOfDesign,
    Inspection,
}

impl VerificationMethod {
    fn label(self) -> &'static str {
        match self {
            VerificationMethod::Test => "Test",
            VerificationMethod::Analysis => "Analysis",
            VerificationMethod::ReviewOfDesign => "Review of Design",
            VerificationMethod::Inspection => "Inspection",
        }
    }
}

/// Verification status of a VCD line item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum VcdStatus {
    Pass,
    Fail,
    Open,
    NotApplicable,
}

impl VcdStatus {
    fn label(self) -> &'static str {
        match self {
            VcdStatus::Pass => "PASS",
            VcdStatus::Fail => "FAIL",
            VcdStatus::Open => "OPEN",
            VcdStatus::NotApplicable => "N/A",
        }
    }
}

/// One VCD line item.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VcdItem {
    pub req_id: String,
    pub requirement: String,
    pub method: VerificationMethod,
    pub evidence: String,
    pub status: VcdStatus,
    pub note: String,
}

/// A Verification Control Document.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Vcd {
    pub items: Vec<VcdItem>,
}

impl Vcd {
    /// `true` iff no item is `Fail` or `Open` (every requirement is closed Pass/N-A).
    pub fn all_closed(&self) -> bool {
        self.items
            .iter()
            .all(|i| matches!(i.status, VcdStatus::Pass | VcdStatus::NotApplicable))
    }

    /// Markdown traceability table.
    pub fn markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("## Verification Control Document (ECSS-E-ST-10C, structure)\n\n");
        md.push_str(
            "_Requirement IDs are locally generated (`REQ-TS-*`) from the trade's own claims; \
             they are not external tender requirements. Modelled._\n\n",
        );
        md.push_str("| Req ID | Requirement | Method | Evidence | Status | Note |\n");
        md.push_str("|--------|-------------|--------|----------|--------|------|\n");
        for it in &self.items {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                it.req_id,
                it.requirement,
                it.method.label(),
                it.evidence,
                it.status.label(),
                it.note,
            ));
        }
        md.push('\n');
        md
    }
}

fn winner_score_str(report: &TradeReport) -> String {
    report
        .decision
        .as_ref()
        .and_then(|dr| {
            dr.winner
                .and_then(|w| dr.scores.iter().find(|s| s.variant_index == w))
        })
        .map(|s| format!("WSM score {:.4}", s.score))
        .unwrap_or_else(|| "—".to_string())
}

/// Build the VCD for a completed trade study, applying the report's gate thresholds.
pub fn build_vcd(report: &TradeReport) -> Vcd {
    let th: &GateThresholds = &report.gate;
    let mut items: Vec<VcdItem> = Vec::new();

    // REQ-TS-010 — a unique selection exists.
    items.push(VcdItem {
        req_id: "REQ-TS-010".into(),
        requirement: "A single justified architecture selection shall be identified.".into(),
        method: VerificationMethod::Analysis,
        evidence: winner_score_str(report),
        status: if report.winner.is_some() {
            VcdStatus::Pass
        } else {
            VcdStatus::Fail
        },
        note: report
            .winner
            .as_deref()
            .map(|w| format!("winner: {w}"))
            .unwrap_or_else(|| "no winner".into()),
    });

    let dr = match &report.decision {
        Some(d) => d,
        None => {
            // Single-FoM study: the rest of the matrix is not applicable.
            items.push(VcdItem {
                req_id: "REQ-TS-020".into(),
                requirement: "Multi-criteria weighting shall be consistent.".into(),
                method: VerificationMethod::Analysis,
                evidence: "single figure of merit".into(),
                status: VcdStatus::NotApplicable,
                note: "no multi-criteria decision".into(),
            });
            return Vcd { items };
        }
    };

    // REQ-TS-100 — the winner survived every knock-out constraint.
    if let Some(w) = dr.winner {
        if let Some(ws) = dr.scores.iter().find(|s| s.variant_index == w) {
            items.push(VcdItem {
                req_id: "REQ-TS-100".into(),
                requirement: "The selected architecture shall satisfy all knock-out constraints."
                    .into(),
                method: VerificationMethod::Analysis,
                evidence: "winner not eliminated by any criterion constraint".into(),
                status: if ws.eliminated {
                    VcdStatus::Fail
                } else {
                    VcdStatus::Pass
                },
                note: ws
                    .eliminated_by
                    .as_deref()
                    .map(|c| format!("eliminated by {c}"))
                    .unwrap_or_else(|| "all constraints met".into()),
            });
        }
    }

    // REQ-TS-020 — weight consistency (AHP CR, else N/A for direct weights).
    let (cons_status, cons_ev, cons_note) = match &dr.ahp {
        Some(ahp) => (
            if ahp.acceptable {
                VcdStatus::Pass
            } else if th.require_ahp_consistent {
                VcdStatus::Fail
            } else {
                VcdStatus::Open
            },
            format!(
                "AHP CR = {} (threshold {})",
                ahp.consistency_ratio
                    .map(|c| format!("{c:.4}"))
                    .unwrap_or_else(|| "n/a".into()),
                ahp.threshold
            ),
            "principal-eigenvector priorities".to_string(),
        ),
        None => (
            VcdStatus::NotApplicable,
            "direct stakeholder weights".to_string(),
            "no AHP consistency test applicable".to_string(),
        ),
    };
    items.push(VcdItem {
        req_id: "REQ-TS-020".into(),
        requirement: "The criterion weighting shall be consistent (AHP CR < threshold).".into(),
        method: VerificationMethod::Analysis,
        evidence: cons_ev,
        status: cons_status,
        note: cons_note,
    });

    // REQ-TS-030 — Pareto optimality.
    items.push(VcdItem {
        req_id: "REQ-TS-030".into(),
        requirement: "The selected architecture shall be Pareto-optimal on the raw criteria."
            .into(),
        method: VerificationMethod::Analysis,
        evidence: format!(
            "winner Pareto membership: {}",
            if dr.winner_on_pareto() { "on front" } else { "dominated" }
        ),
        status: if dr.winner_on_pareto() {
            VcdStatus::Pass
        } else {
            VcdStatus::Fail
        },
        note: format!("front size {}", dr.pareto_front.len()),
    });

    // REQ-TS-040 — SMAA robustness.
    let smaa = dr.winner_smaa();
    items.push(VcdItem {
        req_id: "REQ-TS-040".into(),
        requirement: format!(
            "The winner's SMAA rank-1 acceptability shall be ≥ {:.2} (representative).",
            th.min_smaa_winner
        ),
        method: VerificationMethod::Analysis,
        evidence: smaa
            .map(|p| format!("SMAA P(winner) = {p:.3}"))
            .unwrap_or_else(|| "—".into()),
        status: match smaa {
            Some(p) if p >= th.min_smaa_winner => VcdStatus::Pass,
            Some(_) => VcdStatus::Fail,
            None => VcdStatus::Open,
        },
        note: "Dirichlet weight sampling".into(),
    });

    // REQ-TS-041 — flip robustness.
    let flip_ok = match &dr.min_flip {
        None => true, // dominant winner cannot flip
        Some(f) => f.l1_change >= th.min_flip_l1,
    };
    items.push(VcdItem {
        req_id: "REQ-TS-041".into(),
        requirement: format!(
            "The winner shall not flip under a single-criterion weight change of L1 < {:.2} \
             (representative).",
            th.min_flip_l1
        ),
        method: VerificationMethod::Analysis,
        evidence: match &dr.min_flip {
            None => "no single-criterion flip possible (dominant)".to_string(),
            Some(f) => format!("min flip L1 = {:.4} via '{}'", f.l1_change, f.criterion),
        },
        status: if flip_ok { VcdStatus::Pass } else { VcdStatus::Fail },
        note: "weight-perturbation sweep".into(),
    });

    // REQ-TS-050 — TRL floor.
    let winner_trl = report
        .winner
        .as_deref()
        .and_then(|w| report.trl.get(w))
        .map(|t| t.level);
    items.push(VcdItem {
        req_id: "REQ-TS-050".into(),
        requirement: format!(
            "The selected architecture shall meet the TRL floor (≥ {}, ISO 16290, representative).",
            th.min_trl
        ),
        method: VerificationMethod::ReviewOfDesign,
        evidence: winner_trl
            .map(|l| format!("declared TRL-{l}"))
            .unwrap_or_else(|| "no TRL declared".into()),
        status: match winner_trl {
            Some(l) if l >= th.min_trl => VcdStatus::Pass,
            Some(_) => VcdStatus::Fail,
            None => VcdStatus::Open, // fail-closed: maturity not demonstrated
        },
        note: "self-declared maturity".into(),
    });

    // REQ-TS-060 — reproducibility / integrity.
    let prov = super::provenance::build_provenance(report);
    let root_ok = prov.merkle_root.len() == 64 && prov.merkle_root.chars().all(|c| c.is_ascii_hexdigit());
    items.push(VcdItem {
        req_id: "REQ-TS-060".into(),
        requirement: "The trade shall be reproducible and tamper-evident.".into(),
        method: VerificationMethod::Inspection,
        evidence: format!("W3C-PROV Merkle root {}…", &prov.merkle_root[..prov.merkle_root.len().min(16)]),
        status: if root_ok { VcdStatus::Pass } else { VcdStatus::Fail },
        note: "SHA-256 over inputs/outcomes/weights/decision".into(),
    });

    Vcd { items }
}

#[cfg(test)]
mod tests {
    use super::super::decision::{TradeCriterion, Weighting};
    use super::super::{run_trade_study, GateThresholds, TradeSpace, TrlLevel, Variant};
    use super::*;

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

    fn space() -> TradeSpace {
        TradeSpace::multi_criteria(
            "clock trade",
            vec![
                Variant { label: "optical".into(), scenario_toml: clock_variant("optical", 1.0e-30) },
                Variant { label: "csac".into(), scenario_toml: clock_variant("csac", 9.0e-20) },
            ],
            vec![
                TradeCriterion::benefit("holdover", "quantum.fom.holdover_s", 0.7),
                TradeCriterion::cost("rms_ns", "quantum.fom.timing_rms_ns", 0.3),
            ],
        )
        .with_weighting(Weighting::Direct)
        .with_trl("optical", TrlLevel::new(8))
        .with_trl("csac", TrlLevel::new(8))
        .with_gate(GateThresholds::default())
    }

    #[test]
    fn vcd_traces_every_decision_claim() {
        let report = run_trade_study(&space()).unwrap();
        let vcd = build_vcd(&report);
        let ids: Vec<&str> = vcd.items.iter().map(|i| i.req_id.as_str()).collect();
        for want in ["REQ-TS-010", "REQ-TS-020", "REQ-TS-030", "REQ-TS-040", "REQ-TS-050", "REQ-TS-060"] {
            assert!(ids.contains(&want), "VCD must trace {want}");
        }
        let md = vcd.markdown();
        assert!(md.contains("Verification Control Document"));
        assert!(md.contains("Review of Design")); // the TRL row's method
    }

    #[test]
    fn direct_weighting_marks_consistency_not_applicable() {
        let report = run_trade_study(&space()).unwrap();
        let vcd = build_vcd(&report);
        let cons = vcd.items.iter().find(|i| i.req_id == "REQ-TS-020").unwrap();
        assert_eq!(cons.status, VcdStatus::NotApplicable);
    }

    #[test]
    fn reproducibility_row_inspects_the_merkle_root() {
        let report = run_trade_study(&space()).unwrap();
        let vcd = build_vcd(&report);
        let repro = vcd.items.iter().find(|i| i.req_id == "REQ-TS-060").unwrap();
        assert_eq!(repro.method, VerificationMethod::Inspection);
        assert_eq!(repro.status, VcdStatus::Pass);
    }
}
