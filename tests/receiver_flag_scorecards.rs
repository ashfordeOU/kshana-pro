// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Research backlog epic E13, receiver scorecards: flag efficacy (B1301), unit scorecards with
//! firmware regression (B1302), the discount-scheme device scorecard (B1303) and the timing
//! recovery metric (B1304). Every log here is a small synthetic fixture (MODELLED).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::exposure::scheme::{self, EvidenceCheck, Loaded};
use kshana_pro::pics::log::{Device, DeviceLog, Epoch};
use kshana_pro::scorecards::flags::{
    self, flag_normal_while_wrong, load_scenario, synth, wilson95, Bars, FlagLog, Indicator,
    LoadedRun, Meaning, Request, Unit, WrongInput,
};
use kshana_pro::scorecards::levels::Verdict;
use kshana_pro::scorecards::units;
use kshana_pro::timing::recovery::{self, synth as tsynth};

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e13-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
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

// ------------------------------------------------------------------------------------------
// B1301 flag efficacy
// ------------------------------------------------------------------------------------------

fn ind(model: &str, name: &str, meaning: Meaning, pre: &[&str]) -> Indicator {
    Indicator {
        model: model.into(),
        name: name.into(),
        meaning,
        raise_at: if name == "spoof" { Some(2.0) } else { None },
        raise_values: vec![],
        documented_in: Some("synthetic interface description".into()),
        preconditions: pre.iter().map(|s| s.to_string()).collect(),
    }
}

fn indicators(model: &str, spoof_pre: &[&str]) -> Vec<Indicator> {
    vec![
        ind(model, "spoof", Meaning::Spoofing, spoof_pre),
        ind(model, "jam", Meaning::Jamming, &[]),
        ind(model, "cw", Meaning::JammingCw, &[]),
        ind(model, "integrity", Meaning::Integrity, &[]),
    ]
}

const SCEN: [&str; 5] = [
    "flags-clean",
    "flags-jam-cw",
    "flags-jam-broadband",
    "flags-spoof-simplistic",
    "flags-spoof-drag",
];

fn campaign(models: &[(&str, synth::Behaviour)], runs_per: usize) -> Vec<LoadedRun> {
    let mut out = vec![];
    for (model, b) in models {
        let unit = Unit {
            model: model.to_string(),
            firmware: Some("1.0".into()),
            serial: None,
            class: Some("module".into()),
        };
        for sc in SCEN {
            let spec = load_scenario(&format!("receiver-flags/{sc}"), None).unwrap();
            for k in 0..runs_per {
                let constellations = if k % 2 == 1 { "single" } else { "multi" };
                let log = synth::log(
                    &spec,
                    &unit,
                    *b,
                    &[
                        ("constellations", constellations),
                        ("start", "genuine-first"),
                    ],
                    3.0,
                );
                let bytes = log.to_json().into_bytes();
                out.push(LoadedRun {
                    id: format!("{model}-{sc}-{k}"),
                    log,
                    spec: spec.clone(),
                    log_bytes: bytes,
                });
            }
        }
    }
    out
}

fn request(inds: Vec<Indicator>) -> Request {
    Request {
        format: flags::REQUEST_FORMAT.into(),
        title: "t".into(),
        bars: Bars {
            detect_within_s: 30.0,
            ..Bars::default()
        },
        indicators: inds,
        runs: vec![flags::RunSpec {
            id: None,
            log: "unused".into(),
            scenario: None,
            preconditions: BTreeMap::new(),
        }],
        base_dir: None,
    }
}

fn row<'a>(s: &'a flags::Scorecard, model: &str, ind: &str) -> &'a flags::IndicatorRow {
    s.table
        .iter()
        .find(|r| r.model == model && r.indicator == ind)
        .unwrap()
}

fn class<'a>(r: &'a flags::IndicatorRow, c: &str) -> &'a flags::ClassRow {
    r.classes.iter().find(|x| x.class == c).unwrap()
}

#[test]
fn wilson_interval_and_rounding() {
    let (lo, hi) = wilson95(3, 3).unwrap();
    assert!((lo - 0.438_503_0).abs() < 1e-6, "{lo}");
    assert_eq!(hi, 1.0);
    let (lo, hi) = wilson95(0, 4).unwrap();
    assert_eq!(lo, 0.0);
    assert!((hi - 0.489_890_8).abs() < 1e-6, "{hi}");
    assert!(wilson95(0, 0).is_none());
    assert_eq!(flags::sig9(1.234_567_890_12), 1.234_567_89);
    assert_eq!(flags::sig9(-0.000_123_456_789_99), -0.000_123_456_79);
}

