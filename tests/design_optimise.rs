// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The design optimiser, held to real runs of the open engine.
//!
//! Every expected value here is compared with a direct `kshana::api::run_scenario` call
//! made in the test, not only with a typed constant: the published anchor, the lunar
//! front against a brute-force front over scenarios the test writes itself, every front
//! design re-run stand-alone, the link-budget front against the closed form, and the
//! search kernel against an analytic problem with a known front.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use kshana::mcda::Objective;
use kshana_pro::design::evaluate::parallel_map;
use kshana_pro::design::front::{kernel_front, Fitness};
use kshana_pro::design::search::{nsga2, Config, Problem};
use kshana_pro::design::space::sha256_hex;
use kshana_pro::design::{run_study, Options, Study};
use serde_json::Value;

const THREADS: usize = 4;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/pro-power/design")
}

fn study(name: &str) -> String {
    std::fs::read_to_string(dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn run(src: &str, threads: usize) -> (String, Value) {
    let out = run_study(
        src,
        &Options {
            threads: Some(threads),
        },
    )
    .unwrap_or_else(|e| panic!("the study runs: {e}"));
    let v = serde_json::from_str(&out.json).expect("the result is JSON");
    (out.json, v)
}

/// One direct run of the open engine: the result document and its SHA-256.
fn direct(text: &str) -> (Value, String) {
    let out = kshana::api::run_scenario(text).unwrap_or_else(|e| panic!("direct run: {e}"));
    (
        serde_json::from_str(&out.json).expect("engine JSON"),
        sha256_hex(out.json.as_bytes()),
    )
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("{v} is not a number"))
}

fn tmp(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("design-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The whole-Moon study, run once for the tests that read it.
fn lunar() -> &'static (String, Value) {
    static RUN: OnceLock<(String, Value)> = OnceLock::new();
    RUN.get_or_init(|| run(&study("lunar-navigation-global.toml"), THREADS))
}

// The lunar design space, written out by hand (not read from the study file).
const TOTALS: [u64; 6] = [8, 12, 14, 16, 20, 24];
const PLANES: [u64; 6] = [2, 3, 4, 6, 7, 8];
const PHASINGS: [u64; 2] = [0, 1];
const ALTITUDES: [f64; 4] = [1500.0, 3000.0, 5000.0, 8000.0];
const INCLINATIONS: [f64; 4] = [45.0, 60.0, 75.0, 90.0];

/// A lunar Walker scenario written by the test itself, not by the evaluator.
fn lunar_text(t: u64, p: u64, f: u64, h: f64, i: f64) -> String {
    format!(
        "kind = \"constellation-design\"\nbody = \"moon\"\nduration_s = 86400.0\nstep_s = 1800.0\nmask_deg = 10.0\npdop_threshold = 6.0\ngrid_step_deg = 10.0\nclock = \"common\"\ntrack_points = 3\nmax_tracks = 1\n[[constellation]]\nname = \"Lunar navigation shell\"\n[[constellation.shell]]\npattern = \"delta\"\ntotal = {t}\nplanes = {p}\nphasing = {f}\naltitude_km = {h:.1}\ninclination_deg = {i:.1}\n"
    )
}

type Key = (u64, u64, u64, u64, u64);

fn key_of(vars: &Value) -> Key {
    (
        vars["total"].as_u64().unwrap(),
        vars["planes"].as_u64().unwrap(),
        vars["phasing"].as_u64().unwrap(),
        num(&vars["altitude_km"]) as u64,
        num(&vars["inclination_deg"]) as u64,
    )
}

/// True when `a` (satellites, availability) dominates `b`: fewer or equal satellites,
/// higher or equal availability, and strictly better on one.
fn dominates_lunar(a: (f64, f64), b: (f64, f64)) -> bool {
    a.0 <= b.0 && a.1 >= b.1 && (a.0 < b.0 || a.1 > b.1)
}

// ───────────────────────────────────────────────────────────── 1. published anchor

#[test]
fn published_anchor_gps_baseline_reads_the_sps_values_through_the_evaluator() {
    // The GPS baseline 24-slot preset under the conditions of the GPS Standard
    // Positioning Service Performance Standard, Appendix B. Published global horizontal
    // dilution of precision: median 0.94, 90 % 1.16, 95 % 1.25.
    let gps = "kind = \"constellation-design\"\nbody = \"earth\"\nduration_s = 86100.0\nstep_s = 300.0\nmask_deg = 5.0\ngrid_step_deg = 4.0\nclock = \"common\"\n\n[[constellation]]\nname = \"GPS\"\npreset = \"gps-baseline\"\n";
    let src = format!(
        r#"kind = "pro-design-optimise"
title = "GPS baseline anchor"
seed = 1
base_scenario = '''
{gps}'''

[[variable]]
name = "mask_deg"
key = "mask_deg"
type = "choice"
values = [5.0]

[[constraint]]
result = {{ path = "global.hdop.p90", op = "<=", value = 1.19 }}

[[objective]]
name = "hdop_median"
sense = "min"
path = "global.hdop.median"

[[objective]]
name = "hdop_p95"
sense = "min"
path = "global.hdop.p95"

[[objective]]
name = "pdop_median"
sense = "min"
path = "global.pdop.median"

[reference]
label = "GPS baseline"
scenario = '''
{gps}'''
"#
    );
    let (_, v) = run(&src, 2);
    let (d, sha) = direct(gps);
    let r = &v["reference"];
    assert_eq!(r["status"], "ok");
    assert_eq!(r["result_sha256"].as_str(), Some(sha.as_str()));
    // Through the evaluator == the direct run == the published figures.
    for (name, path, expected) in [
        ("hdop_median", ["hdop", "median"], 0.94),
        ("hdop_p95", ["hdop", "p95"], 1.255),
        ("pdop_median", ["pdop", "median"], 1.795),
    ] {
        let got = num(&r["objectives"][name]);
        assert_eq!(got, num(&d["global"][path[0]][path[1]]), "{name}");
        assert_eq!(got, expected, "{name}");
    }
    // The 90th percentile is read through the result constraint.
    let p90 = num(&r["constraints"][0]["value"]);
    assert_eq!(p90, num(&d["global"]["hdop"]["p90"]));
    assert_eq!(p90, 1.165);
    assert_eq!(r["constraints"][0]["ok"], true);
    // Within the 0.03 bar of the published 0.94 / 1.16 / 1.25.
    for (got, published) in [
        (num(&r["objectives"]["hdop_median"]), 0.94),
        (p90, 1.16),
        (num(&r["objectives"]["hdop_p95"]), 1.25),
    ] {
        assert!((got - published).abs() <= 0.03, "{got} vs {published}");
    }
    // The one design of the space is the same constellation, so the reference sits on
    // the front and equals it.
    assert_eq!(v["work"]["evaluations"], 1);
    assert_eq!(r["on_front"], true);
    assert_eq!(r["equal_to"], serde_json::json!(["001"]));
    assert_eq!(v["front"][0]["objectives"], r["objectives"]);
}

// ───────────────────────────────────────────── 2. exhaustive equals brute force

#[test]
fn exhaustive_front_equals_a_brute_force_front_over_scenarios_the_test_writes() {
    let (_, v) = lunar();
    // Hand enumeration of the space, and one direct engine run per feasible design.
    let mut designs: Vec<(Key, String)> = Vec::new();
    let mut hand_infeasible = 0usize;
    for &t in &TOTALS {
        for &p in &PLANES {
            for &f in &PHASINGS {
                for &h in &ALTITUDES {
                    for &i in &INCLINATIONS {
                        if t % p == 0 && f < p {
                            designs
                                .push(((t, p, f, h as u64, i as u64), lunar_text(t, p, f, h, i)));
                        } else {
                            hand_infeasible += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(designs.len(), 608);
    assert_eq!(hand_infeasible, 544);
    let direct_runs: Vec<(f64, f64)> = parallel_map(designs.len(), THREADS, |n| {
        let (d, _) = direct(&designs[n].1);
        (
            num(&d["total_satellites"]),
            num(&d["global"]["availability_pct"]),
        )
    });
    let by_key: BTreeMap<Key, (f64, f64)> = designs
        .iter()
        .map(|(k, _)| *k)
        .zip(direct_runs.iter().copied())
        .collect();

    // The study ran exactly those designs, in the same order, to the same numbers.
    let evaluated = v["evaluated"].as_array().unwrap();
    assert_eq!(evaluated.len(), 608);
    assert_eq!(v["work"]["evaluations"], 608);
    assert_eq!(v["work"]["infeasible"], 544);
    assert_eq!(v["work"]["failed"], 0);
    assert_eq!(v["space"]["feasible"], 608);
    assert_eq!(v["exhaustive"], true);
    for (n, e) in evaluated.iter().enumerate() {
        let k = key_of(&e["variables"]);
        assert_eq!(k, designs[n].0, "design order");
        let (sats, avail) = by_key[&k];
        assert_eq!(num(&e["objectives"]["satellites"]), sats);
        assert_eq!(num(&e["objectives"]["availability_pct"]), avail);
    }

    // Brute force: a plain double loop over the direct runs.
    let brute: BTreeSet<Key> = designs
        .iter()
        .enumerate()
        .filter(|(a, _)| {
            !(0..designs.len()).any(|b| b != *a && dominates_lunar(direct_runs[b], direct_runs[*a]))
        })
        .map(|(_, (k, _))| *k)
        .collect();
    let front: BTreeSet<Key> = v["front"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| key_of(&f["variables"]))
        .collect();
    assert_eq!(front, brute);
    assert_eq!(v["front_summary"]["designs"], brute.len());

    // The distinct points of the front, from the direct runs.
    let mut points: Vec<(u64, f64)> = brute.iter().map(|k| (k.0, by_key[k].1)).collect::<Vec<_>>();
    points.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    points.dedup();
    assert_eq!(
        points,
        vec![
            (8, 19.5446),
            (12, 55.4222),
            (14, 73.5637),
            (16, 85.0404),
            (20, 93.2496),
            (24, 100.0)
        ]
    );
    assert_eq!(v["front_summary"]["distinct_objective_points"], 6);
    assert_eq!(v["knee"]["exists"], true);
    assert_eq!(num(&v["knee"]["objectives"]["satellites"]), 16.0);
}

// ───────────────────────────── 3. every front design reproduces, stand-alone

#[test]
fn every_front_scenario_reproduces_its_objectives_and_result_hash_stand_alone() {
    let link = run(&study("link-budget-trade.toml"), 2).1;
    let mars = run(&study("mars-navigation-shell.toml"), THREADS).1;
    for (v, paths) in [
        (
            &lunar().1,
            vec![
                ("satellites", "total_satellites"),
                ("availability_pct", "global.availability_pct"),
            ],
        ),
        (&link, vec![("margin_db", "margin_db")]),
        (&mars, vec![("availability_pct", "global.availability_pct")]),
    ] {
        let front = v["front"].as_array().unwrap();
        assert!(!front.is_empty());
        for f in front {
            let text = f["scenario"].as_str().unwrap();
            assert_eq!(
                f["scenario_sha256"].as_str().unwrap(),
                sha256_hex(text.as_bytes())
            );
            let (d, sha) = direct(text);
            assert_eq!(
                f["result_sha256"].as_str().unwrap(),
                sha,
                "front {}",
                f["id"]
            );
            for (name, path) in &paths {
                let mut at = &d;
                for step in path.split('.') {
                    at = &at[step];
                }
                assert_eq!(num(&f["objectives"][*name]), num(at), "{name}");
            }
            // A front scenario is a plain open scenario: no Pro kind, no study key.
            let parsed: toml::Value = toml::from_str(text).unwrap();
            assert!(!parsed["kind"].as_str().unwrap().starts_with("pro-"));
            assert!(parsed.get("base_scenario").is_none());
        }
    }
}

// ───────────────────────────────────────────────────────── 4. search kernel oracle

#[test]
fn nsga2_converges_on_the_analytic_zdt1_front() {
    // ZDT1 (Zitzler, Deb and Thiele, 2000): 30 variables in [0, 1], f1 = x1,
    // f2 = g (1 - sqrt(f1 / g)), g = 1 + 9 mean(x2..x30). True front: g = 1, so
    // f2 = 1 - sqrt(f1). The bar was fixed before the first run: no returned point
    // below the true front, mean distance to it at most 0.05, with population 100 and
    // 250 generations. (The first run missed it at 0.177: the crossover lacked its
    // exchange step. The operator was corrected; the bar was not moved.)
    const N: usize = 30;
    const LEVELS: usize = 1001;
    let feasible = |_: &[usize]| true;
    let problem = Problem {
        levels: vec![LEVELS; N],
        ordered: vec![true; N],
        senses: vec![Objective::Min, Objective::Min],
        feasible: &feasible,
        all_feasible: None,
    };
    let cfg = Config {
        population: 100,
        generations: 250,
        budget: 100 * 251,
        seed: 1,
    };
    let zdt1 = |d: &[usize]| -> Vec<f64> {
        let x: Vec<f64> = d.iter().map(|&i| i as f64 / (LEVELS - 1) as f64).collect();
        let g = 1.0 + 9.0 * x[1..].iter().sum::<f64>() / (N - 1) as f64;
        vec![x[0], g * (1.0 - (x[0] / g).sqrt())]
    };
    let mut score = |batch: &[Vec<usize>]| -> Vec<Fitness> {
        batch
            .iter()
            .map(|d| Fitness::Scored {
                objectives: zdt1(d),
                violations: 0,
            })
            .collect()
    };
    let log = nsga2(&problem, &cfg, &mut score);
    assert_eq!(log.designs.len(), 100 * 251);
    let points: Vec<Vec<f64>> = log
        .fitness
        .iter()
        .map(|f| match f {
            Fitness::Scored { objectives, .. } => objectives.clone(),
            Fitness::Failed => panic!("ZDT1 never fails"),
        })
        .collect();
    let front = kernel_front(&points, &problem.senses);
    assert!(front.len() >= 20, "front of {} points", front.len());
    // Distance of a point to the true front, by dense sampling of the curve.
    let distance = |p: &[f64]| -> f64 {
        (0..=4000)
            .map(|k| {
                let t = k as f64 / 4000.0;
                ((p[0] - t).powi(2) + (p[1] - (1.0 - t.sqrt())).powi(2)).sqrt()
            })
            .fold(f64::INFINITY, f64::min)
    };
    let mut sum = 0.0;
    let mut all: Vec<f64> = front.iter().map(|&i| distance(&points[i])).collect();
    all.sort_by(f64::total_cmp);
    println!(
        "ZDT1 distances: min {:.5}, median {:.5}, max {:.5}; generations run {}; cache hits {}",
        all[0],
        all[all.len() / 2],
        all[all.len() - 1],
        log.generations_run,
        log.cache_hits
    );
    for &i in &front {
        let p = &points[i];
        assert!(
            p[1] >= 1.0 - p[0].sqrt() - 1e-12,
            "a point below the true front: {p:?}"
        );
        sum += distance(p);
    }
    let mean = sum / front.len() as f64;
    println!(
        "ZDT1: front of {} points, mean distance to the true front {mean:.6}",
        front.len()
    );
    assert!(mean <= 0.05, "mean distance {mean} is above the 0.05 bar");
}

// ─────────────────────────────────────────────── 5. NSGA-II against the real space

fn with_search(src: &str, search: &str) -> String {
    let at = src
        .find("[search]")
        .expect("the study has a [search] table");
    let end = src[at..]
        .find("[reference]")
        .map(|i| at + i)
        .unwrap_or(src.len());
    format!("{}[search]\n{search}\n\n{}", &src[..at], &src[end..])
}

#[test]
fn nsga2_with_the_whole_space_in_budget_recovers_the_exhaustive_front() {
    let exhaustive = &lunar().1;
    let src = with_search(
        &study("lunar-navigation-global.toml"),
        "method = \"nsga2\"\nbudget = 700\npopulation = 40\ngenerations = 40",
    );
    let (_, v) = run(&src, THREADS);
    assert_eq!(v["search"]["method_used"], "nsga2");
    assert_eq!(v["work"]["evaluations"], 608);
    assert_eq!(v["exhaustive"], true, "every feasible design was run");
    let set = |v: &Value| -> BTreeSet<(Key, String)> {
        v["front"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (key_of(&f["variables"]), f["result_sha256"].to_string()))
            .collect()
    };
    assert_eq!(set(&v), set(exhaustive));
    assert!(!v["convergence"].as_array().unwrap().is_empty());
}

#[test]
fn nsga2_with_a_small_budget_returns_real_non_dominated_designs_and_says_best_found() {
    let src = with_search(
        &study("lunar-navigation-global.toml"),
        "method = \"nsga2\"\nbudget = 150\npopulation = 30\ngenerations = 40",
    );
    let (json, v) = run(&src, THREADS);
    assert_eq!(v["work"]["evaluations"], 150);
    assert_eq!(v["exhaustive"], false);
    assert!(v["label"].as_str().unwrap().starts_with("MODELLED"));
    assert!(v["label"]
        .as_str()
        .unwrap()
        .contains("best found in 150 runs"));
    assert!(v["front_statement"]
        .as_str()
        .unwrap()
        .starts_with("best found in 150 runs"));
    assert!(
        !json.to_lowercase().contains("optimal"),
        "a best-found front is never called optimal"
    );
    let evaluated = v["evaluated"].as_array().unwrap();
    let all: Vec<(f64, f64)> = evaluated
        .iter()
        .map(|e| {
            (
                num(&e["objectives"]["satellites"]),
                num(&e["objectives"]["availability_pct"]),
            )
        })
        .collect();
    // No design is run twice, and every one is a feasible Walker pattern.
    let keys: BTreeSet<Key> = evaluated.iter().map(|e| key_of(&e["variables"])).collect();
    assert_eq!(keys.len(), 150);
    for k in &keys {
        assert!(
            k.0 % k.1 == 0 && k.2 < k.1,
            "infeasible design was run: {k:?}"
        );
        assert!(TOTALS.contains(&k.0) && PLANES.contains(&k.1));
    }
    for f in v["front"].as_array().unwrap() {
        // A real design: the scenario re-run directly gives the stored numbers.
        let (d, sha) = direct(f["scenario"].as_str().unwrap());
        assert_eq!(f["result_sha256"].as_str().unwrap(), sha);
        let me = (
            num(&d["total_satellites"]),
            num(&d["global"]["availability_pct"]),
        );
        assert_eq!(me.1, num(&f["objectives"]["availability_pct"]));
        // Non-dominated among the evaluated designs.
        assert!(!all.iter().any(|o| dominates_lunar(*o, me)));
    }
    // The convergence log never loses hypervolume and ends at the reported value.
    let conv = v["convergence"].as_array().unwrap();
    assert!(conv.len() >= 2);
    for w in conv.windows(2) {
        assert!(num(&w[1]["hypervolume"]) >= num(&w[0]["hypervolume"]) - 1e-12);
    }
    assert_eq!(
        num(&conv.last().unwrap()["hypervolume"]),
        num(&v["hypervolume"]["value"])
    );
    assert_eq!(conv.last().unwrap()["evaluations"], 150);
}

// ─────────────────────────────────────────────────────────────── 6. determinism

/// A 76-design corner of the lunar space, to keep repeated runs short.
fn small_lunar(search: &str, seed: u64) -> String {
    let src = study("lunar-navigation-global.toml")
        .replace(
            "values = [1500.0, 3000.0, 5000.0, 8000.0]",
            "values = [8000.0]",
        )
        .replace(
            "min = 45.0\nmax = 90.0\nstep = 15.0",
            "min = 45.0\nmax = 60.0\nstep = 15.0",
        )
        .replace("seed = 1\n", &format!("seed = {seed}\n"));
    assert!(src.contains("values = [8000.0]") && src.contains("max = 60.0"));
    with_search(&src, search)
}

#[test]
fn the_same_study_and_seed_give_the_same_bytes_for_any_thread_count() {
    let exhaustive = small_lunar("method = \"exhaustive\"\nbudget = 100", 1);
    let (one, v) = run(&exhaustive, 1);
    let (four, _) = run(&exhaustive, 4);
    assert_eq!(v["work"]["evaluations"], 76);
    assert_eq!(one, four, "exhaustive: threads 1 and 4 differ");

    let ga = "method = \"nsga2\"\nbudget = 40\npopulation = 10\ngenerations = 20";
    let (a1, va) = run(&small_lunar(ga, 1), 1);
    let (a4, _) = run(&small_lunar(ga, 1), 4);
    let (again, _) = run(&small_lunar(ga, 1), 3);
    assert_eq!(a1, a4, "NSGA-II: threads 1 and 4 differ");
    assert_eq!(a1, again, "NSGA-II: two runs with one seed differ");
    let (_, vb) = run(&small_lunar(ga, 2), 4);
    let order = |v: &Value| -> Vec<Key> {
        v["evaluated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| key_of(&e["variables"]))
            .collect()
    };
    assert_eq!(order(&va).len(), 40);
    assert_ne!(
        order(&va),
        order(&vb),
        "a different seed gives a different search"
    );
    assert_ne!(va["scenario_hash"], vb["scenario_hash"]);
    // `threads` written in the study file changes neither the hash nor the result.
    let with_threads = small_lunar(&format!("{ga}\nthreads = 2"), 1);
    assert_eq!(run(&with_threads, 2).0, a1);
}

#[test]
fn the_study_hash_ignores_comments_and_key_order_and_follows_every_value() {
    let base = study("link-budget-trade.toml");
    let hash = |src: &str| Study::parse(src).unwrap().resolve().unwrap().scenario_hash;
    let h = hash(&base);
    assert_eq!(h.len(), 64);
    // Comments, in the study and in the base scenario.
    let commented = base
        .replace("seed = 1\n", "seed = 1 # a comment\n")
        .replace(
            "band = \"x\"\n",
            "# inside the base scenario\nband = \"x\"\n",
        );
    assert_ne!(commented, base);
    assert_eq!(hash(&commented), h);
    // Key order.
    let reordered = base.replace(
        "title = \"X-band link: margin against transmit power\"\nseed = 1\n",
        "seed = 1\ntitle = \"X-band link: margin against transmit power\"\n",
    );
    assert_ne!(reordered, base);
    assert_eq!(hash(&reordered), h);
    // A value.
    let changed = base.replace("range_km = 2000.0", "range_km = 2000.5");
    assert_ne!(changed, base);
    assert_ne!(hash(&changed), h);
    assert_ne!(hash(&base.replace("seed = 1\n", "seed = 2\n")), h);
}

// ─────────────────────────────────────────────────────────────── 7. constraints

#[test]
fn a_design_whose_planes_do_not_divide_its_total_is_never_run() {
    let (_, v) = lunar();
    let mut hand = 0usize;
    for &t in &TOTALS {
        for &p in &PLANES {
            for &f in &PHASINGS {
                if !(t % p == 0 && f < p) {
                    hand += ALTITUDES.len() * INCLINATIONS.len();
                }
            }
        }
    }
    assert_eq!(hand, 544);
    assert_eq!(v["work"]["infeasible"], hand);
    assert_eq!(v["space"]["infeasible"], hand);
    assert_eq!(v["space"]["grid_size"], 1152);
    for e in v["evaluated"].as_array().unwrap() {
        let k = key_of(&e["variables"]);
        assert!(k.0 % k.1 == 0 && k.2 < k.1, "{k:?} was run");
    }
    // Engine runs: one per feasible design plus the reference, and nothing else.
    assert_eq!(v["reproducibility"]["engine_runs"], 609);
}

#[test]
fn a_run_that_breaks_a_result_constraint_stays_in_the_log_and_off_the_front() {
    let src = small_lunar("method = \"exhaustive\"\nbudget = 100", 1).replace(
        "[[objective]]\nname = \"satellites\"",
        "[[constraint]]\nresult = { path = \"global.worst_site_availability_pct\", op = \">=\", value = 50.0 }\n\n[[objective]]\nname = \"satellites\"",
    );
    assert!(src.contains("worst_site_availability_pct"));
    let (_, v) = run(&src, THREADS);
    let evaluated = v["evaluated"].as_array().unwrap();
    assert_eq!(evaluated.len(), 76, "every design is still in the log");
    let mut violated = 0;
    for e in evaluated {
        // Hand check against a direct run of a scenario the test writes itself.
        let k = key_of(&e["variables"]);
        let (d, _) = direct(&lunar_text(k.0, k.1, k.2, k.3 as f64, k.4 as f64));
        let worst = num(&d["global"]["worst_site_availability_pct"]);
        assert_eq!(num(&e["constraints"][0]["value"]), worst);
        if worst >= 50.0 {
            assert_eq!(e["status"], "ok");
        } else {
            violated += 1;
            assert_eq!(e["status"], "constraint-violated");
            assert_eq!(e["on_front"], false);
            assert!(
                e["objectives"].is_object(),
                "its objectives are still reported"
            );
        }
    }
    assert!(violated > 0 && violated < 76, "{violated} violators");
    assert_eq!(v["work"]["constraint_violated"], violated);
    // The reference is outside the space and is checked against the same constraint.
    assert!(v["reference"]["constraints"][0]["value"].is_number());
}

#[test]
fn a_failed_engine_run_is_recorded_with_its_error_and_counted() {
    // An altitude of -100 km puts the orbit inside the Moon: the engine rejects it.
    let src = small_lunar("method = \"exhaustive\"\nbudget = 200", 1)
        .replace("values = [8000.0]", "values = [-100.0, 8000.0]");
    let (_, v) = run(&src, THREADS);
    let evaluated = v["evaluated"].as_array().unwrap();
    assert_eq!(evaluated.len(), 152);
    let failed: Vec<&Value> = evaluated
        .iter()
        .filter(|e| e["status"] == "failed")
        .collect();
    assert_eq!(failed.len(), 76, "one failure per design at -100 km");
    assert_eq!(v["work"]["failed"], 76);
    for e in &failed {
        assert_eq!(num(&e["variables"]["altitude_km"]), -100.0);
        assert!(e["objectives"].is_null());
        assert_eq!(e["on_front"], false);
        let error = e["error"].as_str().unwrap();
        // The same error a direct run gives.
        let k = key_of(&serde_json::json!({
            "total": e["variables"]["total"], "planes": e["variables"]["planes"],
            "phasing": e["variables"]["phasing"], "altitude_km": 0.0,
            "inclination_deg": e["variables"]["inclination_deg"],
        }));
        let direct_err = kshana::api::run_scenario(&lunar_text(k.0, k.1, k.2, -100.0, k.4 as f64))
            .unwrap_err()
            .to_string();
        assert_eq!(error, direct_err);
    }
    assert!(v["front"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| num(&f["variables"]["altitude_km"]) == 8000.0));
}

// ───────────────────────────────────────────── 8. any kind: link-budget closed form

#[test]
fn a_link_budget_study_gives_the_front_the_closed_form_predicts() {
    let src = study("link-budget-trade.toml");
    let (_, v) = run(&src, 2);
    assert_eq!(v["base"]["kind"], "link-budget");
    // The anchor: the bundled link budget, run directly.
    let base = "kind = \"link-budget\"\nband = \"x\"\neirp_dbw = 55.0\ng_over_t_db = 53.0\nrange_km = 2000.0\ndata_rate_bps = 1.0e6\nother_losses_db = 3.0\nrequired_eb_n0_db = 4.5\n";
    let (d, _) = direct(base);
    let m0 = num(&d["margin_db"]);
    assert_eq!(num(&v["reference"]["objectives"]["margin_db"]), m0);

    // Every design: +1 dB of margin per dB of EIRP, -10 log10 of the rate ratio.
    let evaluated = v["evaluated"].as_array().unwrap();
    assert_eq!(evaluated.len(), 15);
    for e in evaluated {
        let eirp = num(&e["variables"]["eirp_dbw"]);
        let rate = num(&e["variables"]["data_rate_bps"]);
        let predicted = m0 + (eirp - 55.0) - 10.0 * (rate / 1.0e6).log10();
        let got = num(&e["objectives"]["margin_db"]);
        assert!(
            (got - predicted).abs() < 1e-9,
            "{eirp} dBW, {rate} bit/s: {got} vs {predicted}"
        );
        assert_eq!(num(&e["objectives"]["eirp_dbw"]), eirp);
    }
    // So the front is the lowest data rate at each EIRP: five designs, 5 dB apart.
    let front = v["front"].as_array().unwrap();
    assert_eq!(front.len(), 5);
    for (n, f) in front.iter().enumerate() {
        let eirp = 40.0 + 5.0 * n as f64;
        assert_eq!(num(&f["variables"]["eirp_dbw"]), eirp);
        assert_eq!(num(&f["variables"]["data_rate_bps"]), 1.0e5);
        assert!((num(&f["objectives"]["margin_db"]) - (m0 + (eirp - 55.0) + 10.0)).abs() < 1e-9);
    }
    // A straight-line front has no knee.
    assert_eq!(v["knee"]["exists"], false);
    // The reference (55 dBW at 1 Mbit/s) against the front, by the closed form: at a
    // tenth of the data rate the 45 dBW design has the same margin for 10 dB less
    // power, and the 50 and 55 dBW designs have more margin for no more power. The
    // 40 dBW design has 5 dB less margin and the 60 dBW one needs more power.
    assert_eq!(v["reference"]["in_design_space"], true);
    assert_eq!(v["reference"]["on_front"], false);
    assert_eq!(
        v["reference"]["dominated_by"],
        serde_json::json!(["002", "003", "004"])
    );
    assert_eq!(
        v["work"]["cache_hits"], 1,
        "the reference reuses the evaluated design"
    );
    assert_eq!(v["work"]["reference_runs"], 0);
}

// ─────────────────────────────────────────────────────────────────── 9. any body

#[test]
fn a_leo_shell_around_the_earth_and_a_shell_around_mars_both_run() {
    // A 27-design corner of the low-Earth-orbit study (the full study is the showcase).
    let leo = study("leo-positioning-earth.toml")
        .replace(
            "values = [120, 144, 168, 192, 216, 240, 264, 288, 312, 336, 360]",
            "values = [240, 288, 336]",
        )
        .replace("values = [6, 8, 12, 16, 18, 24]", "values = [12, 16]")
        .replace("values = [600.0, 900.0, 1200.0]", "values = [1200.0]");
    let (_, e) = run(&leo, THREADS);
    assert_eq!(e["base"]["kind"], "constellation-design");
    // 240 and 336 divide by 12 and 16, 288 by both: 6 patterns x 3 phasings x 3 inclinations.
    assert_eq!(e["work"]["evaluations"], 54);
    assert_eq!(e["objectives"].as_array().unwrap().len(), 3);
    let front = e["front"].as_array().unwrap();
    assert!(!front.is_empty());
    for f in front {
        let (d, sha) = direct(f["scenario"].as_str().unwrap());
        assert_eq!(d["body"]["name"], "Earth");
        assert_eq!(f["result_sha256"].as_str().unwrap(), sha);
        assert_eq!(
            num(&f["objectives"]["pdop_p95"]),
            num(&d["global"]["pdop"]["p95"])
        );
        assert_eq!(
            num(&f["objectives"]["availability_pct"]),
            num(&d["global"]["availability_pct"])
        );
        assert!(
            num(&d["global"]["fix_pct"]) >= 99.0,
            "the result constraint holds on the front"
        );
        assert_eq!(
            num(&f["objectives"]["satellites"]),
            num(&d["total_satellites"])
        );
    }

    let (_, m) = run(&study("mars-navigation-shell.toml"), THREADS);
    // 12 and 24 divide by 3, 4 and 6; 18 by 3 and 6: 8 patterns x 3 altitudes x 3 inclinations.
    assert_eq!(m["work"]["evaluations"], 72);
    assert_eq!(m["exhaustive"], true);
    let front = m["front"].as_array().unwrap();
    assert!(front.len() >= 3);
    for f in front {
        let (d, sha) = direct(f["scenario"].as_str().unwrap());
        assert_eq!(d["body"]["name"], "Mars");
        assert_eq!(f["result_sha256"].as_str().unwrap(), sha);
        assert_eq!(
            num(&f["objectives"]["availability_pct"]),
            num(&d["global"]["availability_pct"])
        );
        assert_eq!(
            num(&f["objectives"]["satellites"]),
            num(&d["total_satellites"])
        );
    }
}

// ────────────────────────────────────────────────────────────────── 10. licence

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
}

#[test]
fn the_binary_refuses_the_study_without_a_licence_and_runs_it_with_dev() {
    let path = dir().join("link-budget-trade.toml");
    // `run` without a licence: refused at the Evaluation tier, no licensee name.
    let out = bin().arg("run").arg(&path).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("'pro-design-optimise' requires the 'enterprise' tier; current license tier is 'evaluation'"),
        "{err}"
    );
    assert!(!err.contains("licensee"), "{err}");
    assert!(
        !err.contains("no license)"),
        "the licensee name leaked: {err}"
    );
    assert!(out.stdout.is_empty(), "nothing is run");

    // `run --dev`: the JSON on standard output is the study's result.
    let out = bin().arg("run").arg(&path).arg("--dev").output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["kind"], "pro-design-optimise");
    let (expected, _) = run(&study("link-budget-trade.toml"), 1);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim_end(), expected);

    // `design` without a licence: refused before anything is written.
    let d = tmp("licence");
    let out = bin()
        .arg("design")
        .arg(&path)
        .arg("--out")
        .arg(&d)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("requires the 'enterprise' tier"), "{err}");
    assert!(!err.contains("no license)"), "{err}");
    assert!(!d.join("design.result.json").exists());

    // `design --dev`: the five outputs.
    let out = bin()
        .arg("design")
        .arg(&path)
        .arg("--out")
        .arg(&d)
        .arg("--dev")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = std::fs::read_to_string(d.join("design.result.json")).unwrap();
    assert_eq!(json.trim_end(), expected);
    let v: Value = serde_json::from_str(&json).unwrap();
    let svg = std::fs::read_to_string(d.join("design.pareto.svg")).unwrap();
    assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    let html = std::fs::read_to_string(d.join("design.report.html")).unwrap();
    assert!(html.contains("<svg") && html.contains("MODELLED"));
    // Self-contained: nothing is fetched.
    for needle in [
        "http://", "https://", "<script", "<link", "<img", "@import", "url(",
    ] {
        let body = html.replace("http://www.w3.org/2000/svg", "");
        assert!(!body.contains(needle), "the report references {needle}");
    }
    let csv = std::fs::read_to_string(d.join("front/index.csv")).unwrap();
    let rows: Vec<&str> = csv.lines().collect();
    assert_eq!(rows.len(), 1 + v["front"].as_array().unwrap().len());
    assert!(
        rows[0].starts_with("id,file,knee,eirp_dbw,data_rate_bps,margin_db,eirp_dbw (objective),")
    );
    for f in v["front"].as_array().unwrap() {
        // Each front file is the scenario text of the result, byte for byte.
        let file = std::fs::read_to_string(d.join(f["file"].as_str().unwrap())).unwrap();
        assert_eq!(file, f["scenario"].as_str().unwrap());
        assert!(csv.contains(f["result_sha256"].as_str().unwrap()));
    }
    // The registry route gives the same document as the command-line route.
    let via_registry = kshana_pro::registry::run(&study("link-budget-trade.toml")).unwrap();
    assert_eq!(via_registry.json, expected);
    assert_eq!(via_registry.csv.as_deref(), Some(csv.as_str()));
}

// ────────────────────────────────────────────────────────────── 11. adversarial

fn error_of(src: &str) -> String {
    match run_study(src, &Options { threads: Some(2) }) {
        Ok(out) => panic!("expected an error, got a front: {}", out.summary),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_broken_study_gives_a_clear_error_and_never_a_front() {
    let good = study("link-budget-trade.toml");
    assert!(run_study(&good, &Options::default()).is_ok());

    // An empty variable list.
    let at = good.find("[[variable]]").unwrap();
    let end = good.find("[[objective]]").unwrap();
    let no_vars = format!("{}{}", &good[..at], &good[end..]).replace(
        "sense = \"min\"\nvariable = \"eirp_dbw\"",
        "sense = \"min\"\npath = \"eirp_dbw\"",
    );
    let no_vars = &no_vars[..no_vars.find("[reference]").unwrap()];
    let e = error_of(no_vars);
    assert!(e.contains("no [[variable]]"), "{e}");

    // A NaN bound.
    let e = error_of(&good.replace("min = 40.0", "min = nan"));
    assert!(
        e.contains("variable 'eirp_dbw': min is not a finite number"),
        "{e}"
    );
    let e = error_of(&good.replace("max = 60.0", "max = inf"));
    assert!(e.contains("max is not a finite number"), "{e}");

    // A key that does not exist in the base scenario.
    let e = error_of(&good.replace("key = \"eirp_dbw\"", "key = \"transmitter.eirp_dbw\""));
    assert!(
        e.contains("key 'transmitter.eirp_dbw' does not exist in the base scenario"),
        "{e}"
    );

    // An objective path that is missing from the result.
    let e = error_of(&good.replace("path = \"margin_db\"", "path = \"link.margin_db\""));
    assert!(
        e.contains("objective 'margin_db': path 'link.margin_db' is not in the result of a `link-budget` run"),
        "{e}"
    );

    // More of the same family.
    let e = error_of(&good.replace("path = \"margin_db\"", "path = \"band\""));
    assert!(e.contains("holds a string, not a number"), "{e}");
    let e = error_of(&good.replace("step = 5.0", "step = 0.0"));
    assert!(e.contains("step must be greater than zero"), "{e}");
    let e = error_of(&good.replace("sense = \"max\"", "sense = \"best\""));
    assert!(e.contains("is not min or max"), "{e}");
    let e = error_of(&good.replace("budget = 100", "budget = 10"));
    assert!(
        e.contains("exhaustive needs 15 runs but the budget is 10"),
        "{e}"
    );
    let e = error_of(&good.replace("variable = \"eirp_dbw\"", "variable = \"power\""));
    assert!(e.contains("'power' is not a variable of this study"), "{e}");
    let e = error_of(&good.replace("kind = \"link-budget\"", "kind = \"link-budgett\""));
    assert!(
        e.contains("no design could be scored") || e.contains("link-budgett"),
        "{e}"
    );
    let one_objective = &good[..good.find("[[objective]]\nname = \"eirp_dbw\"").unwrap()];
    let e = error_of(one_objective);
    assert!(e.contains("two or three [[objective]]"), "{e}");
    let e = error_of(&good.replace(
        "values = { eirp_dbw = 55.0, data_rate_bps = 1.0e6 }",
        "values = { eirp_dbw = 56.0, data_rate_bps = 1.0e6 }",
    ));
    assert!(e.contains("is not a level of variable 'eirp_dbw'"), "{e}");

    // A Walker constraint nothing satisfies: no feasible design, no front.
    let none = study("mars-navigation-shell.toml").replace("values = [3, 4, 6]", "values = [5, 7]");
    let e = error_of(&none);
    assert!(e.contains("no feasible design"), "{e}");
}

// ─────────────────────────────────────────────────────── 12. the result contract

/// Every numeric leaf of a document, as (path with indices dropped, value).
fn numeric_leaves(v: &Value, path: &str, out: &mut Vec<String>) {
    match v {
        Value::Number(_) => out.push(path.to_string()),
        Value::Array(a) => {
            for x in a {
                numeric_leaves(x, &format!("{path}[]"), out);
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                numeric_leaves(x, &p, out);
            }
        }
        _ => {}
    }
}

#[test]
fn the_result_carries_the_pro_result_contract() {
    let link = run(&study("link-budget-trade.toml"), 2).1;
    let ga = run(
        &small_lunar(
            "method = \"nsga2\"\nbudget = 40\npopulation = 10\ngenerations = 20",
            1,
        ),
        THREADS,
    )
    .1;
    for v in [&lunar().1, &link, &ga] {
        assert_eq!(v["schema_version"], "1.0");
        assert_eq!(v["kind"], "pro-design-optimise");
        assert_eq!(v["overlay"], "kshana-pro");
        assert_eq!(v["overlay_version"], kshana_pro::version());
        assert_eq!(v["engine_version"], kshana_pro::design::engine_version());
        assert!(v["seed"].is_u64());
        assert_eq!(v["scenario_hash"].as_str().unwrap().len(), 64);
        assert!(v["label"].as_str().unwrap().starts_with("MODELLED"));
        assert!(!v["not_modelled"].as_array().unwrap().is_empty());

        // Units: every numeric field of the document has an entry.
        let units = v["units"].as_object().unwrap();
        let mut leaves = Vec::new();
        for (k, x) in v.as_object().unwrap() {
            if k != "units" {
                numeric_leaves(x, k, &mut leaves);
            }
        }
        leaves.sort();
        leaves.dedup();
        assert!(leaves.len() > 20);
        for path in &leaves {
            let entry = units
                .get(path)
                .unwrap_or_else(|| panic!("no units entry for '{path}'"));
            assert!(entry["unit"].is_string(), "{path}");
            assert!(
                ["computed", "input", "constant"].contains(&entry["provenance"].as_str().unwrap()),
                "{path}"
            );
            assert!(entry["note"].is_string(), "{path}");
        }

        // The run digest: SHA-256 over every member result's SHA-256, in design order.
        let mut all = String::new();
        for e in v["evaluated"].as_array().unwrap() {
            all.push_str(e["result_sha256"].as_str().unwrap());
            all.push('\n');
        }
        assert_eq!(
            v["reproducibility"]["run_digest"].as_str().unwrap(),
            sha256_hex(all.as_bytes())
        );
        let evaluations = v["work"]["evaluations"].as_u64().unwrap();
        assert_eq!(v["evaluated"].as_array().unwrap().len() as u64, evaluations);
        assert_eq!(
            v["reproducibility"]["engine_runs"].as_u64().unwrap(),
            evaluations + v["work"]["reference_runs"].as_u64().unwrap()
        );
        for key in ["cache_hits", "infeasible", "failed"] {
            assert!(v["work"][key].is_u64(), "work.{key}");
        }
        assert_eq!(
            v["convergence"].as_array().unwrap().is_empty(),
            v["search"]["method_used"] == "exhaustive"
        );

        // Figure tiers: never above the weakest matrix row that names the base kind.
        let kind = v["base"]["kind"].as_str().unwrap();
        let needle = format!("`{kind}`");
        let weakest_is_validated = kshana::verification::verification_matrix()
            .iter()
            .filter(|r| r.capability.contains(&needle) || r.requirement.contains(&needle))
            .all(|r| r.status == kshana::verification::VerificationStatus::Validated);
        let figures = v["figure_tiers"]["figures"].as_array().unwrap();
        assert!(!figures.is_empty());
        for f in figures {
            let path = f["path"].as_str().unwrap();
            let tier = f["tier"].as_str().unwrap();
            assert!(f["requirement"].as_str().is_some_and(|r| !r.is_empty()));
            assert!(["VALIDATED", "MODELLED", "PARTNER"].contains(&tier));
            if !weakest_is_validated || path.starts_with("front[]") || path.starts_with("knee") {
                assert_ne!(tier, "VALIDATED", "{path} was raised");
            }
        }
    }
    // The two real cases, spelled out. constellation-design has a MODELLED row, so
    // nothing in a constellation study is VALIDATED.
    let tiers = |v: &Value, path: &str| -> String {
        v["figure_tiers"]["figures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == path)
            .map(|f| f["tier"].as_str().unwrap().to_string())
            .unwrap_or_else(|| "untiered".into())
    };
    assert_eq!(
        tiers(&lunar().1, "reference.objectives.availability_pct"),
        "MODELLED"
    );
    assert_eq!(
        tiers(&lunar().1, "knee.objectives.availability_pct"),
        "MODELLED"
    );
    // link-budget's only row is VALIDATED: the engine's own number keeps that label,
    // but its place on the front does not.
    assert_eq!(tiers(&link, "reference.objectives.margin_db"), "VALIDATED");
    assert_eq!(
        tiers(&link, "evaluated[].objectives.margin_db"),
        "VALIDATED"
    );
    assert_eq!(tiers(&link, "front[].objectives.margin_db"), "MODELLED");
    assert_eq!(tiers(&link, "knee.objectives.margin_db"), "MODELLED");
    // A design variable is an input: no tier is claimed for it.
    assert_eq!(tiers(&link, "front[].objectives.eirp_dbw"), "untiered");
    // Units come from the engine's own units block.
    assert_eq!(link["units"]["front[].objectives.margin_db"]["unit"], "dB");
    assert_eq!(link["units"]["front[].variables.eirp_dbw"]["unit"], "dBW");
    assert_eq!(
        link["units"]["front[].variables.data_rate_bps"]["unit"],
        "bit/s"
    );
    assert_eq!(
        lunar().1["units"]["front[].objectives.availability_pct"]["unit"],
        "%"
    );
    assert_eq!(
        lunar().1["units"]["front[].objectives.satellites"]["unit"],
        "count"
    );
}

#[test]
fn the_reference_design_is_placed_against_the_front() {
    // Whole Moon: the bundled two-shell design, run directly.
    let (_, v) = lunar();
    let bundled =
        std::fs::read_to_string(dir().join("engine/lunar-relay-constellation.toml")).unwrap();
    let (d, sha) = direct(&bundled);
    let r = &v["reference"];
    assert_eq!(r["result_sha256"].as_str().unwrap(), sha);
    assert_eq!(r["in_design_space"], false);
    assert_eq!(
        num(&r["objectives"]["satellites"]),
        num(&d["total_satellites"])
    );
    assert_eq!(num(&r["objectives"]["satellites"]), 14.0);
    assert_eq!(
        num(&r["objectives"]["availability_pct"]),
        num(&d["global"]["availability_pct"])
    );
    // Hand placement against the front of the result.
    let me = (
        num(&d["total_satellites"]),
        num(&d["global"]["availability_pct"]),
    );
    let mut dominated_by = Vec::new();
    for f in v["front"].as_array().unwrap() {
        let o = (
            num(&f["objectives"]["satellites"]),
            num(&f["objectives"]["availability_pct"]),
        );
        if dominates_lunar(o, me) {
            dominated_by.push(f["id"].clone());
        }
    }
    assert_eq!(r["dominated_by"], Value::Array(dominated_by.clone()));
    assert_eq!(r["on_front"], dominated_by.is_empty());
}

// ────────────────────────────────────────────────── 13. provenance and isolation

fn files_under(root: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn no_scenario_or_fixture_of_this_feature_uses_the_optional_leo_preset() {
    // The open engine's optional preset of workshop-derived numbers must never reach a
    // Pro scenario: the feature has to work on any system and with that preset absent.
    let needle: String = ["ce", "le", "ste"].concat();
    let mut files = Vec::new();
    files_under(&dir(), &mut files);
    assert!(files.len() >= 6, "found {} files", files.len());
    for f in files {
        let text = String::from_utf8_lossy(&std::fs::read(&f).unwrap()).to_lowercase();
        assert!(
            !text.contains(&needle),
            "{} mentions the optional preset",
            f.display()
        );
        // The name inside this folder, so the folder the repository is checked out in
        // cannot fail the test.
        let inside = f.strip_prefix(dir()).unwrap_or(&f);
        assert!(
            !inside.to_string_lossy().to_lowercase().contains(&needle),
            "{} is named after the optional preset",
            f.display()
        );
    }
}

#[test]
fn engine_scenarios_are_the_files_the_provenance_note_names() {
    let provenance = std::fs::read_to_string(dir().join("PROVENANCE.md")).unwrap();
    let mut copies = Vec::new();
    files_under(&dir().join("engine"), &mut copies);
    assert_eq!(copies.len(), 2);
    for f in &copies {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let sha = sha256_hex(&std::fs::read(f).unwrap());
        assert!(
            provenance.contains(&format!("| `{name}` | `{sha}` |")),
            "PROVENANCE.md does not pin {name} at {sha}"
        );
        // When the engine checkout is next to this one, the copy is its file.
        let upstream = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../kshana-wt-pro-engine/scenarios")
            .join(&name);
        if upstream.exists() {
            assert_eq!(
                std::fs::read(&upstream).unwrap(),
                std::fs::read(f).unwrap(),
                "{name}"
            );
        }
    }
    // The reference design of the whole-Moon study is the engine's file, byte for byte.
    let bundled =
        std::fs::read_to_string(dir().join("engine/lunar-relay-constellation.toml")).unwrap();
    let global = Study::parse(&study("lunar-navigation-global.toml")).unwrap();
    assert_eq!(global.reference.unwrap().scenario.unwrap(), bundled);
    // The polar study's reference is that file with three stated lines changed.
    let polar = Study::parse(&study("lunar-navigation-south-polar.toml")).unwrap();
    let polar_ref = polar.reference.unwrap().scenario.unwrap();
    assert_eq!(
        polar_ref.replace(
            "grid_step_deg = 5.0\nlat_min_deg = -90.0\nlat_max_deg = -70.0\n",
            "grid_step_deg = 10.0\n"
        ),
        bundled
    );
    // The link study's base scenario is the engine's link budget without its comments.
    let link = Study::parse(&study("link-budget-trade.toml")).unwrap();
    let a: toml::Value = toml::from_str(&link.base_scenario).unwrap();
    let b: toml::Value =
        toml::from_str(&std::fs::read_to_string(dir().join("engine/link-budget.toml")).unwrap())
            .unwrap();
    assert_eq!(a, b);
}

#[test]
fn every_shipped_study_parses_and_its_space_is_the_stated_size() {
    for (name, feasible) in [
        ("lunar-navigation-global.toml", 608),
        ("lunar-navigation-south-polar.toml", 608),
        ("leo-positioning-earth.toml", 1431),
        ("mars-navigation-shell.toml", 72),
        ("link-budget-trade.toml", 15),
    ] {
        let r = Study::parse(&study(name)).unwrap().resolve().unwrap();
        assert_eq!(r.space.enumerate().unwrap().len(), feasible, "{name}");
        assert!(feasible <= r.study.search.budget, "{name} is exhaustive");
    }
}

// ─────────────────────────────────────────────────────── 14. the guide's numbers

/// A number as the guide writes it: at most four decimals, no trailing zeros.
fn shown(x: f64) -> String {
    let s = format!("{x:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn front_points(v: &Value) -> BTreeSet<(u64, String)> {
    v["front"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                num(&f["objectives"]["satellites"]) as u64,
                shown(num(&f["objectives"]["availability_pct"])),
            )
        })
        .collect()
}

#[test]
fn the_user_guide_quotes_the_numbers_the_studies_give() {
    // A number in prose is a copy. Each one the guide quotes is re-derived here.
    let guide = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/DESIGN-OPTIMISER.md"),
    )
    .unwrap();
    let has = |needle: &str| assert!(guide.contains(needle), "the guide lacks: {needle}");

    // Whole Moon.
    let (_, g) = lunar();
    has("1 152 combinations");
    assert_eq!(g["space"]["grid_size"], 1152);
    has("removes 544 of them");
    has("The front has 12 designs at\n6 distinct points");
    assert_eq!(g["front_summary"]["designs"], 12);
    let points = front_points(g);
    assert_eq!(points.len(), 6);
    for (sats, avail) in &points {
        let cell = if avail == "100" {
            "100.0"
        } else {
            avail.as_str()
        };
        has(&format!("| {sats} | {cell} |"));
    }
    for f in g["front"].as_array().unwrap() {
        assert_eq!(
            num(&f["variables"]["altitude_km"]),
            8000.0,
            "highest altitude offered"
        );
    }
    has("its worst-site availability is\n0 %), which the open engine's coverage map of `front/004.toml` shows");
    let knee = &g["front"][3];
    assert_eq!(knee["id"], "004");
    assert_eq!(knee["knee"], true);
    let (kd, _) = direct(knee["scenario"].as_str().unwrap());
    assert_eq!(num(&kd["global"]["worst_site_availability_pct"]), 0.0);
    assert_eq!(num(&kd["global"]["availability_pct"]), 85.0404);
    let at_full = g["front"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| num(&f["objectives"]["availability_pct"]) == 100.0)
        .count();
    assert_eq!(at_full, 7, "seven designs reach it");
    has(&format!(
        "14 satellites and {} % availability, and two\nfront designs",
        shown(num(&g["reference"]["objectives"]["availability_pct"]))
    ));
    assert_eq!(g["reference"]["dominated_by"].as_array().unwrap().len(), 2);

    // South polar cap.
    let (_, p) = run(&study("lunar-navigation-south-polar.toml"), THREADS);
    assert_eq!(p["work"]["evaluations"], 608);
    assert_eq!(p["front_summary"]["designs"], 9);
    has("give a front of 9 designs at 4 points: 8 satellites 24.5261 %, 12 satellites 69.5825 %,\n14 satellites 91.6739 % (the knee) and 16 satellites 100 %");
    let expected: BTreeSet<(u64, String)> = [
        (8, "24.5261"),
        (12, "69.5825"),
        (14, "91.6739"),
        (16, "100"),
    ]
    .into_iter()
    .map(|(n, a)| (n, a.to_string()))
    .collect();
    assert_eq!(front_points(&p), expected);
    assert_eq!(num(&p["knee"]["objectives"]["satellites"]), 14.0);
    let r = &p["reference"];
    has("14 satellites and 96.7241 %: no front design dominates it, and it dominates the best\n14-satellite single-shell design");
    assert_eq!(shown(num(&r["objectives"]["availability_pct"])), "96.7241");
    assert_eq!(num(&r["objectives"]["satellites"]), 14.0);
    assert_eq!(r["on_front"], true);
    assert_eq!(r["dominated_by"], serde_json::json!([]));
    assert_eq!(r["dominates"], serde_json::json!(["003"]));
    assert_eq!(num(&p["front"][2]["objectives"]["satellites"]), 14.0);
    // The reference of the polar study, run directly.
    let polar_ref = Study::parse(&study("lunar-navigation-south-polar.toml"))
        .unwrap()
        .reference
        .unwrap()
        .scenario
        .unwrap();
    let (d, sha) = direct(&polar_ref);
    assert_eq!(r["result_sha256"].as_str().unwrap(), sha);
    assert_eq!(
        num(&r["objectives"]["availability_pct"]),
        num(&d["global"]["availability_pct"])
    );

    // Low Earth orbit.
    let (_, e) = run(&study("leo-positioning-earth.toml"), THREADS);
    has("1 782 combinations, 351\n  removed by the Walker constraint, 1 431 run, 1 141 of those kept off the front");
    assert_eq!(e["space"]["grid_size"], 1782);
    assert_eq!(e["work"]["infeasible"], 351);
    assert_eq!(e["work"]["evaluations"], 1431);
    assert_eq!(e["work"]["constraint_violated"], 1141);
    assert_eq!(e["work"]["failed"], 0);
    has("the front has 22 designs and its knee is 168 satellites, 90.0661 %\n  availability and a 95th-percentile PDOP of 8.81");
    assert_eq!(e["front_summary"]["designs"], 22);
    let knee = e["front"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["knee"] == true)
        .expect("the front has a knee");
    assert_eq!(num(&knee["objectives"]["satellites"]), 168.0);
    let (k, _) = direct(knee["scenario"].as_str().unwrap());
    assert_eq!(num(&k["global"]["pdop"]["p95"]), 8.81);
    assert_eq!(num(&k["global"]["availability_pct"]), 90.0661);
    assert_eq!(num(&knee["objectives"]["pdop_p95"]), 8.81);
    assert_eq!(num(&knee["objectives"]["availability_pct"]), 90.0661);

    // Mars.
    has("a Walker shell around Mars, 72 designs, a front of 8");
    let (_, m) = run(&study("mars-navigation-shell.toml"), THREADS);
    assert_eq!(m["work"]["evaluations"], 72);
    assert_eq!(m["front_summary"]["designs"], 8);

    // The budgeted search paragraph.
    has("seeds 1, 2 and 3 found 4, 5 and 3 of the 6\ntrue front points, and the hypervolume of the front found was 0.6217, 0.6347 and 0.6087\nagainst 0.6558 for the true front. With 300 runs they found 5, 6 and 5.");
    assert_eq!(shown(num(&g["hypervolume"]["value"])), "0.6558");
    let mut found = Vec::new();
    let mut volumes = Vec::new();
    for budget in [150, 300] {
        for seed in [1, 2, 3] {
            let src = with_search(
                &study("lunar-navigation-global.toml").replacen(
                    "seed = 1\n",
                    &format!("seed = {seed}\n"),
                    1,
                ),
                &format!(
                    "method = \"nsga2\"\nbudget = {budget}\npopulation = 30\ngenerations = 40"
                ),
            );
            let (_, v) = run(&src, THREADS);
            found.push(front_points(&v).intersection(&points).count());
            if budget == 150 {
                volumes.push(shown(num(&v["hypervolume"]["value"])));
            }
        }
    }
    assert_eq!(found, vec![4, 5, 3, 5, 6, 5]);
    assert_eq!(volumes, vec!["0.6217", "0.6347", "0.6087"]);

    // The ZDT1 figure and the published anchor are held by their own tests above.
    has("the mean distance to it is 0.00018 on the development machine (the bar, fixed before the\nfirst run, is 0.05)");
    has("a global HDOP median of 0.94, a 90th\npercentile of 1.165 and a 95th percentile of 1.255 (published: 0.94, 1.16 and 1.25) and\na PDOP median of 1.795");
}

// ─────────────────────────────── 15. defects found by the independent verification

#[test]
fn an_unknown_key_is_an_error_and_never_a_silently_dropped_constraint() {
    // Before the fix a misspelt table was ignored: `[[constraints]]` (plural) dropped the
    // constraint and the study still returned a front, as if the constraint held.
    let good = study("link-budget-trade.toml");
    let cases: [(String, &str); 9] = [
        (
            good.replace(
                "[[objective]]\nname = \"margin_db\"",
                "[[constraints]]\nresult = { path = \"margin_db\", op = \">=\", value = 1.0e9 }\n\n[[objective]]\nname = \"margin_db\"",
            ),
            "constraints",
        ),
        (good.replace("seed = 1\n", "sead = 1\n"), "sead"),
        (
            good.replace("step = 5.0", "step = 5.0\nunitt = \"dBW\""),
            "unitt",
        ),
        (
            good.replace("budget = 100", "budget = 100\nbuget = 10"),
            "buget",
        ),
        (
            good.replace(
                "label = \"Bundled link budget (55 dBW, 1 Mbit/s)\"",
                "label = \"Bundled link budget (55 dBW, 1 Mbit/s)\"\nnote = \"x\"",
            ),
            "note",
        ),
        (
            good.replace("sense = \"max\"", "sense = \"max\"\nweight = 2.0"),
            "weight",
        ),
        (
            good.replace(
                "[[objective]]\nname = \"margin_db\"",
                "[[constraint]]\nresult = { path = \"margin_db\", op = \">=\", value = 0.0, valeu = 1.0 }\n\n[[objective]]\nname = \"margin_db\"",
            ),
            "valeu",
        ),
        (
            study("mars-navigation-shell.toml").replace(
                "walker = { total = \"total\", planes = \"planes\" }",
                "walker = { total = \"total\", planes = \"planes\", phase = \"planes\" }",
            ),
            "phase",
        ),
        (
            good.replace(
                "kind = \"pro-design-optimise\"",
                "kind = \"pro-trade-study\"",
            ),
            "pro-trade-study",
        ),
    ];
    for (src, word) in &cases {
        assert_ne!(src, &good, "the case for '{word}' changed nothing");
        let e = error_of(src);
        assert!(e.contains(word), "'{word}' is not named in: {e}");
    }
    // The registry route (`kshana-pro run`) refuses the same file.
    let e = match kshana_pro::registry::run(&cases[0].0) {
        Ok(_) => panic!("the registry ran a study with a misspelt table"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("constraints"), "{e}");
    // A study without the `kind` line is still a study for `kshana-pro design`.
    let without_kind = good.replace("kind = \"pro-design-optimise\"\n", "");
    assert_ne!(without_kind, good);
    let hash = |src: &str| Study::parse(src).unwrap().resolve().unwrap().scenario_hash;
    assert_eq!(hash(&without_kind), hash(&good));
}

#[test]
fn the_engines_own_unit_is_never_replaced_by_a_unit_written_in_the_study() {
    // Before the fix `unit = "W"` on an objective or a variable relabelled a number the
    // engine states in dB or dBW.
    let src = study("link-budget-trade.toml")
        .replace("path = \"margin_db\"", "path = \"margin_db\"\nunit = \"W\"")
        .replace("step = 5.0", "step = 5.0\nunit = \"W\"");
    assert!(src.matches("unit = \"W\"").count() == 2);
    let (_, v) = run(&src, 2);
    for group in ["evaluated[]", "front[]", "knee", "reference"] {
        let o = &v["units"][format!("{group}.objectives.margin_db")];
        assert_eq!(o["unit"], "dB", "{group}");
        assert!(
            o["note"]
                .as_str()
                .unwrap()
                .contains("the study file states 'W'"),
            "{o}"
        );
        let x = &v["units"][format!("{group}.variables.eirp_dbw")];
        assert_eq!(x["unit"], "dBW", "{group}");
        assert!(
            x["note"]
                .as_str()
                .unwrap()
                .contains("the study file states 'W'"),
            "{x}"
        );
        // The objective that is the variable follows the variable.
        assert_eq!(
            v["units"][format!("{group}.objectives.eirp_dbw")]["unit"],
            "dBW"
        );
    }
    assert_eq!(v["objectives"][0]["unit"], "dB");
    assert_eq!(v["objectives"][1]["unit"], "dBW");
    assert_eq!(v["space"]["variables"][0]["unit"], "dBW");
    // Where the engine states no unit, the study's unit is used, as the guide says.
    let lunar = &lunar().1;
    assert_eq!(
        lunar["units"]["front[].variables.altitude_km"]["unit"],
        "km"
    );
    assert!(!lunar["units"]["front[].variables.altitude_km"]["note"]
        .as_str()
        .unwrap()
        .contains("the study file states"));
}

#[test]
fn an_existing_front_folder_keeps_the_files_the_tool_did_not_write() {
    // Before the fix the whole `front/` folder of the output directory was deleted.
    let d = tmp("keep");
    let front = d.join("front");
    std::fs::create_dir_all(&front).unwrap();
    std::fs::write(front.join("notes.txt"), "mine\n").unwrap();
    std::fs::write(front.join("my-design.toml"), "kind = \"clock\"\n").unwrap();
    // Left by an earlier run with a longer front: these two must go.
    std::fs::write(front.join("999.toml"), "stale\n").unwrap();
    std::fs::write(front.join("index.csv"), "stale\n").unwrap();
    let out = bin()
        .arg("design")
        .arg(dir().join("link-budget-trade.toml"))
        .arg("--out")
        .arg(&d)
        .arg("--dev")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(front.join("notes.txt")).unwrap(),
        "mine\n"
    );
    assert_eq!(
        std::fs::read_to_string(front.join("my-design.toml")).unwrap(),
        "kind = \"clock\"\n"
    );
    assert!(
        !front.join("999.toml").exists(),
        "a stale front file stayed"
    );
    let csv = std::fs::read_to_string(front.join("index.csv")).unwrap();
    assert!(csv.starts_with("id,file,knee,"), "{csv}");
    let mut names: Vec<String> = std::fs::read_dir(&front)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "001.toml",
            "002.toml",
            "003.toml",
            "004.toml",
            "005.toml",
            "index.csv",
            "my-design.toml",
            "notes.txt"
        ]
    );
}

#[test]
fn the_report_never_shows_a_small_number_as_zero() {
    // Before the fix the report and the chart rounded to four decimals, so a ground
    // sample distance of 0.000004 m read "0".
    let src = r#"kind = "pro-design-optimise"
title = "small numbers"
base_scenario = """
kind = "eo-coverage"
altitude_km = 700.0
half_fov_deg = 7.5
ifov_microrad = 14.0
max_off_nadir_deg = 45.0
"""

[[variable]]
name = "altitude_km"
key = "altitude_km"
type = "choice"
values = [400.0, 500.0]

[[variable]]
name = "ifov_microrad"
key = "ifov_microrad"
type = "choice"
values = [0.00001, 0.00002]

[[objective]]
name = "nadir_gsd_m"
sense = "min"
path = "nadir_gsd_m"

[[objective]]
name = "swath_width_km"
sense = "max"
path = "swath_width_km"
"#;
    let out = run_study(src, &Options { threads: Some(2) }).unwrap();
    let v: Value = serde_json::from_str(&out.json).unwrap();
    let front = v["front"].as_array().unwrap();
    assert_eq!(front.len(), 2);
    let gsd = num(&front[0]["objectives"]["nadir_gsd_m"]);
    // The engine's number, from a direct run of the front scenario: 400 km x 0.00001 microrad.
    let (d, _) = direct(front[0]["scenario"].as_str().unwrap());
    assert_eq!(gsd, num(&d["nadir_gsd_m"]));
    assert!((gsd - 4.0e-6).abs() < 1e-15, "{gsd}");
    assert!(
        out.html.contains("<td class=\"n\">4e-6</td>"),
        "the report does not show 4e-6"
    );
    assert!(
        out.html.contains("<td class=\"n\">1e-5</td>"),
        "the instantaneous field of view"
    );
    assert!(
        !out.html.contains("<td class=\"n\">0</td>"),
        "a non-zero number is shown as 0"
    );
    assert!(
        out.svg.contains("nadir_gsd_m 4e-6"),
        "the chart's hover text"
    );
    assert!(!out.svg.contains("nadir_gsd_m 0 "), "the chart shows 0");
}
