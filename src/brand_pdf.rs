// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The Ashforde document style, PDF face
//!
//! The PDF half of [`crate::report_brand`]: the furniture every page of every Kshana Pro
//! PDF carries, drawn from `printpdf` primitives.
//!
//! - **The seal**, drawn as vectors from the registered file itself
//!   (`assets/brand/seal-light.svg`, the same bytes the HTML face embeds). The file is
//!   parsed at run time into its paths, circles and lines, and each is drawn with the
//!   file's own colours, stroke widths, joins and opacities, scaled uniformly. Nothing is
//!   recoloured, cropped or added.
//! - **The watermark**: that seal, centred on every page, behind the content, at
//!   [`crate::report_brand::WATERMARK_OPACITY`]. It is vector art, not text, so the text
//!   a reader or a PDF tool extracts from the page is unchanged by it.
//! - **The cover lockup** ([`cover_lockup`]): a parchment band with the ASHFORDE wordmark
//!   in Cormorant Garamond, "Ashforde OÜ", and the seal.
//! - **The running header** (every page after the cover): the seal, the wordmark and the
//!   kind of document, over a brass hairline.
//! - **The footer** (every page): "Ashforde OÜ" (with the seal on the cover), the renderer's own footer
//!   line (its honesty wording kept verbatim), the document id and "page N of M".
//! - **The SAMPLE mark** (only when [`crate::report_brand::sample`] is on): a diagonal
//!   SAMPLE across each page and the line
//!   [`crate::report_brand::SAMPLE_TEXT`] at its foot.
//!
//! ## Type
//! The wordmark and the entity name are set in Cormorant Garamond, embedded from
//! `assets/brand/fonts/` (OFL 1.1; subset to Latin-1). Body text stays in the PDF
//! standard serif (Times) and data in Courier: the renderers' honesty phrases must stay
//! plain searchable literals in the content stream, which the built-in fonts give and an
//! embedded Type 0 font (hex glyph ids) does not.
//!
//! ## Determinism
//! The embedded fonts and the transparency states are registered under fixed resource
//! names (printpdf would name them at random), so two renders still differ only in the
//! trailer `/ID`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use printpdf::{
    BuiltinFont, Color, ExtendedGraphicsState, ExtendedGraphicsStateId, FontId, Line,
    LineJoinStyle, LinePoint, Mm, Op, PaintMode, ParsedFont, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Polygon, PolygonRing, Pt, TextItem, TextMatrix, WindingOrder,
};
use sha2::{Digest, Sha256};

use crate::report_brand as brand;
use crate::report_pdf::{rgb, text_width_mm, MARGIN_MM, MM_PER_PT, PAGE_H_MM, PAGE_W_MM};

const CORMORANT_600: &[u8] =
    include_bytes!("../assets/brand/fonts/CormorantGaramond-600-subset.ttf");
const CORMORANT_700: &[u8] =
    include_bytes!("../assets/brand/fonts/CormorantGaramond-700-subset.ttf");
/// Noto Sans (OFL 1.1, `assets/fonts/`): the face for documents whose text the PDF
/// standard fonts cannot show (roadmap F8c). Latin, Greek and Cyrillic.
const NOTO_400: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
const NOTO_700: &[u8] = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
const FONT_ID_NOTO_400: &str = "FKshNotoSans400";
const FONT_ID_NOTO_700: &str = "FKshNotoSans700";
const FONT_ID_600: &str = "FAshCormorant600";
const FONT_ID_700: &str = "FAshCormorant700";
const GS_PREFIX: &str = "GSash";

/// Height of the cover's parchment band, from the top edge (mm).
pub const COVER_BAND_MM: f32 = 60.0;

/// What the furniture of one PDF names.
#[derive(Debug, Clone)]
pub struct PdfMark {
    /// The kind of document, shown in the running header ("Mission dossier").
    pub kind: String,
    /// The document's identity, shown in the footer.
    pub doc_id: String,
}

impl PdfMark {
    pub fn new(kind: impl Into<String>, doc_id: impl Into<String>) -> Self {
        PdfMark {
            kind: kind.into(),
            doc_id: doc_id.into(),
        }
    }
}

/// A content id for a document with no reference of its own: the first 16 hex digits of
/// the SHA-256 of everything drawn on its pages, before the furniture.
pub fn content_id(pages: &[Vec<Op>]) -> String {
    let mut h = Sha256::new();
    for p in pages {
        h.update(format!("{p:?}").as_bytes());
    }
    let hex = hex::encode(h.finalize());
    format!("SHA-256 {}", &hex[..16])
}

// --- Embedded display type ------------------------------------------------------------

/// Cormorant Garamond SemiBold, embedded.
pub fn display() -> PdfFontHandle {
    PdfFontHandle::External(FontId(FONT_ID_600.into()))
}

/// Cormorant Garamond Bold, embedded.
pub fn display_bold() -> PdfFontHandle {
    PdfFontHandle::External(FontId(FONT_ID_700.into()))
}

