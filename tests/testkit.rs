// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The test-as-code kit (`kshana_pro::testkit`): neutral scenarios, the compiler and
//! validator over the simulator adapters, scoring, the CI runner with signed verdicts, the
//! regression diff, incident-to-regression, replay fidelity, the packs and the CLI.
//! Evidence: MODELLED (synthetic receiver logs).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use kshana_pro::license::{self, Entitlement, Tier};
use kshana_pro::pics::log::DeviceLog;
use kshana_pro::scorecards::levels::Verdict;
use kshana_pro::testkit::compile::{self, Support};
use kshana_pro::testkit::fidelity::{self, Capture};
use kshana_pro::testkit::spec::{Metric, Spec, Technique, ThreatType};
use kshana_pro::testkit::{diff, incident, library, packs, run, score};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-testkit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

const SEED: [u8; 32] = [7u8; 32];
const UTC: &str = "2026-11-20T09:00:00Z";

fn pack_spec(id: &str) -> Spec {
    Spec::parse(packs::find(id).unwrap().1)
        .unwrap()
        .resolve()
        .unwrap()
}

fn all_pack_specs() -> Vec<Spec> {
    packs::PACKS
        .iter()
        .flat_map(|p| p.2.iter())
        .map(|(_, src)| Spec::parse(src).unwrap().resolve().unwrap())
        .collect()
}

const MINIMAL: &str = r#"
format = "kshana-testkit-scenario/1"
id = "mini"
title = "Minimal"
[scene]
start_utc = "2026-01-01T00:00:00Z"
duration_s = 100
[trajectory]
lat_deg = 10.0
lon_deg = 20.0
[[threat]]
id = "j"
type = "jamming"
start_s = 20
duration_s = 30
center_hz = 1575420000.0
js_db = 30
[[criterion]]
id = "detect"
metric = "detection_latency_s"
op = "<="
value = 5
"#;

#[test]
fn every_pack_scenario_is_valid_modelled_and_compiles_for_every_adapter() {
    let specs = all_pack_specs();
    assert_eq!(specs.len(), 17);
    for p in ["dp-vessel", "aviation", "uas", "timing", "rpcf"] {
        assert!(packs::pack(p).is_ok(), "{p}");
    }
    for s in &specs {
        assert_eq!(s.evidence, "MODELLED", "{}", s.id);
        assert!(!s.modelled_on.is_empty(), "{}", s.id);
        assert!(!s.criteria.is_empty() && !s.threats.is_empty(), "{}", s.id);
        for a in compile::ADAPTERS {
            let c = compile::compile(s, a).unwrap_or_else(|e| panic!("{} {a}: {e}", s.id));
            assert!(c.files.contains_key("scenario.toml"));
            if a == "gps-sdr-sim" {
                assert!(c.files["run.sh"].contains("gps-sdr-sim"));
                assert!(!c.files.contains_key("spoofer.csv"));
            } else {
                assert!(
                    c.files.contains_key("commands.txt") && c.files.contains_key("trajectory.nmea")
                );
            }
            // The scenario written by the compiler parses back to the same scenario.
            let back = Spec::parse(&c.files["scenario.toml"]).unwrap();
            assert_eq!(back.sha256(), s.sha256(), "{} {a}", s.id);
        }
    }
}

#[test]
fn packs_cover_the_requested_themes_and_never_claim_conformance() {
    let all = all_pack_specs();
    let text = |s: &Spec| format!("{} {} {:?}", s.title, s.description, s.modelled_on);
    assert!(all
        .iter()
        .filter(|s| s.id.starts_with("dp-"))
        .all(|s| text(s).contains("IMCA IN 1730")));
    for theme in [
        "position reference disagreement",
        "slow drift",
        "stable but wrong",
    ] {
        assert!(all.iter().any(|s| text(s).contains(theme)), "{theme}");
    }
    for l in 1..=4 {
        let s = pack_spec(&format!("rpcf-level-{l}"));
        assert!(text(&s).contains("modelled on"));
        assert_eq!(
            s.criteria.iter().filter_map(|c| c.level).max(),
            Some(l as u8)
        );
    }
    assert!(all.iter().any(|s| s.id == "aviation-post-spoof-recovery"));
    assert!(all.iter().any(|s| s.id == "aviation-post-spoof-reset"));
    assert!(all.iter().filter(|s| s.id.starts_with("uas-")).count() >= 3);
    assert!(all.iter().filter(|s| s.id.starts_with("timing-")).count() >= 3);
}

