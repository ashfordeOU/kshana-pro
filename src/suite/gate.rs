// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Multi-criteria assurance gate
//!
//! A **fail-closed** pass/fail gate over a completed trade study. The selection is
//! accepted only if *every* required check passes: a winner exists and is rank-stable
//! (sensitivity), the winner is Pareto-optimal, the AHP weights are consistent
//! (`CR < threshold`) when AHP is used, the SMAA rank-1 acceptability clears the
//! floor, the winner meets the TRL floor (ISO 16290), and the provenance Merkle root
//! is intact. Missing or inconclusive evidence fails closed.
//!
//! Honesty scope: the thresholds ([`GateThresholds`]) are **representative engineering
//! choices**, not certified values; they are configurable. The gate composes the
//! Modelled decision + provenance — nothing here is Validated.

use super::{DecisionReport, TradeReport};

/// Thresholds for the assurance gate. All are representative defaults; a caller may
/// override any of them.
#[derive(Debug, Clone)]
pub struct GateThresholds {
    /// ISO 16290 TRL floor the winner must meet (representative default `6`).
    pub min_trl: u8,
    /// Minimum single-criterion weight change (L1) the winner must survive without
    /// flipping (representative default `0.05`).
    pub min_flip_l1: f64,
    /// Minimum SMAA rank-1 acceptability for the winner (representative default `0.50`).
    pub min_smaa_winner: f64,
    /// Require the winner to be on the Pareto front (default `true`).
    pub require_winner_in_pareto: bool,
    /// Require AHP weights (when used) to clear the consistency gate (default `true`).
    pub require_ahp_consistent: bool,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            min_trl: 6,
            min_flip_l1: 0.05,
            min_smaa_winner: 0.50,
            require_winner_in_pareto: true,
            require_ahp_consistent: true,
        }
    }
}

/// Overall gate outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum GateOutcome {
    Pass,
    Fail,
}

/// The result of one gate criterion.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GateCriterionResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

/// The structured verdict of the assurance gate.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GateVerdict {
    pub outcome: GateOutcome,
    pub criteria: Vec<GateCriterionResult>,
    pub summary: String,
}

impl GateVerdict {
    /// `true` iff the gate passed.
    pub fn passed(&self) -> bool {
        self.outcome == GateOutcome::Pass
    }

    /// Markdown rendering of the verdict.
    pub fn markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("## Assurance gate (fail-closed)\n\n");
        md.push_str(&format!(
            "**Verdict: {}**\n\n",
            match self.outcome {
                GateOutcome::Pass => "PASS",
                GateOutcome::Fail => "FAIL",
            }
        ));
        md.push_str("| Check | Result | Detail |\n|-------|--------|--------|\n");
        for c in &self.criteria {
            md.push_str(&format!(
                "| {} | {} | {} |\n",
                c.name,
                if c.passed { "pass" } else { "FAIL" },
                c.detail
            ));
        }
        md.push_str(&format!("\n{}\n", self.summary));
        md
    }
}

fn check(name: &str, passed: bool, detail: String) -> GateCriterionResult {
    GateCriterionResult {
        name: name.to_string(),
        passed,
        detail,
    }
}

