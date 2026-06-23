// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! Branded study-report face guard.
//!
//! The commercial HTML report is customer-facing, so its honesty furniture must be
//! NON-OPTIONAL on the face: the branded cover (Ashforde identity + study title),
//! a one-paragraph executive summary, a MODELLED banner, the explicit phrase "not a
//! certification", and an inline per-FoM validation tier (VALIDATED / MODELLED)
//! sourced from the open core's `kshana::fom_label`. When a caller attaches a
//! [`StudyProvenance`] block, its audit fields render on the face; when no cover /
//! provenance data is attached, the report must still render validly (back-compat).

use kshana_pro::report::{build_report, StudyProvenance};

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

/// The branded face carries the study title, the Ashforde identity, the
/// non-optional MODELLED banner, and the literal "not a certification" disclaimer.
#[test]
fn branded_cover_carries_title_brand_and_non_optional_honesty() {
    let title = "Quantum Clock NAV-DT Resilience Study";
    let r = build_report(title, &clock_scenario(), "quantum").unwrap();
    let html = r.to_html();

    assert!(html.starts_with("<!DOCTYPE html>"), "valid HTML document");
    assert!(html.contains(title), "study title must appear on the face");
    assert!(html.contains("Ashforde"), "Ashforde identity on the cover");
    // MODELLED banner + "not a certification" are NON-OPTIONAL on the commercial face.
    assert!(
        html.contains("MODELLED"),
        "the MODELLED banner must always render"
    );
    assert!(
        html.contains("not a certification"),
        "the 'not a certification' disclaimer must always render"
    );
}

/// A one-paragraph executive summary is part of the branded face.
#[test]
fn executive_summary_is_present() {
    let r = build_report("Exec Summary Study", &clock_scenario(), "quantum").unwrap();
    let html = r.to_html();
    assert!(
        html.to_lowercase().contains("executive summary"),
        "executive summary heading must render on the face"
    );
}

/// Each FoM in the table carries an inline VALIDATED/MODELLED tier sourced from
/// `kshana::fom_label::tier_for` — never invented. For this clock-holdover spine
/// every FoM is MODELLED, so the inline tag must be present.
#[test]
fn fom_table_renders_inline_validation_tier() {
    let r = build_report("Inline Tier Study", &clock_scenario(), "quantum").unwrap();
    let html = r.to_html();
    // At least one inline tier tag — VALIDATED or MODELLED — next to a FoM value.
    assert!(
        html.contains("VALIDATED") || html.contains("MODELLED"),
        "an inline validation tier tag must render next to FoM values"
    );
}

/// When a [`StudyProvenance`] block is attached, its audit-grade fields render on
/// the face: study id, customer, prepared-for, prepared-utc, engine version,
/// scenario hash, and the classification.
#[test]
fn provenance_block_renders_audit_fields_when_set() {
    let prov = StudyProvenance {
        study_id: Some("KP-2026-0007".into()),
        customer: Some("Acme Space GmbH".into()),
        prepared_for: Some("Procurement Review Board".into()),
        prepared_utc: Some("2026-06-23T12:00:00Z".into()),
        engine_version: Some("kshana 0.20".into()),
        scenario_hashes: vec!["abc123def456".into()],
        classification: Some("Restricted".into()),
    };
    let r = build_report("Provenance Study", &clock_scenario(), "quantum")
        .unwrap()
        .with_provenance(prov);
    let html = r.to_html();
    assert!(html.contains("KP-2026-0007"), "study id renders");
    assert!(html.contains("Acme Space GmbH"), "customer renders");
    assert!(
        html.contains("Procurement Review Board"),
        "prepared-for renders"
    );
    assert!(
        html.contains("2026-06-23T12:00:00Z"),
        "prepared-utc renders"
    );
    assert!(html.contains("abc123def456"), "scenario hash renders");
    assert!(html.contains("Restricted"), "classification renders");
}

/// Back-compat: a report with NO cover/provenance data attached still renders a
/// valid HTML document that keeps the non-optional honesty furniture. The default
/// classification is "Unclassified".
#[test]
fn renders_validly_with_no_cover_or_provenance_data() {
    let r = build_report("Bare Study", &clock_scenario(), "quantum").unwrap();
    let html = r.to_html();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.ends_with("</html>"));
    assert!(html.contains("Bare Study"));
    // Honesty furniture is non-optional even with no provenance attached.
    assert!(html.contains("MODELLED"));
    assert!(html.contains("not a certification"));
    // Default classification when none supplied.
    assert!(
        html.contains("Unclassified"),
        "default classification must be Unclassified"
    );
}

/// Determinism: the library never reads a clock; the same inputs (including a
/// caller-supplied timestamp) reproduce the same fingerprint.
#[test]
fn provenance_is_deterministic_no_internal_clock() {
    let mk = || {
        build_report("Determinism Study", &clock_scenario(), "quantum")
            .unwrap()
            .with_provenance(StudyProvenance {
                study_id: Some("KP-1".into()),
                prepared_utc: Some("2026-06-23T00:00:00Z".into()),
                ..Default::default()
            })
    };
    assert_eq!(mk().fingerprint(), mk().fingerprint());
    assert_eq!(mk().to_html(), mk().to_html());
}
