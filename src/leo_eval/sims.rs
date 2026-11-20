// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Neutral multi-simulator LEO-PNT procedures and scorecards (B2101).
//!
//! A procedure (`kshana-pro-leo-sim-procedure/1`) names its steps, the metric of each step and,
//! where the user has one, a bar with its source. Its SHA-256 is registered before the runs;
//! a scorecard refuses runs scored under a procedure whose hash differs. Runs come from each
//! simulator as `simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix`. Per simulator and
//! step: horizontal RMS and 95th percentile, availability (share of epochs with a fix), time to
//! first fix, clock RMS. Simulators are listed in input order and never ranked; a simulator
//! whose metric lies more than 3 scaled median absolute deviations from the others' median is
//! marked as differing from the other simulators, which says the simulators disagree, not which
//! is right.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::transport::{quantile, Table};

/// Procedure format.
pub const FORMAT: &str = "kshana-pro-leo-sim-procedure/1";
/// Scorecard format.
pub const SCORE_FORMAT: &str = "kshana-pro-leo-sim-scorecard/1";
/// Metrics a step may use, with the direction of "better".
pub const METRICS: [(&str, &str); 5] = [
    ("h_rms_m", "max"),
    ("h_p95_m", "max"),
    ("availability", "min"),
    ("ttff_s", "max"),
    ("clock_rms_s", "max"),
];

/// One step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub what: String,
    pub metric: String,
    #[serde(default)]
    pub bar: Option<f64>,
    #[serde(default)]
    pub bar_source: Option<String>,
}

/// A procedure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Procedure {
    pub format: String,
    pub id: String,
    pub title: String,
    #[serde(rename = "step")]
    pub steps: Vec<Step>,
}

impl Procedure {
    /// Check it.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT {
            return Err(format!("format '{}' is not {FORMAT}", self.format));
        }
        if self.steps.is_empty() {
            return Err("a procedure needs at least one [[step]]".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for s in &self.steps {
            if !ids.insert(&s.id) {
                return Err(format!("step '{}' declared twice", s.id));
            }
            if !METRICS.iter().any(|m| m.0 == s.metric) {
                return Err(format!("step '{}': metric '{}' unknown", s.id, s.metric));
            }
            if s.bar.is_some() && s.bar_source.as_deref().is_none_or(|x| x.trim().is_empty()) {
                return Err(format!("step '{}': a bar needs its bar_source", s.id));
            }
        }
        Ok(())
    }
}

/// SHA-256 of the procedure text, to register before the runs.
pub fn procedure_hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// One cell of the scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cell {
    pub simulator: String,
    pub value: Option<f64>,
    pub epochs: usize,
    pub meets_bar: Option<bool>,
    pub differs_from_others: bool,
}

/// One step of the scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StepScore {
    pub step: String,
    pub metric: String,
    pub bar: Option<f64>,
    pub bar_source: Option<String>,
    pub median: Option<f64>,
    pub spread: Option<f64>,
    pub cells: Vec<Cell>,
}

/// The scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scorecard {
    pub format: &'static str,
    pub procedure: String,
    pub procedure_sha256: String,
    pub simulators: Vec<String>,
    pub steps: Vec<StepScore>,
    pub statement: &'static str,
}

#[derive(Default)]
struct Acc {
    t: Vec<f64>,
    h: Vec<f64>,
    clk: Vec<f64>,
    fix: Vec<bool>,
}

fn metric(a: &Acc, m: &str) -> Option<f64> {
    let hf: Vec<f64> =
        a.h.iter()
            .zip(&a.fix)
            .filter(|x| *x.1)
            .map(|x| *x.0)
            .collect();
    match m {
        "h_rms_m" => (!hf.is_empty())
            .then(|| (hf.iter().map(|x| x * x).sum::<f64>() / hf.len() as f64).sqrt()),
        "h_p95_m" => quantile(&hf, 0.95),
        "availability" => (!a.fix.is_empty())
            .then(|| a.fix.iter().filter(|x| **x).count() as f64 / a.fix.len() as f64),
        "ttff_s" => a.t.iter().zip(&a.fix).find(|x| *x.1).map(|x| *x.0),
        "clock_rms_s" => {
            let c: Vec<f64> = a
                .clk
                .iter()
                .zip(&a.fix)
                .filter(|x| *x.1 && x.0.is_finite())
                .map(|x| *x.0)
                .collect();
            (!c.is_empty()).then(|| (c.iter().map(|x| x * x).sum::<f64>() / c.len() as f64).sqrt())
        }
        _ => None,
    }
}

