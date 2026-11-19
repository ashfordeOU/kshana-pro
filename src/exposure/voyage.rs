// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Equipment-resilience rating and voyage risk score for underwriters (W1119 C7; research 02
//! gaps 25, 26)
//!
//! P&I clubs and war-risk mutuals already give premium rebates for interference protection
//! (research 02 [I5][I6]), but the rebate rests on the kit fitted, not on how it performed.
//! This module gives two neutral, method-stated outputs:
//!
//! * **Equipment rating** ([`rate_equipment`]): per device, a 0–100 score from the evidence
//!   supplied: a Kshana receiver scorecard (`kshana-pro-receiver-scorecard/1`: its DHS RPCF
//!   level mapping and misleading-output totals), an OSNMA scorecard (C6,
//!   `kshana-pro-osnma-scorecard/1`), a PICS scorecard if one is present (any JSON whose
//!   clauses carry `met` / `not met` verdicts), or, without test evidence, the declared
//!   capabilities, capped so that declarations alone never reach the top band. The vessel's
//!   rating combines the primary receiver with the best independent alternative source
//!   ([`METHOD`] states the weights). Bands A to D.
//! * **Voyage risk** ([`voyage_risk`]): the route's exposure to historic interference (unified
//!   events, [`crate::ievents`]; area-console history converts with
//!   [`crate::ievents::from_area_event`]) along a corridor, weighted by kind, confidence and
//!   recency, combined with the equipment rating into a 0–100 score with its explanation: the
//!   contributing route legs and events, and the effect of the equipment.
//!
//! Both are signed by the CLI (`kshana-pro exposure equipment|voyage`). Wording: a rating of the
//! evidence supplied under a stated method; not an underwriting recommendation, premium advice,
//! a forecast or a certification of the equipment.
//!
//! Evidence: **MODELLED**. The weights are engineering choices stated in [`METHOD`] and checked
//! for monotonic, bounded behaviour by tests (`tests/insurer_voyage.rs`); they are not fitted to
//! claims or loss data.

use std::path::{Path, PathBuf};

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};

use super::SourceCheck;
use crate::forensics::inputs::{parse_utc, round};
use crate::ievents;

pub const EQUIPMENT_FORMAT: &str = "kshana-pro-equipment-rating/1";
pub const VOYAGE_FORMAT: &str = "kshana-pro-voyage-risk/1";
pub const EVIDENCE: &str = "MODELLED: weights are stated engineering choices, checked for bounded \
and monotonic behaviour; not fitted to claims or loss data.";
pub const STATEMENT: &str = "A rating of the evidence supplied under the method stated. Not an \
underwriting recommendation, premium advice, a forecast or a certification of any equipment.";
pub const METHOD: &str =
    "Device score 0-100: receiver scorecard → 25 × highest supported DHS RPCF \
level (0-4), −20 if any misleading epoch without warning; OSNMA scorecard → up to +10 × fraction \
of tests met; PICS → 100 × fraction of clauses met (replaces the receiver term when larger). \
Without test evidence: declared capabilities (multi-constellation 10, multi-frequency 10, \
spoofing detection 10, jamming detection 5, OSNMA 10, CRPA or null-steering antenna 15) capped at \
50. Vessel rating = 0.6 × primary GNSS + 0.4 × best independent alternative (eLoran, R-Mode, \
inertial with holdover, or a second diverse receiver), bands A ≥ 80, B ≥ 60, C ≥ 40, D < 40. \
Voyage exposure E = 1 − exp(−Σ wᵢ / 5), wᵢ = kind weight (spoofing or meaconing 1.0, jamming 0.6, \
outage 0.3, other 0.4) × confidence × exp(−age / 180 d) × share of the route within the event's \
reach; risk = 100 × E × (0.2 + 0.8 × (1 − rating/100)).";

/// A device in the equipment request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSpec {
    pub name: String,
    /// `primary_gnss`, `secondary_gnss`, `eloran`, `rmode`, `inertial`, `timing`.
    pub role: String,
    /// A signed receiver scorecard directory (holds `receiver-scorecard.json`).
    #[serde(default)]
    pub scorecard: Option<String>,
    /// A signed OSNMA scorecard directory (holds `osnma-scorecard.json`).
    #[serde(default)]
    pub osnma_scorecard: Option<String>,
    /// A PICS scorecard JSON file (or a directory holding `pics-scorecard.json`).
    #[serde(default)]
    pub pics: Option<String>,
    #[serde(default)]
    pub declared: Declared,
}

