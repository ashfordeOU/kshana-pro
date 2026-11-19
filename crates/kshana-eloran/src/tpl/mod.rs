// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! # ASF-aware timing protection level (TPL) for UTC(k) time transfer from eLoran
//!
//! (Patent candidate E2; counsel decides filings. Do not publish.)
//!
//! A timing user at a known position estimates its clock against UTC(k) from the stations
//! steered to that UTC(k). Each station's pseudo-delay is predicted with the **nominal**
//! groundwave model (primary factor plus the nominal secondary factor and ASF); what is left
//! is the error of that prediction plus receiver noise and transmitter steering residual.
//! This module turns the propagation uncertainty into a bound on the resulting time error
//! at a stated integrity risk.
//!
//! ## Method
//!
//! 1. **Propagation uncertainty** ([`crate::prop::uncertainty`]): a seeded Monte Carlo over
//!    ground-conductivity class (per-class log-normal spread of the P.527/P.832 class,
//!    **one draw per class shared by every path crossing it**), permittivity, effective
//!    earth radius factor, terrain-height noise (with a DEM) and the seasonal land
//!    conductivity change with the season unknown. Each sample gives the groundwave delay
//!    error `e_i = delay_i − nominal_i` of every station, jointly.
//! 2. **Covariance from shared ground classes**: the sample covariance `C_asf` of the `e_i`.
//!    Stations whose paths cross the same mis-estimated class get correlated errors, which
//!    averaging cannot remove. `C = C_asf + diag(σ_meas² + σ_tx²)`.
//! 3. **Timing solution**: generalised least squares for one clock offset,
//!    `w = C⁻¹1 / (1ᵀC⁻¹1)`; the time error is `wᵀe`.
//! 4. **Overbound**: the weighted ASF error samples `z = wᵀe` get a Gaussian overbound
//!    `N(μ, σ_ob²)`: `σ_ob ≥` the sample sd and covers every empirical tail quantile beyond
//!    one sigma (`|z₍q₎ − μ| ≤ σ_ob·|Φ⁻¹(q)|`); the noise terms are added in quadrature.
//! 5. **Hypotheses** (integrity risk `P_HMI` split equally over them):
//!    * H0, fault-free: `TPL₀ = K(P₀)·σ + |μ|`, `K(p) = Φ⁻¹(1 − p/2)`;
//!    * H_i, station `i` faulty or biased (prior `p_station_fault`): solution separation
//!      against the subset solution without `i`, threshold `T_i = K(P_FA/n)·σ_Δi + |μ_Δi|`,
//!      `TPL_i = T_i + K(P_md,i)·σ⁽ⁱ⁾ + |μ⁽ⁱ⁾|`, `P_md,i = P_alloc / p_station_fault`;
//!    * H_ASF, ASF model error (prior `p_asf_model_fault`): the class spreads widened by
//!      `asf_model_error_log10` in quadrature (a mis-mapped atlas class), a common-mode
//!      error no solution separation can see: `TPL_ASF = K(P_md,ASF)·σ_ASF + |μ_ASF|`.
//!
//!    `TPL = max` over the hypotheses. With fewer than two stations a station fault cannot
//!    be monitored: unless its prior is within the allocation, the TPL is unavailable.
//! 6. **Availability**: `TPL ≤ alert limit` ([`availability_map`] over a grid).
//!
//! ## Evidence label: MODELLED
//!
//! Checked by internal Monte Carlo coverage tests ([`tests`]): over independent randomised
//! conductivity draws (a different seed from the one the bound is built from), the time
//! error exceeds the bound no more often than the stated rate, by a one-sided binomial test
//! with a fixed seed. Limits: the input distributions (class spreads, seasonal fraction,
//! model-error width, priors) are assumptions, not fitted to measured ASF; the Gaussian tail
//! is extrapolated beyond the sampled quantiles (a 10⁻⁷ risk is far beyond any sample size
//! here), so operational risks rest on that assumption; temporal ASF other than the seasonal
//! conductivity term (diurnal, weather) and skywave are not in the bound. No external data.

