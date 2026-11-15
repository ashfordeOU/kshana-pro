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
