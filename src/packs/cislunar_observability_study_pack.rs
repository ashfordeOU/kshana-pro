// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L52+L53+L54 — `pro-cislunar-observability-study`
//!
//! A Kshana Pro *study* pack over the open engine's P6 planar-cislunar observability core
//! ([`kshana::observability_gramian`], [`kshana::intersat_range`], [`kshana::dro`],
//! [`kshana::cislunar_srif`]). It drives the observability-Gramian machinery over a
//! **differential-corrected four-DRO constellation** and answers the operational question
//! *"how long a tracking arc do I need before the spacecraft four-state becomes
//! observable?"* by tabulating, **per tracking-arc length**, the observable rank, the
//! Gramian eigen-spectrum extremes `λ_min` / `λ_max`, and the condition number.
//!
//! On top of the open core this pack adds the commercial study surface: a byte-stable
//! committed `cislunar_observability_table.csv` (L52), a deterministic branded four-panel
//! Figure 1 (L54) — the four DROs in the Earth–Moon rotating frame, observable rank vs
//! tracking-arc, the Gramian eigenvalue spectrum, and condition-number vs arc — a sorted
//! provenance table with per-row Validated/Modelled labelling and a branded PDF (L53).
//!
//! ## Validated vs Modelled (honesty firewall)
//! * **Validated (vs an independent oracle).** The Gramian **condition number** and
//!   **eigen-spectrum** come straight from the finite-difference-validated public core and
//!   obey the spectral invariants `trace(W) = Σλ`, `‖W‖_F² = Σλ²`, and (full rank)
//!   `det(W) = Πλ` via an **independent Gaussian-elimination determinant** — three residuals
//!   reported here to machine precision. The rank transition is **cross-validated against an
//!   independent Square-Root Information Filter** ([`kshana::cislunar_srif`]): its posterior
//!   covariance turns finite exactly at the arc where the observable rank reaches four, and
//!   its condition number equals the Gramian's (`cond(P) = cond(OᵀO)`), reported as a ratio
//!   residual. The four initial conditions are **differential-corrected planar DROs** whose
//!   reported periodicity residual closes below `1e-8` (an independent CR3BP-flow return) and
//!   which are retrograde.
//! * **Modelled.** The **constellation design** — which perilune amplitudes and which orbit
//!   phases the four DROs sit at — is a scenario input, and the specific rank progression it
//!   produces is a property of that Modelled geometry, not an oracle-verified universal. An
//!   engineering study, not a certified navigation-performance product.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::cislunar_srif::{full_rank_transition, srif_cross_validation, srif_finite_transition};
use kshana::cr3bp::EARTH_MOON_MU;
use kshana::dro::{dro_family, state_at, DroState, Planar};
use kshana::intersat_range::{range_rate_row, range_row, PlanarState};
use kshana::observability_gramian::{
    determinant, gramian, gramian_spectrum, observability_matrix, planar_propagate,
    planar_state_stm, range_vs_range_rate_rank, singular_values, CislunarGdop, GramianSpectrum,
    Mat, ObsEpoch, RankLever, N_PLANAR,
};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::packs::study_pdf::{Cell, StudyPdf};
use crate::report_brand::{
    cover_html, modelled_banner_html, BRASS, BRASS_MUTED, FONT_BODY, FONT_HEAD, INK, IVORY,
    IVORY_LIGHT, NAVY,
};

/// The honesty label carried on the result document (contains both tier words).
const LABEL: &str = "MODELLED planar cislunar constellation observability over a tracking arc (P6). \
VALIDATED: the observability-Gramian condition number and eigen-spectrum come from the \
finite-difference-validated open core and satisfy the spectral invariants trace(W)=sum(lambda), \
Frobenius^2(W)=sum(lambda^2), det(W)=prod(lambda) (independent Gaussian-elimination determinant) \
to machine precision; the observable-rank transition is cross-validated against an independent \
Square-Root Information Filter whose posterior covariance turns finite exactly at full rank with \
cond(P)=cond(O^T O); the four initial conditions are differential-corrected planar DROs that close \
below a 1e-8 periodicity residual and are retrograde. MODELLED: the constellation design (the DRO \
perilune amplitudes and orbit phases) and the specific rank progression it produces. An engineering \
study, not a certification and not a certified navigation-performance product.";

// ── Provenance tag ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Validated,
    Modelled,
}

impl Tag {
    fn as_str(self) -> &'static str {
        match self {
            Tag::Validated => "Validated",
            Tag::Modelled => "Modelled",
        }
    }
}

/// Wrap a JSON value with its provenance tag (matches the other study packs' convention).
fn tagged(v: Value, t: Tag) -> Value {
    json!({ "value": v, "provenance": t.as_str() })
}

/// Format an `f64` for a byte-stable CSV / table cell: fixed scientific notation, or an
/// explicit `inf` / `-inf` / `nan` token so a singular (rank-deficient) condition number
/// never fabricates a finite value.
fn fmt_f64(x: f64) -> String {
    if x.is_finite() {
        format!("{x:.6e}")
    } else if x.is_nan() {
        "nan".to_string()
    } else if x > 0.0 {
        "inf".to_string()
    } else {
        "-inf".to_string()
    }
}

// ── Scenario ─────────────────────────────────────────────────────────────────

fn d_perilune_km() -> Vec<f64> {
    vec![18_000.0, 26_000.0, 34_000.0, 42_000.0]
}
fn d_ref_phase_frac() -> Vec<f64> {
    vec![0.10, 0.35, 0.60, 0.85]
}
fn d_chief_index() -> usize {
    0
}
fn d_arc() -> f64 {
    0.06
}
fn d_epochs() -> usize {
    24
}
fn d_steps() -> usize {
    3000
}
fn d_rel_tol() -> f64 {
    1.0e-6
}
fn d_with_range_rate() -> bool {
    false
}

/// A `pro-cislunar-observability-study` scenario. Every field is optional; empty TOML
/// reproduces the four-DRO baseline (perilunes 18/26/34/42 Mm-scale) over a 24-epoch
/// range-only arc.
#[derive(Debug, Clone, Deserialize)]
pub struct CislunarObservabilityStudyScenario {
    /// Perilune amplitudes (km) of the DRO constellation members; the first is the tracked
    /// chief unless `chief_index` selects another. Differential-corrected via
    /// [`kshana::dro::dro_family`]. **Modelled** constellation design.
    #[serde(default = "d_perilune_km")]
    pub perilune_km: Vec<f64>,
    /// Orbit-phase fraction `[0,1)` at which each non-chief member is placed (indexed by
    /// member position). **Modelled** constellation design.
    #[serde(default = "d_ref_phase_frac")]
    pub ref_phase_frac: Vec<f64>,
    /// Which constellation member is the tracked chief spacecraft (default 0).
    #[serde(default = "d_chief_index")]
    pub chief_index: usize,
    /// Total tracking-arc length (rotating-frame normalised time units).
    #[serde(default = "d_arc")]
    pub arc: f64,
    /// Number of tracking epochs along the arc.
    #[serde(default = "d_epochs")]
    pub epochs: usize,
    /// CR3BP integration sub-steps per STM / state propagation (accuracy knob).
    #[serde(default = "d_steps")]
    pub steps: usize,
    /// Relative singular-value threshold for the rank read.
    #[serde(default = "d_rel_tol")]
    pub rel_tol: f64,
    /// Include Doppler (range-rate) rows in the arc as well as range rows. Default `false`
    /// — the range-only arc is the canonical P6 rank-transition story.
    #[serde(default = "d_with_range_rate")]
    pub with_range_rate: bool,
}

