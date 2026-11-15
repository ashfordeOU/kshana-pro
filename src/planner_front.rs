// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Planner Pareto front with cost (roadmap 0.41 B1b, signed planned item of 0.42 F6)
//!
//! The open monitoring-network planner (`kshana::planner`, row B1a) chooses `m` stations from
//! candidates for one objective: the share of the site grid where at least `k` stations detect
//! with probability 0.9 and the TDOA DRMS meets the requirement. It has no cost. This kind adds
//! the **cost axis**: every candidate site carries a capital cost and a yearly running cost, a
//! network costs `sum(capex) + years x sum(opex)`, and the result is the **Pareto front** of cost
//! against objective over every subset of `m_min` to `m_max` stations (exhaustive, up to
//! [`MAX_SUBSETS`]), with the cheapest network that meets a target objective when one is given.
//!
//! Objectives come from the open planner unchanged ([`kshana::planner::objective_of`]); the
//! front is exact for the subsets searched and byte-identical on every run. MODELLED: the maps
//! are free-space predictions (as in B1a) and the costs are the user's.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::planner::{self, PlanSpec};
use serde::{Deserialize, Serialize};

/// The scenario kind.
pub const KIND: &str = "pro-planner-front";
/// Largest number of subsets searched.
pub const MAX_SUBSETS: u64 = 20_000;
/// The result's format tag.
pub const FORMAT: &str = "kshana-pro-planner-front/1";

/// One candidate's cost.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateCost {
    /// Capital cost (installation), in the scenario's currency.
    pub capex: f64,
    /// Running cost per year.
    pub opex_per_year: f64,
}

/// A `pro-planner-front` scenario.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannerFrontScenario {
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Path of a `kshana.planner/1` JSON document with its candidates (relative to the working
    /// directory), or the document inline as `planner_spec`.
    #[serde(default)]
    pub planner: Option<String>,
    #[serde(default)]
    pub planner_spec: Option<PlanSpec>,
    /// One cost per candidate, in candidate order.
    #[serde(rename = "cost")]
    pub costs: Vec<CandidateCost>,
    /// Years of running cost counted (default 5).
    #[serde(default = "default_years")]
    pub years: f64,
    pub m_min: usize,
    pub m_max: usize,
    /// Currency label for the outputs (default "EUR").
    #[serde(default)]
    pub currency: Option<String>,
    /// The objective a network must reach for the "cheapest meeting the target" answer.
    #[serde(default)]
    pub target_objective: Option<f64>,
}

fn default_years() -> f64 {
    5.0
}

/// One evaluated network.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub subset: Vec<usize>,
    pub cost: f64,
    pub objective: f64,
    /// Median DRMS, m (`None` when most cells are not locatable).
    pub median_drms_m: Option<f64>,
}

/// The result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Front {
    pub kind: String,
    pub format: String,
    pub title: String,
    pub currency: String,
    pub years: f64,
    pub candidates: usize,
    pub evaluated: usize,
    /// The non-dominated networks, cheapest first (objective strictly increasing).
    pub front: Vec<Point>,
    /// The best objective for each `m`, as the open planner's exhaustive search defines it.
    pub best_per_m: Vec<(usize, Point)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cheapest_meeting_target: Option<Point>,
    pub label: String,
}

fn n_choose(n: usize, k: usize) -> u64 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut r: u64 = 1;
    for i in 0..k {
        r = r.saturating_mul((n - i) as u64) / (i as u64 + 1);
    }
    r
}

/// Every subset of `0..n` of size `k`, in lexicographic order.
fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    if k == 0 || k > n {
        return out;
    }
    let mut c: Vec<usize> = (0..k).collect();
    loop {
        out.push(c.clone());
        let mut i = k;
        while i > 0 && c[i - 1] == n - k + i - 1 {
            i -= 1;
        }
        if i == 0 {
            return out;
        }
        c[i - 1] += 1;
        for j in i..k {
            c[j] = c[j - 1] + 1;
        }
    }
}

