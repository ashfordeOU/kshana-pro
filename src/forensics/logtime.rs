// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Log time-integrity checker (research 2026-11-20 gaps, item B0603)
//!
//! Operational logs are evidence only when their time stamps can be trusted: a VDR or ECDIS
//! log whose clock followed a spoofed receiver, an aircraft clock "running backwards" in its
//! CPDLC and ACARS traffic, a SATCOM terminal or GMDSS console showing the wrong time, an FDM
//! record whose time base stepped. This module reads such logs and checks every clock in them
//! against the reference each record carries (a GNSS-disciplined recorder stamp, a ground
//! station's receive time, a `ref_time` column), against the other logs, and against itself:
//!
//! * **steps and jumps**: a change of the clock's offset to its reference beyond a threshold
//!   (or, without a reference, a backward step); named with the time-bomb classifier of the
//!   timing work ([`crate::timing::timebomb::classify_step`]): GPS week rollover (1024 weeks),
//!   a two-digit year in the wrong century, a 32-bit seconds wrap, GPS time given as UTC;
//! * **ordering violations**: the clock (or the reference) going backwards;
//! * **frozen clocks**: the same time stamp on consecutive records while the reference advances;
//! * **drift rate**: the slope of the offset per segment between steps (ppm, with its standard
//!   uncertainty), and a constant offset beyond a limit;
//! * **leap seconds**: a 23:59:60 on a day that ended without one, and a ±1 s step at midnight
//!   (one clock applied a leap second, the other did not), checked against the open engine's
//!   leap-second table and any dates the request adds;
//! * **absolute epoch**: without a reference, a log whose times sit 1024 weeks or a century away
//!   from the stated incident window;
//! * **cross-log disagreement**: per time bin, the offsets of every log against the common
//!   reference axis are voted with the clock self-check's weighted vote
//!   ([`crate::timing::selfcheck::vote`]); a log outside the consensus is named, and a consensus
//!   away from zero says the reference itself is off.
//!
//! Each log also gets a **corrected timeline**: per segment between steps, the offset is fitted
//! against the reference and removed; a segment with no reference is bridged across the step at
//! the log's own cadence (a clock that stepped back 37 s is put back where it would have been).
//! Each corrected time carries a standard uncertainty. Records whose time cannot be trusted as
//! logged form **integrity intervals** (start, end, reason), which the dispute evidence report
//! ([`super::dispute`]) reuses.
//!
//! Formats: a documented CSV/JSON schema, NMEA 0183 ZDA/RMC (VDR export with an optional recorder
//! stamp before the `$`, split by talker), and ACARS/CPDLC message logs as JSON lines with the
//! ground station's receive time and the aircraft's message time. See `docs/INCIDENT-EVIDENCE.md`.
//!
//! **Label: MODELLED.** The checks are exact arithmetic on the logs as supplied, under thresholds
//! fixed in [`Params`] and written into every report; tested on synthetic logs with known truth
//! only. A finding is evidence to investigate, not a legal finding.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::inputs::sha256_hex;
use crate::timing::selfcheck;
use crate::timing::timebomb::{classify_step, Label as UtcLabel};

/// The command feature (`kshana-pro forensics log-time`).
pub const FEATURE: &str = "pro-log-time-integrity";
/// The report format tag.
pub const FORMAT: &str = "kshana-pro-log-time-integrity/1";
/// The method; every threshold is a field of [`Params`] and is written into the report.
pub const METHOD_ID: &str = "kshana-pro/log-time-integrity/v1";
/// The report's main file in an output directory.
pub const RESULT_FILE: &str = "log-time-report.json";
/// The honesty label.
pub const LABEL: &str = "MODELLED: exact arithmetic on the time stamps as supplied, under \
thresholds fixed before the run; the reference stamps and their stated uncertainties are the \
operator's. Tested on synthetic logs with known truth only; not validated on a labelled set of \
real incidents.";
/// What every report says about itself.
pub const STATEMENT: &str = "A time-integrity record of the logs supplied: which clocks stepped, \
froze, drifted, rolled over or disagreed, and a corrected timeline with its uncertainty. It is an \
engineering record, not a legal finding and not a finding of fact.";

/// Pre-registered thresholds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// A change of the offset to the reference between consecutive records beyond this is a
    /// step (s).
    pub step_s: f64,
    /// A segment's offset to the reference beyond this is reported (s).
    pub offset_limit_s: f64,
    /// This many consecutive equal time stamps make a frozen clock.
    pub frozen_n: usize,
    /// A drift rate beyond this is reported (ppm), when significant at 3 sigma.
    pub drift_ppm: f64,
    /// Shortest segment whose drift is judged (s of reference time).
    pub min_drift_span_s: f64,
    /// Without a reference, a forward gap beyond this many cadences is reported.
    pub gap_cadences: f64,
    /// Cross-log bin (s of reference time).
    pub bin_s: f64,
    /// Cross-log vote multiplier.
    pub k: f64,
    /// Cross-log vote floor (s).
    pub floor_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            step_s: 0.8,
            offset_limit_s: 1.0,
            frozen_n: 3,
            drift_ppm: 20.0,
            min_drift_span_s: 600.0,
            gap_cadences: 5.0,
            bin_s: 60.0,
            k: 3.0,
            floor_s: 0.1,
        }
    }
}

fn d_u() -> f64 {
    0.5
}
fn d_true() -> bool {
    true
}

/// One log of the request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogSpec {
    pub name: String,
    /// Informative: `vdr`, `ecdis`, `acars`, `cpdlc`, `fdm`, `satcom`, `gmdss`, ...
    #[serde(default)]
    pub kind: Option<String>,
    /// `csv`, `json`, `nmea` or `acars-json`.
    pub format: String,
    pub path: String,
    /// Standard uncertainty of one time stamp against its reference (s), for the cross-log vote.
    #[serde(default = "d_u")]
    pub u_s: f64,
    /// Split into one clock per NMEA talker, per aircraft (`tail`, else `flight`) or per
    /// `source` column.
    #[serde(default = "d_true")]
    pub split: bool,
    /// `acars-json`: the key holding the message (aircraft) time; default `msg_time`, and
    /// failing that a `/TS<hhmmss>,<ddmmyy>` field in `text`.
    #[serde(default)]
    pub time_field: Option<String>,
    /// This log's offset limit (s), when its reference has a known latency (a ground
    /// station's receive time trails the message time); default `params.offset_limit_s`.
    #[serde(default)]
    pub offset_limit_s: Option<f64>,
    /// Clocks are voted against one another only within a reference group: the clocks of one
    /// log by default (they share its reference stamp); logs given the same group name are
    /// compared together (their reference stamps must be one time scale).
    #[serde(default)]
    pub reference_group: Option<String>,
    /// Informative: what the per-record reference stamp is (e.g. "VDR recorder clock, GNSS
    /// disciplined").
    #[serde(default)]
    pub reference: Option<String>,
}

/// A request (`kshana-pro forensics log-time <request.toml>`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// The incident window, for the absolute-epoch check of logs without a reference.
    #[serde(default)]
    pub window_start_utc: Option<String>,
    #[serde(default)]
    pub window_end_utc: Option<String>,
    /// Extra leap-second dates (`YYYY-MM-DD`, the day that ends with 23:59:60), for leap
    /// seconds announced after the engine's table.
    #[serde(default)]
    pub leap_dates: Vec<String>,
    #[serde(default)]
    pub params: Params,
    pub logs: Vec<LogSpec>,
}

impl Request {
    pub fn parse(text: &str) -> Result<Request, String> {
        let r: Request = toml::from_str(text).map_err(|e| format!("invalid request: {e}"))?;
        if r.logs.is_empty() {
            return Err("the request names no [[logs]]".into());
        }
        Ok(r)
    }
}

// ---------------------------------------------------------------------------------------
// Time parsing
// ---------------------------------------------------------------------------------------

/// A parsed time: Unix seconds (23:59:60 coincides with the next 00:00:00, as in Unix time)
/// and whether it was written as second 60.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamp {
    pub t: f64,
    pub leap: bool,
}

/// Parse Unix seconds or ISO 8601 UTC (`YYYY-MM-DD[T ]HH:MM:SS[.fff][Z]`, second 60 allowed).
pub fn parse_time(s: &str) -> Option<Stamp> {
    let s = s
        .trim()
        .trim_start_matches('[')
        .trim_end_matches([']', ',', ':', ';'])
        .trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(x) = s.parse::<f64>() {
        return x.is_finite().then_some(Stamp { t: x, leap: false });
    }
    if !s.contains(':') {
        return None;
    }
    let l = UtcLabel::parse_iso(s).ok()?;
    let core = s.trim_end_matches('Z');
    let frac = match core.rsplit_once('.') {
        Some((head, f)) if head.contains(':') && f.chars().all(|c| c.is_ascii_digit()) => {
            format!("0.{f}").parse::<f64>().ok()?
        }
        _ => 0.0,
    };
    Some(Stamp {
        t: l.seconds() as f64 + frac,
        leap: l.sod >= 86_400,
    })
}

