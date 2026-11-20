// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Aviation operational analytics (`kshana_pro::aviation`, docs/AVIATION-ANALYTICS.md):
//! approach availability and go-around exposure on a one-hour terminal outage at a GNSS-only
//! airport, oceanic RNP support after spoofing, residual effects across consecutive flights,
//! nuisance-alert counts worked by hand, unpublished areas and short bursts with the
//! minimum-evidence rule, signing and the licence gates. All inputs are synthetic.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::aviation::{self, approach, nuisance, residual, synth, unpublished, Inputs};
use kshana_pro::license::{self, Entitlement, Tier};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");
const SEED: [u8; 32] = [9u8; 32];
const UTC: &str = "2026-11-20T12:00:00Z";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-aviation-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn approach_result() -> approach::ApproachResult {
    let req: approach::Request = toml::from_str(&synth::approach_request()).unwrap();
    approach::analyse(
        &req,
        &[("adsb.csv".into(), synth::approach_adsb())],
        &[("map.geojson".into(), synth::approach_map())],
        &Inputs::default(),
    )
    .unwrap()
}

fn t(s: &str) -> f64 {
    aviation::utc(s).unwrap()
}

#[test]
fn one_hour_outage_at_a_gnss_only_airport_is_found() {
    let r = approach_result();
    assert_eq!(r.outage_windows.len(), 1, "{:#?}", r.outage_windows);
    let w = &r.outage_windows[0];
    let (s, e) = (synth::T0 + synth::OUTAGE.0, synth::T0 + synth::OUTAGE.1);
    assert!(
        (t(&w.start_utc) - s).abs() <= 600.0,
        "start {}",
        w.start_utc
    );
    assert!((t(&w.end_utc) - e).abs() <= 600.0, "end {}", w.end_utc);
    assert!((50.0..=70.0).contains(&w.duration_min));
    assert!(w.degraded_aircraft >= 10);
    // Both GNSS procedures lose the same minutes; availability below 80 % over ~4 h observed.
    assert_eq!(r.procedures.len(), 2);
    for p in &r.procedures {
        assert!(p.gnss_dependent);
        assert!((p.unavailable_min - w.duration_min).abs() < 1e-6);
        let a = p.availability.unwrap();
        assert!(a > 0.6 && a < 0.85, "availability {a}");
    }
    // Final-segment exposure from the open map: the airport cell is degraded.
    let rnp27 = &r.procedures[0];
    assert_eq!(rnp27.final_segment_map_exposure.len(), 1);
    assert!(rnp27.final_segment_map_exposure[0].share_degraded > 0.99);
    assert!(r.procedures[1].final_segment_map_exposure.is_empty());
    assert!(r.fallback.gnss_only_airport);
    assert_eq!(r.fallback.outage_min_with_conventional_fallback, 0.0);
    assert!(r
        .risk_factors
        .iter()
        .any(|f| f.contains("GNSS-only airport")));
}

#[test]
fn go_arounds_are_split_by_degradation() {
    let r = approach_result();
    let g = &r.go_around_exposure;
    assert_eq!(g.go_arounds.len(), 4, "{:#?}", g.go_arounds);
    assert_eq!(g.under_degradation.go_arounds, 3);
    assert_eq!(g.otherwise.go_arounds, 1);
    // Every arrival is one approach (departures are not).
    assert_eq!(
        g.under_degradation.approaches + g.otherwise.approaches,
        synth::ARRIVALS
    );
    assert!((12..=14).contains(&g.under_degradation.approaches));
    let ratio = g.rate_ratio.unwrap();
    assert!(ratio > 5.0, "ratio {ratio}");
    let (lo, hi) = g.under_degradation.ci95.unwrap();
    assert!(lo < g.under_degradation.rate.unwrap() && hi > g.under_degradation.rate.unwrap());
    for ga in &g.go_arounds {
        assert!(ga.lowest_agl_ft < 1_500.0 && ga.lowest_agl_ft > 500.0);
    }
    // Capacity: GNSS-only, fallback zero, about 12 arrivals lost in one hour.
    let c = r.capacity.as_ref().unwrap();
    assert_eq!(c.fallback_arrivals_per_hour, 0.0);
    assert!((c.arrivals_lost_modelled - 12.0).abs() <= 2.0, "{c:?}");
    assert!(c.arrivals_observed_in_outage >= 8);
}

