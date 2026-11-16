// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The forensics method (`kshana-pro/incident-forensics/v1`).
//!
//! * **Receiver logs.** The open engine's `receiver-trust` monitors run over each log with
//!   the method's fixed parameters. Their alarm epochs are grouped into episodes; each
//!   episode is attributed (jamming-like, spoofing-like, both, or a degradation that is not
//!   attributed to interference) from which monitor families alarmed, and from the breadth
//!   of the signal loss across satellites and its dependence on elevation (a jammer lowers
//!   every satellite; terrain and buildings take the low ones).
//! * **AIS and ADS-B.** Per-track detectors on the broadcast positions (implausible jumps
//!   and speeds, circular tracks, many vessels at one position, integrity categories that
//!   drop below what the same aircraft reported before), aggregated into one finding per
//!   file and detector.
//! * **Confidence.** A stated points rule per finding (duration, number of indicators, the
//!   discriminator, independent corroboration). Without an independent source a finding is
//!   at most `medium`.
//! * **Area and time.** Onset and end are bounded by the last clean and the first alarmed
//!   epoch; the affected area is the extent of the observers that showed the effect, which
//!   is not an emitter location.

use std::collections::{BTreeMap, BTreeSet};

use kshana::api::KshanaError;
use kshana::interference_map::grid::haversine_m;
use kshana::receiver_trust::monitors::{EpochTrust, Monitor, TrustState};
use kshana::receiver_trust::scenario::{
    run_receiver_trust_bytes, ReceiverTrustResult, ReceiverTrustScenario,
};
use kshana::receiver_trust::{LogFormat, Timeline};
use serde::Serialize;

use super::inputs::{
    iso, load_all, parse_tracks, parse_utc, round, set_coverage, InputRecord, InputRole,
    LoadedInput, PositionReport, Tracks,
};
use super::{
    Incident, IncidentScenario, Platform, ReceiverInput, StatementInput, Subject, Variant, FORMAT,
    LABEL, METHOD_ID, VALIDATION_STATUS,
};

/// The method's parameters. Every one is fixed for [`METHOD_ID`] and written into each report.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MethodParams {
    /// Calibration window of a static receiver's log, s (the baseline; never alarms).
    pub static_calibration_s: f64,
    /// Calibration window of a vessel's log, s.
    pub vessel_calibration_s: f64,
    /// Receiver monitors: C/N0 drop, satellites lost, position jump.
    pub cn0_drop_db: f64,
    pub sats_lost: u32,
    pub position_jump_m: f64,
    /// Vessel limits when the incident does not state them.
    pub vessel_max_speed_kn: f64,
    pub vessel_max_accel_mps2: f64,
    pub vessel_max_turn_rate_dps: f64,
    pub vessel_antenna_height_m: f64,
    /// Alarm epochs closer than this are one episode, s.
    pub merge_gap_s: f64,
    /// Episodes shorter than this are recorded as transients, not findings, s.
    pub min_episode_s: f64,
    /// An episode at least this long earns the duration point, s.
    pub strong_episode_s: f64,
    /// A satellite counts as affected when lost or its C/N0 fell by at least this, dB.
    pub breadth_drop_db: f64,
    /// Jamming-like needs at least this share of the baseline satellites affected.
    pub breadth_min_share: f64,
    /// Affected satellites this much lower in elevation than unaffected ones, with a narrow
    /// breadth, make the degradation obstruction-like (not attributed), degrees.
    pub obstruction_elev_margin_deg: f64,
    /// Findings of different sources within this time of each other corroborate, s.
    pub corroboration_window_s: f64,
    /// AIS: implausible speed, knots; jump minimum distance, m; largest gap compared, s.
    pub ais_max_speed_kn: f64,
    pub ais_jump_min_m: f64,
    pub ais_jump_max_dt_s: f64,
    /// AIS circular track: total turning, degrees; reports; longest span, s.
    pub ais_circle_turn_deg: f64,
    pub ais_circle_min_reports: usize,
    pub ais_circle_max_span_s: f64,
    /// AIS same position: rounding, degrees; window, s; distinct vessels.
    pub ais_same_pos_round_deg: f64,
    pub ais_same_pos_window_s: f64,
    pub ais_same_pos_min_vessels: usize,
    /// ADS-B: integrity below these categories counts as degraded, for an aircraft that
    /// reported at or above them before.
    pub adsb_nic_min: u8,
    pub adsb_nacp_min: u8,
    /// ADS-B: implausible ground speed, knots; jump minimum distance, m.
    pub adsb_max_speed_kn: f64,
    pub adsb_jump_min_m: f64,
    /// Track data this long before the window is read as each track's baseline, s.
    pub baseline_pad_s: f64,
    /// Margin added around the affected observers, m.
    pub area_margin_m: f64,
    /// Share of the window an input must cover for the claim-grade checklist.
    pub claim_min_coverage: f64,
}

impl MethodParams {
    pub const V1: MethodParams = MethodParams {
        static_calibration_s: 60.0,
        vessel_calibration_s: 180.0,
        cn0_drop_db: 6.0,
        sats_lost: 4,
        position_jump_m: 50.0,
        vessel_max_speed_kn: 30.0,
        vessel_max_accel_mps2: 0.3,
        vessel_max_turn_rate_dps: 3.0,
        vessel_antenna_height_m: 15.0,
        merge_gap_s: 30.0,
        min_episode_s: 10.0,
        strong_episode_s: 30.0,
        breadth_drop_db: 6.0,
        breadth_min_share: 0.6,
        obstruction_elev_margin_deg: 10.0,
        corroboration_window_s: 900.0,
        ais_max_speed_kn: 50.0,
        ais_jump_min_m: 500.0,
        ais_jump_max_dt_s: 900.0,
        ais_circle_turn_deg: 300.0,
        ais_circle_min_reports: 10,
        ais_circle_max_span_s: 3600.0,
        ais_same_pos_round_deg: 1e-4,
        ais_same_pos_window_s: 300.0,
        ais_same_pos_min_vessels: 3,
        adsb_nic_min: 7,
        adsb_nacp_min: 8,
        adsb_max_speed_kn: 900.0,
        adsb_jump_min_m: 5000.0,
        baseline_pad_s: 1800.0,
        area_margin_m: 5000.0,
        claim_min_coverage: 0.9,
    };

    /// SHA-256 of the parameters' JSON (field order fixed by the struct).
    pub fn sha256(&self) -> String {
        super::inputs::sha256_hex(&serde_json::to_vec(self).unwrap_or_default())
    }
}

// ─── Report types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EngineInfo {
    pub kshana_pro: String,
    pub open_engine: String,
    pub method_id: &'static str,
    pub method_params_sha256: String,
}

/// A time with its bounds: the event happened between `earliest_utc` and `latest_utc`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TimeBound {
    pub earliest_utc: String,
    pub latest_utc: String,
    /// `latest - earliest`, s.
    pub uncertainty_s: f64,
    /// How the bounds were set.
    pub basis: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
    /// `measured` (the receiver's own fix) or `reported` (a broadcast position).
    pub class: &'static str,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Confidence {
    /// `high`, `medium` or `low`.
    pub level: &'static str,
    /// Points of 4 under the stated rule.
    pub points: u8,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EvidenceItem {
    /// `measured`, `reported` or `modelled`.
    pub class: &'static str,
    pub statement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_sha256: Option<String>,
}

/// One finding.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: String,
    /// `jamming`, `spoofing`, `jamming-and-spoofing` or `degradation-not-attributed`.
    pub category: &'static str,
    /// The input's label.
    pub source: String,
    /// `receiver`, `ais` or `adsb`.
    pub source_kind: &'static str,
    /// The class of the data it rests on.
    pub data_class: &'static str,
    /// Always `modelled`: a finding is the method's assessment.
    pub inference_class: &'static str,
    pub onset: TimeBound,
    pub end: TimeBound,
    pub duration_s: f64,
    /// Monitors or detectors that fired.
    pub indicators: Vec<String>,
    pub magnitude: BTreeMap<String, f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_at_onset: Option<Position>,
    pub confidence: Confidence,
    pub corroborated_by: Vec<String>,
    /// Meets the claim-grade bar: high confidence (so corroborated by an independent source).
    pub supported: bool,
    /// Whether the finding concerns the incident's subject.
    pub about_subject: bool,
    pub evidence: Vec<EvidenceItem>,
    pub statement: String,
    #[serde(skip)]
    t_onset: f64,
    #[serde(skip)]
    t_end: f64,
    #[serde(skip)]
    points_wo_corr: u8,
    #[serde(skip)]
    reasons_wo_corr: Vec<String>,
    #[serde(skip)]
    affected: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryVerdict {
    /// `indicated`, `not-indicated` or `inconclusive`.
    pub status: &'static str,
    /// Highest confidence among the findings, when indicated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<&'static str>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Verdict {
    pub jamming: CategoryVerdict,
    pub spoofing: CategoryVerdict,
    pub sentence: String,
}

/// What one receiver log showed overall.
#[derive(Debug, Clone, Serialize)]
pub struct ReceiverSummary {
    pub label: String,
    pub role: &'static str,
    pub platform: &'static str,
    pub format: LogFormat,
    pub input_sha256: String,
    pub epochs: usize,
    pub start_utc: Option<String>,
    pub calibration_end_utc: Option<String>,
    pub observables: Vec<String>,
    pub monitors_run: Vec<Monitor>,
    pub nominal_epochs: usize,
    pub degraded_epochs: usize,
    pub untrusted_epochs: usize,
    /// Alarm episodes shorter than the method's minimum (not findings).
    pub transients: usize,
    /// Episodes outside the incident window (not findings).
    pub episodes_outside_window: usize,
    /// The receiver's own spoofing/jamming flags, where the log carries them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_self_report: Option<kshana::receiver_trust::monitors::SelfReportSummary>,
    /// The open engine's one-sentence verdict on the whole log.
    pub engine_verdict: String,
    /// The open engine's honesty label for its result.
    pub engine_label: String,
}

