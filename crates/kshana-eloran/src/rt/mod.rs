// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Real-time streaming software eLoran receiver (stream F1).
//!
//! [`RtReceiver`] turns the block (batch) receiver of [`crate::rx`] into a continuous,
//! bounded-memory processor that consumes sample blocks incrementally from *any* source — a
//! [`crate::hw::Device`] RX, a file/SigMF reader ([`crate::replay`]) or memory — and emits a
//! stream of time-tagged [`crate::rx::Measurement`]s and periodic position/time solutions.
//!
//! # How it stays equal to the batch receiver
//!
//! The receiver keeps a sliding window of samples. Each processing step runs the *existing*
//! batch estimator ([`crate::rx::Receiver`]) over a window of `stride_s + 2·margin_s`
//! seconds, but only emits the measurements whose reference group falls in the window's
//! **core** `[cursor, cursor + stride_s)`. The `margin_s` overlap on each side guarantees
//! every core pulse is fitted with its full surrounding context (front-end filter group
//! delay, the fit window and whole tracking windows), so the per-group fit is identical to
//! the one the whole-recording batch run would produce — only the book-keeping is
//! incremental. Group numbers are absolute (computed from the scene epoch), so emission is
//! exactly-once and de-duplication across windows is trivial.
//!
//! # Continuity, re-acquisition and loss
//!
//! Each window re-acquires, so a station that turns on late (after an `Off` fault) is picked
//! up as soon as it appears, and one that is lost simply stops producing measurements until
//! it returns — the closed-loop behaviour asked for, achieved by re-using acquisition per
//! window rather than a bespoke tracking loop. Cycle slips are flagged two ways: the batch
//! within-window carrier-continuity check ([`crate::rx::Measurement::cycle_slip`]), and a
//! cross-window check here that compares each station's new carrier time against the one
//! predicted from its previous emitted measurement (GRI motion), raising the flag on a jump
//! larger than a quarter carrier period.
//!
//! # Memory and latency
//!
//! Only `stride_s + 2·margin_s` of samples are held at a time (older samples are trimmed
//! behind the cursor), plus a bounded ring of recent measurements for the solutions. The
//! [`StatusSnapshot`] reports the unprocessed backlog (its latency), per-station lock state,
//! processed span and processing wall time.

use crate::chain::Chain;
use crate::geo::Geo;
use crate::rx::{
    position_solution, time_solution, Corrections, Measurement, PositionConfig, PositionSolution,
    Receiver, RxConfig, RxInput, TimeSolution,
};
use crate::time::{AbsTime, Ps, PS_PER_S};
use std::collections::{HashMap, VecDeque};

/// Configuration of the streaming receiver.
#[derive(Clone, Debug)]
pub struct RtConfig {
    /// The underlying batch-receiver configuration (band, antenna, track window, CRI …).
    pub rx: RxConfig,
    /// Core advance per processing step (s). A smaller stride lowers latency at more cost.
    pub stride_s: f64,
    /// Overlap kept on each side of the core (s). `None` picks a safe value from the chains
    /// (the longest tracking window plus the fit window and filter span).
    pub margin_s: Option<f64>,
    /// Emit a position/time solution at most this often (scene s).
    pub solution_every_s: f64,
    /// Produce range-domain position+clock solutions. When `false` (or no position can be
    /// solved) and `known_position` is set, a time solution is produced instead.
    pub position: bool,
    /// Known receiver position for the time solution (and the position-solution initial
    /// guess).
    pub known_position: Option<Geo>,
    /// Propagation corrections for the solutions.
    pub corrections: Corrections,
    /// Bounded number of recent measurements kept for the solutions.
    pub keep_measurements: usize,
    /// Only measurements of the last `solution_window_s` feed each solution (0 = use all
    /// kept). For a moving receiver keep this near one epoch.
    pub solution_window_s: f64,
}

impl Default for RtConfig {
    fn default() -> Self {
        RtConfig {
            rx: RxConfig::default(),
            stride_s: 0.4,
            margin_s: None,
            solution_every_s: 1.0,
            position: true,
            known_position: None,
            corrections: Corrections::default(),
            keep_measurements: 4096,
            solution_window_s: 0.0,
        }
    }
}

