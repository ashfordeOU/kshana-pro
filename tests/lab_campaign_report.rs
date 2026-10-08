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

/// `(value, op, n, tag, printed)` of every traced number in the document; `tag` is the element
/// name carrying the attributes and `printed` the text up to the next `<`.
fn traced(doc: &str) -> Vec<(f64, String, usize, String, String)> {
    let mut out = Vec::new();
    for (i, _) in doc.match_indices("data-v=\"") {
        let tag_at = doc[..i].rfind('<').unwrap() + 1;
        let tag: String = doc[tag_at..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        let rest = &doc[i + 8..];
        let v: f64 = rest[..rest.find('"').unwrap()].parse().unwrap();
        let op_at = rest.find("data-op=\"").unwrap() + 9;
        let op = &rest[op_at..op_at + rest[op_at..].find('"').unwrap()];
        let n_at = rest.find("data-n=\"").unwrap() + 8;
        let n: usize = rest[n_at..n_at + rest[n_at..].find('"').unwrap()]
            .parse()
            .unwrap();
        let close = rest[n_at..].find('>').unwrap() + n_at + 1;
        let printed =
            rest[close..rest[close..].find('<').map_or(rest.len(), |p| close + p)].to_owned();
        out.push((v, op.to_owned(), n, tag, printed));
    }
    out
}

/// The cell documents, as JSON.
fn cells() -> Vec<serde_json::Value> {
    let mut v: Vec<_> = std::fs::read_dir(fixture().join("cells"))
        .unwrap()
        .flatten()
        .map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap())
        .collect();
    v.sort_by_key(|c: &serde_json::Value| c["key"].as_str().unwrap().to_owned());
    v
}

fn scorecard_rows() -> Vec<serde_json::Value> {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture().join("scorecard.json")).unwrap())
            .unwrap();
    v["rows"].as_array().unwrap().clone()
}

type Key = (String, u64, usize);

fn key(op: &str, v: f64, n: usize) -> Key {
    (op.to_owned(), v.to_bits(), n)
}

