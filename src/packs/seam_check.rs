// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The **seam-check** proof pack — the P0 architecture proof.
//!
//! This is intentionally *not* a product capability. It is the smallest real
//! [`ExternalPack`] that exercises the whole overlay contract end-to-end:
//!
//!  1. It is defined **out-of-tree** (here, in the private Pro crate), proving Pro
//!     can add a pack without forking the open core.
//!  2. Its [`run`](Scenario::run) calls into the **embedded open engine**
//!     ([`kshana::api::list_scenario_kinds`]), proving the dependency link is live
//!     and the open engine really is shipped inside the Pro build.
//!  3. It returns a valid [`RunOutput`] through the same `{ json, svg, summary }`
//!     envelope every open pack uses, proving the surface is uniform.
//!
//! If this pack runs green, the superset architecture is proven; the value layers
//! attach the same way.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

/// A `pro-seam-check` scenario. The only field is an optional free-text label,
/// echoed back so a caller can confirm round-tripping of pack-specific input.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct SeamCheckPack {
    /// Optional label echoed into the result (proves pack-specific fields parse).
    #[serde(default)]
    pub label: Option<String>,
}

impl SeamCheckPack {
    /// The kind string this pack answers to.
    pub const KIND: &'static str = "pro-seam-check";

    /// Static metadata, usable without an instance (the registry lists packs
    /// before any scenario is parsed).
    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro overlay seam check: confirms the private overlay embeds and can call the open Kshana engine (P0 architecture proof; not a product capability).",
            required_fields: &[],
            optional_fields: &["label"],
        }
    }
}

impl Scenario for SeamCheckPack {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        // Reach into the embedded open engine: enumerate every scenario kind the
        // open core ships. A non-empty list proves Pro is built on top of a live
        // open engine — the heart of the superset relationship.
        let open_kinds: Vec<&'static str> = kshana::api::list_scenario_kinds()
            .iter()
            .map(|m| m.name)
            .collect();
        if open_kinds.is_empty() {
            // A build/linkage integrity failure (the embedded engine is broken),
            // not a solver convergence failure — surface it as Unsupported so a
            // binding caller does not mistake it for a retryable numeric issue.
            return Err(KshanaError::Unsupported(
                "embedded open engine is unavailable (zero scenario kinds; overlay build may be broken)".into(),
            ));
        }
        let pro_kinds: Vec<&'static str> = crate::registry::pro_kinds()
            .iter()
            .map(|m| m.name)
            .collect();

        let label = self.label.clone().unwrap_or_default();
        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "open_engine_kinds": open_kinds.len(),
            "open_engine_kind_names": open_kinds,
            "pro_kinds": pro_kinds,
            "label": label,
            "seam_ok": true,
        })
        .to_string();

        let summary = format!(
            "Kshana Pro {} overlay live | embeds open engine with {} scenario kinds | {} Pro pack(s) registered | seam check OK{}",
            crate::version(),
            open_kinds.len(),
            pro_kinds.len(),
            if label.is_empty() { String::new() } else { format!(" | label: {label}") },
        );

        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"96\" font-family=\"system-ui,sans-serif\">\
             <rect width=\"480\" height=\"96\" fill=\"#0c0b08\"/>\
             <text x=\"16\" y=\"36\" fill=\"#c084fc\" font-size=\"16\" font-weight=\"bold\">Kshana Pro · seam check OK</text>\
             <text x=\"16\" y=\"62\" fill=\"#8c8273\" font-size=\"12\">embeds open engine · {} scenario kinds inherited</text>\
             <text x=\"16\" y=\"82\" fill=\"#62594b\" font-size=\"11\">overlay v{}</text>\
             </svg>",
            open_kinds.len(),
            crate::version(),
        );

        Ok(RunOutput { json, svg, summary })
    }
}

impl ExternalPack for SeamCheckPack {
    fn kind_name(&self) -> &'static str {
        Self::KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seam_check_runs_and_sees_the_open_engine() {
        let pack = SeamCheckPack::default();
        let out = pack.run().expect("seam check must run");
        // It must report a positive count of open-engine scenario kinds — the
        // proof that the open engine is embedded and callable from the overlay.
        assert!(out.summary.contains("seam check OK"));
        assert!(out.json.contains("\"seam_ok\":true"));
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert!(v["open_engine_kinds"].as_u64().unwrap() >= 15);
        assert!(!out.svg.is_empty());
    }

    #[test]
    fn seam_check_echoes_its_label() {
        let pack = SeamCheckPack {
            label: Some("bid-demo".into()),
        };
        let out = pack.run().unwrap();
        assert!(out.summary.contains("bid-demo"));
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["label"], "bid-demo");
    }

    #[test]
    fn external_pack_contract_is_satisfied() {
        let pack = SeamCheckPack::default();
        assert_eq!(pack.kind_name(), "pro-seam-check");
        assert_eq!(pack.meta().name, "pro-seam-check");
    }
}
