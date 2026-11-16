// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! V3: the M4 position-offset bar with the new method of
//! `docs/evidence/v3-m4-offset-successor/2026-11-15_PREREGISTRATION.md`.
//!
//! * `KSHANA_PRO_V3_DEV=1`: the development run on seeds 50 to 59 over the pre-registered grid;
//!   writes `dev.json` next to the pre-registration.
//! * `KSHANA_PRO_V3_SCORE=1`: the one scored run on seeds 5000 to 5009 with the committed pair;
//!   writes `results.json`. Run once.
//! * Without either: the recorded results are checked against the pre-registered bars and the
//!   committed verdicts, and determinism (V3-G5) is checked on one development seed.

use kshana::m4_dark_vessel::{generate, Label};
use kshana::m4_logic::{decide, decided_classes, metrics, M4Params};
use kshana_pro::integrity::offset_v4::{self, Params, GRID};
use serde_json::{json, Value};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/v3-m4-offset-successor"
);

type Row = (Label, Label, Option<f64>);

fn rows_for(seed: u64, grid: &[Params]) -> (Vec<Row>, Vec<Vec<Row>>) {
    let scene = generate(seed);
    let d1 = decide(&scene, &M4Params::v3());
    let base = decided_classes(&scene, &d1);
    let classes = offset_v4::stage2_grid(&scene.ais, &scene.rf, &d1.class, grid);
    let per = classes
        .into_iter()
        .map(|c| decided_classes(&scene, &offset_v4::with_classes(&d1, c)))
        .collect();
    (base, per)
}

fn recall_ge5(m: &Value) -> (u64, u64) {
    let mut k = 0;
    let mut n = 0;
    for d in ["5", "10", "20", "50"] {
        let r = &m["offset_recall_by_km"][d];
        let nn = r["n"].as_u64().unwrap();
        n += nn;
        k += (r["recall"].as_f64().unwrap() * nn as f64).round() as u64;
    }
    (k, n)
}

#[test]
fn v3_development_run_on_seeds_50_to_59() {
    if std::env::var_os("KSHANA_PRO_V3_DEV").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3_DEV=1 for the development run");
        return;
    }
    dev_run(50..60, &GRID, "dev.json", "2026-11-15_PREREGISTRATION.md");
}

#[test]
fn v3b_development_run_on_seeds_60_to_69() {
    if std::env::var_os("KSHANA_PRO_V3B_DEV").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3B_DEV=1 for the V3b development run");
        return;
    }
    dev_run(
        60..70,
        &offset_v4::GRID_B,
        "V3b_dev.json",
        "2026-11-15_V3b_PREREGISTRATION.md",
    );
}

fn dev_run(seeds: std::ops::Range<u64>, grid: &[Params], file: &str, prereg: &str) {
    let label = format!("{} to {}", seeds.start, seeds.end - 1);
    let mut base_all = Vec::new();
    let mut per_all: Vec<Vec<Row>> = vec![Vec::new(); grid.len()];
    for seed in seeds {
        let (b, per) = rows_for(seed, grid);
        base_all.extend(b);
        for (g, r) in per.into_iter().enumerate() {
            per_all[g].extend(r);
        }
        eprintln!("seed {seed} done");
    }
    let mut table = Vec::new();
    for (g, p) in grid.iter().enumerate() {
        let m = metrics(&per_all[g]);
        let (k, n) = recall_ge5(&m);
        table.push(json!({
            "m": p.m, "g_pair": p.g_pair,
            "false_alarm": m["false_alarm"],
            "offset_recall_ge5": k as f64 / n as f64, "offset_hits_ge5": k, "offset_n_ge5": n,
            "metrics": m,
        }));
    }
    // The pre-registered rule: highest recall at d >= 5 km subject to FA <= 0.005; ties to the
    // smaller m, then the smaller g_pair (the grid is in that order); none eligible: lowest FA.
    let eligible: Vec<usize> = (0..grid.len())
        .filter(|&g| table[g]["false_alarm"].as_f64().unwrap() <= 0.005)
        .collect();
    let chosen = if eligible.is_empty() {
        (0..grid.len())
            .min_by(|&a, &b| {
                table[a]["false_alarm"]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&table[b]["false_alarm"].as_f64().unwrap())
            })
            .unwrap()
    } else {
        let mut best = eligible[0];
        for &g in &eligible[1..] {
            if table[g]["offset_hits_ge5"].as_u64() > table[best]["offset_hits_ge5"].as_u64() {
                best = g;
            }
        }
        best
    };
    let doc = json!({
        "pre_registration": prereg,
        "seeds": label,
        "stage1_open_v3": metrics(&base_all),
        "grid": table,
        "eligible": eligible,
        "chosen": {"m": grid[chosen].m, "g_pair": grid[chosen].g_pair},
    });
    std::fs::write(
        format!("{DIR}/{file}"),
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
}

