// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! ADS-B phantom-target plausibility evidence (research 2026-11-20 gaps B1002) on a SYNTHETIC
//! scene: every injected phantom (a ghost cluster, a teleporting address, an address in two
//! places, an unhearable target, a gross altitude mismatch, a Mach-4 track) is flagged with the
//! expected reason; the genuine traffic (45 aircraft, with dropouts, departures and aircraft
//! already in coverage at the start) is not flagged, with and without the reference source; the
//! pack is signed, needs a licence, and pseudonymised packs carry no address.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::adsb_phantom as ap;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b1002-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rx() -> Vec<ap::Receiver> {
    let (_, r, _, _) = ap::synth();
    toml::from_str::<ap::Receivers>(&r).unwrap().receiver
}

fn codes(t: &ap::Target) -> Vec<&str> {
    t.reasons.iter().map(|r| r.code).collect()
}

#[test]
fn injected_phantoms_are_flagged_and_genuine_traffic_is_not() {
    let (csv, _, reference, inj) = ap::synth();
    let p = ap::Params::default();
    for with_ref in [true, false] {
        let r = ap::check(
            &csv,
            &rx(),
            with_ref.then_some(reference.as_str()),
            &p,
            None,
        )
        .unwrap();
        let by = |a: &str| r.targets.iter().find(|t| t.address == a).unwrap();
        for a in &inj {
            assert!(by(a).flagged, "{a} (ref {with_ref}): {:?}", by(a).reasons);
        }
        for g in ["a0b000", "a0b001", "a0b002", "a0b003"] {
            assert!(codes(by(g)).contains(&"cluster"), "{g}");
        }
        assert!(codes(by("a0c001")).contains(&"teleport"));
        assert!(codes(by("a0c002")).contains(&"two-places"));
        assert!(codes(by("a0c003")).contains(&"unhearable"));
        assert!(codes(by("a0c004")).contains(&"altitude-gross"));
        assert!(codes(by("a0c005")).contains(&"physics"));
        if with_ref {
            assert!(codes(by("a0b000")).contains(&"no-independent-track"));
        }
        let genuine: Vec<&ap::Target> = r
            .targets
            .iter()
            .filter(|t| !inj.contains(&t.address))
            .collect();
        assert_eq!(genuine.len(), 45);
        let false_flags = genuine.iter().filter(|t| t.flagged).count();
        // Stated rate on this scene: 0 of 45 genuine aircraft (docs/ADSB-PHANTOM.md).
        assert_eq!(
            false_flags,
            0,
            "{:?}",
            genuine.iter().filter(|t| t.flagged).collect::<Vec<_>>()
        );
        assert_eq!(r.flagged, inj.len());
    }
}

