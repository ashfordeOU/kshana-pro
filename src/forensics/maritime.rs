// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Maritime incident evidence pack: actor vs victim (W1119 C3; research 02 gaps 5, 24, 34)
//!
//! Insurers, adjusters, flag states and investigators need one question answered after a
//! GNSS incident at sea: **was the vessel's GNSS attacked (the vessel is a victim), or was its
//! AIS report manipulated (the vessel, or someone using its identity, is the actor)?** The
//! answer decides which clauses apply (research 02 §5, Kpler [I4]).
//!
//! From the vessel's own NMEA 0183 log (GGA, RMC, GSV, GSA, plus HDT/VHW where the VDR export
//! carries them), the AIS tracks of the vessel and its neighbours, and optionally Kshana
//! Station events (in the unified schema, [`crate::ievents`]), this module reconstructs what
//! happened:
//!
//! * **Jamming**: C/N0 collapse against the vessel's own calibration window, satellites lost,
//!   loss of fix.
//! * **Spoofing**: a position jump (implied speed above a vessel limit), GNSS speed and course
//!   departing from the ship's own log (speed through water) and gyro (heading), C/N0 raised
//!   and uniform across satellites, false positions clustering with the neighbours'.
//! * **Ship-side fault**: the receiver reports no fix while the signals it reports are normal.
//! * **AIS manipulation**: the vessel's AIS broadcast departs from what its own NMEA says at
//!   the same time, while its NMEA is clean.
//!
//! For each GNSS episode it asks whether the vessel's broadcast track is consistent with its
//! own NMEA and whether neighbours (AIS) or a Station saw the same anomaly at the same time.
//! The pack ([`EvidencePack`], format [`FORMAT`]) carries findings labelled MEASURED (receiver
//! and ship-sensor observables as logged), DERIVED (computed here) or REPORTED (AIS broadcasts,
//! Station reports), a timeline, unified interference events, the uncertainty of every onset
//! and end, and the limits. The CLI (`kshana-pro forensics maritime`) signs the directory with
//! [`crate::signed_dossier`].
//!
//! Wording: this is an **evidence pack**, an engineering record of the data supplied under
//! thresholds fixed before the run ([`Params`], written into every pack). It is not a legal
//! opinion or a finding of fact, and its admissibility is for the lawyers.
//!
//! Evidence: **MODELLED**. The rules are exercised on synthetic scenarios with known truth
//! ([`synth`], tests in `tests/maritime_evidence.rs`); they have not been validated against a
//! labelled set of real incidents.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::haversine_m;
use kshana::receiver_trust::ingest::read_nmea;
use serde::{Deserialize, Serialize};

use super::inputs::{iso, parse_tracks, parse_utc, round, sha256_hex, PositionReport};
use crate::ievents;

/// The pack's format tag.
pub const FORMAT: &str = "kshana-pro-maritime-evidence-pack/1";
/// The method; every threshold is a field of [`Params`] and is written into the pack.
pub const METHOD_ID: &str = "kshana-pro/maritime-actor-victim/v1";
/// The pack's main file.
pub const RESULT_FILE: &str = "evidence-pack.json";
/// Evidence label of the method.
pub const EVIDENCE: &str = "MODELLED: rules checked on synthetic scenarios with known truth; not \
validated against a labelled set of real incidents.";
/// What every pack says about itself.
pub const STATEMENT: &str = "An evidence pack: an engineering reconstruction of the data supplied, \
under thresholds fixed before the run. MEASURED = receiver and ship-sensor observables as logged; \
DERIVED = computed by this method; REPORTED = AIS broadcasts and third-party or Station reports. It \
is not a legal opinion or a finding of fact, and it says nothing about intent.";

const KN: f64 = 1852.0 / 3600.0; // m/s per knot

/// Pre-registered thresholds of the method.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Calibration window at the start of the log (s): the baseline C/N0.
    pub calib_s: f64,
    /// C/N0 drop (dB) against the baseline that marks jamming.
    pub cn0_drop_db: f64,
    /// C/N0 rise (dB) that, together with a uniform C/N0, marks spoofing.
    pub cn0_rise_db: f64,
    /// Spread (std, dB) of C/N0 across satellites below which C/N0 is "uniform".
    pub uniform_std_db: f64,
    /// Fastest credible vessel speed (kn): an implied speed above it is a jump.
    pub max_speed_kn: f64,
    /// Largest credible |GNSS SOG − speed through water| (kn), allowing for current.
    pub sog_stw_kn: f64,
    /// Largest credible |GNSS COG − gyro heading| (deg), allowing for leeway and set.
    pub cog_hdg_deg: f64,
    /// Course is compared only above this speed (kn).
    pub min_sog_for_course_kn: f64,
    /// Flagged epochs closer than this (s) form one episode.
    pub merge_gap_s: f64,
    /// Shortest episode kept (s), unless it holds a position jump.
    pub min_episode_s: f64,
    /// AIS and own NMEA agree when their positions at the same time are within this (m).
    pub ais_nmea_tol_m: f64,
    /// Neighbours are AIS identities within this distance (m) of the vessel before an episode.
    pub neighbour_radius_m: f64,
    /// Neighbours needed to call an episode localised when none saw it.
    pub min_neighbours: usize,
    /// Time margin (s) around an episode when looking for the same anomaly elsewhere.
    pub corroboration_margin_s: f64,
    /// Positions within this (m) of the vessel's false position count as clustered.
    pub cluster_m: f64,
    /// Illustrative user range error (m) for the position uncertainty (HDOP × UERE).
    pub uere_m: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            calib_s: 120.0,
            cn0_drop_db: 8.0,
            cn0_rise_db: 4.0,
            uniform_std_db: 1.5,
            max_speed_kn: 40.0,
            sog_stw_kn: 4.0,
            cog_hdg_deg: 30.0,
            min_sog_for_course_kn: 3.0,
            merge_gap_s: 30.0,
            min_episode_s: 5.0,
            ais_nmea_tol_m: 300.0,
            neighbour_radius_m: 30_000.0,
            min_neighbours: 2,
            corroboration_margin_s: 120.0,
            cluster_m: 500.0,
            uere_m: 5.0,
        }
    }
}

/// A request file (`kshana-pro forensics maritime <request.toml>`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaritimeRequest {
    pub title: String,
    /// The vessel's identifier as the AIS CSV names it (`vessel_id` column, e.g. its MMSI).
    pub vessel_id: String,
    /// The vessel's own NMEA 0183 log (a VDR export or receiver log).
    pub nmea: String,
    /// AIS CSV (`timestamp,vessel_id,lat,lon[,sog_kn]`) of the vessel and its neighbours.
    #[serde(default)]
    pub ais: Option<String>,
    /// Kshana Station events: a JSON array of unified interference events.
    #[serde(default)]
    pub station_events: Option<String>,
    #[serde(default)]
    pub params: Params,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl MaritimeRequest {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r: Self = toml::from_str(&src).map_err(|e| format!("invalid request: {e}"))?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok(r)
    }

    fn path(&self, p: &str) -> PathBuf {
        match &self.base_dir {
            Some(d) if Path::new(p).is_relative() => d.join(p),
            _ => PathBuf::from(p),
        }
    }

    /// Read every input named by the request.
    pub fn load(&self) -> Result<MaritimeInputs, String> {
        let read = |p: &str| {
            std::fs::read(self.path(p)).map_err(|e| format!("{}: {e}", self.path(p).display()))
        };
        let name = |p: &str| {
            Path::new(p)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string())
        };
        let nmea = read(&self.nmea)?;
        let mut files = vec![(name(&self.nmea), nmea.clone())];
        let ais = match &self.ais {
            Some(p) => {
                let b = read(p)?;
                files.push((name(p), b.clone()));
                Some(String::from_utf8_lossy(&b).to_string())
            }
            None => None,
        };
        let station = match &self.station_events {
            Some(p) => {
                let b = read(p)?;
                files.push((name(p), b.clone()));
                serde_json::from_slice::<Vec<ievents::Event>>(&b)
                    .map_err(|e| format!("{p}: station events: {e}"))?
            }
            None => vec![],
        };
        Ok(MaritimeInputs {
            title: self.title.clone(),
            vessel_id: self.vessel_id.clone(),
            nmea: String::from_utf8_lossy(&nmea).to_string(),
            ais,
            station,
            files,
            params: self.params.clone(),
        })
    }
}

