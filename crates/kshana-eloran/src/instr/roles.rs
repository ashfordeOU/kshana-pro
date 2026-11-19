// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Role drivers over a [`ScpiSession`] and a command profile.
//!
//! Each driver implements one role trait of [`super`] and sends only the profile's
//! templates, so the same code drives any instrument whose manual the profile was mapped
//! to. All exchanges reach the session's measurement log.

use super::profile::{
    fill, fill_all, FreqCounterCommands, InstrumentProfile, PowerMeterCommands, Role, SaCommands,
    ScopeCommands, TicCommands,
};
use super::{
    parse_number, parse_number_list, FrequencySource, PowerSource, ScpiSession, SharedLog,
    SpectrumSource, TicSource, Trace, Waveform, WaveformSource,
};
use crate::hw::calib::TicUnit;

fn need<T: Clone>(p: &InstrumentProfile, role: Role, c: &Option<T>) -> Result<T, String> {
    if p.role != role {
        return Err(format!(
            "profile {} has role {:?}, expected {role:?}",
            p.id, p.role
        ));
    }
    c.clone()
        .ok_or_else(|| format!("profile {}: missing command set for {role:?}", p.id))
}

/// Time-interval counter (start = reference PPS, stop = device under test).
pub struct TimeIntervalCounter {
    pub session: ScpiSession,
    pub cmd: TicCommands,
}

impl TimeIntervalCounter {
    pub fn new(mut session: ScpiSession, cmd: TicCommands) -> Result<Self, String> {
        session.write_all(&cmd.setup)?;
        Ok(TimeIntervalCounter { session, cmd })
    }
    pub fn open(p: &InstrumentProfile, log: Option<SharedLog>) -> Result<Self, String> {
        let c = need(p, Role::TimeIntervalCounter, &p.tic)?;
        Self::new(p.connect(log)?, c)
    }
    fn to_ns(&self, v: f64) -> f64 {
        (match self.cmd.unit {
            TicUnit::Seconds => v * 1e9,
            TicUnit::Nanoseconds => v,
        }) - self.cmd.offset_ns
    }
}

impl TicSource for TimeIntervalCounter {
    fn id(&self) -> String {
        self.session.id.clone()
    }
    fn arm_pps(&mut self) -> Result<(), String> {
        let c = self.cmd.arm_pps.clone();
        self.session.write_all(&c)
    }
    fn read_n(&mut self, n: usize) -> Result<Vec<f64>, String> {
        if let Some(t) = self.cmd.read_block.clone() {
            let q = fill(&t, &[("n", n.to_string())])?;
            let v = parse_number_list(&self.session.query(&q)?)?;
            if v.len() != n {
                return Err(format!(
                    "{}: asked for {n} readings, got {}",
                    self.session.id,
                    v.len()
                ));
            }
            return Ok(v.into_iter().map(|x| self.to_ns(x)).collect());
        }
        let q = self.cmd.read_one.clone();
        (0..n)
            .map(|_| {
                self.session
                    .query(&q)
                    .and_then(|r| parse_number(&r))
                    .map(|x| self.to_ns(x))
            })
            .collect()
    }
}

/// Oscilloscope capturing one channel triggered on the PPS.
pub struct Oscilloscope {
    pub session: ScpiSession,
    pub cmd: ScopeCommands,
}

impl Oscilloscope {
    pub fn new(mut session: ScpiSession, cmd: ScopeCommands) -> Result<Self, String> {
        let v = [
            ("trigger_source", cmd.trigger_source.clone()),
            ("trigger_level_v", cmd.trigger_level_v.to_string()),
        ];
        session.write_all(&fill_all(&cmd.trigger_pps, &v)?)?;
        Ok(Oscilloscope { session, cmd })
    }
    pub fn open(p: &InstrumentProfile, log: Option<SharedLog>) -> Result<Self, String> {
        let c = need(p, Role::Oscilloscope, &p.scope)?;
        Self::new(p.connect(log)?, c)
    }
    fn scope_channel(&self, channel: usize) -> usize {
        self.cmd
            .channel_map
            .get(channel)
            .copied()
            .unwrap_or(channel + 1)
    }
    fn vars(&self, channel: usize) -> Vec<(&'static str, String)> {
        vec![
            ("channel", self.scope_channel(channel).to_string()),
            ("points", self.cmd.points.to_string()),
            ("time_per_div_s", self.cmd.time_per_div_s.to_string()),
            ("trigger_source", self.cmd.trigger_source.clone()),
            ("trigger_level_v", self.cmd.trigger_level_v.to_string()),
        ]
    }
}

