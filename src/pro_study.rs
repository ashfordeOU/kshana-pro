// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # ProStudy — multi-pack study orchestration
//!
//! Composes one or more pack report *sections* into a SINGLE branded study
//! document: ONE shared cover ([`crate::report_brand::cover_html`]), ONE provenance
//! block, the always-on MODELLED / "not a certification" banner
//! ([`crate::report_brand::modelled_banner_html`]), then each present pack's section
//! in order. With only a Pack-A section it renders the Pack-A content under a single
//! cover; with no sections it still produces a valid (empty-but-branded) document.
//!
//! ## Why a `Vec` of sections (not three hardcoded packs)
//! Packs are modelled as optional/extensible. A [`ProStudy`] carries a
//! `Vec<ProSection>`; Pack A is added today via [`ProStudy::with_pack_a`] (and a
//! pack's section HTML comes from [`crate::report_pack_a::PackAReport::section_fragment_html`]),
//! and Packs B/C are simply not added yet — no enum of exactly three kinds to churn.
//!
//! ## Honesty (THE MOAT — non-optional)
//! The MODELLED banner and the literal "not a certification" disclaimer render
//! EXACTLY ONCE for the whole study, at the top — never once per pack. Each pack's
//! section is embedded WITHOUT its own cover/banner (it contributes only its
//! content + its own honest furniture, e.g. the arXiv-preprint citation), so the
//! per-pack honesty (the preprint stays a preprint) survives while the document-
//! level banner is not duplicated.

use crate::report::{StudyCover, StudyProvenance, DEFAULT_CLASSIFICATION};
use crate::report_brand as brand;
use crate::report_pack_a::PackAReport;

/// The standing "scope" clause for the single, shared MODELLED banner. Generic
/// across packs (a study may compose packs with different corpora), it keeps the
/// honest framing without claiming any one pack's specifics.
const BANNER_DETAIL: &str =
    "simulation figures, not flight/in-orbit validated; each section states its own data provenance.";

/// One pack's contribution to a composed study: a human pack label plus that pack's
/// already-rendered section HTML fragment (its content WITHOUT a cover or banner).
///
/// Pack A produces its fragment via
/// [`crate::report_pack_a::PackAReport::section_fragment_html`]; future packs (B, C,
/// …) are added the same way once they exist.
#[derive(Debug, Clone)]
pub struct ProSection {
    /// A short pack label shown above the section (e.g. `"Pack A"`).
    pub label: String,
    /// The pack's rendered section HTML (no document wrapper, cover, or banner).
    pub html: String,
}

impl ProSection {
    /// Build a section from a pack label and its rendered section-fragment HTML.
    pub fn new(label: impl Into<String>, html: String) -> Self {
        Self {
            label: label.into(),
            html,
        }
    }
}

/// A multi-pack branded study: ONE cover + provenance + MODELLED banner, then each
/// present pack's section in order.
#[derive(Debug, Clone)]
pub struct ProStudy {
    /// The study title (rendered in the single shared cover).
    pub title: String,
    /// Optional study-level cover copy (sub-title + executive summary).
    pub cover: Option<StudyCover>,
    /// Optional study-level audit provenance block.
    pub provenance: Option<StudyProvenance>,
    /// The composed pack sections, rendered in order under the single cover.
    pub sections: Vec<ProSection>,
}

