// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! UTM / U-space positioning integrity (`kshana_pro::utm`, docs/UTM-INTEGRITY.md): the SORA
//! template populated from the synthetic fixture, the USSP feed's advisory classes, the
//! Remote ID checks on a consistent and a cloned identity, the per-flight attestation of a
//! clean and a faulty flight, signing and verification with the Pro client, and the licence
//! gates. All inputs are synthetic.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{self, Entitlement, Tier};
use kshana_pro::utm::{self, synth};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");
const SEED: [u8; 32] = [7u8; 32];
const UTC: &str = "2026-11-20T12:00:00Z";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-utm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fixture(tag: &str) -> PathBuf {
    let d = tmp(tag);
    synth::write_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (bool, String) {
    let o = Command::new(BIN).args(args).output().unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr),
    )
}

fn produce(dir: &Path, cmd: &str, req: &str) -> PathBuf {
    let key = dir.join("key.hex");
    std::fs::write(&key, hex::encode(SEED)).unwrap();
    let out = dir.join(format!("out-{cmd}-{req}"));
    let (ok, text) = run(&[
        "utm",
        cmd,
        dir.join(req).to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "test",
        "--key-file",
        key.to_str().unwrap(),
        "--utc",
        UTC,
        "--dev",
    ]);
    assert!(ok, "{text}");
    out
}

fn json(p: PathBuf) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn the_sora_template_is_populated_from_the_fixture() {
    let d = fixture("sora");
    let l = utm::load(&d.join("sora.toml")).unwrap();
    let r = utm::sora(&l.req, &l.maps).unwrap();
    assert_eq!(r.maps.len(), 5);
    assert_eq!(r.days_affected, 2, "{:#?}", r.maps);
    let rate = r.day_affected_rate.unwrap();
    assert!((rate[0] - 0.4).abs() < 1e-9 && rate[1] < 0.4 && rate[2] > 0.4);
    // The degraded first cell holds part of the 6 km route; the coast through it at 10 m/s
    // with the multirotor terms is recomputed here from the closed form.
    assert!(r.worst_degraded_m > 100.0 && r.worst_degraded_m < 6000.0);
    let dr = utm::DeadReckoning::for_class("multirotor").unwrap();
    let t = r.worst_degraded_m / 10.0;
    assert!((r.worst_dr_error_m - dr.error_m(10.0, t)).abs() < 0.01);
    assert_eq!(
        r.interference_class,
        if r.containment_margin_m < 0.0 {
            "high"
        } else {
            "medium"
        }
    );
    let md = utm::sora_markdown(&r);
    assert!(!md.contains("{{"), "unfilled placeholder:\n{md}");
    assert!(md.contains("SURVEY-1") && md.contains("2026-06-04"));
    assert!(md.contains("Decision support only"));
    for w in ["compliant", "certified", "approved"] {
        assert!(!md.to_lowercase().contains(w), "{w}");
    }

    let out = produce(&d, "sora", "sora.toml");
    assert!(out.join("sora-input.md").exists());
    let v = json(out.join("sora.json"));
    assert_eq!(v["format"], "kshana-pro-utm-sora/1");
    assert_eq!(v["evidence"], "MODELLED");
}

#[test]
fn the_feed_classes_cells_by_their_days() {
    let d = fixture("feed");
    let l = utm::load(&d.join("feed.toml")).unwrap();
    let f = utm::feed(&l.req, &l.maps).unwrap();
    assert_eq!(f.days.len(), 5);
    let first = f.records.iter().find(|r| r.degraded_days > 0).unwrap();
    assert_eq!(first.degraded_days, 2);
    assert_eq!(first.days_observed, 5);
    assert_eq!(first.advisory, "recurring");
    assert_eq!(first.latest_state, "not_degraded");
    assert_eq!(first.valid_from, "2026-06-06T00:00:00Z");
    assert_eq!(first.valid_to, "2026-06-07T00:00:00Z");
    assert!(f
        .records
        .iter()
        .filter(|r| r.degraded_days == 0)
        .all(|r| r.advisory == "none"));
    let lines = utm::feed_jsonl(&f);
    assert_eq!(lines.lines().count(), f.records.len() + 1);
    let out = produce(&d, "feed", "feed.toml");
    assert!(out.join("feed.jsonl").exists());
}