/// What one AIS or ADS-B file showed in the window and area.
#[derive(Debug, Clone, Serialize)]
pub struct AreaActivity {
    pub label: String,
    /// `ais` or `adsb`.
    pub source: &'static str,
    pub input_sha256: String,
    pub rows: usize,
    pub rows_refused: usize,
    /// Distinct vessels or aircraft with a report in the window and area.
    pub observed: usize,
    /// Of those, flagged by each detector.
    pub flagged: BTreeMap<String, usize>,
    /// Flagged by any detector.
    pub flagged_any: usize,
    /// ADS-B: aircraft that never reported integrity at or above the method's minimum, so
    /// their degradation cannot be assessed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unassessed: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackPoint {
    pub utc: String,
    pub lat: f64,
    pub lon: f64,
    pub flagged: bool,
}

/// The subject's track as one input gives it.
#[derive(Debug, Clone, Serialize)]
pub struct Track {
    pub source: String,
    /// `measured` (receiver fix) or `reported` (AIS/ADS-B).
    pub class: &'static str,
    pub points: Vec<TrackPoint>,
}

/// The affected area as modelled from the observers.
#[derive(Debug, Clone, Serialize)]
pub struct AffectedArea {
    pub class: &'static str,
    /// Observers (receivers, vessels, aircraft) that showed an effect, with a position.
    pub affected_observers: usize,
    /// Observers in the window with no effect, inside the area's box.
    pub unaffected_observers_inside: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub centroid: Option<[f64; 2]>,
    /// Distance from the centroid to the furthest affected observer plus the margin, m.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_m: Option<f64>,
    /// `[lat_min, lat_max, lon_min, lon_max]` of the affected observers plus the margin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    pub points: Vec<[f64; 2]>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimelineEntry {
    pub utc: String,
    /// `window`, `onset`, `end`, `statement`.
    pub kind: &'static str,
    /// `measured`, `reported`, `modelled` or `stated`.
    pub class: &'static str,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
    #[serde(skip)]
    pub t: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckItem {
    pub id: &'static str,
    pub item: &'static str,
    /// `both` or `claim-grade`.
    pub required_for: &'static str,
    /// `pass`, `fail` or `pending`.
    pub result: &'static str,
    pub detail: String,
}

/// The expert review fields of a claim-grade report: empty at build time. A named reviewer
/// completes them with `kshana-pro forensics review`, which writes and signs
/// `review/expert-review.json` against this report's SHA-256.
#[derive(Debug, Clone, Serialize)]
pub struct ExpertReviewFields {
    pub required: bool,
    /// `not-required`, `awaiting-review` or `not-eligible` (checklist not met).
    pub status: &'static str,
    pub reviewer_name: Option<String>,
    pub qualification: Option<String>,
    pub organisation: Option<String>,
    pub independence_statement: Option<String>,
    pub decision: Option<String>,
    pub statement: Option<String>,
    pub reviewed_utc: Option<String>,
    pub how_to_complete: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reproduce {
    pub recipe: &'static str,
    pub command: String,
    pub note: String,
}

/// The forensic report.
#[derive(Debug, Clone, Serialize)]
pub struct ForensicReport {
    pub format: &'static str,
    pub method_id: &'static str,
    pub variant: Variant,
    /// `final` (standard), `awaiting-expert-review` or `claim-grade-checklist-not-met`.
    pub status: &'static str,
    pub label: &'static str,
    pub validation_status: &'static str,
    pub incident: Incident,
    pub generated_utc: String,
    pub engine: EngineInfo,
    pub method: MethodParams,
    pub inputs: Vec<InputRecord>,
    pub verdict: Verdict,
    pub findings: Vec<Finding>,
    pub receivers: Vec<ReceiverSummary>,
    pub area_activity: Vec<AreaActivity>,
    pub area: AffectedArea,
    pub tracks: Vec<Track>,
    pub timeline: Vec<TimelineEntry>,
    pub statements: Vec<StatementInput>,
    pub checklist: Vec<CheckItem>,
    pub expert_review: ExpertReviewFields,
    pub limits: Vec<String>,
    pub reproduce: Reproduce,
}

// ─── Receiver logs ────────────────────────────────────────────────────────────────────

const JAM: &[Monitor] = &[
    Monitor::Cn0Drop,
    Monitor::Agc,
    Monitor::JamInd,
    Monitor::LossOfLock,
    Monitor::SolveFailure,
];
/// Spoofing-like indicators in the signal domain (C/N0 shape).
const SPOOF_SIGNAL: &[Monitor] = &[Monitor::Cn0Rise, Monitor::Cn0Spread];
/// Spoofing-like indicators in the position, time and authentication domain.
const SPOOF_STATE: &[Monitor] = &[
    Monitor::PositionJump,
    Monitor::Raim,
    Monitor::Clock,
    Monitor::Kinematic,
    Monitor::HeadingCourse,
    Monitor::SpeedLog,
    Monitor::SeaLevel,
    Monitor::TimeConsistency,
    Monitor::Osnma,
];

/// The attribution rules of a method version. The parameters ([`MethodParams::V1`]) are the
/// same for both; v2 changes only how an epoch counts toward an episode's spoofing-like share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    /// `kshana-pro/incident-forensics/v1`.
    V1,
    /// `kshana-pro/incident-forensics/v2` (V2b pre-registration,
    /// `docs/evidence/v2-forensics-detection/2026-11-16_V2b_PREREGISTRATION.md`): R1, a
    /// `cn0-spread` or `cn0-rise` alarm counts only in an epoch with no jamming-family alarm
    /// and a median tracked C/N0 at most [`V2_MEDIAN_DROP_DB`] below the baseline median;
    /// R2, in an epoch with a jamming-family alarm the position-domain state monitors do not
    /// count (`time-consistency`, `osnma` and `sea-level` always count).
    V2,
}

/// The method id of method v2.
pub const METHOD_ID_V2: &str = "kshana-pro/incident-forensics/v2";
/// R1's power-consistency limit, dB (fixed a priori by the V2b pre-registration).
pub const V2_MEDIAN_DROP_DB: f64 = 6.0;
/// R2: the position-domain state monitors that do not count in a jammed epoch under v2.
const POSITION_DOMAIN: &[Monitor] = &[
    Monitor::PositionJump,
    Monitor::Raim,
    Monitor::Clock,
    Monitor::Kinematic,
    Monitor::HeadingCourse,
    Monitor::SpeedLog,
];

impl Rules {
    pub fn method_id(self) -> &'static str {
        match self {
            Rules::V1 => METHOD_ID,
            Rules::V2 => METHOD_ID_V2,
        }
    }
}

fn monitor_name(m: Monitor) -> String {
    serde_json::to_value(m)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{m:?}"))
}

pub fn trust_scenario_for(
    r: &ReceiverInput,
    p: &MethodParams,
) -> Result<ReceiverTrustScenario, KshanaError> {
    let fmt = serde_json::to_value(r.format)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    let cal = match r.platform {
        Platform::Static => p.static_calibration_s,
        Platform::Vessel => p.vessel_calibration_s,
    };
    let mut t = format!(
        "kind = \"receiver-trust\"\nname = {name:?}\n[log]\nformat = \"{fmt}\"\npath = \"input\"\n\
         [monitors]\ncalibration_s = {cal:.1}\ncn0_drop_db = {:.1}\nsats_lost = {}\nposition_jump_m = {:.1}\n",
        p.cn0_drop_db,
        p.sats_lost,
        p.position_jump_m,
        name = r.label,
    );
    if r.platform == Platform::Vessel {
        t.push_str(&format!(
            "[platform]\nkind = \"vessel\"\nmax_speed_kn = {:.2}\nmax_accel_mps2 = {:.3}\nmax_turn_rate_dps = {:.2}\n\
             antenna_height_m = {:.2}\nheading_sensor = {}\n",
            r.max_speed_kn.unwrap_or(p.vessel_max_speed_kn),
            p.vessel_max_accel_mps2,
            p.vessel_max_turn_rate_dps,
            r.antenna_height_m.unwrap_or(p.vessel_antenna_height_m),
            r.heading_sensor.unwrap_or(true),
        ));
    }
    toml::from_str(&t).map_err(|e| KshanaError::InvalidInput(format!("{}: monitors: {e}", r.label)))
}

/// One alarm episode of a receiver log.
struct Episode {
    start_s: f64,
    end_s: f64,
    clean_before_s: Option<f64>,
    clean_after_s: Option<f64>,
    epochs: Vec<usize>,
}

fn episodes(epochs: &[EpochTrust], merge_gap_s: f64) -> Vec<Episode> {
    let alarmed = |e: &EpochTrust| !e.alarms.is_empty() && e.state != TrustState::Calibrating;
    let clean = |e: &EpochTrust| e.alarms.is_empty() && e.state != TrustState::Calibrating;
    let mut out: Vec<Episode> = Vec::new();
    for (i, e) in epochs.iter().enumerate() {
        if !alarmed(e) {
            continue;
        }
        match out.last_mut() {
            Some(ep) if e.t_s - ep.end_s <= merge_gap_s => {
                ep.end_s = e.t_s;
                ep.epochs.push(i);
            }
            _ => out.push(Episode {
                start_s: e.t_s,
                end_s: e.t_s,
                clean_before_s: None,
                clean_after_s: None,
                epochs: vec![i],
            }),
        }
    }
    for ep in &mut out {
        ep.clean_before_s = epochs
            .iter()
            .filter(|e| e.t_s < ep.start_s && clean(e))
            .map(|e| e.t_s)
            .next_back();
        ep.clean_after_s = epochs
            .iter()
            .find(|e| e.t_s > ep.end_s && clean(e))
            .map(|e| e.t_s);
    }
    out
}

/// Per-satellite baseline and the breadth of a loss at given epochs.
struct Breadth {
    /// Median C/N0 per (satellite, band) over the calibration window.
    base: BTreeMap<(String, String), f64>,
    /// Median elevation per satellite over the calibration window, where reported.
    elev: BTreeMap<String, f64>,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

impl Breadth {
    fn new(tl: &Timeline, cal_s: f64) -> Breadth {
        let mut c: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
        let mut el: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for e in tl.epochs.iter().filter(|e| e.t_s < cal_s) {
            for s in &e.cn0 {
                c.entry((s.sat.clone(), s.band.clone()))
                    .or_default()
                    .push(s.cn0_dbhz);
            }
            for s in &e.sky {
                el.entry(s.sat.clone()).or_default().push(s.elev_deg);
            }
        }
        Breadth {
            base: c
                .into_iter()
                .filter_map(|(k, mut v)| median(&mut v).map(|m| (k, m)))
                .collect(),
            elev: el
                .into_iter()
                .filter_map(|(k, mut v)| median(&mut v).map(|m| (k, m)))
                .collect(),
        }
    }

