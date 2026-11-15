// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Reading and hashing the inputs of an incident: every file is read once, its SHA-256 is
//! recorded, and the AIS and ADS-B position reports are parsed from the open engine's
//! documented CSV formats (the same column names its interference map reads).

use std::collections::BTreeMap;

use kshana::api::KshanaError;
use kshana::interference_map::time::parse_timestamp;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::IncidentScenario;

/// The SHA-256 of `b`, lower-case hex.
pub fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// What an input file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputRole {
    ReceiverLog,
    RinexNav,
    Ais,
    Adsb,
}

impl InputRole {
    pub fn as_str(self) -> &'static str {
        match self {
            InputRole::ReceiverLog => "receiver-log",
            InputRole::RinexNav => "rinex-nav",
            InputRole::Ais => "ais",
            InputRole::Adsb => "adsb",
        }
    }
    /// The evidence class of the data in such a file.
    pub fn data_class(self) -> &'static str {
        match self {
            // The receiver's own observables (C/N0, AGC, its fix as it computed it).
            InputRole::ReceiverLog | InputRole::RinexNav => "measured",
            // Positions broadcast by the vessel or aircraft and relayed by a network.
            InputRole::Ais | InputRole::Adsb => "reported",
        }
    }
}

/// One input file as the report records it.
#[derive(Debug, Clone, Serialize)]
pub struct InputRecord {
    pub role: InputRole,
    /// The scenario's label for it.
    pub label: String,
    /// The file name inside the bundle (`inputs/<name>`), unique per bundle.
    pub file: String,
    pub sha256: String,
    pub bytes: usize,
    /// `measured` or `reported` ([`InputRole::data_class`]).
    pub data_class: &'static str,
    /// First and last time stamp in the file, ISO-8601 UTC, where it has absolute times.
    pub first_utc: Option<String>,
    pub last_utc: Option<String>,
    /// Share of the incident window the file's time span covers, 0 to 1.
    pub window_coverage: Option<f64>,
    /// The largest gap between consecutive time stamps inside the window, s.
    pub largest_gap_s: Option<f64>,
}

/// The bytes of one input, kept for the analysis and the bundle.
#[derive(Debug, Clone)]
pub struct LoadedInput {
    pub record: InputRecord,
    pub data: Vec<u8>,
}

fn base_name(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_string()
}

/// Read every file the scenario names, in scenario order. File names in the bundle are made
/// unique (`2-name` for a second file with the same base name).
pub fn load_all(scn: &IncidentScenario) -> Result<Vec<LoadedInput>, KshanaError> {
    let mut wanted: Vec<(InputRole, String, String)> = Vec::new();
    for r in &scn.receivers {
        wanted.push((InputRole::ReceiverLog, r.label.clone(), r.path.clone()));
        if let Some(n) = &r.nav {
            wanted.push((
                InputRole::RinexNav,
                format!("{} (navigation)", r.label),
                n.clone(),
            ));
        }
    }
    for a in &scn.ais {
        wanted.push((InputRole::Ais, a.label.clone(), a.path.clone()));
    }
    for a in &scn.adsb {
        wanted.push((InputRole::Adsb, a.label.clone(), a.path.clone()));
    }
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let mut out = Vec::new();
    for (role, label, path) in wanted {
        let full = scn.resolve(&path);
        let data = std::fs::read(&full)
            .map_err(|e| KshanaError::InvalidInput(format!("{label}: {}: {e}", full.display())))?;
        let base = base_name(&path);
        let n = used.entry(base.clone()).or_insert(0);
        *n += 1;
        let file = if *n == 1 { base } else { format!("{n}-{base}") };
        out.push(LoadedInput {
            record: InputRecord {
                role,
                label,
                file,
                sha256: sha256_hex(&data),
                bytes: data.len(),
                data_class: role.data_class(),
                first_utc: None,
                last_utc: None,
                window_coverage: None,
                largest_gap_s: None,
            },
            data,
        });
    }
    Ok(out)
}

/// Fill in the time coverage of a record from its sorted time stamps (Unix s).
pub fn set_coverage(rec: &mut InputRecord, times: &[f64], w0: f64, w1: f64) {
    if times.is_empty() {
        return;
    }
    let (first, last) = (times[0], times[times.len() - 1]);
    rec.first_utc = Some(iso(first));
    rec.last_utc = Some(iso(last));
    let span = (w1 - w0).max(1e-9);
    let cov = ((last.min(w1) - first.max(w0)).max(0.0) / span).clamp(0.0, 1.0);
    rec.window_coverage = Some(round(cov, 3));
    let inside: Vec<f64> = times
        .iter()
        .copied()
        .filter(|t| *t >= w0 && *t <= w1)
        .collect();
    rec.largest_gap_s = inside
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(None, |m: Option<f64>, g| Some(m.map_or(g, |m| m.max(g))))
        .map(|g| round(g, 1));
}

/// Round to `d` decimals (for stable, readable JSON).
pub fn round(x: f64, d: i32) -> f64 {
    let f = 10f64.powi(d);
    (x * f).round() / f
}

