// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! SBAS L1 message framing and the message types the failure tests use.
//!
//! A frame is 250 bits, sent at one per second: an 8-bit preamble (one of three bytes that
//! rotate frame by frame), a 6-bit message type, 212 data bits and a 24-bit CRC-24Q parity
//! over the first 226 bits. The layout is the public SBAS L1 interface (the same frame the
//! open receivers decode); the procedures are modelled on RTCA DO-229F Appendix A, which the
//! applicant holds.
//!
//! Message types built here:
//!
//! | MT | content |
//! |----|---------|
//! | 0  | do not use this SBAS for safety applications (data field zero) |
//! | 1  | PRN mask (210 slots) and IODP |
//! | 2 to 5 | fast corrections (13 slots each: 12-bit, 0.125 m) and UDREI |
//! | 6  | integrity information: IODF of MT2 to MT5, UDREI for 51 slots |
//! | 7  | fast-correction degradation factor indicators and system latency |
//! | 26 | ionospheric grid delays (15 IGPs: 9-bit, 0.125 m) and GIVEI |
//! | 63 | null message |
//!
//! Frames are held as 32 bytes, most significant bit first, the last 6 bits zero, and
//! written as 64 hex characters.

use serde::{Deserialize, Serialize};

/// The three preamble bytes, in broadcast order.
pub const PREAMBLES: [u8; 3] = [0x53, 0x9A, 0xC6];
/// Bits in a frame.
pub const FRAME_BITS: usize = 250;
/// Bits covered by the parity.
pub const PARITY_COVERED_BITS: usize = 226;
/// Slots in the PRN mask.
pub const MASK_SLOTS: usize = 210;
/// Masked satellites one SBAS message stream can describe.
pub const MAX_MASKED: usize = 51;
/// UDREI value meaning "not monitored".
pub const UDREI_NOT_MONITORED: u8 = 14;
/// UDREI value meaning "do not use".
pub const UDREI_DO_NOT_USE: u8 = 15;
/// GIVEI value meaning "not monitored".
pub const GIVEI_NOT_MONITORED: u8 = 15;
/// Largest fast correction the 12-bit field carries (m).
pub const FC_MAX_M: f64 = 2047.0 * 0.125;
/// Largest grid delay the 9-bit field carries (m); the all-ones value means "do not use".
pub const IGP_DELAY_MAX_M: f64 = 510.0 * 0.125;

/// CRC-24Q (polynomial 0x1864CFB, initial value 0) over the first `nbits` bits of `bits`
/// (most significant bit first).
pub fn crc24q_bits(bits: &[u8], nbits: usize) -> u32 {
    let mut crc: u32 = 0;
    for i in 0..nbits {
        let b = ((bits[i / 8] >> (7 - (i % 8))) & 1) as u32;
        let top = ((crc >> 23) & 1) ^ b;
        crc = (crc << 1) & 0x00FF_FFFF;
        if top == 1 {
            crc ^= 0x0086_4CFB;
        }
    }
    crc
}

/// CRC-24Q over whole bytes.
pub fn crc24q(bytes: &[u8]) -> u32 {
    crc24q_bits(bytes, bytes.len() * 8)
}

/// A 250-bit frame buffer with bit-field access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame(pub [u8; 32]);

impl Frame {
    fn zero() -> Frame {
        Frame([0u8; 32])
    }

    /// Write the `n` low bits of `v` at bit offset `pos`.
    pub fn put(&mut self, pos: usize, n: usize, v: u32) {
        for k in 0..n {
            let bit = (v >> (n - 1 - k)) & 1;
            let i = pos + k;
            let mask = 1u8 << (7 - (i % 8));
            if bit == 1 {
                self.0[i / 8] |= mask;
            } else {
                self.0[i / 8] &= !mask;
            }
        }
    }

    /// Read `n` bits at bit offset `pos` as unsigned.
    pub fn get(&self, pos: usize, n: usize) -> u32 {
        let mut v = 0u32;
        for k in 0..n {
            let i = pos + k;
            v = (v << 1) | ((self.0[i / 8] >> (7 - (i % 8))) & 1) as u32;
        }
        v
    }

    /// Read `n` bits at `pos` as two's-complement signed.
    pub fn get_signed(&self, pos: usize, n: usize) -> i32 {
        let v = self.get(pos, n) as i64;
        if v >> (n - 1) & 1 == 1 {
            (v - (1i64 << n)) as i32
        } else {
            v as i32
        }
    }

