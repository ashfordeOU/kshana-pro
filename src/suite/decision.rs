// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Multi-criteria decision layer (MCDA)
//!
//! The trade study upgraded from a single figure of merit to a defensible
//! **multi-criteria decision**. Each architecture variant is scored on several
//! incommensurate criteria; the decision is the composition of four open-core MCDA
//! pieces ([`kshana::mcda`]):
//!
//! * **WSM** — a [`kshana::mcda::wsm::DecisionMatrix`] over variants × criteria, with
//!   per-criterion value-function normalisation, hard knock-out constraints, and a
//!   weighted-sum aggregate score + ranking.
//! * **AHP** — when the weighting is [`Weighting::Ahp`], the criterion weights are the
//!   priority vector of a Saaty pairwise-comparison matrix, with the
//!   Consistency-Ratio gate (`CR < threshold`) surfaced in the report.
//! * **Pareto** — the non-dominated (Pareto-optimal) variants over the raw criterion
//!   objectives, plus the knee-point estimate; the winner being on the front is a
//!   necessary sanity check.
//! * **Sensitivity** — a tornado over criterion weights, an SMAA rank-1 acceptability
//!   index under weight uncertainty, and the minimum single-criterion weight change
//!   that flips the winner.
//!
//! ## Honesty scope (load-bearing)
//!
//! This layer **composes** the open core's MCDA math; it does not reimplement it. The
//! AHP priority/Consistency-Ratio kernel and the WSM aggregate are *Validated* against
//! external oracles **in the open core** (pymcdm, Saaty's RI table, SciPy/LAPACK). The
//! *composed decision* here — which criteria, which weights, which value functions —
//! is a **Modelled** structuring of those validated kernels over the simulator's
//! outputs. Garbage criteria in, garbage decision out: the product is the
//! [sensitivity][crate::suite::decision::DecisionReport::min_flip] caution, not a
//! claim that the chosen weights are correct. Nothing here is Validated.

use kshana::api::KshanaError;
use kshana::mcda::ahp::PairwiseMatrix;
use kshana::mcda::wsm::{Alternative, Constraint, Criterion, DecisionMatrix, Direction, ValueFn};
use kshana::mcda::{pareto, sensitivity, Objective};

/// One decision criterion: a name, the dot-path that extracts its raw value from each
/// variant's open-engine result JSON, an importance weight, a preference direction, a
/// value-function normalisation, and an optional hard knock-out constraint (an
/// ECSS *shall*-style elimination on the raw value).
#[derive(Debug, Clone)]
pub struct TradeCriterion {
    pub name: String,
    /// Dot-path into each variant's result JSON, e.g. `quantum.fom.holdover_s`.
    pub metric: String,
    /// Importance weight (≥ 0). Used directly under [`Weighting::Direct`]; under
    /// [`Weighting::Ahp`] the AHP priority vector overrides it.
    pub weight: f64,
    pub direction: Direction,
    pub value_fn: ValueFn,
    /// Optional knock-out constraint on the raw value; `None` = no constraint.
    pub constraint: Option<Constraint>,
}

impl TradeCriterion {
    /// A benefit criterion (higher raw value is better), min–max normalised.
    pub fn benefit(name: impl Into<String>, metric: impl Into<String>, weight: f64) -> Self {
        Self {
            name: name.into(),
            metric: metric.into(),
            weight,
            direction: Direction::Benefit,
            value_fn: ValueFn::MinMax,
            constraint: None,
        }
    }

    /// A cost criterion (lower raw value is better), min–max normalised.
    pub fn cost(name: impl Into<String>, metric: impl Into<String>, weight: f64) -> Self {
        Self {
            name: name.into(),
            metric: metric.into(),
            weight,
            direction: Direction::Cost,
            value_fn: ValueFn::MinMax,
            constraint: None,
        }
    }

    /// Builder: set the value function.
    pub fn with_value_fn(mut self, vf: ValueFn) -> Self {
        self.value_fn = vf;
        self
    }

    /// Builder: attach a hard knock-out constraint on the raw value.
    pub fn with_constraint(mut self, c: Constraint) -> Self {
        self.constraint = Some(c);
        self
    }

