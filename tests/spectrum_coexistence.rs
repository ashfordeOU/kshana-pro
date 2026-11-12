// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Spectrum coexistence: acceptance tests.
//!
//! Every expected value here is read from a direct run of the open engine inside the
//! test (`kshana::api::run_scenario` on `leo-signal` or `constellation-design`), never
//! typed in, except the four coefficients the open document states for the public Xona
//! Pulsar X5 signal (test 2), which are the published cross-check of what is read.

use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::spectrum_coexistence::{self as sc, plan::LEO_SIGNAL_PRESET_FILES};
use serde_json::Value;

const DB2: f64 = 3.010_299_956_639_812; // 10 log10(2)

fn open(src: &str) -> Value {
    let out =
        kshana::api::run_scenario(src).unwrap_or_else(|e| panic!("open run failed: {e}\n{src}"));
    serde_json::from_str(&out.json).expect("open result is JSON")
}

fn run(src: &str) -> Value {
    sc::run_study(src).unwrap_or_else(|e| panic!("study failed: {e}\n{src}"))
}

fn run_err(src: &str) -> String {
    match sc::run_study(src) {
        Ok(_) => panic!("the study should have been refused:\n{src}"),
        Err(e) => e.to_string(),
    }
}

/// The generic representative designs, read from a direct open run of their presets.
fn generic_designs() -> Vec<Value> {
    let r = open("kind = \"leo-signal\"\npresets = [\"generic-bands\", \"generic-c-band\"]\n");
    r["signals"].as_array().unwrap().clone()
}

/// A design read from an open run, written back as an inline `leo-signal` signal table
/// with a stated received power.
fn inline_design(s: &Value, power_dbw: f64) -> toml::Table {
    let mut t = toml::Table::new();
    let st = |k: &str| s[k].as_str().map(|x| toml::Value::String(x.to_string()));
    t.insert("name".into(), st("name").unwrap());
    if let Some(v) = st("system") {
        t.insert("system".into(), v);
    }
    t.insert("source".into(), st("source").unwrap());
    if let Some(v) = st("source_ref") {
        t.insert("source_ref".into(), v);
    }
    t.insert(
        "centre_mhz".into(),
        toml::Value::Float(s["centre_hz"].as_f64().unwrap() / 1e6),
    );
    t.insert(
        "tx_bandwidth_mhz".into(),
        toml::Value::Float(s["tx_bandwidth_hz"].as_f64().unwrap() / 1e6),
    );
    t.insert("allocation".into(), st("allocation").unwrap());
    t.insert("received_power_dbw".into(), toml::Value::Float(power_dbw));
    t.insert(
        "ranging".into(),
        toml::Value::Boolean(s["ranging"].as_bool().unwrap()),
    );
    let comps = s["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let mut ct = toml::Table::new();
            ct.insert(
                "role".into(),
                toml::Value::String(c["role"].as_str().unwrap().into()),
            );
            ct.insert(
                "modulation".into(),
                toml::Value::String(c["modulation"].as_str().unwrap().into()),
            );
            ct.insert(
                "power_fraction".into(),
                toml::Value::Float(c["power_fraction"].as_f64().unwrap()),
            );
            if let Some(n) = c["code_length_chips"].as_u64() {
                ct.insert("code_length_chips".into(), toml::Value::Integer(n as i64));
            }
            if let Some(d) = c["data_rate_sps"].as_f64() {
                ct.insert("data_rate_sps".into(), toml::Value::Float(d));
            }
            let offs: Vec<toml::Value> = c["fdma_offsets_hz"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| toml::Value::Float(x.as_f64().unwrap() / 1e6))
                .collect();
            if !offs.is_empty() {
                ct.insert("fdma_offsets_mhz".into(), toml::Value::Array(offs));
            }
            toml::Value::Table(ct)
        })
        .collect();
    t.insert("components".into(), toml::Value::Array(comps));
    t
}

/// A one-line inline table for `signal = { ... }`.
fn toml_inline(t: &toml::Table) -> String {
    format!("{}", toml::Value::Table(t.clone()))
}

const GRID: &str = "[grid]\ngrid_step_deg = 15.0\nduration_s = 3600.0\nstep_s = 300.0\n";

fn study(plans: &str, victims: &[&str], limit: &str) -> String {
    let v: Vec<String> = victims.iter().map(|v| format!("\"{v}\"")).collect();
    format!(
        "kind = \"pro-spectrum-coexistence\"\ntitle = \"test\"\n{limit}\nvictims = [{}]\n\n{GRID}\n{plans}",
        v.join(", ")
    )
}

const LIMIT: &str = "max_degradation_db = 1.0\nlimit_source = \"test limit\"";
const ALL: &[&str] = sc::OPEN_VICTIMS;

