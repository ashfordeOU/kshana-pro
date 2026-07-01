// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L3 — Multi-method concordance (rank-agreement across the MCDA panel)
//!
//! A single MCDA method's ranking is only as trustworthy as its own assumptions:
//! TOPSIS rewards closeness to an ideal, VIKOR trades group utility against worst-case
//! regret, WSM sums, WPM multiplies, PROMETHEE outranks. A trade recommendation that
//! survives *all* of them is defensible in a way no single ranking can be — and one
//! that flips between them is a warning the decision is method-sensitive, not robust.
//!
//! This module runs the same eligible decision through the full open-core MCDA panel
//! ([`kshana::mcda`]) — **WSM, WPM, WASPAS, MOORA, TOPSIS, VIKOR, PROMETHEE II** and
//! (when a cost criterion is present) **COPRAS** — and quantifies how much they agree:
//!
//! * per-method rankings and winners;
//! * pairwise **Kendall τ** rank correlation between every pair of methods (the
//!   open core's `resilience::stats::kendall_tau`, validated against SciPy);
//! * the mean pairwise τ as a single **concordance** score;
//! * the **consensus winner** (plurality across methods), whether it is **unanimous**,
//!   and the fraction of methods that agree; and
//! * the number of **rank-reversal** variant pairs — pairs whose relative order is not
//!   the same under every method.
//!
//! ## Honesty scope (load-bearing)
//!
//! Every method in the panel is *Validated in the open core* against an independent
//! third-party library (pymcdm for WSM/WPM/WASPAS/MOORA/TOPSIS/VIKOR/PROMETHEE II;
//! pyDecision for COPRAS). The **concordance analysis
//! here is a Modelled composition** of those validated kernels: it measures agreement
//! among methods on the *given* criteria and weights — it does **not** validate that
//! the criteria, weights or value judgements are correct. High concordance means the
//! recommendation is robust to *method choice*, nothing more. Nothing here is Validated.

use kshana::mcda::copras::copras;
use kshana::mcda::moora::moora;
use kshana::mcda::promethee::{promethee_ii, PreferenceFunction};
use kshana::mcda::topsis::topsis;
use kshana::mcda::vikor::vikor;
use kshana::mcda::waspas::waspas;
use kshana::mcda::wpm::wpm;
use kshana::mcda::wsm::{Alternative, Criterion, DecisionMatrix, Direction};
use kshana::mcda::Objective;
use kshana::resilience::stats::kendall_tau;

/// One method's outcome in the panel.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PanelRanking {
    /// Method name: `"WSM"`, `"WPM"`, `"WASPAS"`, `"MOORA"`, `"TOPSIS"`, `"VIKOR"`,
    /// `"PROMETHEE II"`, `"COPRAS"`.
    pub method: String,
    /// Survivor variant indices, best-first (mapped to the caller's variant indices).
    pub ranking: Vec<usize>,
    /// Winning variant index, or `None` if there were no survivors.
    pub winner: Option<usize>,
}

/// Pairwise Kendall-τ agreement between two methods' rankings.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PairAgreement {
    pub method_a: String,
    pub method_b: String,
    /// Kendall τ ∈ [−1, 1]; 1 = identical order, −1 = exactly reversed.
    pub kendall_tau: f64,
}

/// The multi-method concordance report.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MultiMethodReport {
    /// Per-method rankings (only methods that could run on the data).
    pub methods: Vec<PanelRanking>,
    /// Pairwise Kendall τ between every pair of methods.
    pub pairwise_tau: Vec<PairAgreement>,
    /// Mean of the pairwise τ — the panel concordance (higher = more method-robust).
    /// `None` when fewer than two methods ran.
    pub mean_kendall_tau: Option<f64>,
    /// Plurality winner across methods (ties broken by smallest variant index).
    pub consensus_winner: Option<usize>,
    /// `true` iff every method that ran picked the same winner.
    pub unanimous_winner: bool,
    /// Fraction of methods whose winner is the consensus winner, in `[0, 1]`.
    pub winner_agreement: f64,
    /// Number of unordered survivor pairs whose relative order is not identical under
    /// every method (rank reversals across the panel).
    pub rank_reversal_pairs: usize,
    /// Honest note on scope and any method skipped (e.g. WPM on non-positive data).
    pub note: String,
}

