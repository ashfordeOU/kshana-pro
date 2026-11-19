// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! PICS scenarios exported to GNSS simulators through the existing adapters of
//! `kshana-eloran::sync`: each scenario becomes a `[gnss_degradation]` timeline
//! ([`kshana_eloran::sync::degrade::GnssDegradation`]) rendered with
//! [`kshana_eloran::sync::degrade::translate`] for a simulator command profile (Skydel,
//! Spirent, LabSat, R&S SMBV).
//!
//! What carries over: jammers (CW, chirp, broadband as `wideband`) become timed jammer
//! commands where the profile has them. A spoofer cannot be expressed as a timed command in
//! any of these profiles: it is kept as a schedule marker for the record, and the export
//! states the spoofer's motion ([`super::log::threat_effect`], sampled every second) for the
//! operator to configure in the simulator's own spoofing scenario. Profiles marked
//! unconfirmed in `kshana-eloran` stay unconfirmed here (`profile_confirmed`).

use kshana_eloran::sync::degrade::{translate, DegradationEvent, GnssDegradation, JamKind};
use kshana_eloran::sync::profile::{CommandProfile, CommandSchedule};
use serde::Serialize;

use super::log::threat_effect;
use super::{Scenario, Suite};

/// The simulator profiles PICS exports to.
pub const PROFILES: [&str; 4] = ["skydel", "spirent", "labsat", "rs-smbv"];

pub fn profile(name: &str) -> Result<CommandProfile, String> {
    use kshana_eloran::sync::{labsat, rs_smbv, skydel, spirent};
    Ok(match name {
        "skydel" => skydel::default_profile(),
        "spirent" => spirent::default_profile(),
        "labsat" => labsat::default_profile(),
        "rs-smbv" => rs_smbv::default_profile(),
        other => {
            return Err(format!(
                "unknown simulator profile '{other}' (expected {})",
                PROFILES.join(", ")
            ))
        }
    })
}

/// The scenario as a degradation timeline (scenario time: 0 is the baseline start).
pub fn degradation(suite: &Suite, sc: &Scenario) -> GnssDegradation {
    let (onset, end) = suite.threat_window();
    let p = |k: &str| sc.params.get(k).copied().unwrap_or(0.0);
    let ev = if sc.class == "jamming" {
        DegradationEvent::Jamming {
            at_s: onset,
            duration_s: end - onset,
            id: sc.id.to_ascii_lowercase(),
            kind: match sc.technique.as_str() {
                "cw" => JamKind::Cw,
                "chirp" => JamKind::Chirp,
                _ => JamKind::Wideband,
            },
            center_hz: p("center_hz"),
            bandwidth_hz: p("bandwidth_hz"),
            js_db: p("js_db"),
        }
    } else {
        DegradationEvent::Spoofing {
            at_s: onset,
            until_s: Some(end),
            label: format!("{} ({})", sc.id, sc.technique),
        }
    };
    GnssDegradation {
        description: format!("PICS {}: {}", sc.id, sc.title),
        events: vec![ev],
    }
}

/// One second of the spoofer's counterfeit offset.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpoofSample {
    pub t_s: f64,
    pub east_m: f64,
    pub north_m: f64,
    pub time_ns: f64,
}

/// A scenario exported for one simulator.
#[derive(Debug, Clone, Serialize)]
pub struct Export {
    pub format: &'static str,
    pub suite: String,
    pub scenario: String,
    pub profile: String,
    pub degradation: GnssDegradation,
    pub schedule: CommandSchedule,
    /// The spoofer's counterfeit offset from truth, each second of the threat (empty for
    /// jamming). The simulator's own spoofing scenario must realise it.
    pub spoofer_offsets: Vec<SpoofSample>,
    pub spoofer_params: std::collections::BTreeMap<String, f64>,
    pub notes: Vec<String>,
}

pub const EXPORT_FORMAT: &str = "kshana-pics-simulator-export/1";

pub fn export(suite: &Suite, sc: &Scenario, profile_name: &str) -> Result<Export, String> {
    let prof = profile(profile_name)?;
    let deg = degradation(suite, sc);
    let schedule = translate(&deg, &prof)?;
    let (onset, end) = suite.threat_window();
    let spoofer_offsets = if sc.class == "spoofing" {
        (0..=((end - onset).round() as usize))
            .map(|k| {
                let u = k as f64;
                let (e, n, t) = threat_effect(sc, u);
                SpoofSample {
                    t_s: onset + u,
                    east_m: e,
                    north_m: n,
                    time_ns: t,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut notes = vec![
        "MODELLED: representative threat parameters; the simulator, its calibration and the RF path are the test lab's.".to_string(),
        "Conducted (cabled) or shielded use only; never radiate.".to_string(),
    ];
    if sc.class == "spoofing" {
        notes.push("The spoofer is a schedule marker: configure it in the simulator's own spoofing scenario from spoofer_offsets.".into());
    }
    if !schedule.profile_confirmed {
        notes.push(format!(
            "The {profile_name} command profile is an unconfirmed template: check every command against the vendor's manual before use."
        ));
    }
    if !schedule.unsupported.is_empty() {
        notes.push(format!(
            "Not expressible in this profile: {}",
            schedule.unsupported.join("; ")
        ));
    }
    Ok(Export {
        format: EXPORT_FORMAT,
        suite: format!("{} {}", suite.suite.id, suite.suite.version),
        scenario: sc.id.clone(),
        profile: profile_name.into(),
        degradation: deg,
        schedule,
        spoofer_offsets,
        spoofer_params: sc.params.clone(),
        notes,
    })
}
