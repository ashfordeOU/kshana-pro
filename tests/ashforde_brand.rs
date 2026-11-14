// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Every Kshana Pro document is an Ashforde document.
//!
//! Guards on the shared Ashforde document style (`report_brand`, `brand_pdf`):
//! the PDF opens with a PDF library and has pages; the registered seal is drawn in
//! full as the watermark on every page, behind the content; the display face is
//! embedded; every page has its footer and page number; the HTML face embeds the
//! registered seal file, names Ashforde OÜ and the engine, fetches nothing, and keeps
//! the renderer's own text; and without `--sample` no SAMPLE mark appears. The SAMPLE
//! mark itself is guarded in `tests/ashforde_sample.rs` (its own process, because the
//! switch is process-wide).

use kshana_pro::report::{build_report, ConsolidatedReport, StudyCover, StudyProvenance};
use kshana_pro::report_brand::{self as brand, DocMark};
use kshana_pro::report_pdf::to_pdf;
use lopdf::{Document, Object};

fn report() -> ConsolidatedReport {
    let scenario = "seed = 42\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 7200.0\n\
        [gnss]\nwindows = [ { t0 = 0.0, t1 = 600.0, state = \"nominal\" }, { t0 = 600.0, t1 = 7200.0, state = \"denied\" } ]\n\
        [clock_quantum]\nid = \"optical-sr-lattice\"\nprovenance = \"demo\"\ny0 = 5.0e-17\nq_wf = 1.0e-30\nq_rw = 0.0\n\
        [clock_classical]\nid = \"csac-sa45s\"\nprovenance = \"demo\"\ny0 = 5.0e-10\nq_wf = 9.0e-20\nq_rw = 0.0\n";
    build_report("Quantum Clock Resilience Study", scenario, "quantum")
        .unwrap()
        .with_cover(StudyCover {
            subtitle: Some("Performance & Resilience Study".into()),
            executive_summary: Some("A bespoke executive summary paragraph.".into()),
        })
        .with_provenance(StudyProvenance {
            study_id: Some("KP-2026-0007".into()),
            classification: Some("Restricted".into()),
            ..Default::default()
        })
}

fn page_streams(doc: &Document) -> Vec<Vec<u8>> {
    doc.get_pages()
        .values()
        .map(|id| doc.get_page_content(*id).expect("page content"))
        .collect()
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn count(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len()).filter(|w| *w == needle).count()
}

/// The watermark is the whole seal: its elements carry the file's own opacities
/// (1.0, 0.85, 0.8, 0.6), each multiplied by the watermark's 0.07.
const WATERMARK_STATES: [&[u8]; 4] = [
    b"/GSash70 gs",
    b"/GSash60 gs",
    b"/GSash56 gs",
    b"/GSash42 gs",
];

#[test]
fn the_pdf_opens_and_every_page_carries_the_watermark_footer_and_number() {
    let bytes = to_pdf(&report()).unwrap();
    let doc = Document::load_mem(&bytes).expect("a PDF library opens the file");
    let pages = page_streams(&doc);
    let n = pages.len();
    assert!(
        n >= 2,
        "the report has a cover and at least one more page, got {n}"
    );
    for (i, s) in pages.iter().enumerate() {
        for state in WATERMARK_STATES {
            assert!(
                contains(s, state),
                "page {} lacks watermark state {}",
                i + 1,
                String::from_utf8_lossy(state)
            );
        }
        // 39 drawables in the seal; the watermark loads a transparency state for each.
        let loads: usize = WATERMARK_STATES.iter().map(|st| count(s, st)).sum();
        assert!(
            loads >= 39,
            "page {}: {loads} watermark elements, want the whole seal (39)",
            i + 1
        );
        // Behind the content: the watermark is drawn before the page's first text.
        let wm = s.windows(11).position(|w| w == b"/GSash70 gs").unwrap();
        let first_text = s.windows(2).position(|w| w == b"BT").unwrap();
        assert!(
            wm < first_text,
            "page {}: the watermark must be under the content",
            i + 1
        );
        let number = format!("page {} of {n}", i + 1);
        assert!(
            contains(s, number.as_bytes()),
            "page {} lacks '{number}'",
            i + 1
        );
        // The entity name in the embedded display face, on every page.
        assert!(
            contains(s, b"/FAshCormorant600"),
            "page {} lacks the display face",
            i + 1
        );
        assert!(!contains(s, b"SAMPLE"), "no SAMPLE mark without --sample");
        // The honesty wording of the footer stays a plain literal.
        assert!(contains(
            s,
            b"MODELLED engineering study - not a certification"
        ));
    }
    // The ExtGState really is a transparency of 0.07.
    let gs = doc
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find_map(|d| d.get(b"GSash70").ok().cloned())
        .expect("the watermark's graphics state is registered");
    let gs = match gs {
        Object::Reference(r) => doc.get_object(r).unwrap().clone(),
        o => o,
    };
    let d = gs.as_dict().unwrap();
    let ca = d.get(b"CA").unwrap().as_float().unwrap();
    assert!((ca - 0.07).abs() < 1e-6, "watermark opacity {ca}");
}

