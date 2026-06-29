// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-trade-study` pack — exposes the L3 trade-study & assurance framework
//! ([`crate::suite`]) as an out-of-tree [`ExternalPack`]. It runs each architecture
//! variant through the embedded open engine, ranks them on a chosen figure of merit,
//! and emits an audit-grade evidence pack (with reproducible scenario hashes).

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::suite::{run_with_evidence, TradeSpace, Variant};

/// A variant entry in a `pro-trade-study` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VariantSpec {
    pub label: String,
    /// A full open-engine scenario, inline as a TOML string.
    pub scenario: String,
}

/// A `pro-trade-study` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TradeStudyScenario {
    pub title: String,
    /// Dot-path into each variant's result JSON, e.g. `quantum.fom.holdover_s`.
    pub metric: String,
    #[serde(default)]
    pub lower_better: bool,
    #[serde(rename = "variant")]
    pub variants: Vec<VariantSpec>,
}

impl TradeStudyScenario {
    pub const KIND: &'static str = "pro-trade-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro L3 demonstrator: run architecture variants through the open engine, rank them on a figure of merit, and emit an audit-grade evidence pack with reproducible scenario hashes. Built on open capabilities; the licensed Pro SKU (full ECSS/MBSE) is demand-gated.",
            required_fields: &["title", "metric", "variant"],
            optional_fields: &["lower_better"],
        }
    }

    fn trade_space(&self) -> TradeSpace {
        TradeSpace {
            title: self.title.clone(),
            metric: self.metric.clone(),
            lower_better: self.lower_better,
            variants: self
                .variants
                .iter()
                .map(|v| Variant {
                    label: v.label.clone(),
                    scenario_toml: v.scenario.clone(),
                })
                .collect(),
        }
    }
}

impl Scenario for TradeStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let space = self.trade_space();
        let (report, evidence_md) = run_with_evidence(&space)?;

        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "title": report.title,
            "metric": report.metric,
            "lower_better": report.lower_better,
            "winner": report.winner,
            "recommendation": report.recommendation,
            "variants": report.outcomes.iter().map(|o| serde_json::json!({
                "label": o.label,
                "rank": o.rank,
                "metric_value": o.metric_value,
                "scenario_hash": o.scenario_hash,
                "error": o.error,
            })).collect::<Vec<_>>(),
            "evidence_pack_markdown": evidence_md,
        })
        .to_string();

        let ranked = report.outcomes.iter().filter(|o| o.rank.is_some()).count();
        let summary = format!(
            "Kshana Pro L3 trade study | {} | metric '{}' ({}) | {} variant(s), {} ranked | winner: {} | {}",
            report.title,
            report.metric,
            if report.lower_better { "lower better" } else { "higher better" },
            report.outcomes.len(),
            ranked,
            report.winner.as_deref().unwrap_or("none"),
            report.recommendation,
        );

        let svg = trade_svg(&report);
        Ok(RunOutput { json, svg, summary })
    }
}

