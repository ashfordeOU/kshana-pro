// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # The Ashforde document style
//!
//! Every document Kshana Pro produces is an Ashforde document. This module holds the one
//! shared style for all of them, HTML and PDF alike:
//!
//! - the palette of the Ashforde light theme (the tokens of ashforde.org's light scheme:
//!   ground `#f6f4f0`, plate `#fbfaf7`, parchment `#ece6d8`, ink `#212121`, the navy of
//!   the seal `#1b2742`, and brass: `#c7aa66` for rules and fills, `#9a7e45` where brass
//!   carries text on the light ground);
//! - the type: Cormorant Garamond for display, EB Garamond for text, IBM Plex Mono for
//!   data, embedded in every HTML report as data URIs (no web-font fetch), all under the
//!   SIL Open Font License 1.1 (`assets/brand/fonts/OFL.txt`);
//! - the registered Ashforde seal, copied byte for byte from the brand folder into
//!   `assets/brand/` and baked in at compile time. It is drawn complete and unaltered:
//!   the light variant on the light ground, never recoloured, cropped or given effects;
//! - [`finish_html`], which gives any HTML report the shared furniture: the seal and
//!   ASHFORDE lockup at the head, the seal as a faint watermark behind the content, a
//!   footer naming Ashforde OÜ, the document id and the engine that produced the
//!   figures, page furniture for print, and, for a public sample, the SAMPLE mark.
//!
//! The PDF face of the same style is [`crate::brand_pdf`].
//!
//! ## Legacy colour names
//! The renderers were written against a dark ground and name colours by their old
//! role: `NAVY` for the page ground, `IVORY` for text on it. Those names are kept so the
//! renderers did not have to change, and now hold the light theme's value for the same
//! role: `NAVY` is the ground, `NAVY_PANEL` a panel, `IVORY` body text, `IVORY_LIGHT`
//! strong text and titles. New code uses the role names ([`PAPER`], [`INK`], ...).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

// --- Palette: the Ashforde light theme -----------------------------------------

/// The page ground (ashforde.org light `--bg`).
pub const PAPER: &str = "#f6f4f0";
/// A raised panel on the ground (ashforde.org light `--plate`).
pub const PLATE: &str = "#fbfaf7";
/// Parchment: the cover and banner ground.
pub const PARCHMENT: &str = "#ece6d8";
/// The navy of the seal's letter (seal-light.svg): titles and strong text.
pub const NAVY_INK: &str = "#1b2742";
/// Ink: body text on the light ground.
pub const INK: &str = "#212121";
/// Secondary text (ashforde.org light `--ink-2`).
pub const INK_2: &str = "#5f5a50";
/// A hairline between rows.
pub const LINE: &str = "#d9d3c7";
/// The brass of the seal: rules, borders and fills.
pub const BRASS_SEAL: &str = "#c7aa66";
/// Brass that carries text on the light ground (ashforde.org light `--brass`).
pub const BRASS: &str = "#9a7e45";
/// Deeper brass for small labels (ashforde.org light `--brass-2`).
pub const BRASS_MUTED: &str = "#7d6636";

/// Legacy name: the page ground (see the module note).
pub const NAVY: &str = PAPER;
/// Legacy name: a panel on the ground.
pub const NAVY_PANEL: &str = PLATE;
/// Legacy name: body text.
pub const IVORY: &str = INK;
/// Legacy name: titles and strong text.
pub const IVORY_LIGHT: &str = NAVY_INK;

/// Display type stack (Cormorant Garamond is embedded by [`finish_html`]).
pub const FONT_HEAD: &str = "'Cormorant Garamond', Georgia, 'Times New Roman', serif";
/// Text type stack (EB Garamond is embedded by [`finish_html`]).
pub const FONT_BODY: &str = "'EB Garamond', Georgia, 'Times New Roman', serif";
/// Data type stack (IBM Plex Mono is embedded by [`finish_html`]).
pub const FONT_MONO: &str = "'IBM Plex Mono', ui-monospace, Menlo, Consolas, monospace";

// --- The registered seal ------------------------------------------------------------

/// The registered seal for a light ground: brass rings, navy letter. Byte for byte the
/// brand folder's `seal-light.svg`.
pub const SEAL_LIGHT_SVG: &str = include_str!("../assets/brand/seal-light.svg");
/// The registered seal for a dark ground (`seal-dark.svg`).
pub const SEAL_DARK_SVG: &str = include_str!("../assets/brand/seal-dark.svg");
/// The registered single-colour brass seal (`seal-mono-brass.svg`).
pub const SEAL_MONO_BRASS_SVG: &str = include_str!("../assets/brand/seal-mono-brass.svg");
/// The registered single-colour ink seal (`seal-mono-ink.svg`).
pub const SEAL_MONO_INK_SVG: &str = include_str!("../assets/brand/seal-mono-ink.svg");

