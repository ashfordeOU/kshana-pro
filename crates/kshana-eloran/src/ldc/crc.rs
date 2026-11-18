// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Cyclic redundancy checks, MSB-first (non-reflected), computed bit by bit so that
//! messages need not be a whole number of bytes.
//!
//! * CRC-24Q (Qualcomm; used by RTCM 3 and SBAS): polynomial 0x864CFB, init 0, no final
//!   XOR. Check value of ASCII "123456789" is 0xCDE703.
//! * CRC-14 frame check of the Eurofix profile: NEEDS CONFIRMATION. The published profile
//!   has a 14-bit CRC per 70-bit RS information block; its polynomial is not reproduced
//!   here from the source text; modelled with [`CRC14_POLY`].

use serde::{Deserialize, Serialize};

/// CRC-24Q generator (x²⁴ implicit).
pub const CRC24Q_POLY: u32 = 0x86_4CFB;
/// CRC-14 generator used for the Eurofix frame check, 0x0805 = x¹⁴+x¹¹+x²+1 (the
/// CRC-14/DARC generator, applied MSB-first, init 0). NEEDS CONFIRMATION against the
/// Eurofix definition.
pub const CRC14_POLY: u32 = 0x0805;
/// CRC-16/CCITT generator, offered as an alternative frame check.
pub const CRC16_POLY: u32 = 0x1021;

/// Generic MSB-first CRC over a bit string.
pub fn crc_bits(bits: &[bool], width: u32, poly: u32, init: u32) -> u32 {
    let mask: u64 = (1u64 << width) - 1;
    let mut reg = init as u64 & mask;
    for &b in bits {
        let top = (reg >> (width - 1)) & 1 == 1;
        reg = (reg << 1) & mask;
        if top ^ b {
            reg ^= poly as u64;
        }
    }
    reg as u32
}

/// CRC-24Q of a bit string.
pub fn crc24q_bits(bits: &[bool]) -> u32 {
    crc_bits(bits, 24, CRC24Q_POLY, 0)
}

/// CRC-24Q of bytes.
pub fn crc24q(bytes: &[u8]) -> u32 {
    crc24q_bits(&super::bits::unpack_bytes(bytes))
}

/// Frame check sequence appended to each RS information block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameCrc {
    /// No frame check (RS decoding failure is the only detector).
    #[default]
    None,
    /// 14-bit check ([`CRC14_POLY`]), the Eurofix frame check (polynomial needs confirmation).
    Crc14,
    /// CRC-16/CCITT.
    Crc16,
    /// CRC-24Q.
    Crc24q,
}

impl FrameCrc {
    pub fn width(self) -> usize {
        match self {
            FrameCrc::None => 0,
            FrameCrc::Crc14 => 14,
            FrameCrc::Crc16 => 16,
            FrameCrc::Crc24q => 24,
        }
    }
    pub fn compute(self, bits: &[bool]) -> u32 {
        match self {
            FrameCrc::None => 0,
            FrameCrc::Crc14 => crc_bits(bits, 14, CRC14_POLY, 0),
            FrameCrc::Crc16 => crc_bits(bits, 16, CRC16_POLY, 0),
            FrameCrc::Crc24q => crc24q_bits(bits),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc24q_check_value() {
        assert_eq!(crc24q(b"123456789"), 0xCDE703);
        assert_eq!(crc24q(b""), 0);
    }

    #[test]
    fn crc16_ccitt_xmodem_check_value() {
        // CRC-16/XMODEM (poly 0x1021, init 0) of "123456789" is 0x31C3.
        let b = super::super::bits::unpack_bytes(b"123456789");
        assert_eq!(crc_bits(&b, 16, CRC16_POLY, 0), 0x31C3);
    }

    #[test]
    fn crc_detects_single_bit_errors() {
        let b = super::super::bits::unpack_bytes(b"eLoran data channel");
        let c = crc24q_bits(&b);
        for i in 0..b.len() {
            let mut x = b.clone();
            x[i] = !x[i];
            assert_ne!(crc24q_bits(&x), c);
            assert_ne!(FrameCrc::Crc14.compute(&x), FrameCrc::Crc14.compute(&b));
        }
    }
}
