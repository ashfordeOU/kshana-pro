// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! V5: the holdover time-to-breach prediction bars, scored exactly as pre-registered in
//! `docs/evidence/v5-holdover-time-to-breach/2026-11-15_PREREGISTRATION.md` on
//!
//! * D-R, REAL public data: eleven IGS GPS IIF rubidium clocks, holdover emulated by freezing
//!   the estimated state (no GNSS removal took place), and
//! * D-S, SYNTHETIC: twenty clocks of four classes from the crate's own seeded generator.
//!
//! The scored numbers are committed in `results.json` beside the pre-registration; this test
//! recomputes them and fails if the committed file, or `timing::V5_PASSED`, no longer matches
//! (regenerate with `KSHANA_PRO_REGEN_V5=1`). It never fails because a bar missed: a miss is
//! published, not hidden.

use kshana_pro::timing::model::{class_noise, synth_clock, HoldoverModel, SplitMix, Z95};

const RESULTS: &str = "docs/evidence/v5-holdover-time-to-breach/results.json";
const CLOCKS: &str = include_str!("fixtures/timing_v5_igs/clocks.txt");
// V5b (`2026-11-15_V5b_PREREGISTRATION.md`): the successor predictor on data V5 did not use.
const RESULTS_B: &str = "docs/evidence/v5-holdover-time-to-breach/V5b_results.json";
const CLOCKS_B: &str = include_str!("fixtures/timing_v5b_igs/clocks.txt");

type Fit = fn(&[f64], f64, f64) -> Result<HoldoverModel, String>;

/// One data set's scoring inputs.
struct DataSet {
    name: &'static str,
    tau0: f64,
    window_s: f64,
    horizon_s: f64,
    cover_h: &'static [f64],
    limits: &'static [f64],
    /// (clock, phase by index; None = missing)
    clocks: Vec<(String, Vec<Option<f64>>)>,
    handovers: Vec<usize>,
}

#[derive(Default, Clone, serde::Serialize)]
struct Tally {
    runs: usize,
    excluded: usize,
    cover_points: usize,
    cover_in: usize,
    pairs_breach: usize,
    early: usize,
    ratios_warn: Vec<f64>,
    within2: usize,
    pairs_nobreach: usize,
    false_alarm: usize,
}

fn real() -> DataSet {
    real_from(
        CLOCKS,
        "D-R (real, IGS GPS IIF rubidium, emulated holdover)",
    )
}

fn real_from(text: &str, name: &'static str) -> DataSet {
    let mut clocks: Vec<(String, Vec<Option<f64>>)> = Vec::new();
    let mut acc = 0i64;
    let mut first = 0.0;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('@') {
            let prn = rest.split_whitespace().next().unwrap().to_string();
            first = rest
                .split("first_bias_s=")
                .nth(1)
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse()
                .unwrap();
            acc = 0;
            clocks.push((prn, vec![None; 4032]));
            continue;
        }
        let mut it = line.split_whitespace();
        let i: usize = it.next().unwrap().parse().unwrap();
        let d: i64 = it.next().unwrap().parse().unwrap();
        acc += d;
        clocks.last_mut().unwrap().1[i] = Some(first + acc as f64 * 1e-13);
    }
    assert_eq!(clocks.len(), 11);
    DataSet {
        name,
        tau0: 300.0,
        window_s: 48.0 * 3600.0,
        horizon_s: 48.0 * 3600.0,
        cover_h: &[1.0, 2.0, 4.0, 8.0, 12.0, 24.0, 48.0],
        limits: &[1e-9, 3e-9, 10e-9],
        clocks,
        handovers: (0..20).map(|j| 2016 + 72 * j).collect(),
    }
}

fn synthetic() -> DataSet {
    synthetic_seed(20261115, "D-S (SYNTHETIC, four classes)")
}

fn synthetic_seed(seed: u64, name: &'static str) -> DataSet {
    let tau0 = 10.0;
    // Four days spanned: 4 d / tau0 intervals, so 34 561 samples.
    let n = (4.0 * 86_400.0 / tau0) as usize + 1;
    let mut rng = SplitMix::new(seed);
    let mut clocks = Vec::new();
    for (class, dmax) in [
        ("tcxo", 1e-14),
        ("ocxo", 1e-15),
        ("rafs", 1e-16),
        ("csac", 1e-15),
    ] {
        let nz = class_noise(class).unwrap();
        for i in 0..5 {
            let drift = rng.range(-dmax, dmax);
            let y0 = rng.range(-1e-9, 1e-9);
            let x = synth_clock(&mut rng, n, tau0, nz.q_wf, nz.q_rw, drift, y0, 1e-9);
            clocks.push((format!("{class}-{i}"), x.into_iter().map(Some).collect()));
        }
    }
    DataSet {
        name,
        tau0,
        window_s: 48.0 * 3600.0,
        horizon_s: 24.0 * 3600.0,
        cover_h: &[1.0, 2.0, 4.0, 8.0, 12.0, 24.0],
        limits: &[100e-9, 1.1e-6, 100e-6],
        clocks,
        handovers: (0..5).map(|j| 17_280 + 2160 * j).collect(),
    }
}

