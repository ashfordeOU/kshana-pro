// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Episodes (`kshana-pro/interference-episodes/v1`): when an interference episode starts and
//! ends, written down as rules (the EASA/EUROCONTROL action plan of Feb 2026 asks for a
//! common definition).
//!
//! **Members.** A normalised report (not a NOTAM, and only when its time is known to
//! `max_time_uncertainty_min`) is active from its start to its stated end, else to start plus
//! its time to recover, else to start plus `report_default_duration_min`. An ADS-B aircraft
//! is active while its NIC is below `adsb_nic_min` or its NACp below `adsb_nacp_min`, having
//! reported at or above them earlier (aircraft never seen at or above are counted as
//! unassessed); degraded samples closer than `adsb_run_gap_s` form one run, and runs shorter
//! than `adsb_min_run_s` are dropped. A jump of at least `adsb_jump_km` at more than
//! `adsb_jump_kn` is a spoofing-like member at that instant.
//!
//! **Linking.** Two members are linked when their active intervals are no more than
//! `quiet_gap_min` apart and they are within `join_radius_km` of each other, or belong to the
//! same subject (a moving aircraft or vessel). A candidate episode is a connected group of
//! linked members (order-independent).
//!
//! **Start and end.** An episode starts at its earliest member start and ends at its latest
//! member end: it is closed because no member was active within `quiet_gap_min` after that
//! end, unless the input data stops within that gap (`open_at_data_end`).
//!
//! **Threshold.** A candidate becomes an episode with at least `min_reports` reports or at
//! least `adsb_min_aircraft` degraded ADS-B aircraft; the others are listed as below
//! threshold.
//!
//! **Kind.** Jamming when only jamming-like members (jamming reports, NIC/NACp drops),
//! spoofing when only spoofing-like ones (spoofing reports, ADS-B jumps), mixed with both,
//! unknown otherwise.
//!
//! Evidence: MODELLED. The rules are engineering choices stated in every output; they are not
//! an agreed regulatory definition and were checked on synthetic and sample inputs only.

use std::collections::{BTreeMap, BTreeSet};

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};

use super::text::{iso, round};
use super::*;
use crate::forensics::inputs::{parse_tracks, PositionReport};
use crate::ievents;

pub const FORMAT: &str = "kshana-pro-interference-episodes/1";
pub const METHOD_ID: &str = "kshana-pro/interference-episodes/v1";
pub const RESULT_FILE: &str = "episodes.json";
pub const EVIDENCE: &str = "MODELLED: start and end rules are stated engineering choices, checked on \
synthetic and sample inputs; not an agreed regulatory definition and not validated on a labelled set of real episodes.";

/// Every threshold of the method; written into each output with its SHA-256.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub join_radius_km: f64,
    pub quiet_gap_min: f64,
    pub report_default_duration_min: f64,
    pub max_time_uncertainty_min: f64,
    pub min_reports: usize,
    pub adsb_nic_min: u8,
    pub adsb_nacp_min: u8,
    pub adsb_min_aircraft: usize,
    pub adsb_run_gap_s: f64,
    pub adsb_min_run_s: f64,
    pub adsb_jump_km: f64,
    pub adsb_jump_kn: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            join_radius_km: 150.0,
            quiet_gap_min: 15.0,
            report_default_duration_min: 5.0,
            max_time_uncertainty_min: 60.0,
            min_reports: 1,
            adsb_nic_min: 7,
            adsb_nacp_min: 8,
            adsb_min_aircraft: 2,
            adsb_run_gap_s: 120.0,
            adsb_min_run_s: 30.0,
            adsb_jump_km: 5.0,
            adsb_jump_kn: 900.0,
        }
    }
}

impl Params {
    pub fn check(&self) -> Result<(), String> {
        let pos = [
            ("join_radius_km", self.join_radius_km),
            ("quiet_gap_min", self.quiet_gap_min),
            (
                "report_default_duration_min",
                self.report_default_duration_min,
            ),
            ("max_time_uncertainty_min", self.max_time_uncertainty_min),
            ("adsb_run_gap_s", self.adsb_run_gap_s),
            ("adsb_jump_km", self.adsb_jump_km),
            ("adsb_jump_kn", self.adsb_jump_kn),
        ];
        for (n, v) in pos {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("params: {n} must be positive"));
            }
        }
        if !(self.adsb_min_run_s.is_finite() && self.adsb_min_run_s >= 0.0) {
            return Err("params: adsb_min_run_s must be zero or more".into());
        }
        if self.min_reports == 0 && self.adsb_min_aircraft == 0 {
            return Err("params: min_reports and adsb_min_aircraft cannot both be 0".into());
        }
        Ok(())
    }

    /// The default parameters as TOML (a starting point for a parameter file).
    pub fn default_toml() -> String {
        toml::to_string(&Params::default()).unwrap_or_default()
    }
}

