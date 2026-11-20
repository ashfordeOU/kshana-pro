// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Underwriting variables, feed and claims triage (B2805)
//!
//! **Feed** ([`build_feed`]): per vessel or aircraft and period, from its AIS or ADS-B track
//! (the forensics CSV) and the interference events supplied (`kshana-pnt-interference-event/1`
//! with a location):
//!
//! * RFI exposure: track hours inside an event's footprint while it was active, split by
//!   jamming and spoofing, and the events encountered;
//! * position-jump statistics: consecutive reports implying more than `max_speed_kn` over more
//!   than `min_jump_km`, per 1,000 track hours, and the largest jump;
//! * war-risk pricing inputs: hours inside the user's own listed areas (an area file whose
//!   `kind = "listed"` areas name their source; no market list ships with Kshana Pro);
//! * a cyber exposure indicator from spoofing encountered, position jumps and the declared
//!   controls (an indicator with its basis, not a cyber rating);
//! * data quality from track coverage.
//!
//! The feed is JSON Lines (`kshana-pro-underwriting-feed/1`): one header record, then one record
//! per subject; [`schema`] is its JSON Schema and [`sample`] builds a synthetic sample feed.
//!
//! **Claims triage** ([`triage`]): a claimed GNSS interference loss (who, when, where, the
//! claimed cause) set against interference events, signed evidence directories and a signed
//! hosted cause verdict, giving `corroborated`, `partly corroborated`, `conflicting evidence`
//! or `no Kshana evidence found`, with reasons and a handler checklist. A triage aid for the
//! handler, never a claims decision; absence of evidence here is not evidence against a claim.
//!
//! Evidence: **MODELLED**. Variables are counts and durations computed from the inputs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{distance_km, esc, fingerprint, html_table, iso, num, page, sig9, utc, InputRef};
use crate::briefings::areas;
use crate::ievents::{Event, Kind};

pub const FEED_FORMAT: &str = "kshana-pro-underwriting-feed/1";
pub const FEED_FILE: &str = "feed.jsonl";
pub const FEED_LABEL: &str = "MODELLED: variables counted from the tracks and events supplied; \
inputs to the user's own underwriting, not a rating, a price or an underwriting decision.";
pub const TRIAGE_FORMAT: &str = "kshana-pro-claims-triage/1";
pub const TRIAGE_FILE: &str = "triage.json";
pub const TRIAGE_LABEL: &str = "MODELLED: a triage aid that sets a claim against the evidence \
supplied; not a claims decision, a loss adjustment or a finding of fact.";

fn d_speed() -> f64 {
    50.0
}
fn d_jump() -> f64 {
    1.0
}
fn d_gap() -> f64 {
    30.0
}

/// A feed request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedRequest {
    pub title: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub tracks: String,
    /// `vessel_id` (AIS) or `aircraft_id` (ADS-B).
    pub id_column: String,
    #[serde(default)]
    pub events: Vec<String>,
    /// An area file (the briefing format); `kind = "listed"` areas count as listed.
    #[serde(default)]
    pub areas: Option<String>,
    #[serde(default = "d_speed")]
    pub max_speed_kn: f64,
    #[serde(default = "d_jump")]
    pub min_jump_km: f64,
    #[serde(default = "d_gap")]
    pub max_gap_min: f64,
    #[serde(default)]
    pub subject: Vec<SubjectSpec>,
}

/// Declared facts about a subject.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectSpec {
    pub id: String,
    #[serde(default)]
    pub declared_controls: Vec<String>,
}

/// One subject's variables (a feed record).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SubjectRecord {
    #[serde(rename = "type")]
    pub kind: String,
    pub subject_id: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub track_reports: usize,
    pub track_hours: f64,
    pub coverage: f64,
    pub data_quality: String,
    pub rfi_exposure_h: f64,
    pub jamming_exposure_h: f64,
    pub spoofing_exposure_h: f64,
    pub events_encountered: usize,
    pub event_ids: Vec<String>,
    pub position_jumps: usize,
    pub position_jumps_per_1000h: f64,
    pub max_jump_km: f64,
    pub listed_area_h: f64,
    pub area_hours: BTreeMap<String, f64>,
    pub declared_controls: Vec<String>,
    pub cyber_exposure_indicator: String,
    pub cyber_basis: String,
}

