// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Statistical gate** — tell noise from change for a Monte Carlo figure using the
//! ensemble's own statistics.
//!
//! A relative tolerance on a Monte Carlo mean is the wrong test: the mean of 200
//! realisations of a zero-mean process moves by a large *relative* amount from one set
//! of seeds to the next while nothing changed. This gate compares two ensembles as
//! samples instead.
//!
//! **The decision** (fixed before any run; the alpha is stated in the manifest, default
//! 0.01):
//!
//! * **Mean.** A two-sample Welch t-test (unequal variances), two-sided, on the two
//!   means. A regression is declared only when `p < alpha` **and** the current mean is
//!   worse in the watch's direction. A significant change in the better direction is an
//!   improvement.
//! * **Spread.** A one-sided F-test on the ratio of the current variance to the baseline
//!   variance. A regression is declared when `p < alpha`, that is when the spread grew
//!   beyond what two samples of one distribution give at that level.
//!
//! The engine reports the population standard deviation; the tests use the sample
//! variance `std^2 * n / (n - 1)`. Both p-values are reported. The distributions are
//! computed with the regularised incomplete beta function (continued fraction,
//! Numerical Recipes 3rd edition section 6.4), checked in the unit tests against
//! reference values.
//!
//! Alpha is the probability of calling a change when there is none: at 0.01, one watched
//! ensemble in a hundred re-run with fresh seeds is expected to trip a test by chance.

use crate::pack_c::watch::WatchDirection;
use serde::{Deserialize, Serialize};

/// The test, in words, as reports state it.
pub const TEST_DESCRIPTION: &str = "two-sample Welch t-test (two-sided) on the means, \
    a regression only when p < alpha and the change is in the worse direction; one-sided \
    F-test on the variance ratio (current over baseline), a regression when p < alpha; \
    sample variances from the reported population standard deviations; alpha fixed in the \
    manifest before the first run (default 0.01)";

/// The mean, population standard deviation and size of one ensemble.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnsembleStats {
    /// Ensemble mean.
    pub mean: f64,
    /// Population standard deviation, as the engine reports it.
    pub std: f64,
    /// Number of realisations.
    pub n: f64,
}

impl EnsembleStats {
    /// The sample variance `std^2 n / (n - 1)`.
    pub fn sample_variance(&self) -> f64 {
        self.std * self.std * self.n / (self.n - 1.0)
    }

    fn valid(&self) -> bool {
        self.mean.is_finite() && self.std.is_finite() && self.std >= 0.0 && self.n >= 2.0
    }
}

/// The statistical verdict for one watch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatVerdict {
    /// The test, in words.
    pub test: String,
    /// The significance level used.
    pub alpha: f64,
    /// Baseline ensemble.
    pub baseline: EnsembleStats,
    /// Current ensemble.
    pub current: EnsembleStats,
    /// Welch t statistic (current minus baseline).
    pub t: f64,
    /// Welch-Satterthwaite degrees of freedom.
    pub df: f64,
    /// Two-sided p-value of the mean test.
    pub p_mean: f64,
    /// Variance ratio, current over baseline.
    pub f: f64,
    /// Degrees of freedom of the F ratio (current, baseline).
    pub df_f: (f64, f64),
    /// One-sided p-value of the variance test, P(F >= f).
    pub p_spread: f64,
    /// The mean changed significantly in the worse direction.
    pub mean_regressed: bool,
    /// The mean changed significantly in the better direction.
    pub mean_improved: bool,
    /// The spread grew significantly.
    pub spread_regressed: bool,
}

impl StatVerdict {
    /// A regression on either test.
    pub fn regressed(&self) -> bool {
        self.mean_regressed || self.spread_regressed
    }
}

/// Decide a watch with an ensemble. Errors when either ensemble cannot be tested.
pub fn decide(
    baseline: EnsembleStats,
    current: EnsembleStats,
    direction: WatchDirection,
    alpha: f64,
) -> Result<StatVerdict, String> {
    if !baseline.valid() || !current.valid() {
        return Err(format!(
            "an ensemble needs a finite mean, a finite non-negative std and n >= 2 (baseline {baseline:?}, current {current:?})"
        ));
    }
    let v1 = baseline.sample_variance();
    let v2 = current.sample_variance();
    let (n1, n2) = (baseline.n, current.n);
    let se2 = v1 / n1 + v2 / n2;
    let (t, df, p_mean) = if se2 > 0.0 {
        let t = (current.mean - baseline.mean) / se2.sqrt();
        let df = se2 * se2 / ((v1 / n1).powi(2) / (n1 - 1.0) + (v2 / n2).powi(2) / (n2 - 1.0));
        (t, df, student_t_two_sided(t, df))
    } else if current.mean == baseline.mean {
        (0.0, n1 + n2 - 2.0, 1.0)
    } else {
        (
            f64::INFINITY.copysign(current.mean - baseline.mean),
            n1 + n2 - 2.0,
            0.0,
        )
    };
    let (f, p_spread) = if v1 > 0.0 {
        let f = v2 / v1;
        (f, f_upper(f, n2 - 1.0, n1 - 1.0))
    } else if v2 > 0.0 {
        (f64::INFINITY, 0.0)
    } else {
        (1.0, 1.0)
    };
    let significant = p_mean < alpha;
    let worse = direction
        .degradation(baseline.mean, current.mean)
        .map(|d| d > 0.0);
    Ok(StatVerdict {
        test: TEST_DESCRIPTION.to_string(),
        alpha,
        baseline,
        current,
        t,
        df,
        p_mean,
        f,
        df_f: (n2 - 1.0, n1 - 1.0),
        p_spread,
        mean_regressed: significant && worse == Some(true),
        mean_improved: significant && worse == Some(false),
        spread_regressed: p_spread < alpha,
    })
}

