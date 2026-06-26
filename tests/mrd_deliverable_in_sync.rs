// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! INVARIANT (A03 / D1.4): the committed Mission Requirements Document deliverable
//! is byte-identical to the output of the machine-checked [`kshana_pro::mrd`] table.
//!
//! A03 promises the MRD "cannot drift from the code". The unit tests in `mrd.rs`
//! already enforce that the *table* cannot drift from the engine (every measured row
//! resolves to a real FoM field + a live verification-matrix oracle). This test
//! closes the last gap: the *deliverable document* a prime drops into D1.4 is the
//! exact, regenerable output of `kshana-pro mrd` — not hand-maintained prose that
//! could silently diverge. If anyone edits the table without regenerating the doc,
//! or hand-edits the generated block, this fails.

const DOC: &str = include_str!("../docs/tenders/cross-tender/D1.4-mrd-fom-mapping.md");

const BEGIN: &str = "<!-- BEGIN GENERATED: `kshana-pro mrd` -->";
const END: &str = "<!-- END GENERATED -->";

#[test]
fn committed_d1_4_doc_matches_the_generated_table() {
    let start = DOC
        .find(BEGIN)
        .expect("D1.4 doc is missing the BEGIN GENERATED marker")
        + BEGIN.len();
    let end = DOC
        .find(END)
        .expect("D1.4 doc is missing the END GENERATED marker");
    assert!(end > start, "END GENERATED marker precedes BEGIN");

    let embedded = DOC[start..end].trim();
    let generated = kshana_pro::mrd::to_markdown();

    assert_eq!(
        embedded,
        generated.trim(),
        "The D1.4 MRD deliverable has drifted from `kshana-pro mrd` output. \
         Regenerate it:\n  cargo run -q -- mrd\nthen re-embed the output between \
         the `BEGIN GENERATED` / `END GENERATED` markers in \
         docs/tenders/cross-tender/D1.4-mrd-fom-mapping.md"
    );
}
