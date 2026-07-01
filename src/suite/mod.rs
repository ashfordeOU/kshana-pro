// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Trade-study & assurance **framework** (demonstrator)
//!
//! Turns a set of architecture variants into a ranked, audit-grade trade study by
//! running each variant through the **embedded open engine**
//! ([`kshana::api::run_scenario`]) and comparing them. Each variant's result carries
//! the open core's reproducible `scenario_hash`, so the study is replayable — the
//! spine of an assurance evidence pack (PRR/ISRR-style).
//!
//! ## Single figure of merit → multi-criteria decision
//!
//! A [`TradeSpace`] with no [`criteria`](TradeSpace::criteria) ranks on a single
//! figure of merit (the original behaviour, now routed through the open core's
//! [`kshana::mcda::wsm`] single-criterion scorer — provably a plain sort). Adding
//! criteria upgrades the study to a full **multi-criteria decision** ([`decision`]):
//! a [`kshana::mcda::wsm::DecisionMatrix`] over variants × criteria, optional
//! AHP-derived weights with the `CR < 0.10` gate, a Pareto front, and a weight-
//! sensitivity pass. On top of the decision the suite emits a tamper-evident
//! [W3C-PROV record][provenance], a [GSN][gsn] claim–argument–evidence structure, an
//! [ECSS Verification Control Document][vcd], [ECSS Annex L / NASA §6.8 reports][reports],
//! and a multi-criteria [assurance gate][gate].
//!
//! ## Honesty scope (load-bearing)
//!
//! Everything in this suite is **Modelled**. It composes the open core's MCDA kernels
//! (the AHP priority/Consistency-Ratio and WSM aggregate are *Validated* against
//! external oracles *in the open core*) and structures the result against published
//! standards (ECSS-E-ST-10C Annex L, NASA SE Handbook §6.8, ISO 16290 TRL, GSN). The
//! standards are implemented *structurally*; thresholds that are engineering choices
//! (TRL floor, SMAA acceptability, flip robustness) are marked configurable and
//! representative. There is no external measured dataset here, so **nothing in this
//! suite is Validated**.
//!
//! ## What is built vs still gated
//!
//! - **Built (demonstrator):** the automated multi-criteria trade engine, the
//!   assurance-artifact generators, and the gate — all on open capabilities.
//! - **Still gated:** packaging this as a *sold, licensed Kshana Pro SKU* waits on
//!   proven demand (a paying customer) — see [`product_gated`].

pub mod decision;
pub mod gate;
pub mod gsn;
pub mod multi_method;
pub mod provenance;
pub mod reports;
pub mod vcd;
pub mod vv_dossier;

use std::collections::BTreeMap;

use kshana::api::{run_scenario, KshanaError};
use kshana::mcda::wsm::{Alternative, Criterion, DecisionMatrix};

use crate::util::dot_path_number;

pub use decision::{run_decision, DecisionReport, SensitivityCfg, TradeCriterion, Weighting};
pub use gate::{GateThresholds, GateVerdict};
pub use multi_method::{run_multi_method, MultiMethodReport, PairAgreement, PanelRanking};
pub use provenance::DecisionProvenance;
pub use reports::TrlLevel;
pub use vv_dossier::{build_dossier, CredibilityDossier, TwinValidationEvidence};

/// One architecture option in the trade space: a label + a full scenario (the open
/// engine runs it unchanged, so any open scenario kind is a valid variant).
#[derive(Debug, Clone)]
pub struct Variant {
    pub label: String,
    pub scenario_toml: String,
}

