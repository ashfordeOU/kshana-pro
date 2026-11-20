// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Incident reconstruction and dispute evidence (research 2026-11-20 gaps E06): the log
//! time-integrity checker (B0603) on synthetic logs with known truth, and the commercial
//! dispute evidence report (B0606) on a synthetic DP-charter case, signed and verified.

use std::path::PathBuf;

use kshana_pro::forensics::dispute;
use kshana_pro::forensics::logtime::{self, synth, synth::Fault, LogSpec, Report, Request};
use kshana_pro::license::{explicit_tier, public_key_for, Tier};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kpro-e06-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn log(name: &str, format: &str) -> LogSpec {
    LogSpec {
        name: name.into(),
        kind: None,
        format: format.into(),
        path: "x".into(),
        u_s: 0.5,
        split: true,
        time_field: None,
        offset_limit_s: None,
        reference_group: None,
        reference: None,
    }
}

fn check(spec: LogSpec, text: &str, window: bool) -> Report {
    let req = Request {
        title: "test".into(),
        window_start_utc: window.then(|| "2026-06-01T08:00:00Z".into()),
        window_end_utc: window.then(|| "2026-06-01T08:30:00Z".into()),
        logs: vec![spec],
        ..Default::default()
    };
    logtime::check_texts(&req, &[text.as_bytes().to_vec()]).expect("checks")
}

fn clock<'a>(r: &'a Report, name: &str) -> &'a logtime::ClockReport {
    r.clocks
        .iter()
        .find(|c| c.clock == name)
        .unwrap_or_else(|| {
            panic!(
                "no clock {name}: {:?}",
                r.clocks.iter().map(|c| &c.clock).collect::<Vec<_>>()
            )
        })
}

fn kinds(c: &logtime::ClockReport) -> Vec<&str> {
    c.faults
        .iter()
        .filter(|f| f.severity == "fault")
        .map(|f| f.kind.as_str())
        .collect()
}

// ---------------------------------------------------------------------------------------
// B0603
// ---------------------------------------------------------------------------------------

#[test]
fn clock_running_backwards_is_detected_and_corrected_within_1s() {
    // Acceptance (B0603): a log with no reference whose clock steps back 37 s and runs on.
    let (text, truth) = synth::bare_csv(Fault::Backwards);
    let r = check(log("console", "csv"), &text, true);
    let c = clock(&r, "console");
    assert_eq!(kinds(c), vec!["backward"], "{:#?}", c.faults);
    let f = &c.faults[0];
    assert_eq!(f.index, synth::AT);
    assert!((f.value + 37.0).abs() < 1e-6, "step {}", f.value);
    // The corrected timeline matches the truth within 1 s everywhere.
    assert_eq!(c.timeline.len(), truth.len());
    for (row, t) in c.timeline.iter().zip(&truth) {
        assert!(
            (row.corrected - t).abs() <= 1.0,
            "record {}: corrected {} truth {t}",
            row.index,
            row.corrected
        );
    }
    assert_eq!(c.segments.len(), 2);
    assert_eq!(c.segments[0].anchored_by, "assumed");
    assert_eq!(c.segments[1].anchored_by, "bridged");
    // The untrusted interval runs from the step to the end of the log.
    assert_eq!(c.intervals.len(), 1);
    assert_eq!(c.intervals[0].start_utc, "2026-06-01T08:10:00.000Z");
    assert!(r.verdict.starts_with("TIME INTEGRITY FAULTS"));
}

