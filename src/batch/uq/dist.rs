// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Input distributions and their inverse distribution functions.
//!
//! A study states each uncertain input as one of five distributions. A sample is drawn
//! by mapping a uniform number `u` in (0, 1) through the inverse cumulative
//! distribution function, so the same `u` always gives the same value and the
//! stratification of a Latin hypercube carries over to the input's own scale.
//!
//! The inverse of the normal distribution is Wichura's algorithm AS 241 (routine
//! PPND16, relative accuracy about 1e-16): M. J. Wichura, "Algorithm AS 241: The
//! Percentage Points of the Normal Distribution", Applied Statistics 37(3), 1988,
//! pp. 477-484.

use serde_json::{json, Value};

/// One input distribution.
#[derive(Debug, Clone, PartialEq)]
pub enum Dist {
    /// Normal with the given mean and standard deviation.
    Normal { mean: f64, std: f64 },
    /// Uniform on `[min, max]`.
    Uniform { min: f64, max: f64 },
    /// Lognormal: `median * exp(ln_std * z)` with `z` standard normal.
    Lognormal { median: f64, ln_std: f64 },
    /// Triangular on `[min, max]` with its peak at `mode`.
    Triangular { min: f64, mode: f64, max: f64 },
    /// A finite list of values with probabilities (`weights`, normalised to sum 1).
    Discrete { values: Vec<f64>, weights: Vec<f64> },
}

