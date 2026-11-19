// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! PICS (W1119 C1): suite lock and tamper detection, scoring on synthetic good and bad device
//! logs, converters, simulator export, mapping coverage, signed scorecard and the CLI.
//! Every device log here is SYNTHETIC.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::pics::log::{self, Behaviour, DeviceLog};
use kshana_pro::pics::score::{self, Scorecard};
use kshana_pro::pics::{self, export, Mapping, Suite, LOCK_V1, MAPPING_V1, SUITE_V1, UNCONFIRMED};
use kshana_pro::scorecards::levels::Verdict;

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pics-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn suite() -> Suite {
    Suite::parse(SUITE_V1).unwrap()
}

fn score_logs(logs: &[DeviceLog]) -> Scorecard {
    score::score(SUITE_V1, LOCK_V1, &Mapping::builtin(), logs).unwrap()
}

fn all(b: Behaviour, react: f64) -> Vec<DeviceLog> {
    let s = suite();
    s.scenarios
        .iter()
        .map(|sc| log::synth(&s, sc, b, react))
        .collect()
}

#[test]
fn lock_matches_the_suite() {
    let s = suite();
    if std::env::var("PICS_WRITE_LOCK").is_ok() {
        std::fs::write(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("pics/suite-v1.lock"),
            s.lock_text(),
        )
        .unwrap();
    }
    assert_eq!(s.lock_text(), LOCK_V1, "pics/suite-v1.lock is out of date");
    s.check_lock(LOCK_V1).unwrap();
    assert_eq!(s.scenarios.len(), 8);
    // Every class and technique of the brief is present.
    for t in [
        "cw",
        "chirp",
        "broadband",
        "simplistic",
        "intermediate",
        "time-push",
        "position-drag",
        "meaconing",
    ] {
        assert!(s.scenarios.iter().any(|x| x.technique == t), "{t}");
    }
}

#[test]
fn a_loosened_bar_is_refused_as_tampering() {
    // Loosen the CW detection bar from 10 s to 30 s.
    let tampered = SUITE_V1.replacen("detect_within_s = 10.0", "detect_within_s = 30.0", 1);
    assert_ne!(tampered, SUITE_V1);
    let e = score::score(
        &tampered,
        LOCK_V1,
        &Mapping::builtin(),
        &all(Behaviour::Good, 3.0),
    )
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("PICS-JAM-CW: bars changed since pre-registration"),
        "{e}"
    );
    // A scenario removed from the suite is reported too.
    let s = suite();
    let mut fewer = s.clone();
    fewer.scenarios.retain(|x| x.id != "PICS-SPF-MEACON");
    let e = fewer.check_lock(LOCK_V1).unwrap_err();
    assert!(
        e.contains("PICS-SPF-MEACON: pre-registered but missing"),
        "{e}"
    );
    // Changing a parameter (not a bar) keeps the lock: the lock covers the bars.
    let reparam = SUITE_V1.replacen("js_db = 40.0", "js_db = 41.0", 1);
    Suite::parse(&reparam).unwrap().check_lock(LOCK_V1).unwrap();
}

#[test]
fn a_well_behaved_device_passes_every_scenario_up_to_level_3() {
    let s = score_logs(&all(Behaviour::Good, 3.0));
    assert_eq!(s.totals.cases, 8);
    assert_eq!(
        s.totals.pass,
        8,
        "{:#?}",
        s.cases
            .iter()
            .map(|c| (
                &c.scenario,
                &c.verdict,
                &c.detection.detail,
                &c.recovery.detail
            ))
            .collect::<Vec<_>>()
    );
    for c in &s.cases {
        assert_eq!(c.detection_s, Some(3.0), "{}", c.scenario);
        assert_eq!(c.misleading_epochs, 0, "{}", c.scenario);
        // The device reports no fix for 20 s after the threat, then is correct: the first
        // correct epoch is 20 s after the end.
        assert_eq!(c.recovery_s, Some(20.0), "{}", c.scenario);
        assert_eq!(c.detected_by.as_deref(), Some("flag"));
        assert_eq!(c.levels.l1, Verdict::Met);
        assert_eq!(c.levels.l2, Verdict::Met);
        assert_eq!(c.levels.l3, Verdict::Met);
    }
    // Jamming: the fix is withdrawn from 3 s, so availability is 3/300 and L4 is not met;
    // spoofing: the good device keeps a correct (flagged) output, L4 met.
    for c in &s.cases {
        if c.class == "jamming" {
            assert_eq!(c.full.verdict, Verdict::NotMet, "{}", c.scenario);
            assert!((c.threat_availability.unwrap() - 0.01).abs() < 1e-9);
        } else {
            assert_eq!(c.full.verdict, Verdict::Met, "{}", c.scenario);
        }
    }
    assert_eq!(s.levels.highest_supported, 3);
    assert!(score::check_bars(&s).is_empty());
}