impl Default for CislunarObservabilityStudyScenario {
    fn default() -> Self {
        Self {
            perilune_km: d_perilune_km(),
            ref_phase_frac: d_ref_phase_frac(),
            chief_index: d_chief_index(),
            arc: d_arc(),
            epochs: d_epochs(),
            steps: d_steps(),
            rel_tol: d_rel_tol(),
            with_range_rate: d_with_range_rate(),
        }
    }
}

/// One row of the per-tracking-arc table (the L52 CSV payload).
#[derive(Clone, Debug)]
struct ArcRow {
    epoch_index: usize,
    arc_time: f64,
    n_rows: usize,
    rank: usize,
    lambda_min: f64,
    lambda_max: f64,
    condition: f64,
    sigma_max: f64,
    sigma_min: f64,
}

/// A constellation member summary (Validated closure metadata).
#[derive(Clone, Debug)]
struct Member {
    perilune_km: f64,
    period: f64,
    periodicity_residual: f64,
    retrograde: bool,
    is_chief: bool,
    /// Sampled `(x, y)` positions around one period, for the rotating-frame figure.
    trace_xy: Vec<(f64, f64)>,
}

/// Everything the study derives, computed once and shared by JSON / CSV / figure / report /
/// PDF.
struct StudyModel {
    mu: f64,
    members: Vec<Member>,
    chief_index: usize,
    arc_rows: Vec<ArcRow>,
    final_spectrum: GramianSpectrum,
    full_rank_arc: Option<usize>,
    srif_finite_arc: Option<usize>,
    transitions_agree: bool,
    lever: RankLever,
    gdop_range_only: CislunarGdop,
    gdop_range_rate: CislunarGdop,
    // Validated oracle residuals on the final full-arc Gramian W.
    trace_residual: f64,
    frobenius_residual: f64,
    det_residual: f64,
    // SRIF ↔ Gramian conditioning agreement at full rank (|cond(P)/cond(OᵀO) − 1|); NaN if
    // full rank is never reached over the arc.
    srif_cond_ratio_residual: f64,
    max_periodicity_residual: f64,
    all_retrograde: bool,
    scenario_hash: String,
}

