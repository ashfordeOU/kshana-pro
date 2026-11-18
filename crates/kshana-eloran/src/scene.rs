// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The synthesis engine: N chains × M stations × P paths → K real output channels.
//!
//! The scene runs block by block (default 10 ms). For each block it takes the receiver
//! state, asks the propagation model for every station's paths, lists the pulses whose
//! arrival overlaps the block, and **evaluates each pulse analytically at every sample
//! instant**. The time of each sample relative to a pulse's arrival is computed exactly in
//! integer arithmetic (`n·10¹² − T_ps·fs`, so the sample rate must be a whole number of hertz),
//! which makes delay resolution independent of the sample rate. Cross-rate interference
//! needs no special handling: every chain's pulses are summed. Interference and noise
//! are added after the pulses. The engine also writes a truth record per station per group
//! (time of arrival, delay components, field, SNR, time-reference offset).

use crate::antenna::OutputChannel;
use crate::chain::{emissions, Chain};
use crate::interf::Interferer;
use crate::prop::{PathKind, PropagationModel};
use crate::signal::GroupModulator;
use crate::time::{AbsTime, Ps, PS_PER_S};
use crate::timebase::Timebase;
use crate::traj::Trajectory;
use serde::{Deserialize, Serialize};

/// Destination of output samples (field strength in output units after scaling).
pub trait SampleSink {
    /// One block: `channels[c][n]`.
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String>;
    fn finish(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// In-memory sink.
#[derive(Default, Debug, Clone)]
pub struct MemSink {
    pub channels: Vec<Vec<f64>>,
}

impl SampleSink for MemSink {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        if self.channels.is_empty() {
            self.channels = vec![Vec::new(); channels.len()];
        }
        for (d, s) in self.channels.iter_mut().zip(channels) {
            d.extend_from_slice(s);
        }
        Ok(())
    }
}

/// A sink that discards samples (for truth-only runs and benchmarks).
#[derive(Default, Debug)]
pub struct NullSink {
    pub samples: u64,
}

impl SampleSink for NullSink {
    fn write_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        self.samples += channels.first().map(|c| c.len()).unwrap_or(0) as u64;
        Ok(())
    }
}

/// Truth of one station's group as received (ground path).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TruthRecord {
    /// Scene time of the true emission of pulse 0 (s).
    pub t_emit_s: f64,
    pub chain: String,
    pub station: String,
    pub group: i64,
    /// Scene time of arrival of pulse 0's time origin via the groundwave (s).
    pub toa_s: f64,
    /// Total groundwave delay and its parts (ns).
    pub delay_ns: f64,
    pub geom_ns: f64,
    pub pf_ns: f64,
    pub sf_ns: f64,
    pub asf_ns: f64,
    /// ECD at the receiver (transmitted + propagation), µs.
    pub ecd_us: f64,
    /// Groundwave RMS field at pulse peak (µV/m) and its SNR in 20 kHz (dB).
    pub field_uv_m: f64,
    pub snr_db: f64,
    /// Skywave: delay after the groundwave (µs) and sky/ground ratio (dB), if present.
    pub sky_delay_us: Option<f64>,
    pub sky_ratio_db: Option<f64>,
    /// Offset of the station's time reference from UTC at emission (ns), and the
    /// emission error (true − nominal, ns) including transmitter faults.
    pub time_ref_offset_ns: f64,
    pub emission_error_ns: f64,
    /// Receiver position and heading at arrival.
    pub rx_lat_deg: f64,
    pub rx_lon_deg: f64,
    pub rx_heading_deg: f64,
    pub bearing_deg: f64,
    pub distance_km: f64,
}

/// Parameters of a scene run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneParams {
    /// Sample rate (Hz), a whole number, ≥ 400 kHz.
    pub fs_hz: f64,
    pub duration_s: f64,
    /// Block length (s).
    #[serde(default = "d_block")]
    pub block_s: f64,
    /// Field (µV/m) that maps to output value 1.0.
    #[serde(default = "d_scale")]
    pub output_scale_uv_m: f64,
    #[serde(default)]
    pub seed: u64,
}

fn d_block() -> f64 {
    0.01
}
fn d_scale() -> f64 {
    1.0e5
}

