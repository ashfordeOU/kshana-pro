// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! OEM SDK and programme licences (backlog E39 B3901, B3902; docs/KS-INSIDE.md,
//! docs/PROGRAMME-LICENCES.md): the KS-Inside facade and C ABI with per-call metering, the
//! meter log's chain and per-unit report, and the feeder, host, teaching and bench licence
//! kinds with their gates. Negative cases: wrong machine, wrong key, expired, tampered,
//! wrong kind, stale feed, refused grants. Keys are throwaway test seeds (the debug-only
//! demo key for the C ABI, as tests/pack_c_subscription.rs does).

use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use kshana_pro::ks_inside::{self, ffi, CallError, Config, KsInside, METHODS};
use kshana_pro::license::meter::{self, Outcome};
use kshana_pro::license::programme::{
    self, bench_entitlement, bench_key_id, feed_entitlement, issue_receipt, BenchBinder,
    BindingClass, Kind, Programme, SoftwareBinder, FEED_GRACE_SECS,
};
use kshana_pro::license::{
    explicit_tier, issue, issue_v2, machine, public_key_for, revocation, verify_with, Keyring,
    LicenseClaims, Product, Tier, VerifyOptions, KEY_LEN, PRO_COMMAND_FEATURES,
};

const SEED: [u8; KEY_LEN] = [39; KEY_LEN];
const OTHER: [u8; KEY_LEN] = [40; KEY_LEN];
const DAY: i64 = 86_400;
const T0: i64 = 1_800_000_000;

