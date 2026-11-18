// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The open-source `gps-sdr-sim` GPS L1 C/A baseband generator (github.com/osqzss/gps-sdr-sim).
//!
//! gps-sdr-sim is an offline generator: it reads a RINEX GPS navigation file and a
//! receiver position or motion and writes an I/Q file that an SDR then plays. This module
//! generates its motion inputs and its exact command line, and can spawn it with
//! [`std::process::Command`]:
//!
//! * user motion in ECEF (`-u file`, lines `t,x,y,z`), in LLH (`-x file`, lines
//!   `t,lat_deg,lon_deg,h_m`) or as NMEA GGA (`-g file`); gps-sdr-sim steps its motion at
//!   **10 Hz**, so the writers here always produce 10 Hz files (time column informative);
//! * static position (`-l lat,lon,hgt` or `-c x,y,z`);
//! * start time `-t YYYY/MM/DD,hh:mm:ss` (or `-T` to also overwrite TOC/TOE), duration
//!   `-d`, output `-o`, sample rate `-s`, I/Q bits `-b`, `-i` (no ionosphere), `-p [gain]`
//!   (no path loss / fixed gain), `-v`.
//!
//! Options follow the project's README/usage text; check them against the `gps-sdr-sim`
//! build in use (`gps-sdr-sim` without arguments prints its usage): the LLH motion option
//! `-x` and the maximum motion length (`USER_MOTION_SIZE`) differ between versions, and
//! whether `-t` is read as GPS time or UTC should be confirmed (the default here passes the
//! GPST calendar time; set `epoch_scale` to change it).
//!
//! Synchronisation: the generated file has no PPS alignment of its own. Play it with an
//! SDR player that starts on the common 1PPS edge labelled `start_utc`; `arm(NextPps)` is
//! therefore refused here.

use super::degrade::{Action, GnssDegradation};
use super::export::{nmea_checksum, sample_trajectory, TrajSample};
use super::motion::llh_to_ecef;
use super::{
    ArmMode, EpochScale, GnssSimAdapter, GnssSimState, GnssSimStatus, StartEpoch, TrajFormat,
    TrajectoryFile, UtcStamp,
};
use crate::scenario::Scenario;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// gps-sdr-sim's motion update rate (Hz).
pub const MOTION_RATE_HZ: f64 = 10.0;

/// Receiver position / motion input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MotionInput {
    None,
    /// `-l lat,lon,hgt`.
    StaticLlh { lat_deg: f64, lon_deg: f64, h_m: f64 },
    /// `-c x,y,z`.
    StaticEcef { x: f64, y: f64, z: f64 },
    /// `-u file` (ECEF `t,x,y,z`).
    UserEcef { path: PathBuf },
    /// `-x file` (`t,lat,lon,h`).
    UserLlh { path: PathBuf },
    /// `-g file` (NMEA GGA).
    NmeaGga { path: PathBuf },
}

/// Which gps-sdr-sim motion option a CSV trajectory file is given with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CsvMotionKind {
    #[default]
    Ecef,
    Llh,
}

/// gps-sdr-sim invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GpsSdrSimConfig {
    /// Executable (path or name on PATH).
    pub binary: PathBuf,
    /// RINEX GPS navigation file (`-e`, required).
    pub nav_file: PathBuf,
    /// Output I/Q file (`-o`).
    pub output: PathBuf,
    /// Sample rate (`-s`, Hz).
    pub sample_rate_hz: f64,
    /// I/Q bits (`-b`: 1, 8 or 16).
    pub iq_bits: u8,
    pub motion: MotionInput,
    /// Start (set by `set_start_epoch`).
    pub start: Option<UtcStamp>,
    /// Scale of the `-t` argument (default GPST calendar).
    pub epoch_scale: EpochScale,
    /// Use `-T` (overwrite TOC/TOE with the start time) instead of `-t`.
    pub overwrite_toc_toe: bool,
    /// Duration (`-d`, s).
    pub duration_s: Option<f64>,
    /// `-i`.
    pub disable_iono: bool,
    /// `-p [gain]`: `Some(None)` disables path loss, `Some(Some(g))` fixes the gain.
    pub fixed_gain: Option<Option<i32>>,
    pub verbose: bool,
    /// Kind of CSV trajectory files passed to `load_trajectory`.
    pub csv_motion: CsvMotionKind,
    pub extra_args: Vec<String>,
}

