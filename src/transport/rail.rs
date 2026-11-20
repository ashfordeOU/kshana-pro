// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Rail GNSS safety evidence (backlog B3005, first slice)
//!
//! Research 2026-11-20 gaps 07:H01, H02, H06, H09 and 08:D17, E10, E11.
//!
//! * [`thr`]: a position-error hazard model for a GNSS-based virtual balise or train
//!   position function, against the tolerable hazard rate (THR) apportioned to it. The
//!   hazard is an along-track error beyond the alert limit (the confidence interval the
//!   train relies on) that nobody flagged. Contributions per hour: nominal errors (Gaussian
//!   overbound with bias, per independent sample), undetected satellite or system faults,
//!   and **jamming and spoofing episodes** with their own error and detection probability,
//!   so that interference is modelled as position error, not only as outage. Each
//!   contribution's share of the THR and the margin in decades.
//! * [`balise`]: a virtual-balise passage error test: reported passage against the reference
//!   (a physical balise, a track survey or a reference trajectory) per passage, along-track
//!   error with the mean, the standard deviation and the expanded uncertainty of the mean
//!   (GUM, k = 2), exceedances of the alert limit with an exact bound, and a Gaussian
//!   overbound check of the errors normalised by the claimed sigma
//!   ([`super::integrity::overbound`]).
//! * The EN 50129 evidence catalogue `rail-gnss-sil4` (`report-templates/regpacks/`),
//!   assessed with `kshana-pro regpack assess`, states per safety-case heading which signed
//!   evidence was supplied: the SIL 4 evidence template, populated from the evidence.
//!
//! Not in this slice: carrier-phase virtual-balise integrity validation, EGNOS-as-
//! pre-existing-item argument text, rail datasets with ground truth under jamming, and
//! freight-train integrity and level-crossing checks.
//!
//! **Label: MODELLED.** The THR model is arithmetic on stated parameters; the balise test
//! statistics of the passages as supplied. Neither is a safety assessment.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::integrity::{self, Overbound};
use super::{cp95, mean, norm_cdf, quantile, Table};

pub const FEATURE: &str = "pro-rail-evidence";
pub const THR_FORMAT: &str = "kshana-pro-rail-thr/1";
pub const BALISE_FORMAT: &str = "kshana-pro-rail-balise/1";
pub const LABEL: &str = "MODELLED: arithmetic on the stated parameters and the passages as supplied. Evidence for the safety case; acceptance is for the independent safety assessor and the notified or designated body.";
pub const SIL_BASIS: &str = "EN 50129 relates SIL 4 to a tolerable hazard rate of 10⁻⁹ to 10⁻⁸ per hour per function (to confirm against the edition in use); the share apportioned to the position function is the applicant's.";

/// Invert a small square matrix by Gauss-Jordan elimination with partial pivoting.
pub fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut m: Vec<Vec<f64>> = a
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut x = r.clone();
            x.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            x
        })
        .collect();
    for c in 0..n {
        let p = (c..n).max_by(|x, y| m[*x][c].abs().total_cmp(&m[*y][c].abs()))?;
        if m[p][c].abs() < 1e-300 {
            return None;
        }
        m.swap(c, p);
        let d = m[c][c];
        for v in m[c].iter_mut() {
            *v /= d;
        }
        let pivot = m[c].clone();
        for (r, row) in m.iter_mut().enumerate() {
            let f = row[c];
            if r != c && f != 0.0 {
                for (x, p) in row.iter_mut().zip(&pivot) {
                    *x -= f * p;
                }
            }
        }
    }
    Some(m.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// An interference contribution.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Interference {
    /// Episodes per hour of operation on the line (exposure).
    pub rate_per_hour: f64,
    /// Probability that an episode is not flagged before the error matters.
    pub p_undetected: f64,
    /// Along-track error standard deviation during an unflagged episode (m); when absent,
    /// `p_exceed` is used.
    pub error_sigma_m: Option<f64>,
    /// Probability that an unflagged episode puts the error beyond the alert limit.
    pub p_exceed: Option<f64>,
}