fn score(ds: &DataSet, fit: Fit) -> (Tally, Vec<(String, Tally)>) {
    let w = (ds.window_s / ds.tau0).round() as usize;
    let h = (ds.horizon_s / ds.tau0).round() as usize;
    let mut total = Tally::default();
    let mut per = Vec::new();
    for (name, x) in &ds.clocks {
        let mut t = Tally::default();
        for &th in &ds.handovers {
            let span = &x[th - w..=th + h];
            if span.iter().any(|v| v.is_none()) {
                t.excluded += 1;
                continue;
            }
            t.runs += 1;
            let train: Vec<f64> = x[th - w..=th].iter().map(|v| v.unwrap()).collect();
            let m = fit(&train, ds.tau0, ds.window_s).expect("fit");
            let te = |k: usize| {
                let u = k as f64 * ds.tau0;
                x[th + k].unwrap() - (m.phase_s + m.freq * u)
            };
            for ch in ds.cover_h {
                let k = (ch * 3600.0 / ds.tau0).round() as usize;
                let u = k as f64 * ds.tau0;
                t.cover_points += 1;
                if (te(k) - m.mean(u)).abs() <= Z95 * m.sigma(u) {
                    t.cover_in += 1;
                }
            }
            for &l in ds.limits {
                let b = m.time_to_breach(l, ds.tau0, ds.horizon_s);
                let act = (1..=h)
                    .find(|&k| te(k).abs() >= l)
                    .map(|k| k as f64 * ds.tau0);
                match act {
                    Some(ta) => {
                        t.pairs_breach += 1;
                        if b.t_warn_s.is_some_and(|tw| tw <= ta) {
                            t.early += 1;
                        }
                        t.ratios_warn.push(b.t_warn_s.unwrap_or(ds.horizon_s) / ta);
                        let r50 = b.t_50_s.unwrap_or(ds.horizon_s) / ta;
                        if (0.5..=2.0).contains(&r50) {
                            t.within2 += 1;
                        }
                    }
                    None => {
                        t.pairs_nobreach += 1;
                        if b.t_50_s.is_some() {
                            t.false_alarm += 1;
                        }
                    }
                }
            }
        }
        total.runs += t.runs;
        total.excluded += t.excluded;
        total.cover_points += t.cover_points;
        total.cover_in += t.cover_in;
        total.pairs_breach += t.pairs_breach;
        total.early += t.early;
        total.ratios_warn.extend(t.ratios_warn.iter().copied());
        total.within2 += t.within2;
        total.pairs_nobreach += t.pairs_nobreach;
        total.false_alarm += t.false_alarm;
        per.push((name.clone(), t));
    }
    (total, per)
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    if s.is_empty() {
        return f64::NAN;
    }
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        0.5 * (s[n / 2 - 1] + s[n / 2])
    }
}

fn r4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// The bars of one data set: (id, value, verdict) and whether the set passes.
fn bars(t: &Tally) -> (Vec<serde_json::Value>, bool) {
    let cov = t.cover_in as f64 / t.cover_points.max(1) as f64;
    let enough_b = t.pairs_breach >= 30;
    let enough_n = t.pairs_nobreach >= 30;
    let b2 = t.early as f64 / t.pairs_breach.max(1) as f64;
    let b3 = median(&t.ratios_warn);
    let b4 = t.within2 as f64 / t.pairs_breach.max(1) as f64;
    let b5 = t.false_alarm as f64 / t.pairs_nobreach.max(1) as f64;
    let v = |ok: bool| if ok { "pass" } else { "MISS" };
    let na = "not assessable";
    let b1_ok = (0.90..=0.99).contains(&cov);
    let b2_ok = enough_b && b2 >= 0.90;
    let b3_ok = enough_b && b3 >= 0.20;
    let b4_ok = enough_b && b4 >= 0.60;
    let b5_ok = !enough_n || b5 <= 0.25;
    let rows = vec![
        serde_json::json!({"bar": "B1", "metric": "95 % interval coverage", "value": r4(cov), "pass_if": "0.90 to 0.99", "verdict": v(b1_ok)}),
        serde_json::json!({"bar": "B2", "metric": "early warning share", "value": r4(b2), "pass_if": ">= 0.90", "verdict": if enough_b { v(b2_ok) } else { na }}),
        serde_json::json!({"bar": "B3", "metric": "median T_warn / T_act", "value": r4(b3), "pass_if": ">= 0.20", "verdict": if enough_b { v(b3_ok) } else { na }}),
        serde_json::json!({"bar": "B4", "metric": "T_50 within a factor 2", "value": r4(b4), "pass_if": ">= 0.60", "verdict": if enough_b { v(b4_ok) } else { na }}),
        serde_json::json!({"bar": "B5", "metric": "false-alarm share", "value": r4(b5), "pass_if": "<= 0.25", "verdict": if enough_n { v(b5_ok) } else { na }}),
    ];
    (rows, b1_ok && b2_ok && b3_ok && b4_ok && b5_ok)
}

