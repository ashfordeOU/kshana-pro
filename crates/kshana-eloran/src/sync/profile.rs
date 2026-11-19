// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Profile-driven remote control of GNSS simulators over TCP.
//!
//! Vendor remote-control protocols differ in framing (newline text, length-prefixed
//! frames), in whether every command is acknowledged, and in their command vocabulary.
//! This module separates the three:
//!
//! * a [`CommandEncoder`] frames one command and reads one response (built-ins:
//!   [`LineEncoder`], [`LengthPrefixedEncoder`]; a custom encoder can be plugged in with
//!   [`ProfileChannel::with_encoder`]);
//! * a [`CommandProfile`] (TOML, editable) maps our operations ([`ops`]) to command
//!   templates with `${placeholder}` fields, and says how responses are judged and which
//!   status texts mean which [`GnssSimState`];
//! * [`ProfileAdapter`] implements [`GnssSimAdapter`] and the extension
//!   [`GnssSimControl`] (power offsets, motion streaming, timed command schedules) on top.
//!
//! **No profile in this crate is a verified vendor command set.** The vendor modules
//! ([`super::skydel`], [`super::spirent`], [`super::rs_smbv`], [`super::labsat`]) ship
//! *templates* whose `confirmed` flag is `false`; every command must be checked against
//! the vendor's remote-control manual for the installed software version before use, and
//! the profile edited accordingly (then set `confirmed = true` and record the manual
//! revision in `notes`).

use super::motion::MotionSample;
use super::{
    ArmMode, EpochScale, GnssSimAdapter, GnssSimState, GnssSimStatus, StartEpoch, TrajFormat,
    TrajectoryFile,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Template placeholder values.
pub type Fields = BTreeMap<String, String>;

/// Operation names used as keys of [`CommandProfile::commands`].
pub mod ops {
    /// Sent after connecting; the first response is the identity.
    pub const CONNECT: &str = "connect";
    /// Start epoch; fields of [`crate::sync::StartEpoch::fields`].
    pub const SET_START_TIME: &str = "set_start_time";
    /// Load a trajectory file; fields `path`, `format`, `rate_hz`.
    pub const LOAD_TRAJECTORY: &str = "load_trajectory";
    /// Before / per sample / after a streamed trajectory; sample fields of
    /// [`crate::sync::motion::MotionSample::fields`], plus `rate_hz`, `n`.
    pub const STREAM_BEGIN: &str = "stream_begin";
    pub const STREAM_SAMPLE: &str = "stream_sample";
    pub const STREAM_END: &str = "stream_end";
    /// Absolute power offset (dB) of a constellation/signal group: `constellation`,
    /// `signal`, `offset_db`.
    pub const SET_POWER_OFFSET: &str = "set_power_offset";
    /// Absolute C/N0 offset (dB); falls back to `set_power_offset` when not defined.
    pub const SET_CN0_OFFSET: &str = "set_cn0_offset";
    /// Satellite off / on: `constellation`, `prn`.
    pub const SAT_DISABLE: &str = "sat_disable";
    pub const SAT_ENABLE: &str = "sat_enable";
    /// Whole signal (or constellation) off / on: `constellation`, `signal`.
    pub const SIGNAL_OFF: &str = "signal_off";
    pub const SIGNAL_ON: &str = "signal_on";
    /// Interference: `jammer_id`, `kind`, `center_hz`, `center_mhz`, `bandwidth_hz`,
    /// `js_db`, `power_dbm`.
    pub const JAMMER_START: &str = "jammer_start";
    pub const JAMMER_STOP: &str = "jammer_stop";
    /// Spoofing scenario markers: `label`.
    pub const SPOOF_START: &str = "spoof_start";
    pub const SPOOF_STOP: &str = "spoof_stop";
    pub const ARM_PPS: &str = "arm_pps";
    pub const ARM_TRIGGER: &str = "arm_trigger";
    pub const START_IMMEDIATE: &str = "start_immediate";
    pub const STOP: &str = "stop";
    /// Status query (one command; its response is matched against the status map).
    pub const STATUS: &str = "status";
}

/// GNSS constellations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Constellation {
    Gps,
    Galileo,
    Glonass,
    Beidou,
    Qzss,
    Navic,
    Sbas,
}

impl Constellation {
    pub const ALL: [Constellation; 7] = [
        Constellation::Gps,
        Constellation::Galileo,
        Constellation::Glonass,
        Constellation::Beidou,
        Constellation::Qzss,
        Constellation::Navic,
        Constellation::Sbas,
    ];
    /// Key used in profiles (`gps`, `galileo`, …).
    pub fn key(self) -> &'static str {
        match self {
            Constellation::Gps => "gps",
            Constellation::Galileo => "galileo",
            Constellation::Glonass => "glonass",
            Constellation::Beidou => "beidou",
            Constellation::Qzss => "qzss",
            Constellation::Navic => "navic",
            Constellation::Sbas => "sbas",
        }
    }
}

