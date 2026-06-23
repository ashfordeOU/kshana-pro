// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! Tests for the tamper-evident study ledger (P7): a stable per-record hash, a
//! chain that `verify()`s true untampered, and detection of any mutation to an
//! earlier record. Hashing is deterministic — no clock is read in the library.

use kshana_pro::study_ledger::{StudyLedger, StudyRecord, GENESIS_PREV_HASH};

fn record(study_id: &str, request_hash: &str) -> StudyRecord {
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
fn appending_a_record_yields_a_stable_record_hash() {
    let mut a = StudyLedger::new();
    a.append(record("s1", "abc"));
    let mut b = StudyLedger::new();
    b.append(record("s1", "abc"));
    assert_eq!(
        a.records()[0].record_hash,
        b.records()[0].record_hash,
        "the same first record must hash identically"
    );
    assert_eq!(
        a.records()[0].prev_hash,
        GENESIS_PREV_HASH,
        "genesis record chains to the fixed constant"
    );
    assert!(!a.records()[0].record_hash.is_empty());
}

#[test]
fn chain_links_each_record_to_the_previous() {
    let mut l = StudyLedger::new();
    l.append(record("s1", "abc"));
    l.append(record("s2", "def"));
    assert_eq!(
        l.records()[1].prev_hash,
        l.records()[0].record_hash,
        "second record's prev_hash must be the first's record_hash"
    );
}

#[test]
fn verify_is_true_for_an_untampered_chain() {
    let mut l = StudyLedger::new();
    l.append(record("s1", "abc"));
    l.append(record("s2", "def"));
    l.append(record("s3", "ghi"));
    assert!(l.verify(), "an untampered chain must verify");
}

#[test]
fn mutating_an_earlier_record_field_breaks_verify() {
    let mut l = StudyLedger::new();
    l.append(record("s1", "abc"));
    l.append(record("s2", "def"));
    assert!(l.verify());
    // Tamper with the first record's request_hash without recomputing the chain.
    let mut tampered = l.clone();
    tampered.records_mut()[0].request_hash = "TAMPERED".into();
    assert!(
        !tampered.verify(),
        "mutating an earlier record must make verify() false"
    );
}

#[test]
fn mutating_a_record_hash_breaks_verify() {
    let mut l = StudyLedger::new();
    l.append(record("s1", "abc"));
    l.append(record("s2", "def"));
    let mut tampered = l.clone();
    tampered.records_mut()[1].record_hash = "0".repeat(64);
    assert!(!tampered.verify());
}

#[test]
fn json_round_trips_and_still_verifies() {
    let mut l = StudyLedger::new();
    l.append(record("s1", "abc"));
    l.append(record("s2", "def"));
    let json = l.to_json();
    let back = StudyLedger::from_json(&json).expect("ledger deserializes");
    assert!(back.verify(), "a round-tripped ledger must still verify");
    assert_eq!(back.records().len(), 2);
}

#[test]
fn caller_supplied_delivered_utc_is_part_of_the_hash() {
    // Determinism: the library never reads a clock; timestamps are caller-supplied.
    let mut a = StudyLedger::new();
    let mut ra = record("s1", "abc");
    ra.delivered_utc = Some("2026-06-23T00:00:00Z".into());
    a.append(ra);

    let mut b = StudyLedger::new();
    let mut rb = record("s1", "abc");
    rb.delivered_utc = Some("2026-06-24T00:00:00Z".into());
    b.append(rb);

    assert_ne!(
        a.records()[0].record_hash,
        b.records()[0].record_hash,
        "a different delivered_utc must change the record hash"
    );
}