#[test]
fn a_device_that_follows_the_threat_fails_with_the_right_reasons() {
    let s = score_logs(&all(Behaviour::Bad, 3.0));
    assert_eq!(s.totals.fail, 8);
    assert_eq!(s.levels.highest_supported, 0);
    let case = |id: &str| s.cases.iter().find(|c| c.scenario == id).unwrap();
    for c in &s.cases {
        assert_eq!(c.detection.verdict, Verdict::NotMet, "{}", c.scenario);
        assert_eq!(c.no_misleading.verdict, Verdict::NotMet, "{}", c.scenario);
        assert_eq!(c.recovery.verdict, Verdict::NotMet, "{}", c.scenario);
        assert_eq!(c.recovery_s, Some(120.0), "{}", c.scenario);
        assert_eq!(c.levels.l1, Verdict::NotMet);
    }
    // CW: the bad device drifts 5 ns/s unflagged, so the 100 ns bar is crossed after about
    // 20 s and it stays beyond until 120 s after the threat: about 280 + 120 epochs.
    let cw = case("PICS-JAM-CW");
    assert!(
        (395..=402).contains(&cw.misleading_epochs),
        "{}",
        cw.misleading_epochs
    );
    // Simplistic: the 10 km false position is reported unflagged for the whole threat and
    // the 120 s recovery: 300 + 120 epochs, the largest error close to 10 km.
    let sim = case("PICS-SPF-SIMPLE");
    assert_eq!(sim.misleading_epochs, 420);
    let h = sim.max_unflagged_h_error_m.unwrap();
    assert!((h - 10_000.0).abs() < 5.0, "{h}");
    // Time push: 10 ns/s from 20 s; 100 ns (+ the 4 ns wander) is crossed about 10 s later.
    let tp = case("PICS-SPF-TPUSH");
    let t = tp.max_unflagged_time_error_ns.unwrap();
    assert!((t - 2800.0).abs() < 5.0, "{t}");
    assert!(
        (385..=392).contains(&tp.misleading_epochs),
        "{}",
        tp.misleading_epochs
    );
    assert_eq!(
        tp.max_unflagged_h_error_m, None,
        "a timing-only scenario scores no position"
    );
    // Drag: 0.05 m/s2 to 5 m/s; after 280 s the offset is 250 + 5 * 180 = 1150 m.
    let dr = case("PICS-SPF-DRAG");
    let h = dr.max_unflagged_h_error_m.unwrap();
    assert!((h - 1150.0).abs() < 2.0, "{h}");
    assert_eq!(dr.bounded.verdict, Verdict::NotMet);
}

#[test]
fn late_detection_fails_the_bars_but_keeps_the_levels_it_earned() {
    let su = suite();
    let sc = su.scenario("PICS-JAM-CW").unwrap();
    let l = log::synth(&su, sc, Behaviour::Good, 15.0);
    let s = score_logs(&[l]);
    let c = &s.cases[0];
    assert_eq!(c.detection.verdict, Verdict::NotMet);
    assert_eq!(c.detection_s, Some(15.0));
    assert_eq!(c.verdict, "fail");
    assert_eq!(c.levels.l2, Verdict::Met);
}

