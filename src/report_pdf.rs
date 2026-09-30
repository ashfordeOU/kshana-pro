// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Pure-Rust PDF export of the branded study report
//!
//! A self-contained PDF renderer for [`crate::report::ConsolidatedReport`]. It
//! builds the document **from the report's structured data** — the same fields the
//! HTML face reads ([`crate::report`]) — never by parsing the rendered HTML. There
//! is **no external tool** (no headless browser, no `wkhtmltopdf`) and **no bundled
//! font binary**: it uses [`printpdf`]'s built-in standard-14 fonts (Times for the
//! serif headings, Helvetica for body/labels) so the deliverable is fully
//! pure-Rust and portable.
//!
//! ## Honesty parity with the HTML face
//! The PDF carries the same non-negotiable furniture: the branded cover (Ashforde
//! identity + study title), the executive summary, the **MODELLED** banner with the
//! literal "not a certification" disclaimer, the figure-of-merit table with each
//! FoM's inline validation tier from the open core's single source of truth
//! ([`kshana::fom_label::tier_for`], via [`crate::report`]'s `fom_tier_tag`), the
//! provenance/traceability block, and the caveats. No new claims are introduced.
//!
//! ## Determinism
//! The library never reads a clock. printpdf's `PdfDocumentInfo::default()` pins
//! `CreationDate` / `ModDate` / the metadata date to the Unix epoch, so those are
//! stable. The **one** field we cannot control is the PDF trailer `/ID` array:
//! `printpdf 0.9.1` fills it with two random 32-character strings inside
//! `serialize_pdf` (`serialize.rs:386-387`) with no override hook. Two renders of
//! the same report are therefore byte-identical **except** for that `/ID` array —
//! see `tests/report_pdf.rs`, which masks exactly that field and asserts the rest
//! is identical. We do not read the system clock anywhere in this module.

use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PaintMode, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Polygon, PolygonRing, Pt, Rgb, TextItem, WindingOrder,
};

use crate::packs::optimism_gap_audit::ARXIV_PREPRINT_ID;
use crate::report::{ConsolidatedReport, DEFAULT_CLASSIFICATION};
use crate::report_brand as brand;
use crate::report_pack_a::PackAReport;

// --- A4 page geometry (mm) ---------------------------------------------------
pub(crate) const PAGE_W_MM: f32 = 210.0;
pub(crate) const PAGE_H_MM: f32 = 297.0;
pub(crate) const MARGIN_MM: f32 = 20.0;
pub(crate) const CONTENT_W_MM: f32 = PAGE_W_MM - 2.0 * MARGIN_MM;
pub(crate) const TOP_Y_MM: f32 = PAGE_H_MM - MARGIN_MM;
pub(crate) const BOTTOM_Y_MM: f32 = MARGIN_MM + 6.0; // leave room above the footer

// 1 mm = 72/25.4 pt.
pub(crate) const MM_PER_PT: f32 = 25.4 / 72.0;

/// A brand colour as a printpdf RGB fill/stroke colour (parses `#rrggbb`).
pub(crate) fn rgb(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    let parse =
        |i: usize| -> f32 { u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f32 / 255.0 };
    Color::Rgb(Rgb {
        r: parse(0),
        g: parse(2),
        b: parse(4),
        icc_profile: None,
    })
}

/// Width, in mm, of `n` characters at point size `pt` for a Helvetica/Times-like
/// face. The standard-14 metrics are not exposed by printpdf for built-in fonts,
/// so we use a conservative average advance (~0.52 em) to wrap text. This keeps
/// layout deterministic without any font-file or glyph-metric dependency.
pub(crate) fn text_width_mm(n: usize, pt: f32) -> f32 {
    let em_mm = pt * MM_PER_PT;
    n as f32 * em_mm * 0.52
}

/// Greedy word-wrap to a column `width_mm` wide at point size `pt`.
pub(crate) fn wrap(text: &str, width_mm: f32, pt: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let candidate = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if text_width_mm(candidate.chars().count(), pt) <= width_mm || cur.is_empty() {
            cur = candidate;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// A serif heading vs sans body distinction, mapped to built-in standard-14 fonts.
pub(crate) fn serif() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesRoman)
}
pub(crate) fn serif_bold() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesBold)
}
pub(crate) fn sans() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::Helvetica)
}
pub(crate) fn sans_bold() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::HelveticaBold)
}

