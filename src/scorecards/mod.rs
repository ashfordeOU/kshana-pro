// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Referee scorecards and the receiver resilience test report (roadmap 0.42, E3)
//!
//! A **test campaign** puts one receiver through a set of threat cases and scores how it
//! behaved, under bars written into the campaign before it is scored:
//!
//! * a **library case** takes a scenario of the threat library (E2) and scores its
//!   analogue receiver stream (the open engine's NMEA receiver model as the unit under test,
//!   with the analogue's true track and time as truth). This is how a public sample is made;
//! * a **log case** takes the log the receiver under test wrote while a simulator replayed a
//!   threat (for example a threat library test-bench export) or during a range test, the
//!   events stated before scoring (or taken from a library scenario), and optionally the
//!   true motion as an I4 motion CSV.
//!
//! Every log is read and assessed by the open engine (`kshana::receiver_trust`: its readers,
//! its monitors and its event scoring); this module adds the receiver-level measures (fix
//! availability, error against truth, misleading output without warning, recovery), the
//! mapping to resilience levels ([`levels`]) and to the IEEE P1952 draft topics, and the
//! signed scorecard and test report ([`report`]).
//!
//! **Frozen format.** The scorecard JSON is format [`FORMAT`]: its fields are listed in
//! [`SCORECARD_FIELDS`] and [`CASE_FIELDS`], and a test fails if the serialised scorecard
//! drifts from them. A change of field means a new format version.
//!
//! **Wording.** Results are test evidence **mapped to** a framework; a scorecard never says a
//! receiver is compliant, conformant or certified.

pub mod levels;
pub mod report;

use std::path::{Path, PathBuf};

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use kshana::interference_map::grid::haversine_m;
use kshana::receiver_trust::scenario::{
    run_receiver_trust_bytes, EventCfg, ReceiverTrustResult, ReceiverTrustScenario,
};
use kshana::receiver_trust::{LogFormat, Timeline};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use levels::{CaseLevels, LevelMapping, Verdict};

/// The scenario kind.
pub const KIND: &str = "pro-receiver-scorecard";
/// The frozen scorecard format.
pub const FORMAT: &str = "kshana-pro-receiver-scorecard/1";
/// The scoring method; every rule below is a constant of this version.
pub const METHOD_ID: &str = "kshana-pro/receiver-scorecard/v1";
/// Method v2 (E3b pre-registration, `docs/evidence/e3-receiver-scorecard/2026-11-16_E3b_PREREGISTRATION.md`):
/// v1 except that the position error is the WGS-84 geodesic distance (Karney 2013, the open
/// engine's `kshana::geodesic`), and each epoch is stamped with its arrival time on a clock
/// independent of the unit under test, the truth taken at that time. For an analogue the arrival
/// time is the generator's epoch time; a log case carries no host arrival timestamps in this
/// campaign format, so v2 does not score its errors (and says so).
pub const METHOD_ID_V2: &str = "kshana-pro/receiver-scorecard/v2";

/// Which scoring method a campaign asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    V1,
    V2,
}

impl Method {
    pub fn id(self) -> &'static str {
        match self {
            Method::V1 => METHOD_ID,
            Method::V2 => METHOD_ID_V2,
        }
    }
}

/// The honesty label of every scorecard.
pub const LABEL: &str = "MODELLED verdicts of the open receiver-trust engine and of this \
scorecard, from MEASURED receiver logs (fix, flags, C/N0), scored under bars stated before \
scoring. Test evidence for the cases run, under the conditions stated; not a statement about \
cases not run, and not a certification.";

/// The top-level fields of a scorecard, in order (frozen for [`FORMAT`]).
pub const SCORECARD_FIELDS: [&str; 15] = [
    "format",
    "kind",
    "method",
    "label",
    "campaign",
    "receiver",
    "bars",
    "bars_sha256",
    "campaign_sha256",
    "engine",
    "cases",
    "levels",
    "p1952",
    "totals",
    "limits",
];

/// The fields of one case of a scorecard, in order (frozen for [`FORMAT`]).
pub const CASE_FIELDS: [&str; 15] = [
    "id",
    "title",
    "source",
    "threat",
    "log_sha256",
    "epochs",
    "truth",
    "events",
    "clean",
    "fix_availability",
    "max_error_m",
    "max_time_error_s",
    "misleading",
    "recovery_s",
    "levels",
];

/// The receiver under test, as the campaign owner states it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Receiver {
    pub maker: String,
    pub model: String,
    #[serde(default)]
    pub firmware: Option<String>,
    #[serde(default)]
    pub antenna: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Who ran the campaign, where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CampaignInfo {
    pub id: String,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub facility: Option<String>,
    #[serde(default)]
    pub dates: Option<String>,
}

/// The bars, stated before scoring. Defaults are the method's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bars {
    /// When the bars were written (ISO-8601 UTC), as the campaign owner states it.
    pub fixed_utc: Option<String>,
    /// A valid fix farther than this from the truth is wrong output, m.
    pub misleading_error_m: f64,
    /// A reported time farther than this from true time is wrong output, s.
    pub misleading_time_error_s: f64,
    /// Level 1: the receiver must be back to correct output within this long after the
    /// threat ends, s.
    pub max_recovery_s: f64,
    /// Correct output must hold this long to count as recovered, s.
    pub recovery_hold_s: f64,
    /// Level 3: during the threat every valid fix within this of the truth, m.
    pub bounded_error_m: f64,
    /// Level 3: during the threat every reported time within this of true time, s.
    pub bounded_time_error_s: f64,
    /// Level 3: share of threat epochs with a fix at least this.
    pub bounded_min_availability: f64,
    /// Level 4: during the threat every valid fix within this of the truth, m.
    pub full_error_m: f64,
    /// Level 4: share of threat epochs with a fix at least this.
    pub full_min_availability: f64,
    /// Whether the open monitor's alarm counts as a warning for level 2 (default false: the
    /// receiver's own output only; the monitored view is reported beside it either way).
    pub monitor_counts_as_warning: bool,
}

