// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The neutral scenario spec (format [`FORMAT`], TOML or JSON): what a receiver test is,
//! independent of any simulator.
//!
//! ```toml
//! format = "kshana-testkit-scenario/1"
//! id = "dp-slow-drift"
//! version = 1
//! title = "DP vessel: slow GNSS drift on station"
//! modelled_on = ["IMCA IN 1730 theme: slow drift (modelled on, not a test method of the note)"]
//!
//! [scene]
//! start_utc = "2026-03-01T10:00:00Z"
//! duration_s = 1200
//!
//! [[signal]]                     # constellations and signals the simulator generates
//! constellation = "gps"
//! signals = ["L1CA", "L5"]
//!
//! [trajectory]                   # static, waypoints or time-tagged samples
//! platform = "vessel"
//! lat_deg = 57.1
//! lon_deg = 1.9
//!
//! [environment]
//! ionosphere = "nominal"         # nominal | off | storm
//! multipath = "low"              # none | low | medium | high
//! sensors = ["gyro", "dp-reference-2"]   # synchronised non-GNSS inputs the test needs
//!
//! [[threat]]                     # signal-parameter level only
//! id = "drift"
//! type = "spoofing"              # jamming | spoofing | cn0_offset | power_step | outage | sat_mask
//! technique = "position_drag"    # simplistic | position_drag | time_push | meaconing
//! start_s = 300
//! duration_s = 600
//! drag_rate_mps = 0.05
//! drag_bearing_deg = 90
//!
//! [expected]
//! behaviour = "The receiver flags the drift or the DP system rejects the GNSS reference."
//!
//! [[criterion]]
//! id = "detect"
//! metric = "detection_latency_s" # see Metric
//! op = "<="
//! value = 60
//! ```
//!
//! A threat may name a threat-library scenario instead (`library = "<id>"`, no `type`):
//! [`Spec::resolve`] replaces it with that scenario's analogue events at signal-parameter
//! level, shifted by the threat's `start_s`.

use std::collections::BTreeMap;

use kshana_eloran::sync::degrade::JamKind;
use kshana_eloran::sync::profile::Constellation;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The format tag of a neutral scenario.
pub const FORMAT: &str = "kshana-testkit-scenario/1";

fn one() -> u32 {
    1
}
fn modelled() -> String {
    "MODELLED".into()
}
fn yes() -> bool {
    true
}

/// A neutral scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub format: String,
    pub id: String,
    #[serde(default = "one")]
    pub version: u32,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// The evidence label (`MODELLED` unless validated against real data).
    #[serde(default = "modelled")]
    pub evidence: String,
    /// What the scenario is modelled on (a standard's theme, a framework's level); never a
    /// claim of conformance.
    #[serde(default)]
    pub modelled_on: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub scene: Scene,
    #[serde(default, rename = "signal")]
    pub signals: Vec<SignalSet>,
    pub trajectory: Trajectory,
    #[serde(default)]
    pub environment: Environment,
    #[serde(default, rename = "threat")]
    pub threats: Vec<Threat>,
    #[serde(default)]
    pub expected: Expected,
    #[serde(default, rename = "criterion")]
    pub criteria: Vec<Criterion>,
    /// Set by the incident converter: where the scenario came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident: Option<IncidentRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    pub start_utc: String,
    pub duration_s: f64,
}

/// One constellation and the signals generated for it (empty `prns`: all visible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalSet {
    pub constellation: Constellation,
    pub signals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prns: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    #[default]
    Static,
    Land,
    Vessel,
    Aircraft,
    Uas,
    Timing,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Static => "static",
            Platform::Land => "land",
            Platform::Vessel => "vessel",
            Platform::Aircraft => "aircraft",
            Platform::Uas => "uas",
            Platform::Timing => "timing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Waypoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_m: Option<f64>,
    /// Speed on the leg towards this waypoint, m/s.
    pub speed_mps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackPoint {
    pub t_s: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub height_m: f64,
}

/// The true receiver motion: static at the start point, along waypoints, or time-tagged
/// samples (linear interpolation; the first and last are held outside their span).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trajectory {
    #[serde(default)]
    pub platform: Platform,
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub height_m: f64,
    #[serde(default, rename = "waypoint", skip_serializing_if = "Vec::is_empty")]
    pub waypoints: Vec<Waypoint>,
    #[serde(default, rename = "sample", skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<TrackPoint>,
}

