// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Simulator total cost of ownership (format [`FORMAT`]): buying, keeping a legacy unit in
//! service, renting per campaign or buying test as a service, compared over the same years
//! and campaign count, on the user's own quotes.
//!
//! ```toml
//! format = "kshana-simlab-tco/1"
//! currency = "USD"
//! years = 5
//! discount_rate = 0.06          # per year, for the present value
//! campaigns_per_year = 4
//! [[option]]
//! name = "Buy a multi-constellation simulator"
//! purchase = 250000             # year 0
//! annual_maintenance = 61000    # years 1..N
//! annual_staff = 15000
//! per_campaign = 2000           # consumables, travel, lab time
//! residual_value = 25000        # recovered at year N
//! ```
//!
//! Per option: the nominal total, the present value
//! `PV = purchase + Σ_{y=1..N} (annual + per_campaign·n) / (1 + i)^y − residual / (1 + i)^N`,
//! the cost per campaign (`PV / (n·N)`) and the equivalent annual cost
//! (`PV · i / (1 − (1 + i)^−N)`). Between each pair, the **break-even campaign rate**: the
//! campaigns per year at which their present values are equal (none when one is cheaper at
//! every rate). No price is built in: every figure is the user's.

use serde::{Deserialize, Serialize};

use super::r;

