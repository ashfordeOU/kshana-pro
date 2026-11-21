// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Yearly re-scoring scheduler and per-test uncertainty budgets (research 2026-11-20 gaps,
//! B2505, Pro part)
//!
//! Products scored under a Kshana method are re-scored on a schedule, and every test of a method
//! carries an uncertainty budget. The public register and the revocation list are not here (they
//! belong elsewhere); this module gives:
//!
//! * [`schedule`]: from the user's register of scorings (`product_id,method_id,scored_utc`) and a
//!   policy (interval in months, warning window in days), the next re-scoring date of each
//!   product under each method, with OVERDUE / DUE SOON / SCHEDULED status;
//! * [`budget`]: per test of a method, a GUM uncertainty budget: Type A contributions from
//!   repeated runs (standard deviation of the mean, `n − 1` degrees of freedom) and Type B
//!   contributions (standard, 95 % expanded or rectangular half-width, with stated or infinite
//!   degrees of freedom), sensitivity coefficients, the combined standard uncertainty, the
//!   Welch-Satterthwaite effective degrees of freedom and the coverage factor for 95 % from the
//!   Student t distribution; with a reference value, the bias and its normalised error `E_n`;
//! * a **method report** of all tests ("scored under method X"), which says how well the method
//!   measures, not that a product is good.
//!
//! **MODELLED**: arithmetic on the user's runs and stated contributions (JCGM 100:2008 §§4-6 and
//! Annex G).

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};
use crate::pack_c::stat_gate::student_t_two_sided;
use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-method-uncertainty";
/// Formats.
pub const SCHEDULE_FORMAT: &str = "kshana-pro-rescoring-schedule/1";
pub const REPORT_FORMAT: &str = "kshana-pro-method-uncertainty-report/1";

/// Re-scoring policy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default = "d_months")]
    pub interval_months: u32,
    #[serde(default = "d_warn")]
    pub warn_days: f64,
}

fn d_months() -> u32 {
    12
}
fn d_warn() -> f64 {
    60.0
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            interval_months: d_months(),
            warn_days: d_warn(),
        }
    }
}

/// One line of the schedule.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Due {
    pub product_id: String,
    pub method_id: String,
    pub last_scored_utc: String,
    pub next_due_utc: String,
    pub days_left: f64,
    /// OVERDUE, DUE SOON or SCHEDULED.
    pub status: &'static str,
}

/// The schedule.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Schedule {
    pub format: &'static str,
    pub at_utc: String,
    pub policy: Policy,
    pub items: Vec<Due>,
    pub verdict: String,
}

fn add_months(utc: &str, n: u32) -> String {
    let y: i64 = utc[..4].parse().unwrap_or(1970);
    let m: i64 = utc[5..7].parse().unwrap_or(1);
    let t = y * 12 + (m - 1) + n as i64;
    let (ny, nm) = (t / 12, t % 12 + 1);
    let mut d: u32 = utc[8..10].parse().unwrap_or(1);
    let dim = match nm {
        2 if ny % 4 == 0 && (ny % 100 != 0 || ny % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    d = d.min(dim);
    format!("{ny:04}-{nm:02}-{d:02}{}", &utc[10..])
}

/// The schedule of a register CSV at `utc`.
pub fn schedule(register: &str, policy: &Policy, utc: &str) -> Result<Schedule, String> {
    if policy.interval_months == 0 || policy.warn_days.is_nan() || policy.warn_days < 0.0 {
        return Err("interval_months must be positive and warn_days zero or positive".into());
    }
    let now = parse_utc(utc).ok_or("bad --utc")?;
    let t = Table::parse(register)?;
    let (cp, cm, cs) = (
        t.need("product_id")?,
        t.need("method_id")?,
        t.need("scored_utc")?,
    );
    let mut last: std::collections::BTreeMap<(String, String), f64> = Default::default();
    for i in 0..t.rows.len() {
        let s = t.text(i, Some(cs));
        let ts = parse_utc(s).ok_or_else(|| format!("row {}: bad scored_utc '{s}'", i + 2))?;
        let k = (
            t.text(i, Some(cp)).to_string(),
            t.text(i, Some(cm)).to_string(),
        );
        let e = last.entry(k).or_insert(ts);
        *e = e.max(ts);
    }
    let mut items: Vec<Due> = last
        .into_iter()
        .map(|((p, m), ts)| {
            let next = add_months(&iso(ts), policy.interval_months);
            let left = (parse_utc(&next).unwrap_or(ts) - now) / 86_400.0;
            Due {
                product_id: p,
                method_id: m,
                last_scored_utc: iso(ts),
                next_due_utc: next,
                days_left: crate::forensics::inputs::round(left, 1),
                status: if left < 0.0 {
                    "OVERDUE"
                } else if left <= policy.warn_days {
                    "DUE SOON"
                } else {
                    "SCHEDULED"
                },
            }
        })
        .collect();
    items.sort_by(|a, b| a.days_left.total_cmp(&b.days_left));
    let c = |s| items.iter().filter(|x| x.status == s).count();
    let verdict = format!(
        "{} product-method pair(s): {} overdue, {} due within {} days, {} scheduled.",
        items.len(),
        c("OVERDUE"),
        c("DUE SOON"),
        policy.warn_days,
        c("SCHEDULED")
    );
    Ok(Schedule {
        format: SCHEDULE_FORMAT,
        at_utc: utc.into(),
        policy: policy.clone(),
        items,
        verdict,
    })
}

/// How a Type B contribution is stated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Standard,
    Expanded95,
    RectangularHalfWidth,
}

/// A Type B contribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeB {
    pub name: String,
    pub value: f64,
    pub kind: Kind,
    #[serde(default = "d_c")]
    pub sensitivity: f64,
    /// Degrees of freedom; omitted means infinite.
    #[serde(default)]
    pub dof: Option<f64>,
    pub source: String,
}

