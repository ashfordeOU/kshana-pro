// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The `arksa` entitlement (roadmap 0.42 X1).
//!
//! * Tokens issued before the claim existed still verify; they lack the claim.
//! * Only an Arksa issuer key can grant it: a Pro key's token carrying it is refused, and
//!   editing it into or out of a token breaks the signature.
//! * Every Arksa-exclusive kind and feature refuses a standalone Pro token, whatever its
//!   licence tier or feature grants, with a message that names no kind and no method.
//! * This crate contains none of them and refuses them by the manifest's hash, even under
//!   an Arksa-entitled token. The Arksa-exclusive code and the tests that run it live in
//!   the private server-side crate that registers through `kshana_pro::extension`.
//!
//! The two seeds below are TEST keys. They exist only in the tests and sign nothing that
//! ships; the public half of the Arksa one is the server-side crate's placeholder issuer key.

use kshana_pro::license::tiers::{self, NOT_IN_THIS_EDITION};
use kshana_pro::license::{
    self, issue, issue_arksa, issue_v2, verify_with, Entitlement, Keyring, LicenseClaims, Tier,
    VerifyOptions,
};

const PRO_TEST_SEED: [u8; 32] = [7; 32];
const ARKSA_TEST_SEED: [u8; 32] = *b"arksa-issuer-test-key-not-real!!";

fn ring() -> Keyring {
    let mut r = Keyring::single(license::public_key_for(&PRO_TEST_SEED));
    r.add_arksa_issuer(license::public_key_for(&ARKSA_TEST_SEED));
    r
}

fn claims(tier: &str) -> LicenseClaims {
    LicenseClaims {
        licensee: "ACME Space".into(),
        tier: tier.into(),
        expires_unix: 10_000,
        products: vec!["cli".into(), "service".into()],
        ..Default::default()
    }
}

fn at(now: i64) -> VerifyOptions<'static> {
    VerifyOptions::at(now)
}

/// Rewrite the JSON payload of a signed token, keeping its signature.
fn edit_payload(token: &str, f: impl Fn(String) -> String) -> String {
    let (p, sig) = token.split_once('.').unwrap();
    let json = String::from_utf8(hex::decode(p).unwrap()).unwrap();
    format!("{}.{sig}", hex::encode(f(json)))
}

#[test]
fn old_tokens_verify_and_lack_the_claim() {
    let r = ring();
    let v1 = issue(
        &PRO_TEST_SEED,
        &LicenseClaims {
            licensee: "ACME".into(),
            tier: "enterprise".into(),
            expires_unix: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let v2 = issue_v2(&PRO_TEST_SEED, &claims("enterprise")).unwrap();
    for t in [&v1, &v2] {
        let l = verify_with(&r, t, &at(1)).unwrap();
        assert!(!l.claims.arksa);
        assert!(!l.entitlement.arksa);
        // The wire form of a token without the claim does not mention it.
        let json = String::from_utf8(hex::decode(t.split_once('.').unwrap().0).unwrap()).unwrap();
        assert!(!json.contains("arksa"), "{json}");
    }
}

#[test]
fn an_arksa_token_verifies_under_the_issuer_slot_only() {
    let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    let l = verify_with(&ring(), &t, &at(1)).unwrap();
    assert!(l.claims.arksa && l.entitlement.arksa);
    assert_eq!(l.entitlement.tier, Tier::Pro);
    // A build with no Arksa issuer (every default build) refuses it.
    let pro_only = Keyring::single(license::public_key_for(&PRO_TEST_SEED));
    assert!(verify_with(&pro_only, &t, &at(1)).is_err());
    // The issuer key is not a Pro key: a token without the claim signed by it is refused.
    let plain = issue_v2(&ARKSA_TEST_SEED, &claims("enterprise")).unwrap();
    assert!(verify_with(&ring(), &plain, &at(1)).is_err());
    // Expiry still applies.
    assert!(verify_with(&ring(), &t, &at(10_000)).is_err());
}

#[test]
fn a_pro_key_cannot_grant_the_claim() {
    let mut c = claims("enterprise");
    c.arksa = true;
    let forged = issue_v2(&PRO_TEST_SEED, &c).unwrap();
    assert!(verify_with(&ring(), &forged, &at(1)).is_err());
    let forged_v1 = issue(&PRO_TEST_SEED, &c).unwrap();
    assert!(verify_with(&ring(), &forged_v1, &at(1)).is_err());
}

#[test]
fn tampered_claims_fail() {
    let r = ring();
    // The claim added to a Pro token.
    let pro = issue_v2(&PRO_TEST_SEED, &claims("enterprise")).unwrap();
    let added = edit_payload(&pro, |j| j.replacen('{', "{\"arksa\":true,", 1));
    assert!(verify_with(&r, &added, &at(1)).is_err());
    // The claim removed from an Arksa token.
    let ark = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    let removed = edit_payload(&ark, |j| j.replace(",\"arksa\":true", ""));
    assert_ne!(removed, ark);
    assert!(verify_with(&r, &removed, &at(1)).is_err());
    // Another claim of an Arksa token changed: tier, expiry, licensee.
    for (from, to) in [
        ("\"tier\":\"pro\"", "\"tier\":\"enterprise\""),
        ("\"expires_unix\":10000", "\"expires_unix\":0"),
        ("ACME Space", "Someone Else"),
    ] {
        let t = edit_payload(&ark, |j| j.replace(from, to));
        assert_ne!(t, ark, "{from}");
        assert!(verify_with(&r, &t, &at(1)).is_err(), "{from}");
    }
    // The key ID of an Arksa token swapped to the Pro key's.
    let pro_kid = license::key_id(&license::public_key_for(&PRO_TEST_SEED));
    let ark_kid = license::key_id(&license::public_key_for(&ARKSA_TEST_SEED));
    let swapped = edit_payload(&ark, |j| j.replace(&ark_kid, &pro_kid));
    assert!(verify_with(&r, &swapped, &at(1)).is_err());
}

#[test]
fn a_retired_issuer_key_is_refused() {
    let mut r = ring();
    r.retire(&license::key_id(&license::public_key_for(&ARKSA_TEST_SEED)));
    let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    assert!(verify_with(&r, &t, &at(1)).is_err());
    assert!(r.arksa_issuer_ids().is_empty());
}

/// A standalone Pro token at the highest tier, granting every Arksa-exclusive id by name.
fn standalone_enterprise_naming(ids: &[&str]) -> Entitlement {
    let t = issue_v2(
        &PRO_TEST_SEED,
        &LicenseClaims {
            features: ids.iter().map(|s| s.to_string()).collect(),
            ..claims("enterprise")
        },
    )
    .unwrap();
    verify_with(&ring(), &t, &at(1)).unwrap().entitlement
}

/// Assert `kind` is refused for `ent` with the neutral message.
fn assert_refused_neutrally(ent: &Entitlement, kind: &str) {
    assert!(!ent.allows(kind, 1), "{kind}");
    let e = license::check(ent, kind, 1).unwrap_err().to_string();
    assert!(e.contains(NOT_IN_THIS_EDITION), "{kind}: {e}");
    assert!(!e.contains(kind), "the refusal names the kind: {e}");
    let src = format!("kind = \"{kind}\"\n");
    let e = kshana_pro::registry::run_gated(&src, ent, 1)
        .unwrap_err()
        .to_string();
    assert!(e.contains(NOT_IN_THIS_EDITION), "{kind}: {e}");
    assert!(!e.contains(kind), "the refusal names the kind: {e}");
}

/// The Arksa-exclusive kind and feature names, read from the private term list at run time
/// (the `pro-` entries of its `[arksa]` section) so this source names none of them.
fn arksa_ids() -> Vec<String> {
    let terms = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/protect/private-terms.txt"
    ))
    .expect("protect/private-terms.txt is readable");
    let mut on = false;
    let mut ids = Vec::new();
    for line in terms.lines().map(str::trim) {
        if line.starts_with('[') {
            on = line == "[arksa]";
        } else if on && line.starts_with("pro-") && !line.contains(char::is_whitespace) {
            ids.push(line.to_string());
        }
    }
    ids
}

