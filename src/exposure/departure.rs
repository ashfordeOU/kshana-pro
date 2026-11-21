// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Departure go/no-go and pre-entry warning against verified zones (research 2026-11-20
//! gaps, B0806)
//!
//! Before sailing, and again before entering a zone where the ship leans hardest on GNSS
//! (a VTS area, a ship-to-ship transfer area, ice navigation, a pilotage district), the master
//! wants one advisory: what the recent interference record says about each zone on the
//! passage, how sure that is, and what to check. This module gives it from:
//!
//! * the **verified zones**: polygons with a type (`vts`, `sts`, `ice`, `pilotage`), who
//!   verified them and when; an unverified zone is refused;
//! * the **interference record**: unified events ([`crate::ievents`]; area-console events
//!   convert with [`crate::ievents::from_area_event`]), of which only the affected place
//!   (`location`, the circle where receivers were affected) is used. A `source_estimate` is
//!   never read;
//! * the **passage**: waypoints, departure time and planned speed; optionally the ship's
//!   equipment rating ([`super::voyage::rate_equipment`]).
//!
//! Per zone on the passage: the ETA at entry, the time to issue the pre-entry warning, the
//! events touching the zone with their recency weight, whether interference is active at the
//! decision time, a zone risk, a status (`PROCEED`, `PROCEED-WITH-CHECKS`,
//! `HOLD-AND-VERIFY`), a confidence, and the recommended checks. The departure advisory is the
//! worst zone status.
//!
//! An **advisory** under a stated method ([`METHOD`]): the decision is the master's, and the
//! absence of reports in a zone is not evidence of no interference. **MODELLED**: weights are
//! engineering choices exercised on a synthetic passage. Nothing here locates an emitter or
//! fuses sensors for navigation.

use std::path::Path;

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc, round, sha256_hex};
use crate::ievents;

/// Pro feature.
pub const FEATURE: &str = "pro-departure-advisory";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-departure-advisory/1";
/// Evidence label.
pub const EVIDENCE: &str = "MODELLED: weights are stated engineering choices exercised on a \
synthetic passage; not fitted to incident or loss data.";
/// What every advisory says about itself.
pub const STATEMENT: &str = "An advisory from the interference record supplied, under the \
method stated. The decision to sail or to enter a zone is the master's. Zones with no report \
show no history, which is not evidence of no interference.";
/// The method.
pub const METHOD: &str = "Event weight w = kind (spoofing or meaconing 1.0, jamming 0.6, \
outage 0.3, other 0.4) × confidence × exp(−age / recency_h) for events whose affected circle \
touches the zone (age from the event's end to the decision time; 0 while active). Zone \
exposure E = 1 − exp(−Σw). Zone risk = 100 × E × criticality (pilotage 1.0, sts 1.0, ice 0.9, \
vts 0.8) × (0.2 + 0.8 × (1 − rating/100)), rating 0 when none is given. Status: \
HOLD-AND-VERIFY when interference is active in the zone at the decision time and risk ≥ \
hold_risk, or risk ≥ 2 × hold_risk; PROCEED-WITH-CHECKS when risk ≥ checks_risk; else PROCEED. \
Confidence = 1 − Π(1 − confidence × exp(−age / recency_h)) over the zone's events; with none, \
0.5 if the record holds any event within recency_h of the decision time (the record is live), \
else 0.2.";

/// Zone type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneType {
    Vts,
    Sts,
    Ice,
    Pilotage,
}

impl ZoneType {
    fn criticality(self) -> f64 {
        match self {
            ZoneType::Pilotage | ZoneType::Sts => 1.0,
            ZoneType::Ice => 0.9,
            ZoneType::Vts => 0.8,
        }
    }

