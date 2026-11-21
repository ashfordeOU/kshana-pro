// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Crisis PNT service-availability model
//!
//! A planner preparing for a crisis asks: if GNSS is denied over this area for this long, and
//! these terrestrial and LEO services remain, what service does each class of user get? This
//! module answers per grid cell and user class, from models that already exist:
//!
//! * **eLoran**: the eLoran network planner ([`kshana_eloran::plan::Planner`]) on the stations
//!   that stay on air: field strength and SNR against the stated noise, weighted HDOP, 95 %
//!   horizontal accuracy and the availability over independent station outages, with the
//!   chosen propagation model.
//! * **LEO PNT**: coverage from the open engine's Walker geometry
//!   ([`kshana::walker::coverage_half_angle_rad`], [`kshana::walker::street_half_width_rad`]):
//!   continuous where a street of coverage closes and the latitude is within reach, else the
//!   expected fraction of time in view; the accuracy and timing figures are the provider's stated
//!   values, carried as stated.
//! * **Holdover**: the open engine's closed-form clock holdover
//!   ([`kshana::holdover::holdover_for_class`]) to each timing class's threshold, against the
//!   denial duration.
//!
//! Per user class and cell: whether GNSS is denied, which allowed sources meet the class's
//! need, the modelled service availability `1 − Π(1 − aᵢ)` over the sources that meet it, the
//! accuracy class, and for timing classes the holdover and the time beyond it. Per class: the
//! share of the area served at the required availability.
//!
//! A **planning and evaluation tool**: it computes no navigation or timing solution and fuses
//! nothing. **MODELLED** throughout: the eLoran figures are the planner's, the LEO figures the
//! provider's stated ones, and the clock classes the open engine's representative classes.

use std::collections::BTreeMap;

use kshana::clock_state::ClockClass;
use kshana_eloran::geo::Geo;
use kshana_eloran::plan::{Candidate, PlanArea, PlanSettings, Planner};
use kshana_eloran::prop::{self, BuildContext, PropConfig};
use kshana_eloran::time::AbsTime;
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::round;

/// Pro feature.
pub const FEATURE: &str = "pro-crisis-pnt-availability";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-crisis-pnt-availability/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: eLoran figures from the network planner's propagation \
and geometry model; LEO accuracy and timing as the provider states them with Walker coverage \
geometry; holdover from representative clock classes. A planning tool, not a measurement.";
/// Scope.
pub const SCOPE: &str = "Service availability for planning: no navigation or timing solution \
is computed and no sources are fused.";

/// The area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
    pub step_deg: f64,
}

/// The denial.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denial {
    pub start_utc: String,
    pub duration_h: f64,
    /// Polygon `[lat, lon]` of the denied area; the whole area when absent.
    #[serde(default)]
    pub polygon: Option<Vec<[f64; 2]>>,
}

/// An eLoran station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EloranStation {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub power_kw: f64,
    /// On air during the crisis.
    #[serde(default = "d_true")]
    pub on_air: bool,
}

fn d_true() -> bool {
    true
}

/// eLoran part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EloranSection {
    #[serde(rename = "station")]
    pub stations: Vec<EloranStation>,
    #[serde(default)]
    pub noise_uv_m_20k: Option<f64>,
    #[serde(default)]
    pub station_availability: Option<f64>,
    /// Timing accuracy the eLoran service states for a timing user (ns).
    pub timing_ns: f64,
    #[serde(default)]
    pub propagation: PropConfig,
}

/// LEO PNT part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeoSection {
    pub name: String,
    pub altitude_km: f64,
    pub inclination_deg: f64,
    pub planes: usize,
    pub sats_per_plane: usize,
    pub min_elev_deg: f64,
    /// Stated by the provider.
    pub accuracy95_m: f64,
    pub timing_ns: f64,
    /// Service availability the provider states when in view.
    #[serde(default = "d_one")]
    pub availability: f64,
    pub source_of_figures: String,
}

fn d_one() -> f64 {
    1.0
}

/// What a user class needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Need {
    Position,
    Timing,
}

