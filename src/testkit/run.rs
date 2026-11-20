// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The CI runner: a scenario set against a receiver under test, one signed verdict per
//! scenario per build.
//!
//! Receiver output comes from one of two places:
//!
//! * **Offline** ([`Source::Logs`]): a directory of recorded or replayed receiver output,
//!   `<scenario-id>.json` (PICS device log) or `<scenario-id>.nmea` (NMEA 0183, its first
//!   epoch at scenario time `t0`). A scenario without a log is recorded as `not-run`.
//! * **Live** ([`run_live`]): the scenario compiled for a profile adapter, driven on a lab
//!   simulator over its remote-control port (start epoch, trajectory, timed schedule), while
//!   the receiver's NMEA is read from a TCP port for the scene duration. Conducted (cabled)
//!   or shielded set-ups only: the runner drives the simulator's own controls and never
//!   generates a signal itself.
//!
//! Layout of a build directory (`<out>/<build>/`): per scenario `<id>/verdict.json`,
//! `<id>/scenario.toml`, `<id>/receiver-log.json` and `<id>/signature.json` (the dossier
//! signature of [`crate::signed_dossier`]); `summary.json` and `summary.csv`; and a
//! `signature.json` over the whole build directory.

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kshana_eloran::sync::degrade::translate;
use kshana_eloran::sync::profile::{CommandProfile, GnssSimControl, ProfileAdapter, ScheduleMode};
use kshana_eloran::sync::{ArmMode, GnssSimAdapter, StartEpoch, TrajFormat, TrajectoryFile};
use serde::{Deserialize, Serialize};

use super::fidelity::{self, Capture};
use super::score::{self, ScenarioVerdict};
use super::spec::Spec;
use crate::license::KEY_LEN;
use crate::pics::log::{self, DeviceLog};

pub const SUMMARY_FORMAT: &str = "kshana-testkit-build/1";

/// Load scenarios from files, directories (every `*.toml` / `*.json` in them) and packs
/// (`pack:<name>`), resolved; sorted by id, duplicates refused.
pub fn load_set(items: &[String]) -> Result<Vec<Spec>, String> {
    let mut v = Vec::new();
    for it in items {
        if let Some(p) = it.strip_prefix("pack:") {
            for (_, src) in super::packs::pack(p)? {
                v.push(Spec::parse(src)?);
            }
            continue;
        }
        let p = Path::new(it);
        if p.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(p)
                .map_err(|e| format!("{it}: {e}"))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|f| f.extension().is_some_and(|x| x == "toml" || x == "json"))
                .collect();
            files.sort();
            for f in files {
                let text =
                    std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
                if text.contains(super::spec::FORMAT) {
                    v.push(Spec::parse(&text).map_err(|e| format!("{}: {e}", f.display()))?);
                }
            }
        } else if p.is_file() {
            v.push(Spec::load(p)?);
        } else if let Some((_, src)) = super::packs::find(it) {
            v.push(Spec::parse(src)?);
        } else {
            return Err(format!(
                "{it}: not a file, a directory, pack:<name> or a pack scenario"
            ));
        }
    }
    let mut out = v
        .into_iter()
        .map(|s| s.resolve())
        .collect::<Result<Vec<_>, _>>()?;
    out.sort_by(|a, b| a.id.cmp(&b.id));
    if let Some(w) = out.windows(2).find(|w| w[0].id == w[1].id) {
        return Err(format!(
            "scenario id '{}' appears twice in the set",
            w[0].id
        ));
    }
    if out.is_empty() {
        return Err("the scenario set is empty".into());
    }
    Ok(out)
}

/// Where the receiver output comes from.
pub enum Source {
    Logs { dir: PathBuf, t0_s: f64 },
}

/// A build run's settings.
pub struct RunConfig {
    pub build: String,
    pub receiver: String,
    pub capture: Option<Capture>,
}

/// The build summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub format: String,
    pub method: String,
    pub label: String,
    pub build: String,
    pub receiver: String,
    pub scenarios: Vec<SummaryRow>,
    pub pass: usize,
    pub fail: usize,
    pub incomplete: usize,
    pub not_run: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryRow {
    pub scenario: String,
    pub scenario_sha256: String,
    pub verdict: String,
    pub level_supported: Option<u8>,
    pub fidelity_grade: Option<String>,
}

