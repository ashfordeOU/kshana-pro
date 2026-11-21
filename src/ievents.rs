// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! One schema for PNT interference events across systems (GNSS bands, eLoran chains and
//! stations, R-Mode, LEO PNT): research 01 gap 23 and 02 gap 6. IALA notes there is "no
//! harmonised way of detecting and reporting PNT service interferences".
//!
//! * [`Event`] (`kshana-pnt-interference-event/1`): when, which systems, what kind, where
//!   (point + radius, or a named area), who observed it, measured effects, an optional source
//!   estimate, confidence, basis, evidence file hashes, reporter.
//! * [`validate`]: required fields, time order, coordinates, confidence range.
//! * [`merge`]: reports of the same event from different observers become one corroborated
//!   event (time overlap within a tolerance, overlapping location circles, a shared system);
//!   confidence combines as 1 − Π(1 − cᵢ), observers and evidence are united.
//! * [`coarsen`]: privacy for publication: positions snapped to a grid cell, radius widened to
//!   cover the cell, observer ids hashed.
//! * [`to_geojson`] / [`to_csv`] for maps and spreadsheets; [`from_area_event`] converts Kshana
//!   Pro area-trust events.
//!
//! Evidence: a data format and processing; no measurement claims.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const FORMAT: &str = "kshana-pnt-interference-event/1";

/// An affected system.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "system", rename_all = "snake_case")]
pub enum System {
    Gnss {
        band: String,
    },
    Eloran {
        chain: String,
        #[serde(default)]
        station: Option<String>,
    },
    RMode {
        band: String,
    },
    Leo {
        constellation: String,
    },
    Other {
        name: String,
    },
}

impl System {
    fn family(&self) -> &'static str {
        match self {
            System::Gnss { .. } => "gnss",
            System::Eloran { .. } => "eloran",
            System::RMode { .. } => "rmode",
            System::Leo { .. } => "leo",
            System::Other { .. } => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Jamming,
    Spoofing,
    Meaconing,
    Outage,
    StationFault,
    Unknown,
}

/// A place with an uncertainty radius.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub radius_m: f64,
}

/// A file the event rests on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub file: String,
    pub sha256: String,
}

/// One interference event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    #[serde(default = "fmt")]
    pub format: String,
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub systems: Vec<System>,
    pub kind: Kind,
    #[serde(default)]
    pub location: Option<Place>,
    #[serde(default)]
    pub area: Option<String>,
    pub observers: Vec<String>,
    /// Measured effects: e.g. `cn0_drop_db`, `js_db`, `timing_offset_ns`, `position_error_m`,
    /// `field_change_db`.
    #[serde(default)]
    pub metrics: std::collections::BTreeMap<String, f64>,
    #[serde(default)]
    pub source_estimate: Option<Place>,
    pub confidence: f64,
    pub basis: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub reporter: String,
    /// True when merged from several independent observers.
    #[serde(default)]
    pub corroborated: bool,
}

fn fmt() -> String {
    FORMAT.into()
}

fn secs(iso: &str) -> Result<i64, String> {
    // YYYY-MM-DDTHH:MM:SSZ (seconds resolution).
    let b = iso.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[10] != b'T' || !iso.ends_with('Z') {
        return Err(format!("{iso}: expected YYYY-MM-DDTHH:MM:SSZ"));
    }
    let n = |a: usize, l: usize| {
        iso[a..a + l]
            .parse::<i64>()
            .map_err(|_| format!("{iso}: bad number"))
    };
    let (y, mo, d, h, mi, s) = (
        n(0, 4)?,
        n(5, 2)?,
        n(8, 2)?,
        n(11, 2)?,
        n(14, 2)?,
        n(17, 2)?,
    );
    // Days from civil (Howard Hinnant).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok((era * 146_097 + doe - 719_468) * 86_400 + h * 3600 + mi * 60 + s)
}