#[test]
fn flag_normal_while_wrong_groups_episodes() {
    let e = |t: f64, valid: bool, normal: bool, h: f64| WrongInput {
        t_s: t,
        valid,
        all_normal: normal,
        h_error_m: Some(h),
        time_error_ns: None,
    };
    let eps = vec![
        e(0.0, true, true, 1.0),
        e(1.0, true, true, 80.0),
        e(2.0, true, true, 120.0),
        e(3.0, true, false, 200.0), // flagged: not silent
        e(4.0, true, true, 90.0),
        e(5.0, false, true, 300.0), // withdrawn: not silent
        e(9.0, true, true, 60.0),
    ];
    let w = flag_normal_while_wrong(&eps, 50.0, 1000.0, 1.5);
    assert_eq!(w.len(), 3, "{w:?}");
    assert_eq!((w[0].from_s, w[0].to_s, w[0].epochs), (1.0, 2.0, 2));
    assert_eq!(w[0].max_h_error_m, Some(120.0));
    assert_eq!((w[1].from_s, w[1].epochs), (4.0, 1));
    assert_eq!((w[2].from_s, w[2].epochs), (9.0, 1));
    // A time error alone is enough.
    let t = [WrongInput {
        t_s: 0.0,
        valid: true,
        all_normal: true,
        h_error_m: None,
        time_error_ns: Some(-2000.0),
    }];
    assert_eq!(
        flag_normal_while_wrong(&t, 50.0, 1000.0, 1.5)[0].max_time_error_ns,
        Some(2000.0)
    );
}

#[test]
fn good_receiver_detects_every_class_without_false_alarms() {
    let runs = campaign(&[("Model-G", synth::Behaviour::Good)], 4);
    let r = request(indicators("Model-G", &[]));
    let s = flags::score_loaded(&r, "src", &runs).unwrap();
    assert_eq!(s.format, flags::FORMAT);
    let spoof = row(&s, "Model-G", "spoof");
    for c in ["spoofing-simplistic", "spoofing-position_drag"] {
        let x = class(spoof, c);
        assert_eq!(x.expected, Some(true));
        assert_eq!((x.response.hits, x.response.runs), (4, 4), "{c}");
        assert_eq!(x.response.rate, Some(1.0));
        assert_eq!(x.median_latency_s, Some(3.0));
    }
    // The spoofing flag stays quiet under jamming; the CW indicator under broadband.
    assert_eq!(class(spoof, "jamming-cw").response.hits, 0);
    let cw = row(&s, "Model-G", "cw");
    assert_eq!(class(cw, "jamming-cw").response.hits, 4);
    assert_eq!(class(cw, "jamming-broadband").expected, Some(false));
    assert_eq!(class(cw, "jamming-broadband").response.hits, 0);
    assert_eq!(spoof.false_alarm_epochs, 0);
    assert!(spoof.baseline_epochs > 600 * 4);
    assert!(
        s.findings.is_empty(),
        "a well-behaved unit has no finding: {:?}",
        s.findings
    );
    assert!(s.runs.iter().all(|x| x.normal_while_wrong.is_empty()));
}

#[test]
fn spoof_flag_that_never_fires_is_reported_with_silent_wrong_output() {
    // The low-cost timing-module case: the spoofing flag never fired.
    let runs = campaign(&[("Model-S", synth::Behaviour::SilentSpoof)], 3);
    let s = flags::score_loaded(&request(indicators("Model-S", &[])), "src", &runs).unwrap();
    let spoof = row(&s, "Model-S", "spoof");
    let x = class(spoof, "spoofing-simplistic");
    assert_eq!((x.response.hits, x.response.runs), (0, 3));
    assert_eq!(x.response.rate, Some(0.0));
    let never: Vec<_> = s
        .findings
        .iter()
        .filter(|f| f.kind == "never-fired" && f.indicator.as_deref() == Some("spoof"))
        .collect();
    assert_eq!(never.len(), 2, "{:?}", s.findings);
    // The integrity indicator reads ok while the position is 500 m off.
    assert!(s.findings.iter().any(
        |f| f.kind == "normal-while-wrong" && f.text.contains("Model-S-flags-spoof-simplistic")
    ));
    let run = s
        .runs
        .iter()
        .find(|r| r.id == "Model-S-flags-spoof-simplistic-0")
        .unwrap();
    let ep = &run.normal_while_wrong[0];
    assert_eq!((ep.from_s, ep.to_s), (120.0, 360.0));
    assert!((ep.max_h_error_m.unwrap() - 500.0).abs() < 2.0, "{ep:?}");
}