/// Every `(op, value, inputs)` a count, sum or ratio may legitimately take, recomputed from
/// the cell files and `scorecard.json` here, independently of the report's aggregation.
fn expected_derived() -> BTreeSet<Key> {
    use std::collections::BTreeMap;
    let cells = cells();
    let mut out = BTreeSet::new();
    let f = |v: &serde_json::Value, k: &str| v[k].as_f64().unwrap_or(0.0);
    let name = |v: &serde_json::Value, k: &str| v[k]["name"].as_str().unwrap().to_owned();

    // Whole-run sums and the false-lock ratio, per (chain, design).
    let mut whole: BTreeMap<(String, String), Vec<&serde_json::Value>> = BTreeMap::new();
    for c in &cells {
        for s in c["satellites"].as_array().unwrap() {
            whole
                .entry((name(c, "frontend"), name(c, "design")))
                .or_default()
                .push(&s["whole_run"]);
        }
    }
    for runs in whole.values() {
        let n = runs.len();
        let episodes: f64 = runs.iter().map(|r| f(r, "false_lock_episodes")).sum();
        let losses: f64 = runs.iter().map(|r| f(r, "loss_count")).sum();
        let locked_h: f64 = runs.iter().map(|r| f(r, "locked_s")).sum::<f64>() / 3600.0;
        out.insert(key("sum", episodes, n));
        out.insert(key("sum", losses, n));
        if locked_h > 0.0 {
            out.insert(key("ratio", episodes / locked_h, n));
        }
    }

    // Event counts, and curve-bin sums, per (recording, event, chain, design).
    let mut events: BTreeMap<(String, String, String, String), Vec<&serde_json::Value>> =
        BTreeMap::new();
    for c in &cells {
        for s in c["satellites"].as_array().unwrap() {
            for e in s["events"].as_array().unwrap() {
                events
                    .entry((
                        c["recording"]["id"].as_str().unwrap().to_owned(),
                        e["event_id"].as_str().unwrap().to_owned(),
                        name(c, "frontend"),
                        name(c, "design"),
                    ))
                    .or_default()
                    .push(e);
            }
        }
    }
    for evs in events.values() {
        let lost = evs.iter().filter(|e| e["lost"] == true).count();
        let reacq = evs.iter().filter(|e| e["reacquired"] == true).count();
        out.insert(key("count", lost as f64, evs.len()));
        out.insert(key("count", reacq as f64, lost));
        let mut bins: BTreeMap<u64, (f64, usize)> = BTreeMap::new();
        for e in evs {
            for b in e["cn0_curve"].as_array().unwrap() {
                let slot = bins
                    .entry(b["js_db"].as_f64().unwrap().to_bits())
                    .or_default();
                slot.0 += f(b, "n");
                slot.1 += 1;
            }
        }
        for (sum, k) in bins.into_values() {
            out.insert(key("sum", sum, k));
        }
    }

    // Counts of the campaign's own parts, printed as "n of n": cells, recordings, chains, designs.
    let distinct = |f: &dyn Fn(&serde_json::Value) -> String| {
        cells.iter().map(f).collect::<BTreeSet<_>>().len()
    };
    for n in [
        cells.len(),
        distinct(&|c| c["recording"]["id"].as_str().unwrap().to_owned()),
        distinct(&|c| name(c, "frontend")),
        distinct(&|c| name(c, "design")),
    ] {
        out.insert(key("count", n as f64, n));
    }

    // Verdict counts: PASS / (PASS + FAIL) of the engine's rows, grouped every way the report
    // groups them, and in total.
    let rows = scorecard_rows();
    let tally = |sel: &dyn Fn(&serde_json::Value) -> bool| {
        let (mut p, mut e) = (0usize, 0usize);
        for r in rows.iter().filter(|r| sel(r)) {
            match r["verdict"].as_str().unwrap_or("") {
                "PASS" => {
                    p += 1;
                    e += 1;
                }
                "FAIL" => e += 1,
                _ => {}
            }
        }
        (p, e)
    };
    let mut add = |(p, e): (usize, usize)| {
        if e > 0 {
            out.insert(key("count", p as f64, e));
            out.insert(key("count", e as f64, e));
        }
    };
    add(tally(&|_| true));
    let mut groups = BTreeSet::new();
    for r in &rows {
        let s = |k: &str| r[k].as_str().unwrap_or("").to_owned();
        groups.insert((
            s("scope"),
            s("frontend"),
            s("design"),
            s("recording"),
            s("event_id"),
        ));
    }
    for (scope, fe, de, rec, ev) in groups {
        add(tally(&|r| {
            let s = |k: &str| r[k].as_str().unwrap_or("");
            s("scope") == scope && s("frontend") == fe && s("design") == de
        }));
        add(tally(&|r| {
            let s = |k: &str| r[k].as_str().unwrap_or("");
            s("scope") == scope
                && s("frontend") == fe
                && s("design") == de
                && s("recording") == rec
                && s("event_id") == ev
        }));
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
    let expected = expected_derived();
    let t = traced(&doc);
    assert!(
        t.len() > 100,
        "expected many traced numbers, found {}",
        t.len()
    );
    let mut seen = BTreeSet::new();
    let mut failures = Vec::new();
    for (v, op, n, _tag, _printed) in t {
        seen.insert(op.clone());
        let ok = match op.as_str() {
            "cell" => has(v),
            // An odd count prints the middle value; an even one the mean of the middle two.
            "median" if n % 2 == 1 => has(v),
            "median" => n > 0 && (has(v) || mid(v)),
            "count" | "sum" | "ratio" => expected.contains(&key(&op, v, n)),
            other => panic!("unknown data-op {other:?}"),
        };
        if !ok {
            failures.push(format!("printed {v} ({op} over {n})"));
        }
    }
    assert!(
        failures.is_empty(),
        "do not trace to the cells: {failures:?}"
    );
    for op in ["cell", "median", "count", "sum", "ratio"] {
        assert!(seen.contains(op), "the fixture report has no {op} number");
    }
}

#[test]
fn a_wrong_count_sum_or_ratio_would_not_trace() {
    // The check is not "any non-negative value": a perturbed number is not in the expected set.
    let expected = expected_derived();
    let (op, bits, n) = expected.iter().find(|k| k.0 == "sum").cloned().unwrap();
    let v = f64::from_bits(bits);
    assert!(expected.contains(&key(&op, v, n)));
    // A sum of whole-number counts never has a fractional part.
    assert!(!expected.contains(&key(&op, v + 0.5, n)));
    assert!(!expected.contains(&key(&op, v, n + 1000)));
    let r = expected.iter().find(|k| k.0 == "ratio").cloned().unwrap();
    assert!(!expected.contains(&key("ratio", f64::from_bits(r.1) * 1.01 + 1e-9, r.2)));
}

#[test]
fn the_summary_and_heatmap_numbers_are_traced_and_match_their_text() {
    let doc = html();
    let t = traced(&doc);
    let spans: Vec<_> = t.iter().filter(|x| x.3 == "span").collect();
    // Intro (recordings, chains, designs, cells), per-event findings and the verdict totals.
    assert!(spans.len() >= 8, "summary spans: {}", spans.len());
    for (v, op, _n, _tag, printed) in &spans {
        let p: f64 = printed
            .parse()
            .unwrap_or_else(|_| panic!("{printed:?} is not a number"));
        let decimals = printed.split('.').nth(1).map_or(0, str::len) as i32;
        assert!(
            (p - v).abs() <= 0.5 * 10f64.powi(-decimals) + 1e-12,
            "{op}: printed {printed} but traced {v}"
        );
    }
    let texts: Vec<_> = t.iter().filter(|x| x.3 == "text").collect();
    assert_eq!(texts.len(), 4, "one traced heatmap cell per chain x design");
    for (v, op, _n, _tag, printed) in &texts {
        assert_eq!(op, "median");
        assert_eq!(*printed, format!("{:.1} %", 100.0 * v));
    }
    // The intro counts equal the loaded campaign.
    let c = load(&fixture()).unwrap();
    let counts: Vec<f64> = spans
        .iter()
        .filter(|x| x.1 == "count")
        .map(|x| x.0)
        .take(4)
        .collect();
    assert_eq!(
        counts,
        vec![
            c.manifest.recordings.len() as f64,
            2.0,
            2.0,
            c.cells.len() as f64
        ]
    );
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
    assert!(flat.contains("baseline re-acquired fastest"));
    // The outage times to loss differ by about 1e-8 s, below the printed precision: a tie,
    // never "held lock longest".
    assert!(flat.contains("every design lost lock after the same median time (0.24 s"));
    assert!(!flat.contains("held lock longest"));
    assert!(flat.contains("every design held lock throughout"));
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

// ── per-recording bars that relax a campaign bar ───────────────────────────────────────

/// A scratch copy of the fixture whose first recording states its own `bars`.
fn with_recording_bars(bars: serde_json::Value) -> PathBuf {
    with_bars(None, bars)
}

/// As above, also replacing the campaign bars when given.
fn with_bars(campaign: Option<serde_json::Value>, bars: serde_json::Value) -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "kshana-pro-relaxed-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cells")).unwrap();
    let src = fixture();
    for f in ["scorecard.json", "DIGEST"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    for e in std::fs::read_dir(src.join("cells")).unwrap().flatten() {
        std::fs::copy(e.path(), dir.join("cells").join(e.file_name())).unwrap();
    }
    let mut v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("campaign.json")).unwrap()).unwrap();
    if let Some(c) = campaign {
        v["scoring"]["bars"] = c;
    }
    v["recordings"][0]["conditions"]["bars"] = bars;
    std::fs::write(
        dir.join("campaign.json"),
        serde_json::to_string_pretty(&v).unwrap(),
    )
    .unwrap();
    dir
}

