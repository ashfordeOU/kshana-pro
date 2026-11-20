// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Ship bridge features (`kshana_pro::bridge`): the cross-sensor consistency aid on synthetic
//! bridge logs from the open engine's training generator and on hand-made logs, the
//! dead-reckoning budget, the ECDIS EP tester, the drill packs and the CLI with its licence
//! gates. Evidence: MODELLED (synthetic logs with known truth).

use std::path::PathBuf;
use std::process::Command;

use kshana_pro::bridge::consistency::{self, Config, DepthZone, RadarMark};
use kshana_pro::bridge::{dr, drills, ep};
use kshana_pro::license::{self, Tier};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-bridge-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SEED: [u8; 32] = [9u8; 32];

fn key_file(dir: &std::path::Path) -> (String, String) {
    let p = dir.join("signing.key");
    std::fs::write(&p, hex::encode(SEED)).unwrap();
    (
        p.to_string_lossy().into_owned(),
        hex::encode(license::public_key_for(&SEED)),
    )
}

fn cli(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// A coastal passage with a smooth drag-off of the reported position, every second logged.
fn scenario(events: &str) -> String {
    format!(
        r#"
[scenario]
name = "test-passage"
seed = 7
start_utc = "2026-06-01T10:00:00Z"
duration_s = 1200
rate_hz = 1
[vessel]
lat_deg = 54.00
lon_deg = 12.00
speed_kn = 12
[[waypoint]]
lat_deg = 54.25
lon_deg = 12.60
speed_kn = 12
[environment]
current_speed_kn = 1.0
current_set_deg = 30
[output]
log_interval_s = 1
{events}
"#
    )
}

const DRAG: &str = r#"
[[event]]
kind = "drag-off"
start_s = 400
duration_s = 600
ramp_s = 360
recovery_s = 0
final_offset_m = 1200
bearing_deg = 300
"#;

fn generate(events: &str) -> kshana::nmea_synth::Generated {
    kshana::nmea_synth::generate_from_toml(&scenario(events), None).unwrap()
}

#[test]
fn drag_off_is_flagged_before_a_position_alarm_and_gnss_is_named() {
    let g = generate(DRAG);
    let r = consistency::run(g.nmea_text().as_bytes(), &Config::default()).unwrap();
    assert_eq!(r.alerts.len(), 1, "{}", consistency::summary_text(&r));
    let a = &r.alerts[0];
    assert!(
        a.start_s >= 400.0,
        "no alert before the drag starts: {}",
        a.start_s
    );
    // A position alarm at 0.1 NM: the first second the true position error reaches it.
    let alarm = g
        .log
        .track
        .iter()
        .find(|t| t.position_error_m.is_some_and(|e| e >= 185.2))
        .map(|t| t.t_s)
        .unwrap();
    assert!(
        a.start_s < alarm,
        "detected at {} s, a 0.1 NM position alarm at {alarm} s",
        a.start_s
    );
    assert_eq!(a.first_check, "velocity");
    assert!(a.named.starts_with("gnss:"), "{a:?}");
    // Gyro, log and GNSS are all in the velocity check and nothing else clears them: the
    // data alone cannot separate them, and the report says so.
    assert!(!a.separable);
    assert_eq!(a.candidates.len(), 3);
    assert!(a.checks.iter().any(|(c, _)| c == "position"));
    assert!(r.outages.is_empty());
    assert!(r.score_min.unwrap() < 50.0);
}

#[test]
fn a_clean_passage_raises_nothing() {
    let g = generate("");
    let r = consistency::run(g.nmea_text().as_bytes(), &Config::default()).unwrap();
    assert!(r.alerts.is_empty(), "{}", consistency::summary_text(&r));
    assert!(r.score_min.unwrap() > 50.0);
    let avail = |c: &str| r.availability.iter().find(|a| a.check == c).unwrap().epochs;
    assert!(avail("velocity") > 1000 && avail("position") > 1000 && avail("time") > 1000);
    assert_eq!(avail("radar"), 0, "no marks configured");
}

#[test]
fn jamming_is_an_outage_not_a_disagreement() {
    let g = generate(
        r#"
[[event]]
kind = "jamming"
start_s = 300
duration_s = 400
ramp_s = 60
recovery_s = 30
cn0_drop_db = 40
"#,
    );
    let r = consistency::run(g.nmea_text().as_bytes(), &Config::default()).unwrap();
    assert!(!r.outages.is_empty());
    assert!(r.alerts.is_empty(), "{}", consistency::summary_text(&r));
}

#[test]
fn a_time_spoof_names_the_gnss_time() {
    let g = generate(
        r#"
[[event]]
kind = "time-spoof"
start_s = 300
duration_s = 600
ramp_s = 120
recovery_s = 0
offset_s = 40
"#,
    );
    let r = consistency::run(g.nmea_text().as_bytes(), &Config::default()).unwrap();
    assert!(!r.alerts.is_empty());
    let a = &r.alerts[0];
    assert_eq!(a.first_check, "time");
    assert!(a.start_s >= 300.0);
    assert_eq!(a.named, "gnss:GN/time");
    assert!(a.separable);
}

#[test]
fn an_iec_61162_450_capture_reads_as_the_plain_log() {
    let g = generate(DRAG);
    let plain = consistency::run(g.nmea_text().as_bytes(), &Config::default()).unwrap();
    let cap = kshana::iec61162_450::wrap(&g.nmea_text(), "NI0001");
    let r = consistency::run(&cap, &Config::default()).unwrap();
    assert!(r.input.datagrams > 1000);
    assert_eq!(r.alerts.len(), plain.alerts.len());
    assert_eq!(r.alerts[0].start_s, plain.alerts[0].start_s);
}

fn sentence(body: &str) -> String {
    format!(
        "${body}*{:02X}\r\n",
        kshana::interop::testbench::nmea_checksum(body)
    )
}

fn dm(deg: f64, w: usize, pos: char, neg: char) -> String {
    let h = if deg < 0.0 { neg } else { pos };
    let a = deg.abs();
    let d = a.floor();
    format!("{:0w$}{:07.4},{h}", d as u32, (a - d) * 60.0, w = w)
}

/// A hand-made log: a ship on 045 at 12 kn with a gyro (HE), a GNSS receiver that is also a
/// GNSS compass (GP), a Doppler log (VD), an echo-sounder (SD) and a radar tracking one mark
/// (RA). `gnss_off(t)` adds a position offset (north, east metres) and `compass_off(t)` a
/// heading error to the GNSS outputs; `depth(t)` is the sounder.
fn bridge_log(
    n: usize,
    gnss_off: impl Fn(f64) -> (f64, f64),
    compass_off: impl Fn(f64) -> f64,
    depth: impl Fn(f64) -> f64,
    mark: (f64, f64),
) -> String {
    let (lat0, lon0, hdg, kn) = (57.0f64, 11.0f64, 45.0f64, 12.0f64);
    let v = kn * 1852.0 / 3600.0;
    let mut s = String::new();
    let pos = |t: f64| {
        let (dn, de) = (
            v * t * hdg.to_radians().cos(),
            v * t * hdg.to_radians().sin(),
        );
        kshana_pro::bridge::move_ne(lat0, lon0, dn, de)
    };
    for k in 0..n {
        let t = k as f64;
        let (on, oe) = gnss_off(t);
        let (dn1, de1) = gnss_off(t + 1.0);
        let (la, lo) = pos(t);
        let (gla, glo) = kshana_pro::bridge::move_ne(la, lo, on, oe);
        // GNSS velocity: true plus the offset rate (a step has no Doppler signature).
        let (rn, re) = if (dn1 - on).hypot(de1 - oe) < 50.0 {
            (dn1 - on, de1 - oe)
        } else {
            (0.0, 0.0)
        };
        let (vn, ve) = (
            v * hdg.to_radians().cos() + rn,
            v * hdg.to_radians().sin() + re,
        );
        let sog = vn.hypot(ve) / (1852.0 / 3600.0);
        let cog = ve.atan2(vn).to_degrees().rem_euclid(360.0);
        let hh = 12 + (k / 3600);
        let mm = (k / 60) % 60;
        let ss = k % 60;
        let time = format!("{hh:02}{mm:02}{ss:02}.00");
        s.push_str(&sentence(&format!(
            "GPRMC,{time},A,{},{},{sog:.2},{cog:.2},010626,,,A",
            dm(gla, 2, 'N', 'S'),
            dm(glo, 3, 'E', 'W')
        )));
        s.push_str(&sentence(&format!("HEHDT,{hdg:.2},T")));
        s.push_str(&sentence(&format!(
            "GPHDT,{:.2},T",
            (hdg + compass_off(t)).rem_euclid(360.0)
        )));
        s.push_str(&sentence(&format!(
            "VDVBW,{kn:.2},0.00,A,{kn:.2},0.00,A,,,,"
        )));
        s.push_str(&sentence(&format!("SDDPT,{:.1},0.0", depth(t))));
        let rng = kshana_pro::bridge::distance_m(la, lo, mark.0, mark.1) / 1852.0;
        let brg = kshana_pro::bridge::bearing_deg(la, lo, mark.0, mark.1);
        s.push_str(&sentence(&format!(
            "RATTM,07,{rng:.3},{brg:.1},T,0.0,0.0,T,0.0,0.0,N,BUOY,T,,{time},A"
        )));
    }
    s
}

#[test]
fn heading_anomaly_is_flagged_before_the_position_and_the_compass_is_named() {
    // The GNSS compass heading departs at 60 s; the position starts to drift at 150 s.
    let log = bridge_log(
        400,
        |t| {
            if t > 150.0 {
                (0.0, 3.0 * (t - 150.0))
            } else {
                (0.0, 0.0)
            }
        },
        |t| if t > 60.0 { 8.0 } else { 0.0 },
        |_| 40.0,
        (57.05, 11.10),
    );
    let r = consistency::run(log.as_bytes(), &Config::default()).unwrap();
    let a = &r.alerts[0];
    assert_eq!(a.first_check, "heading");
    assert!(a.start_s >= 60.0 && a.start_s < 150.0, "{}", a.start_s);
    assert_eq!(a.named, "gnss-compass:GP/heading");
    assert!(a.separable, "{a:?}");
    assert!(a.cleared.contains(&"gyro:HE/heading".to_string()));
    let pos_first = r.first_failure_s.get("velocity").copied().unwrap();
    assert!(r.first_failure_s["heading"] < pos_first);
}

#[test]
fn radar_marks_and_depth_zones_catch_a_position_jump() {
    let mark = (57.05, 11.10);
    let zone = DepthZone {
        name: "channel".into(),
        polygon: vec![[56.9, 10.9], [56.9, 11.2], [57.1, 11.2], [57.1, 10.9]],
        min_m: 30.0,
        max_m: 60.0,
    };
    let cfg = Config {
        radar_marks: vec![RadarMark {
            name: "buoy".into(),
            target: 7,
            lat_deg: mark.0,
            lon_deg: mark.1,
        }],
        depth_zones: vec![zone],
        ..Default::default()
    };
    // Clean: radar and depth agree with GNSS.
    let clean = bridge_log(300, |_| (0.0, 0.0), |_| 0.0, |_| 40.0, mark);
    let r = consistency::run(clean.as_bytes(), &cfg).unwrap();
    assert!(r.alerts.is_empty(), "{}", consistency::summary_text(&r));
    let av = |c: &str| r.availability.iter().find(|a| a.check == c).unwrap().epochs;
    assert!(av("radar") > 250 && av("depth") > 250);
    // A 600 m step of the GNSS position at 100 s (no velocity signature) and a sounder that
    // shows 12 m in a 30-60 m channel from 200 s.
    let jump = bridge_log(
        300,
        |t| if t >= 100.0 { (600.0, 0.0) } else { (0.0, 0.0) },
        |_| 0.0,
        |t| if t >= 200.0 { 12.0 } else { 40.0 },
        mark,
    );
    let r = consistency::run(jump.as_bytes(), &cfg).unwrap();
    assert!(
        r.first_failure_s.contains_key("radar"),
        "{:?}",
        r.first_failure_s
    );
    assert!(r.first_failure_s.contains_key("position"));
    assert!(r.first_failure_s.contains_key("depth"));
    let a = &r.alerts[0];
    assert!(a.start_s >= 100.0 && a.start_s < 110.0, "{}", a.start_s);
    assert_eq!(a.named, "gnss:GP/position");
    assert!(a.separable, "{a:?}");
}

#[test]
fn dead_reckoning_closed_form_matches_simulation() {
    for m in [
        dr::DrModel::default(),
        dr::DrModel {
            speed_kn: 6.0,
            depth_m: Some(30.0),
            ..Default::default()
        },
    ] {
        for t in [900.0, 7200.0] {
            let mc = m.monte_carlo(t, 3000, 11);
            assert!((mc.r95_m / m.r95(t) - 1.0).abs() < 0.08, "{t}: {mc:?}");
        }
    }
    // Deeper than the log's bottom-track limit, the unknown current enters.
    let shallow = dr::DrModel {
        depth_m: Some(150.0),
        ..Default::default()
    };
    let deep = dr::DrModel {
        depth_m: Some(250.0),
        ..Default::default()
    };
    assert!(deep.time_to_limit(1852.0).unwrap() < shallow.time_to_limit(1852.0).unwrap());
}

#[test]
fn ep_vectors_have_checkpoints_and_the_reference_table_passes() {
    for id in ep::ids() {
        let b = ep::build(id).unwrap();
        let r = ep::check(&b.vector, &ep::parse_observed(&b.reference_csv).unwrap());
        assert!(r.passed, "{id}: {:?}", r.criteria);
    }
    let m = ep::build("ep-manual-position").unwrap();
    assert!(m.vector.manual.is_some());
    assert!(m.vector.checkpoints.iter().any(|c| c.t_s > 600.0));
}

#[test]
fn drills_build_with_instructor_log_rubric_and_aid_reference() {
    let b = drills::build("drill-coastal-drag-off").unwrap();
    assert!(b.instructor_json.as_deref().unwrap().contains("drag-off"));
    let aid = b.aid.unwrap();
    assert_eq!(aid.alerts.len(), 1);
    assert!(aid.alerts[0].start_s >= b.drill.onset_s);
    let b = drills::build("drill-natural-degradation").unwrap();
    assert!(
        b.aid.unwrap().alerts.is_empty(),
        "the negative control raises no alert"
    );
    for (d, _) in drills::all() {
        let b = drills::build(&d.id).unwrap();
        assert_eq!(b.stimulus.is_some(), d.has_stimulus);
    }
}

#[test]
fn features_are_registered_with_their_tiers() {
    for (f, t) in [
        ("pro-bridge-consistency", Tier::Pro),
        ("pro-ecdis-ep-test", Tier::Pro),
        ("pro-bridge-drills", Tier::Pro),
    ] {
        assert_eq!(license::explicit_tier(f), Some(t));
        assert!(license::pro_command_features().contains(&f));
    }
}

#[test]
fn cli_check_signs_verifies_and_is_gated() {
    let d = tmp("cli");
    let (kf, pk) = key_file(&d);
    let log = d.join("drag.nmea");
    std::fs::write(&log, generate(DRAG).nmea_text()).unwrap();
    let log = log.to_string_lossy().into_owned();
    // Evaluation tier (no licence): refused.
    let (ok, out) = cli(&["bridge", "check", &log]);
    assert!(!ok && out.contains("requires"), "{out}");
    let (ok, out) = cli(&["bridge", "check", &log, "--dev"]);
    assert!(ok && out.contains("ALERT"), "{out}");
    let res = d.join("res");
    let res_s = res.to_string_lossy().into_owned();
    let (ok, out) = cli(&[
        "bridge",
        "check",
        &log,
        "--out",
        &res_s,
        "--signer",
        "Test",
        "--key-file",
        &kf,
        "--utc",
        "2026-11-20T10:00:00Z",
        "--dev",
    ]);
    assert!(ok, "{out}");
    for f in [
        "consistency.json",
        "epochs.csv",
        "summary.txt",
        "signature.json",
    ] {
        assert!(res.join(f).exists(), "{f}");
    }
    let (ok, out) = cli(&["bridge", "verify", &res_s, "--trust", &pk]);
    assert!(ok, "{out}");
    std::fs::write(res.join("summary.txt"), "edited").unwrap();
    let (ok, _) = cli(&["bridge", "verify", &res_s, "--trust", &pk]);
    assert!(!ok);
}

#[test]
fn cli_dr_ep_and_drills() {
    let d = tmp("cli2");
    let (kf, _) = key_file(&d);
    let (ok, out) = cli(&[
        "bridge",
        "dr",
        "--outage-s",
        "3600",
        "--limit-m",
        "1852",
        "--json",
    ]);
    assert!(ok, "{out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["table"].as_array().unwrap().len(), 7);
    assert!(v["time_to_limit_s"].as_f64().unwrap() > 0.0);
    let vecs = d.join("vectors");
    let vs = vecs.to_string_lossy().into_owned();
    let (ok, out) = cli(&["bridge", "ep-vectors", "--out", &vs]);
    assert!(ok, "{out}");
    let vd = vecs.join("ep-loss-straight");
    let obs = vd.join("reference-observed.csv");
    let (ok, out) = cli(&[
        "bridge",
        "ep-check",
        &vd.to_string_lossy(),
        "--observed",
        &obs.to_string_lossy(),
        "--dev",
    ]);
    assert!(ok && out.contains("PASS"), "{out}");
    let (ok, out) = cli(&["bridge", "drills", "list"]);
    assert!(ok && out.contains("readiness-checklist"), "{out}");
    let db = d.join("drill");
    let (ok, out) = cli(&[
        "bridge",
        "drills",
        "build",
        "drill-port-time-spoof",
        "--out",
        &db.to_string_lossy(),
        "--dev",
    ]);
    assert!(ok, "{out}");
    for f in [
        "drill.json",
        "stimulus.nmea",
        "instructor-log.json",
        "rubric.csv",
        "aid-reference.txt",
    ] {
        assert!(db.join(f).exists(), "{f}");
    }
    let actions = d.join("actions.csv");
    std::fs::write(&actions, "t_s,action,by\n420,detect-time,OOW\n700,list-affected,OOW\n800,report-master,OOW\n900,log-entry,OOW\n").unwrap();
    let sd = d.join("score");
    let (ok, out) = cli(&[
        "bridge",
        "drills",
        "score",
        "drill-port-time-spoof",
        "--actions",
        &actions.to_string_lossy(),
        "--out",
        &sd.to_string_lossy(),
        "--signer",
        "Instructor",
        "--key-file",
        &kf,
        "--trainee",
        "Cadet A",
        "--utc",
        "2026-11-20T10:00:00Z",
        "--dev",
    ]);
    assert!(ok, "{out}");
    let s: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sd.join("score.json")).unwrap()).unwrap();
    assert_eq!(s["session"]["trainee"], "Cadet A");
    // Every item in time except the report to the authority (10 points): 70 of 80.
    assert_eq!(s["earned"].as_f64().unwrap(), 70.0);
    assert!(sd.join("signature.json").exists());
}