    /// The open-core WSM criterion for this trade criterion, with an explicit weight
    /// (so an AHP-derived weight can override the declared one).
    fn to_wsm(&self, weight: f64) -> Criterion {
        Criterion {
            name: self.name.clone(),
            weight,
            direction: self.direction,
            value_fn: self.value_fn,
            constraint: self.constraint,
        }
    }
}

/// How the criterion weights are derived.
#[derive(Debug, Clone, Default)]
pub enum Weighting {
    /// Use each criterion's declared `weight` directly (WSM normalises to sum one).
    #[default]
    Direct,
    /// Derive weights from an AHP reciprocal pairwise-comparison matrix on Saaty's
    /// 1–9 scale; the Consistency Ratio is checked against `cr_threshold` (the
    /// canonical acceptance gate is `0.10`) and surfaced in the report.
    Ahp {
        /// Row-major `n × n` reciprocal matrix, `n` = number of criteria.
        pairwise: Vec<Vec<f64>>,
        /// CR acceptance threshold (representative default `0.10`, configurable).
        cr_threshold: f64,
    },
}

/// Sensitivity-analysis configuration. All knobs are representative defaults a caller
/// may override; none is a certified threshold.
#[derive(Debug, Clone)]
pub struct SensitivityCfg {
    /// Relative ± weight perturbation for the tornado (default `0.10`).
    pub tornado_delta: f64,
    /// Number of SMAA weight draws (default `8192`).
    pub smaa_samples: usize,
    /// Seed for the deterministic SMAA Dirichlet RNG (default `0x00C0FFEE`).
    pub smaa_seed: u64,
    /// Dirichlet concentration per criterion; `None` = symmetric `[1, …]` (the
    /// classic "no preference information" uniform-simplex SMAA case).
    pub smaa_alpha: Option<Vec<f64>>,
}

impl Default for SensitivityCfg {
    fn default() -> Self {
        Self {
            tornado_delta: 0.10,
            smaa_samples: 8192,
            smaa_seed: 0x00C0_FFEE,
            smaa_alpha: None,
        }
    }
}

/// Summary of the AHP weight-consistency check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AhpSummary {
    /// Principal eigenvalue estimate `λ_max`.
    pub lambda_max: f64,
    /// Consistency Index `CI = (λ_max − n)/(n − 1)`.
    pub consistency_index: f64,
    /// Consistency Ratio `CR = CI / RI(n)`; `None` for `n ≤ 2` (trivially consistent).
    pub consistency_ratio: Option<f64>,
    /// The acceptance threshold applied (`CR < threshold`).
    pub threshold: f64,
    /// `true` iff the judgements clear the gate.
    pub acceptable: bool,
}

/// Per-criterion descriptor carried in the report (effective weight after weighting).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CriterionInfo {
    pub name: String,
    pub metric: String,
    /// `"benefit"` or `"cost"`.
    pub direction: String,
    /// Human-readable value-function description.
    pub value_fn: String,
    /// Effective normalised weight actually used in the aggregate (sums to one).
    pub weight: f64,
}

/// The score and disposition of one variant entering the decision.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VariantScore {
    /// Index into the original `TradeSpace::variants`.
    pub variant_index: usize,
    pub name: String,
    /// Weighted-sum aggregate in `[0, 1]`.
    pub score: f64,
    /// `true` if a knock-out constraint eliminated this variant.
    pub eliminated: bool,
    /// Name of the first criterion whose constraint eliminated it, if any.
    pub eliminated_by: Option<String>,
    /// `true` if this variant is on the Pareto front of the raw objectives.
    pub on_pareto: bool,
    /// WSM rank (1 = best) among survivors, or `None` if eliminated.
    pub rank: Option<usize>,
    /// Raw criterion values (aligned to the criteria order).
    pub raw: Vec<f64>,
    /// Per-criterion `[0, 1]` preference values.
    pub preference: Vec<f64>,
    /// Per-criterion weighted contributions (`preference · weight`); sum = `score`.
    pub weighted: Vec<f64>,
}

