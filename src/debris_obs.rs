// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Observation scheduling for space-debris sensors (evidence pack, planning only)
//!
//! A sensor operator with a target list wants to know when each object can be observed, which
//! passes to take given priorities and the sensor's limits, and what each scheduled pass is
//! expected to do for the object's orbit knowledge. From the sensor (location, elevation and
//! azimuth limits, maximum range, optical or radar, track and slew times, measurement noise) and
//! the targets (TLE, or a CCSDS OEM ephemeris), with the open engine's tools:
//!
//! * **visibility windows**: each target propagated (SGP4 for a TLE through
//!   [`kshana::sgp4`]; linear interpolation between OEM states parsed with
//!   [`kshana::oem::parse_oem`]), carried TEME→ECEF by sidereal time
//!   ([`kshana::frames::teme_to_ecef`]) and looked at from the sensor
//!   ([`kshana::frames::look_angles`]); for an optical sensor also the sensor in darkness (Sun
//!   below the stated depression) and the target sunlit (outside the Earth's cylindrical shadow),
//!   with the Sun from [`kshana::ephem::sun_position`];
//! * **a scheduled plan**: targets taken in priority order (then window start); each pass is a
//!   track of `track_s` placed at the earliest time inside a window that clears every already
//!   scheduled track by the slew time; at most `max_passes_per_target` per target;
//! * **expected covariance improvement per scheduled pass**: the Fisher information of the pass's
//!   measurements, `Σ Φᵀ Hᵀ W H Φ`, from the open engine's analytic measurement partials
//!   ([`kshana::orbit_determination::measurement_with_partials`]) and the variational
//!   state-transition matrix ([`kshana::orbit_determination::propagate_with_stm`], two-body +
//!   J2), added to the stated prior; the position 1-sigma before and after. Each pass is
//!   evaluated against the stated prior at its own epoch (not chained).
//!
//! **Planning only**: no observation is taken and no orbit is determined from data. MODELLED.

use kshana::frames::{look_angles, teme_to_ecef, Geodetic};
use kshana::orbit_determination::{
    measurement_with_partials, propagate_with_stm, MeasurementType, Station,
};
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc, round};

/// Pro feature.
pub const FEATURE: &str = "pro-debris-obs-schedule";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-debris-obs-schedule/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: SGP4 or OEM-interpolated geometry, sidereal-time TEME to \
Earth-fixed rotation without polar motion or refraction, a greedy priority scheduler, and the \
Fisher information of the stated measurement noise on a two-body + J2 state-transition matrix. \
Planning only: no observation is taken and no orbit is determined from data.";

/// Sensor kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorKind {
    Optical,
    Radar,
}

/// The sensor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sensor {
    pub name: String,
    pub kind: SensorKind,
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub alt_m: f64,
    pub min_elev_deg: f64,
    /// Azimuth limits (deg, clockwise from north; `az_min > az_max` wraps through north).
    #[serde(default)]
    pub az_min_deg: Option<f64>,
    #[serde(default)]
    pub az_max_deg: Option<f64>,
    #[serde(default)]
    pub max_range_km: Option<f64>,
    /// Optical: Sun depression below the horizon needed at the sensor (deg).
    #[serde(default = "d_dark")]
    pub sun_depression_deg: f64,
    /// Track length per scheduled pass, slew and settle time between tracks (s).
    pub track_s: f64,
    pub slew_s: f64,
    /// Measurement interval within a track (s).
    #[serde(default = "d_meas")]
    pub meas_interval_s: f64,
    /// Angle noise (arcsec, both axes) and, for radar, range noise (m).
    pub sigma_angle_arcsec: f64,
    #[serde(default)]
    pub sigma_range_m: Option<f64>,
}

fn d_dark() -> f64 {
    12.0
}
fn d_meas() -> f64 {
    10.0
}