use crate::chain::Station;
use crate::geo::Geo;
use crate::prop::conductivity::ConductivityMap;
use crate::prop::uncertainty::{Engine, Problem, UncertaintyConfig};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Configuration of the TPL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TplConfig {
    /// Integrity risk per estimate (probability of hazardously misleading information).
    pub integrity_risk: f64,
    /// Total false-alert probability of the station-fault monitor.
    pub false_alert: f64,
    /// Prior probability that a given station is faulty or biased.
    pub p_station_fault: f64,
    /// Prior probability of an ASF model error (atlas class wrong beyond its spread).
    pub p_asf_model_fault: f64,
    /// Under H_ASF: extra sd of log₁₀ σ added in quadrature to every class.
    pub asf_model_error_log10: f64,
    /// Receiver measurement noise per station (ns, 1σ, after averaging).
    pub meas_sigma_ns: f64,
    /// Transmitter steering residual to UTC(k) per station (ns, 1σ).
    pub tx_sigma_ns: f64,
    /// Seasonal relative land conductivity variation (season unknown).
    pub seasonal_sigma_frac: f64,
    /// Monte Carlo samples and seed for building the bound.
    pub samples: usize,
    pub seed: u64,
    /// Other propagation uncertainties (k-factor, terrain, class overrides).
    pub uncertainty: UncertaintyConfig,
    /// Time-transfer alert limit (ns).
    pub alert_limit_ns: f64,
    /// Use only stations steered to this time reference (default: all).
    pub time_ref: Option<String>,
}

impl Default for TplConfig {
    fn default() -> Self {
        TplConfig {
            integrity_risk: 1e-7,
            false_alert: 1e-5,
            p_station_fault: 1e-5,
            p_asf_model_fault: 1e-4,
            asf_model_error_log10: 0.3,
            meas_sigma_ns: 5.0,
            tx_sigma_ns: 5.0,
            seasonal_sigma_frac: 0.15,
            samples: 1000,
            seed: 1,
            uncertainty: UncertaintyConfig {
                sky_height_sd_km: 0.0,
                ..Default::default()
            },
            alert_limit_ns: 100.0,
            time_ref: None,
        }
    }
}

impl TplConfig {
    fn validate(&self) -> Result<(), String> {
        for (n, p) in [
            ("integrity_risk", self.integrity_risk),
            ("false_alert", self.false_alert),
            ("p_station_fault", self.p_station_fault),
            ("p_asf_model_fault", self.p_asf_model_fault),
        ] {
            if !(p > 0.0 && p < 1.0) {
                return Err(format!("tpl: {n} must be in (0, 1)"));
            }
        }
        for (n, v) in [
            ("meas_sigma_ns", self.meas_sigma_ns),
            ("tx_sigma_ns", self.tx_sigma_ns),
            ("asf_model_error_log10", self.asf_model_error_log10),
        ] {
            if !(v >= 0.0 && v.is_finite()) {
                return Err(format!("tpl: {n} must be finite and ≥ 0"));
            }
        }
        if self.samples < 50 {
            return Err("tpl: at least 50 samples".into());
        }
        Ok(())
    }

    fn engine_cfg(&self, seed: u64, extra_log10: f64) -> UncertaintyConfig {
        UncertaintyConfig {
            samples: self.samples,
            seed,
            seasonal_sigma_frac: self.seasonal_sigma_frac,
            sigma_log10_extra_sd: extra_log10,
            ..self.uncertainty.clone()
        }
    }
}

/// Standard normal CDF.
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

/// Complementary error function (Numerical Recipes `erfcc`, |rel. error| < 1.2e-7),
/// refined by one Newton step where used in [`norm_quantile`].
fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 1.0 / (1.0 + 0.5 * z);
    let r = t
        * (-z * z - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87
                                        + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 {
        r
    } else {
        2.0 - r
    }
}

/// Standard normal quantile Φ⁻¹(p) (Acklam's rational approximation, |rel. error| <
/// 1.2e-9 before rounding of the CDF used to refine it).
pub fn norm_quantile(p: f64) -> f64 {
    assert!(p > 0.0 && p < 1.0, "norm_quantile: p in (0, 1)");
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
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
        -norm_quantile(1.0 - p)
    }
}

/// Two-sided multiplier `K(p) = Φ⁻¹(1 − p/2)` (0 when `p ≥ 1`).
pub fn k_two_sided(p: f64) -> f64 {
    if p >= 1.0 {
        0.0
    } else {
        norm_quantile(1.0 - p / 2.0)
    }
}

