// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Standards-shaped trade reports + ISO 16290 TRL
//!
//! Renders a completed [`TradeReport`](crate::suite::TradeReport) into the *structure*
//! of two recognised trade/decision report formats, plus a Technology-Readiness-Level
//! table:
//!
//! * [`ecss_annex_l_markdown`] — an **ECSS-E-ST-10C Annex L** ("Trade-off") style
//!   report: objective, criteria & weighting factors, the weighted decision matrix,
//!   a sensitivity summary, and the recommendation.
//! * [`nasa_se_68_markdown`] — a **NASA Systems Engineering Handbook (NASA/SP-2016-6105
//!   Rev 2) §6.8 Decision Analysis** report with a Table 6.8-1-style weighted
//!   decision matrix and the recommended alternative.
//! * [`trl_assessment`] / [`trl_markdown`] — per-variant **ISO 16290** TRL with a
//!   configurable TRL-floor knock-out flag.
//!
//! Honesty scope: these implement the *required fields and layout* of the cited
//! standards over the open-core MCDA result. They do **not** reproduce any specific
//! tender's requirement identifiers, certified thresholds, or numbers — the TRL floor
//! and weighting are engineering choices, marked representative. Everything here is
//! **Modelled**.

use super::{DecisionReport, TradeReport};

/// An ISO 16290 Technology Readiness Level (1–9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TrlLevel {
    /// Level 1–9 (clamped on construction).
    pub level: u8,
}

impl TrlLevel {
    /// Construct, clamping to the valid ISO 16290 range `[1, 9]`.
    pub fn new(level: u8) -> Self {
        Self {
            level: level.clamp(1, 9),
        }
    }

    /// The ISO 16290 short definition of this level.
    pub fn definition(&self) -> &'static str {
        match self.level {
            1 => "Basic principles observed and reported",
            2 => "Technology concept and/or application formulated",
            3 => "Analytical and experimental critical function / characteristic proof-of-concept",
            4 => "Component and/or breadboard functional verification in laboratory environment",
            5 => "Component and/or breadboard critical function verification in a relevant environment",
            6 => "Model demonstrating the critical functions in a relevant environment",
            7 => "Model demonstrating the element performance in the operational environment",
            8 => "Actual system completed and accepted for flight (qualification)",
            _ => "Actual system flight-proven through successful mission operations",
        }
    }
}

/// A per-variant TRL assessment against a floor.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrlAssessment {
    pub variant: String,
    /// Declared TRL, or `None` if the variant did not declare one.
    pub trl: Option<u8>,
    /// The applied TRL floor (representative, configurable).
    pub floor: u8,
    /// `true` iff a TRL is declared and meets the floor.
    pub meets_floor: bool,
    /// `true` iff the variant is knocked out (no TRL, or below floor) — fail-closed.
    pub knocked_out: bool,
}

/// Assess every variant's declared TRL against `min_trl`. Fail-closed: a variant with
/// no declared TRL is knocked out (maturity cannot be demonstrated).
pub fn trl_assessment(report: &TradeReport, min_trl: u8) -> Vec<TrlAssessment> {
    report
        .outcomes
        .iter()
        .map(|o| {
            let trl = report.trl.get(&o.label).map(|t| t.level);
            let meets = trl.map(|l| l >= min_trl).unwrap_or(false);
            TrlAssessment {
                variant: o.label.clone(),
                trl,
                floor: min_trl,
                meets_floor: meets,
                knocked_out: !meets,
            }
        })
        .collect()
}

/// Markdown TRL table (ISO 16290) with the floor knock-out flag.
pub fn trl_markdown(report: &TradeReport, min_trl: u8) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "## Technology readiness (ISO 16290), floor TRL-{min_trl} (representative)\n\n"
    ));
    md.push_str("| Variant | TRL | Definition | Meets floor | Knock-out |\n");
    md.push_str("|---------|-----|------------|-------------|-----------|\n");
    for a in trl_assessment(report, min_trl) {
        let (trl_s, def) = match a.trl {
            Some(l) => (l.to_string(), TrlLevel::new(l).definition()),
            None => ("—".to_string(), "not declared"),
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            a.variant,
            trl_s,
            def,
            if a.meets_floor { "yes" } else { "no" },
            if a.knocked_out { "KNOCK-OUT" } else { "" },
        ));
    }
    md.push('\n');
    md
}

