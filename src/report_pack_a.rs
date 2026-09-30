// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Pack-A study report — the field-vs-bench (optimism-gap) study section
//!
//! Renders the buyable Pack-A study as a branded, self-contained HTML face that
//! reuses the same brand furniture ([`crate::report_brand`]) and the same cover /
//! provenance copy types ([`crate::report::StudyCover`] /
//! [`crate::report::StudyProvenance`]) as the consolidated study report, so a Pack-A
//! deliverable looks identical in hand to the rest of the study line.
//!
//! The headline is the **predicted field-vs-bench degradation** = the optimism gap:
//! ID AUC, OOD AUC, the gap, and (when available) a per-class table and the ID-only
//! predictor result. The renderer accepts the data from either source:
//!
//!  * the published-baseline pack ([`crate::packs::optimism_gap_audit::OptimismGapAudit`]), or
//!  * a customer intake ([`crate::packs::detector_intake::DetectorGap`]).
//!
//! ## Honesty (THE MOAT — non-optional)
//! The MODELLED banner and the "not a certification" disclaimer are ALWAYS present,
//! and the proof is cited as **arXiv:2606.22054, an arXiv preprint (not
//! peer-reviewed)** — never as a certification, peer-reviewed result, or
//! ESA/TRL claim.

use crate::packs::detector_intake::DetectorGap;
use crate::packs::optimism_gap_audit::{OptimismGapAudit, PerClassGap, ARXIV_PREPRINT_ID};
use crate::report::{StudyCover, StudyProvenance};
use crate::report_brand as brand;

/// The headline + supporting numbers of a Pack-A study, abstracted over the data
/// source (baseline pack or customer intake). The renderer only ever sees this.
#[derive(Debug, Clone)]
pub struct PackASection {
    /// Detector / source label shown in the headline (e.g. the baseline detector, or
    /// the customer name).
    pub source_label: String,
    /// In-distribution (bench) AUC — the headline ID number.
    pub auc_id: f64,
    /// Out-of-distribution (field) AUC — the headline OOD number.
    pub auc_ood: f64,
    /// The optimism gap = `auc_id − auc_ood` (the predicted field-vs-bench degradation).
    pub optimism_gap: f64,
    /// Optional per-class ID/OOD/gap rows (present for the baseline pack).
    pub per_class: Vec<PerClassGap>,
    /// Optional ID-only predictor headline `(loo_detector_r2, loo_class_r2)` (baseline pack).
    pub predictor_r2: Option<(f64, f64)>,
    /// Optional sample counts `(n_id, n_ood)` (present for a customer intake).
    pub sample_counts: Option<(usize, usize)>,
}

impl PackASection {
    /// Build the section from the published-baseline audit (A1).
    pub fn from_audit(a: &OptimismGapAudit) -> Self {
        Self {
            source_label: "fused(max-z) baseline (demonstrative)".into(),
            auc_id: a.auc_id_mean,
            auc_ood: a.auc_ood_mean,
            optimism_gap: a.optimism_gap_mean,
            per_class: a.per_class.clone(),
            predictor_r2: Some((a.loo_detector_r2, a.loo_class_r2)),
            sample_counts: None,
        }
    }

    /// Build the section from a customer-detector intake gap (A2).
    pub fn from_customer_gap(customer_label: &str, g: &DetectorGap) -> Self {
        Self {
            source_label: customer_label.to_string(),
            auc_id: g.auc_id,
            auc_ood: g.auc_ood,
            optimism_gap: g.optimism_gap,
            per_class: Vec::new(),
            predictor_r2: None,
            sample_counts: Some((g.n_id, g.n_ood)),
        }
    }
}

/// A full Pack-A study report face: cover, MODELLED banner, the field-vs-bench
/// headline section, provenance, and the preprint citation.
#[derive(Debug, Clone)]
pub struct PackAReport {
    /// Study title (rendered in the cover).
    pub title: String,
    /// The field-vs-bench section data.
    pub section: PackASection,
    /// Optional branded cover copy (sub-title + executive summary).
    pub cover: Option<StudyCover>,
    /// Optional audit-grade provenance block.
    pub provenance: Option<StudyProvenance>,
}

