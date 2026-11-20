// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Protection-level statistics and Gaussian overbound checks, shared by the automotive
//! ([`super::auto`]) and rail ([`super::rail`]) tools.
//!
//! * [`stanford`]: every epoch of position error `e`, protection level `PL` and alert limit
//!   `AL` falls into one Stanford-diagram region: **nominal** (`e ≤ PL < AL`), **misleading
//!   information** (MI: `PL < e < AL`, `PL < AL`), **hazardously misleading information**
//!   (HMI: `PL < AL ≤ e`) or **unavailable** (`PL ≥ AL`). The epochs are not independent,
//!   so the bound on the MI rate uses an effective sample size `n·Δt/τ` from the stated
//!   decorrelation time `τ`; the bound is the exact (Clopper-Pearson) two-sided 95 % upper
//!   limit. The epochs needed to support a target rate with no MI observed follow from the
//!   same limit.
//! * [`overbound`]: a zero-mean Gaussian of the stated sigma overbounds the normalised errors
//!   `z = e/σ` in the tails if, at every threshold `k`, the observed exceedance `P(|z| > k)`
//!   (its 95 % lower limit, so that sampling noise alone does not fail the check) is no more
//!   than `2(1 − Φ(k))`. This is the two-sided CDF overbound condition used in safety cases;
//!   it is evidence for the stated sigma, not a proof of it beyond the data.
//!
//! MODELLED: statistics of the records as supplied.

use serde::Serialize;

use super::{cp95, norm_cdf};

/// One epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Epoch {
    pub err_m: f64,
    pub pl_m: f64,
    pub al_m: f64,
}

/// The Stanford region of an epoch.
pub fn region(e: &Epoch) -> &'static str {
    if e.pl_m >= e.al_m {
        "unavailable"
    } else if e.err_m <= e.pl_m {
        "nominal"
    } else if e.err_m < e.al_m {
        "MI"
    } else {
        "HMI"
    }
}

/// Stanford-diagram statistics.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Stanford {
    pub epochs: usize,
    pub nominal: usize,
    pub mi: usize,
    pub hmi: usize,
    pub unavailable: usize,
    /// Epochs where the error exceeded the PL, in any region (including unavailable).
    pub pl_exceeded: usize,
    pub availability: f64,
    /// Largest error/PL ratio observed (above 1 means the PL did not bound the error).
    pub max_err_over_pl: Option<f64>,
    /// Effective independent samples `n·Δt/τ` (at most `n`).
    pub effective_samples: f64,
    /// 95 % upper limit of the per-sample MI-or-HMI rate among available epochs.
    pub mi_rate_upper95: Option<f64>,
    /// 95 % upper limit of the per-sample HMI rate among available epochs.
    pub hmi_rate_upper95: Option<f64>,
}

/// Stanford statistics of `epochs` sampled every `dt_s` with decorrelation time `tau_s`.
pub fn stanford(epochs: &[Epoch], dt_s: f64, tau_s: f64) -> Stanford {
    let mut s = Stanford {
        epochs: epochs.len(),
        nominal: 0,
        mi: 0,
        hmi: 0,
        unavailable: 0,
        pl_exceeded: 0,
        availability: 0.0,
        max_err_over_pl: None,
        effective_samples: 0.0,
        mi_rate_upper95: None,
        hmi_rate_upper95: None,
    };
    for e in epochs {
        match region(e) {
            "nominal" => s.nominal += 1,
            "MI" => s.mi += 1,
            "HMI" => s.hmi += 1,
            _ => s.unavailable += 1,
        }
        if e.err_m > e.pl_m {
            s.pl_exceeded += 1;
        }
        if e.pl_m > 0.0 {
            let r = e.err_m / e.pl_m;
            s.max_err_over_pl = Some(s.max_err_over_pl.map_or(r, |m: f64| m.max(r)));
        }
    }
    let avail = s.nominal + s.mi + s.hmi;
    if !epochs.is_empty() {
        s.availability = avail as f64 / epochs.len() as f64;
    }
    let ratio = if tau_s > dt_s && dt_s > 0.0 {
        dt_s / tau_s
    } else {
        1.0
    };
    s.effective_samples = avail as f64 * ratio;
    let n_eff = s.effective_samples.floor() as usize;
    if n_eff > 0 {
        // Events are scaled the same way, never below one when any occurred.
        let scale = |k: usize| -> usize {
            if k == 0 {
                0
            } else {
                ((k as f64 * ratio).ceil() as usize).clamp(1, n_eff)
            }
        };
        s.mi_rate_upper95 = Some(cp95(scale(s.mi + s.hmi), n_eff).1);
        s.hmi_rate_upper95 = Some(cp95(scale(s.hmi), n_eff).1);
    }
    s
}

