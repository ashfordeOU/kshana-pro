// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Business-interruption and loss quantification (B2803)
//!
//! Five small models, each a closed-form calculation on the user's operating data with every
//! assumption listed and a one-at-a-time sensitivity table (each parameter moved by
//! ±`step_pct` %, the headline figure recomputed, rows sorted by swing):
//!
//! * `timing`: telecom or finance BI from a timing outage (SVN-23 class). Hours with
//!   |time error| above the user's tolerance come from a time-error log (CSV
//!   `utc,time_error_ns`) or a UTC-level step scenario (preset `svn23`: the 13 µs error of
//!   26 January 2016, duration the scenario's); loss = (outage + recovery hours) × revenue per
//!   hour × affected fraction + breaches × penalty + recovery hours × recovery cost per hour.
//! * `yield`: a precision-agriculture outage in an operating window: area delayed, days of
//!   delay, yield loss at the user's agronomic loss per day of delay, idle cost.
//! * `port`: vessel delay (from a CSV `vessel,delay_h` or vessels × mean delay) at the
//!   demurrage rate, plus berth productivity lost during the outage.
//! * `grounding`: a severity index from the kinetic energy at grounding (½ m v²) scaled by the
//!   user's bottom and environment factors, banded, and a cost from the user's cost table.
//! * `cascade`: fleet-cascade risk: the number of units of a fleet affected in one period under
//!   a beta-binomial model (per-unit probability, intra-fleet correlation); the headline is the
//!   1-in-100 (99th percentile) loss.
//!
//! Every coefficient is the user's; defaults are stated where a parameter has one. Evidence:
//! **MODELLED**: arithmetic on the inputs supplied, not a loss adjustment or a claim figure.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{esc, fingerprint, html_table, num, page, read_input, sig9, utc, InputRef};

pub const FORMAT: &str = "kshana-pro-bi-quantification/1";
pub const METHOD_ID: &str = "kshana-pro/bi-quantification/v1";
pub const RESULT_FILE: &str = "bi.json";
pub const LABEL: &str = "MODELLED: a calculation on the operating data and coefficients supplied, \
with its assumptions and sensitivity; not a loss adjustment, a claim figure or financial advice.";

/// The models.
pub const MODELS: &[&str] = &["timing", "yield", "port", "grounding", "cascade"];

fn d_step() -> f64 {
    20.0
}

/// A BI request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiRequest {
    pub title: String,
    pub model: String,
    pub currency: String,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    #[serde(default = "d_step")]
    pub step_pct: f64,
    /// Timing: `svn23` sets step_ns and duration_h (explicit params override).
    #[serde(default)]
    pub preset: Option<String>,
    /// Timing: CSV `utc,time_error_ns`.
    #[serde(default)]
    pub te_log: Option<String>,
    /// Any model with `outage_h`: CSV `start_utc,end_utc`, summed.
    #[serde(default)]
    pub outage_log: Option<String>,
    /// Port: CSV `vessel,delay_h`.
    #[serde(default)]
    pub vessels: Option<String>,
}

/// One parameter as used.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParamRow {
    pub name: String,
    pub value: f64,
    pub unit: String,
    /// `stated`, `default`, `preset` or `derived from <file>`.
    pub source: String,
    pub meaning: String,
}

/// One sensitivity row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SensRow {
    pub param: String,
    pub low: f64,
    pub high: f64,
    pub headline_low: f64,
    pub headline_high: f64,
    pub swing: f64,
}

/// A BI result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BiResult {
    pub format: String,
    pub method: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub model: String,
    pub currency: String,
    pub params: Vec<ParamRow>,
    pub headline_name: String,
    pub headline: f64,
    pub parts: BTreeMap<String, f64>,
    pub assumptions: Vec<String>,
    pub step_pct: f64,
    pub sensitivity: Vec<SensRow>,
    pub inputs: Vec<InputRef>,
    pub notes: Vec<String>,
    pub fingerprint: String,
}