impl SceneParams {
    pub fn validate(&self) -> Result<(), String> {
        if self.fs_hz.fract() != 0.0 || self.fs_hz < 400_000.0 || self.fs_hz > 100e6 {
            return Err(format!("fs_hz {} must be a whole number of Hz in 400 kHz..100 MHz", self.fs_hz));
        }
        if !(self.duration_s > 0.0) || !(self.block_s > 0.0) {
            return Err("duration_s and block_s must be positive".into());
        }
        if !(self.output_scale_uv_m > 0.0) {
            return Err("output_scale_uv_m must be positive".into());
        }
        Ok(())
    }
}

/// Run statistics.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunStats {
    pub samples_per_channel: u64,
    pub channels: usize,
    pub pulses: u64,
    pub truth_records: u64,
    pub clipped_samples: u64,
    pub peak_abs: f64,
    pub wall_s: f64,
    /// Simulated seconds per wall second (≥ 1 means real time on this host).
    pub realtime_factor: f64,
}

/// A configured scene.
pub struct Scene {
    pub params: SceneParams,
    pub start: AbsTime,
    pub chains: Vec<Chain>,
    pub modulators: Vec<Option<Box<dyn GroupModulator>>>,
    pub timebase: Timebase,
    pub prop: Box<dyn PropagationModel>,
    pub trajectory: Trajectory,
    pub outputs: Vec<OutputChannel>,
    pub interferers: Vec<Box<dyn Interferer>>,
    /// RMS noise in 20 kHz (µV/m), for the SNR in the truth.
    pub noise_rms_20k: f64,
}

impl Scene {
    pub fn validate(&self) -> Result<(), String> {
        self.params.validate()?;
        self.trajectory.validate()?;
        if self.outputs.is_empty() {
            return Err("at least one output channel".into());
        }
        for c in &self.chains {
            c.validate()?;
        }
        if self.modulators.len() != self.chains.len() {
            return Err("one modulator slot per chain".into());
        }
        Ok(())
    }

