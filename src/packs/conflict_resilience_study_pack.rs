// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # L55 — `pro-conflict-resilience-study`
//!
//! A Kshana Pro *study* pack over the open engine's P7 layered-PNT conflict-resilience core
//! ([`kshana::conflict_resilience`], [`kshana::conflict_threat_params`]). It drives the
//! intensity-swept seeded Monte-Carlo, the inverse-variance fuse and the Gaussian-copula
//! correlated sweep to answer the two questions a resilience architect needs before
//! trusting *"more layers = safer"*: how much does layering actually buy, and does that
//! benefit survive correlation?
//!
//! On top of the open core this pack adds the commercial study surface: a byte-stable
//! committed `conflict_resilience_table.csv`, a deterministic branded three-panel Figure 1
//! — median fused error vs threat intensity, total-loss probability vs intensity for the
//! 4/3/2-layer configurations, and a per-vector survival panel — a branded PDF with the
//! Validated-vs-Modelled split and a **non-certification** disclaimer on the artifact face,
//! and a `papers/conflict-resilience/` skeleton.
//!
//! ## Validated vs Modelled (honesty firewall)
//! * **Validated (vs an independent oracle).** The Monte-Carlo total-loss probability
//!   converges to the **closed-form independent product** `Π_i layer_loss_prob_i` (residual
//!   reported per intensity). The **inverse-variance fuse** is a closed-form identity
//!   (`fuse([3,4]) = 12/5`, residual reported). At **`ρ = 0` the Gaussian copula reduces to
//!   the independent model** (their total-loss estimates agree within Monte-Carlo error for
//!   the same seed, residual reported).
//! * **Modelled.** The per-layer vulnerability / availability / accuracy magnitudes are
//!   sourced-but-Modelled inputs (cited via [`kshana::conflict_threat_params`] — JammerTest
//!   2024, TEXBAT, EASA SIB, RTCA DO-229, LunaNet/IOAG); the ~7× headline ratio and the
//!   ratio-vs-correlation curve shape are properties of that Modelled parameterisation, not
//!   certified figures. **Not a certified navigation-availability product.**

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::conflict_resilience::{
    inverse_variance_fuse, prior_sensitivity, resilience_ratio_closed_form, simulate_correlated,
    simulate_independent, sweep_correlation, sweep_intensity, ConflictLayer, CorrelationStats,
    IntensityStats, PriorSensitivity,
};
use kshana::conflict_threat_params::conflict_baseline;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::packs::study_pdf::{Cell, StudyPdf};
use crate::report_brand::{
    cover_html, modelled_banner_html, BRASS, BRASS_MUTED, FONT_BODY, FONT_HEAD, INK, IVORY,
    IVORY_LIGHT, NAVY,
};

/// The honesty label carried on the result document (contains both tier words).
const LABEL: &str = "MODELLED layered-PNT conflict resilience study (P7/L55). VALIDATED core: the \
Monte-Carlo total-loss probability converges to the closed-form independent product \
Pi_i layer_loss_prob_i (residual reported per intensity); the inverse-variance position fuse \
sigma_fused=(sum_i 1/sigma_i^2)^(-1/2) is a closed-form identity (fuse([3,4])=12/5); at correlation \
rho=0 the Gaussian copula reduces to the independent model (agreeing within Monte-Carlo error for the \
same seed). MODELLED: the per-layer vulnerability / availability / accuracy magnitudes are \
sourced-but-Modelled inputs (kshana::conflict_threat_params — JammerTest 2024, TEXBAT, EASA SIB, \
RTCA DO-229, LunaNet/IOAG); the ~7x headline resilience ratio and the ratio-vs-correlation curve \
shape are properties of that Modelled parameterisation, not certified figures. This is an engineering \
study, NOT a certification and NOT a certified navigation-availability product.";

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

fn tagged(v: Value, t: Tag) -> Value {
    json!({ "value": v, "provenance": t.as_str() })
}

/// Format an `f64` for a byte-stable CSV / table cell (fixed scientific notation; explicit
/// `nan` / `inf` tokens so a total-loss-only intensity never fabricates a finite error).
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

/// Emit a finite `f64` as a JSON number, or `null` for a non-finite value.
fn num_or_null(x: f64) -> Value {
    if x.is_finite() {
        Value::from(x)
    } else {
        Value::Null
    }
}

// ── Scenario ─────────────────────────────────────────────────────────────────

fn d_intensity_min() -> f64 {
    0.0
}
fn d_intensity_max() -> f64 {
    1.0
}
fn d_intensity_steps() -> usize {
    11
}
fn d_rho_values() -> Vec<f64> {
    vec![0.0, 0.2, 0.4, 0.6, 0.8, 0.95]
}
fn d_trials() -> usize {
    4000
}
fn d_seed() -> u64 {
    20_260_709
}
fn d_primary_layer() -> usize {
    0
}
fn d_sensitivity_samples() -> usize {
    2000
}

/// A `pro-conflict-resilience-study` scenario. Every field is optional; empty TOML
/// reproduces the sourced four-layer conflict baseline over `[0, 1]` intensity.
#[derive(Debug, Clone, Deserialize)]
pub struct ConflictResilienceStudyScenario {
    /// PNT layers; empty ⇒ the sourced conflict baseline
    /// ([`kshana::conflict_threat_params::conflict_baseline`]). **Modelled** magnitudes.
    #[serde(default)]
    pub layers: Vec<ConflictLayer>,
    #[serde(default = "d_intensity_min")]
    pub intensity_min: f64,
    #[serde(default = "d_intensity_max")]
    pub intensity_max: f64,
    #[serde(default = "d_intensity_steps")]
    pub intensity_steps: usize,
    /// Denial-correlation values swept for the resilience-ratio curve.
    #[serde(default = "d_rho_values")]
    pub rho_values: Vec<f64>,
    /// Monte-Carlo trials per grid point.
    #[serde(default = "d_trials")]
    pub trials: usize,
    /// Seed for the deterministic (ChaCha8) RNG.
    #[serde(default = "d_seed")]
    pub seed: u64,
    /// Index of the single layer used as the ratio baseline.
    #[serde(default = "d_primary_layer")]
    pub primary_layer: usize,
    /// Monte-Carlo samples for the prior-sensitivity block.
    #[serde(default = "d_sensitivity_samples")]
    pub sensitivity_samples: usize,
}