/// The parts of a TrueType file a PDF needs: the character map, advances and extents.
#[derive(Debug, Clone)]
struct Ttf {
    cmap: BTreeMap<u32, u16>,
    widths: BTreeMap<u16, u16>,
    upem: u16,
    ascent: i16,
    descent: i16,
    bbox: [i16; 4],
}

fn be16(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.get(o)?,
        *b.get(o + 1)?,
        *b.get(o + 2)?,
        *b.get(o + 3)?,
    ]))
}

fn parse_ttf(b: &[u8]) -> Option<Ttf> {
    let n = be16(b, 4)? as usize;
    let mut tables = BTreeMap::new();
    for i in 0..n {
        let r = 12 + 16 * i;
        let tag = std::str::from_utf8(b.get(r..r + 4)?).ok()?.to_string();
        tables.insert(tag, be32(b, r + 8)? as usize);
    }
    let head = *tables.get("head")?;
    let hhea = *tables.get("hhea")?;
    let hmtx = *tables.get("hmtx")?;
    let maxp = *tables.get("maxp")?;
    let cmap_t = *tables.get("cmap")?;
    let upem = be16(b, head + 18)?;
    let bbox = [
        be16(b, head + 36)? as i16,
        be16(b, head + 38)? as i16,
        be16(b, head + 40)? as i16,
        be16(b, head + 42)? as i16,
    ];
    let ascent = be16(b, hhea + 4)? as i16;
    let descent = be16(b, hhea + 6)? as i16;
    let n_hm = be16(b, hhea + 34)? as usize;
    let n_glyphs = be16(b, maxp + 4)? as usize;
    let mut widths = BTreeMap::new();
    let mut last = 0u16;
    for g in 0..n_glyphs {
        if g < n_hm {
            last = be16(b, hmtx + 4 * g)?;
        }
        widths.insert(g as u16, last);
    }
    // cmap: the Windows Unicode BMP subtable, format 4.
    let n_sub = be16(b, cmap_t + 2)? as usize;
    let mut sub = None;
    for i in 0..n_sub {
        let r = cmap_t + 4 + 8 * i;
        let (pid, eid) = (be16(b, r)?, be16(b, r + 2)?);
        let off = cmap_t + be32(b, r + 4)? as usize;
        if be16(b, off)? == 4 && ((pid == 3 && eid == 1) || pid == 0) {
            sub = Some(off);
            if pid == 3 {
                break;
            }
        }
    }
    let t = sub?;
    let seg2 = be16(b, t + 6)? as usize;
    let ends = t + 14;
    let starts = ends + seg2 + 2;
    let deltas = starts + seg2;
    let ranges = deltas + seg2;
    let mut cmap = BTreeMap::new();
    for s in 0..seg2 / 2 {
        let end = be16(b, ends + 2 * s)?;
        let start = be16(b, starts + 2 * s)?;
        let delta = be16(b, deltas + 2 * s)?;
        let ro = be16(b, ranges + 2 * s)?;
        if start == 0xFFFF {
            continue;
        }
        for c in start..=end {
            let gid = if ro == 0 {
                c.wrapping_add(delta)
            } else {
                let at = ranges + 2 * s + ro as usize + 2 * (c - start) as usize;
                match be16(b, at)? {
                    0 => 0,
                    g => g.wrapping_add(delta),
                }
            };
            if gid != 0 {
                cmap.insert(u32::from(c), gid);
            }
            if c == 0xFFFF {
                break;
            }
        }
    }
    Some(Ttf {
        cmap,
        widths,
        upem,
        ascent,
        descent,
        bbox,
    })
}

fn ttf(bold: bool) -> &'static Ttf {
    static T600: OnceLock<Ttf> = OnceLock::new();
    static T700: OnceLock<Ttf> = OnceLock::new();
    if bold {
        T700.get_or_init(|| parse_ttf(CORMORANT_700).expect("the embedded Cormorant 700 parses"))
    } else {
        T600.get_or_init(|| parse_ttf(CORMORANT_600).expect("the embedded Cormorant 600 parses"))
    }
}

fn noto_ttf(bold: bool) -> &'static Ttf {
    static N400: OnceLock<Ttf> = OnceLock::new();
    static N700: OnceLock<Ttf> = OnceLock::new();
    if bold {
        N700.get_or_init(|| parse_ttf(NOTO_700).expect("the embedded Noto Sans Bold parses"))
    } else {
        N400.get_or_init(|| parse_ttf(NOTO_400).expect("the embedded Noto Sans parses"))
    }
}

/// The Unicode face ([`NOTO_400`] or bold), for text the standard fonts cannot show.
pub fn unicode_font(bold: bool) -> PdfFontHandle {
    PdfFontHandle::External(FontId(
        if bold {
            FONT_ID_NOTO_700
        } else {
            FONT_ID_NOTO_400
        }
        .into(),
    ))
}

