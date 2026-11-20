// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Incident to regression: a field log and an incident description become a reproducible
//! neutral scenario, plus the field log aligned to the scenario clock (so the incident's
//! own verdict can be reproduced offline, and every later build re-run against it).
//!
//! The incident file (format [`INCIDENT_FORMAT`], TOML):
//!
//! ```toml
//! format = "kshana-testkit-incident/1"
//! id = "inc-2026-031"
//! title = "Supply vessel: DP GNSS position walked off while on station"
//! description = "Bridge reported both DGNSS receivers drifting; the gyro and the taut wire stayed steady."
//! kind = "spoofing"            # jamming | spoofing | meaconing | time-spoofing
//! onset_s = 600                # seconds from the first log epoch
//! end_s = 1500
//! platform = "vessel"
//! # optional: a surveyed truth (static); otherwise the track outside the incident is truth
//! # truth_lat_deg = 57.1
//! # truth_lon_deg = 1.9
//! # known threat parameters (otherwise stated defaults, recorded as assumptions)
//! # js_db = 40
//! # delay_ns = 2000
//! # time_offset_ns = 1500
//! detect_within_s = 30
//! recover_within_s = 120
//! ```
//!
//! What is estimated from the log (every estimate is listed in the scenario's
//! `[incident.derived]` with how it was made):
//!
//! * **Truth**: the surveyed position when given; else, when the clean fixes (valid,
//!   unflagged) before the onset (or, with fewer than 10 of them, outside the incident) stay
//!   within 30 m of their mean, that mean (static); else the clean track outside the
//!   incident, one point per 10 s, bridged in a straight line across the incident. A
//!   receiver that stays wrong after the incident spoils a moving truth: give a surveyed
//!   track then.
//! * **Spoofing**: the offset of each fix in the incident from truth; the largest offset,
//!   its bearing and the time to reach 90 % of it give a `position_drag` (a `simplistic` step
//!   when that time is under 2 s). A time spoof uses the logged time errors when the log has
//!   them, else `time_offset_ns`.
//! * **Jamming**: the share of the incident without a valid fix is reported; J/S is the
//!   incident's `js_db`, else 40 dB, stated as an assumption.
//! * **Meaconing**: `delay_ns` from the incident (a log cannot tell it apart from a drag).
//!
//! The scenario is MODELLED: a parameter fit to one log, not a reconstruction of the
//! signals.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::library::{default_criteria, L1_HZ};
use super::spec::{
    en, Criterion, Expected, IncidentRef, Metric, Op, Platform, Scene, Spec, Technique, Threat,
    ThreatType, TrackPoint, Trajectory, FORMAT,
};
use crate::pics::log::DeviceLog;
use kshana_eloran::sync::degrade::JamKind;

pub const INCIDENT_FORMAT: &str = "kshana-testkit-incident/1";