/// Gaussian overbound of samples: (mean, σ_ob, inflation σ_ob / sample sd).
pub fn overbound(z: &[f64]) -> (f64, f64, f64) {
    let n = z.len();
    if n < 2 {
        return (z.first().copied().unwrap_or(0.0), 0.0, 1.0);
    }
    let mu = z.iter().sum::<f64>() / n as f64;
    let sd = (z.iter().map(|x| (x - mu).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt();
    let mut s: Vec<f64> = z.to_vec();
    s.sort_by(f64::total_cmp);
    let mut ob = sd;
    for (k, v) in s.iter().enumerate() {
        let q = (k as f64 + 0.5) / n as f64;
        let zq = norm_quantile(q);
        if zq.abs() >= 1.0 {
            ob = ob.max((v - mu) / zq);
        }
    }
    (mu, ob, if sd > 0.0 { ob / sd } else { 1.0 })
}

/// Solve `A x = b` (small dense, partial pivoting); `None` when singular.
fn solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let mut m: Vec<Vec<f64>> = a
        .iter()
        .zip(b)
        .map(|(r, &bi)| {
            let mut r = r.clone();
            r.push(bi);
            r
        })
        .collect();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[p][c].abs() < 1e-300 {
            return None;
        }
        m.swap(c, p);
        for r in 0..n {
            if r != c {
                let f = m[r][c] / m[c][c];
                for k in c..=n {
                    m[r][k] -= f * m[c][k];
                }
            }
        }
    }
    Some((0..n).map(|i| m[i][n] / m[i][i]).collect())
}

/// GLS weights of a common offset over the stations in `subset` (others get weight 0).
pub fn gls_weights(c: &[Vec<f64>], subset: &[usize]) -> Option<Vec<f64>> {
    let cs: Vec<Vec<f64>> = subset
        .iter()
        .map(|&i| subset.iter().map(|&j| c[i][j]).collect())
        .collect();
    let u = solve(&cs, &vec![1.0; subset.len()])?;
    let s: f64 = u.iter().sum();
    if !(s > 0.0) {
        return None;
    }
    let mut w = vec![0.0; c.len()];
    for (k, &i) in subset.iter().enumerate() {
        w[i] = u[k] / s;
    }
    Some(w)
}

/// Joint groundwave delay error samples of the stations at one point: `[sample][station]`.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorSamples {
    pub nominal_delay_ns: Vec<f64>,
    pub nominal_asf_ns: Vec<f64>,
    pub e: Vec<Vec<f64>>,
}

impl ErrorSamples {
    fn n(&self) -> usize {
        self.nominal_delay_ns.len()
    }
    pub fn mean(&self) -> Vec<f64> {
        let m = self.e.len().max(1) as f64;
        (0..self.n())
            .map(|i| self.e.iter().map(|s| s[i]).sum::<f64>() / m)
            .collect()
    }
    pub fn covariance(&self) -> Vec<Vec<f64>> {
        let mu = self.mean();
        let m = (self.e.len().max(2) - 1) as f64;
        let n = self.n();
        let mut c = vec![vec![0.0; n]; n];
        for s in &self.e {
            for i in 0..n {
                for j in 0..n {
                    c[i][j] += (s[i] - mu[i]) * (s[j] - mu[j]) / m;
                }
            }
        }
        c
    }
    /// Weighted samples `wᵀe`.
    pub fn weighted(&self, w: &[f64]) -> Vec<f64> {
        self.e
            .iter()
            .map(|s| s.iter().zip(w).map(|(a, b)| a * b).sum())
            .collect()
    }
}

/// Monte Carlo error samples at every point (`[point]`), from one engine run.
pub fn error_samples(
    stations: &[Station],
    points: &[Geo],
    map: &ConductivityMap,
    k_factor: f64,
    sample_km: f64,
    ucfg: UncertaintyConfig,
) -> Result<Vec<ErrorSamples>, String> {
    let mut problem = Problem::new(stations.to_vec(), points.to_vec(), map, k_factor);
    problem.sample_km = sample_km;
    let eng = Engine::new(problem, ucfg)?;
    let (nom, samples) = eng.joint_samples();
    Ok((0..points.len())
        .map(|p| {
            let idx: Vec<usize> = (0..stations.len()).map(|s| eng.path_index(s, p)).collect();
            ErrorSamples {
                nominal_delay_ns: idx.iter().map(|&k| nom[k][0]).collect(),
                nominal_asf_ns: idx.iter().map(|&k| nom[k][1]).collect(),
                e: samples
                    .iter()
                    .map(|s| idx.iter().map(|&k| s[k][0] - nom[k][0]).collect())
                    .collect(),
            }
        })
        .collect())
}

