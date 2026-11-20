// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! SBAS message-sequence generator for failure tests.
//!
//! A sequence spec states the masked satellites, a nominal broadcast schedule (how often each
//! message stream is sent: a **test-design choice**, set to the maximum update intervals of
//! the applicant's copy of the MOPS to test the worst case) and scripted events. The generator
//! produces one frame per second (or silence), deterministic for a given spec, and the
//! **expected-behaviour timeline**: for each event, what the equipment under test is expected
//! to do, from which reference time, within which named limit. The limits are parameters the
//! procedure names and the applicant supplies ([`super::procedure`]).
//!
//! Events (signal content only; the generator writes message bits, never RF):
//!
//! | kind | effect on the stream | expectation |
//! |------|----------------------|-------------|
//! | `udrei` | the satellite's UDREI set (14 not monitored, 15 do not use) in MT2-5 and MT6; an MT6 with IODF 3 (alarm) for its block at onset | excluded by `alert_time_s` after the first message carrying it; used again after the event ends |
//! | `withhold_fast` | the fast-correction message of the satellite's block is not sent | every satellite of the block excluded by `fast_timeout_s` after the block's last message |
//! | `do_not_use` | MT0 every second | SBAS-based modes left by `alert_time_s` after the first MT0 |
//! | `silence` | no frame at all | SBAS-based modes left by `message_loss_s` after the last frame |
//! | `givei` | one grid point's GIVEI set (15 not monitored) in MT26 | informational (the log carries no grid use) |
//! | `fc_step` | a step of `delta_m` in one satellite's fast correction | the satellite stays in use |
//! | `iodp_change` | IODP changed; MT1 sent at once | informational |

use serde::{Deserialize, Serialize};

use super::sbas_msg::{self, Message, MAX_MASKED};

/// The format tag of a stand-alone sequence spec file.
pub const FORMAT: &str = "kshana-sbas-sequence/1";
/// The format tag of a generated sequence.
pub const OUTPUT_FORMAT: &str = "kshana-sbas-stream/1";

fn d_mask() -> u32 {
    60
}
fn d_fast() -> u32 {
    6
}
fn d_integrity() -> u32 {
    6
}
fn d_degradation() -> u32 {
    120
}
fn d_udrei() -> u8 {
    5
}
fn d_givei() -> u8 {
    7
}

/// How often each stream is broadcast (s); 0 switches a stream off. Test-design choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    #[serde(default = "d_mask")]
    pub mask_s: u32,
    #[serde(default = "d_fast")]
    pub fast_s: u32,
    #[serde(default = "d_integrity")]
    pub integrity_s: u32,
    #[serde(default = "d_degradation")]
    pub degradation_s: u32,
    /// MT26 per block; 0 (default) sends no ionospheric messages.
    #[serde(default)]
    pub iono_s: u32,
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            mask_s: d_mask(),
            fast_s: d_fast(),
            integrity_s: d_integrity(),
            degradation_s: d_degradation(),
            iono_s: 0,
        }
    }
}

/// Nominal content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nominal {
    #[serde(default = "d_udrei")]
    pub udrei: u8,
    #[serde(default = "d_givei")]
    pub givei: u8,
    /// MT7 degradation factor indicator for every satellite.
    #[serde(default)]
    pub ai: u8,
    /// MT7 system latency (s).
    #[serde(default)]
    pub t_lat_s: u8,
    /// MT26 band number.
    #[serde(default)]
    pub iono_band: u8,
    /// MT26 blocks sent (0 to n-1).
    #[serde(default)]
    pub iono_blocks: u8,
}

impl Default for Nominal {
    fn default() -> Self {
        Nominal {
            udrei: d_udrei(),
            givei: d_givei(),
            ai: 0,
            t_lat_s: 0,
            iono_band: 0,
            iono_blocks: 0,
        }
    }
}

/// The kind of a scripted event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Udrei,
    WithholdFast,
    DoNotUse,
    Silence,
    Givei,
    FcStep,
    IodpChange,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Udrei => "udrei",
            EventKind::WithholdFast => "withhold_fast",
            EventKind::DoNotUse => "do_not_use",
            EventKind::Silence => "silence",
            EventKind::Givei => "givei",
            EventKind::FcStep => "fc_step",
            EventKind::IodpChange => "iodp_change",
        }
    }
}

