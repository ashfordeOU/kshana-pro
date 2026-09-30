// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Campaign watch** — the `[[watch]]` tables of a scenario-pack manifest, and their
//! evaluation against one run.
//!
//! A watch names one figure of one pack entry and says what "worse" means for it:
//!
//! ```toml
//! [[watch]]
//! name = "holdover end time error"
//! entry = "mission"                                  # the entry's label
//! metric = "campaign.phase.holdover.end.time_error_ns"
//! direction = "lower"                                # higher | lower | target
//! tolerance = { rel = 0.02, abs = 0.5 }              # allowed worsening
//! requirement = "watched <= 70 ns"                   # optional absolute criterion
//! ```
//!
//! `metric` is a result path in the grammar of [`crate::mbse::criterion`] (aggregates
//! included, so `max(timeline.channels.time_error_ns.values[])` works), a named campaign
//! figure from [`crate::pack_c::campaign`], or, with `crossing`, the name of a sweep
//! metric. `requirement` is a criterion evaluated against the entry's result document,
//! with units checked; the word `watched` in it names the watched figure itself, in the
//! figure's unit. `ensemble` gives the paths of the mean, standard deviation and sample
//! count of a Monte Carlo figure; such a watch is decided by the statistical gate
//! ([`crate::pack_c::stat_gate`]) instead of a tolerance.
//!
//! A watch never guesses. A figure whose path does not resolve is reported as *not
//! found* and fails the gate; a requirement that cannot be checked (a missing unit, a
//! unit mismatch) is *not verified* and fails the gate too; a figure with no declared
//! direction is informational and never regresses.

use crate::mbse::criterion::{self, Operand, UnitLookup, Verdict};
use crate::mbse::units::parse_unit;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The default significance level of the statistical gate.
pub const DEFAULT_ALPHA: f64 = 0.01;

/// The word that names the watched figure inside a watch's `requirement`.
pub const WATCHED: &str = "watched";

/// Allowed worsening before a watched figure is a regression.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tolerance {
    /// Relative allowance, as a fraction of the baseline magnitude (0.05 = 5 %).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel: Option<f64>,
    /// Absolute allowance, in the figure's own unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abs: Option<f64>,
}

impl Tolerance {
    /// The allowed worsening against `baseline`: the larger of the two allowances (the
    /// more permissive one), or `default_rel` of the baseline when neither is set.
    pub fn allowance(&self, baseline: f64, default_rel: f64) -> f64 {
        if self.rel.is_none() && self.abs.is_none() {
            return default_rel * baseline.abs();
        }
        let rel = self.rel.map(|r| r * baseline.abs()).unwrap_or(0.0);
        let abs = self.abs.unwrap_or(0.0);
        rel.max(abs)
    }
}

/// The result paths of a Monte Carlo figure's mean, standard deviation and count.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ensemble {
    /// Path of the ensemble mean.
    pub mean: String,
    /// Path of the ensemble standard deviation (the engine reports the population one).
    pub std: String,
    /// Path of the number of realisations.
    pub n: String,
}

/// For a sweep: the threshold whose first crossing along the first axis is the figure.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crossing {
    /// The figure is the first axis value at which the metric falls below this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<f64>,
    /// The figure is the first axis value at which the metric rises above this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above: Option<f64>,
}

/// One `[[watch]]` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watch {
    /// Optional display name; also the watch's identity across runs when given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The label of the pack entry whose result is watched.
    pub entry: String,
    /// The watched figure (see the module documentation).
    pub metric: String,
    /// `higher`, `lower` or `target`; when absent, the Pack C name table decides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// The target value for `direction = "target"`, in the figure's unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<f64>,
    /// Allowed worsening; when absent the run's default relative threshold applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<Tolerance>,
    /// An absolute acceptance criterion over the entry's result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    /// The Monte Carlo statistics of the figure, for the statistical gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ensemble: Option<Ensemble>,
    /// Significance level of the statistical gate (default 0.01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha: Option<f64>,
    /// For a sweep: watch the first axis value at which `metric` crosses a threshold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crossing: Option<Crossing>,
    /// Free text: where a requirement comes from, why the figure matters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A watch's resolved better-or-worse direction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "value")]