/// The truth at one instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TruthState {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
    pub heading_deg: f64,
    pub speed_mps: f64,
}

/// Metres per degree of latitude and of longitude at `lat_deg` (spherical, local).
pub fn m_per_deg(lat_deg: f64) -> (f64, f64) {
    let m = std::f64::consts::PI * 6_371_008.8 / 180.0;
    (m, m * lat_deg.to_radians().cos())
}

/// Move `(lat, lon)` by `east_m`, `north_m` (local flat-earth, for offsets up to tens of km).
pub fn offset(lat: f64, lon: f64, east_m: f64, north_m: f64) -> (f64, f64) {
    let (mlat, mlon) = m_per_deg(lat);
    (lat + north_m / mlat, lon + east_m / mlon.max(1e-6))
}

/// East and north metres from `a` to `b` (local).
pub fn en(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (mlat, mlon) = m_per_deg(a.0);
    ((b.1 - a.1) * mlon, (b.0 - a.0) * mlat)
}

impl Trajectory {
    pub fn is_static(&self) -> bool {
        self.waypoints.is_empty() && self.samples.len() <= 1
    }

    /// The true state at `t_s` seconds from the scene start.
    pub fn at(&self, t_s: f64) -> TruthState {
        let stat = |lat, lon, h| TruthState {
            lat_deg: lat,
            lon_deg: lon,
            height_m: h,
            heading_deg: 0.0,
            speed_mps: 0.0,
        };
        if !self.samples.is_empty() {
            let s = &self.samples;
            if t_s <= s[0].t_s || s.len() == 1 {
                return stat(s[0].lat_deg, s[0].lon_deg, s[0].height_m);
            }
            let last = &s[s.len() - 1];
            if t_s >= last.t_s {
                return stat(last.lat_deg, last.lon_deg, last.height_m);
            }
            let k = s.partition_point(|p| p.t_s <= t_s) - 1;
            let (a, b) = (&s[k], &s[k + 1]);
            let f = (t_s - a.t_s) / (b.t_s - a.t_s);
            let (de, dn) = en((a.lat_deg, a.lon_deg), (b.lat_deg, b.lon_deg));
            let dt = b.t_s - a.t_s;
            return TruthState {
                lat_deg: a.lat_deg + f * (b.lat_deg - a.lat_deg),
                lon_deg: a.lon_deg + f * (b.lon_deg - a.lon_deg),
                height_m: a.height_m + f * (b.height_m - a.height_m),
                heading_deg: de.atan2(dn).to_degrees().rem_euclid(360.0),
                speed_mps: (de * de + dn * dn).sqrt() / dt,
            };
        }
        let mut pos = (self.lat_deg, self.lon_deg, self.height_m);
        let mut left = t_s.max(0.0);
        for w in &self.waypoints {
            let h = w.height_m.unwrap_or(pos.2);
            let (de, dn) = en((pos.0, pos.1), (w.lat_deg, w.lon_deg));
            let dist = (de * de + dn * dn + (h - pos.2).powi(2)).sqrt();
            if w.speed_mps <= 0.0 || dist < 1e-9 {
                pos = (w.lat_deg, w.lon_deg, h);
                continue;
            }
            let leg_t = dist / w.speed_mps;
            let heading = de.atan2(dn).to_degrees().rem_euclid(360.0);
            if left < leg_t {
                let f = left / leg_t;
                return TruthState {
                    lat_deg: pos.0 + f * (w.lat_deg - pos.0),
                    lon_deg: pos.1 + f * (w.lon_deg - pos.1),
                    height_m: pos.2 + f * (h - pos.2),
                    heading_deg: heading,
                    speed_mps: w.speed_mps,
                };
            }
            left -= leg_t;
            pos = (w.lat_deg, w.lon_deg, h);
        }
        stat(pos.0, pos.1, pos.2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtmosModel {
    #[default]
    Nominal,
    Off,
    Storm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Multipath {
    #[default]
    None,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    #[serde(default)]
    pub ionosphere: AtmosModel,
    #[serde(default)]
    pub troposphere: AtmosModel,
    #[serde(default)]
    pub multipath: Multipath,
    /// A named sky obstruction (`fjord`, `container-terminal`, `urban-canyon`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obscuration: Option<String>,
    /// Synchronised non-GNSS inputs the test needs (`imu`, `gyro`, `log`, `odometer`,
    /// `baro`, `dp-reference-2`, `reference-clock`, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sensors: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatType {
    Jamming,
    Spoofing,
    Cn0Offset,
    PowerStep,
    Outage,
    SatMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Technique {
    /// A step to a constant false position/time.
    Simplistic,
    /// A position walked away from truth (a rate, an acceleration limit, or a final offset
    /// over the ramp).
    PositionDrag,
    /// Time pushed at a rate.
    TimePush,
    /// Re-broadcast with a delay.
    Meaconing,
}

impl Technique {
    pub fn as_str(self) -> &'static str {
        match self {
            Technique::Simplistic => "simplistic",
            Technique::PositionDrag => "position_drag",
            Technique::TimePush => "time_push",
            Technique::Meaconing => "meaconing",
        }
    }
}

/// One threat or degradation event, at signal-parameter level. Which fields apply depends
/// on `type` (and `technique`); [`Spec::validate`] refuses missing ones.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Threat {
    pub id: String,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<ThreatType>,
    /// A threat-library scenario to expand ([`Spec::resolve`]); kept on expanded threats as
    /// provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    pub start_s: f64,
    #[serde(default)]
    pub duration_s: f64,
    #[serde(default)]
    pub ramp_s: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constellation: Option<Constellation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prns: Vec<u32>,
    // jamming
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waveform: Option<JamKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bandwidth_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub js_db: Option<f64>,
    // spoofing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub technique: Option<Technique>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_east_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_north_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_bearing_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_rate_mps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_accel_mps2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_max_mps: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_offset_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_offset_ns: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_rate_ns_per_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ns: Option<f64>,
    /// Spoofer power above the genuine signals, dB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_advantage_db: Option<f64>,
    // cn0_offset / power_step
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_db: Option<f64>,
}

impl Threat {
    pub fn end_s(&self) -> f64 {
        self.start_s + self.duration_s
    }

    /// The counterfeit offset a spoofing threat applies `u` seconds after its onset:
    /// (east m, north m, time ns). Zero for every other type and outside the threat.
    pub fn counterfeit(&self, u: f64) -> (f64, f64, f64) {
        if self.kind != Some(ThreatType::Spoofing) || u < 0.0 || u > self.duration_s {
            return (0.0, 0.0, 0.0);
        }
        let b = self.drag_bearing_deg.unwrap_or(0.0).to_radians();
        let along = |d: f64| (d * b.sin(), d * b.cos());
        match self.technique {
            Some(Technique::Simplistic) => (
                self.offset_east_m.unwrap_or(0.0),
                self.offset_north_m.unwrap_or(0.0),
                self.time_offset_ns.unwrap_or(0.0),
            ),
            Some(Technique::PositionDrag) => {
                let d = if let Some(f) = self.final_offset_m {
                    // smoothstep over the ramp (a step when there is none)
                    let x = if self.ramp_s > 0.0 {
                        (u / self.ramp_s).clamp(0.0, 1.0)
                    } else {
                        1.0
                    };
                    f * x * x * (3.0 - 2.0 * x)
                } else if let Some(a) = self.drag_accel_mps2 {
                    let vmax = self.drag_max_mps.unwrap_or(f64::INFINITY);
                    let ts = if a > 0.0 { vmax / a } else { 0.0 };
                    if u <= ts {
                        0.5 * a * u * u
                    } else {
                        0.5 * a * ts * ts + vmax * (u - ts)
                    }
                } else {
                    self.drag_rate_mps.unwrap_or(0.0) * u
                };
                let (e, n) = along(d);
                (e, n, 0.0)
            }
            Some(Technique::TimePush) => {
                let r = self.push_rate_ns_per_s.unwrap_or(0.0) * u;
                let t = match self.time_offset_ns {
                    Some(cap) if r.abs() > cap.abs() => cap,
                    _ => r,
                };
                (0.0, 0.0, t)
            }
            Some(Technique::Meaconing) => (
                self.offset_east_m.unwrap_or(0.0),
                self.offset_north_m.unwrap_or(0.0),
                self.delay_ns.unwrap_or(0.0),
            ),
            None => (0.0, 0.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// The expected behaviour, in words.
    #[serde(default)]
    pub behaviour: String,
    /// Whether withdrawing the fix counts as detection (jamming), or only an explicit flag.
    #[serde(default = "yes")]
    pub withdraw_counts_as_detection: bool,
    /// A valid, unflagged fix farther than this from truth is misleading output.
    #[serde(default = "d_mis_h")]
    pub misleading_h_error_m: f64,
    #[serde(default = "d_mis_t")]
    pub misleading_time_error_ns: f64,
    /// Correct output after a threat: within this of truth.
    #[serde(default = "d_rec_h")]
    pub recover_h_error_m: f64,
    #[serde(default = "d_rec_t")]
    pub recover_time_error_ns: f64,
    /// Correct output must hold this long to count as recovered.
    #[serde(default = "d_dwell")]
    pub recover_dwell_s: f64,
}

fn d_mis_h() -> f64 {
    50.0
}
fn d_mis_t() -> f64 {
    1000.0
}
fn d_rec_h() -> f64 {
    20.0
}
fn d_rec_t() -> f64 {
    100.0
}
fn d_dwell() -> f64 {
    10.0
}

impl Default for Expected {
    fn default() -> Self {
        Expected {
            behaviour: String::new(),
            withdraw_counts_as_detection: true,
            misleading_h_error_m: d_mis_h(),
            misleading_time_error_ns: d_mis_t(),
            recover_h_error_m: d_rec_h(),
            recover_time_error_ns: d_rec_t(),
            recover_dwell_s: d_dwell(),
        }
    }
}

/// What a criterion measures. See [`super::score`] for the definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    DetectionLatencyS,
    MisleadingEpochs,
    MaxUnflaggedHErrorM,
    MaxUnflaggedTimeErrorNs,
    RecoveryS,
    Availability,
    MaxHErrorM,
    P95HErrorM,
    MaxTimeErrorNs,
    FalseAlarms,
}

impl Metric {
    pub fn as_str(self) -> &'static str {
        match self {
            Metric::DetectionLatencyS => "detection_latency_s",
            Metric::MisleadingEpochs => "misleading_epochs",
            Metric::MaxUnflaggedHErrorM => "max_unflagged_h_error_m",
            Metric::MaxUnflaggedTimeErrorNs => "max_unflagged_time_error_ns",
            Metric::RecoveryS => "recovery_s",
            Metric::Availability => "availability",
            Metric::MaxHErrorM => "max_h_error_m",
            Metric::P95HErrorM => "p95_h_error_m",
            Metric::MaxTimeErrorNs => "max_time_error_ns",
            Metric::FalseAlarms => "false_alarms",
        }
    }
    /// The window a metric is measured in when the criterion names none.
    pub fn default_window(self) -> Window {
        match self {
            Metric::DetectionLatencyS | Metric::Availability => Window::Threat,
            Metric::RecoveryS => Window::After,
            Metric::FalseAlarms => Window::Baseline,
            Metric::MisleadingEpochs
            | Metric::MaxUnflaggedHErrorM
            | Metric::MaxUnflaggedTimeErrorNs => Window::FromOnset,
            Metric::MaxHErrorM | Metric::P95HErrorM | Metric::MaxTimeErrorNs => Window::Threat,
        }
    }
    /// Whether the metric reads time error (a timing criterion).
    pub fn is_timing(self) -> bool {
        matches!(
            self,
            Metric::MaxUnflaggedTimeErrorNs | Metric::MaxTimeErrorNs
        )
    }
}

/// Where a criterion is measured: before the first onset, during the threat (onset to
/// end), after the end, from the onset to the end of the run, or the whole run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Window {
    Baseline,
    Threat,
    After,
    FromOnset,
    All,
}

impl Window {
    pub fn as_str(self) -> &'static str {
        match self {
            Window::Baseline => "baseline",
            Window::Threat => "threat",
            Window::After => "after",
            Window::FromOnset => "from_onset",
            Window::All => "all",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">=")]
    Ge,
}

impl Op {
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Le => "<=",
            Op::Ge => ">=",
        }
    }
    pub fn holds(self, measured: f64, value: f64) -> bool {
        match self {
            Op::Le => measured <= value,
            Op::Ge => measured >= value,
        }
    }
}

