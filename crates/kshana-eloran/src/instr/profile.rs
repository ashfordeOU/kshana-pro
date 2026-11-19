// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! User-editable instrument profiles (TOML).
//!
//! A profile names the instrument, its role, how to reach it and the command templates of
//! its role:
//!
//! ```toml
//! id = "tic1"
//! role = "time_interval_counter"
//! model = "counter model"
//! serial = "SN123"
//! cal_due = "2027-06-30"
//! init = ["*RST", "*CLS"]
//!
//! [transport]
//! kind = "tcp"            # or "serial"
//! host = "192.168.1.20"
//! port = 5025
//! timeout_ms = 5000
//! terminator = "\n"
//!
//! [tic]
//! arm_pps = [":TRIG:SOUR EXT", ":INIT"]
//! read_one = ":FETC?"
//! unit = "seconds"
//! ```
//!
//! Templates use `{name}` placeholders, listed with each command set. **The defaults are
//! generic IEEE 488.2 / SCPI-1999-style placeholders, not any vendor's command set**: map
//! every command to the instrument's programming manual. Export a starting profile with
//! [`InstrumentProfile::default_for`] and [`InstrumentProfile::to_toml`].

use super::{SerialTransport, SharedLog, TcpTransport, Transport};
use crate::hw::calib::TicUnit;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Instrument role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    TimeIntervalCounter,
    Oscilloscope,
    SpectrumAnalyser,
    PowerMeter,
    FrequencyCounter,
    /// TAPR TICC on a serial port (free-running; see [`super::ticc`]).
    Ticc,
}

/// How to reach the instrument.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransportConfig {
    Tcp {
        host: String,
        #[serde(default = "d_port")]
        port: u16,
        #[serde(default = "d_timeout")]
        timeout_ms: u64,
        #[serde(default = "d_lf")]
        terminator: String,
    },
    /// Device file; line settings must be set externally (see [`SerialTransport`]).
    Serial {
        path: PathBuf,
        #[serde(default = "d_crlf")]
        terminator: String,
    },
}

fn d_port() -> u16 {
    5025
}
fn d_timeout() -> u64 {
    5000
}
fn d_lf() -> String {
    "\n".into()
}
fn d_crlf() -> String {
    "\r\n".into()
}

impl TransportConfig {
    pub fn open(&self) -> Result<Box<dyn Transport>, String> {
        Ok(match self {
            TransportConfig::Tcp {
                host,
                port,
                timeout_ms,
                terminator,
            } => Box::new(
                TcpTransport::connect(host, *port, Duration::from_millis(*timeout_ms))?
                    .with_terminator(terminator),
            ),
            TransportConfig::Serial { path, terminator } => {
                Box::new(SerialTransport::open(path)?.with_terminator(terminator))
            }
        })
    }
}

/// Time-interval counter commands. Placeholders: `{n}` (number of readings).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TicCommands {
    /// Configure a start(ch1)→stop(ch2) time-interval measurement.
    pub setup: Vec<String>,
    /// Arm so the first measurement starts on the next PPS.
    pub arm_pps: Vec<String>,
    /// Query returning one reading.
    pub read_one: String,
    /// Optional query returning `{n}` readings as a comma-separated list (faster).
    #[serde(default)]
    pub read_block: Option<String>,
    /// Unit of the readings.
    #[serde(default)]
    pub unit: TicUnit,
    /// Constant subtracted from each reading (ns), e.g. the counter's channel offset.
    #[serde(default)]
    pub offset_ns: f64,
}

impl Default for TicCommands {
    fn default() -> Self {
        TicCommands {
            setup: vec![":CONF:TINT (@1),(@2)".into()],
            arm_pps: vec![":TRIG:SOUR EXT".into(), ":INIT".into()],
            read_one: ":READ?".into(),
            read_block: None,
            unit: TicUnit::Seconds,
            offset_ns: 0.0,
        }
    }
}