fn d_c() -> f64 {
    1.0
}

/// One test of a method.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    pub id: String,
    pub quantity: String,
    #[serde(default)]
    pub unit: String,
    /// Repeated results under repeatability conditions (Type A).
    #[serde(default)]
    pub runs: Vec<f64>,
    #[serde(default)]
    pub type_b: Vec<TypeB>,
    /// A reference value and its expanded (95 %) uncertainty, for the bias.
    #[serde(default)]
    pub reference: Option<f64>,
    #[serde(default)]
    pub reference_u95: Option<f64>,
}

/// The method request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodRequest {
    pub method_id: String,
    pub title: String,
    pub tests: Vec<Test>,
}

/// One line of a budget.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Line {
    pub name: String,
    pub kind: String,
    pub standard_uncertainty: f64,
    pub sensitivity: f64,
    pub contribution: f64,
    pub dof: Option<f64>,
    pub share: f64,
    pub source: String,
}

/// The budget of one test.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Budget {
    pub test_id: String,
    pub quantity: String,
    pub unit: String,
    pub mean: Option<f64>,
    pub lines: Vec<Line>,
    pub combined_standard_uncertainty: f64,
    /// Welch-Satterthwaite; `None` when infinite.
    pub effective_dof: Option<f64>,
    pub coverage_factor: f64,
    pub expanded_uncertainty: f64,
    pub bias: Option<f64>,
    pub en: Option<f64>,
}

/// The method report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method_id: String,
    pub title: String,
    pub statement: String,
    pub basis: &'static str,
    pub budgets: Vec<Budget>,
    pub verdict: String,
}

