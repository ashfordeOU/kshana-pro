// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Generic LXI/SCPI arbitrary waveform generator over a raw TCP socket (pure `std::net`).
//!
//! An AWG plays a waveform from its own memory; it does not stream. The driver therefore
//! works with **segments** compiled by [`super::awg::compile`]:
//!
//! * [`ScpiAwg::play_segment`] ([`SegmentPlayer`]) uploads one segment per channel, sets the
//!   sample rate, amplitude, offset, 10 MHz reference, loop mode and trigger, and switches
//!   the output on. `StartTime::NextPps` selects the external trigger (wire the reference
//!   1PPS to the trigger input); `Now` starts immediately.
//! * As a [`Device`], `write_tx` collects samples into memory (up to the profile's
//!   `max_points`) and **`stop_tx` uploads and starts playback** (looped when
//!   `loop_on_stop`, the default — write exactly one period, e.g. a compiled segment).
//!   Playback continues after `stop_tx` until [`ScpiAwg::stop_playback`].
//!
//! # Profiles
//!
//! Instruments differ in their command sets, so every command comes from an editable TOML
//! [`ScpiProfile`] with placeholders (`{ch}`, `{name}`, `{len}`, `{block}`, `{data}`,
//! `{rate}`, `{vpp}`, `{offset}`). The built-in profiles (`generic-binary`,
//! `generic-ascii`, in `src/hw/awg_profiles/`) use **neutral placeholder commands in the
//! general style of IEEE 488.2/SCPI; they are not the command set of any vendor** and must
//! be edited from the instrument's programming manual before use. Binary uploads use the
//! IEEE 488.2 definite-length arbitrary block (`#<n><len><bytes>`, IEEE 488.2-1992 §7.7.6).
//!
//! **Output policy: conducted use only** — the output is for cables or antenna simulators,
//! never a radiating antenna; printed on connect.
//!
//! Evidence: protocol framing tested against a mock TCP server
//! (`plays_segment_through_mock_instrument`); **UNTESTED ON HARDWARE** — needs a real
//! instrument and a profile written for it.

use super::awg::{AwgSegment, SegmentPlayer};
use super::{announce_output_policy, Device, DeviceConfig, DeviceStatus, RefSource, StartTime};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

/// How waveform points are encoded in an upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataFormat {
    /// IEEE 488.2 definite-length block of signed 16-bit little-endian codes.
    Int16LeBlock,
    /// IEEE 488.2 definite-length block of signed 16-bit big-endian codes.
    Int16BeBlock,
    /// Comma-separated normalised values in [−1, 1].
    FloatCsv,
    /// Comma-separated signed integer codes.
    IntCsv,
}

/// An instrument command profile (see the module documentation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScpiProfile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "d_port")]
    pub port: u16,
    #[serde(default = "d_term")]
    pub terminator: String,
    pub max_points: usize,
    #[serde(default = "d_min")]
    pub min_points: usize,
    /// Point count must be a multiple of this.
    #[serde(default = "d_one")]
    pub granularity: usize,
    pub dac_bits: u32,
    pub data_format: DataFormat,
    #[serde(default)]
    pub idn: Option<String>,
    #[serde(default)]
    pub error_query: Option<String>,
    /// A response to `error_query` starting with this (after an optional '+') is "no error".
    #[serde(default = "d_noerr")]
    pub no_error: String,
    #[serde(default)]
    pub opc: Option<String>,
    #[serde(default)]
    pub init: Vec<String>,
    pub upload: Vec<String>,
    #[serde(default)]
    pub select: Vec<String>,
    #[serde(default)]
    pub sample_rate: Vec<String>,
    #[serde(default)]
    pub amplitude: Vec<String>,
    #[serde(default)]
    pub offset: Vec<String>,
    #[serde(default)]
    pub loop_continuous: Vec<String>,
    #[serde(default)]
    pub loop_once: Vec<String>,
    #[serde(default)]
    pub ref_external: Vec<String>,
    #[serde(default)]
    pub ref_internal: Vec<String>,
    #[serde(default)]
    pub trigger_external: Vec<String>,
    #[serde(default)]
    pub trigger_immediate: Vec<String>,
    #[serde(default)]
    pub software_trigger: Vec<String>,
    #[serde(default)]
    pub output_on: Vec<String>,
    #[serde(default)]
    pub output_off: Vec<String>,
}

