// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! NavIC test kit and integrity evidence (research 2026-11-20 gaps B1605) on SYNTHETIC logs:
//! the scaffold carries no value; unhealthy-satellite use and NavIC-only fixes with three
//! healthy satellites are caught; a NavIC solution offset from GPS and Galileo shows on both of
//! its pairs; timing evidence is signed and verifies.

use std::process::Command;

use kshana_pro::forensics::inputs::iso;
use kshana_pro::navic;

const T0: f64 = 1_780_272_000.0;

#[test]
fn scaffold_is_all_placeholders() {
    let s = navic::scaffold();
    for c in s["cases"].as_array().unwrap() {
        for (_, f) in c["fields"].as_object().unwrap() {
            assert!(f["value"].is_null() && f["source"].is_null());
        }
    }
}

#[test]
fn health_catches_unhealthy_use_and_three_satellite_fixes() {
    // Epochs 0-9: four NavIC tracked (I2..I5); from 5 s, I5 is unhealthy but still used at 5-6 s.
    let mut sats = String::from("timestamp,system,prn,used\n");
    let mut fixes = String::from("timestamp,fix_systems\n");
    for k in 0..10 {
        let t = iso(T0 + k as f64);
        for p in 2..=5 {
            let used = !(p == 5 && k >= 7);
            sats.push_str(&format!("{t},I,{p},{}\n", used as u8));
        }
        sats.push_str(&format!("{t},G,7,1\n"));
        // NavIC-only fix every epoch: fine before 5 s, not after (three healthy left).
        fixes.push_str(&format!("{t},{}\n", if k % 2 == 0 { "I" } else { "G+I" }));
    }
    let health = format!(
        "timestamp,system,prn,healthy\n{},I,5,1\n{},I,5,0\n",
        iso(T0 - 100.0),
        iso(T0 + 5.0)
    );
    let h = navic::health(&sats, &health, &fixes).unwrap();
    assert_eq!(h.epochs, 10);
    assert_eq!(h.epochs_under_four, 5);
    assert_eq!(h.unhealthy_used.len(), 2, "{:?}", h.unhealthy_used);
    assert_eq!(h.unhealthy_used[0].utc, iso(T0 + 5.0));
    // NavIC-only fixes at 6 and 8 s with three healthy.
    assert_eq!(
        h.navic_only_under_four.len(),
        2,
        "{:?}",
        h.navic_only_under_four
    );
}

#[test]
fn consistency_points_at_the_offset_system() {
    let mut csv = String::from("timestamp,solution,lat,lon,sigma_h_m\n");
    for k in 0..120 {
        let t = iso(T0 + k as f64);
        let off = if (40..80).contains(&k) { 0.001 } else { 0.0 }; // ~111 m north
        csv.push_str(&format!(
            "{t},I,{},77.0,3\n{t},G,12.0,77.0,3\n{t},E,12.0,77.0,3\n",
            12.0 + off
        ));
    }
    let c = navic::consistency(&csv, 5.0, 10.0).unwrap();
    assert_eq!(c.episodes.len(), 2);
    assert!(c
        .episodes
        .iter()
        .all(|e| e.pair.contains('I') && e.epochs == 40));
    assert_eq!(c.pairs["E-G"], (120, 0));
    assert!(navic::consistency(&csv, 0.0, 10.0).is_err());
}

#[test]
fn timing_evidence_is_signed() {
    let d = std::env::temp_dir().join(format!("b1605-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let mut csv = String::from("timestamp,offset_navic_s,offset_gps_s\n");
    for k in 0..600 {
        let x = 20e-9 * (k as f64 / 600.0);
        csv.push_str(&format!(
            "{},{:.4e},{:.4e}\n",
            iso(T0 + k as f64),
            x + 5e-9,
            x
        ));
    }
    std::fs::write(d.join("off.csv"), &csv).unwrap();
    std::fs::write(
        d.join("t.toml"),
        "title = \"SYNTHETIC NavIC timing\"\noffsets = \"off.csv\"\ntraceability = [\"receiver -> NavIC system time (as broadcast)\", \"NavIC system time -> UTC(k) (to confirm)\"]\n",
    )
    .unwrap();
    let q: navic::TimingRequest =
        toml::from_str(&std::fs::read_to_string(d.join("t.toml")).unwrap()).unwrap();
    let t = navic::timing(&q, &csv).unwrap();
    assert!((t.navic_minus_gps_mean_s - 5e-9).abs() < 1e-12);
    assert!(t.navic_minus_gps_rms_s < 1e-12);
    let hex: String = [4u8; 32].iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("k.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("o");
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args([
            "navic",
            "timing",
            d.join("t.toml").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "t",
            "--key-file",
            d.join("k.txt").to_str().unwrap(),
            "--dev",
        ])
        .env("KSHANA_PRO_HOME", d.join("home"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v = kshana_pro::signed_dossier::verify(
        &out,
        &[kshana_pro::license::public_key_for(&[4u8; 32])],
    )
    .unwrap();
    assert!(v.passed());
    let mut q2 = q;
    q2.traceability.clear();
    assert!(navic::timing(&q2, &csv).is_err());
}
