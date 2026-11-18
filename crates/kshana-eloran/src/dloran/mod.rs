// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Differential eLoran (dLoran) reference station and user-side corrector (stream F2).
//!
//! A reference receiver at a **surveyed** position measures each station's time of arrival
//! and forms a ToA correction = *observed − predicted*, where the prediction is the Primary
//! Factor delay to the surveyed position ([`crate::rx::predicted_delay_s`] with no ASF). The
//! residual it sees is the Additional Secondary Factor (ASF) plus a common receiver-clock
//! term that is the same for every station. Broadcasting the residual as the per-station ToA
//! correction lets a nearby user cancel the slowly varying, spatially correlated ASF: the
//! common clock term is absorbed by the clock unknown the user already solves for
//! ([`crate::rx::position_solution`]), and what remains is only the ASF *decorrelation*
//! between the reference and the user — small for users within the reference's coverage.
//!
//! The corrections are smoothed with a configurable filter ([`Smoothing`]) to reject
//! measurement noise, and carry integrity flags: a residual whose magnitude exceeds a limit,
//! or that jumps away from its filtered value, marks the station **unusable**.
//!
//! The corrections are serialised as the existing [`crate::ldc::msg::Message::Dloran`]
//! data-channel messages; a [`UserCorrector`] ingests those messages and yields
//! [`crate::rx::Corrections`] for the user's solver, or applies them directly to a user's
//! measurements.
//!
//! Sign convention follows the message definition: `correction_ns` is **added to the
//! measured ToA**; the equivalent ASF a predictor would add is its negative.

use crate::chain::Chain;
use crate::ldc::msg::{DloranSpecEntry, Message, MessageSpec};
use crate::rx::{observations, predicted_delay_s, Corrections, Measurement};
use crate::time::AbsTime;
use std::collections::HashMap;

/// Default station code of station index `si` of a chain (matches the StationId message's
/// default: `b'A' + index`).
pub fn default_station_code(si: usize) -> u8 {
    b'A'.wrapping_add(si as u8)
}

/// Correction smoothing filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Smoothing {
    /// No smoothing: the correction is the latest residual.
    None,
    /// First-order IIR: `y ← y + α (x − y)`, `0 < α ≤ 1`.
    Exponential { alpha: f64 },
    /// Mean of the last `window` residuals.
    MovingAverage { window: usize },
}

impl Smoothing {
    fn valid(&self) -> Result<(), String> {
        match self {
            Smoothing::Exponential { alpha } if !(0.0..=1.0).contains(alpha) || *alpha == 0.0 => {
                Err("dloran: exponential alpha must be in (0, 1]".into())
            }
            Smoothing::MovingAverage { window } if *window == 0 => {
                Err("dloran: moving-average window must be ≥ 1".into())
            }
            _ => Ok(()),
        }
    }
}

/// Reference-station configuration.
#[derive(Clone, Debug)]
pub struct DloranConfig {
    pub smoothing: Smoothing,
    /// |filtered correction| beyond this marks the station unusable (ns).
    pub max_correction_ns: f64,
    /// A raw residual this far from the filtered value flags an anomaly (ns).
    pub max_residual_jump_ns: f64,
    /// Reference-site identifier put in the broadcast messages.
    pub reference_site: u16,
}

impl Default for DloranConfig {
    fn default() -> Self {
        DloranConfig {
            smoothing: Smoothing::Exponential { alpha: 0.2 },
            max_correction_ns: 10000.0,
            max_residual_jump_ns: 500.0,
            reference_site: 1,
        }
    }
}

/// Per-station filter state at the reference.
#[derive(Clone, Debug, Default)]
struct FilterState {
    filtered_ns: f64,
    history: std::collections::VecDeque<f64>,
    initialised: bool,
    updates: u64,
}

/// One station's correction produced by the reference.
#[derive(Clone, Debug, PartialEq)]
pub struct StationCorrection {
    pub chain_index: usize,
    pub station_index: usize,
    pub chain: String,
    pub station: String,
    pub station_code: u8,
    /// The latest raw residual (observed − PF-predicted) in ns.
    pub raw_residual_ns: f64,
    /// The smoothed residual (ns): this is what a *predictor* adds as ASF.
    pub filtered_residual_ns: f64,
    /// ToA correction to broadcast / add to a measured ToA (= −filtered residual, ns).
    pub correction_ns: f64,
    pub n: usize,
    /// Station usable (within residual limits, no anomaly).
    pub usable: bool,
    /// Why not usable (empty when usable).
    pub flag: String,
}

/// A differential-eLoran reference station.
pub struct ReferenceStation {
    pos: crate::geo::Geo,
    chains: Vec<Chain>,
    cfg: DloranConfig,
    filters: HashMap<(usize, usize), FilterState>,
    last: Vec<StationCorrection>,
}

impl ReferenceStation {
    /// A reference at `surveyed` position for `chains`.
    pub fn new(
        surveyed: crate::geo::Geo,
        chains: &[Chain],
        cfg: DloranConfig,
    ) -> Result<ReferenceStation, String> {
        cfg.smoothing.valid()?;
        for c in chains {
            c.validate()?;
        }
        Ok(ReferenceStation {
            pos: surveyed,
            chains: chains.to_vec(),
            cfg,
            filters: HashMap::new(),
            last: Vec::new(),
        })
    }