impl PackAReport {
    /// A Pack-A report over the published-baseline audit (A1).
    pub fn from_audit(title: &str, a: &OptimismGapAudit) -> Self {
        Self {
            title: title.to_string(),
            section: PackASection::from_audit(a),
            cover: None,
            provenance: None,
        }
    }

    /// A Pack-A report over a customer-detector intake gap (A2).
    pub fn from_customer_gap(title: &str, customer_label: &str, g: &DetectorGap) -> Self {
        Self {
            title: title.to_string(),
            section: PackASection::from_customer_gap(customer_label, g),
            cover: None,
            provenance: None,
        }
    }

    /// Attach branded cover copy.
    pub fn with_cover(mut self, cover: StudyCover) -> Self {
        self.cover = Some(cover);
        self
    }

    /// Attach an audit-grade provenance block.
    pub fn with_provenance(mut self, prov: StudyProvenance) -> Self {
        self.provenance = Some(prov);
        self
    }

    fn engine_version(&self) -> String {
        format!(
            "kshana-pro {} (embeds the open kshana optimism-gap engine)",
            crate::version(),
        )
    }

    fn classification(&self) -> String {
        self.provenance
            .as_ref()
            .and_then(|p| p.classification.clone())
            .unwrap_or_else(|| crate::report::DEFAULT_CLASSIFICATION.to_string())
    }

    fn cover_html(&self) -> String {
        let subtitle = self
            .cover
            .as_ref()
            .and_then(|c| c.subtitle.clone())
            .unwrap_or_else(|| "Optimism-Gap Audit — Field-vs-Bench Study".to_string());
        format!(
            "<header style=\"text-align:center;border-bottom:2px solid {brass};\
             padding:36px 0 28px;margin-bottom:8px\">\
             <div style=\"display:flex;justify-content:center;margin-bottom:14px\">{seal}</div>\
             <div style=\"font-family:{head};font-size:15px;letter-spacing:.42em;\
             text-transform:uppercase;color:{brass};font-weight:700\">Ashforde</div>\
             <div style=\"font-family:{body};font-size:11px;letter-spacing:.22em;\
             text-transform:uppercase;color:{brass_muted};margin-top:2px\">\
             Ashforde O&Uuml; &middot; Quantitative PNT Resilience</div>\
             <h1 style=\"font-family:{head};font-size:34px;font-weight:700;color:{ivory_light};\
             margin:22px 0 6px;line-height:1.15\">{title}</h1>\
             <div style=\"font-family:{body};font-size:16px;color:{ivory};font-style:italic\">{subtitle}</div>\
             </header>",
            brass = brand::BRASS,
            brass_muted = brand::BRASS_MUTED,
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            ivory = brand::IVORY,
            ivory_light = brand::IVORY_LIGHT,
            seal = brand::seal_img_html(96),
            title = xml(&self.title),
            subtitle = xml(&subtitle),
        )
    }

    /// The NON-OPTIONAL MODELLED banner with the "not a certification" disclaimer.
    fn modelled_banner_html(&self) -> String {
        format!(
            "<div style=\"background:{brass};color:{ink};font-family:{body};font-weight:700;\
             text-align:center;padding:11px 16px;border-radius:4px;margin:18px 0;\
             letter-spacing:.04em\">\
             MODELLED RESULTS &mdash; simulation figures over a synthetic, parameter-grounded \
             corpus (never field/IQ). This is an engineering study, <strong>not a \
             certification</strong> and not ESA acceptance or TRL maturation.</div>",
            brass = brand::BRASS,
            ink = brand::INK,
            body = brand::FONT_BODY,
        )
    }

