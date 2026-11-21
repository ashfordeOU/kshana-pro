// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Neutral multi-simulator LEO-PNT procedures and scorecards (B2101).
//!
//! A procedure (`kshana-pro-leo-sim-procedure/1`, versioned; the published ones are frozen at
//! 1.0) names its steps, the metric of each step, where the user has one a bar with its source,
//! and the tolerance within which two simulators count as equivalent. As in the LEO timing
//! scorecard, the **method hash** (SHA-256 of the canonical method part: id, version, steps) is
//! registered before the runs; a scorecard states whether the registered hash matches. Runs come
//! from each simulator as `simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix[,receiver]`. Per simulator and
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
    /// Two simulators are equivalent on this step when their metrics differ by no more.
    #[serde(default)]
    pub equivalence_tolerance: Option<f64>,
}

/// A procedure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Procedure {
    pub format: String,
    pub id: String,
    pub version: String,
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

#[derive(Serialize)]
struct MethodPart<'a> {
    method: &'a str,
    id: &'a str,
    version: &'a str,
    steps: &'a [Step],
}

/// Method identifier of the scorecard.
pub const METHOD_ID: &str = "kshana-pro/leo-sim-scorecard/v1";

/// SHA-256 (hex) of the method part of a procedure (id, version, steps as canonical JSON), to
/// register before the runs. Titles and wording are not part of it.
pub fn method_sha256(p: &Procedure) -> String {
    let m = MethodPart {
        method: METHOD_ID,
        id: &p.id,
        version: &p.version,
        steps: &p.steps,
    };
    hex::encode(Sha256::digest(serde_json::to_vec(&m).unwrap_or_default()))
}

/// The method hash of a procedure text.
pub fn procedure_hash(text: &str) -> Result<String, String> {
    let p: Procedure = toml::from_str(text).map_err(|e| format!("procedure: {e}"))?;
    p.validate()?;
    Ok(method_sha256(&p))
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
    pub procedure_version: String,
    pub method_sha256: String,
    /// Whether a registered hash was given and matches.
    pub prereg_matches: bool,
    pub label: String,
    pub simulators: Vec<String>,
    pub steps: Vec<StepScore>,
    pub statement: &'static str,
}

#[derive(Default, Clone)]
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

type Runs = (
    Vec<String>,
    Vec<String>,
    BTreeMap<(String, String, String), Acc>,
);

/// Parse runs: simulators and receivers in input order, accumulators by (simulator, receiver,
/// step).
fn parse_runs(p: &Procedure, runs: &str) -> Result<Runs, String> {
    let t = Table::parse(runs)?;
    let (cs, cst, ct) = (t.need("simulator")?, t.need("step")?, t.need("t_s")?);
    let (ce, cn, cc, cf) = (
        t.need("err_e_m")?,
        t.need("err_n_m")?,
        t.col("clock_err_s"),
        t.need("fix")?,
    );
    let crx = t.col("receiver");
    let mut sims: Vec<String> = Vec::new();
    let mut rxs: Vec<String> = Vec::new();
    let mut acc: BTreeMap<(String, String, String), Acc> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let sim = t.text(i, Some(cs)).to_string();
        if !sims.contains(&sim) {
            sims.push(sim.clone());
        }
        let rx = match t.text(i, crx) {
            "" => "receiver".to_string(),
            r => r.to_string(),
        };
        if !rxs.contains(&rx) {
            rxs.push(rx.clone());
        }
        let step = t.text(i, Some(cst)).to_string();
        if !p.steps.iter().any(|s| s.id == step) {
            return Err(format!(
                "row {}: step '{step}' is not in the procedure",
                i + 2
            ));
        }
        let a = acc.entry((sim, rx, step)).or_default();
        a.t.push(t.num(i, Some(ct))?.ok_or("empty t_s")?);
        let (e, n) = (
            t.num(i, Some(ce))?.unwrap_or(f64::NAN),
            t.num(i, Some(cn))?.unwrap_or(f64::NAN),
        );
        a.h.push(e.hypot(n));
        a.clk.push(t.num(i, cc)?.unwrap_or(f64::NAN));
        a.fix.push(matches!(t.text(i, Some(cf)), "1" | "true"));
    }
    Ok((sims, rxs, acc))
}

/// Score runs under a procedure; `prereg` is the method hash registered before the runs.
pub fn score(proc_text: &str, prereg: Option<&str>, runs: &str) -> Result<Scorecard, String> {
    let p: Procedure = toml::from_str(proc_text).map_err(|e| format!("procedure: {e}"))?;
    p.validate()?;
    let h = method_sha256(&p);
    let matches = prereg.is_some_and(|x| x.eq_ignore_ascii_case(&h));
    let (sims, _rxs, by_rx) = parse_runs(&p, runs)?;
    // The scorecard pools every receiver's runs per simulator and step.
    let mut acc: BTreeMap<(String, String), Acc> = BTreeMap::new();
    for ((sim, _, step), a) in &by_rx {
        let e = acc.entry((sim.clone(), step.clone())).or_insert(Acc {
            t: vec![],
            h: vec![],
            clk: vec![],
            fix: vec![],
        });
        e.t.extend(&a.t);
        e.h.extend(&a.h);
        e.clk.extend(&a.clk);
        e.fix.extend(&a.fix);
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
        procedure_version: p.version,
        method_sha256: h,
        prereg_matches: matches,
        label: if matches {
            "MODELLED unless the runs are named lab runs; method hash registered before the runs".into()
        } else {
            "MODELLED: the method hash was not registered before scoring, or does not match".into()
        },
        simulators: sims,
        steps,
        statement: "Simulators in input order, never ranked. 'Differs from the others' says the simulators disagree on a step, not which one is right.",
    })
}

