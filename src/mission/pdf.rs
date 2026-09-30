// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The PDF face of the mission dossier.
//!
//! A4, drawn from `printpdf` primitives with the built-in fonts and the brand colours of
//! [`crate::report_brand`], on the page machinery of [`crate::report_pdf`]
//! ([`Layout`], `wrap`, the font and colour functions). It renders the dossier body and
//! nothing else: the cover, the MODELLED banner with the words "not a certification",
//! the verdict summary, the verification matrix, one sheet per run with its charts
//! drawn as vectors, the trade section, the open items, the reproducibility record, the
//! inputs, the sign-off table and the limitations. Every table repeats its header on a
//! new page, and every page carries its number.
//!
//! No clock is read. `printpdf` pins the document dates to the Unix epoch, so two
//! renders of the same dossier are identical except for the trailer `/ID`, which
//! `printpdf` fills with random characters (see `tests/mission_dossier.rs` for the
//! mask).

use printpdf::{BuiltinFont, Op, PdfFontHandle};

use super::compose::{Dossier, RunSheet, BANNER_LINES};
use super::figures::FigureRow;
use super::pdf_chart;
use crate::report_brand as brand;
use crate::report_pdf::{
    rgb, sans, sans_bold, serif, serif_bold, wrap, Layout, BOTTOM_Y_MM, CONTENT_W_MM, MARGIN_MM,
    MM_PER_PT, PAGE_H_MM,
};

const MET: &str = "#1f7a52";
const NOT_MET: &str = "#b3261e";
const NOT_VERIFIED: &str = "#8a6d00";
const FLAGGED: &str = "#6a3fb5";
const GRID: &str = "#d9d3c7";
const MUTED: &str = "#6b645a";

/// Figures shown on a run's sheet before the rest is left to `dossier.json`.
pub const MAX_SHEET_FIGURES: usize = 30;
/// Lines of text one table cell may hold in the PDF.
const MAX_CELL_LINES: usize = 40;
/// Outputs of another provenance class shown on a run's sheet.
pub const MAX_SHEET_OTHER_OUTPUTS: usize = 12;
/// Not-modelled notes shown on a run's sheet before the rest is left to the report.
pub const MAX_SHEET_NOTES: usize = 6;

/// The rendered PDF.
#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub pages: usize,
}

fn mono() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::Courier)
}

/// Text the built-in fonts can show: ASCII, with the common typographic characters of
/// the engine's notes written out. Anything else becomes `?`.
pub fn ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2014}' | '\u{2013}' | '\u{2212}' | '\u{2010}' | '\u{2011}' => out.push('-'),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201c}' | '\u{201d}' => out.push('"'),
            '\u{2026}' => out.push_str("..."),
            '\u{00d7}' => out.push('x'),
            '\u{00b7}' | '\u{2022}' => out.push('|'),
            '\u{2264}' => out.push_str("<="),
            '\u{2265}' => out.push_str(">="),
            '\u{2192}' => out.push_str("->"),
            '\u{2190}' => out.push_str("<-"),
            '\u{00b0}' => out.push_str(" deg"),
            '\u{00b2}' => out.push_str("^2"),
            '\u{00b3}' => out.push_str("^3"),
            '\u{00b5}' | '\u{03bc}' => out.push('u'),
            '\u{03c3}' => out.push_str("sigma"),
            '\u{03c7}' => out.push_str("chi"),
            '\u{0394}' => out.push_str("delta"),
            '\u{00b1}' => out.push_str("+/-"),
            '\u{222a}' => out.push_str("and"),
            '\u{00a0}' => out.push(' '),
            '\u{00dc}' => out.push('U'),
            '\u{00fc}' => out.push('u'),
            '\t' | '\n' | '\r' => out.push(' '),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// A number as the dossier prints it: six significant digits, no trailing zeros.
