// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Standardised regression coefficients (SRC): a cheap sensitivity ranking from the
//! plain sample, with no extra runs.
//!
//! The output is regressed on the inputs by ordinary least squares,
//! `y = b_0 + sum_i b_i x_i + residual`. The standardised coefficient of input `i` is
//! `b_i * std(x_i) / std(y)`: the change of the output, in its own standard
//! deviations, for one standard deviation of the input. For independent inputs and an
//! exactly linear response the squared coefficients sum to one and each equals the
//! input's first-order Sobol' index.
//!
//! The coefficient of determination `R^2` says how much of the output variance the
//! linear fit explains. When it is low the response is not linear in the inputs and
//! the ranking is **not reliable**; the result says so in words, and the Sobol' method
//! is the one to use. (A. Saltelli and others, "Global Sensitivity Analysis: The
//! Primer", Wiley, 2008, section 1.2.)

/// Below this `R^2` the linear ranking is reported as not reliable.
pub const R2_RELIABLE: f64 = 0.7;

/// The regression result for one input.
#[derive(Debug, Clone, PartialEq)]
pub struct SrcEntry {
    /// The regression slope: output units per input unit.
    pub coefficient: f64,
    /// The standardised coefficient.
    pub src: f64,
}

/// The regression of one output on every input.
#[derive(Debug, Clone, PartialEq)]
pub struct SrcOutcome {
    /// Realisations used.
    pub n: usize,
    /// One entry per input, or the reason there is none.
    pub entries: Vec<Result<SrcEntry, String>>,
    /// `R^2` of the fit; `None` when nothing was fitted.
    pub r_squared: Option<f64>,
    /// Sum of the squared standardised coefficients.
    pub sum_src_squared: Option<f64>,
    /// Whether `R^2` is high enough for the ranking to be trusted.
    pub reliable: bool,
    /// A plain statement for the reader (always set when the fit is not reliable).
    pub statement: String,
    /// Why the whole output has no coefficients.
    pub reason: Option<String>,
}

/// Solve `m x = rhs` by Gaussian elimination with partial pivoting; `None` when the
/// matrix is singular to working precision.
fn solve(mut m: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let n = rhs.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&p, &q| m[p][col].abs().total_cmp(&m[q][col].abs()))?;
        if m[pivot][col].abs() < 1e-10 {
            return None;
        }
        m.swap(col, pivot);
        rhs.swap(col, pivot);
        let (upper, lower) = m.split_at_mut(col + 1);
        let pivot_row = &upper[col];
        for (offset, row) in lower.iter_mut().enumerate() {
            let f = row[col] / pivot_row[col];
            if f == 0.0 {
                continue;
            }
            for (cell, p) in row.iter_mut().zip(pivot_row).skip(col) {
                *cell -= f * p;
            }
            rhs[col + 1 + offset] -= f * rhs[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut s = rhs[row];
        for c in row + 1..n {
            s -= m[row][c] * x[c];
        }
        x[row] = s / m[row][row];
    }
    Some(x)
}

/// Regress `y` on the columns of `x` (rows are realisations).
pub fn regress(x: &[Vec<f64>], y: &[f64]) -> SrcOutcome {
    let n = y.len();
    let k = x.first().map(Vec::len).unwrap_or(0);
    let none = |reason: String| SrcOutcome {
        n,
        entries: (0..k).map(|_| Err(reason.clone())).collect(),
        r_squared: None,
        sum_src_squared: None,
        reliable: false,
        statement: reason.clone(),
        reason: Some(reason),
    };
    if k == 0 {
        return none("the study has no uncertain input".to_string());
    }
    let mean_y = y.iter().sum::<f64>() / n.max(1) as f64;
    let var_y = y.iter().map(|v| (v - mean_y).powi(2)).sum::<f64>() / n.max(1) as f64;
    if super::sobol::is_constant(mean_y, var_y) {
        return none(
            "the output does not vary over the sample (zero variance), so there is no \
             variance to attribute to any input"
                .to_string(),
        );
    }
    let sd_y = var_y.sqrt();

    // Standardise every input that varies; the rest get no coefficient.
    let mut means = vec![0.0; k];
    let mut sds = vec![0.0; k];
    for i in 0..k {
        means[i] = x.iter().map(|r| r[i]).sum::<f64>() / n as f64;
        sds[i] = (x.iter().map(|r| (r[i] - means[i]).powi(2)).sum::<f64>() / n as f64).sqrt();
    }
    let active: Vec<usize> = (0..k).filter(|&i| sds[i] > 0.0).collect();
    if active.is_empty() {
        return none("no input varies in the sample".to_string());
    }
    if n <= active.len() + 1 {
        return none(format!(
            "{n} realisations cannot fit {} coefficients and an intercept",
            active.len()
        ));
    }
    let p = active.len();
    let mut gram = vec![vec![0.0; p]; p];
    let mut rhs = vec![0.0; p];
    for (row, yv) in x.iter().zip(y) {
        let z: Vec<f64> = active
            .iter()
            .map(|&i| (row[i] - means[i]) / sds[i])
            .collect();
        let yz = (yv - mean_y) / sd_y;
        for a in 0..p {
            rhs[a] += z[a] * yz;
            for b in 0..p {
                gram[a][b] += z[a] * z[b];
            }
        }
    }
    let Some(beta) = solve(gram, rhs) else {
        return none(
            "the inputs are collinear in the sample, so their separate effects cannot be \
             told apart"
                .to_string(),
        );
    };

    // R^2 from the residuals of the standardised fit.
    let mut sse = 0.0;
    for (row, yv) in x.iter().zip(y) {
        let fit: f64 = active
            .iter()
            .zip(&beta)
            .map(|(&i, b)| b * (row[i] - means[i]) / sds[i])
            .sum();
        sse += ((yv - mean_y) / sd_y - fit).powi(2);
    }
    let r2 = 1.0 - sse / n as f64;

    let mut entries: Vec<Result<SrcEntry, String>> = (0..k)
        .map(|_| {
            Err(
                "the input takes a single value in the sample (zero variance), so it \
                 cannot explain any variance"
                    .to_string(),
            )
        })
        .collect();
    for (&i, b) in active.iter().zip(&beta) {
        entries[i] = Ok(SrcEntry {
            coefficient: b * sd_y / sds[i],
            src: *b,
        });
    }
    let reliable = r2 >= R2_RELIABLE;
    let statement = if reliable {
        format!(
            "The linear fit explains {:.1} % of the output variance (R-squared {:.4}).",
            100.0 * r2,
            r2
        )
    } else {
        format!(
            "The linear fit explains only {:.1} % of the output variance (R-squared {:.4}, \
             below {R2_RELIABLE}): the response is not linear in the inputs and this ranking \
             is NOT reliable. Use sensitivity method \"sobol\" for this output.",
            100.0 * r2,
            r2
        )
    };
    SrcOutcome {
        n,
        entries,
        r_squared: Some(r2),
        sum_src_squared: Some(beta.iter().map(|b| b * b).sum()),
        reliable,
        statement,
        reason: None,
    }
}