#[test]
fn spec_rules_refuse_bad_scenarios() {
    let ok = Spec::parse(MINIMAL).unwrap();
    assert_eq!(ok.version, 1);
    assert_eq!(ok.signal_sets()[0].signals, vec!["L1CA".to_string()]);
    let bad = |from: &str, to: &str, want: &str| {
        let e = Spec::parse(&MINIMAL.replace(from, to)).unwrap_err();
        assert!(e.contains(want), "{e}");
    };
    bad(
        "title = \"Minimal\"",
        "title = \"A certified test\"",
        "certified",
    );
    bad("js_db = 30", "", "js_db");
    bad("type = \"jamming\"", "type = \"spoofing\"", "technique");
    bad(
        "value = 5",
        "value = 5\nthreat = \"nope\"",
        "unknown threat",
    );
    bad("start_s = 20", "start_s = 200", "after the scene");
    bad(
        "kshana-testkit-scenario/1",
        "kshana-testkit-scenario/9",
        "format",
    );
    // JSON and TOML carry the same scenario.
    let json = serde_json::to_string(&ok).unwrap();
    assert_eq!(Spec::parse(&json).unwrap(), ok);
    assert_eq!(Spec::parse(&ok.to_toml()).unwrap().sha256(), ok.sha256());
}

#[test]
fn trajectories_and_counterfeit_models() {
    let s = pack_spec("uas-jamming-flight");
    let a = s.trajectory.at(0.0);
    let b = s.trajectory.at(100.0);
    let d = kshana::interference_map::grid::haversine_m(a.lat_deg, a.lon_deg, b.lat_deg, b.lon_deg);
    assert!((d - 1500.0).abs() < 5.0, "{d}");
    assert!((b.speed_mps - 15.0).abs() < 1e-9);
    // a drag at 0.2 m/s: 20 m after 100 s, nothing after the end
    let s = pack_spec("dp-position-reference-disagreement");
    let (e, n, _) = s.counterfeit_at(400.0);
    assert!((e.hypot(n) - 20.0).abs() < 1e-6 && (e - 20.0).abs() < 1e-6);
    assert_eq!(s.counterfeit_at(700.0), (0.0, 0.0, 0.0));
    // a time push capped at its offset
    let s = pack_spec("timing-matched-power-time-push");
    let t = &s.threats[0];
    assert_eq!(t.technique, Some(Technique::TimePush));
    assert!((s.counterfeit_at(t.start_s + 50.0).2 - 1000.0).abs() < 1e-6);
    assert!((s.counterfeit_at(t.start_s + 500.0).2 - 2000.0).abs() < 1e-6);
}

#[test]
fn threat_library_scenarios_become_neutral_scenarios() {
    for (id, _) in kshana_pro::threat_library::LIBRARY {
        let s = library::spec(id).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(!s.threats.is_empty(), "{id}");
        for a in compile::ADAPTERS {
            compile::validate(&s, a).unwrap();
        }
    }
    let h = library::spec("hormuz-position-displacement").unwrap();
    let t = &h.threats[0];
    assert_eq!(t.final_offset_m, Some(8000.0));
    assert_eq!(t.technique, Some(Technique::PositionDrag));
    let src = kshana_pro::threat_library::get("baltic-gulf-of-finland-jamming").unwrap();
    let drop = src.analogue.unwrap().events[0].cn0_drop_db.unwrap();
    let j = library::spec("baltic-gulf-of-finland-jamming").unwrap();
    assert_eq!(j.threats[0].js_db, Some(drop + library::JS_MARGIN_DB));
}

