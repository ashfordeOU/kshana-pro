// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Campaign watch: acceptance tests over real open-engine runs.
//!
//! Every expected value is read from a run made in the test (a campaign's own timeline,
//! a stand-alone run of one of its members, the ensemble statistics the engine reports),
//! never typed from a document, except where the test checks that a run still agrees
//! with a figure the open documentation states. Runs go through the `kshana-pro` binary
//! with `--dev` (the test entitlement path) or through `kshana_pro::registry::run`.
//! Working copies of packs live under the crate's `CARGO_TARGET_TMPDIR`.

use kshana_pro::pack_c::{self, CiReport};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const UTC: &str = "2026-09-30T00:00:00Z";
const BIN: &str = env!("CARGO_BIN_EXE_kshana-pro");

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("campaign-watch")
        .join(name);
    if d.exists() {
        std::fs::remove_dir_all(&d).expect("clear scratch dir");
    }
    std::fs::create_dir_all(&d).expect("create scratch dir");
    d
}

fn copy_pack(pack: &str, to: &Path) {
    for e in std::fs::read_dir(Path::new("scenarios/packs").join(pack)).expect("pack dir") {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).expect("copy pack file");
    }
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn kp(args: &[&str]) -> Out {
    let o = Command::new(BIN)
        .args(args)
        .output()
        .expect("run kshana-pro");
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

/// `kshana-pro ci --pack <dir>/pack.toml --dev --utc UTC --json <json> [extra...]`.
fn ci(dir: &Path, json: &Path, extra: &[&str]) -> (Out, CiReport) {
    let pack = dir.join("pack.toml");
    let mut args = vec![
        "ci",
        "--pack",
        pack.to_str().unwrap(),
        "--dev",
        "--utc",
        UTC,
        "--json",
        json.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    let out = kp(&args);
    let text = std::fs::read_to_string(json)
        .unwrap_or_else(|e| panic!("no report written ({e}); stderr:\n{}", out.stderr));
    let report = CiReport::from_json(&text).expect("report parses");
    (out, report)
}

fn outcome<'a>(r: &'a CiReport, id: &str) -> &'a pack_c::WatchOutcome {
    r.watch
        .as_ref()
        .expect("watch section")
        .outcomes
        .iter()
        .find(|o| o.id == id)
        .unwrap_or_else(|| panic!("no watch '{id}'"))
}

fn edit(path: &Path, from: &str, to: &str) {
    let s = std::fs::read_to_string(path).unwrap();
    assert!(
        s.contains(from),
        "{} does not contain {from:?}",
        path.display()
    );
    std::fs::write(path, s.replacen(from, to, 1)).unwrap();
}

fn run_doc(path: &str) -> Value {
    let src = std::fs::read_to_string(path).unwrap();
    let out = kshana_pro::registry::run(&src).expect("scenario runs");
    serde_json::from_str(&out.json).unwrap()
}

fn write_pack(dir: &Path, body: &str) {
    std::fs::write(dir.join("pack.toml"), body).unwrap();
}

// 1 ---------------------------------------------------------------------------------

