// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Safran Skydel GNSS simulator over its TCP remote API — **template profile**.
//!
//! Skydel is controlled remotely through a TCP command API (the vendor's SDK wraps it in
//! Python/C++ clients). Its exact wire format, command names and parameter names are
//! defined by the Skydel SDK / user manual of the installed version and are **not
//! verified here**. This module therefore implements the command channel generically:
//!
//! * the framing is pluggable ([`super::profile::CommandEncoder`]; the template uses one
//!   JSON object per line — switch `framing` to a length-prefixed variant, or plug a custom
//!   encoder with [`SkydelAdapter::with_encoder`], to match the real protocol);
//! * each of our operations maps to command templates in an editable TOML profile
//!   ([`DEFAULT_PROFILE`]) with `${…}` placeholders.
//!
//! What must be confirmed against the Skydel documentation before use (the profile has
//! `confirmed = false`): the TCP port, the frame format, every `CmdName` and parameter
//! name/unit (radians vs degrees, dB conventions), the response format used for
//! success/failure detection, the status strings, whether interference can be added while
//! running, and how hardware-in-the-loop trajectory streaming is carried (Skydel may use a
//! dedicated streaming socket and timing rules for HIL position pushes; the template sends
//! them on the command channel).

use super::export::sample_trajectory;
use super::motion::motion_from_traj;
use super::profile::{
    delegate_profile_adapter, CommandEncoder, CommandProfile, GnssSimControl, ProfileAdapter,
};
use crate::scenario::Scenario;

/// The editable template profile (TOML). Command names are placeholders modelled on the
/// style of the Skydel SDK and must be confirmed against the vendor manual.
pub const DEFAULT_PROFILE: &str = r#"
name = "skydel-template"
vendor = "Safran Skydel"
confirmed = false
notes = [
  "TEMPLATE: command names, parameters, framing, port and responses are NOT verified.",
  "Confirm every command against the Skydel SDK / remote API documentation of the installed version.",
  "Trajectory streaming (HIL) may require Skydel's dedicated streaming mechanism; confirm before use.",
]
framing = "line"
default_port = 4820
response = "every_command"
error_patterns = ["Failure", "Exception", "\"Success\":false"]
epoch_scale = "gpst"
stream_rate_hz = 100.0

[constellations]
gps = "GPS"
galileo = "Galileo"
glonass = "GLONASS"
beidou = "BeiDou"
qzss = "QZSS"
navic = "NavIC"
sbas = "SBAS"

[commands]
connect = ['{"CmdName":"GetSimulatorState"}']
set_start_time = ['{"CmdName":"SetGpsStartTime","Start":"${epoch_iso}"}']
load_trajectory = ['{"CmdName":"ImportVehicleTrajectory","Path":"${path}","Format":"${format}"}']
stream_begin = ['{"CmdName":"SetVehicleTrajectory","Name":"HIL"}']
stream_sample = ['{"CmdName":"PushEcefNed","ElapsedTime":${t_s},"X":${x},"Y":${y},"Z":${z},"VX":${vx},"VY":${vy},"VZ":${vz},"Yaw":${yaw_rad},"Pitch":${pitch_rad},"Roll":${roll_rad}}']
set_power_offset = ['{"CmdName":"SetSignalPowerOffset","System":"${constellation}","Signal":"${signal}","Offset":${offset_db}}']
sat_disable = ['{"CmdName":"EnableSV","System":"${constellation}","SvId":${prn},"Enabled":false}']
sat_enable = ['{"CmdName":"EnableSV","System":"${constellation}","SvId":${prn},"Enabled":true}']
signal_off = ['{"CmdName":"EnableSignal","System":"${constellation}","Signal":"${signal}","Enabled":false}']
signal_on = ['{"CmdName":"EnableSignal","System":"${constellation}","Signal":"${signal}","Enabled":true}']
jammer_start = ['{"CmdName":"AddInterference","Id":"${jammer_id}","Type":"${kind}","CentralFreq":${center_hz},"Bandwidth":${bandwidth_hz},"Power":${power_dbm}}']
jammer_stop = ['{"CmdName":"RemoveInterference","Id":"${jammer_id}"}']
arm_pps = ['{"CmdName":"ArmPPS"}']
arm_trigger = ['{"CmdName":"Arm"}']
start_immediate = ['{"CmdName":"Start"}']
stop = ['{"CmdName":"Stop"}']
status = ['{"CmdName":"GetSimulatorState"}']

[[status_map]]
contains = "Error"
state = "error"
[[status_map]]
contains = "Streaming"
state = "running"
[[status_map]]
contains = "Started"
state = "running"
[[status_map]]
contains = "Armed"
state = "armed"
[[status_map]]
contains = "Stopped"
state = "stopped"
[[status_map]]
contains = "Idle"
state = "idle"
"#;

/// The default template profile.
pub fn default_profile() -> CommandProfile {
    CommandProfile::from_toml(DEFAULT_PROFILE).expect("built-in Skydel profile parses")
}

/// Skydel adapter: a [`ProfileAdapter`] with the Skydel profile and trajectory streaming.
pub struct SkydelAdapter(pub ProfileAdapter);

delegate_profile_adapter!(SkydelAdapter);