pub enum WatchDirection {
    /// Larger is better.
    Higher,
    /// Smaller is better.
    Lower,
    /// Closer to the value is better.
    Target(f64),
    /// No declared direction: informational.
    None,
}

impl WatchDirection {
    /// A short word for reports.
    pub fn describe(&self) -> String {
        match self {
            WatchDirection::Higher => "higher is better".into(),
            WatchDirection::Lower => "lower is better".into(),
            WatchDirection::Target(t) => format!("closer to {t} is better"),
            WatchDirection::None => "no declared direction (informational)".into(),
        }
    }

    /// How much worse `current` is than `baseline` (positive = worse); `None` when
    /// the direction is unknown.
    pub fn degradation(&self, baseline: f64, current: f64) -> Option<f64> {
        match self {
            WatchDirection::Higher => Some(baseline - current),
            WatchDirection::Lower => Some(current - baseline),
            WatchDirection::Target(t) => Some((current - t).abs() - (baseline - t).abs()),
            WatchDirection::None => None,
        }
    }
}

impl Watch {
    /// The watch's identity across runs: its name, else `entry::metric` (plus the
    /// crossing threshold for a sweep crossing).
    pub fn id(&self) -> String {
        if let Some(n) = self.name.as_ref().filter(|n| !n.trim().is_empty()) {
            return n.trim().to_string();
        }
        format!("{}::{}", self.entry, self.figure_name())
    }

    /// The figure's name as it appears in metric keys.
    pub fn figure_name(&self) -> String {
        match &self.crossing {
            Some(c) => match (c.below, c.above) {
                (Some(b), _) => format!("sweep_crossing({} < {b})", self.metric),
                (_, Some(a)) => format!("sweep_crossing({} > {a})", self.metric),
                _ => format!("sweep_crossing({})", self.metric),
            },
            None => self.metric.clone(),
        }
    }

    /// The resolved direction: the manifest's, else the Pack C name table's, else none.
    pub fn resolved_direction(&self) -> WatchDirection {
        match self.direction.as_deref() {
            Some("higher") => WatchDirection::Higher,
            Some("lower") => WatchDirection::Lower,
            Some("target") => WatchDirection::Target(self.target.unwrap_or(0.0)),
            _ => match crate::pack_c::gate::direction_for(&self.metric) {
                Some(crate::pack_c::gate::Direction::HigherIsBetter) => WatchDirection::Higher,
                Some(crate::pack_c::gate::Direction::LowerIsBetter) => WatchDirection::Lower,
                None => WatchDirection::None,
            },
        }
    }

    /// The statistical gate's significance level.
    pub fn alpha(&self) -> f64 {
        self.alpha.unwrap_or(DEFAULT_ALPHA)
    }

