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

use crate::lunar_gauge::{IDX_OFFSET, N_GAUGE, T_BASE_S};
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

// ── Per-node state model (the fault-observability substrate) ─────────────────────────
//
// The differential rows above verify the datum⊕timescale gauge on the nine-vector datum
// `θ`, but that state is rank-1 (only scale is observable) — too impoverished to carry a
// fault-observability or untrusted-peer analysis. The estimated state of an autonomous
// constellation is instead PER NODE: every node carries a position, a clock offset and a
// clock rate. The eight-dimensional datum⊕timescale gauge of [`differential_range_row`]
// (the P4 free-network gauge — three translations, three rotations, a common clock offset
// and a common clock rate) reappears here as a genuine SUBSPACE of the per-node null space,
// while the remaining `state_dim − 8` directions are observable. `range(G)` and the parity
// space are therefore both non-trivial — the standard secure-state-estimation substrate.

/// Number of datum⊕timescale gauge generators returned by [`datum_gauge_generators`].
///
/// The eight are three translations, three rotations, a common clock offset and a common
/// clock rate — the P4 free-network gauge embedded in per-node coordinates.
pub const N_DATUM_GAUGE: usize = 8;

/// Cross product `a × b`.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Index layout and dimension of the stacked per-node state vector.
///
/// Each of the `M = n_nodes` nodes carries a position error `δp_j ∈ ℝ³`, a clock-offset
/// error `δτ_j` and a clock-rate error `δα_j`. Stacked:
///
/// ```text
/// x = [ δp_1 … δp_M | δτ_1 … δτ_M | δα_1 … δα_M ],   state_dim = 5·M
/// ```
///
/// with three contiguous blocks:
/// * **positions** `[0, 3M)` — node `j` occupies `[3j, 3j+3)` ([`Self::pos_idx`]),
/// * **offsets** `[3M, 4M)` — node `j` at `3M + j` ([`Self::off_idx`]),
/// * **rates** `[4M, 5M)` — node `j` at `4M + j` ([`Self::rate_idx`]).
///
/// The eight-dimensional datum⊕timescale gauge ([`datum_gauge_generators`]) is a subspace
/// of the null space of the per-node information matrix; the other `5M − 8` directions are
/// observable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkLayout {
    /// Number of network nodes `M`.
    pub n_nodes: usize,
    /// Dimension of the stacked per-node state, `5·M`.
    pub state_dim: usize,
}

impl NetworkLayout {
    /// Layout for a network of `n_nodes` nodes (`state_dim = 5·n_nodes`).
    pub fn new(n_nodes: usize) -> Self {
        Self {
            n_nodes,
            state_dim: 5 * n_nodes,
        }
    }

    /// First index of node `j`'s position block; `δp_j` occupies `pos_idx(j)..pos_idx(j)+3`.
    pub fn pos_idx(&self, j: usize) -> usize {
        3 * j
    }

    /// Index of node `j`'s clock offset `δτ_j`.
    pub fn off_idx(&self, j: usize) -> usize {
        3 * self.n_nodes + j
    }

    /// Index of node `j`'s clock rate `δα_j`.
    pub fn rate_idx(&self, j: usize) -> usize {
        4 * self.n_nodes + j
    }
}

/// One differential inter-node one-way range row in per-node coordinates.
///
/// For a one-way range `ρ_ab` between nodes `a` and `b`, the linearised sensitivity to the
/// per-node state is
/// * `∂ρ/∂δp_a = +û_ab`, `∂ρ/∂δp_b = −û_ab` — the unit line of sight and its negative,
/// * `∂ρ/∂δτ_a = +1`, `∂ρ/∂δτ_b = −1` — each node clock offset enters directly,
/// * `∂ρ/∂δα_a = +elapsed_s/T_BASE_S`, `∂ρ/∂δα_b = −elapsed_s/T_BASE_S` — each rate
///   accumulates over the elapsed integration window; dividing by [`T_BASE_S`] keeps the
///   entry `O(1)`, mirroring [`crate::lunar_gauge::oneway_range_row`].
///
/// `u_ab` is the unit line of sight from real DE440 geometry
/// (`û_ab = (r_a − r_b)/‖r_a − r_b‖`, the shared Moon-relative offset cancelling). The
/// returned row has length `layout.state_dim` and is otherwise zero.
pub fn pernode_range_row(
    layout: &NetworkLayout,
    node_a: usize,
    node_b: usize,
    u_ab: [f64; 3],
    elapsed_s: f64,
) -> Vec<f64> {
    let mut row = vec![0.0_f64; layout.state_dim];
    let (pa, pb) = (layout.pos_idx(node_a), layout.pos_idx(node_b));
    for k in 0..3 {
        row[pa + k] = u_ab[k];
        row[pb + k] = -u_ab[k];
    }
    row[layout.off_idx(node_a)] = 1.0;
    row[layout.off_idx(node_b)] = -1.0;
    let kappa = elapsed_s / T_BASE_S;
    row[layout.rate_idx(node_a)] = kappa;
    row[layout.rate_idx(node_b)] = -kappa;
    row
}