pub fn num(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let a = v.abs();
    if !(1e-4..1e7).contains(&a) {
        return format!("{v:.5e}");
    }
    let decimals = (5 - a.log10().floor() as i32).clamp(0, 9) as usize;
    let s = format!("{v:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Wrap `text` to a cell `width_mm` wide. A token longer than the cell (a field path, a
/// hash) is cut to fit, so nothing runs into the next column. `em` is the mean advance
/// of the face in ems (0.52 for the proportional faces, 0.6 for the monospaced one).
fn wrap_cell(text: &str, width_mm: f32, pt: f32, em: f32) -> Vec<String> {
    let max_chars = ((width_mm / (pt * MM_PER_PT * em)).floor() as usize).max(4);
    let mut pieces: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        let chars: Vec<char> = word.chars().collect();
        if chars.len() <= max_chars {
            pieces.push(word.to_string());
        } else {
            for chunk in chars.chunks(max_chars) {
                pieces.push(chunk.iter().collect());
            }
        }
    }
    // `wrap` measures with the proportional advance; scale the width for another face.
    wrap(&pieces.join(" "), width_mm * 0.52 / em, pt)
}

struct Col {
    title: &'static str,
    w: f32,
    mono: bool,
}

#[derive(Clone)]
struct Cell {
    text: String,
    bold: bool,
    colour: &'static str,
}

fn cell(text: impl Into<String>) -> Cell {
    Cell {
        text: text.into(),
        bold: false,
        colour: brand::INK,
    }
}

fn bold(text: impl Into<String>, colour: &'static str) -> Cell {
    Cell {
        text: text.into(),
        bold: true,
        colour,
    }
}

fn verdict_colour(v: &str) -> &'static str {
    match v {
        "met" => MET,
        "not met" => NOT_MET,
        _ => NOT_VERIFIED,
    }
}

fn table_header(l: &mut Layout, cols: &[Col], pt: f32) {
    let mut x = MARGIN_MM;
    for c in cols {
        l.text_at(x, c.title, sans_bold(), pt, rgb(brand::BRASS_MUTED));
        x += c.w;
    }
    l.advance(1.6);
    l.rule(rgb(brand::BRASS_MUTED), 0.5);
    l.advance(4.0);
}

/// A table whose header is drawn again at the top of every page it runs onto.
fn table(l: &mut Layout, cols: &[Col], rows: &[Vec<Cell>], pt: f32) {
    let lh = pt * MM_PER_PT * 1.25;
    if l.y_mm - (8.0 + lh * 2.0) < BOTTOM_Y_MM {
        l.new_page();
    }
    table_header(l, cols, pt);
    for row in rows {
        let wrapped: Vec<Vec<String>> = row
            .iter()
            .zip(cols)
            .map(|(c, col)| {
                let (size, em) = if col.mono {
                    (pt - 0.8, 0.6)
                } else {
                    (pt, 0.52)
                };
                wrap_cell(&ascii(&c.text), col.w - 2.0, size, em)
            })
            .collect();
        // A cell never grows past what one page holds.
        let wrapped: Vec<Vec<String>> = wrapped
            .into_iter()
            .map(|mut lines| {
                if lines.len() > MAX_CELL_LINES {
                    lines.truncate(MAX_CELL_LINES);
                    lines.push("[cut here; the full text is in dossier.json]".to_string());
                }
                lines
            })
            .collect();
        let lines = wrapped.iter().map(Vec::len).max().unwrap_or(1);
        let height = lines as f32 * lh + 1.6;
        if l.y_mm - height < BOTTOM_Y_MM {
            l.new_page();
            table_header(l, cols, pt);
        }
        let top = l.y_mm;
        let mut x = MARGIN_MM;
        for ((c, col), cell_lines) in row.iter().zip(cols).zip(&wrapped) {
            let (font, size) = if col.mono {
                (mono(), pt - 0.8)
            } else if c.bold {
                (sans_bold(), pt)
            } else {
                (sans(), pt)
            };
            for (i, line) in cell_lines.iter().enumerate() {
                l.y_mm = top - i as f32 * lh;
                l.text_at(x, line, font.clone(), size, rgb(c.colour));
            }
            x += col.w;
        }
        l.y_mm = top - (lines as f32 - 1.0) * lh;
        l.advance(1.8);
        l.rule(rgb(GRID), 0.25);
        l.advance(lh);
    }
    l.advance(2.0);
}

fn para(l: &mut Layout, text: &str, pt: f32) {
    l.paragraph(
        &ascii(text),
        sans(),
        pt,
        pt * MM_PER_PT * 1.3,
        rgb(brand::INK),
    );
}

fn small(l: &mut Layout, text: &str) {
    l.paragraph(&ascii(text), sans(), 8.0, 3.7, rgb(MUTED));
}