impl WaveformSource for Oscilloscope {
    fn id(&self) -> String {
        self.session.id.clone()
    }
    fn arm(&mut self, channel: usize) -> Result<(), String> {
        let v = self.vars(channel);
        let mut c = fill_all(&self.cmd.setup_channel, &v)?;
        c.extend(fill_all(&self.cmd.arm, &v)?);
        self.session.write_all(&c)
    }
    fn fetch(&mut self, channel: usize) -> Result<Waveform, String> {
        let v = self.vars(channel);
        let w = self.cmd.wait.clone();
        self.session.query(&w)?;
        // select the channel for transfer
        let src: Vec<String> = fill_all(&self.cmd.setup_channel, &v)?
            .into_iter()
            .filter(|c| c.contains("SOUR"))
            .collect();
        self.session.write_all(&src)?;
        let (xi, xo, d) = (
            self.cmd.x_increment.clone(),
            self.cmd.x_origin.clone(),
            self.cmd.data.clone(),
        );
        let dt = parse_number(&self.session.query(&xi)?)?;
        if dt.is_nan() || dt <= 0.0 {
            return Err(format!("{}: invalid sample interval {dt}", self.session.id));
        }
        let t0 = parse_number(&self.session.query(&xo)?)?;
        let samples = parse_number_list(&self.session.query(&d)?)?;
        Ok(Waveform {
            fs_hz: 1.0 / dt,
            t0_s: t0,
            samples,
            unit: "V".into(),
        })
    }
}

/// Spectrum analyser.
pub struct SpectrumAnalyser {
    pub session: ScpiSession,
    pub cmd: SaCommands,
    last: Option<(f64, f64)>,
}

impl SpectrumAnalyser {
    pub fn new(session: ScpiSession, cmd: SaCommands) -> Self {
        SpectrumAnalyser {
            session,
            cmd,
            last: None,
        }
    }
    pub fn open(p: &InstrumentProfile, log: Option<SharedLog>) -> Result<Self, String> {
        let c = need(p, Role::SpectrumAnalyser, &p.sa)?;
        Ok(Self::new(p.connect(log)?, c))
    }
}

impl SpectrumSource for SpectrumAnalyser {
    fn id(&self) -> String {
        self.session.id.clone()
    }
    fn select_input(&mut self, input: usize) -> Result<(), String> {
        let c = fill_all(&self.cmd.select_input, &[("input", input.to_string())])?;
        self.session.write_all(&c)
    }
    fn sweep(&mut self, start_hz: f64, stop_hz: f64, rbw_hz: f64) -> Result<Trace, String> {
        let v = [
            ("start_hz", start_hz.to_string()),
            ("stop_hz", stop_hz.to_string()),
            ("rbw_hz", rbw_hz.to_string()),
            ("points", self.cmd.points.to_string()),
        ];
        let mut c = fill_all(&self.cmd.set_sweep, &v)?;
        c.extend(self.cmd.sweep.clone());
        self.session.write_all(&c)?;
        let (w, t) = (self.cmd.wait.clone(), self.cmd.trace.clone());
        self.session.query(&w)?;
        let dbm = parse_number_list(&self.session.query(&t)?)?;
        if dbm.len() < 2 {
            return Err(format!(
                "{}: trace has {} points",
                self.session.id,
                dbm.len()
            ));
        }
        self.last = Some((start_hz, stop_hz));
        Ok(Trace::uniform(
            start_hz,
            stop_hz,
            dbm,
            rbw_hz,
            self.cmd.enbw_factor,
        ))
    }
    fn marker_peak(&mut self) -> Result<(f64, f64), String> {
        let c = self.cmd.marker_peak.clone();
        self.session.write_all(&c)?;
        let (qx, qy) = (self.cmd.marker_x.clone(), self.cmd.marker_y.clone());
        let x = parse_number(&self.session.query(&qx)?)?;
        let y = parse_number(&self.session.query(&qy)?)?;
        Ok((x, y))
    }
}

