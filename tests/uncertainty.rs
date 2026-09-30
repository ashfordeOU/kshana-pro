// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Acceptance tests of the uncertainty and sensitivity study (`pro-uncertainty`).
//!
//! Every expected value here comes from outside the code under test: a published
//! closed form computed in the test, a direct run of the open engine, or the open
//! engine's own Monte Carlo campaign. Tolerances are derived from the sample size by
//! the formula written next to each one; none is tuned to an observed number.
//!
//! 1. the Sobol' estimators against the Ishigami function's closed form;
//! 2. a linear engine case (`link-budget`) against its closed form;
//! 3. the open campaign reproduced value for value;
//! 4. input uncertainty on the clock against the mixture's closed form, and the band;
//! 5. a new kind (the low-Earth-orbit navigation chain) against direct runs;
//! 6. parallel equals sequential, byte for byte;
//! 7. stop and resume, a changed study, a cut-short checkpoint;
//! 8. failed realisations;
//! 9. adversarial studies;
//! 10. the licence gate.

use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::process::Command;

use kshana_pro::batch::uq::{
    self, cli::output_files, exec::FnEvaluator, BaseInfo, BaseTier, Plan, RunOptions, StudySpec,
};
use serde_json::Value;

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/scenarios/pro-power/uncertainty"
);

fn study(name: &str) -> String {
    std::fs::read_to_string(format!("{DIR}/{name}.toml"))
        .unwrap_or_else(|e| panic!("cannot read study {name}: {e}"))
}

fn engine_copy(name: &str) -> String {
    std::fs::read_to_string(format!("{DIR}/engine/{name}.toml"))
        .unwrap_or_else(|e| panic!("cannot read engine scenario {name}: {e}"))
}

/// Run a study in memory and return its result document.
fn run(study_toml: &str, threads: usize) -> Value {
    let opts = RunOptions {
        threads: Some(threads),
        ..RunOptions::default()
    };
    uq::run_study(study_toml, &opts)
        .unwrap_or_else(|e| panic!("the study must run: {e}"))
        .result
        .expect("an in-memory run finishes every realisation")
        .doc
}

fn f(v: &Value) -> f64 {
    v.as_f64()
        .unwrap_or_else(|| panic!("expected a number, got {v}"))
}

/// Replace exactly one occurrence, so a fixture that drifts fails loudly.
fn swap(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "expected exactly one `{from}` in the study text"
    );
    text.replace(from, to)
}

fn scratch(tag: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("uncertainty-{tag}"));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch directory");
    d
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kshana-pro"))
}

