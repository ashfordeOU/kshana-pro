// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Power-grid and broadcast SFN timing evidence (backlog E31: B3101, B3102) on SYNTHETIC
//! records built here: C37.118.2 captures (CFG-2 and data frames with their CRC), PMU and
//! recording-device clock logs with injected time error (a slow drag, a step, a 60 ms
//! offset), SCADA and sequence-of-events logs, and DAB transmitter logs reproducing the
//! SVN-23 13 µs UTC error with mixed receiver reactions.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::timing::grid::{self, DriftParams, EventRequest, PmuRequest, Prc002Request};
use kshana_pro::timing::sfn::{self, SfnRequest};

/// 2026-11-19T12:00:00Z.
const T0: f64 = 1_795_089_600.0;
const UTC: &str = "2026-11-19T12:00:00Z";

fn tmp(name: &str) -> PathBuf {
    let d =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("e31-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Deterministic pseudo-noise in [-1, 1).
fn noise(i: u64) -> f64 {
    let x = i
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

/// A clock log `unix_time,offset_s` at 1 s for `n` s from `start`.
fn clock_csv(start: f64, n: usize, seed: u64, te: impl Fn(f64) -> f64) -> String {
    let mut s = String::from("# synthetic clock log: unix_time,offset_s\n");
    for i in 0..n {
        let t = start + i as f64;
        let e = 30e-9 * noise(seed * 1_000_003 + i as u64) + te(t - start);
        s.push_str(&format!("{t},{e:e}\n"));
    }
    s
}

// ------------------------------------------------------------- C37.118.2 frames

fn crc(data: &[u8]) -> u16 {
    kshana::c37118::crc_ccitt(data)
}

fn frame(ftype: u8, id: u16, soc: u32, fracsec: u32, body: &[u8]) -> Vec<u8> {
    let size = 14 + body.len() + 2;
    let mut f = vec![0xAA, (ftype << 4) | 1];
    f.extend((size as u16).to_be_bytes());
    f.extend(id.to_be_bytes());
    f.extend(soc.to_be_bytes());
    f.extend(fracsec.to_be_bytes());
    f.extend(body);
    let c = crc(&f);
    f.extend(c.to_be_bytes());
    f
}

/// CFG-2 with PMUs of one integer phasor each, integer frequency.
fn cfg2(ids: &[u16], soc: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(1_000_000u32.to_be_bytes());
    b.extend((ids.len() as u16).to_be_bytes());
    for id in ids {
        let mut stn = format!("SUB-{id}").into_bytes();
        stn.resize(16, b' ');
        b.extend(stn);
        b.extend(id.to_be_bytes());
        b.extend(0u16.to_be_bytes()); // FORMAT: integers
        b.extend(1u16.to_be_bytes()); // PHNMR
        b.extend(0u16.to_be_bytes()); // ANNMR
        b.extend(0u16.to_be_bytes()); // DGNMR
        let mut name = b"VA".to_vec();
        name.resize(16, b' ');
        b.extend(name);
        b.extend(0u32.to_be_bytes()); // PHUNIT
        b.extend(1u16.to_be_bytes()); // FNOM (50 Hz flag)
        b.extend(0u16.to_be_bytes()); // CFGCNT
    }
    b.extend(30u16.to_be_bytes()); // DATA_RATE
    frame(3, 1, soc, 0, &b)
}

/// One data frame: per PMU its STAT word, then one phasor (4 bytes) and FREQ/DFREQ (4 bytes).
fn data(soc: u32, stats: &[u16]) -> Vec<u8> {
    let mut b = Vec::new();
    for s in stats {
        b.extend(s.to_be_bytes());
        b.extend([0u8; 8]);
    }
    frame(0, 1, soc, 0, &b)
}

/// STAT with a sync error flag and a PMU time quality code.
fn stat(sync_error: bool, tq: u16) -> u16 {
    (if sync_error { 0x2000 } else { 0 }) | (tq << 6)
}

/// The PMU fixture: PMU 1 healthy, PMU 2 dragged at 20 ns/s from +600 s for 3,000 s while
/// its flags keep saying "in sync, better than 1 µs"; PMU 3 has a 200 s sync loss and no log.
fn pmu_fixture(d: &Path) -> PmuRequest {
    let n = 3600;
    std::fs::write(d.join("pmu1.csv"), clock_csv(T0, n, 1, |_| 0.0)).unwrap();
    let ramp = |t: f64| if t > 600.0 { 20e-9 * (t - 600.0) } else { 0.0 };
    std::fs::write(d.join("pmu2.csv"), clock_csv(T0, n, 2, ramp)).unwrap();
    let mut cap = cfg2(&[1, 2, 3], T0 as u32);
    for i in 0..n as u32 {
        let lost = (1000..1200).contains(&i);
        cap.extend(data(
            T0 as u32 + i,
            &[
                stat(false, 2),
                stat(false, 2),
                stat(lost, if lost { 5 } else { 2 }),
            ],
        ));
    }
    std::fs::write(d.join("capture.c37"), cap).unwrap();
    toml::from_str(&format!(
        r#"
title = "synthetic substation"
nominal_hz = 60.0
[[pmu]]
name = "PMU-1"
id = 1
capture = "capture.c37"
clock = {{ format = "csv", path = "pmu1.csv" }}
[[pmu]]
name = "PMU-2"
id = 2
capture = "capture.c37"
clock = {{ format = "csv", path = "pmu2.csv" }}
[[pmu]]
name = "PMU-3"
id = 3
capture = "capture.c37"
[[event]]
name = "late window"
start_unix = {}
end_unix = {}
"#,
        T0 + 3000.0,
        T0 + 3060.0
    ))
    .unwrap()
}

#[test]
fn pmu_phase_error_and_silent_flags() {
    let d = tmp("pmu");
    let req = pmu_fixture(&d);
    let r = grid::assess_pmus(&req, &d).unwrap();
    assert!((r.timing_budget_s - 26.526e-6).abs() < 0.01e-6);
    let p1 = &r.pmus[0];
    assert_eq!(p1.status, "within-budget");
    assert!(p1.max_abs_te_s.unwrap() < 31e-9);
    assert_eq!(p1.capture.as_ref().unwrap().reports, 3600);
    assert_eq!(p1.quality_claim_exceeded_readings, 0);
    let p2 = &r.pmus[1];
    assert_eq!(p2.status, "beyond-budget-flags-silent", "{}", r.verdict);
    // 20 ns/s × 3000 s = 60 µs at the end: 1.296° at 60 Hz.
    assert!((p2.max_abs_te_s.unwrap() - 60e-6).abs() < 0.1e-6);
    assert!((p2.max_abs_phase_error_deg.unwrap() - 1.296).abs() < 0.003);
    assert!(p2.max_tve_from_timing.unwrap() > 0.02);
    assert_eq!(p2.beyond_budget.len(), 1);
    let start = p2.beyond_budget[0].start_unix - T0;
    assert!((start - (600.0 + 26.526e-6 / 20e-9)).abs() < 3.0, "{start}");
    assert!(p2.flags_silent_readings > 1500);
    // Claimed < 1 µs from 650 s on.
    assert!(p2.quality_claim_exceeded_readings > 2900);
    let p3 = &r.pmus[2];
    assert_eq!(p3.status, "flags-only");
    let c3 = p3.capture.as_ref().unwrap();
    assert_eq!(c3.sync_error_reports, 200);
    assert_eq!(c3.sync_losses.len(), 1);
    assert_eq!(c3.worst_claimed_quality_s, Some(1e-3));
    // At the event (median at +3030 s), PMU-2 is 48.6 µs off: 1.050° of angle-difference
    // error against PMU-1.
    let e = &r.events[0];
    assert_eq!(e.pairs.len(), 1);
    let pe = &e.pairs[0];
    assert_eq!((pe.a.as_str(), pe.b.as_str()), ("PMU-1", "PMU-2"));
    assert!((pe.angle_difference_error_deg + 360.0 * 60.0 * 48.6e-6).abs() < 0.002);
    assert!(r.verdict.contains("PMU-2") && r.verdict.contains("silent"));
    // A capture with several PMUs needs the id.
    let mut bad = req.clone();
    bad.pmu[0].id = None;
    assert!(grid::assess_pmus(&bad, &d)
        .unwrap_err()
        .contains("give the PMU's id"));
}

#[test]
fn gradual_spoof_detected_before_budget_and_clean_series_quiet() {
    let d = tmp("drift");
    pmu_fixture(&d);
    let req: grid::DriftRequest = toml::from_str(
        r#"
name = "PMU-2 clock"
nominal_hz = 60.0
clock = { format = "csv", path = "pmu2.csv" }
"#,
    )
    .unwrap();
    let r = grid::run_drift(&req, &d).unwrap();
    assert_eq!(r.kind, "gradual-drift", "{}", r.verdict);
    let a = r.first_alarm_unix.unwrap() - T0;
    assert!(a > 600.0 && a < 700.0, "alarm {a}");
    let onset = r.estimated_onset_unix.unwrap() - T0;
    assert!((onset - 600.0).abs() < 60.0, "onset {onset}");
    // The budget is used up at ~1926 s: warning of more than 20 minutes.
    assert!(r.warning_s.unwrap() > 1200.0, "{}", r.verdict);
    assert!(r.te_at_alarm_s.unwrap() < 2e-6);
    let rr = r.rate_after_alarm.unwrap();
    assert!(rr > 5e-9 && rr < 40e-9, "{rr}");
    assert!(grid::drift_csv(&r).lines().count() == 3601);

    let clean = grid::DriftRequest {
        clock: Some(grid::ClockLogSpec {
            format: "csv".into(),
            path: "pmu1.csv".into(),
            ..Default::default()
        }),
        ..req.clone()
    };
    let r = grid::run_drift(&clean, &d).unwrap();
    assert_eq!(r.kind, "none", "{}", r.verdict);

    // Two PMUs on one bus, PMU b on an independent time source: the same drag read from the
    // angle difference (with a 12.5° standing difference and wrap-around at ±180°).
    let mut s = String::new();
    for i in 0..3600u64 {
        let t = T0 + i as f64;
        let te = if i > 600 {
            20e-9 * (i as f64 - 600.0)
        } else {
            0.0
        };
        let b = 175.0 + 0.0005 * noise(i);
        let a = b + 12.5 + 360.0 * 60.0 * te;
        let a = if a > 180.0 { a - 360.0 } else { a };
        s.push_str(&format!("{t},{a},{b}\n"));
    }
    std::fs::write(d.join("angles.csv"), s).unwrap();
    let ang = grid::DriftRequest {
        clock: None,
        angle_pair: Some(grid::AnglePairSpec {
            path: "angles.csv".into(),
        }),
        detector: DriftParams {
            noise_s: 50e-9,
            ..Default::default()
        },
        ..req.clone()
    };
    let r = grid::run_drift(&ang, &d).unwrap();
    assert_eq!(r.input, "angle-pair");
    assert_eq!(r.kind, "gradual-drift", "{}", r.verdict);
    assert!((r.first_alarm_unix.unwrap() - T0 - 600.0) < 100.0);
    // The standing 12.5° is removed: the drag alone remains, detected well before the budget.
    assert!(
        (r.max_abs_te_s - 60e-6).abs() < 0.1e-6,
        "{}",
        r.max_abs_te_s
    );
    assert!(r.warning_s.unwrap() > 1200.0, "{}", r.verdict);
    let no_hz = grid::DriftRequest {
        nominal_hz: None,
        ..ang
    };
    assert!(grid::run_drift(&no_hz, &d).is_err());
}

fn key(d: &Path) -> (PathBuf, String) {
    let seed = [11u8; 32];
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let k = d.join("key.txt");
    std::fs::write(&k, format!("private_key={}\n", hex(&seed))).unwrap();
    (k, hex(&kshana_pro::license::public_key_for(&seed)))
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

fn prc002_fixture(d: &Path) -> String {
    // 6 h at 1 s. DFR-1 within; SER-1 3.1 ms off for 10 min; DDR-1 with a 20 min gap.
    let n = 6 * 3600;
    std::fs::write(d.join("dfr1.csv"), clock_csv(T0, n, 3, |_| 40e-6)).unwrap();
    std::fs::write(
        d.join("ser1.csv"),
        clock_csv(T0, n, 4, |t| {
            if (7200.0..7800.0).contains(&t) {
                3.1e-3
            } else {
                0.0
            }
        }),
    )
    .unwrap();
    let ddr: String = clock_csv(T0, n, 5, |_| 0.0)
        .lines()
        .enumerate()
        .filter(|(i, _)| !(10_000..11_200).contains(i))
        .map(|(_, l)| format!("{l}\n"))
        .collect();
    std::fs::write(d.join("ddr1.csv"), ddr).unwrap();
    let req = r#"
site = "Substation North (synthetic)"
gap_s = 120
[[device]]
name = "DFR-1"
kind = "dfr"
clock = { format = "csv", path = "dfr1.csv" }
[device.resilience]
period_h = 24
oscillator = { class = "ocxo" }
u_initial_te_s = 1e-6
[[device]]
name = "SER-1"
kind = "ser"
clock = { format = "csv", path = "ser1.csv" }
[device.resilience]
period_h = 24
oscillator = { class = "tcxo", initial_freq = 1e-7, u_initial_freq = 1e-7 }
[[device]]
name = "DDR-1"
kind = "ddr"
clock = { format = "csv", path = "ddr1.csv" }
"#;
    std::fs::write(d.join("prc002.toml"), req).unwrap();
    req.to_string()
}

#[test]
fn prc002_evidence_with_gaps_and_resilience() {
    let d = tmp("prc002");
    let src = prc002_fixture(&d);
    let req: Prc002Request = toml::from_str(&src).unwrap();
    let r = grid::assess_prc002(&req, &d, UTC).unwrap();
    assert_eq!(r.limit_s, 2e-3);
    let s: Vec<&str> = r.devices.iter().map(|x| x.status.as_str()).collect();
    assert_eq!(s, ["within", "beyond", "within-with-gaps"]);
    assert!((r.devices[0].coverage - 1.0).abs() < 1e-9);
    let ser = &r.devices[1];
    assert_eq!(ser.beyond.len(), 1);
    assert_eq!(ser.beyond[0].readings, 600);
    assert!((ser.share_within - (1.0 - 600.0 / 21_600.0)).abs() < 1e-9);
    let ddr = &r.devices[2];
    assert_eq!(ddr.gaps.len(), 1);
    assert!((ddr.gaps[0].end_unix - ddr.gaps[0].start_unix - 1201.0).abs() < 1e-6);
    // An OCXO holds ±2 ms for a day; a TCXO 1e-7 off does not (8.6 ms after 24 h).
    assert!(r.devices[0].resilience.as_ref().unwrap().supported);
    assert!(!r.devices[1].resilience.as_ref().unwrap().supported);
    assert!(r.devices[2].resilience.is_none());
    assert_eq!(r.counts["resilience-not-supported"], 1);
    assert!(r.verdict.starts_with("1 of 3 device(s)"), "{}", r.verdict);
    assert!(r.verdict.contains("SER-1") && r.verdict.contains("DDR-1"));
    let json = grid::rounded_json(&r);
    // The wording rule: no claim words in a report (spelled in parts here).
    for w in [
        concat!("compl", "iant"),
        concat!("con", "form"),
        concat!("cert", "ified"),
    ] {
        assert!(!json.to_lowercase().contains(w), "{w}");
    }
    assert!(json.contains("not a regulatory determination"));

    // Signed through the command line; verified offline; a change breaks it.
    let home = d.join("home");
    let (k, pk) = key(&d);
    let out = d.join("pack");
    let t = d.join("prc002.toml");
    let args = [
        "grid",
        "prc002",
        t.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--signer",
        "Ops",
        "--key-file",
        k.to_str().unwrap(),
        "--utc",
        UTC,
    ];
    let (ok, o) = run(&home, &args);
    assert!(!ok && o.contains("requires"), "needs a licence: {o}");
    let mut dev = args.to_vec();
    dev.push("--dev");
    let (ok, o) = run(&home, &dev);
    assert!(ok, "{o}");
    for f in [
        "report.json",
        "report.html",
        "inputs/prc002.toml",
        "signature.json",
    ] {
        assert!(out.join(f).exists(), "{f}");
    }
    let (ok, o) = run(
        &home,
        &["dossier", "verify", out.to_str().unwrap(), "--trust", &pk],
    );
    assert!(ok, "{o}");
    std::fs::write(out.join("report.html"), "changed").unwrap();
    let (ok, _) = run(
        &home,
        &["dossier", "verify", out.to_str().unwrap(), "--trust", &pk],
    );
    assert!(!ok);
    // Floats in the signed report carry at most 9 significant digits.
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("report.json")).unwrap()).unwrap();
    let x = v["devices"][1]["share_within"].as_f64().unwrap();
    assert_eq!(x, grid::sig9(x));
}

#[test]
fn scada_time_stamp_findings() {
    let d = tmp("scada");
    let mut s = String::from("device,source_unix,receive_unix,seq\n");
    for i in 0..480u64 {
        let r = T0 + i as f64 * 30.0;
        // Millisecond stamps with a little transport jitter.
        let j = 0.001 * ((i * 37) % 97) as f64;
        // WTG-01 healthy; WTG-02 steps 4 s ahead at record 240; WTG-03 loses 4 s/day;
        // WTG-04 repeats and reverses stamps; WTG-05 stamps whole seconds.
        s.push_str(&format!("WTG-01,{:.3},{r:.3},{i}\n", r - 0.25 - j));
        let off = if i >= 240 { 4.0 } else { 0.0 };
        s.push_str(&format!("WTG-02,{:.3},{r:.3},{i}\n", r - 0.25 - j + off));
        let drift = 4.0 * (r - T0) / 86_400.0;
        s.push_str(&format!("WTG-03,{:.3},{r:.3},{i}\n", r - 0.25 - j - drift));
        let src = if i == 100 || i == 101 {
            T0 + 99.0 * 30.0
        } else if i == 300 {
            r - 100.0
        } else {
            r - 0.25 - j
        };
        s.push_str(&format!("WTG-04,{src:.3},{r:.3},{i}\n"));
        s.push_str(&format!("WTG-05,{:.0},{:.3},{i}\n", r - 1.0, r + 0.37));
    }
    std::fs::write(d.join("scada.csv"), s).unwrap();
    let spec: grid::ScadaSpec =
        toml::from_str("path = \"scada.csv\"\nwindow_s = 1800\nrequired_resolution_s = 0.001\n")
            .unwrap();
    let r = grid::check_scada(&spec, &d).unwrap();
    let f = |n: usize| r.devices[n].findings.clone();
    assert_eq!(r.devices[0].device, "WTG-01");
    assert_eq!(f(0), vec!["ok"], "{:?}", r.devices[0]);
    assert_eq!(r.devices[0].apparent_resolution_s, Some(1e-3));
    assert!(f(1).contains(&"clock-ahead".into()) && f(1).contains(&"clock-step".into()));
    assert_eq!(f(2), vec!["clock-drift"], "{:?}", r.devices[2]);
    let dr = r.devices[2].floor_drift_s_per_day.unwrap();
    assert!((dr - 4.0).abs() < 0.5, "{dr}");
    assert!(f(3).contains(&"duplicates".into()) && f(3).contains(&"backwards".into()));
    assert_eq!(r.devices[3].duplicates, 1);
    assert_eq!(f(4), vec!["coarse-resolution"], "{:?}", r.devices[4]);
    assert_eq!(r.devices[4].apparent_resolution_s, Some(1.0));
    assert!(r.verdict.starts_with("4 of 5"), "{}", r.verdict);
}

#[test]
fn event_forensics_implicates_timing_and_explains_inverted_sequence() {
    let d = tmp("event");
    pmu_fixture(&d);
    // SER-2 runs 60 ms ahead; SER-1 is good. True sequence: relay trip at +3010.000 s,
    // breaker open at +3010.040 s. SER-2 recorded the trip, SER-1 the breaker.
    std::fs::write(d.join("ser1.csv"), clock_csv(T0, 3600, 7, |_| 0.0)).unwrap();
    std::fs::write(d.join("ser2.csv"), clock_csv(T0, 3600, 8, |_| 60e-3)).unwrap();
    std::fs::write(
        d.join("soe.csv"),
        format!(
            "SER-2,{:.6},relay 21 trip\nSER-1,{:.6},breaker 52A open\nSER-1,{:.6},breaker 52B open\n",
            T0 + 3010.060,
            T0 + 3010.040,
            T0 + 3009.950
        ),
    )
    .unwrap();
    let src = format!(
        r#"
name = "synthetic line trip"
start_unix = {}
end_unix = {}
nominal_hz = 60.0
pre_s = 3000
[[device]]
name = "PMU-1"
kind = "pmu"
id = 1
capture = "capture.c37"
clock = {{ format = "csv", path = "pmu1.csv" }}
[[device]]
name = "PMU-2"
kind = "pmu"
id = 2
capture = "capture.c37"
clock = {{ format = "csv", path = "pmu2.csv" }}
[[device]]
name = "SER-1"
kind = "ser"
clock = {{ format = "csv", path = "ser1.csv" }}
[[device]]
name = "SER-2"
kind = "ser"
budget_s = 0.1
clock = {{ format = "csv", path = "ser2.csv" }}
[[device]]
name = "RTU-9"
kind = "rtu"
[soe]
path = "soe.csv"
[[soe.order]]
before = "relay 21 trip"
after = "breaker 52A open"
min_gap_s = 0.0
[[soe.order]]
before = "relay 21 trip"
after = "breaker 52B open"
[[soe.order]]
before = "relay 21 trip"
after = "breaker 99 open"
"#,
        T0 + 3000.0,
        T0 + 3020.0
    );
    std::fs::write(d.join("event.toml"), &src).unwrap();
    let req: EventRequest = toml::from_str(&src).unwrap();
    let r = grid::assess_event(&req, &d, UTC).unwrap();
    let st: Vec<(&str, &str)> = r
        .devices
        .iter()
        .map(|x| (x.name.as_str(), x.status.as_str()))
        .collect();
    assert_eq!(
        st,
        [
            ("PMU-1", "timing-not-implicated"),
            ("PMU-2", "timing-implicated"),
            ("SER-1", "timing-not-implicated"),
            ("SER-2", "timing-implicated"),
            ("RTU-9", "insufficient"),
        ],
        "{:#?}",
        r.devices
    );
    let p2 = &r.devices[1];
    assert_eq!(p2.drift_kind.as_deref(), Some("gradual-drift"));
    assert!(p2.drift_alarm_unix.unwrap() - T0 < 700.0);
    assert!(p2.phase_error_deg.unwrap() > 1.0);
    // The trip/52A inversion is explained by SER-2's 60 ms; 52B was genuinely earlier.
    assert_eq!(r.order[0].status, "inverted-explained-by-timing");
    assert!((r.order[0].recorded_gap_s.unwrap() + 0.020).abs() < 1e-6);
    assert!((r.order[0].corrected_gap_s.unwrap() - 0.040).abs() < 1e-6);
    assert_eq!(r.order[1].status, "inverted-not-explained");
    assert_eq!(r.order[2].status, "missing");
    assert!(
        r.verdict.starts_with("timing is implicated"),
        "{}",
        r.verdict
    );

    // Without the drifting PMU and the offset SER, timing is not implicated.
    let mut calm = req.clone();
    calm.device
        .retain(|x| x.name == "PMU-1" || x.name == "SER-1");
    calm.soe = None;
    let r2 = grid::assess_event(&calm, &d, UTC).unwrap();
    assert!(
        r2.verdict.starts_with("timing is not implicated"),
        "{}",
        r2.verdict
    );

    // Signed pack through the command line.
    let home = d.join("home");
    let (k, pk) = key(&d);
    let out = d.join("pack");
    let (ok, o) = run(
        &home,
        &[
            "grid",
            "event",
            d.join("event.toml").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "Ops",
            "--key-file",
            k.to_str().unwrap(),
            "--utc",
            UTC,
            "--dev",
        ],
    );
    assert!(ok && o.contains("timing is implicated"), "{o}");
    let (ok, o) = run(
        &home,
        &["dossier", "verify", out.to_str().unwrap(), "--trust", &pk],
    );
    assert!(ok, "{o}");
    assert!(std::fs::read_to_string(out.join("report.html"))
        .unwrap()
        .contains("inverted-explained-by-timing"));
}

// ------------------------------------------------------------------- SFN

fn sfn_fixture(d: &Path) -> String {
    // Four DAB mode I transmitters logged against an independent reference at 10 s for 2 h.
    // At +1 h, SVN-23-like: A, B and C follow a 13 µs UTC error; D rejects it and holds over.
    for (name, seed, follows) in [
        ("a", 11, true),
        ("b", 12, true),
        ("c", 13, true),
        ("d", 14, false),
    ] {
        let mut s = String::new();
        for i in 0..720u64 {
            let t = T0 + i as f64 * 10.0;
            let step = if follows && i >= 360 { 13e-6 } else { 0.0 };
            s.push_str(&format!(
                "{t},{:e}\n",
                step + 40e-9 * noise(seed * 7919 + i)
            ));
        }
        std::fs::write(d.join(format!("tx-{name}.csv")), s).unwrap();
    }
    let src = r#"
network = "Synthetic DAB SFN"
system = "dab-mode-1"
utc_limit_s = 1e-6
bin_s = 60

[[transmitter]]
name = "TX-A"
on_bad_reference = "accept"
log = { format = "csv", path = "tx-a.csv" }
oscillator = { class = "ocxo" }
[[transmitter]]
name = "TX-B"
on_bad_reference = "accept"
log = { format = "csv", path = "tx-b.csv" }
oscillator = { class = "ocxo" }
[[transmitter]]
name = "TX-C"
on_bad_reference = "accept"
log = { format = "csv", path = "tx-c.csv" }
oscillator = { class = "ocxo", initial_freq = 5e-12, u_initial_freq = 2e-12 }
[[transmitter]]
name = "TX-D"
on_bad_reference = "holdover"
log = { format = "csv", path = "tx-d.csv" }
oscillator = { class = "ocxo", initial_freq = -5e-12, u_initial_freq = 2e-12 }

[[pair]]
a = "TX-A"
b = "TX-B"
planned_offset_us = 150
[[pair]]
a = "TX-C"
b = "TX-D"
planned_offset_us = 240

[[scenario]]
name = "SVN-23 as configured"
preset = "svn23"

[[scenario]]
name = "SVN-23 seen at A-C only"
preset = "svn23"
transmitters = ["TX-A", "TX-B", "TX-C"]

[[scenario]]
name = "GNSS lost everywhere"
kind = "gnss-loss"
duration_h = 24
"#;
    std::fs::write(d.join("sfn.toml"), src).unwrap();
    src.to_string()
}

#[test]
fn sfn_svn23_reproduced_in_logs_and_scenarios() {
    let d = tmp("sfn");
    let src = sfn_fixture(&d);
    let req = SfnRequest::parse(&src).unwrap();
    let r = sfn::assess(&req, &d, UTC).unwrap();
    assert!((r.guard_s - 246.09375e-6).abs() < 1e-12);
    // Recorded logs: the common mode moves 13 µs (UTC level), TX-D departs from the others,
    // and the C–D pair (planned 240 µs of 246 µs) runs out of margin; A–B stays inside.
    let m = r.monitor.as_ref().unwrap();
    assert_eq!(m.bins, 120);
    assert!((m.max_abs_common_mode_s - 13e-6).abs() < 0.2e-6);
    let kinds: Vec<(&str, &str)> = m
        .events
        .iter()
        .map(|e| (e.kind.as_str(), e.subject.as_str()))
        .collect();
    assert!(kinds.contains(&("utc-level", "network")), "{kinds:?}");
    assert!(kinds.contains(&("transmitter-offset", "TX-D")), "{kinds:?}");
    assert!(kinds.contains(&("pair-margin", "TX-C–TX-D")), "{kinds:?}");
    assert!(!kinds.iter().any(|k| k.1 == "TX-A–TX-B"));
    let cd = &m.pairs[1];
    assert_eq!(cd.bins_beyond, 60);
    assert!((cd.min_margin_s - (6.09375e-6 - 13e-6)).abs() < 0.2e-6);
    let utc_ev = m.events.iter().find(|e| e.kind == "utc-level").unwrap();
    assert!((utc_ev.start_unix - (T0 + 3600.0)).abs() < 1.0);

    // Scenario 1 (as configured): D holds over, C–D out of margin; one alarm.
    let s1 = &r.scenarios[0];
    assert_eq!(s1.kind, "utc-step");
    assert_eq!(s1.sfn_status, "self-interference-risk", "{}", s1.verdict);
    assert_eq!(s1.utc_status, "utc-level-error");
    assert_eq!((s1.alarms, s1.holdover_entries, s1.muted), (1, 1, 0));
    assert_eq!(s1.pairs[0].status, "ok");
    assert_eq!(s1.pairs[1].status, "self-interference-risk");
    // Scenario 2: only A-C receive the bad UTC (they follow it), D stays locked: C–D breaks,
    // and no transmitter raises an alarm.
    let s2 = &r.scenarios[1];
    assert_eq!(s2.pairs[1].status, "self-interference-risk");
    assert_eq!(s2.alarms, 0);
    // Scenario 3: 24 h of holdover; the pair budget lasts, the 1 µs UTC limit does not.
    let s3 = &r.scenarios[2];
    assert_eq!(s3.holdover_entries, 4);
    assert_eq!(s3.sfn_status, "intact", "{}", s3.verdict);
    assert!(s3.first_utc_breach_s.unwrap() < 24.0 * 3600.0);
    assert!(
        s3.first_pair_breach_s.is_none_or(|x| x > 24.0 * 3600.0),
        "{}",
        s3.verdict
    );

    // All four follow the error together: SFN intact, off UTC.
    let mut all = req.clone();
    all.transmitter[3].on_bad_reference = "accept".into();
    all.scenario.truncate(1);
    let r2 = sfn::assess(&all, &d, UTC).unwrap();
    let s = &r2.scenarios[0];
    assert_eq!(
        (s.sfn_status.as_str(), s.utc_status.as_str()),
        ("intact", "utc-level-error")
    );
    assert!(s.verdict.contains("not at UTC level"), "{}", s.verdict);
    // A transmitter that mutes on a bad reference: coverage reduced.
    let mut mute = all.clone();
    mute.transmitter[3].on_bad_reference = "mute".into();
    let r3 = sfn::assess(&mute, &d, UTC).unwrap();
    assert_eq!(r3.scenarios[0].coverage, "reduced");
    assert_eq!(r3.scenarios[0].pairs[1].status, "one-muted");

    // Errors: unknown system, unknown transmitter, holdover without an oscillator.
    let mut bad = req.clone();
    bad.system = Some("dab-mode-9".into());
    assert!(sfn::assess(&bad, &d, UTC).is_err());
    let mut bad = req.clone();
    bad.pair[0].a = "TX-Z".into();
    assert!(sfn::assess(&bad, &d, UTC).is_err());
    let mut bad = req.clone();
    bad.transmitter[3].oscillator = None;
    assert!(sfn::assess(&bad, &d, UTC)
        .unwrap_err()
        .contains("needs its oscillator"));

    // Signed through the command line; `systems` is free.
    let home = d.join("home");
    let (k, pk) = key(&d);
    let out = d.join("pack");
    let (ok, o) = run(
        &home,
        &[
            "sfn",
            "assess",
            d.join("sfn.toml").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--signer",
            "Ops",
            "--key-file",
            k.to_str().unwrap(),
            "--utc",
            UTC,
            "--dev",
        ],
    );
    assert!(ok, "{o}");
    let (ok, o) = run(
        &home,
        &["dossier", "verify", out.to_str().unwrap(), "--trust", &pk],
    );
    assert!(ok, "{o}");
    let (ok, o) = run(&home, &["sfn", "systems"]);
    assert!(
        ok && o.contains("dab-mode-1") && o.contains("246.094"),
        "{o}"
    );
    let (ok, o) = run(
        &home,
        &["sfn", "assess", d.join("sfn.toml").to_str().unwrap()],
    );
    assert!(!ok && o.contains("requires"), "{o}");
}

#[test]
fn grid_cli_json_commands() {
    let d = tmp("cli");
    pmu_fixture(&d);
    std::fs::write(
        d.join("pmu.toml"),
        "nominal_hz = 60.0\n[[pmu]]\nname = \"PMU-2\"\nid = 2\ncapture = \"capture.c37\"\nclock = { format = \"csv\", path = \"pmu2.csv\" }\n",
    )
    .unwrap();
    std::fs::write(
        d.join("drift.toml"),
        "nominal_hz = 60.0\nclock = { format = \"csv\", path = \"pmu2.csv\" }\n[detector]\nwindow_s = 300\n",
    )
    .unwrap();
    let home = d.join("home");
    let j = d.join("pmu.json");
    let (ok, o) = run(
        &home,
        &[
            "grid",
            "pmu",
            d.join("pmu.toml").to_str().unwrap(),
            "--json",
            j.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok && o.contains("beyond-budget-flags-silent"), "{o}");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&j).unwrap()).unwrap();
    assert_eq!(v["format"], grid::PMU_FORMAT);
    let csv = d.join("trace.csv");
    let (ok, o) = run(
        &home,
        &[
            "grid",
            "drift",
            d.join("drift.toml").to_str().unwrap(),
            "--json",
            d.join("drift.json").to_str().unwrap(),
            "--csv",
            csv.to_str().unwrap(),
            "--dev",
        ],
    );
    assert!(ok && o.contains("gradual-drift alarm"), "{o}");
    assert!(csv.exists());
    let (ok, o) = run(&home, &["grid", "bogus", "x"]);
    assert!(!ok && o.contains("usage: kshana-pro grid"), "{o}");
    let (ok, o) = run(
        &home,
        &["grid", "pmu", d.join("pmu.toml").to_str().unwrap()],
    );
    assert!(!ok && o.contains("requires"), "{o}");
}