/// `true` when `a` dominates `b`: no more costly, no worse, and better in one.
pub fn dominates(a: &Point, b: &Point) -> bool {
    a.cost <= b.cost && a.objective >= b.objective && (a.cost < b.cost || a.objective > b.objective)
}

impl PlannerFrontScenario {
    pub const KIND: &'static str = KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Planner Pareto front with cost: every network of m_min to m_max stations from the open planner's candidates, the cost (capital plus years of running cost) against the planner's objective, and the cheapest network meeting a target.",
            required_fields: &["cost", "m_min", "m_max"],
            optional_fields: &["title", "planner", "planner_spec", "years", "currency", "target_objective"],
        }
    }

    fn spec(&self) -> Result<PlanSpec, String> {
        match (&self.planner, &self.planner_spec) {
            (Some(p), None) => {
                let t = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
                serde_json::from_str(&t).map_err(|e| format!("{p}: {e}"))
            }
            (None, Some(s)) => Ok(s.clone()),
            _ => Err("give exactly one of planner (a path) and planner_spec (inline)".into()),
        }
    }

    /// Compute the front.
    pub fn compute(&self) -> Result<Front, String> {
        let spec = self.spec()?;
        // The open planner validates its own document.
        planner::map(&PlanSpec {
            stations: Vec::new(),
            ..spec.clone()
        })?;
        let n = spec.candidates.len();
        if self.costs.len() != n {
            return Err(format!("{} costs for {n} candidates", self.costs.len()));
        }
        if let Some(c) = self.costs.iter().position(|c| {
            !(c.capex.is_finite()
                && c.capex >= 0.0
                && c.opex_per_year.is_finite()
                && c.opex_per_year >= 0.0)
        }) {
            return Err(format!(
                "cost {}: capex and opex_per_year are finite and not negative",
                c + 1
            ));
        }
        if !(self.years.is_finite() && self.years >= 0.0) {
            return Err("years is finite and not negative".into());
        }
        if self.m_min < 1 || self.m_max < self.m_min || self.m_max > n {
            return Err(format!("1 <= m_min <= m_max <= {n} candidates"));
        }
        let total: u64 = (self.m_min..=self.m_max).map(|m| n_choose(n, m)).sum();
        if total > MAX_SUBSETS {
            return Err(format!(
                "{total} subsets exceed the exhaustive limit of {MAX_SUBSETS}; narrow m_min..m_max or the candidates"
            ));
        }
        let mut points = Vec::new();
        for m in self.m_min..=self.m_max {
            for s in subsets(n, m) {
                let (objective, med) = planner::objective_of(&spec, &s);
                let cost: f64 = s
                    .iter()
                    .map(|&i| self.costs[i].capex + self.years * self.costs[i].opex_per_year)
                    .sum();
                points.push(Point {
                    subset: s,
                    cost,
                    objective,
                    median_drms_m: med.is_finite().then_some(med),
                });
            }
        }
        // Best per m, with the open planner's tie-break: objective, then median DRMS, then the
        // lexicographically smaller subset.
        let mut best_per_m = Vec::new();
        for m in self.m_min..=self.m_max {
            let best = points
                .iter()
                .filter(|p| p.subset.len() == m)
                .min_by(|a, b| {
                    b.objective
                        .total_cmp(&a.objective)
                        .then(
                            a.median_drms_m
                                .unwrap_or(f64::INFINITY)
                                .total_cmp(&b.median_drms_m.unwrap_or(f64::INFINITY)),
                        )
                        .then(a.subset.cmp(&b.subset))
                })
                .cloned();
            if let Some(b) = best {
                best_per_m.push((m, b));
            }
        }
        // The front: cheapest first; among equal cost the best objective; keep a point only if
        // its objective is strictly above every cheaper point's.
        let mut order: Vec<&Point> = points.iter().collect();
        order.sort_by(|a, b| {
            a.cost
                .total_cmp(&b.cost)
                .then(b.objective.total_cmp(&a.objective))
                .then(a.subset.len().cmp(&b.subset.len()))
                .then(a.subset.cmp(&b.subset))
        });
        let mut front: Vec<Point> = Vec::new();
        for p in order {
            if front.last().is_none_or(|l| p.objective > l.objective) {
                front.push(p.clone());
            }
        }
        let cheapest_meeting_target = self
            .target_objective
            .and_then(|t| front.iter().find(|p| p.objective >= t).cloned());
        Ok(Front {
            kind: KIND.into(),
            format: FORMAT.into(),
            title: self.title.clone().unwrap_or_else(|| "Planner Pareto front with cost".into()),
            currency: self.currency.clone().unwrap_or_else(|| "EUR".into()),
            years: self.years,
            candidates: n,
            evaluated: points.len(),
            front,
            best_per_m,
            cheapest_meeting_target,
            label: "MODELLED: objectives from the open planner's free-space maps (B1a); costs as given. Exhaustive over the subsets searched.".into(),
        })
    }
}