/// The wordmark. No trademark symbol.
pub const WORDMARK: &str = "ASHFORDE";
/// The legal entity.
pub const ENTITY: &str = "Ashforde OÜ";
/// The mark every page of a public sample carries.
pub const SAMPLE_TEXT: &str = "SAMPLE: illustrative programme on public data, not a real mission";
/// Opacity of the seal watermark: faint enough that text over it reads as on plain paper.
pub const WATERMARK_OPACITY: f32 = 0.07;

const FONT_CORMORANT: &[u8] =
    include_bytes!("../assets/brand/fonts/cormorant-garamond-600-normal-latin.woff2");
const FONT_EB_GARAMOND: &[u8] =
    include_bytes!("../assets/brand/fonts/eb-garamond-400-normal-latin.woff2");
const FONT_EB_GARAMOND_ITALIC: &[u8] =
    include_bytes!("../assets/brand/fonts/eb-garamond-400-italic-latin.woff2");
const FONT_PLEX_MONO: &[u8] =
    include_bytes!("../assets/brand/fonts/ibm-plex-mono-400-normal-latin.woff2");

/// Standard base64 (RFC 4648, with padding).
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The light seal as a `data:` URI (the file, base64-encoded, nothing changed).
pub fn seal_data_uri() -> &'static str {
    static URI: OnceLock<String> = OnceLock::new();
    URI.get_or_init(|| {
        format!(
            "data:image/svg+xml;base64,{}",
            base64(SEAL_LIGHT_SVG.as_bytes())
        )
    })
}

/// The seal as an `<img>` of `px` square.
pub fn seal_img_html(px: u32) -> String {
    format!(
        "<img src=\"{}\" alt=\"Ashforde seal\" width=\"{px}\" height=\"{px}\" style=\"display:block\"/>",
        seal_data_uri()
    )
}

// --- The SAMPLE switch ----------------------------------------------------------------

static SAMPLE: AtomicBool = AtomicBool::new(false);

/// Mark every document this process writes from now on as a public SAMPLE (the CLI's
/// `--sample`). Off by default: a customer deliverable carries the seal watermark only.
pub fn set_sample(on: bool) {
    SAMPLE.store(on, Ordering::SeqCst);
}

/// Whether documents are being marked as a public SAMPLE.
pub fn sample() -> bool {
    SAMPLE.load(Ordering::SeqCst)
}

/// "Produced with Kshana Pro on the open Kshana engine vX": the engine that produced the
/// figures, named on every document.
pub fn produced_with() -> String {
    format!(
        "Produced with Kshana Pro {} on the open Kshana engine v{}",
        crate::version(),
        crate::design::engine_version()
    )
}

// --- HTML ---------------------------------------------------------------------------

/// Minimal XML/HTML escaping for text interpolated into brand furniture.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Text inside a CSS string literal.
fn css_string(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

/// A content id for a document with no reference of its own: the first 16 hex digits of
/// the SHA-256 of the renderer's page before the furniture.
pub fn content_id(body: &str) -> String {
    use sha2::{Digest, Sha256};
    let hex = hex::encode(Sha256::digest(body.as_bytes()));
    format!("SHA-256 {}", &hex[..16])
}

/// What the furniture of one document names.
#[derive(Debug, Clone)]
pub struct DocMark {
    /// The kind of document ("Mission dossier", "Design study", ...).
    pub kind: String,
    /// The document's identity: a reference, or the hash that identifies its content.
    pub doc_id: String,
}

impl DocMark {
    pub fn new(kind: impl Into<String>, doc_id: impl Into<String>) -> Self {
        DocMark {
            kind: kind.into(),
            doc_id: doc_id.into(),
        }
    }
}

/// The embedded `@font-face` rules (computed once).
fn fonts_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(|| {
        let face = |family: &str, style: &str, weight: &str, bytes: &[u8]| {
            format!(
                "@font-face{{font-family:'{family}';font-style:{style};font-weight:{weight};\
                 font-display:swap;src:url(data:font/woff2;base64,{}) format('woff2')}}\n",
                base64(bytes)
            )
        };
        let mut s = String::new();
        s += &face("Cormorant Garamond", "normal", "300 700", FONT_CORMORANT);
        s += &face("EB Garamond", "normal", "400 800", FONT_EB_GARAMOND);
        s += &face("EB Garamond", "italic", "400 800", FONT_EB_GARAMOND_ITALIC);
        s += &face("IBM Plex Mono", "normal", "400", FONT_PLEX_MONO);
        s
    })
}

