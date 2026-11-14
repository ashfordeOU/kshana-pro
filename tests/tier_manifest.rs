// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The signed tier manifest (roadmap 0.42 X2): `commercial/tier-manifest.json` names the
//! edition tier of every Pro kind and command feature, is signed with Ed25519, is verified
//! at start-up, and agrees with the code. Kinds it does not list are refused.
//!
//! The manifest committed here is signed with a TEST key whose private half is below and
//! nowhere else; its public half is `tiers::PLACEHOLDER_MANIFEST_PUBKEY`. A release signs
//! with the vendor's offline manifest key instead (docs/LICENSING-KEYS.md, section 6).
//!
//! Regenerate after adding, removing or re-tiering a kind (the hidden entries come from
//! `tiers::ARKSA_ENTRIES`, by hash; this crate does not know their names):
//!
//!     KSHANA_PRO_REGEN_TIER_MANIFEST=1 cargo test --test tier_manifest

use ed25519_dalek::{Signer, SigningKey};
use kshana_pro::license::tiers::{self, Edition, ManifestDoc, ManifestEntry};
use kshana_pro::license::{self, Entitlement, Tier};

/// The test manifest key. Test only: never used to sign anything that ships.
const MANIFEST_TEST_SEED: [u8; 32] = *b"tier-manifest-test-key-not-real!";

const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/commercial/tier-manifest.json");

fn sign(bytes: &[u8]) -> String {
    hex::encode(
        SigningKey::from_bytes(&MANIFEST_TEST_SEED)
            .sign(bytes)
            .to_bytes(),
    )
}

fn test_key() -> [u8; 32] {
    license::public_key_for(&MANIFEST_TEST_SEED)
}

fn enterprise() -> Entitlement {
    Entitlement {
        licensee: "ACME".into(),
        tier: Tier::Enterprise,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    }
}

#[test]
fn placeholder_manifest_key_is_the_test_key() {
    assert_eq!(hex::encode(test_key()), tiers::PLACEHOLDER_MANIFEST_PUBKEY);
}

#[test]
fn committed_manifest_is_the_one_the_code_implies() {
    let want = tiers::manifest_text(&tiers::manifest_from_code());
    if std::env::var_os("KSHANA_PRO_REGEN_TIER_MANIFEST").is_some() {
        std::fs::write(MANIFEST_PATH, &want).unwrap();
        std::fs::write(format!("{MANIFEST_PATH}.sig"), sign(want.as_bytes()) + "\n").unwrap();
        eprintln!("regenerated {MANIFEST_PATH} and its signature; rebuild to embed them");
        return;
    }
    assert_eq!(
        tiers::EMBEDDED_MANIFEST,
        want,
        "commercial/tier-manifest.json is out of date; regenerate it (see this file's header)"
    );
}

#[test]
fn committed_signature_is_current() {
    // Ed25519 is deterministic: the committed signature must be the test key's signature
    // over the committed bytes. An edit to the manifest without re-signing fails here and
    // at start-up.
    let committed = std::fs::read_to_string(format!("{MANIFEST_PATH}.sig")).unwrap();
    assert_eq!(
        committed.trim(),
        sign(tiers::EMBEDDED_MANIFEST.as_bytes()),
        "the manifest signature is stale; regenerate (see this file's header)"
    );
}

#[test]
fn embedded_manifest_verifies_at_start_up_and_matches_this_build() {
    tiers::startup_check().expect("the start-up check passes");
    let m = tiers::manifest().unwrap();
    // Every name in the clear is a Pro or open-seam kind or feature of this build, and
    // every compiled Pro kind is listed.
    let mut named: Vec<&str> = m.named().into_iter().map(|(n, _, _)| n).collect();
    named.sort();
    named.dedup();
    let mut compiled: Vec<&str> = kshana_pro::registry::pro_kinds()
        .iter()
        .map(|k| k.name)
        .chain(license::pro_command_features())
        .collect();
    compiled.sort();
    compiled.dedup();
    assert_eq!(named, compiled);
    // The hidden tier is present (by hash), so this crate refuses the Arksa-exclusive kinds
    // by name without containing the names.
    assert_eq!(m.count_in(Edition::Arksa), tiers::ARKSA_ENTRIES.len());
    assert!(m.count_in(Edition::Arksa) > 0);
    // Every entry of the hidden tiers is a hash, never a name.
    for e in &m.doc().entries {
        let hidden = Edition::parse(&e.tier).unwrap().is_hidden();
        assert_eq!(e.kind.is_none(), hidden, "{e:?}");
        assert_eq!(e.kind_sha256.is_some(), hidden, "{e:?}");
    }
}

#[test]
fn manifest_kind_count_is_every_registry_kind_of_every_edition() {
    let m = tiers::manifest().unwrap();
    let pro_kinds = kshana_pro::registry::pro_kinds().len();
    assert_eq!(m.kind_count(), pro_kinds + tiers_missing_from_this_build());
}

