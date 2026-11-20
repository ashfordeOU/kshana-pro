// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Station fleet management and calibration register (research 2026-11-20 gaps B2604) on
//! SYNTHETIC records: the configuration history is hash-chained and any edit shows; the site log
//! follows the history; the ageing curve predicts when a delay leaves tolerance; a monthly
//! clock-quality statement is signed and verifies offline; the dashboard renders.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_eloran::calreg::{EntryBody, Register, SubjectKind};
use kshana_pro::station_fleet as sf;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("b2604-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn change(station: &str, utc: &str, item: &str, value: &str, serial: &str) -> sf::ConfigBody {
    sf::ConfigBody {
        seq: 0,
        station: station.into(),
        utc: utc.into(),
        item: item.into(),
        value: value.into(),
        serial: serial.into(),
        notes: String::new(),
        by: "Ops".into(),
    }
}

#[test]
fn configuration_history_is_chained_and_edits_show() {
    let d = tmp("cfg");
    let p = d.join("config.jsonl");
    sf::config_append(
        &p,
        change(
            "STA1",
            "2025-01-01T00:00:00Z",
            "receiver",
            "SYNTH-RX v1",
            "R1",
        ),
    )
    .unwrap();
    sf::config_append(
        &p,
        change("STA1", "2025-01-01T00:00:00Z", "antenna", "SYNTH-ANT", "A1"),
    )
    .unwrap();
    sf::config_append(
        &p,
        change(
            "STA2",
            "2025-03-01T00:00:00Z",
            "receiver",
            "SYNTH-RX v1",
            "R2",
        ),
    )
    .unwrap();
    sf::config_append(
        &p,
        change(
            "STA1",
            "2026-02-01T00:00:00Z",
            "receiver",
            "SYNTH-RX v2",
            "R3",
        ),
    )
    .unwrap();
    let c = sf::config_verify(&p).unwrap();
    assert!(c.problems.is_empty() && c.entries == 4, "{c:?}");
    assert!(sf::config_append(
        &p,
        change("STA1", "2026-02-01T00:00:00Z", "toaster", "x", "")
    )
    .is_err());
    let log = sf::site_log(&p, "STA1", "Ops", "2026-11-20T00:00:00Z").unwrap();
    assert!(log.contains("3.1  Receiver Type") && log.contains("3.2  Receiver Type"));
    assert!(
        log.contains("Date Removed           : 2026-02-01T00:00:00Z"),
        "{log}"
    );
    assert!(log.contains("4.1  Antenna Type"));
    // Edit a line: the chain breaks there, and no further change is appended.
    let t = std::fs::read_to_string(&p)
        .unwrap()
        .replacen("SYNTH-ANT", "OTHER-ANT", 1);
    std::fs::write(&p, t).unwrap();
    let c = sf::config_verify(&p).unwrap();
    assert!(c.problems.iter().any(|x| x.contains("line 2")), "{c:?}");
    assert!(sf::config_append(
        &p,
        change("STA1", "2026-03-01T00:00:00Z", "firmware", "2", "")
    )
    .is_err());
    // Removing a line breaks the chain too.
    let d2 = tmp("cfg2");
    let p2 = d2.join("config.jsonl");
    for k in 0..3 {
        sf::config_append(
            &p2,
            change("S", "2025-01-01T00:00:00Z", "firmware", &k.to_string(), ""),
        )
        .unwrap();
    }
    let ls: Vec<String> = std::fs::read_to_string(&p2)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    std::fs::write(&p2, format!("{}\n{}\n", ls[0], ls[2])).unwrap();
    assert!(!sf::config_verify(&p2).unwrap().problems.is_empty());
}