/// The shared stylesheet of an Ashforde HTML document. Appended after a renderer's own
/// styles, so it sets the palette and the type over them.
fn document_css(mark: &DocMark) -> String {
    let foot = css_string(&format!("{ENTITY}  ·  {}", mark.doc_id));
    let head = css_string(&format!("{WORDMARK}  ·  {}", mark.kind));
    format!(
        "{fonts}\
:root:root:root{{color-scheme:light;--bg:{paper};--paper:{paper};--surface:{paper};--panel:{plate};\
--card:{plate};--plate:{plate};--fg:{ink};--ink:{ink};--text:{ink};--muted:{ink2};--line:{line};\
--rule:{brass_seal};--grid:{line};--accent:{navy};--navy:{navy};--brass:{brass};--brassm:{brass_muted};\
--ivory:{parchment};--node:{parchment};--ash-display:{head_font};--ash-text:{body_font};--ash-mono:{mono_font}}}\n\
html body{{background:{paper};color:{ink};font-family:var(--ash-text)}}\n\
body :is(p,li,td,th,dt,dd,div,span,a,summary,figcaption,caption,label,small,strong,em,b,i,button,input,select):not(svg *){{font-family:var(--ash-text)!important}}\n\
body :is(h1,h2,h3,h4):not(svg *){{font-family:var(--ash-display)!important;font-weight:600;letter-spacing:.01em}}\n\
body :is(code,pre,kbd,samp,tt),body :is(code,pre,kbd,samp,tt) *{{font-family:var(--ash-mono)!important}}\n\
body :is(td,th,dd){{font-variant-numeric:lining-nums tabular-nums}}\n\
.ash-wm{{position:fixed;inset:0;z-index:-1;display:flex;align-items:center;justify-content:center;pointer-events:none}}\n\
.ash-wm img{{width:min(72vmin,560px);height:auto;opacity:{wm}}}\n\
.ash-head{{display:flex;align-items:center;gap:16px;max-width:1080px;margin:0 auto;padding:20px 16px 14px;border-bottom:1px solid {brass_seal}}}\n\
.ash-head img{{flex:none}}\n\
.ash-lock{{flex:1;min-width:0}}\n\
.ash-word{{font-family:var(--ash-display)!important;font-weight:600;font-size:26px;line-height:1;letter-spacing:.24em;color:{ink}}}\n\
.ash-ent{{font-family:var(--ash-text)!important;font-size:12px;letter-spacing:.18em;text-transform:uppercase;color:{brass};margin-top:6px}}\n\
.ash-id{{font-family:var(--ash-text)!important;font-size:11px;color:{ink2};text-align:right;max-width:46%;overflow-wrap:anywhere}}\n\
.ash-id code{{font-size:10.5px;color:{ink}}}\n\
.ash-foot{{display:flex;align-items:center;flex-wrap:wrap;gap:6px 14px;max-width:1080px;margin:40px auto 0;padding:12px 16px 28px;border-top:1px solid {brass_seal};font-size:12px;color:{ink2}}}\n\
.ash-foot img{{flex:none}}\n\
.ash-foot code{{font-size:11px;overflow-wrap:anywhere;color:{ink}}}\n\
.ash-sample{{max-width:1080px;margin:10px auto 0;padding:10px 14px;border:2px solid {sample_red};color:{sample_red};background:{plate};font-weight:700;letter-spacing:.03em;text-align:center}}\n\
.ash-sample-x{{position:fixed;inset:0;z-index:2147483000;pointer-events:none;display:flex;align-items:center;justify-content:center;overflow:hidden}}\n\
.ash-sample-x div{{transform:rotate(-32deg);text-align:center;color:{sample_red};opacity:.16;white-space:nowrap}}\n\
.ash-sample-x b{{display:block;font-family:var(--ash-display)!important;font-size:140px;line-height:1;letter-spacing:.12em}}\n\
.ash-sample-x span{{display:block;font-family:var(--ash-text)!important;font-size:22px;font-weight:700}}\n\
@media (max-width:640px){{.ash-head{{flex-wrap:wrap}}.ash-id{{max-width:none;text-align:left}}.ash-sample-x b{{font-size:72px}}.ash-sample-x span{{font-size:12px}}}}\n\
@media print{{html body{{background:#fff}}.ash-wm img{{width:150mm}}\
.ash-head,.ash-foot,.ash-sample{{-webkit-print-color-adjust:exact;print-color-adjust:exact}}\
@page{{size:A4;margin:16mm 14mm 18mm;\
@top-left{{content:\"{head}\";font:9pt 'EB Garamond',Georgia,serif;color:{brass}}}\
@bottom-left{{content:\"{foot}\";font:8pt 'EB Garamond',Georgia,serif;color:{ink2}}}\
@bottom-right{{content:\"page \" counter(page) \" of \" counter(pages);font:8pt 'EB Garamond',Georgia,serif;color:{ink2}}}}}}}\n",
        fonts = fonts_css(),
        paper = PAPER,
        plate = PLATE,
        parchment = PARCHMENT,
        navy = NAVY_INK,
        ink = INK,
        ink2 = INK_2,
        line = LINE,
        brass = BRASS,
        brass_muted = BRASS_MUTED,
        brass_seal = BRASS_SEAL,
        head_font = FONT_HEAD,
        body_font = FONT_BODY,
        mono_font = FONT_MONO,
        wm = WATERMARK_OPACITY,
        sample_red = SAMPLE_RED,
    )
}