fn civil(days: i64) -> (i64, i64, i64) {
    kshana_eloran::time::civil_from_days(days)
}

fn days_of(y: i64, m: i64, d: i64) -> i64 {
    kshana_eloran::time::days_from_civil(y, m, d)
}

/// ISO 8601 UTC with milliseconds.
pub fn iso_ms(t: f64) -> String {
    let ms = (t * 1000.0).round() as i64;
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil(days);
    let (h, mi, s, f) = (
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000,
    );
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{f:03}Z")
}

/// Round to 9 significant digits (stable JSON across platforms).
pub fn sig9(x: f64) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let mag = x.abs().log10().floor() as i32;
    let p = 8 - mag;
    if p >= 0 {
        let f = 10f64.powi(p);
        (x * f).round() / f
    } else {
        let f = 10f64.powi(-p);
        (x / f).round() * f
    }
}

/// Whether the UTC day `days` (from 1970-01-01) ends with an inserted leap second, by the
/// open engine's TAI − UTC table or the request's extra dates.
pub fn leap_day(days: i64, extra: &[i64]) -> bool {
    if extra.contains(&days) {
        return true;
    }
    let dat = |d: i64| {
        let (y, m, dd) = civil(d);
        kshana::timescales::tai_minus_utc(kshana::timescales::julian_date(
            y as i32, m as u32, dd as u32, 0, 0, 0.0,
        ))
    };
    (dat(days + 1) - dat(days) - 1.0).abs() < 1e-9
}

// ---------------------------------------------------------------------------------------
// Readers
// ---------------------------------------------------------------------------------------

/// One time-stamped record of one clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// The clock under test (Unix seconds).
    pub t: f64,
    /// The record's reference stamp (Unix seconds), when it has one.
    pub r: Option<f64>,
    /// Written as second 60.
    pub leap: bool,
    /// The device flagged its time valid (RMC status, `valid` column).
    pub valid: bool,
    /// 1-based line (or JSON element) number.
    pub line: usize,
}

/// A log read into one or more clocks (series).
#[derive(Clone, Debug, Default)]
pub struct ReadLog {
    pub series: BTreeMap<String, Vec<Record>>,
    pub unreadable: usize,
}

fn checksum_ok(s: &str) -> Option<&str> {
    let body = s.trim().strip_prefix('$')?;
    match body.split_once('*') {
        Some((b, cs)) => {
            let want = u8::from_str_radix(cs.get(..2)?, 16).ok()?;
            (kshana::nmea_synth::nmea::checksum(b) == want).then_some(b)
        }
        None => Some(body),
    }
}

fn hms_frac(f: &str) -> Option<f64> {
    let (whole, frac) = f.split_once('.').unwrap_or((f, ""));
    if whole.len() != 6 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let p = |r: std::ops::Range<usize>| whole.get(r)?.parse::<i64>().ok();
    let (h, m, s) = (p(0..2)?, p(2..4)?, p(4..6)?);
    if h > 23 || m > 59 || s > 60 || (s == 60 && !(h == 23 && m == 59)) {
        return None;
    }
    let fr = if frac.is_empty() {
        0.0
    } else {
        format!("0.{frac}").parse::<f64>().ok()?
    };
    Some((h * 3600 + m * 60 + s) as f64 + fr)
}

fn stamp_of(y: i64, mo: i64, d: i64, sod: f64) -> Option<Stamp> {
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    let leap = sod >= 86_400.0;
    Some(Stamp {
        t: days_of(y, mo, d) as f64 * 86_400.0 + sod,
        leap,
    })
}

/// Read NMEA 0183: ZDA per talker when the talker sends any, else RMC (two-digit year read as
/// 20yy). Text before the `$` on a line is the recorder's reference stamp.
pub fn read_nmea(text: &str, split: bool) -> ReadLog {
    let mut zda_talkers = std::collections::BTreeSet::new();
    for line in text.lines() {
        if let Some(i) = line.find('$') {
            let b = &line[i + 1..];
            if b.get(2..6) == Some("ZDA,") {
                zda_talkers.insert(b[..2].to_string());
            }
        }
    }
    let mut out = ReadLog::default();
    for (n, line) in text.lines().enumerate() {
        let Some(i) = line.find('$') else {
            continue;
        };
        let Some(body) = checksum_ok(&line[i..]) else {
            out.unreadable += 1;
            continue;
        };
        let f: Vec<&str> = body.split(',').collect();
        if f[0].len() < 5 {
            continue;
        }
        let (talker, kind) = f[0].split_at(2);
        let use_zda = zda_talkers.contains(talker);
        let parsed = match kind {
            "ZDA" if use_zda && f.len() >= 5 => (|| {
                let sod = hms_frac(f[1])?;
                let st = stamp_of(
                    f[4].parse().ok()?,
                    f[3].parse().ok()?,
                    f[2].parse().ok()?,
                    sod,
                )?;
                Some((st, true))
            })(),
            "RMC" if !use_zda && f.len() >= 10 => (|| {
                let sod = hms_frac(f[1])?;
                let dt = f[9];
                if dt.len() != 6 {
                    return None;
                }
                let p = |r: std::ops::Range<usize>| dt.get(r)?.parse::<i64>().ok();
                let st = stamp_of(2000 + p(4..6)?, p(2..4)?, p(0..2)?, sod)?;
                Some((st, f[2] == "A"))
            })(),
            "ZDA" | "RMC" => {
                if (kind == "ZDA") == use_zda {
                    None
                } else {
                    continue;
                }
            }
            _ => continue,
        };
        let Some((st, valid)) = parsed else {
            out.unreadable += 1;
            continue;
        };
        let r = parse_time(&line[..i]).map(|s| s.t);
        let key = if split {
            talker.to_string()
        } else {
            String::new()
        };
        out.series.entry(key).or_default().push(Record {
            t: st.t,
            r,
            leap: st.leap,
            valid,
            line: n + 1,
        });
    }
    out
}

fn value_time(v: &serde_json::Value) -> Option<Stamp> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().map(|t| Stamp { t, leap: false }),
        serde_json::Value::String(s) => parse_time(s),
        _ => None,
    }
}

/// The `/TS<hhmmss>,<ddmmyy>` time field of a message text.
pub fn ts_field(text: &str) -> Option<Stamp> {
    let i = text.find("/TS")?;
    let rest = &text[i + 3..];
    let hms = rest.get(..6)?;
    let date = rest.get(7..13)?;
    if rest.as_bytes().get(6) != Some(&b',') || !date.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let sod = hms_frac(hms)?;
    let p = |r: std::ops::Range<usize>| date.get(r)?.parse::<i64>().ok();
    stamp_of(2000 + p(4..6)?, p(2..4)?, p(0..2)?, sod)
}

fn json_items(text: &str) -> Result<Vec<serde_json::Value>, String> {
    let t = text.trim_start();
    if t.starts_with('[') {
        return match serde_json::from_str::<serde_json::Value>(t) {
            Ok(serde_json::Value::Array(a)) => Ok(a),
            Ok(_) => Err("expected a JSON array".into()),
            Err(e) => Err(format!("invalid JSON: {e}")),
        };
    }
    Ok(text
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l.trim()).unwrap_or(serde_json::Value::Null)
        })
        .filter(|v| !(v.is_null()))
        .collect())
}

/// Read ACARS/CPDLC JSON lines (acarsdec/dumpvdl2-like): `timestamp` is the ground station's
/// receive time (the reference); the message time is `time_field` (default `msg_time`) or a
/// `/TS` field in `text`. One clock per aircraft (`tail`, else `flight`).
pub fn read_acars_json(text: &str, split: bool, time_field: Option<&str>) -> ReadLog {
    let mut out = ReadLog::default();
    let key = time_field.unwrap_or("msg_time");
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(l) else {
            out.unreadable += 1;
            continue;
        };
        let r = v.get("timestamp").and_then(value_time).map(|s| s.t);
        let t = v
            .get(key)
            .and_then(value_time)
            .or_else(|| v.get("text").and_then(|x| x.as_str()).and_then(ts_field));
        let Some(t) = t else {
            // A message with no time of its own is not a fault of the clock.
            continue;
        };
        let id = ["tail", "flight", "reg"]
            .iter()
            .find_map(|k| v.get(*k).and_then(|x| x.as_str()))
            .map(|s| s.trim_start_matches('.').to_string())
            .unwrap_or_default();
        out.series
            .entry(if split { id } else { String::new() })
            .or_default()
            .push(Record {
                t: t.t,
                r,
                leap: t.leap,
                valid: true,
                line: n + 1,
            });
    }
    out
}