    fn checks(self) -> &'static [&'static str] {
        match self {
            ZoneType::Vts => &[
                "Report the ship's GNSS status to the VTS centre on entry and ask for any interference reports in the area.",
                "Keep the radar overlay on the ECDIS and confirm it registers with charted coastline and fixed marks.",
                "Cross-check position by radar range and bearing on charted objects at each reporting point.",
            ],
            ZoneType::Sts => &[
                "Agree with the other ship and the POAC how the approach is managed if GNSS-derived data (AIS, DP, ECDIS) degrade.",
                "Use radar and visual ranges for the approach and mooring; do not rely on GNSS-derived relative position.",
                "Confirm both ships' AIS positions against radar before the approach.",
            ],
            ZoneType::Ice => &[
                "Plan conservative routing with margins that do not depend on GNSS accuracy.",
                "Fix by radar on charted features at a shortened interval; keep the echo sounder running.",
                "Confirm the icebreaker or ice service's interference reports for the area.",
            ],
            ZoneType::Pilotage => &[
                "Brief the pilot on the ship's GNSS status and any interference reports; check the pilot's portable unit against ship's radar.",
                "Keep parallel indexing and radar fixes on charted objects throughout the passage.",
                "Agree abort points that are identifiable without GNSS.",
            ],
        }
    }
}

/// A verified zone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub name: String,
    #[serde(rename = "type")]
    pub zone_type: ZoneType,
    /// Polygon `[lat, lon]` vertices (closed implicitly).
    pub polygon: Vec<[f64; 2]>,
    pub verified_by: String,
    pub verified_utc: String,
}

/// Thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub recency_h: f64,
    pub checks_risk: f64,
    pub hold_risk: f64,
    /// Lead time of the pre-entry warning (minutes before entry).
    pub warn_lead_min: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            recency_h: 72.0,
            checks_risk: 15.0,
            hold_risk: 35.0,
            warn_lead_min: 60.0,
        }
    }
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub vessel: String,
    pub waypoints: Vec<[f64; 2]>,
    /// The decision time: planned departure (or now, for a pre-entry update).
    pub departure_utc: String,
    pub speed_kn: f64,
    pub zones: Vec<Zone>,
    /// Unified event files.
    pub events: Vec<String>,
    /// Equipment rating 0..100, if made.
    #[serde(default)]
    pub equipment_rating: Option<f64>,
    #[serde(default)]
    pub params: Params,
}

/// One contributing event.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Touch {
    pub event_id: String,
    pub kind: ievents::Kind,
    pub end_utc: String,
    pub active: bool,
    pub age_h: f64,
    pub weight: f64,
}

/// A zone on the passage.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ZoneAdvice {
    pub zone: String,
    pub zone_type: ZoneType,
    pub verified_by: String,
    pub verified_utc: String,
    pub on_passage: bool,
    pub entry_eta_utc: Option<String>,
    pub warn_at_utc: Option<String>,
    pub events: Vec<Touch>,
    pub active: bool,
    pub exposure: f64,
    pub risk: f64,
    pub status: &'static str,
    pub confidence: f64,
    pub checks: Vec<String>,
    pub explanation: String,
}

/// The advisory.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Advisory {
    pub format: &'static str,
    pub evidence: &'static str,
    pub statement: &'static str,
    pub method: &'static str,
    pub title: String,
    pub vessel: String,
    pub decision_utc: String,
    pub params: Params,
    pub inputs: Vec<(String, String)>,
    pub events_in_record: usize,
    pub source_estimates_ignored: usize,
    pub departure: &'static str,
    pub zones: Vec<ZoneAdvice>,
    pub limits: Vec<String>,
    pub verdict: String,
}

fn inside(poly: &[[f64; 2]], lat: f64, lon: f64) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a[0] > lat) != (b[0] > lat) && lon < (b[1] - a[1]) * (lat - a[0]) / (b[0] - a[0]) + a[1]
        {
            c = !c;
        }
    }
    c
}

/// Least distance (m) from a point to the polygon (0 inside), by edge sampling.
fn dist_to(poly: &[[f64; 2]], lat: f64, lon: f64) -> f64 {
    if inside(poly, lat, lon) {
        return 0.0;
    }
    let n = poly.len();
    let mut best = f64::INFINITY;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        for k in 0..=50 {
            let f = k as f64 / 50.0;
            let (x, y) = (a[0] + f * (b[0] - a[0]), a[1] + f * (b[1] - a[1]));
            best = best.min(haversine_m(lat, lon, x, y));
        }
    }
    best
}

fn kind_w(k: ievents::Kind) -> f64 {
    match k {
        ievents::Kind::Spoofing | ievents::Kind::Meaconing => 1.0,
        ievents::Kind::Jamming => 0.6,
        ievents::Kind::Outage => 0.3,
        _ => 0.4,
    }
}