    /// The preamble byte.
    pub fn preamble(&self) -> u8 {
        self.get(0, 8) as u8
    }

    /// The message type.
    pub fn message_type(&self) -> u8 {
        self.get(8, 6) as u8
    }

    /// The parity field as sent.
    pub fn parity(&self) -> u32 {
        self.get(PARITY_COVERED_BITS, 24)
    }

    /// Whether the parity matches the first 226 bits.
    pub fn parity_ok(&self) -> bool {
        crc24q_bits(&self.0, PARITY_COVERED_BITS) == self.parity()
    }

    /// Set the parity over the first 226 bits.
    fn seal(&mut self) {
        let crc = crc24q_bits(&self.0, PARITY_COVERED_BITS);
        self.put(PARITY_COVERED_BITS, 24, crc);
    }

    /// 64 lower-case hex characters.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse 64 hex characters.
    pub fn from_hex(s: &str) -> Result<Frame, String> {
        let b = hex::decode(s.trim()).map_err(|e| format!("frame hex: {e}"))?;
        let arr: [u8; 32] = b
            .try_into()
            .map_err(|_| "frame hex: expected 32 bytes (64 hex characters)".to_string())?;
        Ok(Frame(arr))
    }
}

/// The content of one SBAS message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// MT 0: do not use (data field zero).
    DoNotUse,
    /// MT 1: PRN mask (slot numbers, 1-based, ascending) and IODP.
    Mask { slots: Vec<u16>, iodp: u8 },
    /// MT 2 to 5: fast corrections and UDREI for 13 masked satellites from
    /// `first = 13·(mt−2)`.
    Fast {
        mt: u8,
        iodf: u8,
        iodp: u8,
        /// Fast corrections (m), 13 entries (0 for an unused entry).
        prc_m: Vec<f64>,
        /// UDREI, 13 entries.
        udrei: Vec<u8>,
    },
    /// MT 6: IODF of MT2 to MT5 and UDREI for 51 slots.
    Integrity { iodf: [u8; 4], udrei: Vec<u8> },
    /// MT 7: system latency (s), IODP and 51 degradation factor indicators.
    Degradation { t_lat_s: u8, iodp: u8, ai: Vec<u8> },
    /// MT 26: ionospheric delays for one band and block: 15 grid delays (m) with GIVEI.
    Iono {
        band: u8,
        block: u8,
        delay_m: Vec<f64>,
        givei: Vec<u8>,
        iodi: u8,
    },
    /// MT 63: null message.
    Null,
}

impl Message {
    /// The message type number.
    pub fn mt(&self) -> u8 {
        match self {
            Message::DoNotUse => 0,
            Message::Mask { .. } => 1,
            Message::Fast { mt, .. } => *mt,
            Message::Integrity { .. } => 6,
            Message::Degradation { .. } => 7,
            Message::Iono { .. } => 26,
            Message::Null => 63,
        }
    }
}

fn check_len<T>(what: &str, v: &[T], n: usize) -> Result<(), String> {
    if v.len() != n {
        return Err(format!("{what}: expected {n} entries, got {}", v.len()));
    }
    Ok(())
}

fn check_max(what: &str, v: u32, bits: usize) -> Result<(), String> {
    if v >= (1u32 << bits) {
        return Err(format!("{what}: {v} does not fit {bits} bits"));
    }
    Ok(())
}

fn quantise(what: &str, m: f64, lsb: f64, bits: usize, signed: bool) -> Result<u32, String> {
    if !m.is_finite() {
        return Err(format!("{what}: not a finite number"));
    }
    let q = (m / lsb).round() as i64;
    let (lo, hi) = if signed {
        (-(1i64 << (bits - 1)) + 1, (1i64 << (bits - 1)) - 1)
    } else {
        (0, (1i64 << bits) - 1)
    };
    if q < lo || q > hi {
        return Err(format!("{what}: {m} m is outside the {bits}-bit field"));
    }
    Ok((q & ((1i64 << bits) - 1)) as u32)
}

