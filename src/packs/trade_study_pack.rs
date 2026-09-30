// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-trade-study` pack — exposes the L3 trade-study & assurance framework
//! ([`crate::suite`]) as an out-of-tree [`ExternalPack`]. It runs each architecture
//! variant through the embedded open engine and either ranks them on a single figure
//! of merit (back-compatible) or, when `criteria` are supplied, runs a full
//! multi-criteria decision (WSM + optional AHP weights + Pareto + sensitivity) and
//! emits the assurance layer: a W3C-PROV evidence pack with a Merkle root, an ECSS
//! Annex L / NASA §6.8 trade report, a GSN argument, an ECSS Verification Control
//! Document, an ISO 16290 TRL table, and a fail-closed assurance gate.

use std::collections::BTreeMap;

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::mcda::wsm::Constraint;

/// The honesty label every `pro-trade-study` result carries, in the open engine's `label`
/// form. A variant's own `label` field (inside `variants`) is its name, not this.
pub const TRADE_LABEL: &str = "MODELLED - a trade study over open-engine simulation runs: every variant's figures are simulation output of the open engine, and the ranking, decision and assurance artefacts are computed from those figures and the stated weights. It is an engineering study, not a certification.";

use crate::suite::decision::{TradeCriterion, Weighting};
use crate::suite::{
    evidence_pack_markdown, gate, gsn, reports, run_trade_study, vcd, GateThresholds, TradeReport,
    TradeSpace, TrlLevel, Variant,
};

/// A variant entry in a `pro-trade-study` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VariantSpec {
    pub label: String,
    /// A full open-engine scenario, inline as a TOML string.
    pub scenario: String,
}

/// One decision criterion in a multi-criteria `pro-trade-study` scenario.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CriterionSpec {
    pub name: String,
    /// Dot-path into each variant's result JSON for this criterion's raw value.
    pub metric: String,
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// `true` ⇒ cost (lower is better); `false` ⇒ benefit (higher is better).
    #[serde(default)]
    pub lower_better: bool,
    /// Optional knock-out: minimum acceptable raw value.
    #[serde(default)]
    pub min: Option<f64>,
    /// Optional knock-out: maximum acceptable raw value.
    #[serde(default)]
    pub max: Option<f64>,
}

fn default_weight() -> f64 {
    1.0
}

/// Optional AHP weighting block.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AhpSpec {
    /// Row-major reciprocal pairwise-comparison matrix (Saaty 1–9 scale).
    pub pairwise: Vec<Vec<f64>>,
    #[serde(default = "default_cr")]
    pub cr_threshold: f64,
}

fn default_cr() -> f64 {
    0.10
}

/// A `pro-trade-study` scenario.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct TradeStudyScenario {
    pub title: String,
    /// Single figure of merit (dot-path). Used only when `criterion` is empty.
    #[serde(default)]
    pub metric: String,
    #[serde(default)]
    pub lower_better: bool,
    #[serde(rename = "variant")]
    pub variants: Vec<VariantSpec>,
    /// Multi-criteria decision criteria. Empty ⇒ single-FoM mode.
    #[serde(rename = "criterion", default)]
    pub criteria: Vec<CriterionSpec>,
    /// Optional AHP weighting (overrides the per-criterion weights).
    #[serde(default)]
    pub ahp: Option<AhpSpec>,
    /// Per-variant ISO 16290 TRL declarations, keyed by variant label.
    #[serde(default)]
    pub trl: BTreeMap<String, u8>,
    /// Optional TRL floor for the assurance gate (representative default 6).
    #[serde(default)]
    pub min_trl: Option<u8>,
}

