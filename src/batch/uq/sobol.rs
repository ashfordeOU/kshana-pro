// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Variance-based global sensitivity indices (Sobol' indices).
//!
//! For an output `Y = f(X_1 .. X_k)` with independent inputs:
//!
//! * the **first-order index** `S_i = Var(E[Y | X_i]) / Var(Y)` is the share of the
//!   output variance that would disappear if `X_i` were known exactly;
//! * the **total index** `ST_i = E[Var(Y | X_~i)] / Var(Y)` is the share that remains
//!   while `X_i` stays unknown, its interactions with the other inputs included.
//!
//! From the two base matrices `A` and `B` and the `k` matrices `A_B^(i)` (`A` with
//! column `i` from `B`), with `N` rows each:
//!
//! * first order, the estimator of A. Saltelli, P. Annoni, I. Azzini, F. Campolongo,
//!   M. Ratto and S. Tarantola, "Variance based sensitivity analysis of model output.
//!   Design and estimator for the total sensitivity index", Computer Physics
//!   Communications 181(2), 2010, pp. 259-270 (Table 2, row b):
//!   `V_i = (1/N) sum_j f(B)_j * (f(A_B^(i))_j - f(A)_j)`;
//! * total, the estimator of M. J. W. Jansen, "Analysis of variance designs for model
//!   output", Computer Physics Communications 117(1-2), 1999, pp. 35-43:
//!   `VT_i = (1/2N) sum_j (f(A)_j - f(A_B^(i))_j)^2`.
//!
//! Both are divided by the variance of the `2N` values of `A` and `B` together. The
//! outputs are centred on the mean of those `2N` values first: the first-order
//! estimator is unbiased for any centring constant, and centring removes the part of
//! its sampling error that grows with the square of the output's mean.
//!
//! Estimates are **not clamped**: a first-order estimate of an input with no effect
//! scatters around zero and may be slightly negative. The 95 % intervals are
//! percentile-bootstrap intervals over the `N` rows (a row keeps its `A`, `B` and
//! `A_B^(i)` values together), from a fixed-seed stream.

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

use super::stats::percentile_sorted;

/// Bootstrap resamples behind each interval.
pub const SOBOL_BOOTSTRAP_RESAMPLES: usize = 1000;

/// An output whose standard deviation is below this fraction of its mean magnitude is
/// treated as constant: what is left is rounding noise, not a response.
pub const CONSTANT_OUTPUT_REL_STD: f64 = 1e-12;

/// The indices of one input for one output.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolIndex {
    pub first_order: f64,
    pub first_order_ci95: (f64, f64),
    pub total: f64,
    pub total_ci95: (f64, f64),
}

/// The Sobol' analysis of one output.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolOutcome {
    /// Rows used (rows where every one of the `k + 2` runs gave a value).
    pub n_rows: usize,
    /// Variance of the output over `A` and `B` together (divisor `2N`).
    pub variance: f64,
    /// One entry per input: the indices, or the reason there are none.
    pub indices: Vec<Result<SobolIndex, String>>,
    /// `1 - sum of the first-order indices`, over the inputs that have one; `None`
    /// when no input has an index.
    pub interaction_share: Option<f64>,
    /// Why the whole output has no indices (a constant output, too few rows).
    pub reason: Option<String>,
}

/// Point estimates for one set of rows: `(variance, [first, total] per input)`.
fn point(ya: &[f64], yb: &[f64], yab: &[Vec<f64>], rows: &[usize]) -> (f64, Vec<(f64, f64)>) {
    let n = rows.len() as f64;
    let mut sum = 0.0;
    for &j in rows {
        sum += ya[j] + yb[j];
    }
    let f0 = sum / (2.0 * n);
    let mut ss = 0.0;
    for &j in rows {
        ss += (ya[j] - f0).powi(2) + (yb[j] - f0).powi(2);
    }
    let var = ss / (2.0 * n);
    let est = yab
        .iter()
        .map(|col| {
            let (mut vi, mut vt) = (0.0, 0.0);
            for &j in rows {
                let d = col[j] - ya[j];
                vi += (yb[j] - f0) * d;
                vt += d * d;
            }
            (vi / n / var, vt / (2.0 * n) / var)
        })
        .collect();
    (var, est)
}

/// Whether an output with this mean and variance is constant for our purposes.
pub fn is_constant(mean: f64, variance: f64) -> bool {
    variance.is_nan() || variance <= 0.0 || variance.sqrt() <= CONSTANT_OUTPUT_REL_STD * mean.abs()
}