#[test]
fn conventional_fallback_changes_the_capacity_model() {
    let mut req: approach::Request = toml::from_str(&synth::approach_request()).unwrap();
    req.procedures.push(approach::Procedure {
        id: "ILS 27".into(),
        kind: "ils".into(),
        runway: Some("27".into()),
        faf: None,
        threshold: None,
    });
    req.capacity.as_mut().unwrap().fallback_arrivals_per_hour = Some(8.0);
    let r = approach::analyse(
        &req,
        &[("adsb.csv".into(), synth::approach_adsb())],
        &[],
        &Inputs::default(),
    )
    .unwrap();
    assert!(!r.fallback.gnss_only_airport);
    let ils = r.procedures.iter().find(|p| p.id == "ILS 27").unwrap();
    assert!(!ils.gnss_dependent && ils.unavailable_min == 0.0);
    let c = r.capacity.unwrap();
    assert!((c.arrivals_lost_modelled - 4.0).abs() <= 1.0, "{c:?}");
    // Unknown procedure kinds are refused.
    req.procedures[0].kind = "magic".into();
    assert!(approach::analyse(&req, &[], &[], &Inputs::default()).is_err());
}

#[test]
fn oceanic_rnp_support_after_spoofing() {
    let r = approach_result();
    let o = r.oceanic.unwrap();
    assert_eq!(o.aircraft.len(), 3);
    for a in &o.aircraft {
        if a.aircraft == "OCN1" {
            assert!(a.spoofing_jumps >= 1);
            assert!((a.minutes_rnp_not_supported - 20.0).abs() <= 1.0, "{a:?}");
            assert!((a.minutes_not_supported_after_jump - 20.0).abs() <= 1.0);
        } else {
            assert_eq!(a.minutes_rnp_not_supported, 0.0, "{a:?}");
            assert_eq!(a.spoofing_jumps, 0);
        }
    }
    assert!(!o.rule.contains("complian"));
}

#[test]
fn clean_traffic_gives_no_outage() {
    let clean: String = synth::approach_adsb()
        .lines()
        .map(|l| match l.strip_suffix(",0,0") {
            Some(head) => format!("{head},8,9\n"),
            None => format!("{l}\n"),
        })
        .collect();
    let req: approach::Request = toml::from_str(&synth::approach_request()).unwrap();
    let r = approach::analyse(&req, &[("a".into(), clean)], &[], &Inputs::default()).unwrap();
    assert!(r.outage_windows.is_empty());
    assert_eq!(r.procedures[0].availability, Some(1.0));
    assert_eq!(r.go_around_exposure.under_degradation.approaches, 0);
    assert_eq!(r.go_around_exposure.otherwise.go_arounds, 4);
    assert!(r.go_around_exposure.rate_ratio.is_none());
}

fn residual_req(n: usize, alerts: bool, resets: bool) -> residual::Request {
    residual::Request {
        title: "t".into(),
        aircraft: "SYN-01".into(),
        flights: (1..=n)
            .map(|k| residual::FlightRef {
                id: format!("F{k}"),
                csv: format!("F{k}.csv"),
            })
            .collect(),
        alerts: alerts.then(|| "alerts.csv".into()),
        resets: resets.then(|| "resets.csv".into()),
        areas: vec![],
        params: residual::Params::default(),
    }
}

#[test]
fn residual_clock_and_alert_on_the_next_flight_recommend_a_reset() {
    let (flights, alerts, _) = synth::residual_case();
    let req = residual_req(2, true, false);
    let r =
        residual::analyse(&req, &flights[..2], Some(&alerts), None, &Inputs::default()).unwrap();
    assert_eq!(r.outcome, residual::RESET_RECOMMENDED, "{:#?}", r.reasons);
    assert!(r.flights[0].exposed && !r.flights[1].exposed);
    assert_eq!(r.flights[0].spoofing_events, 1);
    assert_eq!(r.clock_reference_s, Some(0.0));
    assert_eq!(r.flights[0].clock_offset_post_s, Some(30.0));
    let ind = |name: &str, f: &str| {
        r.indicators
            .iter()
            .any(|i| i.indicator == name && i.flight == f)
    };
    assert!(
        ind("clock_offset_after_exposure", "F1"),
        "same flight, after settling"
    );
    assert!(ind("clock_offset_after_exposure", "F2"), "next flight");
    assert!(
        ind("alert_without_interference", "F2"),
        "TAWS alert on the next flight"
    );
}

