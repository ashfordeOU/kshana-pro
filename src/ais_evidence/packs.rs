// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Sanctions-review, IUU-review and subsea-asset evidence packs (E07, Pro)
//!
//! Three packs over the shared track engine ([`super`]): each takes the user's decoded AIS
//! and a request with pre-registered thresholds, and writes indicators per vessel, each
//! labelled REPORTED (as broadcast) or DERIVED (computed here), with the limits.
//!
//! * [`sanctions`]: dark-activity review of the vessels the user names. No sanctions list is
//!   bundled and no determination is made: screening and any conclusion are the user's.
//! * [`iuu`]: fishing-activity review against zones the user declares.
//! * [`subsea`]: activity in declared cable or pipeline corridors and around a reported
//!   fault. Presence near an asset is not attribution of damage.
//!
//! **MODELLED.** The fishing-like speed band and the anchor-drag indicator are stated
//! heuristics, not a classifier trained on labelled tracks.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::trust::{
    anchor_drag, classify_gap, dark_sts, port_calls, trust_scores, DarkSts, Drag, GapClass,
    PortCall, Trust,
};
use super::vms::{self, VmsResult};
use super::{
    collision, encounters, gaps, hash_inputs, identity_changes, interp, jumps, load_tracks, r,
    seg_dist_m, shared_imo, Area, Collision, Encounter, Gap, IdentityChange, InputHash, Jump,
    Params, Report as AisReport, Tracks,
};
use crate::forensics::inputs::{iso, parse_utc};

/// Pro feature of the sanctions-review pack.
pub const FEATURE_SANCTIONS: &str = "pro-ais-sanctions-pack";
/// Pro feature of the IUU-review pack.
pub const FEATURE_IUU: &str = "pro-ais-iuu-pack";
/// Pro feature of the subsea-asset pack.
pub const FEATURE_SUBSEA: &str = "pro-ais-subsea-pack";

/// Label of an indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Label {
    /// As broadcast or supplied.
    #[serde(rename = "REPORTED")]
    Reported,
    /// Computed by this method.
    #[serde(rename = "DERIVED")]
    Derived,
}

/// One indicator for review.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Indicator {
    pub code: &'static str,
    pub label: Label,
    pub text: String,
    /// The observations behind it (UTC times and what was seen).
    pub observations: Vec<String>,
}

fn ind(code: &'static str, label: Label, text: String, observations: Vec<String>) -> Indicator {
    Indicator {
        code,
        label,
        text,
        observations,
    }
}

fn gap_obs(x: &Gap) -> Vec<String> {
    vec![
        format!(
            "last report {} at {:.4},{:.4}",
            x.start_utc, x.start[0], x.start[1]
        ),
        format!(
            "next report {} at {:.4},{:.4}",
            x.end_utc, x.end[0], x.end[1]
        ),
    ]
}

fn enc_obs(x: &Encounter) -> Vec<String> {
    vec![format!(
        "{} and {} within {:.0} m from {} to {} near {:.4},{:.4}",
        x.a, x.b, x.min_separation_m, x.start_utc, x.end_utc, x.position[0], x.position[1]
    )]
}

/// Common head of every pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Head {
    pub format: &'static str,
    pub method: &'static str,
    pub title: String,
    pub evidence: &'static str,
    pub statement: &'static str,
    pub gap_note: &'static str,
    pub inputs: Vec<InputHash>,
    pub ais_rows: usize,
    pub ais_refused: usize,
    pub params: Params,
}

fn head(
    format: &'static str,
    method: &'static str,
    title: &str,
    inputs: Vec<InputHash>,
    t: &Tracks,
    p: &Params,
) -> Head {
    Head {
        format,
        method,
        title: title.into(),
        evidence: super::EVIDENCE,
        statement: super::STATEMENT,
        gap_note: super::GAP_NOTE,
        inputs,
        ais_rows: t.rows,
        ais_refused: t.refused,
        params: p.clone(),
    }
}

/// The vessels to report: those named (must be present) or all.
fn focus(t: &Tracks, named: &[String]) -> Result<BTreeSet<String>, String> {
    if named.is_empty() {
        return Ok(t.by_mmsi.keys().cloned().collect());
    }
    let missing: Vec<&str> = named
        .iter()
        .filter(|m| !t.by_mmsi.contains_key(*m))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "no position report for vessel(s) {}",
            missing.join(", ")
        ));
    }
    Ok(named.iter().cloned().collect())
}

fn load(
    base: &Path,
    ais: &str,
    stat: Option<&str>,
    p: &Params,
) -> Result<(Tracks, Vec<InputHash>), String> {
    p.validate()?;
    let t = load_tracks(base, ais, stat)?;
    let mut files = vec![ais];
    files.extend(stat);
    Ok((t, hash_inputs(base, &files)?))
}

/// One grid sample of a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub lat: f64,
    pub lon: f64,
    pub sog: Option<f64>,
    /// Navigational status of the latest report at or before `t`.
    pub nav: Option<u8>,
}

