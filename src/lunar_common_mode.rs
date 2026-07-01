// SPDX-License-Identifier: AGPL-3.0-only
//! Common-mode integrity core: the blind-subspace projector and the split of a
//! measurement error into its RAIM-invisible and RAIM-detectable parts.
//!
//! Snapshot RAIM / ARAIM detects a fault only through the *parity*
//! (measurement-inconsistency) subspace. For the linearised model
//! `y = G·x + ε` with geometry rows `gᵢ = [-eᵢₓ, -eᵢy, -eᵢz, 1]` (the negative
//! line-of-sight unit vector plus a clock column), the least-squares state
//! estimate is `x̂ = (GᵀG)⁻¹Gᵀ y = S·y` and the parity residual is
//! `r = y − G·x̂ = P⊥·y`, where `P⊥ = I − G·S` projects onto the orthogonal
//! complement of `range(G)`.
//!
//! Any measurement error `δy` splits uniquely into
//!
//! * a **blind** part in `range(G)`, absorbed as a state error `S·δy` and
//!   therefore invisible to *any* residual test (`P⊥` annihilates it), and
//! * a **detectable** part in the parity space, which shows up as `r`.
//!
//! This module builds that exact projector/split. The blindness is a known,
//! *correct* property of all snapshot RAIM — a common-mode bias absorbed by the
//! clock column is the trivial 1-D case (see
//! [`crate::spoof_monitors::parity_raim_test`], which states plainly that a
//! bias on every pseudorange leaves its statistic unchanged). Here we expose the
//! general split so a caller can quantify *how much* of a given error is blind.
//!
//! Everything in this module is **Modelled / analytic**: it is the linear
//! algebra of the projector, with no external-oracle or Validated claim. The
//! general n-dimensional characterisation of the blind subspace, the
//! frame/ephemeris application, the common-mode protection-level (CMPL) bound,
//! and the real-data Validated anchor are built in later tasks.

use crate::orbit::{invert4, los_unit};

/// Build the RAIM geometry matrix `G` (rows `[-eₓ, -e_y, -e_z, 1]`) for a `user`
/// observing a set of satellite positions `sats`, using
/// [`crate::orbit::los_unit`] for each line-of-sight unit vector `e`.
///
/// Returns `None` when there are fewer than 5 satellites (RAIM needs redundancy,
/// `dof = n − 4 ≥ 1`) or when any line of sight is undefined (a satellite that
/// coincides with the user).
pub fn geometry_from_los(user: [f64; 3], sats: &[[f64; 3]]) -> Option<Vec<[f64; 4]>> {
    if sats.len() < 5 {
        return None;
    }
    let mut g: Vec<[f64; 4]> = Vec::with_capacity(sats.len());
    for &s in sats {
        let e = los_unit(user, s)?;
        g.push([-e[0], -e[1], -e[2], 1.0]);
    }
    Some(g)
}

/// The decomposition of a measurement error `δy` into the part the user silently
/// absorbs (RAIM-invisible) and the part RAIM can detect (the parity residual).
///
/// All fields are analytic consequences of the geometry `G`; there is no
/// stochastic or external-oracle content.
#[derive(Clone, Debug)]
pub struct CommonModeSplit {
    /// `S·δy` — the position+clock state error the user absorbs. This is the
    /// blind part: it is completely invisible to any RAIM residual test.
    pub blind_dx: [f64; 4],
    /// `r = δy − G·blind_dx` — the parity residual, i.e. exactly what RAIM sees.
    pub detectable_residual: Vec<f64>,
    /// `‖G·blind_dx‖` — the magnitude of `δy` that lives in `range(G)`.
    pub blind_norm: f64,
    /// `‖r‖` — the magnitude of `δy` that lives in the parity space.
    pub detectable_norm: f64,
    /// `blind_norm / ‖δy‖` — 1.0 when `δy` is perfectly blind (entirely in
    /// `range(G)`), 0.0 when it is fully detectable (entirely in parity space).
    pub blind_fraction: f64,
}