fn tmp(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let d = std::env::temp_dir().join(format!(
        "kshana-pro-oem-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ring() -> Keyring {
    Keyring::single(public_key_for(&SEED))
}

fn at(now: i64) -> VerifyOptions<'static> {
    VerifyOptions::at(now)
}

fn base(tier: &str, products: &[&str], days: i64) -> LicenseClaims {
    LicenseClaims {
        licensee: "Test Holder".into(),
        tier: tier.into(),
        expires_unix: if days > 0 { T0 + days * DAY } else { 0 },
        issued_unix: T0,
        products: products.iter().map(|p| p.to_string()).collect(),
        ..Default::default()
    }
}

fn feeder_claims(stations: &[&str]) -> LicenseClaims {
    let mut c = base("pro", &["cli", "station"], 365);
    c.programme = Some(Programme {
        kind: "feeder".into(),
        stations: stations.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    });
    c
}

fn tamper(token: &str) -> String {
    let (p, s) = token.split_once('.').unwrap();
    let mut claims: serde_json::Value = serde_json::from_slice(&hex::decode(p).unwrap()).unwrap();
    claims["tier"] = "enterprise".into();
    format!("{}.{s}", hex::encode(serde_json::to_vec(&claims).unwrap()))
}

// ---------------------------------------------------------------------------------------
// Programme licences
// ---------------------------------------------------------------------------------------

#[test]
fn feeder_licence_is_evaluation_until_a_fresh_feed_receipt_and_drops_48h_after() {
    let tok = issue_v2(&SEED, &feeder_claims(&["ee-tll-01"])).unwrap();
    let lic = verify_with(&ring(), &tok, &at(T0)).unwrap();
    assert_eq!(lic.claims.v, programme::PROGRAMME_CLAIMS_VERSION);
    // Fail closed: verification alone grants nothing.
    assert_eq!(lic.entitlement.tier, Tier::Evaluation);
    assert!(!lic.entitlement.allows("pro-twin", T0));

    let r = issue_receipt(&SEED, &lic.token_id, "ee-tll-01", T0 - 3_600, T0).unwrap();
    let g = feed_entitlement(&ring(), &lic, &[&r], T0).unwrap();
    assert_eq!(g.entitlement.tier, Tier::Pro);
    assert!(g.entitlement.allows("pro-twin", T0));
    assert_eq!(g.active_stations, vec!["ee-tll-01".to_string()]);
    assert_eq!(g.holds_until_unix, T0 - 3_600 + FEED_GRACE_SECS);

    // 48 hours after the last observation: still Pro; one second later: Evaluation.
    assert!(feed_entitlement(&ring(), &lic, &[&r], T0 - 3_600 + FEED_GRACE_SECS).is_ok());
    let e = feed_entitlement(&ring(), &lic, &[&r], T0 - 3_600 + FEED_GRACE_SECS + 1)
        .unwrap_err()
        .to_string();
    assert!(e.contains("feed stopped"), "{e}");
    // No receipt at all.
    assert!(feed_entitlement(&ring(), &lic, &[], T0).is_err());
}

#[test]
fn feed_receipts_that_do_not_belong_are_refused() {
    let tok = issue_v2(&SEED, &feeder_claims(&["st-a"])).unwrap();
    let lic = verify_with(&ring(), &tok, &at(T0)).unwrap();
    // Another licence's receipt.
    let foreign = issue_receipt(&SEED, "kt_someone_else", "st-a", T0, T0).unwrap();
    assert!(feed_entitlement(&ring(), &lic, &[&foreign], T0)
        .unwrap_err()
        .to_string()
        .contains("another licence"));
    // A station the licence does not list.
    let unlisted = issue_receipt(&SEED, &lic.token_id, "st-b", T0, T0).unwrap();
    assert!(feed_entitlement(&ring(), &lic, &[&unlisted], T0).is_err());
    // Signed by a key the build does not trust (made up locally).
    let forged = issue_receipt(&OTHER, &lic.token_id, "st-a", T0, T0).unwrap();
    assert!(feed_entitlement(&ring(), &lic, &[&forged], T0).is_err());
    // Tampered: the last-observation time moved forward after signing.
    let good = issue_receipt(&SEED, &lic.token_id, "st-a", T0 - 10 * DAY, T0 - 10 * DAY).unwrap();
    let (p, s) = good.split_once('.').unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&hex::decode(p).unwrap()).unwrap();
    v["last_obs_unix"] = T0.into();
    v["issued_unix"] = T0.into();
    let moved = format!("{}.{s}", hex::encode(serde_json::to_vec(&v).unwrap()));
    assert!(feed_entitlement(&ring(), &lic, &[&moved], T0).is_err());
    // A receipt is not a licence token and the other way round.
    assert!(verify_with(&ring(), &good, &at(T0)).is_err());
    assert!(programme::verify_receipt(&ring(), &tok).is_err());
    // A receipt dated in the future.
    let future = issue_receipt(&SEED, &lic.token_id, "st-a", T0 + DAY, T0 + DAY).unwrap();
    assert!(feed_entitlement(&ring(), &lic, &[&future], T0).is_err());
}

#[test]
fn host_licence_counts_active_stations_and_caps_seats() {
    let mut c = base("pro", &["station", "desktop"], 300);
    c.seats = 2;
    c.programme = Some(Programme {
        kind: "host".into(),
        stations: vec!["port-1".into(), "port-2".into()],
        ..Default::default()
    });
    let tok = issue_v2(&SEED, &c).unwrap();
    let lic = verify_with(&ring(), &tok, &at(T0)).unwrap();
    let r1 = issue_receipt(&SEED, &lic.token_id, "port-1", T0 - 60, T0).unwrap();
    let r2 = issue_receipt(&SEED, &lic.token_id, "port-2", T0 - 3 * DAY, T0).unwrap();
    let g = feed_entitlement(&ring(), &lic, &[&r1, &r2], T0).unwrap();
    assert_eq!(g.active_stations, vec!["port-1".to_string()]);
    // Seats above one per listed station are refused at issue.
    c.seats = 3;
    assert!(issue_v2(&SEED, &c).is_err());
}

