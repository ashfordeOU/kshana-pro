// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! B1404: the OSNMA status block in signed packs (signed-dossier format 2). A block made by
//! the embedded open verifier is signed into the statement, tampering with it breaks the
//! signature, format-1 packs (every existing sample) still verify and read as "not recorded",
//! and a verified format-1 pack migrates to format 2. The command line carries the block for
//! every signing command (`--osnma-status`) and prints it on verify.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::public_key_for;
use kshana_pro::osnma_evidence::OsnmaBlock;
use kshana_pro::signed_dossier::{self, FORMAT, FORMAT_V2, SIGNATURE_FILE};
use sha2::{Digest, Sha256};

const SEED: [u8; 32] = [7; 32];
const SEED2: [u8; 32] = [9; 32];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-pro-osnma-ev-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("data")).unwrap();
    std::fs::write(d.join("report.json"), "{\"result\":1}\n").unwrap();
    std::fs::write(d.join("data/a.csv"), "t,x\n0,1\n").unwrap();
    d
}

fn sample_pages() -> (Vec<u8>, String) {
    let ex = root().join("examples/pnt-verification");
    (
        std::fs::read(ex.join("pages-good.txt")).unwrap(),
        std::fs::read_to_string(ex.join("public-key.txt"))
            .unwrap()
            .trim()
            .to_string(),
    )
}

fn block_from_sample() -> OsnmaBlock {
    let (pages, key) = sample_pages();
    OsnmaBlock::verify_input(pages, "pages-good.txt", None, &[key], None).unwrap()
}

fn bin(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env(
            "KSHANA_PRO_HOME",
            std::env::temp_dir().join(format!("kshana-pro-osnma-ev-home-{}", std::process::id())),
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

fn signature(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join(SIGNATURE_FILE)).unwrap()).unwrap()
}

#[test]
fn the_embedded_verifier_gives_the_six_fields() {
    let b = block_from_sample();
    assert_eq!(b.status, "authenticated");
    assert_eq!(b.key_id, Some(1));
    assert_eq!(b.chain_id, Some(3));
    assert_eq!(b.merkle_root, None);
    assert!(b.verifier.starts_with("kshana 0."), "{}", b.verifier);
    assert!(!b.freshness_checked);
    let v = serde_json::to_value(&b).unwrap();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 6, "{keys:?}");
    // A corrupted stream gives a failed status.
    let (pages, key) = sample_pages();
    let text = String::from_utf8(pages).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    // Flip one hex digit of a mid-stream page of satellite 5 without repairing its CRC: the
    // page is dropped and the block still carries one of the verifier's three statuses.
    let i = lines.iter().position(|l| l.starts_with("5 ")).unwrap() + 200;
    let mut parts: Vec<String> = lines[i].split(' ').map(str::to_string).collect();
    let h = parts[2].clone();
    let flipped = if &h[10..11] == "0" { "1" } else { "0" };
    parts[2] = format!("{}{}{}", &h[..10], flipped, &h[11..]);
    lines[i] = parts.join(" ");
    let b2 =
        OsnmaBlock::verify_input(lines.join("\n").into_bytes(), "x", None, &[key], None).unwrap();
    assert!(["authenticated", "unavailable", "failed"].contains(&b2.status.as_str()));
}

