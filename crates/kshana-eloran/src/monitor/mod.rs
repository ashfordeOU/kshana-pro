// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Live monitor (G1): snapshots of a running simulation or a replayed recording, processed
//! by the real-time receiver ([`crate::rt`]), for a GUI.
//!
//! The monitor is a [`SampleSink`]: feed it blocks (from [`crate::scene::Scene::run`] or a
//! SigMF recording) and every `interval_s` of signal time it emits a [`Snapshot`]:
//!
//! * **spectrum** of the last `spectrum_n` samples (channel 0) over `spectrum_lo..hi_hz`,
//!   dB relative to full scale (Goertzel per bin);
//! * **pulse view**: per tracked station, the received waveform around its latest time of
//!   arrival (`pulse_window_us`, field strength µV/m);
//! * **stations**: latest measurement per station (SNR, ECD, field, bearing, cycle slips,
//!   lock count);
//! * **fix**: the latest real-time position or time solution;
//! * **health**: real-time factor, latency, backlog, peak level, clipping.
//!
//! Snapshots stream as JSON lines (one header line, then one line per snapshot, then a
//! summary line) so a dashboard can follow a run live, and [`html`] renders a
//! self-contained dashboard (map, station table, spectrum, pulse view, health, timeline
//! player; optional live follow of a growing JSON-lines file when served over HTTP).
//! When the truth of a simulation is known, the summary compares the measurements with it.

use crate::chain::Chain;
use crate::geo::Geo;
use crate::rt::{RtConfig, RtFix, RtReceiver};
use crate::rx::{solve::Corrections, Antenna, LoopMode, Measurement, RxConfig};
use crate::scene::{SampleSink, TruthRecord};
use crate::time::AbsTime;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::io::Write;

/// Monitor settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorConfig {
    /// Snapshot interval (s of signal time).
    pub interval_s: f64,
    pub spectrum_lo_hz: f64,
    pub spectrum_hi_hz: f64,
    pub spectrum_bins: usize,
    /// Samples used for the spectrum.
    pub spectrum_n: usize,
    /// Pulse view window around the time of arrival (µs).
    pub pulse_window_us: (f64, f64),
    /// Points per pulse trace (decimated).
    pub pulse_points: usize,
    /// Use crossed loops beam-steered when the scenario has two H-loops.
    pub per_loop: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        MonitorConfig {
            interval_s: 0.5,
            spectrum_lo_hz: 70e3,
            spectrum_hi_hz: 130e3,
            spectrum_bins: 121,
            spectrum_n: 8192,
            pulse_window_us: (-20.0, 280.0),
            pulse_points: 150,
            per_loop: false,
        }
    }
}

/// A station in the header (for the map).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonStation {
    pub chain: String,
    pub id: String,
    pub role: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub power_kw: f64,
}

/// First record of a monitor stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorHeader {
    pub format: String,
    pub version: u32,
    pub source: String,
    pub scenario_sha256: Option<String>,
    pub fs_hz: f64,
    pub duration_s: f64,
    pub channels: usize,
    pub start_utc: String,
    pub stations: Vec<MonStation>,
    /// Receiver position over time (s, lat, lon), when known.
    pub receiver_track: Vec<(f64, f64, f64)>,
    pub spectrum_hz: Vec<f64>,
    pub pulse_us: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationView {
    pub chain: String,
    pub station: String,
    pub channel: Option<usize>,
    pub toa_s: f64,
    pub snr_db: f64,
    pub ecd_us: f64,
    pub field_uv_m: f64,
    pub bearing_deg: Option<f64>,
    pub cycle_slip: bool,
    pub measurements: u64,
    pub cycle_slips: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FixView {
    pub at_s: f64,
    pub kind: String,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    pub hdop: Option<f64>,
    /// Clock biases (position fix) or per-time-reference offsets (time fix), ns.
    pub clock_ns: Vec<(String, f64)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HealthView {
    pub realtime_factor: f64,
    pub latency_s: f64,
    pub backlog_samples: u64,
    pub peak_fs: f64,
    pub clipped: u64,
    pub locks: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PulseTrace {
    pub chain: String,
    pub station: String,
    pub values: Vec<f64>,
}

/// One snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub t_s: f64,
    pub spectrum_db: Vec<f64>,
    pub pulses: Vec<PulseTrace>,
    pub stations: Vec<StationView>,
    pub fix: Option<FixView>,
    pub health: HealthView,
}

/// Truth comparison of one station over the run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationTruth {
    pub chain: String,
    pub station: String,
    pub n: usize,
    pub toa_mean_ns: f64,
    pub toa_rms_ns: f64,
    pub cycle_errors: usize,
}

/// Last record of a monitor stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorSummary {
    pub snapshots: usize,
    pub measurements: usize,
    pub stations_tracked: usize,
    pub truth: Vec<StationTruth>,
    pub final_fix_error_m: Option<f64>,
    pub evidence_label: String,
}

