// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Threat-library scenarios ([`crate::threat_library`]) as neutral-spec threats, at
//! signal-parameter level.
//!
//! The library's analogue events describe what a receiver *shows* (a C/N0 drop, a drag-off,
//! a time step); a simulator needs what is *transmitted*. The mapping is stated, not
//! measured:
//!
//! * `jamming` (C/N0 drop `d` dB) → a wideband jammer at L1 (1575.42 MHz, 2 MHz) with
//!   J/S = `d` + [`JS_MARGIN_DB`] dB. Representative: the C/N0 a receiver loses for a given
//!   J/S depends on its front end and processing gain.
//! * `drag-off` → `position_drag` to `final_offset_m` over the ramp, along the stated
//!   bearing (a relative bearing is turned into a true one with the truth heading at onset).
//! * `time-spoof` → `time_push` reaching `offset_s` over the ramp (a step when there is none).
//! * `replay-delay` → `meaconing` with the stated delay.
//! * `natural-degradation` is not a threat; it is left out (an obscuration belongs in
//!   `[environment]`).
//! * A timing profile alone: `denial` → L1 jamming for its duration; `time-push` and
//!   `time-step` → `time_push` / `simplistic` time spoofing.

use kshana::nmea_synth::config::EventKind;
use kshana_eloran::sync::degrade::JamKind;

use super::spec::{
    Criterion, Expected, Metric, Op, Platform, Scene, Spec, Technique, Threat, ThreatType,
    Trajectory, Waypoint, FORMAT,
};

/// J/S above the analogue's C/N0 drop when a library jamming event becomes a jammer, dB.
pub const JS_MARGIN_DB: f64 = 10.0;
/// GPS L1 centre frequency, Hz.
pub const L1_HZ: f64 = 1_575_420_000.0;
const KN: f64 = 1852.0 / 3600.0;

fn jammer(id: String, start: f64, dur: f64, js: f64, lib: &str) -> Threat {
    Threat {
        id,
        kind: Some(ThreatType::Jamming),
        library: Some(lib.into()),
        start_s: start,
        duration_s: dur,
        waveform: Some(JamKind::Wideband),
        center_hz: Some(L1_HZ),
        bandwidth_hz: Some(2.0e6),
        js_db: Some(js),
        ..Threat::default()
    }
}

/// The library scenario `lib` as threats, onsets shifted by `offset_s`; ids are
/// `<prefix>-<n>`. `heading_deg` turns a relative drag bearing into a true one. With
/// `prefer_timing` (a timing receiver), a scenario's timing profile is used when it has one
/// (the analogue's time effects are scaled for an NMEA display, the profile for a clock).
pub fn threats(
    lib: &str,
    offset_s: f64,
    prefix: &str,
    heading_deg: f64,
    prefer_timing: bool,
) -> Result<Vec<Threat>, String> {
    let sc = crate::threat_library::get(lib).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    let use_analogue = sc.analogue.is_some() && !(prefer_timing && sc.timing.is_some());
    if let (true, Some(a)) = (use_analogue, &sc.analogue) {
        for (i, ev) in a.events.iter().enumerate() {
            let id = format!("{prefix}-{}", i + 1);
            let start = offset_s + ev.start_s;
            let dur = ev.duration_s + ev.recovery_s;
            let base = Threat {
                id: id.clone(),
                library: Some(lib.into()),
                start_s: start,
                duration_s: dur,
                ramp_s: ev.ramp_s,
                power_advantage_db: ev.counterfeit_cn0_dbhz.map(|_| 3.0),
                ..Threat::default()
            };
            let t = match ev.kind {
                EventKind::Jamming => jammer(
                    id,
                    start,
                    dur,
                    ev.cn0_drop_db.unwrap_or(20.0) + JS_MARGIN_DB,
                    lib,
                ),
                EventKind::DragOff => Threat {
                    kind: Some(ThreatType::Spoofing),
                    technique: Some(Technique::PositionDrag),
                    final_offset_m: ev.final_offset_m,
                    drag_bearing_deg: Some(
                        ev.bearing_deg
                            .unwrap_or(heading_deg + ev.relative_bearing_deg.unwrap_or(0.0))
                            .rem_euclid(360.0),
                    ),
                    ..base
                },
                EventKind::TimeSpoof => {
                    let off_ns = ev.offset_s.unwrap_or(0.0) * 1e9;
                    if ev.ramp_s > 0.0 {
                        Threat {
                            kind: Some(ThreatType::Spoofing),
                            technique: Some(Technique::TimePush),
                            push_rate_ns_per_s: Some(off_ns / ev.ramp_s),
                            time_offset_ns: Some(off_ns),
                            ..base
                        }
                    } else {
                        Threat {
                            kind: Some(ThreatType::Spoofing),
                            technique: Some(Technique::Simplistic),
                            time_offset_ns: Some(off_ns),
                            ..base
                        }
                    }
                }
                EventKind::ReplayDelay => Threat {
                    kind: Some(ThreatType::Spoofing),
                    technique: Some(Technique::Meaconing),
                    delay_ns: Some(ev.delay_s.unwrap_or(0.0) * 1e9),
                    ..base
                },
                EventKind::NaturalDegradation => continue,
            };
            out.push(t);
        }
    } else if let Some(tp) = &sc.timing {
        let id = format!("{prefix}-1");
        let start = offset_s + tp.start_s;
        let base = Threat {
            id: id.clone(),
            library: Some(lib.into()),
            start_s: start,
            duration_s: tp.duration_s,
            ..Threat::default()
        };
        out.push(match tp.kind.as_str() {
            "denial" => jammer(id, start, tp.duration_s, 40.0, lib),
            "time-push" => Threat {
                kind: Some(ThreatType::Spoofing),
                technique: Some(Technique::TimePush),
                push_rate_ns_per_s: tp.rate_s_per_s.map(|r| r * 1e9),
                time_offset_ns: tp.max_offset_s.map(|m| m * 1e9),
                ..base
            },
            _ => Threat {
                kind: Some(ThreatType::Spoofing),
                technique: Some(Technique::Simplistic),
                time_offset_ns: tp.step_s.map(|s| s * 1e9),
                ..base
            },
        });
    }
    if out.is_empty() {
        return Err(format!(
            "threat library scenario '{lib}' has no event a simulator can express"
        ));
    }
    Ok(out)
}