/// `HH:MM:SS.ss` of a scene time (hours may exceed 23).
pub fn hms(t_s: f64) -> String {
    let cs = (t_s.max(0.0) * 100.0).round() as u64;
    let s = cs / 100;
    format!(
        "{:02}:{:02}:{:02}.{:02}",
        s / 3600,
        (s / 60) % 60,
        s % 60,
        cs % 100
    )
}

/// Fill `${key}` placeholders (`$$` is a literal `$`); an unknown key is an error. The
/// `${…}` syntax leaves JSON braces in templates untouched.
pub fn fill_template(template: &str, fields: &Fields) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = template;
    while let Some(a) = rest.find('$') {
        out.push_str(&rest[..a]);
        let tail = &rest[a + 1..];
        if let Some(t) = tail.strip_prefix('$') {
            out.push('$');
            rest = t;
        } else if let Some(t) = tail.strip_prefix('{') {
            let b = t
                .find('}')
                .ok_or_else(|| format!("unclosed placeholder in {template:?}"))?;
            let key = &t[..b];
            out.push_str(
                fields
                    .get(key)
                    .ok_or_else(|| format!("unknown placeholder ${{{key}}} in {template:?}"))?,
            );
            rest = &t[b + 1..];
        } else {
            out.push('$');
            rest = tail;
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Frame format on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Framing {
    /// Text lines ended by [`CommandProfile::terminator`]; responses end in `\n`.
    #[default]
    Line,
    /// 4-byte big-endian payload length, then the payload.
    U32BePrefixed,
    /// 4-byte little-endian payload length, then the payload.
    U32LePrefixed,
}

/// When the simulator answers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    /// Only queries (first word ends in [`CommandProfile::query_suffix`]) are answered.
    #[default]
    QueriesOnly,
    /// Every command gets exactly one response.
    EveryCommand,
    /// Nothing is ever read back (status is then unavailable).
    None,
}

/// How a timed schedule is executed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleMode {
    /// The host sends each command at its time (after the scenario started).
    #[default]
    HostTimed,
    /// Commands carry a scenario time tag (`${t_tag}`) and may all be sent up front.
    TimeTagged,
}

/// Format of `${t_tag}` for time-tagged commands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeTagFormat {
    /// `HH:MM:SS.ss` scenario time.
    #[default]
    Hms,
    /// Seconds with millisecond resolution.
    Seconds,
}

/// A status text rule: a response containing `contains` means `state`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StatusRule {
    pub contains: String,
    pub state: GnssSimState,
}

/// An editable remote-control profile (TOML). See the module documentation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandProfile {
    pub name: String,
    pub vendor: String,
    /// `true` only once every command has been checked against the vendor manual.
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub framing: Framing,
    /// Line terminator for [`Framing::Line`] (default `"\n"`).
    #[serde(default = "d_term")]
    pub terminator: String,
    #[serde(default)]
    pub default_port: Option<u16>,
    #[serde(default)]
    pub response: ResponseMode,
    #[serde(default = "d_qs")]
    pub query_suffix: String,
    /// If non-empty, every response to a non-query must contain this.
    #[serde(default)]
    pub ok_pattern: String,
    /// A response containing any of these fails the command.
    #[serde(default)]
    pub error_patterns: Vec<String>,
    /// Optional error query after every non-query command (SCPI `SYST:ERR?`).
    #[serde(default)]
    pub error_query: Option<String>,
    #[serde(default)]
    pub error_ok_prefix: String,
    #[serde(default)]
    pub schedule: ScheduleMode,
    #[serde(default)]
    pub time_tag_format: TimeTagFormat,
    /// `${t_tag}` of untimed (immediate) commands.
    #[serde(default = "d_imm")]
    pub immediate_tag: String,
    /// Scale of the start epoch.
    #[serde(default)]
    pub epoch_scale: EpochScale,
    /// Our constellation key → vendor name (default: upper-case key).
    #[serde(default)]
    pub constellations: BTreeMap<String, String>,
    /// Our signal name → vendor name (default: unchanged).
    #[serde(default)]
    pub signals: BTreeMap<String, String>,
    /// `${signal}` / `${constellation}` when an event applies to all.
    #[serde(default = "d_all")]
    pub all_name: String,
    /// Nominal signal power (dBm) that J/S is referred to for `${power_dbm}`.
    #[serde(default = "d_ref_pwr")]
    pub ref_signal_power_dbm: f64,
    /// Default rate of streamed motion (Hz).
    #[serde(default = "d_rate")]
    pub stream_rate_hz: f64,
    /// Operation → command templates (see [`ops`]). Missing / empty = unsupported.
    #[serde(default)]
    pub commands: BTreeMap<String, Vec<String>>,
    /// Status rules, checked in order.
    #[serde(default)]
    pub status_map: Vec<StatusRule>,
}

