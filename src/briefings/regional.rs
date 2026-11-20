// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Versioned regional exposure datasets (B0903)
//!
//! From the open engine's daily interference-map layers, a dataset per named region (built-in:
//! `india-nw-border`, `delhi-tma`, `korea-west`, `korea-dmz`, `baltic`, `gulf`, `caribbean`,
//! and the others of `briefing areas`; or the user's): for every cell of the region, its state
//! on every layer day (degraded, not degraded, unassessed, not observed), its counts and a
//! pattern class (`persistent`, `recurrent`, `episodic`, `none`); per day, the counts of
//! observed and degraded cells; and **pattern separation**: each day's degraded cells grouped
//! into touching clusters (the open engine's `observatory::cluster`) and linked from day to day
//! by shared cells into tracked patterns, so that spatially separate sources in one region are
//! reported apart. Optionally, per airport, the patterns and cells within a radius: a pattern
//! library for a civil aviation authority.
//!
//! **Byte-identical rebuild.** The dataset carries no generation time; inputs are ordered and
//! named as the request names them; floats are rounded to 9 significant digits; maps are
//! ordered. The same request and the same input bytes give the same files, and
//! `MANIFEST.sha256` lists their hashes. Real inputs are built with the open engine (see
//! `scripts/exposure/build-regional-inputs.sh`); only data whose licence allows redistribution
//! may be shipped with a dataset.
//!
//! MODELLED: every derived figure is a count or share of the layers' cell states. Overlapping
//! sources that degrade the same cells on the same days cannot be separated from cell states;
//! they stay one pattern.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::{haversine_m, CellId};
use kshana::interference_map::observatory::cluster;
use serde::{Deserialize, Serialize};

use super::layers::{state_str, CellState, Layer};
use super::{round, sig9, InputRef};

pub const FORMAT: &str = "kshana-pro-regional-exposure-dataset/1";
pub const RESULT_FILE: &str = "dataset.json";

pub const METHOD: &str = "Per region: cells whose centre lies in the region; per cell and \
layer day the state (degraded, not_degraded, unassessed, not_observed). Pattern class over \
observed days: persistent when degraded on at least persistent_share of them, recurrent at \
least recurrent_share, episodic when ever degraded, none otherwise. Patterns: each day's \
degraded cells grouped into clusters of touching cells (including diagonally); a cluster joins \
the pattern it shares most cells with among patterns active within link_gap_days, otherwise \
starts a new one. Airports: patterns with a cell centre within radius_km.";

/// An airport for the pattern library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Airport {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    #[serde(default = "hundred")]
    pub radius_km: f64,
}

fn hundred() -> f64 {
    100.0
}

/// The request (`regional.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionalRequest {
    pub dataset: String,
    pub version: String,
    pub regions: Vec<String>,
    #[serde(default)]
    pub area_file: Option<String>,
    pub layers: Vec<String>,
    /// Only layers of this source kind (`adsb` or `ais`); they are never mixed.
    pub source_kind: String,
    #[serde(default = "p60")]
    pub persistent_share: f64,
    #[serde(default = "p20")]
    pub recurrent_share: f64,
    #[serde(default = "three")]
    pub link_gap_days: u32,
    #[serde(default)]
    pub airports: Vec<Airport>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn p60() -> f64 {
    0.6
}
fn p20() -> f64 {
    0.2
}
fn three() -> u32 {
    3
}

impl RegionalRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// One cell's series.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellSeries {
    pub cell_i: i32,
    pub cell_j: i32,
    /// `[south, west, north, east]`.
    pub bounds: [f64; 4],
    /// State per layer day, in the order of `dates`.
    pub states: Vec<String>,
    pub observed_days: usize,
    pub degraded_days: usize,
    pub pattern_class: String,
}

/// One day's counts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DayCount {
    pub date: String,
    pub observed_cells: usize,
    pub degraded_cells: usize,
    pub clusters: usize,
}

/// A tracked pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: String,
    pub first_date: String,
    pub last_date: String,
    pub active_days: usize,
    /// Every cell the pattern ever covered, `[i, j]`.
    pub cells: Vec<[i32; 2]>,
    pub max_cells_one_day: usize,
}

/// One region's dataset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionData {
    pub region_id: String,
    pub name: String,
    pub boundary_source: String,
    pub cells: Vec<CellSeries>,
    pub days: Vec<DayCount>,
    pub patterns: Vec<Pattern>,
    pub class_counts: BTreeMap<String, usize>,
}