/// The receiver log of one scenario from a log directory.
pub fn read_log(dir: &Path, spec: &Spec, t0_s: f64) -> Result<Option<DeviceLog>, String> {
    let j = dir.join(format!("{}.json", spec.id));
    if j.is_file() {
        let t = std::fs::read_to_string(&j).map_err(|e| format!("{}: {e}", j.display()))?;
        let mut l = DeviceLog::parse(&t).map_err(|e| format!("{}: {e}", j.display()))?;
        l.scenario = spec.id.clone();
        return Ok(Some(l));
    }
    let n = dir.join(format!("{}.nmea", spec.id));
    if n.is_file() {
        let t = std::fs::read_to_string(&n).map_err(|e| format!("{}: {e}", n.display()))?;
        return log::from_nmea(&t, &spec.id, t0_s)
            .map(Some)
            .map_err(|e| format!("{}: {e}", n.display()));
    }
    Ok(None)
}

/// Score one scenario's log (or record it not run), with the fidelity statement when the
/// output is a replay.
pub fn verdict_for(
    spec: &Spec,
    logd: Option<&DeviceLog>,
    cfg: &RunConfig,
) -> Result<ScenarioVerdict, String> {
    let mut v = match logd {
        Some(l) => score::score(spec, l, &cfg.build, &cfg.receiver)?,
        None => score::not_run(spec, &cfg.build, &cfg.receiver, "no receiver log"),
    };
    if let Some(c) = &cfg.capture {
        let f = fidelity::assess(c, spec);
        v.qualification = fidelity::qualification(&f);
        v.fidelity = Some(f);
    }
    Ok(v)
}

/// Write and sign one scenario's verdict directory.
pub fn write_verdict(
    dir: &Path,
    spec: &Spec,
    v: &ScenarioVerdict,
    logd: Option<&DeviceLog>,
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(
        dir.join("verdict.json"),
        serde_json::to_string_pretty(v).unwrap(),
    )
    .map_err(io)?;
    std::fs::write(dir.join("scenario.toml"), spec.to_toml()).map_err(io)?;
    if let Some(l) = logd {
        std::fs::write(dir.join("receiver-log.json"), l.to_json()).map_err(io)?;
    }
    crate::signed_dossier::sign(dir, seed, signer, utc).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write the build summary and sign the build directory as a whole.
pub fn finish_build(
    build_dir: &Path,
    cfg: &RunConfig,
    verdicts: &[ScenarioVerdict],
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
) -> Result<Summary, String> {
    let count = |s: &str| verdicts.iter().filter(|v| v.verdict == s).count();
    let s = Summary {
        format: SUMMARY_FORMAT.into(),
        method: score::METHOD_ID.into(),
        label: super::LABEL.into(),
        build: cfg.build.clone(),
        receiver: cfg.receiver.clone(),
        scenarios: verdicts
            .iter()
            .map(|v| SummaryRow {
                scenario: v.scenario.clone(),
                scenario_sha256: v.scenario_sha256.clone(),
                verdict: v.verdict.clone(),
                level_supported: v.level_supported,
                fidelity_grade: v.fidelity.as_ref().map(|f| f.grade.clone()),
            })
            .collect(),
        pass: count("pass"),
        fail: count("fail"),
        incomplete: count("incomplete"),
        not_run: count("not-run"),
    };
    let io = |e: std::io::Error| format!("{}: {e}", build_dir.display());
    std::fs::write(
        build_dir.join("summary.json"),
        serde_json::to_string_pretty(&s).unwrap(),
    )
    .map_err(io)?;
    let mut csv = String::from("scenario,verdict,level_supported,fidelity,scenario_sha256\n");
    for r in &s.scenarios {
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            r.scenario,
            r.verdict,
            r.level_supported.map(|l| l.to_string()).unwrap_or_default(),
            r.fidelity_grade.clone().unwrap_or_default(),
            r.scenario_sha256
        ));
    }
    std::fs::write(build_dir.join("summary.csv"), csv).map_err(io)?;
    crate::signed_dossier::sign(build_dir, seed, signer, utc).map_err(|e| e.to_string())?;
    Ok(s)
}