fn sub_heading(l: &mut Layout, title: &str) {
    l.ensure(12.0);
    l.advance(2.0);
    l.text_at(MARGIN_MM, &ascii(title), sans_bold(), 10.0, rgb(brand::INK));
    l.advance(5.0);
}

/// A section heading that is never left alone at the foot of a page.
fn section(l: &mut Layout, title: &str) {
    l.ensure(46.0);
    l.heading(title);
}

fn banner(l: &mut Layout) {
    let h = 4.0 + BANNER_LINES.len() as f32 * 5.0 + 4.5;
    l.ensure(h);
    l.fill_rect(
        MARGIN_MM,
        l.y_mm - h + 4.0,
        CONTENT_W_MM,
        h,
        rgb(brand::BRASS_SEAL),
    );
    l.advance(2.0);
    for line in BANNER_LINES {
        l.advance(5.0);
        l.text_at(MARGIN_MM + 3.0, line, sans_bold(), 8.6, rgb(brand::INK));
    }
    l.advance(9.5);
}

fn key_values(l: &mut Layout, rows: &[(&str, String)], mono_values: bool) {
    let label_w = 40.0_f32;
    for (label, value) in rows {
        let (font, pt, em) = if mono_values {
            (mono(), 8.0, 0.6)
        } else {
            (sans(), 9.0, 0.52)
        };
        let lines = wrap_cell(&ascii(value), CONTENT_W_MM - label_w, pt, em);
        for (j, line) in lines.iter().enumerate() {
            l.ensure(4.6);
            if j == 0 {
                l.text_at(MARGIN_MM, label, sans_bold(), 9.0, rgb(brand::BRASS_MUTED));
            }
            l.text_at(MARGIN_MM + label_w, line, font.clone(), pt, rgb(brand::INK));
            l.advance(4.6);
        }
    }
}

fn cover(l: &mut Layout, d: &Dossier) {
    let b = &d.body;
    let id = &b.identification;
    // The Ashforde lockup across the top of the cover, then the title in navy.
    l.ops.extend(crate::brand_pdf::cover_lockup_ops(
        "Quantitative PNT Resilience",
    ));
    l.y_mm = PAGE_H_MM - crate::brand_pdf::COVER_BAND_MM - 16.0;
    for line in wrap(&ascii(&id.title), CONTENT_W_MM, 23.0).iter().take(3) {
        l.text_at(MARGIN_MM, line, serif_bold(), 23.0, rgb(brand::NAVY_INK));
        l.advance(10.5);
    }
    l.advance(1.0);
    l.text_at(
        MARGIN_MM,
        "Mission dossier: requirements, runs and verification matrix",
        serif(),
        13.0,
        rgb(brand::INK),
    );
    l.advance(12.0);

    banner(l);

    let engines = if id.engine_versions.is_empty() {
        "not stamped".to_string()
    } else {
        id.engine_versions.join(", ")
    };
    let rows: Vec<(&str, String)> = vec![
        (
            "Reference",
            if id.reference.is_empty() {
                "-".into()
            } else {
                id.reference.clone()
            },
        ),
        ("Issue", id.issue.clone()),
        ("Classification", id.classification.clone()),
        (
            "Issued",
            d.issued_utc.clone().unwrap_or_else(|| "not stamped".into()),
        ),
        (
            "Engine",
            format!(
                "open Kshana {engines}, {} scenario kinds",
                id.open_engine_kinds
            ),
        ),
        (
            "Overlay",
            format!(
                "Kshana Pro {}, {} build",
                id.overlay_version, id.build_profile
            ),
        ),
    ];
    key_values(l, &rows, false);
    key_values(l, &[("Dossier SHA-256", d.dossier_sha256.clone())], true);
    l.advance(6.0);

    // Verdict tiles.
    let s = &b.summary;
    let tiles: [(&str, usize, &str); 5] = [
        ("Requirements", s.requirements, brand::INK),
        ("Met", s.met, MET),
        ("Not met", s.not_met, NOT_MET),
        ("Not verified", s.not_verified, NOT_VERIFIED),
        ("Open items", s.open_items, brand::INK),
    ];
    let gap = 3.0;
    let w = (CONTENT_W_MM - gap * 4.0) / 5.0;
    let h = 24.0;
    l.ensure(h + 4.0);
    let top = l.y_mm;
    for (i, (label, n, colour)) in tiles.iter().enumerate() {
        let x = MARGIN_MM + i as f32 * (w + gap);
        l.fill_rect(x, top - h, w, h, rgb(brand::PLATE));
        l.fill_rect(x, top - 1.0, w, 1.0, rgb(colour));
        l.y_mm = top - 12.0;
        l.text_at(x + 3.0, &n.to_string(), serif_bold(), 22.0, rgb(colour));
        l.y_mm = top - 19.5;
        l.text_at(x + 3.0, label, sans(), 8.5, rgb(MUTED));
    }
    l.y_mm = top - h - 8.0;
    para(
        l,
        &format!(
            "{} scenario runs{}. {} verdicts, one per requirement and linked run: {} on a VALIDATED basis, {} on a MODELLED basis, {} on a PARTNER basis.",
            s.runs,
            if s.trade_studies > 0 { " and one trade study" } else { "" },
            s.verdicts,
            s.verdicts_by_basis.validated,
            s.verdicts_by_basis.modelled,
            s.verdicts_by_basis.partner
        ),
        9.5,
    );
}