/// A parameter of a model: name, unit, default, integer, bounded to [0, 1], meaning.
pub type Spec = (
    &'static str,
    &'static str,
    Option<f64>,
    bool,
    bool,
    &'static str,
);

pub fn specs(model: &str) -> &'static [Spec] {
    match model {
        "timing" => &[
            (
                "tolerance_ns",
                "ns",
                None,
                false,
                false,
                "the service's time-error tolerance (the user's own requirement)",
            ),
            (
                "outage_h",
                "h",
                None,
                false,
                false,
                "hours with |time error| above the tolerance",
            ),
            (
                "breaches",
                "count",
                Some(1.0),
                true,
                false,
                "separate tolerance breaches",
            ),
            (
                "revenue_per_hour",
                "currency/h",
                None,
                false,
                false,
                "revenue or margin earned by the affected service per hour",
            ),
            (
                "affected_fraction",
                "0-1",
                Some(1.0),
                false,
                true,
                "share of that revenue lost while out of tolerance",
            ),
            (
                "penalty_per_breach",
                "currency",
                Some(0.0),
                false,
                false,
                "contractual or regulatory penalty per breach",
            ),
            (
                "recovery_h",
                "h",
                Some(0.0),
                false,
                false,
                "hours to restore service after time is back in tolerance",
            ),
            (
                "recovery_cost_per_hour",
                "currency/h",
                Some(0.0),
                false,
                false,
                "staff and call-out cost per recovery hour",
            ),
        ],
        "yield" => &[
            (
                "area_ha",
                "ha",
                None,
                false,
                false,
                "area due for the operation in the window",
            ),
            (
                "outage_h",
                "h",
                None,
                false,
                false,
                "hours of correction-service or GNSS outage in working time",
            ),
            (
                "operating_rate_ha_per_h",
                "ha/h",
                None,
                false,
                false,
                "area worked per hour",
            ),
            (
                "working_hours_per_day",
                "h",
                Some(10.0),
                false,
                false,
                "working hours per day",
            ),
            (
                "yield_t_per_ha",
                "t/ha",
                None,
                false,
                false,
                "expected yield",
            ),
            (
                "price_per_t",
                "currency/t",
                None,
                false,
                false,
                "crop price",
            ),
            (
                "yield_loss_pct_per_day",
                "%/day",
                None,
                false,
                false,
                "yield lost per day of delay (the user's agronomic coefficient)",
            ),
            (
                "idle_cost_per_hour",
                "currency/h",
                Some(0.0),
                false,
                false,
                "crew and machinery idle cost",
            ),
        ],
        "port" => &[
            (
                "outage_h",
                "h",
                None,
                false,
                false,
                "hours the port worked under the outage",
            ),
            (
                "vessels_affected",
                "count",
                Some(0.0),
                true,
                false,
                "vessels delayed (when no vessel file is given)",
            ),
            (
                "mean_delay_h",
                "h",
                Some(0.0),
                false,
                false,
                "mean delay per vessel (when no vessel file is given)",
            ),
            (
                "vessel_delay_h",
                "h",
                Some(0.0),
                false,
                false,
                "total vessel delay (derived from the vessel file when given)",
            ),
            (
                "demurrage_per_day",
                "currency/day",
                None,
                false,
                false,
                "demurrage or delay cost per vessel-day",
            ),
            (
                "berth_loss_per_hour",
                "currency/h",
                Some(0.0),
                false,
                false,
                "berth productivity lost per outage hour",
            ),
        ],
        "grounding" => &[
            (
                "displacement_t",
                "t",
                None,
                false,
                false,
                "displacement at grounding",
            ),
            (
                "speed_kn",
                "kn",
                None,
                false,
                false,
                "speed over ground at grounding",
            ),
            (
                "bottom_factor",
                "0-1",
                None,
                false,
                true,
                "the user's bottom-type factor (soft sand low, rock high)",
            ),
            (
                "environment_factor",
                "x",
                Some(1.0),
                false,
                false,
                "the user's environmental sensitivity multiplier",
            ),
            (
                "salvage_base",
                "currency",
                Some(0.0),
                false,
                false,
                "fixed refloating and survey cost",
            ),
            (
                "cost_per_mj",
                "currency/MJ",
                None,
                false,
                false,
                "the user's cost per unit of severity index",
            ),
            (
                "off_hire_days",
                "days",
                Some(0.0),
                false,
                false,
                "days off hire",
            ),
            (
                "hire_per_day",
                "currency/day",
                Some(0.0),
                false,
                false,
                "hire or earnings per day",
            ),
        ],
        "cascade" => &[
            (
                "fleet_size",
                "units",
                None,
                true,
                false,
                "units in the fleet",
            ),
            (
                "p_unit",
                "0-1",
                None,
                false,
                true,
                "probability that one unit is affected in the period",
            ),
            (
                "correlation",
                "0-1",
                Some(0.0),
                false,
                true,
                "intra-fleet correlation (common exposure, same routes)",
            ),
            (
                "loss_per_unit",
                "currency",
                None,
                false,
                false,
                "loss per affected unit",
            ),
            (
                "threshold_units",
                "units",
                Some(2.0),
                true,
                false,
                "the cascade threshold: this many units or more at once",
            ),
        ],
        _ => &[],
    }
}