/// The THR request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrRequest {
    pub title: String,
    /// The THR apportioned to the position function (per hour).
    pub thr_per_hour: f64,
    /// Alert limit along track (m).
    pub alert_limit_m: f64,
    /// Nominal overbound: sigma and bias along track (m).
    pub sigma_m: f64,
    #[serde(default)]
    pub bias_m: f64,
    /// Independent samples per hour (3600 / decorrelation time).
    pub samples_per_hour: f64,
    /// Satellite or system faults per hour reaching the user, and the monitor's
    /// missed-detection probability.
    #[serde(default)]
    pub fault_per_hour: f64,
    #[serde(default = "one")]
    pub fault_p_md: f64,
    pub jamming: Option<Interference>,
    pub spoofing: Option<Interference>,
}

fn one() -> f64 {
    1.0
}

/// One contribution.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Contribution {
    pub name: String,
    pub per_hour: f64,
    pub share_of_thr: f64,
    pub basis: String,
}

/// The THR report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ThrReport {
    pub format: &'static str,
    pub label: &'static str,
    pub sil_basis: &'static str,
    pub title: String,
    pub utc: String,
    pub thr_per_hour: f64,
    pub contributions: Vec<Contribution>,
    pub total_per_hour: f64,
    /// `log10(THR / total)`; positive means headroom.
    pub margin_decades: f64,
    /// The nominal sigma at which the nominal term alone would use a tenth of the THR.
    pub sigma_for_tenth_m: Option<f64>,
    pub within: bool,
    pub verdict: String,
}

/// Probability that a Gaussian `N(bias, sigma)` lies beyond `±limit`.
pub fn p_beyond(limit: f64, bias: f64, sigma: f64) -> f64 {
    if !super::pos(sigma) {
        return if bias.abs() > limit { 1.0 } else { 0.0 };
    }
    norm_cdf(-(limit - bias) / sigma) + norm_cdf(-(limit + bias) / sigma)
}

fn interference(i: &Interference, al: f64, name: &str) -> Result<Contribution, String> {
    if !super::nonneg(i.rate_per_hour) || !(0.0..=1.0).contains(&i.p_undetected) {
        return Err(format!(
            "{name}: rate_per_hour >= 0 and p_undetected in [0, 1]"
        ));
    }
    let (pe, basis) = match (i.error_sigma_m, i.p_exceed) {
        (Some(s), _) => (p_beyond(al, 0.0, s), format!("unflagged error σ {s} m")),
        (None, Some(p)) if (0.0..=1.0).contains(&p) => {
            (p, format!("stated exceedance probability {p}"))
        }
        _ => (1.0, "every unflagged episode taken as hazardous".into()),
    };
    Ok(Contribution {
        name: name.into(),
        per_hour: i.rate_per_hour * i.p_undetected * pe,
        share_of_thr: 0.0,
        basis: format!(
            "{} /h × undetected {} × exceed {:.3e} ({basis})",
            i.rate_per_hour, i.p_undetected, pe
        ),
    })
}

/// The THR position-error model.
pub fn thr(req: &ThrRequest, utc: &str) -> Result<ThrReport, String> {
    if !(req.thr_per_hour > 0.0
        && req.alert_limit_m > 0.0
        && req.sigma_m > 0.0
        && req.samples_per_hour > 0.0)
    {
        return Err("thr_per_hour, alert_limit_m, sigma_m and samples_per_hour must be > 0".into());
    }
    if !super::nonneg(req.fault_per_hour) || !(0.0..=1.0).contains(&req.fault_p_md) {
        return Err("fault_per_hour >= 0 and fault_p_md in [0, 1]".into());
    }
    let pn = p_beyond(req.alert_limit_m, req.bias_m, req.sigma_m);
    let mut c = vec![
        Contribution {
            name: "nominal error".into(),
            per_hour: req.samples_per_hour * pn,
            share_of_thr: 0.0,
            basis: format!(
                "{} samples/h × P(|N({}, {})| > {} m) = {:.3e}",
                req.samples_per_hour, req.bias_m, req.sigma_m, req.alert_limit_m, pn
            ),
        },
        Contribution {
            name: "undetected fault".into(),
            per_hour: req.fault_per_hour * req.fault_p_md,
            share_of_thr: 0.0,
            basis: format!(
                "{} faults/h × missed detection {} (error taken beyond the limit)",
                req.fault_per_hour, req.fault_p_md
            ),
        },
    ];
    if let Some(j) = &req.jamming {
        c.push(interference(j, req.alert_limit_m, "jamming")?);
    }
    if let Some(s) = &req.spoofing {
        c.push(interference(s, req.alert_limit_m, "spoofing")?);
    }
    let total: f64 = c.iter().map(|x| x.per_hour).sum();
    for x in &mut c {
        x.share_of_thr = x.per_hour / req.thr_per_hour;
    }
    // Bisection on sigma for the nominal term = THR/10.
    let target = req.thr_per_hour / 10.0;
    let f = |s: f64| req.samples_per_hour * p_beyond(req.alert_limit_m, req.bias_m, s) - target;
    let sigma_tenth = if req.bias_m.abs() >= req.alert_limit_m {
        None
    } else {
        let (mut lo, mut hi) = (1e-6, req.alert_limit_m * 10.0);
        if f(lo) > 0.0 || f(hi) < 0.0 {
            None
        } else {
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if f(mid) > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            Some(lo)
        }
    };
    let margin = if total > 0.0 {
        (req.thr_per_hour / total).log10()
    } else {
        f64::INFINITY
    };
    let within = total <= req.thr_per_hour;
    let largest = c
        .iter()
        .max_by(|a, b| a.per_hour.total_cmp(&b.per_hour))
        .map(|x| x.name.clone())
        .unwrap_or_default();
    let verdict = format!(
        "modelled hazard rate {total:.2e}/h against the apportioned {:.1e}/h: {} (margin {:+.2} decades); largest contribution: {largest}",
        req.thr_per_hour,
        if within { "within" } else { "beyond" },
        margin
    );
    Ok(ThrReport {
        format: THR_FORMAT,
        label: LABEL,
        sil_basis: SIL_BASIS,
        title: req.title.clone(),
        utc: utc.into(),
        thr_per_hour: req.thr_per_hour,
        contributions: c,
        total_per_hour: total,
        margin_decades: margin,
        sigma_for_tenth_m: sigma_tenth,
        within,
        verdict,
    })
}