fn truthy(s: &str) -> bool {
    !matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "v" | "invalid" | "n"
    )
}

/// Read the documented CSV schema: a header naming `time` (required), `ref_time`, `valid` and
/// `source` (any order; `,` or `;`), or no header and `time[,ref_time]`.
pub fn read_csv(text: &str, split: bool) -> ReadLog {
    let mut out = ReadLog::default();
    // (time, ref_time, valid, source) column indices.
    type Cols = (usize, Option<usize>, Option<usize>, Option<usize>);
    let mut cols: Option<Cols> = None;
    let mut first = true;
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = l.split([',', ';']).map(str::trim).collect();
        if std::mem::replace(&mut first, false) {
            let low: Vec<String> = f.iter().map(|x| x.to_ascii_lowercase()).collect();
            let pos = |names: &[&str]| low.iter().position(|x| names.contains(&x.as_str()));
            if let Some(t) = pos(&["time", "log_time", "t"]) {
                cols = Some((
                    t,
                    pos(&["ref_time", "reference_time", "ref"]),
                    pos(&["valid"]),
                    pos(&["source", "series", "clock"]),
                ));
                continue;
            }
            cols = Some((0, Some(1), None, None));
        }
        let (ti, ri, vi, si) = cols.unwrap_or((0, Some(1), None, None));
        let Some(st) = f.get(ti).and_then(|x| parse_time(x)) else {
            out.unreadable += 1;
            continue;
        };
        let r = ri
            .and_then(|i| f.get(i))
            .and_then(|x| parse_time(x))
            .map(|s| s.t);
        let valid = vi.and_then(|i| f.get(i)).map(|x| truthy(x)).unwrap_or(true);
        let key = match (split, si.and_then(|i| f.get(i))) {
            (true, Some(s)) => s.to_string(),
            _ => String::new(),
        };
        out.series.entry(key).or_default().push(Record {
            t: st.t,
            r,
            leap: st.leap,
            valid,
            line: n + 1,
        });
    }
    out
}

/// Read the documented JSON schema: an array, or JSON lines, of objects with `time`
/// (required), `ref_time`, `valid` and `source`.
pub fn read_json(text: &str, split: bool) -> Result<ReadLog, String> {
    let mut out = ReadLog::default();
    for (n, v) in json_items(text)?.iter().enumerate() {
        let Some(st) = v.get("time").and_then(value_time) else {
            out.unreadable += 1;
            continue;
        };
        let r = v.get("ref_time").and_then(value_time).map(|s| s.t);
        let valid = match v.get("valid") {
            Some(serde_json::Value::Bool(b)) => *b,
            Some(serde_json::Value::String(s)) => truthy(s),
            _ => true,
        };
        let key = match (split, v.get("source").and_then(|x| x.as_str())) {
            (true, Some(s)) => s.to_string(),
            _ => String::new(),
        };
        out.series.entry(key).or_default().push(Record {
            t: st.t,
            r,
            leap: st.leap,
            valid,
            line: n + 1,
        });
    }
    Ok(out)
}

/// Read one log by its format.
pub fn read_log(spec: &LogSpec, text: &str) -> Result<ReadLog, String> {
    Ok(match spec.format.as_str() {
        "nmea" => read_nmea(text, spec.split),
        "acars-json" => read_acars_json(text, spec.split, spec.time_field.as_deref()),
        "csv" => read_csv(text, spec.split),
        "json" => read_json(text, spec.split)?,
        other => {
            return Err(format!(
                "{}: unknown format '{other}' (csv, json, nmea, acars-json)",
                spec.name
            ))
        }
    })
}

// ---------------------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------------------

/// One finding about a clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fault {
    /// `backward`, `step`, `week-rollover`, `century`, `wrap-32`, `gps-utc-offset`, `frozen`,
    /// `drift`, `offset`, `false-leap`, `leap-step`, `leap-second`, `reference-backward`,
    /// `absolute-epoch`, `forward-jump-or-gap`, `invalid-flag`.
    pub kind: String,
    /// `fault` (the time as logged cannot be trusted there) or `notice`.
    pub severity: String,
    /// Index of the record where it shows (0-based within the clock).
    pub index: usize,
    pub line: usize,
    pub log_utc: String,
    pub ref_utc: Option<String>,
    /// The corrected time of the record.
    pub corrected_utc: String,
    /// The step, offset or rate it rests on (s, or ppm for drift).
    pub value: f64,
    pub detail: String,
}

/// A run of records between steps, with its offset model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub n: usize,
    pub first_index: usize,
    pub last_index: usize,
    pub start_utc: String,
    pub end_utc: String,
    /// `reference-fit`, `reference-constant`, `bridged` or `assumed` (no reference anywhere:
    /// the first segment is taken as logged).
    pub anchored_by: String,
    /// The clock's offset (log minus true) at the segment's first record, s.
    pub offset_s: f64,
    pub drift_ppm: Option<f64>,
    pub u_drift_ppm: Option<f64>,
    /// Standard uncertainty of a corrected time in this segment, s.
    pub u_s: f64,
    /// RMS residual of the offset fit, s.
    pub rms_s: Option<f64>,
}

/// A time range where the clock's time as logged cannot be trusted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// The clock (`<log>` or `<log>:<series>`).
    pub clock: String,
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    pub reason: String,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
}

/// One clock's result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClockReport {
    /// `<log>` or `<log>:<series>`.
    pub clock: String,
    pub log: String,
    /// The reference group it is voted in.
    pub group: String,
    pub kind: Option<String>,
    pub format: String,
    pub records: usize,
    pub with_reference: usize,
    pub invalid_flagged: usize,
    pub cadence_s: Option<f64>,
    pub first_log_utc: Option<String>,
    pub last_log_utc: Option<String>,
    /// Median and largest |offset| to the reference over the log (s).
    pub median_offset_s: Option<f64>,
    pub max_abs_offset_s: Option<f64>,
    pub faults: Vec<Fault>,
    pub segments: Vec<Segment>,
    pub intervals: Vec<Interval>,
    /// Largest standard uncertainty of a corrected time (s).
    pub corrected_u_s: Option<f64>,
    /// `intact`, `faults`, or `unverified` (no reference: only the internal checks ran).
    pub status: String,
    pub statement: String,
    #[serde(skip)]
    pub timeline: Vec<TimelineRow>,
    #[serde(skip)]
    pub u_s: f64,
}

/// One row of the corrected timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineRow {
    pub index: usize,
    pub line: usize,
    pub t: f64,
    pub r: Option<f64>,
    pub corrected: f64,
    pub u: f64,
    pub segment: usize,
    pub ok: bool,
}

/// A cross-log disagreement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossEvent {
    /// The reference group.
    pub group: String,
    /// The vote status (`source-outlier`, `node-off` read as `reference-off`, `no-majority`,
    /// `pair-disagree`).
    pub status: String,
    pub start_utc: String,
    pub end_utc: String,
    pub bins: usize,
    pub outliers: Vec<String>,
    /// Consensus offset of the clocks against the reference (s), when there is one.
    pub consensus_s: Option<f64>,
    pub detail: String,
    #[serde(skip)]
    pub t0: f64,
    #[serde(skip)]
    pub t1: f64,
}

/// The cross-log comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossLog {
    pub clocks_compared: Vec<String>,
    pub bins_compared: usize,
    pub bins_agree: usize,
    pub events: Vec<CrossEvent>,
}

/// An input file's record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputFile {
    pub log: String,
    pub file: String,
    pub format: String,
    pub sha256: String,
    pub bytes: usize,
    pub unreadable_lines: usize,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub format: String,
    pub method: String,
    pub label: String,
    pub statement: String,
    pub title: String,
    pub params: Params,
    pub inputs: Vec<InputFile>,
    pub clocks: Vec<ClockReport>,
    pub cross_log: CrossLog,
    /// One line: intact, faults found, or unverified.
    pub verdict: String,
    /// Every interval of every clock, plus the cross-log disagreements, in time order.
    pub integrity_intervals: Vec<Interval>,
    /// The integrity statement for evidence use, one line per clock.
    pub integrity_statement: Vec<String>,
    pub limits: Vec<String>,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

fn is_step_kind(k: &str) -> bool {
    matches!(
        k,
        "backward" | "step" | "week-rollover" | "century" | "wrap-32" | "gps-utc-offset"
    )
}

struct Fit {
    a: f64,
    b: f64,
    r0: f64,
    u_b: Option<f64>,
    rms: f64,
}

