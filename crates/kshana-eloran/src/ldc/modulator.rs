// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The generic LDC transmit chain: message → block (CRC-24Q) → frames (frame CRC + RS) →
//! channel symbols (one per group) → per-group pulse modifications.
//!
//! ## Frames
//! A frame is one RS codeword. Its k·m information bits are `data_bits` of message-block
//! bits followed by the optional frame CRC; the n·m codeword bits are cut into channel
//! symbols of b = [`Mapper::bits_per_symbol`] bits (zero-padded), one per group, so a frame
//! occupies ⌈n·m / b⌉ groups. A message block occupies ⌈len / data_bits⌉ whole frames (the
//! last zero-padded).
//!
//! ## Schedule (pure function of the group number)
//! Frames are aligned to the Loran epoch: group `g` belongs to frame `⌊g / G⌋` (G groups per
//! frame) and is its symbol `g mod G`. Each station has a *cycle*: its round-robin message
//! list (each entry repeated `repeat` times) laid end to end in frames; frame f is in cycle
//! `⌊f / F⌋` at position `f mod F` (F frames per cycle). Message content that depends on time
//! (UTC time, DGNSS Z-count) is rendered for the message's first group, and the auth tag
//! covers the messages before it in the same cycle — so every group's symbol is a pure
//! function of (station, group), as [`GroupModulator::modulate`] requires (the scene calls
//! it more than once per group).

use super::bits::{from_symbols, pack_bytes, to_symbols, BitWriter};
use super::crc::FrameCrc;
use super::hmac::hmac_sha256;
use super::mapper::Mapper;
use super::msg::{Message, MessageSpec, MsgContext};
use super::rs::ReedSolomon;
use crate::signal::{GroupModulator, Parity, PulseSpec, Role};
use crate::time::Ps;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::PI;
use std::sync::{Arc, Mutex};

/// Cache of encoded frames: (station, role, frame) → channel symbols.
type FrameCache = HashMap<(usize, Role, i64), Arc<Vec<u32>>>;

/// Shared log of transmitted channel symbols: (station, group) → symbol.
pub type SymbolLog = Arc<Mutex<BTreeMap<(usize, i64), u32>>>;

/// Frame encoder/decoder: frame CRC + RS(n, k) over GF(2^m) + symbol packing.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameCodec {
    pub rs: ReedSolomon,
    pub frame_crc: FrameCrc,
    /// Channel bits per group.
    pub bits_per_symbol: u32,
}

/// Result of decoding one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameReport {
    /// The frame's data bits, when RS decoding and the frame CRC both succeeded.
    pub data: Option<Vec<bool>>,
    /// RS symbols corrected (0 when decoding failed).
    pub corrected: usize,
    pub rs_ok: bool,
    pub crc_ok: bool,
}