/// Incremental, page-aware layout builder. Holds the current page's ops and the
/// cursor (in mm from the page bottom-left), flushing to a new page when the
/// cursor would cross the bottom margin.
pub(crate) struct Layout {
    pub(crate) pages: Vec<Vec<Op>>,
    pub(crate) ops: Vec<Op>,
    pub(crate) y_mm: f32,
}

impl Layout {
    pub(crate) fn new() -> Self {
        Layout {
            pages: Vec::new(),
            ops: Vec::new(),
            y_mm: TOP_Y_MM,
        }
    }

    /// Start a fresh page, resetting the cursor to the top margin.
    pub(crate) fn new_page(&mut self) {
        if !self.ops.is_empty() {
            let done = std::mem::take(&mut self.ops);
            self.pages.push(done);
        }
        self.y_mm = TOP_Y_MM;
    }

    /// Ensure `need_mm` of vertical space remains; break to a new page if not.
    pub(crate) fn ensure(&mut self, need_mm: f32) {
        if self.y_mm - need_mm < BOTTOM_Y_MM {
            self.new_page();
        }
    }

    /// Advance the cursor down by `mm`.
    pub(crate) fn advance(&mut self, mm: f32) {
        self.y_mm -= mm;
    }

    /// A filled rectangle in page coordinates (mm), e.g. the banner background.
    ///
    /// Drawn as a filled four-point polygon. `printpdf` 0.9.1 writes `Op::DrawRectangle`
    /// as a clipping path (`re W n`) whatever paint mode it is given, so a rectangle
    /// drawn that way painted nothing and clipped everything after it on the page to
    /// its own area.
    pub(crate) fn fill_rect(&mut self, x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32, col: Color) {
        let corner = |x: f32, y: f32| LinePoint {
            p: Point::new(Mm(x), Mm(y)),
            bezier: false,
        };
        self.ops.push(Op::SetFillColor { col });
        self.ops.push(Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing {
                    points: vec![
                        corner(x_mm, y_mm),
                        corner(x_mm + w_mm, y_mm),
                        corner(x_mm + w_mm, y_mm + h_mm),
                        corner(x_mm, y_mm + h_mm),
                    ],
                }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        });
    }

    /// A horizontal rule (brass accent) across the content column at the cursor.
    pub(crate) fn rule(&mut self, col: Color, thickness_pt: f32) {
        let y = self.y_mm;
        self.ops.push(Op::SetOutlineColor { col });
        self.ops.push(Op::SetOutlineThickness {
            pt: Pt(thickness_pt),
        });
        self.ops.push(Op::DrawLine {
            line: Line {
                points: vec![
                    LinePoint {
                        p: Point::new(Mm(MARGIN_MM), Mm(y)),
                        bezier: false,
                    },
                    LinePoint {
                        p: Point::new(Mm(PAGE_W_MM - MARGIN_MM), Mm(y)),
                        bezier: false,
                    },
                ],
                is_closed: false,
            },
        });
    }

    /// Draw a single line of text at `(x_mm, self.y_mm)` in the given font/size/colour.
    /// Does NOT advance the cursor (caller controls line spacing).
    pub(crate) fn text_at(
        &mut self,
        x_mm: f32,
        text: &str,
        font: PdfFontHandle,
        pt: f32,
        col: Color,
    ) {
        self.ops.push(Op::StartTextSection);
        self.ops.push(Op::SetTextCursor {
            pos: Point::new(Mm(x_mm), Mm(self.y_mm)),
        });
        self.ops.push(Op::SetFont { font, size: Pt(pt) });
        self.ops.push(Op::SetFillColor { col });
        self.ops.push(Op::ShowText {
            items: vec![TextItem::Text(text.to_string())],
        });
        self.ops.push(Op::EndTextSection);
    }

    /// A wrapped paragraph across the full content column (left margin to right
    /// margin), advancing the cursor and breaking pages as needed. `lh_mm` is the
    /// line height.
    pub(crate) fn paragraph(
        &mut self,
        text: &str,
        font: PdfFontHandle,
        pt: f32,
        lh_mm: f32,
        col: Color,
    ) {
        for line in wrap(text, CONTENT_W_MM, pt) {
            self.ensure(lh_mm);
            self.text_at(MARGIN_MM, &line, font.clone(), pt, col.clone());
            self.advance(lh_mm);
        }
    }

    /// A section heading (serif bold, brass) with a thin brass underline rule.
    pub(crate) fn heading(&mut self, title: &str) {
        self.ensure(14.0);
        self.advance(4.0);
        self.text_at(MARGIN_MM, title, serif_bold(), 15.0, rgb(brand::BRASS));
        self.advance(6.0);
        self.rule(rgb(brand::BRASS_MUTED), 0.6);
        self.advance(6.0);
    }

    /// Finalise: push the in-progress page and append a footer to every page.
    pub(crate) fn finish(mut self, footer: &str) -> Vec<PdfPage> {
        if !self.ops.is_empty() {
            let done = std::mem::take(&mut self.ops);
            self.pages.push(done);
        }
        if self.pages.is_empty() {
            self.pages.push(Vec::new());
        }
        let n = self.pages.len();
        self.pages
            .into_iter()
            .enumerate()
            .map(|(i, mut ops)| {
                // Footer rule + line at the very bottom of each page.
                let fy = MARGIN_MM - 4.0;
                ops.push(Op::SetOutlineColor {
                    col: rgb(brand::BRASS_MUTED),
                });
                ops.push(Op::SetOutlineThickness { pt: Pt(0.4) });
                ops.push(Op::DrawLine {
                    line: Line {
                        points: vec![
                            LinePoint {
                                p: Point::new(Mm(MARGIN_MM), Mm(fy + 4.0)),
                                bezier: false,
                            },
                            LinePoint {
                                p: Point::new(Mm(PAGE_W_MM - MARGIN_MM), Mm(fy + 4.0)),
                                bezier: false,
                            },
                        ],
                        is_closed: false,
                    },
                });
                // ASCII separator: the built-in fonts use WinAnsi encoding, and a middle
                // dot written as UTF-8 showed as two wrong characters in the footer.
                let label = format!("{footer}   |   page {} of {}", i + 1, n);
                ops.push(Op::StartTextSection);
                ops.push(Op::SetTextCursor {
                    pos: Point::new(Mm(MARGIN_MM), Mm(fy)),
                });
                ops.push(Op::SetFont {
                    font: sans(),
                    size: Pt(8.0),
                });
                ops.push(Op::SetFillColor {
                    col: rgb(brand::BRASS_MUTED),
                });
                ops.push(Op::ShowText {
                    items: vec![TextItem::Text(label)],
                });
                ops.push(Op::EndTextSection);
                PdfPage::new(Mm(PAGE_W_MM), Mm(PAGE_H_MM), ops)
            })
            .collect()
    }
}

