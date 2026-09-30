// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! **Tamper-evident watch history** — an append-only, hash-chained `history.jsonl`,
//! one line per CI run of a pack.
//!
//! Follows the pattern of [`crate::study_ledger`]: each record carries the previous
//! record's hash, and its own hash is `sha256(prev_hash || content)`. It differs in one
//! deliberate way: the content that was hashed is stored **verbatim** on the line,
//!
//! ```text
//! {"hash":"<64 hex>","record":<the exact JSON that was hashed>}
//! ```
//!
//! so [`verify`] re-hashes the bytes as written instead of a re-serialisation of parsed
//! numbers (a parsed and re-printed double is not guaranteed to reproduce the same
//! text). Changing any byte of a record changes its hash; the next record's `prev_hash`
//! no longer matches; [`verify`] names the first record that fails and why.
//!
//! The library reads no clock: the time stamp is supplied by the caller (`--utc`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

/// The genesis `prev_hash`, shared with the study ledger.
pub const GENESIS_PREV_HASH: &str = crate::study_ledger::GENESIS_PREV_HASH;

/// One watched figure as it stood in one run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryPoint {
    /// The value, when the figure resolved.
    #[serde(default)]
    pub value: Option<f64>,
    /// Its unit.
    #[serde(default)]
    pub unit: Option<String>,
    /// The watch status in that run (`regressed`, `improved`, ...).
    pub status: String,
    /// The requirement verdict, when the watch has one.
    #[serde(default)]
    pub requirement: Option<String>,
    /// Requirement limits on the figure, in its unit, for the trend chart.
    #[serde(default)]
    pub limits: Vec<crate::pack_c::watch::Limit>,
    /// The evidence label of the entry's result (VALIDATED, MODELLED, PARTNER, MIXED).
    #[serde(default)]
    pub evidence: Option<String>,
}

/// The hashed content of one history record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryRecord {
    /// Position in the chain, from 0.
    pub index: u64,
    /// Caller-supplied time stamp of the run.
    #[serde(default)]
    pub time_utc: Option<String>,
    /// Engine version that produced the run.
    #[serde(default)]
    pub engine_version: Option<String>,
    /// Pack identity.
    pub pack_id: String,
    /// Pack version.
    pub pack_version: String,
    /// Pack content hash.
    pub pack_hash: String,
    /// SHA-256 of the run's metrics, as serialised in its `ci.json`.
    pub metrics_digest: String,
    /// `pass`, `fail` or `informational`.
    pub verdict: String,
    /// Watch identities that regressed or failed in this run.
    #[serde(default)]
    pub failures: Vec<String>,
    /// Every watched figure, by watch identity.
    #[serde(default)]
    pub figures: BTreeMap<String, HistoryPoint>,
    /// SHA-256 of each member result, by entry label.
    #[serde(default)]
    pub result_sha256: BTreeMap<String, String>,
    /// The previous record's hash (the genesis constant for the first).
    pub prev_hash: String,
}

/// A record together with its hash, as read back from a history file.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainedRecord {
    /// The record content.
    pub record: HistoryRecord,
    /// `sha256(prev_hash || content)`.
    pub hash: String,
}

/// Where and why a history file fails verification.
#[derive(Clone, Debug, PartialEq)]
pub struct Broken {
    /// The index of the first failing record (its line number is `index + 1`).
    pub index: usize,
    /// Why it fails.
    pub reason: String,
}

impl std::fmt::Display for Broken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "record {} (line {}) fails verification: {}",
            self.index,
            self.index + 1,
            self.reason
        )
    }
}

fn hash_of(prev_hash: &str, content: &str) -> String {
    let mut h = Sha256::new();
    h.update(prev_hash.as_bytes());
    h.update(content.as_bytes());
    hex::encode(h.finalize())
}

const PREFIX: &str = "{\"hash\":\"";
const MIDDLE: &str = "\",\"record\":";