fn build_dir(out: &Path, build: &str) -> Result<PathBuf, String> {
    if build.is_empty()
        || !build
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        || build.starts_with('.')
    {
        return Err(format!(
            "build id '{build}' must be letters, digits, '-', '_' or '.'"
        ));
    }
    let d = out.join(build);
    if d.join("summary.json").exists() {
        return Err(format!(
            "{} already holds a build; verdicts are never overwritten",
            d.display()
        ));
    }
    Ok(d)
}

/// Run a set offline and write the signed build directory.
pub fn run_offline(
    set: &[Spec],
    src: &Source,
    cfg: &RunConfig,
    out: &Path,
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
) -> Result<Summary, String> {
    let bd = build_dir(out, &cfg.build)?;
    let Source::Logs { dir, t0_s } = src;
    let mut verdicts = Vec::new();
    for spec in set {
        let l = read_log(dir, spec, *t0_s)?;
        let v = verdict_for(spec, l.as_ref(), cfg)?;
        write_verdict(&bd.join(&spec.id), spec, &v, l.as_ref(), seed, signer, utc)?;
        verdicts.push(v);
    }
    finish_build(&bd, cfg, &verdicts, seed, signer, utc)
}

/// Live run settings.
pub struct Live {
    /// The simulator's command profile.
    pub profile: CommandProfile,
    /// `host:port` of the simulator's remote control.
    pub sim_addr: String,
    /// `host:port` serving the receiver's NMEA.
    pub receiver_addr: String,
    /// The trajectory file's path as the simulator sees it (when the profile loads files).
    pub sim_trajectory_path: Option<String>,
    /// Send the schedule in real time (true) or at once (time-tagged profiles, loopback tests).
    pub real_time: bool,
    /// Extra seconds of receiver output after the scene.
    pub tail_s: f64,
}

/// Drive one scenario on a lab simulator and collect the receiver's NMEA as a device log.
pub fn run_live(spec: &Spec, live: &Live) -> Result<DeviceLog, String> {
    let sched = translate(&super::compile::degradation(spec), &live.profile)?;
    let mut sim = ProfileAdapter::new(&live.sim_addr, live.profile.clone());
    sim.connect()?;
    if live
        .profile
        .supports(kshana_eloran::sync::profile::ops::SET_START_TIME)
    {
        sim.set_start_epoch(&StartEpoch::from_utc_iso(
            &spec.scene.start_utc,
            live.profile.epoch_scale,
        )?)?;
    }
    if !spec.trajectory.is_static() {
        if let Some(p) = &live.sim_trajectory_path {
            sim.load_trajectory(&TrajectoryFile {
                path: p.into(),
                format: TrajFormat::Nmea,
                rate_hz: 1.0,
                samples: (spec.scene.duration_s as usize) + 1,
            })?;
        } else {
            return Err("a moving scenario needs --sim-trajectory <path the simulator sees> (compile writes trajectory.nmea)".into());
        }
    }
    // Receiver reader for the whole run.
    let rx = TcpStream::connect(&live.receiver_addr)
        .map_err(|e| format!("receiver {}: {e}", live.receiver_addr))?;
    let total = spec.scene.duration_s + live.tail_s;
    let limit = if live.real_time {
        total
    } else {
        live.tail_s.max(0.5)
    };
    rx.set_read_timeout(Some(Duration::from_millis(200))).ok();
    let reader = std::thread::spawn(move || {
        let start = Instant::now();
        let mut r = BufReader::new(rx);
        let mut text = String::new();
        let mut first: Option<f64> = None;
        let mut line = String::new();
        while start.elapsed().as_secs_f64() < limit {
            line.clear();
            match r.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if first.is_none() && line.starts_with('$') {
                        first = Some(start.elapsed().as_secs_f64());
                    }
                    text.push_str(&line);
                }
                Err(_) => continue,
            }
        }
        (text, first.unwrap_or(0.0))
    });
    sim.arm(ArmMode::Immediate)?;
    let t_start = Instant::now();
    let mut wait = |t: f64| {
        if live.real_time && sched.mode == ScheduleMode::HostTimed {
            let el = t_start.elapsed().as_secs_f64();
            if t > el {
                std::thread::sleep(Duration::from_secs_f64(t - el));
            }
        }
    };
    sim.execute_schedule(&sched, &mut wait)?;
    let (text, t_first) = reader
        .join()
        .map_err(|_| "receiver reader failed".to_string())?;
    sim.stop().ok();
    let mut l = log::from_nmea(&text, &spec.id, if live.real_time { t_first } else { 0.0 })?;
    l.source = Some(format!("live:{}", live.profile.name));
    Ok(l)
}

