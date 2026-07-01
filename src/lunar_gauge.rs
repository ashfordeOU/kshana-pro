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
}