impl Default for ConflictResilienceStudyScenario {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            intensity_min: d_intensity_min(),
            intensity_max: d_intensity_max(),
            intensity_steps: d_intensity_steps(),
            rho_values: d_rho_values(),
            trials: d_trials(),
            seed: d_seed(),
            primary_layer: d_primary_layer(),
            sensitivity_samples: d_sensitivity_samples(),
        }
    }
}

/// A total-loss-vs-intensity curve for a k-layer sub-configuration.
struct LayerConfigCurve {
    n_layers: usize,
    total_loss_mc: Vec<f64>,
}

/// Everything the study derives, computed once and shared by JSON / CSV / figure / report /
/// PDF.
struct StudyModel {
    layers: Vec<ConflictLayer>,
    citations: Vec<String>,
    priors: Vec<(f64, f64)>,
    primary: usize,
    trials: usize,
    grid: Vec<f64>,
    reference_intensity: f64,
    intensity_sweep: Vec<IntensityStats>,
    correlation_sweep: Vec<CorrelationStats>,
    layer_configs: Vec<LayerConfigCurve>,
    ratio_closed_form: f64,
    ratio_mc_independent: f64,
    sensitivity: PriorSensitivity,
    // Validated oracle residuals.
    mc_closed_form_max_residual: f64,
    fuse_identity_residual: f64,
    copula_rho0_residual: f64,
    scenario_hash: String,
}