/// A pass/fail criterion, stated before the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    pub id: String,
    pub metric: Metric,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<Window>,
    /// The threat whose window applies (default: from the first onset to the last end).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threat: Option<String>,
    pub op: Op,
    pub value: f64,
    /// An error threshold the metric uses (misleading, recovery, availability), overriding
    /// `[expected]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    /// A required criterion decides the verdict; an informative one is reported only.
    #[serde(default = "yes")]
    pub required: bool,
    /// The resilience level (1 to 4) the criterion evidences, modelled on the DHS RPCF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// Where an incident-derived scenario came from ([`super::incident`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentRef {
    pub id: String,
    pub title: String,
    pub log_sha256: String,
    pub log_source: String,
    /// Each parameter estimated from the log, and how.
    pub derived: BTreeMap<String, String>,
}

/// Words a scenario may not use about itself (the threat library's rule).
const FORBIDDEN: [&str; 4] = ["compliant", "conformant", "certified", "certification"];

impl Spec {
    /// Parse TOML or JSON (by the first non-blank character) and validate.
    pub fn parse(src: &str) -> Result<Spec, String> {
        let s: Spec = if src.trim_start().starts_with('{') {
            serde_json::from_str(src).map_err(|e| format!("scenario: {e}"))?
        } else {
            toml::from_str(src).map_err(|e| format!("scenario: {e}"))?
        };
        s.validate()?;
        Ok(s)
    }