fn d_term() -> String {
    "\n".into()
}
fn d_qs() -> String {
    "?".into()
}
fn d_imm() -> String {
    "-".into()
}
fn d_all() -> String {
    "ALL".into()
}
fn d_ref_pwr() -> f64 {
    -128.5
}
fn d_rate() -> f64 {
    10.0
}

impl CommandProfile {
    pub fn from_toml(src: &str) -> Result<CommandProfile, String> {
        let p: CommandProfile = toml::from_str(src).map_err(|e| format!("profile: {e}"))?;
        p.validate()?;
        Ok(p)
    }
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("serialisable")
    }
    pub fn load(path: &std::path::Path) -> Result<CommandProfile, String> {
        CommandProfile::from_toml(
            &std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )
    }
    /// Check the templates parse and every op name is known.
    pub fn validate(&self) -> Result<(), String> {
        const KNOWN: &[&str] = &[
            ops::CONNECT,
            ops::SET_START_TIME,
            ops::LOAD_TRAJECTORY,
            ops::STREAM_BEGIN,
            ops::STREAM_SAMPLE,
            ops::STREAM_END,
            ops::SET_POWER_OFFSET,
            ops::SET_CN0_OFFSET,
            ops::SAT_DISABLE,
            ops::SAT_ENABLE,
            ops::SIGNAL_OFF,
            ops::SIGNAL_ON,
            ops::JAMMER_START,
            ops::JAMMER_STOP,
            ops::SPOOF_START,
            ops::SPOOF_STOP,
            ops::ARM_PPS,
            ops::ARM_TRIGGER,
            ops::START_IMMEDIATE,
            ops::STOP,
            ops::STATUS,
        ];
        for (k, v) in &self.commands {
            if !KNOWN.contains(&k.as_str()) {
                return Err(format!("profile {}: unknown operation {k:?}", self.name));
            }
            for t in v {
                // unclosed placeholders are caught here; unknown keys only at fill time
                let mut rest = t.as_str();
                while let Some(a) = rest.find("${") {
                    match rest[a..].find('}') {
                        Some(b) => rest = &rest[a + b + 1..],
                        None => {
                            return Err(format!(
                                "profile {}: unclosed placeholder in {t:?}",
                                self.name
                            ))
                        }
                    }
                }
            }
        }
        if self.stream_rate_hz.is_nan() || self.stream_rate_hz <= 0.0 {
            return Err(format!("profile {}: stream_rate_hz must be > 0", self.name));
        }
        Ok(())
    }
    /// Templates of `op` (empty slice when unsupported).
    pub fn templates(&self, op: &str) -> &[String] {
        self.commands.get(op).map(|v| v.as_slice()).unwrap_or(&[])
    }
    pub fn supports(&self, op: &str) -> bool {
        !self.templates(op).is_empty()
    }
    pub fn is_query(&self, cmd: &str) -> bool {
        !self.query_suffix.is_empty()
            && cmd
                .split_whitespace()
                .next()
                .is_some_and(|w| w.ends_with(self.query_suffix.as_str()))
    }
    /// Vendor name of a constellation (`None` = all).
    pub fn constellation_name(&self, c: Option<Constellation>) -> String {
        match c {
            None => self.all_name.clone(),
            Some(c) => self
                .constellations
                .get(c.key())
                .cloned()
                .unwrap_or_else(|| c.key().to_uppercase()),
        }
    }
    /// Vendor name of a signal (`None` = all).
    pub fn signal_name(&self, s: Option<&str>) -> String {
        match s {
            None => self.all_name.clone(),
            Some(s) => self
                .signals
                .get(s)
                .cloned()
                .unwrap_or_else(|| s.to_string()),
        }
    }
    /// `t_s`, `t_hms`, `t_tag` fields for a command at scene time `t_s` (`None` =
    /// immediate).
    pub fn time_fields(&self, t_s: Option<f64>) -> Fields {
        let mut f = Fields::new();
        let t = t_s.unwrap_or(0.0);
        f.insert("t_s".into(), format!("{t:.3}"));
        f.insert("t_hms".into(), hms(t));
        f.insert(
            "t_tag".into(),
            match t_s {
                None => self.immediate_tag.clone(),
                Some(t) => match self.time_tag_format {
                    TimeTagFormat::Hms => hms(t),
                    TimeTagFormat::Seconds => format!("{t:.3}"),
                },
            },
        );
        f
    }
    /// Constellation/signal group fields.
    pub fn group_fields(&self, c: Option<Constellation>, signal: Option<&str>) -> Fields {
        let mut f = Fields::new();
        f.insert("constellation".into(), self.constellation_name(c));
        f.insert("signal".into(), self.signal_name(signal));
        f
    }
    /// Render the command lines of `op` (immediate time fields merged under `fields`).
    pub fn render(&self, op: &str, fields: &Fields) -> Result<Vec<String>, String> {
        let mut all = self.time_fields(None);
        all.extend(fields.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.templates(op)
            .iter()
            .map(|t| fill_template(t, &all))
            .collect()
    }
    /// Map a status response to a state.
    pub fn state_of(&self, raw: &str) -> GnssSimState {
        self.status_map
            .iter()
            .find(|r| !r.contains.is_empty() && raw.contains(&r.contains))
            .map(|r| r.state)
            .unwrap_or(GnssSimState::Unknown)
    }
}