/// Render the **ECSS-E-ST-10C Annex L** trade-off report (markdown). Requires a
/// multi-criteria decision; for a single-FoM study a short note is returned instead.
pub fn ecss_annex_l_markdown(report: &TradeReport) -> String {
    let dr = match &report.decision {
        Some(d) => d,
        None => {
            return format!(
                "# Trade-off report (ECSS-E-ST-10C Annex L, structure)\n\n\
                 _Single-figure-of-merit study '{}' on `{}` — no multi-criteria weighting \
                 to report. Supply `criteria` for the full Annex L layout._\n",
                report.title, report.metric
            )
        }
    };
    let mut md = String::new();
    md.push_str("# Trade-off report (ECSS-E-ST-10C Annex L, structure)\n\n");
    md.push_str(&format!("**Trade-off objective.** {}\n\n", report.title));
    md.push_str(
        "_Method: weighted-sum trade-off over normalised criteria, per ECSS-E-ST-10C Annex L. \
         Modelled; weights are stakeholder choices, not certified values._\n\n",
    );

    // L.1 Criteria & weighting factors.
    md.push_str("## Selection criteria & weighting factors\n\n");
    md.push_str("| # | Criterion | Direction | Value function | Weighting factor |\n");
    md.push_str("|---|-----------|-----------|----------------|------------------|\n");
    for (i, c) in dr.criteria.iter().enumerate() {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {:.4} |\n",
            i + 1,
            c.name,
            c.direction,
            c.value_fn,
            c.weight
        ));
    }
    if let Some(ahp) = &dr.ahp {
        md.push_str(&format!(
            "\nWeights derived by AHP (pairwise comparison). Consistency Ratio = {} \
             (threshold {}, {}).\n",
            ahp.consistency_ratio
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "n/a".into()),
            ahp.threshold,
            if ahp.acceptable {
                "consistent"
            } else {
                "INCONSISTENT — weights not defensible"
            }
        ));
    }
    md.push('\n');

    // L.2 Weighted decision matrix (preference × weight per criterion).
    md.push_str("## Weighted decision matrix\n\n");
    md.push_str(&weighted_matrix_table(dr));

    // L.3 Sensitivity.
    md.push_str("\n## Sensitivity analysis\n\n");
    md.push_str(&sensitivity_block(dr));

    // L.3b Multi-method concordance — robustness of the ranking to method choice.
    md.push('\n');
    md.push_str(&multi_method_block(dr));

    // L.4 Recommendation.
    md.push_str("\n## Recommendation\n\n");
    md.push_str(&format!("{}\n", report.recommendation));
    md
}

/// Render the **NASA SE Handbook §6.8** decision-analysis report (markdown), with a
/// Table 6.8-1-style weighted decision matrix.
pub fn nasa_se_68_markdown(report: &TradeReport) -> String {
    let dr = match &report.decision {
        Some(d) => d,
        None => {
            return format!(
                "# Decision analysis report (NASA SE Handbook §6.8, structure)\n\n\
                 _Single-figure-of-merit study '{}' on `{}` — supply `criteria` for the \
                 Table 6.8-1 weighted decision matrix._\n",
                report.title, report.metric
            )
        }
    };
    let mut md = String::new();
    md.push_str("# Decision analysis report (NASA SE Handbook §6.8, structure)\n\n");
    md.push_str(&format!(
        "**Decision to be made.** Select the architecture for: {}\n\n",
        report.title
    ));
    md.push_str(
        "_Process (NASA/SP-2016-6105 Rev 2, §6.8): (1) define criteria & weights, \
         (2) identify alternatives, (3) evaluate against criteria, (4) select & document, \
         (5) test the decision's sensitivity. Modelled structure; representative weights._\n\n",
    );

    md.push_str("## Evaluation criteria (weights normalised to 1.0)\n\n");
    md.push_str("| Criterion | Weight |\n|-----------|--------|\n");
    for c in &dr.criteria {
        md.push_str(&format!("| {} | {:.4} |\n", c.name, c.weight));
    }
    md.push('\n');

    md.push_str("## Table 6.8-1 — weighted decision matrix\n\n");
    md.push_str(&weighted_matrix_table(dr));

    md.push_str("\n## Selection & rationale\n\n");
    md.push_str(&format!("{}\n\n", report.recommendation));
    md.push_str("## Decision sensitivity\n\n");
    md.push_str(&sensitivity_block(dr));

    // Multi-method concordance — does the recommendation survive the full MCDA panel?
    md.push('\n');
    md.push_str(&multi_method_block(dr));
    md
}