#[test]
fn precondition_dependence_and_disclosure() {
    let runs = campaign(&[("Model-M", synth::Behaviour::MultiOnly)], 4);
    // Vendor says nothing about constellations: undisclosed.
    let s = flags::score_loaded(
        &request(indicators("Model-M", &["start=genuine-first"])),
        "src",
        &runs,
    )
    .unwrap();
    let x = class(row(&s, "Model-M", "spoof"), "spoofing-simplistic");
    let by = &x.by_preconditions;
    assert_eq!(by["constellations=multi,start=genuine-first"].hits, 2);
    assert_eq!(by["constellations=single,start=genuine-first"].hits, 0);
    let kinds: Vec<&str> = s.findings.iter().map(|f| f.kind.as_str()).collect();
    assert!(kinds.contains(&"precondition-dependent"), "{kinds:?}");
    assert!(kinds.contains(&"undisclosed-precondition"), "{kinds:?}");
    // Two runs per set is below min_runs (3): the per-set rate is not assessed.
    assert_eq!(by["constellations=single,start=genuine-first"].rate, None);
    // Vendor states multi-constellation: disclosed.
    let s2 = flags::score_loaded(
        &request(indicators(
            "Model-M",
            &["constellations=multi", "start=genuine-first"],
        )),
        "src",
        &runs,
    )
    .unwrap();
    assert!(s2
        .findings
        .iter()
        .any(|f| f.kind == "precondition-dependent"));
    let spoof_undisclosed = |s: &flags::Scorecard| {
        s.findings.iter().any(|f| {
            f.kind == "undisclosed-precondition" && f.indicator.as_deref() == Some("spoof")
        })
    };
    assert!(spoof_undisclosed(&s));
    assert!(!spoof_undisclosed(&s2));
    // The integrity indicator follows the spoof flag and its vendor states no preconditions.
    assert!(s2.findings.iter().any(
        |f| f.kind == "undisclosed-precondition" && f.indicator.as_deref() == Some("integrity")
    ));
}

#[test]
fn cw_indicator_firing_under_broadband_is_a_wrong_class_indication() {
    let runs = campaign(&[("Model-C", synth::Behaviour::CwConfused)], 3);
    let s = flags::score_loaded(&request(indicators("Model-C", &[])), "src", &runs).unwrap();
    let cw = row(&s, "Model-C", "cw");
    assert_eq!(class(cw, "jamming-broadband").response.hits, 3);
    assert!(s
        .findings
        .iter()
        .any(|f| f.kind == "wrong-class" && f.indicator.as_deref() == Some("cw")));
    let csv = flags::csv(&s);
    assert!(csv.contains("Model-C,1.0,cw,jamming_cw,jamming-broadband,quiet,3,3,1,"));
}

#[test]
fn requests_are_validated_and_bars_hashed() {
    let runs = campaign(&[("Model-G", synth::Behaviour::Good)], 1);
    // An undeclared model is refused: indicators come before scoring.
    let e = flags::score_loaded(&request(indicators("Other", &[])), "src", &runs).unwrap_err();
    assert!(e.contains("no indicator is declared"), "{e}");
    let mut r = request(indicators("Model-G", &["constellations multi"]));
    assert!(r.validate().unwrap_err().contains("key=value"));
    r = request(vec![]);
    assert!(r.validate().is_err());
    let a = flags::bars_sha256(&Bars::default());
    let b = flags::bars_sha256(&Bars {
        detect_within_s: 61.0,
        ..Bars::default()
    });
    assert_ne!(a, b);
    let src = format!(
        "format = \"{}\"\ntitle = \"x\"\n[bars]\ndetect_within_s = 0\n[[indicator]]\nmodel = \"m\"\nname = \"n\"\nmeaning = \"spoofing\"\n[[run]]\nlog = \"l.json\"\n",
        flags::REQUEST_FORMAT
    );
    assert!(Request::parse(&src)
        .unwrap_err()
        .contains("detect_within_s"));
}