/// Grid samples of a track between its first and last report (none across a gap longer
/// than the interpolation limit).
pub fn samples(v: &[AisReport], p: &Params) -> Vec<Sample> {
    let (Some(f), Some(l)) = (v.first(), v.last()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut t = f.t;
    while t <= l.t {
        if let Some((lat, lon, sog)) = interp(v, t, p.interp_max_s) {
            let i = v.partition_point(|x| x.t <= t);
            let nav = i.checked_sub(1).and_then(|j| v[j].nav_status);
            out.push(Sample {
                t,
                lat,
                lon,
                sog,
                nav,
            });
        }
        t += p.grid_s;
    }
    out
}

fn hours(n: usize, p: &Params) -> f64 {
    r(n as f64 * p.grid_s / 3600.0, 2)
}

// ---------------------------------------------------------------------------------------------
// Sanctions review
// ---------------------------------------------------------------------------------------------

/// Sanctions-review pack.
pub mod sanctions {
    use super::*;

    /// Format tag.
    pub const FORMAT: &str = "kshana-pro-ais-sanctions-review/1";
    /// Method identifier.
    pub const METHOD_ID: &str = "kshana-pro/ais-dark-activity/v1";

    /// The request.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Request {
        pub title: String,
        pub ais: String,
        /// Optional static-data CSV.
        #[serde(default, rename = "static")]
        pub static_data: Option<String>,
        /// Vessels to review (MMSI); empty means every vessel in the file.
        #[serde(default)]
        pub vessels: Vec<String>,
        /// Areas of interest the user declares (e.g. a port approach or an anchorage).
        #[serde(default)]
        pub areas: Vec<Area>,
        /// A gap end within this distance of an area counts as near it (m).
        #[serde(default = "d_near")]
        pub near_area_m: f64,
        /// Ports the user declares, for concealed port-call candidates.
        #[serde(default)]
        pub ports: Vec<Area>,
        /// Service speed for reconstructing dark periods (kn).
        #[serde(default = "d_service")]
        pub service_speed_kn: f64,
        /// Shortest port call that counts (h).
        #[serde(default = "d_stay")]
        pub min_port_stay_h: f64,
        /// Period of the position-trust score (h).
        #[serde(default = "d_period")]
        pub trust_period_h: f64,
        #[serde(default)]
        pub params: Params,
    }

    fn d_near() -> f64 {
        20_000.0
    }
    pub(crate) fn d_service() -> f64 {
        15.0
    }
    fn d_stay() -> f64 {
        6.0
    }
    pub(crate) fn d_period() -> f64 {
        24.0
    }

    /// Findings for one vessel.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Vessel {
        pub mmsi: String,
        pub reports: usize,
        pub first_utc: String,
        pub last_utc: String,
        pub gaps: Vec<Gap>,
        pub jumps: Vec<Jump>,
        pub collision: Option<Collision>,
        pub identity_changes: Vec<IdentityChange>,
        pub encounters: Vec<Encounter>,
        /// Explainable position-trust score per period.
        pub trust: Vec<Trust>,
        pub port_calls: Vec<PortCall>,
        pub indicators: Vec<Indicator>,
    }

    /// The pack.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Pack {
        #[serde(flatten)]
        pub head: Head,
        pub request: Request,
        pub vessels: Vec<Vessel>,
        /// Overlapping gaps of two vessels with a feasible meeting point.
        pub dark_sts: Vec<DarkSts>,
        /// IMO numbers seen under more than one MMSI.
        pub shared_imo: std::collections::BTreeMap<String, Vec<String>>,
        pub verdict: String,
        pub limits: Vec<&'static str>,
    }

    const LIMITS: &[&str] = &[
        "No sanctions list is bundled and no vessel is screened: the vessels reviewed are those the user names.",
        "An encounter is proximity at low speed for a sustained time; it does not show a transfer of cargo.",
        "Identity changes are as broadcast in static data; a broadcast identity can be wrong without intent.",
        "Areas must not cross the antimeridian.",
        "Port calls and dark ship-to-ship meetings are candidates the time budget allows, not findings that they happened.",
        "The position-trust score uses stated weights, not weights fitted to labelled cases.",
    ];

    /// Build the pack.
    pub fn run(q: &Request, base: &Path) -> Result<Pack, String> {
        for a in q.areas.iter().chain(&q.ports) {
            a.validate()?;
        }
        for (n, v) in [
            ("service_speed_kn", q.service_speed_kn),
            ("trust_period_h", q.trust_period_h),
        ] {
            if v.is_nan() || v <= 0.0 {
                return Err(format!("{n} must be positive"));
            }
        }
        if q.min_port_stay_h.is_nan() || q.min_port_stay_h < 0.0 {
            return Err("min_port_stay_h must be zero or positive".into());
        }
        let (t, inputs) = load(base, &q.ais, q.static_data.as_deref(), &q.params)?;
        let f = focus(&t, &q.vessels)?;
        let p = &q.params;
        let enc = encounters(&t, &f, p);
        let near = |lat: f64, lon: f64| {
            q.areas
                .iter()
                .find(|a| a.distance_m(lat, lon) <= q.near_area_m)
                .map(|a| a.name.clone())
        };
        let mut vessels = Vec::new();
        for m in &f {
            let v = &t.by_mmsi[m];
            let g = gaps(v, p);
            let j = jumps(v, p);
            let c = collision(v, p);
            let ids = t
                .statics
                .get(m)
                .map(|s| identity_changes(s))
                .unwrap_or_default();
            let e: Vec<Encounter> = enc
                .iter()
                .filter(|x| &x.a == m || &x.b == m)
                .cloned()
                .collect();
            let mut inds = Vec::new();
            for x in &g {
                let area = near(x.start[0], x.start[1]).or_else(|| near(x.end[0], x.end[1]));
                inds.push(ind(
                    "gap",
                    Label::Derived,
                    format!(
                        "no report for {:.1} h from {}; {:.1} km displacement ({:.1} kn implied){}",
                        x.duration_h,
                        x.start_utc,
                        x.displacement_km,
                        x.implied_speed_kn,
                        area.map(|a| format!("; starts or ends near area '{a}'"))
                            .unwrap_or_default()
                    ),
                    gap_obs(x),
                ));
            }
            for x in &e {
                let other = if &x.a == m { &x.b } else { &x.a };
                inds.push(ind(
                    "encounter",
                    Label::Derived,
                    format!(
                        "with {other} for {:.1} h from {} (closest {:.0} m)",
                        x.duration_h, x.start_utc, x.min_separation_m
                    ),
                    enc_obs(x),
                ));
            }
            if !j.is_empty() {
                inds.push(ind(
                    "jump",
                    Label::Derived,
                    format!(
                        "{} implied-speed jump(s) above {} kn; largest {:.0} kn",
                        j.len(),
                        p.max_speed_kn,
                        j.iter().map(|x| x.implied_speed_kn).fold(0.0, f64::max)
                    ),
                    j.iter()
                        .map(|x| {
                            format!(
                                "{}: {:.4},{:.4} to {:.4},{:.4} ({:.0} kn)",
                                x.utc, x.from[0], x.from[1], x.to[0], x.to[1], x.implied_speed_kn
                            )
                        })
                        .collect(),
                ));
            }
            if let Some(c) = &c {
                inds.push(ind(
                    "mmsi-collision",
                    Label::Derived,
                    format!(
                        "{} away-and-back alternations from {}: consistent with two transmitters on one MMSI",
                        c.alternations, c.first_utc
                    ),
                    vec![format!(
                        "{} alternations among {} reports from {}",
                        c.alternations, c.reports, c.first_utc
                    )],
                ));
            }
            let pc = port_calls(&g, &q.ports, q.service_speed_kn, q.min_port_stay_h);
            for x in &pc {
                inds.push(ind(
                    "port-call-candidate",
                    Label::Derived,
                    format!(
                        "a call at '{}' of at least {} h fits in the gap from {} ({:.1} h slack at {} kn)",
                        x.port, q.min_port_stay_h, x.gap_start_utc, x.slack_h, q.service_speed_kn
                    ),
                    vec![
                        format!("last report {}", x.gap_start_utc),
                        format!("next report {}", x.gap_end_utc),
                    ],
                ));
            }
            for x in &ids {
                inds.push(ind(
                    "identity-change",
                    Label::Reported,
                    format!(
                        "{} changed from '{}' to '{}' at {}",
                        x.field, x.from, x.to, x.utc
                    ),
                    vec![format!("static record at {}", x.utc)],
                ));
            }
            vessels.push(Vessel {
                mmsi: m.clone(),
                reports: v.len(),
                first_utc: iso(v[0].t),
                last_utc: iso(v[v.len() - 1].t),
                gaps: g,
                jumps: j,
                collision: c,
                identity_changes: ids,
                encounters: e,
                trust: trust_scores(&t, m, q.trust_period_h * 3600.0, p),
                port_calls: pc,
                indicators: inds,
            });
        }
        let dsts = dark_sts(&t, &f, p, q.service_speed_kn);
        for d in &dsts {
            for v in vessels
                .iter_mut()
                .filter(|v| v.mmsi == d.a || v.mmsi == d.b)
            {
                let other = if v.mmsi == d.a { &d.b } else { &d.a };
                v.indicators.push(ind(
                    "dark-sts-candidate",
                    Label::Derived,
                    format!(
                        "silent together with {other} for {:.1} h from {}; a common meeting point near {:.3},{:.3} fits ({:.1} h slack)",
                        d.overlap_h, d.overlap_start_utc, d.point[0], d.point[1], d.slack_h
                    ),
                    d.observations.clone(),
                ));
            }
        }
        let shared = shared_imo(&t);
        let flagged = vessels.iter().filter(|v| !v.indicators.is_empty()).count();
        let verdict = format!(
            "{} vessel(s) reviewed; {} with at least one indicator for review; {} IMO number(s) under more than one MMSI. Not a sanctions determination.",
            vessels.len(),
            flagged,
            shared.len()
        );
        Ok(Pack {
            head: head(FORMAT, METHOD_ID, &q.title, inputs, &t, p),
            request: q.clone(),
            vessels,
            dark_sts: dsts,
            shared_imo: shared,
            verdict,
            limits: LIMITS.to_vec(),
        })
    }

    /// HTML page.
    pub fn html(p: &Pack) -> String {
        indicator_page(
            &p.head,
            &p.verdict,
            p.vessels.iter().map(|v| (v.mmsi.as_str(), &v.indicators)),
        )
    }
}

