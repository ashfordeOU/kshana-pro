// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Cost-aware gauge-constrained optimal-experiment-design (OED) optimizer.
//!
//! Given a menu of candidate measurement blocks — each holding a 7×7 Fisher
//! information contribution and a scalar relative cost — selects the
//! budget-constrained subset that best breaks the lunocenter-X ↔ scale
//! degeneracy quantified by [`kshana::lunar_identifiability::decompose`].
//!
//! # Open-core boundary
//! The *menu* is open: [`kshana::lunar_techniques`] supplies `MeasurementBlock`,
//! `block_from_rows`, `combine` and `representative_lunar_menu`, which keep the
//! open reference result (the marginal-gain finding) fully reproducible from the
//! AGPL engine alone. What lives here is the budget-constrained **optimizer** the
//! open crate deliberately does not ship: the greedy/exact subset selection and
//! the cost/degeneracy Pareto frontier.
//!
//! Two solvers are provided:
//! - [`greedy_design`]: forward-greedy heuristic; fast, near-optimal for
//!   concave-monotone objectives (Modelled).
//! - [`exhaustive_best`]: exact brute-force oracle over all 2ⁿ subsets
//!   (n ≤ 16 guarded).
//!
//! # Honesty note
//! `greedy_design` is a **Modelled** capability: it is provably optimal on
//! representative menus where the optimal first choice at every step is unique
//! (monotone gain ordering), but can be sub-optimal on adversarial inputs.
//! `exhaustive_best` is the exact ground truth; the invariant `e ≥ g` always
//! holds. All degeneracy metrics and CRLB figures inherit the **Modelled** status
//! from `kshana::lunar_identifiability::decompose`.

use kshana::lunar_techniques::{combine, MeasurementBlock};

#[derive(Clone, Debug)]
pub struct DesignResult {
    pub chosen: Vec<usize>,
    pub total_cost: f64,
    pub degeneracy_metric: f64,
    pub origin_crlb_m: f64,
}

/// One point of the cost/degeneracy trade frontier.
#[derive(Clone, Debug)]
pub struct FrontierPoint {
    pub budget: f64,
    pub total_cost: f64,
    pub degeneracy_metric: f64,
    pub origin_crlb_m: f64,
    /// Labels of the selected blocks at this budget level.
    pub chosen: Vec<String>,
}

/// Evaluate degeneracy metric and origin CRLB for a chosen subset.
fn evaluate(blocks: &[MeasurementBlock], chosen: &[usize], rel_tol: f64) -> (f64, f64) {
    let combined = combine(blocks, chosen);
    let d = kshana::lunar_identifiability::decompose(&combined, rel_tol);
    (d.degeneracy_metric, d.origin_crlb_m)
}

/// Forward-greedy heuristic: at each step, add the unchosen affordable block
/// that maximises `(new_metric − cur_metric) / cost`; stop when no affordable
/// block yields a positive marginal gain.
///
/// **Modelled:** near-optimal for concave-monotone objectives; can be
/// sub-optimal on adversarial menus. Validated against [`exhaustive_best`] on
/// representative menus — see `tests::greedy_matches_exhaustive_on_constructed_menu`.
pub fn greedy_design(blocks: &[MeasurementBlock], budget: f64, rel_tol: f64) -> DesignResult {
    let mut chosen: Vec<usize> = Vec::new();
    let mut total_cost = 0.0_f64;
    let mut cur_metric = evaluate(blocks, &[], rel_tol).0;

    loop {
        let mut best_gpc = 0.0_f64;
        let mut best_idx: Option<usize> = None;

        for (i, block) in blocks.iter().enumerate() {
            if chosen.contains(&i) {
                continue;
            }
            if total_cost + block.cost > budget + 1e-12 {
                continue;
            }
            let mut trial = chosen.clone();
            trial.push(i);
            let (new_metric, _) = evaluate(blocks, &trial, rel_tol);
            let gain = new_metric - cur_metric;
            if gain <= 0.0 {
                continue;
            }
            let gpc = gain / block.cost;
            if best_idx.is_none() || gpc > best_gpc {
                best_gpc = gpc;
                best_idx = Some(i);
            }
        }

        match best_idx {
            None => break,
            Some(idx) => {
                chosen.push(idx);
                total_cost += blocks[idx].cost;
                cur_metric = evaluate(blocks, &chosen, rel_tol).0;
            }
        }
    }

    let (degeneracy_metric, origin_crlb_m) = evaluate(blocks, &chosen, rel_tol);
    DesignResult {
        chosen,
        total_cost,
        degeneracy_metric,
        origin_crlb_m,
    }
}

