// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Bridge cross-sensor consistency aid: "is it spoofing?" (research 05 #42/#56, 07 A03/A04,
//! A09/A10, B04, B11, B21, 07b M13).
//!
//! From a recorded bridge log ([`super::nmea`]) the aid runs these checks every epoch, each
//! between named sensors:
//!
//! | Check | Compares | Fails when |
//! |---|---|---|
//! | `heading` | every pair of heading sources (gyro, GNSS compass) | the windowed mean difference exceeds `heading_deg` |
//! | `velocity` | GNSS speed and course with gyro heading and log speeds | the windowed vector difference exceeds `velocity_kn` (plus `water_track_extra_kn` without bottom track) |
//! | `position` | GNSS position with dead reckoning from gyro and log since the last consistent anchor | the gap exceeds `position_floor_m` plus `dr_k` sigmas of the [`super::dr`] budget |
//! | `gnss-pair` | the primary GNSS position with every other GNSS receiver | the windowed distance exceeds `gnss_pair_m` |
//! | `time` | the GNSS time with the bridge cadence (or TAG block time) | the GNSS clock runs at a rate off 1 s/s by more than `time_rate`, or jumps by more than `time_jump_s` |
//! | `depth` | the echo-sounder with the expected depth band of a configured zone at the GNSS position | the windowed median depth leaves the band |
//! | `radar` | radar tracked targets of configured fixed marks with the range and bearing predicted from the GNSS position | the range or bearing residual exceeds its tolerance |
//!
//! Every check reports a ratio (residual over threshold) per epoch; a check fails once the
//! ratio has stayed above 1 for `persist_s`. The epoch's **consistency score** is 100 times
//! the mean over the available checks of `clamp(2 - ratio, 0, 1)`. Consecutive epochs with a
//! failing check form an **alert**: it lists the checks in the order they first failed (a
//! heading check first is the "heading before position" warning) and names the sensor the
//! data implicates under a single-fault rule: the candidates are the sensors common to every
//! failing check at the alert's onset, less those an instantaneous check that holds at that
//! epoch clears (heading, velocity, GNSS pair, time, radar; not the integrating position check
//! or the coarse depth band). When more than one candidate remains,
//! the data cannot separate them; the alert says so, lists them, and names the GNSS-family
//! source first because it is the one exposed to radio interference (the stated prior).
//!
//! A GNSS outage (no valid fix) is reported as such, not as an inconsistency: jamming shows
//! as unavailability, spoofing as disagreement. A check is never run on data that is not
//! there, and the report lists which checks were available.
//!
//! **Advisory.** The aid names a disagreement and the evidence; it gives no position and no
//! instruction. MODELLED (see [`super::LABEL`]).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::dr::DrModel;
use super::nmea::{self, Epoch, ReadSummary, ReaderConfig, Role};
use super::{ang_diff, bearing_deg, distance_m, move_ne, sig9, KN_MPS};

/// The method tag.
pub const METHOD: &str = "kshana-pro/bridge-consistency/v1";
/// The report's format tag.
pub const FORMAT: &str = "kshana-pro-bridge-consistency/1";
/// The report's main file.
pub const RESULT_FILE: &str = "consistency.json";
/// The per-epoch table.
pub const EPOCHS_FILE: &str = "epochs.csv";

/// The check names, in report order.
pub const CHECKS: [&str; 7] = [
    "heading",
    "velocity",
    "position",
    "gnss-pair",
    "time",
    "depth",
    "radar",
];

/// The checks whose holding clears their sensors in the attribution (see `make_alert`).
pub const CLEARING: [&str; 5] = ["heading", "velocity", "gnss-pair", "time", "radar"];

/// Thresholds; every one is written into the report.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Thresholds {
    pub heading_deg: f64,
    pub velocity_kn: f64,
    pub water_track_extra_kn: f64,
    pub position_floor_m: f64,
    pub dr_k: f64,
    /// Re-anchor the dead reckoning on GNSS after this long with every check passing.
    pub dr_reanchor_s: f64,
    pub gnss_pair_m: f64,
    pub time_rate: f64,
    pub time_jump_s: f64,
    pub time_window_s: f64,
    pub radar_range_m: f64,
    pub radar_range_frac: f64,
    pub radar_bearing_deg: f64,
    /// Smoothing window, seconds.
    pub window_s: f64,
    /// A ratio above 1 must last this long to fail a check, seconds.
    pub persist_s: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            heading_deg: 3.0,
            velocity_kn: 1.0,
            water_track_extra_kn: 1.5,
            position_floor_m: 50.0,
            dr_k: 3.0,
            dr_reanchor_s: 300.0,
            gnss_pair_m: 50.0,
            time_rate: 0.02,
            time_jump_s: 2.0,
            time_window_s: 60.0,
            radar_range_m: 50.0,
            radar_range_frac: 0.01,
            radar_bearing_deg: 2.0,
            window_s: 10.0,
            persist_s: 5.0,
        }
    }
}

