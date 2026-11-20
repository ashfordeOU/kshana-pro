// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # A-GNSS performance vector set scaffolding (backlog B3004)
//!
//! The A-GNSS minimum performance test cases of 3GPP (TS 37.571-1 / -2 for the test
//! procedures, TS 38.171 for NR minimum performance) define, per case, the assistance given,
//! the satellites and their signal levels, the scenario, the accuracy and response-time
//! limits, the fraction of trials that must meet them and the test tolerances. **This work did
//! not read those specifications, so Kshana ships no value from them.** This module gives:
//!
//! * [`scaffold`]: a vector-set skeleton whose every value is a placeholder (`null`) with a
//!   `source` the user fills when transcribing from the text they hold. The case categories
//!   follow the structure of the A-GNSS minimum performance specifications (sensitivity with
//!   coarse and with fine time assistance, nominal accuracy, dynamic range, multipath, a
//!   moving scenario with periodic update); category names are to confirm against the release
//!   in use, and the user may add, remove or rename cases;
//! * [`status`]: which fields of which cases are still placeholders, and any value that lacks
//!   its source (refused: every transcribed value must cite its clause);
//! * [`score`]: per case, the device's trials (`case_id,trial,error_m[,response_time_s]`)
//!   against the transcribed limits relaxed by the transcribed test tolerances
//!   ([`super::tolerance`]), the observed success fraction with its exact 95 % interval, and
//!   a decision under the selected rule. Scoring refuses a case with a placeholder it needs.
//!
//! A score is a pre-assessment on user-transcribed values, not a 3GPP test verdict.
//! **Label: MODELLED.**

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::tolerance::{test_limit, Decision, Relax};
use super::{cp95, Table};

/// Format tag of a vector set.
pub const FORMAT: &str = "kshana-pro-agnss-vector-set/1";
/// Format tag of a score.
pub const SCORE_FORMAT: &str = "kshana-pro-agnss-vector-score/1";
/// What every scaffold says.
pub const BASIS: &str =
    "Scaffold only. Every value is a placeholder until the user transcribes it, \
with its clause, from the specification text they hold (TS 37.571-1 / -2, TS 38.171; release to \
state). Kshana ships no value from these specifications. Case categories follow the structure of \
the A-GNSS minimum performance specifications and are to confirm against the release in use.";

/// One transcribed value with its source; `value: null` is a placeholder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field<T> {
    pub value: Option<T>,
    pub source: Option<String>,
}

impl<T> Default for Field<T> {
    fn default() -> Self {
        Field {
            value: None,
            source: None,
        }
    }
}

/// One case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub category: String,
    pub spec_clause: Field<String>,
    /// Constellations and signals (e.g. `["GPS L1 C/A", "Galileo E1"]`).
    pub signals: Field<Vec<String>>,
    /// Assistance data provided (free text as transcribed).
    pub assistance: Field<String>,
    pub satellites: Field<u32>,
    /// Signal level per satellite (dBm), as transcribed.
    pub signal_levels_dbm: Field<Vec<f64>>,
    pub hdop_range: Field<[f64; 2]>,
    /// Scenario (static, moving, multipath ...), as transcribed.
    pub scenario: Field<String>,
    pub accuracy_limit_m: Field<f64>,
    pub response_time_limit_s: Field<f64>,
    pub required_fraction: Field<f64>,
    pub trials: Field<u32>,
    pub tolerance_accuracy_m: Field<f64>,
    pub tolerance_response_time_s: Field<f64>,
    pub tolerance_signal_level_db: Field<f64>,
}

impl Case {
    fn blank(id: &str, category: &str) -> Case {
        Case {
            id: id.into(),
            category: category.into(),
            spec_clause: Field::default(),
            signals: Field::default(),
            assistance: Field::default(),
            satellites: Field::default(),
            signal_levels_dbm: Field::default(),
            hdop_range: Field::default(),
            scenario: Field::default(),
            accuracy_limit_m: Field::default(),
            response_time_limit_s: Field::default(),
            required_fraction: Field::default(),
            trials: Field::default(),
            tolerance_accuracy_m: Field::default(),
            tolerance_response_time_s: Field::default(),
            tolerance_signal_level_db: Field::default(),
        }
    }