/// The header record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeedHeader {
    #[serde(rename = "type")]
    pub kind: String,
    pub format: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub max_speed_kn: f64,
    pub min_jump_km: f64,
    pub listed_area_sources: Vec<String>,
    pub inputs: Vec<InputRef>,
    pub subjects: usize,
    /// SHA-256 of the subject records (compact JSON array).
    pub fingerprint: String,
}

/// A built feed.
#[derive(Clone, Debug)]
pub struct Feed {
    pub header: FeedHeader,
    pub records: Vec<SubjectRecord>,
}

/// The tracks, events and areas a feed is built from, already read.
pub struct FeedData {
    pub tracks: String,
    pub events: Vec<Event>,
    pub areas: Vec<areas::Area>,
    pub inputs: Vec<InputRef>,
}

/// Read a feed request's inputs.
pub fn load(r: &FeedRequest, base: &Option<PathBuf>) -> Result<FeedData, String> {
    let mut inputs = vec![];
    let (b, rf) = super::read_input(base, &r.tracks, "tracks")?;
    inputs.push(rf);
    let events = crate::briefings::load_events(base, &r.events, &mut inputs)?;
    let areas = match &r.areas {
        Some(a) => {
            let (b, rf) = super::read_input(base, a, "areas")?;
            inputs.push(rf);
            areas::parse_areas(&String::from_utf8_lossy(&b))?
        }
        None => vec![],
    };
    Ok(FeedData {
        tracks: String::from_utf8_lossy(&b).into_owned(),
        events,
        areas,
        inputs,
    })
}

fn active(e: &Event, t: f64, lat: f64, lon: f64) -> bool {
    let (Some(a), Some(b)) = (utc("", &e.start_utc).ok(), utc("", &e.end_utc).ok()) else {
        return false;
    };
    match &e.location {
        Some(p) if t >= a && t <= b => {
            distance_km(lat, lon, p.lat_deg, p.lon_deg) <= p.radius_m / 1000.0
        }
        _ => false,
    }
}

