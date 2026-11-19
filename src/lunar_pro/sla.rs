// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # D7. Lunar navigation service (LNSP) SLA verification (gaps 16, 25)
//!
//! An independent monthly check of a commercial lunar navigation service against its service
//! level agreement, from a reference station's observations. Per provider it estimates the
//! signal-in-space error (SISE) in the range domain, the user-equipment error (UEE) it removes,
//! the availability and the continuity, each with a confidence interval, and gives a three-way
//! verdict per target: **MEETS** (the whole interval is inside the target), **BREACH** (the whole
//! interval is outside it) or **INCONCLUSIVE**. The report is written to a directory and signed
//! with the repository's Ed25519 dossier signature ([`crate::signed_dossier`]).
//!
//! ## Input (one CSV per month or more; other months are ignored)
//! `time,station,provider,node,status,range_residual_m,uee_sigma_m`, with `time` in
//! `YYYY-MM-DDTHH:MM:SS` (any one time scale, used consistently), `status` one of
//! * `ok`: the node was tracked and flagged healthy; `range_residual_m` is the pseudorange minus
//!   the geometric range from the node's **broadcast** ephemeris and clock to the surveyed station,
//!   with the station's own clock offset (from its reference clock) removed: SISE plus UEE;
//! * `unhealthy`: tracked but flagged unhealthy by the provider (unavailable, not an integrity
//!   event);
//! * `missing`: scheduled in view but not received (unavailable).
//!
//! `uee_sigma_m` is the station's own receiver noise and multipath (1σ) from its calibration.
//! Forming the residual needs the station's surveyed position and clock; this module starts from
//! the residuals.
//!
//! ## Estimators
//! * **k-sigma SISE** (`ksigma`, e.g. 3σ): `k·RMS_SISE`, `RMS_SISE² = mean(e²) − mean(σ_UEE²)`
//!   (bias included, so conservative); interval from the χ² distribution with `n_eff` degrees of
//!   freedom (Wilson–Hilferty).
//! * **Percentile SISE** (`percentile`, e.g. 95 %): the `p` quantile of `|e|`, corrected for UEE
//!   under a Gaussian assumption `q_SISE = √(q² − z_p²·σ_UEE²)`; distribution-free interval from
//!   the order statistics of `n_eff` independent samples.
//! * `n_eff`: samples are correlated in time (orbit and clock errors drift), so the count of
//!   independent samples is the number of distinct `decorrelation_s` blocks with data.
//! * **Window**: with `window_h` (e.g. Moonlight's "any 24 h"), the metric is evaluated on sliding
//!   windows (step `window_h/24`) and the worst window is reported and judged.
//! * **Availability**: the fraction of the provider's scheduled epochs at which at least
//!   `min_nodes` nodes are `ok` with `|e| ≤ alarm_limit_m`; Wilson interval on `n_eff` blocks.
//! * **Integrity events**: `ok` samples with `|e| > alarm_limit_m` (counted, and unavailable).
//! * **Continuity**: unscheduled losses of service per hour of available service `λ`; the
//!   probability of no loss over `continuity_interval_s` is `exp(−λ·T)` (with a χ²-based
//!   interval on the Poisson count). The longest continuous available run per day is reported.
//!
//! ## Targets (configurable; the built-in profiles are as cited in the research note, 03 §1.3)
//! * `lcrns-srd`: SISE 13.43 m at 3σ (LCRNS SRD ESC-LCRNS-REQ-0090, source [28]). The SRD states
//!   a **position** SISE; a station measures the range-domain projection plus clock, so the
//!   comparison is **illustrative until confirmed against the SRD's definition**.
//! * `moonlight-owr`: OWR SISE 10 m at 95 % over any 24 h, availability 95 % (sources [29][30]).
//! * `lnss-ioc`: SISE 20 m at 2σ (source [17]).
//!
//! **Evidence label: MODELLED.** Checked on synthetic providers (one meeting, one breaching), on
//! the estimators' coverage by Monte Carlo, and on signature tamper detection. No lunar navigation
//! service exists yet, so nothing is VALIDATED.

use std::collections::{BTreeMap, BTreeSet};

use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use super::budget::std_normal;

/// The statement every output of this module carries.
pub const STATUS: &str = "MODELLED (Kshana Pro D7): SLA verification checked on synthetic \
    providers; no lunar navigation service exists yet; SRD/SLA clause mapping illustrative until \
    confirmed against the documents";

