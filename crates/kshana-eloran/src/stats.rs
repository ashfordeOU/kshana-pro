// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Small statistics shared by the guards, the R-Mode performance tests and the claim checks:
//! normal and χ² quantiles, binomial confidence bounds (Wilson and order-statistic bounds on a
//! quantile), the Poisson rate upper bound and the sample size that demonstrates a probability
//! with zero failures. Evidence: checked against tabulated values in the tests below.

/// Standard normal quantile Φ⁻¹(p) (Acklam's rational approximation, |ε| < 1.2e-9).
pub fn normal_quantile(p: f64) -> f64 {
    if !(p > 0.0 && p < 1.0) {
        return if p <= 0.0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    const A: [f64; 6] = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.38357751867269e2,
        -3.066479806614716e1,
        2.506628277459239e0,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838e0,
        -2.549732539343734e0,
        4.374664141464968e0,
        2.938163982698783e0,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996e0,
        3.754408661907416e0,
    ];
    let pl = 0.02425;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -normal_quantile(1.0 - p)
    }
}

/// χ² quantile with `k` degrees of freedom at probability `p`: exact for k = 1, 2;
/// Wilson–Hilferty above (slightly conservative in the far tail for small k).
pub fn chi2_quantile(p: f64, k: usize) -> f64 {
    match k {
        0 => 0.0,
        1 => {
            let z = normal_quantile(0.5 + p / 2.0);
            z * z
        }
        2 => -2.0 * (1.0 - p).ln(),
        _ => {
            let k = k as f64;
            let z = normal_quantile(p);
            let a = 2.0 / (9.0 * k);
            k * (1.0 - a + z * a.sqrt()).powi(3)
        }
    }
}

/// Wilson score interval for `k` successes in `n` trials at two-sided confidence `conf`.
/// `None` when `n == 0`.
pub fn wilson(k: usize, n: usize, conf: f64) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let z = normal_quantile(0.5 + conf / 2.0);
    let n = n as f64;
    let p = k as f64 / n;
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    Some(((c - h).max(0.0), (c + h).min(1.0)))
}

/// One-sided Wilson bounds at confidence `conf` each (the two-sided interval at `2·conf − 1`).
pub fn wilson_one_sided(k: usize, n: usize, conf: f64) -> Option<(f64, f64)> {
    wilson(k, n, 2.0 * conf - 1.0)
}

/// The whole binomial CDF `[P(X ≤ 0), …, P(X ≤ n)]` for X ~ Bin(n, p), in log space
/// (stable for large n).
pub fn binomial_cdf_all(n: usize, p: f64) -> Vec<f64> {
    if p <= 0.0 {
        return vec![1.0; n + 1];
    }
    if p >= 1.0 {
        let mut v = vec![0.0; n + 1];
        v[n] = 1.0;
        return v;
    }
    let (lp, lq) = (p.ln(), (1.0 - p).ln());
    let mut l = vec![0.0f64; n + 1];
    l[0] = n as f64 * lq;
    for i in 0..n {
        l[i + 1] = l[i] + ((n - i) as f64).ln() - ((i + 1) as f64).ln() + lp - lq;
    }
    let m = l.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let w: Vec<f64> = l.iter().map(|x| (x - m).exp()).collect();
    let tot: f64 = w.iter().sum();
    let mut acc = 0.0;
    w.iter()
        .map(|x| {
            acc += x;
            (acc / tot).clamp(0.0, 1.0)
        })
        .collect()
}

/// Binomial CDF P(X ≤ k) for X ~ Bin(n, p).
pub fn binomial_cdf(k: usize, n: usize, p: f64) -> f64 {
    if k >= n {
        return 1.0;
    }
    binomial_cdf_all(n, p)[k]
}

