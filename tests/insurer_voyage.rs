// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Equipment-resilience rating and voyage risk score (W1119 C7).

use std::path::{Path, PathBuf};

use kshana_pro::exposure::voyage::{
    declared_term, rate_equipment, voyage_risk, Declared, EquipmentRequest, VoyageRequest,
};
use kshana_pro::ievents::{self, Event, Kind, Place};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-voyage-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn receiver_card(dir: &Path, level: u8, misleading: u64) -> PathBuf {
    let p = dir.join(format!("rx-{level}-{misleading}.json"));
    let v = serde_json::json!({
        "format": kshana_pro::scorecards::FORMAT,
        "levels": {"highest_supported": level},
        "totals": {"misleading_unwarned_epochs": misleading}
    });
    std::fs::write(&p, v.to_string()).unwrap();
    p
}

fn eq_req(toml_src: &str, dir: &Path) -> EquipmentRequest {
    let p = dir.join("equipment.toml");
    std::fs::write(&p, toml_src).unwrap();
    EquipmentRequest::from_file(&p).unwrap()
}

#[test]
fn declarations_alone_never_reach_the_top_band() {
    let all = Declared {
        multi_constellation: true,
        multi_frequency: true,
        spoof_detection: true,
        jam_detection: true,
        osnma: true,
        crpa: true,
        holdover_h: 48.0,
    };
    assert_eq!(declared_term(&all, "primary_gnss").0, 50.0);
    assert_eq!(declared_term(&Declared::default(), "primary_gnss").0, 0.0);
    let d = tmp("decl");
    let r = eq_req(
        "vessel = \"V\"\n[[devices]]\nname = \"GNSS\"\nrole = \"primary_gnss\"\n[devices.declared]\nmulti_constellation = true\nmulti_frequency = true\nspoof_detection = true\nosnma = true\ncrpa = true\n[[devices]]\nname = \"eLoran\"\nrole = \"eloran\"\n[devices.declared]\nholdover_h = 24.0\n",
        &d,
    );
    let e = rate_equipment(&r, &[]).unwrap();
    assert_eq!(e.devices[0].basis, "DECLARED");
    assert_eq!(e.devices[0].score, 50.0);
    // eLoran declared: 20 (holdover) + 20 (independent) = 40.
    assert_eq!(e.devices[1].score, 40.0);
    assert_eq!(e.rating, 0.6 * 50.0 + 0.4 * 40.0);
    assert_eq!(e.band, "C");
}

#[test]
fn tested_evidence_lifts_the_rating_and_misleading_output_lowers_it() {
    let d = tmp("tested");
    let good = receiver_card(&d, 4, 0);
    let bad = receiver_card(&d, 4, 12);
    let log = kshana_pro::osnma_conformance::synth::log(true);
    let bytes = serde_json::to_vec(&log).unwrap();
    let card = kshana_pro::osnma_conformance::score(&log, &bytes, &Default::default()).unwrap();
    let os = d.join("osnma.json");
    std::fs::write(&os, serde_json::to_string(&card).unwrap()).unwrap();
    let src = |rx: &Path| {
        format!(
            "vessel = \"V\"\n[[devices]]\nname = \"GNSS\"\nrole = \"primary_gnss\"\nscorecard = \"{}\"\nosnma_scorecard = \"{}\"\n[[devices]]\nname = \"INS\"\nrole = \"inertial\"\nscorecard = \"{}\"\n",
            rx.display(),
            os.display(),
            receiver_card(&d, 2, 0).display()
        )
    };
    let a = rate_equipment(&eq_req(&src(&good), &d), &[]).unwrap();
    assert_eq!(a.devices[0].basis, "TESTED");
    assert_eq!(a.devices[0].score, 100.0, "100 + 10 OSNMA, clamped");
    assert_eq!(a.devices[1].score, 50.0);
    assert_eq!(a.rating, 80.0);
    assert_eq!(a.band, "A");
    let b = rate_equipment(&eq_req(&src(&bad), &d), &[]).unwrap();
    assert_eq!(b.devices[0].score, 90.0, "80 + 10");
    assert!(b.rating < a.rating);
}

#[test]
fn a_tampered_signed_scorecard_counts_for_nothing() {
    let d = tmp("signed");
    let sc = d.join("rx");
    std::fs::create_dir_all(&sc).unwrap();
    let v = serde_json::json!({"format": kshana_pro::scorecards::FORMAT, "levels": {"highest_supported": 3}, "totals": {}});
    std::fs::write(sc.join("receiver-scorecard.json"), v.to_string()).unwrap();
    let seed = [9u8; 32];
    kshana_pro::signed_dossier::sign(&sc, &seed, "Lab", "2026-11-19T00:00:00Z").unwrap();
    let pk = kshana_pro::license::public_key_for(&seed);
    let src = "vessel = \"V\"\n[[devices]]\nname = \"GNSS\"\nrole = \"primary_gnss\"\nscorecard = \"rx\"\n";
    let ok = rate_equipment(&eq_req(src, &d), &[pk]).unwrap();
    assert_eq!(ok.devices[0].score, 75.0);
    assert_eq!(ok.devices[0].sources[0].signer_trusted, Some(true));
    let v2 = serde_json::json!({"format": kshana_pro::scorecards::FORMAT, "levels": {"highest_supported": 4}, "totals": {}});
    std::fs::write(sc.join("receiver-scorecard.json"), v2.to_string()).unwrap();
    let bad = rate_equipment(&eq_req(src, &d), &[pk]).unwrap();
    assert_eq!(bad.devices[0].score, 0.0);
}