/// Two-sided 95 % Student t quantile for `dof` degrees of freedom (bisection).
pub fn t95(dof: f64) -> f64 {
    if !dof.is_finite() || dof > 1e7 {
        return 1.959_963_984_540_054;
    }
    let (mut lo, mut hi) = (0.0, 1000.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if student_t_two_sided(mid, dof) > 0.05 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// The budget of one test.
pub fn budget(t: &Test) -> Result<Budget, String> {
    let mut lines = Vec::new();
    let n = t.runs.len();
    let mean = (n > 0).then(|| t.runs.iter().sum::<f64>() / n as f64);
    if n == 1 {
        return Err(format!(
            "test '{}': one run gives no Type A estimate; give two or more or none",
            t.id
        ));
    }
    if let Some(m) = mean {
        if n >= 2 {
            let s = (t.runs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt();
            let u = s / (n as f64).sqrt();
            lines.push(Line {
                name: "repeatability (Type A)".into(),
                kind: "type-a".into(),
                standard_uncertainty: u,
                sensitivity: 1.0,
                contribution: u,
                dof: Some((n - 1) as f64),
                share: 0.0,
                source: format!("{n} runs"),
            });
        }
    }
    for b in &t.type_b {
        if b.value.is_nan() || b.value < 0.0 || b.source.trim().is_empty() {
            return Err(format!(
                "test '{}': contribution '{}' needs a value >= 0 and a source",
                t.id, b.name
            ));
        }
        let u = match b.kind {
            Kind::Standard => b.value,
            Kind::Expanded95 => b.value / 1.959_963_984_540_054,
            Kind::RectangularHalfWidth => b.value / 3f64.sqrt(),
        };
        lines.push(Line {
            name: b.name.clone(),
            kind: format!("{:?}", b.kind).to_lowercase(),
            standard_uncertainty: u,
            sensitivity: b.sensitivity,
            contribution: (b.sensitivity * u).abs(),
            dof: b.dof,
            share: 0.0,
            source: b.source.clone(),
        });
    }
    if lines.is_empty() {
        return Err(format!("test '{}': no contribution", t.id));
    }
    let uc2: f64 = lines.iter().map(|l| l.contribution.powi(2)).sum();
    let uc = uc2.sqrt();
    for l in &mut lines {
        l.share = if uc2 > 0.0 {
            l.contribution.powi(2) / uc2
        } else {
            0.0
        };
    }
    let denom: f64 = lines
        .iter()
        .filter_map(|l| l.dof.map(|d| l.contribution.powi(4) / d))
        .sum();
    let veff = (denom > 0.0).then(|| uc.powi(4) / denom);
    let k = t95(veff.unwrap_or(f64::INFINITY));
    let (bias, en) = match (mean, t.reference) {
        (Some(m), Some(r)) => {
            let b = m - r;
            let ur = t.reference_u95.unwrap_or(0.0);
            let den = ((k * uc).powi(2) + ur * ur).sqrt();
            (Some(b), (den > 0.0).then(|| b / den))
        }
        _ => (None, None),
    };
    Ok(Budget {
        test_id: t.id.clone(),
        quantity: t.quantity.clone(),
        unit: t.unit.clone(),
        mean,
        lines,
        combined_standard_uncertainty: uc,
        effective_dof: veff,
        coverage_factor: k,
        expanded_uncertainty: k * uc,
        bias,
        en,
    })
}

/// The method report.
pub fn report(q: &MethodRequest) -> Result<Report, String> {
    if q.tests.is_empty() {
        return Err("at least one [[tests]] entry is needed".into());
    }
    let budgets = q.tests.iter().map(budget).collect::<Result<Vec<_>, _>>()?;
    let bad = budgets
        .iter()
        .filter(|b| b.en.is_some_and(|e| e.abs() > 1.0))
        .count();
    Ok(Report {
        format: REPORT_FORMAT,
        method_id: q.method_id.clone(),
        title: q.title.clone(),
        statement: format!(
            "How well tests are measured under method {}; results of products are \"scored under method {}\". This report does not rate any product.",
            q.method_id, q.method_id
        ),
        basis: "JCGM 100:2008 (GUM) sections 4-6 and Annex G: root-sum-square of standard uncertainties, Welch-Satterthwaite effective degrees of freedom, Student t coverage factor for 95 %.",
        verdict: format!(
            "{} test(s) budgeted; {} with |E_n| > 1 against its reference.",
            budgets.len(),
            bad
        ),
        budgets,
    })
}

/// HTML page of a method report.
pub fn html(r: &Report) -> String {
    let sections: Vec<crate::timing::grid::Section> = r
        .budgets
        .iter()
        .map(|b| {
            let mut rows: Vec<Vec<String>> = b
                .lines
                .iter()
                .map(|l| {
                    vec![
                        l.name.clone(),
                        l.kind.clone(),
                        format!("{:.4e}", l.contribution),
                        l.dof
                            .map(|d| format!("{d:.1}"))
                            .unwrap_or_else(|| "inf".into()),
                        format!("{:.1} %", 100.0 * l.share),
                        l.source.clone(),
                    ]
                })
                .collect();
            rows.push(vec![
                "combined (k, U)".into(),
                String::new(),
                format!("{:.4e}", b.combined_standard_uncertainty),
                b.effective_dof
                    .map(|d| format!("{d:.1}"))
                    .unwrap_or_else(|| "inf".into()),
                format!("k = {:.3}", b.coverage_factor),
                format!("U = {:.4e} {}", b.expanded_uncertainty, b.unit),
            ]);
            (
                format!("{} ({})", b.test_id, b.quantity),
                vec!["contribution", "kind", "c·u", "dof", "share", "source"],
                rows,
            )
        })
        .collect();
    crate::transport::page(
        &r.title,
        &r.statement,
        &r.verdict,
        &[r.basis.to_string()],
        &sections,
    )
}