#[test]
fn a_pics_device_log_reads_as_a_flag_log() {
    let d = DeviceLog {
        format: kshana_pro::pics::log::FORMAT.into(),
        device: Device {
            name: "RX".into(),
            firmware: Some("2".into()),
            serial: None,
        },
        scenario: "receiver-flags/flags-jam-cw".into(),
        truth: None,
        source: None,
        epochs: vec![Epoch {
            t_s: 0.0,
            jamming: Some(true),
            integrity: Some("caution".into()),
            ..Epoch::default()
        }],
    };
    let f = FlagLog::parse(&d.to_json()).unwrap();
    assert_eq!(f.unit.model, "RX");
    let i = ind("RX", "integrity", Meaning::Integrity, &[]);
    assert_eq!(i.raised(&f.epochs[0]), Some(true));
    let j = ind("RX", "jamming", Meaning::Jamming, &[]);
    assert_eq!(j.raised(&f.epochs[0]), Some(true));
    let s = ind("RX", "spoofing", Meaning::Spoofing, &[]);
    assert_eq!(s.raised(&f.epochs[0]), None);
}

// ------------------------------------------------------------------------------------------
// B1302 unit scorecards and firmware regression
// ------------------------------------------------------------------------------------------

/// A board under broadband jamming that withdraws its fix 2 s after onset and recovers `rec`
/// s after the end.
fn board_log(fw: &str, serial: &str, rec: f64) -> DeviceLog {
    let spec = load_scenario("receiver-flags/flags-jam-broadband", None).unwrap();
    let (on, off) = spec.threat_window(None).unwrap();
    let tr = spec.trajectory.at(0.0);
    DeviceLog {
        format: kshana_pro::pics::log::FORMAT.into(),
        device: Device {
            name: "Board-X".into(),
            firmware: Some(fw.into()),
            serial: Some(serial.into()),
        },
        scenario: spec.id.clone(),
        truth: None,
        source: Some("synthetic".into()),
        epochs: (0..=spec.scene.duration_s as usize)
            .map(|k| {
                let t = k as f64;
                let jammed = t >= on + 2.0 && t < off + rec;
                Epoch {
                    t_s: t,
                    fix_valid: Some(!jammed),
                    lat_deg: Some(tr.lat_deg),
                    lon_deg: Some(tr.lon_deg),
                    jamming: Some(jammed && t < off),
                    ..Epoch::default()
                }
            })
            .collect(),
    }
}

fn unit_request(
    units_: &[(&str, &str, f64)],
    compare: Option<(&str, &str)>,
) -> (units::Request, Vec<units::LoadedRun>) {
    let spec = load_scenario("receiver-flags/flags-jam-broadband", None).unwrap();
    let mut r = units::Request {
        format: units::REQUEST_FORMAT.into(),
        title: "boards".into(),
        bars: units::Bars::default(),
        units: vec![],
        runs: vec![],
        compares: compare
            .map(|(a, b)| {
                vec![units::Compare {
                    model: "Board-X".into(),
                    from: a.into(),
                    to: b.into(),
                }]
            })
            .unwrap_or_default(),
        base_dir: None,
    };
    let mut runs = vec![];
    for (id, fw, rec) in units_ {
        r.units.push(units::UnitSpec {
            id: id.to_string(),
            model: "Board-X".into(),
            class: Some("board".into()),
            firmware: fw.to_string(),
            serial: Some(id.to_string()),
            provenance: Some("bought independently".into()),
        });
        r.runs.push(units::RunSpec {
            unit: id.to_string(),
            scenario: "receiver-flags/flags-jam-broadband".into(),
            log: format!("{id}.json"),
            t0_s: 0.0,
        });
        runs.push(units::LoadedRun {
            unit: id.to_string(),
            spec: spec.clone(),
            log: board_log(fw, id, *rec),
        });
    }
    (r, runs)
}

