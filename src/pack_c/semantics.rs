// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Which leaves of a result are figures.**
//!
//! The legacy extractor ([`crate::pack_c::metrics::extract`]) collects every numeric
//! leaf, so a spectrum run contributes its receiver noise figure and its waterfall grid
//! size next to the carrier-to-noise density ratios it computed. A watched pack uses
//! this module instead: a leaf is a figure only when the result's own `units` block
//! says the run produced it.
//!
//! **The rule.** A numeric leaf is a figure when its entry in the result's `units`
//! block (looked up with the open engine's own path grammar,
//! [`kshana::field_schema::lookup`]) has provenance `computed`, `modelled` or `derived`:
//! the engine's classes for a number the run itself produced. `modelled` is included
//! because the engine uses it for the output of a documented model (the low-Earth-orbit
//! chain's signal-in-space range error and its fused position error carry it). Excluded
//! are what went in or a fixed reference (`input`, `modelled-input`, `spec`,
//! `published`, `constant`, `measured`, `measured-or-input`) and algebraic restatements
//! of inputs and self-checks (`closed-form`, `internal-consistency`).
//!
//! A computed leaf that only describes the layout of the output (a grid size, a bin
//! width, a sample count of an exported file, the campaign's run bookkeeping) is not a
//! figure either; [`is_structural`] names those, and the list is stated in
//! `docs/CAMPAIGN-WATCH.md`.
//!
//! Each figure carries the unit its `units` entry states. The direction of a figure
//! comes from the manifest or the Pack C name table, never from the unit. A result with
//! no `units` block at all falls back to every numeric leaf, with no unit, and the report
//! says so.
//!
//! Arrays are flattened exactly as the legacy extractor flattens them (indexed, and
//! skipped when longer than 8): a long series is read through a watch aggregate or a
//! campaign figure, not as hundreds of metrics.

use crate::pack_c::metrics::MetricSet;
use serde_json::Value;
use std::collections::BTreeMap;

/// Provenance classes whose numbers the run produced.
pub const FIGURE_PROVENANCE: [&str; 3] = ["computed", "modelled", "derived"];

/// The same array cap as the legacy extractor.
const MAX_ARRAY: usize = 8;

/// The figures of one result, with the unit of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Figures {
    /// `"{label}::{dotted_path}" -> value`.
    pub metrics: MetricSet,
    /// `"{label}::{dotted_path}" -> unit` for every figure whose unit is stated.
    pub units: BTreeMap<String, String>,
    /// True when the result had no `units` block and every numeric leaf was taken.
    pub fallback_all_leaves: bool,
}

/// Whether a computed leaf only describes the output's own layout (see the module docs).
pub fn is_structural(dotted: &str) -> bool {
    let segs: Vec<&str> = dotted.split('.').collect();
    let leaf = segs
        .iter()
        .rev()
        .find(|s| !s.chars().all(|c| c.is_ascii_digit()))
        .copied()
        .unwrap_or("");
    if segs.first() == Some(&"reproducibility") {
        return true;
    }
    if segs
        .iter()
        .any(|s| matches!(*s, "sigmf" | "coords" | "axes" | "shape") || s.contains(':'))
    {
        return true;
    }
    leaf.starts_with("n_")
        || leaf.starts_with("source_n_")
        || matches!(
            leaf,
            "nfft" | "bin_width_hz" | "row_duration_s" | "bins_compared" | "segments"
        )
}

/// Extract the figures of `result_json` under `label`.
pub fn figures(label: &str, result_json: &str) -> Result<Figures, String> {
    let doc: Value =
        serde_json::from_str(result_json).map_err(|e| format!("invalid result JSON: {e}"))?;
    Ok(figures_of(label, &doc))
}

