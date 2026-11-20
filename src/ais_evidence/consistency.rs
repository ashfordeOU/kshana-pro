// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Cross-source consistency of AIS (E07, Pro)
//!
//! Compares the AIS positions a vessel broadcasts with independent position sources the user
//! holds: radar or VTS tracks, LRIT reports, satellite AIS from another supplier, detections
//! from imagery. Each source declares its 1-sigma horizontal accuracy and, optionally, its
//! coverage polygon. Per independent report:
//!
//! * with a declared `mmsi` (the source's own association, e.g. LRIT): the AIS position of
//!   that vessel interpolated to the report time, the residual and the gate
//!   `k·√(σ_ais² + σ_src²) + SOG·τ`, where `τ` is the declared time-stamp tolerance; the
//!   report is **consistent**, **inconsistent** or **AIS absent** (no AIS for the vessel
//!   within the interpolation limit);
//! * without one: the nearest AIS vessel at that time; within the gate it is **consistent**
//!   and associated, otherwise a **target without AIS** within the gate.
//!
//! Where a source declares coverage, an AIS vessel inside it at a source scan time with no
//! independent report within the gate counts as **unconfirmed** (what a fabricated AIS
//! target or a position offset would produce; also what a missed detection produces).
//! Consecutive inconsistent reports of one vessel and source form episodes.
//!
//! **MODELLED.** Gaussian gate; no track-to-track fusion, no navigation solution: each
//! independent report is compared once with the broadcast track.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{hash_inputs, load_tracks, r, Area, InputHash, Params, Tracks, KN};
use crate::forensics::inputs::{iso, parse_utc};
use crate::transport::{dist_m, wilson95, Table};

/// Pro feature of this check.
pub const FEATURE: &str = "pro-ais-consistency";
/// Method identifier.
pub const METHOD_ID: &str = "kshana-pro/ais-cross-source/v1";

/// One independent source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub name: String,
    /// The user's label: `radar`, `vts`, `lrit`, `satellite-ais`, `imagery`, ...
    #[serde(default)]
    pub kind: String,
    /// CSV `timestamp,track_id,lat,lon[,mmsi]`.
    pub file: String,
    /// Declared 1-sigma horizontal accuracy (m).
    pub sigma_m: f64,
    /// Where the source sees targets; omitted means no unconfirmed-vessel check.
    #[serde(default)]
    pub coverage: Option<Area>,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// Decoded AIS positions CSV.
    pub ais: String,
    /// Declared 1-sigma accuracy of AIS positions (m).
    #[serde(default = "d_ais_sigma")]
    pub ais_sigma_m: f64,
    /// Gate multiplier `k`.
    #[serde(default = "d_k")]
    pub gate_k: f64,
    /// Time-stamp tolerance `τ` between sources (s).
    #[serde(default = "d_tau")]
    pub time_tolerance_s: f64,
    /// Inconsistent reports closer than this join one episode (s).
    #[serde(default = "d_join")]
    pub episode_join_s: f64,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub params: Params,
}

fn d_ais_sigma() -> f64 {
    30.0
}
fn d_k() -> f64 {
    3.0
}
fn d_tau() -> f64 {
    10.0
}
fn d_join() -> f64 {
    900.0
}

/// Outcome of one independent report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Consistent,
    Inconsistent,
    AisAbsent,
    TargetWithoutAis,
}

/// One compared report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Association {
    pub source: String,
    pub track_id: String,
    pub utc: String,
    #[serde(skip)]
    pub t: f64,
    /// The independent report's position.
    pub position: [f64; 2],
    pub mmsi: Option<String>,
    pub residual_m: Option<f64>,
    pub gate_m: f64,
    pub outcome: Outcome,
}

/// Inconsistent run of one vessel against one source.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Episode {
    pub mmsi: String,
    pub source: String,
    pub start_utc: String,
    pub end_utc: String,
    pub reports: usize,
    pub max_residual_m: f64,
}

/// Per-vessel summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VesselSummary {
    pub mmsi: String,
    pub compared: usize,
    pub consistent: usize,
    pub inconsistent: usize,
    pub ais_absent: usize,
    /// Consistent share with a Wilson 95 % interval.
    pub consistent_share: Option<f64>,
    pub consistent_share_95: Option<(f64, f64)>,
    pub max_residual_m: Option<f64>,
    /// Scan times inside a source's coverage with no independent report within the gate.
    pub unconfirmed: usize,
    pub episodes: usize,
}