/// The THR page.
pub fn thr_html(r: &ThrReport) -> String {
    let rows = r
        .contributions
        .iter()
        .map(|c| {
            vec![
                c.name.clone(),
                format!("{:.3e}", c.per_hour),
                format!("{:.3e}", c.share_of_thr),
                c.basis.clone(),
            ]
        })
        .collect();
    super::page(
        &format!("Rail position hazard against THR: {}", r.title),
        r.label,
        &r.verdict,
        &[r.sil_basis.into()],
        &[(
            "Contributions".into(),
            vec!["Contribution", "Per hour", "Share of THR", "Basis"],
            rows,
        )],
    )
}

// ---------------------------------------------------------------------------------------
// Virtual balise passages.

/// The balise request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaliseRequest {
    pub title: String,
    /// CSV: `balise` and either `truth_time,reported_time,speed_mps` or
    /// `truth_pos_m,reported_pos_m`; optional `sigma_m` (claimed along-track sigma).
    pub passages: String,
    pub alert_limit_m: Option<f64>,
}

/// The balise report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BaliseReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub utc: String,
    pub passages: usize,
    pub balises: usize,
    pub mean_m: f64,
    pub sd_m: f64,
    /// Expanded uncertainty (k = 2) of the mean error.
    pub u_mean_k2_m: f64,
    pub p95_abs_m: Option<f64>,
    pub max_abs_m: Option<f64>,
    pub alert_limit_m: Option<f64>,
    pub exceedances: Option<usize>,
    pub exceedance_upper95: Option<f64>,
    pub overbound: Option<Overbound>,
    pub verdict: String,
}

