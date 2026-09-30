// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-uncertainty` pack — exposes the uncertainty and sensitivity study
//! ([`crate::batch::uq`]) as an out-of-tree [`ExternalPack`].
//!
//! A study states how uncertain the inputs of any open scenario are, and gets back
//! confidence bands, the probability of meeting a limit and a ranking of the inputs
//! that drive each output. Through this pack the study runs in memory; the
//! `kshana-pro uncertainty` command runs the same study with a checkpoint file, so a
//! long one can be stopped and resumed. `docs/UNCERTAINTY.md` is the user guide.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::batch::uq::{self, render, RunOptions, StudySpec};

/// A `pro-uncertainty` scenario: the study file itself.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct UncertaintyScenario {
    pub study: StudySpec,
}

impl UncertaintyScenario {
    pub const KIND: &'static str = uq::KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro uncertainty and sensitivity: state how uncertain the inputs of any open scenario are (normal, uniform, lognormal, triangular or discrete, each with its source) and get, per output, the mean, spread and 5th/50th/95th percentiles with a bootstrap interval, the probability of meeting a limit, the percentile band of a time series, and a ranking of the inputs by Sobol' indices or standardised regression coefficients. Seeded, byte-identical for any thread count; `kshana-pro uncertainty` adds stop and resume.",
            required_fields: &["title", "seed", "base_scenario", "output", "sampling"],
            optional_fields: &["input", "sensitivity", "run"],
        }
    }
}

impl Scenario for UncertaintyScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let prepared = uq::prepare_spec(&self.study).map_err(KshanaError::InvalidInput)?;
        let run = uq::run_prepared(&prepared, &RunOptions::default())
            .map_err(KshanaError::InvalidInput)?;
        let result = run.result.ok_or_else(|| {
            KshanaError::NonConvergence("the study did not finish every realisation".into())
        })?;
        // The chart that answers the study's question: what drives the result when a
        // sensitivity method ran, the band when there is a series, else the intervals.
        let svg = if result.doc["sensitivity"]["method"] != "none" {
            render::sensitivity_svg(&result.doc)
        } else {
            render::bands_svg(&result.doc)
        };
        Ok(RunOutput {
            json: result.doc.to_string(),
            svg,
            summary: result.summary,
            csv: None,
        })
    }
}

impl ExternalPack for UncertaintyScenario {
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

#[cfg(test)]
mod tests {
    use super::*;

    const STUDY: &str = r#"kind = "pro-uncertainty"
title = "link margin"
seed = 5

base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""

[[input]]
name = "eirp"
key = "eirp_dbw"
dist = "normal"
mean = 55.0
std = 1.0
source = "assumed"

[[output]]
name = "margin"
path = "margin_db"

[sampling]
n = 32

[sensitivity]
method = "src"
"#;

    #[test]
    fn the_pack_parses_a_study_file_and_runs_it() {
        let pack: UncertaintyScenario = toml::from_str(STUDY).expect("a study file parses");
        let out = pack.run().expect("runs");
        let v: serde_json::Value = serde_json::from_str(&out.json).expect("JSON");
        assert_eq!(v["kind"], "pro-uncertainty");
        assert_eq!(v["runs"]["total"], 32);
        // One input, exactly linear: its standardised coefficient is 1.
        let src = v["sensitivity"]["outputs"]["margin"]["inputs"][0]["src"]
            .as_f64()
            .expect("a coefficient");
        assert!((src - 1.0).abs() < 1e-9, "src = {src}");
        assert!(out.svg.starts_with("<svg"));
        assert!(out.summary.contains("MODELLED"));
    }

    #[test]
    fn external_contract() {
        let pack: UncertaintyScenario = toml::from_str(STUDY).expect("parses");
        assert_eq!(pack.kind_name(), "pro-uncertainty");
        assert_eq!(pack.meta().name, "pro-uncertainty");
    }

    #[test]
    fn a_misspelled_field_is_refused_not_ignored() {
        let bad = STUDY.replace("std = 1.0", "stdev = 1.0");
        assert!(toml::from_str::<UncertaintyScenario>(&bad).is_err());
    }
}
