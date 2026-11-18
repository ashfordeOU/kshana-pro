// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receive-side LDC API for software receivers (`crate::rx`).
//!
//! 1. Per group: give [`LdcDecoder::decide`] the measured timing offset (ns) of each pulse
//!    from its nominal position, indexed by LDC pulse index (0..=7 group pulses, 8 = data
//!    pulse; NaN where not measured), and optionally the carrier phase offsets (rad, phase
//!    code removed). A common offset is removed first: the mean offset of the measured
//!    *unmodulated* group pulses (Eurofix: pulses 1–2; 9th pulse: pulses 1–8), and, for
//!    balanced tri-state, the mean of the modulated pulses (maximum-likelihood decision
//!    over the 128 patterns). [`LdcDecoder::offsets_from_emissions`] produces these inputs
//!    from simulated [`Emission`]s.
//! 2. Per stream: [`LdcDecoder::decode_symbols`] RS-decodes frames, checks frame CRCs,
//!    reassembles message blocks, checks CRC-24Q and returns messages with error counts.
//!    The stream must start on a frame boundary (group number divisible by
//!    [`LdcDecoder::groups_per_frame`]; [`LdcDecoder::align`] finds it otherwise).
//! 3. [`LdcDecoder::verify_auth`] checks authentication tags with the shared key.

use super::hmac::{hmac_sha256, tags_equal};
use super::mapper::{Mapper, DATA_PULSE, LDC_PULSES};
use super::modulator::{auth_input, DataPulse, FrameCodec, FrameReport};
use super::msg::{parse_block, Message, ParseError, MSG_CRC_BITS};
use super::LdcConfig;
use crate::chain::Emission;
use crate::signal::{Parity, PulseSignal, Role};

/// Maximum frames a single message may span (guards garbage parses).
const MAX_MSG_FRAMES: usize = 64;

/// A decoded message.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodedMessage {
    pub message: Message,
    /// Index (in the decoded stream) of its first frame, and its frame count.
    pub first_frame: usize,
    pub frames: usize,
    /// RS symbols corrected over its frames.
    pub corrected_symbols: usize,
    /// Type + body bits (no CRC), as received.
    pub body_bits: Vec<bool>,
}

/// Result of decoding a symbol stream.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DecodeReport {
    pub frames: Vec<FrameReport>,
    pub messages: Vec<DecodedMessage>,
    pub frames_ok: usize,
    /// Frames whose RS decoding or frame CRC failed.
    pub frames_failed: usize,
    pub symbols_corrected: usize,
    /// Candidate message blocks rejected by CRC-24Q.
    pub crc_failures: usize,
}

/// Authentication check of one decoded auth message.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthCheck {
    /// Index into [`DecodeReport::messages`].
    pub message: usize,
    /// `Some(ok)` when every covered message was decoded; `None` otherwise.
    pub verified: Option<bool>,
}

/// LDC demodulator/decoder of one scheme.
#[derive(Clone, Debug)]
pub struct LdcDecoder {
    pub name: String,
    pub codec: FrameCodec,
    pub mapper: Mapper,
    pub data_pulse: Option<DataPulse>,
    auth_key: Vec<u8>,
}

impl LdcDecoder {
    /// Decoder for `cfg` (`None` for no modulation).
    pub fn new(cfg: &LdcConfig) -> Result<Option<LdcDecoder>, String> {
        Ok(cfg.compile()?.map(|c| LdcDecoder {
            name: c.name,
            codec: c.codec,
            mapper: c.mapper,
            data_pulse: c.data_pulse,
            auth_key: c.auth_key,
        }))
    }

    pub fn bits_per_symbol(&self) -> u32 {
        self.mapper.bits_per_symbol()
    }
    pub fn groups_per_frame(&self) -> usize {
        self.codec.groups_per_frame()
    }