#[test]
fn teaching_licence_grants_pro_directly_within_its_policy() {
    let mut c = base("pro", &["cli", "desktop"], 200);
    c.seats = 40;
    c.programme = Some(Programme {
        kind: "teaching".into(),
        institution: "Example University".into(),
        course: "GNSS resilience 101".into(),
        ..Default::default()
    });
    let tok = issue_v2(&SEED, &c).unwrap();
    let lic = verify_with(&ring(), &tok, &at(T0)).unwrap();
    assert_eq!(lic.entitlement.tier, Tier::Pro);
    assert_eq!(programme::kind_of(&lic), Some(Kind::Teaching));
    // Expired.
    assert!(verify_with(&ring(), &tok, &at(T0 + 201 * DAY)).is_err());
    // Tampered (tier raised after signing).
    assert!(verify_with(&ring(), &tamper(&tok), &at(T0)).is_err());

    let refused = |f: &dyn Fn(&mut LicenseClaims)| {
        let mut x = c.clone();
        f(&mut x);
        issue_v2(&SEED, &x).unwrap_err().to_string()
    };
    assert!(refused(&|x| x.tier = "enterprise".into()).contains("at most 'pro'"));
    assert!(refused(&|x| x.programme.as_mut().unwrap().course.clear()).contains("course"));
    assert!(refused(&|x| x.expires_unix = 0).contains("expire"));
    assert!(refused(&|x| x.expires_unix = T0 + 400 * DAY).contains("expire"));
    assert!(refused(&|x| x.products = vec!["service".into()]).contains("not 'service'"));
    assert!(refused(&|x| x.products = vec!["sdk".into()]).contains("not 'sdk'"));
    assert!(refused(&|x| x.features = vec!["onprem-contract".into()]).contains("never"));
    assert!(refused(&|x| x.features = vec!["pro-ks-inside".into()]).contains("never"));
    assert!(refused(&|x| x.features = vec!["pro-batch".into()]).contains("above"));
    assert!(refused(&|x| x.features = vec!["pro-not-a-feature".into()]).contains("above"));
    assert!(refused(&|x| x.vetted_buyer = true).contains("vetting"));
    assert!(
        refused(&|x| x.programme.as_mut().unwrap().kind = "sponsor".into())
            .contains("unknown programme")
    );
    assert!(
        refused(&|x| x.programme.as_mut().unwrap().stations = vec!["s".into()])
            .contains("no stations")
    );
}

#[test]
fn programme_rules_are_enforced_at_verification_not_only_at_issue() {
    // Signed directly (bypassing issue_v2's checks), as a careless or compromised issuing
    // path might: verification still refuses the policy breach.
    let mut c = feeder_claims(&["st"]);
    c.tier = "enterprise".into();
    c.v = 3;
    let tok = issue(&SEED, &c).unwrap();
    assert!(verify_with(&ring(), &tok, &at(T0))
        .unwrap_err()
        .to_string()
        .contains("at most 'pro'"));
    // A programme in a v2 token (an older build would ignore it): refused.
    let mut c2 = feeder_claims(&["st"]);
    c2.v = 2;
    assert!(verify_with(&ring(), &issue(&SEED, &c2).unwrap(), &at(T0)).is_err());
    // A v3 token with no programme: refused.
    let mut c3 = base("pro", &["cli"], 30);
    c3.v = 3;
    assert!(verify_with(&ring(), &issue(&SEED, &c3).unwrap(), &at(T0)).is_err());
    // A version this build does not read.
    let mut c4 = feeder_claims(&["st"]);
    c4.v = 4;
    assert!(verify_with(&ring(), &issue(&SEED, &c4).unwrap(), &at(T0))
        .unwrap_err()
        .to_string()
        .contains("newer than this build"));
    // An ordinary token is unchanged on the wire: no programme, no opt-in, v2.
    let plain = issue_v2(&SEED, &base("pro", &["cli"], 30)).unwrap();
    let (p, _) = plain.split_once('.').unwrap();
    let text = String::from_utf8(hex::decode(p).unwrap()).unwrap();
    assert!(!text.contains("programme") && !text.contains("telemetry") && text.contains("\"v\":2"));
    assert_eq!(
        verify_with(&ring(), &plain, &at(T0))
            .unwrap()
            .entitlement
            .tier,
        Tier::Pro
    );
}