/// A member's kind of evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemberSource {
    Report,
    AdsbIntegrity,
    AdsbJump,
}

/// One member of an episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub source: MemberSource,
    /// The report id, or `adsb:<aircraft id>#<run>`.
    pub id: String,
    /// Registration / IMO / MMSI of a report, or the ADS-B aircraft id.
    #[serde(default)]
    pub subject_id: Option<String>,
    pub platform: Platform,
    pub start_utc: String,
    pub end_utc: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub alt_ft: Option<f64>,
    /// jamming-like, spoofing-like or unknown.
    pub kind: Kind,
    pub detail: String,
    #[serde(skip)]
    t0: f64,
    #[serde(skip)]
    t1: f64,
}

/// One episode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_min: f64,
    pub start_rule: String,
    pub end_rule: String,
    pub open_at_data_end: bool,
    pub centroid_lat_deg: f64,
    pub centroid_lon_deg: f64,
    /// Largest member distance from the centroid, km.
    pub radius_km: f64,
    #[serde(default)]
    pub lower_ft: Option<f64>,
    #[serde(default)]
    pub upper_ft: Option<f64>,
    /// `jamming`, `spoofing`, `mixed` or `unknown`.
    pub kind: String,
    pub kind_basis: BTreeMap<String, usize>,
    pub reports: Vec<String>,
    pub adsb_aircraft: Vec<String>,
    /// Distinct subjects (report subjects and ADS-B aircraft), the count of independent
    /// observers behind the episode.
    pub observers: usize,
    #[serde(default)]
    pub max_time_to_recover_min: Option<f64>,
    /// NOTAMs whose window and area overlap the episode (context only).
    pub context_notams: Vec<String>,
    pub members: Vec<Member>,
}

/// A member left out, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LeftOut {
    pub id: String,
    pub reason: String,
}

/// ADS-B coverage summary.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AdsbSummary {
    pub files: usize,
    pub rows: usize,
    pub refused_rows: usize,
    pub aircraft: usize,
    /// Aircraft seen at or above the NIC/NACp thresholds at some point.
    pub assessed: usize,
    pub unassessed: usize,
    pub integrity_runs: usize,
    pub jumps: usize,
}

/// The episodes document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpisodesDoc {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub definition: Vec<String>,
    pub params: Params,
    pub params_sha256: String,
    pub inputs: Vec<InputFile>,
    pub reports_read: usize,
    pub adsb: AdsbSummary,
    pub episodes: Vec<Episode>,
    pub below_threshold: Vec<LeftOut>,
    pub not_clustered: Vec<LeftOut>,
}

/// The rules, as written into every output.
pub fn definition(p: &Params) -> Vec<String> {
    vec![
        format!(
            "Report member: active from its start to its stated end, else start + time to recover, else start + {} min; \
             NOTAMs are context only; reports with time uncertainty above {} min are not clustered.",
            p.report_default_duration_min, p.max_time_uncertainty_min
        ),
        format!(
            "ADS-B member: NIC < {} or NACp < {} after the aircraft reported at or above them; samples within {} s form one run; \
             runs under {} s dropped. A jump of at least {} km at more than {} kn is a spoofing-like member.",
            p.adsb_nic_min, p.adsb_nacp_min, p.adsb_run_gap_s, p.adsb_min_run_s, p.adsb_jump_km, p.adsb_jump_kn
        ),
        format!(
            "Linking: two members are linked when within {} km of each other (or of the same subject) and their active intervals are at most {} min apart; an episode is a connected group.",
            p.join_radius_km, p.quiet_gap_min
        ),
        format!(
            "Start: the earliest member start. End: the latest member end, closed when no member is active within {} min after it \
             (open_at_data_end when the data stops inside that gap).",
            p.quiet_gap_min
        ),
        format!(
            "Threshold: at least {} report(s) or at least {} degraded ADS-B aircraft.",
            p.min_reports, p.adsb_min_aircraft
        ),
        "Kind: jamming (jamming reports, NIC/NACp drops), spoofing (spoofing reports, ADS-B jumps), mixed when both, unknown otherwise.".into(),
    ]
}