/// Power meter.
pub struct PowerMeter {
    pub session: ScpiSession,
    pub cmd: PowerMeterCommands,
}

impl PowerMeter {
    pub fn new(session: ScpiSession, cmd: PowerMeterCommands) -> Self {
        PowerMeter { session, cmd }
    }
    pub fn open(p: &InstrumentProfile, log: Option<SharedLog>) -> Result<Self, String> {
        let c = need(p, Role::PowerMeter, &p.power_meter)?;
        Ok(Self::new(p.connect(log)?, c))
    }
}

impl PowerSource for PowerMeter {
    fn id(&self) -> String {
        self.session.id.clone()
    }
    fn read_dbm(&mut self, freq_hz: f64) -> Result<f64, String> {
        let c = fill_all(&self.cmd.set_freq, &[("freq_hz", freq_hz.to_string())])?;
        self.session.write_all(&c)?;
        let q = self.cmd.read.clone();
        let v = parse_number(&self.session.query(&q)?)?;
        match self.cmd.unit.to_ascii_lowercase().as_str() {
            "dbm" => Ok(v),
            "w" => {
                if v <= 0.0 {
                    return Err(format!("{}: non-positive power {v} W", self.session.id));
                }
                Ok(10.0 * (v * 1e3).log10())
            }
            u => Err(format!("{}: unknown power unit {u:?}", self.session.id)),
        }
    }
}

/// Frequency counter.
pub struct FrequencyCounter {
    pub session: ScpiSession,
    pub cmd: FreqCounterCommands,
}

impl FrequencyCounter {
    pub fn new(session: ScpiSession, cmd: FreqCounterCommands) -> Self {
        FrequencyCounter { session, cmd }
    }
    pub fn open(p: &InstrumentProfile, log: Option<SharedLog>) -> Result<Self, String> {
        let c = need(p, Role::FrequencyCounter, &p.freq_counter)?;
        Ok(Self::new(p.connect(log)?, c))
    }
}

impl FrequencySource for FrequencyCounter {
    fn id(&self) -> String {
        self.session.id.clone()
    }
    fn read_hz(&mut self, gate_s: f64) -> Result<f64, String> {
        let c = fill_all(&self.cmd.setup, &[("gate_s", gate_s.to_string())])?;
        self.session.write_all(&c)?;
        let q = self.cmd.read.clone();
        parse_number(&self.session.query(&q)?)
    }
}

#[cfg(test)]
mod tests {
    use super::super::mock::MockTcpInstrument;
    use super::super::profile::TransportConfig;
    use super::super::MeasurementLog;
    use super::*;

    fn tcp(m: &MockTcpInstrument) -> TransportConfig {
        TransportConfig::Tcp {
            host: "127.0.0.1".into(),
            port: m.port(),
            timeout_ms: 2000,
            terminator: "\n".into(),
        }
    }

