// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Percentile bands of a time series over an ensemble: at every time step, the 5th,
//! 50th and 95th percentile (nearest rank, the same rule as the scalar statistics) of
//! the realisations that gave a value at that step.

use super::stats::{percentile_sorted, LimitOp};

/// The band of one series output.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// Realisations that contributed a series.
    pub n: usize,
    /// Per time step: how many realisations gave a number there.
    pub n_at_step: Vec<usize>,
    pub p05: Vec<Option<f64>>,
    pub p50: Vec<Option<f64>>,
    pub p95: Vec<Option<f64>>,
    /// Per time step: the share of contributing realisations that meet the limit
    /// (empty when the output has no limit).
    pub share_meeting_limit: Vec<Option<f64>>,
}

/// Build the band over `series` (one slice per realisation, all of length `steps`; a
/// not-a-number entry is a missing value at that step).
pub fn band(series: &[&[f64]], steps: usize, limit: Option<(LimitOp, f64)>) -> Band {
    let mut out = Band {
        n: series.len(),
        n_at_step: Vec::with_capacity(steps),
        p05: Vec::with_capacity(steps),
        p50: Vec::with_capacity(steps),
        p95: Vec::with_capacity(steps),
        share_meeting_limit: Vec::new(),
    };
    let mut column: Vec<f64> = Vec::with_capacity(series.len());
    for t in 0..steps {
        column.clear();
        column.extend(series.iter().map(|s| s[t]).filter(|v| !v.is_nan()));
        out.n_at_step.push(column.len());
        if column.is_empty() {
            out.p05.push(None);
            out.p50.push(None);
            out.p95.push(None);
            if limit.is_some() {
                out.share_meeting_limit.push(None);
            }
            continue;
        }
        if let Some((op, value)) = limit {
            let met = column.iter().filter(|v| op.met(**v, value)).count();
            out.share_meeting_limit
                .push(Some(met as f64 / column.len() as f64));
        }
        column.sort_by(|a, b| a.total_cmp(b));
        out.p05.push(Some(percentile_sorted(&column, 0.05)));
        out.p50.push(Some(percentile_sorted(&column, 0.50)));
        out.p95.push(Some(percentile_sorted(&column, 0.95)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_step_percentiles_of_a_hand_checked_ensemble() {
        // Ten realisations; at step t the values are (r + 1) * (t + 1) for r = 0..10.
        let data: Vec<Vec<f64>> = (0..10)
            .map(|r| (0..3).map(|t| f64::from((r + 1) * (t + 1))).collect())
            .collect();
        let refs: Vec<&[f64]> = data.iter().map(Vec::as_slice).collect();
        let b = band(&refs, 3, Some((LimitOp::Le, 12.0)));
        // Nearest rank on 1..=10 scaled by (t + 1): 1st, 5th and 10th values.
        assert_eq!(b.p05, vec![Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(b.p50, vec![Some(5.0), Some(10.0), Some(15.0)]);
        assert_eq!(b.p95, vec![Some(10.0), Some(20.0), Some(30.0)]);
        // Values <= 12: all ten at t = 0, six (2..=12) at t = 1, four (3..=12) at t = 2.
        assert_eq!(b.share_meeting_limit, vec![Some(1.0), Some(0.6), Some(0.4)]);
        assert_eq!(b.n_at_step, vec![10, 10, 10]);
    }

    #[test]
    fn a_missing_value_is_left_out_of_its_step_only() {
        let a = [1.0, f64::NAN];
        let b = [3.0, f64::NAN];
        let c = [2.0, 7.0];
        let out = band(&[&a, &b, &c], 2, None);
        assert_eq!(out.n_at_step, vec![3, 1]);
        assert_eq!(out.p50, vec![Some(2.0), Some(7.0)]);
        assert!(out.share_meeting_limit.is_empty());
        let empty = band(&[&[f64::NAN][..]], 1, Some((LimitOp::Lt, 1.0)));
        assert_eq!(empty.p50, vec![None]);
        assert_eq!(empty.share_meeting_limit, vec![None]);
    }
}