/// Per-station terms of a TPL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationTerm {
    pub station: String,
    pub time_ref: String,
    pub nominal_asf_ns: f64,
    /// Groundwave delay error statistics (ns) under H0.
    pub asf_bias_ns: f64,
    pub asf_sigma_ns: f64,
    pub weight: f64,
}

/// One hypothesis' contribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub name: String,
    pub prior: f64,
    /// Allowed probability of a missed (or, for H0, any) exceedance under it.
    pub p_md: f64,
    /// Detection threshold (station hypotheses).
    pub threshold_ns: Option<f64>,
    pub sigma_ns: f64,
    pub bias_ns: f64,
    pub tpl_ns: f64,
}

/// The TPL at one point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TplResult {
    pub point: Geo,
    pub stations: Vec<StationTerm>,
    /// Correlation matrix of the stations' delay errors (from shared ground classes).
    pub correlation: Vec<Vec<f64>>,
    /// Fault-free solution: σ (overbound, noise included) and bias.
    pub sigma_ns: f64,
    pub bias_ns: f64,
    pub overbound_inflation: f64,
    pub hypotheses: Vec<Hypothesis>,
    pub tpl_ns: f64,
    pub alert_limit_ns: f64,
    pub available: bool,
    pub integrity_risk: f64,
    pub evidence_label: String,
}

impl TplResult {
    pub fn hypothesis(&self, name: &str) -> Option<&Hypothesis> {
        self.hypotheses.iter().find(|h| h.name == name)
    }
}

/// σ and bias of `wᵀe` plus the per-station noise: Gaussian overbound of the ASF part.
fn combined(s: &ErrorSamples, w: &[f64], noise_var: f64) -> (f64, f64, f64) {
    let (mu, ob, infl) = overbound(&s.weighted(w));
    let nv: f64 = w.iter().map(|x| x * x * noise_var).sum();
    (mu, (ob * ob + nv).sqrt(), infl)
}

/// The monitor and TPL derived from H0 samples: weights, thresholds, per-hypothesis terms.
#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub w: Vec<f64>,
    /// Subset weights without station i.
    pub w_sub: Vec<Option<Vec<f64>>>,
    pub thresholds: Vec<f64>,
}

impl Monitor {
    /// Station alarms for one set of errors (`true` = separation beyond threshold).
    pub fn alarms(&self, e: &[f64]) -> Vec<bool> {
        self.w_sub
            .iter()
            .zip(&self.thresholds)
            .map(|(ws, t)| match ws {
                Some(ws) => {
                    let d: f64 = self
                        .w
                        .iter()
                        .zip(ws)
                        .zip(e)
                        .map(|((a, b), x)| (a - b) * x)
                        .sum();
                    d.abs() > *t
                }
                None => false,
            })
            .collect()
    }
    /// Time error of the full solution.
    pub fn error(&self, e: &[f64]) -> f64 {
        self.w.iter().zip(e).map(|(a, b)| a * b).sum()
    }
}

