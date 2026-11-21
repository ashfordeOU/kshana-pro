// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Third-party interference product benchmark (research 2026-11-20 gaps, B1005)
//!
//! A buyer comparing interference-monitoring products wants them scored on the same events,
//! against truth, under rules fixed before anyone saw the results. This module gives that
//! scorecard:
//!
//! * a **protocol** registered before the data ([`Protocol`], hashed with [`protocol_sha256`];
//!   `kshana-pro bench register` is free): attribution guard, the affected-area cell size and
//!   optional pass bars. Scoring refuses a protocol that no longer hashes to the registered
//!   value;
//! * **truth**: the common events (on-air windows) and, per event, the positions of the victim
//!   receivers that were affected; the truth affected area is the set of cells holding them;
//! * **product reports**: per product (anonymised identifier), time-stamped detections, each
//!   with the cells the product says were affected. Report records accept no other field: a
//!   product's emitter-position estimate, bearing or region around a source is **refused, never
//!   scored**.
//!
//! Metrics per product: **detection rate** (events with at least one attributed report),
//! **latency** (first attributed report after onset: median and worst), **false reports per
//! hour** of clean time, and **affected-area accuracy** (cell precision, recall and IoU of the
//! product's affected cells against the truth cells, per event and pooled). Attribution and
//! false reports come from the open engine's referee (`kshana jammertest locate-score`,
//! [`kshana::jammertest::location::score`], used unchanged) on truth and reports stripped of any
//! position: with no point and no region it attributes by time and counts unattributed reports;
//! it is never asked to score a location.
//!
//! Fixtures name no vendor. Results name the products as their owner supplies them; publishing
//! a vendor's results is the owner's decision. **MODELLED** on the synthetic fixture; the
//! metric arithmetic is checked against hand-computed values.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use kshana::jammertest::location as referee;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::forensics::inputs::{iso, parse_utc, round, sha256_hex};

/// Pro feature of scoring.
pub const FEATURE: &str = "pro-interference-product-benchmark";
/// Formats.
pub const PROTOCOL_FORMAT: &str = "kshana-pro-product-bench-protocol/1";
pub const FORMAT: &str = "kshana-pro-product-benchmark/1";
/// Label.
pub const EVIDENCE: &str = "COMPUTED from the reports and truth as supplied, under the \
pre-registered protocol named by its hash. Affected-area accuracy is scored on victim-receiver \
cells only; no emitter position is accepted or scored. MODELLED on the synthetic fixture.";

/// The pre-registered protocol.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    pub title: String,
    /// When it was registered (UTC).
    pub registered_utc: String,
    /// A report within this many seconds of an event's window is attributed to it.
    pub guard_s: f64,
    /// Affected-area cell size (degrees).
    pub cell_deg: f64,
    /// Optional pass bars.
    #[serde(default)]
    pub min_detection_rate: Option<f64>,
    #[serde(default)]
    pub max_median_latency_s: Option<f64>,
    #[serde(default)]
    pub max_false_per_hour: Option<f64>,
    #[serde(default)]
    pub min_area_iou: Option<f64>,
}

/// SHA-256 of the canonical protocol JSON.
pub fn protocol_sha256(p: &Protocol) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_string(p).expect("serialises").as_bytes(),
    ))
}

/// Check a protocol.
pub fn validate_protocol(p: &Protocol) -> Result<(), String> {
    if !(p.guard_s >= 0.0 && p.cell_deg > 0.0 && p.cell_deg <= 1.0) {
        return Err("protocol: guard_s ≥ 0 and 0 < cell_deg ≤ 1".into());
    }
    if parse_utc(&p.registered_utc).is_none() {
        return Err("protocol: registered_utc is not a UTC time".into());
    }
    Ok(())
}

/// One truth event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TruthEvent {
    pub id: String,
    pub start_utc: String,
    pub end_utc: String,
    /// Positions `[lat, lon]` of victim receivers affected in the event.
    pub victims_affected: Vec<[f64; 2]>,
}

/// The truth file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Truth {
    pub observation_start_utc: String,
    pub observation_end_utc: String,
    pub events: Vec<TruthEvent>,
}

/// One product report. Only these fields are accepted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductReport {
    pub utc: String,
    /// Cells `[lat, lon]` (south-west corners on the protocol grid) the product says were
    /// affected.
    #[serde(default)]
    pub affected_cells: Vec<[f64; 2]>,
}

/// A product entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Product {
    pub id: String,
    /// JSON array of [`ProductReport`].
    pub reports: String,
}