/// Whether the Unicode face has a glyph for `c`.
pub fn unicode_covers(c: char) -> bool {
    noto_ttf(false).cmap.contains_key(&(c as u32))
}

fn parsed_font(bytes: &[u8], bold: bool, name: &str) -> ParsedFont {
    parsed_font_from(ttf(bold), bytes, name)
}

fn parsed_font_from(t: &Ttf, bytes: &[u8], name: &str) -> ParsedFont {
    let mut f = ParsedFont::with_glyph_data(
        bytes.to_vec(),
        0,
        Some(name.to_string()),
        t.cmap.clone(),
        t.widths.clone(),
        t.upem,
        printpdf::font::FontMetrics {
            ascent: t.ascent,
            descent: t.descent,
        },
    );
    f.pdf_font_metrics.x_min = t.bbox[0];
    f.pdf_font_metrics.y_min = t.bbox[1];
    f.pdf_font_metrics.x_max = t.bbox[2];
    f.pdf_font_metrics.y_max = t.bbox[3];
    f
}

/// Width in mm of `text` in the embedded display face at `pt`, with `track_pt` of
/// letter-spacing after every character but the last.
pub fn display_width_mm(text: &str, pt: f32, bold: bool, track_pt: f32) -> f32 {
    let t = ttf(bold);
    let mut units = 0.0f32;
    let mut n = 0usize;
    for c in text.chars() {
        let gid = t.cmap.get(&(c as u32)).copied().unwrap_or(0);
        units += f32::from(t.widths.get(&gid).copied().unwrap_or(0));
        n += 1;
    }
    let pts = units / f32::from(t.upem) * pt + track_pt * n.saturating_sub(1) as f32;
    pts * MM_PER_PT
}