fn relaxed_rendered(bars: serde_json::Value) -> (String, String) {
    let dir = with_recording_bars(bars);
    let c = load(&dir).unwrap();
    let html = render_html(&c, &synthetic()).unwrap();
    let pdf = pdf_text(&render_pdf(&c, &synthetic()).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    (html, pdf)
}

#[test]
fn a_recording_bar_looser_than_the_campaign_bar_is_flagged_in_html_and_pdf() {
    // The campaign bars are min_availability 0.5 and max_reacq_s 3.0.
    let (html, pdf) = relaxed_rendered(serde_json::json!({
        "min_availability": 0.2, "max_reacq_s": 20.0
    }));
    for (what, doc) in [("HTML", &html), ("PDF", &pdf)] {
        assert!(doc.contains("RELAXED BAR"), "{what} lacks the flag");
        assert!(
            doc.contains("min_availability"),
            "{what} lacks the bar name"
        );
        assert!(doc.contains("max_reacq_s"), "{what} lacks the second bar");
    }
    // Next to the affected recording's event rows and in section 7.
    assert!(html.matches("RELAXED BAR").count() >= 4);
    assert!(html.contains("[relaxed bar]"));
}

#[test]
fn a_stricter_or_equal_recording_bar_is_not_flagged() {
    let (h, pdf) = relaxed_rendered(serde_json::json!({
        "min_availability": 0.9, "max_reacq_s": 1.0
    }));
    assert!(!h.contains("RELAXED BAR") && !pdf.contains("RELAXED BAR"));
    let (h, _) = relaxed_rendered(serde_json::json!({ "min_availability": 0.5 }));
    assert!(!h.contains("RELAXED BAR"));
    // A bar the campaign does not state is an extra bar, not a relaxation.
    let (h, _) = relaxed_rendered(serde_json::json!({ "max_false_lock_per_hour": 9.0 }));
    assert!(!h.contains("RELAXED BAR"));
    // The unmodified fixture has none.
    assert!(!html().contains("RELAXED BAR"));
}

#[test]
fn an_unknown_or_non_numeric_recording_bar_is_refused_by_the_loader() {
    for bars in [
        serde_json::json!({ "availability_goal": 0.1 }),
        serde_json::json!({ "min_availability": "low" }),
    ] {
        let dir = with_recording_bars(bars);
        let err = load(&dir).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            matches!(err, kshana_pro::lab_campaign::LoadError::BadBars { .. }),
            "{err}"
        );
    }
}

#[test]
fn every_engine_bar_has_a_direction_and_relaxes_the_right_way() {
    use kshana_pro::lab_campaign::model::BarDir;
    use kshana_pro::lab_campaign::model::BAR_DIRECTIONS;
    assert_eq!(BAR_DIRECTIONS.len(), 7);
    for (name, dir) in BAR_DIRECTIONS {
        let (strict, loose) = match dir {
            BarDir::Min => (10.0, 5.0),
            BarDir::Max => (5.0, 10.0),
        };
        let relaxed = |own: f64| {
            let dir = with_bars(
                Some(serde_json::json!({ *name: strict })),
                serde_json::json!({ *name: own }),
            );
            let c = load(&dir).unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            render_html(&c, &synthetic())
                .unwrap()
                .contains("RELAXED BAR")
        };
        assert!(relaxed(loose), "{name}: {loose} vs {strict} should relax");
        assert!(!relaxed(strict), "{name}: equal is not relaxed");
        let tighter = match dir {
            BarDir::Min => 20.0,
            BarDir::Max => 1.0,
        };
        assert!(!relaxed(tighter), "{name}: stricter is not relaxed");
    }
}