/// Split a measurement error `delta_y` into its RAIM-invisible (`range(G)`) and
/// RAIM-detectable (parity) parts for the geometry `geometry` (rows
/// `[-eₓ, -e_y, -e_z, 1]`).
///
/// Uses `S = (GᵀG)⁻¹Gᵀ` (unit measurement weighting) built with
/// [`crate::orbit::invert4`], then `blind_dx = S·δy`, `pred = G·blind_dx`, and
/// `detectable_residual = δy − pred`. Norms are Euclidean.
///
/// Returns `None` when `geometry.len() != delta_y.len()`, when there are fewer
/// than 5 rows (no redundancy), when `GᵀG` is singular (rank-deficient
/// geometry), or when `δy` is exactly zero (the blind fraction is then
/// undefined — a zero perturbation has no direction to classify).
pub fn common_mode_split(geometry: &[[f64; 4]], delta_y: &[f64]) -> Option<CommonModeSplit> {
    let n = geometry.len();
    if n != delta_y.len() || n < 5 {
        return None;
    }
    let dy_norm = norm(delta_y);
    // A zero perturbation has no direction, so its blind fraction is undefined.
    if dy_norm == 0.0 {
        return None;
    }

    // Normal matrix GtG and its inverse A0 = (GtG)^-1.
    let mut gtg = [[0.0_f64; 4]; 4];
    for row in geometry {
        for i in 0..4 {
            for j in 0..4 {
                gtg[i][j] += row[i] * row[j];
            }
        }
    }
    let a0 = invert4(gtg)?;

    // S = A0 * G^T  (4 x n): columns s_c map measurement errors to the state.
    let s: Vec<[f64; 4]> = (0..n)
        .map(|c| {
            let mut col = [0.0_f64; 4];
            for (i, ci) in col.iter_mut().enumerate() {
                *ci = (0..4).map(|k| a0[i][k] * geometry[c][k]).sum();
            }
            col
        })
        .collect();

    // blind_dx = S * delta_y.
    let mut blind_dx = [0.0_f64; 4];
    for (c, &dy) in delta_y.iter().enumerate() {
        for i in 0..4 {
            blind_dx[i] += s[c][i] * dy;
        }
    }

    // pred = G * blind_dx (the range(G) part of delta_y); residual = delta_y - pred.
    let mut detectable_residual = Vec::with_capacity(n);
    let mut blind_sq = 0.0_f64;
    for (c, &dy) in delta_y.iter().enumerate() {
        let pred: f64 = (0..4).map(|k| geometry[c][k] * blind_dx[k]).sum();
        blind_sq += pred * pred;
        detectable_residual.push(dy - pred);
    }
    let blind_norm = blind_sq.sqrt();
    let detectable_norm = norm(&detectable_residual);
    let blind_fraction = blind_norm / dy_norm;

    Some(CommonModeSplit {
        blind_dx,
        detectable_residual,
        blind_norm,
        detectable_norm,
        blind_fraction,
    })
}

