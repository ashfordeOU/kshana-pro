// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Incident forensics (roadmap 0.42, E1)
//!
//! Given an incident (a time window, an area, a subject vessel, aircraft or Station) and the
//! data files that bear on it (receiver logs, AIS and ADS-B position reports, Station logs,
//! third-party statements), this module produces a forensic report that says
//!
//! * **what happened**: jamming-like or spoofing-like interference, or neither, one finding
//!   per episode, each with its evidence class;
//! * **when and where**: a timeline, the onset and end of each finding bounded by the last
//!   clean and the first alarmed epoch, the affected area as modelled from the observers,
//!   and the tracks;
//! * **how confident**: a stated confidence rule per finding and the uncertainty of every
//!   time and place it gives, with the labels MEASURED, MODELLED and REPORTED;
//! * **what it rests on**: the SHA-256 of every input file;
//! * **how to reproduce it**: a recipe (the incident file with the inputs bundled) and the
//!   engine and method versions.
//!
//! Two variants: `standard` and `claim-grade` (prices: docs/INCIDENT-FORENSICS.md), which
//! applies a stricter evidence checklist and carries the expert
//! review fields. Those fields are left empty: a named reviewer completes them afterwards
//! with `kshana-pro forensics review`, which signs the review against the report's hash.
//! Nothing here fills in a reviewer.
//!
//! The report bundle is signed as a whole with the signed-dossier format
//! ([`crate::signed_dossier`]), so it verifies with `kshana-pro dossier verify` or
//! `kshana-pro forensics verify` under any licence or none, and with
//! `scripts/verify-dossier-signature.py`. Each receiver log also gets an open-engine evidence
//! pack inside the bundle, which the free open `kshana evidence verify` checks.
//!
//! **Delivery.** The kind `pro-incident-forensics` is a Pro pack kind (roadmap 0.42 C10): it
//! is listed at the `pro` edition in the signed tier manifest, but it is registered only when
//! a verified Pro pack whose signed manifest provides it is loaded
//! ([`ForensicsActivator`], [`crate::propack::loader::PackStore::load`]). Until then the kind
//! is refused with a message that says it comes in a Pro pack. Running it also needs a
//! licence tier that unlocks it ([`crate::license::check`]).
//!
//! **Honest limits.** The detection method is pre-registered (its parameters are constants of
//! [`METHOD_ID`] and are written into every report) but its detection bars on a labelled set
//! (roadmap V2) are not yet met: the report says so. A finding is an engineering assessment
//! of the data supplied, not a legal opinion or a finding of fact.

pub mod bundle;
pub mod detect;
pub mod dispute;
pub mod inputs;
pub mod logtime;
pub mod maritime;
pub mod render;

use std::path::{Path, PathBuf};

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::receiver_trust::LogFormat;
use serde::{Deserialize, Serialize};

use crate::extension::{ExtensionKind, PackExtension};
use crate::license::Tier;
use crate::propack::loader::{Activator, OpenedPack};

/// The scenario kind.
pub const KIND: &str = "pro-incident-forensics";
/// The Pro pack that provides it (the catalogue id in the release pipeline).
pub const PACK_ID: &str = "pro-forensics";
/// The method version. Every detection parameter is a constant of this version
/// ([`detect::MethodParams::V1`]); changing one means a new version.
pub const METHOD_ID: &str = "kshana-pro/incident-forensics/v1";
/// The report format tag.
pub const FORMAT: &str = "kshana-pro-forensic-report/1";
/// The payload entry a forensics pack must carry: the method it was built for.
pub const PACK_METHOD_ENTRY: &str = "forensics/method.json";

/// The honesty label every report carries.
pub const LABEL: &str =
    "MEASURED receiver observables, REPORTED broadcast positions and statements, \
MODELLED findings: each finding is the method's assessment of the data supplied, under thresholds \
fixed before the run. It is an engineering record, not a legal opinion, not a finding of fact and \
not a certification.";