/// Build the advisory. `base` resolves event file paths.
pub fn advise(q: &Request, base: &Path) -> Result<Advisory, String> {
    let p = &q.params;
    if q.waypoints.len() < 2 {
        return Err("a passage needs at least two waypoints".into());
    }
    if !(q.speed_kn > 0.0 && p.recency_h > 0.0 && p.warn_lead_min >= 0.0) {
        return Err("speed_kn and recency_h must be positive".into());
    }
    let dep = parse_utc(&q.departure_utc).ok_or("departure_utc: not a UTC time")?;
    for z in &q.zones {
        if z.verified_by.trim().is_empty() || parse_utc(&z.verified_utc).is_none() {
            return Err(format!(
                "zone '{}' is not verified: verified_by and verified_utc are required",
                z.name
            ));
        }
        if z.polygon.len() < 3 {
            return Err(format!("zone '{}': a polygon needs three vertices", z.name));
        }
    }
    let mut inputs = vec![];
    let mut events: Vec<ievents::Event> = vec![];
    for f in &q.events {
        let path = if Path::new(f).is_absolute() {
            f.into()
        } else {
            base.join(f)
        };
        let b = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        inputs.push((f.clone(), sha256_hex(&b)));
        let v: Vec<ievents::Event> = serde_json::from_slice(&b).map_err(|e| format!("{f}: {e}"))?;
        events.extend(v);
    }
    let src_est = events
        .iter()
        .filter(|e| e.source_estimate.is_some())
        .count();
    // Only what was known at the decision time.
    let known: Vec<&ievents::Event> = events
        .iter()
        .filter(|e| parse_utc(&e.start_utc).map(|t| t <= dep).unwrap_or(false))
        .collect();
    let live = known.iter().any(|e| {
        parse_utc(&e.end_utc)
            .map(|t| dep - t <= p.recency_h * 3600.0)
            .unwrap_or(false)
    });
    // Passage samples (every 500 m) with distance along.
    let mut pts: Vec<(f64, f64, f64)> = vec![];
    let mut along = 0.0;
    for w in q.waypoints.windows(2) {
        let d = haversine_m(w[0][0], w[0][1], w[1][0], w[1][1]);
        let n = (d / 500.0).ceil().max(1.0) as usize;
        for k in 0..n {
            let f = k as f64 / n as f64;
            pts.push((
                w[0][0] + f * (w[1][0] - w[0][0]),
                w[0][1] + f * (w[1][1] - w[0][1]),
                along + f * d,
            ));
        }
        along += d;
    }
    if let Some(l) = q.waypoints.last() {
        pts.push((l[0], l[1], along));
    }
    let v_ms = q.speed_kn * 1852.0 / 3600.0;
    let rating = q.equipment_rating.unwrap_or(0.0).clamp(0.0, 100.0);
    let vul = 0.2 + 0.8 * (1.0 - rating / 100.0);
    let mut zones = vec![];
    for z in &q.zones {
        let entry = pts.iter().find(|x| inside(&z.polygon, x.0, x.1));
        let eta = entry.map(|x| dep + x.2 / v_ms);
        let mut touches = vec![];
        for e in &known {
            let Some(pl) = &e.location else { continue };
            if dist_to(&z.polygon, pl.lat_deg, pl.lon_deg) > pl.radius_m {
                continue;
            }
            let end = parse_utc(&e.end_utc).unwrap_or(dep);
            let active = end >= dep;
            let age_h = ((dep - end) / 3600.0).max(0.0);
            touches.push(Touch {
                event_id: e.id.clone(),
                kind: e.kind,
                end_utc: e.end_utc.clone(),
                active,
                age_h: round(age_h, 1),
                weight: round(
                    kind_w(e.kind) * e.confidence.clamp(0.0, 1.0) * (-age_h / p.recency_h).exp(),
                    4,
                ),
            });
        }
        touches.sort_by(|a, b| b.weight.total_cmp(&a.weight));
        let sum: f64 = touches.iter().map(|t| t.weight).sum();
        let exposure = 1.0 - (-sum).exp();
        let risk = round(100.0 * exposure * z.zone_type.criticality() * vul, 1);
        let active = touches.iter().any(|t| t.active);
        let status = if (active && risk >= p.hold_risk) || risk >= 2.0 * p.hold_risk {
            "HOLD-AND-VERIFY"
        } else if risk >= p.checks_risk {
            "PROCEED-WITH-CHECKS"
        } else {
            "PROCEED"
        };
        let confidence = if touches.is_empty() {
            if live {
                0.5
            } else {
                0.2
            }
        } else {
            let conf_of = |id: &str| {
                known
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.confidence.clamp(0.0, 1.0))
                    .unwrap_or(0.0)
            };
            1.0 - touches.iter().fold(1.0, |a, t| {
                a * (1.0 - conf_of(&t.event_id) * (-t.age_h / p.recency_h).exp())
            })
        };
        let mut checks: Vec<String> = vec![];
        if status != "PROCEED" {
            checks.extend(z.zone_type.checks().iter().map(|s| s.to_string()));
            if touches
                .iter()
                .any(|t| matches!(t.kind, ievents::Kind::Spoofing | ievents::Kind::Meaconing))
            {
                checks.push("Spoofing is in the record: compare GNSS position, speed and course with radar fixes, log and gyro before entry; treat a GNSS position jump or a frozen position as an alarm.".into());
            }
            if touches.iter().any(|t| t.kind == ievents::Kind::Jamming) {
                checks.push("Jamming is in the record: confirm the alternatives the passage plan names (radar fixes, eLoran or R-Mode where available, dead reckoning) before entry.".into());
            }
        } else {
            checks.push("Standard watchkeeping; confirm the zone's latest interference reports at the pre-entry warning.".into());
        }
        let explanation = if touches.is_empty() {
            format!(
                "No event in the record touches {}; the record is {}.",
                z.name,
                if live {
                    "live (events within the recency window elsewhere)"
                } else {
                    "stale (no event within the recency window)"
                }
            )
        } else {
            format!(
                "{} event(s) touch {} ({} active at the decision time); exposure {:.2}, risk {:.0}.",
                touches.len(),
                z.name,
                touches.iter().filter(|t| t.active).count(),
                exposure,
                risk
            )
        };
        zones.push(ZoneAdvice {
            zone: z.name.clone(),
            zone_type: z.zone_type,
            verified_by: z.verified_by.clone(),
            verified_utc: z.verified_utc.clone(),
            on_passage: entry.is_some(),
            entry_eta_utc: eta.map(iso),
            warn_at_utc: eta.map(|t| iso((t - p.warn_lead_min * 60.0).max(dep))),
            events: touches,
            active,
            exposure: round(exposure, 3),
            risk,
            status,
            confidence: round(confidence, 2),
            checks,
            explanation,
        });
    }
    let rank = |s: &str| match s {
        "HOLD-AND-VERIFY" => 2,
        "PROCEED-WITH-CHECKS" => 1,
        _ => 0,
    };
    let departure = zones
        .iter()
        .filter(|z| z.on_passage)
        .map(|z| z.status)
        .max_by_key(|s| rank(s))
        .unwrap_or("PROCEED");
    let verdict = format!(
        "Departure advisory {departure}: {} zone(s) on the passage; {} ADVISORY, MODELLED",
        zones.iter().filter(|z| z.on_passage).count(),
        zones
            .iter()
            .filter(|z| z.on_passage && z.status != "PROCEED")
            .map(|z| format!("{} {}", z.zone, z.status))
            .collect::<Vec<_>>()
            .join("; ")
    );
    Ok(Advisory {
        format: FORMAT,
        evidence: EVIDENCE,
        statement: STATEMENT,
        method: METHOD,
        title: q.title.clone(),
        vessel: q.vessel.clone(),
        decision_utc: iso(dep),
        params: p.clone(),
        inputs,
        events_in_record: events.len(),
        source_estimates_ignored: src_est,
        departure,
        zones,
        limits: vec![
            "Only events that started before the decision time are used; this is not a forecast.".into(),
            "Events are placed by their affected circle; an event with only a named area is not placed.".into(),
            "Entry ETAs assume the planned speed throughout.".into(),
            "Source estimates in the record are never read.".into(),
        ],
        verdict,
    })
}

