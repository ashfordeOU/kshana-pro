// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Area and route briefings with stated confidence and data window (B0901)
//!
//! For named areas (chokepoints, regions, listed areas) and an optional planned route, over a
//! data window, a briefing states:
//!
//! * per area: the events recorded in the window that touch the area (by kind, corroborated or
//!   not), and from the layers: the days the area was observed, the days with any degraded
//!   cell, the share of observed cell-days that were degraded, and the cells degraded on at
//!   least half of the days they were observed ("persistent");
//! * for the route: per layer day, the open engine's route exposure
//!   (`kshana::interference_map::route::exposure`: shares degraded, not degraded, unassessed,
//!   not observed), and the historic-event exposure of `crate::exposure::voyage` along a
//!   corridor;
//! * for each subject, a [`super::Confidence`] under [`super::CONFIDENCE_METHOD`], the data
//!   window and the provenance (file, SHA-256, licence and attribution) of every input.
//!
//! Decision support with stated confidence; never an operational instruction.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};

use super::areas::Area;
use super::layers::{CellState, Layer};
use super::{iso, round, sig9, Confidence, DataWindow, InputRef};
use crate::ievents::Event;

pub const FORMAT: &str = "kshana-pro-exposure-briefing/1";
pub const RESULT_FILE: &str = "briefing.json";

/// The request (`brief.toml`).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefRequest {
    pub title: String,
    /// Area ids to brief (built-in or from `area_file`).
    #[serde(default)]
    pub areas: Vec<String>,
    #[serde(default)]
    pub area_file: Option<String>,
    /// A planned route `[lat, lon]`.
    #[serde(default)]
    pub route: Option<Vec<[f64; 2]>>,
    #[serde(default)]
    pub route_name: Option<String>,
    pub window_from_utc: String,
    pub window_to_utc: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub layers: Vec<String>,
    /// Corridor half-width around the route for events (km).
    #[serde(default = "corridor")]
    pub corridor_km: f64,
    /// An equipment rating (0-100, `kshana-pro exposure equipment`) for the route's risk term.
    #[serde(default)]
    pub equipment_rating: Option<f64>,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

fn corridor() -> f64 {
    25.0
}

impl BriefRequest {
    pub fn from_file(path: &Path) -> Result<(Self, String), String> {
        let (mut r, src): (Self, String) = super::read_request(path)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }
}

/// One area's briefing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AreaBrief {
    pub area_id: String,
    pub name: String,
    pub kind: String,
    pub boundary_source: String,
    pub events: usize,
    pub events_by_kind: BTreeMap<String, usize>,
    pub events_corroborated: usize,
    #[serde(default)]
    pub last_event_utc: Option<String>,
    pub cells: usize,
    pub layer_days: usize,
    pub observed_days: usize,
    pub degraded_days: usize,
    pub cell_days_observed: usize,
    pub cell_days_degraded: usize,
    pub share_degraded: f64,
    pub persistent_cells: usize,
    pub summary: String,
    pub confidence: Confidence,
}

/// One layer day of the route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteDay {
    pub date: String,
    pub source_kind: String,
    pub share_degraded: f64,
    pub share_not_degraded: f64,
    pub share_unassessed: f64,
    pub share_not_observed: f64,
}

/// The route's briefing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RouteBrief {
    pub name: String,
    pub route_km: f64,
    pub days: Vec<RouteDay>,
    /// Mean degraded share over the days, of the observed length.
    pub mean_share_degraded_of_observed: f64,
    pub mean_share_not_observed: f64,
    pub days_with_degraded: usize,
    /// Historic-event exposure and risk (`crate::exposure::voyage`).
    pub event_exposure: f64,
    pub event_risk: f64,
    pub event_band: String,
    pub contributing_events: Vec<String>,
    pub summary: String,
    pub confidence: Confidence,
}

/// The briefing document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Briefing {
    pub format: String,
    pub evidence: String,
    pub statement: String,
    pub confidence_method: String,
    pub title: String,
    pub window: DataWindow,
    pub areas: Vec<AreaBrief>,
    #[serde(default)]
    pub route: Option<RouteBrief>,
    pub inputs: Vec<InputRef>,
    /// Licence and attribution of each layer source.
    pub data_licences: Vec<String>,
    pub limits: Vec<String>,
}

fn ev_times(e: &Event) -> Option<(f64, f64)> {
    Some((
        crate::forensics::inputs::parse_utc(&e.start_utc)?,
        crate::forensics::inputs::parse_utc(&e.end_utc)?,
    ))
}

