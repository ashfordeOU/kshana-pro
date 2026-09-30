// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Summary statistics of one output over an ensemble.
//!
//! The mean, the standard deviation, the nearest-rank percentiles and the bootstrap
//! interval are computed **exactly as the open engine's `campaign [monte_carlo]`
//! computes them** (same summation order, same divisor, same resampling stream), so a
//! study that varies only the engine seed reproduces the open campaign's numbers bit
//! for bit. `tests/uncertainty.rs` holds that equality against a real campaign run.
//!
//! * mean: the sum in realisation order, divided by `n`;
//! * `std`: the root of the mean squared deviation, divisor `n` (the open campaign's
//!   definition); `std_sample` is the same with divisor `n - 1`;
//! * percentiles: nearest rank, `rank = ceil(p * n)` on the sorted sample;
//! * interval on the mean: percentile bootstrap, 2000 resamples, fixed seed.
//!
//! The probability of meeting a limit carries a Wilson score interval (E. B. Wilson,
//! "Probable Inference, the Law of Succession, and Statistical Inference", Journal of
//! the American Statistical Association 22(158), 1927, pp. 209-212).

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Resamples of the bootstrap interval on the mean (the open campaign's count).
pub const BOOTSTRAP_RESAMPLES: usize = 2000;

/// The two-sided 95 % point of the standard normal distribution.
pub const Z95: f64 = 1.959_963_984_540_054;

/// Mean of `values` (0 for an empty slice).
pub fn mean(values: &[f64]) -> f64 {
    let n = values.len().max(1) as f64;
    values.iter().sum::<f64>() / n
}

/// Variance with divisor `n` about the mean.
pub fn variance_pop(values: &[f64]) -> f64 {
    let n = values.len().max(1) as f64;
    let m = mean(values);
    values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n
}

/// Nearest-rank percentile of an ascending slice, `p` in `[0, 1]`.
pub fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let rank = (p * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
}

/// An ascending copy of `values`.
pub fn sorted(values: &[f64]) -> Vec<f64> {
    let mut s = values.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s
}

/// Percentile-bootstrap 95 % interval on the mean: `resamples` resamples of size `n`
/// drawn with replacement from a fixed-seed stream.
pub fn bootstrap_ci95_mean(values: &[f64], resamples: usize, seed: u64) -> (f64, f64) {
    if values.len() < 2 {
        let m = values.first().copied().unwrap_or(f64::NAN);
        return (m, m);
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let n = values.len();
    let mut means: Vec<f64> = Vec::with_capacity(resamples);
    for _ in 0..resamples {
        let mut s = 0.0;
        for _ in 0..n {
            let j = (rng.next_u64() % n as u64) as usize;
            s += values[j];
        }
        means.push(s / n as f64);
    }
    means.sort_by(|a, c| a.total_cmp(c));
    (
        percentile_sorted(&means, 0.025),
        percentile_sorted(&means, 0.975),
    )
}

/// The summary of one output over the realisations that produced a value.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Realisations with a value.
    pub n: usize,
    pub mean: f64,
    /// Divisor `n` (the open campaign's definition).
    pub std: f64,
    /// Divisor `n - 1`.
    pub std_sample: f64,
    pub min: f64,
    pub max: f64,
    pub p05: f64,
    pub p50: f64,
    pub p95: f64,
    pub ci95_low: f64,
    pub ci95_high: f64,
}

/// Summarise `values` (which must not be empty); `boot_seed` fixes the bootstrap.
pub fn summarise(values: &[f64], boot_seed: u64) -> Option<Summary> {
    if values.is_empty() {
        return None;
    }
    let n = values.len();
    let m = mean(values);
    let var = variance_pop(values);
    let s = sorted(values);
    let (lo, hi) = bootstrap_ci95_mean(values, BOOTSTRAP_RESAMPLES, boot_seed);
    Some(Summary {
        n,
        mean: m,
        std: var.sqrt(),
        std_sample: if n > 1 {
            (var * n as f64 / (n as f64 - 1.0)).sqrt()
        } else {
            0.0
        },
        min: s[0],
        max: s[n - 1],
        p05: percentile_sorted(&s, 0.05),
        p50: percentile_sorted(&s, 0.50),
        p95: percentile_sorted(&s, 0.95),
        ci95_low: lo,
        ci95_high: hi,
    })
}

/// One point of a running estimate: the statistics over the first `n` values.
#[derive(Debug, Clone, PartialEq)]
pub struct RunningPoint {
    pub n: usize,
    pub mean: f64,
    pub std: f64,
    pub p05: f64,
    pub p50: f64,
    pub p95: f64,
}

/// The sizes a running estimate is reported at: 16, 32, 64, ... and the full size.
pub fn convergence_sizes(n: usize) -> Vec<usize> {
    let mut sizes = Vec::new();
    let mut m = 16usize;
    while m < n {
        sizes.push(m);
        m *= 2;
    }
    if n > 0 {
        sizes.push(n);
    }
    sizes
}

/// The running estimate of the mean, spread and percentiles as realisations accumulate
/// in order.
pub fn running(values: &[f64]) -> Vec<RunningPoint> {
    convergence_sizes(values.len())
        .into_iter()
        .map(|m| {
            let head = &values[..m];
            let s = sorted(head);
            RunningPoint {
                n: m,
                mean: mean(head),
                std: variance_pop(head).sqrt(),
                p05: percentile_sorted(&s, 0.05),
                p50: percentile_sorted(&s, 0.50),
                p95: percentile_sorted(&s, 0.95),
            }
        })
        .collect()
}

/// Wilson score 95 % interval for `successes` out of `n` trials.
pub fn wilson_ci95(successes: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (f64::NAN, f64::NAN);
    }
    let nf = n as f64;
    let p = successes as f64 / nf;
    let z2 = Z95 * Z95;
    let denom = 1.0 + z2 / nf;
    let centre = (p + z2 / (2.0 * nf)) / denom;
    let half = Z95 * (p * (1.0 - p) / nf + z2 / (4.0 * nf * nf)).sqrt() / denom;
    // At 0 of n the lower end is exactly 0 and at n of n the upper end exactly 1; the
    // rounding of `centre - half` would otherwise leave a stray 1e-17.
    let lo = if successes == 0 {
        0.0
    } else {
        (centre - half).max(0.0)
    };
    let hi = if successes == n {
        1.0
    } else {
        (centre + half).min(1.0)
    };
    (lo, hi)
}