    /// The executive-summary block (heading + one paragraph).
    fn exec_summary_html(&self) -> String {
        let summary = self
            .cover
            .as_ref()
            .and_then(|c| c.executive_summary.clone())
            .unwrap_or_else(|| {
                format!(
                    "This Pack-A study reports the predicted field-vs-bench degradation \
                     (the optimism gap) for {src}: the in-distribution (bench) AUC over-states \
                     the out-of-distribution (field) AUC by {gap:.3} of the AUC range. Numbers \
                     are MODELLED over a synthetic, parameter-grounded corpus and are an \
                     engineering study, not a certification. The phenomenon and the in-distribution-only \
                     predictor are established in {arxiv}, an arXiv preprint (not peer-reviewed).",
                    src = xml(&self.section.source_label),
                    gap = self.section.optimism_gap,
                    arxiv = ARXIV_PREPRINT_ID,
                )
            });
        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:22px;font-weight:700;\
             border-bottom:1px solid {brass_muted};padding-bottom:6px\">Executive Summary</h2>\
             <p style=\"font-family:{body};font-size:16px;line-height:1.6;color:{ivory}\">{summary}</p>\
             </section>",
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            brass_muted = brand::BRASS_MUTED,
            ivory = brand::IVORY,
            summary = xml(&summary),
        )
    }

    /// The headline field-vs-bench section: the big gap number, the ID/OOD pair, an
    /// optional per-class table, the optional predictor result, and sample counts.
    fn section_html(&self) -> String {
        let s = &self.section;
        // Headline numbers.
        let headline = format!(
            "<div style=\"display:flex;flex-wrap:wrap;gap:18px;margin:14px 0\">\
             {gap}{idn}{oodn}</div>",
            gap = stat_card(
                "Predicted field-vs-bench degradation (optimism gap)",
                &format!("{:.3}", s.optimism_gap),
                "AUC_id &minus; AUC_ood",
            ),
            idn = stat_card(
                "In-distribution AUC (bench)",
                &format!("{:.3}", s.auc_id),
                "tuned / nominal regime"
            ),
            oodn = stat_card(
                "Out-of-distribution AUC (field)",
                &format!("{:.3}", s.auc_ood),
                "shifted / subtler regime"
            ),
        );

        // Per-class table (baseline pack only).
        let per_class = if s.per_class.is_empty() {
            String::new()
        } else {
            let mut rows = String::new();
            for p in &s.per_class {
                rows.push_str(&format!(
                    "<tr style=\"border-top:1px solid {bm}\">\
                     <td style=\"padding:7px 12px 7px 0;color:{iv}\">{cls}</td>\
                     <td style=\"padding:7px 12px;color:{ivl};font-weight:700;white-space:nowrap\">{id:.3}</td>\
                     <td style=\"padding:7px 12px;color:{ivl};font-weight:700;white-space:nowrap\">{ood:.3}</td>\
                     <td style=\"padding:7px 0;color:{iv}\">{gap:.3}</td></tr>",
                    bm = brand::BRASS_MUTED,
                    iv = brand::IVORY,
                    ivl = brand::IVORY_LIGHT,
                    cls = xml(&p.class),
                    id = p.auc_id,
                    ood = p.auc_ood,
                    gap = p.optimism_gap,
                ));
            }
            format!(
                "<h3 style=\"font-family:{head};color:{brass};font-size:17px;margin:18px 0 4px\">\
                 Per-class field-vs-bench AUC</h3>\
                 <table style=\"border-collapse:collapse;width:100%;font-family:{body};font-size:15px\">\
                 <thead><tr style=\"text-align:left;color:{brass}\">\
                 <th style=\"padding:0 12px 6px 0\">Impairment class</th>\
                 <th style=\"padding:0 12px 6px\">AUC (bench)</th>\
                 <th style=\"padding:0 12px 6px\">AUC (field)</th>\
                 <th style=\"padding:0 0 6px\">Gap</th></tr></thead><tbody>{rows}</tbody></table>",
                head = brand::FONT_HEAD,
                body = brand::FONT_BODY,
                brass = brand::BRASS,
            )
        };

        // Predictor headline (baseline pack only).
        let predictor = match s.predictor_r2 {
            Some((det, cls)) => format!(
                "<p style=\"font-family:{body};font-size:15px;color:{iv};margin:14px 0 0\">\
                 <strong>ID-only gap predictor:</strong> in-distribution diagnostics predict the \
                 optimism gap for a detector held entirely out of training \
                 (leave-one-detector-out R&sup2; = {det:.3}) and for an impairment class held out \
                 (leave-one-class-out R&sup2; = {cls:.3}); an R&sup2; above zero beats \
                 predict-the-mean.</p>",
                body = brand::FONT_BODY,
                iv = brand::IVORY,
            ),
            None => String::new(),
        };

        // Sample counts (customer intake only).
        let counts = match s.sample_counts {
            Some((n_id, n_ood)) => format!(
                "<p style=\"font-family:{body};font-size:14px;color:{iv};opacity:.85;margin:10px 0 0\">\
                 Computed over the customer submission: {n_id} in-distribution and {n_ood} \
                 out-of-distribution scored samples.</p>",
                body = brand::FONT_BODY,
                iv = brand::IVORY,
            ),
            None => String::new(),
        };

        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:22px;font-weight:700;\
             border-bottom:1px solid {brass_muted};padding-bottom:6px\">\
             Field-vs-Bench Degradation &mdash; {src}</h2>\
             {headline}{per_class}{predictor}{counts}</section>",
            head = brand::FONT_HEAD,
            brass = brand::BRASS,
            brass_muted = brand::BRASS_MUTED,
            src = xml(&self.section.source_label),
        )
    }

    /// The provenance block (sensible defaults when none attached).
    fn provenance_html(&self) -> String {
        let p = self.provenance.clone().unwrap_or_default();
        let dash = "&mdash;".to_string();
        let cell = |v: Option<String>| {
            v.filter(|s| !s.is_empty())
                .map(|s| xml(&s))
                .unwrap_or_else(|| dash.clone())
        };
        let row = |label: &str, value: String| {
            format!(
                "<tr><th style=\"text-align:left;font-family:{body};font-weight:700;color:{brass};\
                 padding:4px 14px 4px 0;white-space:nowrap;vertical-align:top\">{label}</th>\
                 <td style=\"font-family:{body};color:{iv};padding:4px 0\">{value}</td></tr>",
                body = brand::FONT_BODY,
                brass = brand::BRASS,
                iv = brand::IVORY,
            )
        };
        let hashes = if p.scenario_hashes.is_empty() {
            dash.clone()
        } else {
            xml(&p.scenario_hashes.join(", "))
        };
        format!(
            "<section style=\"margin:22px 0;background:{panel};border:1px solid {bm};\
             border-radius:4px;padding:16px 20px\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:20px;font-weight:700;margin-top:0\">\
             Provenance &amp; Traceability</h2>\
             <table style=\"border-collapse:collapse;font-size:14px\">{rows}</table></section>",
            panel = brand::NAVY_PANEL,
            head = brand::FONT_HEAD,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
            rows = [
                row("Study ID", cell(p.study_id)),
                row("Customer", cell(p.customer)),
                row("Prepared for", cell(p.prepared_for)),
                row("Prepared (UTC)", cell(p.prepared_utc)),
                row(
                    "Engine version",
                    xml(&p.engine_version.unwrap_or_else(|| self.engine_version())),
                ),
                row("Scenario hash(es)", hashes),
                row("Proof", xml(&format!("{ARXIV_PREPRINT_ID} (arXiv preprint — not peer-reviewed)"))),
                row("Classification", xml(&self.classification())),
            ]
            .concat(),
        )
    }

    /// The preprint-citation footer block (always present, always accurate).
    fn citation_html(&self) -> String {
        format!(
            "<section style=\"margin:22px 0\">\
             <h2 style=\"font-family:{head};color:{brass};font-size:18px;font-weight:700;\
             border-bottom:1px solid {bm};padding-bottom:6px\">Reference</h2>\
             <p style=\"font-family:{body};font-size:14px;color:{iv};line-height:1.55\">\
             The optimism-gap phenomenon and the in-distribution-only gap predictor used in this \
             study are established in <strong>{arxiv}</strong>, an <strong>arXiv preprint</strong> \
             &mdash; a preprint, <strong>not peer-reviewed</strong> and not a certification.</p>\
             </section>",
            head = brand::FONT_HEAD,
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
            iv = brand::IVORY,
            arxiv = ARXIV_PREPRINT_ID,
        )
    }

    /// The Pack-A *section* fragment — the executive summary, the field-vs-bench
    /// headline section (the optimism-gap number + ID/OOD AUCs + optional per-class
    /// table/predictor/counts), the provenance block, and the preprint citation —
    /// WITHOUT the document wrapper, the cover, or the MODELLED banner.
    ///
    /// This is what a multi-pack composer ([`crate::pro_study::ProStudy`]) embeds so
    /// the cover and the MODELLED/"not a certification" banner render exactly ONCE
    /// for the whole study, never once per pack. The standalone [`Self::to_html`]
    /// face is unchanged and still carries its own cover + banner.
    pub fn section_fragment_html(&self) -> String {
        format!(
            "{summary}{section}{provenance}{citation}",
            summary = self.exec_summary_html(),
            section = self.section_html(),
            provenance = self.provenance_html(),
            citation = self.citation_html(),
        )
    }

    /// The self-contained, branded Pack-A HTML study. No external fetches. The
    /// MODELLED banner, the "not a certification" disclaimer, and the preprint
    /// citation are always present.
    pub fn to_html(&self) -> String {
        // Every Kshana Pro document is an Ashforde document: the shared furniture of
        // crate::report_brand goes around the renderer's own page, which is unchanged.
        let body = self.to_html_unbranded();
        let id = crate::report_brand::content_id(&body);
        crate::report_brand::finish_html(
            &body,
            &crate::report_brand::DocMark::new("Optimism-gap study", id),
        )
    }

    fn to_html_unbranded(&self) -> String {
        format!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>{title}</title></head>\
             <body style=\"background:{navy};color:{ivory};font-family:{body};\
             max-width:1000px;margin:auto;padding:24px 32px 48px\">\
             {cover}{banner}{summary}{section}{provenance}{citation}\
             <footer style=\"text-align:center;font-family:{body};font-size:12px;color:{bm};\
             border-top:1px solid {bm};margin-top:24px;padding-top:14px\">\
             Ashforde O&Uuml; &middot; {classification} &middot; MODELLED engineering study &mdash; \
             not a certification. Proof: {arxiv} (arXiv preprint).</footer>\
             </body></html>",
            title = xml(&self.title),
            navy = brand::NAVY,
            ivory = brand::IVORY,
            body = brand::FONT_BODY,
            bm = brand::BRASS_MUTED,
            cover = self.cover_html(),
            banner = self.modelled_banner_html(),
            summary = self.exec_summary_html(),
            section = self.section_html(),
            provenance = self.provenance_html(),
            citation = self.citation_html(),
            classification = xml(&self.classification()),
            arxiv = ARXIV_PREPRINT_ID,
        )
    }
}

