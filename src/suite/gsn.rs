// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Goal Structuring Notation (GSN) assurance argument
//!
//! Builds a **claim–argument–evidence** structure (GSN Community Standard v3 /
//! SCSC-141C) for the claim *"variant X is the justified selection"*. The argument
//! decomposes the top goal over a strategy into sub-goals (WSM rank, weight
//! consistency, Pareto-optimality, robustness, TRL, reproducibility), each discharged
//! by a **Solution** node pointing at a concrete artifact in the trade report.
//!
//! Honesty scope: GSN is an *argument structure*, not a measurement. The nodes here
//! are generated from the open-core MCDA result; the argument is only as strong as
//! the (Modelled) decision it documents. Nothing here is Validated.

use super::TradeReport;

/// GSN element type (the standard's principal node kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum GsnNodeKind {
    /// A claim to be supported (G).
    Goal,
    /// The reasoning step linking a goal to its sub-goals/solutions (S).
    Strategy,
    /// A reference to evidence discharging a goal (Sn).
    Solution,
    /// Contextual scoping for a goal/strategy (C).
    Context,
    /// A stated assumption the argument rests on (A).
    Assumption,
    /// A justification for a strategy or goal (J).
    Justification,
}

impl GsnNodeKind {
    fn tag(self) -> &'static str {
        match self {
            GsnNodeKind::Goal => "Goal",
            GsnNodeKind::Strategy => "Strategy",
            GsnNodeKind::Solution => "Solution",
            GsnNodeKind::Context => "Context",
            GsnNodeKind::Assumption => "Assumption",
            GsnNodeKind::Justification => "Justification",
        }
    }
}

/// A GSN node.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GsnNode {
    pub id: String,
    pub kind: GsnNodeKind,
    pub statement: String,
}

/// The relationship between two GSN nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum GsnLinkKind {
    /// Inferential support (goal → sub-goal/strategy/solution).
    SupportedBy,
    /// Contextual association (goal/strategy → context/assumption/justification).
    InContextOf,
}

/// A directed GSN link.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GsnLink {
    pub from: String,
    pub to: String,
    pub kind: GsnLinkKind,
}

/// A complete GSN argument (nodes + links).
#[derive(Debug, Clone, serde::Serialize)]
pub struct GsnArgument {
    pub nodes: Vec<GsnNode>,
    pub links: Vec<GsnLink>,
}

impl GsnArgument {
    fn node(&mut self, id: &str, kind: GsnNodeKind, statement: String) {
        self.nodes.push(GsnNode {
            id: id.to_string(),
            kind,
            statement,
        });
    }

    fn supported_by(&mut self, from: &str, to: &str) {
        self.links.push(GsnLink {
            from: from.to_string(),
            to: to.to_string(),
            kind: GsnLinkKind::SupportedBy,
        });
    }

    fn in_context_of(&mut self, from: &str, to: &str) {
        self.links.push(GsnLink {
            from: from.to_string(),
            to: to.to_string(),
            kind: GsnLinkKind::InContextOf,
        });
    }

    /// Look up a node by id.
    pub fn get(&self, id: &str) -> Option<&GsnNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Render the argument as an indented markdown tree rooted at the top goal.
    pub fn markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("## Assurance argument (GSN — claim/argument/evidence)\n\n");
        // Flat, ordered listing (creation order keeps parents before children).
        for n in &self.nodes {
            let ctx: Vec<&str> = self
                .links
                .iter()
                .filter(|l| l.from == n.id && l.kind == GsnLinkKind::InContextOf)
                .map(|l| l.to.as_str())
                .collect();
            let kids: Vec<&str> = self
                .links
                .iter()
                .filter(|l| l.from == n.id && l.kind == GsnLinkKind::SupportedBy)
                .map(|l| l.to.as_str())
                .collect();
            md.push_str(&format!(
                "- **{} [{}]** — {}\n",
                n.id,
                n.kind.tag(),
                n.statement
            ));
            if !kids.is_empty() {
                md.push_str(&format!("  - supported by: {}\n", kids.join(", ")));
            }
            if !ctx.is_empty() {
                md.push_str(&format!("  - in context of: {}\n", ctx.join(", ")));
            }
        }
        md.push('\n');
        md
    }
}

