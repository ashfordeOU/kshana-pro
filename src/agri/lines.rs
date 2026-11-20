// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Guidance lines: a neutral form, converters, and realignment.
//!
//! **Neutral form** (`kshana-guidance-lines/1`, JSON): a list of lines, each with a name, a
//! kind (`ab` for a straight A-B line, `curve` for a polyline), its points (latitude,
//! longitude in degrees, WGS 84) and an optional swath width.
//!
//! **Converters.** GeoJSON (`LineString`, `MultiLineString`, a `Feature` or a
//! `FeatureCollection`), KML (`<LineString><coordinates>`), CSV (`name,lat,lon` rows, a line
//! per name), an A point with a heading, and a subset of the ISO 11783-10 task-data file
//! (ISOXML): guidance patterns (`GPN`, attribute `C` the pattern type: 1 A-B, 2 A+heading,
//! 3 curve) carrying a line string (`LSG A="5"`) of points (`PNT`, `C` latitude, `D`
//! longitude); an A+ pattern's heading is attribute `G`. The subset was written from the
//! published element and attribute structure; it has not been imported into any terminal or
//! farm-management system, so check the result in yours. Proprietary in-cab formats with no
//! public description are not read.
//!
//! **Realignment.** After an outage or a change of correction source, stored lines can sit
//! off the rows actually planted. Driving along existing rows (or re-measuring markers whose
//! stored position is known) gives offsets; the least-squares horizontal shift that best
//! explains them is applied to every line. With only parallel lines the along-line component
//! is not observable and is reported as such (no shift applied along the lines). Rotation is
//! not estimated: residuals larger than the tolerance are reported so a rotation or a
//! distortion is not hidden.

use serde::{Deserialize, Serialize};

use super::geo::{azimuth_deg, unit, LocalFrame};
use super::log::{Fix, MachineLog};

/// The neutral format name.
pub const FORMAT: &str = "kshana-guidance-lines/1";

/// A line's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Ab,
    Curve,
}

/// One guidance line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GuidanceLine {
    pub name: String,
    pub kind: LineKind,
    /// Points as `[lat, lon]` degrees.
    pub points: Vec<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_m: Option<f64>,
}

impl GuidanceLine {
    /// An A-B line from an A point, a heading (deg from true north) and a length (m).
    pub fn from_heading(name: &str, a: (f64, f64), heading_deg: f64, length_m: f64) -> Self {
        let f = LocalFrame::new(a.0, a.1, 0.0);
        let u = unit(heading_deg);
        let b = f.latlon(u[0] * length_m, u[1] * length_m);
        GuidanceLine {
            name: name.into(),
            kind: LineKind::Ab,
            points: vec![[a.0, a.1], [b.0, b.1]],
            width_m: None,
        }
    }

    /// Heading (deg) from the first to the last point.
    pub fn heading_deg(&self) -> f64 {
        let (a, b) = (self.points[0], self.points[self.points.len() - 1]);
        let f = LocalFrame::new(a[0], a[1], 0.0);
        let p = f.en(b[0], b[1]);
        azimuth_deg(p[0], p[1])
    }

    /// Length (m) along the points.
    pub fn length_m(&self) -> f64 {
        let f = LocalFrame::new(self.points[0][0], self.points[0][1], 0.0);
        self.points
            .windows(2)
            .map(|w| {
                let (a, b) = (f.en(w[0][0], w[0][1]), f.en(w[1][0], w[1][1]));
                ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
            })
            .sum()
    }

    fn check(&self) -> Result<(), String> {
        if self.points.len() < 2 {
            return Err(format!("line '{}' needs at least two points", self.name));
        }
        if self.kind == LineKind::Ab && self.points.len() != 2 {
            return Err(format!("A-B line '{}' needs exactly two points", self.name));
        }
        for p in &self.points {
            if !(-90.0..=90.0).contains(&p[0]) || !(-180.0..=180.0).contains(&p[1]) {
                return Err(format!("line '{}': point {p:?} out of range", self.name));
            }
        }
        Ok(())
    }
}

/// A set of lines in the neutral form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineSet {
    pub format: String,
    pub lines: Vec<GuidanceLine>,
}