impl ProStudy {
    /// A new, empty study with the given title (no sections yet — a valid,
    /// empty-but-branded document).
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            cover: None,
            provenance: None,
            sections: Vec::new(),
        }
    }

    /// Attach study-level branded cover copy (sub-title + executive summary).
    pub fn with_cover(mut self, cover: StudyCover) -> Self {
        self.cover = Some(cover);
        self
    }

    /// Attach a study-level audit-grade provenance block.
    pub fn with_provenance(mut self, prov: StudyProvenance) -> Self {
        self.provenance = Some(prov);
        self
    }

    /// Append an already-built [`ProSection`].
    pub fn with_section(mut self, section: ProSection) -> Self {
        self.sections.push(section);
        self
    }

    /// Convenience constructor: append a Pack-A section, taking its content from the
    /// Pack-A renderer ([`PackAReport::section_fragment_html`]) so the gap headline
    /// and the arXiv-preprint citation come from the existing Pack-A machinery
    /// (never duplicated here). The Pack-A report's own cover/banner are NOT used —
    /// the study's single shared cover/banner stand in.
    pub fn with_pack_a(mut self, label: impl Into<String>, report: &PackAReport) -> Self {
        self.sections
            .push(ProSection::new(label, report.section_fragment_html()));
        self
    }

    /// The resolved cover sub-title (caller-supplied or an honest default).
    fn subtitle(&self) -> String {
        self.cover
            .as_ref()
            .and_then(|c| c.subtitle.clone())
            .unwrap_or_else(|| "Multi-Pack Study".to_string())
    }

    /// The resolved executive summary (caller-supplied or an honest default that
    /// keeps the MODELLED framing and the "not a certification" scope).
    fn executive_summary(&self) -> String {
        self.cover
            .as_ref()
            .and_then(|c| c.executive_summary.clone())
            .unwrap_or_else(|| {
                let n = self.sections.len();
                let packs = if n == 1 { "pack" } else { "packs" };
                format!(
                    "This study composes {n} {packs} into a single branded document. \
                     Figures are MODELLED engineering results — each section states its own \
                     data provenance — and the document carries the standing MODELLED banner \
                     above. See each section for its specific scope and references."
                )
            })
    }

    /// The resolved handling classification (caller-supplied or the conservative
    /// [`DEFAULT_CLASSIFICATION`]).
    fn classification(&self) -> String {
        self.provenance
            .as_ref()
            .and_then(|p| p.classification.clone())
            .unwrap_or_else(|| DEFAULT_CLASSIFICATION.to_string())
    }

    /// The resolved engine-version string (caller-supplied or the live Pro version).
    fn engine_version(&self) -> String {
        self.provenance
            .as_ref()
            .and_then(|p| p.engine_version.clone())
            .unwrap_or_else(|| format!("kshana-pro {}", crate::version()))
    }

    /// The executive-summary block (heading + one paragraph).
    fn exec_summary_html(&self) -> String {
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
            summary = xml(&self.executive_summary()),
        )
    }

    /// The study-level provenance/traceability block (sensible defaults when none
    /// is attached) — always rendered so the composed face is traceable.
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
        let packs = self
            .sections
            .iter()
            .map(|s| s.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let packs = if packs.is_empty() {
            dash.clone()
        } else {
            xml(&packs)
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
                row("Engine version", xml(&self.engine_version())),
                row("Packs", packs),
                row("Scenario hash(es)", hashes),
                row("Classification", xml(&self.classification())),
            ]
            .concat(),
        )
    }

    /// One composed section: a pack label heading + the pack's section fragment.
    fn section_block_html(s: &ProSection) -> String {
        format!(
            "<section style=\"margin:30px 0 0\">\
             <div style=\"font-family:{body};font-size:12px;letter-spacing:.18em;\
             text-transform:uppercase;color:{brass};font-weight:700;border-top:1px solid {bm};\
             padding-top:14px\">{label}</div>{frag}</section>",
            body = brand::FONT_BODY,
            brass = brand::BRASS,
            bm = brand::BRASS_MUTED,
            label = xml(&s.label),
            frag = s.html,
        )
    }

    /// The self-contained, branded composed study HTML. No external fetches. The
    /// MODELLED banner and the "not a certification" disclaimer render EXACTLY ONCE,
    /// at the top, then each pack's section in order.
    pub fn to_html(&self) -> String {
        let sections = self
            .sections
            .iter()
            .map(Self::section_block_html)
            .collect::<String>();
        format!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>{title}</title></head>\
             <body style=\"background:{navy};color:{ivory};font-family:{body};\
             max-width:1000px;margin:auto;padding:24px 32px 48px\">\
             {cover}{banner}{summary}{provenance}{sections}\
             <footer style=\"text-align:center;font-family:{body};font-size:12px;color:{bm};\
             border-top:1px solid {bm};margin-top:24px;padding-top:14px\">\
             Ashforde O&Uuml; &middot; {classification} &middot; MODELLED engineering study &mdash; \
             not a certification.</footer>\
             </body></html>",
            title = xml(&self.title),
            navy = brand::NAVY,
            ivory = brand::IVORY,
            body = brand::FONT_BODY,
            bm = brand::BRASS_MUTED,
            cover = brand::cover_html(&self.title, &self.subtitle()),
            banner = brand::modelled_banner_html(BANNER_DETAIL),
            summary = self.exec_summary_html(),
            provenance = self.provenance_html(),
            sections = sections,
            classification = xml(&self.classification()),
        )
    }
}

/// Minimal XML/HTML escaping for text interpolated into the composed study.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::optimism_gap_audit::OptimismGapAuditScenario;

    fn fast_audit() -> crate::packs::optimism_gap_audit::OptimismGapAudit {
        OptimismGapAuditScenario {
            n_per_class: 80,
            frac_train: 0.7,
            severities: vec![0.3, 0.6, 0.9],
            seeds: vec![1, 2],
            target_pfa: 0.05,
            ridge_lambda: 0.1,
            n_perms: 20,
            perm_seed: 7,
        }
        .compute()
        .expect("audit computes")
    }

    fn count(hay: &str, needle: &str) -> usize {
        hay.matches(needle).count()
    }

    #[test]
    fn footer_uses_default_classification_when_none() {
        let html = ProStudy::new("S").to_html();
        assert!(html.contains("Unclassified"));
    }

    #[test]
    fn single_pack_a_banner_appears_exactly_once() {
        let a = fast_audit();
        let pack = PackAReport::from_audit("Pack A", &a);
        let html = ProStudy::new("S").with_pack_a("Pack A", &pack).to_html();
        // Exactly one MODELLED banner + one "not a certification" in the body.
        // (The footer carries the standing tagline too, so we count the banner's
        // own unique marker.)
        assert_eq!(count(&html, "MODELLED RESULTS &mdash;"), 1);
        // The Pack-A citation (preprint) survives composition.
        assert!(html.contains("arXiv:2606.22054"));
        assert!(html.contains("arXiv preprint"));
    }

    #[test]
    fn empty_study_is_valid_and_branded() {
        let html = ProStudy::new("Empty").to_html();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.ends_with("</html>"));
        assert!(html.contains("Ashforde"));
        assert_eq!(count(&html, "MODELLED RESULTS &mdash;"), 1);
    }
}
