// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! ELOF on the command line: a scenario's truth table converts to ELOF (JSON lines and CSV),
//! both validate, a corrupted file is refused with a non-zero exit, and conversion needs the
//! licence while validation does not.

use std::path::PathBuf;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");

fn scenario() -> String {
    format!(
        "{}/crates/kshana-eloran/scenarios/rx-high-snr.toml",
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
    let d = std::env::temp_dir().join(format!("kshana-elof-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn truth_table_converts_validates_and_corruption_is_refused() {
    let d = tmp("cli");
    let base = d.join("run");
    let o = run(&[
        "eloran",
        "run",
        &scenario(),
        "--out",
        base.to_str().unwrap(),
        "--format",
        "none",
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    let truth = format!("{}.truth.csv", base.display());
    let jsonl = d.join("run.elof.jsonl");
    // conversion is licensed
    let o = run(&[
        "eloran",
        "elof",
        "convert",
        &truth,
        "--scenario",
        &scenario(),
        "--out",
        jsonl.to_str().unwrap(),
    ]);
    assert!(!o.status.success(), "{}", text(&o));
    let o = run(&[
        "eloran",
        "elof",
        "convert",
        &truth,
        "--scenario",
        &scenario(),
        "--out",
        jsonl.to_str().unwrap(),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("valid"), "{}", text(&o));
    // validation is free
    let o = run(&[
        "eloran",
        "elof",
        "validate",
        jsonl.to_str().unwrap(),
        "--json",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    let rep: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    assert!(rep["errors"].as_array().unwrap().is_empty(), "{rep}");
    assert!(rep["counts"]["truth"].as_u64().unwrap() > 10, "{rep}");
    let body = std::fs::read_to_string(&jsonl).unwrap();
    let first: serde_json::Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
    assert_eq!(first["format"], "ELOF");
    assert_eq!(first["version"], "0.1");
    assert_eq!(first["truth_included"], true);
    assert_eq!(first["scenarios"][0]["sha256"].as_str().unwrap().len(), 64);
    // ELOF → CSV profile (truth has no CSV form: the CSV holds the header only, still valid)
    let csv = d.join("run.elof.csv");
    let o = run(&[
        "eloran",
        "elof",
        "convert",
        jsonl.to_str().unwrap(),
        "--out",
        csv.to_str().unwrap(),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("no CSV form"), "{}", text(&o));
    let o = run(&["eloran", "elof", "validate", csv.to_str().unwrap()]);
    assert!(o.status.success(), "{}", text(&o));
    // a file whose ToA unit was changed is refused
    let bad = d.join("bad.elof.jsonl");
    std::fs::write(
        &bad,
        body.replacen("\"toa_ns\":\"ns\"", "\"toa_ns\":\"us\"", 1),
    )
    .unwrap();
    let o = run(&["eloran", "elof", "validate", bad.to_str().unwrap()]);
    assert!(!o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("units: toa_ns"), "{}", text(&o));
    let _ = std::fs::remove_dir_all(&d);
}
