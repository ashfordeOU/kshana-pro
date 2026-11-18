// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Reed–Solomon codes RS(n, k) over GF(2^m), shortened when n < 2^m − 1.
//!
//! * Systematic encoding: the codeword is the k message symbols followed by n − k parity
//!   symbols. Array index `i` of a codeword is the coefficient of x^(n−1−i).
//! * Generator `g(x) = ∏_{j=0}^{n−k−1} (x − α^(fcr+j))`, first consecutive root `fcr`
//!   (default 1).
//! * Decoding: syndromes, Berlekamp–Massey for the error locator Λ(x), Chien search for
//!   its roots, Forney's formula for the error values. Corrects up to ⌊(n−k)/2⌋ symbol
//!   errors; reports failure when Λ's degree exceeds that, when Λ has the wrong number of
//!   roots in the (shortened) code positions, or when the corrected word is not a codeword.
//!
//! NEEDS CONFIRMATION: `fcr` (first consecutive root) of the published Eurofix and 9th-pulse
//! codes. It does not change the correction capability.

use super::gf::Gf;

/// An RS(n, k) code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReedSolomon {
    pub gf: Gf,
    pub n: usize,
    pub k: usize,
    pub fcr: i64,
    /// Generator polynomial, highest degree first, monic (len n − k + 1).
    gen: Vec<u16>,
}

/// Decoding failure: more errors than the code can correct (detected).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RsFailure;

impl ReedSolomon {
    pub fn new(m: u32, n: usize, k: usize) -> Result<ReedSolomon, String> {
        Self::with_fcr(m, n, k, 1)
    }

    pub fn with_fcr(m: u32, n: usize, k: usize, fcr: i64) -> Result<ReedSolomon, String> {
        let gf = Gf::new(m)?;
        if n > gf.order() || n < 2 {
            return Err(format!("RS over GF(2^{m}): n = {n} must be 2..={}", gf.order()));
        }
        if k == 0 || k >= n {
            return Err(format!("RS({n}, {k}): k must be 1..n"));
        }
        let mut gen = vec![1u16];
        for j in 0..(n - k) as i64 {
            let r = gf.alpha_pow(fcr + j);
            // gen · (x + r), highest first
            let mut ng = vec![0u16; gen.len() + 1];
            for (i, c) in gen.iter().enumerate() {
                ng[i] ^= c;
                ng[i + 1] ^= gf.mul(*c, r);
            }
            gen = ng;
        }
        Ok(ReedSolomon { gf, n, k, fcr, gen })
    }

    /// Number of correctable symbol errors ⌊(n−k)/2⌋.
    pub fn t(&self) -> usize {
        (self.n - self.k) / 2
    }

    /// Systematic encoding of `msg` (k symbols, each < 2^m).
    pub fn encode(&self, msg: &[u16]) -> Vec<u16> {
        assert_eq!(msg.len(), self.k, "RS encode: message must have k symbols");
        let nk = self.n - self.k;
        let mut rem = vec![0u16; nk];
        for &d in msg {
            debug_assert!((d as usize) < self.gf.size);
            let fb = d ^ rem[0];
            rem.rotate_left(1);
            rem[nk - 1] = 0;
            if fb != 0 {
                for (j, r) in rem.iter_mut().enumerate() {
                    *r ^= self.gf.mul(fb, self.gen[j + 1]);
                }
            }
        }
        let mut cw = msg.to_vec();
        cw.extend(rem);
        cw
    }

    /// Syndromes S_j = c(α^(fcr+j)), j = 0..n−k.
    pub fn syndromes(&self, cw: &[u16]) -> Vec<u16> {
        (0..(self.n - self.k) as i64)
            .map(|j| {
                let x = self.gf.alpha_pow(self.fcr + j);
                cw.iter().fold(0u16, |s, c| self.gf.mul(s, x) ^ c)
            })
            .collect()
    }

