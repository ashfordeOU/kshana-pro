// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! Bench readiness on the command line with the built-in datasheet catalogue: the catalogue is
//! free, the rest needs the licence; the example benches check, kit and rehearse; an RF-only
//! SDR is refused as an eLoran source with its datasheet value.

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");

fn bench_file(name: &str) -> String {
    format!(
        "{}/crates/kshana-eloran/scenarios/bench/{name}.toml",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .env_remove("KSHANA_PRO_LICENSE")
        .output()
        .expect("run kshana-pro")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kshana-bench-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn catalogue_is_free_and_lists_lf_sources() {
    let o = run(&["eloran", "bench", "catalog", "--lf"]);
    assert!(o.status.success(), "{}", text(&o));
    let t = text(&o);
    assert!(t.contains("redpitaya-stemlab-125-14"), "{t}");
    assert!(t.contains("ettus-lftx"), "{t}");
    assert!(
        !t.contains("ettus-usrp-b210"),
        "an RF-only SDR is not an LF source: {t}"
    );
    let o = run(&["eloran", "bench", "show", "ettus-usrp-b210"]);
    assert!(o.status.success());
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["tx"]["freq_min_hz"], 70e6);
    assert!(v["sources"][0]["url"].as_str().unwrap().starts_with("http"));
}

#[test]
fn check_needs_the_licence() {
    let o = run(&[
        "eloran",
        "bench",
        "check",
        "--inventory",
        &bench_file("redpitaya-lf-bench"),
    ]);
    assert!(!o.status.success(), "{}", text(&o));
}

#[test]
fn example_benches_check_kit_and_rehearse() {
    for name in ["redpitaya-lf-bench", "lab-awg-bench"] {
        let f = bench_file(name);
        let o = run(&[
            "eloran",
            "bench",
            "check",
            "--inventory",
            &f,
            "--json",
            "--dev",
        ]);
        assert!(o.status.success(), "{name}: {}", text(&o));
        let a: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(a["ready"], true, "{name}: {a}");
        let d = tmp(name);
        let kit = d.join("kit");
        let o = run(&[
            "eloran",
            "bench",
            "kit",
            "--inventory",
            &f,
            "--out",
            kit.to_str().unwrap(),
            "--dev",
        ]);
        assert!(o.status.success(), "{name}: {}", text(&o));
        for file in [
            "WIRING.md",
            "PROCEDURE.md",
            "assessment.json",
            "devices.json",
            "inventory.toml",
        ] {
            assert!(kit.join(file).exists(), "{name}: {file} missing");
        }
        let wiring = std::fs::read_to_string(kit.join("WIRING.md")).unwrap();
        assert!(wiring.contains("Conducted use only"));
        assert!(wiring.contains("attenuate"), "{wiring}");
        let rep = d.join("rehearsal.json");
        let o = run(&[
            "eloran",
            "bench",
            "rehearse",
            "--inventory",
            &f,
            "--out",
            rep.to_str().unwrap(),
            "--dev",
        ]);
        assert!(o.status.success(), "{name}: {}", text(&o));
        let r: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&rep).unwrap()).unwrap();
        let results = r["report"]["results"].as_array().unwrap();
        for x in results {
            assert!(
                x["verdict"] == "pass" || x["verdict"] == "not_run",
                "{name}: {x}"
            );
        }
        assert!(
            results
                .iter()
                .any(|x| x["id"] == "AT-PPS-JIT" && x["verdict"] == "pass"),
            "{name}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn rf_only_sdr_is_refused_as_eloran_source() {
    let o = run(&[
        "eloran",
        "bench",
        "check",
        "--inventory",
        &bench_file("rf-sdr-as-eloran-source"),
        "--json",
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    let a: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(a["ready"], false);
    let band = a["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["check"] == "90–110 kHz band")
        .unwrap();
    assert_eq!(band["status"], "fail");
    assert!(
        band["detail"].as_str().unwrap().contains("70.000 MHz"),
        "{band}"
    );
    let o = run(&["eloran", "bench", "twin", "ettus-usrp-b210", "--dev"]);
    assert!(!o.status.success());
    assert!(text(&o).contains("cannot output 90–110 kHz"));
    let o = run(&[
        "eloran",
        "stream",
        &bench_file("rf-sdr-as-eloran-source"),
        "--device",
        "twin:nonexistent",
        "--dev",
    ]);
    assert!(!o.status.success());
}

#[test]
fn probe_needs_the_hardware_licence_feature_and_reports_unreachable_hosts() {
    let o = run(&[
        "eloran",
        "bench",
        "probe",
        "--host",
        "127.0.0.1:1",
        "--timeout-ms",
        "300",
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("unreachable"));
}