/// Oscilloscope commands. Placeholders: `{channel}` (1-based scope channel),
/// `{trigger_source}`, `{trigger_level_v}`, `{points}`, `{time_per_div_s}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScopeCommands {
    /// Trigger on the PPS (rising edge).
    pub trigger_pps: Vec<String>,
    /// Acquisition and transfer set-up of a channel (ASCII waveform format).
    pub setup_channel: Vec<String>,
    /// Single acquisition.
    pub arm: Vec<String>,
    /// Query that returns when the acquisition is complete.
    pub wait: String,
    /// Sample interval query (s).
    pub x_increment: String,
    /// Time of the first sample relative to the trigger query (s).
    pub x_origin: String,
    /// Waveform data query (ASCII, comma-separated, in volts).
    pub data: String,
    pub trigger_source: String,
    pub trigger_level_v: f64,
    pub points: usize,
    pub time_per_div_s: f64,
    /// Scope channel used for simulator channel `i` = `channel_map[i]` (1-based).
    #[serde(default)]
    pub channel_map: Vec<usize>,
}

impl Default for ScopeCommands {
    fn default() -> Self {
        ScopeCommands {
            trigger_pps: vec![
                ":TRIG:EDGE:SOUR {trigger_source}".into(),
                ":TRIG:EDGE:LEV {trigger_level_v}".into(),
                ":TRIG:EDGE:SLOP POS".into(),
            ],
            setup_channel: vec![
                ":TIM:SCAL {time_per_div_s}".into(),
                ":WAV:SOUR CHAN{channel}".into(),
                ":WAV:FORM ASC".into(),
                ":WAV:POIN {points}".into(),
            ],
            arm: vec![":SING".into()],
            wait: "*OPC?".into(),
            x_increment: ":WAV:XINC?".into(),
            x_origin: ":WAV:XOR?".into(),
            data: ":WAV:DATA?".into(),
            trigger_source: "EXT".into(),
            trigger_level_v: 1.0,
            points: 100_000,
            time_per_div_s: 50e-6,
            channel_map: vec![],
        }
    }
}

/// Spectrum-analyser commands. Placeholders: `{start_hz}`, `{stop_hz}`, `{rbw_hz}`,
/// `{points}`, `{input}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaCommands {
    pub set_sweep: Vec<String>,
    /// Input selection (RF switch); empty = single input.
    #[serde(default)]
    pub select_input: Vec<String>,
    pub sweep: Vec<String>,
    pub wait: String,
    /// Trace query (ASCII dBm list, evenly spaced from start to stop).
    pub trace: String,
    pub marker_peak: Vec<String>,
    pub marker_x: String,
    pub marker_y: String,
    pub points: usize,
    /// ENBW/RBW of the analyser's resolution filter (see [`super::Trace::enbw_factor`]).
    pub enbw_factor: f64,
}

impl Default for SaCommands {
    fn default() -> Self {
        SaCommands {
            set_sweep: vec![
                ":FREQ:STAR {start_hz}".into(),
                ":FREQ:STOP {stop_hz}".into(),
                ":BAND {rbw_hz}".into(),
                ":SWE:POIN {points}".into(),
                ":FORM ASC".into(),
            ],
            select_input: vec![],
            sweep: vec![":INIT:IMM".into()],
            wait: "*OPC?".into(),
            trace: ":TRAC? TRACE1".into(),
            marker_peak: vec![":CALC:MARK1:MAX".into()],
            marker_x: ":CALC:MARK1:X?".into(),
            marker_y: ":CALC:MARK1:Y?".into(),
            points: 1001,
            enbw_factor: 1.0,
        }
    }
}

/// Power-meter commands. Placeholder: `{freq_hz}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PowerMeterCommands {
    pub set_freq: Vec<String>,
    pub read: String,
    /// Response unit: "dbm" or "w".
    pub unit: String,
}

impl Default for PowerMeterCommands {
    fn default() -> Self {
        PowerMeterCommands {
            set_freq: vec![":SENS:FREQ {freq_hz}".into()],
            read: ":READ?".into(),
            unit: "dbm".into(),
        }
    }
}

/// Frequency-counter commands. Placeholder: `{gate_s}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FreqCounterCommands {
    pub setup: Vec<String>,
    pub read: String,
}