impl Dist {
    /// The distribution's name as written in a study file.
    pub fn name(&self) -> &'static str {
        match self {
            Dist::Normal { .. } => "normal",
            Dist::Uniform { .. } => "uniform",
            Dist::Lognormal { .. } => "lognormal",
            Dist::Triangular { .. } => "triangular",
            Dist::Discrete { .. } => "discrete",
        }
    }

    /// Check the parameters. Every number must be finite; a spread may be zero (the
    /// input is then constant and its sensitivity is reported as null) but never
    /// negative or not-a-number.
    pub fn validate(&self) -> Result<(), String> {
        let finite = |what: &str, v: f64| {
            if v.is_finite() {
                Ok(())
            } else {
                Err(format!("`{what}` must be a finite number (got {v})"))
            }
        };
        match self {
            Dist::Normal { mean, std } => {
                finite("mean", *mean)?;
                finite("std", *std)?;
                if *std < 0.0 {
                    return Err(format!("`std` must not be negative (got {std})"));
                }
            }
            Dist::Uniform { min, max } => {
                finite("min", *min)?;
                finite("max", *max)?;
                if min > max {
                    return Err(format!("`min` ({min}) is above `max` ({max})"));
                }
            }
            Dist::Lognormal { median, ln_std } => {
                finite("median", *median)?;
                finite("ln_std", *ln_std)?;
                if *median <= 0.0 {
                    return Err(format!("`median` must be positive (got {median})"));
                }
                if *ln_std < 0.0 {
                    return Err(format!("`ln_std` must not be negative (got {ln_std})"));
                }
            }
            Dist::Triangular { min, mode, max } => {
                finite("min", *min)?;
                finite("mode", *mode)?;
                finite("max", *max)?;
                if !(min <= mode && mode <= max) {
                    return Err(format!(
                        "need min <= mode <= max (got min {min}, mode {mode}, max {max})"
                    ));
                }
            }
            Dist::Discrete { values, weights } => {
                if values.is_empty() {
                    return Err("`values` must list at least one value".into());
                }
                if values.len() != weights.len() {
                    return Err(format!(
                        "`values` has {} entries but `weights` has {}",
                        values.len(),
                        weights.len()
                    ));
                }
                for v in values {
                    finite("values", *v)?;
                }
                for w in weights {
                    finite("weights", *w)?;
                    if *w < 0.0 {
                        return Err(format!("a weight must not be negative (got {w})"));
                    }
                }
                if weights.iter().sum::<f64>() <= 0.0 {
                    return Err("the weights sum to zero".into());
                }
            }
        }
        Ok(())
    }

    /// The value at cumulative probability `u`, for `u` strictly inside (0, 1).
    pub fn inverse_cdf(&self, u: f64) -> f64 {
        match self {
            Dist::Normal { mean, std } => {
                if *std == 0.0 {
                    *mean
                } else {
                    mean + std * normal_inverse_cdf(u)
                }
            }
            Dist::Uniform { min, max } => min + (max - min) * u,
            Dist::Lognormal { median, ln_std } => {
                if *ln_std == 0.0 {
                    *median
                } else {
                    median * (ln_std * normal_inverse_cdf(u)).exp()
                }
            }
            Dist::Triangular { min, mode, max } => {
                let span = max - min;
                if span == 0.0 {
                    return *min;
                }
                let split = (mode - min) / span;
                if u < split {
                    min + (u * span * (mode - min)).sqrt()
                } else {
                    max - ((1.0 - u) * span * (max - mode)).sqrt()
                }
            }
            Dist::Discrete { values, weights } => {
                let total: f64 = weights.iter().sum();
                let mut acc = 0.0;
                for (v, w) in values.iter().zip(weights) {
                    acc += w / total;
                    if u < acc {
                        return *v;
                    }
                }
                // Rounding left `acc` a hair under 1: the last value with a weight.
                values
                    .iter()
                    .zip(weights)
                    .rev()
                    .find(|(_, w)| **w > 0.0)
                    .map(|(v, _)| *v)
                    .unwrap_or(values[values.len() - 1])
            }
        }
    }

    /// The distribution's mean (closed form).
    pub fn mean(&self) -> f64 {
        match self {
            Dist::Normal { mean, .. } => *mean,
            Dist::Uniform { min, max } => 0.5 * (min + max),
            Dist::Lognormal { median, ln_std } => median * (0.5 * ln_std * ln_std).exp(),
            Dist::Triangular { min, mode, max } => (min + mode + max) / 3.0,
            Dist::Discrete { values, weights } => {
                let total: f64 = weights.iter().sum();
                values.iter().zip(weights).map(|(v, w)| v * w).sum::<f64>() / total
            }
        }
    }

    /// The distribution's variance (closed form).
    pub fn variance(&self) -> f64 {
        match self {
            Dist::Normal { std, .. } => std * std,
            Dist::Uniform { min, max } => (max - min).powi(2) / 12.0,
            Dist::Lognormal { median, ln_std } => {
                let s2 = ln_std * ln_std;
                median * median * s2.exp() * (s2.exp() - 1.0)
            }
            Dist::Triangular { min, mode, max } => {
                (min * min + mode * mode + max * max - min * mode - min * max - mode * max) / 18.0
            }
            Dist::Discrete { values, weights } => {
                let total: f64 = weights.iter().sum();
                let m = self.mean();
                values
                    .iter()
                    .zip(weights)
                    .map(|(v, w)| (v - m).powi(2) * w)
                    .sum::<f64>()
                    / total
            }
        }
    }

    /// The distribution as it is echoed in a result document.
    pub fn to_json(&self) -> Value {
        match self {
            Dist::Normal { mean, std } => json!({"type": "normal", "mean": mean, "std": std}),
            Dist::Uniform { min, max } => json!({"type": "uniform", "min": min, "max": max}),
            Dist::Lognormal { median, ln_std } => {
                json!({"type": "lognormal", "median": median, "ln_std": ln_std})
            }
            Dist::Triangular { min, mode, max } => {
                json!({"type": "triangular", "min": min, "mode": mode, "max": max})
            }
            Dist::Discrete { values, weights } => {
                let total: f64 = weights.iter().sum();
                let p: Vec<f64> = weights.iter().map(|w| w / total).collect();
                json!({"type": "discrete", "values": values, "probabilities": p})
            }
        }
    }

    /// One line for a report, for example `normal(mean 55, std 1)`.
    pub fn describe(&self) -> String {
        match self {
            Dist::Normal { mean, std } => format!("normal(mean {}, std {})", g(*mean), g(*std)),
            Dist::Uniform { min, max } => format!("uniform({} to {})", g(*min), g(*max)),
            Dist::Lognormal { median, ln_std } => {
                format!("lognormal(median {}, ln_std {})", g(*median), g(*ln_std))
            }
            Dist::Triangular { min, mode, max } => {
                format!("triangular({}, mode {}, {})", g(*min), g(*mode), g(*max))
            }
            Dist::Discrete { values, .. } => format!("discrete({} values)", values.len()),
        }
    }
}

/// A parameter as a reader writes it: plain between 1e-4 and 1e6, else with an exponent
/// (`4.5e-20`, not twenty zeros).
fn g(v: f64) -> String {
    let a = v.abs();
    if v == 0.0 || (1e-4..1e6).contains(&a) {
        format!("{v}")
    } else {
        format!("{v:e}")
    }
}

