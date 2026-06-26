// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Regenerate the committed **SAMPLE** Pack-A study report (HTML + PDF) that the
//! Pack-A one-pager (`docs/packs/PACK-A.md`) links to.
//!
//! Run it from the crate root:
//!
//! ```text
//! cargo run -p kshana-pro --example gen_sample_study
//! ```
//!
//! It is fully reproducible and uses **no customer data**:
//!
//!  * the gap numbers are computed by the open optimism-gap engine via the Pack-A
//!    pack ([`kshana_pro::packs::optimism_gap_audit::OptimismGapAuditScenario`]) on a
//!    FIXED, seeded grid (a smaller-than-published grid — this is a *sample*, and the
//!    artifact says so);
//!  * the cover / provenance is a clearly **FICTITIOUS, illustrative** sample (the
//!    word SAMPLE is in the title, the cover, and the provenance), and the date is a
//!    FIXED string (never the system clock), so two runs produce the same artifact;
//!  * both faces carry the always-on MODELLED banner, the literal "not a
//!    certification" disclaimer, and cite arXiv:2606.22054 as an arXiv **preprint**
//!    (not peer-reviewed).
//!
//! The HTML face comes from [`kshana_pro::report_pack_a::PackAReport::to_html`]; the
//! PDF face from the pure-Rust [`kshana_pro::report_pdf::to_pdf_pack_a`].

use std::path::Path;

use kshana_pro::packs::optimism_gap_audit::OptimismGapAuditScenario;
use kshana_pro::report::{StudyCover, StudyProvenance};
use kshana_pro::report_pack_a::PackAReport;
use kshana_pro::report_pdf::to_pdf_pack_a;

// A FIXED preparation date — NOT read from the system clock — so the sample is
// byte-stable across runs.
const SAMPLE_PREPARED_UTC: &str = "2026-06-23T00:00:00Z";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── 1. Compute the gap numbers from the code (open engine via Pack A) ────
    //
    // A FIXED, seeded sample grid: smaller than the published arXiv:2606.22054
    // baseline (13 detectors × 4 classes × {0.2,0.4,0.6,0.8} × 5 seeds, 400
    // cases/class) so the example runs in seconds, but it exercises the full
    // pipeline (corpus → split → AUC/gap → ID-only predictor + leave-one-out CV +
    // permutation null). It is deterministic: the open engine reads no clock and
    // every draw is seeded, so the artifact is reproducible. The report labels it a
    // SAMPLE so it is never mistaken for the published headline or a real delivery.
    let scenario = OptimismGapAuditScenario {
        n_per_class: 200,
        frac_train: 0.7,
        severities: vec![0.2, 0.4, 0.6, 0.8],
        seeds: vec![1, 2, 3],
        target_pfa: 0.05,
        ridge_lambda: 0.1,
        n_perms: 200,
        perm_seed: 7,
    };
    let audit = scenario.compute()?;

    // ── 2. Build the Pack-A report with a FICTITIOUS sample cover + provenance ─
    let title = "Pack A — Optimism-Gap Audit (SAMPLE)";
    let summary = format!(
        "SAMPLE report — illustrative, generated from the open Kshana optimism-gap engine on a \
         seeded sample grid (smaller than the published arXiv:2606.22054 baseline), with NO \
         customer data. It reports the predicted field-vs-bench degradation (the optimism gap) \
         for the demonstrative fused baseline: the in-distribution (bench) AUC of {id:.3} \
         over-states the out-of-distribution (field) AUC of {ood:.3} by an optimism gap of \
         {gap:.3} of the AUC range. Numbers are MODELLED over a synthetic, parameter-grounded \
         corpus (never field/IQ) and are an engineering study, not a certification. The \
         phenomenon and the in-distribution-only gap predictor are established in \
         arXiv:2606.22054, an arXiv preprint (not peer-reviewed).",
        id = audit.auc_id_mean,
        ood = audit.auc_ood_mean,
        gap = audit.optimism_gap_mean,
    );
    // A content-derived sample study id (the rounded gap × 1000) — deterministic and
    // obviously a sample, never a real internal study number.
    let study_id = format!(
        "KP-SAMPLE-A-{:03}",
        (audit.optimism_gap_mean * 1000.0).round() as i64
    );

    let report = PackAReport::from_audit(title, &audit)
        .with_cover(StudyCover {
            subtitle: Some("SAMPLE — Optimism-Gap Audit · Field-vs-Bench Study".into()),
            executive_summary: Some(summary),
        })
        .with_provenance(StudyProvenance {
            study_id: Some(study_id),
            customer: Some("Sample — Detector R&D (illustrative)".into()),
            prepared_for: Some("Sample — Detector R&D (illustrative)".into()),
            prepared_utc: Some(SAMPLE_PREPARED_UTC.into()),
            engine_version: None,
            scenario_hashes: vec![
                "sample-grid: n_per_class=200, frac_train=0.7, severities=[0.2,0.4,0.6,0.8], \
                 seeds=[1,2,3], ridge_lambda=0.1, n_perms=200, perm_seed=7"
                    .into(),
            ],
            // ASCII separator (not an em-dash): this value is the PDF footer's
            // classification, and printpdf hex-encodes any literal containing a
            // non-WinAnsi char like an em-dash, which would hide the footer's
            // honesty phrase from the content stream. Keep it plain ASCII so
            // "MODELLED engineering study - not a certification" stays verbatim.
            classification: Some("Sample: illustrative, not a customer deliverable".into()),
        });

    // ── 3. Render HTML → docs/packs/sample-optimism-gap-study.html ───────────
    let out_dir = Path::new("docs/packs");
    let html_path = out_dir.join("sample-optimism-gap-study.html");
    let html = report.to_html();
    std::fs::write(&html_path, html.as_bytes())?;

    // ── 4. Render PDF (pure-Rust path) → …/sample-optimism-gap-study.pdf ─────
    let pdf_path = out_dir.join("sample-optimism-gap-study.pdf");
    let pdf = to_pdf_pack_a(&report)?;
    std::fs::write(&pdf_path, &pdf)?;

    eprintln!(
        "[gen_sample_study] gap {:.3} = AUC_id {:.3} - AUC_ood {:.3} | LOO-detector R2 {:.3}, \
         LOO-class R2 {:.3} | {} predictor rows",
        audit.optimism_gap_mean,
        audit.auc_id_mean,
        audit.auc_ood_mean,
        audit.loo_detector_r2,
        audit.loo_class_r2,
        audit.n_rows,
    );
    eprintln!(
        "[gen_sample_study] wrote {} ({} bytes) and {} ({} bytes)",
        html_path.display(),
        html.len(),
        pdf_path.display(),
        pdf.len(),
    );
    Ok(())
}
