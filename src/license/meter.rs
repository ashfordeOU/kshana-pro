// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Meter log (backlog E39 B3901; docs/KS-INSIDE.md)
//!
//! Per-call, per-unit metering for embedded use (the KS-Inside SDK, [`crate::ks_inside`]).
//! Every metered call, allowed or refused, appends one JSON line **before** its result is
//! returned; when the line cannot be written the call fails, so there is no unmetered use.
//!
//! Each line carries the SHA-256 of the line before it (like the issuance register,
//! [`super::register`]), so an edited, reordered or deleted line breaks the chain
//! ([`verify`]). A unit (an OEM's device serial) is stored only as a salted hash bound to
//! the licence ([`unit_hash`]): the vendor can check a given serial during an audit, but the
//! log does not list serials in clear. The log holds no input values, only their SHA-256.
//!
//! What it is not: tamper-proof. Whoever controls the device can delete the whole file or
//! stop writing it by removing the SDK. The chain makes partial edits evident; the per-unit
//! royalty report ([`summarise`]) carries the hash of the last line, so the vendor can match
//! a later report to an earlier one. One log has one writer: give each process (or unit)
//! its own file.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kshana::api::KshanaError;
use sha2::{Digest, Sha256};

/// The `format` of a meter line.
pub const FORMAT: &str = "kshana-pro-meter/1";

/// The `prev` of the first line.
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// How a metered call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The call ran and returned a result.
    Ok,
    /// The licence did not allow it.
    Refused,
    /// It was allowed but failed (bad input, method error).
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Ok => "ok",
            Outcome::Refused => "refused",
            Outcome::Error => "error",
        }
    }
}

/// One line of the log.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterLine {
    pub format: String,
    pub seq: u64,
    pub unix: i64,
    /// The licence's token ID.
    pub licence: String,
    /// [`unit_hash`] of the unit.
    pub unit: String,
    pub method: String,
    /// The Pro kind or feature the call needed.
    pub feature: String,
    pub outcome: Outcome,
    pub input_sha256: String,
    pub prev: String,
}

/// The hash a unit is stored under: SHA-256 over a fixed prefix, the licence ID and the
/// unit ID as given.
pub fn unit_hash(licence_id: &str, unit_id: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kshana-unit-v1:");
    h.update(licence_id.as_bytes());
    h.update(b":");
    h.update(unit_id.trim().as_bytes());
    hex::encode(h.finalize())
}

fn sha_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn err(path: &Path, e: impl std::fmt::Display) -> KshanaError {
    KshanaError::InvalidInput(format!("meter log {}: {e}", path.display()))
}

/// Read and check a log: every line parses, `seq` counts from 0, every `prev` is the
/// SHA-256 of the line before. Returns the lines and the hash of the last one.
fn read_chain(path: &Path) -> Result<(Vec<MeterLine>, String), KshanaError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(err(path, e)),
    };
    let mut prev = GENESIS.to_string();
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let l: MeterLine =
            serde_json::from_str(raw).map_err(|e| err(path, format!("line {}: {e}", i + 1)))?;
        if l.format != FORMAT {
            return Err(err(path, format!("line {}: unknown format", i + 1)));
        }
        if l.seq != out.len() as u64 {
            return Err(err(path, format!("line {}: sequence broken", i + 1)));
        }
        if l.prev != prev {
            return Err(err(
                path,
                format!(
                    "line {}: chain broken (edited, reordered or deleted line)",
                    i + 1
                ),
            ));
        }
        prev = sha_hex(raw.as_bytes());
        out.push(l);
    }
    Ok((out, prev))
}

/// Check a log's chain; the number of lines.
pub fn verify(path: &Path) -> Result<usize, KshanaError> {
    read_chain(path).map(|(l, _)| l.len())
}

/// An open log, appended to by one process.
#[derive(Debug)]
pub struct MeterLog {
    path: PathBuf,
    /// The next sequence number and the hash of the last line.
    head: Mutex<(u64, String)>,
}