#[test]
fn validator_reports_what_each_adapter_cannot_express() {
    let s = pack_spec("dp-slow-drift"); // GPS + GLONASS, spoofing, sensors
    let g = compile::validate(&s, "gps-sdr-sim").unwrap();
    assert!(!g.expressible);
    assert!(g
        .findings
        .iter()
        .any(|f| f.support == Support::Unsupported && f.detail.contains("glonass")));
    assert!(g.findings.iter().any(|f| f.detail.contains("attack IQ")));
    let sk = compile::validate(&s, "skydel").unwrap();
    assert!(sk.expressible && !sk.profile_confirmed);
    assert!(sk
        .findings
        .iter()
        .any(|f| f.support == Support::Approximated));
    assert!(sk
        .findings
        .iter()
        .any(|f| f.feature == "sensor gyro" && f.support == Support::Manual));
    let ls = compile::validate(&s, "labsat").unwrap();
    assert!(
        !ls.expressible,
        "LabSat replays a file and generates no spoofer"
    );
    // LabSat has no jammer command either
    let j = compile::validate(&pack_spec("dp-gnss-jamming"), "labsat").unwrap();
    assert!(j.findings.iter().any(|f| f.support == Support::Unsupported));
    assert!(compile::validate(&s, "nope").is_err());
}

#[test]
fn compiled_files_carry_the_schedule_trajectory_and_spoofer() {
    let s = pack_spec("dp-gnss-jamming");
    let c = compile::compile(&s, "skydel").unwrap();
    assert!(
        c.files["commands.txt"].contains("AddInterference"),
        "{}",
        c.files["commands.txt"]
    );
    assert!(c.files["commands.txt"].contains("never radiate"));
    let sched: serde_json::Value = serde_json::from_str(&c.files["schedule.json"]).unwrap();
    assert_eq!(sched["entries"].as_array().unwrap().len(), 2);
    let sp = compile::compile(&pack_spec("dp-stable-but-wrong"), "spirent").unwrap();
    let csv = &sp.files["spoofer.csv"];
    assert!(
        csv.lines()
            .nth(1)
            .unwrap()
            .starts_with("300.0,60.000,53.000"),
        "{csv}"
    );
    let m = compile::compile(&pack_spec("uas-jamming-flight"), "gps-sdr-sim").unwrap();
    assert!(m.files["run.sh"].contains("-x motion.csv"));
    assert!(m.files["motion.csv"].lines().count() > 2000);
    let d = tmp("compile");
    compile::write(&c, &d).unwrap();
    let rep: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(d.join("COMPILE.json")).unwrap()).unwrap();
    assert_eq!(rep["format"], "kshana-testkit-compiled/1");
    assert!(rep["label"].as_str().unwrap().starts_with("MODELLED"));
}

#[test]
fn good_receivers_pass_and_bad_ones_fail_every_pack_scenario() {
    for s in all_pack_specs() {
        let good = score::score(&s, &run::synth_log(&s, true), "b", "r").unwrap();
        let bad = score::score(&s, &run::synth_log(&s, false), "b", "r").unwrap();
        let expect_good = if s.id == "rpcf-level-3" || s.id == "rpcf-level-4" {
            "fail"
        } else {
            "pass"
        };
        assert_eq!(good.verdict, expect_good, "{}: {:#?}", s.id, good.criteria);
        assert_eq!(bad.verdict, "fail", "{}: {:#?}", s.id, bad.criteria);
        assert!(good.label.starts_with("MODELLED"));
    }
    // levels: the flagging receiver supports level 2, the silent one none
    let s = pack_spec("rpcf-level-4");
    assert_eq!(
        score::score(&s, &run::synth_log(&s, true), "b", "r")
            .unwrap()
            .level_supported,
        Some(2)
    );
    assert_eq!(
        score::score(&s, &run::synth_log(&s, false), "b", "r")
            .unwrap()
            .level_supported,
        Some(0)
    );
}

#[test]
fn missing_data_is_not_assessed_never_met() {
    let s = pack_spec("timing-meaconing-delay");
    let mut l = run::synth_log(&s, true);
    for e in &mut l.epochs {
        e.time_error_ns = None;
        e.lat_deg = None;
        e.lon_deg = None;
    }
    let v = score::score(&s, &l, "b", "r").unwrap();
    let tb = v.criteria.iter().find(|c| c.id == "time-bounded").unwrap();
    assert_eq!(tb.verdict, Verdict::NotAssessed);
    assert_eq!(v.verdict, "incomplete");
    // a log that ends before recovery could be seen
    let s = pack_spec("dp-gnss-jamming");
    let mut l = run::synth_log(&s, true);
    l.epochs.retain(|e| e.t_s < 610.0);
    let v = score::score(&s, &l, "b", "r").unwrap();
    assert_eq!(
        v.criteria
            .iter()
            .find(|c| c.id == "recover")
            .unwrap()
            .verdict,
        Verdict::NotAssessed
    );
    assert!(Metric::MaxTimeErrorNs.is_timing());
}

