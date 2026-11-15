// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Threat scenario library (roadmap 0.42, E2)
//!
//! Curated real interference events (the Baltic, the Strait of Hormuz, the Red Sea, the
//! JammerTest campaigns at Andøya and the Texas Spoofing Test Battery) written down as
//! **reproducible scenarios** with their provenance and licence notes. Each scenario file
//! (`threat-library/scenarios/<id>.toml`, format [`FORMAT`]) carries
//!
//! * **what was published**: the sources (title, publisher, date, address or citation) and
//!   one line on what each says, the provenance class, and how the scenario's parameters
//!   were chosen from them (`parameter_basis`);
//! * **licence notes**: the terms of the source material, and the rule this library keeps:
//!   **no source data is redistributed**. TEXBAT recordings and JammerTest data are never
//!   copied here; a scenario names where the user fetches them (`[replay]`) and the files
//!   to expect, nothing more;
//! * **a reproducible analogue**: an open-engine training scenario (`[analogue]`, the
//!   `kshana::nmea_synth` format) that produces the same class of effect, deterministic per
//!   seed, labelled as representative and not a reconstruction; and/or a timing profile
//!   (`[timing]`) that the timing scorecards (T4) apply to a grandmaster.
//!
//! Exports go through the open engine's I4 writers (`kshana::interop::testbench`: motion CSV
//! and metadata, waypoints, truth NMEA, events CSV and TOML, conditions), plus the analogue
//! receiver's NMEA stream and a `receiver-trust` scenario that scores it with the open engine.
//!
//! **Feed.** The library is published as a monthly feed: a directory of the scenario files
//! with `feed.json` (the edition, the previous edition's manifest hash, every scenario's
//! SHA-256 and the edition it was added or changed in), signed as a whole with the dossier
//! signature ([`crate::signed_dossier`]). Building a feed needs the `pro-threat-library`
//! feature; verifying one needs nothing ([`feed::verify`]).
//!
//! **Honest limits.** A scenario is a description and a parameter set, not a measurement.
//! The analogue parameters are representative of the published descriptions; they were not
//! read off any recording, and every export says so.

pub mod export;
pub mod feed;

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::nmea_synth::TrainingScenario;
use serde::{Deserialize, Serialize};

/// The scenario kind: run one library scenario's analogue through the open monitors.
pub const KIND: &str = "pro-threat-scenario";
/// The command feature of exports and feed building.
pub const FEATURE: &str = "pro-threat-library";
/// The format tag of a scenario file.
pub const FORMAT: &str = "kshana-pro-threat-scenario/1";
/// The library edition this build carries (the feed edition of the embedded files).
pub const EDITION: &str = "2026-11";

/// The honesty label of every library output.
pub const LABEL: &str = "MODELLED analogue of a published event: the parameters are \
representative of the published descriptions, not read off any recording; no source data is \
included. REPORTED: the sources, as cited.";

/// The library files this build carries, by id.
pub const LIBRARY: &[(&str, &str)] = &[
    (
        "baltic-gulf-of-finland-jamming",
        include_str!("../../threat-library/scenarios/baltic-gulf-of-finland-jamming.toml"),
    ),
    (
        "baltic-tartu-approach-jamming",
        include_str!("../../threat-library/scenarios/baltic-tartu-approach-jamming.toml"),
    ),
    (
        "baltic-timing-holdover-under-jamming",
        include_str!("../../threat-library/scenarios/baltic-timing-holdover-under-jamming.toml"),
    ),
    (
        "hormuz-position-displacement",
        include_str!("../../threat-library/scenarios/hormuz-position-displacement.toml"),
    ),
    (
        "hormuz-meaconing-delay",
        include_str!("../../threat-library/scenarios/hormuz-meaconing-delay.toml"),
    ),
    (
        "red-sea-bab-el-mandeb-jamming",
        include_str!("../../threat-library/scenarios/red-sea-bab-el-mandeb-jamming.toml"),
    ),
    (
        "red-sea-jamming-then-drag-off",
        include_str!("../../threat-library/scenarios/red-sea-jamming-then-drag-off.toml"),
    ),
    (
        "jammertest-stepped-power-jamming",
        include_str!("../../threat-library/scenarios/jammertest-stepped-power-jamming.toml"),
    ),
    (
        "jammertest-position-spoofing",
        include_str!("../../threat-library/scenarios/jammertest-position-spoofing.toml"),
    ),
    (
        "jammertest-time-spoofing",
        include_str!("../../threat-library/scenarios/jammertest-time-spoofing.toml"),
    ),
    (
        "texbat-ds3-matched-power-time-push",
        include_str!("../../threat-library/scenarios/texbat-ds3-matched-power-time-push.toml"),
    ),
    (
        "texbat-ds4-matched-power-position-push",
        include_str!("../../threat-library/scenarios/texbat-ds4-matched-power-position-push.toml"),
    ),
];