/// The inverse of the standard normal cumulative distribution function (Wichura's
/// AS 241, routine PPND16). `p` outside (0, 1) returns an infinity of the right sign.
pub fn normal_inverse_cdf(p: f64) -> f64 {
    if p.is_nan() {
        return f64::NAN;
    }
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }
    const A: [f64; 8] = [
        3.387_132_872_796_366_5,
        1.331_416_678_917_843_8e2,
        1.971_590_950_306_551_3e3,
        1.373_169_376_550_946_2e4,
        4.592_195_393_154_987e4,
        6.726_577_092_700_87e4,
        3.343_057_558_358_813e4,
        2.509_080_928_730_122_7e3,
    ];
    const B: [f64; 8] = [
        1.0,
        4.231_333_070_160_091e1,
        6.871_870_074_920_579e2,
        5.394_196_021_424_751e3,
        2.121_379_430_158_659_7e4,
        3.930_789_580_009_271e4,
        2.872_908_573_572_194_3e4,
        5.226_495_278_852_854e3,
    ];
    const C: [f64; 8] = [
        1.423_437_110_749_683_5,
        4.630_337_846_156_546,
        5.769_497_221_460_691,
        3.647_848_324_763_204_5,
        1.270_458_252_452_368_4,
        2.417_807_251_774_506e-1,
        2.272_384_498_926_918_4e-2,
        7.745_450_142_783_414e-4,
    ];
    const D: [f64; 8] = [
        1.0,
        2.053_191_626_637_759,
        1.676_384_830_183_803_8,
        6.897_673_349_851e-1,
        1.481_039_764_274_800_8e-1,
        1.519_866_656_361_645_7e-2,
        5.475_938_084_995_345e-4,
        1.050_750_071_644_416_9e-9,
    ];
    const E: [f64; 8] = [
        6.657_904_643_501_103,
        5.463_784_911_164_114,
        1.784_826_539_917_291_3,
        2.965_605_718_285_048_7e-1,
        2.653_218_952_657_612_4e-2,
        1.242_660_947_388_078_4e-3,
        2.711_555_568_743_487_6e-5,
        2.010_334_399_292_288_1e-7,
    ];
    const F: [f64; 8] = [
        1.0,
        5.998_322_065_558_88e-1,
        1.369_298_809_227_358e-1,
        1.487_536_129_085_061_5e-2,
        7.868_691_311_456_133e-4,
        1.846_318_317_510_054_8e-5,
        1.421_511_758_316_446e-7,
        2.044_263_103_389_939_7e-15,
    ];
    // Horner evaluation, highest coefficient first.
    fn poly(c: &[f64; 8], r: f64) -> f64 {
        c.iter().rev().fold(0.0, |acc, k| acc * r + k)
    }
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        let r = 0.180_625 - q * q;
        return q * poly(&A, r) / poly(&B, r);
    }
    let tail = if q < 0.0 { p } else { 1.0 - p };
    let r = (-tail.ln()).sqrt();
    let val = if r <= 5.0 {
        let r = r - 1.6;
        poly(&C, r) / poly(&D, r)
    } else {
        let r = r - 5.0;
        poly(&E, r) / poly(&F, r)
    };
    if q < 0.0 {
        -val
    } else {
        val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard normal cumulative distribution function by composite Simpson
    /// integration of the density from 0: an independent check of the inverse, with
    /// no shared code and no shared constants.
    fn normal_cdf_by_integration(x: f64) -> f64 {
        let n = 20_000usize; // even
        let h = x / n as f64;
        let pdf = |t: f64| (-0.5 * t * t).exp() / (2.0 * std::f64::consts::PI).sqrt();
        let mut s = pdf(0.0) + pdf(x);
        for i in 1..n {
            s += pdf(i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
        }
        0.5 + s * h / 3.0
    }

    #[test]
    fn normal_inverse_matches_published_quantiles() {
        // Standard values (for example NIST/SEMATECH e-Handbook, table of the normal
        // distribution; the 97.5 % point is the familiar 1.959964).
        for (p, z) in [
            (0.5, 0.0),
            (0.975, 1.959_963_984_540_054),
            (0.95, 1.644_853_626_951_472_6),
            (0.841_344_746_068_542_9, 1.0),
            (0.001, -3.090_232_306_167_813_6),
            (1.0e-10, -6.361_340_902_404_056),
        ] {
            let got = normal_inverse_cdf(p);
            assert!((got - z).abs() < 1e-9, "p {p}: got {got}, expected {z}");
        }
    }

    #[test]
    fn normal_inverse_round_trips_through_an_independent_integral() {
        // All three branches of the algorithm: centre, near tail, far tail.
        for p in [
            0.08,
            0.3,
            0.5,
            0.7,
            0.92,
            0.999,
            1.0e-4,
            1.0e-9,
            1.0 - 1.0e-9,
        ] {
            let z = normal_inverse_cdf(p);
            let back = normal_cdf_by_integration(z);
            assert!(
                (back - p).abs() < 1e-11,
                "p {p}: inverse {z} integrates back to {back}"
            );
        }
        assert_eq!(normal_inverse_cdf(0.0), f64::NEG_INFINITY);
        assert_eq!(normal_inverse_cdf(1.0), f64::INFINITY);
    }

    /// The mean and variance of `d` by pushing a fine midpoint grid of probabilities
    /// through its inverse distribution function.
    fn moments_by_quadrature(d: &Dist) -> (f64, f64) {
        let n = 400_000usize;
        let xs: Vec<f64> = (0..n)
            .map(|i| d.inverse_cdf((i as f64 + 0.5) / n as f64))
            .collect();
        let m = xs.iter().sum::<f64>() / n as f64;
        let v = xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n as f64;
        (m, v)
    }

    #[test]
    fn every_inverse_reproduces_its_closed_form_mean_and_variance() {
        let cases = [
            Dist::Normal {
                mean: 55.0,
                std: 1.5,
            },
            Dist::Uniform {
                min: -2.0,
                max: 6.0,
            },
            Dist::Lognormal {
                median: 3.0,
                ln_std: 0.4,
            },
            Dist::Triangular {
                min: 1.0,
                mode: 2.0,
                max: 6.0,
            },
            Dist::Discrete {
                values: vec![1.0, 2.0, 10.0],
                weights: vec![1.0, 2.0, 1.0],
            },
        ];
        for d in cases {
            d.validate().expect("valid parameters");
            let (m, v) = moments_by_quadrature(&d);
            let tol = 2e-3;
            assert!(
                (m - d.mean()).abs() <= tol * d.mean().abs().max(1.0),
                "{}: mean {m} vs closed form {}",
                d.name(),
                d.mean()
            );
            assert!(
                (v - d.variance()).abs() <= tol * d.variance().max(1.0),
                "{}: variance {v} vs closed form {}",
                d.name(),
                d.variance()
            );
        }
    }

    #[test]
    fn a_description_is_short_for_very_small_and_very_large_parameters() {
        let d = Dist::Uniform {
            min: 4.5e-20,
            max: 2.7e-19,
        };
        assert_eq!(d.describe(), "uniform(4.5e-20 to 2.7e-19)");
        let d = Dist::Normal {
            mean: 55.0,
            std: 1.0,
        };
        assert_eq!(d.describe(), "normal(mean 55, std 1)");
    }

    #[test]
    fn triangular_inverse_is_exact_at_known_points() {
        // Symmetric triangle on [0, 2]: the median is the mode, and the cumulative
        // probability at x <= 1 is x^2 / 2, so u = 0.125 gives x = 0.5.
        let d = Dist::Triangular {
            min: 0.0,
            mode: 1.0,
            max: 2.0,
        };
        assert!((d.inverse_cdf(0.5) - 1.0).abs() < 1e-12);
        assert!((d.inverse_cdf(0.125) - 0.5).abs() < 1e-12);
        assert!((d.inverse_cdf(0.875) - 1.5).abs() < 1e-12);
    }

    #[test]
    fn discrete_inverse_follows_the_weights() {
        let d = Dist::Discrete {
            values: vec![10.0, 20.0, 30.0],
            weights: vec![1.0, 1.0, 2.0],
        };
        assert_eq!(d.inverse_cdf(0.10), 10.0);
        assert_eq!(d.inverse_cdf(0.30), 20.0);
        assert_eq!(d.inverse_cdf(0.60), 30.0);
        assert_eq!(d.inverse_cdf(0.999_999), 30.0);
    }

    #[test]
    fn bad_parameters_are_refused() {
        let bad = [
            Dist::Normal {
                mean: 0.0,
                std: -1.0,
            },
            Dist::Normal {
                mean: 0.0,
                std: f64::NAN,
            },
            Dist::Normal {
                mean: f64::INFINITY,
                std: 1.0,
            },
            Dist::Uniform { min: 2.0, max: 1.0 },
            Dist::Lognormal {
                median: 0.0,
                ln_std: 0.1,
            },
            Dist::Lognormal {
                median: 1.0,
                ln_std: -0.1,
            },
            Dist::Triangular {
                min: 0.0,
                mode: 3.0,
                max: 2.0,
            },
            Dist::Discrete {
                values: vec![],
                weights: vec![],
            },
            Dist::Discrete {
                values: vec![1.0, 2.0],
                weights: vec![1.0],
            },
            Dist::Discrete {
                values: vec![1.0],
                weights: vec![-1.0],
            },
            Dist::Discrete {
                values: vec![1.0],
                weights: vec![0.0],
            },
        ];
        for d in bad {
            assert!(d.validate().is_err(), "{d:?} must be refused");
        }
        // A zero spread is allowed: the input is constant.
        assert!(Dist::Normal {
            mean: 1.0,
            std: 0.0
        }
        .validate()
        .is_ok());
    }
}