#[test]
fn spoofing_needs_an_explicit_flag_and_jamming_accepts_a_withdrawn_fix() {
    let su = suite();
    for (id, want) in [
        ("PICS-SPF-MEACON", Verdict::NotMet),
        ("PICS-JAM-BB", Verdict::Met),
    ] {
        let sc = su.scenario(id).unwrap();
        let mut l = log::synth(&su, sc, Behaviour::Good, 3.0);
        // Replace every flag by a withdrawn fix.
        for e in &mut l.epochs {
            if e.flagged() {
                e.jamming = Some(false);
                e.spoofing = Some(false);
                e.integrity = Some("ok".into());
                e.fix_valid = Some(false);
            }
        }
        let c = &score_logs(&[l]).cases[0];
        assert_eq!(c.detection.verdict, want, "{id}: {}", c.detection.detail);
    }
}

#[test]
fn missing_data_is_not_assessed_never_passed() {
    let su = suite();
    let sc = su.scenario("PICS-SPF-TPUSH").unwrap();
    let mut l = log::synth(&su, sc, Behaviour::Good, 3.0);
    for e in &mut l.epochs {
        e.time_error_ns = None;
    }
    let c = &score_logs(&[l]).cases[0];
    assert_eq!(c.no_misleading.verdict, Verdict::NotAssessed);
    assert_eq!(c.recovery.verdict, Verdict::NotAssessed);
    assert_eq!(c.verdict, "incomplete");
    // A log that stops 30 s after the threat cannot show a 60 s recovery window.
    let sc = su.scenario("PICS-JAM-CW").unwrap();
    let mut l = log::synth(&su, sc, Behaviour::Bad, 3.0);
    l.epochs.retain(|e| e.t_s <= 450.0);
    let c = &score_logs(&[l]).cases[0];
    assert_eq!(c.recovery.verdict, Verdict::NotAssessed);
}

#[test]
fn a_log_for_another_scenario_or_a_bad_log_is_refused() {
    let su = suite();
    let mut l = log::synth(
        &su,
        su.scenario("PICS-JAM-CW").unwrap(),
        Behaviour::Good,
        3.0,
    );
    l.scenario = "PICS-NOPE".into();
    assert!(score::score(SUITE_V1, LOCK_V1, &Mapping::builtin(), &[l.clone()]).is_err());
    l.scenario = "PICS-JAM-CW".into();
    l.epochs[5].t_s = 1.0; // time going backwards
    assert!(score::score(SUITE_V1, LOCK_V1, &Mapping::builtin(), &[l.clone()]).is_err());
    let mut j = log::synth(
        &su,
        su.scenario("PICS-JAM-CW").unwrap(),
        Behaviour::Good,
        3.0,
    )
    .to_json();
    j = j.replacen("kshana-pics-device-log/1", "something-else/1", 1);
    assert!(DeviceLog::parse(&j).is_err());
}

fn nmea_line(body: &str) -> String {
    let ck = body.bytes().fold(0u8, |a, b| a ^ b);
    format!("${body}*{ck:02X}\r\n")
}