/// A SISE metric.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Metric {
    /// `k·RMS`.
    Ksigma { k: f64 },
    /// The `p` quantile of `|e|` (0 < p < 1).
    Percentile { p: f64 },
}

/// One SLA profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sla {
    pub id: String,
    pub source: String,
    pub sise_target_m: f64,
    pub sise_metric: Metric,
    /// Evaluate on sliding windows of this length and judge the worst.
    #[serde(default)]
    pub window_h: Option<f64>,
    #[serde(default)]
    pub availability_target: Option<f64>,
    #[serde(default)]
    pub continuity_target: Option<f64>,
    #[serde(default = "d_cont")]
    pub continuity_interval_s: f64,
    /// Nodes needed at once for the service to count as available.
    #[serde(default = "d_one")]
    pub min_nodes: usize,
    /// A healthy sample beyond this error is an integrity event (m).
    pub alarm_limit_m: f64,
    #[serde(default)]
    pub note: String,
}
fn d_cont() -> f64 {
    3600.0
}
fn d_one() -> usize {
    1
}

impl Sla {
    /// The built-in profiles.
    pub fn builtin(id: &str) -> Result<Sla, String> {
        match id {
            "lcrns-srd" => Ok(Sla {
                id: id.into(),
                source: "LCRNS SRD ESC-LCRNS-REQ-0090 as cited in research note 03 [28]: SISE position 13.43 m (3 sigma)".into(),
                sise_target_m: 13.43,
                sise_metric: Metric::Ksigma { k: 3.0 },
                window_h: None,
                availability_target: None,
                continuity_target: None,
                continuity_interval_s: 3600.0,
                min_nodes: 1,
                alarm_limit_m: 3.0 * 13.43,
                note: "range-domain estimate compared with a position SISE: illustrative until confirmed against the SRD".into(),
            }),
            "moonlight-owr" => Ok(Sla {
                id: id.into(),
                source: "ESA Moonlight LCNS as cited in research note 03 [29][30]: OWR SISE 10 m (95%, any 24 h), 95% availability".into(),
                sise_target_m: 10.0,
                sise_metric: Metric::Percentile { p: 0.95 },
                window_h: Some(24.0),
                availability_target: Some(0.95),
                continuity_target: None,
                continuity_interval_s: 3600.0,
                min_nodes: 1,
                alarm_limit_m: 30.0,
                note: "availability counted at one node; a positioning service needs min_nodes = 4".into(),
            }),
            "lnss-ioc" => Ok(Sla {
                id: id.into(),
                source: "JAXA LNSS as cited in research note 03 [17]: SISE 20 m (2 sigma) at IOC".into(),
                sise_target_m: 20.0,
                sise_metric: Metric::Ksigma { k: 2.0 },
                window_h: None,
                availability_target: None,
                continuity_target: None,
                continuity_interval_s: 3600.0,
                min_nodes: 1,
                alarm_limit_m: 60.0,
                note: String::new(),
            }),
            o => Err(format!("unknown SLA '{o}' (lcrns-srd, moonlight-owr, lnss-ioc, or --sla-file)")),
        }
    }

    fn check(&self) -> Result<(), String> {
        let ok = self.sise_target_m > 0.0
            && self.alarm_limit_m > 0.0
            && self.min_nodes >= 1
            && self.continuity_interval_s > 0.0
            && match self.sise_metric {
                Metric::Ksigma { k } => k > 0.0,
                Metric::Percentile { p } => p > 0.0 && p < 1.0,
            }
            && self.window_h.is_none_or(|w| w > 0.0)
            && self
                .availability_target
                .is_none_or(|a| (0.0..=1.0).contains(&a))
            && self
                .continuity_target
                .is_none_or(|a| (0.0..=1.0).contains(&a));
        if ok {
            Ok(())
        } else {
            Err(format!(
                "SLA {}: a target or parameter is out of range",
                self.id
            ))
        }
    }
}

/// A file of custom SLA profiles (`[[sla]]` tables).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlaFile {
    pub sla: Vec<Sla>,
}

/// Parse a custom SLA file.
pub fn parse_sla_file(text: &str) -> Result<Vec<Sla>, String> {
    let f: SlaFile = toml::from_str(text).map_err(|e| format!("SLA file: {e}"))?;
    for s in &f.sla {
        s.check()?;
    }
    Ok(f.sla)
}

/// Status of one sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Status {
    Ok,
    Unhealthy,
    Missing,
}

