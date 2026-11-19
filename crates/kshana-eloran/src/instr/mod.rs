// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Bench instrument automation (stream B, A7).
//!
//! # Layers
//!
//! * [`transport`]: a line-oriented [`Transport`] trait with two implementations —
//!   [`TcpTransport`] (a raw SCPI socket, the "port 5025" convention of LXI instruments,
//!   `std::net` only, no VXI-11/RPC layer) and [`SerialTransport`] (reads and writes a
//!   device file such as `/dev/ttyUSB0`; **line settings — baud rate, parity, stop bits, raw
//!   mode — must be set outside this crate**, e.g. `stty -F /dev/ttyUSB0 115200 raw -echo`,
//!   because the crate takes no termios dependency).
//! * [`ScpiSession`]: one instrument connection with an identifier; every write, query and
//!   unsolicited read is recorded to a [`MeasurementLog`] (JSON lines: instrument id,
//!   command, response, UTC timestamp) kept as evidence.
//! * [`profile`]: user-editable command profiles (TOML) per **instrument role**. The default
//!   commands are generic IEEE 488.2 / SCPI-1999 style **placeholders**, not any vendor's
//!   command set: map them to the instrument's programming manual before use.
//! * [`roles`]: the role drivers — [`roles::TimeIntervalCounter`], [`roles::Oscilloscope`],
//!   [`roles::SpectrumAnalyser`], [`roles::PowerMeter`], [`roles::FrequencyCounter`] — each
//!   implementing one of the role traits below, which is all calibration and acceptance
//!   code depends on (so mocks and other drivers plug in).
//! * [`ticc`]: a dedicated parser and reader for the open TAPR TICC time-stamping counter's
//!   serial output.
//! * [`mock`]: an in-process mock TCP instrument for tests and dry runs.
//!
//! # Evidence label
//!
//! Protocol handling is **MODELLED**: verified against the in-process mock instrument only.
//! The drivers are **UNTESTED ON HARDWARE** until run against real instruments with
//! profiles mapped to their manuals.

pub mod log;
pub mod mock;
pub mod profile;
pub mod roles;
pub mod ticc;
pub mod transport;

pub use log::{LogEntry, MeasurementLog, SharedLog};
pub use transport::{SerialTransport, TcpTransport, Transport};

use serde::{Deserialize, Serialize};

/// One instrument connection: an id, a transport and an optional evidence log.
pub struct ScpiSession {
    pub id: String,
    transport: Box<dyn Transport>,
    log: Option<SharedLog>,
}

impl ScpiSession {
    pub fn new(id: &str, transport: Box<dyn Transport>, log: Option<SharedLog>) -> ScpiSession {
        ScpiSession {
            id: id.to_string(),
            transport,
            log,
        }
    }

    fn record(&self, kind: &str, command: &str, response: Option<&str>, error: Option<&str>) {
        if let Some(l) = &self.log {
            if let Ok(mut l) = l.lock() {
                // a failing evidence write must not hide the measurement error itself
                let _ = l.record(&self.id, kind, command, response, error);
            }
        }
    }

    /// Send one command line (no response expected).
    pub fn write(&mut self, cmd: &str) -> Result<(), String> {
        let r = self.transport.send(cmd);
        self.record("write", cmd, None, r.as_ref().err().map(|e| e.as_str()));
        r.map_err(|e| format!("{}: {e}", self.id))
    }

    /// Send every command of a list, in order.
    pub fn write_all(&mut self, cmds: &[String]) -> Result<(), String> {
        for c in cmds {
            self.write(c)?;
        }
        Ok(())
    }

    /// Send a query and read one response line.
    pub fn query(&mut self, cmd: &str) -> Result<String, String> {
        let r = self
            .transport
            .send(cmd)
            .and_then(|_| self.transport.recv_line());
        match &r {
            Ok(resp) => self.record("query", cmd, Some(resp), None),
            Err(e) => self.record("query", cmd, None, Some(e)),
        }
        r.map_err(|e| format!("{}: {e}", self.id))
    }

    /// Read one unsolicited line (free-running instruments such as the TICC).
    pub fn read_line(&mut self) -> Result<String, String> {
        let r = self.transport.recv_line();
        match &r {
            Ok(resp) => self.record("read", "", Some(resp), None),
            Err(e) => self.record("read", "", None, Some(e)),
        }
        r.map_err(|e| format!("{}: {e}", self.id))
    }

    /// Record an operator/procedure note in the evidence log.
    pub fn note(&self, text: &str) {
        self.record("note", text, None, None);
    }