/// A complete monitor result (what [`html`] renders).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MonitorResult {
    pub header: MonitorHeader,
    pub snapshots: Vec<Snapshot>,
    pub summary: MonitorSummary,
}

/// The monitor sink.
pub struct Monitor {
    cfg: MonitorConfig,
    fs: f64,
    rt: RtReceiver,
    ring: Vec<VecDeque<f64>>,
    ring_cap: usize,
    n_total: u64,
    next_snap: f64,
    output_scale: f64,
    peak: f64,
    clipped: u64,
    latest: BTreeMap<(String, String, Option<usize>), Measurement>,
    all: Vec<Measurement>,
    last_fix: Option<FixView>,
    spectrum_hz: Vec<f64>,
    pub header: MonitorHeader,
    pub snapshots: Vec<Snapshot>,
    out: Option<Box<dyn Write + Send>>,
}

impl Monitor {
    /// A monitor for `chains` sampled at `fs_hz`. `outputs_loops` lists the indices of H-loop
    /// channels (two or more → crossed-loop processing; otherwise channel 0 as E-field).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        header: MonitorHeader,
        chains: &[Chain],
        start: AbsTime,
        loops: &[usize],
        output_scale_uv_m: f64,
        corrections: Corrections,
        cfg: MonitorConfig,
        out: Option<Box<dyn Write + Send>>,
    ) -> Result<Monitor, String> {
        if !(cfg.interval_s > 0.0) || cfg.spectrum_bins < 2 || cfg.pulse_points < 2 {
            return Err(
                "monitor: interval, spectrum bins and pulse points must be positive".into(),
            );
        }
        let fs = header.fs_hz;
        let mut rx = RxConfig {
            output_scale_uv_m,
            ..Default::default()
        };
        rx.antenna = if loops.len() >= 2 {
            Antenna::CrossedLoop {
                a: loops[0],
                b: loops[1],
                mode: if cfg.per_loop {
                    LoopMode::PerLoop
                } else {
                    LoopMode::BeamSteer
                },
            }
        } else {
            Antenna::EField { channel: 0 }
        };
        let rtc = RtConfig {
            rx,
            corrections,
            ..Default::default()
        };
        let rt = RtReceiver::new(fs, start, chains, header.channels, rtc)?;
        let n = cfg.spectrum_bins;
        let spectrum_hz: Vec<f64> = (0..n)
            .map(|i| {
                cfg.spectrum_lo_hz
                    + (cfg.spectrum_hi_hz - cfg.spectrum_lo_hz) * i as f64 / (n - 1) as f64
            })
            .collect();
        let mut header = header;
        header.spectrum_hz = spectrum_hz.clone();
        let (a, b) = cfg.pulse_window_us;
        header.pulse_us = (0..cfg.pulse_points)
            .map(|i| a + (b - a) * i as f64 / (cfg.pulse_points - 1) as f64)
            .collect();
        let ring_cap = (2.5 * fs) as usize;
        let mut m = Monitor {
            ring: vec![VecDeque::with_capacity(ring_cap); header.channels.max(1)],
            ring_cap,
            fs,
            rt,
            n_total: 0,
            next_snap: cfg.interval_s,
            output_scale: output_scale_uv_m,
            peak: 0.0,
            clipped: 0,
            latest: BTreeMap::new(),
            all: Vec::new(),
            last_fix: None,
            spectrum_hz,
            header,
            snapshots: Vec::new(),
            out,
            cfg,
        };
        let line =
            serde_json::to_string(&serde_json::json!({"type": "header", "header": m.header}))
                .unwrap();
        m.emit(&line)?;
        Ok(m)
    }

    fn emit(&mut self, line: &str) -> Result<(), String> {
        if let Some(o) = self.out.as_mut() {
            o.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
            o.write_all(b"\n").map_err(|e| e.to_string())?;
            o.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn spectrum(&self) -> Vec<f64> {
        let ch = &self.ring[0];
        let n = self.cfg.spectrum_n.min(ch.len());
        if n < 16 {
            return vec![-200.0; self.spectrum_hz.len()];
        }
        let x: Vec<f64> = ch.iter().skip(ch.len() - n).copied().collect();
        // Hann window, Goertzel per bin
        let w: Vec<f64> = (0..n)
            .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (n - 1) as f64).cos())
            .collect();
        let wsum: f64 = w.iter().sum();
        self.spectrum_hz
            .iter()
            .map(|&f| {
                let k = std::f64::consts::TAU * f / self.fs;
                let coeff = 2.0 * k.cos();
                let (mut s1, mut s2) = (0.0f64, 0.0f64);
                for (xi, wi) in x.iter().zip(&w) {
                    let s0 = xi * wi + coeff * s1 - s2;
                    s2 = s1;
                    s1 = s0;
                }
                let p = s1 * s1 + s2 * s2 - coeff * s1 * s2;
                let amp = 2.0 * p.max(0.0).sqrt() / wsum;
                20.0 * amp.max(1e-10).log10()
            })
            .collect()
    }

    fn pulse(&self, toa_s: f64) -> Option<Vec<f64>> {
        let ch = &self.ring[0];
        let first = self.n_total - ch.len() as u64;
        let (a, b) = self.cfg.pulse_window_us;
        let s0 = ((toa_s + a * 1e-6) * self.fs).round() as i64;
        let s1 = ((toa_s + b * 1e-6) * self.fs).round() as i64;
        if s0 < first as i64 || s1 >= self.n_total as i64 {
            return None;
        }
        let pts = self.cfg.pulse_points;
        Some(
            (0..pts)
                .map(|i| {
                    let s = s0 + ((s1 - s0) as f64 * i as f64 / (pts - 1) as f64).round() as i64;
                    ch[(s as u64 - first) as usize] * self.output_scale
                })
                .collect(),
        )
    }

    fn snapshot(&mut self, t_s: f64) -> Result<(), String> {
        for m in self.rt.drain_measurements() {
            let key = (m.m.chain.clone(), m.m.station.clone(), m.m.channel);
            self.all.push(m.m.clone());
            self.latest.insert(key, m.m);
        }
        for s in self.rt.drain_solutions() {
            self.last_fix = Some(match s.fix {
                RtFix::Position(p) => FixView {
                    at_s: s.at_s,
                    kind: "position".into(),
                    lat_deg: Some(p.position.lat_deg),
                    lon_deg: Some(p.position.lon_deg),
                    hdop: Some(p.hdop),
                    clock_ns: p.clock_bias_ns,
                },
                RtFix::Time(t) => FixView {
                    at_s: s.at_s,
                    kind: "time".into(),
                    lat_deg: None,
                    lon_deg: None,
                    hdop: None,
                    clock_ns: t
                        .by_time_ref
                        .iter()
                        .map(|(r, v, _)| (r.clone(), *v))
                        .collect(),
                },
            });
        }
        let st = self.rt.status();
        let lock = |c: &str, s: &str, ch: Option<usize>| {
            st.locks
                .iter()
                .find(|l| l.chain == c && l.station == s && l.channel == ch)
                .map(|l| (l.measurements, l.cycle_slips))
                .unwrap_or((0, 0))
        };
        let stations: Vec<StationView> = self
            .latest
            .values()
            .map(|m| {
                let (n, slips) = lock(&m.chain, &m.station, m.channel);
                StationView {
                    chain: m.chain.clone(),
                    station: m.station.clone(),
                    channel: m.channel,
                    toa_s: m.toa_s,
                    snr_db: m.snr_db,
                    ecd_us: m.ecd_us,
                    field_uv_m: m.field_uv_m,
                    bearing_deg: m.bearing_deg,
                    cycle_slip: m.cycle_slip,
                    measurements: n,
                    cycle_slips: slips,
                }
            })
            .collect();
        let pulses: Vec<PulseTrace> = self
            .latest
            .values()
            .filter(|m| m.channel.is_none() || m.channel == Some(0))
            .filter_map(|m| {
                self.pulse(m.toa_s).map(|v| PulseTrace {
                    chain: m.chain.clone(),
                    station: m.station.clone(),
                    values: v,
                })
            })
            .collect();
        let snap = Snapshot {
            t_s,
            spectrum_db: self.spectrum(),
            pulses,
            stations,
            fix: self.last_fix.clone(),
            health: HealthView {
                realtime_factor: st.realtime_factor,
                latency_s: st.latency_s,
                backlog_samples: st.backlog_samples,
                peak_fs: self.peak,
                clipped: self.clipped,
                locks: st.locks.len(),
            },
        };
        let line =
            serde_json::to_string(&serde_json::json!({"type": "snapshot", "snapshot": snap}))
                .unwrap();
        self.emit(&line)?;
        self.snapshots.push(snap);
        Ok(())
    }

    /// Finish: flush the receiver, take a last snapshot, compare with `truth` (if any) and
    /// write the summary line.
    pub fn finish_with_truth(
        mut self,
        truth: &[TruthRecord],
        final_rx: Option<Geo>,
    ) -> Result<MonitorResult, String> {
        self.rt.flush()?;
        let t = self.n_total as f64 / self.fs;
        self.snapshot(t)?;
        let mut by: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
        for m in &self.all {
            if let Some(r) = truth
                .iter()
                .find(|r| r.chain == m.chain && r.station == m.station && r.group == m.group)
            {
                by.entry((m.chain.clone(), m.station.clone()))
                    .or_default()
                    .push((m.toa_s - r.toa_s) * 1e9);
            }
        }
        let truth_rows: Vec<StationTruth> = by
            .into_iter()
            .map(|((c, s), e)| {
                let n = e.len();
                let mean = e.iter().sum::<f64>() / n as f64;
                let rms = (e.iter().map(|x| x * x).sum::<f64>() / n as f64).sqrt();
                StationTruth {
                    chain: c,
                    station: s,
                    n,
                    toa_mean_ns: mean,
                    toa_rms_ns: rms,
                    cycle_errors: e.iter().filter(|x| x.abs() > 2500.0).count(),
                }
            })
            .collect();
        let final_fix_error_m = match (&self.last_fix, final_rx) {
            (
                Some(FixView {
                    lat_deg: Some(la),
                    lon_deg: Some(lo),
                    ..
                }),
                Some(p),
            ) => Some(crate::geo::inverse(Geo::new(*la, *lo), p).distance_m),
            _ => None,
        };
        let stations_tracked = self
            .all
            .iter()
            .map(|m| (m.chain.clone(), m.station.clone()))
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let summary = MonitorSummary {
            snapshots: self.snapshots.len(),
            measurements: self.all.len(),
            stations_tracked,
            truth: truth_rows,
            final_fix_error_m,
            evidence_label: "MODELLED".into(),
        };
        let line =
            serde_json::to_string(&serde_json::json!({"type": "summary", "summary": summary}))
                .unwrap();
        self.emit(&line)?;
        Ok(MonitorResult {
            header: self.header,
            snapshots: self.snapshots,
            summary,
        })
    }
}

