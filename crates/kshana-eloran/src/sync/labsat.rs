// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Racelogic LabSat record/replay units: scenario/trajectory export plus a remote-control
//! **template profile**.
//!
//! A LabSat replays RF files. Simulated scenarios are generated offline (Racelogic SatGen)
//! from a trajectory, then replayed. So the workflow is:
//!
//! 1. [`export_scenario`] writes the receiver trajectory (NMEA GGA+RMC; SatGen-style tools
//!    import NMEA trajectories) and a JSON manifest with the start epoch, duration,
//!    requested constellations/signals and the settings to enter in the generator;
//! 2. the operator generates the replay file with the vendor's tool from those inputs;
//! 3. [`LabsatAdapter`] loads/starts/stops replay through the template profile.
//!
//! Not verified here and to be confirmed with Racelogic documentation: the exact NMEA
//! sentence set, rate and height convention SatGen accepts; whether the unit can start
//! replay on an external 1PPS and how; the remote-control transport (Ethernet/serial),
//! port and the ASCII command set in [`DEFAULT_PROFILE`]; and the status strings. Runtime
//! degradations (power steps, masks, jamming) are generally not available on a replay
//! unit, so the template supports none of them — [`super::degrade::translate`] lists them
//! as unsupported; bake them into the generated scenario instead.

use super::export::{sample_trajectory, NmeaTrajectoryExport, TrajectoryExport};
use super::profile::{ops, CommandProfile, Constellation, Fields, GnssSimControl, ProfileAdapter};
use super::{
    ArmMode, GnssSimAdapter, GnssSimStatus, StartEpoch, TrajFormat, TrajectoryFile, UtcStamp,
};
use crate::scenario::Scenario;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Editable template profile (TOML). Not a verified LabSat command set.
pub const DEFAULT_PROFILE: &str = r#"
name = "labsat-template"
vendor = "Racelogic LabSat"
confirmed = false
notes = [
  "TEMPLATE: remote-control transport, port and ASCII commands are NOT verified.",
  "Confirm against the LabSat remote-control documentation for the unit and firmware.",
  "Replay unit: degradations must be baked into the generated scenario file.",
]
framing = "line"
terminator = "\r\n"
response = "every_command"
error_patterns = ["ERR", "FAIL"]

[commands]
connect = ["TYPE?"]
load_trajectory = ["PLAY:FILE ${path}"]
arm_pps = ["PLAY:START:PPS"]
arm_trigger = ["PLAY:START:TRIG"]
start_immediate = ["PLAY:START"]
stop = ["PLAY:STOP"]
status = ["STATUS?"]

[[status_map]]
contains = "ERR"
state = "error"
[[status_map]]
contains = "PLAYING"
state = "running"
[[status_map]]
contains = "WAIT"
state = "armed"
[[status_map]]
contains = "STOPPED"
state = "stopped"
[[status_map]]
contains = "IDLE"
state = "idle"
"#;

pub fn default_profile() -> CommandProfile {
    CommandProfile::from_toml(DEFAULT_PROFILE).expect("built-in LabSat profile parses")
}

/// Export options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabsatExportConfig {
    /// Base file name (without extension).
    pub name: String,
    /// Trajectory rate (Hz), default 10.
    pub rate_hz: f64,
    pub constellations: Vec<Constellation>,
    /// Signals, e.g. ["L1CA", "E1", "B1I"].
    pub signals: Vec<String>,
}

impl Default for LabsatExportConfig {
    fn default() -> Self {
        LabsatExportConfig {
            name: "scenario".into(),
            rate_hz: 10.0,
            constellations: vec![Constellation::Gps],
            signals: vec!["L1CA".into()],
        }
    }
}

/// The manifest written next to the trajectory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabsatManifest {
    pub scenario_hash: String,
    pub start_utc: String,
    pub start_gpst: String,
    pub gps_week: i64,
    pub gps_tow_s: f64,
    pub duration_s: f64,
    pub trajectory: TrajectoryFile,
    pub constellations: Vec<Constellation>,
    pub signals: Vec<String>,
    /// Settings the operator enters in the scenario generator.
    pub generator_settings: Vec<String>,
    pub notes: Vec<String>,
}