    /// (share of baseline keys affected, elevation of unaffected minus affected, degrees)
    /// at the timeline epoch nearest `t_s`.
    fn at(&self, tl: &Timeline, t_s: f64, drop_db: f64) -> Option<(f64, Option<f64>)> {
        if self.base.is_empty() {
            return None;
        }
        let e = tl
            .epochs
            .iter()
            .min_by(|a, b| (a.t_s - t_s).abs().total_cmp(&(b.t_s - t_s).abs()))?;
        let now: BTreeMap<(String, String), f64> = e
            .cn0
            .iter()
            .map(|s| ((s.sat.clone(), s.band.clone()), s.cn0_dbhz))
            .collect();
        let (mut aff, mut ok) = (Vec::new(), Vec::new());
        for (k, b) in &self.base {
            let hit = now.get(k).is_none_or(|c| b - c >= drop_db);
            if hit {
                aff.push(k.0.clone());
            } else {
                ok.push(k.0.clone());
            }
        }
        let share = aff.len() as f64 / self.base.len() as f64;
        let mean_el = |v: &[String]| {
            let xs: Vec<f64> = v.iter().filter_map(|s| self.elev.get(s)).copied().collect();
            (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64)
        };
        let bias = match (mean_el(&ok), mean_el(&aff)) {
            (Some(o), Some(a)) => Some(o - a),
            _ => None,
        };
        Some((share, bias))
    }
}

/// What one receiver log contributed.
pub(crate) struct ReceiverAnalysis {
    pub summary: ReceiverSummary,
    pub findings: Vec<Finding>,
    pub track: Option<Track>,
    pub times: Vec<f64>,
    /// Calibration end, Unix s.
    pub calibration_end: Option<f64>,
}

fn clean_or(t: Option<f64>, fallback: f64) -> f64 {
    t.unwrap_or(fallback)
}

#[allow(clippy::too_many_arguments)]
fn analyse_receiver(
    r: &ReceiverInput,
    log: &LoadedInput,
    nav: Option<&LoadedInput>,
    p: &MethodParams,
    rules: Rules,
    w0: f64,
    w1: f64,
    is_subject: bool,
) -> Result<ReceiverAnalysis, KshanaError> {
    let scn = trust_scenario_for(r, p)?;
    let res: ReceiverTrustResult =
        run_receiver_trust_bytes(&scn, &log.data, nav.map(|n| n.data.as_slice()))
            .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", r.label)))?;
    let tl = kshana::receiver_trust::ingest::read_log(r.format, &log.data)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", r.label)))?;
    let t0 = res.log.start_label.as_deref().and_then(parse_utc);
    let cal_s = match r.platform {
        Platform::Static => p.static_calibration_s,
        Platform::Vessel => p.vessel_calibration_s,
    };
    let abs = |t_s: f64| t0.map(|t| t + t_s);
    let times: Vec<f64> = tl.epochs.iter().filter_map(|e| abs(e.t_s)).collect();
    let breadth = Breadth::new(&tl, cal_s);
    // v2 (R1): the median C/N0 of the tracked satellites at each epoch, and the baseline
    // median (the median of those per-epoch medians over the calibration window).
    let median_cn0: BTreeMap<i64, f64> = tl
        .epochs
        .iter()
        .filter_map(|e| {
            let mut v: Vec<f64> = e.cn0.iter().map(|s| s.cn0_dbhz).collect();
            median(&mut v).map(|m| ((e.t_s * 1000.0).round() as i64, m))
        })
        .collect();
    let base_median = {
        let mut v: Vec<f64> = tl
            .epochs
            .iter()
            .filter(|e| e.t_s < cal_s)
            .filter_map(|e| median_cn0.get(&((e.t_s * 1000.0).round() as i64)).copied())
            .collect();
        median(&mut v)
    };
    let eps = episodes(&res.epochs, p.merge_gap_s);
    let (mut transients, mut outside) = (0usize, 0usize);
    let mut findings = Vec::new();
    let fix_at = |t_s: f64| -> Option<(f64, f64)> {
        tl.epochs
            .iter()
            .filter(|e| e.t_s <= t_s)
            .filter_map(|e| e.fix.map(|f| (f.lat_deg, f.lon_deg)))
            .next_back()
    };
    let mut flagged_spans: Vec<(f64, f64)> = Vec::new();
    for ep in &eps {
        let (Some(a), Some(b)) = (abs(ep.start_s), abs(ep.end_s)) else {
            continue;
        };
        if b < w0 || a > w1 {
            outside += 1;
            continue;
        }
        if ep.end_s - ep.start_s < p.min_episode_s {
            transients += 1;
            continue;
        }
        flagged_spans.push((ep.start_s, ep.end_s));
        let mut fired: BTreeSet<Monitor> = BTreeSet::new();
        let (mut jam_n, mut sig_n, mut st_n) = (0usize, 0usize, 0usize);
        // Epochs each monitor counted toward its family (diagnostic, not part of the verdict).
        let mut counted: BTreeMap<String, usize> = BTreeMap::new();
        let mut mag: BTreeMap<String, f64> = BTreeMap::new();
        let put_max = |k: &str, v: f64, m: &mut BTreeMap<String, f64>| {
            let e = m.entry(k.to_string()).or_insert(v);
            if v > *e {
                *e = v;
            }
        };
        let mut min_sats = usize::MAX;
        let mut self_flags = 0usize;
        for &i in &ep.epochs {
            let e = &res.epochs[i];
            fired.extend(e.alarms.iter().copied());
            let jammed = e.alarms.iter().any(|m| JAM.contains(m));
            if jammed {
                jam_n += 1;
            }
            let (sig, st) = match rules {
                Rules::V1 => (
                    e.alarms.iter().any(|m| SPOOF_SIGNAL.contains(m)),
                    e.alarms.iter().any(|m| SPOOF_STATE.contains(m)),
                ),
                Rules::V2 => {
                    // R1: signal-shape evidence counts only at unjammed, unlowered power.
                    let power_ok = !jammed
                        && match (
                            median_cn0.get(&((e.t_s * 1000.0).round() as i64)).copied(),
                            base_median,
                        ) {
                            (Some(m), Some(b)) => b - m <= V2_MEDIAN_DROP_DB,
                            _ => false,
                        };
                    let sig = power_ok && e.alarms.iter().any(|m| SPOOF_SIGNAL.contains(m));
                    // R2: position-domain state evidence does not count in a jammed epoch.
                    let st = e.alarms.iter().any(|m| {
                        SPOOF_STATE.contains(m) && !(jammed && POSITION_DOMAIN.contains(m))
                    });
                    (sig, st)
                }
            };
            if sig {
                sig_n += 1;
                for m in e.alarms.iter().filter(|m| SPOOF_SIGNAL.contains(m)) {
                    *counted.entry(monitor_name(*m)).or_insert(0) += 1;
                }
            }
            if st {
                st_n += 1;
                for m in e.alarms.iter().filter(|m| SPOOF_STATE.contains(m)) {
                    if rules == Rules::V1 || !(jammed && POSITION_DOMAIN.contains(m)) {
                        *counted.entry(monitor_name(*m)).or_insert(0) += 1;
                    }
                }
            }
            if jammed {
                for m in e.alarms.iter().filter(|m| JAM.contains(m)) {
                    *counted.entry(monitor_name(*m)).or_insert(0) += 1;
                }
            }
            if let Some(d) = e.cn0_drop_db {
                put_max("peak_cn0_drop_db", round(d, 1), &mut mag);
            }
            if let Some(o) = e.position_offset_m {
                put_max("max_position_offset_m", round(o, 1), &mut mag);
            }
            if let Some(z) = e.agc_z {
                put_max("max_agc_z", round(z, 1), &mut mag);
            }
            if let Some(j) = e.jam_ind {
                put_max("max_jam_ind", j, &mut mag);
            }
            min_sats = min_sats.min(e.n_sats);
            if e.self_report.as_ref().is_some_and(|s| s.receiver_flag) {
                self_flags += 1;
            }
        }
        if min_sats != usize::MAX {
            mag.insert("min_satellites_tracked".into(), min_sats as f64);
        }
        if let Some(m) = res.baseline.tracked_median {
            mag.insert("baseline_satellites_tracked".into(), m);
        }
        if rules == Rules::V2 {
            // v2 reports which monitors counted toward each family (V2b section 6).
            for (k, v) in &counted {
                mag.insert(format!("v2_counted_{k}_epochs"), *v as f64);
            }
            mag.insert("v2_episode_epochs".into(), ep.epochs.len() as f64);
            mag.insert("v2_spoofing_like_epochs".into(), sig_n.max(st_n) as f64);
        }
        let n = ep.epochs.len().max(1) as f64;
        let spoof_n = sig_n.max(st_n);
        let jam_share = jam_n as f64 / n;
        let spoof_share = spoof_n as f64 / n;
        // Breadth at the epoch of the largest loss inside the episode.
        let mut best: Option<(f64, Option<f64>)> = None;
        for &i in &ep.epochs {
            if let Some(b) = breadth.at(&tl, res.epochs[i].t_s, p.breadth_drop_db) {
                if best.is_none_or(|x| b.0 > x.0) {
                    best = Some(b);
                }
            }
        }
        if let Some((s, bias)) = best {
            mag.insert("breadth_share".into(), round(s, 2));
            if let Some(b) = bias {
                mag.insert("elevation_bias_deg".into(), round(b, 1));
            }
        }
        let broad = best.is_some_and(|(s, _)| s >= p.breadth_min_share);
        let obstruction = best.is_some_and(|(s, bias)| {
            s < p.breadth_min_share && bias.is_some_and(|b| b >= p.obstruction_elev_margin_deg)
        });
        let jam_like = jam_share >= 0.2;
        let spoof_like = spoof_share >= 0.2;
        let category: &'static str = match (jam_like, spoof_like) {
            (true, true) if !obstruction => "jamming-and-spoofing",
            (true, false) if obstruction => "degradation-not-attributed",
            (true, true) => "spoofing",
            (true, false) => "jamming",
            (false, true) => "spoofing",
            (false, false) => "degradation-not-attributed",
        };
        let dur = ep.end_s - ep.start_s;
        let fam: Vec<&Monitor> = fired
            .iter()
            .filter(|m| match category {
                "jamming" => JAM.contains(m),
                "spoofing" => SPOOF_SIGNAL.contains(m) || SPOOF_STATE.contains(m),
                _ => true,
            })
            .collect();
        let mut pts = 0u8;
        let mut reasons = Vec::new();
        if dur >= p.strong_episode_s {
            pts += 1;
            reasons.push(format!(
                "sustained for {dur:.0} s (at least {:.0} s)",
                p.strong_episode_s
            ));
        } else {
            reasons.push(format!(
                "short: {dur:.0} s (under {:.0} s)",
                p.strong_episode_s
            ));
        }
        if fam.len() >= 2 {
            pts += 1;
            reasons.push(format!(
                "{} independent monitors of the attributed kind",
                fam.len()
            ));
        } else {
            reasons.push("a single monitor of the attributed kind".into());
        }
        let disc = match category {
            "jamming" | "jamming-and-spoofing" => broad,
            "spoofing" => fired.iter().any(|m| SPOOF_STATE.contains(m)),
            _ => false,
        };
        match category {
            "jamming" | "jamming-and-spoofing" if disc => reasons.push(format!(
                "the loss is broad: at least {:.0} % of the baseline satellites affected at once",
                p.breadth_min_share * 100.0
            )),
            "jamming" | "jamming-and-spoofing" => reasons.push(
                "the loss is not broad across satellites (or the log carries no per-satellite C/N0)"
                    .into(),
            ),
            "spoofing" if disc => reasons
                .push("a position, time or authentication monitor fired, not only C/N0 shape".into()),
            "spoofing" => reasons.push("only the C/N0 shape changed".into()),
            _ => reasons.push(
                "affected satellites are mostly low in elevation and the loss is narrow: consistent \
                 with obstruction (terrain, structures), not attributed to interference"
                    .into(),
            ),
        }
        if disc {
            pts += 1;
        }
        let onset_lo = abs(clean_or(ep.clean_before_s, ep.start_s)).unwrap_or(a);
        let end_hi = abs(clean_or(ep.clean_after_s, ep.end_s)).unwrap_or(b);
        let pos = fix_at(clean_or(ep.clean_before_s, ep.start_s));
        let indicators: Vec<String> = fired.iter().map(|m| monitor_name(*m)).collect();
        let mut evidence = vec![
            EvidenceItem {
                class: "measured",
                statement: format!(
                    "{}: the receiver's own observables ({}) in the episode",
                    r.label,
                    tl.observables.join(", ")
                ),
                input_sha256: Some(log.record.sha256.clone()),
            },
            EvidenceItem {
                class: "modelled",
                statement: format!(
                    "the open engine's receiver-trust monitors ({}) at the method's thresholds, \
                     attributed by {}",
                    indicators.join(", "),
                    rules.method_id()
                ),
                input_sha256: None,
            },
        ];
        if self_flags > 0 {
            evidence.push(EvidenceItem {
                class: "reported",
                statement: format!(
                    "the receiver itself flagged spoofing or jamming at {self_flags} epoch(s) of the episode \
                     (its own detector, not validated here)"
                ),
                input_sha256: Some(log.record.sha256.clone()),
            });
        }
        findings.push(Finding {
            id: String::new(),
            category,
            source: r.label.clone(),
            source_kind: "receiver",
            data_class: "measured",
            inference_class: "modelled",
            onset: TimeBound {
                earliest_utc: iso(onset_lo),
                latest_utc: iso(a),
                uncertainty_s: round(a - onset_lo, 1),
                basis: "last clean epoch before the episode to its first alarmed epoch".into(),
            },
            end: TimeBound {
                earliest_utc: iso(b),
                latest_utc: iso(end_hi),
                uncertainty_s: round(end_hi - b, 1),
                basis: "last alarmed epoch to the first clean epoch after it".into(),
            },
            duration_s: round(b - a, 1),
            indicators,
            magnitude: mag,
            position_at_onset: pos.map(|(lat, lon)| Position {
                lat: round(lat, 5),
                lon: round(lon, 5),
                class: "measured",
                note: "the receiver's own fix at the last clean epoch before the episode".into(),
            }),
            confidence: Confidence {
                level: "low",
                points: pts,
                reasons: vec![],
            },
            corroborated_by: vec![],
            supported: false,
            about_subject: is_subject,
            evidence,
            statement: String::new(),
            t_onset: a,
            t_end: b,
            points_wo_corr: pts,
            reasons_wo_corr: reasons,
            affected: pos.into_iter().collect(),
        });
    }
    let track = if is_subject {
        let mut pts: Vec<TrackPoint> = Vec::new();
        let step = (tl.epochs.len() / 600).max(1);
        for (i, e) in tl.epochs.iter().enumerate() {
            let (Some(f), Some(t)) = (e.fix, abs(e.t_s)) else {
                continue;
            };
            if t < w0 - p.baseline_pad_s || t > w1 + p.baseline_pad_s {
                continue;
            }
            let flagged = flagged_spans
                .iter()
                .any(|(a, b)| e.t_s >= *a && e.t_s <= *b);
            if i % step == 0 || flagged {
                pts.push(TrackPoint {
                    utc: iso(t),
                    lat: round(f.lat_deg, 5),
                    lon: round(f.lon_deg, 5),
                    flagged,
                });
            }
        }
        (!pts.is_empty()).then(|| Track {
            source: r.label.clone(),
            class: "measured",
            points: pts,
        })
    } else {
        None
    };
    Ok(ReceiverAnalysis {
        summary: ReceiverSummary {
            label: r.label.clone(),
            role: r.role.as_str(),
            platform: match r.platform {
                Platform::Static => "static",
                Platform::Vessel => "vessel",
            },
            format: r.format,
            input_sha256: log.record.sha256.clone(),
            epochs: res.log.epochs,
            start_utc: t0.map(iso),
            calibration_end_utc: abs(cal_s).map(iso),
            observables: res.log.observables.clone(),
            monitors_run: res.monitors_run.clone(),
            nominal_epochs: res.states.nominal_epochs,
            degraded_epochs: res.states.degraded_epochs,
            untrusted_epochs: res.states.untrusted_epochs,
            transients,
            episodes_outside_window: outside,
            receiver_self_report: res.receiver_self_report.clone(),
            engine_verdict: res.verdict.clone(),
            engine_label: res.label.clone(),
        },
        findings,
        track,
        times,
        calibration_end: abs(cal_s),
    })
}