/// Estimate the indices of every input for one output.
///
/// `ya`, `yb` and each `yab[i]` are the output over the rows of `A`, `B` and
/// `A_B^(i)`; `None` marks a run that failed or gave no value, and its whole row is
/// left out. `input_varies[i]` is false for an input that takes a single value in the
/// sample: it gets no index.
pub fn estimate(
    ya: &[Option<f64>],
    yb: &[Option<f64>],
    yab: &[Vec<Option<f64>>],
    input_varies: &[bool],
    boot_seed: u64,
) -> SobolOutcome {
    let k = yab.len();
    let n = ya.len();
    let rows: Vec<usize> = (0..n)
        .filter(|&j| ya[j].is_some() && yb[j].is_some() && yab.iter().all(|c| c[j].is_some()))
        .collect();
    let none = |reason: String, n_rows: usize, variance: f64| SobolOutcome {
        n_rows,
        variance,
        indices: (0..k).map(|_| Err(reason.clone())).collect(),
        interaction_share: None,
        reason: Some(reason),
    };
    if rows.len() < 2 {
        return none(
            format!(
                "only {} complete row(s) of the design gave a value; at least two are needed",
                rows.len()
            ),
            rows.len(),
            f64::NAN,
        );
    }
    let unwrap = |v: &[Option<f64>]| -> Vec<f64> { v.iter().map(|x| x.unwrap_or(0.0)).collect() };
    let (a, b) = (unwrap(ya), unwrap(yb));
    let ab: Vec<Vec<f64>> = yab.iter().map(|c| unwrap(c)).collect();

    let (var, est) = point(&a, &b, &ab, &rows);
    let mean = rows.iter().map(|&j| a[j] + b[j]).sum::<f64>() / (2.0 * rows.len() as f64);
    if is_constant(mean, var) {
        return none(
            "the output does not vary over the sample (zero variance), so there is no \
             variance to attribute to any input"
                .to_string(),
            rows.len(),
            var,
        );
    }

    // Bootstrap over rows: one index draw per resample, shared by every input.
    let mut rng = ChaCha8Rng::seed_from_u64(boot_seed);
    let m = rows.len();
    let mut boot_first: Vec<Vec<f64>> = vec![Vec::with_capacity(SOBOL_BOOTSTRAP_RESAMPLES); k];
    let mut boot_total: Vec<Vec<f64>> = vec![Vec::with_capacity(SOBOL_BOOTSTRAP_RESAMPLES); k];
    let mut pick = vec![0usize; m];
    for _ in 0..SOBOL_BOOTSTRAP_RESAMPLES {
        for p in pick.iter_mut() {
            *p = rows[(rng.next_u64() % m as u64) as usize];
        }
        let (v, e) = point(&a, &b, &ab, &pick);
        if v.is_nan() || v <= 0.0 {
            continue;
        }
        for (i, (s, t)) in e.into_iter().enumerate() {
            boot_first[i].push(s);
            boot_total[i].push(t);
        }
    }
    let interval = |xs: &mut Vec<f64>, centre: f64| -> (f64, f64) {
        if xs.is_empty() {
            return (centre, centre);
        }
        xs.sort_by(|p, q| p.total_cmp(q));
        (percentile_sorted(xs, 0.025), percentile_sorted(xs, 0.975))
    };

    let mut indices = Vec::with_capacity(k);
    let mut first_sum = 0.0;
    let mut any = false;
    for i in 0..k {
        if !input_varies.get(i).copied().unwrap_or(true) {
            indices.push(Err(
                "the input takes a single value in the sample (zero variance), so it \
                 cannot explain any variance"
                    .to_string(),
            ));
            continue;
        }
        let (s, t) = est[i];
        first_sum += s;
        any = true;
        indices.push(Ok(SobolIndex {
            first_order: s,
            first_order_ci95: interval(&mut boot_first[i], s),
            total: t,
            total_ci95: interval(&mut boot_total[i], t),
        }));
    }
    SobolOutcome {
        n_rows: rows.len(),
        variance: var,
        indices,
        interaction_share: any.then_some(1.0 - first_sum),
        reason: None,
    }
}

/// Point estimates over the first `m` rows, for the convergence record: per input
/// `(first_order, total)`, or `None` where there is no estimate.
pub fn running_point(
    ya: &[Option<f64>],
    yb: &[Option<f64>],
    yab: &[Vec<Option<f64>>],
    m: usize,
) -> Option<Vec<(f64, f64)>> {
    let m = m.min(ya.len());
    let rows: Vec<usize> = (0..m)
        .filter(|&j| ya[j].is_some() && yb[j].is_some() && yab.iter().all(|c| c[j].is_some()))
        .collect();
    if rows.len() < 2 {
        return None;
    }
    let unwrap = |v: &[Option<f64>]| -> Vec<f64> { v.iter().map(|x| x.unwrap_or(0.0)).collect() };
    let (a, b) = (unwrap(ya), unwrap(yb));
    let ab: Vec<Vec<f64>> = yab.iter().map(|c| unwrap(c)).collect();
    let (var, est) = point(&a, &b, &ab, &rows);
    let mean = rows.iter().map(|&j| a[j] + b[j]).sum::<f64>() / (2.0 * rows.len() as f64);
    if is_constant(mean, var) {
        return None;
    }
    Some(est)
}