pub const FORMAT: &str = "kshana-simlab-tco/1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostOption {
    pub name: String,
    #[serde(default)]
    pub purchase: f64,
    #[serde(default)]
    pub annual_maintenance: f64,
    #[serde(default)]
    pub annual_staff: f64,
    #[serde(default)]
    pub annual_other: f64,
    #[serde(default)]
    pub per_campaign: f64,
    #[serde(default)]
    pub residual_value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tco {
    pub format: String,
    #[serde(default)]
    pub currency: String,
    pub years: u32,
    pub discount_rate: f64,
    pub campaigns_per_year: f64,
    #[serde(rename = "option")]
    pub options: Vec<CostOption>,
    /// Source notes for the figures (shown in the result).
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Tco {
    pub fn parse(src: &str) -> Result<Tco, String> {
        let t: Tco = toml::from_str(src).map_err(|e| format!("tco: {e}"))?;
        if t.format != FORMAT {
            return Err(format!("tco: format '{}' (expected {FORMAT})", t.format));
        }
        if t.years == 0 || t.years > 50 {
            return Err("tco: years must be 1 to 50".into());
        }
        if !(0.0..1.0).contains(&t.discount_rate) || t.campaigns_per_year < 0.0 {
            return Err("tco: discount_rate in [0, 1) and campaigns_per_year ≥ 0".into());
        }
        if t.options.len() < 2 {
            return Err("tco: give at least two [[option]]s to compare".into());
        }
        for o in &t.options {
            let v = [
                o.purchase,
                o.annual_maintenance,
                o.annual_staff,
                o.annual_other,
                o.per_campaign,
                o.residual_value,
            ];
            if v.iter().any(|x| !x.is_finite() || *x < 0.0) {
                return Err(format!("tco: option '{}': amounts must be ≥ 0", o.name));
            }
        }
        Ok(t)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OptionResult {
    pub name: String,
    pub nominal_total: f64,
    pub present_value: f64,
    pub per_campaign: Option<f64>,
    pub equivalent_annual: f64,
    pub rank: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BreakEven {
    pub a: String,
    pub b: String,
    /// Campaigns per year at which A and B cost the same; `None` when one is cheaper at
    /// every rate.
    pub campaigns_per_year: Option<f64>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TcoReport {
    pub label: &'static str,
    pub currency: String,
    pub years: u32,
    pub discount_rate: f64,
    pub campaigns_per_year: f64,
    pub options: Vec<OptionResult>,
    pub break_even: Vec<BreakEven>,
    pub notes: Vec<String>,
}

pub const LABEL: &str = "MODELLED: arithmetic over the figures supplied; no price is built in. \
Not a quotation and not financial advice.";

/// Present-value factor of a level annual amount over years 1..N.
fn annuity(i: f64, n: u32) -> f64 {
    if i == 0.0 {
        n as f64
    } else {
        (1.0 - (1.0 + i).powi(-(n as i32))) / i
    }
}

/// `(fixed PV, PV per campaign-per-year)` of an option.
fn pv_parts(o: &CostOption, i: f64, n: u32) -> (f64, f64) {
    let a = annuity(i, n);
    let fixed = o.purchase + (o.annual_maintenance + o.annual_staff + o.annual_other) * a
        - o.residual_value / (1.0 + i).powi(n as i32);
    (fixed, o.per_campaign * a)
}

pub fn compute(t: &Tco) -> TcoReport {
    let i = t.discount_rate;
    let n = t.years;
    let k = t.campaigns_per_year;
    let mut res: Vec<OptionResult> = t
        .options
        .iter()
        .map(|o| {
            let (f, v) = pv_parts(o, i, n);
            let pv = f + v * k;
            let nominal = o.purchase
                + (o.annual_maintenance + o.annual_staff + o.annual_other + o.per_campaign * k)
                    * n as f64
                - o.residual_value;
            OptionResult {
                name: o.name.clone(),
                nominal_total: r(nominal, 2),
                present_value: r(pv, 2),
                per_campaign: (k > 0.0).then(|| r(pv / (k * n as f64), 2)),
                equivalent_annual: r(pv / annuity(i, n), 2),
                rank: 0,
            }
        })
        .collect();
    let mut order: Vec<usize> = (0..res.len()).collect();
    order.sort_by(|a, b| res[*a].present_value.total_cmp(&res[*b].present_value));
    for (rank, idx) in order.iter().enumerate() {
        res[*idx].rank = rank + 1;
    }
    let mut be = Vec::new();
    for x in 0..t.options.len() {
        for y in x + 1..t.options.len() {
            let (fa, va) = pv_parts(&t.options[x], i, n);
            let (fb, vb) = pv_parts(&t.options[y], i, n);
            let (a, b) = (&t.options[x].name, &t.options[y].name);
            let (rate, detail) = if (va - vb).abs() < 1e-9 {
                (
                    None,
                    format!("same cost per campaign: the one with the lower fixed cost ({}) is cheaper at every rate", if fa <= fb { a } else { b }),
                )
            } else {
                let c = (fb - fa) / (va - vb);
                if c > 0.0 {
                    let cheap_low = if fa < fb { a } else { b };
                    let cheap_high = if fa < fb { b } else { a };
                    (
                        Some(r(c, 3)),
                        format!("below {c:.2} campaigns a year {cheap_low} costs less; above it {cheap_high}"),
                    )
                } else {
                    let w = if fa + va <= fb + vb { a } else { b };
                    (None, format!("{w} costs less at every campaign rate"))
                }
            };
            be.push(BreakEven {
                a: a.clone(),
                b: b.clone(),
                campaigns_per_year: rate,
                detail,
            });
        }
    }
    TcoReport {
        label: LABEL,
        currency: t.currency.clone(),
        years: n,
        discount_rate: i,
        campaigns_per_year: k,
        options: res,
        break_even: be,
        notes: t.notes.clone(),
    }
}

pub fn markdown(r: &TcoReport) -> String {
    let mut s = format!(
        "# Simulator total cost of ownership\n\n{} years, {} campaigns a year, discount rate {} %. Amounts in {}.\n\n| rank | option | nominal total | present value | per campaign | equivalent annual |\n|---|---|---|---|---|---|\n",
        r.years,
        r.campaigns_per_year,
        r.discount_rate * 100.0,
        if r.currency.is_empty() { "the currency of the inputs" } else { &r.currency }
    );
    let mut o = r.options.clone();
    o.sort_by_key(|x| x.rank);
    for x in &o {
        s.push_str(&format!(
            "| {} | {} | {:.0} | {:.0} | {} | {:.0} |\n",
            x.rank,
            x.name,
            x.nominal_total,
            x.present_value,
            x.per_campaign
                .map(|v| format!("{v:.0}"))
                .unwrap_or_else(|| "-".into()),
            x.equivalent_annual
        ));
    }
    s.push_str("\n## Break-even\n\n");
    for b in &r.break_even {
        s.push_str(&format!("- {} vs {}: {}\n", b.a, b.b, b.detail));
    }
    if !r.notes.is_empty() {
        s.push_str("\n## Sources of the figures\n\n");
        for n in &r.notes {
            s.push_str(&format!("- {n}\n"));
        }
    }
    s.push_str(&format!("\n{}\n", r.label));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_even_between_buying_and_renting() {
        let t = Tco::parse(
            r#"format = "kshana-simlab-tco/1"
years = 1
discount_rate = 0.0
campaigns_per_year = 2
[[option]]
name = "buy"
purchase = 100
per_campaign = 10
[[option]]
name = "rent"
per_campaign = 60
"#,
        )
        .unwrap();
        let rep = compute(&t);
        assert_eq!(rep.options[0].present_value, 120.0);
        assert_eq!(rep.options[1].present_value, 120.0);
        assert_eq!(rep.break_even[0].campaigns_per_year, Some(2.0));
        assert!((annuity(0.1, 2) - 1.735537).abs() < 1e-6);
    }
}