/// Register the embedded fonts and every transparency state the pages use, under fixed
/// names, then save. Every Kshana Pro PDF is saved through here.
pub fn save(title: &str, pages: Vec<PdfPage>) -> Result<Vec<u8>, String> {
    let mut doc = PdfDocument::new(title);
    doc.resources.fonts.map.insert(
        FontId(FONT_ID_600.into()),
        printpdf::font::PdfFont::new(parsed_font(
            CORMORANT_600,
            false,
            "CormorantGaramond-SemiBold",
        )),
    );
    doc.resources.fonts.map.insert(
        FontId(FONT_ID_700.into()),
        printpdf::font::PdfFont::new(parsed_font(CORMORANT_700, true, "CormorantGaramond-Bold")),
    );
    // The Unicode face is embedded only in a document that uses it, so every other
    // document keeps its bytes.
    let uses = |id: &str| {
        pages.iter().any(|p| {
            p.ops.iter().any(|op| {
                matches!(op, Op::SetFont { font: PdfFontHandle::External(FontId(f)), .. } if f == id)
            })
        })
    };
    for (id, bytes, bold, name) in [
        (FONT_ID_NOTO_400, NOTO_400, false, "NotoSans-Regular"),
        (FONT_ID_NOTO_700, NOTO_700, true, "NotoSans-Bold"),
    ] {
        if uses(id) {
            doc.resources.fonts.map.insert(
                FontId(id.into()),
                printpdf::font::PdfFont::new(parsed_font_from(noto_ttf(bold), bytes, name)),
            );
        }
    }
    for page in &pages {
        for op in &page.ops {
            if let Op::LoadGraphicsState { gs } = op {
                if let Some(permille) = gs.0.strip_prefix(GS_PREFIX) {
                    let a = permille.parse::<u32>().unwrap_or(1000) as f32 / 1000.0;
                    // printpdf 0.9.1 writes the fill alpha under the stroke key and the
                    // stroke alpha under the fill key; setting both to one value is right
                    // either way.
                    doc.resources.extgstates.map.insert(
                        gs.clone(),
                        ExtendedGraphicsState::default()
                            .with_current_fill_alpha(a)
                            .with_current_stroke_alpha(a),
                    );
                }
            }
        }
    }
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

fn gs(alpha: f32) -> ExtendedGraphicsStateId {
    ExtendedGraphicsStateId(format!(
        "{GS_PREFIX}{}",
        (alpha.clamp(0.0, 1.0) * 1000.0).round() as u32
    ))
}

// --- The seal, from the registered file ------------------------------------------------

#[derive(Debug, Clone)]
enum Paint {
    Stroke {
        colour: String,
        width: f32,
        round: bool,
    },
    Fill {
        colour: String,
    },
}

/// One drawable of the seal in SVG user units (y down), each subpath a list of
/// (x, y, is_control_point).
#[derive(Debug, Clone)]
struct Shape {
    subpaths: Vec<Vec<(f32, f32, bool)>>,
    closed: bool,
    paint: Paint,
    opacity: f32,
}

#[derive(Debug, Clone, Default)]
struct Style {
    stroke: Option<String>,
    fill: Option<String>,
    width: Option<f32>,
    round: bool,
    opacity: f32,
    /// (tx, ty, sx, sy): x' = tx + sx*x.
    transform: (f32, f32, f32, f32),
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    Some(tag[i..j].to_string())
}

fn numbers(s: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<f32>| {
        if !cur.is_empty() {
            if let Ok(v) = cur.parse() {
                out.push(v);
            }
            cur.clear();
        }
    };
    for c in s.chars() {
        match c {
            '0'..='9' | '.' => {
                if c == '.' && cur.contains('.') {
                    flush(&mut cur, &mut out);
                }
                cur.push(c);
            }
            '-' => {
                if !cur.ends_with('e') {
                    flush(&mut cur, &mut out);
                }
                cur.push(c);
            }
            'e' => cur.push(c),
            _ => flush(&mut cur, &mut out),
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn style_of(tag: &str, parent: &Style) -> Style {
    let mut s = parent.clone();
    if let Some(v) = attr(tag, "stroke") {
        s.stroke = Some(v);
    }
    if let Some(v) = attr(tag, "fill") {
        s.fill = Some(v);
    }
    if let Some(v) = attr(tag, "stroke-width").and_then(|v| v.parse().ok()) {
        s.width = Some(v);
    }
    if attr(tag, "stroke-linejoin").as_deref() == Some("round") {
        s.round = true;
    }
    if let Some(v) = attr(tag, "opacity").and_then(|v| v.parse::<f32>().ok()) {
        s.opacity *= v;
    }
    if let Some(t) = attr(tag, "transform") {
        let (mut tx, mut ty, mut sx, mut sy) = (0.0, 0.0, 1.0, 1.0);
        if let Some(i) = t.find("translate(") {
            let v = numbers(&t[i + 10..t[i..].find(')').map_or(t.len(), |j| i + j)]);
            tx = v.first().copied().unwrap_or(0.0);
            ty = v.get(1).copied().unwrap_or(0.0);
        }
        if let Some(i) = t.find("scale(") {
            let v = numbers(&t[i + 6..t[i..].find(')').map_or(t.len(), |j| i + j)]);
            sx = v.first().copied().unwrap_or(1.0);
            sy = v.get(1).copied().unwrap_or(sx);
        }
        // Compose with the parent: parent(child(x)).
        let (ptx, pty, psx, psy) = parent.transform;
        s.transform = (ptx + psx * tx, pty + psy * ty, psx * sx, psy * sy);
    }
    s
}

fn apply(t: (f32, f32, f32, f32), x: f32, y: f32) -> (f32, f32) {
    (t.0 + t.2 * x, t.1 + t.3 * y)
}

/// Parse SVG path data (M, L, H, V, Q, C, Z and their relative forms) into subpaths.
/// Subpaths of (x, y, is_control_point).
type Subpaths = Vec<Vec<(f32, f32, bool)>>;

fn path_points(d: &str, t: (f32, f32, f32, f32)) -> (Subpaths, bool) {
    let mut subpaths: Vec<Vec<(f32, f32, bool)>> = Vec::new();
    let mut closed = false;
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    let mut cmds: Vec<(char, Vec<f32>)> = Vec::new();
    let mut start = None;
    for (i, c) in d.char_indices() {
        if c.is_ascii_alphabetic() && c != 'e' {
            if let Some((cmd, s)) = start {
                cmds.push((cmd, numbers(&d[s..i])));
            }
            start = Some((c, i + 1));
        }
    }
    if let Some((cmd, s)) = start {
        cmds.push((cmd, numbers(&d[s..])));
    }
    let push = |sp: &mut Vec<Vec<(f32, f32, bool)>>, x: f32, y: f32, ctl: bool| {
        let (px, py) = apply(t, x, y);
        if let Some(last) = sp.last_mut() {
            last.push((px, py, ctl));
        }
    };
    for (cmd, v) in cmds {
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                for (k, p) in v.chunks(2).enumerate() {
                    if p.len() < 2 {
                        break;
                    }
                    let (bx, by) = if rel { (cx, cy) } else { (0.0, 0.0) };
                    cx = bx + p[0];
                    cy = by + p[1];
                    if k == 0 {
                        subpaths.push(Vec::new());
                    }
                    push(&mut subpaths, cx, cy, false);
                }
            }
            'L' => {
                for p in v.chunks(2) {
                    if p.len() < 2 {
                        break;
                    }
                    let (bx, by) = if rel { (cx, cy) } else { (0.0, 0.0) };
                    cx = bx + p[0];
                    cy = by + p[1];
                    push(&mut subpaths, cx, cy, false);
                }
            }
            'H' => {
                for x in v {
                    cx = if rel { cx + x } else { x };
                    push(&mut subpaths, cx, cy, false);
                }
            }
            'V' => {
                for y in v {
                    cy = if rel { cy + y } else { y };
                    push(&mut subpaths, cx, cy, false);
                }
            }
            'Q' => {
                for p in v.chunks(4) {
                    if p.len() < 4 {
                        break;
                    }
                    let (bx, by) = if rel { (cx, cy) } else { (0.0, 0.0) };
                    let (qx, qy, ex, ey) = (bx + p[0], by + p[1], bx + p[2], by + p[3]);
                    // The exact cubic form of the quadratic.
                    let c1 = (cx + 2.0 / 3.0 * (qx - cx), cy + 2.0 / 3.0 * (qy - cy));
                    let c2 = (ex + 2.0 / 3.0 * (qx - ex), ey + 2.0 / 3.0 * (qy - ey));
                    push(&mut subpaths, c1.0, c1.1, true);
                    push(&mut subpaths, c2.0, c2.1, true);
                    push(&mut subpaths, ex, ey, false);
                    cx = ex;
                    cy = ey;
                }
            }
            'C' => {
                for p in v.chunks(6) {
                    if p.len() < 6 {
                        break;
                    }
                    push(&mut subpaths, ox + p[0], oy + p[1], true);
                    push(&mut subpaths, ox + p[2], oy + p[3], true);
                    cx = ox + p[4];
                    cy = oy + p[5];
                    push(&mut subpaths, cx, cy, false);
                }
            }
            'Z' => {
                closed = true;
                if let Some(first) = subpaths.last().and_then(|s| s.first()).copied() {
                    // Back to the subpath start in user units (invert the transform).
                    cx = (first.0 - t.0) / t.2;
                    cy = (first.1 - t.1) / t.3;
                }
            }
            _ => {}
        }
    }
    (subpaths, closed)
}

fn circle_points(cx: f32, cy: f32, r: f32, t: (f32, f32, f32, f32)) -> Vec<(f32, f32, bool)> {
    const K: f32 = 0.552_284_8;
    let k = K * r;
    let raw = [
        (cx + r, cy, false),
        (cx + r, cy + k, true),
        (cx + k, cy + r, true),
        (cx, cy + r, false),
        (cx - k, cy + r, true),
        (cx - r, cy + k, true),
        (cx - r, cy, false),
        (cx - r, cy - k, true),
        (cx - k, cy - r, true),
        (cx, cy - r, false),
        (cx + k, cy - r, true),
        (cx + r, cy - k, true),
        (cx + r, cy, false),
    ];
    raw.iter()
        .map(|&(x, y, c)| {
            let (px, py) = apply(t, x, y);
            (px, py, c)
        })
        .collect()
}

/// Every drawable of the registered seal, parsed once from the file.
fn seal_shapes() -> &'static [Shape] {
    static SHAPES: OnceLock<Vec<Shape>> = OnceLock::new();
    SHAPES.get_or_init(|| parse_seal(brand::SEAL_LIGHT_SVG))
}

fn parse_seal(svg: &str) -> Vec<Shape> {
    let root = Style {
        opacity: 1.0,
        transform: (0.0, 0.0, 1.0, 1.0),
        ..Style::default()
    };
    let mut stack = vec![root];
    let mut shapes = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find('<') {
        let Some(j) = rest[i..].find('>') else { break };
        let tag = &rest[i..i + j + 1];
        rest = &rest[i + j + 1..];
        let parent = stack.last().cloned().unwrap_or_default();
        let name: String = tag[1..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '/')
            .collect();
        match name.as_str() {
            "g" => stack.push(style_of(tag, &parent)),
            "/g" => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            "path" | "circle" | "line" => {
                let s = style_of(tag, &parent);
                let (subpaths, closed) = match name.as_str() {
                    "path" => path_points(&attr(tag, "d").unwrap_or_default(), s.transform),
                    "circle" => {
                        let f = |k| {
                            attr(tag, k)
                                .and_then(|v| v.parse::<f32>().ok())
                                .unwrap_or(0.0)
                        };
                        (
                            vec![circle_points(f("cx"), f("cy"), f("r"), s.transform)],
                            true,
                        )
                    }
                    _ => {
                        let f = |k| {
                            attr(tag, k)
                                .and_then(|v| v.parse::<f32>().ok())
                                .unwrap_or(0.0)
                        };
                        let a = apply(s.transform, f("x1"), f("y1"));
                        let b = apply(s.transform, f("x2"), f("y2"));
                        (vec![vec![(a.0, a.1, false), (b.0, b.1, false)]], false)
                    }
                };
                let fill = s.fill.clone().filter(|f| f != "none");
                let stroke = s.stroke.clone().filter(|f| f != "none");
                if name != "line" {
                    if let Some(colour) = fill.clone() {
                        shapes.push(Shape {
                            subpaths: subpaths.clone(),
                            closed: true,
                            paint: Paint::Fill { colour },
                            opacity: s.opacity,
                        });
                    }
                }
                if let Some(colour) = stroke {
                    // Stroke width is in user units of the element's own coordinates.
                    let width = s.width.unwrap_or(1.0) * s.transform.2.abs();
                    shapes.push(Shape {
                        subpaths,
                        closed,
                        paint: Paint::Stroke {
                            colour,
                            width,
                            round: s.round,
                        },
                        opacity: s.opacity,
                    });
                }
            }
            _ => {}
        }
    }
    shapes
}

/// The registered seal as vector ops, centred at (`cx_mm`, `cy_mm`), `size_mm` across
/// the 512-unit view box, every element's opacity multiplied by `alpha`.
pub fn seal_ops(cx_mm: f32, cy_mm: f32, size_mm: f32, alpha: f32) -> Vec<Op> {
    let k = size_mm / 512.0;
    let to_page = |x: f32, y: f32| {
        Point::new(
            Mm(cx_mm - size_mm / 2.0 + x * k),
            Mm(cy_mm + size_mm / 2.0 - y * k),
        )
    };
    let mut ops = Vec::new();
    for s in seal_shapes() {
        ops.push(Op::SaveGraphicsState);
        let a = s.opacity * alpha;
        if a < 0.999 {
            ops.push(Op::LoadGraphicsState { gs: gs(a) });
        }
        let rings: Vec<PolygonRing> = s
            .subpaths
            .iter()
            .map(|sp| PolygonRing {
                points: sp
                    .iter()
                    .map(|&(x, y, c)| LinePoint {
                        p: to_page(x, y),
                        bezier: c,
                    })
                    .collect(),
            })
            .collect();
        match &s.paint {
            Paint::Fill { colour } => {
                ops.push(Op::SetFillColor { col: rgb(colour) });
                ops.push(Op::DrawPolygon {
                    polygon: Polygon {
                        rings,
                        mode: PaintMode::Fill,
                        winding_order: WindingOrder::NonZero,
                    },
                });
            }
            Paint::Stroke {
                colour,
                width,
                round,
            } => {
                ops.push(Op::SetOutlineColor { col: rgb(colour) });
                ops.push(Op::SetOutlineThickness {
                    pt: Pt(width * k / MM_PER_PT),
                });
                if *round {
                    ops.push(Op::SetLineJoinStyle {
                        join: LineJoinStyle::Round,
                    });
                }
                for ring in rings {
                    ops.push(Op::DrawLine {
                        line: Line {
                            points: ring.points,
                            is_closed: s.closed,
                        },
                    });
                }
            }
        }
        ops.push(Op::RestoreGraphicsState);
    }
    ops
}

// --- Text helpers ---------------------------------------------------------------------

fn text(ops: &mut Vec<Op>, x: f32, y: f32, s: &str, font: PdfFontHandle, pt: f32, col: Color) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::SetFont { font, size: Pt(pt) });
    ops.push(Op::SetFillColor { col });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(s.to_string())],
    });
    ops.push(Op::EndTextSection);
}