/// A measurement with its scene-absolute timing and any cross-window cycle slip.
#[derive(Clone, Debug)]
pub struct RtMeasurement {
    /// The measurement, with `toa_s`/`envelope_toa_s` relative to the scene start.
    pub m: Measurement,
    /// A carrier-time jump versus the station's previous emitted measurement was seen.
    pub cross_window_slip: bool,
}

/// What a periodic solution produced.
#[derive(Clone, Debug)]
pub enum RtFix {
    Position(PositionSolution),
    Time(TimeSolution),
}

/// A periodic solution tagged with the scene time it covers up to.
#[derive(Clone, Debug)]
pub struct RtSolution {
    /// Scene time (s) up to which measurements were used.
    pub at_s: f64,
    pub n_measurements: usize,
    pub fix: RtFix,
}

/// Per-station lock state in the status snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct StationLock {
    pub chain: String,
    pub station: String,
    pub channel: Option<usize>,
    /// Scene time of the last emitted measurement (s).
    pub last_toa_s: f64,
    pub last_group: i64,
    pub measurements: u64,
    pub cycle_slips: u64,
}

/// A snapshot of the receiver's state.
#[derive(Clone, Debug)]
pub struct StatusSnapshot {
    /// Samples pushed in, samples whose core has been processed.
    pub samples_in: u64,
    pub samples_processed: u64,
    /// Unprocessed backlog (samples) and the latency it represents (s).
    pub backlog_samples: u64,
    pub latency_s: f64,
    /// Cumulative processing wall time (s) and the real-time factor over it.
    pub processing_wall_s: f64,
    pub realtime_factor: f64,
    pub windows_processed: u64,
    pub measurements_emitted: u64,
    pub buffered_samples: usize,
    pub locks: Vec<StationLock>,
}

/// The streaming receiver.
pub struct RtReceiver {
    cfg: RtConfig,
    chains: Vec<Chain>,
    fs: f64,
    scene_start: AbsTime,
    n_ch: usize,
    /// Buffered samples per channel; `n_base` is the absolute sample index of `buf[*][0]`.
    buf: Vec<VecDeque<f64>>,
    n_base: u64,
    /// Absolute index of the next core start.
    cursor: u64,
    stride_n: u64,
    margin_n: u64,
    /// Recent measurements (scene-absolute toa), bounded.
    recent: VecDeque<Measurement>,
    /// Per station: (last group, last carrier toa scene s).
    last: HashMap<(usize, usize, Option<usize>), (i64, f64)>,
    locks: HashMap<(usize, usize, Option<usize>), StationLock>,
    out_meas: Vec<RtMeasurement>,
    out_sol: Vec<RtSolution>,
    next_solution_s: f64,
    // stats
    samples_in: u64,
    windows: u64,
    emitted: u64,
    wall_s: f64,
}

impl RtReceiver {
    /// Build a streaming receiver. Validates the chains and the batch configuration once.
    pub fn new(
        fs_hz: f64,
        scene_start: AbsTime,
        chains: &[Chain],
        n_channels: usize,
        cfg: RtConfig,
    ) -> Result<RtReceiver, String> {
        if n_channels == 0 {
            return Err("rt: need at least one channel".into());
        }
        if !(cfg.stride_s > 0.0) {
            return Err("rt: stride_s must be positive".into());
        }
        for c in chains {
            c.validate()?;
        }
        // A dummy one-sample input validates the batch config (antenna channels, FIR, LDC).
        let probe = RxInput::new(vec![vec![0.0]; n_channels], fs_hz, scene_start);
        Receiver::new(&probe, chains, cfg.rx.clone())?;

        let max_gri_s = chains
            .iter()
            .map(|c| c.gri_us as f64 * 1e-6)
            .fold(0.0, f64::max);
        let fit = cfg.rx.fit_window_us.1.max(cfg.rx.envelope_search_us) * 1e-6;
        // margin: a full tracking window plus the fit window and a slack for the filter.
        let auto_margin = cfg.rx.track_gris as f64 * max_gri_s + fit + 2e-3;
        let margin_s = cfg.margin_s.unwrap_or(auto_margin).max(auto_margin);
        let stride_n = (cfg.stride_s * fs_hz).round().max(1.0) as u64;
        let margin_n = (margin_s * fs_hz).round().max(1.0) as u64;
        Ok(RtReceiver {
            n_ch: n_channels,
            buf: (0..n_channels).map(|_| VecDeque::new()).collect(),
            n_base: 0,
            cursor: 0,
            stride_n,
            margin_n,
            recent: VecDeque::new(),
            last: HashMap::new(),
            locks: HashMap::new(),
            out_meas: Vec::new(),
            out_sol: Vec::new(),
            next_solution_s: cfg.solution_every_s,
            samples_in: 0,
            windows: 0,
            emitted: 0,
            wall_s: 0.0,
            cfg,
            chains: chains.to_vec(),
            fs: fs_hz,
            scene_start,
        })
    }

