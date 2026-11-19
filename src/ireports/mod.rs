// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Interference reports: from crew, controller and mariner reports to verified, signed
//! evidence (research 05 G1/G12/G13, 07 G5, 07b Q1/Q15).
//!
//! Authorities count only what crews report, and every authority asks for it in another
//! shape. This module turns those shapes into one versioned record and builds on it:
//!
//! * [`normalise`]: DGCA-style forms, EASA occurrence-report (ECR/ECCAIRS-like) forms,
//!   ASRS-like de-identified narratives, NOTAM text, ship log entries and CSV exports into
//!   [`Report`] (`kshana-pro-interference-report/1`).
//! * [`validate`]: the mandatory fields of each regime (DGCA 10-minute rule, EU 376/2014
//!   72-hour rule, ASRS, a maritime profile and a generic one), flagged per report.
//! * [`episodes`]: reports and ADS-B NIC/NACp degradations clustered into episodes with
//!   explicit, configurable start and end rules.
//! * [`verify`]: one report cross-checked against receiver logs, ADS-B and AIS, labelled
//!   CORROBORATED / UNCORROBORATED / CONTRADICTED with reasons, signed.
//! * [`ledger`]: a per-aircraft or per-vessel exposure ledger for maintenance planning, and
//!   the post-spoofing "reset recommended" check (MODELLED, a recommendation only).
//!
//! Licence features (explicit tiers in `license::explicit_tier`): normalising, validating
//! and episodes need [`FEATURE_REPORTS`] (Pro); verification and the reset check need
//! [`FEATURE_VERIFICATION`] (Pro); the fleet exposure ledger needs [`FEATURE_LEDGER`]
//! (Enterprise). Checking a signed directory needs none.
//!
//! Detection, evidence and forensics on civil signals only: nothing here radiates, and no
//! emitter is located.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod episodes;
pub mod ledger;
pub mod normalise;
pub mod synth;
pub mod text;
pub mod validate;
pub mod verify;

/// Normalise, validate and build episodes.
pub const FEATURE_REPORTS: &str = "pro-interference-reports";
/// Cross-check a report against observations (signed record) and the post-spoofing reset check.
pub const FEATURE_VERIFICATION: &str = "pro-report-verification";
/// The per-aircraft or per-vessel exposure ledger of a fleet.
pub const FEATURE_LEDGER: &str = "pro-exposure-ledger";

/// The record format of one normalised report.
pub const REPORT_FORMAT: &str = "kshana-pro-interference-report/1";
/// The document holding a set of normalised reports.
pub const REPORTS_FORMAT: &str = "kshana-pro-interference-reports/1";
/// The schema version of [`Report`]; a field change that breaks readers raises it.
pub const SCHEMA_VERSION: u32 = 1;
/// The normaliser's method id.
pub const NORMALISER_ID: &str = "kshana-pro/interference-report-normaliser/v1";

/// The kind of interference, as far as the report says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Jamming,
    Spoofing,
    Unknown,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Jamming => "jamming",
            Kind::Spoofing => "spoofing",
            Kind::Unknown => "unknown",
        }
    }
}

/// The type the reporter stated (the DGCA's four types plus what other forms use).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportedType {
    Jamming,
    Spoofing,
    SignalLoss,
    IntegrityError,
    InterferenceUnspecified,
    /// A NOTAM announcing that GNSS may be unavailable or unreliable.
    AnnouncedOutage,
    NotStated,
}

impl ReportedType {
    /// The kind it implies: only an explicit jamming or spoofing statement sets one.
    pub fn kind(self) -> Kind {
        match self {
            ReportedType::Jamming => Kind::Jamming,
            ReportedType::Spoofing => Kind::Spoofing,
            _ => Kind::Unknown,
        }
    }
}

/// Who reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReporterRole {
    Pilot,
    Crew,
    Atc,
    Mariner,
    Authority,
    Notam,
    NotStated,
}

/// What the report is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Aircraft,
    Vessel,
    Ground,
    /// An area (a NOTAM).
    Area,
    NotStated,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Aircraft => "aircraft",
            Platform::Vessel => "vessel",
            Platform::Ground => "ground",
            Platform::Area => "area",
            Platform::NotStated => "not_stated",
        }
    }
}

/// The shape a report came in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    /// A key-value form with the content India's DGCA asks for (10 Nov 2025 circular).
    Dgca,
    /// A key-value occurrence report with EASA ECR/ECCAIRS-like field names.
    Eccairs,
    /// An ASRS-like de-identified record: coarse date and time, narrative.
    Asrs,
    /// ICAO NOTAM text (Q) to G) items).
    Notam,
    /// Ship log entries, one per dated line.
    ShipLog,
    /// A CSV export with a header row.
    Csv,
}

impl SourceFormat {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "dgca" => SourceFormat::Dgca,
            "eccairs" | "easa" | "ecr" => SourceFormat::Eccairs,
            "asrs" => SourceFormat::Asrs,
            "notam" => SourceFormat::Notam,
            "shiplog" | "ship-log" => SourceFormat::ShipLog,
            "csv" => SourceFormat::Csv,
            _ => return None,
        })
    }
}

/// For a spoofing report: which layer the reporter says was spoofed (the RIN survey: 36 % of
/// mariners are not confident telling AIS spoofing from GNSS spoofing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Gnss,
    Ais,
    Both,
    NotStated,
}

/// Where the report came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub format: SourceFormat,
    pub file: String,
    pub sha256: String,
    /// Record number in the file, from 1.
    pub record: usize,
}

/// A reported position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Radius the report gives (a NOTAM's area), metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius_m: Option<f64>,
    /// The text it was read from.
    pub text: String,
}