/// A variant excluded from the decision because it lacked a criterion value.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExcludedVariant {
    pub variant_index: usize,
    pub name: String,
    pub reason: String,
}

/// Knee-point estimate, mapped back to variant indices.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KneeInfo {
    pub variant_index: usize,
    pub name: String,
    pub distance: f64,
    /// The two front anchors (variant indices) defining the chord.
    pub anchors: (usize, usize),
}

/// One tornado bar, with the criterion named.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TornadoEntry {
    pub criterion: String,
    pub margin_low: f64,
    pub margin_high: f64,
    pub swing: f64,
}

/// SMAA rank-1 acceptability for one surviving variant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SmaaEntry {
    pub variant_index: usize,
    pub name: String,
    /// Probability this variant is the rank-1 winner under weight uncertainty.
    pub p_rank1: f64,
}

/// The minimum single-criterion weight change that flips the winner.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlipInfo {
    pub criterion: String,
    /// `L1` distance between the original and boundary normalised weight vectors.
    pub l1_change: f64,
    /// Variant index that becomes the winner just past the boundary.
    pub new_winner_index: usize,
    pub new_winner_name: String,
    /// The normalised weight vector at the flip boundary.
    pub new_weights: Vec<f64>,
}

/// The full multi-criteria decision report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DecisionReport {
    pub title: String,
    pub criteria: Vec<CriterionInfo>,
    /// `"direct"` or `"ahp"`.
    pub weighting_method: String,
    pub ahp: Option<AhpSummary>,
    /// Variant indices that entered the decision (had all criterion values).
    pub eligible: Vec<usize>,
    /// Variants dropped for a missing criterion value.
    pub excluded: Vec<ExcludedVariant>,
    /// One per eligible variant.
    pub scores: Vec<VariantScore>,
    /// Variant indices, best-first (survivors only).
    pub ranking: Vec<usize>,
    /// Winning variant index, or `None` if every variant was knocked out.
    pub winner: Option<usize>,
    /// Non-dominated variant indices (over the raw criterion objectives).
    pub pareto_front: Vec<usize>,
    pub knee: Option<KneeInfo>,
    pub tornado: Vec<TornadoEntry>,
    pub smaa: Vec<SmaaEntry>,
    pub min_flip: Option<FlipInfo>,
}

impl DecisionReport {
    /// The winning variant's name, if any.
    pub fn winner_name(&self) -> Option<&str> {
        self.winner
            .and_then(|w| self.scores.iter().find(|s| s.variant_index == w))
            .map(|s| s.name.as_str())
    }

    /// The winner's SMAA rank-1 acceptability, if a winner exists.
    pub fn winner_smaa(&self) -> Option<f64> {
        let w = self.winner?;
        self.smaa
            .iter()
            .find(|e| e.variant_index == w)
            .map(|e| e.p_rank1)
    }

    /// `true` iff the winner is on the Pareto front (vacuously `false` with no winner).
    pub fn winner_on_pareto(&self) -> bool {
        match self.winner {
            Some(w) => self.pareto_front.contains(&w),
            None => false,
        }
    }

    /// Run the full open-core MCDA panel (WSM · WPM · TOPSIS · VIKOR · PROMETHEE II)
    /// over this decision's **surviving** variants and measure how much the methods
    /// agree — a Modelled robustness cross-check derived from the report's own fields
    /// (no re-simulation). A recommendation that survives every method is defensible in
    /// a way a single ranking is not; a split panel is a warning the decision is
    /// method-sensitive. See [`crate::suite::multi_method`].
    pub fn multi_method(&self) -> super::multi_method::MultiMethodReport {
        let survivors: Vec<&VariantScore> = self.scores.iter().filter(|s| !s.eliminated).collect();
        let survivor_ids: Vec<usize> = survivors.iter().map(|s| s.variant_index).collect();
        let matrix: Vec<Vec<f64>> = survivors.iter().map(|s| s.raw.clone()).collect();
        let weights: Vec<f64> = self.criteria.iter().map(|c| c.weight).collect();
        let directions: Vec<Direction> = self
            .criteria
            .iter()
            .map(|c| {
                if c.direction == "cost" {
                    Direction::Cost
                } else {
                    Direction::Benefit
                }
            })
            .collect();
        super::multi_method::run_multi_method(&survivor_ids, &matrix, &weights, &directions)
    }
}