fn write_logs(dir: &Path, specs: &[Spec], good: bool) {
    std::fs::create_dir_all(dir).unwrap();
    for s in specs {
        std::fs::write(
            dir.join(format!("{}.json", s.id)),
            run::synth_log(s, good).to_json(),
        )
        .unwrap();
    }
}

#[test]
fn runner_signs_verdicts_per_build_and_diff_finds_regressions() {
    let d = tmp("run");
    let set = run::load_set(&["pack:dp-vessel".into(), "pack:uas".into()]).unwrap();
    assert_eq!(set.len(), 7);
    let (good, bad) = (d.join("good"), d.join("bad"));
    write_logs(&good, &set, true);
    write_logs(&bad, &set[..5], false); // two scenarios left without a log
    let cfg = |b: &str| run::RunConfig {
        build: b.into(),
        receiver: "RX-1".into(),
        capture: None,
    };
    let out = d.join("builds");
    let s1 = run::run_offline(
        &set,
        &run::Source::Logs {
            dir: good,
            t0_s: 0.0,
        },
        &cfg("fw-1"),
        &out,
        &SEED,
        "Lab",
        UTC,
    )
    .unwrap();
    assert_eq!((s1.pass, s1.fail, s1.not_run), (7, 0, 0));
    let s2 = run::run_offline(
        &set,
        &run::Source::Logs {
            dir: bad.clone(),
            t0_s: 0.0,
        },
        &cfg("fw-2"),
        &out,
        &SEED,
        "Lab",
        UTC,
    )
    .unwrap();
    assert_eq!((s2.fail, s2.not_run), (5, 2));
    // never overwritten
    assert!(run::run_offline(
        &set,
        &run::Source::Logs {
            dir: bad,
            t0_s: 0.0
        },
        &cfg("fw-2"),
        &out,
        &SEED,
        "Lab",
        UTC
    )
    .is_err());
    let pk = license::public_key_for(&SEED);
    for b in ["fw-1", "fw-2"] {
        assert!(kshana_pro::signed_dossier::verify(&out.join(b), &[pk])
            .unwrap()
            .passed());
        assert!(
            kshana_pro::signed_dossier::verify(&out.join(b).join("dp-slow-drift"), &[pk])
                .unwrap()
                .passed()
        );
    }
    let dd = diff::diff(&out.join("fw-1"), &out.join("fw-2"), &[pk]).unwrap();
    assert_eq!(dd.regressions, 7);
    assert!(
        dd.signature_problems.is_empty(),
        "{:?}",
        dd.signature_problems
    );
    let back = diff::diff(&out.join("fw-2"), &out.join("fw-1"), &[]).unwrap();
    assert_eq!((back.regressions, back.fixed), (0, 5));
    assert_eq!(
        back.scenarios
            .iter()
            .filter(|x| x.change == "now-run")
            .count(),
        2
    );
    let det = dd
        .scenarios
        .iter()
        .find(|x| x.scenario == "dp-stable-but-wrong")
        .unwrap();
    assert!(det
        .criteria
        .iter()
        .any(|c| c.id == "no-misleading" && c.trend == "worse"));
    assert!(diff::markdown(&dd).contains("| dp-slow-drift | pass | fail | regression | no |"));
    // tampering with a verdict is caught by the diff's signature check
    let v = out.join("fw-2").join("dp-slow-drift").join("verdict.json");
    let t = std::fs::read_to_string(&v)
        .unwrap()
        .replace("\"fail\"", "\"pass\"");
    std::fs::write(&v, t).unwrap();
    let dd = diff::diff(&out.join("fw-1"), &out.join("fw-2"), &[pk]).unwrap();
    assert!(
        dd.signature_problems
            .iter()
            .any(|p| p.contains("dp-slow-drift")),
        "{:?}",
        dd.signature_problems
    );
}