// ─── AIS and ADS-B ────────────────────────────────────────────────────────────────────

const KN_PER_MPS: f64 = 1.943_844_492;

/// One detector hit on one track.
#[derive(Debug, Clone)]
struct Hit {
    t: f64,
    id: String,
    /// The last position before the hit that the detector did not flag.
    last_clean: Option<(f64, f64)>,
    last_clean_t: Option<f64>,
    value: f64,
}

fn bearing_deg(a: &PositionReport, b: &PositionReport) -> f64 {
    let (la1, la2) = (a.lat.to_radians(), b.lat.to_radians());
    let dl = (b.lon - a.lon).to_radians();
    let y = dl.sin() * la2.cos();
    let x = la1.cos() * la2.sin() - la1.sin() * la2.cos() * dl.cos();
    y.atan2(x).to_degrees()
}

fn in_scope(r: &PositionReport, w0: f64, w1: f64, inc: &Incident) -> bool {
    r.t >= w0 && r.t <= w1 && inc.area.is_none_or(|a| a.contains(r.lat, r.lon))
}

fn jumps(v: &[PositionReport], max_kn: f64, min_m: f64, max_dt: f64) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    for i in 1..v.len() {
        let (a, b) = (&v[i - 1], &v[i]);
        let dt = b.t - a.t;
        if dt <= 0.0 || dt > max_dt {
            continue;
        }
        let d = haversine_m(a.lat, a.lon, b.lat, b.lon);
        let kn = d / dt * KN_PER_MPS;
        if d >= min_m && kn > max_kn {
            out.push((i, kn));
        }
    }
    out
}