impl Default for Bars {
    fn default() -> Self {
        Bars {
            fixed_utc: None,
            misleading_error_m: 50.0,
            misleading_time_error_s: 0.5,
            max_recovery_s: 120.0,
            recovery_hold_s: 10.0,
            bounded_error_m: 500.0,
            bounded_time_error_s: 1.0,
            bounded_min_availability: 0.5,
            full_error_m: 20.0,
            full_min_availability: 0.99,
            monitor_counts_as_warning: false,
        }
    }
}

impl Bars {
    fn validate(&self) -> Result<(), String> {
        let pos = [
            self.misleading_error_m,
            self.misleading_time_error_s,
            self.max_recovery_s,
            self.recovery_hold_s,
            self.bounded_error_m,
            self.bounded_time_error_s,
            self.full_error_m,
        ];
        if pos.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err("bars: every distance, time and duration must be positive".into());
        }
        for a in [self.bounded_min_availability, self.full_min_availability] {
            if !(0.0..=1.0).contains(&a) {
                return Err("bars: availabilities are between 0 and 1".into());
            }
        }
        if self.full_error_m > self.bounded_error_m {
            return Err("bars: full_error_m cannot exceed bounded_error_m".into());
        }
        Ok(())
    }
}

/// A receiver log of a log case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogInput {
    pub format: LogFormat,
    pub path: String,
    /// RINEX broadcast navigation for a `rinex` log.
    #[serde(default)]
    pub nav: Option<String>,
    /// `static` (default) or `vessel` (the moving-platform monitors run).
    #[serde(default)]
    pub platform: Option<String>,
}

/// The true motion of a log case: an I4 motion CSV (`kshana::interop::testbench`), such as
/// the one in a threat library test-bench export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TruthInput {
    pub motion_csv: String,
    /// Motion time (s) at the log's first epoch.
    #[serde(default)]
    pub motion_t0_s: f64,
}

/// One case of a campaign.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseSpec {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    /// A threat library scenario: alone, its analogue is the case; with `log`, its events
    /// are the case's events, shifted by `library_offset_s`.
    #[serde(default)]
    pub library: Option<String>,
    #[serde(default)]
    pub library_offset_s: f64,
    #[serde(default)]
    pub log: Option<LogInput>,
    #[serde(default)]
    pub truth: Option<TruthInput>,
    /// Events stated before scoring (receiver-trust `[[events]]`).
    #[serde(default)]
    pub events: Vec<EventCfg>,
    /// Calibration of the open monitors, s (default 120).
    #[serde(default)]
    pub calibration_s: Option<f64>,
    /// The analogue generator's seed for this case, overriding the library file's
    /// (`analogue` cases only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analogue_seed: Option<u64>,
}