fn d_port() -> u16 {
    5025
}
fn d_term() -> String {
    "\n".into()
}
fn d_min() -> usize {
    1
}
fn d_one() -> usize {
    1
}
fn d_noerr() -> String {
    "0".into()
}

/// Built-in profile sources (TOML).
pub const BUILTIN_PROFILES: &[(&str, &str)] = &[
    (
        "generic-binary",
        include_str!("awg_profiles/generic_binary.toml"),
    ),
    (
        "generic-ascii",
        include_str!("awg_profiles/generic_ascii.toml"),
    ),
    // Vendor profiles: commands quoted from the vendors' programming manuals (cited in each file).
    (
        "keysight-33500b",
        include_str!("awg_profiles/keysight_33500b.toml"),
    ),
    (
        "keysight-33600a",
        include_str!("awg_profiles/keysight_33600a.toml"),
    ),
    (
        "rigol-dg1000z",
        include_str!("awg_profiles/rigol_dg1000z.toml"),
    ),
];

impl ScpiProfile {
    pub fn from_toml(s: &str) -> Result<ScpiProfile, String> {
        let p: ScpiProfile = toml::from_str(s).map_err(|e| format!("AWG profile: {e}"))?;
        p.validate()?;
        Ok(p)
    }
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("serialisable")
    }
    pub fn load(path: &Path) -> Result<ScpiProfile, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        ScpiProfile::from_toml(&s)
    }
    /// A built-in profile by name (see [`BUILTIN_PROFILES`]).
    pub fn builtin(name: &str) -> Result<ScpiProfile, String> {
        let src = BUILTIN_PROFILES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, s)| *s)
            .ok_or_else(|| format!("no built-in AWG profile {name:?}"))?;
        ScpiProfile::from_toml(src)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=16).contains(&self.dac_bits) {
            return Err("dac_bits must be 2..=16".into());
        }
        if self.max_points == 0 || self.min_points > self.max_points || self.granularity == 0 {
            return Err("inconsistent point limits".into());
        }
        if self.upload.is_empty() {
            return Err("profile needs an upload command".into());
        }
        let needs = matches!(
            self.data_format,
            DataFormat::Int16LeBlock | DataFormat::Int16BeBlock
        );
        let tag = if needs { "{block}" } else { "{data}" };
        if !self.upload.iter().any(|c| c.contains(tag)) {
            return Err(format!(
                "upload command must contain {tag} for {:?}",
                self.data_format
            ));
        }
        Ok(())
    }
}

/// IEEE 488.2 definite-length arbitrary block header for `len` bytes.
pub fn block_header(len: usize) -> String {
    let l = len.to_string();
    format!("#{}{}", l.len(), l)
}

/// A line-oriented SCPI connection over TCP (also used by the Red Pitaya driver).
pub struct ScpiConn {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    term: String,
}

impl ScpiConn {
    pub fn connect(addr: &str, timeout: Duration, term: &str) -> Result<ScpiConn, String> {
        let sa = addr
            .to_socket_addrs()
            .map_err(|e| format!("{addr}: {e}"))?
            .next()
            .ok_or_else(|| format!("{addr}: no address"))?;
        let stream =
            TcpStream::connect_timeout(&sa, timeout).map_err(|e| format!("{addr}: {e}"))?;
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| e.to_string())?;
        let _ = stream.set_nodelay(true);
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        Ok(ScpiConn {
            stream,
            reader,
            term: term.into(),
        })
    }
    /// Send one command (terminator appended).
    pub fn send(&mut self, cmd: &str) -> Result<(), String> {
        self.send_bytes(cmd.as_bytes())
    }
    /// Send raw bytes followed by the terminator (for commands carrying binary blocks).
    pub fn send_bytes(&mut self, b: &[u8]) -> Result<(), String> {
        self.stream
            .write_all(b)
            .and_then(|_| self.stream.write_all(self.term.as_bytes()))
            .map_err(|e| format!("SCPI write: {e}"))
    }
    /// Read one response line (terminator stripped).
    pub fn read_line(&mut self) -> Result<String, String> {
        let mut s = String::new();
        let n = self
            .reader
            .read_line(&mut s)
            .map_err(|e| format!("SCPI read: {e}"))?;
        if n == 0 {
            return Err("SCPI connection closed".into());
        }
        Ok(s.trim_end_matches(['\r', '\n']).to_string())
    }
    pub fn query(&mut self, cmd: &str) -> Result<String, String> {
        self.send(cmd)?;
        self.read_line()
    }
    /// Read exactly `n` raw bytes.
    pub fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, String> {
        let mut b = vec![0u8; n];
        self.reader
            .read_exact(&mut b)
            .map_err(|e| format!("SCPI read: {e}"))?;
        Ok(b)
    }
}