/// Build the feed.
pub fn build_feed(r: &FeedRequest, d: FeedData) -> Result<Feed, String> {
    let (p0, p1) = (
        utc("period_start_utc", &r.period_start_utc)?,
        utc("period_end_utc", &r.period_end_utc)?,
    );
    if p1 <= p0 {
        return Err("the period is empty".into());
    }
    let tracks = crate::forensics::inputs::parse_tracks(&d.tracks, &r.id_column)
        .map_err(|e| format!("{}: {e}", r.tracks))?;
    let declared: BTreeMap<&str, &SubjectSpec> =
        r.subject.iter().map(|s| (s.id.as_str(), s)).collect();
    let max_gap = r.max_gap_min * 60.0;
    let period_h = (p1 - p0) / 3600.0;
    let mut records = vec![];
    for (id, reps) in &tracks.by_id {
        let reps: Vec<_> = reps.iter().filter(|x| x.t >= p0 && x.t <= p1).collect();
        if reps.is_empty() {
            continue;
        }
        let (mut hours, mut jam, mut spf, mut rfi, mut listed) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut area_h: BTreeMap<String, f64> = BTreeMap::new();
        let mut hit: std::collections::BTreeSet<String> = Default::default();
        let (mut jumps, mut max_jump) = (0usize, 0.0f64);
        for w in reps.windows(2) {
            let (a, b) = (w[0], w[1]);
            let dt = b.t - a.t;
            let dist = distance_km(a.lat, a.lon, b.lat, b.lon);
            if dt > 0.0 && dist > r.min_jump_km && dist / (dt / 3600.0) / 1.852 > r.max_speed_kn {
                jumps += 1;
                max_jump = max_jump.max(dist);
            }
            if dt <= 0.0 || dt > max_gap {
                continue;
            }
            hours += dt;
            let (tm, la, lo) = ((a.t + b.t) / 2.0, a.lat, a.lon);
            let mut any = false;
            for e in d.events.iter().filter(|e| active(e, tm, la, lo)) {
                any = true;
                hit.insert(e.id.clone());
                match e.kind {
                    Kind::Jamming => jam += dt,
                    Kind::Spoofing | Kind::Meaconing => spf += dt,
                    _ => {}
                }
            }
            if any {
                rfi += dt;
            }
            for ar in &d.areas {
                if areas::point_in_polygon(&ar.polygon, la, lo) {
                    *area_h.entry(ar.id.clone()).or_default() += dt / 3600.0;
                    if ar.kind == "listed" {
                        listed += dt;
                    }
                }
            }
        }
        let th = hours / 3600.0;
        let coverage = th / period_h;
        let controls = declared
            .get(id.as_str())
            .map(|s| s.declared_controls.clone())
            .unwrap_or_default();
        let auth = controls.iter().any(|c| {
            let c = c.to_ascii_lowercase();
            c.contains("osnma") || c.contains("authentic")
        });
        let (ind, basis) = match (spf > 0.0, jumps > 0, auth) {
            (true, _, false) => ("high", "spoofing encountered; no signal-authentication control declared"),
            (true, _, true) => ("elevated", "spoofing encountered; signal authentication declared"),
            (false, true, false) => ("elevated", "position jumps without a spoofing event on record; no signal-authentication control declared"),
            _ => ("low", "no spoofing encountered and no unexplained position jumps, or authentication declared"),
        };
        records.push(SubjectRecord {
            kind: "subject".into(),
            subject_id: id.clone(),
            period_start_utc: r.period_start_utc.clone(),
            period_end_utc: r.period_end_utc.clone(),
            track_reports: reps.len(),
            track_hours: sig9(th),
            coverage: sig9(coverage),
            data_quality: match coverage {
                c if c >= 0.5 => "sufficient",
                c if c >= 0.1 => "limited",
                _ => "insufficient",
            }
            .into(),
            rfi_exposure_h: sig9(rfi / 3600.0),
            jamming_exposure_h: sig9(jam / 3600.0),
            spoofing_exposure_h: sig9(spf / 3600.0),
            events_encountered: hit.len(),
            event_ids: hit.into_iter().collect(),
            position_jumps: jumps,
            position_jumps_per_1000h: if th > 0.0 {
                sig9(jumps as f64 / th * 1000.0)
            } else {
                0.0
            },
            max_jump_km: sig9(max_jump),
            listed_area_h: sig9(listed / 3600.0),
            area_hours: area_h.into_iter().map(|(k, v)| (k, sig9(v))).collect(),
            declared_controls: controls,
            cyber_exposure_indicator: ind.into(),
            cyber_basis: basis.into(),
        });
    }
    let mut sources: Vec<String> = d
        .areas
        .iter()
        .filter(|a| a.kind == "listed")
        .map(|a| format!("{}: {}", a.id, a.source))
        .collect();
    sources.sort();
    Ok(Feed {
        header: FeedHeader {
            kind: "header".into(),
            format: FEED_FORMAT.into(),
            label: FEED_LABEL.into(),
            statement: super::STATEMENT.into(),
            title: r.title.clone(),
            period_start_utc: r.period_start_utc.clone(),
            period_end_utc: r.period_end_utc.clone(),
            max_speed_kn: r.max_speed_kn,
            min_jump_km: r.min_jump_km,
            listed_area_sources: sources,
            inputs: d.inputs,
            subjects: records.len(),
            fingerprint: fingerprint(&records),
        },
        records,
    })
}

/// The feed as JSON Lines.
pub fn to_jsonl(f: &Feed) -> String {
    let mut s = serde_json::to_string(&f.header).unwrap_or_default() + "\n";
    for r in &f.records {
        s.push_str(&(serde_json::to_string(r).unwrap_or_default() + "\n"));
    }
    s
}

/// The feed's subject records as CSV (area hours left out; they are in the JSON Lines).
pub fn to_csv(f: &Feed) -> String {
    let mut s = String::from(
        "subject_id,track_hours,coverage,data_quality,rfi_exposure_h,jamming_exposure_h,spoofing_exposure_h,events_encountered,position_jumps,position_jumps_per_1000h,max_jump_km,listed_area_h,cyber_exposure_indicator\n",
    );
    for r in &f.records {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            r.subject_id,
            r.track_hours,
            r.coverage,
            r.data_quality,
            r.rfi_exposure_h,
            r.jamming_exposure_h,
            r.spoofing_exposure_h,
            r.events_encountered,
            r.position_jumps,
            r.position_jumps_per_1000h,
            r.max_jump_km,
            r.listed_area_h,
            r.cyber_exposure_indicator
        ));
    }
    s
}