// ---------------------------------------------------------------------------------------
// Encoders
// ---------------------------------------------------------------------------------------

/// Frames commands and reads responses.
pub trait CommandEncoder: Send {
    fn encode(&self, cmd: &str) -> Vec<u8>;
    /// Read one response; `Ok(None)` at end of stream.
    fn read_response(&self, r: &mut dyn BufRead) -> std::io::Result<Option<String>>;
}

/// Newline-terminated text.
#[derive(Clone, Debug)]
pub struct LineEncoder {
    pub terminator: String,
}

impl CommandEncoder for LineEncoder {
    fn encode(&self, cmd: &str) -> Vec<u8> {
        format!("{cmd}{}", self.terminator).into_bytes()
    }
    fn read_response(&self, r: &mut dyn BufRead) -> std::io::Result<Option<String>> {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        Ok(Some(line.trim_end_matches(['\r', '\n']).to_string()))
    }
}

/// 4-byte length prefix + UTF-8 payload.
#[derive(Clone, Debug)]
pub struct LengthPrefixedEncoder {
    pub big_endian: bool,
    /// Largest accepted response payload (bytes).
    pub max_len: usize,
}

impl CommandEncoder for LengthPrefixedEncoder {
    fn encode(&self, cmd: &str) -> Vec<u8> {
        let n = cmd.len() as u32;
        let mut v = if self.big_endian {
            n.to_be_bytes().to_vec()
        } else {
            n.to_le_bytes().to_vec()
        };
        v.extend_from_slice(cmd.as_bytes());
        v
    }
    fn read_response(&self, r: &mut dyn BufRead) -> std::io::Result<Option<String>> {
        let mut h = [0u8; 4];
        match r.read_exact(&mut h) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        let n = if self.big_endian {
            u32::from_be_bytes(h)
        } else {
            u32::from_le_bytes(h)
        } as usize;
        if n > self.max_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("response frame of {n} bytes exceeds {}", self.max_len),
            ));
        }
        let mut b = vec![0u8; n];
        r.read_exact(&mut b)?;
        Ok(Some(String::from_utf8_lossy(&b).into_owned()))
    }
}

/// The built-in encoder of a profile.
pub fn encoder_for(p: &CommandProfile) -> Box<dyn CommandEncoder> {
    match p.framing {
        Framing::Line => Box::new(LineEncoder {
            terminator: p.terminator.clone(),
        }),
        Framing::U32BePrefixed | Framing::U32LePrefixed => Box::new(LengthPrefixedEncoder {
            big_endian: p.framing == Framing::U32BePrefixed,
            max_len: 16 << 20,
        }),
    }
}

// ---------------------------------------------------------------------------------------
// Channel
// ---------------------------------------------------------------------------------------

/// A TCP command channel driven by a profile.
pub struct ProfileChannel {
    pub addr: String,
    pub profile: CommandProfile,
    pub timeout: Duration,
    encoder: Box<dyn CommandEncoder>,
    stream: Option<(TcpStream, BufReader<TcpStream>)>,
    /// Every frame sent and received (`> cmd`, `< response`).
    pub transcript: Vec<String>,
}