impl Default for GpsSdrSimConfig {
    fn default() -> Self {
        GpsSdrSimConfig {
            binary: "gps-sdr-sim".into(),
            nav_file: "brdc.nav".into(),
            output: "gpssim.bin".into(),
            sample_rate_hz: 2.6e6,
            iq_bits: 16,
            motion: MotionInput::None,
            start: None,
            epoch_scale: EpochScale::Gpst,
            overwrite_toc_toe: false,
            duration_s: None,
            disable_iono: false,
            fixed_gain: None,
            verbose: false,
            csv_motion: CsvMotionKind::Ecef,
            extra_args: vec![],
        }
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-" {
        "0".into()
    } else {
        s.to_string()
    }
}

impl GpsSdrSimConfig {
    /// The argument vector (without the program).
    pub fn args(&self) -> Result<Vec<String>, String> {
        if ![1, 8, 16].contains(&self.iq_bits) {
            return Err("iq_bits must be 1, 8 or 16".into());
        }
        if !(self.sample_rate_hz > 0.0) {
            return Err("sample_rate_hz must be > 0".into());
        }
        let p = |p: &Path| p.display().to_string();
        let mut a = vec!["-e".to_string(), p(&self.nav_file)];
        match &self.motion {
            MotionInput::None => {}
            MotionInput::StaticLlh {
                lat_deg,
                lon_deg,
                h_m,
            } => a.extend(["-l".into(), format!("{},{},{}", num(*lat_deg), num(*lon_deg), num(*h_m))]),
            MotionInput::StaticEcef { x, y, z } => {
                a.extend(["-c".into(), format!("{},{},{}", num(*x), num(*y), num(*z))])
            }
            MotionInput::UserEcef { path } => a.extend(["-u".into(), p(path)]),
            MotionInput::UserLlh { path } => a.extend(["-x".into(), p(path)]),
            MotionInput::NmeaGga { path } => a.extend(["-g".into(), p(path)]),
        }
        if let Some(s) = self.start {
            let e = StartEpoch {
                utc: s,
                scale: self.epoch_scale,
            }
            .in_scale();
            let (y, mo, d) = e.ymd();
            let (h, mi, se, _) = e.hms();
            a.push(if self.overwrite_toc_toe { "-T" } else { "-t" }.into());
            a.push(format!("{y:04}/{mo:02}/{d:02},{h:02}:{mi:02}:{se:02}"));
        }
        if let Some(d) = self.duration_s {
            if !(d > 0.0) {
                return Err("duration_s must be > 0".into());
            }
            a.extend(["-d".into(), num(d)]);
        }
        a.extend(["-o".into(), p(&self.output)]);
        a.extend(["-s".into(), num(self.sample_rate_hz)]);
        a.extend(["-b".into(), self.iq_bits.to_string()]);
        if self.disable_iono {
            a.push("-i".into());
        }
        match self.fixed_gain {
            None => {}
            Some(None) => a.push("-p".into()),
            Some(Some(g)) => a.extend(["-p".into(), g.to_string()]),
        }
        if self.verbose {
            a.push("-v".into());
        }
        a.extend(self.extra_args.iter().cloned());
        Ok(a)
    }
    /// The full command line, shell-quoted for display/logging.
    pub fn command_line(&self) -> Result<String, String> {
        let q = |s: &str| {
            if !s.is_empty()
                && s
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_./,:=+".contains(c))
            {
                s.to_string()
            } else {
                format!("'{}'", s.replace('\'', "'\\''"))
            }
        };
        let mut v = vec![q(&self.binary.display().to_string())];
        v.extend(self.args()?.iter().map(|s| q(s)));
        Ok(v.join(" "))
    }
    /// A [`Command`] ready to spawn.
    pub fn command(&self) -> Result<Command, String> {
        let mut c = Command::new(&self.binary);
        c.args(self.args()?);
        Ok(c)
    }
}

