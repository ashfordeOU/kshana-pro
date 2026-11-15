// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Key custody S7-a: licence and pack issuing through AWS KMS, against a fake KMS endpoint
//! on the loopback address. The fake checks every request's SigV4 signature with the test
//! credentials (recomputed independently from the request bytes), the `TrentService`
//! target, `MessageType: RAW` and `ED25519_SHA_512`, then signs with a throwaway seed. A
//! token signed through KMS must equal, byte for byte, the token the same seed signs
//! directly, and verify with the unchanged `verify` code.
//!
//! Runs only with `--features kms` (CI step "KMS signer (S7-a)").
#![cfg(feature = "kms")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ed25519_dalek::{Signer, SigningKey};
use kshana_pro::license::kms::{
    base64_decode, base64_encode, sigv4_authorization, CredentialSource, Credentials, KmsSigner,
    SigV4Request,
};
use kshana_pro::license::revocation::{self, RevocationList};
use kshana_pro::license::{
    self, key_id, public_key_for, verify_with, Keyring, LicenceSigner, LicenseClaims,
    VerifyOptions, KEY_LEN,
};

const FAKE_SEED: [u8; KEY_LEN] = [0x42; KEY_LEN];
const AKID: &str = "AKIDTESTONLY";
const SECRET: &str = "test-secret-not-a-real-credential";
const REGION: &str = "eu-central-1";
const NOW: i64 = 1_790_000_000;

struct Fake {
    url: String,
    calls: Arc<AtomicUsize>,
}

fn read_request(s: &mut TcpStream) -> Option<(String, Vec<(String, String)>, Vec<u8>)> {
    let mut r = BufReader::new(s.try_clone().ok()?);
    let mut line = String::new();
    r.read_line(&mut line).ok()?;
    let mut headers = Vec::new();
    loop {
        let mut l = String::new();
        r.read_line(&mut l).ok()?;
        let l = l.trim_end().to_string();
        if l.is_empty() {
            break;
        }
        let (k, v) = l.split_once(':')?;
        headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
    }
    let n: usize = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; n];
    r.read_exact(&mut body).ok()?;
    Some((line, headers, body))
}

fn answer(s: &mut TcpStream, status: u16, body: &serde_json::Value) {
    let b = body.to_string();
    let _ = write!(
        s,
        "HTTP/1.1 {status} X\r\ncontent-type: application/x-amz-json-1.1\r\ncontent-length: {}\r\n\r\n{b}",
        b.len()
    );
}

/// The fake KMS. `key_spec` lets a test serve a key of the wrong kind.
fn fake_kms(key_spec: &'static str) -> Fake {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let c2 = calls.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let Some((line, headers, body)) = read_request(&mut s) else {
                continue;
            };
            c2.fetch_add(1, Ordering::SeqCst);
            let h = |n: &str| {
                headers
                    .iter()
                    .find(|(k, _)| k == n)
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            };
            // Recompute the SigV4 signature from what arrived.
            let auth = h("authorization");
            let signed_names: Vec<String> = auth
                .split("SignedHeaders=")
                .nth(1)
                .and_then(|r| r.split(',').next())
                .unwrap_or("")
                .split(';')
                .map(str::to_string)
                .collect();
            let signed: Vec<(String, String)> =
                signed_names.iter().map(|n| (n.clone(), h(n))).collect();
            let date = h("x-amz-date");
            let expect = sigv4_authorization(&SigV4Request {
                method: "POST",
                path: "/",
                query: "",
                headers: &signed,
                body: &body,
                access_key_id: AKID,
                secret_access_key: SECRET,
                region: REGION,
                service: "kms",
                amz_date: &date,
            });
            let required = ["host", "x-amz-date", "x-amz-target", "content-type"];
            if !line.starts_with("POST / ")
                || auth != expect
                || !required.iter().all(|r| signed_names.iter().any(|n| n == r))
            {
                answer(
                    &mut s,
                    400,
                    &serde_json::json!({"__type": "InvalidSignatureException", "message": "signature mismatch"}),
                );
                continue;
            }
            let req: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
            match h("x-amz-target").as_str() {
                "TrentService.GetPublicKey" => {
                    let mut der = vec![
                        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
                    ];
                    der.extend_from_slice(&public_key_for(&FAKE_SEED));
                    answer(
                        &mut s,
                        200,
                        &serde_json::json!({
                            "KeyId": req["KeyId"], "KeySpec": key_spec, "KeyUsage": "SIGN_VERIFY",
                            "PublicKey": base64_encode(&der),
                            "SigningAlgorithms": ["ED25519_SHA_512"],
                        }),
                    );
                }
                "TrentService.Sign" => {
                    if req["MessageType"] != "RAW" || req["SigningAlgorithm"] != "ED25519_SHA_512" {
                        answer(
                            &mut s,
                            400,
                            &serde_json::json!({"__type": "ValidationException", "message": "wrong algorithm"}),
                        );
                        continue;
                    }
                    let msg = base64_decode(req["Message"].as_str().unwrap_or("")).unwrap();
                    let sig = SigningKey::from_bytes(&FAKE_SEED).sign(&msg).to_bytes();
                    answer(
                        &mut s,
                        200,
                        &serde_json::json!({
                            "KeyId": req["KeyId"], "Signature": base64_encode(&sig),
                            "SigningAlgorithm": "ED25519_SHA_512",
                        }),
                    );
                }
                _ => answer(
                    &mut s,
                    400,
                    &serde_json::json!({"__type": "UnknownOperationException"}),
                ),
            }
        }
    });
    Fake { url, calls }
}