/// Encode `msg` as a sealed frame with preamble `PREAMBLES[seq % 3]`.
pub fn encode(msg: &Message, seq: u64) -> Result<Frame, String> {
    let mut f = Frame::zero();
    f.put(0, 8, PREAMBLES[(seq % 3) as usize] as u32);
    f.put(8, 6, msg.mt() as u32);
    match msg {
        Message::DoNotUse | Message::Null => {}
        Message::Mask { slots, iodp } => {
            if slots.len() > MAX_MASKED {
                return Err(format!(
                    "mask: {} slots set, at most {MAX_MASKED}",
                    slots.len()
                ));
            }
            check_max("iodp", *iodp as u32, 2)?;
            for &s in slots {
                if s == 0 || s as usize > MASK_SLOTS {
                    return Err(format!("mask: slot {s} outside 1..={MASK_SLOTS}"));
                }
                f.put(14 + s as usize - 1, 1, 1);
            }
            f.put(224, 2, *iodp as u32);
        }
        Message::Fast {
            mt,
            iodf,
            iodp,
            prc_m,
            udrei,
        } => {
            if !(2..=5).contains(mt) {
                return Err(format!("fast corrections: MT {mt} is not 2 to 5"));
            }
            check_max("iodf", *iodf as u32, 2)?;
            check_max("iodp", *iodp as u32, 2)?;
            check_len("fast corrections", prc_m, 13)?;
            check_len("udrei", udrei, 13)?;
            f.put(14, 2, *iodf as u32);
            f.put(16, 2, *iodp as u32);
            for (i, &p) in prc_m.iter().enumerate() {
                f.put(
                    18 + 12 * i,
                    12,
                    quantise("fast correction", p, 0.125, 12, true)?,
                );
            }
            for (i, &u) in udrei.iter().enumerate() {
                check_max("udrei", u as u32, 4)?;
                f.put(174 + 4 * i, 4, u as u32);
            }
        }
        Message::Integrity { iodf, udrei } => {
            check_len("udrei", udrei, MAX_MASKED)?;
            for (j, &x) in iodf.iter().enumerate() {
                check_max("iodf", x as u32, 2)?;
                f.put(14 + 2 * j, 2, x as u32);
            }
            for (i, &u) in udrei.iter().enumerate() {
                check_max("udrei", u as u32, 4)?;
                f.put(22 + 4 * i, 4, u as u32);
            }
        }
        Message::Degradation { t_lat_s, iodp, ai } => {
            check_max("t_lat", *t_lat_s as u32, 4)?;
            check_max("iodp", *iodp as u32, 2)?;
            check_len("ai", ai, MAX_MASKED)?;
            f.put(14, 4, *t_lat_s as u32);
            f.put(18, 2, *iodp as u32);
            for (i, &a) in ai.iter().enumerate() {
                check_max("ai", a as u32, 4)?;
                f.put(22 + 4 * i, 4, a as u32);
            }
        }
        Message::Iono {
            band,
            block,
            delay_m,
            givei,
            iodi,
        } => {
            check_max("band", *band as u32, 4)?;
            check_max("block", *block as u32, 4)?;
            check_max("iodi", *iodi as u32, 2)?;
            check_len("grid delays", delay_m, 15)?;
            check_len("givei", givei, 15)?;
            f.put(14, 4, *band as u32);
            f.put(18, 4, *block as u32);
            for i in 0..15 {
                check_max("givei", givei[i] as u32, 4)?;
                let d = if delay_m[i].is_nan() {
                    511 // "do not use"
                } else {
                    quantise("grid delay", delay_m[i], 0.125, 9, false)?.min(510)
                };
                f.put(22 + 13 * i, 9, d);
                f.put(31 + 13 * i, 4, givei[i] as u32);
            }
            f.put(217, 2, *iodi as u32);
        }
    }
    f.seal();
    Ok(f)
}