/// Assemble the per-node Fisher information matrix `GᵀWG` (`state_dim × state_dim`).
///
/// Each `(row, sigma)` pair contributes `w · rowᵀrow` with diagonal weight `w = 1/σ²`.
/// No column preconditioning is applied: every partial is already `O(1)` — unit
/// line-of-sight components, unit offset sensitivities, and `elapsed_s/T_BASE_S` rate
/// sensitivities — so the matrix is well scaled for the eigenvalue-count rank diagnosis.
pub fn assemble_pernode_info(rows: &[(Vec<f64>, f64)], state_dim: usize) -> Vec<Vec<f64>> {
    let mut info = vec![vec![0.0_f64; state_dim]; state_dim];
    for (row, sigma) in rows {
        let w = 1.0 / (sigma * sigma);
        for p in 0..state_dim {
            let jw = row[p] * w;
            if jw == 0.0 {
                continue;
            }
            for q in 0..state_dim {
                info[p][q] += jw * row[q];
            }
        }
    }
    info
}

/// The eight analytic datum⊕timescale gauge generators in per-node coordinates.
///
/// These are the P4 free-network gauge directions embedded in the per-node state: a rigid
/// transform of the whole lunar frame plus a common timescale re-choice leaves every
/// differential observable unchanged, so each generator lies in the null space of the
/// per-node information matrix built from [`pernode_range_row`]. The eight are
/// * three **translations** — `δp_j += ê_k` for every node `j` (`k = 0,1,2`),
/// * three **rotations** — `δp_j += ê_k × p_j` for every node `j`, with `p_j` its inertial
///   position (`node_positions[j]`); the line of sight is parallel to the inter-node
///   baseline, so `û_ab·(ê_k × (p_a − p_b)) = 0`,
/// * one **common clock offset** — `δτ_j += 1` for every node,
/// * one **common clock rate** — `δα_j += 1` for every node.
///
/// The global-scale direction `δp_j += p_j` is deliberately *absent*: a fractional scale
/// change stretches every baseline (`û_ab·(p_a − p_b) = ‖p_a − p_b‖ ≠ 0`), so scale is
/// observable, not gauged. Returned as [`N_DATUM_GAUGE`] vectors of length
/// `layout.state_dim`; `node_positions` must have `layout.n_nodes` entries.
pub fn datum_gauge_generators(
    layout: &NetworkLayout,
    node_positions: &[[f64; 3]],
) -> Vec<Vec<f64>> {
    debug_assert_eq!(
        node_positions.len(),
        layout.n_nodes,
        "node_positions must have one entry per node"
    );
    let mut gens = Vec::with_capacity(N_DATUM_GAUGE);

    // Three translations: δp_j += ê_k for every node.
    for k in 0..3 {
        let mut g = vec![0.0_f64; layout.state_dim];
        for j in 0..layout.n_nodes {
            g[layout.pos_idx(j) + k] = 1.0;
        }
        gens.push(g);
    }

    // Three rotations about the frame origin: δp_j += ê_k × p_j for every node.
    for k in 0..3 {
        let mut e = [0.0_f64; 3];
        e[k] = 1.0;
        let mut g = vec![0.0_f64; layout.state_dim];
        for (j, &p) in node_positions.iter().enumerate() {
            let c = cross(e, p);
            let base = layout.pos_idx(j);
            g[base] = c[0];
            g[base + 1] = c[1];
            g[base + 2] = c[2];
        }
        gens.push(g);
    }

    // Common clock offset: δτ_j += 1 for every node.
    let mut g_off = vec![0.0_f64; layout.state_dim];
    for j in 0..layout.n_nodes {
        g_off[layout.off_idx(j)] = 1.0;
    }
    gens.push(g_off);

    // Common clock rate: δα_j += 1 for every node.
    let mut g_rate = vec![0.0_f64; layout.state_dim];
    for j in 0..layout.n_nodes {
        g_rate[layout.rate_idx(j)] = 1.0;
    }
    gens.push(g_rate);

    gens
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

    // ── Per-node state-model verification ───────────────────────────────────────────

    /// Body-frame PA positions of the representative per-node network: the five near-side
    /// reflectors plus three relay nodes placed in distinct octants and at different radii
    /// so the complete inter-node graph affinely spans three dimensions (generic global
    /// rigidity ⇒ the rigid-motion defect is exactly six).
    ///
    /// Network: `M = 8` nodes ⇒ `state_dim = 40`.
    fn pernode_body_nodes() -> Vec<Vec3> {
        let mut nodes: Vec<Vec3> = crate::lunar_llr::reflectors()
            .iter()
            .map(|r| r.pa_body_m)
            .collect();
        nodes.push([1_600_000.0, 700_000.0, 900_000.0]);
        nodes.push([-1_200_000.0, 1_000_000.0, -800_000.0]);
        nodes.push([300_000.0, -1_500_000.0, 1_100_000.0]);
        nodes
    }

    /// Real-DE440 Moon-relative inertial node positions at the reference epoch `T0`.
    fn pernode_inertial(nodes: &[Vec3]) -> Vec<[f64; 3]> {
        nodes
            .iter()
            .map(|&b| crate::lunar_orientation::de440_moon_pa_body_to_inertial(b, T0))
            .collect()
    }

    /// Distinct clock-integration windows (seconds); ≥ 2 elapsed baselines separate each
    /// node's offset from its rate, so the temporal defect is exactly two (common offset +
    /// common rate) rather than collapsing offset and rate together.
    const PERNODE_ELAPSED_S: [f64; 3] = [21_600.0, 43_200.0, 86_400.0];

    /// Stack differential one-way range rows over every node pair and every elapsed window.
    /// Pairs = C(8,2) = 28, windows = 3 ⇒ 84 rows (≥ state_dim − 8 = 32 with margin).
    fn pernode_rows(layout: &NetworkLayout, inertial: &[[f64; 3]]) -> Vec<(Vec<f64>, f64)> {
        let m = layout.n_nodes;
        let mut rows = Vec::new();
        for a in 0..m {
            for b in (a + 1)..m {
                let d = [
                    inertial[a][0] - inertial[b][0],
                    inertial[a][1] - inertial[b][1],
                    inertial[a][2] - inertial[b][2],
                ];
                let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                let u = [d[0] / n, d[1] / n, d[2] / n];
                for &elapsed in &PERNODE_ELAPSED_S {
                    rows.push((pernode_range_row(layout, a, b, u, elapsed), 1.0));
                }
            }
        }
        rows
    }

    fn mat_vec(m: &[Vec<f64>], v: &[f64]) -> Vec<f64> {
        m.iter()
            .map(|row| row.iter().zip(v).map(|(&a, &b)| a * b).sum())
            .collect()
    }

    fn vnorm(v: &[f64]) -> f64 {
        v.iter().map(|&x| x * x).sum::<f64>().sqrt()
    }

    fn fro(m: &[Vec<f64>]) -> f64 {
        m.iter()
            .flat_map(|r| r.iter())
            .map(|&x| x * x)
            .sum::<f64>()
            .sqrt()
    }

    /// Each of the eight `datum_gauge_generators` lies in the null space of the per-node
    /// information matrix: the 8-dim datum⊕timescale gauge is genuine in per-node
    /// coordinates (the P4 free-network gauge embedded per node).
    #[test]
    fn pernode_gauge_generators_are_null() {
        let nodes = pernode_body_nodes();
        let layout = NetworkLayout::new(nodes.len());
        let inertial = pernode_inertial(&nodes);
        let rows = pernode_rows(&layout, &inertial);
        let info = assemble_pernode_info(&rows, layout.state_dim);
        let gens = datum_gauge_generators(&layout, &inertial);
        assert_eq!(gens.len(), 8, "eight datum⊕timescale gauge generators");

        let info_fro = fro(&info);
        for (idx, g) in gens.iter().enumerate() {
            let v = mat_vec(&info, g);
            let rel = vnorm(&v) / (info_fro * vnorm(g));
            assert!(
                rel < 1e-8,
                "gauge generator {idx} must lie in N(GᵀWG): relative residual {rel:.3e}"
            );
        }
    }

    /// rank(GᵀWG) = state_dim − 8: the eight-dimensional gauge is the ENTIRE null space,
    /// so range(G) has dimension state_dim − 8 (rich, not rank-1) and the parity space is
    /// non-trivial. A clear spectral gap separates the eight null eigenvalues from the
    /// observable spectrum, so the rank is genuine (not a threshold artifact).
    #[test]
    fn pernode_rank_is_state_dim_minus_eight() {
        let nodes = pernode_body_nodes();
        let layout = NetworkLayout::new(nodes.len());
        let inertial = pernode_inertial(&nodes);
        let rows = pernode_rows(&layout, &inertial);
        let info = assemble_pernode_info(&rows, layout.state_dim);
        assert!(
            rows.len() >= layout.state_dim - 8,
            "network must have ≥ state_dim − 8 rows for the true rank"
        );

        let eig = crate::fim::sym_eig(&info);
        let lam_max = *eig.values.last().unwrap();
        let expected_rank = layout.state_dim - 8;

        // Matched relative threshold; count observable eigenvalues.
        let thr = 1e-9 * lam_max;
        let rank = eig.values.iter().filter(|&&v| v > thr).count();
        assert_eq!(
            rank, expected_rank,
            "rank(GᵀWG) must be state_dim − 8 = {expected_rank}, got {rank}; eigenvalues {:?}",
            eig.values
        );

        // Clear spectral gap: the eight null eigenvalues are numerically zero relative to
        // the spectrum, and the smallest observable eigenvalue is well separated.
        let null_max = eig.values[7];
        let obs_min = eig.values[8];
        assert!(
            null_max < 1e-9 * lam_max,
            "eighth eigenvalue must be numerically null: {null_max:.3e} vs λ_max {lam_max:.3e}"
        );
        assert!(
            obs_min > 1e-6 * lam_max,
            "smallest observable eigenvalue must be well separated: {obs_min:.3e} vs λ_max {lam_max:.3e}"
        );
        let gap = obs_min / null_max.abs().max(f64::MIN_POSITIVE);
        assert!(
            gap > 1e6,
            "spectral gap between null and observable subspaces must be clear: {gap:.3e}"
        );
    }

    /// The global-scale generator (δp_j += p_j for all nodes) is NOT in the null space:
    /// a fractional scale change stretches every baseline, so scale is observable and is
    /// therefore NOT one of the eight gauge generators (consistent with the differential
    /// scale-observability result).
    #[test]
    fn pernode_scale_is_observable() {
        let nodes = pernode_body_nodes();
        let layout = NetworkLayout::new(nodes.len());
        let inertial = pernode_inertial(&nodes);
        let rows = pernode_rows(&layout, &inertial);
        let info = assemble_pernode_info(&rows, layout.state_dim);

        // Global-scale generator: δp_j += p_j (inertial position), clocks untouched.
        let mut scale_gen = vec![0.0_f64; layout.state_dim];
        for (j, &p) in inertial.iter().enumerate() {
            let base = layout.pos_idx(j);
            scale_gen[base] = p[0];
            scale_gen[base + 1] = p[1];
            scale_gen[base + 2] = p[2];
        }

        let v = mat_vec(&info, &scale_gen);
        let rel = vnorm(&v) / (fro(&info) * vnorm(&scale_gen));
        assert!(
            rel > 1e-3,
            "scale must be observable (not gauged): relative residual {rel:.3e} must be bounded away from 0"
        );
    }
}