/// Input and output formats of the converter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Neutral,
    GeoJson,
    Kml,
    IsoXml,
    Csv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "json" | "neutral" | "kshana" => Format::Neutral,
            "geojson" => Format::GeoJson,
            "kml" => Format::Kml,
            "isoxml" | "xml" | "taskdata" => Format::IsoXml,
            "csv" => Format::Csv,
            o => {
                return Err(format!(
                    "unknown line format '{o}' (json, geojson, kml, isoxml, csv)"
                ))
            }
        })
    }

    /// Guess from content.
    pub fn sniff(text: &str) -> Result<Format, String> {
        let t = text.trim_start();
        if t.contains("ISO11783_TaskData") || t.contains("<GPN") {
            Ok(Format::IsoXml)
        } else if t.contains("<kml") || t.contains("<LineString") {
            Ok(Format::Kml)
        } else if t.starts_with('{') && t.contains(FORMAT) {
            Ok(Format::Neutral)
        } else if t.starts_with('{') {
            Ok(Format::GeoJson)
        } else if t.lines().next().is_some_and(|l| l.contains(',')) {
            Ok(Format::Csv)
        } else {
            Err("cannot tell the line format; pass --from".into())
        }
    }
}

impl LineSet {
    pub fn new(lines: Vec<GuidanceLine>) -> Self {
        LineSet {
            format: FORMAT.into(),
            lines,
        }
    }

    /// Read lines in `fmt` (or sniffed).
    pub fn read(text: &str, fmt: Option<Format>) -> Result<LineSet, String> {
        let fmt = match fmt {
            Some(f) => f,
            None => Format::sniff(text)?,
        };
        let set = match fmt {
            Format::Neutral => {
                let s: LineSet = serde_json::from_str(text).map_err(|e| e.to_string())?;
                if s.format != FORMAT {
                    return Err(format!("format '{}' is not {FORMAT}", s.format));
                }
                s
            }
            Format::GeoJson => from_geojson(text)?,
            Format::Kml => from_kml(text)?,
            Format::IsoXml => from_isoxml(text)?,
            Format::Csv => from_csv(text)?,
        };
        if set.lines.is_empty() {
            return Err("no guidance lines found".into());
        }
        for l in &set.lines {
            l.check()?;
        }
        Ok(set)
    }

    /// Write in `fmt`.
    pub fn write(&self, fmt: Format) -> String {
        match fmt {
            Format::Neutral => serde_json::to_string_pretty(self).unwrap(),
            Format::GeoJson => to_geojson(self),
            Format::Kml => to_kml(self),
            Format::IsoXml => to_isoxml(self),
            Format::Csv => to_csv(self),
        }
    }
}

fn kind_of(n: usize) -> LineKind {
    if n == 2 {
        LineKind::Ab
    } else {
        LineKind::Curve
    }
}