/// A trade space: variants compared either on one figure of merit (no `criteria`) or
/// across several criteria (a full multi-criteria decision).
#[derive(Debug, Clone)]
pub struct TradeSpace {
    pub title: String,
    /// Single figure of merit: dot-path into each variant's result JSON, e.g.
    /// `quantum.fom.holdover_s`. Used only when `criteria` is empty.
    pub metric: String,
    /// True if a *lower* single-FoM metric value is better (e.g. position error).
    pub lower_better: bool,
    pub variants: Vec<Variant>,
    /// Multi-criteria decision criteria. **Empty ⇒ single-FoM legacy mode.**
    pub criteria: Vec<TradeCriterion>,
    /// How criterion weights are derived (direct, or AHP pairwise comparison).
    pub weighting: Weighting,
    /// Sensitivity-analysis configuration (tornado / SMAA / flip).
    pub sensitivity: SensitivityCfg,
    /// Optional per-variant ISO 16290 TRL self-declarations, keyed by variant label.
    pub trl: BTreeMap<String, TrlLevel>,
    /// Assurance-gate thresholds (representative, configurable).
    pub gate: GateThresholds,
}

impl TradeSpace {
    /// A single-figure-of-merit trade space (the original constructor shape).
    pub fn single_fom(
        title: impl Into<String>,
        metric: impl Into<String>,
        lower_better: bool,
        variants: Vec<Variant>,
    ) -> Self {
        Self {
            title: title.into(),
            metric: metric.into(),
            lower_better,
            variants,
            criteria: Vec::new(),
            weighting: Weighting::Direct,
            sensitivity: SensitivityCfg::default(),
            trl: BTreeMap::new(),
            gate: GateThresholds::default(),
        }
    }

    /// A multi-criteria trade space over the given criteria (direct weighting,
    /// default sensitivity config). Builders refine it from there.
    pub fn multi_criteria(
        title: impl Into<String>,
        variants: Vec<Variant>,
        criteria: Vec<TradeCriterion>,
    ) -> Self {
        Self {
            title: title.into(),
            metric: String::new(),
            lower_better: false,
            variants,
            criteria,
            weighting: Weighting::Direct,
            sensitivity: SensitivityCfg::default(),
            trl: BTreeMap::new(),
            gate: GateThresholds::default(),
        }
    }

    /// Builder: set the weighting method.
    pub fn with_weighting(mut self, w: Weighting) -> Self {
        self.weighting = w;
        self
    }

    /// Builder: set the sensitivity configuration.
    pub fn with_sensitivity(mut self, s: SensitivityCfg) -> Self {
        self.sensitivity = s;
        self
    }

    /// Builder: declare a variant's ISO 16290 TRL.
    pub fn with_trl(mut self, label: impl Into<String>, trl: TrlLevel) -> Self {
        self.trl.insert(label.into(), trl);
        self
    }

    /// Builder: set the assurance-gate thresholds.
    pub fn with_gate(mut self, g: GateThresholds) -> Self {
        self.gate = g;
        self
    }

    /// `true` when this space drives a multi-criteria decision.
    pub fn is_multi_criteria(&self) -> bool {
        !self.criteria.is_empty()
    }
}

/// The outcome for one variant.
#[derive(Debug, Clone)]
pub struct VariantOutcome {
    pub label: String,
    pub scenario_hash: Option<String>,
    /// Single-FoM metric value, or — in multi-criteria mode — the weighted-sum score.
    pub metric_value: Option<f64>,
    pub rank: Option<usize>,
    pub error: Option<String>,
    /// Per-criterion raw values (aligned to `TradeSpace::criteria`); empty in
    /// single-FoM mode.
    pub criterion_values: Vec<Option<f64>>,
}

/// The ranked trade study + the material for an assurance evidence pack.
#[derive(Debug, Clone)]
pub struct TradeReport {
    pub title: String,
    /// Label of the quantity ranked on (the single FoM, or `"MCDA weighted-sum"`).
    pub metric: String,
    pub lower_better: bool,
    pub outcomes: Vec<VariantOutcome>,
    pub winner: Option<String>,
    pub recommendation: String,
    /// The full multi-criteria decision, when criteria were supplied.
    pub decision: Option<DecisionReport>,
    /// Per-variant ISO 16290 TRL declarations carried through for the assurance layer.
    pub trl: BTreeMap<String, TrlLevel>,
    /// The assurance-gate thresholds in force (carried for the gate + reports).
    pub gate: GateThresholds,
}