fn assumptions(model: &str) -> Vec<String> {
    let a: &[&str] = match model {
        "timing" => &[
            "The service is lost or degraded only while |time error| exceeds the stated tolerance.",
            "Revenue accrues evenly in time; the affected fraction applies throughout.",
            "Each separate excursion above the tolerance is one breach.",
            "A time-error log counts each sample's interval (capped at 600 s) above the tolerance.",
        ],
        "yield" => &[
            "The operation stops during the outage and resumes at the same rate afterwards.",
            "Delay days are outage hours over working hours per day.",
            "Yield loss is linear in delay at the stated rate, capped at 100 %, on the area delayed only.",
        ],
        "port" => &[
            "Each delayed vessel costs demurrage pro rata per hour of delay.",
            "Berth productivity loss accrues for every outage hour.",
        ],
        "grounding" => &[
            "Severity scales with kinetic energy at grounding, 0.5 m v^2, in MJ.",
            "Bottom and environment factors are the user's; bands are engineering choices: under 1 minor, under 10 moderate, under 100 serious, else severe.",
            "Cost is linear in the severity index plus a fixed base and off-hire.",
        ],
        "cascade" => &[
            "Units are exchangeable; their correlation is a beta-binomial intra-class correlation (0 = independent).",
            "The headline is the 99th percentile of the number of units affected times the loss per unit.",
        ],
        _ => &[],
    };
    a.iter().map(|s| s.to_string()).collect()
}

/// Beta-binomial pmf of k = 0..=n affected units.
pub fn beta_binomial(n: usize, p: f64, rho: f64) -> Vec<f64> {
    let mut pmf = vec![0.0; n + 1];
    if p <= 0.0 {
        pmf[0] = 1.0;
        return pmf;
    }
    if p >= 1.0 {
        pmf[n] = 1.0;
        return pmf;
    }
    if rho <= 1e-12 {
        pmf[0] = (1.0 - p).powi(n as i32);
        for k in 0..n {
            pmf[k + 1] = pmf[k] * (n - k) as f64 / (k + 1) as f64 * p / (1.0 - p);
        }
    } else {
        let r = rho.min(1.0 - 1e-9);
        let (a, b) = (p * (1.0 - r) / r, (1.0 - p) * (1.0 - r) / r);
        pmf[0] = (0..n)
            .map(|i| (b + i as f64) / (a + b + i as f64))
            .product();
        for k in 0..n {
            pmf[k + 1] = pmf[k] * (n - k) as f64 / (k + 1) as f64 * (k as f64 + a)
                / ((n - k - 1) as f64 + b);
        }
    }
    let s: f64 = pmf.iter().sum();
    pmf.iter().map(|x| x / s).collect()
}