/// Least-squares offset = a + b·(r − r0).
fn fit(points: &[(f64, f64)]) -> Option<Fit> {
    let n = points.len();
    if n == 0 {
        return None;
    }
    let r0 = points[0].0;
    let mr = points.iter().map(|p| p.0 - r0).sum::<f64>() / n as f64;
    let mo = points.iter().map(|p| p.1).sum::<f64>() / n as f64;
    let sxx: f64 = points.iter().map(|p| (p.0 - r0 - mr).powi(2)).sum();
    if n < 3 || sxx <= 0.0 {
        let rms = (points.iter().map(|p| (p.1 - mo).powi(2)).sum::<f64>() / n as f64).sqrt();
        return Some(Fit {
            a: mo,
            b: 0.0,
            r0,
            u_b: None,
            rms,
        });
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - r0 - mr) * (p.1 - mo)).sum();
    let b = sxy / sxx;
    let a = mo - b * mr;
    let ss: f64 = points
        .iter()
        .map(|p| (p.1 - a - b * (p.0 - r0)).powi(2))
        .sum();
    let s2 = ss / (n as f64 - 2.0);
    Some(Fit {
        a,
        b,
        r0,
        u_b: Some((s2 / sxx).sqrt()),
        rms: (ss / n as f64).sqrt(),
    })
}

struct Ctx<'a> {
    p: &'a Params,
    extra_leaps: &'a [i64],
    window: Option<(f64, f64)>,
}

fn fault(kind: &str, severity: &str, i: usize, rec: &Record, value: f64, detail: String) -> Fault {
    Fault {
        kind: kind.into(),
        severity: severity.into(),
        index: i,
        line: rec.line,
        log_utc: iso_ms(rec.t),
        ref_utc: rec.r.map(iso_ms),
        corrected_utc: String::new(),
        value: sig9(value),
        detail,
    }
}

fn near_midnight(t: f64, tol: f64) -> Option<i64> {
    let day = (t / 86_400.0).round() as i64;
    ((t - day as f64 * 86_400.0).abs() <= tol).then_some(day)
}