/// Independent samples needed for the 95 % upper limit of a zero-event rate to reach `p`:
/// the smallest `n` with `1 − 0.025^(1/n) ≤ p`.
pub fn samples_needed(p: f64) -> Option<f64> {
    if !(p > 0.0 && p < 1.0) {
        return None;
    }
    Some((0.025f64.ln() / (1.0 - p).ln()).ceil())
}

/// One threshold of an overbound check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OverboundPoint {
    pub k: f64,
    pub model: f64,
    pub observed: f64,
    pub observed_lower95: f64,
    pub exceed: usize,
    pub bounded: bool,
}

/// The overbound check of normalised errors.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Overbound {
    pub samples: usize,
    pub points: Vec<OverboundPoint>,
    /// Every threshold bounded.
    pub bounded: bool,
    /// Largest |z| seen.
    pub max_abs_z: Option<f64>,
    /// The sigma inflation that would make every observed point bounded (1 when none needed).
    pub inflation_needed: f64,
}

/// Check that a zero-mean unit Gaussian overbounds `z` at thresholds 1, 1.5, … 6.
pub fn overbound(z: &[f64]) -> Overbound {
    let n = z.len();
    let mut points = Vec::new();
    let mut infl: f64 = 1.0;
    let mut k = 1.0;
    while k <= 6.0 + 1e-9 {
        let exceed = z.iter().filter(|x| x.abs() > k).count();
        let model = 2.0 * (1.0 - norm_cdf(k));
        let observed = if n > 0 { exceed as f64 / n as f64 } else { 0.0 };
        let lower = if n > 0 { cp95(exceed, n).0 } else { 0.0 };
        let bounded = lower <= model;
        if !bounded {
            // The Gaussian sigma s with 2(1 − Φ(k/s)) = lower.
            let k_eq = super::norm_quantile(1.0 - lower / 2.0);
            if k_eq > 0.0 {
                infl = infl.max(k / k_eq);
            }
        }
        points.push(OverboundPoint {
            k,
            model,
            observed,
            observed_lower95: lower,
            exceed,
            bounded,
        });
        k += 0.5;
    }
    Overbound {
        samples: n,
        bounded: points.iter().all(|p| p.bounded),
        points,
        max_abs_z: z
            .iter()
            .map(|x| x.abs())
            .fold(None, |m, x| Some(m.map_or(x, |m: f64| m.max(x)))),
        inflation_needed: infl,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(err: f64, pl: f64) -> Epoch {
        Epoch {
            err_m: err,
            pl_m: pl,
            al_m: 10.0,
        }
    }

    #[test]
    fn regions_and_counts() {
        assert_eq!(region(&ep(1.0, 2.0)), "nominal");
        assert_eq!(region(&ep(3.0, 2.0)), "MI");
        assert_eq!(region(&ep(12.0, 2.0)), "HMI");
        assert_eq!(region(&ep(1.0, 11.0)), "unavailable");
        let s = stanford(
            &[ep(1.0, 2.0), ep(3.0, 2.0), ep(12.0, 2.0), ep(1.0, 11.0)],
            1.0,
            1.0,
        );
        assert_eq!((s.nominal, s.mi, s.hmi, s.unavailable), (1, 1, 1, 1));
        assert_eq!(s.pl_exceeded, 2);
        assert!((s.availability - 0.75).abs() < 1e-12);
    }

    #[test]
    fn decorrelation_reduces_evidence() {
        let v: Vec<Epoch> = (0..10_000).map(|_| ep(1.0, 2.0)).collect();
        let a = stanford(&v, 1.0, 1.0);
        let b = stanford(&v, 1.0, 100.0);
        assert!((b.effective_samples - 100.0).abs() < 1e-9);
        assert!(b.mi_rate_upper95.unwrap() > a.mi_rate_upper95.unwrap() * 50.0);
        // Rule of three, two-sided 95 %: about 3.69/p.
        let n = samples_needed(1e-3).unwrap();
        assert!((3680.0..3700.0).contains(&n), "{n}");
    }

    #[test]
    fn gaussian_is_bounded_and_heavy_tail_is_not() {
        // Deterministic normal sample by the inverse CDF on a regular grid.
        let n = 20_000;
        let z: Vec<f64> = (0..n)
            .map(|i| super::super::norm_quantile((i as f64 + 0.5) / n as f64))
            .collect();
        let o = overbound(&z);
        assert!(o.bounded, "{:?}", o.points);
        let heavy: Vec<f64> = z.iter().map(|x| x * 1.6).collect();
        let h = overbound(&heavy);
        assert!(!h.bounded);
        assert!(
            h.inflation_needed > 1.3 && h.inflation_needed < 1.8,
            "{}",
            h.inflation_needed
        );
    }
}