/// One scripted event, active over `[t_s, t_s + duration_s)` (no duration: to the end).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub kind: EventKind,
    pub t_s: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prn: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udrei: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub givei: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub igp: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iodp: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta_m: Option<f64>,
}

impl Event {
    fn active(&self, t: u32) -> bool {
        t >= self.t_s && self.duration_s.is_none_or(|d| t < self.t_s + d)
    }
    /// The end of the event (exclusive), if it ends.
    pub fn end_s(&self) -> Option<u32> {
        self.duration_s.map(|d| self.t_s + d)
    }
}

/// A sequence spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceSpec {
    pub duration_s: u32,
    /// The masked satellites, in mask order (GPS 1 to 37, SBAS 120 to 158).
    pub prns: Vec<u16>,
    #[serde(default)]
    pub iodp: u8,
    #[serde(default)]
    pub schedule: Schedule,
    #[serde(default)]
    pub nominal: Nominal,
    #[serde(default, rename = "event")]
    pub events: Vec<Event>,
}

impl SequenceSpec {
    /// Parse a stand-alone sequence file (TOML).
    pub fn parse_file(src: &str) -> Result<SequenceSpec, String> {
        let mut v: toml::Table = toml::from_str(src).map_err(|e| format!("sequence: {e}"))?;
        match v.remove("format") {
            Some(toml::Value::String(f)) if f == FORMAT => {}
            _ => return Err(format!("sequence: needs format = \"{FORMAT}\"")),
        }
        let s: SequenceSpec = v.try_into().map_err(|e| format!("sequence: {e}"))?;
        s.validate()?;
        Ok(s)
    }

    /// The index (0-based) of `prn` in the mask.
    pub fn index_of(&self, prn: u16) -> Option<usize> {
        self.prns.iter().position(|p| *p == prn)
    }

    /// The fast-correction message type carrying mask index `i`.
    pub fn block_mt(i: usize) -> u8 {
        2 + (i / 13) as u8
    }

    /// The masked satellites whose fast corrections travel in message type `mt`.
    pub fn block_prns(&self, mt: u8) -> Vec<u16> {
        self.prns
            .iter()
            .enumerate()
            .filter(|(i, _)| Self::block_mt(*i) == mt)
            .map(|(_, p)| *p)
            .collect()
    }

    /// Check the spec.
    pub fn validate(&self) -> Result<(), String> {
        if self.duration_s == 0 || self.duration_s > 86_400 {
            return Err("sequence: duration_s must be 1 to 86400".into());
        }
        if self.prns.is_empty() || self.prns.len() > MAX_MASKED {
            return Err(format!("sequence: 1 to {MAX_MASKED} PRNs"));
        }
        let mut seen = std::collections::BTreeSet::new();
        for p in &self.prns {
            sbas_msg::slot_of_prn(*p)?;
            if !seen.insert(*p) {
                return Err(format!("sequence: PRN {p} masked twice"));
            }
        }
        if self.iodp > 3 {
            return Err("sequence: iodp is 0 to 3".into());
        }
        if self.nominal.udrei > 13 {
            return Err("sequence: nominal udrei must be 0 to 13 (a usable value)".into());
        }
        if self.nominal.givei > 14 || self.nominal.ai > 15 || self.nominal.t_lat_s > 15 {
            return Err("sequence: nominal givei 0 to 14, ai and t_lat_s 0 to 15".into());
        }
        if self.schedule.iono_s > 0 && self.nominal.iono_blocks == 0 {
            return Err("sequence: iono_s set but nominal.iono_blocks is 0".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for e in &self.events {
            let at = |m: &str| format!("event '{}': {m}", e.id);
            if !ids.insert(e.id.clone()) {
                return Err(at("duplicate id"));
            }
            if e.t_s >= self.duration_s {
                return Err(at("starts after the sequence ends"));
            }
            if e.duration_s == Some(0) {
                return Err(at("duration_s must be positive"));
            }
            let need_prn = || -> Result<u16, String> {
                let p = e.prn.ok_or_else(|| at("needs prn"))?;
                if self.index_of(p).is_none() {
                    return Err(at(&format!("PRN {p} is not in the mask")));
                }
                Ok(p)
            };
            match e.kind {
                EventKind::Udrei => {
                    need_prn()?;
                    match e.udrei {
                        Some(u) if u <= 15 => {}
                        _ => return Err(at("needs udrei 0 to 15")),
                    }
                }
                EventKind::WithholdFast => {
                    need_prn()?;
                }
                EventKind::FcStep => {
                    need_prn()?;
                    match e.delta_m {
                        Some(d) if d.is_finite() && d.abs() <= 100.0 => {}
                        _ => return Err(at("needs delta_m within ±100 m")),
                    }
                }
                EventKind::Givei => {
                    if self.schedule.iono_s == 0 {
                        return Err(at("givei needs schedule.iono_s > 0"));
                    }
                    match (e.block, e.igp, e.givei) {
                        (Some(b), Some(i), Some(g))
                            if b < self.nominal.iono_blocks && i < 15 && g <= 15 => {}
                        _ => {
                            return Err(at(
                                "needs block (a sent block), igp 0 to 14 and givei 0 to 15",
                            ))
                        }
                    }
                }
                EventKind::IodpChange => match e.iodp {
                    Some(i) if i <= 3 && i != self.iodp => {}
                    _ => return Err(at("needs iodp 0 to 3, different from the nominal")),
                },
                EventKind::DoNotUse | EventKind::Silence => {}
            }
        }
        Ok(())
    }
}

/// One generated second.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameRecord {
    pub t_s: u32,
    /// Frame counter (sets the preamble rotation); `None` for a silent second.
    pub seq: Option<u64>,
    pub mt: Option<u8>,
    pub hex: Option<String>,
    /// Why this message was sent (`schedule`, `alarm`, `event:<id>`), or `silence`.
    pub why: String,
}

/// What the equipment is expected to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectKind {
    /// The satellites are not used from `t_ref_s + limit` until the event ends.
    Exclude,
    /// The satellites are used again within `limit` after the event ends.
    Include,
    /// The equipment's mode is at most `mode` from `t_ref_s + limit`.
    ModeAtMost,
    /// The satellites stay in use throughout the event.
    RemainUsed,
    /// Recorded for the reader; not scored from the log.
    Informational,
}