/// Check one clock.
fn check_clock(clock: &str, spec: &LogSpec, recs: &[Record], cx: &Ctx) -> ClockReport {
    let p = cx.p;
    let n = recs.len();
    let mut faults: Vec<Fault> = Vec::new();
    let with_ref = recs.iter().filter(|r| r.r.is_some()).count();
    let invalid = recs.iter().filter(|r| !r.valid).count();
    let mut dts: Vec<f64> = recs
        .windows(2)
        .map(|w| match (w[0].r, w[1].r) {
            (Some(a), Some(b)) => b - a,
            _ => w[1].t - w[0].t,
        })
        .filter(|d| *d > 1e-9)
        .collect();
    let cadence = median(&mut dts);
    let cad = cadence.unwrap_or(1.0);
    let lim = spec.offset_limit_s.unwrap_or(p.offset_limit_s);
    let mut frozen = vec![false; n];

    // Pass 1: steps, ordering, frozen runs, leap seconds.
    let mut run_start = 0usize;
    for i in 0..n {
        let rec = &recs[i];
        if rec.leap {
            let day = (rec.t / 86_400.0).round() as i64 - 1;
            if leap_day(day, cx.extra_leaps) {
                faults.push(fault(
                    "leap-second",
                    "notice",
                    i,
                    rec,
                    1.0,
                    "23:59:60 logged on a day that ended with an inserted leap second".into(),
                ));
            } else {
                faults.push(fault(
                    "false-leap",
                    "fault",
                    i,
                    rec,
                    1.0,
                    "23:59:60 logged on a day that ended with no leap second (engine table and request dates)".into(),
                ));
            }
        }
        if i == 0 {
            continue;
        }
        // Frozen: equal stamps while the reference (or the record count) advances.
        let last = &recs[i - 1];
        let advancing = match (last.r, rec.r) {
            (Some(a), Some(b)) => b - a > 1e-9,
            _ => true,
        };
        if (rec.t - last.t).abs() <= 1e-9 && advancing && !rec.leap && !last.leap {
            if i - run_start + 1 == p.frozen_n {
                faults.push(fault(
                    "frozen",
                    "fault",
                    run_start,
                    &recs[run_start],
                    0.0,
                    format!(
                        "the clock repeats {} from this record on while records keep arriving",
                        iso_ms(rec.t)
                    ),
                ));
            }
            if i - run_start + 1 >= p.frozen_n {
                for f in frozen.iter_mut().take(i + 1).skip(run_start) {
                    *f = true;
                }
            }
            continue;
        }
        // After a frozen run, compare with the run's first record (whose time was right).
        let base = if frozen[i - 1] { run_start } else { i - 1 };
        run_start = i;
        let prev = &recs[base];
        let dt = rec.t - prev.t;
        let expected = (i - base) as f64 * cad;
        if let (Some(a), Some(b)) = (prev.r, rec.r) {
            if b < a {
                faults.push(fault(
                    "reference-backward",
                    "notice",
                    i,
                    rec,
                    b - a,
                    "the reference stamp goes backwards: records out of order in the file, or the reference clock stepped".into(),
                ));
            }
            let dstep = dt - (b - a);
            if let Some(day) = near_midnight(rec.t, 3.0) {
                if (dstep.abs() - 1.0).abs() <= 0.25 {
                    let real = leap_day(day - 1, cx.extra_leaps);
                    faults.push(fault(
                        if real { "leap-step" } else { "false-leap" },
                        "fault",
                        i,
                        rec,
                        dstep,
                        if real {
                            format!("{dstep:+.3} s at midnight after a leap-second day: one of the clock and its reference did not apply the leap second")
                        } else {
                            format!("{dstep:+.3} s at midnight on a day with no leap second: a leap second applied that did not exist")
                        },
                    ));
                    continue;
                }
            }
            if dstep.abs() > p.step_s {
                let (k, d) = classify_step(dstep.round() as i64);
                let kind = if k == "time-jump" {
                    if dt < 0.0 {
                        "backward"
                    } else {
                        "step"
                    }
                } else {
                    k
                };
                faults.push(fault(
                    kind,
                    "fault",
                    i,
                    rec,
                    dstep,
                    format!("offset to the reference changed by {dstep:+.3} s ({d})"),
                ));
            }
        } else if dt < 0.0 {
            let step = dt - expected;
            let (k, d) = classify_step(step.round() as i64);
            faults.push(fault(
                if k == "time-jump" { "backward" } else { k },
                "fault",
                i,
                rec,
                step,
                format!("the clock ran backwards by {:.3} s ({d})", -dt),
            ));
        } else if dt - expected > cad * (p.gap_cadences - 1.0).max(0.0) {
            let step = dt - expected;
            let (k, d) = classify_step(step.round() as i64);
            if k == "time-jump" {
                faults.push(fault(
                    "forward-jump-or-gap",
                    "notice",
                    i,
                    rec,
                    step,
                    format!("{dt:.3} s between records against a cadence of {cad:.3} s: a data gap or a forward step (no reference to tell them apart; the timeline is not bridged here)"),
                ));
            } else {
                faults.push(fault(
                    k,
                    "fault",
                    i,
                    rec,
                    step,
                    format!("forward by {step:.0} s ({d})"),
                ));
            }
        }
    }
    if invalid > 0 {
        if let Some(i) = recs.iter().position(|r| !r.valid) {
            faults.push(fault(
                "invalid-flag",
                "notice",
                i,
                &recs[i],
                invalid as f64,
                format!("{invalid} record(s) flagged invalid by the device (e.g. RMC status V)"),
            ));
        }
    }

    // Segments between steps.
    let mut cuts: Vec<usize> = faults
        .iter()
        .filter(|f| {
            is_step_kind(&f.kind)
                || f.kind == "leap-step"
                || (f.kind == "false-leap"
                    && f.value.abs() > 0.5
                    && f.ref_utc.is_some()
                    && !recs[f.index].leap)
        })
        .map(|f| f.index)
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut bounds = vec![0usize];
    bounds.extend(cuts.iter().copied().filter(|c| *c > 0 && *c < n));
    bounds.push(n);
    bounds.dedup();

    struct Seg {
        lo: usize,
        hi: usize, // exclusive
        fit: Option<Fit>,
        // offset of the first record (log − true) when bridged / assumed
        bridged: Option<(f64, f64)>, // (offset, u)
        anchored_by: &'static str,
    }
    let mut segs: Vec<Seg> = bounds
        .windows(2)
        .filter(|w| w[1] > w[0])
        .map(|w| {
            let pts: Vec<(f64, f64)> = (w[0]..w[1])
                .filter(|i| !frozen[*i])
                .filter_map(|i| recs[i].r.map(|r| (r, recs[i].t - r)))
                .collect();
            let f = fit(&pts);
            let anchored_by = match pts.len() {
                0 => "",
                1 | 2 => "reference-constant",
                _ => "reference-fit",
            };
            Seg {
                lo: w[0],
                hi: w[1],
                fit: f,
                bridged: None,
                anchored_by,
            }
        })
        .collect();
    // Offset of the clock at record i under its segment's model (log − true).
    let offset_at = |s: &Seg, i: usize| -> f64 {
        if let Some(f) = &s.fit {
            let x = recs[i].r.unwrap_or(recs[i].t - f.a);
            f.a + f.b * (x - f.r0)
        } else {
            s.bridged.map(|b| b.0).unwrap_or(0.0)
        }
    };
    let u_of = |s: &Seg, base_u: f64| -> f64 {
        match (&s.fit, s.bridged) {
            (Some(f), _) => (f.rms * f.rms + base_u * base_u).sqrt(),
            (None, Some((_, u))) => u,
            _ => base_u,
        }
    };
    let anchored: Vec<bool> = segs.iter().map(|s| s.fit.is_some()).collect();
    let any_anchor = anchored.iter().any(|a| *a);
    if !any_anchor && !segs.is_empty() {
        segs[0].bridged = Some((0.0, 0.0));
        segs[0].anchored_by = "assumed";
    }
    // Bridge forward from the previous segment, then backward for leading unanchored ones.
    for k in 1..segs.len() {
        if segs[k].fit.is_some() || segs[k].bridged.is_some() {
            continue;
        }
        let prev = &segs[k - 1];
        if prev.fit.is_none() && prev.bridged.is_none() {
            continue;
        }
        let last = prev.hi - 1;
        let corr_last = recs[last].t - offset_at(prev, last);
        let u_prev = u_of(prev, spec.u_s.min(cad / 2.0));
        let first = segs[k].lo;
        let off = recs[first].t - (corr_last + cad);
        let u = (u_prev * u_prev + (cad / 2.0).powi(2)).sqrt();
        segs[k].bridged = Some((off, u));
        segs[k].anchored_by = "bridged";
    }
    for k in (0..segs.len().saturating_sub(1)).rev() {
        if segs[k].fit.is_some() || segs[k].bridged.is_some() {
            continue;
        }
        let next = &segs[k + 1];
        let first_next = next.lo;
        let corr_first = recs[first_next].t - offset_at(next, first_next);
        let u_next = u_of(next, spec.u_s.min(cad / 2.0));
        let last = segs[k].hi - 1;
        let off = recs[last].t - (corr_first - cad);
        let u = (u_next * u_next + (cad / 2.0).powi(2)).sqrt();
        segs[k].bridged = Some((off, u));
        segs[k].anchored_by = "bridged";
    }

    // Timeline and per-record trust.
    let base_u = if with_ref > 0 { spec.u_s } else { 0.0 };
    let mut timeline = Vec::with_capacity(n);
    let mut offsets: Vec<f64> = Vec::new();
    for (si, s) in segs.iter().enumerate() {
        let u = u_of(s, base_u);
        let mut frozen_base: Option<(usize, f64)> = None;
        for i in s.lo..s.hi {
            let rec = &recs[i];
            let corrected = if frozen[i] {
                match rec.r {
                    Some(r) => r,
                    None => {
                        let (k0, c0) = *frozen_base.get_or_insert((i, rec.t - offset_at(s, i)));
                        c0 + (i - k0) as f64 * cad
                    }
                }
            } else {
                rec.t - offset_at(s, i)
            };
            if let Some(r) = rec.r {
                offsets.push(rec.t - r);
            }
            let off_ok = match rec.r {
                Some(r) => (rec.t - r).abs() <= lim,
                None => s.anchored_by != "bridged",
            };
            timeline.push(TimelineRow {
                index: i,
                line: rec.line,
                t: rec.t,
                r: rec.r,
                corrected,
                u,
                segment: si + 1,
                ok: off_ok && !frozen[i] && !rec.leap_false(cx.extra_leaps),
            });
        }
    }
    for f in &mut faults {
        if let Some(row) = timeline.get(f.index) {
            f.corrected_utc = iso_ms(row.corrected);
        }
    }

    // Segment reports, drift and offset faults.
    let mut seg_out = Vec::new();
    for (si, s) in segs.iter().enumerate() {
        let first = &timeline[s.lo];
        let last = &timeline[s.hi - 1];
        let (drift, u_drift, rms) = match &s.fit {
            Some(f) => (
                f.u_b.map(|_| f.b * 1e6),
                f.u_b.map(|u| u * 1e6),
                Some(f.rms),
            ),
            None => (None, None, None),
        };
        let span = match (recs[s.lo].r, recs[s.hi - 1].r) {
            (Some(a), Some(b)) => b - a,
            _ => 0.0,
        };
        if let (Some(d), Some(ud)) = (drift, u_drift) {
            if d.abs() > p.drift_ppm && d.abs() > 3.0 * ud && span >= p.min_drift_span_s {
                let mut f = fault(
                    "drift",
                    "fault",
                    s.lo,
                    &recs[s.lo],
                    d,
                    format!(
                        "the clock drifts {d:+.1} ± {ud:.1} ppm against its reference over {span:.0} s (limit {} ppm)",
                        p.drift_ppm
                    ),
                );
                f.corrected_utc = iso_ms(first.corrected);
                faults.push(f);
            }
        }
        let seg_offset = offset_at(s, s.lo);
        if s.fit.is_some() {
            let mut offs: Vec<f64> = (s.lo..s.hi)
                .filter(|i| !frozen[*i])
                .filter_map(|i| recs[i].r.map(|r| recs[i].t - r))
                .collect();
            if let Some(m) = median(&mut offs) {
                if m.abs() > lim {
                    let (k, d) = classify_step(m.round() as i64);
                    let already = faults
                        .iter()
                        .any(|f| f.index == s.lo && f.kind == k && k != "time-jump");
                    if !already {
                        let mut f = fault(
                            if k == "time-jump" { "offset" } else { k },
                            "fault",
                            s.lo,
                            &recs[s.lo],
                            m,
                            format!(
                                "the clock runs {m:+.3} s from its reference over this segment (limit {lim} s; {d})"
                            ),
                        );
                        f.corrected_utc = iso_ms(first.corrected);
                        faults.push(f);
                    }
                }
            }
        }
        seg_out.push(Segment {
            n: si + 1,
            first_index: s.lo,
            last_index: s.hi - 1,
            start_utc: iso_ms(first.corrected),
            end_utc: iso_ms(last.corrected),
            anchored_by: s.anchored_by.into(),
            offset_s: sig9(seg_offset),
            drift_ppm: drift.map(sig9),
            u_drift_ppm: u_drift.map(sig9),
            u_s: sig9(first.u),
            rms_s: rms.map(sig9),
        });
    }

    // Absolute epoch check without a reference.
    if with_ref == 0 {
        if let (Some((w0, w1)), Some(first)) = (cx.window, recs.first()) {
            let outside = if first.t < w0 - 86_400.0 {
                Some(first.t - w0)
            } else if first.t > w1 + 86_400.0 {
                Some(first.t - w1)
            } else {
                None
            };
            if let Some(dx) = outside {
                let (k, d) = classify_step(dx.round() as i64);
                faults.push(Fault {
                    corrected_utc: iso_ms(first.t),
                    ..fault(
                        "absolute-epoch",
                        "fault",
                        0,
                        first,
                        dx,
                        format!(
                            "the log's first time stamp lies {:.1} days outside the incident window ({k}: {d})",
                            dx / 86_400.0
                        ),
                    )
                });
            }
        }
    }
    faults.sort_by(|a, b| a.index.cmp(&b.index).then(a.kind.cmp(&b.kind)));

    // Intervals of untrusted time.
    let mut intervals = Vec::new();
    let mut i = 0;
    while i < timeline.len() {
        if timeline[i].ok {
            i += 1;
            continue;
        }
        let j0 = i;
        while i < timeline.len() && !timeline[i].ok {
            i += 1;
        }
        let (a, b) = (&timeline[j0], &timeline[i - 1]);
        let reason = interval_reason(&faults, &frozen, a.index, b.index, recs, lim);
        let (t0, t1) = (a.corrected, b.corrected);
        intervals.push(Interval {
            clock: clock.into(),
            start_utc: iso_ms(t0),
            end_utc: iso_ms(t1),
            duration_s: sig9(t1 - t0),
            reason,
            t0,
            t1,
        });
    }

    let max_abs = offsets
        .iter()
        .map(|x| x.abs())
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))));
    let med = median(&mut offsets.clone());
    let n_fault = faults.iter().filter(|f| f.severity == "fault").count();
    let status = if n_fault > 0 {
        "faults"
    } else if with_ref == 0 {
        "unverified"
    } else {
        "intact"
    };
    let u_max = timeline
        .iter()
        .map(|r| r.u)
        .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x))));
    let kinds: Vec<String> = {
        let mut k: Vec<String> = faults
            .iter()
            .filter(|f| f.severity == "fault")
            .map(|f| f.kind.clone())
            .collect();
        k.sort();
        k.dedup();
        k
    };
    let statement = match status {
        "intact" => format!(
            "{clock}: {} records, offset to its reference within {:.3} s (largest), median {:+.3} s; no step, freeze, rollover or leap fault under the thresholds.",
            n,
            max_abs.unwrap_or(0.0),
            med.unwrap_or(0.0)
        ),
        "unverified" => format!(
            "{clock}: {n} records with no reference stamp: only the internal checks ran (ordering, freeze, rollover, leap); its absolute time is NOT verified."
        ),
        _ => format!(
            "{clock}: {n_fault} fault(s) ({}); {} untrusted interval(s) totalling {:.1} s; corrected timeline with standard uncertainty up to {:.3} s.",
            kinds.join(", "),
            intervals.len(),
            intervals.iter().map(|x| x.duration_s).sum::<f64>(),
            u_max.unwrap_or(0.0)
        ),
    };
    ClockReport {
        clock: clock.into(),
        log: spec.name.clone(),
        group: spec
            .reference_group
            .clone()
            .unwrap_or_else(|| spec.name.clone()),
        kind: spec.kind.clone(),
        format: spec.format.clone(),
        records: n,
        with_reference: with_ref,
        invalid_flagged: invalid,
        cadence_s: cadence.map(sig9),
        first_log_utc: recs.first().map(|r| iso_ms(r.t)),
        last_log_utc: recs.last().map(|r| iso_ms(r.t)),
        median_offset_s: med.map(sig9),
        max_abs_offset_s: max_abs.map(sig9),
        faults,
        segments: seg_out,
        intervals,
        corrected_u_s: u_max.map(sig9),
        status: status.into(),
        statement,
        timeline,
        u_s: spec.u_s,
    }
}

