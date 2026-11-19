// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The compiler and validator: a neutral scenario in each simulator adapter's native form,
//! through the existing adapters of `kshana-eloran::sync`, and a report of what each adapter
//! cannot express.
//!
//! * **Profile adapters** (`skydel`, `spirent`, `rs-smbv`, `labsat`): the threats become a
//!   `[gnss_degradation]` timeline rendered by
//!   [`kshana_eloran::sync::degrade::translate`] for the adapter's command profile; the
//!   setup (start epoch, trajectory) is rendered from the same profile; the trajectory is
//!   written as NMEA and CSV; a spoofer is a schedule marker plus its per-second counterfeit
//!   offsets, for the simulator's own spoofing scenario (as PICS exports do).
//! * **gps-sdr-sim**: the command line and a 10 Hz LLH motion file; it has no run-time
//!   controls, so every timed event is listed with how to realise it otherwise
//!   ([`kshana_eloran::sync::gpssdrsim::degradation_support`]). No spoofer input is written
//!   for it: a counterfeit signal file is attack IQ, which only the vetted `attack-iq` route
//!   produces.
//!
//! Each finding is `native` (the adapter expresses it), `approximated` (expressed with a
//! stated loss), `manual` (the operator sets it in the simulator, as the finding says) or
//! `unsupported`. Profiles that are unconfirmed templates stay unconfirmed here.

use std::collections::BTreeMap;
use std::path::Path;

use kshana_eloran::sync::degrade::{translate, DegradationEvent, GnssDegradation};
use kshana_eloran::sync::export::{
    CsvTrajectoryExport, NmeaTrajectoryExport, TrajSample, TrajectoryExport,
};
use kshana_eloran::sync::gpssdrsim::{self, GpsSdrSimConfig, MotionInput};
use kshana_eloran::sync::profile::{ops, CommandProfile, Constellation};
use kshana_eloran::sync::{StartEpoch, UtcStamp};
use serde::Serialize;

use super::spec::{AtmosModel, Multipath, Spec, ThreatType};

/// The adapters a scenario compiles for.
pub const ADAPTERS: [&str; 5] = ["skydel", "spirent", "rs-smbv", "labsat", "gps-sdr-sim"];
/// The format of a compile report.
pub const COMPILED_FORMAT: &str = "kshana-testkit-compiled/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    Native,
    Approximated,
    Manual,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub feature: String,
    pub support: Support,
    pub detail: String,
}

/// What one adapter can do with one scenario.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Validation {
    pub scenario: String,
    pub adapter: String,
    pub profile: String,
    pub profile_confirmed: bool,
    /// No finding is `unsupported`.
    pub expressible: bool,
    pub counts: BTreeMap<String, usize>,
    pub findings: Vec<Finding>,
}

fn profile(name: &str) -> Result<Option<CommandProfile>, String> {
    use kshana_eloran::sync::{labsat, rs_smbv, skydel, spirent};
    Ok(match name {
        "skydel" => Some(skydel::default_profile()),
        "spirent" => Some(spirent::default_profile()),
        "rs-smbv" => Some(rs_smbv::default_profile()),
        "labsat" => Some(labsat::default_profile()),
        "gps-sdr-sim" => None,
        other => {
            return Err(format!(
                "unknown adapter '{other}' (expected {})",
                ADAPTERS.join(", ")
            ))
        }
    })
}

