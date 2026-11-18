// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! LDC message types and their bit layouts.
//!
//! **These layouts are Kshana's own definitions** (the task of carrying station ID, time,
//! almanac, integrity, differential LORAN/GNSS corrections and authentication follows the
//! cited eLoran LDC literature; the bit fields below are not taken from any standard and
//! are MODELLED until replaced by the governing standard's layouts).
//!
//! A message is `[type: 4 bits][body]`; the *message block* that the link layer carries is
//! the message followed by its CRC-24Q (24 bits) computed over type and body. All fields are
//! MSB first; signed fields are two's complement and saturate at their range.
//!
//! | type | name        | body (bits)                                                                 | body len |
//! |------|-------------|-----------------------------------------------------------------------------|----------|
//! | 0    | null        | —                                                                           | 0        |
//! | 1    | station_id  | chain_id 8, gri/10 µs 14, station_code 8, secondary 1, signal_version 4, station_index 3 | 38 |
//! | 2    | utc_time    | loran_seconds 40, sub-second (10 µs) 17, TAI−UTC (s, signed) 8, leap_pending 2, days_to_leap 10 | 77 |
//! | 3    | almanac     | station_code 8, gri/10 µs 14, secondary 1, lat (1e-5°, s) 25, lon (1e-5°, s) 26, emission delay (10 ns) 24, power kW 12, healthy 1 | 111 |
//! | 4    | warning     | station_code 8, kind 4, magnitude ns (s) 16, validity s 16                  | 44       |
//! | 5    | dloran      | reference_site 12, age s 8, count 2, 3 × (station_code 8, ToA correction ns (s) 16) | 94 |
//! | 6    | dgnss       | constellation 2, modified Z-count (0.6 s) 13, count 2, 3 × (PRN 6, IODE 8, PRC (0.02 m, s) 16, RRC (0.002 m/s, s) 8, UDRE 2) | 137 |
//! | 7    | auth        | key_id 4, covered 6, tag_len−1 (bytes) 5, tag 8·tag_len                      | 15+8L    |
//! | 8    | raw         | len (bytes) 8, data 8·len                                                   | 8+8L     |
//!
//! *Time*: `loran_seconds` and `sub_second_10us` give the start of the message's first group
//! (the chain's master group epoch) in the continuous Loran time scale counted from
//! 1958-01-01T00:00:00 (this crate's [`crate::time::AbsTime`] scale). `leap_pending`: 0 none,
//! 1 positive leap second, 2 negative, 3 reserved; `days_to_leap` days until it applies.
//!
//! *DGNSS*: an RTCM SC-104 type-1-like compact form (PRC/RRC/UDRE/IODE per satellite).
//!
//! *Authentication*: `tag` = the first `tag_len` bytes of HMAC-SHA256(key, station index
//! (u16 BE) ‖ first frame number of the auth message (i64 BE) ‖ for each of the `covered`
//! preceding messages of this station, its packed type+body bits). See
//! [`crate::ldc::modulator`].

use super::bits::{BitReader, BitWriter};
use super::crc::crc24q_bits;
use crate::signal::Role;
use serde::{Deserialize, Serialize};

/// Width of the message type field.
pub const TYPE_BITS: u32 = 4;
/// Width of the message CRC-24Q.
pub const MSG_CRC_BITS: usize = 24;

/// Warning / integrity kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    /// Clears an earlier warning.
    #[default]
    Clear,
    /// Station unusable (do not use for navigation or timing).
    Unusable,
    /// Propagation anomaly (e.g. ASF change) of `magnitude_ns`.
    PropagationAnomaly,
    /// Station timing degraded by `magnitude_ns`.
    TimingDegraded,
    /// Data channel under test: do not use LDC content.
    LdcTest,
}

impl WarningKind {
    fn code(self) -> u64 {
        match self {
            WarningKind::Clear => 0,
            WarningKind::Unusable => 1,
            WarningKind::PropagationAnomaly => 2,
            WarningKind::TimingDegraded => 3,
            WarningKind::LdcTest => 4,
        }
    }
    fn from_code(c: u64) -> Option<WarningKind> {
        Some(match c {
            0 => WarningKind::Clear,
            1 => WarningKind::Unusable,
            2 => WarningKind::PropagationAnomaly,
            3 => WarningKind::TimingDegraded,
            4 => WarningKind::LdcTest,
            _ => return None,
        })
    }
}