#[test]
fn a_reset_that_clears_the_effect_and_one_that_does_not() {
    let (flights, alerts, resets) = synth::residual_case();
    let r = residual::analyse(
        &residual_req(3, true, true),
        &flights,
        Some(&alerts),
        Some(&resets),
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.outcome, residual::NO_RESIDUAL, "{:#?}", r.reasons);
    assert!(r.indicators.iter().all(|i| !i.after_reset));
    // Now F3 keeps the wrong clock after the reset.
    let mut f3 = synth::FdmFlight::new(synth::T0 + 3.0 * 86_400.0);
    f3.clock_offset_s = 30.0;
    let mut fl = flights.clone();
    fl[2].1 = f3.csv();
    let r = residual::analyse(
        &residual_req(3, true, true),
        &fl,
        Some(&alerts),
        Some(&resets),
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.outcome, residual::LRU_CHECK, "{:#?}", r.reasons);
    assert!(r
        .indicators
        .iter()
        .any(|i| i.flight == "F3" && i.after_reset));
}

#[test]
fn degradation_after_leaving_the_area_and_clean_histories() {
    let (csv, area) = synth::area_exit_case();
    let mut req = residual_req(1, false, false);
    req.areas = vec![area];
    let r = residual::analyse(
        &req,
        &[("F1".into(), csv.clone())],
        None,
        None,
        &Inputs::default(),
    )
    .unwrap();
    let i = r
        .indicators
        .iter()
        .find(|i| i.indicator == "degradation_after_area_exit")
        .expect("flagged");
    assert!(i.value > 400.0 && i.value < 520.0, "{i:?}");
    assert_eq!(r.outcome, residual::RESET_RECOMMENDED);
    // A wide area covering the whole flight: no exit, nothing flagged.
    req.areas = vec![[50.0, 10.0, 65.0, 40.0]];
    let r = residual::analyse(&req, &[("F1".into(), csv)], None, None, &Inputs::default()).unwrap();
    assert!(r.indicators.is_empty());
    assert_eq!(r.outcome, residual::NO_RESIDUAL);
    // No exposure at all.
    let clean = synth::FdmFlight::new(synth::T0).csv();
    let r = residual::analyse(
        &residual_req(1, false, false),
        &[("F1".into(), clean)],
        None,
        None,
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.outcome, residual::NO_EXPOSURE);
}

fn nuisance_result() -> nuisance::NuisanceResult {
    let req: nuisance::Request = toml::from_str(
        "title = \"t\"\nalerts = [\"alerts.csv\"]\nexposure = [\"intervals.csv\"]\nhours = \"hours.csv\"\n",
    )
    .unwrap();
    nuisance::analyse(
        &req,
        &[("alerts.csv".into(), synth::NUISANCE_ALERTS.into())],
        &[("intervals.csv".into(), synth::NUISANCE_INTERVALS.into())],
        Some(synth::NUISANCE_HOURS),
        &Inputs::default(),
    )
    .unwrap()
}

