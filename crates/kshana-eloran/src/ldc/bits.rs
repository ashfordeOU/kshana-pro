// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Bit packing helpers (MSB first everywhere).

/// Appends unsigned / two's-complement fields, most significant bit first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitWriter {
    pub bits: Vec<bool>,
}

impl BitWriter {
    pub fn new() -> BitWriter {
        BitWriter::default()
    }
    /// Append the low `n` bits of `v` (n ≤ 64). Higher bits of `v` are ignored.
    pub fn put(&mut self, v: u64, n: u32) {
        for i in (0..n).rev() {
            self.bits.push((v >> i) & 1 == 1);
        }
    }
    /// Append `v` as an `n`-bit two's-complement field (saturated to the field range).
    pub fn put_signed(&mut self, v: i64, n: u32) {
        let lo = -(1i64 << (n - 1));
        let hi = (1i64 << (n - 1)) - 1;
        self.put(v.clamp(lo, hi) as u64, n);
    }
    pub fn put_bits(&mut self, b: &[bool]) {
        self.bits.extend_from_slice(b);
    }
    pub fn put_bytes(&mut self, b: &[u8]) {
        for x in b {
            self.put(*x as u64, 8);
        }
    }
    pub fn len(&self) -> usize {
        self.bits.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }
}

/// Reads fields back; every getter returns `None` when the input runs out.
#[derive(Clone, Debug)]
pub struct BitReader<'a> {
    bits: &'a [bool],
    pub pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(bits: &'a [bool]) -> BitReader<'a> {
        BitReader { bits, pos: 0 }
    }
    pub fn get(&mut self, n: u32) -> Option<u64> {
        if self.pos + n as usize > self.bits.len() {
            return None;
        }
        let mut v = 0u64;
        for _ in 0..n {
            v = (v << 1) | self.bits[self.pos] as u64;
            self.pos += 1;
        }
        Some(v)
    }
    pub fn get_signed(&mut self, n: u32) -> Option<i64> {
        let v = self.get(n)?;
        if n < 64 && (v >> (n - 1)) & 1 == 1 {
            Some(v as i64 - (1i64 << n))
        } else {
            Some(v as i64)
        }
    }
    pub fn get_bytes(&mut self, len: usize) -> Option<Vec<u8>> {
        (0..len).map(|_| self.get(8).map(|v| v as u8)).collect()
    }
    pub fn remaining(&self) -> usize {
        self.bits.len() - self.pos
    }
}

/// Pack bits into bytes, MSB first, zero-padding the last byte.
pub fn pack_bytes(bits: &[bool]) -> Vec<u8> {
    bits.chunks(8)
        .map(|c| {
            c.iter()
                .enumerate()
                .fold(0u8, |a, (i, b)| a | ((*b as u8) << (7 - i)))
        })
        .collect()
}

/// Bits of `bytes`, MSB first.
pub fn unpack_bytes(bytes: &[u8]) -> Vec<bool> {
    let mut w = BitWriter::new();
    w.put_bytes(bytes);
    w.bits
}

/// Split `bits` into `width`-bit symbols (MSB first), zero-padding the last.
pub fn to_symbols(bits: &[bool], width: u32) -> Vec<u32> {
    bits.chunks(width as usize)
        .map(|c| {
            let mut v = 0u32;
            for i in 0..width as usize {
                v = (v << 1) | c.get(i).copied().unwrap_or(false) as u32;
            }
            v
        })
        .collect()
}

/// Concatenate `width`-bit symbols into bits (MSB first).
pub fn from_symbols(syms: &[u32], width: u32) -> Vec<bool> {
    let mut w = BitWriter::new();
    for s in syms {
        w.put(*s as u64, width);
    }
    w.bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_fields() {
        let mut w = BitWriter::new();
        w.put(0b1011, 4);
        w.put_signed(-5, 7);
        w.put_signed(1000, 8); // saturates to 127
        w.put(0xABCDE, 20);
        let mut r = BitReader::new(&w.bits);
        assert_eq!(r.get(4), Some(0b1011));
        assert_eq!(r.get_signed(7), Some(-5));
        assert_eq!(r.get_signed(8), Some(127));
        assert_eq!(r.get(20), Some(0xABCDE));
        assert_eq!(r.get(1), None);
        assert_eq!(pack_bytes(&unpack_bytes(&[0x12, 0xF0])), vec![0x12, 0xF0]);
        let s = to_symbols(&w.bits, 7);
        assert_eq!(&from_symbols(&s, 7)[..w.len()], &w.bits[..]);
    }
}
