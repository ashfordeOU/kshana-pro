// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// OIDC bearer tokens (roadmap A9): tokens signed by an independent implementation (Python
// `cryptography`, tests/fixtures/oidc/make_tokens.py) pass with the right principal, and
// every kind of bad token is refused for the right reason.

use std::path::Path;

use kshana_pro::serve::auth::{Authenticator, Refusal, Role, Source};
use kshana_pro::serve::oidc::{b64url_decode, Verifier};

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oidc")
}

fn tokens() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir().join("tokens.json")).unwrap()).unwrap()
}

#[test]
fn independent_tokens_pass_with_their_principal() {
    let v = Verifier::from_file(&dir().join("oidc.toml")).unwrap();
    let t = tokens();
    let now = t["now"].as_i64().unwrap();
    let pass = t["pass"].as_object().unwrap();
    assert!(pass.len() >= 11);
    for (name, case) in pass {
        let p = v
            .principal(case[0].as_str().unwrap(), now)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let want = &case[1];
        assert_eq!(p.id, want["id"].as_str().unwrap(), "{name}");
        assert_eq!(p.tenant, want["tenant"].as_str().unwrap(), "{name}");
        assert_eq!(p.role.as_str(), want["role"].as_str().unwrap(), "{name}");
        assert_eq!(p.source, Source::Oidc);
    }
}

#[test]
fn bad_tokens_are_refused_for_the_right_reason() {
    let v = Verifier::from_file(&dir().join("oidc.toml")).unwrap();
    let t = tokens();
    let now = t["now"].as_i64().unwrap();
    let refuse = t["refuse"].as_object().unwrap();
    assert!(refuse.len() >= 15);
    for (name, case) in refuse {
        let e = v.principal(case[0].as_str().unwrap(), now).expect_err(name);
        assert_eq!(format!("{e:?}"), case[1].as_str().unwrap(), "{name}");
    }
    // The leeway is 60 s: a token expired 30 s ago passes, one expired 120 s ago does not;
    // the same good token far in the future is expired.
    let good = t["pass"]["es256"][0].as_str().unwrap();
    assert!(v.principal(good, now + 10_000).is_err());
}

#[test]
fn the_authenticator_sends_jwts_to_the_verifier_and_keys_elsewhere() {
    let v = Verifier::from_file(&dir().join("oidc.toml")).unwrap();
    let a = Authenticator::new(None, None).with_oidc(v);
    let t = tokens();
    let now = t["now"].as_i64().unwrap();
    let good = t["pass"]["rs256"][0].as_str().unwrap();
    let p = a
        .authenticate(Some(&format!("Bearer {good}")), now)
        .unwrap();
    assert_eq!(p.role, Role::Admin);
    let bad = t["refuse"]["tampered"][0].as_str().unwrap();
    assert_eq!(
        a.authenticate(Some(&format!("Bearer {bad}")), now),
        Err(Refusal::Invalid)
    );
    assert_eq!(
        a.authenticate(Some("Bearer kpk_123"), now),
        Err(Refusal::Invalid)
    );
    assert_eq!(a.authenticate(None, now), Err(Refusal::Missing));
}

#[test]
fn a_replaced_key_set_is_read_again() {
    let tmp = std::env::temp_dir().join(format!("kshana-pro-oidc-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    for f in ["oidc.toml", "jwks.json"] {
        std::fs::copy(dir().join(f), tmp.join(f)).unwrap();
    }
    let v = Verifier::from_file(&tmp.join("oidc.toml")).unwrap();
    let t = tokens();
    let now = t["now"].as_i64().unwrap();
    let good = t["pass"]["es256"][0].as_str().unwrap();
    assert!(v.principal(good, now).is_ok());
    // Rotate: the EC key is removed. Bump the modification time so the change is seen.
    let mut jwks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp.join("jwks.json")).unwrap()).unwrap();
    jwks["keys"]
        .as_array_mut()
        .unwrap()
        .retain(|k| k["kid"] != "ec1");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(tmp.join("jwks.json"), jwks.to_string()).unwrap();
    assert_eq!(
        format!("{:?}", v.principal(good, now).unwrap_err()),
        "UnknownKey"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn base64url_is_strict() {
    assert_eq!(b64url_decode("TWFu").unwrap(), b"Man");
    assert_eq!(b64url_decode("TWE").unwrap(), b"Ma");
    assert_eq!(b64url_decode("_-8").unwrap(), [0xff, 0xef]);
    assert!(b64url_decode("TWFu=").is_none());
    assert!(b64url_decode("T").is_none());
    assert!(b64url_decode("TW+u").is_none());
}
