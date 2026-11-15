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

fn run_with(dir: &Path, terms: &Path, register: &Path, today: &str) -> (Option<i32>, String) {
    let out = Command::new("python3")
        .arg(root().join("scripts/protect/arksa-copy-check.py"))
        .args(["--scope", "site", "--dir"])
        .arg(dir)
        .arg("--terms")
        .arg(terms)
        .arg("--register")
        .arg(register)
        .args(["--today", today])
        .output()
        .expect("python3 runs");
    (
        out.status.code(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// Roadmap 0.42 X7: nothing on the trade-secret register is published before its filing
/// date, and a secret item never; findings name the item and the term ID only.
#[test]
fn the_register_blocks_disclosure_before_filing() {
    let d = site(
        "register",
        &[
            (
                "blog.md",
                "# News\n\nToday we explain the FilteredSkyTrick in depth.\n",
            ),
            (
                "paper.html",
                "<p>The covariance-free horizon map, as filed.</p>",
            ),
            ("plain.md", "Nothing private here.\n"),
        ],
    );
    let t = terms_file(&d);
    let reg = std::env::temp_dir().join(format!("kpro-register-{}.toml", std::process::id()));
    let tid = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(b"secret-method-word"))[..12].to_string()
    };
    std::fs::write(
        &reg,
        format!(
            r#"format = "kshana-pro-trade-secret-register/1"
[[item]]
id = "P1"
status = "filed"
filing_date = "2027-03-01"
term_ids = ["{tid}"]
terms = ["covariance-free horizon map"]
[[item]]
id = "P4"
status = "secret"
terms = ["FilteredSkyTrick"]
[[item]]
id = "P9"
status = "public"
terms = ["Nothing private"]
"#
        ),
    )
    .unwrap();
    let (code, out) = run_with(&d, &t, &reg, "2026-11-15");
    assert_eq!(code, Some(1), "{out}");
    assert!(
        out.contains("blog.md:3: trade-secret register item P4 (secret)"),
        "{out}"
    );
    assert!(
        out.contains(
            "paper.html:1: trade-secret register item P1 (before its filing date 2027-03-01)"
        ),
        "{out}"
    );
    assert!(!out.contains("plain.md"), "a public item is free: {out}");
    for secret in ["FilteredSkyTrick", "covariance-free", "horizon map"] {
        assert!(!out.contains(secret), "term printed: {out}");
    }
    // From the filing date the filed item may be published; the secret one never.
    let (code, out) = run_with(&d, &t, &reg, "2027-03-01");
    assert_eq!(code, Some(1), "{out}");
    assert!(!out.contains("paper.html"), "{out}");
    assert!(out.contains("item P4"), "{out}");
    std::fs::write(d.join("blog.md"), "# News\n").unwrap();
    let (code, out) = run_with(&d, &t, &reg, "2027-03-01");
    assert_eq!(code, Some(0), "{out}");

    // A broken register is an error, never a silent pass.
    for bad in [
        "format = \"kshana-pro-trade-secret-register/1\"\n[[item]]\nid = \"P1\"\nstatus = \"maybe\"\n",
        "format = \"kshana-pro-trade-secret-register/1\"\n[[item]]\nid = \"P1\"\nstatus = \"filed\"\n",
        "format = \"kshana-pro-trade-secret-register/1\"\n[[item]]\nid = \"P1\"\nstatus = \"secret\"\nterm_ids = [\"000000000000\"]\n",
        "format = \"kshana-pro-trade-secret-register/1\"\n[[item]]\nid = \"P1\"\nstatus = \"secret\"\n[[item]]\nid = \"P1\"\nstatus = \"secret\"\n",
        "format = \"other\"\n",
    ] {
        std::fs::write(&reg, bad).unwrap();
        let (code, out) = run_with(&d, &t, &reg, "2026-11-15");
        assert_eq!(code, Some(2), "{bad}: {out}");
    }
}

#[test]
fn the_committed_register_is_well_formed_and_lists_the_roadmap_items() {
    let text = std::fs::read_to_string(root().join("protect/trade-secret-register.toml")).unwrap();
    let v: toml::Value = toml::from_str(&text).unwrap();
    let items = v["item"].as_array().unwrap();
    let get = |id: &str| {
        items
            .iter()
            .find(|i| i["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("register has no {id}"))
    };
    assert_eq!(get("P4")["status"].as_str(), Some("secret"));
    for id in ["P1", "P2", "P5", "B6"] {
        assert_eq!(get(id)["status"].as_str(), Some("to-file"), "{id}");
    }
}