impl SampleSink for Monitor {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        let n = channels.first().map(|c| c.len()).unwrap_or(0);
        for (r, c) in self.ring.iter_mut().zip(channels) {
            for &v in c {
                let a = v.abs();
                if a > self.peak {
                    self.peak = a;
                }
                if a > 1.0 {
                    self.clipped += 1;
                }
                if r.len() == self.ring_cap {
                    r.pop_front();
                }
                r.push_back(v);
            }
        }
        self.n_total += n as u64;
        self.rt.push_block(channels)?;
        let t = self.n_total as f64 / self.fs;
        while t >= self.next_snap {
            let ts = self.next_snap;
            self.snapshot(ts)?;
            self.next_snap += self.cfg.interval_s;
        }
        Ok(())
    }
}

fn header_for(
    sc: &crate::Scenario,
    source: &str,
    fs: f64,
    channels: usize,
    duration_s: f64,
) -> MonitorHeader {
    let stations = sc
        .chains
        .iter()
        .flat_map(|c| {
            c.chain.stations.iter().map(move |s| MonStation {
                chain: c.chain.name.clone(),
                id: s.id.clone(),
                role: format!("{:?}", s.role).to_lowercase(),
                lat_deg: s.position.lat_deg,
                lon_deg: s.position.lon_deg,
                power_kw: s.power_kw,
            })
        })
        .collect();
    let steps = 50usize;
    let receiver_track = (0..=steps)
        .map(|i| {
            let t = duration_s * i as f64 / steps as f64;
            let st = sc.receiver.state(t);
            (t, st.pos.lat_deg, st.pos.lon_deg)
        })
        .collect();
    MonitorHeader {
        format: "kshana-eloran-monitor".into(),
        version: 1,
        source: source.into(),
        scenario_sha256: Some(sc.hash()),
        fs_hz: fs,
        duration_s,
        channels,
        start_utc: sc.scene.start_utc.clone(),
        stations,
        receiver_track,
        spectrum_hz: vec![],
        pulse_us: vec![],
    }
}

