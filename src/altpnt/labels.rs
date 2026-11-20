// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Digital-twin versus flight-test labels (`kshana-pro altpnt labels`).
//!
//! A performance figure of an alternative-PNT system may come from a digital twin, a
//! simulator, a lab, a field trial or a flight. The figure reads the same in a brochure. This
//! labels every stated claim by where its results came from and compares a model's prediction
//! with the operational measurement of the same metric:
//!
//! * the claim's **label** is the [`super::evidence_label`] of its results, raised to
//!   `VALIDATED` when an operational (field or flight) measurement exists and the model
//!   (twin or simulator) prediction agrees with it within the claim's stated relative
//!   tolerance: the model has been checked against the operation for this metric;
//! * the claim is **met** or **not met** by the least favourable value of its strongest
//!   environment (no environment's result is averaged into a stronger one's);
//! * the **model gap** is the model mean against the operational mean, relative to the
//!   operational mean, and called `model-optimistic`, `model-pessimistic` or `agrees`.
//!
//! A claim labelled MODELLED is an architectural target, not an empirical result, and the
//! output says so in those words.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{evidence_label, rnd, Environment, Inputs, EVIDENCE, SCOPE};

/// The result format.
pub const FORMAT: &str = "kshana-pro-altpnt-labels/1";
/// Default relative tolerance for model against operation.
pub const DEFAULT_TOLERANCE: f64 = 0.2;

fn default_tol() -> f64 {
    DEFAULT_TOLERANCE
}

/// A stated performance claim.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    pub metric: String,
    pub unit: String,
    /// `lower` (an error) or `higher` (an availability) is better.
    pub better: String,
    pub claimed: f64,
    /// Relative tolerance for model against operation.
    #[serde(default = "default_tol")]
    pub tolerance: f64,
    /// Where the claim is stated (datasheet, proposal, paper).
    #[serde(default)]
    pub stated_in: String,
}

/// One result for a claim's metric.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultIn {
    pub claim: String,
    pub environment: Environment,
    pub value: f64,
    #[serde(default)]
    pub reference: String,
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub system: String,
    #[serde(rename = "claim")]
    pub claims: Vec<Claim>,
    #[serde(rename = "result", default)]
    pub results: Vec<ResultIn>,
}

/// The model-against-operation comparison of one claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelGap {
    pub model_mean: f64,
    pub operational_mean: f64,
    /// (model - operational) / |operational|.
    pub relative_gap: f64,
    /// agrees, model-optimistic, model-pessimistic.
    pub reading: &'static str,
}

/// One labelled claim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Labelled {
    pub id: String,
    pub metric: String,
    pub unit: String,
    pub claimed: f64,
    pub stated_in: String,
    /// Results per environment (values in input order).
    pub results: BTreeMap<String, Vec<f64>>,
    /// CLAIMED, MODELLED, MEASURED-LAB, MEASURED-OPERATIONAL or VALIDATED.
    pub label: &'static str,
    pub strongest_environment: Option<Environment>,
    /// The least favourable value of the strongest environment.
    pub deciding_value: Option<f64>,
    pub met: Option<bool>,
    pub model_gap: Option<ModelGap>,
    pub statement: String,
}

/// The labels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Labels {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub system: String,
    pub claims: Vec<Labelled>,
    /// Claims per label.
    pub counts: BTreeMap<String, usize>,
    pub scope: &'static str,
}