/// One airport's library entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AirportEntry {
    pub airport_id: String,
    pub name: String,
    pub radius_km: f64,
    /// `region_id/pattern_id`.
    pub patterns: Vec<String>,
    pub cells_within: usize,
    pub persistent_cells_within: usize,
    pub days_with_degraded_within: usize,
}

/// The dataset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dataset {
    pub format: String,
    pub dataset: String,
    pub version: String,
    pub evidence: String,
    pub method: String,
    pub source_kind: String,
    pub cell_deg: f64,
    pub dates: Vec<String>,
    pub regions: Vec<RegionData>,
    pub airports: Vec<AirportEntry>,
    pub inputs: Vec<InputRef>,
    pub data_licences: Vec<String>,
    pub limits: Vec<String>,
}

fn class(obs: usize, deg: usize, r: &RegionalRequest) -> &'static str {
    if obs == 0 || deg == 0 {
        return "none";
    }
    let s = deg as f64 / obs as f64;
    if s >= r.persistent_share {
        "persistent"
    } else if s >= r.recurrent_share {
        "recurrent"
    } else {
        "episodic"
    }
}

/// Track patterns across days from each day's degraded cells (sorted by date).
pub fn track_patterns(per_day: &[(String, Vec<CellId>)], link_gap_days: u32) -> Vec<Pattern> {
    struct P {
        first: usize,
        last: usize,
        days: BTreeSet<usize>,
        cells: BTreeSet<(i32, i32)>,
        recent: BTreeSet<(i32, i32)>,
        max: usize,
    }
    let mut ps: Vec<P> = vec![];
    let day_no = |d: &str| -> i64 {
        crate::forensics::inputs::parse_utc(&format!("{d}T00:00:00Z"))
            .map(|t| (t / 86_400.0).round() as i64)
            .unwrap_or(0)
    };
    let nums: Vec<i64> = per_day.iter().map(|(d, _)| day_no(d)).collect();
    for (k, (_, cells)) in per_day.iter().enumerate() {
        let mut taken: BTreeSet<usize> = BTreeSet::new();
        for cl in cluster(cells) {
            let set: BTreeSet<(i32, i32)> = cl.iter().map(|c| (c.i, c.j)).collect();
            let best = ps
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    !taken.contains(i) && nums[k] - nums[p.last] <= link_gap_days as i64
                })
                .map(|(i, p)| (i, p.recent.intersection(&set).count()))
                .filter(|(_, n)| *n > 0)
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)));
            match best {
                Some((i, _)) => {
                    let p = &mut ps[i];
                    p.last = k;
                    p.days.insert(k);
                    p.cells.extend(set.iter().copied());
                    p.recent = set.clone();
                    p.max = p.max.max(set.len());
                    taken.insert(i);
                }
                None => {
                    ps.push(P {
                        first: k,
                        last: k,
                        days: [k].into(),
                        cells: set.clone(),
                        recent: set.clone(),
                        max: set.len(),
                    });
                    taken.insert(ps.len() - 1);
                }
            }
        }
    }
    ps.iter()
        .enumerate()
        .map(|(i, p)| Pattern {
            id: format!("P{:03}", i + 1),
            first_date: per_day[p.first].0.clone(),
            last_date: per_day[p.last].0.clone(),
            active_days: p.days.len(),
            cells: p.cells.iter().map(|c| [c.0, c.1]).collect(),
            max_cells_one_day: p.max,
        })
        .collect()
}