/// A manifest with no watch tables runs exactly as it did at the branch point: the
/// committed fixture is the `ci.json` the branch-point binary (feat/pro-power 26d069a)
/// wrote for the resilience-baseline pack with the same `--utc`; this build must write
/// the same bytes, so the pack hash and every metric are unchanged.
#[test]
fn backward_compatible_with_the_branch_point() {
    let dir = scratch("compat");
    let json = dir.join("ci.json");
    let pack = "scenarios/packs/resilience-baseline/pack.toml";
    let out = kp(&[
        "ci",
        "--pack",
        pack,
        "--dev",
        "--utc",
        UTC,
        "--json",
        json.to_str().unwrap(),
    ]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let now = std::fs::read_to_string(&json).unwrap();
    let then = std::fs::read_to_string(
        "tests/fixtures/campaign-watch/resilience-baseline.branch-point.ci.json",
    )
    .unwrap();
    assert_eq!(
        now, then,
        "a watch-free pack must write the branch-point report byte for byte"
    );
    let manifest = pack_c::parse_pack(&std::fs::read_to_string(pack).unwrap()).unwrap();
    assert_eq!(
        manifest.pack_hash(),
        "762220a20d4b4ed52385c8dc4feea73f0c9ddd82d55d3b23b274fc3cf1d4464b"
    );
    assert!(
        !now.contains("\"watch\""),
        "no watch section for a watch-free pack"
    );
}

// 2 ---------------------------------------------------------------------------------

#[test]
fn figures_exclude_echoed_inputs() {
    let path = "scenarios/packs/spectrum-baseline/l-band-waterfall-jamming.toml";
    let src = std::fs::read_to_string(path).unwrap();
    let json = kshana_pro::registry::run(&src).unwrap().json;
    let legacy = pack_c::metrics::extract("l-band", &json).unwrap();
    let figs = pack_c::semantics::figures("l-band", &json).unwrap();
    eprintln!(
        "l-band-waterfall-jamming: {} metrics before (every numeric leaf), {} figures after",
        legacy.len(),
        figs.metrics.len()
    );
    for echoed in [
        "l-band::receiver.noise_figure_db",
        "l-band::waterfall.n_freq",
    ] {
        assert!(
            legacy.get(echoed).is_some(),
            "the legacy set holds {echoed}"
        );
        assert!(
            figs.metrics.get(echoed).is_none(),
            "{echoed} is not a figure"
        );
    }
    assert!(figs.metrics.len() < legacy.len());
    // Every figure carries its unit, and the band timeline figures are there.
    assert_eq!(figs.units.len(), figs.metrics.len());
    assert_eq!(
        figs.units["l-band::timeline.bands.0.min_cn0_dbhz"], "dB-Hz",
        "units come from the result's own units block"
    );
}

// 3 ---------------------------------------------------------------------------------

fn member_scenario(campaign: &toml::Value, phase: usize, run: usize) -> String {
    let s = &campaign["phases"][phase]["runs"][run]["scenario"];
    toml::to_string(s).unwrap()
}

#[test]
fn campaign_figures_equal_the_stand_alone_numbers() {
    let path = "scenarios/packs/campaign-baseline/campaign-jam-spoof-holdover-integrity.toml";
    let doc = run_doc(path);
    let figs = pack_c::campaign::figures(&doc);
    let get = |n: &str| {
        figs.iter()
            .find(|f| f.name == n)
            .map(|f| f.value)
            .unwrap_or_else(|| panic!("no campaign figure {n}"))
    };
    let campaign: toml::Value = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    // The spoofing phase ends at the stand-alone spoof run's detection time.
    let spoof: Value = serde_json::from_str(
        &kshana_pro::registry::run(&member_scenario(&campaign, 2, 0))
            .unwrap()
            .json,
    )
    .unwrap();
    let detect = spoof["classical"]["detect_time_s"].as_f64().unwrap();
    assert_eq!(get("phase.spoofing.duration_s"), detect);
    assert_eq!(detect, 370.0, "docs/CAMPAIGNS.md states 370 s");

    // The jamming phase's C/N0: the mean over the stand-alone jamming run's satellites
    // at its latest epoch at or before the phase's last grid time.
    let tl = &doc["timeline"];
    let t_s: Vec<f64> = tl["t_s"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let (t0, t1) = (
        tl["phases"][1]["t0_s"].as_f64().unwrap(),
        tl["phases"][1]["t1_s"].as_f64().unwrap(),
    );
    let last_grid = t_s
        .iter()
        .copied()
        .filter(|t| *t >= t0 && *t < t1)
        .fold(f64::NAN, f64::max);
    let jam: Value = serde_json::from_str(
        &kshana_pro::registry::run(&member_scenario(&campaign, 1, 1))
            .unwrap()
            .json,
    )
    .unwrap();
    let epoch = jam["epochs"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|e| e["t"].as_f64().unwrap() <= last_grid - t0 + 1e-9)
        .unwrap();
    let cn0: Vec<f64> = epoch["sats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["cn0_effective_dbhz"].as_f64().unwrap())
        .collect();
    let mean = cn0.iter().sum::<f64>() / cn0.len() as f64;
    let got = get("phase.jamming.end.cn0_dbhz");
    assert!((got - mean).abs() < 1e-9, "{got} vs stand-alone {mean}");
    assert_eq!(
        format!("{got:.1}"),
        "27.7",
        "docs/CAMPAIGNS.md states 27.7 dB-Hz"
    );

    // The time error at the end of holdover is the value the campaign's own timeline
    // holds at the phase's last grid time.
    let (h0, h1) = (
        tl["phases"][3]["t0_s"].as_f64().unwrap(),
        tl["phases"][3]["t1_s"].as_f64().unwrap(),
    );
    let k = t_s.iter().rposition(|t| *t >= h0 && *t < h1).unwrap();
    let held = tl["channels"]["time_error_ns"]["values"][k]
        .as_f64()
        .unwrap();
    assert_eq!(get("phase.holdover.end.time_error_ns"), held);
    eprintln!(
        "spoofing phase {detect} s; jamming-phase C/N0 {got} dB-Hz; holdover end time error {held} ns at t = {} s",
        t_s[k]
    );
}

// 4, 5 ------------------------------------------------------------------------------

#[test]
fn the_statistical_gate_tells_noise_from_change() {
    let dir = scratch("stats");
    copy_pack("campaign-baseline", &dir);
    let (out, base) = ci(&dir, &dir.join("base.json"), &[]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let mean_id = "clock final error, ensemble mean";
    let spread_id = "clock final error, ensemble spread";
    let b = outcome(&base, mean_id).ensemble.expect("ensemble stats");
    assert_eq!(b.n, 200.0);
    assert_eq!(
        format!("{:.2}", b.std),
        "14.93",
        "docs/CAMPAIGNS.md states 14.93 ns"
    );
    let lo = base
        .metrics
        .get("clock-ensemble::campaign.mc.final_error.ci95_low")
        .unwrap();
    let hi = base
        .metrics
        .get("clock-ensemble::campaign.mc.final_error.ci95_high")
        .unwrap();
    assert_eq!(format!("[{lo:.2}, {hi:.2}]"), "[-2.52, 1.59]");

    // Same physics, other seeds: not a regression although the mean moves a lot.
    let file = dir.join("campaign-monte-carlo-clock-holdover.toml");
    edit(
        &file,
        "[monte_carlo.scenario]\nseed = 42",
        "[monte_carlo.scenario]\nseed = 1042",
    );
    let base_path = dir.join("base.json");
    let (_, reseeded) = ci(
        &dir,
        &dir.join("reseeded.json"),
        &["--baseline", base_path.to_str().unwrap()],
    );
    let m = outcome(&reseeded, mean_id);
    let s = outcome(&reseeded, spread_id);
    let stat = m.stat.as_ref().expect("statistical verdict");
    eprintln!(
        "reseeded: mean {} -> {} (relative change {:+.1} %), Welch p = {:.4}; std {} -> {}, F p = {:.4}",
        b.mean,
        stat.current.mean,
        m.rel_change.unwrap() * 100.0,
        stat.p_mean,
        b.std,
        stat.current.std,
        stat.p_spread
    );
    assert!(
        m.rel_change.unwrap().abs() > 0.5,
        "the mean moved by a large relative amount"
    );
    assert_ne!(m.status, "regressed");
    assert_ne!(s.status, "regressed");
    assert!(stat.p_mean >= 0.01 && stat.alpha == 0.01);

    // q_wf times four: the closed-form std doubles (16.459 ns to 32.918 ns).
    edit(&file, "seed = 1042", "seed = 42");
    let text = std::fs::read_to_string(&file).unwrap();
    let marker = "[monte_carlo.scenario.clock_classical]";
    let at = text.find(marker).unwrap();
    let (head, tail) = text.split_at(at);
    std::fs::write(
        &file,
        format!(
            "{head}{}",
            tail.replacen("q_wf = 9.0e-20", "q_wf = 3.6e-19", 1)
        ),
    )
    .unwrap();
    let (out, wider) = ci(
        &dir,
        &dir.join("wider.json"),
        &["--baseline", base_path.to_str().unwrap()],
    );
    let s = outcome(&wider, spread_id);
    let sv = s.stat.as_ref().unwrap();
    eprintln!(
        "q_wf x4: std {} -> {} (ratio {:.3}), F = {:.3}, p = {:.3e}",
        sv.baseline.std,
        sv.current.std,
        sv.current.std / sv.baseline.std,
        sv.f,
        sv.p_spread
    );
    assert_eq!(s.status, "regressed");
    assert!(sv.spread_regressed);
    let ratio = sv.current.std / sv.baseline.std;
    assert!((1.6..2.5).contains(&ratio), "{ratio}");
    assert_eq!(out.code, 1);
}

// 6 ---------------------------------------------------------------------------------

#[test]
fn direction_decides_regression_and_improvement() {
    let dir = scratch("direction");
    std::fs::copy(
        "scenarios/packs/constellation-baseline/lunar-relay-constellation.toml",
        dir.join("lunar.toml"),
    )
    .unwrap();
    write_pack(
        &dir,
        "id = \"d\"\nversion = \"1\"\n[[entries]]\npath = \"lunar.toml\"\nlabel = \"lunar\"\n\
         [[watch]]\nentry = \"lunar\"\nmetric = \"global.availability_pct\"\ndirection = \"higher\"\ntolerance = { abs = 0.0 }\n",
    );
    let (_, base) = ci(&dir, &dir.join("a.json"), &[]);
    edit(
        &dir.join("lunar.toml"),
        "pdop_threshold = 6.0",
        "pdop_threshold = 4.0",
    );
    let a = dir.join("a.json");
    let (out, dropped) = ci(
        &dir,
        &dir.join("b.json"),
        &["--baseline", a.to_str().unwrap()],
    );
    let o = outcome(&dropped, "lunar::global.availability_pct");
    eprintln!(
        "availability {} -> {}",
        o.baseline.unwrap(),
        o.value.unwrap()
    );
    assert!(o.value.unwrap() < o.baseline.unwrap());
    assert_eq!(o.status, "regressed");
    assert_eq!(out.code, 1);
    assert_eq!(
        o.baseline,
        outcome(&base, "lunar::global.availability_pct").value
    );
    edit(
        &dir.join("lunar.toml"),
        "pdop_threshold = 4.0",
        "pdop_threshold = 6.0",
    );
    let b = dir.join("b.json");
    let (out, rose) = ci(
        &dir,
        &dir.join("c.json"),
        &["--baseline", b.to_str().unwrap()],
    );
    let o = outcome(&rose, "lunar::global.availability_pct");
    assert_eq!(o.status, "improved");
    assert_eq!(out.code, 0);
}

// 7 ---------------------------------------------------------------------------------

#[test]
fn an_injected_regression_on_the_leo_chain_fails_the_pipeline() {
    let dir = scratch("leo");
    copy_pack("leo-pnt-baseline", &dir);
    let (out, _) = ci(&dir, &dir.join("base.json"), &[]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let file = dir.join("leo-pnt-end-to-end.toml");
    edit(&file, "od_sisre_m = 0.25", "od_sisre_m = 0.50");
    let base = dir.join("base.json");
    let (out, r) = ci(
        &dir,
        &dir.join("new.json"),
        &["--baseline", base.to_str().unwrap()],
    );
    let o = outcome(&r, "generic: signal-in-space range error handed on");
    let (b, v) = (o.baseline.unwrap(), o.value.unwrap());
    eprintln!("navmsg.sisre_rms_m {b} -> {v}");
    assert_eq!(format!("{b:.4}"), "0.2500");
    assert_eq!(format!("{v:.4}"), "0.5000");
    let standalone = run_doc(file.to_str().unwrap())["navmsg"]["sisre_rms_m"]
        .as_f64()
        .unwrap();
    assert_eq!(v, standalone);
    assert_eq!(o.status, "regressed");
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("WATCH FAILURES"), "{}", out.stderr);
}

// 8 ---------------------------------------------------------------------------------

#[test]
fn requirements_are_absolute_and_unit_checked() {
    let dir = scratch("requirements");
    for f in ["gps-baseline-sps.toml", "lunar-relay-constellation.toml"] {
        std::fs::copy(
            Path::new("scenarios/packs/constellation-baseline").join(f),
            dir.join(f),
        )
        .unwrap();
    }
    write_pack(
        &dir,
        "id = \"r\"\nversion = \"1\"\n\
         [[entries]]\npath = \"gps-baseline-sps.toml\"\nlabel = \"gps\"\n\
         [[entries]]\npath = \"lunar-relay-constellation.toml\"\nlabel = \"lunar\"\n\
         [[watch]]\nname = \"gps\"\nentry = \"gps\"\nmetric = \"global.availability_pct\"\ndirection = \"higher\"\nrequirement = \"global.availability_pct >= 99 %\"\n\
         [[watch]]\nname = \"lunar\"\nentry = \"lunar\"\nmetric = \"global.availability_pct\"\ndirection = \"higher\"\nrequirement = \"global.availability_pct >= 99 %\"\n",
    );
    let (out, r) = ci(&dir, &dir.join("r.json"), &[]);
    let gps = outcome(&r, "gps");
    let lunar = outcome(&r, "lunar");
    assert_eq!(gps.value, Some(100.0));
    assert_eq!(gps.requirement.as_ref().unwrap().verdict, "met");
    let lunar_value =
        run_doc("scenarios/packs/constellation-baseline/lunar-relay-constellation.toml")["global"]
            ["availability_pct"]
            .as_f64()
            .unwrap();
    assert_eq!(lunar.value, Some(lunar_value));
    assert_eq!(format!("{lunar_value:.4}"), "21.3286");
    assert_eq!(lunar.requirement.as_ref().unwrap().verdict, "not met");
    assert_eq!(out.code, 1, "an unmet requirement fails with no baseline");

    // A criterion in the wrong unit is not verified, never met, and fails the run.
    write_pack(
        &dir,
        "id = \"r\"\nversion = \"1\"\n[[entries]]\npath = \"gps-baseline-sps.toml\"\nlabel = \"gps\"\n\
         [[watch]]\nname = \"gps\"\nentry = \"gps\"\nmetric = \"global.availability_pct\"\nrequirement = \"global.availability_pct >= 99 m\"\n",
    );
    let (out, r) = ci(&dir, &dir.join("unit.json"), &[]);
    let req = outcome(&r, "gps").requirement.clone().unwrap();
    assert_eq!(req.verdict, "not verified");
    assert!(req.reason.unwrap().contains("unit mismatch"));
    assert_eq!(out.code, 1);
}

// 9 ---------------------------------------------------------------------------------

#[test]
fn history_chains_verifies_names_a_tampered_record_and_feeds_the_trend() {
    let dir = scratch("history");
    copy_pack("spectrum-baseline", &dir);
    let hist = dir.join("history.jsonl");
    let trend = dir.join("trend.html");
    let mut prev: Option<PathBuf> = None;
    for i in 0..3 {
        let json = dir.join(format!("run{i}.json"));
        let mut extra = vec![
            "--history",
            hist.to_str().unwrap(),
            "--trend",
            trend.to_str().unwrap(),
        ];
        let base;
        if let Some(p) = &prev {
            base = p.to_str().unwrap().to_string();
            extra.push("--baseline");
            extra.push(&base);
        }
        let (out, _) = ci(&dir, &json, &extra);
        assert_eq!(out.code, 0, "{}", out.stderr);
        prev = Some(json);
    }
    let v = kp(&["ci", "history", "verify", hist.to_str().unwrap(), "--dev"]);
    assert_eq!(v.code, 0, "{}{}", v.stdout, v.stderr);
    assert!(
        v.stdout.contains("3 record(s), chain intact"),
        "{}",
        v.stdout
    );

    let html = std::fs::read_to_string(&trend).unwrap();
    let pack =
        pack_c::parse_pack(&std::fs::read_to_string(dir.join("pack.toml")).unwrap()).unwrap();
    for w in &pack.watches {
        let id = w
            .id()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        assert!(html.contains(&format!("<h2>{id}</h2>")), "trend lists {id}");
    }
    assert_eq!(
        html.matches("<circle").count(),
        3 * pack.watches.len(),
        "one point per run for every watched figure"
    );
    let tj: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("trend.json")).unwrap()).unwrap();
    for f in tj["figures"].as_array().unwrap() {
        assert_eq!(f["points"].as_array().unwrap().len(), 3);
    }

    // Change one byte of the second record.
    let text = std::fs::read_to_string(&hist).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let pos = lines[1].find("\"verdict\":\"pass\"").unwrap() + "\"verdict\":\"".len();
    let mut bytes = lines[1].as_bytes().to_vec();
    bytes[pos] = b'q';
    let tampered = format!(
        "{}\n{}\n{}\n",
        lines[0],
        String::from_utf8(bytes).unwrap(),
        lines[2]
    );
    std::fs::write(&hist, tampered).unwrap();
    let v = kp(&["ci", "history", "verify", hist.to_str().unwrap(), "--dev"]);
    assert_eq!(v.code, 1);
    assert!(v.stdout.contains("record 1 (line 2)"), "{}", v.stdout);
    // And a run refuses to append to the broken history.
    let pack_path = dir.join("pack.toml");
    let out = kp(&[
        "ci",
        "--pack",
        pack_path.to_str().unwrap(),
        "--dev",
        "--utc",
        UTC,
        "--history",
        hist.to_str().unwrap(),
    ]);
    assert_eq!(out.code, 1);
    assert!(out.stderr.contains("record 1 (line 2)"), "{}", out.stderr);
}

// 10, 11 ----------------------------------------------------------------------------

#[test]
fn exact_and_diff_report_exactly_what_changed() {
    let dir = scratch("exact");
    copy_pack("spectrum-baseline", &dir);
    let (_, a) = ci(&dir, &dir.join("a.json"), &["--exact"]);
    let ap = dir.join("a.json");
    let (out, b) = ci(
        &dir,
        &dir.join("b.json"),
        &["--exact", "--baseline", ap.to_str().unwrap()],
    );
    let ex = b.watch.as_ref().unwrap().exact.clone().unwrap();
    assert!(ex.compared);
    assert!(
        ex.changed.is_empty(),
        "two runs of one engine: {:?}",
        ex.changed
    );
    assert_eq!(ex.unchanged.len(), 2);
    assert!(
        out.stderr.contains("exact: 0 result(s) changed"),
        "{}",
        out.stderr
    );
    assert_eq!(a.metrics, b.metrics);

    // Edit one scenario: exactly that entry changes.
    edit(
        &dir.join("l-band-waterfall-jamming.toml"),
        "noise_figure_db = 2.0",
        "noise_figure_db = 2.5",
    );
    let (out, c) = ci(
        &dir,
        &dir.join("c.json"),
        &["--exact", "--baseline", ap.to_str().unwrap()],
    );
    let ex = c.watch.as_ref().unwrap().exact.clone().unwrap();
    assert_eq!(ex.changed, vec!["l-band".to_string()]);
    assert_eq!(ex.unchanged, vec!["multi-band".to_string()]);
    assert!(out.stderr.contains("exact: 1 result(s) changed: l-band"));

    // `ci diff` lists the figures that moved and no others.
    let cp = dir.join("c.json");
    let dj = dir.join("diff.json");
    let d = kp(&[
        "ci",
        "diff",
        ap.to_str().unwrap(),
        cp.to_str().unwrap(),
        "--json",
        dj.to_str().unwrap(),
        "--dev",
    ]);
    assert_eq!(d.code, 0, "{}", d.stderr);
    let diff: Value = serde_json::from_str(&std::fs::read_to_string(&dj).unwrap()).unwrap();
    let listed: Vec<String> = diff["moved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["key"].as_str().unwrap().to_string())
        .collect();
    let mut expected: Vec<String> = a
        .metrics
        .0
        .iter()
        .filter(|(k, v)| c.metrics.get(k) != Some(**v))
        .map(|(k, _)| k.clone())
        .chain(
            c.metrics
                .keys()
                .filter(|k| a.metrics.get(k).is_none())
                .cloned(),
        )
        .collect();
    expected.sort();
    assert!(!expected.is_empty());
    assert_eq!(listed, expected);
    for m in diff["moved"].as_array().unwrap() {
        assert!(m["key"].as_str().unwrap().starts_with("l-band::"));
        assert_eq!(m["result_sha256_changed"], Value::Bool(true));
    }
    eprintln!("ci diff: {} figure(s) moved:\n{}", listed.len(), d.stdout);
}

// 12 --------------------------------------------------------------------------------

#[test]
fn every_bundled_pack_passes_first_and_reproduces_byte_for_byte() {
    for pack in [
        "leo-pnt-baseline",
        "constellation-baseline",
        "spectrum-baseline",
        "campaign-baseline",
        "solar-system-baseline",
    ] {
        let dir = scratch(&format!("pack-{pack}"));
        let manifest = format!("scenarios/packs/{pack}/pack.toml");
        let run = |name: &str| {
            let json = dir.join(name);
            let out = kp(&[
                "ci",
                "--pack",
                &manifest,
                "--dev",
                "--utc",
                UTC,
                "--json",
                json.to_str().unwrap(),
            ]);
            (out, std::fs::read(&json).unwrap())
        };
        let (first, a) = run("first.json");
        assert_eq!(first.code, 0, "{pack}: {}", first.stderr);
        let (_, b) = run("second.json");
        assert_eq!(a, b, "{pack}: the second run must be byte-identical");
        let r = CiReport::from_json(std::str::from_utf8(&a).unwrap()).unwrap();
        let w = r.watch.unwrap();
        assert!(w.watched && w.passed);
        assert!(
            w.outcomes.iter().all(|o| o.value.is_some()),
            "{pack}: every watch resolves"
        );
        assert!(
            w.members.iter().all(|m| m.evidence != "not stated"),
            "{pack}: every member states an evidence label"
        );
    }
}

// 13 --------------------------------------------------------------------------------

#[test]
fn adversarial_manifests_and_histories() {
    let dir = scratch("adversarial");
    std::fs::copy(
        "scenarios/packs/solar-system-baseline/mars-orbit-pnt.toml",
        dir.join("m.toml"),
    )
    .unwrap();
    let head = "id = \"x\"\nversion = \"1\"\n[[entries]]\npath = \"m.toml\"\nlabel = \"m\"\n";
    let pack = dir.join("pack.toml");
    let run = |body: &str| {
        write_pack(&dir, body);
        kp(&[
            "ci",
            "--pack",
            pack.to_str().unwrap(),
            "--dev",
            "--utc",
            UTC,
        ])
    };
    let o = run(&format!(
        "{head}[[watch]]\nentry = \"ghost\"\nmetric = \"fom.median_pdop\"\n"
    ));
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("not a label"), "{}", o.stderr);

    let o = run(&format!("{head}[[watch]]\nentry = \"m\"\nmetric = \"fom.median_pdop\"\ntolerance = {{ rel = nan }}\n"));
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("finite"), "{}", o.stderr);

    let o = run(&format!(
        "{head}[[watch]]\nentry = \"m\"\nmetric = \"fom.no_such_figure\"\ndirection = \"lower\"\n"
    ));
    assert_eq!(o.code, 1, "a missing figure never passes");
    let r = CiReport::from_json(&o.stdout).unwrap();
    let w = &r.watch.as_ref().unwrap().outcomes[0];
    assert_eq!(w.status, "not-found");
    assert!(
        w.reason.as_ref().unwrap().starts_with("not found"),
        "{:?}",
        w.reason
    );
    assert!(o.stderr.contains("not-found"));

    // An empty history file is an empty, valid history, and a run appends record 0.
    let hist = dir.join("empty.jsonl");
    std::fs::write(&hist, "").unwrap();
    let v = kp(&["ci", "history", "verify", hist.to_str().unwrap(), "--dev"]);
    assert_eq!(v.code, 0);
    assert!(v.stdout.contains("empty (0 records)"), "{}", v.stdout);
    write_pack(
        &dir,
        &format!(
            "{head}[[watch]]\nentry = \"m\"\nmetric = \"fom.median_pdop\"\ndirection = \"lower\"\n"
        ),
    );
    let o = kp(&[
        "ci",
        "--pack",
        pack.to_str().unwrap(),
        "--dev",
        "--utc",
        UTC,
        "--history",
        hist.to_str().unwrap(),
    ]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let recs = pack_c::history::load(&hist).unwrap();
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].record.index, 0);
}