/// Render the consolidated study report to a self-contained PDF (`%PDF-…`).
///
/// Reads the report's structured data directly (cover, executive summary, the
/// MODELLED + "not a certification" banner, the FoM table with inline tiers, and
/// the provenance block) — it never parses the HTML face. Returns the PDF bytes,
/// or an error string if serialization produced no output.
pub fn to_pdf(report: &ConsolidatedReport) -> Result<Vec<u8>, String> {
    let mut l = Layout::new();

    // --- Branded cover -------------------------------------------------------
    l.advance(6.0);
    l.text_at(MARGIN_MM, "ASHFORDE", sans_bold(), 13.0, rgb(brand::BRASS));
    l.advance(6.0);
    l.text_at(
        MARGIN_MM,
        "Ashforde OU  |  Quantitative PNT Resilience",
        sans(),
        9.0,
        rgb(brand::BRASS_MUTED),
    );
    l.advance(10.0);
    // Study title (serif bold), wrapped.
    for line in wrap(&report.title, CONTENT_W_MM, 24.0) {
        l.ensure(11.0);
        l.text_at(MARGIN_MM, &line, serif_bold(), 24.0, rgb(brand::INK));
        l.advance(11.0);
    }
    // Sub-title from the cover (or the honest default), italic-ish serif.
    let subtitle = report
        .cover
        .as_ref()
        .and_then(|c| c.subtitle.clone())
        .unwrap_or_else(|| "Performance & Resilience Study".to_string());
    l.advance(2.0);
    l.text_at(MARGIN_MM, &subtitle, serif(), 13.0, rgb(brand::INK));
    l.advance(8.0);
    l.rule(rgb(brand::BRASS), 1.2);
    l.advance(10.0);

    // --- MODELLED banner (non-optional honesty furniture) --------------------
    let banner_lines = [
        "MODELLED RESULTS - simulation figures, not flight/in-orbit validated.",
        "This is an engineering study, not a certification, and not ESA acceptance",
        "or TRL maturation.",
    ];
    let banner_h = 4.0 + banner_lines.len() as f32 * 5.0 + 3.0;
    l.ensure(banner_h);
    // Brass background panel.
    l.fill_rect(
        MARGIN_MM,
        l.y_mm - banner_h + 4.0,
        CONTENT_W_MM,
        banner_h,
        rgb(brand::BRASS),
    );
    l.advance(2.0);
    for line in banner_lines {
        l.advance(5.0);
        l.text_at(MARGIN_MM + 3.0, line, sans_bold(), 9.5, rgb(brand::INK));
    }
    l.advance(8.0);

    // --- Executive summary ---------------------------------------------------
    l.heading("Executive Summary");
    let summary = report.executive_summary_text();
    l.paragraph(&summary, serif(), 11.0, 5.4, rgb(brand::INK));
    l.advance(4.0);

    // --- Provenance & traceability ------------------------------------------
    l.heading("Provenance & Traceability");
    let prov = report.provenance.clone().unwrap_or_default();
    let hashes = if prov.scenario_hashes.is_empty() {
        report.bundle.scenario_hash.clone().unwrap_or_default()
    } else {
        prov.scenario_hashes.join(", ")
    };
    let dash = "-".to_string();
    let cell = |v: Option<String>| v.filter(|s| !s.is_empty()).unwrap_or_else(|| dash.clone());
    let rows: Vec<(&str, String)> = vec![
        ("Study ID", cell(prov.study_id)),
        ("Customer", cell(prov.customer)),
        ("Prepared for", cell(prov.prepared_for)),
        ("Prepared (UTC)", cell(prov.prepared_utc)),
        ("Engine version", report.engine_version_text()),
        (
            "Scenario hash(es)",
            if hashes.is_empty() {
                dash.clone()
            } else {
                hashes
            },
        ),
        ("Report fingerprint", report.fingerprint()),
        (
            "Classification",
            prov.classification
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| DEFAULT_CLASSIFICATION.to_string()),
        ),
    ];
    let label_w = 42.0_f32;
    for (label, value) in rows {
        // Wrap the value column; the label sits on the first wrapped line.
        let value_lines = wrap(&value, CONTENT_W_MM - label_w, 10.0);
        for (j, vline) in value_lines.iter().enumerate() {
            l.ensure(5.0);
            if j == 0 {
                l.text_at(MARGIN_MM, label, sans_bold(), 10.0, rgb(brand::BRASS));
            }
            l.text_at(MARGIN_MM + label_w, vline, sans(), 10.0, rgb(brand::INK));
            l.advance(5.0);
        }
    }
    l.advance(4.0);

    // --- Figures of merit (with inline validation tier) ----------------------
    l.heading("Figures of Merit");
    // Column header.
    l.ensure(6.0);
    l.text_at(
        MARGIN_MM,
        "Figure of merit",
        sans_bold(),
        9.5,
        rgb(brand::BRASS),
    );
    l.text_at(
        MARGIN_MM + 70.0,
        "Value",
        sans_bold(),
        9.5,
        rgb(brand::BRASS),
    );
    l.text_at(
        MARGIN_MM + 100.0,
        "Tier",
        sans_bold(),
        9.5,
        rgb(brand::BRASS),
    );
    l.text_at(
        MARGIN_MM + 125.0,
        "Oracle",
        sans_bold(),
        9.5,
        rgb(brand::BRASS),
    );
    l.advance(5.0);
    l.rule(rgb(brand::BRASS_MUTED), 0.4);
    l.advance(4.0);
    for f in report.bundle.all_foms() {
        l.ensure(5.5);
        let val = f
            .value
            .map(|v| format!("{v:.4}"))
            .unwrap_or_else(|| "—".into());
        // Inline tier from the open core's single source of truth (via report).
        let tier = report.fom_tier_label(&f.key).unwrap_or("—");
        // Truncate the oracle to fit its column deterministically.
        let oracle: String = {
            let max = 24;
            if f.oracle.chars().count() > max {
                let mut s: String = f.oracle.chars().take(max - 3).collect();
                s.push_str("...");
                s
            } else {
                f.oracle.clone()
            }
        };
        l.text_at(MARGIN_MM, &f.name, sans(), 9.0, rgb(brand::INK));
        l.text_at(MARGIN_MM + 70.0, &val, sans_bold(), 9.0, rgb(brand::INK));
        l.text_at(
            MARGIN_MM + 100.0,
            tier,
            sans_bold(),
            9.0,
            rgb(brand::BRASS_MUTED),
        );
        l.text_at(MARGIN_MM + 125.0, &oracle, sans(), 8.5, rgb(brand::INK));
        l.advance(5.5);
    }
    l.advance(4.0);

    // --- Verification posture ------------------------------------------------
    l.heading("Verification Posture");
    let posture = format!(
        "{} validated (external oracle)  |  {} modelled  |  {} partner-owned \
         (live from the open verification matrix).",
        report.matrix.validated, report.matrix.modelled, report.matrix.partner
    );
    l.paragraph(&posture, sans(), 10.0, 5.0, rgb(brand::INK));
    if let Some(im) = &report.impairment {
        l.advance(2.0);
        let s = format!(
            "RF-impairment evaluation: fused-detector AUC {:.3} over {} synthetic cases. {}",
            im.fused_auc, im.n_cases, im.note
        );
        l.paragraph(&s, sans(), 9.0, 4.6, rgb(brand::INK));
    }
    l.advance(4.0);

    // --- Caveats (read first) ------------------------------------------------
    l.heading("Caveats (read first)");
    for c in &report.caveats {
        let bullet = format!("-  {c}");
        l.paragraph(&bullet, sans(), 9.5, 4.8, rgb(brand::INK));
        l.advance(1.5);
    }

    // --- Assemble & serialize ------------------------------------------------
    let classification = report
        .provenance
        .as_ref()
        .and_then(|p| p.classification.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_CLASSIFICATION.to_string());
    // ASCII separators only: the built-in standard-14 fonts use WinAnsi encoding,
    // and printpdf splits a `Tj` literal at characters it cannot encode — keeping
    // the honesty phrase contiguous ASCII guarantees "not a certification" survives
    // intact in the content stream (and the test that asserts it).
    let footer = format!(
        "Ashforde OU  |  {classification}  |  MODELLED engineering study - not a certification"
    );
    let pages = l.finish(&footer);

    let mut doc = PdfDocument::new(&report.title);
    // `PdfDocumentInfo::default()` pins all dates to the Unix epoch — no clock is
    // read here or in printpdf for our build path. The only nondeterministic byte
    // sequence is the trailer `/ID` array (random in `serialize_pdf`, no hook),
    // documented in the module header and masked by tests/report_pdf.rs.
    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let bytes = doc.with_pages(pages).save(&opts, &mut warnings);

    if bytes.is_empty() || !bytes.starts_with(b"%PDF-") {
        return Err(format!(
            "printpdf produced no valid PDF (len {}, warnings: {})",
            bytes.len(),
            warnings.len()
        ));
    }
    Ok(bytes)
}

