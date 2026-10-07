//! Serde mirror types of the engine's lab-replay results.
//!
//! The contract is public `kshana` `docs/design/LAB-CAMPAIGN.md` (0.34.0 "Lab replay",
//! proposal @ `f69efe9`). Every struct rejects unknown fields and every document's `schema`
//! string is checked against [`CELL_SCHEMAS`] / [`CAMPAIGN_SCHEMAS`], so a change in the
//! engine's output fails loudly here instead of being misread.
//!
//! [`CampaignManifest`] (the resolved `campaign.json`) is **provisional**: the design note
//! does not yet fix its fields. It mirrors what the report needs, and the request is with the
//! engine side to carry exactly these (resolved designs, resolved conditions, `data_class`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Cell schema versions this overlay can read.
pub const CELL_SCHEMAS: &[&str] = &["kshana.campaign-cell/1"];
/// Resolved-campaign schema versions this overlay can read.
pub const CAMPAIGN_SCHEMAS: &[&str] = &["kshana.campaign/1"];
/// The only label the engine may put on the analytic reference curve.
pub const MODELLED_LABEL: &str = "MODELLED";

/// The data class of a campaign. A report states it on every page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataClass {
    /// A campaign over Kshana-generated scenes. The only class allowed in this repository.
    Synthetic,
    /// A campaign over a client's recordings. Never committed (export-control item N6).
    ClientConfidential,
}

impl DataClass {
    /// The banner text.
    pub fn label(self) -> &'static str {
        match self {
            DataClass::Synthetic => "SYNTHETIC",
            DataClass::ClientConfidential => "CLIENT-CONFIDENTIAL",
        }
    }
}

/// A named, hashed input (front-end chain or loop design) as a cell records it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedHash {
    pub name: String,
    pub hash: String,
}

/// The recording a cell ran on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellRecording {
    pub id: String,
    pub sha256: String,
    pub conditions_hash: String,
}

/// Whole-run metrics of one satellite in one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WholeRun {
    pub availability: f64,
    pub locked_s: f64,
    pub phase_lock_frac: f64,
    pub code_lock_frac: f64,
    pub baseline_cn0_dbhz: Option<f64>,
    pub pll_jitter_deg: Option<f64>,
    pub dll_jitter_chips: Option<f64>,
    pub false_lock_episodes: u64,
    pub false_lock_per_hour: Option<f64>,
    pub loss_count: u64,
    pub reacq_count: u64,
}

/// One J/S bin of the C/N0 degradation curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cn0Bin {
    pub js_db: f64,
    pub n: u64,
    pub measured_cn0_dbhz: Option<f64>,
    pub measured_degradation_db: Option<f64>,
    pub modelled_cn0_dbhz: Option<f64>,
    pub modelled_degradation_db: Option<f64>,
}

/// The analytic reference attached to an interference event's curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modelled {
    pub label: String,
    pub q: f64,
    pub chip_rate_hz: f64,
    pub formula: String,
}

/// Metrics of one satellite inside one stated event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventResult {
    pub event_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub lost: bool,
    pub time_to_loss_s: Option<f64>,
    pub js_at_loss_db: Option<f64>,
    pub reacquired: Option<bool>,
    pub reacq_time_s: Option<f64>,
    pub availability: f64,
    pub pll_jitter_deg: Option<f64>,
    pub dll_jitter_chips: Option<f64>,
    pub false_lock_episodes: u64,
    #[serde(default)]
    pub cn0_curve: Vec<Cn0Bin>,
    #[serde(default)]
    pub modelled: Option<Modelled>,
    #[serde(default)]
    pub pass: BTreeMap<String, bool>,
}

/// One satellite's results in one cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SatelliteResult {
    pub signal: String,
    /// PRN, or the GLONASS frequency channel (may be negative).
    pub id: i64,
    pub whole_run: WholeRun,
    #[serde(default)]
    pub events: Vec<EventResult>,
}

/// `cells/<key>.json`: one (recording, front-end chain, loop design).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignCell {
    pub schema: String,
    pub key: String,
    pub engine_version: String,
    pub recording: CellRecording,
    pub frontend: NamedHash,
    pub design: NamedHash,
    pub run_hash: String,
    pub scoring_hash: String,
    pub samples_processed: u64,
    pub satellites: Vec<SatelliteResult>,
}

/// A recording as the resolved campaign describes it (PROVISIONAL shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestRecording {
    pub id: String,
    pub sha256: String,
    pub conditions_hash: String,
    /// The resolved `kshana.test-conditions/1` document (events, J/S profiles, receiver text).
    pub conditions: serde_json::Value,
}

/// A front-end chain or loop design with its resolved parameters (PROVISIONAL shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestInput {
    pub name: String,
    pub hash: String,
    pub params: serde_json::Value,
}

/// `campaign.json`: the resolved campaign (PROVISIONAL shape, see the module note).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignManifest {
    pub schema: String,
    pub name: String,
    pub engine_version: String,
    #[serde(default)]
    pub data_class: Option<DataClass>,
    pub recordings: Vec<ManifestRecording>,
    pub frontends: Vec<ManifestInput>,
    pub designs: Vec<ManifestInput>,
    pub run_hash: String,
    pub scoring_hash: String,
    /// The resolved `[scoring]` table, including any `[scoring.bars]`.
    pub scoring: serde_json::Value,
    /// Every cell key of the full product, in the engine's fixed order.
    pub cells: Vec<String>,
}