#[test]
fn same_model_two_firmware_regression_is_detected() {
    let (r, runs) = unit_request(
        &[
            ("a1", "1.0", 10.0),
            ("a2", "1.0", 11.0),
            ("a3", "1.0", 12.0),
            ("b1", "1.1", 58.0),
            ("b2", "1.1", 60.0),
            ("b3", "1.1", 62.0),
        ],
        Some(("1.0", "1.1")),
    );
    let s = units::score_loaded(&r, "src", &runs).unwrap();
    assert_eq!(s.format, units::FORMAT);
    assert_eq!(s.models.len(), 2);
    let m10 = s.models.iter().find(|m| m.firmware == "1.0").unwrap();
    let rec = m10
        .criteria
        .iter()
        .find(|c| c.criterion == "recover")
        .unwrap();
    // Recovery = first fully valid dwell after the end: rec + 0 s (dwell starts at off + rec).
    assert_eq!(
        (rec.median, rec.min, rec.max, rec.spread),
        (Some(11.0), Some(10.0), Some(12.0), Some(2.0))
    );
    assert_eq!(rec.units_met, 3);
    let g = &s.regressions[0];
    let c = g.changes.iter().find(|c| c.criterion == "recover").unwrap();
    assert_eq!(c.worse_by, Some(49.0));
    // max(floor 2, 0.2 x 11, 1 x spread 4) = 4
    assert_eq!(c.tolerance, Some(4.0));
    assert_eq!(c.change, "regression");
    assert_eq!(c.before_met, "3/3");
    assert_eq!(c.after_met, "0/3");
    assert_eq!(g.regressions, 1, "{:?}", g.changes);
    assert!(
        g.alerts[0].contains("Board-X firmware 1.1 vs 1.0: 'recover' (recovery_s)"),
        "{:?}",
        g.alerts
    );
    assert!(units::alerts_markdown(&s).contains("1 regression(s)"));
    // Detection latency is the same on both: within tolerance.
    let d = g.changes.iter().find(|c| c.criterion == "detect").unwrap();
    assert_eq!(d.change, "within-tolerance");
}

#[test]
fn unit_to_unit_spread_is_not_called_a_regression_and_variability_is_reported() {
    // Firmware 1.1's median is worse by 6 s, inside a unit spread of 30 s.
    let (r, runs) = unit_request(
        &[
            ("a1", "1.0", 5.0),
            ("a2", "1.0", 20.0),
            ("a3", "1.0", 35.0),
            ("b1", "1.1", 11.0),
            ("b2", "1.1", 26.0),
            ("b3", "1.1", 29.0),
        ],
        Some(("1.0", "1.1")),
    );
    let s = units::score_loaded(&r, "src", &runs).unwrap();
    let c = s.regressions[0]
        .changes
        .iter()
        .find(|c| c.criterion == "recover")
        .unwrap();
    assert_eq!(c.worse_by, Some(6.0));
    assert_eq!(c.tolerance, Some(30.0));
    // 1.0: a3 (35 s) missed the 30 s bar; 1.1: all met. Not a regression.
    assert_eq!(c.change, "within-tolerance");
    assert!(s.variability.iter().any(|v| v.firmware == "1.0"
        && v.criterion == "recover"
        && v.text.starts_with("2 of 3 units")));
    // A compare naming a firmware no unit runs is refused.
    let (mut r2, _) = unit_request(&[("a1", "1.0", 5.0)], None);
    r2.compares.push(units::Compare {
        model: "Board-X".into(),
        from: "1.0".into(),
        to: "9.9".into(),
    });
    assert!(r2.validate().unwrap_err().contains("9.9"));
}

// ------------------------------------------------------------------------------------------
// B1304 timing recovery
// ------------------------------------------------------------------------------------------

fn rec_request(ids: &[&str], ensemble: bool) -> recovery::Request {
    recovery::Request {
        format: recovery::REQUEST_FORMAT.into(),
        title: "t".into(),
        attack_scenario: None,
        attack: recovery::Attack {
            start_s: 600.0,
            end_s: 900.0,
        },
        bars: recovery::Bars::default(),
        receivers: ids
            .iter()
            .map(|i| recovery::ReceiverSpec {
                id: i.to_string(),
                model: Some("Module-T".into()),
                firmware: None,
                log: recovery::LogRef {
                    format: "csv".into(),
                    path: format!("{i}.csv"),
                    channel: None,
                    clock: None,
                },
                t0_s: 0.0,
            })
            .collect(),
        ensemble_median: ensemble,
        base_dir: None,
    }
}

fn fixtures() -> Vec<(Vec<recovery::Sample>, Vec<u8>)> {
    [
        tsynth::push_and_slew(5.0, 600.0, 900.0, 10.0, 100.0, 2400.0, None),
        tsynth::push_and_slew(-5.0, 600.0, 900.0, 10.0, 5.0, 2400.0, None),
        tsynth::push_and_slew(0.0, 600.0, 900.0, 10.0, 1.0, 2400.0, Some((960.0, 250.0))),
    ]
    .into_iter()
    .map(|s| {
        let csv = tsynth::csv(&s);
        // Round trip through the CSV reader.
        (recovery::read_csv(&csv, 0.0).unwrap(), csv.into_bytes())
    })
    .collect()
}

