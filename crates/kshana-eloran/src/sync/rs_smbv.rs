// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Rohde & Schwarz SMBV100B / SMW200A GNSS option via SCPI over TCP (raw socket, usually
//! port 5025) — **template profile**.
//!
//! The instrument is SCPI, so framing (newline text, queries end in `?`) and the error
//! queue (`SYST:ERR?` → `0,"No error"`) follow IEEE 488.2 / SCPI conventions and are
//! reliable. The **GNSS subsystem command paths** in [`DEFAULT_PROFILE`] (`:BB:GNSS:…`)
//! are written in the style of the R&S GNSS user manuals but are **not verified**; they
//! differ between SMBV100A/B, SMW200A, firmware and installed options (K44/K66/K107/K108…).
//! Confirm in the instrument's GNSS user manual (and with the instrument's SCPI recorder)
//! before use: start date/time and time basis, trigger/arm commands (PPS used as external
//! trigger), movement file loading, the real-time HIL position command and its parameter
//! list, per-system and per-SV power/state commands, and the status query. Interference
//! (AWGN/CW/second baseband) is option-specific and left unsupported in the template.
//!
//! The status query used by the template (`:BB:GNSS:TRIG:RMOD?`) returns run/stop only,
//! which cannot separate "armed, waiting for trigger" from "stopped";
//! [`SmbvAdapter::status`] resolves this from the last command it sent.

use super::export::sample_trajectory;
use super::motion::motion_from_traj;
use super::profile::{CommandProfile, Fields, GnssSimControl, ProfileAdapter};
use super::{ArmMode, GnssSimAdapter, GnssSimState, GnssSimStatus, StartEpoch, TrajectoryFile};
use crate::scenario::Scenario;

/// Editable template profile (TOML). Not a verified R&S command set.
pub const DEFAULT_PROFILE: &str = r#"
name = "rs-smbv-smw-gnss-template"
vendor = "Rohde & Schwarz SMBV100B / SMW200A (GNSS)"
confirmed = false
notes = [
  "TEMPLATE: SCPI framing and SYST:ERR? are standard; the :BB:GNSS command paths are NOT verified.",
  "Confirm every path in the GNSS user manual of the instrument/firmware/options (use the SCPI recorder).",
]
framing = "line"
default_port = 5025
response = "queries_only"
error_query = "SYST:ERR?"
error_ok_prefix = "0"
epoch_scale = "utc"
stream_rate_hz = 100.0
all_name = "ALL"

[constellations]
gps = "GPS"
galileo = "GALileo"
glonass = "GLONass"
beidou = "BEIDou"
qzss = "QZSS"
navic = "NAVic"
sbas = "SBAS"

[commands]
connect = ["*IDN?", "*CLS"]
set_start_time = [
  ":SOUR1:BB:GNSS:TIME:STAR:TBAS UTC",
  ":SOUR1:BB:GNSS:TIME:STAR:DATE ${year},${month},${day}",
  ":SOUR1:BB:GNSS:TIME:STAR:TIME ${hour},${minute},${second}",
]
load_trajectory = [
  ":SOUR1:BB:GNSS:REC:V1:POS MOV",
  ":SOUR1:BB:GNSS:REC:V1:MOV:FILE '${path}'",
]
stream_begin = [":SOUR1:BB:GNSS:REC:V1:POS RT"]
stream_sample = [":SOUR1:BB:GNSS:RT:REC:V1:HILP:MODE:A ${t_s},${x},${y},${z},${vx},${vy},${vz},${yaw_rad},${pitch_rad},${roll_rad}"]
set_power_offset = [":SOUR1:BB:GNSS:SYST:${constellation}:POW:OFFS ${offset_db}"]
sat_disable = [":SOUR1:BB:GNSS:SVID${prn}:${constellation}:STAT 0"]
sat_enable = [":SOUR1:BB:GNSS:SVID${prn}:${constellation}:STAT 1"]
signal_off = [":SOUR1:BB:GNSS:SYST:${constellation}:STAT 0"]
signal_on = [":SOUR1:BB:GNSS:SYST:${constellation}:STAT 1"]
arm_pps = [":SOUR1:BB:GNSS:TRIG:SOUR EXT", ":SOUR1:BB:GNSS:TRIG:SEQ AAUT", ":SOUR1:BB:GNSS:STAT ON"]
arm_trigger = [":SOUR1:BB:GNSS:TRIG:SOUR EXT", ":SOUR1:BB:GNSS:TRIG:SEQ AAUT", ":SOUR1:BB:GNSS:STAT ON"]
start_immediate = [":SOUR1:BB:GNSS:TRIG:SEQ AUTO", ":SOUR1:BB:GNSS:STAT ON"]
stop = [":SOUR1:BB:GNSS:STAT OFF"]
status = [":SOUR1:BB:GNSS:TRIG:RMOD?"]

[[status_map]]
contains = "RUN"
state = "running"
[[status_map]]
contains = "STOP"
state = "stopped"
"#;

