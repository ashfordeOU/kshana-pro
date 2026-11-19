// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Receiver log importers (D4) into one observation record, and comparisons.
//!
//! Every importer produces [`ObsRecord`]s: time (POSIX UTC seconds), chain and station,
//! ToA or TD, ECD, SNR, position fix, timing output and normalised status flags. Sources:
//!
//! * **(a) generic CSV** ([`CsvProfile`], TOML): delimiter, header, comment prefix, a column
//!   (name or 0-based index, with scale and offset) per target field, a time format, a
//!   status-column → flag map and constants (e.g. the chain of a single-chain log);
//! * **(b) NMEA-0183** ([`NmeaImporter`]): checksum-verified `$ttSSS,…*hh` sentences. Fix
//!   and time: `GGA`, `RMC`, `ZDA`, `GLL`. LORAN-C: `GLC` (geographic position, Loran-C:
//!   GRI, master TOA, up to five TDs with status), `LCD` (Loran-C signal data: GRI, master
//!   SNR/ECD, then SNR/ECD of up to five secondaries), `RMA` (recommended minimum Loran-C:
//!   status, position, TD A/B, speed, course). The LORAN sentence layouts were implemented
//!   from secondary descriptions of NMEA 0183 (public sentence summaries such as gpsd's
//!   *NMEA Revealed*), **not from the NMEA 0183 standard text**: field order, units (LCD
//!   SNR/ECD are "relative" there) and status letters (A valid, B blink, C cycle warning,
//!   S SNR warning, V invalid) must be confirmed against NMEA 0183 and against each
//!   receiver's manual. LORAN sentences carry no time: they take the last time seen in a
//!   `GGA`/`RMC`/`ZDA`/`GLL` sentence (date from `RMC`/`ZDA`).
//! * **(c) proprietary eLoran receiver sentences** ([`SplitProfile`], TOML): no regular
//!   expressions; each record type is matched on a field value (e.g. field 0 = `$PXXX` and
//!   field 1 = `TOA`), fields are mapped by index, and a **repeat group** maps per-station
//!   blocks (`start`, `stride`, per-block field indices). Standard NMEA sentences in the
//!   same stream can be passed to the NMEA parser.
//! * **(d) 1PPS time-interval-counter CSV** ([`TicProfile`]): a CSV profile plus unit,
//!   sign convention (receiver − reference) and a fixed delay correction, into
//!   `timing_ns`; [`timing_stats`] summarises it.
//!
//! # Conventions
//!
//! * `toa_us`: arrival of the station's group relative to the **nominal emission of the
//!   chain's master group** of the same group number on the Loran epoch (µs, receiver
//!   clock). Receivers report TOA with their own reference; map it with scale/offset.
//! * `td_us`: arrival of secondary − arrival of master (same group), µs, including the
//!   emission delay (the LORAN-C TD convention).
//! * `timing_ns`: receiver timing output (e.g. 1PPS) − reference (ns).
//!
//! # Comparisons
//!
//! [`obs_from_truth`] and [`obs_from_measurements`] turn simulator truth and software
//! receiver measurements into the same records; [`compare_records`] matches two record
//! sets by chain/station (chain labels may be names or GRIs, station labels ids or
//! `TD1`…`TD5`, see [`canonical_key`]) and nearest time, and reports per-quantity
//! statistics (ToA, TD, ECD, SNR, horizontal position, timing).
//!
//! # Needs
//!
//! Real receiver logs (and their manuals) to confirm the NMEA LORAN layouts and to write
//! vendor profiles; nothing here has been run on a real receiver's output.

use crate::chain::Chain;
use crate::geo::{inverse, Geo};
use crate::rx::{nominal_emission_s, Measurement};
use crate::signal::Role;
use crate::time::AbsTime;
use crate::TruthRecord;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::{unix_ns_from_iso, unix_s_from_abs, unix_s_from_civil};

/// Normalised flag names.
pub mod flags {
    pub const BLINK: &str = "blink";
    pub const CYCLE_WARNING: &str = "cycle_warning";
    pub const SNR_WARNING: &str = "snr_warning";
    pub const INVALID: &str = "invalid";
    pub const NO_FIX: &str = "no_fix";
    pub const CYCLE_SLIP: &str = "cycle_slip";
}

/// One observation from any source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ObsRecord {
    /// Source tag (importer / file / receiver).
    pub source: String,
    /// POSIX UTC seconds, when known.
    pub t_unix_s: Option<f64>,
    pub chain: Option<String>,
    pub station: Option<String>,
    pub toa_us: Option<f64>,
    pub td_us: Option<f64>,
    pub ecd_us: Option<f64>,
    pub snr_db: Option<f64>,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    pub alt_m: Option<f64>,
    pub timing_ns: Option<f64>,
    pub flags: BTreeSet<String>,
    pub extra: BTreeMap<String, String>,
}

impl ObsRecord {
    pub fn has_fix(&self) -> bool {
        self.lat_deg.is_some() && self.lon_deg.is_some() && !self.flags.contains(flags::NO_FIX)
    }
    fn set(&mut self, key: &str, v: f64) -> Result<(), String> {
        match key {
            "toa_us" => self.toa_us = Some(v),
            "td_us" => self.td_us = Some(v),
            "ecd_us" => self.ecd_us = Some(v),
            "snr_db" => self.snr_db = Some(v),
            "lat_deg" => self.lat_deg = Some(v),
            "lon_deg" => self.lon_deg = Some(v),
            "alt_m" => self.alt_m = Some(v),
            "timing_ns" => self.timing_ns = Some(v),
            _ => return Err(format!("unknown numeric field {key:?}")),
        }
        Ok(())
    }
}

/// Import statistics.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImportReport {
    pub lines: usize,
    pub records: usize,
    pub skipped: usize,
    pub bad_checksum: usize,
    /// (1-based line number, message), first 100.
    pub errors: Vec<(usize, String)>,
}

impl ImportReport {
    fn err(&mut self, line: usize, msg: String) {
        if self.errors.len() < 100 {
            self.errors.push((line, msg));
        }
    }
}

// ---------------------------------------------------------------------------------------
// Field mapping shared by the CSV, split and TIC profiles

/// A column reference: header name or 0-based index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ColRef {
    Index(usize),
    Name(String),
}

/// A mapped field: a column, optionally with `value · scale + offset`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldSpec {
    Col(ColRef),
    Full {
        column: ColRef,
        #[serde(default = "one")]
        scale: f64,
        #[serde(default)]
        offset: f64,
    },
}

fn one() -> f64 {
    1.0
}

impl FieldSpec {
    fn col(&self) -> &ColRef {
        match self {
            FieldSpec::Col(c) => c,
            FieldSpec::Full { column, .. } => column,
        }
    }
    fn scale_offset(&self) -> (f64, f64) {
        match self {
            FieldSpec::Col(_) => (1.0, 0.0),
            FieldSpec::Full { scale, offset, .. } => (*scale, *offset),
        }
    }
}

/// Time column formats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeFormat {
    /// `YYYY-MM-DDTHH:MM:SS[.fff]Z` (also with a space instead of `T`).
    #[default]
    Iso8601,
    /// POSIX seconds.
    UnixS,
    /// GPS seconds since 1980-01-06 (converted with the leap-second table).
    GpsS,
    /// `HH:MM:SS[.fff]` or `HHMMSS[.fff]` on the profile's `date`.
    Hms,
    /// Seconds of day on the profile's `date`.
    SecondsOfDay,
}

/// Status column → flags.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlagMap {
    pub column: Option<ColRef>,
    /// Exact value → flag names (comma separated allowed).
    #[serde(default)]
    pub map: BTreeMap<String, String>,
    /// Per character of the value → flag (e.g. "BC" → blink + cycle warning).
    #[serde(default)]
    pub per_char: BTreeMap<String, String>,
}