/// Two-sided p-value of Student's t with `df` degrees of freedom.
pub fn student_t_two_sided(t: f64, df: f64) -> f64 {
    if !t.is_finite() {
        return 0.0;
    }
    let x = df / (df + t * t);
    reg_inc_beta(df / 2.0, 0.5, x).clamp(0.0, 1.0)
}

/// Upper-tail probability P(F >= f) of the F distribution with (d1, d2) degrees of
/// freedom.
pub fn f_upper(f: f64, d1: f64, d2: f64) -> f64 {
    if f.is_infinite() {
        return 0.0;
    }
    if f <= 0.0 {
        return 1.0;
    }
    let x = d2 / (d2 + d1 * f);
    reg_inc_beta(d2 / 2.0, d1 / 2.0, x).clamp(0.0, 1.0)
}

/// ln Gamma(x) by the Lanczos approximation (g = 7, n = 9), accurate to about 1e-15.
fn ln_gamma(x: f64) -> f64 {
    const G: f64 = 7.0;
    const C: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = C[0];
    let t = x + G + 0.5;
    for (i, c) in C.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularised incomplete beta function I_x(a, b).
pub fn reg_inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let ln_front = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    let front = ln_front.exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_cf(a, b, x) / a
    } else {
        1.0 - front * beta_cf(b, a, 1.0 - x) / b
    }
}

/// Continued fraction for the incomplete beta function (modified Lentz).
fn beta_cf(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    const EPS: f64 = 1e-15;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..=10_000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            break;
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference values from scipy.stats 1.x: t.sf(2.0, 10) * 2, f.sf(2.0, 10, 20),
    // f.sf(4.0, 199, 199), t.sf(1.0, 300) * 2.
    #[test]
    fn distributions_match_reference_values() {
        assert!((student_t_two_sided(2.0, 10.0) - 0.073_388_034_770_740_37).abs() < 1e-12);
        assert!((f_upper(2.0, 10.0, 20.0) - 0.089_782_714_843_75).abs() < 1e-12);
        let p = f_upper(4.0, 199.0, 199.0);
        assert!((p / 2.420_658_122_695_693_8e-21 - 1.0).abs() < 1e-8, "{p}");
        assert!((student_t_two_sided(1.0, 300.0) - 0.318_116_404_430_626_33).abs() < 1e-12);
        assert!((student_t_two_sided(-2.0, 10.0) - student_t_two_sided(2.0, 10.0)).abs() < 1e-15);
    }

    #[test]
    fn noise_is_not_a_regression_but_a_doubled_spread_is() {
        let base = EnsembleStats {
            mean: -0.45,
            std: 14.93,
            n: 200.0,
        };
        let reseeded = EnsembleStats {
            mean: 1.2,
            std: 16.1,
            n: 200.0,
        };
        let v = decide(base, reseeded, WatchDirection::Target(0.0), 0.01).unwrap();
        assert!(!v.regressed(), "{v:?}");
        let wider = EnsembleStats {
            mean: 0.3,
            std: 29.9,
            n: 200.0,
        };
        let v = decide(base, wider, WatchDirection::Target(0.0), 0.01).unwrap();
        assert!(v.spread_regressed && v.regressed(), "{v:?}");
    }

    #[test]
    fn a_significant_mean_shift_regresses_only_in_the_worse_direction() {
        let base = EnsembleStats {
            mean: 10.0,
            std: 1.0,
            n: 100.0,
        };
        let up = EnsembleStats {
            mean: 11.0,
            std: 1.0,
            n: 100.0,
        };
        let lower = decide(base, up, WatchDirection::Lower, 0.01).unwrap();
        assert!(lower.mean_regressed && !lower.mean_improved);
        let higher = decide(base, up, WatchDirection::Higher, 0.01).unwrap();
        assert!(!higher.mean_regressed && higher.mean_improved);
        let bad = EnsembleStats {
            mean: 1.0,
            std: 1.0,
            n: 1.0,
        };
        assert!(decide(base, bad, WatchDirection::Lower, 0.01).is_err());
    }
}