/// Arksa-exclusive kinds this crate does not contain (all of them: their code is in the
/// server-side extension crate).
fn tiers_missing_from_this_build() -> usize {
    let m = tiers::manifest().unwrap();
    let hidden_kinds = m
        .doc()
        .entries
        .iter()
        .filter(|e| e.entry_type == "kind" && e.kind_sha256.is_some())
        .count();
    assert_eq!(
        hidden_kinds,
        tiers::ARKSA_ENTRIES
            .iter()
            .filter(|(_, ty)| *ty == "kind")
            .count()
    );
    hidden_kinds
}

#[test]
fn a_tampered_manifest_is_refused() {
    let key = test_key();
    let text = tiers::EMBEDDED_MANIFEST;
    let sig = sign(text.as_bytes());
    assert!(tiers::verify_manifest(text.as_bytes(), &sig, &key).is_ok());

    // Move a Pro kind to the free tier: the signature no longer verifies.
    let moved = text.replacen(
        "\"kind\": \"pro-twin\",\n      \"type\": \"kind\",\n      \"tier\": \"pro\"",
        "\"kind\": \"pro-twin\",\n      \"type\": \"kind\",\n      \"tier\": \"open-seam\"",
        1,
    );
    assert_ne!(moved, text, "the edit applies");
    let e = tiers::verify_manifest(moved.as_bytes(), &sig, &key)
        .unwrap_err()
        .to_string();
    assert!(e.contains("does not verify"), "{e}");

    // Another key, or a malformed signature: refused.
    let other = license::public_key_for(&[3; 32]);
    assert!(tiers::verify_manifest(text.as_bytes(), &sig, &other).is_err());
    assert!(tiers::verify_manifest(text.as_bytes(), "abcd", &key).is_err());
}

fn signed(doc: &ManifestDoc) -> (String, String) {
    let t = tiers::manifest_text(doc);
    let s = sign(t.as_bytes());
    (t, s)
}

#[test]
fn a_signed_manifest_of_the_wrong_form_is_refused() {
    let key = test_key();
    let entry = |kind: Option<&str>, hash: Option<String>, tier: &str| ManifestEntry {
        kind: kind.map(str::to_string),
        kind_sha256: hash,
        entry_type: "kind".into(),
        tier: tier.into(),
    };
    let doc = |entries| ManifestDoc {
        format: tiers::MANIFEST_FORMAT.into(),
        version: 1,
        entries,
    };
    // A hidden tier named in the clear.
    let (t, s) = signed(&doc(vec![entry(Some("pro-x"), None, "arksa")]));
    assert!(tiers::verify_manifest(t.as_bytes(), &s, &key).is_err());
    // An open tier given by hash.
    let (t, s) = signed(&doc(vec![entry(
        None,
        Some(tiers::name_hash("pro-x")),
        "pro",
    )]));
    assert!(tiers::verify_manifest(t.as_bytes(), &s, &key).is_err());
    // A tier that does not exist; an entry twice; another format.
    let (t, s) = signed(&doc(vec![entry(Some("pro-x"), None, "gold")]));
    assert!(tiers::verify_manifest(t.as_bytes(), &s, &key).is_err());
    let twice = vec![
        entry(Some("pro-x"), None, "pro"),
        entry(Some("pro-x"), None, "pro"),
    ];
    let (t, s) = signed(&doc(twice));
    assert!(tiers::verify_manifest(t.as_bytes(), &s, &key).is_err());
    let mut d = doc(vec![]);
    d.format = "something-else".into();
    let (t, s) = signed(&d);
    assert!(tiers::verify_manifest(t.as_bytes(), &s, &key).is_err());
    // The right form verifies, and a hidden entry is found by its name's hash.
    let (t, s) = signed(&doc(vec![
        entry(Some("pro-x"), None, "pro"),
        entry(None, Some(tiers::name_hash("pro-y")), "arksa"),
        entry(None, Some(tiers::name_hash("pro-z")), "core"),
    ]));
    let m = tiers::verify_manifest(t.as_bytes(), &s, &key).unwrap();
    assert_eq!(m.edition_of("pro-x"), Some(Edition::Pro));
    assert_eq!(m.edition_of("pro-y"), Some(Edition::Arksa));
    assert_eq!(m.edition_of("pro-z"), Some(Edition::Core));
    assert_eq!(m.edition_of("pro-w"), None);
}

#[test]
fn unknown_kinds_are_refused_even_for_enterprise() {
    let ent = enterprise();
    assert!(!ent.allows("pro-not-a-kind", 0));
    let e = license::check(&ent, "pro-not-a-kind", 0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("not a registered Pro pack"), "{e}");
    let e = kshana_pro::registry::run_gated("kind = \"pro-not-a-kind\"\n", &ent, 0)
        .unwrap_err()
        .to_string();
    assert!(e.contains("not a registered Pro pack"), "{e}");
    // Open kinds stay free and Pro kinds keep their licence tiers.
    assert!(ent.allows("ephemeris", 0));
    assert!(ent.allows("pro-batch", 0));
    assert!(ent.allows("pro-twin", 0));
}