#[test]
fn nuisance_rates_reproduce_from_the_logs() {
    let r = nuisance_result();
    let o = &r.overall;
    // Worked by hand from synth::NUISANCE_ALERTS (see the module comment there).
    assert_eq!(
        (
            o.alerts,
            o.stated_nuisance,
            o.stated_genuine,
            o.undispositioned,
            o.interference_attributed,
            o.likely_nuisance
        ),
        (9, 2, 2, 5, 3, 4)
    );
    assert_eq!(o.stated_nuisance_share, Some(0.5));
    assert_eq!(o.likely_nuisance_share, Some(0.4444));
    let r1 = &r.by_route["R1"];
    assert_eq!((r1.alerts, r1.likely_nuisance), (6, 3));
    assert_eq!(r1.crew_hours, Some(20.0));
    assert_eq!(r1.alerts_per_crew_hour, Some(0.3));
    assert_eq!(r1.likely_nuisance_per_crew_hour, Some(0.15));
    // Handling: TAWS 30 s + GNSS 20 s + TAWS 30 s = 80 s over 20 h.
    assert_eq!(r1.handling_min_per_crew_hour, Some(0.0667));
    let r2 = &r.by_route["R2"];
    assert_eq!((r2.alerts, r2.likely_nuisance), (3, 1));
    assert_eq!(r2.likely_nuisance_per_crew_hour, Some(0.0333));
    assert_eq!(r.by_class["taws"].alerts, 5);
    assert_eq!(r.by_class["other"].alerts, 1);
    assert_eq!(r.by_phase["cruise"].likely_nuisance, 4);
    // Training extract: R1 / cruise / taws first.
    let t0 = &r.training_extract[0];
    assert_eq!(
        (
            t0.route.as_str(),
            t0.phase.as_str(),
            t0.class.as_str(),
            t0.likely_nuisance
        ),
        ("R1", "cruise", "taws", 2)
    );
    let csv = nuisance::training_csv(&r);
    assert!(
        csv.starts_with("route,phase,class") && csv.lines().count() == 1 + r.training_extract.len()
    );
}

#[test]
fn nuisance_reads_unified_events_and_bridge_alarms() {
    let events = serde_json::json!([{
        "id": "e1", "start_utc": "2026-06-01T09:00:00Z", "end_utc": "2026-06-01T09:30:00Z",
        "systems": [{"system": "gnss", "band": "L1"}], "kind": "jamming",
        "observers": ["vessel:V1"], "confidence": 0.9, "basis": "test"
    }])
    .to_string();
    let alerts = "time,vessel,voyage,area,alert,disposition\n\
        2026-06-01T09:10:00Z,V1,V-A,coastal,GPS POSITION LOST,\n\
        2026-06-01T09:11:00Z,V1,V-A,coastal,BNWAS WATCH ALARM,\n\
        2026-06-01T11:00:00Z,V1,V-A,harbour,GPS POSITION LOST,\n";
    let req: nuisance::Request = toml::from_str("title = \"bridge\"\nalerts = [\"a\"]\n").unwrap();
    let r = nuisance::analyse(
        &req,
        &[("a".into(), alerts.into())],
        &[("e".into(), events)],
        None,
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.exposure_intervals, 1);
    assert_eq!(r.overall.alerts, 3);
    assert_eq!(r.overall.likely_nuisance, 2);
    assert_eq!(r.by_class["bnwas"].likely_nuisance, 1);
    assert_eq!(r.by_route["V-A"].crew_hours, None);
}

fn areas_req(extra: &str) -> unpublished::Request {
    toml::from_str(&format!("title = \"t\"\nadsb = [\"a\"]\n{extra}")).unwrap()
}

#[test]
fn short_burst_with_no_notam_is_unpublished() {
    let adsb = synth::burst_adsb(8, true, (32.75, -97.25));
    let r = unpublished::analyse(
        &areas_req(""),
        &[("a".into(), adsb)],
        &[],
        &[],
        &Inputs::default(),
    )
    .unwrap();
    let e = r
        .events
        .iter()
        .find(|e| e.cell[0] == 32.5 && e.cell[1] == -97.5)
        .expect("burst found");
    assert_eq!(e.status, "UNPUBLISHED");
    assert!(e.short_burst && e.kind == "spoofing");
    assert_eq!(e.distinct_aircraft, 8);
    assert!(e.duration_s <= 300.0);
    // The jump back is located at the true position: no event in the false position's cell.
    assert_eq!(r.events.len(), 1, "{:#?}", r.events);
    assert!(r.unpublished == 1 && r.short_bursts == 1);
    let alerts = unpublished::alerts_table(&r);
    assert!(alerts.starts_with("t_unix,id,source,kind,severity,area,text"));
    let parsed = kshana_pro::alerts::parse_alerts(&alerts).unwrap();
    assert!(parsed.iter().any(|a| a.kind == "unpublished-spoofing"));
    let gj: serde_json::Value = serde_json::from_str(&unpublished::geojson(&r)).unwrap();
    assert_eq!(gj["features"].as_array().unwrap().len(), r.events.len());
    // No identity in the output.
    let out = serde_json::to_string(&r).unwrap();
    assert!(!out.contains("FLT0"));
}