fn better_of(c: &Claim) -> Result<bool, String> {
    match c.better.as_str() {
        "lower" => Ok(true),
        "higher" => Ok(false),
        o => Err(format!(
            "{}: better must be lower or higher, not '{o}'",
            c.id
        )),
    }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Label one claim from its results.
pub fn label_claim(c: &Claim, results: &[(Environment, f64)]) -> Result<Labelled, String> {
    let lower = better_of(c)?;
    if !(c.claimed.is_finite() && c.tolerance.is_finite() && c.tolerance >= 0.0) {
        return Err(format!(
            "{}: claimed and tolerance must be finite, tolerance >= 0",
            c.id
        ));
    }
    if let Some((_, v)) = results.iter().find(|(_, v)| !v.is_finite()) {
        return Err(format!("{}: result value {v} is not finite", c.id));
    }
    let mut by: BTreeMap<Environment, Vec<f64>> = BTreeMap::new();
    for (e, v) in results {
        by.entry(*e).or_default().push(*v);
    }
    let envs: Vec<Environment> = by.keys().copied().collect();
    let strongest = envs.iter().max().copied();
    let deciding = strongest.map(|e| {
        let v = &by[&e];
        if lower {
            v.iter().cloned().fold(f64::MIN, f64::max)
        } else {
            v.iter().cloned().fold(f64::MAX, f64::min)
        }
    });
    let met = deciding.map(|d| {
        if lower {
            d <= c.claimed
        } else {
            d >= c.claimed
        }
    });
    let model: Vec<f64> = by
        .iter()
        .filter(|(e, _)| !e.measured())
        .flat_map(|(_, v)| v.iter().copied())
        .collect();
    let oper: Vec<f64> = by
        .iter()
        .filter(|(e, _)| e.operational())
        .flat_map(|(_, v)| v.iter().copied())
        .collect();
    let model_gap = if !model.is_empty() && !oper.is_empty() {
        let (m, o) = (mean(&model), mean(&oper));
        let rel = if o == 0.0 {
            if m == 0.0 {
                0.0
            } else {
                f64::INFINITY
            }
        } else {
            (m - o) / o.abs()
        };
        let reading = if rel.abs() <= c.tolerance {
            "agrees"
        } else if (rel < 0.0) == lower {
            "model-optimistic"
        } else {
            "model-pessimistic"
        };
        Some(ModelGap {
            model_mean: rnd(m, 6),
            operational_mean: rnd(o, 6),
            relative_gap: if rel.is_finite() { rnd(rel, 4) } else { 1e9 },
            reading,
        })
    } else {
        None
    };
    let mut label = evidence_label(&envs);
    if model_gap.as_ref().is_some_and(|g| g.reading == "agrees") {
        label = "VALIDATED";
    }
    let met_txt = match met {
        Some(true) => "meets the claim",
        Some(false) => "does not meet the claim",
        None => "",
    };
    let statement = match (label, strongest, deciding) {
        ("CLAIMED", _, _) => "CLAIMED: no result supports this figure.".to_string(),
        ("MODELLED", Some(e), Some(d)) => format!(
            "MODELLED: an architectural target, not an empirical result; the {} value {} {} {}.",
            e.name(),
            rnd(d, 6),
            c.unit,
            met_txt
        ),
        (l, Some(e), Some(d)) => {
            let mut s = format!(
                "{l}: the least favourable {} value {} {} {met_txt}",
                e.name(),
                rnd(d, 6),
                c.unit
            );
            if let Some(g) = &model_gap {
                s += &format!(
                    "; the model is {} against the operational mean ({:+.1} %)",
                    g.reading,
                    g.relative_gap * 100.0
                );
            }
            s + "."
        }
        _ => unreachable!("a label other than CLAIMED has results"),
    };
    Ok(Labelled {
        id: c.id.clone(),
        metric: c.metric.clone(),
        unit: c.unit.clone(),
        claimed: c.claimed,
        stated_in: c.stated_in.clone(),
        results: by
            .iter()
            .map(|(e, v)| (e.name().to_string(), v.clone()))
            .collect(),
        label,
        strongest_environment: strongest,
        deciding_value: deciding.map(|d| rnd(d, 6)),
        met,
        model_gap,
        statement,
    })
}

/// Label every claim.
pub fn label(req: &Request) -> Result<Labels, String> {
    if let Some(r) = req
        .results
        .iter()
        .find(|r| !req.claims.iter().any(|c| c.id == r.claim))
    {
        return Err(format!("result for unknown claim '{}'", r.claim));
    }
    let mut claims = Vec::new();
    let mut counts = BTreeMap::new();
    for c in &req.claims {
        if claims.iter().any(|l: &Labelled| l.id == c.id) {
            return Err(format!("claim '{}' listed twice", c.id));
        }
        let rs: Vec<(Environment, f64)> = req
            .results
            .iter()
            .filter(|r| r.claim == c.id)
            .map(|r| (r.environment, r.value))
            .collect();
        let l = label_claim(c, &rs)?;
        *counts.entry(l.label.to_string()).or_insert(0) += 1;
        claims.push(l);
    }
    Ok(Labels {
        format: FORMAT,
        evidence: EVIDENCE,
        title: req.title.clone(),
        system: req.system.clone(),
        claims,
        counts,
        scope: SCOPE,
    })
}

/// Load a request.
pub fn load(path: &Path) -> Result<(Request, Inputs), String> {
    super::read_request(path)
}

/// The labels as a Markdown table.
pub fn markdown(l: &Labels) -> String {
    let mut s = format!(
        "# Evidence labels: {}\n\nSystem: {}. Evidence class of this table: {}.\n\n\
         | Claim | Metric | Claimed | Label | Deciding value | Met | Model vs operation |\n\
         |---|---|---|---|---|---|---|\n",
        l.title, l.system, l.evidence
    );
    for c in &l.claims {
        s += &format!(
            "| {} | {} | {} {} | {} | {} | {} | {} |\n",
            c.id,
            c.metric,
            c.claimed,
            c.unit,
            c.label,
            c.deciding_value
                .map(|d| format!(
                    "{d} {} ({})",
                    c.unit,
                    c.strongest_environment.map(|e| e.name()).unwrap_or("")
                ))
                .unwrap_or_else(|| "-".into()),
            match c.met {
                Some(true) => "yes",
                Some(false) => "no",
                None => "-",
            },
            c.model_gap
                .as_ref()
                .map(|g| format!("{} ({:+.1} %)", g.reading, g.relative_gap * 100.0))
                .unwrap_or_else(|| "-".into())
        );
    }
    s += "\nVALIDATED: an operational measurement exists and the model agrees with it within the \
          stated tolerance. MEASURED-OPERATIONAL: field or flight. MEASURED-LAB: HIL or RF lab. \
          MODELLED: twin or simulator only, an architectural target rather than an empirical \
          result. CLAIMED: no result.\n\n";
    s += l.scope;
    s += "\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use Environment::*;

    fn claim(better: &str, claimed: f64) -> Claim {
        Claim {
            id: "C".into(),
            metric: "m".into(),
            unit: "m".into(),
            better: better.into(),
            claimed,
            tolerance: 0.2,
            stated_in: String::new(),
        }
    }

    #[test]
    fn a_twin_only_claim_is_an_architectural_target() {
        let l = label_claim(&claim("lower", 50.0), &[(Twin, 20.0)]).unwrap();
        assert_eq!(l.label, "MODELLED");
        assert_eq!(l.met, Some(true));
        assert!(l.statement.contains("architectural target"));
        assert_eq!(
            label_claim(&claim("lower", 50.0), &[]).unwrap().label,
            "CLAIMED"
        );
    }

    #[test]
    fn an_optimistic_twin_is_not_validated() {
        let l = label_claim(
            &claim("lower", 50.0),
            &[(Twin, 20.0), (Flight, 70.0), (Flight, 60.0)],
        )
        .unwrap();
        assert_eq!(l.label, "MEASURED-OPERATIONAL");
        assert_eq!(l.deciding_value, Some(70.0));
        assert_eq!(l.met, Some(false));
        let g = l.model_gap.unwrap();
        assert_eq!(g.reading, "model-optimistic");
        assert!((g.relative_gap + 0.6923).abs() < 1e-3);
        let v = label_claim(
            &claim("higher", 0.99),
            &[(Simulator, 0.995), (Field, 0.993)],
        )
        .unwrap();
        assert_eq!(v.label, "VALIDATED");
        assert_eq!(v.met, Some(true));
    }
}