/// The validation status every report states (roadmap V2).
pub const VALIDATION_STATUS: &str = "The method's pre-registered detection bars on a labelled set \
(roadmap V2: public events plus synthetic) are not yet met. The method has been exercised on the \
open engine's synthetic training scenarios only; no detection rate, false-alarm rate or accuracy \
is claimed for real incidents.";

/// Standard or claim-grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Variant {
    /// The standard forensics pack.
    #[default]
    Standard,
    /// The claim-grade pack: stricter evidence checklist and expert review fields.
    ClaimGrade,
}

impl Variant {
    pub fn as_str(self) -> &'static str {
        match self {
            Variant::Standard => "standard",
            Variant::ClaimGrade => "claim-grade",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Variant::Standard => "Standard forensics pack",
            Variant::ClaimGrade => "Claim-grade forensics pack",
        }
    }
}

/// What the incident is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subject {
    #[default]
    Vessel,
    Aircraft,
    Station,
    /// An area with no single subject.
    Area,
}

impl Subject {
    pub fn as_str(self) -> &'static str {
        match self {
            Subject::Vessel => "vessel",
            Subject::Aircraft => "aircraft",
            Subject::Station => "station",
            Subject::Area => "area",
        }
    }
}

/// A latitude/longitude box, degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AreaBox {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
}

impl AreaBox {
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lat >= self.lat_min && lat <= self.lat_max && lon >= self.lon_min && lon <= self.lon_max
    }
}

/// The incident as stated by whoever asked for the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incident {
    /// The case reference.
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub subject: Subject,
    /// The subject's identifier as it appears in the AIS or ADS-B data (vessel or aircraft id).
    #[serde(default)]
    pub subject_id: Option<String>,
    /// Window start, ISO-8601 UTC.
    pub start_utc: String,
    /// Window end, ISO-8601 UTC.
    pub end_utc: String,
    /// The area of interest. Without it, the area is every position in the data.
    #[serde(default)]
    pub area: Option<AreaBox>,
    /// What was reported about the incident, as given (labelled REPORTED).
    #[serde(default)]
    pub reported: Option<String>,
    /// Who asked for the report, as given.
    #[serde(default)]
    pub requested_by: Option<String>,
}

/// Where a receiver log comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiverRole {
    /// The subject's own receiver (the vessel's bridge GNSS, the aircraft's receiver).
    #[default]
    Subject,
    /// A fixed Kshana Station or other reference receiver near the area.
    Station,
    /// Any other receiver in the area.
    Other,
}

impl ReceiverRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ReceiverRole::Subject => "subject",
            ReceiverRole::Station => "station",
            ReceiverRole::Other => "other",
        }
    }
}

/// How the receiver moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// A fixed receiver (a Station).
    #[default]
    Static,
    /// A vessel (the moving-platform monitors run).
    Vessel,
}

/// One receiver log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverInput {
    pub label: String,
    pub path: String,
    pub format: LogFormat,
    #[serde(default)]
    pub role: ReceiverRole,
    #[serde(default)]
    pub platform: Platform,
    /// RINEX broadcast navigation file, for a `rinex` log.
    #[serde(default)]
    pub nav: Option<String>,
    /// Vessel limits (moving platform only); the method's defaults otherwise.
    #[serde(default)]
    pub max_speed_kn: Option<f64>,
    #[serde(default)]
    pub antenna_height_m: Option<f64>,
    /// Whether the stream carries a gyro heading (HDT). Default true for a vessel.
    #[serde(default)]
    pub heading_sensor: Option<bool>,
}

/// One AIS or ADS-B file in the documented CSV format of the open engine's interference map
/// (AIS: `timestamp, vessel_id, lat, lon[, sog_kn]`; ADS-B: `timestamp, aircraft_id, lat, lon,
/// alt_baro_ft, nic and/or nacp[, source_type]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackInput {
    pub label: String,
    pub path: String,
}

/// A statement by a third party (crew, NAVTEX, NOTAM, authority), recorded as given.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatementInput {
    pub source: String,
    /// When the statement says the event happened (or when it was made), ISO-8601 UTC.
    pub utc: String,
    pub text: String,
}