    fn unmodulated_group_pulses(&self) -> impl Iterator<Item = usize> + '_ {
        (0..8).filter(|i| !self.mapper.pulses().contains(i))
    }

    /// Decide the channel symbol of one group (see the module docs for the inputs).
    pub fn decide(&self, offsets_ns: &[f64], phases_rad: Option<&[f64]>) -> u32 {
        let at = |v: &[f64], i: usize| v.get(i).copied().unwrap_or(f64::NAN);
        let refs: Vec<f64> = self.unmodulated_group_pulses().map(|i| at(offsets_ns, i)).filter(|x| x.is_finite()).collect();
        let r = if refs.is_empty() { 0.0 } else { refs.iter().sum::<f64>() / refs.len() as f64 };
        let x: Vec<f64> = (0..LDC_PULSES).map(|i| at(offsets_ns, i) - r).collect();
        let ph = phases_rad.map(|p| {
            let (s, c) = self
                .unmodulated_group_pulses()
                .map(|i| at(p, i))
                .filter(|x| x.is_finite())
                .fold((0.0, 0.0), |(s, c), a| (s + a.sin(), c + a.cos()));
            let pr = if s == 0.0 && c == 0.0 { 0.0 } else { s.atan2(c) };
            (0..LDC_PULSES).map(|i| at(p, i) - pr).collect::<Vec<f64>>()
        });
        self.mapper.decide(&x, ph.as_deref())
    }

    /// Per-LDC-index timing offsets (ns) and phase offsets (rad) of one station's group from
    /// its emissions (any order, one station, one group): each pulse's emission time minus
    /// its nominal position, less the mean of the unmodulated group pulses, so the
    /// station's clock error cancels — what a receiver tracking the group measures. Without
    /// unmodulated pulses a common offset remains (balanced decisions remove it).
    pub fn offsets_from_emissions(
        &self,
        signal: &PulseSignal,
        role: Role,
        group: &[Emission],
    ) -> ([f64; LDC_PULSES], [f64; LDC_PULSES]) {
        let mut t = [f64::NAN; LDC_PULSES];
        let mut p = [f64::NAN; LDC_PULSES];
        let Some(first) = group.first() else { return (t, p) };
        let parity = Parity::of_group(first.group);
        let nominal = signal.group(role, parity);
        let n_nom = nominal.len();
        let mut raw = [f64::NAN; LDC_PULSES];
        for e in group {
            let k = e.pulse.k;
            let (idx, nom_ps, nom_phase) = if k < 8 && k < n_nom {
                (k, nominal[k].offset, nominal[k].phase_rad)
            } else if k >= n_nom {
                let Some(dp) = self.data_pulse else { continue };
                let Some(p8) = nominal.iter().find(|q| q.k == 7) else { continue };
                let code = match parity {
                    Parity::A => dp.code_a,
                    Parity::B => dp.code_b,
                };
                (
                    DATA_PULSE,
                    p8.offset + crate::time::Ps::from_us(dp.after(role)),
                    if code < 0 { std::f64::consts::PI } else { 0.0 },
                )
            } else {
                continue; // master identification pulse
            };
            // arrival relative to an arbitrary origin (the first emission) minus nominal;
            // the common part is removed with the reference below
            raw[idx] = (e.t_emit - first.t_emit - nom_ps).as_ns();
            let d = e.pulse.phase_rad - nom_phase;
            p[idx] = d.sin().atan2(d.cos());
        }
        // reference: the true group start as a receiver would track it on unmodulated pulses
        let refs: Vec<f64> = self.unmodulated_group_pulses().map(|i| raw[i]).filter(|x| x.is_finite()).collect();
        let r = if refs.is_empty() { 0.0 } else { refs.iter().sum::<f64>() / refs.len() as f64 };
        for i in 0..LDC_PULSES {
            t[i] = raw[i] - r;
        }
        (t, p)
    }

    /// Decode one frame's channel symbols.
    pub fn decode_frame(&self, symbols: &[u32]) -> FrameReport {
        self.codec.decode_frame(symbols)
    }

    /// Decode a frame-aligned symbol stream into messages.
    pub fn decode_symbols(&self, symbols: &[u32]) -> DecodeReport {
        let g = self.groups_per_frame();
        let frames: Vec<FrameReport> = symbols.chunks_exact(g).map(|c| self.codec.decode_frame(c)).collect();
        let mut rep = DecodeReport {
            frames_ok: frames.iter().filter(|f| f.data.is_some()).count(),
            frames_failed: frames.iter().filter(|f| f.data.is_none()).count(),
            symbols_corrected: frames.iter().map(|f| f.corrected).sum(),
            ..Default::default()
        };
        let mut i = 0;
        'outer: while i < frames.len() {
            let mut buf: Vec<bool> = Vec::new();
            let mut corrected = 0;
            let mut j = i;
            loop {
                // a failed frame, or the end of the stream (an incomplete message, or a
                // continuation frame misread as a start): try the next frame as a start
                let Some(d) = frames.get(j).and_then(|f| f.data.as_ref()) else {
                    i += 1;
                    continue 'outer;
                };
                buf.extend_from_slice(d);
                corrected += frames[j].corrected;
                j += 1;
                match parse_block(&buf) {
                    Ok((m, used, true)) => {
                        rep.messages.push(DecodedMessage {
                            message: m,
                            first_frame: i,
                            frames: j - i,
                            corrected_symbols: corrected,
                            body_bits: buf[..used - MSG_CRC_BITS].to_vec(),
                        });
                        i = j;
                        continue 'outer;
                    }
                    Ok((_, _, false)) => {
                        rep.crc_failures += 1;
                        i += 1;
                        continue 'outer;
                    }
                    Err(ParseError::NeedMore) if j - i < MAX_MSG_FRAMES => {}
                    Err(_) => {
                        i += 1;
                        continue 'outer;
                    }
                }
            }
        }
        rep.frames = frames;
        rep
    }

    /// Symbol offset (0..groups_per_frame) at which frames decode best, if any decodes.
    pub fn align(&self, symbols: &[u32]) -> Option<usize> {
        let g = self.groups_per_frame();
        (0..g)
            .map(|o| {
                let ok = symbols.get(o..).unwrap_or(&[]).chunks_exact(g).filter(|c| self.codec.decode_frame(c).data.is_some()).count();
                (ok, o)
            })
            .filter(|(ok, _)| *ok > 0)
            .max_by_key(|(ok, o)| (*ok, std::cmp::Reverse(*o)))
            .map(|(_, o)| o)
    }

    /// Verify every auth message of `rep` for `station`, the stream's first frame being
    /// absolute frame `first_frame` (group number / groups per frame).
    pub fn verify_auth(&self, station: usize, rep: &DecodeReport, first_frame: i64) -> Vec<AuthCheck> {
        let mut out = Vec::new();
        for (a, dm) in rep.messages.iter().enumerate() {
            let Message::Auth { covered, tag, .. } = &dm.message else { continue };
            let c = *covered as usize;
            let mut verified = None;
            if a >= c {
                let cov = &rep.messages[a - c..a];
                let contiguous = cov
                    .iter()
                    .chain(std::iter::once(dm))
                    .collect::<Vec<_>>()
                    .windows(2)
                    .all(|w| w[0].first_frame + w[0].frames == w[1].first_frame);
                if contiguous {
                    let blocks: Vec<Vec<bool>> = cov.iter().map(|m| m.body_bits.clone()).collect();
                    let mac = hmac_sha256(&self.auth_key, &auth_input(station, first_frame + dm.first_frame as i64, &blocks));
                    verified = Some(tags_equal(&mac[..tag.len()], tag));
                }
            }
            out.push(AuthCheck { message: a, verified });
        }
        out
    }
}