fn loops_of(sc: &crate::Scenario) -> Vec<usize> {
    sc.outputs
        .iter()
        .enumerate()
        .filter(|(_, o)| matches!(o, crate::antenna::OutputChannel::HLoop { .. }))
        .map(|(i, _)| i)
        .collect()
}

/// Monitor a simulation of `sc` (the scene is generated block by block and processed live).
pub fn monitor_scenario(
    sc: &crate::Scenario,
    cfg: MonitorConfig,
    out: Option<Box<dyn Write + Send>>,
) -> Result<MonitorResult, String> {
    let mut scene = sc.build()?;
    let p = &sc.scene.params;
    let header = header_for(sc, "simulation", p.fs_hz, sc.outputs.len(), p.duration_s);
    let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let mut mon = Monitor::new(
        header,
        &chains,
        sc.start()?,
        &loops_of(sc),
        p.output_scale_uv_m,
        Corrections::default(),
        cfg,
        out,
    )?;
    let mut truth = Vec::new();
    scene.run(&mut mon, &mut truth)?;
    let final_rx = sc.receiver.state(p.duration_s).pos;
    mon.finish_with_truth(&truth, Some(final_rx))
}

/// Monitor a SigMF recording (the scenario supplies chains, start, antennas and scale).
pub fn monitor_recording(
    sc: &crate::Scenario,
    base: &std::path::Path,
    cfg: MonitorConfig,
    out: Option<Box<dyn Write + Send>>,
) -> Result<MonitorResult, String> {
    let mut rd = crate::replay::reader::SigmfReader::open(base)?;
    let dur = rd.duration_s();
    let header = header_for(
        sc,
        &format!("recording {}", base.display()),
        rd.fs_hz,
        rd.channels,
        dur,
    );
    let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
    let mut mon = Monitor::new(
        header,
        &chains,
        sc.start()?,
        &loops_of(sc),
        sc.scene.params.output_scale_uv_m,
        Corrections::default(),
        cfg,
        out,
    )?;
    let block = ((0.01 * rd.fs_hz) as usize).max(1);
    loop {
        let b = rd.read(block)?;
        if b.first().map(|c| c.is_empty()).unwrap_or(true) {
            break;
        }
        mon.write_block(&b)?;
    }
    mon.finish_with_truth(&[], None)
}