/// Compute the headline and its parts from the parameters.
fn compute(model: &str, p: &BTreeMap<String, f64>) -> (f64, BTreeMap<String, f64>) {
    let g = |k: &str| p.get(k).copied().unwrap_or(0.0);
    let mut parts = BTreeMap::new();
    let mut put = |k: &str, v: f64| {
        parts.insert(k.to_string(), sig9(v));
        v
    };
    let headline = match model {
        "timing" => {
            let lost = put(
                "revenue_lost",
                (g("outage_h") + g("recovery_h")) * g("revenue_per_hour") * g("affected_fraction"),
            );
            let pen = put("penalties", g("breaches") * g("penalty_per_breach"));
            let rec = put(
                "recovery_cost",
                g("recovery_h") * g("recovery_cost_per_hour"),
            );
            lost + pen + rec
        }
        "yield" => {
            let area = put(
                "area_delayed_ha",
                g("area_ha").min(g("outage_h") * g("operating_rate_ha_per_h")),
            );
            let days = put(
                "delay_days",
                if g("working_hours_per_day") > 0.0 {
                    g("outage_h") / g("working_hours_per_day")
                } else {
                    0.0
                },
            );
            let frac = (g("yield_loss_pct_per_day") / 100.0 * days).min(1.0);
            let y = put(
                "yield_loss",
                area * g("yield_t_per_ha") * g("price_per_t") * frac,
            );
            let idle = put("idle_cost", g("outage_h") * g("idle_cost_per_hour"));
            y + idle
        }
        "port" => {
            let vd = if g("vessel_delay_h") > 0.0 {
                g("vessel_delay_h")
            } else {
                g("vessels_affected") * g("mean_delay_h")
            };
            put("vessel_delay_h_used", vd);
            let dem = put("demurrage", vd / 24.0 * g("demurrage_per_day"));
            let berth = put("berth_loss", g("outage_h") * g("berth_loss_per_hour"));
            dem + berth
        }
        "grounding" => {
            let v = g("speed_kn") * 0.514_444;
            let ke = put(
                "kinetic_energy_mj",
                0.5 * g("displacement_t") * 1000.0 * v * v / 1e6,
            );
            let idx = put(
                "severity_index",
                ke * g("bottom_factor") * g("environment_factor"),
            );
            put(
                "severity_band",
                match idx {
                    x if x < 1.0 => 1.0,
                    x if x < 10.0 => 2.0,
                    x if x < 100.0 => 3.0,
                    _ => 4.0,
                },
            );
            let c = put("damage_cost", g("salvage_base") + g("cost_per_mj") * idx);
            let h = put("off_hire_cost", g("off_hire_days") * g("hire_per_day"));
            c + h
        }
        "cascade" => {
            let n = g("fleet_size").max(0.0) as usize;
            let pmf = beta_binomial(n, g("p_unit"), g("correlation"));
            let thr = g("threshold_units").max(0.0) as usize;
            put("expected_units", n as f64 * g("p_unit"));
            put("p_cascade", pmf.iter().skip(thr.min(n + 1)).sum::<f64>());
            put("expected_loss", n as f64 * g("p_unit") * g("loss_per_unit"));
            let mut c = 0.0;
            let mut q99 = n;
            for (k, x) in pmf.iter().enumerate() {
                c += x;
                if c >= 0.99 - 1e-12 {
                    q99 = k;
                    break;
                }
            }
            put("units_p99", q99 as f64);
            put("loss_p99", q99 as f64 * g("loss_per_unit"))
        }
        _ => 0.0,
    };
    (headline, parts)
}

fn headline_name(model: &str) -> &'static str {
    match model {
        "cascade" => "1-in-100 fleet loss (99th percentile)",
        "grounding" => "modelled grounding cost",
        _ => "modelled business-interruption loss",
    }
}

fn csv(text: &str, name: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head = lines
        .next()
        .ok_or_else(|| format!("{name}: empty"))?
        .split(',')
        .map(|c| c.trim().to_ascii_lowercase())
        .collect();
    Ok((
        head,
        lines
            .map(|l| l.split(',').map(|c| c.trim().to_string()).collect())
            .collect(),
    ))
}

fn idx(h: &[String], c: &str, n: &str) -> Result<usize, String> {
    h.iter()
        .position(|x| x == c)
        .ok_or_else(|| format!("{n}: no '{c}' column"))
}

