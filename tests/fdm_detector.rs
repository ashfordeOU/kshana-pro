// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! FDM GNSS jamming/spoofing detector (W1119 C5) on synthetic flights with known truth.

use kshana_pro::fdm::{analyse, synth, synth::Flight, Params};
use kshana_pro::forensics::inputs::parse_utc;
use kshana_pro::ievents::{self, Kind};

fn run(f: Flight) -> kshana_pro::fdm::FdmReport {
    analyse("TEST1", &synth::make(f), &Params::default()).expect("analyses")
}

fn onset(e: &kshana_pro::fdm::FdmEvent) -> f64 {
    parse_utc(&e.start_utc).unwrap() - synth::T0
}

#[test]
fn clean_flight_has_no_event() {
    let r = run(Flight::Clean);
    assert!(r.events.is_empty(), "{:#?}", r.events);
    assert_eq!(r.rows, 2400);
    // The GNSS-baro offset of the synthetic day (350 ft) is recovered.
    assert!((r.gnss_baro_offset_ft.unwrap() - 350.0).abs() < 1.0);
    assert_eq!(r.sib.jamming_events + r.sib.spoofing_events, 0);
}

#[test]
fn jamming_is_integrity_loss_without_spoofing_indicators() {
    let r = run(Flight::Jamming);
    assert_eq!(r.events.len(), 1, "{:#?}", r.events);
    let e = &r.events[0];
    assert_eq!(e.kind, Kind::Jamming);
    assert!((onset(e) - synth::EV.0).abs() < 1.5);
    assert!((e.duration_s - (synth::EV.1 - synth::EV.0 - 1.0)).abs() < 2.0);
    assert_eq!(e.indicators.len(), 1, "{:?}", e.indicators);
    assert_eq!(e.indicators["integrity_loss_rows"].rows, 300);
    assert_eq!(r.sib.jamming_events, 1);
}

#[test]
fn spoofing_shows_every_independent_indicator() {
    let r = run(Flight::Spoofing);
    // The jump in at the start and out at the end are inside one merged event.
    assert_eq!(r.events.len(), 1, "{:#?}", r.events);
    let e = &r.events[0];
    assert_eq!(e.kind, Kind::Spoofing);
    assert!((onset(e) - synth::EV.0).abs() < 1.5);
    for k in [
        "position_jump_implied_kt",
        "gnss_time_jump_s",
        "gnss_baro_disagreement_ft",
        "gnss_ins_gs_disagreement_kt",
    ] {
        assert!(
            e.indicators.contains_key(k),
            "missing {k}: {:?}",
            e.indicators
        );
    }
    assert!((e.indicators["gnss_time_jump_s"].worst - 30.0).abs() < 0.5);
    assert!((e.indicators["gnss_baro_disagreement_ft"].worst - 2000.0).abs() < 2.0);
    assert!((e.indicators["gnss_ins_gs_disagreement_kt"].worst - 120.0).abs() < 0.5);
    assert!(
        !e.indicators.contains_key("integrity_loss_rows"),
        "integrity looked normal"
    );
    assert!(e.confidence > 0.9);
    assert_eq!(r.sib.spoofing_events, 1);
}

#[test]
fn a_real_holding_pattern_is_not_a_circle_spoof() {
    let r = run(Flight::Holding);
    assert!(r.events.is_empty(), "{:#?}", r.events);
}

#[test]
fn circle_pattern_against_a_straight_inertial_track_is_spoofing() {
    let r = run(Flight::CircleSpoof);
    assert!(!r.events.is_empty());
    let e = &r.events[0];
    assert_eq!(e.kind, Kind::Spoofing);
    assert!(
        e.indicators.contains_key("circle_gnss_course_rotation_deg"),
        "{:?}",
        e.indicators
    );
    assert!(e.indicators["circle_gnss_course_rotation_deg"].worst >= 360.0);
}

#[test]
fn unified_events_validate_and_the_summary_cites_the_sib() {
    let r = run(Flight::Spoofing);
    assert_eq!(r.unified_events.len(), 1);
    for ev in &r.unified_events {
        assert!(
            ievents::validate(ev).is_empty(),
            "{:?}",
            ievents::validate(ev)
        );
        assert_eq!(ev.observers, ["aircraft:TEST1"]);
    }
    assert!(r.sib.bulletin.contains("2022-02R4"));
    assert!(r.sib.note.contains("illustrative until confirmed"));
    assert_eq!(r.sib.events_below_10000_ft, 0);
}

#[test]
fn missing_columns_are_named_in_the_limits() {
    let csv: String = synth::make(Flight::Clean)
        .lines()
        .map(|l| l.split(',').take(7).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\n");
    let r = analyse("X", &csv, &Params::default()).unwrap();
    assert!(r.limits.iter().any(|l| l.contains("ins_gs_kt")));
    assert!(r.limits.iter().any(|l| l.contains("gnss_utc")));
}

#[test]
fn signed_report_directory_verifies() {
    let csv = synth::make(Flight::Jamming);
    let r = analyse("T", &csv, &Params::default()).unwrap();
    let dir = std::env::temp_dir().join(format!("kp-fdm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    kshana_pro::fdm::write_dir(&dir, &r, &csv).unwrap();
    let seed = [5u8; 32];
    kshana_pro::signed_dossier::sign(&dir, &seed, "T", "2026-11-19T00:00:00Z").unwrap();
    let v = kshana_pro::signed_dossier::verify(&dir, &[kshana_pro::license::public_key_for(&seed)])
        .unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    let _ = std::fs::remove_dir_all(&dir);
}