impl FrameCodec {
    pub fn new(
        rs: ReedSolomon,
        frame_crc: FrameCrc,
        bits_per_symbol: u32,
    ) -> Result<FrameCodec, String> {
        let c = FrameCodec {
            rs,
            frame_crc,
            bits_per_symbol,
        };
        if c.rs.k * c.m() <= frame_crc.width() + 8 {
            return Err(format!(
                "RS({}, {}) over GF(2^{}) leaves no room for data after a {}-bit frame CRC",
                c.rs.n,
                c.rs.k,
                c.m(),
                frame_crc.width()
            ));
        }
        if bits_per_symbol == 0 {
            return Err("mapper carries no bits".into());
        }
        Ok(c)
    }
    pub fn m(&self) -> usize {
        self.rs.gf.m as usize
    }
    /// Message-block bits carried per frame.
    pub fn data_bits(&self) -> usize {
        self.rs.k * self.m() - self.frame_crc.width()
    }
    /// Groups per frame.
    pub fn groups_per_frame(&self) -> usize {
        (self.rs.n * self.m()).div_ceil(self.bits_per_symbol as usize)
    }
    /// Frames needed for a block of `len` bits.
    pub fn frames_for(&self, len: usize) -> usize {
        len.div_ceil(self.data_bits()).max(1)
    }
    /// Encode one frame from up to `data_bits` bits (zero-padded) into channel symbols.
    pub fn encode_frame(&self, data: &[bool]) -> Vec<u32> {
        let mut d = data.to_vec();
        d.resize(self.data_bits(), false);
        let crc = self.frame_crc.compute(&d);
        let mut w = BitWriter::new();
        w.put_bits(&d);
        w.put(crc as u64, self.frame_crc.width() as u32);
        let info: Vec<u16> = to_symbols(&w.bits, self.m() as u32)
            .into_iter()
            .map(|s| s as u16)
            .collect();
        let cw = self.rs.encode(&info);
        let bits = from_symbols(
            &cw.iter().map(|s| *s as u32).collect::<Vec<_>>(),
            self.m() as u32,
        );
        let mut syms = to_symbols(&bits, self.bits_per_symbol);
        syms.resize(self.groups_per_frame(), 0);
        syms
    }
    /// Encode a message block into frames of channel symbols.
    pub fn encode_block(&self, block: &[bool]) -> Vec<Vec<u32>> {
        let db = self.data_bits();
        (0..self.frames_for(block.len()))
            .map(|i| {
                self.encode_frame(
                    &block[(i * db).min(block.len())..((i + 1) * db).min(block.len())],
                )
            })
            .collect()
    }
    /// Decode one frame from its channel symbols (`groups_per_frame` of them).
    pub fn decode_frame(&self, syms: &[u32]) -> FrameReport {
        let bits = from_symbols(syms, self.bits_per_symbol);
        let m = self.m();
        let need = self.rs.n * m;
        if bits.len() < need {
            return FrameReport {
                data: None,
                corrected: 0,
                rs_ok: false,
                crc_ok: false,
            };
        }
        let mut cw: Vec<u16> = to_symbols(&bits[..need], m as u32)
            .into_iter()
            .map(|s| s as u16)
            .collect();
        match self.rs.decode(&mut cw) {
            Err(_) => FrameReport {
                data: None,
                corrected: 0,
                rs_ok: false,
                crc_ok: false,
            },
            Ok(corrected) => {
                let info = from_symbols(
                    &cw[..self.rs.k]
                        .iter()
                        .map(|s| *s as u32)
                        .collect::<Vec<_>>(),
                    m as u32,
                );
                let db = self.data_bits();
                let data = info[..db].to_vec();
                let mut rx = 0u32;
                for b in &info[db..db + self.frame_crc.width()] {
                    rx = (rx << 1) | *b as u32;
                }
                let crc_ok = rx == self.frame_crc.compute(&data);
                FrameReport {
                    data: crc_ok.then_some(data),
                    corrected,
                    rs_ok: true,
                    crc_ok,
                }
            }
        }
    }
}

/// The inserted data pulse (9th-pulse LDC).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataPulse {
    /// Nominal offset after the 8th group pulse, secondaries (µs).
    pub after_us: f64,
    /// Nominal offset after the 8th group pulse, master (µs).
    pub master_after_us: f64,
    /// Phase code (+1/−1) in A and B intervals.
    pub code_a: i8,
    pub code_b: i8,
    pub amp: f64,
}

impl DataPulse {
    pub fn after(&self, role: Role) -> f64 {
        match role {
            Role::Master => self.master_after_us,
            Role::Secondary => self.after_us,
        }
    }
    /// Insert the (unmodulated) data pulse into `pulses`; returns its position.
    pub fn insert(&self, role: Role, group: i64, pulses: &mut Vec<PulseSpec>) -> Option<usize> {
        let p8 = pulses.iter().find(|p| p.k == 7)?;
        let offset = p8.offset + Ps::from_us(self.after(role));
        let k = pulses.iter().map(|p| p.k).max().unwrap_or(0) + 1;
        let code = match Parity::of_group(group) {
            Parity::A => self.code_a,
            Parity::B => self.code_b,
        };
        let spec = PulseSpec {
            offset,
            phase_rad: if code < 0 { PI } else { 0.0 },
            amp: self.amp,
            k,
        };
        let pos = pulses
            .iter()
            .position(|p| p.offset > offset)
            .unwrap_or(pulses.len());
        pulses.insert(pos, spec);
        Some(pos)
    }
}

/// One message entry of a cycle.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanEntry {
    pub spec: MessageSpec,
    /// First frame within the cycle, and frame count.
    pub start: i64,
    pub frames: i64,
    /// For auth entries: index of the first covered entry.
    pub cover_from: usize,
}

