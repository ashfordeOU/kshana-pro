// SPDX-License-Identifier: AGPL-3.0-only
//! Coupled frame⊕timescale gauge / datum-defect theorem for lunar PNT.
//!
//! ## 9-vector layout
//! The coupled datum vector is `θ = [t_x, t_y, t_z, s, θ_x, θ_y, θ_z, δτ, δα]`, indexed as:
//! * `0..3`  — three-component translation `t` (metres)
//! * `3`     — fractional scale `s` (dimensionless)
//! * `4..7`  — three-component infinitesimal rotation `θ` (radians)
//! * `7`     — clock-offset `δτ` (seconds; preconditioned as `c·δτ` metres)
//! * `8`     — clock-rate `δα` (fractional; preconditioned in equivalent-range metres
//!   via `T_BASE_S · c`)
//!
//! Index constants: [`IDX_SCALE`] = 3, [`IDX_OFFSET`] = 7, [`IDX_RATE`] = 8.
//! Gauge size: [`N_GAUGE`] = 9.
//!
//! ## Validation status
//! * **Validated (ExternalDataset):** the numpy reproduction of the coupled-Fisher null-space
//!   on real DE440 + real `lunar_time` coefficients — established in Task 5 (future).
//! * **Modelled (InternalConsistency):** the three frame→rate coupling Jacobian entries
//!   [`rate_frame_jacobian`] are derived from first principles (general relativistic clock
//!   rate at post-Newtonian order), cross-checked to the published [56, 59] µs/day band.
//!   They are NOT certified values from flight data.
//!
//! ## SI-second convention
//! All internal computations use SI seconds and c-defined metres.  Temporal columns in
//! the observation-row builders (Tasks 2–4) are carried as **equivalent-range metres**
//! to make all 9 partials O(1) for the Fisher preconditioner.

use crate::lunar_llr::Vec3;

/// Number of parameters in the coupled datum vector `θ`.
pub const N_GAUGE: usize = 9;

/// Index of the fractional-scale parameter in `θ` (the `s` entry).
pub const IDX_SCALE: usize = 3;

/// Index of the clock-offset parameter in `θ` (the `δτ` entry, in equivalent-range metres).
pub const IDX_OFFSET: usize = 7;

/// Index of the clock-rate parameter in `θ` (the `δα` entry, in equivalent-range metres).
pub const IDX_RATE: usize = 8;

/// The three partial derivatives linking a frame-datum perturbation to the
/// relativistic clock-rate offset `δα` of a lunar-surface clock.
///
/// All three entries are **Modelled** (first-principles, post-Newtonian order),
/// cross-checked against the [56, 59] µs/day reference band. They are NOT
/// certified operational values.
///
/// Signs: `δα = d_alpha_d_scale·δs + d_alpha_d_radial·δr − |d_alpha_d_velocity|·|δv|`
/// (the `d_alpha_d_velocity` coefficient is negative: `≈ −1.11e-14`; the last term subtracts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateFrameJacobian {
    /// `∂α/∂s = +U_moon / c²` ≈ +3.140e-11 per unit fractional scale.
    pub d_alpha_d_scale: f64,
    /// `∂α/∂v = −|v| / c²` ≈ −1.11e-14 per (m/s).
    pub d_alpha_d_velocity: f64,
    /// `∂α/∂r_radial = +g_moon / c²` ≈ +1.807e-17 per metre.
    pub d_alpha_d_radial: f64,
}

/// Compute the three frame→rate coupling Jacobian entries at TT epoch `t_tt_jc`
/// (Julian centuries since J2000.0).
///
/// Uses `GM_moon` from [`crate::forces::MU_MOON`] and `RE_moon`, `c²`, velocity from
/// [`crate::lunar_time`]. The velocity entry `d_alpha_d_velocity` is time-dependent
/// because the geocentric Moon speed varies ≈ ±1 % over the lunar month.
pub fn rate_frame_jacobian(t_tt_jc: f64) -> RateFrameJacobian {
    let mu = crate::forces::MU_MOON;
    let r = crate::lunar_time::RE_MOON_M;
    let c2 = crate::lunar_time::C2_M2_S2;

    // Gravitational potential at the lunar surface: U_moon = GM / R (m²/s²).
    let u_moon = mu / r;
    // Surface gravitational acceleration: g_moon = GM / R² (m/s²).
    let g_moon = mu / (r * r);

    // Geocentric Moon velocity at this epoch (m/s).
    let v = crate::lunar_time::moon_geocentric_velocity_m_s(t_tt_jc);
    let v_mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();

    RateFrameJacobian {
        d_alpha_d_scale: u_moon / c2,    // +3.140e-11 per unit scale
        d_alpha_d_velocity: -v_mag / c2, // ≈ −1.11e-14 per (m/s)
        d_alpha_d_radial: g_moon / c2,   // +1.807e-17 per metre
    }
}