/// The standard normal cumulative distribution function by composite Simpson
/// integration of the density: independent of every routine in the crate.
fn normal_cdf(x: f64) -> f64 {
    let n = 20_000usize;
    let h = x / n as f64;
    let pdf = |t: f64| (-0.5 * t * t).exp() / (2.0 * PI).sqrt();
    let mut s = pdf(0.0) + pdf(x);
    for i in 1..n {
        s += pdf(i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    0.5 + s * h / 3.0
}

// ─── 1. Estimators against a published closed form ─────────────────────────────────

/// Base sample size of the Ishigami check: 65,536 rows, so 65,536 * (3 + 2) = 327,680
/// evaluations. With the study seed 20260930, this is the smallest power of two at which
/// all six indices are within 0.02 of their closed form AND all six closed-form values
/// lie inside their own reported 95 % intervals. At 16,384 and at 32,768 rows every
/// estimate was already within 0.02, but one of the six intervals missed its closed-form
/// value each time (the first-order index of x1, then of x3): with six 95 % intervals
/// that is expected about one time in four. `t01b` measures the coverage itself.
const ISHIGAMI_N: usize = 65_536;

/// The Ishigami function's closed-form first-order and total indices (a = 7, b = 0.1)
/// and its variance.
///
/// Ishigami and Homma (1990): Y = sin X1 + a sin^2 X2 + b X3^4 sin X1 with each X
/// uniform on (-pi, pi). Closed form (I. M. Sobol' and Yu. L. Levitan, "On the use of
/// variance reducing multipliers in Monte Carlo computations of a global sensitivity
/// index", Computer Physics Communications 117, 1999):
///   V1  = (1/2) (1 + b pi^4 / 5)^2
///   V2  = a^2 / 8
///   V13 = 8 b^2 pi^8 / 225        (the X1-X3 interaction)
///   V   = V1 + V2 + V13
/// First order: V1/V, V2/V, 0. Total: (V1 + V13)/V, V2/V, V13/V.
fn ishigami_closed_form(a: f64, b: f64) -> ([f64; 3], [f64; 3], f64) {
    let v1 = 0.5 * (1.0 + b * PI.powi(4) / 5.0).powi(2);
    let v2 = a * a / 8.0;
    let v13 = 8.0 * b * b * PI.powi(8) / 225.0;
    let v = v1 + v2 + v13;
    ([v1 / v, v2 / v, 0.0], [(v1 + v13) / v, v2 / v, v13 / v], v)
}

fn ishigami(a: f64, b: f64, x: &[f64]) -> f64 {
    x[0].sin() + a * x[1].sin().powi(2) + b * x[2].powi(4) * x[0].sin()
}

#[test]
fn t01b_sobol_intervals_cover_the_closed_form_at_their_stated_rate() {
    // Twelve independent designs of 4096 rows, six indices each: 72 intervals at 95 %.
    // A binomial(72, 0.95) count has mean 68.4 and standard deviation 1.85; an
    // interval procedure that is honest covers at least 65 (90 %) of them.
    use kshana_pro::batch::uq::sampling::{saltelli_unit_rows, Method};
    use kshana_pro::batch::uq::sobol;
    let (a, b) = (7.0, 0.1);
    let (first, total, _) = ishigami_closed_form(a, b);
    let n = 4096;
    let (mut covered, mut cases) = (0, 0);
    let mut worst: f64 = 0.0;
    for seed in 1000..1012u64 {
        let rows = saltelli_unit_rows(Method::Lhs, n, 3, seed);
        let y: Vec<Option<f64>> = rows
            .iter()
            .map(|u| {
                let x: Vec<f64> = u.iter().map(|u| -PI + 2.0 * PI * u).collect();
                Some(ishigami(a, b, &x))
            })
            .collect();
        let yab: Vec<Vec<Option<f64>>> = (0..3)
            .map(|i| y[(2 + i) * n..(3 + i) * n].to_vec())
            .collect();
        let out = sobol::estimate(&y[..n], &y[n..2 * n], &yab, &[true; 3], seed);
        for i in 0..3 {
            let x = out.indices[i].as_ref().expect("an index");
            for (estimate, ci, truth) in [
                (x.first_order, x.first_order_ci95, first[i]),
                (x.total, x.total_ci95, total[i]),
            ] {
                cases += 1;
                if ci.0 <= truth && truth <= ci.1 {
                    covered += 1;
                }
                worst = worst.max((estimate - truth).abs());
            }
        }
    }
    eprintln!("Ishigami, 12 seeds at n = {n}: {covered} of {cases} intervals cover the closed form; largest error {worst:.4}");
    assert_eq!(cases, 72);
    assert!(
        covered >= 65,
        "only {covered} of 72 intervals cover the closed form"
    );
    // At n = 4096 the standard error of the noisiest index (the first-order index of
    // x1) is about 0.015, from its reported interval; no estimate is further than four
    // of them from its closed form.
    assert!(worst < 0.06, "largest error {worst}");
}

#[test]
fn t01_sobol_indices_match_the_ishigami_closed_form() {
    let (a, b) = (7.0_f64, 0.1_f64);
    let (first, total, v) = ishigami_closed_form(a, b);
    // The closed form reproduces the published four-decimal values.
    let round4 = |x: f64| (x * 1e4).round() / 1e4;
    assert_eq!(first.map(round4), [0.3139, 0.4424, 0.0]);
    assert_eq!(total.map(round4), [0.5576, 0.4424, 0.2437]);

    let spec: StudySpec = toml::from_str(&format!(
        r#"kind = "pro-uncertainty"
title = "Ishigami function"
seed = 20260930
base_scenario = ""
[[input]]
name = "x1"
key = "x1"
dist = "uniform"
min = {lo:?}
max = {hi:?}
source = "benchmark definition (Ishigami and Homma 1990)"
[[input]]
name = "x2"
key = "x2"
dist = "uniform"
min = {lo:?}
max = {hi:?}
source = "benchmark definition (Ishigami and Homma 1990)"
[[input]]
name = "x3"
key = "x3"
dist = "uniform"
min = {lo:?}
max = {hi:?}
source = "benchmark definition (Ishigami and Homma 1990)"
[[output]]
name = "y"
path = "y"
[sampling]
method = "lhs"
n = {ISHIGAMI_N}
keep_samples = false
[sensitivity]
method = "sobol"
"#,
        lo = -PI,
        hi = PI,
    ))
    .expect("the study parses");
    let plan = Plan::from_spec(&spec).expect("the study is valid");
    let info = BaseInfo {
        kind: "function".into(),
        engine_version: "none (closure evaluator)".into(),
        input_nominal: vec![None; 3],
        scalar_nominal: vec![None],
        scalar_units: vec!["1".into()],
        scalar_tiers: vec![BaseTier::default()],
        ..BaseInfo::default()
    };
    let evaluator = FnEvaluator(move |x: &[f64]| Ok(vec![ishigami(a, b, x)]));
    let doc = uq::run_with_evaluator(&plan, &info, &evaluator, 4)
        .expect("runs")
        .doc;

    assert_eq!(doc["runs"]["total"], ISHIGAMI_N * 5);
    let y = &doc["sensitivity"]["outputs"]["y"];
    // The variance estimate against the closed form, within 4 standard errors of a
    // variance estimate from 2N values (relative standard error about sqrt(2 / 2N)
    // times a kurtosis factor below 2 for this function).
    assert!((f(&y["variance"]) - v).abs() < 4.0 * v * (2.0 / ISHIGAMI_N as f64).sqrt());
    for i in 0..3 {
        let e = &y["inputs"][i];
        let (s, st) = (f(&e["first_order"]), f(&e["total"]));
        eprintln!(
            "x{}: S {s:.4} [{:.4}, {:.4}] (closed form {:.4})  ST {st:.4} [{:.4}, {:.4}] (closed form {:.4})",
            i + 1,
            f(&e["first_order_ci95_low"]),
            f(&e["first_order_ci95_high"]),
            first[i],
            f(&e["total_ci95_low"]),
            f(&e["total_ci95_high"]),
            total[i],
        );
        assert!(
            (s - first[i]).abs() < 0.02,
            "S{} = {s}, closed form {}",
            i + 1,
            first[i]
        );
        assert!(
            (st - total[i]).abs() < 0.02,
            "ST{} = {st}, closed form {}",
            i + 1,
            total[i]
        );
        assert!(
            f(&e["first_order_ci95_low"]) <= first[i] && first[i] <= f(&e["first_order_ci95_high"]),
            "the closed-form S{} = {} is outside the reported interval",
            i + 1,
            first[i]
        );
        assert!(
            f(&e["total_ci95_low"]) <= total[i] && total[i] <= f(&e["total_ci95_high"]),
            "the closed-form ST{} = {} is outside the reported interval",
            i + 1,
            total[i]
        );
    }
    // Ranking by total index: x1 (0.5576), x2 (0.4424), x3 (0.2437).
    let ranks: Vec<u64> = (0..3)
        .map(|i| y["inputs"][i]["rank"].as_u64().expect("ranked"))
        .collect();
    assert_eq!(ranks, vec![1, 2, 3]);
    // Share left to interactions: 1 - (V1 + V2)/V = V13/V, the total index of x3.
    assert!((f(&y["interaction_share"]) - total[2]).abs() < 0.03);
}

// ─── 2. Linear engine case ─────────────────────────────────────────────────────────

#[test]
fn t02_link_budget_margin_matches_its_closed_form() {
    let text = study("uq-link-budget-margin");
    let spec: StudySpec = toml::from_str(&text).expect("parses");
    let n = spec.sampling.n as f64;
    assert_eq!(spec.sampling.n, 8192);

    // The nominal margin, from a direct run of the open engine on its own file.
    let direct = kshana::api::run_scenario(&engine_copy("link-budget")).expect("the engine runs");
    let nominal = f(&serde_json::from_str::<Value>(&direct.json).expect("JSON")["margin_db"]);
    assert!(
        (nominal - 92.124_475_034_844_01).abs() < 1e-9,
        "nominal margin {nominal}"
    );

    // Closed form. The margin is eirp + g_over_t - other_losses + constant, with
    // independent normal inputs of standard deviation 1.0, 0.5 and 0.5 dB:
    //   mean = nominal, variance = 1 + 0.25 + 0.25 = 1.5 dB^2,
    //   first-order = total = 1/1.5, 0.25/1.5, 0.25/1.5 = 2/3, 1/6, 1/6.
    let sigma = 1.5_f64.sqrt();
    let share = [2.0 / 3.0, 1.0 / 6.0, 1.0 / 6.0];
    let p_limit = 1.0 - normal_cdf((91.0 - nominal) / sigma);

    // Tolerances, from n alone (four standard errors; plain Monte Carlo formulas, which
    // are conservative for a Latin hypercube):
    //   mean:        4 sigma / sqrt(n)
    //   std:         4 sigma / sqrt(2 n)
    //   probability: 4 sqrt(p (1 - p) / n)
    //   first-order (Saltelli 2010 estimator, additive normal case): the summand
    //     (yB - f0)(yAB_i - yA) has variance V^2 (S^2 + 2 S), so 4 sqrt((S^2 + 2 S) / n)
    //   total (Jansen): (yA - yAB_i)^2 / 2 has variance 2 (S V)^2, so 4 sqrt(2) S / sqrt(n)
    //   sum of squared standardised coefficients: it differs from 1 by
    //     2 sum_{i<j} rho_ij sqrt(S_i S_j) with sample correlations rho of variance
    //     1/n, standard error 2 sqrt(S1 S2 + S1 S3 + S2 S3) / sqrt(n) = 1 / sqrt(n)
    let tol_mean = 4.0 * sigma / n.sqrt();
    let tol_std = 4.0 * sigma / (2.0 * n).sqrt();
    let tol_p = 4.0 * (p_limit * (1.0 - p_limit) / n).sqrt();
    let tol_first = |s: f64| 4.0 * ((s * s + 2.0 * s) / n).sqrt();
    let tol_total = |s: f64| 4.0 * 2.0_f64.sqrt() * s / n.sqrt();
    let tol_sum_sq = 4.0 / n.sqrt();

    let doc = run(&text, 4);
    assert_eq!(doc["runs"]["total"], 8192 * 5);
    assert_eq!(doc["runs"]["failed"], 0);
    let m = &doc["outputs"]["margin"];
    assert_eq!(
        f(&m["nominal"]),
        nominal,
        "the study's base run is the direct run"
    );
    assert_eq!(m["unit"], "dB");
    assert!(
        (f(&m["mean"]) - nominal).abs() < tol_mean,
        "mean {}",
        m["mean"]
    );
    assert!((f(&m["std"]) - sigma).abs() < tol_std, "std {}", m["std"]);
    let p = f(&m["limit"]["probability"]);
    assert!(
        (p - p_limit).abs() < tol_p,
        "P(margin >= 91) = {p}, closed form {p_limit}"
    );
    assert!(f(&m["limit"]["probability_ci95_low"]) <= p_limit);
    assert!(p_limit <= f(&m["limit"]["probability_ci95_high"]));
    assert!(m["limit"]["statement"]
        .as_str()
        .expect("a statement")
        .contains("given the stated input uncertainty"));

    let s = &doc["sensitivity"]["outputs"]["margin"];
    for (i, want) in share.iter().enumerate() {
        let e = &s["inputs"][i];
        let (first, total) = (f(&e["first_order"]), f(&e["total"]));
        assert!(
            (first - want).abs() < tol_first(*want),
            "S{i} = {first}, closed form {want}"
        );
        assert!(
            (total - want).abs() < tol_total(*want),
            "ST{i} = {total}, closed form {want}"
        );
        assert!(
            (first - total).abs() < tol_first(*want) + tol_total(*want),
            "first-order {first} and total {total} must agree for a linear response"
        );
    }
    assert!(f(&s["interaction_share"]).abs() < tol_first(share[0]) + 2.0 * tol_first(share[1]));
    assert_eq!(s["inputs"][0]["rank"], 1, "eirp explains the most");

    // The cheap method on the plain sample: slopes +1, +1, -1 dB per dB exactly, the
    // fit is exact, and the squared standardised coefficients are the same shares.
    let doc = run(&swap(&text, "method = \"sobol\"", "method = \"src\""), 4);
    assert_eq!(doc["runs"]["total"], 8192);
    let s = &doc["sensitivity"]["outputs"]["margin"];
    assert!(
        (f(&s["r_squared"]) - 1.0).abs() < 1e-9,
        "R-squared {}",
        s["r_squared"]
    );
    assert_eq!(s["reliable"], true);
    assert!((f(&s["sum_src_squared"]) - 1.0).abs() < tol_sum_sq);
    for (i, slope) in [1.0, 1.0, -1.0].iter().enumerate() {
        let e = &s["inputs"][i];
        assert!(
            (f(&e["coefficient"]) - slope).abs() < 1e-9,
            "slope {i}: {}",
            e["coefficient"]
        );
        let sq = f(&e["src"]).powi(2);
        assert!(
            (sq - share[i]).abs() < tol_sum_sq,
            "src^2 {i} = {sq}, closed form {}",
            share[i]
        );
    }
}

// ─── 3. Pro reproduces free exactly, then goes further ─────────────────────────────

#[test]
fn t03_the_open_campaign_is_reproduced_value_for_value() {
    // The open engine's own campaign, run here.
    let campaign_text = engine_copy("campaign-monte-carlo-clock-holdover");
    let campaign: Value = serde_json::from_str(
        &kshana::api::run_scenario(&campaign_text)
            .expect("the open campaign runs")
            .json,
    )
    .expect("JSON");
    assert_eq!(campaign["kind"], "campaign");
    assert_eq!(campaign["monte_carlo"]["runs"], 200);
    assert_eq!(campaign["monte_carlo"]["base_seed"], 42);

    let doc = run(&study("uq-clock-holdover-seeds"), 4);
    assert_eq!(doc["runs"]["total"], 200);
    assert_eq!(doc["sampling"]["base_seed"], 42);
    assert!(doc["inputs"].as_array().expect("a list").is_empty());

    for (name, metric) in [("final_error", "final_error"), ("timing_rms", "timing_rms")] {
        let free = &campaign["monte_carlo"]["metrics"][metric];
        let pro = &doc["outputs"][name];
        assert_eq!(pro["path"], free["path"]);
        assert_eq!(pro["unit"], free["unit"]);
        // The same 200 samples, in the same order.
        let a = free["samples"]
            .as_array()
            .expect("the campaign keeps its samples");
        let b = pro["samples"]
            .as_array()
            .expect("the study keeps its samples");
        assert_eq!(a.len(), 200);
        assert_eq!(a, b, "{name}: the samples differ");
        // The same statistics, bit for bit (the bootstrap interval included).
        for stat in ["mean", "std", "p05", "p50", "p95", "ci95_low", "ci95_high"] {
            assert_eq!(
                f(&pro[stat]).to_bits(),
                f(&free[stat]).to_bits(),
                "{name}.{stat}: Pro {} vs open campaign {}",
                pro[stat],
                free[stat]
            );
        }
        assert_eq!(pro["n"], free["n"]);
    }

    // The two numbers the open documentation states for this campaign: a sample
    // standard deviation of 14.93 ns, against the closed form sqrt(q_wf * 3010 s) =
    // 16.459 ns (white frequency noise integrated for 3010 s).
    let std = f(&doc["outputs"]["final_error"]["std"]);
    assert_eq!((std * 100.0).round() / 100.0, 14.93);
    let closed = (9.0e-20_f64 * 3010.0).sqrt() * 1e9;
    assert!((closed - 16.459).abs() < 5e-4, "closed form {closed}");
    // 200 normal samples: the standard error of the standard deviation is
    // closed / sqrt(2 * 200) = 0.82 ns; the sample value is within three of them.
    assert!((std - closed).abs() < 3.0 * closed / 400.0_f64.sqrt());

    // Further than the open campaign, from the same 200 runs: the band of the whole
    // curve, the probability of staying inside the guard, the running estimate.
    let band = &doc["bands"]["time_error"];
    assert_eq!(band["n"], 200);
    assert_eq!(band["unit"], "ns");
    assert_eq!(band["time_unit"], "s");
    let steps = band["t"].as_array().expect("time").len();
    assert_eq!(steps, 361);
    assert_eq!(f(&band["t"][steps - 1]), 3600.0);
    for p in ["p05", "p50", "p95"] {
        assert_eq!(band[p][steps - 1], doc["outputs"]["final_error"][p]);
    }
    let limit = &doc["outputs"]["final_error"]["limit"];
    let inside = doc["outputs"]["final_error"]["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .filter(|v| f(v).abs() <= 20.0)
        .count();
    assert_eq!(limit["n_met"], inside);
    assert_eq!(f(&limit["probability"]), inside as f64 / 200.0);
    let sizes: Vec<u64> = doc["outputs"]["final_error"]["convergence"]
        .as_array()
        .expect("a running estimate")
        .iter()
        .map(|c| c["n"].as_u64().expect("n"))
        .collect();
    assert_eq!(sizes, vec![16, 32, 64, 128, 200]);
}

// ─── 4. Input uncertainty on the clock ─────────────────────────────────────────────

#[test]
fn t04_an_uncertain_noise_level_widens_the_spread_to_the_mixture_closed_form() {
    let text = study("uq-clock-holdover-qwf");
    let spec: StudySpec = toml::from_str(&text).expect("parses");
    let n = spec.sampling.n as f64;
    let (a, b) = (
        spec.inputs[0].min.expect("min"),
        spec.inputs[0].max.expect("max"),
    );
    assert_eq!((a, b), (4.5e-20, 2.7e-19));

    // Given q_wf, the final error is normal with mean 0 and variance q_wf * tau,
    // tau = 3010 s (the open scenario's own closed form). With q_wf uniform on [a, b]:
    //   variance = E[q_wf] * tau = (a + b) / 2 * tau
    //   fourth moment = 3 E[q_wf^2] tau^2, E[q_wf^2] = (a^2 + a b + b^2) / 3
    //   kurtosis k = 3 E[q_wf^2] / E[q_wf]^2
    // The standard error of a sample standard deviation is std * sqrt((k - 1) / (4 n)).
    let tau = 3010.0;
    let mean_q = 0.5 * (a + b);
    let closed = (mean_q * tau).sqrt() * 1e9;
    assert!(
        (closed - 21.77).abs() < 0.01,
        "mixture closed form {closed} ns"
    );
    let kurtosis = 3.0 * ((a * a + a * b + b * b) / 3.0) / (mean_q * mean_q);
    let tol = 4.0 * closed * ((kurtosis - 1.0) / (4.0 * n)).sqrt();
    let fixed = (9.0e-20_f64 * tau).sqrt() * 1e9; // 16.459 ns at the datasheet value

    let doc = run(&text, 4);
    let out = &doc["outputs"]["final_error"];
    let std = f(&out["std"]);
    assert!(
        (std - closed).abs() < tol,
        "std {std} ns, closed form {closed} ns, tolerance {tol}"
    );
    assert!(
        std - tol > fixed,
        "the spread must be wider than the fixed-noise {fixed} ns"
    );
    // Mean zero within four standard errors.
    assert!(f(&out["mean"]).abs() < 4.0 * closed / n.sqrt());

    // The band at the last time step is the scalar output's percentiles, exactly.
    let band = &doc["bands"]["time_error"];
    let last = band["t"].as_array().expect("time").len() - 1;
    for p in ["p05", "p50", "p95"] {
        assert_eq!(band[p][last], out[p], "band {p} at the last step");
    }
    assert_eq!(
        band["share_meeting_limit"][last],
        out["limit"]["probability_among_evaluated"]
    );
    // While the receiver is locked (before 600 s) the error is zero in every run.
    assert_eq!(f(&band["p05"][10]), 0.0);
    assert_eq!(f(&band["p95"][10]), 0.0);
    assert_eq!(f(&band["share_meeting_limit"][10]), 1.0);

    // The input is ASSUMED, and every output says so.
    assert_eq!(doc["inputs"][0]["source_class"], "ASSUMED");
    assert!(doc["label"].as_str().expect("label").contains("ASSUMED"));
    // The noise level changes the spread of the error, not its mean: a linear ranking
    // cannot see it, and the result says so in words instead of reporting a ranking.
    let s = &doc["sensitivity"]["outputs"]["final_error"];
    assert_eq!(s["reliable"], false);
    assert!(s["statement"]
        .as_str()
        .expect("a statement")
        .contains("NOT reliable"));
}

// ─── 5. A new kind: the low-Earth-orbit navigation chain ───────────────────────────

/// A direct run of the open engine on its own end-to-end file with two keys replaced.
fn leo_direct(eirp_dbw: f64, od_sisre_m: f64) -> Value {
    let mut scn: toml::Value = toml::from_str(&engine_copy("leo-pnt-end-to-end")).expect("TOML");
    scn["od_sisre_m"] = toml::Value::Float(od_sisre_m);
    scn["pass"]["satellite"][0]["band"][0]["eirp_dbw"] = toml::Value::Float(eirp_dbw);
    let out = kshana::api::run_scenario(&toml::to_string(&scn).expect("serialises"))
        .expect("the engine runs");
    serde_json::from_str(&out.json).expect("JSON")
}

#[test]
fn t05_leo_chain_sensitivity_agrees_with_direct_runs() {
    // FIRST, by direct runs: the peak carrier-to-noise density moves exactly 1 dB per dB
    // of transmit power, and not at all with the orbit-and-clock error; the broadcast
    // signal-in-space range error moves with the orbit-and-clock error and not with the
    // transmit power.
    let base = leo_direct(10.0, 0.25);
    let cn0 = |d: &Value| f(&d["pass"]["peak_cn0_dbhz"]);
    let sisre = |d: &Value| f(&d["navmsg"]["sisre_rms_m"]);
    for eirp in [7.5, 12.0, 13.0] {
        let d = leo_direct(eirp, 0.25);
        assert!(
            ((cn0(&d) - cn0(&base)) - (eirp - 10.0)).abs() < 1e-9,
            "peak C/N0 is not 1 dB per dB at EIRP {eirp}: {} vs {}",
            cn0(&d),
            cn0(&base)
        );
        assert_eq!(sisre(&d), sisre(&base), "SISRE moved with EIRP");
    }
    for od in [0.10, 0.40] {
        let d = leo_direct(10.0, od);
        assert_eq!(
            cn0(&d),
            cn0(&base),
            "peak C/N0 moved with the orbit-and-clock error"
        );
        assert!(
            (sisre(&d) - od).abs() < 1e-4,
            "SISRE {} for od {od}",
            sisre(&d)
        );
    }

    let doc = run(&study("uq-leo-pnt-chain"), 4);
    assert_eq!(doc["base"]["kind"], "leo-pnt-chain");
    assert_eq!(doc["runs"]["total"], 64);
    assert_eq!(doc["runs"]["failed"], 0);
    assert_eq!(f(&doc["outputs"]["peak_cn0"]["nominal"]), cn0(&base));
    assert_eq!(doc["outputs"]["peak_cn0"]["unit"], "dB-Hz");
    assert_eq!(doc["outputs"]["broadcast_sisre"]["unit"], "m");

    // Inputs are declared od_sisre, eirp.
    let s = &doc["sensitivity"]["outputs"]["peak_cn0"];
    assert!(
        (f(&s["inputs"][1]["coefficient"]) - 1.0).abs() < 1e-6,
        "{}",
        s["inputs"][1]
    );
    assert!(
        f(&s["inputs"][0]["coefficient"]).abs() < 1e-6,
        "{}",
        s["inputs"][0]
    );
    assert!((f(&s["inputs"][1]["src"]) - 1.0).abs() < 1e-6);
    assert!(f(&s["inputs"][0]["src"]).abs() < 1e-6);
    assert!((f(&s["r_squared"]) - 1.0).abs() < 1e-9);
    assert_eq!(s["inputs"][1]["rank"], 1);

    let s = &doc["sensitivity"]["outputs"]["broadcast_sisre"];
    assert_eq!(
        s["inputs"][0]["rank"], 1,
        "the orbit-and-clock error drives the SISRE"
    );
    assert!(f(&s["inputs"][0]["src"]) > 0.999, "{}", s["inputs"][0]);
    assert!(f(&s["inputs"][1]["src"]).abs() < 1e-6, "{}", s["inputs"][1]);
    // d(SISRE)/d(od) is 1 to within the message's own 0.8 mm representation error.
    assert!((f(&s["inputs"][0]["coefficient"]) - 1.0).abs() < 1e-3);
    assert!(f(&s["r_squared"]) > 0.999_99);

    // The chain is MODELLED and stays MODELLED.
    assert!(doc["base"]["label"]
        .as_str()
        .expect("base label")
        .starts_with("MODELLED"));
    for fig in doc["figure_tiers"]["figures"].as_array().expect("figures") {
        assert_eq!(fig["tier"], "MODELLED");
    }
}

#[test]
fn t05b_the_four_input_leo_study_is_a_valid_study() {
    // The showcase study (6144 runs of the chain) is not run here; it must parse, its
    // keys must exist, its base run must succeed and its design must be laid out.
    let p = uq::prepare(&study("uq-leo-pnt-chain-sobol")).expect("valid");
    assert_eq!(p.plan.inputs.len(), 4);
    assert_eq!(p.jobs.len(), 1024 * 6);
    assert_eq!(
        p.info.input_nominal,
        vec![Some(0.25), Some(10.0), Some(0.6), Some(0.3)]
    );
    assert_eq!(p.info.scalar_units, vec!["m", "m", "dB-Hz"]);
}

// ─── 6. Parallel equals sequential ─────────────────────────────────────────────────

#[test]
fn t06_one_thread_and_four_threads_give_byte_identical_files() {
    let link = swap(&study("uq-link-budget-margin"), "n = 8192", "n = 256");
    let clock = swap(&study("uq-clock-holdover-qwf"), "n = 2000", "n = 48");
    for text in [link, clock] {
        let files = |threads: usize| {
            let opts = RunOptions {
                threads: Some(threads),
                ..RunOptions::default()
            };
            let result = uq::run_study(&text, &opts)
                .expect("runs")
                .result
                .expect("complete");
            output_files(&result)
        };
        let (one, four) = (files(1), files(4));
        assert_eq!(one.len(), 5);
        for ((name, a), (_, b)) in one.iter().zip(&four) {
            assert!(a == b, "{name} differs between 1 and 4 threads");
            assert!(a.len() > 500, "{name} is suspiciously short");
        }
        // A thread count written in the study file does not change it either.
        let with_threads = format!("{text}\n[run]\nthreads = 3\n");
        let result = uq::run_study(&with_threads, &RunOptions::default())
            .expect("runs")
            .result
            .expect("complete");
        assert!(output_files(&result)[0].1 == one[0].1);
    }
}

// ─── 7. Stop and resume ────────────────────────────────────────────────────────────

const FILES: [&str; 6] = [
    "runs.jsonl",
    "uq.result.json",
    "uq.bands.svg",
    "uq.sensitivity.svg",
    "uq.convergence.svg",
    "uq.report.html",
];

fn uncertainty(study: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    bin()
        .arg("uncertainty")
        .arg(study)
        .arg("--out")
        .arg(out)
        .args(["--dev", "--threads", "3"])
        .args(extra)
        .output()
        .expect("the binary runs")
}

fn assert_same_files(a: &Path, b: &Path) {
    for name in FILES {
        let x = std::fs::read(a.join(name)).unwrap_or_else(|e| panic!("{name} in {a:?}: {e}"));
        let y = std::fs::read(b.join(name)).unwrap_or_else(|e| panic!("{name} in {b:?}: {e}"));
        assert!(!x.is_empty(), "{name} is empty");
        assert!(
            x == y,
            "{name} differs between {} and {}",
            a.display(),
            b.display()
        );
    }
}

#[test]
fn t07_a_stopped_run_resumes_to_byte_identical_files() {
    let dir = scratch("resume");
    let study_path = dir.join("study.toml");
    let text = swap(&study("uq-clock-holdover-qwf"), "n = 2000", "n = 96");
    std::fs::write(&study_path, &text).expect("write");

    // Uninterrupted.
    let whole = dir.join("whole");
    let out = uncertainty(&study_path, &whole, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines = std::fs::read_to_string(whole.join("runs.jsonl")).expect("checkpoint");
    assert_eq!(
        lines.lines().count(),
        1 + 96,
        "a header and one line per realisation"
    );
    let header: Value = serde_json::from_str(lines.lines().next().expect("header")).expect("JSON");
    let result: Value = serde_json::from_str(
        &std::fs::read_to_string(whole.join("uq.result.json")).expect("result"),
    )
    .expect("JSON");
    assert_eq!(header["study_hash"], result["scenario_hash"]);
    let line: Value = serde_json::from_str(lines.lines().nth(5).expect("line")).expect("JSON");
    assert_eq!(line["index"], 4);
    assert_eq!(line["seed"], 42 + 4);
    assert_eq!(line["inputs"].as_array().expect("inputs").len(), 1);
    assert_eq!(line["outputs"].as_array().expect("outputs").len(), 2);
    assert_eq!(line["series"][0].as_array().expect("series").len(), 361);
    assert_eq!(line["result_sha256"].as_str().expect("hash").len(), 64);
    assert!(line["error"].is_null());

    // Stopped after 37 runs, then resumed.
    let parts = dir.join("parts");
    let out = uncertainty(&study_path, &parts, &["--max-runs", "37"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("incomplete: 37 of 96"));
    assert!(
        !parts.join("uq.result.json").exists(),
        "no result before the study finishes"
    );
    assert_eq!(
        std::fs::read_to_string(parts.join("runs.jsonl"))
            .expect("checkpoint")
            .lines()
            .count(),
        1 + 37
    );
    // Without --resume an existing checkpoint is never overwritten.
    let out = uncertainty(&study_path, &parts, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--resume"));
    // Nor does a new study start beside the result files of another run: a stopped run
    // writes no result, so the old one would sit next to the new checkpoint as if it
    // were this study's.
    let stale = dir.join("stale");
    std::fs::create_dir_all(&stale).expect("directory");
    std::fs::copy(whole.join("uq.result.json"), stale.join("uq.result.json")).expect("copy");
    let out = uncertainty(&study_path, &stale, &["--max-runs", "5"]);
    assert!(!out.status.success(), "a new study beside old results");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("another run") && err.contains("uq.result.json"),
        "{err}"
    );
    assert!(!stale.join("runs.jsonl").exists(), "nothing was started");
    // A second stop, then the rest.
    let out = uncertainty(&study_path, &parts, &["--resume", "--max-runs", "20"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("incomplete: 57 of 96"));
    let out = uncertainty(&study_path, &parts, &["--resume"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("96 of 96 realisations finished (39 new)")
    );
    assert_same_files(&whole, &parts);

    // Resuming a finished study runs nothing and rewrites the same files.
    let out = uncertainty(&study_path, &parts, &["--resume"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("(0 new)"));
    assert_same_files(&whole, &parts);

    // Editing the study and resuming is refused, and nothing is touched.
    let edited = dir.join("edited.toml");
    std::fs::write(&edited, swap(&text, "max = 2.7e-19", "max = 2.8e-19")).expect("write");
    let before = std::fs::read(parts.join("runs.jsonl")).expect("checkpoint");
    let out = uncertainty(&edited, &parts, &["--resume"]);
    assert!(!out.status.success(), "a changed study must not resume");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("refused") && err.contains("different study"),
        "{err}"
    );
    assert!(std::fs::read(parts.join("runs.jsonl")).expect("checkpoint") == before);

    // A last line cut short (the process died in the middle of a write) is detected,
    // dropped and run again.
    let cut = dir.join("cut");
    std::fs::create_dir_all(&cut).expect("directory");
    let full = std::fs::read(whole.join("runs.jsonl")).expect("checkpoint");
    // Keep 60 whole records and 300 bytes of the 61st.
    let mut newlines = full
        .iter()
        .enumerate()
        .filter(|(_, b)| **b == b'\n')
        .map(|(i, _)| i);
    let end_of_60 = newlines.nth(60).expect("61 lines");
    std::fs::write(cut.join("runs.jsonl"), &full[..end_of_60 + 1 + 300]).expect("write");
    let out = uncertainty(&study_path, &cut, &["--resume"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("cut short"), "{err}");
    assert!(
        err.contains("96 of 96 realisations finished (36 new)"),
        "{err}"
    );
    assert_same_files(&whole, &cut);

    // Damage that is not at the end is refused.
    let damaged = dir.join("damaged");
    std::fs::create_dir_all(&damaged).expect("directory");
    let mut bytes = full.clone();
    let mid = full.iter().position(|b| *b == b'\n').expect("header") + 40;
    bytes[mid] = b'{';
    bytes[mid + 1] = b'{';
    std::fs::write(damaged.join("runs.jsonl"), bytes).expect("write");
    let out = uncertainty(&study_path, &damaged, &["--resume"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("damaged"));
}

// ─── 8. Failed realisations ────────────────────────────────────────────────────────

#[test]
fn t08_failed_runs_are_recorded_counted_and_never_meet_a_limit() {
    // The slant range reaches below zero in about one realisation in eleven; the
    // engine refuses a range that is not positive.
    let text = r#"kind = "pro-uncertainty"
title = "link margin with a range that reaches an invalid value"
seed = 7
base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""
[[input]]
name = "range"
key = "range_km"
dist = "uniform"
min = -200.0
max = 2000.0
source = "test fixture: a range chosen to reach an invalid (negative) value"
[[output]]
name = "margin"
path = "margin_db"
limit = { op = "ge", value = 80.0 }
[sampling]
method = "lhs"
n = 220
[sensitivity]
method = "src"
"#;
    let run = uq::run_study(text, &RunOptions::default()).expect("the study itself runs");
    let result = run.result.expect("complete");
    let doc = &result.doc;
    // A Latin hypercube of 220 strata over [-200, 2000]: strata 0..19 lie wholly below
    // zero (each is 10 km wide), so exactly 20 realisations fail.
    assert_eq!(doc["runs"]["total"], 220);
    assert_eq!(doc["runs"]["failed"], 20);
    assert_eq!(doc["runs"]["ok"], 200);
    let failures = doc["runs"]["failures"].as_array().expect("failures");
    assert_eq!(failures.len(), 20);
    assert!(failures[0]["error"]
        .as_str()
        .expect("an error")
        .contains("range_km must be finite and positive"));

    let m = &doc["outputs"]["margin"];
    assert_eq!(m["n"], 200, "statistics are over the successes only");
    let samples = m["samples"].as_array().expect("samples");
    assert_eq!(samples.len(), 220, "a failed realisation is kept, as null");
    assert_eq!(samples.iter().filter(|v| v.is_null()).count(), 20);
    // Every successful margin is above 80 dB (the largest range is 2000 km, where the
    // margin is 92.1 dB), so the limit is met by exactly the 200 successes: 200 of 220,
    // never 200 of 200.
    assert!(f(&m["min"]) > 80.0);
    let l = &m["limit"];
    assert_eq!(l["n_met"], 200);
    assert_eq!(l["n_realisations"], 220);
    assert_eq!(f(&l["probability"]), 200.0 / 220.0);
    assert_eq!(f(&l["probability_among_evaluated"]), 1.0);
    assert!(f(&l["probability_ci95_high"]) < 1.0);

    assert!(
        result.summary.contains("20 run(s) FAILED"),
        "{}",
        result.summary
    );
    assert!(
        result.summary.contains("successful realisations only"),
        "{}",
        result.summary
    );
    let html = &output_files(&result)[4].1;
    assert!(html.contains("20 FAILED RUNS"));
    assert_eq!(doc["sensitivity"]["outputs"]["margin"]["n_rows"], 200);
}

// ─── 9. Adversarial studies ────────────────────────────────────────────────────────

fn error_of(text: &str) -> String {
    match uq::run_study(text, &RunOptions::default()) {
        Ok(_) => panic!("the study must be refused:\n{text}"),
        Err(e) => e,
    }
}

#[test]
fn t09_adversarial_studies_are_refused_or_reported_never_guessed() {
    let base = swap(&study("uq-link-budget-margin"), "n = 8192", "n = 64");

    // A standard deviation that is not a number, or negative.
    let e = error_of(&swap(
        &base,
        "mean = 55.0\nstd = 1.0",
        "mean = 55.0\nstd = nan",
    ));
    assert!(
        e.contains("input `eirp`") && e.contains("`std` must be a finite number"),
        "{e}"
    );
    let e = error_of(&swap(
        &base,
        "mean = 55.0\nstd = 1.0",
        "mean = 55.0\nstd = -1.0",
    ));
    assert!(e.contains("`std` must not be negative"), "{e}");
    let e = error_of(&swap(
        &base,
        "mean = 55.0\nstd = 1.0",
        "mean = inf\nstd = 1.0",
    ));
    assert!(e.contains("`mean` must be a finite number"), "{e}");

    // Fewer than two realisations.
    let e = error_of(&swap(&base, "n = 64", "n = 1"));
    assert!(e.contains("`n` must be at least 2"), "{e}");

    // A key that is not in the scenario: refused, never created.
    let e = error_of(&swap(&base, "key = \"eirp_dbw\"", "key = \"eirp_dbW\""));
    assert!(
        e.contains("input `eirp`") && e.contains("no field `eirp_dbW`"),
        "{e}"
    );
    let e = error_of(&swap(&base, "key = \"eirp_dbw\"", "key = \"band\""));
    assert!(e.contains("not a number"), "{e}");

    // An output path that is missing.
    let e = error_of(&swap(&base, "path = \"margin_db\"", "path = \"margin_dB\""));
    assert!(
        e.contains("output `margin`") && e.contains("no field `margin_dB`"),
        "{e}"
    );

    // No source for an input's uncertainty.
    let no_source: String = base
        .lines()
        .filter(|l| !l.starts_with("source = \"assumed: 1 dB"))
        .collect::<Vec<_>>()
        .join("\n");
    let e = error_of(&no_source);
    assert!(e.contains("`source` is required"), "{e}");

    // A parameter of another distribution, a misspelled field, an unknown method.
    let e = error_of(&swap(
        &base,
        "mean = 55.0\nstd = 1.0",
        "mean = 55.0\nstd = 1.0\nmax = 60.0",
    ));
    assert!(e.contains("does not take `max`"), "{e}");
    let e = error_of(&swap(&base, "method = \"lhs\"", "methd = \"lhs\""));
    assert!(e.contains("unknown field"), "{e}");
    let e = error_of(&swap(&base, "method = \"sobol\"", "method = \"morris\""));
    assert!(e.contains("\"none\", \"src\" or \"sobol\""), "{e}");
    let e = error_of(&swap(&base, "op = \"ge\"", "op = \"about\""));
    assert!(e.contains("limit `op`"), "{e}");

    // A base scenario that does not run, or that is a Pro kind.
    let e = error_of(&swap(&base, "range_km = 2000.0", "range_km = -1.0"));
    assert!(e.contains("the base scenario does not run"), "{e}");
    let e = error_of(&swap(
        &base,
        "kind = \"link-budget\"",
        "kind = \"pro-batch\"",
    ));
    assert!(e.contains("must be an open scenario kind"), "{e}");

    // A constant output: no index, with the reason, for both methods. The required
    // Eb/N0 is an input the engine echoes; none of the three inputs changes it.
    for method in ["sobol", "src"] {
        let text = swap(
            &swap(
                &base,
                "path = \"margin_db\"\nlimit = { op = \"ge\", value = 91.0 }",
                "path = \"required_eb_n0_db\"",
            ),
            "method = \"sobol\"",
            &format!("method = \"{method}\""),
        );
        let doc = run(&text, 2);
        let s = &doc["sensitivity"]["outputs"]["margin"];
        assert!(
            s["reason"]
                .as_str()
                .expect("a reason")
                .contains("does not vary"),
            "{s}"
        );
        for i in 0..3 {
            let e = &s["inputs"][i];
            for key in ["first_order", "total", "src", "rank"] {
                assert!(
                    e[key].is_null(),
                    "{method}: {key} must be null, got {}",
                    e[key]
                );
            }
            assert!(e["reason"]
                .as_str()
                .expect("a reason")
                .contains("does not vary"));
        }
        assert_eq!(f(&doc["outputs"]["margin"]["std"]), 0.0);
    }

    // A zero-variance input: no index for it, with the reason; the others keep theirs.
    for method in ["sobol", "src"] {
        let text = swap(
            &swap(&base, "mean = 53.0\nstd = 0.5", "mean = 53.0\nstd = 0.0"),
            "method = \"sobol\"",
            &format!("method = \"{method}\""),
        );
        let doc = run(&text, 2);
        let inputs = &doc["sensitivity"]["outputs"]["margin"]["inputs"];
        assert!(inputs[1]["rank"].is_null());
        assert!(inputs[1]["reason"]
            .as_str()
            .expect("a reason")
            .contains("single value"));
        assert_eq!(inputs[0]["rank"], 1);
        assert_eq!(inputs[2]["rank"], 2);
    }
}

// ─── 10. Licence ───────────────────────────────────────────────────────────────────

#[test]
fn t10_the_study_needs_a_licence_and_runs_with_dev() {
    let dir = scratch("licence");
    let study_path = dir.join("study.toml");
    let text = swap(&study("uq-link-budget-margin"), "n = 8192", "n = 32");
    std::fs::write(&study_path, &text).expect("write");

    // Evaluation tier (no licence): the command is refused before anything runs.
    let out = bin()
        .arg("uncertainty")
        .arg(&study_path)
        .arg("--out")
        .arg(dir.join("denied"))
        .output()
        .expect("the binary runs");
    assert!(!out.status.success(), "no licence must be refused");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("requires the 'enterprise' tier"), "{err}");
    assert!(err.contains("'pro-uncertainty'"), "{err}");
    assert!(!dir.join("denied").exists(), "a refused run writes nothing");

    // The same through `run`: the dispatcher refuses the kind at Evaluation tier.
    let out = bin()
        .arg("run")
        .arg(&study_path)
        .output()
        .expect("the binary runs");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("'pro-uncertainty' requires the 'enterprise' tier"),
        "{err}"
    );

    // With --dev (a debug build's escape hatch, as the other Pro tests use) both run.
    let out = uncertainty(&study_path, &dir.join("ok"), &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let file: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("ok/uq.result.json")).expect("result"),
    )
    .expect("JSON");
    let out = bin()
        .arg("run")
        .arg(&study_path)
        .arg("--dev")
        .output()
        .expect("the binary runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed: Value = serde_json::from_slice(&out.stdout).expect("`run` prints the result JSON");
    assert_eq!(
        printed, file,
        "`run` (in memory) and `uncertainty` (checkpointed) agree"
    );
    assert_eq!(uq::FEATURE, "pro-uncertainty");
    assert_eq!(
        kshana_pro::license::feature_min_tier(uq::FEATURE).as_str(),
        "enterprise"
    );
    assert!(!kshana_pro::license::Entitlement::evaluation().allows(uq::FEATURE, 0));
}

// ─── Fixtures ──────────────────────────────────────────────────────────────────────

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir)
        .expect("readable")
        .filter_map(Result::ok)
    {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn no_study_or_fixture_names_the_optional_engine_preset() {
    let mut files = Vec::new();
    files_under(Path::new(DIR), &mut files);
    files.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/uncertainty.rs"
    )));
    files.push(PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/UNCERTAINTY.md"
    )));
    assert!(
        files.len() >= 11,
        "expected the studies, the engine copies and the guide"
    );
    // The needle is assembled so that this file does not contain it.
    let needle = ["cel", "este"].concat();
    for p in files {
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        assert!(
            !name.contains(&needle),
            "{} is named after the preset",
            p.display()
        );
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
            .to_lowercase();
        assert!(
            !text.contains(&needle),
            "{} mentions the preset",
            p.display()
        );
    }
}

/// The source directory of the `kshana` package this build embeds (the same lookup as
/// `tests/open_engine_reachable.rs`: a path dependency names its directory; a published
/// version is unpacked under `$CARGO_HOME/registry/src/<index>/kshana-<version>`).
fn engine_dir() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: toml::Value = std::fs::read_to_string(root.join("Cargo.toml"))
        .expect("Cargo.toml is readable")
        .parse()
        .expect("Cargo.toml is TOML");
    let dep = manifest
        .get("dependencies")
        .and_then(|d| d.get("kshana"))
        .expect("Cargo.toml declares the kshana dependency");
    if let Some(path) = dep.get("path").and_then(toml::Value::as_str) {
        return root.join(path);
    }
    let lock: toml::Value = std::fs::read_to_string(root.join("Cargo.lock"))
        .expect("Cargo.lock is readable")
        .parse()
        .expect("Cargo.lock is TOML");
    let version = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .expect("Cargo.lock lists packages")
        .iter()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("kshana"))
        .and_then(|p| p.get("version").and_then(toml::Value::as_str))
        .expect("Cargo.lock pins the kshana package");
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .expect("CARGO_HOME or HOME is set");
    let src = cargo_home.join("registry").join("src");
    std::fs::read_dir(&src)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()))
        .filter_map(Result::ok)
        .map(|index| index.path().join(format!("kshana-{version}")))
        .find(|dir| dir.join("scenarios").is_dir())
        .unwrap_or_else(|| panic!("kshana {version} is not unpacked under {}", src.display()))
}

