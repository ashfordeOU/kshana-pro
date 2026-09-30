// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The delivery ledger
//!
//! Every succeeded job appends three hash-chained records to `<data-dir>/ledger.json`,
//! using the overlay's tamper-evident study ledger ([`crate::study_ledger`]):
//!
//! | Record | `study_id` | `request_hash` | `run_hash` | `delivered_utc` |
//! |---|---|---|---|---|
//! | request | job identifier | SHA-256 of `request.json` | none | none |
//! | run | job identifier | same | SHA-256 of the delivery manifest | none |
//! | delivered | job identifier | same | same | server clock at delivery |
//!
//! The *delivery manifest* is the text `"<sha256>  result.json\n"` followed by one
//! `"<sha256>  artefacts/<name>\n"` line per artefact in name order (the layout of the
//! `sha256sum` tool), so the run hash covers the result and every artefact.
//!
//! A failed job is never written to the ledger. The append is idempotent per job: a
//! job whose delivered record is already present is not appended again, so a worker
//! that is stopped between the ledger write and the status write, and re-runs the job
//! after a restart, does not duplicate it. Each append rewrites the file atomically.
//!
//! [`DeliveryLedger::verify`] checks the chain and names the first record that no
//! longer matches; it also recomputes, for every delivered record, the request hash
//! and the delivery-manifest hash from the files in the job store, so a result or an
//! artefact changed on disk after delivery is reported too.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

use super::jobs::{sha256_hex, write_atomic, ArtefactEntry, JobStore};
use crate::study_ledger::{StudyLedger, StudyRecord};

/// The delivery-manifest text for a result hash and its artefacts.
pub fn delivery_manifest(result_sha256: &str, artefacts: &[ArtefactEntry]) -> String {
    let mut sorted: Vec<&ArtefactEntry> = artefacts.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut text = format!("{result_sha256}  result.json\n");
    for a in sorted {
        text.push_str(&format!("{}  artefacts/{}\n", a.sha256, a.name));
    }
    text
}

/// A stored file that no longer matches its delivered record.
#[derive(Debug, Clone, Serialize)]
pub struct StoreMismatch {
    pub job_id: String,
    pub record: usize,
    pub what: String,
}

/// The verification report served at `GET /ledger/verify`.
#[derive(Debug, Clone, Serialize)]
pub struct VerifyReport {
    /// True only when the chain is intact AND every delivered job's stored files match.
    pub verified: bool,
    /// True when every record's links and hashes recompute.
    pub chain_intact: bool,
    pub records: usize,
    pub delivered_jobs: usize,
    /// Index (from 0) of the first record whose hash or link no longer recomputes.
    pub first_bad_record: Option<usize>,
    /// The `study_id` (job identifier) that record carries as stored.
    pub first_bad_job_id: Option<String>,
    /// Why verification failed, in words.
    pub reason: Option<String>,
    /// Delivered jobs whose files were recomputed against their records.
    pub store_checked: usize,
    pub store_mismatches: Vec<StoreMismatch>,
}

/// The ledger file and the lock that serialises appends.
pub struct DeliveryLedger {
    path: PathBuf,
    lock: Mutex<()>,
}

impl DeliveryLedger {
    /// The ledger at `<data_dir>/ledger.json` (created on the first append).
    pub fn new(data_dir: &Path) -> DeliveryLedger {
        DeliveryLedger {
            path: data_dir.join("ledger.json"),
            lock: Mutex::new(()),
        }
    }