#[test]
fn vdr_week_rollover_named_and_outvoted_by_the_other_receivers() {
    let (text, truth) = synth::vdr(Fault::WeekRollover);
    let r = check(log("vdr", "nmea"), &text, true);
    assert_eq!(r.clocks.len(), 3, "GP, GN and II");
    assert_eq!(clock(&r, "vdr:GP").status, "intact");
    assert_eq!(clock(&r, "vdr:II").status, "intact");
    let gn = clock(&r, "vdr:GN");
    assert!(kinds(gn).contains(&"week-rollover"), "{:#?}", gn.faults);
    assert_eq!(gn.faults[0].index, synth::AT);
    // GN's corrected times follow the reference: within 1 s of the truth.
    for (row, t) in gn.timeline.iter().zip(&truth) {
        assert!((row.corrected - t).abs() <= 1.0, "{} vs {t}", row.corrected);
    }
    // The cross-log vote names GN from 08:10 on.
    assert_eq!(r.cross_log.clocks_compared.len(), 3);
    let ev = &r.cross_log.events;
    assert_eq!(ev.len(), 1, "{ev:#?}");
    assert_eq!(ev[0].status, "source-outlier");
    assert_eq!(ev[0].outliers, vec!["vdr:GN".to_string()]);
    assert_eq!(ev[0].start_utc, "2026-06-01T08:10:00.000Z");
    assert!(r
        .integrity_intervals
        .iter()
        .any(|i| i.clock == "cross-log:vdr:GN"));
}

#[test]
fn clean_vdr_is_intact_and_bare_log_is_unverified() {
    let (text, _) = synth::vdr(Fault::None);
    let r = check(log("vdr", "nmea"), &text, false);
    assert!(r.verdict.starts_with("INTACT"), "{}", r.verdict);
    assert!(r.clocks.iter().all(|c| c.status == "intact"));
    assert!(r.integrity_intervals.is_empty());
    let gp = clock(&r, "vdr:GP");
    assert!((gp.median_offset_s.unwrap() + 0.12).abs() < 0.01);

    let (bare, _) = synth::bare_csv(Fault::None);
    let r = check(log("console", "csv"), &bare, true);
    assert_eq!(r.clocks[0].status, "unverified");
    assert!(r.verdict.starts_with("UNVERIFIED"));
}

#[test]
fn frozen_clock_without_a_spurious_catch_up_step() {
    let (text, _) = synth::vdr(Fault::Frozen);
    let r = check(log("vdr", "nmea"), &text, false);
    let gn = clock(&r, "vdr:GN");
    assert_eq!(kinds(gn), vec!["frozen"], "{:#?}", gn.faults);
    assert_eq!(gn.intervals.len(), 1);
    let i = &gn.intervals[0];
    assert!((i.duration_s - 19.0).abs() < 0.5, "{i:?}");
}

#[test]
fn drift_rate_measured() {
    let (text, _) = synth::vdr(Fault::Drift);
    let r = check(log("vdr", "nmea"), &text, false);
    let gn = clock(&r, "vdr:GN");
    assert_eq!(kinds(gn), vec!["drift"], "{:#?}", gn.faults);
    let d = gn.segments[0].drift_ppm.unwrap();
    assert!((d - 200.0).abs() < 5.0, "drift {d}");
}

#[test]
fn gps_time_logged_as_utc() {
    let (text, _) = synth::vdr(Fault::GpsAsUtc);
    let r = check(log("vdr", "nmea"), &text, false);
    let gn = clock(&r, "vdr:GN");
    assert!(kinds(gn).contains(&"gps-utc-offset"), "{:#?}", gn.faults);
}

#[test]
fn cpdlc_aircraft_clock_backwards_and_cross_log() {
    let text = synth::acars(Fault::Backwards);
    let mut spec = log("cpdlc", "acars-json");
    spec.u_s = 2.0;
    spec.offset_limit_s = Some(3.0);
    let r = check(spec, &text, false);
    assert_eq!(r.clocks.len(), 3);
    assert_eq!(clock(&r, "cpdlc:D-EFGH").status, "intact");
    assert_eq!(clock(&r, "cpdlc:F-IJKL").status, "intact");
    let g = clock(&r, "cpdlc:G-ABCD");
    assert!(kinds(g).contains(&"backward"), "{:#?}", g.faults);
    assert!(r
        .cross_log
        .events
        .iter()
        .any(|e| e.outliers == vec!["cpdlc:G-ABCD".to_string()]));
}

