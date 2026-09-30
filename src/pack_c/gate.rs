// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Regression gate** — compare a current [`MetricSet`] against a baseline and
//! decide pass/fail under per-key relative thresholds.
//!
//! The gate is *honest decision-support*, not a certification: it only fails a
//! metric whose better/worse direction it actually knows (see [`direction_for`]).
//! A metric of unknown direction is reported as **informational** — surfaced so a
//! human can look, but never silently turned into a pass or a fail. A baseline of
//! (near-)zero is likewise informational, because no relative change can be formed
//! against it without dividing by zero.
//!
//! For a known-direction metric the gate computes a signed `rel_change` of
//! `(current - baseline) / |baseline|` and a one-sided *degradation* magnitude
//! that is positive exactly when the current run is worse. If degradation exceeds
//! the metric's threshold it is a [`Regression`]; a strictly-better metric is an
//! improvement; anything in between is silently within tolerance. The overall
//! verdict passes iff there are no regressions. Iteration is over the sorted
//! intersection of both [`MetricSet`]s, so the verdict is fully deterministic.

use crate::pack_c::metrics::MetricSet;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Whether a figure of merit is better when it goes up or when it goes down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// A larger value is better (availability, AUC, holdover time, …).
    HigherIsBetter,
    /// A smaller value is better (protection levels, errors, RMSE, fault rate, …).
    LowerIsBetter,
}

/// Infer the optimisation direction of a metric from its key, or `None` when the
/// direction is unknown (in which case the gate treats it as informational).
///
/// The decision is made on the metric's *leaf* name — the substring after the
/// last `.` if the key contains one, else after the last `:` if it contains one,
/// else the whole key. So `"araim::fom.mean_hpl_m"` is judged on `"mean_hpl_m"`.
///
/// The exact [`HigherIsBetter`](Direction::HigherIsBetter) set is checked *before*
/// the [`LowerIsBetter`](Direction::LowerIsBetter) patterns.
pub fn direction_for(metric_key: &str) -> Option<Direction> {
    let leaf = if let Some(idx) = metric_key.rfind('.') {
        &metric_key[idx + 1..]
    } else if let Some(idx) = metric_key.rfind(':') {
        &metric_key[idx + 1..]
    } else {
        metric_key
    };

    // Higher-is-better exact names take priority over the lower-is-better
    // patterns (e.g. `holdover_s` ends in nothing telling, so it is listed here).
    const HIGHER: [&str; 6] = [
        "raim_availability",
        "availability",
        "detect_auc",
        "integrity",
        "security",
        "holdover_s",
    ];
    if HIGHER.contains(&leaf) {
        return Some(Direction::HigherIsBetter);
    }

    if leaf.ends_with("_m")
        || leaf.ends_with("_ns")
        || leaf == "fault_rate"
        || leaf.contains("error")
        || leaf.contains("rmse")
        || leaf.ends_with("_pl")
        || leaf.contains("protection")
    {
        return Some(Direction::LowerIsBetter);
    }

    None
}

/// Relative-change tolerances for the gate: a default plus optional per-key
/// overrides. A tolerance of `0.05` means "a 5% degradation is allowed".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    /// The tolerance applied to any metric without a per-key override.
    pub default_rel: f64,
    /// Per-metric tolerance overrides, keyed exactly by the full metric key.
    pub per_key: BTreeMap<String, f64>,
}

impl Thresholds {
    /// A threshold set with the given default tolerance and no per-key overrides.
    pub fn new(default_rel: f64) -> Self {
        Self {
            default_rel,
            per_key: BTreeMap::new(),
        }
    }

    /// Builder: set a per-key tolerance override, returning `self`.
    pub fn with(mut self, key: impl Into<String>, rel: f64) -> Self {
        self.per_key.insert(key.into(), rel);
        self
    }

    /// The tolerance for `key`: its per-key override if present, else the default.
    pub fn rel_for(&self, key: &str) -> f64 {
        self.per_key.get(key).copied().unwrap_or(self.default_rel)
    }
}

impl Default for Thresholds {
    /// A 5% default tolerance with no per-key overrides.
    fn default() -> Self {
        Self::new(0.05)
    }
}