/// Write `<name>.nmea` and `<name>_manifest.json` into `dir`.
pub fn export_scenario(
    sc: &Scenario,
    dir: &Path,
    cfg: &LabsatExportConfig,
) -> Result<LabsatManifest, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let start = UtcStamp::parse(&sc.scene.start_utc)?;
    let ep = StartEpoch {
        utc: start,
        scale: super::EpochScale::Utc,
    };
    let (w, tow) = start.gps_week_tow();
    let exp = NmeaTrajectoryExport {
        rate_hz: cfg.rate_hz,
        ..Default::default()
    };
    let tpath = dir.join(format!("{}.nmea", cfg.name));
    let traj = exp.write(sc, &tpath)?;
    let samples = sample_trajectory(sc, cfg.rate_hz)?;
    let first = samples.first().ok_or("empty trajectory")?;
    let m = LabsatManifest {
        scenario_hash: sc.hash(),
        start_utc: start.iso(),
        start_gpst: ep.gpst_calendar().iso(),
        gps_week: w,
        gps_tow_s: tow,
        duration_s: sc.scene.params.duration_s,
        trajectory: traj,
        constellations: cfg.constellations.clone(),
        signals: cfg.signals.clone(),
        generator_settings: vec![
            format!("Start time (UTC): {}", start.iso()),
            format!("Duration: {} s", sc.scene.params.duration_s),
            format!(
                "Trajectory: {} (NMEA GGA+RMC, {} Hz, heights ellipsoidal with geoid separation 0)",
                tpath.display(),
                cfg.rate_hz
            ),
            format!(
                "Initial position: {:.7}°, {:.7}°, {:.2} m",
                first.lat_deg, first.lon_deg, first.h_m
            ),
            format!(
                "Constellations: {}",
                cfg.constellations
                    .iter()
                    .map(|c| c.key())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            format!("Signals: {}", cfg.signals.join(", ")),
        ],
        notes: vec![
            "Generate the replay file with the vendor's scenario generator; the accepted NMEA \
             sentence set, rate and height convention must be confirmed in its manual."
                .into(),
            "Replay must start on the common 1PPS edge labelled start_utc for the eLoran \
             and GNSS outputs to be synchronised."
                .into(),
        ],
    };
    let mpath = dir.join(format!("{}_manifest.json", cfg.name));
    std::fs::write(&mpath, serde_json::to_string_pretty(&m).expect("serialisable"))
        .map_err(|e| format!("{}: {e}", mpath.display()))?;
    Ok(m)
}

/// LabSat replay adapter. `set_start_epoch` only records the epoch (the replay file
/// defines GNSS time) unless the profile has a `set_start_time` command;
/// `load_trajectory` sends the generated replay file's path (`path`), not the NMEA file.
pub struct LabsatAdapter {
    pub inner: ProfileAdapter,
    pub epoch: Option<StartEpoch>,
}

impl LabsatAdapter {
    pub fn new(addr: &str) -> LabsatAdapter {
        LabsatAdapter::with_profile(addr, default_profile())
    }
    pub fn with_profile(addr: &str, profile: CommandProfile) -> LabsatAdapter {
        LabsatAdapter {
            inner: ProfileAdapter::new(addr, profile),
            epoch: None,
        }
    }
    /// Load a replay file by path.
    pub fn load_replay(&mut self, path: &Path) -> Result<(), String> {
        self.load_trajectory(&TrajectoryFile {
            path: PathBuf::from(path),
            format: TrajFormat::Nmea,
            rate_hz: 0.0,
            samples: 0,
        })
    }
}

