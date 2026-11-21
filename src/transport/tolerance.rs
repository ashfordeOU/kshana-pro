// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Test-tolerance calculator in the manner of 3GPP TS 37.571-1 (backlog B3004)
//!
//! 3GPP UE positioning test specifications relax each minimum requirement by a **test
//! tolerance** so that the uncertainty of the test system is shared between the tester and
//! the device ("shared risk"). The tolerance values themselves are tabulated in the
//! specification; **this work did not read TS 37.571-1, so Kshana ships no tolerance or
//! requirement value**. This module does the arithmetic on values the user transcribes,
//! each with its source, and marks everything else as a placeholder:
//!
//! * per parameter, either a **transcribed tolerance** (with its clause) or a **test-system
//!   uncertainty budget**: contributions given as standard uncertainties, 95 % expanded
//!   values or rectangular half-widths, combined by root-sum-square (GUM, JCGM 100:2008 §5.1)
//!   and expanded with the stated coverage factor; under the shared-risk principle the
//!   derived tolerance equals that expanded uncertainty (to confirm against the
//!   specification's own derivation annex);
//! * the **test requirement** `MR ± TT`, relaxed in the direction the user states eases the
//!   device;
//! * optionally a **measured value** with its uncertainty classified as pass with margin
//!   (inside the requirement by at least the uncertainty), pass within uncertainty, or fail;
//! * optionally a **success-rate** requirement (a fraction of trials): the observed fraction,
//!   its exact 95 % interval, and a decision under the rule the user selects (`point` or
//!   `lower-bound`); the specification's own statistical method is to confirm.
//!
//! A parameter whose requirement or tolerance is missing stays **PLACEHOLDER** and has no
//! test requirement. **Label: MODELLED** (arithmetic on stated values).

use serde::{Deserialize, Serialize};

use super::cp95;

/// Pro feature of the calculator and of A-GNSS vector scoring.
pub const FEATURE: &str = "pro-3gpp-tolerance";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-3gpp-test-tolerance/1";
/// What every result says about the source of its values.
pub const BASIS: &str = "Arithmetic on values the user transcribed, each with its stated source. \
Kshana ships no TS 37.571-1 requirement, tolerance or statistical table: those were not read in \
this work and are to confirm against the specification release in use. A result is a \
pre-assessment, not a 3GPP test verdict.";
/// Placeholder marker.
pub const PLACEHOLDER: &str = "PLACEHOLDER";

/// How a contribution is stated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A standard uncertainty (1 sigma).
    Standard,
    /// A 95 % expanded uncertainty of a normal distribution (k = 1.96).
    Expanded95,
    /// The half-width of a rectangular distribution.
    RectangularHalfWidth,
}

/// One test-system uncertainty contribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub name: String,
    pub value: f64,
    pub kind: Kind,
}

impl Contribution {
    /// The standard uncertainty.
    pub fn standard(&self) -> f64 {
        match self.kind {
            Kind::Standard => self.value,
            Kind::Expanded95 => self.value / 1.959_963_984_540_054,
            Kind::RectangularHalfWidth => self.value / 3f64.sqrt(),
        }
    }
}

/// Which way the tolerance relaxes the requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relax {
    /// The requirement is an upper limit (e.g. an error): the test limit is `MR + TT`.
    Increase,
    /// The requirement is a lower limit (e.g. a level to be met): the test limit is `MR − TT`.
    Decrease,
}

/// A success-rate requirement and its trials.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuccessRate {
    /// Required fraction of successful trials; omitted is a placeholder.
    #[serde(default)]
    pub required_fraction: Option<f64>,
    #[serde(default)]
    pub required_fraction_source: Option<String>,
    pub trials: usize,
    pub successes: usize,
}