#[test]
fn incident_becomes_a_reproducible_regression_scenario() {
    // A field log: the receiver followed a 0.2 m/s drag for 300 s without warning.
    let truth = pack_spec("dp-position-reference-disagreement");
    let mut field = run::synth_log(&truth, false);
    for e in &mut field.epochs {
        e.t_s += 1000.0; // the log's own clock
    }
    let inc = incident::Incident::parse(
        r#"format = "kshana-testkit-incident/1"
id = "inc-2026-031"
title = "Supply vessel: GNSS walked off on station"
description = "Both DGNSS drifted east; gyro and taut wire steady."
kind = "spoofing"
onset_s = 300
end_s = 600
platform = "vessel"
sensors = ["dp-reference-2"]
detect_within_s = 30
"#,
    )
    .unwrap();
    let c = incident::convert(&inc, &field).unwrap();
    let s = &c.spec;
    assert!(s.trajectory.is_static());
    let t = &s.threats[0];
    assert_eq!(t.kind, Some(ThreatType::Spoofing));
    assert_eq!(t.technique, Some(Technique::PositionDrag));
    let off = t.final_offset_m.unwrap();
    assert!((off - 60.0).abs() < 3.0, "{off}");
    assert!((t.drag_bearing_deg.unwrap() - 90.0).abs() < 3.0);
    assert!(s.incident.as_ref().unwrap().derived.contains_key("truth"));
    assert_eq!(c.observed.epochs[0].t_s, 0.0);
    // The field log reproduces the incident's failure; a good receiver passes the scenario.
    let v = score::score(s, &c.observed, "field", "RX").unwrap();
    assert_eq!(v.verdict, "fail");
    assert!(v
        .criteria
        .iter()
        .any(|c| c.id == "no-misleading" && c.verdict == Verdict::NotMet));
    let g = score::score(s, &run::synth_log(s, true), "fix", "RX").unwrap();
    assert_eq!(g.verdict, "pass", "{:#?}", g.criteria);
    // It runs through the CLI path too (written, re-read).
    let back = Spec::parse(&s.to_toml()).unwrap();
    assert_eq!(back, *s);
    assert!(incident::Incident::parse("format = \"kshana-testkit-incident/1\"\nid=\"x\"\ntitle=\"t\"\nkind=\"meaconing\"\nonset_s=1\nend_s=2\n").is_err());
}

fn capture(bits: u8, extra: &str) -> Capture {
    Capture::parse(&format!(
        "format = \"kshana-testkit-capture/1\"\nbit_depth = {bits}\nbandwidth_hz = 10e6\n{extra}"
    ))
    .unwrap()
}

