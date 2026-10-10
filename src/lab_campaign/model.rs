//! Serde mirror types of the engine's lab-replay results.
//!
//! The contract is the public `kshana` lab-replay campaign (0.34.0), as implemented on branch
//! `kshana` 0.34.0 (crates.io; developed on `feat/034-campaign`, PR #46, last head `51ba4787`): `src/iq/campaign/{runner,score,report}.rs`. Every struct
//! rejects unknown fields, and every document's `schema` string is checked against the lists
//! below. A change in the engine's output therefore fails loudly here instead of being misread.
//! Re-pin to the released engine at 0.34.0.
//!
//! The inputs the engine resolves and carries verbatim (test conditions, front-end chains,
//! loop designs, scoring settings) are kept as JSON values: the report prints them and never
//! interprets them, and the engine owns their schemas.

use serde::{Deserialize, Serialize};

/// Cell schema versions this overlay can read.
pub const CELL_SCHEMAS: &[&str] = &["kshana.campaign-cell/1"];
/// Resolved-campaign (`campaign.json`) schema versions this overlay can read.
pub const PLAN_SCHEMAS: &[&str] = &["kshana.campaign-plan/1"];
/// Scorecard schema versions this overlay can read.
pub const SCORECARD_SCHEMAS: &[&str] = &["kshana.campaign-scorecard/1"];
/// The C/N0 estimators the engine can score with (`[scoring] cn0_estimator`).
pub const CN0_ESTIMATORS: &[&str] = &["m2m4", "nwpr"];
/// Lock sources this overlay can describe (`CellResult::lock_source`).
pub const LOCK_SOURCES: &[&str] = &["track-session"];
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
    /// The engine's spelling (`synthetic`, `client-confidential`), as it enters the cell key.
    pub fn as_str(self) -> &'static str {
        match self {
            DataClass::Synthetic => "synthetic",
            DataClass::ClientConfidential => "client-confidential",
        }
    }

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

/// The recording a cell ran on (`RecordingStamp`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellRecording {
    pub id: String,
    pub sha256: String,
    pub conditions_hash: String,
}

/// How tracking was handed off for one satellite (`Handoff`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub source: String,
    pub doppler_hz: Option<f64>,
    pub code_phase_chips: Option<f64>,
    pub statistic: Option<f64>,
    pub threshold: Option<f64>,
}

/// Whole-run metrics of one satellite in one cell (`WholeRunScore`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WholeRun {
    pub scored_s: f64,
    pub locked_s: f64,
    pub availability: Option<f64>,
    pub phase_lock_frac: Option<f64>,
    pub code_lock_frac: Option<f64>,
    /// By the campaign's `cn0_estimator`; both estimates follow.
    pub median_cn0_dbhz: Option<f64>,
    pub median_cn0_nwpr_dbhz: Option<f64>,
    pub median_cn0_m2m4_dbhz: Option<f64>,
    pub pll_jitter_deg: Option<f64>,
    pub dll_jitter_chips: Option<f64>,
    pub false_lock_episodes: u64,
    pub false_lock_per_hour: Option<f64>,
    pub loss_count: u64,
    pub reacq_count: u64,
}

/// One J/S bin of the C/N0 degradation curve (`Cn0Bin`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cn0Bin {
    pub js_db: f64,
    pub n: u64,
    pub measured_cn0_dbhz: Option<f64>,
    pub measured_cn0_nwpr_dbhz: Option<f64>,
    pub measured_cn0_m2m4_dbhz: Option<f64>,
    pub measured_degradation_db: Option<f64>,
    pub modelled_cn0_dbhz: Option<f64>,
    pub modelled_degradation_db: Option<f64>,
}

/// The analytic reference attached to an interference event's curve (`ModelledReference`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modelled {
    pub label: String,
    pub q: f64,
    pub q_source: String,
    pub chip_rate_hz: f64,
    pub formula: String,
}