impl MultiMethodReport {
    /// A compact robustness verdict for a human reader.
    pub fn verdict(&self) -> String {
        match (self.unanimous_winner, self.mean_kendall_tau) {
            (true, _) => "unanimous winner across all methods — method-robust".into(),
            (false, Some(t)) if t >= 0.8 => {
                "strong concordance (mean τ ≥ 0.8) with a split winner — inspect the tie".into()
            }
            (false, Some(t)) if t >= 0.5 => {
                "moderate concordance — the ranking is somewhat method-sensitive".into()
            }
            (false, Some(_)) => {
                "weak concordance — the recommendation is method-sensitive, treat with caution"
                    .into()
            }
            (false, None) => "too few methods ran to assess concordance".into(),
        }
    }
}

/// Map a WSM [`Direction`] to an [`Objective`].
fn objective_of(d: Direction) -> Objective {
    match d {
        Direction::Benefit => Objective::Max,
        Direction::Cost => Objective::Min,
    }
}

/// Rank vector (position → 0-based rank) for a best-first ranking of `n` items, as
/// `f64` for [`kendall_tau`]. Lower rank number = better.
fn rank_vector(ranking: &[usize], n: usize) -> Vec<f64> {
    let mut rv = vec![0.0f64; n];
    for (rank, &pos) in ranking.iter().enumerate() {
        rv[pos] = rank as f64;
    }
    rv
}