fn degraded(r: &PositionReport, p: &Params) -> Option<bool> {
    match (r.nic, r.nacp) {
        (None, None) => None,
        (a, b) => {
            Some(a.is_some_and(|v| v < p.adsb_nic_min) || b.is_some_and(|v| v < p.adsb_nacp_min))
        }
    }
}

/// ADS-B members from parsed tracks.
fn adsb_members(
    tracks: &BTreeMap<String, Vec<PositionReport>>,
    p: &Params,
    sum: &mut AdsbSummary,
) -> Vec<Member> {
    let mut out = vec![];
    for (id, v) in tracks {
        sum.aircraft += 1;
        let mut ever_good = false;
        let mut runs: Vec<(usize, usize)> = vec![];
        let mut last_clean: Option<&PositionReport> = None;
        let mut cur: Option<(usize, usize)> = None;
        let mut run_anchor: Vec<Option<(f64, f64)>> = vec![];
        for (i, r) in v.iter().enumerate() {
            match degraded(r, p) {
                Some(false) => {
                    ever_good = true;
                    last_clean = Some(r);
                    if let Some(c) = cur.take() {
                        runs.push(c);
                    }
                }
                Some(true) if ever_good => {
                    cur = match cur {
                        Some((a, b)) if r.t - v[b].t <= p.adsb_run_gap_s => Some((a, i)),
                        Some(c) => {
                            runs.push(c);
                            run_anchor.push(last_clean.map(|c| (c.lat, c.lon)));
                            Some((i, i))
                        }
                        None => {
                            run_anchor.push(last_clean.map(|c| (c.lat, c.lon)));
                            Some((i, i))
                        }
                    };
                }
                _ => {}
            }
        }
        if let Some(c) = cur {
            runs.push(c);
        }
        if ever_good {
            sum.assessed += 1;
        } else {
            sum.unassessed += 1;
        }
        for (k, (a, b)) in runs.iter().enumerate() {
            let (t0, t1) = (v[*a].t, v[*b].t);
            if t1 - t0 < p.adsb_min_run_s {
                continue;
            }
            sum.integrity_runs += 1;
            // Located at the last clean position before the run (a spoofed or degraded
            // position is not used), else the first degraded sample.
            let (lat, lon) = run_anchor
                .get(k)
                .copied()
                .flatten()
                .unwrap_or((v[*a].lat, v[*a].lon));
            out.push(Member {
                source: MemberSource::AdsbIntegrity,
                id: format!("adsb:{id}#{}", k + 1),
                subject_id: Some(id.clone()),
                platform: Platform::Aircraft,
                start_utc: iso(t0),
                end_utc: iso(t1),
                lat_deg: lat,
                lon_deg: lon,
                alt_ft: v[*a].alt_ft,
                kind: Kind::Jamming,
                detail: format!(
                    "NIC/NACp below {}/{} for {:.0} s ({} samples)",
                    p.adsb_nic_min,
                    p.adsb_nacp_min,
                    t1 - t0,
                    b - a + 1
                ),
                t0,
                t1,
            });
        }
        for (j, w) in v.windows(2).enumerate() {
            let d = haversine_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
            let dt = (w[1].t - w[0].t).max(1e-3);
            let kn = d / dt / 0.514_444;
            if d >= p.adsb_jump_km * 1000.0 && kn > p.adsb_jump_kn {
                sum.jumps += 1;
                out.push(Member {
                    source: MemberSource::AdsbJump,
                    id: format!("adsb:{id}@jump{}", j + 1),
                    subject_id: Some(id.clone()),
                    platform: Platform::Aircraft,
                    start_utc: iso(w[1].t),
                    end_utc: iso(w[1].t),
                    lat_deg: w[0].lat,
                    lon_deg: w[0].lon,
                    alt_ft: w[0].alt_ft,
                    kind: Kind::Spoofing,
                    detail: format!("position jump {:.1} km at {:.0} kn implied", d / 1000.0, kn),
                    t0: w[1].t,
                    t1: w[1].t,
                });
            }
        }
    }
    out
}

