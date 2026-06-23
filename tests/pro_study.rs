// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! ProStudy multi-pack orchestration guard.
//!
//! A `ProStudy` composes one or more pack report sections into a SINGLE branded
//! study document: ONE shared cover + provenance + the always-on MODELLED /
//! "not a certification" banner, then each present pack's section in order. The
//! honesty furniture (the MODELLED banner and the literal "not a certification"
//! disclaimer) must appear EXACTLY ONCE — never duplicated per pack — and the
//! Pack-A content (the optimism-gap headline + the arXiv preprint citation) must
//! survive composition. An empty `ProStudy` (no sections) must still render a
//! valid, branded document (back-compat / empty case).

use kshana_pro::packs::optimism_gap_audit::OptimismGapAuditScenario;
use kshana_pro::pro_study::{ProSection, ProStudy};
use kshana_pro::report::{StudyCover, StudyProvenance};
use kshana_pro::report_pack_a::PackAReport;

fn fast_audit() -> kshana_pro::packs::optimism_gap_audit::OptimismGapAudit {
    OptimismGapAuditScenario {
        n_per_class: 120,
        frac_train: 0.7,
        severities: vec![0.3, 0.6, 0.9],
        seeds: vec![1, 2, 3],
        target_pfa: 0.05,
        ridge_lambda: 0.1,
        n_perms: 50,
        perm_seed: 7,
    }
    .compute()
    .expect("audit computes")
}

/// Count non-overlapping occurrences of `needle` in `hay`.
fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// A ProStudy with a single Pack-A section renders: the cover/title, EXACTLY ONE
/// MODELLED banner + ONE "not a certification", the Pack-A gap headline, and the
/// preprint citation.
#[test]
fn single_pack_a_section_renders_one_cover_one_banner_and_pack_content() {
    let a = fast_audit();
    let pack_a = PackAReport::from_audit("Pack A — Optimism-Gap Audit", &a);
    let study = ProStudy::new("Pro Study — Multi-Pack").with_pack_a("Pack A", &pack_a);
    let html = study.to_html();

    // A valid, branded document.
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.ends_with("</html>"));
    assert!(
        html.contains("Pro Study — Multi-Pack"),
        "study title on cover"
    );
    assert!(html.contains("Ashforde"), "Ashforde identity on the cover");

    // THE MOAT: the shared MODELLED banner appears EXACTLY ONCE — a single banner at
    // the top of the document, never repeated per pack. `MODELLED RESULTS &mdash;`
    // is the banner's unique marker (the per-pack citation may honestly restate
    // "not a certification" about its preprint, and the footer carries the standing
    // tagline — but the BANNER itself is rendered once).
    let modelled_banners = count(&html, "MODELLED RESULTS &mdash;");
    assert_eq!(
        modelled_banners, 1,
        "exactly one MODELLED banner, got {modelled_banners}"
    );
    // The "not a certification" disclaimer is present (it rides on the single banner
    // and is also restated in the footer / honest per-pack citation).
    assert!(
        html.contains("not a certification"),
        "the 'not a certification' disclaimer must be present"
    );

    // The Pack-A content survives composition: the optimism-gap headline + AUC
    // numbers and the preprint citation.
    assert!(html.contains("optimism gap"), "Pack-A gap headline present");
    assert!(html.contains(&format!("{:.3}", a.optimism_gap_mean)));
    assert!(html.contains(&format!("{:.3}", a.auc_id_mean)));
    assert!(html.contains(&format!("{:.3}", a.auc_ood_mean)));
    // The proof stays an arXiv preprint — never a certification / peer-reviewed.
    assert!(html.contains("arXiv:2606.22054"));
    assert!(html.contains("arXiv preprint"));
    assert!(!html.to_lowercase().contains("peer-reviewed result"));
}

/// A ProStudy with NO sections still renders a valid, branded document with the
/// non-optional honesty furniture present exactly once (back-compat / empty case).
#[test]
fn empty_pro_study_renders_valid_branded_document() {
    let study = ProStudy::new("Empty Pro Study");
    let html = study.to_html();

    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.ends_with("</html>"));
    assert!(html.contains("Empty Pro Study"), "title on the cover");
    assert!(html.contains("Ashforde"));

    // The single shared MODELLED banner is present exactly once even with no
    // sections; the "not a certification" disclaimer rides on it (and the footer).
    assert_eq!(count(&html, "MODELLED RESULTS &mdash;"), 1);
    assert!(html.contains("not a certification"));
}

/// Cover + provenance attach once at the study level and render on the face.
#[test]
fn study_level_cover_and_provenance_render_once() {
    let a = fast_audit();
    let pack_a = PackAReport::from_audit("Pack A", &a);
    let study = ProStudy::new("Composed Study")
        .with_cover(StudyCover {
            subtitle: Some("Multi-pack engagement".into()),
            executive_summary: Some("Board-level summary across packs.".into()),
        })
        .with_provenance(StudyProvenance {
            study_id: Some("KP-2026-PROSTUDY".into()),
            customer: Some("ACME".into()),
            classification: Some("Commercial-in-Confidence".into()),
            ..Default::default()
        })
        .with_pack_a("Pack A", &pack_a);
    let html = study.to_html();

    assert!(html.contains("Multi-pack engagement"));
    assert!(html.contains("Board-level summary across packs."));
    assert!(html.contains("KP-2026-PROSTUDY"));
    assert!(html.contains("Commercial-in-Confidence"));
    // Still exactly one shared banner, even with custom cover copy.
    assert_eq!(count(&html, "MODELLED RESULTS &mdash;"), 1);
    assert!(html.contains("not a certification"));
    // Pack content still present.
    assert!(html.contains("arXiv:2606.22054"));
}

/// The `ProSection` shape is constructible directly (extensible: B/C are simply
/// not added yet — a Vec of sections, not three hardcoded packs).
#[test]
fn pro_section_is_constructible_directly() {
    let a = fast_audit();
    let pack_a = PackAReport::from_audit("Pack A", &a);
    let section = ProSection::new("Pack A", pack_a.section_fragment_html());
    let study = ProStudy::new("Direct Section Study").with_section(section);
    let html = study.to_html();
    assert_eq!(count(&html, "MODELLED RESULTS &mdash;"), 1);
    assert!(html.contains("arXiv:2606.22054"));
}
