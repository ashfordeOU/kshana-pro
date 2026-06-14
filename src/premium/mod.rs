// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # Premium fidelity (PR19) — quantum-clock systematics budget
//!
//! The open core models a clock at the *coefficient* level (white/flicker FM,
//! drift). This Pro module adds the **systematic** fractional-frequency shifts that
//! a real optical-clock payload must account for — the "full-physics quantum vs
//! coefficient-level" upgrade — as an auditable error budget:
//!
//!  * **Blackbody-radiation (BBR) shift** — scales as `(T/300 K)^4`.
//!  * **Gravitational redshift** — `g·h/c²` (exact, textbook).
//!  * **Second-order Zeeman shift** — scales as `B²`.
//!  * Arbitrary extra named effects (AC-Stark/lattice, collisional, …).
//!
//! **Honesty model:** the *formulas* are implemented here (simple, unit-tested);
//! the species-specific *coefficients* (BBR coefficient at 300 K, Zeeman
//! coefficient) are **cited inputs**, not claims baked into Pro — exactly the L1
//! pattern (calibrate from published data). A "full-physics" twin then composes
//! this deterministic budget with the open core's stochastic noise model.

use kshana::api::KshanaError;

/// Standard gravity (m/s²) and speed of light (m/s) — CODATA / defined constants.
const G_ACCEL: f64 = 9.806_65;
const C_LIGHT: f64 = 299_792_458.0;
/// BBR reference temperature (K) the published coefficient is quoted at.
const T_REF_K: f64 = 300.0;

/// One systematic effect in the budget.
#[derive(Debug, Clone)]
pub struct SystematicEffect {
    pub name: String,
    /// Fractional frequency shift Δν/ν (signed).
    pub fractional_shift: f64,
    /// Absolute (1σ) uncertainty on that shift (fractional).
    pub uncertainty: f64,
}

/// Inputs for the systematics budget. Coefficients are cited published inputs.
#[derive(Debug, Clone)]
pub struct QuantumSystematicsSpec {
    pub species: String,
    /// Published BBR fractional shift at 300 K (typically negative, e.g. Sr ≈ -2.35e-14).
    pub bbr_coeff_300k: f64,
    /// Operating temperature (K).
    pub temperature_k: f64,
    /// Published second-order Zeeman coefficient (fractional per Tesla²; ≤ 0).
    pub zeeman_coeff_per_t2: f64,
    /// Static magnetic field (Tesla).
    pub magnetic_field_t: f64,
    /// Height above the reference geoid (m) for the gravitational redshift.
    pub altitude_m: f64,
    /// Default per-effect relative (1σ) uncertainty applied to the computed shifts.
    pub rel_uncertainty: f64,
    /// Extra named effects supplied directly (name, fractional shift, abs uncertainty).
    pub extra: Vec<SystematicEffect>,
}

/// The computed budget.
#[derive(Debug, Clone)]
pub struct SystematicsBudget {
    pub species: String,
    pub effects: Vec<SystematicEffect>,
    /// Sum of the signed shifts (the net deterministic offset a clock must correct).
    pub total_shift: f64,
    /// Root-sum-square of the per-effect uncertainties (the residual floor after
    /// correcting the deterministic shifts).
    pub total_uncertainty: f64,
}

impl SystematicsBudget {
    /// Timing error that accumulates over `seconds` if the *net* shift is left
    /// uncorrected (|total_shift| · seconds).
    pub fn uncorrected_timing_error_s(&self, seconds: f64) -> f64 {
        self.total_shift.abs() * seconds
    }
    /// Residual timing error over `seconds` from the uncertainty floor after the
    /// deterministic shifts are corrected (total_uncertainty · seconds).
    pub fn residual_timing_error_s(&self, seconds: f64) -> f64 {
        self.total_uncertainty * seconds
    }
}

