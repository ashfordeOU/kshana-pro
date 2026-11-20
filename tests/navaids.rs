// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Terrestrial navaids (backlog E37: B3703) on a SYNTHETIC network built here (invented
//! facility positions, not a real State's network): coverage against the published service
//! volumes and the ICAO Doc 9613 DME/DME criteria, critical DMEs, RNP scenarios with GNSS
//! loss, outages and an inertial coast, the NACp backup test, and flight-inspection truth
//! assurance with a corrupted truth and a facility deviation.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::navaids::{
    self, coverage_radius_nm, dist_bearing, dmes_in_view, inspection, rnp, Kind, Station, FT, NM,
};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e37n-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(home: &Path, args: &[&str]) -> (bool, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
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
}

fn st(id: &str, kind: Kind, lat: f64, lon: f64, ssv: Option<&str>) -> Station {
    Station {
        id: id.into(),
        kind,
        lat_deg: lat,
        lon_deg: lon,
        elev_ft: 300.0,
        ssv: ssv.map(String::from),
        doc_nm: None,
        doc_ft: None,
        out: false,
    }
}

/// Five DMEs on a ring of about 60 NM around (50 N, 10 E) and a high VOR/DME at the centre.
fn network() -> Vec<Station> {
    let mut v = vec![st("CTR", Kind::VorDme, 50.0, 10.0, Some("H"))];
    for k in 0..5 {
        let a = (k as f64 * 72.0).to_radians();
        v.push(st(
            &format!("D{k}"),
            Kind::Dme,
            50.0 + 1.0 * a.cos(),
            10.0 + 1.0 / 50f64.to_radians().cos() * a.sin(),
            None,
        ));
    }
    v
}

#[test]
fn coverage_radius_matches_the_published_service_volumes() {
    let h = st("H", Kind::Vor, 50.0, 10.0, Some("H"));
    let t = st("T", Kind::Vor, 50.0, 10.0, Some("T"));
    // FAA AIM 1-1-8 Table 1-1-1 (altitudes above the facility at 300 ft).
    assert_eq!(coverage_radius_nm(&h, 30_300.0), Some(130.0));
    assert_eq!(coverage_radius_nm(&h, 10_300.0), Some(40.0));
    assert_eq!(coverage_radius_nm(&h, 16_300.0), Some(100.0));
    assert_eq!(coverage_radius_nm(&t, 3_300.0), Some(25.0));
    assert_eq!(coverage_radius_nm(&t, 13_300.0), None);
    // No stated volume: line of sight, 1.23 (√h_ant + √h_ac) NM; antenna 30 ft above the site.
    let dme = st("D", Kind::Dme, 50.0, 10.0, None);
    let r = coverage_radius_nm(&dme, 10_300.0).unwrap();
    let aim = 1.23 * (30f64.sqrt() + 10_000f64.sqrt());
    assert!((r - aim).abs() < 1.5, "{r} vs {aim}");
    // A stated DOC overrides the SSV and its ceiling applies.
    let mut doc = h.clone();
    doc.doc_nm = Some(60.0);
    doc.doc_ft = Some(25_000.0);
    assert_eq!(coverage_radius_nm(&doc, 20_000.0), Some(60.0));
    assert_eq!(coverage_radius_nm(&doc, 30_000.0), None);
}

#[test]
fn dme_dme_criteria_of_doc_9613() {
    // Two DMEs 100 NM north and south of the point: 180° apart, no pair.
    let n = st("N", Kind::Dme, 50.0 + 100.0 / 60.0, 10.0, None);
    let s = st("S", Kind::Dme, 50.0 - 100.0 / 60.0, 10.0, None);
    let e = st(
        "E",
        Kind::Dme,
        50.0,
        10.0 + 100.0 / 60.0 / 50f64.to_radians().cos(),
        None,
    );
    let v = dmes_in_view(&[n.clone(), s.clone()], 50.0, 10.0, 35_000.0, None);
    assert_eq!(v.len(), 2);
    assert!(navaids::best_pair(&[n.clone(), s.clone()], &v).is_none());
    let net = [n.clone(), s, e];
    let v = dmes_in_view(&net, 50.0, 10.0, 35_000.0, None);
    let (e2drms, pair) = navaids::best_pair(&net, &v).unwrap();
    assert!(pair.0 == "E" || pair.1 == "E");
    // Two 100 NM ranges at 90°: σ = √(0.125² + 0.05²) NM each, 2drms = 2√2 σ.
    let sig = (0.125f64.powi(2) + 0.05f64.powi(2)).sqrt();
    assert!(
        (e2drms / NM - 2.0 * 2f64.sqrt() * sig).abs() < 0.01,
        "{}",
        e2drms / NM
    );
    // Beyond 160 NM: not used, even with line of sight at FL450.
    let far = st("F", Kind::Dme, 50.0 + 170.0 / 60.0, 10.0, None);
    assert!(dmes_in_view(&[far], 50.0, 10.0, 45_000.0, None).is_empty());
    // Overhead (elevation ≥ 40°) and closer than 3 NM: not used.
    assert!(dmes_in_view(
        std::slice::from_ref(&n),
        50.0 + 100.0 / 60.0,
        10.0,
        20_000.0,
        None
    )
    .is_empty());
}

