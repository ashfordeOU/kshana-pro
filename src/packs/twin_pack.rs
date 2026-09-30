// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-twin` pack — exposes the L1 twin-calibration framework
//! ([`crate::twin`]) as an out-of-tree [`ExternalPack`]. It calibrates a clock twin
//! to a published device's Allan-deviation budget and reports how well the twin
//! reproduces it (validated with the open-core Allan estimator).

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::twin::{calibrate, calibrate3, validate, DeviceSpec};

/// The honesty label every `pro-twin` result carries: a twin reproduces a published budget
/// with the open clock model, so it is MODELLED, never a characterisation of a real unit.
pub const TWIN_LABEL: &str = "MODELLED - a clock digital twin: the open engine's clock model fitted to the device's published Allan-deviation points and checked with the open engine's overlapping Allan-deviation estimator within the stated tolerance. It reproduces a published budget; it is not a measurement or a characterisation of any real unit.";

fn default_tolerance() -> f64 {
    0.25
}
fn default_tau0() -> f64 {
    1.0
}
fn default_n_samples() -> usize {
    200_000
}
fn default_seed() -> u64 {
    7
}

/// A `pro-twin` scenario: a published device characterisation to calibrate against.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TwinScenario {
    pub device: String,
    #[serde(default)]
    pub provenance: Option<String>,
    /// Published `[tau_s, adev]` pairs (>= 2).
    pub adev_points: Vec<[f64; 2]>,
    #[serde(default)]
    pub drift_per_s: f64,
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
    #[serde(default = "default_tau0")]
    pub tau0_s: f64,
    #[serde(default = "default_n_samples")]
    pub n_samples: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
}

impl TwinScenario {
    pub const KIND: &'static str = "pro-twin";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro L1 demonstrator: calibrate a clock digital twin to a published device Allan-deviation budget and validate it reproduces the curve (open-core Allan estimator). Framework runs on published data; a real customer-device twin is NDA/contract-gated.",
            required_fields: &["device", "adev_points"],
            optional_fields: &["provenance", "drift_per_s", "tolerance", "tau0_s", "n_samples", "seed"],
        }
    }

    fn device_spec(&self) -> DeviceSpec {
        DeviceSpec {
            device: self.device.clone(),
            provenance: self
                .provenance
                .clone()
                .unwrap_or_else(|| self.device.clone()),
            adev_points: self.adev_points.iter().map(|p| (p[0], p[1])).collect(),
            drift_per_s: self.drift_per_s,
        }
    }
}

impl Scenario for TwinScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let spec = self.device_spec();
        // Use the 3-parameter fit (white-FM + flicker + long-τ random-walk-FM) when
        // there are enough points to resolve the third basis function; fall back to
        // the 2-parameter fit for a 2-point budget. Both are open-core ClockModels.
        let twin = if spec.adev_points.len() >= 3 {
            calibrate3(&spec)?
        } else {
            calibrate(&spec)?
        };
        let report = validate(
            &twin,
            &spec,
            self.tolerance,
            self.tau0_s,
            self.n_samples,
            self.seed,
        )?;

        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "device": self.device,
            "provenance": spec.provenance,
            "label": TWIN_LABEL,
            "calibration": {
                "q_wf": twin.q_wf,
                "flicker_floor": twin.flicker_floor,
                "q_rw": twin.q_rw,
                "drift_per_s": twin.drift_per_s,
                "params": if twin.q_rw > 0.0 { 3 } else { 2 },
                "model": "kshana::models::ClockModel (open-core ErrorModel)",
            },
            "validation": {
                "estimator": "kshana::allan::overlapping_adev_curve",
                "tolerance": report.tolerance,
                "max_rel_err": report.max_rel_err,
                "passed": report.passed,
                "points": report.points.iter().map(|p| serde_json::json!({
                    "tau_s": p.tau_s,
                    "published_adev": p.published_adev,
                    "twin_adev": p.twin_adev,
                    "rel_err": p.rel_err,
                })).collect::<Vec<_>>(),
            },
        })
        .to_string();

        let summary = format!(
            "Kshana Pro L1 twin | device: {} | calibrated q_wf={:.3e} flicker_floor={:.3e} q_rw={:.3e} | validation vs open-core Allan: max rel err {:.1}% (tol {:.0}%) => {} | MODELLED",
            self.device,
            twin.q_wf,
            twin.flicker_floor,
            twin.q_rw,
            report.max_rel_err * 100.0,
            report.tolerance * 100.0,
            if report.passed { "PASS" } else { "FAIL" },
        );

        let svg = adev_svg(&self.device, &report);
        Ok(RunOutput {
            json,
            svg,
            summary,
            csv: None,
        })
    }
}

impl ExternalPack for TwinScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        // Drive the open registry seam: add THIS pack's id→factory binding so the
        // superset `pro_registry()` can build it by id. Mirrors `kind_name`/`meta`.
        reg.register(crate::registry::pro_factory::<Self>(Self::KIND));
    }
}