/// What a call records.
#[derive(Debug, Clone)]
pub struct Event<'a> {
    pub licence: &'a str,
    pub unit_hash: &'a str,
    pub method: &'a str,
    pub feature: &'a str,
    pub outcome: Outcome,
    pub input: &'a [u8],
}

impl MeterLog {
    /// Open (or start) a log. A log whose chain is broken is refused: start a new file and
    /// report the broken one to the vendor.
    pub fn open(path: impl Into<PathBuf>) -> Result<MeterLog, KshanaError> {
        let path = path.into();
        let (lines, prev) = read_chain(&path)?;
        Ok(MeterLog {
            head: Mutex::new((lines.len() as u64, prev)),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one line and flush it to disk before returning.
    pub fn record(&self, ev: &Event<'_>, now: i64) -> Result<MeterLine, KshanaError> {
        let mut head = self.head.lock().unwrap_or_else(|p| p.into_inner());
        let line = MeterLine {
            format: FORMAT.into(),
            seq: head.0,
            unix: now,
            licence: ev.licence.into(),
            unit: ev.unit_hash.into(),
            method: ev.method.into(),
            feature: ev.feature.into(),
            outcome: ev.outcome,
            input_sha256: sha_hex(ev.input),
            prev: head.1.clone(),
        };
        let text = serde_json::to_string(&line).map_err(|e| err(&self.path, e))?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| err(&self.path, e))?;
        f.write_all(format!("{text}\n").as_bytes())
            .and_then(|_| f.sync_data())
            .map_err(|e| err(&self.path, e))?;
        *head = (head.0 + 1, sha_hex(text.as_bytes()));
        Ok(line)
    }
}

/// The calls of one calendar month (UTC) in a summary.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct Month {
    /// `YYYY-MM`.
    pub month: String,
    /// Distinct units with at least one `ok` call (the per-unit count).
    pub active_units: usize,
    pub ok: u64,
    pub refused: u64,
    pub error: u64,
    /// `ok` calls per method.
    pub by_method: BTreeMap<String, u64>,
}

/// The per-unit report an OEM sends the vendor.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Summary {
    pub format: &'static str,
    pub lines: usize,
    pub licences: Vec<String>,
    /// SHA-256 of the last line: a later report's chain must pass through it.
    pub head: String,
    pub months: Vec<Month>,
}

fn month_of(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}")
}

/// Verify a log and count its calls per month and its active units.
pub fn summarise(path: &Path) -> Result<Summary, KshanaError> {
    let (lines, head) = read_chain(path)?;
    let mut months: BTreeMap<String, (Month, BTreeSet<String>)> = BTreeMap::new();
    let mut licences = BTreeSet::new();
    for l in &lines {
        licences.insert(l.licence.clone());
        let key = month_of(l.unix);
        let (m, units) = months.entry(key.clone()).or_insert_with(|| {
            (
                Month {
                    month: key,
                    ..Default::default()
                },
                BTreeSet::new(),
            )
        });
        match l.outcome {
            Outcome::Ok => {
                m.ok += 1;
                *m.by_method.entry(l.method.clone()).or_default() += 1;
                units.insert(l.unit.clone());
            }
            Outcome::Refused => m.refused += 1,
            Outcome::Error => m.error += 1,
        }
    }
    Ok(Summary {
        format: "kshana-pro-meter-summary/1",
        lines: lines.len(),
        licences: licences.into_iter().collect(),
        head,
        months: months
            .into_values()
            .map(|(mut m, u)| {
                m.active_units = u.len();
                m
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_from_unix() {
        assert_eq!(month_of(0), "1970-01");
        assert_eq!(month_of(1_800_000_000), "2027-01");
        assert_eq!(month_of(1_798_761_599), "2026-12");
    }

    #[test]
    fn unit_hash_is_bound_to_the_licence() {
        assert_ne!(unit_hash("kt_a", "SN-1"), unit_hash("kt_b", "SN-1"));
        assert_eq!(unit_hash("kt_a", " SN-1 "), unit_hash("kt_a", "SN-1"));
    }
}