impl ConflictResilienceStudyScenario {
    pub const KIND: &'static str = "pro-conflict-resilience-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro P7/L55 study: drive the open layered-PNT conflict-resilience core (kshana::conflict_resilience) — the intensity-swept seeded Monte-Carlo, the closed-form inverse-variance fuse and the Gaussian-copula correlated sweep — to quantify how much layering buys and whether it survives correlation. Emits a byte-stable conflict_resilience_table.csv, a deterministic branded three-panel Figure 1 (median fused error vs threat intensity; total-loss probability vs intensity for the 4/3/2-layer configurations; a per-vector survival panel), a branded PDF with the Validated-vs-Modelled split and a NON-CERTIFICATION disclaimer on the artifact face, and a papers/conflict-resilience/ skeleton. VALIDATED: the Monte-Carlo total-loss converges to the closed-form independent product Pi_i layer_loss_prob_i; the inverse-variance fuse is a closed-form identity; at rho=0 the copula reduces to the independent model. MODELLED: the per-layer vulnerability/availability/accuracy magnitudes are sourced-but-Modelled inputs (JammerTest 2024, TEXBAT, EASA SIB, RTCA DO-229, LunaNet/IOAG via conflict_threat_params), and the ~7x headline and the ratio-vs-correlation curve shape follow from that parameterisation. Not a certified navigation-availability product.",
            required_fields: &[],
            optional_fields: &[
                "layers",
                "intensity_min",
                "intensity_max",
                "intensity_steps",
                "rho_values",
                "trials",
                "seed",
                "primary_layer",
                "sensitivity_samples",
            ],
        }
    }

    /// A deterministic 12-hex scenario fingerprint over the canonical config.
    fn compute_hash(&self, layers: &[ConflictLayer]) -> String {
        let lstr: Vec<String> = layers
            .iter()
            .map(|l| {
                format!(
                    "{}|{:.4}|{:.4}|{:.4}|{:.4}",
                    l.name, l.availability, l.sigma_m, l.vulnerability, l.vector_weight
                )
            })
            .collect();
        let rho: Vec<String> = self.rho_values.iter().map(|r| format!("{r:.4}")).collect();
        let canonical = format!(
            "conflict-resilience-study|layers=[{}]|int=[{:.4},{:.4}]/{}|rho=[{}]|trials={}|seed={}|primary={}|samples={}",
            lstr.join(";"),
            self.intensity_min,
            self.intensity_max,
            self.intensity_steps,
            rho.join(","),
            self.trials,
            self.seed,
            self.primary_layer,
            self.sensitivity_samples,
        );
        let mut h = Sha256::new();
        h.update(canonical.as_bytes());
        hex::encode(h.finalize()).chars().take(12).collect()
    }

    /// Resolve the layer set (sourced baseline when none is supplied), the paired
    /// `[min, max]` vulnerability priors, and a provenance citation per layer.
    fn resolved(&self) -> (Vec<ConflictLayer>, Vec<(f64, f64)>, Vec<String>) {
        if self.layers.is_empty() {
            let base = conflict_baseline();
            let layers = base
                .iter()
                .map(|p| ConflictLayer {
                    name: p.layer.to_string(),
                    availability: p.availability,
                    sigma_m: p.sigma_m,
                    vulnerability: p.vulnerability_nominal,
                    vector_weight: p.vector_weight,
                })
                .collect();
            let priors = base
                .iter()
                .map(|p| (p.vulnerability_min, p.vulnerability_max))
                .collect();
            let citations = base.iter().map(|p| p.citation.to_string()).collect();
            (layers, priors, citations)
        } else {
            let layers: Vec<ConflictLayer> = self
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let mut l = l.clone();
                    if l.name.trim().is_empty() {
                        l.name = format!("layer {i}");
                    }
                    l
                })
                .collect();
            let priors = layers
                .iter()
                .map(|l| {
                    (
                        (l.vulnerability - 0.1).clamp(0.0, 1.0),
                        (l.vulnerability + 0.1).clamp(0.0, 1.0),
                    )
                })
                .collect();
            let citations = layers
                .iter()
                .map(|_| "User-supplied Modelled layer (no external provenance).".to_string())
                .collect();
            (layers, priors, citations)
        }
    }

    fn compute(&self) -> Result<StudyModel, KshanaError> {
        let (layers, priors, citations) = self.resolved();
        if layers.is_empty() {
            return Err(KshanaError::InvalidInput(
                "conflict-resilience-study needs at least one layer".into(),
            ));
        }
        for (i, l) in layers.iter().enumerate() {
            if !(0.0..=1.0).contains(&l.availability) {
                return Err(KshanaError::InvalidInput(format!(
                    "layer {i} availability {} not in [0, 1]",
                    l.availability
                )));
            }
            if !(l.sigma_m.is_finite() && l.sigma_m > 0.0) {
                return Err(KshanaError::InvalidInput(format!(
                    "layer {i} sigma_m must be finite and positive"
                )));
            }
            if !(l.vulnerability.is_finite() && l.vulnerability >= 0.0) {
                return Err(KshanaError::InvalidInput(format!(
                    "layer {i} vulnerability must be finite and >= 0"
                )));
            }
            if !(l.vector_weight.is_finite() && l.vector_weight >= 0.0) {
                return Err(KshanaError::InvalidInput(format!(
                    "layer {i} vector_weight must be finite and >= 0"
                )));
            }
        }
        if self.primary_layer >= layers.len() {
            return Err(KshanaError::InvalidInput(format!(
                "primary_layer {} out of range (0..{})",
                self.primary_layer,
                layers.len()
            )));
        }
        if self.trials == 0 {
            return Err(KshanaError::InvalidInput("trials must be >= 1".into()));
        }
        if !(self.intensity_min.is_finite() && self.intensity_max.is_finite())
            || self.intensity_max <= self.intensity_min
        {
            return Err(KshanaError::InvalidInput(format!(
                "intensity grid must have finite min < max (got [{}, {}])",
                self.intensity_min, self.intensity_max
            )));
        }
        if self.intensity_steps < 2 {
            return Err(KshanaError::InvalidInput(
                "intensity_steps must be >= 2".into(),
            ));
        }
        if self.rho_values.is_empty() {
            return Err(KshanaError::InvalidInput(
                "rho_values must have at least one value".into(),
            ));
        }
        for &r in &self.rho_values {
            if !(0.0..=1.0).contains(&r) {
                return Err(KshanaError::InvalidInput(format!(
                    "rho values must lie in [0, 1] (got {r})"
                )));
            }
        }

        let primary = self.primary_layer;
        let trials = self.trials;
        let seed = self.seed;
        let grid: Vec<f64> = (0..self.intensity_steps)
            .map(|i| {
                self.intensity_min
                    + (self.intensity_max - self.intensity_min) * i as f64
                        / (self.intensity_steps - 1) as f64
            })
            .collect();
        let reference_intensity = *grid.last().unwrap();

        // ── Core sweeps (L34/L35), driven by the open engine ──
        let intensity_sweep = sweep_intensity(&layers, &grid, trials, seed);
        let correlation_sweep = sweep_correlation(
            &layers,
            reference_intensity,
            &self.rho_values,
            primary,
            trials,
            seed,
        );

        // ── 4/3/2-layer total-loss curves ("more layers = safer") ──
        let n = layers.len();
        let mut layer_configs = Vec::new();
        for k in [n, n.saturating_sub(1), n.saturating_sub(2)] {
            if k < 1
                || (layer_configs
                    .iter()
                    .any(|c: &LayerConfigCurve| c.n_layers == k))
            {
                continue;
            }
            let subset = &layers[..k];
            let sweep = sweep_intensity(subset, &grid, trials, seed);
            layer_configs.push(LayerConfigCurve {
                n_layers: k,
                total_loss_mc: sweep.iter().map(|s| s.total_loss_probability).collect(),
            });
        }

        // ── Headline resilience ratio (closed form + MC at ρ = 0) ──
        let ratio_closed_form = resilience_ratio_closed_form(&layers, reference_intensity, primary);
        let ratio_mc_independent = {
            let s = simulate_independent(&layers, reference_intensity, trials, seed ^ 0xA5A5);
            let single = s
                .per_layer_usable
                .get(primary)
                .map(|&u| 1.0 - u)
                .unwrap_or(1.0);
            if s.total_loss_probability > 0.0 {
                single / s.total_loss_probability
            } else {
                f64::INFINITY
            }
        };

        let sensitivity = prior_sensitivity(
            &layers,
            &priors,
            reference_intensity,
            primary,
            self.sensitivity_samples,
            seed,
        );

        // ── Validated oracle residuals ──
        // (1) MC total-loss vs closed-form independent product, worst over the grid.
        let mc_closed_form_max_residual = intensity_sweep
            .iter()
            .map(|s| (s.total_loss_probability - s.total_loss_closed_form).abs())
            .fold(0.0_f64, f64::max);
        // (2) inverse-variance fuse identity fuse([3,4]) = 12/5.
        let fuse_identity_residual = inverse_variance_fuse(&[3.0, 4.0])
            .map(|f| (f - 2.4).abs())
            .unwrap_or(f64::INFINITY);
        // (3) at ρ = 0 the copula reduces to the independent model (same seed).
        let copula_rho0_residual = {
            let indep = simulate_independent(&layers, reference_intensity, trials, seed ^ 0x5A5A);
            let corr0 =
                simulate_correlated(&layers, reference_intensity, 0.0, trials, seed ^ 0x5A5A);
            (indep.total_loss_probability - corr0.total_loss_probability).abs()
        };

        Ok(StudyModel {
            layers,
            citations,
            priors,
            primary,
            trials,
            grid,
            reference_intensity,
            intensity_sweep,
            correlation_sweep,
            layer_configs,
            ratio_closed_form,
            ratio_mc_independent,
            sensitivity,
            mc_closed_form_max_residual,
            fuse_identity_residual,
            copula_rho0_residual,
            scenario_hash: self.compute_hash(&self.resolved().0),
        })
    }

    /// The committed, byte-stable `conflict_resilience_table.csv`: one row per intensity
    /// with the layered MC total-loss, the closed-form independent product, the median /
    /// mean fused error, the 4/3/2-layer total-loss curves and the per-layer usable
    /// fraction. Fixed-precision scientific notation.
    fn csv_string(&self, m: &StudyModel) -> String {
        let mut s = String::new();
        s.push_str(
            "# kshana-pro conflict-resilience study (L55) — layered-PNT resilience vs threat intensity\n",
        );
        s.push_str(&format!(
            "# scenario_hash={} n_layers={} trials={} reference_intensity={:.4} ratio_closed_form={:.4} ratio_mc={:.4}\n",
            m.scenario_hash,
            m.layers.len(),
            m.trials,
            m.reference_intensity,
            m.ratio_closed_form,
            m.ratio_mc_independent,
        ));
        s.push_str(
            "# provenance: total_loss_mc converges to total_loss_closed_form = Prod_i layer_loss_prob_i (Validated); per-layer magnitudes = Modelled (see conflict_threat_params). NOT a certification.\n",
        );
        // Header.
        s.push_str("intensity,total_loss_mc,total_loss_closed_form,median_fused_error_m,mean_fused_error_m");
        for cfg in &m.layer_configs {
            s.push_str(&format!(",loss_{}layer", cfg.n_layers));
        }
        for (i, l) in m.layers.iter().enumerate() {
            let _ = l;
            s.push_str(&format!(",usable_L{i}"));
        }
        s.push('\n');
        for (row, stats) in m.intensity_sweep.iter().enumerate() {
            s.push_str(&format!(
                "{},{},{},{},{}",
                fmt_f64(stats.intensity),
                fmt_f64(stats.total_loss_probability),
                fmt_f64(stats.total_loss_closed_form),
                fmt_f64(stats.median_fused_error_m),
                fmt_f64(stats.mean_fused_error_m),
            ));
            for cfg in &m.layer_configs {
                s.push_str(&format!(",{}", fmt_f64(cfg.total_loss_mc[row])));
            }
            for u in &stats.per_layer_usable {
                s.push_str(&format!(",{}", fmt_f64(*u)));
            }
            s.push('\n');
        }
        s
    }

    fn json(&self, m: &StudyModel) -> Value {
        let layers: Vec<Value> = m
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let (lo, hi) = m.priors[i];
                json!({
                    "index": i,
                    "name": l.name,
                    "availability": tagged(json!(l.availability), Tag::Modelled),
                    "sigma_m": tagged(json!(l.sigma_m), Tag::Modelled),
                    "vulnerability": tagged(json!(l.vulnerability), Tag::Modelled),
                    "vector_weight": tagged(json!(l.vector_weight), Tag::Modelled),
                    "vulnerability_prior_min": lo,
                    "vulnerability_prior_max": hi,
                    "citation": m.citations.get(i).cloned().unwrap_or_default(),
                })
            })
            .collect();

        let intensity_sweep: Vec<Value> = m
            .intensity_sweep
            .iter()
            .map(|s| {
                json!({
                    "intensity": s.intensity,
                    "total_loss_probability": s.total_loss_probability,
                    "total_loss_closed_form": s.total_loss_closed_form,
                    "median_fused_error_m": num_or_null(s.median_fused_error_m),
                    "mean_fused_error_m": num_or_null(s.mean_fused_error_m),
                    "per_layer_usable": s.per_layer_usable,
                    "per_layer_deny_rate": s.per_layer_deny_rate,
                })
            })
            .collect();

        let correlation_sweep: Vec<Value> = m
            .correlation_sweep
            .iter()
            .map(|s| {
                json!({
                    "rho": s.rho,
                    "layered_total_loss": s.layered_total_loss,
                    "single_layer_loss": s.single_layer_loss,
                    "resilience_ratio": num_or_null(s.resilience_ratio),
                })
            })
            .collect();

        let layer_configs: Vec<Value> = m
            .layer_configs
            .iter()
            .map(|c| {
                json!({
                    "n_layers": c.n_layers,
                    "total_loss_mc": c.total_loss_mc,
                })
            })
            .collect();

        json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "paper": "P7 — layered-PNT conflict resilience (L55)",
            "scenario_hash": m.scenario_hash,
            "label": LABEL,
            "certification": "NOT A CERTIFICATION — a MODELLED engineering study, not a certified navigation-availability product and not ESA acceptance or TRL maturation.",
            "provenance_legend": {
                "Validated": "the layered Monte-Carlo total-loss converges to the closed-form independent product Pi_i layer_loss_prob_i; the inverse-variance fuse is a closed-form identity; at rho=0 the Gaussian copula reduces to the independent model.",
                "Modelled": "the per-layer vulnerability/availability/accuracy magnitudes are sourced-but-Modelled inputs (conflict_threat_params); the ~7x headline and the ratio-vs-correlation curve shape.",
            },
            "trials": m.trials,
            "primary_layer": m.primary,
            "reference_intensity": m.reference_intensity,
            "intensity_grid": m.grid,
            "layers": layers,
            "resilience_ratio": {
                "closed_form_independent": tagged(num_or_null(m.ratio_closed_form), Tag::Modelled),
                "monte_carlo_independent": tagged(num_or_null(m.ratio_mc_independent), Tag::Modelled),
                "headline": "~7x layered-vs-single-layer reduction in total-loss probability under the INDEPENDENCE assumption; see correlation_sweep for how it shrinks as denial correlation rises (correlation defeats layering).",
            },
            "intensity_sweep": intensity_sweep,
            "layer_configs": layer_configs,
            "correlation_sweep": {
                "reference_intensity": m.reference_intensity,
                "rows": correlation_sweep,
            },
            "prior_sensitivity": {
                "nominal_ratio": num_or_null(m.sensitivity.nominal_ratio),
                "ratio_ci_95": [m.sensitivity.ratio_ci.0, m.sensitivity.ratio_ci.1],
                "total_loss_ci_95": [m.sensitivity.total_loss_ci.0, m.sensitivity.total_loss_ci.1],
                "effort_reallocation_ratio_ci_95": [m.sensitivity.effort_ratio_ci.0, m.sensitivity.effort_ratio_ci.1],
                "samples": m.sensitivity.samples,
            },
            "oracles": {
                "mc_vs_closed_form_max_residual": tagged(json!(m.mc_closed_form_max_residual), Tag::Validated),
                "inverse_variance_fuse_identity_residual": tagged(json!(m.fuse_identity_residual), Tag::Validated),
                "copula_rho0_vs_independent_residual": tagged(json!(m.copula_rho0_residual), Tag::Validated),
                "note": "The conflict-resilience FOM has no separate oracle-registration seam in the registry; per the repo's established pattern (see optical_rf_hybrid_study_pack) the Validated oracle residuals are surfaced here in the result document.",
            },
            "figure": { "fig1_svg": fig_svg(m) },
            "report_html": report_html(self, m),
            "csv": self.csv_string(m),
            "csv_filename": "conflict_resilience_table.csv",
        })
    }

    fn summary(&self, m: &StudyModel) -> String {
        let first = m.correlation_sweep.first();
        let last = m.correlation_sweep.last();
        format!(
            "Kshana Pro conflict-resilience study (L55) | {n} layers | reference intensity {ri:.2} | \
             resilience ratio closed-form {cf:.2}x MC {mc:.2}x (layered vs single) | correlation defeats \
             layering: {r0:.2}x @ rho {rho0:.2} -> {r1:.2}x @ rho {rho1:.2} | oracles: MC-vs-closed-form {o1:.1e} \
             fuse-identity {o2:.1e} copula(rho=0) {o3:.1e} (Validated) | ~7x headline MODELLED | \
             CSV conflict_resilience_table.csv (byte-stable) — NOT a certification",
            n = m.layers.len(),
            ri = m.reference_intensity,
            cf = m.ratio_closed_form,
            mc = m.ratio_mc_independent,
            r0 = first.map(|s| s.resilience_ratio).unwrap_or(f64::NAN),
            rho0 = first.map(|s| s.rho).unwrap_or(0.0),
            r1 = last.map(|s| s.resilience_ratio).unwrap_or(f64::NAN),
            rho1 = last.map(|s| s.rho).unwrap_or(0.0),
            o1 = m.mc_closed_form_max_residual,
            o2 = m.fuse_identity_residual,
            o3 = m.copula_rho0_residual,
        )
    }

    /// Render the branded PDF deliverable (self-contained `%PDF-…`).
    fn to_pdf(&self, m: &StudyModel) -> Result<Vec<u8>, String> {
        let mut pdf = StudyPdf::new();
        pdf.cover(
            "Layered-PNT Conflict Resilience",
            "P7 / L55 — how much layering buys, and whether it survives correlation",
        );
        pdf.modelled_banner(&[
            "The MC->closed-form convergence, the inverse-variance fuse and the rho=0 copula",
            "reduction are Validated; the per-layer threat magnitudes are sourced-but-Modelled.",
        ]);

        pdf.heading("Executive Summary");
        pdf.paragraph(&self.summary(m));
        pdf.paragraph(
            "A PNT user in a conflict zone fields several navigation layers that all ride the \
             same RF band. This study denies each layer with a seeded Monte-Carlo, fuses the \
             survivors by the closed-form inverse-variance rule, and reports the total-loss \
             probability against a threat-intensity sweep. The headline resilience ratio is ~7x \
             UNDER THE INDEPENDENCE ASSUMPTION; a Gaussian-copula correlated-denial sweep then \
             shows that benefit collapse toward 1 as denial correlation rises.",
        );

        pdf.heading("NON-CERTIFICATION");
        pdf.paragraph(
            "This artifact is a MODELLED engineering study. It is NOT a certification, NOT a \
             certified navigation-availability product, and NOT ESA acceptance or TRL maturation. \
             The per-layer vulnerability / availability / accuracy magnitudes are sourced-but-\
             Modelled inputs (see the citations table); only the stated oracle identities are \
             Validated.",
        );

        pdf.heading("Validated oracle residuals");
        pdf.header_row(&[(0.0, "Oracle"), (120.0, "Residual"), (156.0, "Provenance")]);
        let oracle_rows = [
            (
                "MC total-loss = Π layer_loss (worst grid)",
                m.mc_closed_form_max_residual,
            ),
            (
                "inverse-variance fuse([3,4]) = 12/5",
                m.fuse_identity_residual,
            ),
            (
                "copula(ρ=0) = independent (same seed)",
                m.copula_rho0_residual,
            ),
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
                    x_mm: 120.0,
                    text: &fmt_f64(res),
                    bold: true,
                    colour: INK,
                },
                Cell {
                    x_mm: 156.0,
                    text: "Validated",
                    bold: true,
                    colour: BRASS_MUTED,
                },
            ]);
        }

        pdf.gap(2.0);
        pdf.heading("Resilience ratio vs denial correlation");
        pdf.header_row(&[
            (0.0, "rho"),
            (40.0, "layered loss"),
            (100.0, "ratio"),
            (140.0, "Provenance"),
        ]);
        for cs in &m.correlation_sweep {
            pdf.row(&[
                Cell {
                    x_mm: 0.0,
                    text: &format!("{:.2}", cs.rho),
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 40.0,
                    text: &fmt_f64(cs.layered_total_loss),
                    bold: false,
                    colour: INK,
                },
                Cell {
                    x_mm: 100.0,
                    text: &format!("{:.2}x", cs.resilience_ratio),
                    bold: true,
                    colour: INK,
                },
                Cell {
                    x_mm: 140.0,
                    text: "Modelled",
                    bold: true,
                    colour: BRASS_MUTED,
                },
            ]);
        }

        pdf.gap(2.0);
        pdf.heading("Modelled per-layer priors (sourced provenance)");
        for (i, l) in m.layers.iter().enumerate() {
            let cite = m.citations.get(i).cloned().unwrap_or_default();
            pdf.kv(
                &format!("{}. {}", i + 1, l.name),
                &format!(
                    "avail {:.2}, sigma {:.1} m, vuln {:.2}, vector {:.2} — {}",
                    l.availability, l.sigma_m, l.vulnerability, l.vector_weight, cite
                ),
            );
        }

        pdf.heading("Caveats (read first)");
        pdf.paragraph(
            "- Only the MC->closed-form convergence, the inverse-variance fuse identity and the \
             rho=0 copula reduction are Validated; the ~7x headline magnitude and the \
             ratio-vs-correlation curve shape are properties of the Modelled priors.",
        );
        pdf.paragraph(
            "- Correlation defeats layering: as the shared-vector denial correlation rises the \
             resilience ratio collapses toward 1. Layer DIVERSITY (not count) is what buys \
             resilience.",
        );

        let footer =
            "Ashforde OU  |  Kshana Pro  |  MODELLED engineering study - NOT a certification";
        pdf.render("Kshana Pro - conflict resilience (L55)", footer)
    }

    /// Write the L55 paper artifacts (committed CSV + figure SVG + HTML report + branded PDF)
    /// into `papers/conflict-resilience/` (a NEW dir — the founder's other paper dirs are
    /// untouched). Invoked out-of-band, never from `run`.
    pub fn emit_paper_artifacts(&self) -> std::io::Result<std::path::PathBuf> {
        let m = self
            .compute()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("papers/conflict-resilience");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("conflict_resilience_table.csv"),
            self.csv_string(&m),
        )?;
        std::fs::write(dir.join("fig1_conflict_resilience.svg"), fig_svg(&m))?;
        std::fs::write(dir.join("report.html"), report_html(self, &m))?;
        let pdf = self.to_pdf(&m).map_err(std::io::Error::other)?;
        std::fs::write(dir.join("report.pdf"), pdf)?;
        Ok(dir)
    }
}