#[test]
fn recovery_metric_on_three_fixtures_matches_closed_forms() {
    let r = rec_request(&["fast", "slow", "holdover"], true);
    let rep = recovery::score_logs(&r, "src", &fixtures()).unwrap();
    assert_eq!(rep.format, recovery::FORMAT);
    let g = |id: &str| rep.receivers.iter().find(|x| x.id == id).unwrap();
    // Fast: 3000 ns peak, slews back at 100 ns/s; within 100 ns from 900 + 29 s.
    let f = g("fast");
    assert_eq!(f.baseline_ns, Some(5.0));
    assert_eq!(f.attack_max_dev_ns, Some(3000.0));
    assert_eq!(f.recovery, "recovered");
    assert_eq!(f.recovery_s, Some(29.0));
    assert_eq!(f.residual_ns, Some(0.0));
    assert_eq!(f.max_step_after_ns, Some(100.0));
    assert!(
        f.criteria.iter().all(|c| c.verdict == Verdict::Met),
        "{:?}",
        f.criteria
    );
    // Slow: 5 ns/s; within 100 ns after 580 s (bar 600 s: met).
    let s = g("slow");
    assert_eq!(s.recovery_s, Some(580.0));
    assert_eq!(s.criteria[0].verdict, Verdict::Met);
    // Holdover: no deviation during the attack, re-lock at 960 s with a 250 ns step.
    let h = g("holdover");
    assert_eq!(h.attack_max_dev_ns, Some(0.0));
    assert_eq!(h.holdover_entry_latency_s, Some(0.0));
    assert_eq!(h.holdover_exit_s, Some(960.0));
    assert_eq!(h.holdover_exit_step_ns, Some(250.0));
    assert_eq!(h.recovery_s, Some(61.0));
    let step = h
        .criteria
        .iter()
        .find(|c| c.id == "holdover-exit-step")
        .unwrap();
    assert_eq!(step.verdict, Verdict::NotMet);
    // Median of three: recovers when the fast and holdover modules agree again.
    let e = g(recovery::ENSEMBLE);
    assert_eq!(e.recovery_s, Some(61.0));
    assert_eq!(e.attack_max_dev_ns, Some(2995.0));
    assert_eq!(rep.ranking[0], "fast");
    assert_eq!(rep.ranking.last().unwrap(), "slow");
    assert!(
        rep.hardening[1].contains("recovered faster than the slowest"),
        "{:?}",
        rep.hardening
    );
}

#[test]
fn recovery_not_observed_when_the_log_ends_early_and_not_recovered_when_it_stays_off() {
    let r = rec_request(&["x"], false);
    // Ends 30 s after the attack: too early to tell.
    let short = tsynth::push_and_slew(0.0, 600.0, 900.0, 10.0, 1.0, 930.0, None);
    let rep = recovery::score_logs(&r, "s", &[(short, vec![])]).unwrap();
    assert_eq!(rep.receivers[0].recovery, "not observed");
    assert_eq!(rep.receivers[0].criteria[0].verdict, Verdict::NotAssessed);
    // Never slews back (slew 0): not recovered, residual 3000 ns.
    let stuck = tsynth::push_and_slew(0.0, 600.0, 900.0, 10.0, 0.0, 2400.0, None);
    let rep = recovery::score_logs(&r, "s", &[(stuck, vec![])]).unwrap();
    let x = &rep.receivers[0];
    assert_eq!(x.recovery, "not recovered");
    assert_eq!(x.residual_ns, Some(3000.0));
    assert_eq!(x.criteria[0].verdict, Verdict::NotMet);
    assert!(rec_request(&["a", "b"], true).validate().is_err());
    assert!(recovery::read_csv("t,te\n1,2\n", 0.0).is_err());
}

// ------------------------------------------------------------------------------------------
// B1303 discount-scheme device scorecard
// ------------------------------------------------------------------------------------------

fn scheme_request(model: &str, paid_by: &str) -> scheme::Request {
    let src = format!(
        r#"format = "kshana-pro-scheme-device-request/1"
title = "Anti-spoof kit under a discount scheme"
scheme = "Example mutual's resilience discount (as published)"
[device]
maker = "Maker"
model = "{model}"
firmware = "1.0"
[funding]
paid_by = "{paid_by}"
[[evidence]]
id = "flags"
kind = "flag-efficacy"
path = "flags"
[[evidence]]
id = "timing"
kind = "timing-recovery"
path = "timing"
[[claim]]
id = "spoof"
text = "Detects GNSS spoofing"
kind = "spoof_detection"
min_rate = 0.9
evidence = ["flags"]
[[claim]]
id = "jam"
text = "Detects jamming"
kind = "jam_detection"
evidence = ["flags"]
[[claim]]
id = "nomis"
text = "Never outputs a wrong position without warning"
kind = "no_misleading_output"
[[claim]]
id = "apnt"
text = "The backup keeps time within 1 microsecond while GNSS is attacked"
kind = "apnt_holdover"
value = 1000
receiver = "holdover"
evidence = ["timing"]
"#
    );
    scheme::Request::parse(&src).unwrap()
}

