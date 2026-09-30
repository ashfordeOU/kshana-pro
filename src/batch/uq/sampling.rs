// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Seeded sampling designs on the unit hypercube.
//!
//! * **Latin hypercube** (M. D. McKay, R. J. Beckman and W. J. Conover, "A Comparison
//!   of Three Methods for Selecting Values of Input Variables in the Analysis of Output
//!   from a Computer Code", Technometrics 21(2), 1979, pp. 239-245): each input's
//!   range is cut into `n` equally probable strata and every stratum is used exactly
//!   once, in a random order that is independent between inputs.
//! * **Plain random**: independent uniform numbers.
//! * **The two Saltelli base matrices** `A` and `B`: two independent samples of the
//!   same size, from which the `A_B^(i)` matrices of a Sobol' analysis are formed by
//!   taking column `i` from `B` and every other column from `A`.
//!
//! Every column of every matrix has its own random stream, derived from the study seed,
//! the matrix and the column number. A design is therefore a pure function of
//! `(seed, method, n, number of inputs)`, and adding an input leaves the columns of the
//! inputs already there unchanged.

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// How the unit sample is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Latin hypercube sampling.
    Lhs,
    /// Plain (independent) random sampling.
    Random,
}

impl Method {
    /// Parse the study file's spelling.
    pub fn parse(s: &str) -> Result<Method, String> {
        match s {
            "lhs" => Ok(Method::Lhs),
            "random" => Ok(Method::Random),
            other => Err(format!(
                "sampling method must be \"lhs\" or \"random\" (got \"{other}\")"
            )),
        }
    }

    /// The study file's spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Lhs => "lhs",
            Method::Random => "random",
        }
    }
}

/// Which of the two independent base matrices a column belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matrix {
    /// The sample the statistics are taken over.
    A,
    /// The second, independent sample a Sobol' analysis needs.
    B,
}

/// The random stream of one column of one matrix.
fn column_rng(seed: u64, matrix: Matrix, column: usize) -> ChaCha8Rng {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let m = match matrix {
        Matrix::A => 1u64,
        Matrix::B => 2u64,
    };
    rng.set_stream((m << 32) | column as u64);
    rng
}

/// A uniform number strictly inside (0, 1), on a grid of 2^53 midpoints: never 0 and
/// never 1, so an inverse distribution function never returns an infinity.
fn open_unit(rng: &mut ChaCha8Rng) -> f64 {
    ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
}

/// One column of `n` numbers in (0, 1).
pub fn unit_column(method: Method, n: usize, seed: u64, matrix: Matrix, column: usize) -> Vec<f64> {
    let mut rng = column_rng(seed, matrix, column);
    match method {
        Method::Random => (0..n).map(|_| open_unit(&mut rng)).collect(),
        Method::Lhs => {
            // A random permutation of the strata (Fisher-Yates), then one point at a
            // random position inside each stratum.
            let mut strata: Vec<usize> = (0..n).collect();
            for i in (1..n).rev() {
                let j = (rng.next_u64() % (i as u64 + 1)) as usize;
                strata.swap(i, j);
            }
            strata
                .into_iter()
                .map(|s| (s as f64 + open_unit(&mut rng)) / n as f64)
                .collect()
        }
    }
}

/// An `n` by `k` sample of the unit hypercube, as rows.
pub fn unit_sample(method: Method, n: usize, k: usize, seed: u64, matrix: Matrix) -> Vec<Vec<f64>> {
    let cols: Vec<Vec<f64>> = (0..k)
        .map(|c| unit_column(method, n, seed, matrix, c))
        .collect();
    (0..n)
        .map(|r| cols.iter().map(|c| c[r]).collect())
        .collect()
}

/// Which block of a Sobol' design a realisation belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// Row of matrix `A`.
    A,
    /// Row of matrix `B`.
    B,
    /// Row of `A_B^(i)`: `A` with column `i` taken from `B`.
    Ab(usize),
}

/// The layout of a Sobol' design of base size `n` over `k` inputs: realisations
/// `0..n` are `A`, `n..2n` are `B`, and `2n + i*n .. 2n + (i+1)*n` are `A_B^(i)`.
pub fn sobol_block(n: usize, k: usize, index: usize) -> Option<(Block, usize)> {
    if n == 0 || index >= n * (k + 2) {
        return None;
    }
    let (b, row) = (index / n, index % n);
    Some(match b {
        0 => (Block::A, row),
        1 => (Block::B, row),
        i => (Block::Ab(i - 2), row),
    })
}

