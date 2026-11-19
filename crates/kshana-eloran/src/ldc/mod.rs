// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! LORAN Data Channel (LDC) schemes, each a [`crate::signal::GroupModulator`].
//!
//! **Exact on-air formats are MODELLED from the cited literature until confirmed against
//! the governing standard text.** Where a published value is not reproduced with confidence
//! it is configurable, and the doc comment says NEEDS CONFIRMATION.
//!
//! One generic transmit chain ([`modulator`]) serves every scheme:
//!
//! ```text
//! message ─▶ type+body bits ─▶ + CRC-24Q ─▶ frames (data + frame CRC) ─▶ RS(n,k) GF(2^m)
//!        ─▶ channel symbols (b bits/group) ─▶ mapper ─▶ pulse time shifts / phase / data pulse
//! ```
//!
//! Schemes ([`LdcConfig`], `ldc = { scheme = "…", … }` on a scenario chain):
//!
//! * `generic`: any [`mapper::MapperSpec`] (N-ary PPM, tri-state PPM, phase), any RS code and
//!   frame CRC, optional inserted data pulse.
//! * `eurofix`: van Willigen, Offermans, Helwig, "EUROFIX: definition and current status",
//!   IEEE PLANS 1998; "Eurofix: test results of a cost-effective DGNSS augmentation
//!   system", J. Navigation 1997. Tri-state PPM (0, ±1 µs) on pulses 3–8, balanced patterns
//!   only (141 exist; 128 carry 7 bits per GRI), RS over GF(2^7) — published (as recalled)
//!   as RS(30, 10): 30 GRIs per message, 70 information bits = 56 data bits + a 14-bit
//!   CRC, 140 parity bits (corrects 10 symbol errors). Defaults follow that; NEEDS
//!   CONFIRMATION: the RS field polynomial and first root, the CRC-14 polynomial, the
//!   pattern-to-bits mapping (here lexicographic, see [`mapper`]) and any interleaving.
//! * `ninth_pulse`: Dykstra & Peterson, "The LORAN Data Channel: progress to date and
//!   future plans", ILA 2006; Lo, Peterson, Enge, Swaszek, "LORAN data modulation:
//!   extensions and examples", IEEE TAES 2007. An extra pulse after the 8th pulse of
//!   secondaries (and a data pulse on the master), 32-state PPM (5 bits/GRI), RS over
//!   GF(2^5). Defaults: data pulse 1000 µs after the 8th pulse (master and secondary),
//!   32 states 1 µs apart centred on nominal, RS(24, 9) (45 data bits = 120 channel bits
//!   per message, 24 GRIs), no frame CRC, data-pulse phase code +/− in A/B. NEEDS
//!   CONFIRMATION: all of these values — the offset of the master's data pulse, the
//!   state spacing and centring, the RS parameters, field polynomial and the phase code.
//!
//! Each frame carries `data_bits` of a message block; Eurofix's published 56-bit data
//! field is respected as the frame data width, and Kshana's message layer ([`msg`]) with its
//! CRC-24Q rides on top (a message may span several frames).
//!
//! The receive side is [`rx::LdcDecoder`] (symbol decisions from per-pulse timing offsets,
//! FEC/CRC decoding to messages with error counts, frame alignment, authentication check).

pub mod bits;
pub mod crc;
pub mod delay_auth;
pub mod gf;
pub mod hmac;
pub mod mapper;
pub mod modulator;
pub mod msg;
pub mod rs;
pub mod rx;

#[cfg(test)]
mod tests;

use crate::chain::Chain;
use crate::signal::GroupModulator;
use crc::FrameCrc;
use mapper::{Mapper, MapperSpec, DATA_PULSE};
use modulator::{make_plan, Compiled, DataPulse, FrameCodec, LdcModulator};
use msg::MessageSpec;
use rs::ReedSolomon;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use modulator::SymbolLog;
pub use rx::LdcDecoder;