/// Circular tracks: a run of reports whose course turns by at least `turn_deg` in total,
/// within `max_span_s`, in one direction.
fn circles(v: &[PositionReport], p: &MethodParams) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    if v.len() < p.ais_circle_min_reports {
        return out;
    }
    let br: Vec<f64> = v.windows(2).map(|w| bearing_deg(&w[0], &w[1])).collect();
    let turns: Vec<f64> = br
        .windows(2)
        .map(|w| {
            let mut d = w[1] - w[0];
            while d > 180.0 {
                d -= 360.0;
            }
            while d < -180.0 {
                d += 360.0;
            }
            d
        })
        .collect();
    let mut i = 0;
    while i < turns.len() {
        let mut sum = 0.0;
        let mut j = i;
        while j < turns.len() && v[j + 2].t - v[i].t <= p.ais_circle_max_span_s {
            sum += turns[j];
            if sum.abs() >= p.ais_circle_turn_deg && j + 2 - i + 1 >= p.ais_circle_min_reports {
                out.push((i + 1, sum.abs()));
                break;
            }
            j += 1;
        }
        if out.last().is_some_and(|(k, _)| *k == i + 1) {
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn hit_at(v: &[PositionReport], i: usize, value: f64) -> Hit {
    let prev = i.checked_sub(1).map(|k| &v[k]);
    Hit {
        t: v[i].t,
        id: v[i].id.clone(),
        last_clean: prev.map(|r| (r.lat, r.lon)),
        last_clean_t: prev.map(|r| r.t),
        value,
    }
}

struct TrackAnalysis {
    activity: AreaActivity,
    findings: Vec<Finding>,
    track: Option<Track>,
    unaffected: Vec<(f64, f64)>,
}

#[allow(clippy::too_many_arguments)]
fn track_finding(
    mid: &'static str,
    det: &str,
    category: &'static str,
    source_kind: &'static str,
    input: &LoadedInput,
    hits: &[Hit],
    subject: Option<&str>,
    what: &str,
    unit_key: &str,
) -> Option<Finding> {
    if hits.is_empty() {
        return None;
    }
    let ids: BTreeSet<&str> = hits.iter().map(|h| h.id.as_str()).collect();
    let a = hits.iter().map(|h| h.t).fold(f64::INFINITY, f64::min);
    let b = hits.iter().map(|h| h.t).fold(f64::NEG_INFINITY, f64::max);
    let lo = hits
        .iter()
        .filter(|h| h.t == a)
        .filter_map(|h| h.last_clean_t)
        .fold(a, f64::min);
    let about_subject = subject.is_some_and(|s| ids.contains(s));
    let mut mag = BTreeMap::new();
    let what_n = if source_kind == "ais" {
        "vessels"
    } else {
        "aircraft"
    };
    mag.insert(format!("{what_n}_flagged"), ids.len() as f64);
    mag.insert("reports_flagged".into(), hits.len() as f64);
    let peak = hits
        .iter()
        .map(|h| h.value)
        .fold(f64::NEG_INFINITY, f64::max);
    if peak.is_finite() && !unit_key.is_empty() {
        mag.insert(unit_key.into(), round(peak, 1));
    }
    let affected: Vec<(f64, f64)> = {
        let mut seen = BTreeSet::new();
        hits.iter()
            .filter(|h| seen.insert(h.id.clone()))
            .filter_map(|h| h.last_clean)
            .collect()
    };
    let mut pts = 0u8;
    let mut reasons = Vec::new();
    if b - a >= MethodParams::V1.strong_episode_s {
        pts += 1;
        reasons.push(format!("seen over {:.0} s", b - a));
    } else {
        reasons.push("seen at one moment only".into());
    }
    if ids.len() >= 2 {
        pts += 1;
        reasons.push(format!("{} distinct {what_n} affected", ids.len()));
    } else {
        reasons.push(format!(
            "one {} affected",
            &what_n[..what_n.len() - if what_n == "vessels" { 1 } else { 0 }]
        ));
    }
    if hits.len() >= 3 {
        pts += 1;
        reasons.push(format!("{} reports flagged", hits.len()));
    } else {
        reasons.push(format!("only {} report(s) flagged", hits.len()));
    }
    let list: Vec<&str> = ids.iter().copied().take(20).collect();
    Some(Finding {
        id: String::new(),
        category,
        source: input.record.label.clone(),
        source_kind,
        data_class: "reported",
        inference_class: "modelled",
        onset: TimeBound {
            earliest_utc: iso(lo),
            latest_utc: iso(a),
            uncertainty_s: round(a - lo, 1),
            basis: "the report before the first flagged report to the first flagged report".into(),
        },
        end: TimeBound {
            earliest_utc: iso(b),
            latest_utc: iso(b),
            uncertainty_s: 0.0,
            basis: "the last flagged report (the next report is not used: a track may end)".into(),
        },
        duration_s: round(b - a, 1),
        indicators: vec![det.to_string()],
        magnitude: mag,
        position_at_onset: hits
            .iter()
            .find(|h| h.t == a)
            .and_then(|h| h.last_clean)
            .map(|(lat, lon)| Position {
                lat: round(lat, 5),
                lon: round(lon, 5),
                class: "reported",
                note: "the broadcast position before the first flagged report".into(),
            }),
        confidence: Confidence {
            level: "low",
            points: pts,
            reasons: vec![],
        },
        corroborated_by: vec![],
        supported: false,
        about_subject,
        evidence: vec![
            EvidenceItem {
                class: "reported",
                statement: format!(
                    "{}: broadcast positions of {} ({}{})",
                    input.record.label,
                    what_n,
                    list.join(", "),
                    if ids.len() > list.len() { ", ..." } else { "" }
                ),
                input_sha256: Some(input.record.sha256.clone()),
            },
            EvidenceItem {
                class: "modelled",
                statement: format!("{what} ({mid})"),
                input_sha256: None,
            },
        ],
        statement: String::new(),
        t_onset: a,
        t_end: b,
        points_wo_corr: pts,
        reasons_wo_corr: reasons,
        affected,
    })
}

fn subject_track(
    tr: &Tracks,
    subject: Option<&str>,
    flagged: &BTreeSet<(String, i64)>,
    label: &str,
    w0: f64,
    w1: f64,
    pad: f64,
) -> Option<Track> {
    let s = subject?;
    let v = tr.by_id.get(s)?;
    let pts: Vec<TrackPoint> = v
        .iter()
        .filter(|r| r.t >= w0 - pad && r.t <= w1 + pad)
        .map(|r| TrackPoint {
            utc: iso(r.t),
            lat: round(r.lat, 5),
            lon: round(r.lon, 5),
            flagged: flagged.contains(&(r.id.clone(), (r.t * 10.0) as i64)),
        })
        .collect();
    (!pts.is_empty()).then(|| Track {
        source: label.to_string(),
        class: "reported",
        points: pts,
    })
}

fn analyse_ais(
    mid: &'static str,
    input: &LoadedInput,
    inc: &Incident,
    p: &MethodParams,
    w0: f64,
    w1: f64,
) -> Result<(TrackAnalysis, Vec<f64>), KshanaError> {
    let text = String::from_utf8_lossy(&input.data);
    let tr = parse_tracks(&text, "vessel_id")
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", input.record.label)))?;
    let subject = inc.subject_id.as_deref();
    let (mut j_hits, mut s_hits, mut c_hits, mut p_hits) = (vec![], vec![], vec![], vec![]);
    let mut observed = BTreeSet::new();
    let mut unaffected = Vec::new();
    for (id, v) in &tr.by_id {
        let v: Vec<PositionReport> = v
            .iter()
            .filter(|r| r.t >= w0 - p.baseline_pad_s && r.t <= w1)
            .cloned()
            .collect();
        if !v.iter().any(|r| in_scope(r, w0, w1, inc)) {
            continue;
        }
        observed.insert(id.clone());
        let n0 = j_hits.len() + s_hits.len() + c_hits.len();
        for (i, kn) in jumps(
            &v,
            p.ais_max_speed_kn,
            p.ais_jump_min_m,
            p.ais_jump_max_dt_s,
        ) {
            if in_scope(&v[i], w0, w1, inc) {
                j_hits.push(hit_at(&v, i, kn));
            }
        }
        for (i, r) in v.iter().enumerate() {
            if r.sog_kn.is_some_and(|s| s > p.ais_max_speed_kn) && in_scope(r, w0, w1, inc) {
                s_hits.push(hit_at(&v, i, r.sog_kn.unwrap_or(0.0)));
            }
        }
        for (i, turn) in circles(&v, p) {
            if in_scope(&v[i], w0, w1, inc) {
                c_hits.push(hit_at(&v, i, turn));
            }
        }
        if j_hits.len() + s_hits.len() + c_hits.len() == n0 {
            let inside: Vec<&PositionReport> =
                v.iter().filter(|r| in_scope(r, w0, w1, inc)).collect();
            if let Some(r) = inside.get(inside.len() / 2) {
                unaffected.push((r.lat, r.lon));
            }
        }
    }
    // Many vessels at one rounded position in one time bin.
    let mut groups: BTreeMap<(i64, i64, i64), BTreeMap<String, PositionReport>> = BTreeMap::new();
    for v in tr.by_id.values() {
        for r in v.iter().filter(|r| in_scope(r, w0, w1, inc)) {
            let k = (
                (r.lat / p.ais_same_pos_round_deg).round() as i64,
                (r.lon / p.ais_same_pos_round_deg).round() as i64,
                (r.t / p.ais_same_pos_window_s).floor() as i64,
            );
            groups
                .entry(k)
                .or_default()
                .entry(r.id.clone())
                .or_insert_with(|| r.clone());
        }
    }
    for g in groups
        .values()
        .filter(|g| g.len() >= p.ais_same_pos_min_vessels)
    {
        for r in g.values() {
            let v = &tr.by_id[&r.id];
            let i = v.iter().position(|x| x.t == r.t).unwrap_or(0);
            p_hits.push(hit_at(v, i, g.len() as f64));
        }
    }
    let mut flagged: BTreeMap<String, usize> = BTreeMap::new();
    let mut any = BTreeSet::new();
    let mut fl_pts = BTreeSet::new();
    for (name, hs) in [
        ("ais-position-jump", &j_hits),
        ("ais-implausible-speed", &s_hits),
        ("ais-circular-track", &c_hits),
        ("ais-same-position", &p_hits),
    ] {
        let ids: BTreeSet<&str> = hs.iter().map(|h| h.id.as_str()).collect();
        flagged.insert(name.into(), ids.len());
        any.extend(ids.iter().map(|s| s.to_string()));
        for h in hs.iter() {
            fl_pts.insert((h.id.clone(), (h.t * 10.0) as i64));
        }
    }
    unaffected.retain(|_| true);
    let mut findings = Vec::new();
    findings.extend(track_finding(
        mid,
        "ais-position-jump",
        "spoofing",
        "ais",
        input,
        &j_hits,
        subject,
        &format!(
            "consecutive broadcast positions at least {:.0} m apart implying more than {:.0} kn",
            p.ais_jump_min_m, p.ais_max_speed_kn
        ),
        "peak_implied_speed_kn",
    ));
    findings.extend(track_finding(
        mid,
        "ais-implausible-speed",
        "spoofing",
        "ais",
        input,
        &s_hits,
        subject,
        &format!(
            "reported speed over ground above {:.0} kn",
            p.ais_max_speed_kn
        ),
        "peak_reported_speed_kn",
    ));
    findings.extend(track_finding(
        mid,
        "ais-circular-track",
        "spoofing",
        "ais",
        input,
        &c_hits,
        subject,
        &format!(
            "a broadcast track turning through at least {:.0} degrees within {:.0} s",
            p.ais_circle_turn_deg, p.ais_circle_max_span_s
        ),
        "peak_turn_deg",
    ));
    findings.extend(track_finding(
        mid,
        "ais-same-position",
        "spoofing",
        "ais",
        input,
        &p_hits,
        subject,
        &format!(
            "at least {} distinct vessels at one position (rounded to {} degrees) within {:.0} s",
            p.ais_same_pos_min_vessels, p.ais_same_pos_round_deg, p.ais_same_pos_window_s
        ),
        "largest_group_vessels",
    ));
    let track = subject_track(
        &tr,
        subject,
        &fl_pts,
        &input.record.label,
        w0,
        w1,
        p.baseline_pad_s,
    );
    let times = tr.times();
    Ok((
        TrackAnalysis {
            activity: AreaActivity {
                label: input.record.label.clone(),
                source: "ais",
                input_sha256: input.record.sha256.clone(),
                rows: tr.rows,
                rows_refused: tr.refused,
                observed: observed.len(),
                flagged,
                flagged_any: any.len(),
                unassessed: None,
            },
            findings,
            track,
            unaffected,
        },
        times,
    ))
}

fn analyse_adsb(
    mid: &'static str,
    input: &LoadedInput,
    inc: &Incident,
    p: &MethodParams,
    w0: f64,
    w1: f64,
) -> Result<(TrackAnalysis, Vec<f64>), KshanaError> {
    let text = String::from_utf8_lossy(&input.data);
    let tr = parse_tracks(&text, "aircraft_id")
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", input.record.label)))?;
    let subject = inc.subject_id.as_deref();
    let good = |r: &PositionReport| {
        r.nic.is_some_and(|n| n >= p.adsb_nic_min) || r.nacp.is_some_and(|n| n >= p.adsb_nacp_min)
    };
    let degraded = |r: &PositionReport| {
        r.nic.is_some_and(|n| n < p.adsb_nic_min) || r.nacp.is_some_and(|n| n < p.adsb_nacp_min)
    };
    let (mut d_hits, mut j_hits) = (vec![], vec![]);
    let mut observed = BTreeSet::new();
    let mut unassessed = 0usize;
    let mut unaffected = Vec::new();
    for (id, all) in &tr.by_id {
        let v: Vec<PositionReport> = all
            .iter()
            .filter(|r| r.t >= w0 - p.baseline_pad_s && r.t <= w1)
            .cloned()
            .collect();
        if !v.iter().any(|r| in_scope(r, w0, w1, inc)) {
            continue;
        }
        observed.insert(id.clone());
        let n0 = d_hits.len() + j_hits.len();
        // Degradation is judged only against what the same aircraft reported before.
        let first_good = v.iter().position(good);
        match first_good {
            None => unassessed += 1,
            Some(g) => {
                let mut last_good: Option<&PositionReport> = Some(&v[g]);
                for r in &v[g..] {
                    if good(r) && !degraded(r) {
                        last_good = Some(r);
                    } else if degraded(r) && in_scope(r, w0, w1, inc) {
                        let epu = r
                            .nacp
                            .and_then(kshana::interference_map::adsb::nacp_epu_bound_m)
                            .unwrap_or(f64::NAN);
                        d_hits.push(Hit {
                            t: r.t,
                            id: r.id.clone(),
                            last_clean: last_good.map(|x| (x.lat, x.lon)),
                            last_clean_t: last_good.map(|x| x.t),
                            value: epu,
                        });
                    }
                }
            }
        }
        for (i, kn) in jumps(&v, p.adsb_max_speed_kn, p.adsb_jump_min_m, 600.0) {
            if in_scope(&v[i], w0, w1, inc) {
                j_hits.push(hit_at(&v, i, kn));
            }
        }
        if d_hits.len() + j_hits.len() == n0 {
            let inside: Vec<&PositionReport> =
                v.iter().filter(|r| in_scope(r, w0, w1, inc)).collect();
            if let Some(r) = inside.get(inside.len() / 2) {
                unaffected.push((r.lat, r.lon));
            }
        }
    }
    let mut flagged = BTreeMap::new();
    let mut any = BTreeSet::new();
    let mut fl_pts = BTreeSet::new();
    for (name, hs) in [
        ("adsb-integrity-drop", &d_hits),
        ("adsb-position-jump", &j_hits),
    ] {
        let ids: BTreeSet<&str> = hs.iter().map(|h| h.id.as_str()).collect();
        flagged.insert(name.to_string(), ids.len());
        any.extend(ids.iter().map(|s| s.to_string()));
        for h in hs.iter() {
            fl_pts.insert((h.id.clone(), (h.t * 10.0) as i64));
        }
    }
    let mut findings = Vec::new();
    findings.extend(track_finding(
        mid,
        "adsb-integrity-drop",
        "jamming",
        "adsb",
        input,
        &d_hits,
        subject,
        &format!(
            "NIC below {} or NACp below {} from an aircraft that reported at or above them earlier \
             (consistent with interference; jamming is the usual cause, spoofing can also do it)",
            p.adsb_nic_min, p.adsb_nacp_min
        ),
        "peak_epu_bound_m",
    ));
    findings.extend(track_finding(
        mid,
        "adsb-position-jump",
        "spoofing",
        "adsb",
        input,
        &j_hits,
        subject,
        &format!(
            "consecutive broadcast positions at least {:.0} m apart implying more than {:.0} kn",
            p.adsb_jump_min_m, p.adsb_max_speed_kn
        ),
        "peak_implied_speed_kn",
    ));
    let track = subject_track(
        &tr,
        subject,
        &fl_pts,
        &input.record.label,
        w0,
        w1,
        p.baseline_pad_s,
    );
    let times = tr.times();
    Ok((
        TrackAnalysis {
            activity: AreaActivity {
                label: input.record.label.clone(),
                source: "adsb",
                input_sha256: input.record.sha256.clone(),
                rows: tr.rows,
                rows_refused: tr.refused,
                observed: observed.len(),
                flagged,
                flagged_any: any.len(),
                unassessed: Some(unassessed),
            },
            findings,
            track,
            unaffected,
        },
        times,
    ))
}

// ─── Putting it together ──────────────────────────────────────────────────────────────

fn compatible(a: &str, b: &str) -> bool {
    let j = |c: &str| c == "jamming" || c == "jamming-and-spoofing";
    let s = |c: &str| c == "spoofing" || c == "jamming-and-spoofing";
    (j(a) && j(b)) || (s(a) && s(b))
}

fn corroborate(findings: &mut [Finding], window_s: f64) {
    let n = findings.len();
    for i in 0..n {
        let mut by = Vec::new();
        for k in 0..n {
            if i == k || findings[i].source == findings[k].source {
                continue;
            }
            let (a, b) = (&findings[i], &findings[k]);
            if a.category == "degradation-not-attributed"
                || b.category == "degradation-not-attributed"
                || !compatible(a.category, b.category)
            {
                continue;
            }
            let overlap = a.t_onset <= b.t_end + window_s && b.t_onset <= a.t_end + window_s;
            if overlap {
                by.push(b.id.clone());
            }
        }
        let f = &mut findings[i];
        let mut reasons = f.reasons_wo_corr.clone();
        let mut pts = f.points_wo_corr;
        if f.category == "degradation-not-attributed" {
            reasons.push("not attributed: no confidence in interference is stated".into());
        } else if by.is_empty() {
            reasons.push("no independent source shows a compatible effect at that time".into());
        } else {
            pts += 1;
            reasons.push(format!(
                "corroborated by {} from another source within {:.0} s",
                by.join(", "),
                window_s
            ));
        }
        f.confidence = Confidence {
            level: match (pts, by.is_empty()) {
                (4, false) => "high",
                (3.., _) => "medium",
                (2, _) => "medium",
                _ => "low",
            },
            points: pts,
            reasons,
        };
        if f.category == "degradation-not-attributed" {
            f.confidence.level = "low";
        }
        f.supported = f.confidence.level == "high";
        f.corroborated_by = by;
    }
}

fn describe(f: &Finding) -> String {
    let what = match f.category {
        "jamming" => "Jamming-like interference",
        "spoofing" => "Spoofing-like manipulation",
        "jamming-and-spoofing" => "Jamming-like and spoofing-like effects together",
        _ => "A degradation not attributed to interference",
    };
    format!(
        "{what} in {} ({}) from between {} and {} for {:.0} s; indicators: {}; confidence {}.",
        f.source,
        f.data_class,
        f.onset.earliest_utc,
        f.onset.latest_utc,
        f.duration_s,
        f.indicators.join(", "),
        f.confidence.level
    )
}

fn category_verdict(findings: &[Finding], cat: &str, have_data: bool) -> CategoryVerdict {
    let rank = |l: &str| match l {
        "high" => 3,
        "medium" => 2,
        _ => 1,
    };
    let hits: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.category != "degradation-not-attributed" && compatible(f.category, cat))
        .collect();
    if hits.is_empty() {
        return CategoryVerdict {
            status: if have_data {
                "not-indicated"
            } else {
                "inconclusive"
            },
            confidence: None,
            findings: vec![],
        };
    }
    let best = hits
        .iter()
        .map(|f| f.confidence.level)
        .max_by_key(|l| rank(l))
        .unwrap_or("low");
    CategoryVerdict {
        status: "indicated",
        confidence: Some(best),
        findings: hits.iter().map(|f| f.id.clone()).collect(),
    }
}

fn affected_area(findings: &[Finding], unaffected: &[(f64, f64)], margin_m: f64) -> AffectedArea {
    let pts: Vec<(f64, f64)> = findings
        .iter()
        .filter(|f| f.category != "degradation-not-attributed")
        .flat_map(|f| f.affected.iter().copied())
        .collect();
    let note =
        "MODELLED: the extent of the observers (receivers, vessels, aircraft) that showed an \
effect, at their last clean position, plus a margin. It is not an emitter location. The real \
affected area may be larger (where nobody observed) or smaller (the margin), and unaffected \
observers inside the box weigh against a single wide-area source."
            .to_string();
    if pts.is_empty() {
        return AffectedArea {
            class: "modelled",
            affected_observers: 0,
            unaffected_observers_inside: 0,
            centroid: None,
            radius_m: None,
            bbox: None,
            points: vec![],
            note,
        };
    }
    let n = pts.len() as f64;
    let (clat, clon) = (
        pts.iter().map(|p| p.0).sum::<f64>() / n,
        pts.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let r = pts
        .iter()
        .map(|p| haversine_m(clat, clon, p.0, p.1))
        .fold(0.0, f64::max)
        + margin_m;
    let dlat = margin_m / 111_195.0;
    let dlon = margin_m / (111_195.0 * clat.to_radians().cos().max(0.05));
    let bbox = [
        pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min) - dlat,
        pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max) + dlat,
        pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min) - dlon,
        pts.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max) + dlon,
    ];
    let inside = unaffected
        .iter()
        .filter(|(la, lo)| *la >= bbox[0] && *la <= bbox[1] && *lo >= bbox[2] && *lo <= bbox[3])
        .count();
    AffectedArea {
        class: "modelled",
        affected_observers: pts.len(),
        unaffected_observers_inside: inside,
        centroid: Some([round(clat, 5), round(clon, 5)]),
        radius_m: Some(round(r, 0)),
        bbox: Some(bbox.map(|x| round(x, 5))),
        points: pts.iter().map(|p| [round(p.0, 5), round(p.1, 5)]).collect(),
        note,
    }
}