/// Unix seconds to ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`, fractional seconds kept to 0.1 s
/// when present).
pub fn iso(t: f64) -> String {
    let day = (t / 86_400.0).floor();
    let (y, m, d) = kshana::interference_map::time::civil_from_days(day as i64);
    let sod = t - day * 86_400.0;
    let tenths = (sod * 10.0).round() as i64;
    let (hh, rem) = (tenths / 36_000, tenths % 36_000);
    let (mm, rem) = (rem / 600, rem % 600);
    let (ss, frac) = (rem / 10, rem % 10);
    if frac == 0 {
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
    } else {
        format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.{frac}Z")
    }
}

/// Parse an ISO-8601 UTC time stamp (or Unix seconds) to Unix seconds.
pub fn parse_utc(s: &str) -> Option<f64> {
    let s = s.trim();
    // Drop fractional seconds the shared parser does not split off ("…:03.000Z").
    parse_timestamp(s).or_else(|| {
        let core = s.strip_suffix('Z').unwrap_or(s);
        let (head, frac) = core.rsplit_once('.')?;
        let f: f64 = format!("0.{frac}").parse().ok()?;
        parse_timestamp(head).map(|t| t + f)
    })
}

/// One position report from AIS or ADS-B.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionReport {
    pub t: f64,
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    /// AIS speed over ground, knots (102.3 "not available" is dropped).
    pub sog_kn: Option<f64>,
    /// ADS-B barometric altitude, feet.
    pub alt_ft: Option<f64>,
    /// ADS-B navigation integrity category.
    pub nic: Option<u8>,
    /// ADS-B navigation accuracy category for position.
    pub nacp: Option<u8>,
}

/// A parsed CSV file: reports by identifier, each sorted by time, and the rows refused.
#[derive(Debug, Clone, Default)]
pub struct Tracks {
    pub by_id: BTreeMap<String, Vec<PositionReport>>,
    pub rows: usize,
    pub refused: usize,
}

impl Tracks {
    /// Every time stamp, sorted.
    pub fn times(&self) -> Vec<f64> {
        let mut t: Vec<f64> = self.by_id.values().flatten().map(|r| r.t).collect();
        t.sort_by(f64::total_cmp);
        t
    }
}

fn header_index(h: &[&str], name: &str) -> Option<usize> {
    h.iter().position(|c| c.trim().eq_ignore_ascii_case(name))
}

/// Parse the documented AIS (`id_col = "vessel_id"`) or ADS-B (`"aircraft_id"`) CSV.
pub fn parse_tracks(text: &str, id_col: &str) -> Result<Tracks, KshanaError> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head: Vec<&str> = lines
        .next()
        .ok_or_else(|| KshanaError::InvalidInput("empty CSV".into()))?
        .split(',')
        .collect();
    let need = |n: &str| {
        header_index(&head, n)
            .ok_or_else(|| KshanaError::InvalidInput(format!("CSV has no '{n}' column")))
    };
    let (ct, cid, clat, clon) = (
        need("timestamp")?,
        need(id_col)?,
        need("lat")?,
        need("lon")?,
    );
    let (csog, calt, cnic, cnacp) = (
        header_index(&head, "sog_kn"),
        header_index(&head, "alt_baro_ft"),
        header_index(&head, "nic"),
        header_index(&head, "nacp"),
    );
    let mut out = Tracks::default();
    for line in lines {
        out.rows += 1;
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        let get = |k: usize| f.get(k).copied().unwrap_or("");
        let num = |k: Option<usize>| k.and_then(|k| get(k).parse::<f64>().ok());
        let (Some(t), Ok(lat), Ok(lon)) = (
            parse_utc(get(ct)),
            get(clat).parse::<f64>(),
            get(clon).parse::<f64>(),
        ) else {
            out.refused += 1;
            continue;
        };
        let id = get(cid);
        if id.is_empty() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            out.refused += 1;
            continue;
        }
        let sog = num(csog).filter(|v| *v >= 0.0 && (*v - 102.3).abs() > 1e-6);
        out.by_id
            .entry(id.to_string())
            .or_default()
            .push(PositionReport {
                t,
                id: id.to_string(),
                lat,
                lon,
                sog_kn: sog,
                alt_ft: num(calt),
                nic: num(cnic)
                    .filter(|v| (0.0..=15.0).contains(v))
                    .map(|v| v as u8),
                nacp: num(cnacp)
                    .filter(|v| (0.0..=15.0).contains(v))
                    .map(|v| v as u8),
            });
    }
    for v in out.by_id.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trips() {
        let t = parse_utc("2026-05-14T06:05:30Z").unwrap();
        assert_eq!(iso(t), "2026-05-14T06:05:30Z");
        assert_eq!(iso(t + 0.5), "2026-05-14T06:05:30.5Z");
        assert_eq!(parse_utc("2026-05-14T06:05:30.250Z"), Some(t + 0.25));
    }

    #[test]
    fn tracks_parse_and_refuse() {
        let csv = "timestamp,vessel_id,lat,lon,sog_kn\n\
                   2026-03-01T10:02:00Z,A,30.1,-40.39,12\n\
                   2026-03-01T10:00:00Z,A,30.1,-40.40,102.3\n\
                   bad,A,30.1,-40.4,1\n";
        let t = parse_tracks(csv, "vessel_id").unwrap();
        assert_eq!((t.rows, t.refused), (3, 1));
        let a = &t.by_id["A"];
        assert!(a[0].t < a[1].t);
        assert_eq!(a[0].sog_kn, None);
        assert!(parse_tracks("timestamp,lat\n", "vessel_id").is_err());
    }
}