/// The colour of the SAMPLE mark: a clear signal red, legible on the light ground.
pub const SAMPLE_RED: &str = "#a3261c";

/// The furniture at the top of the body: watermark, SAMPLE marks, the lockup bar.
fn head_html(mark: &DocMark, sample: bool) -> String {
    let mut h = String::new();
    h += &format!(
        "<div class=\"ash-wm\" aria-hidden=\"true\"><img src=\"{}\" alt=\"\"></div>",
        seal_data_uri()
    );
    if sample {
        h += &format!(
            "<div class=\"ash-sample-x\" aria-hidden=\"true\"><div><b>SAMPLE</b><span>{}</span></div></div>",
            xml(SAMPLE_TEXT)
        );
    }
    h += &format!(
        "<header class=\"ash-head\">{seal}<div class=\"ash-lock\"><div class=\"ash-word\">{WORDMARK}</div>\
         <div class=\"ash-ent\">{entity} &middot; {kind}</div></div>\
         <div class=\"ash-id\">Document<br><code>{id}</code></div></header>",
        seal = seal_img_html(56),
        entity = xml(ENTITY),
        kind = xml(&mark.kind),
        id = xml(&mark.doc_id),
    );
    if sample {
        h += &format!(
            "<div class=\"ash-sample\" role=\"note\">{}</div>",
            xml(SAMPLE_TEXT)
        );
    }
    h
}

/// The furniture at the end of the body.
fn foot_html(mark: &DocMark) -> String {
    format!(
        "<footer class=\"ash-foot\">{seal}<span>{entity}</span><span>Document <code>{id}</code></span>\
         <span>{produced}</span></footer>",
        seal = seal_img_html(22),
        entity = xml(ENTITY),
        id = xml(&mark.doc_id),
        produced = xml(&produced_with()),
    )
}

/// Switch a renderer's dark colour scheme off: an Ashforde document is a light,
/// print-first page. The media query is kept, made never to match.
fn light_only(html: &str) -> String {
    html.replace(
        "(prefers-color-scheme: dark)",
        "(prefers-color-scheme: dark) and (max-width: 0px)",
    )
    .replace(
        "(prefers-color-scheme:dark)",
        "(prefers-color-scheme:dark) and (max-width:0px)",
    )
}

/// Give a rendered HTML report the Ashforde furniture, marked SAMPLE when [`sample`] is
/// on. See [`finish_html_with`].
pub fn finish_html(html: &str, mark: &DocMark) -> String {
    finish_html_with(html, mark, sample())
}

/// Give a rendered HTML report the Ashforde furniture: the shared stylesheet (palette,
/// embedded type, print page furniture) before `</head>`, the watermark, lockup bar and,
/// with `sample`, the SAMPLE marks after `<body…>`, and the footer before `</body>`.
/// Nothing the renderer wrote is removed or reworded.
pub fn finish_html_with(html: &str, mark: &DocMark, sample: bool) -> String {
    let html = light_only(html);
    let style = format!(
        "<style id=\"ashforde-document\">{}</style>",
        document_css(mark)
    );
    let mut out = String::with_capacity(html.len() + style.len() + 4096);
    // Stylesheet: before </head>, else at the very start.
    let rest = match find_ci(&html, "</head>") {
        Some(i) => {
            out.push_str(&html[..i]);
            out.push_str(&style);
            &html[i..]
        }
        None => {
            out.push_str(&style);
            &html[..]
        }
    };
    // Head furniture: after the opening <body ...> tag.
    let rest = match find_ci(rest, "<body") {
        Some(i) => match rest[i..].find('>') {
            Some(j) => {
                let end = i + j + 1;
                out.push_str(&rest[..end]);
                out.push_str(&head_html(mark, sample));
                &rest[end..]
            }
            None => rest,
        },
        None => {
            out.push_str(&head_html(mark, sample));
            rest
        }
    };
    // Footer: before the last </body>.
    match rfind_ci(rest, "</body>") {
        Some(i) => {
            out.push_str(&rest[..i]);
            out.push_str(&foot_html(mark));
            out.push_str(&rest[i..]);
        }
        None => {
            out.push_str(rest);
            out.push_str(&foot_html(mark));
        }
    }
    out
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(needle)
}