/// Build episodes from normalised reports and ADS-B CSV files (`(name, text)`).
pub fn build(
    reports: &[Report],
    adsb: &[(String, String)],
    p: &Params,
    inputs: Vec<InputFile>,
) -> Result<EpisodesDoc, String> {
    p.check()?;
    let mut members: Vec<Member> = vec![];
    let mut not_clustered = vec![];
    let mut notams: Vec<&Report> = vec![];
    for r in reports {
        if r.reporter_role == ReporterRole::Notam {
            notams.push(r);
            continue;
        }
        let (Some(t0), Some(t1)) = (r.start_t(), r.end_t(p.report_default_duration_min)) else {
            not_clustered.push(LeftOut {
                id: r.id.clone(),
                reason: "no start time".into(),
            });
            continue;
        };
        if r.time_uncertainty_s > p.max_time_uncertainty_min * 60.0 {
            not_clustered.push(LeftOut {
                id: r.id.clone(),
                reason: format!(
                    "time uncertain by {:.0} min (over {} min)",
                    r.time_uncertainty_s / 60.0,
                    p.max_time_uncertainty_min
                ),
            });
            continue;
        }
        let Some(pos) = &r.position else {
            not_clustered.push(LeftOut {
                id: r.id.clone(),
                reason: "no position".into(),
            });
            continue;
        };
        members.push(Member {
            source: MemberSource::Report,
            id: r.id.clone(),
            subject_id: r.subject_id.clone().or_else(|| r.flight_or_voyage.clone()),
            platform: r.platform,
            start_utc: iso(t0),
            end_utc: iso(t1),
            lat_deg: pos.lat_deg,
            lon_deg: pos.lon_deg,
            alt_ft: r.altitude.as_ref().and_then(|a| a.lower_ft),
            kind: r.kind,
            detail: format!(
                "{:?} report ({:?}), type {:?}",
                r.reporter_role, r.source.format, r.reported_type
            )
            .to_lowercase(),
            t0,
            t1,
        });
    }
    let mut sum = AdsbSummary::default();
    let mut data_end = members
        .iter()
        .map(|m| m.t1)
        .fold(f64::NEG_INFINITY, f64::max);
    for (name, text) in adsb {
        let tr = parse_tracks(text, "aircraft_id").map_err(|e| format!("{name}: {e}"))?;
        sum.files += 1;
        sum.rows += tr.rows;
        sum.refused_rows += tr.refused;
        if let Some(t) = tr.times().last() {
            data_end = data_end.max(*t);
        }
        members.extend(adsb_members(&tr.by_id, p, &mut sum));
    }
    members.sort_by(|a, b| a.t0.total_cmp(&b.t0).then(a.id.cmp(&b.id)));

    // Linking: two members are linked when their active intervals are no more than the quiet
    // gap apart and they are within the join radius, or belong to the same subject (a moving
    // aircraft or vessel); an episode is a connected group (union-find, order-independent).
    struct Building {
        idx: Vec<usize>,
        lat: f64,
        lon: f64,
    }
    let gap = p.quiet_gap_min * 60.0;
    let n = members.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for a in 0..n {
        for b in a + 1..n {
            let (ma, mb) = (&members[a], &members[b]);
            if mb.t0 > ma.t1 + gap || ma.t0 > mb.t1 + gap {
                continue;
            }
            let same = ma.subject_id.is_some() && ma.subject_id == mb.subject_id;
            if same
                || haversine_m(ma.lat_deg, ma.lon_deg, mb.lat_deg, mb.lon_deg)
                    <= p.join_radius_km * 1000.0
            {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                if ra != rb {
                    parent[ra.max(rb)] = ra.min(rb);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut eps: Vec<Building> = groups
        .into_values()
        .map(|idx| {
            let k = idx.len() as f64;
            let lat = idx.iter().map(|&i| members[i].lat_deg).sum::<f64>() / k;
            let lon = idx.iter().map(|&i| members[i].lon_deg).sum::<f64>() / k;
            Building { idx, lat, lon }
        })
        .collect();
    eps.sort_by(|a, b| members[a.idx[0]].t0.total_cmp(&members[b.idx[0]].t0));

    let mut episodes = vec![];
    let mut below = vec![];
    for b in eps {
        let ms: Vec<&Member> = b.idx.iter().map(|&i| &members[i]).collect();
        let reports_in: Vec<String> = ms
            .iter()
            .filter(|m| m.source == MemberSource::Report)
            .map(|m| m.id.clone())
            .collect();
        let aircraft: BTreeSet<String> = ms
            .iter()
            .filter(|m| m.source == MemberSource::AdsbIntegrity)
            .filter_map(|m| m.subject_id.clone())
            .collect();
        let enough = (p.min_reports > 0 && reports_in.len() >= p.min_reports)
            || (p.adsb_min_aircraft > 0 && aircraft.len() >= p.adsb_min_aircraft);
        if !enough {
            for m in &ms {
                below.push(LeftOut {
                    id: m.id.clone(),
                    reason: format!(
                        "candidate with {} report(s) and {} degraded aircraft: below threshold",
                        reports_in.len(),
                        aircraft.len()
                    ),
                });
            }
            continue;
        }
        let t0 = ms.iter().map(|m| m.t0).fold(f64::INFINITY, f64::min);
        let t1 = ms.iter().map(|m| m.t1).fold(f64::NEG_INFINITY, f64::max);
        let first = ms
            .iter()
            .min_by(|a, b| a.t0.total_cmp(&b.t0))
            .expect("non-empty");
        let last = ms
            .iter()
            .max_by(|a, b| a.t1.total_cmp(&b.t1))
            .expect("non-empty");
        let open = data_end - t1 < gap;
        let radius = ms
            .iter()
            .map(|m| haversine_m(b.lat, b.lon, m.lat_deg, m.lon_deg))
            .fold(0.0, f64::max);
        let mut basis = BTreeMap::new();
        for m in &ms {
            let key = match (m.source, m.kind) {
                (MemberSource::Report, k) => format!("reports_{}", k.as_str()),
                (MemberSource::AdsbIntegrity, _) => "adsb_integrity_runs".into(),
                (MemberSource::AdsbJump, _) => "adsb_jumps".into(),
            };
            *basis.entry(key).or_insert(0) += 1;
        }
        let jam = ms.iter().any(|m| m.kind == Kind::Jamming);
        let spoof = ms.iter().any(|m| m.kind == Kind::Spoofing);
        let kind = match (jam, spoof) {
            (true, true) => "mixed",
            (true, false) => "jamming",
            (false, true) => "spoofing",
            _ => "unknown",
        };
        let alts: Vec<f64> = ms.iter().filter_map(|m| m.alt_ft).collect();
        let observers: BTreeSet<String> = ms
            .iter()
            .map(|m| m.subject_id.clone().unwrap_or_else(|| m.id.clone()))
            .collect();
        let ttr = ms
            .iter()
            .filter(|m| m.source == MemberSource::Report)
            .filter_map(|m| {
                reports
                    .iter()
                    .find(|r| r.id == m.id)
                    .and_then(|r| r.time_to_recover_min)
            })
            .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |a| a.max(v))));
        let ctx: Vec<String> = notams
            .iter()
            .filter(|n| {
                let (Some(s), Some(pos)) = (n.start_t(), &n.position) else {
                    return false;
                };
                let e = n
                    .occurred_end_utc
                    .as_deref()
                    .and_then(text::parse_utc)
                    .unwrap_or(f64::INFINITY);
                let d = haversine_m(b.lat, b.lon, pos.lat_deg, pos.lon_deg);
                s <= t1 && e >= t0 && d <= pos.radius_m.unwrap_or(0.0) + radius + 1.0
            })
            .map(|n| n.id.clone())
            .collect();
        let n = episodes.len() + 1;
        let stamp: String = iso(t0)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(12)
            .collect();
        episodes.push(Episode {
            id: format!("ep-{stamp}-{n}"),
            start_utc: iso(t0),
            end_utc: iso(t1),
            duration_min: round((t1 - t0) / 60.0, 1),
            start_rule: format!("earliest member start: {} ({})", first.id, first.detail),
            end_rule: if open {
                format!(
                    "latest member end: {}; input data ends within {} min, so the episode may continue",
                    last.id, p.quiet_gap_min
                )
            } else {
                format!(
                    "latest member end: {}; no member active within {} min after it",
                    last.id, p.quiet_gap_min
                )
            },
            open_at_data_end: open,
            centroid_lat_deg: round(b.lat, 5),
            centroid_lon_deg: round(b.lon, 5),
            radius_km: round(radius / 1000.0, 1),
            lower_ft: alts.iter().copied().reduce(f64::min),
            upper_ft: alts.iter().copied().reduce(f64::max),
            kind: kind.into(),
            kind_basis: basis,
            reports: reports_in,
            adsb_aircraft: aircraft.into_iter().collect(),
            observers: observers.len(),
            max_time_to_recover_min: ttr,
            context_notams: ctx,
            members: ms.into_iter().cloned().collect(),
        });
    }
    Ok(EpisodesDoc {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        evidence: EVIDENCE.into(),
        definition: definition(p),
        params: p.clone(),
        params_sha256: params_hash(p),
        inputs,
        reports_read: reports.len(),
        adsb: sum,
        episodes,
        below_threshold: below,
        not_clustered,
    })
}

/// The episodes as unified interference events (`kshana-pnt-interference-event/1`).
pub fn to_events(doc: &EpisodesDoc) -> Vec<ievents::Event> {
    doc.episodes
        .iter()
        .map(|e| {
            let kind = match e.kind.as_str() {
                "jamming" => ievents::Kind::Jamming,
                "spoofing" => ievents::Kind::Spoofing,
                _ => ievents::Kind::Unknown,
            };
            let mut metrics = BTreeMap::new();
            metrics.insert("duration_min".into(), e.duration_min);
            metrics.insert("reports".into(), e.reports.len() as f64);
            metrics.insert("adsb_aircraft".into(), e.adsb_aircraft.len() as f64);
            ievents::Event {
                format: ievents::FORMAT.into(),
                id: e.id.clone(),
                start_utc: e.start_utc.clone(),
                end_utc: e.end_utc.clone(),
                systems: vec![ievents::System::Gnss { band: "L1".into() }],
                kind,
                location: Some(ievents::Place {
                    lat_deg: e.centroid_lat_deg,
                    lon_deg: e.centroid_lon_deg,
                    radius_m: (e.radius_km * 1000.0).max(5000.0),
                }),
                area: None,
                observers: e
                    .members
                    .iter()
                    .map(|m| m.subject_id.clone().unwrap_or_else(|| m.id.clone()))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                metrics,
                source_estimate: None,
                // A count rule, not a probability: 1 − 0.5^observers, capped at 0.95.
                confidence: round((1.0 - 0.5f64.powi(e.observers as i32)).min(0.95), 3),
                basis: format!(
                    "{METHOD_ID}: {} report(s), {} ADS-B aircraft; confidence is 1 - 0.5^observers (a count rule, not a probability)",
                    e.reports.len(),
                    e.adsb_aircraft.len()
                ),
                evidence: doc
                    .inputs
                    .iter()
                    .map(|i| ievents::EvidenceRef {
                        file: i.file.clone(),
                        sha256: i.sha256.clone(),
                    })
                    .collect(),
                reporter: "kshana-pro reports episodes".into(),
                corroborated: e.observers >= 2,
            }
        })
        .collect()
}

/// `episodes.csv`: one row per episode.
pub fn episodes_csv(doc: &EpisodesDoc) -> String {
    let mut s = String::from(
        "id,start_utc,end_utc,duration_min,kind,centroid_lat,centroid_lon,radius_km,reports,adsb_aircraft,observers,open_at_data_end\n",
    );
    for e in &doc.episodes {
        s += &format!(
            "{},{},{},{},{},{},{},{},{},{},{},{}\n",
            e.id,
            e.start_utc,
            e.end_utc,
            e.duration_min,
            e.kind,
            e.centroid_lat_deg,
            e.centroid_lon_deg,
            e.radius_km,
            e.reports.len(),
            e.adsb_aircraft.len(),
            e.observers,
            e.open_at_data_end
        );
    }
    s
}

/// Write the episodes directory: `episodes.json`, `episodes.csv`, `events.json` and the
/// inputs byte for byte. The caller signs it.
pub fn write_dir(
    dir: &std::path::Path,
    doc: &EpisodesDoc,
    inputs: &[(String, Vec<u8>)],
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let w = |n: &str, s: String| {
        std::fs::write(dir.join(n), s).map_err(|e| format!("{}: {e}", dir.join(n).display()))
    };
    w(RESULT_FILE, json_text(doc))?;
    w("episodes.csv", episodes_csv(doc))?;
    w("events.json", json_text(&to_events(doc)))?;
    write_inputs(dir, inputs)
}