// ---------------------------------------------------------------------------------------------
// IUU review
// ---------------------------------------------------------------------------------------------

/// IUU-review pack.
pub mod iuu {
    use super::*;

    /// Format tag.
    pub const FORMAT: &str = "kshana-pro-ais-iuu-review/1";
    /// Method identifier.
    pub const METHOD_ID: &str = "kshana-pro/ais-zone-activity/v1";
    /// AIS navigational status "engaged in fishing".
    pub const NAV_FISHING: u8 = 7;

    /// The request.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Request {
        pub title: String,
        pub ais: String,
        #[serde(default, rename = "static")]
        pub static_data: Option<String>,
        #[serde(default)]
        pub vessels: Vec<String>,
        /// Zones the user declares (closed areas, protected areas, another state's waters).
        pub zones: Vec<Area>,
        /// Fishing-like speed band `[low, high]` (kn); a stated heuristic.
        #[serde(default = "d_band")]
        pub fishing_band_kn: [f64; 2],
        /// A gap end within this distance of a zone counts as near it (m).
        #[serde(default = "d_near")]
        pub near_zone_m: f64,
        /// Optional VMS export `timestamp,vessel_id,lat,lon[,sog_kn,cog_deg]`.
        #[serde(default)]
        pub vms: Option<String>,
        /// Declared VMS polling interval (s).
        #[serde(default = "d_poll")]
        pub vms_interval_s: f64,
        /// VMS vessel id to MMSI.
        #[serde(default)]
        pub vms_mmsi: std::collections::BTreeMap<String, String>,
        /// VMS-to-AIS distance above which they disagree (m).
        #[serde(default = "d_disagree")]
        pub vms_disagreement_m: f64,
        /// Neighbours within this distance inform the gap cause (m).
        #[serde(default = "d_radius")]
        pub neighbour_radius_m: f64,
        #[serde(default)]
        pub params: Params,
    }

    fn d_poll() -> f64 {
        3600.0
    }
    fn d_disagree() -> f64 {
        2000.0
    }
    fn d_radius() -> f64 {
        30_000.0
    }

    fn d_band() -> [f64; 2] {
        [0.5, 5.0]
    }
    fn d_near() -> f64 {
        20_000.0
    }

    /// Activity of one vessel in one zone.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct ZoneActivity {
        pub zone: String,
        pub kind: String,
        pub hours_inside: f64,
        pub hours_in_band: f64,
        pub hours_status_fishing: f64,
        pub first_inside_utc: Option<String>,
        /// Gaps that start or end inside the zone or within the near distance.
        pub gaps_near: usize,
    }

    /// Findings for one vessel.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Vessel {
        pub mmsi: String,
        pub reports: usize,
        pub zones: Vec<ZoneActivity>,
        pub gaps: Vec<Gap>,
        /// What the data say about each gap's cause (AIS off, interference, undetermined).
        pub gap_causes: Vec<GapClass>,
        pub encounters: Vec<Encounter>,
        pub indicators: Vec<Indicator>,
    }

    /// The pack.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Pack {
        #[serde(flatten)]
        pub head: Head,
        pub request: Request,
        pub vessels: Vec<Vessel>,
        /// VMS integrity indicators per VMS vessel.
        pub vms: Vec<VmsResult>,
        pub verdict: String,
        pub limits: Vec<&'static str>,
    }

    const LIMITS: &[&str] = &[
        "Zones are as the user declares them; Kshana bundles no maritime boundaries.",
        "The fishing-like speed band is a heuristic: transiting slowly or drifting also falls in it.",
        "Navigational status is as broadcast and is often left unchanged by crews.",
        "An encounter at sea is proximity at low speed; it does not show transshipment.",
        "Zones must not cross the antimeridian.",
        "VMS indicators and gap causes are what the data are consistent with, not findings of tampering or intent.",
    ];

    /// Build the pack.
    pub fn run(q: &Request, base: &Path) -> Result<Pack, String> {
        if q.zones.is_empty() {
            return Err("at least one [[zones]] entry is needed".into());
        }
        for z in &q.zones {
            z.validate()?;
        }
        let [lo, hi] = q.fishing_band_kn;
        if !(lo >= 0.0 && hi > lo) {
            return Err("fishing_band_kn must be [low, high] with 0 <= low < high".into());
        }
        if !(q.vms_interval_s > 0.0 && q.vms_disagreement_m > 0.0 && q.neighbour_radius_m > 0.0) {
            return Err(
                "vms_interval_s, vms_disagreement_m and neighbour_radius_m must be positive".into(),
            );
        }
        let (t, mut inputs) = load(base, &q.ais, q.static_data.as_deref(), &q.params)?;
        let f = focus(&t, &q.vessels)?;
        let p = &q.params;
        let vms_tracks = match &q.vms {
            Some(file) => {
                inputs.extend(hash_inputs(base, &[file.as_str()])?);
                vms::parse(&super::super::read_rel(base, file)?)
                    .map_err(|e| format!("{file}: {e}"))?
            }
            None => Default::default(),
        };
        let vms_results: Vec<VmsResult> = vms_tracks
            .iter()
            .map(|(id, v)| {
                vms::check(
                    id,
                    v,
                    q.vms_interval_s,
                    q.vms_mmsi.get(id).map(String::as_str),
                    &t,
                    q.vms_disagreement_m,
                    p,
                )
            })
            .collect();
        let enc = encounters(&t, &f, p);
        let mut vessels = Vec::new();
        for m in &f {
            let v = &t.by_mmsi[m];
            let s = samples(v, p);
            let g = gaps(v, p);
            let vpts = q
                .vms_mmsi
                .iter()
                .find(|(_, mm)| *mm == m)
                .and_then(|(id, _)| vms_tracks.get(id))
                .map(|v| vms::points(v));
            let causes: Vec<GapClass> = g
                .iter()
                .map(|x| classify_gap(x, &t, vpts.as_deref(), q.neighbour_radius_m, p))
                .collect();
            let mut zones = Vec::new();
            let mut inds = Vec::new();
            for z in &q.zones {
                let inside: Vec<&Sample> = s.iter().filter(|x| z.contains(x.lat, x.lon)).collect();
                let band = inside
                    .iter()
                    .filter(|x| x.sog.is_some_and(|v| v >= lo && v <= hi))
                    .count();
                let fish = inside.iter().filter(|x| x.nav == Some(NAV_FISHING)).count();
                let gn = g
                    .iter()
                    .filter(|x| {
                        z.distance_m(x.start[0], x.start[1]) <= q.near_zone_m
                            || z.distance_m(x.end[0], x.end[1]) <= q.near_zone_m
                    })
                    .count();
                let za = ZoneActivity {
                    zone: z.name.clone(),
                    kind: z.kind.clone(),
                    hours_inside: hours(inside.len(), p),
                    hours_in_band: hours(band, p),
                    hours_status_fishing: hours(fish, p),
                    first_inside_utc: inside.first().map(|x| iso(x.t)),
                    gaps_near: gn,
                };
                if za.hours_status_fishing > 0.0 {
                    inds.push(ind(
                        "status-fishing-in-zone",
                        Label::Reported,
                        format!(
                            "navigational status 'engaged in fishing' for {:.1} h inside '{}'",
                            za.hours_status_fishing, z.name
                        ),
                        inside
                            .iter()
                            .filter(|x| x.nav == Some(NAV_FISHING))
                            .take(20)
                            .map(|x| format!("{} at {:.4},{:.4}", iso(x.t), x.lat, x.lon))
                            .collect(),
                    ));
                }
                if za.hours_in_band > 0.0 {
                    inds.push(ind(
                        "fishing-band-in-zone",
                        Label::Derived,
                        format!(
                            "{:.1} h at {lo}-{hi} kn inside '{}' (of {:.1} h inside)",
                            za.hours_in_band, z.name, za.hours_inside
                        ),
                        inside
                            .iter()
                            .filter(|x| x.sog.is_some_and(|v| v >= lo && v <= hi))
                            .take(20)
                            .map(|x| format!("{} at {:.1} kn", iso(x.t), x.sog.unwrap_or(0.0)))
                            .collect(),
                    ));
                }
                if gn > 0 {
                    inds.push(ind(
                        "gap-near-zone",
                        Label::Derived,
                        format!(
                            "{gn} gap(s) starting or ending within {:.0} km of '{}'",
                            q.near_zone_m / 1000.0,
                            z.name
                        ),
                        g.iter()
                            .filter(|x| {
                                z.distance_m(x.start[0], x.start[1]) <= q.near_zone_m
                                    || z.distance_m(x.end[0], x.end[1]) <= q.near_zone_m
                            })
                            .flat_map(gap_obs)
                            .collect(),
                    ));
                }
                zones.push(za);
            }
            let e: Vec<Encounter> = enc
                .iter()
                .filter(|x| &x.a == m || &x.b == m)
                .cloned()
                .collect();
            for x in &e {
                let other = if &x.a == m { &x.b } else { &x.a };
                let zn = q
                    .zones
                    .iter()
                    .find(|z| z.distance_m(x.position[0], x.position[1]) <= q.near_zone_m)
                    .map(|z| format!(" near '{}'", z.name))
                    .unwrap_or_default();
                inds.push(ind(
                    "encounter-at-sea",
                    Label::Derived,
                    format!(
                        "with {other} for {:.1} h from {}{zn}",
                        x.duration_h, x.start_utc
                    ),
                    enc_obs(x),
                ));
            }
            for c in &causes {
                inds.push(ind(
                    "gap-cause",
                    Label::Derived,
                    format!("gap from {} to {}: {}", c.start_utc, c.end_utc, c.meaning),
                    c.observations.clone(),
                ));
            }
            for r in vms_results
                .iter()
                .filter(|r| r.mmsi.as_deref() == Some(m.as_str()))
            {
                for i in &r.indicators {
                    inds.push(ind(
                        "vms",
                        Label::Derived,
                        format!("VMS {} {}: {}", r.vessel_id, i.code, i.text),
                        i.observations.clone(),
                    ));
                }
            }
            vessels.push(Vessel {
                mmsi: m.clone(),
                reports: v.len(),
                zones,
                gaps: g,
                gap_causes: causes,
                encounters: e,
                indicators: inds,
            });
        }
        let flagged = vessels.iter().filter(|v| !v.indicators.is_empty()).count();
        let verdict = format!(
            "{} vessel(s) reviewed against {} zone(s); {} with at least one indicator for review. Not a finding of illegal fishing.",
            vessels.len(),
            q.zones.len(),
            flagged
        );
        Ok(Pack {
            head: head(FORMAT, METHOD_ID, &q.title, inputs, &t, p),
            request: q.clone(),
            vessels,
            vms: vms_results,
            verdict,
            limits: LIMITS.to_vec(),
        })
    }

    /// HTML page.
    pub fn html(p: &Pack) -> String {
        indicator_page(
            &p.head,
            &p.verdict,
            p.vessels.iter().map(|v| (v.mmsi.as_str(), &v.indicators)),
        )
    }
}

