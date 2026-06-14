// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! Small shared helpers used across Pro packs: JSON metric extraction, scenario
//! seed injection, and percentiles. Kept dependency-light and pure.

use kshana::api::KshanaError;

/// Walk a dot-path (`a.b.c`) into a JSON value, returning a finite number if present.
/// A numeric path segment indexes into an array (`samples.0.alt_km`).
pub(crate) fn dot_path_number(root: &serde_json::Value, path: &str) -> Option<f64> {
    let mut cur = root;
    for key in path.split('.') {
        cur = match key.parse::<usize>() {
            Ok(idx) => cur.get(idx)?,
            Err(_) => cur.get(key)?,
        };
    }
    cur.as_f64().filter(|v| v.is_finite())
}

/// Set the top-level `seed` field of a scenario TOML to `seed`, returning the
/// re-serialised TOML. Used to fan a single scenario across many Monte-Carlo seeds
/// without the caller hand-editing TOML. (Mirrors the open core's `inject_eop`.)
pub(crate) fn inject_seed(src: &str, seed: u64) -> Result<String, KshanaError> {
    let mut value: toml::Value = toml::from_str(src)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid scenario TOML: {e}")))?;
    let table = value
        .as_table_mut()
        .ok_or_else(|| KshanaError::InvalidInput("scenario TOML is not a table".into()))?;
    table.insert("seed".to_string(), toml::Value::Integer(seed as i64));
    toml::to_string(&value)
        .map_err(|e| KshanaError::InvalidInput(format!("could not re-serialise scenario: {e}")))
}

/// Nearest-rank percentile of a slice (0.0..=1.0). Returns `None` for an empty
/// slice. The input need not be sorted; a sorted copy is made.
pub(crate) fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p = p.clamp(0.0, 1.0);
    // Nearest-rank: rank = ceil(p * n), 1-based, clamped into range.
    let rank = (p * v.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(v.len() - 1);
    Some(v[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_path_reads_nested_numbers() {
        let j = serde_json::json!({"a": {"b": {"c": 3.5}}, "x": "str"});
        assert_eq!(dot_path_number(&j, "a.b.c"), Some(3.5));
        assert_eq!(dot_path_number(&j, "a.b"), None); // object, not a number
        assert_eq!(dot_path_number(&j, "x"), None); // string
        assert_eq!(dot_path_number(&j, "nope"), None);
    }

    #[test]
    fn inject_seed_sets_top_level_seed() {
        let out = inject_seed("seed = 1\nthreshold_ns = 20.0\n", 99).unwrap();
        let v: toml::Value = toml::from_str(&out).unwrap();
        assert_eq!(v.get("seed").unwrap().as_integer(), Some(99));
        // Other fields are preserved.
        assert!((v.get("threshold_ns").unwrap().as_float().unwrap() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn inject_seed_adds_seed_when_absent() {
        let out = inject_seed("threshold_ns = 20.0\n", 7).unwrap();
        let v: toml::Value = toml::from_str(&out).unwrap();
        assert_eq!(v.get("seed").unwrap().as_integer(), Some(7));
    }

    #[test]
    fn percentile_nearest_rank() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&v, 1.0), Some(10.0));
        assert_eq!(percentile(&v, 0.5), Some(5.0));
        assert_eq!(percentile(&[], 0.95), None);
        // Unsorted input handled.
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 1.0), Some(3.0));
    }
}