impl Default for FreqCounterCommands {
    fn default() -> Self {
        FreqCounterCommands {
            setup: vec![":CONF:FREQ (@1)".into(), ":FREQ:GATE:TIME {gate_s}".into()],
            read: ":READ?".into(),
        }
    }
}

/// TAPR TICC reader settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TiccSettings {
    /// Start (reference PPS) and stop channels: 'A' and 'B' by default.
    pub start_channel: char,
    pub stop_channel: char,
    /// Constant subtracted from each interval (ns): the TICC's A/B channel offset.
    #[serde(default)]
    pub offset_ns: f64,
    /// Give up after this many lines without completing the requested readings.
    pub max_lines: usize,
}

impl Default for TiccSettings {
    fn default() -> Self {
        TiccSettings {
            start_channel: 'A',
            stop_channel: 'B',
            offset_ns: 0.0,
            max_lines: 100_000,
        }
    }
}

/// An instrument profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstrumentProfile {
    pub id: String,
    pub role: Role,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub serial: String,
    #[serde(default)]
    pub cal_due: Option<String>,
    /// Commands sent after connecting.
    #[serde(default)]
    pub init: Vec<String>,
    pub transport: TransportConfig,
    #[serde(default)]
    pub tic: Option<TicCommands>,
    #[serde(default)]
    pub scope: Option<ScopeCommands>,
    #[serde(default)]
    pub sa: Option<SaCommands>,
    #[serde(default)]
    pub power_meter: Option<PowerMeterCommands>,
    #[serde(default)]
    pub freq_counter: Option<FreqCounterCommands>,
    #[serde(default)]
    pub ticc: Option<TiccSettings>,
}