pub mod html;
pub use html::html;

#[cfg(test)]
mod tests {
    use super::*;

    const SCN: &str = include_str!("../../scenarios/rx-cri.toml");

    #[test]
    fn monitor_tracks_stations_and_streams_json_lines() {
        let mut sc = crate::Scenario::from_toml(SCN).unwrap();
        sc.scene.params.duration_s = 2.0;
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        struct W(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for W {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let r = monitor_scenario(
            &sc,
            MonitorConfig::default(),
            Some(Box::new(W(buf.clone()))),
        )
        .unwrap();
        assert!(r.snapshots.len() >= 4, "{}", r.snapshots.len());
        assert!(r.summary.stations_tracked >= 4, "{:?}", r.summary);
        assert!(
            r.summary
                .truth
                .iter()
                .all(|t| t.cycle_errors == 0 && t.toa_rms_ns < 50.0),
            "{:?}",
            r.summary.truth
        );
        let last = r.snapshots.last().unwrap();
        assert_eq!(last.spectrum_db.len(), r.header.spectrum_hz.len());
        // the spectrum peaks near the 100 kHz carrier
        let (imax, _) = last
            .spectrum_db
            .iter()
            .enumerate()
            .fold(
                (0, f64::MIN),
                |a, (i, v)| if *v > a.1 { (i, *v) } else { a },
            );
        assert!(
            (r.header.spectrum_hz[imax] - 100e3).abs() <= 5e3,
            "peak at {}",
            r.header.spectrum_hz[imax]
        );
        assert!(!last.pulses.is_empty());
        assert_eq!(last.pulses[0].values.len(), r.header.pulse_us.len());
        let text = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("\"header\""));
        assert!(lines.last().unwrap().contains("\"summary\""));
        assert_eq!(lines.len(), r.snapshots.len() + 2);
        let page = html(&r);
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("kshana-eloran-monitor"));
    }
}