/// The index of row `row` of a block in the same layout.
pub fn sobol_index(n: usize, block: Block, row: usize) -> usize {
    match block {
        Block::A => row,
        Block::B => n + row,
        Block::Ab(i) => (2 + i) * n + row,
    }
}

/// The unit-hypercube rows of a whole Sobol' design, in realisation order.
pub fn saltelli_unit_rows(method: Method, n: usize, k: usize, seed: u64) -> Vec<Vec<f64>> {
    let a = unit_sample(method, n, k, seed, Matrix::A);
    let b = unit_sample(method, n, k, seed, Matrix::B);
    let mut rows = Vec::with_capacity(n * (k + 2));
    rows.extend(a.iter().cloned());
    rows.extend(b.iter().cloned());
    for i in 0..k {
        for r in 0..n {
            let mut row = a[r].clone();
            row[i] = b[r][i];
            rows.push(row);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_latin_hypercube_uses_every_stratum_exactly_once() {
        let n = 257;
        for column in 0..4 {
            let col = unit_column(Method::Lhs, n, 11, Matrix::A, column);
            let mut seen = vec![0usize; n];
            for u in &col {
                assert!(*u > 0.0 && *u < 1.0);
                seen[(u * n as f64).floor() as usize] += 1;
            }
            assert!(
                seen.iter().all(|c| *c == 1),
                "column {column}: a stratum was used more or less than once"
            );
        }
    }

    #[test]
    fn a_design_is_a_pure_function_of_its_seed() {
        let a = unit_sample(Method::Lhs, 64, 3, 5, Matrix::A);
        assert_eq!(a, unit_sample(Method::Lhs, 64, 3, 5, Matrix::A));
        assert_ne!(a, unit_sample(Method::Lhs, 64, 3, 6, Matrix::A));
        assert_ne!(a, unit_sample(Method::Lhs, 64, 3, 5, Matrix::B));
        assert_ne!(a, unit_sample(Method::Random, 64, 3, 5, Matrix::A));
    }

    #[test]
    fn adding_an_input_keeps_the_existing_columns() {
        let three = unit_sample(Method::Lhs, 32, 3, 9, Matrix::A);
        let four = unit_sample(Method::Lhs, 32, 4, 9, Matrix::A);
        for (r3, r4) in three.iter().zip(&four) {
            assert_eq!(r3[..], r4[..3]);
        }
    }

    #[test]
    fn columns_are_not_copies_of_each_other() {
        let s = unit_sample(Method::Random, 2000, 2, 3, Matrix::A);
        let (mut sx, mut sy, mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for r in &s {
            sx += r[0];
            sy += r[1];
            sxy += r[0] * r[1];
            sxx += r[0] * r[0];
            syy += r[1] * r[1];
        }
        let n = s.len() as f64;
        let corr = (sxy / n - sx / n * sy / n)
            / ((sxx / n - (sx / n).powi(2)) * (syy / n - (sy / n).powi(2))).sqrt();
        // Independent columns: |r| of order 1/sqrt(n) = 0.022; five times that.
        assert!(corr.abs() < 0.11, "columns correlate: r = {corr}");
    }

    #[test]
    fn the_saltelli_layout_swaps_exactly_one_column() {
        let (n, k) = (8, 3);
        let rows = saltelli_unit_rows(Method::Lhs, n, k, 21);
        assert_eq!(rows.len(), n * (k + 2));
        for (index, row) in rows.iter().enumerate() {
            let (block, r) = sobol_block(n, k, index).expect("in range");
            assert_eq!(sobol_index(n, block, r), index);
            let (a, b) = (&rows[r], &rows[n + r]);
            match block {
                Block::A => assert_eq!(row, a),
                Block::B => assert_eq!(row, b),
                Block::Ab(i) => {
                    for c in 0..k {
                        let want = if c == i { b[c] } else { a[c] };
                        assert_eq!(row[c], want);
                    }
                }
            }
        }
        assert!(sobol_block(n, k, n * (k + 2)).is_none());
    }
}
