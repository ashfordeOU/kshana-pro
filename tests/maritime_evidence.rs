// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Maritime incident evidence pack (W1119 C3): synthetic scenarios of each class with the
//! expected verdicts, the labels, the unified events and the signed pack.

use kshana_pro::forensics::maritime::{
    analyse, synth, synth::Case, write_pack, Answer, Conclusion, EventType, Label,
};
use kshana_pro::ievents;

fn run(case: Case) -> kshana_pro::forensics::maritime::EvidencePack {
    analyse(&synth::inputs(case)).expect("analyses")
}

#[test]
fn clean_voyage_has_no_event() {
    let p = run(Case::Clean);
    assert!(p.episodes.is_empty(), "{:#?}", p.episodes);
    assert_eq!(p.verdict.conclusion, Conclusion::NoEvent);
    assert_eq!(p.verdict.gnss_interference, Answer::NotIndicated);
    assert_eq!(p.verdict.ais_manipulation, Answer::NotIndicated);
    assert!(p.record.has_log_speed && p.record.has_heading);
    assert_eq!(p.record.nmea_epochs, 1800);
    let b = p.record.baseline_cn0_dbhz.unwrap();
    assert!((40.0..43.0).contains(&b), "baseline {b}");
}

#[test]
fn area_jamming_is_a_victim_with_corroborating_neighbours() {
    let p = run(Case::AreaJamming);
    assert_eq!(p.episodes.len(), 1, "{:#?}", p.episodes);
    let e = &p.episodes[0];
    assert_eq!(e.event_type, EventType::Jamming);
    assert!(e.metrics["cn0_drop_db"] >= 8.0);
    assert!(e.metrics["no_fix_s"] > 300.0);
    // Onset bounded within a few seconds of the true onset (600 s) and the 8 dB bar crossed by
    // the 22 dB / 60 s ramp at about 22 s.
    let onset = kshana_pro::forensics::inputs::parse_utc(&e.start_utc).unwrap() - synth::T0;
    assert!((600.0..640.0).contains(&onset), "onset {onset}");
    assert_eq!(e.ais_vs_own_nmea, "silent");
    assert_eq!(e.neighbours_present.len(), 4);
    assert_eq!(e.neighbours_corroborating.len(), 4);
    assert_eq!(e.conclusion, Conclusion::VictimOfExternalInterference);
    assert_eq!(
        p.verdict.conclusion,
        Conclusion::VictimOfExternalInterference
    );
    assert_eq!(p.verdict.gnss_interference, Answer::Indicated);
    assert_eq!(p.verdict.ais_manipulation, Answer::NotIndicated);
}

#[test]
fn area_spoofing_is_a_victim_ais_consistent_and_clustered() {
    let p = run(Case::AreaSpoofing);
    assert_eq!(p.episodes.len(), 1, "{:#?}", p.episodes);
    let e = &p.episodes[0];
    assert_eq!(e.event_type, EventType::Spoofing);
    // The jump to the false position (≈ 20 km) and the C/N0 signature.
    assert!(e.metrics["position_jump_m"] > 5_000.0, "{:?}", e.metrics);
    assert!(e.metrics["cn0_spread_db"] < 1.5);
    assert!(e.metrics["cn0_rise_db"] >= 4.0);
    assert!(e.metrics["sog_minus_stw_kn"] > 4.0);
    // The false position is found within the cluster radius of the truth.
    let off = e.metrics["false_position_offset_m"];
    let [la, lo] = e.last_clean_position.unwrap();
    let truth =
        kshana::interference_map::grid::haversine_m(la, lo, synth::FALSE_POS.0, synth::FALSE_POS.1);
    assert!((off - truth).abs() < 500.0, "offset {off} vs {truth}");
    assert_eq!(e.ais_vs_own_nmea, "consistent");
    assert_eq!(
        e.neighbours_corroborating.len(),
        4,
        "{:?}",
        e.neighbours_corroborating
    );
    assert_eq!(e.conclusion, Conclusion::VictimOfExternalInterference);
    assert_eq!(p.verdict.ais_manipulation, Answer::NotIndicated);
    assert!(e.confidence > 0.9);
}

#[test]
fn local_spoofing_is_localised_to_the_vessel() {
    let p = run(Case::LocalSpoofing);
    assert_eq!(p.episodes.len(), 1);
    let e = &p.episodes[0];
    assert_eq!(e.event_type, EventType::Spoofing);
    assert_eq!(e.neighbours_present.len(), 4);
    assert!(e.neighbours_corroborating.is_empty());
    assert_eq!(e.conclusion, Conclusion::LocalisedToVessel);
    assert_eq!(p.verdict.conclusion, Conclusion::LocalisedToVessel);
}