/// Resample trajectory samples to gps-sdr-sim's 10 Hz grid (the scenario sampler already
/// interpolates, so this just samples the scenario at 10 Hz).
pub fn motion_samples(sc: &Scenario) -> Result<Vec<TrajSample>, String> {
    sample_trajectory(sc, MOTION_RATE_HZ)
}

/// ECEF user-motion CSV (`t,x,y,z`, m).
pub fn render_user_motion_ecef(samples: &[TrajSample]) -> String {
    let mut s = String::new();
    for p in samples {
        let e = llh_to_ecef(p.lat_deg, p.lon_deg, p.h_m);
        s.push_str(&format!("{:.1},{:.3},{:.3},{:.3}\n", p.t_s, e[0], e[1], e[2]));
    }
    s
}

/// LLH user-motion CSV (`t,lat_deg,lon_deg,h_m`).
pub fn render_user_motion_llh(samples: &[TrajSample]) -> String {
    let mut s = String::new();
    for p in samples {
        s.push_str(&format!(
            "{:.1},{:.9},{:.9},{:.3}\n",
            p.t_s, p.lat_deg, p.lon_deg, p.h_m
        ));
    }
    s
}

/// NMEA GGA-only stream (one sentence per sample).
pub fn render_nmea_gga(samples: &[TrajSample]) -> String {
    let ang = |v: f64, w: usize, pos: char, neg: char| {
        let m5 = (v.abs() * 60.0 * 1e5).round() as u64;
        let (d, rem) = (m5 / 6_000_000, m5 % 6_000_000);
        format!(
            "{d:0w$}{:02}.{:05},{}",
            rem / 100_000,
            rem % 100_000,
            if v < 0.0 { neg } else { pos }
        )
    };
    let mut s = String::new();
    for p in samples {
        let (h, m, sec, ns) = p.utc.hms();
        let body = format!(
            "GPGGA,{h:02}{m:02}{sec:02}.{:02},{},{},1,12,0.8,{:.2},M,0.00,M,,",
            ns / 10_000_000,
            ang(p.lat_deg, 2, 'N', 'S'),
            ang(p.lon_deg, 3, 'E', 'W'),
            p.h_m
        );
        s.push_str(&format!("${body}*{:02X}\r\n", nmea_checksum(&body)));
    }
    s
}

/// Write a 10 Hz motion file for `sc` in the given form and return the matching input.
pub fn write_motion(sc: &Scenario, path: &Path, kind: MotionFileKind) -> Result<MotionInput, String> {
    let s = motion_samples(sc)?;
    let (txt, m) = match kind {
        MotionFileKind::Ecef => (
            render_user_motion_ecef(&s),
            MotionInput::UserEcef { path: path.into() },
        ),
        MotionFileKind::Llh => (
            render_user_motion_llh(&s),
            MotionInput::UserLlh { path: path.into() },
        ),
        MotionFileKind::NmeaGga => (
            render_nmea_gga(&s),
            MotionInput::NmeaGga { path: path.into() },
        ),
    };
    std::fs::write(path, txt).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(m)
}

/// Motion file forms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionFileKind {
    Ecef,
    Llh,
    NmeaGga,
}

