// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! **Figure-of-merit extraction** — a scenario run's result JSON, flattened.
//!
//! When the open engine runs a scenario it returns a result JSON whose top level
//! carries bookkeeping (`schema_version`, `engine_version`, `scenario_hash`,
//! `seed`) plus a `fom` object — and, for some scenarios, further nested objects
//! (e.g. `quantum` / `classical` clock branches). [`extract`] walks that value
//! and collects every *numeric leaf* into a flat, comparable [`MetricSet`] keyed
//! by `"{label}::{dotted_path}"`, so a later regression gate can compare runs
//! metric-by-metric without knowing each scenario's shape.
//!
//! The bookkeeping numbers `schema_version` and `seed` (top level only) are
//! deliberately *not* metrics and are skipped; string/boolean/null leaves are
//! naturally non-numeric and skipped too. Ordering is stable ([`BTreeMap`]) so
//! the same JSON always yields an equal `MetricSet`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A flat, comparable map of figure-of-merit metrics: `"{label}::{path}" -> value`.
///
/// Serialises transparently as a JSON object (serde newtype over the inner map).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricSet(pub BTreeMap<String, f64>);

impl MetricSet {
    /// The metric value at `key`, if present.
    pub fn get(&self, key: &str) -> Option<f64> {
        self.0.get(key).copied()
    }

    /// Insert all of `other`'s entries; `other` wins on key collision.
    pub fn merge(&mut self, other: MetricSet) {
        for (k, v) in other.0 {
            self.0.insert(k, v);
        }
    }

    /// Iterate the metric keys in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    /// The number of metrics held.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no metrics.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Extract every numeric leaf of a scenario run's `result_json` into a
/// [`MetricSet`], keyed by `"{label}::{dotted_path}"`.
///
/// Dotted paths append `.key` for object entries and `.{index}` for array
/// elements; the root path is empty (so a top-level number `x` has path `"x"`).
/// The top-level bookkeeping keys `schema_version` and `seed` are skipped;
/// non-numeric leaves (strings, booleans, null) are skipped naturally.
///
/// Returns `Err` if `result_json` is not valid JSON.
pub fn extract(label: &str, result_json: &str) -> Result<MetricSet, String> {
    let value: serde_json::Value =
        serde_json::from_str(result_json).map_err(|e| format!("invalid result JSON: {e}"))?;

    let mut out = MetricSet::default();
    walk_root(label, &value, &mut out);
    Ok(out)
}

/// Walk the top-level value, skipping the bookkeeping keys `schema_version` and
/// `seed`, then recursing into everything else.
fn walk_root(label: &str, value: &serde_json::Value, out: &mut MetricSet) {
    if let serde_json::Value::Object(map) = value {
        for (key, child) in map {
            if key == "schema_version" || key == "seed" {
                continue;
            }
            walk(label, key, child, out);
        }
    } else {
        // A non-object root (e.g. a bare number/array) still gets walked with an
        // empty path so a top-level numeric leaf is recorded as `"{label}::"`.
        walk(label, "", value, out);
    }
}

/// Recursively collect numeric leaves of `value` under the dotted `path`.
fn walk(label: &str, path: &str, value: &serde_json::Value, out: &mut MetricSet) {
    match value {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                out.0.insert(format!("{label}::{path}"), f);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk(label, &child_path, child, out);
            }
        }
        serde_json::Value::Array(items) => {
            // Raw-data arrays (e.g. per-epoch GNSS measurement tables) are not figures of
            // merit; flattening them bloats the report and the gate's informational list.
            // Cap recursion to short arrays, which is all that legitimate FoM vectors need.
            const MAX_ARRAY: usize = 8;
            if items.len() > MAX_ARRAY {
                return;
            }
            for (idx, child) in items.iter().enumerate() {
                let child_path = if path.is_empty() {
                    idx.to_string()
                } else {
                    format!("{path}.{idx}")
                };
                walk(label, &child_path, child, out);
            }
        }
        // Strings, booleans, null: non-numeric, skipped.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_araim_fom_skipping_bookkeeping_and_strings() {
        let json = r#"{"schema_version":1,"seed":42,"fom":{"raim_availability":0.82,"mean_hpl_m":22.1},"engine_version":"0.20.0","scenario_hash":"abc"}"#;
        let m = extract("araim", json).expect("valid JSON must extract");

