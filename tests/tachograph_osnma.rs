// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Smart-tachograph and road-charging OSNMA field verification (research 2026-11-20 gaps B1403)
//! on SYNTHETIC vehicle-unit logs: a unit behaving as Annex IC of (EU) 2021/1228 reads shows no
//! departure; one that adjusts time without authentication (also too soon before the next
//! adjustment), misses a time-conflict event and repeats an anomaly event within ten minutes
//! shows each; the catalogue maps the rows; the
//! signed pack verifies.

use std::process::Command;

use kshana_pro::tacho_osnma::{self as t, Thresholds};

#[test]
fn good_unit_has_no_departure_and_bad_unit_shows_each() {
    let g = t::verify(&t::synth(true), &Thresholds::default()).unwrap();
    assert!(g.findings.iter().all(|f| f.ok), "{:#?}", g.findings);
    assert!(
        (g.not_authenticated_s - 43_200.0).abs() < 1.0,
        "half a day without authentication"
    );
    assert_eq!(g.first_authenticated_after_s, Some(1200.0));
    let b = t::verify(&t::synth(false), &Thresholds::default()).unwrap();
    let bad: Vec<&str> = b
        .findings
        .iter()
        .filter(|f| !f.ok)
        .map(|f| f.check)
        .collect();
    assert_eq!(
        bad,
        vec![
            "time-conflict-event",
            "adjust-from-authenticated-time",
            "readjustment-interval",
            "anomaly-event-spacing"
        ],
        "{:#?}",
        b.findings
    );
    assert!(b
        .findings
        .iter()
        .filter(|f| !f.ok)
        .all(|f| !f.observations.is_empty()));
    assert!(b
        .verdict
        .contains("not a statement that the unit meets the regulation"));
    assert!(t::verify(
        "timestamp,position_auth\n2026-01-01T00:00:00Z,maybe\n2026-01-01T00:00:01Z,none\n",
        &Thresholds::default()
    )
    .is_err());
}

#[test]
fn catalogue_maps_the_rows_read() {
    let c = kshana_pro::regpacks::CATALOGUES
        .iter()
        .find(|c| c.0 == "eu-2021-1228-tachograph-osnma")
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(c.1).unwrap();
    let rows = v["rows"].as_array().unwrap();
    assert!(rows.iter().any(|r| r["clause"] == "Annex IC req. 86"));
    assert!(rows.iter().any(|r| r["clause"] == "clause to be confirmed"));
    let s = c.1.to_lowercase();
    for w in [concat!("compl", "iant"), concat!("certi", "fied")] {
        assert!(!s.contains(w));
    }
}

#[test]
fn cli_signs_the_pack() {
    let d = std::env::temp_dir().join(format!("b1403-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let home = d.join("home");
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
    let log = d.join("vu.csv");
    assert!(run(&["tacho", "synth", "bad", log.to_str().unwrap()]).0);
    let hex: String = [3u8; 32].iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(d.join("k.txt"), format!("private_key={hex}\n")).unwrap();
    let out = d.join("pack");
    let (o, k) = (
        out.to_str().unwrap().to_string(),
        d.join("k.txt").to_str().unwrap().to_string(),
    );
    let args = [
        "tacho",
        "verify",
        log.to_str().unwrap(),
        "--out",
        &o,
        "--signer",
        "t",
        "--key-file",
        &k,
    ];
    assert!(!run(&args).0);
    let mut a = args.to_vec();
    a.push("--dev");
    let (ok, m) = run(&a);
    assert!(ok, "{m}");
    let v = kshana_pro::signed_dossier::verify(
        &out,
        &[kshana_pro::license::public_key_for(&[3u8; 32])],
    )
    .unwrap();
    assert!(v.passed());
}