/// The scoring request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub protocol: Protocol,
    pub protocol_sha256: String,
    pub truth: String,
    pub products: Vec<Product>,
}

/// Per-event result of a product.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EventResult {
    pub event: String,
    pub detected: bool,
    pub latency_s: Option<f64>,
    pub reports: usize,
    pub area_precision: Option<f64>,
    pub area_recall: Option<f64>,
    pub area_iou: Option<f64>,
}

/// A product's scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProductCard {
    pub product: String,
    pub reports_sha256: String,
    pub detection_rate: f64,
    pub median_latency_s: Option<f64>,
    pub worst_latency_s: Option<f64>,
    pub false_reports: usize,
    pub clean_hours: f64,
    pub false_per_hour: Option<f64>,
    pub area_precision: Option<f64>,
    pub area_recall: Option<f64>,
    pub area_iou: Option<f64>,
    pub events: Vec<EventResult>,
    /// Bar name → met (only bars the protocol sets).
    pub bars: BTreeMap<String, bool>,
}

/// The benchmark.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Benchmark {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub protocol: Protocol,
    pub protocol_sha256: String,
    pub truth_sha256: String,
    pub events: usize,
    pub products: Vec<ProductCard>,
    pub referee: &'static str,
    pub limits: Vec<String>,
    pub verdict: String,
}