/// Declared capabilities (used only without test evidence).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Declared {
    pub multi_constellation: bool,
    pub multi_frequency: bool,
    pub spoof_detection: bool,
    pub jam_detection: bool,
    pub osnma: bool,
    pub crpa: bool,
    /// Holdover within the user's limit (h), for timing and inertial roles.
    pub holdover_h: f64,
}

/// The equipment request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentRequest {
    pub vessel: String,
    pub devices: Vec<DeviceSpec>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl EquipmentRequest {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r: Self =
            toml::from_str(&s).map_err(|e| format!("invalid equipment request: {e}"))?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok(r)
    }
    fn path(&self, p: &str) -> PathBuf {
        match &self.base_dir {
            Some(d) if Path::new(p).is_relative() => d.join(p),
            _ => PathBuf::from(p),
        }
    }
}

/// One device's rating.
#[derive(Clone, Debug, Serialize)]
pub struct DeviceRating {
    pub name: String,
    pub role: String,
    pub score: f64,
    /// `TESTED` (from a scorecard) or `DECLARED`.
    pub basis: String,
    pub terms: Vec<String>,
    pub sources: Vec<SourceCheck>,
}

/// The vessel's equipment rating.
#[derive(Clone, Debug, Serialize)]
pub struct EquipmentRating {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub method: String,
    pub vessel: String,
    pub devices: Vec<DeviceRating>,
    pub primary: Option<String>,
    pub alternative: Option<String>,
    pub rating: f64,
    pub band: String,
    pub limits: Vec<String>,
}

fn band(r: f64) -> &'static str {
    match r {
        r if r >= 80.0 => "A",
        r if r >= 60.0 => "B",
        r if r >= 40.0 => "C",
        _ => "D",
    }
}

fn read_json(dir_or_file: &Path, name: &str) -> Result<serde_json::Value, String> {
    let p = if dir_or_file.is_dir() {
        dir_or_file.join(name)
    } else {
        dir_or_file.to_path_buf()
    };
    let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_json::from_slice(&b).map_err(|e| format!("{}: {e}", p.display()))
}

/// Score from a receiver scorecard JSON: (score, term).
pub fn receiver_term(v: &serde_json::Value) -> Result<(f64, String), String> {
    if v["format"].as_str() != Some(crate::scorecards::FORMAT) {
        return Err(format!("not a {} scorecard", crate::scorecards::FORMAT));
    }
    let lvl = v["levels"]["highest_supported"]
        .as_u64()
        .unwrap_or(0)
        .min(4) as f64;
    let mis = v["totals"]["misleading_unwarned_epochs"]
        .as_u64()
        .unwrap_or(0);
    let s = (25.0 * lvl - if mis > 0 { 20.0 } else { 0.0 }).max(0.0);
    Ok((s, format!("receiver scorecard: RPCF level {lvl:.0} supported, {mis} misleading unwarned epochs → {s:.0}")))
}

/// OSNMA bonus from an OSNMA scorecard JSON.
pub fn osnma_term(v: &serde_json::Value) -> Result<(f64, String), String> {
    if v["format"].as_str() != Some(crate::osnma_conformance::FORMAT) {
        return Err(format!(
            "not a {} scorecard",
            crate::osnma_conformance::FORMAT
        ));
    }
    let (m, n) = (
        v["met"].as_f64().unwrap_or(0.0),
        v["not_met"].as_f64().unwrap_or(0.0),
    );
    let b = if m + n > 0.0 { 10.0 * m / (m + n) } else { 0.0 };
    Ok((
        b,
        format!(
            "OSNMA scorecard: {m:.0} of {:.0} tests met → +{b:.1}",
            m + n
        ),
    ))
}

/// PICS term: fraction of clauses met (any JSON with `clauses[].verdict`).
pub fn pics_term(v: &serde_json::Value) -> Result<(f64, String), String> {
    let cl = v["clauses"]
        .as_array()
        .ok_or("PICS scorecard has no clauses array")?;
    let met = cl
        .iter()
        .filter(|c| c["verdict"].as_str() == Some("met"))
        .count();
    let judged = cl
        .iter()
        .filter(|c| matches!(c["verdict"].as_str(), Some("met") | Some("not met")))
        .count();
    if judged == 0 {
        return Err("PICS scorecard has no judged clause".into());
    }
    let s = 100.0 * met as f64 / judged as f64;
    Ok((s, format!("PICS: {met} of {judged} clauses met → {s:.0}")))
}

