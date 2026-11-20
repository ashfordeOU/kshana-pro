// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! B2405: lunar clocks, deep-space tracking and radiometric evidence.
//!
//! * **Clock characterisation** (`clock`): a measured phase record (`time_s,phase_s`, uniform
//!   spacing) is detrended (a quadratic fit removes the frequency offset and the linear drift,
//!   which is reported), and its overlapping Allan deviation with noise-type 95 % intervals is
//!   computed by the open `kshana::allan::overlapping_adev_curve` (Riley, NIST SP 1065). Each
//!   averaging time is judged against a named lunar clock class of the open
//!   `kshana::clock_specs` (optical-master, passive-h-maser, rafs, mini-rafs) or a caller's mask
//!   of `[tau_s, adev]` points (log-log interpolation): **WITHIN** (the whole interval under),
//!   **EXCEEDS** (the whole interval over) or **INCONCLUSIVE**.
//! * **Station delay calibration** (`station-delay`): repeated calibration sessions
//!   (`session,delay_ns`) give the Type A term (mean, standard deviation of the mean); the
//!   caller's Type B terms (cable, reference plane, temperature, ...) are added in a GUM budget
//!   (JCGM 100:2008, [`crate::lunar_pro::Budget`]). The coverage factor is the Student t 95 %
//!   quantile at the Welch–Satterthwaite effective degrees of freedom (JCGM 100 G.4). Against a
//!   previous calibration the En number `|Δ| / √(U₁² + U₂²)` (ISO 13528) says whether the
//!   delay moved beyond the two expanded uncertainties.
//! * **Radiometric forensics and track quality** (`radiometric`): per pass and observable
//!   (`time_s,pass,kind,residual,sigma`, residuals after the orbit-determination fit), outliers
//!   (`|r/σ| > outlier_k` about the median) are set aside, then a constant, a line and a single
//!   step are compared (BIC); a ramp or step whose χ² reduction exceeds `model_k²` is a finding,
//!   as is a mean bias beyond `model_k` standard errors, and gaps longer than `gap_s`. A
//!   track-quality score per pass is `100 × coverage × (1 − outlier fraction) × min(1, 1/NRMS)`,
//!   halved by a step, ramp or bias finding; grades A ≥ 90, B ≥ 75, C ≥ 50, else D. The score is
//!   this module's convention, stated so a reviewer can recompute it.
//!
//! Reproducing CAPSTONE's public tracking data (the backlog's acceptance test) is not done: the
//! data are not in this repository. MODELLED: checked on synthetic clocks, calibrations and
//! passes with injected faults.

use std::collections::BTreeMap;

use kshana::allan::{overlapping_adev_curve, AdevPoint};
use kshana::clock_specs::{LunarClock, CLOCK_F_H_HZ};
use kshana::powerlaw::allan_deviation;
use serde::{Deserialize, Serialize};

use super::service::line_fit;
use super::{csv_rows, mean_std, median, num, rnd, Loaded};
use crate::lunar_pro::budget::{Budget, Term, TermKind};

// ---------------------------------------------------------------------------------------------
// Clock characterisation
// ---------------------------------------------------------------------------------------------