fn cell_key(lat: f64, lon: f64, step: f64) -> (i64, i64) {
    ((lat / step).floor() as i64, (lon / step).floor() as i64)
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

fn set_scores(claim: &BTreeSet<(i64, i64)>, truth: &BTreeSet<(i64, i64)>) -> (f64, f64, f64) {
    let inter = claim.intersection(truth).count() as f64;
    let uni = claim.union(truth).count() as f64;
    let p = if claim.is_empty() {
        0.0
    } else {
        inter / claim.len() as f64
    };
    let r = if truth.is_empty() {
        0.0
    } else {
        inter / truth.len() as f64
    };
    (p, r, if uni > 0.0 { inter / uni } else { 0.0 })
}

/// Score. `base` resolves file paths.
pub fn score(q: &Request, base: &Path) -> Result<Benchmark, String> {
    let p = &q.protocol;
    validate_protocol(p)?;
    let h = protocol_sha256(p);
    if h != q.protocol_sha256.trim() {
        return Err(format!(
            "the protocol does not hash to the registered value (registered {}, now {h}): it was changed after registration",
            q.protocol_sha256
        ));
    }
    let tt = crate::timing::grid::read_text(base, &q.truth)?;
    let truth: Truth = serde_json::from_str(&tt).map_err(|e| format!("{}: {e}", q.truth))?;
    let t_obs0 = parse_utc(&truth.observation_start_utc).ok_or("observation_start_utc")?;
    let t_obs1 = parse_utc(&truth.observation_end_utc).ok_or("observation_end_utc")?;
    let mut evs = vec![];
    for e in &truth.events {
        let (a, b) = (
            parse_utc(&e.start_utc).ok_or(format!("event {}: start_utc", e.id))?,
            parse_utc(&e.end_utc).ok_or(format!("event {}: end_utc", e.id))?,
        );
        if b < a {
            return Err(format!("event {}: end before start", e.id));
        }
        let cells: BTreeSet<(i64, i64)> = e
            .victims_affected
            .iter()
            .map(|v| cell_key(v[0], v[1], p.cell_deg))
            .collect();
        evs.push((e.id.clone(), a, b, cells));
    }
    // The referee's truth, with no position.
    let rtruth = referee::Truth {
        schema: "kshana.referee-location-truth/1".into(),
        guard_s: p.guard_s,
        hold_s: 0.0,
        radii_m: vec![],
        levels: vec![],
        observation: referee::Span {
            start: t_obs0,
            end: t_obs1,
        },
        events: evs
            .iter()
            .map(|(id, a, b, _)| referee::Event {
                id: id.clone(),
                t_on: *a,
                t_off: *b,
                position: None,
                track: None,
            })
            .collect(),
    };
    let mut cards = vec![];
    let mut ids = BTreeSet::new();
    for prod in &q.products {
        if !ids.insert(prod.id.clone()) {
            return Err(format!("product '{}' is listed twice", prod.id));
        }
        let txt = crate::timing::grid::read_text(base, &prod.reports)?;
        let reps: Vec<ProductReport> = serde_json::from_str(&txt).map_err(|e| {
            format!(
                "product {}: {e} (a report carries only utc and affected_cells; emitter estimates are not accepted)",
                prod.id
            )
        })?;
        let mut rr = vec![];
        for r in &reps {
            let t = parse_utc(&r.utc).ok_or(format!("product {}: bad utc '{}'", prod.id, r.utc))?;
            rr.push(referee::Report {
                t,
                event: None,
                point: None,
                regions: vec![],
            });
        }
        // Time order, as the referee expects; keep the mapping to the reports.
        let mut order: Vec<usize> = (0..rr.len()).collect();
        order.sort_by(|a, b| rr[*a].t.total_cmp(&rr[*b].t));
        let sorted: Vec<referee::Report> = order.iter().map(|&i| rr[i].clone()).collect();
        let sc = referee::score(&rtruth, &sorted)?;
        let mut per: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for s in &sc.reports {
            if let Some(e) = &s.event {
                per.entry(e.clone()).or_default().push(order[s.index]);
            }
        }
        let mut events = vec![];
        let (mut all_claim, mut all_truth) = (BTreeSet::new(), BTreeSet::new());
        let mut lat = vec![];
        for (id, a, _, cells) in &evs {
            let idx = per.get(id).cloned().unwrap_or_default();
            let first = idx.iter().map(|&i| rr[i].t).fold(f64::INFINITY, f64::min);
            let claim: BTreeSet<(i64, i64)> = idx
                .iter()
                .flat_map(|&i| reps[i].affected_cells.iter())
                .map(|c| cell_key(c[0] + 1e-9, c[1] + 1e-9, p.cell_deg))
                .collect();
            let detected = !idx.is_empty();
            let latency = detected.then(|| (first - a).max(0.0));
            if let Some(l) = latency {
                lat.push(l);
            }
            let (pr, rc, iou) = set_scores(&claim, cells);
            for c in &claim {
                all_claim.insert((id.clone(), *c));
            }
            for c in cells {
                all_truth.insert((id.clone(), *c));
            }
            events.push(EventResult {
                event: id.clone(),
                detected,
                latency_s: latency.map(|x| round(x, 1)),
                reports: idx.len(),
                area_precision: detected.then(|| round(pr, 3)),
                area_recall: Some(round(rc, 3)),
                area_iou: Some(round(iou, 3)),
            });
        }
        let inter = all_claim.intersection(&all_truth).count() as f64;
        let uni = all_claim.union(&all_truth).count() as f64;
        let n_ev = evs.len().max(1) as f64;
        let detection_rate = events.iter().filter(|e| e.detected).count() as f64 / n_ev;
        let med = median(lat.clone());
        let fph = sc.false_locations.per_hour;
        let a_iou = (uni > 0.0).then(|| inter / uni);
        let mut bars = BTreeMap::new();
        if let Some(b) = p.min_detection_rate {
            bars.insert("min_detection_rate".into(), detection_rate >= b);
        }
        if let Some(b) = p.max_median_latency_s {
            bars.insert(
                "max_median_latency_s".into(),
                med.map(|m| m <= b).unwrap_or(false),
            );
        }
        if let Some(b) = p.max_false_per_hour {
            bars.insert(
                "max_false_per_hour".into(),
                fph.map(|f| f <= b).unwrap_or(true),
            );
        }
        if let Some(b) = p.min_area_iou {
            bars.insert(
                "min_area_iou".into(),
                a_iou.map(|x| x >= b).unwrap_or(false),
            );
        }
        cards.push(ProductCard {
            product: prod.id.clone(),
            reports_sha256: sha256_hex(txt.as_bytes()),
            detection_rate: round(detection_rate, 3),
            median_latency_s: med.map(|x| round(x, 1)),
            worst_latency_s: lat.iter().copied().reduce(f64::max).map(|x| round(x, 1)),
            false_reports: sc.false_locations.count,
            clean_hours: round(sc.false_locations.clean_s / 3600.0, 3),
            false_per_hour: fph.map(|x| round(x, 3)),
            area_precision: (!all_claim.is_empty())
                .then(|| round(inter / all_claim.len() as f64, 3)),
            area_recall: (!all_truth.is_empty()).then(|| round(inter / all_truth.len() as f64, 3)),
            area_iou: a_iou.map(|x| round(x, 3)),
            events,
            bars,
        });
    }
    let verdict = format!(
        "{} product(s) on {} common event(s) under protocol {} (COMPUTED; MODELLED on synthetic data where so labelled)",
        cards.len(),
        evs.len(),
        &h[..12]
    );
    Ok(Benchmark {
        format: FORMAT,
        evidence: EVIDENCE,
        title: q.title.clone(),
        protocol: p.clone(),
        protocol_sha256: h,
        truth_sha256: sha256_hex(tt.as_bytes()),
        events: evs.len(),
        products: cards,
        referee: "kshana jammertest locate-score (kshana::jammertest::location::score), unchanged, on truth and reports without any position: attribution by time window and unattributed reports per clean hour only",
        limits: vec![
            "Products are listed in request order; the scorecard does not rank them.".into(),
            "Affected-area accuracy is on the protocol's cells of victim receivers; a product's emitter estimate is not accepted and is never scored.".into(),
            "Latency is from the truth onset to the first attributed report time as the product stamps it.".into(),
            "A report inside the guard of two events is attributed to the earlier onset (the referee's rule).".into(),
        ],
        verdict,
    })
}

/// HTML page.
pub fn html(b: &Benchmark) -> String {
    let o = |x: Option<f64>| x.map(|v| format!("{v}")).unwrap_or_else(|| "-".into());
    crate::transport::page(
        &b.title,
        b.evidence,
        &b.verdict,
        &[
            format!(
                "Protocol {} registered {}; SHA-256 {}.",
                b.protocol.title, b.protocol.registered_utc, b.protocol_sha256
            ),
            format!("Referee: {}.", b.referee),
        ],
        &[
            (
                "Scorecard".into(),
                vec![
                    "product",
                    "detection rate",
                    "median latency (s)",
                    "worst latency (s)",
                    "false per hour",
                    "area precision",
                    "area recall",
                    "area IoU",
                    "bars",
                ],
                b.products
                    .iter()
                    .map(|c| {
                        vec![
                            c.product.clone(),
                            format!("{}", c.detection_rate),
                            o(c.median_latency_s),
                            o(c.worst_latency_s),
                            o(c.false_per_hour),
                            o(c.area_precision),
                            o(c.area_recall),
                            o(c.area_iou),
                            c.bars
                                .iter()
                                .map(|(k, v)| {
                                    format!("{k}: {}", if *v { "met" } else { "not met" })
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        ]
                    })
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                b.limits.iter().map(|l| vec![l.clone()]).collect(),
            ),
        ],
    )
}

/// A SYNTHETIC benchmark: three events over a two-hour observation, two anonymised products.
/// PRODUCT-A detects all three (latencies 20, 40, 60 s), one false report, exact cells on two
/// events; PRODUCT-B detects two (latencies 90, 150 s), no false report, wider cells.
/// Returns (truth.json, product-a.json, product-b.json, protocol TOML body).
pub fn synth() -> (String, String, String, String) {
    let t = |s: u32| iso(1_780_300_800.0 + s as f64);
    let truth = format!(
        r#"{{"observation_start_utc":"{}","observation_end_utc":"{}","events":[
{{"id":"E1","start_utc":"{}","end_utc":"{}","victims_affected":[[50.012,1.012],[50.013,1.031],[50.031,1.012]]}},
{{"id":"E2","start_utc":"{}","end_utc":"{}","victims_affected":[[50.212,1.212]]}},
{{"id":"E3","start_utc":"{}","end_utc":"{}","victims_affected":[[50.412,1.412],[50.412,1.432]]}}]}}
"#,
        t(0),
        t(7200),
        t(600),
        t(1200),
        t(3000),
        t(3600),
        t(5400),
        t(6000)
    );
    let a = format!(
        r#"[{{"utc":"{}","affected_cells":[[50.00,1.00],[50.00,1.02],[50.02,1.00]]}},
{{"utc":"{}","affected_cells":[[50.20,1.20]]}},
{{"utc":"{}","affected_cells":[[50.40,1.40]]}},
{{"utc":"{}","affected_cells":[[49.00,0.00]]}}]
"#,
        t(620),
        t(3040),
        t(5460),
        t(2000)
    );
    let b = format!(
        r#"[{{"utc":"{}","affected_cells":[[50.00,1.00],[50.00,1.02],[50.02,1.00],[50.02,1.02]]}},
{{"utc":"{}","affected_cells":[[50.40,1.40],[50.40,1.42],[50.42,1.40]]}}]
"#,
        t(690),
        t(5550)
    );
    let proto = format!(
        "title = \"SYNTHETIC product benchmark protocol\"\nregistered_utc = \"{}\"\nguard_s = 60.0\ncell_deg = 0.02\nmin_detection_rate = 0.9\nmax_median_latency_s = 120.0\nmax_false_per_hour = 1.0\nmin_area_iou = 0.5\n",
        t(0).replace("2026-06-01", "2026-05-01")
    );
    (truth, a, b, proto)
}