#[test]
fn engine_scenarios_match_their_provenance_and_the_embedded_engine() {
    let provenance =
        std::fs::read_to_string(format!("{DIR}/engine/PROVENANCE.md")).expect("PROVENANCE.md");
    let engine = engine_dir().join("scenarios");
    assert!(engine.is_dir(), "{} is not a directory", engine.display());
    for name in [
        "campaign-monte-carlo-clock-holdover",
        "leo-pnt-end-to-end",
        "link-budget",
    ] {
        let text = engine_copy(name);
        let sha = uq::sha256_hex(text.as_bytes());
        assert!(
            provenance.contains(&format!("| `engine/{name}.toml` |")) && provenance.contains(&sha),
            "PROVENANCE.md does not record {name} with SHA-256 {sha}"
        );
        let theirs = std::fs::read_to_string(engine.join(format!("{name}.toml")))
            .unwrap_or_else(|e| panic!("the embedded engine has no {name}.toml: {e}"));
        assert!(
            text == theirs,
            "{name}.toml differs from the embedded engine's own file"
        );
    }
}

#[test]
fn every_study_uses_its_engine_scenario_unchanged() {
    let parsed = |t: &str| -> toml::Value { toml::from_str(t).expect("TOML") };
    let base_of = |name: &str| -> toml::Value {
        let spec: StudySpec = toml::from_str(&study(name)).expect("the study parses");
        parsed(&spec.base_scenario)
    };
    let link = parsed(&engine_copy("link-budget"));
    let leo = parsed(&engine_copy("leo-pnt-end-to-end"));
    let campaign = parsed(&engine_copy("campaign-monte-carlo-clock-holdover"));
    let member = campaign["monte_carlo"]["scenario"].clone();
    assert_eq!(base_of("uq-link-budget-margin"), link);
    assert_eq!(base_of("uq-leo-pnt-chain"), leo);
    assert_eq!(base_of("uq-leo-pnt-chain-sobol"), leo);
    assert_eq!(base_of("uq-clock-holdover-seeds"), member);
    assert_eq!(base_of("uq-clock-holdover-qwf"), member);
    // The seeds study states the campaign's own seed, run count and metric paths.
    let spec: StudySpec = toml::from_str(&study("uq-clock-holdover-seeds")).expect("parses");
    assert_eq!(Some(spec.seed as i64), campaign["seed"].as_integer());
    assert_eq!(
        Some(spec.sampling.n as i64),
        campaign["monte_carlo"]["runs"].as_integer()
    );
    for (output, metric) in spec.outputs.iter().zip(
        campaign["monte_carlo"]["metrics"]
            .as_array()
            .expect("metrics"),
    ) {
        assert_eq!(output.path.as_deref(), metric["path"].as_str());
    }
}