    /// `(field, has value, has source)` for every field.
    fn fields(&self) -> Vec<(&'static str, bool, bool)> {
        fn f<T>(n: &'static str, x: &Field<T>) -> (&'static str, bool, bool) {
            (
                n,
                x.value.is_some(),
                x.source.as_deref().is_some_and(|s| !s.trim().is_empty()),
            )
        }
        vec![
            f("spec_clause", &self.spec_clause),
            f("signals", &self.signals),
            f("assistance", &self.assistance),
            f("satellites", &self.satellites),
            f("signal_levels_dbm", &self.signal_levels_dbm),
            f("hdop_range", &self.hdop_range),
            f("scenario", &self.scenario),
            f("accuracy_limit_m", &self.accuracy_limit_m),
            f("response_time_limit_s", &self.response_time_limit_s),
            f("required_fraction", &self.required_fraction),
            f("trials", &self.trials),
            f("tolerance_accuracy_m", &self.tolerance_accuracy_m),
            f("tolerance_response_time_s", &self.tolerance_response_time_s),
            f("tolerance_signal_level_db", &self.tolerance_signal_level_db),
        ]
    }
}

/// A vector set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorSet {
    pub format: String,
    pub basis: String,
    /// The specification and release the values were transcribed from.
    pub release: Field<String>,
    pub cases: Vec<Case>,
}

/// Case categories of the scaffold (to confirm against the release in use).
pub const CATEGORIES: &[(&str, &str)] = &[
    (
        "sensitivity-coarse-time",
        "Sensitivity, coarse time assistance",
    ),
    ("sensitivity-fine-time", "Sensitivity, fine time assistance"),
    ("nominal-accuracy", "Nominal accuracy"),
    ("dynamic-range", "Dynamic range"),
    ("multipath", "Multipath scenario"),
    ("moving-periodic", "Moving scenario and periodic update"),
];

/// The scaffold: one blank case per category.
pub fn scaffold() -> VectorSet {
    VectorSet {
        format: FORMAT.into(),
        basis: BASIS.into(),
        release: Field::default(),
        cases: CATEGORIES
            .iter()
            .map(|(id, c)| Case::blank(id, &format!("{c} (category to confirm)")))
            .collect(),
    }
}

/// Placeholders of one case.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaseStatus {
    pub id: String,
    pub placeholders: Vec<&'static str>,
    pub filled: usize,
    /// Whether every field scoring needs is filled.
    pub scorable: bool,
}

/// Status of a set.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Status {
    pub release_filled: bool,
    pub cases: Vec<CaseStatus>,
    pub placeholders: usize,
    pub complete: bool,
}

const SCORING: &[&str] = &[
    "accuracy_limit_m",
    "required_fraction",
    "tolerance_accuracy_m",
];

fn check(set: &VectorSet) -> Result<(), String> {
    if set.format != FORMAT {
        return Err(format!("format '{}', expected '{FORMAT}'", set.format));
    }
    let mut ids = std::collections::BTreeSet::new();
    for c in &set.cases {
        if !ids.insert(&c.id) {
            return Err(format!("case '{}' appears twice", c.id));
        }
        for (n, v, s) in c.fields() {
            if v && !s {
                return Err(format!(
                    "case '{}': {n} has a value but no source; every transcribed value must cite its clause",
                    c.id
                ));
            }
        }
        if let Some(f) = c.required_fraction.value {
            if !(0.0..=1.0).contains(&f) {
                return Err(format!(
                    "case '{}': required_fraction must be in [0, 1]",
                    c.id
                ));
            }
        }
        for (n, x) in [
            ("accuracy_limit_m", c.accuracy_limit_m.value),
            ("tolerance_accuracy_m", c.tolerance_accuracy_m.value),
            ("response_time_limit_s", c.response_time_limit_s.value),
            (
                "tolerance_response_time_s",
                c.tolerance_response_time_s.value,
            ),
        ] {
            if x.is_some_and(|v| v.is_nan() || v < 0.0) {
                return Err(format!("case '{}': {n} must be zero or positive", c.id));
            }
        }
    }
    if set.release.value.is_some() && set.release.source.is_none() {
        return Err("release has a value but no source".into());
    }
    Ok(())
}

/// Which values are still placeholders.
pub fn status(set: &VectorSet) -> Result<Status, String> {
    check(set)?;
    let cases: Vec<CaseStatus> = set
        .cases
        .iter()
        .map(|c| {
            let f = c.fields();
            let ph: Vec<&'static str> = f.iter().filter(|x| !x.1).map(|x| x.0).collect();
            CaseStatus {
                id: c.id.clone(),
                scorable: SCORING.iter().all(|s| !ph.contains(s)),
                filled: f.len() - ph.len(),
                placeholders: ph,
            }
        })
        .collect();
    let n = cases.iter().map(|c| c.placeholders.len()).sum::<usize>()
        + usize::from(set.release.value.is_none());
    Ok(Status {
        release_filled: set.release.value.is_some(),
        complete: n == 0,
        placeholders: n,
        cases,
    })
}