impl ExternalPack for TradeStudyScenario {
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

/// A small ranked bar chart of the metric across variants.
fn trade_svg(report: &crate::suite::TradeReport) -> String {
    let (w, mt, mb, ml, mr, row_h) = (560.0_f64, 50.0_f64, 24.0_f64, 150.0_f64, 70.0_f64, 30.0_f64);
    let ranked: Vec<&crate::suite::VariantOutcome> = {
        let mut v: Vec<_> = report
            .outcomes
            .iter()
            .filter(|o| o.metric_value.is_some())
            .collect();
        v.sort_by_key(|o| o.rank.unwrap_or(usize::MAX));
        v
    };
    let h = mt + mb + (ranked.len().max(1) as f64) * row_h;
    let vmax = ranked
        .iter()
        .map(|o| o.metric_value.unwrap().abs())
        .fold(0.0_f64, f64::max)
        .max(1e-300);
    let bar_w = w - ml - mr;

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" font-family=\"system-ui,sans-serif\" font-size=\"11\">"
    );
    s += "<rect width=\"100%\" height=\"100%\" fill=\"#0c0b08\"/>";
    s += "<text x=\"16\" y=\"24\" fill=\"#c084fc\" font-size=\"14\" font-weight=\"bold\">Kshana Pro L3 trade study</text>";
    s += &format!(
        "<text x=\"16\" y=\"40\" fill=\"#8c8273\">{} · metric: {}</text>",
        esc(&report.title),
        esc(&report.metric)
    );
    for (i, o) in ranked.iter().enumerate() {
        let y = mt + i as f64 * row_h;
        let frac = (o.metric_value.unwrap().abs() / vmax).clamp(0.0, 1.0);
        let bw = (bar_w * frac).max(1.0);
        let is_winner = o.rank == Some(1);
        let col = if is_winner { "#2dd4bf" } else { "#6f6450" };
        s += &format!(
            "<text x=\"{}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"#cdd6e3\">{}{}</text>",
            ml - 8.0,
            y + 18.0,
            if is_winner { "★ " } else { "" },
            esc(&o.label)
        );
        s += &format!("<rect x=\"{ml}\" y=\"{:.1}\" width=\"{bw:.1}\" height=\"18\" fill=\"{col}\" rx=\"3\"/>", y + 6.0);
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#8c8273\">{:.4}</text>",
            ml + bw + 6.0,
            y + 19.0,
            o.metric_value.unwrap()
        );
    }
    if ranked.is_empty() {
        s += &format!(
            "<text x=\"16\" y=\"{}\" fill=\"#f87171\">no comparable metric across variants</text>",
            mt + 20.0
        );
    }
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

    fn clock_variant(quantum_id: &str, q_wf: f64) -> String {
        format!(
            "kind = \"clock\"\n\
             seed = 42\n\
             threshold_ns = 20.0\n\
             [time]\n\
             step_s = 10.0\n\
             duration_s = 3600.0\n\
             [gnss]\n\
             windows = [\n\
             {{ t0 = 0.0, t1 = 600.0, state = \"nominal\" }},\n\
             {{ t0 = 600.0, t1 = 3600.0, state = \"denied\" }},\n\
             ]\n\
             [clock_quantum]\n\
             id = \"{quantum_id}\"\n\
             provenance = \"trade variant\"\n\
             y0 = 0.0\n\
             q_wf = {q_wf}\n\
             q_rw = 0.0\n\
             [clock_classical]\n\
             id = \"csac\"\n\
             provenance = \"trade variant\"\n\
             y0 = 0.0\n\
             q_wf = 9.0e-20\n\
             q_rw = 0.0\n"
        )
    }

    #[test]
    fn quantum_vs_classical_clock_trade_picks_better_clock() {
        // On-thesis: optical (tiny q_wf) should hold timing far longer than CSAC.
        let pack = TradeStudyScenario {
            title: "quantum vs classical clock holdover".into(),
            metric: "quantum.fom.holdover_s".into(),
            lower_better: false,
            variants: vec![
                VariantSpec {
                    label: "optical-lattice".into(),
                    scenario: clock_variant("optical", 1.0e-30),
                },
                VariantSpec {
                    label: "csac".into(),
                    scenario: clock_variant("csac", 9.0e-20),
                },
            ],
        };
        let out = pack.run().expect("trade study runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["winner"], "optical-lattice", "summary: {}", out.summary);
        assert!(out.json.contains("evidence_pack_markdown"));
        assert!(out.svg.contains("<svg"));
    }

    #[test]
    fn trade_study_external_contract() {
        let pack = TradeStudyScenario {
            title: "t".into(),
            metric: "n_samples".into(),
            lower_better: false,
            variants: vec![],
        };
        assert_eq!(pack.kind_name(), "pro-trade-study");
        assert_eq!(pack.meta().name, "pro-trade-study");
    }
}