/// Mapping of split fields into a record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Target field → column. Targets: time, chain, station, toa_us, td_us, ecd_us,
    /// snr_db, lat_deg, lon_deg, alt_m, timing_ns.
    #[serde(default)]
    pub fields: BTreeMap<String, FieldSpec>,
    #[serde(default)]
    pub time_format: TimeFormat,
    /// Date for `hms` / `seconds_of_day` (YYYY-MM-DD).
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub flags: Option<FlagMap>,
    /// Constant text fields (chain, station, source) applied to every record.
    #[serde(default)]
    pub constants: BTreeMap<String, String>,
}

const TARGETS: &[&str] = &[
    "time",
    "chain",
    "station",
    "toa_us",
    "td_us",
    "ecd_us",
    "snr_db",
    "lat_deg",
    "lon_deg",
    "alt_m",
    "timing_ns",
];

impl FieldMapping {
    fn validate(&self) -> Result<(), String> {
        for k in self.fields.keys() {
            if !TARGETS.contains(&k.as_str()) {
                return Err(format!("unknown target field {k:?} (one of {TARGETS:?})"));
            }
        }
        for k in self.constants.keys() {
            if !["chain", "station", "source"].contains(&k.as_str()) {
                return Err(format!("constant {k:?}: only chain, station, source"));
            }
        }
        if matches!(self.time_format, TimeFormat::Hms | TimeFormat::SecondsOfDay)
            && self.fields.contains_key("time")
            && self.date.is_none()
        {
            return Err("time_format hms/seconds_of_day needs `date`".into());
        }
        Ok(())
    }

    fn resolve(col: &ColRef, header: &[String]) -> Result<usize, String> {
        match col {
            ColRef::Index(i) => Ok(*i),
            ColRef::Name(n) => header
                .iter()
                .position(|h| h == n)
                .ok_or_else(|| format!("column {n:?} not in header {header:?}")),
        }
    }