/// A single metric that degraded beyond its threshold.
///
/// `rel_change` is signed `(current - baseline) / |baseline|`: it is negative
/// when worse for a [`HigherIsBetter`](Direction::HigherIsBetter) metric and
/// positive when worse for a [`LowerIsBetter`](Direction::LowerIsBetter) one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Regression {
    /// The full metric key that regressed.
    pub key: String,
    /// The baseline value.
    pub baseline: f64,
    /// The current value.
    pub current: f64,
    /// Signed relative change `(current - baseline) / |baseline|`.
    pub rel_change: f64,
    /// The tolerance that was exceeded.
    pub threshold: f64,
    /// The metric's optimisation direction.
    pub direction: Direction,
}

/// The outcome of a gate comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GateVerdict {
    /// Whether the gate passed (no regressions).
    pub passed: bool,
    /// Metrics that degraded beyond their threshold.
    pub regressions: Vec<Regression>,
    /// Keys of metrics that strictly improved.
    pub improvements: Vec<String>,
    /// Keys reported for information only (unknown direction or zero baseline).
    pub informational: Vec<String>,
}

impl GateVerdict {
    /// A process exit code: `0` on pass, `1` on fail.
    pub fn exit_code(&self) -> i32 {
        if self.passed {
            0
        } else {
            1
        }
    }
}

/// Below this absolute baseline magnitude no relative change can be formed.
const ZERO_BASELINE_EPS: f64 = 1e-12;

/// Compare `current` against `baseline` under `thr`, producing a [`GateVerdict`].
///
/// Only keys present in *both* sets are gated, iterated in sorted order. Each key
/// of known [`Direction`] with a non-zero baseline is classified as a regression,
/// an improvement, or within-tolerance; unknown-direction or zero-baseline keys
/// are reported as informational. The verdict passes iff there are no regressions.
pub fn compare(baseline: &MetricSet, current: &MetricSet, thr: &Thresholds) -> GateVerdict {
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    let mut informational = Vec::new();

    // Sorted intersection: BTreeMap iterates keys in order, so this is stable.
    for (key, &b) in baseline.0.iter() {
        let Some(&c) = current.0.get(key) else {
            continue; // Key absent from current: not gated.
        };

        let Some(dir) = direction_for(key) else {
            informational.push(key.clone());
            continue;
        };

        if b.abs() < ZERO_BASELINE_EPS {
            informational.push(key.clone());
            continue;
        }

        let rel_change = (c - b) / b.abs();
        let deg = match dir {
            Direction::HigherIsBetter => (b - c) / b.abs(),
            Direction::LowerIsBetter => (c - b) / b.abs(),
        };

        let t = thr.rel_for(key);
        if deg > t {
            regressions.push(Regression {
                key: key.clone(),
                baseline: b,
                current: c,
                rel_change,
                threshold: t,
                direction: dir,
            });
        } else if deg < 0.0 {
            improvements.push(key.clone());
        }
        // Else: within tolerance and not strictly better — record nothing.
    }

    let passed = regressions.is_empty();
    GateVerdict {
        passed,
        regressions,
        improvements,
        informational,
    }
}

