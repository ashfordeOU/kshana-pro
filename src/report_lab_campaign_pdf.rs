//! # Lab campaign report: the PDF edition (step L4)
//!
//! The same report as [`crate::report_lab_campaign`], laid out as A4 pages in the Ashforde
//! PDF style ([`crate::brand_pdf`] furniture through the crate's PDF [`Layout`]). It reads
//! the same aggregates and the same summary, so the two editions cannot disagree. The
//! curves are given as tables here; the HTML edition also draws them.
//!
//! Text uses the PDF built-in fonts, so every string is reduced to ASCII first (a dash for
//! an en dash, "deg" for the degree sign).

use crate::lab_campaign::aggregate::{aggregate, Aggregates, Stat};
use crate::lab_campaign::model::DataClass;
use crate::lab_campaign::LoadedCampaign;
use crate::report_brand as brand;
use crate::report_lab_campaign::{
    check, summary, LabReportOptions, RenderError, CAVEATS, DEFINITIONS, EXPORT_NOTICE_PLACEHOLDER,
};
use crate::report_pdf::{
    rgb, sans, sans_bold, serif, serif_bold, wrap, Layout, CONTENT_W_MM, MARGIN_MM,
};

/// Reduce text to what the built-in PDF fonts can show.
fn ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '–' | '—' | '−' => out.push('-'),
            '°' => out.push_str(" deg"),
            '→' => out.push_str("->"),
            '·' => out.push('|'),
            '×' => out.push('x'),
            '\u{202f}' | '\u{a0}' => out.push(' '),
            '’' | '‘' => out.push('\''),
            '“' | '”' => out.push('"'),
            c if c.is_ascii() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

fn stat(s: &Stat, decimals: usize, unit: &str, none: &str) -> String {
    match s.value {
        Some(v) => format!("{v:.decimals$}{unit}"),
        None => none.to_owned(),
    }
}

fn pct(s: &Stat) -> String {
    match s.value {
        Some(v) => format!("{:.1} %", 100.0 * v),
        None => "no data".to_owned(),
    }
}

/// A simple table: a bold header row, then rows; each column gets a fixed share of the
/// content width, and cell text that does not fit is cut with "...".
fn table(l: &mut Layout, widths: &[f32], header: &[&str], rows: &[Vec<String>]) {
    let pt = 7.6;
    let lh = 4.4;
    let total: f32 = widths.iter().sum();
    let cols: Vec<f32> = widths.iter().map(|w| w / total * CONTENT_W_MM).collect();
    let draw = |l: &mut Layout, cells: &[String], bold: bool| {
        l.ensure(lh + 1.0);
        let mut x = MARGIN_MM;
        for (i, c) in cells.iter().enumerate() {
            let w = cols.get(i).copied().unwrap_or(20.0);
            let mut t = ascii(c);
            let lines = wrap(&t, w - 1.5, pt);
            if lines.len() > 1 {
                t = format!("{}...", lines[0].trim_end());
            }
            let font = if bold { sans_bold() } else { sans() };
            let col = if bold {
                rgb(brand::INK_2)
            } else {
                rgb(brand::INK)
            };
            l.text_at(x, &t, font, pt, col);
            x += w;
        }
        l.advance(lh);
    };
    // Keep the header with at least two rows, and put its rule below the descenders.
    l.ensure(3.0 * lh + 4.0);
    let head: Vec<String> = header.iter().map(|h| h.to_string()).collect();
    draw(l, &head, true);
    l.advance(-lh + 1.6);
    l.rule(rgb(brand::LINE), 0.4);
    l.advance(lh - 0.6);
    for r in rows {
        draw(l, r, false);
    }
    l.advance(3.0);
}

fn para(l: &mut Layout, text: &str) {
    l.paragraph(&ascii(text), serif(), 10.0, 4.9, rgb(brand::INK));
    l.advance(2.0);
}