#[test]
fn whole_log_a_week_era_early_is_an_absolute_epoch_fault() {
    let mut s = String::from("time\n");
    for k in 0..100 {
        let t = synth::T0 + k as f64 - 1024.0 * 604_800.0;
        s.push_str(&format!("{}\n", logtime::iso_ms(t)));
    }
    let r = check(log("fdm", "csv"), &s, true);
    let f = &r.clocks[0].faults;
    assert!(
        f.iter()
            .any(|x| x.kind == "absolute-epoch" && x.detail.contains("week-rollover")),
        "{f:#?}"
    );
}

#[test]
fn demo_runs_from_files_and_is_deterministic() {
    let d = tmp("logtime-demo");
    synth::write_demo(&d).unwrap();
    let text = std::fs::read_to_string(d.join("request.toml")).unwrap();
    let req = Request::parse(&text).unwrap();
    let a = logtime::check(&req, &d).unwrap();
    let b = logtime::check(&req, &d).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    assert_eq!(a.format, logtime::FORMAT);
    assert!(a.label.starts_with("MODELLED"));
    assert_eq!(a.inputs.len(), 3);
    assert!(a.inputs.iter().all(|i| i.sha256.len() == 64));
    // vdr:GP, vdr:GN, vdr:II, three aircraft, the console.
    assert_eq!(a.clocks.len(), 7);
    assert_eq!(clock(&a, "console").status, "faults");
    assert_eq!(clock(&a, "vdr:GN").status, "faults");
    // Clocks are voted within their reference group (one per log here): one event each.
    let groups: Vec<&str> = a
        .cross_log
        .events
        .iter()
        .map(|e| e.group.as_str())
        .collect();
    assert_eq!(groups, vec!["cpdlc", "vdr"]);
    let csv = logtime::timeline_csv(&a);
    assert!(csv.starts_with("clock,index,line,log_utc"));
    let out = d.join("out");
    logtime::write_dir(&out, &a, &text, &[]).unwrap();
    assert!(out.join(logtime::RESULT_FILE).exists());
    assert!(std::fs::read_to_string(out.join("README.txt"))
        .unwrap()
        .contains("Integrity statement"));
}

#[test]
fn cli_log_time_json_and_exit_code() {
    let d = tmp("logtime-cli");
    let bin = env!("CARGO_BIN_EXE_kshana-pro");
    let st = std::process::Command::new(bin)
        .args(["forensics", "log-time-synth"])
        .arg(&d)
        .status()
        .unwrap();
    assert!(st.success());
    let out = std::process::Command::new(bin)
        .args(["forensics", "log-time"])
        .arg(d.join("request.toml"))
        .args(["--json", "--dev"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["format"], logtime::FORMAT);
    // Without a licence the feature is refused.
    let out = std::process::Command::new(bin)
        .args(["forensics", "log-time"])
        .arg(d.join("request.toml"))
        .env("HOME", &d)
        .env("XDG_CONFIG_HOME", &d)
        .env("APPDATA", &d)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("pro-log-time-integrity"));
}

// ---------------------------------------------------------------------------------------
// B0606
// ---------------------------------------------------------------------------------------

fn demo_case(name: &str) -> (PathBuf, dispute::Request) {
    let d = tmp(name);
    dispute::synth::write_demo(&d).unwrap();
    let req =
        dispute::Request::parse(&std::fs::read_to_string(d.join("request.toml")).unwrap()).unwrap();
    (d, req)
}