/// One expected behaviour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expectation {
    pub id: String,
    pub event: String,
    pub kind: ExpectKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prns: Vec<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// The reference time the limit counts from (s).
    pub t_ref_s: f64,
    /// The end of the window the behaviour must hold over (s); `None`: to the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_s: Option<f64>,
    /// The parameter naming the limit (empty when the check has none).
    pub limit_param: String,
    pub what: String,
}

/// A generated sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub format: String,
    pub label: String,
    pub spec_sha256: String,
    pub frames: Vec<FrameRecord>,
    pub expectations: Vec<Expectation>,
    /// SHA-256 over the frame lines (`t_s mt hex`), for reproducibility checks.
    pub frames_sha256: String,
}

impl Stream {
    /// The neutral frame file: one line per second, `t_s,mt,hex` (empty fields when silent).
    pub fn frames_csv(&self) -> String {
        let mut s = String::from("t_s,mt,hex,why\n");
        for f in &self.frames {
            s.push_str(&format!(
                "{},{},{},{}\n",
                f.t_s,
                f.mt.map(|m| m.to_string()).unwrap_or_default(),
                f.hex.clone().unwrap_or_default(),
                f.why
            ));
        }
        s
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum StreamId {
    Mask,
    Fast(u8),
    Integrity,
    Degradation,
    Iono(u8),
}

fn nominal_prc(prn: u16) -> f64 {
    ((prn as i64 * 37) % 17) as f64 * 0.125 - 1.0
}

/// Generate the stream for `spec`.
pub fn generate(spec: &SequenceSpec) -> Result<Stream, String> {
    spec.validate()?;
    let spec_json = serde_json::to_string(spec).map_err(|e| e.to_string())?;
    let n_blocks = spec.prns.len().div_ceil(13) as u8;
    let mut streams: Vec<(StreamId, u32)> = Vec::new();
    if spec.schedule.mask_s > 0 {
        streams.push((StreamId::Mask, spec.schedule.mask_s));
    }
    if spec.schedule.fast_s > 0 {
        for b in 0..n_blocks {
            streams.push((StreamId::Fast(2 + b), spec.schedule.fast_s));
        }
    }
    if spec.schedule.integrity_s > 0 {
        streams.push((StreamId::Integrity, spec.schedule.integrity_s));
    }
    if spec.schedule.degradation_s > 0 {
        streams.push((StreamId::Degradation, spec.schedule.degradation_s));
    }
    if spec.schedule.iono_s > 0 {
        for b in 0..spec.nominal.iono_blocks {
            streams.push((StreamId::Iono(b), spec.schedule.iono_s));
        }
    }
    let mut last: std::collections::BTreeMap<StreamId, Option<u32>> =
        streams.iter().map(|(s, _)| (*s, None)).collect();
    let mut iodf = [0u8; 4];
    let mut seq: u64 = 0;
    let mut frames = Vec::new();
    // Per-event bookkeeping for the expectations.
    let mut first_carrier: std::collections::BTreeMap<String, u32> = Default::default();
    let mut last_block_before: std::collections::BTreeMap<String, Option<u32>> = Default::default();
    let mut last_frame_before: std::collections::BTreeMap<String, Option<u32>> = Default::default();
    let mut last_frame: Option<u32> = None;

    for t in 0..spec.duration_s {
        let active: Vec<&Event> = spec.events.iter().filter(|e| e.active(t)).collect();
        let iodp = active
            .iter()
            .filter(|e| e.kind == EventKind::IodpChange)
            .filter_map(|e| e.iodp)
            .next_back()
            .unwrap_or(spec.iodp);
        let udrei_of = |prn: u16| -> u8 {
            active
                .iter()
                .filter(|e| e.kind == EventKind::Udrei && e.prn == Some(prn))
                .filter_map(|e| e.udrei)
                .max()
                .unwrap_or(spec.nominal.udrei)
        };
        let prc_of = |prn: u16| -> f64 {
            nominal_prc(prn)
                + active
                    .iter()
                    .filter(|e| e.kind == EventKind::FcStep && e.prn == Some(prn))
                    .filter_map(|e| e.delta_m)
                    .sum::<f64>()
        };
        let withheld = |mt: u8| -> bool {
            active.iter().any(|e| {
                e.kind == EventKind::WithholdFast
                    && e.prn
                        .and_then(|p| spec.index_of(p))
                        .is_some_and(|i| SequenceSpec::block_mt(i) == mt)
            })
        };
        // Record the last block message before each withhold event starts, and the last frame
        // before each silence.
        for e in spec.events.iter().filter(|e| e.t_s == t) {
            match e.kind {
                EventKind::WithholdFast => {
                    let mt = SequenceSpec::block_mt(spec.index_of(e.prn.unwrap()).unwrap());
                    last_block_before.insert(e.id.clone(), last[&StreamId::Fast(mt)]);
                }
                EventKind::Silence => {
                    last_frame_before.insert(e.id.clone(), last_frame);
                }
                _ => {}
            }
        }

        if active.iter().any(|e| e.kind == EventKind::Silence) {
            frames.push(FrameRecord {
                t_s: t,
                seq: None,
                mt: None,
                hex: None,
                why: "silence".into(),
            });
            continue;
        }

        // Choose this second's message.
        let onset_or_end = |e: &&Event| e.t_s == t || e.end_s() == Some(t);
        let (msg, why, stream): (Message, String, Option<StreamId>) =
            if let Some(e) = active.iter().find(|e| e.kind == EventKind::DoNotUse) {
                (Message::DoNotUse, format!("event:{}", e.id), None)
            } else if let Some(e) = spec
                .events
                .iter()
                .filter(|e| e.kind == EventKind::IodpChange)
                .find(onset_or_end)
            {
                (
                    Message::Mask {
                        slots: spec.prns.clone(),
                        iodp,
                    },
                    format!("event:{}", e.id),
                    Some(StreamId::Mask),
                )
            } else if let Some(e) = spec
                .events
                .iter()
                .filter(|e| e.kind == EventKind::Udrei)
                .find(onset_or_end)
            {
                // Alarm (or all-clear) at once in MT6; IODF 3 for the block at an alarm onset.
                let mut f = iodf;
                if e.t_s == t && e.udrei.unwrap_or(0) >= sbas_msg::UDREI_NOT_MONITORED {
                    let mt = SequenceSpec::block_mt(spec.index_of(e.prn.unwrap()).unwrap());
                    f[(mt - 2) as usize] = 3;
                }
                (
                    Message::Integrity {
                        iodf: f,
                        udrei: (0..MAX_MASKED)
                            .map(|i| spec.prns.get(i).map(|p| udrei_of(*p)).unwrap_or(15))
                            .collect(),
                    },
                    if e.t_s == t {
                        format!("alarm:{}", e.id)
                    } else {
                        format!("event-end:{}", e.id)
                    },
                    Some(StreamId::Integrity),
                )
            } else {
                // The most overdue scheduled stream, fixed order on ties.
                let mut best: Option<(f64, StreamId)> = None;
                for (sid, every) in &streams {
                    if let StreamId::Fast(mt) = sid {
                        if withheld(*mt) {
                            continue;
                        }
                    }
                    let urgency = match last[sid] {
                        None => f64::INFINITY,
                        Some(l) => (t - l) as f64 / *every as f64,
                    };
                    if urgency >= 1.0 && best.is_none_or(|(u, _)| urgency > u) {
                        best = Some((urgency, *sid));
                    }
                }
                match best {
                    None => (Message::Null, "filler".into(), None),
                    Some((_, sid)) => {
                        let m = match sid {
                            StreamId::Mask => Message::Mask {
                                slots: spec.prns.clone(),
                                iodp,
                            },
                            StreamId::Fast(mt) => {
                                let b = (mt - 2) as usize;
                                iodf[b] = (iodf[b] + 1) % 3;
                                let mut prc = vec![0.0; 13];
                                let mut ud = vec![15u8; 13];
                                for k in 0..13 {
                                    if let Some(p) = spec.prns.get(b * 13 + k) {
                                        prc[k] = prc_of(*p);
                                        ud[k] = udrei_of(*p);
                                    }
                                }
                                Message::Fast {
                                    mt,
                                    iodf: iodf[b],
                                    iodp,
                                    prc_m: prc,
                                    udrei: ud,
                                }
                            }
                            StreamId::Integrity => Message::Integrity {
                                iodf,
                                udrei: (0..MAX_MASKED)
                                    .map(|i| spec.prns.get(i).map(|p| udrei_of(*p)).unwrap_or(15))
                                    .collect(),
                            },
                            StreamId::Degradation => Message::Degradation {
                                t_lat_s: spec.nominal.t_lat_s,
                                iodp,
                                ai: (0..MAX_MASKED)
                                    .map(|i| {
                                        if i < spec.prns.len() {
                                            spec.nominal.ai
                                        } else {
                                            0
                                        }
                                    })
                                    .collect(),
                            },
                            StreamId::Iono(b) => Message::Iono {
                                band: spec.nominal.iono_band,
                                block: b,
                                delay_m: (0..15).map(|i| 2.0 + i as f64 * 0.125).collect(),
                                givei: (0..15u8)
                                    .map(|i| {
                                        active
                                            .iter()
                                            .filter(|e| {
                                                e.kind == EventKind::Givei
                                                    && e.block == Some(b)
                                                    && e.igp == Some(i)
                                            })
                                            .filter_map(|e| e.givei)
                                            .max()
                                            .unwrap_or(spec.nominal.givei)
                                    })
                                    .collect(),
                                iodi: 0,
                            },
                        };
                        (m, "schedule".into(), Some(sid))
                    }
                }
            };
        if let Some(sid) = stream {
            if let Some(slot) = last.get_mut(&sid) {
                *slot = Some(t);
            }
        }
        // First message carrying each event's content.
        for e in &active {
            if first_carrier.contains_key(&e.id) {
                continue;
            }
            let carries = match (e.kind, &msg) {
                (EventKind::DoNotUse, Message::DoNotUse) => true,
                (EventKind::Udrei, Message::Integrity { .. }) => true,
                (EventKind::Udrei, Message::Fast { mt, .. }) => {
                    SequenceSpec::block_mt(spec.index_of(e.prn.unwrap()).unwrap()) == *mt
                }
                (EventKind::FcStep, Message::Fast { mt, .. }) => {
                    SequenceSpec::block_mt(spec.index_of(e.prn.unwrap()).unwrap()) == *mt
                }
                (EventKind::Givei, Message::Iono { block, .. }) => e.block == Some(*block),
                (EventKind::IodpChange, Message::Mask { .. }) => true,
                _ => false,
            };
            if carries {
                first_carrier.insert(e.id.clone(), t);
            }
        }
        let f = sbas_msg::encode(&msg, seq)?;
        frames.push(FrameRecord {
            t_s: t,
            seq: Some(seq),
            mt: Some(msg.mt()),
            hex: Some(f.to_hex()),
            why,
        });
        seq += 1;
        last_frame = Some(t);
    }

    // Expectations.
    let mut ex = Vec::new();
    for e in &spec.events {
        let until = e.end_s().map(|x| x as f64);
        let carrier = first_carrier.get(&e.id).map(|t| *t as f64);
        match e.kind {
            EventKind::Udrei => {
                let u = e.udrei.unwrap();
                if u >= sbas_msg::UDREI_NOT_MONITORED {
                    let prn = e.prn.unwrap();
                    ex.push(Expectation {
                        id: format!("{}-exclude", e.id),
                        event: e.id.clone(),
                        kind: ExpectKind::Exclude,
                        prns: vec![prn],
                        mode: None,
                        t_ref_s: carrier.unwrap_or(e.t_s as f64),
                        until_s: until,
                        limit_param: "alert_time_s".into(),
                        what: format!(
                            "PRN {prn} UDREI {u} ({}): no longer used, from the first message \
                             carrying it",
                            if u == 15 {
                                "do not use"
                            } else {
                                "not monitored"
                            }
                        ),
                    });
                    if let Some(end) = until {
                        ex.push(Expectation {
                            id: format!("{}-include", e.id),
                            event: e.id.clone(),
                            kind: ExpectKind::Include,
                            prns: vec![prn],
                            mode: None,
                            t_ref_s: end,
                            until_s: None,
                            limit_param: "return_time_s".into(),
                            what: format!(
                                "PRN {prn} used again after the UDREI returns to nominal"
                            ),
                        });
                    }
                } else {
                    ex.push(info(
                        e,
                        "UDREI raised but still usable; the protection levels grow",
                    ));
                }
            }
            EventKind::WithholdFast => {
                let mt = SequenceSpec::block_mt(spec.index_of(e.prn.unwrap()).unwrap());
                let prns = spec.block_prns(mt);
                let t_ref = last_block_before
                    .get(&e.id)
                    .copied()
                    .flatten()
                    .map(|x| x as f64)
                    .unwrap_or(e.t_s as f64);
                ex.push(Expectation {
                    id: format!("{}-exclude", e.id),
                    event: e.id.clone(),
                    kind: ExpectKind::Exclude,
                    prns: prns.clone(),
                    mode: None,
                    t_ref_s: t_ref,
                    until_s: until,
                    limit_param: "fast_timeout_s".into(),
                    what: format!(
                        "MT{mt} withheld: its satellites no longer used once their fast \
                         corrections time out, counted from the block's last message"
                    ),
                });
                if let Some(end) = until {
                    ex.push(Expectation {
                        id: format!("{}-include", e.id),
                        event: e.id.clone(),
                        kind: ExpectKind::Include,
                        prns,
                        mode: None,
                        t_ref_s: end,
                        until_s: None,
                        limit_param: "return_time_s".into(),
                        what: format!("MT{mt} satellites used again once the block is sent"),
                    });
                }
            }
            EventKind::DoNotUse => {
                ex.push(Expectation {
                    id: format!("{}-mode", e.id),
                    event: e.id.clone(),
                    kind: ExpectKind::ModeAtMost,
                    prns: vec![],
                    mode: Some("gnss-only".into()),
                    t_ref_s: carrier.unwrap_or(e.t_s as f64),
                    until_s: until,
                    limit_param: "alert_time_s".into(),
                    what: "MT0 received: no SBAS-based mode, from the first MT0".into(),
                });
            }
            EventKind::Silence => {
                let t_ref = last_frame_before
                    .get(&e.id)
                    .copied()
                    .flatten()
                    .map(|x| x as f64)
                    .unwrap_or(e.t_s as f64);
                ex.push(Expectation {
                    id: format!("{}-mode", e.id),
                    event: e.id.clone(),
                    kind: ExpectKind::ModeAtMost,
                    prns: vec![],
                    mode: Some("gnss-only".into()),
                    t_ref_s: t_ref,
                    until_s: until,
                    limit_param: "message_loss_s".into(),
                    what: "no SBAS messages: no SBAS-based mode, counted from the last frame"
                        .into(),
                });
            }
            EventKind::FcStep => {
                ex.push(Expectation {
                    id: format!("{}-kept", e.id),
                    event: e.id.clone(),
                    kind: ExpectKind::RemainUsed,
                    prns: vec![e.prn.unwrap()],
                    mode: None,
                    t_ref_s: e.t_s as f64,
                    until_s: until,
                    limit_param: String::new(),
                    what: "a fast-correction step within the field: the satellite stays in use"
                        .into(),
                });
            }
            EventKind::Givei => ex.push(info(
                e,
                "grid point GIVEI changed; the equipment's grid use is not in the log format",
            )),
            EventKind::IodpChange => ex.push(info(
                e,
                "IODP changed with the mask resent; corrections tagged with the old IODP no \
                 longer apply",
            )),
        }
    }

    let mut lines = String::new();
    for f in &frames {
        lines.push_str(&format!(
            "{} {} {}\n",
            f.t_s,
            f.mt.map(|m| m.to_string()).unwrap_or_default(),
            f.hex.clone().unwrap_or_default()
        ));
    }
    Ok(Stream {
        format: OUTPUT_FORMAT.into(),
        label: super::LABEL.into(),
        spec_sha256: super::sha256_hex(spec_json.as_bytes()),
        frames_sha256: super::sha256_hex(lines.as_bytes()),
        frames,
        expectations: ex,
    })
}

fn info(e: &Event, what: &str) -> Expectation {
    Expectation {
        id: format!("{}-info", e.id),
        event: e.id.clone(),
        kind: ExpectKind::Informational,
        prns: vec![],
        mode: None,
        t_ref_s: e.t_s as f64,
        until_s: e.end_s().map(|x| x as f64),
        limit_param: String::new(),
        what: what.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> SequenceSpec {
        SequenceSpec {
            duration_s: 120,
            prns: vec![1, 3, 6, 9, 12, 14, 17, 19, 22, 25, 28, 31, 32, 2],
            iodp: 1,
            schedule: Schedule::default(),
            nominal: Nominal::default(),
            events: vec![Event {
                id: "dnu".into(),
                kind: EventKind::Udrei,
                t_s: 40,
                duration_s: Some(30),
                prn: Some(6),
                udrei: Some(15),
                givei: None,
                block: None,
                igp: None,
                iodp: None,
                delta_m: None,
            }],
        }
    }

    #[test]
    fn deterministic_and_decodable() {
        let a = generate(&spec()).unwrap();
        let b = generate(&spec()).unwrap();
        assert_eq!(a.frames_sha256, b.frames_sha256);
        assert_eq!(a.frames.len(), 120);
        for f in &a.frames {
            let fr = sbas_msg::Frame::from_hex(f.hex.as_ref().unwrap()).unwrap();
            sbas_msg::decode(&fr).unwrap();
        }
        // Two fast-correction blocks for 14 satellites.
        assert!(a.frames.iter().any(|f| f.mt == Some(3)));
    }

    #[test]
    fn alarm_is_sent_at_onset_with_iodf_3() {
        let s = generate(&spec()).unwrap();
        let f = &s.frames[40];
        assert_eq!(f.mt, Some(6));
        let m =
            sbas_msg::decode(&sbas_msg::Frame::from_hex(f.hex.as_ref().unwrap()).unwrap()).unwrap();
        match m {
            Message::Integrity { iodf, udrei } => {
                assert_eq!(iodf[0], 3);
                assert_eq!(udrei[2], 15);
                assert_eq!(udrei[0], 5);
            }
            _ => panic!(),
        }
        let e = &s.expectations[0];
        assert_eq!(e.kind, ExpectKind::Exclude);
        assert_eq!(e.t_ref_s, 40.0);
        assert_eq!(s.expectations[1].kind, ExpectKind::Include);
        assert_eq!(s.expectations[1].t_ref_s, 70.0);
    }

    #[test]
    fn withheld_block_is_not_sent() {
        let mut sp = spec();
        sp.events = vec![Event {
            id: "wf".into(),
            kind: EventKind::WithholdFast,
            t_s: 30,
            duration_s: Some(40),
            prn: Some(2),
            udrei: None,
            givei: None,
            block: None,
            igp: None,
            iodp: None,
            delta_m: None,
        }];
        let s = generate(&sp).unwrap();
        assert!(!s.frames[30..70].iter().any(|f| f.mt == Some(3)));
        assert!(s.frames[70..].iter().any(|f| f.mt == Some(3)));
        assert_eq!(s.expectations[0].prns, vec![2]);
        assert!(s.expectations[0].t_ref_s < 30.0);
    }

    #[test]
    fn bad_events_are_refused() {
        let mut sp = spec();
        sp.events[0].prn = Some(30);
        assert!(generate(&sp).is_err());
        let mut sp = spec();
        sp.events[0].t_s = 500;
        assert!(generate(&sp).is_err());
    }
}
