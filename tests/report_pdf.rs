// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! Pure-Rust PDF export guard for the branded study report.
//!
//! `kshana_pro::report_pdf::to_pdf` renders the SAME structured report the HTML
//! face renders (cover title, executive summary, the MODELLED + "not a
//! certification" banner, the FoM table with inline `kshana::fom_label` tiers, and
//! the provenance block) — from the report's data, never by parsing the HTML.
//!
//! Two guards:
//!  (a) the output is a real PDF (`%PDF-` magic, non-trivial length); and
//!  (b) it is **reproducible** — two `to_pdf` calls on the same report are
//!      byte-identical, EXCEPT for the PDF trailer `/ID` array, which printpdf
//!      0.9.1 fills with two random 32-char strings inside `serialize_pdf` with NO
//!      override hook (`serialize.rs:386-387`). That one field is the only
//!      unavoidable nondeterminism; we mask exactly it and assert the rest is
//!      identical. All document dates (CreationDate/ModDate/metadata) are pinned to
//!      the Unix epoch by printpdf's `PdfDocumentInfo::default()`, so they do NOT
//!      vary — the library never reads our clock and neither do we.

use kshana_pro::report::{build_report, ConsolidatedReport, StudyCover, StudyProvenance};
use kshana_pro::report_pdf::to_pdf;

fn clock_scenario() -> String {
    "seed = 42\n\
     threshold_ns = 20.0\n\
     [time]\n\
     step_s = 10.0\n\
     duration_s = 7200.0\n\
     [gnss]\n\
     windows = [\n\
       { t0 = 0.0,   t1 = 600.0,  state = \"nominal\" },\n\
       { t0 = 600.0, t1 = 7200.0, state = \"denied\" },\n\
     ]\n\
     [clock_quantum]\n\
     id = \"optical-sr-lattice\"\n\
     provenance = \"demo; q_wf=sigma_y(1s)^2\"\n\
     y0   = 5.0e-17\n\
     q_wf = 1.0e-30\n\
     q_rw = 0.0\n\
     [clock_classical]\n\
     id = \"csac-sa45s\"\n\
     provenance = \"demo; q_wf=sigma_y(1s)^2\"\n\
     y0   = 5.0e-10\n\
     q_wf = 9.0e-20\n\
     q_rw = 0.0\n"
        .into()
}

/// A report carrying a cover + provenance — the customer-facing surface.
fn report() -> ConsolidatedReport {
    build_report(
        "Quantum Clock NAV-DT Resilience Study",
        &clock_scenario(),
        "quantum",
    )
    .unwrap()
    .with_cover(StudyCover {
        subtitle: Some("Performance & Resilience Study".into()),
        executive_summary: Some("A bespoke executive summary paragraph for the PDF face.".into()),
    })
    .with_provenance(StudyProvenance {
        study_id: Some("KP-2026-0007".into()),
        customer: Some("Acme Space GmbH".into()),
        prepared_for: Some("Procurement Review Board".into()),
        prepared_utc: Some("2026-06-23T12:00:00Z".into()),
        classification: Some("Restricted".into()),
        ..Default::default()
    })
}

/// (a) The result is Ok, the bytes are a real PDF, and the document is non-trivial.
#[test]
fn to_pdf_emits_a_real_pdf() {
    let bytes = to_pdf(&report()).expect("to_pdf must succeed");
    assert!(
        bytes.starts_with(b"%PDF-"),
        "output must begin with the PDF magic header"
    );
    assert!(
        bytes.len() > 1000,
        "a full study report PDF is well over 1 KB (got {})",
        bytes.len()
    );
}

/// Mask the single unavoidably-nondeterministic field: the trailer `/ID` array's
/// two 32-character random literal strings (printpdf fills these in
/// `serialize_pdf`, with no API hook to pin them). Everything else must match.
///
/// The trailer reads `/ID[ (<32 chars>) (<32 chars>) ]`. We blank the contents of
/// the two `(...)` literals that follow the `/ID[` marker, leaving structure and
/// every other byte intact, so any *real* nondeterminism would still trip the test.
fn mask_pdf_id(bytes: &[u8]) -> Vec<u8> {
    let needle = b"/ID[";
    let mut out = bytes.to_vec();
    if let Some(pos) = bytes.windows(needle.len()).position(|w| w == needle) {
        // From the marker, blank the bytes inside the next two parenthesised
        // literals (the document-id and instance-id) to a fixed sentinel.
        let mut i = pos + needle.len();
        let mut literals_blanked = 0;
        while i < out.len() && literals_blanked < 2 {
            if out[i] == b'(' {
                i += 1;
                while i < out.len() && out[i] != b')' {
                    out[i] = b'#';
                    i += 1;
                }
                literals_blanked += 1;
            } else if out[i] == b']' {
                break;
            } else {
                i += 1;
            }
        }
    }
    out
}

/// (b) Determinism: two renders of the same report are byte-identical once the
/// documented `/ID` field is masked. (Sanity: the masking is doing real work — the
/// raw outputs differ ONLY because of `/ID`.)
#[test]
fn to_pdf_is_deterministic_modulo_documented_id_field() {
    let r = report();
    let a = to_pdf(&r).expect("first render");
    let b = to_pdf(&r).expect("second render");

    // The raw bytes differ only in the random /ID; lengths stay equal because both
    // ids are always exactly 32 chars.
    assert_eq!(a.len(), b.len(), "same report → same byte length");

    // After masking the one documented field, the two renders are byte-identical.
    assert_eq!(
        mask_pdf_id(&a),
        mask_pdf_id(&b),
        "two renders of the same report must be byte-identical apart from the \
         masked /ID array"
    );

    // Guard the guard: the mask actually changed something (the /ID marker exists
    // and was found), so we are not vacuously passing on unmasked bytes.
    assert!(
        a.windows(b"/ID[".len()).any(|w| w == b"/ID["),
        "the trailer /ID array must be present (the field we mask)"
    );
}

/// The PDF face carries the same non-negotiable honesty furniture as the HTML
/// face: the MODELLED banner and the literal "not a certification" disclaimer must
/// appear as visible text operators in the content stream.
#[test]
fn pdf_carries_modelled_banner_and_not_a_certification() {
    let bytes = to_pdf(&report()).expect("render");
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("MODELLED"),
        "the MODELLED banner text must be present in the PDF content"
    );
    assert!(
        text.contains("not a certification"),
        "the 'not a certification' disclaimer must be present in the PDF content"
    );
}
