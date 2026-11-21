// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Aviation and drone event reconstruction (research 2026-11-20 gaps B0602) on a SYNTHETIC King
//! Air-type flight: the clock offsets of the ADS-B and tracker records are recovered, the ISA
//! and datum parts of the GNSS-baro difference are explained, and the injected cruise
//! divergence is reproduced as one episode common to every GNSS-derived source. No operator or
//! published event data are used.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::aviation::event_recon::{self, synth, Explanation, Request};
use kshana_pro::forensics::inputs::parse_utc;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b0602-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path) -> Request {
    let (f, a, t, r) = synth::make();
    std::fs::write(d.join("fdm.csv"), f).unwrap();
    std::fs::write(d.join("adsb.csv"), a).unwrap();
    std::fs::write(d.join("tracker.csv"), t).unwrap();
    std::fs::write(d.join("request.toml"), &r).unwrap();
    toml::from_str(&r).unwrap()
}

#[test]
fn king_air_type_divergence_is_reproduced() {
    let d = tmp("ka");
    let q = write(&d);
    let (r, _) = event_recon::run(&q, &d).unwrap();
    let off = |s: &str| r.alignment.iter().find(|a| a.source == s).unwrap().offset_s;
    assert_eq!(off("adsb"), -synth::ADSB_CLOCK_S, "{:?}", r.alignment);
    assert_eq!(off("tracker"), -synth::TRACKER_CLOCK_S, "{:?}", r.alignment);
    // The ISA term explains about 1000 ft of cruise GNSS-baro change; the datum is estimated.
    assert!(r.explained.isa_term_ft_max > 900.0, "{:?}", r.explained);
    assert!(r.explained.outside_episodes_raw_ft > 900.0);
    assert!(
        r.explained.outside_episodes_residual_ft < 100.0,
        "{:?}",
        r.explained
    );
    let datum = r.explained.datum_ft.unwrap();
    assert!((datum - synth::GEOID_FT).abs() < 20.0, "{datum}");
    assert!(r.explained.datum_estimated);
    // One episode: the injected window, common to all GNSS-derived sources.
    assert_eq!(r.episodes.len(), 1, "{:#?}", r.episodes);
    let e = &r.episodes[0];
    assert_eq!(e.explanation, Explanation::CommonGnssDivergence);
    assert_eq!(e.diverging.len(), 3, "{:?}", e.diverging);
    let s = parse_utc(&e.start_utc).unwrap() - synth::T0;
    let x = parse_utc(&e.end_utc).unwrap() - synth::T0;
    assert!(s >= synth::EV.0 && s <= synth::EV.0 + 30.0, "{s}");
    assert!((x - (synth::EV.1 - 1.0)).abs() <= 2.0, "{x}");
    for (k, v) in &e.residual_ft {
        assert!((v - synth::PEAK_FT).abs() < 150.0, "{k} {v}");
    }
    assert!(r.evidence.starts_with("MODELLED"));
    assert!(r.scope.contains("no source is fused"));
}

#[test]
fn without_the_isa_deviation_the_atmosphere_is_not_explained() {
    let d = tmp("noisa");
    let mut q = write(&d);
    q.isa_dev_c = None;
    let (r, _) = event_recon::run(&q, &d).unwrap();
    // The cruise ISA difference is now unexplained: more flagged time than the injection.
    let total: f64 = r.episodes.iter().map(|e| e.duration_s).sum();
    assert!(total > synth::EV.1 - synth::EV.0 + 300.0, "{total}");
    let mut bad = q.clone();
    bad.data_source = "".into();
    assert!(event_recon::run(&bad, &d).is_err());
}

#[test]
fn cli_signs_an_investigator_pack() {
    let d = tmp("cli");
    write(&d);
    let home = d.join("home");
    let seed = [9u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("key.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("pack");
    let (r, kf) = (d.join("request.toml"), d.join("key.txt"));
    let mut args = vec![
        "aviation-recon".to_string(),
        "run".into(),
        r.to_str().unwrap().into(),
        "--out".into(),
        out.to_str().unwrap().into(),
        "--signer".into(),
        "t".into(),
        "--key-file".into(),
        kf.to_str().unwrap().into(),
        "--utc".into(),
        "2026-11-20T00:00:00Z".into(),
    ];
    let run = |a: &[String]| {
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
    assert!(!run(&args).0, "needs a licence");
    args.push("--dev".into());
    let (ok, m) = run(&args);
    assert!(ok && m.contains("CommonGnssDivergence"), "{m}");
    let v = kshana_pro::signed_dossier::verify(&out, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
}