fn kms(f: &Fake, secret: &str) -> KmsSigner {
    KmsSigner::new(
        "alias/kshana-issuing-test",
        REGION,
        Some(&f.url),
        CredentialSource::Fixed(Credentials {
            access_key_id: AKID.into(),
            secret_access_key: secret.into(),
            session_token: Some("session-token-test".into()),
        }),
    )
    .unwrap()
    .with_clock(Arc::new(|| NOW))
}

fn claims() -> LicenseClaims {
    LicenseClaims {
        licensee: "KMS test".into(),
        tier: "pro".into(),
        expires_unix: NOW + 35 * 86_400,
        products: vec!["cli".into()],
        id: "kt_000000000000000000000001".into(),
        issued_unix: NOW,
        ..Default::default()
    }
}

#[test]
fn a_kms_signed_token_equals_the_seed_signed_token_and_verifies() {
    let f = fake_kms("ECC_NIST_EDWARDS25519");
    let k = kms(&f, SECRET);
    assert_eq!(k.public_key().unwrap(), public_key_for(&FAKE_SEED));
    assert_eq!(k.kid().unwrap(), key_id(&public_key_for(&FAKE_SEED)));
    let via_kms = license::issue_v2(&k, &claims()).unwrap();
    let via_seed = license::issue_v2(&FAKE_SEED, &claims()).unwrap();
    assert_eq!(via_kms, via_seed, "byte for byte");
    let ring = Keyring::single(public_key_for(&FAKE_SEED));
    let lic = verify_with(&ring, &via_kms, &VerifyOptions::at(NOW)).unwrap();
    assert_eq!(lic.kid, key_id(&public_key_for(&FAKE_SEED)));
    assert_eq!(lic.claims.licensee, "KMS test");
    // The public key is fetched once and cached.
    let before = f.calls.load(Ordering::SeqCst);
    license::issue_v2(&k, &claims()).unwrap();
    assert_eq!(f.calls.load(Ordering::SeqCst), before + 1);
}

