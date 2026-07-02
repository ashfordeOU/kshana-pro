// SPDX-License-Identifier: AGPL-3.0-only
//! Free-network (self-referential) fault-observability substrate for a sparse
//! multi-provider lunar constellation.
//!
//! Where the coupled-datum layer in [`crate::lunar_gauge`] builds *anchored*
//! observation rows — a node ranges to a body point whose geometry is known, so a
//! geometrically diverse network pins the reference frame (datum defect → 0) — this
//! module builds the *free-network* analog. Both endpoints of every measurement are
//! network nodes subject to the **same** datum `θ` (Helmert 7-parameter frame plus a
//! common clock offset/rate), so the network can only ever observe quantities that are
//! invariant under a re-choice of that shared datum.
//!
//! ## The differential observation
//! For an inter-node range `ρ_ab = ‖p_a − p_b‖` where both endpoints transform under the
//! same `θ`, the observation-row is `∂ρ_ab/∂θ = û_ab·(J_a − J_b)`, with `û_ab` the unit
//! line of sight and `J_a`, `J_b` each endpoint's position-Jacobian w.r.t. `θ`
//! (built from [`crate::lunar_datum::partials_datum7`]). Two structural cancellations
//! follow directly, and they are the physics of the free-network gauge:
//! * **Rigid frame (translations + rotations) cancels.** A rigid transform of the whole
//!   lunar frame moves both endpoints together and preserves every inter-node distance,
//!   so the three translation columns and the three rotation columns of every
//!   differential row vanish.
//! * **Common timescale cancels.** A single global clock offset/rate shared by both nodes
//!   cancels in the difference, so the `IDX_OFFSET` and `IDX_RATE` columns vanish.
//!
//! What survives is **scale**: a fractional scale change of the lunar frame stretches every
//! inter-node baseline by `‖p_a − p_b‖·δs`, so the scale column equals the baseline length
//! and is observable. The resulting null space is exactly eight-dimensional —
//! `{3 translations, 3 rotations, common offset, common rate}` — matching the published
//! free-network datum-defect result, with scale the one observable degree of freedom.
//!
//! ## Validation status
//! **Modelled (InternalConsistency).** The eight-dimensional gauge is a first-principles
//! consequence of rigid-transform invariance of inter-node ranges and common-mode clock
//! cancellation; the numeric defect is confirmed by [`crate::lunar_gauge::classify_null_space`]
//! on a representative real-DE440 multi-node, multi-epoch network. Geometry is
//! representative (fixed lunar-frame nodes), not a fitted deployment.

use crate::lunar_gauge::{IDX_OFFSET, N_GAUGE};
use crate::lunar_llr::Vec3;

/// Differential inter-node range row over the nine-vector coupled datum `θ`.
///
/// Both endpoints are network nodes given by their PA body-frame positions
/// `node_a_body`, `node_b_body`; they are carried into geocentric-inertial coordinates via
/// the real DE440 PA-frame orientation path ([`crate::lunar_orientation::de440_moon_pa_body_to_inertial`])
/// at epoch `t_tt_jc` (Julian centuries from J2000.0 TT). The epoch is required because the
/// frame-orientation columns of each endpoint's datum-Jacobian depend on the physical
/// libration `R(t)`.
///
/// The row is `∂ρ_ab/∂θ = û_ab·(J_a − J_b)` for the seven Helmert columns, computed as the
/// difference of two [`crate::lunar_datum::partials_datum7`] evaluations contracted with the
/// **same** line of sight `û_ab = (r_a − r_b)/‖r_a − r_b‖`. The two temporal columns
/// (`IDX_OFFSET`, `IDX_RATE`) are zero: a common timescale shared by both nodes cancels in
/// the difference.
///
/// The line of sight is formed from the Moon-relative inertial positions
/// `R(t)·p` (the geocentric Moon position, common to both nodes, cancels analytically and is
/// omitted to avoid a `~3.8e8 m` cancellation). This is exactly the rotation applied inside
/// `partials_datum7`, so the scale and rotation columns are numerically consistent.
///
/// By construction the three translation columns are identically zero (the translation
/// Jacobian is node-independent) and the three rotation columns vanish analytically
/// (`û_ab·(â_k × Δr) = 0` since `û_ab ∥ Δr`); only the scale column survives, equal to the
/// inter-node baseline length.
pub fn differential_range_row(
    node_a_body: Vec3,
    node_b_body: Vec3,
    t_tt_jc: f64,
) -> [f64; N_GAUGE] {
    // Moon-relative inertial positions (r − r_moon); the shared r_moon cancels in the LOS.
    let ra_rel = crate::lunar_orientation::de440_moon_pa_body_to_inertial(node_a_body, t_tt_jc);
    let rb_rel = crate::lunar_orientation::de440_moon_pa_body_to_inertial(node_b_body, t_tt_jc);
    let dv = [
        ra_rel[0] - rb_rel[0],
        ra_rel[1] - rb_rel[1],
        ra_rel[2] - rb_rel[2],
    ];
    let n = (dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2]).sqrt();
    let uhat = [dv[0] / n, dv[1] / n, dv[2] / n];

    // Each endpoint's datum-Jacobian contracted with the SAME line of sight.
    let ja = crate::lunar_datum::partials_datum7(uhat, node_a_body, t_tt_jc);
    let jb = crate::lunar_datum::partials_datum7(uhat, node_b_body, t_tt_jc);

    let mut row = [0.0_f64; N_GAUGE];
    for (c, r) in row.iter_mut().take(7).enumerate() {
        *r = ja[c] - jb[c];
    }
    // IDX_OFFSET and IDX_RATE remain 0.0: the common timescale cancels in the differential.
    row
}