/// A scheduled message: the message plus how many consecutive times it is sent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScheduleEntry {
    #[serde(flatten)]
    pub message: MessageSpec,
    #[serde(default = "d_one")]
    pub repeat: u32,
}

/// A station-specific schedule (station index within the chain).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationSchedule {
    pub station: usize,
    pub schedule: Vec<ScheduleEntry>,
}

fn d_one() -> u32 {
    1
}
fn d_log_limit() -> usize {
    1_000_000
}

/// Settings common to every scheme.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LdcCommon {
    /// Round-robin message list of every station without its own schedule. Empty: station
    /// ID, UTC time (when the GRI is known: from the chain, or `gri_us`) and a null message.
    #[serde(default)]
    pub schedule: Vec<ScheduleEntry>,
    #[serde(default)]
    pub station_schedules: Vec<StationSchedule>,
    /// Stations (indices) that carry data; all when absent.
    #[serde(default)]
    pub stations: Option<Vec<usize>>,
    /// The chain's GRI (µs), needed by time-dependent messages (UTC time, DGNSS Z-count).
    /// Optional: the scenario builder takes it from the chain ([`LdcConfig::for_chain`]);
    /// an explicit value is an override that must equal the chain's GRI.
    #[serde(default)]
    pub gri_us: Option<u32>,
    /// HMAC key (hex) of authentication messages; empty key when absent.
    #[serde(default)]
    pub auth_key_hex: Option<String>,
    /// Maximum entries of the transmitted-symbol log.
    #[serde(default = "d_log_limit")]
    pub log_limit: usize,
}

impl Default for LdcCommon {
    fn default() -> Self {
        LdcCommon {
            schedule: vec![],
            station_schedules: vec![],
            stations: None,
            gri_us: None,
            auth_key_hex: None,
            log_limit: d_log_limit(),
        }
    }
}

/// RS code parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FecSpec {
    /// Bits per RS symbol (3..=8).
    pub m: u32,
    pub n: usize,
    pub k: usize,
}

impl Default for FecSpec {
    fn default() -> Self {
        FecSpec { m: 5, n: 31, k: 15 }
    }
}

/// Inserted data pulse (scenario side).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataPulseSpec {
    #[serde(default = "d1000")]
    pub after_us: f64,
    #[serde(default = "d1000")]
    pub master_after_us: f64,
    #[serde(default = "d_pos")]
    pub code_a: i8,
    #[serde(default = "d_neg")]
    pub code_b: i8,
    #[serde(default = "d_amp")]
    pub amp: f64,
}

impl Default for DataPulseSpec {
    fn default() -> Self {
        DataPulseSpec {
            after_us: 1000.0,
            master_after_us: 1000.0,
            code_a: 1,
            code_b: -1,
            amp: 1.0,
        }
    }
}

fn d1000() -> f64 {
    1000.0
}
fn d_pos() -> i8 {
    1
}
fn d_neg() -> i8 {
    -1
}
fn d_amp() -> f64 {
    1.0
}
fn d_step1() -> f64 {
    1.0
}
fn d_eurofix_pulses() -> Vec<usize> {
    vec![2, 3, 4, 5, 6, 7]
}
fn d30() -> usize {
    30
}
fn d10() -> usize {
    10
}
fn d24() -> usize {
    24
}
fn d9() -> usize {
    9
}
fn d32() -> u32 {
    32
}
fn d_crc14() -> FrameCrc {
    FrameCrc::Crc14
}

/// Generic scheme: any mapper, RS code and frame CRC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GenericLdc {
    pub mapper: MapperSpec,
    #[serde(default)]
    pub fec: FecSpec,
    #[serde(default)]
    pub frame_crc: FrameCrc,
    /// Required when the mapper uses pulse index 8.
    #[serde(default)]
    pub data_pulse: Option<DataPulseSpec>,
    #[serde(flatten)]
    pub common: LdcCommon,
}