#[test]
fn this_crate_contains_no_arksa_exclusive_kind_and_refuses_them_by_hash() {
    let ids = arksa_ids();
    assert!(
        ids.len() >= 15,
        "the term list names the Arksa-exclusive ids"
    );
    let m = tiers::manifest().unwrap();
    assert_eq!(
        m.count_in(tiers::Edition::Arksa),
        tiers::ARKSA_ENTRIES.len()
    );
    // Nothing registers an extension in this crate's tests.
    assert!(kshana_pro::extension::get().is_none());
    for k in kshana_pro::registry::pro_kinds() {
        assert_ne!(tiers::edition_of(k.name), Some(tiers::Edition::Arksa));
    }
    let names: Vec<&str> = ids.iter().map(String::as_str).collect();
    let standalone = standalone_enterprise_naming(&names);
    assert!(!standalone.arksa);
    for id in &names {
        assert_eq!(tiers::edition_of(id), Some(tiers::Edition::Arksa), "{id}");
        assert!(!kshana_pro::registry::is_pro_kind(id), "{id}");
        assert_refused_neutrally(&standalone, id);
        assert_refused_neutrally(&Entitlement::evaluation(), id);
    }
    // Pro kinds are unchanged for the same token.
    assert!(standalone.allows("pro-twin", 1));
    assert!(standalone.allows("pro-batch", 1));
}

#[test]
fn an_arksa_token_runs_nothing_exclusive_in_this_crate() {
    // An Arksa-entitled token passes the licence check, but this crate has no Arksa code:
    // the dispatcher refuses each hidden kind with the same neutral message.
    let t = issue_arksa(&ARKSA_TEST_SEED, &claims("pro")).unwrap();
    let ent = verify_with(&ring(), &t, &at(1)).unwrap().entitlement;
    assert!(ent.arksa);
    for id in arksa_ids() {
        assert!(ent.allows(&id, 1), "{id}");
        let e = kshana_pro::registry::run_gated(&format!("kind = \"{id}\"\n"), &ent, 1)
            .unwrap_err()
            .to_string();
        assert!(e.contains(NOT_IN_THIS_EDITION), "{e}");
        assert!(!e.contains(&id), "the refusal names the kind: {e}");
    }
    // Expired: nothing exclusive is even allowed.
    let mut o = at(20_000);
    o.allow_expired = true;
    let ent = verify_with(&ring(), &t, &o).unwrap().entitlement;
    for id in arksa_ids() {
        assert!(!ent.allows(&id, 20_000), "{id}");
    }
}

#[test]
fn this_crate_trusts_no_arksa_issuer_of_its_own() {
    // Without a registered extension there is no Arksa issuer key, so a keyring built from
    // this crate's own keys refuses every token that carries the claim.
    assert!(license::embedded_arksa_issuers().unwrap().is_empty());
}
