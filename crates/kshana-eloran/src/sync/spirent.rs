// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Spirent GNSS simulators driven by SimREMOTE-style text commands over TCP —
//! **template profile**.
//!
//! Spirent's SimREMOTE interface accepts text commands, many of which can carry a scenario
//! time tag so that they execute at a given scenario time (here
//! [`super::profile::ScheduleMode::TimeTagged`] with `${t_tag}`, immediate commands using
//! `immediate_tag`). The command mnemonics, field order, units, time-tag syntax, the TCP
//! port and the response format in [`DEFAULT_PROFILE`] are **placeholders in that style
//! and are not verified**: they must be confirmed against the SimREMOTE / PosApp remote
//! command manual of the installed SimGEN/PosApp version (and the profile's
//! `confirmed` flag set only then). In particular confirm: the time-tag format, whether a
//! time-tagged command must be sent before the run starts, power-level command semantics
//! (absolute vs relative, per satellite vs per signal), the motion command and units of
//! the remote-motion (HIL) stream, the interference commands (often a separate
//! interference generator), and the response/acknowledgement format.

use super::export::sample_trajectory;
use super::motion::motion_from_traj;
use super::profile::{delegate_profile_adapter, CommandProfile, GnssSimControl, ProfileAdapter};
use crate::scenario::Scenario;

/// Editable template profile (TOML). Not a verified Spirent command set.
pub const DEFAULT_PROFILE: &str = r#"
name = "spirent-simremote-template"
vendor = "Spirent (SimREMOTE style)"
confirmed = false
notes = [
  "TEMPLATE: SimREMOTE-style mnemonics, field order, units and time-tag syntax are NOT verified.",
  "Confirm against the SimREMOTE / PosApp remote command manual for the installed version.",
]
framing = "line"
terminator = "\r\n"
default_port = 15650
response = "every_command"
error_patterns = ["<status>1", "ERROR", "Error"]
schedule = "time_tagged"
time_tag_format = "hms"
immediate_tag = "-"
epoch_scale = "gpst"
stream_rate_hz = 10.0

[constellations]
gps = "GPS"
galileo = "GALILEO"
glonass = "GLONASS"
beidou = "BEIDOU"
qzss = "QZSS"
navic = "NAVIC"
sbas = "SBAS"

[commands]
connect = ["${t_tag},NULL"]
set_start_time = ["${t_tag},START_TIME,${date},${time}"]
load_trajectory = ["${t_tag},LOAD_MOTION,${path},${format}"]
stream_sample = ["${t_tag},MOT,v1_m1,${t_s},${x},${y},${z},${vx},${vy},${vz},${yaw_rad},${pitch_rad},${roll_rad}"]
set_power_offset = ["${t_tag},POW_LEV,v1_a1,${offset_db},${constellation},${signal}"]
sat_disable = ["${t_tag},POW_ON,v1_a1,0,${constellation},${prn}"]
sat_enable = ["${t_tag},POW_ON,v1_a1,1,${constellation},${prn}"]
signal_off = ["${t_tag},SIG_ON,v1_a1,0,${constellation},${signal}"]
signal_on = ["${t_tag},SIG_ON,v1_a1,1,${constellation},${signal}"]
jammer_start = ["${t_tag},INTF,${jammer_id},ON,${kind},${center_hz},${bandwidth_hz},${power_dbm}"]
jammer_stop = ["${t_tag},INTF,${jammer_id},OFF"]
spoof_start = ["${t_tag},MARKER,SPOOF_START,${label}"]
spoof_stop = ["${t_tag},MARKER,SPOOF_STOP,${label}"]
arm_pps = ["${t_tag},AR"]
arm_trigger = ["${t_tag},AR"]
start_immediate = ["${t_tag},RU"]
stop = ["${t_tag},EN"]
status = ["${t_tag},NULL"]

[[status_map]]
contains = "<state>error"
state = "error"
[[status_map]]
contains = "<state>running"
state = "running"
[[status_map]]
contains = "<state>armed"
state = "armed"
[[status_map]]
contains = "<state>ended"
state = "stopped"
[[status_map]]
contains = "<state>ready"
state = "idle"
"#;