/// Where an event comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    Baltic,
    Hormuz,
    RedSea,
    Jammertest,
    Texbat,
}

impl Family {
    pub fn as_str(self) -> &'static str {
        match self {
            Family::Baltic => "baltic",
            Family::Hormuz => "hormuz",
            Family::RedSea => "red-sea",
            Family::Jammertest => "jammertest",
            Family::Texbat => "texbat",
        }
    }
    /// Families whose source data are datasets with their own terms: the library never
    /// carries their data, and a scenario must say where the user fetches them.
    pub fn is_dataset(self) -> bool {
        matches!(self, Family::Jammertest | Family::Texbat)
    }
}

/// The kind of threat a scenario describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Threat {
    Jamming,
    Spoofing,
    Meaconing,
    TimeSpoofing,
}

/// The receiving domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Domain {
    Maritime,
    Aviation,
    LandStatic,
    Timing,
}

/// How strong the published basis is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceClass {
    /// A published test plan or log of an organised test campaign.
    PublishedTestCampaign,
    /// A dataset described in a peer-reviewed or conference paper.
    PublishedDataset,
    /// A notice or advisory by an authority or an official maritime/aviation body.
    OfficialNotice,
    /// An operator's own public statement.
    OperatorStatement,
}

/// One published source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub title: String,
    pub publisher: String,
    /// When it was published, as precisely as known (`2024-04-29`, `2024`, `2012-09`).
    pub date: String,
    #[serde(default)]
    pub url: Option<String>,
    /// A citation or identifier (DOI, notice number) when there is no stable address.
    #[serde(default)]
    pub citation: Option<String>,
    /// What the source says that this scenario uses, in one or two sentences.
    pub says: String,
}

/// Where the scenario's description and numbers come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub class: ProvenanceClass,
    #[serde(rename = "source")]
    pub sources: Vec<Source>,
    /// How the analogue and timing parameters were chosen from the sources.
    pub parameter_basis: String,
    /// What a user should check against the sources before relying on the scenario.
    #[serde(default)]
    pub verify_before_use: Option<String>,
}

/// The licence notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenceNotes {
    /// The terms of the source material, as far as they are known.
    pub source_terms: String,
    /// Whether any source data is in the library. Always false: the validator refuses true.
    pub redistributed: bool,
    /// The terms of this scenario file and its analogue (Kshana Pro content).
    pub library_terms: String,
}

/// Original data the user may fetch to replay the event, never included.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    /// The dataset's name and part (`TEXBAT ds3`, `JammerTest 2024`).
    pub dataset: String,
    /// Where it is published (an address, a DOI or the publisher's page).
    pub fetch: String,
    /// The files the user should expect, by name, as the publisher names them.
    #[serde(default)]
    pub files: Vec<String>,
    /// The format of those files (`complex int16 IQ, 25 Msps`, `u-blox UBX`, `RINEX 3`).
    pub format: String,
    /// How to use them with Kshana (which command, through which equipment).
    pub how: String,
    /// Where the event times for scoring come from (never guessed here).
    pub events_from: String,
}

/// A time-offset profile applied to a GNSS-disciplined clock (used by the timing scorecards).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimingProfile {
    /// `time-push` (a ramp at `rate_s_per_s` up to `max_offset_s`), `time-step` (`step_s` at
    /// onset) or `denial` (the reference is lost: the clock is in holdover for `duration_s`).
    pub kind: String,
    /// Onset after the clock's handover point, s.
    #[serde(default)]
    pub start_s: f64,
    /// How long the condition lasts, s.
    pub duration_s: f64,
    #[serde(default)]
    pub rate_s_per_s: Option<f64>,
    #[serde(default)]
    pub max_offset_s: Option<f64>,
    #[serde(default)]
    pub step_s: Option<f64>,
    /// `all-receivers` (a wide-area event that reaches every GNSS receiver of a site) or
    /// `one-receiver` (a local event that reaches one antenna).
    pub scope: String,
}