    /// Process a batch of reference-receiver measurements (one epoch, or a short window),
    /// updating the smoothed corrections and returning the current correction set.
    pub fn update(&mut self, meas: &[Measurement], start: AbsTime) -> Vec<StationCorrection> {
        let obs = observations(meas, &self.chains, start);
        let mut out = Vec::new();
        for o in &obs {
            let st = &self.chains[o.chain_index].stations[o.station_index];
            let pred = predicted_delay_s(st, self.pos, &o.chain, &Corrections::default());
            let raw_ns = (o.pseudo_delay_s - pred) * 1e9;
            let key = (o.chain_index, o.station_index);
            let f = self.filters.entry(key).or_default();
            let jump = if f.initialised {
                (raw_ns - f.filtered_ns).abs()
            } else {
                0.0
            };
            // update filter
            match self.cfg.smoothing {
                Smoothing::None => f.filtered_ns = raw_ns,
                Smoothing::Exponential { alpha } => {
                    if f.initialised {
                        f.filtered_ns += alpha * (raw_ns - f.filtered_ns);
                    } else {
                        f.filtered_ns = raw_ns;
                    }
                }
                Smoothing::MovingAverage { window } => {
                    f.history.push_back(raw_ns);
                    while f.history.len() > window {
                        f.history.pop_front();
                    }
                    f.filtered_ns = f.history.iter().sum::<f64>() / f.history.len() as f64;
                }
            }
            f.initialised = true;
            f.updates += 1;
            let mut usable = true;
            let mut flag = String::new();
            if f.filtered_ns.abs() > self.cfg.max_correction_ns {
                usable = false;
                flag = format!("|correction| {:.0} ns over limit", f.filtered_ns);
            } else if jump > self.cfg.max_residual_jump_ns {
                usable = false;
                flag = format!("residual jump {jump:.0} ns");
            }
            out.push(StationCorrection {
                chain_index: o.chain_index,
                station_index: o.station_index,
                chain: o.chain.clone(),
                station: o.station.clone(),
                station_code: default_station_code(o.station_index),
                raw_residual_ns: raw_ns,
                filtered_residual_ns: f.filtered_ns,
                correction_ns: -f.filtered_ns,
                n: o.n,
                usable,
                flag,
            });
        }
        self.last = out.clone();
        out
    }

    /// The current corrections as a [`Corrections`] table a predictor/solver can use
    /// directly (ASF = filtered residual; unusable stations omitted).
    pub fn corrections(&self) -> Corrections {
        let mut c = Corrections::default();
        for s in &self.last {
            if s.usable {
                c.asf_ns
                    .insert(Corrections::key(&s.chain, &s.station), s.filtered_residual_ns);
            }
        }
        c
    }

    /// The current corrections as broadcast [`MessageSpec::Dloran`] messages (usable
    /// stations only), at most three entries per message.
    pub fn message_specs(&self, age_s: u8) -> Vec<MessageSpec> {
        let entries: Vec<DloranSpecEntry> = self
            .last
            .iter()
            .filter(|s| s.usable)
            .map(|s| DloranSpecEntry {
                station_code: s.station_code,
                correction_ns: s.correction_ns,
            })
            .collect();
        entries
            .chunks(3)
            .map(|c| MessageSpec::Dloran {
                reference_site: self.cfg.reference_site,
                age_s,
                corrections: c.to_vec(),
            })
            .collect()
    }

    /// The last correction set.
    pub fn last(&self) -> &[StationCorrection] {
        &self.last
    }
}

/// A user-side applier of broadcast differential corrections.
#[derive(Clone, Debug, Default)]
pub struct UserCorrector {
    /// station_code → ToA correction (ns, add to the measured ToA).
    by_code: HashMap<u8, f64>,
    pub reference_site: Option<u16>,
    pub age_s: Option<u8>,
}

impl UserCorrector {
    /// Ingest decoded [`Message::Dloran`] messages (later messages override earlier ones for
    /// the same station code).
    pub fn ingest(&mut self, messages: &[Message]) {
        for m in messages {
            if let Message::Dloran {
                reference_site,
                age_s,
                entries,
            } = m
            {
                self.reference_site = Some(*reference_site);
                self.age_s = Some(*age_s);
                for e in entries {
                    self.by_code.insert(e.station_code, e.correction_ns as f64);
                }
            }
        }
    }

    /// From decoded messages in one call.
    pub fn from_messages(messages: &[Message]) -> UserCorrector {
        let mut u = UserCorrector::default();
        u.ingest(messages);
        u
    }

    /// The ToA correction (ns) for a station index (its default code), if any.
    pub fn correction_ns(&self, station_index: usize) -> Option<f64> {
        self.by_code.get(&default_station_code(station_index)).copied()
    }

    /// Build a [`Corrections`] table for the user's solver: ASF = −correction for every
    /// station of `chains` that a correction covers (by default station code).
    pub fn corrections(&self, chains: &[Chain]) -> Corrections {
        let mut c = Corrections::default();
        for ch in chains {
            for (si, st) in ch.stations.iter().enumerate() {
                if let Some(corr) = self.correction_ns(si) {
                    c.asf_ns.insert(Corrections::key(&ch.name, &st.id), -corr);
                }
            }
        }
        c
    }

    /// Apply the corrections directly to a user's measurements (adds the ToA correction to
    /// `toa_s`), for pipelines that correct measurements rather than predictions. Returns the
    /// number of measurements corrected.
    pub fn apply(&self, meas: &mut [Measurement]) -> usize {
        let mut n = 0;
        for m in meas.iter_mut() {
            if let Some(corr) = self.correction_ns(m.station_index) {
                m.toa_s += corr * 1e-9;
                m.envelope_toa_s += corr * 1e-9;
                n += 1;
            }
        }
        n
    }
}

#[cfg(test)]
mod tests;