/// One chain-of-custody record for an input file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustodyEntry {
    /// The input's `label`.
    pub input: String,
    pub received_from: String,
    pub received_utc: String,
    /// How it was received (e-mail, courier, download from the operator's server, ...).
    pub method: String,
    /// The SHA-256 the sender stated, when they stated one (checked against the file).
    #[serde(default)]
    pub sender_sha256: Option<String>,
}

/// A `pro-incident-forensics` scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentScenario {
    pub kind: String,
    #[serde(default)]
    pub variant: Variant,
    pub incident: Incident,
    #[serde(default, rename = "receiver")]
    pub receivers: Vec<ReceiverInput>,
    #[serde(default)]
    pub ais: Vec<TrackInput>,
    #[serde(default)]
    pub adsb: Vec<TrackInput>,
    #[serde(default, rename = "statement")]
    pub statements: Vec<StatementInput>,
    #[serde(default)]
    pub custody: Vec<CustodyEntry>,
    /// Where relative paths are resolved (set by the command line from the file's folder;
    /// never written to a recipe).
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl IncidentScenario {
    /// Parse the TOML text of an incident.
    pub fn parse(src: &str) -> Result<Self, KshanaError> {
        let s: IncidentScenario = toml::from_str(src)
            .map_err(|e| KshanaError::InvalidInput(format!("invalid {KIND} scenario: {e}")))?;
        if s.kind != KIND {
            return Err(KshanaError::InvalidInput(format!(
                "kind must be '{KIND}', got '{}'",
                s.kind
            )));
        }
        Ok(s)
    }

    /// Parse a file; relative input paths are resolved from its folder.
    pub fn from_file(path: &Path) -> Result<Self, KshanaError> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
        let mut s = Self::parse(&src)?;
        s.base_dir = path.parent().map(Path::to_path_buf);
        Ok(s)
    }

    /// The path of an input, resolved against [`IncidentScenario::base_dir`].
    pub fn resolve(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        match (&self.base_dir, pb.is_absolute()) {
            (Some(b), false) => b.join(pb),
            _ => pb,
        }
    }

    /// Static metadata.
    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description:
                "Incident forensics (E1): for a vessel, aircraft or Station and a time window, \
whether there was jamming-like or spoofing-like interference, when and where, how confident, \
from which inputs (hashed), with a reproducible recipe; standard or claim-grade.",
            required_fields: &["incident"],
            optional_fields: &["variant", "receiver", "ais", "adsb", "statement", "custody"],
        }
    }
}

impl Scenario for IncidentScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let report = detect::analyse(self, None)?;
        let json = serde_json::to_string_pretty(&report)
            .map_err(|e| KshanaError::InvalidInput(format!("report: {e}")))?;
        Ok(RunOutput {
            summary: report.verdict.sentence.clone(),
            svg: render::timeline_svg(&report),
            csv: Some(render::timeline_csv(&report)),
            json,
        })
    }
}

impl ExternalPack for IncidentScenario {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
}

fn build(src: &str) -> Result<Box<dyn ExternalPack>, KshanaError> {
    IncidentScenario::parse(src).map(|s| Box::new(s) as Box<dyn ExternalPack>)
}

fn register_into(reg: &mut kshana::registry::PackRegistry) {
    reg.register(crate::registry::pro_factory::<IncidentScenario>(KIND));
}

/// What the forensics pack adds once verified: the kind, at the `pro` edition and the Pro
/// licence tier.
pub fn pack_extension(pack_id: &str) -> PackExtension {
    PackExtension {
        pack_id: pack_id.to_string(),
        kinds: vec![ExtensionKind {
            meta: IncidentScenario::meta_static(),
            min_tier: Tier::Pro,
            build,
            register_into,
        }],
        features: vec![],
    }
}