/// One reference-station sample.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Sample {
    /// Seconds since J2000 of `time`.
    pub t: f64,
    pub month: String,
    pub station: String,
    pub provider: String,
    pub node: String,
    pub status: Status,
    pub residual_m: Option<f64>,
    pub uee_sigma_m: f64,
}

/// Parse the observation CSV.
pub fn parse_samples(text: &str) -> Result<Vec<Sample>, String> {
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("time,") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let e = |m: &str| format!("line {}: {m}", i + 1);
        if f.len() != 7 {
            return Err(e(&format!("expected 7 fields, found {}", f.len())));
        }
        let ep = super::lugre::parse_epoch(f[0]).map_err(|x| e(&x))?;
        let status = match f[4] {
            "ok" => Status::Ok,
            "unhealthy" => Status::Unhealthy,
            "missing" => Status::Missing,
            s => return Err(e(&format!("status '{s}' is not ok, unhealthy or missing"))),
        };
        let num = |s: &str| -> Result<Option<f64>, String> {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map(Some)
                    .ok_or_else(|| e(&format!("bad number '{s}'")))
            }
        };
        let residual = num(f[5])?;
        if status == Status::Ok && residual.is_none() {
            return Err(e("an ok sample needs range_residual_m"));
        }
        let uee = num(f[6])?.unwrap_or(0.0);
        if uee < 0.0 {
            return Err(e("uee_sigma_m < 0"));
        }
        if f[2].is_empty() || f[3].is_empty() {
            return Err(e("empty provider or node"));
        }
        out.push(Sample {
            t: super::lunex::secs(&ep),
            month: format!("{:04}-{:02}", ep.year, ep.month),
            station: f[1].into(),
            provider: f[2].into(),
            node: f[3].into(),
            status,
            residual_m: residual,
            uee_sigma_m: uee,
        });
    }
    if out.is_empty() {
        return Err("no samples".into());
    }
    Ok(out)
}

// ------------------------------------------------------------------------------ statistics

#[allow(clippy::excessive_precision)]
fn norm_quantile(p: f64) -> f64 {
    // Acklam's rational approximation (|error| < 1.2e-9), refined by one Newton step.
    let a = [
        -3.969683028665376e1,
        2.209460984245205e2,
        -2.759285104469687e2,
        1.383577518672690e2,
        -3.066479806614716e1,
        2.506628277459239,
    ];
    let b = [
        -5.447609879822406e1,
        1.615858368580409e2,
        -1.556989798598866e2,
        6.680131188771972e1,
        -1.328068155288572e1,
    ];
    let c = [
        -7.784894002430293e-3,
        -3.223964580411365e-1,
        -2.400758277161838,
        -2.549732539343734,
        4.374664141464968,
        2.938163982698783,
    ];
    let d = [
        7.784695709041462e-3,
        3.224671290700398e-1,
        2.445134137142996,
        3.754408661907416,
    ];
    let pl = 0.02425;
    if p < pl {
        let q = (-2.0 * p.ln()).sqrt();
        (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5])
            / ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0)
    } else if p <= 1.0 - pl {
        let q = p - 0.5;
        let r = q * q;
        (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q
            / (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0)
    } else {
        -norm_quantile(1.0 - p)
    }
}

/// χ² quantile with `nu` degrees of freedom (Wilson–Hilferty).
fn chi2_quantile(p: f64, nu: f64) -> f64 {
    let z = norm_quantile(p);
    let h = 2.0 / (9.0 * nu);
    (nu * (1.0 - h + z * h.sqrt()).powi(3)).max(0.0)
}

/// Wilson interval of a proportion.
fn wilson(x: f64, n: f64, z: f64) -> [f64; 2] {
    if n <= 0.0 {
        return [0.0, 1.0];
    }
    let p = x / n;
    let d = 1.0 + z * z / n;
    let c = (p + z * z / (2.0 * n)) / d;
    let h = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    [(c - h).max(0.0), (c + h).min(1.0)]
}

/// A metric's estimate with its interval.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Estimate {
    pub value: f64,
    pub lo: f64,
    pub hi: f64,
    pub n: usize,
    pub n_eff: usize,
    /// How the interval was formed.
    pub method: String,
}

fn n_eff(ts: &[f64], decor_s: f64) -> usize {
    ts.iter()
        .map(|t| (t / decor_s).floor() as i64)
        .collect::<BTreeSet<_>>()
        .len()
}