impl InstrumentProfile {
    pub fn from_toml(s: &str) -> Result<InstrumentProfile, String> {
        toml::from_str(s).map_err(|e| format!("instrument profile: {e}"))
    }
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("serialisable")
    }

    /// A starting profile for `role` with the placeholder commands filled in.
    pub fn default_for(id: &str, role: Role, transport: TransportConfig) -> InstrumentProfile {
        let mut p = InstrumentProfile {
            id: id.into(),
            role,
            model: String::new(),
            serial: String::new(),
            cal_due: None,
            init: if role == Role::Ticc {
                vec![]
            } else {
                vec!["*CLS".into()]
            },
            transport,
            tic: None,
            scope: None,
            sa: None,
            power_meter: None,
            freq_counter: None,
            ticc: None,
        };
        match role {
            Role::TimeIntervalCounter => p.tic = Some(TicCommands::default()),
            Role::Oscilloscope => p.scope = Some(ScopeCommands::default()),
            Role::SpectrumAnalyser => p.sa = Some(SaCommands::default()),
            Role::PowerMeter => p.power_meter = Some(PowerMeterCommands::default()),
            Role::FrequencyCounter => p.freq_counter = Some(FreqCounterCommands::default()),
            Role::Ticc => p.ticc = Some(TiccSettings::default()),
        }
        p
    }

    /// A built-in profile with the vendor's own commands for catalogue entry `catalog_id` in
    /// `role`, when one exists. Commands are quoted from the programming manuals named in
    /// `model`; trigger levels default to 1.0 V (half of a TTL PPS into 50 Ω): change them to
    /// half your PPS amplitude.
    pub fn builtin(
        catalog_id: &str,
        role: Role,
        transport: TransportConfig,
    ) -> Option<InstrumentProfile> {
        let mut p = InstrumentProfile::default_for(catalog_id, role, transport);
        let v = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match (catalog_id, role) {
            // Keysight 53220A/53230A User's Guide 53220-90001 Ed. 3 (2017): CONF:TINT, INP:LEV,
            // INP:SLOP, INP:IMP, ROSC:SOUR, SAMP:COUN, READ?, CONF:FREQ, FREQ:GATE:TIME, SYST:ERR?.
            ("keysight-53230a", Role::TimeIntervalCounter) => {
                p.model = "Keysight 53230A (commands: User's Guide 53220-90001 Ed. 3)".into();
                p.init = v(&["*RST", "*CLS"]);
                p.tic = Some(TicCommands {
                    setup: v(&[
                        "CONF:TINT (@1),(@2)",
                        "INP1:IMP 50",
                        "INP2:IMP 50",
                        "INP1:LEV 1.0",
                        "INP2:LEV 1.0",
                        "INP1:SLOP POS",
                        "INP2:SLOP POS",
                        "ROSC:SOUR EXT",
                    ]),
                    arm_pps: v(&["TRIG:SOUR IMM"]),
                    read_one: "READ?".into(),
                    read_block: Some("SAMP:COUN {n};:READ?".into()),
                    unit: TicUnit::Seconds,
                    offset_ns: 0.0,
                });
            }
            ("keysight-53230a", Role::FrequencyCounter) => {
                p.model = "Keysight 53230A (commands: User's Guide 53220-90001 Ed. 3)".into();
                p.init = v(&["*RST", "*CLS"]);
                p.freq_counter = Some(FreqCounterCommands {
                    setup: v(&["CONF:FREQ (@1)", "FREQ:GATE:TIME {gate_s}", "ROSC:SOUR EXT"]),
                    read: "READ?".into(),
                });
            }
            // Pendulum CNT-90/91 Programmer's Handbook, 10th ed. (May 2017): :CONF:TINT, :INP:LEV,
            // :INP:SLOP, :INP:IMP, :ROSC:SOUR, :INIT, :FETC?, :SYST:ERR?. GPIB/USB only: reach it
            // through a GPIB- or USB-to-LAN bridge or a serial adapter.
            ("pendulum-cnt-91", Role::TimeIntervalCounter) => {
                p.model = "Pendulum CNT-91 (commands: Programmer's Handbook 10th ed.)".into();
                p.init = v(&["*RST", "*CLS"]);
                p.tic = Some(TicCommands {
                    setup: v(&[
                        ":CONF:TINT (@1),(@2)",
                        ":INP1:IMP 50",
                        ":INP2:IMP 50",
                        ":INP1:LEV 1.0",
                        ":INP2:LEV 1.0",
                        ":INP1:SLOP POS",
                        ":INP2:SLOP POS",
                        ":ROSC:SOUR EXT",
                    ]),
                    arm_pps: v(&[]),
                    read_one: ":INIT;:FETC?".into(),
                    read_block: None,
                    unit: TicUnit::Seconds,
                    offset_ns: 0.0,
                });
            }
            _ => return None,
        }
        Some(p)
    }

    /// Open the transport, start a session and send the `init` commands.
    pub fn connect(&self, log: Option<SharedLog>) -> Result<super::ScpiSession, String> {
        let mut s = super::ScpiSession::new(&self.id, self.transport.open()?, log);
        s.write_all(&self.init)?;
        Ok(s)
    }

    /// The calibration record entry of this instrument.
    pub fn as_calibration_instrument(&self) -> crate::hw::calib::Instrument {
        crate::hw::calib::Instrument {
            role: format!("{:?}", self.role),
            model: self.model.clone(),
            serial: self.serial.clone(),
            cal_due: self.cal_due.clone(),
        }
    }
}