/// Run the method over an incident. `generated_utc` is the report's time stamp (the
/// command line's `--utc`); `None` uses the incident's window end, so a run without it is
/// reproducible too.
pub fn analyse(
    scn: &IncidentScenario,
    generated_utc: Option<&str>,
) -> Result<ForensicReport, KshanaError> {
    let (inputs, report) = analyse_with_inputs(scn, generated_utc)?;
    drop(inputs);
    Ok(report)
}

/// [`analyse`], also returning the inputs as read (for the bundle).
pub fn analyse_with_inputs(
    scn: &IncidentScenario,
    generated_utc: Option<&str>,
) -> Result<(Vec<LoadedInput>, ForensicReport), KshanaError> {
    analyse_with_rules(scn, generated_utc, Rules::V1)
}

/// [`analyse`] under the attribution rules of a given method version (v1 is the product's
/// method; v2 is the V2b pre-registered method).
pub fn analyse_with_method(
    scn: &IncidentScenario,
    generated_utc: Option<&str>,
    rules: Rules,
) -> Result<ForensicReport, KshanaError> {
    Ok(analyse_with_rules(scn, generated_utc, rules)?.1)
}

fn analyse_with_rules(
    scn: &IncidentScenario,
    generated_utc: Option<&str>,
    rules: Rules,
) -> Result<(Vec<LoadedInput>, ForensicReport), KshanaError> {
    let p = MethodParams::V1;
    let inc = &scn.incident;
    let bad = |m: String| KshanaError::InvalidInput(m);
    let w0 = parse_utc(&inc.start_utc).ok_or_else(|| {
        bad(format!(
            "incident.start_utc '{}' is not ISO-8601 UTC",
            inc.start_utc
        ))
    })?;
    let w1 = parse_utc(&inc.end_utc).ok_or_else(|| {
        bad(format!(
            "incident.end_utc '{}' is not ISO-8601 UTC",
            inc.end_utc
        ))
    })?;
    if w1 <= w0 {
        return Err(bad(
            "incident.end_utc must be after incident.start_utc".into()
        ));
    }
    if let Some(a) = inc.area {
        if !(a.lat_min < a.lat_max && a.lon_min < a.lon_max) {
            return Err(bad(
                "incident.area: the minimum must be below the maximum".into()
            ));
        }
    }
    if scn.receivers.is_empty() && scn.ais.is_empty() && scn.adsb.is_empty() {
        return Err(bad(
            "the incident names no data file (receiver, ais or adsb)".into(),
        ));
    }
    for s in &scn.statements {
        if parse_utc(&s.utc).is_none() {
            return Err(bad(format!(
                "statement '{}': utc is not ISO-8601 UTC",
                s.source
            )));
        }
    }
    let mut labels = BTreeSet::new();
    for l in scn
        .receivers
        .iter()
        .map(|r| &r.label)
        .chain(scn.ais.iter().map(|a| &a.label))
        .chain(scn.adsb.iter().map(|a| &a.label))
    {
        if !labels.insert(l.clone()) {
            return Err(bad(format!("input label '{l}' is used twice")));
        }
    }
    let mut inputs = load_all(scn)?;
    let find = |inputs: &[LoadedInput], role: InputRole, label: &str| {
        inputs
            .iter()
            .position(|i| i.record.role == role && i.record.label == label)
    };
    let mut findings: Vec<Finding> = Vec::new();
    let mut receivers = Vec::new();
    let mut tracks = Vec::new();
    let mut activity = Vec::new();
    let mut unaffected = Vec::new();
    let mut cal_ends: Vec<(String, Option<f64>)> = Vec::new();
    let mut untimed: Vec<String> = Vec::new();
    for r in &scn.receivers {
        let li = find(&inputs, InputRole::ReceiverLog, &r.label).unwrap_or(0);
        let nav_label = format!("{} (navigation)", r.label);
        let ni = r
            .nav
            .as_ref()
            .and_then(|_| find(&inputs, InputRole::RinexNav, &nav_label));
        let is_subject = r.role == super::ReceiverRole::Subject;
        let a = analyse_receiver(
            r,
            &inputs[li],
            ni.map(|i| &inputs[i]),
            &p,
            rules,
            w0,
            w1,
            is_subject,
        )?;
        if a.summary.start_utc.is_none() {
            untimed.push(r.label.clone());
        }
        let times = a.times.clone();
        set_coverage(&mut inputs[li].record, &times, w0, w1);
        cal_ends.push((r.label.clone(), a.calibration_end));
        findings.extend(a.findings);
        tracks.extend(a.track);
        receivers.push(a.summary);
    }
    for t in &scn.ais {
        let li = find(&inputs, InputRole::Ais, &t.label).unwrap_or(0);
        let (a, times) = analyse_ais(rules.method_id(), &inputs[li], inc, &p, w0, w1)?;
        set_coverage(&mut inputs[li].record, &times, w0, w1);
        findings.extend(a.findings);
        tracks.extend(a.track);
        activity.push(a.activity);
        unaffected.extend(a.unaffected);
    }
    for t in &scn.adsb {
        let li = find(&inputs, InputRole::Adsb, &t.label).unwrap_or(0);
        let (a, times) = analyse_adsb(rules.method_id(), &inputs[li], inc, &p, w0, w1)?;
        set_coverage(&mut inputs[li].record, &times, w0, w1);
        findings.extend(a.findings);
        tracks.extend(a.track);
        activity.push(a.activity);
        unaffected.extend(a.unaffected);
    }
    findings.sort_by(|a, b| {
        a.t_onset
            .total_cmp(&b.t_onset)
            .then(a.source.cmp(&b.source))
    });
    for (i, f) in findings.iter_mut().enumerate() {
        f.id = format!("F{}", i + 1);
    }
    corroborate(&mut findings, p.corroboration_window_s);
    for f in &mut findings {
        f.statement = describe(f);
    }
    let have_data = inputs
        .iter()
        .any(|i| i.record.window_coverage.unwrap_or(0.0) > 0.0);
    let jamming = category_verdict(&findings, "jamming", have_data);
    let spoofing = category_verdict(&findings, "spoofing", have_data);
    let say = |v: &CategoryVerdict, what: &str| match v.status {
        "indicated" => format!(
            "{what} is indicated ({} confidence; {})",
            v.confidence.unwrap_or("low"),
            v.findings.join(", ")
        ),
        "not-indicated" => format!("{what} is not indicated by the data supplied"),
        _ => format!("{what} cannot be assessed (no data in the window)"),
    };
    let sentence = format!(
        "{} {} to {}: {}; {}.",
        inc.id,
        inc.start_utc,
        inc.end_utc,
        say(&jamming, "Jamming"),
        say(&spoofing, "spoofing")
    );
    let area = affected_area(&findings, &unaffected, p.area_margin_m);

    // Timeline.
    let mut timeline = vec![
        TimelineEntry {
            utc: iso(w0),
            kind: "window",
            class: "stated",
            text: "incident window opens".into(),
            finding: None,
            t: w0,
        },
        TimelineEntry {
            utc: iso(w1),
            kind: "window",
            class: "stated",
            text: "incident window closes".into(),
            finding: None,
            t: w1,
        },
    ];
    for f in &findings {
        timeline.push(TimelineEntry {
            utc: f.onset.latest_utc.clone(),
            kind: "onset",
            class: "modelled",
            text: format!("{} begins ({}, {})", f.category, f.source, f.data_class),
            finding: Some(f.id.clone()),
            t: f.t_onset,
        });
        timeline.push(TimelineEntry {
            utc: f.end.earliest_utc.clone(),
            kind: "end",
            class: "modelled",
            text: format!("{} last seen ({})", f.category, f.source),
            finding: Some(f.id.clone()),
            t: f.t_end,
        });
    }
    for s in &scn.statements {
        let t = parse_utc(&s.utc).unwrap_or(w0);
        timeline.push(TimelineEntry {
            utc: iso(t),
            kind: "statement",
            class: "reported",
            text: format!("{}: {}", s.source, s.text),
            finding: None,
            t,
        });
    }
    timeline.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.kind.cmp(b.kind)));

    // Checklist.
    let claim = scn.variant == Variant::ClaimGrade;
    let mut checklist = Vec::new();
    let mut chk = |id, item, required_for, ok: Option<bool>, detail: String| {
        checklist.push(CheckItem {
            id,
            item,
            required_for,
            result: match ok {
                Some(true) => "pass",
                Some(false) => "fail",
                None => "pending",
            },
            detail,
        })
    };
    chk(
        "C1",
        "Every input file is hashed (SHA-256) in the report",
        "both",
        Some(true),
        format!("{} file(s)", inputs.len()),
    );
    chk(
        "C2",
        "Every receiver log states absolute time (UTC), so it can be placed in the window",
        "both",
        Some(untimed.is_empty()),
        if untimed.is_empty() {
            "all receiver logs carry UTC".into()
        } else {
            format!("no UTC in: {}", untimed.join(", "))
        },
    );
    let low_cov: Vec<String> = inputs
        .iter()
        .filter(|i| i.record.role != InputRole::RinexNav)
        .filter(|i| i.record.window_coverage.unwrap_or(0.0) < p.claim_min_coverage)
        .map(|i| {
            format!(
                "{} ({:.0} %)",
                i.record.label,
                100.0 * i.record.window_coverage.unwrap_or(0.0)
            )
        })
        .collect();
    chk(
        "C3",
        "Every input covers at least 90 % of the incident window",
        "claim-grade",
        Some(low_cov.is_empty()),
        if low_cov.is_empty() {
            "all inputs cover the window".into()
        } else {
            format!("short coverage: {}", low_cov.join(", "))
        },
    );
    let late: Vec<String> = cal_ends
        .iter()
        .filter(|(_, c)| c.is_none_or(|c| c > w0))
        .map(|(l, _)| l.clone())
        .collect();
    chk(
        "C4",
        "Each receiver's baseline (calibration window) ends before the incident window opens",
        "claim-grade",
        Some(late.is_empty()),
        if late.is_empty() {
            "every baseline precedes the window".into()
        } else {
            format!(
                "baseline overlaps the window or has no UTC: {}",
                late.join(", ")
            )
        },
    );
    let custody_missing: Vec<String> = inputs
        .iter()
        .filter(|i| {
            let base = i.record.label.trim_end_matches(" (navigation)");
            !scn.custody
                .iter()
                .any(|c| c.input == i.record.label || c.input == base)
        })
        .map(|i| i.record.label.clone())
        .collect();
    chk(
        "C5",
        "A chain-of-custody record for every input (who sent it, when, how)",
        "claim-grade",
        Some(custody_missing.is_empty()),
        if custody_missing.is_empty() {
            format!("{} record(s)", scn.custody.len())
        } else {
            format!("no record for: {}", custody_missing.join(", "))
        },
    );
    let mut mismatch = Vec::new();
    let mut unknown_custody = Vec::new();
    for c in &scn.custody {
        let hit = inputs.iter().find(|i| i.record.label == c.input);
        match (hit, &c.sender_sha256) {
            (None, _) => unknown_custody.push(c.input.clone()),
            (Some(i), Some(h)) if !h.trim().eq_ignore_ascii_case(&i.record.sha256) => {
                mismatch.push(c.input.clone())
            }
            _ => {}
        }
    }
    chk(
        "C6",
        "Hashes stated by the senders match the files received",
        "both",
        Some(mismatch.is_empty() && unknown_custody.is_empty()),
        if mismatch.is_empty() && unknown_custody.is_empty() {
            "no mismatch".into()
        } else {
            format!(
                "mismatch: {}; custody records naming no input: {}",
                if mismatch.is_empty() {
                    "none".into()
                } else {
                    mismatch.join(", ")
                },
                if unknown_custody.is_empty() {
                    "none".into()
                } else {
                    unknown_custody.join(", ")
                }
            )
        },
    );
    let sources = inputs
        .iter()
        .filter(|i| i.record.role != InputRole::RinexNav)
        .filter(|i| i.record.window_coverage.unwrap_or(0.0) > 0.0)
        .count();
    chk(
        "C7",
        "At least two independent sources with data in the window",
        "claim-grade",
        Some(sources >= 2),
        format!("{sources} source(s) with data in the window"),
    );
    let unsupported: Vec<String> = findings
        .iter()
        .filter(|f| f.category != "degradation-not-attributed" && !f.supported)
        .map(|f| f.id.clone())
        .collect();
    chk(
        "C8",
        "Every attributed finding is corroborated by an independent source (high confidence)",
        "claim-grade",
        Some(unsupported.is_empty()),
        if unsupported.is_empty() {
            "all attributed findings are corroborated".into()
        } else {
            format!(
                "indicative only (not corroborated): {}; the report states them, claim-grade does not rest on them",
                unsupported.join(", ")
            )
        },
    );
    chk(
        "C9",
        "An open-engine evidence pack for every receiver log, inside the signed bundle",
        "claim-grade",
        None,
        "written with the bundle".into(),
    );
    chk(
        "C10",
        "Expert review completed and signed by a named reviewer",
        "claim-grade",
        if claim { None } else { Some(true) },
        if claim {
            "awaiting review: `kshana-pro forensics review`".into()
        } else {
            "not required for the standard pack".into()
        },
    );
    let generated = generated_utc.map(str::to_string).unwrap_or_else(|| iso(w1));
    let mut limits = vec![
        VALIDATION_STATUS.to_string(),
        "Findings rest only on the files listed under Inputs. Data that was not supplied (other \
receivers, radar, VHF, the crew's observations) may change them."
            .to_string(),
        "Receiver-log times are as the receiver stated them; a spoofed receiver can state a false \
time, which the time-consistency monitor tests only against the log's own sequence."
            .to_string(),
        "AIS and ADS-B positions are broadcasts by the vessels and aircraft themselves (REPORTED): \
under spoofing they show the false position, and their absence can have causes other than \
interference (coverage, equipment, switch-off)."
            .to_string(),
        "The affected area is the extent of the observers that showed an effect. No emitter \
location is estimated here."
            .to_string(),
        "Confidence levels follow the stated points rule; they are not probabilities.".to_string(),
    ];
    if inc.subject != Subject::Area
        && inc.subject_id.is_none()
        && (!scn.ais.is_empty() || !scn.adsb.is_empty())
    {
        limits.push(
            "No subject_id was given, so no AIS or ADS-B track is attributed to the subject."
                .into(),
        );
    }
    let report = ForensicReport {
        format: FORMAT,
        method_id: rules.method_id(),
        variant: scn.variant,
        status: "final",
        label: LABEL,
        validation_status: VALIDATION_STATUS,
        incident: inc.clone(),
        generated_utc: generated.clone(),
        engine: EngineInfo {
            kshana_pro: crate::version().to_string(),
            open_engine: kshana::iq::campaign::runner::engine_version().to_string(),
            method_id: rules.method_id(),
            method_params_sha256: p.sha256(),
        },
        method: p,
        inputs: inputs.iter().map(|i| i.record.clone()).collect(),
        verdict: Verdict {
            jamming,
            spoofing,
            sentence,
        },
        findings,
        receivers,
        area_activity: activity,
        area,
        tracks,
        timeline,
        statements: scn.statements.clone(),
        checklist,
        expert_review: ExpertReviewFields {
            required: claim,
            status: "not-required",
            reviewer_name: None,
            qualification: None,
            organisation: None,
            independence_statement: None,
            decision: None,
            statement: None,
            reviewed_utc: None,
            how_to_complete: "The reviewer runs `kshana-pro forensics review <bundle> --reviewer <name> \
--qualification <text> --organisation <text> --independence <text> --decision \
<concur|concur-with-reservations|do-not-concur> --statement-file <file> --key-file <reviewer key>`; \
the issuer then re-signs the bundle."
                .into(),
        },
        limits,
        reproduce: Reproduce {
            recipe: "recipe.toml",
            command: format!(
                "kshana-pro forensics run recipe.toml --out <new folder> --utc {generated} --key-file <issuer key> --signer <issuer>"
            ),
            note: "Run from the bundle folder with the same kshana-pro, open engine and method \
versions: report.json, every CSV, GeoJSON and HTML file and each evidence pack's data files come \
out byte-identical. The PDF differs only in its trailer /ID."
                .into(),
        },
    };
    let mut report = report;
    settle_status(&mut report);
    Ok((inputs, report))
}