/// A campaign (`kind = "pro-receiver-scorecard"`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Campaign {
    pub kind: String,
    pub title: String,
    pub receiver: Receiver,
    pub campaign: CampaignInfo,
    #[serde(default)]
    pub bars: Bars,
    /// The scoring method (`kshana-pro/receiver-scorecard/v1`, the default, or `.../v2`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(rename = "case")]
    pub cases: Vec<CaseSpec>,
    /// Where relative paths are read from (set from the file's folder; not serialised).
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl Campaign {
    pub fn parse(src: &str) -> Result<Campaign, KshanaError> {
        let c: Campaign = toml::from_str(src)
            .map_err(|e| KshanaError::InvalidInput(format!("invalid {KIND} campaign: {e}")))?;
        c.validate().map_err(KshanaError::InvalidInput)?;
        Ok(c)
    }

    pub fn from_file(path: &Path) -> Result<(Campaign, String), KshanaError> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
        let mut c = Campaign::parse(&src)?;
        c.base_dir = path.parent().map(Path::to_path_buf);
        Ok((c, src))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.kind != KIND {
            return Err(format!("kind must be '{KIND}'"));
        }
        if self.receiver.maker.trim().is_empty() || self.receiver.model.trim().is_empty() {
            return Err("receiver: maker and model are needed".into());
        }
        if self.campaign.id.trim().is_empty() {
            return Err("campaign: id is needed".into());
        }
        self.bars.validate()?;
        self.method_kind()?;
        if self.cases.is_empty() {
            return Err("a campaign has at least one [[case]]".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for c in &self.cases {
            if c.id.trim().is_empty() || !seen.insert(c.id.clone()) {
                return Err(format!("case id '{}' is empty or repeated", c.id));
            }
            match (&c.library, &c.log) {
                (None, None) => return Err(format!("case '{}': give library or log", c.id)),
                (None, Some(_)) if c.events.is_empty() => {
                    return Err(format!(
                        "case '{}': a log case states its events before scoring (events or library)",
                        c.id
                    ))
                }
                (Some(_), Some(_)) if !c.events.is_empty() => {
                    return Err(format!("case '{}': events come from library or events, not both", c.id))
                }
                (Some(_), None) if c.truth.is_some() || !c.events.is_empty() => {
                    return Err(format!(
                        "case '{}': a library-only case takes its events and truth from the analogue",
                        c.id
                    ))
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The scoring method this campaign asks for.
    pub fn method_kind(&self) -> Result<Method, String> {
        match self.method.as_deref() {
            None | Some(METHOD_ID) => Ok(Method::V1),
            Some(METHOD_ID_V2) => Ok(Method::V2),
            Some(m) => Err(format!(
                "method '{m}' is not known (use '{METHOD_ID}' or '{METHOD_ID_V2}')"
            )),
        }
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        match (&self.base_dir, pb.is_absolute()) {
            (Some(b), false) => b.join(pb),
            _ => pb,
        }
    }

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Receiver resilience scorecard (E3): a test campaign of threat cases \
(threat library analogues or the receiver's own logs under stated events), scored for detection, \
availability, error, misleading output and recovery under bars stated before scoring; mapped to \
the DHS resilience levels and the IEEE P1952 draft topics as test evidence.",
            required_fields: &["title", "receiver", "campaign", "case"],
            optional_fields: &["bars"],
        }
    }
}

/// The truth at one epoch.
#[derive(Debug, Clone, Copy)]
struct TruthAt {
    lat: f64,
    lon: f64,
    /// Reported time minus true time, s (library cases only).
    time_offset_s: Option<f64>,
}

/// One case's inputs, loaded.
struct Loaded {
    title: String,
    source: String,
    threat: Vec<String>,
    bytes: Vec<u8>,
    nav: Option<Vec<u8>>,
    format: LogFormat,
    vessel: bool,
    events: Vec<EventCfg>,
    calibration_s: f64,
    /// Truth by epoch time (s since the first log epoch), sorted.
    truth: Option<Vec<(f64, TruthAt)>>,
    truth_kind: String,
    /// Every emitted epoch with its arrival time on a clock independent of the unit under
    /// test (v2 analogue cases: the generator's epoch time) and its text, in stream order.
    arrival: Option<Vec<(f64, String)>>,
}

/// How the receiver and the monitor did on one event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EventResult {
    pub label: String,
    pub kind: String,
    pub onset_s: f64,
    pub end_s: f64,
    /// The open monitor's verdict on the receiver's log: `detected`, `late`, `missed` or
    /// `not-evaluable`, and its first-alarm latency.
    pub monitor_outcome: String,
    pub monitor_latency_s: Option<f64>,
    pub monitor_first: Vec<String>,
    /// The receiver's own first flag (spoofing or jamming state 2 or more) after onset, s;
    /// `None` when it never flagged or its log carries no flags.
    pub receiver_flag_latency_s: Option<f64>,
    /// Epochs in the window, and those with a fix.
    pub window_epochs: usize,
    pub window_fix_epochs: usize,
    pub max_error_m: Option<f64>,
    pub p95_error_m: Option<f64>,
    pub max_time_error_s: Option<f64>,
    /// Wrong output epochs from onset until recovery: without the receiver's own warning,
    /// and with neither the receiver's warning nor the monitor's alarm.
    pub misleading_unwarned: Option<usize>,
    pub misleading_unmonitored: Option<usize>,
    /// From the end of the event to recovered output, s; `None` when it never recovered
    /// (or, with no truth, when it could not be judged).
    pub recovery_s: Option<f64>,
    pub recovered: Option<bool>,
}

/// Clean-period figures of a case.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanResult {
    /// Post-calibration epochs outside every event window (and before the first one).
    pub epochs: usize,
    pub monitor_false_alarm_epochs: usize,
    pub receiver_flag_epochs: usize,
    pub p95_error_m: Option<f64>,
}

/// Wrong output without warning, over every event of a case.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Misleading {
    pub unwarned_epochs: Option<usize>,
    pub unmonitored_epochs: Option<usize>,
}

/// One scored case.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaseResult {
    pub id: String,
    pub title: String,
    /// `library:<id>` (analogue) or `log:<file name>`.
    pub source: String,
    pub threat: Vec<String>,
    pub log_sha256: String,
    pub epochs: usize,
    /// `analogue` (the library analogue's true track and time), `motion-csv` or `none`.
    pub truth: String,
    pub events: Vec<EventResult>,
    pub clean: CleanResult,
    /// Share of threat-window epochs with a fix (over every event).
    pub fix_availability: Option<f64>,
    pub max_error_m: Option<f64>,
    pub max_time_error_s: Option<f64>,
    pub misleading: Misleading,
    /// The longest recovery over the case's events, s.
    pub recovery_s: Option<f64>,
    pub levels: CaseLevels,
}

/// The engine that scored the campaign.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EngineInfo {
    pub open_engine: String,
    pub pro: String,
    pub threat_library: String,
}

/// Campaign totals.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Totals {
    pub cases: usize,
    pub events: usize,
    pub monitor_detected: usize,
    pub monitor_late: usize,
    pub monitor_missed: usize,
    pub receiver_flagged: usize,
    pub recovered: usize,
    pub misleading_unwarned_epochs: Option<usize>,
    pub misleading_unmonitored_epochs: Option<usize>,
}

/// One P1952 draft topic and the evidence the scorecard carries for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicEvidence {
    pub topic: String,
    pub summary: String,
    pub evidence: Vec<String>,
}

/// The P1952 block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct P1952 {
    pub standard: String,
    pub status: String,
    pub wording: String,
    pub topics: Vec<TopicEvidence>,
}