impl GnssSimAdapter for LabsatAdapter {
    fn connect(&mut self) -> Result<String, String> {
        self.inner.connect()
    }
    fn set_start_epoch(&mut self, e: &StartEpoch) -> Result<(), String> {
        self.epoch = Some(e.clone());
        if self.inner.profile().supports(ops::SET_START_TIME) {
            self.inner.set_start_epoch(e)?;
        }
        Ok(())
    }
    fn load_trajectory(&mut self, t: &TrajectoryFile) -> Result<(), String> {
        self.inner.load_trajectory(t)
    }
    fn arm(&mut self, m: ArmMode) -> Result<(), String> {
        self.inner.arm(m)
    }
    fn status(&mut self) -> Result<GnssSimStatus, String> {
        self.inner.status()
    }
    fn stop(&mut self) -> Result<(), String> {
        self.inner.stop()
    }
}

impl GnssSimControl for LabsatAdapter {
    fn profile(&self) -> &CommandProfile {
        self.inner.profile()
    }
    fn send_op(&mut self, op: &str, f: &Fields) -> Result<Vec<String>, String> {
        self.inner.send_op(op, f)
    }
    fn send_lines(&mut self, l: &[String]) -> Result<Vec<String>, String> {
        self.inner.send_lines(l)
    }
}

#[cfg(test)]
mod tests {
    use super::super::profile::{mock, Framing};
    use super::super::{arm_gnss, EpochScale, GnssSimState, SyncTimingModel};
    use super::*;

    const SC: &str = r#"
[scene]
start_utc = "2026-11-18T12:00:00Z"
duration_s = 3.0
fs_hz = 1000000
[receiver]
type = "static"
lat_deg = 51.5
lon_deg = -0.1
h_m = 45.0
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.0, lon_deg = 0.0 }
"#;

    #[test]
    fn exports_trajectory_and_manifest() {
        let sc = Scenario::from_toml(SC).unwrap();
        let dir = std::env::temp_dir().join(format!("kshana-labsat-{}", std::process::id()));
        let m = export_scenario(&sc, &dir, &LabsatExportConfig::default()).unwrap();
        assert_eq!(m.trajectory.samples, 31);
        assert_eq!(m.start_gpst, "2026-11-18T12:00:18.000000000Z");
        let nmea = std::fs::read_to_string(dir.join("scenario.nmea")).unwrap();
        assert_eq!(nmea.lines().count(), 62);
        let j: LabsatManifest = serde_json::from_str(
            &std::fs::read_to_string(dir.join("scenario_manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(j, m);
        assert!(j.generator_settings.iter().any(|s| s.contains("51.5000000")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn replay_control_against_mock() {
        let mut st = "IDLE";
        let (addr, log, h) = mock::serve(Framing::Line, move |c| {
            Some(
                match c {
                    "TYPE?" => "LabSat 3 Wideband (mock)",
                    "PLAY:START:PPS" => {
                        st = "WAIT PPS";
                        "OK"
                    }
                    "PLAY:STOP" => {
                        st = "STOPPED";
                        "OK"
                    }
                    "STATUS?" => st,
                    _ => "OK",
                }
                .to_string(),
            )
        });
        let mut a = LabsatAdapter::new(&addr);
        let plan = SyncTimingModel {
            common_10mhz: true,
            common_pps: true,
            eloran_minus_gnss_ns: 0.0,
            eloran_minus_gnss_u_ns: 0.0,
            gnss_arm: ArmMode::NextPps,
            epoch_scale: EpochScale::Utc,
        }
        .plan("2026-11-18T12:00:00Z")
        .unwrap();
        let tf = TrajectoryFile {
            path: "/media/scen.ls3w".into(),
            format: TrajFormat::Nmea,
            rate_hz: 0.0,
            samples: 0,
        };
        let hs = arm_gnss(&mut a, &plan, Some(&tf)).unwrap();
        assert_eq!(hs.status.state, GnssSimState::Armed);
        assert!(a.epoch.is_some());
        a.stop().unwrap();
        drop(a);
        h.join().unwrap();
        assert_eq!(
            log.lock().unwrap().clone(),
            vec!["TYPE?", "PLAY:FILE /media/scen.ls3w", "PLAY:START:PPS", "STATUS?", "PLAY:STOP"]
        );
    }
}