/// Exact brute-force oracle: evaluate all 2ⁿ subsets (n ≤ 16 recommended;
/// logs a warning if larger). Returns the affordable subset with the largest
/// `degeneracy_metric` (tie-break: lower cost).
///
/// This is the ground-truth optimizer. The invariant `exhaustive_best ≥
/// greedy_design` always holds.
pub fn exhaustive_best(blocks: &[MeasurementBlock], budget: f64, rel_tol: f64) -> DesignResult {
    let n = blocks.len();
    if n > 16 {
        eprintln!(
            "exhaustive_best: n={n} > 16; iterating {} subsets may be slow",
            1u64 << n
        );
    }

    let mut best_metric = f64::NEG_INFINITY;
    let mut best_cost = f64::INFINITY;
    let mut best_chosen: Vec<usize> = Vec::new();

    for mask in 0u64..(1u64 << n) {
        let chosen: Vec<usize> = (0..n)
            .filter(|&i| mask & (1u64 << (i as u32)) != 0)
            .collect();
        let cost: f64 = chosen.iter().map(|&i| blocks[i].cost).sum();
        if cost > budget + 1e-12 {
            continue;
        }
        let (metric, _) = evaluate(blocks, &chosen, rel_tol);
        // Primary: highest metric. Tie-break: lower cost.
        let improves = metric > best_metric + 1e-15
            || (metric >= best_metric - 1e-15 && cost < best_cost - 1e-12);
        if improves {
            best_metric = metric;
            best_cost = cost;
            best_chosen = chosen;
        }
    }

    // Re-evaluate to get origin_crlb_m (avoids storing it in the loop).
    let (degeneracy_metric, origin_crlb_m) = evaluate(blocks, &best_chosen, rel_tol);
    DesignResult {
        chosen: best_chosen,
        total_cost: best_cost,
        degeneracy_metric,
        origin_crlb_m,
    }
}