/// Build the GSN argument for the trade report's selection.
///
/// `min_trl` is the (representative) TRL floor referenced by the maturity sub-goal.
pub fn build_argument(report: &TradeReport, min_trl: u8) -> GsnArgument {
    let mut g = GsnArgument {
        nodes: Vec::new(),
        links: Vec::new(),
    };
    let winner = report.winner.as_deref().unwrap_or("(no winner)");

    // Top goal + context.
    g.node(
        "G1",
        GsnNodeKind::Goal,
        format!(
            "Variant '{winner}' is the justified architecture selection for: {}",
            report.title
        ),
    );
    g.node(
        "C1",
        GsnNodeKind::Context,
        "Trade conducted per ECSS-E-ST-10C Annex L and NASA SE Handbook §6.8; each \
         variant simulated on the open Kshana engine (reproducible scenario hashes)."
            .to_string(),
    );
    g.in_context_of("G1", "C1");

    // If there is no decision (single-FoM or inconclusive), the argument is honest
    // about its limited support and stops at the ranking sub-goal.
    let dr = match &report.decision {
        Some(d) => d,
        None => {
            g.node(
                "A1",
                GsnNodeKind::Assumption,
                "Single figure of merit fully captures stakeholder value (no multi-criteria \
                 weighting performed)."
                    .to_string(),
            );
            g.in_context_of("G1", "A1");
            g.node(
                "G2",
                GsnNodeKind::Goal,
                format!(
                    "'{winner}' is rank-1 on the agreed figure of merit '{}'.",
                    report.metric
                ),
            );
            g.supported_by("G1", "G2");
            g.node(
                "Sn1",
                GsnNodeKind::Solution,
                "Ranked outcome table with reproducible scenario hashes (evidence pack)."
                    .to_string(),
            );
            g.supported_by("G2", "Sn1");
            return g;
        }
    };

    // Strategy: argue over the multi-criteria decision.
    g.node(
        "S1",
        GsnNodeKind::Strategy,
        "Argue selection over a multi-criteria decision analysis with explicit weight \
         sensitivity, Pareto-optimality, technology maturity and reproducibility."
            .to_string(),
    );
    g.supported_by("G1", "S1");
    g.node(
        "J1",
        GsnNodeKind::Justification,
        "A single FoM hides incommensurate trade-offs; a weighted multi-criteria decision \
         with sensitivity makes the value judgements and their fragility explicit."
            .to_string(),
    );
    g.in_context_of("S1", "J1");

    // G2 — WSM ranking.
    g.node(
        "G2",
        GsnNodeKind::Goal,
        format!("'{winner}' ranks first under the agreed weighted-sum model."),
    );
    g.supported_by("S1", "G2");
    g.node(
        "Sn1",
        GsnNodeKind::Solution,
        format!(
            "WSM weighted decision matrix (winner score {}).",
            dr.scores
                .iter()
                .find(|s| Some(s.variant_index) == dr.winner)
                .map(|s| format!("{:.4}", s.score))
                .unwrap_or_else(|| "n/a".into())
        ),
    );
    g.supported_by("G2", "Sn1");

    // G3 — weight consistency (AHP CR, or a stakeholder-agreement assumption).
    g.node(
        "G3",
        GsnNodeKind::Goal,
        "The criterion weights are consistent and defensible.".to_string(),
    );
    g.supported_by("S1", "G3");
    match &dr.ahp {
        Some(ahp) => {
            g.node(
                "Sn2",
                GsnNodeKind::Solution,
                format!(
                    "AHP Consistency Ratio {} < threshold {} ({}).",
                    ahp.consistency_ratio
                        .map(|c| format!("{c:.4}"))
                        .unwrap_or_else(|| "n/a".into()),
                    ahp.threshold,
                    if ahp.acceptable {
                        "ACCEPTED"
                    } else {
                        "REJECTED"
                    }
                ),
            );
            g.supported_by("G3", "Sn2");
        }
        None => {
            g.node(
                "A2",
                GsnNodeKind::Assumption,
                "Direct weights are stakeholder-agreed (no AHP consistency test applied)."
                    .to_string(),
            );
            g.in_context_of("G3", "A2");
        }
    }

    // G4 — Pareto optimality.
    g.node(
        "G4",
        GsnNodeKind::Goal,
        format!("'{winner}' is non-dominated (Pareto-optimal) on the raw criteria."),
    );
    g.supported_by("S1", "G4");
    g.node(
        "Sn3",
        GsnNodeKind::Solution,
        format!(
            "Pareto-front membership of the winner: {}.",
            if dr.winner_on_pareto() {
                "confirmed"
            } else {
                "NOT on front"
            }
        ),
    );
    g.supported_by("G4", "Sn3");

    // G5 — robustness to weight uncertainty.
    g.node(
        "G5",
        GsnNodeKind::Goal,
        "The selection is robust to weight uncertainty.".to_string(),
    );
    g.supported_by("S1", "G5");
    let robustness = match &dr.min_flip {
        Some(f) => format!(
            "Min weight change to flip winner: L1 = {:.4}; SMAA rank-1 acceptability {}.",
            f.l1_change,
            dr.winner_smaa()
                .map(|p| format!("{p:.3}"))
                .unwrap_or_else(|| "n/a".into())
        ),
        None => format!(
            "Winner dominates — no single-criterion weight change can flip it; SMAA rank-1 {}.",
            dr.winner_smaa()
                .map(|p| format!("{p:.3}"))
                .unwrap_or_else(|| "n/a".into())
        ),
    };
    g.node("Sn4", GsnNodeKind::Solution, robustness);
    g.supported_by("G5", "Sn4");

    // G6 — technology maturity.
    g.node(
        "G6",
        GsnNodeKind::Goal,
        format!("'{winner}' meets the technology-maturity floor (TRL ≥ {min_trl})."),
    );
    g.supported_by("S1", "G6");
    let trl = report.trl.get(winner).map(|t| t.level);
    g.node(
        "Sn5",
        GsnNodeKind::Solution,
        match trl {
            Some(l) => format!("ISO 16290 TRL declaration: TRL-{l} (floor {min_trl})."),
            None => {
                format!("No TRL declared for the winner — TRL-{min_trl} floor NOT demonstrated.")
            }
        },
    );
    g.supported_by("G6", "Sn5");

    // G7 — reproducibility / integrity.
    g.node(
        "G7",
        GsnNodeKind::Goal,
        "The decision is reproducible and tamper-evident.".to_string(),
    );
    g.supported_by("S1", "G7");
    g.node(
        "Sn6",
        GsnNodeKind::Solution,
        "W3C-PROV record with a SHA-256 Merkle root over inputs, outcomes, weights and \
         decision (evidence pack)."
            .to_string(),
    );
    g.supported_by("G7", "Sn6");

    let _ = dr; // dr fields are consumed above; keep binding explicit.
    g
}