impl Scenario for ConflictResilienceStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let m = self.compute()?;
        let doc = self.json(&m).to_string();
        let svg = fig_svg(&m);
        let summary = self.summary(&m);
        Ok(RunOutput {
            json: doc,
            svg,
            summary,
        })
    }
}

impl ExternalPack for ConflictResilienceStudyScenario {
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

// ── Figure 1 — three panels (deterministic SVG) ───────────────────────────────

fn layer_color(i: usize) -> &'static str {
    const PAL: [&str; 6] = [
        "#f0a868", "#e5645a", "#c7aa66", "#9b8cff", "#6fae7a", "#5fb0c9",
    ];
    PAL[i % PAL.len()]
}

fn config_color(i: usize) -> &'static str {
    const PAL: [&str; 3] = ["#2dd4bf", "#f0a868", "#e5645a"];
    PAL[i % PAL.len()]
}

fn fig_svg(m: &StudyModel) -> String {
    let (w, h) = (900.0_f64, 700.0_f64);
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"{FONT_BODY}\" font-size=\"11\">"
    );
    s += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{NAVY}\"/>");
    s += &format!("<text x=\"20\" y=\"28\" fill=\"{BRASS}\" font-family=\"{FONT_HEAD}\" font-size=\"18\" font-weight=\"bold\">Figure 1 — layered-PNT conflict resilience (P7)</text>");
    s += &format!("<text x=\"20\" y=\"46\" fill=\"{IVORY}\">MODELLED priors &#8226; <tspan fill=\"#2dd4bf\">MC&#8594;closed-form / fuse / copula(&#961;=0) Validated</tspan> &#8226; NOT a certification</text>");

    let imax = m.grid.last().copied().unwrap_or(1.0).max(1e-9);

    // ── Panel (a): median fused error vs threat intensity ──
    panel_median_error(&mut s, m, 60.0, 90.0, 360.0, 210.0, imax);
    // ── Panel (b): total-loss probability vs intensity for 4/3/2-layer configs ──
    panel_total_loss(&mut s, m, 500.0, 90.0, 360.0, 210.0, imax);
    // ── Panel (c): per-vector survival panel (per-layer usable vs intensity) ──
    panel_survival(&mut s, m, 60.0, 420.0, 800.0, 210.0, imax);

    s += "</svg>";
    s
}