impl Record {
    fn leap_false(&self, extra: &[i64]) -> bool {
        self.leap && !leap_day((self.t / 86_400.0).round() as i64 - 1, extra)
    }
}

fn interval_reason(
    faults: &[Fault],
    frozen: &[bool],
    a: usize,
    b: usize,
    recs: &[Record],
    lim: f64,
) -> String {
    let mut why: Vec<String> = faults
        .iter()
        .filter(|f| f.severity == "fault" && f.index <= b)
        .filter(|f| f.index >= a || is_step_kind(&f.kind) || f.kind == "offset")
        .map(|f| f.kind.clone())
        .collect();
    if frozen[a..=b].iter().any(|x| *x) {
        why.push("frozen".into());
    }
    if recs[a..=b]
        .iter()
        .any(|r| r.r.is_some_and(|x| (r.t - x).abs() > lim))
    {
        why.push("offset-beyond-limit".into());
    }
    why.sort();
    why.dedup();
    if why.is_empty() {
        "corrected by bridging a step (no reference)".into()
    } else {
        why.join(", ")
    }
}

fn cross_log(clocks: &[ClockReport], p: &Params) -> CrossLog {
    let mut groups: Vec<&str> = clocks.iter().map(|c| c.group.as_str()).collect();
    groups.sort_unstable();
    groups.dedup();
    let mut out = CrossLog {
        clocks_compared: vec![],
        bins_compared: 0,
        bins_agree: 0,
        events: vec![],
    };
    for g in groups {
        let members: Vec<ClockReport> = clocks.iter().filter(|c| c.group == g).cloned().collect();
        let r = cross_group(g, &members, p);
        out.clocks_compared.extend(r.clocks_compared);
        out.bins_compared += r.bins_compared;
        out.bins_agree += r.bins_agree;
        out.events.extend(r.events);
    }
    out
}

fn cross_group(group: &str, clocks: &[ClockReport], p: &Params) -> CrossLog {
    // Per clock, the median offset per bin of reference time.
    let mut grid: BTreeMap<i64, Vec<(String, f64, f64)>> = BTreeMap::new();
    let mut names = Vec::new();
    for c in clocks.iter().filter(|c| c.with_reference > 0) {
        names.push(c.clock.clone());
        let mut bins: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
        for row in &c.timeline {
            if let Some(r) = row.r {
                bins.entry((r / p.bin_s).floor() as i64)
                    .or_default()
                    .push(row.t - r);
            }
        }
        for (b, mut v) in bins {
            if let Some(m) = median(&mut v) {
                grid.entry(b).or_default().push((c.clock.clone(), m, c.u_s));
            }
        }
    }
    if names.len() < 2 {
        return CrossLog {
            clocks_compared: vec![],
            bins_compared: 0,
            bins_agree: 0,
            events: vec![],
        };
    }
    let mut out = CrossLog {
        clocks_compared: names,
        bins_compared: 0,
        bins_agree: 0,
        events: vec![],
    };
    let mut open: Option<CrossEvent> = None;
    let mut prev_bin: Option<i64> = None;
    for (b, cells) in &grid {
        if cells.len() < 2 {
            continue;
        }
        out.bins_compared += 1;
        let input: Vec<(String, f64, f64, bool, Option<bool>)> = cells
            .iter()
            .map(|(n, o, u)| (n.clone(), *o, *u, false, None))
            .collect();
        let v = selfcheck::vote(&input, p.k, p.floor_s);
        let status = match v.status.as_str() {
            "agree" | "pair-agree" => None,
            "node-off" => Some("reference-off".to_string()),
            s => Some(s.to_string()),
        };
        let t0 = *b as f64 * p.bin_s;
        let t1 = t0 + p.bin_s;
        match status {
            None => {
                out.bins_agree += 1;
                if let Some(e) = open.take() {
                    out.events.push(e);
                }
            }
            Some(s) => {
                let contiguous = prev_bin == Some(b - 1);
                let same = open
                    .as_ref()
                    .is_some_and(|e| e.status == s && e.outliers == v.outliers && contiguous);
                if same {
                    let e = open.as_mut().expect("open");
                    e.bins += 1;
                    e.t1 = t1;
                    e.end_utc = iso_ms(t1);
                } else {
                    if let Some(e) = open.take() {
                        out.events.push(e);
                    }
                    let detail = match s.as_str() {
                        "source-outlier" => format!(
                            "{} disagree(s) with the other clocks' consensus",
                            v.outliers.join(", ")
                        ),
                        "reference-off" => format!(
                            "the clocks agree with one another but sit {:+.3} s from the common reference: the reference stamp itself is off",
                            v.consensus_s.unwrap_or(0.0)
                        ),
                        "pair-disagree" => "two clocks disagree; with only two the outlier cannot be named".into(),
                        _ => "no majority among the clocks".into(),
                    };
                    open = Some(CrossEvent {
                        group: group.to_string(),
                        status: s,
                        start_utc: iso_ms(t0),
                        end_utc: iso_ms(t1),
                        bins: 1,
                        outliers: v.outliers.clone(),
                        consensus_s: v.consensus_s.map(sig9),
                        detail,
                        t0,
                        t1,
                    });
                }
            }
        }
        prev_bin = Some(*b);
    }
    if let Some(e) = open.take() {
        out.events.push(e);
    }
    out
}

/// Check every log of a request; paths are relative to `base`.
pub fn check(req: &Request, base: &Path) -> Result<Report, String> {
    let mut texts = Vec::new();
    for l in &req.logs {
        let p = resolve(base, &l.path);
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        texts.push(bytes);
    }
    check_texts(req, &texts)
}

fn resolve(base: &Path, p: &str) -> PathBuf {
    if Path::new(p).is_relative() {
        base.join(p)
    } else {
        PathBuf::from(p)
    }
}