/// Split one line into (hash, verbatim content).
fn split_line(line: &str) -> Result<(&str, &str), String> {
    let rest = line
        .strip_prefix(PREFIX)
        .ok_or("the line does not start with {\"hash\":\"")?;
    if rest.len() < 64 {
        return Err("the line is too short to hold a hash".into());
    }
    let (hash, rest) = rest.split_at(64);
    if !hash
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err("the hash is not 64 lowercase hexadecimal digits".into());
    }
    let rest = rest
        .strip_prefix(MIDDLE)
        .ok_or("the hash is not followed by ,\"record\":")?;
    let content = rest
        .strip_suffix('}')
        .ok_or("the line does not end with }")?;
    Ok((hash, content))
}

/// Verify a history file's text: every line parses, the indexes run 0, 1, 2, ..., each
/// `prev_hash` is the previous record's hash, and each hash matches the content as
/// written. Returns the records, or the first failure. An empty text is an empty,
/// valid history.
pub fn verify(text: &str) -> Result<Vec<ChainedRecord>, Broken> {
    let mut out = Vec::new();
    let mut expected_prev = GENESIS_PREV_HASH.to_string();
    for (i, line) in text.lines().enumerate() {
        let fail = |reason: String| Broken { index: i, reason };
        if line.trim().is_empty() {
            return Err(fail("an empty line inside the history".into()));
        }
        let (hash, content) = split_line(line).map_err(fail)?;
        let record: HistoryRecord = serde_json::from_str(content)
            .map_err(|e| fail(format!("the record does not parse: {e}")))?;
        if record.index != i as u64 {
            return Err(fail(format!("its index is {}, expected {i}", record.index)));
        }
        if record.prev_hash != expected_prev {
            return Err(fail(
                "its prev_hash is not the previous record's hash".into(),
            ));
        }
        let recomputed = hash_of(&record.prev_hash, content);
        if recomputed != hash {
            return Err(fail(
                "its hash does not match its content (the record was changed)".into(),
            ));
        }
        expected_prev = hash.to_string();
        out.push(ChainedRecord {
            record,
            hash: hash.to_string(),
        });
    }
    Ok(out)
}

/// Read and verify a history file. A missing file is an empty history.
pub fn load(path: &Path) -> Result<Vec<ChainedRecord>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("cannot read history {}: {e}", path.display())),
    };
    verify(&text).map_err(|b| format!("history {}: {b}", path.display()))
}

/// Chain `record` onto `existing` (its `index` and `prev_hash` are set here) and return
/// the line to append, without the trailing newline.
pub fn chain(existing: &[ChainedRecord], mut record: HistoryRecord) -> (ChainedRecord, String) {
    record.index = existing.len() as u64;
    record.prev_hash = existing
        .last()
        .map(|r| r.hash.clone())
        .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());
    let content = serde_json::to_string(&record).expect("history record serialises");
    let hash = hash_of(&record.prev_hash, &content);
    let line = format!("{PREFIX}{hash}{MIDDLE}{content}}}");
    (ChainedRecord { record, hash }, line)
}

/// Refuse a history whose records belong to another pack: one history file follows one
/// pack, so its trend never mixes two packs' figures into one series.
pub fn check_pack(existing: &[ChainedRecord], pack_id: &str) -> Result<(), String> {
    match existing.iter().find(|r| r.record.pack_id != pack_id) {
        Some(r) => Err(format!(
            "the history holds runs of pack '{}' (record {}), not of pack '{pack_id}'; \
             use one history file per pack",
            r.record.pack_id, r.record.index
        )),
        None => Ok(()),
    }
}

/// Verify the file at `path`, then append `record` to it. Refuses to append to a broken
/// history, or to the history of another pack. Returns the chained record.
pub fn append(path: &Path, record: HistoryRecord) -> Result<ChainedRecord, String> {
    let existing = load(path)?;
    check_pack(&existing, &record.pack_id)
        .map_err(|e| format!("history {}: {e}", path.display()))?;
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let (chained, line) = chain(&existing, record);
    let mut out = text;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&line);
    out.push('\n');
    std::fs::write(path, out)
        .map_err(|e| format!("cannot write history {}: {e}", path.display()))?;
    Ok(chained)
}

