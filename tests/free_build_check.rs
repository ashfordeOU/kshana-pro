// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! F10e: `scripts/free-build-check.sh` must find the Pro markers in a Pro binary (so a
//! free build that passes it really carries none of them) and pass a file without them.

use std::path::Path;
use std::process::Command;

fn script() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/free-build-check.sh")
}

#[test]
fn the_check_finds_pro_code_in_a_pro_binary() {
    let out = Command::new(script())
        .arg(env!("CARGO_BIN_EXE_kshana-pro"))
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "the Pro binary passed the free-build check"
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("kshana_pro"));
}

#[test]
fn the_check_passes_a_file_without_pro_code() {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("free_build_check-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let f = d.join("free.bin");
    std::fs::write(&f, b"kshana engine 0.40 open build\0run_toml_lang\0").unwrap();
    let out = Command::new(script()).arg(&f).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}