/// Check one event; returns the problems.
pub fn validate(e: &Event) -> Vec<String> {
    let mut p = vec![];
    if e.format != FORMAT {
        p.push(format!("format must be {FORMAT}"));
    }
    if e.id.trim().is_empty() {
        p.push("id is empty".into());
    }
    match (secs(&e.start_utc), secs(&e.end_utc)) {
        (Ok(a), Ok(b)) if b < a => p.push("end_utc is before start_utc".into()),
        (Err(x), _) | (_, Err(x)) => p.push(x),
        _ => {}
    }
    if e.systems.is_empty() {
        p.push("no affected system".into());
    }
    if e.observers.is_empty() {
        p.push("no observer".into());
    }
    if !(0.0..=1.0).contains(&e.confidence) {
        p.push("confidence must be in 0..1".into());
    }
    for (n, pl) in [
        ("location", &e.location),
        ("source_estimate", &e.source_estimate),
    ] {
        if let Some(pl) = pl {
            if !(-90.0..=90.0).contains(&pl.lat_deg)
                || !(-180.0..=180.0).contains(&pl.lon_deg)
                || pl.radius_m < 0.0
            {
                p.push(format!("{n}: coordinates or radius out of range"));
            }
        }
    }
    if e.location.is_none() && e.area.is_none() {
        p.push("give a location or an area".into());
    }
    for ev in &e.evidence {
        if ev.sha256.len() != 64 || !ev.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            p.push(format!(
                "evidence {}: sha256 must be 64 hex digits",
                ev.file
            ));
        }
    }
    p
}

