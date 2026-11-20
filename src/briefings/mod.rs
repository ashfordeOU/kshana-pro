// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Exposure, route risk and briefings (research backlog E09: B0901 to B0905)
//!
//! Interference exposure along the routes people actually travel, kept as history and read
//! back as briefings that state their confidence and their data window:
//!
//! * [`transit`] (B0901): per-transit exposure records from AIS tracks (time in named areas,
//!   in degraded cells of the open interference-map layers, within reach of recorded
//!   interference events), listed-area records per transit, and a per-vessel fleet view.
//! * [`brief`] (B0901): a briefing for named areas (chokepoints, regions, listed areas) and an
//!   optional planned route, over a stated data window, with a stated confidence and the
//!   provenance of every input. JSON, HTML and PDF.
//! * [`aviation`] (B0902): a per-tail exposure ledger from ADS-B tracks over a period (90 days
//!   by default), altitude-resolved below 2,000 ft; a low-altitude cell map; flight briefings
//!   and an operations-control alert list for planned flights, with the data latency stated.
//! * [`regional`] (B0903): versioned regional datasets (per-cell exposure time series for
//!   named regions) that rebuild byte-identically from the same inputs, with pattern
//!   separation and per-airport pattern libraries.
//! * [`road`] (B0904): a corridor exposure index and heat map from telematics jammer-detection
//!   events, aggregated so that no output identifies a device or vehicle.
//! * [`spillover`] (B0905): a NOTAM-to-geometry converter with harmonised start and end
//!   criteria, a modelled spillover footprint around the **declared** test area by receiver
//!   altitude, the users inside it, and agreement against ADS-B degradation layers.
//!
//! Granularity: every footprint, map and area output is either a named area or a grid of cells
//! of at least [`MIN_CELL_DEG`] degrees, the open engine's publication granularity. Nothing
//! here estimates where an emitter is: the spillover model starts from the area an authority
//! declared, never from the observations.
//!
//! Evidence: **MODELLED** throughout. Briefings are decision support with stated confidence;
//! they are never operational instructions, navigation warnings or routing orders.

pub mod areas;
pub mod aviation;
pub mod brief;
pub mod layers;
pub mod regional;
pub mod road;
pub mod spillover;
pub mod transit;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Route and voyage exposure history, fleet view and briefings (B0901).
pub const FEATURE_ROUTE: &str = "pro-route-exposure";
/// Per-tail ledger, low-altitude map and flight briefings / OCC alerts (B0902).
pub const FEATURE_AVIATION: &str = "pro-aviation-exposure";
/// Versioned regional exposure datasets and airport pattern libraries (B0903).
pub const FEATURE_REGIONAL: &str = "pro-regional-datasets";
/// Road and logistics corridor exposure index (B0904).
pub const FEATURE_ROAD: &str = "pro-road-exposure";
/// Spillover footprint around declared test areas (B0905). The NOTAM converter is free.
pub const FEATURE_SPILLOVER: &str = "pro-spillover-footprint";

/// Every command feature of this module, for registration checks.
pub const FEATURES: &[&str] = &[
    FEATURE_ROUTE,
    FEATURE_AVIATION,
    FEATURE_REGIONAL,
    FEATURE_ROAD,
    FEATURE_SPILLOVER,
];

/// The coarsest-allowed granularity: no map or footprint here is finer than this.
pub const MIN_CELL_DEG: f64 = kshana::interference_map::observatory::MIN_CELL_DEG;

/// The evidence label of every output.
pub const EVIDENCE: &str = "MODELLED: computed from the tracks, events and interference-map \
layers supplied under the stated method; not validated against independent ground truth.";

/// What every briefing says about itself.
pub const DECISION_SUPPORT: &str = "Decision support only. This briefing summarises past \
observations under a stated method, with a stated confidence and data window. It is not an \
operational instruction, a navigation warning, a NOTAM, a routing order or a forecast; the \
master, the pilot in command, the operator and the competent authority decide.";

/// Round to 9 significant digits (every float that feeds a hash or a dataset byte stream).
pub fn sig9(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return if x.is_finite() { 0.0 } else { x };
    }
    format!("{x:.8e}").parse().unwrap_or(x)
}

/// Round to `d` decimals (display values).
pub fn round(x: f64, d: i32) -> f64 {
    crate::forensics::inputs::round(x, d)
}

/// One input file the result rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRef {
    /// `tracks`, `events`, `layer`, `areas`, `notam`, `detections`.
    pub role: String,
    /// The file as the request names it (never a resolved local path).
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
}

/// Resolve `p` against the request's directory.
pub fn resolve(base: &Option<PathBuf>, p: &str) -> PathBuf {
    match base {
        Some(d) if Path::new(p).is_relative() => d.join(p),
        _ => PathBuf::from(p),
    }
}