    /// `*IDN?` (IEEE 488.2 common query).
    pub fn identify(&mut self) -> Result<String, String> {
        self.query("*IDN?")
    }

    pub fn transport_description(&self) -> String {
        self.transport.describe()
    }
}

/// A captured waveform (oscilloscope or SDR capture) of one channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Waveform {
    /// Sample rate (Hz).
    pub fs_hz: f64,
    /// Time of the first sample relative to the trigger (the PPS edge), s. Negative for a
    /// pre-trigger record (the usual SCPI `X origin`).
    pub t0_s: f64,
    pub samples: Vec<f64>,
    /// Sample unit, e.g. "V" or "FS" (device full scale).
    pub unit: String,
}

impl Waveform {
    /// PPS instants inside the record (s from its first sample) for a pulse period
    /// `period_s` (the trigger PPS and the following ones within the record).
    pub fn pps_times_s(&self, period_s: f64) -> Vec<f64> {
        let len_s = self.samples.len() as f64 / self.fs_hz;
        let mut v = Vec::new();
        let mut t = -self.t0_s;
        while t < len_s {
            if t >= 0.0 {
                v.push(t);
            }
            t += period_s;
        }
        v
    }
}

/// A spectrum-analyser trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub freqs_hz: Vec<f64>,
    pub dbm: Vec<f64>,
    /// Resolution bandwidth (Hz).
    pub rbw_hz: f64,
    /// Equivalent-noise-bandwidth to RBW ratio of the analyser's filter (≈ 1.06 for a
    /// 4-pole Gaussian-like filter, 1.0 if the analyser reports noise-bandwidth-corrected
    /// data). Taken from the profile.
    pub enbw_factor: f64,
}

impl Trace {
    /// Evenly spaced trace from `start_hz` to `stop_hz`.
    pub fn uniform(start_hz: f64, stop_hz: f64, dbm: Vec<f64>, rbw_hz: f64, enbw: f64) -> Trace {
        let n = dbm.len();
        let step = if n > 1 {
            (stop_hz - start_hz) / (n - 1) as f64
        } else {
            0.0
        };
        Trace {
            freqs_hz: (0..n).map(|i| start_hz + i as f64 * step).collect(),
            dbm,
            rbw_hz,
            enbw_factor: enbw,
        }
    }

    /// Integrated power in `[f_lo, f_hi]` (dBm): the bin powers summed and scaled by
    /// bin spacing / noise bandwidth — the usual channel-power integration of swept
    /// analysers (e.g. Keysight Application Note 150, "Spectrum Analysis Basics", channel
    /// power). Valid for bin spacing ≤ RBW. `None` if no bin falls in the band.
    pub fn band_power_dbm(&self, f_lo: f64, f_hi: f64) -> Option<f64> {
        if self.freqs_hz.len() < 2 {
            return None;
        }
        let spacing = (self.freqs_hz[self.freqs_hz.len() - 1] - self.freqs_hz[0])
            / (self.freqs_hz.len() - 1) as f64;
        let mut mw = 0.0;
        let mut any = false;
        for (f, p) in self.freqs_hz.iter().zip(&self.dbm) {
            if *f >= f_lo && *f <= f_hi {
                mw += 10f64.powf(p / 10.0);
                any = true;
            }
        }
        if !any {
            return None;
        }
        Some(10.0 * (mw * spacing / (self.rbw_hz * self.enbw_factor)).log10())
    }

    /// Highest bin in `[f_lo, f_hi]`: (frequency Hz, dBm).
    pub fn peak_in(&self, f_lo: f64, f_hi: f64) -> Option<(f64, f64)> {
        self.freqs_hz
            .iter()
            .zip(&self.dbm)
            .filter(|(f, _)| **f >= f_lo && **f <= f_hi)
            .map(|(f, p)| (*f, *p))
            .fold(None, |a, b| match a {
                Some((_, pa)) if pa >= b.1 => a,
                _ => Some(b),
            })
    }
}

/// Time-interval counter role: start = reference (1PPS), stop = device under test.
pub trait TicSource {
    fn id(&self) -> String;
    /// Arm the counter so that measurements start on the next PPS.
    fn arm_pps(&mut self) -> Result<(), String>;
    /// Read `n` time-interval readings, in **nanoseconds**.
    fn read_n(&mut self, n: usize) -> Result<Vec<f64>, String>;
}