/// Render the **multi-method concordance** cross-check (markdown): runs the full
/// open-core MCDA panel (WSM · WPM · WASPAS · MOORA · TOPSIS · VIKOR · PROMETHEE II,
/// plus COPRAS when a cost criterion is present) over the decision's surviving variants
/// and reports how much the methods agree — a Modelled robustness check on the
/// recommendation. A recommendation that survives every method is defensible in a way a
/// single ranking is not; a split panel warns the decision is method-sensitive. Returns
/// a short note for a single-figure-of-merit study (no panel to run).
pub fn multi_method_markdown(report: &TradeReport) -> String {
    match &report.decision {
        Some(dr) => multi_method_block(dr),
        None => format!(
            "## Multi-method concordance (MCDA panel)\n\n\
             _Single-figure-of-merit study '{}' on `{}` — no multi-criteria panel to \
             cross-check. Supply `criteria` to run the WSM/WPM/WASPAS/MOORA/TOPSIS/VIKOR/\
             PROMETHEE II (+COPRAS) concordance._\n",
            report.title, report.metric
        ),
    }
}

/// The concordance section body, shared by [`multi_method_markdown`] and the two
/// standards-shaped reports. Runs [`DecisionReport::multi_method`] and tabulates it.
fn multi_method_block(dr: &DecisionReport) -> String {
    let mm = dr.multi_method();
    let name_of = |idx: usize| -> String {
        dr.scores
            .iter()
            .find(|s| s.variant_index == idx)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("v{idx}"))
    };

    let mut md = String::new();
    md.push_str("## Multi-method concordance (MCDA panel)\n\n");
    md.push_str(&format!("_{}_\n\n", mm.note));

    md.push_str("| Method | Winner | Ranking (best → worst) |\n");
    md.push_str("|--------|--------|------------------------|\n");
    for m in &mm.methods {
        let winner = m.winner.map(name_of).unwrap_or_else(|| "—".into());
        let order = m
            .ranking
            .iter()
            .map(|&i| name_of(i))
            .collect::<Vec<_>>()
            .join(" › ");
        md.push_str(&format!("| {} | {} | {} |\n", m.method, winner, order));
    }
    md.push('\n');

    let tau = mm
        .mean_kendall_tau
        .map(|t| format!("{t:.3}"))
        .unwrap_or_else(|| "n/a".into());
    let consensus = mm
        .consensus_winner
        .map(name_of)
        .unwrap_or_else(|| "—".into());
    md.push_str(&format!(
        "- **Consensus winner:** {} ({:.0}% of the {} method(s); {}).\n",
        consensus,
        mm.winner_agreement * 100.0,
        mm.methods.len(),
        if mm.unanimous_winner {
            "unanimous"
        } else {
            "split — inspect the tie"
        }
    ));
    md.push_str(&format!(
        "- **Panel concordance (mean pairwise Kendall τ):** {tau}.\n"
    ));
    md.push_str(&format!(
        "- **Rank-reversal variant pairs across the panel:** {}.\n",
        mm.rank_reversal_pairs
    ));
    md.push_str(&format!("- **Verdict:** {}.\n", mm.verdict()));
    md
}

/// A weighted decision-matrix table: rows = variants (ranked), columns = criteria
/// showing `preference × weight`, plus the total weighted score.
fn weighted_matrix_table(dr: &DecisionReport) -> String {
    let mut md = String::new();
    md.push('|');
    md.push_str(" Rank | Alternative ");
    for c in &dr.criteria {
        md.push_str(&format!("| {} ", c.name));
    }
    md.push_str("| Score |\n|");
    md.push_str("------|-------------");
    for _ in &dr.criteria {
        md.push_str("|------");
    }
    md.push_str("|-------|\n");

    let mut scores: Vec<&super::decision::VariantScore> = dr.scores.iter().collect();
    scores.sort_by_key(|s| s.rank.unwrap_or(usize::MAX));
    for s in scores {
        md.push_str(&format!(
            "| {} | {} ",
            s.rank.map(|r| r.to_string()).unwrap_or_else(|| "—".into()),
            s.name
        ));
        for w in &s.weighted {
            md.push_str(&format!("| {w:.4} "));
        }
        md.push_str(&format!("| {:.4} |\n", s.score));
    }
    md
}

