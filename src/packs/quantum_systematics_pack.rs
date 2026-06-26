// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-quantum-systematics` pack — exposes the PR19 premium-fidelity
//! quantum-clock systematics budget ([`crate::premium`]) as an [`ExternalPack`].
//! It computes the deterministic systematic frequency shifts (BBR, gravitational
//! redshift, Zeeman, extras) and their uncertainty floor for an optical clock.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::premium::{quantum_systematics, QuantumSystematicsSpec, SystematicEffect};

fn default_rel_uncertainty() -> f64 {
    0.02
}
fn default_temperature() -> f64 {
    300.0
}

/// An extra named effect supplied in the scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExtraEffect {
    pub name: String,
    pub fractional_shift: f64,
    #[serde(default)]
    pub uncertainty: f64,
}

/// A `pro-quantum-systematics` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct QuantumSystematicsScenario {
    pub species: String,
    /// Published BBR fractional shift at 300 K (cited; typically negative).
    pub bbr_coeff_300k: f64,
    #[serde(default = "default_temperature")]
    pub temperature_k: f64,
    #[serde(default)]
    pub zeeman_coeff_per_t2: f64,
    #[serde(default)]
    pub magnetic_field_t: f64,
    #[serde(default)]
    pub altitude_m: f64,
    #[serde(default = "default_rel_uncertainty")]
    pub rel_uncertainty: f64,
    #[serde(rename = "extra", default)]
    pub extras: Vec<ExtraEffect>,
}

impl QuantumSystematicsScenario {
    pub const KIND: &'static str = "pro-quantum-systematics";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro premium fidelity (PR19): full-physics quantum-clock systematics budget (BBR T^4, gravitational redshift g·h/c², 2nd-order Zeeman B², extras) from cited coefficients, with an uncertainty floor. The deterministic 'full-physics' upgrade over the open core's coefficient-level clock.",
            required_fields: &["species", "bbr_coeff_300k"],
            optional_fields: &["temperature_k", "zeeman_coeff_per_t2", "magnetic_field_t", "altitude_m", "rel_uncertainty", "extra"],
        }
    }

    fn spec(&self) -> QuantumSystematicsSpec {
        QuantumSystematicsSpec {
            species: self.species.clone(),
            bbr_coeff_300k: self.bbr_coeff_300k,
            temperature_k: self.temperature_k,
            zeeman_coeff_per_t2: self.zeeman_coeff_per_t2,
            magnetic_field_t: self.magnetic_field_t,
            altitude_m: self.altitude_m,
            rel_uncertainty: self.rel_uncertainty,
            extra: self
                .extras
                .iter()
                .map(|e| SystematicEffect {
                    name: e.name.clone(),
                    fractional_shift: e.fractional_shift,
                    uncertainty: e.uncertainty,
                })
                .collect(),
        }
    }
}

impl Scenario for QuantumSystematicsScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let budget = quantum_systematics(&self.spec())?;
        let day = 86_400.0;

        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "species": budget.species,
            "temperature_k": self.temperature_k,
            "effects": budget.effects.iter().map(|e| serde_json::json!({
                "name": e.name,
                "fractional_shift": e.fractional_shift,
                "uncertainty": e.uncertainty,
            })).collect::<Vec<_>>(),
            "total_shift": budget.total_shift,
            "total_uncertainty": budget.total_uncertainty,
            // Signed drift gives the correction direction; the *_error_* fields are magnitudes.
            "signed_timing_drift_s_per_day": budget.total_shift * day,
            "uncorrected_timing_error_magnitude_s_per_day": budget.uncorrected_timing_error_s(day),
            "residual_timing_error_s_per_day": budget.residual_timing_error_s(day),
        })
        .to_string();

        let summary = format!(
            "Kshana Pro quantum systematics | {} | {} effects | net shift {:.3e} ± {:.3e} | uncorrected {:.3e} s/day, residual floor {:.3e} s/day",
            budget.species,
            budget.effects.len(),
            budget.total_shift,
            budget.total_uncertainty,
            budget.uncorrected_timing_error_s(day),
            budget.residual_timing_error_s(day),
        );

        let svg = budget_svg(&budget);
        Ok(RunOutput { json, svg, summary })
    }
}