/// A scorecard.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scorecard {
    pub format: &'static str,
    pub kind: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub campaign: CampaignInfo,
    pub receiver: Receiver,
    pub bars: Bars,
    pub bars_sha256: String,
    pub campaign_sha256: String,
    pub engine: EngineInfo,
    pub cases: Vec<CaseResult>,
    pub levels: LevelMapping,
    pub p1952: P1952,
    pub totals: Totals,
    /// What the scorecard does not show.
    pub limits: Vec<String>,
}

/// The open engine release this crate pins (`Cargo.toml`); a test keeps them equal.
pub const OPEN_ENGINE_PIN: &str = "0.41.0";

/// The P1952 draft mapping template.
pub const P1952_TEMPLATE: &str =
    include_str!("../../report-templates/scorecards/ieee-p1952-draft.json");
/// The DHS levels template (wording and sources).
pub const DHS_TEMPLATE: &str =
    include_str!("../../report-templates/scorecards/dhs-rpcf-levels.json");

fn round(v: f64, dp: i32) -> f64 {
    let k = 10f64.powi(dp);
    (v * k).round() / k
}

fn p95(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let i = ((v.len() as f64) * 0.95).ceil() as usize;
    Some(v[i.clamp(1, v.len()) - 1])
}

fn monitor_name(m: &kshana::receiver_trust::monitors::Monitor) -> String {
    serde_json::to_value(m)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// The threat scenario of a case, as scored: its `library` (a library id, or a path to a
/// file in the threat library's format, relative to the campaign) with the case's
/// `analogue_seed` applied. `None` for a log case without a library.
pub fn case_scenario(
    c: &Campaign,
    cs: &CaseSpec,
) -> Result<Option<crate::threat_library::ThreatScenario>, KshanaError> {
    let bad = |m: String| KshanaError::InvalidInput(format!("case '{}': {m}", cs.id));
    let lib = match &cs.library {
        Some(id) if id.ends_with(".toml") => {
            let p = c.resolve(id);
            let src =
                std::fs::read_to_string(&p).map_err(|e| bad(format!("{}: {e}", p.display())))?;
            Some(crate::threat_library::ThreatScenario::parse(&src)?)
        }
        Some(id) => Some(crate::threat_library::get(id)?),
        None => None,
    };
    match (lib, cs.analogue_seed) {
        (Some(mut s), Some(seed)) => {
            match s.analogue.as_mut() {
                Some(a) => a.scenario.seed = seed,
                None => return Err(bad("analogue_seed needs a scenario with an analogue".into())),
            }
            Ok(Some(s))
        }
        (_, Some(_)) => Err(bad("analogue_seed needs a library case".into())),
        (l, None) => Ok(l),
    }
}

fn load_case(c: &Campaign, cs: &CaseSpec) -> Result<Loaded, KshanaError> {
    let bad = |m: String| KshanaError::InvalidInput(format!("case '{}': {m}", cs.id));
    let lib = case_scenario(c, cs)?;
    let threat_of = |s: &crate::threat_library::ThreatScenario| {
        s.threat
            .iter()
            .map(|t| {
                serde_json::to_value(t)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
    };
    let cal = cs.calibration_s.unwrap_or(120.0);
    if let (Some(s), None) = (&lib, &cs.log) {
        let an = crate::threat_library::export::analogue(s)?
            .ok_or_else(|| bad(format!("library scenario '{}' has no analogue", s.id)))?;
        let truth = an
            .log()
            .track
            .iter()
            .map(|r| {
                (
                    r.t_s,
                    TruthAt {
                        lat: r.true_lat_deg,
                        lon: r.true_lon_deg,
                        time_offset_s: Some(r.time_offset_s),
                    },
                )
            })
            .collect();
        return Ok(Loaded {
            title: cs.title.clone().unwrap_or_else(|| s.title.clone()),
            source: format!("library:{}", s.id),
            threat: threat_of(s),
            bytes: an.nmea().into_bytes(),
            nav: None,
            format: LogFormat::Nmea,
            vessel: !s.analogue_is_static(),
            events: an.trust.events.clone(),
            calibration_s: an.trust.monitors.calibration_s,
            truth: Some(truth),
            truth_kind: "analogue".into(),
            arrival: Some(
                an.generated
                    .epochs
                    .iter()
                    .map(|e| {
                        let mut t = String::new();
                        for line in &e.lines {
                            t.push_str(line);
                            t.push_str("\r\n");
                        }
                        (e.t_s, t)
                    })
                    .collect(),
            ),
        });
    }
    let log = cs.log.as_ref().expect("validated: a log case has a log");
    let path = c.resolve(&log.path);
    let bytes = std::fs::read(&path).map_err(|e| bad(format!("{}: {e}", path.display())))?;
    let nav = match &log.nav {
        Some(n) => {
            let p = c.resolve(n);
            Some(std::fs::read(&p).map_err(|e| bad(format!("{}: {e}", p.display())))?)
        }
        None => None,
    };
    let events = match &lib {
        Some(s) => {
            let an = crate::threat_library::export::analogue(s)?
                .ok_or_else(|| bad(format!("library scenario '{}' has no analogue", s.id)))?;
            an.trust
                .events
                .iter()
                .map(|e| EventCfg {
                    onset_s: e.onset_s + cs.library_offset_s,
                    end_s: e.end_s.map(|v| v + cs.library_offset_s),
                    ..e.clone()
                })
                .collect()
        }
        None => cs.events.clone(),
    };
    let truth = match &cs.truth {
        Some(t) => {
            let p = c.resolve(&t.motion_csv);
            let text =
                std::fs::read_to_string(&p).map_err(|e| bad(format!("{}: {e}", p.display())))?;
            let m = kshana::interop::testbench::read_motion_csv(&text)
                .map_err(|e| bad(format!("{}: {e}", p.display())))?;
            Some(
                m.iter()
                    .map(|s| {
                        (
                            s.t_s - t.motion_t0_s,
                            TruthAt {
                                lat: s.lat_deg,
                                lon: s.lon_deg,
                                time_offset_s: None,
                            },
                        )
                    })
                    .collect(),
            )
        }
        None => None,
    };
    let vessel = match log.platform.as_deref() {
        None | Some("static") => false,
        Some("vessel") => true,
        Some(o) => return Err(bad(format!("platform '{o}' (static or vessel)"))),
    };
    Ok(Loaded {
        title: cs.title.clone().unwrap_or_else(|| cs.id.clone()),
        source: format!(
            "log:{}",
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        ),
        threat: match &lib {
            Some(s) => threat_of(s),
            None => {
                let mut k: Vec<String> = events
                    .iter()
                    .map(|e| {
                        serde_json::to_value(e.kind)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_string))
                            .unwrap_or_default()
                    })
                    .collect();
                k.sort();
                k.dedup();
                k
            }
        },
        bytes,
        nav,
        format: log.format,
        vessel,
        events,
        calibration_s: cal,
        truth_kind: if truth.is_some() {
            "motion-csv"
        } else {
            "none"
        }
        .into(),
        arrival: None,
        truth,
    })
}

fn trust_scenario(l: &Loaded) -> Result<ReceiverTrustScenario, KshanaError> {
    let fmt = serde_json::to_value(l.format)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let mut t = format!(
        "kind = \"receiver-trust\"\n[log]\nformat = \"{fmt}\"\npath = \"input\"\n[monitors]\ncalibration_s = {:.1}\n",
        l.calibration_s
    );
    if l.vessel {
        t.push_str("[platform]\nkind = \"vessel\"\nmax_speed_kn = 30.0\nheading_sensor = true\n");
    }
    let mut scn: ReceiverTrustScenario =
        toml::from_str(&t).map_err(|e| KshanaError::InvalidInput(format!("monitors: {e}")))?;
    scn.events = l.events.clone();
    Ok(scn)
}

fn truth_at(truth: &[(f64, TruthAt)], t: f64) -> Option<TruthAt> {
    let i = truth.partition_point(|(ts, _)| *ts < t);
    let pick = |k: usize| {
        truth
            .get(k)
            .filter(|(ts, _)| (ts - t).abs() <= 1.0)
            .map(|x| x.1)
    };
    match (i.checked_sub(1).and_then(pick), pick(i)) {
        (Some(a), Some(b)) => {
            let (ta, tb) = (truth[i - 1].0, truth[i].0);
            if (tb - ta).abs() < 1e-9 {
                return Some(b);
            }
            let w = (t - ta) / (tb - ta);
            Some(TruthAt {
                lat: a.lat + w * (b.lat - a.lat),
                lon: a.lon + w * (b.lon - a.lon),
                time_offset_s: match (a.time_offset_s, b.time_offset_s) {
                    (Some(x), Some(y)) => Some(x + w * (y - x)),
                    _ => None,
                },
            })
        }
        (a, b) => a.or(b),
    }
}

/// One epoch, as the scorecard sees it.
#[derive(Debug, Clone)]
struct Ep {
    t: f64,
    fix: bool,
    err_m: Option<f64>,
    time_err_s: Option<f64>,
    flag: bool,
    alarm: bool,
    calibrating: bool,
}

fn epochs_of(
    l: &Loaded,
    tl: &Timeline,
    res: &ReceiverTrustResult,
    method: Method,
) -> Result<Vec<Ep>, String> {
    let alarm_at: std::collections::BTreeMap<i64, (bool, bool, bool)> = res
        .epochs
        .iter()
        .map(|e| {
            (
                (e.t_s * 1000.0).round() as i64,
                (
                    !e.alarms.is_empty(),
                    e.state == kshana::receiver_trust::monitors::TrustState::Calibrating,
                    e.self_report.as_ref().is_some_and(|s| s.receiver_flag),
                ),
            )
        })
        .collect();
    // v2 on an analogue: one scored epoch per emitted epoch, stamped with its arrival time
    // (the generator's epoch time), its fix and flags read from that epoch's own sentences,
    // the truth taken at the arrival time and the error on the WGS-84 ellipsoid. The open
    // monitor's alarm is the one of the stream epoch that carries the same stated time.
    if let (Method::V2, Some(arr)) = (method, &l.arrival) {
        let by_label: std::collections::BTreeMap<&str, f64> = tl
            .epochs
            .iter()
            .filter_map(|e| e.time_label.as_deref().map(|lb| (lb, e.t_s)))
            .collect();
        let mut out = Vec::with_capacity(arr.len());
        for (t, text) in arr {
            let one = kshana::receiver_trust::ingest::read_log(l.format, text.as_bytes())?;
            let e = match one.epochs.as_slice() {
                [e] => e,
                [] => {
                    out.push(Ep {
                        t: *t,
                        fix: false,
                        err_m: None,
                        time_err_s: None,
                        flag: false,
                        alarm: false,
                        calibrating: false,
                    });
                    continue;
                }
                _ => {
                    return Err(format!(
                        "v2: the epoch emitted at {t:.1} s reads as {} epochs",
                        one.epochs.len()
                    ))
                }
            };
            let stream_t = e
                .time_label
                .as_deref()
                .and_then(|lb| by_label.get(lb))
                .copied();
            let (alarm, calibrating, mflag) = stream_t
                .and_then(|st| alarm_at.get(&((st * 1000.0).round() as i64)).copied())
                .unwrap_or((false, false, false));
            let flag = mflag || e.self_report.as_ref().is_some_and(|s| s.flagged());
            let tr = l.truth.as_deref().and_then(|tt| truth_at(tt, *t));
            let err_m = match (e.fix, tr) {
                (Some(f), Some(tr)) => Some(kshana::geodesic::distance_m(
                    f.lat_deg, f.lon_deg, tr.lat, tr.lon,
                )),
                _ => None,
            };
            out.push(Ep {
                t: *t,
                fix: e.fix.is_some(),
                err_m,
                time_err_s: tr.and_then(|t| t.time_offset_s).filter(|_| e.fix.is_some()),
                flag,
                alarm,
                calibrating,
            });
        }
        return Ok(out);
    }
    Ok(tl
        .epochs
        .iter()
        .map(|e| {
            let key = (e.t_s * 1000.0).round() as i64;
            let (alarm, calibrating, mflag) =
                alarm_at.get(&key).copied().unwrap_or((false, false, false));
            let flag = mflag || e.self_report.as_ref().is_some_and(|s| s.flagged());
            let t = e.t_s;
            let tr = l.truth.as_deref().and_then(|tt| truth_at(tt, t));
            let err_m = match (e.fix, tr) {
                (Some(f), Some(tr)) => Some(match method {
                    Method::V1 => haversine_m(f.lat_deg, f.lon_deg, tr.lat, tr.lon),
                    Method::V2 => {
                        kshana::geodesic::distance_m(f.lat_deg, f.lon_deg, tr.lat, tr.lon)
                    }
                }),
                _ => None,
            };
            Ep {
                t,
                fix: e.fix.is_some(),
                err_m,
                time_err_s: tr.and_then(|t| t.time_offset_s).filter(|_| e.fix.is_some()),
                flag,
                alarm,
                calibrating,
            }
        })
        .collect())
}

/// Wrong output at one epoch (a fix too far or a time too far from the truth); `None`
/// without truth.
fn wrong(e: &Ep, b: &Bars, has_truth: bool) -> Option<bool> {
    if !has_truth {
        return None;
    }
    if !e.fix {
        return Some(false);
    }
    Some(
        e.err_m.is_some_and(|d| d > b.misleading_error_m)
            || e.time_err_s
                .is_some_and(|d| d.abs() > b.misleading_time_error_s),
    )
}

fn score_case(c: &Campaign, cs: &CaseSpec) -> Result<CaseResult, KshanaError> {
    let b = &c.bars;
    let method = c.method_kind().map_err(KshanaError::InvalidInput)?;
    let mut l = load_case(c, cs)?;
    if method == Method::V2 && l.arrival.is_none() && l.truth.is_some() {
        // v2 takes truth at the arrival time; this campaign format carries no host arrival
        // timestamps for a log, so its errors are not scored.
        l.truth = None;
        l.truth_kind = "none".into();
    }
    let bad = |m: String| KshanaError::InvalidInput(format!("case '{}': {m}", cs.id));
    let scn = trust_scenario(&l)?;
    let res = run_receiver_trust_bytes(&scn, &l.bytes, l.nav.as_deref()).map_err(&bad)?;
    let tl = kshana::receiver_trust::ingest::read_log(l.format, &l.bytes).map_err(&bad)?;
    let eps = epochs_of(&l, &tl, &res, method).map_err(&bad)?;
    let has_truth = l.truth.is_some();
    let end_log = eps.last().map(|e| e.t).unwrap_or(0.0);
    let mut events = Vec::new();
    let mut windows: Vec<(f64, f64)> = Vec::new();
    for (k, sc) in res.events.iter().enumerate() {
        let onset = sc.onset_s;
        let end = l.events[k].end_s.unwrap_or(sc.window_end_s);
        windows.push((onset, end));
        let next_onset = res
            .events
            .iter()
            .map(|e| e.onset_s)
            .filter(|t| *t > onset)
            .fold(f64::INFINITY, f64::min);
        let in_w: Vec<&Ep> = eps.iter().filter(|e| e.t >= onset && e.t <= end).collect();
        let fix_n = in_w.iter().filter(|e| e.fix).count();
        let errs: Vec<f64> = in_w.iter().filter_map(|e| e.err_m).collect();
        let terrs: Vec<f64> = in_w
            .iter()
            .filter_map(|e| e.time_err_s.map(f64::abs))
            .collect();
        let flag_lat = eps
            .iter()
            .find(|e| e.t >= onset && e.t <= end && e.flag)
            .map(|e| round(e.t - onset, 1));
        // Recovery: from the end, the first epoch from which correct output (with truth) or
        // a fix without alarm (without truth) holds for the hold time, before the next event.
        let ok_after = |e: &Ep| -> bool {
            if has_truth {
                e.fix && wrong(e, b, true) == Some(false)
            } else {
                e.fix && !e.alarm
            }
        };
        let mut recovery = None;
        let after: Vec<&Ep> = eps
            .iter()
            .filter(|e| e.t >= end && e.t < next_onset)
            .collect();
        let mut i = 0;
        while i < after.len() {
            if ok_after(after[i]) {
                let t0 = after[i].t;
                let mut j = i;
                while j < after.len() && ok_after(after[j]) {
                    j += 1;
                }
                let last = after[j - 1].t;
                let reaches_end = j == after.len() && (next_onset.is_finite() || last >= end_log);
                if last - t0 >= b.recovery_hold_s || reaches_end && last - t0 >= 0.0 && j - i > 1 {
                    recovery = Some(round(t0 - end, 1));
                    break;
                }
                i = j;
            } else {
                i += 1;
            }
        }
        let recovered = if end >= end_log {
            None
        } else {
            Some(recovery.is_some())
        };
        // Wrong output without warning from onset to recovery (or the next event).
        let stop = recovery
            .map(|r| end + r)
            .unwrap_or(next_onset.min(end_log + 1.0));
        let span: Vec<&Ep> = eps.iter().filter(|e| e.t >= onset && e.t < stop).collect();
        let count = |monitored: bool| -> Option<usize> {
            if !has_truth {
                return None;
            }
            Some(
                span.iter()
                    .filter(|e| wrong(e, b, true) == Some(true))
                    .filter(|e| !(e.flag || monitored && e.alarm))
                    .count(),
            )
        };
        events.push(EventResult {
            label: sc.label.clone(),
            kind: serde_json::to_value(sc.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            onset_s: round(onset, 1),
            end_s: round(end, 1),
            monitor_outcome: sc.outcome.clone(),
            monitor_latency_s: sc.latency_s.map(|v| round(v, 1)),
            monitor_first: sc.first_monitors.iter().map(monitor_name).collect(),
            receiver_flag_latency_s: flag_lat,
            window_epochs: in_w.len(),
            window_fix_epochs: fix_n,
            max_error_m: errs.iter().copied().reduce(f64::max).map(|v| round(v, 1)),
            p95_error_m: p95(errs).map(|v| round(v, 1)),
            max_time_error_s: terrs.iter().copied().reduce(f64::max).map(|v| round(v, 3)),
            misleading_unwarned: count(false),
            misleading_unmonitored: count(true),
            recovery_s: recovery,
            recovered,
        });
    }
    // Clean epochs: after calibration, outside every window and its recovery.
    let clean: Vec<&Ep> = eps
        .iter()
        .filter(|e| !e.calibrating && e.t >= l.calibration_s)
        .filter(|e| {
            !windows.iter().zip(&events).any(|((a, z), ev)| {
                let until = z + ev
                    .recovery_s
                    .unwrap_or(b.max_recovery_s)
                    .max(b.max_recovery_s);
                e.t >= a - 10.0 && e.t <= until
            })
        })
        .collect();
    let clean_res = CleanResult {
        epochs: clean.len(),
        monitor_false_alarm_epochs: clean.iter().filter(|e| e.alarm).count(),
        receiver_flag_epochs: clean.iter().filter(|e| e.flag).count(),
        p95_error_m: p95(clean.iter().filter_map(|e| e.err_m).collect()).map(|v| round(v, 1)),
    };
    let w_n: usize = events.iter().map(|e| e.window_epochs).sum();
    let w_fix: usize = events.iter().map(|e| e.window_fix_epochs).sum();
    let availability = (w_n > 0).then(|| round(w_fix as f64 / w_n as f64, 3));
    let max_err = events.iter().filter_map(|e| e.max_error_m).reduce(f64::max);
    let max_terr = events
        .iter()
        .filter_map(|e| e.max_time_error_s)
        .reduce(f64::max);
    let sum_opt = |f: &dyn Fn(&EventResult) -> Option<usize>| -> Option<usize> {
        events
            .iter()
            .map(f)
            .try_fold(0usize, |a, v| v.map(|x| a + x))
    };
    let mis = Misleading {
        unwarned_epochs: sum_opt(&|e| e.misleading_unwarned),
        unmonitored_epochs: sum_opt(&|e| e.misleading_unmonitored),
    };
    let recovery = events.iter().filter_map(|e| e.recovery_s).reduce(f64::max);
    // The level criteria (fixed by METHOD_ID).
    let recover = {
        let r: Vec<Option<bool>> = events
            .iter()
            .map(|e| match e.recovered {
                Some(true) => Some(e.recovery_s.is_some_and(|s| s <= b.max_recovery_s)),
                other => other,
            })
            .collect();
        r.iter()
            .fold(Verdict::Met, |a, v| a.and(Verdict::from_bool(*v)))
    };
    let no_misleading = Verdict::from_bool(
        if b.monitor_counts_as_warning {
            mis.unmonitored_epochs
        } else {
            mis.unwarned_epochs
        }
        .map(|n| n == 0),
    );
    let within = |err_bar: f64, t_bar: f64, avail: f64| -> Verdict {
        if !has_truth {
            return Verdict::NotAssessed;
        }
        let e_ok = max_err.is_none_or(|e| e <= err_bar);
        let t_ok = max_terr.is_none_or(|t| t <= t_bar);
        let a_ok = availability.is_some_and(|a| a >= avail);
        Verdict::from_bool(Some(e_ok && t_ok && a_ok))
    };
    let bounded = within(
        b.bounded_error_m,
        b.bounded_time_error_s,
        b.bounded_min_availability,
    );
    let full = within(
        b.full_error_m,
        b.misleading_time_error_s,
        b.full_min_availability,
    );
    Ok(CaseResult {
        id: cs.id.clone(),
        title: l.title,
        source: l.source,
        threat: l.threat,
        log_sha256: hex::encode(Sha256::digest(&l.bytes)),
        epochs: eps.len(),
        truth: l.truth_kind,
        events,
        clean: clean_res,
        fix_availability: availability,
        max_error_m: max_err,
        max_time_error_s: max_terr,
        misleading: mis,
        recovery_s: recovery,
        levels: CaseLevels::from_criteria(recover, no_misleading, bounded, full),
    })
}

/// The P1952 block of a scorecard, from the template and the campaign's evidence.
pub fn p1952_block() -> Result<P1952, KshanaError> {
    serde_json::from_str(P1952_TEMPLATE)
        .map_err(|e| KshanaError::InvalidInput(format!("P1952 template: {e}")))
}

/// SHA-256 of the canonical JSON of the bars.
pub fn bars_sha256(b: &Bars) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_vec(b).unwrap_or_default().as_slice(),
    ))
}