    /// Run the scene into `sink`, appending truth to `truth`.
    pub fn run(&mut self, sink: &mut dyn SampleSink, truth: &mut Vec<TruthRecord>) -> Result<RunStats, String> {
        self.validate()?;
        let wall0 = std::time::Instant::now();
        let fs = self.params.fs_hz;
        let fs_i = fs as i128;
        let n_total = (self.params.duration_s * fs).round() as u64;
        let block_n = ((self.params.block_s * fs).round() as u64).max(1);
        let n_ch = self.outputs.len();
        let scale = 1.0 / self.params.output_scale_uv_m;
        let mut stats = RunStats { channels: n_ch, ..Default::default() };
        let mut bufs: Vec<Vec<f64>> = vec![Vec::new(); n_ch];
        let mut n0: u64 = 0;
        while n0 < n_total {
            let n = block_n.min(n_total - n0);
            for b in bufs.iter_mut() {
                b.clear();
                b.resize(n as usize, 0.0);
            }
            let t_a = Ps(((n0 as i128 * PS_PER_S as i128) / fs_i) as i64);
            let t_b = Ps((((n0 + n) as i128 * PS_PER_S as i128) / fs_i) as i64);
            let rx = self.trajectory.state((t_a.as_s() + t_b.as_s()) / 2.0);
            for (ci, chain) in self.chains.iter().enumerate() {
                let extent = chain.signal.envelope.extent_s();
                // paths per station for this block
                let paths: Vec<_> = chain.stations.iter().map(|s| self.prop.paths(s, &rx)).collect();
                let bearings: Vec<_> = chain
                    .stations
                    .iter()
                    .map(|s| crate::geo::inverse(rx.pos, s.position))
                    .collect();
                let max_delay = paths
                    .iter()
                    .flatten()
                    .map(|p| p.delay)
                    .max()
                    .unwrap_or(Ps::ZERO);
                let lookback = max_delay + Ps::from_s(extent) + Ps::from_us(50.0);
                let em = emissions(
                    chain,
                    ci,
                    self.start,
                    t_a - lookback,
                    t_b,
                    &self.timebase,
                    self.modulators[ci].as_deref(),
                );
                for e in &em {
                    let st_paths = &paths[e.station];
                    let br = bearings[e.station].az1_rad;
                    let gains: Vec<f64> = self.outputs.iter().map(|o| o.gain(br, rx.heading_rad)).collect();
                    let mut counted = false;
                    for p in st_paths {
                        let arr = e.t_emit + p.delay;
                        let arr_end = arr + Ps::from_s(extent);
                        if arr_end <= t_a || arr >= t_b {
                            continue;
                        }
                        counted = true;
                        // first and last sample indices (global) within the pulse and block
                        let first = ceil_div(arr.0 as i128 * fs_i, PS_PER_S as i128).max(n0 as i128) as u64;
                        let last = floor_div(arr_end.0 as i128 * fs_i, PS_PER_S as i128).min((n0 + n - 1) as i128);
                        if (first as i128) > last {
                            continue;
                        }
                        let amp = p.field_uv_m * std::f64::consts::SQRT_2 * e.amp_scale * e.pulse.amp * scale;
                        let ecd = e.ecd_s + p.ecd_s;
                        let phase = e.pulse.phase_rad + p.phase_rad;
                        let denom = fs * PS_PER_S as f64;
                        for gn in first..=(last as u64) {
                            let num = gn as i128 * PS_PER_S as i128 - arr.0 as i128 * fs_i;
                            let tau = num as f64 / denom;
                            let v = amp * chain.signal.pulse(tau, ecd, phase);
                            if v != 0.0 {
                                let idx = (gn - n0) as usize;
                                for (c, g) in gains.iter().enumerate() {
                                    bufs[c][idx] += v * g;
                                }
                            }
                        }
                        // truth: pulse 0, ground path, once
                        if e.pulse.k == 0 && p.kind == PathKind::Ground && arr >= t_a && arr < t_b {
                            let st = &chain.stations[e.station];
                            let sky = st_paths.iter().find(|q| matches!(q.kind, PathKind::Sky { .. }));
                            truth.push(TruthRecord {
                                t_emit_s: e.t_emit.as_s(),
                                chain: chain.name.clone(),
                                station: st.id.clone(),
                                group: e.group,
                                toa_s: arr.as_s(),
                                delay_ns: p.delay.as_ns(),
                                geom_ns: p.geom_ns,
                                pf_ns: p.pf_ns,
                                sf_ns: p.sf_ns,
                                asf_ns: p.asf_ns,
                                ecd_us: ecd * 1e6,
                                field_uv_m: p.field_uv_m * e.amp_scale,
                                snr_db: if self.noise_rms_20k > 0.0 {
                                    20.0 * (p.field_uv_m * e.amp_scale / self.noise_rms_20k).log10()
                                } else {
                                    f64::INFINITY
                                },
                                sky_delay_us: sky.map(|q| (q.delay - p.delay).as_us()),
                                sky_ratio_db: sky.map(|q| 20.0 * (q.field_uv_m / p.field_uv_m).log10()),
                                time_ref_offset_ns: self.timebase.offset_s(&st.time_ref, e.t_nominal.as_s()) * 1e9,
                                emission_error_ns: (e.t_emit - e.t_nominal).as_ns(),
                                rx_lat_deg: rx.pos.lat_deg,
                                rx_lon_deg: rx.pos.lon_deg,
                                rx_heading_deg: rx.heading_rad.to_degrees(),
                                bearing_deg: br.to_degrees(),
                                distance_km: bearings[e.station].distance_m / 1000.0,
                            });
                            stats.truth_records += 1;
                        }
                    }
                    if counted {
                        stats.pulses += 1;
                    }
                }
            }
            for (c, b) in bufs.iter_mut().enumerate() {
                if !self.interferers.is_empty() {
                    let mut add = vec![0.0; b.len()];
                    for i in self.interferers.iter_mut() {
                        i.add(t_a, fs, c, &mut add);
                    }
                    for (x, a) in b.iter_mut().zip(&add) {
                        *x += a * scale;
                    }
                }
                for x in b.iter() {
                    let a = x.abs();
                    if a > stats.peak_abs {
                        stats.peak_abs = a;
                    }
                    if a > 1.0 {
                        stats.clipped_samples += 1;
                    }
                }
            }
            sink.write_block(&bufs)?;
            n0 += n;
        }
        sink.finish()?;
        stats.samples_per_channel = n_total;
        stats.wall_s = wall0.elapsed().as_secs_f64();
        stats.realtime_factor = if stats.wall_s > 0.0 { self.params.duration_s / stats.wall_s } else { f64::INFINITY };
        truth.sort_by(|a, b| a.toa_s.partial_cmp(&b.toa_s).unwrap_or(std::cmp::Ordering::Equal));
        Ok(stats)
    }
}