/// Oscilloscope (or any triggered capture) role.
pub trait WaveformSource {
    fn id(&self) -> String;
    /// Arm a single acquisition of `channel` triggered on the PPS.
    fn arm(&mut self, channel: usize) -> Result<(), String>;
    /// Wait for the acquisition and transfer the waveform.
    fn fetch(&mut self, channel: usize) -> Result<Waveform, String>;
}

/// Spectrum-analyser role.
pub trait SpectrumSource {
    fn id(&self) -> String;
    /// Select an input (RF switch or analyser input); no-op where not configured.
    fn select_input(&mut self, input: usize) -> Result<(), String>;
    /// One sweep from `start_hz` to `stop_hz` with resolution bandwidth `rbw_hz`.
    fn sweep(&mut self, start_hz: f64, stop_hz: f64, rbw_hz: f64) -> Result<Trace, String>;
    /// Peak marker of the last sweep: (Hz, dBm).
    fn marker_peak(&mut self) -> Result<(f64, f64), String>;
}

/// Power-meter role.
pub trait PowerSource {
    fn id(&self) -> String;
    /// Average power (dBm) with the sensor's frequency correction set to `freq_hz`.
    fn read_dbm(&mut self, freq_hz: f64) -> Result<f64, String>;
}

/// Frequency-counter role.
pub trait FrequencySource {
    fn id(&self) -> String;
    /// One frequency reading (Hz) with gate time `gate_s`.
    fn read_hz(&mut self, gate_s: f64) -> Result<f64, String>;
}

/// Parse the first number of an instrument response (`"+1.234E-08"`, `"1.2e-8 s"`,
/// `"-12.5 dBm"`, `"1.0,OK"`).
pub fn parse_number(resp: &str) -> Result<f64, String> {
    let tok = resp
        .trim()
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .find(|t| !t.is_empty())
        .ok_or_else(|| format!("empty response {resp:?}"))?;
    let t = tok.trim_start_matches('+');
    t.parse::<f64>()
        .map_err(|_| format!("not a number: {resp:?}"))
}

/// Parse a comma- (or whitespace-) separated list of numbers, skipping an IEEE 488.2
/// definite-length block header (`#<d><len>`) if the ASCII data is wrapped in one.
pub fn parse_number_list(resp: &str) -> Result<Vec<f64>, String> {
    let mut s = resp.trim();
    if let Some(rest) = s.strip_prefix('#') {
        let d = rest
            .chars()
            .next()
            .and_then(|c| c.to_digit(10))
            .ok_or("bad block header")? as usize;
        s = rest.get(1 + d..).ok_or("short block header")?;
    }
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| {
            t.trim_start_matches('+')
                .parse::<f64>()
                .map_err(|_| format!("not a number: {t:?}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_parsing() {
        assert_eq!(parse_number("+1.234E-08").unwrap(), 1.234e-8);
        assert_eq!(parse_number(" -12.5 dBm").unwrap(), -12.5);
        assert_eq!(parse_number("3,OK").unwrap(), 3.0);
        assert!(parse_number("abc").is_err());
        assert_eq!(parse_number_list("1, 2,+3e0").unwrap(), vec![1.0, 2.0, 3.0]);
        assert_eq!(parse_number_list("#15 1,2").unwrap(), vec![1.0, 2.0]);
    }

    #[test]
    fn band_power_of_a_tone_and_of_flat_noise() {
        // flat noise density -100 dBm/Hz shown with RBW 100 Hz → -80 dBm per bin, 50 Hz bins
        let n = 401;
        let t = Trace::uniform(80e3, 100e3, vec![-80.0; n], 100.0, 1.0);
        let p = t.band_power_dbm(90e3, 100e3).unwrap();
        // 10 kHz of -100 dBm/Hz = -60 dBm (bin-count edge effect: 201 bins × 50 Hz)
        let want = -100.0 + 10.0 * (201.0f64 * 50.0).log10();
        assert!((p - want).abs() < 1e-9, "{p} {want}");
        let pk = t.peak_in(85e3, 86e3).unwrap();
        assert!(pk.0 >= 85e3 && pk.1 == -80.0);
        assert!(t.band_power_dbm(200e3, 300e3).is_none());
    }

    #[test]
    fn waveform_pps_times() {
        let w = Waveform {
            fs_hz: 1000.0,
            t0_s: -0.1,
            samples: vec![0.0; 2500],
            unit: "V".into(),
        };
        let p = w.pps_times_s(1.0);
        assert_eq!(p.len(), 3);
        assert!((p[0] - 0.1).abs() < 1e-12 && (p[2] - 2.1).abs() < 1e-12);
    }
}