/// A station's message cycle.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub entries: Vec<PlanEntry>,
    pub cycle_frames: i64,
}

/// A message as scheduled (truth).
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduledMessage {
    pub message: Message,
    /// Absolute frame number (since the Loran epoch) of its first frame.
    pub first_frame: i64,
    pub frames: i64,
    /// The message block bits (type + body + CRC-24Q).
    pub block: Vec<bool>,
}

/// HMAC input for an authentication message (see [`super::msg`]).
pub fn auth_input(station: usize, auth_first_frame: i64, covered: &[Vec<bool>]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&(station as u16).to_be_bytes());
    v.extend_from_slice(&auth_first_frame.to_be_bytes());
    for b in covered {
        v.extend(pack_bytes(b));
    }
    v
}

/// Everything the transmit and receive sides share, compiled from an [`super::LdcConfig`].
#[derive(Clone, Debug, PartialEq)]
pub struct Compiled {
    pub name: String,
    pub codec: FrameCodec,
    pub mapper: Mapper,
    pub data_pulse: Option<DataPulse>,
    pub stations: Option<Vec<usize>>,
    /// Plan 0 is the default; `station_plan` maps stations with their own schedule.
    pub plans: Vec<Plan>,
    pub station_plan: BTreeMap<usize, usize>,
    pub gri_us: Option<u32>,
    pub auth_key: Vec<u8>,
    pub log_limit: usize,
}

/// Build a plan from (spec, repeat) entries.
pub fn make_plan(codec: &FrameCodec, entries: &[(MessageSpec, u32)]) -> Result<Plan, String> {
    let dummy = MsgContext {
        station: 0,
        role: Role::Secondary,
        first_group: 0,
        gri_us: Some(99_300),
    };
    let mut out: Vec<PlanEntry> = Vec::new();
    let mut start = 0i64;
    let mut last_auth_end = 0usize;
    for (spec, rep) in entries {
        spec.validate()?;
        for _ in 0..(*rep).max(1) {
            let frames = codec.frames_for(spec.render(&dummy).to_block().len()) as i64;
            let idx = out.len();
            let cover_from = if matches!(spec, MessageSpec::Auth { .. }) {
                let c = last_auth_end;
                if idx - c > 63 {
                    return Err("auth message covers at most 63 messages".into());
                }
                last_auth_end = idx + 1;
                c
            } else {
                0
            };
            out.push(PlanEntry {
                spec: spec.clone(),
                start,
                frames,
                cover_from,
            });
            start += frames;
        }
    }
    if out.is_empty() {
        return Err("LDC schedule is empty".into());
    }
    Ok(Plan {
        entries: out,
        cycle_frames: start,
    })
}

/// The LDC [`GroupModulator`] of every scheme.
pub struct LdcModulator {
    pub c: Compiled,
    cache: Mutex<FrameCache>,
    log: SymbolLog,
}

impl LdcModulator {
    pub fn new(c: Compiled) -> LdcModulator {
        LdcModulator {
            c,
            cache: Mutex::new(HashMap::new()),
            log: SymbolLog::default(),
        }
    }

    pub fn codec(&self) -> &FrameCodec {
        &self.c.codec
    }
    pub fn mapper(&self) -> &Mapper {
        &self.c.mapper
    }
    pub fn groups_per_frame(&self) -> i64 {
        self.c.codec.groups_per_frame() as i64
    }
    /// Whether station `station` carries data.
    pub fn modulates(&self, station: usize) -> bool {
        self.c
            .stations
            .as_ref()
            .is_none_or(|s| s.contains(&station))
    }
    fn plan(&self, station: usize) -> &Plan {
        &self.c.plans[self.c.station_plan.get(&station).copied().unwrap_or(0)]
    }

    /// Frame number of group `group`.
    pub fn frame_of(&self, group: i64) -> i64 {
        group.div_euclid(self.groups_per_frame())
    }

    /// (entry index, absolute first frame of that message) of frame `frame`.
    fn locate(&self, station: usize, frame: i64) -> (usize, i64) {
        let plan = self.plan(station);
        let pos = frame.rem_euclid(plan.cycle_frames);
        let base = frame - pos;
        let i = plan.entries.partition_point(|e| e.start <= pos) - 1;
        (i, base + plan.entries[i].start)
    }