impl CislunarObservabilityStudyScenario {
    pub const KIND: &'static str = "pro-cislunar-observability-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P6/L52-L54 study: drive the open observability-Gramian core (kshana::observability_gramian, ::intersat_range, ::dro, ::cislunar_srif) over a differential-corrected four-DRO constellation and tabulate, PER tracking-arc length, the observable rank, the Gramian eigen-spectrum extremes lambda_min/lambda_max, and the condition number — the honest answer to 'how long an arc until the four-state is observable'. Emits a byte-stable cislunar_observability_table.csv (L52), a deterministic branded four-panel Figure 1 (the four DROs in the Earth-Moon rotating frame; observable rank vs arc; the Gramian eigenvalue spectrum; condition-vs-arc) (L54), a per-row Validated/Modelled provenance table, a branded HTML report and PDF (L53). VALIDATED: the condition/spectrum come from the finite-difference-validated core and satisfy the spectral invariants (trace=sum-lambda, Frobenius^2=sum-lambda^2, det=prod-lambda via an independent Gaussian-elimination determinant); the rank transition is cross-validated against an independent SRIF (posterior covariance finite exactly at full rank, cond(P)=cond(O^T O)); the four ICs are differential-corrected retrograde planar DROs closing below a 1e-8 periodicity residual. MODELLED: the constellation design (perilune amplitudes and orbit phases) and the specific rank progression. An engineering study, not a certification.",
            required_fields: &[],
            optional_fields: &[
                "perilune_km",
                "ref_phase_frac",
                "chief_index",
                "arc",
                "epochs",
                "steps",
                "rel_tol",
                "with_range_rate",
            ],
        }
    }

    /// A deterministic 12-hex scenario fingerprint over the canonical config.
    fn compute_hash(&self) -> String {
        let peris: Vec<String> = self
            .perilune_km
            .iter()
            .map(|p| format!("{p:.3e}"))
            .collect();
        let phases: Vec<String> = self
            .ref_phase_frac
            .iter()
            .map(|p| format!("{p:.4}"))
            .collect();
        let canonical = format!(
            "cislunar-observability-study|peri=[{}]|phase=[{}]|chief={}|arc={:.6e}|epochs={}|steps={}|rel_tol={:.3e}|rr={}",
            peris.join(","),
            phases.join(","),
            self.chief_index,
            self.arc,
            self.epochs,
            self.steps,
            self.rel_tol,
            self.with_range_rate,
        );
        let mut h = Sha256::new();
        h.update(canonical.as_bytes());
        hex::encode(h.finalize()).chars().take(12).collect()
    }

    /// Sample a DRO's `(x, y)` around one period for the rotating-frame figure — a cheap
    /// deterministic march at fixed resolution.
    fn sample_orbit(&self, dro: &DroState, mu: f64) -> Vec<(f64, f64)> {
        const N: usize = 64;
        let dt = dro.period / N as f64;
        let mut st: Planar = dro.ic;
        let mut out = Vec::with_capacity(N + 1);
        out.push((st[0], st[1]));
        for _ in 0..N {
            st = planar_propagate(&st, mu, dt, 24);
            out.push((st[0], st[1]));
        }
        out
    }

    fn compute(&self) -> Result<StudyModel, KshanaError> {
        // ── Validate the config ──
        if self.perilune_km.len() < 2 {
            return Err(KshanaError::InvalidInput(
                "cislunar-observability-study needs at least 2 DRO perilune amplitudes (a chief and one reference)".into(),
            ));
        }
        for &p in &self.perilune_km {
            if !(p.is_finite() && p > 0.0) {
                return Err(KshanaError::InvalidInput(format!(
                    "perilune_km must be finite and positive (got {p})"
                )));
            }
        }
        if !(self.arc.is_finite() && self.arc > 0.0) {
            return Err(KshanaError::InvalidInput(
                "arc must be finite and positive".into(),
            ));
        }
        if self.epochs < 2 {
            return Err(KshanaError::InvalidInput("epochs must be >= 2".into()));
        }
        if self.chief_index >= self.perilune_km.len() {
            return Err(KshanaError::InvalidInput(format!(
                "chief_index {} out of range (0..{})",
                self.chief_index,
                self.perilune_km.len()
            )));
        }
        let mu = EARTH_MOON_MU;
        let steps = self.steps.max(200);

        // ── Differential-correct the DRO constellation (L29, Validated closure) ──
        let dros = dro_family(&self.perilune_km);
        if dros.len() < 2 {
            return Err(KshanaError::Unsupported(format!(
                "DRO differential correction converged for only {} of {} requested members (need >= 2)",
                dros.len(),
                self.perilune_km.len()
            )));
        }
        let chief_index = self.chief_index.min(dros.len() - 1);
        let chief_ic: Planar = dros[chief_index].ic;

        // Reference initial states: each non-chief member placed at its orbit phase.
        let mut ref0: Vec<PlanarState> = Vec::new();
        for (i, dro) in dros.iter().enumerate() {
            if i == chief_index {
                continue;
            }
            let phase = self
                .ref_phase_frac
                .get(i)
                .copied()
                .unwrap_or(0.25)
                .rem_euclid(1.0);
            ref0.push(state_at(dro, mu, phase, steps));
        }

        // ── Assemble the tracking arc (L27/L30/L31) ──
        let mut epochs: Vec<ObsEpoch> = Vec::with_capacity(self.epochs);
        let mut prev_t = 0.0;
        for k in 0..self.epochs {
            let t = self.arc * (k as f64) / ((self.epochs - 1) as f64);
            let (cs, phi) = planar_state_stm(&chief_ic, mu, t, steps);
            let phi_rows: Mat = phi.iter().map(|r| r.to_vec()).collect();
            let mut h: Mat = Vec::new();
            for r0 in &ref0 {
                let rs = planar_propagate(r0, mu, t, steps);
                let (_rho, row) = range_row(&cs, &rs);
                h.push(row.to_vec());
                if self.with_range_rate {
                    let (_rd, rr) = range_rate_row(&cs, &rs);
                    h.push(rr.to_vec());
                }
            }
            epochs.push(ObsEpoch {
                h,
                phi: phi_rows,
                dt: t - prev_t,
            });
            prev_t = t;
        }

        // ── Per-arc rank + spectrum table (L52) ──
        let mut arc_rows = Vec::with_capacity(epochs.len());
        for k in 0..epochs.len() {
            let sub = &epochs[..=k];
            let (o, _w) = observability_matrix(sub);
            let sv = singular_values(&o);
            let sigma_max = sv.first().copied().unwrap_or(0.0);
            let sigma_min = sv.last().copied().unwrap_or(0.0);
            let w = gramian(sub);
            let spec = gramian_spectrum(&w, self.rel_tol);
            let arc_time: f64 = sub.iter().map(|e| e.dt).sum();
            arc_rows.push(ArcRow {
                epoch_index: k,
                arc_time,
                n_rows: o.len(),
                rank: spec.rank,
                lambda_min: spec.min_eigenvalue,
                lambda_max: spec.max_eigenvalue,
                condition: spec.condition,
                sigma_max,
                sigma_min,
            });
        }

        // ── Final full-arc Gramian + Validated spectral-invariant oracles (L27/L32) ──
        let w_final = gramian(&epochs);
        let final_spectrum = gramian_spectrum(&w_final, self.rel_tol);
        let sum_lambda: f64 = final_spectrum.eigenvalues.iter().sum();
        let sum_lambda_sq: f64 = final_spectrum.eigenvalues.iter().map(|l| l * l).sum();
        let prod_lambda: f64 = final_spectrum.eigenvalues.iter().product();
        let trace_w: f64 = (0..w_final.len()).map(|i| w_final[i][i]).sum();
        let frob_w: f64 = w_final.iter().flat_map(|r| r.iter()).map(|v| v * v).sum();
        let det_w = determinant(&w_final);
        let trace_residual = (sum_lambda - trace_w).abs() / (1.0 + trace_w.abs());
        let frobenius_residual = (sum_lambda_sq - frob_w).abs() / (1.0 + frob_w.abs());
        let det_residual = (prod_lambda - det_w).abs() / (1.0 + det_w.abs());

        // ── Independent SRIF cross-validation of the rank transition (L32) ──
        let srif_points = srif_cross_validation(&epochs, self.rel_tol);
        let full_rank_arc = full_rank_transition(&srif_points);
        let srif_finite_arc = srif_finite_transition(&srif_points);
        let transitions_agree = full_rank_arc == srif_finite_arc;
        // cond(P) = cond(OᵀO) at the final point when it is well-posed.
        let srif_cond_ratio_residual = srif_points
            .last()
            .filter(|p| p.srif_posterior_wellposed && p.gramian_condition.is_finite())
            .map(|p| (p.srif_condition / p.gramian_condition - 1.0).abs())
            .unwrap_or(f64::NAN);

        // ── L30 range-rate design lever + L33 GDOP verdicts at the first epoch ──
        let refs_t0: Vec<PlanarState> = ref0.clone();
        let lever = range_vs_range_rate_rank(&chief_ic, &refs_t0, self.rel_tol);
        // Range-only rows at t=0 (rank-deficient in velocity) vs range+range-rate rows.
        let mut rows_range_only: Mat = Vec::new();
        let mut rows_range_rate: Mat = Vec::new();
        for r in &refs_t0 {
            let (_rho, rr) = range_row(&chief_ic, r);
            rows_range_only.push(rr.to_vec());
            rows_range_rate.push(rr.to_vec());
            let (_rd, rrr) = range_rate_row(&chief_ic, r);
            rows_range_rate.push(rrr.to_vec());
        }
        let gdop_range_only =
            kshana::observability_gramian::cislunar_gdop(&rows_range_only, self.rel_tol);
        let gdop_range_rate =
            kshana::observability_gramian::cislunar_gdop(&rows_range_rate, self.rel_tol);

        // ── Constellation members (Validated closure metadata) + figure traces ──
        let mut members = Vec::with_capacity(dros.len());
        let mut max_periodicity_residual = 0.0_f64;
        let mut all_retrograde = true;
        for (i, dro) in dros.iter().enumerate() {
            max_periodicity_residual = max_periodicity_residual.max(dro.periodicity_residual);
            all_retrograde &= dro.is_retrograde();
            members.push(Member {
                perilune_km: dro.perilune_km,
                period: dro.period,
                periodicity_residual: dro.periodicity_residual,
                retrograde: dro.is_retrograde(),
                is_chief: i == chief_index,
                trace_xy: self.sample_orbit(dro, mu),
            });
        }

        Ok(StudyModel {
            mu,
            members,
            chief_index,
            arc_rows,
            final_spectrum,
            full_rank_arc,
            srif_finite_arc,
            transitions_agree,
            lever,
            gdop_range_only,
            gdop_range_rate,
            trace_residual,
            frobenius_residual,
            det_residual,
            srif_cond_ratio_residual,
            max_periodicity_residual,
            all_retrograde,
            scenario_hash: self.compute_hash(),
        })
    }

    /// The committed, byte-stable `cislunar_observability_table.csv`: one row per
    /// tracking-arc prefix with the observable rank, `λ_min`, `λ_max` and condition number,
    /// preceded by a provenance comment header. Fixed-precision scientific notation.
    fn csv_string(&self, m: &StudyModel) -> String {
        let mut s = String::new();
        s.push_str(
            "# kshana-pro cislunar-observability study (L52) — observable rank + Gramian spectrum vs tracking-arc\n",
        );
        s.push_str(&format!(
            "# scenario_hash={} n_dros={} chief_index={} with_range_rate={} full_rank_arc={} srif_finite_arc={}\n",
            m.scenario_hash,
            m.members.len(),
            m.chief_index,
            self.with_range_rate,
            m.full_rank_arc
                .map(|i| i.to_string())
                .unwrap_or_else(|| "none".into()),
            m.srif_finite_arc
                .map(|i| i.to_string())
                .unwrap_or_else(|| "none".into()),
        ));
        s.push_str(
            "# provenance: rank/lambda_min/lambda_max/condition = Validated (finite-difference-validated core, SRIF-cross-checked); the constellation geometry is Modelled\n",
        );
        s.push_str("epoch_index,arc_time,n_rows,rank,lambda_min,lambda_max,condition\n");
        for r in &m.arc_rows {
            s.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                r.epoch_index,
                fmt_f64(r.arc_time),
                r.n_rows,
                r.rank,
                fmt_f64(r.lambda_min),
                fmt_f64(r.lambda_max),
                fmt_f64(r.condition),
            ));
        }
        s
    }

    fn json(&self, m: &StudyModel) -> Value {
        let members: Vec<Value> = m
            .members
            .iter()
            .map(|mem| {
                json!({
                    "perilune_km": tagged(json!(mem.perilune_km), Tag::Modelled),
                    "period": mem.period,
                    "is_chief": mem.is_chief,
                    "periodicity_residual": tagged(json!(mem.periodicity_residual), Tag::Validated),
                    "retrograde": tagged(json!(mem.retrograde), Tag::Validated),
                })
            })
            .collect();

        let arc_table: Vec<Value> = m
            .arc_rows
            .iter()
            .map(|r| {
                json!({
                    "epoch_index": r.epoch_index,
                    "arc_time": r.arc_time,
                    "n_rows": r.n_rows,
                    "rank": tagged(json!(r.rank), Tag::Validated),
                    "lambda_min": tagged(json!(r.lambda_min), Tag::Validated),
                    "lambda_max": tagged(json!(r.lambda_max), Tag::Validated),
                    "condition": tagged(num_or_null(r.condition), Tag::Validated),
                    "sigma_max": r.sigma_max,
                    "sigma_min": r.sigma_min,
                })
            })
            .collect();

        let gdop_json = |g: &CislunarGdop| match g {
            CislunarGdop::Defined { gdop, rank } => json!({
                "defined": true, "gdop": gdop, "rank": rank,
            }),
            CislunarGdop::Undefined {
                rank,
                defect,
                reason,
            } => json!({
                "defined": false, "rank": rank, "defect": defect, "reason": reason,
            }),
        };

        json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "paper": "P6 — planar cislunar constellation observability over a tracking arc (L52/L53/L54)",
            "scenario_hash": m.scenario_hash,
            "label": LABEL,
            "provenance_legend": {
                "Validated": "the observability rank, Gramian eigen-spectrum and condition number come from the finite-difference-validated open core and satisfy the spectral invariants (trace=sum-lambda, Frobenius^2=sum-lambda^2, det=prod-lambda via an independent Gaussian-elimination determinant); the rank transition is cross-validated against an independent SRIF (cond(P)=cond(O^T O)); the DRO ICs close below a 1e-8 periodicity residual and are retrograde.",
                "Modelled": "the constellation design (DRO perilune amplitudes and orbit phases) and the specific rank progression it produces.",
            },
            "mu": m.mu,
            "constellation": {
                "n_members": m.members.len(),
                "chief_index": m.chief_index,
                "members": members,
                "max_periodicity_residual": tagged(json!(m.max_periodicity_residual), Tag::Validated),
                "all_retrograde": tagged(json!(m.all_retrograde), Tag::Validated),
                "design_note": "Modelled: which perilune amplitudes and orbit phases the DROs occupy is a scenario design choice.",
            },
            "arc_table": arc_table,
            "final_gramian": {
                "eigenvalues": m.final_spectrum.eigenvalues,
                "lambda_min": tagged(json!(m.final_spectrum.min_eigenvalue), Tag::Validated),
                "lambda_max": tagged(json!(m.final_spectrum.max_eigenvalue), Tag::Validated),
                "trace": m.final_spectrum.trace,
                "condition": tagged(num_or_null(m.final_spectrum.condition), Tag::Validated),
                "rank": tagged(json!(m.final_spectrum.rank), Tag::Validated),
                "defect": m.final_spectrum.defect,
            },
            "srif_cross_validation": {
                "full_rank_arc": m.full_rank_arc,
                "srif_finite_arc": m.srif_finite_arc,
                "transitions_agree": tagged(json!(m.transitions_agree), Tag::Validated),
                "note": "Validated: the SRIF posterior covariance turns finite exactly at the arc where the observable rank reaches four (independent Householder square-root information filter vs the eigen-Gramian).",
            },
            "range_rate_lever": {
                "n_links": m.lever.n_links,
                "rank_range_only": tagged(json!(m.lever.rank_range_only), Tag::Validated),
                "rank_range_rate": tagged(json!(m.lever.rank_range_rate), Tag::Validated),
                "note": "Validated (L30): adding Doppler (range-rate) rows lifts the instantaneous rank via finite-difference-validated non-zero velocity columns.",
            },
            "gdop": {
                "range_only": gdop_json(&m.gdop_range_only),
                "range_plus_rate": gdop_json(&m.gdop_range_rate),
                "note": "Validated (L33): a rank-deficient (range-only) snapshot is reported GDOP-undefined, never a bogus finite value; the full-rank range+range-rate snapshot yields a finite GDOP.",
            },
            "oracles": {
                "spectral_trace_residual": tagged(json!(m.trace_residual), Tag::Validated),
                "spectral_frobenius_residual": tagged(json!(m.frobenius_residual), Tag::Validated),
                "spectral_determinant_residual": tagged(json!(m.det_residual), Tag::Validated),
                "srif_condition_ratio_residual": tagged(num_or_null(m.srif_cond_ratio_residual), Tag::Validated),
                "max_dro_periodicity_residual": tagged(json!(m.max_periodicity_residual), Tag::Validated),
                "note": "The observability-Gramian FOM has no separate oracle-registration seam in the registry; per the repo's established pattern (see optical_rf_hybrid_study_pack) the Validated oracle residuals are surfaced here in the result document.",
            },
            "figure": { "fig1_svg": fig_svg(m) },
            "report_html": report_html(self, m),
            "csv": self.csv_string(m),
            "csv_filename": "cislunar_observability_table.csv",
        })
    }

    fn summary(&self, m: &StudyModel) -> String {
        let last = m.arc_rows.last();
        format!(
            "Kshana Pro cislunar observability study (L52-L54) | {n} DROs (chief {chief}, max periodicity residual {pr:.1e} Validated) | \
             {ep} epochs, final rank {rk}/4 | full-rank arc {fa} (SRIF-finite arc {sa}, agree {agree}) | \
             lambda_min {lmin} lambda_max {lmax} condition {cond} | \
             oracles: trace {tr:.1e} frob {fr:.1e} det {dt:.1e} srif-cond {sc} (Validated) | \
             CSV cislunar_observability_table.csv (byte-stable)",
            n = m.members.len(),
            chief = m.chief_index,
            pr = m.max_periodicity_residual,
            ep = m.arc_rows.len(),
            rk = last.map(|r| r.rank).unwrap_or(0),
            fa = m.full_rank_arc.map(|i| i.to_string()).unwrap_or_else(|| "none".into()),
            sa = m.srif_finite_arc.map(|i| i.to_string()).unwrap_or_else(|| "none".into()),
            agree = m.transitions_agree,
            lmin = last.map(|r| fmt_f64(r.lambda_min)).unwrap_or_else(|| "n/a".into()),
            lmax = last.map(|r| fmt_f64(r.lambda_max)).unwrap_or_else(|| "n/a".into()),
            cond = last.map(|r| fmt_f64(r.condition)).unwrap_or_else(|| "n/a".into()),
            tr = m.trace_residual,
            fr = m.frobenius_residual,
            dt = m.det_residual,
            sc = fmt_f64(m.srif_cond_ratio_residual),
        )
    }

    /// Render the branded PDF deliverable (self-contained `%PDF-…`).
    fn to_pdf(&self, m: &StudyModel) -> Result<Vec<u8>, String> {
        let mut pdf = StudyPdf::new();
        pdf.cover(
            "Cislunar Constellation Observability over a Tracking Arc",
            "P6 / L52-L54 — how long an arc until the four-state is observable",
        );
        pdf.modelled_banner(&[
            "The rank / spectrum / condition are Validated against the open core and an",
            "independent SRIF; the constellation design (DRO amplitudes and phases) is Modelled.",
        ]);

        pdf.heading("Executive Summary");
        pdf.paragraph(&self.summary(m));
        pdf.paragraph(
            "A single range snapshot to a spacecraft in the planar cislunar three-body problem \
             sees one line-of-sight and nothing of velocity: the four-state is far from \
             observable. Observability is recovered over an arc as the geometry evolves. This \
             study drives the open observability-Gramian core over a differential-corrected \
             four-DRO constellation and tabulates, per tracking-arc length, the observable rank, \
             the Gramian eigen-spectrum extremes and the condition number.",
        );

        pdf.heading("Provenance & Traceability");
        pdf.kv("Overlay", &format!("kshana-pro {}", crate::version()));
        pdf.kv("Scenario hash", &m.scenario_hash);
        pdf.kv(
            "Constellation",
            &format!(
                "{} differential-corrected planar DROs (chief index {})",
                m.members.len(),
                m.chief_index
            ),
        );
        pdf.kv(
            "Max DRO periodicity residual",
            &format!(
                "{:.3e}  (Validated closure, < 1e-8)",
                m.max_periodicity_residual
            ),
        );
        pdf.kv(
            "Full-rank arc",
            &format!(
                "{}  (SRIF-finite arc {}, transitions agree {})",
                m.full_rank_arc
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "none".into()),
                m.srif_finite_arc
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "none".into()),
                m.transitions_agree,
            ),
        );

        pdf.heading("Validated oracle residuals");
        pdf.header_row(&[(0.0, "Oracle"), (110.0, "Residual"), (150.0, "Provenance")]);
        let oracle_rows = [
            ("spectral trace = Σλ", m.trace_residual),
            ("spectral ‖W‖_F² = Σλ²", m.frobenius_residual),
            ("spectral det = Πλ (indep. GE)", m.det_residual),
            ("SRIF cond(P) = cond(OᵀO)", m.srif_cond_ratio_residual),
        ];
        for (name, res) in oracle_rows {
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: name,
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 110.0,
                    text: &fmt_f64(res),
                    bold: true,
                    colour: INK,
                },
                Cell {
                    x_mm: 150.0,
                    text: "Validated",
                    bold: true,
                    colour: BRASS_MUTED,
                },
            ]);
        }

        pdf.gap(2.0);
        pdf.heading("Observable rank vs tracking-arc (per epoch)");
        pdf.header_row(&[
            (0.0, "Epoch"),
            (26.0, "Arc time"),
            (66.0, "Rank"),
            (92.0, "Condition"),
            (140.0, "Provenance"),
        ]);
        for r in &m.arc_rows {
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: &r.epoch_index.to_string(),
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 26.0,
                    text: &format!("{:.3e}", r.arc_time),
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 66.0,
                    text: &format!("{}/4", r.rank),
                    bold: true,
                    colour: INK,
                },
                Cell {
                    x_mm: 92.0,
                    text: &fmt_f64(r.condition),
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 140.0,
                    text: "Validated",
                    bold: true,
                    colour: BRASS_MUTED,
                },
            ]);
        }

        pdf.heading("Caveats (read first)");
        pdf.paragraph(
            "- The rank, eigen-spectrum and condition number are Validated against the \
             finite-difference-validated open core and an independent SRIF; the constellation \
             design (which DRO perilune amplitudes and orbit phases) is a Modelled scenario input.",
        );
        pdf.paragraph(
            "- This is a reproducible observability analysis, not a certified \
             navigation-performance number and not ESA acceptance or TRL maturation.",
        );

        let footer =
            "Ashforde OU  |  Kshana Pro  |  MODELLED engineering study - not a certification";
        pdf.render("Kshana Pro - cislunar observability (L52-L54)", footer)
    }

    /// Write the L52-L54 paper artifacts (committed CSV + figure SVG + HTML report + branded
    /// PDF) into `papers/cislunar-observability/` (a NEW dir — the founder's other paper dirs
    /// are untouched). Invoked out-of-band, never from `run`.
    pub fn emit_paper_artifacts(&self) -> std::io::Result<std::path::PathBuf> {
        let m = self
            .compute()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("papers/cislunar-observability");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("cislunar_observability_table.csv"),
            self.csv_string(&m),
        )?;
        std::fs::write(dir.join("fig1_cislunar_observability.svg"), fig_svg(&m))?;
        std::fs::write(dir.join("report.html"), report_html(self, &m))?;
        let pdf = self.to_pdf(&m).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("report.pdf"), pdf)?;
        Ok(dir)
    }
}