pub fn default_profile() -> CommandProfile {
    CommandProfile::from_toml(DEFAULT_PROFILE).expect("built-in R&S profile parses")
}

/// R&S SMBV/SMW adapter.
pub struct SmbvAdapter {
    pub inner: ProfileAdapter,
    last: Option<ArmMode>,
    stopped: bool,
}

impl SmbvAdapter {
    pub fn new(addr: &str) -> SmbvAdapter {
        SmbvAdapter::with_profile(addr, default_profile())
    }
    pub fn with_profile(addr: &str, profile: CommandProfile) -> SmbvAdapter {
        SmbvAdapter {
            inner: ProfileAdapter::new(addr, profile),
            last: None,
            stopped: false,
        }
    }
    /// Stream the scenario trajectory as real-time HIL positions.
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

impl GnssSimAdapter for SmbvAdapter {
    fn connect(&mut self) -> Result<String, String> {
        self.inner.connect()
    }
    fn set_start_epoch(&mut self, e: &StartEpoch) -> Result<(), String> {
        self.inner.set_start_epoch(e)
    }
    fn load_trajectory(&mut self, t: &TrajectoryFile) -> Result<(), String> {
        self.inner.load_trajectory(t)
    }
    fn arm(&mut self, m: ArmMode) -> Result<(), String> {
        self.inner.arm(m)?;
        self.last = Some(m);
        self.stopped = false;
        Ok(())
    }
    /// The instrument's run/stop answer, refined: "stopped" after an arm (and no stop)
    /// means armed and waiting for the trigger.
    fn status(&mut self) -> Result<GnssSimStatus, String> {
        let mut s = self.inner.status()?;
        if s.state == GnssSimState::Stopped
            && !self.stopped
            && matches!(
                self.last,
                Some(ArmMode::NextPps) | Some(ArmMode::ExternalTrigger)
            )
        {
            s.state = GnssSimState::Armed;
        }
        if s.state == GnssSimState::Stopped && self.last.is_none() {
            s.state = GnssSimState::Idle;
        }
        Ok(s)
    }
    fn stop(&mut self) -> Result<(), String> {
        self.inner.stop()?;
        self.stopped = true;
        Ok(())
    }
}

impl GnssSimControl for SmbvAdapter {
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
    use super::super::profile::{mock, Constellation, Framing};
    use super::super::EpochScale;
    use super::*;

    #[test]
    fn scpi_session_with_error_queue_against_mock() {
        let mut running = false;
        let mut err = String::from("0,\"No error\"");
        let (addr, log, h) = mock::serve(Framing::Line, move |c| match c {
            "*IDN?" => Some("Rohde&Schwarz,SMBV100B,0000,5.00 (mock)".into()),
            "SYST:ERR?" => Some(std::mem::replace(&mut err, "0,\"No error\"".into())),
            ":SOUR1:BB:GNSS:TRIG:RMOD?" => Some(if running { "RUN" } else { "STOP" }.into()),
            ":SOUR1:BB:GNSS:TRIG:SEQ AUTO" => {
                running = true;
                None
            }
            c if c.contains("SVID99") => {
                err = "-222,\"Data out of range\"".into();
                None
            }
            _ => None,
        });
        let mut a = SmbvAdapter::new(&addr);
        assert!(a.connect().unwrap().contains("SMBV100B"));
        assert_eq!(a.status().unwrap().state, GnssSimState::Idle);
        a.set_start_epoch(
            &StartEpoch::from_utc_iso("2026-11-18T01:02:03Z", EpochScale::Gpst).unwrap(),
        )
        .unwrap();
        a.set_power_offset(Some(Constellation::Galileo), None, -2.0)
            .unwrap();
        let mut f = Fields::new();
        f.insert("constellation".into(), "GPS".into());
        f.insert("prn".into(), "99".into());
        let e = a
            .send_op(super::super::profile::ops::SAT_DISABLE, &f)
            .unwrap_err();
        assert!(e.contains("-222"), "{e}");
        a.arm(ArmMode::NextPps).unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Armed);
        a.stop().unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Stopped);
        a.arm(ArmMode::Immediate).unwrap();
        assert_eq!(a.status().unwrap().state, GnssSimState::Running);
        drop(a);
        h.join().unwrap();
        let l = log.lock().unwrap().clone();
        // profile epoch scale (UTC) wins over the caller's
        assert!(
            l.contains(&":SOUR1:BB:GNSS:TIME:STAR:TIME 01,02,03".to_string()),
            "{l:?}"
        );
        assert!(l.contains(&":SOUR1:BB:GNSS:SYST:GALileo:POW:OFFS -2.00".to_string()));
        assert!(l.contains(&":SOUR1:BB:GNSS:TRIG:SOUR EXT".to_string()));
        // every setting command is followed by an error-queue check
        let i = l
            .iter()
            .position(|c| c == ":SOUR1:BB:GNSS:STAT OFF")
            .unwrap();
        assert_eq!(l[i + 1], "SYST:ERR?");
    }
}