fn d_det() -> f64 {
    30.0
}
fn d_rec() -> f64 {
    120.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incident {
    pub format: String,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// `jamming`, `spoofing`, `meaconing` or `time-spoofing`.
    pub kind: String,
    pub onset_s: f64,
    pub end_s: f64,
    #[serde(default)]
    pub platform: Platform,
    /// UTC of the first log epoch (default 2026-01-01T00:00:00Z, recorded as derived: the
    /// scenario clock is relative to the first epoch).
    #[serde(default)]
    pub start_utc: Option<String>,
    #[serde(default)]
    pub truth_lat_deg: Option<f64>,
    #[serde(default)]
    pub truth_lon_deg: Option<f64>,
    #[serde(default)]
    pub truth_height_m: Option<f64>,
    #[serde(default)]
    pub js_db: Option<f64>,
    #[serde(default)]
    pub delay_ns: Option<f64>,
    #[serde(default)]
    pub time_offset_ns: Option<f64>,
    #[serde(default)]
    pub sensors: Vec<String>,
    #[serde(default = "d_det")]
    pub detect_within_s: f64,
    #[serde(default = "d_rec")]
    pub recover_within_s: f64,
}

impl Incident {
    pub fn parse(src: &str) -> Result<Incident, String> {
        let i: Incident = toml::from_str(src).map_err(|e| format!("incident: {e}"))?;
        if i.format != INCIDENT_FORMAT {
            return Err(format!(
                "incident format '{}' (expected {INCIDENT_FORMAT})",
                i.format
            ));
        }
        if !["jamming", "spoofing", "meaconing", "time-spoofing"].contains(&i.kind.as_str()) {
            return Err(format!(
                "incident kind '{}' (jamming, spoofing, meaconing or time-spoofing)",
                i.kind
            ));
        }
        if !(i.onset_s >= 0.0 && i.end_s > i.onset_s) {
            return Err("incident: need 0 <= onset_s < end_s".into());
        }
        if i.kind == "meaconing" && i.delay_ns.is_none() {
            return Err("incident: meaconing needs delay_ns (a log cannot measure it)".into());
        }
        Ok(i)
    }
}

/// The converter's output: the scenario, and the field log on the scenario clock.
pub struct Converted {
    pub spec: Spec,
    pub observed: DeviceLog,
}

/// Convert `log` (epoch times from its first epoch) and `inc` into a scenario.
pub fn convert(inc: &Incident, log: &DeviceLog) -> Result<Converted, String> {
    log.validate()?;
    let t0 = log.epochs[0].t_s;
    let mut observed = log.clone();
    for e in &mut observed.epochs {
        e.t_s -= t0;
    }
    observed.scenario = inc.id.clone();
    let duration = observed
        .epochs
        .last()
        .map(|e| e.t_s)
        .unwrap_or(0.0)
        .max(inc.end_s + 1.0);
    let mut derived = BTreeMap::new();
    let (on, off) = (inc.onset_s, inc.end_s);
    let clean: Vec<(f64, f64, f64, f64)> = observed
        .epochs
        .iter()
        .filter(|e| (e.t_s < on || e.t_s >= off) && e.fix_valid != Some(false) && !e.flagged())
        .filter_map(|e| Some((e.t_s, e.lat_deg?, e.lon_deg?, e.height_m.unwrap_or(0.0))))
        .collect();
    let traj = if let (Some(la), Some(lo)) = (inc.truth_lat_deg, inc.truth_lon_deg) {
        derived.insert(
            "truth".into(),
            "surveyed position from the incident file".into(),
        );
        Trajectory {
            platform: inc.platform,
            lat_deg: la,
            lon_deg: lo,
            height_m: inc.truth_height_m.unwrap_or(0.0),
            waypoints: vec![],
            samples: vec![],
        }
    } else {
        if clean.is_empty() {
            return Err("the log has no valid, unflagged fix outside the incident to take truth from; give truth_lat_deg/truth_lon_deg".into());
        }
        // Truth from the epochs before the onset when there are enough of them (a receiver
        // may stay wrong for a while after an incident); else from every clean epoch.
        let pre: Vec<(f64, f64, f64, f64)> = clean.iter().copied().filter(|c| c.0 < on).collect();
        let base = if pre.len() >= 10 { &pre } else { &clean };
        let n = base.len() as f64;
        let mean = (
            base.iter().map(|c| c.1).sum::<f64>() / n,
            base.iter().map(|c| c.2).sum::<f64>() / n,
            base.iter().map(|c| c.3).sum::<f64>() / n,
        );
        let spread = base
            .iter()
            .map(|c| {
                let (e, nn) = en((mean.0, mean.1), (c.1, c.2));
                (e * e + nn * nn).sqrt()
            })
            .fold(0.0, f64::max);
        if spread <= 30.0 {
            derived.insert("truth".into(), format!("static: mean of {} clean fixes (before the onset when there are 10 or more; spread {spread:.1} m)", base.len()));
            Trajectory {
                platform: inc.platform,
                lat_deg: mean.0,
                lon_deg: mean.1,
                height_m: mean.2,
                waypoints: vec![],
                samples: vec![],
            }
        } else {
            let mut pts: Vec<TrackPoint> = Vec::new();
            for c in &clean {
                if pts.last().is_none_or(|p| c.0 - p.t_s >= 10.0) {
                    pts.push(TrackPoint {
                        t_s: c.0,
                        lat_deg: c.1,
                        lon_deg: c.2,
                        height_m: c.3,
                    });
                }
            }
            derived.insert("truth".into(), format!("moving: {} track points (10 s) from clean fixes, bridged in a straight line across the incident", pts.len()));
            Trajectory {
                platform: inc.platform,
                lat_deg: pts[0].lat_deg,
                lon_deg: pts[0].lon_deg,
                height_m: pts[0].height_m,
                waypoints: vec![],
                samples: pts,
            }
        }
    };
    let dur = off - on;
    let mut threat = Threat {
        id: "incident".into(),
        start_s: on,
        duration_s: dur,
        ..Threat::default()
    };
    let in_inc: Vec<_> = observed
        .epochs
        .iter()
        .filter(|e| e.t_s >= on && e.t_s < off)
        .collect();
    match inc.kind.as_str() {
        "jamming" => {
            let lost = in_inc
                .iter()
                .filter(|e| e.fix_valid == Some(false) || e.lat_deg.is_none())
                .count();
            derived.insert(
                "fix_loss".into(),
                format!(
                    "{lost} of {} incident epochs without a valid fix",
                    in_inc.len()
                ),
            );
            derived.insert(
                "js_db".into(),
                match inc.js_db {
                    Some(_) => "from the incident file".into(),
                    None => "ASSUMPTION: 40 dB (the log carries no J/S)".into(),
                },
            );
            threat = Threat {
                kind: Some(ThreatType::Jamming),
                waveform: Some(JamKind::Wideband),
                center_hz: Some(L1_HZ),
                bandwidth_hz: Some(2.0e6),
                js_db: Some(inc.js_db.unwrap_or(40.0)),
                ..threat
            };
        }
        "meaconing" => {
            derived.insert("delay_ns".into(), "from the incident file".into());
            threat = Threat {
                kind: Some(ThreatType::Spoofing),
                technique: Some(Technique::Meaconing),
                delay_ns: inc.delay_ns,
                ..threat
            };
        }
        "time-spoofing" => {
            let te: Vec<(f64, f64)> = in_inc
                .iter()
                .filter_map(|e| Some((e.t_s - on, e.time_error_ns?)))
                .collect();
            let (peak, rate) = if let Some(p) = te
                .iter()
                .map(|x| x.1)
                .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            {
                // least-squares slope through the origin
                let sxx: f64 = te.iter().map(|x| x.0 * x.0).sum();
                let sxy: f64 = te.iter().map(|x| x.0 * x.1).sum();
                derived.insert(
                    "time_push".into(),
                    format!("fitted to {} logged time errors", te.len()),
                );
                (p, if sxx > 0.0 { sxy / sxx } else { 0.0 })
            } else {
                let p = inc
                    .time_offset_ns
                    .ok_or("time-spoofing: the log has no time error; give time_offset_ns")?;
                derived.insert(
                    "time_push".into(),
                    "time_offset_ns from the incident file, reached at the end".into(),
                );
                (p, p / dur)
            };
            threat = Threat {
                kind: Some(ThreatType::Spoofing),
                technique: Some(Technique::TimePush),
                push_rate_ns_per_s: Some(rate),
                time_offset_ns: Some(peak),
                ..threat
            };
        }
        _ => {
            // spoofing: offsets from truth
            let offs: Vec<(f64, f64, f64)> = in_inc
                .iter()
                .filter(|e| e.fix_valid != Some(false))
                .filter_map(|e| {
                    let tr = traj.at(e.t_s);
                    let (de, dn) = en((tr.lat_deg, tr.lon_deg), (e.lat_deg?, e.lon_deg?));
                    Some((e.t_s - on, de, dn))
                })
                .collect();
            let Some(&(tp, pe, pn)) = offs
                .iter()
                .max_by(|a, b| a.1.hypot(a.2).total_cmp(&b.1.hypot(b.2)))
            else {
                return Err(
                    "spoofing: no valid fix inside the incident to estimate the offset from".into(),
                );
            };
            let peak = pe.hypot(pn);
            let bearing = pe.atan2(pn).to_degrees().rem_euclid(360.0);
            let ramp = offs
                .iter()
                .find(|o| o.1.hypot(o.2) >= 0.9 * peak)
                .map(|o| o.0)
                .unwrap_or(tp);
            derived.insert("offset".into(), format!("largest offset {peak:.1} m at {bearing:.0}° true, {tp:.0} s after onset; 90 % reached after {ramp:.0} s"));
            threat = if ramp < 2.0 {
                Threat {
                    kind: Some(ThreatType::Spoofing),
                    technique: Some(Technique::Simplistic),
                    offset_east_m: Some(pe),
                    offset_north_m: Some(pn),
                    ..threat
                }
            } else {
                Threat {
                    kind: Some(ThreatType::Spoofing),
                    technique: Some(Technique::PositionDrag),
                    final_offset_m: Some(peak),
                    drag_bearing_deg: Some(bearing),
                    ramp_s: ramp.min(dur),
                    ..threat
                }
            };
        }
    }
    let mut criteria = default_criteria(std::slice::from_ref(&threat));
    for c in &mut criteria {
        match c.metric {
            Metric::DetectionLatencyS => c.value = inc.detect_within_s,
            Metric::RecoveryS => c.value = inc.recover_within_s,
            _ => {}
        }
    }
    if inc.kind != "time-spoofing" && inc.kind != "meaconing" {
        criteria.push(Criterion {
            id: "baseline-quiet".into(),
            metric: Metric::FalseAlarms,
            window: None,
            threat: None,
            op: Op::Le,
            value: 0.0,
            threshold: None,
            required: false,
            level: None,
            note: "informative: warnings before the onset".into(),
        });
    }
    let log_sha = hex::encode(Sha256::digest(log.to_json().as_bytes()));
    let start_utc = inc.start_utc.clone().unwrap_or_else(|| {
        derived.insert(
            "start_utc".into(),
            "not given: 2026-01-01T00:00:00Z used (the scenario clock is relative)".into(),
        );
        "2026-01-01T00:00:00Z".into()
    });
    let spec = Spec {
        format: FORMAT.into(),
        id: inc.id.clone(),
        version: 1,
        title: inc.title.clone(),
        description: format!("Regression scenario from incident {}: {}", inc.id, inc.description),
        evidence: "MODELLED".into(),
        modelled_on: vec![format!("Field incident {} (one log; parameters fitted, not a signal reconstruction)", inc.id)],
        tags: vec!["incident".into(), inc.kind.clone()],
        scene: Scene { start_utc, duration_s: duration },
        signals: vec![],
        trajectory: traj,
        environment: super::spec::Environment {
            sensors: inc.sensors.clone(),
            ..Default::default()
        },
        threats: vec![threat],
        expected: Expected {
            behaviour: format!("The receiver should warn within {:.0} s of onset, never output the counterfeit or degraded fix without warning, and recover within {:.0} s of the end.", inc.detect_within_s, inc.recover_within_s),
            withdraw_counts_as_detection: inc.kind == "jamming",
            ..Expected::default()
        },
        criteria,
        incident: Some(IncidentRef {
            id: inc.id.clone(),
            title: inc.title.clone(),
            log_sha256: log_sha,
            log_source: log.source.clone().unwrap_or_else(|| "json".into()),
            derived,
        }),
    };
    spec.validate()?;
    Ok(Converted { spec, observed })
}