/// A reported altitude or band, feet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Altitude {
    pub lower_ft: Option<f64>,
    pub upper_ft: Option<f64>,
    /// `FL`, `AMSL`, `AGL`, `SFC` or `unstated`.
    pub reference: String,
    pub text: String,
}

/// One normalised interference report (`kshana-pro-interference-report/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub format: String,
    pub schema_version: u32,
    /// `rep-` and 12 hex characters of the SHA-256 of the source hash and record number.
    pub id: String,
    pub source: Source,
    pub reporter_role: ReporterRole,
    #[serde(default)]
    pub reporter: Option<String>,
    pub platform: Platform,
    /// Aircraft registration, IMO or MMSI number, or a NOTAM number.
    #[serde(default)]
    pub subject_id: Option<String>,
    /// `registration`, `imo`, `mmsi`, `icao24` or `notam`.
    #[serde(default)]
    pub subject_id_type: Option<String>,
    #[serde(default)]
    pub flight_or_voyage: Option<String>,
    #[serde(default)]
    pub platform_type: Option<String>,
    /// When it started (UTC, ISO 8601).
    #[serde(default)]
    pub occurred_start_utc: Option<String>,
    #[serde(default)]
    pub occurred_end_utc: Option<String>,
    /// How far the stated start may be from the true one, seconds (60 for a time given to
    /// the minute; hours for a de-identified ASRS-like record).
    pub time_uncertainty_s: f64,
    /// When it was reported (UTC).
    #[serde(default)]
    pub reported_utc: Option<String>,
    #[serde(default)]
    pub position: Option<Position>,
    #[serde(default)]
    pub altitude: Option<Altitude>,
    pub kind: Kind,
    pub reported_type: ReportedType,
    pub spoofed_layer: Layer,
    /// Normalised system names (`GNSS receiver`, `FMS`, `ADS-B`, `TAWS/EGPWS`, `AIS`, ...).
    pub systems_affected: Vec<String>,
    #[serde(default)]
    pub recovery_action: Option<String>,
    #[serde(default)]
    pub time_to_recover_min: Option<f64>,
    #[serde(default)]
    pub fms_screenshot: Option<bool>,
    #[serde(default)]
    pub logs_preserved: Option<bool>,
    #[serde(default)]
    pub narrative: Option<String>,
    /// Fields of the source that have no place in the schema, kept as given.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
    /// What the normaliser inferred or could not read.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Report {
    /// Start, Unix seconds.
    pub fn start_t(&self) -> Option<f64> {
        self.occurred_start_utc.as_deref().and_then(text::parse_utc)
    }

    /// End, Unix seconds: the stated end, else start plus the time to recover, else start
    /// plus `default_min`.
    pub fn end_t(&self, default_min: f64) -> Option<f64> {
        let s = self.start_t()?;
        if let Some(e) = self.occurred_end_utc.as_deref().and_then(text::parse_utc) {
            if e >= s {
                return Some(e);
            }
        }
        Some(s + 60.0 * self.time_to_recover_min.unwrap_or(default_min).max(0.0))
    }

    /// Reported time, Unix seconds.
    pub fn reported_t(&self) -> Option<f64> {
        self.reported_utc.as_deref().and_then(text::parse_utc)
    }
}

/// A set of normalised reports (`kshana-pro-interference-reports/1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportsDoc {
    pub format: String,
    pub schema_version: u32,
    pub normaliser: String,
    pub reports: Vec<Report>,
    /// Validation per regime, when asked for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation: Vec<validate::Validation>,
}

impl ReportsDoc {
    pub fn new(reports: Vec<Report>) -> Self {
        ReportsDoc {
            format: REPORTS_FORMAT.into(),
            schema_version: SCHEMA_VERSION,
            normaliser: NORMALISER_ID.into(),
            reports,
            validation: vec![],
        }
    }

    /// Read a reports document; a single report is accepted too.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        match v.get("format").and_then(|f| f.as_str()) {
            Some(REPORTS_FORMAT) => serde_json::from_value(v).map_err(|e| e.to_string()),
            Some(REPORT_FORMAT) => Ok(ReportsDoc::new(vec![
                serde_json::from_value(v).map_err(|e| e.to_string())?
            ])),
            other => Err(format!(
                "not a normalised reports file (format {other:?}; expected {REPORTS_FORMAT})"
            )),
        }
    }
}

/// The SHA-256 of `b`, lower-case hex.
pub fn sha256_hex(b: &[u8]) -> String {
    crate::forensics::inputs::sha256_hex(b)
}

/// The SHA-256 of a parameter set's canonical JSON.
pub fn params_hash<T: Serialize>(p: &T) -> String {
    sha256_hex(serde_json::to_string(p).unwrap_or_default().as_bytes())
}

/// An input file recorded in an output: its name and SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFile {
    pub role: String,
    pub file: String,
    pub sha256: String,
}

/// Input files to copy into an output directory: (name, bytes).
pub type Copies = Vec<(String, Vec<u8>)>;

/// Write `files` (name, bytes) into `<dir>/inputs/`, byte for byte.
pub fn write_inputs(dir: &std::path::Path, files: &[(String, Vec<u8>)]) -> Result<(), String> {
    let d = dir.join("inputs");
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    for (name, bytes) in files {
        let p = d.join(safe_name(name));
        std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

/// A file name with no directory part (inputs are copied flat).
pub fn safe_name(path: &str) -> String {
    let n = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "input".into());
    if n.is_empty() || n == "." || n == ".." {
        "input".into()
    } else {
        n
    }
}

/// Pretty JSON with a final newline.
pub fn json_text<T: Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default() + "\n"
}