    /// Apply to split fields (with an optional header for names).
    fn apply(&self, f: &[&str], header: &[String], rec: &mut ObsRecord) -> Result<(), String> {
        let get = |col: &ColRef| -> Result<Option<&str>, String> {
            let i = Self::resolve(col, header)?;
            Ok(f.get(i).map(|s| s.trim()).filter(|s| !s.is_empty()))
        };
        for (k, v) in &self.constants {
            match k.as_str() {
                "chain" => rec.chain = Some(v.clone()),
                "station" => rec.station = Some(v.clone()),
                _ => rec.source = v.clone(),
            }
        }
        for (k, spec) in &self.fields {
            let Some(s) = get(spec.col())? else { continue };
            match k.as_str() {
                "time" => {
                    rec.t_unix_s = Some(parse_time(s, self.time_format, self.date.as_deref())?)
                }
                "chain" => rec.chain = Some(s.to_string()),
                "station" => rec.station = Some(s.to_string()),
                _ => {
                    let v: f64 = s.parse().map_err(|e| format!("{k} = {s:?}: {e}"))?;
                    let (a, b) = spec.scale_offset();
                    rec.set(k, v * a + b)?;
                }
            }
        }
        if let Some(fm) = &self.flags {
            if let Some(col) = &fm.column {
                if let Some(s) = get(col)? {
                    if let Some(fl) = fm.map.get(s) {
                        rec.flags
                            .extend(fl.split(',').map(|x| x.trim().to_string()));
                    }
                    for c in s.chars() {
                        if let Some(fl) = fm.per_char.get(&c.to_string()) {
                            rec.flags.insert(fl.clone());
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Parse a time field to POSIX seconds.
pub fn parse_time(s: &str, fmt: TimeFormat, date: Option<&str>) -> Result<f64, String> {
    let day0 = || -> Result<f64, String> {
        let d = date.ok_or("time format needs a date")?;
        Ok(unix_ns_from_iso(&format!("{d}T00:00:00Z"))? as f64 * 1e-9)
    };
    match fmt {
        TimeFormat::Iso8601 => {
            let mut t = s.replace(' ', "T");
            if !t.ends_with('Z') {
                t.push('Z');
            }
            let ns = unix_ns_from_iso(&t)?;
            Ok(ns.div_euclid(1_000_000_000) as f64 + ns.rem_euclid(1_000_000_000) as f64 * 1e-9)
        }
        TimeFormat::UnixS => s.parse().map_err(|e| format!("{s:?}: {e}")),
        TimeFormat::GpsS => {
            let g: f64 = s.parse().map_err(|e| format!("{s:?}: {e}"))?;
            // GPS = TAI − 19 s; POSIX = UTC. Iterate the leap-second lookup once.
            let gps0 = unix_s_from_civil(1980, 1, 6, 0.0);
            let approx = gps0 + g;
            let leap = |u: f64| {
                let (y, m, d) = crate::time::civil_from_days((u / 86_400.0).floor() as i64);
                crate::time::tai_minus_utc_s(y, m, d) - 19.0
            };
            Ok(approx - leap(approx - leap(approx)))
        }
        TimeFormat::Hms => Ok(day0()? + parse_hms(s)?),
        TimeFormat::SecondsOfDay => {
            Ok(day0()? + s.parse::<f64>().map_err(|e| format!("{s:?}: {e}"))?)
        }
    }
}

/// `HH:MM:SS[.fff]` or `HHMMSS[.fff]` → seconds of day.
pub fn parse_hms(s: &str) -> Result<f64, String> {
    let bad = || format!("bad time of day {s:?}");
    let (h, m, sec) = if s.contains(':') {
        let p: Vec<&str> = s.split(':').collect();
        if p.len() != 3 {
            return Err(bad());
        }
        (p[0], p[1], p[2])
    } else {
        if s.len() < 6 {
            return Err(bad());
        }
        (&s[0..2], &s[2..4], &s[4..])
    };
    let h: f64 = h.parse().map_err(|_| bad())?;
    let m: f64 = m.parse().map_err(|_| bad())?;
    let sec: f64 = sec.parse().map_err(|_| bad())?;
    if h >= 24.0 || m >= 60.0 || sec >= 61.0 {
        return Err(bad());
    }
    Ok(h * 3600.0 + m * 60.0 + sec)
}

/// Split a delimited line, honouring double quotes (`""` inside quotes is a quote).
pub fn split_line(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if q {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    cur.push('"');
                    it.next();
                } else {
                    q = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            q = true;
        } else if c == delim {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

fn delim_char(s: &str) -> Result<char, String> {
    match s {
        "\\t" | "tab" => Ok('\t'),
        "space" | " " => Ok(' '),
        _ => {
            let mut c = s.chars();
            match (c.next(), c.next()) {
                (Some(x), None) => Ok(x),
                _ => Err(format!("delimiter must be one character, got {s:?}")),
            }
        }
    }
}

fn split_by(line: &str, d: char) -> Vec<String> {
    if d == ' ' {
        line.split_whitespace().map(String::from).collect()
    } else {
        split_line(line, d)
    }
}

// ---------------------------------------------------------------------------------------
// (a) Generic CSV

/// Generic CSV column profile (TOML).
///
/// ```toml
/// delimiter = ","
/// header = true
/// time_format = "iso8601"
/// [fields]
/// time = "utc"
/// chain = "gri"
/// station = "stn"
/// toa_us = { column = "toa_ns", scale = 0.001 }
/// snr_db = 5
/// [flags]
/// column = "status"
/// per_char = { B = "blink", C = "cycle_warning" }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsvProfile {
    #[serde(default = "d_delim")]
    pub delimiter: String,
    #[serde(default = "yes")]
    pub header: bool,
    #[serde(default = "d_comment")]
    pub comment: String,
    #[serde(default)]
    pub skip_lines: usize,
    #[serde(flatten)]
    pub mapping: FieldMapping,
}

fn d_delim() -> String {
    ",".into()
}
fn d_comment() -> String {
    "#".into()
}
fn yes() -> bool {
    true
}

impl CsvProfile {
    pub fn from_toml(src: &str) -> Result<CsvProfile, String> {
        let p: CsvProfile = toml::from_str(src).map_err(|e| format!("csv profile: {e}"))?;
        p.mapping.validate()?;
        delim_char(&p.delimiter)?;
        Ok(p)
    }

    /// Import a CSV text.
    pub fn import(
        &self,
        text: &str,
        source: &str,
    ) -> Result<(Vec<ObsRecord>, ImportReport), String> {
        self.mapping.validate()?;
        let d = delim_char(&self.delimiter)?;
        let mut rep = ImportReport::default();
        let mut header: Vec<String> = Vec::new();
        let mut out = Vec::new();
        let mut need_header = self.header;
        for (i, raw) in text.lines().enumerate().skip(self.skip_lines) {
            rep.lines += 1;
            let line = raw.trim_end_matches('\r');
            if line.trim().is_empty()
                || (!self.comment.is_empty() && line.trim_start().starts_with(&self.comment))
            {
                rep.skipped += 1;
                continue;
            }
            let f = split_by(line, d);
            if need_header {
                header = f.iter().map(|s| s.trim().to_string()).collect();
                need_header = false;
                continue;
            }
            let fr: Vec<&str> = f.iter().map(|s| s.as_str()).collect();
            let mut rec = ObsRecord {
                source: source.into(),
                ..Default::default()
            };
            match self.mapping.apply(&fr, &header, &mut rec) {
                Ok(()) => {
                    out.push(rec);
                    rep.records += 1;
                }
                Err(e) => {
                    rep.skipped += 1;
                    rep.err(i + 1, e);
                }
            }
        }
        Ok((out, rep))
    }
}

// ---------------------------------------------------------------------------------------
// (b) NMEA-0183

/// Verify and strip an NMEA checksum. `Ok(body)` without `$`/`!` and `*hh`; a sentence
/// without a checksum is accepted (`checked = false`).
pub fn nmea_body(line: &str) -> Result<(&str, bool), String> {
    let l = line.trim();
    let l = l
        .strip_prefix('$')
        .or_else(|| l.strip_prefix('!'))
        .ok_or("not an NMEA sentence")?;
    match l.rsplit_once('*') {
        Some((body, cs)) => {
            let want = u8::from_str_radix(cs.trim(), 16)
                .map_err(|_| format!("bad checksum field {cs:?}"))?;
            let got = body.bytes().fold(0u8, |a, b| a ^ b);
            if got != want {
                return Err(format!("checksum {got:02X} != {want:02X}"));
            }
            Ok((body, true))
        }
        None => Ok((l, false)),
    }
}

/// Append the NMEA checksum to a body (without `$`): returns `$body*hh`.
pub fn nmea_sentence(body: &str) -> String {
    let cs = body.bytes().fold(0u8, |a, b| a ^ b);
    format!("${body}*{cs:02X}")
}

/// `ddmm.mmmm` + hemisphere → signed degrees.
pub fn nmea_latlon(v: &str, hemi: &str, deg_digits: usize) -> Option<f64> {
    if v.len() < deg_digits + 2 {
        return None;
    }
    let d: f64 = v[..deg_digits].parse().ok()?;
    let m: f64 = v[deg_digits..].parse().ok()?;
    let x = d + m / 60.0;
    match hemi {
        "N" | "E" => Some(x),
        "S" | "W" => Some(-x),
        _ => None,
    }
}

/// NMEA-0183 importer (stateful: date and last time carry across sentences).
#[derive(Clone, Debug, Default)]
pub struct NmeaImporter {
    pub source: String,
    /// Station labels of TD1…TD5 (e.g. ["W","X","Y","Z"]); default `TD1`…`TD5`.
    pub secondaries: Vec<String>,
    /// Require a checksum on every sentence.
    pub require_checksum: bool,
    date: Option<(i64, i64, i64)>,
    tod: Option<f64>,
    pub report: ImportReport,
}

fn status_flags(s: &str, rec: &mut ObsRecord) {
    for c in s.chars() {
        match c {
            'B' => {
                rec.flags.insert(flags::BLINK.into());
            }
            'C' => {
                rec.flags.insert(flags::CYCLE_WARNING.into());
            }
            'S' => {
                rec.flags.insert(flags::SNR_WARNING.into());
            }
            'V' => {
                rec.flags.insert(flags::INVALID.into());
            }
            _ => {}
        }
    }
}

impl NmeaImporter {
    pub fn new(source: &str) -> NmeaImporter {
        NmeaImporter {
            source: source.into(),
            ..Default::default()
        }
    }
    /// Start date for logs whose first sentences carry no date (YYYY-MM-DD).
    pub fn with_date(mut self, date: &str) -> Result<NmeaImporter, String> {
        let p: Vec<i64> = date
            .split('-')
            .map(|x| x.parse::<i64>().map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        if p.len() != 3 {
            return Err(format!("date {date:?}"));
        }
        self.date = Some((p[0], p[1], p[2]));
        Ok(self)
    }

    fn now(&self) -> Option<f64> {
        let (y, m, d) = self.date?;
        Some(unix_s_from_civil(y, m, d, self.tod?))
    }

    fn rec(&self) -> ObsRecord {
        ObsRecord {
            source: self.source.clone(),
            t_unix_s: self.now(),
            ..Default::default()
        }
    }

    fn set_tod(&mut self, f: &str) {
        if let Ok(t) = parse_hms(f) {
            self.tod = Some(t);
        }
    }

    fn secondary(&self, i: usize) -> String {
        self.secondaries
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("TD{}", i + 1))
    }

    /// Parse one line; returns the records it produced.
    pub fn line(&mut self, line: &str) -> Vec<ObsRecord> {
        self.report.lines += 1;
        let ln = self.report.lines;
        if line.trim().is_empty() {
            self.report.skipped += 1;
            return vec![];
        }
        let (body, checked) = match nmea_body(line) {
            Ok(b) => b,
            Err(e) => {
                if e.starts_with("checksum") {
                    self.report.bad_checksum += 1;
                } else {
                    self.report.skipped += 1;
                }
                self.report.err(ln, e);
                return vec![];
            }
        };
        if self.require_checksum && !checked {
            self.report.skipped += 1;
            self.report.err(ln, "missing checksum".into());
            return vec![];
        }
        let f: Vec<&str> = body.split(',').collect();
        let tag = f[0];
        let kind = if tag.len() >= 5 && !tag.starts_with('P') {
            &tag[tag.len() - 3..]
        } else {
            tag
        };
        let g = |i: usize| f.get(i).copied().unwrap_or("").trim();
        let num = |i: usize| g(i).parse::<f64>().ok();
        let out = match kind {
            "GGA" => {
                self.set_tod(g(1));
                let mut r = self.rec();
                r.lat_deg = nmea_latlon(g(2), g(3), 2);
                r.lon_deg = nmea_latlon(g(4), g(5), 3);
                r.alt_m = num(9);
                let q = g(6);
                r.extra.insert("fix_quality".into(), q.into());
                if let Some(h) = num(8) {
                    r.extra.insert("hdop".into(), h.to_string());
                }
                if q.is_empty() || q == "0" {
                    r.flags.insert(flags::NO_FIX.into());
                }
                r.extra.insert("sentence".into(), "GGA".into());
                vec![r]
            }
            "RMC" => {
                self.set_tod(g(1));
                let d = g(9);
                if d.len() == 6 {
                    if let (Ok(dd), Ok(mm), Ok(yy)) = (
                        d[0..2].parse::<i64>(),
                        d[2..4].parse::<i64>(),
                        d[4..6].parse::<i64>(),
                    ) {
                        // two-digit year: 1980–2079
                        let y = if yy >= 80 { 1900 + yy } else { 2000 + yy };
                        self.date = Some((y, mm, dd));
                    }
                }
                let mut r = self.rec();
                r.lat_deg = nmea_latlon(g(3), g(4), 2);
                r.lon_deg = nmea_latlon(g(5), g(6), 3);
                if g(2) != "A" {
                    r.flags.insert(flags::NO_FIX.into());
                    r.flags.insert(flags::INVALID.into());
                }
                if let Some(v) = num(7) {
                    r.extra.insert("speed_kn".into(), v.to_string());
                }
                if let Some(v) = num(8) {
                    r.extra.insert("course_deg".into(), v.to_string());
                }
                r.extra.insert("sentence".into(), "RMC".into());
                vec![r]
            }
            "ZDA" => {
                self.set_tod(g(1));
                if let (Ok(d), Ok(m), Ok(y)) = (
                    g(2).parse::<i64>(),
                    g(3).parse::<i64>(),
                    g(4).parse::<i64>(),
                ) {
                    self.date = Some((y, m, d));
                }
                vec![]
            }
            "GLL" => {
                self.set_tod(g(5));
                let mut r = self.rec();
                r.lat_deg = nmea_latlon(g(1), g(2), 2);
                r.lon_deg = nmea_latlon(g(3), g(4), 3);
                if g(6) != "A" {
                    r.flags.insert(flags::NO_FIX.into());
                }
                r.extra.insert("sentence".into(), "GLL".into());
                vec![r]
            }
            "GLC" => {
                // GRI/10, master TOA + status, TD1..TD5 + status
                let chain = g(1).to_string();
                let mut v = Vec::new();
                if let Some(toa) = num(2) {
                    let mut r = self.rec();
                    r.chain = Some(chain.clone());
                    r.station = Some("M".into());
                    r.toa_us = Some(toa);
                    status_flags(g(3), &mut r);
                    r.extra.insert("sentence".into(), "GLC".into());
                    v.push(r);
                }
                for k in 0..5 {
                    if let Some(td) = num(4 + 2 * k) {
                        let mut r = self.rec();
                        r.chain = Some(chain.clone());
                        r.station = Some(self.secondary(k));
                        r.td_us = Some(td);
                        status_flags(g(5 + 2 * k), &mut r);
                        r.extra.insert("sentence".into(), "GLC".into());
                        v.push(r);
                    }
                }
                v
            }
            "LCD" => {
                // GRI/10, master SNR, master ECD, then (SNR, ECD) × 5 — "relative" units
                let chain = g(1).to_string();
                let mut v = Vec::new();
                for k in 0..6 {
                    let (snr, ecd) = (num(2 + 2 * k), num(3 + 2 * k));
                    if snr.is_none() && ecd.is_none() {
                        continue;
                    }
                    let mut r = self.rec();
                    r.chain = Some(chain.clone());
                    r.station = Some(if k == 0 {
                        "M".into()
                    } else {
                        self.secondary(k - 1)
                    });
                    r.snr_db = snr;
                    r.ecd_us = ecd;
                    r.extra.insert("sentence".into(), "LCD".into());
                    r.extra
                        .insert("units".into(), "as reported (relative; confirm)".into());
                    v.push(r);
                }
                v
            }
            "RMA" => {
                let mut v = Vec::new();
                let mut r = self.rec();
                r.lat_deg = nmea_latlon(g(2), g(3), 2);
                r.lon_deg = nmea_latlon(g(4), g(5), 3);
                if g(1) != "A" {
                    r.flags.insert(flags::NO_FIX.into());
                    r.flags.insert(flags::INVALID.into());
                }
                r.extra.insert("sentence".into(), "RMA".into());
                for (i, lab) in [(6usize, "TDA"), (7, "TDB")] {
                    if let Some(td) = num(i) {
                        r.extra.insert(lab.to_lowercase(), td.to_string());
                        let mut t = self.rec();
                        t.station = Some(lab.into());
                        t.td_us = Some(td);
                        t.extra.insert("sentence".into(), "RMA".into());
                        v.push(t);
                    }
                }
                v.insert(0, r);
                v
            }
            _ => {
                self.report.skipped += 1;
                return vec![];
            }
        };
        self.report.records += out.len();
        out
    }

    /// Parse a whole text.
    pub fn import(&mut self, text: &str) -> Vec<ObsRecord> {
        text.lines().flat_map(|l| self.line(l)).collect()
    }
}

// ---------------------------------------------------------------------------------------
// (c) Proprietary sentences: field-split profiles

/// A repeated per-station block inside one sentence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepeatGroup {
    /// Index of the first field of the first block.
    pub start: usize,
    /// Fields per block.
    pub stride: usize,
    /// Optional field holding the number of blocks.
    #[serde(default)]
    pub count_field: Option<usize>,
    /// Target → offset inside a block.
    pub fields: BTreeMap<String, FieldSpec>,
    #[serde(default)]
    pub flags: Option<FlagMap>,
}

/// One record type of a split profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitRecord {
    /// (field index, required value) pairs; all must match.
    #[serde(rename = "match")]
    pub matches: Vec<(usize, String)>,
    #[serde(flatten)]
    pub mapping: FieldMapping,
    #[serde(default)]
    pub repeat: Option<RepeatGroup>,
    /// Records of this type set the running time (for later records without time).
    #[serde(default)]
    pub sets_time: bool,
}

/// Field-split profile for vendor sentences (TOML).
///
/// ```toml
/// delimiter = ","
/// strip_checksum = true
/// standard_nmea = true
/// [[records]]
/// match = [[0, "$PVND"], [1, "TOA"]]
/// fields = { time = 2, chain = 3 }
/// time_format = "unix_s"
/// repeat = { start = 5, stride = 3, count_field = 4, fields = { station = 0, toa_us = 1, snr_db = 2 } }
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitProfile {
    #[serde(default = "d_delim")]
    pub delimiter: String,
    /// Drop a trailing `*hh` (verifying it when the line starts with `$`).
    #[serde(default = "yes")]
    pub strip_checksum: bool,
    /// Pass standard NMEA sentences (GGA, RMC, ZDA, GLL, GLC, LCD, RMA) to [`NmeaImporter`].
    #[serde(default)]
    pub standard_nmea: bool,
    /// Station labels of TD1…TD5 for the NMEA parser.
    #[serde(default)]
    pub secondaries: Vec<String>,
    pub records: Vec<SplitRecord>,
}

impl SplitProfile {
    pub fn from_toml(src: &str) -> Result<SplitProfile, String> {
        let p: SplitProfile = toml::from_str(src).map_err(|e| format!("split profile: {e}"))?;
        delim_char(&p.delimiter)?;
        for r in &p.records {
            r.mapping.validate()?;
            if let Some(g) = &r.repeat {
                if g.stride == 0 {
                    return Err("repeat.stride must be ≥ 1".into());
                }
                for k in g.fields.keys() {
                    if !TARGETS.contains(&k.as_str()) {
                        return Err(format!("unknown repeat field {k:?}"));
                    }
                }
            }
        }
        Ok(p)
    }

    /// Import a text; `date` seeds the NMEA parser's date (YYYY-MM-DD) when given.
    pub fn import(
        &self,
        text: &str,
        source: &str,
        date: Option<&str>,
    ) -> Result<(Vec<ObsRecord>, ImportReport), String> {
        let d = delim_char(&self.delimiter)?;
        let mut nmea = NmeaImporter::new(source);
        nmea.secondaries = self.secondaries.clone();
        if let Some(dt) = date {
            nmea = nmea.with_date(dt)?;
        }
        let mut rep = ImportReport::default();
        let mut out = Vec::new();
        let mut last_t: Option<f64> = None;
        for (i, raw) in text.lines().enumerate() {
            rep.lines += 1;
            let mut line = raw.trim().to_string();
            if line.is_empty() {
                rep.skipped += 1;
                continue;
            }
            if self.strip_checksum && line.starts_with('$') && line.contains('*') {
                match nmea_body(&line) {
                    Ok((b, _)) => line = format!("${b}"),
                    Err(e) => {
                        rep.bad_checksum += 1;
                        rep.err(i + 1, e);
                        continue;
                    }
                }
            }
            let f = split_by(&line, d);
            let fr: Vec<&str> = f.iter().map(|s| s.as_str()).collect();
            let rt = self.records.iter().find(|r| {
                r.matches
                    .iter()
                    .all(|(k, v)| fr.get(*k).map(|x| x.trim()) == Some(v.as_str()))
            });
            let Some(rt) = rt else {
                if self.standard_nmea && raw.trim_start().starts_with('$') {
                    let recs = nmea.line(raw);
                    if let Some(t) = nmea.now() {
                        last_t = Some(t);
                    }
                    rep.records += recs.len();
                    out.extend(recs);
                } else {
                    rep.skipped += 1;
                }
                continue;
            };
            let mut base = ObsRecord {
                source: source.into(),
                ..Default::default()
            };
            if let Err(e) = rt.mapping.apply(&fr, &[], &mut base) {
                rep.skipped += 1;
                rep.err(i + 1, e);
                continue;
            }
            if base.t_unix_s.is_some() && rt.sets_time {
                last_t = base.t_unix_s;
            }
            if base.t_unix_s.is_none() {
                base.t_unix_s = last_t;
            }
            match &rt.repeat {
                None => {
                    out.push(base);
                    rep.records += 1;
                }
                Some(g) => {
                    let avail = fr.len().saturating_sub(g.start) / g.stride;
                    let n = g
                        .count_field
                        .and_then(|c| fr.get(c).and_then(|s| s.trim().parse::<usize>().ok()))
                        .map(|c| c.min(avail))
                        .unwrap_or(avail);
                    let sub = FieldMapping {
                        fields: g.fields.clone(),
                        flags: g.flags.clone(),
                        ..rt.mapping.clone()
                    };
                    for b in 0..n {
                        let o = g.start + b * g.stride;
                        let blk: Vec<&str> = fr[o..(o + g.stride).min(fr.len())].to_vec();
                        let mut r = base.clone();
                        match sub.apply(&blk, &[], &mut r) {
                            Ok(()) => {
                                out.push(r);
                                rep.records += 1;
                            }
                            Err(e) => rep.err(i + 1, e),
                        }
                    }
                }
            }
        }
        rep.bad_checksum += nmea.report.bad_checksum;
        rep.errors.extend(nmea.report.errors.iter().cloned());
        Ok((out, rep))
    }
}

// ---------------------------------------------------------------------------------------
// (d) Time-interval counter CSV

/// Sign convention of the counter reading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TicSign {
    /// Reading = receiver 1PPS − reference 1PPS (start on reference, stop on receiver).
    #[default]
    RxMinusRef,
    /// Reading = reference − receiver.
    RefMinusRx,
}

/// 1PPS time-interval-counter profile: a CSV profile whose `timing_ns` field is the raw
/// reading in `unit`, then `timing_ns = sign · reading − fixed_delay_ns`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TicProfile {
    pub csv: CsvProfile,
    /// "s", "ms", "us" or "ns".
    #[serde(default = "d_unit")]
    pub unit: String,
    #[serde(default)]
    pub sign: TicSign,
    /// Cable/antenna/distribution delay to remove (ns).
    #[serde(default)]
    pub fixed_delay_ns: f64,
    /// Reject |reading| above this (ns, after conversion): counter wrap / missed edge.
    #[serde(default = "d_reject")]
    pub reject_abs_ns: f64,
}

fn d_unit() -> String {
    "s".into()
}
fn d_reject() -> f64 {
    5e8
}

impl TicProfile {
    pub fn from_toml(src: &str) -> Result<TicProfile, String> {
        let p: TicProfile = toml::from_str(src).map_err(|e| format!("tic profile: {e}"))?;
        p.csv.mapping.validate()?;
        if !p.csv.mapping.fields.contains_key("timing_ns") {
            return Err("tic profile: fields.timing_ns (the reading column) is required".into());
        }
        p.unit_ns()?;
        Ok(p)
    }
    fn unit_ns(&self) -> Result<f64, String> {
        Ok(match self.unit.as_str() {
            "s" => 1e9,
            "ms" => 1e6,
            "us" => 1e3,
            "ns" => 1.0,
            u => return Err(format!("tic unit {u:?} (s, ms, us, ns)")),
        })
    }
    pub fn import(
        &self,
        text: &str,
        source: &str,
    ) -> Result<(Vec<ObsRecord>, ImportReport), String> {
        let k = self.unit_ns()?;
        let s = match self.sign {
            TicSign::RxMinusRef => 1.0,
            TicSign::RefMinusRx => -1.0,
        };
        let (recs, mut rep) = self.csv.import(text, source)?;
        let mut out = Vec::new();
        for mut r in recs {
            if let Some(v) = r.timing_ns {
                let ns = s * v * k - self.fixed_delay_ns;
                if ns.abs() > self.reject_abs_ns {
                    rep.skipped += 1;
                    rep.records -= 1;
                    continue;
                }
                r.timing_ns = Some(ns);
                out.push(r);
            }
        }
        Ok((out, rep))
    }
}

/// Statistics of a timing series.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimingStats {
    pub n: usize,
    pub mean_ns: f64,
    pub std_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
    /// Max − min over the whole series (the MTIE at the full span).
    pub pk_pk_ns: f64,
    /// Least-squares slope (ns/s), when times are known.
    pub slope_ns_per_s: Option<f64>,
    /// 95th percentile of |x|.
    pub abs95_ns: f64,
}

/// Summarise the `timing_ns` values of records.
pub fn timing_stats(recs: &[ObsRecord]) -> Option<TimingStats> {
    let v: Vec<(Option<f64>, f64)> = recs
        .iter()
        .filter_map(|r| r.timing_ns.map(|x| (r.t_unix_s, x)))
        .collect();
    if v.is_empty() {
        return None;
    }
    let xs: Vec<f64> = v.iter().map(|p| p.1).collect();
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let std = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    let min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let slope = if v.iter().all(|p| p.0.is_some()) && v.len() >= 2 {
        let t0 = v[0].0.unwrap_or(0.0);
        let ts: Vec<f64> = v.iter().map(|p| p.0.unwrap_or(0.0) - t0).collect();
        let tm = ts.iter().sum::<f64>() / n;
        let sxx: f64 = ts.iter().map(|t| (t - tm).powi(2)).sum();
        (sxx > 0.0).then(|| {
            ts.iter()
                .zip(&xs)
                .map(|(t, x)| (t - tm) * (x - mean))
                .sum::<f64>()
                / sxx
        })
    } else {
        None
    };
    Some(TimingStats {
        n: xs.len(),
        mean_ns: mean,
        std_ns: std,
        min_ns: min,
        max_ns: max,
        pk_pk_ns: max - min,
        slope_ns_per_s: slope,
        abs95_ns: percentile(xs.iter().map(|x| x.abs()).collect(), 0.95),
    })
}

/// Percentile (nearest rank) of values; NaN when empty.
pub fn percentile(mut v: Vec<f64>, p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let k = ((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1;
    v[k]
}

// ---------------------------------------------------------------------------------------
// Simulator truth / software receiver → records; comparisons

/// Canonical (chain name, station id) of a record's labels against the simulated chains:
/// the chain label may be the chain name, its GRI in µs or its GRI/10 (e.g. "8970"); the
/// station label an id, or `TDk` / `TDA`, `TDB` for the k-th secondary in emission order.
pub fn canonical_key(
    chain: Option<&str>,
    station: Option<&str>,
    chains: &[Chain],
) -> Option<(usize, usize)> {
    let ci = match chain {
        Some(c) => chains.iter().position(|ch| {
            ch.name == c
                || c.parse::<u32>()
                    .is_ok_and(|g| g == ch.gri_us || g * 10 == ch.gri_us)
        })?,
        None if chains.len() == 1 => 0,
        None => return None,
    };
    let ch = &chains[ci];
    let st = station?;
    if let Some(si) = ch.stations.iter().position(|s| s.id == st) {
        return Some((ci, si));
    }
    let k = match st {
        "TDA" => Some(1),
        "TDB" => Some(2),
        _ => st.strip_prefix("TD").and_then(|n| n.parse::<usize>().ok()),
    }?;
    let mut secs: Vec<usize> = (0..ch.stations.len())
        .filter(|&i| ch.stations[i].role == Role::Secondary)
        .collect();
    secs.sort_by(|a, b| {
        ch.stations[*a]
            .emission_delay_us
            .total_cmp(&ch.stations[*b].emission_delay_us)
    });
    secs.get(k.checked_sub(1)?).map(|&si| (ci, si))
}

fn master_index(ch: &Chain) -> Option<usize> {
    ch.stations.iter().position(|s| s.role == Role::Master)
}

/// Truth records → observation records (ToA re the master's nominal emission, TD, ECD,
/// SNR, receiver position).
pub fn obs_from_truth(truth: &[TruthRecord], chains: &[Chain], start: AbsTime) -> Vec<ObsRecord> {
    let t0 = unix_s_from_abs(start);
    let mut master_toa: BTreeMap<(String, i64), f64> = BTreeMap::new();
    for r in truth {
        if let Some(ch) = chains.iter().find(|c| c.name == r.chain) {
            if master_index(ch).is_some_and(|mi| ch.stations[mi].id == r.station) {
                master_toa.insert((r.chain.clone(), r.group), r.toa_s);
            }
        }
    }
    truth
        .iter()
        .filter_map(|r| {
            let ch = chains.iter().find(|c| c.name == r.chain)?;
            let mi = master_index(ch)?;
            let toa_ref = nominal_emission_s(ch, mi, r.group, start);
            Some(ObsRecord {
                source: "truth".into(),
                t_unix_s: Some(t0 + r.toa_s),
                chain: Some(r.chain.clone()),
                station: Some(r.station.clone()),
                toa_us: Some((r.toa_s - toa_ref) * 1e6),
                td_us: if ch.stations[mi].id == r.station {
                    None
                } else {
                    master_toa
                        .get(&(r.chain.clone(), r.group))
                        .map(|m| (r.toa_s - m) * 1e6)
                },
                ecd_us: Some(r.ecd_us),
                snr_db: Some(r.snr_db),
                lat_deg: Some(r.rx_lat_deg),
                lon_deg: Some(r.rx_lon_deg),
                timing_ns: Some(r.time_ref_offset_ns),
                ..Default::default()
            })
        })
        .collect()
}

/// Software receiver measurements → observation records (one per tracking window).
pub fn obs_from_measurements(
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
) -> Vec<ObsRecord> {
    let t0 = unix_s_from_abs(start);
    let mut master_toa: BTreeMap<(usize, i64, Option<usize>), f64> = BTreeMap::new();
    for m in meas {
        let ch = &chains[m.chain_index];
        if ch.stations[m.station_index].role == Role::Master {
            master_toa.insert((m.chain_index, m.group, m.channel), m.toa_s);
        }
    }
    meas.iter()
        .filter_map(|m| {
            let ch = chains.get(m.chain_index)?;
            let mi = master_index(ch)?;
            let toa_ref = nominal_emission_s(ch, mi, m.group, start);
            let mut r = ObsRecord {
                source: "software-rx".into(),
                t_unix_s: Some(t0 + m.toa_s),
                chain: Some(ch.name.clone()),
                station: Some(ch.stations[m.station_index].id.clone()),
                toa_us: Some((m.toa_s - toa_ref) * 1e6),
                td_us: if m.station_index == mi {
                    None
                } else {
                    master_toa
                        .get(&(m.chain_index, m.group, m.channel))
                        .map(|t| (m.toa_s - t) * 1e6)
                },
                ecd_us: Some(m.ecd_us),
                snr_db: Some(m.snr_db),
                ..Default::default()
            };
            if m.cycle_slip {
                r.flags.insert(flags::CYCLE_SLIP.into());
            }
            Some(r)
        })
        .collect()
}

/// Statistics of one compared quantity for one key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuantityStats {
    /// "chain/station", or "fix" for positions.
    pub key: String,
    /// toa_ns, td_ns, ecd_us, snr_db, position_m, timing_ns.
    pub quantity: String,
    pub n: usize,
    pub mean: f64,
    pub std: f64,
    pub rms: f64,
    pub max_abs: f64,
    pub abs95: f64,
}

/// Result of [`compare_records`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub stats: Vec<QuantityStats>,
    /// Test records with a time but no reference within `max_dt_s`.
    pub unmatched: usize,
    /// Test records without a time or an identifiable key.
    pub unusable: usize,
    /// Matched ToA or TD differences larger than half a carrier cycle (5 µs): cycle
    /// identification errors.
    pub cycle_errors: usize,
}

impl Comparison {
    pub fn get(&self, key: &str, quantity: &str) -> Option<&QuantityStats> {
        self.stats
            .iter()
            .find(|s| s.key == key && s.quantity == quantity)
    }
    /// Worst value of `field(stats)` over all keys of a quantity.
    pub fn worst(&self, quantity: &str, field: impl Fn(&QuantityStats) -> f64) -> Option<f64> {
        self.stats
            .iter()
            .filter(|s| s.quantity == quantity)
            .map(field)
            .reduce(f64::max)
    }
}

fn stats(key: String, quantity: &str, d: &[f64]) -> QuantityStats {
    let n = d.len() as f64;
    let mean = d.iter().sum::<f64>() / n;
    QuantityStats {
        key,
        quantity: quantity.into(),
        n: d.len(),
        mean,
        std: (d.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt(),
        rms: (d.iter().map(|x| x * x).sum::<f64>() / n).sqrt(),
        max_abs: d.iter().map(|x| x.abs()).fold(0.0, f64::max),
        abs95: percentile(d.iter().map(|x| x.abs()).collect(), 0.95),
    }
}

/// Compare `test` records with `reference` records (truth or another receiver).
/// Station quantities are matched by canonical key and nearest time (|Δt| ≤ `max_dt_s`);
/// position fixes (records with lat/lon and a fix) against the nearest reference record
/// with a position; timing against the nearest reference with `timing_ns`.
pub fn compare_records(
    test: &[ObsRecord],
    reference: &[ObsRecord],
    chains: &[Chain],
    max_dt_s: f64,
) -> Comparison {
    let mut by_key: BTreeMap<(usize, usize), Vec<&ObsRecord>> = BTreeMap::new();
    for r in reference {
        if r.t_unix_s.is_none() {
            continue;
        }
        if let Some(k) = canonical_key(r.chain.as_deref(), r.station.as_deref(), chains) {
            by_key.entry(k).or_default().push(r);
        }
    }
    let pos_ref: Vec<&ObsRecord> = reference
        .iter()
        .filter(|r| r.t_unix_s.is_some() && r.lat_deg.is_some() && r.lon_deg.is_some())
        .collect();
    let tim_ref: Vec<&ObsRecord> = reference
        .iter()
        .filter(|r| r.t_unix_s.is_some() && r.timing_ns.is_some())
        .collect();
    let nearest = |v: &[&ObsRecord], t: f64| -> Option<ObsRecord> {
        v.iter()
            .min_by(|a, b| {
                (a.t_unix_s.unwrap_or(0.0) - t)
                    .abs()
                    .total_cmp(&(b.t_unix_s.unwrap_or(0.0) - t).abs())
            })
            .filter(|r| (r.t_unix_s.unwrap_or(0.0) - t).abs() <= max_dt_s)
            .map(|r| (*r).clone())
    };
    let mut diffs: BTreeMap<(String, &'static str), Vec<f64>> = BTreeMap::new();
    let mut cmp = Comparison::default();
    for r in test {
        let Some(t) = r.t_unix_s else {
            cmp.unusable += 1;
            continue;
        };
        let mut matched = false;
        if r.has_fix() {
            if let Some(p) = nearest(&pos_ref, t) {
                let d = inverse(
                    Geo::new(r.lat_deg.unwrap_or(0.0), r.lon_deg.unwrap_or(0.0)),
                    Geo::new(p.lat_deg.unwrap_or(0.0), p.lon_deg.unwrap_or(0.0)),
                )
                .distance_m;
                diffs
                    .entry(("fix".into(), "position_m"))
                    .or_default()
                    .push(d);
                matched = true;
            }
        }
        if let Some(x) = r.timing_ns {
            if r.station.is_none() {
                if let Some(p) = nearest(&tim_ref, t) {
                    diffs
                        .entry(("timing".into(), "timing_ns"))
                        .or_default()
                        .push(x - p.timing_ns.unwrap_or(0.0));
                    matched = true;
                }
            }
        }
        if r.station.is_some() {
            let Some(k) = canonical_key(r.chain.as_deref(), r.station.as_deref(), chains) else {
                cmp.unusable += 1;
                continue;
            };
            let key = format!("{}/{}", chains[k.0].name, chains[k.0].stations[k.1].id);
            if let Some(p) = by_key.get(&k).and_then(|v| nearest(v, t)) {
                let mut push = |q: &'static str, a: Option<f64>, b: Option<f64>, s: f64| {
                    if let (Some(a), Some(b)) = (a, b) {
                        diffs.entry((key.clone(), q)).or_default().push((a - b) * s);
                    }
                };
                push("toa_ns", r.toa_us, p.toa_us, 1e3);
                push("td_ns", r.td_us, p.td_us, 1e3);
                push("ecd_us", r.ecd_us, p.ecd_us, 1.0);
                push("snr_db", r.snr_db, p.snr_db, 1.0);
                matched = true;
            }
        }
        if !matched && (r.station.is_some() || r.has_fix()) {
            cmp.unmatched += 1;
        }
    }
    cmp.cycle_errors = diffs
        .iter()
        .filter(|((_, q), _)| *q == "toa_ns" || *q == "td_ns")
        .map(|(_, d)| d.iter().filter(|x| x.abs() > 5_000.0).count())
        .sum();
    cmp.stats = diffs
        .into_iter()
        .map(|((k, q), d)| stats(k, q, &d))
        .collect();
    cmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Station;
    use crate::signal::PulseSignal;

    fn chain() -> Chain {
        let st = |id: &str, role: Role, d: f64| Station {
            id: id.into(),
            name: String::new(),
            role,
            position: Geo::new(55.0, 3.0),
            power_kw: 400.0,
            emission_delay_us: d,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: Default::default(),
            faults: vec![],
        };
        Chain {
            name: "rx-A".into(),
            gri_us: 79800,
            signal: PulseSignal::loran_c(),
            stations: vec![
                st("M", Role::Master, 0.0),
                st("Y", Role::Secondary, 29000.0),
                st("X", Role::Secondary, 13000.0),
            ],
        }
    }

    #[test]
    fn csv_profile_maps_columns_scales_and_flags() {
        let prof = CsvProfile::from_toml(
            r#"
delimiter = ";"
time_format = "iso8601"
[fields]
time = "utc"
chain = "gri"
station = 2
toa_us = { column = "toa_ns", scale = 0.001 }
snr_db = "snr"
[flags]
column = "st"
per_char = { B = "blink", C = "cycle_warning" }
map = { BAD = "invalid" }
"#,
        )
        .unwrap();
        let text = "# comment\nutc;gri;stn;toa_ns;snr;st\n2026-11-18T00:00:01.5Z;7980;X;\"13500123\";12.5;BC\n2026-11-18 00:00:02;7980;M;5000;9;BAD\nbad;7980;M;x;1;\n";
        let (r, rep) = prof.import(text, "log.csv").unwrap();
        assert_eq!(rep.records, 2);
        assert_eq!(rep.errors.len(), 1);
        assert_eq!(r[0].station.as_deref(), Some("X"));
        assert!((r[0].toa_us.unwrap() - 13500.123).abs() < 1e-9);
        assert_eq!(r[0].t_unix_s, Some(1_794_960_001.5));
        assert!(r[0].flags.contains("blink") && r[0].flags.contains("cycle_warning"));
        assert!(r[1].flags.contains("invalid"));
        assert!(CsvProfile::from_toml("[fields]\nfoo = 1\n").is_err());
        assert!(CsvProfile::from_toml("time_format = \"hms\"\n[fields]\ntime = 0\n").is_err());
    }

    #[test]
    fn nmea_checksum_fix_time_and_loran_sentences() {
        let lines = [
            nmea_sentence("GPZDA,000001.00,18,11,2026,00,00"),
            nmea_sentence("GPGGA,000002.00,5500.000,N,00300.000,E,1,08,0.9,10.0,M,47.0,M,,"),
            nmea_sentence("LCGLC,7980,26123.45,A,13456.70,B,29010.20,C,,,,,,"),
            nmea_sentence("LCLCD,7980,050,001,045,002,,,,,,,,"),
            nmea_sentence("GPRMC,000003.00,V,5500.000,N,00300.000,W,0.0,0.0,191126,,"),
            nmea_sentence("LCRMA,A,5500.000,N,00300.000,W,13456.7,29010.2,5.0,90.0,,"),
            "$GPGGA,000004.00,5500.000,N,00300.000,E,1,08,0.9,10.0,M,47.0,M,,*00".to_string(),
            "$PVND,1,2*7F".to_string(),
        ];
        let mut imp = NmeaImporter::new("nmea");
        let recs = imp.import(&lines.join("\n"));
        assert_eq!(imp.report.bad_checksum, 2);
        let gga = &recs[0];
        assert_eq!(gga.t_unix_s, Some(1_794_960_002.0));
        assert!((gga.lat_deg.unwrap() - 55.0).abs() < 1e-12);
        assert_eq!(gga.alt_m, Some(10.0));
        let glc: Vec<&ObsRecord> = recs
            .iter()
            .filter(|r| r.extra.get("sentence").map(|s| s.as_str()) == Some("GLC"))
            .collect();
        assert_eq!(glc.len(), 3);
        assert_eq!(glc[0].station.as_deref(), Some("M"));
        assert_eq!(glc[0].toa_us, Some(26123.45));
        assert_eq!(glc[1].station.as_deref(), Some("TD1"));
        assert!(glc[1].flags.contains("blink"));
        assert!(glc[2].flags.contains("cycle_warning"));
        assert_eq!(glc[2].t_unix_s, Some(1_794_960_002.0));
        let lcd: Vec<&ObsRecord> = recs
            .iter()
            .filter(|r| r.extra.get("sentence").map(|s| s.as_str()) == Some("LCD"))
            .collect();
        assert_eq!(lcd.len(), 2);
        assert_eq!(lcd[1].snr_db, Some(45.0));
        // RMC sets the date (19 Nov 2026), status V → no fix
        let rmc = recs
            .iter()
            .find(|r| r.extra.get("sentence").map(|s| s.as_str()) == Some("RMC"))
            .unwrap();
        assert!(rmc.flags.contains("no_fix"));
        assert_eq!(rmc.t_unix_s, Some(1_794_960_000.0 + 86_400.0 + 3.0));
        assert!((rmc.lon_deg.unwrap() + 3.0).abs() < 1e-12);
        let rma: Vec<&ObsRecord> = recs
            .iter()
            .filter(|r| r.extra.get("sentence").map(|s| s.as_str()) == Some("RMA"))
            .collect();
        assert_eq!(rma.len(), 3);
        assert_eq!(rma[2].station.as_deref(), Some("TDB"));
        // TD labels resolve to secondaries in emission order (X then Y)
        let ch = vec![chain()];
        assert_eq!(canonical_key(Some("7980"), Some("TD1"), &ch), Some((0, 2)));
        assert_eq!(canonical_key(Some("79800"), Some("TDB"), &ch), Some((0, 1)));
        assert_eq!(canonical_key(Some("rx-A"), Some("M"), &ch), Some((0, 0)));
        assert_eq!(canonical_key(Some("9999"), Some("M"), &ch), None);
    }

    #[test]
    fn split_profile_with_repeat_groups_and_running_time() {
        let prof = SplitProfile::from_toml(
            r#"
standard_nmea = true
secondaries = ["X", "Y"]
[[records]]
match = [[0, "$PVND"], [1, "TIM"]]
fields = { time = 2 }
time_format = "unix_s"
sets_time = true
[[records]]
match = [[0, "$PVND"], [1, "TOA"]]
fields = { chain = 2 }
repeat = { start = 4, stride = 3, count_field = 3, fields = { station = 0, toa_us = 1, snr_db = 2 }, flags = { column = 2, map = { "-1" = "invalid" } } }
"#,
        )
        .unwrap();
        let text = [
            nmea_sentence("PVND,TIM,1794960010.25"),
            nmea_sentence("PVND,TOA,7980,3,M,26000.5,20,X,39000.25,15,Y,55000,-1"),
            nmea_sentence("LCGLC,7980,26000.5,A,13000.0,A,,,,,,,,"),
            "garbage line".to_string(),
        ]
        .join("\n");
        let (r, rep) = prof.import(&text, "vendor", Some("2026-11-18")).unwrap();
        assert_eq!(rep.skipped, 1);
        assert_eq!(r.len(), 1 + 3 + 2);
        let toa: Vec<&ObsRecord> = r
            .iter()
            .filter(|x| x.toa_us.is_some() && x.extra.is_empty())
            .collect();
        assert_eq!(toa.len(), 3);
        assert_eq!(toa[1].station.as_deref(), Some("X"));
        assert_eq!(toa[1].toa_us, Some(39000.25));
        assert_eq!(toa[1].t_unix_s, Some(1794960010.25));
        assert!(toa[2].flags.contains("invalid"));
        let glc_td = r.iter().find(|x| x.td_us.is_some()).unwrap();
        assert_eq!(glc_td.station.as_deref(), Some("X"));
    }

    #[test]
    fn tic_profile_units_sign_delay_and_stats() {
        let prof = TicProfile::from_toml(
            r#"
unit = "s"
sign = "ref_minus_rx"
fixed_delay_ns = 5.0
[csv]
header = false
time_format = "unix_s"
fields = { time = 0, timing_ns = 1 }
"#,
        )
        .unwrap();
        let text = "100,-1.0e-8\n101,-1.2e-8\n102,-1.4e-8\n103,0.9\n";
        let (r, rep) = prof.import(text, "tic").unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!(rep.records, 3);
        assert!((r[0].timing_ns.unwrap() - 5.0).abs() < 1e-9);
        let s = timing_stats(&r).unwrap();
        assert!((s.mean_ns - 7.0).abs() < 1e-9);
        assert!((s.slope_ns_per_s.unwrap() - 2.0).abs() < 1e-9);
        assert!((s.pk_pk_ns - 4.0).abs() < 1e-9);
        assert!(TicProfile::from_toml("[csv]\nfields = { time = 0 }\n").is_err());
    }

    #[test]
    fn time_formats() {
        assert_eq!(
            parse_time("10:00:01.5", TimeFormat::Hms, Some("1970-01-02")).unwrap(),
            86_400.0 + 36_001.5
        );
        assert_eq!(
            parse_time("100001", TimeFormat::Hms, Some("1970-01-01")).unwrap(),
            36_001.0
        );
        // GPS 0 = 1980-01-06 (GPS − UTC = 0 then); in 2026 GPS − UTC = 18 s
        assert_eq!(
            parse_time("0", TimeFormat::GpsS, None).unwrap(),
            315_964_800.0
        );
        let gps_2026 = 1_794_960_000.0 - 315_964_800.0 + 18.0;
        assert_eq!(
            parse_time(&gps_2026.to_string(), TimeFormat::GpsS, None).unwrap(),
            1_794_960_000.0
        );
        assert!(parse_time("25:00:00", TimeFormat::Hms, Some("2026-01-01")).is_err());
    }

    #[test]
    fn compare_against_truth_records() {
        let ch = vec![chain()];
        let start = AbsTime::from_utc_iso("2026-11-18T00:00:00Z").unwrap();
        let t0 = unix_s_from_abs(start);
        let reference = vec![
            ObsRecord {
                t_unix_s: Some(t0 + 1.0),
                chain: Some("rx-A".into()),
                station: Some("X".into()),
                toa_us: Some(13500.0),
                td_us: Some(13400.0),
                lat_deg: Some(55.0),
                lon_deg: Some(3.0),
                ..Default::default()
            },
            ObsRecord {
                t_unix_s: Some(t0 + 1.0),
                timing_ns: Some(10.0),
                ..Default::default()
            },
        ];
        let test = vec![
            ObsRecord {
                t_unix_s: Some(t0 + 1.02),
                chain: Some("7980".into()),
                station: Some("TD1".into()),
                td_us: Some(13400.05),
                ..Default::default()
            },
            ObsRecord {
                t_unix_s: Some(t0 + 1.0),
                lat_deg: Some(55.001),
                lon_deg: Some(3.0),
                ..Default::default()
            },
            ObsRecord {
                t_unix_s: Some(t0 + 1.0),
                timing_ns: Some(13.0),
                ..Default::default()
            },
            ObsRecord {
                t_unix_s: Some(t0 + 9.0),
                chain: Some("rx-A".into()),
                station: Some("X".into()),
                td_us: Some(1.0),
                ..Default::default()
            },
        ];
        let c = compare_records(&test, &reference, &ch, 0.1);
        let td = c.get("rx-A/X", "td_ns").unwrap();
        assert!((td.mean - 50.0).abs() < 1e-6);
        let p = c.get("fix", "position_m").unwrap();
        assert!((p.mean - 111.25).abs() < 0.5, "{}", p.mean);
        assert!((c.get("timing", "timing_ns").unwrap().mean - 3.0).abs() < 1e-9);
        assert_eq!(c.unmatched, 1);
    }
}