/// The HTML page of a feed.
pub fn feed_html(f: &Feed) -> String {
    let rows: Vec<Vec<String>> = f
        .records
        .iter()
        .map(|r| {
            vec![
                r.subject_id.clone(),
                num(r.track_hours),
                r.data_quality.clone(),
                num(r.rfi_exposure_h),
                num(r.spoofing_exposure_h),
                r.events_encountered.to_string(),
                r.position_jumps.to_string(),
                num(r.listed_area_h),
                r.cyber_exposure_indicator.clone(),
            ]
        })
        .collect();
    let b = html_table(
        &[
            "Subject",
            "Track h",
            "Data quality",
            "RFI exposure h",
            "Spoofing h",
            "Events",
            "Jumps",
            "Listed-area h",
            "Cyber indicator",
        ],
        &rows,
    ) + &format!(
        "<p class=\"l\">Fingerprint {}</p>",
        esc(&f.header.fingerprint)
    );
    page(&f.header.title, &f.header.label, &b)
}

/// Write a feed directory: `feed.jsonl`, `feed.csv`, `schema.json`, `report.html`, the request.
pub fn write_feed(out: &Path, f: &Feed, request_src: &str) -> Result<(), String> {
    super::write_text(out, FEED_FILE, to_jsonl(f).as_bytes())?;
    super::write_text(out, "feed.csv", to_csv(f).as_bytes())?;
    super::write_json(out, "schema.json", &schema())?;
    super::write_text(out, "report.html", feed_html(f).as_bytes())?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}

/// The JSON Schema of a feed line.
pub fn schema() -> Value {
    let n = json!({"type": "number", "minimum": 0});
    let i = json!({"type": "integer", "minimum": 0});
    let s = json!({"type": "string"});
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": FEED_FORMAT,
        "title": "Kshana Pro underwriting feed line",
        "description": "One JSON object per line: a header record first, then one subject record per vessel or aircraft. Variables are counts and durations from the tracks and events supplied (MODELLED).",
        "oneOf": [
            {
                "type": "object",
                "required": ["type", "format", "label", "title", "period_start_utc", "period_end_utc", "inputs", "subjects", "fingerprint"],
                "properties": {
                    "type": {"const": "header"},
                    "format": {"const": FEED_FORMAT},
                    "label": {"type": "string", "pattern": "^MODELLED"},
                    "statement": s, "title": s,
                    "period_start_utc": {"type": "string", "format": "date-time"},
                    "period_end_utc": {"type": "string", "format": "date-time"},
                    "max_speed_kn": n, "min_jump_km": n,
                    "listed_area_sources": {"type": "array", "items": s},
                    "inputs": {"type": "array", "items": {"type": "object", "required": ["role", "file", "sha256", "bytes"]}},
                    "subjects": i,
                    "fingerprint": {"type": "string", "pattern": "^[0-9a-f]{64}$"}
                }
            },
            {
                "type": "object",
                "required": ["type", "subject_id", "track_hours", "rfi_exposure_h", "position_jumps", "listed_area_h", "cyber_exposure_indicator", "data_quality"],
                "properties": {
                    "type": {"const": "subject"},
                    "subject_id": s,
                    "period_start_utc": s, "period_end_utc": s,
                    "track_reports": i, "track_hours": n,
                    "coverage": n,
                    "data_quality": {"enum": ["sufficient", "limited", "insufficient"]},
                    "rfi_exposure_h": n, "jamming_exposure_h": n, "spoofing_exposure_h": n,
                    "events_encountered": i,
                    "event_ids": {"type": "array", "items": s},
                    "position_jumps": i, "position_jumps_per_1000h": n, "max_jump_km": n,
                    "listed_area_h": n,
                    "area_hours": {"type": "object", "additionalProperties": n},
                    "declared_controls": {"type": "array", "items": s},
                    "cyber_exposure_indicator": {"enum": ["low", "elevated", "high"]},
                    "cyber_basis": s
                }
            }
        ]
    })
}