/// Decode a frame (parity checked first).
pub fn decode(f: &Frame) -> Result<Message, String> {
    if !PREAMBLES.contains(&f.preamble()) {
        return Err(format!(
            "preamble 0x{:02x} is not an SBAS preamble",
            f.preamble()
        ));
    }
    if !f.parity_ok() {
        return Err("parity (CRC-24Q) fails".into());
    }
    let mt = f.message_type();
    Ok(match mt {
        0 => Message::DoNotUse,
        63 => Message::Null,
        1 => Message::Mask {
            slots: (1..=MASK_SLOTS as u16)
                .filter(|s| f.get(14 + *s as usize - 1, 1) == 1)
                .collect(),
            iodp: f.get(224, 2) as u8,
        },
        2..=5 => Message::Fast {
            mt,
            iodf: f.get(14, 2) as u8,
            iodp: f.get(16, 2) as u8,
            prc_m: (0..13)
                .map(|i| f.get_signed(18 + 12 * i, 12) as f64 * 0.125)
                .collect(),
            udrei: (0..13).map(|i| f.get(174 + 4 * i, 4) as u8).collect(),
        },
        6 => Message::Integrity {
            iodf: [
                f.get(14, 2) as u8,
                f.get(16, 2) as u8,
                f.get(18, 2) as u8,
                f.get(20, 2) as u8,
            ],
            udrei: (0..MAX_MASKED)
                .map(|i| f.get(22 + 4 * i, 4) as u8)
                .collect(),
        },
        7 => Message::Degradation {
            t_lat_s: f.get(14, 4) as u8,
            iodp: f.get(18, 2) as u8,
            ai: (0..MAX_MASKED)
                .map(|i| f.get(22 + 4 * i, 4) as u8)
                .collect(),
        },
        26 => Message::Iono {
            band: f.get(14, 4) as u8,
            block: f.get(18, 4) as u8,
            delay_m: (0..15)
                .map(|i| match f.get(22 + 13 * i, 9) {
                    511 => f64::NAN,
                    d => d as f64 * 0.125,
                })
                .collect(),
            givei: (0..15).map(|i| f.get(31 + 13 * i, 4) as u8).collect(),
            iodi: f.get(217, 2) as u8,
        },
        other => {
            return Err(format!(
                "message type {other} is not decoded by this library"
            ))
        }
    })
}

/// The mask slot of a GPS PRN (1 to 37) or an SBAS PRN (120 to 158).
pub fn slot_of_prn(prn: u16) -> Result<u16, String> {
    match prn {
        1..=37 | 120..=158 => Ok(prn),
        _ => Err(format!(
            "PRN {prn}: this library masks GPS (1 to 37) and SBAS (120 to 158) PRNs"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc24q_check_value() {
        // The CRC-24Q check value of the ASCII string "123456789".
        assert_eq!(crc24q(b"123456789"), 0x00CD_E703);
    }

    #[test]
    fn round_trip_every_type() {
        let msgs = [
            Message::DoNotUse,
            Message::Null,
            Message::Mask {
                slots: vec![1, 3, 6, 37, 120, 158],
                iodp: 2,
            },
            Message::Fast {
                mt: 3,
                iodf: 1,
                iodp: 2,
                prc_m: (0..13).map(|i| i as f64 * 0.5 - 3.0).collect(),
                udrei: (0..13).map(|i| (i % 16) as u8).collect(),
            },
            Message::Integrity {
                iodf: [0, 1, 2, 3],
                udrei: (0..51).map(|i| (i % 16) as u8).collect(),
            },
            Message::Degradation {
                t_lat_s: 4,
                iodp: 1,
                ai: (0..51).map(|i| (i % 16) as u8).collect(),
            },
            Message::Iono {
                band: 9,
                block: 2,
                delay_m: (0..15).map(|i| i as f64 * 1.25).collect(),
                givei: (0..15).map(|i| i as u8).collect(),
                iodi: 3,
            },
        ];
        for (k, m) in msgs.iter().enumerate() {
            let f = encode(m, k as u64).unwrap();
            assert!(f.parity_ok());
            assert_eq!(f.preamble(), PREAMBLES[k % 3]);
            let back = decode(&Frame::from_hex(&f.to_hex()).unwrap()).unwrap();
            assert_eq!(&back, m, "MT {}", m.mt());
        }
    }

    #[test]
    fn a_flipped_bit_fails_parity() {
        let f = encode(&Message::Null, 0).unwrap();
        for bit in [0usize, 13, 100, 225, 226, 249 - 6] {
            let mut g = f;
            let v = g.get(bit, 1);
            g.put(bit, 1, v ^ 1);
            assert!(!g.parity_ok(), "bit {bit}");
        }
    }

    #[test]
    fn out_of_range_fields_are_refused() {
        assert!(encode(
            &Message::Fast {
                mt: 2,
                iodf: 0,
                iodp: 0,
                prc_m: vec![300.0; 13],
                udrei: vec![0; 13]
            },
            0
        )
        .is_err());
        assert!(encode(
            &Message::Mask {
                slots: (1..=52).collect(),
                iodp: 0
            },
            0
        )
        .is_err());
    }
}
