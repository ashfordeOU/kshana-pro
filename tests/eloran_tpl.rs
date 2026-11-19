// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//! `kshana-pro eloran tpl`: licensed; on the inland example it reports a finite TPL with the
//! four stations, the hypotheses and the evidence label, writes JSON, and an availability map.

use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");

fn scenario() -> String {
    format!(
        "{}/crates/kshana-eloran/scenarios/tpl-inland-timing.toml",
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

#[test]
fn tpl_reports_a_bound_and_an_availability_map() {
    let o = run(&["eloran", "tpl", &scenario(), "--samples", "100"]);
    assert!(!o.status.success(), "licensed: {}", text(&o));
    let d = std::env::temp_dir().join(format!("kshana-tpl-cli-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let out = d.join("tpl.json");
    let o = run(&[
        "eloran",
        "tpl",
        &scenario(),
        "--samples",
        "100",
        "--alert-ns",
        "1000000",
        "--out",
        out.to_str().unwrap(),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    let t = text(&o);
    assert!(
        t.contains("AVAILABLE") && t.contains("H_ASF model error"),
        "{t}"
    );
    assert!(t.contains("MODELLED"), "{t}");
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(r["stations"].as_array().unwrap().len(), 4);
    let tpl = r["tpl_ns"].as_f64().unwrap();
    assert!(tpl.is_finite() && tpl > r["sigma_ns"].as_f64().unwrap());
    // UTC(B) has no station in this scenario
    let o = run(&[
        "eloran",
        "tpl",
        &scenario(),
        "--samples",
        "100",
        "--time-ref",
        "UTC(B)",
        "--dev",
    ]);
    assert!(!o.status.success(), "{}", text(&o));
    // availability map
    let m = d.join("map.json");
    let o = run(&[
        "eloran",
        "tpl",
        &scenario(),
        "--samples",
        "60",
        "--grid",
        "51,52,0,1",
        "--res",
        "1",
        "--out",
        m.to_str().unwrap(),
        "--dev",
    ]);
    assert!(o.status.success(), "{}", text(&o));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&m).unwrap()).unwrap();
    assert_eq!(v["points"].as_array().unwrap().len(), 4);
    let _ = std::fs::remove_dir_all(&d);
}
