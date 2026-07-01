// SPDX-License-Identifier: AGPL-3.0-only
//! Coupled frame⊕timescale gauge / datum-defect theorem for lunar PNT.
//!
//! ## 9-vector layout
//! The coupled datum vector is `θ = [t_x, t_y, t_z, ε_x, ε_y, ε_z, s, δτ, δα]`, indexed as:
//! * `0..3`  — three-component translation `t` (metres)
//! * `3..6`  — three-component infinitesimal rotation `ε` (radians)
//! * `6`     — fractional scale `s` (dimensionless)
//! * `7`     — clock-offset `δτ` (seconds; preconditioned as `c·δτ` metres)
//! * `8`     — clock-rate `δα` (fractional; preconditioned in equivalent-range metres
//!   via `T_BASE_S · c`)
//!
//! Index constants: [`IDX_SCALE`] = 6, [`IDX_OFFSET`] = 7, [`IDX_RATE`] = 8.
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

/// Number of parameters in the coupled datum vector `θ`.
pub const N_GAUGE: usize = 9;

/// Index of the fractional-scale parameter in `θ` (the `s` entry).
pub const IDX_SCALE: usize = 6;

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
/// Signs follow the convention `δα = d_alpha_d_scale·δs + d_alpha_d_radial·δr + d_alpha_d_velocity·|δv|`.
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
        assert_eq!(IDX_SCALE, 6);
        assert_eq!(IDX_OFFSET, 7);
        assert_eq!(IDX_RATE, 8);
    }
}