/// The history record of one CI report (its `index` and `prev_hash` are set by
/// [`chain`] / [`append`]).
pub fn record_from_report(report: &crate::pack_c::run::CiReport) -> HistoryRecord {
    let metrics_json = serde_json::to_string(&report.metrics).unwrap_or_default();
    let watched = report.watch.as_ref().filter(|w| w.watched);
    let verdict = match (watched, &report.verdict) {
        (Some(_), _) | (None, Some(_)) => {
            if report.passed() {
                "pass"
            } else {
                "fail"
            }
        }
        (None, None) => "informational",
    };
    let mut failures = Vec::new();
    let mut figures = BTreeMap::new();
    let mut result_sha256 = BTreeMap::new();
    if let Some(w) = &report.watch {
        for o in &w.outcomes {
            if o.failed() {
                failures.push(o.id.clone());
            }
            figures.insert(
                o.id.clone(),
                HistoryPoint {
                    value: o.value,
                    unit: o.unit.clone(),
                    status: o.status.clone(),
                    requirement: o.requirement.as_ref().map(|r| r.verdict.clone()),
                    limits: o.limits.clone(),
                    evidence: Some(o.evidence.clone()),
                },
            );
        }
        for m in &w.members {
            result_sha256.insert(m.label.clone(), m.result_sha256.clone());
        }
    }
    if let Some(v) = &report.verdict {
        failures.extend(v.regressions.iter().map(|r| r.key.clone()));
    }
    HistoryRecord {
        index: 0,
        time_utc: report.generated_utc.clone(),
        engine_version: report.engine_version.clone(),
        pack_id: report.pack_id.clone(),
        pack_version: report.pack_version.clone(),
        pack_hash: report.pack_hash.clone(),
        metrics_digest: hex::encode(Sha256::digest(metrics_json.as_bytes())),
        verdict: verdict.to_string(),
        failures,
        figures,
        result_sha256,
        prev_hash: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(verdict: &str, v: f64) -> HistoryRecord {
        let mut figures = BTreeMap::new();
        figures.insert(
            "w".to_string(),
            HistoryPoint {
                value: Some(v),
                unit: Some("ns".into()),
                status: "unchanged".into(),
                requirement: None,
                limits: vec![],
                evidence: Some("MODELLED".into()),
            },
        );
        HistoryRecord {
            index: 99,
            time_utc: Some("2026-09-30T00:00:00Z".into()),
            engine_version: Some("0.28.0".into()),
            pack_id: "p".into(),
            pack_version: "1".into(),
            pack_hash: "h".into(),
            metrics_digest: "d".into(),
            verdict: verdict.into(),
            failures: vec![],
            figures,
            result_sha256: BTreeMap::new(),
            prev_hash: "ignored".into(),
        }
    }

    fn build(n: usize) -> String {
        let mut recs: Vec<ChainedRecord> = Vec::new();
        let mut text = String::new();
        for i in 0..n {
            let (c, line) = chain(&recs, rec("pass", 0.1 + i as f64));
            recs.push(c);
            text.push_str(&line);
            text.push('\n');
        }
        text
    }

    #[test]
    fn a_chain_verifies_and_one_changed_byte_is_named() {
        let text = build(3);
        let recs = verify(&text).expect("untouched chain verifies");
        assert_eq!(recs.len(), 3);
        assert_eq!(recs[0].record.prev_hash, GENESIS_PREV_HASH);
        assert_eq!(recs[2].record.index, 2);
        // Change one digit of the second record's value.
        let lines: Vec<&str> = text.lines().collect();
        let tampered_line = lines[1].replacen("1.1", "1.2", 1);
        assert_ne!(tampered_line, lines[1]);
        let tampered = format!("{}\n{}\n{}\n", lines[0], tampered_line, lines[2]);
        let b = verify(&tampered).unwrap_err();
        assert_eq!(b.index, 1, "{b}");
        assert!(b.reason.contains("hash does not match"), "{b}");
    }

    #[test]
    fn empty_history_is_valid_and_garbage_is_named() {
        assert!(verify("").unwrap().is_empty());
        let b = verify("not json\n").unwrap_err();
        assert_eq!(b.index, 0);
        let text = build(2);
        let dropped: String = text.lines().skip(1).map(|l| format!("{l}\n")).collect();
        let b = verify(&dropped).unwrap_err();
        assert_eq!(b.index, 0);
        assert!(b.reason.contains("index"), "{b}");
    }
}