/// What of a degradation timeline gps-sdr-sim cannot do at run time (it has none of the
/// runtime controls); each entry says how to realise it otherwise.
pub fn degradation_support(deg: &GnssDegradation) -> Vec<String> {
    let mut v = Vec::new();
    for a in deg.actions() {
        let t = a.t_s;
        let s = match a.action {
            Action::GroupOffset { group, .. } => format!(
                "t = {t} s: power offset {} not available at run time (only a constant gain via -p); \
                 scale the I/Q file segment instead",
                group.label()
            ),
            Action::SatDisable { constellation, prn } | Action::SatEnable { constellation, prn } => {
                format!(
                    "t = {t} s: {} PRN {prn} mask not available at run time; remove the satellite \
                     from the navigation file and generate segments",
                    constellation.key()
                )
            }
            Action::SignalOff { group } | Action::SignalOn { group } => format!(
                "t = {t} s: outage {} not available; blank the I/Q file segment",
                group.label()
            ),
            Action::JammerStart { id, .. } | Action::JammerStop { id } => format!(
                "t = {t} s: jammer {id} not generated by gps-sdr-sim; add interference in the I/Q domain"
            ),
            Action::SpoofStart { label } | Action::SpoofStop { label } => format!(
                "t = {t} s: spoofing marker {label}: generate the spoofer as a second gps-sdr-sim run"
            ),
        };
        v.push(s);
    }
    v
}

/// gps-sdr-sim as a [`GnssSimAdapter`] (a local process, not a remote instrument).
pub struct GpsSdrSim {
    pub config: GpsSdrSimConfig,
    child: Option<Child>,
    /// Exit code of the last finished run.
    pub last_exit: Option<i32>,
}

impl GpsSdrSim {
    pub fn new(config: GpsSdrSimConfig) -> GpsSdrSim {
        GpsSdrSim {
            config,
            child: None,
            last_exit: None,
        }
    }
    /// Write a 10 Hz ECEF motion file for `sc` into `dir` and set start and duration from
    /// the scenario. Returns the command line.
    pub fn prepare(&mut self, sc: &Scenario, dir: &Path) -> Result<String, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        self.config.motion = write_motion(sc, &dir.join("user_motion_ecef.csv"), MotionFileKind::Ecef)?;
        self.config.start = Some(UtcStamp::parse(&sc.scene.start_utc)?);
        self.config.duration_s = Some(sc.scene.params.duration_s);
        self.config.command_line()
    }
    /// Run to completion (blocking); returns the exit code.
    pub fn run_blocking(&mut self) -> Result<i32, String> {
        let st = self
            .config
            .command()?
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("{}: {e}", self.config.binary.display()))?;
        let c = st.code().unwrap_or(-1);
        self.last_exit = Some(c);
        Ok(c)
    }
}

fn on_path(bin: &Path) -> bool {
    if bin.components().count() > 1 {
        return bin.is_file();
    }
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
        .unwrap_or(false)
}