/// The scenario's threats as a degradation timeline (scene time).
pub fn degradation(spec: &Spec) -> GnssDegradation {
    let mut events = Vec::new();
    for t in &spec.threats {
        let Some(kind) = t.kind else { continue };
        events.push(match kind {
            ThreatType::Jamming => DegradationEvent::Jamming {
                at_s: t.start_s,
                duration_s: t.duration_s,
                id: t.id.clone(),
                kind: t.waveform.unwrap_or_default(),
                center_hz: t.center_hz.unwrap_or(0.0),
                bandwidth_hz: t.bandwidth_hz.unwrap_or(0.0),
                js_db: t.js_db.unwrap_or(0.0),
            },
            ThreatType::Spoofing => DegradationEvent::Spoofing {
                at_s: t.start_s,
                until_s: Some(t.end_s()),
                label: format!(
                    "{} ({})",
                    t.id,
                    t.technique.map(|x| x.as_str()).unwrap_or_default()
                ),
            },
            ThreatType::Cn0Offset => DegradationEvent::Cn0Offset {
                at_s: t.start_s,
                until_s: Some(t.end_s()),
                constellation: t.constellation,
                signal: t.signal.clone(),
                offset_db: t.offset_db.unwrap_or(0.0),
            },
            ThreatType::PowerStep => DegradationEvent::PowerStep {
                at_s: t.start_s,
                constellation: t.constellation,
                signal: t.signal.clone(),
                step_db: t.offset_db.unwrap_or(0.0),
            },
            ThreatType::Outage => DegradationEvent::Outage {
                at_s: t.start_s,
                duration_s: t.duration_s,
                constellation: t.constellation,
                signal: t.signal.clone(),
            },
            ThreatType::SatMask => DegradationEvent::SatMask {
                at_s: t.start_s,
                until_s: Some(t.end_s()),
                constellation: t.constellation.unwrap_or(Constellation::Gps),
                prns: t.prns.clone(),
            },
        });
    }
    GnssDegradation {
        description: format!("{}: {}", spec.id, spec.title),
        events,
    }
}

fn f(feature: impl Into<String>, support: Support, detail: impl Into<String>) -> Finding {
    Finding {
        feature: feature.into(),
        support,
        detail: detail.into(),
    }
}

/// The command profile of a profile adapter (an error for gps-sdr-sim, which has none).
pub fn adapter_profile(name: &str) -> Result<CommandProfile, String> {
    profile(name)?.ok_or_else(|| format!("{name} has no remote-control profile"))
}

/// What `adapter` can express of `spec` (resolved first).
pub fn validate(spec: &Spec, adapter: &str) -> Result<Validation, String> {
    validate_with(spec, adapter, None)
}

