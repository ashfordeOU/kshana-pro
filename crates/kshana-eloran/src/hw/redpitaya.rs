// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Red Pitaya STEMlab 125-14 (feature `redpitaya`, pure `std::net`): generator control and
//! acquisition over its SCPI server, continuous receive from its streaming server.
//!
//! Hardware facts used (Red Pitaya documentation, STEMlab 125-14): 14-bit ADCs and DACs at
//! 125 MS/s, two fast inputs (±1 V LV / ±20 V HV jumper) and two fast outputs (±1 V into
//! high impedance); the generator and acquisition buffers hold 16 384 samples per channel;
//! SCPI server on TCP port 5000 with `\r\n` terminators. The standard board has **no
//! external 10 MHz input** (an external-clock variant or modification is needed) — the
//! driver refuses [`RefSource::External`] unless `external_clock_board` is set.
//!
//! * **Transmit** — continuous DAC streaming is limited on this board, so transmit uses
//!   **arbitrary-waveform upload** of a compiled segment ([`super::awg`]): `SOUR<n>:FUNC
//!   ARBITRARY`, `SOUR<n>:TRAC:DATA:DATA` (normalised values), `SOUR<n>:FREQ:FIX` = sample
//!   rate / points (the firmware plays the uploaded table once per period), amplitude,
//!   burst/continuous mode, trigger (`EXT_PE` for a PPS on the external trigger input,
//!   else `INT`), `OUTPUT<n>:STATE ON`. With 16 384 points a full phase-code cycle
//!   (2×GRI ≈ 80–200 ms) cannot be held at an LF-useful sample rate, so in practice this
//!   is for single-group / pulse tests in burst mode; use UHD or SoapySDR hardware for
//!   continuous scenes. As a [`Device`], `write_tx` collects up to 16 384 samples and
//!   `stop_tx` uploads and plays them (looped when `loop_on_stop`).
//! * **Receive** — [`RxMode::Scpi`]: triggered buffer acquisitions (`ACQ:DEC`, `ACQ:START`,
//!   `ACQ:TRIG`, poll `ACQ:TRIG:STAT?` for `TD`, `ACQ:SOUR<n>:DATA?`); each read is a new
//!   16 384-sample acquisition, so successive blocks are **not contiguous** (every gap is
//!   counted as an overflow). [`RxMode::Stream`]: the streaming server (TCP), read as
//!   frames of `header_bytes` + `payload_bytes` of interleaved samples per
//!   [`StreamFraming`]. The streaming protocol has changed between Red Pitaya OS versions;
//!   the default framing (no header, interleaved int16 little-endian) is **NEEDS
//!   CONFIRMATION** against the server version in use and must be configured to match it.
//!
//! SCPI command names follow the Red Pitaya OS 2.x SCPI documentation; older OS versions
//! differ in places (e.g. burst-state values) — NEEDS CONFIRMATION per OS version.
//!
//! **Output policy: conducted use only** — cables or antenna simulators, never a radiating
//! antenna; printed on connect.
//!
//! Evidence: protocol tested against mock TCP servers; **UNTESTED ON HARDWARE**.

use super::awg::{AwgSegment, SegmentPlayer};
use super::scpi_awg::ScpiConn;
use super::{announce_output_policy, Device, DeviceConfig, DeviceStatus, RefSource, StartTime};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// ADC/DAC sample clock of the STEMlab 125-14 (Hz).
pub const RP_CLOCK_HZ: f64 = 125e6;
/// Generator / acquisition buffer length (samples per channel).
pub const RP_BUFFER: usize = 16_384;

/// Sample encoding of the streaming server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamSample {
    I16Le,
    I8,
}

/// Framing of the streaming server's TCP data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamFraming {
    /// Bytes skipped before every frame's payload.
    pub header_bytes: usize,
    /// Payload bytes per frame (0 = one continuous stream without headers).
    pub payload_bytes: usize,
    /// Interleaved channels in the payload.
    pub channels: usize,
    pub sample: StreamSample,
    /// Count corresponding to ±1.0 (ADC full scale).
    pub full_scale_counts: f64,
    /// Stream sample rate (Hz) = 125 MHz / decimation, as set in the streaming server.
    pub sample_rate_hz: f64,
}

impl Default for StreamFraming {
    fn default() -> Self {
        StreamFraming {
            header_bytes: 0,
            payload_bytes: 0,
            channels: 2,
            sample: StreamSample::I16Le,
            full_scale_counts: 8192.0,
            sample_rate_hz: RP_CLOCK_HZ / 64.0,
        }
    }
}