fn run_scored(seeds: std::ops::Range<u64>, p: Params) -> (Value, Value) {
    let mut base_all = Vec::new();
    let mut all = Vec::new();
    for seed in seeds {
        let (b, per) = rows_for(seed, &[p]);
        base_all.extend(b);
        all.extend(per.into_iter().next().unwrap());
        eprintln!("seed {seed} done");
    }
    (metrics(&base_all), metrics(&all))
}

/// The pre-registered bars on a metrics document: (B4, G1, G2, G3, G4).
fn bars(m: &Value) -> [bool; 5] {
    let r = |d: &str| m["offset_recall_by_km"][d]["recall"].as_f64().unwrap();
    let pc = |c: &str, k: &str| m["per_class"][c][k].as_f64().unwrap();
    [
        r("5") >= 0.85 && r("10") >= 0.98 && r("20") >= 0.98 && r("50") >= 0.98,
        m["false_alarm"].as_f64().unwrap() <= 0.01,
        pc("DARK", "recall") >= 0.95 && pc("DARK", "precision") >= 0.80,
        pc("GAP", "recall") >= 0.85,
        pc("IDENTITY-DUPLICATE", "recall") >= 0.95,
    ]
}

#[test]
fn v3_scored_run_on_seeds_5000_to_5009() {
    if std::env::var_os("KSHANA_PRO_V3_SCORE").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3_SCORE=1 for the one scored run");
        return;
    }
    let p = offset_v4::V3_CHOSEN;
    let (base, m) = run_scored(5000..5010, p);
    let b = bars(&m);
    let doc = json!({
        "pre_registration": "2026-11-15_PREREGISTRATION.md",
        "seeds": "5000 to 5009",
        "params": {"m": p.m, "g_pair": p.g_pair},
        "stage1_open_v3": base,
        "metrics": m,
        "bars": {"V3-B4": b[0], "V3-G1": b[1], "V3-G2": b[2], "V3-G3": b[3], "V3-G4": b[4]},
        "met": b.iter().all(|x| *x),
    });
    std::fs::write(
        format!("{DIR}/results.json"),
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
}

#[test]
fn v3b_scored_run_on_seeds_6000_to_6009() {
    if std::env::var_os("KSHANA_PRO_V3B_SCORE").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3B_SCORE=1 for the one V3b scored run");
        return;
    }
    let p = offset_v4::V3B_CHOSEN;
    let (base, m) = run_scored(6000..6010, p);
    let b = bars(&m);
    let doc = json!({
        "pre_registration": "2026-11-15_V3b_PREREGISTRATION.md",
        "seeds": "6000 to 6009",
        "params": {"m": p.m, "g_pair": p.g_pair, "b": p.b},
        "stage1_open_v3": base,
        "metrics": m,
        "bars": {"V3-B4": b[0], "V3-G1": b[1], "V3-G2": b[2], "V3-G3": b[3], "V3-G4": b[4]},
        "met": b.iter().all(|x| *x),
    });
    std::fs::write(
        format!("{DIR}/V3b_results.json"),
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
}

