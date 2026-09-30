// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Shared branded-PDF builder for the Pro study packs
//!
//! A small, self-contained, pure-Rust PDF layout engine reused by the Pro *study*
//! packs (L48 lunar-time-budget, L49/L50 realtime-frame-EOP) so each pack can emit a
//! branded PDF deliverable without duplicating the [`printpdf`] plumbing. It mirrors
//! the primitives of [`crate::report_pdf`] (which renders the *consolidated* report
//! from its own structured type) but is generic over free-form study content: a
//! branded cover, the non-negotiable **MODELLED** banner, headings, wrapped
//! paragraphs, key/value rows and simple tabular rows.
//!
//! ## Honesty & determinism
//! The builder carries the same non-optional furniture as [`crate::report_pdf`] — the
//! Ashforde cover, the MODELLED "not a certification" banner — and reads no clock:
//! `printpdf`'s `PdfDocumentInfo::default()` pins all document dates to the Unix
//! epoch, so two renders of the same content are byte-identical except for the PDF
//! trailer `/ID` array (random inside `printpdf`'s serializer, with no override
//! hook), exactly as documented for [`crate::report_pdf`]. Only ASCII text is drawn
//! so the built-in WinAnsi standard-14 fonts encode every glyph (the honesty phrase
//! "not a certification" therefore survives contiguously in the content stream).

use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PaintMode, PdfFontHandle, Point, Polygon,
    PolygonRing, Pt, Rgb, TextItem, WindingOrder,
};

use crate::report_brand as brand;

// --- A4 page geometry (mm) ---------------------------------------------------
const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_MM: f32 = 20.0;
const CONTENT_W_MM: f32 = PAGE_W_MM - 2.0 * MARGIN_MM;
const TOP_Y_MM: f32 = PAGE_H_MM - MARGIN_MM;
const BOTTOM_Y_MM: f32 = MARGIN_MM + 6.0;

// 1 mm = 72/25.4 pt.
const MM_PER_PT: f32 = 25.4 / 72.0;

/// A brand colour as a printpdf RGB colour (parses `#rrggbb`).
fn rgb(hex: &str) -> Color {
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

/// Conservative average glyph advance (~0.52 em) — the built-in standard-14 metrics
/// are not exposed by printpdf, so this keeps wrapping deterministic and font-free.
fn text_width_mm(n: usize, pt: f32) -> f32 {
    let em_mm = pt * MM_PER_PT;
    n as f32 * em_mm * 0.52
}

/// Greedy word-wrap to a `width_mm`-wide column at point size `pt`.
fn wrap(text: &str, width_mm: f32, pt: f32) -> Vec<String> {
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

fn serif() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesRoman)
}
fn serif_bold() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesBold)
}
// Every face of an Ashforde document is a serif: the former sans labels use Times too.
fn sans() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesRoman)
}
fn sans_bold() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesBold)
}

/// One placed cell of a tabular row: `(x-offset-mm-from-left-margin, text, bold)`.
pub struct Cell<'a> {
    /// Millimetres to the right of the left margin where this cell's text begins.
    pub x_mm: f32,
    /// The (ASCII) text to draw.
    pub text: &'a str,
    /// Draw with the bold sans face when `true`, else the regular sans face.
    pub bold: bool,
    /// `#rrggbb` fill colour for the text.
    pub colour: &'a str,
}

/// An incremental, page-aware branded-PDF builder. Public methods append content and
/// flow onto a new page when the cursor would cross the bottom margin.
pub struct StudyPdf {
    pages: Vec<Vec<Op>>,
    ops: Vec<Op>,
    y_mm: f32,
}

impl Default for StudyPdf {
    fn default() -> Self {
        Self::new()
    }
}

impl StudyPdf {
    /// A fresh builder positioned at the top margin.
    pub fn new() -> Self {
        StudyPdf {
            pages: Vec::new(),
            ops: Vec::new(),
            y_mm: TOP_Y_MM,
        }
    }

    fn new_page(&mut self) {
        if !self.ops.is_empty() {
            let done = std::mem::take(&mut self.ops);
            self.pages.push(done);
        }
        self.y_mm = TOP_Y_MM;
    }

    fn ensure(&mut self, need_mm: f32) {
        if self.y_mm - need_mm < BOTTOM_Y_MM {
            self.new_page();
        }
    }

    fn advance(&mut self, mm: f32) {
        self.y_mm -= mm;
    }