impl TradeStudyScenario {
    pub const KIND: &'static str = "pro-trade-study";

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro L3: run architecture variants through the open engine, then either rank on a figure of merit or run a full multi-criteria decision (WSM + AHP weights + Pareto + sensitivity) and emit the assurance layer — W3C-PROV evidence pack with a Merkle root, ECSS Annex L / NASA §6.8 trade report, GSN argument, ECSS Verification Control Document, ISO 16290 TRL table, and a fail-closed gate. All Modelled; standards implemented structurally. The licensed Pro SKU (full ECSS/MBSE) is demand-gated.",
            required_fields: &["title", "variant"],
            optional_fields: &["metric", "lower_better", "criterion", "ahp", "trl", "min_trl"],
        }
    }

    /// The [`TradeSpace`] this scenario describes (shared with the study dossier).
    pub fn trade_space(&self) -> TradeSpace {
        let variants: Vec<Variant> = self
            .variants
            .iter()
            .map(|v| Variant {
                label: v.label.clone(),
                scenario_toml: v.scenario.clone(),
            })
            .collect();

        if self.criteria.is_empty() {
            // Single figure of merit (back-compatible).
            return self.apply_common(TradeSpace::single_fom(
                self.title.clone(),
                self.metric.clone(),
                self.lower_better,
                variants,
            ));
        }

        // Multi-criteria decision.
        let criteria: Vec<TradeCriterion> = self
            .criteria
            .iter()
            .map(|c| {
                let mut tc = if c.lower_better {
                    TradeCriterion::cost(c.name.clone(), c.metric.clone(), c.weight)
                } else {
                    TradeCriterion::benefit(c.name.clone(), c.metric.clone(), c.weight)
                };
                if c.min.is_some() || c.max.is_some() {
                    tc = tc.with_constraint(Constraint {
                        min: c.min,
                        max: c.max,
                    });
                }
                tc
            })
            .collect();

        let mut space = TradeSpace::multi_criteria(self.title.clone(), variants, criteria);
        if let Some(ahp) = &self.ahp {
            space = space.with_weighting(Weighting::Ahp {
                pairwise: ahp.pairwise.clone(),
                cr_threshold: ahp.cr_threshold,
            });
        }
        self.apply_common(space)
    }

    /// Apply the TRL declarations and gate thresholds shared by both modes.
    fn apply_common(&self, mut space: TradeSpace) -> TradeSpace {
        for (label, &lvl) in &self.trl {
            space.trl.insert(label.clone(), TrlLevel::new(lvl));
        }
        let mut gate = GateThresholds::default();
        if let Some(t) = self.min_trl {
            gate.min_trl = t;
        }
        space.gate = gate;
        space
    }
}

impl Scenario for TradeStudyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let space = self.trade_space();
        let report = run_trade_study(&space)?;
        let evidence_md = evidence_pack_markdown(&report);

        let min_trl = report.gate.min_trl;

        // Assurance artifacts (multi-criteria mode produces the full set).
        let prov = crate::suite::provenance::build_provenance(&report);
        let gsn_arg = gsn::build_argument(&report, min_trl);
        let vcd_doc = vcd::build_vcd(&report);
        let verdict = gate::assurance_gate(&report);
        let trl_rows = reports::trl_assessment(&report, min_trl);

        let json = serde_json::json!({
            "kind": Self::KIND,
            "overlay": "kshana-pro",
            "overlay_version": crate::version(),
            "title": report.title,
            "label": TRADE_LABEL,
            "metric": report.metric,
            "lower_better": report.lower_better,
            "multi_criteria": report.decision.is_some(),
            "winner": report.winner,
            "recommendation": report.recommendation,
            "variants": report.outcomes.iter().map(|o| serde_json::json!({
                "label": o.label,
                "rank": o.rank,
                "metric_value": o.metric_value,
                "scenario_hash": o.scenario_hash,
                "criterion_values": o.criterion_values,
                "error": o.error,
            })).collect::<Vec<_>>(),
            "decision": report.decision,
            "provenance": prov,
            "gsn": gsn_arg,
            "vcd": vcd_doc,
            "trl": trl_rows,
            "gate": verdict,
            "evidence_pack_markdown": evidence_md,
            "assurance_pack_markdown": assurance_pack_markdown(&report, &prov, &gsn_arg, &vcd_doc, &verdict, min_trl),
        })
        .to_string();

        let ranked = report.outcomes.iter().filter(|o| o.rank.is_some()).count();
        let summary = if report.decision.is_some() {
            format!(
                "Kshana Pro L3 multi-criteria trade | {} | {} variant(s), {} ranked | winner: {} | gate: {} | {}",
                report.title,
                report.outcomes.len(),
                ranked,
                report.winner.as_deref().unwrap_or("none"),
                if verdict.passed() { "PASS" } else { "FAIL" },
                report.recommendation,
            )
        } else {
            format!(
                "Kshana Pro L3 trade study | {} | metric '{}' ({}) | {} variant(s), {} ranked | winner: {} | {}",
                report.title,
                report.metric,
                if report.lower_better { "lower better" } else { "higher better" },
                report.outcomes.len(),
                ranked,
                report.winner.as_deref().unwrap_or("none"),
                report.recommendation,
            )
        };

        let svg = trade_svg(&report);
        Ok(RunOutput {
            json,
            svg,
            summary,
            csv: None,
        })
    }
}