fn sub(l: &mut Layout, text: &str) {
    // Room for the heading, a table header and two rows, so a heading never ends a page.
    l.ensure(30.0);
    l.advance(2.0);
    l.text_at(
        MARGIN_MM,
        &ascii(text),
        serif_bold(),
        11.5,
        rgb(brand::NAVY_INK),
    );
    l.advance(5.5);
}

/// Render the PDF edition.
pub fn render_pdf(c: &LoadedCampaign, o: &LabReportOptions) -> Result<Vec<u8>, String> {
    check(c, o).map_err(|e: RenderError| e.to_string())?;
    let a = aggregate(c);
    let mut l = Layout::new();
    cover(&mut l, c, o);
    summary_section(&mut l, c, &a);
    matrix_section(&mut l, c);
    scorecard_section(&mut l, &a);
    events_section(&mut l, &a);
    curves_section(&mut l, &a);
    stability_section(&mut l, &a);
    pass_fail_section(&mut l, c, &a);
    method_section(&mut l);
    provenance_section(&mut l, c);
    limits_section(&mut l, o);
    appendix(&mut l, c);
    let footer = format!(
        "Ashforde OU  |  Lab campaign report  |  {}  |  MEASURED loop response; MODELLED reference only",
        o.data_class.label()
    );
    let id = o.doc_id.clone().unwrap_or_else(|| l.content_id());
    let mark = crate::brand_pdf::PdfMark::new("Lab campaign report", id);
    let pages = l.finish(&footer, &mark);
    crate::brand_pdf::save(
        &format!("Lab campaign report {}", ascii(&c.manifest.name)),
        pages,
    )
}

fn cover(l: &mut Layout, c: &LoadedCampaign, o: &LabReportOptions) {
    l.cover_lockup();
    l.ensure(12.0);
    l.text_at(
        MARGIN_MM,
        "Lab campaign report",
        serif_bold(),
        24.0,
        rgb(brand::INK),
    );
    l.advance(10.0);
    let subtitle = match &o.client {
        Some(cl) => format!("Campaign {} | prepared for {}", c.manifest.name, cl),
        None => format!("Campaign {}", c.manifest.name),
    };
    l.text_at(MARGIN_MM, &ascii(&subtitle), serif(), 13.0, rgb(brand::INK));
    l.advance(7.0);
    l.rule(rgb(brand::BRASS), 1.2);
    l.advance(8.0);
    banner(l, c, o);
    let mut rows = vec![
        vec!["Campaign".to_owned(), c.manifest.name.clone()],
        vec!["Data class".to_owned(), o.data_class.label().to_owned()],
        vec![
            "Engine that produced the cells".to_owned(),
            format!("Kshana {}", c.manifest.engine_version),
        ],
        vec!["Report produced with".to_owned(), brand::produced_with()],
        vec![
            "Cells".to_owned(),
            format!(
                "{} of {} present, {}",
                c.cells.len(),
                c.manifest.cells.len(),
                if c.is_complete() {
                    "complete"
                } else {
                    "INCOMPLETE"
                }
            ),
        ],
        vec!["DIGEST, verified".to_owned(), c.digest.clone()],
    ];
    if let Some(cl) = &o.client {
        rows.insert(1, vec!["Prepared for".to_owned(), cl.clone()]);
    }
    if let Some(t) = &o.prepared_utc {
        rows.push(vec!["Prepared UTC".to_owned(), t.clone()]);
    }
    table(l, &[1.0, 2.6], &["Item", "Value"], &rows);
}