/// V3-G5: the same seed decided twice gives the same classes (both methods), and stage 2 never
/// touches a class other than NORMAL.
#[test]
fn v3_g5_determinism_on_a_development_seed() {
    let grid = [offset_v4::V3_CHOSEN, offset_v4::V3B_CHOSEN];
    let scene = generate(50);
    let d1 = decide(&scene, &M4Params::v3());
    let a = offset_v4::stage2_grid(&scene.ais, &scene.rf, &d1.class, &grid);
    let b = offset_v4::stage2_grid(&scene.ais, &scene.rf, &d1.class, &grid);
    assert_eq!(a, b);
    for c in &a {
        for (id, l) in c {
            let s1 = d1.class[id];
            assert!(*l == s1 || (s1 == Label::Normal && *l == Label::PositionOffset));
        }
    }
}

/// The recorded scored runs agree with the pre-registered bars: V3 and V3b NOT MET, missing
/// V3-B4 and V3-G1, with the guards V3-G2 to V3-G4 met.
#[test]
fn v3_and_v3b_recorded_verdicts() {
    for f in ["results.json", "V3b_results.json"] {
        let d: Value =
            serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/{f}")).unwrap()).unwrap();
        let b = bars(&d["metrics"]);
        assert_eq!(b, [false, false, true, true, true], "{f}");
        assert_eq!(d["met"], false, "{f}");
        assert_eq!(d["bars"]["V3-B4"], false);
        assert_eq!(d["bars"]["V3-G1"], false);
    }
}

// ---------------------------------------------------------------------------------------
// V3c (2026-11-16_V3c_PREREGISTRATION.md)
// ---------------------------------------------------------------------------------------

/// V3c: the recall ceiling on development seeds 70 to 79, from the generator's truth only,
/// published before the method is built. Write with KSHANA_PRO_V3C_CEILING=1.
#[test]
fn v3c_recall_ceiling_on_development_seeds_70_to_79() {
    if std::env::var_os("KSHANA_PRO_V3C_CEILING").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3C_CEILING=1 to compute the V3c ceiling");
        return;
    }
    use kshana_pro::integrity::offset_v5;
    let mut acc: std::collections::BTreeMap<(usize, u32, u32), (usize, usize)> = Default::default();
    for seed in 70..80 {
        let scene = generate(seed);
        for m in offset_v5::M_GRID {
            for ts in offset_v5::T_SPAN_GRID {
                for (km, (k, n)) in offset_v5::recall_ceiling(&scene, m, ts) {
                    let e = acc.entry((m, ts, km)).or_insert((0, 0));
                    e.0 += k;
                    e.1 += n;
                }
            }
        }
        eprintln!("seed {seed} done");
    }
    let mut rows = Vec::new();
    for m in offset_v5::M_GRID {
        for ts in offset_v5::T_SPAN_GRID {
            let mut by = serde_json::Map::new();
            for km in [2u32, 5, 10, 20, 50] {
                let (k, n) = acc[&(m, ts, km)];
                by.insert(
                    km.to_string(),
                    json!({"reachable": k, "n": n, "ceiling": k as f64 / n as f64}),
                );
            }
            let blocks = [10u32, 20, 50].iter().any(|km| {
                let (k, n) = acc[&(m, ts, *km)];
                (k as f64) / (n as f64) < 0.98
            });
            rows.push(json!({"m": m, "t_span_s": ts, "by_km": by, "below_0_98_at_10_20_or_50_km": blocks}));
        }
    }
    let doc = json!({
        "pre_registration": "2026-11-16_V3c_PREREGISTRATION.md",
        "what": "V3c recall ceiling on development seeds 70 to 79: share of POSITION-OFFSET vessels with at least m post-onset RF fixes of their own (generator truth: rf_owner, onset_s) whose first and last are at least T_span apart. Computed from the generator's truth only, before the V3c method was built.",
        "seeds": "70 to 79",
        "ceiling": rows,
    });
    std::fs::write(
        format!("{DIR}/V3c_dev_ceiling.json"),
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
}