/// One simulator pair on one step for one receiver.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PairDiff {
    pub receiver: String,
    pub a: Option<f64>,
    pub b: Option<f64>,
    /// `b − a`.
    pub diff: Option<f64>,
    pub within: Option<bool>,
}

/// Equivalence of two simulators on one step.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Equivalence {
    pub step: String,
    pub metric: String,
    pub simulator_a: String,
    pub simulator_b: String,
    pub tolerance: f64,
    pub receivers: Vec<PairDiff>,
    /// `consistent`, `simulator-deviation`, `receiver-deviation`, `unattributed` or
    /// `inconclusive` (the simulator self-test vocabulary).
    pub verdict: &'static str,
}

/// Equivalence report format.
pub const EQUIV_FORMAT: &str = "kshana-pro-leo-sim-equivalence/1";

/// The simulator-equivalence report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EquivalenceReport {
    pub format: &'static str,
    pub procedure: String,
    pub method_sha256: String,
    pub prereg_matches: bool,
    pub reference_simulator: String,
    pub steps: Vec<Equivalence>,
    pub statement: &'static str,
}

/// Equivalence of every simulator against the first one listed (the reference, by input order
/// only), per step that states a tolerance. Attribution follows the simulator self-test: to the
/// simulators when at least two receivers each differ beyond the tolerance with the same sign;
/// to the receivers that differ when the others agree; unattributed with one receiver;
/// inconclusive when all differ with opposite signs.
pub fn equivalence(
    proc_text: &str,
    prereg: Option<&str>,
    runs: &str,
) -> Result<EquivalenceReport, String> {
    let p: Procedure = toml::from_str(proc_text).map_err(|e| format!("procedure: {e}"))?;
    p.validate()?;
    let h = method_sha256(&p);
    let (sims, rxs, acc) = parse_runs(&p, runs)?;
    if sims.len() < 2 {
        return Err("equivalence needs runs from at least two simulators".into());
    }
    let a = &sims[0];
    let mut steps = Vec::new();
    for s in &p.steps {
        let Some(tol) = s.equivalence_tolerance else {
            continue;
        };
        for b in &sims[1..] {
            let get = |sim: &String, rx: &String| {
                acc.get(&(sim.clone(), rx.clone(), s.id.clone()))
                    .and_then(|x| metric(x, &s.metric))
            };
            let diffs: Vec<PairDiff> = rxs
                .iter()
                .filter_map(|rx| {
                    let (va, vb) = (get(a, rx), get(b, rx));
                    let d = vb? - va?;
                    Some(PairDiff {
                        receiver: rx.clone(),
                        a: va,
                        b: vb,
                        diff: Some(d),
                        within: Some(d.abs() <= tol),
                    })
                })
                .collect();
            let out: Vec<&PairDiff> = diffs.iter().filter(|x| x.within == Some(false)).collect();
            let verdict = if diffs.is_empty() {
                "inconclusive"
            } else if out.is_empty() {
                "consistent"
            } else if diffs.len() == 1 {
                "unattributed"
            } else if out.len() < diffs.len() {
                "receiver-deviation"
            } else {
                let pos = out.iter().filter(|x| x.diff.unwrap_or(0.0) > 0.0).count();
                if pos == out.len() || pos == 0 {
                    "simulator-deviation"
                } else {
                    "inconclusive"
                }
            };
            steps.push(Equivalence {
                step: s.id.clone(),
                metric: s.metric.clone(),
                simulator_a: a.clone(),
                simulator_b: b.clone(),
                tolerance: tol,
                receivers: diffs,
                verdict,
            });
        }
    }
    Ok(EquivalenceReport {
        format: EQUIV_FORMAT,
        procedure: p.id,
        prereg_matches: prereg.is_some_and(|x| x.eq_ignore_ascii_case(&h)),
        method_sha256: h,
        reference_simulator: a.clone(),
        steps,
        statement: "The reference simulator is the first listed, by input order only. A consistent result is evidence that the simulators agree within the stated tolerance for these receivers; it is not a calibration of either simulator.",
    })
}

/// Gravitational parameter (m³/s²), mean Earth radius (m) and speed of light (m/s).
const GM: f64 = 3.986_004_418e14;
const RE: f64 = 6_371_000.0;
const CLIGHT: f64 = 299_792_458.0;

/// Doppler bounds of a circular orbit seen from the ground, Earth rotation neglected: circular
/// speed `v = √(GM/(R+h))`; largest range rate (at the horizon) `v·R/(R+h)`; largest range
/// acceleration (overhead) `v²/h`; both scaled by `f/c`. Returns `(Hz, Hz/s)`.
pub fn doppler_bounds(altitude_km: f64, carrier_hz: f64) -> Result<(f64, f64), String> {
    if !(altitude_km > 0.0 && carrier_hz > 0.0) {
        return Err("altitude and carrier must be positive".into());
    }
    let h = altitude_km * 1000.0;
    let v = (GM / (RE + h)).sqrt();
    Ok((
        carrier_hz * v * RE / (RE + h) / CLIGHT,
        carrier_hz * v * v / h / CLIGHT,
    ))
}

/// The published procedures (frozen at 1.0): (id, TOML).
pub const PUBLISHED: &[(&str, &str)] = &[
    (
        "leo-doppler-stress",
        include_str!("../../leo/procedures/leo-doppler-stress.toml"),
    ),
    (
        "leo-acquisition",
        include_str!("../../leo/procedures/leo-acquisition.toml"),
    ),
    (
        "leo-signal-profiles",
        include_str!("../../leo/procedures/leo-signal-profiles.toml"),
    ),
];