// Packs hygiene ---------------------------------------------------------------------

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                files_under(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

#[test]
fn packs_never_use_the_workshop_preset_and_match_their_provenance() {
    let mut files = Vec::new();
    for pack in [
        "leo-pnt-baseline",
        "constellation-baseline",
        "spectrum-baseline",
        "campaign-baseline",
        "solar-system-baseline",
    ] {
        files_under(&Path::new("scenarios/packs").join(pack), &mut files);
    }
    files_under(Path::new("scenarios/pro-power/watch-demo"), &mut files);
    assert!(files.len() > 15);
    for f in &files {
        let text = std::fs::read_to_string(f)
            .unwrap_or_default()
            .to_lowercase();
        assert!(
            !text.contains("celeste"),
            "{} names the workshop preset",
            f.display()
        );
    }
    // Every scenario listed in a PROVENANCE.md has the SHA-256 it states.
    use sha2::{Digest, Sha256};
    for f in files
        .iter()
        .filter(|f| f.file_name().unwrap() == "PROVENANCE.md")
    {
        let text = std::fs::read_to_string(f).unwrap();
        let dir = f.parent().unwrap();
        let mut checked = 0;
        for line in text.lines().filter(|l| l.starts_with("| `")) {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let name = cells[1].trim_matches('`');
            let sha = cells[3].trim_matches('`');
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert_eq!(
                hex::encode(Sha256::digest(&bytes)),
                sha,
                "{}: {name}",
                f.display()
            );
            checked += 1;
        }
        assert!(checked >= 1, "{}", f.display());
    }
}