    #[test]
    fn tic_reads_one_at_a_time_and_in_blocks_and_logs_everything() {
        let mut k = 0;
        let m = MockTcpInstrument::spawn(move |cmd| match cmd {
            ":READ?" => {
                k += 1;
                Some(format!("+{:.6E}", 100e-9 + k as f64 * 1e-9))
            }
            c if c.starts_with(":SAMP:COUN") => None,
            ":FETC:ARR? 3" => Some("1.0E-7,1.1E-7,1.2E-7".into()),
            c if c.starts_with(":FETC:ARR?") => Some("1.0E-7".into()),
            _ => None,
        })
        .unwrap();
        let log = MeasurementLog::memory().with_clock(|| "T".into()).shared();
        let p = InstrumentProfile::default_for("tic", Role::TimeIntervalCounter, tcp(&m));
        let mut tic = TimeIntervalCounter::open(&p, Some(log.clone())).unwrap();
        tic.arm_pps().unwrap();
        let r = tic.read_n(2).unwrap();
        assert!(
            (r[0] - 101.0).abs() < 1e-6 && (r[1] - 102.0).abs() < 1e-6,
            "{r:?}"
        );
        tic.cmd.read_block = Some(":FETC:ARR? {n}".into());
        tic.cmd.offset_ns = 100.0;
        let b = tic.read_n(3).unwrap();
        assert!((b[2] - 20.0).abs() < 1e-6, "{b:?}");
        assert!(tic.read_n(4).unwrap_err().contains("got 1"));
        let l = log.lock().unwrap();
        let cmds: Vec<&str> = l.entries().iter().map(|e| e.command.as_str()).collect();
        assert_eq!(
            &cmds[..6],
            &[
                "*CLS",
                ":CONF:TINT (@1),(@2)",
                ":TRIG:SOUR EXT",
                ":INIT",
                ":READ?",
                ":READ?"
            ]
        );
        assert_eq!(l.entries()[4].response.as_deref(), Some("+1.010000E-7"));
    }

    #[test]
    fn scope_power_meter_counter_and_analyser_over_mock_tcp() {
        // scope: 5 samples, 1 µs interval, origin −2 µs
        let m = MockTcpInstrument::spawn(|cmd| match cmd {
            "*OPC?" => Some("1".into()),
            ":WAV:XINC?" => Some("1.0E-6".into()),
            ":WAV:XOR?" => Some("-2.0E-6".into()),
            ":WAV:DATA?" => Some("0.0,0.5,1.0,0.5,0.0".into()),
            ":READ?" => Some("-10.0".into()),
            ":TRAC? TRACE1" => Some("-90,-90,-20,-90,-90".into()),
            ":CALC:MARK1:X?" => Some("100000".into()),
            ":CALC:MARK1:Y?" => Some("-20".into()),
            _ => None,
        })
        .unwrap();
        let log = MeasurementLog::memory().shared();
        let mut s = Oscilloscope::open(
            &InstrumentProfile::default_for("scope", Role::Oscilloscope, tcp(&m)),
            Some(log.clone()),
        )
        .unwrap();
        s.arm(1).unwrap();
        let w = s.fetch(1).unwrap();
        assert_eq!(w.samples.len(), 5);
        assert!((w.fs_hz - 1e6).abs() < 1e-6 && (w.t0_s + 2e-6).abs() < 1e-15);
        assert!(log
            .lock()
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.command == ":WAV:SOUR CHAN2"));

        let mut pm = PowerMeter::open(
            &InstrumentProfile::default_for("pm", Role::PowerMeter, tcp(&m)),
            None,
        )
        .unwrap();
        assert_eq!(pm.read_dbm(100e3).unwrap(), -10.0);
        pm.cmd.unit = "w".into();
        assert!(pm.read_dbm(100e3).is_err()); // -10 W is not a power

        let mut fc = FrequencyCounter::open(
            &InstrumentProfile::default_for("fc", Role::FrequencyCounter, tcp(&m)),
            None,
        )
        .unwrap();
        assert_eq!(fc.read_hz(1.0).unwrap(), -10.0);

        let mut sa = SpectrumAnalyser::open(
            &InstrumentProfile::default_for("sa", Role::SpectrumAnalyser, tcp(&m)),
            None,
        )
        .unwrap();
        sa.select_input(0).unwrap();
        let t = sa.sweep(80e3, 120e3, 1e3).unwrap();
        assert_eq!(t.freqs_hz[2], 100e3);
        assert_eq!(t.peak_in(0.0, 1e6).unwrap(), (100e3, -20.0));
        assert_eq!(sa.marker_peak().unwrap(), (100e3, -20.0));

        // a profile of the wrong role is refused
        let p = InstrumentProfile::default_for("x", Role::PowerMeter, tcp(&m));
        assert!(TimeIntervalCounter::open(&p, None).is_err());
    }
}