    /// Validate one watch against the pack's entry labels. The error names the watch
    /// and what is wrong.
    pub fn validate(&self, labels: &[String]) -> Result<(), String> {
        let who = format!("watch '{}'", self.id());
        if self.entry.trim().is_empty() {
            return Err(format!("{who}: 'entry' must not be empty"));
        }
        if !labels.iter().any(|l| l == &self.entry) {
            return Err(format!(
                "{who}: entry '{}' is not a label of this pack (labels: {})",
                self.entry,
                labels.join(", ")
            ));
        }
        if self.metric.trim().is_empty() {
            return Err(format!("{who}: 'metric' must not be empty"));
        }
        match self.direction.as_deref() {
            None | Some("higher") | Some("lower") => {
                if self.target.is_some() {
                    return Err(format!(
                        "{who}: 'target' is only meaningful with direction = \"target\""
                    ));
                }
            }
            Some("target") => match self.target {
                Some(t) if t.is_finite() => {}
                Some(_) => return Err(format!("{who}: 'target' must be a finite number")),
                None => return Err(format!("{who}: direction = \"target\" needs 'target'")),
            },
            Some(other) => {
                return Err(format!(
                    "{who}: direction '{other}' is not one of higher, lower, target"
                ))
            }
        }
        if let Some(t) = &self.tolerance {
            for (name, v) in [("rel", t.rel), ("abs", t.abs)] {
                if let Some(v) = v {
                    if !v.is_finite() || v < 0.0 {
                        return Err(format!(
                            "{who}: tolerance.{name} must be a finite, non-negative number (got {v})"
                        ));
                    }
                }
            }
            if self.ensemble.is_some() {
                return Err(format!(
                    "{who}: a watch with 'ensemble' is decided by the statistical gate; remove 'tolerance'"
                ));
            }
        }
        if let Some(a) = self.alpha {
            if !(a.is_finite() && a > 0.0 && a < 0.5) {
                return Err(format!(
                    "{who}: alpha must be a number between 0 and 0.5 (got {a})"
                ));
            }
            if self.ensemble.is_none() {
                return Err(format!("{who}: 'alpha' needs 'ensemble'"));
            }
        }
        if let Some(e) = &self.ensemble {
            for (name, p) in [("mean", &e.mean), ("std", &e.std), ("n", &e.n)] {
                if p.trim().is_empty() {
                    return Err(format!("{who}: ensemble.{name} must not be empty"));
                }
            }
            if self.crossing.is_some() {
                return Err(format!(
                    "{who}: 'ensemble' and 'crossing' cannot be combined"
                ));
            }
        }
        if let Some(c) = &self.crossing {
            match (c.below, c.above) {
                (Some(v), None) | (None, Some(v)) if v.is_finite() => {}
                (Some(_), Some(_)) => {
                    return Err(format!("{who}: crossing takes one of 'below' or 'above'"))
                }
                (None, None) => return Err(format!("{who}: crossing needs 'below' or 'above'")),
                _ => return Err(format!("{who}: the crossing threshold must be finite")),
            }
        }
        if let Some(r) = &self.requirement {
            criterion::parse(r).map_err(|e| format!("{who}: requirement: {e}"))?;
        }
        Ok(())
    }

    /// The numeric limits a requirement draws on the watched figure, converted into the
    /// figure's unit: every clause that compares `watched` (or the watch's own metric
    /// path) with a literal. Used to draw requirement lines on a trend chart.
    pub fn limits(&self, figure_unit: Option<&str>) -> Vec<Limit> {
        let Some(src) = &self.requirement else {
            return Vec::new();
        };
        let Ok(c) = criterion::parse(src) else {
            return Vec::new();
        };
        let names_figure = |o: &Operand| match o {
            Operand::Path(p) => p == WATCHED || p == &self.metric,
            _ => false,
        };
        let mut out = Vec::new();
        for cl in &c.clauses {
            let (lit, op) = if names_figure(&cl.cmp.lhs) {
                (&cl.cmp.rhs, op_text(&cl.cmp.op, false))
            } else if names_figure(&cl.cmp.rhs) {
                (&cl.cmp.lhs, op_text(&cl.cmp.op, true))
            } else {
                continue;
            };
            let Operand::Number { value, unit } = lit else {
                continue;
            };
            let v = match (unit, figure_unit) {
                (None, _) => Some(*value),
                (Some(u), Some(fu)) => match (parse_unit(u), parse_unit(fu)) {
                    (Some(from), Some(to)) if from.commensurable(&to) => from.convert(*value, &to),
                    _ => None,
                },
                (Some(_), None) => None,
            };
            if let Some(v) = v.filter(|v| v.is_finite()) {
                out.push(Limit { op, value: v });
            }
        }
        out
    }
}