#[test]
fn the_display_face_is_embedded_under_a_fixed_name() {
    let bytes = to_pdf(&report()).unwrap();
    assert!(contains(&bytes, b"CormorantGaramond-SemiBold"));
    assert!(contains(&bytes, b"/FontFile2"));
    assert!(contains(&bytes, b"/ToUnicode"));
    // Only the trailer /ID may differ between two renders.
    let mask = |b: &[u8]| -> Vec<u8> {
        let i = b.windows(4).rposition(|w| w == b"/ID[").unwrap();
        let j = i + b[i..].iter().position(|&c| c == b']').unwrap();
        [&b[..i], &b[j..]].concat()
    };
    assert_eq!(mask(&bytes), mask(&to_pdf(&report()).unwrap()));
}

/// The text a reader extracts from the built-in-font text of the pages: every literal
/// string shown with `Tj`, in order. (lopdf's own extractor gives up on the whole
/// document at the display face's ToUnicode map, which poppler and MuPDF read fine; the
/// body text is in the built-in fonts, so the literals are the page text.)
fn literals(stream: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < stream.len() {
        if stream[i] == b'(' {
            let mut s = Vec::new();
            let mut depth = 1;
            i += 1;
            while i < stream.len() && depth > 0 {
                match stream[i] {
                    b'\\' if i + 1 < stream.len() => {
                        s.push(stream[i + 1]);
                        i += 1;
                    }
                    b'(' => {
                        depth += 1;
                        s.push(b'(');
                    }
                    b')' => {
                        depth -= 1;
                        if depth > 0 {
                            s.push(b')');
                        }
                    }
                    c => s.push(c),
                }
                i += 1;
            }
            out.push(String::from_utf8_lossy(&s).into_owned());
        } else {
            i += 1;
        }
    }
    out
}

#[test]
fn the_extracted_text_keeps_the_report_and_adds_only_branding() {
    let r = report();
    let bytes = to_pdf(&r).unwrap();
    let doc = Document::load_mem(&bytes).unwrap();
    let text: Vec<String> = page_streams(&doc)
        .iter()
        .flat_map(|s| literals(s))
        .collect();
    let all = text.join("\n");
    for must in [
        "Quantum Clock Resilience Study",
        "Performance & Resilience Study",
        "MODELLED RESULTS - simulation figures, not flight/in-orbit validated.",
        "Executive Summary",
        "A bespoke executive summary paragraph.",
        "Restricted",
    ] {
        assert!(all.contains(must), "extracted text lost {must:?}");
    }
    // What the furniture adds in the built-in fonts is only the footer line and the
    // running header's document kind.
    assert!(all.contains("Study report"));
    assert!(!all.contains("SAMPLE"));
}

#[test]
fn the_html_face_embeds_the_seal_names_the_engine_and_fetches_nothing() {
    let html = report().to_html();
    let seal = format!(
        "data:image/svg+xml;base64,{}",
        brand::base64(brand::SEAL_LIGHT_SVG.as_bytes())
    );
    assert!(
        html.contains(&seal),
        "the registered seal file, embedded unaltered"
    );
    assert!(html.contains("class=\"ash-wm\""), "the watermark");
    assert!(html.contains("Ashforde OÜ"));
    assert!(html.contains("ASHFORDE"));
    assert!(html.contains("Produced with Kshana Pro"));
    assert!(html.contains("on the open Kshana engine v"));
    assert!(html.contains("font/woff2;base64,"), "the type is embedded");
    assert!(
        !html.contains(brand::SAMPLE_TEXT),
        "no SAMPLE mark without --sample"
    );
    for banned in [
        "src=\"http",
        "href=\"http",
        "url(http",
        "@import",
        "<link ",
        "src='http",
    ] {
        assert!(
            !html.contains(banned),
            "the report must be self-contained: found {banned}"
        );
    }
    // The honesty label is kept exactly.
    assert!(html
        .contains("MODELLED RESULTS &mdash; simulation figures, not flight/in-orbit validated."));
}

#[test]
fn finishing_keeps_every_byte_the_renderer_wrote() {
    let body = "<!doctype html><html><head><title>T</title><style>body{color:red}</style></head>\
        <body><main><h1>Title</h1><p>Figure 1.234567 m, hash 9f86d081884c7d65</p><p>MODELLED</p></main></body></html>";
    for sample in [false, true] {
        let out = brand::finish_html_with(body, &DocMark::new("Kind", "id-1"), sample);
        // Remove the three inserted pieces and the original is back.
        let s = out.find("<style id=\"ashforde-document\">").unwrap();
        let e = s + out[s..].find("</style>").unwrap() + "</style>".len();
        let out = format!("{}{}", &out[..s], &out[e..]);
        let b = out.find("<body>").unwrap() + "<body>".len();
        let m = out.find("<main>").unwrap();
        let out = format!("{}{}", &out[..b], &out[m..]);
        let f = out.find("<footer class=\"ash-foot\">").unwrap();
        let g = out.rfind("</body>").unwrap();
        let out = format!("{}{}", &out[..f], &out[g..]);
        assert_eq!(out, body);
    }
}