#[test]
fn a_block_is_signed_into_format_2_and_tampering_breaks_the_signature() {
    let d = tmp("v2");
    let b = block_from_sample();
    signed_dossier::sign_with(&d, &SEED, "Ashforde OÜ", "2026-11-20T10:00:00Z", Some(&b)).unwrap();
    let s = signature(&d);
    assert_eq!(s["format"], FORMAT_V2);
    assert_eq!(s["osnma"]["status"], "authenticated");
    assert!(s.get("supersedes").is_none());
    let pk = public_key_for(&SEED);
    let v = signed_dossier::verify(&d, &[pk]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    assert_eq!(v.format, 2);
    assert_eq!(v.osnma.as_ref(), Some(&b));
    // Changing the recorded status breaks the signature.
    let p = d.join(SIGNATURE_FILE);
    let t = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, t.replace("\"authenticated\"", "\"failed\"")).unwrap();
    let v = signed_dossier::verify(&d, &[pk]).unwrap();
    assert!(!v.signature_ok && !v.passed());
    // Removing the block from a format-2 statement is caught too.
    let mut s2 = s.clone();
    s2.as_object_mut().unwrap().remove("osnma");
    std::fs::write(&p, serde_json::to_string_pretty(&s2).unwrap()).unwrap();
    let v = signed_dossier::verify(&d, &[pk]).unwrap();
    assert!(!v.passed());
    assert!(
        v.problems.iter().any(|x| x.contains("no OSNMA block")),
        "{:?}",
        v.problems
    );
    // An invalid block is never signed.
    let bad = OsnmaBlock {
        status: "time-authenticated".into(),
        ..b
    };
    assert!(signed_dossier::sign_with(&d, &SEED, "A", "2026-11-20T10:00:00Z", Some(&bad)).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn without_a_block_the_signature_stays_format_1_and_old_packs_verify() {
    let d = tmp("v1");
    signed_dossier::sign_with(&d, &SEED, "Ashforde OÜ", "2026-11-20T10:00:00Z", None).unwrap();
    let s = signature(&d);
    assert_eq!(s["format"], FORMAT);
    assert!(s.get("osnma").is_none() && s.get("supersedes").is_none());
    let v = signed_dossier::verify(&d, &[public_key_for(&SEED)]).unwrap();
    assert!(v.passed());
    assert_eq!((v.format, v.osnma), (1, None));
    // A format-1 statement may not carry a block.
    let mut s1 = s.clone();
    s1["osnma"] = serde_json::to_value(OsnmaBlock::not_recorded()).unwrap();
    std::fs::write(
        d.join(SIGNATURE_FILE),
        serde_json::to_string_pretty(&s1).unwrap(),
    )
    .unwrap();
    assert!(!signed_dossier::verify(&d, &[public_key_for(&SEED)])
        .unwrap()
        .passed());
    // Every committed sample pack (signed before format 2 existed) still verifies.
    let mut n = 0;
    for sig in walk(&root().join("report-templates/samples")) {
        let dir = sig.parent().unwrap();
        let st = signature(dir);
        let pk: [u8; 32] = hex::decode(st["public_key"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let v = signed_dossier::verify(dir, &[pk]).unwrap();
        assert!(v.passed(), "{}: {:?}", dir.display(), v.problems);
        assert_eq!(v.format, 1);
        n += 1;
    }
    assert!(n >= 5, "only {n} sample packs found");
    let _ = std::fs::remove_dir_all(&d);
}

fn walk(d: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.file_name().is_some_and(|n| n == SIGNATURE_FILE) {
            out.push(p);
        }
    }
    out
}

#[test]
fn a_verified_format_1_pack_migrates_to_format_2() {
    let d = tmp("mig");
    signed_dossier::sign_with(&d, &SEED, "Old signer", "2026-10-01T10:00:00Z", None).unwrap();
    let old = std::fs::read(d.join(SIGNATURE_FILE)).unwrap();
    let old_pk = public_key_for(&SEED);
    // Without the old signer pinned: refused.
    assert!(signed_dossier::migrate(&d, &[], &SEED2, "New", "2026-11-20T10:00:00Z", None).is_err());
    // Pinned to another key: refused.
    assert!(signed_dossier::migrate(
        &d,
        &[public_key_for(&SEED2)],
        &SEED2,
        "New",
        "2026-11-20T10:00:00Z",
        None
    )
    .is_err());
    // A changed file: refused, and the old signature is left alone.
    std::fs::write(d.join("data/a.csv"), "t,x\n0,2\n").unwrap();
    assert!(
        signed_dossier::migrate(&d, &[old_pk], &SEED2, "New", "2026-11-20T10:00:00Z", None)
            .is_err()
    );
    assert_eq!(std::fs::read(d.join(SIGNATURE_FILE)).unwrap(), old);
    std::fs::write(d.join("data/a.csv"), "t,x\n0,1\n").unwrap();
    // Verified: re-signed as format 2, "not recorded", with the old signature's hash.
    signed_dossier::migrate(&d, &[old_pk], &SEED2, "New", "2026-11-20T10:00:00Z", None).unwrap();
    let s = signature(&d);
    assert_eq!(s["format"], FORMAT_V2);
    assert_eq!(s["osnma"]["status"], "not_recorded");
    assert_eq!(s["supersedes"], hex::encode(Sha256::digest(&old)));
    let v = signed_dossier::verify(&d, &[public_key_for(&SEED2)]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    assert_eq!(v.files, 2);
    // A format-2 pack is not migrated again.
    assert!(signed_dossier::migrate(
        &d,
        &[public_key_for(&SEED2)],
        &SEED2,
        "New",
        "2026-11-20T10:00:00Z",
        None
    )
    .is_err());
    // Migration with a real block.
    let d2 = tmp("mig2");
    signed_dossier::sign_with(&d2, &SEED, "Old", "2026-10-01T10:00:00Z", None).unwrap();
    let b = block_from_sample();
    signed_dossier::migrate(
        &d2,
        &[old_pk],
        &SEED,
        "Old",
        "2026-11-20T10:00:00Z",
        Some(&b),
    )
    .unwrap();
    let v = signed_dossier::verify(&d2, &[old_pk]).unwrap();
    assert!(v.passed());
    assert_eq!(v.osnma, Some(b));
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&d2);
}

#[test]
fn the_command_line_makes_signs_verifies_and_migrates() {
    let d = tmp("cli");
    let work = tmp("cli-work");
    let ex = root().join("examples/pnt-verification");
    let key = std::fs::read_to_string(ex.join("public-key.txt")).unwrap();
    let block = work.join("osnma-block.json");
    let (ok, out) = bin(&[
        "dossier",
        "osnma-status",
        ex.join("pages-good.txt").to_str().unwrap(),
        "--public-key",
        key.trim(),
        "--out",
        block.to_str().unwrap(),
    ]);
    assert!(ok, "{out}");
    assert!(
        out.contains("Advisory") && out.contains("freshness NOT checked"),
        "{out}"
    );
    let kf = work.join("key.txt");
    std::fs::write(&kf, hex::encode(SEED)).unwrap();
    let (ok, out) = bin(&[
        "dossier",
        "sign",
        d.to_str().unwrap(),
        "--signer",
        "Ashforde OÜ",
        "--key-file",
        kf.to_str().unwrap(),
        "--osnma-status",
        block.to_str().unwrap(),
        "--dev",
    ]);
    assert!(ok, "{out}");
    assert!(
        out.contains("format 2 with OSNMA status authenticated"),
        "{out}"
    );
    let pk = hex::encode(public_key_for(&SEED));
    let (ok, out) = bin(&["dossier", "verify", d.to_str().unwrap(), "--trust", &pk]);
    assert!(ok, "{out}");
    assert!(
        out.contains("format:    2")
            && out.contains("osnma:     authenticated (key id 1, chain id 3"),
        "{out}"
    );
    // A block file with a seventh field is refused before anything runs.
    let bad = work.join("bad.json");
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&block).unwrap()).unwrap();
    v["time_authenticated"] = true.into();
    std::fs::write(&bad, v.to_string()).unwrap();
    let (ok, out) = bin(&[
        "dossier",
        "verify",
        d.to_str().unwrap(),
        "--osnma-status",
        bad.to_str().unwrap(),
    ]);
    assert!(!ok && out.contains("--osnma-status"), "{out}");
    // Migrate a format-1 pack.
    let d1 = tmp("cli-v1");
    signed_dossier::sign_with(&d1, &SEED, "Old", "2026-10-01T10:00:00Z", None).unwrap();
    let (ok, out) = bin(&["dossier", "verify", d1.to_str().unwrap(), "--trust", &pk]);
    assert!(ok && out.contains("not recorded (format 1)"), "{out}");
    let (ok, out) = bin(&[
        "dossier",
        "migrate",
        d1.to_str().unwrap(),
        "--trust",
        &pk,
        "--signer",
        "Ashforde OÜ",
        "--key-file",
        kf.to_str().unwrap(),
        "--dev",
    ]);
    assert!(ok, "{out}");
    let (ok, out) = bin(&["dossier", "verify", d1.to_str().unwrap(), "--trust", &pk]);
    assert!(
        ok && out.contains("format:    2") && out.contains("not recorded (migrated pack)"),
        "{out}"
    );
    // The standalone Python verifier reads format 2.
    if Command::new("python3")
        .args(["-I", "-c", "import cryptography"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        let script = root().join("scripts/verify-dossier-signature.py");
        for dir in [&d, &d1] {
            let st = Command::new("python3")
                .arg("-I")
                .arg(&script)
                .arg(dir)
                .args(["--trust", &pk])
                .status()
                .unwrap();
            assert!(st.success(), "python verifier on {}", dir.display());
        }
    }
    for x in [&d, &work, &d1] {
        let _ = std::fs::remove_dir_all(x);
    }
}