/// [`validate`] with a lab's own (confirmed) profile in place of the built-in template.
pub fn validate_with(
    spec: &Spec,
    adapter: &str,
    custom: Option<&CommandProfile>,
) -> Result<Validation, String> {
    let spec = spec.resolve()?;
    let prof = match custom {
        Some(p) => Some(p.clone()),
        None => profile(adapter)?,
    };
    let mut v = Vec::new();
    let moving = !spec.trajectory.is_static();
    let env = &spec.environment;
    match &prof {
        Some(p) => {
            // Start epoch and trajectory.
            v.push(if p.supports(ops::SET_START_TIME) {
                f("start-epoch", Support::Native, "set_start_time command")
            } else {
                f(
                    "start-epoch",
                    Support::Manual,
                    "no start-time command: the epoch is in the file or set at the instrument",
                )
            });
            let replay_only = adapter == "labsat";
            v.push(if replay_only {
                f("trajectory", Support::Manual, "LabSat plays a recorded (or separately generated) file: the trajectory must already be in it")
            } else if !moving {
                if p.supports(ops::LOAD_TRAJECTORY) || p.supports(ops::STREAM_SAMPLE) {
                    f("trajectory", Support::Native, "static position")
                } else {
                    f("trajectory", Support::Manual, "position is fixed by the recording/file the instrument plays")
                }
            } else if p.supports(ops::LOAD_TRAJECTORY) {
                f("trajectory", Support::Native, "trajectory file loaded (load_trajectory); trajectory.nmea and trajectory.csv written")
            } else if p.supports(ops::STREAM_SAMPLE) {
                f("trajectory", Support::Native, "motion streamed (stream_sample)")
            } else {
                f("trajectory", Support::Unsupported, "no trajectory command: the motion must already be in the file the instrument plays (record or generate it separately)")
            });
            // Signals.
            for s in spec.signal_sets() {
                let c = s.constellation.key();
                let known = p.constellations.is_empty() || p.constellations.contains_key(c);
                let sigs_known =
                    p.signals.is_empty() || s.signals.iter().all(|x| p.signals.contains_key(x));
                v.push(if p.supports(ops::SET_POWER_OFFSET) || p.supports(ops::SIGNAL_ON) {
                    if known && sigs_known {
                        f(format!("signals {c}"), Support::Native, s.signals.join(", "))
                    } else {
                        f(format!("signals {c}"), Support::Unsupported, format!("the profile has no name for {c} {}", s.signals.join(", ")))
                    }
                } else {
                    f(format!("signals {c}"), Support::Manual, "no signal commands: the constellations are those of the file the instrument plays")
                });
            }
            // Environment.
            for (name, set) in [
                ("ionosphere", env.ionosphere != AtmosModel::Nominal),
                ("troposphere", env.troposphere != AtmosModel::Nominal),
                ("multipath", env.multipath != Multipath::None),
                ("obscuration", env.obscuration.is_some()),
            ] {
                if set {
                    v.push(f(
                        name,
                        Support::Manual,
                        "no command in the profile: set it in the simulator's own scenario",
                    ));
                }
            }
            // Threats.
            let sched = translate(&degradation(&spec), p)?;
            for u in &sched.unsupported {
                v.push(f("threat", Support::Unsupported, u.clone()));
            }
            for t in &spec.threats {
                if t.kind == Some(ThreatType::Spoofing) && replay_only {
                    v.push(f(
                        format!("threat {}", t.id),
                        Support::Unsupported,
                        "LabSat replays a file and generates no spoofer: the counterfeit must already be in the recording (not produced here)",
                    ));
                } else if t.kind == Some(ThreatType::Spoofing) {
                    v.push(f(
                        format!("threat {}", t.id),
                        Support::Approximated,
                        "spoofer as a schedule marker and per-second counterfeit offsets (spoofer.csv): configure it in the simulator's own spoofing scenario",
                    ));
                } else if !sched.unsupported.iter().any(|u| u.contains(&t.id)) {
                    v.push(f(
                        format!("threat {}", t.id),
                        Support::Native,
                        "timed command",
                    ));
                }
            }
            if !p.confirmed {
                v.push(f(
                    "profile",
                    Support::Manual,
                    format!("{} is an unconfirmed template: check every command against the vendor's manual before use", p.name),
                ));
            }
        }
        None => {
            v.push(f("start-epoch", Support::Native, "-t"));
            v.push(f(
                "trajectory",
                Support::Native,
                if moving {
                    "user motion file (-x, 10 Hz LLH)"
                } else {
                    "static position (-l)"
                },
            ));
            for s in spec.signal_sets() {
                let c = s.constellation.key();
                for sig in &s.signals {
                    v.push(if s.constellation == Constellation::Gps && sig == "L1CA" {
                        f(format!("signals {c}"), Support::Native, "GPS L1 C/A")
                    } else {
                        f(
                            format!("signals {c}"),
                            Support::Unsupported,
                            format!("gps-sdr-sim generates GPS L1 C/A only, not {c} {sig}"),
                        )
                    });
                }
            }
            v.push(match env.ionosphere {
                AtmosModel::Nominal => f("ionosphere", Support::Native, "broadcast model"),
                AtmosModel::Off => f("ionosphere", Support::Native, "-i"),
                AtmosModel::Storm => f("ionosphere", Support::Unsupported, "no storm model"),
            });
            if env.troposphere != AtmosModel::Nominal {
                v.push(f("troposphere", Support::Unsupported, "fixed model only"));
            }
            if env.multipath != Multipath::None {
                v.push(f("multipath", Support::Unsupported, "no multipath model"));
            }
            if env.obscuration.is_some() {
                v.push(f(
                    "obscuration",
                    Support::Manual,
                    "remove obscured satellites from the navigation file per segment",
                ));
            }
            let deg = degradation(&spec);
            // Spoofing markers are covered by the finding below (no spoofer is produced).
            for line in gpssdrsim::degradation_support(&deg)
                .into_iter()
                .filter(|l| !l.contains("spoofing marker"))
            {
                let sup = if line.contains("jammer") {
                    Support::Unsupported
                } else {
                    Support::Manual
                };
                v.push(f("threat", sup, line));
            }
            if spec
                .threats
                .iter()
                .any(|t| t.kind == Some(ThreatType::Spoofing))
            {
                v.push(f("threat spoofer", Support::Unsupported, "no counterfeit signal input is written for gps-sdr-sim: a spoofer signal file is attack IQ (vetted `attack-iq` route only)"));
            }
        }
    }
    for s in &env.sensors {
        v.push(f(
            format!("sensor {s}"),
            Support::Manual,
            "not a GNSS simulator signal: feed it from a synchronised source (the trajectory files carry the truth to drive it)",
        ));
    }
    let mut counts = BTreeMap::new();
    for x in &v {
        let k = serde_json::to_value(x.support)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        *counts.entry(k).or_insert(0) += 1;
    }
    Ok(Validation {
        scenario: spec.id.clone(),
        adapter: adapter.into(),
        profile: prof
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "gps-sdr-sim".into()),
        profile_confirmed: prof.as_ref().is_none_or(|p| p.confirmed),
        expressible: !v.iter().any(|x| x.support == Support::Unsupported),
        counts,
        findings: v,
    })
}