fn ceil_div(a: i128, b: i128) -> i128 {
    -((-a).div_euclid(b))
}
fn floor_div(a: i128, b: i128) -> i128 {
    a.div_euclid(b)
}

/// Write truth records as CSV.
pub fn truth_csv(records: &[TruthRecord]) -> String {
    let mut s = String::from(
        "t_emit_s,chain,station,group,toa_s,delay_ns,geom_ns,pf_ns,sf_ns,asf_ns,ecd_us,field_uv_m,snr_db,sky_delay_us,sky_ratio_db,time_ref_offset_ns,emission_error_ns,rx_lat_deg,rx_lon_deg,rx_heading_deg,bearing_deg,distance_km\n",
    );
    let o = |v: Option<f64>| v.map(|x| format!("{x:.6}")).unwrap_or_default();
    for r in records {
        s.push_str(&format!(
            "{:.12},{},{},{},{:.12},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:.2},{},{},{:.3},{:.3},{:.8},{:.8},{:.3},{:.4},{:.3}\n",
            r.t_emit_s, r.chain, r.station, r.group, r.toa_s, r.delay_ns, r.geom_ns, r.pf_ns, r.sf_ns, r.asf_ns,
            r.ecd_us, r.field_uv_m, r.snr_db, o(r.sky_delay_us), o(r.sky_ratio_db), r.time_ref_offset_ns,
            r.emission_error_ns, r.rx_lat_deg, r.rx_lon_deg, r.rx_heading_deg, r.bearing_deg, r.distance_km
        ));
    }
    s
}

/// Read truth records written by [`truth_csv`].
pub fn truth_from_csv(text: &str) -> Result<Vec<TruthRecord>, String> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split(',').collect();
        if f.len() != 22 {
            return Err(format!("truth line {}: expected 22 fields, got {}", i + 1, f.len()));
        }
        let num = |k: usize| -> Result<f64, String> {
            f[k].parse::<f64>().map_err(|e| format!("truth line {} field {}: {e}", i + 1, k + 1))
        };
        let opt = |k: usize| -> Result<Option<f64>, String> {
            if f[k].is_empty() {
                Ok(None)
            } else {
                num(k).map(Some)
            }
        };
        out.push(TruthRecord {
            t_emit_s: num(0)?,
            chain: f[1].to_string(),
            station: f[2].to_string(),
            group: f[3].parse().map_err(|e| format!("truth line {}: {e}", i + 1))?,
            toa_s: num(4)?,
            delay_ns: num(5)?,
            geom_ns: num(6)?,
            pf_ns: num(7)?,
            sf_ns: num(8)?,
            asf_ns: num(9)?,
            ecd_us: num(10)?,
            field_uv_m: num(11)?,
            snr_db: if f[12] == "inf" { f64::INFINITY } else { num(12)? },
            sky_delay_us: opt(13)?,
            sky_ratio_db: opt(14)?,
            time_ref_offset_ns: num(15)?,
            emission_error_ns: num(16)?,
            rx_lat_deg: num(17)?,
            rx_lon_deg: num(18)?,
            rx_heading_deg: num(19)?,
            bearing_deg: num(20)?,
            distance_km: num(21)?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod truth_io_tests {
    use super::*;

    #[test]
    fn truth_csv_round_trips() {
        let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios/synthetic-two-chain.toml")).unwrap();
        let (_s, truth, _st) = crate::run_to_memory(&src).unwrap();
        let back = truth_from_csv(&truth_csv(&truth)).unwrap();
        assert_eq!(back.len(), truth.len());
        assert_eq!(back[0].station, truth[0].station);
        assert!((back[0].toa_s - truth[0].toa_s).abs() < 1e-11);
    }
}