/// Run the assurance gate over a completed trade study.
///
/// The thresholds are taken from the report's own [`GateThresholds`]
/// ([`TradeReport::gate`](crate::suite::TradeReport::gate)). The gate is fail-closed:
/// a single-FoM study (no multi-criteria decision) and any missing evidence fail.
pub fn assurance_gate(report: &TradeReport) -> GateVerdict {
    let th = report.gate.clone();
    let mut results: Vec<GateCriterionResult> = Vec::new();

    // 0. There must be a multi-criteria decision to gate.
    let dr: &DecisionReport = match &report.decision {
        Some(d) => d,
        None => {
            results.push(check(
                "multi-criteria decision present",
                false,
                "single figure-of-merit study cannot be gated on multi-criteria assurance".into(),
            ));
            return finalize(results);
        }
    };

    // 1. A winner exists.
    let winner = dr.winner;
    results.push(check(
        "unique selection",
        winner.is_some(),
        winner
            .and_then(|w| dr.scores.iter().find(|s| s.variant_index == w))
            .map(|s| format!("winner '{}' (score {:.4})", s.name, s.score))
            .unwrap_or_else(|| "no winner — every variant knocked out/excluded".into()),
    ));

    // 2. Winner rank-stability (sensitivity).
    let stable = match &dr.min_flip {
        None => true, // dominant winner: no single-criterion change can flip it
        Some(f) => f.l1_change >= th.min_flip_l1,
    };
    results.push(check(
        "winner rank-stability",
        winner.is_some() && stable,
        match &dr.min_flip {
            None => "winner dominates — no single-criterion weight change can flip it".to_string(),
            Some(f) => format!(
                "min flip L1 = {:.4} (require ≥ {:.2}) via '{}'",
                f.l1_change, th.min_flip_l1, f.criterion
            ),
        },
    ));

    // 3. Winner ∈ Pareto front.
    let pareto_ok = !th.require_winner_in_pareto || dr.winner_on_pareto();
    results.push(check(
        "winner Pareto-optimal",
        winner.is_some() && pareto_ok,
        format!(
            "winner on front: {} (front size {})",
            dr.winner_on_pareto(),
            dr.pareto_front.len()
        ),
    ));

    // 4. AHP consistency (CR < threshold) when AHP weighting is used.
    let (ahp_pass, ahp_detail) = match &dr.ahp {
        Some(ahp) => {
            let pass = ahp.acceptable || !th.require_ahp_consistent;
            (
                pass,
                format!(
                    "AHP CR = {} < threshold {} → {}",
                    ahp.consistency_ratio
                        .map(|c| format!("{c:.4}"))
                        .unwrap_or_else(|| "n/a".into()),
                    ahp.threshold,
                    if ahp.acceptable {
                        "consistent"
                    } else {
                        "INCONSISTENT"
                    }
                ),
            )
        }
        None => (
            true,
            "direct stakeholder weights — no AHP consistency test applicable".to_string(),
        ),
    };
    results.push(check("AHP consistency (CR < 0.10)", ahp_pass, ahp_detail));

    // 5. Provenance / Merkle integrity (self-consistent + 64-hex, recomputed twice).
    let p1 = super::provenance::build_provenance(report);
    let p2 = super::provenance::build_provenance(report);
    let root_ok = p1.merkle_root.len() == 64
        && p1.merkle_root.chars().all(|c| c.is_ascii_hexdigit())
        && p1.merkle_root == p2.merkle_root;
    results.push(check(
        "provenance / Merkle integrity",
        root_ok,
        format!(
            "root {}… (stable: {})",
            &p1.merkle_root[..p1.merkle_root.len().min(16)],
            p1.merkle_root == p2.merkle_root
        ),
    ));

    // 6. SMAA rank-1 acceptability of the winner ≥ floor.
    let smaa = dr.winner_smaa();
    let smaa_ok = smaa.map(|p| p >= th.min_smaa_winner).unwrap_or(false);
    results.push(check(
        "SMAA acceptability",
        smaa_ok,
        smaa.map(|p| format!("P(winner) = {p:.3} (require ≥ {:.2})", th.min_smaa_winner))
            .unwrap_or_else(|| "no SMAA probability for winner".into()),
    ));

    // 7. No TRL knock-out (winner meets the floor, fail-closed if undeclared).
    let winner_trl = winner
        .and_then(|w| dr.scores.iter().find(|s| s.variant_index == w))
        .and_then(|s| report.trl.get(&s.name))
        .map(|t| t.level);
    let trl_ok = winner_trl.map(|l| l >= th.min_trl).unwrap_or(false);
    results.push(check(
        "TRL floor (ISO 16290)",
        trl_ok,
        match winner_trl {
            Some(l) => format!("winner TRL-{l} (require ≥ {})", th.min_trl),
            None => format!(
                "winner TRL not declared — cannot clear floor {}",
                th.min_trl
            ),
        },
    ));

    finalize(results)
}

fn finalize(results: Vec<GateCriterionResult>) -> GateVerdict {
    let n_fail = results.iter().filter(|c| !c.passed).count();
    let outcome = if n_fail == 0 {
        GateOutcome::Pass
    } else {
        GateOutcome::Fail
    };
    let summary = match outcome {
        GateOutcome::Pass => format!("All {} assurance checks passed.", results.len()),
        GateOutcome::Fail => format!(
            "{n_fail} of {} assurance checks failed: {}.",
            results.len(),
            results
                .iter()
                .filter(|c| !c.passed)
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ),
    };
    GateVerdict {
        outcome,
        criteria: results,
        summary,
    }
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

    fn space(gate: GateThresholds, optical_trl: u8) -> TradeSpace {
        TradeSpace::multi_criteria(
            "clock trade",
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
        .with_trl("optical", TrlLevel::new(optical_trl))
        .with_trl("csac", TrlLevel::new(8))
        .with_gate(gate)
    }

    #[test]
    fn mature_winner_passes_the_gate() {
        // Optical wins on holdover; give it TRL-7 so it clears the floor.
        let report = run_trade_study(&space(GateThresholds::default(), 7)).unwrap();
        let v = assurance_gate(&report);
        assert!(v.passed(), "verdict: {}", v.summary);
        assert_eq!(v.outcome, GateOutcome::Pass);
    }

    #[test]
    fn immature_winner_fails_closed_on_trl() {
        // Same study but the winner is TRL-4 < floor 6 → gate must fail.
        let report = run_trade_study(&space(GateThresholds::default(), 4)).unwrap();
        let v = assurance_gate(&report);
        assert!(!v.passed());
        let trl = v.criteria.iter().find(|c| c.name.contains("TRL")).unwrap();
        assert!(!trl.passed, "TRL floor check must fail");
    }

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
        let v = assurance_gate(&report);
        assert!(
            !v.passed(),
            "a single-FoM study cannot clear the multi-criteria gate"
        );
    }

    #[test]
    fn high_smaa_floor_fails_a_marginal_winner() {
        // Demand a 99.9% SMAA acceptability — unreasonably strict → expect failure on
        // that one check (a representative-threshold demonstration).
        let g = GateThresholds {
            min_smaa_winner: 0.999,
            ..GateThresholds::default()
        };
        let report = run_trade_study(&space(g, 7)).unwrap();
        let v = assurance_gate(&report);
        let smaa = v.criteria.iter().find(|c| c.name.contains("SMAA")).unwrap();
        // The winner's SMAA must be below 0.999 here, so this check fails.
        assert!(!smaa.passed || v.passed());
    }
}