fn dist_m(a: &Place, b: &Place) -> f64 {
    let r = 6_371_000.0;
    let (p1, p2) = (a.lat_deg.to_radians(), b.lat_deg.to_radians());
    let dl = (b.lon_deg - a.lon_deg).to_radians();
    let h = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

fn same_event(a: &Event, b: &Event, tol_s: i64) -> bool {
    let (Ok(a0), Ok(a1), Ok(b0), Ok(b1)) = (
        secs(&a.start_utc),
        secs(&a.end_utc),
        secs(&b.start_utc),
        secs(&b.end_utc),
    ) else {
        return false;
    };
    let overlap = a0 <= b1 + tol_s && b0 <= a1 + tol_s;
    let place = match (&a.location, &b.location, &a.area, &b.area) {
        (Some(p), Some(q), _, _) => dist_m(p, q) <= p.radius_m + q.radius_m,
        (_, _, Some(x), Some(y)) => x == y,
        _ => false,
    };
    let shared = a
        .systems
        .iter()
        .any(|s| b.systems.iter().any(|t| t.family() == s.family()));
    overlap && place && shared
}

/// Merge reports of the same event (see the module text). `tol_s`: time tolerance.
pub fn merge(events: &[Event], tol_s: i64) -> Vec<Event> {
    let mut groups: Vec<Vec<usize>> = vec![];
    for (i, e) in events.iter().enumerate() {
        let hit = groups
            .iter_mut()
            .find(|g| g.iter().any(|&j| same_event(&events[j], e, tol_s)));
        match hit {
            Some(g) => g.push(i),
            None => groups.push(vec![i]),
        }
    }
    groups
        .into_iter()
        .map(|g| {
            let mut m = events[g[0]].clone();
            if g.len() == 1 {
                return m;
            }
            let es: Vec<&Event> = g.iter().map(|&i| &events[i]).collect();
            m.start_utc = es.iter().map(|e| e.start_utc.clone()).min().unwrap();
            m.end_utc = es.iter().map(|e| e.end_utc.clone()).max().unwrap();
            let mut sys: Vec<System> = es.iter().flat_map(|e| e.systems.clone()).collect();
            sys.sort();
            sys.dedup();
            m.systems = sys;
            let mut obs: Vec<String> = es.iter().flat_map(|e| e.observers.clone()).collect();
            obs.sort();
            obs.dedup();
            m.corroborated = obs.len() > 1;
            m.observers = obs;
            let mut ev: Vec<EvidenceRef> = es.iter().flat_map(|e| e.evidence.clone()).collect();
            ev.sort();
            ev.dedup();
            m.evidence = ev;
            m.confidence = 1.0 - es.iter().map(|e| 1.0 - e.confidence).product::<f64>();
            // Kind: the most specific agreed kind; disagreement → unknown.
            let kinds: Vec<Kind> = es
                .iter()
                .map(|e| e.kind)
                .filter(|k| *k != Kind::Unknown)
                .collect();
            m.kind = match kinds.first() {
                Some(k) if kinds.iter().all(|x| x == k) => *k,
                Some(_) => Kind::Unknown,
                None => Kind::Unknown,
            };
            // Location: the smallest-radius circle among the reports.
            m.location = es
                .iter()
                .filter_map(|e| e.location.clone())
                .min_by(|a, b| a.radius_m.total_cmp(&b.radius_m));
            for e in &es {
                for (k, v) in &e.metrics {
                    let cur = m.metrics.entry(k.clone()).or_insert(*v);
                    if v.abs() > cur.abs() {
                        *cur = *v;
                    }
                }
            }
            m.basis = format!(
                "merged from {} reports: {}",
                es.len(),
                es.iter()
                    .map(|e| e.basis.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            );
            m.id = format!(
                "merged-{}",
                &hex::encode(Sha256::digest(
                    es.iter()
                        .map(|e| e.id.as_str())
                        .collect::<Vec<_>>()
                        .join("|")
                        .as_bytes()
                ))[..12]
            );
            m
        })
        .collect()
}

/// Privacy coarsening for publication: snap to a `cell_deg` grid, widen the radius to cover
/// the cell, hash observer ids.
pub fn coarsen(e: &Event, cell_deg: f64) -> Event {
    let mut c = e.clone();
    let snap = |p: &Place| {
        let lat = ((p.lat_deg / cell_deg).floor() + 0.5) * cell_deg;
        let lon = ((p.lon_deg / cell_deg).floor() + 0.5) * cell_deg;
        let half_diag = 111_320.0 * cell_deg * std::f64::consts::FRAC_1_SQRT_2;
        Place {
            lat_deg: lat,
            lon_deg: lon,
            radius_m: p.radius_m.max(half_diag),
        }
    };
    c.location = e.location.as_ref().map(snap);
    c.source_estimate = e.source_estimate.as_ref().map(snap);
    c.observers = e
        .observers
        .iter()
        .map(|o| format!("obs-{}", &hex::encode(Sha256::digest(o.as_bytes()))[..10]))
        .collect();
    c
}

/// GeoJSON FeatureCollection (events with a location).
pub fn to_geojson(events: &[Event]) -> serde_json::Value {
    let feats: Vec<serde_json::Value> = events
        .iter()
        .filter_map(|e| {
            let l = e.location.as_ref()?;
            Some(serde_json::json!({
                "type": "Feature",
                "geometry": {"type": "Point", "coordinates": [l.lon_deg, l.lat_deg]},
                "properties": {"id": e.id, "kind": e.kind, "start_utc": e.start_utc, "end_utc": e.end_utc,
                    "radius_m": l.radius_m, "confidence": e.confidence, "corroborated": e.corroborated,
                    "observers": e.observers.len(), "systems": e.systems.iter().map(|s| s.family()).collect::<Vec<_>>()}
            }))
        })
        .collect();
    serde_json::json!({"type": "FeatureCollection", "features": feats})
}

/// CSV, one row per event.
pub fn to_csv(events: &[Event]) -> String {
    let mut s = String::from("id,start_utc,end_utc,kind,systems,lat_deg,lon_deg,radius_m,confidence,corroborated,observers\n");
    for e in events {
        let (la, lo, r) = e
            .location
            .as_ref()
            .map(|l| {
                (
                    l.lat_deg.to_string(),
                    l.lon_deg.to_string(),
                    l.radius_m.to_string(),
                )
            })
            .unwrap_or_default();
        s += &format!(
            "{},{},{},{:?},{},{},{},{},{:.3},{},{}\n",
            e.id,
            e.start_utc,
            e.end_utc,
            e.kind,
            e.systems
                .iter()
                .map(|x| x.family())
                .collect::<Vec<_>>()
                .join("+"),
            la,
            lo,
            r,
            e.confidence,
            e.corroborated,
            e.observers.len()
        );
    }
    s
}

/// A Kshana Pro area-trust event as an interference event (kind unknown: area trust says
/// "degraded / untrusted", not why). `epoch_unix`: Unix time of g = 0 (GPS time ignored at
/// second resolution for reporting).
pub fn from_area_event(a: &crate::area::Event, center: Option<Place>, epoch_unix: i64) -> Event {
    let iso = |g: i64| {
        let t = epoch_unix + g;
        let days = t.div_euclid(86_400);
        let sod = t.rem_euclid(86_400);
        // civil from days (Howard Hinnant)
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            if m <= 2 { y + 1 } else { y },
            m,
            d,
            sod / 3600,
            sod % 3600 / 60,
            sod % 60
        )
    };
    Event {
        format: FORMAT.into(),
        id: a.id.clone(),
        start_utc: iso(a.start_g),
        end_utc: iso(a.end_g),
        systems: vec![System::Gnss { band: "L1".into() }],
        kind: Kind::Unknown,
        location: center,
        area: Some(a.area.clone()),
        observers: a.sources.clone(),
        metrics: Default::default(),
        source_estimate: None,
        confidence: if a.corroborated { 0.9 } else { 0.6 },
        basis: format!(
            "Kshana Pro area trust ({}), worst state {:?}",
            a.basis, a.worst
        ),
        evidence: vec![],
        reporter: a.tenant.clone(),
        corroborated: a.corroborated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str, obs: &str, lat: f64, t0: &str, t1: &str, kind: Kind, c: f64) -> Event {
        Event {
            format: FORMAT.into(),
            id: id.into(),
            start_utc: t0.into(),
            end_utc: t1.into(),
            systems: vec![System::Gnss { band: "L1".into() }],
            kind,
            location: Some(Place {
                lat_deg: lat,
                lon_deg: 24.0,
                radius_m: 20_000.0,
            }),
            area: None,
            observers: vec![obs.into()],
            metrics: [("cn0_drop_db".to_string(), 10.0)].into_iter().collect(),
            source_estimate: None,
            confidence: c,
            basis: "C/N0 drop".into(),
            evidence: vec![],
            reporter: "r".into(),
            corroborated: false,
        }
    }

    #[test]
    fn validation_catches_bad_events() {
        let good = ev(
            "a",
            "s1",
            59.0,
            "2026-11-19T10:00:00Z",
            "2026-11-19T10:05:00Z",
            Kind::Jamming,
            0.7,
        );
        assert!(validate(&good).is_empty(), "{:?}", validate(&good));
        let mut b = good.clone();
        b.end_utc = "2026-11-19T09:00:00Z".into();
        b.confidence = 1.5;
        b.observers.clear();
        let p = validate(&b);
        assert!(p.len() >= 3, "{p:?}");
        assert_eq!(secs("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(secs("2026-11-19T00:00:00Z").unwrap(), 1_795_046_400);
    }

    #[test]
    fn independent_reports_merge_into_one_corroborated_event() {
        let a = ev(
            "a",
            "s1",
            59.00,
            "2026-11-19T10:00:00Z",
            "2026-11-19T10:05:00Z",
            Kind::Jamming,
            0.6,
        );
        let b = ev(
            "b",
            "s2",
            59.10,
            "2026-11-19T10:04:00Z",
            "2026-11-19T10:09:00Z",
            Kind::Jamming,
            0.5,
        );
        let far = ev(
            "c",
            "s3",
            61.0,
            "2026-11-19T10:00:00Z",
            "2026-11-19T10:05:00Z",
            Kind::Jamming,
            0.9,
        );
        let m = merge(&[a, b, far], 60);
        assert_eq!(m.len(), 2);
        let j = m.iter().find(|e| e.corroborated).unwrap();
        assert_eq!(j.observers, vec!["s1".to_string(), "s2".to_string()]);
        assert!((j.confidence - 0.8).abs() < 1e-12);
        assert_eq!(j.start_utc, "2026-11-19T10:00:00Z");
        assert_eq!(j.end_utc, "2026-11-19T10:09:00Z");
        assert_eq!(j.kind, Kind::Jamming);
        // Disagreeing kinds become unknown.
        let s = ev(
            "d",
            "s4",
            59.05,
            "2026-11-19T10:01:00Z",
            "2026-11-19T10:02:00Z",
            Kind::Spoofing,
            0.5,
        );
        let m = merge(
            &[
                ev(
                    "a",
                    "s1",
                    59.0,
                    "2026-11-19T10:00:00Z",
                    "2026-11-19T10:05:00Z",
                    Kind::Jamming,
                    0.6,
                ),
                s,
            ],
            0,
        );
        assert_eq!(m[0].kind, Kind::Unknown);
    }

    #[test]
    fn coarsening_hides_positions_and_observers() {
        let e = ev(
            "a",
            "station-tallinn-1",
            59.4372,
            "2026-11-19T10:00:00Z",
            "2026-11-19T10:05:00Z",
            Kind::Jamming,
            0.6,
        );
        let c = coarsen(&e, 0.5);
        let l = c.location.clone().unwrap();
        assert_eq!((l.lat_deg, l.lon_deg), (59.25, 24.25));
        assert!(l.radius_m >= 39_000.0);
        assert!(c.observers[0].starts_with("obs-") && !c.observers[0].contains("tallinn"));
        let g = to_geojson(std::slice::from_ref(&c));
        assert_eq!(g["features"][0]["geometry"]["coordinates"][1], 59.25);
        assert!(to_csv(&[c]).lines().count() == 2);
    }
}