fn panel_median_error(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64, imax: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(a) median fused error vs threat intensity</text>", y - 8.0);
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    let vmax = m
        .intensity_sweep
        .iter()
        .map(|s2| s2.median_fused_error_m)
        .filter(|v| v.is_finite())
        .fold(1e-9_f64, f64::max);
    let sx = |t: f64| x + (t / imax) * pw;
    let sy = |v: f64| y + ph * (1.0 - (v / vmax).clamp(0.0, 1.0));
    for g in 0..=4 {
        let frac = g as f64 / 4.0;
        let gy = y + ph * (1.0 - frac);
        *s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{gy:.1}\" x2=\"{:.1}\" y2=\"{gy:.1}\" stroke=\"#1c2637\"/>",
            x + pw
        );
        *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\" text-anchor=\"end\">{:.1} m</text>", x - 4.0, gy + 3.0, frac * vmax);
    }
    let mut pts = String::new();
    for st in &m.intensity_sweep {
        if st.median_fused_error_m.is_finite() {
            pts.push_str(&format!(
                "{:.1},{:.1} ",
                sx(st.intensity),
                sy(st.median_fused_error_m)
            ));
        }
    }
    *s += &format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#d2925e\" stroke-width=\"2\"/>",
        pts.trim_end()
    );
    *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\" font-size=\"9\">threat intensity</text>", x + pw / 2.0, y + ph + 16.0);
}