#[test]
fn a_covering_notam_and_the_minimum_evidence_rule() {
    let adsb = synth::burst_adsb(6, false, (45.25, 10.25));
    let notam = format!(
        "A0001/26 NOTAMN\nQ) LIMM/QGWXX/IV/NBO/E/000/999/4515N01015E040\nA) LIMM B) {} C) {}\nE) GNSS MAY BE UNRELIABLE.\n",
        "2606010000", "2606020000"
    );
    let r = unpublished::analyse(
        &areas_req(""),
        &[("a".into(), adsb.clone())],
        &[],
        &[("notam.txt".into(), notam)],
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.advisories, 1, "{r:#?}");
    assert_eq!(r.events.len(), 1);
    assert_eq!(r.events[0].status, "COVERED");
    assert_eq!(r.events[0].kind, "jamming");
    // The same burst, stated advisory elsewhere: unpublished.
    let r = unpublished::analyse(
        &areas_req("[[advisories]]\nname = \"far\"\nbox = [10.0, 10.0, 11.0, 11.0]\n"),
        &[("a".into(), adsb)],
        &[],
        &[],
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.events[0].status, "UNPUBLISHED");
    // Four aircraft: below the publication minimum, counted but never located.
    let r = unpublished::analyse(
        &areas_req(""),
        &[("a".into(), synth::burst_adsb(4, false, (45.25, 10.25)))],
        &[],
        &[],
        &Inputs::default(),
    )
    .unwrap();
    assert!(r.events.is_empty() && r.windows_below_minimum > 0);
    // Finer cells or a lower minimum are refused.
    for extra in ["[params]\ncell_deg = 0.1\n", "[params]\nmin_aircraft = 3\n"] {
        assert!(
            unpublished::analyse(&areas_req(extra), &[], &[], &[], &Inputs::default()).is_err()
        );
    }
}

#[test]
fn fdm_events_add_evidence_to_areas() {
    // Five FDM events at one place (five aircraft), no ADS-B degradation.
    let ev: Vec<serde_json::Value> = (0..5)
        .map(|k| {
            serde_json::json!({
                "id": format!("fdm-{k}"), "start_utc": "2026-06-01T09:00:00Z", "end_utc": "2026-06-01T09:02:00Z",
                "systems": [{"system": "gnss", "band": "L1"}], "kind": "spoofing",
                "location": {"lat_deg": 50.1, "lon_deg": 20.1, "radius_m": 1852.0},
                "source_estimate": {"lat_deg": 1.0, "lon_deg": 1.0, "radius_m": 1.0},
                "observers": [format!("aircraft:F{k}")], "confidence": 0.9, "basis": "test"
            })
        })
        .collect();
    let r = unpublished::analyse(
        &areas_req(""),
        &[],
        &[(
            "events.json".into(),
            serde_json::Value::Array(ev).to_string(),
        )],
        &[],
        &Inputs::default(),
    )
    .unwrap();
    assert_eq!(r.events.len(), 1);
    assert_eq!(r.events[0].fdm_events, 5);
    assert_eq!(r.events[0].cell, [50.0, 20.0, 50.5, 20.5]);
}

#[test]
fn features_have_explicit_tiers() {
    for (f, tier) in [
        (aviation::FEATURE_APPROACH, Tier::Pro),
        (aviation::FEATURE_RESIDUAL, Tier::Pro),
        (aviation::FEATURE_NUISANCE, Tier::Pro),
        (aviation::FEATURE_AREAS, Tier::Enterprise),
    ] {
        assert_eq!(license::explicit_tier(f), Some(tier), "{f}");
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
    }
    let ent = |tier| Entitlement {
        licensee: "ACME".into(),
        tier,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&ent(Tier::Pro), aviation::FEATURE_APPROACH, 0).is_ok());
    assert!(license::check(&ent(Tier::Pro), aviation::FEATURE_AREAS, 0).is_err());
    assert!(license::check(&ent(Tier::Enterprise), aviation::FEATURE_AREAS, 0).is_ok());
}

