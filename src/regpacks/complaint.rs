// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Harmful-interference report pack (E29 B2903; research 05 #168, 06 J1/J3/J10/J11, 07 N02,
//! 12 #1676/#1755/#1756)
//!
//! Administrations file harmful-interference reports with each other and with the ITU under
//! Article 15 of the Radio Regulations (Section VI, procedure in a case of harmful
//! interference; paragraph numbers to be confirmed against the edition the filer uses), on the
//! report form of RR Appendix 10. Traficom's state-to-state filings (April 2026) and the Baltic
//! RNSS cases before the Radio Regulations Board rest on evidence of effects. This module
//! fills a report **shaped after the headings of Appendix 10** from:
//!
//! * the filer's request (administration, victim service and bands, action requested);
//! * interference episodes from `kshana-pro reports episodes`
//!   (`kshana-pro-interference-episodes/1`) and the filer's own observations;
//! * signed Kshana evidence directories, each verified and listed with its hash.
//!
//! **Effects and areas only.** Kshana does not locate or identify emitters. Every observation
//! is reduced to the grid cell of [`CELL_DEG`] degrees (or a coarser cell the filer chooses)
//! that holds it; the particulars of the interfering station are written as not determined.
//! No observation position finer than the cell leaves this module.
//!
//! The pack is an evidence annex for the administration's own filing: the filing, its form
//! and the action requested are the administration's acts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{esc, page_head, sha256_hex, sig9, write_json, EvidenceItem};

/// The pack's format tag.
pub const FORMAT: &str = "kshana-pro-interference-complaint/1";
/// The file the pack writes.
pub const RESULT_FILE: &str = "interference-complaint.json";
/// The method id.
pub const METHOD: &str = "kshana-pro/interference-complaint-ap10/v1";
/// The finest area cell, degrees.
pub const CELL_DEG: f64 = 0.5;
/// What the pack writes for every particular of the interfering station.
pub const NOT_DETERMINED: &str = "not determined: Kshana does not locate or identify \
emitters; the affected areas are listed under the particulars of the interference";

/// The victim service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Victim {
    pub service: String,
    pub stations: String,
    pub frequencies_mhz: Vec<f64>,
    #[serde(default)]
    pub bands: Vec<String>,
    #[serde(default)]
    pub class_of_emission: String,
}

/// One observation of the filer (precise inputs; reduced to a cell on output).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub start_utc: String,
    pub end_utc: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// `jamming`, `spoofing`, `mixed` or `unknown`.
    pub kind: String,
    pub effect: String,
    #[serde(default = "one")]
    pub observers: usize,
    #[serde(default)]
    pub measured: String,
    #[serde(default)]
    pub source: String,
}

fn one() -> usize {
    1
}

fn default_cell() -> f64 {
    CELL_DEG
}

/// The filer's request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub reporting_administration: String,
    #[serde(default)]
    pub reporting_entity: String,
    #[serde(default)]
    pub reference: String,
    /// The Radio Regulations edition the filer works to.
    pub rr_edition: String,
    #[serde(default = "default_cell")]
    pub cell_deg: f64,
    pub victim: Victim,
    pub action_requested: String,
    #[serde(rename = "observation", default)]
    pub observations: Vec<Observation>,
}

impl Request {
    pub fn parse(src: &str) -> Result<Request, String> {
        let r: Request = toml::from_str(src).map_err(|e| format!("complaint request: {e}"))?;
        if !(CELL_DEG..=10.0).contains(&r.cell_deg) {
            return Err(format!(
                "cell_deg {} refused: areas are reported at {CELL_DEG} degrees or coarser (at most 10)",
                r.cell_deg
            ));
        }
        if r.victim.frequencies_mhz.is_empty() {
            return Err("victim.frequencies_mhz is empty".into());
        }
        for o in &r.observations {
            check_obs(o)?;
        }
        Ok(r)
    }
}

const KINDS: [&str; 4] = ["jamming", "spoofing", "mixed", "unknown"];