/// Score of one case.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CaseScore {
    pub id: String,
    pub trials_logged: usize,
    pub trials_required: Option<u32>,
    pub accuracy_test_limit_m: f64,
    pub response_time_test_limit_s: Option<f64>,
    pub successes: usize,
    pub observed_fraction: f64,
    pub ci95: (f64, f64),
    pub required_fraction: f64,
    pub decision: bool,
    pub notes: Vec<String>,
}

/// A score.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Score {
    pub format: &'static str,
    pub basis: &'static str,
    pub label: &'static str,
    pub release: Option<String>,
    pub decision_rule: Decision,
    pub cases: Vec<CaseScore>,
    /// Cases in the results but not scorable (placeholders), with the reason.
    pub not_scored: BTreeMap<String, String>,
    pub verdict: String,
}

/// Score a device's trials against a filled vector set.
pub fn score(set: &VectorSet, results: &Table, rule: Decision) -> Result<Score, String> {
    let st = status(set)?;
    let (cc, ce) = (results.need("case_id")?, results.need("error_m")?);
    let crt = results.col("response_time_s");
    let mut by: BTreeMap<String, Vec<(f64, Option<f64>)>> = BTreeMap::new();
    for i in 0..results.rows.len() {
        let e = results
            .num(i, Some(ce))?
            .ok_or_else(|| format!("row {}: empty error_m", i + 2))?;
        by.entry(results.text(i, Some(cc)).to_string())
            .or_default()
            .push((e, results.num(i, crt)?));
    }
    let mut cases = Vec::new();
    let mut not_scored = BTreeMap::new();
    for (id, trials) in &by {
        let Some(c) = set.cases.iter().find(|c| &c.id == id) else {
            not_scored.insert(id.clone(), "no such case in the vector set".into());
            continue;
        };
        let s = st.cases.iter().find(|s| &s.id == id).expect("same cases");
        if !s.scorable {
            let missing: Vec<&str> = SCORING
                .iter()
                .copied()
                .filter(|f| s.placeholders.contains(f))
                .collect();
            not_scored.insert(
                id.clone(),
                format!("placeholder(s): {}", missing.join(", ")),
            );
            continue;
        }
        let acc = test_limit(
            c.accuracy_limit_m.value.expect("scorable"),
            c.tolerance_accuracy_m.value.expect("scorable"),
            Relax::Increase,
        );
        let mut notes = Vec::new();
        let rt = match (
            c.response_time_limit_s.value,
            c.tolerance_response_time_s.value,
        ) {
            (Some(l), Some(t)) => Some(test_limit(l, t, Relax::Increase)),
            (Some(_), None) => {
                notes.push(
                    "response-time tolerance is a placeholder: response time not scored".into(),
                );
                None
            }
            _ => None,
        };
        let ok = trials
            .iter()
            .filter(|(e, t)| {
                *e <= acc
                    && match (rt, t) {
                        (Some(l), Some(t)) => *t <= l,
                        (Some(_), None) => false,
                        (None, _) => true,
                    }
            })
            .count();
        if rt.is_some() && trials.iter().any(|(_, t)| t.is_none()) {
            notes.push("trials without response_time_s count as failures".into());
        }
        if let Some(n) = c.trials.value {
            if (trials.len() as u32) < n {
                notes.push(format!("{} trial(s) logged, {n} required", trials.len()));
            }
        }
        let req = c.required_fraction.value.expect("scorable");
        let ci = cp95(ok, trials.len());
        let obs = ok as f64 / trials.len() as f64;
        cases.push(CaseScore {
            id: id.clone(),
            trials_logged: trials.len(),
            trials_required: c.trials.value,
            accuracy_test_limit_m: acc,
            response_time_test_limit_s: rt,
            successes: ok,
            observed_fraction: obs,
            ci95: ci,
            required_fraction: req,
            decision: match rule {
                Decision::Point => obs >= req,
                Decision::LowerBound => ci.0 >= req,
            },
            notes,
        });
    }
    let verdict = format!(
        "{} case(s) scored ({} meeting the transcribed requirement under the {:?} rule), {} not scored. Pre-assessment on user-transcribed values, not a 3GPP test verdict.",
        cases.len(),
        cases.iter().filter(|c| c.decision).count(),
        rule,
        not_scored.len()
    );
    Ok(Score {
        format: SCORE_FORMAT,
        basis: super::tolerance::BASIS,
        label: super::nr::LABEL,
        release: set.release.value.clone(),
        decision_rule: rule,
        cases,
        not_scored,
        verdict,
    })
}