#[test]
fn replay_fidelity_scores_the_known_limits() {
    let jam = pack_spec("dp-gnss-jamming"); // J/S 45 dB, sensors gyro + dp-reference-2
    let f = fidelity::assess(&capture(1, ""), &jam);
    let q = f.limits.iter().find(|l| l.id == "quantisation").unwrap();
    assert_eq!(q.value, Some(1.96));
    assert_eq!(f.grade, "not-faithful");
    assert!(f
        .limits
        .iter()
        .any(|l| l.id == "sensors" && l.detail.contains("gyro")));
    // identical recorder and replayer oscillators: +3 dB (the doubling)
    let pn = "[[recorder_phase_noise]]\noffset_hz = 10\ndbc_hz = -80\n[[recorder_phase_noise]]\noffset_hz = 100000\ndbc_hz = -130\n[[replayer_phase_noise]]\noffset_hz = 10\ndbc_hz = -80\n[[replayer_phase_noise]]\noffset_hz = 100000\ndbc_hz = -130\n";
    let good = capture(
        8,
        &format!("clipping_fraction = 0.0001\nagc = \"fixed\"\nreference = \"gpsdo\"\nsensors_recorded = [\"gyro\", \"dp-reference-2\"]\nsensors_synchronised = true\n{pn}"),
    );
    let f = fidelity::assess(&good, &jam);
    let lo = f.limits.iter().find(|l| l.id == "lo-phase-noise").unwrap();
    assert!(lo.detail.contains("+3.0 dB"), "{}", lo.detail);
    assert_eq!(f.grade, "faithful", "{:#?}", f.limits);
    assert!(f.score >= 90);
    // AGC active under jamming limits; unsynchronised sensors invalidate
    let f = fidelity::assess(&capture(8, &format!("clipping_fraction = 0.0001\nagc = \"active\"\nsensors_recorded = [\"gyro\", \"dp-reference-2\"]\n{pn}")), &jam);
    assert!(f
        .limits
        .iter()
        .any(|l| l.id == "saturation-agc" && l.detail.contains("AGC active")));
    assert_eq!(f.grade, "not-faithful");
    // a free-running TCXO cannot replay a 100 ns timing criterion over the run
    let t = pack_spec("timing-meaconing-delay");
    let f = fidelity::assess(&capture(8, "reference = \"tcxo\""), &t);
    assert!(f
        .limits
        .iter()
        .any(|l| l.id == "reference" && l.severity == fidelity::Severity::Invalidating));
    // phase jitter: a flat -100 dBc/Hz over 10 Hz–1 MHz is sqrt(2e-10*1e6) rad
    let flat = vec![fidelity::PhaseNoisePoint {
        offset_hz: 1.0,
        dbc_hz: -100.0,
    }];
    let j = fidelity::phase_jitter_deg(&flat, 10.0, 1.0e6).unwrap();
    assert!(
        (j - (2.0e-10f64 * (1.0e6 - 10.0)).sqrt().to_degrees()).abs() < 0.01,
        "{j}"
    );
    assert!(Capture::parse("format = \"x\"\nbit_depth = 2\nbandwidth_hz = 1\n").is_err());
}

#[test]
fn replay_verdicts_carry_the_fidelity_statement() {
    let d = tmp("fid");
    let set = vec![pack_spec("dp-gnss-jamming")];
    let logs = d.join("logs");
    write_logs(&logs, &set, true);
    let cfg = run::RunConfig {
        build: "replay-1".into(),
        receiver: "RX".into(),
        capture: Some(capture(2, "agc = \"active\"")),
    };
    let s = run::run_offline(
        &set,
        &run::Source::Logs {
            dir: logs,
            t0_s: 0.0,
        },
        &cfg,
        &d.join("b"),
        &SEED,
        "Lab",
        UTC,
    )
    .unwrap();
    assert_eq!(
        s.scenarios[0].fidelity_grade.as_deref(),
        Some("not-faithful")
    );
    let v: score::ScenarioVerdict = serde_json::from_str(
        &std::fs::read_to_string(d.join("b/replay-1/dp-gnss-jamming/verdict.json")).unwrap(),
    )
    .unwrap();
    assert!(v.qualification.contains("cannot reproduce"));
    assert!(v.fidelity.unwrap().statement.contains("Replay fidelity"));
}

#[test]
fn live_run_drives_a_simulator_and_reads_the_receiver() {
    // A loopback "simulator" that acknowledges every command, and a "receiver" that serves
    // NMEA: no RF anywhere.
    let s = Spec::parse(&MINIMAL.replace("duration_s = 100", "duration_s = 40")).unwrap();
    let sim = TcpListener::bind("127.0.0.1:0").unwrap();
    let sim_addr = sim.local_addr().unwrap().to_string();
    let got: Arc<Mutex<Vec<String>>> = Arc::default();
    let g2 = got.clone();
    std::thread::spawn(move || {
        let (c, _) = sim.accept().unwrap();
        let mut w = c.try_clone().unwrap();
        for line in BufReader::new(c).lines() {
            let Ok(l) = line else { break };
            g2.lock().unwrap().push(l);
            if w.write_all(b"{\"Success\":true,\"State\":\"Started\"}\n")
                .is_err()
            {
                break;
            }
        }
    });
    let rx = TcpListener::bind("127.0.0.1:0").unwrap();
    let rx_addr = rx.local_addr().unwrap().to_string();
    let samples = compile::samples(&s, 1.0).unwrap();
    use kshana_eloran::sync::export::{NmeaTrajectoryExport, TrajectoryExport};
    let nmea = NmeaTrajectoryExport {
        rate_hz: 1.0,
        ..Default::default()
    }
    .render(&samples);
    std::thread::spawn(move || {
        let (mut c, _) = rx.accept().unwrap();
        let _ = c.write_all(nmea.as_bytes());
    });
    let live = run::Live {
        profile: compile::adapter_profile("skydel").unwrap(),
        sim_addr,
        receiver_addr: rx_addr,
        sim_trajectory_path: None,
        real_time: false,
        tail_s: 2.0,
    };
    let l: DeviceLog = run::run_live(&s, &live).unwrap();
    assert!(l.epochs.len() >= 30, "{}", l.epochs.len());
    assert!(l.source.as_deref().unwrap().starts_with("live:skydel"));
    let cmds = got.lock().unwrap().join("\n");
    assert!(
        cmds.contains("AddInterference") && cmds.contains("RemoveInterference"),
        "{cmds}"
    );
    assert!(cmds.contains("SetGpsStartTime"));
    // The receiver never flagged the jammer: the verdict says so.
    let v = score::score(&s, &l, "b", "r").unwrap();
    assert_eq!(v.verdict, "fail");
}