impl ExternalPack for QuantumSystematicsScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
}

/// A horizontal bar chart of |fractional shift| per effect (log-magnitude).
fn budget_svg(b: &crate::premium::SystematicsBudget) -> String {
    let (w, ml, mr, mt, mb, row_h) = (560.0_f64, 168.0_f64, 90.0_f64, 50.0_f64, 20.0_f64, 28.0_f64);
    let n = b.effects.len().max(1);
    let h = mt + mb + n as f64 * row_h;
    let lg = |x: f64| x.abs().max(1e-300).log10();
    let mags: Vec<f64> = b.effects.iter().map(|e| lg(e.fractional_shift)).collect();
    let lo = mags
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min)
        .min(-18.0);
    let hi = mags
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(lo + 1.0);
    let bar_w = w - ml - mr;

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"system-ui,sans-serif\" font-size=\"11\">"
    );
    s += "<rect width=\"100%\" height=\"100%\" fill=\"#0c0b08\"/>";
    s += "<text x=\"16\" y=\"24\" fill=\"#c084fc\" font-size=\"14\" font-weight=\"bold\">Kshana Pro — quantum-clock systematics</text>";
    s += &format!(
        "<text x=\"16\" y=\"40\" fill=\"#8c8273\">{} · |Δν/ν| per effect (log)</text>",
        esc(&b.species)
    );
    for (i, e) in b.effects.iter().enumerate() {
        let y = mt + i as f64 * row_h;
        // hi-lo is already >= 1.0 (log10 decades) via the lo/hi guards above.
        let frac = ((lg(e.fractional_shift) - lo) / (hi - lo).max(1.0)).clamp(0.02, 1.0);
        let bw = bar_w * frac;
        let col = if e.fractional_shift < 0.0 {
            "#d2925e"
        } else {
            "#7fb3c8"
        };
        s += &format!(
            "<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#cdd6e3\">{}</text>",
            ml - 8.0,
            y + 18.0,
            esc(&e.name)
        );
        s += &format!("<rect x=\"{ml}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"16\" fill=\"{col}\" rx=\"3\"/>", y + 7.0);
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#8c8273\" font-size=\"10\">{:.2e}</text>",
            ml + bw + 6.0,
            y + 19.0,
            e.fractional_shift
        );
    }
    s += &format!(
        "<text x=\"16\" y=\"{}\" fill=\"#2dd4bf\">net {:.2e} ± {:.1e}</text>",
        h - 6.0,
        b.total_shift,
        b.total_uncertainty
    );
    s += "</svg>";
    s
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_runs_and_reports_budget() {
        let pack = QuantumSystematicsScenario {
            species: "Sr (representative)".into(),
            bbr_coeff_300k: -5.5e-15,
            temperature_k: 300.0,
            zeeman_coeff_per_t2: -2.3e-2,
            magnetic_field_t: 1.0e-4,
            altitude_m: 400_000.0,
            rel_uncertainty: 0.02,
            extras: vec![ExtraEffect {
                name: "ac-stark".into(),
                fractional_shift: 5.0e-16,
                uncertainty: 1.0e-16,
            }],
        };
        let out = pack.run().expect("runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["effects"].as_array().unwrap().len(), 4);
        assert!(v["total_uncertainty"].as_f64().unwrap() > 0.0);
        assert!(out.svg.contains("<svg"));
        assert!(out.summary.contains("net shift"));
    }

    #[test]
    fn external_contract() {
        let pack = QuantumSystematicsScenario {
            species: "x".into(),
            bbr_coeff_300k: -1e-14,
            temperature_k: 300.0,
            zeeman_coeff_per_t2: 0.0,
            magnetic_field_t: 0.0,
            altitude_m: 0.0,
            rel_uncertainty: 0.02,
            extras: vec![],
        };
        assert_eq!(pack.kind_name(), "pro-quantum-systematics");
        assert_eq!(pack.meta().name, "pro-quantum-systematics");
    }
}