#[test]
fn dispute_report_on_the_synthetic_dp_case() {
    let (d, req) = demo_case("dispute");
    let (r, _) = dispute::assess(&req, &d, &[]).unwrap();
    assert_eq!(r.format, dispute::FORMAT);
    assert!(r.label.starts_with("MODELLED evidence"));
    assert!(r.label.contains("NOT a legal finding"));
    // One merged period: the jamming episode (≈ 08:10–08:18) and the VDR GN clock rollover
    // (08:10–08:20), corroborated by the AIS neighbours and the log clocks.
    assert_eq!(r.periods.len(), 1, "{:#?}", r.periods);
    let p = &r.periods[0];
    assert_eq!(p.agreement, "corroborated");
    assert!(p.independent_sources >= 3, "{p:#?}");
    assert!(p.kinds.contains(&"gnss-jamming".to_string()));
    assert!(p.kinds.contains(&"timing-integrity-lost".to_string()));
    assert!(p.causes.contains(&"external-interference".to_string()));
    assert!(
        (560.0..=640.0).contains(&r.integrity_lost_s),
        "{}",
        r.integrity_lost_s
    );
    assert!(r.integrity_lost_min_s <= r.integrity_lost_s);
    assert!(r.integrity_lost_max_s >= r.integrity_lost_s);
    // DP downtime 08:10:30–08:25:00 (REPORTED).
    assert!(
        (r.dp_downtime_s - 870.0).abs() < 1e-6,
        "{}",
        r.dp_downtime_s
    );
    // The charterer's 20-minute off-hire claim is about half explained.
    let c = r
        .claims
        .iter()
        .find(|c| c.label == "Charterer off-hire")
        .unwrap();
    assert_eq!(c.claimed_s, 1200.0);
    assert!((0.45..0.55).contains(&c.fraction_explained), "{c:?}");
    assert!((c.not_explained_s + c.with_integrity_loss_s - 1200.0).abs() < 1e-6);
    // Five clause templates, each answered, citing Kshana Pro pack IDs.
    assert_eq!(r.clauses.len(), 5);
    let all: Vec<&String> = r
        .clauses
        .iter()
        .flat_map(|c| c.answers.iter().flat_map(|a| a.cites.iter()))
        .collect();
    assert!(all
        .iter()
        .any(|c| c.starts_with("KS:kshana-pro/maritime-actor-victim/v1#episode-1@")));
    assert!(all
        .iter()
        .any(|c| c.starts_with("KS:kshana-pro/log-time-integrity/v1")));
    assert!(all.iter().any(|c| c.starts_with("REPORTED:dp-status.csv@")));
    let clause = |id: &str| r.clauses.iter().find(|c| c.id == id).unwrap();
    let off = clause("Clause 17");
    assert_eq!(off.answers[0].answer, "indicated");
    assert_eq!(off.answers[1].answer, "indicated");
    assert!(off.answers[1].basis.contains("external interference"));
    assert_eq!(off.answers[2].answer, "indicated", "longest ≥ 0.1 h");
    let dp = clause("DP performance");
    assert_eq!(dp.answers[0].value, Some(870.0));
    assert_eq!(dp.answers[1].answer, "indicated");
    let v = dp.answers[1].value.unwrap();
    assert!(
        (450.0..600.0).contains(&v),
        "DP downtime with lost integrity {v}"
    );
    assert!(dp.answers[2].basis.contains("DGNSS1"));
    assert!(!dp.answers[2].basis.contains("HPR"));
    let dev = clause("Deviation");
    assert_eq!(
        dev.answers[0].answer, "not-indicated",
        "jamming, not spoofing"
    );
    let sp = clause("Safe port");
    assert_eq!(sp.answers[0].answer, "indicated");
    assert_eq!(sp.answers[2].answer, "indicated");
    let lay = clause("Laycan");
    assert_eq!(lay.answers[0].answer, "indicated");
    // Every clause states the time base from the log check.
    assert!(r
        .clauses
        .iter()
        .all(|c| c.answers.last().unwrap().question.contains("time stamps")));
    assert!(r.time_base.starts_with("TIME INTEGRITY FAULTS"));
}