impl GnssSimAdapter for GpsSdrSim {
    fn connect(&mut self) -> Result<String, String> {
        if !on_path(&self.config.binary) {
            return Err(format!(
                "gps-sdr-sim executable {} not found",
                self.config.binary.display()
            ));
        }
        if !self.config.nav_file.is_file() {
            return Err(format!(
                "RINEX navigation file {} not found",
                self.config.nav_file.display()
            ));
        }
        Ok(format!(
            "gps-sdr-sim (offline I/Q generator) {}",
            self.config.binary.display()
        ))
    }
    fn set_start_epoch(&mut self, e: &StartEpoch) -> Result<(), String> {
        self.config.start = Some(e.utc);
        Ok(())
    }
    fn load_trajectory(&mut self, t: &TrajectoryFile) -> Result<(), String> {
        if (t.rate_hz - MOTION_RATE_HZ).abs() > 1e-9 {
            return Err(format!(
                "gps-sdr-sim reads motion at {MOTION_RATE_HZ} Hz; trajectory is {} Hz",
                t.rate_hz
            ));
        }
        self.config.motion = match t.format {
            TrajFormat::Nmea => MotionInput::NmeaGga {
                path: t.path.clone(),
            },
            // the CSV must be in gps-sdr-sim's form (see write_motion), not export.rs's CSV
            TrajFormat::Csv => match self.config.csv_motion {
                CsvMotionKind::Ecef => MotionInput::UserEcef {
                    path: t.path.clone(),
                },
                CsvMotionKind::Llh => MotionInput::UserLlh {
                    path: t.path.clone(),
                },
            },
        };
        Ok(())
    }
    fn arm(&mut self, mode: ArmMode) -> Result<(), String> {
        if mode != ArmMode::Immediate {
            return Err(
                "gps-sdr-sim generates an I/Q file offline and cannot arm on a PPS; generate \
                 now (ArmMode::Immediate) and start the SDR playback on the PPS instead"
                    .into(),
            );
        }
        if self.child.is_some() {
            return Err("gps-sdr-sim already running".into());
        }
        let c = self
            .config
            .command()?
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{}: {e}", self.config.binary.display()))?;
        self.child = Some(c);
        self.last_exit = None;
        Ok(())
    }
    fn status(&mut self) -> Result<GnssSimStatus, String> {
        if let Some(c) = self.child.as_mut() {
            match c.try_wait().map_err(|e| e.to_string())? {
                None => {
                    return Ok(GnssSimStatus {
                        state: GnssSimState::Running,
                        raw: "running".into(),
                    })
                }
                Some(st) => {
                    self.last_exit = Some(st.code().unwrap_or(-1));
                    self.child = None;
                }
            }
        }
        Ok(match self.last_exit {
            None => GnssSimStatus {
                state: GnssSimState::Idle,
                raw: "idle".into(),
            },
            Some(0) => GnssSimStatus {
                state: GnssSimState::Stopped,
                raw: "exit 0".into(),
            },
            Some(c) => GnssSimStatus {
                state: GnssSimState::Error,
                raw: format!("exit {c}"),
            },
        })
    }
    fn stop(&mut self) -> Result<(), String> {
        if let Some(mut c) = self.child.take() {
            c.kill().ok();
            let st = c.wait().map_err(|e| e.to_string())?;
            self.last_exit = Some(st.code().unwrap_or(-1));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SC: &str = r#"
[scene]
start_utc = "2026-11-18T12:00:00Z"
duration_s = 1.0
fs_hz = 1000000
[receiver]
type = "waypoints"
points = [
  { t_s = 0.0, lat_deg = 35.0, lon_deg = 139.0, h_m = 50.0 },
  { t_s = 100.0, lat_deg = 35.01, lon_deg = 139.0, h_m = 50.0 },
]
[[outputs]]
type = "e_field"
[[chains]]
name = "c"
gri_us = 99300
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 36.0, lon_deg = 129.3 }
"#;

    #[test]
    fn exact_command_line() {
        let mut c = GpsSdrSimConfig {
            nav_file: "brdc3220.26n".into(),
            output: "out dir/sim.bin".into(),
            motion: MotionInput::UserEcef {
                path: "motion.csv".into(),
            },
            start: Some(UtcStamp::parse("2026-11-18T12:00:00Z").unwrap()),
            duration_s: Some(120.0),
            sample_rate_hz: 2.5e6,
            iq_bits: 8,
            fixed_gain: Some(Some(-20)),
            ..Default::default()
        };
        assert_eq!(
            c.command_line().unwrap(),
            "gps-sdr-sim -e brdc3220.26n -u motion.csv -t 2026/11/18,12:00:18 -d 120 \
             -o 'out dir/sim.bin' -s 2500000 -b 8 -p -20"
        );
        c.epoch_scale = EpochScale::Utc;
        c.overwrite_toc_toe = true;
        c.motion = MotionInput::StaticLlh {
            lat_deg: 35.681298,
            lon_deg: 139.766247,
            h_m: 10.0,
        };
        c.fixed_gain = Some(None);
        c.disable_iono = true;
        let a = c.args().unwrap();
        assert_eq!(a[2..6], ["-l", "35.681298,139.766247,10", "-T", "2026/11/18,12:00:00"]);
        assert!(a.ends_with(&["-i".to_string(), "-p".to_string()]));
        c.iq_bits = 4;
        assert!(c.args().is_err());
    }

    #[test]
    fn motion_files_are_10hz_and_consistent() {
        let sc = Scenario::from_toml(SC).unwrap();
        let s = motion_samples(&sc).unwrap();
        assert_eq!(s.len(), 11);
        let ecef = render_user_motion_ecef(&s);
        let rows: Vec<&str> = ecef.lines().collect();
        assert_eq!(rows.len(), 11);
        assert!(rows[1].starts_with("0.1,"));
        let f: Vec<f64> = rows[0].split(',').map(|x| x.parse().unwrap()).collect();
        let (la, lo, h) = super::super::motion::ecef_to_llh([f[1], f[2], f[3]]);
        assert!((la - 35.0).abs() < 1e-7 && (lo - 139.0).abs() < 1e-7 && (h - 50.0).abs() < 1e-2);
        let llh = render_user_motion_llh(&s);
        assert!(llh.starts_with("0.0,35.000000000,139.000000000,50.000\n"));
        let gga = render_nmea_gga(&s);
        let l0 = gga.lines().next().unwrap();
        assert!(l0.starts_with("$GPGGA,120000.00,3500.00000,N,13900.00000,E,1,12"), "{l0}");
        let (body, cs) = l0.trim_start_matches('$').split_once('*').unwrap();
        assert_eq!(u8::from_str_radix(cs, 16).unwrap(), nmea_checksum(body));
    }

    #[test]
    fn adapter_prepares_and_refuses_pps_arm() {
        let sc = Scenario::from_toml(SC).unwrap();
        let dir = std::env::temp_dir().join(format!("kshana-gpssdrsim-{}", std::process::id()));
        let mut g = GpsSdrSim::new(GpsSdrSimConfig::default());
        let cl = g.prepare(&sc, &dir).unwrap();
        assert!(cl.contains("-u ") && cl.contains("user_motion_ecef.csv"), "{cl}");
        assert!(cl.contains("-t 2026/11/18,12:00:18 -d 1 "), "{cl}");
        assert!(g.arm(ArmMode::NextPps).unwrap_err().contains("PPS"));
        assert!(g
            .load_trajectory(&TrajectoryFile {
                path: "x.nmea".into(),
                format: TrajFormat::Nmea,
                rate_hz: 1.0,
                samples: 3
            })
            .is_err());
        g.load_trajectory(&TrajectoryFile {
            path: "x.nmea".into(),
            format: TrajFormat::Nmea,
            rate_hz: 10.0,
            samples: 3,
        })
        .unwrap();
        assert_eq!(
            g.config.motion,
            MotionInput::NmeaGga {
                path: "x.nmea".into()
            }
        );
        // connect fails without a navigation file
        g.config.binary = "/bin/true".into();
        assert!(g.connect().unwrap_err().contains("navigation"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn spawns_process_and_tracks_exit() {
        if !Path::new("/bin/true").exists() || !Path::new("/bin/false").exists() {
            return;
        }
        let nav = std::env::temp_dir().join(format!("kshana-nav-{}.n", std::process::id()));
        std::fs::write(&nav, "synthetic").unwrap();
        let mut g = GpsSdrSim::new(GpsSdrSimConfig {
            binary: "/bin/true".into(),
            nav_file: nav.clone(),
            ..Default::default()
        });
        assert!(g.connect().unwrap().contains("gps-sdr-sim"));
        assert_eq!(g.status().unwrap().state, GnssSimState::Idle);
        g.arm(ArmMode::Immediate).unwrap();
        let mut st = g.status().unwrap().state;
        for _ in 0..200 {
            if st != GnssSimState::Running {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            st = g.status().unwrap().state;
        }
        assert_eq!(st, GnssSimState::Stopped);
        g.config.binary = "/bin/false".into();
        assert_eq!(g.run_blocking().unwrap(), 1);
        assert_eq!(g.status().unwrap().state, GnssSimState::Error);
        std::fs::remove_file(&nav).ok();
    }
}