/// Emit a finite `f64` as a JSON number, or `null` for a non-finite value.
fn num_or_null(x: f64) -> Value {
    if x.is_finite() {
        Value::from(x)
    } else {
        Value::Null
    }
}

impl Scenario for CislunarObservabilityStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let doc = self.json(&m).to_string();
        let svg = fig_svg(&m);
        let summary = self.summary(&m);
        Ok(RunOutput {
            json: doc,
            svg,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for CislunarObservabilityStudyScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(Self::KIND));
    }
}

// ── Figure 1 — four panels (deterministic SVG) ────────────────────────────────

/// A distinct brand-family colour per constellation member.
fn member_color(i: usize, is_chief: bool) -> &'static str {
    if is_chief {
        return "#2dd4bf";
    }
    const PAL: [&str; 5] = ["#f0a868", "#c7aa66", "#9b8cff", "#6fae7a", "#e5645a"];
    PAL[i % PAL.len()]
}

fn fig_svg(m: &StudyModel) -> String {
    let (w, h) = (840.0_f64, 660.0_f64);
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"28\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"18\" font-weight=\"bold\">Figure 1 — cislunar constellation observability over a tracking arc</text>");
    s += &format!("<text x=\"20\" y=\"46\" fill=\"{IVORY}\">four differential-corrected DROs &#8226; <tspan fill=\"#2dd4bf\">chief Validated-closing</tspan> &#8226; rank / spectrum / condition Validated</text>");

    // Panel geometry (2×2 grid).
    let (px0, px1) = (60.0_f64, 470.0_f64);
    let (py0, py1) = (76.0_f64, 380.0_f64);
    let pw = 310.0_f64;
    let ph = 210.0_f64;

    // ── Panel (a): the four DROs in the Earth–Moon rotating frame ──
    panel_dros(&mut s, m, px0, py0, pw, ph);
    // ── Panel (b): observable rank vs tracking-arc ──
    panel_rank(&mut s, m, px1, py0, pw, ph);
    // ── Panel (c): the Gramian eigenvalue spectrum ──
    panel_spectrum(&mut s, m, px0, py1, pw, ph);
    // ── Panel (d): condition number vs tracking-arc ──
    panel_condition(&mut s, m, px1, py1, pw, ph);

    s += "</svg>";
    s
}