fn summary_and_matrix(l: &mut Layout, d: &Dossier) {
    let b = &d.body;
    let s = &b.summary;
    l.new_page();
    l.heading("Verdict summary");
    para(
        l,
        &format!(
            "{} requirements: {} met, {} not met, {} not verified. Met on a MODELLED or PARTNER basis, and flagged: {}. With no run traced: {}. Open items to close before sign-off: {}.",
            s.requirements,
            s.met,
            s.not_met,
            s.not_verified,
            s.met_on_modelled_basis,
            s.without_evidence,
            s.open_items
        ),
        10.0,
    );
    l.advance(1.5);
    small(
        l,
        "Met: the criterion holds on every linked run. Not met: it fails on at least one. Not verified: a field is missing, a unit does not match, an array is not reduced, the run's files fail an integrity check, or no run is linked. VALIDATED, MODELLED and PARTNER are the open engine's own labels for the capability behind a number; a verdict carries the weakest label it rests on.",
    );
    l.advance(3.0);

    section(l, "Verification matrix");
    let cols = [
        Col {
            title: "Requirement",
            w: 26.0,
            mono: false,
        },
        Col {
            title: "Criterion",
            w: 46.0,
            mono: true,
        },
        Col {
            title: "Run",
            w: 25.0,
            mono: false,
        },
        Col {
            title: "Verdict",
            w: 20.0,
            mono: false,
        },
        Col {
            title: "Basis",
            w: 20.0,
            mono: false,
        },
        Col {
            title: "Value read",
            w: 33.0,
            mono: false,
        },
    ];
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for r in &b.matrix {
        let crit = r.criterion.clone().unwrap_or_else(|| "none".into());
        if r.cells.is_empty() {
            rows.push(vec![
                bold(r.requirement.clone(), brand::INK),
                cell(crit),
                cell("-"),
                bold("not verified", NOT_VERIFIED),
                cell("-"),
                cell(format!("no run traced; method {}", r.method)),
            ]);
            continue;
        }
        for (i, c) in r.cells.iter().enumerate() {
            let value = if c.values.is_empty() {
                c.reason.clone().unwrap_or_default()
            } else {
                c.values
                    .iter()
                    .map(|v| {
                        let n = v
                            .value
                            .as_f64()
                            .map(num)
                            .unwrap_or_else(|| v.value.to_string());
                        match v.unit.as_deref() {
                            Some(u) if u != "1" => format!("{n} {u}"),
                            _ => n,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            rows.push(vec![
                if i == 0 {
                    bold(r.requirement.clone(), brand::INK)
                } else {
                    cell("")
                },
                if i == 0 { cell(crit.clone()) } else { cell("") },
                cell(c.run.clone()),
                bold(c.verdict.clone(), verdict_colour(&c.verdict)),
                bold(c.basis.clone(), if c.flagged { FLAGGED } else { MET }),
                cell(value),
            ]);
        }
    }
    table(l, &cols, &rows, 8.0);

    sub_heading(l, "Requirements and what each verdict read");
    let cols = [
        Col {
            title: "Requirement",
            w: 26.0,
            mono: false,
        },
        Col {
            title: "Method",
            w: 24.0,
            mono: false,
        },
        Col {
            title: "Text and evidence",
            w: 120.0,
            mono: false,
        },
    ];
    let rows: Vec<Vec<Cell>> = b
        .matrix
        .iter()
        .map(|r| {
            let mut t = r.text.clone();
            for c in &r.cells {
                for v in &c.values {
                    t.push_str(&format!(" [{}: {}]", c.run, v.detail));
                }
                if c.values.is_empty() {
                    if let Some(reason) = &c.reason {
                        t.push_str(&format!(" [{}: {}]", c.run, reason));
                    }
                }
            }
            vec![
                bold(r.requirement.clone(), brand::INK),
                cell(r.method.clone()),
                cell(t),
            ]
        })
        .collect();
    table(l, &cols, &rows, 8.0);
}

fn figure_rows(figs: &[FigureRow]) -> Vec<Vec<Cell>> {
    figs.iter()
        .map(|f| {
            vec![
                cell(f.path.clone()),
                bold(num(f.value), brand::INK),
                cell(f.unit.clone().unwrap_or_else(|| "-".into())),
                cell(match (&f.element, &f.note) {
                    (Some(e), Some(n)) => format!("{e}: {n}"),
                    (Some(e), None) => e.clone(),
                    (None, Some(n)) => n.clone(),
                    (None, None) => String::new(),
                }),
            ]
        })
        .collect()
}

fn sheet(l: &mut Layout, s: &RunSheet) {
    l.new_page();
    l.heading(&ascii(&format!("Run {}", s.id)));
    for line in wrap(&ascii(&s.title), CONTENT_W_MM, 13.0).iter().take(2) {
        l.text_at(MARGIN_MM, line, serif_bold(), 13.0, rgb(brand::INK));
        l.advance(6.0);
    }
    if let Some(desc) = &s.description {
        para(l, desc, 9.5);
    }
    l.advance(1.0);
    key_values(
        l,
        &[
            ("Kind", s.kind.clone()),
            (
                "Stamp",
                format!(
                    "seed {}, engine {}",
                    s.seed
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "not recorded".into()),
                    s.engine_version
                        .clone()
                        .unwrap_or_else(|| "not recorded".into())
                ),
            ),
            ("Engine summary", s.summary.clone()),
        ],
        false,
    );
    key_values(
        l,
        &[
            (
                "Scenario hash",
                s.scenario_hash
                    .clone()
                    .unwrap_or_else(|| "not stamped by this kind".into()),
            ),
            ("Result SHA-256", s.result.sha256.clone()),
        ],
        true,
    );
    if let Some(label) = &s.label {
        l.advance(1.5);
        small(l, &format!("Run label: {label}"));
    }
    l.advance(2.0);

    if !s.verdicts.is_empty() {
        sub_heading(l, "Requirements verified on this run");
        let cols = [
            Col {
                title: "Requirement",
                w: 60.0,
                mono: false,
            },
            Col {
                title: "Verdict",
                w: 50.0,
                mono: false,
            },
            Col {
                title: "Basis",
                w: 60.0,
                mono: false,
            },
        ];
        let rows: Vec<Vec<Cell>> = s
            .verdicts
            .iter()
            .map(|v| {
                vec![
                    bold(v.requirement.clone(), brand::INK),
                    bold(v.verdict.clone(), verdict_colour(&v.verdict)),
                    cell(v.basis.clone()),
                ]
            })
            .collect();
        table(l, &cols, &rows, 8.5);
    }

    sub_heading(l, "Headline figures: scalars the result marks as computed");
    if s.headline.is_empty() {
        small(
            l,
            if s.no_provenance {
                "The result document carries no units block with provenance, so nothing in it can be called a figure."
            } else {
                "The result marks no scalar as computed."
            },
        );
        l.advance(2.0);
    } else {
        let cols = [
            Col {
                title: "Field",
                w: 66.0,
                mono: true,
            },
            Col {
                title: "Value",
                w: 24.0,
                mono: false,
            },
            Col {
                title: "Unit",
                w: 18.0,
                mono: false,
            },
            Col {
                title: "What it is",
                w: 62.0,
                mono: false,
            },
        ];
        let shown = &s.headline[..s.headline.len().min(MAX_SHEET_FIGURES)];
        table(l, &cols, &figure_rows(shown), 7.6);
        if s.headline.len() > shown.len() {
            small(
                l,
                &format!(
                    "{} more computed scalars are listed in dossier.json.",
                    s.headline.len() - shown.len()
                ),
            );
        }
    }
    if !s.other_outputs.is_empty() {
        sub_heading(l, "Outputs the result marks with another provenance class");
        let cols = [
            Col {
                title: "Field",
                w: 66.0,
                mono: true,
            },
            Col {
                title: "Value",
                w: 24.0,
                mono: false,
            },
            Col {
                title: "Unit",
                w: 18.0,
                mono: false,
            },
            Col {
                title: "Provenance and what it is",
                w: 62.0,
                mono: false,
            },
        ];
        let shown = &s.other_outputs[..s.other_outputs.len().min(MAX_SHEET_OTHER_OUTPUTS)];
        let rows: Vec<Vec<Cell>> = shown
            .iter()
            .map(|f| {
                vec![
                    cell(f.path.clone()),
                    bold(num(f.value), brand::INK),
                    cell(f.unit.clone().unwrap_or_else(|| "-".into())),
                    cell(format!(
                        "{}: {}",
                        f.provenance,
                        f.note.clone().unwrap_or_default()
                    )),
                ]
            })
            .collect();
        table(l, &cols, &rows, 7.6);
        if s.other_outputs.len() > shown.len() {
            small(
                l,
                &format!(
                    "{} more are listed in dossier.json.",
                    s.other_outputs.len() - shown.len()
                ),
            );
        }
    }
    small(
        l,
        &format!(
            "In dossier.json for this run: {} computed scalars, {} computed columns, {} outputs of another provenance class, {} echoed inputs and constants (listed as inputs, never as figures), {} numeric fields with no units entry.",
            s.figures.len(),
            s.series.len(),
            s.other_outputs.len(),
            s.inputs.len(),
            s.unlabelled_numeric_fields
        ),
    );
    l.advance(2.0);

    for c in &s.charts {
        let h = pdf_chart::height_mm(c);
        l.ensure(h + 8.0);
        l.text_at(
            MARGIN_MM,
            &ascii(&c.title),
            sans_bold(),
            9.5,
            rgb(brand::INK),
        );
        l.advance(2.0);
        pdf_chart::draw(l, MARGIN_MM, l.y_mm, CONTENT_W_MM, c, &ascii);
        l.advance(h);
        if c.stride > 1 {
            small(
                l,
                &format!(
                    "Every {}th of the result's {} samples is drawn.",
                    c.stride, c.points
                ),
            );
        }
        l.advance(2.0);
    }

    sub_heading(l, "Capability labels, from the run's report.json");
    if s.capabilities.is_empty() {
        small(l, &s.integrity);
        l.advance(2.0);
    } else {
        let cols = [
            Col {
                title: "Capability",
                w: 118.0,
                mono: false,
            },
            Col {
                title: "Label",
                w: 28.0,
                mono: false,
            },
            Col {
                title: "Role",
                w: 24.0,
                mono: false,
            },
        ];
        let rows: Vec<Vec<Cell>> = s
            .capabilities
            .iter()
            .map(|c| {
                vec![
                    cell(c.requirement.clone()),
                    bold(
                        c.label.clone(),
                        if c.label == "VALIDATED" { MET } else { FLAGGED },
                    ),
                    cell(c.role.clone()),
                ]
            })
            .collect();
        table(l, &cols, &rows, 8.0);
    }

    if !s.not_modelled.is_empty() {
        sub_heading(l, "Not modelled, from the run's report.json");
        for n in s.not_modelled.iter().take(MAX_SHEET_NOTES) {
            small(l, &format!("- {} [{}]", n.text, n.source));
            l.advance(1.0);
        }
        if s.not_modelled.len() > MAX_SHEET_NOTES {
            small(
                l,
                &format!(
                    "{} more are listed in dossier.json and in the run's report.",
                    s.not_modelled.len() - MAX_SHEET_NOTES
                ),
            );
        }
    }
}

fn closing(l: &mut Layout, d: &Dossier) {
    let b = &d.body;
    if let Some(t) = &b.trade {
        l.new_page();
        l.heading("Trade study");
        para(l, &t.title, 10.5);
        l.advance(1.0);
        para(
            l,
            &format!(
                "Metric {} ({} is better), read from each variant's own run.",
                t.metric,
                if t.lower_better { "lower" } else { "higher" }
            ),
            9.0,
        );
        l.advance(2.0);
        let cols = [
            Col {
                title: "Rank",
                w: 14.0,
                mono: false,
            },
            Col {
                title: "Variant",
                w: 46.0,
                mono: false,
            },
            Col {
                title: "Metric value",
                w: 30.0,
                mono: false,
            },
            Col {
                title: "Scenario hash",
                w: 80.0,
                mono: true,
            },
        ];
        let rows: Vec<Vec<Cell>> = t
            .variants
            .iter()
            .map(|v| {
                vec![
                    cell(v.rank.map(|r| r.to_string()).unwrap_or_else(|| "-".into())),
                    bold(v.label.clone(), brand::INK),
                    cell(match (&v.metric_value, &v.error) {
                        (Some(x), _) => num(*x),
                        (None, Some(e)) => format!("error: {e}"),
                        (None, None) => "-".into(),
                    }),
                    cell(
                        v.scenario_hash
                            .clone()
                            .unwrap_or_else(|| "not stamped by this kind".into()),
                    ),
                ]
            })
            .collect();
        table(l, &cols, &rows, 8.5);
        if let Some(r) = &t.recommendation {
            para(l, r, 9.5);
            l.advance(1.5);
        }
        if let (Some(o), Some(s)) = (&t.gate_outcome, &t.gate_summary) {
            small(l, &format!("The trade's own assurance gate: {o}. {s}"));
        }
        small(
            l,
            "The trade study is a Kshana Pro kind. Each variant is an open-engine scenario run unchanged; the trade itself is not reproduced by the free engine.",
        );
    }

    l.new_page();
    l.heading(&format!("Open items: {}", b.open_items.len()));
    if b.open_items.is_empty() {
        para(l, "None.", 9.5);
    } else {
        let cols = [
            Col {
                title: "Category",
                w: 30.0,
                mono: false,
            },
            Col {
                title: "Requirement",
                w: 24.0,
                mono: false,
            },
            Col {
                title: "Run",
                w: 26.0,
                mono: false,
            },
            Col {
                title: "What must be closed",
                w: 90.0,
                mono: false,
            },
        ];
        let rows: Vec<Vec<Cell>> = b
            .open_items
            .iter()
            .map(|o| {
                vec![
                    bold(o.category.clone(), brand::INK),
                    cell(o.requirement.clone().unwrap_or_else(|| "-".into())),
                    cell(o.run.clone().unwrap_or_else(|| "-".into())),
                    cell(o.text.clone()),
                ]
            })
            .collect();
        table(l, &cols, &rows, 8.0);
    }

    section(l, "Reproducibility record");
    small(l, &b.reproducibility_note);
    l.advance(3.0);
    for r in &b.reproducibility {
        l.ensure(40.0);
        l.text_at(
            MARGIN_MM,
            &ascii(&r.run),
            sans_bold(),
            10.0,
            rgb(brand::INK),
        );
        l.advance(5.0);
        key_values(
            l,
            &[
                ("Command", r.command.clone()),
                ("Run it in", format!("a copy of {}", r.directory)),
                ("Scenario SHA-256", r.scenario.sha256.clone()),
                ("Result SHA-256", r.result.sha256.clone()),
                (
                    "Report SHA-256",
                    r.report
                        .as_ref()
                        .map(|f| f.sha256.clone())
                        .unwrap_or_else(|| "no report".into()),
                ),
            ],
            true,
        );
        small(l, &r.statement);
        l.advance(3.0);
    }

    section(l, "Inputs");
    let cols = [
        Col {
            title: "File",
            w: 66.0,
            mono: false,
        },
        Col {
            title: "SHA-256",
            w: 104.0,
            mono: true,
        },
    ];
    let rows: Vec<Vec<Cell>> = b
        .identification
        .inputs
        .iter()
        .map(|f| vec![cell(f.path.clone()), cell(f.sha256.clone())])
        .collect();
    table(l, &cols, &rows, 8.0);

    section(l, "Sign-off");
    let cols = [
        Col {
            title: "Role",
            w: 44.0,
            mono: false,
        },
        Col {
            title: "Name",
            w: 46.0,
            mono: false,
        },
        Col {
            title: "Signature",
            w: 46.0,
            mono: false,
        },
        Col {
            title: "Date",
            w: 34.0,
            mono: false,
        },
    ];
    let rows: Vec<Vec<Cell>> = b
        .sign_off
        .iter()
        .map(|s| {
            vec![
                bold(s.role.clone(), brand::INK),
                cell(s.name.clone()),
                cell(""),
                cell(""),
            ]
        })
        .collect();
    // Room to sign: a taller row than the text needs.
    let lh = 8.0 * MM_PER_PT * 1.25;
    if l.y_mm - (8.0 + lh * 2.0) < BOTTOM_Y_MM {
        l.new_page();
    }
    table_header(l, &cols, 8.5);
    for row in &rows {
        if l.y_mm - 12.0 < BOTTOM_Y_MM {
            l.new_page();
            table_header(l, &cols, 8.5);
        }
        let mut x = MARGIN_MM;
        for (c, col) in row.iter().zip(&cols) {
            let font = if c.bold { sans_bold() } else { sans() };
            l.text_at(x, &ascii(&c.text), font, 8.5, rgb(c.colour));
            x += col.w;
        }
        l.advance(8.0);
        l.rule(rgb(GRID), 0.25);
        l.advance(5.0);
    }
    l.advance(2.0);

    section(l, "Limitations");
    for line in &b.limitations {
        small(l, &format!("- {line}"));
        l.advance(1.0);
    }
    l.advance(2.0);
    small(
        l,
        &format!(
            "Abbreviations: {}.",
            b.terms
                .iter()
                .map(|t| format!("{} - {}", t.term, t.meaning))
                .collect::<Vec<_>>()
                .join("; ")
        ),
    );
}

/// Count the ops of the page in progress plus the finished pages.
fn page_count(pages: &[Vec<Op>], current: &[Op]) -> usize {
    pages.len() + usize::from(!current.is_empty())
}

/// Render the dossier as a PDF.
pub fn render(d: &Dossier) -> Result<PdfOutput, String> {
    let mut l = Layout::new();
    cover(&mut l, d);
    summary_and_matrix(&mut l, d);
    for s in &d.body.sheets {
        sheet(&mut l, s);
    }
    closing(&mut l, d);

    let id = &d.body.identification;
    let reference = if id.reference.is_empty() {
        String::new()
    } else {
        format!("{} issue {}  |  ", id.reference, id.issue)
    };
    // No parentheses and ASCII only, so the phrase stays one literal in the stream.
    let footer = ascii(&format!(
        "Ashforde OU  |  {}  |  {reference}MODELLED engineering study - not a certification",
        id.classification
    ))
    .replace(['(', ')'], "");
    let pages_n = page_count(&l.pages, &l.ops).max(1);
    let doc_id = format!(
        "SHA-256 {}",
        &d.dossier_sha256[..d.dossier_sha256.len().min(16)]
    );
    let mark = crate::brand_pdf::PdfMark::new("Mission dossier", doc_id);
    let pages = l.finish(&footer, &mark);
    debug_assert_eq!(pages.len(), pages_n);
    let n = pages.len();
    let bytes = crate::brand_pdf::save(&ascii(&id.title), pages)?;
    Ok(PdfOutput { bytes, pages: n })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_with_six_significant_digits() {
        assert_eq!(num(21.3286), "21.3286");
        assert_eq!(num(1.8827043913863812), "1.8827");
        assert_eq!(num(7.5), "7.5");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(458.0), "458");
        assert_eq!(num(44.07518719422811), "44.0752");
        assert_eq!(num(-3.2328586592106117), "-3.23286");
        assert_eq!(num(5.263446411738822e-6), "5.26345e-6");
    }

    #[test]
    fn text_is_reduced_to_what_the_built_in_fonts_show() {
        assert_eq!(ascii("σ_y ≤ 3×10² — ok"), "sigma_y <= 3x10^2 - ok");
        assert_eq!(ascii("Ashforde OÜ"), "Ashforde OU");
        assert!(ascii("日本").chars().all(|c| c == '?'));
    }

    #[test]
    fn a_long_token_is_cut_to_the_cell() {
        let hash = "a".repeat(64);
        let lines = wrap_cell(&hash, 40.0, 7.0, 0.6);
        assert!(lines.len() > 1);
        assert_eq!(lines.concat(), hash);
    }
}