#[test]
fn revocation_lists_the_register_and_packs_sign_through_kms() {
    let f = fake_kms("ECC_NIST_EDWARDS25519");
    let k: license::signer::SharedSigner = Arc::new(kms(&f, SECRET));
    let ring = Keyring::single(public_key_for(&FAKE_SEED));
    let list = RevocationList {
        sequence: 7,
        revoked: vec!["kt_a".into()],
        ..Default::default()
    };
    let text = revocation::sign(k.as_ref(), &list).unwrap();
    assert_eq!(text, revocation::sign(&FAKE_SEED, &list).unwrap());
    assert!(revocation::verify(&ring, &text).unwrap().is_revoked("kt_a"));

    // The issuance register and the licence service (issue-invoice, renew, online checks).
    let d = std::env::temp_dir().join(format!("kpro-kms-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let svc = license::lifecycle::LicenceService::open_with_signer(k.clone(), &[], &d).unwrap();
    let (tok, _) = svc
        .register
        .issue(k.as_ref(), &claims(), "manual", "", NOW)
        .unwrap();
    assert!(verify_with(&ring, &tok, &VerifyOptions::at(NOW)).is_ok());
    let _ = std::fs::remove_dir_all(&d);

    // A Pro pack sealed through KMS verifies under the KMS key's public half.
    let mut base = kshana_pro::propack::format::Container::default();
    base.put("readme.txt", b"pack".to_vec()).unwrap();
    let input = kshana_pro::propack::format::SealInput {
        pack_id: "kms-test",
        version: "1",
        platform: kshana_pro::propack::format::PLATFORMS[0],
        licensee: "KMS test",
        token_id: "kt_000000000000000000000001",
        device: &"ab".repeat(32),
        issued_unix: NOW,
        expires_unix: NOW + 86_400,
        grace_days: 30,
        provides: vec!["pro-twin".into()],
        apple_team_id: None,
    };
    let (file, manifest) =
        kshana_pro::propack::format::seal(&base, &input, &[7u8; 32], k.as_ref()).unwrap();
    assert_eq!(manifest.kid, key_id(&public_key_for(&FAKE_SEED)));
    assert!(
        kshana_pro::propack::format::verify_signed(&file, &[public_key_for(&FAKE_SEED)]).is_ok()
    );
    assert!(
        kshana_pro::propack::format::verify_signed(&file, &[public_key_for(&[1u8; 32])]).is_err()
    );
}

#[test]
fn bad_credentials_and_a_wrong_key_spec_are_refused() {
    let f = fake_kms("ECC_NIST_EDWARDS25519");
    let e = kms(&f, "wrong-secret").public_key().unwrap_err();
    assert!(e.to_string().contains("InvalidSignatureException"), "{e}");
    assert!(license::issue_v2(&kms(&f, "wrong-secret"), &claims()).is_err());
    let g = fake_kms("ECC_NIST_P256");
    let e = kms(&g, SECRET).public_key().unwrap_err();
    assert!(e.to_string().contains("ECC_NIST_EDWARDS25519"), "{e}");
}

#[test]
fn the_cli_issues_through_kms_when_configured() {
    let f = fake_kms("ECC_NIST_EDWARDS25519");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args([
            "license",
            "issue",
            "--tier",
            "pro",
            "--licensee",
            "CLI KMS",
            "--days",
            "3",
        ])
        .env("KSHANA_PRO_SIGNER", "kms")
        .env("KSHANA_PRO_KMS_KEY_ID", "alias/kshana-issuing-test")
        .env("KSHANA_PRO_KMS_REGION", REGION)
        .env("KSHANA_PRO_KMS_ENDPOINT", &f.url)
        .env("AWS_ACCESS_KEY_ID", AKID)
        .env("AWS_SECRET_ACCESS_KEY", SECRET)
        .env_remove("AWS_SESSION_TOKEN")
        .env_remove("KSHANA_PRO_SIGNING_KEY")
        .env(
            "KSHANA_PRO_KMS_EXPECT_KID",
            key_id(&public_key_for(&FAKE_SEED)),
        )
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("DEMO"), "{stderr}");
    let tok = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let ring = Keyring::single(public_key_for(&FAKE_SEED));
    let lic = verify_with(&ring, &tok, &VerifyOptions::at(now())).unwrap();
    assert_eq!(lic.claims.licensee, "CLI KMS");
    // A pinned key ID that does not match is refused before anything is signed.
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args([
            "license",
            "issue",
            "--tier",
            "pro",
            "--licensee",
            "x",
            "--days",
            "3",
        ])
        .env("KSHANA_PRO_SIGNER", "kms")
        .env("KSHANA_PRO_KMS_KEY_ID", "alias/kshana-issuing-test")
        .env("KSHANA_PRO_KMS_REGION", REGION)
        .env("KSHANA_PRO_KMS_ENDPOINT", &f.url)
        .env("AWS_ACCESS_KEY_ID", AKID)
        .env("AWS_SECRET_ACCESS_KEY", SECRET)
        .env_remove("AWS_SESSION_TOKEN")
        .env("KSHANA_PRO_KMS_EXPECT_KID", "0000000000000000")
        .output()
        .unwrap();
    assert!(!out.status.success());
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