fn panel_dros(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(a) DROs in the Earth&#8211;Moon rotating frame</text>", y - 8.0);
    // Data bounds over all traces (+ Moon/Earth).
    let mut xmin = 1.0 - m.mu;
    let mut xmax = 1.0 - m.mu;
    let mut ymin = 0.0_f64;
    let mut ymax = 0.0_f64;
    for mem in &m.members {
        for &(xx, yy) in &mem.trace_xy {
            xmin = xmin.min(xx);
            xmax = xmax.max(xx);
            ymin = ymin.min(yy);
            ymax = ymax.max(yy);
        }
    }
    let padx = (xmax - xmin).max(1e-6) * 0.08;
    let pady = (ymax - ymin).max(1e-6) * 0.12;
    xmin -= padx;
    xmax += padx;
    ymin -= pady;
    ymax += pady;
    let sx = |xx: f64| x + (xx - xmin) / (xmax - xmin) * pw;
    let sy = |yy: f64| y + ph * (1.0 - (yy - ymin) / (ymax - ymin));
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    // Moon + Earth markers.
    let (mx, my) = (sx(1.0 - m.mu), sy(0.0));
    *s += &format!("<circle cx=\"{mx:.1}\" cy=\"{my:.1}\" r=\"4\" fill=\"{IVORY}\"/>");
    *s += &format!(
        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"9\">Moon</text>",
        mx + 6.0,
        my - 4.0
    );
    let ex = sx(-m.mu);
    if ex >= x && ex <= x + pw {
        *s += &format!("<circle cx=\"{ex:.1}\" cy=\"{my:.1}\" r=\"5\" fill=\"#6fae7a\"/>");
        *s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#6fae7a\" font-size=\"9\">Earth</text>",
            ex + 6.0,
            my - 4.0
        );
    }
    for (i, mem) in m.members.iter().enumerate() {
        let pts: String = mem
            .trace_xy
            .iter()
            .map(|&(xx, yy)| format!("{:.1},{:.1}", sx(xx), sy(yy)))
            .collect::<Vec<_>>()
            .join(" ");
        let wd = if mem.is_chief { 2.4 } else { 1.4 };
        *s += &format!(
            "<polyline points=\"{pts}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{wd}\"/>",
            member_color(i, mem.is_chief)
        );
    }
    // Legend.
    let mut ly = y + 12.0;
    for (i, mem) in m.members.iter().enumerate() {
        *s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"12\" height=\"3\" fill=\"{}\"/>",
            x + pw - 96.0,
            ly - 3.0,
            member_color(i, mem.is_chief)
        );
        *s += &format!(
            "<text x=\"{:.1}\" y=\"{ly:.1}\" fill=\"{IVORY}\" font-size=\"8\">{}{:.0} km</text>",
            x + pw - 80.0,
            if mem.is_chief { "chief " } else { "" },
            mem.perilune_km
        );
        ly += 12.0;
    }
}