/// Fill `{name}` placeholders of a template; an unknown placeholder is an error (so a
/// typo in a profile is caught before anything is sent).
pub fn fill(template: &str, vars: &[(&str, String)]) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(a) = rest.find('{') {
        out.push_str(&rest[..a]);
        let after = &rest[a + 1..];
        let b = after
            .find('}')
            .ok_or_else(|| format!("unclosed placeholder in {template:?}"))?;
        let name = &after[..b];
        let v = vars
            .iter()
            .find(|(k, _)| *k == name)
            .ok_or_else(|| format!("unknown placeholder {{{name}}} in {template:?}"))?;
        out.push_str(&v.1);
        rest = &after[b + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Fill every template of a list.
pub fn fill_all(templates: &[String], vars: &[(&str, String)]) -> Result<Vec<String>, String> {
    templates.iter().map(|t| fill(t, vars)).collect()
}

#[cfg(test)]
mod tests {

    #[test]
    fn builtin_keysight_tic_profile_drives_a_counter() {
        use crate::instr::mock::MockTcpInstrument;
        use crate::instr::roles::{FrequencyCounter, TimeIntervalCounter};
        use crate::instr::{FrequencySource, TicSource};
        let m = MockTcpInstrument::scripted(&[
            ("*IDN?", "Keysight Technologies,53230A,MY123,1.0"),
            ("SAMP:COUN 3;:READ?", "+1.00E-07,+1.10E-07,+9.00E-08"),
            ("READ?", "+1.0000002E+07"),
            ("SYST:ERR?", "+0,\"No error\""),
        ])
        .unwrap();
        let tr = TransportConfig::Tcp {
            host: "127.0.0.1".into(),
            port: m.port(),
            timeout_ms: 2000,
            terminator: "\n".into(),
        };
        let p =
            InstrumentProfile::builtin("keysight-53230a", Role::TimeIntervalCounter, tr.clone())
                .unwrap();
        assert!(p.model.contains("53220-90001"));
        let mut t = TimeIntervalCounter::open(&p, None).unwrap();
        t.arm_pps().unwrap();
        let r = t.read_n(3).unwrap();
        assert_eq!(r.len(), 3);
        assert!(
            (r[0] - 100.0).abs() < 1e-9 && (r[2] - 90.0).abs() < 1e-9,
            "{r:?}"
        );
        let f = InstrumentProfile::builtin("keysight-53230a", Role::FrequencyCounter, tr).unwrap();
        let mut c = FrequencyCounter::open(&f, None).unwrap();
        assert!((c.read_hz(10.0).unwrap() - 10_000_002.0).abs() < 1e-3);
        let got = m.received();
        for want in [
            "CONF:TINT (@1),(@2)",
            "INP1:IMP 50",
            "ROSC:SOUR EXT",
            "CONF:FREQ (@1)",
            "FREQ:GATE:TIME 10",
        ] {
            assert!(got.iter().any(|c| c == want), "missing {want}: {got:?}");
        }
        assert!(InstrumentProfile::builtin(
            "unknown-model",
            Role::TimeIntervalCounter,
            TransportConfig::Serial {
                path: "/dev/null".into(),
                terminator: "\n".into()
            }
        )
        .is_none());
    }

    use super::*;

    #[test]
    fn placeholders_fill_and_reject_unknown() {
        let v = [("n", "10".to_string()), ("channel", "2".to_string())];
        assert_eq!(fill(":SAMP {n};CH{channel}", &v).unwrap(), ":SAMP 10;CH2");
        assert!(fill(":X {nn}", &v)
            .unwrap_err()
            .contains("unknown placeholder"));
        assert!(fill(":X {n", &v).is_err());
    }

    #[test]
    fn every_default_profile_round_trips_through_toml() {
        for role in [
            Role::TimeIntervalCounter,
            Role::Oscilloscope,
            Role::SpectrumAnalyser,
            Role::PowerMeter,
            Role::FrequencyCounter,
            Role::Ticc,
        ] {
            let t = if role == Role::Ticc {
                TransportConfig::Serial {
                    path: "/dev/ttyACM0".into(),
                    terminator: "\r\n".into(),
                }
            } else {
                TransportConfig::Tcp {
                    host: "127.0.0.1".into(),
                    port: 5025,
                    timeout_ms: 1000,
                    terminator: "\n".into(),
                }
            };
            let p = InstrumentProfile::default_for("x", role, t);
            let s = p.to_toml();
            assert_eq!(InstrumentProfile::from_toml(&s).unwrap(), p, "{s}");
        }
        let src = r#"
id = "pm"
role = "power_meter"
[transport]
kind = "tcp"
host = "10.0.0.5"
[power_meter]
set_freq = []
read = "FETCH?"
unit = "w"
"#;
        let p = InstrumentProfile::from_toml(src).unwrap();
        assert!(matches!(
            p.transport,
            TransportConfig::Tcp { port: 5025, .. }
        ));
        assert_eq!(p.power_meter.unwrap().unit, "w");
    }
}
