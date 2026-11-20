// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Approach availability, go-around exposure and capacity impact (B1101)
//!
//! For one airport and a period, from ADS-B position reports in the terminal area (the
//! documented `timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp` CSV):
//!
//! * **Outage windows.** Time bins of `bin_s` in which at least `min_aircraft` distinct
//!   aircraft, each of which reported good accuracy somewhere in the data (the open map's
//!   equipment baseline guard), were mostly degraded (NIC below `nic_min` or NACp below
//!   `nacp_min`) inside `terminal_radius_nm` and below `terminal_max_agl_ft`, and they were
//!   at least `min_share` of the aircraft sampled there. Outage bins closer than
//!   `merge_gap_s` form one window.
//! * **Approach availability.** For each GNSS-dependent procedure (RNP, RNP AR, LPV, LNAV,
//!   LNAV/VNAV, GLS), the share of observed minutes outside outage windows. Conventional
//!   procedures (ILS, LOC, VOR, NDB, DME) are not affected by a GNSS outage in this model;
//!   their own signals are not assessed. With a final approach segment and open interference
//!   maps, the segment's exposure per map day comes from the open engine's route exposure.
//! * **Go-around exposure.** Approaches (a descent of at least `descent_ft` to below
//!   `approach_floor_agl_ft` inside `approach_radius_nm`) and go-arounds (a climb of
//!   `go_around_climb_ft` after the lowest point) per aircraft, split by whether the approach
//!   fell in an outage window or the aircraft itself was degraded in the `lookback_s` before
//!   its lowest point; rates with Wilson 95 % intervals.
//! * **Capacity impact (modelled).** Outage hours × (nominal − fallback arrival rate), the
//!   fallback rate being what a conventional procedure sustains (stated, or assumed equal to
//!   nominal when one exists and zero when the airport is GNSS-only).
//! * **Oceanic RNP support (optional).** In a named airspace box, the minutes in which an
//!   aircraft's reported accuracy does not support the stated RNP value (EPU bound from NACp
//!   above RNP, or the NIC containment bound above twice RNP, both from the open engine's
//!   DO-260B tables), and how much of that followed a spoofing-like position jump.
//!
//! MODELLED: ADS-B NIC/NACp is a proxy for the GNSS integrity an approach needs, not a
//! RAIM/FDE prediction and not the avionics' own availability. Decision support only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kshana::interference_map::adsb::{nacp_epu_bound_m, nic_rc_bound_range_m};
use kshana::interference_map::grid::haversine_m;
use kshana::interference_map::route;
use serde::{Deserialize, Serialize};

use super::*;
use crate::forensics::inputs::{iso, parse_tracks, PositionReport};

pub const FORMAT: &str = "kshana-pro-approach-availability/1";
pub const METHOD: &str = "kshana-pro/approach-availability/v1";
pub const RESULT_FILE: &str = "approach-availability.json";
pub const EVIDENCE: &str = "MODELLED: rules exercised on synthetic terminal-area traffic with \
known truth; ADS-B NIC/NACp stands in for the integrity a GNSS approach needs. Not validated \
against avionics availability records or ANSP capacity data.";

const NM: f64 = 1852.0;

/// Pre-registered parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub terminal_radius_nm: f64,
    pub terminal_max_agl_ft: f64,
    pub nic_min: u8,
    pub nacp_min: u8,
    pub bin_s: f64,
    pub min_aircraft: usize,
    pub min_share: f64,
    pub merge_gap_s: f64,
    pub approach_radius_nm: f64,
    pub approach_floor_agl_ft: f64,
    pub descent_ft: f64,
    pub go_around_climb_ft: f64,
    pub lookback_s: f64,
    /// Spoofing-like jump: at least this far ...
    pub jump_km: f64,
    /// ... at more than this implied speed.
    pub jump_kn: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            terminal_radius_nm: 30.0,
            terminal_max_agl_ft: 10_000.0,
            nic_min: 7,
            nacp_min: 8,
            bin_s: 300.0,
            min_aircraft: 2,
            min_share: 0.5,
            merge_gap_s: 600.0,
            approach_radius_nm: 12.0,
            approach_floor_agl_ft: 1_500.0,
            descent_ft: 1_000.0,
            go_around_climb_ft: 500.0,
            lookback_s: 600.0,
            jump_km: 5.0,
            jump_kn: 900.0,
        }
    }
}