pub fn default_profile() -> CommandProfile {
    CommandProfile::from_toml(DEFAULT_PROFILE).expect("built-in Spirent profile parses")
}

/// Spirent adapter over a [`ProfileAdapter`].
pub struct SpirentAdapter(pub ProfileAdapter);

delegate_profile_adapter!(SpirentAdapter);

impl SpirentAdapter {
    pub fn new(addr: &str) -> SpirentAdapter {
        SpirentAdapter(ProfileAdapter::new(addr, default_profile()))
    }
    pub fn with_profile(addr: &str, profile: CommandProfile) -> SpirentAdapter {
        SpirentAdapter(ProfileAdapter::new(addr, profile))
    }
    /// Stream the scenario trajectory as remote-motion commands at `rate_hz`.
    pub fn stream_trajectory(
        &mut self,
        sc: &Scenario,
        rate_hz: Option<f64>,
        pace: &mut dyn FnMut(f64),
    ) -> Result<usize, String> {
        let r = rate_hz.unwrap_or(self.profile().stream_rate_hz);
        let m = motion_from_traj(&sample_trajectory(sc, r)?);
        self.stream_motion(&m, r, pace)
    }
}

#[cfg(test)]
mod tests {
    use super::super::degrade::{translate, GnssDegradation};
    use super::super::profile::{mock, Framing};
    use super::super::{
        ArmMode, EpochScale, GnssSimAdapter, GnssSimState, StartEpoch, TrajFormat, TrajectoryFile,
    };
    use super::*;

    #[test]
    fn controls_mock_and_uploads_time_tagged_schedule() {
        let mut state = "ready";
        let (addr, log, h) = mock::serve(Framing::Line, move |c| {
            let f: Vec<&str> = c.split(',').collect();
            if f.get(1) == Some(&"AR") {
                state = "armed";
            }
            if f.get(1) == Some(&"EN") {
                state = "ended";
            }
            if f.get(1) == Some(&"BOGUS") {
                return Some("<msg><status>1</status><error>unknown</error></msg>".into());
            }
            Some(format!("<msg><status>0</status><state>{state}</state></msg>"))
        });
        let mut a = SpirentAdapter::new(&addr);
        assert!(a.connect().unwrap().contains("<state>ready"));
        a.set_start_epoch(&StartEpoch::from_utc_iso("2026-11-18T00:00:00Z", EpochScale::Utc).unwrap())
            .unwrap();
        a.load_trajectory(&TrajectoryFile {
            path: "C:\\motion\\rx.nmea".into(),
            format: TrajFormat::Nmea,
            rate_hz: 10.0,
            samples: 100,
        })
        .unwrap();
        let deg = GnssDegradation::from_toml_table(
            &toml::from_str(
                r#"
[[events]]
type = "power_step"
at_s = 65.5
constellation = "gps"
step_db = -10.0
[[events]]
type = "sat_mask"
at_s = 70.0
constellation = "galileo"
prns = [3]
"#,
            )
            .unwrap(),
        )
        .unwrap();
        let s = translate(&deg, a.profile()).unwrap();
        let mut waited = vec![];
        a.execute_schedule(&s, &mut |t| waited.push(t)).unwrap();
        assert!(waited.is_empty(), "time-tagged schedules are sent at once");
        a.arm(ArmMode::NextPps).unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Armed);
        let e = a.send_lines(&["-,BOGUS".to_string()]).unwrap_err();
        assert!(e.contains("<status>1"), "{e}");
        a.stop().unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Stopped);
        drop(a);
        h.join().unwrap();
        let l = log.lock().unwrap().clone();
        assert_eq!(l[0], "-,NULL");
        assert_eq!(l[1], "-,START_TIME,2026-11-18,00:00:18");
        assert_eq!(l[2], "-,LOAD_MOTION,C:\\motion\\rx.nmea,nmea");
        assert_eq!(l[3], "00:01:05.50,POW_LEV,v1_a1,-10.00,GPS,ALL");
        assert_eq!(l[4], "00:01:10.00,POW_ON,v1_a1,0,GALILEO,3");
        assert_eq!(l[5], "-,AR");
    }
}