/// Compute the quantum-clock systematics budget from the spec.
pub fn quantum_systematics(
    spec: &QuantumSystematicsSpec,
) -> Result<SystematicsBudget, KshanaError> {
    if spec.temperature_k <= 0.0 || !spec.temperature_k.is_finite() {
        return Err(KshanaError::InvalidInput(
            "temperature_k must be a positive, finite value (Kelvin)".into(),
        ));
    }
    if spec.rel_uncertainty < 0.0 || !spec.rel_uncertainty.is_finite() {
        return Err(KshanaError::InvalidInput(
            "rel_uncertainty must be a non-negative, finite fraction".into(),
        ));
    }
    // Guard the remaining numeric inputs: a NaN/Inf here would silently propagate
    // into total_shift/total_uncertainty and serialise as JSON `null` (a NaN budget).
    for (name, v) in [
        ("bbr_coeff_300k", spec.bbr_coeff_300k),
        ("magnetic_field_t", spec.magnetic_field_t),
        ("altitude_m", spec.altitude_m),
    ] {
        if !v.is_finite() {
            return Err(KshanaError::InvalidInput(format!(
                "{name} must be a finite value"
            )));
        }
    }

    let mut effects = Vec::new();
    let ru = spec.rel_uncertainty;

    // Blackbody-radiation shift: scales as (T / 300 K)^4.
    let bbr = spec.bbr_coeff_300k * (spec.temperature_k / T_REF_K).powi(4);
    effects.push(SystematicEffect {
        name: "blackbody-radiation".into(),
        fractional_shift: bbr,
        uncertainty: bbr.abs() * ru,
    });

    // Gravitational redshift: g·h/c² (exact). Positive above the reference.
    let redshift = G_ACCEL * spec.altitude_m / (C_LIGHT * C_LIGHT);
    effects.push(SystematicEffect {
        name: "gravitational-redshift".into(),
        fractional_shift: redshift,
        // Dominated by the uncertainty in h; use the same relative figure.
        uncertainty: redshift.abs() * ru,
    });

    // Second-order Zeeman shift: scales as B².
    let zeeman = spec.zeeman_coeff_per_t2 * spec.magnetic_field_t * spec.magnetic_field_t;
    effects.push(SystematicEffect {
        name: "second-order-zeeman".into(),
        fractional_shift: zeeman,
        uncertainty: zeeman.abs() * ru,
    });

    // Any extra effects supplied directly (already carry their own uncertainty).
    for e in &spec.extra {
        if !e.fractional_shift.is_finite() || !e.uncertainty.is_finite() || e.uncertainty < 0.0 {
            return Err(KshanaError::InvalidInput(format!(
                "extra effect '{}' has a non-finite shift or invalid uncertainty",
                e.name
            )));
        }
        effects.push(e.clone());
    }

    let total_shift = effects.iter().map(|e| e.fractional_shift).sum();
    let total_uncertainty = effects
        .iter()
        .map(|e| e.uncertainty * e.uncertainty)
        .sum::<f64>()
        .sqrt();

    Ok(SystematicsBudget {
        species: spec.species.clone(),
        effects,
        total_shift,
        total_uncertainty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sr_spec() -> QuantumSystematicsSpec {
        // Representative Sr optical-lattice values (cited inputs, not Pro claims):
        // BBR ~ -2.35e-14 at 300 K; 2nd-order Zeeman coeff ~ -2.3e-2 /T² (small).
        QuantumSystematicsSpec {
            species: "Sr optical lattice (representative)".into(),
            bbr_coeff_300k: -2.35e-14,
            temperature_k: 300.0,
            zeeman_coeff_per_t2: -2.3e-2,
            magnetic_field_t: 1.0e-4, // 1 Gauss
            altitude_m: 0.0,
            rel_uncertainty: 0.02,
            extra: vec![],
        }
    }

    #[test]
    fn bbr_equals_coefficient_at_reference_temperature() {
        let b = quantum_systematics(&sr_spec()).unwrap();
        let bbr = b
            .effects
            .iter()
            .find(|e| e.name == "blackbody-radiation")
            .unwrap();
        assert!(
            (bbr.fractional_shift - (-2.35e-14)).abs() < 1e-28,
            "BBR at 300K = the coeff"
        );
    }

    #[test]
    fn bbr_scales_as_t_to_the_fourth() {
        let mut s = sr_spec();
        s.temperature_k = 600.0; // 2x temperature → 2^4 = 16x shift
        let b = quantum_systematics(&s).unwrap();
        let bbr = b
            .effects
            .iter()
            .find(|e| e.name == "blackbody-radiation")
            .unwrap();
        assert!((bbr.fractional_shift / (-2.35e-14) - 16.0).abs() < 1e-6);
    }

    #[test]
    fn gravitational_redshift_matches_g_h_over_c2() {
        let mut s = sr_spec();
        s.altitude_m = 1000.0;
        let b = quantum_systematics(&s).unwrap();
        let rs = b
            .effects
            .iter()
            .find(|e| e.name == "gravitational-redshift")
            .unwrap();
        let expected = 9.806_65 * 1000.0 / (299_792_458.0_f64).powi(2); // ~1.09e-13
        assert!((rs.fractional_shift - expected).abs() < 1e-20);
        assert!(rs.fractional_shift > 0.0, "higher clock ticks faster");
        assert!(
            (rs.fractional_shift - 1.09e-13).abs() < 0.02e-13,
            "~1.09e-13 at 1 km"
        );
    }

    #[test]
    fn zeeman_scales_as_b_squared() {
        let mut s = sr_spec();
        s.magnetic_field_t = 2.0e-4; // 2x field → 4x shift
        let b = quantum_systematics(&s).unwrap();
        let z = b
            .effects
            .iter()
            .find(|e| e.name == "second-order-zeeman")
            .unwrap();
        let base = -2.3e-2 * (1.0e-4_f64).powi(2);
        assert!((z.fractional_shift / base - 4.0).abs() < 1e-6);
    }

    #[test]
    fn total_is_sum_and_uncertainty_is_rss() {
        let mut s = sr_spec();
        s.altitude_m = 400_000.0; // LEO-ish: redshift dominates
        let b = quantum_systematics(&s).unwrap();
        let sum: f64 = b.effects.iter().map(|e| e.fractional_shift).sum();
        assert!((b.total_shift - sum).abs() < 1e-26);
        let rss = b
            .effects
            .iter()
            .map(|e| e.uncertainty.powi(2))
            .sum::<f64>()
            .sqrt();
        assert!((b.total_uncertainty - rss).abs() < 1e-26);
    }

    #[test]
    fn extra_effects_are_included() {
        let mut s = sr_spec();
        s.extra.push(SystematicEffect {
            name: "ac-stark-lattice".into(),
            fractional_shift: 5.0e-16,
            uncertainty: 1.0e-16,
        });
        let b = quantum_systematics(&s).unwrap();
        assert!(b.effects.iter().any(|e| e.name == "ac-stark-lattice"));
        assert_eq!(b.effects.len(), 4);
    }

    #[test]
    fn rejects_nonpositive_temperature() {
        let mut s = sr_spec();
        s.temperature_k = 0.0;
        assert!(matches!(
            quantum_systematics(&s),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn rejects_nonfinite_numeric_inputs() {
        let setters: [fn(&mut QuantumSystematicsSpec); 3] = [
            |s| s.magnetic_field_t = f64::NAN,
            |s| s.altitude_m = f64::INFINITY,
            |s| s.bbr_coeff_300k = f64::NAN,
        ];
        for set in setters {
            let mut s = sr_spec();
            set(&mut s);
            assert!(matches!(
                quantum_systematics(&s),
                Err(KshanaError::InvalidInput(_))
            ));
        }
    }

    #[test]
    fn timing_error_helpers() {
        let mut s = sr_spec();
        s.altitude_m = 1000.0;
        let b = quantum_systematics(&s).unwrap();
        let day = 86_400.0;
        assert!((b.uncorrected_timing_error_s(day) - b.total_shift.abs() * day).abs() < 1e-18);
        assert!(b.residual_timing_error_s(day) >= 0.0);
    }
}