fn check_obs(o: &Observation) -> Result<(), String> {
    if !(-90.0..=90.0).contains(&o.lat_deg) || !(-180.0..=180.0).contains(&o.lon_deg) {
        return Err(format!(
            "observation {}: position out of range",
            o.start_utc
        ));
    }
    if !KINDS.contains(&o.kind.as_str()) {
        return Err(format!(
            "observation {}: kind '{}' is not one of {}",
            o.start_utc,
            o.kind,
            KINDS.join(", ")
        ));
    }
    let (Some(a), Some(b)) = (
        crate::forensics::inputs::parse_utc(&o.start_utc),
        crate::forensics::inputs::parse_utc(&o.end_utc),
    ) else {
        return Err(format!("observation {}: bad UTC time", o.start_utc));
    };
    if b < a {
        return Err(format!(
            "observation {}: ends before it starts",
            o.start_utc
        ));
    }
    Ok(())
}

/// Observations from a `kshana-pro-interference-episodes/1` document.
pub fn observations_from_episodes(text: &str) -> Result<Vec<Observation>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("episodes: {e}"))?;
    if v["format"].as_str() != Some("kshana-pro-interference-episodes/1") {
        return Err("episodes: not a kshana-pro-interference-episodes/1 document".into());
    }
    let mut out = Vec::new();
    for e in v["episodes"].as_array().cloned().unwrap_or_default() {
        let s = |k: &str| e[k].as_str().unwrap_or("").to_string();
        let kind = s("kind");
        let o = Observation {
            start_utc: s("start_utc"),
            end_utc: s("end_utc"),
            lat_deg: e["centroid_lat_deg"]
                .as_f64()
                .ok_or("episode without centroid")?,
            lon_deg: e["centroid_lon_deg"]
                .as_f64()
                .ok_or("episode without centroid")?,
            kind: if KINDS.contains(&kind.as_str()) {
                kind
            } else {
                "unknown".into()
            },
            effect: format!(
                "interference episode {} from crew, controller or mariner reports{}",
                s("id"),
                if e["adsb_aircraft"].as_array().is_some_and(|a| !a.is_empty()) {
                    " and ADS-B integrity degradations"
                } else {
                    ""
                }
            ),
            observers: e["observers"].as_u64().unwrap_or(1) as usize,
            measured: String::new(),
            source: format!("episode {}", s("id")),
        };
        check_obs(&o)?;
        out.push(o);
    }
    Ok(out)
}

/// The cell holding a point: south-west corner, degrees.
pub fn cell_of(lat: f64, lon: f64, cell: f64) -> (f64, f64) {
    let c = |x: f64| sig9((x / cell).floor() * cell);
    (c(lat.min(90.0 - cell)), c(lon.min(180.0 - cell)))
}

/// The interference in one area cell.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Area {
    /// `lat [s, n) lon [w, e)`, degrees.
    pub cell: String,
    pub south_deg: f64,
    pub west_deg: f64,
    pub size_deg: f64,
    pub first_utc: String,
    pub last_utc: String,
    pub observations: usize,
    /// Sum of independent observers over the observations (may count an observer twice
    /// across observations).
    pub observers: usize,
    pub kinds: Vec<String>,
    pub effects: Vec<String>,
    pub measured: Vec<String>,
    pub sources: Vec<String>,
}

/// The particulars of the interfering station (all not determined).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Interferer {
    pub identity: &'static str,
    pub location: &'static str,
    pub frequency_measured: &'static str,
    pub class_of_emission: &'static str,
    pub field_strength: &'static str,
    /// What the effects suggest, in effect terms only.
    pub nature_of_effects: String,
}

/// The pack.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Complaint {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub statement: &'static str,
    pub form: &'static str,
    pub rr_edition: String,
    pub reporting_administration: String,
    pub reporting_entity: String,
    pub reference: String,
    pub request_sha256: String,
    pub episodes_sha256: Option<String>,
    pub interfering_station: Interferer,
    pub victim: Victim,
    pub period_utc: [String; 2],
    pub areas: Vec<Area>,
    pub action_requested: String,
    pub evidence: Vec<EvidenceItem>,
    pub limits: Vec<&'static str>,
}

