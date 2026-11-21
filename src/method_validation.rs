// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Method precision study: repeatability and reproducibility (research 2026-11-20 gaps,
//! B2505 Pro part; docs/METHOD-VALIDATION.md)
//!
//! An interlaboratory study of a scoring method in the form of the ISO 5725-2 basic method,
//! implemented from its published formulae (the standard's text has not been read for this
//! build; the critical values are cross-checked against two published table entries, and the
//! rest is to confirm). Per level, with `p` laboratories and `n_i` replicates each:
//!
//! * cell means and standard deviations; repeatability variance
//!   `s_r² = Σ(n_i − 1)s_i² / Σ(n_i − 1)`; between-laboratory variance
//!   `s_L² = max(0, (s_d² − s_r²)/n̄)` with `s_d² = Σ n_i(ȳ_i − ȳ)²/(p − 1)` and
//!   `n̄ = (Σn_i − Σn_i²/Σn_i)/(p − 1)`; reproducibility `s_R² = s_L² + s_r²`;
//!   limits `r = 2.8 s_r`, `R = 2.8 s_R`;
//! * consistency: Mandel's h (between) and k (within), Cochran's C on the largest variance and
//!   Grubbs' single-outlier statistics on the extreme cell means, each against its critical
//!   value at the straggler (5 %) and outlier (1 %) levels, computed from the F and t
//!   distributions;
//! * with a reference value: the bias of the method and its acceptance bound
//!   `A·s_R = 1.96 √((s_R² − (1 − 1/n̄) s_r²)/p)`.
//!
//! Laboratories are excluded only when the request lists them with a reason; the study is
//! reported with all laboratories and, if any are excluded, without them. MODELLED statistics
//! of the data supplied; the report states precision under the method as run, nothing more.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::pack_c::stat_gate::{f_upper, student_t_two_sided};

/// Pro feature.
pub const FEATURE: &str = "pro-method-validation";
/// Report format.
pub const FORMAT: &str = "kshana-pro-method-precision-study/1";

/// One excluded laboratory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    pub lab: String,
    pub reason: String,
}

/// One level of the study.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    pub name: String,
    #[serde(default)]
    pub reference_value: Option<f64>,
    /// Laboratory → replicate results.
    pub results: BTreeMap<String, Vec<f64>>,
}

/// The study request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub title: String,
    pub method_id: String,
    pub quantity: String,
    #[serde(default)]
    pub unit: String,
    #[serde(default = "d_straggler")]
    pub alpha_straggler: f64,
    #[serde(default = "d_outlier")]
    pub alpha_outlier: f64,
    #[serde(default)]
    pub exclude: Vec<Exclusion>,
    #[serde(rename = "level")]
    pub levels: Vec<Level>,
}

fn d_straggler() -> f64 {
    0.05
}
fn d_outlier() -> f64 {
    0.01
}

/// Upper-tail quantile of F(d1, d2): the f with P(F ≥ f) = q.
pub fn f_quantile_upper(q: f64, d1: f64, d2: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 1.0);
    while f_upper(hi, d1, d2) > q && hi < 1e12 {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f_upper(mid, d1, d2) > q {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Upper-tail quantile of Student's t: the t with P(T ≥ t) = q.
pub fn t_quantile_upper(q: f64, df: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 1e6);
    for _ in 0..300 {
        let mid = 0.5 * (lo + hi);
        if student_t_two_sided(mid, df) / 2.0 > q {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Critical value of Cochran's C for `p` cells of `n` replicates at level `alpha`.
pub fn cochran_critical(p: usize, n: usize, alpha: f64) -> f64 {
    let nu = (n - 1) as f64;
    let f = f_quantile_upper(alpha / p as f64, nu, (p as f64 - 1.0) * nu);
    1.0 / (1.0 + (p as f64 - 1.0) / f)
}

/// Critical value of Grubbs' single-outlier statistic for `p` values at level `alpha`.
pub fn grubbs_critical(p: usize, alpha: f64) -> f64 {
    let pf = p as f64;
    let t = t_quantile_upper(alpha / (2.0 * pf), pf - 2.0);
    (pf - 1.0) / pf.sqrt() * (t * t / (pf - 2.0 + t * t)).sqrt()
}

/// One laboratory's cell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cell {
    pub lab: String,
    pub n: usize,
    pub mean: f64,
    pub sd: f64,
    pub h: f64,
    pub k: f64,
    pub excluded: bool,
}

/// A test against its critical values.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Test {
    pub test: &'static str,
    pub lab: String,
    pub statistic: f64,
    pub straggler_critical: f64,
    pub outlier_critical: f64,
    /// `ok`, `straggler` or `outlier`.
    pub class: &'static str,
}

/// Precision estimates from a set of cells.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Precision {
    pub labs: usize,
    pub mean: f64,
    pub s_r: f64,
    pub s_l: f64,
    pub s_rr: f64,
    pub r_limit: f64,
    pub rr_limit: f64,
    pub bias: Option<f64>,
    pub bias_bound: Option<f64>,
    pub bias_within_bound: Option<bool>,
}

/// One level's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LevelResult {
    pub name: String,
    pub reference_value: Option<f64>,
    pub cells: Vec<Cell>,
    pub tests: Vec<Test>,
    pub all_labs: Precision,
    pub after_exclusions: Option<Precision>,
}