/// Concatenate every assurance artifact into one self-contained markdown pack.
fn assurance_pack_markdown(
    report: &TradeReport,
    prov: &crate::suite::DecisionProvenance,
    gsn_arg: &gsn::GsnArgument,
    vcd_doc: &vcd::Vcd,
    verdict: &gate::GateVerdict,
    min_trl: u8,
) -> String {
    let mut md = evidence_pack_markdown(report);
    if report.decision.is_some() {
        md.push_str("\n---\n\n");
        md.push_str(&reports::ecss_annex_l_markdown(report));
        md.push_str("\n---\n\n");
        md.push_str(&reports::nasa_se_68_markdown(report));
        md.push_str("\n---\n\n");
        md.push_str(&reports::trl_markdown(report, min_trl));
        md.push_str(&gsn_arg.markdown());
        md.push_str(&vcd_doc.markdown());
        md.push_str(&verdict.markdown());
    }
    // The evidence pack already carries the provenance; reference its root here too.
    let _ = prov;
    md
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

/// A small ranked bar chart of the metric/score across variants.
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
        "<text x=\"16\" y=\"40\" fill=\"#8c8273\">{} · {}</text>",
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
            ..Default::default()
        };
        let out = pack.run().expect("trade study runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["winner"], "optical-lattice", "summary: {}", out.summary);
        assert_eq!(v["multi_criteria"], false);
        assert!(out.json.contains("evidence_pack_markdown"));
        assert!(out.svg.contains("<svg"));
    }

    #[test]
    fn multi_criteria_scenario_emits_full_assurance_layer() {
        let pack = TradeStudyScenario {
            title: "clock architecture (multi-criteria)".into(),
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
            criteria: vec![
                CriterionSpec {
                    name: "holdover".into(),
                    metric: "quantum.fom.holdover_s".into(),
                    weight: 0.7,
                    lower_better: false,
                    min: None,
                    max: None,
                },
                CriterionSpec {
                    name: "rms_ns".into(),
                    metric: "quantum.fom.timing_rms_ns".into(),
                    weight: 0.3,
                    lower_better: true,
                    min: None,
                    max: None,
                },
            ],
            trl: BTreeMap::from([("optical-lattice".into(), 7u8), ("csac".into(), 8u8)]),
            ..Default::default()
        };
        let out = pack.run().expect("multi-criteria trade runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["multi_criteria"], true);
        assert!(v["decision"].is_object(), "decision block present");
        assert!(v["provenance"]["merkle_root"].is_string());
        assert!(v["gsn"]["nodes"].is_array());
        assert!(v["vcd"]["items"].is_array());
        assert!(v["gate"]["outcome"].is_string());
        // The combined assurance pack carries every section.
        let pack_md = v["assurance_pack_markdown"].as_str().unwrap();
        assert!(pack_md.contains("Annex L"));
        assert!(pack_md.contains("§6.8"));
        assert!(pack_md.contains("GSN"));
        assert!(pack_md.contains("Verification Control Document"));
        assert!(pack_md.contains("Assurance gate"));
        assert!(pack_md.contains("Merkle root"));
    }

    #[test]
    fn ahp_weighting_parses_and_surfaces_consistency() {
        let pack = TradeStudyScenario {
            title: "ahp trade".into(),
            variants: vec![
                VariantSpec {
                    label: "optical".into(),
                    scenario: clock_variant("optical", 1.0e-30),
                },
                VariantSpec {
                    label: "csac".into(),
                    scenario: clock_variant("csac", 9.0e-20),
                },
            ],
            criteria: vec![
                CriterionSpec {
                    name: "holdover".into(),
                    metric: "quantum.fom.holdover_s".into(),
                    weight: 1.0,
                    lower_better: false,
                    min: None,
                    max: None,
                },
                CriterionSpec {
                    name: "rms_ns".into(),
                    metric: "quantum.fom.timing_rms_ns".into(),
                    weight: 1.0,
                    lower_better: true,
                    min: None,
                    max: None,
                },
            ],
            ahp: Some(AhpSpec {
                pairwise: vec![vec![1.0, 3.0], vec![1.0 / 3.0, 1.0]],
                cr_threshold: 0.10,
            }),
            ..Default::default()
        };
        let out = pack.run().expect("ahp trade runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
        assert_eq!(v["decision"]["weighting_method"], "ahp");
        assert!(v["decision"]["ahp"]["acceptable"].as_bool().unwrap());
    }

    #[test]
    fn trade_study_external_contract() {
        let pack = TradeStudyScenario {
            title: "t".into(),
            metric: "n_samples".into(),
            ..Default::default()
        };
        assert_eq!(pack.kind_name(), "pro-trade-study");
        assert_eq!(pack.meta().name, "pro-trade-study");
    }
}