fn banner(l: &mut Layout, c: &LoadedCampaign, o: &LabReportOptions) {
    let (fill, lines): (&str, Vec<String>) = match o.data_class {
        DataClass::Synthetic => (
            "#e6eef8",
            wrap(
                "SYNTHETIC CAMPAIGN: Kshana-generated scenes, not a client's recordings. The numbers \
                 illustrate the method; they are not a measurement of any receiver.",
                CONTENT_W_MM - 6.0,
                9.5,
            ),
        ),
        DataClass::ClientConfidential => {
            let mut v = wrap(
                "CLIENT-CONFIDENTIAL: results on recordings supplied by the client. Do not forward.",
                CONTENT_W_MM - 6.0,
                9.5,
            );
            v.extend(wrap(
                &ascii(o.export_notice.as_deref().unwrap_or(EXPORT_NOTICE_PLACEHOLDER)),
                CONTENT_W_MM - 6.0,
                9.5,
            ));
            ("#f8e7e3", v)
        }
    };
    let mut lines = lines;
    if !c.is_complete() {
        lines.push(format!(
            "INCOMPLETE CAMPAIGN: {} of {} cells present.",
            c.cells.len(),
            c.manifest.cells.len()
        ));
    }
    let h = 4.0 + lines.len() as f32 * 5.0 + 3.0;
    l.ensure(h);
    l.fill_rect(MARGIN_MM, l.y_mm - h + 4.0, CONTENT_W_MM, h, rgb(fill));
    l.advance(2.0);
    for line in &lines {
        l.advance(5.0);
        l.text_at(MARGIN_MM + 3.0, line, sans_bold(), 9.5, rgb(brand::INK));
    }
    l.advance(8.0);
}

fn summary_section(l: &mut Layout, c: &LoadedCampaign, a: &Aggregates) {
    let sm = summary(c, a);
    l.heading("1. Summary");
    para(l, &sm.intro);
    for (head, body) in &sm.items {
        para(l, &format!("- {head}: {body}"));
    }
    if let Some(b) = &sm.bars {
        para(l, &format!("- {b}"));
    }
    for (tag, text) in CAVEATS {
        let t = if tag.is_empty() {
            text.to_owned()
        } else {
            format!("{tag}: {text}")
        };
        para(l, &t);
    }
}

fn matrix_section(l: &mut Layout, c: &LoadedCampaign) {
    l.heading("2. Test matrix");
    sub(l, "Recordings and stated test conditions");
    let mut rows = Vec::new();
    for r in &c.manifest.recordings {
        let dut = r
            .conditions
            .pointer("/receiver/dut")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let events = r
            .conditions
            .get("event")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();
        if events.is_empty() {
            rows.push(vec![
                r.id.clone(),
                dut.clone(),
                "none".into(),
                String::new(),
                String::new(),
            ]);
        }
        for ev in events {
            let g = |k: &str| {
                ev.get(k)
                    .map(|v| v.to_string().trim_matches('"').to_owned())
                    .unwrap_or_default()
            };
            rows.push(vec![
                r.id.clone(),
                dut.clone(),
                g("id"),
                format!("{} / {}", g("kind"), g("type")),
                format!("{}-{} s", g("onset_s"), g("offset_s")),
            ]);
        }
    }
    table(
        l,
        &[1.3, 1.6, 0.8, 1.5, 1.0],
        &[
            "Recording",
            "Receiver as stated",
            "Event",
            "Kind / type",
            "Window",
        ],
        &rows,
    );
    sub(l, "Front-end chains and loop designs");
    let mut rows = Vec::new();
    for f in &c.manifest.frontends {
        rows.push(vec!["chain".into(), f.name.clone(), params(&f.params)]);
    }
    for d in &c.manifest.designs {
        rows.push(vec!["design".into(), d.name.clone(), params(&d.params)]);
    }
    table(l, &[0.6, 1.0, 4.0], &["Input", "Name", "Parameters"], &rows);
}

fn params(v: &serde_json::Value) -> String {
    match v.as_object() {
        Some(m) if m.is_empty() => "empty chain".into(),
        Some(m) => m
            .iter()
            .map(|(k, v)| format!("{k}={}", v.to_string().trim_matches('"')))
            .collect::<Vec<_>>()
            .join(", "),
        None => v.to_string(),
    }
}