/// The study report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub title: String,
    pub method_id: String,
    pub quantity: String,
    pub unit: String,
    pub alpha_straggler: f64,
    pub alpha_outlier: f64,
    pub exclusions: Vec<Exclusion>,
    pub levels: Vec<LevelResult>,
    pub basis: &'static str,
    pub verdict: String,
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let s = if v.len() > 1 {
        (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    (m, s)
}

fn precision(cells: &[&Cell], reference: Option<f64>) -> Result<Precision, String> {
    let p = cells.len();
    if p < 3 {
        return Err("at least three laboratories are needed".into());
    }
    let dof_r: f64 = cells.iter().map(|c| c.n as f64 - 1.0).sum();
    if dof_r <= 0.0 {
        return Err("replicates are needed for repeatability".into());
    }
    let s_r2 = cells
        .iter()
        .map(|c| (c.n as f64 - 1.0) * c.sd * c.sd)
        .sum::<f64>()
        / dof_r;
    let tn: f64 = cells.iter().map(|c| c.n as f64).sum();
    let grand = cells.iter().map(|c| c.n as f64 * c.mean).sum::<f64>() / tn;
    let s_d2 = cells
        .iter()
        .map(|c| c.n as f64 * (c.mean - grand).powi(2))
        .sum::<f64>()
        / (p as f64 - 1.0);
    let nbar = (tn - cells.iter().map(|c| (c.n * c.n) as f64).sum::<f64>() / tn) / (p as f64 - 1.0);
    let s_l2 = ((s_d2 - s_r2) / nbar).max(0.0);
    let s_rr2 = s_l2 + s_r2;
    // The general mean is the mean of the cell means (equal weight per laboratory).
    let m = cells.iter().map(|c| c.mean).sum::<f64>() / p as f64;
    let (bias, bound) = match reference {
        Some(mu) => {
            let a = 1.96 * ((s_rr2 - (1.0 - 1.0 / nbar) * s_r2).max(0.0) / p as f64).sqrt();
            (Some(m - mu), Some(a))
        }
        None => (None, None),
    };
    Ok(Precision {
        labs: p,
        mean: m,
        s_r: s_r2.sqrt(),
        s_l: s_l2.sqrt(),
        s_rr: s_rr2.sqrt(),
        r_limit: 2.8 * s_r2.sqrt(),
        rr_limit: 2.8 * s_rr2.sqrt(),
        bias_within_bound: bias.zip(bound).map(|(b, a)| b.abs() <= a),
        bias,
        bias_bound: bound,
    })
}

fn classify(x: f64, s: f64, o: f64) -> &'static str {
    if x > o {
        "outlier"
    } else if x > s {
        "straggler"
    } else {
        "ok"
    }
}