/// A target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub name: String,
    /// 1 = highest.
    pub priority: u32,
    #[serde(default)]
    pub tle_line1: Option<String>,
    #[serde(default)]
    pub tle_line2: Option<String>,
    /// A CCSDS OEM file and the segment's OBJECT_NAME.
    #[serde(default)]
    pub oem: Option<String>,
    #[serde(default)]
    pub oem_object: Option<String>,
    /// Prior 1-sigma of position (m) and velocity (m/s), per axis.
    pub prior_sigma_pos_m: f64,
    pub prior_sigma_vel_mps: f64,
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub data_source: String,
    pub window_start_utc: String,
    pub window_hours: f64,
    #[serde(default = "d_step")]
    pub step_s: f64,
    #[serde(default = "d_max")]
    pub max_passes_per_target: usize,
    pub sensor: Sensor,
    #[serde(rename = "target")]
    pub targets: Vec<Target>,
}

fn d_step() -> f64 {
    20.0
}
fn d_max() -> usize {
    2
}

/// A visibility window.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Window {
    pub target: String,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    pub max_elev_deg: f64,
    pub min_range_km: f64,
    #[serde(skip)]
    t0: f64,
    #[serde(skip)]
    t1: f64,
}

/// A scheduled track.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Track {
    pub target: String,
    pub priority: u32,
    pub start_utc: String,
    pub end_utc: String,
    pub measurements: usize,
    pub prior_pos_sigma_m: f64,
    pub posterior_pos_sigma_m: f64,
    /// Prior over posterior position sigma.
    pub improvement_factor: f64,
}

/// A target's summary.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TargetSummary {
    pub target: String,
    pub priority: u32,
    pub windows: usize,
    pub scheduled: usize,
    pub reason_unscheduled: Option<String>,
}

/// The pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Plan {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub data_source: String,
    pub sensor: Sensor,
    pub window_start_utc: String,
    pub window_hours: f64,
    pub windows: Vec<Window>,
    pub schedule: Vec<Track>,
    pub targets: Vec<TargetSummary>,
    pub limits: Vec<String>,
    pub verdict: String,
}

enum Orbit {
    Sgp4(Box<kshana::sgp4::Sgp4>),
    /// (unix s, position m, velocity m/s), sorted.
    Table(Vec<(f64, [f64; 3], [f64; 3])>),
}

impl Orbit {
    /// TEME position and velocity (m, m/s) at unix time `t`.
    fn state(&self, t: f64) -> Option<([f64; 3], [f64; 3])> {
        match self {
            Orbit::Sgp4(s) => {
                let jd = t / 86_400.0 + 2_440_587.5;
                let (r, v) = s.propagate((jd - s.epoch_jd_utc()) * 1440.0).ok()?;
                Some((r.map(|x| x * 1e3), v.map(|x| x * 1e3)))
            }
            Orbit::Table(v) => {
                let i = v.partition_point(|x| x.0 <= t);
                let (a, b) = (v.get(i.checked_sub(1)?)?, v.get(i)?);
                let f = (t - a.0) / (b.0 - a.0);
                let l = |x: [f64; 3], y: [f64; 3]| [0, 1, 2].map(|k| x[k] + f * (y[k] - x[k]));
                Some((l(a.1, b.1), l(a.2, b.2)))
            }
        }
    }
}