/// Render a Pack-A optimism-gap study ([`PackAReport`]) to a self-contained PDF
/// (`%PDF-…`) using the same pure-Rust [`Layout`] machinery as [`to_pdf`].
///
/// Reads the report's structured data directly (cover title/sub-title, executive
/// summary, the MODELLED + "not a certification" banner, the field-vs-bench headline
/// numbers, the per-class table, the ID-only predictor result, the provenance block,
/// and the arXiv-**preprint** citation) — it never parses the HTML face. The honesty
/// furniture (MODELLED banner, "not a certification", and the preprint cited as a
/// preprint, not peer-reviewed) is non-optional, exactly as on the HTML face.
///
/// Determinism: as with [`to_pdf`], no clock is read; printpdf pins all document
/// dates to the Unix epoch and the only nondeterministic bytes are the trailer
/// `/ID` array.
pub fn to_pdf_pack_a(report: &PackAReport) -> Result<Vec<u8>, String> {
    let s = &report.section;
    let mut l = Layout::new();

    // --- Branded cover -------------------------------------------------------
    l.advance(6.0);
    l.text_at(MARGIN_MM, "ASHFORDE", sans_bold(), 13.0, rgb(brand::BRASS));
    l.advance(6.0);
    l.text_at(
        MARGIN_MM,
        "Ashforde OU  |  Quantitative PNT Resilience",
        sans(),
        9.0,
        rgb(brand::BRASS_MUTED),
    );
    l.advance(10.0);
    for line in wrap(&report.title, CONTENT_W_MM, 24.0) {
        l.ensure(11.0);
        l.text_at(MARGIN_MM, &line, serif_bold(), 24.0, rgb(brand::INK));
        l.advance(11.0);
    }
    let subtitle = report
        .cover
        .as_ref()
        .and_then(|c| c.subtitle.clone())
        .unwrap_or_else(|| "Optimism-Gap Audit - Field-vs-Bench Study".to_string());
    l.advance(2.0);
    l.text_at(MARGIN_MM, &subtitle, serif(), 13.0, rgb(brand::INK));
    l.advance(8.0);
    l.rule(rgb(brand::BRASS), 1.2);
    l.advance(10.0);

    // --- MODELLED banner (non-optional honesty furniture) --------------------
    let banner_lines = [
        "MODELLED RESULTS - simulation figures over a synthetic, parameter-grounded",
        "corpus (never field/IQ). This is an engineering study, not a certification,",
        "and not ESA acceptance or TRL maturation.",
    ];
    let banner_h = 4.0 + banner_lines.len() as f32 * 5.0 + 3.0;
    l.ensure(banner_h);
    l.fill_rect(
        MARGIN_MM,
        l.y_mm - banner_h + 4.0,
        CONTENT_W_MM,
        banner_h,
        rgb(brand::BRASS),
    );
    l.advance(2.0);
    for line in banner_lines {
        l.advance(5.0);
        l.text_at(MARGIN_MM + 3.0, line, sans_bold(), 9.5, rgb(brand::INK));
    }
    l.advance(8.0);

    // --- Executive summary ---------------------------------------------------
    l.heading("Executive Summary");
    let summary = report
        .cover
        .as_ref()
        .and_then(|c| c.executive_summary.clone())
        .unwrap_or_else(|| {
            format!(
                "This Pack-A study reports the predicted field-vs-bench degradation (the \
                 optimism gap) for {src}: the in-distribution (bench) AUC over-states the \
                 out-of-distribution (field) AUC by {gap:.3} of the AUC range. Numbers are \
                 MODELLED over a synthetic, parameter-grounded corpus and are an engineering \
                 study, not a certification. The phenomenon and the in-distribution-only \
                 predictor are established in {arxiv}, an arXiv preprint (not peer-reviewed).",
                src = s.source_label,
                gap = s.optimism_gap,
                arxiv = ARXIV_PREPRINT_ID,
            )
        });
    l.paragraph(&summary, serif(), 11.0, 5.4, rgb(brand::INK));
    l.advance(4.0);

    // --- Field-vs-bench headline --------------------------------------------
    l.heading(&format!("Field-vs-Bench Degradation - {}", s.source_label));
    let headline = format!(
        "Predicted field-vs-bench degradation (optimism gap): {gap:.3} = AUC_id {idn:.3} \
         (bench) - AUC_ood {oodn:.3} (field).",
        gap = s.optimism_gap,
        idn = s.auc_id,
        oodn = s.auc_ood,
    );
    l.paragraph(&headline, sans_bold(), 11.0, 5.2, rgb(brand::INK));
    l.advance(2.0);

    // Per-class table (baseline pack only).
    if !s.per_class.is_empty() {
        l.advance(2.0);
        l.ensure(6.0);
        l.text_at(
            MARGIN_MM,
            "Impairment class",
            sans_bold(),
            9.5,
            rgb(brand::BRASS),
        );
        l.text_at(
            MARGIN_MM + 70.0,
            "AUC (bench)",
            sans_bold(),
            9.5,
            rgb(brand::BRASS),
        );
        l.text_at(
            MARGIN_MM + 110.0,
            "AUC (field)",
            sans_bold(),
            9.5,
            rgb(brand::BRASS),
        );
        l.text_at(
            MARGIN_MM + 150.0,
            "Gap",
            sans_bold(),
            9.5,
            rgb(brand::BRASS),
        );
        l.advance(5.0);
        l.rule(rgb(brand::BRASS_MUTED), 0.4);
        l.advance(4.0);
        for p in &s.per_class {
            l.ensure(5.5);
            l.text_at(MARGIN_MM, &p.class, sans(), 9.0, rgb(brand::INK));
            l.text_at(
                MARGIN_MM + 70.0,
                &format!("{:.3}", p.auc_id),
                sans_bold(),
                9.0,
                rgb(brand::INK),
            );
            l.text_at(
                MARGIN_MM + 110.0,
                &format!("{:.3}", p.auc_ood),
                sans_bold(),
                9.0,
                rgb(brand::INK),
            );
            l.text_at(
                MARGIN_MM + 150.0,
                &format!("{:.3}", p.optimism_gap),
                sans(),
                9.0,
                rgb(brand::INK),
            );
            l.advance(5.5);
        }
        l.advance(2.0);
    }

    // ID-only predictor headline (baseline pack only).
    if let Some((det, cls)) = s.predictor_r2 {
        let pred = format!(
            "ID-only gap predictor: in-distribution diagnostics predict the optimism gap for a \
             detector held entirely out of training (leave-one-detector-out R2 = {det:.3}) and \
             for an impairment class held out (leave-one-class-out R2 = {cls:.3}); an R2 above \
             zero beats predict-the-mean."
        );
        l.paragraph(&pred, sans(), 9.5, 4.8, rgb(brand::INK));
    }
    // Sample counts (customer intake only).
    if let Some((n_id, n_ood)) = s.sample_counts {
        let counts = format!(
            "Computed over the submission: {n_id} in-distribution and {n_ood} \
             out-of-distribution scored samples."
        );
        l.paragraph(&counts, sans(), 9.5, 4.8, rgb(brand::INK));
    }
    l.advance(4.0);

    // --- Provenance & traceability ------------------------------------------
    l.heading("Provenance & Traceability");
    let prov = report.provenance.clone().unwrap_or_default();
    let hashes = prov.scenario_hashes.join(", ");
    let dash = "-".to_string();
    let cell = |v: Option<String>| v.filter(|s| !s.is_empty()).unwrap_or_else(|| dash.clone());
    let engine = prov
        .engine_version
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            format!(
                "kshana-pro {} (embeds the open kshana optimism-gap engine)",
                crate::version()
            )
        });
    let classification = prov
        .classification
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_CLASSIFICATION.to_string());
    let rows: Vec<(&str, String)> = vec![
        ("Study ID", cell(prov.study_id)),
        ("Customer", cell(prov.customer)),
        ("Prepared for", cell(prov.prepared_for)),
        ("Prepared (UTC)", cell(prov.prepared_utc)),
        ("Engine version", engine),
        (
            "Scenario hash(es)",
            if hashes.is_empty() {
                dash.clone()
            } else {
                hashes
            },
        ),
        (
            "Proof",
            format!("{ARXIV_PREPRINT_ID} - an arXiv preprint, not peer-reviewed"),
        ),
        ("Classification", classification.clone()),
    ];
    let label_w = 42.0_f32;
    for (label, value) in rows {
        let value_lines = wrap(&value, CONTENT_W_MM - label_w, 10.0);
        for (j, vline) in value_lines.iter().enumerate() {
            l.ensure(5.0);
            if j == 0 {
                l.text_at(MARGIN_MM, label, sans_bold(), 10.0, rgb(brand::BRASS));
            }
            l.text_at(MARGIN_MM + label_w, vline, sans(), 10.0, rgb(brand::INK));
            l.advance(5.0);
        }
    }
    l.advance(4.0);

    // --- Reference (the preprint, cited as a preprint) -----------------------
    //
    // Two lines, each rendered as a single un-wrapped text op, so the load-bearing
    // honesty literals ("arXiv preprint", "not peer-reviewed", "not a certification")
    // survive intact in the content stream (greedy wrapping could otherwise split a
    // phrase across two text ops). No parentheses: printpdf hex-encodes a literal
    // that contains `(`/`)`, which would also hide the plain text.
    l.heading("Reference");
    let ref_line_1 =
        "The optimism-gap phenomenon and the in-distribution-only gap predictor used here are";
    let ref_line_2 = format!(
        "established in {ARXIV_PREPRINT_ID}, an arXiv preprint - not peer-reviewed and not a certification."
    );
    l.ensure(5.0);
    l.text_at(MARGIN_MM, ref_line_1, sans(), 9.5, rgb(brand::INK));
    l.advance(4.8);
    l.ensure(5.0);
    l.text_at(MARGIN_MM, &ref_line_2, sans(), 9.5, rgb(brand::INK));
    l.advance(4.8);

    // --- Assemble & serialize ------------------------------------------------
    // No parentheses in the footer: a parenthesised literal is hex-encoded by
    // printpdf, which would hide "not a certification" from the content stream.
    let footer = format!(
        "Ashforde OU  |  {classification}  |  MODELLED engineering study - not a certification. \
         Proof: {ARXIV_PREPRINT_ID}, an arXiv preprint."
    );
    let pages = l.finish(&footer);

    let mut doc = PdfDocument::new(&report.title);
    let opts = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let bytes = doc.with_pages(pages).save(&opts, &mut warnings);

    if bytes.is_empty() || !bytes.starts_with(b"%PDF-") {
        return Err(format!(
            "printpdf produced no valid PDF (len {}, warnings: {})",
            bytes.len(),
            warnings.len()
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{build_report, StudyCover, StudyProvenance};

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

    #[test]
    fn rgb_parses_brand_hex() {
        // Brass #c7aa66 → (199, 170, 102) / 255.
        if let Color::Rgb(c) = rgb(brand::BRASS) {
            assert!((c.r - 199.0 / 255.0).abs() < 1e-6);
            assert!((c.g - 170.0 / 255.0).abs() < 1e-6);
            assert!((c.b - 102.0 / 255.0).abs() < 1e-6);
        } else {
            panic!("expected an RGB colour");
        }
    }

    #[test]
    fn wrap_breaks_long_text_and_never_empties() {
        let lines = wrap(
            "the quick brown fox jumps over the lazy dog repeatedly",
            20.0,
            11.0,
        );
        assert!(lines.len() > 1, "long text wraps to multiple lines");
        assert!(
            wrap("", 20.0, 11.0).len() == 1,
            "empty wraps to one empty line"
        );
    }

    #[test]
    fn minimal_report_renders_a_pdf() {
        // No cover / no provenance — back-compat path must still produce a PDF.
        let r = build_report("Bare Study", &clock_scenario(), "quantum").unwrap();
        let bytes = to_pdf(&r).expect("render");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 1000);
    }

    #[test]
    fn full_report_carries_honesty_text() {
        let r = build_report("Full Study", &clock_scenario(), "quantum")
            .unwrap()
            .with_cover(StudyCover {
                subtitle: Some("Pilot".into()),
                executive_summary: Some("Bespoke summary.".into()),
            })
            .with_provenance(StudyProvenance {
                study_id: Some("KP-9".into()),
                classification: Some("Restricted".into()),
                ..Default::default()
            });
        let bytes = to_pdf(&r).expect("render");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("MODELLED"));
        assert!(text.contains("not a certification"));
    }

    /// The Pack-A PDF face renders a real PDF and carries the same non-negotiable
    /// honesty furniture as the HTML face: the MODELLED banner, the literal "not a
    /// certification" disclaimer, and the arXiv id cited as a preprint.
    #[test]
    fn pack_a_pdf_renders_with_honesty_furniture() {
        use crate::packs::optimism_gap_audit::OptimismGapAuditScenario;
        use crate::report_pack_a::PackAReport;
        let a = OptimismGapAuditScenario {
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
        .expect("audit computes");
        let report = PackAReport::from_audit("Pack A - Optimism-Gap Audit (SAMPLE)", &a)
            .with_provenance(StudyProvenance {
                study_id: Some("KP-SAMPLE".into()),
                ..Default::default()
            });
        let bytes = to_pdf_pack_a(&report).expect("pack-a render");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 1000);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("MODELLED"));
        assert!(text.contains("not a certification"));
        assert!(text.contains("preprint"));
    }
}