fn scorecard_section(l: &mut Layout, a: &Aggregates) {
    l.heading("3. Scorecard, whole run");
    para(
        l,
        "MEASURED. Per front-end chain and loop design, over every recording and satellite; \
         medians over satellites. False locks are episodes over locked hours, summed across satellites.",
    );
    let rows: Vec<Vec<String>> = a
        .scorecard
        .iter()
        .map(|r| {
            vec![
                r.frontend.clone(),
                r.design.clone(),
                r.satellites.to_string(),
                pct(&r.availability),
                pct(&r.phase_lock_frac),
                stat(&r.losses, 0, "", "no data"),
                stat(&r.false_lock_per_locked_hour, 2, "", "no locked time"),
                stat(&r.pll_jitter_deg, 2, " deg", "no data"),
                stat(&r.dll_jitter_chips, 3, " chip", "no data"),
            ]
        })
        .collect();
    table(
        l,
        &[1.1, 1.1, 0.7, 0.9, 0.9, 0.6, 1.0, 0.9, 0.9],
        &[
            "Chain",
            "Design",
            "Sats",
            "Avail.",
            "Phase lock",
            "Losses",
            "False/locked h",
            "PLL jitter",
            "DLL jitter",
        ],
        &rows,
    );
}

fn events_section(l: &mut Layout, a: &Aggregates) {
    l.heading("4. Results per stated event");
    para(
        l,
        "MEASURED. Per chain and design over the satellites each event affected. Time to loss from \
         the onset; re-acquisition from the end of the event or the loss, if later.",
    );
    let mut seen: Vec<(String, String)> = Vec::new();
    for e in &a.events {
        let k = (e.recording.clone(), e.event_id.clone());
        if seen.contains(&k) {
            continue;
        }
        seen.push(k.clone());
        sub(
            l,
            &format!("{} - {}, recording {}", e.event_id, e.kind, e.recording),
        );
        let rows: Vec<Vec<String>> = a
            .events
            .iter()
            .filter(|r| r.recording == k.0 && r.event_id == k.1)
            .map(|r| {
                vec![
                    r.frontend.clone(),
                    r.design.clone(),
                    r.satellites.to_string(),
                    stat(&r.lost, 0, "", "-"),
                    stat(&r.time_to_loss_s, 2, " s", "held lock"),
                    stat(&r.js_at_loss_db, 1, " dB", "no J/S stated"),
                    stat(&r.reacq_time_s, 2, " s", "not within grace"),
                    pct(&r.availability),
                ]
            })
            .collect();
        table(
            l,
            &[1.1, 1.1, 0.6, 0.6, 1.0, 1.1, 1.2, 0.9],
            &[
                "Chain",
                "Design",
                "Sats",
                "Lost",
                "Time to loss",
                "J/S at loss",
                "Re-acq. time",
                "Avail.",
            ],
            &rows,
        );
    }
    if seen.is_empty() {
        para(l, "No stated events: whole-run metrics only.");
    }
}

fn curves_section(l: &mut Layout, a: &Aggregates) {
    l.heading("5. C/N0 degradation against stated J/S");
    para(
        l,
        "MEASURED: median degradation per J/S bin over satellites, per design. MODELLED: the analytic \
         spectral-separation reference, for comparison only; it is not a validation. The HTML \
         edition also draws these as curves.",
    );
    if a.curves.is_empty() {
        para(l, "No event states a J/S profile; no curve.");
        return;
    }
    let mut seen: Vec<(String, String, String)> = Vec::new();
    for cv in &a.curves {
        let k = (
            cv.recording.clone(),
            cv.event_id.clone(),
            cv.frontend.clone(),
        );
        if seen.contains(&k) {
            continue;
        }
        seen.push(k.clone());
        sub(l, &format!("{} - recording {}, chain {}", k.1, k.0, k.2));
        let mut rows = Vec::new();
        let mut formula = None;
        for c in a
            .curves
            .iter()
            .filter(|c| c.recording == k.0 && c.event_id == k.1 && c.frontend == k.2)
        {
            if formula.is_none() {
                formula = c.formula.clone().map(|f| match c.q {
                    Some(q) => format!("{f}, Q = {q}"),
                    None => f,
                });
            }
            for b in &c.bins {
                rows.push(vec![
                    c.design.clone(),
                    format!("{} dB", b.js_db),
                    stat(&b.n, 0, "", "-"),
                    stat(&b.measured_degradation_db, 2, " dB", "no locked epochs"),
                    stat(&b.modelled_degradation_db, 2, " dB", "-"),
                ]);
            }
        }
        table(
            l,
            &[1.2, 0.8, 1.0, 1.4, 1.4],
            &[
                "Design",
                "Stated J/S",
                "Locked epochs",
                "MEASURED degr.",
                "MODELLED degr.",
            ],
            &rows,
        );
        if let Some(f) = formula {
            para(l, &format!("MODELLED reference: {f}."));
        }
    }
}

