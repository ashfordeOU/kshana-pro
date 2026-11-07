// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The air-gapped install bundle (roadmap L7) and the Debian package (L6), built from the
//! test binary: checksums, a bill of materials, a signature verified offline, and an
//! offline install that activates a licence. A changed file stops both the checksum
//! check and the installer.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("offline_bundle-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kshana-pro")
}

#[test]
fn bundle_signs_checks_and_installs_offline() {
    if !cfg!(debug_assertions) || option_env!("KSHANA_PRO_LICENSE_PUBKEY").is_some() {
        return; // signs with --dev and activates a demo-key token: debug builds without a licence key
    }
    let work = tmp("bundle");
    // A bundle signing key.
    let out = Command::new(bin())
        .args(["dossier", "keygen"])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let field = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")))
            .unwrap()
            .to_string()
    };
    std::fs::write(work.join("bundle.key"), field("private_key")).unwrap();
    let public = field("public_key");

    let bundle = work.join("bundle");
    let o = Command::new(root().join("scripts/offline-bundle.sh"))
        .arg(bin())
        .arg(&bundle)
        .args([
            "--sign",
            work.join("bundle.key").to_str().unwrap(),
            "--signer",
            "Ashforde OÜ",
        ])
        .env_remove("KSHANA_PRO_VENDOR_LICENSE")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    for f in [
        "bin/kshana-pro",
        "install.sh",
        "SHA256SUMS",
        "sbom.cdx.json",
        "signature.json",
        "deploy/kshana-pro.service",
        "docs/LICENSING.md",
        "BUNDLE.txt",
    ] {
        assert!(bundle.join(f).exists(), "{f}");
    }
    // The bill of materials names the engine and the TLS stack the binary links.
    let sbom: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(bundle.join("sbom.cdx.json")).unwrap())
            .unwrap();
    assert_eq!(sbom["bomFormat"], "CycloneDX");
    let names: Vec<&str> = sbom["components"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    assert!(
        names.contains(&"kshana") && names.contains(&"rustls") && names.contains(&"ed25519-dalek")
    );
    assert_eq!(sbom["metadata"]["component"]["name"], "kshana-pro");
    // The signature verifies offline.
    let v = Command::new(bin())
        .args([
            "dossier",
            "verify",
            bundle.to_str().unwrap(),
            "--trust",
            &public,
        ])
        .output()
        .unwrap();
    assert!(v.status.success(), "{}", String::from_utf8_lossy(&v.stdout));

    // Offline install into a prefix, activating a (demo-key) token.
    let tok = Command::new(bin())
        .args([
            "license",
            "issue",
            "--tier",
            "pro",
            "--licensee",
            "Offline Site",
            "--days",
            "30",
            "--product",
            "cli",
        ])
        .env_remove("KSHANA_PRO_SIGNING_KEY")
        .output()
        .unwrap();
    std::fs::write(
        work.join("token.txt"),
        String::from_utf8_lossy(&tok.stdout).trim(),
    )
    .unwrap();
    let home = work.join("home");
    let prefix = work.join("prefix");
    let i = Command::new(bundle.join("install.sh"))
        .args([
            "--prefix",
            prefix.to_str().unwrap(),
            "--licence",
            work.join("token.txt").to_str().unwrap(),
        ])
        .env("KSHANA_PRO_HOME", &home)
        .output()
        .unwrap();
    assert!(
        i.status.success(),
        "{}{}",
        String::from_utf8_lossy(&i.stdout),
        String::from_utf8_lossy(&i.stderr)
    );
    let st = Command::new(prefix.join("bin/kshana-pro"))
        .args(["license", "status", "--json"])
        .env("KSHANA_PRO_HOME", &home)
        .output()
        .unwrap();
    let st: serde_json::Value = serde_json::from_slice(&st.stdout).unwrap();
    assert_eq!(st["licensee"], "Offline Site");

    // A changed file: the signature and the installer both refuse.
    std::fs::write(bundle.join("docs/SERVICE.md"), "changed").unwrap();
    let v = Command::new(bin())
        .args([
            "dossier",
            "verify",
            bundle.to_str().unwrap(),
            "--trust",
            &public,
        ])
        .output()
        .unwrap();
    assert!(!v.status.success());
    let i = Command::new(bundle.join("install.sh"))
        .args(["--prefix", work.join("prefix2").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!i.status.success(), "a damaged bundle was installed");
    assert!(!work.join("prefix2/bin/kshana-pro").exists());
}

#[test]
fn debian_package_holds_the_binary_the_unit_and_the_settings() {
    if Command::new("dpkg-deb").arg("--version").output().is_err() {
        eprintln!("skipped: dpkg-deb is not installed (not a Debian-family machine)");
        return;
    }
    let work = tmp("deb");
    let o = Command::new(root().join("scripts/build-deb.sh"))
        .arg(bin())
        .arg(&work)
        .arg("amd64")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let deb = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let list = Command::new("dpkg-deb")
        .args(["-c", &deb])
        .output()
        .unwrap();
    let list = String::from_utf8_lossy(&list.stdout);
    for f in [
        "./usr/bin/kshana-pro",
        "./lib/systemd/system/kshana-pro.service",
        "./etc/kshana-pro/serve.conf",
        "./usr/share/doc/kshana-pro/SERVICE.md",
    ] {
        assert!(list.contains(f), "{f} missing:\n{list}");
    }
    let info = Command::new("dpkg-deb")
        .args(["-I", &deb])
        .output()
        .unwrap();
    let info = String::from_utf8_lossy(&info.stdout);
    assert!(
        info.contains("Package: kshana-pro") && info.contains("Architecture: amd64"),
        "{info}"
    );
    let conf = Command::new("dpkg-deb")
        .args(["-f", &deb, "Version"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&conf.stdout).trim().is_empty());
}