/// A SYNTHETIC receiver log for a scenario, to try the runner and for the tests (MODELLED: a
/// test fixture, not a model of any receiver). `good`: flags each threat 2 s after onset
/// (and withdraws its fix under jamming), keeps the genuine solution until it flags, and is
/// correct 5 s after the end. Not good: follows a
/// spoofer without warning, coasts on a drifting fix (1 m/s, 5 ns/s) under jamming without
/// warning, and keeps its last error for 150 s after the end.
pub fn synth_log(spec: &Spec, good: bool) -> DeviceLog {
    use super::spec::ThreatType;
    let timing =
        spec.has_timing_criteria() || spec.trajectory.platform == super::spec::Platform::Timing;
    let n = spec.scene.duration_s.floor() as usize;
    let mut epochs = Vec::with_capacity(n + 1);
    let mut held: Option<(f64, f64, f64)> = None; // bad receiver: last error held after a threat
    let mut held_until = f64::NEG_INFINITY;
    for k in 0..=n {
        let t = k as f64;
        let tr = spec.trajectory.at(t);
        let noise = 0.5 * (t * 0.7).sin();
        let active: Vec<&super::spec::Threat> = spec
            .threats
            .iter()
            .filter(|x| t >= x.start_s && t < x.end_s())
            .collect();
        let jam = active
            .iter()
            .find(|x| matches!(x.kind, Some(ThreatType::Jamming) | Some(ThreatType::Outage)));
        let spf = active.iter().find(|x| x.kind == Some(ThreatType::Spoofing));
        let recent_end = spec
            .threats
            .iter()
            .filter(|x| t >= x.end_s())
            .map(|x| x.end_s())
            .fold(f64::NEG_INFINITY, f64::max);
        let (mut e, mut nn, mut te) = (noise, -noise, 5.0);
        let (mut valid, mut fj, mut fs) = (true, false, false);
        if let Some(j) = jam {
            let u = t - j.start_s;
            if good {
                if u >= 2.0 {
                    valid = false;
                    fj = true;
                }
            } else {
                e += u;
                te += 5.0 * u;
            }
        }
        if let Some(s) = spf {
            // the good receiver keeps the genuine solution until it flags, then reports
            // what it tracks with the flag up
            let flagged = good && t - s.start_s >= 2.0;
            if !good || flagged {
                let (ce, cn, ct) = spec.counterfeit_at(t);
                e += ce;
                nn += cn;
                te += ct;
            }
            fs = flagged;
        }
        if jam.is_none() && spf.is_none() {
            if good && t - recent_end < 5.0 {
                fs = true;
            }
            if !good {
                if let Some(h) = held {
                    if t < held_until {
                        e += h.0;
                        nn += h.1;
                        te += h.2;
                    }
                }
            }
        } else if !good {
            held = Some((e - noise, nn + noise, te - 5.0));
            held_until = active.iter().map(|x| x.end_s()).fold(0.0, f64::max) + 150.0;
        }
        let (la, lo) = super::spec::offset(tr.lat_deg, tr.lon_deg, e, nn);
        epochs.push(log::Epoch {
            t_s: t,
            fix_valid: Some(valid),
            lat_deg: valid.then_some(la),
            lon_deg: valid.then_some(lo),
            height_m: valid.then_some(tr.height_m),
            time_error_ns: timing.then_some(te),
            jamming: Some(fj),
            spoofing: Some(fs),
            integrity: None,
        });
    }
    DeviceLog {
        format: log::FORMAT.into(),
        device: log::Device {
            name: format!("synthetic-{}", if good { "good" } else { "bad" }),
            firmware: None,
            serial: None,
        },
        scenario: spec.id.clone(),
        truth: None,
        source: Some("synthetic".into()),
        epochs,
    }
}