impl ProfileChannel {
    pub fn new(addr: &str, profile: CommandProfile) -> ProfileChannel {
        let encoder = encoder_for(&profile);
        ProfileChannel::with_encoder(addr, profile, encoder)
    }
    /// Use a custom encoder instead of the profile's built-in framing.
    pub fn with_encoder(
        addr: &str,
        profile: CommandProfile,
        encoder: Box<dyn CommandEncoder>,
    ) -> ProfileChannel {
        ProfileChannel {
            addr: addr.into(),
            profile,
            timeout: Duration::from_secs(5),
            encoder,
            stream: None,
            transcript: vec![],
        }
    }
    pub fn is_open(&self) -> bool {
        self.stream.is_some()
    }
    pub fn open(&mut self) -> Result<(), String> {
        use std::net::ToSocketAddrs;
        let sa = self
            .addr
            .to_socket_addrs()
            .map_err(|e| format!("{}: {e}", self.addr))?
            .next()
            .ok_or_else(|| format!("{}: no address", self.addr))?;
        let s = TcpStream::connect_timeout(&sa, self.timeout)
            .map_err(|e| format!("{}: {e}", self.addr))?;
        s.set_read_timeout(Some(self.timeout))
            .map_err(|e| e.to_string())?;
        s.set_nodelay(true).ok();
        let r = BufReader::new(s.try_clone().map_err(|e| e.to_string())?);
        self.stream = Some((s, r));
        Ok(())
    }
    pub fn close(&mut self) {
        self.stream = None;
    }
    fn expects_response(&self, cmd: &str) -> bool {
        match self.profile.response {
            ResponseMode::None => false,
            ResponseMode::EveryCommand => true,
            ResponseMode::QueriesOnly => self.profile.is_query(cmd),
        }
    }
    /// Send one raw command; returns its response when one is expected.
    pub fn send_raw(&mut self, cmd: &str) -> Result<Option<String>, String> {
        let want = self.expects_response(cmd);
        let (w, r) = self.stream.as_mut().ok_or("not connected")?;
        w.write_all(&self.encoder.encode(cmd))
            .map_err(|e| format!("{}: {e}", self.addr))?;
        self.transcript.push(format!("> {cmd}"));
        if !want {
            return Ok(None);
        }
        let resp = self
            .encoder
            .read_response(r)
            .map_err(|e| format!("{}: {e}", self.addr))?
            .ok_or_else(|| format!("{}: connection closed", self.addr))?;
        self.transcript.push(format!("< {resp}"));
        Ok(Some(resp))
    }
    /// Send a command and judge it: error patterns, ok pattern (non-queries) and the
    /// error query.
    pub fn command(&mut self, cmd: &str) -> Result<Option<String>, String> {
        let r = self.send_raw(cmd)?;
        let p = &self.profile;
        if let Some(resp) = &r {
            if let Some(e) = p
                .error_patterns
                .iter()
                .find(|e| !e.is_empty() && resp.contains(e.as_str()))
            {
                return Err(format!("simulator rejected {cmd:?} ({e}): {resp}"));
            }
            if !p.is_query(cmd) && !p.ok_pattern.is_empty() && !resp.contains(&p.ok_pattern) {
                return Err(format!("simulator did not acknowledge {cmd:?}: {resp}"));
            }
        }
        if !self.profile.is_query(cmd) {
            if let Some(q) = self.profile.error_query.clone() {
                let e = self.send_raw(&q)?.unwrap_or_default();
                if !e.starts_with(&self.profile.error_ok_prefix) {
                    return Err(format!("simulator rejected {cmd:?}: {e}"));
                }
            }
        }
        Ok(r)
    }
    /// Send pre-rendered lines; returns the responses.
    pub fn send_lines(&mut self, lines: &[String]) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for l in lines {
            if let Some(r) = self.command(l)? {
                out.push(r);
            }
        }
        Ok(out)
    }
    /// Render and send `op`; an unsupported op is an error.
    pub fn op(&mut self, op: &str, fields: &Fields) -> Result<Vec<String>, String> {
        if !self.profile.supports(op) {
            return Err(format!(
                "profile {} has no command for operation {op:?}",
                self.profile.name
            ));
        }
        let lines = self.profile.render(op, fields)?;
        self.send_lines(&lines)
    }
}

// ---------------------------------------------------------------------------------------
// Schedules
// ---------------------------------------------------------------------------------------

/// One timed operation of a schedule, rendered for a profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduledCommand {
    /// Scene time (s) at which the command takes effect.
    pub t_s: f64,
    pub op: String,
    /// Rendered command lines.
    pub lines: Vec<String>,
    /// Human-readable description of the source event.
    pub what: String,
}

/// A marker in a schedule that needs no command (e.g. a spoofing interval that the
/// simulator realises from its own scenario), kept for the test record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleMarker {
    pub t_s: f64,
    pub label: String,
}

/// Timed commands for one simulator profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandSchedule {
    pub profile: String,
    pub mode: ScheduleMode,
    /// Sorted by time.
    pub entries: Vec<ScheduledCommand>,
    pub markers: Vec<ScheduleMarker>,
    /// Events the profile cannot express (no template for the operation).
    pub unsupported: Vec<String>,
    /// `false` when the profile is an unconfirmed template.
    pub profile_confirmed: bool,
}

impl CommandSchedule {
    /// All lines in order (for a time-tagged upload).
    pub fn all_lines(&self) -> Vec<String> {
        self.entries.iter().flat_map(|e| e.lines.clone()).collect()
    }
}