        assert_eq!(m.len(), 2);
        assert_eq!(m.get("araim::fom.raim_availability"), Some(0.82));
        assert_eq!(m.get("araim::fom.mean_hpl_m"), Some(22.1));

        // Bookkeeping numbers are skipped.
        assert_eq!(m.get("araim::seed"), None);
        assert_eq!(m.get("araim::schema_version"), None);
        // String fields never appear.
        assert_eq!(m.get("araim::engine_version"), None);
        assert_eq!(m.get("araim::scenario_hash"), None);
    }

    #[test]
    fn extracts_nested_objects() {
        let json = r#"{"quantum":{"holdover_s":3600.0},"classical":{"holdover_s":1800.0}}"#;
        let m = extract("clock", json).expect("valid JSON must extract");

        assert_eq!(m.len(), 2);
        assert_eq!(m.get("clock::quantum.holdover_s"), Some(3600.0));
        assert_eq!(m.get("clock::classical.holdover_s"), Some(1800.0));
    }

    #[test]
    fn extracts_array_elements_by_index() {
        let json = r#"{"vals":[1.0, 2.0]}"#;
        let m = extract("arr", json).expect("valid JSON must extract");

        assert_eq!(m.len(), 2);
        assert_eq!(m.get("arr::vals.0"), Some(1.0));
        assert_eq!(m.get("arr::vals.1"), Some(2.0));
    }

    #[test]
    fn short_arrays_index_but_long_arrays_are_skipped() {
        // A SHORT array (<= 8 numbers) is still flattened to indexed keys: these
        // are legitimate FoM vectors.
        let short = r#"{"v":[1.0,2.0,3.0]}"#;
        let ms = extract("label", short).expect("valid JSON must extract");
        assert_eq!(ms.get("label::v.0"), Some(1.0));
        assert_eq!(ms.get("label::v.1"), Some(2.0));
        assert_eq!(ms.get("label::v.2"), Some(3.0));
        assert_eq!(ms.len(), 3);

        // A LONG array (> 8 numbers, e.g. a per-epoch raw-measurement table) is
        // NOT a figure of merit: its leaves are skipped entirely.
        let long = r#"{"v":[1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0,10.0,11.0,12.0]}"#;
        let ml = extract("label", long).expect("valid JSON must extract");
        for idx in 0..12 {
            assert_eq!(ml.get(&format!("label::v.{idx}")), None);
        }
        assert_eq!(ml.len(), 0);
    }

    #[test]
    fn merge_overwrites_get_returns_and_extraction_is_deterministic() {
        // merge: other wins on collision.
        let mut a = MetricSet::default();
        a.0.insert("k::x".to_string(), 1.0);
        a.0.insert("k::y".to_string(), 2.0);
        let mut b = MetricSet::default();
        b.0.insert("k::x".to_string(), 9.0);
        b.0.insert("k::z".to_string(), 3.0);
        a.merge(b);
        assert_eq!(a.get("k::x"), Some(9.0)); // other won
        assert_eq!(a.get("k::y"), Some(2.0));
        assert_eq!(a.get("k::z"), Some(3.0));
        assert_eq!(a.len(), 3);
        assert_eq!(a.get("k::missing"), None);

        // determinism: same JSON twice yields equal MetricSets.
        let json = r#"{"fom":{"a":0.1,"b":0.2,"c":0.3}}"#;
        let m1 = extract("d", json).unwrap();
        let m2 = extract("d", json).unwrap();
        assert_eq!(m1, m2);
    }

    #[test]
    fn invalid_json_is_err() {
        assert!(extract("x", "{not valid").is_err());
        assert!(extract("x", "").is_err());
    }
}