/// Estimate a SISE metric from residuals `e` (with UEE σ and times), confidence `conf`.
pub fn estimate_sise(
    e: &[f64],
    uee: &[f64],
    ts: &[f64],
    metric: &Metric,
    decor_s: f64,
    conf: f64,
) -> Option<Estimate> {
    let n = e.len();
    if n < 2 {
        return None;
    }
    let ne = n_eff(ts, decor_s).max(1);
    let alpha = 1.0 - conf;
    let mean_u2 = uee.iter().map(|u| u * u).sum::<f64>() / n as f64;
    match *metric {
        Metric::Ksigma { k } => {
            let ms = e.iter().map(|x| x * x).sum::<f64>() / n as f64;
            let nu = ne as f64;
            let lo_ms = ms * nu / chi2_quantile(1.0 - alpha / 2.0, nu);
            let hi_ms = ms * nu / chi2_quantile(alpha / 2.0, nu);
            let f = |m: f64| k * (m - mean_u2).max(0.0).sqrt();
            Some(Estimate {
                value: f(ms),
                lo: f(lo_ms),
                hi: f(hi_ms),
                n,
                n_eff: ne,
                method: "chi-square on n_eff".into(),
            })
        }
        Metric::Percentile { p } => {
            let mut a: Vec<f64> = e.iter().map(|x| x.abs()).collect();
            a.sort_by(|x, y| x.total_cmp(y));
            let q = |pp: f64| {
                let r = (pp * (n - 1) as f64).clamp(0.0, (n - 1) as f64);
                let (i, fr) = (r.floor() as usize, r - r.floor());
                if i + 1 < n {
                    a[i] + fr * (a[i + 1] - a[i])
                } else {
                    a[n - 1]
                }
            };
            // Order-statistic interval on n_eff independent samples, mapped to ranks of n; when
            // n_eff is too small for a distribution-free bound (p ± half reaches 0 or 1), the
            // scale-family interval (shape known, scale from chi-square on n_eff) is used.
            let z = norm_quantile(1.0 - alpha / 2.0);
            let ner = ne as f64;
            let half = z * (ner * p * (1.0 - p)).sqrt() / ner;
            let zp = norm_quantile((1.0 + p) / 2.0);
            let corr = |v: f64| (v * v - zp * zp * mean_u2).max(0.0).sqrt();
            let qp = q(p);
            if p + half < 1.0 && p - half > 0.0 {
                Some(Estimate {
                    value: corr(qp),
                    lo: corr(q(p - half)),
                    hi: corr(q(p + half)),
                    n,
                    n_eff: ne,
                    method: "order statistics on n_eff (distribution-free)".into(),
                })
            } else {
                let lo = qp * (ner / chi2_quantile(1.0 - alpha / 2.0, ner)).sqrt();
                let hi = qp * (ner / chi2_quantile(alpha / 2.0, ner)).sqrt();
                Some(Estimate {
                    value: corr(qp),
                    lo: corr(lo),
                    hi: corr(hi),
                    n,
                    n_eff: ne,
                    method: "scale family, chi-square on n_eff (n_eff too small for a distribution-free bound)".into(),
                })
            }
        }
    }
}

/// A three-way verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Verdict {
    Meets,
    Breach,
    Inconclusive,
    /// No target, or not enough data.
    NotAssessed,
}

/// Verdict for an upper-bounded quantity (smaller is better).
fn verdict_upper(e: &Estimate, target: f64) -> Verdict {
    if e.hi <= target {
        Verdict::Meets
    } else if e.lo > target {
        Verdict::Breach
    } else {
        Verdict::Inconclusive
    }
}

/// Verdict for a lower-bounded quantity (larger is better).
fn verdict_lower(lo: f64, hi: f64, target: f64) -> Verdict {
    if lo >= target {
        Verdict::Meets
    } else if hi < target {
        Verdict::Breach
    } else {
        Verdict::Inconclusive
    }
}

// ------------------------------------------------------------------------------ analysis

/// Options of a monthly verification.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Options {
    pub month: String,
    pub decorrelation_s: f64,
    pub confidence: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            month: String::new(),
            decorrelation_s: 1800.0,
            confidence: 0.95,
        }
    }
}

/// Per-node figures.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeRow {
    pub node: String,
    pub samples: usize,
    pub ok: usize,
    pub mean_m: f64,
    pub rms_m: f64,
    pub uee_rms_m: f64,
    pub integrity_events: usize,
}