/// `[clock]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockReq {
    pub data: String,
    #[serde(default)]
    pub clock: String,
    /// A named class of `kshana::clock_specs`.
    #[serde(default)]
    pub spec: Option<String>,
    /// `[[tau_s, adev], ...]`, used instead of `spec` when given.
    #[serde(default)]
    pub mask: Option<Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClockPoint {
    pub tau_s: f64,
    pub adev: f64,
    pub ci95: [f64; 2],
    pub noise: Option<String>,
    pub limit: f64,
    pub verdict: String,
    /// `adev · tau`, the time-error scale at this averaging time (ns).
    pub time_error_ns: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ClockReport {
    pub schema: String,
    pub clock: String,
    pub against: String,
    pub samples: usize,
    pub tau0_s: f64,
    pub frequency_offset: f64,
    /// Linear frequency drift (1/s) removed before the ADEV.
    pub drift_per_s: f64,
    pub points: Vec<ClockPoint>,
    pub verdict: String,
    pub evidence: String,
}

/// The limit curve: a named class or a log-log mask.
pub fn limit_at(spec: Option<&str>, mask: Option<&[[f64; 2]]>, tau: f64) -> Result<f64, String> {
    if let Some(m) = mask {
        if m.len() < 2
            || m.windows(2).any(|w| w[1][0] <= w[0][0])
            || m.iter().any(|p| p[0] <= 0.0 || p[1] <= 0.0)
        {
            return Err(
                "mask needs two or more [tau_s, adev] points, tau increasing, all positive".into(),
            );
        }
        let i = m
            .windows(2)
            .position(|w| tau <= w[1][0])
            .unwrap_or(m.len() - 2);
        let (a, b) = (m[i], m[i + 1]);
        let f = (tau.ln() - a[0].ln()) / (b[0].ln() - a[0].ln());
        return Ok((a[1].ln() + f * (b[1].ln() - a[1].ln())).exp());
    }
    let name = spec.ok_or("give spec (a clock class) or mask")?;
    let c = LunarClock::all()
        .into_iter()
        .find(|c| c.name() == name)
        .ok_or_else(|| {
            format!(
                "unknown clock class '{name}' (known: {})",
                LunarClock::all().map(|c| c.name()).join(", ")
            )
        })?;
    Ok(allan_deviation(&c.powerlaw(), tau, CLOCK_F_H_HZ))
}

/// Characterise a phase record against a class or a mask (library form).
pub fn characterise(
    t: &[f64],
    x: &[f64],
    spec: Option<&str>,
    mask: Option<&[[f64; 2]]>,
) -> Result<(f64, f64, f64, Vec<ClockPoint>), String> {
    if t.len() < 32 {
        return Err("a clock record needs 32 or more samples".into());
    }
    let dts: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).collect();
    let tau0 = median(&dts);
    if tau0 <= 0.0 || dts.iter().any(|d| (d - tau0).abs() > 0.01 * tau0) {
        return Err(
            "the phase record must be uniformly spaced with no gaps (split it at gaps)".into(),
        );
    }
    // Quadratic fit x = a + b t + c t² (normal equations on centred time).
    let tm = t.iter().sum::<f64>() / t.len() as f64;
    let mut s = [[0.0f64; 3]; 3];
    let mut rhs = [0.0f64; 3];
    for (ti, xi) in t.iter().zip(x) {
        let u = ti - tm;
        let v = [1.0, u, u * u];
        for i in 0..3 {
            rhs[i] += v[i] * xi;
            for j in 0..3 {
                s[i][j] += v[i] * v[j];
            }
        }
    }
    let c = solve(&mut s.iter().map(|r| r.to_vec()).collect::<Vec<_>>(), &rhs)
        .ok_or("phase record is degenerate")?;
    let resid: Vec<f64> = t
        .iter()
        .zip(x)
        .map(|(ti, xi)| {
            let u = ti - tm;
            xi - (c[0] + c[1] * u + c[2] * u * u)
        })
        .collect();
    let curve: Vec<AdevPoint> = overlapping_adev_curve(&resid, tau0);
    let mut pts = Vec::new();
    for p in curve {
        let lim = limit_at(spec, mask, p.tau_s)?;
        let v = if p.ci_hi <= lim {
            "WITHIN"
        } else if p.ci_lo > lim {
            "EXCEEDS"
        } else {
            "INCONCLUSIVE"
        };
        pts.push(ClockPoint {
            tau_s: p.tau_s,
            adev: p.adev,
            ci95: [p.ci_lo, p.ci_hi],
            noise: p.noise.map(|n| format!("{n:?}")),
            limit: lim,
            verdict: v.into(),
            time_error_ns: rnd(p.adev * p.tau_s * 1e9, 6),
        });
    }
    Ok((tau0, c[1], 2.0 * c[2], pts))
}

