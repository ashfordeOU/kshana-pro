// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Study dossier guard.
//!
//! Runs the dossier on the bundled timing scenario and checks the two properties a
//! reviewer relies on: the dossier is **deterministic** (same scenario, same result
//! hash, byte-identical files) and the **tier split** is honest (every figure's tier is
//! exactly what the open verification matrix grants it, and the counts add up). It
//! also drives the real binary, so the command, its files and its licence gate are
//! covered end to end.

use std::path::PathBuf;
use std::process::Command;

use kshana_pro::study_dossier::{build, DossierOptions, FRAMEWORK_LABEL};

const SCENARIO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scenarios/dossier-holdover-csac.toml"
);

fn src() -> String {
    std::fs::read_to_string(SCENARIO).expect("bundled dossier scenario is readable")
}

#[test]
fn same_scenario_gives_the_same_hash_twice() {
    let a = build(&src(), &DossierOptions::default()).expect("dossier builds");
    let b = build(&src(), &DossierOptions::default()).expect("dossier builds");
    assert_eq!(a.result_hash, b.result_hash);
    assert_eq!(a.to_json(), b.to_json());
    assert_eq!(a.to_html(), b.to_html());
    assert!(a.body.assurance.determinism_identical);
    assert_eq!(
        a.body.assurance.provenance.merkle_root,
        b.body.assurance.provenance.merkle_root
    );
}

#[test]
fn the_issue_stamp_does_not_move_the_hash() {
    let a = build(&src(), &DossierOptions::default()).unwrap();
    let b = build(
        &src(),
        &DossierOptions {
            issued_utc: Some("2026-09-25T12:00:00Z".into()),
        },
    )
    .unwrap();
    assert_eq!(a.result_hash, b.result_hash);
    assert!(b.to_html().contains("2026-09-25T12:00:00Z"));
}

#[test]
fn a_changed_input_changes_the_hash() {
    let a = build(&src(), &DossierOptions::default()).unwrap();
    let edited = src().replace("threshold_ns = 20.0", "threshold_ns = 25.0");
    let b = build(&edited, &DossierOptions::default()).unwrap();
    assert_ne!(a.result_hash, b.result_hash);
}

#[test]
fn every_figure_carries_the_tier_the_open_matrix_grants() {
    let d = build(&src(), &DossierOptions::default()).unwrap();
    let s = &d.body.tier_split;
    assert!(s.total > 0);
    assert_eq!(s.validated + s.modelled + s.partial, s.total);
    assert_eq!(s.total, d.body.figures.len());
    for f in &d.body.figures {
        assert!(
            f.path.starts_with("classical.fom."),
            "subject is the classical side: {}",
            f.path
        );
        let leaf = f.path.rsplit('.').next().unwrap();
        let want = kshana::fom_label::tier_for(leaf)
            .map(|t| t.tag())
            .unwrap_or("MODELLED");
        assert_eq!(f.tier, want, "tier of {}", f.path);
    }
    // The HTML shows the same split.
    let html = d.to_html();
    assert!(html.contains(&format!(
        "{} VALIDATED · {} MODELLED · {} PARTIAL of {} figures",
        s.validated, s.modelled, s.partial, s.total
    )));
}

#[test]
fn the_timing_section_and_framework_mapping_are_present_and_labelled() {
    let d = build(&src(), &DossierOptions::default()).unwrap();
    let t = d
        .body
        .timing
        .as_ref()
        .expect("clock scenario has a timing section");
    assert_eq!(t.longest_denial_s, Some(6600.0));
    assert_eq!(t.tier, "MODELLED");
    assert_eq!(t.clock_classes.len(), 3);
    let fm = &d.body.framework_mapping;
    assert_eq!(fm.label, FRAMEWORK_LABEL);
    assert!(fm.tentative_behavioural_level.unwrap_or(0) <= 3);
    assert!(fm.p1952_note.contains("not publicly available"));
    // The sweep ran every declared case without failure.
    assert_eq!(d.body.sensitivity.cases.len(), 5);
    assert!(d.body.sensitivity.cases.iter().all(|c| c.error.is_none()));
    // The technical-note layout.
    let html = d.to_html();
    for h in [
        "1. Identification",
        "2. Scope",
        "3. Applicable and reference documents",
        "4. Method",
        "5. Results",
        "6. Limitations",
    ] {
        assert!(html.contains(h), "missing section {h}");
    }
}

fn out_dir(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("dossier-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn the_cli_writes_identical_files_twice_and_is_licence_gated() {
    let bin = env!("CARGO_BIN_EXE_kshana-pro");
    let mut hashes = Vec::new();
    let mut dirs = Vec::new();
    for tag in ["a", "b"] {
        let dir = out_dir(tag);
        let out = Command::new(bin)
            .args(["study", "--dossier", SCENARIO, "--out"])
            .arg(&dir)
            .arg("--dev")
            .output()
            .expect("binary runs");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        hashes.push(String::from_utf8(out.stdout).unwrap().trim().to_string());
        dirs.push(dir);
    }
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(hashes[0].len(), 64);
    for f in ["dossier.json", "dossier.html", "scenario.run.toml"] {
        let a = std::fs::read(dirs[0].join(f)).unwrap();
        let b = std::fs::read(dirs[1].join(f)).unwrap();
        assert_eq!(a, b, "{f} differs between runs");
    }

    // Fail-closed: no licence means Evaluation tier, which may not build a dossier.
    let denied = Command::new(bin)
        .args(["study", "--dossier", SCENARIO, "--out"])
        .arg(out_dir("denied"))
        .output()
        .expect("binary runs");
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("requires the 'pro' tier"));
}