    fn render(&self, station: usize, role: Role, entry: usize, first_frame: i64) -> Message {
        let plan = self.plan(station);
        let e = &plan.entries[entry];
        let ctx = MsgContext {
            station,
            role,
            first_group: first_frame * self.groups_per_frame(),
            gri_us: self.c.gri_us,
        };
        let mut m = e.spec.render(&ctx);
        if let Message::Auth { covered, tag, .. } = &mut m {
            let blocks: Vec<Vec<bool>> = (e.cover_from..entry)
                .map(|j| {
                    let ff = first_frame - e.start + plan.entries[j].start;
                    self.render(station, role, j, ff).to_bits()
                })
                .collect();
            *covered = blocks.len() as u8;
            let mac = hmac_sha256(&self.c.auth_key, &auth_input(station, first_frame, &blocks));
            let l = tag.len();
            tag.copy_from_slice(&mac[..l]);
        }
        m
    }

    /// The message whose frames include absolute frame `frame` (truth).
    pub fn message_at(&self, station: usize, role: Role, frame: i64) -> ScheduledMessage {
        let (i, ff) = self.locate(station, frame);
        let m = self.render(station, role, i, ff);
        let block = m.to_block();
        ScheduledMessage {
            message: m,
            first_frame: ff,
            frames: self.plan(station).entries[i].frames,
            block,
        }
    }

    /// Messages that start in frames `[f0, f1)` (truth).
    pub fn messages_starting(
        &self,
        station: usize,
        role: Role,
        f0: i64,
        f1: i64,
    ) -> Vec<ScheduledMessage> {
        let mut out = Vec::new();
        let mut f = f0;
        while f < f1 {
            let m = self.message_at(station, role, f);
            if m.first_frame == f {
                f += m.frames;
                out.push(m);
            } else {
                f = m.first_frame + m.frames;
            }
        }
        out
    }

    /// Channel symbols of absolute frame `frame`.
    pub fn frame_symbols(&self, station: usize, role: Role, frame: i64) -> Arc<Vec<u32>> {
        if let Some(v) = self.cache.lock().unwrap().get(&(station, role, frame)) {
            return v.clone();
        }
        let m = self.message_at(station, role, frame);
        let frames = self.c.codec.encode_block(&m.block);
        let v = Arc::new(frames[(frame - m.first_frame) as usize].clone());
        let mut c = self.cache.lock().unwrap();
        if c.len() > 4096 {
            c.clear();
        }
        c.insert((station, role, frame), v.clone());
        v
    }

    /// The channel symbol of `station`'s group `group` (truth), `None` if it carries no data.
    pub fn symbol(&self, station: usize, role: Role, group: i64) -> Option<u32> {
        if !self.modulates(station) {
            return None;
        }
        let g = self.groups_per_frame();
        Some(self.frame_symbols(station, role, group.div_euclid(g))[group.rem_euclid(g) as usize])
    }

    /// Symbols for groups `[g0, g1)` (truth).
    pub fn symbols(&self, station: usize, role: Role, g0: i64, g1: i64) -> Vec<u32> {
        (g0..g1)
            .filter_map(|g| self.symbol(station, role, g))
            .collect()
    }

    /// Handle on the shared log of symbols actually passed to the pulse modulation.
    pub fn log(&self) -> SymbolLog {
        self.log.clone()
    }

    /// Logged (group, symbol) pairs of `station`, in group order.
    pub fn transmitted(&self, station: usize) -> Vec<(i64, u32)> {
        self.log
            .lock()
            .unwrap()
            .range((station, i64::MIN)..=(station, i64::MAX))
            .map(|((_, g), s)| (*g, *s))
            .collect()
    }
}

impl GroupModulator for LdcModulator {
    fn name(&self) -> &str {
        &self.c.name
    }

    fn modulate(&self, station: usize, role: Role, group: i64, pulses: &mut Vec<PulseSpec>) {
        if !self.modulates(station) {
            return;
        }
        let dp = self
            .c
            .data_pulse
            .and_then(|d| d.insert(role, group, pulses));
        let Some(sym) = self.symbol(station, role, group) else {
            return;
        };
        self.c.mapper.apply(sym, pulses, dp);
        let mut log = self.log.lock().unwrap();
        if log.len() < self.c.log_limit {
            log.insert((station, group), sym);
        }
    }
}