fn one_sat_plan(label: &str, signal: &str) -> String {
    format!(
        "[[plan]]\nlabel = \"{label}\"\nsignal = {signal}\nmask_deg = 10.0\n\n[[plan.satellite]]\naltitude_km = 1000.0\ninclination_deg = 53.0\n\n"
    )
}

const WALKER: &str = "[[plan.shell]]\npattern = \"delta\"\ntotal = 96\nplanes = 8\nphasing = 1\naltitude_km = 1000.0\ninclination_deg = 55.0\n";

fn walker_plan(label: &str, signal: &str) -> String {
    format!("[[plan]]\nlabel = \"{label}\"\nsignal = {signal}\nmask_deg = 10.0\n\n{WALKER}\n")
}

fn entry<'a>(r: &'a Value, plan: usize, dir: &str, gnss: &str) -> &'a Value {
    r["plans"][plan][dir]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["gnss"] == gnss)
        .unwrap_or_else(|| panic!("no {dir} entry for {gnss}"))
}

fn direct_compat(design: &toml::Table) -> Vec<Value> {
    let mut root = toml::Table::new();
    root.insert("kind".into(), toml::Value::String("leo-signal".into()));
    root.insert(
        "signals".into(),
        toml::Value::Array(vec![toml::Value::Table(design.clone())]),
    );
    let r = open(&toml::to_string(&root).unwrap());
    r["signals"][0]["compatibility"].as_array().unwrap().clone()
}

// ── 1. One satellite reproduces the free number ─────────────────────────────────────

#[test]
fn one_satellite_reproduces_the_free_single_satellite_loss_for_every_generic_design() {
    let mut compared = 0;
    for s in generic_designs() {
        let design = inline_design(&s, -150.0);
        let free = direct_compat(&design);
        let r = run(&study(
            &one_sat_plan("one", &toml_inline(&design)),
            ALL,
            LIMIT,
        ));
        for f in &free {
            let g = f["gnss"].as_str().unwrap();
            let e = entry(&r, 0, "caused", g);
            match f["gnss_cn0_degradation_db"].as_f64() {
                Some(expected) => {
                    let single = e["single_satellite_loss_db"].as_f64().unwrap();
                    assert!(
                        (single - expected).abs() < 1e-9,
                        "{g}: {single} vs {expected}"
                    );
                    // Only one satellite exists, so the worst cell has one in view and the
                    // upper bound at the reference power is the free number.
                    assert_eq!(e["worst_cell"]["satellites_in_view"].as_f64(), Some(1.0));
                    let worst = e["worst_loss_db"].as_f64().unwrap();
                    assert!(
                        (worst - expected).abs() < 1e-9,
                        "{g}: {worst} vs {expected}"
                    );
                    assert_eq!(
                        e["engine_single_satellite_loss_db"].as_f64(),
                        Some(expected)
                    );
                    compared += 1;
                }
                None => {
                    assert!(e["worst_loss_db"].is_null());
                    assert_eq!(e["reason"], "no spectral overlap");
                }
            }
        }
    }
    assert!(
        compared >= 4,
        "the generic L-band design overlaps the E5/L5 victims; compared {compared}"
    );
}

// ── 2. The published case, as a cross-check of what is read ─────────────────────────