/// The default criteria for a set of threats: detection, no misleading output and
/// recovery, each tied to the resilience level it evidences (modelled on the DHS RPCF).
pub fn default_criteria(threats: &[Threat]) -> Vec<Criterion> {
    let timing = threats.iter().any(|t| {
        matches!(
            t.technique,
            Some(Technique::TimePush) | Some(Technique::Meaconing)
        ) || t.time_offset_ns.is_some()
    });
    let c = |id: &str, metric, op, value, level| Criterion {
        id: id.into(),
        metric,
        window: None,
        threat: None,
        op,
        value,
        threshold: None,
        required: true,
        level: Some(level),
        note: String::new(),
    };
    let mut v = vec![
        c("detect", Metric::DetectionLatencyS, Op::Le, 30.0, 2),
        c("no-misleading", Metric::MisleadingEpochs, Op::Le, 0.0, 2),
        c("recover", Metric::RecoveryS, Op::Le, 120.0, 1),
    ];
    if timing {
        v.push(c(
            "time-bounded",
            Metric::MaxUnflaggedTimeErrorNs,
            Op::Le,
            1000.0,
            3,
        ));
    }
    v
}

/// A whole neutral scenario from a library scenario: its analogue's time base, track and
/// events, with the default criteria.
pub fn spec(lib: &str) -> Result<Spec, String> {
    let sc = crate::threat_library::get(lib).map_err(|e| e.to_string())?;
    let (start_utc, duration_s, traj) = match &sc.analogue {
        Some(a) => {
            let v = &a.vessel;
            let mut speed = v.speed_kn;
            let wps = a
                .waypoints
                .iter()
                .map(|w| {
                    speed = w.speed_kn.unwrap_or(speed);
                    Waypoint {
                        lat_deg: w.lat_deg,
                        lon_deg: w.lon_deg,
                        height_m: None,
                        speed_mps: speed * KN,
                    }
                })
                .collect();
            let platform = if sc.analogue_is_static() {
                Platform::Static
            } else {
                Platform::Vessel
            };
            (
                a.scenario.start_utc.clone(),
                a.scenario.duration_s,
                Trajectory {
                    platform,
                    lat_deg: v.lat_deg,
                    lon_deg: v.lon_deg,
                    height_m: 0.0,
                    waypoints: wps,
                    samples: vec![],
                },
            )
        }
        None => {
            let d = sc.timing.as_ref().map(|t| t.duration_s).unwrap_or(3600.0);
            (
                "2026-01-01T00:00:00Z".to_string(),
                d + 600.0,
                Trajectory {
                    platform: Platform::Timing,
                    lat_deg: 59.437,
                    lon_deg: 24.7536,
                    height_m: 30.0,
                    waypoints: vec![],
                    samples: vec![],
                },
            )
        }
    };
    let probe = Spec {
        format: FORMAT.into(),
        id: lib.into(),
        version: 1,
        title: sc.title.clone(),
        description: String::new(),
        evidence: "MODELLED".into(),
        modelled_on: vec![],
        tags: vec![],
        scene: Scene {
            start_utc,
            duration_s,
        },
        signals: vec![],
        trajectory: traj,
        environment: Default::default(),
        threats: vec![],
        expected: Expected::default(),
        criteria: vec![],
        incident: None,
    };
    let first = sc
        .analogue
        .as_ref()
        .and_then(|a| a.events.first())
        .map(|e| e.start_s)
        .unwrap_or(0.0);
    let heading = probe.trajectory.at(first).heading_deg;
    let threats = threats(lib, 0.0, "t", heading, false)?;
    let threats: Vec<Threat> = threats
        .into_iter()
        .filter(|t| t.start_s < duration_s)
        .map(|mut t| {
            t.duration_s = t.duration_s.min(duration_s - t.start_s);
            t
        })
        .collect();
    let s = Spec {
        description: format!(
            "Neutral form of threat-library scenario {lib} (edition {}). {}",
            crate::threat_library::EDITION,
            crate::threat_library::LABEL
        ),
        modelled_on: vec![format!(
            "Threat library {lib}: published descriptions, representative parameters"
        )],
        tags: vec!["threat-library".into(), sc.family.as_str().into()],
        expected: Expected {
            behaviour: "The receiver warns of the threat (or withdraws its fix under jamming), \
                        never outputs a misleading fix without warning, and recovers after it."
                .into(),
            ..Expected::default()
        },
        criteria: default_criteria(&threats),
        threats,
        ..probe
    };
    s.validate()?;
    Ok(s)
}