/// Gaussian elimination with partial pivoting (small dense systems).
#[allow(clippy::needless_range_loop)]
pub(crate) fn solve(a: &mut [Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = b.len();
    let mut m: Vec<Vec<f64>> = a
        .iter()
        .zip(b)
        .map(|(r, bi)| {
            let mut r = r.clone();
            r.push(*bi);
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

/// Clock characterisation command.
pub fn clock(l: &mut Loaded) -> Result<ClockReport, String> {
    let r = l
        .req
        .clock
        .clone()
        .ok_or("the request has no [clock] section")?;
    let text = l.read(&r.data, "clock-phase")?;
    let mut t = Vec::new();
    let mut x = Vec::new();
    for f in csv_rows(&text, &["time_s", "phase_s"])? {
        t.push(num(&f[0], "time_s")?);
        x.push(num(&f[1], "phase_s")?);
    }
    let (tau0, y, drift, points) = characterise(&t, &x, r.spec.as_deref(), r.mask.as_deref())?;
    let verdict = if points.iter().any(|p| p.verdict == "EXCEEDS") {
        "EXCEEDS at one or more averaging times"
    } else if points.iter().all(|p| p.verdict == "WITHIN") {
        "WITHIN at every averaging time measured"
    } else {
        "WITHIN or INCONCLUSIVE (record too short for some averaging times)"
    };
    Ok(ClockReport {
        schema: "kshana-pro/lunar-clock-characterisation/v1".into(),
        clock: r.clock,
        against: match (&r.mask, &r.spec) {
            (Some(_), _) => "caller mask".into(),
            (None, Some(s)) => format!("kshana::clock_specs class {s}"),
            _ => String::new(),
        },
        samples: t.len(),
        tau0_s: tau0,
        frequency_offset: y,
        drift_per_s: drift,
        points,
        verdict: verdict.into(),
        evidence: super::EVIDENCE.into(),
    })
}

// ---------------------------------------------------------------------------------------------
// Station delay calibration
// ---------------------------------------------------------------------------------------------

/// One Type B term of a delay budget.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelayTerm {
    pub name: String,
    #[serde(default)]
    pub value_ns: f64,
    /// Standard uncertainty (normal) or half-width (rectangular), ns.
    pub u_ns: f64,
    #[serde(default = "d_normal")]
    pub dist: String,
    #[serde(default)]
    pub source: String,
}
fn d_normal() -> String {
    "normal".into()
}

/// `[station_delay]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelayReq {
    pub station: String,
    pub sessions: String,
    #[serde(default)]
    pub terms: Vec<DelayTerm>,
    /// Previous calibration value and its expanded (95 %) uncertainty, ns.
    #[serde(default)]
    pub previous_ns: Option<f64>,
    #[serde(default)]
    pub previous_expanded_ns: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DelayReport {
    pub schema: String,
    pub station: String,
    pub sessions: usize,
    pub delay_ns: f64,
    pub u_c_ns: f64,
    pub nu_eff: f64,
    pub k: f64,
    pub expanded_ns: f64,
    pub budget: Budget,
    pub en: Option<f64>,
    pub verdict: String,
    pub evidence: String,
}

/// Two-sided 95 % Student t quantile (Cornish–Fisher expansion; exact to ~1e-3 for ν ≥ 3).
pub fn t95(nu: f64) -> f64 {
    let z: f64 = 1.959_964;
    if !nu.is_finite() || nu > 1e6 {
        return z;
    }
    let nu = nu.max(1.0);
    let (z3, z5, z7) = (z.powi(3), z.powi(5), z.powi(7));
    z + (z3 + z) / (4.0 * nu)
        + (5.0 * z5 + 16.0 * z3 + 3.0 * z) / (96.0 * nu * nu)
        + (3.0 * z7 + 19.0 * z5 + 17.0 * z3 - 15.0 * z) / (384.0 * nu.powi(3))
}

/// Station delay calibration with a GUM budget.
pub fn station_delay(l: &mut Loaded) -> Result<DelayReport, String> {
    let r = l
        .req
        .station_delay
        .clone()
        .ok_or("the request has no [station_delay] section")?;
    let text = l.read(&r.sessions, "calibration-sessions")?;
    let v: Vec<f64> = csv_rows(&text, &["session", "delay_ns"])?
        .iter()
        .map(|f| num(&f[1], "delay_ns"))
        .collect::<Result<_, _>>()?;
    if v.len() < 2 {
        return Err("a delay calibration needs two or more sessions".into());
    }
    let (m, s) = mean_std(&v);
    let ua = s / (v.len() as f64).sqrt();
    let mut b = Budget::new(vec![Term::normal(
        "repeatability",
        m,
        ua,
        TermKind::A,
        &format!(
            "mean of {} sessions, standard deviation of the mean",
            v.len()
        ),
    )]);
    for t in &r.terms {
        if t.u_ns < 0.0 {
            return Err(format!("term {}: u_ns must not be negative", t.name));
        }
        b.push(match t.dist.as_str() {
            "normal" => Term::normal(&t.name, t.value_ns, t.u_ns, TermKind::B, &t.source),
            "rectangular" => Term::rectangular(&t.name, t.value_ns, t.u_ns, &t.source),
            o => {
                return Err(format!(
                    "term {}: dist '{o}' must be normal or rectangular",
                    t.name
                ))
            }
        });
    }
    let uc = b.u_c();
    // Welch–Satterthwaite: only the Type A term has finite degrees of freedom.
    let nu = if ua > 0.0 {
        uc.powi(4) / (ua.powi(4) / (v.len() as f64 - 1.0))
    } else {
        f64::INFINITY
    };
    let k = t95(nu);
    let big_u = k * uc;
    let en = match (r.previous_ns, r.previous_expanded_ns) {
        (Some(p), Some(pu)) => Some(rnd(
            (b.value() - p).abs() / (big_u * big_u + pu * pu).sqrt(),
            3,
        )),
        _ => None,
    };
    let verdict = match en {
        Some(e) if e <= 1.0 => "CONSISTENT with the previous calibration (En ≤ 1)".into(),
        Some(_) => {
            "MOVED beyond the combined expanded uncertainties (En > 1): recalibrate or explain"
                .into()
        }
        None => "CALIBRATED (no previous calibration given)".to_string(),
    };
    Ok(DelayReport {
        schema: "kshana-pro/station-delay-calibration/v1".into(),
        station: r.station,
        sessions: v.len(),
        delay_ns: rnd(b.value(), 4),
        u_c_ns: rnd(uc, 4),
        nu_eff: if nu.is_finite() { rnd(nu, 1) } else { 1e9 },
        k: rnd(k, 3),
        expanded_ns: rnd(big_u, 4),
        budget: b,
        en,
        verdict,
        evidence: super::EVIDENCE.into(),
    })
}

// ---------------------------------------------------------------------------------------------
// Radiometric forensics and track quality
// ---------------------------------------------------------------------------------------------

fn d_outlier() -> f64 {
    5.0
}
fn d_model() -> f64 {
    6.0
}
fn d_gap() -> f64 {
    60.0
}

/// `[radiometric]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadioReq {
    pub data: String,
    /// Planned pass windows `pass,start_s,end_s` (coverage against plan).
    #[serde(default)]
    pub planned: Option<String>,
    #[serde(default = "d_outlier")]
    pub outlier_k: f64,
    #[serde(default = "d_model")]
    pub model_k: f64,
    #[serde(default = "d_gap")]
    pub gap_s: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PassQuality {
    pub pass: String,
    pub kind: String,
    pub samples: usize,
    pub outliers: usize,
    pub nrms: f64,
    pub coverage: f64,
    pub gaps: Vec<[f64; 2]>,
    pub findings: Vec<String>,
    pub score: f64,
    pub grade: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RadioReport {
    pub schema: String,
    pub passes: Vec<PassQuality>,
    pub score_formula: String,
    pub evidence: String,
}

fn rss_const(z: &[f64]) -> f64 {
    let (m, _) = mean_std(z);
    z.iter().map(|v| (v - m) * (v - m)).sum()
}

/// Score one pass (library form): `pts` are `(time_s, residual/sigma)`.
pub fn score_pass(
    pass: &str,
    kind: &str,
    pts: &[(f64, f64)],
    window: Option<(f64, f64)>,
    q: &RadioReq,
) -> PassQuality {
    // Outliers about a running median of the 11 neighbouring samples (robust to steps).
    let zs: Vec<f64> = pts.iter().map(|p| p.1).collect();
    let (mut keep, mut out) = (Vec::new(), Vec::new());
    for (i, p) in pts.iter().enumerate() {
        let w = &zs[i.saturating_sub(5)..(i + 6).min(zs.len())];
        if (p.1 - median(w)).abs() <= q.outlier_k {
            keep.push(*p);
        } else {
            out.push(*p);
        }
    }
    let mut findings = Vec::new();
    if !out.is_empty() {
        findings.push(format!("{} outlier(s) beyond {}σ", out.len(), q.outlier_k));
    }
    let (t, z): (Vec<f64>, Vec<f64>) = keep.iter().copied().unzip();
    let n = z.len();
    let nrms = if n > 0 {
        (z.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt()
    } else {
        f64::NAN
    };
    let mut model_finding = false;
    if n >= 12 {
        let r0 = rss_const(&z);
        let (my, b) = line_fit(&t, &z);
        let tm = t.iter().sum::<f64>() / n as f64;
        let r1: f64 = t
            .iter()
            .zip(&z)
            .map(|(ti, zi)| (zi - my - b * (ti - tm)).powi(2))
            .sum();
        let (mut r2, mut split) = (f64::MAX, 0);
        for s in 5..n - 5 {
            let v = rss_const(&z[..s]) + rss_const(&z[s..]);
            if v < r2 {
                r2 = v;
                split = s;
            }
        }
        let nf = n as f64;
        let bic = |rss: f64, k: f64| nf * (rss.max(1e-12) / nf).ln() + k * nf.ln();
        let thr = q.model_k * q.model_k;
        let (b1, b2) = (bic(r1, 2.0), bic(r2, 3.0));
        if r0 - r2 > thr && b2 < b1 {
            let (m1, _) = mean_std(&z[..split]);
            let (m2, _) = mean_std(&z[split..]);
            findings.push(format!("step of {:.2}σ at t = {:.0} s", m2 - m1, t[split]));
            model_finding = true;
        } else if r0 - r1 > thr {
            let span = t[n - 1] - t[0];
            findings.push(format!(
                "ramp of {:.2}σ over the pass ({:.0} s)",
                b * span,
                span
            ));
            model_finding = true;
        } else {
            let (m, _) = mean_std(&z);
            if m.abs() * nf.sqrt() > q.model_k {
                findings.push(format!("bias of {m:.2}σ"));
                model_finding = true;
            }
        }
    }
    let mut ts: Vec<f64> = pts.iter().map(|p| p.0).collect();
    ts.sort_by(|a, b| a.total_cmp(b));
    let dts: Vec<f64> = ts.windows(2).map(|w| w[1] - w[0]).collect();
    let dt = median(&dts);
    let gap_min = q.gap_s.max(3.0 * dt);
    let gaps: Vec<[f64; 2]> = ts
        .windows(2)
        .filter(|w| w[1] - w[0] > gap_min)
        .map(|w| [w[0], w[1]])
        .collect();
    if !gaps.is_empty() {
        findings.push(format!("{} gap(s) longer than {gap_min:.0} s", gaps.len()));
    }
    let coverage = match (window, ts.first(), ts.last()) {
        (Some((a, b)), _, _) if b > a => (ts.len() as f64 * dt / (b - a)).min(1.0),
        (_, Some(a), Some(b)) if b > a => {
            1.0 - gaps.iter().map(|g| g[1] - g[0]).sum::<f64>() / (b - a)
        }
        _ => 0.0,
    };
    let of = out.len() as f64 / pts.len().max(1) as f64;
    let mut score = 100.0 * coverage * (1.0 - of) * (1.0 / nrms).min(1.0);
    if model_finding {
        score *= 0.5;
    }
    if !score.is_finite() {
        score = 0.0;
    }
    let grade = match score {
        s if s >= 90.0 => "A",
        s if s >= 75.0 => "B",
        s if s >= 50.0 => "C",
        _ => "D",
    };
    PassQuality {
        pass: pass.into(),
        kind: kind.into(),
        samples: pts.len(),
        outliers: out.len(),
        nrms: rnd(nrms, 3),
        coverage: rnd(coverage, 4),
        gaps,
        findings,
        score: rnd(score, 1),
        grade: grade.into(),
    }
}

/// Radiometric anomaly forensics and track-quality scoring.
pub fn radiometric(l: &mut Loaded) -> Result<RadioReport, String> {
    let q = l
        .req
        .radiometric
        .clone()
        .ok_or("the request has no [radiometric] section")?;
    let text = l.read(&q.data, "radiometric-residuals")?;
    let mut by: BTreeMap<(String, String), Vec<(f64, f64)>> = BTreeMap::new();
    for f in csv_rows(&text, &["time_s", "pass", "kind", "residual", "sigma"])? {
        let s = num(&f[4], "sigma")?;
        if s <= 0.0 {
            return Err("sigma must be positive".into());
        }
        by.entry((f[1].clone(), f[2].clone()))
            .or_default()
            .push((num(&f[0], "time_s")?, num(&f[3], "residual")? / s));
    }
    let mut plan: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    if let Some(p) = &q.planned {
        let text = l.read(p, "planned-passes")?;
        for f in csv_rows(&text, &["pass", "start_s", "end_s"])? {
            plan.insert(f[0].clone(), (num(&f[1], "start_s")?, num(&f[2], "end_s")?));
        }
    }
    let mut passes = Vec::new();
    for ((pass, kind), mut pts) in by {
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        passes.push(score_pass(&pass, &kind, &pts, plan.get(&pass).copied(), &q));
    }
    for p in plan.keys() {
        if !passes.iter().any(|x| &x.pass == p) {
            passes.push(PassQuality {
                pass: p.clone(),
                kind: String::new(),
                samples: 0,
                outliers: 0,
                nrms: f64::NAN,
                coverage: 0.0,
                gaps: vec![],
                findings: vec!["planned pass with no data (lost pass)".into()],
                score: 0.0,
                grade: "D".into(),
            });
        }
    }
    Ok(RadioReport {
        schema: "kshana-pro/radiometric-track-quality/v1".into(),
        passes,
        score_formula:
            "100 x coverage x (1 - outlier fraction) x min(1, 1/NRMS), x0.5 for a step, \
                        ramp or bias finding; A>=90, B>=75, C>=50, else D"
                .into(),
        evidence: super::EVIDENCE.into(),
    })
}
