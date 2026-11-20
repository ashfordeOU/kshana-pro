// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Simulation fidelity and lab tooling (`kshana_pro::simlab`, docs/SIMLAB.md): the recorder
//! chain cascade and its register with change detection, feeding `testkit fidelity`; the
//! multi-sensor replay check on an aligned and a misaligned set; the simulator self-test
//! attributing a common power offset to the simulator and a lost PRN to one receiver; two lab
//! layouts normalised to the common report and compared with the En number; the TCO
//! break-even; signing, verification and the licence gates. All inputs are synthetic.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{self, Entitlement, Tier};
use kshana_pro::simlab::{self, chain, labreport, selftest, sync, synth, tco};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");
const SEED: [u8; 32] = [9u8; 32];
const UTC: &str = "2026-11-20T12:00:00Z";

fn fixture(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kp-simlab-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    synth::write_all(&d).unwrap();
    std::fs::write(d.join("key.hex"), hex::encode(SEED)).unwrap();
    d
}

fn run(args: &[&str]) -> (bool, String) {
    let o = Command::new(BIN).args(args).output().unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr),
    )
}

fn produce(d: &Path, cmd: &str, inputs: &[&str], out: &str, extra: &[&str]) -> PathBuf {
    let o = d.join(out);
    let ins: Vec<String> = inputs
        .iter()
        .map(|i| {
            if Path::new(i).is_absolute() {
                i.to_string()
            } else {
                d.join(i).display().to_string()
            }
        })
        .collect();
    let mut a: Vec<&str> = vec!["simlab", cmd];
    a.extend(ins.iter().map(String::as_str));
    let key = d.join("key.hex");
    a.extend([
        "--out",
        o.to_str().unwrap(),
        "--signer",
        "test",
        "--key-file",
        key.to_str().unwrap(),
        "--utc",
        UTC,
        "--dev",
    ]);
    a.extend(extra);
    let (ok, text) = run(&a);
    assert!(ok, "{text}");
    o
}