/// Whether event `e` touches the area: its place (with radius) reaches the area's box and its
/// centre or the nearest box point is inside the polygon; or its named area equals the id.
pub fn event_touches(e: &Event, a: &Area) -> bool {
    if e.area.as_deref() == Some(a.id.as_str()) {
        return true;
    }
    let Some(pl) = &e.location else {
        return false;
    };
    if a.contains(pl.lat_deg, pl.lon_deg) {
        return true;
    }
    let (s, w, n, ea) = a.bbox();
    let (la, lo) = (pl.lat_deg.clamp(s, n), pl.lon_deg.clamp(w, ea));
    a.contains(la, lo) && haversine_m(pl.lat_deg, pl.lon_deg, la, lo) <= pl.radius_m
}

/// Brief one area.
pub fn area_brief(
    a: &Area,
    layers: &[&Layer],
    events: &[&Event],
    days_in_window: usize,
) -> AreaBrief {
    let mine: Vec<&&Event> = events.iter().filter(|e| event_touches(e, a)).collect();
    let mut by_kind = BTreeMap::new();
    for e in &mine {
        *by_kind
            .entry(format!("{:?}", e.kind).to_lowercase())
            .or_insert(0) += 1;
    }
    let corr = mine.iter().filter(|e| e.corroborated).count();
    let last = mine.iter().map(|e| e.end_utc.clone()).max();
    let (mut observed_days, mut degraded_days, mut cdo, mut cdd) = (0, 0, 0, 0);
    let mut per_cell: BTreeMap<(i32, i32), (usize, usize)> = BTreeMap::new();
    let mut ncells = 0;
    let mut coverage_sum = 0.0;
    for l in layers {
        let cells = a.cells(&l.grid());
        ncells = ncells.max(cells.len());
        let (mut o, mut d) = (0, 0);
        for c in &cells {
            match l.cells.get(c) {
                Some(CellState::Degraded) => {
                    o += 1;
                    d += 1;
                    let e = per_cell.entry((c.i, c.j)).or_default();
                    e.0 += 1;
                    e.1 += 1;
                }
                Some(CellState::NotDegraded) => {
                    o += 1;
                    per_cell.entry((c.i, c.j)).or_default().0 += 1;
                }
                _ => {}
            }
        }
        if o > 0 {
            observed_days += 1;
        }
        if d > 0 {
            degraded_days += 1;
        }
        cdo += o;
        cdd += d;
        if !cells.is_empty() {
            coverage_sum += o as f64 / cells.len() as f64;
        }
    }
    let persistent = per_cell
        .values()
        .filter(|(o, d)| *o > 0 && 2 * d >= *o && *d > 0)
        .count();
    let share = if cdo > 0 {
        cdd as f64 / cdo as f64
    } else {
        0.0
    };
    let coverage = if layers.is_empty() {
        0.0
    } else {
        coverage_sum / layers.len() as f64
    };
    // Days with data are the days the layers observed any of the area's cells.
    let conf = super::assess(coverage, observed_days, days_in_window, mine.len(), corr);
    let summary = if observed_days == 0 && mine.is_empty() {
        format!(
            "{}: no layer observed the area and no event touched it in the window; nothing can be said about exposure.",
            a.name
        )
    } else {
        format!(
            "{}: {} event(s) in the window ({} corroborated); degraded cells on {} of {} observed day(s); {:.1} % of observed cell-days degraded; {} persistent cell(s). Confidence {}.",
            a.name,
            mine.len(),
            corr,
            degraded_days,
            observed_days,
            100.0 * share,
            persistent,
            conf.level
        )
    };
    AreaBrief {
        area_id: a.id.clone(),
        name: a.name.clone(),
        kind: a.kind.clone(),
        boundary_source: a.source.clone(),
        events: mine.len(),
        events_by_kind: by_kind,
        events_corroborated: corr,
        last_event_utc: last,
        cells: ncells,
        layer_days: {
            let mut d: Vec<&str> = layers.iter().map(|l| l.date.as_str()).collect();
            d.dedup();
            d.len()
        },
        observed_days,
        degraded_days,
        cell_days_observed: cdo,
        cell_days_degraded: cdd,
        share_degraded: sig9(round(share, 4)),
        persistent_cells: persistent,
        summary,
        confidence: conf,
    }
}