    /// The ledger file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load(&self) -> io::Result<StudyLedger> {
        match fs::read_to_string(&self.path) {
            Ok(s) => StudyLedger::from_json(&s).map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(StudyLedger::new()),
            Err(e) => Err(e),
        }
    }

    /// The raw ledger document (an empty ledger when none has been written yet).
    pub fn read_json(&self) -> io::Result<String> {
        match fs::read_to_string(&self.path) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(StudyLedger::new().to_json()),
            Err(e) => Err(e),
        }
    }

    /// Append the request, run and delivered records of one succeeded job, unless its
    /// delivered record is already present. Returns whether records were appended.
    pub fn append_delivery(
        &self,
        job_id: &str,
        request_hash: &str,
        run_hash: &str,
        delivered_utc: &str,
    ) -> io::Result<bool> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut ledger = self.load()?;
        if !ledger.verify() {
            // Never extend a chain that no longer verifies: that would bury the break.
            return Err(io::Error::other(
                "the delivery ledger no longer verifies; not appending (see GET /ledger/verify)",
            ));
        }
        if ledger
            .records()
            .iter()
            .any(|r| r.study_id == job_id && r.delivered_utc.is_some())
        {
            return Ok(false);
        }
        let rec = |run: Option<&str>, delivered: Option<&str>| StudyRecord {
            study_id: job_id.to_string(),
            request_hash: request_hash.to_string(),
            run_hash: run.map(str::to_string),
            delivered_utc: delivered.map(str::to_string),
            prev_hash: String::new(),
            record_hash: String::new(),
        };
        ledger.append(rec(None, None));
        ledger.append(rec(Some(run_hash), None));
        ledger.append(rec(Some(run_hash), Some(delivered_utc)));
        write_atomic(&self.path, ledger.to_json().as_bytes())?;
        Ok(true)
    }

    /// Verify the chain, then recompute every delivered job's hashes from the store.
    pub fn verify(&self, store: &JobStore) -> VerifyReport {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut report = VerifyReport {
            verified: false,
            chain_intact: false,
            records: 0,
            delivered_jobs: 0,
            first_bad_record: None,
            first_bad_job_id: None,
            reason: None,
            store_checked: 0,
            store_mismatches: Vec::new(),
        };
        let text = match fs::read_to_string(&self.path) {
            Ok(s) => s,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                report.verified = true;
                report.chain_intact = true;
                return report;
            }
            Err(e) => {
                report.reason = Some(format!("the ledger file cannot be read: {e}"));
                return report;
            }
        };
        let ledger = match StudyLedger::from_json(&text) {
            Ok(l) => l,
            Err(e) => {
                report.reason = Some(format!("the ledger file is not a valid ledger: {e}"));
                return report;
            }
        };
        let records = ledger.records();
        report.records = records.len();
        report.delivered_jobs = records.iter().filter(|r| r.delivered_utc.is_some()).count();

        if ledger.verify() {
            report.chain_intact = true;
        } else {
            // A prefix verifies up to the first broken record and never after it, so a
            // binary search over prefix lengths finds that record.
            let prefix_ok = |n: usize| {
                let mut p = ledger.clone();
                p.records_mut().truncate(n);
                p.verify()
            };
            let (mut lo, mut hi) = (0usize, records.len()); // prefix_ok(lo), !prefix_ok(hi)
            while hi - lo > 1 {
                let mid = (lo + hi) / 2;
                if prefix_ok(mid) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let bad = hi - 1;
            report.first_bad_record = Some(bad);
            report.first_bad_job_id = Some(records[bad].study_id.clone());
            report.reason = Some(format!(
                "record {bad} (job {}) no longer matches its hash or its link to the record before it",
                records[bad].study_id
            ));
            return report;
        }

        for (i, rec) in records.iter().enumerate() {
            if rec.delivered_utc.is_none() {
                continue;
            }
            report.store_checked += 1;
            let mismatch = |what: String| StoreMismatch {
                job_id: rec.study_id.clone(),
                record: i,
                what,
            };
            if !super::jobs::is_job_id(&rec.study_id) {
                report
                    .store_mismatches
                    .push(mismatch("the record does not name a job".into()));
                continue;
            }
            match store.request_bytes(&rec.study_id) {
                Ok(bytes) if sha256_hex(&bytes) == rec.request_hash => {}
                Ok(_) => report
                    .store_mismatches
                    .push(mismatch("request.json changed after delivery".into())),
                Err(e) => report
                    .store_mismatches
                    .push(mismatch(format!("request.json unreadable: {e}"))),
            }
            let run_hash = store_run_hash(store, &rec.study_id);
            match run_hash {
                Ok(h) if Some(&h) == rec.run_hash.as_ref() => {}
                Ok(_) => report.store_mismatches.push(mismatch(
                    "the result or an artefact changed after delivery".into(),
                )),
                Err(e) => report
                    .store_mismatches
                    .push(mismatch(format!("stored outputs unreadable: {e}"))),
            }
        }
        report.verified = report.store_mismatches.is_empty();
        if !report.verified {
            report.reason = Some(format!(
                "{} stored file(s) no longer match the ledger",
                report.store_mismatches.len()
            ));
        }
        report
    }
}

/// Recompute a stored job's delivery-manifest hash from the files on disk (the artefact
/// names come from its status; each artefact's bytes are re-read and re-hashed).
fn store_run_hash(store: &JobStore, id: &str) -> io::Result<String> {
    let status = store
        .status(id)?
        .ok_or_else(|| io::Error::other("no such job in the store"))?;
    let result = store.read_result(id)?;
    let mut entries = Vec::new();
    for a in &status.artefacts {
        let body = store
            .read_artefact(id, &a.name)?
            .ok_or_else(|| io::Error::other(format!("artefact {} missing", a.name)))?;
        entries.push(ArtefactEntry {
            name: a.name.clone(),
            bytes: body.len() as u64,
            sha256: sha256_hex(body.as_bytes()),
        });
    }
    Ok(sha256_hex(
        delivery_manifest(&sha256_hex(result.as_bytes()), &entries).as_bytes(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_orders_artefacts_by_name() {
        let a = |n: &str| ArtefactEntry {
            name: n.into(),
            bytes: 1,
            sha256: format!("{n}-h"),
        };
        let m = delivery_manifest("r", &[a("z.svg"), a("a.csv")]);
        assert_eq!(
            m,
            "r  result.json\na.csv-h  artefacts/a.csv\nz.svg-h  artefacts/z.svg\n"
        );
    }
}