/// One parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    #[serde(default)]
    pub unit: String,
    /// The minimum requirement; omitted is a placeholder.
    #[serde(default)]
    pub minimum_requirement: Option<f64>,
    /// Where the requirement was transcribed from (required with a value).
    #[serde(default)]
    pub requirement_source: Option<String>,
    pub relax: Relax,
    /// A tolerance transcribed from the specification (used instead of the budget).
    #[serde(default)]
    pub test_tolerance: Option<f64>,
    #[serde(default)]
    pub tolerance_source: Option<String>,
    /// Test-system uncertainty budget (used when no tolerance is transcribed).
    #[serde(default)]
    pub uncertainty: Vec<Contribution>,
    /// Coverage factor of the expanded uncertainty.
    #[serde(default = "d_k")]
    pub coverage_k: f64,
    /// A measured value of the device, with its standard uncertainty.
    #[serde(default)]
    pub measured: Option<f64>,
    #[serde(default)]
    pub measured_standard_uncertainty: Option<f64>,
    #[serde(default)]
    pub success_rate: Option<SuccessRate>,
}

fn d_k() -> f64 {
    1.96
}

/// Success-rate decision rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// Observed fraction at or above the required fraction.
    #[default]
    Point,
    /// The exact 95 % lower limit at or above the required fraction.
    LowerBound,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// Specification release the values were transcribed from (free text).
    #[serde(default)]
    pub release: Option<String>,
    #[serde(default)]
    pub decision: Decision,
    pub parameters: Vec<Parameter>,
}

/// Where a parameter's tolerance came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToleranceFrom {
    Transcribed,
    Budget,
    Placeholder,
}

/// Outcome of a measured value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    PassWithMargin,
    PassWithinUncertainty,
    Fail,
}

/// Success-rate result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RateResult {
    pub observed: f64,
    pub ci95: (f64, f64),
    pub required: Option<f64>,
    pub decision: Option<bool>,
}

/// One parameter's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ParamResult {
    pub name: String,
    pub unit: String,
    pub status: &'static str,
    pub placeholders: Vec<String>,
    pub minimum_requirement: Option<f64>,
    pub combined_standard_uncertainty: Option<f64>,
    pub expanded_uncertainty: Option<f64>,
    pub test_tolerance: Option<f64>,
    pub tolerance_from: ToleranceFrom,
    pub test_requirement: Option<f64>,
    pub outcome: Option<Outcome>,
    pub success_rate: Option<RateResult>,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub title: String,
    pub basis: &'static str,
    pub label: &'static str,
    pub request: Request,
    pub parameters: Vec<ParamResult>,
    pub placeholders: usize,
    pub verdict: String,
}

/// RSS of the standard uncertainties.
pub fn combine(c: &[Contribution]) -> f64 {
    c.iter().map(|x| x.standard().powi(2)).sum::<f64>().sqrt()
}

fn blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn validate(q: &Request) -> Result<(), String> {
    if q.parameters.is_empty() {
        return Err("at least one [[parameters]] entry is needed".into());
    }
    for p in &q.parameters {
        let e = |m: &str| Err(format!("parameter '{}': {m}", p.name));
        if p.minimum_requirement.is_some() && p.requirement_source.as_deref().is_none_or(blank) {
            return e("a minimum_requirement needs its requirement_source");
        }
        if let Some(t) = p.test_tolerance {
            if t.is_nan() || t < 0.0 {
                return e("test_tolerance must be zero or positive");
            }
            if p.tolerance_source.as_deref().is_none_or(blank) {
                return e("a test_tolerance needs its tolerance_source");
            }
        }
        if p.uncertainty
            .iter()
            .any(|c| c.value.is_nan() || c.value < 0.0)
        {
            return e("uncertainty values must be zero or positive");
        }
        if p.coverage_k.is_nan() || p.coverage_k <= 0.0 {
            return e("coverage_k must be positive");
        }
        if let Some(s) = &p.success_rate {
            if s.successes > s.trials || s.trials == 0 {
                return e("success_rate needs trials > 0 and successes <= trials");
            }
            if let Some(f) = s.required_fraction {
                if !(0.0..=1.0).contains(&f) {
                    return e("required_fraction must be in [0, 1]");
                }
                if s.required_fraction_source.as_deref().is_none_or(blank) {
                    return e("a required_fraction needs its required_fraction_source");
                }
            }
        }
    }
    Ok(())
}