#[test]
fn tiers_are_explicit_pro_and_enterprise() {
    assert_eq!(license::explicit_tier("pro-test-kit"), Some(Tier::Pro));
    assert_eq!(
        license::explicit_tier("pro-test-kit-regression"),
        Some(Tier::Enterprise)
    );
    assert!(license::pro_command_features().contains(&"pro-test-kit"));
    assert!(license::pro_command_features().contains(&"pro-test-kit-regression"));
    let pro = Entitlement {
        licensee: "L".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&pro, "pro-test-kit", 0).is_ok());
    assert!(license::check(&pro, "pro-test-kit-regression", 0).is_err());
}

fn bin(args: &[&str], home: &Path) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
        .args(args)
        .env("KSHANA_PRO_HOME", home)
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

#[test]
fn cli_validate_compile_run_diff_and_incident() {
    let d = tmp("cli");
    let home = d.join("home");
    let p = |x: &Path| x.to_str().unwrap().to_string();
    let (ok, out) = bin(&["testkit", "packs"], &home);
    assert!(
        ok && out.contains("dp-slow-drift") && out.contains("rpcf-level-4"),
        "{out}"
    );
    let (ok, out) = bin(
        &[
            "testkit",
            "validate",
            "dp-vessel/dp-slow-drift",
            "--adapter",
            "gps-sdr-sim",
        ],
        &home,
    );
    assert!(ok && out.contains("NOT fully expressible"), "{out}");
    let cd = d.join("compiled");
    let (ok, out) = bin(
        &[
            "testkit",
            "compile",
            "dp-gnss-jamming",
            "--adapter",
            "spirent",
            "--out",
            &p(&cd),
        ],
        &home,
    );
    assert!(!ok && out.contains("requires the 'pro' tier"), "{out}");
    let (ok, out) = bin(
        &[
            "testkit",
            "compile",
            "dp-gnss-jamming",
            "--adapter",
            "spirent",
            "--out",
            &p(&cd),
            "--dev",
        ],
        &home,
    );
    assert!(ok && cd.join("COMPILE.json").is_file(), "{out}");
    // sample logs, two builds, diff
    let logs1 = d.join("l1");
    let logs2 = d.join("l2");
    std::fs::create_dir_all(&logs1).unwrap();
    std::fs::create_dir_all(&logs2).unwrap();
    for (dir, b) in [(&logs1, "good"), (&logs2, "bad")] {
        let (ok, out) = bin(
            &[
                "testkit",
                "sample",
                "uas-hover-drag-off",
                "--behaviour",
                b,
                "--out",
                &p(&dir.join("uas-hover-drag-off.json")),
            ],
            &home,
        );
        assert!(ok, "{out}");
    }
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex::encode(SEED))).unwrap();
    let builds = d.join("builds");
    for (b, l) in [("v1", &logs1), ("v2", &logs2)] {
        let (ok, out) = bin(
            &[
                "testkit",
                "run",
                "uas/uas-hover-drag-off",
                "--build",
                b,
                "--out",
                &p(&builds),
                "--logs",
                &p(l),
                "--signer",
                "Lab",
                "--key-file",
                &p(&key),
                "--utc",
                UTC,
                "--dev",
            ],
            &home,
        );
        assert!(ok, "{out}");
    }
    let pk = hex::encode(license::public_key_for(&SEED));
    let (ok, out) = bin(
        &["testkit", "verify", &p(&builds.join("v1")), "--trust", &pk],
        &home,
    );
    assert!(ok && out.contains("trusted"), "{out}");
    let (ok, out) = bin(
        &[
            "testkit",
            "diff",
            &p(&builds.join("v1")),
            &p(&builds.join("v2")),
            "--trust",
            &pk,
            "--dev",
        ],
        &home,
    );
    assert!(!ok && out.contains("1 regression(s)"), "{out}");
    let (ok, out) = bin(
        &[
            "testkit",
            "diff",
            &p(&builds.join("v1")),
            &p(&builds.join("v2")),
        ],
        &home,
    );
    assert!(!ok && out.contains("enterprise"), "{out}");
    // incident from a JSON field log
    let inc = d.join("inc.toml");
    std::fs::write(&inc, "format = \"kshana-testkit-incident/1\"\nid = \"inc-1\"\ntitle = \"Drone drifted\"\nkind = \"spoofing\"\nonset_s = 120\nend_s = 240\nplatform = \"uas\"\n").unwrap();
    let scen = d.join("inc-1.toml");
    let (ok, out) = bin(
        &[
            "testkit",
            "from-incident",
            "--log",
            &p(&logs2.join("uas-hover-drag-off.json")),
            "--input",
            "json",
            "--incident",
            &p(&inc),
            "--out",
            &p(&scen),
            "--dev",
        ],
        &home,
    );
    assert!(ok, "{out}");
    let s = Spec::load(&scen).unwrap();
    assert_eq!(s.incident.unwrap().id, "inc-1");
    assert!(d.join("inc-1.json").is_file());
    let (ok, out) = bin(
        &["testkit", "from-library", "hormuz-meaconing-delay", "--dev"],
        &home,
    );
    assert!(ok && out.contains("meaconing"), "{out}");
}