    /// Correct `cw` in place; returns the number of symbols corrected.
    pub fn decode(&self, cw: &mut [u16]) -> Result<usize, RsFailure> {
        assert_eq!(cw.len(), self.n, "RS decode: codeword must have n symbols");
        let gf = &self.gf;
        let s = self.syndromes(cw);
        if s.iter().all(|x| *x == 0) {
            return Ok(0);
        }
        // Berlekamp–Massey (polynomials lowest degree first).
        let mut lambda = vec![1u16];
        let mut b = vec![1u16];
        let mut l = 0usize;
        let mut shift = 1usize;
        let mut bb = 1u16;
        for r in 0..s.len() {
            let mut d = s[r];
            for i in 1..=l.min(lambda.len() - 1) {
                d ^= gf.mul(lambda[i], s[r - i]);
            }
            if d == 0 {
                shift += 1;
                continue;
            }
            let coef = gf.div(d, bb);
            let mut next = lambda.clone();
            if next.len() < b.len() + shift {
                next.resize(b.len() + shift, 0);
            }
            for (i, bi) in b.iter().enumerate() {
                next[i + shift] ^= gf.mul(coef, *bi);
            }
            if 2 * l <= r {
                b = lambda;
                l = r + 1 - l;
                bb = d;
                shift = 1;
            } else {
                shift += 1;
            }
            lambda = next;
        }
        while lambda.len() > 1 && *lambda.last().unwrap() == 0 {
            lambda.pop();
        }
        let deg = lambda.len() - 1;
        if deg != l || l > self.t() || l == 0 {
            return Err(RsFailure);
        }
        // Chien search over the n code positions.
        let mut pos = Vec::with_capacity(l);
        for i in 0..self.n {
            let p = (self.n - 1 - i) as i64;
            if gf.eval_low_first(&lambda, gf.alpha_pow(-p)) == 0 {
                pos.push(i);
            }
        }
        if pos.len() != l {
            return Err(RsFailure);
        }
        // Forney: Ω(x) = S(x) Λ(x) mod x^(n−k); e = X^(1−fcr) Ω(X⁻¹) / Λ'(X⁻¹).
        let nk = s.len();
        let mut omega = vec![0u16; nk];
        for (i, si) in s.iter().enumerate() {
            for (j, lj) in lambda.iter().enumerate() {
                if i + j < nk {
                    omega[i + j] ^= gf.mul(*si, *lj);
                }
            }
        }
        let dlambda: Vec<u16> = (1..lambda.len()).map(|i| if i % 2 == 1 { lambda[i] } else { 0 }).collect();
        for &i in &pos {
            let p = (self.n - 1 - i) as i64;
            let x = gf.alpha_pow(p);
            let xinv = gf.alpha_pow(-p);
            let den = gf.eval_low_first(&dlambda, xinv);
            if den == 0 {
                return Err(RsFailure);
            }
            let num = gf.mul(gf.pow(x, 1 - self.fcr), gf.eval_low_first(&omega, xinv));
            cw[i] ^= gf.div(num, den);
        }
        if self.syndromes(cw).iter().any(|x| *x != 0) {
            return Err(RsFailure);
        }
        Ok(l)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    fn check_code(m: u32, n: usize, k: usize, seeds: u64) {
        let rs = ReedSolomon::new(m, n, k).unwrap();
        let q = 1u16 << m;
        for seed in 0..seeds {
            let mut rng = ChaCha8Rng::seed_from_u64(seed ^ ((m as u64) << 40) ^ ((n as u64) << 20));
            let msg: Vec<u16> = (0..k).map(|_| rng.gen_range(0..q)).collect();
            let cw = rs.encode(&msg);
            assert_eq!(&cw[..k], &msg[..], "systematic");
            assert!(rs.syndromes(&cw).iter().all(|s| *s == 0), "codeword has zero syndromes");
            let ne = rng.gen_range(0..=rs.t());
            let mut bad = cw.clone();
            let mut positions: Vec<usize> = (0..n).collect();
            for i in 0..ne {
                let j = rng.gen_range(i..n);
                positions.swap(i, j);
                bad[positions[i]] ^= rng.gen_range(1..q);
            }
            let r = rs.decode(&mut bad);
            assert_eq!(r, Ok(ne), "m={m} n={n} k={k} seed={seed} errors={ne}");
            assert_eq!(bad, cw);
        }
    }

    #[test]
    fn corrects_up_to_t_errors_property() {
        check_code(7, 30, 10, 400); // Eurofix-like RS(30,10), t = 10
        check_code(5, 24, 9, 400); // 9th-pulse-like RS(24,9), t = 7
        check_code(7, 127, 101, 100); // full-length code
        check_code(8, 255, 223, 50);
        check_code(5, 31, 15, 200);
        check_code(4, 15, 11, 200);
        check_code(7, 21, 10, 200); // odd n−k
    }

    #[test]
    fn beyond_t_is_mostly_detected_never_panics() {
        let rs = ReedSolomon::new(7, 30, 10).unwrap();
        let mut detected = 0;
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        for _ in 0..300 {
            let msg: Vec<u16> = (0..10).map(|_| rng.gen_range(0..128)).collect();
            let cw = rs.encode(&msg);
            let mut bad = cw.clone();
            for e in bad.iter_mut().take(14) {
                *e ^= rng.gen_range(1..128);
            }
            match rs.decode(&mut bad) {
                Err(_) => detected += 1,
                Ok(_) => assert_ne!(bad, cw), // a miscorrection to another codeword
            }
        }
        assert!(detected > 250, "detected {detected}/300");
    }

    #[test]
    fn rejects_bad_parameters() {
        assert!(ReedSolomon::new(7, 128, 10).is_err());
        assert!(ReedSolomon::new(7, 30, 30).is_err());
        assert!(ReedSolomon::new(9, 30, 10).is_err());
    }
}