/// One differential-LORAN correction (decoded units).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DloranEntry {
    pub station_code: u8,
    /// ToA correction (ns) to add to the measured ToA.
    pub correction_ns: i16,
}

/// One DGNSS satellite correction (raw field units).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DgnssSat {
    pub prn: u8,
    pub iode: u8,
    /// Pseudorange correction, 0.02 m units.
    pub prc_002m: i16,
    /// Range-rate correction, 0.002 m/s units.
    pub rrc_0002ms: i8,
    pub udre: u8,
}

/// A decoded (or to-be-transmitted) message, in field units.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Null,
    StationId {
        chain_id: u8,
        gri_10us: u16,
        station_code: u8,
        secondary: bool,
        signal_version: u8,
        station_index: u8,
    },
    UtcTime {
        loran_seconds: u64,
        sub_second_10us: u32,
        tai_minus_utc_s: i8,
        leap_pending: u8,
        days_to_leap: u16,
    },
    Almanac {
        station_code: u8,
        gri_10us: u16,
        secondary: bool,
        lat_1e5deg: i32,
        lon_1e5deg: i32,
        emission_delay_10ns: u32,
        power_kw: u16,
        healthy: bool,
    },
    Warning {
        station_code: u8,
        kind: WarningKind,
        magnitude_ns: i16,
        valid_s: u16,
    },
    Dloran {
        reference_site: u16,
        age_s: u8,
        entries: Vec<DloranEntry>,
    },
    Dgnss {
        constellation: u8,
        z_count: u16,
        sats: Vec<DgnssSat>,
    },
    Auth {
        key_id: u8,
        covered: u8,
        tag: Vec<u8>,
    },
    Raw {
        data: Vec<u8>,
    },
}

/// Why a message failed to parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// More bits are needed (the message continues in the next frame).
    NeedMore,
    /// The bits cannot be a valid message.
    Invalid,
}

impl Message {
    pub fn type_code(&self) -> u8 {
        match self {
            Message::Null => 0,
            Message::StationId { .. } => 1,
            Message::UtcTime { .. } => 2,
            Message::Almanac { .. } => 3,
            Message::Warning { .. } => 4,
            Message::Dloran { .. } => 5,
            Message::Dgnss { .. } => 6,
            Message::Auth { .. } => 7,
            Message::Raw { .. } => 8,
        }
    }

    /// Type and body bits (no CRC).
    pub fn to_bits(&self) -> Vec<bool> {
        let mut w = BitWriter::new();
        w.put(self.type_code() as u64, TYPE_BITS);
        match self {
            Message::Null => {}
            Message::StationId { chain_id, gri_10us, station_code, secondary, signal_version, station_index } => {
                w.put(*chain_id as u64, 8);
                w.put(*gri_10us as u64, 14);
                w.put(*station_code as u64, 8);
                w.put(*secondary as u64, 1);
                w.put(*signal_version as u64, 4);
                w.put(*station_index as u64, 3);
            }
            Message::UtcTime { loran_seconds, sub_second_10us, tai_minus_utc_s, leap_pending, days_to_leap } => {
                w.put(*loran_seconds, 40);
                w.put(*sub_second_10us as u64, 17);
                w.put_signed(*tai_minus_utc_s as i64, 8);
                w.put(*leap_pending as u64, 2);
                w.put((*days_to_leap).min(1023) as u64, 10);
            }
            Message::Almanac {
                station_code,
                gri_10us,
                secondary,
                lat_1e5deg,
                lon_1e5deg,
                emission_delay_10ns,
                power_kw,
                healthy,
            } => {
                w.put(*station_code as u64, 8);
                w.put(*gri_10us as u64, 14);
                w.put(*secondary as u64, 1);
                w.put_signed(*lat_1e5deg as i64, 25);
                w.put_signed(*lon_1e5deg as i64, 26);
                w.put((*emission_delay_10ns).min((1 << 24) - 1) as u64, 24);
                w.put((*power_kw).min(4095) as u64, 12);
                w.put(*healthy as u64, 1);
            }
            Message::Warning { station_code, kind, magnitude_ns, valid_s } => {
                w.put(*station_code as u64, 8);
                w.put(kind.code(), 4);
                w.put_signed(*magnitude_ns as i64, 16);
                w.put(*valid_s as u64, 16);
            }
            Message::Dloran { reference_site, age_s, entries } => {
                w.put(*reference_site as u64, 12);
                w.put(*age_s as u64, 8);
                w.put(entries.len().min(3) as u64, 2);
                for i in 0..3 {
                    let e = entries.get(i).copied().unwrap_or(DloranEntry { station_code: 0, correction_ns: 0 });
                    w.put(e.station_code as u64, 8);
                    w.put_signed(e.correction_ns as i64, 16);
                }
            }
            Message::Dgnss { constellation, z_count, sats } => {
                w.put(*constellation as u64, 2);
                w.put(*z_count as u64, 13);
                w.put(sats.len().min(3) as u64, 2);
                for i in 0..3 {
                    let s = sats.get(i).copied().unwrap_or(DgnssSat { prn: 0, iode: 0, prc_002m: 0, rrc_0002ms: 0, udre: 0 });
                    w.put(s.prn as u64, 6);
                    w.put(s.iode as u64, 8);
                    w.put_signed(s.prc_002m as i64, 16);
                    w.put_signed(s.rrc_0002ms as i64, 8);
                    w.put(s.udre as u64, 2);
                }
            }
            Message::Auth { key_id, covered, tag } => {
                w.put(*key_id as u64, 4);
                w.put((*covered).min(63) as u64, 6);
                let l = tag.len().clamp(1, 32);
                w.put(l as u64 - 1, 5);
                let mut t = tag.clone();
                t.resize(l, 0);
                w.put_bytes(&t);
            }
            Message::Raw { data } => {
                let l = data.len().min(255);
                w.put(l as u64, 8);
                w.put_bytes(&data[..l]);
            }
        }
        w.bits
    }

