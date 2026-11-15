// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `pro-timing-holdover` kind (roadmap 0.42 T1): a timing fleet in, every clock's holdover
//! model and time to breach out ([`crate::timing`]). Source `path`s are read relative to the
//! working directory; `kshana-pro timing assess <fleet.toml>` reads them relative to the file.

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};

use crate::timing::fleet::{self, human_duration, FleetSpec};

/// A `pro-timing-holdover` scenario: the fleet file itself.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(transparent)]
pub struct TimingHoldoverScenario {
    pub spec: FleetSpec,
}

impl TimingHoldoverScenario {
    pub const KIND: &'static str = crate::timing::KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: Self::KIND,
            description: "Timing assurance: holdover model and time to breach of user-set limits for every clock of a mixed-vendor fleet, each read from its own source (TICC, TimeLab, RINEX clock, CGGTTS, linuxptp, chrony, ntpd, White Rabbit, PPS).",
            required_fields: &["limits", "clocks"],
            optional_fields: &["title", "fleet", "window_h", "horizon_h"],
        }
    }
}

impl Scenario for TimingHoldoverScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        // `parse` validates; a scenario built by the dispatcher arrives unvalidated.
        self.spec.validate()?;
        let a = fleet::assess(&self.spec)?;
        let worst = a
            .order
            .first()
            .and_then(|id| a.clock(id))
            .map(|c| {
                format!(
                    "{} ({}, earliest warning {})",
                    c.id,
                    c.status,
                    human_duration(c.worst_warn_s())
                )
            })
            .unwrap_or_default();
        let summary = format!(
            "Kshana Pro timing | {} clocks: {} act, {} watch, {} ok, {} beyond a limit | first: {} | MODELLED",
            a.counts.clocks, a.counts.act, a.counts.watch, a.counts.ok, a.counts.breached, worst
        );
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"72\" font-family=\"system-ui,sans-serif\"><rect width=\"480\" height=\"72\" fill=\"#0f1216\"/><text x=\"16\" y=\"30\" fill=\"#8cc4f5\" font-size=\"15\" font-weight=\"bold\">Timing fleet · {} clocks</text><text x=\"16\" y=\"54\" fill=\"#9aa3af\" font-size=\"12\">{} act · {} watch · {} ok · {} beyond a limit</text></svg>",
            a.counts.clocks, a.counts.act, a.counts.watch, a.counts.ok, a.counts.breached
        );
        let csv = Some(a.to_csv());
        Ok(RunOutput {
            json: serde_json::to_string(&a).expect("the assessment is plain data"),
            svg,
            summary,
            csv,
        })
    }
}

impl ExternalPack for TimingHoldoverScenario {
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