/// HTML page.
pub fn html(a: &Advisory) -> String {
    crate::transport::page(
        &a.title,
        a.evidence,
        &a.verdict,
        &[a.statement.to_string(), a.method.to_string()],
        &[
            (
                "Zones".into(),
                vec![
                    "zone",
                    "type",
                    "ETA at entry",
                    "warn at",
                    "risk",
                    "status",
                    "confidence",
                    "why",
                ],
                a.zones
                    .iter()
                    .map(|z| {
                        vec![
                            z.zone.clone(),
                            format!("{:?}", z.zone_type),
                            z.entry_eta_utc
                                .clone()
                                .unwrap_or_else(|| "not on passage".into()),
                            z.warn_at_utc.clone().unwrap_or_default(),
                            format!("{:.0}", z.risk),
                            z.status.to_string(),
                            format!("{:.2}", z.confidence),
                            z.explanation.clone(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Recommended checks".into(),
                vec!["zone", "check"],
                a.zones
                    .iter()
                    .flat_map(|z| {
                        z.checks
                            .iter()
                            .map(move |c| vec![z.zone.clone(), c.clone()])
                    })
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                a.limits.iter().map(|l| vec![l.clone()]).collect(),
            ),
        ],
    )
}

/// A SYNTHETIC passage: departure 2026-06-02T06:00Z at 12 kn through a VTS area, an STS area
/// (spoofing active at departure), a pilotage district (jamming 36 h old) and past an ice
/// zone off the route. Returns (events.json, request.toml).
pub fn synth() -> (String, String) {
    let ev = |id: &str, s: &str, e: &str, kind: &str, lat: f64, lon: f64, r: f64, c: f64| {
        format!(
            "{{\"id\":\"{id}\",\"start_utc\":\"{s}\",\"end_utc\":\"{e}\",\"systems\":[{{\"system\":\"gnss\",\"band\":\"L1\"}}],\"kind\":\"{kind}\",\"location\":{{\"lat_deg\":{lat},\"lon_deg\":{lon},\"radius_m\":{r}}},\"observers\":[\"SYNTH-OBS\"],\"confidence\":{c},\"basis\":\"SYNTHETIC\",\"source_estimate\":{{\"lat_deg\":0.0,\"lon_deg\":0.0,\"radius_m\":1.0}}}}"
        )
    };
    let events = format!(
        "[{},{},{},{}]\n",
        ev(
            "SYN-STS-1",
            "2026-06-02T02:00:00Z",
            "2026-06-02T09:00:00Z",
            "spoofing",
            55.30,
            12.40,
            10000.0,
            0.8
        ),
        ev(
            "SYN-STS-2",
            "2026-06-01T20:00:00Z",
            "2026-06-01T23:00:00Z",
            "spoofing",
            55.32,
            12.45,
            10000.0,
            0.7
        ),
        ev(
            "SYN-PIL-1",
            "2026-05-31T16:00:00Z",
            "2026-05-31T18:00:00Z",
            "jamming",
            55.60,
            12.70,
            8000.0,
            0.6
        ),
        ev(
            "SYN-FAR-1",
            "2026-06-01T12:00:00Z",
            "2026-06-01T13:00:00Z",
            "jamming",
            57.50,
            11.00,
            5000.0,
            0.9
        ),
    );
    let req = r#"title = "SYNTHETIC departure advisory"
vessel = "SYNTH VESSEL"
waypoints = [[55.00, 12.00], [55.30, 12.40], [55.60, 12.70], [55.80, 12.90]]
departure_utc = "2026-06-02T06:00:00Z"
speed_kn = 12.0
events = ["events.json"]

[[zones]]
name = "SYNTH VTS"
type = "vts"
polygon = [[55.05, 11.95], [55.05, 12.25], [55.20, 12.25], [55.20, 11.95]]
verified_by = "SYNTHETIC zone register"
verified_utc = "2026-05-01T00:00:00Z"

[[zones]]
name = "SYNTH STS area"
type = "sts"
polygon = [[55.25, 12.30], [55.25, 12.50], [55.35, 12.50], [55.35, 12.30]]
verified_by = "SYNTHETIC zone register"
verified_utc = "2026-05-01T00:00:00Z"

[[zones]]
name = "SYNTH pilotage"
type = "pilotage"
polygon = [[55.55, 12.60], [55.55, 12.80], [55.70, 12.80], [55.70, 12.60]]
verified_by = "SYNTHETIC zone register"
verified_utc = "2026-05-01T00:00:00Z"

[[zones]]
name = "SYNTH ice"
type = "ice"
polygon = [[57.00, 10.00], [57.00, 11.50], [58.00, 11.50], [58.00, 10.00]]
verified_by = "SYNTHETIC zone register"
verified_utc = "2026-05-01T00:00:00Z"
"#;
    (events, req.to_string())
}