/// Score from declared capabilities (capped at 50).
pub fn declared_term(d: &Declared, role: &str) -> (f64, String) {
    let mut s = 0.0;
    for (on, w) in [
        (d.multi_constellation, 10.0),
        (d.multi_frequency, 10.0),
        (d.spoof_detection, 10.0),
        (d.jam_detection, 5.0),
        (d.osnma, 10.0),
        (d.crpa, 15.0),
    ] {
        if on {
            s += w;
        }
    }
    if matches!(role, "inertial" | "timing" | "eloran" | "rmode") {
        s += (d.holdover_h / 24.0).min(1.0) * 20.0
            + if role == "eloran" || role == "rmode" {
                20.0
            } else {
                0.0
            };
    }
    let s = s.min(50.0);
    (
        s,
        format!("declared capabilities only → {s:.0} (capped at 50 without test evidence)"),
    )
}

/// Rate a vessel's equipment. `trust`: signer keys for the scorecard directories.
pub fn rate_equipment(r: &EquipmentRequest, trust: &[[u8; 32]]) -> Result<EquipmentRating, String> {
    let mut devices = vec![];
    for d in &r.devices {
        let (mut terms, mut sources) = (vec![], vec![]);
        let mut tested: Option<f64> = None;
        if let Some(p) = &d.scorecard {
            let path = r.path(p);
            if path.is_dir() {
                sources.push(super::check(&path, p, trust));
            }
            let (s, t) = receiver_term(&read_json(&path, "receiver-scorecard.json")?)?;
            tested = Some(s);
            terms.push(t);
        }
        if let Some(p) = &d.pics {
            let path = r.path(p);
            if path.is_dir() {
                sources.push(super::check(&path, p, trust));
            }
            let (s, t) = pics_term(&read_json(&path, "pics-scorecard.json")?)?;
            tested = Some(tested.unwrap_or(0.0).max(s));
            terms.push(t);
        }
        let mut score = match tested {
            Some(s) => s,
            None => {
                let (s, t) = declared_term(&d.declared, &d.role);
                terms.push(t);
                s
            }
        };
        if let Some(p) = &d.osnma_scorecard {
            let path = r.path(p);
            if path.is_dir() {
                sources.push(super::check(&path, p, trust));
            }
            let (b, t) = osnma_term(&read_json(&path, crate::osnma_conformance::RESULT_FILE)?)?;
            if tested.is_some() {
                score += b;
            }
            terms.push(t);
        }
        // Evidence whose signature fails counts for nothing.
        if sources
            .iter()
            .any(|s| !s.signature_valid || s.signer_trusted == Some(false))
        {
            terms.push("a scorecard's signature did not verify or its signer is not trusted: score set to 0".into());
            score = 0.0;
        }
        devices.push(DeviceRating {
            name: d.name.clone(),
            role: d.role.clone(),
            score: round(score.clamp(0.0, 100.0), 1),
            basis: if tested.is_some() {
                "TESTED"
            } else {
                "DECLARED"
            }
            .into(),
            terms,
            sources,
        });
    }
    let best = |roles: &[&str]| {
        devices
            .iter()
            .filter(|d| roles.contains(&d.role.as_str()))
            .max_by(|a, b| a.score.total_cmp(&b.score))
    };
    let primary = best(&["primary_gnss"]);
    let alt = best(&["eloran", "rmode", "inertial", "secondary_gnss", "timing"]);
    let rating = round(
        0.6 * primary.map(|d| d.score).unwrap_or(0.0) + 0.4 * alt.map(|d| d.score).unwrap_or(0.0),
        1,
    );
    let mut limits = vec![
        "The rating reflects the evidence supplied; equipment not covered by a scorecard is rated on declarations, capped.".to_string(),
        "Installation, crew procedures and the bridge alert system are not rated.".into(),
    ];
    if primary.is_none() {
        limits.push("No primary GNSS device was listed: its term is 0.".into());
    }
    if alt.is_none() {
        limits.push("No independent alternative source was listed: its term is 0.".into());
    }
    Ok(EquipmentRating {
        format: EQUIPMENT_FORMAT.into(),
        evidence: EVIDENCE.into(),
        statement: STATEMENT.into(),
        method: METHOD.into(),
        vessel: r.vessel.clone(),
        primary: primary.map(|d| d.name.clone()),
        alternative: alt.map(|d| d.name.clone()),
        band: band(rating).into(),
        rating,
        devices,
        limits,
    })
}