/// **Run the trade study.** Each variant is executed once on the open engine; then,
/// depending on whether the space declares criteria, the variants are ranked either
/// on a single figure of merit (routed through [`kshana::mcda::wsm`]) or by a full
/// multi-criteria decision ([`decision::run_decision`]). Variant failures are captured
/// (not fatal) so a partial trade still produces a report.
pub fn run_trade_study(space: &TradeSpace) -> Result<TradeReport, KshanaError> {
    if space.variants.len() < 2 {
        return Err(KshanaError::InvalidInput(
            "a trade study needs >= 2 variants".into(),
        ));
    }
    let multi = space.is_multi_criteria();
    if !multi && space.metric.trim().is_empty() {
        return Err(KshanaError::InvalidInput(
            "a single-FoM trade study needs a non-empty `metric` dot-path".into(),
        ));
    }

    // Execute every variant once; keep its result JSON for metric extraction.
    struct Run {
        hash: Option<String>,
        json: serde_json::Value,
        run_err: Option<String>,
    }
    let runs: Vec<Run> = space
        .variants
        .iter()
        .map(|v| match run_scenario(&v.scenario_toml) {
            Ok(out) => {
                let json: serde_json::Value =
                    serde_json::from_str(&out.json).unwrap_or(serde_json::Value::Null);
                let hash = json
                    .get("scenario_hash")
                    .and_then(|h| h.as_str())
                    .map(String::from);
                Run {
                    hash,
                    json,
                    run_err: None,
                }
            }
            Err(e) => Run {
                hash: None,
                json: serde_json::Value::Null,
                run_err: Some(format!("run failed: {e}")),
            },
        })
        .collect();

    // Build the per-variant outcome skeletons (criterion values for the multi path).
    let mut outcomes: Vec<VariantOutcome> = space
        .variants
        .iter()
        .zip(runs.iter())
        .map(|(v, r)| {
            let criterion_values: Vec<Option<f64>> = if multi {
                space
                    .criteria
                    .iter()
                    .map(|c| {
                        if r.run_err.is_some() {
                            None
                        } else {
                            dot_path_number(&r.json, &c.metric)
                        }
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let legacy_metric = if !multi && r.run_err.is_none() {
                dot_path_number(&r.json, &space.metric)
            } else {
                None
            };
            VariantOutcome {
                label: v.label.clone(),
                scenario_hash: r.hash.clone(),
                metric_value: legacy_metric,
                rank: None,
                error: r.run_err.clone(),
                criterion_values,
            }
        })
        .collect();

    if multi {
        finish_multi_criteria(space, &mut outcomes)
    } else {
        finish_single_fom(space, outcomes)
    }
}

/// Single-figure-of-merit ranking (original behaviour), routed through the open
/// core's WSM single-criterion scorer — which reproduces a plain ascending/descending
/// sort exactly (the back-compat golden).
fn finish_single_fom(
    space: &TradeSpace,
    mut outcomes: Vec<VariantOutcome>,
) -> Result<TradeReport, KshanaError> {
    // Variants that ran but produced no metric get the canonical not-found note.
    for o in &mut outcomes {
        if o.error.is_none() && o.metric_value.is_none() {
            o.error = Some(format!(
                "metric '{}' not found or non-numeric",
                space.metric
            ));
        }
    }

    let winner = rank_single_fom(&mut outcomes, space.lower_better);
    let ranked = outcomes.iter().filter(|o| o.rank.is_some()).count();
    let recommendation = match (&winner, ranked) {
        (Some(w), n) => format!(
            "Recommended architecture: '{w}' — best '{}' ({}) across {} comparable variant(s).",
            space.metric,
            if space.lower_better {
                "lower is better"
            } else {
                "higher is better"
            },
            n
        ),
        (None, _) => "No variant produced a comparable metric; trade inconclusive.".to_string(),
    };

    Ok(TradeReport {
        title: space.title.clone(),
        metric: space.metric.clone(),
        lower_better: space.lower_better,
        outcomes,
        winner,
        recommendation,
        decision: None,
        trl: space.trl.clone(),
        gate: space.gate.clone(),
    })
}

/// Rank the metric-bearing variants on a single criterion via the open WSM scorer,
/// stamping `rank` in place and returning the winner's label.
fn rank_single_fom(outcomes: &mut [VariantOutcome], lower_better: bool) -> Option<String> {
    let idx: Vec<usize> = (0..outcomes.len())
        .filter(|&i| outcomes[i].metric_value.is_some())
        .collect();
    if idx.is_empty() {
        return None;
    }
    let crit = if lower_better {
        Criterion::cost("fom", 1.0)
    } else {
        Criterion::benefit("fom", 1.0)
    };
    let alts: Vec<Alternative> = idx
        .iter()
        .map(|&i| {
            Alternative::new(
                outcomes[i].label.clone(),
                vec![outcomes[i].metric_value.unwrap()],
            )
        })
        .collect();
    let dm = DecisionMatrix::new(vec![crit], alts).ok()?;
    let rep = dm.score().ok()?;
    for (rank, &pos) in rep.ranking.iter().enumerate() {
        outcomes[idx[pos]].rank = Some(rank + 1);
    }
    rep.winner().map(|pos| outcomes[idx[pos]].label.clone())
}

/// Multi-criteria decision: build the decision over the extracted criterion values,
/// then fold the result back into the per-variant outcomes.
fn finish_multi_criteria(
    space: &TradeSpace,
    outcomes: &mut Vec<VariantOutcome>,
) -> Result<TradeReport, KshanaError> {
    let labels: Vec<String> = outcomes.iter().map(|o| o.label.clone()).collect();
    let raw: Vec<Vec<Option<f64>>> = outcomes
        .iter()
        .map(|o| o.criterion_values.clone())
        .collect();

    let dr = decision::run_decision(
        &space.title,
        &space.criteria,
        &space.weighting,
        &space.sensitivity,
        &labels,
        &raw,
    )?;

    // Map decision scores/ranks/eliminations back onto the outcomes.
    for o in outcomes.iter_mut() {
        o.rank = None;
    }
    for s in &dr.scores {
        let o = &mut outcomes[s.variant_index];
        o.metric_value = Some(s.score);
        o.rank = s.rank;
        if s.eliminated {
            o.error = Some(format!(
                "knocked out by constraint on '{}'",
                s.eliminated_by.as_deref().unwrap_or("?")
            ));
        }
    }
    for ex in &dr.excluded {
        outcomes[ex.variant_index].error = Some(ex.reason.clone());
    }

    let winner = dr.winner_name().map(String::from);
    let n_eligible = dr.eligible.len();
    let n_crit = dr.criteria.len();
    let recommendation = match &winner {
        Some(w) => {
            let smaa = dr
                .winner_smaa()
                .map(|p| format!("{p:.2}"))
                .unwrap_or_else(|| "n/a".into());
            format!(
                "Recommended architecture: '{w}' — rank-1 of {n_eligible} eligible variant(s) on a \
                 {n_crit}-criterion weighted-sum decision ({} weights); Pareto-optimal: {}; \
                 SMAA rank-1 acceptability {smaa}.",
                dr.weighting_method,
                if dr.winner_on_pareto() { "yes" } else { "no" },
            )
        }
        None => "Every variant was knocked out or excluded; decision inconclusive.".to_string(),
    };

    Ok(TradeReport {
        title: space.title.clone(),
        metric: format!("MCDA weighted-sum ({n_crit} criteria)"),
        lower_better: false,
        outcomes: std::mem::take(outcomes),
        winner,
        recommendation,
        decision: Some(dr),
        trl: space.trl.clone(),
        gate: space.gate.clone(),
    })
}

/// Render an audit-grade evidence-pack markdown: the reproducible record of the trade
/// (every variant's scenario hash + metric + rank), the multi-criteria decision (when
/// present), and a tamper-evident W3C-PROV provenance record with a Merkle root —
/// suitable as the basis of a PRR/ISRR-style assurance artifact.
pub fn evidence_pack_markdown(report: &TradeReport) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Trade-study evidence pack — {}\n\n",
        report.title
    ));
    md.push_str(&format!(
        "Generated by Kshana Pro v{} over the open Kshana engine.\n\n",
        crate::version()
    ));
    md.push_str(&format!(
        "**Figure of merit:** `{}` ({})\n\n",
        report.metric,
        if report.lower_better {
            "lower is better"
        } else {
            "higher is better"
        }
    ));
    md.push_str("| Rank | Variant | Metric | Scenario hash (reproducible) | Note |\n");
    md.push_str("|------|---------|--------|------------------------------|------|\n");
    let mut rows: Vec<&VariantOutcome> = report.outcomes.iter().collect();
    rows.sort_by_key(|o| o.rank.unwrap_or(usize::MAX));
    for o in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            o.rank.map(|r| r.to_string()).unwrap_or_else(|| "—".into()),
            o.label,
            o.metric_value
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "—".into()),
            o.scenario_hash
                .as_deref()
                .map(|h| &h[..h.len().min(16)])
                .unwrap_or("—"),
            o.error.as_deref().unwrap_or("ok"),
        ));
    }
    md.push_str(&format!(
        "\n**Recommendation:** {}\n\n",
        report.recommendation
    ));

    // Multi-criteria decision detail.
    if let Some(dr) = &report.decision {
        md.push_str(&decision_markdown(dr));
    }

    // Tamper-evident provenance.
    let prov = provenance::build_provenance(report);
    md.push_str(&prov.markdown());

    md.push_str(
        "\n_Reproducibility: each scenario hash is the open engine's SHA-256 of the \
         exact scenario; re-running it reproduces the bits. The Merkle root above binds \
         the inputs, per-variant outcomes, weights and decision into one tamper-evident \
         digest. This record is the spine of an assurance evidence pack (PRR/ISRR-style)._\n",
    );
    md
}

