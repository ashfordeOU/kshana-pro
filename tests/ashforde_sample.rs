// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The SAMPLE mark (`--sample`) appears on every page of a public sample, and only then.
//!
//! One test in its own binary: the switch is process-wide, so no other test may run
//! beside it.

use kshana_pro::report::build_report;
use kshana_pro::report_brand as brand;
use kshana_pro::report_pdf::to_pdf;
use lopdf::Document;

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn the_sample_mark_is_on_every_page_only_with_sample() {
    let scenario = "seed = 1\nthreshold_ns = 20.0\n[time]\nstep_s = 10.0\nduration_s = 3600.0\n\
        [gnss]\nwindows = [ { t0 = 0.0, t1 = 3600.0, state = \"denied\" } ]\n\
        [clock_quantum]\nid = \"q\"\nprovenance = \"demo\"\ny0 = 5.0e-17\nq_wf = 1.0e-30\nq_rw = 0.0\n\
        [clock_classical]\nid = \"c\"\nprovenance = \"demo\"\ny0 = 5.0e-10\nq_wf = 9.0e-20\nq_rw = 0.0\n";
    let r = build_report("Sample check", scenario, "quantum").unwrap();
    let literal = format!("({}) Tj", brand::SAMPLE_TEXT);

    let pages = |bytes: &[u8]| -> Vec<Vec<u8>> {
        let doc = Document::load_mem(bytes).unwrap();
        doc.get_pages()
            .values()
            .map(|id| doc.get_page_content(*id).unwrap())
            .collect()
    };

    brand::set_sample(false);
    let plain_pdf = to_pdf(&r).unwrap();
    let plain_html = r.to_html();
    for p in pages(&plain_pdf) {
        assert!(!contains(&p, b"SAMPLE"));
    }
    assert!(!plain_html.contains(brand::SAMPLE_TEXT));

    brand::set_sample(true);
    let sample_pdf = to_pdf(&r).unwrap();
    let sample_html = r.to_html();
    brand::set_sample(false);
    let sp = pages(&sample_pdf);
    assert!(!sp.is_empty());
    for (i, p) in sp.iter().enumerate() {
        // The sentence at the foot and along the diagonal, and the diagonal word.
        let n = p
            .windows(literal.len())
            .filter(|w| *w == literal.as_bytes())
            .count();
        assert!(n >= 2, "page {} carries the sentence {n} times", i + 1);
        assert!(
            contains(p, b"/FAshCormorant700"),
            "page {}: the diagonal SAMPLE",
            i + 1
        );
    }
    assert!(sample_html.matches(brand::SAMPLE_TEXT).count() >= 2);
    assert!(sample_html.contains("class=\"ash-sample-x\""));
    // The mark is added; nothing else moves: the page count is the same.
    assert_eq!(sp.len(), pages(&plain_pdf).len());
}