fn ev(id: &str, kind: Kind, lat: f64, lon: f64, start: &str, conf: f64) -> Event {
    Event {
        format: ievents::FORMAT.into(),
        id: id.into(),
        start_utc: start.into(),
        end_utc: start.into(),
        systems: vec![ievents::System::Gnss { band: "L1".into() }],
        kind,
        location: Some(Place {
            lat_deg: lat,
            lon_deg: lon,
            radius_m: 20_000.0,
        }),
        area: None,
        observers: vec!["x".into()],
        metrics: Default::default(),
        source_estimate: None,
        confidence: conf,
        basis: "test".into(),
        evidence: vec![],
        reporter: "test".into(),
        corroborated: false,
    }
}

fn voyage(waypoints: Vec<[f64; 2]>) -> VoyageRequest {
    VoyageRequest {
        title: "T".into(),
        vessel: "V".into(),
        waypoints,
        departure_utc: "2026-11-01T00:00:00Z".into(),
        events: vec![],
        equipment: None,
        corridor_km: 25.0,
        base_dir: None,
    }
}

/// Strait-of-Hormuz-like route east, and a clear route far south.
fn routes() -> (VoyageRequest, VoyageRequest) {
    (
        voyage(vec![[26.0, 55.0], [26.3, 56.4], [25.5, 57.5]]),
        voyage(vec![[15.0, 60.0], [15.0, 62.0]]),
    )
}

fn history() -> Vec<Event> {
    vec![
        ev(
            "hot-1",
            Kind::Spoofing,
            26.3,
            56.4,
            "2026-10-20T00:00:00Z",
            0.9,
        ),
        ev(
            "hot-2",
            Kind::Jamming,
            26.1,
            55.6,
            "2026-09-01T00:00:00Z",
            0.8,
        ),
        ev(
            "old",
            Kind::Spoofing,
            25.6,
            57.4,
            "2024-01-01T00:00:00Z",
            0.9,
        ),
        ev(
            "future",
            Kind::Spoofing,
            26.3,
            56.4,
            "2026-12-01T00:00:00Z",
            1.0,
        ),
    ]
}

#[test]
fn a_route_through_a_hot_area_scores_higher_than_a_clear_one() {
    let (hot, clear) = routes();
    let h = voyage_risk(&hot, &history(), Some(40.0)).unwrap();
    let c = voyage_risk(&clear, &history(), Some(40.0)).unwrap();
    assert!(h.risk > 0.0 && c.risk == 0.0, "{} {}", h.risk, c.risk);
    assert_eq!(c.contributions.len(), 0);
    // The event after departure is ignored; the recent spoofing event weighs most.
    assert_eq!(h.events_considered, 3);
    assert!(h.contributions.iter().all(|c| c.event_id != "future"));
    assert_eq!(h.contributions[0].event_id, "hot-1");
    // Recency: the 2024 event is weighted by exp(-age/180) ≈ 0.
    let old = h
        .contributions
        .iter()
        .find(|c| c.event_id == "old")
        .unwrap();
    assert!(old.weight < 0.01 * h.contributions[0].weight);
    assert!(h.explanation.len() >= 3);
}

#[test]
fn risk_falls_monotonically_with_the_equipment_rating_and_keeps_a_floor() {
    let (hot, _) = routes();
    let rs: Vec<f64> = [0.0, 25.0, 50.0, 75.0, 100.0]
        .iter()
        .map(|r| voyage_risk(&hot, &history(), Some(*r)).unwrap().risk)
        .collect();
    assert!(rs.windows(2).all(|w| w[1] < w[0]), "{rs:?}");
    let top = voyage_risk(&hot, &history(), Some(100.0)).unwrap();
    assert!((top.vulnerability - 0.2).abs() < 1e-12);
    assert!((rs[4] / rs[0] - 0.2).abs() < 0.01);
}

#[test]
fn exposure_is_bounded_and_grows_with_more_events() {
    let (hot, _) = routes();
    let mut many = history();
    for i in 0..200 {
        many.push(ev(
            &format!("e{i}"),
            Kind::Spoofing,
            26.3,
            56.4,
            "2026-10-30T00:00:00Z",
            1.0,
        ));
    }
    let a = voyage_risk(&hot, &history(), None).unwrap();
    let b = voyage_risk(&hot, &many, None).unwrap();
    assert!(b.exposure > a.exposure && b.exposure <= 1.0);
    assert!(b.risk <= 100.0);
    assert_eq!(a.vulnerability, 1.0);
}

#[test]
fn a_wider_corridor_catches_more() {
    let (_, mut clear) = routes();
    let far = vec![ev(
        "near-ish",
        Kind::Jamming,
        15.6,
        61.0,
        "2026-10-25T00:00:00Z",
        1.0,
    )];
    assert_eq!(
        voyage_risk(&clear, &far, None).unwrap().contributions.len(),
        0
    );
    clear.corridor_km = 60.0;
    assert_eq!(
        voyage_risk(&clear, &far, None).unwrap().contributions.len(),
        1
    );
}