    /// Feed one block (`channels[c][n]`); processes any windows that are now complete.
    pub fn push_block(&mut self, channels: &[Vec<f64>]) -> Result<(), String> {
        if channels.len() != self.n_ch {
            return Err(format!(
                "rt: block has {} channels, expected {}",
                channels.len(),
                self.n_ch
            ));
        }
        let n = channels.first().map(|c| c.len()).unwrap_or(0);
        for (d, s) in self.buf.iter_mut().zip(channels) {
            d.extend(s.iter().copied());
        }
        self.samples_in += n as u64;
        self.process(false)?;
        Ok(())
    }

    /// Process the final partial window (call once the source is exhausted).
    pub fn flush(&mut self) -> Result<(), String> {
        self.process(true)
    }

    fn available(&self) -> u64 {
        self.n_base + self.buf.first().map(|b| b.len() as u64).unwrap_or(0)
    }

    fn process(&mut self, final_flush: bool) -> Result<(), String> {
        loop {
            let avail = self.available();
            let want = self.cursor + self.stride_n + self.margin_n;
            if !final_flush && avail < want {
                break;
            }
            // Nothing left to do on a flush once the core is caught up.
            if final_flush && self.cursor >= avail {
                break;
            }
            let t0 = std::time::Instant::now();
            let lo = self.cursor.saturating_sub(self.margin_n).max(self.n_base);
            let hi = (self.cursor + self.stride_n + self.margin_n).min(avail);
            let core_hi = if final_flush {
                avail
            } else {
                self.cursor + self.stride_n
            };
            self.run_window(lo, hi, self.cursor, core_hi)?;
            self.windows += 1;
            // advance and trim
            self.cursor = core_hi;
            let keep_from = self.cursor.saturating_sub(self.margin_n).max(self.n_base);
            let drop = (keep_from - self.n_base) as usize;
            if drop > 0 {
                for b in self.buf.iter_mut() {
                    b.drain(..drop.min(b.len()));
                }
                self.n_base += drop as u64;
            }
            self.wall_s += t0.elapsed().as_secs_f64();
            // periodic solutions
            while core_hi as f64 / self.fs >= self.next_solution_s {
                let at = self.next_solution_s;
                self.solve(at);
                self.next_solution_s += self.cfg.solution_every_s;
            }
            if final_flush && self.cursor >= avail {
                break;
            }
        }
        Ok(())
    }

    fn window_base_ps(&self, lo: u64) -> i64 {
        (lo as i128 * PS_PER_S as i128 / self.fs as i128) as i64
    }