/// Hours above `tol_ns` and the number of separate excursions in a time-error log.
pub fn te_exceedance(rows: &[(f64, f64)], tol_ns: f64) -> (f64, usize) {
    let (mut s, mut n, mut prev) = (0.0, 0, false);
    for i in 0..rows.len() {
        let over = rows[i].1.abs() > tol_ns;
        let dt = rows
            .get(i + 1)
            .map(|x| (x.0 - rows[i].0).min(600.0))
            .unwrap_or(0.0);
        if over {
            s += dt;
            if !prev {
                n += 1;
            }
        }
        prev = over;
    }
    (s / 3600.0, n)
}

/// Run the model. `path` is the request file.
pub fn run(path: &Path) -> Result<(BiResult, String), String> {
    let (r, src, base): (BiRequest, _, _) = super::read_request(path)?;
    quantify(&r, &src, &base)
}

/// Run the model on a parsed request.
pub fn quantify(
    r: &BiRequest,
    src: &str,
    base: &Option<PathBuf>,
) -> Result<(BiResult, String), String> {
    if !MODELS.contains(&r.model.as_str()) {
        return Err(format!(
            "unknown model '{}' ({})",
            r.model,
            MODELS.join(", ")
        ));
    }
    let sp = specs(&r.model);
    for k in r.params.keys() {
        if !sp.iter().any(|s| s.0 == k) {
            return Err(format!("{}: unknown parameter '{k}'", r.model));
        }
    }
    let mut vals: BTreeMap<String, f64> = BTreeMap::new();
    let mut source: BTreeMap<String, String> = BTreeMap::new();
    let mut inputs = vec![];
    let mut notes = vec![];
    // Data-derived values first; stated params override nothing derived from data.
    if let Some(n) = &r.outage_log {
        let (b, rf) = read_input(base, n, "outage_log")?;
        let (h, rows) = csv(&String::from_utf8_lossy(&b), n)?;
        let (cs, ce) = (idx(&h, "start_utc", n)?, idx(&h, "end_utc", n)?);
        let mut hours = 0.0;
        for (i, row) in rows.iter().enumerate() {
            let w = format!("{n} row {}", i + 2);
            let g = |k: usize| row.get(k).map(String::as_str).unwrap_or("");
            let (a, e) = (utc(&w, g(cs))?, utc(&w, g(ce))?);
            hours += ((e - a) / 3600.0).max(0.0);
        }
        vals.insert("outage_h".into(), hours);
        source.insert("outage_h".into(), format!("derived from {n}"));
        inputs.push(rf);
    }
    if r.model == "timing" {
        let tol = r
            .params
            .get("tolerance_ns")
            .copied()
            .ok_or("timing: tolerance_ns is required (the service's own requirement)")?;
        if let Some(n) = &r.te_log {
            let (b, rf) = read_input(base, n, "te_log")?;
            let (h, rows) = csv(&String::from_utf8_lossy(&b), n)?;
            let (cu, ct) = (idx(&h, "utc", n)?, idx(&h, "time_error_ns", n)?);
            let mut pts = vec![];
            for (i, row) in rows.iter().enumerate() {
                let w = format!("{n} row {}", i + 2);
                let t = utc(&w, row.get(cu).map(String::as_str).unwrap_or(""))?;
                let v: f64 = row
                    .get(ct)
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| format!("{w}: time_error_ns is not a number"))?;
                pts.push((t, v));
            }
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (hrs, n_b) = te_exceedance(&pts, tol);
            for (k, v) in [("outage_h", hrs), ("breaches", n_b as f64)] {
                vals.insert(k.into(), v);
                source.insert(k.into(), format!("derived from {n}"));
            }
            inputs.push(rf);
        } else {
            let (step, dur, basis) = match r.preset.as_deref() {
                Some("svn23") => (
                    Some(crate::timing::sfn::SVN23_STEP_S * 1e9),
                    Some(12.0),
                    Some(crate::timing::sfn::SVN23_BASIS),
                ),
                Some(p) => return Err(format!("unknown preset '{p}' (svn23)")),
                None => (None, None, None),
            };
            if let Some(b) = basis {
                notes.push(b.to_string());
            }
            if !r.params.contains_key("outage_h") {
                let step = step.ok_or(
                    "timing: give outage_h, a te_log, or preset = \"svn23\" for a UTC-level step",
                )?;
                let h = if step > tol { dur.unwrap_or(0.0) } else { 0.0 };
                vals.insert("outage_h".into(), h);
                source.insert(
                    "outage_h".into(),
                    format!(
                        "preset: a {} ns step for {} h against {} ns tolerance",
                        step,
                        dur.unwrap_or(0.0),
                        tol
                    ),
                );
            }
        }
    }
    if r.model == "port" {
        if let Some(n) = &r.vessels {
            let (b, rf) = read_input(base, n, "vessels")?;
            let (h, rows) = csv(&String::from_utf8_lossy(&b), n)?;
            let cd = idx(&h, "delay_h", n)?;
            let mut s = 0.0;
            for (i, row) in rows.iter().enumerate() {
                s += row
                    .get(cd)
                    .and_then(|v| v.parse::<f64>().ok())
                    .ok_or_else(|| format!("{n} row {}: delay_h is not a number", i + 2))?;
            }
            vals.insert("vessel_delay_h".into(), s);
            source.insert("vessel_delay_h".into(), format!("derived from {n}"));
            inputs.push(rf);
        }
    }
    for (k, v) in &r.params {
        if source.get(k).is_some_and(|s| s.starts_with("derived")) {
            notes.push(format!(
                "parameter {k} is derived from data; the stated value was not used"
            ));
            continue;
        }
        vals.insert(k.clone(), *v);
        source.insert(k.clone(), "stated".into());
    }
    let mut params = vec![];
    for (name, unit, def, _, unit01, meaning) in sp {
        let v = match (vals.get(*name), def) {
            (Some(v), _) => *v,
            (None, Some(d)) => {
                vals.insert(name.to_string(), *d);
                source.insert(name.to_string(), "default".into());
                *d
            }
            (None, None) => return Err(format!("{}: parameter '{name}' is required", r.model)),
        };
        if !v.is_finite() || v < 0.0 || (*unit01 && v > 1.0) {
            return Err(format!(
                "{name} = {v}: must be finite, not negative{}",
                if *unit01 { " and at most 1" } else { "" }
            ));
        }
        params.push(ParamRow {
            name: name.to_string(),
            value: sig9(v),
            unit: unit.to_string(),
            source: source.get(*name).cloned().unwrap_or_default(),
            meaning: meaning.to_string(),
        });
    }
    let (headline, parts) = compute(&r.model, &vals);
    let step = r.step_pct / 100.0;
    let mut sensitivity = vec![];
    for (name, _, _, int, unit01, _) in sp {
        let base_v = vals[*name];
        if base_v == 0.0 {
            continue;
        }
        let adj = |f: f64| {
            let mut x = base_v * f;
            if *int {
                x = x.round();
            }
            if *unit01 {
                x = x.clamp(0.0, 1.0);
            }
            x
        };
        let (lo, hi) = (adj(1.0 - step), adj(1.0 + step));
        let at = |x: f64| {
            let mut v = vals.clone();
            v.insert(name.to_string(), x);
            compute(&r.model, &v).0
        };
        let (hl, hh) = (at(lo), at(hi));
        sensitivity.push(SensRow {
            param: name.to_string(),
            low: sig9(lo),
            high: sig9(hi),
            headline_low: sig9(hl),
            headline_high: sig9(hh),
            swing: sig9((hh - hl).abs()),
        });
    }
    sensitivity.sort_by(|a, b| b.swing.total_cmp(&a.swing).then(a.param.cmp(&b.param)));
    let mut res = BiResult {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        label: LABEL.into(),
        statement: super::STATEMENT.into(),
        title: r.title.clone(),
        model: r.model.clone(),
        currency: r.currency.clone(),
        params,
        headline_name: headline_name(&r.model).into(),
        headline: sig9(headline),
        parts,
        assumptions: assumptions(&r.model),
        step_pct: r.step_pct,
        sensitivity,
        inputs,
        notes,
        fingerprint: String::new(),
    };
    res.fingerprint = fingerprint(&res);
    Ok((res, src.to_string()))
}