/// A synthetic sample: two vessels over one day, one crossing a jamming footprint and showing a
/// spoofing jump inside a listed area. Returns (request TOML, tracks CSV, events JSON, areas
/// TOML) named `request.toml`, `tracks.csv`, `events.json`, `areas.toml`.
pub fn sample() -> (String, String, String, String) {
    let t0 = utc("", "2026-10-01T00:00:00Z").unwrap_or(0.0);
    let mut csv = String::from("timestamp,vessel_id,lat,lon,sog_kn\n");
    for k in 0..=144 {
        let t = t0 + k as f64 * 600.0;
        // SYN-A steams east along 34.0 N from 32.0 E at ~12 kn.
        let lon_a = 32.0 + k as f64 * 0.0381;
        let (la, lo) = if (60..63).contains(&k) {
            (35.2, lon_a) // three reports displaced ~130 km north: a spoofing jump
        } else {
            (34.0, lon_a)
        };
        csv.push_str(&format!("{},SYN-A,{la:.5},{lo:.5},12.0\n", iso(t)));
        // SYN-B stays far away in the west.
        csv.push_str(&format!(
            "{},SYN-B,40.0,{:.5},10.0\n",
            iso(t),
            -20.0 + k as f64 * 0.03
        ));
    }
    let ev = json!([
        {"id": "SYN-J1", "start_utc": "2026-10-01T06:00:00Z", "end_utc": "2026-10-01T12:00:00Z",
         "systems": [{"system": "gnss", "band": "L1"}], "kind": "jamming",
         "location": {"lat_deg": 34.0, "lon_deg": 33.5, "radius_m": 60000.0},
         "observers": ["synthetic-station"], "confidence": 0.9,
         "basis": "SYNTHETIC event for the sample feed", "reporter": "synthetic"},
        {"id": "SYN-S1", "start_utc": "2026-10-01T09:30:00Z", "end_utc": "2026-10-01T10:40:00Z",
         "systems": [{"system": "gnss", "band": "L1"}], "kind": "spoofing",
         "location": {"lat_deg": 34.0, "lon_deg": 34.3, "radius_m": 40000.0},
         "observers": ["synthetic-station"], "confidence": 0.8,
         "basis": "SYNTHETIC event for the sample feed", "reporter": "synthetic"}
    ]);
    let areas = "[[area]]\nid = \"syn-listed\"\nname = \"Synthetic listed area\"\nkind = \"listed\"\nsource = \"SYNTHETIC area for the sample feed; not a market listed area\"\nbbox = [33.0, 34.0, 36.0, 36.0]\n".to_string();
    let req = "title = \"Sample underwriting feed (SYNTHETIC)\"\nperiod_start_utc = \"2026-10-01T00:00:00Z\"\nperiod_end_utc = \"2026-10-02T00:00:00Z\"\ntracks = \"tracks.csv\"\nid_column = \"vessel_id\"\nevents = [\"events.json\"]\nareas = \"areas.toml\"\n\n[[subject]]\nid = \"SYN-A\"\ndeclared_controls = [\"multi-constellation\"]\n\n[[subject]]\nid = \"SYN-B\"\ndeclared_controls = [\"osnma\"]\n".to_string();
    (
        req,
        csv,
        serde_json::to_string_pretty(&ev).unwrap_or_default(),
        areas,
    )
}

// ----- claims triage --------------------------------------------------------------------------

fn d_tol() -> f64 {
    2.0
}

/// A claim to triage.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub claim_id: String,
    pub subject_id: String,
    pub start_utc: String,
    pub end_utc: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub radius_km: f64,
    /// `jamming`, `spoofing` or `outage`.
    pub claimed_cause: String,
    #[serde(default = "d_tol")]
    pub time_tolerance_h: f64,
}

/// A triage request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriageRequest {
    pub claim: Claim,
    #[serde(default)]
    pub events: Vec<String>,
    /// Signed evidence directories (forensics, dispute, exposure and the like).
    #[serde(default)]
    pub evidence: Vec<String>,
    /// A signed hosted cause verdict (`kshana-pro verdict cause --out`).
    #[serde(default)]
    pub cause_verdict: Option<String>,
}