fn evidence(
    status: &str,
    flags_card: flags::Scorecard,
    timing: recovery::Report,
) -> Vec<(EvidenceCheck, Option<Loaded>)> {
    let ck = |id: &str, kind: &str| EvidenceCheck {
        id: id.into(),
        kind: kind.into(),
        status: status.into(),
        source: None,
    };
    vec![
        (
            ck("flags", "flag-efficacy"),
            Some(Loaded::Flags(Box::new(flags_card))),
        ),
        (
            ck("timing", "timing-recovery"),
            Some(Loaded::Timing(Box::new(timing))),
        ),
    ]
}

fn flags_card(model: &str, b: synth::Behaviour) -> flags::Scorecard {
    let runs = campaign(&[(model, b)], 3);
    flags::score_loaded(&request(indicators(model, &[])), "src", &runs).unwrap()
}

#[test]
fn scheme_claims_are_checked_against_the_evidence() {
    let timing = recovery::score_logs(
        &rec_request(&["fast", "slow", "holdover"], false),
        "s",
        &fixtures(),
    )
    .unwrap();
    // A unit that detects: claims supported; independent funding and signed: publishable.
    let r = scheme_request("Model-G", "independent");
    let s = scheme::check_loaded(
        &r,
        "src",
        &evidence(
            "signed",
            flags_card("Model-G", synth::Behaviour::Good),
            timing.clone(),
        ),
    );
    let v = |s: &scheme::Scorecard, id: &str| {
        s.claims
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .verdict
            .clone()
    };
    assert_eq!(v(&s, "spoof"), "supported", "{:?}", s.claims);
    let sp = s.claims.iter().find(|c| c.id == "spoof").unwrap();
    assert_eq!(sp.measured, Some(1.0));
    assert!(sp.ci95.unwrap().0 > 0.6);
    assert_eq!(v(&s, "jam"), "supported");
    assert_eq!(v(&s, "nomis"), "supported");
    assert_eq!(v(&s, "apnt"), "supported");
    assert!(s.publishable, "{}", s.publication_note);
    // The silent unit: detection and no-misleading claims not supported.
    let r = scheme_request("Model-S", "independent");
    let s = scheme::check_loaded(
        &r,
        "src",
        &evidence(
            "signed",
            flags_card("Model-S", synth::Behaviour::SilentSpoof),
            timing.clone(),
        ),
    );
    assert_eq!(v(&s, "spoof"), "not supported");
    assert_eq!(v(&s, "nomis"), "not supported");
    assert_eq!(s.not_supported, 2);
    // Paid for by the maker: not publishable as a neutral scorecard.
    let r = scheme_request("Model-G", "subject");
    let s = scheme::check_loaded(
        &r,
        "src",
        &evidence(
            "signed",
            flags_card("Model-G", synth::Behaviour::Good),
            timing.clone(),
        ),
    );
    assert!(!s.publishable);
    assert!(s.publication_note.contains("maker"));
    // Unsigned evidence is never enough for supported.
    let r = scheme_request("Model-G", "independent");
    let s = scheme::check_loaded(
        &r,
        "src",
        &evidence(
            "unsigned",
            flags_card("Model-G", synth::Behaviour::Good),
            timing,
        ),
    );
    assert_eq!(v(&s, "spoof"), "insufficient evidence");
    assert!(!s.publishable);
    // Bad requests.
    let bad = scheme_request("M", "independent");
    let mut b2 = bad.clone();
    b2.funding.paid_by = "someone".into();
    assert!(b2.validate().is_err());
}

// ------------------------------------------------------------------------------------------
// The commands, end to end: flags, units, recovery and scheme with signed evidence.
// ------------------------------------------------------------------------------------------

