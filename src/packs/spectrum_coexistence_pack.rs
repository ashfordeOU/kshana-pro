// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-spectrum-coexistence` pack: exposes [`crate::spectrum_coexistence`] as an
//! [`ExternalPack`], so `kshana-pro run <study.toml>` prints the study's result JSON
//! and the open ∪ Pro registry can build it by id.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::spectrum_coexistence as sc;

/// A `pro-spectrum-coexistence` study, kept as its parsed table: the study module
/// validates it (and refuses what is out of scope) when it runs.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct SpectrumCoexistenceScenario {
    pub study: toml::Table,
}

impl SpectrumCoexistenceScenario {
    pub const KIND: &'static str = sc::KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Kshana Pro spectrum coexistence: test whole candidate signal plans and constellations against every open-service GNSS signal the open engine models, in both directions, and give a worldwide verdict against the limit the study sets (a MODELLED upper bound over the open leo-signal and constellation-design kinds).",
            required_fields: &["title", "max_degradation_db", "limit_source", "plan", "victims", "grid"],
            optional_fields: &["receiver"],
        }
    }
}

impl Scenario for SpectrumCoexistenceScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let result = sc::run_study_table(&self.study)?;
        Ok(RunOutput {
            json: sc::to_json_string(&result),
            svg: sc::render::matrix_svg(&result),
            summary: sc::summary_line(&result),
            csv: None,
        })
    }
}

impl ExternalPack for SpectrumCoexistenceScenario {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_contract() {
        let p: SpectrumCoexistenceScenario =
            toml::from_str("kind = \"pro-spectrum-coexistence\"\n").unwrap();
        assert_eq!(p.kind_name(), "pro-spectrum-coexistence");
        assert_eq!(p.meta().name, "pro-spectrum-coexistence");
        // A study with no limit is an error, not a run.
        assert!(p.run().is_err());
    }
}
