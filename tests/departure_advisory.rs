// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Departure go/no-go and pre-entry warning against verified zones (research 2026-11-20 gaps
//! B0806) on a SYNTHETIC passage: active spoofing in the STS area holds the departure, recent
//! jamming in the pilotage district asks for checks, the clean VTS area proceeds with a
//! confidence that says the record is only live, the ice zone off the route does not count,
//! source estimates are never read, and unverified zones are refused.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::exposure::departure::{self, Request};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b0806-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn setup(d: &Path) -> Request {
    let (ev, req) = departure::synth();
    std::fs::write(d.join("events.json"), ev).unwrap();
    std::fs::write(d.join("request.toml"), &req).unwrap();
    toml::from_str(&req).unwrap()
}

#[test]
fn advisory_follows_the_record_zone_by_zone() {
    let d = tmp("adv");
    let q = setup(&d);
    let a = departure::advise(&q, &d).unwrap();
    let z = |n: &str| a.zones.iter().find(|z| z.zone == n).unwrap();
    assert_eq!(
        z("SYNTH STS area").status,
        "HOLD-AND-VERIFY",
        "{:?}",
        z("SYNTH STS area")
    );
    assert!(z("SYNTH STS area").active);
    assert_eq!(
        z("SYNTH pilotage").status,
        "PROCEED-WITH-CHECKS",
        "{:?}",
        z("SYNTH pilotage")
    );
    assert_eq!(z("SYNTH VTS").status, "PROCEED");
    assert_eq!(z("SYNTH VTS").confidence, 0.5);
    assert!(!z("SYNTH ice").on_passage);
    assert_eq!(a.departure, "HOLD-AND-VERIFY");
    assert_eq!(a.source_estimates_ignored, 4);
    // ETAs in passage order, warnings an hour before (or at departure).
    let eta = |n: &str| z(n).entry_eta_utc.clone().unwrap();
    assert!(
        eta("SYNTH VTS") < eta("SYNTH STS area") && eta("SYNTH STS area") < eta("SYNTH pilotage")
    );
    assert!(z("SYNTH pilotage").warn_at_utc.clone().unwrap() < eta("SYNTH pilotage"));
    assert!(z("SYNTH STS area")
        .checks
        .iter()
        .any(|c| c.contains("Spoofing")));
    // An equipment rating of 100 lowers the risk to the floor.
    let mut good = q.clone();
    good.equipment_rating = Some(100.0);
    let b = departure::advise(&good, &d).unwrap();
    let sts = b.zones.iter().find(|z| z.zone == "SYNTH STS area").unwrap();
    assert!(sts.risk < z("SYNTH STS area").risk);
    // Events after the decision time are not known.
    let mut early = q.clone();
    early.departure_utc = "2026-05-30T00:00:00Z".into();
    let c = departure::advise(&early, &d).unwrap();
    assert_eq!(c.departure, "PROCEED");
}

#[test]
fn unverified_zones_are_refused() {
    let d = tmp("unv");
    let mut q = setup(&d);
    q.zones[0].verified_by = " ".into();
    assert!(departure::advise(&q, &d)
        .unwrap_err()
        .contains("not verified"));
}

#[test]
fn cli_signs_the_advisory() {
    let d = tmp("cli");
    let home = d.join("home");
    let seed = [6u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("key.txt"), format!("private_key={hex}\n")).unwrap();
    let run = |a: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(a)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (ok, m) = run(&["exposure", "departure-synth", d.to_str().unwrap()]);
    assert!(ok, "{m}");
    let out = d.join("adv");
    let (r, kf) = (d.join("request.toml"), d.join("key.txt"));
    let args = [
        "exposure",
        "departure",
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
    assert!(!run(&args).0, "needs a licence");
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(ok && m.contains("HOLD-AND-VERIFY"), "{m}");
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}