/// Eurofix profile (MODELLED; see the module docs for values needing confirmation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EurofixLdc {
    /// Tri-state shift (µs), published ±1 µs.
    #[serde(default = "d_step1")]
    pub step_us: f64,
    /// LDC pulse indices modulated (0-based; published pulses 3–8 = indices 2..=7).
    #[serde(default = "d_eurofix_pulses")]
    pub pulses: Vec<usize>,
    /// RS(n, k) over GF(2^7); published RS(30, 10) as recalled — NEEDS CONFIRMATION.
    #[serde(default = "d30")]
    pub rs_n: usize,
    #[serde(default = "d10")]
    pub rs_k: usize,
    /// Frame CRC; published 14 bits (polynomial NEEDS CONFIRMATION).
    #[serde(default = "d_crc14")]
    pub frame_crc: FrameCrc,
    #[serde(flatten)]
    pub common: LdcCommon,
}

/// 9th-pulse LDC profile (MODELLED; see the module docs for values needing confirmation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NinthPulseLdc {
    /// Data pulse offset after the 8th pulse, secondaries (µs).
    #[serde(default = "d1000")]
    pub offset_us: f64,
    /// Data pulse offset after the 8th pulse, master (µs).
    #[serde(default = "d1000")]
    pub master_offset_us: f64,
    /// PPM states (power of two), published 32.
    #[serde(default = "d32")]
    pub states: u32,
    /// PPM state spacing (µs).
    #[serde(default = "d_step1")]
    pub step_us: f64,
    /// Phase modulation of the data pulse with this many states instead of PPM.
    #[serde(default)]
    pub phase_states: Option<u32>,
    /// RS symbol bits; defaults to the bits per GRI (5 for 32 states).
    #[serde(default)]
    pub rs_m: Option<u32>,
    #[serde(default = "d24")]
    pub rs_n: usize,
    #[serde(default = "d9")]
    pub rs_k: usize,
    #[serde(default)]
    pub frame_crc: FrameCrc,
    #[serde(default = "d_pos")]
    pub code_a: i8,
    #[serde(default = "d_neg")]
    pub code_b: i8,
    #[serde(flatten)]
    pub common: LdcCommon,
}

/// LDC configuration of a chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "scheme", rename_all = "snake_case")]
pub enum LdcConfig {
    /// No modulation.
    None,
    Generic(GenericLdc),
    Eurofix(EurofixLdc),
    NinthPulse(NinthPulseLdc),
}

impl LdcConfig {
    /// A Eurofix configuration with the default (published-as-recalled) parameters.
    pub fn eurofix(common: LdcCommon) -> LdcConfig {
        LdcConfig::Eurofix(EurofixLdc {
            step_us: 1.0,
            pulses: d_eurofix_pulses(),
            rs_n: 30,
            rs_k: 10,
            frame_crc: FrameCrc::Crc14,
            common,
        })
    }
    /// A 9th-pulse configuration with the default parameters.
    pub fn ninth_pulse(common: LdcCommon) -> LdcConfig {
        LdcConfig::NinthPulse(NinthPulseLdc {
            offset_us: 1000.0,
            master_offset_us: 1000.0,
            states: 32,
            step_us: 1.0,
            phase_states: None,
            rs_m: None,
            rs_n: 24,
            rs_k: 9,
            frame_crc: FrameCrc::None,
            code_a: 1,
            code_b: -1,
            common,
        })
    }

    /// Settings common to every scheme (`None` for no modulation).
    pub fn common(&self) -> Option<&LdcCommon> {
        match self {
            LdcConfig::None => None,
            LdcConfig::Generic(g) => Some(&g.common),
            LdcConfig::Eurofix(e) => Some(&e.common),
            LdcConfig::NinthPulse(p) => Some(&p.common),
        }
    }
    fn common_mut(&mut self) -> Option<&mut LdcCommon> {
        match self {
            LdcConfig::None => None,
            LdcConfig::Generic(g) => Some(&mut g.common),
            LdcConfig::Eurofix(e) => Some(&mut e.common),
            LdcConfig::NinthPulse(p) => Some(&mut p.common),
        }
    }

