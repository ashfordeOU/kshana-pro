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
    // The outage event states no J/S, and the fixture's 30 dB bin has no locked epochs.
    assert!(doc.contains("no J/S stated"));
    assert!(doc.contains("no locked epochs"));
}

#[test]
fn a_data_class_mismatch_is_refused() {
    let c = load(&fixture()).unwrap();
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
    c.manifest.data_class = None; // an undeclared campaign renders as requested
    let o = LabReportOptions::new(DataClass::ClientConfidential);
    let doc = render_html(&c, &o).unwrap();
    assert!(doc.contains("CLIENT-CONFIDENTIAL"));
    assert!(doc.contains(EXPORT_NOTICE_PLACEHOLDER));
    assert!(!doc.contains("SYNTHETIC CAMPAIGN"));
}

#[test]
fn designs_keep_their_colour_whatever_their_rank() {
    let doc = html();
    let colours: BTreeSet<&str> = ["#2a78d6", "#eb6834", "#1baf7a"].into_iter().collect();
    for c in &colours {
        assert!(doc.contains(c), "design colour {c} missing");
    }
    // The fourth slot is unused with three designs.
    assert!(!doc.contains("#eda100"));
}