#[test]
fn xona_x5_coefficients_are_the_ones_the_open_document_states() {
    let r = run(&study(&one_sat_plan("x5", "\"xona-x5\""), ALL, LIMIT));
    let ssc = |g: &str| entry(&r, 0, "caused", g)["ssc_db_per_hz"].as_f64().unwrap();
    let round1 = |x: f64| (x * 10.0).round() / 10.0;
    assert_eq!(round1(ssc("galileo-e5a")), -91.2);
    assert_eq!(round1(ssc("gps-l5")), -91.2);
    assert_eq!(round1(ssc("galileo-e5b")), -103.2);
    assert_eq!(round1(ssc("galileo-e5-altboc")), -86.2);
    // One satellite at the published minimum power raises the noise density by
    // 0.0001 to 0.005 dB (the open document's rounding).
    let losses: Vec<f64> = ["gps-l5", "galileo-e5a", "galileo-e5b", "galileo-e5-altboc"]
        .iter()
        .map(|g| {
            entry(&r, 0, "caused", g)["single_satellite_loss_db"]
                .as_f64()
                .unwrap()
        })
        .collect();
    let lo = losses.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = losses.iter().cloned().fold(0.0, f64::max);
    assert_eq!((lo * 1e4).round() / 1e4, 0.0001, "smallest {lo}");
    assert_eq!((hi * 1e3).round() / 1e3, 0.005, "largest {hi}");
    // And each equals the direct open run of the preset.
    let direct = open("kind = \"leo-signal\"\npresets = [\"xona-pulsar\"]\n");
    let x5 = direct["signals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "xona-x5")
        .unwrap();
    for c in x5["compatibility"].as_array().unwrap() {
        let g = c["gnss"].as_str().unwrap();
        assert_eq!(
            entry(&r, 0, "caused", g)["ssc_db_per_hz"],
            c["ssc_leo_into_gnss_db_per_hz"]
        );
    }
    // The design states a maximum received power; the bound uses it.
    assert_eq!(
        r["plans"][0]["signal"]["max_power_dbw"].as_f64(),
        x5["received_power_max_dbw"].as_f64()
    );
}

// ── 3. Aggregation identity ─────────────────────────────────────────────────────────

#[test]
fn n_in_view_gives_ten_log_one_plus_n_r() {
    let l = generic_designs()
        .into_iter()
        .find(|s| s["name"] == "generic-l")
        .unwrap();
    let design = inline_design(&l, -150.0);
    let r = run(&study(&walker_plan("L", &toml_inline(&design)), ALL, LIMIT));
    let p = &r["plans"][0];
    let counts = p["constellation"]["grid_max_in_view"].as_array().unwrap();
    let mut r_max_victim: f64 = 0.0;
    let mut checked = 0;
    for e in p["caused"].as_array().unwrap() {
        let Some(l1) = e["single_satellite_loss_db"].as_f64() else {
            continue;
        };
        let rr = 10f64.powf(l1 / 10.0) - 1.0; // r from the single-satellite loss
        r_max_victim = r_max_victim.max(rr);
        let n = e["worst_cell"]["satellites_in_view"].as_f64().unwrap();
        assert!(n > 1.0);
        let expected = 10.0 * (1.0 + n * rr).log10();
        assert!((e["worst_loss_db"].as_f64().unwrap() - expected).abs() < 1e-10);
        checked += 1;
    }
    assert!(checked >= 4);
    // The map holds, per cell, the largest victim's 10 log10(1 + N r).
    for (i, row) in p["map_loss_db"].as_array().unwrap().iter().enumerate() {
        for (j, v) in row.as_array().unwrap().iter().enumerate() {
            let n = counts[i][j].as_f64().unwrap();
            if n == 0.0 {
                assert!(v.is_null());
            } else {
                let expected = 10.0 * (1.0 + n * r_max_victim).log10();
                assert!(
                    (v.as_f64().unwrap() - expected).abs() < 1e-10,
                    "cell {i},{j}"
                );
            }
        }
    }
}

// ── 4. Scaling ──────────────────────────────────────────────────────────────────────

#[test]
fn doubling_satellites_or_raising_power_by_3_0103_db_raises_the_ratio_by_3_0103_db() {
    let l = generic_designs()
        .into_iter()
        .find(|s| s["name"] == "generic-l")
        .unwrap();
    let base = inline_design(&l, -150.0);
    let sat = "[[plan.satellite]]\naltitude_km = 1000.0\ninclination_deg = 53.0\n";
    let plans = format!(
        "[[plan]]\nlabel = \"one\"\nsignal = {s}\nmask_deg = 10.0\n{sat}\n\
         [[plan]]\nlabel = \"two\"\nsignal = {s}\nmask_deg = 10.0\n{sat}{sat}\n",
        s = toml_inline(&base)
    );
    let r = run(&study(&plans, &["galileo-e5a"], LIMIT));
    let inr = |p: usize| {
        entry(&r, p, "caused", "galileo-e5a")["worst_inr_db"]
            .as_f64()
            .unwrap()
    };
    assert!(
        (inr(1) - inr(0) - DB2).abs() < 1e-9,
        "{} {}",
        inr(0),
        inr(1)
    );

    let mut up = base.clone();
    up.insert(
        "received_power_dbw".into(),
        toml::Value::Float(-150.0 + 3.0103),
    );
    let plans = format!(
        "{}{}",
        one_sat_plan("base", &toml_inline(&base)),
        one_sat_plan("up", &toml_inline(&up))
    );
    let r = run(&study(&plans, &["galileo-e5a"], LIMIT));
    let inr = |p: usize| {
        entry(&r, p, "caused", "galileo-e5a")["worst_inr_db"]
            .as_f64()
            .unwrap()
    };
    assert!((inr(1) - inr(0) - 3.0103).abs() < 1e-9);
    let single = |p: usize| {
        entry(&r, p, "caused", "galileo-e5a")["single_satellite_inr_db"]
            .as_f64()
            .unwrap()
    };
    assert!((single(1) - single(0) - 3.0103).abs() < 1e-9);
}

// ── 5. Verdict against the limit ────────────────────────────────────────────────────

#[test]
fn a_limit_just_above_the_worst_loss_passes_and_just_below_fails() {
    let plans = walker_plan("L", "\"generic-l\"");
    let r = run(&study(&plans, ALL, LIMIT));
    let w = r["plans"][0]["worst_loss_caused_db"].as_f64().unwrap();
    assert!(w > 0.0);
    let at = |lim: f64| {
        run(&study(
            &plans,
            ALL,
            &format!("max_degradation_db = {lim:e}\nlimit_source = \"test limit\""),
        ))
    };
    let above = at(w * (1.0 + 1e-6));
    assert_eq!(above["plans"][0]["verdict_caused"]["status"], "compatible");
    let below = at(w * (1.0 - 1e-6));
    assert_eq!(
        below["plans"][0]["verdict_caused"]["status"],
        "not compatible"
    );
    let e = below["plans"][0]["caused"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["verdict"]["status"] == "not compatible")
        .expect("a failing victim");
    assert!(e["verdict"]["margin_db"].as_f64().unwrap() < 0.0);
}

#[test]
fn a_study_without_a_limit_or_its_source_is_an_error() {
    let plans = one_sat_plan("L", "\"generic-l\"");
    let msg = run_err(&study(&plans, ALL, "limit_source = \"x\""));
    assert!(msg.contains("max_degradation_db"), "{msg}");
    let msg = run_err(&study(&plans, ALL, "max_degradation_db = 1.0"));
    assert!(msg.contains("limit_source"), "{msg}");
    let msg = run_err(&study(
        &plans,
        ALL,
        "max_degradation_db = 1.0\nlimit_source = \"  \"",
    ));
    assert!(msg.contains("limit_source"), "{msg}");
}

// ── 6. No overlap ───────────────────────────────────────────────────────────────────

#[test]
fn a_c_band_design_has_no_spectral_overlap_with_the_l_band_victims() {
    let r = run(&study(
        &walker_plan("C", "\"generic-c-band-leo\""),
        ALL,
        LIMIT,
    ));
    let p = &r["plans"][0];
    for e in p["caused"].as_array().unwrap() {
        assert_eq!(e["spectral_overlap"], false);
        assert!(e["ssc_db_per_hz"].is_null());
        assert!(e["single_satellite_loss_db"].is_null());
        assert!(e["worst_loss_db"].is_null(), "null, never zero");
        assert_eq!(e["reason"], "no spectral overlap");
        assert_eq!(e["verdict"]["status"], "compatible");
        assert!(e["verdict"]["basis"]
            .as_str()
            .unwrap()
            .contains("no spectral overlap"));
        assert!(e["verdict"]["margin_db"].is_null());
    }
    assert!(p["worst_loss_caused_db"].is_null());
    assert!(p["map_loss_db"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .all(Value::is_null));
    // The open run agrees: it states no coefficient into any of these victims.
    let direct = open("kind = \"leo-signal\"\npresets = [\"generic-c-band\"]\n");
    for c in direct["signals"][0]["compatibility"].as_array().unwrap() {
        assert!(c["ssc_leo_into_gnss_db_per_hz"].is_null());
    }
}

// ── 7. The numbers in view are the constellation-design grid ─────────────────────────

#[test]
fn numbers_in_view_equal_the_constellation_design_grid() {
    let r = run(&study(
        &walker_plan("W", "\"generic-l\""),
        &["gps-l5", "galileo-e5a"],
        LIMIT,
    ));
    let direct = open(&format!(
        "kind = \"constellation-design\"\nduration_s = 3600.0\nstep_s = 300.0\nmask_deg = 10.0\ngrid_step_deg = 15.0\n\
         [[constellation]]\nname = \"W\"\n{}",
        WALKER.replace("[[plan.shell]]", "[[constellation.shell]]")
    ));
    let c = &r["plans"][0]["constellation"];
    assert_eq!(c["grid_max_in_view"], direct["grid"]["max_visible"]);
    assert_eq!(c["grid_mean_in_view"], direct["grid"]["mean_visible"]);
    assert_eq!(c["total_satellites"], direct["total_satellites"]);
    assert_eq!(r["grid"]["lat_deg"], direct["grid"]["lat_deg"]);
    assert_eq!(r["grid"]["lon_deg"], direct["grid"]["lon_deg"]);
    // The reverse direction's GNSS counts are the open presets' own grids.
    for (system, preset) in [("GPS", "gps-baseline"), ("Galileo", "galileo")] {
        let d = open(&format!(
            "kind = \"constellation-design\"\nduration_s = 3600.0\nstep_s = 300.0\nmask_deg = 10.0\ngrid_step_deg = 15.0\n\
             [[constellation]]\nname = \"{system}\"\npreset = \"{preset}\"\n"
        ));
        let g = r["gnss_constellations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["system"] == system)
            .unwrap();
        assert_eq!(g["grid_max_in_view"], d["grid"]["max_visible"]);
        assert_eq!(g["grid_mean_in_view"], d["grid"]["mean_visible"]);
    }
    // And the worst cell's count is the grid's largest.
    let most = direct["grid"]["max_visible"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .filter_map(Value::as_u64)
        .max()
        .unwrap();
    assert_eq!(
        entry(&r, 0, "caused", "gps-l5")["worst_cell"]["satellites_in_view"].as_f64(),
        Some(most as f64)
    );
}

// ── 8. Exclusions ───────────────────────────────────────────────────────────────────

#[test]
fn adversarial_tables_are_refused() {
    let plans = one_sat_plan("L", "\"generic-l\"");
    for table in [
        "jammers", "jammer", "spoofer", "spoofers", "attack", "threat",
    ] {
        let src = format!(
            "{}\n[[{table}]]\npower_dbw = -100.0\n",
            study(&plans, ALL, LIMIT)
        );
        let msg = run_err(&src);
        assert!(msg.contains("outside this pack"), "{table}: {msg}");
    }
    // Nested inside a plan too.
    let src = study(&format!("{plans}[[plan.jammers]]\nx = 1\n"), ALL, LIMIT);
    assert!(run_err(&src).contains("outside this pack"));
}

fn keys(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                out.push(k.clone());
                keys(v, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| keys(x, out)),
        _ => {}
    }
}

#[test]
fn the_result_carries_no_jammer_or_spoofing_field() {
    let r = run(&study(
        &format!(
            "{}{}",
            walker_plan("L", "\"generic-l\""),
            walker_plan("X", "\"xona-x5\"")
        ),
        ALL,
        LIMIT,
    ));
    let mut ks = vec![];
    keys(&r, &mut ks);
    // The units block is keyed by result path, so it is covered by the same walk.
    for k in &ks {
        let l = k.to_ascii_lowercase();
        assert!(
            !l.contains("jam") && !l.contains("js_") && !l.contains("spoof"),
            "key {k:?} is not allowed"
        );
    }
    let text = sc::to_json_string(&r).to_ascii_lowercase();
    assert!(!text.contains("needed to deny"));
}

#[test]
fn a_victim_outside_the_open_list_is_refused() {
    let plans = one_sat_plan("L", "\"generic-l\"");
    // GPS L2C is open since engine 0.41 (a preset and a GNSS victim), so it is no longer refused.
    for v in ["gps-p", "galileo-prs", "gps-m-code", "my-signal"] {
        let msg = run_err(&study(&plans, &[v], LIMIT));
        assert!(
            msg.contains("refused") && msg.contains("gps-l1ca"),
            "{v}: {msg}"
        );
    }
}

// ── 9. Determinism and licence ──────────────────────────────────────────────────────

#[test]
fn two_runs_are_byte_identical() {
    let src = study(
        &format!(
            "{}{}",
            walker_plan("L", "\"generic-l\""),
            walker_plan("C", "\"generic-c-band-leo\"")
        ),
        ALL,
        LIMIT,
    );
    let a = sc::to_json_string(&run(&src));
    let b = sc::to_json_string(&run(&src));
    assert_eq!(a, b);
    // The scenario hash is the SHA-256 of the canonical JSON of the parsed study.
    let t: toml::Table = toml::from_str(&src).unwrap();
    let canon = sc::canonical_json(&serde_json::to_value(&t).unwrap());
    let r: Value = serde_json::from_str(&a).unwrap();
    assert_eq!(
        r["scenario_hash"].as_str().unwrap(),
        sc::sha256_hex(canon.as_bytes())
    );
}

fn tmp(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("coexist-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_licence_gate_refuses_evaluation_and_runs_with_dev() {
    use kshana_pro::license::Entitlement;
    let src = study(&one_sat_plan("L", "\"generic-l\""), &["gps-l5"], LIMIT);
    let eval = Entitlement::evaluation();
    let e = kshana_pro::registry::run_gated(&src, &eval, 0).unwrap_err();
    assert!(e.to_string().contains("requires the"), "{e}");

    let dir = tmp("licence");
    let study_path = dir.join("study.toml");
    std::fs::write(&study_path, &src).unwrap();
    let bin = env!("CARGO_BIN_EXE_kshana-pro");
    let out_dir = dir.join("out");
    let no = Command::new(bin)
        .args([
            "coexist",
            study_path.to_str().unwrap(),
            "--out",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!no.status.success());
    assert!(String::from_utf8_lossy(&no.stderr).contains("requires the"));
    assert!(
        !out_dir.exists(),
        "nothing is written without an entitlement"
    );

    let yes = Command::new(bin)
        .args([
            "coexist",
            study_path.to_str().unwrap(),
            "--out",
            out_dir.to_str().unwrap(),
            "--dev",
        ])
        .output()
        .unwrap();
    assert!(
        yes.status.success(),
        "{}",
        String::from_utf8_lossy(&yes.stderr)
    );
    for f in sc::cli::OUTPUTS {
        assert!(out_dir.join(f).is_file(), "{f} missing");
    }
    let written = std::fs::read_to_string(out_dir.join("coexistence.result.json")).unwrap();
    assert_eq!(written, sc::to_json_string(&run(&src)));

    // `kshana-pro run` prints the same document.
    let printed = Command::new(bin)
        .args(["run", study_path.to_str().unwrap(), "--dev"])
        .output()
        .unwrap();
    assert!(
        printed.status.success(),
        "{}",
        String::from_utf8_lossy(&printed.stderr)
    );
    let stdout = String::from_utf8_lossy(&printed.stdout);
    assert!(
        stdout.contains(&written.trim_end().to_string()),
        "run prints the result JSON"
    );
}

// ── 10. Adversarial inputs ──────────────────────────────────────────────────────────

#[test]
fn an_empty_plan_list_is_an_error() {
    let msg = run_err(&study("", ALL, LIMIT));
    assert!(msg.contains("no candidate plan"), "{msg}");
    let src = study("", ALL, LIMIT).replace("[grid]", "plan = []\n\n[grid]");
    let msg = run_err(&src);
    assert!(msg.contains("no candidate plan"), "{msg}");
}

#[test]
fn a_nan_or_negative_limit_is_an_error() {
    let plans = one_sat_plan("L", "\"generic-l\"");
    for lim in ["nan", "inf", "-0.5"] {
        let msg = run_err(&study(
            &plans,
            ALL,
            &format!("max_degradation_db = {lim}\nlimit_source = \"x\""),
        ));
        assert!(msg.contains("max_degradation_db"), "{lim}: {msg}");
    }
}

#[test]
fn a_constellation_never_in_view_gives_a_null_loss_with_a_reason() {
    // An equatorial satellite at 200 km seen only within 0.03 deg of the zenith: no
    // cell centre of a 15 deg grid (latitudes 7.5, 22.5, ...) is ever that close.
    let plan = "[[plan]]\nlabel = \"never\"\nsignal = \"generic-l\"\nmask_deg = 89.0\n\n\
                [[plan.satellite]]\naltitude_km = 200.0\ninclination_deg = 0.0\n";
    let r = run(&study(plan, &["galileo-e5a"], LIMIT));
    let p = &r["plans"][0];
    assert_eq!(p["constellation"]["most_in_view"], 0);
    let e = entry(&r, 0, "caused", "galileo-e5a");
    assert_eq!(e["spectral_overlap"], true);
    assert!(e["worst_loss_db"].is_null());
    assert!(e["reason"].as_str().unwrap().contains("in view"));
    assert_eq!(e["verdict"]["status"], "compatible");
    assert!(p["worst_loss_caused_db"].is_null());
}

#[test]
fn malformed_plans_are_refused() {
    let base = |p: &str| run_err(&study(p, ALL, LIMIT));
    assert!(
        base("[[plan]]\nlabel = \"a\"\nsignal = \"generic-l\"\nmask_deg = 10.0\n")
            .contains("[[plan.shell]]")
    );
    assert!(base(&format!(
        "{}{}",
        one_sat_plan("a", "\"generic-l\""),
        one_sat_plan("a", "\"generic-l\"")
    ))
    .contains("twice"));
    assert!(base(&one_sat_plan("a", "\"no-such-signal\"")).contains("no signal named"));
    assert!(base(
        &one_sat_plan("a", "\"generic-l\"").replace("mask_deg = 10.0", "mask_deg = 90.0")
    )
    .contains("mask_deg"));
    assert!(base(
        &one_sat_plan("a", "\"generic-l\"").replace("mask_deg = 10.0", "mask_deg = 10.0\ntypo = 1")
    )
    .contains("unknown key"));
    // A maximum received power below the design's reference power is refused.
    let msg = base(&one_sat_plan("a", "\"xona-x5\"").replace(
        "mask_deg = 10.0",
        "mask_deg = 10.0\nreceived_power_max_dbw = -200.0",
    ));
    assert!(msg.contains("below the reference power"), "{msg}");
}

#[test]
fn a_received_power_of_one_watt_or_more_is_refused() {
    // A received power at the user antenna of 0 dBW (1 W) or more is not a satellite
    // signal at a ground receiver: it is an EIRP, or a value in the wrong unit. Such a
    // value once ran to a 1118 dB "loss", and 4000 dBW overflowed to an infinite ratio
    // that the JSON wrote as null beside a "not compatible" verdict.
    let base = |p: &str| run_err(&study(p, ALL, LIMIT));
    for (key, v) in [
        ("received_power_dbw", "30.0"),
        ("received_power_dbw", "4000.0"),
        ("received_power_max_dbw", "0.0"),
        ("received_power_max_dbw", "1000.0"),
    ] {
        let msg = base(
            &one_sat_plan("a", "\"generic-l\"")
                .replace("mask_deg = 10.0", &format!("mask_deg = 10.0\n{key} = {v}")),
        );
        assert!(msg.contains("0 dBW (1 W) or more"), "{key} = {v}: {msg}");
    }
    // The guard also covers a power read from the design, but an inline design stating
    // such a power is not exercised here: the open leo-signal run itself does not return
    // for a stated received power above about -125 dBW (its acquisition detection
    // probability sum), so the study never reaches the guard.
}

// ── The result contract ─────────────────────────────────────────────────────────────

fn numeric_paths(v: &Value, path: String, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                numeric_paths(x, p, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                numeric_paths(x, format!("{path}[]"), out);
            }
        }
        Value::Number(_) => out.push(path),
        _ => {}
    }
}

#[test]
fn the_result_follows_the_pro_contract() {
    let r = run(&study(
        &format!(
            "{}{}",
            walker_plan("L", "\"generic-l\""),
            walker_plan("C", "\"generic-c-band-leo\"")
        ),
        ALL,
        LIMIT,
    ));
    assert_eq!(r["kind"], "pro-spectrum-coexistence");
    assert_eq!(r["overlay"], "kshana-pro");
    assert!(r["schema_version"].is_string());
    assert!(r["overlay_version"].is_string());
    assert_eq!(
        r["engine_version"].as_str(),
        open("kind = \"leo-signal\"\n")["engine_version"].as_str()
    );
    assert_eq!(r["scenario_hash"].as_str().unwrap().len(), 64);
    let label = r["label"].as_str().unwrap();
    assert!(label.starts_with("MODELLED"));
    assert!(label.contains("VALIDATED") && label.contains("upper bound"));
    assert!(r["bound"].as_str().unwrap().contains("upper bound"));
    assert!(!r["not_modelled"].as_array().unwrap().is_empty());
    for f in r["figure_tiers"]["figures"].as_array().unwrap() {
        assert_eq!(f["tier"], "MODELLED");
    }
    // Every numeric field has a units row.
    let mut paths = vec![];
    numeric_paths(&r, String::new(), &mut paths);
    paths.sort();
    paths.dedup();
    let units = r["units"].as_object().unwrap();
    for p in &paths {
        if p.starts_with("units.") {
            continue;
        }
        let u = units
            .get(p)
            .unwrap_or_else(|| panic!("no units row for {p}"));
        assert!(u["unit"].is_string() && u["provenance"].is_string() && u["note"].is_string());
    }
    // Members: every open run made, with SHA-256 of its scenario and result.
    let members = r["members"].as_array().unwrap();
    // Two plans × (signal + constellation) + GPS + Galileo coverage.
    assert_eq!(members.len(), 6);
    for m in members {
        assert!(["leo-signal", "constellation-design"].contains(&m["kind"].as_str().unwrap()));
        assert_eq!(m["scenario_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(m["result_sha256"].as_str().unwrap().len(), 64);
    }
    // The Pareto front comes from the open MCDA routine over the read objectives.
    let pts: Vec<Vec<f64>> = r["pareto"]["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            vec![
                p["ranging_accuracy_m"].as_f64().unwrap(),
                p["loss_for_ranking_db"].as_f64().unwrap(),
                p["total_satellites"].as_f64().unwrap(),
            ]
        })
        .collect();
    use kshana::mcda::Objective;
    let front = kshana::mcda::pareto::pareto_front(&pts, &[Objective::Min; 3]);
    let labels: Vec<&str> = front
        .iter()
        .map(|&i| r["pareto"]["points"][i]["plan"].as_str().unwrap())
        .collect();
    let got: Vec<&str> = r["pareto"]["front"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(got, labels);
    // Ranging accuracy is the open run's.
    let direct = open("kind = \"leo-signal\"\npresets = [\"generic-bands\"]\n");
    let l = direct["signals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "generic-l")
        .unwrap();
    assert_eq!(
        r["plans"][0]["signal"]["ranging_accuracy_m"],
        l["tracking"]["ranging_accuracy_m"]
    );
}

#[test]
fn the_preset_list_is_the_one_the_open_engine_reports() {
    let e = kshana::api::run_scenario("kind = \"leo-signal\"\npresets = [\"no-such-preset\"]\n")
        .unwrap_err()
        .to_string();
    let list = e
        .split("the library has ")
        .nth(1)
        .expect("the engine names its presets");
    let names: Vec<&str> = list.trim().split(", ").map(str::trim).collect();
    assert_eq!(names, LEO_SIGNAL_PRESET_FILES);
}

#[test]
fn the_showcase_study_runs_on_the_generic_designs_only() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scenarios/pro-power/coexistence/generic-walker-coexistence.toml");
    let src = std::fs::read_to_string(&p).expect("the showcase study exists");
    let t: toml::Table = toml::from_str(&src).unwrap();
    for plan in t["plan"].as_array().unwrap() {
        let s = plan["signal"].as_str().expect("a preset signal name");
        assert!(
            s.starts_with("generic-"),
            "showcase uses only generic designs: {s}"
        );
    }
    let r = run(&src);
    assert!(r["label"].as_str().unwrap().starts_with("MODELLED"));
    assert!(r["plans"].as_array().unwrap().len() >= 3);
}

#[test]
fn no_file_of_this_track_mentions_the_isolated_workshop_preset() {
    let needle = ["cel", "este"].concat();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<PathBuf> = vec![
        root.join("src/packs/spectrum_coexistence_pack.rs"),
        root.join("tests/spectrum_coexistence.rs"),
        root.join("docs/SPECTRUM-COEXISTENCE.md"),
    ];
    for dir in [
        "src/spectrum_coexistence",
        "scenarios/pro-power/coexistence",
    ] {
        for e in std::fs::read_dir(root.join(dir)).unwrap() {
            files.push(e.unwrap().path());
        }
    }
    for f in files {
        let text =
            std::fs::read_to_string(&f).unwrap_or_else(|_| panic!("{} readable", f.display()));
        assert!(
            !text.to_ascii_lowercase().contains(&needle),
            "{} mentions the isolated preset",
            f.display()
        );
    }
}

// ── F8b. Other systems in the band ──────────────────────────────────────────────────

#[test]
fn an_identical_other_system_doubles_the_ratio_and_takes_half() {
    let l = generic_designs()
        .into_iter()
        .find(|s| s["name"] == "generic-l")
        .unwrap();
    let design = toml_inline(&inline_design(&l, -150.0));
    let alone = run(&study(&walker_plan("L", &design), ALL, LIMIT));
    let other = walker_plan("Incumbent", &design)
        .replace("[[plan]]", "[[other_system]]")
        .replace("[[plan.shell]]", "[[other_system.shell]]");
    let both = run(&study(&(walker_plan("L", &design) + &other), ALL, LIMIT));
    // The other system is not a candidate: one plan, one entry under other_systems.
    assert_eq!(both["plans"].as_array().unwrap().len(), 1);
    assert_eq!(both["other_systems"][0]["label"], "Incumbent");
    assert_eq!(both["summary"]["plans"], 1);
    // The plan alone is unchanged by the other system's presence.
    assert_eq!(alone["plans"][0]["caused"], both["plans"][0]["caused"]);
    let w = &both["plans"][0]["with_other_systems"];
    assert_eq!(w["systems"][0], "Incumbent");
    let mut checked = 0;
    for e in w["victims"].as_array().unwrap() {
        let solo = entry(&alone, 0, "caused", e["gnss"].as_str().unwrap());
        let Some(solo_loss) = solo["worst_loss_db"].as_f64() else {
            assert!(e["worst_loss_db"].is_null());
            continue;
        };
        // Same satellites, same cells: the summed ratio is twice the plan's.
        let r = 10f64.powf(solo_loss / 10.0) - 1.0;
        let expected = 10.0 * (1.0 + 2.0 * r).log10();
        assert!(
            (e["worst_loss_db"].as_f64().unwrap() - expected).abs() < 1e-9,
            "{e}"
        );
        assert!((e["plan_share_at_worst"].as_f64().unwrap() - 0.5).abs() < 1e-12);
        assert!(
            (e["worst_inr_db"].as_f64().unwrap() - (solo["worst_inr_db"].as_f64().unwrap() + DB2))
                .abs()
                < 1e-9
        );
        checked += 1;
    }
    assert!(checked >= 4);
    // A limit between the plan's own worst loss and the summed one: the plan alone
    // passes, with the incumbent it does not.
    let worst_alone = alone["plans"][0]["worst_loss_caused_db"].as_f64().unwrap();
    let limit = format!(
        "max_degradation_db = {}\nlimit_source = \"test limit\"",
        worst_alone + 1e-6
    );
    let tight = run(&study(&(walker_plan("L", &design) + &other), ALL, &limit));
    assert_eq!(tight["plans"][0]["verdict_caused"]["status"], "compatible");
    assert_eq!(
        tight["plans"][0]["with_other_systems"]["verdict"]["status"],
        "not compatible"
    );
    // Labels are one set across plans and other systems.
    let dup = walker_plan("L", &design) + &other.replace("Incumbent", "L");
    assert!(run_err(&study(&dup, ALL, LIMIT)).contains("used twice"));
    // Without other systems the result has none and no plan carries the block.
    assert_eq!(alone["other_systems"].as_array().unwrap().len(), 0);
    assert!(alone["plans"][0].get("with_other_systems").is_none());
}
