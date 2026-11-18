// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Calibration register (stream B, C4): an append-only JSON-lines history of calibration
//! results with a SHA-256 hash chain.
//!
//! # File format
//!
//! One JSON object per line:
//!
//! ```json
//! {"body":{"seq":0,"subject_id":"RX-001","subject_kind":"receiver","date":"2026-11-18",
//!          "quantity":"receiver_delay_ns","value":251.3,"unit":"ns","expanded_u":3.2,"k":2.0,
//!          "operator":"A. Operator","report_sha256":"…","interval_days":365,"notes":""},
//!  "prev":"000…000","hash":"…"}
//! ```
//!
//! `hash = SHA-256(prev ‖ "\n" ‖ body)` (hex), where `body` is the **exact text** of the
//! `"body"` object as stored on the line (written as `serde_json`'s compact serialisation
//! of [`EntryBody`], fields in declaration order). Hashing the stored text rather than a
//! re-serialisation keeps the check independent of JSON number parsing. `prev` is the
//! previous entry's hash, 64 zeros for the first entry; lines keep the layout
//! `{"body":{…},"prev":"…","hash":"…"}` written by [`Register::append`]. Editing, inserting,
//! deleting or reordering any line breaks the chain from that line on, which
//! [`Register::verify`] reports. A hash chain is **tamper-evident, not tamper-proof**:
//! anyone able to rewrite the whole file can recompute every hash, so keep a copy of the
//! latest hash elsewhere (e.g. in the calibration report or a signed record) and compare
//! it with [`VerifyReport::head`].
//!
//! # Queries
//!
//! [`Register::latest`], [`Register::history`], [`Register::drift`] (least-squares
//! straight line of value against date, with the standard error of the slope) and
//! [`Register::due_date`] (latest date + calibration interval).

use crate::time::{civil_from_days, days_from_civil};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Hash of "no previous entry".
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// What was calibrated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Receiver,
    Simulator,
    Instrument,
    Other,
}

/// The hashed content of an entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryBody {
    /// Position in the register (0, 1, 2, …), filled by [`Register::append`].
    pub seq: u64,
    pub subject_id: String,
    pub subject_kind: SubjectKind,
    /// Calibration date, `YYYY-MM-DD`.
    pub date: String,
    /// Measurand, e.g. "receiver_delay_ns", "channel0_delay_ns".
    pub quantity: String,
    pub value: f64,
    pub unit: String,
    /// Expanded uncertainty and its coverage factor.
    pub expanded_u: f64,
    pub k: f64,
    pub operator: String,
    /// SHA-256 of the calibration report the result comes from.
    pub report_sha256: String,
    /// Calibration interval (days) for the due date.
    pub interval_days: Option<u32>,
    pub notes: String,
}

/// One register line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub body: EntryBody,
    pub prev: String,
    pub hash: String,
}

/// Hash of a body's stored JSON text chained to `prev`.
pub fn entry_hash(prev: &str, body_json: &str) -> String {
    let mut h = Sha256::new();
    h.update(prev.as_bytes());
    h.update(b"\n");
    h.update(body_json.as_bytes());
    hex::encode(h.finalize())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    prev: String,
    hash: String,
}

/// Split a stored line into (body text, parsed body, prev, hash).
fn split_line(line: &str) -> Result<(&str, EntryBody, String, String), String> {
    let rest = line
        .strip_prefix("{\"body\":")
        .ok_or("line does not start with {\"body\":")?;
    let i = rest.rfind(",\"prev\":").ok_or("no \"prev\" field")?;
    let body_text = &rest[..i];
    let body: EntryBody = serde_json::from_str(body_text).map_err(|e| format!("body: {e}"))?;
    let link: Link = serde_json::from_str(&format!("{{{}", &rest[i + 1..]))
        .map_err(|e| format!("link: {e}"))?;
    Ok((body_text, body, link.prev, link.hash))
}

