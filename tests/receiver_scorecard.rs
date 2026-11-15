// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Receiver resilience scorecards (roadmap 0.42, E3). The development inputs here are the
//! open engine's training scenarios (tests/fixtures/forensics/training), not the published
//! sample campaign's cases.

use std::path::{Path, PathBuf};

use kshana_pro::scorecards::{self, Campaign};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e3-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Generate a training scenario's NMEA and its truth as an I4 motion CSV into `dir`.
fn training_case(name: &str, dir: &Path) -> String {
    use kshana::interop::testbench::{MotionSample, Trajectory};
    let text = std::fs::read_to_string(root().join(format!("tests/fixtures/forensics/training/{name}.toml"))).unwrap();
    let mut scn = kshana::nmea_synth::TrainingScenario::parse(&text).unwrap();
    scn.output.log_interval_s = 1.0;
    let g = kshana::nmea_synth::generate(&scn).unwrap();
    std::fs::write(dir.join(format!("{name}.nmea")), g.nmea_text()).unwrap();
    let samples = g
        .log
        .track
        .iter()
        .map(|r| {
            let gd = kshana::frames::Geodetic { lat_rad: r.true_lat_deg.to_radians(), lon_rad: r.true_lon_deg.to_radians(), alt_m: 0.0 };
            MotionSample { t_s: r.t_s, lat_deg: r.true_lat_deg, lon_deg: r.true_lon_deg, h_m: 0.0, ecef_m: kshana::frames::geodetic_to_ecef(gd), v_ecef_m_s: [0.0; 3], heading_deg: 0.0, pitch_deg: 0.0, roll_deg: 0.0 }
        })
        .collect();
    let t = Trajectory {
        kind: "test".into(),
        scenario_hash: "000000000000".into(),
        epoch: kshana::interop::UtcEpoch::parse_iso(&scn.scenario.start_utc).unwrap(),
        note: "test".into(),
        samples,
        events: vec![],
    };
    std::fs::write(dir.join(format!("{name}.motion.csv")), kshana::interop::testbench::write_motion_csv(&t)).unwrap();
    let mut ev = String::new();
    for e in &scn.events {
        ev.push_str(&format!(
            "[[case.events]]\nlabel = \"{}\"\nkind = \"{}\"\nonset_s = {}\nend_s = {}\n",
            e.kind.name(),
            kshana_pro::threat_library::ThreatScenario::event_kind(e.kind),
            e.start_s,
            e.start_s + e.duration_s + e.recovery_s
        ));
    }
    ev
}

fn dev_campaign(dir: &Path) -> String {
    let drag = training_case("coastal-drag-off", dir);
    let jam = training_case("open-sea-jamming", dir);
    format!(
        "kind = \"pro-receiver-scorecard\"\ntitle = \"Development campaign\"\n\
         [receiver]\nmaker = \"Open engine\"\nmodel = \"NMEA receiver model\"\n\
         [campaign]\nid = \"DEV-1\"\nfacility = \"desk (synthetic)\"\n\
         [bars]\nfixed_utc = \"2026-11-15T12:00:00Z\"\n\
         [[case]]\nid = \"drag\"\n[case.log]\nformat = \"nmea\"\npath = \"coastal-drag-off.nmea\"\nplatform = \"vessel\"\n[case.truth]\nmotion_csv = \"coastal-drag-off.motion.csv\"\n{drag}\
         [[case]]\nid = \"jam\"\n[case.log]\nformat = \"nmea\"\npath = \"open-sea-jamming.nmea\"\nplatform = \"vessel\"\n[case.truth]\nmotion_csv = \"open-sea-jamming.motion.csv\"\n{jam}\
         [[case]]\nid = \"jam-no-truth\"\n[case.log]\nformat = \"nmea\"\npath = \"open-sea-jamming.nmea\"\nplatform = \"vessel\"\n{jam}"
    )
}

#[test]
fn explore_dev() {
    let d = tmp("dev");
    let src = dev_campaign(&d);
    std::fs::write(d.join("campaign.toml"), &src).unwrap();
    let (c, src) = Campaign::from_file(&d.join("campaign.toml")).unwrap();
    let s = scorecards::score(&c, &src).unwrap();
    println!("{}", serde_json::to_string_pretty(&s.cases).unwrap());
    println!("{:?}", s.levels);
}