#[test]
fn nmea_and_timing_logs_convert_to_the_common_form() {
    let mut text = String::new();
    for s in 0..5 {
        let hh = format!("1200{s:02}.00");
        text.push_str(&nmea_line(&format!(
            "GPGGA,{hh},5926.2200,N,02445.2160,E,1,08,0.9,30.0,M,18.0,M,,"
        )));
        text.push_str(&nmea_line(&format!(
            "GPRMC,{hh},A,5926.2200,N,02445.2160,E,0.0,0.0,191126,,,A"
        )));
    }
    let l = log::from_nmea(&text, "PICS-JAM-CW", 100.0).unwrap();
    assert_eq!(l.epochs.len(), 5);
    assert_eq!(l.epochs[0].t_s, 100.0);
    assert_eq!(l.epochs[4].t_s, 104.0);
    let e = &l.epochs[2];
    assert_eq!(e.fix_valid, Some(true));
    assert!((e.lat_deg.unwrap() - 59.437).abs() < 1e-6);
    assert!((e.lon_deg.unwrap() - 24.7536).abs() < 1e-6);
    // Timing: a TimeLab single-column phase log (s) at 1 s.
    let inp = kshana::clock_log::ClockLogInput {
        format: "timelab".into(),
        text: Some("1e-9\n2e-9\n-3e-9\n".into()),
        tau0_s: Some(1.0),
        ..Default::default()
    };
    let t = log::from_timing(&inp, "PICS-SPF-TPUSH", 10.0).unwrap();
    assert_eq!(t.epochs.len(), 3);
    assert_eq!(t.epochs[1].t_s, 11.0);
    assert!((t.epochs[2].time_error_ns.unwrap() + 3.0).abs() < 1e-9);
    // Merge the timing errors into the position log by nearest epoch.
    let mut m = l.clone();
    let mut tt = t.clone();
    for e in &mut tt.epochs {
        e.t_s += 90.0;
    }
    m.merge_time_errors(&tt, 0.5);
    assert!((m.epochs[0].time_error_ns.unwrap() - 1.0).abs() < 1e-9);
    assert_eq!(m.epochs[4].time_error_ns, None);
}

#[test]
fn scenarios_export_to_the_simulator_profiles() {
    let su = suite();
    let cw = su.scenario("PICS-JAM-CW").unwrap();
    for p in export::PROFILES {
        let x = export::export(&su, cw, p).unwrap();
        assert_eq!(x.degradation.events.len(), 1);
        assert!(x.spoofer_offsets.is_empty());
        // Either the jammer is commanded at onset and stopped at the end, or the profile
        // says it cannot express it.
        if x.schedule.unsupported.is_empty() {
            let ts: Vec<f64> = x.schedule.entries.iter().map(|e| e.t_s).collect();
            assert!(ts.contains(&120.0) && ts.contains(&420.0), "{p}: {ts:?}");
        }
    }
    let tp = su.scenario("PICS-SPF-TPUSH").unwrap();
    let x = export::export(&su, tp, "skydel").unwrap();
    assert_eq!(x.spoofer_offsets.len(), 301);
    assert_eq!(x.spoofer_offsets[0].t_s, 120.0);
    assert_eq!(x.spoofer_offsets[20].time_ns, 0.0);
    assert!((x.spoofer_offsets[300].time_ns - 2800.0).abs() < 1e-9);
    assert_eq!(x.schedule.markers.len(), 2, "spoof start and stop markers");
    assert!(export::export(&su, tp, "nope").is_err());
}

#[test]
fn the_mapping_covers_every_standard_and_scenario_and_cites_only_known_clauses() {
    let su = suite();
    let m = Mapping::parse(MAPPING_V1).unwrap();
    assert!(
        m.coverage_problems(&su).is_empty(),
        "{:?}",
        m.coverage_problems(&su)
    );
    for req in pics::REQUIRED_STANDARDS {
        assert!(m.rows.iter().any(|r| r.standard == req), "{req}");
    }
    // Standards whose clause numbers the research does not give are all "to be confirmed".
    for r in &m.rows {
        if !pics::CITABLE.contains(&r.standard.as_str()) {
            assert_eq!(r.clause, UNCONFIRMED, "{}: {}", r.standard, r.clause);
        }
    }
    // Circ.1575 paragraphs and the G.8272.1 clauses are cited.
    assert!(m
        .rows
        .iter()
        .any(|r| r.standard == "imo-msc1-circ-1575" && r.clause.contains("53 to 57")));
    assert!(m
        .rows
        .iter()
        .any(|r| r.standard == "itu-t-g8272-1" && r.clause.contains("8.3.1")));
    // Coverage checking catches an invented clause and an unmapped scenario.
    let mut bad = m.clone();
    bad.rows
        .iter_mut()
        .find(|r| r.standard == "iec-61108-1")
        .unwrap()
        .clause = "clause 5.6.1".into();
    bad.rows
        .retain(|r| !r.scenarios.iter().all(|s| s == "PICS-SPF-TPUSH") || r.scenarios.len() > 1);
    for r in &mut bad.rows {
        r.scenarios.retain(|s| s != "PICS-SPF-DRAG");
    }
    let p = bad.coverage_problems(&su);
    assert!(p.iter().any(|x| x.contains("clause 5.6.1")), "{p:?}");
    assert!(
        p.iter()
            .any(|x| x.contains("PICS-SPF-DRAG is mapped to nothing")),
        "{p:?}"
    );
}