/// Metrics of one satellite inside one stated event (`EventScore`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventResult {
    pub event_id: String,
    /// `interference`, `spoofing`, `outage` or `other`.
    pub kind: String,
    /// The lab's stated type label (`cw`, `broadband`, …).
    #[serde(rename = "type")]
    pub event_type: String,
    pub onset_s: f64,
    pub offset_s: f64,
    pub locked_at_onset: bool,
    pub lost: bool,
    pub time_to_loss_s: Option<f64>,
    pub js_at_loss_db: Option<f64>,
    pub reacquired: bool,
    pub reacq_time_s: Option<f64>,
    pub outage_s: Option<f64>,
    pub availability: Option<f64>,
    pub baseline_cn0_dbhz: Option<f64>,
    pub baseline_cn0_nwpr_dbhz: Option<f64>,
    pub baseline_cn0_m2m4_dbhz: Option<f64>,
    pub baseline_source: Option<String>,
    pub baseline_pll_jitter_deg: Option<f64>,
    pub baseline_dll_jitter_chips: Option<f64>,
    pub pll_jitter_deg: Option<f64>,
    pub dll_jitter_chips: Option<f64>,
    pub false_lock_episodes: u64,
    pub cn0_curve: Vec<Cn0Bin>,
    pub modelled: Option<Modelled>,
}

/// One satellite's results in one cell (`CellSat`: the hand-off plus the flattened `SatScore`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SatelliteResult {
    pub handoff: Handoff,
    pub signal: String,
    /// PRN, or the GLONASS frequency channel (may be negative).
    pub id: i64,
    pub whole_run: WholeRun,
    pub events: Vec<EventResult>,
}

/// A cell's kept tracking output (`EpochFiles`), present only when the campaign keeps epochs.
/// Paths are relative to the campaign's output directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EpochFiles {
    /// `csv`, `jsonl` or `binary` (`kshana.track-epoch/1`).
    pub format: String,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    /// The lock-event file (JSON Lines).
    pub events_path: String,
    pub events_sha256: String,
}

/// `cells/<key>.json`: one (recording, front-end chain, loop design) (`CellResult`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignCell {
    pub schema: String,
    pub key: String,
    /// What the data is; part of the cell key.
    pub data_class: DataClass,
    pub engine_version: String,
    pub recording: CellRecording,
    pub frontend: NamedHash,
    pub design: NamedHash,
    pub run_hash: String,
    pub scoring_hash: String,
    /// How the lock state was derived (`track-session`).
    pub lock_source: String,
    /// The C/N0 estimator behind the cell's reported C/N0 (`m2m4` or `nwpr`).
    pub cn0_estimator: String,
    pub sample_rate_hz: f64,
    pub samples_processed: u64,
    pub satellites: Vec<SatelliteResult>,
    /// The per-update tracking output, when the campaign keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epochs: Option<EpochFiles>,
}

/// A recording in the resolved campaign (`PlannedRecording`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestRecording {
    pub id: String,
    /// The test-condition file, as the campaign named it.
    pub conditions_file: String,
    /// The recording file, as the conditions named it.
    pub path: String,
    pub sha256: String,
    pub conditions_hash: String,
    /// The resolved `kshana.test-conditions/1` document (events, J/S profiles, receiver text).
    pub conditions: serde_json::Value,
}

/// One planned cell: indices into the campaign's recordings, chains and designs (`PlannedCell`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedCell {
    pub key: String,
    pub recording: usize,
    pub frontend: usize,
    pub design: usize,
}

/// Pass/fail bars, as the engine's `Bars` (`iq/campaign/score.rs`): every bar optional, unknown
/// fields refused. A campaign states them at `scoring.bars`; a recording may override any of them
/// at `conditions.bars`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bars {
    pub min_time_to_loss_s: Option<f64>,
    pub max_reacq_s: Option<f64>,
    pub min_availability: Option<f64>,
    pub min_event_availability: Option<f64>,
    pub max_false_lock_per_hour: Option<f64>,
    pub max_pll_jitter_deg: Option<f64>,
    pub max_dll_jitter_chips: Option<f64>,
}