/// A target an independent source saw with no AIS within the gate.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DarkTarget {
    pub source: String,
    pub track_id: String,
    pub reports: usize,
    pub first_utc: String,
    pub last_utc: String,
    pub first_position: [f64; 2],
    /// Nearest AIS vessel at the first report and its distance (m), for review.
    pub nearest_ais: Option<(String, f64)>,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub title: String,
    pub evidence: &'static str,
    pub statement: &'static str,
    pub request: Request,
    pub inputs: Vec<InputHash>,
    pub ais_rows: usize,
    pub ais_refused: usize,
    pub vessels: Vec<VesselSummary>,
    pub episodes: Vec<Episode>,
    pub dark_targets: Vec<DarkTarget>,
    pub associations: Vec<Association>,
    pub verdict: String,
    pub limits: Vec<&'static str>,
}

/// Report format tag.
pub const FORMAT: &str = "kshana-pro-ais-consistency/1";

const LIMITS: &[&str] = &[
    "The gate is Gaussian on the declared accuracies; a source with heavier tails than declared raises inconsistent counts.",
    "Association without a declared MMSI is nearest-neighbour within the gate; dense traffic can mis-associate.",
    "An unconfirmed vessel can be a missed detection by the independent source as well as a broadcast position that is not the vessel's.",
    "Areas and coverage polygons must not cross the antimeridian.",
];

struct Indep {
    t: f64,
    track: String,
    lat: f64,
    lon: f64,
    mmsi: Option<String>,
}

fn read_source(base: &Path, s: &Source) -> Result<Vec<Indep>, String> {
    let text = super::read_rel(base, &s.file)?;
    let t = Table::parse(&text).map_err(|e| format!("{}: {e}", s.file))?;
    let e = |x: String| format!("{}: {x}", s.file);
    let (ct, ctr, cla, clo) = (
        t.need("timestamp").map_err(e)?,
        t.need("track_id").map_err(e)?,
        t.need("lat").map_err(e)?,
        t.need("lon").map_err(e)?,
    );
    let cm = t.col("mmsi");
    let mut out = Vec::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts)
            .or_else(|| ts.parse().ok())
            .ok_or_else(|| format!("{}: row {}: bad timestamp '{ts}'", s.file, i + 2))?;
        let num = |c| {
            t.text(i, Some(c))
                .parse::<f64>()
                .map_err(|_| format!("{}: row {}: bad coordinate", s.file, i + 2))
        };
        out.push(Indep {
            t: tt,
            track: t.text(i, Some(ctr)).to_string(),
            lat: num(cla)?,
            lon: num(clo)?,
            mmsi: Some(t.text(i, cm).to_string()).filter(|m| !m.is_empty()),
        });
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok(out)
}

fn validate(q: &Request) -> Result<(), String> {
    q.params.validate()?;
    for (n, v) in [
        ("ais_sigma_m", q.ais_sigma_m),
        ("gate_k", q.gate_k),
        ("episode_join_s", q.episode_join_s),
    ] {
        if !(v > 0.0 && v.is_finite()) {
            return Err(format!("{n} must be positive"));
        }
    }
    if q.time_tolerance_s.is_nan() || q.time_tolerance_s < 0.0 {
        return Err("time_tolerance_s must be zero or positive".into());
    }
    if q.sources.is_empty() {
        return Err("at least one [[sources]] entry is needed".into());
    }
    let mut names = BTreeSet::new();
    for s in &q.sources {
        if !(s.sigma_m > 0.0 && s.sigma_m.is_finite()) {
            return Err(format!("source '{}': sigma_m must be positive", s.name));
        }
        if !names.insert(&s.name) {
            return Err(format!("source '{}' named twice", s.name));
        }
        if let Some(c) = &s.coverage {
            c.validate()?;
        }
    }
    Ok(())
}