#[test]
fn the_rules_on_small_cases() {
    let rxs = rx();
    // Radio horizon: a low target is out of reach long before the declared range.
    assert!(ap::reach_m(&rxs[0], 0.0) < 30_000.0);
    assert!((ap::reach_m(&rxs[0], 12_000.0) - 150.0 * 1852.0).abs() < 1e-6);
    let p = ap::Params::default();
    let head = "timestamp,aircraft_id,lat,lon,alt_baro_ft,source_type\n";
    // A steady track, then one jump: teleport, not two places.
    let mut a = String::from(head);
    for i in 0..20 {
        let lon = 5.0 + i as f64 * 0.03 + if i >= 10 { 1.0 } else { 0.0 };
        a.push_str(&format!(
            "{},abc123,52.0,{lon},30000,adsb_icao\n",
            1000 + i * 10
        ));
    }
    // Non-ADS-B rows are excluded; an address that is not hex is reported.
    a.push_str("1000,abc123,10.0,10.0,30000,mlat\n");
    for i in 0..5 {
        a.push_str(&format!(
            "{},zz9,52.0,{},30000,adsb_icao\n",
            1000 + i * 10,
            5.0 + i as f64 * 0.03
        ));
    }
    let r = ap::check(&a, &rxs, None, &p, None).unwrap();
    assert_eq!(r.stats.excluded_non_adsb, 1);
    let t = r.targets.iter().find(|t| t.address == "abc123").unwrap();
    assert_eq!(codes(t), vec!["teleport"]);
    let z = r.targets.iter().find(|t| t.address == "zz9").unwrap();
    assert!(codes(z).contains(&"bad-address"));
    // Pseudonymised: no address appears in the report.
    let r = ap::check(&a, &rxs, None, &p, Some("salt-1")).unwrap();
    let j = serde_json::to_string(&r).unwrap();
    assert!(!j.contains("abc123") && !j.contains("zz9") && r.pseudonymised);
    // A report naming a receiver is checked against that receiver only.
    let far = format!(
        "{head}1000,abc124,51.5,9.5,30000,adsb_icao\n1010,abc124,51.5,9.53,30000,adsb_icao\n"
    );
    let r = ap::check(
        &far.replace(",source_type", ",source_type,receiver_id")
            .replace("adsb_icao", "adsb_icao,RX-N"),
        &rxs,
        None,
        &p,
        None,
    )
    .unwrap();
    assert!(codes(&r.targets[0]).contains(&"unhearable"));
    assert!(ap::check(&a, &[], None, &p, None).is_err());
}

#[test]
fn signed_pack_needs_a_licence_and_pseudonymises() {
    let d = tmp("cli");
    let home = d.join("home");
    let seed = [41u8; 32];
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={hex}\n")).unwrap();
    let run = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(args)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap();
        (
            o.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
        )
    };
    let s = d.to_str().unwrap();
    let (ok, m) = run(&["adsb-phantom", "synth", s]);
    assert!(ok, "{m}");
    let (a, r, f) = (
        d.join("adsb.csv"),
        d.join("receivers.toml"),
        d.join("reference.csv"),
    );
    let (a, r, f, k) = (
        a.to_str().unwrap(),
        r.to_str().unwrap(),
        f.to_str().unwrap(),
        key.to_str().unwrap(),
    );
    let out = d.join("pack");
    let o = out.to_str().unwrap();
    let (ok, m) = run(&[
        "adsb-phantom",
        "check",
        a,
        r,
        "--reference",
        f,
        "--out",
        o,
        "--signer",
        "T",
        "--key-file",
        k,
    ]);
    assert!(!ok, "{m}");
    let (ok, m) = run(&[
        "adsb-phantom",
        "check",
        a,
        r,
        "--reference",
        f,
        "--out",
        o,
        "--signer",
        "T",
        "--key-file",
        k,
        "--dev",
    ]);
    assert!(ok && m.contains("9 of 54"), "{m}");
    let pk = kshana_pro::license::public_key_for(&seed);
    assert!(kshana_pro::signed_dossier::verify(&out, &[pk])
        .unwrap()
        .passed());
    let html = std::fs::read_to_string(out.join("report.html")).unwrap();
    assert!(html.contains("No emitter location"));
    // Pseudonymised: neither the report nor the inputs carry an address.
    let out2 = d.join("pseudo");
    let o2 = out2.to_str().unwrap();
    let (ok, m) = run(&[
        "adsb-phantom",
        "check",
        a,
        r,
        "--out",
        o2,
        "--signer",
        "T",
        "--key-file",
        k,
        "--dev",
        "--pseudonymise",
        "s3",
    ]);
    assert!(ok, "{m}");
    assert!(!out2.join("inputs").join("adsb.csv").exists());
    assert!(out2.join("inputs").join("adsb.csv.sha256").exists());
    for f in ["report.json", "report.html"] {
        assert!(
            !std::fs::read_to_string(out2.join(f))
                .unwrap()
                .contains("a0c00"),
            "{f}"
        );
    }
    assert!(kshana_pro::signed_dossier::verify(&out2, &[pk])
        .unwrap()
        .passed());
}