/// A P-256 binder standing in for a TPM-held key, to test the interface (no TPM here).
struct P256TestBinder(p256::ecdsa::SigningKey);

impl BenchBinder for P256TestBinder {
    fn class(&self) -> BindingClass {
        BindingClass::Tpm
    }
    fn algorithm(&self) -> &'static str {
        "p256"
    }
    fn public_key(&self) -> Result<Vec<u8>, kshana::api::KshanaError> {
        Ok(self
            .0
            .verifying_key()
            .to_sec1_point(false)
            .as_bytes()
            .to_vec())
    }
    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, kshana::api::KshanaError> {
        use p256::ecdsa::signature::Signer as _;
        let s: p256::ecdsa::Signature = self.0.sign(msg);
        Ok(s.to_bytes().to_vec())
    }
}

fn bench_claims(key: &str, class: &str) -> LicenseClaims {
    let mut c = base("enterprise", &["cli", "station"], 0);
    c.features = vec!["pro-eloran-hw".into()];
    c.programme = Some(Programme {
        kind: "bench".into(),
        bench_key: key.into(),
        bench_class: class.into(),
        ..Default::default()
    });
    c
}

#[test]
fn bench_licence_needs_the_bound_key_of_the_required_class() {
    let dir = tmp("bench");
    let soft = SoftwareBinder::load_or_create(&dir.join("bench-key.seed")).unwrap();
    // The same file loads the same key.
    let again = SoftwareBinder::load(&dir.join("bench-key.seed")).unwrap();
    assert_eq!(soft.key_id(), again.key_id());

    let tok = issue_v2(&SEED, &bench_claims(&soft.key_id(), "software")).unwrap();
    let lic = verify_with(&ring(), &tok, &at(T0)).unwrap();
    assert_eq!(
        lic.entitlement.tier,
        Tier::Evaluation,
        "gated until the key answers"
    );
    let ent = bench_entitlement(&lic, &soft, T0).unwrap();
    assert_eq!(ent.tier, Tier::Enterprise);
    assert!(ent.allows("pro-eloran-hw", T0));

    // Wrong machine: another bench's key.
    let other = SoftwareBinder::from_seed([9; 32]);
    assert!(bench_entitlement(&lic, &other, T0)
        .unwrap_err()
        .to_string()
        .contains("not the one"));
    // A TPM-class token is not satisfied by the software key, even the bound one.
    let tpm_tok = issue_v2(&SEED, &bench_claims(&soft.key_id(), "tpm")).unwrap();
    let tpm_lic = verify_with(&ring(), &tpm_tok, &at(T0)).unwrap();
    assert!(bench_entitlement(&tpm_lic, &soft, T0)
        .unwrap_err()
        .to_string()
        .contains("requires a 'tpm' key"));
    // The interface accepts a P-256 key reported as TPM-held (the shape a TPM binder takes).
    let p = P256TestBinder(p256::ecdsa::SigningKey::from_slice(&[5u8; 32]).unwrap());
    let pid = bench_key_id("p256", &p.public_key().unwrap());
    let p_lic = verify_with(
        &ring(),
        &issue_v2(&SEED, &bench_claims(&pid, "tpm")).unwrap(),
        &at(T0),
    )
    .unwrap();
    assert_eq!(
        bench_entitlement(&p_lic, &p, T0).unwrap().tier,
        Tier::Enterprise
    );
    // Wrong kind: a feeder licence binds no bench key.
    let f = verify_with(
        &ring(),
        &issue_v2(&SEED, &feeder_claims(&["x"])).unwrap(),
        &at(T0),
    )
    .unwrap();
    assert!(bench_entitlement(&f, &soft, T0).is_err());
    // Expired bench licence.
    let mut c = bench_claims(&soft.key_id(), "software");
    c.expires_unix = T0 + DAY;
    let short = issue_v2(&SEED, &c).unwrap();
    assert!(verify_with(&ring(), &short, &at(T0 + 2 * DAY)).is_err());
    // Bench tokens never carry the on-premises contract grant.
    let mut c = bench_claims(&soft.key_id(), "software");
    c.features.push("onprem-contract".into());
    assert!(issue_v2(&SEED, &c).is_err());
    // A malformed key ID or class is refused at issue.
    assert!(issue_v2(&SEED, &bench_claims("rsa:00", "software")).is_err());
    assert!(issue_v2(&SEED, &bench_claims(&soft.key_id(), "hsm")).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_programme_kind_has_an_explicit_tier_cap() {
    for k in Kind::ALL {
        let p = k.policy();
        assert!(p.max_tier >= Tier::Pro, "{k:?}");
        assert!(!p.products.is_empty());
        assert!(
            !p.products.contains(&"sdk"),
            "no programme opens the OEM SDK"
        );
    }
    assert_eq!(Kind::Feeder.policy().max_tier, Tier::Pro);
    assert_eq!(Kind::Host.policy().max_tier, Tier::Pro);
    assert_eq!(Kind::Teaching.policy().max_tier, Tier::Pro);
    assert_eq!(Kind::Bench.policy().max_tier, Tier::Enterprise);
}

// ---------------------------------------------------------------------------------------
// KS-Inside SDK and metering
// ---------------------------------------------------------------------------------------

fn oem_claims(days: i64) -> LicenseClaims {
    let mut c = base("enterprise", &["sdk"], days);
    c.features = vec![ks_inside::FEATURE.into()];
    c
}

fn cfg(token: String, dir: &std::path::Path) -> Config {
    Config {
        token,
        unit_id: "SN-0001".into(),
        meter_log: dir.join("meter.jsonl"),
        revocations: Vec::new(),
    }
}

fn drift_input() -> String {
    let series: Vec<[f64; 2]> = (0..900)
        .map(|i| {
            let t = 1_800_000_000.0 + i as f64;
            let te = if i < 400 {
                0.0
            } else {
                (i - 400) as f64 * 5e-9
            };
            [t, te]
        })
        .collect();
    serde_json::json!({ "series": series, "name": "unit-test clock" }).to_string()
}

#[test]
fn sdk_opens_only_under_an_oem_licence() {
    let dir = tmp("open");
    let ok = issue_v2(&SEED, &oem_claims(365)).unwrap();
    assert!(KsInside::open_with(&ring(), &cfg(ok.clone(), &dir), T0).is_ok());

    let refused = |c: LicenseClaims| match KsInside::open_with(
        &ring(),
        &cfg(issue_v2(&SEED, &c).unwrap(), &dir),
        T0,
    ) {
        Err(e) => e,
        Ok(_) => panic!("opened under {c:?}"),
    };
    // No sdk product.
    let mut c = oem_claims(365);
    c.products = vec!["cli".into()];
    assert!(matches!(refused(c), CallError::Licence(_)));
    // Every product but sdk (a default `license issue` token).
    let mut c = oem_claims(365);
    c.products = Product::DEFAULT.iter().map(|p| p.as_str().into()).collect();
    assert!(matches!(refused(c), CallError::Licence(_)));
    // No explicit grant, even at Enterprise.
    let mut c = oem_claims(365);
    c.features.clear();
    assert!(refused(c).message().contains("grant"));
    // The grant at Pro tier: the feature is Enterprise.
    let mut c = oem_claims(365);
    c.tier = "pro".into();
    assert!(matches!(refused(c), CallError::Licence(_)));
    // Bound to another machine.
    let mut c = oem_claims(365);
    c.machine = Some(machine::hash_machine_id("some-other-device"));
    assert!(refused(c).message().contains("another machine"));
    // Expired.
    let e = KsInside::open_with(&ring(), &cfg(ok.clone(), &dir), T0 + 400 * DAY).unwrap_err();
    assert!(matches!(e, CallError::Licence(_)));
    // Tampered.
    assert!(KsInside::open_with(&ring(), &cfg(tamper(&ok), &dir), T0).is_err());
    // Signed by another key.
    let other = issue_v2(&OTHER, &oem_claims(365)).unwrap();
    assert!(KsInside::open_with(&ring(), &cfg(other, &dir), T0).is_err());
    // Revoked by a signed list shipped with the firmware.
    let id = verify_with(&ring(), &ok, &at(T0)).unwrap().token_id;
    let list = revocation::sign(
        &SEED,
        &revocation::RevocationList {
            format: revocation::FORMAT.into(),
            sequence: 1,
            issued_unix: T0,
            revoked: vec![id],
            ..Default::default()
        },
    )
    .unwrap();
    let mut rc = cfg(ok.clone(), &dir);
    rc.revocations = vec![list];
    assert!(KsInside::open_with(&ring(), &rc, T0).is_err());
    // An empty unit ID.
    let mut uc = cfg(ok, &dir);
    uc.unit_id = " ".into();
    assert!(matches!(
        KsInside::open_with(&ring(), &uc, T0).unwrap_err(),
        CallError::Argument(_)
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn every_call_is_metered_and_the_report_counts_units() {
    let dir = tmp("meter");
    let tok = issue_v2(&SEED, &oem_claims(30)).unwrap();
    let k = KsInside::open_with(&ring(), &cfg(tok, &dir), T0).unwrap();
    let log = dir.join("meter.jsonl");

    let out = k.call("timing.drift", &drift_input(), T0).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["samples"], 900);
    assert!(v["kind"].is_string());
    assert_eq!(meter::verify(&log).unwrap(), 1);

    // Bad input: metered as an error.
    assert!(matches!(
        k.call("timing.drift", "{\"series\": []}", T0).unwrap_err(),
        CallError::Input(_)
    ));
    // An open scenario through scenario.run: metered as an error, not run.
    assert!(matches!(
        k.call("scenario.run", "kind = \"clock\"\n", T0)
            .unwrap_err(),
        CallError::Input(_)
    ));
    // Unknown method: no Pro use, not metered.
    assert!(matches!(
        k.call("timing.secret", "{}", T0).unwrap_err(),
        CallError::Method(_)
    ));
    // After expiry the open handle refuses, and the refusal is metered.
    assert!(matches!(
        k.call("timing.drift", &drift_input(), T0 + 31 * DAY)
            .unwrap_err(),
        CallError::Refused(_)
    ));
    assert_eq!(meter::verify(&log).unwrap(), 4);

    let s = meter::summarise(&log).unwrap();
    assert_eq!(s.lines, 4);
    let ok: u64 = s.months.iter().map(|m| m.ok).sum();
    let refused: u64 = s.months.iter().map(|m| m.refused).sum();
    let error: u64 = s.months.iter().map(|m| m.error).sum();
    assert_eq!((ok, refused, error), (1, 1, 2));
    assert_eq!(s.months[0].active_units, 1);
    assert_eq!(s.months[0].by_method["timing.drift"], 1);
    // The log holds the unit only as a hash bound to the licence, and no input values.
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(!text.contains("SN-0001") && !text.contains("unit-test clock"));
    let first: meter::MeterLine = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(first.outcome, Outcome::Ok);
    assert_eq!(first.unit, meter::unit_hash(&first.licence, "SN-0001"));

    // Tampering breaks the chain: verification fails and the log is not reopened.
    // (an edit of the first line; the last line is pinned by the summary's `head`).
    let edited = text.replacen("\"outcome\":\"ok\"", "\"outcome\":\"error\"", 1);
    assert_ne!(edited, text);
    std::fs::write(&log, edited).unwrap();
    assert!(meter::verify(&log).is_err());
    let tok2 = issue_v2(&SEED, &oem_claims(30)).unwrap();
    assert!(matches!(
        KsInside::open_with(&ring(), &cfg(tok2, &dir), T0).unwrap_err(),
        CallError::Meter(_)
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_unmetered_use_when_the_meter_log_cannot_be_written() {
    let dir = tmp("nometer");
    let tok = issue_v2(&SEED, &oem_claims(30)).unwrap();
    let mut c = cfg(tok, &dir);
    // A directory that does not exist: opening reads an empty log, the first write fails.
    c.meter_log = dir.join("missing").join("meter.jsonl");
    let k = KsInside::open_with(&ring(), &c, T0).unwrap();
    assert!(matches!(
        k.call("timing.drift", &drift_input(), T0).unwrap_err(),
        CallError::Meter(_)
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn timebomb_method_reads_a_capture() {
    let dir = tmp("tb");
    let tok = issue_v2(&SEED, &oem_claims(30)).unwrap();
    let k = KsInside::open_with(&ring(), &cfg(tok, &dir), T0).unwrap();
    let spec = kshana_pro::timing::timebomb::catalogue(&Default::default())
        .into_iter()
        .next()
        .expect("the catalogue has a vector");
    let expected = kshana_pro::timing::timebomb::expected_csv(&spec);
    let input = serde_json::json!({
        "vector": spec,
        "capture_format": "csv",
        "capture": expected,
    })
    .to_string();
    let out = k.call("timing.timebomb-analyse", &input, T0).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["verdict"].is_string(), "{v}");
    assert!(v["device_records"].as_u64().unwrap() > 0, "{v}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sdk_feature_and_methods_have_explicit_tiers() {
    assert!(PRO_COMMAND_FEATURES.contains(&ks_inside::FEATURE));
    assert_eq!(explicit_tier(ks_inside::FEATURE), Some(Tier::Enterprise));
    for m in METHODS {
        if let Some(f) = m.feature {
            assert!(explicit_tier(f).is_some(), "{f}");
        }
    }
    assert!(!Product::DEFAULT.contains(&Product::Sdk));
    assert!(Product::ALL.contains(&Product::Sdk));
}

// ---------------------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------------------

#[test]
fn header_matches_the_abi() {
    let h = include_str!("../sdk/ks-inside/include/ks_inside.h");
    let src = include_str!("../src/ks_inside/ffi.rs");
    for f in [
        "ks_inside_abi_version",
        "ks_inside_open",
        "ks_inside_call",
        "ks_inside_info",
        "ks_inside_free_string",
        "ks_inside_close",
    ] {
        assert!(h.contains(&format!("{f}(")), "header lacks {f}");
        assert!(
            src.contains(&format!("extern \"C\" fn {f}(")),
            "ffi lacks {f}"
        );
    }
    for (name, code) in [
        ("KS_OK", ffi::KS_OK),
        ("KS_ERR_ARGUMENT", ffi::KS_ERR_ARGUMENT),
        ("KS_ERR_LICENCE", ffi::KS_ERR_LICENCE),
        ("KS_ERR_REFUSED", ffi::KS_ERR_REFUSED),
        ("KS_ERR_METHOD", ffi::KS_ERR_METHOD),
        ("KS_ERR_INPUT", ffi::KS_ERR_INPUT),
        ("KS_ERR_METER", ffi::KS_ERR_METER),
        ("KS_ERR_INTERNAL", ffi::KS_ERR_INTERNAL),
    ] {
        assert!(h.contains(&format!("#define {name} {code}")), "{name}");
    }
    assert!(h.contains(&format!(
        "#define KS_INSIDE_ABI_VERSION {}",
        ks_inside::ABI_VERSION
    )));
    assert_eq!(ffi::ks_inside_abi_version(), ks_inside::ABI_VERSION);
}

#[test]
fn c_abi_checks_its_arguments() {
    let mut h: *mut ffi::KsInsideHandle = std::ptr::null_mut();
    let mut err: *mut std::ffi::c_char = std::ptr::null_mut();
    let rc = unsafe {
        ffi::ks_inside_open(
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            &mut h,
            &mut err,
        )
    };
    assert_eq!(rc, ffi::KS_ERR_ARGUMENT);
    assert!(h.is_null());
    assert!(!err.is_null());
    let msg = unsafe { CStr::from_ptr(err) }.to_str().unwrap().to_string();
    assert!(msg.contains("token is null"), "{msg}");
    unsafe { ffi::ks_inside_free_string(err) };
    // A call on a null handle.
    let m = CString::new("timing.drift").unwrap();
    let mut out: *mut std::ffi::c_char = std::ptr::null_mut();
    let rc = unsafe { ffi::ks_inside_call(std::ptr::null(), m.as_ptr(), m.as_ptr(), &mut out) };
    assert_eq!(rc, ffi::KS_ERR_ARGUMENT);
    unsafe {
        ffi::ks_inside_free_string(out);
        ffi::ks_inside_close(std::ptr::null_mut());
        assert!(ffi::ks_inside_info(std::ptr::null()).is_null());
    }
}

/// End to end through the C ABI with the debug-only demo key (a release build has no
/// demo key, so the same token is refused there).
#[cfg(debug_assertions)]
#[test]
fn c_abi_round_trip_with_the_demo_key() {
    if option_env!("KSHANA_PRO_LICENSE_PUBKEY").is_some() {
        return; // a build with real keys does not trust the demo key
    }
    const DEMO: [u8; KEY_LEN] = *b"kshana-pro-demo-key-debug-only!!";
    let dir = tmp("ffi");
    let mut c = oem_claims(0);
    c.issued_unix = 0;
    let tok = CString::new(issue_v2(&DEMO, &c).unwrap()).unwrap();
    let unit = CString::new("SN-C-1").unwrap();
    let log = CString::new(dir.join("m.jsonl").to_str().unwrap()).unwrap();
    let mut h: *mut ffi::KsInsideHandle = std::ptr::null_mut();
    let mut err: *mut std::ffi::c_char = std::ptr::null_mut();
    let rc = unsafe {
        ffi::ks_inside_open(
            tok.as_ptr(),
            unit.as_ptr(),
            log.as_ptr(),
            std::ptr::null(),
            &mut h,
            &mut err,
        )
    };
    assert_eq!(rc, ffi::KS_OK, "{:?}", unsafe {
        (!err.is_null()).then(|| CStr::from_ptr(err).to_string_lossy().into_owned())
    });
    let info = unsafe { ffi::ks_inside_info(h) };
    let iv: serde_json::Value =
        serde_json::from_str(unsafe { CStr::from_ptr(info) }.to_str().unwrap()).unwrap();
    assert_eq!(iv["abi_version"], ks_inside::ABI_VERSION);
    unsafe { ffi::ks_inside_free_string(info) };

    let m = CString::new("timing.drift").unwrap();
    let input = CString::new(drift_input()).unwrap();
    let mut out: *mut std::ffi::c_char = std::ptr::null_mut();
    let rc = unsafe { ffi::ks_inside_call(h, m.as_ptr(), input.as_ptr(), &mut out) };
    assert_eq!(rc, ffi::KS_OK);
    let v: serde_json::Value =
        serde_json::from_str(unsafe { CStr::from_ptr(out) }.to_str().unwrap()).unwrap();
    assert_eq!(v["samples"], 900);
    unsafe { ffi::ks_inside_free_string(out) };

    let bad = CString::new("no.such").unwrap();
    let rc = unsafe { ffi::ks_inside_call(h, bad.as_ptr(), input.as_ptr(), &mut out) };
    assert_eq!(rc, ffi::KS_ERR_METHOD);
    unsafe {
        ffi::ks_inside_free_string(out);
        ffi::ks_inside_close(h);
    }
    assert_eq!(meter::verify(&dir.join("m.jsonl")).unwrap(), 1);
    let _ = std::fs::remove_dir_all(dir);
}