/// Which way a bar is strict: a `Min` bar is looser when smaller, a `Max` bar when larger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarDir {
    Min,
    Max,
}

/// Every bar and its direction (the engine's `Bars` fields).
pub const BAR_DIRECTIONS: &[(&str, BarDir)] = &[
    ("min_time_to_loss_s", BarDir::Min),
    ("max_reacq_s", BarDir::Max),
    ("min_availability", BarDir::Min),
    ("min_event_availability", BarDir::Min),
    ("max_false_lock_per_hour", BarDir::Max),
    ("max_pll_jitter_deg", BarDir::Max),
    ("max_dll_jitter_chips", BarDir::Max),
];

/// `campaign.json`: the resolved campaign (`Plan`, `kshana.campaign-plan/1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignManifest {
    pub schema: String,
    pub name: String,
    pub engine_version: String,
    /// What the data is (required by the engine; copied into every cell and its key).
    pub data_class: DataClass,
    pub recordings: Vec<ManifestRecording>,
    /// `[front-end spec, hash]` pairs; the spec carries `name` and the stage parameters.
    pub frontends: Vec<(serde_json::Value, String)>,
    /// Resolved loop designs, each with `name` and `hash`.
    pub designs: Vec<serde_json::Value>,
    pub run_hash: String,
    /// Resolved scoring settings, including any `bars`.
    pub scoring: serde_json::Value,
    pub scoring_hash: String,
    /// Every cell of the full product, in the engine's fixed order.
    pub cells: Vec<PlannedCell>,
}

impl CampaignManifest {
    /// The name of the front-end chain at `i`.
    pub fn frontend_name(&self, i: usize) -> Option<&str> {
        self.frontends
            .get(i)
            .and_then(|(spec, _)| spec.get("name")?.as_str())
    }

    /// The hash of the front-end chain at `i`.
    pub fn frontend_hash(&self, i: usize) -> Option<&str> {
        self.frontends.get(i).map(|(_, h)| h.as_str())
    }

    /// The name of the loop design at `i`.
    pub fn design_name(&self, i: usize) -> Option<&str> {
        self.designs.get(i)?.get("name")?.as_str()
    }

    /// The hash of the loop design at `i`.
    pub fn design_hash(&self, i: usize) -> Option<&str> {
        self.designs.get(i)?.get("hash")?.as_str()
    }
}

/// One row of `scorecard.json` (`Row`): the engine's own verdict against the client's bars.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreRow {
    pub recording: String,
    pub frontend: String,
    pub design: String,
    pub signal: String,
    pub sat: i64,
    /// `run` (whole run) or `event`.
    pub scope: String,
    pub event_id: String,
    pub event_type: String,
    pub handoff: String,
    pub availability: Option<f64>,
    pub time_to_loss_s: Option<f64>,
    pub js_at_loss_db: Option<f64>,
    pub reacq_time_s: Option<f64>,
    pub outage_s: Option<f64>,
    pub cn0_dbhz: Option<f64>,
    /// The estimator behind `cn0_dbhz` (`m2m4` or `nwpr`).
    pub cn0_estimator: String,
    pub cn0_nwpr_dbhz: Option<f64>,
    pub cn0_m2m4_dbhz: Option<f64>,
    pub pll_jitter_deg: Option<f64>,
    pub dll_jitter_chips: Option<f64>,
    pub false_lock_episodes: u64,
    pub false_lock_per_hour: Option<f64>,
    /// `PASS`, `FAIL`, or empty when no bar applies.
    pub verdict: String,
    pub failed_bars: Vec<String>,
    /// The cell key the row comes from.
    pub cell: String,
}

/// `scorecard.json` (`kshana.campaign-scorecard/1`). Only the fields the report reads are
/// typed; the curves duplicate the cells and are not read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scorecard {
    pub schema: String,
    pub campaign: String,
    pub data_class: DataClass,
    pub engine_version: String,
    pub cells_total: usize,
    pub cells_present: usize,
    pub complete: bool,
    pub digest: Option<String>,
    pub rows: Vec<ScoreRow>,
}