/// Brief a route.
#[allow(clippy::too_many_arguments)]
pub fn route_brief(
    name: &str,
    waypoints: &[[f64; 2]],
    layers: &[&Layer],
    events: &[Event],
    window_to_utc: &str,
    corridor_km: f64,
    rating: Option<f64>,
    days_in_window: usize,
) -> Result<RouteBrief, String> {
    let route: Vec<(f64, f64)> = waypoints.iter().map(|w| (w[0], w[1])).collect();
    let mut days = vec![];
    for l in layers {
        let e = kshana::interference_map::route::exposure(&route, &l.map);
        days.push(RouteDay {
            date: e.date,
            source_kind: e.source_kind,
            share_degraded: sig9(round(e.share_degraded, 4)),
            share_not_degraded: sig9(round(e.share_not_degraded, 4)),
            share_unassessed: sig9(round(e.share_unassessed, 4)),
            share_not_observed: sig9(round(e.share_not_observed, 4)),
        });
    }
    let n = days.len().max(1) as f64;
    let mean_not_obs = days.iter().map(|d| d.share_not_observed).sum::<f64>() / n;
    let mean_deg_obs = days
        .iter()
        .map(|d| {
            let obs = d.share_degraded + d.share_not_degraded;
            if obs > 0.0 {
                d.share_degraded / obs
            } else {
                0.0
            }
        })
        .sum::<f64>()
        / n;
    let dwd = days.iter().filter(|d| d.share_degraded > 0.0).count();
    let vr = crate::exposure::voyage::VoyageRequest {
        title: name.into(),
        vessel: name.into(),
        waypoints: waypoints.to_vec(),
        departure_utc: window_to_utc.into(),
        events: vec![],
        equipment: None,
        corridor_km,
        base_dir: None,
    };
    let risk = crate::exposure::voyage::voyage_risk(&vr, events, rating)?;
    let near: Vec<&Event> = events
        .iter()
        .filter(|e| risk.contributions.iter().any(|c| c.event_id == e.id))
        .collect();
    let corr = near.iter().filter(|e| e.corroborated).count();
    let mut dates: Vec<&str> = days
        .iter()
        .filter(|d| d.share_not_observed < 1.0)
        .map(|d| d.date.as_str())
        .collect();
    dates.dedup();
    let coverage = if days.is_empty() {
        0.0
    } else {
        1.0 - mean_not_obs
    };
    let conf = super::assess(coverage, dates.len(), days_in_window, near.len(), corr);
    let summary = format!(
        "{} ({:.0} km): degraded on {} of {} layer day(s); on average {:.1} % of the observed length degraded and {:.1} % of the length not observed; {} historic event(s) within {:.0} km, event exposure {:.3}. Confidence {}.",
        name,
        risk.route_km,
        dwd,
        days.len(),
        100.0 * mean_deg_obs,
        100.0 * mean_not_obs,
        near.len(),
        corridor_km,
        risk.exposure,
        conf.level
    );
    Ok(RouteBrief {
        name: name.into(),
        route_km: risk.route_km,
        mean_share_degraded_of_observed: sig9(round(mean_deg_obs, 4)),
        mean_share_not_observed: sig9(round(mean_not_obs, 4)),
        days_with_degraded: dwd,
        days,
        event_exposure: risk.exposure,
        event_risk: risk.risk,
        event_band: risk.band,
        contributing_events: risk
            .contributions
            .iter()
            .map(|c| c.event_id.clone())
            .collect(),
        summary,
        confidence: conf,
    })
}