#[test]
fn ais_manipulation_with_clean_nmea_points_to_the_report() {
    let p = run(Case::AisManipulation);
    assert_eq!(p.episodes.len(), 1, "{:#?}", p.episodes);
    let e = &p.episodes[0];
    assert_eq!(e.event_type, EventType::AisManipulation);
    assert!(e.metrics["ais_minus_nmea_max_m"] > 39_000.0);
    // 600 s of fabricated reports every 10 s.
    assert!((55.0..=61.0).contains(&e.metrics["ais_reports_discrepant"]));
    assert!(e.neighbours_corroborating.is_empty());
    assert_eq!(p.verdict.conclusion, Conclusion::AisManipulation);
    assert_eq!(p.verdict.ais_manipulation, Answer::Indicated);
    assert_eq!(p.verdict.gnss_interference, Answer::NotIndicated);
    assert!(p
        .findings
        .iter()
        .any(|f| f.label == Label::Reported && f.source.starts_with("AIS SUBJ")));
}

#[test]
fn ship_side_fault_is_not_interference() {
    let p = run(Case::ShipSideFault);
    assert_eq!(p.episodes.len(), 1, "{:#?}", p.episodes);
    let e = &p.episodes[0];
    assert_eq!(e.event_type, EventType::ShipSideFault);
    assert_eq!(e.conclusion, Conclusion::ShipSideFault);
    assert_eq!(e.ais_vs_own_nmea, "no_own_fix");
    assert_eq!(p.verdict.conclusion, Conclusion::ShipSideFault);
    assert_eq!(p.verdict.gnss_interference, Answer::NotIndicated);
}

#[test]
fn findings_carry_all_three_labels_and_events_validate() {
    let p = run(Case::AreaSpoofing);
    for l in [Label::Measured, Label::Derived, Label::Reported] {
        assert!(p.findings.iter().any(|f| f.label == l), "no {l:?} finding");
    }
    assert_eq!(p.events.len(), 1);
    for ev in &p.events {
        assert!(
            ievents::validate(ev).is_empty(),
            "{:?}",
            ievents::validate(ev)
        );
        assert_eq!(ev.kind, ievents::Kind::Spoofing);
        assert!(ev.corroborated);
        assert_eq!(ev.evidence.len(), 2);
    }
    assert!(p.timeline.len() >= 3);
    assert!(p.timeline.windows(2).all(|w| w[0].utc <= w[1].utc));
    let txt = serde_json::to_string(&p).unwrap().to_lowercase();
    assert!(!txt.contains("court-grade") && !txt.contains("court grade"));
    assert!(p.limits.len() >= 5);
}

#[test]
fn no_neighbours_leaves_the_question_open() {
    let mut i = synth::inputs(Case::LocalSpoofing);
    i.ais = None;
    let p = analyse(&i).unwrap();
    assert_eq!(p.episodes[0].conclusion, Conclusion::Inconclusive);
    assert_eq!(p.verdict.ais_manipulation, Answer::Undetermined);
}

#[test]
fn a_station_event_corroborates_a_local_episode() {
    let mut i = synth::inputs(Case::LocalSpoofing);
    let p0 = analyse(&i).unwrap();
    let [la, lo] = p0.episodes[0].last_clean_position.unwrap();
    i.station = vec![ievents::Event {
        format: ievents::FORMAT.into(),
        id: "station-1".into(),
        start_utc: p0.episodes[0].start_utc.clone(),
        end_utc: p0.episodes[0].end_utc.clone(),
        systems: vec![ievents::System::Gnss { band: "L1".into() }],
        kind: ievents::Kind::Spoofing,
        location: Some(ievents::Place {
            lat_deg: la,
            lon_deg: lo,
            radius_m: 5000.0,
        }),
        area: None,
        observers: vec!["station:A".into()],
        metrics: Default::default(),
        source_estimate: None,
        confidence: 0.8,
        basis: "test".into(),
        evidence: vec![],
        reporter: "test".into(),
        corroborated: false,
    }];
    let p = analyse(&i).unwrap();
    assert!(p.episodes[0].station_corroborates);
    assert_eq!(
        p.episodes[0].conclusion,
        Conclusion::VictimOfExternalInterference
    );
}

#[test]
fn signed_pack_verifies_and_detects_tampering() {
    let i = synth::inputs(Case::AreaJamming);
    let p = analyse(&i).unwrap();
    let dir = std::env::temp_dir().join(format!("kp-maritime-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_pack(&dir, &p, &i).unwrap();
    let seed = [7u8; 32];
    kshana_pro::signed_dossier::sign(&dir, &seed, "Test Signer", "2026-11-19T00:00:00Z").unwrap();
    let pk = kshana_pro::license::public_key_for(&seed);
    let v = kshana_pro::signed_dossier::verify(&dir, &[pk]).unwrap();
    assert!(v.passed(), "{:?}", v.problems);
    for f in [
        "evidence-pack.json",
        "events.json",
        "events.geojson",
        "timeline.csv",
        "inputs/own.nmea",
    ] {
        assert!(dir.join(f).exists(), "{f}");
    }
    std::fs::write(dir.join("inputs/ais.csv"), "tampered").unwrap();
    let v = kshana_pro::signed_dossier::verify(&dir, &[pk]).unwrap();
    assert!(!v.passed());
    let _ = std::fs::remove_dir_all(&dir);
}