fn op_text(op: &criterion::CmpOp, mirrored: bool) -> String {
    use criterion::CmpOp::*;
    let s = match (op, mirrored) {
        (Lt, false) | (Gt, true) => "<",
        (Le, false) | (Ge, true) => "<=",
        (Gt, false) | (Lt, true) => ">",
        (Ge, false) | (Le, true) => ">=",
        (Eq, _) => "==",
        (Ne, _) => "!=",
    };
    s.to_string()
}

/// One requirement limit on a watched figure, in the figure's unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    /// The comparison the figure must satisfy against `value`.
    pub op: String,
    /// The limit, in the figure's unit.
    pub value: f64,
}

/// A [`UnitLookup`] over a result document's own `units` block, in the open engine's
/// path grammar.
pub struct DocUnits<'a> {
    /// The document whose `units` block is read.
    pub doc: &'a Value,
    /// Describes where a unit came from in evidence text.
    pub source: &'a str,
}

impl UnitLookup for DocUnits<'_> {
    fn unit_for(&self, canonical_path: &str) -> Option<(String, String)> {
        let units = self.doc.get("units")?.as_object()?;
        let (key, v) = kshana::field_schema::lookup(units, canonical_path)?;
        let unit = v.get("unit")?.as_str()?.to_string();
        Some((unit, format!("{}: units.\"{key}\"", self.source)))
    }
}

/// Resolve a plain dotted path (`a.b.0.c` or `a.b[0].c`) to a numeric leaf.
pub fn resolve_plain(doc: &Value, path: &str) -> Option<f64> {
    let mut cur = doc;
    for seg in path.split('.') {
        let (name, idx) = match seg.find('[') {
            Some(i) => {
                let inner = seg[i + 1..].strip_suffix(']')?;
                (&seg[..i], Some(inner.parse::<usize>().ok()?))
            }
            None => (seg, None),
        };
        if name.is_empty() {
            return None;
        }
        cur = match cur {
            Value::Object(m) => m.get(name)?,
            Value::Array(a) => a.get(name.parse::<usize>().ok()?)?,
            _ => return None,
        };
        if let Some(i) = idx {
            cur = cur.as_array()?.get(i)?;
        }
    }
    cur.as_f64()
}

/// The canonical (`[]`-suffixed) spelling of a plain dotted path: numeric segments
/// and `[i]` indexes become `[]` on the array they index.
pub fn canonical(path: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for seg in path.split('.') {
        if !seg.is_empty() && seg.chars().all(|c| c.is_ascii_digit()) {
            if let Some(last) = out.last_mut() {
                last.push_str("[]");
                continue;
            }
        }
        match seg.find('[') {
            Some(i) => out.push(format!("{}[]", &seg[..i])),
            None => out.push(seg.to_string()),
        }
    }
    out.join(".")
}