// ---------------------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------------------

/// Extended control beyond [`GnssSimAdapter`].
pub trait GnssSimControl: GnssSimAdapter {
    fn profile(&self) -> &CommandProfile;
    /// Render and send an operation now.
    fn send_op(&mut self, op: &str, fields: &Fields) -> Result<Vec<String>, String>;
    /// Send pre-rendered lines.
    fn send_lines(&mut self, lines: &[String]) -> Result<Vec<String>, String>;

    /// Absolute power offset of a constellation/signal group (dB, `None` = all).
    fn set_power_offset(
        &mut self,
        c: Option<Constellation>,
        signal: Option<&str>,
        offset_db: f64,
    ) -> Result<(), String> {
        let mut f = self.profile().group_fields(c, signal);
        f.insert("offset_db".into(), format!("{offset_db:.2}"));
        self.send_op(ops::SET_POWER_OFFSET, &f).map(|_| ())
    }
    /// Absolute C/N0 offset (dB); uses the power-offset command when the profile has no
    /// C/N0 command (at a fixed noise floor ΔC/N0 = ΔP).
    fn set_cn0_offset(
        &mut self,
        c: Option<Constellation>,
        signal: Option<&str>,
        offset_db: f64,
    ) -> Result<(), String> {
        let op = if self.profile().supports(ops::SET_CN0_OFFSET) {
            ops::SET_CN0_OFFSET
        } else {
            ops::SET_POWER_OFFSET
        };
        let mut f = self.profile().group_fields(c, signal);
        f.insert("offset_db".into(), format!("{offset_db:.2}"));
        self.send_op(op, &f).map(|_| ())
    }
    /// Stream motion samples: `stream_begin`, one `stream_sample` per sample (calling
    /// `pace(t_s)` before each, e.g. to send in real time), `stream_end`.
    fn stream_motion(
        &mut self,
        samples: &[MotionSample],
        rate_hz: f64,
        pace: &mut dyn FnMut(f64),
    ) -> Result<usize, String> {
        let p = self.profile().clone();
        if !p.supports(ops::STREAM_SAMPLE) {
            return Err(format!("profile {} cannot stream motion", p.name));
        }
        let mut f = Fields::new();
        f.insert("rate_hz".into(), format!("{rate_hz}"));
        f.insert("n".into(), samples.len().to_string());
        if p.supports(ops::STREAM_BEGIN) {
            self.send_op(ops::STREAM_BEGIN, &f)?;
        }
        for s in samples {
            pace(s.t_s);
            let mut sf = f.clone();
            sf.extend(s.fields());
            self.send_lines(&p.render(ops::STREAM_SAMPLE, &sf)?)?;
        }
        if p.supports(ops::STREAM_END) {
            self.send_op(ops::STREAM_END, &f)?;
        }
        Ok(samples.len())
    }
    /// Execute a schedule: host-timed entries call `wait_until(t_s)` before sending,
    /// time-tagged ones are sent at once.
    fn execute_schedule(
        &mut self,
        s: &CommandSchedule,
        wait_until: &mut dyn FnMut(f64),
    ) -> Result<(), String> {
        for e in &s.entries {
            if s.mode == ScheduleMode::HostTimed {
                wait_until(e.t_s);
            }
            self.send_lines(&e.lines)
                .map_err(|err| format!("t = {} s, {}: {err}", e.t_s, e.what))?;
        }
        Ok(())
    }
}

/// [`GnssSimAdapter`] + [`GnssSimControl`] over a [`ProfileChannel`].
pub struct ProfileAdapter {
    pub channel: ProfileChannel,
}

impl ProfileAdapter {
    pub fn new(addr: &str, profile: CommandProfile) -> ProfileAdapter {
        ProfileAdapter {
            channel: ProfileChannel::new(addr, profile),
        }
    }
    pub fn with_encoder(
        addr: &str,
        profile: CommandProfile,
        encoder: Box<dyn CommandEncoder>,
    ) -> ProfileAdapter {
        ProfileAdapter {
            channel: ProfileChannel::with_encoder(addr, profile, encoder),
        }
    }
    pub fn transcript(&self) -> &[String] {
        &self.channel.transcript
    }
}

/// Trajectory file fields: `path`, `format`, `rate_hz`, `samples`.
pub fn trajectory_fields(t: &TrajectoryFile) -> Fields {
    let mut f = Fields::new();
    f.insert("path".into(), t.path.display().to_string());
    f.insert(
        "format".into(),
        match t.format {
            TrajFormat::Nmea => "nmea",
            TrajFormat::Csv => "csv",
        }
        .into(),
    );
    f.insert("rate_hz".into(), format!("{}", t.rate_hz));
    f.insert("samples".into(), t.samples.to_string());
    f
}