impl Params {
    pub fn check(&self) -> Result<(), String> {
        let pos = [
            ("terminal_radius_nm", self.terminal_radius_nm),
            ("terminal_max_agl_ft", self.terminal_max_agl_ft),
            ("bin_s", self.bin_s),
            ("approach_radius_nm", self.approach_radius_nm),
            ("descent_ft", self.descent_ft),
            ("go_around_climb_ft", self.go_around_climb_ft),
        ];
        for (n, v) in pos {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("params: {n} must be above 0"));
            }
        }
        if self.min_aircraft == 0 {
            return Err("params: min_aircraft must be at least 1".into());
        }
        if !(0.0..=1.0).contains(&self.min_share) {
            return Err("params: min_share must be within 0 to 1".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Airport {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub lat: f64,
    pub lon: f64,
    #[serde(default)]
    pub elevation_ft: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Procedure {
    pub id: String,
    /// `rnp`, `rnp-ar`, `lpv`, `lnav`, `lnav-vnav`, `gls`, `ils`, `loc`, `vor`, `ndb`, `dme`.
    pub kind: String,
    #[serde(default)]
    pub runway: Option<String>,
    /// Final approach fix `[lat, lon]` and threshold, for map exposure of the final segment.
    #[serde(default)]
    pub faf: Option<[f64; 2]>,
    #[serde(default)]
    pub threshold: Option<[f64; 2]>,
}

/// Whether a procedure kind needs GNSS (`None` for an unknown kind).
pub fn gnss_dependent(kind: &str) -> Option<bool> {
    match kind.to_ascii_lowercase().as_str() {
        "rnp" | "rnp-ar" | "rnp-apch" | "lpv" | "lnav" | "lnav-vnav" | "lp" | "gls"
        | "rnav-gnss" => Some(true),
        "ils" | "loc" | "vor" | "vor-dme" | "ndb" | "dme" | "par" | "visual" => Some(false),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capacity {
    /// Arrivals per hour the airport plans with GNSS approaches available.
    pub nominal_arrivals_per_hour: f64,
    /// Arrivals per hour on the fallback (conventional) procedure; default: nominal when a
    /// conventional procedure exists, zero when the airport is GNSS-only.
    #[serde(default)]
    pub fallback_arrivals_per_hour: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Oceanic {
    pub name: String,
    /// `[south, west, north, east]`.
    #[serde(rename = "box")]
    pub area: [f64; 4],
    #[serde(default = "rnp4")]
    pub rnp_nm: f64,
}

fn rnp4() -> f64 {
    4.0
}

/// An approach-availability request (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub airport: Airport,
    #[serde(default)]
    pub procedures: Vec<Procedure>,
    pub adsb: Vec<String>,
    /// Open interference maps (GeoJSON) for the final-segment exposure.
    #[serde(default)]
    pub maps: Vec<String>,
    #[serde(default)]
    pub capacity: Option<Capacity>,
    #[serde(default)]
    pub oceanic: Option<Oceanic>,
    #[serde(default)]
    pub params: Params,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub n: usize,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_min: f64,
    /// Most distinct degraded aircraft in one bin.
    pub degraded_aircraft_max: usize,
    /// Distinct aircraft degraded at some point in the window.
    pub degraded_aircraft: usize,
    pub sampled_aircraft: usize,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MapExposure {
    pub date: String,
    pub share_degraded: f64,
    pub share_unassessed: f64,
    pub share_not_observed: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProcedureAvailability {
    pub id: String,
    pub kind: String,
    pub runway: Option<String>,
    pub gnss_dependent: bool,
    pub observed_min: f64,
    pub unavailable_min: f64,
    /// Share of observed minutes available; `None` when nothing was observed.
    pub availability: Option<f64>,
    pub final_segment_map_exposure: Vec<MapExposure>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fallback {
    pub gnss_only_airport: bool,
    pub conventional_procedures: Vec<String>,
    pub outage_min: f64,
    /// Outage minutes during which a conventional procedure was configured.
    pub outage_min_with_conventional_fallback: f64,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoAround {
    pub aircraft: String,
    pub lowest_utc: String,
    pub lowest_agl_ft: f64,
    pub in_outage_window: bool,
    pub own_degradation_before: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RateCell {
    pub approaches: usize,
    pub go_arounds: usize,
    pub rate: Option<f64>,
    pub ci95: Option<(f64, f64)>,
}

impl RateCell {
    fn new(approaches: usize, go_arounds: usize) -> Self {
        RateCell {
            approaches,
            go_arounds,
            rate: (approaches > 0).then(|| rnd(go_arounds as f64 / approaches as f64, 4)),
            ci95: wilson95(go_arounds, approaches),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoAroundExposure {
    pub under_degradation: RateCell,
    pub otherwise: RateCell,
    /// Rate under degradation over the rate otherwise (when both are defined and the second
    /// is above zero).
    pub rate_ratio: Option<f64>,
    pub go_arounds: Vec<GoAround>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CapacityImpact {
    pub nominal_arrivals_per_hour: f64,
    pub fallback_arrivals_per_hour: f64,
    pub fallback_basis: String,
    pub outage_hours: f64,
    pub arrivals_lost_modelled: f64,
    pub arrivals_expected_in_outage: f64,
    /// Approaches seen ending low (landing or data end) inside outage windows.
    pub arrivals_observed_in_outage: usize,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OceanicAircraft {
    pub aircraft: String,
    pub minutes_in_area: f64,
    pub minutes_rnp_not_supported: f64,
    pub longest_run_min: f64,
    pub spoofing_jumps: usize,
    pub minutes_not_supported_after_jump: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OceanicResult {
    pub name: String,
    pub area: [f64; 4],
    pub rnp_nm: f64,
    pub rule: String,
    pub aircraft: Vec<OceanicAircraft>,
}

/// The result (`kshana-pro-approach-availability/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ApproachResult {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub statement: String,
    pub title: String,
    pub airport: Airport,
    pub period_start_utc: Option<String>,
    pub period_end_utc: Option<String>,
    pub aircraft: usize,
    pub aircraft_with_baseline: usize,
    pub observed_min: f64,
    pub outage_windows: Vec<Window>,
    pub procedures: Vec<ProcedureAvailability>,
    pub fallback: Fallback,
    pub go_around_exposure: GoAroundExposure,
    pub capacity: Option<CapacityImpact>,
    pub oceanic: Option<OceanicResult>,
    pub risk_factors: Vec<String>,
    pub limits: Vec<String>,
    pub params: Params,
    pub inputs: Vec<InputRef>,
}

/// A run of outage bins: first and last bin, most degraded in one bin, degraded and sampled aircraft.
type Run<'a> = (i64, i64, usize, BTreeSet<&'a String>, BTreeSet<&'a String>);

fn degraded(r: &PositionReport, p: &Params) -> Option<bool> {
    match (r.nic, r.nacp) {
        (None, None) => None,
        (a, b) => Some(a.is_some_and(|v| v < p.nic_min) || b.is_some_and(|v| v < p.nacp_min)),
    }
}

/// Analyse parsed inputs: ADS-B texts and map texts `(name, text)`.
pub fn analyse(
    req: &Request,
    adsb: &[(String, String)],
    maps: &[(String, String)],
    inputs: &Inputs,
) -> Result<ApproachResult, String> {
    let p = &req.params;
    p.check()?;
    let ap = &req.airport;
    if !((-90.0..=90.0).contains(&ap.lat) && (-180.0..=180.0).contains(&ap.lon)) {
        return Err("airport: lat/lon out of range".into());
    }
    for pr in &req.procedures {
        if gnss_dependent(&pr.kind).is_none() {
            return Err(format!(
                "procedure {}: unknown kind '{}' (rnp, rnp-ar, lpv, lnav, lnav-vnav, gls, ils, loc, vor, ndb, dme)",
                pr.id, pr.kind
            ));
        }
    }
    let mut tracks: BTreeMap<String, Vec<PositionReport>> = BTreeMap::new();
    for (name, text) in adsb {
        let t = parse_tracks(text, "aircraft_id").map_err(|e| format!("{name}: {e}"))?;
        for (id, v) in t.by_id {
            tracks.entry(id).or_default().extend(v);
        }
    }
    for v in tracks.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    let elev = ap.elevation_ft;
    let dist_nm = |r: &PositionReport| haversine_m(ap.lat, ap.lon, r.lat, r.lon) / NM;
    let in_terminal = |r: &PositionReport| {
        dist_nm(r) <= p.terminal_radius_nm
            && r.alt_ft.is_none_or(|a| a - elev <= p.terminal_max_agl_ft)
    };
    let baseline: BTreeSet<&String> = tracks
        .iter()
        .filter(|(_, v)| v.iter().any(|r| degraded(r, p) == Some(false)))
        .map(|(k, _)| k)
        .collect();
    let all_t: Vec<f64> = tracks
        .values()
        .flatten()
        .filter(|r| in_terminal(r))
        .map(|r| r.t)
        .collect();
    let t_min = all_t.iter().copied().fold(f64::INFINITY, f64::min);
    let t_max = all_t.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let start = if t_min.is_finite() {
        (t_min / p.bin_s).floor() * p.bin_s
    } else {
        0.0
    };
    // Per bin: aircraft -> (assessable, degraded) sample counts.
    let mut bins: BTreeMap<i64, BTreeMap<&String, (usize, usize)>> = BTreeMap::new();
    for (id, v) in &tracks {
        if !baseline.contains(id) {
            continue;
        }
        for r in v.iter().filter(|r| in_terminal(r)) {
            if let Some(d) = degraded(r, p) {
                let b = ((r.t - start) / p.bin_s).floor() as i64;
                let e = bins.entry(b).or_default().entry(id).or_default();
                e.0 += 1;
                e.1 += d as usize;
            }
        }
    }
    let observed_min = bins.len() as f64 * p.bin_s / 60.0;
    let mut outage_bins: Vec<(i64, BTreeSet<&String>, BTreeSet<&String>)> = vec![];
    for (b, m) in &bins {
        let deg: BTreeSet<&String> = m
            .iter()
            .filter(|(_, (n, d))| 2 * d >= *n && *d > 0)
            .map(|(k, _)| *k)
            .collect();
        if deg.len() >= p.min_aircraft && deg.len() as f64 >= p.min_share * m.len() as f64 {
            outage_bins.push((*b, deg, m.keys().copied().collect()));
        }
    }
    let mut windows: Vec<Window> = vec![];
    let mut cur: Option<Run> = None;
    let gap_bins = (p.merge_gap_s / p.bin_s).floor() as i64;
    let close = |c: Run, w: &mut Vec<Window>| {
        let t0 = start + c.0 as f64 * p.bin_s;
        let t1 = start + (c.1 + 1) as f64 * p.bin_s;
        w.push(Window {
            n: w.len() + 1,
            start_utc: iso(t0),
            end_utc: iso(t1),
            duration_min: rnd((t1 - t0) / 60.0, 1),
            degraded_aircraft_max: c.2,
            degraded_aircraft: c.3.len(),
            sampled_aircraft: c.4.len(),
            t0,
            t1,
        });
    };
    for (b, deg, samp) in outage_bins {
        cur = match cur.take() {
            Some(mut c) if b - c.1 <= gap_bins + 1 => {
                c.1 = b;
                c.2 = c.2.max(deg.len());
                c.3.extend(deg);
                c.4.extend(samp);
                Some(c)
            }
            Some(c) => {
                close(c, &mut windows);
                Some((b, b, deg.len(), deg, samp))
            }
            None => Some((b, b, deg.len(), deg, samp)),
        };
    }
    if let Some(c) = cur {
        close(c, &mut windows);
    }
    let in_window = |t: f64| windows.iter().any(|w| t >= w.t0 && t <= w.t1);
    let outage_min: f64 = windows.iter().map(|w| (w.t1 - w.t0) / 60.0).sum();

    // Procedures.
    let mut loaded = vec![];
    for (name, text) in maps {
        loaded.push(route::load_map(text).map_err(|e| format!("{name}: {e}"))?);
    }
    let conventional: Vec<String> = req
        .procedures
        .iter()
        .filter(|pr| gnss_dependent(&pr.kind) == Some(false))
        .map(|pr| pr.id.clone())
        .collect();
    let procedures: Vec<ProcedureAvailability> = req
        .procedures
        .iter()
        .map(|pr| {
            let g = gnss_dependent(&pr.kind) == Some(true);
            let unavailable = if g { outage_min } else { 0.0 };
            let exposure = match (pr.faf, pr.threshold) {
                (Some(a), Some(b)) => loaded
                    .iter()
                    .map(|m| {
                        let e = route::exposure(&[(a[0], a[1]), (b[0], b[1])], m);
                        MapExposure {
                            date: e.date,
                            share_degraded: rnd(e.share_degraded, 4),
                            share_unassessed: rnd(e.share_unassessed, 4),
                            share_not_observed: rnd(e.share_not_observed, 4),
                        }
                    })
                    .collect(),
                _ => vec![],
            };
            ProcedureAvailability {
                id: pr.id.clone(),
                kind: pr.kind.clone(),
                runway: pr.runway.clone(),
                gnss_dependent: g,
                observed_min: rnd(observed_min, 1),
                unavailable_min: rnd(unavailable, 1),
                availability: (observed_min > 0.0)
                    .then(|| rnd(1.0 - unavailable / observed_min, 4)),
                final_segment_map_exposure: exposure,
                note: if g {
                    "GNSS-dependent: unavailable during outage windows (ADS-B integrity proxy)."
                        .into()
                } else {
                    "Conventional: not affected by a GNSS outage in this model; its own signal is not assessed.".into()
                },
            }
        })
        .collect();
    let gnss_only = conventional.is_empty();
    let fallback = Fallback {
        gnss_only_airport: gnss_only,
        conventional_procedures: conventional.clone(),
        outage_min: rnd(outage_min, 1),
        outage_min_with_conventional_fallback: rnd(if gnss_only { 0.0 } else { outage_min }, 1),
        note: if gnss_only {
            "No conventional procedure is configured: during outage windows arrivals depend on visual conditions, holding or diversion.".into()
        } else {
            "A conventional procedure is configured; whether its ground aids were serviceable is not assessed.".into()
        },
    };

    // Approaches and go-arounds.
    let mut ga_events = vec![];
    let (mut ap_in, mut ga_in, mut ap_out, mut ga_out) = (0usize, 0usize, 0usize, 0usize);
    let mut landed_in_outage = 0usize;
    for (id, v) in &tracks {
        let near: Vec<&PositionReport> = v
            .iter()
            .filter(|r| r.alt_ft.is_some() && dist_nm(r) <= p.approach_radius_nm)
            .collect();
        let Some(first) = near.first() else { continue };
        let mut peak = first.alt_ft.unwrap_or(0.0) - elev;
        let mut low: Option<(f64, f64)> = None; // agl, t
        let own_deg = |t: f64| {
            v.iter().any(|r| {
                r.t <= t
                    && r.t >= t - p.lookback_s
                    && in_terminal(r)
                    && degraded(r, p) == Some(true)
            })
        };
        let mut record = |agl: f64, t: f64, ga: bool, out: &mut Vec<GoAround>| {
            let iw = in_window(t);
            let od = own_deg(t);
            let under = iw || od;
            match (under, ga) {
                (true, true) => {
                    ap_in += 1;
                    ga_in += 1
                }
                (true, false) => ap_in += 1,
                (false, true) => {
                    ap_out += 1;
                    ga_out += 1
                }
                (false, false) => ap_out += 1,
            }
            if ga {
                out.push(GoAround {
                    aircraft: id.clone(),
                    lowest_utc: iso(t),
                    lowest_agl_ft: rnd(agl, 0),
                    in_outage_window: iw,
                    own_degradation_before: od,
                });
            } else if iw {
                landed_in_outage += 1;
            }
        };
        for r in &near {
            let agl = r.alt_ft.unwrap_or(0.0) - elev;
            match low {
                None => {
                    if agl > peak {
                        peak = agl;
                    } else if peak - agl >= p.descent_ft && agl <= p.approach_floor_agl_ft {
                        low = Some((agl, r.t));
                    }
                }
                Some((la, lt)) => {
                    if agl < la {
                        low = Some((agl, r.t));
                    } else if agl - la >= p.go_around_climb_ft {
                        record(la, lt, true, &mut ga_events);
                        peak = agl;
                        low = None;
                    }
                }
            }
        }
        if let Some((la, lt)) = low {
            record(la, lt, false, &mut ga_events);
        }
    }
    let under = RateCell::new(ap_in, ga_in);
    let other = RateCell::new(ap_out, ga_out);
    let rate_ratio = match (under.rate, other.rate) {
        (Some(a), Some(b)) if b > 0.0 => Some(rnd(a / b, 3)),
        _ => None,
    };

    let capacity = req.capacity.as_ref().map(|c| {
        let (fb, basis) = match c.fallback_arrivals_per_hour {
            Some(f) => (f, "stated in the request".to_string()),
            None if gnss_only => (0.0, "assumed zero: GNSS-only airport".to_string()),
            None => (
                c.nominal_arrivals_per_hour,
                "assumed equal to nominal: a conventional procedure is configured".to_string(),
            ),
        };
        let h = outage_min / 60.0;
        CapacityImpact {
            nominal_arrivals_per_hour: rnd(c.nominal_arrivals_per_hour, 2),
            fallback_arrivals_per_hour: rnd(fb, 2),
            fallback_basis: basis,
            outage_hours: rnd(h, 3),
            arrivals_lost_modelled: rnd(h * (c.nominal_arrivals_per_hour - fb).max(0.0), 2),
            arrivals_expected_in_outage: rnd(h * c.nominal_arrivals_per_hour, 2),
            arrivals_observed_in_outage: landed_in_outage,
            note: "Modelled: lost arrivals = outage hours × (nominal − fallback rate). Holding, delay and diversion costs are not modelled.".into(),
        }
    });

    let oceanic = match &req.oceanic {
        Some(o) => Some(oceanic(o, &tracks, p)?),
        None => None,
    };

    let mut risk = vec![];
    if !windows.is_empty() {
        risk.push(format!(
            "{} outage window(s), {:.0} min in total, in the terminal area",
            windows.len(),
            outage_min
        ));
        if gnss_only {
            risk.push("GNSS-only airport: no conventional approach to fall back on during the outage windows".into());
        }
    }
    if ga_in > 0 {
        risk.push(format!(
            "{ga_in} go-around(s) in {ap_in} approach(es) under degradation{}",
            rate_ratio
                .map(|r| format!(", {r}× the rate otherwise"))
                .unwrap_or_default()
        ));
    }
    let limits = vec![
        "ADS-B NIC/NACp is a proxy: it shows the integrity the aircraft broadcast, not the approach availability its avionics computed (RAIM/FDE, SBAS).".to_string(),
        "Go-arounds are inferred from altitude only; a go-around for weather, traffic or an unstable approach looks the same. Rates are associations, not causes.".into(),
        "Aircraft without a good-accuracy report in the data are left out of the outage windows (equipment baseline guard).".into(),
        "Capacity impact is a stated-rate model, not a simulation of the arrival flow.".into(),
        "Observed minutes count only bins with assessable ADS-B traffic in the terminal area; quiet periods are not observed.".into(),
    ];
    Ok(ApproachResult {
        format: FORMAT.into(),
        method: METHOD.into(),
        evidence: EVIDENCE.into(),
        statement: DECISION_SUPPORT.into(),
        title: req.title.clone(),
        airport: req.airport.clone(),
        period_start_utc: t_min.is_finite().then(|| iso(t_min)),
        period_end_utc: t_max.is_finite().then(|| iso(t_max)),
        aircraft: tracks.len(),
        aircraft_with_baseline: baseline.len(),
        observed_min: rnd(observed_min, 1),
        outage_windows: windows,
        procedures,
        fallback,
        go_around_exposure: GoAroundExposure {
            under_degradation: under,
            otherwise: other,
            rate_ratio,
            go_arounds: ga_events,
        },
        capacity,
        oceanic,
        risk_factors: risk,
        limits,
        params: p.clone(),
        inputs: inputs.refs.clone(),
    })
}

fn oceanic(
    o: &Oceanic,
    tracks: &BTreeMap<String, Vec<PositionReport>>,
    p: &Params,
) -> Result<OceanicResult, String> {
    let b = GeoBox::from_array(o.area)?;
    if !(o.rnp_nm.is_finite() && o.rnp_nm > 0.0) {
        return Err("oceanic: rnp_nm must be above 0".into());
    }
    let epu_max = o.rnp_nm * NM;
    let rc_max = 2.0 * o.rnp_nm * NM;
    // Some(true) supported, Some(false) not, None unknown.
    let supports = |r: &PositionReport| -> Option<bool> {
        let e = r
            .nacp
            .map(|c| nacp_epu_bound_m(c).is_some_and(|m| m <= epu_max));
        let c = r
            .nic
            .map(|c| nic_rc_bound_range_m(c).is_some_and(|(_, hi)| hi <= rc_max));
        match (e, c) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(true) && b.unwrap_or(true)),
        }
    };
    let mut out = vec![];
    for (id, v) in tracks {
        let inside: Vec<&PositionReport> = v.iter().filter(|r| b.contains(r.lat, r.lon)).collect();
        if inside.len() < 2 {
            continue;
        }
        let (mut in_s, mut bad_s, mut run, mut longest, mut after_jump) =
            (0.0, 0.0, 0.0f64, 0.0f64, 0.0);
        let mut jumps = 0usize;
        let mut jumped = false;
        for w in inside.windows(2) {
            let dt = (w[1].t - w[0].t).max(0.0);
            let d = haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
            if d >= p.jump_km * 1000.0 && d / dt.max(1e-3) / 0.514_444 > p.jump_kn {
                jumps += 1;
                jumped = true;
            }
            let dt = dt.min(600.0); // a gap in coverage is not counted as time in the area
            in_s += dt;
            if supports(w[0]) == Some(false) {
                bad_s += dt;
                run += dt;
                longest = longest.max(run);
                if jumped {
                    after_jump += dt;
                }
            } else {
                run = 0.0;
            }
        }
        out.push(OceanicAircraft {
            aircraft: id.clone(),
            minutes_in_area: rnd(in_s / 60.0, 1),
            minutes_rnp_not_supported: rnd(bad_s / 60.0, 1),
            longest_run_min: rnd(longest / 60.0, 1),
            spoofing_jumps: jumps,
            minutes_not_supported_after_jump: rnd(after_jump / 60.0, 1),
        });
    }
    Ok(OceanicResult {
        name: o.name.clone(),
        area: o.area,
        rnp_nm: o.rnp_nm,
        rule: format!(
            "A report does not support RNP {} when its NACp EPU bound exceeds {} NM or its NIC containment bound (largest over versions) exceeds {} NM; tables from the open engine (DO-260B values). Unknown codes count as not supported.",
            o.rnp_nm,
            o.rnp_nm,
            2.0 * o.rnp_nm
        ),
        aircraft: out,
    })
}

/// Read a request file and its inputs, and analyse.
pub fn run(path: &Path) -> Result<(ApproachResult, Inputs), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut inp = Inputs::default();
    inp.add_text("request.toml", "request", &text);
    let mut adsb = vec![];
    for a in &req.adsb {
        adsb.push((a.clone(), inp.read(base, a, "adsb")?));
    }
    let mut maps = vec![];
    for m in &req.maps {
        maps.push((m.clone(), inp.read(base, m, "interference-map")?));
    }
    let r = analyse(&req, &adsb, &maps, &inp)?;
    Ok((r, inp))
}

/// The plain-text summary.
pub fn summary(r: &ApproachResult) -> String {
    let mut s = format!(
        "Approach availability: {} ({})\n{}\nEvidence: {}\n\nObserved {} min of terminal-area traffic; {} outage window(s)\n",
        r.airport.id, r.title, r.statement, r.evidence, r.observed_min, r.outage_windows.len()
    );
    for w in &r.outage_windows {
        s += &format!(
            "  #{} {} to {} ({} min, {} aircraft degraded of {} sampled)\n",
            w.n, w.start_utc, w.end_utc, w.duration_min, w.degraded_aircraft, w.sampled_aircraft
        );
    }
    for p in &r.procedures {
        s += &format!(
            "Procedure {} ({}): availability {}\n",
            p.id,
            p.kind,
            p.availability
                .map(|a| format!("{:.1} %", a * 100.0))
                .unwrap_or_else(|| "not observed".into())
        );
    }
    let g = &r.go_around_exposure;
    s += &format!(
        "Go-arounds: {} of {} approaches under degradation, {} of {} otherwise\n",
        g.under_degradation.go_arounds,
        g.under_degradation.approaches,
        g.otherwise.go_arounds,
        g.otherwise.approaches
    );
    if let Some(c) = &r.capacity {
        s += &format!(
            "Capacity (modelled): {} arrivals lost over {} h of outage\n",
            c.arrivals_lost_modelled, c.outage_hours
        );
    }
    for f in &r.risk_factors {
        s += &format!("Risk factor: {f}\n");
    }
    s
}