    /// Message block: type + body + CRC-24Q.
    pub fn to_block(&self) -> Vec<bool> {
        let mut b = self.to_bits();
        let c = crc24q_bits(&b);
        let mut w = BitWriter::new();
        w.put(c as u64, MSG_CRC_BITS as u32);
        b.extend(w.bits);
        b
    }

    /// Parse type + body from the start of `bits`; returns the message and bits consumed.
    pub fn parse(bits: &[bool]) -> Result<(Message, usize), ParseError> {
        let mut r = BitReader::new(bits);
        let m = Self::parse_reader(&mut r)?;
        Ok((m, r.pos))
    }

    fn parse_reader(r: &mut BitReader) -> Result<Message, ParseError> {
        use ParseError::*;
        let g = |r: &mut BitReader, n| r.get(n).ok_or(NeedMore);
        let gs = |r: &mut BitReader, n| r.get_signed(n).ok_or(NeedMore);
        let t = g(r, TYPE_BITS)?;
        Ok(match t {
            0 => Message::Null,
            1 => Message::StationId {
                chain_id: g(r, 8)? as u8,
                gri_10us: g(r, 14)? as u16,
                station_code: g(r, 8)? as u8,
                secondary: g(r, 1)? == 1,
                signal_version: g(r, 4)? as u8,
                station_index: g(r, 3)? as u8,
            },
            2 => {
                let m = Message::UtcTime {
                    loran_seconds: g(r, 40)?,
                    sub_second_10us: g(r, 17)? as u32,
                    tai_minus_utc_s: gs(r, 8)? as i8,
                    leap_pending: g(r, 2)? as u8,
                    days_to_leap: g(r, 10)? as u16,
                };
                if let Message::UtcTime { sub_second_10us, .. } = m {
                    if sub_second_10us >= 100_000 {
                        return Err(Invalid);
                    }
                }
                m
            }
            3 => Message::Almanac {
                station_code: g(r, 8)? as u8,
                gri_10us: g(r, 14)? as u16,
                secondary: g(r, 1)? == 1,
                lat_1e5deg: gs(r, 25)? as i32,
                lon_1e5deg: gs(r, 26)? as i32,
                emission_delay_10ns: g(r, 24)? as u32,
                power_kw: g(r, 12)? as u16,
                healthy: g(r, 1)? == 1,
            },
            4 => {
                let station_code = g(r, 8)? as u8;
                let kind = WarningKind::from_code(g(r, 4)?);
                let magnitude_ns = gs(r, 16)? as i16;
                let valid_s = g(r, 16)? as u16;
                Message::Warning { station_code, kind: kind.ok_or(Invalid)?, magnitude_ns, valid_s }
            }
            5 => {
                let reference_site = g(r, 12)? as u16;
                let age_s = g(r, 8)? as u8;
                let count = g(r, 2)? as usize;
                let mut entries = Vec::new();
                for i in 0..3 {
                    let e = DloranEntry { station_code: g(r, 8)? as u8, correction_ns: gs(r, 16)? as i16 };
                    if i < count {
                        entries.push(e);
                    }
                }
                Message::Dloran { reference_site, age_s, entries }
            }
            6 => {
                let constellation = g(r, 2)? as u8;
                let z_count = g(r, 13)? as u16;
                let count = g(r, 2)? as usize;
                let mut sats = Vec::new();
                for i in 0..3 {
                    let s = DgnssSat {
                        prn: g(r, 6)? as u8,
                        iode: g(r, 8)? as u8,
                        prc_002m: gs(r, 16)? as i16,
                        rrc_0002ms: gs(r, 8)? as i8,
                        udre: g(r, 2)? as u8,
                    };
                    if i < count {
                        sats.push(s);
                    }
                }
                Message::Dgnss { constellation, z_count, sats }
            }
            7 => {
                let key_id = g(r, 4)? as u8;
                let covered = g(r, 6)? as u8;
                let l = g(r, 5)? as usize + 1;
                Message::Auth { key_id, covered, tag: r.get_bytes(l).ok_or(NeedMore)? }
            }
            8 => {
                let l = g(r, 8)? as usize;
                Message::Raw { data: r.get_bytes(l).ok_or(NeedMore)? }
            }
            _ => return Err(Invalid),
        })
    }
}