/// Score runs under a procedure whose text hashes to `registered_sha256`.
pub fn score(proc_text: &str, registered_sha256: &str, runs: &str) -> Result<Scorecard, String> {
    let h = procedure_hash(proc_text);
    if h != registered_sha256 {
        return Err(format!("the procedure hashes to {h}, not the registered {registered_sha256}: it changed after registration"));
    }
    let p: Procedure = toml::from_str(proc_text).map_err(|e| format!("procedure: {e}"))?;
    p.validate()?;
    let t = Table::parse(runs)?;
    let (cs, cst, ct) = (t.need("simulator")?, t.need("step")?, t.need("t_s")?);
    let (ce, cn, cc, cf) = (
        t.need("err_e_m")?,
        t.need("err_n_m")?,
        t.col("clock_err_s"),
        t.need("fix")?,
    );
    let mut sims: Vec<String> = Vec::new();
    let mut acc: BTreeMap<(String, String), Acc> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let sim = t.text(i, Some(cs)).to_string();
        if !sims.contains(&sim) {
            sims.push(sim.clone());
        }
        let step = t.text(i, Some(cst)).to_string();
        if !p.steps.iter().any(|s| s.id == step) {
            return Err(format!(
                "row {}: step '{step}' is not in the procedure",
                i + 2
            ));
        }
        let a = acc.entry((sim, step)).or_default();
        a.t.push(t.num(i, Some(ct))?.ok_or("empty t_s")?);
        let (e, n) = (
            t.num(i, Some(ce))?.unwrap_or(f64::NAN),
            t.num(i, Some(cn))?.unwrap_or(f64::NAN),
        );
        a.h.push(e.hypot(n));
        a.clk.push(t.num(i, cc)?.unwrap_or(f64::NAN));
        a.fix.push(matches!(t.text(i, Some(cf)), "1" | "true"));
    }
    let mut steps = Vec::new();
    for s in &p.steps {
        let dir = METRICS
            .iter()
            .find(|m| m.0 == s.metric)
            .map(|m| m.1)
            .unwrap_or("max");
        let vals: Vec<(String, Option<f64>, usize)> = sims
            .iter()
            .map(|sim| {
                let a = acc.get(&(sim.clone(), s.id.clone()));
                (
                    sim.clone(),
                    a.and_then(|a| metric(a, &s.metric)),
                    a.map(|a| a.t.len()).unwrap_or(0),
                )
            })
            .collect();
        let present: Vec<f64> = vals.iter().filter_map(|v| v.1).collect();
        let med = quantile(&present, 0.5);
        let mad = med.and_then(|m| {
            let d: Vec<f64> = present.iter().map(|x| (x - m).abs()).collect();
            quantile(&d, 0.5).map(|x| 1.4826 * x)
        });
        let cells = vals
            .into_iter()
            .map(|(sim, v, n)| Cell {
                simulator: sim,
                value: v,
                epochs: n,
                meets_bar: match (v, s.bar) {
                    (Some(x), Some(b)) => Some(if dir == "max" { x <= b } else { x >= b }),
                    _ => None,
                },
                differs_from_others: match (v, med, mad) {
                    (Some(x), Some(m), Some(d)) if present.len() >= 3 => {
                        (x - m).abs() > 3.0 * d.max(1e-12 * m.abs().max(1.0))
                    }
                    _ => false,
                },
            })
            .collect();
        steps.push(StepScore {
            step: s.id.clone(),
            metric: s.metric.clone(),
            bar: s.bar,
            bar_source: s.bar_source.clone(),
            median: med,
            spread: mad,
            cells,
        });
    }
    Ok(Scorecard {
        format: SCORE_FORMAT,
        procedure: p.id,
        procedure_sha256: h,
        simulators: sims,
        steps,
        statement: "Simulators in input order, never ranked. 'Differs from the others' says the simulators disagree on a step, not which one is right.",
    })
}