/// Rank the inputs of one output by total index, largest first (1 = most influential);
/// ties and inputs with no index keep their declared order, the latter unranked.
pub fn rank_by_total(indices: &[Result<SobolIndex, String>]) -> Vec<Option<usize>> {
    let mut order: Vec<usize> = (0..indices.len()).filter(|&i| indices[i].is_ok()).collect();
    let total = |i: usize| indices[i].as_ref().map(|x| x.total).unwrap_or(f64::NAN);
    order.sort_by(|&p, &q| total(q).total_cmp(&total(p)).then(p.cmp(&q)));
    let mut rank = vec![None; indices.len()];
    for (r, i) in order.into_iter().enumerate() {
        rank[i] = Some(r + 1);
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::super::sampling::{saltelli_unit_rows, Method};
    use super::*;

    /// Evaluate `f` over a Saltelli design on the unit cube and estimate the indices.
    fn analyse(n: usize, k: usize, seed: u64, f: impl Fn(&[f64]) -> f64) -> SobolOutcome {
        let rows = saltelli_unit_rows(Method::Lhs, n, k, seed);
        let y: Vec<Option<f64>> = rows.iter().map(|r| Some(f(r))).collect();
        let ya = y[..n].to_vec();
        let yb = y[n..2 * n].to_vec();
        let yab: Vec<Vec<Option<f64>>> = (0..k)
            .map(|i| y[(2 + i) * n..(3 + i) * n].to_vec())
            .collect();
        estimate(&ya, &yb, &yab, &vec![true; k], seed ^ 0x50B0)
    }

    #[test]
    fn additive_function_has_first_order_equal_to_total() {
        // Y = 2 U1 + U2 + 0 U3 with U uniform on (0, 1): the variances are 4/12, 1/12
        // and 0, so S = ST = 4/5, 1/5, 0 and nothing is left to interactions.
        let out = analyse(4096, 3, 3, |u| 100.0 + 2.0 * u[0] + u[1]);
        let want = [0.8, 0.2, 0.0];
        for (i, w) in want.iter().enumerate() {
            let x = out.indices[i].as_ref().expect("an index");
            assert!((x.first_order - w).abs() < 0.03, "S{i} = {}", x.first_order);
            assert!((x.total - w).abs() < 0.03, "ST{i} = {}", x.total);
        }
        // The unused input's indices are exactly zero: its A_B matrix gives the same
        // outputs as A.
        let x3 = out.indices[2].as_ref().expect("an index");
        assert_eq!((x3.first_order, x3.total), (0.0, 0.0));
        assert!(out.interaction_share.expect("a share").abs() < 0.04);
        assert!((out.variance - 5.0 / 12.0).abs() < 0.02);
        assert_eq!(rank_by_total(&out.indices), vec![Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn pure_interaction_has_no_first_order_effect() {
        // Y = (U1 - 1/2)(U2 - 1/2): E[Y | U1] = 0, so S1 = S2 = 0, and each total
        // index is 1 (the whole variance is the interaction).
        let out = analyse(8192, 2, 5, |u| (u[0] - 0.5) * (u[1] - 0.5));
        for i in 0..2 {
            let x = out.indices[i].as_ref().expect("an index");
            assert!(x.first_order.abs() < 0.05, "S{i} = {}", x.first_order);
            assert!((x.total - 1.0).abs() < 0.05, "ST{i} = {}", x.total);
        }
        assert!((out.interaction_share.expect("a share") - 1.0).abs() < 0.08);
    }

    #[test]
    fn a_large_offset_does_not_swamp_the_estimate() {
        // The same additive function on top of an offset a million times its spread.
        let out = analyse(4096, 2, 9, |u| 1.0e6 + 2.0 * u[0] + u[1]);
        let s1 = out.indices[0].as_ref().expect("an index").first_order;
        assert!((s1 - 0.8).abs() < 0.03, "S1 with a large offset = {s1}");
    }

    #[test]
    fn constant_output_and_constant_input_give_no_index_with_a_reason() {
        let out = analyse(64, 2, 1, |_| 3.25);
        assert!(out.indices.iter().all(Result::is_err));
        assert!(out
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("does not vary"));
        assert!(out.interaction_share.is_none());

        let rows = saltelli_unit_rows(Method::Lhs, 64, 2, 1);
        let y: Vec<Option<f64>> = rows.iter().map(|r| Some(r[0])).collect();
        let out = estimate(
            &y[..64],
            &y[64..128],
            &[y[128..192].to_vec(), y[192..256].to_vec()],
            &[true, false],
            1,
        );
        assert!(out.indices[0].is_ok());
        let why = out.indices[1].as_ref().expect_err("no index");
        assert!(why.contains("single value"), "{why}");
    }

    #[test]
    fn a_failed_run_drops_its_whole_row_and_nothing_else() {
        let (n, k) = (256, 2);
        let rows = saltelli_unit_rows(Method::Lhs, n, k, 4);
        let mut y: Vec<Option<f64>> = rows.iter().map(|r| Some(2.0 * r[0] + r[1])).collect();
        y[5] = None; // A row 5
        y[n + 9] = None; // B row 9
        y[2 * n + 5] = None; // A_B^(0) row 5 (same row as the first)
        let out = estimate(
            &y[..n],
            &y[n..2 * n],
            &[y[2 * n..3 * n].to_vec(), y[3 * n..4 * n].to_vec()],
            &[true, true],
            4,
        );
        assert_eq!(out.n_rows, n - 2);
        assert!(out.indices[0].is_ok());
    }
}