/// Settings of a [`ScpiAwg`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScpiAwgSettings {
    /// Full-scale amplitude (Vpp) corresponding to ±1.0.
    pub full_scale_vpp: f64,
    pub offset_v: f64,
    /// Waveform name stem (channel number appended).
    pub waveform_name: String,
    /// Instrument channel of each transmit channel (1-based); default 1, 2, …
    pub channel_map: Vec<u32>,
    /// `stop_tx` loops the collected samples (else plays once).
    pub loop_on_stop: bool,
    pub timeout_ms: u64,
}

impl Default for ScpiAwgSettings {
    fn default() -> Self {
        ScpiAwgSettings {
            full_scale_vpp: 1.0,
            offset_v: 0.0,
            waveform_name: "KSHANA".into(),
            channel_map: vec![],
            loop_on_stop: true,
            timeout_ms: 5000,
        }
    }
}

/// A generic SCPI AWG.
pub struct ScpiAwg {
    conn: ScpiConn,
    profile: ScpiProfile,
    settings: ScpiAwgSettings,
    cfg: Option<DeviceConfig>,
    idn: String,
    buf: Vec<Vec<f64>>,
    start: StartTime,
    status: DeviceStatus,
}

fn fill(t: &str, kv: &[(&str, &str)]) -> String {
    let mut s = t.to_string();
    for (k, v) in kv {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

impl ScpiAwg {
    /// Connect to `host` (or `host:port`; the profile's port otherwise), query the
    /// identity, run the profile's `init` commands. Prints the output policy.
    pub fn connect(
        host: &str,
        profile: ScpiProfile,
        settings: ScpiAwgSettings,
    ) -> Result<ScpiAwg, String> {
        profile.validate()?;
        let addr = if host.contains(':') {
            host.to_string()
        } else {
            format!("{host}:{}", profile.port)
        };
        announce_output_policy(&format!("scpi-awg:{addr} ({})", profile.name));
        let conn = ScpiConn::connect(
            &addr,
            Duration::from_millis(settings.timeout_ms),
            &profile.terminator,
        )?;
        let mut a = ScpiAwg {
            conn,
            profile,
            settings,
            cfg: None,
            idn: String::new(),
            buf: vec![],
            start: StartTime::Now,
            status: DeviceStatus::default(),
        };
        if let Some(q) = a.profile.idn.clone() {
            a.idn = a.conn.query(&q)?;
        }
        let init = a.profile.init.clone();
        a.run(&init, &[])?;
        a.check_errors()?;
        Ok(a)
    }

    pub fn idn(&self) -> &str {
        &self.idn
    }
    pub fn profile(&self) -> &ScpiProfile {
        &self.profile
    }

    fn run(&mut self, cmds: &[String], kv: &[(&str, &str)]) -> Result<(), String> {
        for c in cmds {
            self.conn.send(&fill(c, kv))?;
        }
        Ok(())
    }

    /// Drain the instrument error queue; an error if any entry is not "no error".
    pub fn check_errors(&mut self) -> Result<(), String> {
        let Some(q) = self.profile.error_query.clone() else {
            return Ok(());
        };
        let mut errs = vec![];
        for _ in 0..16 {
            let r = self.conn.query(&q)?;
            if r.trim_start_matches('+')
                .starts_with(&self.profile.no_error)
            {
                break;
            }
            errs.push(r);
        }
        if errs.is_empty() {
            Ok(())
        } else {
            Err(format!("instrument errors: {}", errs.join("; ")))
        }
    }

    fn encode(&self, x: &[f64]) -> (Option<Vec<u8>>, Option<String>) {
        let l = ((1i32 << (self.profile.dac_bits - 1)) - 1) as f64;
        let code = |v: f64| (v.clamp(-1.0, 1.0) * l).round() as i16;
        match self.profile.data_format {
            DataFormat::Int16LeBlock => (
                Some(x.iter().flat_map(|&v| code(v).to_le_bytes()).collect()),
                None,
            ),
            DataFormat::Int16BeBlock => (
                Some(x.iter().flat_map(|&v| code(v).to_be_bytes()).collect()),
                None,
            ),
            DataFormat::FloatCsv => (
                None,
                Some(
                    x.iter()
                        .map(|v| format!("{:.6}", v.clamp(-1.0, 1.0)))
                        .collect::<Vec<_>>()
                        .join(","),
                ),
            ),
            DataFormat::IntCsv => (
                None,
                Some(
                    x.iter()
                        .map(|&v| code(v).to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                ),
            ),
        }
    }

    /// Upload and play normalised channel data (`x[c]`, ±1.0 = full scale).
    pub fn play(
        &mut self,
        x: &[Vec<f64>],
        fs_hz: f64,
        looped: bool,
        at: StartTime,
    ) -> Result<(), String> {
        let p = self.profile.clone();
        let n = x.first().map(|c| c.len()).unwrap_or(0);
        if n < p.min_points || n > p.max_points || !n.is_multiple_of(p.granularity) {
            return Err(format!(
                "{n} points outside the instrument's limits ({}..={}, multiple of {})",
                p.min_points, p.max_points, p.granularity
            ));
        }
        let ext = self
            .cfg
            .as_ref()
            .map(|c| c.clock_source == RefSource::External)
            .unwrap_or(false);
        self.run(
            if ext {
                &p.ref_external
            } else {
                &p.ref_internal
            },
            &[],
        )?;
        for (c, data) in x.iter().enumerate() {
            let ch = self
                .settings
                .channel_map
                .get(c)
                .copied()
                .unwrap_or(c as u32 + 1)
                .to_string();
            let name = format!("{}{}", self.settings.waveform_name, ch);
            let rate = format!("{fs_hz}");
            let vpp = format!("{}", self.settings.full_scale_vpp);
            let off = format!("{}", self.settings.offset_v);
            let len = n.to_string();
            let kv: [(&str, &str); 6] = [
                ("ch", &ch),
                ("name", &name),
                ("rate", &rate),
                ("vpp", &vpp),
                ("offset", &off),
                ("len", &len),
            ];
            self.run(&p.output_off, &kv)?;
            let (bin, csv) = self.encode(data);
            for t in &p.upload {
                let t = fill(t, &kv);
                if let Some(b) = &bin {
                    if let Some(i) = t.find("{block}") {
                        let mut msg = t[..i].as_bytes().to_vec();
                        msg.extend_from_slice(block_header(b.len()).as_bytes());
                        msg.extend_from_slice(b);
                        msg.extend_from_slice(t[i + 7..].as_bytes());
                        self.conn.send_bytes(&msg)?;
                        continue;
                    }
                }
                let t = t.replace("{data}", csv.as_deref().unwrap_or(""));
                self.conn.send(&t)?;
            }
            self.run(&p.select, &kv)?;
            self.run(&p.sample_rate, &kv)?;
            self.run(&p.amplitude, &kv)?;
            self.run(&p.offset, &kv)?;
            self.run(
                if looped {
                    &p.loop_continuous
                } else {
                    &p.loop_once
                },
                &kv,
            )?;
            let trig = match at {
                StartTime::NextPps | StartTime::AtDeviceTimeNs(_) => &p.trigger_external,
                StartTime::Now => &p.trigger_immediate,
            };
            self.run(trig, &kv)?;
            self.run(&p.output_on, &kv)?;
        }
        if at == StartTime::Now {
            self.run(&p.software_trigger, &[])?;
        }
        if let Some(q) = p.opc.clone() {
            self.conn.query(&q)?;
        }
        self.check_errors()?;
        self.status.tx_streaming = true;
        self.status.pps_aligned = at == StartTime::NextPps;
        self.status.message = format!(
            "playing {n} points ({})",
            if looped { "loop" } else { "once" }
        );
        Ok(())
    }
}

impl SegmentPlayer for ScpiAwg {
    fn play_segment(&mut self, seg: &AwgSegment, at: StartTime) -> Result<(), String> {
        let x: Vec<Vec<f64>> = (0..seg.codes.len()).map(|c| seg.normalised(c)).collect();
        self.play(&x, seg.report.sample_rate_hz, seg.report.periodic, at)
    }
    fn stop_playback(&mut self) -> Result<(), String> {
        let n = self.cfg.as_ref().map(|c| c.tx_channels).unwrap_or(1).max(1);
        let off = self.profile.output_off.clone();
        for c in 0..n {
            let ch = self
                .settings
                .channel_map
                .get(c)
                .copied()
                .unwrap_or(c as u32 + 1)
                .to_string();
            self.run(&off, &[("ch", &ch)])?;
        }
        self.status.tx_streaming = false;
        self.check_errors()
    }
}

impl Device for ScpiAwg {
    fn name(&self) -> String {
        format!("scpi-awg:{} ({})", self.profile.name, self.idn)
    }
    fn realtime(&self) -> bool {
        false
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        cfg.validate()?;
        if cfg.rx_channels > 0 {
            return Err("an AWG has no receive channels".into());
        }
        self.cfg = Some(cfg.clone());
        Ok(())
    }
    fn sample_rate_hz(&self) -> Option<f64> {
        self.cfg.as_ref().map(|c| c.sample_rate_hz)
    }
    fn start_tx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        let cfg = self.cfg.as_ref().ok_or("device not configured")?;
        self.buf = vec![Vec::new(); cfg.tx_channels];
        self.start = at;
        self.status.samples_tx = 0;
        Ok(None)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        if block.len() != self.buf.len() {
            return Err("transmit stream not started or channel count differs".into());
        }
        let n = block.first().map(|c| c.len()).unwrap_or(0);
        if self.buf[0].len() + n > self.profile.max_points {
            return Err(format!(
                "AWG memory full ({} points): compile a periodic segment with hw::awg",
                self.profile.max_points
            ));
        }
        for (b, x) in self.buf.iter_mut().zip(block) {
            b.extend_from_slice(x);
        }
        self.status.samples_tx += n as u64;
        Ok(())
    }
    fn stop_tx(&mut self) -> Result<(), String> {
        let fs = self.sample_rate_hz().ok_or("device not configured")?;
        let x = std::mem::take(&mut self.buf);
        if x.first().map(|c| c.is_empty()).unwrap_or(true) {
            return Ok(());
        }
        let looped = self.settings.loop_on_stop;
        self.play(&x, fs, looped, self.start)
    }
    fn start_rx(&mut self, _at: StartTime) -> Result<Option<i64>, String> {
        Err("an AWG has no receive channels".into())
    }
    fn read_rx(&mut self, _n: usize) -> Result<Vec<Vec<f64>>, String> {
        Err("an AWG has no receive channels".into())
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        self.status.clone()
    }
}

#[cfg(test)]
pub(crate) mod mock {
    //! A mock SCPI instrument: records commands (binary blocks decoded), answers queries.
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, PartialEq)]
    pub enum Rx {
        Line(String),
        /// A command carrying a binary block: (text before, bytes, text after).
        Block(String, Vec<u8>, String),
    }

    pub struct Mock {
        pub addr: String,
        pub log: Arc<Mutex<Vec<Rx>>>,
    }

    /// Start a mock; `answer` maps a query line to its response (None = not a query).
    pub fn start(answer: fn(&str) -> Option<String>) -> Mock {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let log = Arc::new(Mutex::new(vec![]));
        let lg = log.clone();
        std::thread::spawn(move || {
            let Ok((mut s, _)) = l.accept() else { return };
            let mut buf = vec![];
            let mut byte = [0u8; 1];
            loop {
                if s.read(&mut byte).unwrap_or(0) == 0 {
                    return;
                }
                let b = byte[0];
                if b == b'#' {
                    // IEEE 488.2 definite-length block
                    s.read_exact(&mut byte).unwrap();
                    let nd = (byte[0] - b'0') as usize;
                    let mut d = vec![0u8; nd];
                    s.read_exact(&mut d).unwrap();
                    let len: usize = String::from_utf8(d).unwrap().parse().unwrap();
                    let mut data = vec![0u8; len];
                    s.read_exact(&mut data).unwrap();
                    let before = String::from_utf8(std::mem::take(&mut buf)).unwrap();
                    let mut after = vec![];
                    loop {
                        s.read_exact(&mut byte).unwrap();
                        if byte[0] == b'\n' {
                            break;
                        }
                        after.push(byte[0]);
                    }
                    let after = String::from_utf8(after).unwrap();
                    lg.lock().unwrap().push(Rx::Block(before, data, after));
                    continue;
                }
                if b == b'\n' {
                    let line = String::from_utf8(std::mem::take(&mut buf)).unwrap();
                    let line = line.trim_end_matches('\r').to_string();
                    if let Some(r) = answer(&line) {
                        s.write_all(format!("{r}\r\n").as_bytes()).unwrap();
                    }
                    lg.lock().unwrap().push(Rx::Line(line));
                } else {
                    buf.push(b);
                }
            }
        });
        Mock { addr, log }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{self, Rx};
    use super::*;

    fn answer(q: &str) -> Option<String> {
        match q {
            "*IDN?" => Some("MOCK,AWG,0,1.0".into()),
            "SYST:ERR?" | ":SYST:ERR?" => Some("+0,\"No error\"".into()),
            "*OPC?" => Some("1".into()),
            _ => None,
        }
    }

    fn lines(log: &[Rx]) -> Vec<String> {
        log.iter()
            .filter_map(|r| match r {
                Rx::Line(l) => Some(l.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn builtin_profiles_parse_and_round_trip() {
        for (n, _) in BUILTIN_PROFILES {
            let p = ScpiProfile::builtin(n).unwrap();
            assert_eq!(&p.name, n);
            assert_eq!(ScpiProfile::from_toml(&p.to_toml()).unwrap(), p);
        }
        assert_eq!(block_header(1234), "#41234");
        let mut bad = ScpiProfile::builtin("generic-binary").unwrap();
        bad.upload = vec!["DATA {data}".into()];
        assert!(bad.validate().is_err());
    }

    #[test]
    fn plays_segment_through_mock_instrument() {
        let m = mock::start(answer);
        let mut a = ScpiAwg::connect(
            &m.addr,
            ScpiProfile::builtin("generic-binary").unwrap(),
            ScpiAwgSettings::default(),
        )
        .unwrap();
        assert_eq!(a.idn(), "MOCK,AWG,0,1.0");
        let mut cfg = DeviceConfig::tx(1e6, 1);
        cfg.clock_source = RefSource::External;
        a.configure(&cfg).unwrap();
        // through the Device interface: collect, upload on stop
        a.start_tx(StartTime::NextPps).unwrap();
        let x: Vec<f64> = (0..1000).map(|i| (i as f64 / 1000.0) - 0.5).collect();
        a.write_tx(&[x[..600].to_vec()]).unwrap();
        a.write_tx(&[x[600..].to_vec()]).unwrap();
        a.stop_tx().unwrap();
        assert!(a.status().tx_streaming && a.status().pps_aligned);
        a.stop_playback().unwrap();
        drop(a);
        std::thread::sleep(Duration::from_millis(50));
        let log = m.log.lock().unwrap().clone();
        let l = lines(&log);
        for want in [
            "REF:SOUR EXT",
            "OUTP1 OFF",
            "WAVE1:SEL KSHANA1",
            "WAVE1:SRAT 1000000",
            "OUTP1:AMPL 1",
            "WAVE1:MODE CONT",
            "TRIG1:SOUR EXT",
            "OUTP1 ON",
        ] {
            assert!(l.iter().any(|c| c == want), "missing {want}: {l:?}");
        }
        assert!(!l.iter().any(|c| c == "*TRG"));
        let (before, data, after) = log
            .iter()
            .find_map(|r| match r {
                Rx::Block(b, d, a) => Some((b.clone(), d.clone(), a.clone())),
                _ => None,
            })
            .unwrap();
        assert_eq!(before, "WAVE1:DATA KSHANA1,");
        assert_eq!(after, "");
        assert_eq!(data.len(), 2000);
        let codes: Vec<i16> = data
            .chunks(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        for (i, c) in codes.iter().enumerate() {
            assert_eq!(*c, (x[i] * 32767.0).round() as i16);
        }
    }

    #[test]
    fn vendor_profiles_send_the_manual_commands() {
        // Keysight 33500B: DAC block upload, LSB first, named arb selected, 50 Ω load.
        let m = mock::start(answer);
        let mut a = ScpiAwg::connect(
            &m.addr,
            ScpiProfile::builtin("keysight-33500b").unwrap(),
            ScpiAwgSettings::default(),
        )
        .unwrap();
        let mut cfg = DeviceConfig::tx(2e6, 1);
        cfg.clock_source = RefSource::External;
        a.configure(&cfg).unwrap();
        let x: Vec<f64> = (0..400).map(|i| ((i as f64) * 0.05).sin() * 0.9).collect();
        a.play(std::slice::from_ref(&x), 2e6, true, StartTime::NextPps)
            .unwrap();
        drop(a);
        std::thread::sleep(Duration::from_millis(50));
        let log = m.log.lock().unwrap().clone();
        let l = lines(&log);
        for want in [
            "FORM:BORD SWAP",
            "ROSC:SOUR EXT",
            "SOUR1:DATA:VOL:CLE",
            "SOUR1:FUNC ARB",
            "SOUR1:FUNC:ARB KSHANA1",
            "SOUR1:FUNC:ARB:SRAT 2000000",
            "OUTP1:LOAD 50",
            "SOUR1:VOLT 1",
            "SOUR1:BURS:STAT OFF",
            "TRIG1:SOUR EXT",
            "OUTP1 ON",
        ] {
            assert!(l.iter().any(|c| c == want), "missing {want}: {l:?}");
        }
        let (before, data, _) = log
            .iter()
            .find_map(|r| match r {
                Rx::Block(b, d, a) => Some((b.clone(), d.clone(), a.clone())),
                _ => None,
            })
            .unwrap();
        assert_eq!(before, "SOUR1:DATA:ARB:DAC KSHANA1,");
        let c0 = i16::from_le_bytes([data[2], data[3]]);
        assert_eq!(c0, (x[1] * 32767.0).round() as i16);
        // Rigol DG1000Z: normalised CSV into the volatile waveform, at most 16384 points.
        let m = mock::start(answer);
        let mut a = ScpiAwg::connect(
            &m.addr,
            ScpiProfile::builtin("rigol-dg1000z").unwrap(),
            ScpiAwgSettings::default(),
        )
        .unwrap();
        a.configure(&DeviceConfig::tx(1e6, 1)).unwrap();
        a.play(&[x[..100].to_vec()], 1e6, false, StartTime::Now)
            .unwrap();
        assert!(a
            .play(&[vec![0.0; 20_000]], 1e6, false, StartTime::Now)
            .unwrap_err()
            .contains("outside"));
        drop(a);
        std::thread::sleep(Duration::from_millis(50));
        let l = lines(&m.log.lock().unwrap().clone());
        assert!(
            l.iter().any(|c| c.starts_with(":SOUR1:DATA VOLATILE,0")),
            "{l:?}"
        );
        for want in [
            ":SOUR1:FUNC:ARB:MODE SRAT",
            ":SOUR1:FUNC:ARB:SRAT 1000000",
            ":SOUR1:BURS:NCYC 1",
            ":TRIG1:SOUR INT",
            "*TRG",
        ] {
            assert!(l.iter().any(|c| c == want), "missing {want}: {l:?}");
        }
    }

    #[test]
    fn ascii_profile_and_limits_and_instrument_errors() {
        fn answer_err(q: &str) -> Option<String> {
            match q {
                "SYST:ERR?" => Some("-222,\"Data out of range\"".into()),
                q if q.ends_with('?') => Some("1".into()),
                _ => None,
            }
        }
        let m = mock::start(answer);
        let mut a = ScpiAwg::connect(
            &m.addr,
            ScpiProfile::builtin("generic-ascii").unwrap(),
            ScpiAwgSettings {
                channel_map: vec![2],
                ..Default::default()
            },
        )
        .unwrap();
        a.configure(&DeviceConfig::tx(2e6, 1)).unwrap();
        assert!(a.play(&[vec![0.0; 4]], 2e6, true, StartTime::Now).is_err());
        a.play(
            &[vec![0.5, -0.25, 1.5, 0.0, 0.1, 0.2, 0.3, 0.4]],
            2e6,
            false,
            StartTime::Now,
        )
        .unwrap();
        drop(a);
        std::thread::sleep(Duration::from_millis(50));
        let l = lines(&m.log.lock().unwrap());
        assert!(l.iter().any(|c| c
            == "WAVE2:DATA:ASC KSHANA2,0.500000,-0.250000,1.000000,0.000000,0.100000,0.200000,0.300000,0.400000"));
        assert!(l.iter().any(|c| c == "WAVE2:MODE ONCE"));
        assert!(l.iter().any(|c| c == "*TRG"));
        // an instrument reporting errors makes the connect fail with the message
        let m = mock::start(answer_err);
        let e = ScpiAwg::connect(
            &m.addr,
            ScpiProfile::builtin("generic-binary").unwrap(),
            ScpiAwgSettings::default(),
        )
        .err()
        .unwrap();
        assert!(e.contains("Data out of range"), "{e}");
    }
}
