// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Signed observation provenance (E25 B2501): a feeder signs its `kshana.obs/1` batches with
//! `kshana-pro obs-provenance sign`; `verify` accepts them against the feeder keyring, rejects
//! a tampered batch, keeps unsigned batches readable (refused only with --require-signed) and
//! checks each feeder's sequence. The batches stay valid `kshana.obs/1` for the open engine.

use std::path::{Path, PathBuf};
use std::process::Command;

const SEED: [u8; 32] = [0x31; 32];

fn bin(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env(
            "KSHANA_PRO_HOME",
            std::env::temp_dir().join(format!("kshana-pro-op-home-{}", std::process::id())),
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

fn batch(n: usize, offset: f64) -> String {
    let station = kshana::obs::Station {
        id: "TLL-01".into(),
        lat_deg: Some(59.4370),
        lon_deg: Some(24.7536),
        height_m: Some(35.0),
        operator: Some("test network".into()),
        ..Default::default()
    };
    let tl = kshana::receiver_trust::Timeline {
        epochs: (0..n)
            .map(|i| kshana::receiver_trust::LogEpoch {
                t_s: offset + i as f64,
                jam_ind: Some(10.0 + i as f64),
                ..Default::default()
            })
            .collect(),
        start_label: Some("2026-11-20T08:00:00Z".into()),
        observables: vec!["jam_ind".into()],
        skipped_records: 0,
    };
    kshana::obs::to_jsonl(&kshana::obs::from_timeline(&tl, &station, Some("ubx"))).unwrap()
}

fn s(p: &Path) -> String {
    p.display().to_string()
}

fn setup(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("kshana-pro-op-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("batches")).unwrap();
    std::fs::write(
        root.join("feeder.key"),
        format!("private_key={}\n", hex::encode(SEED)),
    )
    .unwrap();
    let pk = hex::encode(kshana_pro::license::public_key_for(&SEED));
    let (ok, out) = bin(&[
        "obs-provenance",
        "keyring-add",
        &s(&root.join("keyring.json")),
        "--feeder",
        "feeder-tll",
        "--public-key",
        &pk,
        "--valid-from",
        "2026-01-01T00:00:00Z",
    ]);
    assert!(ok, "{out}");
    for i in 0..3 {
        std::fs::write(
            root.join(format!("batches/b{i}.jsonl")),
            batch(4, 4.0 * i as f64),
        )
        .unwrap();
    }
    root
}

fn sign(root: &Path, i: usize, extra: &[&str]) -> (bool, String) {
    let b = s(&root.join(format!("batches/b{i}.jsonl")));
    let seq = i.to_string();
    let mut args = vec![
        "obs-provenance",
        "sign",
        &b,
        "--feeder",
        "feeder-tll",
        "--key-file",
    ];
    let kf = s(&root.join("feeder.key"));
    args.push(&kf);
    args.extend(["--seq", &seq, "--utc", "2026-11-20T08:05:00Z", "--dev"]);
    let prev = (i > 0).then(|| s(&root.join(format!("batches/b{}.jsonl", i - 1))));
    if let Some(p) = &prev {
        args.extend(["--prev", p]);
    }
    args.extend(extra);
    bin(&args)
}

fn verify(root: &Path, extra: &[&str]) -> (bool, String) {
    let d = s(&root.join("batches"));
    let k = s(&root.join("keyring.json"));
    let mut args = vec!["obs-provenance", "verify", &d, "--keyring", &k];
    args.extend(extra);
    bin(&args)
}

#[test]
fn a_signed_sequence_verifies_and_a_tampered_batch_is_rejected() {
    let root = setup("seq");
    for i in 0..3 {
        let (ok, out) = sign(&root, i, &[]);
        assert!(ok, "{out}");
    }
    let (ok, out) = verify(&root, &["--require-signed"]);
    assert!(ok, "{out}");
    assert_eq!(out.matches(": SIGNED").count(), 3, "{out}");
    // The batch file itself is untouched and still valid kshana.obs/1.
    let text = std::fs::read_to_string(root.join("batches/b1.jsonl")).unwrap();
    assert_eq!(text, batch(4, 4.0));
    assert!(kshana::obs::check(&text).is_ok());
    // One changed reading in one batch.
    std::fs::write(
        root.join("batches/b1.jsonl"),
        text.replacen("\"jam_ind\":11.0", "\"jam_ind\":12.0", 1),
    )
    .unwrap();
    let (ok, out) = verify(&root, &[]);
    assert!(!ok && out.contains("REJECTED"), "{out}");
    // JSON output names the problem.
    let (_, out) = verify(&root, &["--json"]);
    let j: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(j["passed"], false);
    assert_eq!(j["batches"][1]["status"], "rejected");
}

#[test]
fn unsigned_batches_are_readable_unless_signatures_are_required() {
    let root = setup("unsigned");
    let (ok, out) = sign(&root, 0, &[]);
    assert!(ok, "{out}");
    // b1 and b2 unsigned; only b0 signed.
    std::fs::remove_file(root.join("batches/b2.jsonl")).unwrap();
    let (ok, out) = verify(&root, &[]);
    assert!(ok, "{out}");
    assert!(out.contains("UNSIGNED"), "{out}");
    let (ok, out) = verify(&root, &["--require-signed"]);
    assert!(!ok, "{out}");
}

#[test]
fn a_missing_batch_breaks_the_sequence() {
    let root = setup("gap");
    for i in 0..3 {
        assert!(sign(&root, i, &[]).0);
    }
    std::fs::remove_file(root.join("batches/b1.jsonl")).unwrap();
    std::fs::remove_file(root.join("batches/b1.jsonl.prov.json")).unwrap();
    let (ok, out) = verify(&root, &[]);
    assert!(!ok && out.contains("missing batches"), "{out}");
}

#[test]
fn signing_needs_the_feature_and_a_header_position_cannot_be_overridden() {
    let root = setup("gate");
    let b = s(&root.join("batches/b0.jsonl"));
    let kf = s(&root.join("feeder.key"));
    let (ok, out) = bin(&[
        "obs-provenance",
        "sign",
        &b,
        "--feeder",
        "feeder-tll",
        "--key-file",
        &kf,
    ]);
    assert!(!ok && out.contains("pro-obs-provenance"), "{out}");
    let (ok, out) = sign(&root, 0, &["--lat", "10", "--lon", "10"]);
    assert!(!ok && out.contains("header"), "{out}");
    // A key not in the keyring.
    std::fs::write(
        root.join("feeder.key"),
        format!("private_key={}\n", hex::encode([0x32u8; 32])),
    )
    .unwrap();
    assert!(sign(&root, 0, &[]).0);
    let (ok, out) = verify(&root, &[]);
    assert!(!ok && out.contains("not a key of feeder"), "{out}");
}
