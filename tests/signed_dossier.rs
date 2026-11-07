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