/// The voyage request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoyageRequest {
    pub title: String,
    pub vessel: String,
    /// Waypoints `[lat, lon]` in order.
    pub waypoints: Vec<[f64; 2]>,
    /// Voyage start (ISO-8601 UTC): the reference for event age.
    pub departure_utc: String,
    /// Unified event files (JSON arrays) with the interference history.
    pub events: Vec<String>,
    /// An equipment request to rate, or a rating already made (`equipment-rating.json`).
    #[serde(default)]
    pub equipment: Option<String>,
    /// Corridor half-width (km) around the route.
    #[serde(default = "default_corridor")]
    pub corridor_km: f64,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn default_corridor() -> f64 {
    25.0
}

impl VoyageRequest {
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r: Self = toml::from_str(&s).map_err(|e| format!("invalid voyage request: {e}"))?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok(r)
    }
    pub fn path(&self, p: &str) -> PathBuf {
        match &self.base_dir {
            Some(d) if Path::new(p).is_relative() => d.join(p),
            _ => PathBuf::from(p),
        }
    }
    /// Load the events and the equipment rating (`None` when no equipment was named).
    pub fn load(&self, trust: &[[u8; 32]]) -> Result<(Vec<ievents::Event>, Option<f64>), String> {
        let mut ev = vec![];
        for f in &self.events {
            let p = self.path(f);
            let b = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let v: Vec<ievents::Event> =
                serde_json::from_slice(&b).map_err(|e| format!("{f}: {e}"))?;
            ev.extend(v);
        }
        let rating = match &self.equipment {
            None => None,
            Some(e) if e.ends_with(".json") => {
                let v = read_json(&self.path(e), "equipment-rating.json")?;
                Some(v["rating"].as_f64().ok_or(format!("{e}: no rating"))?)
            }
            Some(e) => {
                Some(rate_equipment(&EquipmentRequest::from_file(&self.path(e))?, trust)?.rating)
            }
        };
        Ok((ev, rating))
    }
}

/// One contributing event.
#[derive(Clone, Debug, Serialize)]
pub struct Contribution {
    pub event_id: String,
    pub kind: ievents::Kind,
    pub start_utc: String,
    pub age_days: f64,
    pub route_share: f64,
    pub legs: Vec<usize>,
    pub weight: f64,
}

/// The voyage risk.
#[derive(Clone, Debug, Serialize)]
pub struct VoyageRisk {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub method: String,
    pub title: String,
    pub vessel: String,
    pub route_km: f64,
    pub corridor_km: f64,
    pub events_considered: usize,
    pub exposure: f64,
    pub equipment_rating: Option<f64>,
    pub vulnerability: f64,
    pub risk: f64,
    pub band: String,
    pub contributions: Vec<Contribution>,
    pub explanation: Vec<String>,
    pub limits: Vec<String>,
}

fn kind_w(k: ievents::Kind) -> f64 {
    match k {
        ievents::Kind::Spoofing | ievents::Kind::Meaconing => 1.0,
        ievents::Kind::Jamming => 0.6,
        ievents::Kind::Outage => 0.3,
        _ => 0.4,
    }
}

/// Points along the route every `step_m`, with the leg index.
fn sample(w: &[[f64; 2]], step_m: f64) -> (Vec<(f64, f64, usize)>, f64) {
    let mut pts = vec![];
    let mut total = 0.0;
    for (i, p) in w.windows(2).enumerate() {
        let d = haversine_m(p[0][0], p[0][1], p[1][0], p[1][1]);
        total += d;
        let n = (d / step_m).ceil().max(1.0) as usize;
        for k in 0..n {
            let f = k as f64 / n as f64;
            // Linear in lat/lon: adequate for legs of tens to hundreds of km at these scales.
            pts.push((
                p[0][0] + f * (p[1][0] - p[0][0]),
                p[0][1] + f * (p[1][1] - p[0][1]),
                i + 1,
            ));
        }
    }
    if let Some(l) = w.last() {
        pts.push((l[0], l[1], w.len().saturating_sub(1).max(1)));
    }
    (pts, total)
}