/// TPL from H0 and H_ASF samples (the core; `ids` = (station id, time ref)).
pub fn tpl_from_samples(
    point: Geo,
    ids: &[(String, String)],
    h0: &ErrorSamples,
    hasf: &ErrorSamples,
    cfg: &TplConfig,
) -> Result<(TplResult, Monitor), String> {
    let n = h0.n();
    if n == 0 {
        return Err("tpl: no stations".into());
    }
    let noise_var = cfg.meas_sigma_ns.powi(2) + cfg.tx_sigma_ns.powi(2);
    let mut c = h0.covariance();
    for (i, r) in c.iter_mut().enumerate() {
        r[i] += noise_var;
    }
    let all: Vec<usize> = (0..n).collect();
    let w = gls_weights(&c, &all).ok_or("tpl: singular covariance")?;
    let n_hyp = n + 2;
    let alloc = cfg.integrity_risk / n_hyp as f64;
    let (mu0, s0, infl) = combined(h0, &w, noise_var);
    let mut hyps = vec![Hypothesis {
        name: "H0 fault-free".into(),
        prior: 1.0,
        p_md: alloc,
        threshold_ns: None,
        sigma_ns: s0,
        bias_ns: mu0,
        tpl_ns: k_two_sided(alloc) * s0 + mu0.abs(),
    }];
    let p_md_st = (alloc / cfg.p_station_fault).min(1.0);
    let k_fa = k_two_sided(cfg.false_alert / n as f64);
    let mut w_sub = Vec::new();
    let mut thresholds = Vec::new();
    for i in 0..n {
        let subset: Vec<usize> = (0..n).filter(|&j| j != i).collect();
        let ws = if subset.is_empty() {
            None
        } else {
            gls_weights(&c, &subset)
        };
        let (tpl, thr, sig, bias) = match &ws {
            Some(ws) => {
                let dw: Vec<f64> = w.iter().zip(ws).map(|(a, b)| a - b).collect();
                let (md, sd, _) = combined(h0, &dw, noise_var);
                let t = k_fa * sd + md.abs();
                let (mi, si, _) = combined(h0, ws, noise_var);
                (t + k_two_sided(p_md_st) * si + mi.abs(), t, si, mi)
            }
            // a single station cannot be monitored: the hypothesis must fit the budget
            None if cfg.p_station_fault <= alloc => (0.0, f64::NAN, 0.0, 0.0),
            None => (f64::INFINITY, f64::NAN, f64::NAN, f64::NAN),
        };
        hyps.push(Hypothesis {
            name: format!("H{} station {} faulty", i + 1, ids[i].0),
            prior: cfg.p_station_fault,
            p_md: p_md_st,
            threshold_ns: thr.is_finite().then_some(thr),
            sigma_ns: sig,
            bias_ns: bias,
            tpl_ns: tpl,
        });
        thresholds.push(thr);
        w_sub.push(ws);
    }
    let p_md_asf = (alloc / cfg.p_asf_model_fault).min(1.0);
    let (ma, sa, _) = combined(hasf, &w, noise_var);
    hyps.push(Hypothesis {
        name: "H_ASF model error".into(),
        prior: cfg.p_asf_model_fault,
        p_md: p_md_asf,
        threshold_ns: None,
        sigma_ns: sa,
        bias_ns: ma,
        tpl_ns: k_two_sided(p_md_asf) * sa + ma.abs(),
    });
    let tpl = hyps.iter().map(|h| h.tpl_ns).fold(0.0, f64::max);
    let mu = h0.mean();
    let c_asf = h0.covariance();
    let corr = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    let d = (c_asf[i][i] * c_asf[j][j]).sqrt();
                    if d > 0.0 {
                        c_asf[i][j] / d
                    } else if i == j {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect();
    let stations = (0..n)
        .map(|i| StationTerm {
            station: ids[i].0.clone(),
            time_ref: ids[i].1.clone(),
            nominal_asf_ns: h0.nominal_asf_ns[i],
            asf_bias_ns: mu[i],
            asf_sigma_ns: c_asf[i][i].sqrt(),
            weight: w[i],
        })
        .collect();
    Ok((
        TplResult {
            point,
            stations,
            correlation: corr,
            sigma_ns: s0,
            bias_ns: mu0,
            overbound_inflation: infl,
            hypotheses: hyps,
            tpl_ns: tpl,
            alert_limit_ns: cfg.alert_limit_ns,
            available: tpl <= cfg.alert_limit_ns,
            integrity_risk: cfg.integrity_risk,
            evidence_label: "MODELLED: Monte Carlo propagation uncertainty (assumed input \
                             distributions), Gaussian tail extrapolation; no external data"
                .into(),
        },
        Monitor {
            w,
            w_sub,
            thresholds,
        },
    ))
}

/// What the TPL is computed for.
pub struct TplProblem<'a> {
    pub stations: Vec<Station>,
    pub points: Vec<Geo>,
    pub map: &'a ConductivityMap,
    pub k_factor: f64,
    pub sample_km: f64,
}