/// Representative elapsed baseline for clock-rate column preconditioning (one Julian day, 86 400 s).
///
/// The `[IDX_RATE]` column of `oneway_range_row` is `elapsed_s / T_BASE_S`, placing it
/// O(1) for observation arcs near one day.  This is a conditioning choice: the null-space
/// structure (Task 3) is invariant under positive column scaling.
pub const T_BASE_S: f64 = 86_400.0;

/// One-way orbiter→surface-beacon range row over the 9-vector coupled datum.
///
/// Spatial columns 0..7 are identical to
/// [`lunar_datum::orbiter_range_row_datum7`](crate::lunar_datum::orbiter_range_row_datum7),
/// computed via the real DE440 PA-frame orientation path.
///
/// Temporal columns:
/// - `[IDX_OFFSET] = 1.0` — ∂range/∂(c·δτ) = 1; the node clock offset enters directly.
/// - `[IDX_RATE] = elapsed_s / T_BASE_S` — rate accumulates linearly with elapsed time;
///   dividing by `T_BASE_S` keeps the entry O(1).
///
/// **Modelled** — geometry is representative (circular orbit), not a fitted ephemeris.
pub fn oneway_range_row(
    r_orbiter_inertial: Vec3,
    beacon_pa_body_m: Vec3,
    t_tt_jc: f64,
    elapsed_s: f64,
) -> [f64; 9] {
    let d7 =
        crate::lunar_datum::orbiter_range_row_datum7(r_orbiter_inertial, beacon_pa_body_m, t_tt_jc);
    let mut row = [0.0_f64; N_GAUGE];
    row[..7].copy_from_slice(&d7);
    row[IDX_OFFSET] = 1.0;
    row[IDX_RATE] = elapsed_s / T_BASE_S;
    row
}

/// Two-way (round-trip) orbiter↔surface-beacon range row over the 9-vector coupled datum.
///
/// Spatial columns 0..7 are identical to `oneway_range_row` at the same geometry — the
/// beacon position enters equally in both legs of the round-trip.
///
/// Temporal columns:
/// - `[IDX_OFFSET] = 0.0` — round-trip cancels the common node clock offset.
/// - `[IDX_RATE]   = 0.0` — round-trip cancels the common node clock rate.
///
/// Adding two-way rows to a design matrix lifts the offset–rate near-defect without
/// introducing new spatial information (two-way lift, Task 4).
pub fn twoway_range_row(
    r_orbiter_inertial: Vec3,
    beacon_pa_body_m: Vec3,
    t_tt_jc: f64,
) -> [f64; 9] {
    let d7 =
        crate::lunar_datum::orbiter_range_row_datum7(r_orbiter_inertial, beacon_pa_body_m, t_tt_jc);
    let mut row = [0.0_f64; N_GAUGE];
    row[..7].copy_from_slice(&d7);
    // IDX_OFFSET and IDX_RATE remain 0.0: round-trip cancellation.
    row
}

/// External absolute-time tie row: observes the clock offset `δτ` directly.
///
/// Returns `[0, 0, 0, 0, 0, 0, 0, 1.0, 0.0]`, representing a common-view Earth–Moon
/// time comparison (e.g. via `timegeo::common_view_offset`) that measures the difference
/// between LTC and TT.  This row lifts the pure temporal-offset datum defect.
pub fn time_tie_row() -> [f64; 9] {
    let mut row = [0.0_f64; N_GAUGE];
    row[IDX_OFFSET] = 1.0;
    row
}