#[allow(clippy::too_many_arguments)] // a text run: position, face, size, tracking, colour
fn tracked(
    ops: &mut Vec<Op>,
    x: f32,
    y: f32,
    s: &str,
    bold: bool,
    pt: f32,
    track: f32,
    col: Color,
) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::SetFont {
        font: if bold { display_bold() } else { display() },
        size: Pt(pt),
    });
    ops.push(Op::SetCharacterSpacing { multiplier: track });
    ops.push(Op::SetFillColor { col });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(s.to_string())],
    });
    ops.push(Op::SetCharacterSpacing { multiplier: 0.0 });
    ops.push(Op::EndTextSection);
}

fn hline(ops: &mut Vec<Op>, x0: f32, x1: f32, y: f32, col: Color, pt: f32) {
    ops.push(Op::SetOutlineColor { col });
    ops.push(Op::SetOutlineThickness { pt: Pt(pt) });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint {
                    p: Point::new(Mm(x0), Mm(y)),
                    bezier: false,
                },
                LinePoint {
                    p: Point::new(Mm(x1), Mm(y)),
                    bezier: false,
                },
            ],
            is_closed: false,
        },
    });
}

fn times() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesRoman)
}
fn times_bold() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesBold)
}
fn times_italic() -> PdfFontHandle {
    PdfFontHandle::Builtin(BuiltinFont::TimesItalic)
}