/// A small log-log Allan-deviation chart: published points vs the calibrated twin.
fn adev_svg(device: &str, report: &crate::twin::ValidationReport) -> String {
    let (w, h, ml, mr, mt, mb) = (560.0_f64, 360.0_f64, 64.0_f64, 18.0_f64, 46.0_f64, 52.0_f64);
    // Log10 ranges over both published and twin values.
    let mut vals: Vec<f64> = Vec::new();
    let mut taus: Vec<f64> = Vec::new();
    for p in &report.points {
        taus.push(p.tau_s);
        vals.push(p.published_adev);
        vals.push(p.twin_adev);
    }
    let lg = |x: f64| x.max(1e-300).log10();
    let (tmin, tmax) = (
        taus.iter().cloned().fold(f64::INFINITY, f64::min),
        taus.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let (vmin, vmax) = (
        vals.iter().cloned().fold(f64::INFINITY, f64::min),
        vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let (lxmin, lxmax) = (lg(tmin), lg(tmax).max(lg(tmin) + 1.0));
    let (lymin, lymax) = (lg(vmin) - 0.2, lg(vmax) + 0.2);
    let px = |t: f64| ml + (lg(t) - lxmin) / (lxmax - lxmin).max(1e-9) * (w - ml - mr);
    let py = |v: f64| mt + (1.0 - (lg(v) - lymin) / (lymax - lymin).max(1e-9)) * (h - mt - mb);

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"system-ui,sans-serif\" font-size=\"11\">"
    );
    s += "<rect width=\"100%\" height=\"100%\" fill=\"#0c0b08\"/>";
    s += &format!("<text x=\"{ml}\" y=\"24\" fill=\"#c084fc\" font-size=\"14\" font-weight=\"bold\">Kshana Pro L1 twin — Allan deviation: {device}</text>");
    s += &format!("<text x=\"{ml}\" y=\"40\" fill=\"#8c8273\">published (○) vs calibrated twin (●) · open-core estimator</text>");
    // Axes.
    let ax = h - mb;
    s += &format!("<line x1=\"{ml}\" y1=\"{mt}\" x2=\"{ml}\" y2=\"{ax}\" stroke=\"#342c21\"/>");
    s += &format!(
        "<line x1=\"{ml}\" y1=\"{ax}\" x2=\"{}\" y2=\"{ax}\" stroke=\"#342c21\"/>",
        w - mr
    );
    s += &format!("<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"#8c8273\">averaging time τ (s, log)</text>", ml + (w - ml - mr) / 2.0, h - 16.0);
    // Twin polyline.
    let twin_poly: String = report
        .points
        .iter()
        .map(|p| format!("{:.1},{:.1}", px(p.tau_s), py(p.twin_adev)))
        .collect::<Vec<_>>()
        .join(" ");
    s += &format!(
        "<polyline points=\"{twin_poly}\" fill=\"none\" stroke=\"#c084fc\" stroke-width=\"2\"/>"
    );
    for p in &report.points {
        // Published: hollow amber circle. Twin: filled violet dot.
        s += &format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"none\" stroke=\"#eab308\" stroke-width=\"2\"/>", px(p.tau_s), py(p.published_adev));
        s += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3.2\" fill=\"#c084fc\"/>",
            px(p.tau_s),
            py(p.twin_adev)
        );
    }
    let verdict = if report.passed { "PASS" } else { "FAIL" };
    let vcol = if report.passed { "#2dd4bf" } else { "#f87171" };
    s += &format!("<text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"{vcol}\" font-weight=\"bold\">{verdict} · max err {:.0}%</text>", w - mr, mt + 14.0, report.max_rel_err * 100.0);
    s += "</svg>";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twin_pack_runs_and_passes_on_representative_csac() {
        let pack = TwinScenario {
            device: "CSAC (representative)".into(),
            provenance: Some("representative CSAC-class published ADEV".into()),
            adev_points: vec![[1.0, 3.0e-10], [10.0, 1.0e-10], [100.0, 3.2e-11]],
            drift_per_s: 0.0,
            tolerance: 0.25,
            tau0_s: 1.0,
            n_samples: 200_000,
            seed: 7,
        };
        let out = pack.run().expect("twin pack runs");
        assert!(out.summary.contains("PASS"), "summary: {}", out.summary);
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["validation"]["passed"], true);
        assert!(out.svg.contains("<svg"));
        // Every Pro output is labelled MODELLED (docs/PRO.md of the open repository says so).
        let label = v["label"]
            .as_str()
            .expect("the twin result carries a label");
        assert!(label.starts_with("MODELLED"), "label: {label}");
        assert!(
            out.summary.ends_with("| MODELLED"),
            "summary: {}",
            out.summary
        );
    }

    #[test]
    fn twin_pack_external_contract() {
        let pack = TwinScenario {
            device: "x".into(),
            provenance: None,
            adev_points: vec![[1.0, 1e-10], [10.0, 3e-11]],
            drift_per_s: 0.0,
            tolerance: 0.25,
            tau0_s: 1.0,
            n_samples: 1024,
            seed: 1,
        };
        assert_eq!(pack.kind_name(), "pro-twin");
        assert_eq!(pack.meta().name, "pro-twin");
    }
}