fn stability_section(l: &mut Layout, a: &Aggregates) {
    l.heading("6. Tracking stability");
    let rows: Vec<Vec<String>> = a
        .scorecard
        .iter()
        .map(|r| {
            vec![
                r.frontend.clone(),
                r.design.clone(),
                stat(&r.pll_jitter_deg, 2, " deg", "no data"),
                stat(&r.dll_jitter_chips, 3, " chip", "no data"),
                stat(&r.false_lock_episodes, 0, "", "no data"),
                stat(&r.false_lock_per_locked_hour, 2, "", "no locked time"),
            ]
        })
        .collect();
    table(
        l,
        &[1.1, 1.1, 1.0, 1.0, 1.0, 1.1],
        &[
            "Chain",
            "Design",
            "PLL jitter",
            "DLL jitter",
            "False locks",
            "per locked h",
        ],
        &rows,
    );
}

fn pass_fail_section(l: &mut Layout, c: &LoadedCampaign, a: &Aggregates) {
    l.heading("7. Pass/fail");
    let bars = c.manifest.scoring.get("bars").and_then(|b| b.as_object());
    let any = a.events.iter().any(|r| !r.bars.is_empty());
    let (Some(bars), true) = (bars, any) else {
        para(
            l,
            "The client set no pass/fail bars for this campaign, so no pass or fail is implied.",
        );
        return;
    };
    let stated = bars
        .iter()
        .map(|(k, v)| format!("{k} = {v}"))
        .collect::<Vec<_>>()
        .join(", ");
    para(l, &format!("Bars as stated: {stated}."));
    let mut rows = Vec::new();
    for r in a.events.iter().filter(|r| !r.bars.is_empty()) {
        for (k, (p, e)) in &r.bars {
            rows.push(vec![
                r.event_id.clone(),
                r.frontend.clone(),
                r.design.clone(),
                k.clone(),
                p.to_string(),
                e.to_string(),
            ]);
        }
    }
    table(
        l,
        &[0.9, 1.1, 1.1, 1.2, 0.7, 0.8],
        &["Event", "Chain", "Design", "Bar", "Passed", "Evaluated"],
        &rows,
    );
}

fn method_section(l: &mut Layout) {
    l.heading("8. Method and definitions");
    para(
        l,
        "Each cell replays one recording through one front-end chain and one loop design in Kshana's \
         software receiver and scores the epoch stream online against the stated test conditions. \
         Definitions for cell schema kshana.campaign-cell/1:",
    );
    for (k, v) in DEFINITIONS {
        para(l, &format!("{k}: {v}"));
    }
}