fn jd(t: f64) -> f64 {
    t / 86_400.0 + 2_440_587.5
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// Earth-fixed to TEME by sidereal time (the inverse of `teme_to_ecef`).
fn ecef_to_teme(r: [f64; 3], jd_ut1: f64) -> [f64; 3] {
    let th = kshana::sgp4::gstime(jd_ut1);
    let (s, c) = th.sin_cos();
    [c * r[0] - s * r[1], s * r[0] + c * r[1], r[2]]
}

fn sun_teme(t: f64) -> [f64; 3] {
    // TT ≈ UTC + 69.184 s; the mean-of-date Sun is taken as TEME (sub-degree difference).
    kshana::ephem::sun_position((jd(t) + 69.184 / 86_400.0 - 2_451_545.0) / 36_525.0)
}

fn sunlit(r: [f64; 3], sun: [f64; 3]) -> bool {
    let u = sun.map(|x| x / norm(sun));
    let p = dot(r, u);
    if p > 0.0 {
        return true;
    }
    let perp = norm(sub(r, u.map(|x| x * p)));
    perp > 6_378_137.0
}

fn az_ok(az: f64, s: &Sensor) -> bool {
    match (s.az_min_deg, s.az_max_deg) {
        (Some(a), Some(b)) if a <= b => az >= a && az <= b,
        (Some(a), Some(b)) => az >= a || az <= b,
        _ => true,
    }
}

fn inv6(m: [[f64; 6]; 6]) -> Option<[[f64; 6]; 6]> {
    let mut a = [[0.0; 12]; 6];
    for i in 0..6 {
        a[i][..6].copy_from_slice(&m[i]);
        a[i][6 + i] = 1.0;
    }
    for c in 0..6 {
        let p = (c..6).max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        let d = a[c][c];
        for v in a[c].iter_mut() {
            *v /= d;
        }
        for r in 0..6 {
            if r != c {
                let f = a[r][c];
                if f != 0.0 {
                    let row = a[c];
                    for (k, v) in a[r].iter_mut().enumerate() {
                        *v -= f * row[k];
                    }
                }
            }
        }
    }
    let mut out = [[0.0; 6]; 6];
    for i in 0..6 {
        out[i].copy_from_slice(&a[i][6..]);
    }
    Some(out)
}

fn pos_sigma(p: &[[f64; 6]; 6]) -> f64 {
    (p[0][0] + p[1][1] + p[2][2]).max(0.0).sqrt()
}

fn load_orbit(t: &Target, base: &std::path::Path) -> Result<Orbit, String> {
    match (&t.tle_line1, &t.tle_line2, &t.oem) {
        (Some(a), Some(b), None) => {
            let tle = kshana::tle::parse_tle(a, b).map_err(|e| format!("{}: {e}", t.name))?;
            Ok(Orbit::Sgp4(Box::new(
                tle.to_sgp4(kshana::sgp4::wgs72(), false),
            )))
        }
        (None, None, Some(f)) => {
            let text = crate::timing::grid::read_text(base, f)?;
            let o = kshana::oem::parse_oem(&text).map_err(|e| format!("{f}: {e}"))?;
            let seg = o
                .segments
                .iter()
                .find(|s| {
                    t.oem_object
                        .as_deref()
                        .is_none_or(|n| s.meta.object_name == n)
                })
                .ok_or_else(|| format!("{f}: no segment for {}", t.name))?;
            if !seg.meta.ref_frame.eq_ignore_ascii_case("TEME") {
                return Err(format!(
                    "{f}: REF_FRAME {} (only TEME is supported here)",
                    seg.meta.ref_frame
                ));
            }
            // Epochs are taken as UTC (they are compared with UTC windows and SGP4 targets on
            // UTC); another TIME_SYSTEM would shift every pass by its offset from UTC.
            if !seg.meta.time_system.trim().eq_ignore_ascii_case("UTC") {
                return Err(format!(
                    "{f}: TIME_SYSTEM {} (only UTC is supported here; convert the epochs to UTC first)",
                    seg.meta.time_system
                ));
            }
            let mut v: Vec<(f64, [f64; 3], [f64; 3])> = seg
                .states
                .iter()
                .map(|s| {
                    let e = &s.epoch;
                    let j = kshana::timescales::julian_date(
                        e.year, e.month, e.day, e.hour, e.minute, e.second,
                    );
                    (
                        (j - 2_440_587.5) * 86_400.0,
                        s.pos_km.map(|x| x * 1e3),
                        s.vel_km_s.map(|x| x * 1e3),
                    )
                })
                .collect();
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
            if v.len() < 2 {
                return Err(format!("{f}: fewer than two states"));
            }
            Ok(Orbit::Table(v))
        }
        _ => Err(format!(
            "target {}: give tle_line1 and tle_line2, or oem",
            t.name
        )),
    }
}

/// Build the plan.
pub fn plan(q: &Request, base: &std::path::Path) -> Result<Plan, String> {
    let s = &q.sensor;
    if q.data_source.trim().is_empty() {
        return Err("data_source must be stated".into());
    }
    for (n, v) in [
        ("window_hours", q.window_hours),
        ("step_s", q.step_s),
        ("sensor.track_s", s.track_s),
        ("sensor.meas_interval_s", s.meas_interval_s),
        ("sensor.sigma_angle_arcsec", s.sigma_angle_arcsec),
    ] {
        if v.is_nan() || v <= 0.0 {
            return Err(format!("{n} must be positive"));
        }
    }
    if s.kind == SensorKind::Radar && s.sigma_range_m.is_none() {
        return Err("a radar sensor needs sigma_range_m".into());
    }
    let t0 = parse_utc(&q.window_start_utc).ok_or("window_start_utc is not a UTC time")?;
    let t_end = t0 + q.window_hours * 3600.0;
    let site = Geodetic {
        lat_rad: s.lat_deg.to_radians(),
        lon_rad: s.lon_deg.to_radians(),
        alt_m: s.alt_m,
    };
    let site_ecef = kshana::frames::geodetic_to_ecef(site);
    let mut orbits = vec![];
    for t in &q.targets {
        orbits.push(load_orbit(t, base)?);
    }

    // Visibility windows.
    let mut windows: Vec<Window> = vec![];
    for (ti, t) in q.targets.iter().enumerate() {
        let mut cur: Option<(f64, f64, f64, f64)> = None; // start, last, max el, min range
        let mut tt = t0;
        while tt <= t_end + 1e-9 {
            let ok = orbits[ti].state(tt).and_then(|(r, _)| {
                let la = look_angles(site, teme_to_ecef(r, jd(tt)));
                let (el, az, rng) = (la.el_rad.to_degrees(), la.az_rad.to_degrees(), la.range_m);
                let mut vis = el >= s.min_elev_deg
                    && az_ok(az, s)
                    && s.max_range_km.is_none_or(|m| rng <= m * 1e3);
                if vis && s.kind == SensorKind::Optical {
                    let sun = sun_teme(tt);
                    let sun_el = look_angles(site, teme_to_ecef(sun, jd(tt)))
                        .el_rad
                        .to_degrees();
                    vis = sun_el <= -s.sun_depression_deg && sunlit(r, sun);
                }
                vis.then_some((el, rng))
            });
            match (ok, cur.as_mut()) {
                (Some((el, rng)), Some(c)) => {
                    c.1 = tt;
                    c.2 = c.2.max(el);
                    c.3 = c.3.min(rng);
                }
                (Some((el, rng)), None) => cur = Some((tt, tt, el, rng)),
                (None, Some(_)) => {
                    let c = cur.take().unwrap_or_default();
                    windows.push(mk_window(&t.name, c));
                }
                (None, None) => {}
            }
            tt += q.step_s;
        }
        if let Some(c) = cur {
            windows.push(mk_window(&t.name, c));
        }
    }
    windows.retain(|w| w.t1 - w.t0 >= s.track_s);

    // Greedy schedule by priority.
    let mut order: Vec<usize> = (0..q.targets.len()).collect();
    order.sort_by_key(|&i| (q.targets[i].priority, i));
    let mut booked: Vec<(f64, f64)> = vec![];
    let mut schedule = vec![];
    let mut summary = vec![];
    let r_sigma_ang = (s.sigma_angle_arcsec / 3600.0).to_radians();
    for &ti in &order {
        let t = &q.targets[ti];
        let mine: Vec<&Window> = windows.iter().filter(|w| w.target == t.name).collect();
        let mut n = 0usize;
        for w in &mine {
            if n >= q.max_passes_per_target {
                break;
            }
            // Earliest start in the window clearing every booked track by the slew time.
            let mut start = w.t0;
            loop {
                let clash = booked
                    .iter()
                    .find(|(a, b)| start < b + s.slew_s && start + s.track_s + s.slew_s > *a);
                match clash {
                    Some((_, b)) => start = b + s.slew_s,
                    None => break,
                }
            }
            if start + s.track_s > w.t1 {
                continue;
            }
            booked.push((start, start + s.track_s));
            // Fisher information of the track.
            let (r0, v0) = orbits[ti].state(start).ok_or("propagation failed")?;
            let x0 = [r0[0], r0[1], r0[2], v0[0], v0[1], v0[2]];
            let k = (s.track_s / s.meas_interval_s).floor() as usize;
            let stm = propagate_with_stm(&x0, s.meas_interval_s, k);
            let mut info = [[0.0; 6]; 6];
            let mut m = 0;
            for (j, (x, phi)) in stm.iter().enumerate() {
                let tj = start + j as f64 * s.meas_interval_s;
                let st = Station {
                    pos: ecef_to_teme(site_ecef, jd(tj)),
                };
                let mut kinds = vec![(MeasurementType::AzEl, [r_sigma_ang, r_sigma_ang])];
                if let (SensorKind::Radar, Some(sr)) = (s.kind, s.sigma_range_m) {
                    kinds.push((MeasurementType::Range, [sr, sr]));
                }
                for (kind, sig) in kinds {
                    let Some((_, rows)) = measurement_with_partials(kind, x, &st) else {
                        continue;
                    };
                    for (c, h) in rows.iter().enumerate() {
                        // g = h · Φ
                        let mut g = [0.0; 6];
                        for (q_, gq) in g.iter_mut().enumerate() {
                            *gq = (0..6).map(|p| h[p] * phi[p][q_]).sum();
                        }
                        let w_ = 1.0 / (sig[c] * sig[c]);
                        for a in 0..6 {
                            for b in 0..6 {
                                info[a][b] += w_ * g[a] * g[b];
                            }
                        }
                        m += 1;
                    }
                }
            }
            let mut p0 = [[0.0; 6]; 6];
            for i in 0..3 {
                p0[i][i] = t.prior_sigma_pos_m.powi(2);
                p0[i + 3][i + 3] = t.prior_sigma_vel_mps.powi(2);
            }
            let p0i = inv6(p0).ok_or("prior covariance is singular")?;
            let mut a = p0i;
            for i in 0..6 {
                for j in 0..6 {
                    a[i][j] += info[i][j];
                }
            }
            let post = inv6(a).ok_or("posterior covariance is singular")?;
            let (sb, sa) = (pos_sigma(&p0), pos_sigma(&post));
            schedule.push(Track {
                target: t.name.clone(),
                priority: t.priority,
                start_utc: iso(start),
                end_utc: iso(start + s.track_s),
                measurements: m,
                prior_pos_sigma_m: round(sb, 1),
                posterior_pos_sigma_m: round(sa, 2),
                improvement_factor: round(sb / sa.max(1e-12), 2),
            });
            n += 1;
        }
        summary.push(TargetSummary {
            target: t.name.clone(),
            priority: t.priority,
            windows: mine.len(),
            scheduled: n,
            reason_unscheduled: match (mine.len(), n) {
                (0, _) => Some("no visibility window within the sensor's limits".into()),
                (_, 0) => Some("every window is taken by higher-priority tracks".into()),
                _ => None,
            },
        });
    }
    schedule.sort_by(|a, b| a.start_utc.cmp(&b.start_utc));
    let verdict = format!(
        "{} window(s) for {} target(s); {} track(s) scheduled; {} target(s) unscheduled (MODELLED, planning only)",
        windows.len(),
        q.targets.len(),
        schedule.len(),
        summary.iter().filter(|x| x.scheduled == 0).count()
    );
    Ok(Plan {
        format: FORMAT,
        evidence: EVIDENCE,
        title: q.title.clone(),
        data_source: q.data_source.clone(),
        sensor: s.clone(),
        window_start_utc: iso(t0),
        window_hours: q.window_hours,
        windows,
        schedule,
        targets: summary,
        limits: vec![
            "Geometry: sidereal-time TEME to Earth-fixed rotation, no polar motion, refraction or light time; windows resolved to the sampling step.".into(),
            "Optical visibility: Sun depression at the sensor and a cylindrical Earth shadow; no Moon, weather or target brightness.".into(),
            "Covariance improvement: per pass against the stated prior at the pass epoch, two-body + J2 dynamics, no process noise, measurement biases or cross-pass chaining.".into(),
            "The scheduler is greedy by priority; it is not an optimal assignment.".into(),
        ],
        verdict,
    })
}

fn mk_window(name: &str, c: (f64, f64, f64, f64)) -> Window {
    Window {
        target: name.to_string(),
        start_utc: iso(c.0),
        end_utc: iso(c.1),
        duration_s: round(c.1 - c.0, 1),
        max_elev_deg: round(c.2, 2),
        min_range_km: round(c.3 / 1e3, 1),
        t0: c.0,
        t1: c.1,
    }
}

/// HTML page.
pub fn html(p: &Plan) -> String {
    crate::transport::page(
        &p.title,
        p.evidence,
        &p.verdict,
        &[format!(
            "Sensor {} ({:?}) at {:.4}, {:.4}. Data source: {}.",
            p.sensor.name, p.sensor.kind, p.sensor.lat_deg, p.sensor.lon_deg, p.data_source
        )],
        &[
            (
                "Schedule".into(),
                vec![
                    "start",
                    "end",
                    "target",
                    "priority",
                    "measurements",
                    "prior σ (m)",
                    "posterior σ (m)",
                    "improvement",
                ],
                p.schedule
                    .iter()
                    .map(|t| {
                        vec![
                            t.start_utc.clone(),
                            t.end_utc.clone(),
                            t.target.clone(),
                            t.priority.to_string(),
                            t.measurements.to_string(),
                            format!("{:.0}", t.prior_pos_sigma_m),
                            format!("{:.1}", t.posterior_pos_sigma_m),
                            format!("{:.1}×", t.improvement_factor),
                        ]
                    })
                    .collect(),
            ),
            (
                "Targets".into(),
                vec!["target", "priority", "windows", "scheduled", "why not"],
                p.targets
                    .iter()
                    .map(|t| {
                        vec![
                            t.target.clone(),
                            t.priority.to_string(),
                            t.windows.to_string(),
                            t.scheduled.to_string(),
                            t.reason_unscheduled.clone().unwrap_or_default(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Visibility windows".into(),
                vec!["target", "start", "end", "max elev (deg)", "min range (km)"],
                p.windows
                    .iter()
                    .map(|w| {
                        vec![
                            w.target.clone(),
                            w.start_utc.clone(),
                            w.end_utc.clone(),
                            format!("{:.1}", w.max_elev_deg),
                            format!("{:.0}", w.min_range_km),
                        ]
                    })
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                p.limits.iter().map(|l| vec![l.clone()]).collect(),
            ),
        ],
    )
}

/// A SYNTHETIC request: a radar near 50° N with an azimuth sector, four synthetic debris TLEs
/// (epoch 2026-11-16 12:00 UTC) at different inclinations and priorities, 24 h window.
pub fn synth() -> String {
    let t = crate::space_ops::tle_line;
    let objs = [
        (
            "SYNTH-DEB-1",
            1,
            "90101",
            "98.6000",
            "030.0000",
            "14.30000000",
        ),
        (
            "SYNTH-DEB-2",
            1,
            "90102",
            "82.9000",
            "150.0000",
            "14.10000000",
        ),
        (
            "SYNTH-DEB-3",
            2,
            "90103",
            "74.0000",
            "270.0000",
            "13.90000000",
        ),
        (
            "SYNTH-DEB-4",
            3,
            "90104",
            "28.5000",
            "010.0000",
            "15.10000000",
        ),
    ];
    let mut s = String::from(
        "title = \"SYNTHETIC debris observation schedule\"\ndata_source = \"SYNTHETIC (kshana-pro debris-obs synth)\"\nwindow_start_utc = \"2026-11-16T12:00:00Z\"\nwindow_hours = 24.0\nstep_s = 20.0\nmax_passes_per_target = 2\n\n[sensor]\nname = \"SYNTH-RADAR\"\nkind = \"radar\"\nlat_deg = 50.0\nlon_deg = 7.0\nalt_m = 300.0\nmin_elev_deg = 20.0\naz_min_deg = 90.0\naz_max_deg = 270.0\nmax_range_km = 2500.0\ntrack_s = 120.0\nslew_s = 60.0\nmeas_interval_s = 10.0\nsigma_angle_arcsec = 72.0\nsigma_range_m = 15.0\n",
    );
    for (name, pr, id, inc, raan, mm) in objs {
        let l1 = t(&format!(
            "1 {id}U 26001A   26320.50000000  .00000500  00000-0  20000-4 0  999"
        ));
        let l2 = t(&format!(
            "2 {id}  {inc} {raan} 0010000  90.0000 270.0000 {mm}  100"
        ));
        s += &format!(
            "\n[[target]]\nname = \"{name}\"\npriority = {pr}\ntle_line1 = \"{l1}\"\ntle_line2 = \"{l2}\"\nprior_sigma_pos_m = 1000.0\nprior_sigma_vel_mps = 1.0\n"
        );
    }
    s
}