/// A user class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserClass {
    pub name: String,
    pub need: Need,
    /// Position: required 95 % horizontal accuracy (m).
    #[serde(default)]
    pub accuracy95_m: Option<f64>,
    /// Timing: required accuracy (ns) and the clock the user holds over with.
    #[serde(default)]
    pub timing_ns: Option<f64>,
    #[serde(default)]
    pub clock: Option<String>,
    /// Required service availability.
    pub availability: f64,
    /// Sources the class can use: `eloran`, `leo`.
    pub sources: Vec<String>,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub data_source: String,
    pub area: Area,
    pub denial: Denial,
    #[serde(default)]
    pub eloran: Option<EloranSection>,
    #[serde(default)]
    pub leo: Option<LeoSection>,
    #[serde(rename = "user_class")]
    pub classes: Vec<UserClass>,
    /// Accuracy (m) of GNSS where it is not denied.
    #[serde(default = "d_gnss")]
    pub gnss_accuracy95_m: f64,
    /// Timing accuracy (ns) of GNSS where it is not denied.
    #[serde(default = "d_gnss_t")]
    pub gnss_timing_ns: f64,
}

fn d_gnss_t() -> f64 {
    50.0
}

fn d_gnss() -> f64 {
    5.0
}

/// One cell for one class.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cell {
    pub lat: f64,
    pub lon: f64,
    pub gnss_denied: bool,
    pub sources_meeting_need: Vec<String>,
    pub availability: f64,
    /// Best accuracy (m) or timing (ns) available among the sources that meet the need.
    pub best: Option<f64>,
    pub accuracy_class: String,
    pub served: bool,
}

/// One class's result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassResult {
    pub class: String,
    pub need: Need,
    pub cells: usize,
    pub served_share: f64,
    pub denied_cells: usize,
    pub denied_served_share: f64,
    /// Timing: holdover to the threshold (h) and hours of denial beyond it.
    pub holdover_h: Option<f64>,
    pub beyond_holdover_h: Option<f64>,
    pub by_source: BTreeMap<String, usize>,
    pub grid: Vec<Cell>,
    pub summary: String,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub evidence: &'static str,
    pub scope: &'static str,
    pub title: String,
    pub data_source: String,
    pub denial: Denial,
    pub eloran_on_air: Vec<String>,
    pub eloran_off_air: Vec<String>,
    pub leo: Option<String>,
    pub classes: Vec<ClassResult>,
    pub limits: Vec<String>,
    pub verdict: String,
}

fn inside(poly: &[[f64; 2]], lat: f64, lon: f64) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a[0] > lat) != (b[0] > lat) && lon < (b[1] - a[1]) * (lat - a[0]) / (b[0] - a[0]) + a[1]
        {
            c = !c;
        }
    }
    c
}

/// Modelled LEO availability at a latitude (fraction of time a service is in view).
pub fn leo_in_view(l: &LeoSection, lat_deg: f64) -> f64 {
    let Some(lam) = kshana::walker::coverage_half_angle_rad(l.altitude_km, l.min_elev_deg) else {
        return 0.0;
    };
    let reach = l.inclination_deg.min(180.0 - l.inclination_deg) + lam.to_degrees();
    if lat_deg.abs() > reach {
        return 0.0;
    }
    // Continuous when the streets of the planes close around the globe.
    if let Some(c) = kshana::walker::street_half_width_rad(lam, l.sats_per_plane) {
        if l.planes as f64 * 2.0 * c >= std::f64::consts::PI {
            return 1.0;
        }
    }
    // Otherwise the expected fraction: satellites × cap area over the band they cover.
    let cap = (1.0 - lam.cos()) / 2.0;
    let band = (reach.min(90.0).to_radians()).sin();
    ((l.planes * l.sats_per_plane) as f64 * cap / band.max(1e-9)).min(1.0)
}

fn clock_class(s: &str) -> Result<ClockClass, String> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "tcxo" => ClockClass::Tcxo,
        "ocxo" => ClockClass::Ocxo,
        "rafs" | "rubidium" => ClockClass::Rafs,
        "csac" => ClockClass::Csac,
        "uso" => ClockClass::Uso,
        other => {
            return Err(format!(
                "clock '{other}': one of tcxo, ocxo, rafs, csac, uso"
            ))
        }
    })
}

fn pos_class(m: Option<f64>) -> String {
    match m {
        Some(x) if x <= 10.0 => "A (≤10 m)".into(),
        Some(x) if x <= 100.0 => "B (≤100 m)".into(),
        Some(x) if x <= 1000.0 => "C (≤1 km)".into(),
        Some(_) => "D (>1 km)".into(),
        None => "none".into(),
    }
}

fn time_class(ns: Option<f64>) -> String {
    match ns {
        Some(x) if x <= 100.0 => "T1 (≤100 ns)".into(),
        Some(x) if x <= 1000.0 => "T2 (≤1 µs)".into(),
        Some(x) if x <= 1e6 => "T3 (≤1 ms)".into(),
        Some(_) => "T4 (>1 ms)".into(),
        None => "none".into(),
    }
}