/// Rank the inputs by the magnitude of their standardised coefficient, largest first.
pub fn rank_by_magnitude(entries: &[Result<SrcEntry, String>]) -> Vec<Option<usize>> {
    let mut order: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].is_ok()).collect();
    let mag = |i: usize| entries[i].as_ref().map(|e| e.src.abs()).unwrap_or(f64::NAN);
    order.sort_by(|&p, &q| mag(q).total_cmp(&mag(p)).then(p.cmp(&q)));
    let mut rank = vec![None; entries.len()];
    for (r, i) in order.into_iter().enumerate() {
        rank[i] = Some(r + 1);
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small, exactly orthogonal two-level design: the four corners of a square.
    fn corners() -> Vec<Vec<f64>> {
        vec![
            vec![-1.0, -1.0],
            vec![-1.0, 1.0],
            vec![1.0, -1.0],
            vec![1.0, 1.0],
        ]
    }

    #[test]
    fn an_exactly_linear_response_is_recovered_exactly() {
        // y = 10 + 3 x1 - 4 x2 on an orthogonal design with unit input variances:
        // Var(y) = 9 + 16 = 25, so the standardised coefficients are 3/5 and -4/5.
        let x = corners();
        let y: Vec<f64> = x.iter().map(|r| 10.0 + 3.0 * r[0] - 4.0 * r[1]).collect();
        let out = regress(&x, &y);
        let e0 = out.entries[0].as_ref().expect("fitted");
        let e1 = out.entries[1].as_ref().expect("fitted");
        assert!((e0.coefficient - 3.0).abs() < 1e-12);
        assert!((e1.coefficient + 4.0).abs() < 1e-12);
        assert!((e0.src - 0.6).abs() < 1e-12);
        assert!((e1.src + 0.8).abs() < 1e-12);
        assert!((out.r_squared.expect("fitted") - 1.0).abs() < 1e-12);
        assert!((out.sum_src_squared.expect("fitted") - 1.0).abs() < 1e-12);
        assert!(out.reliable);
        assert_eq!(rank_by_magnitude(&out.entries), vec![Some(2), Some(1)]);
    }

    #[test]
    fn a_non_linear_response_is_flagged_as_not_reliable() {
        // y = x1 * x2 on the same design: no linear trend at all, R-squared 0.
        let x = corners();
        let y: Vec<f64> = x.iter().map(|r| r[0] * r[1]).collect();
        let out = regress(&x, &y);
        assert!(out.r_squared.expect("fitted").abs() < 1e-12);
        assert!(!out.reliable);
        assert!(out.statement.contains("NOT reliable"), "{}", out.statement);
    }

    #[test]
    fn constant_output_and_constant_input_give_no_coefficient_with_a_reason() {
        let x = corners();
        let out = regress(&x, &[2.0, 2.0, 2.0, 2.0]);
        assert!(out.entries.iter().all(Result::is_err));
        assert!(out
            .reason
            .as_deref()
            .unwrap_or("")
            .contains("does not vary"));

        let x: Vec<Vec<f64>> = (0..8).map(|i| vec![f64::from(i), 5.0]).collect();
        let y: Vec<f64> = x.iter().map(|r| 2.0 * r[0]).collect();
        let out = regress(&x, &y);
        assert!((out.entries[0].as_ref().expect("fitted").src - 1.0).abs() < 1e-12);
        let why = out.entries[1].as_ref().expect_err("no coefficient");
        assert!(why.contains("single value"), "{why}");
        assert_eq!(rank_by_magnitude(&out.entries), vec![Some(1), None]);
    }

    #[test]
    fn collinear_inputs_are_refused_rather_than_guessed() {
        let x: Vec<Vec<f64>> = (0..8)
            .map(|i| vec![f64::from(i), 2.0 * f64::from(i)])
            .collect();
        let y: Vec<f64> = x.iter().map(|r| r[0] + r[1]).collect();
        let out = regress(&x, &y);
        assert!(out.reason.as_deref().unwrap_or("").contains("collinear"));
    }
}