/// Sweep `budgets` through [`greedy_design`], returning the trade frontier. This is a
/// PARAMETERISED trade curve (Modelled costs/precisions), NOT a single optimum.
pub fn pareto_frontier(
    blocks: &[MeasurementBlock],
    budgets: &[f64],
    rel_tol: f64,
) -> Vec<FrontierPoint> {
    budgets
        .iter()
        .map(|&budget| {
            let result = greedy_design(blocks, budget, rel_tol);
            let chosen = result
                .chosen
                .iter()
                .map(|&i| blocks[i].label.clone())
                .collect();
            FrontierPoint {
                budget,
                total_cost: result.total_cost,
                degeneracy_metric: result.degeneracy_metric,
                origin_crlb_m: result.origin_crlb_m,
                chosen,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kshana::lunar_techniques::{block_from_rows, representative_lunar_menu};

    /// Conventional tolerance for `decompose` across the open core.
    const REL_TOL: f64 = 1e-12;

    /// A menu small enough for the exact oracle to sweep (2^n subsets) while still
    /// being the real representative geometry rather than a toy.
    fn small_menu() -> Vec<MeasurementBlock> {
        let menu = representative_lunar_menu();
        assert!(
            menu.len() >= 3,
            "the open menu collapsed to {} blocks; these tests would be vacuous",
            menu.len()
        );
        menu.into_iter().take(5).collect()
    }

    fn total_cost(blocks: &[MeasurementBlock], chosen: &[usize]) -> f64 {
        chosen.iter().map(|&i| blocks[i].cost).sum()
    }

    /// The test this module's own documentation cites — and which did not exist until
    /// the optimizer was un-gated. `greedy_design`'s doc comment claims it is "validated
    /// against `exhaustive_best` on representative menus — see
    /// `tests::greedy_matches_exhaustive_on_constructed_menu`". A claim that names its
    /// evidence has to have some.
    #[test]
    fn greedy_matches_exhaustive_on_constructed_menu() {
        let blocks = small_menu();
        let budget: f64 = blocks.iter().map(|b| b.cost).sum::<f64>() * 0.6;
        let g = greedy_design(&blocks, budget, REL_TOL);
        let e = exhaustive_best(&blocks, budget, REL_TOL);
        assert!(
            (g.degeneracy_metric - e.degeneracy_metric).abs()
                <= 1e-9 * e.degeneracy_metric.abs().max(1.0),
            "greedy is sub-optimal on the representative menu at budget {budget}: \
             greedy {} vs exact {} (chosen {:?} vs {:?}). The heuristic is Modelled and \
             may lose on adversarial input, but the representative menu is the case the \
             doc comment claims it wins.",
            g.degeneracy_metric,
            e.degeneracy_metric,
            g.chosen,
            e.chosen
        );
    }

    /// The invariant the module states in prose: the exact oracle is never beaten by the
    /// heuristic. Checked across the whole budget range rather than at one point.
    #[test]
    fn exhaustive_is_never_worse_than_greedy_at_any_budget() {
        let blocks = small_menu();
        let full: f64 = blocks.iter().map(|b| b.cost).sum();
        for step in 0..=8 {
            let budget = full * f64::from(step) / 8.0;
            let g = greedy_design(&blocks, budget, REL_TOL);
            let e = exhaustive_best(&blocks, budget, REL_TOL);
            assert!(
                e.degeneracy_metric >= g.degeneracy_metric - 1e-9,
                "e >= g violated at budget {budget}: exact {} < greedy {}",
                e.degeneracy_metric,
                g.degeneracy_metric
            );
        }
    }

    /// A budget is a constraint, not a suggestion — for both solvers.
    #[test]
    fn neither_solver_ever_spends_more_than_the_budget() {
        let blocks = small_menu();
        let full: f64 = blocks.iter().map(|b| b.cost).sum();
        for step in 0..=8 {
            let budget = full * f64::from(step) / 8.0;
            for (name, r) in [
                ("greedy", greedy_design(&blocks, budget, REL_TOL)),
                ("exhaustive", exhaustive_best(&blocks, budget, REL_TOL)),
            ] {
                assert!(
                    total_cost(&blocks, &r.chosen) <= budget + 1e-9,
                    "{name} overspent at budget {budget}: chose {:?} costing {}",
                    r.chosen,
                    total_cost(&blocks, &r.chosen)
                );
                assert!(
                    (r.total_cost - total_cost(&blocks, &r.chosen)).abs() <= 1e-9,
                    "{name} reported total_cost {} but its selection costs {}",
                    r.total_cost,
                    total_cost(&blocks, &r.chosen)
                );
            }
        }
    }

    /// Nothing is affordable at zero budget, and the report says so rather than
    /// returning an unpriced selection.
    #[test]
    fn a_zero_budget_buys_nothing() {
        let blocks = small_menu();
        for (name, r) in [
            ("greedy", greedy_design(&blocks, 0.0, REL_TOL)),
            ("exhaustive", exhaustive_best(&blocks, 0.0, REL_TOL)),
        ] {
            assert!(
                r.chosen.is_empty(),
                "{name} bought {:?} for nothing",
                r.chosen
            );
            assert_eq!(r.total_cost, 0.0, "{name} reported a nonzero cost");
        }
    }

    /// A selection is a set of real, distinct blocks.
    #[test]
    fn a_selection_is_distinct_and_in_range() {
        let blocks = small_menu();
        let full: f64 = blocks.iter().map(|b| b.cost).sum();
        let r = greedy_design(&blocks, full, REL_TOL);
        let mut seen = std::collections::BTreeSet::new();
        for &i in &r.chosen {
            assert!(i < blocks.len(), "index {i} is outside the menu");
            assert!(seen.insert(i), "block {i} was selected twice");
        }
    }

    /// The frontier reports the labels of what it actually chose, at every budget, and
    /// respects the budget it was handed.
    #[test]
    fn the_frontier_reports_the_labels_of_what_it_chose() {
        let blocks = small_menu();
        let full: f64 = blocks.iter().map(|b| b.cost).sum();
        let budgets: Vec<f64> = (0..=4).map(|k| full * f64::from(k) / 4.0).collect();
        let frontier = pareto_frontier(&blocks, &budgets, REL_TOL);
        assert_eq!(frontier.len(), budgets.len());
        for (p, &budget) in frontier.iter().zip(&budgets) {
            assert_eq!(p.budget, budget);
            assert!(p.total_cost <= budget + 1e-9, "frontier point overspent");
            let expect = greedy_design(&blocks, budget, REL_TOL);
            let want: Vec<String> = expect
                .chosen
                .iter()
                .map(|&i| blocks[i].label.clone())
                .collect();
            assert_eq!(
                p.chosen, want,
                "frontier labels disagree with the selection"
            );
        }
    }

    /// A block carrying no information is never worth buying, however cheap. Guards the
    /// `gain <= 0` cut-off in the greedy loop.
    ///
    /// The obvious version of this test is wrong, and was written that way first: a
    /// DUPLICATE of an already-chosen block is not a block that adds nothing. Fisher
    /// information is additive, so buying the same measurement twice halves that
    /// measurement's variance contribution — the optimizer is right to buy it, and the
    /// test asserting otherwise was encoding a misunderstanding rather than a defect. A
    /// genuinely empty block is one whose Jacobian rows are zero.
    #[test]
    fn a_block_that_adds_nothing_is_never_bought() {
        let mut blocks = small_menu();
        blocks.push(block_from_rows(
            "null-observation",
            vec![[0.0_f64; 7]],
            1.0,
            1e-6,
        ));
        let dud_idx = blocks.len() - 1;
        let full: f64 = blocks.iter().map(|b| b.cost).sum();
        let r = greedy_design(&blocks, full, REL_TOL);
        assert!(
            !r.chosen.contains(&dud_idx),
            "bought a block whose Fisher contribution is identically zero: {:?}",
            r.chosen
        );
    }
}