/// Run the check on a request whose files are relative to `base`.
pub fn run(q: &Request, base: &Path) -> Result<Report, String> {
    validate(q)?;
    let tracks = load_tracks(base, &q.ais, None)?;
    let mut files = vec![q.ais.as_str()];
    files.extend(q.sources.iter().map(|s| s.file.as_str()));
    let inputs = hash_inputs(base, &files)?;
    let mut assoc = Vec::new();
    let mut unconfirmed: BTreeMap<String, usize> = BTreeMap::new();
    for s in &q.sources {
        let rows = read_source(base, s)?;
        compare_source(q, s, &rows, &tracks, &mut assoc, &mut unconfirmed);
    }
    assoc.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.source.cmp(&b.source)));
    let episodes = episodes(&assoc, q.episode_join_s);
    let dark = dark_targets(&assoc, &tracks, q);
    let mut vessels = Vec::new();
    for m in tracks.by_mmsi.keys() {
        let mine: Vec<&Association> = assoc
            .iter()
            .filter(|a| a.mmsi.as_deref() == Some(m.as_str()))
            .collect();
        let c = |o| mine.iter().filter(|a| a.outcome == o).count();
        let (ok, bad) = (c(Outcome::Consistent), c(Outcome::Inconsistent));
        let n = ok + bad;
        vessels.push(VesselSummary {
            mmsi: m.clone(),
            compared: mine.len(),
            consistent: ok,
            inconsistent: bad,
            ais_absent: c(Outcome::AisAbsent),
            consistent_share: (n > 0).then(|| r(ok as f64 / n as f64, 4)),
            consistent_share_95: wilson95(ok, n).map(|(a, b)| (r(a, 4), r(b, 4))),
            max_residual_m: mine
                .iter()
                .filter_map(|a| a.residual_m)
                .reduce(f64::max)
                .map(|x| r(x, 1)),
            unconfirmed: unconfirmed.get(m).copied().unwrap_or(0),
            episodes: episodes.iter().filter(|e| &e.mmsi == m).count(),
        });
    }
    let flagged: Vec<&str> = vessels
        .iter()
        .filter(|v| v.inconsistent > 0 || v.unconfirmed > 0)
        .map(|v| v.mmsi.as_str())
        .collect();
    let verdict = format!(
        "{} vessel(s) compared; {} with inconsistent or unconfirmed AIS positions{}; {} independent target(s) without AIS within the gate. For review: not a finding of manipulation.",
        vessels.iter().filter(|v| v.compared + v.unconfirmed > 0).count(),
        flagged.len(),
        if flagged.is_empty() {
            String::new()
        } else {
            format!(" ({})", flagged.join(", "))
        },
        dark.len()
    );
    Ok(Report {
        format: FORMAT,
        method: METHOD_ID,
        title: q.title.clone(),
        evidence: super::EVIDENCE,
        statement: super::STATEMENT,
        request: q.clone(),
        inputs,
        ais_rows: tracks.rows,
        ais_refused: tracks.refused,
        vessels,
        episodes,
        dark_targets: dark,
        associations: assoc,
        verdict,
        limits: LIMITS.to_vec(),
    })
}

