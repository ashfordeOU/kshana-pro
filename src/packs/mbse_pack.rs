// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-mbse` pack — exposes the L3 ECSS/MBSE artifact generator
//! ([`crate::mbse`]) as an [`ExternalPack`]: requirements → margins table +
//! traceability CSV + ReqIF-lite XML (DOORS/Polarion-importable).

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::mbse::{all_compliant, margins, to_csv, to_reqif, validate, Requirement};

/// A requirement entry in a `pro-mbse` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RequirementSpec {
    pub id: String,
    pub text: String,
    #[serde(default = "default_verification")]
    pub verification: String,
    pub target: f64,
    pub achieved: f64,
    #[serde(default)]
    pub lower_is_better: bool,
}

fn default_verification() -> String {
    "Analysis".to_string()
}

/// A `pro-mbse` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MbseScenario {
    pub title: String,
    #[serde(rename = "requirement")]
    pub requirements: Vec<RequirementSpec>,
}

impl MbseScenario {
    pub const KIND: &'static str = "pro-mbse";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro L3 assurance (PR16): generate ECSS/MBSE artifacts from a requirements set — margins table (target vs achieved, PASS/FAIL), traceability CSV, and ReqIF-lite XML for DOORS/Polarion import.",
            required_fields: &["title", "requirement"],
            optional_fields: &[],
        }
    }

    fn reqs(&self) -> Vec<Requirement> {
        self.requirements
            .iter()
            .map(|r| Requirement {
                id: r.id.clone(),
                text: r.text.clone(),
                verification: r.verification.clone(),
                target: r.target,
                achieved: r.achieved,
                lower_is_better: r.lower_is_better,
            })
            .collect()
    }
}

impl Scenario for MbseScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let reqs = self.reqs();
        validate(&reqs)?;
        let rows = margins(&reqs);
        let compliant = all_compliant(&rows);
        let n_pass = rows.iter().filter(|r| r.status == "PASS").count();

        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "title": self.title,
            "all_compliant": compliant,
            "n_requirements": rows.len(),
            "n_pass": n_pass,
            "margins": rows.iter().map(|m| serde_json::json!({
                "id": m.id, "target": m.target, "achieved": m.achieved,
                "margin": m.margin, "status": m.status,
            })).collect::<Vec<_>>(),
            "traceability_csv": to_csv(&reqs),
            "reqif_xml": to_reqif(&self.title, &reqs),
        })
        .to_string();

        let summary = format!(
            "Kshana Pro MBSE | {} | {}/{} requirements compliant → {}",
            self.title,
            n_pass,
            rows.len(),
            if compliant {
                "ALL PASS"
            } else {
                "NON-COMPLIANT"
            },
        );

        let svg = margins_svg(&self.title, &rows);
        Ok(RunOutput { json, svg, summary })
    }
}

impl ExternalPack for MbseScenario {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
}

/// A compliance bar per requirement (green PASS / red FAIL), margin % labelled.
fn margins_svg(title: &str, rows: &[crate::mbse::MarginRow]) -> String {
    let (w, ml, mr, mt, mb, row_h) = (560.0_f64, 110.0_f64, 80.0_f64, 50.0_f64, 16.0_f64, 26.0_f64);
    let n = rows.len().max(1);
    let h = mt + mb + n as f64 * row_h;
    let bar_w = w - ml - mr;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"system-ui,sans-serif\" font-size=\"11\">"
    );
    s += "<rect width=\"100%\" height=\"100%\" fill=\"#0c0b08\"/>";
    s += "<text x=\"16\" y=\"24\" fill=\"#c084fc\" font-size=\"14\" font-weight=\"bold\">Kshana Pro — requirements margins</text>";
    s += &format!(
        "<text x=\"16\" y=\"40\" fill=\"#8c8273\">{}</text>",
        xml_text(title)
    );
    for (i, m) in rows.iter().enumerate() {
        let y = mt + i as f64 * row_h;
        // Map margin [-1..+1] to a centred bar; clamp.
        let frac = m.margin.clamp(-1.0, 1.0);
        let half = bar_w / 2.0;
        let mid = ml + half;
        let (col, x, bw) = if frac >= 0.0 {
            ("#2dd4bf", mid, half * frac)
        } else {
            ("#f87171", mid + half * frac, half * (-frac))
        };
        s += &format!(
            "<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#cdd6e3\">{}</text>",
            ml - 8.0,
            y + 17.0,
            xml_text(&m.id)
        );
        s += &format!(
            "<line x1=\"{mid}\" y1=\"{mt}\" x2=\"{mid}\" y2=\"{:.1}\" stroke=\"#342c21\"/>",
            mt + n as f64 * row_h
        );
        s += &format!("<rect x=\"{x:.1}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"15\" fill=\"{col}\" rx=\"2\"/>", y + 6.0);
        s += &format!(
            "<text x=\"{}\" y=\"{:.1}\" fill=\"#8c8273\" font-size=\"10\">{:.0}% {}</text>",
            w - mr + 4.0,
            y + 17.0,
            m.margin * 100.0,
            m.status
        );
    }
    s += "</svg>";
    s
}

fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> MbseScenario {
        MbseScenario {
            title: "Demo SRD margins".into(),
            requirements: vec![
                RequirementSpec {
                    id: "SYS-001".into(),
                    text: "Payload mass <= 12 kg".into(),
                    verification: "Analysis".into(),
                    target: 12.0,
                    achieved: 10.5,
                    lower_is_better: true,
                },
                RequirementSpec {
                    id: "SYS-002".into(),
                    text: "PNT availability >= 0.99".into(),
                    verification: "Test".into(),
                    target: 0.99,
                    achieved: 0.995,
                    lower_is_better: false,
                },
            ],
        }
    }

    #[test]
    fn mbse_pack_generates_artifacts() {
        let out = pack().run().expect("runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["all_compliant"], true);
        assert_eq!(v["n_requirements"], 2);
        assert!(v["traceability_csv"].as_str().unwrap().contains("SYS-001"));
        assert!(v["reqif_xml"].as_str().unwrap().contains("<REQ-IF"));
        assert!(out.svg.contains("<svg"));
        assert!(out.summary.contains("ALL PASS"));
    }

    #[test]
    fn mbse_pack_flags_noncompliance() {
        let mut p = pack();
        p.requirements[0].achieved = 15.0; // over the 12 kg budget
        let out = p.run().unwrap();
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["all_compliant"], false);
        assert!(out.summary.contains("NON-COMPLIANT"));
    }

    #[test]
    fn external_contract() {
        assert_eq!(pack().kind_name(), "pro-mbse");
        assert_eq!(pack().meta().name, "pro-mbse");
    }
}