/// Inter-node clock-difference row over the nine-vector coupled datum `θ`.
///
/// A simultaneous inter-node clock comparison senses `clock_a − clock_b`. Both nodes carry
/// the common timescale offset (`IDX_OFFSET`) with identical sensitivity, so the difference
/// cancels it; likewise the common rate (`IDX_RATE`) over a shared integration window. The
/// row therefore carries **no** common-datum information — it is the algebraic statement that
/// the common timescale is a two-dimensional gauge, the temporal analog of the rigid-frame
/// cancellation in [`differential_range_row`].
///
/// (Per-node clock differences are real nuisance parameters, but they lie outside the common
/// nine-vector datum modelled here. Anchoring the common timescale — making the offset/rate
/// observable — requires an external time tie such as [`crate::lunar_gauge::rate_tie_row`],
/// not an inter-node comparison.)
pub fn differential_clock_tie_row() -> [f64; N_GAUGE] {
    // Node a contributes +1 at IDX_OFFSET, node b −1: the common offset cancels (1 − 1 = 0).
    let node_a_offset = 1.0_f64;
    let node_b_offset = 1.0_f64;
    let mut row = [0.0_f64; N_GAUGE];
    row[IDX_OFFSET] = node_a_offset - node_b_offset;
    row
}

/// Assemble the free-network Fisher information matrix `GᵀWG` from weighted row blocks.
///
/// Reuses the coupled-datum preconditioning convention of
/// [`crate::lunar_gauge::assemble_coupled_info`]: each `(rows, sigma)` block is weighted by
/// `1/σ²`, columns 3..7 (scale + rotations) are divided by `R_MOON` to bring all nine
/// partials to `O(1)`, and the weighted outer products are accumulated. The null-space
/// structure (defect, spatial/temporal classification) is invariant under positive per-column
/// scaling, so the preconditioning does not change the gauge.
pub fn assemble_faultobs_info(blocks: &[(Vec<[f64; N_GAUGE]>, f64)]) -> Vec<Vec<f64>> {
    crate::lunar_gauge::assemble_coupled_info(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lunar_gauge::{classify_null_space, IDX_RATE, IDX_SCALE};

    /// 2024-01-01 TT (JD 2460310.5), inside the DE440 fixture window.
    const T0: f64 = (2_460_310.5 - 2_451_545.0) / 36_525.0;

    /// Four epochs at two-day steps inside the DE440 fixture window.
    fn epochs() -> [f64; 4] {
        std::array::from_fn(|k| T0 + (k as f64) * 2.0 / 36_525.0)
    }

    /// A representative set of lunar-frame network nodes (PA body-frame metres): the five
    /// near-side reflectors plus one elevated relay node, giving diverse inter-node baselines.
    fn network_nodes() -> Vec<Vec3> {
        let mut nodes: Vec<Vec3> = crate::lunar_llr::reflectors()
            .iter()
            .map(|r| r.pa_body_m)
            .collect();
        // An elevated relay/tower node (~2.0e6 m radius) for baseline diversity.
        nodes.push([1_600_000.0, 700_000.0, 900_000.0]);
        nodes
    }

    /// Stack differential inter-node range rows over all node pairs and all epochs.
    fn free_network_rows() -> Vec<[f64; N_GAUGE]> {
        let nodes = network_nodes();
        let eps = epochs();
        let mut rows = Vec::new();
        for &t in &eps {
            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    rows.push(differential_range_row(nodes[i], nodes[j], t));
                }
            }
        }
        rows
    }

    /// A rigid frame perturbation (translation or rotation) preserves every inter-node
    /// range, so the differential row must vanish on the translation columns (0,1,2) and
    /// the rotation columns (4,5,6); a scale perturbation stretches the baseline, so the
    /// scale column (3) must be nonzero.
    #[test]
    fn differential_row_rigid_transform_invariance() {
        let refl = crate::lunar_llr::reflectors();
        let node_a = refl[0].pa_body_m; // Apollo11
        let node_b = refl[3].pa_body_m; // Lunokhod1 (well-separated baseline)
        let row = differential_range_row(node_a, node_b, T0);

        // Translation columns: exactly zero (translation Jacobian is node-independent).
        for c in [0_usize, 1, 2] {
            assert!(
                row[c].abs() < 1e-9,
                "translation col {c} must vanish (rigid gauge), got {}",
                row[c]
            );
        }
        // Rotation columns: analytically zero (û ∥ Δr ⇒ û·(â_k × Δr) = 0).
        for c in [4_usize, 5, 6] {
            assert!(
                row[c].abs() < 1e-9,
                "rotation col {c} must vanish (rigid gauge), got {}",
                row[c]
            );
        }
        // Scale column: nonzero, equal to the inter-node baseline length.
        assert!(
            row[IDX_SCALE].abs() > 1.0,
            "scale col must be observable (nonzero), got {}",
            row[IDX_SCALE]
        );

        // The temporal columns are zero (common timescale cancels in the differential).
        assert_eq!(
            row[IDX_OFFSET], 0.0,
            "IDX_OFFSET must be 0 (common cancels)"
        );
        assert_eq!(row[IDX_RATE], 0.0, "IDX_RATE must be 0 (common cancels)");
    }

    /// The scale column equals the inter-node baseline length `‖p_a − p_b‖` (a fractional
    /// scale change stretches the baseline by that amount), and it is epoch-independent for
    /// fixed lunar-frame nodes because a rigid rotation preserves the distance.
    #[test]
    fn differential_scale_column_equals_baseline_length() {
        let refl = crate::lunar_llr::reflectors();
        let a = refl[0].pa_body_m;
        let b = refl[2].pa_body_m;
        let baseline =
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        for &t in &epochs() {
            let row = differential_range_row(a, b, t);
            let rel = (row[IDX_SCALE].abs() - baseline).abs() / baseline;
            assert!(
                rel < 1e-9,
                "scale col {} must equal baseline {} (rel {}), epoch {}",
                row[IDX_SCALE],
                baseline,
                rel,
                t
            );
        }
    }

    /// An inter-node clock difference carries no common-datum information: both the common
    /// offset and the common rate cancel, so the row is the zero nine-vector.
    #[test]
    fn differential_clock_tie_common_cancels() {
        let row = differential_clock_tie_row();
        assert_eq!(row[IDX_OFFSET], 0.0, "common offset must cancel");
        assert_eq!(row[IDX_RATE], 0.0, "common rate must cancel");
        assert!(
            row.iter().all(|&v| v == 0.0),
            "inter-node clock tie carries no common-datum information: {row:?}"
        );
    }

    /// THE GATE. A representative multi-node, multi-epoch real-DE440 free-network of
    /// differential inter-node range rows has datum defect exactly eight — the free-network
    /// gauge `N(G) = span{3 translations, 3 rotations, common offset, common rate}` — with
    /// scale the one observable degree of freedom. If this defect is not eight (or scale is
    /// unobservable) the self-referential reframe is wrong.
    #[test]
    fn free_network_gauge_is_eight() {
        let rows = free_network_rows();
        assert!(
            rows.len() >= 40,
            "network must be representative (many rows)"
        );
        let info = assemble_faultobs_info(&[(rows, 1.0)]);
        let cls = classify_null_space(&info, 1e-9);

        // Datum defect is exactly eight.
        assert_eq!(
            cls.defect, 8,
            "free-network datum defect must be 8, got {cls:?}"
        );
        // Six purely-spatial gauge directions: three translations + three rotations.
        assert_eq!(
            cls.dim_spatial, 6,
            "dim_spatial must be 6 (3 transl + 3 rot)"
        );
        // Two purely-temporal gauge directions: common offset + common rate.
        assert_eq!(
            cls.dim_temporal, 2,
            "dim_temporal must be 2 (offset + rate)"
        );
        // No spatial-temporal coupling in the free-network gauge.
        assert_eq!(cls.coupled_dim, 0, "coupled_dim must be 0");
        assert!(
            cls.p_st_norm < 1e-9,
            "p_st_norm must be ~0 (direct-sum gauge), got {}",
            cls.p_st_norm
        );

        // Scale (IDX_SCALE) is observable: its Fisher diagonal is bounded well away from zero
        // and dominates every other diagonal entry (all rigid/temporal columns are ~0).
        let scale_info = info[IDX_SCALE][IDX_SCALE];
        assert!(
            scale_info > 1.0,
            "scale diagonal must be well-conditioned (>1), got {scale_info}"
        );
        for (i, rowm) in info.iter().enumerate() {
            if i != IDX_SCALE {
                assert!(
                    rowm[i] <= 1e-6 * scale_info,
                    "non-scale diagonal [{i}] must be negligible vs scale, got {}",
                    rowm[i]
                );
            }
        }
    }
}