#[test]
fn remote_id_separates_a_consistent_identity_from_a_clone() {
    let (reports, bad) = utm::parse_rid(&synth::rid_csv()).unwrap();
    assert_eq!(bad, 0);
    let r = utm::rid_check(&reports, bad, &utm::RidParams::default());
    let by = |id: &str| r.identities.iter().find(|i| i.uas_id == id).unwrap();
    assert_eq!(by("SURVEY-1").verdict, "CONSISTENT", "{:?}", by("SURVEY-1"));
    assert!((by("SURVEY-1").max_implied_speed_m_s - 10.0).abs() < 0.1);
    let c = by("CLONE-7");
    assert_eq!(c.verdict, "INCONSISTENT");
    assert!(c.clone_pairs >= 3, "{c:?}");
    // A reported speed far from the track's speed is a soft finding.
    let mut slow = reports.clone();
    for x in slow.iter_mut().filter(|x| x.uas_id == "SURVEY-1") {
        x.speed_m_s = Some(1.0);
    }
    let r = utm::rid_check(&slow, 0, &utm::RidParams::default());
    let s = r
        .identities
        .iter()
        .find(|i| i.uas_id == "SURVEY-1")
        .unwrap();
    assert_eq!(s.verdict, "SUSPECT");
    assert!(s.speed_mismatch > 50);
    assert!(utm::parse_rid("a,b\n1,2\n").is_err());
}

#[test]
fn attestations_of_a_clean_and_a_faulty_flight_verify_with_the_pro_client() {
    let d = fixture("attest");
    let clean = utm::attest(&utm::load(&d.join("attest-clean.toml")).unwrap()).unwrap();
    assert_eq!(clean.statement, "NOMINAL", "{:?}", clean.reasons);
    assert_eq!(clean.flight_utc[0], "2026-06-03T10:00:00Z");
    assert_eq!(clean.track_share_degraded, Some(0.0));
    let faulty = utm::attest(&utm::load(&d.join("attest-faulty.toml")).unwrap()).unwrap();
    assert_eq!(faulty.statement, "INTEGRITY-SUSPECT");
    assert_eq!(faulty.spoofing_epochs, 5);
    assert_eq!(faulty.position_jumps, 1);
    assert!((faulty.longest_gap_s - 20.0).abs() < 1e-9);
    assert!(faulty.reasons.iter().any(|r| r.contains("spoofing")));

    let out = produce(&d, "attest", "attest-faulty.toml");
    let v = json(out.join("attestation.json"));
    assert_eq!(v["statement"], "INTEGRITY-SUSPECT");
    assert_eq!(v["remote_id"]["identities"][0]["uas_id"], "SURVEY-1");
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, text) = run(&["dossier", "verify", out.to_str().unwrap(), "--trust", &pk]);
    assert!(ok, "{text}");
    let (ok, text) = run(&["utm", "check", out.to_str().unwrap(), "--trust", &pk]);
    assert!(ok, "{text}");
    // Tampering with the attestation breaks the signature.
    let f = out.join("attestation.json");
    let t = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, t.replace("INTEGRITY-SUSPECT", "NOMINAL")).unwrap();
    let (ok, _) = run(&["dossier", "verify", out.to_str().unwrap(), "--trust", &pk]);
    assert!(!ok);
}

#[test]
fn features_are_tiered_and_gated() {
    assert_eq!(license::explicit_tier(utm::FEATURE), Some(Tier::Pro));
    assert_eq!(
        license::explicit_tier(utm::FEATURE_FEED),
        Some(Tier::Enterprise)
    );
    assert!(license::PRO_COMMAND_FEATURES.contains(&utm::FEATURE));
    assert!(license::PRO_COMMAND_FEATURES.contains(&utm::FEATURE_FEED));
    let pro = Entitlement {
        licensee: "ACME".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&pro, utm::FEATURE, 0).is_ok());
    assert!(license::check(&pro, utm::FEATURE_FEED, 0).is_err());
    let d = fixture("gate");
    let (ok, text) = run(&["utm", "synth", d.join("s").to_str().unwrap()]);
    assert!(ok, "{text}");
    let (ok, _) = run(&["utm"]);
    assert!(!ok);
}