/// eLoran at one grid point: 95 % accuracy with every station up, and availability.
type EloranPoint = (Option<f64>, f64);

/// Evaluate.
pub fn evaluate(q: &Request) -> Result<Report, String> {
    let a = &q.area;
    if !(a.step_deg > 0.0 && a.lat_max >= a.lat_min && a.lon_max >= a.lon_min) {
        return Err("area: lat/lon bounds and a positive step".into());
    }
    if q.data_source.trim().is_empty() {
        return Err("data_source must be stated".into());
    }
    if q.denial.duration_h.is_nan() || q.denial.duration_h <= 0.0 {
        return Err("denial.duration_h must be positive".into());
    }
    let start = AbsTime::from_utc_iso(&q.denial.start_utc)?;
    if let Some(l) = &q.leo {
        if l.source_of_figures.trim().is_empty() {
            return Err("leo.source_of_figures: say where the stated figures come from".into());
        }
    }
    let nlat = ((a.lat_max - a.lat_min) / a.step_deg).round() as usize + 1;
    let nlon = ((a.lon_max - a.lon_min) / a.step_deg).round() as usize + 1;
    let pts: Vec<(f64, f64)> = (0..nlat)
        .flat_map(|i| (0..nlon).map(move |j| (i, j)))
        .map(|(i, j)| {
            (
                a.lat_min + i as f64 * a.step_deg,
                a.lon_min + j as f64 * a.step_deg,
            )
        })
        .collect();
    let denied: Vec<bool> = pts
        .iter()
        .map(|p| match &q.denial.polygon {
            Some(poly) => inside(poly, p.0, p.1),
            None => true,
        })
        .collect();

    // eLoran: the propagation model and the on-air stations.
    let (on, off): (Vec<&EloranStation>, Vec<&EloranStation>) = q
        .eloran
        .as_ref()
        .map(|e| e.stations.iter().partition(|s| s.on_air))
        .unwrap_or_default();
    let model = match &q.eloran {
        Some(e) => Some(prop::build(
            &e.propagation,
            &BuildContext::new(start, q.denial.duration_h * 3600.0, 1),
        )?),
        None => None,
    };
    // eLoran per class target: (accuracy95 at all up, availability vs target) per point.
    let eloran_eval = |target_m: f64| -> Result<Option<Vec<EloranPoint>>, String> {
        let (Some(e), Some(m)) = (&q.eloran, &model) else {
            return Ok(None);
        };
        if on.is_empty() {
            return Ok(Some(vec![(None, 0.0); pts.len()]));
        }
        let mut st = PlanSettings::default();
        if let Some(n) = e.noise_uv_m_20k {
            st.noise_uv_m_20k = n;
        }
        if let Some(x) = e.station_availability {
            st.station_availability = x;
        }
        st.accuracy_target_m = target_m;
        let cands: Vec<Candidate> = on
            .iter()
            .map(|s| Candidate {
                id: s.id.clone(),
                position: Geo::new(s.lat, s.lon),
                power_kw: s.power_kw,
                cost: 0.0,
            })
            .collect();
        let pl = Planner::new(
            cands,
            PlanArea {
                lat_min: a.lat_min,
                lat_max: a.lat_max,
                lon_min: a.lon_min,
                lon_max: a.lon_max,
                step_deg: a.step_deg,
            },
            st,
            m.as_ref(),
        )?;
        let idx: Vec<usize> = (0..on.len()).collect();
        let c = pl.evaluate(&idx);
        Ok(Some(
            c.points
                .iter()
                .map(|p| (p.accuracy95_m, p.availability))
                .collect(),
        ))
    };

    let mut classes = vec![];
    for uc in &q.classes {
        if !(0.0..=1.0).contains(&uc.availability) {
            return Err(format!("class {}: availability in 0..1", uc.name));
        }
        for s in &uc.sources {
            if s != "eloran" && s != "leo" {
                return Err(format!(
                    "class {}: source '{s}' is not eloran or leo",
                    uc.name
                ));
            }
        }
        let (target, holdover_h) = match uc.need {
            Need::Position => (
                uc.accuracy95_m
                    .ok_or_else(|| format!("class {}: accuracy95_m needed", uc.name))?,
                None,
            ),
            Need::Timing => {
                let t = uc
                    .timing_ns
                    .ok_or_else(|| format!("class {}: timing_ns needed", uc.name))?;
                let c = clock_class(uc.clock.as_deref().unwrap_or("tcxo"))?;
                let h = kshana::holdover::holdover_for_class(c, t * 1e-9) / 3600.0;
                (t, Some(h))
            }
        };
        let held_over = holdover_h
            .map(|h| h >= q.denial.duration_h)
            .unwrap_or(false);
        // eLoran for position: the planner at the class target; for timing: the stated
        // timing figure wherever the planner gives a fix (three usable stations).
        let el = if uc.sources.iter().any(|s| s == "eloran") {
            eloran_eval(if uc.need == Need::Position {
                target
            } else {
                1e9
            })?
        } else {
            None
        };
        let mut grid = vec![];
        let mut by_source: BTreeMap<String, usize> = BTreeMap::new();
        for (k, &(lat, lon)) in pts.iter().enumerate() {
            let mut srcs: Vec<(String, f64, f64)> = vec![]; // (source, availability, figure)
            if !denied[k] {
                srcs.push((
                    "gnss".into(),
                    1.0,
                    match uc.need {
                        Need::Position => q.gnss_accuracy95_m,
                        Need::Timing => q.gnss_timing_ns,
                    },
                ));
            } else {
                if held_over {
                    srcs.push(("holdover".into(), 1.0, target));
                }
                if let (Some(v), Some(e)) = (&el, &q.eloran) {
                    let (acc, av) = v[k];
                    match uc.need {
                        Need::Position => {
                            if acc.is_some_and(|x| x <= target) {
                                srcs.push(("eloran".into(), av, acc.unwrap_or(f64::NAN)));
                            }
                        }
                        Need::Timing => {
                            if acc.is_some() && e.timing_ns <= target {
                                srcs.push(("eloran".into(), av, e.timing_ns));
                            }
                        }
                    }
                }
                if let Some(l) = q
                    .leo
                    .as_ref()
                    .filter(|_| uc.sources.iter().any(|s| s == "leo"))
                {
                    let fig = match uc.need {
                        Need::Position => l.accuracy95_m,
                        Need::Timing => l.timing_ns,
                    };
                    let av = leo_in_view(l, lat) * l.availability;
                    if fig <= target && av > 0.0 {
                        srcs.push(("leo".into(), av, fig));
                    }
                }
            }
            let availability = 1.0 - srcs.iter().fold(1.0, |p, s| p * (1.0 - s.1));
            let best = srcs
                .iter()
                .map(|s| s.2)
                .filter(|x| x.is_finite())
                .reduce(f64::min);
            let served = availability >= uc.availability;
            for s in &srcs {
                *by_source.entry(s.0.clone()).or_insert(0) += 1;
            }
            grid.push(Cell {
                lat: round(lat, 4),
                lon: round(lon, 4),
                gnss_denied: denied[k],
                sources_meeting_need: srcs.iter().map(|s| s.0.clone()).collect(),
                availability: round(availability, 6),
                best: best.map(|x| round(x, 1)),
                accuracy_class: match uc.need {
                    Need::Position => pos_class(best),
                    Need::Timing => time_class(best),
                },
                served,
            });
        }
        let n = grid.len().max(1);
        let served_share = grid.iter().filter(|c| c.served).count() as f64 / n as f64;
        let dn = grid.iter().filter(|c| c.gnss_denied).count();
        let dserved = grid.iter().filter(|c| c.gnss_denied && c.served).count();
        let denied_served_share = if dn > 0 {
            dserved as f64 / dn as f64
        } else {
            1.0
        };
        let beyond = holdover_h.map(|h| round((q.denial.duration_h - h).max(0.0), 3));
        let summary = format!(
            "{}: {:.0} % of the area served at availability ≥ {} ({:.0} % of the denied cells){}",
            uc.name,
            100.0 * served_share,
            uc.availability,
            100.0 * denied_served_share,
            match holdover_h {
                Some(h) => format!(
                    "; clock holdover {:.2} h against a {:.1} h denial",
                    h, q.denial.duration_h
                ),
                None => String::new(),
            }
        );
        classes.push(ClassResult {
            class: uc.name.clone(),
            need: uc.need,
            cells: grid.len(),
            served_share: round(served_share, 4),
            denied_cells: dn,
            denied_served_share: round(denied_served_share, 4),
            holdover_h: holdover_h.map(|h| round(h, 4)),
            beyond_holdover_h: beyond,
            by_source,
            grid,
            summary,
        });
    }
    let verdict = format!(
        "GNSS denied for {:.1} h: {} (MODELLED)",
        q.denial.duration_h,
        classes
            .iter()
            .map(|c| format!(
                "{} {:.0} % of denied cells served",
                c.class,
                100.0 * c.denied_served_share
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );
    Ok(Report {
        format: FORMAT,
        evidence: EVIDENCE,
        scope: SCOPE,
        title: q.title.clone(),
        data_source: q.data_source.clone(),
        denial: q.denial.clone(),
        eloran_on_air: on.iter().map(|s| s.id.clone()).collect(),
        eloran_off_air: off.iter().map(|s| s.id.clone()).collect(),
        leo: q.leo.as_ref().map(|l| format!("{} (figures: {})", l.name, l.source_of_figures)),
        classes,
        limits: vec![
            "eLoran availability counts independent station outages only; the planner's groundwave model and noise level are stated assumptions.".into(),
            "LEO coverage is Walker geometry (continuous where streets close, else an expected fraction); the accuracy and timing figures are the provider's, as stated.".into(),
            "Sources are combined as independent: availability 1 − Π(1 − aᵢ) over sources that meet the need; correlated failures are not modelled.".into(),
            "Holdover uses representative clock classes from the open engine; a measured clock's own figures give a defensible number.".into(),
            "A planning tool: it computes no navigation or timing solution and fuses no sources.".into(),
        ],
        verdict,
    })
}

/// HTML page.
pub fn html(r: &Report) -> String {
    crate::transport::page(
        &r.title,
        r.evidence,
        &r.verdict,
        &[
            r.scope.to_string(),
            format!("Data source: {}.", r.data_source),
        ],
        &[
            (
                "User classes".into(),
                vec![
                    "class",
                    "need",
                    "served (area)",
                    "served (denied cells)",
                    "holdover (h)",
                    "beyond holdover (h)",
                    "cells by source",
                ],
                r.classes
                    .iter()
                    .map(|c| {
                        vec![
                            c.class.clone(),
                            format!("{:?}", c.need),
                            format!("{:.0} %", 100.0 * c.served_share),
                            format!("{:.0} %", 100.0 * c.denied_served_share),
                            c.holdover_h.map(|h| format!("{h:.2}")).unwrap_or_default(),
                            c.beyond_holdover_h
                                .map(|h| format!("{h:.2}"))
                                .unwrap_or_default(),
                            c.by_source
                                .iter()
                                .map(|(k, v)| format!("{k} {v}"))
                                .collect::<Vec<_>>()
                                .join(", "),
                        ]
                    })
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                r.limits.iter().map(|l| vec![l.clone()]).collect(),
            ),
        ],
    )
}

/// A SYNTHETIC crisis: GNSS denied over a 2×2 degree area for 48 h; four eLoran stations, one
/// off air; a LEO service with stated figures; three user classes.
pub fn synth() -> String {
    r#"title = "SYNTHETIC crisis PNT availability"
data_source = "SYNTHETIC (kshana-pro crisis-pnt synth)"
gnss_accuracy95_m = 5.0

[area]
lat_min = 55.0
lat_max = 57.0
lon_min = 10.0
lon_max = 12.0
step_deg = 0.5

[denial]
start_utc = "2026-06-01T00:00:00Z"
duration_h = 48.0
polygon = [[54.9, 9.9], [54.9, 11.6], [57.1, 11.6], [57.1, 9.9]]

[eloran]
timing_ns = 50.0
noise_uv_m_20k = 50.0

[[eloran.station]]
id = "S1"
lat = 58.0
lon = 8.0
power_kw = 400.0

[[eloran.station]]
id = "S2"
lat = 54.0
lon = 14.0
power_kw = 400.0

[[eloran.station]]
id = "S3"
lat = 53.5
lon = 8.5
power_kw = 400.0

[[eloran.station]]
id = "S4"
lat = 58.5
lon = 13.5
power_kw = 400.0
on_air = false

[leo]
name = "SYNTHETIC LEO PNT"
altitude_km = 1000.0
inclination_deg = 87.0
planes = 12
sats_per_plane = 20
min_elev_deg = 10.0
accuracy95_m = 20.0
timing_ns = 100.0
availability = 0.99
source_of_figures = "SYNTHETIC stated figures"

[[user_class]]
name = "maritime coastal navigation"
need = "position"
accuracy95_m = 100.0
availability = 0.99
sources = ["eloran", "leo"]

[[user_class]]
name = "telecom timing"
need = "timing"
timing_ns = 1000.0
clock = "rafs"
availability = 0.999
sources = ["eloran"]

[[user_class]]
name = "first-responder handheld"
need = "position"
accuracy95_m = 10.0
availability = 0.95
sources = ["leo"]
"#
    .into()
}