/// How the receive side gets samples.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RxMode {
    /// Triggered SCPI buffer acquisitions with this decimation (fs = 125 MHz / dec).
    Scpi { decimation: u32 },
    /// The streaming server.
    Stream,
}

/// Settings of a [`RedPitaya`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RedPitayaSettings {
    pub scpi_port: u16,
    pub stream_port: u16,
    pub timeout_ms: u64,
    /// Generator amplitude (V) for ±1.0.
    pub out_full_scale_v: f64,
    /// Input range (V) corresponding to ±1.0 (1 V with the LV jumper).
    pub in_full_scale_v: f64,
    pub rx_mode: RxMode,
    pub stream: StreamFraming,
    /// The board has an external sample clock input (external-clock variant/modification).
    pub external_clock_board: bool,
    pub loop_on_stop: bool,
}

impl Default for RedPitayaSettings {
    fn default() -> Self {
        RedPitayaSettings {
            scpi_port: 5000,
            stream_port: 8900,
            timeout_ms: 5000,
            out_full_scale_v: 1.0,
            in_full_scale_v: 1.0,
            rx_mode: RxMode::Scpi { decimation: 64 },
            stream: StreamFraming::default(),
            external_clock_board: false,
            loop_on_stop: true,
        }
    }
}

/// A Red Pitaya STEMlab 125-14.
pub struct RedPitaya {
    host: String,
    scpi: ScpiConn,
    settings: RedPitayaSettings,
    cfg: Option<DeviceConfig>,
    buf: Vec<Vec<f64>>,
    start: StartTime,
    rx_start: StartTime,
    stream: Option<TcpStream>,
    pending: Vec<u8>,
    frame_left: usize,
    header_left: usize,
    rx_blocks: u64,
    status: DeviceStatus,
}

fn parse_data(s: &str) -> Result<Vec<f64>, String> {
    let t = s.trim().trim_start_matches('{').trim_end_matches('}');
    if t.is_empty() {
        return Ok(vec![]);
    }
    t.split(',')
        .map(|v| {
            v.trim()
                .parse::<f64>()
                .map_err(|e| format!("bad sample {v:?}: {e}"))
        })
        .collect()
}

impl RedPitaya {
    /// Connect to the SCPI server of `host` and reset the generator. Prints the output
    /// policy.
    pub fn connect(host: &str, settings: RedPitayaSettings) -> Result<RedPitaya, String> {
        announce_output_policy(&format!("redpitaya:{host}"));
        let scpi = ScpiConn::connect(
            &format!("{host}:{}", settings.scpi_port),
            Duration::from_millis(settings.timeout_ms),
            "\r\n",
        )?;
        let mut rp = RedPitaya {
            host: host.into(),
            scpi,
            settings,
            cfg: None,
            buf: vec![],
            start: StartTime::Now,
            rx_start: StartTime::Now,
            stream: None,
            pending: vec![],
            frame_left: 0,
            header_left: 0,
            rx_blocks: 0,
            status: DeviceStatus::default(),
        };
        rp.scpi.send("GEN:RST")?;
        Ok(rp)
    }

    /// Upload and play normalised data (≤ 16 384 points per channel, ±1.0 full scale).
    pub fn play(
        &mut self,
        x: &[Vec<f64>],
        fs_hz: f64,
        looped: bool,
        at: StartTime,
    ) -> Result<(), String> {
        if x.len() > 2 {
            return Err("the STEMlab 125-14 has two outputs".into());
        }
        let n = x.first().map(|c| c.len()).unwrap_or(0);
        if n == 0 || n > RP_BUFFER {
            return Err(format!(
                "{n} points: the generator buffer holds 1..={RP_BUFFER} (compile a shorter \
                 segment, or use a streaming device for continuous scenes)"
            ));
        }
        for (c, d) in x.iter().enumerate() {
            let ch = c + 1;
            let data: Vec<String> = d
                .iter()
                .map(|v| format!("{:.5}", v.clamp(-1.0, 1.0)))
                .collect();
            self.scpi.send(&format!("OUTPUT{ch}:STATE OFF"))?;
            self.scpi.send(&format!("SOUR{ch}:FUNC ARBITRARY"))?;
            self.scpi
                .send(&format!("SOUR{ch}:TRAC:DATA:DATA {}", data.join(",")))?;
            self.scpi
                .send(&format!("SOUR{ch}:FREQ:FIX {}", fs_hz / n as f64))?;
            self.scpi
                .send(&format!("SOUR{ch}:VOLT {}", self.settings.out_full_scale_v))?;
            self.scpi.send(&format!("SOUR{ch}:VOLT:OFFS 0"))?;
            if looped {
                self.scpi.send(&format!("SOUR{ch}:BURS:STAT CONTINUOUS"))?;
            } else {
                self.scpi.send(&format!("SOUR{ch}:BURS:STAT BURST"))?;
                self.scpi.send(&format!("SOUR{ch}:BURS:NCYC 1"))?;
            }
            let src = match at {
                StartTime::Now => "INT",
                _ => "EXT_PE",
            };
            self.scpi.send(&format!("SOUR{ch}:TRIG:SOUR {src}"))?;
            self.scpi.send(&format!("OUTPUT{ch}:STATE ON"))?;
            if at == StartTime::Now {
                self.scpi.send(&format!("SOUR{ch}:TRIG:INT"))?;
            }
        }
        self.status.tx_streaming = true;
        self.status.pps_aligned = at == StartTime::NextPps;
        Ok(())
    }

