// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! V4: the multi-source switching bars of
//! `docs/evidence/v4-multi-source-switching/2026-11-15_PREREGISTRATION.md`, scored on D-R1 and
//! D-R2 (recorded IGS clocks, modelled sources, injected faults) and D-S (synthetic). The printed
//! table is the one in `RESULTS.md`; the assertions pin each outcome, misses included.

use kshana_pro::multi_source::v4::{self, Bars, Run};
use kshana_pro::multi_source::{self, Config, Evidence};

const CLOCKS_R1: &str = include_str!("fixtures/timing_v5_igs/clocks.txt");
const CLOCKS_R2: &str = include_str!("fixtures/timing_v5b_igs/clocks.txt");

fn sha(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b))
}

fn show(b: &Bars, t: &v4::Tally) {
    println!(
        "{}: runs {} (dropped {}), epochs {}, with output {}, holdover {}, none {}",
        b.name, t.runs, t.dropped, t.epochs, t.output_epochs, t.holdover_epochs, t.none_epochs
    );
    println!(
        "  B1 misleading {}/{} = {:.5} % -> {}",
        t.misleading,
        t.output_epochs,
        100.0 * b.b1_share,
        b.b1
    );
    println!(
        "  B2 deselection {}/{} = {:.2} % (assessable: {}) -> {}; by type {:?}",
        t.episodes_passed,
        t.episodes_assessable,
        100.0 * b.b2_share,
        b.b2_assessable,
        b.b2,
        t.by_type
    );
    println!(
        "  B3a p95 {:.3} ns / oracle {:.3} ns = {:.4} -> {}",
        b.p95_sel * 1e9,
        b.p95_oracle * 1e9,
        b.b3a_ratio,
        b.b3a
    );
    println!(
        "  B3b p99.9 {:.3} ns vs GNSS-only {:.3} ns -> {}",
        b.b3b_sel * 1e9,
        b.b3b_gnss * 1e9,
        b.b3b
    );
    println!(
        "  B4 changes {} <= {} (transitions {}) -> {}",
        b.b4_changes, b.b4_limit, t.transitions, b.b4
    );
    println!(
        "  B5 external {}/{} = {:.3} % -> {}",
        t.b5_external,
        t.b5_eligible,
        100.0 * b.b5_share,
        b.b5
    );
}

/// B6 on the first run of a data set: two runs byte-identical, the chain verifies, one record
/// per epoch, a one-byte change breaks verification.
fn b6(run: &Run, set: u64) -> bool {
    let cfg = Config::v4(&run.class, run.tau0);
    let gen = v4::generate(run, &cfg, v4::seed(set, 0));
    let text = v4::table(run, &gen);
    let rows = multi_source::parse_table(&cfg, &text).unwrap();
    let a = multi_source::run_table(&cfg, &rows, &sha(text.as_bytes())).unwrap();
    let b = multi_source::run_table(&cfg, &rows, &sha(text.as_bytes())).unwrap();
    let identical = a.decisions_csv == b.decisions_csv
        && serde_json::to_string(&a.evidence).unwrap()
            == serde_json::to_string(&b.evidence).unwrap();
    let verifies = a.evidence.verify(&a.decisions_csv).is_empty();
    let every_epoch = a.decisions_csv.lines().count() - 1 == run.x.len();
    // One byte changed in the middle of the decisions table.
    let mut bytes = a.decisions_csv.clone().into_bytes();
    let mid = bytes.len() / 2;
    bytes[mid] = if bytes[mid] == b'1' { b'2' } else { b'1' };
    let tampered = String::from_utf8(bytes).unwrap();
    let breaks = !a.evidence.verify(&tampered).is_empty();
    // And a changed switch record.
    let mut ev: Evidence = a.evidence.clone();
    if let Some(s) = ev.switches.get_mut(0) {
        s.reason.push('x');
    }
    let breaks_chain = !ev.verify(&a.decisions_csv).is_empty();
    println!(
        "  B6 identical {identical}, verifies {verifies}, every epoch {every_epoch}, one-byte change detected {breaks}, record change detected {breaks_chain}"
    );
    identical && verifies && every_epoch && breaks && breaks_chain
}

#[test]
fn v4_scored_as_pre_registered() {
    let (r1, d1) = v4::igs_runs(CLOCKS_R1);
    let (r2, d2) = v4::igs_runs(CLOCKS_R2);
    let s = v4::synthetic_runs();
    assert_eq!(
        sha(CLOCKS_R1.as_bytes()),
        "d813620393b4b4905aaedc4eaff8a5f2aad29a2a356b54cb2f06ad9810055de3"
    );
    assert_eq!(
        sha(CLOCKS_R2.as_bytes()),
        "3da6d0a5a7facdc1e564fdeed5f5ada890a2f9cdab6a218fa79a83608a3230a4"
    );
    let mut all = Vec::new();
    for (name, set, runs, dropped) in [
        ("D-R1", 1u64, &r1, d1),
        ("D-R2", 2, &r2, d2),
        ("D-S", 3, &s, 0),
    ] {
        let (b, t) = v4::score_set(name, set, runs, dropped);
        show(&b, &t);
        let ok6 = b6(&runs[0], set);
        all.push((b, t, ok6));
    }
    let pass = all.iter().all(|(b, _, ok6)| b.passed() && *ok6);
    println!("V4 passes: {pass}");
    // Pinned outcomes (RESULTS.md).
    let verdicts: Vec<(String, [bool; 7])> = all
        .iter()
        .map(|(b, _, ok6)| (b.name.clone(), [b.b1, b.b2, b.b3a, b.b3b, b.b4, b.b5, *ok6]))
        .collect();
    println!("{verdicts:?}");
    assert!(
        pass,
        "V4 was scored as passing on every data set (RESULTS.md)"
    );
    for (_, v) in &verdicts {
        assert_eq!(*v, [true; 7]);
    }
    // The pinned counts: (runs, epochs, misleading, episodes assessable, passed, changes,
    // transitions, B5 eligible, B5 external, holdover epochs).
    let counts: Vec<[usize; 10]> = all
        .iter()
        .map(|(_, t, _)| {
            [
                t.runs,
                t.epochs,
                t.misleading,
                t.episodes_assessable,
                t.episodes_passed,
                t.changes,
                t.transitions,
                t.b5_eligible,
                t.b5_external,
                t.holdover_epochs,
            ]
        })
        .collect();
    assert_eq!(
        counts,
        vec![
            [12, 44349, 7, 767, 764, 1430, 16068, 43685, 43685, 477],
            [12, 44064, 8, 754, 754, 1451, 16045, 43420, 43420, 465],
            [10, 864000, 256, 50, 49, 259, 1098, 852509, 852507, 9002],
        ]
    );
    let ratios: Vec<String> = all
        .iter()
        .map(|(b, _, _)| {
            format!(
                "{:.4} {:.3} {:.3}",
                b.b3a_ratio,
                b.b3b_sel * 1e9,
                b.b3b_gnss * 1e9
            )
        })
        .collect();
    assert_eq!(
        ratios,
        vec![
            "1.1429 139.311 3559.690",
            "1.1079 136.881 3518.841",
            "1.0055 120.001 3511.657",
        ]
    );
}