/// Result of a verification.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifyReport {
    pub entries: usize,
    /// Hash of the last entry (GENESIS for an empty register).
    pub head: String,
    /// Problems found (empty = intact).
    pub problems: Vec<String>,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Straight-line drift fit `value = a + b·(days since first entry)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DriftFit {
    pub n: usize,
    pub first_date: String,
    /// Fitted value at the first date.
    pub intercept: f64,
    pub slope_per_day: f64,
    pub slope_per_year: f64,
    /// Standard error of the slope (per year); `None` for n < 3.
    pub slope_u_per_year: Option<f64>,
    /// RMS of the residuals.
    pub residual_rms: f64,
}

impl DriftFit {
    /// Predicted value at `date`.
    pub fn predict(&self, date: &str) -> Result<f64, String> {
        let d = day_number(date)? - day_number(&self.first_date)?;
        Ok(self.intercept + self.slope_per_day * d as f64)
    }
}

/// Day number (days from 1970-01-01) of `YYYY-MM-DD` (a longer ISO timestamp is cut to
/// its date).
pub fn day_number(date: &str) -> Result<i64, String> {
    let d = date.get(..10).ok_or_else(|| format!("bad date {date:?}"))?;
    let p: Vec<&str> = d.split('-').collect();
    if p.len() != 3 {
        return Err(format!("bad date {date:?}"));
    }
    let n = |s: &str| s.parse::<i64>().map_err(|_| format!("bad date {date:?}"));
    let (y, m, dd) = (n(p[0])?, n(p[1])?, n(p[2])?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&dd) {
        return Err(format!("bad date {date:?}"));
    }
    let z = days_from_civil(y, m, dd);
    if civil_from_days(z) != (y, m, dd) {
        return Err(format!("bad date {date:?}"));
    }
    Ok(z)
}

/// `YYYY-MM-DD` of a day number.
pub fn date_of(day: i64) -> String {
    let (y, m, d) = civil_from_days(day);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A register file.
pub struct Register {
    path: PathBuf,
}

impl Register {
    /// Open (the file is created on the first append).
    pub fn open(path: &Path) -> Register {
        Register {
            path: path.to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn text(&self) -> Result<String, String> {
        match std::fs::read_to_string(&self.path) {
            Ok(t) => Ok(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(format!("{}: {e}", self.path.display())),
        }
    }

    /// Check every line: parse, sequence numbers, chaining and hashes.
    pub fn verify(&self) -> Result<VerifyReport, String> {
        let text = self.text()?;
        let mut problems = Vec::new();
        let mut prev = GENESIS.to_string();
        let mut n = 0usize;
        for (i, line) in text.lines().enumerate() {
            let ln = i + 1;
            if line.trim().is_empty() {
                problems.push(format!("line {ln}: empty line"));
                continue;
            }
            let (text, body, e_prev, e_hash) = match split_line(line) {
                Ok(v) => v,
                Err(err) => {
                    problems.push(format!("line {ln}: not a register entry: {err}"));
                    continue;
                }
            };
            if body.seq != n as u64 {
                problems.push(format!(
                    "line {ln}: sequence {} where {n} expected",
                    body.seq
                ));
            }
            if e_prev != prev {
                problems.push(format!(
                    "line {ln}: previous-hash link broken (entry removed, inserted or reordered)"
                ));
            }
            if entry_hash(&e_prev, text) != e_hash {
                problems.push(format!("line {ln}: content does not match its hash (edited)"));
            }
            prev = e_hash;
            n += 1;
        }
        Ok(VerifyReport {
            entries: n,
            head: prev,
            problems,
        })
    }

    /// All entries (without verification).
    pub fn entries(&self) -> Result<Vec<Entry>, String> {
        self.text()?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| {
                split_line(l)
                    .map(|(_, body, prev, hash)| Entry { body, prev, hash })
                    .map_err(|e| format!("line {}: {e}", i + 1))
            })
            .collect()
    }

    /// Append an entry. The register is verified first and an append to a broken register is
    /// refused. `body.seq` is overwritten with the next sequence number.
    pub fn append(&self, mut body: EntryBody) -> Result<Entry, String> {
        day_number(&body.date)?;
        if !body.value.is_finite() || !body.expanded_u.is_finite() || body.expanded_u < 0.0 {
            return Err("value and expanded uncertainty must be finite, U ≥ 0".into());
        }
        let v = self.verify()?;
        if !v.ok() {
            return Err(format!(
                "register {} fails verification, append refused: {}",
                self.path.display(),
                v.problems.join("; ")
            ));
        }
        body.seq = v.entries as u64;
        let text = serde_json::to_string(&body).expect("serialisable");
        let hash = entry_hash(&v.head, &text);
        let line = format!(
            "{{\"body\":{text},\"prev\":\"{}\",\"hash\":\"{hash}\"}}",
            v.head
        );
        let e = Entry {
            body,
            prev: v.head,
            hash,
        };
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| format!("{}: {err}", self.path.display()))?;
        writeln!(f, "{line}")
            .and_then(|_| f.flush())
            .map_err(|err| format!("{}: {err}", self.path.display()))?;
        Ok(e)
    }

    /// Entries of a subject (and quantity, if given), in date order (stable for equal dates).
    pub fn history(&self, subject_id: &str, quantity: Option<&str>) -> Result<Vec<Entry>, String> {
        let mut v: Vec<Entry> = self
            .entries()?
            .into_iter()
            .filter(|e| {
                e.body.subject_id == subject_id && quantity.is_none_or(|q| e.body.quantity == q)
            })
            .collect();
        let mut keyed = Vec::with_capacity(v.len());
        for e in v.drain(..) {
            keyed.push((day_number(&e.body.date)?, e));
        }
        keyed.sort_by_key(|(d, _)| *d);
        Ok(keyed.into_iter().map(|(_, e)| e).collect())
    }

    /// Latest entry of a subject (by date, then by register order).
    pub fn latest(&self, subject_id: &str, quantity: Option<&str>) -> Result<Option<Entry>, String> {
        Ok(self.history(subject_id, quantity)?.pop())
    }

    /// Least-squares drift of a subject's quantity. Needs at least two entries on different
    /// dates.
    pub fn drift(&self, subject_id: &str, quantity: &str) -> Result<DriftFit, String> {
        let h = self.history(subject_id, Some(quantity))?;
        if h.len() < 2 {
            return Err("drift needs at least two entries".into());
        }
        let d0 = day_number(&h[0].body.date)?;
        let mut x = Vec::new();
        for e in &h {
            x.push((day_number(&e.body.date)? - d0) as f64);
        }
        let y: Vec<f64> = h.iter().map(|e| e.body.value).collect();
        let n = x.len() as f64;
        let mx = x.iter().sum::<f64>() / n;
        let my = y.iter().sum::<f64>() / n;
        let sxx: f64 = x.iter().map(|v| (v - mx).powi(2)).sum();
        if sxx == 0.0 {
            return Err("drift needs entries on at least two different dates".into());
        }
        let sxy: f64 = x.iter().zip(&y).map(|(a, b)| (a - mx) * (b - my)).sum();
        let b = sxy / sxx;
        let a = my - b * mx;
        let ss: f64 = x.iter().zip(&y).map(|(a_, y_)| (y_ - a - b * a_).powi(2)).sum();
        let slope_u = (h.len() > 2).then(|| (ss / (n - 2.0) / sxx).sqrt() * 365.25);
        Ok(DriftFit {
            n: h.len(),
            first_date: h[0].body.date[..10].to_string(),
            intercept: a,
            slope_per_day: b,
            slope_per_year: b * 365.25,
            slope_u_per_year: slope_u,
            residual_rms: (ss / n).sqrt(),
        })
    }

    /// Next due date: latest date + the latest entry's interval (or `default_interval_days`).
    pub fn due_date(
        &self,
        subject_id: &str,
        quantity: Option<&str>,
        default_interval_days: Option<u32>,
    ) -> Result<Option<String>, String> {
        let Some(e) = self.latest(subject_id, quantity)? else {
            return Ok(None);
        };
        let Some(iv) = e.body.interval_days.or(default_interval_days) else {
            return Ok(None);
        };
        Ok(Some(date_of(day_number(&e.body.date)? + iv as i64)))
    }

    /// Whether the subject is due on `today` (true when never calibrated).
    pub fn is_due(
        &self,
        subject_id: &str,
        quantity: Option<&str>,
        default_interval_days: Option<u32>,
        today: &str,
    ) -> Result<bool, String> {
        Ok(match self.due_date(subject_id, quantity, default_interval_days)? {
            None => self.latest(subject_id, quantity)?.is_none(),
            Some(d) => day_number(today)? >= day_number(&d)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kshana-calreg-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("register.jsonl");
        let _ = std::fs::remove_file(&p);
        p
    }

    fn body(subject: &str, date: &str, value: f64) -> EntryBody {
        EntryBody {
            seq: 0,
            subject_id: subject.into(),
            subject_kind: SubjectKind::Receiver,
            date: date.into(),
            quantity: "receiver_delay_ns".into(),
            value,
            unit: "ns".into(),
            expanded_u: 3.0,
            k: 2.0,
            operator: "op".into(),
            report_sha256: "ab".repeat(32),
            interval_days: Some(365),
            notes: String::new(),
        }
    }

    #[test]
    fn append_query_drift_and_due_date() {
        let p = tmp("q");
        let r = Register::open(&p);
        assert!(r.verify().unwrap().ok());
        assert_eq!(r.verify().unwrap().head, GENESIS);
        // 2 ns/year drift, entries appended out of date order
        r.append(body("RX1", "2024-01-01", 100.0)).unwrap();
        r.append(body("RX1", "2026-01-01", 100.0 + 2.0 * 731.0 / 365.25))
            .unwrap();
        r.append(body("RX2", "2025-06-01", 7.0)).unwrap();
        let e = r
            .append(body("RX1", "2025-01-01", 100.0 + 2.0 * 366.0 / 365.25))
            .unwrap();
        assert_eq!(e.body.seq, 3);
        let v = r.verify().unwrap();
        assert!(v.ok(), "{:?}", v.problems);
        assert_eq!((v.entries, v.head.clone()), (4, e.hash.clone()));
        let l = r.latest("RX1", None).unwrap().unwrap();
        assert_eq!(l.body.date, "2026-01-01");
        assert_eq!(r.history("RX1", Some("receiver_delay_ns")).unwrap().len(), 3);
        let d = r.drift("RX1", "receiver_delay_ns").unwrap();
        assert!((d.slope_per_year - 2.0).abs() < 1e-9, "{d:?}");
        assert!(d.residual_rms < 1e-9);
        assert!((d.predict("2027-01-01").unwrap() - (100.0 + 2.0 * 1096.0 / 365.25)).abs() < 1e-9);
        assert_eq!(
            r.due_date("RX1", None, None).unwrap().as_deref(),
            Some("2027-01-01")
        );
        assert!(!r.is_due("RX1", None, None, "2026-12-31").unwrap());
        assert!(r.is_due("RX1", None, None, "2027-01-01").unwrap());
        assert!(r.is_due("RX9", None, None, "2027-01-01").unwrap());
        assert!(r.drift("RX2", "receiver_delay_ns").is_err());
        assert!(r.append(body("RX1", "2026-02-30", 1.0)).is_err());
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[test]
    fn verify_detects_edit_deletion_reorder_and_extra_fields() {
        let p = tmp("t");
        let r = Register::open(&p);
        for (i, d) in ["2025-01-01", "2025-02-01", "2025-03-01"].iter().enumerate() {
            r.append(body("RX1", d, i as f64)).unwrap();
        }
        let good = std::fs::read_to_string(&p).unwrap();
        let lines: Vec<&str> = good.lines().collect();

        // edited value
        std::fs::write(&p, good.replace("\"value\":1.0", "\"value\":1.5")).unwrap();
        let v = r.verify().unwrap();
        assert!(v.problems.iter().any(|m| m.contains("line 2") && m.contains("edited")), "{v:?}");
        // an append to a tampered register is refused
        assert!(r.append(body("RX1", "2025-04-01", 3.0)).is_err());

        // deleted middle entry
        std::fs::write(&p, format!("{}\n{}\n", lines[0], lines[2])).unwrap();
        let v = r.verify().unwrap();
        assert!(v.problems.iter().any(|m| m.contains("line 2") && m.contains("link")), "{v:?}");

        // reordered
        std::fs::write(&p, format!("{}\n{}\n{}\n", lines[0], lines[2], lines[1])).unwrap();
        assert!(!r.verify().unwrap().ok());

        // extra field smuggled into a body
        std::fs::write(&p, good.replacen("\"notes\":\"\"", "\"notes\":\"\",\"x\":1", 1)).unwrap();
        assert!(!r.verify().unwrap().ok());

        // truncated tail is undetectable by the chain alone: the head hash changes
        std::fs::write(&p, format!("{}\n{}\n", lines[0], lines[1])).unwrap();
        let v = r.verify().unwrap();
        assert!(v.ok());
        let full_head = split_line(lines[2]).unwrap().3;
        assert_ne!(v.head, full_head);

        std::fs::write(&p, &good).unwrap();
        assert!(r.verify().unwrap().ok());
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }
}