impl SkydelAdapter {
    /// `addr` is `host:port`; with the template profile the port is usually
    /// `default_port` (to be confirmed).
    pub fn new(addr: &str) -> SkydelAdapter {
        SkydelAdapter(ProfileAdapter::new(addr, default_profile()))
    }
    pub fn with_profile(addr: &str, profile: CommandProfile) -> SkydelAdapter {
        SkydelAdapter(ProfileAdapter::new(addr, profile))
    }
    /// Plug a custom frame encoder (e.g. the real Skydel wire format).
    pub fn with_encoder(
        addr: &str,
        profile: CommandProfile,
        encoder: Box<dyn CommandEncoder>,
    ) -> SkydelAdapter {
        SkydelAdapter(ProfileAdapter::with_encoder(addr, profile, encoder))
    }
    /// Sample the scenario trajectory at `rate_hz` (profile default when `None`) and
    /// stream position/velocity/attitude; `pace(t_s)` is called before each sample (use
    /// it to send in real time against the scenario clock).
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
    use super::super::profile::{mock, Constellation, Framing, LengthPrefixedEncoder};
    use super::super::{arm_gnss, EpochScale, GnssSimAdapter, GnssSimState, SyncTimingModel};
    use super::*;

    const SC: &str = r#"
[scene]
start_utc = "2026-11-18T12:00:00Z"
duration_s = 2.0
fs_hz = 1000000
[receiver]
type = "waypoints"
points = [
  { t_s = 0.0, lat_deg = 55.5, lon_deg = -3.25, h_m = 10.0 },
  { t_s = 100.0, lat_deg = 55.51, lon_deg = -3.25, h_m = 110.0 },
]
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

    fn reply() -> impl FnMut(&str) -> Option<String> + Send {
        let mut state = "Idle";
        move |c: &str| {
            let v: serde_json::Value = serde_json::from_str(c).expect("JSON command");
            let name = v["CmdName"].as_str().unwrap().to_string();
            Some(match name.as_str() {
                "GetSimulatorState" => format!("{{\"CmdName\":\"Result\",\"State\":\"{state}\"}}"),
                "ArmPPS" => {
                    state = "Armed";
                    "{\"CmdName\":\"SuccessResult\"}".into()
                }
                "Stop" => {
                    state = "Stopped";
                    "{\"CmdName\":\"SuccessResult\"}".into()
                }
                "EnableSV" if v["SvId"] == 99 => "{\"CmdName\":\"Failure\",\"Msg\":\"bad SV\"}".into(),
                _ => "{\"CmdName\":\"SuccessResult\"}".into(),
            })
        }
    }

    #[test]
    fn profile_is_marked_unconfirmed() {
        let p = default_profile();
        assert!(!p.confirmed);
        assert!(p.notes.iter().any(|n| n.contains("NOT verified")));
        assert_eq!(p.epoch_scale, EpochScale::Gpst);
    }

    #[test]
    fn handshake_power_mask_and_streaming_against_mock() {
        let (addr, log, h) = mock::serve(Framing::Line, reply());
        let mut a = SkydelAdapter::new(&addr);
        let sc = Scenario::from_toml(SC).unwrap();
        let plan = SyncTimingModel {
            common_10mhz: true,
            common_pps: true,
            eloran_minus_gnss_ns: 0.0,
            eloran_minus_gnss_u_ns: 0.0,
            gnss_arm: super::super::ArmMode::NextPps,
            epoch_scale: EpochScale::Utc,
        }
        .plan(&sc.scene.start_utc)
        .unwrap();
        let hs = arm_gnss(&mut a, &plan, None).unwrap();
        assert_eq!(hs.status.state, GnssSimState::Armed);
        a.set_power_offset(Some(Constellation::Galileo), Some("E1"), -4.5)
            .unwrap();
        let n = a.stream_trajectory(&sc, Some(10.0), &mut |_| {}).unwrap();
        assert_eq!(n, 21);
        let e = a
            .send_op(
                super::super::profile::ops::SAT_DISABLE,
                &[
                    ("constellation".to_string(), "GPS".to_string()),
                    ("prn".to_string(), "99".to_string()),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap_err();
        assert!(e.contains("Failure"), "{e}");
        a.stop().unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Stopped);
        drop(a);
        h.join().unwrap();
        let l = log.lock().unwrap().clone();
        // epoch in GPST calendar (profile epoch_scale) although the plan was UTC
        assert!(l.iter().any(|c| c.contains("\"Start\":\"2026-11-18T12:00:18.000000000Z\"")), "{l:?}");
        assert!(l.iter().any(|c| c.contains("\"System\":\"Galileo\",\"Signal\":\"E1\",\"Offset\":-4.50")));
        let pushes: Vec<serde_json::Value> = l
            .iter()
            .filter(|c| c.contains("PushEcefNed"))
            .map(|c| serde_json::from_str(c).unwrap())
            .collect();
        assert_eq!(pushes.len(), 21);
        assert_eq!(pushes[10]["ElapsedTime"].as_f64().unwrap(), 1.0);
        // climbing 1 m/s while moving ≈ 11.1 m/s north: VZ positive, pitch ≈ 5.1°
        let pitch = pushes[5]["Pitch"].as_f64().unwrap().to_degrees();
        assert!((pitch - (1.0f64).atan2(11.13).to_degrees()).abs() < 0.1, "{pitch}");
        assert!(pushes[0]["X"].as_f64().unwrap() > 3.0e6);
    }

    #[test]
    fn length_prefixed_encoder_can_be_plugged() {
        let (addr, log, h) = mock::serve(Framing::U32BePrefixed, reply());
        let mut a = SkydelAdapter::with_encoder(
            &addr,
            default_profile(),
            Box::new(LengthPrefixedEncoder {
                big_endian: true,
                max_len: 1 << 20,
            }),
        );
        assert!(a.connect().unwrap().contains("Idle"));
        a.arm(super::super::ArmMode::Immediate).unwrap();
        drop(a);
        h.join().unwrap();
        assert_eq!(log.lock().unwrap()[1], "{\"CmdName\":\"Start\"}");
    }
}