#[test]
fn coverage_twin_and_critical_dmes_cli() {
    let d = tmp("cov");
    let req = navaids::CoverageRequest {
        name: "synthetic ring".into(),
        stations: network(),
        altitude_ft: 25_000.0,
        area: [48.5, 51.5, 7.5, 12.5],
        step_deg: 0.25,
        max_2drms_nm: Some(1.0),
    };
    let r = navaids::coverage(&req).unwrap();
    assert!(r.dme_dme_fraction > 0.6, "{}", r.dme_dme_fraction);
    // The VOR/DME (H, 130 NM at FL250) covers the whole 3 × 5 degree box... within 130 NM.
    let centre = r
        .grid
        .iter()
        .find(|g| (g.lat_deg - 50.0).abs() < 1e-9 && (g.lon_deg - 10.0).abs() < 1e-9)
        .unwrap();
    assert!(centre.dme_dme && centre.vor_dme);
    for g in &r.grid {
        let dist = dist_bearing(g.lat_deg, g.lon_deg, 50.0, 10.0).0 / NM;
        if dist < 128.0 {
            assert!(g.vor_dme, "{g:?}");
        } else if dist > 132.0 {
            assert!(!g.vor_dme, "{g:?}");
        }
    }
    assert_eq!(r.critical.len(), 6);
    // With D0 out the twin reports less coverage.
    let mut out = req.clone();
    out.stations[1].out = true;
    let r2 = navaids::coverage(&out).unwrap();
    assert!(r2.dme_dme_fraction <= r.dme_dme_fraction);

    // CLI with JSON and CSV.
    std::fs::write(d.join("cov.toml"), toml::to_string(&req).unwrap()).unwrap();
    let (ok, text) = run(
        &d,
        &[
            "navaids",
            "coverage",
            d.join("cov.toml").to_str().unwrap(),
            "--json",
            d.join("cov.json").to_str().unwrap(),
            "--csv",
            d.join("cov.csv").to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    assert!(text.contains("MODELLED"));
    let csv = std::fs::read_to_string(d.join("cov.csv")).unwrap();
    assert_eq!(csv.lines().count(), r.points + 1);
    // Licence: refused at Evaluation tier; the SSV table is free.
    let (ok, _) = run(
        &d,
        &["navaids", "coverage", d.join("cov.toml").to_str().unwrap()],
    );
    assert!(!ok);
    let (ok, text) = run(&d, &["navaids", "ssv"]);
    assert!(ok && text.contains("130 NM"), "{text}");
}

fn rnp_request() -> rnp::RnpRequest {
    toml::from_str(&format!(
        r#"{}
[route]
name = "synthetic route"
points = [[49.0, 8.0], [50.0, 10.0], [51.0, 12.5]]
altitude_ft = 25000
ground_speed_kt = 420
sample_nm = 5
nav_spec = "RNAV 1"
rnp_nm = 1.0
fte_95_nm = 0.5
gnss_nse_95_nm = 0.03
vor_sigma_deg = 1.4
required_nacp = 8

[[scenario]]
name = "nominal"

[[scenario]]
name = "GNSS lost"
gnss = false

[[scenario]]
name = "GNSS lost, DMEs out"
gnss = false
out = ["D0", "D1", "D2", "D3", "D4"]
irs_drift_nm_per_h = 2.0

[[scenario]]
name = "GNSS lost, nothing but inertial"
gnss = false
dme_dme = false
vor_dme = false
gnss_out_nm = [40.0, 400.0]
irs_drift_nm_per_h = 4.0

[[scenario]]
name = "e-conspicuity backup"
gnss = false
dme_dme = false
vor_dme = false
backup = {{ name = "mobile-network", epu_95_m = 50.0 }}
"#,
        network()
            .iter()
            .map(|s| format!("[[station]]\n{}", toml::to_string(s).unwrap()))
            .collect::<Vec<_>>()
            .join("\n")
    ))
    .unwrap()
}

#[test]
fn rnp_scenarios_with_gnss_loss_outages_coast_and_backup() {
    let req = rnp_request();
    let r = rnp::run(&req).unwrap();
    let sc = |n: &str| r.scenarios.iter().find(|s| s.name == n).unwrap();
    let nominal = sc("nominal");
    assert_eq!(nominal.meets_rnp_fraction, 1.0);
    assert_eq!(
        nominal.nacp_min, 8,
        "0.03 NM = 55.6 m: NACp 8 (EPU < 92.6 m)"
    );
    let lost = sc("GNSS lost");
    assert!(lost.sensor_share.iter().any(|(s, _)| s == "dme_dme"));
    assert!(lost.meets_rnp_fraction > 0.5, "{}", lost.meets_rnp_fraction);
    assert!(
        lost.meets_nacp_fraction.unwrap() < 0.5,
        "DME/DME cannot carry NACp 8"
    );
    // Ring DMEs out: VOR/DME at the centre, then nothing near the ends.
    let out = sc("GNSS lost, DMEs out");
    assert!(out.sensor_share.iter().any(|(s, _)| s == "vor_dme"));
    assert!(out.meets_rnp_fraction < lost.meets_rnp_fraction);
    // Pure inertial after GNSS goes at 40 NM: the coast grows and alerts later.
    let ins = sc("GNSS lost, nothing but inertial");
    assert!(ins.longest_irs_coast_nm > 150.0);
    let first = ins.first_alert_nm.unwrap();
    // 0.03 NM + 4 NM/h: NSE passes 1 NM after 0.2425 h ≈ 102 NM at 420 kt, from ~40 NM.
    assert!(first > 130.0 && first < 150.0, "{first}");
    // A declared 50 m backup supports NACp 8 everywhere.
    let b = sc("e-conspicuity backup");
    assert_eq!(b.meets_nacp_fraction, Some(1.0));
    assert!(b.sensor_share[0].0.starts_with("backup:"));
}

#[test]
fn rnp_cli_and_unknown_facility() {
    let d = tmp("rnp");
    let req = rnp_request();
    std::fs::write(d.join("rnp.json"), serde_json::to_string(&req).unwrap()).unwrap();
    let (ok, text) = run(
        &d,
        &[
            "navaids",
            "rnp",
            d.join("rnp.json").to_str().unwrap(),
            "--json",
            d.join("out.json").to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    assert!(text.contains("GNSS lost"));
    let mut bad = req;
    bad.scenarios[1].out = vec!["NOPE".into()];
    assert!(rnp::run(&bad).unwrap_err().contains("unknown facility"));
}

/// Track at 10 s over 30 epochs from (49.6, 9.5) heading north-east; inspection truth
/// shifted 400 m north in epochs 10..14; D1 reads 0.3 NM long in epochs 4..6.
fn inspection_fixture(d: &Path) -> PathBuf {
    let net = network();
    let mut track = String::from("t_s,lat_deg,lon_deg,alt_ft,truth_sigma_m\n");
    let mut meas = String::from("t_s,station,kind,value\n");
    for i in 0..30 {
        let t = i as f64 * 10.0;
        let (lat, lon) = (49.6 + 0.01 * i as f64, 9.5 + 0.015 * i as f64);
        let alt = 8_000.0;
        for s in &net {
            let (dd, _) = dist_bearing(lat, lon, s.lat_deg, s.lon_deg);
            let dh = (alt - s.elev_ft) * FT;
            let mut slant = (dd * dd + dh * dh).sqrt() / NM;
            if s.id == "D1" && (4..=6).contains(&i) {
                slant += 0.3;
            }
            meas += &format!("{t},{},dme,{slant:.6}\n", s.id);
            if s.kind == Kind::VorDme {
                let (_, radial) = dist_bearing(s.lat_deg, s.lon_deg, lat, lon);
                meas += &format!(
                    "{t},{},vor,{:.5}\n",
                    s.id,
                    radial.to_degrees().rem_euclid(360.0)
                );
            }
        }
        let shift = if (10..15).contains(&i) {
            400.0 / 111_195.0
        } else {
            0.0
        };
        track += &format!("{t},{},{lon},{alt},5\n", lat + shift);
    }
    std::fs::write(d.join("track.csv"), track).unwrap();
    std::fs::write(d.join("meas.csv"), meas).unwrap();
    let mut req = String::from(
        "track = \"track.csv\"\nmeasurements = \"meas.csv\"\ndme_sigma_m = 20.0\nvor_sigma_deg = 0.05\n",
    );
    for s in &net {
        req += &format!("[[station]]\n{}", toml::to_string(s).unwrap());
    }
    let p = d.join("inspect.toml");
    std::fs::write(&p, req).unwrap();
    p
}

#[test]
fn flight_inspection_truth_assurance() {
    let d = tmp("insp");
    let p = inspection_fixture(&d);
    let out = d.join("insp.json");
    let (ok, text) = run(
        &d,
        &[
            "navaids",
            "inspect",
            p.to_str().unwrap(),
            "--json",
            out.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok, "{text}");
    let r: inspection::InspectionReport =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(r.epochs, 30);
    assert_eq!(
        r.truth_suspect,
        5,
        "{:?}",
        r.verdicts.iter().map(|v| &v.verdict).collect::<Vec<_>>()
    );
    assert_eq!(r.truth_suspect_episodes, vec![[100.0, 140.0]]);
    let off = r.max_truth_offset_m.unwrap();
    assert!((off - 400.0).abs() < 40.0, "{off}");
    assert_eq!(r.facility_deviation.get("D1"), Some(&3));
    assert_eq!(r.consistent, 22);
    assert_eq!(r.unresolved, 0);
}