fn provenance_section(l: &mut Layout, c: &LoadedCampaign) {
    l.heading("9. Provenance and reproducibility");
    para(
        l,
        &format!(
            "DIGEST {} verified against the {} cell files present; every cell key re-derived from its \
             inputs. Engine {}. Reproduce with kshana iq campaign, then kshana-pro lab-report.",
            c.digest,
            c.cells.len(),
            c.manifest.engine_version
        ),
    );
    let mut rows: Vec<Vec<String>> = c
        .cells
        .iter()
        .map(|x| {
            vec![
                x.key.chars().take(16).collect(),
                x.recording.id.clone(),
                x.recording.sha256.chars().take(16).collect(),
                x.frontend.name.clone(),
                x.design.name.clone(),
            ]
        })
        .collect();
    for k in &c.missing {
        rows.push(vec![
            k.chars().take(16).collect(),
            "missing".into(),
            String::new(),
            String::new(),
            String::new(),
        ]);
    }
    table(
        l,
        &[1.3, 1.2, 1.3, 1.0, 1.0],
        &[
            "Cell key",
            "Recording",
            "Recording SHA-256",
            "Chain",
            "Design",
        ],
        &rows,
    );
}

fn limits_section(l: &mut Layout, o: &LabReportOptions) {
    l.heading("10. Limits");
    for t in [
        "Results describe Kshana's software receiver with the stated loop designs and front-end chains on \
         the supplied recordings and stated conditions; they make no claim about the receiver that was recorded.",
        "Kshana generated and transmitted no interference for this work; the stated jammer and spoofer \
         conditions are the lab's.",
        "The MODELLED curve is an analytic reference, not a validation of the model or of the loops.",
        "No extrapolation to untested conditions is made here; calibrated predictions are labelled PREDICTION.",
    ] {
        para(l, &format!("- {t}"));
    }
    if o.data_class == DataClass::Synthetic {
        para(
            l,
            "- This is a SYNTHETIC campaign; its numbers illustrate the method only.",
        );
    }
}

fn appendix(l: &mut Layout, c: &LoadedCampaign) {
    l.heading("Appendix A. Per-satellite results");
    let mut rows = Vec::new();
    for x in &c.cells {
        for s in &x.satellites {
            let w = &s.whole_run;
            rows.push(vec![
                x.recording.id.clone(),
                x.frontend.name.clone(),
                x.design.name.clone(),
                format!("{} {}", s.signal, s.id),
                format!("{:.1} %", 100.0 * w.availability),
                w.loss_count.to_string(),
                w.baseline_cn0_dbhz
                    .map(|v| format!("{v:.1} dB-Hz"))
                    .unwrap_or_else(|| "no data".into()),
                w.pll_jitter_deg
                    .map(|v| format!("{v:.2} deg"))
                    .unwrap_or_else(|| "no data".into()),
            ]);
        }
    }
    table(
        l,
        &[1.3, 1.0, 1.0, 1.0, 0.8, 0.6, 1.0, 0.9],
        &[
            "Recording",
            "Chain",
            "Design",
            "Satellite",
            "Avail.",
            "Losses",
            "Baseline C/N0",
            "PLL jitter",
        ],
        &rows,
    );
    l.heading("Appendix B. Per-satellite event results");
    let mut rows = Vec::new();
    for x in &c.cells {
        for s in &x.satellites {
            for e in &s.events {
                rows.push(vec![
                    x.recording.id.clone(),
                    x.frontend.name.clone(),
                    x.design.name.clone(),
                    format!("{} {}", s.signal, s.id),
                    e.event_id.clone(),
                    e.time_to_loss_s
                        .map(|v| format!("{v:.2} s"))
                        .unwrap_or_else(|| "held lock".into()),
                    e.js_at_loss_db
                        .map(|v| format!("{v:.1} dB"))
                        .unwrap_or_else(|| "no J/S stated".into()),
                    e.reacq_time_s
                        .map(|v| format!("{v:.2} s"))
                        .unwrap_or_else(|| "not within grace".into()),
                ]);
            }
        }
    }
    table(
        l,
        &[1.3, 1.0, 1.0, 1.0, 0.7, 0.9, 1.0, 1.1],
        &[
            "Recording",
            "Chain",
            "Design",
            "Satellite",
            "Event",
            "Time to loss",
            "J/S at loss",
            "Re-acq.",
        ],
        &rows,
    );
}