fn cli(args: &[&str]) -> std::process::Output {
    Command::new(BIN)
        .args(args)
        .env_remove("KSHANA_PRO_LICENSE")
        .output()
        .unwrap()
}

#[test]
fn command_line_end_to_end() {
    let w = tmp("cli");
    let s = |p: &Path| p.display().to_string();
    let o = cli(&["aviation", "synth", &s(&w)]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let key = w.join("issuer.key");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let pk = hex::encode(license::public_key_for(&SEED));

    // Without a licence the commands are refused.
    let out = w.join("no-licence");
    let o = cli(&[
        "aviation",
        "approach",
        &s(&w.join("approach/request.toml")),
        "--out",
        &s(&out),
        "--signer",
        "Ops",
        "--key-file",
        &s(&key),
    ]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains(aviation::FEATURE_APPROACH));

    for (cmd, case, file, expect) in [
        (
            "approach",
            "approach",
            approach::RESULT_FILE,
            "outage window",
        ),
        (
            "residual",
            "residual",
            residual::RESULT_FILE,
            "Outcome: NO RESIDUAL EFFECT FOUND",
        ),
        (
            "nuisance",
            "nuisance",
            nuisance::RESULT_FILE,
            "4 likely nuisance",
        ),
        ("areas", "areas", unpublished::RESULT_FILE, "UNPUBLISHED"),
    ] {
        let out = w.join(format!("out-{cmd}"));
        let o = cli(&[
            "aviation",
            cmd,
            &s(&w.join(case).join("request.toml")),
            "--out",
            &s(&out),
            "--signer",
            "Ops",
            "--key-file",
            &s(&key),
            "--utc",
            UTC,
            "--dev",
        ]);
        assert!(
            o.status.success(),
            "{cmd}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let stdout = String::from_utf8_lossy(&o.stdout);
        assert!(stdout.contains(expect), "{cmd}: {stdout}");
        for f in [file, "summary.txt", "signature.json", "inputs/request.toml"] {
            assert!(out.join(f).exists(), "{cmd}: {f}");
        }
        let o = cli(&["aviation", "check", &s(&out), "--trust", &pk]);
        assert!(
            o.status.success(),
            "{cmd}: {}",
            String::from_utf8_lossy(&o.stdout)
        );
        let text = std::fs::read_to_string(out.join(file)).unwrap();
        assert!(text.contains("Decision support only") && text.contains("MODELLED"));
    }
    assert!(w.join("out-nuisance/training-extract.csv").exists());
    assert!(w.join("out-areas/alerts.csv").exists() && w.join("out-areas/areas.geojson").exists());
    // A covered burst and an unpublished one in the areas case.
    let a: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(w.join("out-areas").join(unpublished::RESULT_FILE)).unwrap(),
    )
    .unwrap();
    let st: Vec<&str> = a["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["status"].as_str().unwrap())
        .collect();
    assert!(
        st.contains(&"COVERED") && st.contains(&"UNPUBLISHED"),
        "{st:?}"
    );

    // Tampering breaks the signature.
    std::fs::write(w.join("out-approach/summary.txt"), "changed").unwrap();
    let o = cli(&[
        "aviation",
        "check",
        &s(&w.join("out-approach")),
        "--trust",
        &pk,
    ]);
    assert!(!o.status.success());
}

#[test]
fn results_are_reproducible_byte_for_byte() {
    let a = serde_json::to_string(&approach_result()).unwrap();
    let b = serde_json::to_string(&approach_result()).unwrap();
    assert_eq!(a, b);
    let n1 = serde_json::to_string(&nuisance_result()).unwrap();
    assert_eq!(n1, serde_json::to_string(&nuisance_result()).unwrap());
}