/// The truth sampled at `rate_hz` over the scene.
pub fn samples(spec: &Spec, rate_hz: f64) -> Result<Vec<TrajSample>, String> {
    let start = UtcStamp::parse(&spec.scene.start_utc)?;
    let n = (spec.scene.duration_s * rate_hz + 1e-9).floor() as usize + 1;
    Ok((0..n)
        .map(|k| {
            let t = k as f64 / rate_hz;
            let s = spec.trajectory.at(t);
            TrajSample {
                t_s: t,
                utc: start.plus(t),
                lat_deg: s.lat_deg,
                lon_deg: s.lon_deg,
                h_m: s.height_m,
                heading_deg: s.heading_deg,
                speed_mps: s.speed_mps,
            }
        })
        .collect())
}

/// A compiled scenario: the files to write and the validation.
#[derive(Debug, Clone, Serialize)]
pub struct Compiled {
    pub format: &'static str,
    pub scenario: String,
    pub scenario_version: u32,
    pub scenario_sha256: String,
    pub adapter: String,
    pub label: &'static str,
    pub validation: Validation,
    /// File name → content.
    #[serde(skip)]
    pub files: BTreeMap<String, String>,
    pub file_names: Vec<String>,
}

/// Compile `spec` for `adapter`.
pub fn compile(spec: &Spec, adapter: &str) -> Result<Compiled, String> {
    compile_with(spec, adapter, None)
}