/// One SLA assessed for one provider.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SlaResult {
    pub sla: String,
    pub source: String,
    pub note: String,
    pub sise_metric: Metric,
    pub sise_target_m: f64,
    pub sise: Option<Estimate>,
    /// For a windowed SLA: the start of the worst window.
    pub worst_window_start: Option<String>,
    pub sise_verdict: Verdict,
    pub availability: f64,
    pub availability_interval: [f64; 2],
    pub availability_target: Option<f64>,
    pub availability_verdict: Verdict,
    pub losses_of_service: usize,
    pub available_hours: f64,
    pub loss_rate_per_h: f64,
    pub continuity: f64,
    pub continuity_interval: [f64; 2],
    pub continuity_interval_s: f64,
    pub continuity_target: Option<f64>,
    pub continuity_verdict: Verdict,
    pub longest_run_h_per_day: Vec<(String, f64)>,
    pub integrity_events: usize,
    pub verdict: Verdict,
}

/// One provider's month.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProviderReport {
    pub provider: String,
    pub stations: Vec<String>,
    pub nodes: Vec<NodeRow>,
    pub epochs: usize,
    pub results: Vec<SlaResult>,
}

/// The monthly report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonthlyReport {
    pub status: String,
    pub month: String,
    pub options: Options,
    pub input_sha256: String,
    pub samples_in_month: usize,
    pub samples_other_months: usize,
    pub providers: Vec<ProviderReport>,
}

fn iso(t: f64) -> String {
    let e = super::lugre::epoch_from_secs(t);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        e.year,
        e.month,
        e.day,
        e.hour,
        e.minute,
        e.second.floor() as u32
    )
}