/// How an output value is compared with its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitOp {
    Lt,
    Le,
    Gt,
    Ge,
    /// The magnitude is below the limit: `|value| < limit`.
    AbsLt,
    /// The magnitude is at or below the limit: `|value| <= limit`.
    AbsLe,
}

impl LimitOp {
    /// Parse the study file's spelling.
    pub fn parse(s: &str) -> Result<LimitOp, String> {
        Ok(match s {
            "lt" | "<" => LimitOp::Lt,
            "le" | "<=" => LimitOp::Le,
            "gt" | ">" => LimitOp::Gt,
            "ge" | ">=" => LimitOp::Ge,
            "abs_lt" => LimitOp::AbsLt,
            "abs_le" => LimitOp::AbsLe,
            other => {
                return Err(format!(
                    "limit `op` must be lt, le, gt, ge, abs_lt or abs_le (got \"{other}\")"
                ))
            }
        })
    }

    /// The canonical spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            LimitOp::Lt => "lt",
            LimitOp::Le => "le",
            LimitOp::Gt => "gt",
            LimitOp::Ge => "ge",
            LimitOp::AbsLt => "abs_lt",
            LimitOp::AbsLe => "abs_le",
        }
    }

    /// The comparison as a reader writes it, for example `>= 91`.
    pub fn phrase(self, value: f64, unit: &str) -> String {
        let u = if unit.is_empty() || unit == "1" {
            String::new()
        } else {
            format!(" {unit}")
        };
        match self {
            LimitOp::Lt => format!("< {value}{u}"),
            LimitOp::Le => format!("<= {value}{u}"),
            LimitOp::Gt => format!("> {value}{u}"),
            LimitOp::Ge => format!(">= {value}{u}"),
            LimitOp::AbsLt => format!("magnitude < {value}{u}"),
            LimitOp::AbsLe => format!("magnitude <= {value}{u}"),
        }
    }

    /// Whether `value` meets the limit. A value that is not a number never does.
    pub fn met(self, value: f64, limit: f64) -> bool {
        if value.is_nan() {
            return false;
        }
        match self {
            LimitOp::Lt => value < limit,
            LimitOp::Le => value <= limit,
            LimitOp::Gt => value > limit,
            LimitOp::Ge => value >= limit,
            LimitOp::AbsLt => value.abs() < limit,
            LimitOp::AbsLe => value.abs() <= limit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_of_a_hand_checked_sample() {
        // 1..=10: mean 5.5; sum of squared deviations 82.5, so the divisor-n standard
        // deviation is sqrt(8.25) and the divisor-(n-1) one is sqrt(82.5 / 9).
        let v: Vec<f64> = (1..=10).map(f64::from).collect();
        let s = summarise(&v, 1).expect("not empty");
        assert_eq!(s.n, 10);
        assert_eq!(s.mean, 5.5);
        assert!((s.std - 8.25f64.sqrt()).abs() < 1e-12);
        assert!((s.std_sample - (82.5f64 / 9.0).sqrt()).abs() < 1e-12);
        // Nearest rank: ceil(0.05*10) = 1st, ceil(0.5*10) = 5th, ceil(0.95*10) = 10th.
        assert_eq!((s.p05, s.p50, s.p95), (1.0, 5.0, 10.0));
        assert_eq!((s.min, s.max), (1.0, 10.0));
        assert!(s.ci95_low < s.mean && s.mean < s.ci95_high);
        assert!(summarise(&[], 1).is_none());
    }

    #[test]
    fn bootstrap_interval_is_reproducible_and_has_the_right_width() {
        // 400 evenly spread values with standard deviation s: the interval on the mean
        // is close to mean +/- 1.96 * s / sqrt(n).
        let v: Vec<f64> = (0..400).map(|i| (i as f64 * 0.618_034).fract()).collect();
        let (lo, hi) = bootstrap_ci95_mean(&v, BOOTSTRAP_RESAMPLES, 7);
        assert_eq!((lo, hi), bootstrap_ci95_mean(&v, BOOTSTRAP_RESAMPLES, 7));
        assert_ne!((lo, hi), bootstrap_ci95_mean(&v, BOOTSTRAP_RESAMPLES, 8));
        let expect = 2.0 * Z95 * variance_pop(&v).sqrt() / 20.0;
        let width = hi - lo;
        assert!(
            (width - expect).abs() < 0.12 * expect,
            "bootstrap width {width} vs normal-theory {expect}"
        );
    }

    #[test]
    fn wilson_interval_matches_the_worked_example() {
        // 81 successes in 263 trials, the example of R. G. Newcombe, "Two-sided
        // confidence intervals for the single proportion", Statistics in Medicine 17,
        // 1998, Table I: Wilson score interval 0.2553 to 0.3662.
        let (lo, hi) = wilson_ci95(81, 263);
        assert!((lo - 0.2553).abs() < 5e-5, "lower {lo}");
        assert!((hi - 0.3662).abs() < 5e-5, "upper {hi}");
        // The interval never leaves [0, 1] and is not degenerate at the ends.
        let (lo0, hi0) = wilson_ci95(0, 50);
        assert_eq!(lo0, 0.0);
        assert!(hi0 > 0.0 && hi0 < 0.1);
        let (lo1, hi1) = wilson_ci95(50, 50);
        assert_eq!(hi1, 1.0);
        assert!(lo1 > 0.9 && lo1 < 1.0);
    }

    #[test]
    fn running_estimates_are_reported_at_powers_of_two_and_the_full_size() {
        assert_eq!(convergence_sizes(100), vec![16, 32, 64, 100]);
        assert_eq!(convergence_sizes(64), vec![16, 32, 64]);
        assert_eq!(convergence_sizes(5), vec![5]);
        let v: Vec<f64> = (0..100).map(f64::from).collect();
        let r = running(&v);
        assert_eq!(r.len(), 4);
        assert_eq!(r[0].mean, 7.5);
        assert_eq!(r[3].mean, 49.5);
    }

    #[test]
    fn limits_compare_as_written_and_never_pass_a_missing_value() {
        assert!(LimitOp::Ge.met(91.0, 91.0));
        assert!(!LimitOp::Gt.met(91.0, 91.0));
        assert!(LimitOp::AbsLe.met(-20.0, 20.0));
        assert!(!LimitOp::AbsLt.met(-20.0, 20.0));
        for op in [
            LimitOp::Lt,
            LimitOp::Le,
            LimitOp::Gt,
            LimitOp::Ge,
            LimitOp::AbsLt,
            LimitOp::AbsLe,
        ] {
            assert!(!op.met(f64::NAN, 1.0));
            assert_eq!(LimitOp::parse(op.as_str()), Ok(op));
        }
        assert!(LimitOp::parse("about").is_err());
    }
}