/// An area with the depth band the chart gives, for the echo-sounder check. The band is
/// the operator's reading of the chart; the aid only tests the sounder against it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DepthZone {
    pub name: String,
    /// Polygon vertices as `[lat_deg, lon_deg]`.
    pub polygon: Vec<[f64; 2]>,
    pub min_m: f64,
    pub max_m: f64,
}

/// A charted fixed mark the radar tracks as a target.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RadarMark {
    pub name: String,
    /// The TTM target number the radar gives it.
    pub target: u32,
    pub lat_deg: f64,
    pub lon_deg: f64,
}

/// The aid's configuration (TOML).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub reader: ReaderConfig,
    pub thresholds: Thresholds,
    /// The DR error budget behind the position check (speed is taken from the log).
    pub dr: DrModel,
    #[serde(rename = "depth_zone")]
    pub depth_zones: Vec<DepthZone>,
    #[serde(rename = "radar_mark")]
    pub radar_marks: Vec<RadarMark>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self, String> {
        let c: Config = toml::from_str(text).map_err(|e| format!("invalid bridge config: {e}"))?;
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), String> {
        let t = &self.thresholds;
        let v = [
            t.heading_deg,
            t.velocity_kn,
            t.position_floor_m,
            t.dr_k,
            t.gnss_pair_m,
            t.time_rate,
            t.time_jump_s,
            t.radar_range_m,
            t.radar_bearing_deg,
        ];
        if v.iter().any(|x| !x.is_finite() || *x <= 0.0) {
            return Err("every threshold must be positive".into());
        }
        if t.window_s < 0.0 || t.persist_s < 0.0 || t.water_track_extra_kn < 0.0 {
            return Err("window_s, persist_s and water_track_extra_kn must not be negative".into());
        }
        self.dr.validate()?;
        for z in &self.depth_zones {
            if z.polygon.len() < 3 || z.min_m > z.max_m {
                return Err(format!(
                    "depth zone '{}' needs three or more vertices and min_m <= max_m",
                    z.name
                ));
            }
        }
        Ok(())
    }
}

/// One check's figures at one epoch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CheckValue {
    pub ratio: f64,
    pub failing: bool,
}

/// One epoch of the result.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EpochResult {
    pub t_s: f64,
    pub gnss_valid: bool,
    /// Consistency score 0 to 100 over the available checks; `None` with none available.
    pub score: Option<f64>,
    pub checks: BTreeMap<String, CheckValue>,
}

/// An alert: consecutive epochs with at least one failing check.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Alert {
    pub start_s: f64,
    pub end_s: f64,
    /// Checks in the order they first failed, with the time.
    pub checks: Vec<(String, f64)>,
    /// The first check to fail.
    pub first_check: String,
    /// The sensor named (see the module notes), as `role:source`.
    pub named: String,
    /// Whether the data alone separates the named sensor from the other candidates.
    pub separable: bool,
    pub candidates: Vec<String>,
    /// Sensors a passing check cleared.
    pub cleared: Vec<String>,
    pub basis: String,
    pub peak_ratio: f64,
    pub min_score: f64,
    /// Cross-checks the crew may make; observations, not instructions.
    pub crew_cross_checks: Vec<String>,
}

/// A GNSS outage of the primary receiver.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Outage {
    pub start_s: f64,
    pub end_s: f64,
}

/// What each check could run on.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Availability {
    pub check: String,
    /// Epochs the check ran on.
    pub epochs: usize,
    /// The sensors it compares.
    pub sensors: Vec<String>,
}

/// The report.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub format: String,
    pub method: String,
    pub label: String,
    pub advisory: String,
    pub config: Config,
    pub input: ReadSummary,
    pub epochs: usize,
    pub duration_s: f64,
    pub availability: Vec<Availability>,
    pub outages: Vec<Outage>,
    pub alerts: Vec<Alert>,
    /// The first time each check failed.
    pub first_failure_s: BTreeMap<String, f64>,
    pub score_min: Option<f64>,
    pub score_mean: Option<f64>,
    #[serde(skip)]
    pub per_epoch: Vec<EpochResult>,
}