impl TimingProfile {
    /// The counterfeit time offset the disciplined clock follows `u` s after onset (zero
    /// before onset and for `denial`).
    pub fn offset(&self, u: f64) -> f64 {
        if u < 0.0 || u > self.duration_s {
            return 0.0;
        }
        match self.kind.as_str() {
            "time-push" => {
                let r = self.rate_s_per_s.unwrap_or(0.0);
                let m = self.max_offset_s.unwrap_or(f64::INFINITY);
                let v = r * u;
                v.signum() * v.abs().min(m)
            }
            "time-step" => self.step_s.unwrap_or(0.0),
            _ => 0.0,
        }
    }
    fn validate(&self) -> Result<(), String> {
        let pos = |v: f64| v.is_finite() && v > 0.0;
        if !pos(self.duration_s) || !self.start_s.is_finite() || self.start_s < 0.0 {
            return Err("timing: duration_s must be positive and start_s not negative".into());
        }
        match self.kind.as_str() {
            "time-push" => {
                if !self.rate_s_per_s.is_some_and(|r| r.is_finite() && r != 0.0)
                    || !self.max_offset_s.is_some_and(pos)
                {
                    return Err("timing: a time-push needs rate_s_per_s and max_offset_s".into());
                }
            }
            "time-step" => {
                if !self.step_s.is_some_and(|s| s.is_finite() && s != 0.0) {
                    return Err("timing: a time-step needs step_s".into());
                }
            }
            "denial" => {}
            other => return Err(format!("timing: unknown kind '{other}'")),
        }
        if self.scope != "all-receivers" && self.scope != "one-receiver" {
            return Err("timing: scope must be all-receivers or one-receiver".into());
        }
        Ok(())
    }
}

/// One library scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreatScenario {
    pub format: String,
    pub id: String,
    /// Bumped whenever the scenario changes; the feed records the edition of each version.
    pub version: u32,
    pub title: String,
    pub family: Family,
    pub threat: Vec<Threat>,
    pub domain: Domain,
    /// When the event happened, as reported.
    pub period: String,
    pub summary: String,
    pub provenance: Provenance,
    pub licence: LicenceNotes,
    #[serde(default)]
    pub analogue: Option<TrainingScenario>,
    #[serde(default)]
    pub timing: Option<TimingProfile>,
    #[serde(default)]
    pub replay: Option<Replay>,
}

/// Words no library text may use about a system.
pub const FORBIDDEN_WORDS: [&str; 5] = [
    "compliant",
    "compliance",
    "conform",
    "conformant",
    "certified",
];

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

impl ThreatScenario {
    /// Parse and validate a scenario file's text.
    pub fn parse(src: &str) -> Result<ThreatScenario, KshanaError> {
        let s: ThreatScenario = toml::from_str(src)
            .map_err(|e| KshanaError::InvalidInput(format!("threat scenario: {e}")))?;
        s.validate()
            .map_err(|e| KshanaError::InvalidInput(format!("threat scenario '{}': {e}", s.id)))?;
        Ok(s)
    }

    /// The library rules: form, provenance, licence and the no-redistribution rule.
    pub fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT {
            return Err(format!("format must be '{FORMAT}'"));
        }
        if !is_slug(&self.id) {
            return Err("id must be lower-case letters, digits and dashes".into());
        }
        if self.version == 0 {
            return Err("version starts at 1".into());
        }
        if self.title.trim().is_empty() || self.summary.trim().is_empty() {
            return Err("a scenario needs a title and a summary".into());
        }
        if self.threat.is_empty() {
            return Err("a scenario names at least one threat".into());
        }
        if self.provenance.sources.is_empty() {
            return Err("a scenario cites at least one source".into());
        }
        for s in &self.provenance.sources {
            if s.title.trim().is_empty()
                || s.publisher.trim().is_empty()
                || s.says.trim().is_empty()
            {
                return Err("every source has a title, a publisher and what it says".into());
            }
            if s.url.is_none() && s.citation.is_none() {
                return Err(format!(
                    "source '{}' needs an address or a citation",
                    s.title
                ));
            }
        }
        if self.provenance.parameter_basis.trim().is_empty() {
            return Err("parameter_basis says how the parameters were chosen".into());
        }
        if self.licence.redistributed {
            return Err(
                "the library never redistributes source data (licence.redistributed = true)".into(),
            );
        }
        if self.licence.source_terms.trim().is_empty()
            || self.licence.library_terms.trim().is_empty()
        {
            return Err("licence notes need the source terms and the library terms".into());
        }
        if self.family.is_dataset() {
            let r = self.replay.as_ref().ok_or(
                "a JammerTest or TEXBAT scenario says where the user fetches the data ([replay])",
            )?;
            if r.fetch.trim().is_empty() || r.events_from.trim().is_empty() {
                return Err("replay needs fetch and events_from".into());
            }
        }
        if self.analogue.is_none() && self.timing.is_none() {
            return Err("a scenario has an [analogue], a [timing] profile, or both".into());
        }
        if let Some(t) = &self.timing {
            t.validate()?;
        }
        if let Some(a) = &self.analogue {
            if a.events.is_empty() {
                return Err("the analogue scripts at least one event".into());
            }
            a.validate().map_err(|e| format!("analogue: {e}"))?;
        }
        let text = toml::to_string(self)
            .unwrap_or_default()
            .to_ascii_lowercase();
        for w in FORBIDDEN_WORDS {
            if text.contains(w) {
                return Err(format!("the scenario text uses '{w}'"));
            }
        }
        Ok(())
    }

    /// The receiver-trust event kind of an analogue event.
    pub fn event_kind(k: kshana::nmea_synth::config::EventKind) -> &'static str {
        use kshana::nmea_synth::config::EventKind as K;
        match k {
            K::Jamming => "jamming",
            K::DragOff | K::TimeSpoof | K::ReplayDelay => "spoofing",
            K::NaturalDegradation => "other",
        }
    }

    /// Whether the analogue receiver stands still (every speed zero).
    pub fn analogue_is_static(&self) -> bool {
        self.analogue.as_ref().is_some_and(|a| {
            a.vessel.speed_kn == 0.0 && a.waypoints.iter().all(|w| w.speed_kn.unwrap_or(0.0) == 0.0)
        })
    }
}