/// The licence and attribution lines of the layers, one per distinct source.
pub fn licences(layers: &[&Layer]) -> Vec<String> {
    let mut v: Vec<String> = layers
        .iter()
        .map(|l| {
            format!(
                "{} layers: licence {}; attribution {}",
                l.source_kind,
                if l.licence.is_empty() {
                    "not stated"
                } else {
                    &l.licence
                },
                if l.attribution.is_empty() {
                    "not stated"
                } else {
                    &l.attribution
                }
            )
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// Build the briefing.
pub fn build(r: &BriefRequest) -> Result<Briefing, String> {
    let t0 = super::utc("window_from_utc", &r.window_from_utc)?;
    let t1 = super::utc("window_to_utc", &r.window_to_utc)?;
    if t1 <= t0 {
        return Err("the window is empty".into());
    }
    if r.areas.is_empty() && r.route.is_none() {
        return Err("name at least one area or a route".into());
    }
    let mut inputs = vec![];
    let all_events = super::load_events(&r.base_dir, &r.events, &mut inputs)?;
    let all_layers = super::layers::load_layers(&r.base_dir, &r.layers, &mut inputs)?;
    let cat = super::transit::load_catalogue(&r.base_dir, &r.area_file, false, &mut inputs)?;
    let areas = super::areas::select(&cat, &r.areas)?;
    let events: Vec<&Event> = all_events
        .iter()
        .filter(|e| ev_times(e).is_some_and(|(a, b)| b >= t0 && a <= t1))
        .collect();
    let layers: Vec<&Layer> = all_layers
        .iter()
        .filter(|l| l.day_start + 86_400.0 > t0 && l.day_start <= t1)
        .collect();
    let days = super::days_between(t0, t1);
    let area_briefs: Vec<AreaBrief> = areas
        .iter()
        .map(|a| area_brief(a, &layers, &events, days))
        .collect();
    let route = match &r.route {
        Some(w) => {
            if w.len() < 2 {
                return Err("a route needs at least two waypoints".into());
            }
            let ev: Vec<Event> = events.iter().map(|e| (*e).clone()).collect();
            Some(route_brief(
                r.route_name.as_deref().unwrap_or("Planned route"),
                w,
                &layers,
                &ev,
                &r.window_to_utc,
                r.corridor_km,
                r.equipment_rating,
                days,
            )?)
        }
        None => None,
    };
    let latest = layers
        .iter()
        .map(|l| l.day_start + 86_400.0)
        .chain(events.iter().filter_map(|e| ev_times(e).map(|x| x.1)))
        .fold(None, |m: Option<f64>, t| Some(m.map_or(t, |m| m.max(t))))
        .map(|t| iso(t.min(t1)));
    let mut layer_days: Vec<String> = layers.iter().map(|l| l.date.clone()).collect();
    layer_days.dedup();
    let mut limits = vec![
        "The briefing summarises past observations in the window; it is not a forecast.".to_string(),
        "A degraded cell does not identify interference as the cause; cells not observed are not evidence of a clear area.".into(),
        "Areas are as named: built-in areas are coarse boxes for accounting, not legal, listed or charted boundaries.".into(),
        "Events are as reported by their observers; the briefing does not verify them beyond their stated confidence and corroboration.".into(),
    ];
    if layers.is_empty() {
        limits.push("No layer falls in the window: statements rest on events only.".into());
    }
    Ok(Briefing {
        format: FORMAT.into(),
        evidence: super::EVIDENCE.into(),
        statement: super::DECISION_SUPPORT.into(),
        confidence_method: super::CONFIDENCE_METHOD.into(),
        title: r.title.clone(),
        window: DataWindow {
            from_utc: r.window_from_utc.clone(),
            to_utc: r.window_to_utc.clone(),
            days_in_window: days,
            layer_days,
            events_in_window: events.len(),
            latest_data_utc: latest,
        },
        areas: area_briefs,
        route,
        inputs,
        data_licences: licences(&layers),
        limits,
    })
}

/// The briefing page.
pub fn render_html(b: &Briefing) -> String {
    use super::esc;
    let mut s = super::html_head(&b.title);
    s.push_str(&format!(
        "<h1>{}</h1><p><b>Data window:</b> {} to {} ({} day(s); {} layer day(s); {} event(s); latest data {}).</p>\
         <p class=\"l\">{}</p><p class=\"l\">{}</p>",
        esc(&b.title),
        esc(&b.window.from_utc),
        esc(&b.window.to_utc),
        b.window.days_in_window,
        b.window.layer_days.len(),
        b.window.events_in_window,
        esc(b.window.latest_data_utc.as_deref().unwrap_or("none")),
        esc(&b.statement),
        esc(&b.evidence)
    ));
    if !b.areas.is_empty() {
        s.push_str("<h2>Areas</h2><table><tr><th>Area</th><th>Events</th><th>Degraded days / observed days</th><th>Cell-days degraded</th><th>Persistent cells</th><th>Confidence</th></tr>");
        for a in &b.areas {
            s.push_str(&format!(
                "<tr><td>{} <span class=\"l\">({})</span></td><td>{} ({} corroborated) {}</td><td>{} / {}</td><td>{:.1} %</td><td>{}</td><td class=\"c-{}\">{}<br><span class=\"l\">{}</span></td></tr>",
                esc(&a.name),
                esc(&a.kind),
                a.events,
                a.events_corroborated,
                esc(&a.events_by_kind.iter().map(|(k, n)| format!("{k} {n}")).collect::<Vec<_>>().join(", ")),
                a.degraded_days,
                a.observed_days,
                100.0 * a.share_degraded,
                a.persistent_cells,
                esc(&a.confidence.level),
                esc(&a.confidence.level),
                esc(&a.confidence.reasons.join("; "))
            ));
        }
        s.push_str("</table>");
    }
    if let Some(r) = &b.route {
        s.push_str(&format!(
            "<h2>Route: {}</h2><p>{}</p><p class=\"l\">Confidence <span class=\"c-{}\">{}</span>: {}</p>\
             <table><tr><th>Day</th><th>Source</th><th>Degraded</th><th>Not degraded</th><th>Unassessed</th><th>Not observed</th></tr>",
            esc(&r.name),
            esc(&r.summary),
            esc(&r.confidence.level),
            esc(&r.confidence.level),
            esc(&r.confidence.reasons.join("; "))
        ));
        for d in &r.days {
            s.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{:.1} %</td><td>{:.1} %</td><td>{:.1} %</td><td>{:.1} %</td></tr>",
                esc(&d.date),
                esc(&d.source_kind),
                100.0 * d.share_degraded,
                100.0 * d.share_not_degraded,
                100.0 * d.share_unassessed,
                100.0 * d.share_not_observed
            ));
        }
        s.push_str("</table>");
    }
    s.push_str(&format!(
        "<h2>How confident</h2><p class=\"l\">{}</p><ul>",
        esc(&b.confidence_method)
    ));
    for l in &b.limits {
        s.push_str(&format!("<li>{}</li>", esc(l)));
    }
    s.push_str("</ul><h2>What it rests on</h2><ul>");
    for i in &b.inputs {
        s.push_str(&format!(
            "<li>{} ({}, {} bytes) sha256 <code>{}</code></li>",
            esc(&i.file),
            esc(&i.role),
            i.bytes,
            esc(&i.sha256)
        ));
    }
    for l in &b.data_licences {
        s.push_str(&format!("<li>{}</li>", esc(l)));
    }
    s.push_str("</ul></body></html>\n");
    s
}

/// The briefing's PDF sections.
pub fn pdf_sections(b: &Briefing) -> Vec<(String, Vec<String>)> {
    let mut v = vec![(
        "What this is".to_string(),
        vec![
            b.statement.clone(),
            b.evidence.clone(),
            format!(
                "Data window {} to {}: {} day(s), {} layer day(s), {} event(s); latest data {}.",
                b.window.from_utc,
                b.window.to_utc,
                b.window.days_in_window,
                b.window.layer_days.len(),
                b.window.events_in_window,
                b.window.latest_data_utc.as_deref().unwrap_or("none")
            ),
        ],
    )];
    if !b.areas.is_empty() {
        v.push((
            "Areas".into(),
            b.areas
                .iter()
                .map(|a| {
                    format!(
                        "{} Reasons: {}.",
                        a.summary,
                        a.confidence.reasons.join("; ")
                    )
                })
                .collect(),
        ));
    }
    if let Some(r) = &b.route {
        v.push((
            format!("Route: {}", r.name),
            vec![
                r.summary.clone(),
                format!("Reasons: {}.", r.confidence.reasons.join("; ")),
            ],
        ));
    }
    let mut how = vec![b.confidence_method.clone()];
    how.extend(b.limits.iter().map(|l| format!("- {l}")));
    v.push(("How confident".into(), how));
    let mut rests: Vec<String> = b
        .inputs
        .iter()
        .map(|i| {
            format!(
                "{} ({}, {} bytes) sha256 {}",
                i.file, i.role, i.bytes, i.sha256
            )
        })
        .collect();
    rests.extend(b.data_licences.iter().cloned());
    v.push(("What it rests on".into(), rests));
    v
}

/// Write `briefing.json`, `briefing.html`, `briefing.pdf` and the request.
pub fn write_dir(out: &Path, b: &Briefing, request_src: &str) -> Result<(), String> {
    super::write_json(out, RESULT_FILE, b)?;
    super::write_text(out, "briefing.html", render_html(b).as_bytes())?;
    let id = crate::forensics::inputs::sha256_hex(
        serde_json::to_string(b).unwrap_or_default().as_bytes(),
    );
    let pdf = super::simple_pdf(
        &format!("Exposure briefing: {}", b.title),
        &format!(
            "Exposure briefing - {} to {} - MODELLED",
            b.window.from_utc, b.window.to_utc
        ),
        &id[..16],
        &pdf_sections(b),
    )?;
    super::write_text(out, "briefing.pdf", &pdf)?;
    super::write_text(out, "request.toml", request_src.as_bytes())
}
