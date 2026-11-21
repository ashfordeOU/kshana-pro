// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Signed dossier (roadmap F7): sign a delivery directory with `kshana-pro dossier
//! sign`, verify it offline with `kshana-pro dossier verify` and with the standalone
//! Python verifier, and see a changed file refused by both.

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env(
            "KSHANA_PRO_HOME",
            std::env::temp_dir().join(format!("kshana-pro-sd-home-{}", std::process::id())),
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

fn python_verify(dir: &Path, key: &str) -> Option<bool> {
    let ok = Command::new("python3")
        .args(["-I", "-c", "import cryptography"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        return None; // the cryptography package is not installed here
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-dossier-signature.py");
    Some(
        Command::new("python3")
            .arg("-I")
            .arg(script)
            .arg(dir)
            .args(["--trust", key])
            .status()
            .unwrap()
            .success(),
    )
}

#[test]
fn sign_and_verify_a_dossier_offline() {
    let root: PathBuf = std::env::temp_dir().join(format!(
        "kshana-pro-signed-dossier-it-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join("dossier");
    std::fs::create_dir_all(dir.join("runs")).unwrap();
    std::fs::write(
        dir.join("dossier.json"),
        "{\"verdict\":\"MODELLED\",\"note\":\"Ünïcode ✓\"}\n",
    )
    .unwrap();
    std::fs::write(dir.join("runs/RUN-1.json"), "{\"x\":1}\n").unwrap();

    let (ok, out) = bin(&["dossier", "keygen"]);
    assert!(ok, "{out}");
    let field = |k: &str| {
        out.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .unwrap()
            .to_string()
    };
    let (seed, public) = (field("private_key"), field("public_key"));
    let kf = root.join("dossier-key");
    std::fs::write(&kf, &seed).unwrap();

    // Signing is Pro: refused at Evaluation.
    let (ok, out) = bin(&[
        "dossier",
        "sign",
        dir.to_str().unwrap(),
        "--signer",
        "Ashforde OÜ",
        "--key-file",
        kf.to_str().unwrap(),
    ]);
    assert!(!ok, "{out}");
    let (ok, out) = bin(&[
        "dossier",
        "sign",
        dir.to_str().unwrap(),
        "--signer",
        "Ashforde OÜ",
        "--key-file",
        kf.to_str().unwrap(),
        "--utc",
        "2026-11-07T10:00:00Z",
        "--dev",
    ]);
    assert!(ok, "{out}");

    let (ok, out) = bin(&[
        "dossier",
        "verify",
        dir.to_str().unwrap(),
        "--trust",
        &public,
    ]);
    assert!(ok && out.contains("VERIFIED"), "{out}");
    // Not pinned: not verified (the key must come from the signer by another route).
    let (ok, _) = bin(&["dossier", "verify", dir.to_str().unwrap()]);
    assert!(!ok);
    if let Some(py) = python_verify(&dir, &public) {
        assert!(py, "the Python verifier disagrees with kshana-pro");
    }

    std::fs::write(dir.join("runs/RUN-1.json"), "{\"x\":2}\n").unwrap();
    let (ok, out) = bin(&[
        "dossier",
        "verify",
        dir.to_str().unwrap(),
        "--trust",
        &public,
    ]);
    assert!(!ok && out.contains("runs/RUN-1.json: changed"), "{out}");
    if let Some(py) = python_verify(&dir, &public) {
        assert!(!py, "the Python verifier accepted a changed file");
    }
}

/// Product certificates against a signed revocation list, and the revalidation calendar
/// (`kshana-pro cert`, docs/CERT-STATUS.md). The list signer here is a test fixture only: lists
/// are issued by the hosted service, never by Pro.
mod cert_status {
    use ed25519_dalek::{Signer, SigningKey};
    use kshana_pro::cert_status::{self as cs, RevocationList, Revoked};
    use kshana_pro::license::{key_id, public_key_for};

    const LAB: [u8; 32] = [31; 32];
    const ISSUER: [u8; 32] = [32; 32];

    /// Test fixture: sign a list as the hosted issuer would (`payload_hex.signature_hex`).
    fn sign_list(seed: &[u8; 32], revoked: Vec<Revoked>, next_update: &str) -> String {
        let l = RevocationList {
            format: cs::LIST_FORMAT.into(),
            issuer: "SYNTHETIC issuer".into(),
            sequence: 7,
            issued_utc: "2027-01-01T00:00:00Z".into(),
            next_update_utc: next_update.into(),
            revoked,
            kid: key_id(&public_key_for(seed)),
        };
        let payload = serde_json::to_vec(&l).unwrap();
        let sig = SigningKey::from_bytes(seed).sign(&payload);
        format!("{}.{}", hex::encode(&payload), hex::encode(sig.to_bytes()))
    }

    fn cert(root: &std::path::Path, id: &str, issued: &str) -> std::path::PathBuf {
        let c = cs::certificate(
            id,
            "SYNTHETIC receiver X1",
            "leo-acquisition",
            "1.0",
            issued,
            None,
            "SYNTHETIC lab",
            "",
        )
        .unwrap();
        let d = root.join(id);
        cs::write_certificate(&d, &c).unwrap();
        kshana_pro::signed_dossier::sign(&d, &LAB, "SYNTHETIC lab", issued).unwrap();
        d
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("kshana-cert-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const NOW: &str = "2027-01-15T00:00:00Z";

    #[test]
    fn revoked_certificate_shows_as_revoked() {
        let root = tmp("revoked");
        let a = cert(&root, "CERT-A", "2026-06-01");
        let b = cert(&root, "CERT-B", "2026-06-01");
        let list = sign_list(
            &ISSUER,
            vec![Revoked {
                id: "CERT-A".into(),
                reason: "SYNTHETIC: method error found".into(),
                revoked_utc: "2027-01-10T00:00:00Z".into(),
            }],
            "2027-02-01T00:00:00Z",
        );
        let lab = [public_key_for(&LAB)];
        let iss = [public_key_for(&ISSUER)];
        let sa = cs::status(&a, Some(&list), &lab, &iss, NOW).unwrap();
        assert_eq!(sa.state, "revoked", "{sa:?}");
        assert!(sa.reason.contains("method error"));
        assert_eq!(sa.revocation.unwrap().revoked_utc, "2027-01-10T00:00:00Z");
        let sb = cs::status(&b, Some(&list), &lab, &iss, NOW).unwrap();
        assert_eq!(sb.state, "valid", "{sb:?}");
        assert_eq!(
            sb.certificate.unwrap().revalidation_due_utc,
            "2027-06-01T00:00:00Z"
        );
        // Before the revocation date the certificate was not yet revoked.
        let early = cs::status(&a, Some(&list), &lab, &iss, "2027-01-05T00:00:00Z").unwrap();
        assert_eq!(early.state, "valid");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_stale_list_does_not_report_valid() {
        let root = tmp("stale");
        let a = cert(&root, "CERT-A", "2026-06-01");
        let b = cert(&root, "CERT-B", "2026-06-01");
        let list = sign_list(
            &ISSUER,
            vec![Revoked {
                id: "CERT-B".into(),
                reason: "SYNTHETIC withdrawn".into(),
                revoked_utc: "2026-12-01T00:00:00Z".into(),
            }],
            "2027-01-08T00:00:00Z",
        );
        let iss = [public_key_for(&ISSUER)];
        let sa = cs::status(&a, Some(&list), &[], &iss, NOW).unwrap();
        assert_eq!(sa.state, "unknown", "{sa:?}");
        assert!(sa.list.unwrap().stale);
        assert!(sa.reason.contains("past its next update"));
        // A revocation on a stale list still counts.
        assert_eq!(
            cs::status(&b, Some(&list), &[], &iss, NOW).unwrap().state,
            "revoked"
        );
        // No list, a list from an untrusted key, a tampered list: never valid.
        assert_eq!(
            cs::status(&a, None, &[], &iss, NOW).unwrap().state,
            "unknown"
        );
        let fresh = sign_list(&ISSUER, vec![], "2027-02-01T00:00:00Z");
        let other = [public_key_for(&[33; 32])];
        let s = cs::status(&a, Some(&fresh), &[], &other, NOW).unwrap();
        assert_eq!(s.state, "unknown");
        assert!(s.reason.contains("does not verify"), "{}", s.reason);
        let (p, sig) = fresh.split_once('.').unwrap();
        let mut pb = hex::decode(p).unwrap();
        let i = pb.iter().position(|&c| c == b'7').unwrap();
        pb[i] = b'8';
        let forged = format!("{}.{sig}", hex::encode(pb));
        assert_eq!(
            cs::status(&a, Some(&forged), &[], &iss, NOW).unwrap().state,
            "unknown"
        );
        assert_eq!(
            cs::status(&a, Some(&fresh), &[], &iss, NOW).unwrap().state,
            "valid"
        );
        assert!(cs::verify_list(&fresh, &[]).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn overdue_and_invalid_certificates() {
        let root = tmp("overdue");
        let old = cert(&root, "CERT-OLD", "2025-12-01");
        let list = sign_list(&ISSUER, vec![], "2027-02-01T00:00:00Z");
        let iss = [public_key_for(&ISSUER)];
        let s = cs::status(&old, Some(&list), &[], &iss, NOW).unwrap();
        assert_eq!(s.state, "revalidation-overdue", "{s:?}");
        // Overdue is known without a list.
        assert_eq!(
            cs::status(&old, None, &[], &iss, NOW).unwrap().state,
            "revalidation-overdue"
        );
        // A certificate from another lab key, or edited after signing, is invalid.
        let s = cs::status(&old, Some(&list), &[public_key_for(&[34; 32])], &iss, NOW).unwrap();
        assert_eq!(s.state, "invalid");
        let p = old.join(cs::CERT_FILE);
        let t = std::fs::read_to_string(&p)
            .unwrap()
            .replace("2026-12-01", "2027-12-01");
        std::fs::write(&p, t).unwrap();
        let s = cs::status(&old, Some(&list), &[], &iss, NOW).unwrap();
        assert_eq!(s.state, "invalid");
        assert!(
            s.certificate_signature
                .contains("certificate.json: changed"),
            "{s:?}"
        );
        assert!(cs::certificate(
            "x",
            "p",
            "m",
            "1",
            "2026-01-01",
            Some("2025-01-01"),
            "i",
            ""
        )
        .is_err());
        // 29 February issue: due 28 February the next year.
        let c = cs::certificate("x", "p", "m", "1", "2028-02-29", None, "i", "").unwrap();
        assert_eq!(c.revalidation_due_utc, "2029-02-28T00:00:00Z");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn revalidation_calendar_lists_due_and_overdue() {
        let root = tmp("calendar");
        let a = cert(&root, "CERT-A", "2026-02-01"); // due 2027-02-01: due soon
        let b = cert(&root, "CERT-B", "2025-12-20"); // due 2026-12-20: overdue
        let c = cert(&root, "CERT-C", "2026-10-01"); // due 2027-10-01: scheduled
        let d = cert(&root, "CERT-D", "2026-10-01"); // revoked
        let list = sign_list(
            &ISSUER,
            vec![Revoked {
                id: "CERT-D".into(),
                reason: "SYNTHETIC".into(),
                revoked_utc: "2027-01-02T00:00:00Z".into(),
            }],
            "2027-02-01T00:00:00Z",
        );
        let iss = [public_key_for(&ISSUER)];
        let cal = cs::schedule(
            &[a.as_path(), b.as_path(), c.as_path(), d.as_path()],
            Some(&list),
            &[public_key_for(&LAB)],
            &iss,
            NOW,
            60.0,
        )
        .unwrap();
        let flags: Vec<(&str, &str)> = cal
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.flag.as_str()))
            .collect();
        assert_eq!(
            flags,
            vec![
                ("CERT-B", "OVERDUE"),
                ("CERT-A", "DUE SOON"),
                ("CERT-C", "SCHEDULED"),
                ("CERT-D", "REVOKED"),
            ]
        );
        assert!(cal
            .verdict
            .starts_with("4 certificates: 1 overdue, 1 due within 60 days"));
        let ics = cs::calendar_ics(&cal);
        assert_eq!(ics.matches("BEGIN:VEVENT").count(), 3);
        assert!(ics.contains("DTSTART;VALUE=DATE:20270201"));
        assert!(cs::calendar_csv(&cal).lines().count() == 5);
        assert!(cs::schedule(
            &[a.as_path(), a.as_path()],
            Some(&list),
            &[],
            &iss,
            NOW,
            60.0
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
