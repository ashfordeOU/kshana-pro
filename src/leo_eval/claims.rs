// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Investor due-diligence report template on constellation claims (B2108, Pro part).
//!
//! The open claim datasets and benchmarks are built in the open engine; this module takes a
//! claim dataset (`kshana-pro-leo-claims/1`: quantity, value, unit, conditions, and the source
//! URL, date and quoted words of each claim) and measured results, and writes an investor
//! due-diligence report template. A value without a source URL, date and quote is refused;
//! every verdict in the report cites its source and carries its label (`REPRODUCED`,
//! `NOT REPRODUCED` or `NOT TESTED`, all MODELLED unless the measurement names its validation
//! data). Kshana cites nothing itself and names no provider as a competitor.

use serde::{Deserialize, Serialize};

/// Format.
pub const FORMAT: &str = "kshana-pro-leo-claims/1";

/// One claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub quantity: String,
    pub unit: String,
    /// `max` (the claim is an upper bound, e.g. an error) or `min` (a lower bound).
    pub direction: String,
    pub value: Option<f64>,
    #[serde(default)]
    pub conditions: String,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub source_date: Option<String>,
    #[serde(default)]
    pub quote: Option<String>,
}

/// A claim dataset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dataset {
    pub format: String,
    pub constellation: String,
    pub operator: String,
    pub claims: Vec<Claim>,
}

/// Problems with a dataset (refusals) and the number of placeholder claims.
pub fn check(d: &Dataset) -> Result<usize, String> {
    if d.format != FORMAT {
        return Err(format!("format '{}' is not {FORMAT}", d.format));
    }
    let mut placeholders = 0;
    for c in &d.claims {
        if !["max", "min"].contains(&c.direction.as_str()) {
            return Err(format!("claim '{}': direction must be max or min", c.id));
        }
        match c.value {
            None => placeholders += 1,
            Some(_) => {
                let ok = |x: &Option<String>| {
                    x.as_deref()
                        .is_some_and(|s| !s.trim().is_empty() && !s.contains("PLACEHOLDER"))
                };
                if !(ok(&c.source_url) && ok(&c.source_date) && ok(&c.quote)) {
                    return Err(format!(
                        "claim '{}' of {}: a value needs its source_url, source_date and quote",
                        c.id, d.constellation
                    ));
                }
            }
        }
    }
    Ok(placeholders)
}

/// One comparison.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Comparison {
    pub claim: String,
    pub claimed: f64,
    pub measured: f64,
    pub unit: String,
    pub reproduced: bool,
    pub source_url: String,
}

/// Compare filled claims with measured values `(claim id, value)`.
pub fn compare(d: &Dataset, measured: &[(String, f64)]) -> Result<Vec<Comparison>, String> {
    check(d)?;
    Ok(d.claims
        .iter()
        .filter_map(|c| {
            let v = c.value?;
            let m = measured.iter().find(|x| x.0 == c.id)?.1;
            Some(Comparison {
                claim: c.id.clone(),
                claimed: v,
                measured: m,
                unit: c.unit.clone(),
                reproduced: if c.direction == "max" { m <= v } else { m >= v },
                source_url: c.source_url.clone().unwrap_or_default(),
            })
        })
        .collect())
}

/// Write the due-diligence report template (Markdown) for one dataset and measured values.
pub fn dd_report(
    d: &Dataset,
    measured: &[(String, f64)],
    measured_source: &str,
) -> Result<String, String> {
    let placeholders = check(d)?;
    let cmp = compare(d, measured)?;
    let mut rows = String::new();
    for c in &d.claims {
        let Some(v) = c.value else { continue };
        let (verdict, m) = match cmp.iter().find(|x| x.claim == c.id) {
            Some(x) if x.reproduced => ("REPRODUCED", format!("{} {}", x.measured, c.unit)),
            Some(x) => ("NOT REPRODUCED", format!("{} {}", x.measured, c.unit)),
            None => ("NOT TESTED", "-".into()),
        };
        rows.push_str(&format!(
            "| {} | {} {} ({}) | {} | {} | [{}]({}), {} | MODELLED |\n",
            c.quantity,
            v,
            c.unit,
            if c.direction == "max" {
                "at most"
            } else {
                "at least"
            },
            m,
            verdict,
            c.source_date.clone().unwrap_or_default(),
            c.source_url.clone().unwrap_or_default(),
            c.quote.clone().unwrap_or_default().replace('|', "/")
        ));
    }
    Ok(format!(
        "# Technical due diligence: {} ({})\n\n_Template. Each verdict cites the operator's public statement it tests and carries its label. Measurements: {measured_source}. {placeholders} claim(s) of the dataset have no cited value and are not assessed._\n\n## 1. Scope and questions\n\n[to fill]\n\n## 2. Public claims tested\n\n| claim | stated | measured | verdict | source | label |\n|---|---|---|---|---|---|\n{rows}\n## 3. How the measurements were made\n\n[to fill: method hash, procedure version, simulator or field set-up, operating envelope]\n\n## 4. What the evidence does not show\n\n[to fill]\n\n## 5. Risks and open questions for the investor\n\n[to fill]\n",
        d.constellation, d.operator
    ))
}