/// Every library scenario of this build, parsed and validated.
pub fn library() -> Result<Vec<ThreatScenario>, KshanaError> {
    LIBRARY
        .iter()
        .map(|(id, src)| {
            let s = ThreatScenario::parse(src)?;
            if s.id != *id {
                return Err(KshanaError::InvalidInput(format!(
                    "library file '{id}' carries id '{}'",
                    s.id
                )));
            }
            Ok(s)
        })
        .collect()
}

/// One library scenario by id.
pub fn get(id: &str) -> Result<ThreatScenario, KshanaError> {
    let src = LIBRARY
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, s)| *s)
        .ok_or_else(|| {
            KshanaError::InvalidInput(format!(
                "no threat scenario '{id}' in this library (edition {EDITION}); `kshana-pro threats list`"
            ))
        })?;
    ThreatScenario::parse(src)
}

/// The text of a library scenario file by id.
pub fn source_of(id: &str) -> Option<&'static str> {
    LIBRARY.iter().find(|(i, _)| *i == id).map(|(_, s)| *s)
}

/// A `pro-threat-scenario` run: one library scenario by id.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreatRunScenario {
    pub kind: String,
    /// The library id.
    pub scenario: String,
}

impl ThreatRunScenario {
    pub const KIND: &'static str = KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Threat scenario library (E2): one curated real event (Baltic, Hormuz, \
Red Sea, JammerTest, TEXBAT) with provenance and licence notes, its reproducible analogue run \
through the open receiver-trust monitors; no source data included.",
            required_fields: &["scenario"],
            optional_fields: &[],
        }
    }
}

impl Scenario for ThreatRunScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let s = get(&self.scenario)?;
        let run = export::run_analogue(&s)?;
        let summary = match &run {
            Some(r) => format!(
                "Kshana Pro threat library | {} | {} | analogue: {} of {} events detected by the open monitors | MODELLED",
                s.id,
                s.title,
                r.events_detected,
                r.events_evaluable
            ),
            None => format!(
                "Kshana Pro threat library | {} | {} | timing profile only | MODELLED",
                s.id, s.title
            ),
        };
        let doc = serde_json::json!({
            "format": "kshana-pro-threat-run/1",
            "kind": KIND,
            "label": LABEL,
            "edition": EDITION,
            "scenario": s,
            "analogue": run,
        });
        let csv = run.as_ref().map(export::events_csv);
        Ok(RunOutput {
            json: serde_json::to_string_pretty(&doc)
                .map_err(|e| KshanaError::InvalidInput(e.to_string()))?,
            svg: format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"72\" font-family=\"system-ui,sans-serif\"><rect width=\"480\" height=\"72\" fill=\"#0f1216\"/><text x=\"16\" y=\"30\" fill=\"#8cc4f5\" font-size=\"15\" font-weight=\"bold\">Threat scenario · {}</text><text x=\"16\" y=\"54\" fill=\"#9aa3af\" font-size=\"12\">{} · MODELLED analogue, no source data</text></svg>",
                kshana_escape(&s.id),
                s.family.as_str()
            ),
            summary,
            csv,
        })
    }
}

fn kshana_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl ExternalPack for ThreatRunScenario {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(KIND));
    }
}
