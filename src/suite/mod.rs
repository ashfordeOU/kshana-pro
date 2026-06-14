// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! # L3 — Trade-study & assurance **framework** (demonstrator)
//!
//! Turns a set of architecture variants into a ranked, audit-grade trade study by
//! running each variant through the **embedded open engine**
//! ([`kshana::api::run_scenario`]) and comparing a chosen figure of merit. Each
//! variant's result carries the open core's reproducible `scenario_hash`, so the
//! study is replayable — the spine of an assurance evidence pack (PRR/ISRR-style).
//!
//! ## What is built vs still gated
//!
//! - **Built (demonstrator):** the automated trade engine + the evidence-pack
//!   generator, built entirely on open capabilities (no gated inputs).
//! - **Still gated:** packaging this as a *sold, licensed Kshana Pro SKU* with full
//!   ECSS/MBSE artifact generation waits on proven demand (a paying customer) — see
//!   [`product_gated`]. The engine is the calculator; this is the workflow on top.

use kshana::api::{run_scenario, KshanaError};

use crate::util::dot_path_number;

/// One architecture option in the trade space: a label + a full scenario (the open
/// engine runs it unchanged, so any open scenario kind is a valid variant).
#[derive(Debug, Clone)]
pub struct Variant {
    pub label: String,
    pub scenario_toml: String,
}

/// A trade space: variants compared on one figure of merit.
#[derive(Debug, Clone)]
pub struct TradeSpace {
    pub title: String,
    /// Dot-path into each variant's result JSON, e.g. `quantum.fom.holdover_s`.
    pub metric: String,
    /// True if a *lower* metric value is better (e.g. position error).
    pub lower_better: bool,
    pub variants: Vec<Variant>,
}

/// The outcome for one variant.
#[derive(Debug, Clone)]
pub struct VariantOutcome {
    pub label: String,
    pub scenario_hash: Option<String>,
    pub metric_value: Option<f64>,
    pub rank: Option<usize>,
    pub error: Option<String>,
}

/// The ranked trade study + the material for an assurance evidence pack.
#[derive(Debug, Clone)]
pub struct TradeReport {
    pub title: String,
    pub metric: String,
    pub lower_better: bool,
    pub outcomes: Vec<VariantOutcome>,
    pub winner: Option<String>,
    pub recommendation: String,
}

/// **Run the trade study**: execute each variant on the open engine, extract the
/// metric, and rank. Variant failures are captured (not fatal) so a partial trade
/// still produces a report.
pub fn run_trade_study(space: &TradeSpace) -> Result<TradeReport, KshanaError> {
    if space.variants.len() < 2 {
        return Err(KshanaError::InvalidInput(
            "a trade study needs >= 2 variants".into(),
        ));
    }
    if space.metric.trim().is_empty() {
        return Err(KshanaError::InvalidInput(
            "a trade study needs a non-empty `metric` dot-path".into(),
        ));
    }

    let mut outcomes: Vec<VariantOutcome> = space
        .variants
        .iter()
        .map(|v| match run_scenario(&v.scenario_toml) {
            Ok(out) => {
                let json: serde_json::Value =
                    serde_json::from_str(&out.json).unwrap_or(serde_json::Value::Null);
                let scenario_hash = json
                    .get("scenario_hash")
                    .and_then(|h| h.as_str())
                    .map(String::from);
                let metric_value = dot_path_number(&json, &space.metric);
                VariantOutcome {
                    label: v.label.clone(),
                    scenario_hash,
                    metric_value,
                    rank: None,
                    error: if metric_value.is_none() {
                        Some(format!(
                            "metric '{}' not found or non-numeric",
                            space.metric
                        ))
                    } else {
                        None
                    },
                }
            }
            Err(e) => VariantOutcome {
                label: v.label.clone(),
                scenario_hash: None,
                metric_value: None,
                rank: None,
                error: Some(format!("run failed: {e}")),
            },
        })
        .collect();

    // Rank the variants that produced a metric.
    let mut ranked: Vec<usize> = (0..outcomes.len())
        .filter(|&i| outcomes[i].metric_value.is_some())
        .collect();
    ranked.sort_by(|&a, &b| {
        let (va, vb) = (
            outcomes[a].metric_value.unwrap(),
            outcomes[b].metric_value.unwrap(),
        );
        let ord = va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal);
        if space.lower_better {
            ord
        } else {
            ord.reverse()
        }
    });
    for (rank, &i) in ranked.iter().enumerate() {
        outcomes[i].rank = Some(rank + 1);
    }

    let winner = ranked.first().map(|&i| outcomes[i].label.clone());
    let recommendation =
        match (&winner, ranked.len()) {
            (Some(w), n) => {
                format!(
            "Recommended architecture: '{w}' — best '{}' ({}) across {} comparable variant(s).",
            space.metric,
            if space.lower_better { "lower is better" } else { "higher is better" },
            n
        )
            }
            (None, _) => "No variant produced a comparable metric; trade inconclusive.".to_string(),
        };

    Ok(TradeReport {
        title: space.title.clone(),
        metric: space.metric.clone(),
        lower_better: space.lower_better,
        outcomes,
        winner,
        recommendation,
    })
}

