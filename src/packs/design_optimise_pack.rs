// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-design-optimise` pack — exposes the design optimiser ([`crate::design`]) as
//! an out-of-tree [`ExternalPack`], so a study file runs through `kshana-pro run`, the
//! service and the registry like any other scenario. It states a design space over a
//! base scenario of any open kind, runs the designs on the embedded open engine and
//! returns the Pareto front, its knee and the place of a reference design.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::design::{run_resolved, Options, Study};

/// A `pro-design-optimise` scenario: a design study (see `docs/DESIGN-OPTIMISER.md`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DesignOptimiseScenario {
    #[serde(flatten)]
    pub study: Study,
}

impl DesignOptimiseScenario {
    pub const KIND: &'static str = crate::design::KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro design optimiser: state a design space over a base scenario of any open kind, constraints and two or three objectives, and get the Pareto front, its knee and the place of a reference design. Every evaluation is one run of the open engine; every front design is exported as a plain open scenario. Exhaustive when the space fits the budget, seeded NSGA-II otherwise; a front that is not exhaustive is labelled best found in N runs.",
            required_fields: &["title", "base_scenario", "variable", "objective"],
            optional_fields: &["seed", "constraint", "search", "reference"],
        }
    }
}

impl Scenario for DesignOptimiseScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let resolved = self.study.resolve()?;
        let outcome = run_resolved(&resolved, &Options::default())?;
        Ok(RunOutput {
            json: outcome.json,
            svg: outcome.svg,
            summary: outcome.summary,
            csv: Some(outcome.index_csv),
        })
    }
}

impl ExternalPack for DesignOptimiseScenario {
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