/// The test limit `MR ± TT`.
pub fn test_limit(mr: f64, tt: f64, relax: Relax) -> f64 {
    match relax {
        Relax::Increase => mr + tt,
        Relax::Decrease => mr - tt,
    }
}

/// Classify a measured value against a test limit with its expanded uncertainty `u`.
pub fn classify(measured: f64, limit: f64, u: f64, relax: Relax) -> Outcome {
    // Margin: how far inside the limit the measurement is.
    let margin = match relax {
        Relax::Increase => limit - measured,
        Relax::Decrease => measured - limit,
    };
    if margin >= u {
        Outcome::PassWithMargin
    } else if margin >= 0.0 {
        Outcome::PassWithinUncertainty
    } else {
        Outcome::Fail
    }
}

/// Run the calculator.
pub fn run(q: &Request) -> Result<Report, String> {
    validate(q)?;
    let mut out = Vec::new();
    for p in &q.parameters {
        let mut ph = Vec::new();
        if p.minimum_requirement.is_none() {
            ph.push("minimum_requirement".to_string());
        }
        let (uc, ue) = if p.uncertainty.is_empty() {
            (None, None)
        } else {
            let u = combine(&p.uncertainty);
            (Some(u), Some(p.coverage_k * u))
        };
        let (tt, from) = match (p.test_tolerance, ue) {
            (Some(t), _) => (Some(t), ToleranceFrom::Transcribed),
            (None, Some(u)) => (Some(u), ToleranceFrom::Budget),
            (None, None) => {
                ph.push("test_tolerance (or an uncertainty budget)".into());
                (None, ToleranceFrom::Placeholder)
            }
        };
        let limit = match (p.minimum_requirement, tt) {
            (Some(m), Some(t)) => Some(test_limit(m, t, p.relax)),
            _ => None,
        };
        let outcome = match (limit, p.measured) {
            (Some(l), Some(m)) => Some(classify(
                m,
                l,
                p.coverage_k * p.measured_standard_uncertainty.unwrap_or(0.0),
                p.relax,
            )),
            _ => None,
        };
        let rate = p.success_rate.as_ref().map(|s| {
            if s.required_fraction.is_none() {
                ph.push("success_rate.required_fraction".into());
            }
            let ci = cp95(s.successes, s.trials);
            let obs = s.successes as f64 / s.trials as f64;
            RateResult {
                observed: obs,
                ci95: ci,
                required: s.required_fraction,
                decision: s.required_fraction.map(|f| match q.decision {
                    Decision::Point => obs >= f,
                    Decision::LowerBound => ci.0 >= f,
                }),
            }
        });
        out.push(ParamResult {
            name: p.name.clone(),
            unit: p.unit.clone(),
            status: if ph.is_empty() {
                "complete"
            } else {
                PLACEHOLDER
            },
            placeholders: ph,
            minimum_requirement: p.minimum_requirement,
            combined_standard_uncertainty: uc,
            expanded_uncertainty: ue,
            test_tolerance: tt,
            tolerance_from: from,
            test_requirement: limit,
            outcome,
            success_rate: rate,
        });
    }
    let n_ph = out.iter().filter(|p| p.status == PLACEHOLDER).count();
    let fails = out
        .iter()
        .filter(|p| {
            p.outcome == Some(Outcome::Fail)
                || p.success_rate.as_ref().and_then(|r| r.decision) == Some(false)
        })
        .count();
    let verdict = format!(
        "{} parameter(s): {} with placeholders, {} with a test requirement; {} failing a measured value or success rate. Pre-assessment on user-transcribed values, not a 3GPP test verdict.",
        out.len(),
        n_ph,
        out.iter().filter(|p| p.test_requirement.is_some()).count(),
        fails
    );
    Ok(Report {
        format: FORMAT,
        title: q.title.clone(),
        basis: BASIS,
        label: super::nr::LABEL,
        request: q.clone(),
        parameters: out,
        placeholders: n_ph,
        verdict,
    })
}