/// Score a voyage from its events and the equipment rating (`None`: rating unknown, treated as 0).
pub fn voyage_risk(
    r: &VoyageRequest,
    events: &[ievents::Event],
    rating: Option<f64>,
) -> Result<VoyageRisk, String> {
    if r.waypoints.len() < 2 {
        return Err("a voyage needs at least two waypoints".into());
    }
    let dep = parse_utc(&r.departure_utc).ok_or("departure_utc: not an ISO-8601 time")?;
    let (pts, total) = sample(&r.waypoints, 2_000.0);
    let corr = r.corridor_km * 1000.0;
    let mut contribs = vec![];
    let mut considered = 0;
    for e in events {
        let Some(pl) = &e.location else { continue };
        let Some(t) = parse_utc(&e.start_utc) else {
            continue;
        };
        if t > dep {
            continue; // history only: events after departure are not known at underwriting time
        }
        considered += 1;
        let reach = pl.radius_m + corr;
        let hit: Vec<&(f64, f64, usize)> = pts
            .iter()
            .filter(|p| haversine_m(p.0, p.1, pl.lat_deg, pl.lon_deg) <= reach)
            .collect();
        if hit.is_empty() {
            continue;
        }
        let share = hit.len() as f64 / pts.len() as f64;
        let age = (dep - t) / 86_400.0;
        let w = kind_w(e.kind) * e.confidence.clamp(0.0, 1.0) * (-age / 180.0).exp() * share;
        let mut legs: Vec<usize> = hit.iter().map(|p| p.2).collect();
        legs.dedup();
        contribs.push(Contribution {
            event_id: e.id.clone(),
            kind: e.kind,
            start_utc: e.start_utc.clone(),
            age_days: round(age, 1),
            route_share: round(share, 4),
            legs,
            weight: round(w, 5),
        });
    }
    contribs.sort_by(|a, b| b.weight.total_cmp(&a.weight));
    let sum: f64 = contribs.iter().map(|c| c.weight).sum();
    let exposure = 1.0 - (-sum / 5.0).exp();
    let rt = rating.unwrap_or(0.0).clamp(0.0, 100.0);
    let vul = 0.2 + 0.8 * (1.0 - rt / 100.0);
    let risk = round(100.0 * exposure * vul, 1);
    let band = match risk {
        x if x < 15.0 => "low",
        x if x < 35.0 => "moderate",
        x if x < 60.0 => "elevated",
        _ => "high",
    };
    let mut explanation = vec![format!(
        "Route {:.0} km; {} historic event(s) before departure; {} within {:.0} km of the route; exposure {:.3}",
        total / 1000.0,
        considered,
        contribs.len(),
        r.corridor_km,
        exposure
    )];
    for c in contribs.iter().take(5) {
        explanation.push(format!(
            "{} ({:?}, {:.0} days before departure) reaches {:.1} % of the route on leg(s) {:?}: weight {:.3}",
            c.event_id,
            c.kind,
            c.age_days,
            c.route_share * 100.0,
            c.legs,
            c.weight
        ));
    }
    explanation.push(match rating {
        Some(x) => format!("Equipment rating {x:.0} ({}) gives vulnerability {vul:.2} (1.0 for no protection, 0.2 floor)", band_of(x)),
        None => "No equipment rating supplied: vulnerability taken as 1.0".into(),
    });
    let mut limits = vec![
        "Exposure is computed from the historic events supplied; it is not a forecast of interference on the voyage.".to_string(),
        "Events report where interference was observed; areas with no observer show no history, which is not evidence of no interference.".into(),
        "Route legs are interpolated linearly in latitude and longitude between waypoints (2 km samples).".into(),
    ];
    if events.iter().any(|e| e.location.is_none()) {
        limits
            .push("Events with a named area but no location were not placed on the route.".into());
    }
    Ok(VoyageRisk {
        format: VOYAGE_FORMAT.into(),
        evidence: EVIDENCE.into(),
        statement: STATEMENT.into(),
        method: METHOD.into(),
        title: r.title.clone(),
        vessel: r.vessel.clone(),
        route_km: round(total / 1000.0, 1),
        corridor_km: r.corridor_km,
        events_considered: considered,
        exposure: round(exposure, 4),
        equipment_rating: rating,
        vulnerability: round(vul, 3),
        risk,
        band: band.into(),
        contributions: contribs,
        explanation,
        limits,
    })
}

fn band_of(r: f64) -> &'static str {
    band(r)
}

/// Write a result as `<name>` plus the request source.
pub fn write_json<T: Serialize>(
    dir: &Path,
    name: &str,
    v: &T,
    request_src: &str,
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(
        dir.join(name),
        serde_json::to_string_pretty(v).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("request.toml"), request_src).map_err(io)?;
    Ok(())
}
