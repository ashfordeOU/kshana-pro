// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! INVARIANT: the worked example in `docs/packs/DETECTOR-INTAKE-FORMAT.md` is the
//! exact output of [`kshana_pro::packs::detector_intake`]. The doc shows a customer
//! `auc_id = 1.0`, `auc_ood = 0.75`, `optimism_gap = 0.25`; if the engine ever
//! computed something else (or the doc were edited to claim a number the code does
//! not produce), this fails — the buyable format cannot drift from the code.

use kshana_pro::packs::detector_intake::{gap_from_samples, DetectorIntake, Distribution};

const DOC: &str = include_str!("../docs/packs/DETECTOR-INTAKE-FORMAT.md");

/// The customer-facing JSON example block in the doc parses, and the engine produces
/// the exact result the doc states.
#[test]
fn doc_worked_example_input_matches_documented_result() {
    // The first fenced ```json block in the doc is the worked-example INPUT.
    let input = first_json_block(DOC, "samples").expect("doc has a samples JSON block");
    let doc_input: DetectorIntake =
        serde_json::from_str(input).expect("doc worked-example input parses");
    let g = doc_input.gap().expect("doc worked example computes");

    // The exact numbers the doc claims.
    assert!(
        (g.auc_id - 1.0).abs() < 1e-12,
        "doc claims auc_id 1.0, got {}",
        g.auc_id
    );
    assert!(
        (g.auc_ood - 0.75).abs() < 1e-12,
        "doc claims auc_ood 0.75, got {}",
        g.auc_ood
    );
    assert!(
        (g.optimism_gap - 0.25).abs() < 1e-12,
        "doc claims optimism_gap 0.25, got {}",
        g.optimism_gap
    );
    assert_eq!(g.n_id, 4);
    assert_eq!(g.n_ood, 4);

    // The doc states the orientation/honesty essentials.
    assert!(DOC.contains("Higher = more likely the impairment is present"));
    assert!(DOC.contains("not a certification"));
    assert!(DOC.contains("arXiv:2606.22054"));
    assert!(DOC.contains("preprint"));
}

/// The same result reached via the free function on hand-built samples (a second,
/// independent check that the documented numbers are reproducible).
#[test]
fn free_function_reproduces_documented_numbers() {
    let s = |score: f64, label: bool, d: Distribution| {
        kshana_pro::packs::detector_intake::ScoredSample {
            score,
            label,
            distribution: d,
        }
    };
    let samples = vec![
        s(9.0, true, Distribution::Id),
        s(8.0, true, Distribution::Id),
        s(2.0, false, Distribution::Id),
        s(1.0, false, Distribution::Id),
        s(7.0, true, Distribution::Ood),
        s(3.0, true, Distribution::Ood),
        s(4.0, false, Distribution::Ood),
        s(2.0, false, Distribution::Ood),
    ];
    let g = gap_from_samples(&samples).unwrap();
    assert!((g.auc_ood - 0.75).abs() < 1e-12);
    assert!((g.optimism_gap - 0.25).abs() < 1e-12);
}

/// Find the first ```json fenced block whose body contains `needle`.
fn first_json_block<'a>(md: &'a str, needle: &str) -> Option<&'a str> {
    let mut rest = md;
    while let Some(open) = rest.find("```json") {
        let after = &rest[open + "```json".len()..];
        let body_start = after.find('\n')? + 1;
        let body = &after[body_start..];
        let close = body.find("```")?;
        let block = &body[..close];
        if block.contains(needle) {
            return Some(block);
        }
        rest = &body[close + 3..];
    }
    None
}