/// The TPL at every point (with each point's monitor).
pub fn compute(p: &TplProblem, cfg: &TplConfig) -> Result<Vec<(TplResult, Monitor)>, String> {
    cfg.validate()?;
    let stations: Vec<Station> = p
        .stations
        .iter()
        .filter(|s| cfg.time_ref.as_ref().is_none_or(|t| &s.time_ref == t))
        .cloned()
        .collect();
    if stations.is_empty() {
        return Err("tpl: no station steered to the requested time reference".into());
    }
    if p.points.is_empty() {
        return Err("tpl: no points".into());
    }
    let ids: Vec<(String, String)> = stations
        .iter()
        .map(|s| (s.id.clone(), s.time_ref.clone()))
        .collect();
    let h0 = error_samples(
        &stations,
        &p.points,
        p.map,
        p.k_factor,
        p.sample_km,
        cfg.engine_cfg(cfg.seed, 0.0),
    )?;
    let ha = error_samples(
        &stations,
        &p.points,
        p.map,
        p.k_factor,
        p.sample_km,
        cfg.engine_cfg(cfg.seed ^ 0x5EED_A5F0, cfg.asf_model_error_log10),
    )?;
    p.points
        .iter()
        .enumerate()
        .map(|(i, pt)| tpl_from_samples(*pt, &ids, &h0[i], &ha[i], cfg))
        .collect()
}

/// One grid point of an availability map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapPoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub tpl_ns: f64,
    pub available: bool,
}

/// TPL availability over a grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AvailabilityMap {
    pub alert_limit_ns: f64,
    pub integrity_risk: f64,
    pub points: Vec<MapPoint>,
    /// Fraction of grid points with TPL ≤ alert limit.
    pub availability: f64,
}

/// TPL and availability on the grid `[lat1, lat2] × [lon1, lon2]` at `res_deg` (one Monte
/// Carlo run covers every point; at most 2 500 points).
pub fn availability_map(
    stations: &[Station],
    bbox: [f64; 4],
    res_deg: f64,
    map: &ConductivityMap,
    k_factor: f64,
    sample_km: f64,
    cfg: &TplConfig,
) -> Result<AvailabilityMap, String> {
    let [la1, la2, lo1, lo2] = bbox;
    if !(res_deg > 0.0) || !(la2 >= la1) || !(lo2 >= lo1) {
        return Err("tpl map: box lat1 ≤ lat2, lon1 ≤ lon2, res > 0".into());
    }
    let nl = ((la2 - la1) / res_deg).round() as usize + 1;
    let no = ((lo2 - lo1) / res_deg).round() as usize + 1;
    if nl * no > 2500 {
        return Err(format!("tpl map: {} points (max 2500)", nl * no));
    }
    let mut points = Vec::new();
    for i in 0..nl {
        for j in 0..no {
            points.push(Geo::new(la1 + i as f64 * res_deg, lo1 + j as f64 * res_deg));
        }
    }
    let r = compute(
        &TplProblem {
            stations: stations.to_vec(),
            points,
            map,
            k_factor,
            sample_km,
        },
        cfg,
    )?;
    let pts: Vec<MapPoint> = r
        .iter()
        .map(|(t, _)| MapPoint {
            lat_deg: t.point.lat_deg,
            lon_deg: t.point.lon_deg,
            tpl_ns: t.tpl_ns,
            available: t.available,
        })
        .collect();
    let a = pts.iter().filter(|p| p.available).count() as f64 / pts.len() as f64;
    Ok(AvailabilityMap {
        alert_limit_ns: cfg.alert_limit_ns,
        integrity_risk: cfg.integrity_risk,
        points: pts,
        availability: a,
    })
}

/// Upper critical value of a binomial test: the largest `k` with
/// `P(X ≥ k) > alpha` for `X ~ Bin(n, p)`; observing more than it rejects "rate ≤ p" at
/// level `alpha` (one-sided).
pub fn binomial_critical(n: usize, p: f64, alpha: f64) -> usize {
    // pmf by recurrence in log space
    let lp = p.ln();
    let lq = (1.0 - p).ln();
    let mut logpmf = vec![0.0; n + 1];
    let mut lc = 0.0; // ln C(n, k)
    for (k, v) in logpmf.iter_mut().enumerate() {
        if k > 0 {
            lc += ((n - k + 1) as f64).ln() - (k as f64).ln();
        }
        *v = lc + k as f64 * lp + (n - k) as f64 * lq;
    }
    let mut tail = 0.0; // P(X ≥ k), accumulated from the top
    for k in (0..=n).rev() {
        tail += logpmf[k].exp();
        if tail > alpha {
            return k;
        }
    }
    0
}
