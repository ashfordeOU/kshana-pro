// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Long-term signatures (E25 B2503): a signed dossier gets a key history and an archive
//! time-stamp chain from two self-made test authorities (tests/fixtures/longterm, made by
//! its gen.sh with OpenSSL; no real authority). Verification holds after the signer's key
//! is rotated and after the authority changes, fails when a renewal is missing at the
//! verification time, and catches every tamper. MODELLED: test authorities and keys only.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{key_id, public_key_for};
use kshana_pro::longterm::{self, KeyHistory, VerifyOptions};

const FIX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/longterm");
/// The test seed the fixture dossier is signed with (see gen.sh). Test only.
const K1: [u8; 32] = [0x15; 32];
const K2: [u8; 32] = [0x16; 32];

fn bin(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env(
            "KSHANA_PRO_HOME",
            std::env::temp_dir().join(format!("kshana-pro-lt-home-{}", std::process::id())),
        )
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

fn fix(p: &str) -> String {
    format!("{FIX}/{p}")
}

fn workdir(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("kshana-pro-lt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let d = root.join("d");
    std::fs::create_dir_all(&d).unwrap();
    for f in ["README.txt", "report.json", "signature.json"] {
        std::fs::copy(fix(&format!("dossier/{f}")), d.join(f)).unwrap();
    }
    std::fs::write(
        root.join("k1.key"),
        format!("private_key={}\n", hex::encode(K1)),
    )
    .unwrap();
    std::fs::write(
        root.join("k2.key"),
        format!("private_key={}\n", hex::encode(K2)),
    )
    .unwrap();
    root
}

fn s(p: &Path) -> String {
    p.display().to_string()
}

/// A dossier with stamp 0 (authority A) and, if `renew`, stamp 1 (authority B).
fn stamped(tag: &str, renew: bool) -> PathBuf {
    let root = workdir(tag);
    let h = root.join("h.json");
    let (ok, out) = bin(&[
        "longterm",
        "key-genesis",
        "--key-file",
        &s(&root.join("k1.key")),
        "--holder",
        "Kshana Test Signer",
        "--valid-from",
        "2026-01-01T00:00:00Z",
        "--out",
        &s(&h),
        "--dev",
    ]);
    assert!(ok, "{out}");
    let d = s(&root.join("d"));
    let (ok, out) = bin(&[
        "longterm",
        "add-token",
        &d,
        "--token",
        &fix("tokens/stamp0-a.tsr"),
        "--tsa-anchor",
        &fix("certs/root-a.crt"),
        "--tsa-name",
        "Test TSA A",
        "--history",
        &s(&h),
        "--dev",
    ]);
    assert!(ok, "{out}");
    if renew {
        let (ok, out) = bin(&[
            "longterm",
            "add-token",
            &d,
            "--token",
            &fix("tokens/stamp1-b.tsr"),
            "--tsa-anchor",
            &fix("certs/root-b.crt"),
            "--tsa-name",
            "Test TSA B",
            "--dev",
        ]);
        assert!(ok, "{out}");
    }
    root
}

fn der(pem: &str) -> Vec<u8> {
    longterm::read_certificates(&std::fs::read(fix(pem)).unwrap())
        .unwrap()
        .remove(0)
}

fn opts(trust: [u8; 32], at: &str) -> VerifyOptions {
    VerifyOptions {
        trust_keys: vec![public_key_for(&trust)],
        tsa_anchors: vec![der("certs/root-a.crt"), der("certs/root-b.crt")],
        tsa_fingerprints: vec![],
        at_unix: kshana_pro::forensics::inputs::parse_utc(at).unwrap() as i64,
    }
}

fn iso(t: i64) -> String {
    kshana_pro::forensics::inputs::iso(t as f64)
}

#[test]
fn renewed_chain_verifies_decades_later_across_an_authority_change() {
    let root = stamped("chain", true);
    let d = root.join("d");
    let v = longterm::verify(&d, &opts(K1, "2045-01-01T00:00:00Z")).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    assert_eq!(v.stamps.len(), 2);
    assert_eq!(v.stamps[0].tsa, "Test TSA A");
    assert_eq!(v.stamps[1].tsa, "Test TSA B");
    // Stamp 1 renewed stamp 0 before A's certificate expired, and B's lasts longer.
    assert!(v.stamps[1].gen_time_unix.unwrap() < v.stamps[0].not_after_unix.unwrap());
    assert!(v.stamps[1].not_after_unix.unwrap() > v.stamps[0].not_after_unix.unwrap());
    // The same through the command line, pinning the roots by fingerprint only.
    let fp = |p: &str| {
        use sha2::Digest;
        hex::encode(sha2::Sha256::digest(der(p)))
    };
    let (ok, out) = bin(&[
        "longterm",
        "verify",
        &s(&d),
        "--trust",
        &hex::encode(public_key_for(&K1)),
        "--tsa-fingerprint",
        &format!("{},{}", fp("certs/root-a.crt"), fp("certs/root-b.crt")),
        "--at",
        "2045-01-01T00:00:00Z",
    ]);
    assert!(ok && out.contains("VERIFIED"), "{out}");
    // Only B's root pinned: stamp 0 has no trusted root, so the chain fails.
    let mut o = opts(K1, "2045-01-01T00:00:00Z");
    o.tsa_anchors.remove(0);
    let v = longterm::verify(&d, &o).unwrap();
    assert!(!v.passed() && !v.stamps[0].pass && v.stamps[1].pass);
    // The signed dossier itself still verifies with the plain dossier verifier:
    // longterm.json is beside the signature, not covered by it.
    let (ok, out) = bin(&[
        "dossier",
        "verify",
        &s(&d),
        "--trust",
        &hex::encode(public_key_for(&K1)),
    ]);
    assert!(ok, "{out}");
}

#[test]
fn a_missing_renewal_shows_at_the_verification_time() {
    let root = stamped("norenew", false);
    let d = root.join("d");
    let v0 = longterm::verify(&d, &opts(K1, "2026-11-25T00:00:00Z")).unwrap();
    // Valid now (A's certificate lasts 40 days from generation) and advising renewal.
    let g = v0.existed_by_unix.unwrap();
    let v = longterm::verify(&d, &opts(K1, &iso(g + 86_400))).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    assert!(v.renew_soon);
    // Decades later the lone stamp's certificates have expired: not verified.
    let v = longterm::verify(&d, &opts(K1, "2045-01-01T00:00:00Z")).unwrap();
    assert!(!v.passed());
    assert!(
        v.problems.iter().any(|p| p.contains("expired")),
        "{:?}",
        v.problems
    );
}

#[test]
fn verification_survives_key_rotation_but_not_a_stamp_after_retirement() {
    let root = stamped("rotate", true);
    let d = root.join("d");
    let g = longterm::verify(&d, &opts(K1, "2045-01-01T00:00:00Z"))
        .unwrap()
        .existed_by_unix
        .unwrap();
    // Scheduled rotation a day after the dossier was stamped, recorded by both keys.
    let h = root.join("h.json");
    let (ok, out) = bin(&[
        "longterm",
        "key-rotate",
        &s(&h),
        "--from-key-file",
        &s(&root.join("k1.key")),
        "--to-key-file",
        &s(&root.join("k2.key")),
        "--effective",
        &iso(g + 86_400),
        "--dev",
    ]);
    assert!(ok, "{out}");
    let (ok, out) = bin(&["longterm", "history", &s(&d), "--history", &s(&h), "--dev"]);
    assert!(ok, "{out}");
    // A verifier who only knows the new key still verifies the old signature.
    let v = longterm::verify(&d, &opts(K2, "2045-01-01T00:00:00Z")).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    // And one who knows only the retired key.
    assert!(longterm::verify(&d, &opts(K1, "2045-01-01T00:00:00Z"))
        .unwrap()
        .passed());
    // An unrelated key reaches nothing.
    assert!(
        !longterm::verify(&d, &opts([0x17; 32], "2045-01-01T00:00:00Z"))
            .unwrap()
            .passed()
    );

    // A compromise dated before the first stamp: the dossier key was no longer to be
    // trusted when the dossier was first stamped, so it is refused.
    let root2 = stamped("compromise", true);
    let d2 = root2.join("d");
    let mut hist = KeyHistory::from_file(&root2.join("h.json")).unwrap();
    hist.rotate(&K1, &K2, &iso(g - 86_400), "compromise")
        .unwrap();
    longterm::update_history(&d2, hist).unwrap();
    let v = longterm::verify(&d2, &opts(K2, "2045-01-01T00:00:00Z")).unwrap();
    assert!(!v.passed());
    assert_eq!(v.key_valid_at_first_stamp, Some(false));
    assert!(
        v.problems.iter().any(|p| p.contains("retired")),
        "{:?}",
        v.problems
    );
    // A history that does not extend the recorded one is refused.
    let other = KeyHistory::genesis(&K2, "Someone", "2026-01-01T00:00:00Z").unwrap();
    assert!(longterm::update_history(&d2, other).is_err());
    assert_eq!(key_id(&public_key_for(&K1)).len(), 16);
}

#[test]
fn tampering_and_wrong_tokens_are_caught() {
    // A token over other data is refused when added.
    let root = workdir("wrong");
    let h = KeyHistory::genesis(&K1, "Kshana Test Signer", "2026-01-01T00:00:00Z").unwrap();
    let e = longterm::add_stamp(
        &root.join("d"),
        Some(h.clone()),
        &std::fs::read(fix("tokens/other-b.tsr")).unwrap(),
        &[],
        &der("certs/root-b.crt"),
        "B",
    )
    .unwrap_err();
    assert!(e.contains("does not stamp"), "{e}");
    // The right token against the wrong root is refused too.
    assert!(longterm::add_stamp(
        &root.join("d"),
        Some(h.clone()),
        &std::fs::read(fix("tokens/stamp0-a.tsr")).unwrap(),
        &[],
        &der("certs/root-b.crt"),
        "A",
    )
    .is_err());
    // Out of order: stamp 1 cannot be first.
    assert!(longterm::add_stamp(
        &root.join("d"),
        Some(h),
        &std::fs::read(fix("tokens/stamp1-b.tsr")).unwrap(),
        &[],
        &der("certs/root-b.crt"),
        "B",
    )
    .is_err());

    let at = "2045-01-01T00:00:00Z";
    // A changed file.
    let root = stamped("tamper-file", true);
    let d = root.join("d");
    std::fs::write(d.join("report.json"), "{\"study\":\"edited\"}\n").unwrap();
    assert!(!longterm::verify(&d, &opts(K1, at)).unwrap().passed());
    // A changed byte in a stored token.
    let root = stamped("tamper-token", true);
    let d = root.join("d");
    let p = d.join("longterm.json");
    let mut rec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    let tok = rec["archive"][1]["token"].as_str().unwrap().to_string();
    let flipped = format!(
        "{}{}",
        &tok[..tok.len() - 2],
        if tok.ends_with("00") { "01" } else { "00" }
    );
    rec["archive"][1]["token"] = serde_json::Value::String(flipped);
    std::fs::write(&p, serde_json::to_string_pretty(&rec).unwrap()).unwrap();
    let v = longterm::verify(&d, &opts(K1, at)).unwrap();
    assert!(!v.passed() && !v.stamps[1].pass);
    // A re-signed dossier (signature.json replaced) no longer matches the record.
    let root = stamped("resign", true);
    let d = root.join("d");
    let (ok, out) = bin(&[
        "dossier",
        "sign",
        &s(&d),
        "--signer",
        "Kshana Test Signer",
        "--key-file",
        &s(&root.join("k1.key")),
        "--utc",
        "2026-11-21T00:00:00Z",
        "--dev",
    ]);
    assert!(ok, "{out}");
    assert!(!longterm::verify(&d, &opts(K1, at)).unwrap().passed());
    // No pinned signer key: not verified.
    let root = stamped("nopin", true);
    let mut o = opts(K1, at);
    o.trust_keys.clear();
    assert!(!longterm::verify(&root.join("d"), &o).unwrap().passed());
}

#[test]
fn adding_stamps_needs_the_licence_feature_and_request_does_not() {
    let root = workdir("gate");
    let d = s(&root.join("d"));
    let req = s(&root.join("r.tsq"));
    let (ok, out) = bin(&["longterm", "request", &d, "--out", &req]);
    assert!(ok, "{out}");
    let r = std::fs::read(&req).unwrap();
    use sha2::Digest;
    let digest: [u8; 32] =
        sha2::Sha256::digest(std::fs::read(fix("dossier/signature.json")).unwrap()).into();
    assert_eq!(r, longterm::timestamp_request(&digest));
    let (ok, out) = bin(&[
        "longterm",
        "add-token",
        &d,
        "--token",
        &fix("tokens/stamp0-a.tsr"),
        "--tsa-anchor",
        &fix("certs/root-a.crt"),
    ]);
    assert!(!ok && out.contains("pro-longterm-signature"), "{out}");
}

#[test]
fn stamp_asks_an_authority_over_http_with_curl() {
    if Command::new("curl").arg("--version").output().is_err() {
        return; // no curl on this machine
    }
    let root = workdir("curl");
    let h = root.join("h.json");
    std::fs::write(
        &h,
        serde_json::to_string(
            &KeyHistory::genesis(&K1, "Kshana Test Signer", "2026-01-01T00:00:00Z").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    // A one-shot local "authority" that answers with the committed token for stamp 0 and
    // records the request it got.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let token = std::fs::read(fix("tokens/stamp0-a.tsr")).unwrap();
    let server = std::thread::spawn(move || {
        use std::io::{Read, Write};
        let (mut c, _) = listener.accept().unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = c.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(i) = text.find("\r\n\r\n") {
                let len: usize = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                if buf.len() >= i + 4 + len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/timestamp-reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            token.len()
        );
        c.write_all(head.as_bytes()).unwrap();
        c.write_all(&token).unwrap();
        String::from_utf8_lossy(&buf).to_string()
    });
    let (ok, out) = bin(&[
        "longterm",
        "stamp",
        &s(&root.join("d")),
        "--tsa-url",
        &format!("http://127.0.0.1:{port}/tsa"),
        "--tsa-anchor",
        &fix("certs/root-a.crt"),
        "--tsa-name",
        "Test TSA A",
        "--history",
        &s(&h),
        "--dev",
    ]);
    let req = server.join().unwrap();
    assert!(ok, "{out}");
    assert!(req.contains("application/timestamp-query"), "{req}");
    let rec = longterm::read_record(&root.join("d")).unwrap().unwrap();
    assert_eq!(rec.archive.len(), 1);
}

#[test]
fn the_python_dossier_verifier_ignores_the_record() {
    let ok = Command::new("python3")
        .args(["-I", "-c", "import cryptography"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return; // the cryptography package is not installed here
    }
    let root = stamped("python", true);
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-dossier-signature.py");
    assert!(Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(root.join("d"))
        .args(["--trust", &hex::encode(public_key_for(&K1))])
        .status()
        .unwrap()
        .success());
}