fn from_geojson(text: &str) -> Result<LineSet, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("GeoJSON: {e}"))?;
    let mut out = Vec::new();
    fn coords(c: &serde_json::Value) -> Result<Vec<[f64; 2]>, String> {
        c.as_array()
            .ok_or("GeoJSON: coordinates not an array")?
            .iter()
            .map(|p| {
                let lon = p
                    .get(0)
                    .and_then(|x| x.as_f64())
                    .ok_or("GeoJSON: bad point")?;
                let lat = p
                    .get(1)
                    .and_then(|x| x.as_f64())
                    .ok_or("GeoJSON: bad point")?;
                Ok([lat, lon])
            })
            .collect()
    }
    fn walk(
        v: &serde_json::Value,
        props: Option<&serde_json::Value>,
        out: &mut Vec<GuidanceLine>,
    ) -> Result<(), String> {
        let name = |i: usize| {
            props
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("line-{}", i + 1))
        };
        let width = props
            .and_then(|p| p.get("width_m"))
            .and_then(|w| w.as_f64());
        match v.get("type").and_then(|t| t.as_str()) {
            Some("FeatureCollection") => {
                for f in v
                    .get("features")
                    .and_then(|f| f.as_array())
                    .ok_or("GeoJSON: no features")?
                {
                    walk(f, None, out)?;
                }
            }
            Some("Feature") => {
                if let Some(g) = v.get("geometry") {
                    walk(g, v.get("properties"), out)?;
                }
            }
            Some("LineString") => {
                let pts = coords(v.get("coordinates").ok_or("GeoJSON: no coordinates")?)?;
                out.push(GuidanceLine {
                    name: name(out.len()),
                    kind: kind_of(pts.len()),
                    points: pts,
                    width_m: width,
                });
            }
            Some("MultiLineString") => {
                for (k, c) in v
                    .get("coordinates")
                    .and_then(|c| c.as_array())
                    .ok_or("GeoJSON: no coordinates")?
                    .iter()
                    .enumerate()
                {
                    let pts = coords(c)?;
                    out.push(GuidanceLine {
                        name: format!("{}-{}", name(out.len()), k + 1),
                        kind: kind_of(pts.len()),
                        points: pts,
                        width_m: width,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }
    walk(&v, None, &mut out)?;
    Ok(LineSet::new(out))
}

fn to_geojson(s: &LineSet) -> String {
    let features: Vec<serde_json::Value> = s
        .lines
        .iter()
        .map(|l| {
            let mut props = serde_json::json!({"name": l.name, "kind": l.kind});
            if let Some(w) = l.width_m {
                props["width_m"] = w.into();
            }
            serde_json::json!({
                "type": "Feature",
                "properties": props,
                "geometry": {
                    "type": "LineString",
                    "coordinates": l.points.iter().map(|p| [p[1], p[0]]).collect::<Vec<_>>(),
                }
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "type": "FeatureCollection",
        "features": features,
    }))
    .unwrap()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// The text between `<tag>` (with any attributes) and `</tag>`, for each occurrence.
fn elements<'a>(text: &'a str, tag: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = text;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        if !after.starts_with(['>', ' ', '\n', '\t', '\r', '/']) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        if after[..gt].ends_with('/') {
            out.push("");
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(j) = body.find(&close) else { break };
        out.push(&body[..j]);
        rest = &body[j + close.len()..];
    }
    out
}

fn from_kml(text: &str) -> Result<LineSet, String> {
    let mut out = Vec::new();
    let marks = elements(text, "Placemark");
    let blocks: Vec<(&str, Option<String>)> = if marks.is_empty() {
        vec![(text, None)]
    } else {
        marks
            .iter()
            .map(|m| {
                (
                    *m,
                    elements(m, "name").first().map(|n| xml_unescape(n.trim())),
                )
            })
            .collect()
    };
    for (b, name) in blocks {
        for (k, ls) in elements(b, "LineString").into_iter().enumerate() {
            let c = elements(ls, "coordinates");
            let c = c.first().ok_or("KML: LineString without coordinates")?;
            let pts: Vec<[f64; 2]> = c
                .split_whitespace()
                .map(|t| {
                    let v: Vec<f64> = t.split(',').filter_map(|x| x.parse().ok()).collect();
                    if v.len() < 2 {
                        Err(format!("KML: bad coordinate '{t}'"))
                    } else {
                        Ok([v[1], v[0]])
                    }
                })
                .collect::<Result<_, _>>()?;
            let base = name
                .clone()
                .unwrap_or_else(|| format!("line-{}", out.len() + 1));
            out.push(GuidanceLine {
                name: if k == 0 {
                    base
                } else {
                    format!("{base}-{}", k + 1)
                },
                kind: kind_of(pts.len()),
                points: pts,
                width_m: None,
            });
        }
    }
    Ok(LineSet::new(out))
}

fn to_kml(s: &LineSet) -> String {
    let mut k = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n",
    );
    for l in &s.lines {
        let c: Vec<String> = l
            .points
            .iter()
            .map(|p| format!("{:.9},{:.9},0", p[1], p[0]))
            .collect();
        k.push_str(&format!(
            "<Placemark><name>{}</name><LineString><coordinates>{}</coordinates></LineString></Placemark>\n",
            xml_escape(&l.name),
            c.join(" ")
        ));
    }
    k.push_str("</Document>\n</kml>\n");
    k
}

/// Attributes of the first tag in `text` (an element's opening tag).
fn attrs(tag_text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = tag_text;
    while let Some(eq) = rest.find("=\"") {
        let name = rest[..eq]
            .rsplit(|c: char| c.is_whitespace() || c == '<')
            .next()
            .unwrap_or("")
            .to_string();
        let after = &rest[eq + 2..];
        let Some(end) = after.find('"') else { break };
        out.push((name, xml_unescape(&after[..end])));
        rest = &after[end + 1..];
    }
    out
}

fn attr<'a>(a: &'a [(String, String)], k: &str) -> Option<&'a str> {
    a.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
}

/// Opening tags `<TAG ...>` (or self-closing) of `tag` in `text`, with their attribute text.
fn open_tags<'a>(text: &'a str, tag: &str) -> Vec<(usize, &'a str)> {
    let open = format!("<{tag}");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find(&open) {
        let s = from + i;
        let after = &text[s + open.len()..];
        if after.starts_with([' ', '>', '/', '\n', '\t', '\r']) {
            if let Some(gt) = after.find('>') {
                out.push((s, &after[..gt]));
            }
        }
        from = s + open.len();
    }
    out
}