/// What every pack says about itself.
pub const STATEMENT: &str = "An evidence annex for the administration's harmful-interference \
report, arranged under the headings of ITU RR Appendix 10. It records effects on the victim \
service and the areas where they were observed, at cell granularity. It does not identify or \
locate any emitter and does not attribute the interference to any party.";

const LIMITS: [&str; 4] = [
    "Areas are the grid cells holding the observations, not the extent of the interference and not an emitter position.",
    "Observer counts add per observation; one observer seen in two observations counts twice.",
    "Episodes inherit the limits of the reports behind them (see the episodes document's definition).",
    "Appendix 10 item numbering and the Article 15 paragraph numbers are to be confirmed against the edition named in rr_edition.",
];

/// Build the pack.
pub fn build(
    r: &Request,
    request_text: &str,
    episodes: Option<(&str, Vec<Observation>)>,
    evidence: Vec<EvidenceItem>,
) -> Result<Complaint, String> {
    let mut obs = r.observations.clone();
    let episodes_sha256 = episodes.as_ref().map(|(t, _)| sha256_hex(t.as_bytes()));
    if let Some((_, e)) = episodes {
        obs.extend(e);
    }
    if obs.is_empty() {
        return Err("no observations: give [[observation]] rows or --episodes".into());
    }
    let mut cells: BTreeMap<(i64, i64), Vec<&Observation>> = BTreeMap::new();
    for o in &obs {
        let (s, w) = cell_of(o.lat_deg, o.lon_deg, r.cell_deg);
        let key = (
            (s / r.cell_deg).round() as i64,
            (w / r.cell_deg).round() as i64,
        );
        cells.entry(key).or_default().push(o);
    }
    let t = |s: &str| crate::forensics::inputs::parse_utc(s).unwrap_or(f64::NAN);
    let iso = crate::forensics::inputs::iso;
    let mut first_all = f64::INFINITY;
    let mut last_all = f64::NEG_INFINITY;
    let mut all_kinds = BTreeSet::new();
    let areas: Vec<Area> = cells
        .into_iter()
        .map(|((si, wi), v)| {
            let s = sig9(si as f64 * r.cell_deg);
            let w = sig9(wi as f64 * r.cell_deg);
            let first = v
                .iter()
                .map(|o| t(&o.start_utc))
                .fold(f64::INFINITY, f64::min);
            let last = v
                .iter()
                .map(|o| t(&o.end_utc))
                .fold(f64::NEG_INFINITY, f64::max);
            first_all = first_all.min(first);
            last_all = last_all.max(last);
            let set = |f: &dyn Fn(&Observation) -> &str| -> Vec<String> {
                v.iter()
                    .map(|o| f(o).to_string())
                    .filter(|x| !x.is_empty())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect()
            };
            let kinds = set(&|o| o.kind.as_str());
            all_kinds.extend(kinds.iter().cloned());
            Area {
                cell: format!(
                    "lat [{s}, {}) lon [{w}, {})",
                    sig9(s + r.cell_deg),
                    sig9(w + r.cell_deg)
                ),
                south_deg: s,
                west_deg: w,
                size_deg: sig9(r.cell_deg),
                first_utc: iso(first),
                last_utc: iso(last),
                observations: v.len(),
                observers: v.iter().map(|o| o.observers).sum(),
                kinds,
                effects: set(&|o| o.effect.as_str()),
                measured: set(&|o| o.measured.as_str()),
                sources: set(&|o| o.source.as_str()),
            }
        })
        .collect();
    let nature = {
        let mut p = Vec::new();
        if all_kinds.contains("jamming") || all_kinds.contains("mixed") {
            p.push("loss or degradation of the wanted signals (effects reported as jamming)");
        }
        if all_kinds.contains("spoofing") || all_kinds.contains("mixed") {
            p.push("false position or time from the receivers (effects reported as spoofing)");
        }
        if p.is_empty() {
            p.push("interference of unstated kind");
        }
        p.join("; ")
    };
    Ok(Complaint {
        format: FORMAT,
        method: METHOD,
        label: super::LABEL,
        statement: STATEMENT,
        form: "ITU Radio Regulations Appendix 10 (report of harmful interference), headings only",
        rr_edition: r.rr_edition.clone(),
        reporting_administration: r.reporting_administration.clone(),
        reporting_entity: r.reporting_entity.clone(),
        reference: r.reference.clone(),
        request_sha256: sha256_hex(request_text.as_bytes()),
        episodes_sha256,
        interfering_station: Interferer {
            identity: NOT_DETERMINED,
            location: NOT_DETERMINED,
            frequency_measured: NOT_DETERMINED,
            class_of_emission: NOT_DETERMINED,
            field_strength: NOT_DETERMINED,
            nature_of_effects: nature,
        },
        victim: Victim {
            frequencies_mhz: r.victim.frequencies_mhz.iter().map(|f| sig9(*f)).collect(),
            ..r.victim.clone()
        },
        period_utc: [iso(first_all), iso(last_all)],
        areas,
        action_requested: r.action_requested.clone(),
        evidence,
        limits: LIMITS.to_vec(),
    })
}