/// Check a request with the logs' bytes given in the order of `req.logs`.
pub fn check_texts(req: &Request, texts: &[Vec<u8>]) -> Result<Report, String> {
    let p = &req.params;
    if p.step_s <= 0.0 || p.bin_s <= 0.0 || p.frozen_n < 2 || p.offset_limit_s <= 0.0 {
        return Err("params: step_s, bin_s and offset_limit_s must be > 0 and frozen_n ≥ 2".into());
    }
    let mut extra = Vec::new();
    for d in &req.leap_dates {
        let l = UtcLabel::parse_iso(d).map_err(|e| format!("leap_dates: {e}"))?;
        extra.push(l.days);
    }
    let w = |s: &Option<String>| -> Result<Option<f64>, String> {
        match s {
            None => Ok(None),
            Some(x) => parse_time(x)
                .map(|s| Some(s.t))
                .ok_or_else(|| format!("invalid window time '{x}'")),
        }
    };
    let window = match (w(&req.window_start_utc)?, w(&req.window_end_utc)?) {
        (Some(a), Some(b)) => Some((a, b)),
        (Some(a), None) => Some((a, a)),
        (None, Some(b)) => Some((b, b)),
        _ => None,
    };
    let cx = Ctx {
        p,
        extra_leaps: &extra,
        window,
    };
    let mut inputs = Vec::new();
    let mut clocks = Vec::new();
    for (spec, bytes) in req.logs.iter().zip(texts) {
        let text = String::from_utf8_lossy(bytes);
        let rl = read_log(spec, &text)?;
        inputs.push(InputFile {
            log: spec.name.clone(),
            file: Path::new(&spec.path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| spec.path.clone()),
            format: spec.format.clone(),
            sha256: sha256_hex(bytes),
            bytes: bytes.len(),
            unreadable_lines: rl.unreadable,
        });
        if rl.series.is_empty() {
            return Err(format!(
                "{}: no time-stamped record could be read as {}",
                spec.name, spec.format
            ));
        }
        for (s, recs) in &rl.series {
            let clock = if s.is_empty() {
                spec.name.clone()
            } else {
                format!("{}:{s}", spec.name)
            };
            clocks.push(check_clock(&clock, spec, recs, &cx));
        }
    }
    let cross = cross_log(&clocks, p);
    let mut intervals: Vec<Interval> = clocks.iter().flat_map(|c| c.intervals.clone()).collect();
    for e in &cross.events {
        intervals.push(Interval {
            clock: if e.outliers.is_empty() {
                "cross-log".into()
            } else {
                format!("cross-log:{}", e.outliers.join("+"))
            },
            start_utc: e.start_utc.clone(),
            end_utc: e.end_utc.clone(),
            duration_s: sig9(e.t1 - e.t0),
            reason: e.status.clone(),
            t0: e.t0,
            t1: e.t1,
        });
    }
    intervals.sort_by(|a, b| a.t0.total_cmp(&b.t0).then(a.clock.cmp(&b.clock)));
    let n_fault: usize = clocks
        .iter()
        .map(|c| c.faults.iter().filter(|f| f.severity == "fault").count())
        .sum();
    let unverified = clocks.iter().filter(|c| c.status == "unverified").count();
    let verdict = if n_fault > 0 || !cross.events.is_empty() {
        format!(
            "TIME INTEGRITY FAULTS: {n_fault} clock fault(s) in {} of {} clock(s), {} cross-log disagreement(s); use the corrected timeline and the integrity intervals",
            clocks.iter().filter(|c| c.status == "faults").count(),
            clocks.len(),
            cross.events.len()
        )
    } else if unverified == clocks.len() {
        "UNVERIFIED: no clock has a reference stamp; the internal checks found no fault".into()
    } else {
        format!(
            "INTACT under the thresholds: {} clock(s) checked, {} against a reference{}",
            clocks.len(),
            clocks.len() - unverified,
            if cross.bins_compared > 0 {
                format!(", {} cross-log bins agree", cross.bins_agree)
            } else {
                String::new()
            }
        )
    };
    let mut statement: Vec<String> = clocks.iter().map(|c| c.statement.clone()).collect();
    if cross.bins_compared > 0 {
        statement.push(format!(
            "Cross-log (within reference groups): {} clocks compared in {} bins of {} s; {} agree; {} disagreement(s).",
            cross.clocks_compared.len(),
            cross.bins_compared,
            p.bin_s,
            cross.bins_agree,
            cross.events.len()
        ));
    }
    Ok(Report {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        label: LABEL.into(),
        statement: STATEMENT.into(),
        title: req.title.clone(),
        params: p.clone(),
        inputs,
        clocks,
        cross_log: cross,
        verdict,
        integrity_intervals: intervals,
        integrity_statement: statement,
        limits: vec![
            "The reference stamp of each record (recorder clock, ground station receive time, ref_time column) is taken as given; if it is wrong, the cross-log vote can show that only when at least two other clocks share it.".into(),
            "Without a reference the absolute time is not verified: only ordering, freezes, rollovers, leap seconds and (with a window) the epoch are checked, and a forward jump cannot be told from a data gap.".into(),
            "A bridged segment assumes the clock stepped once and then ran at the log's cadence; its uncertainty grows by half a cadence per bridge.".into(),
            "Leap seconds are judged against the open engine's table and the request's leap_dates; a leap second announced after the table must be added there.".into(),
            "Tested on synthetic logs with known truth only.".into(),
        ],
    })
}

/// The corrected timeline as CSV.
pub fn timeline_csv(r: &Report) -> String {
    let mut s = String::from(
        "clock,index,line,log_utc,ref_utc,corrected_utc,u_s,offset_s,segment,trusted\n",
    );
    for c in &r.clocks {
        for row in &c.timeline {
            s.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{}\n",
                c.clock,
                row.index,
                row.line,
                iso_ms(row.t),
                row.r.map(iso_ms).unwrap_or_default(),
                iso_ms(row.corrected),
                sig9(row.u),
                row.r
                    .map(|x| sig9(row.t - x).to_string())
                    .unwrap_or_default(),
                row.segment,
                row.ok
            ));
        }
    }
    s
}

/// Human-readable summary.
pub fn render_text(r: &Report) -> String {
    let mut s = format!("Log time integrity: {}\n{}\n\n", r.title, r.verdict);
    for c in &r.clocks {
        s.push_str(&format!(
            "[{}] {} — {} records, {} with reference, cadence {}\n",
            c.status.to_uppercase(),
            c.clock,
            c.records,
            c.with_reference,
            c.cadence_s
                .map(|x| format!("{x} s"))
                .unwrap_or_else(|| "n/a".into())
        ));
        for f in &c.faults {
            s.push_str(&format!(
                "  {:<7} {:<20} line {:<6} {}  {}\n",
                f.severity, f.kind, f.line, f.log_utc, f.detail
            ));
        }
        for i in &c.intervals {
            s.push_str(&format!(
                "  untrusted {} .. {} ({:.1} s): {}\n",
                i.start_utc, i.end_utc, i.duration_s, i.reason
            ));
        }
    }
    if !r.cross_log.events.is_empty() {
        s.push_str("\nCross-log disagreements:\n");
        for e in &r.cross_log.events {
            s.push_str(&format!(
                "  [{}] {} .. {} {:<15} {}\n",
                e.group, e.start_utc, e.end_utc, e.status, e.detail
            ));
        }
    }
    s.push_str("\nIntegrity statement:\n");
    for l in &r.integrity_statement {
        s.push_str(&format!("  {l}\n"));
    }
    s.push_str(&format!("\n{}\n{}\n", r.statement, r.label));
    s
}

/// Write `log-time-report.json`, `timeline.csv`, `README.txt` and `inputs/<file>`.
pub fn write_dir(
    dir: &Path,
    r: &Report,
    req_text: &str,
    files: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir.join("inputs")).map_err(io)?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(r).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("timeline.csv"), timeline_csv(r)).map_err(io)?;
    std::fs::write(dir.join("request.toml"), req_text).map_err(io)?;
    for (n, b) in files {
        std::fs::write(dir.join("inputs").join(n), b).map_err(io)?;
    }
    std::fs::write(dir.join("README.txt"), render_text(r)).map_err(io)?;
    Ok(())
}

/// Synthetic logs with known truth (tests and the demo).
pub mod synth {
    use super::iso_ms;

    /// 2026-06-01T08:00:00Z.
    pub const T0: f64 = 1_780_300_800.0;
    /// Records per log (1 Hz).
    pub const N: usize = 1200;
    /// The record where the injected fault starts.
    pub const AT: usize = 600;

    fn cs(body: &str) -> String {
        let x = body.bytes().fold(0u8, |a, b| a ^ b);
        format!("${body}*{x:02X}")
    }

    fn hms(t: f64) -> String {
        let s = t.rem_euclid(86_400.0);
        let whole = s.floor() as i64;
        format!(
            "{:02}{:02}{:02}.{:02}",
            whole / 3600,
            whole / 60 % 60,
            whole % 60,
            ((s - whole as f64) * 100.0).round() as i64 % 100
        )
    }

    fn ymd(t: f64) -> (i64, i64, i64) {
        kshana_eloran::time::civil_from_days((t / 86_400.0).floor() as i64)
    }

    /// A ZDA sentence for time `t`.
    pub fn zda(talker: &str, t: f64) -> String {
        let (y, m, d) = ymd(t);
        cs(&format!(
            "{talker}ZDA,{},{d:02},{m:02},{y:04},00,00",
            hms(t)
        ))
    }

