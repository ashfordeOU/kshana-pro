// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! INVARIANT (A09): the JSON Schema embedded in the SpaceStream ICD is byte-identical
//! to `spacestream::json_schema()` (emitted by `kshana-pro spacestream --schema`).
//! The codec's golden messages are already validated against the schema by the unit
//! tests; this closes the last gap — the *ICD document* a prime integrates against
//! cannot drift from the codec it documents. The schema is static structural JSON
//! (no result floats), so a byte-identical guard is safe across platforms.

// Arksa-exclusive (docs/PROTECTION.md): runs only in a build with the `arksa` feature.
#![cfg(feature = "arksa")]

const ICD: &str = include_str!("../docs/tenders/13494-ai-payload-pnt/SpaceStream-ICD.md");

const BEGIN: &str = "<!-- BEGIN GENERATED: `kshana-pro spacestream --schema` -->";
const END: &str = "<!-- END GENERATED -->";

#[test]
fn icd_embedded_schema_matches_the_codec() {
    let start = ICD
        .find(BEGIN)
        .expect("ICD is missing the BEGIN GENERATED marker")
        + BEGIN.len();
    let end = ICD
        .find(END)
        .expect("ICD is missing the END GENERATED marker");
    assert!(end > start, "END GENERATED marker precedes BEGIN");

    // The embedded block is a ```json fenced code block; strip the fences.
    let block = ICD[start..end].trim();
    let embedded = block
        .trim_start_matches("```json")
        .trim_end_matches("```")
        .trim();

    let generated = kshana_pro::connectors::spacestream::json_schema();

    assert_eq!(
        embedded,
        generated.trim(),
        "The SpaceStream ICD's embedded schema has drifted from the codec. \
         Regenerate it:\n  cargo run -q --features arksa -- spacestream --schema --dev\nthen re-embed the \
         output (inside the ```json fences) between the BEGIN/END GENERATED markers in \
         docs/tenders/13494-ai-payload-pnt/SpaceStream-ICD.md"
    );
}