/// The pack's page.
pub fn render_html(c: &Complaint) -> String {
    let mut h = page_head("Harmful-interference report annex");
    h.push_str(&format!(
        "<h1>Harmful-interference report annex</h1><p class=\"l\">{}</p><p>{}</p>\
         <p>Reporting administration: {} {}. Reference: {}. Form: {} ({}).</p>\
         <h2>Particulars concerning the station causing the interference</h2>\
         <p>Identity, location, measured frequency, class of emission, field strength: {}</p><p>Nature of the effects: {}</p>\
         <h2>Particulars concerning the station experiencing the interference</h2>\
         <p>Service: {}. Stations: {}. Frequencies (MHz): {}. Bands: {}.</p>\
         <h2>Particulars of the interference</h2><p>Period: {} to {}.</p>\
         <table><tr><th>Area cell</th><th>First</th><th>Last</th><th>Observations</th><th>Observers</th><th>Kinds</th><th>Effects</th><th>Measured</th></tr>",
        esc(c.label),
        esc(c.statement),
        esc(&c.reporting_administration),
        if c.reporting_entity.is_empty() { String::new() } else { format!("({})", esc(&c.reporting_entity)) },
        esc(&c.reference),
        esc(c.form),
        esc(&c.rr_edition),
        esc(NOT_DETERMINED),
        esc(&c.interfering_station.nature_of_effects),
        esc(&c.victim.service),
        esc(&c.victim.stations),
        c.victim.frequencies_mhz.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", "),
        esc(&c.victim.bands.join(", ")),
        esc(&c.period_utc[0]),
        esc(&c.period_utc[1]),
    ));
    for a in &c.areas {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&a.cell),
            esc(&a.first_utc),
            esc(&a.last_utc),
            a.observations,
            a.observers,
            esc(&a.kinds.join(", ")),
            esc(&a.effects.join("; ")),
            esc(&a.measured.join("; "))
        ));
    }
    h.push_str(&format!(
        "</table><h2>Action requested</h2><p>{}</p><h2>Evidence register</h2><table><tr><th>Item</th><th>Products</th><th>Signer</th><th>Signature</th><th>SHA-256</th></tr>",
        esc(&c.action_requested)
    ));
    for e in &c.evidence {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><code>{}</code></td></tr>",
            esc(&e.path),
            esc(&e.products.join("; ")),
            esc(e.signer.as_deref().unwrap_or("")),
            if e.usable() {
                "valid"
            } else {
                "INVALID or not trusted"
            },
            esc(&e.sha256)
        ));
    }
    h.push_str("</table><h2>Limits</h2><ul>");
    for l in &c.limits {
        h.push_str(&format!("<li>{}</li>", esc(l)));
    }
    h.push_str("</ul></body></html>\n");
    h
}

/// Write the pack's directory.
pub fn write_dir(out: &std::path::Path, c: &Complaint) -> Result<(), String> {
    write_json(out, RESULT_FILE, c)?;
    std::fs::write(out.join("report.html"), render_html(c)).map_err(|e| e.to_string())
}