/// Parse a message block (message + CRC-24Q) from the start of `bits`.
/// Returns the message, the block length and whether the CRC matched.
pub fn parse_block(bits: &[bool]) -> Result<(Message, usize, bool), ParseError> {
    let (m, used) = Message::parse(bits)?;
    if bits.len() < used + MSG_CRC_BITS {
        return Err(ParseError::NeedMore);
    }
    let mut r = BitReader::new(&bits[used..used + MSG_CRC_BITS]);
    let rx_crc = r.get(MSG_CRC_BITS as u32).unwrap() as u32;
    Ok((m, used + MSG_CRC_BITS, rx_crc == crc24q_bits(&bits[..used])))
}

/// Context in which a scheduled message is rendered into a [`Message`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MsgContext {
    pub station: usize,
    pub role: Role,
    /// Number (since the Loran epoch) of the message's first group.
    pub first_group: i64,
    /// GRI of the chain (µs), if configured.
    pub gri_us: Option<u32>,
}

impl MsgContext {
    /// Loran-time-scale instant of the first group in units of 10 µs (needs the GRI).
    pub fn start_10us(&self) -> Option<i128> {
        self.gri_us.map(|g| self.first_group as i128 * (g as i128 / 10))
    }
}

fn d_tai_utc() -> i8 {
    37
}
fn d_true() -> bool {
    true
}
fn d_tag_bytes() -> u8 {
    8
}

/// Configuration-side differential LORAN entry (ns).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DloranSpecEntry {
    pub station_code: u8,
    pub correction_ns: f64,
}

/// Configuration-side DGNSS satellite (SI units).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DgnssSpecSat {
    pub prn: u8,
    #[serde(default)]
    pub iode: u8,
    /// Pseudorange correction (m).
    pub prc_m: f64,
    /// Range-rate correction (m/s).
    #[serde(default)]
    pub rrc_m_s: f64,
    #[serde(default)]
    pub udre: u8,
}

/// A message as configured in a scenario (human units, with fields filled from context).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageSpec {
    Null,
    /// Station / chain identity. `station_code` defaults to `b'A' + station index`, the GRI
    /// to the LDC config's `gri_us`, role and index from the station.
    StationId {
        #[serde(default)]
        chain_id: u8,
        #[serde(default)]
        station_code: Option<u8>,
        #[serde(default)]
        signal_version: u8,
    },
    /// Time of the message's first group (needs `gri_us` in the LDC config) and leap info.
    UtcTime {
        #[serde(default = "d_tai_utc")]
        tai_minus_utc_s: i8,
        #[serde(default)]
        leap_pending: u8,
        #[serde(default)]
        days_to_leap: u16,
    },
    /// One almanac entry (one station per message; schedule several for a full almanac).
    Almanac {
        station_code: u8,
        gri_us: u32,
        #[serde(default)]
        secondary: bool,
        lat_deg: f64,
        lon_deg: f64,
        #[serde(default)]
        emission_delay_us: f64,
        #[serde(default)]
        power_kw: u16,
        #[serde(default = "d_true")]
        healthy: bool,
    },
    Warning {
        station_code: u8,
        kind: WarningKind,
        #[serde(default)]
        magnitude_ns: f64,
        #[serde(default)]
        valid_s: u16,
    },
    /// Up to 3 per-station ToA corrections.
    Dloran {
        #[serde(default)]
        reference_site: u16,
        #[serde(default)]
        age_s: u8,
        corrections: Vec<DloranSpecEntry>,
    },
    /// Up to 3 satellites. `z_count` defaults to the message time (needs `gri_us`).
    Dgnss {
        #[serde(default)]
        constellation: u8,
        #[serde(default)]
        z_count: Option<u16>,
        sats: Vec<DgnssSpecSat>,
    },
    /// Truncated HMAC-SHA256 over the preceding messages (since the previous auth message
    /// of the cycle, or the cycle start) of this station.
    Auth {
        #[serde(default)]
        key_id: u8,
        #[serde(default = "d_tag_bytes")]
        tag_bytes: u8,
    },
    /// Arbitrary payload (hex).
    Raw { hex: String },
}