/// Build the dataset.
pub fn build(r: &RegionalRequest) -> Result<Dataset, String> {
    if r.version.trim().is_empty() || r.dataset.trim().is_empty() {
        return Err("dataset and version are required".into());
    }
    if !matches!(r.source_kind.as_str(), "adsb" | "ais") {
        return Err("source_kind must be adsb or ais".into());
    }
    if !(0.0..=1.0).contains(&r.recurrent_share) || r.persistent_share < r.recurrent_share {
        return Err("shares must satisfy 0 <= recurrent_share <= persistent_share <= 1".into());
    }
    let mut inputs = vec![];
    let all = super::layers::load_layers(&r.base_dir, &r.layers, &mut inputs)?;
    let layers: Vec<&Layer> = all
        .iter()
        .filter(|l| l.source_kind == r.source_kind)
        .collect();
    if layers.is_empty() {
        return Err(format!("no {} layer among the inputs", r.source_kind));
    }
    let cell_deg = layers[0].cell_deg;
    if layers.iter().any(|l| (l.cell_deg - cell_deg).abs() > 1e-12) {
        return Err("all layers must share one cell size".into());
    }
    let mut dates: Vec<String> = layers.iter().map(|l| l.date.clone()).collect();
    dates.dedup();
    if dates.len() != layers.len() {
        return Err("two layers of the same source kind share a date; give one per day".into());
    }
    let cat = super::transit::load_catalogue(&r.base_dir, &r.area_file, false, &mut inputs)?;
    let regions = super::areas::select(&cat, &r.regions)?;
    let grid = layers[0].grid();
    let mut out = vec![];
    for a in &regions {
        let cells = a.cells(&grid);
        let mut series = vec![];
        let mut cls: BTreeMap<String, usize> = BTreeMap::new();
        for c in &cells {
            let states: Vec<Option<CellState>> =
                layers.iter().map(|l| l.cells.get(c).copied()).collect();
            let obs = states
                .iter()
                .filter(|s| matches!(s, Some(CellState::Degraded | CellState::NotDegraded)))
                .count();
            let deg = states
                .iter()
                .filter(|s| **s == Some(CellState::Degraded))
                .count();
            let k = class(obs, deg, r);
            *cls.entry(k.into()).or_default() += 1;
            let (s, w, n, e) = grid.bounds(*c);
            series.push(CellSeries {
                cell_i: c.i,
                cell_j: c.j,
                bounds: [sig9(s), sig9(w), sig9(n), sig9(e)],
                states: states
                    .into_iter()
                    .map(|s| state_str(s).to_string())
                    .collect(),
                observed_days: obs,
                degraded_days: deg,
                pattern_class: k.into(),
            });
        }
        let set: BTreeSet<CellId> = cells.iter().copied().collect();
        let per_day: Vec<(String, Vec<CellId>)> = layers
            .iter()
            .map(|l| {
                (
                    l.date.clone(),
                    l.degraded()
                        .into_iter()
                        .filter(|c| set.contains(c))
                        .collect(),
                )
            })
            .collect();
        let days = layers
            .iter()
            .zip(&per_day)
            .map(|(l, (d, deg))| DayCount {
                date: d.clone(),
                observed_cells: cells
                    .iter()
                    .filter(|c| {
                        matches!(
                            l.cells.get(c),
                            Some(CellState::Degraded | CellState::NotDegraded)
                        )
                    })
                    .count(),
                degraded_cells: deg.len(),
                clusters: cluster(deg).len(),
            })
            .collect();
        out.push(RegionData {
            region_id: a.id.clone(),
            name: a.name.clone(),
            boundary_source: a.source.clone(),
            cells: series,
            days,
            patterns: track_patterns(&per_day, r.link_gap_days),
            class_counts: cls,
        });
    }
    let mut airports = vec![];
    for ap in &r.airports {
        let within = |i: i32, j: i32| {
            let (la, lo) = grid.center(CellId { i, j });
            haversine_m(ap.lat, ap.lon, la, lo) <= ap.radius_km * 1000.0
        };
        let mut pats = vec![];
        let mut cells_within: BTreeSet<(i32, i32)> = BTreeSet::new();
        let mut persistent = 0;
        let mut days_deg: BTreeSet<usize> = BTreeSet::new();
        for rg in &out {
            for p in &rg.patterns {
                if p.cells.iter().any(|c| within(c[0], c[1])) {
                    pats.push(format!("{}/{}", rg.region_id, p.id));
                }
            }
            for c in &rg.cells {
                if within(c.cell_i, c.cell_j) && cells_within.insert((c.cell_i, c.cell_j)) {
                    if c.pattern_class == "persistent" {
                        persistent += 1;
                    }
                    for (k, s) in c.states.iter().enumerate() {
                        if s == "degraded" {
                            days_deg.insert(k);
                        }
                    }
                }
            }
        }
        airports.push(AirportEntry {
            airport_id: ap.id.clone(),
            name: ap.name.clone(),
            radius_km: sig9(ap.radius_km),
            patterns: pats,
            cells_within: cells_within.len(),
            persistent_cells_within: persistent,
            days_with_degraded_within: days_deg.len(),
        });
    }
    // Inputs in request order; their content hashes make the dataset traceable.
    Ok(Dataset {
        format: FORMAT.into(),
        dataset: r.dataset.clone(),
        version: r.version.clone(),
        evidence: super::EVIDENCE.into(),
        method: METHOD.into(),
        source_kind: r.source_kind.clone(),
        cell_deg: sig9(cell_deg),
        dates,
        regions: out,
        airports,
        inputs,
        data_licences: super::brief::licences(&layers),
        limits: vec![
            "A degraded cell does not identify interference as the cause; a not-observed cell is not evidence of a clear cell.".into(),
            "Overlapping sources that degrade the same cells on the same days stay one pattern: cell states cannot separate them.".into(),
            "Regions are the boxes named, not legal or administrative boundaries.".into(),
            format!("Cells are {} degree: no figure locates anything more finely.", round(cell_deg, 3)),
        ],
    })
}