/// Read `named` (resolved against `base`) and record it.
pub fn read_input(
    base: &Option<PathBuf>,
    named: &str,
    role: &str,
) -> Result<(Vec<u8>, InputRef), String> {
    let p = resolve(base, named);
    let b = std::fs::read(&p).map_err(|e| format!("{named}: {e}"))?;
    let r = InputRef {
        role: role.into(),
        file: named.into(),
        sha256: crate::forensics::inputs::sha256_hex(&b),
        bytes: b.len(),
    };
    Ok((b, r))
}

/// Expand a list of files or directories: a directory contributes its files with extension
/// `ext`, sorted by name. Returns the names as the request would name them.
pub fn expand(base: &Option<PathBuf>, named: &[String], ext: &str) -> Result<Vec<String>, String> {
    let mut out = vec![];
    for n in named {
        let p = resolve(base, n);
        if p.is_dir() {
            let mut v: Vec<String> = std::fs::read_dir(&p)
                .map_err(|e| format!("{n}: {e}"))?
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|f| f.ends_with(ext))
                .collect();
            v.sort();
            out.extend(
                v.into_iter()
                    .map(|f| format!("{}/{f}", n.trim_end_matches('/'))),
            );
        } else {
            out.push(n.clone());
        }
    }
    Ok(out)
}

/// Load unified interference events (`kshana-pnt-interference-event/1` JSON arrays).
pub fn load_events(
    base: &Option<PathBuf>,
    named: &[String],
    inputs: &mut Vec<InputRef>,
) -> Result<Vec<crate::ievents::Event>, String> {
    let mut ev = vec![];
    for n in named {
        let (b, r) = read_input(base, n, "events")?;
        let v: Vec<crate::ievents::Event> =
            serde_json::from_slice(&b).map_err(|e| format!("{n}: {e}"))?;
        for e in &v {
            let p = crate::ievents::validate(e);
            if !p.is_empty() {
                return Err(format!("{n}: event {}: {}", e.id, p.join("; ")));
            }
        }
        inputs.push(r);
        ev.extend(v);
    }
    Ok(ev)
}

/// The stated confidence of a briefing or alert.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Confidence {
    /// `high`, `medium`, `low` or `insufficient`.
    pub level: String,
    /// Share of the briefed cells (or route length) the layers observed, 0..1.
    pub coverage: f64,
    /// Days in the window that have at least one layer.
    pub days_with_data: usize,
    pub days_in_window: usize,
    /// Events in the window, and those corroborated by more than one observer.
    pub events: usize,
    pub events_corroborated: usize,
    pub reasons: Vec<String>,
}

/// How [`assess`] decides.
pub const CONFIDENCE_METHOD: &str = "Confidence: insufficient when no layer day and no event \
covers the subject in the window; high when the layers observed at least 70 % of the subject, \
layers exist for at least 80 % of the window's days and at least half of the events (if any) \
are corroborated by more than one observer; medium when coverage is at least 40 % and layers \
exist for at least half of the days; low otherwise. A low-altitude subject without \
low-altitude observations is lowered one level.";

/// Assess the confidence of a statement about one subject.
pub fn assess(
    coverage: f64,
    days_with_data: usize,
    days_in_window: usize,
    events: usize,
    events_corroborated: usize,
) -> Confidence {
    let day_share = if days_in_window > 0 {
        days_with_data as f64 / days_in_window as f64
    } else {
        0.0
    };
    let mut reasons = vec![
        format!(
            "layers observed {:.0} % of the subject",
            100.0 * coverage.clamp(0.0, 1.0)
        ),
        format!("layers exist for {days_with_data} of {days_in_window} day(s) in the window"),
    ];
    if events > 0 {
        reasons.push(format!(
            "{events_corroborated} of {events} event(s) corroborated by more than one observer"
        ));
    }
    let corroborated_ok = events == 0 || events_corroborated * 2 >= events;
    let level = if days_with_data == 0 && events == 0 {
        reasons.push("no layer and no event covers the subject in the window".into());
        "insufficient"
    } else if coverage >= 0.7 && day_share >= 0.8 && corroborated_ok {
        "high"
    } else if coverage >= 0.4 && day_share >= 0.5 {
        "medium"
    } else {
        "low"
    };
    Confidence {
        level: level.into(),
        coverage: round(coverage.clamp(0.0, 1.0), 4),
        days_with_data,
        days_in_window,
        events,
        events_corroborated,
        reasons,
    }
}