/// Markdown for the multi-criteria decision section.
fn decision_markdown(dr: &DecisionReport) -> String {
    let mut md = String::new();
    md.push_str("## Multi-criteria decision\n\n");
    md.push_str(&format!("Weighting method: **{}**", dr.weighting_method));
    if let Some(ahp) = &dr.ahp {
        md.push_str(&format!(
            " (AHP CR = {}, threshold {}, {})",
            ahp.consistency_ratio
                .map(|c| format!("{c:.4}"))
                .unwrap_or_else(|| "n/a".into()),
            ahp.threshold,
            if ahp.acceptable {
                "ACCEPTABLE"
            } else {
                "REJECTED"
            }
        ));
    }
    md.push_str("\n\n### Criteria & weights\n\n");
    md.push_str("| Criterion | Metric | Direction | Value fn | Weight |\n");
    md.push_str("|-----------|--------|-----------|----------|--------|\n");
    for c in &dr.criteria {
        md.push_str(&format!(
            "| {} | `{}` | {} | {} | {:.4} |\n",
            c.name, c.metric, c.direction, c.value_fn, c.weight
        ));
    }
    md.push_str("\n### Weighted scores\n\n");
    md.push_str("| Rank | Variant | Score | Pareto | Note |\n");
    md.push_str("|------|---------|-------|--------|------|\n");
    let mut scores: Vec<&decision::VariantScore> = dr.scores.iter().collect();
    scores.sort_by_key(|s| s.rank.unwrap_or(usize::MAX));
    for s in scores {
        md.push_str(&format!(
            "| {} | {} | {:.4} | {} | {} |\n",
            s.rank.map(|r| r.to_string()).unwrap_or_else(|| "—".into()),
            s.name,
            s.score,
            if s.on_pareto { "✓" } else { "" },
            if s.eliminated {
                format!(
                    "knocked out by '{}'",
                    s.eliminated_by.as_deref().unwrap_or("?")
                )
            } else {
                "ok".to_string()
            },
        ));
    }
    md.push_str(&format!(
        "\nPareto front: {}\n\n",
        if dr.pareto_front.is_empty() {
            "—".to_string()
        } else {
            dr.pareto_front
                .iter()
                .filter_map(|&i| dr.scores.iter().find(|s| s.variant_index == i))
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));

    md.push_str("### Weight sensitivity\n\n");
    if let Some(f) = &dr.min_flip {
        md.push_str(&format!(
            "- Minimum single-criterion weight change to flip the winner: **L1 = {:.4}** \
             (via criterion '{}', new winner '{}').\n",
            f.l1_change, f.criterion, f.new_winner_name
        ));
    } else {
        md.push_str(
            "- The winner cannot be flipped by any single-criterion weight change \
             (it dominates on the criteria) — maximally robust.\n",
        );
    }
    if let Some(top) = dr.tornado.first() {
        md.push_str(&format!(
            "- Most decision-sensitive criterion (tornado): '{}' (margin swing {:.4}).\n",
            top.criterion, top.swing
        ));
    }
    md.push('\n');
    md
}

/// The seam guard for the **gated** part of L3: packaging this as the sold, licensed
/// Kshana Pro SKU with full ECSS/MBSE artifact generation. The framework is built and
/// runs; productisation waits on proven demand.
pub fn product_gated() -> KshanaError {
    KshanaError::Unsupported(
        "Packaging the trade-study/assurance suite as a licensed Kshana Pro SKU (with full \
         ECSS/MBSE artifact generation) is gated on proven demand (a second paying customer); \
         not built on spec. The framework itself is built. See BUILD-TRACKER PR15–PR18."
            .into(),
    )
}

/// Convenience: run a trade study and also produce its evidence-pack markdown.
pub fn run_with_evidence(space: &TradeSpace) -> Result<(TradeReport, String), KshanaError> {
    let report = run_trade_study(space)?;
    let md = evidence_pack_markdown(&report);
    Ok((report, md))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two ephemeris variants differing only in run length → different n_samples.
    // Deterministic and fast; exercises the full trade mechanics.
    fn ephem(duration_s: f64) -> String {
        format!(
            "kind = \"ephemeris\"\n\
             tle = \"\"\"\n\
             1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
             2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
             \"\"\"\n\
             step_s = 30.0\n\
             duration_s = {duration_s}\n"
        )
    }

    fn trade() -> TradeSpace {
        TradeSpace::single_fom(
            "ephemeris sample-count trade",
            "n_samples",
            false,
            vec![
                Variant {
                    label: "short".into(),
                    scenario_toml: ephem(60.0),
                },
                Variant {
                    label: "long".into(),
                    scenario_toml: ephem(300.0),
                },
            ],
        )
    }

    #[test]
    fn ranks_variants_by_metric() {
        let report = run_trade_study(&trade()).unwrap();
        assert_eq!(report.winner.as_deref(), Some("long")); // more samples, higher-better
        let long = report.outcomes.iter().find(|o| o.label == "long").unwrap();
        let short = report.outcomes.iter().find(|o| o.label == "short").unwrap();
        assert_eq!(long.rank, Some(1));
        assert_eq!(short.rank, Some(2));
        assert!(long.metric_value.unwrap() > short.metric_value.unwrap());
        assert!(
            long.scenario_hash.is_some(),
            "open engine must stamp a scenario hash"
        );
        assert!(
            report.decision.is_none(),
            "single-FoM has no decision block"
        );
    }

    #[test]
    fn lower_better_inverts_ranking() {
        let mut t = trade();
        t.lower_better = true;
        let report = run_trade_study(&t).unwrap();
        assert_eq!(report.winner.as_deref(), Some("short"));
    }

    #[test]
    fn missing_metric_is_captured_not_fatal() {
        let mut t = trade();
        t.metric = "does.not.exist".into();
        let report = run_trade_study(&t).unwrap();
        assert!(report.winner.is_none());
        assert!(report.outcomes.iter().all(|o| o.error.is_some()));
    }

    #[test]
    fn failed_variant_is_captured_not_fatal() {
        let mut t = trade();
        // An ephemeris with neither tle nor orbit → the open engine rejects it.
        t.variants[0].scenario_toml = "kind = \"ephemeris\"\nstep_s = 30.0\n".into();
        let report = run_trade_study(&t).unwrap();
        let bad = report.outcomes.iter().find(|o| o.label == "short").unwrap();
        assert!(bad.error.is_some() && bad.metric_value.is_none());
        // The good variant still ranks.
        assert_eq!(report.winner.as_deref(), Some("long"));
    }

    #[test]
    fn evidence_pack_records_hashes_and_recommendation() {
        let (report, md) = run_with_evidence(&trade()).unwrap();
        assert!(md.contains("evidence pack"));
        assert!(md.contains("Scenario hash"));
        assert!(md.contains("Recommendation"));
        assert!(md.contains(&report.winner.unwrap()));
        // The provenance section is always appended.
        assert!(md.contains("Merkle root"));
    }

    #[test]
    fn too_few_variants_is_an_error() {
        let mut t = trade();
        t.variants.truncate(1);
        assert!(matches!(
            run_trade_study(&t),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn product_sku_is_gated() {
        assert!(matches!(product_gated(), KshanaError::Unsupported(_)));
    }

    /// Back-compat golden: the WSM-routed single-FoM ranking reproduces a plain sort
    /// exactly — both senses, with a known input — so the upgrade to the MCDA spine
    /// did not change the single-criterion behaviour.
    #[test]
    fn single_fom_reproduces_plain_sort_exactly() {
        let mut outcomes: Vec<VariantOutcome> = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0]
            .iter()
            .enumerate()
            .map(|(i, &v)| VariantOutcome {
                label: format!("a{i}"),
                scenario_hash: None,
                metric_value: Some(v),
                rank: None,
                error: None,
                criterion_values: Vec::new(),
            })
            .collect();
        let vals: Vec<f64> = outcomes.iter().map(|o| o.metric_value.unwrap()).collect();

        // Higher-better → descending sort, ties by ascending index.
        let mut hi = outcomes.clone();
        let w = rank_single_fom(&mut hi, false).unwrap();
        let mut expect: Vec<usize> = (0..vals.len()).collect();
        expect.sort_by(|&i, &j| vals[j].total_cmp(&vals[i]).then(i.cmp(&j)));
        let got: Vec<usize> = {
            let mut v: Vec<usize> = (0..hi.len()).collect();
            v.sort_by_key(|&i| hi[i].rank.unwrap());
            v
        };
        assert_eq!(got, expect, "benefit ranking must equal a descending sort");
        assert_eq!(w, format!("a{}", expect[0]));

        // Lower-better → ascending sort.
        let lo = rank_single_fom(&mut outcomes, true).unwrap();
        let mut expect_lo: Vec<usize> = (0..vals.len()).collect();
        expect_lo.sort_by(|&i, &j| vals[i].total_cmp(&vals[j]).then(i.cmp(&j)));
        assert_eq!(lo, format!("a{}", expect_lo[0]));
    }
}