#[cfg(test)]
mod tests {
    use super::super::decision::{TradeCriterion, Weighting};
    use super::super::{run_trade_study, TradeSpace, TrlLevel, Variant};
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

    fn multi_space() -> TradeSpace {
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
        .with_weighting(Weighting::Ahp {
            pairwise: vec![vec![1.0, 3.0], vec![1.0 / 3.0, 1.0]],
            cr_threshold: 0.10,
        })
        .with_trl("optical", TrlLevel::new(5))
        .with_trl("csac", TrlLevel::new(8))
    }

    #[test]
    fn argument_has_top_goal_strategy_and_solutions() {
        let report = run_trade_study(&multi_space()).unwrap();
        let g = build_argument(&report, 6);
        assert!(g.get("G1").is_some());
        assert_eq!(g.get("G1").unwrap().kind, GsnNodeKind::Goal);
        assert!(g.get("S1").is_some());
        assert_eq!(g.get("S1").unwrap().kind, GsnNodeKind::Strategy);
        // Every sub-goal under the strategy is discharged by a solution.
        for sub in ["G2", "G4", "G5", "G6", "G7"] {
            let has_solution = g
                .links
                .iter()
                .any(|l| l.from == sub && l.kind == GsnLinkKind::SupportedBy);
            assert!(has_solution, "{sub} must be supported by a solution");
        }
        let md = g.markdown();
        assert!(md.contains("GSN"));
        assert!(md.contains(report.winner.as_deref().unwrap()));
    }

    #[test]
    fn single_fom_argument_is_honest_about_its_assumption() {
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
        let g = build_argument(&report, 6);
        // The single-FoM argument records the limiting assumption explicitly.
        assert!(g.get("A1").is_some());
        assert_eq!(g.get("A1").unwrap().kind, GsnNodeKind::Assumption);
    }
}