fn set_json(ds: &DataSet, per_clock: bool, fit: Fit) -> (serde_json::Value, bool) {
    let (t, per) = score(ds, fit);
    let (rows, pass) = bars(&t);
    let mut doc = serde_json::json!({
        "data_set": ds.name,
        "runs": t.runs,
        "excluded_runs": t.excluded,
        "coverage_points": t.cover_points,
        "pairs_with_breach": t.pairs_breach,
        "pairs_without_breach": t.pairs_nobreach,
        "bars": rows,
        "passes": pass,
    });
    if per_clock {
        doc["per_clock"] = per
            .iter()
            .map(|(n, c)| {
                serde_json::json!({
                    "clock": n,
                    "runs": c.runs,
                    "excluded": c.excluded,
                    "coverage": r4(c.cover_in as f64 / c.cover_points.max(1) as f64),
                    "pairs_with_breach": c.pairs_breach,
                    "early_warning": r4(c.early as f64 / c.pairs_breach.max(1) as f64),
                    "median_warn_ratio": r4(median(&c.ratios_warn)),
                    "t50_within_2x": r4(c.within2 as f64 / c.pairs_breach.max(1) as f64),
                    "pairs_without_breach": c.pairs_nobreach,
                    "false_alarms": c.false_alarm,
                })
            })
            .collect();
    }
    (doc, pass)
}

/// Score both data sets with `fit`, print the table, and compare with (or, with
/// `KSHANA_PRO_REGEN_V5`, write) the committed results file. Returns whether every bar passed.
fn publish(pre: &str, results: &str, sets: [DataSet; 2], fit: Fit) -> bool {
    let [a, b] = sets;
    let (dr, pr) = set_json(&a, true, fit);
    let (ds, ps) = set_json(&b, true, fit);
    let passed = pr && ps;
    let doc = serde_json::json!({
        "format": "kshana-pro-v5-results/1",
        "pre_registration": pre,
        "v5_passed": passed,
        "data_sets": [dr, ds],
    });
    let text = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    for set in doc["data_sets"].as_array().unwrap() {
        eprintln!(
            "{}: runs {} (excluded {})",
            set["data_set"], set["runs"], set["excluded_runs"]
        );
        for b in set["bars"].as_array().unwrap() {
            eprintln!(
                "  {} {:<28} {:>8} [{}] {}",
                b["bar"],
                b["metric"].as_str().unwrap(),
                b["value"],
                b["pass_if"].as_str().unwrap(),
                b["verdict"]
            );
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(results);
    if std::env::var_os("KSHANA_PRO_REGEN_V5").is_some() {
        std::fs::write(&path, &text).unwrap();
        eprintln!("wrote {results}");
        return passed;
    }
    let committed = std::fs::read_to_string(&path).expect("the results file is committed");
    assert_eq!(
        committed, text,
        "the published results no longer match the code; regenerate and re-publish"
    );
    passed
}

#[test]
fn v5_scored_as_pre_registered_and_published() {
    let passed = publish(
        "2026-11-15_PREREGISTRATION.md",
        RESULTS,
        [real(), synthetic()],
        HoldoverModel::fit,
    );
    if std::env::var_os("KSHANA_PRO_REGEN_V5").is_none() {
        assert_eq!(
            kshana_pro::timing::V5_PASSED,
            passed,
            "timing::V5_PASSED must equal the scored V5 outcome"
        );
    }
}

#[test]
fn v5b_scored_as_pre_registered_and_published() {
    let passed = publish(
        "2026-11-15_V5b_PREREGISTRATION.md",
        RESULTS_B,
        [
            real_from(
                CLOCKS_B,
                "D-R2 (real, IGS GPS IIF rubidium 2025-08-17..30, emulated holdover)",
            ),
            synthetic_seed(20261116, "D-S2 (SYNTHETIC, four classes, seed 20261116)"),
        ],
        HoldoverModel::fit_kalman,
    );
    if std::env::var_os("KSHANA_PRO_REGEN_V5").is_none() {
        assert_eq!(
            kshana_pro::timing::V5B_PASSED,
            passed,
            "timing::V5B_PASSED must equal the scored V5b outcome"
        );
    }
}