#[test]
fn ageing_predicts_leaving_tolerance() {
    let d = tmp("age");
    let reg = Register::open(&d.join("cal.jsonl"));
    for (date, v) in [
        ("2023-01-01", 100.0),
        ("2024-01-01", 101.0),
        ("2025-01-01", 102.0),
    ] {
        reg.append(EntryBody {
            seq: 0,
            subject_id: "CBL-7".into(),
            subject_kind: SubjectKind::Instrument,
            date: date.into(),
            quantity: "cable_delay_ns".into(),
            value: v,
            unit: "ns".into(),
            expanded_u: 0.2,
            k: 2.0,
            operator: "Ops".into(),
            report_sha256: "0".repeat(64),
            interval_days: Some(365),
            notes: String::new(),
        })
        .unwrap();
    }
    let a = sf::ageing(
        &d.join("cal.jsonl"),
        "CBL-7",
        "cable_delay_ns",
        Some([95.0, 104.0]),
        "2026-01-01",
    )
    .unwrap();
    assert!(
        (a.drift_per_year - 1.0).abs() < 0.01,
        "{}",
        a.drift_per_year
    );
    assert!((a.predicted_now - 103.0).abs() < 0.02);
    // 104 ns is reached about a year after 2026-01-01.
    let leaves = a.leaves_tolerance.clone().unwrap();
    assert!(
        leaves.starts_with("2026-12") || leaves.starts_with("2027-01"),
        "{leaves}"
    );
    assert_eq!(a.due_date.as_deref(), Some("2026-01-01"));
    assert!(a.chain_ok);
    let h = sf::dashboard(&d.join("none.jsonl"), &[a], "2026-11-20T00:00:00Z").unwrap();
    assert!(h.contains("CBL-7") && h.contains("Calibration ageing"));
}

#[test]
fn monthly_statement_is_signed_and_verifies() {
    let d = tmp("stmt");
    // SYNTHETIC: 1 s offsets for one hour in June with a 50 ns linear ramp, plus one sample in July.
    let mut csv = String::from("timestamp,offset_s\n");
    for k in 0..3600 {
        let t = kshana_pro::forensics::inputs::iso(1_780_272_000.0 + k as f64);
        csv.push_str(&format!("{t},{:.3e}\n", 50e-9 * k as f64 / 3600.0));
    }
    csv.push_str("2026-07-01T00:00:00Z,1e-6\n");
    let s = sf::statement("STA1", "SYNTH-REF", "2026-06", &csv, &[1.0, 100.0, 1000.0]).unwrap();
    assert_eq!(s.samples, 3600, "July excluded");
    assert!((s.peak_to_peak_s - 50e-9 * 3599.0 / 3600.0).abs() < 1e-10);
    let m1000 = s.mtie.iter().find(|x| x.0 == 1000.0).unwrap().1;
    assert!((m1000 - 50e-9 * 1000.0 / 3600.0).abs() < 1e-11, "{m1000}");
    assert!(s.statement.contains("not a certificate"));
    let home = d.join("home");
    std::fs::write(d.join("off.csv"), &csv).unwrap();
    let hex: String = [6u8; 32].iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("k.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("stmt");
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args([
            "fleet",
            "statement",
            d.join("off.csv").to_str().unwrap(),
            "--station",
            "STA1",
            "--reference",
            "SYNTH-REF",
            "--month",
            "2026-06",
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "Ops",
            "--key-file",
            d.join("k.txt").to_str().unwrap(),
            "--dev",
        ])
        .env("KSHANA_PRO_HOME", &home)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let v = kshana_pro::signed_dossier::verify(
        &out,
        &[kshana_pro::license::public_key_for(&[6u8; 32])],
    )
    .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    // config-verify is free; config-add needs the licence.
    let cfg = d.join("c.jsonl");
    let run = |a: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
            .args(a)
            .env("KSHANA_PRO_HOME", &home)
            .output()
            .unwrap()
            .status
            .success()
    };
    let c = cfg.to_str().unwrap();
    assert!(!run(&[
        "fleet",
        "config-add",
        c,
        "--station",
        "S",
        "--item",
        "receiver",
        "--value",
        "x",
        "--by",
        "Ops"
    ]));
    assert!(run(&[
        "fleet",
        "config-add",
        c,
        "--station",
        "S",
        "--item",
        "receiver",
        "--value",
        "x",
        "--by",
        "Ops",
        "--dev"
    ]));
    assert!(run(&["fleet", "config-verify", c]));
}
