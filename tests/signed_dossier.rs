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

/// Fleet revalidation plan over open product registers (`kshana-pro revalidation plan`,
/// docs/REVALIDATION-PLAN.md). Registers and lists are signed here by test fixtures only: the
/// publisher signs registers with the open `kshana register sign`, the issuer signs lists.
mod revalidation_plan {
    use ed25519_dalek::{Signer, SigningKey};
    use kshana_pro::revalidation::{self as rv, Crl, Entry, Lead, Register, Request, Revoked};
    use sha2::{Digest, Sha256};

    const PUB: [u8; 32] = [41; 32];
    const ISS: [u8; 32] = [42; 32];

    fn pk(seed: &[u8; 32]) -> String {
        hex::encode(SigningKey::from_bytes(seed).verifying_key().to_bytes())
    }

    fn sign_register(mut r: Register, seed: &[u8; 32]) -> String {
        r.signer = pk(seed);
        r.signature.clear();
        let sig = SigningKey::from_bytes(seed).sign(&rv::signing_bytes(rv::REGISTER_SCHEMA, &r));
        r.signature = hex::encode(sig.to_bytes());
        serde_json::to_string_pretty(&r).unwrap()
    }

    fn sign_crl(mut c: Crl, seed: &[u8; 32]) -> String {
        c.signer = pk(seed);
        c.signature.clear();
        let sig = SigningKey::from_bytes(seed).sign(&rv::signing_bytes(rv::CRL_SCHEMA, &c));
        c.signature = hex::encode(sig.to_bytes());
        serde_json::to_string(&c).unwrap()
    }

    fn entry(id: &str, maker: &str, scored: &str, by: &str) -> Entry {
        Entry {
            id: id.into(),
            product: format!("SYNTHETIC {id}"),
            maker: maker.into(),
            version: "1.0".into(),
            method: "leo-acquisition".into(),
            method_version: "1.0".into(),
            scored: scored.into(),
            revalidate_by: by.into(),
            outcome: "band B".into(),
            evidence_sha256: None,
            evidence_url: None,
            scenario_set: None,
            scenario_set_version: None,
        }
    }

    fn register(publisher: &str, entries: Vec<Entry>) -> Register {
        Register {
            schema: rv::REGISTER_SCHEMA.into(),
            publisher: publisher.into(),
            issued: "2026-11-01".into(),
            entries,
            signer: String::new(),
            signature: String::new(),
        }
    }

    fn crl(next_update: &str, revoked: Vec<Revoked>) -> Crl {
        Crl {
            schema: rv::CRL_SCHEMA.into(),
            issuer: "SYNTHETIC issuer".into(),
            issued: "2026-11-15".into(),
            next_update: next_update.into(),
            revoked,
            signer: String::new(),
            signature: String::new(),
        }
    }

    #[test]
    fn the_published_crl_test_vector_verifies() {
        let seed: [u8; 32] = Sha256::digest(b"register crl test vector").into();
        let key = "a028f6212dcbd196a38fe8e13927a9d997f4d72ab00341276b9437d8f2273a8d";
        assert_eq!(pk(&seed), key);
        let doc = format!(
            r#"{{"schema":"kshana.register-crl/1","issuer":"Example issuer (test vector)","issued":"2026-11-20","next_update":"2026-11-27","revoked":[{{"id":"rx-007","revoked":"2026-11-19","reason":"score withdrawn"}},{{"id":"ant-12.v2","revoked":"2026-11-20","reason":"method superseded"}}],"signer":"{key}","signature":"d62b00f7ca67821e0bca98f6366df1cc44989810c4e678b1f32b82420f3427eb80df766fe72d24ad5e44bcb710bbaf0ac431de47f3cd8b79299c636991099107"}}"#
        );
        let c = rv::check_crl(&doc, key).unwrap();
        assert_eq!(c.revoked.len(), 2);
        assert_eq!(c.revoked[1].id, "ant-12.v2");
        // Another pinned key, or a changed reason, does not verify.
        assert!(rv::check_crl(&doc, &pk(&ISS)).is_err());
        assert!(rv::check_crl(&doc.replace("score withdrawn", "score withdrawn."), key).is_err());
        assert!(rv::check_crl(&doc, "").is_err());
    }

