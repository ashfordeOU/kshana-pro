// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! OSNMA receiver conformance scorecard (W1119 C6).

use kshana_pro::osnma_conformance::{
    bars_sha256, score, stimulus_check, synth, Bars, ReceiverLog, Status, TestKind,
};

fn card(good: bool) -> kshana_pro::osnma_conformance::Scorecard {
    let log = synth::log(good);
    let bytes = serde_json::to_vec(&log).unwrap();
    score(&log, &bytes, &Bars::default()).unwrap()
}

fn verdict<'a>(
    c: &'a kshana_pro::osnma_conformance::Scorecard,
    id: &str,
) -> &'a kshana_pro::osnma_conformance::TestResult {
    c.tests.iter().find(|t| t.id == id).unwrap()
}

#[test]
fn the_open_engine_rejects_the_forged_stimulus() {
    let s = stimulus_check();
    assert!(s.tesla_genuine_key_verifies);
    assert!(s.tesla_forged_key_rejected);
    assert!(s.tesla_replayed_key_rejected);
    assert!(s.merkle_genuine_pk_verifies);
    assert!(s.merkle_forged_pk_rejected);
}

#[test]
fn a_well_behaved_receiver_meets_every_bar() {
    let c = card(true);
    assert_eq!(
        (c.met, c.not_met, c.not_assessed),
        (5, 0, 0),
        "{:#?}",
        c.tests
    );
    assert_eq!(verdict(&c, "ttfaf-cold").metrics["ttfaf_s"], 150.0);
    let f = verdict(&c, "forged-adkd0");
    assert_eq!(f.metrics["rejection_latency_s"], 40.0);
    assert_eq!(f.metrics["misleading_epochs_after_bar"], 0.0);
    assert_eq!(f.metrics["genuine_authenticated_fraction"], 1.0);
    assert_eq!(
        verdict(&c, "replay-delayed").metrics["rejection_latency_s"],
        30.0
    );
    assert_eq!(verdict(&c, "chain-renewal").metrics["outage_s"], 30.0);
    assert_eq!(
        verdict(&c, "pk-revocation").metrics["authenticated_after_revocation_s"],
        9.0
    );
}

#[test]
fn a_faulty_receiver_fails_each_bar_for_the_right_reason() {
    let c = card(false);
    assert_eq!(c.met, 0, "{:#?}", c.tests);
    assert_eq!(c.not_met, 5);
    assert_eq!(verdict(&c, "ttfaf-cold").metrics["ttfaf_s"], 420.0);
    let f = verdict(&c, "forged-adkd0");
    assert_eq!(f.metrics["rejection_latency_s"], -1.0, "never rejected");
    // Epochs strictly after the 360 s deadline: 361 to 899.
    assert_eq!(f.metrics["misleading_epochs_after_bar"], 539.0);
    assert!(verdict(&c, "chain-renewal").metrics["false_failure_epochs"] > 0.0);
    assert!(verdict(&c, "pk-revocation").metrics["authenticated_after_revocation_s"] > 400.0);
}

#[test]
fn tighter_bars_change_the_verdict_and_the_hash() {
    let log = synth::log(true);
    let b = Bars {
        reject_within_s: 20.0,
        ..Bars::default()
    };
    let c = score(&log, b"x", &b).unwrap();
    assert_eq!(verdict(&c, "forged-adkd0").verdict, "not met");
    assert_ne!(bars_sha256(&b), bars_sha256(&Bars::default()));
}

#[test]
fn a_late_forged_acceptance_after_rejection_is_misleading() {
    let mut log: ReceiverLog = synth::log(true);
    let t = log
        .tests
        .iter_mut()
        .find(|t| t.kind == TestKind::ForgedNav)
        .unwrap();
    // The receiver re-accepts E11 at 700 s.
    for r in t
        .records
        .iter_mut()
        .filter(|r| r.t_s >= 700.0 && r.t_s < 710.0)
    {
        for s in r.sats.iter_mut().filter(|s| s.sv == "E11") {
            s.status = Status::A;
            s.used = true;
        }
    }
    let c = score(&log, b"x", &Bars::default()).unwrap();
    let f = verdict(&c, "forged-adkd0");
    assert_eq!(f.verdict, "not met");
    assert_eq!(f.metrics["misleading_epochs_after_bar"], 10.0);
}

#[test]
fn wrong_format_is_refused_and_missing_tests_are_limits() {
    let mut log = synth::log(true);
    log.format = "other".into();
    assert!(score(&log, b"", &Bars::default()).is_err());
    let mut log = synth::log(true);
    log.tests.retain(|t| t.kind == TestKind::Ttfaf);
    let c = score(&log, b"", &Bars::default()).unwrap();
    assert!(c.limits.iter().any(|l| l.contains("ForgedNav")));
    let txt = serde_json::to_string(&c).unwrap();
    assert!(!txt.contains("certified") && txt.contains("not a certification"));
}

#[test]
fn signed_scorecard_verifies() {
    let log = synth::log(true);
    let bytes = serde_json::to_vec_pretty(&log).unwrap();
    let c = score(&log, &bytes, &Bars::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("kp-osnma-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    kshana_pro::osnma_conformance::write_dir(&dir, &c, &bytes).unwrap();
    let seed = [3u8; 32];
    kshana_pro::signed_dossier::sign(&dir, &seed, "Lab", "2026-11-19T00:00:00Z").unwrap();
    let v = kshana_pro::signed_dossier::verify(&dir, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed());
    let _ = std::fs::remove_dir_all(&dir);
}