/// Set the report's status and the expert review status from its checklist. A failed item
/// required for both variants, or (claim-grade) any failed claim-grade item other than C8,
/// blocks; C8 is advisory: uncorroborated findings stay in the report, labelled indicative.
pub fn settle_status(r: &mut ForensicReport) {
    let claim = r.variant == Variant::ClaimGrade;
    let blocking = r
        .checklist
        .iter()
        .any(|c| c.result == "fail" && (c.required_for == "both" || (claim && c.id != "C8")));
    r.status = match (claim, blocking) {
        (false, false) => "final",
        (false, true) => "final-with-checklist-failures",
        (true, true) => "claim-grade-checklist-not-met",
        (true, false) => "awaiting-expert-review",
    };
    r.expert_review.status = match (claim, blocking) {
        (false, _) => "not-required",
        (true, true) => "not-eligible",
        (true, false) => "awaiting-review",
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: f64, id: &str, lat: f64, lon: f64) -> PositionReport {
        PositionReport {
            t,
            id: id.into(),
            lat,
            lon,
            sog_kn: None,
            alt_ft: None,
            nic: None,
            nacp: None,
        }
    }

    #[test]
    fn method_hash_is_stable_for_v1() {
        assert_eq!(MethodParams::V1.sha256(), MethodParams::V1.sha256());
        assert_eq!(MethodParams::V1.sha256().len(), 64);
    }

    #[test]
    fn jumps_need_distance_and_speed() {
        let v = vec![
            r(0.0, "a", 30.0, -40.0),
            r(60.0, "a", 30.001, -40.0),
            r(120.0, "a", 30.2, -40.0),
        ];
        let j = jumps(&v, 50.0, 500.0, 900.0);
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].0, 2);
    }

    #[test]
    fn a_circle_is_found_and_a_straight_line_is_not() {
        let p = MethodParams::V1;
        let circ: Vec<PositionReport> = (0..24)
            .map(|k| {
                let th = (20.0 * k as f64).to_radians();
                r(
                    k as f64 * 60.0,
                    "c",
                    30.2 + 2000.0 * th.sin() / 111_195.0,
                    -39.7 + 2000.0 * th.cos() / (111_195.0 * 30.2f64.to_radians().cos()),
                )
            })
            .collect();
        assert!(!circles(&circ, &p).is_empty());
        let line: Vec<PositionReport> = (0..24)
            .map(|k| r(k as f64 * 60.0, "l", 30.0, -40.0 + 0.01 * k as f64))
            .collect();
        assert!(circles(&line, &p).is_empty());
    }
}