    #[test]
    fn plan_flags_by_month_and_maker_with_lead_times() {
        let reg_a = sign_register(
            register(
                "SYNTHETIC lab A",
                vec![
                    entry("rx-1", "Maker One", "2025-11-01", "2026-11-10"), // overdue
                    entry("rx-2", "Maker One", "2025-12-15", "2026-12-15"), // due soon
                    entry("rx-3", "Maker Two", "2026-06-01", "2027-06-01"), // scheduled
                    entry("rx-4", "Maker Two", "2026-06-01", "2027-06-01"), // revoked
                ],
            ),
            &PUB,
        );
        let reg_b = sign_register(
            register(
                "SYNTHETIC lab B",
                vec![entry("ant-1", "Maker One", "2026-03-01", "2027-03-01")],
            ),
            &PUB,
        );
        let list = sign_crl(
            crl(
                "2026-12-15",
                vec![Revoked {
                    id: "rx-4".into(),
                    revoked: "2026-11-18".into(),
                    reason: "score withdrawn".into(),
                }],
            ),
            &ISS,
        );
        let a = rv::check_register(&reg_a, Some(&pk(&PUB))).unwrap();
        let b = rv::check_register(&reg_b, None).unwrap();
        let c = rv::check_crl(&list, &pk(&ISS)).unwrap();
        let q = Request {
            title: "SYNTHETIC fleet".into(),
            warn_days: 60,
            lead_days: 90,
            lead: vec![Lead {
                maker: "Maker Two".into(),
                days: 120,
            }],
            registers: vec![],
        };
        let p = rv::plan(
            &q,
            &[("a.json".into(), a, Some(c)), ("b.json".into(), b, None)],
            "2026-11-21",
        )
        .unwrap();
        let f: Vec<(&str, &str, &str)> = p
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.flag.as_str(), i.status.as_str()))
            .collect();
        assert_eq!(
            f,
            vec![
                ("rx-1", "OVERDUE", "due"),
                ("rx-2", "DUE SOON", "current"),
                ("ant-1", "SCHEDULED", "current"),
                ("rx-3", "SCHEDULED", "current"),
                ("rx-4", "REVOKED", "revoked"),
            ]
        );
        let rx3 = p.items.iter().find(|i| i.id == "rx-3").unwrap();
        assert_eq!((rx3.lead_days, rx3.start_by.as_str()), (120, "2027-02-01"));
        let rx2 = p.items.iter().find(|i| i.id == "rx-2").unwrap();
        assert_eq!((rx2.start_by.as_str(), rx2.days_left), ("2026-09-16", 24));
        assert_eq!(
            p.items.iter().find(|i| i.id == "ant-1").unwrap().revocation,
            "unknown"
        );
        assert_eq!(p.by_month["2027-06"]["SCHEDULED"], 1);
        assert_eq!(p.by_month["2027-06"]["REVOKED"], 1);
        let one = p.makers.iter().find(|m| m.maker == "Maker One").unwrap();
        assert_eq!(
            (one.entries, one.overdue, one.due_soon, one.scheduled),
            (3, 1, 1, 1)
        );
        assert_eq!(one.next_revalidate_by.as_deref(), Some("2026-11-10"));
        assert!(p.verdict.contains("revocation unchecked for 1 registers"));
        let ics = rv::ics(&p);
        assert_eq!(ics.matches("BEGIN:VEVENT").count(), 8);
        assert!(ics.contains("DTSTART;VALUE=DATE:20270201"));
        assert_eq!(rv::csv(&p).lines().count(), 6);
    }

    #[test]
    fn stale_list_revocations_still_apply_and_say_stale() {
        let reg = sign_register(
            register(
                "SYNTHETIC lab A",
                vec![
                    entry("rx-1", "M", "2026-06-01", "2027-06-01"),
                    entry("rx-2", "M", "2026-06-01", "2027-06-01"),
                ],
            ),
            &PUB,
        );
        let list = sign_crl(
            crl(
                "2026-11-20",
                vec![
                    Revoked {
                        id: "rx-1".into(),
                        revoked: "2026-11-16".into(),
                        reason: "method superseded".into(),
                    },
                    Revoked {
                        id: "rx-2".into(),
                        revoked: "2026-12-01".into(),
                        reason: "later".into(),
                    },
                    Revoked {
                        id: "other-9".into(),
                        revoked: "2026-11-01".into(),
                        reason: "x".into(),
                    },
                ],
            ),
            &ISS,
        );
        let r = rv::check_register(&reg, None).unwrap();
        let c = rv::check_crl(&list, &pk(&ISS)).unwrap();
        let q: Request = toml::from_str("title = \"t\"\n[[register]]\nfile = \"x\"\n").unwrap();
        let p = rv::plan(&q, &[("x".into(), r, Some(c))], "2026-11-21").unwrap();
        assert_eq!(p.registers[0].revocation, "stale");
        assert_eq!(p.registers[0].crl_unmatched, vec!["other-9".to_string()]);
        let get = |id: &str| p.items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(get("rx-1").status, "revoked");
        assert_eq!(get("rx-1").revocation, "stale");
        // A revocation dated after the plan date does not apply yet.
        assert_eq!(get("rx-2").status, "current");
        assert!(p.verdict.contains("list stale for 1"));
    }

    #[test]
    fn signatures_dates_and_files_are_checked() {
        let good = sign_register(
            register(
                "SYNTHETIC",
                vec![entry("rx-1", "M", "2026-06-01", "2027-06-01")],
            ),
            &PUB,
        );
        assert!(rv::check_register(&good, Some(&pk(&ISS))).is_err());
        assert!(rv::check_register(&good.replace("band B", "band A"), None).is_err());
        let bad_date = sign_register(
            register(
                "SYNTHETIC",
                vec![entry("rx-1", "M", "2026-06-01", "2026-02-30")],
            ),
            &PUB,
        );
        assert!(rv::check_register(&bad_date, None).is_err());
        let backwards = sign_register(
            register(
                "SYNTHETIC",
                vec![entry("rx-1", "M", "2026-06-01", "2026-05-01")],
            ),
            &PUB,
        );
        assert!(rv::check_register(&backwards, None).is_err());
        // Through files: a crl without its issuer key is refused.
        let d = std::env::temp_dir().join(format!("kshana-reval-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("r.json"), &good).unwrap();
        std::fs::write(d.join("c.json"), sign_crl(crl("2026-12-31", vec![]), &ISS)).unwrap();
        let q: Request =
            toml::from_str("title = \"t\"\n[[register]]\nfile = \"r.json\"\ncrl = \"c.json\"\n")
                .unwrap();
        assert!(rv::run(&q, &d, "2026-11-21")
            .unwrap_err()
            .contains("crl_key"));
        let q: Request = toml::from_str(&format!(
            "title = \"t\"\n[[register]]\nfile = \"r.json\"\nkey = \"{}\"\ncrl = \"c.json\"\ncrl_key = \"{}\"\n",
            pk(&PUB),
            pk(&ISS)
        ))
        .unwrap();
        let (p, inputs) = rv::run(&q, &d, "2026-11-21").unwrap();
        assert_eq!(p.registers[0].revocation, "checked");
        assert_eq!(inputs.len(), 2);
        assert!(rv::run(&q, &d, "2026-13-01").is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
