// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
// Roadmap 0.42 X8: scripts/protect/arksa-copy-check.py allows "in Arksa" claims only for
// the four outcome names, and never with a private term nearby.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(dir: &Path, scope: &str, terms: Option<&Path>) -> (bool, String) {
    let mut c = Command::new("python3");
    c.arg(root().join("scripts/protect/arksa-copy-check.py"))
        .args(["--scope", scope, "--dir"])
        .arg(dir);
    if let Some(t) = terms {
        c.arg("--terms").arg(t);
    }
    let out = c.output().expect("python3 runs");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

fn site(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-copy-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for (p, t) in files {
        let f = d.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, t).unwrap();
    }
    d
}

fn terms_file(dir: &Path) -> PathBuf {
    let p = dir.join("terms.txt");
    std::fs::write(
        &p,
        "# test terms\n[pro]\nsecret-method-word\nCovarianceTrick\n",
    )
    .unwrap();
    p
}

#[test]
fn allowed_outcomes_pass() {
    // The private repository's names, built here so this file never spells them (the source guard).
    let crate_name = "arksa";
    let notes = format!(
        "The code lives in {crate_name}-kpro and {crate_name}_kpro. The licence says: it is provided only within Arksa. The rule label is \"In Arksa\".\n"
    );
    let d = site(
        "ok",
        &[
            ("index.md", "# Arksa\n\nThe Mission navigation digital twin is available in Arksa.\n\nIn Arksa: Lunar navigation and timing assurance, Independent interference referee reports.\n"),
            ("page.html", "<p>Spaceborne GNSS receiver interference assurance, only in Arksa.</p>"),
            ("notes.md", notes.as_str()),
        ],
    );
    let t = terms_file(&d);
    let (ok, out) = run(&d, "site", Some(&t));
    assert!(ok, "{out}");
}

#[test]
fn other_claims_and_method_words_fail_by_term_id() {
    let d = site(
        "bad",
        &[
            ("a.md", "Our new jamming locator is available in Arksa.\n"),
            ("b.md", "The Mission navigation digital twin, in Arksa, uses the CovarianceTrick to fuse clocks.\n"),
            ("c.html", "<li>Within Arksa you get orbit determination.</li>"),
        ],
    );
    let t = terms_file(&d);
    let (ok, out) = run(&d, "site", Some(&t));
    assert!(!ok, "{out}");
    assert!(out.contains("a.md:1: an \"in Arksa\" claim"), "{out}");
    assert!(out.contains("c.html:1: an \"in Arksa\" claim"), "{out}");
    assert!(
        !out.contains("b.md:1: an \"in Arksa\" claim"),
        "allowed outcome flagged: {out}"
    );
    assert!(out.contains("b.md:1: private term"), "{out}");
    // The term itself never reaches the output, only its ID.
    assert!(!out.contains("CovarianceTrick"), "{out}");
}

#[test]
fn this_repository_passes_its_own_check() {
    let (ok, out) = run(&root(), "kpro", None);
    assert!(ok, "{out}");
}