/// Distribution-free confidence bounds on the `q` quantile from `n` sorted samples:
/// `(lower_index, upper_index)` (0-based into the sorted samples), each at one-sided
/// confidence `conf`. The upper bound is the smallest order statistic `x_(k)` with
/// `P(Bin(n, q) ≤ k − 1) ≥ conf`; `None` when even the largest sample is not enough.
pub fn quantile_bounds(n: usize, q: f64, conf: f64) -> (Option<usize>, Option<usize>) {
    if n == 0 {
        return (None, None);
    }
    let cdf = binomial_cdf_all(n, q);
    // Upper: smallest k (1-based) with cdf(k-1) >= conf.
    let upper = (1..=n).find(|&k| cdf[k - 1] >= conf).map(|k| k - 1);
    // Lower: largest j (1-based) with P(X >= j) = 1 - cdf(j-1) >= conf.
    let lower = (1..=n)
        .rev()
        .find(|&j| 1.0 - cdf[j - 1] >= conf)
        .map(|j| j - 1);
    (lower, upper)
}

/// One-sided upper confidence bound on a Poisson rate from `k` events in exposure `t`:
/// `χ²(conf, 2k + 2) / (2t)`.
pub fn poisson_rate_upper(k: usize, t: f64, conf: f64) -> f64 {
    if t <= 0.0 {
        return f64::INFINITY;
    }
    chi2_quantile(conf, 2 * k + 2) / (2.0 * t)
}

/// Number of independent trials with zero failures that demonstrates a success probability
/// of at least `p` at confidence `conf`: `⌈ln(1 − conf) / ln p⌉`.
pub fn zero_failure_trials(p: f64, conf: f64) -> Option<usize> {
    if !(p > 0.0 && p < 1.0 && conf > 0.0 && conf < 1.0) {
        return None;
    }
    Some(((1.0 - conf).ln() / p.ln()).ceil() as usize)
}

/// Mean and sample standard deviation.
pub fn mean_std(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    if x.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let m = x.iter().sum::<f64>() / n;
    if x.len() < 2 {
        return (m, 0.0);
    }
    let v = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, v.sqrt())
}

/// Linear-interpolated empirical quantile of sorted samples.
pub fn quantile_sorted(s: &[f64], q: f64) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let h = (s.len() - 1) as f64 * q.clamp(0.0, 1.0);
    let i = h.floor() as usize;
    let j = (i + 1).min(s.len() - 1);
    Some(s[i] + (h - i as f64) * (s[j] - s[i]))
}

#[cfg(test)]
mod stats_tests {
    use super::*;

    #[test]
    fn wilson_matches_a_textbook_value() {
        // 8 of 10 at 95 %: Wilson (0.490, 0.943).
        let (l, u) = wilson(8, 10, 0.95).unwrap();
        assert!(
            (l - 0.4902).abs() < 1e-3 && (u - 0.9433).abs() < 1e-3,
            "{l} {u}"
        );
    }

    #[test]
    fn binomial_cdf_and_quantile_bounds() {
        assert!((binomial_cdf(0, 10, 0.5) - 1.0 / 1024.0).abs() < 1e-12);
        assert!((binomial_cdf(5, 10, 0.5) - 638.0 / 1024.0).abs() < 1e-12);
        // 59 samples: the maximum bounds the 95th percentile at 95 % (0.95^59 < 0.05).
        let (_, u) = quantile_bounds(59, 0.95, 0.95);
        assert_eq!(u, Some(58));
        let (_, u) = quantile_bounds(58, 0.95, 0.95);
        assert_eq!(u, None);
        let (l, u) = quantile_bounds(1000, 0.95, 0.95);
        assert!(l.unwrap() < 950 && u.unwrap() > 949, "{l:?} {u:?}");
    }

    #[test]
    fn zero_failure_and_poisson_bounds() {
        assert_eq!(zero_failure_trials(0.95, 0.95), Some(59));
        assert_eq!(zero_failure_trials(0.998, 0.95), Some(1497));
        // Zero events: upper rate = −ln(0.05)/t ≈ 2.996/t.
        assert!((poisson_rate_upper(0, 1.0, 0.95) - 2.9957).abs() < 1e-3);
        // Two events, 95 %: 6.296 (table).
        assert!((poisson_rate_upper(2, 1.0, 0.95) - 6.296).abs() < 0.05);
    }
}
