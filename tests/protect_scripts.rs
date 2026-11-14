// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The protection scripts (roadmap 0.42 X4 and X5) checked on real inputs:
//!
//! * `scripts/protect/symbol-check.sh` passes on the binary of a default build and FAILS on
//!   the binary of a build with the `arksa` feature, so the term list keeps matching what
//!   the Arksa-exclusive code puts in a binary;
//! * `scripts/protect/open-leak-gate.sh` fails on a repository that carries a private term
//!   and passes on one that does not, and never prints the term itself.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(script: &str, args: &[&str]) -> (bool, String) {
    let out = Command::new("bash")
        .arg(root().join(script))
        .args(args)
        .output()
        .expect("bash runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

/// The terms of `section` in the private list.
fn terms(section: &str) -> Vec<String> {
    let text = std::fs::read_to_string(root().join("protect/private-terms.txt")).unwrap();
    let mut on = false;
    let mut v = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if l.starts_with('[') && l.ends_with(']') {
            on = &l[1..l.len() - 1] == section;
            continue;
        }
        if on {
            v.push(line.to_string());
        }
    }
    v
}

#[test]
fn symbol_check_matches_the_build() {
    let bin = env!("CARGO_BIN_EXE_kshana-pro");
    let (ok, out) = run("scripts/protect/symbol-check.sh", &[bin]);
    if cfg!(feature = "arksa") {
        assert!(
            !ok,
            "a build with the arksa feature must fail the check, or the term list no longer matches the code:\n{out}"
        );
        assert!(out.contains("FAIL"), "{out}");
    } else {
        assert!(ok, "the default build carries a private term:\n{out}");
    }
    // Never prints a term.
    for t in terms("arksa").iter().chain(&terms("core")) {
        assert!(
            !out.contains(t.as_str()),
            "the check printed a private term"
        );
    }
}

#[test]
fn symbol_check_fails_on_a_planted_term_and_names_only_its_id() {
    let dir = tempdir("symbol-check");
    let planted = &terms("core")[0];
    std::fs::write(dir.join("a.bin"), format!("\0\0xx{planted}yy\0")).unwrap();
    std::fs::write(dir.join("b.bin"), b"\0clean\0").unwrap();
    let (ok, out) = run("scripts/protect/symbol-check.sh", &[dir.to_str().unwrap()]);
    assert!(!ok, "{out}");
    assert!(
        out.contains("a.bin") && !out.contains("b.bin carries"),
        "{out}"
    );
    assert!(
        !out.contains(planted.as_str()),
        "the check printed the term"
    );
    let (ok, out) = run(
        "scripts/protect/symbol-check.sh",
        &[dir.join("b.bin").to_str().unwrap()],
    );
    assert!(ok, "{out}");
}

fn tempdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "kshana-pro-protect-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) {
    let st = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .status()
        .expect("git runs");
    assert!(st.success(), "git {args:?}");
}

#[test]
fn leak_gate_flags_a_new_private_term_and_never_prints_it() {
    let repo = tempdir("leak-gate");
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("README.md"), "an open readme\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "init"]);
    let empty_baseline = repo.join("..").join(format!(
        "{}-baseline.txt",
        repo.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&empty_baseline, "").unwrap();
    let args = |b: &Path| {
        vec![
            "--dir".to_string(),
            repo.to_string_lossy().to_string(),
            "--baseline".to_string(),
            b.to_string_lossy().to_string(),
        ]
    };
    let a = args(&empty_baseline);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let (ok, out) = run("scripts/protect/open-leak-gate.sh", &a);
    assert!(ok, "a clean repository passes:\n{out}");

    // Plant one Pro, one Arksa-exclusive term (in a file) and a Core term (as a path).
    let pro = &terms("pro")[0];
    let ark = &terms("arksa")[0];
    let core = &terms("core")[0];
    std::fs::write(repo.join("notes.md"), format!("text {pro} and {ark}\n")).unwrap();
    std::fs::create_dir_all(repo.join(core)).unwrap();
    std::fs::write(repo.join(core).join("x.txt"), "x\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "leak"]);
    let (ok, out) = run("scripts/protect/open-leak-gate.sh", &a);
    assert!(!ok, "a leak fails the gate:\n{out}");
    assert!(out.contains("notes.md"), "{out}");
    assert!(out.contains("path:"), "{out}");
    for t in [pro, ark] {
        assert!(!out.contains(t.as_str()), "the gate printed a private term");
    }

    // Accepted into a baseline, the same matches pass.
    let baseline = repo.join("..").join(format!(
        "{}-accepted.txt",
        repo.file_name().unwrap().to_string_lossy()
    ));
    let mut w = args(&baseline);
    w.push("--write-baseline".into());
    let w: Vec<&str> = w.iter().map(String::as_str).collect();
    let (ok, out) = run("scripts/protect/open-leak-gate.sh", &w);
    assert!(ok, "{out}");
    let b = args(&baseline);
    let b: Vec<&str> = b.iter().map(String::as_str).collect();
    let (ok, out) = run("scripts/protect/open-leak-gate.sh", &b);
    assert!(ok, "accepted matches pass:\n{out}");
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_file(&empty_baseline);
    let _ = std::fs::remove_file(&baseline);
}

#[test]
fn the_committed_baseline_holds_ids_not_terms() {
    let base = std::fs::read_to_string(root().join("protect/open-leak-baseline.txt")).unwrap();
    for line in base.lines().filter(|l| !l.starts_with('#')) {
        let (_, id) = line.split_once('\t').expect("path TAB id");
        assert!(
            id.len() == 12 && id.chars().all(|c| c.is_ascii_hexdigit()),
            "baseline entries carry a term ID, never a term: {line}"
        );
    }
}