/// [`figures`] over an already-parsed document.
pub fn figures_of(label: &str, doc: &Value) -> Figures {
    let units = doc.get("units").and_then(Value::as_object);
    let mut leaves: Vec<(String, f64)> = Vec::new();
    if let Value::Object(map) = doc {
        for (k, v) in map {
            if k == "schema_version" || k == "seed" || k == "units" {
                continue;
            }
            walk(k, v, &mut leaves);
        }
    }
    let mut out = Figures {
        fallback_all_leaves: units.is_none(),
        ..Figures::default()
    };
    for (path, value) in leaves {
        let key = format!("{label}::{path}");
        match units {
            None => {
                out.metrics.0.insert(key, value);
            }
            Some(u) => {
                let canon = crate::pack_c::watch::canonical(&path);
                let Some((_, entry)) = kshana::field_schema::lookup(u, &canon) else {
                    continue;
                };
                let prov = entry
                    .get("provenance")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !FIGURE_PROVENANCE.contains(&prov) || is_structural(&path) {
                    continue;
                }
                if let Some(unit) = entry.get("unit").and_then(Value::as_str) {
                    out.units.insert(key.clone(), unit.to_string());
                }
                out.metrics.0.insert(key, value);
            }
        }
    }
    out
}

fn walk(path: &str, v: &Value, out: &mut Vec<(String, f64)>) {
    match v {
        Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                out.push((path.to_string(), f));
            }
        }
        Value::Object(m) => {
            for (k, c) in m {
                walk(&format!("{path}.{k}"), c, out);
            }
        }
        Value::Array(a) => {
            if a.len() > MAX_ARRAY {
                return;
            }
            for (i, c) in a.iter().enumerate() {
                walk(&format!("{path}.{i}"), c, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_produced_numbers_are_figures() {
        let doc = r#"{
            "seed": 1,
            "receiver": {"noise_figure_db": 2.0, "system_temp_k": 459.0},
            "timeline": {"bands": [{"min_cn0_dbhz": 3.2}]},
            "waterfall": {"n_freq": 144, "peak_dbw_per_hz": -161.3},
            "nav": {"sisre_rms_m": 0.25},
            "units": {
                "receiver.noise_figure_db": {"unit": "dB", "provenance": "input"},
                "receiver.system_temp_k": {"unit": "K", "provenance": "closed-form"},
                "timeline.bands[].min_cn0_dbhz": {"unit": "dB-Hz", "provenance": "computed"},
                "waterfall.n_freq": {"unit": "count", "provenance": "computed"},
                "waterfall.peak_dbw_per_hz": {"unit": "dBW/Hz", "provenance": "computed"},
                "nav.sisre_rms_m": {"unit": "m", "provenance": "modelled"}
            }
        }"#;
        let f = figures("s", doc).unwrap();
        let keys: Vec<&String> = f.metrics.keys().collect();
        assert_eq!(
            keys,
            vec![
                "s::nav.sisre_rms_m",
                "s::timeline.bands.0.min_cn0_dbhz",
                "s::waterfall.peak_dbw_per_hz"
            ]
        );
        assert_eq!(f.units["s::timeline.bands.0.min_cn0_dbhz"], "dB-Hz");
        assert!(!f.fallback_all_leaves);
    }

    #[test]
    fn a_result_without_units_falls_back_to_every_leaf() {
        let f = figures("s", r#"{"fom":{"a":1.0,"b":2.0}}"#).unwrap();
        assert_eq!(f.metrics.len(), 2);
        assert!(f.units.is_empty());
        assert!(f.fallback_all_leaves);
    }

    #[test]
    fn structural_leaves() {
        assert!(is_structural("waterfall.n_freq"));
        assert!(is_structural("iq.sigmf.data_bytes"));
        assert!(is_structural("reproducibility.runs_total"));
        assert!(is_structural("sweep.nodes.0.coords.jammer_power"));
        assert!(!is_structural("timeline.bands.0.min_cn0_dbhz"));
        assert!(!is_structural("fom.min_tracking"));
    }
}