/// Resolve a watched metric against a (campaign-augmented) result document: a plain
/// path first, then a criterion expression (aggregates, `[]` columns). Returns the value
/// and its unit, or why it could not be read.
pub fn resolve_metric(
    doc: &Value,
    metric: &str,
    source: &str,
) -> Result<(f64, Option<String>), String> {
    let lookup = DocUnits { doc, source };
    if let Some(v) = resolve_plain(doc, metric) {
        let unit = lookup.unit_for(&canonical(metric)).map(|(u, _)| u);
        return Ok((v, unit));
    }
    // A criterion that compares the expression with itself evaluates the expression
    // through the criterion grammar (aggregates, units) without changing that module.
    let expr = format!("{metric} == {metric}");
    let c = criterion::parse(&expr)
        .map_err(|e| format!("'{metric}' is not a path or expression: {e}"))?;
    let ev = criterion::evaluate(&c, doc, &lookup);
    match ev.verdict {
        Verdict::Met | Verdict::NotMet => {
            let obs = ev
                .observations
                .first()
                .ok_or_else(|| format!("'{metric}' produced no value"))?;
            let v = obs
                .value
                .as_f64()
                .ok_or_else(|| format!("'{metric}' is not a number"))?;
            Ok((v, obs.unit.clone()))
        }
        Verdict::NotVerified => Err(ev
            .reason
            .unwrap_or_else(|| format!("'{metric}' could not be read"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn w(src: &str) -> Watch {
        toml::from_str(src).expect("watch parses")
    }

    #[test]
    fn validation_catches_the_adversarial_cases() {
        let labels = vec!["a".to_string()];
        assert!(w("entry = \"a\"\nmetric = \"x\"").validate(&labels).is_ok());
        let e = w("entry = \"nope\"\nmetric = \"x\"")
            .validate(&labels)
            .unwrap_err();
        assert!(e.contains("not a label"), "{e}");
        let e = w("entry = \"a\"\nmetric = \"x\"\ntolerance = { rel = nan }")
            .validate(&labels)
            .unwrap_err();
        assert!(e.contains("finite"), "{e}");
        let e = w("entry = \"a\"\nmetric = \"x\"\ndirection = \"up\"")
            .validate(&labels)
            .unwrap_err();
        assert!(e.contains("not one of"), "{e}");
        let e = w("entry = \"a\"\nmetric = \"x\"\ndirection = \"target\"")
            .validate(&labels)
            .unwrap_err();
        assert!(e.contains("needs 'target'"), "{e}");
        let e = w("entry = \"a\"\nmetric = \"x\"\nrequirement = \"x <<< 3\"")
            .validate(&labels)
            .unwrap_err();
        assert!(e.contains("requirement"), "{e}");
    }

    #[test]
    fn tolerance_takes_the_more_permissive_allowance() {
        let t = Tolerance {
            rel: Some(0.1),
            abs: Some(0.5),
        };
        assert_eq!(t.allowance(2.0, 0.05), 0.5);
        assert_eq!(t.allowance(20.0, 0.05), 2.0);
        let none = Tolerance {
            rel: None,
            abs: None,
        };
        assert_eq!(none.allowance(10.0, 0.05), 0.5);
    }

    #[test]
    fn direction_degradation_signs() {
        assert_eq!(WatchDirection::Higher.degradation(10.0, 8.0), Some(2.0));
        assert_eq!(WatchDirection::Lower.degradation(10.0, 8.0), Some(-2.0));
        assert_eq!(
            WatchDirection::Target(0.0).degradation(-1.0, 3.0),
            Some(2.0)
        );
        assert_eq!(WatchDirection::None.degradation(1.0, 2.0), None);
    }

    #[test]
    fn limits_are_converted_into_the_figure_unit() {
        let x = w(
            "entry = \"a\"\nmetric = \"t\"\nrequirement = \"watched <= 1 us and 20 ns <= watched\"",
        );
        let l = x.limits(Some("ns"));
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].op, "<=");
        assert!((l[0].value - 1000.0).abs() < 1e-9);
        assert_eq!(l[1].op, ">=");
        assert!((l[1].value - 20.0).abs() < 1e-12);
    }

    #[test]
    fn metric_resolution_plain_indexed_and_aggregate() {
        let doc = json!({
            "a": {"b": [ {"c": 1.5}, {"c": 4.0} ]},
            "units": {"a.b[].c": {"unit": "m", "provenance": "computed"}}
        });
        assert_eq!(
            resolve_metric(&doc, "a.b.1.c", "t").unwrap(),
            (4.0, Some("m".into()))
        );
        assert_eq!(
            resolve_metric(&doc, "a.b[0].c", "t").unwrap(),
            (1.5, Some("m".into()))
        );
        assert_eq!(
            resolve_metric(&doc, "max(a.b[].c)", "t").unwrap(),
            (4.0, Some("m".into()))
        );
        assert!(resolve_metric(&doc, "a.missing", "t").is_err());
        assert_eq!(canonical("a.b.1.c"), "a.b[].c");
        assert_eq!(canonical("a.b[0].c"), "a.b[].c");
    }
}