impl GnssSimAdapter for ProfileAdapter {
    fn connect(&mut self) -> Result<String, String> {
        if !self.channel.is_open() {
            self.channel.open()?;
        }
        let addr = self.channel.addr.clone();
        if !self.channel.profile.supports(ops::CONNECT) {
            return Ok(addr);
        }
        let r = self.channel.op(ops::CONNECT, &Fields::new())?;
        Ok(r.into_iter().next().unwrap_or(addr))
    }
    fn set_start_epoch(&mut self, epoch: &StartEpoch) -> Result<(), String> {
        let e = StartEpoch {
            utc: epoch.utc,
            scale: self.channel.profile.epoch_scale,
        };
        self.channel
            .op(ops::SET_START_TIME, &e.fields())
            .map(|_| ())
    }
    fn load_trajectory(&mut self, traj: &TrajectoryFile) -> Result<(), String> {
        self.channel
            .op(ops::LOAD_TRAJECTORY, &trajectory_fields(traj))
            .map(|_| ())
    }
    fn arm(&mut self, mode: ArmMode) -> Result<(), String> {
        let op = match mode {
            ArmMode::Immediate => ops::START_IMMEDIATE,
            ArmMode::NextPps => ops::ARM_PPS,
            ArmMode::ExternalTrigger => ops::ARM_TRIGGER,
        };
        self.channel.op(op, &Fields::new()).map(|_| ())
    }
    fn status(&mut self) -> Result<GnssSimStatus, String> {
        let lines = self.channel.profile.render(ops::STATUS, &Fields::new())?;
        let q = lines.first().ok_or_else(|| {
            format!(
                "profile {} has no status command",
                self.channel.profile.name
            )
        })?;
        let raw = self
            .channel
            .send_raw(q)?
            .ok_or("status command got no response (check the profile's response mode)")?;
        Ok(GnssSimStatus {
            state: self.channel.profile.state_of(&raw),
            raw,
        })
    }
    fn stop(&mut self) -> Result<(), String> {
        self.channel.op(ops::STOP, &Fields::new()).map(|_| ())
    }
}

impl GnssSimControl for ProfileAdapter {
    fn profile(&self) -> &CommandProfile {
        &self.channel.profile
    }
    fn send_op(&mut self, op: &str, fields: &Fields) -> Result<Vec<String>, String> {
        self.channel.op(op, fields)
    }
    fn send_lines(&mut self, lines: &[String]) -> Result<Vec<String>, String> {
        self.channel.send_lines(lines)
    }
}

/// Implements [`GnssSimAdapter`] and [`GnssSimControl`] for a newtype around
/// [`ProfileAdapter`] (field `.0`).
macro_rules! delegate_profile_adapter {
    ($t:ty) => {
        impl $crate::sync::GnssSimAdapter for $t {
            fn connect(&mut self) -> Result<String, String> {
                self.0.connect()
            }
            fn set_start_epoch(&mut self, e: &$crate::sync::StartEpoch) -> Result<(), String> {
                self.0.set_start_epoch(e)
            }
            fn load_trajectory(&mut self, t: &$crate::sync::TrajectoryFile) -> Result<(), String> {
                self.0.load_trajectory(t)
            }
            fn arm(&mut self, m: $crate::sync::ArmMode) -> Result<(), String> {
                self.0.arm(m)
            }
            fn status(&mut self) -> Result<$crate::sync::GnssSimStatus, String> {
                self.0.status()
            }
            fn stop(&mut self) -> Result<(), String> {
                self.0.stop()
            }
        }
        impl $crate::sync::profile::GnssSimControl for $t {
            fn profile(&self) -> &$crate::sync::profile::CommandProfile {
                self.0.profile()
            }
            fn send_op(
                &mut self,
                op: &str,
                f: &$crate::sync::profile::Fields,
            ) -> Result<Vec<String>, String> {
                self.0.send_op(op, f)
            }
            fn send_lines(&mut self, l: &[String]) -> Result<Vec<String>, String> {
                self.0.send_lines(l)
            }
        }
    };
}
pub(crate) use delegate_profile_adapter;