fn from_isoxml(text: &str) -> Result<LineSet, String> {
    let gpns = open_tags(text, "GPN");
    let mut out = Vec::new();
    for (k, (start, head)) in gpns.iter().enumerate() {
        let end = gpns.get(k + 1).map(|g| g.0).unwrap_or(text.len());
        let body = &text[*start..end];
        let a = attrs(head);
        let name = attr(&a, "B")
            .or(attr(&a, "A"))
            .map(String::from)
            .unwrap_or_else(|| format!("GPN{}", k + 1));
        let ptype = attr(&a, "C").unwrap_or("1");
        // Points of the guidance line string (LSG A="5"), else every PNT in the pattern.
        let lsgs = open_tags(body, "LSG");
        let mut pts = Vec::new();
        for (j, (ls, lh)) in lsgs.iter().enumerate() {
            if attr(&attrs(lh), "A") != Some("5") {
                continue;
            }
            let le = lsgs.get(j + 1).map(|g| g.0).unwrap_or(body.len());
            for (_, ph) in open_tags(&body[*ls..le], "PNT") {
                let pa = attrs(ph);
                let lat = attr(&pa, "C").and_then(|v| v.parse::<f64>().ok());
                let lon = attr(&pa, "D").and_then(|v| v.parse::<f64>().ok());
                match (lat, lon) {
                    (Some(la), Some(lo)) => pts.push([la, lo]),
                    _ => return Err(format!("ISOXML {name}: PNT without C/D coordinates")),
                }
            }
        }
        match ptype {
            "1" | "3" => {
                if pts.len() < 2 {
                    return Err(format!(
                        "ISOXML {name}: pattern type {ptype} needs two points"
                    ));
                }
                out.push(GuidanceLine {
                    name,
                    kind: if ptype == "1" {
                        LineKind::Ab
                    } else {
                        kind_of(pts.len())
                    },
                    points: if ptype == "1" {
                        vec![pts[0], pts[pts.len() - 1]]
                    } else {
                        pts
                    },
                    width_m: None,
                });
            }
            "2" => {
                let h: f64 = attr(&a, "G")
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| format!("ISOXML {name}: A+ pattern without heading (G)"))?;
                let a0 = pts
                    .first()
                    .ok_or_else(|| format!("ISOXML {name}: A+ pattern without a point"))?;
                out.push(GuidanceLine::from_heading(&name, (a0[0], a0[1]), h, 1000.0));
            }
            other => {
                return Err(format!(
                "ISOXML {name}: guidance pattern type {other} (pivot or spiral) is not converted"
            ))
            }
        }
    }
    Ok(LineSet::new(out))
}

fn to_isoxml(s: &LineSet) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ISO11783_TaskData VersionMajor=\"4\" VersionMinor=\"0\" DataTransferOrigin=\"1\" ManagementSoftwareManufacturer=\"kshana-pro\" ManagementSoftwareVersion=\"1\">\n<PFD A=\"PFD1\" C=\"field\" D=\"0\">\n<GGP A=\"GGP1\" B=\"kshana-pro\">\n",
    );
    for (i, l) in s.lines.iter().enumerate() {
        let ptype = if l.kind == LineKind::Ab { 1 } else { 3 };
        x.push_str(&format!(
            "<GPN A=\"GPN{}\" B=\"{}\" C=\"{ptype}\">\n<LSG A=\"5\">\n",
            i + 1,
            xml_escape(&l.name)
        ));
        let n = l.points.len();
        for (k, p) in l.points.iter().enumerate() {
            // PNT type 6 = guidance reference A, 7 = reference B, 9 = guidance point.
            let t = if k == 0 {
                6
            } else if k == n - 1 {
                7
            } else {
                9
            };
            x.push_str(&format!(
                "<PNT A=\"{t}\" C=\"{:.9}\" D=\"{:.9}\"/>\n",
                p[0], p[1]
            ));
        }
        x.push_str("</LSG>\n</GPN>\n");
    }
    x.push_str("</GGP>\n</PFD>\n</ISO11783_TaskData>\n");
    x
}