/// Classify one watched figure against its baseline: `unchanged` when equal, `changed`
/// when the watch has no direction, else `regressed` when it worsened by more than
/// `allowance` (in the figure's unit), `improved` when it got better, and
/// `within-tolerance` otherwise.
pub fn judge_watch(
    direction: crate::pack_c::watch::WatchDirection,
    allowance: f64,
    baseline: f64,
    current: f64,
) -> &'static str {
    if current == baseline {
        return "unchanged";
    }
    match direction.degradation(baseline, current) {
        None => "changed",
        Some(d) if d > allowance => "regressed",
        Some(d) if d < 0.0 => "improved",
        Some(_) => "within-tolerance",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_watch_classifies_by_direction() {
        use crate::pack_c::watch::WatchDirection as D;
        assert_eq!(judge_watch(D::Higher, 0.0, 100.0, 99.0), "regressed");
        assert_eq!(judge_watch(D::Higher, 0.0, 99.0, 100.0), "improved");
        assert_eq!(judge_watch(D::Higher, 2.0, 100.0, 99.0), "within-tolerance");
        assert_eq!(judge_watch(D::Lower, 0.0, 1.0, 1.0), "unchanged");
        assert_eq!(judge_watch(D::None, 0.0, 1.0, 5.0), "changed");
        assert_eq!(
            judge_watch(D::Target(0.0), 0.1, -1.0, 1.05),
            "within-tolerance"
        );
    }

    #[test]
    fn direction_for_classifies_known_and_unknown_leaves() {
        assert_eq!(
            direction_for("araim::fom.raim_availability"),
            Some(Direction::HigherIsBetter)
        );
        assert_eq!(
            direction_for("x::fom.mean_hpl_m"),
            Some(Direction::LowerIsBetter)
        );
        assert_eq!(
            direction_for("x::fom.fault_rate"),
            Some(Direction::LowerIsBetter)
        );
        assert_eq!(
            direction_for("x::clock.holdover_s"),
            Some(Direction::HigherIsBetter)
        );
        assert_eq!(direction_for("x::weird_metric"), None);
    }

    #[test]
    fn higher_is_better_degradation_is_a_regression() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.90)]));
        let current = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.80)]));
        let v = compare(&baseline, &current, &Thresholds::new(0.05));

        assert!(!v.passed);
        assert_eq!(v.regressions.len(), 1);
        assert_eq!(v.exit_code(), 1);
        let r = &v.regressions[0];
        assert_eq!(r.key, "s::fom.availability");
        assert_eq!(r.direction, Direction::HigherIsBetter);
    }

    #[test]
    fn lower_is_better_degradation_is_a_regression() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.mean_hpl_m".to_string(), 20.0)]));
        let current = MetricSet(BTreeMap::from([("s::fom.mean_hpl_m".to_string(), 25.0)]));
        let v = compare(&baseline, &current, &Thresholds::new(0.05));

        assert!(!v.passed);
        assert_eq!(v.regressions.len(), 1);
        let r = &v.regressions[0];
        assert_eq!(r.direction, Direction::LowerIsBetter);
        // 25% worse, signed rel_change positive (higher = worse here).
        assert!((r.rel_change - 0.25).abs() < 1e-9);
    }

    #[test]
    fn within_tolerance_passes() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.90)]));
        let current = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.89)]));
        let v = compare(&baseline, &current, &Thresholds::new(0.05));

        assert!(v.passed);
        assert!(v.regressions.is_empty());
        assert_eq!(v.exit_code(), 0);
    }

    #[test]
    fn improvement_is_recorded_and_passes() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.80)]));
        let current = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.90)]));
        let v = compare(&baseline, &current, &Thresholds::new(0.05));

        assert!(v.passed);
        assert!(v.regressions.is_empty());
        assert_eq!(v.improvements, vec!["s::fom.availability".to_string()]);
    }

    #[test]
    fn unknown_direction_key_is_informational_never_a_regression() {
        let baseline = MetricSet(BTreeMap::from([("s::weird_metric".to_string(), 1.0)]));
        // A huge drop that would regress any known-direction metric.
        let current = MetricSet(BTreeMap::from([("s::weird_metric".to_string(), 0.01)]));
        let v = compare(&baseline, &current, &Thresholds::new(0.05));

        assert!(v.passed);
        assert!(v.regressions.is_empty());
        assert_eq!(v.informational, vec!["s::weird_metric".to_string()]);
    }

    #[test]
    fn per_key_threshold_overrides_default() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.90)]));
        // ~5.6% drop: passes a lax default of 0.10 but fails a tight 0.01 override.
        let current = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.85)]));

        let lax = compare(&baseline, &current, &Thresholds::new(0.10));
        assert!(lax.passed);

        let tight = compare(
            &baseline,
            &current,
            &Thresholds::new(0.10).with("s::fom.availability", 0.01),
        );
        assert!(!tight.passed);
        assert_eq!(tight.regressions.len(), 1);
        assert_eq!(tight.regressions[0].threshold, 0.01);
    }

    #[test]
    fn zero_baseline_is_informational_without_panic() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.mean_hpl_m".to_string(), 0.0)]));
        let current = MetricSet(BTreeMap::from([("s::fom.mean_hpl_m".to_string(), 5.0)]));
        let v = compare(&baseline, &current, &Thresholds::default());

        assert!(v.passed);
        assert!(v.regressions.is_empty());
        assert_eq!(v.informational, vec!["s::fom.mean_hpl_m".to_string()]);
    }

    #[test]
    fn key_only_in_current_is_ignored() {
        let baseline = MetricSet(BTreeMap::from([("s::fom.availability".to_string(), 0.90)]));
        let current = MetricSet(BTreeMap::from([
            ("s::fom.availability".to_string(), 0.90),
            ("s::fom.mean_hpl_m".to_string(), 99.0),
        ]));
        let v = compare(&baseline, &current, &Thresholds::default());

        assert!(v.passed);
        assert!(v.regressions.is_empty());
        // The current-only key never appears anywhere in the verdict.
        assert!(!v.informational.contains(&"s::fom.mean_hpl_m".to_string()));
        assert!(!v.improvements.contains(&"s::fom.mean_hpl_m".to_string()));
    }
}