/// Run the study.
pub fn study(q: &Study) -> Result<Report, String> {
    if q.levels.is_empty() {
        return Err("at least one [[level]] is needed".into());
    }
    if !(q.alpha_outlier > 0.0 && q.alpha_outlier < q.alpha_straggler && q.alpha_straggler < 0.5) {
        return Err("0 < alpha_outlier < alpha_straggler < 0.5 is needed".into());
    }
    for e in &q.exclude {
        if e.reason.trim().is_empty() {
            return Err(format!("exclusion of '{}' needs a reason", e.lab));
        }
    }
    let excluded = |lab: &str| q.exclude.iter().any(|e| e.lab == lab);
    let mut levels = Vec::new();
    let mut flags = 0;
    for lv in &q.levels {
        let mut cells = Vec::new();
        for (lab, v) in &lv.results {
            if v.is_empty() || v.iter().any(|x| !x.is_finite()) {
                return Err(format!(
                    "level '{}', lab '{lab}': results must be finite and non-empty",
                    lv.name
                ));
            }
            let (m, s) = mean_sd(v);
            cells.push(Cell {
                lab: lab.clone(),
                n: v.len(),
                mean: m,
                sd: s,
                h: 0.0,
                k: 0.0,
                excluded: excluded(lab),
            });
        }
        let refs: Vec<&Cell> = cells.iter().collect();
        let all = precision(&refs, lv.reference_value)
            .map_err(|e| format!("level '{}': {e}", lv.name))?;
        // Mandel's h and k.
        let p = cells.len();
        let mm = cells.iter().map(|c| c.mean).sum::<f64>() / p as f64;
        let sm =
            (cells.iter().map(|c| (c.mean - mm).powi(2)).sum::<f64>() / (p as f64 - 1.0)).sqrt();
        for c in &mut cells {
            c.h = if sm > 0.0 { (c.mean - mm) / sm } else { 0.0 };
            c.k = if all.s_r > 0.0 { c.sd / all.s_r } else { 0.0 };
        }
        let mut tests = Vec::new();
        // Cochran on the largest variance (cells with replicates; n taken as the most common n).
        let reps: Vec<&Cell> = cells.iter().filter(|c| c.n >= 2).collect();
        let sum_v: f64 = reps.iter().map(|c| c.sd * c.sd).sum();
        if reps.len() >= 3 && sum_v > 0.0 {
            let mut count = BTreeMap::new();
            for c in &reps {
                *count.entry(c.n).or_insert(0) += 1;
            }
            let n = count
                .iter()
                .max_by_key(|(_, k)| **k)
                .map(|(n, _)| *n)
                .unwrap_or(2);
            let top = reps.iter().max_by(|a, b| a.sd.total_cmp(&b.sd)).unwrap();
            let c = top.sd * top.sd / sum_v;
            let (cs, co) = (
                cochran_critical(reps.len(), n, q.alpha_straggler),
                cochran_critical(reps.len(), n, q.alpha_outlier),
            );
            tests.push(Test {
                test: "Cochran C (largest variance)",
                lab: top.lab.clone(),
                statistic: c,
                straggler_critical: cs,
                outlier_critical: co,
                class: classify(c, cs, co),
            });
        }
        // Grubbs on the extreme cell means.
        if p >= 3 && sm > 0.0 {
            let (gs, go) = (
                grubbs_critical(p, q.alpha_straggler),
                grubbs_critical(p, q.alpha_outlier),
            );
            let hi = cells
                .iter()
                .max_by(|a, b| a.mean.total_cmp(&b.mean))
                .unwrap();
            let lo = cells
                .iter()
                .min_by(|a, b| a.mean.total_cmp(&b.mean))
                .unwrap();
            for (name, c, g) in [
                ("Grubbs (largest mean)", hi, (hi.mean - mm) / sm),
                ("Grubbs (smallest mean)", lo, (mm - lo.mean) / sm),
            ] {
                tests.push(Test {
                    test: name,
                    lab: c.lab.clone(),
                    statistic: g,
                    straggler_critical: gs,
                    outlier_critical: go,
                    class: classify(g, gs, go),
                });
            }
        }
        flags += tests.iter().filter(|t| t.class != "ok").count();
        let after = if cells.iter().any(|c| c.excluded) {
            let kept: Vec<&Cell> = cells.iter().filter(|c| !c.excluded).collect();
            Some(
                precision(&kept, lv.reference_value)
                    .map_err(|e| format!("level '{}' after exclusions: {e}", lv.name))?,
            )
        } else {
            None
        };
        levels.push(LevelResult {
            name: lv.name.clone(),
            reference_value: lv.reference_value,
            cells,
            tests,
            all_labs: all,
            after_exclusions: after,
        });
    }
    let unknown: Vec<&str> = q
        .exclude
        .iter()
        .filter(|e| !q.levels.iter().any(|l| l.results.contains_key(&e.lab)))
        .map(|e| e.lab.as_str())
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "excluded laboratories not in the study: {}",
            unknown.join(", ")
        ));
    }
    let verdict = format!(
        "Precision of method {} for {} over {} level(s): {}; {} consistency test(s) at straggler or outlier level{}. Results are \"scored under method {}\"; this report does not rate any product.",
        q.method_id,
        q.quantity,
        levels.len(),
        levels
            .iter()
            .map(|l| {
                let p = l.after_exclusions.as_ref().unwrap_or(&l.all_labs);
                format!("{} r = {:.4} R = {:.4}{}", l.name, p.r_limit, p.rr_limit, if q.unit.is_empty() { String::new() } else { format!(" {}", q.unit) })
            })
            .collect::<Vec<_>>()
            .join(", "),
        flags,
        if q.exclude.is_empty() { String::new() } else { format!("; {} laboratory exclusion(s) with stated reasons", q.exclude.len()) },
        q.method_id
    );
    Ok(Report {
        format: FORMAT,
        title: q.title.clone(),
        method_id: q.method_id.clone(),
        quantity: q.quantity.clone(),
        unit: q.unit.clone(),
        alpha_straggler: q.alpha_straggler,
        alpha_outlier: q.alpha_outlier,
        exclusions: q.exclude.clone(),
        levels,
        basis: "Basic method in the form of ISO 5725-2, implemented from its published formulae (critical values computed from the F and t distributions; to confirm against the standard's text). MODELLED statistics of the results supplied.",
        verdict,
    })
}