impl MessageSpec {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            MessageSpec::Dloran { corrections, .. } if corrections.len() > 3 => {
                Err("dloran message carries at most 3 corrections (schedule several)".into())
            }
            MessageSpec::Dgnss { sats, .. } if sats.len() > 3 => {
                Err("dgnss message carries at most 3 satellites (schedule several)".into())
            }
            MessageSpec::Auth { tag_bytes, .. } if !(1..=32).contains(tag_bytes) => {
                Err("auth tag_bytes must be 1..=32".into())
            }
            MessageSpec::Raw { hex: h } => {
                let b = hex::decode(h).map_err(|e| format!("raw message hex: {e}"))?;
                if b.len() > 255 {
                    return Err("raw message is at most 255 bytes".into());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Render to a [`Message`]. The auth tag is left zero (filled by the scheduler).
    pub fn render(&self, ctx: &MsgContext) -> Message {
        match self {
            MessageSpec::Null => Message::Null,
            MessageSpec::StationId { chain_id, station_code, signal_version } => Message::StationId {
                chain_id: *chain_id,
                gri_10us: ctx.gri_us.map(|g| (g / 10) as u16).unwrap_or(0),
                station_code: station_code.unwrap_or(b'A'.wrapping_add(ctx.station as u8)),
                secondary: ctx.role == Role::Secondary,
                signal_version: *signal_version,
                station_index: (ctx.station & 7) as u8,
            },
            MessageSpec::UtcTime { tai_minus_utc_s, leap_pending, days_to_leap } => {
                let t = ctx.start_10us().unwrap_or(0);
                Message::UtcTime {
                    loran_seconds: (t.div_euclid(100_000) as u64) & ((1 << 40) - 1),
                    sub_second_10us: t.rem_euclid(100_000) as u32,
                    tai_minus_utc_s: *tai_minus_utc_s,
                    leap_pending: *leap_pending & 3,
                    days_to_leap: *days_to_leap,
                }
            }
            MessageSpec::Almanac {
                station_code,
                gri_us,
                secondary,
                lat_deg,
                lon_deg,
                emission_delay_us,
                power_kw,
                healthy,
            } => Message::Almanac {
                station_code: *station_code,
                gri_10us: (gri_us / 10) as u16,
                secondary: *secondary,
                lat_1e5deg: (lat_deg * 1e5).round() as i32,
                lon_1e5deg: (lon_deg * 1e5).round() as i32,
                emission_delay_10ns: (emission_delay_us * 100.0).round().max(0.0) as u32,
                power_kw: *power_kw,
                healthy: *healthy,
            },
            MessageSpec::Warning { station_code, kind, magnitude_ns, valid_s } => Message::Warning {
                station_code: *station_code,
                kind: *kind,
                magnitude_ns: magnitude_ns.round().clamp(-32768.0, 32767.0) as i16,
                valid_s: *valid_s,
            },
            MessageSpec::Dloran { reference_site, age_s, corrections } => Message::Dloran {
                reference_site: reference_site & 0xFFF,
                age_s: *age_s,
                entries: corrections
                    .iter()
                    .take(3)
                    .map(|c| DloranEntry {
                        station_code: c.station_code,
                        correction_ns: c.correction_ns.round().clamp(-32768.0, 32767.0) as i16,
                    })
                    .collect(),
            },
            MessageSpec::Dgnss { constellation, z_count, sats } => Message::Dgnss {
                constellation: constellation & 3,
                // modified Z-count: 0.6 s units within the hour
                z_count: z_count.unwrap_or_else(|| {
                    let t = ctx.start_10us().unwrap_or(0);
                    ((t.rem_euclid(360_000_000)) / 60_000) as u16
                }) % 6000,
                sats: sats
                    .iter()
                    .take(3)
                    .map(|s| DgnssSat {
                        prn: s.prn & 63,
                        iode: s.iode,
                        prc_002m: (s.prc_m / 0.02).round().clamp(-32768.0, 32767.0) as i16,
                        rrc_0002ms: (s.rrc_m_s / 0.002).round().clamp(-128.0, 127.0) as i8,
                        udre: s.udre & 3,
                    })
                    .collect(),
            },
            MessageSpec::Auth { key_id, tag_bytes } => Message::Auth {
                key_id: key_id & 15,
                covered: 0,
                tag: vec![0; (*tag_bytes).clamp(1, 32) as usize],
            },
            MessageSpec::Raw { hex: h } => Message::Raw { data: hex::decode(h).unwrap_or_default() },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Message> {
        vec![
            Message::Null,
            Message::StationId {
                chain_id: 7,
                gri_10us: 9930,
                station_code: b'W',
                secondary: true,
                signal_version: 2,
                station_index: 1,
            },
            Message::UtcTime {
                loran_seconds: 2_163_000_000,
                sub_second_10us: 99_999,
                tai_minus_utc_s: 37,
                leap_pending: 1,
                days_to_leap: 180,
            },
            Message::Almanac {
                station_code: b'M',
                gri_10us: 6731,
                secondary: false,
                lat_1e5deg: -3_312_345,
                lon_1e5deg: -17_999_999,
                emission_delay_10ns: 2_900_000,
                power_kw: 250,
                healthy: true,
            },
            Message::Warning { station_code: b'X', kind: WarningKind::PropagationAnomaly, magnitude_ns: -120, valid_s: 600 },
            Message::Dloran {
                reference_site: 42,
                age_s: 10,
                entries: vec![
                    DloranEntry { station_code: b'M', correction_ns: -35 },
                    DloranEntry { station_code: b'X', correction_ns: 412 },
                ],
            },
            Message::Dgnss {
                constellation: 0,
                z_count: 5999,
                sats: vec![DgnssSat { prn: 17, iode: 33, prc_002m: -1234, rrc_0002ms: 5, udre: 1 }],
            },
            Message::Auth { key_id: 3, covered: 4, tag: vec![1, 2, 3, 4, 5, 6, 7, 8] },
            Message::Raw { data: b"hello".to_vec() },
        ]
    }

    #[test]
    fn every_type_roundtrips_with_crc() {
        for m in all() {
            let b = m.to_block();
            let (d, used, ok) = parse_block(&b).unwrap();
            assert_eq!(d, m);
            assert_eq!(used, b.len());
            assert!(ok);
            // truncated → need more; corrupted → CRC fails
            assert_eq!(parse_block(&b[..b.len() - 1]).unwrap_err(), ParseError::NeedMore);
            let mut c = b.clone();
            let last = c.len() - 1;
            c[last] = !c[last];
            assert!(!parse_block(&c).unwrap().2);
        }
    }

    #[test]
    fn documented_body_lengths() {
        let lens: Vec<usize> = all().iter().map(|m| m.to_bits().len() - 4).collect();
        assert_eq!(lens, vec![0, 38, 77, 111, 44, 94, 137, 15 + 64, 8 + 40]);
    }

    #[test]
    fn spec_render_units() {
        let ctx = MsgContext { station: 2, role: Role::Secondary, first_group: 1_000_000, gri_us: Some(99_300) };
        let m = MessageSpec::UtcTime { tai_minus_utc_s: 37, leap_pending: 0, days_to_leap: 0 }.render(&ctx);
        // 1e6 groups × 99.3 ms = 99 300 s exactly
        assert_eq!(
            m,
            Message::UtcTime { loran_seconds: 99_300, sub_second_10us: 0, tai_minus_utc_s: 37, leap_pending: 0, days_to_leap: 0 }
        );
        let s = MessageSpec::StationId { chain_id: 1, station_code: None, signal_version: 0 }.render(&ctx);
        assert!(matches!(s, Message::StationId { station_code: b'C', secondary: true, gri_10us: 9930, .. }));
    }
}