// --- Cover ---------------------------------------------------------------------------

/// The cover lockup: a parchment band across the top of the page with the eyebrow line,
/// the ASHFORDE wordmark, "Ashforde OÜ" and the seal. Returns the ops; the caller's
/// content starts below [`COVER_BAND_MM`].
pub fn cover_lockup_ops(eyebrow: &str) -> Vec<Op> {
    let mut ops = Vec::new();
    let top = PAGE_H_MM;
    let bottom = PAGE_H_MM - COVER_BAND_MM;
    let corner = |x: f32, y: f32| LinePoint {
        p: Point::new(Mm(x), Mm(y)),
        bezier: false,
    };
    ops.push(Op::SetFillColor {
        col: rgb(brand::PARCHMENT),
    });
    ops.push(Op::DrawPolygon {
        polygon: Polygon {
            rings: vec![PolygonRing {
                points: vec![
                    corner(0.0, bottom),
                    corner(PAGE_W_MM, bottom),
                    corner(PAGE_W_MM, top),
                    corner(0.0, top),
                ],
            }],
            mode: PaintMode::Fill,
            winding_order: WindingOrder::NonZero,
        },
    });
    hline(
        &mut ops,
        0.0,
        PAGE_W_MM,
        bottom,
        rgb(brand::BRASS_SEAL),
        1.6,
    );
    // The eyebrow: a short brass rule and the tracked line, as on the Ashforde lockup.
    let ey = top - 17.0;
    hline(
        &mut ops,
        MARGIN_MM,
        MARGIN_MM + 12.0,
        ey + 1.1,
        rgb(brand::BRASS),
        0.6,
    );
    let eyebrow = eyebrow.to_uppercase();
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(MARGIN_MM + 16.0), Mm(ey)),
    });
    ops.push(Op::SetFont {
        font: times(),
        size: Pt(8.0),
    });
    ops.push(Op::SetCharacterSpacing { multiplier: 1.4 });
    ops.push(Op::SetFillColor {
        col: rgb(brand::INK_2),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(eyebrow)],
    });
    ops.push(Op::SetCharacterSpacing { multiplier: 0.0 });
    ops.push(Op::EndTextSection);
    // The wordmark.
    tracked(
        &mut ops,
        MARGIN_MM - 0.6,
        top - 33.0,
        brand::WORDMARK,
        false,
        34.0,
        7.5,
        rgb(brand::INK),
    );
    // The legal entity.
    tracked(
        &mut ops,
        MARGIN_MM,
        top - 44.0,
        brand::ENTITY,
        false,
        14.0,
        0.6,
        rgb(brand::BRASS),
    );
    // The seal, complete, at the right of the band.
    let size = 46.0;
    ops.extend(seal_ops(
        PAGE_W_MM - MARGIN_MM - size / 2.0 + 2.0,
        top - COVER_BAND_MM / 2.0,
        size,
        1.0,
    ));
    ops
}

