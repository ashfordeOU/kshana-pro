// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The test-as-code kit (`kshana_pro::testkit`): neutral scenarios, the compiler and
//! validator over the simulator adapters, the packs and the CLI.
//! Evidence: MODELLED (synthetic receiver logs).

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::license::{self, Entitlement, Tier};
use kshana_pro::testkit::compile::{self, Support};
use kshana_pro::testkit::spec::{Spec, Technique};
use kshana_pro::testkit::{library, packs};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-testkit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

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
fn tier_is_explicit_pro() {
    assert_eq!(license::explicit_tier("pro-test-kit"), Some(Tier::Pro));
    assert!(license::pro_command_features().contains(&"pro-test-kit"));
    let pro = Entitlement {
        licensee: "L".into(),
        tier: Tier::Pro,
        expires_unix: 0,
        features: vec![],
        arksa: false,
    };
    assert!(license::check(&pro, "pro-test-kit", 0).is_ok());
    let eval = Entitlement {
        tier: Tier::Evaluation,
        ..pro
    };
    assert!(license::check(&eval, "pro-test-kit", 0).is_err());
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