    /// An RMC sentence for time `t`.
    pub fn rmc(talker: &str, t: f64) -> String {
        let (y, m, d) = ymd(t);
        cs(&format!(
            "{talker}RMC,{},A,2620.000,N,05630.000,E,12.0,090.0,{d:02}{m:02}{:02},,,A",
            hms(t),
            y % 100
        ))
    }

    /// The faults the generators inject.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Fault {
        None,
        /// The clock steps back 37 s at [`AT`] and runs on.
        Backwards,
        /// The second receiver's week number is resolved 1024 weeks early from [`AT`].
        WeekRollover,
        /// The clock freezes for 20 records from [`AT`].
        Frozen,
        /// The clock drifts 200 ppm from the start.
        Drift,
        /// GPS time logged as UTC (+18 s) from [`AT`].
        GpsAsUtc,
    }

    fn device(f: Fault, k: usize, truth: f64) -> f64 {
        match f {
            Fault::None => truth,
            Fault::Backwards if k >= AT => truth - 37.0,
            Fault::WeekRollover if k >= AT => truth - 1024.0 * 604_800.0,
            Fault::Frozen if (AT..AT + 20).contains(&k) => T0 + AT as f64,
            Fault::Drift => truth + (truth - T0) * 200e-6,
            Fault::GpsAsUtc if k >= AT => truth + 18.0,
            _ => truth,
        }
    }

    /// A VDR NMEA export: each line stamped by the recorder (ISO, the reference), with ZDA from
    /// two receivers: `GP` (healthy) and `GN` (carrying the fault). Returns the text and the
    /// true times.
    pub fn vdr(f: Fault) -> (String, Vec<f64>) {
        let mut s = String::new();
        let mut truth = Vec::new();
        for k in 0..N {
            let t = T0 + k as f64;
            truth.push(t);
            let stamp = iso_ms(t + 0.12);
            s.push_str(&format!("{stamp} {}\n", zda("GP", t)));
            s.push_str(&format!("{stamp} {}\n", zda("GN", device(f, k, t))));
            if k % 10 == 0 {
                s.push_str(&format!("{stamp} {}\n", rmc("II", t)));
            }
        }
        (s, truth)
    }

    /// A CPDLC/ACARS log as JSON lines: ground station receive `timestamp` (reference), the
    /// aircraft's `msg_time`; aircraft `.G-ABCD` carries the fault, `.D-EFGH` and `.F-IJKL` are
    /// healthy. One message per aircraft every 30 s.
    pub fn acars(f: Fault) -> String {
        let mut s = String::new();
        for k in (0..N).step_by(30) {
            let t = T0 + k as f64;
            for (i, tail) in [".G-ABCD", ".D-EFGH", ".F-IJKL"].iter().enumerate() {
                let tt = t + i as f64 * 2.0;
                let dev = if i == 0 { device(f, k, tt) } else { tt };
                let rx = tt + 1.5;
                s.push_str(&format!(
                    "{{\"timestamp\":{rx:.3},\"freq\":131.725,\"mode\":\"2\",\"label\":\"H1\",\"tail\":\"{tail}\",\"flight\":\"XX{i:03}\",\"msg_time\":\"{}\",\"text\":\"CPDLC DOWNLINK\"}}\n",
                    iso_ms(dev)
                ));
            }
        }
        s
    }

    /// A log with no reference at all (the documented CSV schema, `time` only): an FDM or
    /// GMDSS console export. Returns the text and the true times.
    pub fn bare_csv(f: Fault) -> (String, Vec<f64>) {
        let mut s = String::from("time\n");
        let mut truth = Vec::new();
        for k in 0..N {
            let t = T0 + k as f64;
            truth.push(t);
            s.push_str(&format!("{}\n", iso_ms(device(f, k, t))));
        }
        (s, truth)
    }

    /// The demo request (three logs) as TOML.
    pub const DEMO_REQUEST: &str = r#"title = "Synthetic: VDR, CPDLC and console clocks"
window_start_utc = "2026-06-01T08:00:00Z"
window_end_utc = "2026-06-01T08:20:00Z"

[[logs]]
name = "vdr"
kind = "vdr"
format = "nmea"
path = "vdr.nmea"
u_s = 0.2
reference = "VDR recorder stamp (GNSS disciplined)"

[[logs]]
name = "cpdlc"
kind = "cpdlc"
format = "acars-json"
path = "cpdlc.jsonl"
u_s = 2.0
offset_limit_s = 3.0
reference = "ground station receive time (trails the message time by the link latency)"

[[logs]]
name = "console"
kind = "gmdss"
format = "csv"
path = "console.csv"
"#;

    /// Write the demo request and its logs into `dir`.
    pub fn write_demo(dir: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("vdr.nmea"), vdr(Fault::WeekRollover).0)?;
        std::fs::write(dir.join("cpdlc.jsonl"), acars(Fault::Backwards))?;
        std::fs::write(dir.join("console.csv"), bare_csv(Fault::Backwards).0)?;
        std::fs::write(dir.join("request.toml"), DEMO_REQUEST)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(format: &str, text: &str) -> Report {
        let req = Request {
            title: "t".into(),
            logs: vec![LogSpec {
                name: "log".into(),
                kind: None,
                format: format.into(),
                path: "x".into(),
                u_s: 0.5,
                split: true,
                time_field: None,
                offset_limit_s: None,
                reference_group: None,
                reference: None,
            }],
            ..Default::default()
        };
        check_texts(&req, &[text.as_bytes().to_vec()]).expect("checks")
    }

    #[test]
    fn parses_iso_with_fraction_and_leap() {
        let s = parse_time("2016-12-31T23:59:60.500Z").unwrap();
        assert!(s.leap);
        assert_eq!(s.t, 1_483_228_800.5);
        assert_eq!(parse_time("1780300800.25").unwrap().t, 1_780_300_800.25);
        assert!(parse_time("garbage").is_none());
        assert_eq!(iso_ms(1_780_300_800.25), "2026-06-01T08:00:00.250Z");
    }

    #[test]
    fn leap_table() {
        let d = days_of(2016, 12, 31);
        assert!(leap_day(d, &[]));
        assert!(!leap_day(days_of(2026, 6, 30), &[]));
        assert!(leap_day(days_of(2026, 6, 30), &[days_of(2026, 6, 30)]));
    }

    #[test]
    fn sig9_rounds() {
        assert_eq!(sig9(1.234_567_891_23), 1.234_567_89);
        assert_eq!(sig9(-0.000_123_456_789_9), -0.000_123_456_79);
        assert_eq!(sig9(0.0), 0.0);
    }

    #[test]
    fn ts_field_parses() {
        let s = ts_field("POS N5012 /TS080102,010626 FL350").unwrap();
        assert_eq!(s.t, synth::T0 + 62.0);
        assert!(ts_field("no time here").is_none());
    }

    #[test]
    fn clean_csv_with_reference_is_intact() {
        let mut s = String::from("time,ref_time\n");
        for k in 0..100 {
            let t = synth::T0 + k as f64;
            s.push_str(&format!("{},{}\n", iso_ms(t + 0.05), iso_ms(t)));
        }
        let r = one("csv", &s);
        assert_eq!(r.clocks[0].status, "intact", "{:#?}", r.clocks[0].faults);
        assert!(r.integrity_intervals.is_empty());
    }

    #[test]
    fn frozen_bare_log() {
        let (s, _) = synth::bare_csv(synth::Fault::Frozen);
        let r = one("csv", &s);
        let c = &r.clocks[0];
        assert!(
            c.faults.iter().any(|f| f.kind == "frozen"),
            "{:#?}",
            c.faults
        );
        assert_eq!(c.intervals.len(), 1);
    }

    #[test]
    fn false_leap_flagged() {
        let s = "time\n2026-06-30T23:59:58Z\n2026-06-30T23:59:59Z\n2026-06-30T23:59:60Z\n2026-07-01T00:00:00Z\n2026-07-01T00:00:01Z\n";
        let r = one("csv", s);
        assert!(r.clocks[0].faults.iter().any(|f| f.kind == "false-leap"));
        let s2 = s
            .replace("2026-06-30", "2016-12-31")
            .replace("2026-07-01", "2017-01-01");
        let r2 = one("csv", &s2);
        assert!(r2.clocks[0].faults.iter().any(|f| f.kind == "leap-second"));
        assert!(!r2.clocks[0].faults.iter().any(|f| f.kind == "false-leap"));
    }

    #[test]
    fn json_schema_reads() {
        let s = r#"[{"time":"2026-06-01T08:00:00Z","ref_time":1780300800,"source":"a"},{"time":"2026-06-01T08:00:01Z","ref_time":1780300801,"source":"a","valid":false}]"#;
        let r = one("json", s);
        assert_eq!(r.clocks[0].clock, "log:a");
        assert_eq!(r.clocks[0].invalid_flagged, 1);
    }
}