#[test]
fn cli_validate_compile_packs_and_library() {
    let d = tmp("cli");
    let home = d.join("home");
    let p = |x: &Path| x.to_str().unwrap().to_string();
    let (ok, out) = bin(&["testkit", "packs"], &home);
    assert!(
        ok && out.contains("dp-slow-drift") && out.contains("rpcf-level-4"),
        "{out}"
    );
    let (ok, out) = bin(&["testkit", "show", "rpcf/rpcf-level-2"], &home);
    assert!(ok && out.contains("resilience level 2"), "{out}");
    let (ok, out) = bin(
        &[
            "testkit",
            "validate",
            "dp-vessel/dp-slow-drift",
            "--adapter",
            "gps-sdr-sim",
        ],
        &home,
    );
    assert!(ok && out.contains("NOT fully expressible"), "{out}");
    let cd = d.join("compiled");
    let (ok, out) = bin(
        &[
            "testkit",
            "compile",
            "dp-gnss-jamming",
            "--adapter",
            "spirent",
            "--out",
            &p(&cd),
        ],
        &home,
    );
    assert!(!ok && out.contains("requires the 'pro' tier"), "{out}");
    let (ok, out) = bin(
        &[
            "testkit",
            "compile",
            "dp-gnss-jamming",
            "--adapter",
            "spirent",
            "--out",
            &p(&cd),
            "--dev",
        ],
        &home,
    );
    assert!(
        ok && cd.join("COMPILE.json").is_file() && cd.join("commands.txt").is_file(),
        "{out}"
    );
    let ex = d.join("pack");
    let (ok, out) = bin(
        &[
            "testkit",
            "packs",
            "export",
            "uas",
            "--out",
            &p(&ex),
            "--dev",
        ],
        &home,
    );
    assert!(ok && ex.join("uas-meaconing.toml").is_file(), "{out}");
    let (ok, out) = bin(
        &["testkit", "from-library", "hormuz-meaconing-delay", "--dev"],
        &home,
    );
    assert!(ok && out.contains("meaconing"), "{out}");
}