fn panel_total_loss(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64, imax: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(b) total-loss probability vs intensity (4/3/2-layer)</text>", y - 8.0);
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    let sx = |t: f64| x + (t / imax) * pw;
    let sy = |p: f64| y + ph * (1.0 - p.clamp(0.0, 1.0));
    for g in 0..=4 {
        let frac = g as f64 / 4.0;
        let gy = sy(frac);
        *s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{gy:.1}\" x2=\"{:.1}\" y2=\"{gy:.1}\" stroke=\"#1c2637\"/>",
            x + pw
        );
        *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\" text-anchor=\"end\">{:.2}</text>", x - 4.0, gy + 3.0, frac);
    }
    let mut ly = y + 12.0;
    for (ci, cfg) in m.layer_configs.iter().enumerate() {
        let mut pts = String::new();
        for (row, st) in m.intensity_sweep.iter().enumerate() {
            pts.push_str(&format!(
                "{:.1},{:.1} ",
                sx(st.intensity),
                sy(cfg.total_loss_mc[row])
            ));
        }
        *s += &format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>",
            pts.trim_end(),
            config_color(ci)
        );
        *s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"12\" height=\"3\" fill=\"{}\"/>",
            x + pw - 92.0,
            ly - 3.0,
            config_color(ci)
        );
        *s += &format!(
            "<text x=\"{:.1}\" y=\"{ly:.1}\" fill=\"{IVORY}\" font-size=\"8\">{}-layer</text>",
            x + pw - 76.0,
            cfg.n_layers
        );
        ly += 12.0;
    }
    *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\" font-size=\"9\">threat intensity</text>", x + pw / 2.0, y + ph + 16.0);
}