    pub fn load(path: &std::path::Path) -> Result<Spec, String> {
        let t = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Spec::parse(&t).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn to_toml(&self) -> String {
        format!(
            "# Kshana test kit: neutral scenario ({FORMAT}). {}\n{}",
            super::LABEL_SHORT,
            toml::to_string(self).expect("spec serialises")
        )
    }

    /// SHA-256 of the canonical JSON (the identity a verdict carries).
    pub fn sha256(&self) -> String {
        hex::encode(Sha256::digest(
            serde_json::to_vec(self).expect("spec serialises"),
        ))
    }

    /// The signals, GPS L1 C/A when none are stated.
    pub fn signal_sets(&self) -> Vec<SignalSet> {
        if self.signals.is_empty() {
            vec![SignalSet {
                constellation: Constellation::Gps,
                signals: vec!["L1CA".into()],
                prns: vec![],
            }]
        } else {
            self.signals.clone()
        }
    }

    /// The window `(from, to)` of a threat id, or of all threats (first onset to last end);
    /// `None` when the scenario has no threat.
    pub fn threat_window(&self, id: Option<&str>) -> Option<(f64, f64)> {
        let ts: Vec<&Threat> = self
            .threats
            .iter()
            .filter(|t| id.is_none_or(|i| t.id == i))
            .collect();
        if ts.is_empty() {
            return None;
        }
        let a = ts.iter().map(|t| t.start_s).fold(f64::INFINITY, f64::min);
        let b = ts
            .iter()
            .map(|t| t.end_s())
            .fold(f64::NEG_INFINITY, f64::max);
        Some((a, b))
    }

    /// The combined counterfeit offset at `t_s` (all spoofing threats).
    pub fn counterfeit_at(&self, t_s: f64) -> (f64, f64, f64) {
        self.threats.iter().fold((0.0, 0.0, 0.0), |acc, t| {
            let c = t.counterfeit(t_s - t.start_s);
            (acc.0 + c.0, acc.1 + c.1, acc.2 + c.2)
        })
    }

    /// Whether any criterion reads time error.
    pub fn has_timing_criteria(&self) -> bool {
        self.criteria.iter().any(|c| c.metric.is_timing())
    }

    pub fn validate(&self) -> Result<(), String> {
        let id = &self.id;
        let e = |m: String| Err(format!("{id}: {m}"));
        if self.format != FORMAT {
            return Err(format!(
                "scenario format '{}' (expected {FORMAT})",
                self.format
            ));
        }
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "scenario id '{id}' must be letters, digits, '-' or '_'"
            ));
        }
        kshana_eloran::sync::UtcStamp::parse(&self.scene.start_utc)
            .map_err(|m| format!("{id}: scene.start_utc: {m}"))?;
        if !(self.scene.duration_s.is_finite() && self.scene.duration_s > 0.0) {
            return e("scene.duration_s must be > 0".into());
        }
        let text = format!(
            "{} {} {}",
            self.title, self.description, self.expected.behaviour
        )
        .to_ascii_lowercase();
        for w in FORBIDDEN {
            if text.contains(w) {
                return e(format!(
                    "a scenario may not call anything '{w}' (use 'modelled on')"
                ));
            }
        }
        let tr = &self.trajectory;
        for (n, v) in [("lat_deg", tr.lat_deg), ("lon_deg", tr.lon_deg)] {
            if !v.is_finite() {
                return e(format!("trajectory.{n} must be finite"));
            }
        }
        if tr.lat_deg.abs() > 90.0 || tr.lon_deg.abs() > 180.0 {
            return e("trajectory start is not a valid position".into());
        }
        if tr.samples.windows(2).any(|w| w[1].t_s <= w[0].t_s) {
            return e("trajectory samples must have increasing t_s".into());
        }
        for s in &self.signals {
            if s.signals.is_empty() {
                return e(format!(
                    "signal {}: no signals listed",
                    s.constellation.key()
                ));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for t in &self.threats {
            if !ids.insert(t.id.as_str()) {
                return e(format!("duplicate threat id '{}'", t.id));
            }
            self.validate_threat(t)
                .map_err(|m| format!("{id}: threat {}: {m}", t.id))?;
        }
        let mut cids = std::collections::BTreeSet::new();
        for c in &self.criteria {
            if !cids.insert(c.id.as_str()) {
                return e(format!("duplicate criterion id '{}'", c.id));
            }
            if !c.value.is_finite() {
                return e(format!("criterion {}: value must be finite", c.id));
            }
            if let Some(t) = &c.threat {
                if !self.threats.iter().any(|x| &x.id == t) {
                    return e(format!("criterion {}: unknown threat '{t}'", c.id));
                }
            }
            if c.level.is_some_and(|l| !(1..=4).contains(&l)) {
                return e(format!("criterion {}: level must be 1 to 4", c.id));
            }
        }
        Ok(())
    }

    fn validate_threat(&self, t: &Threat) -> Result<(), String> {
        let need = |v: Option<f64>, n: &str| match v {
            Some(x) if x.is_finite() => Ok(x),
            _ => Err(format!("needs {n}")),
        };
        if !(t.start_s.is_finite() && t.start_s >= 0.0) {
            return Err("start_s must be >= 0".into());
        }
        let Some(kind) = t.kind else {
            if t.library.is_some() {
                return Ok(()); // expanded by resolve()
            }
            return Err("needs a type (or a library scenario)".into());
        };
        if kind != ThreatType::PowerStep && !(t.duration_s.is_finite() && t.duration_s > 0.0) {
            return Err("duration_s must be > 0".into());
        }
        if t.start_s >= self.scene.duration_s {
            return Err("starts after the scene ends".into());
        }
        match kind {
            ThreatType::Jamming => {
                let c = need(t.center_hz, "center_hz")?;
                if c <= 0.0 {
                    return Err("center_hz must be > 0".into());
                }
                need(t.js_db, "js_db")?;
            }
            ThreatType::Spoofing => match t.technique {
                None => return Err("spoofing needs a technique".into()),
                Some(Technique::Simplistic) => {
                    if t.offset_east_m.is_none()
                        && t.offset_north_m.is_none()
                        && t.time_offset_ns.is_none()
                    {
                        return Err("simplistic spoofing needs an offset".into());
                    }
                }
                Some(Technique::PositionDrag) => {
                    if t.final_offset_m.is_none()
                        && t.drag_rate_mps.is_none()
                        && t.drag_accel_mps2.is_none()
                    {
                        return Err(
                            "position_drag needs final_offset_m, drag_rate_mps or drag_accel_mps2"
                                .into(),
                        );
                    }
                }
                Some(Technique::TimePush) => {
                    need(t.push_rate_ns_per_s, "push_rate_ns_per_s")?;
                }
                Some(Technique::Meaconing) => {
                    need(t.delay_ns, "delay_ns")?;
                }
            },
            ThreatType::Cn0Offset | ThreatType::PowerStep => {
                need(t.offset_db, "offset_db")?;
            }
            ThreatType::Outage => {}
            ThreatType::SatMask => {
                if t.constellation.is_none() || t.prns.is_empty() {
                    return Err("sat_mask needs a constellation and prns".into());
                }
            }
        }
        Ok(())
    }

    /// Expand every `library = "<id>"` threat without a `type` into the library scenario's
    /// analogue events (see [`super::library`]); the rest are kept as written.
    pub fn resolve(&self) -> Result<Spec, String> {
        let mut out = self.clone();
        let mut threats = Vec::new();
        for t in &self.threats {
            match (&t.kind, &t.library) {
                (None, Some(lib)) => {
                    let heading = self.trajectory.at(t.start_s).heading_deg;
                    let timing = self.trajectory.platform == Platform::Timing;
                    for mut x in super::library::threats(lib, t.start_s, &t.id, heading, timing)? {
                        if x.start_s >= self.scene.duration_s {
                            continue;
                        }
                        x.duration_s = x.duration_s.min(self.scene.duration_s - x.start_s);
                        threats.push(x);
                    }
                }
                _ => threats.push(t.clone()),
            }
        }
        out.threats = threats;
        out.validate()?;
        Ok(out)
    }
}