fn direction_str(d: Direction) -> &'static str {
    match d {
        Direction::Benefit => "benefit",
        Direction::Cost => "cost",
    }
}

fn value_fn_str(vf: ValueFn) -> String {
    match vf {
        ValueFn::MinMax => "min-max".to_string(),
        ValueFn::Target { target, spread } => format!("target({target}, ±{spread})"),
        ValueFn::Sigmoid {
            midpoint,
            steepness,
        } => format!("sigmoid(mid={midpoint}, k={steepness})"),
    }
}

/// Run the multi-criteria decision over the per-variant raw criterion values.
///
/// `raw[i]` holds variant `i`'s value for each criterion (aligned to `criteria`);
/// `None` means the metric was missing/non-numeric for that variant — such variants
/// are excluded (with a reason) rather than scored on partial data. At least one
/// variant must be fully eligible.
pub fn run_decision(
    title: &str,
    criteria: &[TradeCriterion],
    weighting: &Weighting,
    sens: &SensitivityCfg,
    labels: &[String],
    raw: &[Vec<Option<f64>>],
) -> Result<DecisionReport, KshanaError> {
    if criteria.is_empty() {
        return Err(KshanaError::InvalidInput(
            "a multi-criteria decision needs >= 1 criterion".into(),
        ));
    }
    let ncrit = criteria.len();

    // Partition variants into eligible (all criterion values present & finite) and
    // excluded (missing at least one).
    let mut eligible: Vec<usize> = Vec::new();
    let mut excluded: Vec<ExcludedVariant> = Vec::new();
    for (i, row) in raw.iter().enumerate() {
        let missing = row
            .iter()
            .position(|v| !v.map(f64::is_finite).unwrap_or(false));
        match missing {
            None => eligible.push(i),
            Some(j) => excluded.push(ExcludedVariant {
                variant_index: i,
                name: labels[i].clone(),
                reason: format!(
                    "missing/non-numeric value for criterion '{}'",
                    criteria[j].name
                ),
            }),
        }
    }
    if eligible.is_empty() {
        return Err(KshanaError::InvalidInput(
            "no variant produced all criterion values; decision inconclusive".into(),
        ));
    }

    // Eligible raw value rows.
    let eligible_raw: Vec<Vec<f64>> = eligible
        .iter()
        .map(|&i| raw[i].iter().map(|v| v.unwrap()).collect())
        .collect();

    // Derive effective weights (Direct or AHP).
    let (eff_weights, weighting_method, ahp) = match weighting {
        Weighting::Direct => {
            let w: Vec<f64> = criteria.iter().map(|c| c.weight).collect();
            (w, "direct".to_string(), None)
        }
        Weighting::Ahp {
            pairwise,
            cr_threshold,
        } => {
            if pairwise.len() != ncrit {
                return Err(KshanaError::InvalidInput(format!(
                    "AHP pairwise matrix is {}×{} but there are {ncrit} criteria",
                    pairwise.len(),
                    pairwise.first().map(|r| r.len()).unwrap_or(0)
                )));
            }
            let pm = PairwiseMatrix::new(pairwise.clone())
                .map_err(|e| KshanaError::InvalidInput(format!("invalid AHP matrix: {e}")))?;
            let r = pm.analyse_with(*cr_threshold, Default::default());
            let summary = AhpSummary {
                lambda_max: r.lambda_max,
                consistency_index: r.consistency_index,
                consistency_ratio: r.consistency_ratio,
                threshold: *cr_threshold,
                acceptable: r.acceptable,
            };
            (r.priorities, "ahp".to_string(), Some(summary))
        }
    };

    // Build and score the open-core WSM decision matrix over the eligible variants.
    let wsm_criteria: Vec<Criterion> = criteria
        .iter()
        .zip(eff_weights.iter())
        .map(|(c, &w)| c.to_wsm(w))
        .collect();
    let alts: Vec<Alternative> = eligible
        .iter()
        .enumerate()
        .map(|(pos, &i)| Alternative::new(labels[i].clone(), eligible_raw[pos].clone()))
        .collect();
    let dm = DecisionMatrix::new(wsm_criteria, alts)
        .map_err(|e| KshanaError::InvalidInput(format!("decision matrix invalid: {e}")))?;
    let wsm_report = dm
        .score()
        .map_err(|e| KshanaError::InvalidInput(format!("WSM scoring failed: {e}")))?;
    let norm_w = dm.normalized_weights();
    let prefs = dm.preference_matrix();

    // Pareto front + knee over the raw objectives (positions into `eligible`).
    let objectives: Vec<Objective> = criteria
        .iter()
        .map(|c| Objective::from(c.direction))
        .collect();
    let front_pos = pareto::pareto_front(&eligible_raw, &objectives);
    let knee = pareto::knee_of_front(&eligible_raw, &objectives, &front_pos).map(|k| KneeInfo {
        variant_index: eligible[k.index],
        name: labels[eligible[k.index]].clone(),
        distance: k.distance,
        anchors: (eligible[k.anchors.0], eligible[k.anchors.1]),
    });
    let on_front: Vec<bool> = (0..eligible.len())
        .map(|p| front_pos.contains(&p))
        .collect();

    // WSM rank map: eligible-position -> rank (1-based).
    let mut rank_of: Vec<Option<usize>> = vec![None; eligible.len()];
    for (r, &pos) in wsm_report.ranking.iter().enumerate() {
        rank_of[pos] = Some(r + 1);
    }

    // Per-variant scores.
    let scores: Vec<VariantScore> = wsm_report
        .scores
        .iter()
        .map(|s| {
            let pos = s.index;
            let weighted: Vec<f64> = prefs[pos]
                .iter()
                .zip(norm_w.iter())
                .map(|(p, w)| p * w)
                .collect();
            VariantScore {
                variant_index: eligible[pos],
                name: s.name.clone(),
                score: s.score,
                eliminated: s.eliminated,
                eliminated_by: s.eliminated_by.clone(),
                on_pareto: on_front[pos],
                rank: rank_of[pos],
                raw: eligible_raw[pos].clone(),
                preference: prefs[pos].clone(),
                weighted,
            }
        })
        .collect();

    let ranking: Vec<usize> = wsm_report.ranking.iter().map(|&p| eligible[p]).collect();
    let winner = wsm_report.winner().map(|p| eligible[p]);
    let pareto_front: Vec<usize> = front_pos.iter().map(|&p| eligible[p]).collect();

    // Sensitivity over the SURVIVING variants (knock-out excluded) so the decision
    // winner and the sensitivity winner share an index space and a tie-break.
    let survivor_pos: Vec<usize> = wsm_report
        .scores
        .iter()
        .filter(|s| !s.eliminated)
        .map(|s| s.index)
        .collect();
    let vm_surv: Vec<Vec<f64>> = survivor_pos.iter().map(|&p| prefs[p].clone()).collect();

    let alpha = sens.smaa_alpha.clone().unwrap_or_else(|| vec![1.0; ncrit]);
    let smaa_p = sensitivity::smaa_rank1(&alpha, &vm_surv, sens.smaa_samples, sens.smaa_seed);
    let smaa: Vec<SmaaEntry> = survivor_pos
        .iter()
        .zip(smaa_p.iter())
        .map(|(&p, &prob)| SmaaEntry {
            variant_index: eligible[p],
            name: labels[eligible[p]].clone(),
            p_rank1: prob,
        })
        .collect();

    let tornado: Vec<TornadoEntry> = sensitivity::tornado(&norm_w, &vm_surv, sens.tornado_delta)
        .into_iter()
        .map(|b| TornadoEntry {
            criterion: criteria[b.criterion].name.clone(),
            margin_low: b.margin_low,
            margin_high: b.margin_high,
            swing: b.swing,
        })
        .collect();

    let min_flip = sensitivity::min_weight_change_to_flip(&norm_w, &vm_surv).map(|f| {
        let new_idx = eligible[survivor_pos[f.new_winner]];
        FlipInfo {
            criterion: criteria[f.criterion].name.clone(),
            l1_change: f.l1_change,
            new_winner_index: new_idx,
            new_winner_name: labels[new_idx].clone(),
            new_weights: f.new_weights,
        }
    });

    let criteria_info: Vec<CriterionInfo> = criteria
        .iter()
        .zip(norm_w.iter())
        .map(|(c, &w)| CriterionInfo {
            name: c.name.clone(),
            metric: c.metric.clone(),
            direction: direction_str(c.direction).to_string(),
            value_fn: value_fn_str(c.value_fn),
            weight: w,
        })
        .collect();

    Ok(DecisionReport {
        title: title.to_string(),
        criteria: criteria_info,
        weighting_method,
        ahp,
        eligible,
        excluded,
        scores,
        ranking,
        winner,
        pareto_front,
        knee,
        tornado,
        smaa,
        min_flip,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 3-variant, 3-criterion study with a clear, hand-checkable structure.
    fn labels() -> Vec<String> {
        vec!["A".into(), "B".into(), "C".into()]
    }

    fn criteria_direct() -> Vec<TradeCriterion> {
        vec![
            TradeCriterion::benefit("coverage", "x.cov", 0.5),
            TradeCriterion::cost("cost", "x.cost", 0.3),
            TradeCriterion::benefit("holdover", "x.hold", 0.2),
        ]
    }

    fn raw_full() -> Vec<Vec<Option<f64>>> {
        vec![
            vec![Some(0.9), Some(100.0), Some(10.0)], // A: high coverage, mid cost
            vec![Some(0.6), Some(50.0), Some(20.0)],  // B: cheap, best holdover
            vec![Some(0.95), Some(200.0), Some(5.0)], // C: best coverage, expensive
        ]
    }

    #[test]
    fn direct_weighting_produces_a_full_decision() {
        let r = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        assert_eq!(r.weighting_method, "direct");
        assert_eq!(r.eligible, vec![0, 1, 2]);
        assert!(r.excluded.is_empty());
        assert!(r.winner.is_some());
        // Effective weights normalise to one.
        let wsum: f64 = r.criteria.iter().map(|c| c.weight).sum();
        assert!((wsum - 1.0).abs() < 1e-12);
        // Winner is on the Pareto front (a non-dominated choice).
        assert!(r.winner_on_pareto(), "winner should be Pareto-optimal");
        // SMAA probabilities over survivors sum to ~1.
        let psum: f64 = r.smaa.iter().map(|e| e.p_rank1).sum();
        assert!((psum - 1.0).abs() < 1e-9);
        // Per-variant weighted contributions reconstruct the score.
        for s in &r.scores {
            let recon: f64 = s.weighted.iter().sum();
            assert!((recon - s.score).abs() < 1e-12);
        }
    }

    #[test]
    fn ahp_weighting_surfaces_the_consistency_ratio() {
        // Consistent geometric 3×3 -> CR = 0, acceptable; priorities [4/7,2/7,1/7].
        let pairwise = vec![
            vec![1.0, 2.0, 4.0],
            vec![0.5, 1.0, 2.0],
            vec![0.25, 0.5, 1.0],
        ];
        let r = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Ahp {
                pairwise,
                cr_threshold: 0.10,
            },
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        assert_eq!(r.weighting_method, "ahp");
        let ahp = r.ahp.expect("AHP summary present");
        assert!(ahp.acceptable);
        assert!(ahp.consistency_ratio.unwrap() < 1e-9);
        // Weights follow the AHP priority vector, not the declared criterion weights.
        assert!((r.criteria[0].weight - 4.0 / 7.0).abs() < 1e-9);
        assert!((r.criteria[1].weight - 2.0 / 7.0).abs() < 1e-9);
        assert!((r.criteria[2].weight - 1.0 / 7.0).abs() < 1e-9);
    }

    #[test]
    fn inconsistent_ahp_is_flagged_not_accepted() {
        let pairwise = vec![
            vec![1.0, 9.0, 5.0],
            vec![1.0 / 9.0, 1.0, 3.0],
            vec![1.0 / 5.0, 1.0 / 3.0, 1.0],
        ];
        let r = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Ahp {
                pairwise,
                cr_threshold: 0.10,
            },
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        let ahp = r.ahp.unwrap();
        assert!(ahp.consistency_ratio.unwrap() > 0.10);
        assert!(!ahp.acceptable, "CR>0.10 must not clear the gate");
    }

    #[test]
    fn knockout_constraint_eliminates_and_excludes_from_ranking() {
        // Cost must be <= 150; variant C (cost 200) is eliminated.
        let mut crit = criteria_direct();
        crit[1] = crit[1].clone().with_constraint(Constraint {
            min: None,
            max: Some(150.0),
        });
        let r = run_decision(
            "t",
            &crit,
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        let c = r.scores.iter().find(|s| s.name == "C").unwrap();
        assert!(c.eliminated);
        assert_eq!(c.eliminated_by.as_deref(), Some("cost"));
        assert!(
            !r.ranking.contains(&2),
            "eliminated variant must not be ranked"
        );
        assert_ne!(r.winner, Some(2));
    }

    #[test]
    fn missing_metric_excludes_a_variant_not_the_whole_study() {
        let mut raw = raw_full();
        raw[1][2] = None; // B is missing holdover
        let r = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw,
        )
        .unwrap();
        assert_eq!(r.eligible, vec![0, 2]);
        assert_eq!(r.excluded.len(), 1);
        assert_eq!(r.excluded[0].variant_index, 1);
        assert!(r.winner.is_some());
    }

    #[test]
    fn no_eligible_variant_is_an_error() {
        let mut raw = raw_full();
        for row in &mut raw {
            row[0] = None;
        }
        assert!(matches!(
            run_decision(
                "t",
                &criteria_direct(),
                &Weighting::Direct,
                &SensitivityCfg::default(),
                &labels(),
                &raw,
            ),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn multi_method_panel_runs_over_a_decision() {
        // A decision report exposes a multi-method concordance panel derived from its
        // own survivor rows — no re-simulation.
        let r = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        let mm = r.multi_method();
        // The full panel ran (WSM/WPM/TOPSIS/VIKOR/PROMETHEE — all raw values > 0).
        assert!(mm.methods.len() >= 4, "panel should run several methods");
        // Every method's winner is one of the eligible survivors.
        for m in &mm.methods {
            if let Some(w) = m.winner {
                assert!(r.eligible.contains(&w));
            }
        }
        assert!(mm.mean_kendall_tau.is_some());
        assert!(!mm.verdict().is_empty());
        // Concordance is bounded and agreement is a fraction.
        let tau = mm.mean_kendall_tau.unwrap();
        assert!((-1.0..=1.0).contains(&tau));
        assert!((0.0..=1.0).contains(&mm.winner_agreement));
    }

    #[test]
    fn multi_method_excludes_knocked_out_variants() {
        // With a knock-out on C, the panel sees only the two survivors.
        let mut crit = criteria_direct();
        crit[1] = crit[1].clone().with_constraint(Constraint {
            min: None,
            max: Some(150.0),
        });
        let r = run_decision(
            "t",
            &crit,
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        let mm = r.multi_method();
        // Variant C (index 2) was eliminated, so no method may crown it.
        for m in &mm.methods {
            assert_ne!(m.winner, Some(2), "eliminated variant cannot win the panel");
            assert!(!m.ranking.contains(&2));
        }
    }

    #[test]
    fn sensitivity_is_seed_deterministic() {
        let a = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        let b = run_decision(
            "t",
            &criteria_direct(),
            &Weighting::Direct,
            &SensitivityCfg::default(),
            &labels(),
            &raw_full(),
        )
        .unwrap();
        let pa: Vec<f64> = a.smaa.iter().map(|e| e.p_rank1).collect();
        let pb: Vec<f64> = b.smaa.iter().map(|e| e.p_rank1).collect();
        assert_eq!(pa, pb, "fixed seed -> identical SMAA");
    }
}