fn v3c_rows(seed: u64, grid: &[offset_v5_p::Params]) -> (Vec<Row>, Vec<Vec<Row>>, Vec<usize>) {
    let scene = generate(seed);
    let d1 = decide(&scene, &M4Params::v3());
    let base = decided_classes(&scene, &d1);
    let out = offset_v5_p::stage2_grid(&scene, &d1, grid);
    let n_found = out.iter().map(|(_, f)| f.len()).collect();
    let per = out
        .into_iter()
        .map(|(d, _)| decided_classes(&scene, &d))
        .collect();
    (base, per, n_found)
}

use kshana_pro::integrity::offset_v5 as offset_v5_p;

/// V3c development run on seeds 70 to 79 over the pre-registered grid, by V3's rule; writes
/// `V3c_dev.json` (with the ceiling published earlier in `V3c_dev_ceiling.json`).
#[test]
fn v3c_development_run_on_seeds_70_to_79() {
    if std::env::var_os("KSHANA_PRO_V3C_DEV").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3C_DEV=1 for the V3c development run");
        return;
    }
    let grid = offset_v5_p::GRID;
    let mut base_all = Vec::new();
    let mut per_all: Vec<Vec<Row>> = vec![Vec::new(); grid.len()];
    for seed in 70..80 {
        let (b, per, _) = v3c_rows(seed, &grid);
        base_all.extend(b);
        for (g, r) in per.into_iter().enumerate() {
            per_all[g].extend(r);
        }
        eprintln!("seed {seed} done");
    }
    let mut table = Vec::new();
    for (g, p) in grid.iter().enumerate() {
        let m = metrics(&per_all[g]);
        let (k, n) = recall_ge5(&m);
        table.push(json!({
            "m": p.m, "t_span_s": p.t_span_s,
            "false_alarm": m["false_alarm"],
            "offset_recall_ge5": k as f64 / n as f64, "offset_hits_ge5": k, "offset_n_ge5": n,
            "bars": {"V3-B4": bars(&m)[0], "V3-G1": bars(&m)[1], "V3-G2": bars(&m)[2], "V3-G3": bars(&m)[3], "V3-G4": bars(&m)[4]},
            "metrics": m,
        }));
    }
    let eligible: Vec<usize> = (0..grid.len())
        .filter(|&g| table[g]["false_alarm"].as_f64().unwrap() <= 0.005)
        .collect();
    let chosen = if eligible.is_empty() {
        (0..grid.len())
            .min_by(|&a, &b| {
                table[a]["false_alarm"]
                    .as_f64()
                    .unwrap()
                    .total_cmp(&table[b]["false_alarm"].as_f64().unwrap())
            })
            .unwrap()
    } else {
        let mut best = eligible[0];
        for &g in &eligible[1..] {
            if table[g]["offset_hits_ge5"].as_u64() > table[best]["offset_hits_ge5"].as_u64() {
                best = g;
            }
        }
        best
    };
    let ceiling: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{DIR}/V3c_dev_ceiling.json")).unwrap(),
    )
    .unwrap();
    let doc = json!({
        "pre_registration": "2026-11-16_V3c_PREREGISTRATION.md",
        "method": offset_v5_p::METHOD_ID,
        "seeds": "70 to 79",
        "stage1_open_v3": metrics(&base_all),
        "grid": table,
        "eligible": eligible,
        "chosen": {"m": grid[chosen].m, "t_span_s": grid[chosen].t_span_s},
        "recall_ceiling": ceiling["ceiling"],
    });
    std::fs::write(
        format!("{DIR}/V3c_dev.json"),
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
}