// --- Page furniture ---------------------------------------------------------------------

/// Split a renderer's footer into the entity (drawn in the display face, with its Ü) and
/// the rest (kept verbatim, in the built-in serif, so its honesty wording stays a plain
/// literal).
fn footer_rest(footer: &str) -> &str {
    footer
        .strip_prefix("Ashforde OU")
        .map(|r| r.trim_start_matches([' ', '|']))
        .unwrap_or(footer)
}

/// Dress finished pages: the watermark under each page's content, the running header on
/// every page after the first, the footer and page number on every page, and the SAMPLE
/// mark when `sample` is on.
pub fn dress(pages: Vec<Vec<Op>>, footer: &str, mark: &PdfMark, sample: bool) -> Vec<PdfPage> {
    let n = pages.len();
    pages
        .into_iter()
        .enumerate()
        .map(|(i, content)| {
            let mut ops = Vec::with_capacity(content.len() + 400);
            // 1. The watermark, first, so all content paints over it.
            ops.extend(seal_ops(
                PAGE_W_MM / 2.0,
                PAGE_H_MM / 2.0 - 6.0,
                150.0,
                brand::WATERMARK_OPACITY,
            ));
            // 2. The content.
            ops.extend(content);
            // 3. Running header after the cover.
            if i > 0 {
                let y = PAGE_H_MM - 12.6;
                ops.extend(seal_ops(MARGIN_MM + 3.5, y + 1.6, 7.5, 1.0));
                tracked(
                    &mut ops,
                    MARGIN_MM + 9.5,
                    y,
                    brand::WORDMARK,
                    false,
                    11.0,
                    2.4,
                    rgb(brand::INK),
                );
                let kind = crate::mission::pdf::ascii(&mark.kind);
                let w = text_width_mm(kind.chars().count(), 8.5) * 0.9;
                text(
                    &mut ops,
                    PAGE_W_MM - MARGIN_MM - w,
                    y,
                    &kind,
                    times_italic(),
                    8.5,
                    rgb(brand::INK_2),
                );
                hline(
                    &mut ops,
                    MARGIN_MM,
                    PAGE_W_MM - MARGIN_MM,
                    y - 3.2,
                    rgb(brand::BRASS_SEAL),
                    0.5,
                );
            }
            // 4. Footer: rule, seal, entity, the renderer's line, the id, the page.
            let fy = MARGIN_MM - 4.0;
            hline(
                &mut ops,
                MARGIN_MM,
                PAGE_W_MM - MARGIN_MM,
                fy + 4.0,
                rgb(brand::BRASS_SEAL),
                0.5,
            );
            // The seal sits in the footer of the cover only; every later page carries it in
            // the running header (and each seal adds about 11 kB of path data to a page).
            let ex = if i == 0 {
                ops.extend(seal_ops(MARGIN_MM + 2.6, fy + 0.9, 5.2, 1.0));
                MARGIN_MM + 6.8
            } else {
                MARGIN_MM
            };
            tracked(
                &mut ops,
                ex,
                fy,
                brand::ENTITY,
                false,
                9.5,
                0.2,
                rgb(brand::INK),
            );
            let rx = ex + display_width_mm(brand::ENTITY, 9.5, false, 0.2) + 2.0;
            let label = format!(
                "|  {}   |   {}   |   page {} of {}",
                footer_rest(footer),
                crate::mission::pdf::ascii(&mark.doc_id),
                i + 1,
                n
            );
            text(&mut ops, rx, fy, &label, times(), 7.2, rgb(brand::INK_2));
            // 5. SAMPLE, over everything, translucent.
            if sample {
                ops.extend(sample_ops());
            }
            PdfPage::new(Mm(PAGE_W_MM), Mm(PAGE_H_MM), ops)
        })
        .collect()
}

