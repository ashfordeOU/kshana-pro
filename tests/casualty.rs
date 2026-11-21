// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Maritime casualty reconstruction (research 2026-11-20 gaps B0601) on SYNTHETIC grounding
//! and collision cases with known truth: the causation follows the injected cause, the
//! reconstructed track stays on the true one within its uncertainty, and the signed pack
//! verifies. No published casualty data are used.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::forensics::casualty::{self, synth, Causation, Request};
use kshana_pro::transport::dist_m;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b0601-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn case(c: synth::Case, name: &str) -> (casualty::Report, PathBuf) {
    let d = tmp(name);
    let s = synth::make(c);
    for (n, t) in &s.files {
        std::fs::write(d.join(n), t).unwrap();
    }
    let q: Request = toml::from_str(&s.request).unwrap();
    let (r, _) = casualty::run(&q, &d).unwrap();
    (r, d)
}

#[test]
fn causation_follows_the_injected_cause() {
    let (r, _) = case(synth::Case::GroundingSpoofed, "spoof");
    assert_eq!(
        r.causation.causation,
        Causation::Interference,
        "{:#?}",
        r.causation
    );
    let a = &r.at_casualty;
    assert!(a.divergence_m.unwrap() > 700.0, "{a:?}");
    assert!(a.displayed_clearance_m.unwrap() > 500.0);
    assert!(a.reconstructed_clearance_m.unwrap().abs() <= 2.0 * a.reconstructed_sigma_m);
    // The reconstructed position sits on the true one within its 2-sigma envelope.
    let truth = synth::shoal();
    let rc = a.reconstructed.unwrap();
    assert!(dist_m(rc[0], rc[1], truth.0, truth.1) - 150.0 <= 2.0 * a.reconstructed_sigma_m);
    assert!(!r.interference_episodes.is_empty());
    assert!(r.dead_reckoning_from_utc.is_some());
    assert!(r.tracks.iter().any(|t| t.source == "dead-reckoning"));

    let (r, _) = case(synth::Case::GroundingSpoofedRadar, "spoof-radar");
    assert_eq!(
        r.causation.causation,
        Causation::InterferenceWithNavigationalFactor
    );
    assert!(r.radar_fixes.iter().any(|f| f.displayed_disagrees));
    // The radar fixes agree with the reconstructed track, not the displayed one.
    let last = r.radar_fixes.last().unwrap();
    assert!(last.vs_reconstructed_m.unwrap() < 150.0, "{last:?}");
    assert!(last.vs_displayed_m.unwrap() > 600.0, "{last:?}");

    let (r, _) = case(synth::Case::GroundingJammed, "jam");
    assert_eq!(
        r.causation.causation,
        Causation::InterferenceWithNavigationalFactor
    );
    assert!(r.at_casualty.displayed.is_none());
    assert!(r.at_casualty.no_fix_before_s > 300.0);

    let (r, _) = case(synth::Case::GroundingNavError, "nav");
    assert_eq!(r.causation.causation, Causation::NavigationalError);
    assert!(r.interference_episodes.is_empty());
    assert!(r.radar_fixes.iter().all(|f| !f.displayed_disagrees));

    let (r, _) = case(synth::Case::CollisionNavError, "collision");
    assert_eq!(r.causation.causation, Causation::NavigationalError);
    assert!(
        r.radar_cpa.iter().rev().take(10).all(|c| c.cpa_m < 926.0),
        "{:?}",
        r.radar_cpa
    );
    assert!(r.evidence.starts_with("MODELLED"));
    assert!(r.data_source.contains("SYNTHETIC"));
    assert!(r.causation.confidence > 0.0 && r.causation.confidence < 1.0);
}

#[test]
fn requests_are_checked() {
    let s = synth::make(synth::Case::GroundingSpoofed);
    let d = tmp("bad");
    for (n, t) in &s.files {
        std::fs::write(d.join(n), t).unwrap();
    }
    let mut q: Request = toml::from_str(&s.request).unwrap();
    q.hazard = None;
    assert!(casualty::run(&q, &d).is_err(), "grounding without a danger");
    let mut q: Request = toml::from_str(&s.request).unwrap();
    q.data_source = " ".into();
    assert!(casualty::run(&q, &d).is_err(), "data source required");
}

#[test]
fn cli_signs_a_pack_that_verifies() {
    let d = tmp("cli");
    let home = d.join("home");
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (ok, m) = run(&[
        "casualty",
        "synth",
        "grounding-spoofed-radar",
        d.to_str().unwrap(),
    ]);
    assert!(ok, "{m}");
    let seed = [7u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("key.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("pack");
    let r = d.join("request.toml");
    let kf = d.join("key.txt");
    let args = [
        "casualty",
        "run",
        r.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        kf.to_str().unwrap(),
        "--utc",
        "2026-11-20T00:00:00Z",
    ];
    let (ok, _) = run(&args);
    assert!(!ok, "needs a licence");
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(
        ok && m.contains("InterferenceWithNavigationalFactor"),
        "{m}"
    );
    for f in [
        "report.json",
        "report.html",
        "tracks.csv",
        "interference/evidence-pack.json",
        "inputs/vdr.nmea",
        "inputs/radar.csv",
    ] {
        assert!(out.join(f).is_file(), "{f}");
    }
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}
