// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! INVARIANT (A02 / D1.1): the committed benefit-vs-baseline deliverable is
//! byte-identical to the output of `kshana-pro benefits` (the float-free provenance
//! skeleton produced by [`kshana_pro::benefits::reference_case`]). This makes the
//! "reproducible benefit table a prime drops into D1.1" deliverable genuinely
//! reachable AND drift-guarded — not hand-written prose that could diverge from the
//! engine. Numeric ratios are intentionally NOT in the committed block (the open
//! core's reproducibility doctrine anchors cross-platform stability on the scenario
//! hash, not on result floats); they are emitted on demand by `--json` / `--svg`.

const DOC: &str =
    include_str!("../docs/tenders/cross-tender/D1.1-benefit-vs-baseline-reference.md");

const BEGIN: &str = "<!-- BEGIN GENERATED: `kshana-pro benefits` -->";
const END: &str = "<!-- END GENERATED -->";

#[test]
fn committed_benefit_deliverable_matches_the_generated_skeleton() {
    let start = DOC
        .find(BEGIN)
        .expect("benefit deliverable is missing the BEGIN GENERATED marker")
        + BEGIN.len();
    let end = DOC
        .find(END)
        .expect("benefit deliverable is missing the END GENERATED marker");
    assert!(end > start, "END GENERATED marker precedes BEGIN");

    let embedded = DOC[start..end].trim();
    let generated = kshana_pro::benefits::reference_case()
        .expect("reference benefit case runs")
        .to_provenance_markdown();

    assert_eq!(
        embedded,
        generated.trim(),
        "The D1.1 benefit deliverable has drifted from `kshana-pro benefits` output. \
         Regenerate it:\n  cargo run -q -- benefits\nthen re-embed the output between \
         the BEGIN/END GENERATED markers in \
         docs/tenders/cross-tender/D1.1-benefit-vs-baseline-reference.md"
    );
}

#[test]
fn reference_benefit_case_is_in_process_reproducible_and_honest() {
    // Determinism in-process (the part the doctrine DOES guarantee), plus the honesty
    // invariant: a MODELLED-vs-MODELLED reference must not silently cross evidence tiers.
    let a = kshana_pro::benefits::reference_case().unwrap();
    let b = kshana_pro::benefits::reference_case().unwrap();
    assert_eq!(
        a.to_json(),
        b.to_json(),
        "benefit case must be reproducible"
    );
    assert!(!a.rows.is_empty(), "reference case has figures of merit");
    assert!(
        a.rows
            .iter()
            .all(|r| r.candidate_label == "MODELLED" && r.baseline_label == "MODELLED"),
        "reference case is MODELLED on both sides"
    );
    assert!(
        !a.any_crosses_label,
        "MODELLED-vs-MODELLED reference must not flag a mixed evidence tier"
    );
}