/// A small brass-bordered stat card (big value + label + caption).
fn stat_card(label: &str, value: &str, caption: &str) -> String {
    format!(
        "<div style=\"flex:1;min-width:180px;background:{panel};border:1px solid {bm};\
         border-radius:4px;padding:14px 16px\">\
         <div style=\"font-family:{body};font-size:12px;letter-spacing:.04em;color:{brass};\
         text-transform:uppercase\">{label}</div>\
         <div style=\"font-family:{head};font-size:30px;font-weight:700;color:{ivl};margin:4px 0\">{value}</div>\
         <div style=\"font-family:{body};font-size:12px;color:{iv};opacity:.8\">{caption}</div></div>",
        panel = brand::NAVY_PANEL,
        bm = brand::BRASS_MUTED,
        body = brand::FONT_BODY,
        head = brand::FONT_HEAD,
        brass = brand::BRASS,
        ivl = brand::IVORY_LIGHT,
        iv = brand::IVORY,
        label = xml(label),
        value = value,
        caption = caption,
    )
}

/// Minimal XML/HTML escaping for text interpolated into the report.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::detector_intake::{gap_from_samples, Distribution, ScoredSample};
    use crate::packs::optimism_gap_audit::OptimismGapAuditScenario;

    fn fast_audit() -> OptimismGapAudit {
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

    /// The MODELLED/"not a certification" banner and the preprint citation are always
    /// present, and the gap headline + AUC numbers render — from the baseline pack.
    #[test]
    fn baseline_report_has_headline_aucs_banner_and_preprint_citation() {
        let a = fast_audit();
        let html = PackAReport::from_audit("Pack A — Optimism-Gap Audit", &a).to_html();
        // Honesty (THE MOAT).
        assert!(html.contains("MODELLED"));
        assert!(html.contains("not a certification"));
        assert!(html.contains("arXiv:2606.22054"));
        assert!(html.contains("arXiv preprint"));
        assert!(html.contains("not peer-reviewed"));
        // The gap headline + the ID/OOD AUC numbers.
        assert!(html.contains("optimism gap"));
        assert!(html.contains(&format!("{:.3}", a.optimism_gap_mean)));
        assert!(html.contains(&format!("{:.3}", a.auc_id_mean)));
        assert!(html.contains(&format!("{:.3}", a.auc_ood_mean)));
        // Per-class table is present for the baseline pack.
        assert!(html.contains("Per-class field-vs-bench AUC"));
        assert!(html.contains("jamming"));
        // The ID-only predictor headline is present.
        assert!(html.contains("ID-only gap predictor"));
        assert!(html.contains("leave-one-detector-out"));
        // No overclaim language.
        assert!(!html.to_lowercase().contains("peer-reviewed result"));
    }

    /// The same renderer works for a CUSTOMER intake (A2) — no per-class table, but
    /// the headline, banner, and citation are all present.
    #[test]
    fn customer_intake_report_renders_with_banner_and_citation() {
        let samples = vec![
            ScoredSample {
                score: 9.0,
                label: true,
                distribution: Distribution::Id,
            },
            ScoredSample {
                score: 8.0,
                label: true,
                distribution: Distribution::Id,
            },
            ScoredSample {
                score: 2.0,
                label: false,
                distribution: Distribution::Id,
            },
            ScoredSample {
                score: 1.0,
                label: false,
                distribution: Distribution::Id,
            },
            ScoredSample {
                score: 7.0,
                label: true,
                distribution: Distribution::Ood,
            },
            ScoredSample {
                score: 3.0,
                label: true,
                distribution: Distribution::Ood,
            },
            ScoredSample {
                score: 4.0,
                label: false,
                distribution: Distribution::Ood,
            },
            ScoredSample {
                score: 2.0,
                label: false,
                distribution: Distribution::Ood,
            },
        ];
        let g = gap_from_samples(&samples).unwrap();
        let html = PackAReport::from_customer_gap(
            "Pack A — Customer Optimism-Gap Audit",
            "ACME Detector",
            &g,
        )
        .to_html();
        assert!(html.contains("MODELLED"));
        assert!(html.contains("not a certification"));
        assert!(html.contains("arXiv:2606.22054"));
        assert!(html.contains("arXiv preprint"));
        // Headline AUCs from the customer gap.
        assert!(html.contains(&format!("{:.3}", g.optimism_gap)));
        assert!(html.contains(&format!("{:.3}", g.auc_id)));
        assert!(html.contains(&format!("{:.3}", g.auc_ood)));
        // Customer source label + sample counts shown; no per-class table.
        assert!(html.contains("ACME Detector"));
        assert!(html.contains("in-distribution and"));
        assert!(!html.contains("Per-class field-vs-bench AUC"));
    }

    /// Cover + provenance compose like the rest of the study line.
    #[test]
    fn cover_and_provenance_compose() {
        let a = fast_audit();
        let html = PackAReport::from_audit("Pack A", &a)
            .with_cover(StudyCover {
                subtitle: Some("Buyable study".into()),
                executive_summary: Some("Custom summary for the board.".into()),
            })
            .with_provenance(StudyProvenance {
                study_id: Some("KP-2026-PACKA".into()),
                customer: Some("ACME".into()),
                classification: Some("Commercial-in-Confidence".into()),
                ..Default::default()
            })
            .to_html();
        assert!(html.contains("Buyable study"));
        assert!(html.contains("Custom summary for the board."));
        assert!(html.contains("KP-2026-PACKA"));
        assert!(html.contains("Commercial-in-Confidence"));
        // Banner + citation still non-optional even with custom copy.
        assert!(html.contains("not a certification"));
        assert!(html.contains("arXiv preprint"));
    }
}