impl Confidence {
    /// Lower the level by one step with a reason (never below `low`; `insufficient` stays).
    pub fn lower(&mut self, why: &str) {
        let next = match self.level.as_str() {
            "high" => "medium",
            "medium" => "low",
            other => other,
        };
        if next != self.level {
            self.level = next.into();
            self.reasons.push(format!("lowered: {why}"));
        } else {
            self.reasons.push(format!("already {}: {why}", self.level));
        }
    }
}

/// The data window of a briefing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DataWindow {
    pub from_utc: String,
    pub to_utc: String,
    pub days_in_window: usize,
    /// Layer days inside the window.
    pub layer_days: Vec<String>,
    pub events_in_window: usize,
    /// The latest observation time the briefing rests on (layer day end or event end).
    #[serde(default)]
    pub latest_data_utc: Option<String>,
}

/// Days from `a` to `b` (inclusive of partial days), at least 1.
pub fn days_between(a: f64, b: f64) -> usize {
    (((b - a) / 86_400.0).ceil() as i64).max(1) as usize
}

/// HTML escape.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The page head shared by the HTML outputs.
pub fn html_head(title: &str) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;margin:2em;max-width:72em}}table{{border-collapse:collapse;margin:.5em 0}}\
         td,th{{border:1px solid #bbb;padding:.25em .5em;text-align:left;vertical-align:top}}.l{{color:#555}}\
         .c-high{{color:#165c2a}}.c-medium{{color:#7a5b00}}.c-low,.c-insufficient{{color:#8a1c1c}}</style></head><body>",
        esc(title)
    )
}

/// ASCII text for the standard PDF fonts.
pub fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' => '-',
            '\u{00b0}' => 'd',
            '\u{2192}' => '>',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

/// A plain PDF of headed sections of paragraphs (the branded layout).
pub fn simple_pdf(
    title: &str,
    footer: &str,
    mark_id: &str,
    sections: &[(String, Vec<String>)],
) -> Result<Vec<u8>, String> {
    use crate::report_brand as brand;
    use crate::report_pdf::{rgb, sans, serif_bold, wrap, Layout};
    let mut l = Layout::new();
    l.cover_lockup();
    for line in wrap(&ascii(title), 170.0, 18.0) {
        l.ensure(10.0);
        l.text_at(20.0, &line, serif_bold(), 18.0, rgb(brand::INK));
        l.advance(8.5);
    }
    for (h, paras) in sections {
        l.heading(&ascii(h));
        for p in paras {
            l.paragraph(&ascii(p), sans(), 9.0, 4.4, rgb(brand::INK));
        }
    }
    let mark = crate::brand_pdf::PdfMark::new(ascii(title), mark_id.to_string());
    let pages = l.finish(&ascii(footer), &mark);
    crate::brand_pdf::save(&ascii(title), pages)
}

/// Write `name` as pretty JSON in `dir`.
pub fn write_json<T: Serialize>(dir: &Path, name: &str, v: &T) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(
        dir.join(name),
        serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| format!("{name}: {e}"))
}

/// Write a text file in `dir`.
pub fn write_text(dir: &Path, name: &str, text: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))
}

/// Read a TOML request; returns it with its source text.
pub fn read_request<T: serde::de::DeserializeOwned>(path: &Path) -> Result<(T, String), String> {
    let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let r: T = toml::from_str(&src).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((r, src))
}

/// Parse an ISO-8601 UTC time or fail naming the field.
pub fn utc(field: &str, s: &str) -> Result<f64, String> {
    crate::forensics::inputs::parse_utc(s)
        .ok_or_else(|| format!("{field}: not an ISO-8601 UTC time"))
}

/// ISO-8601 UTC of Unix seconds.
pub fn iso(t: f64) -> String {
    crate::forensics::inputs::iso(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sig9_keeps_nine_significant_digits() {
        assert_eq!(sig9(1.234_567_891_23), 1.234_567_89);
        assert_eq!(sig9(123_456_789_123.0), 123_456_789_000.0);
        assert_eq!(sig9(0.0), 0.0);
        assert_eq!(sig9(-0.000_123_456_789_99), -0.000_123_456_79);
    }

    #[test]
    fn confidence_levels_follow_the_method() {
        assert_eq!(assess(0.0, 0, 7, 0, 0).level, "insufficient");
        assert_eq!(assess(0.9, 7, 7, 2, 1).level, "high");
        assert_eq!(assess(0.9, 7, 7, 3, 1).level, "medium");
        assert_eq!(assess(0.5, 4, 7, 0, 0).level, "medium");
        assert_eq!(assess(0.2, 7, 7, 0, 0).level, "low");
        let mut c = assess(0.9, 7, 7, 0, 0);
        c.lower("test");
        assert_eq!(c.level, "medium");
    }
}