impl Front {
    pub fn csv(&self) -> String {
        let mut s = format!(
            "cost_{},objective,median_drms_m,stations,subset\n",
            self.currency.to_lowercase()
        );
        for p in &self.front {
            s.push_str(&format!(
                "{:.2},{:.6},{},{},{}\n",
                p.cost,
                p.objective,
                p.median_drms_m
                    .map(|d| format!("{d:.3}"))
                    .unwrap_or_default(),
                p.subset.len(),
                p.subset
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        s
    }
}

impl Scenario for PlannerFrontScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let f = self.compute().map_err(KshanaError::InvalidInput)?;
        let best = f.front.last();
        let summary = format!(
            "Kshana Pro planner front | {} networks evaluated, {} on the front | best objective {:.3} at {:.0} {}{} | MODELLED",
            f.evaluated,
            f.front.len(),
            best.map(|p| p.objective).unwrap_or(0.0),
            best.map(|p| p.cost).unwrap_or(0.0),
            f.currency,
            f.cheapest_meeting_target
                .as_ref()
                .map(|p| format!(" | cheapest meeting the target: {:.0} {} ({} stations)", p.cost, f.currency, p.subset.len()))
                .unwrap_or_default()
        );
        // A small front chart: cost (x) against objective (y).
        let (w, h, l, b) = (480.0, 220.0, 44.0, 28.0);
        let cmax = f.front.iter().map(|p| p.cost).fold(1.0f64, f64::max);
        let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"system-ui,sans-serif\" font-size=\"11\"><rect width=\"{w}\" height=\"{h}\" fill=\"#0f1216\"/><text x=\"{l}\" y=\"16\" fill=\"#8cc4f5\" font-size=\"13\">Cost against objective (Pareto front)</text>");
        let mut path = String::new();
        for (i, p) in f.front.iter().enumerate() {
            let x = l + (w - l - 12.0) * p.cost / cmax;
            let y = h - b - (h - b - 28.0) * p.objective;
            path.push_str(&format!("{}{x:.1},{y:.1}", if i == 0 { "M" } else { "L" }));
            svg.push_str(&format!(
                "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"3\" fill=\"#8cc4f5\"/>"
            ));
        }
        svg.push_str(&format!("<path d=\"{path}\" fill=\"none\" stroke=\"#8cc4f5\"/><text x=\"{l}\" y=\"{}\" fill=\"#9aa3af\">cost ({}) →</text></svg>", h - 8.0, f.currency));
        Ok(RunOutput {
            json: serde_json::to_string(&f).expect("the front is plain data"),
            svg,
            summary,
            csv: Some(f.csv()),
        })
    }
}

impl ExternalPack for PlannerFrontScenario {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(KIND));
    }
}