fn panel_survival(s: &mut String, m: &StudyModel, x: f64, y: f64, pw: f64, ph: f64, imax: f64) {
    *s += &format!("<text x=\"{x:.0}\" y=\"{:.0}\" fill=\"{IVORY_LIGHT}\">(c) per-vector survival — per-layer usable fraction vs threat intensity</text>", y - 8.0);
    *s += &format!("<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{pw:.1}\" height=\"{ph:.1}\" fill=\"#0b111d\" stroke=\"{BRASS_MUTED}\" stroke-width=\"0.5\"/>");
    let sx = |t: f64| x + (t / imax) * pw;
    let sy = |p: f64| y + ph * (1.0 - p.clamp(0.0, 1.0));
    for g in 0..=4 {
        let frac = g as f64 / 4.0;
        let gy = sy(frac);
        *s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{gy:.1}\" x2=\"{:.1}\" y2=\"{gy:.1}\" stroke=\"#1c2637\"/>",
            x + pw
        );
        *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\" text-anchor=\"end\">{:.2}</text>", x - 4.0, gy + 3.0, frac);
    }
    for (li, layer) in m.layers.iter().enumerate() {
        let mut pts = String::new();
        for st in &m.intensity_sweep {
            if let Some(&u) = st.per_layer_usable.get(li) {
                pts.push_str(&format!("{:.1},{:.1} ", sx(st.intensity), sy(u)));
            }
        }
        *s += &format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.8\"/>",
            pts.trim_end(),
            layer_color(li)
        );
        // Legend swatch.
        let lx = x + 12.0 + (li as f64) * 150.0;
        *s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"12\" height=\"3\" fill=\"{}\"/>",
            lx,
            y + ph + 22.0,
            layer_color(li)
        );
        let short: String = layer.name.chars().take(20).collect();
        *s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{IVORY}\" font-size=\"8\">{}</text>",
            lx + 16.0,
            y + ph + 25.0,
            short
        );
    }
    *s += &format!("<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{BRASS}\" text-anchor=\"middle\" font-size=\"9\">threat intensity (usable = available AND not denied)</text>", x + pw / 2.0, y + ph + 44.0);
}

// ── Branded HTML report ──────────────────────────────────────────────────────

fn td(s: &str) -> String {
    format!("<td style=\"border:1px solid #26324a;padding:4px 8px\">{s}</td>")
}
fn th(s: &str) -> String {
    format!("<th style=\"border:1px solid #26324a;padding:4px 8px;text-align:left;color:{IVORY_LIGHT}\">{s}</th>")
}