/// Euclidean (L2) norm of a vector.
fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A user with six well-spread satellites, giving a full-rank geometry.
    fn test_geometry() -> Vec<[f64; 4]> {
        let user = [7.0e6, 0.0, 0.0];
        let sats = [
            [user[0] + 2.0e7, user[1], user[2]],
            [user[0], user[1] + 2.0e7, user[2]],
            [user[0], user[1], user[2] + 2.0e7],
            [user[0] - 1.5e7, user[1] + 1.0e7, user[2] + 0.5e7],
            [user[0] + 1.0e7, user[1] - 1.5e7, user[2] + 1.0e7],
            [user[0] - 1.0e7, user[1] - 1.0e7, user[2] - 1.5e7],
        ];
        geometry_from_los(user, &sats).expect("6 spread satellites give a valid geometry")
    }

    /// `y = G·delta` for a state vector `delta` — a pure `range(G)` error.
    fn g_times(g: &[[f64; 4]], delta: [f64; 4]) -> Vec<f64> {
        g.iter()
            .map(|row| (0..4).map(|k| row[k] * delta[k]).sum())
            .collect()
    }

    /// Any parity-space vector: `w = v − G·(S·v) = P⊥·v`, built by round-tripping
    /// an arbitrary `v` through the split (the returned residual is `P⊥·v`).
    fn parity_vector(g: &[[f64; 4]], v: &[f64]) -> Vec<f64> {
        common_mode_split(g, v)
            .expect("split of a nonzero vector exists")
            .detectable_residual
    }

    #[test]
    fn pure_common_mode_error_is_perfectly_blind() {
        let g = test_geometry();
        let delta = [1.0, -2.0, 0.5, 3.0];
        let delta_y = g_times(&g, delta);

        let split = common_mode_split(&g, &delta_y).expect("valid split");
        assert!(
            (split.blind_fraction - 1.0).abs() < 1e-9,
            "blind_fraction = {}",
            split.blind_fraction
        );
        assert!(
            split.detectable_norm < 1e-9,
            "detectable_norm = {}",
            split.detectable_norm
        );
        for k in 0..4 {
            assert!(
                (split.blind_dx[k] - delta[k]).abs() < 1e-9,
                "blind_dx[{}] = {} vs delta {}",
                k,
                split.blind_dx[k],
                delta[k]
            );
        }
    }

    #[test]
    fn pure_parity_error_is_fully_detectable() {
        let g = test_geometry();
        // An arbitrary seed, then project it into the parity space.
        let v = [0.3, 1.7, -0.4, 2.1, -1.1, 0.9];
        let w = parity_vector(&g, &v);
        let w_norm = norm(&w);
        assert!(w_norm > 1e-6, "parity component must be non-trivial");

        let split = common_mode_split(&g, &w).expect("valid split");
        assert!(
            split.blind_fraction.abs() < 1e-9,
            "blind_fraction = {}",
            split.blind_fraction
        );
        assert!(
            (split.detectable_norm - w_norm).abs() < 1e-9,
            "detectable_norm = {} vs ‖w‖ = {}",
            split.detectable_norm,
            w_norm
        );
    }

    #[test]
    fn split_is_additive() {
        let g = test_geometry();
        let delta = [1.0, -2.0, 0.5, 3.0];
        let g_delta = g_times(&g, delta);
        let v = [0.3, 1.7, -0.4, 2.1, -1.1, 0.9];
        let w = parity_vector(&g, &v);
        let w_norm = norm(&w);

        let a = 2.5;
        let b = -1.3;
        let combined: Vec<f64> = g_delta
            .iter()
            .zip(&w)
            .map(|(&gd, &wi)| a * gd + b * wi)
            .collect();

        let split = common_mode_split(&g, &combined).expect("valid split");
        for k in 0..4 {
            assert!(
                (split.blind_dx[k] - a * delta[k]).abs() < 1e-9,
                "blind_dx[{}] = {} vs a·delta {}",
                k,
                split.blind_dx[k],
                a * delta[k]
            );
        }
        assert!(
            (split.detectable_norm - b.abs() * w_norm).abs() < 1e-9,
            "detectable_norm = {} vs |b|·‖w‖ = {}",
            split.detectable_norm,
            b.abs() * w_norm
        );
    }

    #[test]
    fn too_few_or_singular_returns_none() {
        // Fewer than 5 rows -> None (geometry_from_los and common_mode_split).
        let user = [7.0e6, 0.0, 0.0];
        let few = [[8.0e6, 0.0, 0.0], [7.0e6, 1.0e6, 0.0]];
        assert!(geometry_from_los(user, &few).is_none());

        let g4 = vec![
            [-1.0, 0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0, 1.0],
            [0.0, 0.0, -1.0, 1.0],
            [-0.577, -0.577, -0.577, 1.0],
        ];
        assert!(common_mode_split(&g4, &[1.0, 1.0, 1.0, 1.0]).is_none());

        // Mismatched lengths -> None.
        let g = test_geometry();
        let wrong_len = vec![0.0; g.len() - 1];
        assert!(common_mode_split(&g, &wrong_len).is_none());

        // Zero perturbation -> None (blind fraction undefined).
        let zero = vec![0.0; g.len()];
        assert!(common_mode_split(&g, &zero).is_none());
    }
}