    fn scpi_acquire(&mut self, n: usize, dec: u32) -> Result<Vec<Vec<f64>>, String> {
        let nch = self
            .cfg
            .as_ref()
            .map(|c| c.rx_channels)
            .unwrap_or(1)
            .clamp(1, 2);
        let s = &mut self.scpi;
        s.send("ACQ:RST")?;
        s.send(&format!("ACQ:DEC {dec}"))?;
        s.send("ACQ:DATA:FORMAT ASCII")?;
        s.send("ACQ:DATA:UNITS VOLTS")?;
        s.send(&format!("ACQ:TRIG:DLY {}", RP_BUFFER / 2))?;
        s.send("ACQ:START")?;
        let trig = match self.rx_start {
            StartTime::Now => "NOW",
            _ => "EXT_PE",
        };
        s.send(&format!("ACQ:TRIG {trig}"))?;
        let t0 = Instant::now();
        loop {
            if s.query("ACQ:TRIG:STAT?")?.trim() == "TD" {
                break;
            }
            if t0.elapsed() > Duration::from_millis(self.settings.timeout_ms) {
                return Err("Red Pitaya acquisition: trigger timeout".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut out = vec![];
        for ch in 1..=nch {
            let mut v = parse_data(&s.query(&format!("ACQ:SOUR{ch}:DATA?"))?)?;
            v.truncate(n);
            for x in v.iter_mut() {
                *x /= self.settings.in_full_scale_v;
            }
            out.push(v);
        }
        if self.rx_blocks > 0 {
            self.status.overflows += 1; // gap between acquisitions
        }
        self.rx_blocks += 1;
        Ok(out)
    }

    fn stream_read(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        let fr = self.settings.stream.clone();
        let bps = match fr.sample {
            StreamSample::I16Le => 2,
            StreamSample::I8 => 1,
        };
        let want = n * fr.channels.max(1) * bps;
        let st = self.stream.as_mut().ok_or("receive stream not started")?;
        let mut chunk = vec![0u8; 65_536];
        while self.pending.len() < want {
            let k = st
                .read(&mut chunk)
                .map_err(|e| format!("Red Pitaya stream: {e}"))?;
            if k == 0 {
                break;
            }
            let mut i = 0;
            while i < k {
                if fr.payload_bytes > 0 && self.frame_left == 0 && self.header_left == 0 {
                    self.header_left = fr.header_bytes;
                    self.frame_left = fr.payload_bytes;
                }
                if self.header_left > 0 {
                    let s = self.header_left.min(k - i);
                    self.header_left -= s;
                    i += s;
                    continue;
                }
                let s = if fr.payload_bytes > 0 {
                    self.frame_left.min(k - i)
                } else {
                    k - i
                };
                self.pending.extend_from_slice(&chunk[i..i + s]);
                if fr.payload_bytes > 0 {
                    self.frame_left -= s;
                }
                i += s;
            }
        }
        let frame = fr.channels.max(1) * bps;
        let m = (self.pending.len() / frame).min(n);
        let nch = self
            .cfg
            .as_ref()
            .map(|c| c.rx_channels)
            .unwrap_or(fr.channels)
            .min(fr.channels)
            .max(1);
        let mut out = vec![Vec::with_capacity(m); nch];
        for j in 0..m {
            for (c, o) in out.iter_mut().enumerate() {
                let p = j * frame + c * bps;
                let v = match fr.sample {
                    StreamSample::I16Le => {
                        i16::from_le_bytes([self.pending[p], self.pending[p + 1]]) as f64
                    }
                    StreamSample::I8 => self.pending[p] as i8 as f64,
                };
                o.push(v / fr.full_scale_counts);
            }
        }
        self.pending.drain(..m * frame);
        Ok(out)
    }
}

impl SegmentPlayer for RedPitaya {
    fn play_segment(&mut self, seg: &AwgSegment, at: StartTime) -> Result<(), String> {
        let x: Vec<Vec<f64>> = (0..seg.codes.len()).map(|c| seg.normalised(c)).collect();
        self.play(&x, seg.report.sample_rate_hz, seg.report.periodic, at)
    }
    fn stop_playback(&mut self) -> Result<(), String> {
        self.scpi.send("OUTPUT1:STATE OFF")?;
        self.scpi.send("OUTPUT2:STATE OFF")?;
        self.status.tx_streaming = false;
        Ok(())
    }
}

impl Device for RedPitaya {
    fn name(&self) -> String {
        format!("redpitaya:{}", self.host)
    }
    fn realtime(&self) -> bool {
        false
    }
    fn configure(&mut self, cfg: &DeviceConfig) -> Result<(), String> {
        cfg.validate()?;
        if cfg.tx_channels > 2 || cfg.rx_channels > 2 {
            return Err("the STEMlab 125-14 has two inputs and two outputs".into());
        }
        if cfg.clock_source == RefSource::External && !self.settings.external_clock_board {
            return Err(
                "the standard STEMlab 125-14 has no external 10 MHz input (set \
                 external_clock_board for an external-clock board)"
                    .into(),
            );
        }
        if cfg.rx_channels > 0 {
            let fs = match &self.settings.rx_mode {
                RxMode::Scpi { decimation } => RP_CLOCK_HZ / *decimation as f64,
                RxMode::Stream => self.settings.stream.sample_rate_hz,
            };
            if (fs - cfg.sample_rate_hz).abs() > 1e-6 {
                return Err(format!(
                    "receive rate is {fs} Hz with these settings, configured {} Hz",
                    cfg.sample_rate_hz
                ));
            }
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
        Ok(None)
    }
    fn write_tx(&mut self, block: &[Vec<f64>]) -> Result<(), String> {
        if block.len() != self.buf.len() || self.buf.is_empty() {
            return Err("transmit stream not started or channel count differs".into());
        }
        let n = block.first().map(|c| c.len()).unwrap_or(0);
        if self.buf[0].len() + n > RP_BUFFER {
            return Err(format!(
                "generator buffer full ({RP_BUFFER} samples): continuous streaming is not \
                 supported on this board"
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
    fn start_rx(&mut self, at: StartTime) -> Result<Option<i64>, String> {
        self.cfg.as_ref().ok_or("device not configured")?;
        self.rx_start = at;
        self.rx_blocks = 0;
        if self.settings.rx_mode == RxMode::Stream {
            let addr = format!("{}:{}", self.host, self.settings.stream_port);
            let sa = addr
                .to_socket_addrs()
                .map_err(|e| e.to_string())?
                .next()
                .ok_or("no address")?;
            let t = Duration::from_millis(self.settings.timeout_ms);
            let s = TcpStream::connect_timeout(&sa, t).map_err(|e| format!("{addr}: {e}"))?;
            s.set_read_timeout(Some(t)).map_err(|e| e.to_string())?;
            self.stream = Some(s);
            self.pending.clear();
            self.frame_left = 0;
            self.header_left = 0;
        }
        self.status.rx_streaming = true;
        self.status.pps_aligned = at == StartTime::NextPps;
        Ok(None)
    }
    fn read_rx(&mut self, n: usize) -> Result<Vec<Vec<f64>>, String> {
        if !self.status.rx_streaming {
            return Err("receive stream not started".into());
        }
        let b = match self.settings.rx_mode.clone() {
            RxMode::Scpi { decimation } => self.scpi_acquire(n.min(RP_BUFFER), decimation)?,
            RxMode::Stream => self.stream_read(n)?,
        };
        self.status.samples_rx += b.first().map(|c| c.len()).unwrap_or(0) as u64;
        Ok(b)
    }
    fn stop_rx(&mut self) -> Result<(), String> {
        self.stream = None;
        self.status.rx_streaming = false;
        Ok(())
    }
    fn status(&self) -> DeviceStatus {
        self.status.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::super::scpi_awg::mock::{self, Rx};
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    fn answer(q: &str) -> Option<String> {
        match q {
            "ACQ:TRIG:STAT?" => Some("TD".into()),
            "ACQ:SOUR1:DATA?" => Some("{0.5,-0.25,0.125,0.0}".into()),
            "ACQ:SOUR2:DATA?" => Some("{0.1,0.2,0.3,0.4}".into()),
            _ => None,
        }
    }

    #[test]
    fn generator_upload_and_scpi_acquisition() {
        let m = mock::start(answer);
        let mut rp = RedPitaya::connect(
            "127.0.0.1",
            RedPitayaSettings {
                scpi_port: m.addr.rsplit(':').next().unwrap().parse().unwrap(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut cfg = DeviceConfig::tx(RP_CLOCK_HZ / 64.0, 1);
        cfg.rx_channels = 2;
        rp.configure(&cfg).unwrap();
        let mut ext = cfg.clone();
        ext.clock_source = RefSource::External;
        assert!(rp.configure(&ext).is_err());
        rp.start_tx(StartTime::NextPps).unwrap();
        rp.write_tx(&[vec![0.5, -0.5, 0.25, 2.0]]).unwrap();
        assert!(rp.write_tx(&[vec![0.0; RP_BUFFER]]).is_err());
        rp.stop_tx().unwrap();
        rp.start_rx(StartTime::Now).unwrap();
        let a = rp.read_rx(1000).unwrap();
        assert_eq!(a[0], vec![0.5, -0.25, 0.125, 0.0]);
        assert_eq!(a[1], vec![0.1, 0.2, 0.3, 0.4]);
        let _ = rp.read_rx(1000).unwrap();
        assert_eq!(rp.status().overflows, 1); // second acquisition is not contiguous
        drop(rp);
        std::thread::sleep(Duration::from_millis(50));
        let l: Vec<String> = m
            .log
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| match r {
                Rx::Line(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        for want in [
            "GEN:RST",
            "SOUR1:FUNC ARBITRARY",
            "SOUR1:TRAC:DATA:DATA 0.50000,-0.50000,0.25000,1.00000",
            "SOUR1:BURS:STAT CONTINUOUS",
            "SOUR1:TRIG:SOUR EXT_PE",
            "OUTPUT1:STATE ON",
            "ACQ:DEC 64",
            "ACQ:TRIG NOW",
        ] {
            assert!(l.iter().any(|c| c == want), "missing {want}: {l:?}");
        }
        let f = l.iter().find(|c| c.starts_with("SOUR1:FREQ:FIX")).unwrap();
        let v: f64 = f.split(' ').nth(1).unwrap().parse().unwrap();
        assert!((v - RP_CLOCK_HZ / 64.0 / 4.0).abs() < 1e-6);
    }

    #[test]
    fn streaming_server_frames_are_decoded() {
        let scpi = mock::start(|_| None);
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            // 3 frames: 4-byte header + 4 interleaved (ch1, ch2) int16 pairs each
            for f in 0..3i16 {
                let mut b = vec![0xAA, 0xBB, 0xCC, 0xDD];
                for k in 0..4i16 {
                    let v1 = (f * 4 + k) * 100;
                    b.extend_from_slice(&v1.to_le_bytes());
                    b.extend_from_slice(&(-v1).to_le_bytes());
                }
                s.write_all(&b).unwrap();
            }
        });
        let mut rp = RedPitaya::connect(
            "127.0.0.1",
            RedPitayaSettings {
                scpi_port: scpi.addr.rsplit(':').next().unwrap().parse().unwrap(),
                stream_port: port,
                rx_mode: RxMode::Stream,
                stream: StreamFraming {
                    header_bytes: 4,
                    payload_bytes: 16,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let mut cfg = DeviceConfig::tx(RP_CLOCK_HZ / 64.0, 0);
        cfg.rx_channels = 2;
        rp.configure(&cfg).unwrap();
        rp.start_rx(StartTime::Now).unwrap();
        let mut got: [Vec<f64>; 2] = [vec![], vec![]];
        while got[0].len() < 12 {
            let b = rp.read_rx(12 - got[0].len()).unwrap();
            if b[0].is_empty() {
                break;
            }
            for c in 0..2 {
                got[c].extend(b[c].clone());
            }
        }
        assert_eq!(got[0].len(), 12);
        for i in 0..12 {
            assert_eq!(got[0][i], (i as f64 * 100.0) / 8192.0);
            assert_eq!(got[1][i], -(i as f64 * 100.0) / 8192.0);
        }
    }
}