/// Score a campaign. `src` is the campaign file's text (hashed into the scorecard).
pub fn score(c: &Campaign, src: &str) -> Result<Scorecard, KshanaError> {
    c.validate().map_err(KshanaError::InvalidInput)?;
    let mut cases = Vec::new();
    for cs in &c.cases {
        cases.push(score_case(c, cs)?);
    }
    let levels = levels::map(&cases.iter().map(|c| c.levels.clone()).collect::<Vec<_>>());
    let all_events: Vec<&EventResult> = cases.iter().flat_map(|c| &c.events).collect();
    let sum_opt = |f: &dyn Fn(&CaseResult) -> Option<usize>| -> Option<usize> {
        cases
            .iter()
            .map(f)
            .try_fold(0usize, |a, v| v.map(|x| a + x))
    };
    let totals = Totals {
        cases: cases.len(),
        events: all_events.len(),
        monitor_detected: all_events
            .iter()
            .filter(|e| e.monitor_outcome == "detected")
            .count(),
        monitor_late: all_events
            .iter()
            .filter(|e| e.monitor_outcome == "late")
            .count(),
        monitor_missed: all_events
            .iter()
            .filter(|e| e.monitor_outcome == "missed")
            .count(),
        receiver_flagged: all_events
            .iter()
            .filter(|e| e.receiver_flag_latency_s.is_some())
            .count(),
        recovered: all_events
            .iter()
            .filter(|e| e.recovered == Some(true))
            .count(),
        misleading_unwarned_epochs: sum_opt(&|c| c.misleading.unwarned_epochs),
        misleading_unmonitored_epochs: sum_opt(&|c| c.misleading.unmonitored_epochs),
    };
    let method = c.method_kind().map_err(KshanaError::InvalidInput)?;
    let mut limits = vec![
        "Only the cases listed were run; nothing is said about other threats, powers, signals or receivers.".to_string(),
        "Event windows are the stated times (scheduled or scripted), not measured on-air times.".to_string(),
        "Monitor verdicts are the open receiver-trust engine's, on the receiver's log; they are MODELLED.".to_string(),
    ];
    if cases.iter().any(|c| c.source.starts_with("library:")) {
        limits.push("Library cases score the open engine's synthetic NMEA receiver model on a MODELLED analogue of a published event; they show the method, not the behaviour of any product.".into());
    }
    if cases.iter().any(|c| c.truth == "none") {
        limits.push("Cases without a truth track cannot show error, misleading output or levels 2 to 4; those are 'not assessed'.".into());
    }
    if method == Method::V2 {
        limits.push("Method v2: errors are WGS-84 geodesic distances to the truth at each epoch's arrival time (for an analogue, the time the generator emitted the epoch); log cases carry no host arrival timestamps in this format, so their errors are not scored.".into());
    }
    if c.bars.fixed_utc.is_none() {
        limits.push("The campaign does not state when its bars were fixed.".into());
    }
    Ok(Scorecard {
        format: FORMAT,
        kind: KIND,
        method: method.id(),
        label: LABEL,
        campaign: c.campaign.clone(),
        receiver: c.receiver.clone(),
        bars: c.bars.clone(),
        bars_sha256: bars_sha256(&c.bars),
        campaign_sha256: hex::encode(Sha256::digest(src.as_bytes())),
        engine: EngineInfo {
            open_engine: format!("kshana {OPEN_ENGINE_PIN}"),
            pro: format!("kshana-pro {}", env!("CARGO_PKG_VERSION")),
            threat_library: crate::threat_library::EDITION.into(),
        },
        cases,
        levels,
        p1952: p1952_block()?,
        totals,
        limits,
    })
}

impl Scenario for Campaign {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        let src = toml::to_string(self).unwrap_or_default();
        let s = score(self, &src)?;
        Ok(RunOutput {
            summary: format!(
                "Kshana Pro receiver scorecard | {} {} | {} cases, {} events | monitor {} detected, {} late, {} missed | highest level supported by the evidence: {} | test evidence, MODELLED monitors",
                s.receiver.maker,
                s.receiver.model,
                s.totals.cases,
                s.totals.events,
                s.totals.monitor_detected,
                s.totals.monitor_late,
                s.totals.monitor_missed,
                s.levels.highest_supported
            ),
            svg: report::summary_svg(&s),
            csv: Some(report::cases_csv(&s)),
            json: serde_json::to_string_pretty(&s)
                .map_err(|e| KshanaError::InvalidInput(e.to_string()))?,
        })
    }
}

impl ExternalPack for Campaign {
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