/// The method note shipped with the dataset.
pub fn method_note(d: &Dataset) -> String {
    let mut s = format!(
        "{} {}\n\nFormat: {}\nEvidence: {}\n\nMethod\n------\n{}\n\nSource kind: {}; cell size {} degrees; {} layer day(s) from {} to {}.\n\nLimits\n------\n",
        d.dataset,
        d.version,
        d.format,
        d.evidence,
        d.method,
        d.source_kind,
        d.cell_deg,
        d.dates.len(),
        d.dates.first().map(String::as_str).unwrap_or("-"),
        d.dates.last().map(String::as_str).unwrap_or("-")
    );
    for l in &d.limits {
        s.push_str(&format!("- {l}\n"));
    }
    s.push_str("\nInputs (SHA-256)\n----------------\n");
    for i in &d.inputs {
        s.push_str(&format!("{}  {} ({})\n", i.sha256, i.file, i.role));
    }
    s.push_str("\nData licences\n-------------\n");
    for l in &d.data_licences {
        s.push_str(&format!("- {l}\n"));
    }
    s
}

/// The dataset's files, in a fixed order: `(name, bytes)`.
pub fn files(d: &Dataset, request_src: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut v = vec![(
        RESULT_FILE.to_string(),
        (serde_json::to_string_pretty(d).map_err(|e| e.to_string())? + "\n").into_bytes(),
    )];
    let mut csv = String::from("region_id,cell_i,cell_j,date,state\n");
    for rg in &d.regions {
        for c in &rg.cells {
            for (k, s) in c.states.iter().enumerate() {
                csv.push_str(&format!(
                    "{},{},{},{},{}\n",
                    rg.region_id, c.cell_i, c.cell_j, d.dates[k], s
                ));
            }
        }
    }
    v.push(("cells.csv".into(), csv.into_bytes()));
    v.push(("METHOD.txt".into(), method_note(d).into_bytes()));
    v.push(("request.toml".into(), request_src.as_bytes().to_vec()));
    let manifest: String = v
        .iter()
        .map(|(n, b)| format!("{}  {n}\n", crate::forensics::inputs::sha256_hex(b)))
        .collect();
    v.push(("MANIFEST.sha256".into(), manifest.into_bytes()));
    Ok(v)
}

/// Write the dataset directory.
pub fn write_dir(out: &Path, d: &Dataset, request_src: &str) -> Result<(), String> {
    for (n, b) in files(d, request_src)? {
        super::write_text(out, &n, &b)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_clusters_become_separate_patterns_and_link_across_days() {
        let c = |i, j| CellId { i, j };
        let per_day = vec![
            (
                "2026-06-01".to_string(),
                vec![c(10, 10), c(10, 11), c(20, 20)],
            ),
            ("2026-06-02".to_string(), vec![c(10, 11), c(10, 12)]),
            ("2026-06-03".to_string(), vec![]),
            ("2026-06-09".to_string(), vec![c(10, 12)]),
        ];
        let p = track_patterns(&per_day, 3);
        assert_eq!(p.len(), 3, "{p:?}");
        assert_eq!(p[0].active_days, 2);
        assert_eq!(p[0].cells.len(), 3);
        assert_eq!(p[1].active_days, 1);
        // Seven days later is beyond the link gap: a new pattern.
        assert_eq!(p[2].first_date, "2026-06-09");
    }
}