#[test]
fn dispute_directory_is_signed_and_verifies() {
    let (d, req) = demo_case("dispute-sign");
    let (r, asm) = dispute::assess(&req, &d, &[]).unwrap();
    let out = d.join("out");
    let text = std::fs::read_to_string(d.join("request.toml")).unwrap();
    dispute::write_dir(&out, &r, &asm, &text).unwrap();
    for f in [
        dispute::RESULT_FILE,
        "spans.csv",
        "README.txt",
        "inputs/dp-status.csv",
        "maritime/evidence-pack.json",
        "log-time/log-time-report.json",
    ] {
        assert!(out.join(f).exists(), "{f}");
    }
    let seed = [7u8; 32];
    kshana_pro::signed_dossier::sign(&out, &seed, "Test Issuer", "2026-11-20T00:00:00Z").unwrap();
    let v = kshana_pro::signed_dossier::verify(&out, &[public_key_for(&seed)]).unwrap();
    assert!(v.passed(), "{v:?}");
    // The report on disk names the cited pack by the hash of the pack written beside it.
    let back: dispute::Report =
        serde_json::from_slice(&std::fs::read(out.join(dispute::RESULT_FILE)).unwrap()).unwrap();
    assert_eq!(back.periods.len(), r.periods.len());
    let readme = std::fs::read_to_string(out.join("README.txt")).unwrap();
    assert!(readme.contains("NOT a legal finding"));
}

#[test]
fn dispute_from_a_signed_maritime_pack_directory() {
    let (d, _) = demo_case("dispute-pack");
    // Make the maritime pack separately, signed, then cite it by directory.
    let mreq = kshana_pro::forensics::maritime::MaritimeRequest::from_file(
        &d.join("maritime/request.toml"),
    )
    .unwrap();
    let inp = mreq.load().unwrap();
    let pack = kshana_pro::forensics::maritime::analyse(&inp).unwrap();
    let pd = d.join("pack");
    kshana_pro::forensics::maritime::write_pack(&pd, &pack, &inp).unwrap();
    let seed = [9u8; 32];
    kshana_pro::signed_dossier::sign(&pd, &seed, "Pack Issuer", "2026-11-20T00:00:00Z").unwrap();
    let req = dispute::Request::parse(
        "title = 'from pack'\nwindow_start_utc = '2026-06-01T08:00:00Z'\nwindow_end_utc = '2026-06-01T08:30:00Z'\nmaritime_pack = 'pack'\n\n[[clauses]]\ntemplate = 'safe-port'\nid = 'SP'\n",
    )
    .unwrap();
    let (r, asm) = dispute::assess(&req, &d, &[public_key_for(&seed)]).unwrap();
    assert_eq!(r.cited.len(), 1);
    assert!(
        r.cited[0].provenance.contains("signature valid"),
        "{}",
        r.cited[0].provenance
    );
    assert!(r.cited[0].provenance.contains("key trusted"));
    assert_eq!(asm.cited_files.len(), 1);
    assert_eq!(r.periods.len(), 1);
    // Single maritime source, but the neighbours corroborate it.
    assert_eq!(r.periods[0].agreement, "corroborated");
    assert!(r.time_base.contains("unverified"));
    assert_eq!(r.clauses[0].answers.last().unwrap().answer, "undetermined");
}

#[test]
fn dispute_request_needs_evidence_and_known_templates() {
    let e = dispute::Request::parse(
        "title='x'\nwindow_start_utc='2026-06-01T08:00:00Z'\nwindow_end_utc='2026-06-01T09:00:00Z'\n",
    )
    .unwrap_err();
    assert!(e.contains("names no evidence"), "{e}");
    assert_eq!(dispute::TEMPLATES.len(), 5);
}

#[test]
fn features_are_registered_at_pro() {
    for f in [logtime::FEATURE, dispute::FEATURE] {
        assert_eq!(explicit_tier(f), Some(Tier::Pro), "{f}");
        assert!(
            kshana_pro::license::PRO_COMMAND_FEATURES.contains(&f),
            "{f}"
        );
    }
}