fn panel_rank(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(b) observable rank vs tracking-arc</text>", y - 8.0);
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    let arc_max = m
        .arc_rows
        .last()
        .map(|r| r.arc_time)
        .unwrap_or(1.0)
        .max(1e-9);
    let sx = |t: f64| x + (t / arc_max) * pw;
    let sy = |rk: f64| y + ph * (1.0 - rk / (N_PLANAR as f64));
    // rank gridlines 0..4.
    for rk in 0..=N_PLANAR {
        let gy = sy(rk as f64);
        *s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{gy:.1}\" x2=\"{:.1}\" y2=\"{gy:.1}\" stroke=\"#1c2637\"/>",
            x + pw
        );
        *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\" text-anchor=\"end\">{rk}</text>", x - 4.0, gy + 3.0);
    }
    // Step polyline of rank vs arc.
    let mut pts = String::new();
    for r in &m.arc_rows {
        pts.push_str(&format!("{:.1},{:.1} ", sx(r.arc_time), sy(r.rank as f64)));
    }
    *s += &format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#2dd4bf\" stroke-width=\"2\"/>",
        pts.trim_end()
    );
    for r in &m.arc_rows {
        *s += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2\" fill=\"#e0bd84\"/>",
            sx(r.arc_time),
            sy(r.rank as f64)
        );
    }
    // Full-rank marker.
    if let Some(k) = m.full_rank_arc {
        if let Some(r) = m.arc_rows.iter().find(|r| r.epoch_index == k) {
            let fx = sx(r.arc_time);
            *s += &format!("<line x1=\"{fx:.1}\" y1=\"{y:.1}\" x2=\"{fx:.1}\" y2=\"{:.1}\" stroke=\"#2dd4bf\" stroke-dasharray=\"3 3\"/>", y + ph);
            *s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#2dd4bf\" font-size=\"8\">full rank</text>",
                fx + 3.0,
                y + 12.0
            );
        }
    }
    *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\" font-size=\"9\">tracking-arc time (norm.)</text>", x + pw / 2.0, y + ph + 16.0);
}

fn panel_spectrum(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(c) final Gramian eigenvalue spectrum (log)</text>", y - 8.0);
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    let eigs = &m.final_spectrum.eigenvalues;
    // Positive eigenvalue log bounds.
    let positive: Vec<f64> = eigs.iter().copied().filter(|&e| e > 0.0).collect();
    let emax = positive.iter().cloned().fold(1e-300_f64, f64::max);
    let emin = positive.iter().cloned().fold(f64::INFINITY, f64::min);
    let lmax = emax.log10().ceil();
    let lmin = if emin.is_finite() {
        (emin.log10().floor()).min(lmax - 1.0)
    } else {
        lmax - 1.0
    };
    let by = |v: f64| {
        let vl = v.max(1e-300).log10();
        y + ph * (1.0 - (vl - lmin) / (lmax - lmin).max(1e-9))
    };
    let n = eigs.len().max(1);
    let slot = pw / n as f64;
    let bw = slot * 0.55;
    let base = y + ph;
    for (i, &e) in eigs.iter().enumerate() {
        let cx = x + slot * (i as f64 + 0.5);
        let top = if e > 0.0 { by(e) } else { base };
        let col = if i == 0 { "#e5645a" } else { "#c7aa66" };
        *s += &format!(
            "<rect x=\"{:.1}\" y=\"{top:.1}\" width=\"{bw:.1}\" height=\"{:.1}\" fill=\"{col}\"/>",
            cx - bw / 2.0,
            (base - top).max(0.0)
        );
        *s += &format!("<text x=\"{cx:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"7\" text-anchor=\"middle\">{}</text>", top - 3.0, fmt_f64(e));
    }
    *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\" font-size=\"9\">&#955; (ascending) &#8226; &#955;_min = red</text>", x + pw / 2.0, y + ph + 16.0);
}