/// A sensor and the quantity a check uses from it, as `role:source/quantity`.
fn sensor(role: Role, src: &str, q: &str) -> String {
    format!("{}:{src}/{q}", role.as_str())
}

/// The DR velocity (north, east, m/s) from a heading and the log, and whether it is on
/// the ground track.
fn dr_velocity(e: &Epoch, gyro: Option<(&str, f64)>) -> Option<((f64, f64), bool, String)> {
    let (_, h) = gyro?;
    let (s, c) = h.to_radians().sin_cos();
    if let Some((src, (gl, gt))) = e.ground.iter().next() {
        let (l, t) = (gl * KN_MPS, gt * KN_MPS);
        return Some(((l * c - t * s, l * s + t * c), true, src.clone()));
    }
    let (src, (wl, wt)) = e.water.iter().next()?;
    let (l, t) = (wl * KN_MPS, wt.unwrap_or(0.0) * KN_MPS);
    Some(((l * c - t * s, l * s + t * c), false, src.clone()))
}

fn point_in(poly: &[[f64; 2]], lat: f64, lon: f64) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (yi, xi) = (poly[i][0], poly[i][1]);
        let (yj, xj) = (poly[j][0], poly[j][1]);
        if (yi > lat) != (yj > lat) && lon < (xj - xi) * (lat - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// A moving window of values with their times.
#[derive(Default)]
struct Win<T> {
    q: VecDeque<(f64, T)>,
}

impl<T: Clone> Win<T> {
    fn push(&mut self, t: f64, v: T, span: f64) {
        self.q.push_back((t, v));
        while self.q.front().is_some_and(|(t0, _)| t - t0 > span + 1e-9) {
            self.q.pop_front();
        }
    }
    fn clear(&mut self) {
        self.q.clear();
    }
    fn values(&self) -> impl Iterator<Item = &T> {
        self.q.iter().map(|(_, v)| v)
    }
    fn len(&self) -> usize {
        self.q.len()
    }
}

/// A GNSS ground velocity and a dead-reckoning velocity (north, east, m/s), and whether the
/// latter is on the ground track.
type VelPair = ((f64, f64), (f64, f64), bool);

/// The persistence state of one check.
#[derive(Default)]
struct Persist {
    above_since: Option<f64>,
}

impl Persist {
    fn step(&mut self, t: f64, ratio: f64, persist: f64) -> bool {
        if ratio > 1.0 {
            let s = *self.above_since.get_or_insert(t);
            t - s >= persist - 1e-9
        } else {
            self.above_since = None;
            false
        }
    }
}

/// Run the aid over a log.
pub fn run(bytes: &[u8], cfg: &Config) -> Result<Report, String> {
    cfg.validate()?;
    let (epochs, input) = nmea::read(bytes, &cfg.reader)?;
    let primary = input.primary_gnss.clone().expect("the reader names one");
    let th = &cfg.thresholds;
    let span = th.window_s.max(0.0);

    let mut per_epoch = Vec::with_capacity(epochs.len());
    let mut avail: BTreeMap<&str, (usize, BTreeSet<String>)> = BTreeMap::new();
    let mut persist: BTreeMap<String, Persist> = BTreeMap::new();
    let mut heading_win: BTreeMap<(String, String), Win<f64>> = BTreeMap::new();
    let mut vel_win: Win<VelPair> = Win::default();
    let mut pair_win: BTreeMap<String, Win<f64>> = BTreeMap::new();
    let mut depth_win: Win<f64> = Win::default();
    let mut time_win: Win<f64> = Win::default();
    let mut prev_gnss_t: Option<(f64, f64)> = None;
    // Dead reckoning: anchor time, and the DR position, kept while GNSS is out or suspect.
    let mut dr: Option<(f64, f64, f64)> = None; // (anchor t, lat, lon)
    let mut dr_pos: Option<(f64, f64)> = None;
    let mut last_t: Option<f64> = None;
    let mut clean_since: Option<f64> = None;
    let mut sensors_at: Vec<BTreeMap<String, Vec<String>>> = Vec::with_capacity(epochs.len());
    let mut sensors_of: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for e in &epochs {
        let t = e.t_s;
        let dt = last_t.map(|l| (t - l).max(0.0)).unwrap_or(0.0);
        last_t = Some(t);
        let g = e.gnss.get(&primary);
        let gnss_ok = g.is_some_and(|g| g.valid && g.lat_deg.is_some() && g.lon_deg.is_some());
        let mut ratios: BTreeMap<String, f64> = BTreeMap::new();
        let mut note = |name: &str, sensors: Vec<String>, ratio: f64| {
            let a = avail.entry(CHECKS.iter().find(|c| **c == name).copied().unwrap_or("?"));
            let a = a.or_default();
            a.0 += 1;
            a.1.extend(sensors.iter().cloned());
            sensors_of.insert(name.to_string(), sensors);
            ratios.insert(name.to_string(), sig9(ratio));
        };

        // The gyro: the first heading source with the gyro role.
        let gyro = e
            .heading
            .iter()
            .find(|(_, (r, _))| *r == Role::Gyro)
            .map(|(s, (_, h))| (s.as_str(), *h));

        // ---- heading: every pair of heading sources ----
        let hs: Vec<(&String, &(Role, f64))> = e.heading.iter().collect();
        let mut worst: Option<(f64, Vec<String>)> = None;
        for i in 0..hs.len() {
            for j in i + 1..hs.len() {
                let key = (hs[i].0.clone(), hs[j].0.clone());
                let w = heading_win.entry(key).or_default();
                w.push(t, ang_diff(hs[i].1 .1, hs[j].1 .1), span);
                let mean = w.values().sum::<f64>() / w.len() as f64;
                let r = mean.abs() / th.heading_deg;
                if worst.as_ref().is_none_or(|(x, _)| r > *x) {
                    worst = Some((
                        r,
                        vec![
                            sensor(hs[i].1 .0, hs[i].0, "heading"),
                            sensor(hs[j].1 .0, hs[j].0, "heading"),
                        ],
                    ));
                }
            }
        }
        if let Some((r, s)) = worst {
            note("heading", s, r);
        }

        // ---- velocity: GNSS ground velocity against gyro and log ----
        let drv = dr_velocity(e, gyro);
        match (gnss_ok, g.and_then(|g| g.sog_kn.zip(g.cog_deg)), &drv) {
            (true, Some((sog, cog)), Some((v, ground, log_src))) => {
                let (s, c) = cog.to_radians().sin_cos();
                let gv = (sog * KN_MPS * c, sog * KN_MPS * s);
                vel_win.push(t, (gv, *v, *ground), span);
                let n = vel_win.len() as f64;
                let (mut dn, mut de) = (0.0, 0.0);
                let mut all_ground = true;
                for (a, b, gr) in vel_win.values() {
                    dn += a.0 - b.0;
                    de += a.1 - b.1;
                    all_ground &= *gr;
                }
                let res_kn = (dn / n).hypot(de / n) / KN_MPS;
                let tol = th.velocity_kn
                    + if all_ground {
                        0.0
                    } else {
                        th.water_track_extra_kn
                    };
                note(
                    "velocity",
                    vec![
                        sensor(Role::Gnss, &primary, "velocity"),
                        sensor(Role::Gyro, gyro.map(|g| g.0).unwrap_or("?"), "heading"),
                        sensor(Role::Log, log_src, "speed"),
                    ],
                    res_kn / tol,
                );
            }
            _ => vel_win.clear(),
        }

        // ---- position: GNSS against dead reckoning from the last consistent anchor ----
        if let (Some((v, ground, log_src)), Some(p)) = (&drv, dr_pos) {
            let (la, lo) = move_ne(p.0, p.1, v.0 * dt, v.1 * dt);
            dr_pos = Some((la, lo));
            let _ = ground;
            if gnss_ok {
                let (gl, go) = (
                    g.and_then(|g| g.lat_deg).unwrap_or(0.0),
                    g.and_then(|g| g.lon_deg).unwrap_or(0.0),
                );
                let gap = distance_m(gl, go, la, lo);
                let mut model = cfg.dr.clone();
                model.speed_kn = v.0.hypot(v.1) / KN_MPS;
                model.bottom_track = Some(*ground);
                let (sa, sc) = model.sigmas(t - dr.map(|d| d.0).unwrap_or(t));
                let tol = th.position_floor_m + th.dr_k * sa.hypot(sc);
                note(
                    "position",
                    vec![
                        sensor(Role::Gnss, &primary, "position"),
                        sensor(Role::Gyro, gyro.map(|g| g.0).unwrap_or("?"), "heading"),
                        sensor(Role::Log, log_src, "speed"),
                    ],
                    gap / tol,
                );
            }
        }

        // ---- gnss-pair: the primary against each other receiver ----
        if gnss_ok {
            let (pl, po) = (g.unwrap().lat_deg.unwrap(), g.unwrap().lon_deg.unwrap());
            let mut worst: Option<(f64, String)> = None;
            for (src, o) in &e.gnss {
                if *src == primary || !o.valid {
                    continue;
                }
                if let (Some(la), Some(lo)) = (o.lat_deg, o.lon_deg) {
                    let w = pair_win.entry(src.clone()).or_default();
                    w.push(t, distance_m(pl, po, la, lo), span);
                    let r = (w.values().sum::<f64>() / w.len() as f64) / th.gnss_pair_m;
                    if worst.as_ref().is_none_or(|(x, _)| r > *x) {
                        worst = Some((r, src.clone()));
                    }
                }
            }
            if let Some((r, src)) = worst {
                note(
                    "gnss-pair",
                    vec![
                        sensor(Role::Gnss, &primary, "position"),
                        sensor(Role::Gnss, &src, "position"),
                    ],
                    r,
                );
            }
        }

        // ---- time: GNSS clock against the bridge cadence ----
        if let Some(sod) = g.filter(|g| g.valid).and_then(|g| g.sod) {
            let mut ratio: f64 = 0.0;
            if let Some((pt, ps)) = prev_gnss_t {
                let mut d = sod - ps;
                if d < -43_200.0 {
                    d += 86_400.0;
                }
                let jump = (d - (t - pt)).abs();
                ratio = ratio.max(jump / th.time_jump_s);
            }
            prev_gnss_t = Some((t, sod));
            // Unwrapped GNSS time minus bridge time, over the window: its slope is the rate.
            let unwrapped = {
                let last = time_win.q.back().map(|(tt, off)| (*tt, *off));
                match last {
                    Some((tt, off)) => {
                        let prev_sod = (tt + off).rem_euclid(86_400.0);
                        let mut d = sod - prev_sod;
                        if d < -43_200.0 {
                            d += 86_400.0;
                        }
                        off + d - (t - tt)
                    }
                    None => sod - t,
                }
            };
            time_win.push(t, unwrapped, th.time_window_s);
            if let (Some((t0, o0)), Some((t1, o1))) = (time_win.q.front(), time_win.q.back()) {
                if t1 - t0 >= 0.5 * th.time_window_s && t1 > t0 {
                    let rate = (o1 - o0) / (t1 - t0);
                    ratio = ratio.max(rate.abs() / th.time_rate);
                }
            }
            note(
                "time",
                vec![
                    sensor(Role::Gnss, &primary, "time"),
                    if input.tag_timed {
                        "bridge-clock:tag-block".to_string()
                    } else {
                        "bridge-clock:cadence".to_string()
                    },
                ],
                ratio,
            );
        } else {
            prev_gnss_t = None;
            time_win.clear();
        }

        // ---- depth: echo-sounder against the zone band at the GNSS position ----
        if let (true, Some((src, d))) = (gnss_ok, e.depth_m.iter().next()) {
            let (pl, po) = (g.unwrap().lat_deg.unwrap(), g.unwrap().lon_deg.unwrap());
            if let Some(z) = cfg
                .depth_zones
                .iter()
                .find(|z| point_in(&z.polygon, pl, po))
            {
                depth_win.push(t, *d, span);
                let mut v: Vec<f64> = depth_win.values().copied().collect();
                v.sort_by(|a, b| a.total_cmp(b));
                let med = v[v.len() / 2];
                let half = ((z.max_m - z.min_m) / 2.0).max(1.0);
                let out = if med < z.min_m {
                    z.min_m - med
                } else if med > z.max_m {
                    med - z.max_m
                } else {
                    0.0
                };
                note(
                    "depth",
                    vec![
                        sensor(Role::Gnss, &primary, "position"),
                        sensor(Role::EchoSounder, src, "depth"),
                    ],
                    if out == 0.0 { 0.0 } else { 1.0 + out / half },
                );
            } else {
                depth_win.clear();
            }
        }

        // ---- radar: tracked fixed marks against the GNSS position ----
        if gnss_ok && !cfg.radar_marks.is_empty() {
            let (pl, po) = (g.unwrap().lat_deg.unwrap(), g.unwrap().lon_deg.unwrap());
            let mut worst: Option<(f64, String, bool)> = None;
            for tg in &e.targets {
                let Some(m) = cfg.radar_marks.iter().find(|m| m.target == tg.number) else {
                    continue;
                };
                let brg = if tg.relative {
                    match gyro {
                        Some((_, h)) => (tg.bearing_deg + h).rem_euclid(360.0),
                        None => continue,
                    }
                } else {
                    tg.bearing_deg
                };
                let pr = distance_m(pl, po, m.lat_deg, m.lon_deg);
                let pb = bearing_deg(pl, po, m.lat_deg, m.lon_deg);
                let rr = (tg.range_m - pr).abs() / (th.radar_range_m + th.radar_range_frac * pr);
                let rb = ang_diff(brg, pb).abs() / th.radar_bearing_deg;
                let r = rr.max(rb);
                if worst.as_ref().is_none_or(|(x, _, _)| r > *x) {
                    worst = Some((r, tg.source.clone(), tg.relative));
                }
            }
            if let Some((r, src, rel)) = worst {
                let mut s = vec![
                    sensor(Role::Gnss, &primary, "position"),
                    sensor(Role::Radar, &src, "range-bearing"),
                ];
                if rel {
                    s.push(sensor(
                        Role::Gyro,
                        gyro.map(|g| g.0).unwrap_or("?"),
                        "heading",
                    ));
                }
                note("radar", s, r);
            }
        }

        // ---- persistence, score ----
        let mut checks = BTreeMap::new();
        for (name, r) in &ratios {
            let f = persist
                .entry(name.clone())
                .or_default()
                .step(t, *r, th.persist_s);
            checks.insert(
                name.clone(),
                CheckValue {
                    ratio: *r,
                    failing: f,
                },
            );
        }
        for name in CHECKS {
            if !ratios.contains_key(name) {
                if let Some(p) = persist.get_mut(name) {
                    p.above_since = None;
                }
            }
        }
        let score = (!ratios.is_empty()).then(|| {
            sig9(
                100.0
                    * ratios
                        .values()
                        .map(|r| (2.0 - r).clamp(0.0, 1.0))
                        .sum::<f64>()
                    / ratios.len() as f64,
            )
        });

        // ---- the DR anchor: set on GNSS while every check passes; held otherwise ----
        let suspect = ratios.values().any(|r| *r > 1.0);
        if gnss_ok && !suspect {
            let since = *clean_since.get_or_insert(t);
            let due = dr.is_none_or(|d| t - d.0 >= th.dr_reanchor_s && t - since >= th.persist_s);
            if due {
                let (la, lo) = (g.unwrap().lat_deg.unwrap(), g.unwrap().lon_deg.unwrap());
                dr = Some((t, la, lo));
                dr_pos = Some((la, lo));
            }
        } else {
            clean_since = None;
        }
        if drv.is_none() && !gnss_ok {
            // Nothing to dead-reckon with and nothing to anchor on.
            dr = None;
            dr_pos = None;
        }

        per_epoch.push(EpochResult {
            t_s: t,
            gnss_valid: gnss_ok,
            score,
            checks,
        });
        sensors_at.push(std::mem::take(&mut sensors_of));
    }

    // ---- outages, alerts ----
    let mut outages: Vec<Outage> = Vec::new();
    for r in &per_epoch {
        if !r.gnss_valid {
            match outages.last_mut() {
                Some(o) if (r.t_s - o.end_s) <= 1.5 * step(&per_epoch) => o.end_s = r.t_s,
                _ => outages.push(Outage {
                    start_s: r.t_s,
                    end_s: r.t_s,
                }),
            }
        }
    }
    let mut alerts: Vec<Alert> = Vec::new();
    let mut i = 0;
    while i < per_epoch.len() {
        if !per_epoch[i].checks.values().any(|c| c.failing) {
            i += 1;
            continue;
        }
        let start = i;
        while i < per_epoch.len() && per_epoch[i].checks.values().any(|c| c.failing) {
            i += 1;
        }
        alerts.push(make_alert(&per_epoch[start..i], &sensors_at[start..i]));
    }
    let mut first_failure_s = BTreeMap::new();
    for r in &per_epoch {
        for (n, c) in &r.checks {
            if c.failing {
                first_failure_s.entry(n.clone()).or_insert(r.t_s);
            }
        }
    }
    let scores: Vec<f64> = per_epoch.iter().filter_map(|r| r.score).collect();
    let availability = CHECKS
        .iter()
        .map(|c| {
            let (n, s) = avail.get(c).cloned().unwrap_or_default();
            Availability {
                check: c.to_string(),
                epochs: n,
                sensors: s.into_iter().collect(),
            }
        })
        .collect();
    Ok(Report {
        format: FORMAT.into(),
        method: METHOD.into(),
        label: super::LABEL.into(),
        advisory: super::ADVISORY.into(),
        config: cfg.clone(),
        input,
        epochs: per_epoch.len(),
        duration_s: per_epoch.last().map(|r| r.t_s).unwrap_or(0.0),
        availability,
        outages,
        alerts,
        first_failure_s,
        score_min: scores.iter().copied().reduce(f64::min),
        score_mean: (!scores.is_empty())
            .then(|| sig9(scores.iter().sum::<f64>() / scores.len() as f64)),
        per_epoch,
    })
}

fn step(r: &[EpochResult]) -> f64 {
    if r.len() < 2 {
        return 1.0;
    }
    ((r[r.len() - 1].t_s - r[0].t_s) / (r.len() - 1) as f64).max(1e-6)
}

fn make_alert(run: &[EpochResult], sensors: &[BTreeMap<String, Vec<String>>]) -> Alert {
    let mut order: Vec<(String, f64)> = Vec::new();
    let mut peak: f64 = 0.0;
    let mut min_score: f64 = 100.0;
    for r in run {
        for (n, c) in &r.checks {
            if c.failing {
                peak = peak.max(c.ratio);
                if !order.iter().any(|(m, _)| m == n) {
                    order.push((n.clone(), r.t_s));
                }
            }
        }
        if let Some(s) = r.score {
            min_score = min_score.min(s);
        }
    }
    let failing: BTreeSet<&String> = order.iter().map(|(n, _)| n).collect();
    // Attribution at the onset epoch, before a second effect can join: the sensor-quantities
    // common to every failing check, less those of the checks that held at that epoch.
    let on = &run[0];
    let on_s = &sensors[0];
    let mut cand: Option<BTreeSet<String>> = None;
    for (n, c) in &on.checks {
        if c.failing {
            let s: BTreeSet<String> = on_s
                .get(n)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();
            cand = Some(match cand {
                None => s,
                Some(c) => c.intersection(&s).cloned().collect(),
            });
        }
    }
    let common = cand.unwrap_or_default();
    // Only a check that responds at once to an error in its sensors clears them: the
    // position check integrates (a heading or speed error shows there late) and the depth
    // band is coarse, so holding says little.
    let mut cleared = BTreeSet::new();
    for (n, c) in &on.checks {
        if c.ratio <= 1.0 && CLEARING.contains(&n.as_str()) {
            cleared.extend(on_s.get(n).cloned().unwrap_or_default());
        }
    }
    let mut cand: BTreeSet<String> = common.difference(&cleared).cloned().collect();
    if cand.is_empty() {
        // The holding checks clear every common sensor: more than one effect at once.
        cand = common;
        cleared.clear();
    }
    // A bridge clock is a reference, never the suspect while a sensor remains.
    if cand.len() > 1 {
        cand.retain(|s| !s.starts_with("bridge-clock:"));
    }
    let mut cands: Vec<String> = cand.into_iter().collect();
    // GNSS-family first (the stated prior), then by name.
    cands.sort_by_key(|s| {
        (
            !(s.starts_with("gnss:") || s.starts_with("gnss-compass:")),
            s.clone(),
        )
    });
    let separable = cands.len() == 1;
    let named = cands
        .first()
        .cloned()
        .unwrap_or_else(|| "undetermined".into());
    let basis = if cands.is_empty() {
        "The failing checks share no sensor: more than one source disagrees, or the \
         disagreement is between references; no single sensor is named."
            .to_string()
    } else if separable {
        format!(
            "Single-fault reading at onset: {named} is the only sensor quantity common to every \
             failing check that no holding check clears."
        )
    } else {
        format!(
            "At onset the data cannot separate {}: each is common to every failing check. \
             {named} is named first because GNSS-family sources are the ones exposed to radio \
             interference (the stated prior), not because the data proves it.",
            cands.join(", ")
        )
    };
    let mut crew = Vec::new();
    let has = |n: &str| failing.iter().any(|f| f.as_str() == n);
    if has("velocity") || has("position") || has("radar") || has("depth") || has("gnss-pair") {
        crew.push(
            "Compare the displayed position with radar ranges and bearings of charted marks \
             and with visual bearings."
                .to_string(),
        );
        crew.push("Compare the echo-sounder depth with the charted depth.".to_string());
    }
    if has("heading") {
        crew.push(
            "Compare the heading sources with each other and with the magnetic compass."
                .to_string(),
        );
    }
    if has("time") {
        crew.push(
            "Compare the GNSS time with the ship's clock and with another time source.".to_string(),
        );
    }
    crew.push("Record the times and readings in the log book.".to_string());
    Alert {
        start_s: run[0].t_s,
        end_s: run[run.len() - 1].t_s,
        first_check: order[0].0.clone(),
        checks: order,
        named,
        separable,
        candidates: cands,
        cleared: cleared.into_iter().collect(),
        basis,
        peak_ratio: sig9(peak),
        min_score: sig9(min_score),
        crew_cross_checks: crew,
    }
}

/// The per-epoch table as CSV.
pub fn epochs_csv(r: &Report) -> String {
    let mut s = String::from("t_s,gnss_valid,score");
    for c in CHECKS {
        s.push_str(&format!(",{c}_ratio,{c}_failing"));
    }
    s.push('\n');
    for e in &r.per_epoch {
        s.push_str(&format!(
            "{},{},{}",
            e.t_s,
            e.gnss_valid as u8,
            e.score.map(|x| x.to_string()).unwrap_or_default()
        ));
        for c in CHECKS {
            match e.checks.get(c) {
                Some(v) => s.push_str(&format!(",{},{}", v.ratio, v.failing as u8)),
                None => s.push_str(",,"),
            }
        }
        s.push('\n');
    }
    s
}

/// A short text summary for the watch.
pub fn summary_text(r: &Report) -> String {
    let mut s = format!(
        "Bridge consistency aid ({}). {}\n{}\n\n{} epochs over {:.0} s; mean score {}; lowest {}.\n",
        r.method,
        r.advisory,
        r.label,
        r.epochs,
        r.duration_s,
        r.score_mean.map(|x| format!("{x:.1}")).unwrap_or("-".into()),
        r.score_min.map(|x| format!("{x:.1}")).unwrap_or("-".into()),
    );
    s.push_str("Checks available: ");
    let av: Vec<String> = r
        .availability
        .iter()
        .filter(|a| a.epochs > 0)
        .map(|a| format!("{} ({} epochs)", a.check, a.epochs))
        .collect();
    s.push_str(&if av.is_empty() {
        "none".into()
    } else {
        av.join(", ")
    });
    s.push('\n');
    for o in &r.outages {
        s.push_str(&format!(
            "GNSS unavailable {:.0}-{:.0} s (no valid fix: an outage, not a disagreement).\n",
            o.start_s, o.end_s
        ));
    }
    if r.alerts.is_empty() {
        s.push_str("No disagreement between sensors persisted.\n");
    }
    for a in &r.alerts {
        s.push_str(&format!(
            "ALERT {:.0}-{:.0} s: first {}; checks {}; sensor named {}{}.\n  {}\n",
            a.start_s,
            a.end_s,
            a.first_check,
            a.checks
                .iter()
                .map(|(n, t)| format!("{n}@{t:.0}s"))
                .collect::<Vec<_>>()
                .join(", "),
            a.named,
            if a.separable { "" } else { " (not separable)" },
            a.basis
        ));
        for c in &a.crew_cross_checks {
            s.push_str(&format!("  - {c}\n"));
        }
    }
    s
}

/// Write `consistency.json`, `epochs.csv` and `summary.txt` into `dir`.
pub fn write_dir(dir: &std::path::Path, r: &Report) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let w = |name: &str, text: String| {
        std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))
    };
    w(
        RESULT_FILE,
        serde_json::to_string_pretty(r).map_err(|e| e.to_string())? + "\n",
    )?;
    w(EPOCHS_FILE, epochs_csv(r))?;
    w("summary.txt", summary_text(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_in_polygon() {
        let sq = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
        assert!(point_in(&sq, 0.5, 0.5));
        assert!(!point_in(&sq, 1.5, 0.5));
    }

    #[test]
    fn config_defaults_parse_and_bad_ones_fail() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        assert!(Config::parse("[thresholds]\nheading_deg = 0").is_err());
        assert!(Config::parse("[thresholds]\nunknown = 1").is_err());
    }
}
