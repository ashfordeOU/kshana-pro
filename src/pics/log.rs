// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The device log PICS scores: one common JSON form ([`DeviceLog`], format [`FORMAT`]), and
//! converters from an NMEA 0183 log (through the open engine's reader,
//! `kshana::receiver_trust::ingest::read_nmea`) and from a timing log (any format of
//! `kshana::clock_log`, its phase taken as the device's time error against the reference the
//! log was measured with).
//!
//! The scenario clock: `t_s` is seconds from the scenario start (the start of the baseline);
//! a converter takes the offset between its log's own time axis and the scenario start.
//!
//! [`synth`] writes a synthetic device response (a well-behaved one, or one that follows the
//! threat without warning) for the tests and the public sample. It is MODELLED: a test
//! fixture, not a model of any receiver.

use serde::{Deserialize, Serialize};

use super::{Scenario, Suite};

/// The format tag of a device log.
pub const FORMAT: &str = "kshana-pics-device-log/1";

/// The device under test, as the tester states it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub firmware: Option<String>,
    #[serde(default)]
    pub serial: Option<String>,
}

/// The device's true position (static).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Truth {
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub height_m: f64,
}

/// What the device reported at one epoch. Every field but `t_s` is optional: a log carries
/// what it carries, and a criterion whose data is absent is not assessed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Epoch {
    pub t_s: f64,
    /// The device calls its position/time output valid.
    #[serde(default)]
    pub fix_valid: Option<bool>,
    #[serde(default)]
    pub lat_deg: Option<f64>,
    #[serde(default)]
    pub lon_deg: Option<f64>,
    #[serde(default)]
    pub height_m: Option<f64>,
    /// Device time minus the reference, ns (measured outside the device).
    #[serde(default)]
    pub time_error_ns: Option<f64>,
    #[serde(default)]
    pub jamming: Option<bool>,
    #[serde(default)]
    pub spoofing: Option<bool>,
    /// `ok`, `caution` or `unsafe` (the device's own integrity indication).
    #[serde(default)]
    pub integrity: Option<String>,
}

impl Epoch {
    /// An explicit warning: a jamming, spoofing or non-`ok` integrity indication.
    pub fn flagged(&self) -> bool {
        self.jamming == Some(true)
            || self.spoofing == Some(true)
            || self
                .integrity
                .as_deref()
                .is_some_and(|i| i != "ok" && !i.is_empty())
    }
    /// The output is withdrawn (the device says its fix is not valid).
    pub fn withdrawn(&self) -> bool {
        self.fix_valid == Some(false)
    }
}

/// A device log in the common form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceLog {
    pub format: String,
    #[serde(default)]
    pub device: Device,
    /// The PICS scenario the log was recorded under.
    pub scenario: String,
    /// The true position; the suite's truth when absent.
    #[serde(default)]
    pub truth: Option<Truth>,
    /// Where the log came from (`json`, `nmea`, `timing:<format>`, `synthetic`).
    #[serde(default)]
    pub source: Option<String>,
    pub epochs: Vec<Epoch>,
}