    /// This configuration completed from the chain it modulates: `gri_us` is taken from
    /// the chain (an explicit `gri_us` must agree with it, else an error), and the station
    /// indices of `stations` / `station_schedules` are checked against the chain's
    /// stations. The scenario builder and the receiver call this, so the `ldc` table need
    /// not repeat the chain's GRI.
    pub fn for_chain(&self, chain: &Chain) -> Result<LdcConfig, String> {
        let mut c = self.clone();
        let n = chain.stations.len();
        if let Some(common) = c.common_mut() {
            match common.gri_us {
                Some(g) if g != chain.gri_us => {
                    return Err(format!(
                        "chain {}: ldc gri_us {g} disagrees with the chain's gri_us {}",
                        chain.name, chain.gri_us
                    ))
                }
                _ => common.gri_us = Some(chain.gri_us),
            }
            let bad = common
                .stations
                .iter()
                .flatten()
                .chain(common.station_schedules.iter().map(|s| &s.station))
                .find(|s| **s >= n);
            if let Some(s) = bad {
                return Err(format!(
                    "chain {}: ldc station index {s} out of range (chain has {n} stations)",
                    chain.name
                ));
            }
        }
        Ok(c)
    }

    /// Compile to the shared transmit/receive description (`None` for no modulation).
    pub fn compile(&self) -> Result<Option<Compiled>, String> {
        let (name, mapper, fec, frame_crc, data_pulse, common) = match self {
            LdcConfig::None => return Ok(None),
            LdcConfig::Generic(g) => {
                let mapper = Mapper::from_spec(&g.mapper)?;
                let dp = match (&g.data_pulse, mapper.uses_data_pulse()) {
                    (Some(d), _) => Some(*d),
                    (None, true) => return Err(format!("generic LDC: mapper uses pulse {DATA_PULSE} but no data_pulse is configured")),
                    (None, false) => None,
                };
                ("generic", mapper, g.fec, g.frame_crc, dp, &g.common)
            }
            LdcConfig::Eurofix(e) => {
                let mapper = Mapper::from_spec(&MapperSpec::TriState {
                    pulses: e.pulses.clone(),
                    step_us: e.step_us,
                    balanced: true,
                })?;
                if e.pulses.contains(&DATA_PULSE) {
                    return Err("eurofix modulates group pulses only".into());
                }
                let fec = FecSpec {
                    m: mapper.bits_per_symbol(),
                    n: e.rs_n,
                    k: e.rs_k,
                };
                ("eurofix", mapper, fec, e.frame_crc, None, &e.common)
            }
            LdcConfig::NinthPulse(p) => {
                let mapper = match p.phase_states {
                    Some(s) => Mapper::from_spec(&MapperSpec::Phase {
                        pulses: vec![DATA_PULSE],
                        states: s,
                    })?,
                    None => Mapper::from_spec(&MapperSpec::Ppm {
                        pulses: vec![DATA_PULSE],
                        states: p.states,
                        step_us: p.step_us,
                    })?,
                };
                let m = p
                    .rs_m
                    .unwrap_or_else(|| mapper.bits_per_symbol().clamp(3, 8));
                let dp = DataPulseSpec {
                    after_us: p.offset_us,
                    master_after_us: p.master_offset_us,
                    code_a: p.code_a,
                    code_b: p.code_b,
                    amp: 1.0,
                };
                (
                    "ninth_pulse",
                    mapper,
                    FecSpec {
                        m,
                        n: p.rs_n,
                        k: p.rs_k,
                    },
                    p.frame_crc,
                    Some(dp),
                    &p.common,
                )
            }
        };
        let rs = ReedSolomon::new(fec.m, fec.n, fec.k)?;
        let codec = FrameCodec::new(rs, frame_crc, mapper.bits_per_symbol())?;
        let data_pulse = match data_pulse {
            Some(d) => {
                if !(d.after_us > 300.0 && d.master_after_us > 300.0) {
                    return Err("data pulse must follow the 8th pulse by more than 300 µs".into());
                }
                if ![d.code_a, d.code_b].iter().all(|c| *c == 1 || *c == -1) {
                    return Err("data pulse phase codes are +1 or -1".into());
                }
                Some(DataPulse {
                    after_us: d.after_us,
                    master_after_us: d.master_after_us,
                    code_a: d.code_a,
                    code_b: d.code_b,
                    amp: d.amp,
                })
            }
            None => None,
        };
        let to_pairs = |s: &[ScheduleEntry]| {
            s.iter()
                .map(|e| (e.message.clone(), e.repeat))
                .collect::<Vec<_>>()
        };
        let default_schedule: Vec<(MessageSpec, u32)> = if common.schedule.is_empty() {
            let mut v = vec![(
                MessageSpec::StationId {
                    chain_id: 0,
                    station_code: None,
                    signal_version: 0,
                },
                1,
            )];
            if common.gri_us.is_some() {
                v.push((
                    MessageSpec::UtcTime {
                        tai_minus_utc_s: 37,
                        leap_pending: 0,
                        days_to_leap: 0,
                    },
                    1,
                ));
            }
            v.push((MessageSpec::Null, 1));
            v
        } else {
            to_pairs(&common.schedule)
        };
        let mut plans = vec![make_plan(&codec, &default_schedule)?];
        let mut station_plan = BTreeMap::new();
        for s in &common.station_schedules {
            plans.push(make_plan(&codec, &to_pairs(&s.schedule))?);
            if station_plan.insert(s.station, plans.len() - 1).is_some() {
                return Err(format!("LDC: station {} has two schedules", s.station));
            }
        }
        let needs_time = plans.iter().flat_map(|p| &p.entries).any(|e| {
            matches!(
                e.spec,
                MessageSpec::UtcTime { .. } | MessageSpec::Dgnss { z_count: None, .. }
            )
        });
        if needs_time && common.gri_us.is_none() {
            return Err("LDC: utc_time / dgnss (without z_count) messages need the chain's GRI: build with the \
                        chain (LdcConfig::for_chain) or set gri_us in the ldc table"
                .into());
        }
        if let Some(g) = common.gri_us {
            if !(40_000..=99_990).contains(&g) || g % 10 != 0 {
                return Err(format!("LDC: gri_us {g} is not a valid GRI"));
            }
        }
        let auth_key = match &common.auth_key_hex {
            Some(h) => hex::decode(h).map_err(|e| format!("LDC auth_key_hex: {e}"))?,
            None => vec![],
        };
        Ok(Some(Compiled {
            name: name.into(),
            codec,
            mapper,
            data_pulse,
            stations: common.stations.clone(),
            plans,
            station_plan,
            gri_us: common.gri_us,
            auth_key,
            log_limit: common.log_limit,
        }))
    }
}

/// Build the concrete modulator (keeps access to the symbol log and truth methods) for
/// `chain` (see [`LdcConfig::for_chain`]); `None` uses the configuration as it stands
/// (time-dependent messages then need an explicit `gri_us`).
pub fn build_modulator(
    cfg: &LdcConfig,
    chain: Option<&Chain>,
) -> Result<Option<LdcModulator>, String> {
    let cfg = match chain {
        Some(c) => cfg.for_chain(c)?,
        None => cfg.clone(),
    };
    Ok(cfg.compile()?.map(LdcModulator::new))
}

/// Build the modulator as a trait object (used by the scenario builder, which passes the
/// chain).
pub fn build(
    cfg: &LdcConfig,
    chain: Option<&Chain>,
) -> Result<Option<Box<dyn GroupModulator>>, String> {
    Ok(build_modulator(cfg, chain)?.map(|m| Box::new(m) as Box<dyn GroupModulator>))
}