/// The SAMPLE mark: the word across the page on the diagonal, the full sentence under it,
/// and the sentence again as a line at the foot of the page.
fn sample_ops() -> Vec<Op> {
    let mut ops = Vec::new();
    let red = rgb(brand::SAMPLE_RED);
    let angle = 52.0f32.to_radians();
    let (c, s) = (angle.cos(), angle.sin());
    let (cx, cy) = (PAGE_W_MM / 2.0, PAGE_H_MM / 2.0);
    let word_pt = 110.0;
    let track = 10.0;
    let w = display_width_mm("SAMPLE", word_pt, true, track);
    let cap = 0.63 * word_pt * MM_PER_PT;
    let place = |along: f32, across: f32| {
        // A point `along` the diagonal and `across` it (mm), from the page centre.
        (cx + along * c - across * s, cy + along * s + across * c)
    };
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::LoadGraphicsState { gs: gs(0.16) });
    let (x, y) = place(-w / 2.0, -cap / 2.0);
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Raw([c, s, -s, c, x / MM_PER_PT, y / MM_PER_PT]),
    });
    ops.push(Op::SetFont {
        font: display_bold(),
        size: Pt(word_pt),
    });
    ops.push(Op::SetCharacterSpacing { multiplier: track });
    ops.push(Op::SetFillColor { col: red.clone() });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text("SAMPLE".into())],
    });
    ops.push(Op::SetCharacterSpacing { multiplier: 0.0 });
    ops.push(Op::EndTextSection);
    ops.push(Op::RestoreGraphicsState);
    // The sentence along the diagonal, under the word.
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::LoadGraphicsState { gs: gs(0.45) });
    let line_pt = 12.0;
    let lw = text_width_mm(brand::SAMPLE_TEXT.chars().count(), line_pt) * 0.92;
    let (x, y) = place(-lw / 2.0, -cap / 2.0 - 9.0);
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextMatrix {
        matrix: TextMatrix::Raw([c, s, -s, c, x / MM_PER_PT, y / MM_PER_PT]),
    });
    ops.push(Op::SetFont {
        font: times_bold(),
        size: Pt(line_pt),
    });
    ops.push(Op::SetFillColor { col: red.clone() });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(brand::SAMPLE_TEXT.into())],
    });
    ops.push(Op::EndTextSection);
    ops.push(Op::RestoreGraphicsState);
    // The banner line at the foot of the page.
    text(
        &mut ops,
        MARGIN_MM,
        MARGIN_MM - 9.5,
        brand::SAMPLE_TEXT,
        times_bold(),
        8.5,
        red,
    );
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seal_parses_into_every_element_of_the_file() {
        let shapes = seal_shapes();
        // 1 star path + 2 rings + 24 rays + 12 petals (strokes) + the letter's fill.
        let strokes = shapes
            .iter()
            .filter(|s| matches!(s.paint, Paint::Stroke { .. }))
            .count();
        let fills = shapes
            .iter()
            .filter(|s| matches!(s.paint, Paint::Fill { .. }))
            .count();
        assert_eq!(strokes, 1 + 1 + 24 + 1 + 12);
        assert_eq!(fills, 1);
        // The letter keeps the file's navy and its two subpaths.
        let letter = shapes
            .iter()
            .find(|s| matches!(s.paint, Paint::Fill { .. }))
            .unwrap();
        assert!(matches!(&letter.paint, Paint::Fill { colour } if colour == "#1b2742"));
        assert_eq!(letter.subpaths.len(), 2);
        // Every point lies inside the 512-unit view box.
        for s in shapes {
            for sp in &s.subpaths {
                for &(x, y, _) in sp {
                    assert!((0.0..=512.0).contains(&x) && (0.0..=512.0).contains(&y));
                }
            }
        }
    }

    #[test]
    fn the_embedded_display_face_maps_the_wordmark_and_the_entity() {
        for bold in [false, true] {
            let t = ttf(bold);
            for ch in "ASHFORDE Ashforde OÜ SAMPLE".chars() {
                assert!(t.cmap.contains_key(&(ch as u32)), "{ch} missing");
            }
            assert!(display_width_mm("ASHFORDE", 30.0, bold, 0.0) > 40.0);
        }
    }
}