fn rfind_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().rfind(needle)
}

/// The shared branded cover block (seal + Ashforde identity + study title +
/// sub-title). Both the consolidated report and every pack section reuse the same
/// markup so a study looks identical in hand; callers pass already-escaped or
/// plain `title` / `subtitle` text (escaping is applied here).
pub fn cover_html(title: &str, subtitle: &str) -> String {
    format!(
        "<header style=\"text-align:center;border-bottom:2px solid {brass_seal};\
         padding:36px 0 28px;margin-bottom:8px\">\
         <div style=\"display:flex;justify-content:center;margin-bottom:14px\">{seal}</div>\
         <div style=\"font-family:{head};font-size:15px;letter-spacing:.42em;\
         text-transform:uppercase;color:{brass};font-weight:700\">Ashforde</div>\
         <div style=\"font-family:{body};font-size:11px;letter-spacing:.22em;\
         text-transform:uppercase;color:{brass_muted};margin-top:2px\">\
         Ashforde O&Uuml; &middot; Quantitative PNT Resilience</div>\
         <h1 style=\"font-family:{head};font-size:34px;font-weight:700;color:{navy};\
         margin:22px 0 6px;line-height:1.15\">{title}</h1>\
         <div style=\"font-family:{body};font-size:16px;color:{ink};font-style:italic\">{subtitle}</div>\
         </header>",
        brass = BRASS,
        brass_seal = BRASS_SEAL,
        brass_muted = BRASS_MUTED,
        head = FONT_HEAD,
        body = FONT_BODY,
        ink = INK,
        navy = NAVY_INK,
        seal = seal_img_html(96),
        title = xml(title),
        subtitle = xml(subtitle),
    )
}

/// The shared, NON-OPTIONAL MODELLED banner with the literal "not a certification"
/// disclaimer. `detail` is the (already-plain) middle clause describing the
/// modelling scope; it is appended after the standing "MODELLED RESULTS" lead and
/// before the standing "not a certification / not ESA acceptance" tail, so the
/// honesty furniture is identical no matter which surface renders it.
pub fn modelled_banner_html(detail: &str) -> String {
    format!(
        "<div style=\"background:{brass};color:{ink};font-family:{body};font-weight:700;\
         text-align:center;padding:11px 16px;border-radius:4px;margin:18px 0;\
         letter-spacing:.04em\">\
         MODELLED RESULTS &mdash; {detail} This is an engineering study, \
         <strong>not a certification</strong> and not ESA acceptance or TRL maturation.</div>",
        brass = BRASS_SEAL,
        ink = INK,
        body = FONT_BODY,
        detail = xml(detail),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn the_seal_is_the_registered_file_unaltered() {
        // The five variants differ only in their two colours; the light one is used.
        assert!(SEAL_LIGHT_SVG
            .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 512 512\">"));
        assert!(SEAL_LIGHT_SVG.contains("fill=\"#1b2742\""));
        assert_eq!(SEAL_LIGHT_SVG.len(), 3979);
        assert_eq!(SEAL_DARK_SVG.len(), 3979);
    }

    #[test]
    fn finish_adds_furniture_without_removing_anything() {
        let html = "<!doctype html><html><head><title>t</title></head><body class=\"x\"><p>Figure 1.25 m</p></body></html>";
        let mark = DocMark::new("Test report", "abc123");
        let plain = finish_html_with(html, &mark, false);
        assert!(plain.contains("<p>Figure 1.25 m</p>"));
        assert!(plain.contains("class=\"ash-wm\""));
        assert!(plain.contains("Ashforde OÜ"));
        assert!(plain.contains("abc123"));
        assert!(!plain.contains(SAMPLE_TEXT));
        let marked = finish_html_with(html, &mark, true);
        assert!(marked.matches(SAMPLE_TEXT).count() >= 2);
        let body = marked.find("<body class=\"x\">").unwrap();
        assert!(marked.find("<header class=\"ash-head\"").unwrap() > body);
        assert!(marked.find("ash-foot").unwrap() < marked.find("</body>").unwrap());
    }
}
