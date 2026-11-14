// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Text as the branded PDFs can show it: the built-in standard fonts take ASCII with the
//! common typographic characters written out; the Unicode face takes the text as it is.
//! Shared by every branded PDF (the study PDF, the brand pages and the mission dossier).

/// Text as the document can show it. With the standard fonts: [`to_builtin`]. With the
/// Unicode face (a dossier whose text needs it, F8c): the text as it is, white space
/// made single-line.
pub fn ascii(s: &str) -> String {
    if crate::report_pdf::unicode_face() {
        return s
            .chars()
            .map(|c| {
                if matches!(c, '\t' | '\n' | '\r') {
                    ' '
                } else {
                    c
                }
            })
            .collect();
    }
    to_builtin(s)
}

/// Whether some text of the dossier is lost by [`to_builtin`] but shown by the Unicode
/// face: then the PDF is set in that face.
pub fn needs_unicode(text: &str) -> bool {
    text.chars()
        .any(|c| to_builtin(&c.to_string()) == "?" && crate::brand_pdf::unicode_covers(c))
}

/// Text the built-in fonts can show: ASCII, with the common typographic characters of
/// the engine's notes written out. Anything else becomes `?`.
pub fn to_builtin(s: &str) -> String {
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