#[test]
fn cli_end_to_end_with_signed_evidence() {
    let d = tmp("cli");
    let home = d.join("home");
    let key = d.join("key.txt");
    std::fs::write(&key, format!("private_key={}\n", hex_of(&[7u8; 32]))).unwrap();
    let pk = hex_of(&kshana_pro::license::public_key_for(&[7u8; 32]));
    let s = |p: &Path| p.to_str().unwrap().to_string();
    let produce = |cmd: &[&str], out: &Path| {
        let mut a: Vec<String> = cmd.iter().map(|x| x.to_string()).collect();
        a.extend(
            [
                "--out",
                &s(out),
                "--signer",
                "Lab",
                "--key-file",
                &s(&key),
                "--utc",
                "2026-11-20T09:00:00Z",
                "--dev",
            ]
            .iter()
            .map(|x| x.to_string()),
        );
        let refs: Vec<&str> = a.iter().map(String::as_str).collect();
        bin(&refs, &home)
    };
    // Flags.
    let fd = d.join("flags-in");
    let (ok, out) = bin(&["scorecard", "flags-synth", &s(&fd)], &home);
    assert!(ok, "{out}");
    let fo = d.join("flags");
    let (ok, out) = produce(&["scorecard", "flags", &s(&fd.join("request.toml"))], &fo);
    assert!(ok, "{out}");
    let card: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fo.join(flags::RESULT_FILE)).unwrap())
            .unwrap();
    assert_eq!(card["format"], flags::FORMAT);
    assert!(card["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["kind"] == "never-fired" && f["model"] == "Model-S"));
    let (ok, out) = bin(&["dossier", "verify", &s(&fo), "--trust", &pk], &home);
    assert!(ok, "{out}");
    // Without a licence, refused.
    let (ok, _) = bin(
        &[
            "scorecard",
            "flags",
            &s(&fd.join("request.toml")),
            "--out",
            &s(&d.join("x")),
            "--signer",
            "Lab",
            "--key-file",
            &s(&key),
        ],
        &home,
    );
    assert!(!ok);
    // Units.
    let ud = d.join("units-in");
    let (ok, out) = bin(&["scorecard", "units-synth", &s(&ud)], &home);
    assert!(ok, "{out}");
    let uo = d.join("units");
    let (ok, out) = produce(&["scorecard", "units", &s(&ud.join("request.toml"))], &uo);
    assert!(ok && out.contains("1 firmware regression(s)"), "{out}");
    assert!(std::fs::read_to_string(uo.join("regression-alerts.md"))
        .unwrap()
        .contains("Board-X firmware 1.1 vs 1.0"));
    // Recovery.
    let rd = d.join("rec-in");
    let (ok, out) = bin(&["timing", "recovery-synth", &s(&rd)], &home);
    assert!(ok, "{out}");
    let ro = d.join("timing");
    let (ok, out) = produce(&["timing", "recovery", &s(&rd.join("request.toml"))], &ro);
    assert!(ok && out.contains("4 configuration(s)"), "{out}");
    // Scheme, over the signed flag and timing directories.
    let req = format!(
        "format = \"{}\"\ntitle = \"t\"\nscheme = \"Example scheme\"\n[device]\nmaker = \"m\"\nmodel = \"Model-G\"\n[funding]\npaid_by = \"insurer\"\n\
[[evidence]]\nid = \"flags\"\nkind = \"flag-efficacy\"\npath = \"flags\"\n[[evidence]]\nid = \"timing\"\nkind = \"timing-recovery\"\npath = \"timing\"\n\
[[claim]]\nid = \"spoof\"\ntext = \"Detects spoofing\"\nkind = \"spoof_detection\"\n[[claim]]\nid = \"apnt\"\ntext = \"Backup holds time\"\nkind = \"apnt_holdover\"\nvalue = 1000\nreceiver = \"holdover\"\n",
        scheme::REQUEST_FORMAT
    );
    std::fs::write(d.join("scheme.toml"), req).unwrap();
    let so = d.join("scheme-out");
    let mut a = vec!["exposure", "scheme"];
    let sp = s(&d.join("scheme.toml"));
    a.push(&sp);
    a.extend(["--trust", &pk]);
    let (ok, out) = produce(&a, &so);
    assert!(ok, "{out}");
    assert!(
        out.contains("2 supported") && out.contains("publishable: yes"),
        "{out}"
    );
    let sc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(so.join(scheme::RESULT_FILE)).unwrap())
            .unwrap();
    assert_eq!(sc["evidence"][0]["status"], "signed");
    // Tamper with the flag scorecard: its evidence is set aside.
    let p = fo.join(flags::RESULT_FILE);
    let t = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, t.replacen("\"title\"", "\"title\" ", 1)).unwrap();
    let so2 = d.join("scheme-out2");
    let (ok, out) = produce(&a, &so2);
    assert!(ok, "{out}");
    assert!(out.contains("publishable: no"), "{out}");
}