/// Relativistic rate-tie row: the rank-1 `(J,1)` coupling between the scale datum `s`
/// and the clock-rate offset `δα` within the 9-vector.
///
/// A fractional scale shift `δs` changes the mean lunar gravitational potential seen by
/// surface clocks, inducing a relativistic rate offset `δα = (∂α/∂s)·δs`.  This row
/// encodes that constraint:
///
/// `[IDX_RATE] = 1.0`,  `[IDX_SCALE] = −(∂α/∂s)`,  all other 7 entries = 0.0.
///
/// **Note:** `∂α/∂v` and `∂α/∂r_radial` are real relativistic couplings but do NOT map
/// onto the 9-vector datum (no velocity gauge; uniform translation cancels on a symmetric
/// shell).  They appear in the example envelope (Task 7) as part of the physical
/// rate-uncertainty budget, not as datum-level tie rows.
///
/// The coupling coefficient `d_alpha_d_scale` is **Modelled** (first-principles,
/// cross-checked to the [56, 59] µs/day band).
pub fn rate_tie_row(t_tt_jc: f64) -> [f64; 9] {
    let jac = rate_frame_jacobian(t_tt_jc);
    let mut row = [0.0_f64; N_GAUGE];
    row[IDX_RATE] = 1.0;
    row[IDX_SCALE] = -jac.d_alpha_d_scale;
    row
}

/// Assemble a combined 9×9 coupled Fisher information matrix from multiple observation blocks.
///
/// Mirrors [`crate::lunar_identifiability::assemble_multi_info`] for the 9-vector
/// coupled datum.  For each `(rows, sigma)` block: skip if empty; weight `1/σ²`;
/// precondition by dividing columns 3..7 (scale + rotations, indices 3,4,5,6) by
/// [`crate::lunar_time::RE_MOON_M`]; accumulate `weight · rowᵀrow` via
/// [`crate::fim::information_matrix`].
///
/// Preconditioning brings all 9 partial-derivative columns to O(1), reducing the
/// condition number of the Fisher matrix.  The null-space structure (defect, subspace
/// classification, Schur coupling) is invariant under positive per-column scaling.
///
/// Returns a 9×9 zero matrix if all blocks are empty.
pub fn assemble_coupled_info(blocks: &[(Vec<[f64; 9]>, f64)]) -> Vec<Vec<f64>> {
    let mut combined = vec![vec![0.0_f64; N_GAUGE]; N_GAUGE];
    for (rows, sigma) in blocks {
        if rows.is_empty() {
            continue;
        }
        let weight = 1.0 / (sigma * sigma);
        let r_moon = crate::lunar_time::RE_MOON_M;
        let pre_rows: Vec<Vec<f64>> = rows
            .iter()
            .map(|r| {
                let mut row = r.to_vec();
                row[3..7].iter_mut().for_each(|v| *v /= r_moon);
                row
            })
            .collect();
        let block_weights = vec![weight; pre_rows.len()];
        let block_info = crate::fim::information_matrix(&pre_rows, &block_weights);
        for (ci, bi) in combined.iter_mut().zip(block_info.iter()) {
            for (cv, bv) in ci.iter_mut().zip(bi.iter()) {
                *cv += bv;
            }
        }
    }
    combined
}

/// Classification of the null space of the coupled frame⊕timescale Fisher information matrix.
///
/// Decomposes the `d`-dimensional null space into:
/// - **purely-spatial** directions (null vectors invisible in the temporal rows 7..9)
/// - **purely-temporal** directions (null vectors invisible in the spatial rows 0..7)
/// - **coupled** directions (null vectors projecting onto both subspaces)
///
/// The Frobenius norm of the off-diagonal projector block `P[0..7][7..9]` (where
/// `P = U·Uᵀ` is the rank-`d` null-space projector) quantifies the spatial–temporal
/// coupling strength.
///
/// All classifiers are **basis-invariant**: they depend only on the subspaces spanned
/// by the null eigenvectors, not on the specific orthonormal basis returned by the
/// Jacobi eigensolver.
#[derive(Clone, Copy, Debug)]
pub struct CoupledGaugeClass {
    /// Total null-space dimension (datum defect).
    pub defect: usize,
    /// Number of purely spatial null directions (invisible in temporal rows 7..9).
    ///
    /// `= defect − rank(U_T)` where `U_T` = rows 7..9 of the null-space basis.
    pub dim_spatial: usize,
    /// Number of purely temporal null directions (invisible in spatial rows 0..7).
    ///
    /// `= defect − rank(U_S)` where `U_S` = rows 0..7 of the null-space basis.
    pub dim_temporal: usize,
    /// Number of coupled null directions projecting onto both subspaces.
    ///
    /// `= rank(U_S) + rank(U_T) − defect`.
    pub coupled_dim: usize,
    /// Frobenius norm of the off-diagonal block `(U·Uᵀ)[0..7][7..9]`.
    ///
    /// Zero when the null space is a direct sum of spatial and temporal subspaces
    /// (no spatial–temporal coupling).  Positive when coupled null vectors exist.
    pub p_st_norm: f64,
}