    /// A filled rectangle, drawn as a four-point polygon: `printpdf` 0.9.1 writes
    /// `Op::DrawRectangle` as a clipping path whatever its paint mode, which clipped
    /// everything after it on the page (the page furniture included) to its area.
    fn fill_rect(&mut self, x_mm: f32, y_mm: f32, w_mm: f32, h_mm: f32, col: Color) {
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

    fn rule(&mut self, col: Color, thickness_pt: f32) {
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

    fn text_at(&mut self, x_mm: f32, text: &str, font: PdfFontHandle, pt: f32, col: Color) {
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

    /// The branded cover: the Ashforde identity, the study `title` (wrapped) and an
    /// italic-ish serif `subtitle`, closed by a brass rule.
    pub fn cover(&mut self, title: &str, subtitle: &str) {
        self.ops.extend(crate::brand_pdf::cover_lockup_ops(
            "Quantitative PNT Resilience",
        ));
        self.y_mm = PAGE_H_MM - crate::brand_pdf::COVER_BAND_MM - 14.0;
        for line in wrap(title, CONTENT_W_MM, 22.0) {
            self.ensure(10.0);
            self.text_at(MARGIN_MM, &line, serif_bold(), 22.0, rgb(brand::INK));
            self.advance(10.0);
        }
        self.advance(1.0);
        self.text_at(MARGIN_MM, subtitle, serif(), 12.5, rgb(brand::INK));
        self.advance(7.0);
        self.rule(rgb(brand::BRASS), 1.2);
        self.advance(9.0);
    }

    /// The non-negotiable MODELLED banner (brass panel, ink text). `detail` lines are
    /// drawn between the standing lead and the standing "not a certification" tail so
    /// the honesty furniture is identical to the HTML/consolidated faces.
    pub fn modelled_banner(&mut self, detail_lines: &[&str]) {
        let mut lines: Vec<String> = vec![
            "MODELLED RESULTS - simulation figures, not flight/in-orbit validated.".to_string(),
        ];
        for d in detail_lines {
            lines.push((*d).to_string());
        }
        lines.push("This is an engineering study, not a certification, and not ESA".to_string());
        lines.push("acceptance or TRL maturation.".to_string());

        let banner_h = 4.0 + lines.len() as f32 * 5.0 + 3.0;
        self.ensure(banner_h);
        self.fill_rect(
            MARGIN_MM,
            self.y_mm - banner_h + 4.0,
            CONTENT_W_MM,
            banner_h,
            rgb(brand::BRASS_SEAL),
        );
        self.advance(2.0);
        for line in &lines {
            self.advance(5.0);
            self.text_at(MARGIN_MM + 3.0, line, sans_bold(), 9.5, rgb(brand::INK));
        }
        self.advance(8.0);
    }

    /// A serif-bold brass section heading with a thin brass underline rule.
    pub fn heading(&mut self, title: &str) {
        self.ensure(14.0);
        self.advance(4.0);
        self.text_at(MARGIN_MM, title, serif_bold(), 15.0, rgb(brand::BRASS));
        self.advance(6.0);
        self.rule(rgb(brand::BRASS_MUTED), 0.6);
        self.advance(6.0);
    }

    /// A wrapped serif paragraph across the full content column, at ink colour.
    pub fn paragraph(&mut self, text: &str) {
        for line in wrap(text, CONTENT_W_MM, 11.0) {
            self.ensure(5.4);
            self.text_at(MARGIN_MM, &line, serif(), 11.0, rgb(brand::INK));
            self.advance(5.4);
        }
    }

    /// A key/value row: `label` in brass, `value` (wrapped) in ink.
    pub fn kv(&mut self, label: &str, value: &str) {
        let label_w = 46.0_f32;
        for (j, vline) in wrap(value, CONTENT_W_MM - label_w, 10.0).iter().enumerate() {
            self.ensure(5.0);
            if j == 0 {
                self.text_at(MARGIN_MM, label, sans_bold(), 10.0, rgb(brand::BRASS));
            }
            self.text_at(MARGIN_MM + label_w, vline, sans(), 10.0, rgb(brand::INK));
            self.advance(5.0);
        }
    }

    /// A single tabular row of pre-positioned cells at 9 pt, advancing one line.
    pub fn row(&mut self, cells: &[Cell<'_>]) {
        self.ensure(5.4);
        for c in cells {
            let font = if c.bold { sans_bold() } else { sans() };
            self.text_at(MARGIN_MM + c.x_mm, c.text, font, 9.0, rgb(c.colour));
        }
        self.advance(5.4);
    }

    /// A bold brass table-header row followed by a thin rule.
    pub fn header_row(&mut self, cells: &[(f32, &str)]) {
        self.ensure(6.0);
        for (x, t) in cells {
            self.text_at(MARGIN_MM + *x, t, sans_bold(), 9.5, rgb(brand::BRASS));
        }
        self.advance(5.0);
        self.rule(rgb(brand::BRASS_MUTED), 0.4);
        self.advance(4.0);
    }

    /// A little vertical breathing room.
    pub fn gap(&mut self, mm: f32) {
        self.advance(mm);
    }

    /// Finalise into PDF bytes: dress every page in the Ashforde furniture
    /// ([`crate::brand_pdf::dress`]: watermark, running header, the footer line
    /// `footer` with the document id and page number, the SAMPLE mark when
    /// [`brand::sample`] is on), assemble the document and serialize. Returns `Err` if
    /// the serializer produced no `%PDF-` output.
    pub fn render(mut self, title: &str, footer: &str) -> Result<Vec<u8>, String> {
        if !self.ops.is_empty() {
            let done = std::mem::take(&mut self.ops);
            self.pages.push(done);
        }
        if self.pages.is_empty() {
            self.pages.push(Vec::new());
        }
        let mark = crate::brand_pdf::PdfMark::new(
            crate::mission::pdf::ascii(title),
            crate::brand_pdf::content_id(&self.pages),
        );
        let pages = crate::brand_pdf::dress(self.pages, footer, &mark, brand::sample());
        crate::brand_pdf::save(title, pages)
    }
}