/// Inputs in memory.
#[derive(Clone, Debug)]
pub struct MaritimeInputs {
    pub title: String,
    pub vessel_id: String,
    pub nmea: String,
    pub ais: Option<String>,
    pub station: Vec<ievents::Event>,
    /// (file name, bytes) of every input, hashed into the pack and copied with it.
    pub files: Vec<(String, Vec<u8>)>,
    pub params: Params,
}

/// Evidence class of a finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Label {
    Measured,
    Derived,
    Reported,
}

/// What an episode was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Jamming,
    Spoofing,
    ShipSideFault,
    AisManipulation,
}

/// The actor-vs-victim conclusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    /// No episode found in the data.
    NoEvent,
    /// The vessel's GNSS was interfered with, its AIS matched its own NMEA, and neighbours or
    /// a Station saw the same anomaly: consistent with the vessel being a victim.
    VictimOfExternalInterference,
    /// The vessel's GNSS shows interference that enough neighbours did not see: localised to
    /// the vessel (an on-board or close-range source, or equipment); actor or victim open.
    LocalisedToVessel,
    /// The vessel's AIS broadcast departs from its own clean NMEA: consistent with
    /// manipulation of the AIS report (by whom the data cannot say).
    AisManipulation,
    /// Loss of fix with normal signals: a receiver or installation fault, not interference.
    ShipSideFault,
    /// Not enough independent data to decide.
    Inconclusive,
}

/// A yes / no / open answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Indicated,
    NotIndicated,
    Undetermined,
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub label: Label,
    /// Episode number, or `None` for the whole record.
    pub episode: Option<usize>,
    pub source: String,
    pub text: String,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
}

/// A time bounded by the data: the event happened between `earliest` and `latest`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bound {
    pub earliest_utc: String,
    pub latest_utc: String,
    pub uncertainty_s: f64,
}

/// One episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub n: usize,
    pub event_type: EventType,
    pub start_utc: String,
    pub end_utc: String,
    pub onset: Bound,
    pub end: Bound,
    pub duration_s: f64,
    /// Last clean position before the episode (the vessel's position as its own GNSS gave it).
    pub last_clean_position: Option<[f64; 2]>,
    /// Its 95 % horizontal uncertainty (m): 2 × HDOP × UERE (illustrative UERE).
    pub position_uncertainty_m: Option<f64>,
    pub metrics: BTreeMap<String, f64>,
    /// `consistent`, `inconsistent`, `silent` (no AIS report in the episode), `no_ais`.
    pub ais_vs_own_nmea: String,
    pub neighbours_present: Vec<String>,
    pub neighbours_corroborating: Vec<String>,
    pub station_corroborates: bool,
    pub conclusion: Conclusion,
    /// Rule-based confidence (0..1), see [`CONFIDENCE_RULE`].
    pub confidence: f64,
}

/// The confidence rule written into every pack.
pub const CONFIDENCE_RULE: &str = "Independent indicators combine as 1 − Π(1 − cᵢ): C/N0 \
anomaly 0.5, kinematic inconsistency against the ship's log or gyro 0.5, position jump 0.4, loss \
of fix 0.3, neighbour corroboration 0.6, Station corroboration 0.6, AIS-vs-NMEA discrepancy 0.7. \
A ranking aid stated before the run, not a calibrated probability.";

/// The overall answer for the insurer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActorVictim {
    pub gnss_interference: Answer,
    pub ais_manipulation: Answer,
    pub conclusion: Conclusion,
    pub rationale: Vec<String>,
}

/// One timeline row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub utc: String,
    pub source: String,
    pub label: Label,
    pub text: String,
    #[serde(skip)]
    t: f64,
}

/// An input file's record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputFile {
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
}

/// The evidence pack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvidencePack {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub statement: String,
    pub title: String,
    pub vessel_id: String,
    pub params: Params,
    pub inputs: Vec<InputFile>,
    pub record: RecordSummary,
    pub verdict: ActorVictim,
    pub episodes: Vec<Episode>,
    pub findings: Vec<Finding>,
    pub timeline: Vec<TimelineEntry>,
    pub events: Vec<ievents::Event>,
    pub confidence_rule: String,
    pub uncertainty: Vec<String>,
    pub limits: Vec<String>,
}

/// What the NMEA record holds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordSummary {
    pub nmea_epochs: usize,
    pub nmea_skipped_records: usize,
    pub nmea_start_utc: Option<String>,
    pub nmea_end_utc: Option<String>,
    pub baseline_cn0_dbhz: Option<f64>,
    pub baseline_cn0_spread_db: Option<f64>,
    pub has_log_speed: bool,
    pub has_heading: bool,
    pub ais_identities: usize,
    pub ais_reports_of_vessel: usize,
    pub station_events: usize,
}

// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Epoch {
    t: f64,
    fix: Option<(f64, f64)>,
    hdop: Option<f64>,
    n_trk: usize,
    cn0_mean: Option<f64>,
    cn0_std: Option<f64>,
    sog: Option<f64>,
    cog: Option<f64>,
    hdg: Option<f64>,
    stw: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct Flags {
    jam: bool,
    nofix: bool,
    fault: bool,
    jump: Option<(f64, f64)>, // (distance m, implied speed kn)
    kin: Option<f64>,         // |SOG−STW| kn or course error deg (largest normalised)
    cn0_spoof: bool,
}

impl Flags {
    fn any(&self) -> bool {
        self.jam
            || self.nofix
            || self.fault
            || self.jump.is_some()
            || self.kin.is_some()
            || self.cn0_spoof
    }
    fn spoof(&self) -> bool {
        self.jump.is_some() || self.kin.is_some() || self.cn0_spoof
    }
}