/// Classify the null space given an orthonormal null-space basis U directly.
///
/// `null_space[r][c]` = component `r` of null vector `c`; columns must be orthonormal.
/// The Gram-rank uses an absolute 1e-9 threshold, sound because the null columns are
/// orthonormal so Gram eigenvalues lie in [0,1]; a real near-defect with a sub-threshold
/// temporal projection may sit on the spatial/coupled boundary.
fn classify_from_null_space(null_space: &[Vec<f64>], defect: usize) -> CoupledGaugeClass {
    if defect == 0 {
        return CoupledGaugeClass {
            defect: 0,
            dim_spatial: 0,
            dim_temporal: 0,
            coupled_dim: 0,
            p_st_norm: 0.0,
        };
    }

    // Compute the rank of the submatrix formed by rows row_start..row_end of null_space
    // (a p×d matrix A), defined as the number of eigenvalues of A·Aᵀ exceeding 1e-9.
    let subspace_rank = |row_start: usize, row_end: usize| -> usize {
        let p = row_end - row_start;
        // Gram[i][j] = Σ_{c=0}^{d-1} A[i][c] · A[j][c]
        let mut gram = vec![vec![0.0_f64; p]; p];
        for i in 0..p {
            for j in i..p {
                let s: f64 = null_space[row_start + i]
                    .iter()
                    .zip(null_space[row_start + j].iter())
                    .map(|(&a, &b)| a * b)
                    .sum();
                gram[i][j] = s;
                gram[j][i] = s;
            }
        }
        crate::fim::sym_eig(&gram)
            .values
            .iter()
            .filter(|&&v| v > 1e-9)
            .count()
    };

    let rs = subspace_rank(0, 7); // rank of U_S (spatial rows 0..7)
    let rt = subspace_rank(7, 9); // rank of U_T (temporal rows 7..9)

    let dim_spatial = defect - rt;
    let dim_temporal = defect - rs;
    debug_assert!(
        rs + rt >= defect,
        "rs={rs} + rt={rt} < defect={defect}: coupled_dim would underflow"
    );
    let coupled_dim = (rs + rt).saturating_sub(defect);

    // p_st_norm = ‖P[0..7][7..9]‖_F where P = U·Uᵀ (the null-space projector).
    // P[i][j] = Σ_{c=0}^{d-1} null_space[i][c] · null_space[j][c]
    let mut p_st_sq = 0.0_f64;
    for i in 0..7 {
        for j in 7..9 {
            let p_ij: f64 = null_space[i]
                .iter()
                .zip(null_space[j].iter())
                .map(|(&a, &b)| a * b)
                .sum();
            p_st_sq += p_ij * p_ij;
        }
    }

    CoupledGaugeClass {
        defect,
        dim_spatial,
        dim_temporal,
        coupled_dim,
        p_st_norm: p_st_sq.sqrt(),
    }
}