/// Render an audit-grade evidence-pack markdown: the reproducible record of the
/// trade (every variant's scenario hash + metric + rank), suitable as the basis of
/// a PRR/ISRR-style assurance artifact.
pub fn evidence_pack_markdown(report: &TradeReport) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Trade-study evidence pack — {}\n\n",
        report.title
    ));
    md.push_str(&format!(
        "Generated by Kshana Pro v{} over the open Kshana engine.\n\n",
        crate::version()
    ));
    md.push_str(&format!(
        "**Figure of merit:** `{}` ({})\n\n",
        report.metric,
        if report.lower_better {
            "lower is better"
        } else {
            "higher is better"
        }
    ));
    md.push_str("| Rank | Variant | Metric | Scenario hash (reproducible) | Note |\n");
    md.push_str("|------|---------|--------|------------------------------|------|\n");
    let mut rows: Vec<&VariantOutcome> = report.outcomes.iter().collect();
    rows.sort_by_key(|o| o.rank.unwrap_or(usize::MAX));
    for o in rows {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            o.rank.map(|r| r.to_string()).unwrap_or_else(|| "—".into()),
            o.label,
            o.metric_value
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "—".into()),
            o.scenario_hash
                .as_deref()
                .map(|h| &h[..h.len().min(16)])
                .unwrap_or("—"),
            o.error.as_deref().unwrap_or("ok"),
        ));
    }
    md.push_str(&format!(
        "\n**Recommendation:** {}\n\n",
        report.recommendation
    ));
    md.push_str(
        "_Reproducibility: each scenario hash is the open engine's SHA-256 of the \
         exact scenario; re-running it reproduces the bits. This record is the spine \
         of an assurance evidence pack (PRR/ISRR-style)._\n",
    );
    md
}

/// The seam guard for the **gated** part of L3: packaging this as the sold,
/// licensed Kshana Pro SKU with full ECSS/MBSE artifact generation. The framework
/// is built and runs; productisation waits on proven demand.
pub fn product_gated() -> KshanaError {
    KshanaError::Unsupported(
        "Packaging the trade-study/assurance suite as a licensed Kshana Pro SKU (with full \
         ECSS/MBSE artifact generation) is gated on proven demand (a second paying customer); \
         not built on spec. The framework itself is built. See BUILD-TRACKER PR15–PR18."
            .into(),
    )
}

/// Convenience: run a trade study and also produce its evidence-pack markdown.
pub fn run_with_evidence(space: &TradeSpace) -> Result<(TradeReport, String), KshanaError> {
    let report = run_trade_study(space)?;
    let md = evidence_pack_markdown(&report);
    Ok((report, md))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two ephemeris variants differing only in run length → different n_samples.
    // Deterministic and fast; exercises the full trade mechanics.
    fn ephem(duration_s: f64) -> String {
        format!(
            "kind = \"ephemeris\"\n\
             tle = \"\"\"\n\
             1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
             2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
             \"\"\"\n\
             step_s = 30.0\n\
             duration_s = {duration_s}\n"
        )
    }

    fn trade() -> TradeSpace {
        TradeSpace {
            title: "ephemeris sample-count trade".into(),
            metric: "n_samples".into(),
            lower_better: false,
            variants: vec![
                Variant {
                    label: "short".into(),
                    scenario_toml: ephem(60.0),
                },
                Variant {
                    label: "long".into(),
                    scenario_toml: ephem(300.0),
                },
            ],
        }
    }

    #[test]
    fn ranks_variants_by_metric() {
        let report = run_trade_study(&trade()).unwrap();
        assert_eq!(report.winner.as_deref(), Some("long")); // more samples, higher-better
        let long = report.outcomes.iter().find(|o| o.label == "long").unwrap();
        let short = report.outcomes.iter().find(|o| o.label == "short").unwrap();
        assert_eq!(long.rank, Some(1));
        assert_eq!(short.rank, Some(2));
        assert!(long.metric_value.unwrap() > short.metric_value.unwrap());
        assert!(
            long.scenario_hash.is_some(),
            "open engine must stamp a scenario hash"
        );
    }

    #[test]
    fn lower_better_inverts_ranking() {
        let mut t = trade();
        t.lower_better = true;
        let report = run_trade_study(&t).unwrap();
        assert_eq!(report.winner.as_deref(), Some("short"));
    }

    #[test]
    fn missing_metric_is_captured_not_fatal() {
        let mut t = trade();
        t.metric = "does.not.exist".into();
        let report = run_trade_study(&t).unwrap();
        assert!(report.winner.is_none());
        assert!(report.outcomes.iter().all(|o| o.error.is_some()));
    }

    #[test]
    fn failed_variant_is_captured_not_fatal() {
        let mut t = trade();
        // An ephemeris with neither tle nor orbit → the open engine rejects it.
        t.variants[0].scenario_toml = "kind = \"ephemeris\"\nstep_s = 30.0\n".into();
        let report = run_trade_study(&t).unwrap();
        let bad = report.outcomes.iter().find(|o| o.label == "short").unwrap();
        assert!(bad.error.is_some() && bad.metric_value.is_none());
        // The good variant still ranks.
        assert_eq!(report.winner.as_deref(), Some("long"));
    }

    #[test]
    fn evidence_pack_records_hashes_and_recommendation() {
        let (report, md) = run_with_evidence(&trade()).unwrap();
        assert!(md.contains("evidence pack"));
        assert!(md.contains("Scenario hash"));
        assert!(md.contains("Recommendation"));
        assert!(md.contains(&report.winner.unwrap()));
    }

    #[test]
    fn too_few_variants_is_an_error() {
        let mut t = trade();
        t.variants.truncate(1);
        assert!(matches!(
            run_trade_study(&t),
            Err(KshanaError::InvalidInput(_))
        ));
    }

    #[test]
    fn product_sku_is_gated() {
        assert!(matches!(product_gated(), KshanaError::Unsupported(_)));
    }
}