    fn run_window(&mut self, lo: u64, hi: u64, core_lo: u64, core_hi: u64) -> Result<(), String> {
        let a = (lo - self.n_base) as usize;
        let b = (hi - self.n_base) as usize;
        let channels: Vec<Vec<f64>> = self
            .buf
            .iter()
            .map(|c| c.iter().skip(a).take(b - a).copied().collect())
            .collect();
        let base_ps = self.window_base_ps(lo);
        let base_s = Ps(base_ps).as_s();
        let start = self.scene_start.add_ps(Ps(base_ps));
        let input = RxInput::new(channels, self.fs, start);
        let (_rx, out) = crate::rx::process(&input, &self.chains, self.cfg.rx.clone())?;
        let core_lo_s = core_lo as f64 / self.fs;
        let core_hi_s = core_hi as f64 / self.fs;
        let mut emit: Vec<Measurement> = out
            .measurements
            .into_iter()
            .filter_map(|mut m| {
                let toa_scene = m.toa_s + base_s;
                if toa_scene < core_lo_s || toa_scene >= core_hi_s {
                    return None;
                }
                m.toa_s = toa_scene;
                m.envelope_toa_s += base_s;
                Some(m)
            })
            .collect();
        emit.sort_by(|x, y| x.toa_s.total_cmp(&y.toa_s));
        for m in emit {
            let key = (m.chain_index, m.station_index, m.channel);
            let period = 1.0 / self.chains[m.chain_index].signal.carrier_hz;
            let gri = self.chains[m.chain_index].gri_us as f64 * 1e-6;
            let cross = match self.last.get(&key) {
                Some(&(pg, ptoa)) => {
                    let pred = ptoa + (m.group - pg) as f64 * gri;
                    (m.toa_s - pred).abs() > period / 4.0
                }
                None => false,
            };
            self.last.insert(key, (m.group, m.toa_s));
            let lock = self.locks.entry(key).or_insert_with(|| StationLock {
                chain: m.chain.clone(),
                station: m.station.clone(),
                channel: m.channel,
                last_toa_s: m.toa_s,
                last_group: m.group,
                measurements: 0,
                cycle_slips: 0,
            });
            lock.last_toa_s = m.toa_s;
            lock.last_group = m.group;
            lock.measurements += 1;
            if m.cycle_slip || cross {
                lock.cycle_slips += 1;
            }
            self.recent.push_back(m.clone());
            while self.recent.len() > self.cfg.keep_measurements {
                self.recent.pop_front();
            }
            self.emitted += 1;
            self.out_meas.push(RtMeasurement {
                m,
                cross_window_slip: cross,
            });
        }
        Ok(())
    }

    fn solve(&mut self, at_s: f64) {
        if self.recent.is_empty() {
            return;
        }
        let lo = if self.cfg.solution_window_s > 0.0 {
            at_s - self.cfg.solution_window_s
        } else {
            f64::NEG_INFINITY
        };
        let meas: Vec<Measurement> = self
            .recent
            .iter()
            .filter(|m| m.toa_s >= lo && m.toa_s <= at_s)
            .cloned()
            .collect();
        if meas.is_empty() {
            return;
        }
        let fix = if self.cfg.position {
            let pc = PositionConfig {
                initial: self.cfg.known_position,
                ..Default::default()
            };
            match position_solution(
                &meas,
                &self.chains,
                self.scene_start,
                &self.cfg.corrections,
                &pc,
            ) {
                Ok(s) => Some(RtFix::Position(s)),
                Err(_) => self.cfg.known_position.map(|p| {
                    RtFix::Time(time_solution(
                        &meas,
                        &self.chains,
                        self.scene_start,
                        p,
                        &self.cfg.corrections,
                    ))
                }),
            }
        } else {
            self.cfg.known_position.map(|p| {
                RtFix::Time(time_solution(
                    &meas,
                    &self.chains,
                    self.scene_start,
                    p,
                    &self.cfg.corrections,
                ))
            })
        };
        if let Some(fix) = fix {
            self.out_sol.push(RtSolution {
                at_s,
                n_measurements: meas.len(),
                fix,
            });
        }
    }

    /// Take the measurements produced since the last call.
    pub fn drain_measurements(&mut self) -> Vec<RtMeasurement> {
        std::mem::take(&mut self.out_meas)
    }

    /// Take the solutions produced since the last call.
    pub fn drain_solutions(&mut self) -> Vec<RtSolution> {
        std::mem::take(&mut self.out_sol)
    }

    /// A snapshot of the receiver's state.
    pub fn status(&self) -> StatusSnapshot {
        let backlog = self.available().saturating_sub(self.cursor);
        let processed = self.cursor;
        let mut locks: Vec<StationLock> = self.locks.values().cloned().collect();
        locks.sort_by(|a, b| {
            (a.chain.as_str(), a.station.as_str(), a.channel).cmp(&(
                b.chain.as_str(),
                b.station.as_str(),
                b.channel,
            ))
        });
        StatusSnapshot {
            samples_in: self.samples_in,
            samples_processed: processed,
            backlog_samples: backlog,
            latency_s: backlog as f64 / self.fs,
            processing_wall_s: self.wall_s,
            realtime_factor: if self.wall_s > 0.0 {
                (processed as f64 / self.fs) / self.wall_s
            } else {
                f64::INFINITY
            },
            windows_processed: self.windows,
            measurements_emitted: self.emitted,
            buffered_samples: self.buf.first().map(|b| b.len()).unwrap_or(0),
            locks,
        }
    }
}

#[cfg(test)]
mod tests;