fn from_csv(text: &str) -> Result<LineSet, String> {
    let mut out: Vec<GuidanceLine> = Vec::new();
    for (i, l) in text.lines().map(str::trim).enumerate() {
        if l.is_empty() || l.starts_with('#') || l.to_ascii_lowercase().starts_with("name,") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() < 3 {
            return Err(format!("CSV line {}: expected name,lat,lon", i + 1));
        }
        let lat: f64 = f[1]
            .parse()
            .map_err(|_| format!("CSV line {}: lat", i + 1))?;
        let lon: f64 = f[2]
            .parse()
            .map_err(|_| format!("CSV line {}: lon", i + 1))?;
        match out.iter_mut().find(|g| g.name == f[0]) {
            Some(g) => g.points.push([lat, lon]),
            None => out.push(GuidanceLine {
                name: f[0].into(),
                kind: LineKind::Ab,
                points: vec![[lat, lon]],
                width_m: None,
            }),
        }
    }
    for g in &mut out {
        g.kind = kind_of(g.points.len());
    }
    Ok(LineSet::new(out))
}

fn to_csv(s: &LineSet) -> String {
    let mut c = String::from("name,lat,lon\n");
    for l in &s.lines {
        for p in &l.points {
            c.push_str(&format!("{},{:.9},{:.9}\n", l.name, p[0], p[1]));
        }
    }
    c
}

/// A re-measured marker: its stored position and where it is measured now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub stored: (f64, f64),
    pub now: (f64, f64),
}

impl Marker {
    /// `stored_lat,stored_lon:now_lat,now_lon`.
    pub fn parse(s: &str) -> Result<Marker, String> {
        let (a, b) = s
            .split_once(':')
            .ok_or_else(|| format!("marker '{s}': expected lat,lon:lat,lon"))?;
        Ok(Marker {
            stored: super::parse_latlon(a)?,
            now: super::parse_latlon(b)?,
        })
    }
}

/// Parameters of the realignment.
#[derive(Clone, Copy, Debug)]
pub struct RealignParams {
    /// Lowest fix quality used from the log.
    pub min_fix: Fix,
    /// An epoch further than this (m) from every stored line is not on a known row.
    pub capture_m: f64,
    /// Correlation time (s) of the log's cross-track errors, for the effective sample count.
    pub corr_time_s: f64,
    /// Residual above which a line is flagged (m).
    pub tolerance_m: f64,
}

impl Default for RealignParams {
    fn default() -> Self {
        RealignParams {
            min_fix: Fix::RtkFixed,
            capture_m: 1.0,
            corr_time_s: 10.0,
            tolerance_m: 0.05,
        }
    }
}

/// Per-line evidence of the realignment.
#[derive(Clone, Debug, Serialize)]
pub struct LineOffset {
    pub line: String,
    pub epochs: usize,
    /// Mean signed offset of the driven track from the stored line (m, left of the line
    /// direction positive).
    pub mean_offset_m: f64,
    pub std_m: f64,
    /// Residual after the shift (m).
    pub residual_m: f64,
    pub flagged: bool,
}

/// The realignment result.
#[derive(Clone, Debug, Serialize)]
pub struct Realignment {
    pub label: &'static str,
    /// Shift applied to every stored line (m east, m north).
    pub shift_east_m: f64,
    pub shift_north_m: f64,
    /// Standard uncertainty of the shift components (m; GUM type A, from the scatter).
    pub u_east_m: f64,
    pub u_north_m: f64,
    /// `full` (2-D shift observed) or `cross_track_only` (all observations parallel: the
    /// along-line component is not observable and no shift is applied along the lines).
    pub observability: String,
    pub lines: Vec<LineOffset>,
    pub markers: usize,
    pub epochs_used: usize,
    pub notes: Vec<String>,
    #[serde(skip)]
    pub realigned: Option<LineSet>,
}

