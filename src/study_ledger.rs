// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Tamper-evident study ledger (P7)
//!
//! An append-only ledger of study-lifecycle records. Each [`StudyRecord`] is hash-
//! chained to the one before it: `record_hash = sha256(prev_hash || canonical(record
//! without its own record_hash))`. Any later mutation of an earlier record changes
//! that record's hash, which no longer matches the `prev_hash` baked into the next
//! record, so [`StudyLedger::verify`] returns `false`. This mirrors the existing
//! evidence-fingerprint discipline in [`crate::report`] / the HMAC license token, but
//! across a *sequence* of records rather than a single artifact.
//!
//! **Determinism:** the library never reads a clock. Every timestamp
//! (`delivered_utc`) is caller-supplied and `Option` (`None` by default), so the
//! same inputs always produce the same hashes — the ledger is reproducible and the
//! hashes are stable across machines and runs.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The genesis `prev_hash`: the fixed baseline the first record chains to. 64 hex
/// zeros (one SHA-256's worth), so the chain has a well-defined, constant root.
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// One record in the study lifecycle: a study was requested, (optionally) run, and
/// (optionally) delivered. `prev_hash`/`record_hash` are filled in by
/// [`StudyLedger::append`]; callers leave them empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyRecord {
    /// The deterministic study id (see [`crate::serve::study_intake::study_id`]).
    pub study_id: String,
    /// Hash of the canonical study request that opened this study.
    pub request_hash: String,
    /// Hash of the run output, once the study has been executed (None until then).
    #[serde(default)]
    pub run_hash: Option<String>,
    /// Caller-supplied delivery timestamp (UTC), once delivered (None until then).
    /// The library never reads a clock — see the module docs on determinism.
    #[serde(default)]
    pub delivered_utc: Option<String>,
    /// The previous record's `record_hash` (or [`GENESIS_PREV_HASH`] for the first).
    #[serde(default)]
    pub prev_hash: String,
    /// `sha256(prev_hash || canonical(this record without record_hash))`.
    #[serde(default)]
    pub record_hash: String,
}

/// The record's content as hashed — everything *except* `record_hash`, in a fixed,
/// canonical field order. Serialised deterministically so the hash is stable.
#[derive(Serialize)]
struct RecordContent<'a> {
    study_id: &'a str,
    request_hash: &'a str,
    run_hash: &'a Option<String>,
    delivered_utc: &'a Option<String>,
    prev_hash: &'a str,
}

impl StudyRecord {
    /// Compute this record's hash against `prev_hash`: `sha256(prev_hash ||
    /// canonical(record-without-record_hash))`. Pure and deterministic.
    fn compute_hash(&self, prev_hash: &str) -> String {
        let content = RecordContent {
            study_id: &self.study_id,
            request_hash: &self.request_hash,
            run_hash: &self.run_hash,
            delivered_utc: &self.delivered_utc,
            prev_hash,
        };
        // serde_json with a fixed struct + field order is our canonical form.
        let canonical = serde_json::to_string(&content).expect("record content serializes");
        let mut hasher = Sha256::new();
        hasher.update(prev_hash.as_bytes());
        hasher.update(canonical.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// An append-only, hash-chained ledger of [`StudyRecord`]s.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StudyLedger {
    records: Vec<StudyRecord>,
}

impl StudyLedger {
    /// A fresh, empty ledger.
    pub fn new() -> Self {
        StudyLedger {
            records: Vec::new(),
        }
    }

    /// The records in append order (read-only).
    pub fn records(&self) -> &[StudyRecord] {
        &self.records
    }

    /// Mutable access to the records — for tests and tamper simulation. Mutating a
    /// record without recomputing the chain will (correctly) fail [`verify`].
    ///
    /// [`verify`]: Self::verify
    pub fn records_mut(&mut self) -> &mut Vec<StudyRecord> {
        &mut self.records
    }

    /// Append `record`, chaining it to the previous record's hash (or
    /// [`GENESIS_PREV_HASH`] for the first). The caller's `prev_hash`/`record_hash`
    /// fields are ignored and recomputed, so the chain is always well-formed.
    pub fn append(&mut self, mut record: StudyRecord) {
        let prev_hash = self
            .records
            .last()
            .map(|r| r.record_hash.clone())
            .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());
        record.prev_hash = prev_hash.clone();
        record.record_hash = record.compute_hash(&prev_hash);
        self.records.push(record);
    }

    /// Recompute the whole chain and return `false` if any record was tampered:
    /// a wrong `prev_hash` link or a `record_hash` that no longer matches the
    /// record's content.
    pub fn verify(&self) -> bool {
        let mut expected_prev = GENESIS_PREV_HASH.to_string();
        for rec in &self.records {
            if rec.prev_hash != expected_prev {
                return false;
            }
            if rec.record_hash != rec.compute_hash(&rec.prev_hash) {
                return false;
            }
            expected_prev = rec.record_hash.clone();
        }
        true
    }

    /// Serialise the ledger to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("ledger serializes")
    }

    /// Parse a ledger from JSON. Does not call [`verify`] — the caller decides when
    /// to check integrity.
    ///
    /// [`verify`]: Self::verify
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid study ledger JSON: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(study_id: &str, request_hash: &str) -> StudyRecord {
        StudyRecord {
            study_id: study_id.into(),
            request_hash: request_hash.into(),
            run_hash: None,
            delivered_utc: None,
            prev_hash: String::new(),
            record_hash: String::new(),
        }
    }

    #[test]
    fn genesis_record_chains_to_the_constant() {
        let mut l = StudyLedger::new();
        l.append(rec("s1", "h1"));
        assert_eq!(l.records()[0].prev_hash, GENESIS_PREV_HASH);
        assert_eq!(GENESIS_PREV_HASH.len(), 64);
    }

    #[test]
    fn append_recomputes_caller_supplied_chain_fields() {
        // A caller that pre-fills bogus chain fields must not corrupt the chain.
        let mut l = StudyLedger::new();
        let mut bogus = rec("s1", "h1");
        bogus.prev_hash = "deadbeef".into();
        bogus.record_hash = "cafebabe".into();
        l.append(bogus);
        assert_eq!(l.records()[0].prev_hash, GENESIS_PREV_HASH);
        assert!(l.verify());
    }

    #[test]
    fn untampered_chain_verifies_and_tamper_is_caught() {
        let mut l = StudyLedger::new();
        l.append(rec("s1", "h1"));
        l.append(rec("s2", "h2"));
        assert!(l.verify());
        let mut t = l.clone();
        t.records_mut()[0].study_id = "evil".into();
        assert!(!t.verify());
    }
}