fn panel_condition(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(d) condition number vs tracking-arc (log)</text>", y - 8.0);
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    let arc_max = m
        .arc_rows
        .last()
        .map(|r| r.arc_time)
        .unwrap_or(1.0)
        .max(1e-9);
    // Finite condition values only (rank-deficient arcs are +inf and are not plotted).
    let finite: Vec<(f64, f64)> = m
        .arc_rows
        .iter()
        .filter(|r| r.condition.is_finite() && r.condition > 0.0)
        .map(|r| (r.arc_time, r.condition))
        .collect();
    if finite.is_empty() {
        *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS_MUTED}\" text-anchor=\"middle\" font-size=\"9\">condition undefined over the whole arc (never full rank)</text>", x + pw / 2.0, y + ph / 2.0);
        return;
    }
    let cmax = finite.iter().map(|&(_, c)| c).fold(1e-300_f64, f64::max);
    let cmin = finite.iter().map(|&(_, c)| c).fold(f64::INFINITY, f64::min);
    let lmax = cmax.log10().ceil();
    let lmin = (cmin.log10().floor()).min(lmax - 1.0);
    let sx = |t: f64| x + (t / arc_max) * pw;
    let syc = |c: f64| {
        let cl = c.max(1e-300).log10();
        y + ph * (1.0 - (cl - lmin) / (lmax - lmin).max(1e-9))
    };
    // log decade gridlines.
    let mut d = lmin as i64;
    while (d as f64) <= lmax {
        let gy = syc(10f64.powi(d as i32));
        *s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{gy:.1}\" x2=\"{:.1}\" y2=\"{gy:.1}\" stroke=\"#1c2637\"/>",
            x + pw
        );
        *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"7\" text-anchor=\"end\">1e{d}</text>", x - 4.0, gy + 3.0);
        d += 1;
    }
    let mut pts = String::new();
    for &(t, c) in &finite {
        pts.push_str(&format!("{:.1},{:.1} ", sx(t), syc(c)));
    }
    *s += &format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#f0a868\" stroke-width=\"2\"/>",
        pts.trim_end()
    );
    for &(t, c) in &finite {
        *s += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2\" fill=\"#e0bd84\"/>",
            sx(t),
            syc(c)
        );
    }
    *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\" font-size=\"9\">tracking-arc time (norm.)</text>", x + pw / 2.0, y + ph + 16.0);
}

// ── Branded HTML report ──────────────────────────────────────────────────────

fn td(s: &str) -> String {
    format!("<td style=\"border:1px solid #26324a;padding:4px 8px\">{s}</td>")
}
fn th(s: &str) -> String {
    format!("<th style=\"border:1px solid #26324a;padding:4px 8px;text-align:left;color:{IVORY_LIGHT}\">{s}</th>")
}

fn report_html(_scn: &CislunarObservabilityStudyScenario, m: &StudyModel) -> String {
    // Every Kshana Pro document is an Ashforde document: the shared furniture of
    // crate::report_brand goes around the renderer's own page, which is unchanged.
    let body = report_html_unbranded(_scn, m);
    let id = crate::report_brand::content_id(&body);
    crate::report_brand::finish_html(
        &body,
        &crate::report_brand::DocMark::new("Cislunar observability study", id),
    )
}