fn assess(samples: &[&Sample], sla: &Sla, opt: &Options) -> SlaResult {
    let conf = opt.confidence;
    let z = norm_quantile(1.0 - (1.0 - conf) / 2.0);
    // Healthy, usable samples (integrity events excluded from the SISE statistic, counted).
    let healthy: Vec<&&Sample> = samples.iter().filter(|s| s.status == Status::Ok).collect();
    let events = healthy
        .iter()
        .filter(|s| s.residual_m.is_some_and(|e| e.abs() > sla.alarm_limit_m))
        .count();
    let pick = |v: &[&&Sample]| -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        (
            v.iter().map(|s| s.residual_m.unwrap_or(0.0)).collect(),
            v.iter().map(|s| s.uee_sigma_m).collect(),
            v.iter().map(|s| s.t).collect(),
        )
    };
    // SISE (integrity events stay in the SISE statistic: they are signal-in-space errors).
    let (sise, worst) = match sla.window_h {
        None => {
            let (e, u, t) = pick(&healthy);
            (
                estimate_sise(&e, &u, &t, &sla.sise_metric, opt.decorrelation_s, conf),
                None,
            )
        }
        Some(w) => {
            let ws = w * 3600.0;
            let step = ws / 24.0;
            let (t0, t1) = healthy
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), s| {
                    (a.min(s.t), b.max(s.t))
                });
            let mut best: Option<(Estimate, f64)> = None;
            let mut start = t0;
            while start + ws <= t1 + step * 0.5 && t1.is_finite() {
                let win: Vec<&&Sample> = healthy
                    .iter()
                    .filter(|s| s.t >= start && s.t < start + ws)
                    .copied()
                    .collect();
                let (e, u, t) = pick(&win);
                if let Some(est) =
                    estimate_sise(&e, &u, &t, &sla.sise_metric, opt.decorrelation_s, conf)
                {
                    if best.as_ref().is_none_or(|(b, _)| est.value > b.value) {
                        best = Some((est, start));
                    }
                }
                start += step;
            }
            match best {
                Some((e, s)) => (Some(e), Some(iso(s))),
                None => {
                    // Less than one window of data: the whole month.
                    let (e, u, t) = pick(&healthy);
                    (
                        estimate_sise(&e, &u, &t, &sla.sise_metric, opt.decorrelation_s, conf),
                        None,
                    )
                }
            }
        }
    };
    let sise_verdict = sise
        .as_ref()
        .map(|e| verdict_upper(e, sla.sise_target_m))
        .unwrap_or(Verdict::NotAssessed);
    // Availability per scheduled epoch.
    let mut by_t: BTreeMap<i64, usize> = BTreeMap::new();
    for s in samples {
        let k = (s.t * 1000.0).round() as i64;
        let usable =
            s.status == Status::Ok && s.residual_m.is_some_and(|e| e.abs() <= sla.alarm_limit_m);
        *by_t.entry(k).or_default() += usable as usize;
    }
    let epochs: Vec<(f64, bool)> = by_t
        .iter()
        .map(|(k, n)| (*k as f64 / 1000.0, *n >= sla.min_nodes))
        .collect();
    let n_ep = epochs.len().max(1) as f64;
    let n_av = epochs.iter().filter(|e| e.1).count() as f64;
    let availability = n_av / n_ep;
    let blocks = n_eff(
        &epochs.iter().map(|e| e.0).collect::<Vec<_>>(),
        opt.decorrelation_s,
    ) as f64;
    let ai = wilson(availability * blocks, blocks, z);
    let availability_verdict = sla
        .availability_target
        .map(|a| verdict_lower(ai[0], ai[1], a))
        .unwrap_or(Verdict::NotAssessed);
    // Continuity: losses of service (available → unavailable) per available hour.
    let step = epochs
        .windows(2)
        .map(|w| w[1].0 - w[0].0)
        .fold(f64::INFINITY, f64::min);
    let step = if step.is_finite() { step } else { 0.0 };
    let losses = epochs.windows(2).filter(|w| w[0].1 && !w[1].1).count();
    let avail_h = n_av * step / 3600.0;
    let lam = if avail_h > 0.0 {
        losses as f64 / avail_h
    } else {
        f64::INFINITY
    };
    let ti = sla.continuity_interval_s / 3600.0;
    let continuity = (-lam * ti).exp();
    // Poisson interval on the count (Garwood, through χ²).
    let a = 1.0 - conf;
    let lam_lo = if losses == 0 {
        0.0
    } else {
        chi2_quantile(a / 2.0, 2.0 * losses as f64) / 2.0 / avail_h
    };
    let lam_hi = chi2_quantile(1.0 - a / 2.0, 2.0 * (losses as f64 + 1.0)) / 2.0 / avail_h;
    let ci = [(-lam_hi * ti).exp(), (-lam_lo * ti).exp()];
    let continuity_verdict = match sla.continuity_target {
        Some(c) if avail_h > 0.0 => verdict_lower(ci[0], ci[1], c),
        _ => Verdict::NotAssessed,
    };
    // Longest continuous available run per day.
    let mut runs: BTreeMap<String, f64> = BTreeMap::new();
    let mut run = 0.0;
    let mut prev: Option<(f64, String)> = None;
    for (t, av) in &epochs {
        let day = iso(*t)[..10].to_string();
        let contiguous = prev
            .as_ref()
            .is_some_and(|(pt, pd)| (t - pt - step).abs() < 1e-6 && *pd == day);
        run = if *av {
            if contiguous {
                run + step
            } else {
                step
            }
        } else {
            0.0
        };
        let e = runs.entry(day.clone()).or_insert(0.0);
        *e = e.max(run / 3600.0);
        prev = Some((*t, day));
    }
    let verdicts = [sise_verdict, availability_verdict, continuity_verdict];
    let verdict = if verdicts.contains(&Verdict::Breach) {
        Verdict::Breach
    } else if verdicts.contains(&Verdict::Inconclusive) {
        Verdict::Inconclusive
    } else if verdicts.contains(&Verdict::Meets) {
        Verdict::Meets
    } else {
        Verdict::NotAssessed
    };
    SlaResult {
        sla: sla.id.clone(),
        source: sla.source.clone(),
        note: sla.note.clone(),
        sise_metric: sla.sise_metric.clone(),
        sise_target_m: sla.sise_target_m,
        sise,
        worst_window_start: worst,
        sise_verdict,
        availability,
        availability_interval: ai,
        availability_target: sla.availability_target,
        availability_verdict,
        losses_of_service: losses,
        available_hours: avail_h,
        loss_rate_per_h: lam,
        continuity,
        continuity_interval: ci,
        continuity_interval_s: sla.continuity_interval_s,
        continuity_target: sla.continuity_target,
        continuity_verdict,
        longest_run_h_per_day: runs.into_iter().collect(),
        integrity_events: events,
        verdict,
    }
}