#[test]
fn the_signed_scorecard_detects_a_changed_file_and_changed_bars() {
    let logs = all(Behaviour::Good, 3.0);
    let s = score_logs(&logs);
    let d = tmp("signed");
    score::write_dir(&d, &s, SUITE_V1, &logs).unwrap();
    let seed = [7u8; 32];
    let pk = kshana_pro::license::public_key_for(&seed);
    kshana_pro::signed_dossier::sign(&d, &seed, "Test Lab", "2026-11-19T00:00:00Z").unwrap();
    assert!(kshana_pro::signed_dossier::verify(&d, &[pk])
        .unwrap()
        .passed());
    // Loosen a bar inside the scorecard: the signature fails and the bars no longer hash.
    let p = d.join("scorecard.json");
    let text = std::fs::read_to_string(&p).unwrap();
    let t2 = text.replacen("\"detect_within_s\": 10.0", "\"detect_within_s\": 99.0", 1);
    assert_ne!(t2, text);
    std::fs::write(&p, &t2).unwrap();
    let v = kshana_pro::signed_dossier::verify(&d, &[pk]).unwrap();
    assert!(!v.passed());
    assert!(
        v.problems.iter().any(|x| x.contains("scorecard.json")),
        "{:?}",
        v.problems
    );
    let probs = score::check_bars_json(&serde_json::from_str(&t2).unwrap());
    assert!(probs.iter().any(|x| x.contains("do not hash")), "{probs:?}");
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
fn cli_lists_shows_scores_and_verifies() {
    let d = tmp("cli");
    let home = d.join("home");
    let (ok, out) = bin(&["pics", "list"], &home);
    assert!(ok, "{out}");
    assert!(out.contains("PICS-SPF-MEACON") && out.contains("1.0.0"));
    let (ok, out) = bin(&["pics", "show", "PICS-SPF-TPUSH"], &home);
    assert!(
        ok && out.contains("push_rate_ns_per_s") && out.contains("itu-t-g8273-2"),
        "{out}"
    );
    let log_path = d.join("cw.json");
    let (ok, out) = bin(
        &[
            "pics",
            "sample",
            "PICS-JAM-CW",
            "--behaviour",
            "good",
            "--out",
            log_path.to_str().unwrap(),
        ],
        &home,
    );
    assert!(ok, "{out}");
    // Score without a licence: refused (evaluation tier).
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex_of(&[9u8; 32]))).unwrap();
    let outdir = d.join("out");
    let base = [
        "pics",
        "score",
        log_path.to_str().unwrap(),
        "--out",
        outdir.to_str().unwrap(),
        "--signer",
        "Lab",
        "--key-file",
        key.to_str().unwrap(),
        "--utc",
        "2026-11-19T00:00:00Z",
    ];
    let (ok, out) = bin(&base, &home);
    assert!(!ok, "{out}");
    let mut dev = base.to_vec();
    dev.push("--dev");
    let (ok, out) = bin(&dev, &home);
    assert!(ok, "{out}");
    assert!(out.contains("1 cases: 1 pass"), "{out}");
    let pk = hex_of(&kshana_pro::license::public_key_for(&[9u8; 32]));
    let (ok, out) = bin(
        &["pics", "verify", outdir.to_str().unwrap(), "--trust", &pk],
        &home,
    );
    assert!(ok, "{out}");
    let (ok, out) = bin(
        &["pics", "export", "PICS-JAM-CHIRP", "--profile", "skydel"],
        &home,
    );
    assert!(
        ok && out.contains("kshana-pics-simulator-export/1"),
        "{out}"
    );
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