/// A SYNTHETIC study: eight laboratories, two levels, three replicates each, with laboratory
/// L7 carrying a large bias and L3 a large scatter at level B.
pub fn synth() -> String {
    let mut s: u64 = 17;
    let mut g = || {
        // Box-Muller from a LCG.
        let mut u = || {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let (a, b) = (u(), u());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    };
    let mut o = String::from(
        "title = \"SYNTHETIC interlaboratory study\"\nmethod_id = \"KM-SYNTH-1\"\nquantity = \"time-to-detect\"\nunit = \"s\"\n\n",
    );
    for (name, truth, sr, sl) in [("A", 12.0, 0.4, 0.3), ("B", 30.0, 0.8, 0.6)] {
        o.push_str(&format!(
            "[[level]]\nname = \"{name}\"\nreference_value = {truth}\n[level.results]\n"
        ));
        for l in 1..=8 {
            let lab_bias = sl * g() + if l == 7 { 6.0 * sl + 4.0 * sr } else { 0.0 };
            let scatter = if l == 3 && name == "B" { 5.0 } else { 1.0 };
            let v: Vec<String> = (0..3)
                .map(|_| format!("{:.3}", truth + lab_bias + scatter * sr * g()))
                .collect();
            o.push_str(&format!("L{l} = [{}]\n", v.join(", ")));
        }
        o.push('\n');
    }
    o
}

/// The evidence page.
pub fn html(r: &Report) -> String {
    let f = |x: f64| format!("{x:.4}");
    let mut secs = Vec::new();
    for l in &r.levels {
        secs.push((
            format!("Level {}: cells", l.name),
            vec!["lab", "n", "mean", "sd", "h", "k", "excluded"],
            l.cells
                .iter()
                .map(|c| {
                    vec![
                        c.lab.clone(),
                        c.n.to_string(),
                        f(c.mean),
                        f(c.sd),
                        format!("{:.2}", c.h),
                        format!("{:.2}", c.k),
                        if c.excluded { "yes" } else { "-" }.into(),
                    ]
                })
                .collect(),
        ));
        secs.push((
            format!("Level {}: consistency tests", l.name),
            vec![
                "test",
                "lab",
                "statistic",
                "5 % critical",
                "1 % critical",
                "class",
            ],
            l.tests
                .iter()
                .map(|t| {
                    vec![
                        t.test.into(),
                        t.lab.clone(),
                        f(t.statistic),
                        f(t.straggler_critical),
                        f(t.outlier_critical),
                        t.class.into(),
                    ]
                })
                .collect(),
        ));
        let mut rows = Vec::new();
        for (name, p) in std::iter::once(("all laboratories", &l.all_labs))
            .chain(l.after_exclusions.as_ref().map(|p| ("after exclusions", p)))
        {
            rows.push(vec![
                name.to_string(),
                p.labs.to_string(),
                f(p.mean),
                f(p.s_r),
                f(p.s_rr),
                f(p.r_limit),
                f(p.rr_limit),
                match (p.bias, p.bias_bound) {
                    (Some(b), Some(a)) => format!(
                        "{} (bound {}: {})",
                        f(b),
                        f(a),
                        if b.abs() <= a { "within" } else { "beyond" }
                    ),
                    _ => "-".into(),
                },
            ]);
        }
        secs.push((
            format!("Level {}: precision", l.name),
            vec!["set", "labs", "mean", "s_r", "s_R", "r", "R", "bias"],
            rows,
        ));
    }
    if !r.exclusions.is_empty() {
        secs.push((
            "Exclusions".into(),
            vec!["lab", "reason"],
            r.exclusions
                .iter()
                .map(|e| vec![e.lab.clone(), e.reason.clone()])
                .collect(),
        ));
    }
    crate::transport::page(
        &r.title,
        "MODELLED: statistics of the results supplied",
        &r.verdict,
        &[
            format!(
                "Method {}; quantity {} ({}).",
                r.method_id, r.quantity, r.unit
            ),
            r.basis.to_string(),
        ],
        &secs,
    )
}