/// Verify a month of samples against the SLAs.
pub fn verify(
    samples: &[Sample],
    slas: &[Sla],
    opt: &Options,
    input_sha256: &str,
) -> Result<MonthlyReport, String> {
    if slas.is_empty() {
        return Err("no SLA given".into());
    }
    for s in slas {
        s.check()?;
    }
    if !(opt.decorrelation_s > 0.0 && opt.confidence > 0.0 && opt.confidence < 1.0) {
        return Err("decorrelation_s must be > 0 and confidence in (0, 1)".into());
    }
    let month: Vec<&Sample> = samples.iter().filter(|s| s.month == opt.month).collect();
    if month.is_empty() {
        return Err(format!("no samples in {}", opt.month));
    }
    let mut providers: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
    for s in &month {
        providers.entry(s.provider.as_str()).or_default().push(s);
    }
    let reports = providers
        .into_iter()
        .map(|(p, ss)| {
            let mut nodes: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
            for s in &ss {
                nodes.entry(s.node.as_str()).or_default().push(s);
            }
            let alarm = slas
                .iter()
                .map(|s| s.alarm_limit_m)
                .fold(f64::INFINITY, f64::min);
            let node_rows = nodes
                .into_iter()
                .map(|(n, v)| {
                    let ok: Vec<f64> = v
                        .iter()
                        .filter_map(|s| (s.status == Status::Ok).then_some(s.residual_m).flatten())
                        .collect();
                    let k = ok.len().max(1) as f64;
                    NodeRow {
                        node: n.into(),
                        samples: v.len(),
                        ok: ok.len(),
                        mean_m: ok.iter().sum::<f64>() / k,
                        rms_m: (ok.iter().map(|x| x * x).sum::<f64>() / k).sqrt(),
                        uee_rms_m: (v.iter().map(|s| s.uee_sigma_m.powi(2)).sum::<f64>()
                            / v.len() as f64)
                            .sqrt(),
                        integrity_events: ok.iter().filter(|e| e.abs() > alarm).count(),
                    }
                })
                .collect();
            let epochs = ss
                .iter()
                .map(|s| (s.t * 1000.0).round() as i64)
                .collect::<BTreeSet<_>>()
                .len();
            ProviderReport {
                provider: p.into(),
                stations: ss
                    .iter()
                    .map(|s| s.station.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                nodes: node_rows,
                epochs,
                results: slas.iter().map(|sla| assess(&ss, sla, opt)).collect(),
            }
        })
        .collect();
    Ok(MonthlyReport {
        status: STATUS.into(),
        month: opt.month.clone(),
        options: opt.clone(),
        input_sha256: input_sha256.into(),
        samples_in_month: month.len(),
        samples_other_months: samples.len() - month.len(),
        providers: reports,
    })
}

/// Human-readable report.
pub fn render_text(r: &MonthlyReport) -> String {
    let mut s = format!(
        "{}\nLunar navigation service SLA verification, {}\ninput SHA-256 {}; {} samples in the month ({} outside it ignored)\n\
         confidence {}, decorrelation {} s\n",
        r.status, r.month, r.input_sha256, r.samples_in_month, r.samples_other_months,
        r.options.confidence, r.options.decorrelation_s
    );
    for p in &r.providers {
        s.push_str(&format!(
            "\n== {} (stations: {}; {} epochs)\n",
            p.provider,
            p.stations.join(", "),
            p.epochs
        ));
        s.push_str("  node      samples   ok    mean m   RMS m   UEE m  events\n");
        for n in &p.nodes {
            s.push_str(&format!(
                "  {:<8} {:>8} {:>5} {:>8.2} {:>7.2} {:>7.2} {:>6}\n",
                n.node, n.samples, n.ok, n.mean_m, n.rms_m, n.uee_rms_m, n.integrity_events
            ));
        }
        for x in &p.results {
            let m = match x.sise_metric {
                Metric::Ksigma { k } => format!("{k} sigma"),
                Metric::Percentile { p } => format!("{}%", p * 100.0),
            };
            s.push_str(&format!(
                "  -- SLA {}: {:?}\n     source: {}\n",
                x.sla, x.verdict, x.source
            ));
            if !x.note.is_empty() {
                s.push_str(&format!("     note: {}\n", x.note));
            }
            match &x.sise {
                Some(e) => s.push_str(&format!(
                    "     SISE ({m}) {:.2} m [{:.2}, {:.2}] vs {} m: {:?}  (n {}, n_eff {}){}\n",
                    e.value,
                    e.lo,
                    e.hi,
                    x.sise_target_m,
                    x.sise_verdict,
                    e.n,
                    e.n_eff,
                    x.worst_window_start
                        .as_ref()
                        .map(|w| format!(", worst window from {w}"))
                        .unwrap_or_default()
                )),
                None => s.push_str("     SISE: not enough data\n"),
            }
            s.push_str(&format!(
                "     availability {:.4} [{:.4}, {:.4}] vs {}: {:?}\n",
                x.availability,
                x.availability_interval[0],
                x.availability_interval[1],
                x.availability_target
                    .map(|a| a.to_string())
                    .unwrap_or("-".into()),
                x.availability_verdict
            ));
            s.push_str(&format!(
                "     continuity over {} s {:.5} [{:.5}, {:.5}] ({} losses in {:.1} h) vs {}: {:?}\n     integrity events {}\n",
                x.continuity_interval_s, x.continuity, x.continuity_interval[0], x.continuity_interval[1],
                x.losses_of_service, x.available_hours,
                x.continuity_target.map(|a| a.to_string()).unwrap_or("-".into()), x.continuity_verdict, x.integrity_events
            ));
        }
    }
    s
}

/// Write `report.json` and `report.txt` (sign the directory afterwards).
pub fn write_dir(out: &std::path::Path, r: &MonthlyReport) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let j = serde_json::to_string_pretty(r).map_err(|e| e.to_string())? + "\n";
    std::fs::write(out.join("report.json"), j).map_err(|e| e.to_string())?;
    std::fs::write(out.join("report.txt"), render_text(r)).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------------------ synthetic

/// A synthetic provider.
#[derive(Clone, Debug, PartialEq)]
pub struct SynthProvider {
    pub name: String,
    pub nodes: usize,
    /// SISE 1σ (m): a Gauss–Markov process per node with `tau_s`.
    pub sise_sigma_m: f64,
    pub tau_s: f64,
    /// Fraction of epochs unavailable (in outages of `outage_epochs`).
    pub outage_fraction: f64,
    pub outage_epochs: usize,
}

/// Synthetic samples for a month (`year`, `month`, 30 days) every `step_s`, one station with
/// UEE σ `uee_m`.
pub fn synthetic_csv(
    year: i32,
    month: u32,
    step_s: f64,
    uee_m: f64,
    providers: &[SynthProvider],
    seed: u64,
) -> String {
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let t0 = super::lunex::secs(&kshana::rinex::EpochUtc {
        year,
        month,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0.0,
    });
    let n = (30.0 * 86_400.0 / step_s) as usize;
    let mut s = String::from("# SYNTHETIC reference-station samples (Kshana Pro D7 example)\ntime,station,provider,node,status,range_residual_m,uee_sigma_m\n");
    for p in providers {
        let phi = (-step_s / p.tau_s).exp();
        let mut x: Vec<f64> = (0..p.nodes)
            .map(|_| p.sise_sigma_m * std_normal(&mut rng))
            .collect();
        let n_out =
            ((p.outage_fraction * n as f64) / p.outage_epochs.max(1) as f64).round() as usize;
        let mut down = vec![false; n];
        use rand::Rng;
        for _ in 0..n_out {
            let a = rng.gen_range(0..n.saturating_sub(p.outage_epochs).max(1));
            for d in down.iter_mut().skip(a).take(p.outage_epochs) {
                *d = true;
            }
        }
        for (k, dn) in down.iter().enumerate() {
            let t = iso(t0 + k as f64 * step_s);
            for (j, xj) in x.iter_mut().enumerate() {
                *xj = phi * *xj + p.sise_sigma_m * (1.0 - phi * phi).sqrt() * std_normal(&mut rng);
                if *dn {
                    s.push_str(&format!(
                        "{t},STN-1,{},N{},missing,,{uee_m}\n",
                        p.name,
                        j + 1
                    ));
                } else {
                    let e = *xj + uee_m * std_normal(&mut rng);
                    s.push_str(&format!(
                        "{t},STN-1,{},N{},ok,{e:.3},{uee_m}\n",
                        p.name,
                        j + 1
                    ));
                }
            }
        }
    }
    s
}

/// The example's two providers: `MEETS-CO` (SISE σ 2.5 m, 1 % outage) and `BREACH-CO` (SISE σ
/// 7 m, 10 % outage).
pub fn example_providers() -> Vec<SynthProvider> {
    vec![
        SynthProvider {
            name: "MEETS-CO".into(),
            nodes: 3,
            sise_sigma_m: 2.5,
            tau_s: 1800.0,
            outage_fraction: 0.01,
            outage_epochs: 6,
        },
        SynthProvider {
            name: "BREACH-CO".into(),
            nodes: 3,
            sise_sigma_m: 7.0,
            tau_s: 1800.0,
            outage_fraction: 0.10,
            outage_epochs: 12,
        },
    ]
}