fn mean_std(v: &[f64]) -> Option<(f64, f64)> {
    if v.is_empty() {
        return None;
    }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    let s = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
    Some((m, s))
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn ang_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn iso_s(t: f64) -> String {
    iso(t.floor())
}

fn epochs_from_nmea(text: &str) -> Result<(Vec<Epoch>, usize), String> {
    let tl = read_nmea(text)?;
    let mut out = Vec::with_capacity(tl.epochs.len());
    for e in &tl.epochs {
        let Some(t) = e.time_label.as_deref().and_then(parse_utc) else {
            continue;
        };
        let valid = e
            .marine
            .as_ref()
            .and_then(|m| m.fix_valid)
            .unwrap_or(e.fix.is_some());
        let cn: Vec<f64> = e
            .cn0
            .iter()
            .map(|s| s.cn0_dbhz)
            .filter(|c| *c > 0.0)
            .collect();
        let ms = mean_std(&cn);
        let m = e.marine.clone().unwrap_or_default();
        out.push(Epoch {
            t,
            fix: if valid {
                e.fix.map(|f| (f.lat_deg, f.lon_deg))
            } else {
                None
            },
            hdop: m.hdop,
            n_trk: cn.len(),
            cn0_mean: ms.map(|x| x.0),
            cn0_std: ms.map(|x| x.1),
            sog: m.sog_kn,
            cog: m.cog_deg,
            hdg: m.heading_deg,
            stw: m.stw_kn,
        });
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((out, tl.skipped_records))
}

fn flag_epochs(ep: &[Epoch], p: &Params, base: Option<(f64, f64)>, base_trk: f64) -> Vec<Flags> {
    let mut out = vec![Flags::default(); ep.len()];
    let mut last_fix: Option<(f64, f64, f64)> = None;
    for (i, e) in ep.iter().enumerate() {
        let f = &mut out[i];
        let drop = match (base, e.cn0_mean) {
            (Some((b, _)), Some(m)) => b - m,
            (Some(_), None) => f64::INFINITY,
            _ => 0.0,
        };
        f.nofix = e.fix.is_none();
        f.jam = drop >= p.cn0_drop_db
            || (f.nofix && (drop >= p.cn0_drop_db * 0.75 || (e.n_trk as f64) < 4.0));
        // No fix while the receiver tracks as many satellites as usual at the usual C/N0.
        f.fault = f.nofix && !f.jam && (e.n_trk as f64) >= (base_trk * 0.75).max(4.0) && drop < 3.0;
        if let (Some((b, bs)), Some(m), Some(s)) = (base, e.cn0_mean, e.cn0_std) {
            f.cn0_spoof =
                e.n_trk >= 5 && m - b >= p.cn0_rise_db && s < p.uniform_std_db && bs > 2.0 * s;
        }
        if let Some((lat, lon)) = e.fix {
            if let Some((t0, la0, lo0)) = last_fix {
                let d = haversine_m(la0, lo0, lat, lon);
                let dt = (e.t - t0).max(1e-3);
                let v_kn = d / dt / KN;
                if v_kn > p.max_speed_kn && d > 50.0 {
                    f.jump = Some((d, v_kn));
                }
            }
            last_fix = Some((e.t, lat, lon));
            // GNSS speed and course against the ship's own log and gyro.
            let mut k: Option<f64> = None;
            if let (Some(sog), Some(stw)) = (e.sog, e.stw) {
                let d = (sog - stw).abs();
                if d > p.sog_stw_kn {
                    k = Some(d);
                }
            }
            if let (Some(sog), Some(stw), Some(cog), Some(hdg)) = (e.sog, e.stw, e.cog, e.hdg) {
                if sog.max(stw) >= p.min_sog_for_course_kn && sog >= p.min_sog_for_course_kn {
                    let d = ang_diff(cog, hdg);
                    if d > p.cog_hdg_deg {
                        k = Some(k.unwrap_or(0.0).max(d / p.cog_hdg_deg * p.sog_stw_kn));
                    }
                }
            }
            f.kin = k;
        }
    }
    out
}

struct RawEpisode {
    i0: usize,
    i1: usize,
}

fn group(ep: &[Epoch], fl: &[Flags], p: &Params, calib_end: f64) -> Vec<RawEpisode> {
    let mut eps: Vec<RawEpisode> = vec![];
    for (i, f) in fl.iter().enumerate() {
        if !f.any() || ep[i].t < calib_end {
            continue;
        }
        match eps.last_mut() {
            Some(r) if ep[i].t - ep[r.i1].t <= p.merge_gap_s => r.i1 = i,
            _ => eps.push(RawEpisode { i0: i, i1: i }),
        }
    }
    eps.retain(|r| {
        ep[r.i1].t - ep[r.i0].t + 1.0 >= p.min_episode_s
            || (r.i0..=r.i1).any(|i| fl[i].jump.is_some())
    });
    eps
}

struct Ais {
    tracks: BTreeMap<String, Vec<PositionReport>>,
}

impl Ais {
    fn median_interval(v: &[PositionReport]) -> f64 {
        median(v.windows(2).map(|w| w[1].t - w[0].t).collect()).unwrap_or(10.0)
    }
}

/// Anomalies of one AIS track in [a, b]: jumps, gaps, and reports near `cluster`.
fn track_anomaly(
    v: &[PositionReport],
    a: f64,
    b: f64,
    p: &Params,
    cluster: Option<(f64, f64)>,
) -> Option<String> {
    let mi = Ais::median_interval(v);
    for w in v.windows(2) {
        let (r0, r1) = (&w[0], &w[1]);
        if r1.t < a || r0.t > b {
            continue;
        }
        let d = haversine_m(r0.lat, r0.lon, r1.lat, r1.lon);
        let dt = (r1.t - r0.t).max(1e-3);
        if d / dt / KN > p.max_speed_kn && d > 200.0 {
            return Some(format!(
                "position jump of {:.0} m in {:.0} s at {}",
                d,
                dt,
                iso_s(r1.t)
            ));
        }
        if dt > (3.0 * mi).max(60.0) {
            return Some(format!("no report for {:.0} s from {}", dt, iso_s(r0.t)));
        }
    }
    // A track that ends inside the window (no report after it) is a gap too.
    if let Some(last) = v.last() {
        if last.t >= a - mi && last.t < b - (3.0 * mi).max(60.0) {
            return Some(format!("no report after {}", iso_s(last.t)));
        }
    }
    if let Some((cl, co)) = cluster {
        let n = v
            .iter()
            .filter(|r| r.t >= a && r.t <= b && haversine_m(r.lat, r.lon, cl, co) <= p.cluster_m)
            .count();
        if n >= 2 {
            return Some(format!(
                "{n} reports within {:.0} m of the vessel's false position",
                p.cluster_m
            ));
        }
    }
    None
}

fn nearest_epoch(ep: &[Epoch], t: f64, tol: f64) -> Option<&Epoch> {
    let i = ep.partition_point(|e| e.t < t);
    [i.checked_sub(1), Some(i)]
        .into_iter()
        .flatten()
        .filter_map(|k| ep.get(k))
        .filter(|e| (e.t - t).abs() <= tol)
        .min_by(|a, b| (a.t - t).abs().total_cmp(&(b.t - t).abs()))
}

fn combine(cs: &[f64]) -> f64 {
    1.0 - cs.iter().fold(1.0, |acc, c| acc * (1.0 - c))
}

/// Analyse the inputs and build the (unsigned) evidence pack.
pub fn analyse(inp: &MaritimeInputs) -> Result<EvidencePack, String> {
    let p = &inp.params;
    let (ep, skipped) = epochs_from_nmea(&inp.nmea)?;
    if ep.len() < 10 {
        return Err(format!(
            "the NMEA log has {} timed epochs with a UTC date (RMC); at least 10 are needed",
            ep.len()
        ));
    }
    let t_first = ep[0].t;
    let calib_end = t_first + p.calib_s;
    let cal: Vec<&Epoch> = ep.iter().filter(|e| e.t < calib_end).collect();
    let base = {
        let m: Vec<f64> = cal.iter().filter_map(|e| e.cn0_mean).collect();
        let s: Vec<f64> = cal.iter().filter_map(|e| e.cn0_std).collect();
        match (median(m), median(s)) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    };
    let base_trk = median(cal.iter().map(|e| e.n_trk as f64).collect()).unwrap_or(0.0);
    let fl = flag_epochs(&ep, p, base, base_trk);
    let raw = group(&ep, &fl, p, calib_end);

    let ais = match &inp.ais {
        Some(t) => Some(Ais {
            tracks: parse_tracks(t, "vessel_id")
                .map_err(|e| e.to_string())?
                .by_id,
        }),
        None => None,
    };
    let own_ais: &[PositionReport] = ais
        .as_ref()
        .and_then(|a| a.tracks.get(&inp.vessel_id))
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let evidence_refs: Vec<ievents::EvidenceRef> = inp
        .files
        .iter()
        .map(|(n, b)| ievents::EvidenceRef {
            file: format!("inputs/{n}"),
            sha256: sha256_hex(b),
        })
        .collect();

    let mut findings: Vec<Finding> = vec![];
    let mut timeline: Vec<TimelineEntry> = vec![];
    let mut episodes: Vec<Episode> = vec![];
    let mut events: Vec<ievents::Event> = vec![];
    let tl = |t: f64, source: &str, label: Label, text: String| TimelineEntry {
        utc: iso_s(t),
        source: source.into(),
        label,
        text,
        t,
    };
    if let Some((b, s)) = base {
        findings.push(Finding {
            label: Label::Measured,
            episode: None,
            source: "own NMEA (GSV)".into(),
            text: format!(
                "Calibration window ({:.0} s from the first epoch): median C/N0 {b:.1} dB-Hz across {base_trk:.0} satellites, spread {s:.1} dB",
                p.calib_s
            ),
            value: Some(round(b, 2)),
            unit: Some("dB-Hz".into()),
        });
    }

    // ---- GNSS episodes --------------------------------------------------------------------
    for (k, r) in raw.iter().enumerate() {
        let n = k + 1;
        let span = &ep[r.i0..=r.i1];
        let sf = &fl[r.i0..=r.i1];
        let flagged = sf.iter().filter(|f| f.any()).count().max(1);
        let spoof_n = sf.iter().filter(|f| f.spoof()).count();
        let jam_n = sf.iter().filter(|f| f.jam).count();
        let fault_n = sf.iter().filter(|f| f.fault).count();
        let nofix_n = sf.iter().filter(|f| f.nofix).count();
        let jumps: Vec<(f64, f64)> = sf.iter().filter_map(|f| f.jump).collect();
        let kin_n = sf.iter().filter(|f| f.kin.is_some()).count();
        let cn0s_n = sf.iter().filter(|f| f.cn0_spoof).count();
        let et = if !jumps.is_empty() || spoof_n * 5 >= flagged {
            EventType::Spoofing
        } else if fault_n > jam_n {
            EventType::ShipSideFault
        } else {
            EventType::Jamming
        };
        let (t0, t1) = (span[0].t, span[span.len() - 1].t);
        let before = ep[..r.i0].iter().rev().find(|e| e.fix.is_some());
        let prev_t = r.i0.checked_sub(1).map(|i| ep[i].t).unwrap_or(t0);
        let next_t = ep.get(r.i1 + 1).map(|e| e.t).unwrap_or(t1);
        let ref_pos = before.and_then(|e| e.fix);
        let mut metrics = BTreeMap::new();
        let src_nmea = "own NMEA";
        let mut cs: Vec<f64> = vec![];
        if let Some((b, _)) = base {
            let min_cn0 = span
                .iter()
                .filter_map(|e| e.cn0_mean)
                .fold(f64::INFINITY, f64::min);
            let max_cn0 = span
                .iter()
                .filter_map(|e| e.cn0_mean)
                .fold(f64::NEG_INFINITY, f64::max);
            if jam_n > 0 {
                let drop = if min_cn0.is_finite() { b - min_cn0 } else { b };
                metrics.insert("cn0_drop_db".into(), round(drop, 2));
                findings.push(Finding {
                    label: Label::Measured,
                    episode: Some(n),
                    source: "own NMEA (GSV)".into(),
                    text: format!(
                        "C/N0 fell by up to {drop:.1} dB below the calibration median ({jam_n} epochs at or beyond the {:.0} dB bar){}",
                        p.cn0_drop_db,
                        if min_cn0.is_finite() { String::new() } else { "; no satellite reported a signal at the worst epoch".into() }
                    ),
                    value: Some(round(drop, 2)),
                    unit: Some("dB".into()),
                });
                cs.push(0.5);
            }
            if cn0s_n > 0 {
                let min_s = span
                    .iter()
                    .filter_map(|e| e.cn0_std)
                    .fold(f64::INFINITY, f64::min);
                metrics.insert("cn0_rise_db".into(), round(max_cn0 - b, 2));
                metrics.insert("cn0_spread_db".into(), round(min_s, 2));
                findings.push(Finding {
                    label: Label::Measured,
                    episode: Some(n),
                    source: "own NMEA (GSV)".into(),
                    text: format!(
                        "C/N0 rose {:.1} dB above the calibration median and became uniform across satellites (spread {min_s:.2} dB against {:.1} dB in calibration) for {cn0s_n} epochs: the signature of signals from one transmitter",
                        max_cn0 - b,
                        base.map(|x| x.1).unwrap_or(0.0)
                    ),
                    value: Some(round(min_s, 2)),
                    unit: Some("dB".into()),
                });
                cs.push(0.5);
            }
        }
        if nofix_n > 0 {
            metrics.insert("no_fix_s".into(), nofix_n as f64);
            findings.push(Finding {
                label: Label::Measured,
                episode: Some(n),
                source: "own NMEA (GGA/RMC)".into(),
                text: format!("The receiver reported no valid fix for {nofix_n} epochs"),
                value: Some(nofix_n as f64),
                unit: Some("epochs".into()),
            });
            if et != EventType::ShipSideFault {
                cs.push(0.3);
            }
        }
        if et == EventType::ShipSideFault {
            let trk = median(span.iter().map(|e| e.n_trk as f64).collect()).unwrap_or(0.0);
            findings.push(Finding {
                label: Label::Derived,
                episode: Some(n),
                source: src_nmea.into(),
                text: format!(
                    "No fix while the receiver tracked a median {trk:.0} satellites (calibration {base_trk:.0}) at normal C/N0: points to a receiver, antenna-installation or configuration fault rather than RF interference"
                ),
                value: Some(trk),
                unit: Some("satellites".into()),
            });
            cs.push(0.5);
        }
        if !jumps.is_empty() {
            let (d, v) = jumps
                .iter()
                .cloned()
                .fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
            metrics.insert("position_jump_m".into(), round(d, 1));
            metrics.insert("implied_speed_kn".into(), round(v, 1));
            findings.push(Finding {
                label: Label::Derived,
                episode: Some(n),
                source: src_nmea.into(),
                text: format!(
                    "Position jump of {d:.0} m between consecutive fixes (implied {v:.0} kn against a {:.0} kn vessel limit)",
                    p.max_speed_kn
                ),
                value: Some(round(d, 1)),
                unit: Some("m".into()),
            });
            cs.push(0.4);
        }
        if kin_n > 0 {
            let worst_sog = span
                .iter()
                .filter_map(|e| Some((e.sog? - e.stw?).abs()))
                .fold(0.0, f64::max);
            let worst_cog = span
                .iter()
                .filter(|e| e.sog.unwrap_or(0.0) >= p.min_sog_for_course_kn)
                .filter_map(|e| Some(ang_diff(e.cog?, e.hdg?)))
                .fold(0.0, f64::max);
            metrics.insert("sog_minus_stw_kn".into(), round(worst_sog, 2));
            metrics.insert("cog_minus_heading_deg".into(), round(worst_cog, 1));
            findings.push(Finding {
                label: Label::Measured,
                episode: Some(n),
                source: "own NMEA (RMC vs VHW/HDT)".into(),
                text: format!(
                    "GNSS motion departed from the ship's own sensors for {kin_n} epochs: |SOG − speed through water| up to {worst_sog:.1} kn (bar {:.1}), |COG − gyro heading| up to {worst_cog:.0}° (bar {:.0}°)",
                    p.sog_stw_kn, p.cog_hdg_deg
                ),
                value: Some(round(worst_sog, 2)),
                unit: Some("kn".into()),
            });
            cs.push(0.5);
        }
        // The vessel's false position (spoofing): the centroid of fixes in the episode that
        // are far from where the vessel was.
        let false_pos = if et == EventType::Spoofing {
            ref_pos.and_then(|(la, lo)| {
                let far: Vec<(f64, f64)> = span
                    .iter()
                    .filter_map(|e| e.fix)
                    .filter(|(a, b)| haversine_m(la, lo, *a, *b) > 2.0 * p.cluster_m)
                    .collect();
                let (cla, clo) = (
                    median(far.iter().map(|x| x.0).collect())?,
                    median(far.iter().map(|x| x.1).collect())?,
                );
                Some((cla, clo))
            })
        } else {
            None
        };
        if let (Some((la, lo)), Some((fa, fo))) = (ref_pos, false_pos) {
            let d = haversine_m(la, lo, fa, fo);
            metrics.insert("false_position_offset_m".into(), round(d, 0));
            findings.push(Finding {
                label: Label::Derived,
                episode: Some(n),
                source: src_nmea.into(),
                text: format!(
                    "The receiver's positions during the episode centred on {fa:.4}, {fo:.4}, {:.1} km from the last clean fix",
                    d / 1000.0
                ),
                value: Some(round(d, 0)),
                unit: Some("m".into()),
            });
        }

        // ---- AIS of the vessel against its own NMEA in the episode ----
        let win = (t0, t1);
        let ais_vs = if ais.is_none() || own_ais.is_empty() {
            "no_ais".to_string()
        } else {
            let in_ep: Vec<&PositionReport> = own_ais
                .iter()
                .filter(|r| r.t >= win.0 && r.t <= win.1)
                .collect();
            let cmp: Vec<f64> = in_ep
                .iter()
                .filter_map(|r| {
                    let e = nearest_epoch(&ep, r.t, 2.0)?;
                    let (a, b) = e.fix?;
                    Some(haversine_m(a, b, r.lat, r.lon))
                })
                .collect();
            if in_ep.is_empty() {
                "silent".into()
            } else if cmp.len() * 2 < in_ep.len() {
                // AIS reported while own GNSS had no fix: an internal AIS GNSS or dead reckoning.
                "no_own_fix".into()
            } else if cmp.iter().filter(|d| **d <= p.ais_nmea_tol_m).count() * 5 >= cmp.len() * 4 {
                "consistent".into()
            } else {
                "inconsistent".into()
            }
        };
        let ais_text = match ais_vs.as_str() {
            "consistent" => "The vessel's AIS broadcast matched its own NMEA positions during the episode (anomalous positions included): the transponder sent what the vessel's GNSS gave it",
            "inconsistent" => "The vessel's AIS broadcast departed from its own NMEA positions during the episode",
            "silent" => "The vessel's AIS sent no position report during the episode, consistent with its loss of fix",
            "no_own_fix" => "The vessel's AIS kept reporting while its own receiver had no fix (an internal AIS GNSS or dead reckoning); the two cannot be compared",
            _ => "No AIS of the vessel was supplied; its broadcast track could not be compared",
        };
        findings.push(Finding {
            label: if ais_vs == "no_ais" {
                Label::Derived
            } else {
                Label::Reported
            },
            episode: Some(n),
            source: "AIS vs own NMEA".into(),
            text: ais_text.into(),
            value: None,
            unit: None,
        });

        // ---- neighbours ----
        let m = p.corroboration_margin_s;
        let (mut present, mut corr) = (vec![], vec![]);
        if let (Some(a), Some((la, lo))) = (&ais, ref_pos) {
            for (id, v) in &a.tracks {
                if id == &inp.vessel_id {
                    continue;
                }
                let near = v.iter().any(|r| {
                    r.t >= t0 - m - 600.0
                        && r.t <= t0
                        && haversine_m(la, lo, r.lat, r.lon) <= p.neighbour_radius_m
                });
                if !near {
                    continue;
                }
                present.push(id.clone());
                if let Some(why) = track_anomaly(v, t0 - m, t1 + m, p, false_pos) {
                    let first = v.iter().find(|r| r.t >= t0 - m).map(|r| r.t).unwrap_or(t0);
                    timeline.push(tl(
                        first,
                        &format!("AIS {id}"),
                        Label::Reported,
                        why.clone(),
                    ));
                    findings.push(Finding {
                        label: Label::Reported,
                        episode: Some(n),
                        source: format!("AIS {id}"),
                        text: format!("Neighbour {id}: {why}"),
                        value: None,
                        unit: None,
                    });
                    corr.push(id.clone());
                }
            }
        }
        let station = ref_pos.is_some_and(|(la, lo)| {
            inp.station.iter().any(|s| {
                let (Some(a), Some(b)) = (parse_utc(&s.start_utc), parse_utc(&s.end_utc)) else {
                    return false;
                };
                let near = s.location.as_ref().is_some_and(|pl| {
                    haversine_m(la, lo, pl.lat_deg, pl.lon_deg)
                        <= pl.radius_m + p.neighbour_radius_m
                });
                a <= t1 + m && b >= t0 - m && near
            })
        });
        if station {
            findings.push(Finding {
                label: Label::Reported,
                episode: Some(n),
                source: "Kshana Station".into(),
                text: "A Station event overlaps the episode in time and place".into(),
                value: None,
                unit: None,
            });
            cs.push(0.6);
        }
        findings.push(Finding {
            label: Label::Derived,
            episode: Some(n),
            source: "AIS neighbours".into(),
            text: format!(
                "{} of {} AIS neighbours within {:.0} km showed an anomaly within ±{:.0} s of the episode",
                corr.len(),
                present.len(),
                p.neighbour_radius_m / 1000.0,
                m
            ),
            value: Some(corr.len() as f64),
            unit: Some("neighbours".into()),
        });
        if !corr.is_empty() {
            cs.push(0.6);
        }
        let corroborated = station || (!corr.is_empty() && corr.len() * 2 >= present.len());
        let conclusion = if et == EventType::ShipSideFault {
            // A fault with normal signals stays a fault even when neighbours saw something:
            // the vessel's receiver tracked normal signals, so their anomaly is not its cause.
            Conclusion::ShipSideFault
        } else if ais_vs == "inconsistent" {
            Conclusion::AisManipulation
        } else if corroborated {
            Conclusion::VictimOfExternalInterference
        } else if present.len() >= p.min_neighbours && corr.is_empty() {
            Conclusion::LocalisedToVessel
        } else {
            Conclusion::Inconclusive
        };
        let onset = Bound {
            earliest_utc: iso_s(prev_t),
            latest_utc: iso_s(t0),
            uncertainty_s: round(t0 - prev_t, 3),
        };
        let end = Bound {
            earliest_utc: iso_s(t1),
            latest_utc: iso_s(next_t),
            uncertainty_s: round(next_t - t1, 3),
        };
        let pos_unc = before.map(|e| round(2.0 * e.hdop.unwrap_or(1.0) * p.uere_m, 1));
        let label_txt = match et {
            EventType::Jamming => "jamming",
            EventType::Spoofing => "spoofing",
            EventType::ShipSideFault => "ship-side fault",
            EventType::AisManipulation => "AIS manipulation",
        };
        timeline.push(tl(
            t0,
            src_nmea,
            Label::Derived,
            format!("Episode {n} ({label_txt}) begins"),
        ));
        timeline.push(tl(
            t1,
            src_nmea,
            Label::Derived,
            format!("Episode {n} ({label_txt}) ends"),
        ));
        let confidence = round(combine(&cs), 3);
        let mut obs = vec![format!("vessel:{}", inp.vessel_id)];
        obs.extend(corr.iter().map(|c| format!("ais:{c}")));
        if let Some((la, lo)) = ref_pos {
            events.push(ievents::Event {
                format: ievents::FORMAT.into(),
                id: format!("maritime-{}-{n}", inp.vessel_id),
                start_utc: iso_s(t0),
                end_utc: iso_s(t1.ceil()),
                systems: vec![ievents::System::Gnss { band: "L1".into() }],
                kind: match et {
                    EventType::Jamming => ievents::Kind::Jamming,
                    EventType::Spoofing => ievents::Kind::Spoofing,
                    _ => ievents::Kind::Outage,
                },
                location: Some(ievents::Place {
                    lat_deg: la,
                    lon_deg: lo,
                    radius_m: pos_unc.unwrap_or(10.0).max(10.0),
                }),
                area: None,
                observers: obs,
                metrics: metrics.clone(),
                source_estimate: None,
                confidence,
                basis: METHOD_ID.into(),
                evidence: evidence_refs.clone(),
                reporter: "kshana-pro forensics maritime".into(),
                corroborated,
            });
        }
        episodes.push(Episode {
            n,
            event_type: et,
            start_utc: iso_s(t0),
            end_utc: iso_s(t1),
            onset,
            end,
            duration_s: round(t1 - t0 + 1.0, 1),
            last_clean_position: ref_pos.map(|(a, b)| [round(a, 6), round(b, 6)]),
            position_uncertainty_m: pos_unc,
            metrics,
            ais_vs_own_nmea: ais_vs,
            neighbours_present: present,
            neighbours_corroborating: corr,
            station_corroborates: station,
            conclusion,
            confidence,
        });
    }

    // ---- AIS episodes: the vessel's AIS departs from its clean NMEA ----------------------
    let mut disc: Vec<(f64, f64, f64, f64)> = vec![]; // (t, distance, lat, lon)
    for r in own_ais {
        if let Some(e) = nearest_epoch(&ep, r.t, 2.0) {
            if let Some((a, b)) = e.fix {
                let d = haversine_m(a, b, r.lat, r.lon);
                if d > p.ais_nmea_tol_m {
                    let i = ep.partition_point(|x| x.t < e.t);
                    let clean = fl.get(i).is_some_and(|f| !f.any());
                    if clean {
                        disc.push((r.t, d, r.lat, r.lon));
                    }
                }
            }
        }
    }
    let mi = Ais::median_interval(own_ais);
    let mut groups: Vec<Vec<(f64, f64, f64, f64)>> = vec![];
    for x in disc {
        match groups.last_mut() {
            Some(g) if x.0 - g[g.len() - 1].0 <= 3.0 * mi + 1.0 => g.push(x),
            _ => groups.push(vec![x]),
        }
    }
    groups.retain(|g| g.len() >= 2);
    for g in &groups {
        let n = episodes.len() + 1;
        let (t0, t1) = (g[0].0, g[g.len() - 1].0);
        let dmax = g.iter().map(|x| x.1).fold(0.0, f64::max);
        let pos = nearest_epoch(&ep, t0, 2.0).and_then(|e| e.fix.map(|f| (f, e.hdop)));
        let mut metrics = BTreeMap::new();
        metrics.insert("ais_minus_nmea_max_m".into(), round(dmax, 0));
        metrics.insert("ais_reports_discrepant".into(), g.len() as f64);
        findings.push(Finding {
            label: Label::Reported,
            episode: Some(n),
            source: format!("AIS {}", inp.vessel_id),
            text: format!(
                "{} AIS reports of the vessel between {} and {} were up to {:.1} km from the position its own clean NMEA gave at the same time",
                g.len(),
                iso_s(t0),
                iso_s(t1),
                dmax / 1000.0
            ),
            value: Some(round(dmax, 0)),
            unit: Some("m".into()),
        });
        findings.push(Finding {
            label: Label::Measured,
            episode: Some(n),
            source: "own NMEA".into(),
            text: "The vessel's own GNSS showed no anomaly at those times (C/N0, fix and motion within the bars)".into(),
            value: None,
            unit: None,
        });
        let (mut present, mut corr) = (vec![], vec![]);
        if let (Some(a), Some(((la, lo), _))) = (&ais, pos) {
            for (id, v) in &a.tracks {
                if id == &inp.vessel_id {
                    continue;
                }
                if !v.iter().any(|r| {
                    r.t <= t0
                        && r.t >= t0 - 900.0
                        && haversine_m(la, lo, r.lat, r.lon) <= p.neighbour_radius_m
                }) {
                    continue;
                }
                present.push(id.clone());
                if track_anomaly(
                    v,
                    t0 - p.corroboration_margin_s,
                    t1 + p.corroboration_margin_s,
                    p,
                    None,
                )
                .is_some()
                {
                    corr.push(id.clone());
                }
            }
        }
        findings.push(Finding {
            label: Label::Derived,
            episode: Some(n),
            source: "AIS neighbours".into(),
            text: format!(
                "{} of {} AIS neighbours showed an anomaly at the same time",
                corr.len(),
                present.len()
            ),
            value: Some(corr.len() as f64),
            unit: Some("neighbours".into()),
        });
        timeline.push(tl(
            t0,
            &format!("AIS {}", inp.vessel_id),
            Label::Reported,
            format!("Episode {n}: AIS broadcast departs from own NMEA"),
        ));
        timeline.push(tl(
            t1,
            &format!("AIS {}", inp.vessel_id),
            Label::Reported,
            format!("Episode {n}: last discrepant AIS report"),
        ));
        let confidence = round(combine(&[0.7, if corr.is_empty() { 0.3 } else { 0.0 }]), 3);
        let pos_unc = pos.map(|(_, h)| round(2.0 * h.unwrap_or(1.0) * p.uere_m, 1));
        if let Some(((la, lo), _)) = pos {
            events.push(ievents::Event {
                format: ievents::FORMAT.into(),
                id: format!("maritime-{}-{n}", inp.vessel_id),
                start_utc: iso_s(t0),
                end_utc: iso_s(t1.ceil()),
                systems: vec![ievents::System::Other { name: "AIS".into() }],
                kind: ievents::Kind::Spoofing,
                location: Some(ievents::Place {
                    lat_deg: la,
                    lon_deg: lo,
                    radius_m: pos_unc.unwrap_or(10.0).max(10.0),
                }),
                area: None,
                observers: vec![format!("vessel:{}", inp.vessel_id)],
                metrics: metrics.clone(),
                source_estimate: None,
                confidence,
                basis: METHOD_ID.into(),
                evidence: evidence_refs.clone(),
                reporter: "kshana-pro forensics maritime".into(),
                corroborated: false,
            });
        }
        let gap = mi;
        episodes.push(Episode {
            n,
            event_type: EventType::AisManipulation,
            start_utc: iso_s(t0),
            end_utc: iso_s(t1),
            onset: Bound {
                earliest_utc: iso_s(t0 - gap),
                latest_utc: iso_s(t0),
                uncertainty_s: round(gap, 1),
            },
            end: Bound {
                earliest_utc: iso_s(t1),
                latest_utc: iso_s(t1 + gap),
                uncertainty_s: round(gap, 1),
            },
            duration_s: round(t1 - t0, 1),
            last_clean_position: pos.map(|((a, b), _)| [round(a, 6), round(b, 6)]),
            position_uncertainty_m: pos_unc,
            metrics,
            ais_vs_own_nmea: "inconsistent".into(),
            neighbours_present: present,
            neighbours_corroborating: corr,
            station_corroborates: false,
            conclusion: Conclusion::AisManipulation,
            confidence,
        });
    }

    for s in &inp.station {
        if let Some(t) = parse_utc(&s.start_utc) {
            timeline.push(tl(
                t,
                "Kshana Station",
                Label::Reported,
                format!("Station event {} ({:?})", s.id, s.kind),
            ));
        }
    }
    timeline.sort_by(|a, b| a.t.total_cmp(&b.t));

    // ---- overall verdict ----
    let gnss: Vec<&Episode> = episodes
        .iter()
        .filter(|e| matches!(e.event_type, EventType::Jamming | EventType::Spoofing))
        .collect();
    let ais_ep = episodes
        .iter()
        .any(|e| e.conclusion == Conclusion::AisManipulation);
    let pick = |c: Conclusion| episodes.iter().any(|e| e.conclusion == c);
    let conclusion = if episodes.is_empty() {
        Conclusion::NoEvent
    } else if ais_ep {
        Conclusion::AisManipulation
    } else if pick(Conclusion::VictimOfExternalInterference) {
        Conclusion::VictimOfExternalInterference
    } else if pick(Conclusion::LocalisedToVessel) {
        Conclusion::LocalisedToVessel
    } else if pick(Conclusion::ShipSideFault) {
        Conclusion::ShipSideFault
    } else {
        Conclusion::Inconclusive
    };
    let mut rationale = vec![];
    for e in &episodes {
        rationale.push(format!(
            "Episode {} ({:?}, {} to {}): AIS vs own NMEA {}, {} of {} neighbours corroborate{}; conclusion {:?}",
            e.n,
            e.event_type,
            e.start_utc,
            e.end_utc,
            e.ais_vs_own_nmea,
            e.neighbours_corroborating.len(),
            e.neighbours_present.len(),
            if e.station_corroborates { ", Station corroborates" } else { "" },
            e.conclusion
        ));
    }
    if episodes.is_empty() {
        rationale.push("No episode met the pre-registered bars in the record supplied.".into());
    }
    let verdict = ActorVictim {
        gnss_interference: if !gnss.is_empty() {
            Answer::Indicated
        } else if episodes
            .iter()
            .any(|e| e.event_type == EventType::ShipSideFault)
            || !episodes.is_empty()
            || base.is_some()
        {
            Answer::NotIndicated
        } else {
            Answer::Undetermined
        },
        ais_manipulation: if ais_ep {
            Answer::Indicated
        } else if own_ais.is_empty() {
            Answer::Undetermined
        } else {
            Answer::NotIndicated
        },
        conclusion,
        rationale,
    };

    let mut uncertainty = vec![
        "Onset and end are bounded by the last clean and the first flagged epoch (and the reverse); each episode states the bound in seconds.".to_string(),
        format!(
            "Last clean positions carry a 95 % horizontal uncertainty of 2 × HDOP × {:.0} m (illustrative UERE).",
            p.uere_m
        ),
        "Neighbour corroboration is limited by AIS reporting intervals: a neighbour reporting every 10 s bounds an anomaly's timing to about ±10 s.".into(),
    ];
    if base.is_none() {
        uncertainty.push(
            "The log carries no C/N0 in the calibration window; C/N0 indicators were not assessed."
                .into(),
        );
    }
    let has_stw = ep.iter().any(|e| e.stw.is_some());
    let has_hdg = ep.iter().any(|e| e.hdg.is_some());
    let mut limits = vec![
        "Positions in the NMEA log are the receiver's outputs; MEASURED applies to what was logged, not to the vessel's true position.".to_string(),
        "The pack hashes and copies the files supplied; it cannot show that they are the original VDR or receiver record. Chain of custody before the pack is outside it.".into(),
        "AIS transponders may take position from the ship's main receiver or from an internal GNSS; a mismatch can also come from a misconfigured transponder or a third party using the vessel's identity. An AIS discrepancy indicates manipulation of the report, not who made it.".into(),
        "An episode localised to the vessel can come from an on-board or close-range source or from equipment; the data do not separate them.".into(),
        "Thresholds are pre-registered engineering values (Params in the pack); the method is MODELLED (synthetic scenarios), not validated on a labelled set of real incidents.".into(),
        "Not a legal opinion or a finding of fact; it says nothing about intent. Admissibility differs by jurisdiction.".into(),
    ];
    if !has_stw || !has_hdg {
        limits.push("The NMEA log carries no speed through water (VHW) or no gyro heading (HDT/THS); the kinematic check against ship sensors was partly or wholly unavailable.".into());
    }
    if ais.is_none() {
        limits.push("No AIS supplied: neither the vessel's broadcast track nor its neighbours were assessed.".into());
    }

    let inputs = inp
        .files
        .iter()
        .map(|(n, b)| InputFile {
            file: n.clone(),
            sha256: sha256_hex(b),
            bytes: b.len(),
        })
        .collect();
    Ok(EvidencePack {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        evidence: EVIDENCE.into(),
        statement: STATEMENT.into(),
        title: inp.title.clone(),
        vessel_id: inp.vessel_id.clone(),
        params: p.clone(),
        inputs,
        record: RecordSummary {
            nmea_epochs: ep.len(),
            nmea_skipped_records: skipped,
            nmea_start_utc: ep.first().map(|e| iso_s(e.t)),
            nmea_end_utc: ep.last().map(|e| iso_s(e.t)),
            baseline_cn0_dbhz: base.map(|b| round(b.0, 2)),
            baseline_cn0_spread_db: base.map(|b| round(b.1, 2)),
            has_log_speed: has_stw,
            has_heading: has_hdg,
            ais_identities: ais.as_ref().map(|a| a.tracks.len()).unwrap_or(0),
            ais_reports_of_vessel: own_ais.len(),
            station_events: inp.station.len(),
        },
        verdict,
        episodes,
        findings,
        timeline,
        events,
        confidence_rule: CONFIDENCE_RULE.into(),
        uncertainty,
        limits,
    })
}

/// Write the pack directory (unsigned): `evidence-pack.json`, `events.json`,
/// `events.geojson`, `timeline.csv`, `README.txt` and `inputs/<file>`.
pub fn write_pack(dir: &Path, pack: &EvidencePack, inp: &MaritimeInputs) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("inputs")).map_err(io)?;
    std::fs::write(dir.join(RESULT_FILE), json(pack) + "\n").map_err(io)?;
    std::fs::write(dir.join("events.json"), json(&pack.events) + "\n").map_err(io)?;
    std::fs::write(
        dir.join("events.geojson"),
        serde_json::to_string_pretty(&ievents::to_geojson(&pack.events)).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    let mut csv = String::from("utc,source,label,text\n");
    for t in &pack.timeline {
        csv.push_str(&format!(
            "{},{},{:?},\"{}\"\n",
            t.utc,
            t.source.replace(',', ";"),
            t.label,
            t.text.replace('"', "'")
        ));
    }
    std::fs::write(dir.join("timeline.csv"), csv).map_err(io)?;
    for (n, b) in &inp.files {
        std::fs::write(dir.join("inputs").join(n), b).map_err(io)?;
    }
    let readme = format!(
        "Maritime incident evidence pack ({FORMAT})\n\nTitle: {}\nVessel: {}\nConclusion: {:?}\n\n{STATEMENT}\n\nFiles:\n  evidence-pack.json  findings (MEASURED / DERIVED / REPORTED), episodes, verdict, uncertainty, limits\n  events.json         unified interference events ({})\n  events.geojson      the same for maps\n  timeline.csv        timeline\n  inputs/             the files analysed (hashes in evidence-pack.json)\n  signature.json      Ed25519 signature over every file (verify: kshana-pro dossier verify)\n\nEvidence: {EVIDENCE}\n",
        pack.title, pack.vessel_id, pack.verdict.conclusion, ievents::FORMAT
    );
    std::fs::write(dir.join("README.txt"), readme).map_err(io)?;
    Ok(())
}

fn json<T: Serialize>(v: &T) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

/// Synthetic scenarios with known truth (tests and demonstrations).
pub mod synth {
    use super::KN;

    /// The scenario classes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Case {
        Clean,
        AreaJamming,
        AreaSpoofing,
        LocalSpoofing,
        AisManipulation,
        ShipSideFault,
    }

    /// A generated scenario.
    #[derive(Clone, Debug)]
    pub struct Scenario {
        pub vessel_id: String,
        pub nmea: String,
        pub ais_csv: String,
    }

    /// Start of the scenario (2026-06-01T08:00:00Z) in Unix seconds.
    pub const T0: f64 = 1_780_300_800.0;
    /// Episode window (s after T0).
    pub const EV: (f64, f64) = (600.0, 1100.0);
    /// The false position the spoofer pushes receivers to.
    pub const FALSE_POS: (f64, f64) = (26.32, 56.48);
    const DUR: usize = 1800;

    struct Rng(u64);
    impl Rng {
        fn u(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn n(&mut self) -> f64 {
            (self.u() + self.u() + self.u() + self.u() - 2.0) * 1.732
        }
    }

    fn dest(lat: f64, lon: f64, north_m: f64, east_m: f64) -> (f64, f64) {
        let r = 6_371_000.0;
        (
            lat + (north_m / r).to_degrees(),
            lon + (east_m / (r * lat.to_radians().cos())).to_degrees(),
        )
    }

    fn cs(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, b| a ^ b);
        format!("${body}*{x:02X}\n")
    }

    fn hms(t: f64) -> String {
        let s = (t - (t / 86_400.0).floor() * 86_400.0) as i64;
        format!("{:02}{:02}{:02}.00", s / 3600, s / 60 % 60, s % 60)
    }

    fn ll(lat: f64, lon: f64) -> String {
        let f = |v: f64, w: usize| {
            let a = v.abs();
            let d = a.floor();
            format!("{:0w$}{:07.4}", d as i64, (a - d) * 60.0, w = w)
        };
        format!(
            "{},{},{},{}",
            f(lat, 2),
            if lat >= 0.0 { "N" } else { "S" },
            f(lon, 3),
            if lon >= 0.0 { "E" } else { "W" }
        )
    }

    struct Ship {
        id: &'static str,
        lat: f64,
        lon: f64,
        sog_kn: f64,
        cog: f64,
        phase: usize,
    }

    impl Ship {
        fn at(&self, s: f64) -> (f64, f64) {
            let d = self.sog_kn * KN * s;
            dest(
                self.lat,
                self.lon,
                d * self.cog.to_radians().cos(),
                d * self.cog.to_radians().sin(),
            )
        }
    }

    fn fleet() -> (Ship, Vec<Ship>) {
        let own = Ship {
            id: "SUBJ",
            lat: 26.20,
            lon: 56.30,
            sog_kn: 12.0,
            cog: 90.0,
            phase: 0,
        };
        let n = vec![
            Ship {
                id: "N1",
                lat: 26.23,
                lon: 56.33,
                sog_kn: 10.0,
                cog: 270.0,
                phase: 3,
            },
            Ship {
                id: "N2",
                lat: 26.17,
                lon: 56.36,
                sog_kn: 14.0,
                cog: 45.0,
                phase: 6,
            },
            Ship {
                id: "N3",
                lat: 26.25,
                lon: 56.27,
                sog_kn: 8.0,
                cog: 120.0,
                phase: 1,
            },
            Ship {
                id: "N4",
                lat: 26.15,
                lon: 56.25,
                sog_kn: 11.0,
                cog: 10.0,
                phase: 8,
            },
        ];
        (own, n)
    }

    /// Generate a scenario.
    pub fn make(case: Case) -> Scenario {
        use Case::*;
        let (own, nb) = fleet();
        let mut r = Rng(0x9e37_79b9_7f4a_7c15);
        let prns = [2u32, 5, 7, 9, 13, 15, 18, 21, 30];
        let base: Vec<f64> = vec![46.0, 41.0, 38.0, 44.0, 35.0, 48.0, 40.0, 37.0, 43.0];
        let (e0, e1) = EV;
        let mut nmea = String::new();
        let mut own_reported: Vec<Option<(f64, f64)>> = vec![];
        let mut drag = (0.0f64, 0.0f64); // offset north (m), velocity north (m/s)
        for k in 0..DUR {
            let s = k as f64;
            let t = super::super::inputs::iso(T0 + s);
            let date = format!("{}{}{}", &t[8..10], &t[5..7], &t[2..4]);
            let truth = own.at(s);
            let in_ev = s >= e0 && s < e1;
            // Defaults: truthful, valid.
            let (mut pos, mut sog, mut cog, mut valid) = (Some(truth), own.sog_kn, own.cog, true);
            let mut cn0: Vec<Option<f64>> = base.iter().map(|b| Some(b + 0.7 * r.n())).collect();
            match case {
                AreaJamming if in_ev => {
                    let depth = ((s - e0) / 60.0).min(1.0) * 22.0;
                    for c in cn0.iter_mut() {
                        *c = c.map(|v| v - depth).filter(|v| *v >= 28.0);
                    }
                    if cn0.iter().filter(|c| c.is_some()).count() < 4 {
                        valid = false;
                    }
                }
                AreaSpoofing | LocalSpoofing if in_ev => {
                    for c in cn0.iter_mut() {
                        *c = Some(50.0 + 0.3 * r.n());
                    }
                    if s < 800.0 {
                        drag.1 = (0.05 * (s - e0)).min(8.0);
                        drag.0 += drag.1;
                        pos = Some(dest(truth.0, truth.1, drag.0, 0.0));
                        let (vn, ve) = (
                            own.sog_kn * KN * own.cog.to_radians().cos() + drag.1,
                            own.sog_kn * KN * own.cog.to_radians().sin(),
                        );
                        sog = (vn * vn + ve * ve).sqrt() / KN;
                        cog = ve.atan2(vn).to_degrees().rem_euclid(360.0);
                    } else {
                        pos = Some(dest(FALSE_POS.0, FALSE_POS.1, 3.0 * r.n(), 3.0 * r.n()));
                        sog = 0.1;
                        cog = 0.0;
                    }
                }
                ShipSideFault if (e0..900.0).contains(&s) => valid = false,
                _ => {}
            }
            if !valid {
                pos = None;
            }
            own_reported.push(pos);
            let (la, lo) = pos.unwrap_or(truth);
            let ll_s = if valid { ll(la, lo) } else { ",,,".into() };
            nmea += &cs(&format!(
                "GPRMC,{},{},{},{:.1},{:.1},{date},,,{}",
                hms(T0 + s),
                if valid { "A" } else { "V" },
                ll_s,
                if valid { sog } else { 0.0 },
                if valid { cog } else { 0.0 },
                if valid { "A" } else { "N" }
            ));
            let used: Vec<u32> = prns
                .iter()
                .zip(&cn0)
                .filter(|(_, c)| c.is_some())
                .map(|(p, _)| *p)
                .collect();
            nmea += &cs(&format!(
                "GPGGA,{},{},{},{:02},{:.1},12.0,M,-25.0,M,,",
                hms(T0 + s),
                ll_s,
                if valid { 1 } else { 0 },
                used.len(),
                if valid { 0.9 } else { 99.9 }
            ));
            let mut gsa: Vec<String> = used.iter().map(|p| format!("{p:02}")).collect();
            gsa.resize(12, String::new());
            nmea += &cs(&format!(
                "GPGSA,A,{},{},1.6,0.9,1.3",
                if valid { 3 } else { 1 },
                gsa.join(",")
            ));
            let tot = prns.len().div_ceil(4);
            for (g, ch) in prns.chunks(4).enumerate() {
                let mut b = format!("GPGSV,{tot},{},{:02}", g + 1, prns.len());
                for (j, p) in ch.iter().enumerate() {
                    let i = g * 4 + j;
                    let snr = cn0[i]
                        .map(|v| format!("{:.0}", v.round()))
                        .unwrap_or_default();
                    b += &format!(",{p:02},{:02},{:03},{snr}", 20 + 7 * i, (40 * i) % 360);
                }
                nmea += &cs(&b);
            }
            nmea += &cs(&format!("HEHDT,{:.1},T", own.cog + 0.5 * r.n()));
            nmea += &cs(&format!(
                "VWVHW,{:.1},T,,M,{:.1},N,,K",
                own.cog,
                own.sog_kn + 0.2 * r.n()
            ));
        }
        // AIS every 10 s (phase per vessel).
        let mut ais = String::from("timestamp,vessel_id,lat,lon,sog_kn\n");
        let mut rows: Vec<(f64, String)> = vec![];
        for k in (own.phase..DUR).step_by(10) {
            let s = k as f64;
            let in_ev = s >= e0 && s < e1;
            let p = match case {
                AisManipulation if (e0..1200.0).contains(&s) => {
                    // A fabricated track 40 km south, heading west.
                    let tr = own.at(s);
                    Some(dest(tr.0, tr.1, -40_000.0, -(s - e0) * 6.0))
                }
                ShipSideFault => Some(own.at(s)), // the transponder's internal GNSS keeps reporting
                _ => own_reported[k],
            };
            if case == AreaJamming && in_ev {
                continue;
            }
            if let Some((la, lo)) = p {
                rows.push((
                    s,
                    format!(
                        "{},SUBJ,{la:.6},{lo:.6},{:.1}",
                        super::super::inputs::iso(T0 + s),
                        own.sog_kn
                    ),
                ));
            }
        }
        for v in &nb {
            for k in (v.phase..DUR).step_by(10) {
                let s = k as f64;
                let in_ev = s >= e0 && s < e1;
                let mut p = Some(v.at(s));
                if in_ev {
                    match case {
                        AreaJamming => p = None,
                        AreaSpoofing if s >= 800.0 => {
                            p = Some(dest(FALSE_POS.0, FALSE_POS.1, 40.0 * r.n(), 40.0 * r.n()))
                        }
                        _ => {}
                    }
                }
                if let Some((la, lo)) = p {
                    rows.push((
                        s,
                        format!(
                            "{},{},{la:.6},{lo:.6},{:.1}",
                            super::super::inputs::iso(T0 + s),
                            v.id,
                            v.sog_kn
                        ),
                    ));
                }
            }
        }
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, l) in rows {
            ais += &l;
            ais.push('\n');
        }
        Scenario {
            vessel_id: own.id.into(),
            nmea,
            ais_csv: ais,
        }
    }

    /// The scenario as in-memory inputs.
    pub fn inputs(case: Case) -> super::MaritimeInputs {
        let s = make(case);
        super::MaritimeInputs {
            title: format!("Synthetic {case:?}"),
            vessel_id: s.vessel_id,
            nmea: s.nmea.clone(),
            ais: Some(s.ais_csv.clone()),
            station: vec![],
            files: vec![
                ("own.nmea".into(), s.nmea.into_bytes()),
                ("ais.csv".into(), s.ais_csv.into_bytes()),
            ],
            params: super::Params::default(),
        }
    }
}