/// One item of evidence considered.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriageItem {
    /// `event`, `signed evidence`, `cause verdict`.
    pub source: String,
    pub id: String,
    /// `supports`, `related`, `conflicts`, `set aside`, `content for the handler`.
    pub bearing: String,
    pub detail: String,
}

/// A triage result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriageResult {
    pub format: String,
    pub label: String,
    pub statement: String,
    pub claim: Claim,
    pub outcome: String,
    pub reasons: Vec<String>,
    pub items: Vec<TriageItem>,
    pub checklist: Vec<String>,
    pub inputs: Vec<InputRef>,
    pub fingerprint: String,
}

fn kind_matches(cause: &str, k: Kind) -> bool {
    match cause {
        "jamming" => k == Kind::Jamming,
        "spoofing" => matches!(k, Kind::Spoofing | Kind::Meaconing),
        _ => matches!(k, Kind::Outage | Kind::StationFault),
    }
}

/// Triage a claim. `trust`: signer keys for the evidence directories; `verdict_keys`: the
/// verdict service's public key(s), comma-separated.
pub fn triage(
    path: &Path,
    trust: &[[u8; 32]],
    verdict_keys: Option<&str>,
) -> Result<(TriageResult, String), String> {
    let (r, src, base): (TriageRequest, _, _) = super::read_request(path)?;
    let c = &r.claim;
    if !matches!(c.claimed_cause.as_str(), "jamming" | "spoofing" | "outage") {
        return Err("claimed_cause must be jamming, spoofing or outage".into());
    }
    let (a, b) = (
        utc("claim start_utc", &c.start_utc)?,
        utc("claim end_utc", &c.end_utc)?,
    );
    let tol = c.time_tolerance_h * 3600.0;
    let mut inputs = vec![];
    let events = crate::briefings::load_events(&base, &r.events, &mut inputs)?;
    let mut items = vec![];
    let (mut support_strong, mut support_weak, mut conflict) = (false, false, false);
    for e in &events {
        let (Ok(s), Ok(t)) = (utc("", &e.start_utc), utc("", &e.end_utc)) else {
            continue;
        };
        if t < a - tol || s > b + tol {
            continue;
        }
        let near = match &e.location {
            Some(p) => {
                distance_km(c.lat_deg, c.lon_deg, p.lat_deg, p.lon_deg)
                    <= c.radius_km + p.radius_m / 1000.0
            }
            None => false,
        };
        if !near {
            continue;
        }
        let corroborated = e.corroborated || e.observers.len() >= 2;
        let same = kind_matches(&c.claimed_cause, e.kind);
        if same && corroborated {
            support_strong = true;
        } else {
            support_weak = true;
        }
        items.push(TriageItem {
            source: "event".into(),
            id: e.id.clone(),
            bearing: if same { "supports" } else { "related" }.into(),
            detail: format!(
                "{:?} {} to {}, {} observer(s), confidence {}",
                e.kind,
                e.start_utc,
                e.end_utc,
                e.observers.len(),
                e.confidence
            )
            .to_ascii_lowercase(),
        });
    }
    for d in &r.evidence {
        let chk = crate::exposure::check(&super::resolve(&base, d), d, trust);
        let ok = chk.signature_valid && chk.signer_trusted != Some(false);
        if ok {
            support_weak = true;
        }
        items.push(TriageItem {
            source: "signed evidence".into(),
            id: d.clone(),
            bearing: if ok {
                "content for the handler"
            } else {
                "set aside"
            }
            .into(),
            detail: if ok {
                format!(
                    "signature verifies (signer {}); read its findings",
                    chk.signer.unwrap_or_default()
                )
            } else {
                format!("signature does not verify: {}", chk.problems.join("; "))
            },
        });
    }
    if let Some(vf) = &r.cause_verdict {
        let (bytes, rf) = super::read_input(&base, vf, "cause_verdict")?;
        inputs.push(rf);
        let v: crate::remote_verdict::SignedVerdict =
            serde_json::from_slice(&bytes).map_err(|e| format!("{vf}: {e}"))?;
        let problems = match verdict_keys {
            Some(k) => crate::remote_verdict::verify(&v, k),
            None => vec!["no verdict key given (--verdict-key)".into()],
        };
        let class = v.verdict["classification"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let bearing = if !problems.is_empty() || v.method != "cause" {
            "set aside"
        } else {
            match (c.claimed_cause.as_str(), class.as_str()) {
                ("jamming", "external_interference_jamming")
                | ("spoofing", "external_spoofing") => {
                    support_strong = true;
                    "supports"
                }
                (_, "self_reported_track_falsification") | (_, "equipment_fault") => {
                    conflict = true;
                    "conflicts"
                }
                (_, "insufficient_evidence") => "related",
                _ => {
                    support_weak = true;
                    "related"
                }
            }
        };
        items.push(TriageItem {
            source: "cause verdict".into(),
            id: vf.clone(),
            bearing: bearing.into(),
            detail: if problems.is_empty() {
                format!("classification {class} ({})", v.evidence_class)
            } else {
                problems.join("; ")
            },
        });
    }
    let (outcome, mut reasons) = if conflict {
        (
            "conflicting evidence",
            vec!["a verified cause verdict attributes the episode to another cause".to_string()],
        )
    } else if support_strong {
        (
            "corroborated",
            vec![
                "independent evidence of the claimed cause overlaps the claim in time and place"
                    .to_string(),
            ],
        )
    } else if support_weak {
        (
            "partly corroborated",
            vec![
                "related evidence found, but not independent evidence of the claimed cause"
                    .to_string(),
            ],
        )
    } else {
        (
            "no Kshana evidence found",
            vec![
                "nothing supplied overlaps the claim; this is not evidence against it".to_string(),
            ],
        )
    };
    if events.is_empty() {
        reasons.push("no interference events were supplied".into());
    }
    let mut checklist = vec![
        "Obtain the vessel's or aircraft's own GNSS receiver log for the window (C/N0, AGC, fix status).".to_string(),
        "Compare the reported track with an independent source (radar, coastal AIS, LRIT, MLAT).".into(),
        "Check whether neighbouring vessels or aircraft reported the same interference.".into(),
        "Confirm the policy's own wording and conditions with the policy holder's documents; this aid does not interpret wording.".into(),
    ];
    if c.claimed_cause == "spoofing" {
        checklist.push(
            "Look for position jumps, impossible speeds and time offsets in the window.".into(),
        );
    }
    let mut res = TriageResult {
        format: TRIAGE_FORMAT.into(),
        label: TRIAGE_LABEL.into(),
        statement: super::STATEMENT.into(),
        claim: c.clone(),
        outcome: outcome.into(),
        reasons,
        items,
        checklist,
        inputs,
        fingerprint: String::new(),
    };
    res.fingerprint = fingerprint(&res);
    Ok((res, src))
}

/// The triage HTML page.
pub fn triage_html(r: &TriageResult) -> String {
    let mut b = format!(
        "<p>Claim {} ({}), {} to {}, claimed cause {}.</p><p><b>Triage: {}</b></p><ul>",
        esc(&r.claim.claim_id),
        esc(&r.claim.subject_id),
        esc(&r.claim.start_utc),
        esc(&r.claim.end_utc),
        esc(&r.claim.claimed_cause),
        esc(&r.outcome)
    );
    for x in &r.reasons {
        b.push_str(&format!("<li>{}</li>", esc(x)));
    }
    b.push_str("</ul><h2>Evidence considered</h2>");
    b.push_str(&html_table(
        &["Source", "Id", "Bearing", "Detail"],
        &r.items
            .iter()
            .map(|i| {
                vec![
                    i.source.clone(),
                    i.id.clone(),
                    i.bearing.clone(),
                    i.detail.clone(),
                ]
            })
            .collect::<Vec<_>>(),
    ));
    b.push_str("<h2>Handler checklist</h2><ol>");
    for x in &r.checklist {
        b.push_str(&format!("<li>{}</li>", esc(x)));
    }
    b.push_str("</ol>");
    page(&format!("Claims triage {}", r.claim.claim_id), &r.label, &b)
}