/// Classify the null space of a 9×9 coupled Fisher information matrix.
///
/// ## Algorithm (basis-invariant)
///
/// 1. Compute [`crate::fim::crlb`] with `rel_tol`.  Retrieve `null_space` U (layout:
///    `null_space[row][col]` = component `row` of null vector `col`) and defect `d`.
///    Return the all-zero class if `d == 0`.
/// 2. Let `U_S` = rows 0..7 and `U_T` = rows 7..9 of U (7×d and 2×d sub-matrices).
/// 3. Compute `rank(A)` as the number of eigenvalues of the Gram matrix `A·Aᵀ`
///    that exceed `1e-9`.  (Columns of U are orthonormal, so Gram eigenvalues ∈ [0,1];
///    `1e-9` cleanly separates zero from O(1).)
/// 4. Derive: `dim_spatial = d − rank(U_T)`, `dim_temporal = d − rank(U_S)`,
///    `coupled_dim = rank(U_S) + rank(U_T) − d`.
/// 5. Form the off-diagonal block of the null projector `P = U·Uᵀ` (9×9) restricted
///    to rows 0..7 and columns 7..9 (7×2), and return its Frobenius norm as `p_st_norm`.
pub fn classify_null_space(info: &[Vec<f64>], rel_tol: f64) -> CoupledGaugeClass {
    let cr = crate::fim::crlb(info, rel_tol);
    classify_from_null_space(&cr.null_space, cr.defect)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference epoch: J2000.0 (t_tt_jc = 0.0).
    const T_J2000: f64 = 0.0;

    /// Relative tolerance for the gravitational entries (scale + radial).
    const REL_TOL_GR: f64 = 1e-3;

    /// Relative tolerance for the velocity entry (looser — velocity is time-dependent).
    const REL_TOL_VEL: f64 = 5e-2;

    fn rel_err(got: f64, expect: f64) -> f64 {
        ((got - expect) / expect).abs()
    }

    #[test]
    fn d_alpha_d_scale_approx_3_140e_minus_11() {
        let jac = rate_frame_jacobian(T_J2000);
        // Expected: +U_moon/c² ≈ +3.140e-11.
        let expected = 3.140e-11_f64;
        assert!(
            jac.d_alpha_d_scale > 0.0,
            "d_alpha_d_scale must be positive, got {}",
            jac.d_alpha_d_scale
        );
        assert!(
            rel_err(jac.d_alpha_d_scale, expected) < REL_TOL_GR,
            "d_alpha_d_scale {:.6e} deviates from {:.6e} by {:.2e} (tol {:.2e})",
            jac.d_alpha_d_scale,
            expected,
            rel_err(jac.d_alpha_d_scale, expected),
            REL_TOL_GR
        );
    }

    #[test]
    fn d_alpha_d_radial_approx_1_807e_minus_17() {
        let jac = rate_frame_jacobian(T_J2000);
        // Expected: +g_moon/c² ≈ +1.807e-17.
        let expected = 1.807e-17_f64;
        assert!(
            jac.d_alpha_d_radial > 0.0,
            "d_alpha_d_radial must be positive, got {}",
            jac.d_alpha_d_radial
        );
        assert!(
            rel_err(jac.d_alpha_d_radial, expected) < REL_TOL_GR,
            "d_alpha_d_radial {:.6e} deviates from {:.6e} by {:.2e} (tol {:.2e})",
            jac.d_alpha_d_radial,
            expected,
            rel_err(jac.d_alpha_d_radial, expected),
            REL_TOL_GR
        );
    }

    #[test]
    fn d_alpha_d_velocity_approx_neg_1_11e_minus_14() {
        let jac = rate_frame_jacobian(T_J2000);
        // Expected: −|v|/c² ≈ −1.11e-14 (|v| ≈ 1 km/s).
        let expected = -1.11e-14_f64;
        assert!(
            jac.d_alpha_d_velocity < 0.0,
            "d_alpha_d_velocity must be negative, got {}",
            jac.d_alpha_d_velocity
        );
        assert!(
            rel_err(jac.d_alpha_d_velocity, expected) < REL_TOL_VEL,
            "d_alpha_d_velocity {:.6e} deviates from {:.6e} by {:.2e} (tol {:.2e})",
            jac.d_alpha_d_velocity,
            expected,
            rel_err(jac.d_alpha_d_velocity, expected),
            REL_TOL_VEL
        );
    }

    #[test]
    fn d_alpha_d_radial_equals_scale_over_r_moon() {
        let jac = rate_frame_jacobian(T_J2000);
        let r = crate::lunar_time::RE_MOON_M;
        let expected = jac.d_alpha_d_scale / r;
        // This is an exact algebraic identity: g/c² = (U/R)/c² = (U/c²)/R.
        let abs_err = (jac.d_alpha_d_radial - expected).abs();
        assert!(
            abs_err < expected.abs() * 1e-12,
            "d_alpha_d_radial {:.6e} != d_alpha_d_scale/RE_MOON_M {:.6e} (abs err {:.2e})",
            jac.d_alpha_d_radial,
            expected,
            abs_err
        );
    }

    #[test]
    fn index_consts_are_correct() {
        assert_eq!(N_GAUGE, 9);
        assert_eq!(IDX_SCALE, 3);
        assert_eq!(IDX_OFFSET, 7);
        assert_eq!(IDX_RATE, 8);
    }

    // ── Task 2 tests ────────────────────────────────────────────────────────────────

    /// 2024-01-01 TT (JD 2460310.5), inside the DE440 fixture window.
    const T0_FIXTURE: f64 = (2_460_310.5 - 2_451_545.0) / 36_525.0;

    #[test]
    fn time_tie_row_equals_pure_offset_vector() {
        let row = time_tie_row();
        let expected: [f64; 9] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        assert_eq!(row, expected, "time_tie_row must equal [0,0,0,0,0,0,0,1,0]");
    }

    #[test]
    fn rate_tie_row_has_correct_structure() {
        let jac = rate_frame_jacobian(T_J2000);
        let row = rate_tie_row(T_J2000);
        assert_eq!(row[IDX_RATE], 1.0, "IDX_RATE must be 1.0");
        assert_eq!(
            row[IDX_SCALE], -jac.d_alpha_d_scale,
            "IDX_SCALE must be -d_alpha_d_scale"
        );
        assert!(
            row[IDX_SCALE] < 0.0,
            "IDX_SCALE must be negative (d_alpha_d_scale > 0)"
        );
        for (i, &v) in row.iter().enumerate() {
            if i != IDX_RATE && i != IDX_SCALE {
                assert_eq!(v, 0.0, "rate_tie_row[{i}] must be 0.0, got {v}");
            }
        }
    }

    #[test]
    fn twoway_range_row_has_zero_at_offset_and_rate() {
        let r_orb =
            crate::lunar_datum::orbiter_position(100.0, 85.0, 30.0, 40.0, T0_FIXTURE, T0_FIXTURE);
        let beacon: [f64; 3] = [3.0e5, 1.70e6, 2.0e5];
        let row = twoway_range_row(r_orb, beacon, T0_FIXTURE);
        assert_eq!(row[IDX_OFFSET], 0.0, "twoway IDX_OFFSET must be 0.0");
        assert_eq!(row[IDX_RATE], 0.0, "twoway IDX_RATE must be 0.0");
    }

    #[test]
    fn oneway_range_row_has_correct_temporal_entries() {
        let r_orb =
            crate::lunar_datum::orbiter_position(100.0, 85.0, 30.0, 40.0, T0_FIXTURE, T0_FIXTURE);
        let beacon: [f64; 3] = [3.0e5, 1.70e6, 2.0e5];
        let elapsed = 3_600.0_f64;
        let row = oneway_range_row(r_orb, beacon, T0_FIXTURE, elapsed);
        assert_eq!(row[IDX_OFFSET], 1.0, "oneway IDX_OFFSET must be 1.0");
        assert_eq!(
            row[IDX_RATE],
            elapsed / T_BASE_S,
            "oneway IDX_RATE must equal elapsed/T_BASE_S"
        );
    }

    #[test]
    fn twoway_spatial_equals_oneway_spatial_at_same_geometry() {
        let r_orb =
            crate::lunar_datum::orbiter_position(100.0, 85.0, 30.0, 40.0, T0_FIXTURE, T0_FIXTURE);
        let beacon: [f64; 3] = [3.0e5, 1.70e6, 2.0e5];
        let elapsed = 3_600.0_f64;
        let oneway = oneway_range_row(r_orb, beacon, T0_FIXTURE, elapsed);
        let twoway = twoway_range_row(r_orb, beacon, T0_FIXTURE);
        for i in 0..7 {
            assert_eq!(
                oneway[i], twoway[i],
                "spatial[{i}]: oneway={}, twoway={}",
                oneway[i], twoway[i]
            );
        }
    }

    #[test]
    fn oneway_spatial_equals_orbiter_range_row_datum7() {
        let r_orb =
            crate::lunar_datum::orbiter_position(100.0, 85.0, 30.0, 40.0, T0_FIXTURE, T0_FIXTURE);
        let beacon: [f64; 3] = [3.0e5, 1.70e6, 2.0e5];
        let elapsed = 3_600.0_f64;
        let oneway = oneway_range_row(r_orb, beacon, T0_FIXTURE, elapsed);
        let d7 = crate::lunar_datum::orbiter_range_row_datum7(r_orb, beacon, T0_FIXTURE);
        for i in 0..7 {
            assert_eq!(
                oneway[i], d7[i],
                "spatial[{i}]: oneway={}, datum7={}",
                oneway[i], d7[i]
            );
        }
    }

    // ── Task 3 tests ────────────────────────────────────────────────────────────────

    /// Build a 9×9 diagonal matrix with the given diagonal entries.
    fn make_diag_info(diag: [f64; 9]) -> Vec<Vec<f64>> {
        let mut m = vec![vec![0.0_f64; 9]; 9];
        for (i, row) in m.iter_mut().enumerate() {
            row[i] = diag[i];
        }
        m
    }

    /// (a) Direct-sum null: info_A = I₉ with diagonal[3]=0 and diagonal[7]=0.
    /// Null = span{e₃, e₇} (direct sum: one spatial, one temporal, no coupling).
    #[test]
    fn classify_direct_sum_null() {
        let info = make_diag_info([1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0]);
        let cls = classify_null_space(&info, 1e-9);
        assert_eq!(cls.defect, 2, "defect");
        assert_eq!(cls.dim_spatial, 1, "dim_spatial (e₃ is scale ∈ 0..7)");
        assert_eq!(cls.dim_temporal, 1, "dim_temporal (e₇ is offset ∈ 7..9)");
        assert_eq!(cls.coupled_dim, 0, "coupled_dim");
        assert!(
            cls.p_st_norm < 1e-9,
            "p_st_norm should be ≈0 for direct-sum null, got {}",
            cls.p_st_norm
        );
    }

    /// (b) Coupled null: info_B = I₉ − ŵŵᵀ with ŵ = (e₃+e₇)/√2.
    /// Null = span{ŵ} — the unique direction couples scale (spatial) and offset (temporal).
    #[test]
    fn classify_coupled_null() {
        let sqrt2 = 2.0_f64.sqrt();
        let w3 = 1.0 / sqrt2;
        let w7 = 1.0 / sqrt2;
        let mut info = vec![vec![0.0_f64; 9]; 9];
        // Start from I₉.
        for (i, row) in info.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        // Subtract ŵŵᵀ (entries at rows/cols {3, 7}).
        info[3][3] -= w3 * w3;
        info[7][7] -= w7 * w7;
        info[3][7] -= w3 * w7;
        info[7][3] -= w7 * w3;

        let cls = classify_null_space(&info, 1e-9);
        assert_eq!(cls.defect, 1, "defect");
        assert_eq!(cls.coupled_dim, 1, "coupled_dim");
        assert!(
            (cls.p_st_norm - 0.5_f64).abs() < 1e-9,
            "p_st_norm should be ≈0.5, got {}",
            cls.p_st_norm
        );
        assert_eq!(cls.dim_spatial, 0, "dim_spatial");
        assert_eq!(cls.dim_temporal, 0, "dim_temporal");
    }

    /// (c) Basis-invariance: info_C has null = span{e₃, e₇} but non-unit observable eigenvalues.
    /// Result must be identical to test (a) regardless of the eigensolver's basis choice.
    /// Note: axis-aligned matrices yield the identity null basis from sym_eig; the genuine
    /// rotated-basis invariance test is `classify_is_invariant_under_null_basis_rotation`.
    #[test]
    fn classify_basis_invariant() {
        let info = make_diag_info([2.0, 3.0, 4.0, 0.0, 5.0, 6.0, 7.0, 0.0, 8.0]);
        let cls = classify_null_space(&info, 1e-9);
        assert_eq!(cls.defect, 2, "defect (basis-invariant)");
        assert_eq!(cls.dim_spatial, 1, "dim_spatial (basis-invariant)");
        assert_eq!(cls.dim_temporal, 1, "dim_temporal (basis-invariant)");
        assert_eq!(cls.coupled_dim, 0, "coupled_dim (basis-invariant)");
        assert!(
            cls.p_st_norm < 1e-9,
            "p_st_norm should be ≈0 (basis-invariant), got {}",
            cls.p_st_norm
        );
    }

    /// Genuine rotated-basis invariance test: builds U_axis (9×2, columns e₃ and (e₄+e₇)/√2),
    /// verifies `classify_from_null_space` gives the correct class on the axis-aligned basis,
    /// then rotates the null basis by θ=0.6 rad and asserts the classification is unchanged.
    ///
    /// This subspace has rs=2 (both columns project onto spatial rows) and rt=1 (only the
    /// coupled column projects onto temporal rows), so the test also guards against a U_S/U_T
    /// transposition bug.
    #[test]
    fn classify_is_invariant_under_null_basis_rotation() {
        let inv_sqrt2 = 1.0_f64 / 2.0_f64.sqrt();

        // Build U_axis: 9 rows × 2 columns (null_space[r][c] = component r of null vector c).
        // Column 0 = e₃ (pure spatial: scale at index 3).
        // Column 1 = (e₄ + e₇)/√2 (coupled: θ_x at row 4 ∈ spatial, δτ at row 7 ∈ temporal).
        let mut u_axis = vec![vec![0.0_f64; 2]; 9];
        u_axis[3][0] = 1.0; // e₃: scale component
        u_axis[4][1] = inv_sqrt2; // (e₄+e₇)/√2: θ_x (spatial) component
        u_axis[7][1] = inv_sqrt2; // (e₄+e₇)/√2: δτ (temporal) component

        // Axis-aligned basis: check expected counts and p_st_norm = 0.5.
        let cls_axis = classify_from_null_space(&u_axis, 2);
        assert_eq!(cls_axis.defect, 2, "axis: defect");
        assert_eq!(
            cls_axis.dim_spatial, 1,
            "axis: dim_spatial (e₃ is pure spatial)"
        );
        assert_eq!(cls_axis.dim_temporal, 0, "axis: dim_temporal");
        assert_eq!(cls_axis.coupled_dim, 1, "axis: coupled_dim");
        assert!(
            (cls_axis.p_st_norm - 0.5_f64).abs() < 1e-9,
            "axis: p_st_norm should be 0.5, got {}",
            cls_axis.p_st_norm
        );

        // Rotate the null basis by θ=0.6 rad: U_rot = U_axis · Q where
        // Q = [[cosθ, -sinθ], [sinθ, cosθ]].  U_rot spans the same subspace but neither
        // column is pure spatial or temporal — the real invariance check.
        let theta: f64 = 0.6;
        let (cos_t, sin_t) = (theta.cos(), theta.sin());
        let mut u_rot = vec![vec![0.0_f64; 2]; 9];
        for r in 0..9 {
            u_rot[r][0] = u_axis[r][0] * cos_t + u_axis[r][1] * sin_t;
            u_rot[r][1] = u_axis[r][0] * (-sin_t) + u_axis[r][1] * cos_t;
        }

        // Rotated basis must yield identical classification (P = U_rot·U_rot^T = U_axis·U_axis^T).
        let cls_rot = classify_from_null_space(&u_rot, 2);
        assert_eq!(cls_rot.defect, 2, "rotated: defect");
        assert_eq!(cls_rot.dim_spatial, 1, "rotated: dim_spatial");
        assert_eq!(cls_rot.dim_temporal, 0, "rotated: dim_temporal");
        assert_eq!(cls_rot.coupled_dim, 1, "rotated: coupled_dim");
        assert!(
            (cls_rot.p_st_norm - 0.5_f64).abs() < 1e-9,
            "rotated: p_st_norm should be 0.5, got {}",
            cls_rot.p_st_norm
        );
    }
}
