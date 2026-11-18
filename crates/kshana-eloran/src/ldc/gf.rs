// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Arithmetic in GF(2^m), 3 ≤ m ≤ 8, by log/antilog tables.
//!
//! Field generator polynomials (primitive, α = x is a generator):
//!
//! | m | polynomial              | hex   |
//! |---|-------------------------|-------|
//! | 3 | x³+x+1                  | 0x0B  |
//! | 4 | x⁴+x+1                  | 0x13  |
//! | 5 | x⁵+x²+1                 | 0x25  |
//! | 6 | x⁶+x+1                  | 0x43  |
//! | 7 | x⁷+x³+1                 | 0x89  |
//! | 8 | x⁸+x⁴+x³+x²+1           | 0x11D |
//!
//! NEEDS CONFIRMATION: the field polynomial of the published Eurofix GF(2^7) code and of the
//! 9th-pulse GF(2^5) code. These are the conventional primitive polynomials; a different
//! (but also primitive) choice changes the codewords, not the error-correction capability.

/// A finite field GF(2^m).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gf {
    pub m: u32,
    pub poly: u32,
    /// Field size 2^m.
    pub size: usize,
    exp: Vec<u16>,
    log: Vec<u16>,
}

/// Conventional primitive polynomial of GF(2^m).
pub fn primitive_poly(m: u32) -> Option<u32> {
    Some(match m {
        3 => 0x0B,
        4 => 0x13,
        5 => 0x25,
        6 => 0x43,
        7 => 0x89,
        8 => 0x11D,
        _ => return None,
    })
}

impl Gf {
    pub fn new(m: u32) -> Result<Gf, String> {
        let poly = primitive_poly(m).ok_or_else(|| format!("GF(2^{m}): m must be 3..=8"))?;
        Gf::with_poly(m, poly)
    }

    pub fn with_poly(m: u32, poly: u32) -> Result<Gf, String> {
        let size = 1usize << m;
        let mut exp = vec![0u16; 2 * size];
        let mut log = vec![0u16; size];
        let mut x: u32 = 1;
        for (i, e) in exp.iter_mut().enumerate().take(size - 1) {
            *e = x as u16;
            if i > 0 && x == 1 {
                return Err(format!("GF(2^{m}): polynomial {poly:#x} is not primitive"));
            }
            log[x as usize] = i as u16;
            x <<= 1;
            if x & size as u32 != 0 {
                x ^= poly;
            }
        }
        if x != 1 {
            return Err(format!("GF(2^{m}): polynomial {poly:#x} is not primitive"));
        }
        for i in size - 1..2 * size {
            exp[i] = exp[i - (size - 1)];
        }
        Ok(Gf {
            m,
            poly,
            size,
            exp,
            log,
        })
    }

    /// Order of the multiplicative group, 2^m − 1.
    #[inline]
    pub fn order(&self) -> usize {
        self.size - 1
    }
    #[inline]
    pub fn mul(&self, a: u16, b: u16) -> u16 {
        if a == 0 || b == 0 {
            return 0;
        }
        self.exp[self.log[a as usize] as usize + self.log[b as usize] as usize]
    }
    #[inline]
    pub fn div(&self, a: u16, b: u16) -> u16 {
        assert!(b != 0, "GF division by zero");
        if a == 0 {
            return 0;
        }
        let o = self.order();
        self.exp[(self.log[a as usize] as usize + o - self.log[b as usize] as usize) % o]
    }
    #[inline]
    pub fn inv(&self, a: u16) -> u16 {
        self.div(1, a)
    }
    /// α^i for any integer i.
    #[inline]
    pub fn alpha_pow(&self, i: i64) -> u16 {
        self.exp[i.rem_euclid(self.order() as i64) as usize]
    }
    /// a^e for any integer e (a ≠ 0 when e < 0).
    pub fn pow(&self, a: u16, e: i64) -> u16 {
        if a == 0 {
            return if e == 0 { 1 } else { 0 };
        }
        self.alpha_pow(self.log[a as usize] as i64 * e)
    }
    /// Evaluate a polynomial given lowest degree first at `x`.
    pub fn eval_low_first(&self, p: &[u16], x: u16) -> u16 {
        p.iter().rev().fold(0, |acc, c| self.mul(acc, x) ^ c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_axioms_gf128() {
        let f = Gf::new(7).unwrap();
        assert_eq!(f.size, 128);
        for a in 1..128u16 {
            assert_eq!(f.mul(a, f.inv(a)), 1);
            for b in [1u16, 2, 77, 127] {
                assert_eq!(f.div(f.mul(a, b), b), a);
            }
        }
        // α^127 = 1, and α generates every nonzero element exactly once
        assert_eq!(f.alpha_pow(127), 1);
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..127 {
            seen.insert(f.alpha_pow(i));
        }
        assert_eq!(seen.len(), 127);
        for m in 3..=8 {
            Gf::new(m).unwrap();
        }
        assert!(Gf::with_poly(4, 0x1F).is_err()); // x⁴+x³+x²+x+1 is irreducible, not primitive
    }
}