fn compare_source(
    q: &Request,
    s: &Source,
    rows: &[Indep],
    tracks: &Tracks,
    assoc: &mut Vec<Association>,
    unconfirmed: &mut BTreeMap<String, usize>,
) {
    let base_gate = q.gate_k * (q.ais_sigma_m.powi(2) + s.sigma_m.powi(2)).sqrt();
    let gate = |sog: Option<f64>| base_gate + sog.unwrap_or(0.0) * KN * q.time_tolerance_s;
    let lim = q.params.interp_max_s;
    for x in rows {
        let a = match &x.mmsi {
            Some(m) => match tracks.at(m, x.t, lim) {
                Some((la, lo, sog)) => {
                    let d = dist_m(x.lat, x.lon, la, lo);
                    let g = gate(sog);
                    (
                        Some(m.clone()),
                        Some(d),
                        g,
                        if d <= g {
                            Outcome::Consistent
                        } else {
                            Outcome::Inconsistent
                        },
                    )
                }
                None => (Some(m.clone()), None, base_gate, Outcome::AisAbsent),
            },
            None => {
                let best = tracks
                    .by_mmsi
                    .keys()
                    .filter_map(|m| {
                        let (la, lo, sog) = tracks.at(m, x.t, lim)?;
                        Some((m, dist_m(x.lat, x.lon, la, lo), gate(sog)))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                match best {
                    Some((m, d, g)) if d <= g => (Some(m.clone()), Some(d), g, Outcome::Consistent),
                    Some((_, d, g)) => (None, Some(d), g, Outcome::TargetWithoutAis),
                    None => (None, None, base_gate, Outcome::TargetWithoutAis),
                }
            }
        };
        assoc.push(Association {
            source: s.name.clone(),
            track_id: x.track.clone(),
            utc: iso(x.t),
            t: x.t,
            position: [x.lat, x.lon],
            mmsi: a.0,
            residual_m: a.1.map(|d| r(d, 1)),
            gate_m: r(a.2, 1),
            outcome: a.3,
        });
    }
    // Unconfirmed: at each scan time, AIS vessels inside coverage with no report within gate.
    let Some(cov) = &s.coverage else { return };
    let scans: BTreeSet<i64> = rows.iter().map(|x| x.t.round() as i64).collect();
    for &ts in &scans {
        let t = ts as f64;
        let here: Vec<&Indep> = rows
            .iter()
            .filter(|x| (x.t - t).abs() <= q.time_tolerance_s.max(0.5))
            .collect();
        for m in tracks.by_mmsi.keys() {
            let Some((la, lo, sog)) = tracks.at(m, t, lim) else {
                continue;
            };
            if !cov.contains(la, lo) {
                continue;
            }
            let g = gate(sog);
            if !here.iter().any(|x| dist_m(x.lat, x.lon, la, lo) <= g) {
                *unconfirmed.entry(m.clone()).or_insert(0) += 1;
            }
        }
    }
}

fn episodes(assoc: &[Association], join_s: f64) -> Vec<Episode> {
    let mut by: BTreeMap<(String, String), Vec<&Association>> = BTreeMap::new();
    for a in assoc.iter().filter(|a| a.outcome == Outcome::Inconsistent) {
        if let Some(m) = &a.mmsi {
            by.entry((m.clone(), a.source.clone())).or_default().push(a);
        }
    }
    let mut out = Vec::new();
    for ((m, s), v) in by {
        let mut cur: Vec<&Association> = Vec::new();
        let flush = |cur: &mut Vec<&Association>, out: &mut Vec<Episode>| {
            if let (Some(f), Some(l)) = (cur.first(), cur.last()) {
                out.push(Episode {
                    mmsi: m.clone(),
                    source: s.clone(),
                    start_utc: f.utc.clone(),
                    end_utc: l.utc.clone(),
                    reports: cur.len(),
                    max_residual_m: cur.iter().filter_map(|a| a.residual_m).fold(0.0, f64::max),
                });
            }
            cur.clear();
        };
        for a in v {
            if cur.last().is_some_and(|l| a.t - l.t > join_s) {
                flush(&mut cur, &mut out);
            }
            cur.push(a);
        }
        flush(&mut cur, &mut out);
    }
    out
}

fn dark_targets(assoc: &[Association], tracks: &Tracks, q: &Request) -> Vec<DarkTarget> {
    let mut by: BTreeMap<(String, String), Vec<&Association>> = BTreeMap::new();
    for a in assoc
        .iter()
        .filter(|a| a.outcome == Outcome::TargetWithoutAis)
    {
        by.entry((a.source.clone(), a.track_id.clone()))
            .or_default()
            .push(a);
    }
    by.into_iter()
        .map(|((s, tr), v)| {
            let f = v[0];
            let pos = f.position;
            let nearest = tracks
                .by_mmsi
                .keys()
                .filter_map(|m| {
                    let (la, lo, _) = tracks.at(m, f.t, q.params.interp_max_s)?;
                    Some((m.clone(), r(dist_m(pos[0], pos[1], la, lo), 0)))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            DarkTarget {
                source: s,
                track_id: tr,
                reports: v.len(),
                first_utc: f.utc.clone(),
                last_utc: v[v.len() - 1].utc.clone(),
                first_position: pos,
                nearest_ais: nearest,
            }
        })
        .collect()
}

/// HTML page of a report.
pub fn html(rep: &Report) -> String {
    let vrows = rep
        .vessels
        .iter()
        .map(|v| {
            vec![
                v.mmsi.clone(),
                v.compared.to_string(),
                v.consistent.to_string(),
                v.inconsistent.to_string(),
                v.ais_absent.to_string(),
                v.unconfirmed.to_string(),
                v.max_residual_m
                    .map(|x| format!("{x:.0}"))
                    .unwrap_or_default(),
            ]
        })
        .collect();
    let erows = rep
        .episodes
        .iter()
        .map(|e| {
            vec![
                e.mmsi.clone(),
                e.source.clone(),
                e.start_utc.clone(),
                e.end_utc.clone(),
                e.reports.to_string(),
                format!("{:.0}", e.max_residual_m),
            ]
        })
        .collect();
    let drows = rep
        .dark_targets
        .iter()
        .map(|d| {
            vec![
                d.source.clone(),
                d.track_id.clone(),
                d.reports.to_string(),
                d.first_utc.clone(),
                d.last_utc.clone(),
            ]
        })
        .collect();
    crate::transport::page(
        &rep.title,
        rep.evidence,
        &rep.verdict,
        &[rep.statement.to_string()],
        &[
            (
                "Vessels".into(),
                vec![
                    "MMSI",
                    "compared",
                    "consistent",
                    "inconsistent",
                    "AIS absent",
                    "unconfirmed",
                    "max residual (m)",
                ],
                vrows,
            ),
            (
                "Inconsistent episodes".into(),
                vec![
                    "MMSI",
                    "source",
                    "start",
                    "end",
                    "reports",
                    "max residual (m)",
                ],
                erows,
            ),
            (
                "Targets without AIS".into(),
                vec!["source", "track", "reports", "first", "last"],
                drows,
            ),
        ],
    )
}
