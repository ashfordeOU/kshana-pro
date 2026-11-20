// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Constellation-claim datasets (B2108).
//!
//! A claim dataset (`kshana-pro-leo-claims/1`) lists what a constellation operator has said in
//! public: quantity, value, unit, conditions, and the source (URL, date, and the quoted words).
//! A value without a source URL, date and quote is refused: a claim the user cannot cite is not a
//! claim. Kshana ships templates for Celeste, CentiSpace and Geespace whose claims are
//! placeholders to fill; it cites nothing itself. [`compare`] puts filled claims beside measured
//! results (e.g. from a scorecard) and says whether the measurement reproduced each claim under
//! its own conditions.

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

/// Quantities a template asks about (values to fill from public sources).
const QUANTITIES: [(&str, &str, &str, &str); 5] = [
    (
        "horizontal-accuracy",
        "horizontal positioning accuracy",
        "m",
        "max",
    ),
    ("timing-accuracy", "time transfer accuracy", "ns", "max"),
    ("ttff", "time to first fix", "s", "max"),
    (
        "satellites-in-orbit",
        "satellites in orbit at the stated date",
        "count",
        "min",
    ),
    ("availability", "service availability", "fraction", "min"),
];

/// The templates: placeholders only.
pub fn templates() -> Vec<Dataset> {
    ["Celeste", "CentiSpace", "Geespace"]
        .iter()
        .map(|c| Dataset {
            format: FORMAT.into(),
            constellation: (*c).into(),
            operator: "PLACEHOLDER: operator as named in the source".into(),
            claims: QUANTITIES
                .iter()
                .map(|(id, q, u, d)| Claim {
                    id: (*id).into(),
                    quantity: (*q).into(),
                    unit: (*u).into(),
                    direction: (*d).into(),
                    value: None,
                    conditions: "PLACEHOLDER: conditions as stated in the source".into(),
                    source_url: None,
                    source_date: None,
                    quote: None,
                })
                .collect(),
        })
        .collect()
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
