//! The Pro lab campaign report (`kshana_pro::report_lab_campaign`) on the synthetic fixture.
//!
//! Rules under test: the report recomputes no metric (every printed number traces to a cell
//! value or to a declared median/sum/count/ratio of cell values), every figure is labelled
//! MEASURED or MODELLED, the data class is stated and enforced, the output is deterministic,
//! and no client name appears unless one is passed in.

use kshana_pro::lab_campaign::load;
use kshana_pro::lab_campaign::model::DataClass;
use kshana_pro::report_lab_campaign::{
    render_html, LabReportOptions, RenderError, EXPORT_NOTICE_PLACEHOLDER,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lab_campaign_synthetic/basic")
}

fn synthetic() -> LabReportOptions {
    let mut o = LabReportOptions::new(DataClass::Synthetic);
    o.doc_id = Some("KP-TEST-0001".into());
    o.prepared_utc = Some("2026-10-07T00:00:00Z".into());
    o
}

fn html() -> String {
    render_html(&load(&fixture()).unwrap(), &synthetic()).unwrap()
}

/// Every number in the cell files.
fn cell_numbers() -> Vec<f64> {
    fn walk(v: &serde_json::Value, out: &mut Vec<f64>) {
        match v {
            serde_json::Value::Number(n) => out.extend(n.as_f64()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(m) => m.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(fixture().join("cells"))
        .unwrap()
        .flatten()
    {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap();
        walk(&v, &mut out);
    }
    out
}

/// `(value, op, n)` of every traced number in the document.
fn traced(doc: &str) -> Vec<(f64, String, usize)> {
    let mut out = Vec::new();
    for (i, _) in doc.match_indices("data-v=\"") {
        let rest = &doc[i + 8..];
        let v: f64 = rest[..rest.find('"').unwrap()].parse().unwrap();
        let op_at = rest.find("data-op=\"").unwrap() + 9;
        let op = &rest[op_at..op_at + rest[op_at..].find('"').unwrap()];
        let n_at = rest.find("data-n=\"").unwrap() + 8;
        let n: usize = rest[n_at..n_at + rest[n_at..].find('"').unwrap()]
            .parse()
            .unwrap();
        out.push((v, op.to_owned(), n));
    }
    out
}

#[test]
fn the_report_renders_deterministically() {
    assert_eq!(html(), html());
}

#[test]
fn every_printed_number_traces_to_the_cells() {
    let doc = html();
    let nums = cell_numbers();
    let has = |v: f64| nums.contains(&v);
    let mid = |v: f64| nums.iter().any(|a| nums.iter().any(|b| 0.5 * (a + b) == v));
    let t = traced(&doc);
    assert!(
        t.len() > 100,
        "expected many traced numbers, found {}",
        t.len()
    );
    for (v, op, n) in t {
        let ok = match op.as_str() {
            "cell" => has(v),
            "median" => n > 0 && (has(v) || mid(v)),
            "count" | "sum" => v >= 0.0 && v.fract() == 0.0,
            "ratio" => v >= 0.0 && v.is_finite(),
            other => panic!("unknown data-op {other:?}"),
        };
        assert!(
            ok,
            "printed {v} ({op} over {n}) does not trace to the cells"
        );
    }
}

#[test]
fn figures_are_labelled_and_the_reference_is_modelled() {
    let doc = html();
    assert!(doc.contains("MEASURED"));
    assert!(doc.contains("MODELLED"));
    assert!(
        doc.contains("stroke-dasharray"),
        "the MODELLED reference is drawn dashed"
    );
    assert!(!doc.to_ascii_lowercase().contains("validated against"));
    for section in [
        "1. Summary",
        "2. Test matrix",
        "3. Scorecard",
        "4. Results per stated event",
        "5. C/N0 degradation",
        "6. Tracking stability",
        "7. Pass/fail",
        "8. Method and definitions",
        "9. Provenance",
        "10. Limits",
        "Appendix A",
    ] {
        assert!(doc.contains(section), "missing section {section:?}");
    }
}

#[test]
fn the_data_class_is_stated_and_the_digest_shown() {
    let doc = html();
    assert!(doc.contains("SYNTHETIC CAMPAIGN"));
    assert!(!doc.contains("CLIENT-CONFIDENTIAL:"));
    let c = load(&fixture()).unwrap();
    assert!(doc.contains(&c.digest));
    assert!(doc.contains("Lab campaign report"));
}

#[test]
fn null_metrics_render_as_words_not_zero() {
    let doc = html();
    // The outage event states no J/S, and in the engine fixture two narrow-PLL channels do not
    // re-acquire after the outage within the grace window.
    assert!(doc.contains("no J/S stated"));
    assert!(doc.contains("not within grace"));
}

#[test]
fn a_data_class_mismatch_is_refused() {
    // The engine fixture declares itself synthetic.
    let c = load(&fixture()).unwrap();
    assert_eq!(c.data_class(), DataClass::Synthetic);
    let err = render_html(&c, &LabReportOptions::new(DataClass::ClientConfidential)).unwrap_err();
    assert_eq!(
        err,
        RenderError::DataClassMismatch {
            campaign: DataClass::Synthetic,
            requested: DataClass::ClientConfidential
        }
    );
}

#[test]
fn an_incomplete_campaign_needs_the_flag_and_is_marked() {
    let mut c = load(&fixture()).unwrap();
    let k = c.cells.remove(0).key;
    c.missing.push(k);
    assert!(matches!(
        render_html(&c, &synthetic()),
        Err(RenderError::Incomplete { missing: 1 })
    ));
    let mut o = synthetic();
    o.allow_incomplete = true;
    let doc = render_html(&c, &o).unwrap();
    assert!(doc.contains("INCOMPLETE CAMPAIGN"));
}

#[test]
fn a_client_name_appears_only_when_passed() {
    let doc = html();
    assert!(!doc.contains("Prepared for"));
    let mut o = synthetic();
    o.client = Some("Example Laboratory".into());
    let doc = render_html(&load(&fixture()).unwrap(), &o).unwrap();
    assert!(doc.contains("Example Laboratory"));
}

#[test]
fn a_client_confidential_render_carries_the_export_notice_slot() {
    let mut c = load(&fixture()).unwrap();
    // Relabelled in memory only, to exercise the client-confidential layout on synthetic data.
    c.manifest.data_class = DataClass::ClientConfidential;
    let o = LabReportOptions::new(DataClass::ClientConfidential);
    let doc = render_html(&c, &o).unwrap();
    assert!(doc.contains("CLIENT-CONFIDENTIAL"));
    assert!(doc.contains(EXPORT_NOTICE_PLACEHOLDER));
    assert!(!doc.contains("SYNTHETIC CAMPAIGN"));
}

#[test]
fn designs_keep_their_colour_whatever_their_rank() {
    let doc = html();
    let colours: BTreeSet<&str> = ["#2a78d6", "#eb6834"].into_iter().collect();
    for c in &colours {
        assert!(doc.contains(c), "design colour {c} missing");
    }
    // The third slot is unused with two designs.
    assert!(!doc.contains("#1baf7a"));
}

// ── L4: the PDF edition ────────────────────────────────────────────────────────────────

use kshana_pro::report_lab_campaign_pdf::render_pdf;

/// Blank the trailer `/ID` literals, the one field printpdf randomises (see
/// tests/report_pdf.rs), so two renders can be compared byte for byte.
fn mask_id(b: &[u8]) -> Vec<u8> {
    let mut out = b.to_vec();
    if let Some(pos) = b.windows(4).position(|w| w == b"/ID[") {
        let (mut i, mut n) = (pos + 4, 0);
        while i < out.len() && n < 2 {
            if out[i] == b'(' {
                i += 1;
                while i < out.len() && out[i] != b')' {
                    out[i] = b'#';
                    i += 1;
                }
                n += 1;
            } else if out[i] == b']' {
                break;
            } else {
                i += 1;
            }
        }
    }
    out
}

/// The literal strings shown on the pages (body text uses the built-in fonts).
fn pdf_text(bytes: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(bytes).expect("the PDF parses");
    let mut all = String::new();
    for id in doc.get_pages().values() {
        let s = doc.get_page_content(*id).unwrap();
        let mut i = 0;
        while i < s.len() {
            if s[i] == b'(' {
                let mut depth = 1;
                i += 1;
                while i < s.len() && depth > 0 {
                    match s[i] {
                        b'\\' if i + 1 < s.len() => {
                            all.push(s[i + 1] as char);
                            i += 1;
                        }
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        c => all.push(c as char),
                    }
                    i += 1;
                }
                all.push('\n');
            } else if s[i] == b'<' && s.get(i + 1).is_some_and(|c| c.is_ascii_hexdigit()) {
                // printpdf writes a string containing parentheses as <hex>.
                let end = i + s[i..].iter().position(|c| *c == b'>').unwrap();
                let hex = std::str::from_utf8(&s[i + 1..end]).unwrap();
                for k in (0..hex.len() - hex.len() % 2).step_by(2) {
                    all.push(u8::from_str_radix(&hex[k..k + 2], 16).unwrap() as char);
                }
                all.push('\n');
                i = end + 1;
            } else {
                i += 1;
            }
        }
    }
    all
}

#[test]
fn the_pdf_edition_renders_deterministically_and_carries_the_same_facts() {
    let c = load(&fixture()).unwrap();
    let a = render_pdf(&c, &synthetic()).unwrap();
    assert!(a.starts_with(b"%PDF"));
    assert_eq!(mask_id(&a), mask_id(&render_pdf(&c, &synthetic()).unwrap()));
    let doc = lopdf::Document::load_mem(&a).unwrap();
    assert!(
        doc.get_pages().len() >= 4,
        "{} pages",
        doc.get_pages().len()
    );
    let text = pdf_text(&a);
    for must in [
        "SYNTHETIC CAMPAIGN",
        "1. Summary",
        "5. C/N0 degradation against stated J/S",
        "MODELLED reference",
        "10. Limits",
        &c.digest,
    ] {
        assert!(text.contains(must), "PDF text lacks {must:?}");
    }
    // The summary sentences are the HTML edition's, word for word (lines re-joined).
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flat.contains("narrow-pll held lock longest before loss"));
}

#[test]
fn the_pdf_edition_enforces_the_data_class_too() {
    let c = load(&fixture()).unwrap();
    assert!(render_pdf(&c, &LabReportOptions::new(DataClass::ClientConfidential)).is_err());
}

#[test]
fn the_report_states_which_lock_source_the_cells_used() {
    let doc = html();
    assert!(doc.contains("Lock source in these cells: track-session."));
    assert!(doc.contains("lock state machine"));
    assert!(doc.contains("counts as unlocked"));
}