/// Run the full MCDA panel over the survivor decision matrix and measure concordance.
///
/// * `survivor_ids` — caller variant indices for each survivor row (used to label
///   rankings/winners).
/// * `matrix` — `survivors × criteria` raw values (survivors only, no knocked-out
///   variants).
/// * `weights` — per-criterion weights (used as given; typically the effective,
///   sum-to-one weights the decision used).
/// * `directions` — per-criterion benefit/cost.
///
/// Uniform min–max/normalisation per each method's own convention makes the panel
/// comparable; WPM is skipped (honestly noted) when any value is non-positive, since it
/// requires strictly positive data. Returns `None` inputs guarded by the caller: an
/// empty matrix yields an empty report with a note.
pub fn run_multi_method(
    survivor_ids: &[usize],
    matrix: &[Vec<f64>],
    weights: &[f64],
    directions: &[Direction],
) -> MultiMethodReport {
    let n = matrix.len();
    let ncrit = weights.len();
    let objectives: Vec<Objective> = directions.iter().map(|&d| objective_of(d)).collect();

    let mut methods: Vec<PanelRanking> = Vec::new();
    let mut skipped: Vec<&str> = Vec::new();

    // Helper: convert a survivor-position ranking to caller variant indices.
    let to_ids = |ranking: Vec<usize>| -> Vec<usize> {
        ranking.into_iter().map(|p| survivor_ids[p]).collect()
    };

    // Strictly-positive data is required by the ratio/product/proportional methods
    // (they normalise by division / take powers); COPRAS additionally needs a cost
    // criterion for its S⁻ term to be meaningful. Computed once, reused by the gates.
    let positive = matrix.iter().all(|row| row.iter().all(|&x| x > 0.0));
    let has_cost = directions.iter().any(|&d| matches!(d, Direction::Cost));

    if n >= 1 && ncrit >= 1 {
        // WSM (min–max value functions), via the open-core DecisionMatrix.
        let wsm_crit: Vec<Criterion> = (0..ncrit)
            .map(|j| match directions[j] {
                Direction::Benefit => Criterion::benefit(format!("c{j}"), weights[j]),
                Direction::Cost => Criterion::cost(format!("c{j}"), weights[j]),
            })
            .collect();
        let alts: Vec<Alternative> = matrix
            .iter()
            .enumerate()
            .map(|(i, row)| Alternative::new(format!("v{i}"), row.clone()))
            .collect();
        if let Ok(dm) = DecisionMatrix::new(wsm_crit, alts) {
            if let Ok(rep) = dm.score() {
                let ranking = to_ids(rep.ranking.clone());
                let winner = ranking.first().copied();
                methods.push(PanelRanking {
                    method: "WSM".into(),
                    ranking,
                    winner,
                });
            }
        }

        // WPM — requires strictly positive data.
        if positive {
            if let Ok(r) = wpm(matrix, weights, &objectives) {
                let ranking = to_ids(r.ranking);
                let winner = ranking.first().copied();
                methods.push(PanelRanking {
                    method: "WPM".into(),
                    ranking,
                    winner,
                });
            }
        } else {
            skipped.push("WPM (non-positive values)");
        }

        // WASPAS — the λ=0.5 convex blend of WSM and WPM; strictly positive data
        // (linear normalisation divides).
        if positive {
            if let Ok(r) = waspas(matrix, weights, &objectives, 0.5) {
                let ranking = to_ids(r.ranking);
                let winner = ranking.first().copied();
                methods.push(PanelRanking {
                    method: "WASPAS".into(),
                    ranking,
                    winner,
                });
            }
        } else {
            skipped.push("WASPAS (non-positive values)");
        }

        // TOPSIS.
        if let Ok(r) = topsis(matrix, weights, &objectives) {
            let ranking = to_ids(r.ranking);
            let winner = ranking.first().copied();
            methods.push(PanelRanking {
                method: "TOPSIS".into(),
                ranking,
                winner,
            });
        }

        // VIKOR (consensus v = 0.5).
        if let Ok(r) = vikor(matrix, weights, &objectives, 0.5) {
            let ranking = to_ids(r.ranking);
            let winner = ranking.first().copied();
            methods.push(PanelRanking {
                method: "VIKOR".into(),
                ranking,
                winner,
            });
        }

        // PROMETHEE II (usual criterion).
        let prefs = vec![PreferenceFunction::Usual; ncrit];
        if let Ok(r) = promethee_ii(matrix, weights, &objectives, &prefs) {
            let ranking = to_ids(r.ranking);
            let winner = ranking.first().copied();
            methods.push(PanelRanking {
                method: "PROMETHEE II".into(),
                ranking,
                winner,
            });
        }

        // MOORA (ratio system) — vector normalisation, benefit total minus cost total.
        // Skipped only if a column is degenerate (zero L2 norm), which the open kernel
        // reports as an error.
        if let Ok(r) = moora(matrix, weights, &objectives) {
            let ranking = to_ids(r.ranking);
            let winner = ranking.first().copied();
            methods.push(PanelRanking {
                method: "MOORA".into(),
                ranking,
                winner,
            });
        } else {
            skipped.push("MOORA (degenerate column)");
        }

        // COPRAS (proportional) — needs strictly positive data and at least one cost
        // criterion for its inverse-cost significance term to be defined.
        if positive && has_cost {
            if let Ok(r) = copras(matrix, weights, &objectives) {
                let ranking = to_ids(r.ranking);
                let winner = ranking.first().copied();
                methods.push(PanelRanking {
                    method: "COPRAS".into(),
                    ranking,
                    winner,
                });
            }
        } else if !positive {
            skipped.push("COPRAS (non-positive values)");
        } else {
            skipped.push("COPRAS (no cost criterion)");
        }
    }

    // Pairwise Kendall τ over survivor-position rank vectors.
    // Reconstruct each method's survivor-position ranking (invert the id mapping).
    let id_to_pos = |ids: &[usize]| -> Vec<usize> {
        ids.iter()
            .map(|id| survivor_ids.iter().position(|s| s == id).unwrap())
            .collect()
    };
    let rank_vecs: Vec<Vec<f64>> = methods
        .iter()
        .map(|m| rank_vector(&id_to_pos(&m.ranking), n))
        .collect();

    let mut pairwise_tau: Vec<PairAgreement> = Vec::new();
    let mut tau_sum = 0.0;
    let mut tau_count = 0usize;
    for a in 0..methods.len() {
        for b in (a + 1)..methods.len() {
            let tau = if n >= 2 {
                kendall_tau(&rank_vecs[a], &rank_vecs[b])
            } else {
                1.0 // a single survivor is trivially co-ranked
            };
            tau_sum += tau;
            tau_count += 1;
            pairwise_tau.push(PairAgreement {
                method_a: methods[a].method.clone(),
                method_b: methods[b].method.clone(),
                kendall_tau: tau,
            });
        }
    }
    let mean_kendall_tau = if tau_count > 0 {
        Some(tau_sum / tau_count as f64)
    } else {
        None
    };

    // Consensus winner: plurality of per-method winners, ties → smallest variant index.
    let winners: Vec<usize> = methods.iter().filter_map(|m| m.winner).collect();
    let (consensus_winner, winner_agreement, unanimous_winner) = if winners.is_empty() {
        (None, 0.0, false)
    } else {
        let mut best: Option<(usize, usize)> = None; // (variant_id, count)
        let mut uniq: Vec<usize> = winners.clone();
        uniq.sort_unstable();
        uniq.dedup();
        for &cand in &uniq {
            let count = winners.iter().filter(|&&w| w == cand).count();
            match best {
                Some((_, bc)) if count <= bc => {}
                _ => best = Some((cand, count)),
            }
        }
        let (cw, cc) = best.unwrap();
        let frac = cc as f64 / winners.len() as f64;
        (Some(cw), frac, cc == winners.len())
    };

    // Rank-reversal pairs: unordered survivor pairs whose order differs across methods.
    let mut rank_reversal_pairs = 0usize;
    if methods.len() >= 2 && n >= 2 {
        for i in 0..n {
            for j in (i + 1)..n {
                // Does method 0 order (i before j) agree across all methods?
                let order_in = |rv: &[f64]| rv[i] < rv[j];
                let first = order_in(&rank_vecs[0]);
                if rank_vecs.iter().any(|rv| order_in(rv) != first) {
                    rank_reversal_pairs += 1;
                }
            }
        }
    }

    let note = {
        let mut s = format!(
            "Modelled concordance over {} MCDA method(s) on the given criteria/weights; \
             measures robustness to method choice only, not input correctness.",
            methods.len()
        );
        if !skipped.is_empty() {
            s.push_str(" Skipped: ");
            s.push_str(&skipped.join(", "));
            s.push('.');
        }
        s
    };

    MultiMethodReport {
        methods,
        pairwise_tau,
        mean_kendall_tau,
        consensus_winner,
        unanimous_winner,
        winner_agreement,
        rank_reversal_pairs,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A study with a dominant alternative: variant 2 is best on every criterion, so
    // every method must agree it wins (unanimous, τ high).
    fn dominant_matrix() -> (Vec<usize>, Vec<Vec<f64>>, Vec<f64>, Vec<Direction>) {
        let ids = vec![10, 11, 12];
        let matrix = vec![
            vec![0.5, 0.5, 0.5],
            vec![0.6, 0.6, 0.6],
            vec![0.9, 0.9, 0.9], // dominates
        ];
        let weights = vec![0.4, 0.35, 0.25];
        let dirs = vec![Direction::Benefit, Direction::Benefit, Direction::Benefit];
        (ids, matrix, weights, dirs)
    }

    #[test]
    fn dominant_alternative_is_unanimous() {
        let (ids, m, w, d) = dominant_matrix();
        let r = run_multi_method(&ids, &m, &w, &d);
        // All-benefit positive data runs every method except COPRAS (needs a cost).
        assert!(
            r.methods.len() >= 7,
            "panel should run WSM/WPM/WASPAS/MOORA/TOPSIS/VIKOR/PROMETHEE"
        );
        assert!(r.methods.iter().any(|mm| mm.method == "WASPAS"));
        assert!(r.methods.iter().any(|mm| mm.method == "MOORA"));
        assert!(
            r.methods.iter().all(|mm| mm.method != "COPRAS"),
            "COPRAS is skipped without a cost criterion"
        );
        assert!(r.note.contains("COPRAS"), "note records the COPRAS skip");
        assert_eq!(r.consensus_winner, Some(12));
        assert!(
            r.unanimous_winner,
            "a dominant alternative wins under every method"
        );
        assert!((r.winner_agreement - 1.0).abs() < 1e-12);
        assert_eq!(
            r.rank_reversal_pairs, 0,
            "no reversals when one alt dominates"
        );
        assert!(r.mean_kendall_tau.unwrap() > 0.99);
    }

    #[test]
    fn full_panel_of_eight_runs_with_a_cost_criterion() {
        // Positive data with a genuine cost criterion → every method runs, COPRAS
        // included. Variant 2 has the best benefits and the lowest cost, so it
        // dominates and the whole eight-method panel agrees.
        let ids = vec![0, 1, 2];
        let matrix = vec![
            vec![0.5, 0.5, 3.0], // criterion 2 is a cost
            vec![0.6, 0.6, 2.0],
            vec![0.9, 0.9, 1.0], // best benefits, lowest cost → dominates
        ];
        let weights = vec![0.4, 0.35, 0.25];
        let dirs = vec![Direction::Benefit, Direction::Benefit, Direction::Cost];
        let r = run_multi_method(&ids, &matrix, &weights, &dirs);
        assert_eq!(
            r.methods.len(),
            8,
            "WSM/WPM/WASPAS/MOORA/TOPSIS/VIKOR/PROMETHEE/COPRAS all run"
        );
        assert!(r.methods.iter().any(|mm| mm.method == "COPRAS"));
        assert_eq!(r.consensus_winner, Some(2));
        assert!(
            r.unanimous_winner,
            "the dominant variant wins under all eight"
        );
    }

    #[test]
    fn wpm_skipped_on_nonpositive_values() {
        let ids = vec![0, 1];
        // A zero value blocks WPM (it takes logs/reciprocals) but not the others.
        let m = vec![vec![0.0, 2.0], vec![1.0, 1.0]];
        let w = vec![0.5, 0.5];
        let d = vec![Direction::Benefit, Direction::Benefit];
        let r = run_multi_method(&ids, &m, &w, &d);
        assert!(r.note.contains("WPM"), "note records the WPM skip");
        assert!(r.methods.iter().all(|mm| mm.method != "WPM"));
        assert!(r.methods.iter().any(|mm| mm.method == "TOPSIS"));
    }

    #[test]
    fn identical_rows_give_perfect_concordance() {
        // Every alternative identical → all methods tie → τ = 1 pairwise.
        let ids = vec![0, 1, 2];
        let m = vec![vec![1.0, 1.0], vec![1.0, 1.0], vec![1.0, 1.0]];
        let w = vec![0.5, 0.5];
        let d = vec![Direction::Benefit, Direction::Benefit];
        let r = run_multi_method(&ids, &m, &w, &d);
        assert!(r.mean_kendall_tau.unwrap() > 0.999);
        assert_eq!(r.rank_reversal_pairs, 0);
    }

    #[test]
    fn conflicting_criteria_can_split_the_panel() {
        // Two alternatives, each best on one of two equally-weighted opposing criteria:
        // methods can legitimately disagree, exercising the reversal counter.
        let ids = vec![0, 1];
        let m = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let w = vec![0.5, 0.5];
        let d = vec![Direction::Benefit, Direction::Benefit];
        let r = run_multi_method(&ids, &m, &w, &d);
        // The report is well-formed regardless of which way ties break.
        assert!(r.mean_kendall_tau.is_some());
        assert!(r.winner_agreement >= 0.0 && r.winner_agreement <= 1.0);
        assert!(!r.verdict().is_empty());
    }
}