impl DeviceLog {
    pub fn parse(src: &str) -> Result<DeviceLog, String> {
        let l: DeviceLog =
            serde_json::from_str(src).map_err(|e| format!("PICS device log: {e}"))?;
        if l.format != FORMAT {
            return Err(format!(
                "PICS device log: format '{}' (expected {FORMAT})",
                l.format
            ));
        }
        l.validate()?;
        Ok(l)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.epochs.is_empty() {
            return Err("PICS device log: no epochs".into());
        }
        let mut last = f64::NEG_INFINITY;
        for e in &self.epochs {
            if !e.t_s.is_finite() || e.t_s < last {
                return Err(format!(
                    "PICS device log: epoch times must be finite and non-decreasing (at t = {})",
                    e.t_s
                ));
            }
            last = e.t_s;
            if let Some(i) = &e.integrity {
                if !["ok", "caution", "unsafe"].contains(&i.as_str()) {
                    return Err(format!(
                        "PICS device log: integrity '{i}' at t = {} (expected ok, caution or unsafe)",
                        e.t_s
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("device log serialises")
    }

    /// Merge a timing log into a position log epoch by epoch: each epoch takes the time error
    /// of the nearest timing sample within `tol_s`.
    pub fn merge_time_errors(&mut self, timing: &DeviceLog, tol_s: f64) {
        for e in &mut self.epochs {
            let near = timing
                .epochs
                .iter()
                .filter(|t| t.time_error_ns.is_some())
                .min_by(|a, b| (a.t_s - e.t_s).abs().total_cmp(&(b.t_s - e.t_s).abs()));
            if let Some(t) = near {
                if (t.t_s - e.t_s).abs() <= tol_s {
                    e.time_error_ns = t.time_error_ns;
                }
            }
        }
    }
}

/// Convert an NMEA 0183 log: `t0_s` is the scenario time of the log's first epoch.
/// Fix validity is GGA/RMC's (`MarineObs::fix_valid`), else the presence of a fix; the
/// receiver's own spoofing/jamming states (u-blox codes 2 and above) and an OSNMA failure
/// are taken as flags. NMEA carries no time error: merge a timing log for that.
pub fn from_nmea(text: &str, scenario: &str, t0_s: f64) -> Result<DeviceLog, String> {
    let tl = kshana::receiver_trust::ingest::read_nmea(text)?;
    let epochs = tl
        .epochs
        .iter()
        .map(|e| {
            let osnma_failed = e
                .marine
                .as_ref()
                .and_then(|m| m.osnma)
                .is_some_and(|s| s == kshana::receiver_trust::OsnmaStatus::Failed);
            let sr = e.self_report;
            Epoch {
                t_s: t0_s + e.t_s,
                fix_valid: e
                    .marine
                    .as_ref()
                    .and_then(|m| m.fix_valid)
                    .or(Some(e.fix.is_some())),
                lat_deg: e.fix.map(|f| f.lat_deg),
                lon_deg: e.fix.map(|f| f.lon_deg),
                height_m: e.fix.map(|f| f.height_m),
                time_error_ns: None,
                jamming: sr.and_then(|s| s.jamming).map(|v| v >= 2),
                spoofing: match (sr.and_then(|s| s.spoofing).map(|v| v >= 2), osnma_failed) {
                    (_, true) => Some(true),
                    (v, false) => v,
                },
                integrity: None,
            }
        })
        .collect();
    let l = DeviceLog {
        format: FORMAT.into(),
        device: Device::default(),
        scenario: scenario.into(),
        truth: None,
        source: Some("nmea".into()),
        epochs,
    };
    l.validate()?;
    Ok(l)
}

/// Convert a timing log read by `kshana::clock_log` (its phase is the device's time error
/// against the reference it was measured with); `t0_s` is the scenario time of its first
/// sample.
pub fn from_timing(
    inp: &kshana::clock_log::ClockLogInput,
    scenario: &str,
    t0_s: f64,
) -> Result<DeviceLog, String> {
    let rec = kshana::clock_log::read_input(inp)?;
    let epochs = rec
        .time_s
        .iter()
        .zip(&rec.phase_s)
        .map(|(t, x)| Epoch {
            t_s: t0_s + t,
            time_error_ns: Some(x * 1e9),
            ..Epoch::default()
        })
        .collect();
    let l = DeviceLog {
        format: FORMAT.into(),
        device: Device::default(),
        scenario: scenario.into(),
        truth: None,
        source: Some(format!("timing:{}", inp.format)),
        epochs,
    };
    l.validate()?;
    Ok(l)
}

/// The counterfeit offset a scenario's threat applies at `u` seconds after onset: (east m,
/// north m, time ns). Jamming applies none. This is the scenario's own definition of the
/// spoofer's motion, used by the synthetic responses and written into simulator exports.
pub fn threat_effect(sc: &Scenario, u: f64) -> (f64, f64, f64) {
    let p = |k: &str| sc.params.get(k).copied().unwrap_or(0.0);
    let after = |t0: f64| (u - t0).max(0.0);
    match sc.technique.as_str() {
        "simplistic" => (
            p("false_offset_east_m"),
            p("false_offset_north_m"),
            p("time_offset_ns"),
        ),
        "intermediate" => {
            let d = p("drag_rate_mps") * after(p("takeover_s"));
            let b = p("drag_bearing_deg").to_radians();
            (d * b.sin(), d * b.cos(), 0.0)
        }
        "position-drag" => {
            let t = after(p("takeover_s"));
            let (a, vmax) = (p("drag_accel_mps2"), p("drag_max_mps"));
            let t_sat = if a > 0.0 { vmax / a } else { 0.0 };
            let d = if t <= t_sat {
                0.5 * a * t * t
            } else {
                0.5 * a * t_sat * t_sat + vmax * (t - t_sat)
            };
            let b = p("drag_bearing_deg").to_radians();
            (d * b.sin(), d * b.cos(), 0.0)
        }
        "time-push" => (0.0, 0.0, p("push_rate_ns_per_s") * after(p("takeover_s"))),
        "meaconing" => (
            p("meacon_offset_east_m"),
            p("meacon_offset_north_m"),
            p("delay_ns"),
        ),
        _ => (0.0, 0.0, 0.0),
    }
}

/// A synthetic device behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behaviour {
    /// Flags (or, under jamming, withdraws its fix) `react_s` after onset, never outputs the
    /// counterfeit unflagged, and is correct again 20 s after the threat ends.
    Good,
    /// Follows the threat without any warning (spoofing), or keeps a drifting fix without
    /// warning (jamming: 1 m/s and 5 ns/s), and takes 120 s to recover.
    Bad,
}

/// Metres per degree of latitude, and of longitude at `lat_deg` (spherical, for small offsets).
pub fn metres_per_degree(lat_deg: f64) -> (f64, f64) {
    let m = std::f64::consts::PI * 6_371_008.8 / 180.0;
    (m, m * lat_deg.to_radians().cos())
}

/// A synthetic 1 Hz device log for a scenario. `react_s` is the Good device's reaction time.
pub fn synth(suite: &Suite, sc: &Scenario, b: Behaviour, react_s: f64) -> DeviceLog {
    let i = &suite.suite;
    let (onset, end) = suite.threat_window();
    let (mlat, mlon) = metres_per_degree(i.truth_lat_deg);
    let n = suite.duration_s().round() as usize;
    let recover_after = match b {
        Behaviour::Good => 20.0,
        Behaviour::Bad => 120.0,
    };
    let mut epochs = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let t = k as f64;
        // A small deterministic wander standing in for the device's clean noise.
        let wob = ((k as f64) * 0.7).sin();
        let (mut e, mut nn, mut te) = (0.8 * wob, 0.6 * wob, 4.0 * wob);
        let mut ep = Epoch {
            t_s: t,
            fix_valid: Some(true),
            jamming: Some(false),
            spoofing: Some(false),
            integrity: Some("ok".into()),
            ..Epoch::default()
        };
        let in_threat = t >= onset && t < end;
        let recovering = t >= end && t < end + recover_after;
        if in_threat || recovering {
            let u = t - onset;
            let jam = sc.class == "jamming";
            match b {
                Behaviour::Good => {
                    if u >= react_s && in_threat {
                        if jam {
                            ep.fix_valid = Some(false);
                            ep.jamming = Some(true);
                        } else {
                            ep.spoofing = Some(true);
                            ep.integrity = Some("unsafe".into());
                        }
                    }
                    // A good device rejects the counterfeit: no offset is applied. While it
                    // re-acquires after the threat it reports no fix.
                    if recovering {
                        ep.fix_valid = Some(false);
                    }
                }
                Behaviour::Bad => {
                    let (de, dn, dt) = if jam {
                        let w = u.min(end - onset);
                        (w, 0.0, 5.0 * w)
                    } else {
                        threat_effect(sc, u.min(end - onset))
                    };
                    e += de;
                    nn += dn;
                    te += dt;
                }
            }
        }
        ep.lat_deg = Some(i.truth_lat_deg + nn / mlat);
        ep.lon_deg = Some(i.truth_lon_deg + e / mlon);
        ep.height_m = Some(i.truth_height_m);
        if sc.has_domain("timing") {
            ep.time_error_ns = Some(te);
        }
        if !sc.has_domain("position") {
            ep.lat_deg = None;
            ep.lon_deg = None;
            ep.height_m = None;
        }
        epochs.push(ep);
    }
    DeviceLog {
        format: FORMAT.into(),
        device: Device {
            name: match b {
                Behaviour::Good => "SYNTHETIC well-behaved device".into(),
                Behaviour::Bad => "SYNTHETIC device that follows the threat".into(),
            },
            firmware: None,
            serial: None,
        },
        scenario: sc.id.clone(),
        truth: None,
        source: Some("synthetic".into()),
        epochs,
    }
}