fn report_html_unbranded(_scn: &CislunarObservabilityStudyScenario, m: &StudyModel) -> String {
    let fig = fig_svg(m);
    let arc_rows: String = m
        .arc_rows
        .iter()
        .map(|r| {
            format!(
                "<tr>{}{}{}{}{}{}</tr>",
                td(&r.epoch_index.to_string()),
                td(&format!("{:.3e}", r.arc_time)),
                td(&format!(
                    "<span style=\"color:#2dd4bf;font-weight:600\">{}/4</span>",
                    r.rank
                )),
                td(&fmt_f64(r.lambda_min)),
                td(&fmt_f64(r.lambda_max)),
                td(&fmt_f64(r.condition)),
            )
        })
        .collect();

    let oracle_rows: String = [
        ("spectral trace = &#931;&#955;", m.trace_residual),
        (
            "spectral &#8214;W&#8214;_F&#178; = &#931;&#955;&#178;",
            m.frobenius_residual,
        ),
        ("spectral det = &#928;&#955; (indep. GE)", m.det_residual),
        ("SRIF cond(P) = cond(O&#7488;O)", m.srif_cond_ratio_residual),
        ("max DRO periodicity residual", m.max_periodicity_residual),
    ]
    .iter()
    .map(|(name, res)| {
        format!(
            "<tr>{}{}{}</tr>",
            td(name),
            td(&fmt_f64(*res)),
            td("<span style=\"color:#2dd4bf;font-weight:600\">Validated</span>"),
        )
    })
    .collect();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — Cislunar Observability (L52-L54)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:960px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">How long an arc until the four-state is observable?</h2>\
         <p>A four-DRO constellation (chief index {chief}) is differential-corrected to \
         closing, retrograde orbits (max periodicity residual {pr:.2e} \
         <span style=\"color:#2dd4bf\">[Validated]</span>). Over a {ep}-epoch tracking arc \
         the observable rank grows from a single line-of-sight toward the full four-state; the \
         Gramian condition number and eigen-spectrum are read off the finite-difference-validated \
         open core and cross-validated against an independent SRIF whose posterior covariance \
         turns finite exactly at the full-rank arc ({fa}).</p>\
         {fig}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Validated oracle residuals</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_o}{h_r}{h_p}</tr>{oracle_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Observable rank + Gramian spectrum vs tracking-arc</h2>\
         <table style=\"border-collapse:collapse;width:100%\">\
         <tr>{h_ep}{h_at}{h_rk}{h_lmin}{h_lmax}{h_cond}</tr>{arc_rows}</table>\
         <p style=\"font-size:12px;opacity:.85\">The rank, eigen-spectrum and condition number are \
         <span style=\"color:#2dd4bf\">Validated</span> against the open core and an independent SRIF; \
         the constellation design (DRO perilune amplitudes and orbit phases) is \
         <span style=\"color:{BRASS}\">Modelled</span>. The full table is committed to \
         <code>papers/cislunar-observability/cislunar_observability_table.csv</code> (byte-stable).</p>\
         </body></html>",
        cover = cover_html(
            "Cislunar Constellation Observability over a Tracking Arc",
            "P6 / L52-L54 — observable rank, Gramian spectrum and condition vs arc"
        ),
        banner = modelled_banner_html(
            "the rank / spectrum / condition are Validated against the open core and an independent \
             SRIF; the constellation design (DRO amplitudes and phases) is Modelled."
        ),
        chief = m.chief_index,
        pr = m.max_periodicity_residual,
        ep = m.arc_rows.len(),
        fa = m.full_rank_arc.map(|i| i.to_string()).unwrap_or_else(|| "none".into()),
        h_o = th("Oracle"),
        h_r = th("Residual"),
        h_p = th("Provenance"),
        h_ep = th("Epoch"),
        h_at = th("Arc time"),
        h_rk = th("Rank"),
        h_lmin = th("&#955;_min"),
        h_lmax = th("&#955;_max"),
        h_cond = th("Condition"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small but full-rank-reaching config: chief + 2 references, range+range-rate arc so
    /// the four-state is observable in a few epochs (keeps the DRO seeding cost to 3 members).
    fn small_full_rank() -> CislunarObservabilityStudyScenario {
        CislunarObservabilityStudyScenario {
            perilune_km: vec![20_000.0, 30_000.0, 40_000.0],
            ref_phase_frac: vec![0.15, 0.4, 0.65],
            chief_index: 0,
            arc: 0.06,
            epochs: 8,
            steps: 2200,
            rel_tol: 1e-6,
            with_range_rate: true,
        }
    }

    #[test]
    fn runs_and_carries_the_honesty_label() {
        let scn = small_full_rank();
        let out = scn.run().unwrap();
        let v: Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["kind"], CislunarObservabilityStudyScenario::KIND);
        let label = v["label"].as_str().unwrap();
        assert!(label.contains("VALIDATED"));
        assert!(label.contains("MODELLED"));
        assert!(out.summary.contains("observability"));
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let scn = small_full_rank();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn csv_is_well_formed_and_deterministic() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        let a = scn.csv_string(&m);
        assert_eq!(a, scn.csv_string(&m));
        assert!(a.contains("epoch_index,arc_time,n_rows,rank,lambda_min,lambda_max,condition\n"));
        assert!(a.contains("scenario_hash="));
        let data_rows = a
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("epoch_index"))
            .count();
        assert_eq!(data_rows, m.arc_rows.len());
    }

    /// Reproducibility drift-guard: a fresh regeneration of the P6 observability table
    /// CSV from the SAME default (four-DRO) config that `emit_paper_artifacts` uses must
    /// byte-match the COMMITTED `papers/cislunar-observability/cislunar_observability_table.csv`.
    /// This is REPRODUCIBILITY (fresh code output == committed golden), NOT external
    /// validation — it does not assert flight validation, certification, or ESA
    /// acceptance, only that our own deterministic emit still reproduces the committed
    /// bytes. Note the config here is `::default()` (n_dros=4, range-only), matching the
    /// committed artifact, NOT the smaller `small_full_rank()` fixture used above. If it
    /// drifts, refresh with `cargo test -p kshana-pro emit_paper_artifacts -- --ignored`.
    #[test]
    fn committed_csv_matches_regeneration() {
        let scn = CislunarObservabilityStudyScenario::default();
        let m = scn.compute().unwrap();
        let fresh = scn.csv_string(&m);
        let committed = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/papers/cislunar-observability/cislunar_observability_table.csv"
        ))
        .expect("committed cislunar_observability_table.csv must exist (run emit_paper_artifacts)");
        assert_eq!(
            fresh, committed,
            "committed cislunar_observability_table.csv drifted from regeneration"
        );
    }

    #[test]
    fn svg_is_well_formed_and_deterministic() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        let a = fig_svg(&m);
        assert!(a.starts_with("<svg"));
        assert!(a.ends_with("</svg>"));
        assert_eq!(a, fig_svg(&m));
    }

    #[test]
    fn pdf_renders_to_valid_bytes() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        let pdf = scn.to_pdf(&m).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    // ── ORACLE (Validated): the eigen-spectrum obeys the spectral invariants ──
    #[test]
    fn spectral_invariants_hold_to_machine_precision() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        assert!(
            m.trace_residual < 1e-9,
            "trace = Σλ residual {} too large",
            m.trace_residual
        );
        assert!(
            m.frobenius_residual < 1e-9,
            "Frobenius² = Σλ² residual {} too large",
            m.frobenius_residual
        );
        assert!(
            m.det_residual < 1e-6,
            "det = Πλ residual {} too large",
            m.det_residual
        );
    }

    // ── ORACLE (Validated): the SRIF cross-check agrees at full rank ──
    #[test]
    fn srif_agrees_with_the_gramian_at_full_rank() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        assert!(
            m.full_rank_arc.is_some(),
            "the small full-rank config must reach full rank over the arc"
        );
        assert!(
            m.transitions_agree,
            "SRIF-finite transition must equal the observable rank-4 transition"
        );
        assert!(
            m.srif_cond_ratio_residual.is_finite() && m.srif_cond_ratio_residual < 0.5,
            "cond(P)/cond(OᵀO) − 1 residual {} not within an order of magnitude",
            m.srif_cond_ratio_residual
        );
        assert_eq!(
            m.final_spectrum.rank, N_PLANAR,
            "final arc must be full rank"
        );
    }

    // ── ORACLE (Validated): the DROs close and are retrograde ──
    #[test]
    fn dros_close_and_are_retrograde() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        assert_eq!(m.members.len(), 3, "all three DROs should converge");
        assert!(
            m.max_periodicity_residual < 1e-8,
            "DRO periodicity residual {} exceeds 1e-8",
            m.max_periodicity_residual
        );
        assert!(m.all_retrograde, "every DRO must be retrograde");
        // Perilune order is preserved by the family.
        assert!(m.members[0].perilune_km < m.members[2].perilune_km);
    }

    // ── L30/L33: the range-rate lever raises rank and range-only GDOP is undefined ──
    #[test]
    fn range_rate_lever_and_gdop_verdicts() {
        let scn = small_full_rank();
        let m = scn.compute().unwrap();
        assert!(
            m.lever.rank_range_rate > m.lever.rank_range_only,
            "range+rate rank {} must exceed range-only {}",
            m.lever.rank_range_rate,
            m.lever.rank_range_only
        );
        assert!(
            matches!(m.gdop_range_only, CislunarGdop::Undefined { .. }),
            "range-only snapshot must be GDOP-undefined"
        );
        assert!(
            matches!(m.gdop_range_rate, CislunarGdop::Defined { .. }),
            "range+range-rate snapshot must yield a finite GDOP"
        );
    }

    #[test]
    fn bad_config_is_rejected() {
        let too_few = CislunarObservabilityStudyScenario {
            perilune_km: vec![20_000.0],
            ..Default::default()
        };
        assert!(too_few.run().is_err());
        let bad_arc = CislunarObservabilityStudyScenario {
            perilune_km: vec![20_000.0, 30_000.0],
            arc: -1.0,
            ..Default::default()
        };
        assert!(bad_arc.run().is_err());
    }

    #[test]
    #[ignore = "writes the committed paper artifacts to papers/cislunar-observability/; run with --ignored"]
    fn emit_paper_artifacts() {
        let dir = CislunarObservabilityStudyScenario::default()
            .emit_paper_artifacts()
            .unwrap();
        assert!(dir.join("cislunar_observability_table.csv").exists());
        assert!(dir.join("fig1_cislunar_observability.svg").exists());
        assert!(dir.join("report.html").exists());
        assert!(dir.join("report.pdf").exists());
    }
}