/// Estimate the shift that puts `set` onto what `log` (driven along existing rows) and
/// `markers` show, and the realigned lines.
pub fn realign(
    set: &LineSet,
    log: Option<&MachineLog>,
    markers: &[Marker],
    p: &RealignParams,
) -> Result<Realignment, String> {
    let a0 = set.lines.first().ok_or("no lines")?.points[0];
    let f = LocalFrame::new(a0[0], a0[1], 0.0);
    // Each line's local segments.
    let segs: Vec<Vec<([f64; 2], [f64; 2])>> = set
        .lines
        .iter()
        .map(|l| {
            l.points
                .windows(2)
                .map(|w| (f.en(w[0][0], w[0][1]), f.en(w[1][0], w[1][1])))
                .collect()
        })
        .collect();
    // Normal equations N s = b over scalar observations n·s = d with weight w.
    let mut nm = [[0.0f64; 2]; 2];
    let mut bv = [0.0f64; 2];
    let mut obs: Vec<([f64; 2], f64, f64)> = Vec::new(); // normal, value, weight
    let mut per_line: Vec<Vec<(f64, f64, [f64; 2])>> = vec![Vec::new(); set.lines.len()]; // (t, d, n)
    let mut used = 0;
    if let Some(log) = log {
        for e in &log.epochs {
            if e.fix < p.min_fix || !e.fix.has_position() {
                continue;
            }
            let q = f.en(e.lat, e.lon);
            let mut best: Option<(usize, f64, [f64; 2])> = None;
            for (li, ss) in segs.iter().enumerate() {
                for (a, b) in ss {
                    let ab = [b[0] - a[0], b[1] - a[1]];
                    let l = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt();
                    if l == 0.0 {
                        continue;
                    }
                    let t = ((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1]) / (l * l);
                    if !(-0.02..=1.02).contains(&t) {
                        continue;
                    }
                    let n = [-ab[1] / l, ab[0] / l];
                    let d = (q[0] - a[0]) * n[0] + (q[1] - a[1]) * n[1];
                    if d.abs() <= p.capture_m && best.is_none_or(|b| d.abs() < b.1.abs()) {
                        best = Some((li, d, n));
                    }
                }
            }
            if let Some((li, d, n)) = best {
                per_line[li].push((e.t_s, d, n));
                used += 1;
            }
        }
    }
    let mut offsets = Vec::new();
    for (li, v) in per_line.iter().enumerate() {
        if v.is_empty() {
            continue;
        }
        let m = v.len() as f64;
        let mean = v.iter().map(|x| x.1).sum::<f64>() / m;
        let sd = if v.len() > 1 {
            (v.iter().map(|x| (x.1 - mean).powi(2)).sum::<f64>() / (m - 1.0)).sqrt()
        } else {
            0.0
        };
        let dur = v.last().unwrap().0 - v[0].0;
        let n_eff = (dur / p.corr_time_s).max(1.0);
        let var = (sd * sd / n_eff).max(1e-6); // floor at 1 mm standard deviation
        let nrm = [
            v.iter().map(|x| x.2[0]).sum::<f64>() / m,
            v.iter().map(|x| x.2[1]).sum::<f64>() / m,
        ];
        obs.push((nrm, mean, 1.0 / var));
        offsets.push((li, v.len(), mean, sd, nrm));
    }
    for mk in markers {
        let s = f.en(mk.stored.0, mk.stored.1);
        let q = f.en(mk.now.0, mk.now.1);
        let var = 0.02f64.powi(2); // a re-measured marker: 2 cm standard uncertainty assumed
        obs.push(([1.0, 0.0], q[0] - s[0], 1.0 / var));
        obs.push(([0.0, 1.0], q[1] - s[1], 1.0 / var));
    }
    if obs.is_empty() {
        return Err("nothing to realign from: no log epoch on a stored line and no marker".into());
    }
    for (n, d, w) in &obs {
        for i in 0..2 {
            for j in 0..2 {
                nm[i][j] += w * n[i] * n[j];
            }
            bv[i] += w * n[i] * d;
        }
    }
    let det = nm[0][0] * nm[1][1] - nm[0][1] * nm[1][0];
    let tr = nm[0][0] + nm[1][1];
    let mut notes = Vec::new();
    let (s, u, observability) = if det > 1e-6 * tr * tr {
        let inv = [
            [nm[1][1] / det, -nm[0][1] / det],
            [-nm[1][0] / det, nm[0][0] / det],
        ];
        let s = [
            inv[0][0] * bv[0] + inv[0][1] * bv[1],
            inv[1][0] * bv[0] + inv[1][1] * bv[1],
        ];
        (s, [inv[0][0].sqrt(), inv[1][1].sqrt()], "full")
    } else {
        // Parallel observations only: shift along the common normal.
        let n = obs[0].0;
        let wsum: f64 = obs.iter().map(|o| o.2).sum();
        let c = obs
            .iter()
            .map(|(on, d, w)| w * d * (on[0] * n[0] + on[1] * n[1]).signum())
            .sum::<f64>()
            / wsum;
        let uc = (1.0 / wsum).sqrt();
        notes.push(
            "All observations are along parallel lines: only the cross-track shift is observable; nothing is shifted along the lines. Drive one non-parallel row or add a marker for a full shift.".into(),
        );
        (
            [c * n[0], c * n[1]],
            [uc * n[0].abs(), uc * n[1].abs()],
            "cross_track_only",
        )
    };
    let lines: Vec<LineOffset> = offsets
        .iter()
        .map(|(li, cnt, mean, sd, n)| {
            let r = mean - (n[0] * s[0] + n[1] * s[1]);
            LineOffset {
                line: set.lines[*li].name.clone(),
                epochs: *cnt,
                mean_offset_m: super::round(*mean, 4),
                std_m: super::round(*sd, 4),
                residual_m: super::round(r, 4),
                flagged: r.abs() > p.tolerance_m,
            }
        })
        .collect();
    if lines.iter().any(|l| l.flagged) {
        notes.push(format!(
            "A line's residual exceeds {} m after the shift: the stored lines may be rotated or distorted relative to the rows, which a shift does not correct.",
            p.tolerance_m
        ));
    }
    let realigned = LineSet::new(
        set.lines
            .iter()
            .map(|l| GuidanceLine {
                points: l
                    .points
                    .iter()
                    .map(|pt| {
                        let q = f.en(pt[0], pt[1]);
                        let (la, lo) = f.latlon(q[0] + s[0], q[1] + s[1]);
                        [la, lo]
                    })
                    .collect(),
                ..l.clone()
            })
            .collect(),
    );
    Ok(Realignment {
        label: super::LABEL,
        shift_east_m: super::round(s[0], 4),
        shift_north_m: super::round(s[1], 4),
        u_east_m: super::round(u[0], 4),
        u_north_m: super::round(u[1], 4),
        observability: observability.into(),
        lines,
        markers: markers.len(),
        epochs_used: used,
        notes,
        realigned: Some(realigned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> LineSet {
        LineSet::new(vec![
            GuidanceLine::from_heading("AB1", (49.9, -97.1), 0.0, 400.0),
            GuidanceLine {
                name: "curve".into(),
                kind: LineKind::Curve,
                points: vec![[49.9, -97.1], [49.901, -97.099], [49.902, -97.099]],
                width_m: Some(12.0),
            },
        ])
    }

    fn close(a: &LineSet, b: &LineSet) {
        assert_eq!(a.lines.len(), b.lines.len());
        for (x, y) in a.lines.iter().zip(&b.lines) {
            assert_eq!(x.name, y.name);
            assert_eq!(x.kind, y.kind);
            for (p, q) in x.points.iter().zip(&y.points) {
                assert!((p[0] - q[0]).abs() < 1e-8 && (p[1] - q[1]).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn every_format_round_trips() {
        let s = set();
        for f in [
            Format::Neutral,
            Format::GeoJson,
            Format::Kml,
            Format::IsoXml,
            Format::Csv,
        ] {
            let t = s.write(f);
            assert_eq!(Format::sniff(&t).unwrap(), f, "{t}");
            close(&LineSet::read(&t, None).unwrap(), &s);
        }
    }

    #[test]
    fn heading_and_aplus() {
        let l = GuidanceLine::from_heading("x", (10.0, 20.0), 37.0, 500.0);
        assert!((l.heading_deg() - 37.0).abs() < 1e-6);
        assert!((l.length_m() - 500.0).abs() < 1e-3);
        let x = "<ISO11783_TaskData><GPN A=\"GPN1\" B=\"north\" C=\"2\" G=\"12.5\"><LSG A=\"5\"><PNT A=\"6\" C=\"10.0\" D=\"20.0\"/></LSG></GPN></ISO11783_TaskData>";
        let s = LineSet::read(x, None).unwrap();
        assert!((s.lines[0].heading_deg() - 12.5).abs() < 1e-6);
        let p = "<ISO11783_TaskData><GPN A=\"G\" C=\"4\"></GPN></ISO11783_TaskData>";
        assert!(LineSet::read(p, None).unwrap_err().contains("pivot"));
    }
}