/// The HTML page.
pub fn render_html(r: &BiResult) -> String {
    let mut b = format!(
        "<p><b>{}</b>: {} {}</p><h2>Parameters</h2>",
        esc(&r.headline_name),
        num(r.headline),
        esc(&r.currency)
    );
    b.push_str(&html_table(
        &["Parameter", "Value", "Unit", "Source", "Meaning"],
        &r.params
            .iter()
            .map(|p| {
                vec![
                    p.name.clone(),
                    num(p.value),
                    p.unit.clone(),
                    p.source.clone(),
                    p.meaning.clone(),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    b.push_str("<h2>Parts</h2>");
    b.push_str(&html_table(
        &["Part", "Value"],
        &r.parts
            .iter()
            .map(|(k, v)| vec![k.clone(), num(*v)])
            .collect::<Vec<_>>(),
    ));
    b.push_str("<h2>Assumptions</h2><ul>");
    for a in &r.assumptions {
        b.push_str(&format!("<li>{}</li>", esc(a)));
    }
    b.push_str(&format!(
        "</ul><h2>Sensitivity (each parameter ±{} %)</h2>",
        r.step_pct
    ));
    b.push_str(&html_table(
        &[
            "Parameter",
            "Low",
            "High",
            "Headline low",
            "Headline high",
            "Swing",
        ],
        &r.sensitivity
            .iter()
            .map(|s| {
                vec![
                    s.param.clone(),
                    num(s.low),
                    num(s.high),
                    num(s.headline_low),
                    num(s.headline_high),
                    num(s.swing),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    for n in &r.notes {
        b.push_str(&format!("<p class=\"l\">{}</p>", esc(n)));
    }
    b.push_str(&format!(
        "<p class=\"l\">Fingerprint {}</p>",
        esc(&r.fingerprint)
    ));
    page(&r.title, &r.label, &b)
}

/// The sensitivity table as CSV.
pub fn sensitivity_csv(r: &BiResult) -> String {
    let mut s = String::from("param,low,high,headline_low,headline_high,swing\n");
    for x in &r.sensitivity {
        s.push_str(&format!(
            "{},{},{},{},{},{}\n",
            x.param, x.low, x.high, x.headline_low, x.headline_high, x.swing
        ));
    }
    s
}

/// Write the result directory.
pub fn write_dir(out: &Path, r: &BiResult, request_src: &str) -> Result<(), String> {
    super::write_pack(out, RESULT_FILE, r, &render_html(r), request_src)?;
    super::write_text(out, "sensitivity.csv", sensitivity_csv(r).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beta_binomial_sums_to_one_and_reduces_to_binomial() {
        let b = beta_binomial(4, 0.5, 0.0);
        assert!((b[2] - 0.375).abs() < 1e-12);
        let c = beta_binomial(10, 0.1, 0.3);
        assert!((c.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        // Correlation fattens the tail.
        let i = beta_binomial(10, 0.1, 0.0);
        assert!(c[5..].iter().sum::<f64>() > i[5..].iter().sum::<f64>());
        // Mean is n p either way.
        let m: f64 = c.iter().enumerate().map(|(k, x)| k as f64 * x).sum();
        assert!((m - 1.0).abs() < 1e-9);
    }

    #[test]
    fn te_exceedance_counts_hours_and_breaches() {
        let rows: Vec<(f64, f64)> = (0..10)
            .map(|i| {
                (
                    i as f64 * 60.0,
                    if (3..6).contains(&i) { 2000.0 } else { 0.0 },
                )
            })
            .collect();
        let (h, n) = te_exceedance(&rows, 1500.0);
        assert!((h - 3.0 / 60.0).abs() < 1e-12);
        assert_eq!(n, 1);
    }
}
