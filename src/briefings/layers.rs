// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Interference-map layers: the open engine's per-day maps (`kshana-interference-map/v1`,
//! written by `kshana interference-map adsb|ais|daily`), read with the open engine's own
//! reader (`kshana::interference_map::route::load_map`) and indexed by cell.
//!
//! A cell missing from a map was not observed by enough aircraft or vessels: that is never
//! read as a clear cell.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kshana::interference_map::grid::{CellId, Grid};
use kshana::interference_map::route::{load_map, LoadedMap};

use super::{read_input, InputRef};

/// The state of one cell on one day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CellState {
    Degraded,
    NotDegraded,
    /// Present in the map without a call (insufficient sample, withheld, confounded day).
    Unassessed,
}

impl CellState {
    pub fn as_str(self) -> &'static str {
        match self {
            CellState::Degraded => "degraded",
            CellState::NotDegraded => "not_degraded",
            CellState::Unassessed => "unassessed",
        }
    }
}

/// The state string of an optional cell (`not_observed` when absent).
pub fn state_str(s: Option<CellState>) -> &'static str {
    s.map(CellState::as_str).unwrap_or("not_observed")
}

/// One day's map.
pub struct Layer {
    pub date: String,
    /// Unix seconds of the day's start (00:00 UTC).
    pub day_start: f64,
    pub source_kind: String,
    pub cell_deg: f64,
    pub licence: String,
    pub attribution: String,
    pub cells: BTreeMap<CellId, CellState>,
    pub input: InputRef,
    /// The open engine's loaded map (for `route::exposure`).
    pub map: LoadedMap,
}

impl Layer {
    pub fn grid(&self) -> Grid {
        self.map.grid
    }

    /// The state of the cell containing `(lat, lon)`, `None` when not observed.
    pub fn state_at(&self, lat: f64, lon: f64) -> Option<CellState> {
        self.cells.get(&self.map.grid.cell_of(lat, lon)).copied()
    }

    /// Whether Unix time `t` falls on this layer's day.
    pub fn covers(&self, t: f64) -> bool {
        t >= self.day_start && t < self.day_start + 86_400.0
    }

    /// Degraded cells.
    pub fn degraded(&self) -> Vec<CellId> {
        self.cells
            .iter()
            .filter(|(_, s)| **s == CellState::Degraded)
            .map(|(c, _)| *c)
            .collect()
    }
}

/// Parse one map document.
pub fn parse_layer(bytes: &[u8], input: InputRef) -> Result<Layer, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("{}: {e}", input.file))?;
    let map = load_map(text).map_err(|e| format!("{}: {e}", input.file))?;
    if map.grid.cell_deg < super::MIN_CELL_DEG - 1e-9 {
        return Err(format!(
            "{}: cell size {} degrees is finer than the {} degree granularity these outputs keep",
            input.file,
            map.grid.cell_deg,
            super::MIN_CELL_DEG
        ));
    }
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("{}: {e}", input.file))?;
    let mut cells = BTreeMap::new();
    for f in v["features"].as_array().into_iter().flatten() {
        let p = &f["properties"];
        let (Some(i), Some(j)) = (p["cell_i"].as_i64(), p["cell_j"].as_i64()) else {
            continue;
        };
        // The same classes as the open engine's route reader.
        let s = match p["status"].as_str().unwrap_or("") {
            "degraded" | "anomalous" => CellState::Degraded,
            "not_degraded" | "not_anomalous" => CellState::NotDegraded,
            _ => CellState::Unassessed,
        };
        cells.insert(
            CellId {
                i: i as i32,
                j: j as i32,
            },
            s,
        );
    }
    let day_start = super::utc("map date", &format!("{}T00:00:00Z", map.date))?;
    Ok(Layer {
        date: map.date.clone(),
        day_start,
        source_kind: map.source_kind.clone(),
        cell_deg: map.grid.cell_deg,
        licence: map.licence.clone(),
        attribution: map.attribution.clone(),
        cells,
        input,
        map,
    })
}

/// Load layers (files or directories of `.geojson`), sorted by date then file name.
pub fn load_layers(
    base: &Option<PathBuf>,
    named: &[String],
    inputs: &mut Vec<InputRef>,
) -> Result<Vec<Layer>, String> {
    let mut out = vec![];
    for n in super::expand(base, named, ".geojson")? {
        let (b, r) = read_input(base, &n, "layer")?;
        inputs.push(r.clone());
        out.push(parse_layer(&b, r)?);
    }
    out.sort_by(|a, b| a.date.cmp(&b.date).then(a.input.file.cmp(&b.input.file)));
    Ok(out)
}

/// The combined state at `(lat, lon)` at time `t` over every layer of that day: degraded if
/// any says so, else not degraded if any observed it, else unassessed if any lists it.
pub fn state_at_time(layers: &[Layer], t: f64, lat: f64, lon: f64) -> Option<CellState> {
    layers
        .iter()
        .filter(|l| l.covers(t))
        .filter_map(|l| l.state_at(lat, lon))
        .min()
}

/// A layer document (`kshana-interference-map/v1`) from cell states, in the open engine's
/// shape. Used to write the synthetic fixtures and test layers; real layers come from
/// `kshana interference-map`.
pub fn synthetic_layer(
    date: &str,
    source_kind: &str,
    cell_deg: f64,
    cells: &[(CellId, &str)],
) -> serde_json::Value {
    let g = Grid { cell_deg };
    let features: Vec<serde_json::Value> = cells
        .iter()
        .map(|(c, st)| {
            let (s, w, n, e) = g.bounds(*c);
            serde_json::json!({
                "type": "Feature",
                "properties": {"cell_i": c.i, "cell_j": c.j, "status": st, "degraded": *st == "degraded"},
                "geometry": {"type": "Polygon", "coordinates": [[[w, s], [e, s], [e, n], [w, n], [w, s]]]}
            })
        })
        .collect();
    serde_json::json!({
        "type": "FeatureCollection",
        "kshana_interference_map": {
            "schema": "kshana-interference-map/v1",
            "format_version": 1,
            "source_kind": source_kind,
            "date": date,
            "grid": {"type": "fixed_lat_lon", "cell_deg": cell_deg},
            "data": {"licence": "SYNTHETIC test data", "attribution": "Kshana Pro synthetic fixture"},
            "notice": "SYNTHETIC: not observations."
        },
        "features": features
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_reads_through_the_open_reader() {
        let g = Grid { cell_deg: 0.5 };
        let c = g.cell_of(26.2, 56.2);
        let doc = synthetic_layer(
            "2026-06-01",
            "ais",
            0.5,
            &[
                (c, "anomalous"),
                (CellId { i: c.i, j: c.j + 1 }, "not_anomalous"),
            ],
        );
        let b = doc.to_string().into_bytes();
        let r = InputRef {
            role: "layer".into(),
            file: "x.geojson".into(),
            sha256: String::new(),
            bytes: b.len(),
        };
        let l = parse_layer(&b, r).unwrap();
        assert_eq!(l.state_at(26.2, 56.2), Some(CellState::Degraded));
        assert_eq!(l.state_at(26.2, 56.7), Some(CellState::NotDegraded));
        assert_eq!(l.state_at(20.0, 56.7), None);
        assert!(l.covers(l.day_start + 3600.0));
        let fine = synthetic_layer("2026-06-01", "ais", 0.1, &[]).to_string();
        let r2 = InputRef {
            role: "layer".into(),
            file: "f".into(),
            sha256: String::new(),
            bytes: 0,
        };
        assert!(parse_layer(fine.as_bytes(), r2).is_err());
    }
}