// ---------------------------------------------------------------------------------------
// Test support: an in-process mock simulator
// ---------------------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod mock {
    use super::*;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    pub type Log = Arc<Mutex<Vec<String>>>;

    /// Spawn a mock server speaking `framing`; `reply(cmd)` returns the response (or
    /// `None` for no response). Returns (address, received commands, join handle).
    pub fn serve<F>(framing: Framing, mut reply: F) -> (String, Log, std::thread::JoinHandle<()>)
    where
        F: FnMut(&str) -> Option<String> + Send + 'static,
    {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let log: Log = Arc::new(Mutex::new(Vec::new()));
        let lg = log.clone();
        let h = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            let mut r = BufReader::new(s);
            let enc: Box<dyn CommandEncoder> = match framing {
                Framing::Line => Box::new(LineEncoder {
                    terminator: "\n".into(),
                }),
                f => Box::new(LengthPrefixedEncoder {
                    big_endian: f == Framing::U32BePrefixed,
                    max_len: 1 << 20,
                }),
            };
            while let Ok(Some(cmd)) = enc.read_response(&mut r) {
                lg.lock().unwrap().push(cmd.clone());
                if let Some(rp) = reply(&cmd) {
                    let bytes = match framing {
                        Framing::Line => format!("{rp}\r\n").into_bytes(),
                        _ => enc.encode(&rp),
                    };
                    if w.write_all(&bytes).is_err() {
                        break;
                    }
                }
            }
        });
        (addr, log, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = r#"
name = "test"
vendor = "none"
response = "every_command"
ok_pattern = "OK"
error_patterns = ["ERR"]
[constellations]
galileo = "GAL"
[commands]
connect = ["HELLO"]
set_start_time = ["START_TIME ${date} ${time}"]
set_power_offset = ["PWR ${constellation} ${signal} ${offset_db}"]
stream_sample = ["POS ${t_s} ${x} ${y} ${z}"]
status = ["STATE?"]
stop = ["STOP"]
arm_pps = ["ARM"]
[[status_map]]
contains = "ARMED"
state = "armed"
"#;

    #[test]
    fn fill_handles_json_braces_and_escapes() {
        let mut f = Fields::new();
        f.insert("a".into(), "1".into());
        assert_eq!(
            fill_template(r#"{"x":${a},"p":"$$5"}"#, &f).unwrap(),
            r#"{"x":1,"p":"$5"}"#
        );
        assert!(fill_template("${b}", &f).is_err());
        assert!(fill_template("${a", &f).is_err());
        assert_eq!(fill_template("cost $ 3", &f).unwrap(), "cost $ 3");
        assert_eq!(hms(3725.456), "01:02:05.46");
    }

    #[test]
    fn profile_toml_round_trip_and_validation() {
        let p = CommandProfile::from_toml(P).unwrap();
        assert_eq!(CommandProfile::from_toml(&p.to_toml()).unwrap(), p);
        assert!(!p.confirmed);
        assert_eq!(p.constellation_name(Some(Constellation::Galileo)), "GAL");
        assert_eq!(p.constellation_name(Some(Constellation::Gps)), "GPS");
        assert_eq!(p.constellation_name(None), "ALL");
        let bad = P.replace("stop = [", "stopp = [");
        assert!(CommandProfile::from_toml(&bad).is_err());
    }

    #[test]
    fn adapter_over_both_framings() {
        for framing in [
            Framing::Line,
            Framing::U32BePrefixed,
            Framing::U32LePrefixed,
        ] {
            let mut state = "IDLE";
            let (addr, log, h) = mock::serve(framing, move |c| {
                Some(match c {
                    "HELLO" => "OK SIM 1.0".to_string(),
                    "ARM" => {
                        state = "ARMED";
                        "OK".into()
                    }
                    "STATE?" => state.to_string(),
                    c if c.starts_with("PWR GAL") => "ERR bad signal".into(),
                    _ => "OK".into(),
                })
            });
            let mut p = CommandProfile::from_toml(P).unwrap();
            p.framing = framing;
            let mut a = ProfileAdapter::new(&addr, p);
            assert_eq!(a.connect().unwrap(), "OK SIM 1.0");
            let e = StartEpoch::from_utc_iso("2026-11-18T00:00:00Z", EpochScale::Utc).unwrap();
            a.set_start_epoch(&e).unwrap();
            a.set_power_offset(Some(Constellation::Gps), Some("L1CA"), -3.0)
                .unwrap();
            let err = a
                .set_power_offset(Some(Constellation::Galileo), None, 1.0)
                .unwrap_err();
            assert!(err.contains("ERR"), "{err}");
            a.arm(ArmMode::NextPps).unwrap();
            assert_eq!(a.status().unwrap().state, GnssSimState::Armed);
            assert!(a
                .load_trajectory(&TrajectoryFile {
                    path: "/x".into(),
                    format: TrajFormat::Nmea,
                    rate_hz: 1.0,
                    samples: 1
                })
                .unwrap_err()
                .contains("no command"));
            drop(a);
            h.join().unwrap();
            let l = log.lock().unwrap().clone();
            assert_eq!(l[1], "START_TIME 2026-11-18 00:00:00");
            assert_eq!(l[2], "PWR GPS L1CA -3.00");
        }
    }
}