/// The method record a forensics pack carries at [`PACK_METHOD_ENTRY`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackMethod {
    pub kind: String,
    pub method_id: String,
    /// SHA-256 of the method parameters ([`detect::MethodParams::sha256`]).
    pub params_sha256: String,
}

impl PackMethod {
    /// The record for this build's method.
    pub fn current() -> PackMethod {
        PackMethod {
            kind: KIND.into(),
            method_id: METHOD_ID.into(),
            params_sha256: detect::MethodParams::V1.sha256(),
        }
    }
}

/// The host side of the forensics pack: checks that the verified payload is a forensics pack
/// for this build's method and registers the kind. The pack's signature, device binding,
/// licence state and payload hash were already checked by the loader before this runs.
pub struct ForensicsActivator;

impl Activator for ForensicsActivator {
    fn activate(&self, pack: &OpenedPack, unpack_dir: &Path) -> Result<PackExtension, KshanaError> {
        if !pack.manifest.provides.iter().any(|p| p == KIND) {
            return Err(KshanaError::InvalidInput(format!(
                "pack {} does not provide {KIND}",
                pack.manifest.pack_id
            )));
        }
        let raw = pack.payload.get(PACK_METHOD_ENTRY).ok_or_else(|| {
            KshanaError::InvalidInput(format!(
                "pack {} has no {PACK_METHOD_ENTRY}",
                pack.manifest.pack_id
            ))
        })?;
        let m: PackMethod = serde_json::from_slice(raw)
            .map_err(|e| KshanaError::InvalidInput(format!("{PACK_METHOD_ENTRY}: {e}")))?;
        if m != PackMethod::current() {
            return Err(KshanaError::InvalidInput(format!(
                "pack {} is for method {} ({}), this build runs {METHOD_ID}: update the app or the pack",
                pack.manifest.pack_id, m.method_id, m.params_sha256
            )));
        }
        std::fs::write(unpack_dir.join("method.json"), raw)
            .map_err(|e| KshanaError::InvalidInput(format!("unpack: {e}")))?;
        Ok(pack_extension(&pack.manifest.pack_id))
    }
}

/// Whether the forensics kind is registered in this process (a verified pack was loaded).
pub fn is_loaded() -> bool {
    crate::extension::pack_owns(KIND)
}

/// Load every installed Pro pack in the app data folder `app_data` that provides the
/// forensics kind, under the licence status `licence` on the machine `device` (the hash of
/// [`crate::license::machine::this_machine_hash`]). Returns whether the kind is now
/// registered; the loader's refusals (signature, device, revocation, ended licence,
/// tampering) come back as the error of the last pack tried.
pub fn load_installed(
    app_data: &Path,
    licence: &crate::license::activation::Status,
    device: &str,
    now: i64,
) -> Result<bool, KshanaError> {
    if is_loaded() {
        return Ok(true);
    }
    let store = crate::propack::loader::PackStore::in_app_data(app_data);
    let keys = crate::propack::loader::embedded_pack_keys();
    let ctx = crate::propack::loader::LoadContext {
        keys: &keys,
        device,
        licence: Some(licence),
        revocations: &[],
        now,
    };
    let mut last: Option<KshanaError> = None;
    for id in store.installed() {
        match store.phase(&id, &ctx) {
            Ok((m, _)) if m.provides.iter().any(|p| p == KIND) => {}
            Ok(_) => continue,
            Err(e) => {
                last = Some(e);
                continue;
            }
        }
        match store.load(&id, &ctx, &ForensicsActivator) {
            Ok(_) => return Ok(true),
            Err(e) => last = Some(e),
        }
    }
    match last {
        Some(e) => Err(e),
        None => Ok(false),
    }
}

/// The payload files of a forensics pack, for the release pipeline (`packs build --from`):
/// the method record. Write them into the payload folder next to the native code.
pub fn pack_payload_files() -> Vec<(String, Vec<u8>)> {
    let m = serde_json::to_vec_pretty(&PackMethod::current()).unwrap_or_default();
    vec![(PACK_METHOD_ENTRY.to_string(), m)]
}