/// [`compile`] with a lab's own (confirmed) profile in place of the built-in template.
pub fn compile_with(
    spec: &Spec,
    adapter: &str,
    custom: Option<&CommandProfile>,
) -> Result<Compiled, String> {
    let r = spec.resolve()?;
    let validation = validate_with(&r, adapter, custom)?;
    let mut files = BTreeMap::new();
    files.insert("scenario.toml".to_string(), r.to_toml());
    let prof = match custom {
        Some(p) => Some(p.clone()),
        None => profile(adapter)?,
    };
    match prof {
        Some(p) => {
            let sched = translate(&degradation(&r), &p)?;
            let epoch = StartEpoch::from_utc_iso(&r.scene.start_utc, p.epoch_scale)?;
            let mut setup = Vec::new();
            for (op, fields) in [
                (ops::CONNECT, BTreeMap::new()),
                (ops::SET_START_TIME, {
                    let mut x = p.time_fields(None);
                    x.extend(epoch.fields());
                    x
                }),
                (ops::LOAD_TRAJECTORY, {
                    let mut x = p.time_fields(None);
                    x.insert("path".into(), "trajectory.nmea".into());
                    x.insert("format".into(), "nmea".into());
                    x.insert("rate_hz".into(), "1".into());
                    x
                }),
            ] {
                if p.supports(op) {
                    let mut fl = p.time_fields(None);
                    fl.extend(fields);
                    setup.extend(p.render(op, &fl)?);
                }
            }
            let mut cmd = format!(
                "# {} for {} ({}). MODELLED. Conducted (cabled) or shielded use only; never radiate.\n# setup\n",
                r.id, p.vendor, p.name
            );
            for l in setup {
                cmd.push_str(&l);
                cmd.push('\n');
            }
            cmd.push_str(&format!("# schedule ({:?})\n", sched.mode));
            for e in &sched.entries {
                for l in &e.lines {
                    cmd.push_str(&format!("{:.3}\t{l}\t# {}\n", e.t_s, e.what));
                }
            }
            files.insert("commands.txt".into(), cmd);
            files.insert(
                "schedule.json".into(),
                serde_json::to_string_pretty(&sched).unwrap(),
            );
            let s1 = samples(&r, 1.0)?;
            files.insert(
                "trajectory.nmea".into(),
                NmeaTrajectoryExport {
                    rate_hz: 1.0,
                    ..NmeaTrajectoryExport::default()
                }
                .render(&s1),
            );
            files.insert(
                "trajectory.csv".into(),
                CsvTrajectoryExport { rate_hz: 1.0 }.render(&s1),
            );
        }
        None => {
            let s10 = samples(&r, gpssdrsim::MOTION_RATE_HZ)?;
            let cfg = GpsSdrSimConfig {
                motion: if r.trajectory.is_static() {
                    MotionInput::StaticLlh {
                        lat_deg: r.trajectory.lat_deg,
                        lon_deg: r.trajectory.lon_deg,
                        h_m: r.trajectory.height_m,
                    }
                } else {
                    MotionInput::UserLlh {
                        path: "motion.csv".into(),
                    }
                },
                start: Some(UtcStamp::parse(&r.scene.start_utc)?),
                duration_s: Some(r.scene.duration_s),
                disable_iono: r.environment.ionosphere == AtmosModel::Off,
                ..GpsSdrSimConfig::default()
            };
            files.insert(
                "run.sh".into(),
                format!(
                    "#!/bin/sh\n# {}: gps-sdr-sim (GPS L1 C/A, file output). MODELLED.\n# Replay the I/Q file conducted (cabled) or shielded only; never radiate.\n# brdc.nav: a RINEX GPS navigation file for the scene date (not supplied).\n{}\n",
                    r.id,
                    cfg.command_line()?
                ),
            );
            if !r.trajectory.is_static() {
                files.insert("motion.csv".into(), gpssdrsim::render_user_motion_llh(&s10));
            }
        }
    }
    // The spoofer's counterfeit offsets, for a commercial simulator's spoofing scenario.
    if adapter != "gps-sdr-sim"
        && adapter != "labsat"
        && r.threats
            .iter()
            .any(|t| t.kind == Some(ThreatType::Spoofing))
    {
        let mut csv =
            String::from("t_s,east_m,north_m,time_ns,counterfeit_lat_deg,counterfeit_lon_deg\n");
        for (a, b) in r.threats.iter().filter_map(|t| {
            (t.kind == Some(ThreatType::Spoofing)).then_some((t.start_s, t.end_s()))
        }) {
            let mut t = a;
            while t <= b + 1e-9 {
                let (e, n, ns) = r.counterfeit_at(t);
                let tr = r.trajectory.at(t);
                let (la, lo) = super::spec::offset(tr.lat_deg, tr.lon_deg, e, n);
                csv.push_str(&format!("{t:.1},{e:.3},{n:.3},{ns:.3},{la:.9},{lo:.9}\n"));
                t += 1.0;
            }
        }
        files.insert("spoofer.csv".into(), csv);
    }
    let file_names = files
        .keys()
        .cloned()
        .chain(["COMPILE.json".to_string()])
        .collect();
    Ok(Compiled {
        format: COMPILED_FORMAT,
        scenario: r.id.clone(),
        scenario_version: r.version,
        scenario_sha256: r.sha256(),
        adapter: adapter.into(),
        label: super::LABEL,
        validation,
        files,
        file_names,
    })
}

/// Write a compiled scenario into `dir` (`COMPILE.json` is the report).
pub fn write(c: &Compiled, dir: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    for (name, text) in &c.files {
        std::fs::write(dir.join(name), text).map_err(io)?;
    }
    std::fs::write(
        dir.join("COMPILE.json"),
        serde_json::to_string_pretty(c).unwrap(),
    )
    .map_err(io)
}