fn report_html(_scn: &ConflictResilienceStudyScenario, m: &StudyModel) -> String {
    let fig = fig_svg(m);
    let corr_rows: String = m
        .correlation_sweep
        .iter()
        .map(|s| {
            format!(
                "<tr>{}{}{}{}</tr>",
                td(&format!("{:.2}", s.rho)),
                td(&fmt_f64(s.layered_total_loss)),
                td(&fmt_f64(s.single_layer_loss)),
                td(&format!(
                    "<span style=\"color:#2dd4bf;font-weight:600\">{:.2}x</span>",
                    s.resilience_ratio
                )),
            )
        })
        .collect();

    let oracle_rows: String = [
        (
            "MC total-loss = &#928; layer_loss (worst grid)",
            m.mc_closed_form_max_residual,
        ),
        (
            "inverse-variance fuse([3,4]) = 12/5",
            m.fuse_identity_residual,
        ),
        (
            "copula(&#961;=0) = independent (same seed)",
            m.copula_rho0_residual,
        ),
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

    let layer_rows: String = m
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            format!(
                "<tr>{}{}{}{}{}</tr>",
                td(&l.name),
                td(&format!("{:.2}", l.availability)),
                td(&format!("{:.1}", l.sigma_m)),
                td(&format!("{:.2}", l.vulnerability)),
                td(&format!(
                    "<span style=\"color:{BRASS};font-weight:600\">Modelled</span> — {}",
                    m.citations.get(i).cloned().unwrap_or_default()
                )),
            )
        })
        .collect();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <title>Kshana Pro — Conflict Resilience (L55)</title></head>\
         <body style=\"margin:0;background:{NAVY};color:{IVORY};font-family:{FONT_BODY};\
         max-width:980px;margin:0 auto;padding:0 28px 60px\">\
         {cover}{banner}\
         <div style=\"background:#3a1416;color:#f6d7d3;border:1px solid #e5645a;border-radius:4px;\
         padding:10px 14px;margin:14px 0;font-weight:600\">NON-CERTIFICATION — a MODELLED engineering \
         study, not a certified navigation-availability product and not ESA acceptance or TRL maturation.</div>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Does layering buy resilience — and does it survive correlation?</h2>\
         <p>A {n}-layer PNT architecture is stressed by a seeded Monte-Carlo threat sweep. The layered \
         resilience ratio lands at closed-form <strong>{cf:.2}x</strong> (MC {mc:.2}x) under the \
         INDEPENDENCE assumption; as the shared-vector denial correlation rises it collapses toward 1 \
         (correlation defeats layering).</p>\
         {fig}\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Validated oracle residuals</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_o}{h_r}{h_p}</tr>{oracle_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Correlation defeats layering</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_rho}{h_ll}{h_sl}{h_rt}</tr>{corr_rows}</table>\
         <h2 style=\"font-family:{FONT_HEAD};color:{IVORY_LIGHT}\">Modelled per-layer priors (sourced provenance)</h2>\
         <table style=\"border-collapse:collapse;width:100%\"><tr>{h_ln}{h_av}{h_sg}{h_vu}{h_ci}</tr>{layer_rows}</table>\
         <p style=\"font-size:12px;opacity:.85\">Only the MC&#8594;closed-form convergence, the \
         inverse-variance fuse identity and the &#961;=0 copula reduction are \
         <span style=\"color:#2dd4bf\">Validated</span>; every per-layer magnitude is \
         <span style=\"color:{BRASS}\">Modelled</span>. The full table is committed to \
         <code>papers/conflict-resilience/conflict_resilience_table.csv</code> (byte-stable).</p>\
         </body></html>",
        cover = cover_html(
            "Layered-PNT Conflict Resilience",
            "P7 / L55 — how much layering buys, and whether it survives correlation"
        ),
        banner = modelled_banner_html(
            "the MC->closed-form convergence, the inverse-variance fuse and the rho=0 copula reduction \
             are Validated; the per-layer threat magnitudes are sourced-but-Modelled."
        ),
        n = m.layers.len(),
        cf = m.ratio_closed_form,
        mc = m.ratio_mc_independent,
        h_o = th("Oracle"),
        h_r = th("Residual"),
        h_p = th("Provenance"),
        h_rho = th("&#961;"),
        h_ll = th("Layered loss"),
        h_sl = th("Single-layer loss"),
        h_rt = th("Ratio"),
        h_ln = th("Layer"),
        h_av = th("Avail"),
        h_sg = th("&#963; (m)"),
        h_vu = th("Vuln"),
        h_ci = th("Provenance"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> ConflictResilienceStudyScenario {
        ConflictResilienceStudyScenario {
            trials: 3000,
            sensitivity_samples: 300,
            ..Default::default()
        }
    }

    #[test]
    fn runs_and_carries_the_honesty_label() {
        let out = small().run().unwrap();
        let v: Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["kind"], ConflictResilienceStudyScenario::KIND);
        let label = v["label"].as_str().unwrap();
        assert!(label.contains("VALIDATED"));
        assert!(label.contains("MODELLED"));
        assert!(out.summary.contains("conflict-resilience"));
        // The non-certification disclaimer is on the document face.
        assert!(v["certification"]
            .as_str()
            .unwrap()
            .contains("NOT A CERTIFICATION"));
    }

    #[test]
    fn deterministic_same_input_same_output() {
        let scn = small();
        let a = scn.run().unwrap();
        let b = scn.run().unwrap();
        assert_eq!(a.json, b.json);
        assert_eq!(a.svg, b.svg);
        assert_eq!(a.summary, b.summary);
    }

    #[test]
    fn csv_is_well_formed_and_deterministic() {
        let scn = small();
        let m = scn.compute().unwrap();
        let a = scn.csv_string(&m);
        assert_eq!(a, scn.csv_string(&m));
        assert!(a.contains("intensity,total_loss_mc,total_loss_closed_form,median_fused_error_m,mean_fused_error_m"));
        assert!(a.contains("scenario_hash="));
        let data_rows = a
            .lines()
            .filter(|l| !l.starts_with('#') && !l.starts_with("intensity"))
            .count();
        assert_eq!(data_rows, m.intensity_sweep.len());
    }

    #[test]
    fn svg_is_well_formed_and_deterministic() {
        let scn = small();
        let m = scn.compute().unwrap();
        let a = fig_svg(&m);
        assert!(a.starts_with("<svg"));
        assert!(a.ends_with("</svg>"));
        assert_eq!(a, fig_svg(&m));
    }

    #[test]
    fn pdf_renders_to_valid_bytes() {
        let scn = small();
        let m = scn.compute().unwrap();
        let pdf = scn.to_pdf(&m).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        assert!(pdf.len() > 1000);
    }

    // ── ORACLE (Validated): MC total-loss converges to the closed-form product ──
    #[test]
    fn mc_converges_to_closed_form_product() {
        let scn = ConflictResilienceStudyScenario {
            trials: 12000,
            sensitivity_samples: 200,
            ..Default::default()
        };
        let m = scn.compute().unwrap();
        assert!(
            m.mc_closed_form_max_residual < 0.03,
            "MC vs closed-form worst residual {} too large",
            m.mc_closed_form_max_residual
        );
    }

    // ── ORACLE (Validated): the inverse-variance fuse identity ──
    #[test]
    fn inverse_variance_fuse_identity_holds() {
        let m = small().compute().unwrap();
        assert!(
            m.fuse_identity_residual < 1e-12,
            "fuse([3,4]) = 12/5 residual {} too large",
            m.fuse_identity_residual
        );
    }

    // ── ORACLE (Validated): the rho=0 copula reduces to the independent model ──
    #[test]
    fn copula_at_rho0_matches_independent() {
        let scn = ConflictResilienceStudyScenario {
            trials: 12000,
            sensitivity_samples: 200,
            ..Default::default()
        };
        let m = scn.compute().unwrap();
        assert!(
            m.copula_rho0_residual < 0.03,
            "copula(rho=0) vs independent residual {} too large",
            m.copula_rho0_residual
        );
    }

    #[test]
    fn headline_ratio_is_seven_x_and_correlation_defeats_layering() {
        let m = small().compute().unwrap();
        assert!(
            (6.0..8.0).contains(&m.ratio_closed_form),
            "closed-form ratio {} not ~7x",
            m.ratio_closed_form
        );
        let r0 = m.correlation_sweep.first().unwrap().resilience_ratio;
        let r1 = m.correlation_sweep.last().unwrap().resilience_ratio;
        assert!(r0 > 5.0, "ratio at rho=0 should be ~7x, got {r0}");
        assert!(r1 < r0, "ratio must shrink with correlation: {r0} -> {r1}");
    }

    #[test]
    fn layer_configs_cover_four_three_two() {
        let m = small().compute().unwrap();
        let sizes: Vec<usize> = m.layer_configs.iter().map(|c| c.n_layers).collect();
        assert_eq!(
            sizes,
            vec![4, 3, 2],
            "baseline must yield 4/3/2-layer curves"
        );
    }

    #[test]
    fn bad_config_is_rejected() {
        let bad_grid = ConflictResilienceStudyScenario {
            intensity_min: 1.0,
            intensity_max: 0.0,
            ..Default::default()
        };
        assert!(bad_grid.run().is_err());
        let bad_primary = ConflictResilienceStudyScenario {
            primary_layer: 99,
            ..Default::default()
        };
        assert!(bad_primary.run().is_err());
        let bad_trials = ConflictResilienceStudyScenario {
            trials: 0,
            ..Default::default()
        };
        assert!(bad_trials.run().is_err());
    }

    #[test]
    #[ignore = "writes the committed paper artifacts to papers/conflict-resilience/; run with --ignored"]
    fn emit_paper_artifacts() {
        let dir = ConflictResilienceStudyScenario::default()
            .emit_paper_artifacts()
            .unwrap();
        assert!(dir.join("conflict_resilience_table.csv").exists());
        assert!(dir.join("fig1_conflict_resilience.svg").exists());
        assert!(dir.join("report.html").exists());
        assert!(dir.join("report.pdf").exists());
    }
}