/// The virtual-balise passage test (paths relative to `base`).
pub fn balise(req: &BaliseRequest, base: &Path, utc: &str) -> Result<BaliseReport, String> {
    let t = Table::read(&base.join(&req.passages))?;
    let cb = t.need("balise")?;
    let by_pos = t.col("truth_pos_m").is_some();
    let mut err = Vec::new();
    let mut z = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    for i in 0..t.rows.len() {
        let e = if by_pos {
            match (
                t.num(i, t.col("truth_pos_m"))?,
                t.num(i, t.col("reported_pos_m"))?,
            ) {
                (Some(a), Some(b)) => b - a,
                _ => continue,
            }
        } else {
            match (
                t.num(i, Some(t.need("truth_time")?))?,
                t.num(i, Some(t.need("reported_time")?))?,
                t.num(i, Some(t.need("speed_mps")?))?,
            ) {
                // Reported late means the train had passed: the position lagged by v·Δt.
                (Some(a), Some(b), Some(v)) => v * (b - a),
                _ => continue,
            }
        };
        ids.insert(t.text(i, Some(cb)).to_string());
        if let Some(s) = t.num(i, t.col("sigma_m"))? {
            if s > 0.0 {
                z.push(e / s);
            }
        }
        err.push(e);
    }
    let n = err.len();
    if n < 2 {
        return Err("at least two passages".into());
    }
    let m = mean(&err).unwrap_or(0.0);
    let sd = (err.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt();
    let abs: Vec<f64> = err.iter().map(|x| x.abs()).collect();
    let exc = req
        .alert_limit_m
        .map(|l| abs.iter().filter(|x| **x > l).count());
    let ob = (!z.is_empty()).then(|| integrity::overbound(&z));
    let verdict = format!(
        "{n} passages over {} balise(s): mean {m:+.2} m ± {:.2} m (k = 2), sd {sd:.2} m, |error| p95 {}{}{}",
        ids.len(),
        2.0 * sd / (n as f64).sqrt(),
        super::fo(quantile(&abs, 0.95)),
        match (exc, req.alert_limit_m) {
            (Some(k), Some(l)) => format!("; {k} beyond {l} m"),
            _ => String::new(),
        },
        match &ob {
            Some(o) if o.bounded => "; claimed sigma overbounds the errors".to_string(),
            Some(o) => format!("; claimed sigma does not overbound (inflate by {:.2})", o.inflation_needed),
            None => String::new(),
        }
    );
    Ok(BaliseReport {
        format: BALISE_FORMAT,
        label: LABEL,
        title: req.title.clone(),
        utc: utc.into(),
        passages: n,
        balises: ids.len(),
        mean_m: m,
        sd_m: sd,
        u_mean_k2_m: 2.0 * sd / (n as f64).sqrt(),
        p95_abs_m: quantile(&abs, 0.95),
        max_abs_m: abs.iter().copied().reduce(f64::max),
        alert_limit_m: req.alert_limit_m,
        exceedances: exc,
        exceedance_upper95: exc.map(|k| cp95(k, n).1),
        overbound: ob,
        verdict,
    })
}

/// The balise page.
pub fn balise_html(r: &BaliseReport) -> String {
    let rows = r
        .overbound
        .as_ref()
        .map(|o| {
            o.points
                .iter()
                .map(|p| {
                    vec![
                        format!("{:.1}", p.k),
                        format!("{:.3e}", p.model),
                        format!("{:.3e}", p.observed),
                        format!("{:.3e}", p.observed_lower95),
                        if p.bounded { "yes" } else { "no" }.into(),
                    ]
                })
                .collect()
        })
        .unwrap_or_default();
    super::page(
        &format!("Virtual balise passage test: {}", r.title),
        r.label,
        &r.verdict,
        &[],
        &[(
            "Overbound check (normalised by the claimed sigma)".into(),
            vec![
                "k",
                "Model P(|z|>k)",
                "Observed",
                "Observed 95 % lower",
                "Bounded",
            ],
            rows,
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_identity_and_known() {
        let a = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        let i = invert(&a).unwrap();
        assert!((i[0][0] - 0.6).abs() < 1e-12 && (i[0][1] + 0.7).abs() < 1e-12);
        assert!(invert(&[vec![1.0, 2.0], vec![2.0, 4.0]]).is_none());
    }

    #[test]
    fn jamming_term_dominates_when_unflagged() {
        let req = ThrRequest {
            title: "t".into(),
            thr_per_hour: 1e-9,
            alert_limit_m: 20.0,
            sigma_m: 2.0,
            bias_m: 0.0,
            samples_per_hour: 60.0,
            fault_per_hour: 1e-5,
            fault_p_md: 1e-5,
            jamming: Some(Interference {
                rate_per_hour: 0.01,
                p_undetected: 1e-3,
                error_sigma_m: Some(30.0),
                p_exceed: None,
            }),
            spoofing: None,
        };
        let r = thr(&req, "2026-11-20T00:00:00Z").unwrap();
        assert!(!r.within);
        assert_eq!(r.contributions[2].name, "jamming");
        assert!(r.contributions[2].per_hour > r.contributions[0].per_hour);
        // Nominal at 10 sigma: 60 * 2 * Phi(-10) ≈ 9.2e-22.
        assert!(r.contributions[0].per_hour < 1e-20);
        let s = r.sigma_for_tenth_m.unwrap();
        assert!(s > 2.0 && s < 4.0, "{s}");
    }
}