/// V3c scored once on seeds 7000 to 7009 with the committed pair; writes `V3c_results.json`.
#[test]
fn v3c_scored_run_on_seeds_7000_to_7009() {
    if std::env::var_os("KSHANA_PRO_V3C_SCORE").is_none() {
        eprintln!("skipped: set KSHANA_PRO_V3C_SCORE=1 for the one V3c scored run");
        return;
    }
    let p = offset_v5_p::V3C_CHOSEN;
    let mut base_all = Vec::new();
    let mut all = Vec::new();
    let mut found = 0usize;
    for seed in 7000..7010 {
        let (b, per, n) = v3c_rows(seed, &[p]);
        base_all.extend(b);
        all.extend(per.into_iter().next().unwrap());
        found += n[0];
        eprintln!("seed {seed} done");
    }
    let m = metrics(&all);
    let b = bars(&m);
    let doc = json!({
        "pre_registration": "2026-11-16_V3c_PREREGISTRATION.md",
        "method": offset_v5_p::METHOD_ID,
        "seeds": "7000 to 7009",
        "params": {"m": p.m, "t_span_s": p.t_span_s},
        "stage2_findings": found,
        "stage1_open_v3": metrics(&base_all),
        "metrics": m,
        "bars": {"V3-B4": b[0], "V3-G1": b[1], "V3-G2": b[2], "V3-G3": b[3], "V3-G4": b[4]},
        "met": b.iter().all(|x| *x),
    });
    std::fs::write(
        format!("{DIR}/V3c_results.json"),
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
}

/// V3-G5 and V3-G6 for V3c: the same seed decided twice gives the same classes and flags; stage
/// 1 is the open engine's decision byte for byte; stage 2 changes only NORMAL to POSITION-OFFSET
/// and clears only fix flags of accepted supports.
#[test]
fn v3c_g5_g6_determinism_and_stage1_on_a_development_seed() {
    let scene = generate(70);
    let d1 = decide(&scene, &M4Params::v3());
    let d1b = decide(&scene, &M4Params::v3());
    assert_eq!(d1.class, d1b.class);
    assert_eq!(d1.dark_fix, d1b.dark_fix);
    let a = offset_v5_p::stage2_grid(&scene, &d1, &[offset_v5_p::V3C_CHOSEN]);
    let b = offset_v5_p::stage2_grid(&scene, &d1, &[offset_v5_p::V3C_CHOSEN]);
    assert_eq!(a[0].0.class, b[0].0.class);
    assert_eq!(a[0].0.dark_fix, b[0].0.dark_fix);
    assert_eq!(a[0].1, b[0].1);
    for (id, l) in &a[0].0.class {
        let s1 = d1.class[id];
        assert!(*l == s1 || (s1 == Label::Normal && *l == Label::PositionOffset));
    }
    let support: std::collections::BTreeSet<usize> = a[0]
        .1
        .iter()
        .flat_map(|f| f.fixes.iter().copied())
        .collect();
    for (j, (x, y)) in d1.dark_fix.iter().zip(&a[0].0.dark_fix).enumerate() {
        assert!(x == y || (support.contains(&j) && *x && !*y));
    }
    // Exclusivity: no fix supports two accepted hypotheses.
    let total: usize = a[0].1.iter().map(|f| f.fixes.len()).sum();
    assert_eq!(total, support.len());
}

/// The recorded V3c scored run (seeds 7000 to 7009, run once): NOT MET, missing V3-B4 only;
/// V3-G1 to V3-G4 met.
#[test]
fn v3c_recorded_verdict() {
    let d: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{DIR}/V3c_results.json")).unwrap())
            .unwrap();
    assert_eq!(bars(&d["metrics"]), [false, true, true, true, true]);
    assert_eq!(d["met"], false);
    assert_eq!(d["params"]["m"], 4);
    assert_eq!(d["params"]["t_span_s"], 1200);
}