fn json(p: PathBuf) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn the_recorder_chain_cascades_registers_and_feeds_fidelity() {
    let d = fixture("chain");
    let c =
        chain::Chain::parse(&std::fs::read_to_string(d.join("chain-v1.toml")).unwrap()).unwrap();
    let rep = chain::assess(&c, "x");
    // Friis by hand: LNA 1.8 dB / 28 dB, cable 4.4 dB loss, SAW 2.5 dB loss, IF 4 dB / 20 dB.
    let lin = |x: f64| 10f64.powf(x / 10.0);
    let mut f = lin(1.8);
    let mut g = lin(28.0);
    for (nf, gain) in [(4.4, -4.4), (2.5, -2.5), (4.0, 20.0)] {
        f += (lin(nf) - 1.0) / g;
        g *= lin(gain);
    }
    assert!(
        (rep.cascade_nf_db - 10.0 * f.log10()).abs() < 1e-3,
        "{}",
        rep.cascade_nf_db
    );
    assert!((rep.cascade_gain_db - 41.1).abs() < 1e-9);
    assert_eq!(rep.bandwidth_hz, 16e6);
    assert!((rep.group_delay_ns - 135.0).abs() < 1e-9);
    assert!((rep.group_delay_u_ns - (4.0f64 + 0.25 + 9.0).sqrt()).abs() < 1e-3);
    assert!(
        rep.findings.iter().all(|f| f.severity == "ok"),
        "{:?}",
        rep.findings
    );

    // Register v1 then v2: the LNA swap and the AGC show up.
    let reg = d.join("register.jsonl");
    let reg_s = reg.to_str().unwrap();
    let o1 = produce(
        &d,
        "chain",
        &["chain-v1.toml"],
        "out-c1",
        &["--register", reg_s],
    );
    assert_eq!(
        json(o1.join("chain.json"))["changes_since_previous"],
        serde_json::json!([])
    );
    let o2 = produce(
        &d,
        "chain",
        &["chain-v2.toml"],
        "out-c2",
        &["--register", reg_s],
    );
    let v = json(o2.join("chain.json"));
    assert_eq!(v["previous_registration"], "2026-05-04");
    assert_eq!(v["changes_since_previous"][0]["quantity"], "cascade_nf_db");
    assert!(v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["id"] == "agc"));
    let r = kshana_eloran::calreg::Register::open(&reg);
    let vr = r.verify().unwrap();
    assert!(vr.ok() && vr.entries == 10);
    assert_eq!(v["register_head"].as_str().unwrap(), vr.head);

    // The derived capture runs through testkit fidelity against a pack scenario.
    let (ok, text) = run(&[
        "testkit",
        "fidelity",
        o2.join("capture.toml").to_str().unwrap(),
        "--scenario",
        "timing/timing-holdover-under-jamming",
        "--dev",
    ]);
    assert!(ok && text.contains("agc"), "{text}");
    let cap = kshana_pro::testkit::fidelity::Capture::parse(
        &std::fs::read_to_string(o2.join("capture.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(cap.bit_depth, 4);
    assert_eq!(cap.agc.as_deref(), Some("active"));
    assert_eq!(cap.recorder_phase_noise.len(), 2);
}

#[test]
fn the_replay_check_aligns_a_good_set_and_flags_a_bad_one() {
    let d = fixture("sync");
    let good = sync::check(&sync::load(&d.join("drive-good.toml")).unwrap());
    assert_eq!(good.verdict, "synchronised", "{:#?}", good.streams);
    assert!(good.sensors_synchronised);
    let imu = good.streams.iter().find(|s| s.name == "imu").unwrap();
    assert!((imu.drift_ppm - 20.0).abs() < 0.01, "{}", imu.drift_ppm);
    assert!((imu.offset_s - 999.5).abs() < 1e-4);
    let can = good.streams.iter().find(|s| s.name == "can").unwrap();
    assert_eq!(can.gaps, 1);
    assert!((can.drift_ppm + 30.0).abs() < 0.01);
    let w = good.window_s.unwrap();
    assert!((w[0] - 1000.2).abs() < 1e-6 && (w[1] - (999.5 + 60.0 * 1.00002)).abs() < 1e-6);
    let iq = good.streams.iter().find(|s| s.name == "gnss-iq").unwrap();
    assert_eq!(iq.rate_hz, 1000.0);
    assert_eq!(iq.replay_start_sample, Some(200));

    let bad = sync::check(&sync::load(&d.join("drive-bad.toml")).unwrap());
    assert_eq!(bad.verdict, "not-synchronised");
    assert!(!bad.sensors_synchronised);
    let st = |n: &str| {
        bad.streams
            .iter()
            .find(|s| s.name == n)
            .unwrap()
            .status
            .clone()
    };
    assert_eq!(st("camera"), "unsynchronisable");
    assert_eq!(st("can"), "misaligned");
    assert_eq!(st("imu"), "aligned");
    let o = produce(&d, "sync", &["drive-bad.toml"], "out-sync", &[]);
    assert_eq!(json(o.join("sync.json"))["verdict"], "not-synchronised");
    assert!(o.join("inputs/imu.csv").exists());
}

#[test]
fn the_self_test_separates_simulator_and_receiver_faults() {
    let d = fixture("selftest");
    let sim = selftest::assess(&selftest::load(&d.join("selftest-sim.toml")).unwrap());
    assert_eq!(sim.verdict, "simulator-deviation", "{:#?}", sim.attribution);
    let p = &sim.attribution[0];
    assert_eq!((p.check.as_str(), p.to.as_str()), ("power-level", "source"));
    assert!((p.source_estimate.unwrap() - 1.45).abs() < 0.15);
    assert!(sim.attribution[1..].iter().all(|a| a.to == "none"));

    let rig = selftest::assess(&selftest::load(&d.join("selftest-rig.toml")).unwrap());
    assert_eq!(rig.verdict, "receiver-deviation", "{:#?}", rig.attribution);
    let rel = rig
        .attribution
        .iter()
        .find(|a| a.check == "relative-power")
        .unwrap();
    assert_eq!(rel.to, "receiver");
    assert_eq!(rel.receivers, vec!["REF-B".to_string()]);
    assert_eq!(rig.receivers[1].missing_prns, vec!["G07".to_string()]);

    let o = produce(&d, "selftest", &["selftest-rig.toml"], "out-rig", &[]);
    assert_eq!(
        json(o.join("selftest.json"))["verdict"],
        "receiver-deviation"
    );
}

#[test]
fn two_lab_layouts_map_to_one_report_and_compare() {
    let d = fixture("labs");
    let a = labreport::normalise(&labreport::load(&d.join("lab-a.toml")).unwrap()).unwrap();
    let b = labreport::normalise(&labreport::load(&d.join("lab-b.toml")).unwrap()).unwrap();
    assert_eq!(a.unmapped_columns, vec!["Operator".to_string()]);
    assert!(a.missing_core_metrics.is_empty());
    assert_eq!(
        b.missing_core_metrics,
        vec!["time_error_max_ns".to_string()]
    );
    let ttd = |r: &labreport::LabReport| {
        r.results
            .iter()
            .find(|x| x.scenario == "spoof-sync-takeover" && x.metric == "time_to_detect_s")
            .cloned()
            .unwrap()
    };
    assert_eq!((ttd(&a).value, ttd(&a).expanded_u), (2.4, Some(0.3)));
    assert_eq!((ttd(&b).value, ttd(&b).expanded_u), (2.9, Some(0.2)));
    assert!(
        a.en16803_topics.iter().all(|t| t.status == "carried"),
        "{:#?}",
        a.en16803_topics
    );

    let c = labreport::compare(&[a, b]).unwrap();
    let row = |s: &str, m: &str| {
        c.rows
            .iter()
            .find(|x| x.scenario == s && x.metric == m)
            .unwrap()
            .clone()
    };
    let t = row("spoof-sync-takeover", "time_to_detect_s");
    assert_eq!(t.pairs[0].outcome, "inconsistent");
    assert!((t.pairs[0].en.unwrap() - 0.5 / 0.13f64.sqrt()).abs() < 1e-3);
    assert_eq!(
        row("spoof-sync-takeover", "position_error_max_m").pairs[0].outcome,
        "consistent"
    );
    assert_eq!(
        row("spoof-sync-takeover", "detected").pairs[0].outcome,
        "agree"
    );
    let m = row("spoof-meaconing", "captured");
    assert_eq!(m.pairs[0].outcome, "not-comparable");
    assert!(m.pairs[0].reasons[0].contains("hash differs"));

    // End to end: signed reports, a compare that checks their signatures, tamper detection.
    let pk = hex::encode(license::public_key_for(&SEED));
    let ra = produce(&d, "report", &["lab-a.toml"], "out-a", &[]);
    let rb = produce(&d, "report", &["lab-b.toml"], "out-b", &[]);
    assert!(std::fs::read_to_string(ra.join("lab-report.md"))
        .unwrap()
        .contains("EN 16803 topic index"));
    let cmp = produce(
        &d,
        "compare",
        &[ra.to_str().unwrap(), rb.to_str().unwrap()],
        "out-cmp",
        &["--trust", &pk],
    );
    let v = json(cmp.join("comparison.json"));
    assert_eq!(v["consistent"], c.consistent);
    let (ok, text) = run(&["simlab", "check", cmp.to_str().unwrap(), "--trust", &pk]);
    assert!(ok, "{text}");
    let f = rb.join("lab-report.json");
    let s = std::fs::read_to_string(&f).unwrap();
    std::fs::write(&f, s.replace("2.9", "2.5")).unwrap();
    let key = d.join("key.hex");
    let (ok, text) = run(&[
        "simlab",
        "compare",
        ra.to_str().unwrap(),
        rb.to_str().unwrap(),
        "--out",
        d.join("out-cmp2").to_str().unwrap(),
        "--signer",
        "t",
        "--key-file",
        key.to_str().unwrap(),
        "--trust",
        &pk,
        "--dev",
    ]);
    assert!(!ok && text.contains("signature"), "{text}");
    // Over-the-air results are refused.
    let src = std::fs::read_to_string(d.join("lab-a.toml")).unwrap();
    std::fs::write(
        d.join("lab-ota.toml"),
        src.replace("rf_path = \"conducted\"", "rf_path = \"over-the-air\""),
    )
    .unwrap();
    assert!(labreport::load(&d.join("lab-ota.toml")).is_err());
}

#[test]
fn tco_ranks_options_and_finds_break_even() {
    let d = fixture("tco");
    let t = tco::Tco::parse(&std::fs::read_to_string(d.join("tco.toml")).unwrap()).unwrap();
    let rep = tco::compute(&t);
    assert_eq!(rep.options.len(), 4);
    // Test as a service: 4 × 40k a year over 5 years at 6 % → 160k × 4.21236.
    let svc = rep
        .options
        .iter()
        .find(|o| o.name == "Test as a service")
        .unwrap();
    assert!(
        (svc.present_value - 160_000.0 * 4.212_363_8).abs() < 1.0,
        "{}",
        svc.present_value
    );
    let be = rep
        .break_even
        .iter()
        .find(|b| b.a.starts_with("Record-and-replay") && b.b == "Test as a service")
        .unwrap();
    // 44k + 20k·a = 36k·a·n  →  n = (44k/a + 20k)/36k.
    let a = 4.212_363_8;
    assert!((be.campaigns_per_year.unwrap() - (44_000.0 / a + 20_000.0) / 36_000.0).abs() < 1e-3);
    let (ok, text) = run(&["simlab", "tco", d.join("tco.toml").to_str().unwrap()]);
    assert!(
        ok && text.contains("Break-even") && text.contains("ILLUSTRATIVE"),
        "{text}"
    );
}

#[test]
fn features_are_tiered_and_gated() {
    for f in [
        simlab::FEATURE_REPLAY,
        simlab::FEATURE_SELFTEST,
        simlab::FEATURE_REPORT,
    ] {
        assert_eq!(license::explicit_tier(f), Some(Tier::Pro));
        assert!(license::PRO_COMMAND_FEATURES.contains(&f));
    }
    let free = Entitlement {
        licensee: "ACME".into(),
        tier: Tier::Evaluation,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&free, simlab::FEATURE_REPORT, 0).is_err());
    let pro = Entitlement {
        tier: Tier::Pro,
        ..free
    };
    assert!(license::check(&pro, simlab::FEATURE_SELFTEST, 0).is_ok());
    let (ok, _) = run(&["simlab"]);
    assert!(!ok);
}