// ---------------------------------------------------------------------------------------------
// Subsea assets
// ---------------------------------------------------------------------------------------------

/// Subsea-asset pack.
pub mod subsea {
    use super::*;

    /// Format tag.
    pub const FORMAT: &str = "kshana-pro-ais-subsea-asset/1";
    /// Method identifier.
    pub const METHOD_ID: &str = "kshana-pro/ais-corridor-activity/v1";
    /// AIS navigational status "at anchor".
    pub const NAV_ANCHOR: u8 = 1;
    /// AIS navigational status "moored".
    pub const NAV_MOORED: u8 = 5;

    /// A declared cable or pipeline: a polyline and a corridor half-width.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Asset {
        pub name: String,
        #[serde(default)]
        pub kind: String,
        /// Corridor half-width (m).
        pub buffer_m: f64,
        pub path: Vec<[f64; 2]>,
    }

    impl Asset {
        /// Distance (m) from the asset's route.
        pub fn distance_m(&self, lat: f64, lon: f64) -> f64 {
            self.path
                .windows(2)
                .map(|w| seg_dist_m(lat, lon, w[0], w[1]))
                .fold(f64::INFINITY, f64::min)
        }
        /// Inside the corridor.
        pub fn contains(&self, lat: f64, lon: f64) -> bool {
            self.distance_m(lat, lon) <= self.buffer_m
        }
    }

    /// A reported fault to review the vessels around.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Incident {
        pub utc: String,
        pub lat: f64,
        pub lon: f64,
        /// Hours before the fault time to review.
        #[serde(default = "d_window")]
        pub window_h: f64,
        /// Radius around the fault position (m).
        #[serde(default = "d_radius")]
        pub radius_m: f64,
    }

    fn d_window() -> f64 {
        24.0
    }
    fn d_radius() -> f64 {
        5_000.0
    }

    /// The request.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Request {
        pub title: String,
        pub ais: String,
        #[serde(default, rename = "static")]
        pub static_data: Option<String>,
        #[serde(default)]
        pub vessels: Vec<String>,
        pub assets: Vec<Asset>,
        /// "Slow" inside a corridor: at or below this speed (kn).
        #[serde(default = "d_slow")]
        pub slow_kn: f64,
        /// "Moving" for the anchor-drag indicator: at or above this speed (kn).
        #[serde(default = "d_moving")]
        pub moving_kn: f64,
        #[serde(default)]
        pub incident: Option<Incident>,
        /// Anchor-drag likelihood at which to alert.
        #[serde(default = "d_drag_thr")]
        pub drag_threshold: f64,
        /// Sliding window of the anchor-drag features (s).
        #[serde(default = "d_drag_win")]
        pub drag_window_s: f64,
        /// Alert only within this distance of a corridor edge (m).
        #[serde(default = "d_alert_dist")]
        pub alert_distance_m: f64,
        /// Period of the position-trust score (h).
        #[serde(default = "super::sanctions::d_period")]
        pub trust_period_h: f64,
        #[serde(default)]
        pub params: Params,
    }

    fn d_drag_thr() -> f64 {
        0.7
    }
    fn d_drag_win() -> f64 {
        1800.0
    }
    fn d_alert_dist() -> f64 {
        10_000.0
    }

    fn d_slow() -> f64 {
        3.0
    }
    fn d_moving() -> f64 {
        0.5
    }

    /// Activity of one vessel in one corridor.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct CorridorActivity {
        pub asset: String,
        pub entries: usize,
        pub hours_inside: f64,
        pub hours_slow: f64,
        /// Reports with status at anchor or moored while at or above the moving speed.
        pub anchor_status_moving: usize,
        pub first_anchor_status_moving_utc: Option<String>,
        pub gaps_inside: usize,
    }

    /// One vessel around the incident.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Present {
        pub mmsi: String,
        pub min_distance_m: f64,
        pub closest_utc: String,
        pub hours_within_radius: f64,
        pub hours_slow_within_radius: f64,
        pub anchor_status_moving: usize,
        /// Gap covering the fault time.
        pub gap_over_fault: bool,
    }

    /// Findings for one vessel.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Vessel {
        pub mmsi: String,
        pub corridors: Vec<CorridorActivity>,
        /// Running anchor-drag likelihood and alert timing.
        pub drag: Drag,
        /// Explainable position-trust score per period.
        pub trust: Vec<Trust>,
        pub indicators: Vec<Indicator>,
    }

    /// The pack.
    #[derive(Clone, Debug, PartialEq, Serialize)]
    pub struct Pack {
        #[serde(flatten)]
        pub head: Head,
        pub request: Request,
        pub vessels: Vec<Vessel>,
        /// Vessels within the incident radius in the window, closest first.
        pub incident_presence: Vec<Present>,
        pub verdict: String,
        pub limits: Vec<&'static str>,
    }

    const LIMITS: &[&str] = &[
        "Asset routes and corridor widths are as the user declares them.",
        "Presence near an asset or a fault is not attribution of damage; vessels without AIS are not in this pack.",
        "Anchor or moored status while moving is an indicator of a dragged anchor or of a status left unchanged.",
        "Positions are the vessels' own broadcasts and inherit their errors and any manipulation.",
        "The anchor-drag likelihood uses stated coefficients, not coefficients fitted to labelled incidents.",
    ];

    /// Build the pack.
    pub fn run(q: &Request, base: &Path) -> Result<Pack, String> {
        if q.assets.is_empty() {
            return Err("at least one [[assets]] entry is needed".into());
        }
        for a in &q.assets {
            if a.path.len() < 2 || a.buffer_m.is_nan() || a.buffer_m <= 0.0 {
                return Err(format!(
                    "asset '{}' needs a path of at least 2 points and a positive buffer_m",
                    a.name
                ));
            }
        }
        if !(q.slow_kn > 0.0 && q.moving_kn >= 0.0) {
            return Err("slow_kn must be positive and moving_kn zero or positive".into());
        }
        let ok = q.drag_threshold > 0.0
            && q.drag_threshold < 1.0
            && q.drag_window_s > 0.0
            && q.alert_distance_m >= 0.0
            && q.trust_period_h > 0.0;
        if !ok {
            return Err("drag_threshold must be in (0, 1); drag_window_s and trust_period_h positive; alert_distance_m zero or positive".into());
        }
        let (t, inputs) = load(base, &q.ais, q.static_data.as_deref(), &q.params)?;
        let f = focus(&t, &q.vessels)?;
        let p = &q.params;
        let anchorish = |r: &AisReport| {
            matches!(r.nav_status, Some(NAV_ANCHOR | NAV_MOORED))
                && r.sog_kn.is_some_and(|s| s >= q.moving_kn)
        };
        let mut vessels = Vec::new();
        for m in &f {
            let v = &t.by_mmsi[m];
            let s = samples(v, p);
            let g = gaps(v, p);
            let mut cs = Vec::new();
            let mut inds = Vec::new();
            for a in &q.assets {
                let ins: Vec<bool> = s.iter().map(|x| a.contains(x.lat, x.lon)).collect();
                let entries = ins
                    .iter()
                    .enumerate()
                    .filter(|(i, &b)| b && (*i == 0 || !ins[i - 1]))
                    .count();
                let inside = ins.iter().filter(|&&b| b).count();
                let slow = s
                    .iter()
                    .zip(&ins)
                    .filter(|(x, &b)| b && x.sog.is_some_and(|v| v <= q.slow_kn))
                    .count();
                let drag: Vec<&AisReport> = v
                    .iter()
                    .filter(|r| anchorish(r) && a.contains(r.lat, r.lon))
                    .collect();
                let gi = g
                    .iter()
                    .filter(|x| {
                        a.contains(x.start[0], x.start[1]) || a.contains(x.end[0], x.end[1])
                    })
                    .count();
                let ca = CorridorActivity {
                    asset: a.name.clone(),
                    entries,
                    hours_inside: hours(inside, p),
                    hours_slow: hours(slow, p),
                    anchor_status_moving: drag.len(),
                    first_anchor_status_moving_utc: drag.first().map(|r| iso(r.t)),
                    gaps_inside: gi,
                };
                if ca.anchor_status_moving > 0 {
                    inds.push(ind(
                        "anchor-status-moving",
                        Label::Reported,
                        format!(
                            "{} report(s) with status at anchor or moored at {} kn or more inside the '{}' corridor, first at {}",
                            ca.anchor_status_moving,
                            q.moving_kn,
                            a.name,
                            ca.first_anchor_status_moving_utc.clone().unwrap_or_default()
                        ),
                        drag.iter()
                            .take(20)
                            .map(|r| {
                                format!(
                                    "{} status {} at {:.1} kn, {:.4},{:.4}",
                                    iso(r.t),
                                    r.nav_status.unwrap_or(0),
                                    r.sog_kn.unwrap_or(0.0),
                                    r.lat,
                                    r.lon
                                )
                            })
                            .collect(),
                    ));
                }
                if ca.hours_slow > 0.0 {
                    inds.push(ind(
                        "slow-in-corridor",
                        Label::Derived,
                        format!(
                            "{:.1} h at or below {} kn inside the '{}' corridor",
                            ca.hours_slow, q.slow_kn, a.name
                        ),
                        s.iter()
                            .zip(&ins)
                            .filter(|(x, &b)| b && x.sog.is_some_and(|v| v <= q.slow_kn))
                            .take(20)
                            .map(|(x, _)| format!("{} at {:.1} kn", iso(x.t), x.sog.unwrap_or(0.0)))
                            .collect(),
                    ));
                }
                if gi > 0 {
                    inds.push(ind(
                        "gap-in-corridor",
                        Label::Derived,
                        format!(
                            "{gi} gap(s) starting or ending inside the '{}' corridor",
                            a.name
                        ),
                        g.iter()
                            .filter(|x| {
                                a.contains(x.start[0], x.start[1]) || a.contains(x.end[0], x.end[1])
                            })
                            .flat_map(gap_obs)
                            .collect(),
                    ));
                }
                cs.push(ca);
            }
            let dr = anchor_drag(
                v,
                &q.assets,
                q.drag_window_s,
                q.drag_threshold,
                q.alert_distance_m,
                q.slow_kn,
            );
            if let Some(a) = &dr.alert_utc {
                inds.push(ind(
                    "anchor-drag-alert",
                    Label::Derived,
                    format!(
                        "anchor-drag likelihood {:.2} reached {} at {a} within {:.0} km of an asset{}",
                        dr.max_likelihood,
                        q.drag_threshold,
                        q.alert_distance_m / 1000.0,
                        dr.lead_h
                            .map(|h| format!("; {h:.2} h before slow entry into '{}'", dr.corridor.clone().unwrap_or_default()))
                            .unwrap_or_default()
                    ),
                    dr.observations.clone(),
                ));
            }
            vessels.push(Vessel {
                mmsi: m.clone(),
                corridors: cs,
                drag: dr,
                trust: trust_scores(&t, m, q.trust_period_h * 3600.0, p),
                indicators: inds,
            });
        }
        let presence = match &q.incident {
            Some(i) => incident(i, &t, &f, q, p)?,
            None => Vec::new(),
        };
        let flagged = vessels.iter().filter(|v| !v.indicators.is_empty()).count();
        let verdict = format!(
            "{} vessel(s) reviewed against {} asset corridor(s); {} with at least one indicator for review{}. Presence is not attribution.",
            vessels.len(),
            q.assets.len(),
            flagged,
            if q.incident.is_some() {
                format!("; {} vessel(s) within the incident radius in the window", presence.len())
            } else {
                String::new()
            }
        );
        Ok(Pack {
            head: head(FORMAT, METHOD_ID, &q.title, inputs, &t, p),
            request: q.clone(),
            vessels,
            incident_presence: presence,
            verdict,
            limits: LIMITS.to_vec(),
        })
    }

    fn incident(
        i: &Incident,
        t: &Tracks,
        f: &BTreeSet<String>,
        q: &Request,
        p: &Params,
    ) -> Result<Vec<Present>, String> {
        let tf = parse_utc(&i.utc)
            .ok_or_else(|| format!("incident.utc '{}' is not a UTC time", i.utc))?;
        if !(i.window_h > 0.0 && i.radius_m > 0.0) {
            return Err("incident.window_h and incident.radius_m must be positive".into());
        }
        let t0 = tf - i.window_h * 3600.0;
        let mut out = Vec::new();
        for m in f {
            let v = &t.by_mmsi[m];
            let s: Vec<Sample> = samples(v, p)
                .into_iter()
                .filter(|x| x.t >= t0 && x.t <= tf)
                .collect();
            let d: Vec<(f64, &Sample)> = s
                .iter()
                .map(|x| (crate::transport::dist_m(x.lat, x.lon, i.lat, i.lon), x))
                .collect();
            let within: Vec<&(f64, &Sample)> =
                d.iter().filter(|(dd, _)| *dd <= i.radius_m).collect();
            if within.is_empty() {
                continue;
            }
            let (md, ms) = within
                .iter()
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(a, b)| (*a, *b))
                .expect("non-empty");
            let slow = within
                .iter()
                .filter(|(_, x)| x.sog.is_some_and(|v| v <= q.slow_kn))
                .count();
            let drag = v
                .iter()
                .filter(|r| {
                    r.t >= t0
                        && r.t <= tf
                        && matches!(r.nav_status, Some(NAV_ANCHOR | NAV_MOORED))
                        && r.sog_kn.is_some_and(|s| s >= q.moving_kn)
                        && crate::transport::dist_m(r.lat, r.lon, i.lat, i.lon) <= i.radius_m
                })
                .count();
            let gap_over = gaps(v, p).iter().any(|g| g.t0 < tf && g.t1 > tf);
            out.push(Present {
                mmsi: m.clone(),
                min_distance_m: r(md, 0),
                closest_utc: iso(ms.t),
                hours_within_radius: hours(within.len(), p),
                hours_slow_within_radius: hours(slow, p),
                anchor_status_moving: drag,
                gap_over_fault: gap_over,
            });
        }
        out.sort_by(|a, b| a.min_distance_m.total_cmp(&b.min_distance_m));
        Ok(out)
    }

    /// HTML page.
    pub fn html(p: &Pack) -> String {
        let mut html = indicator_page(
            &p.head,
            &p.verdict,
            p.vessels.iter().map(|v| (v.mmsi.as_str(), &v.indicators)),
        );
        if !p.incident_presence.is_empty() {
            let rows: Vec<Vec<String>> = p
                .incident_presence
                .iter()
                .map(|x| {
                    vec![
                        x.mmsi.clone(),
                        format!("{:.0}", x.min_distance_m),
                        x.closest_utc.clone(),
                        format!("{:.1}", x.hours_within_radius),
                        format!("{:.1}", x.hours_slow_within_radius),
                        x.anchor_status_moving.to_string(),
                        x.gap_over_fault.to_string(),
                    ]
                })
                .collect();
            let extra = crate::transport::page(
                "",
                "",
                "",
                &[],
                &[(
                    "Vessels around the reported fault (presence, not attribution)".into(),
                    vec![
                        "MMSI",
                        "closest (m)",
                        "at",
                        "h within radius",
                        "h slow",
                        "anchor status moving",
                        "gap over fault time",
                    ],
                    rows,
                )],
            );
            if let (Some(a), Some(b)) = (extra.find("<h2>"), html.rfind("</body>")) {
                let tail = &extra[a..extra.rfind("</body>").unwrap_or(extra.len())];
                html.insert_str(b, tail);
            }
        }
        html
    }
}

fn indicator_page<'a>(
    h: &Head,
    verdict: &str,
    v: impl Iterator<Item = (&'a str, &'a Vec<Indicator>)>,
) -> String {
    let rows: Vec<Vec<String>> = v
        .flat_map(|(m, inds)| {
            inds.iter().map(move |i| {
                vec![
                    m.to_string(),
                    i.code.to_string(),
                    format!("{:?}", i.label).to_uppercase(),
                    i.text.clone(),
                ]
            })
        })
        .collect();
    crate::transport::page(
        &h.title,
        h.evidence,
        verdict,
        &[h.statement.to_string(), h.gap_note.to_string()],
        &[(
            "Indicators for review".into(),
            vec!["MMSI", "indicator", "label", "detail"],
            rows,
        )],
    )
}