/// A compact sensitivity summary shared by both report formats.
fn sensitivity_block(dr: &DecisionReport) -> String {
    let mut md = String::new();
    match &dr.min_flip {
        Some(f) => md.push_str(&format!(
            "- **Winner robustness:** smallest single-criterion weight change to flip the \
             winner is L1 = {:.4} (criterion '{}', would elect '{}').\n",
            f.l1_change, f.criterion, f.new_winner_name
        )),
        None => md.push_str(
            "- **Winner robustness:** no single-criterion weight change can flip the winner \
             (it dominates) — maximally robust.\n",
        ),
    }
    if let Some(w) = dr.winner {
        if let Some(e) = dr.smaa.iter().find(|e| e.variant_index == w) {
            md.push_str(&format!(
                "- **SMAA rank-1 acceptability of the winner:** {:.3} under weight uncertainty.\n",
                e.p_rank1
            ));
        }
    }
    md.push_str("- **Tornado (criteria by decision sensitivity):** ");
    let bars: Vec<String> = dr
        .tornado
        .iter()
        .map(|b| format!("{} ({:.3})", b.criterion, b.swing))
        .collect();
    md.push_str(&if bars.is_empty() {
        "—".to_string()
    } else {
        bars.join(", ")
    });
    md.push('\n');
    md
}

#[cfg(test)]
mod tests {
    use super::super::decision::{SensitivityCfg, TradeCriterion, Weighting};
    use super::super::{run_trade_study, TradeSpace, Variant};
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
        .with_sensitivity(SensitivityCfg::default())
        .with_weighting(Weighting::Direct)
        .with_trl("optical", TrlLevel::new(5))
        .with_trl("csac", TrlLevel::new(8))
    }

    #[test]
    fn trl_floor_knocks_out_immature_variants() {
        let report = run_trade_study(&multi_space()).unwrap();
        let a = trl_assessment(&report, 6);
        let optical = a.iter().find(|x| x.variant == "optical").unwrap();
        let csac = a.iter().find(|x| x.variant == "csac").unwrap();
        assert!(optical.knocked_out, "TRL-5 < floor 6 must knock out");
        assert!(!csac.knocked_out, "TRL-8 clears floor 6");
        // Undeclared TRL is fail-closed.
        let md = trl_markdown(&report, 6);
        assert!(md.contains("KNOCK-OUT"));
        assert!(md.contains("ISO 16290"));
    }

    #[test]
    fn ecss_annex_l_has_required_sections() {
        let report = run_trade_study(&multi_space()).unwrap();
        let md = ecss_annex_l_markdown(&report);
        assert!(md.contains("Annex L"));
        assert!(md.contains("weighting factors") || md.contains("weighting factor"));
        assert!(md.contains("Weighted decision matrix"));
        assert!(md.contains("Sensitivity analysis"));
        assert!(md.contains("Multi-method concordance"));
        assert!(md.contains("Recommendation"));
    }

    #[test]
    fn nasa_68_has_table_and_recommendation() {
        let report = run_trade_study(&multi_space()).unwrap();
        let md = nasa_se_68_markdown(&report);
        assert!(md.contains("§6.8"));
        assert!(md.contains("Table 6.8-1"));
        assert!(md.contains("Selection & rationale"));
        assert!(md.contains("Multi-method concordance"));
    }

    #[test]
    fn multi_method_block_lists_panel_and_consensus() {
        // The concordance section names the panel methods and reports the consensus,
        // the Kendall-τ concordance and a verdict. multi_space() has a cost criterion
        // (rms_ns) and positive data, so COPRAS runs too.
        let report = run_trade_study(&multi_space()).unwrap();
        let md = multi_method_markdown(&report);
        assert!(md.contains("Multi-method concordance"));
        assert!(md.contains("TOPSIS"));
        assert!(md.contains("WASPAS"));
        assert!(
            md.contains("COPRAS"),
            "a cost criterion is present → COPRAS runs"
        );
        assert!(md.contains("Consensus winner"));
        assert!(md.contains("Kendall τ"));
        assert!(md.contains("Verdict"));
    }

    #[test]
    fn multi_method_markdown_notes_single_fom() {
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
        assert!(multi_method_markdown(&report).contains("Single-figure-of-merit"));
    }

    #[test]
    fn single_fom_reports_note_the_missing_criteria() {
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
        assert!(ecss_annex_l_markdown(&report).contains("Single-figure-of-merit"));
        assert!(nasa_se_68_markdown(&report).contains("Single-figure-of-merit"));
    }
}
