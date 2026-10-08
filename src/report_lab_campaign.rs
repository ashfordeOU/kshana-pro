//! # Lab campaign report: the branded, partner-ready face of a lab-replay campaign
//!
//! Renders a [`LoadedCampaign`](crate::lab_campaign::LoadedCampaign), the engine's
//! `kshana iq campaign` output after every check in [`crate::lab_campaign::load`], as one
//! self-contained Ashforde HTML document. Plan: `docs/plans/2026-10-07-lab-campaign-report-plan.md`.
//!
//! Rules this renderer keeps:
//! - **It recomputes no metric.** Every number comes from a cell, or from a declared
//!   aggregate in [`crate::lab_campaign::aggregate`]. Each number in a data table carries
//!   `data-v` (the exact value), `data-op` (`cell`, `median`, `sum`, `count`, `ratio`) and
//!   `data-n` (the inputs). That lets a test trace every printed number back to the cells.
//! - **Every figure is labelled.** MEASURED means loop response on the supplied recordings.
//!   MODELLED is the analytic C/N0-vs-J/S reference curve, which is never called validated.
//! - **The data class is on the cover and on every page.** A campaign that declares a class
//!   must match the one requested. A client-confidential render carries the export-control
//!   notice slot.
//! - **The output is deterministic.** No clock is read and the order is fixed: the same
//!   campaign and options give the same bytes.
//! - **No client name is ever defaulted.** It comes only from [`LabReportOptions::client`].

use crate::lab_campaign::aggregate::{
    aggregate, rank, Aggregates, Curve, EventRow, Op, RelaxedBar, Stat,
};
use crate::lab_campaign::model::DataClass;
use crate::lab_campaign::LoadedCampaign;
use crate::report_brand as brand;
use std::fmt::Write as _;

/// Placeholder shown in the export-control slot until counsel supplies the wording (N6).
pub const EXPORT_NOTICE_PLACEHOLDER: &str =
    "Export-control notice: wording pending review by export-control counsel (item N6).";

/// Categorical colours for loop designs, in fixed order (colour follows the design).
/// A validated colour-vision-safe set; more designs than slots fall back to tables only.
const DESIGN_COLOURS: [&str; 8] = [
    "#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948",
];
/// The neutral used for the MODELLED reference line.
const MODELLED_GREY: &str = "#6f6a60";

/// Render options. `data_class` is mandatory; everything else is optional and never
/// defaulted to a client-identifying value.
#[derive(Debug, Clone)]
pub struct LabReportOptions {
    pub data_class: DataClass,
    /// The client the report is prepared for (render-time only; never stored in the repo).
    pub client: Option<String>,
    /// Document reference; defaults to the content hash of the rendered body.
    pub doc_id: Option<String>,
    /// Preparation time (UTC, ISO-8601), passed in; never read from a clock.
    pub prepared_utc: Option<String>,
    /// Export-control notice text for client-confidential renders. `None` shows the
    /// placeholder until counsel has supplied the wording.
    pub export_notice: Option<String>,
    /// Allow rendering a campaign with missing cells (then marked INCOMPLETE throughout).
    pub allow_incomplete: bool,
}

impl LabReportOptions {
    pub fn new(data_class: DataClass) -> Self {
        LabReportOptions {
            data_class,
            client: None,
            doc_id: None,
            prepared_utc: None,
            export_notice: None,
            allow_incomplete: false,
        }
    }
}

/// Why a report was not rendered.
#[derive(Debug, Clone, PartialEq)]
pub enum RenderError {
    /// The campaign states a data class different from the one requested.
    DataClassMismatch {
        campaign: DataClass,
        requested: DataClass,
    },
    /// Cells are missing and `allow_incomplete` was not set.
    Incomplete { missing: usize },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::DataClassMismatch {
                campaign,
                requested,
            } => write!(
                f,
                "the campaign is {} but {} was requested",
                campaign.label(),
                requested.label()
            ),
            RenderError::Incomplete { missing } => write!(
                f,
                "{missing} cell(s) missing; finish the campaign or pass --allow-incomplete"
            ),
        }
    }
}

impl std::error::Error for RenderError {}

/// The checks both editions make before rendering: the data class must match the campaign's
/// own, and a campaign with missing cells needs `allow_incomplete`.
pub(crate) fn check(c: &LoadedCampaign, opts: &LabReportOptions) -> Result<(), RenderError> {
    if c.data_class() != opts.data_class {
        return Err(RenderError::DataClassMismatch {
            campaign: c.data_class(),
            requested: opts.data_class,
        });
    }
    if !c.is_complete() && !opts.allow_incomplete {
        return Err(RenderError::Incomplete {
            missing: c.missing.len(),
        });
    }
    Ok(())
}

/// Render the branded HTML report.
pub fn render_html(c: &LoadedCampaign, opts: &LabReportOptions) -> Result<String, RenderError> {
    check(c, opts)?;
    let a = aggregate(c);
    let body = body_html(c, &a, opts);
    let id = opts
        .doc_id
        .clone()
        .unwrap_or_else(|| brand::content_id(&body));
    Ok(brand::finish_html(
        &body,
        &brand::DocMark::new("Lab campaign report", id),
    ))
}

// ───────────────────────────── formatting ─────────────────────────────

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Shortest round-trip text of a value (the `data-v` attribute).
fn raw(v: f64) -> String {
    format!("{v}")
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Median => "median",
        Op::Sum => "sum",
        Op::Count => "count",
        Op::RatioOfSums => "ratio",
    }
}

/// The relaxed per-recording bars that apply to `rec`.
pub(crate) fn relaxed_for<'a>(a: &'a Aggregates, rec: &str) -> Vec<&'a RelaxedBar> {
    a.relaxed.iter().filter(|r| r.recording == rec).collect()
}

/// Plain text of the relaxed-bar note for `rec` (shared by the HTML and the PDF), if any.
pub(crate) fn relaxed_text(a: &Aggregates, rec: &str) -> Option<String> {
    let v = relaxed_for(a, rec);
    (!v.is_empty()).then(|| {
        format!(
            "RELAXED BAR: {}.",
            v.iter()
                .map(|r| r.describe())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })
}

/// The note shown next to the rows of a recording whose own bar is looser than the campaign's.
fn relaxed_notice(a: &Aggregates, rec: &str) -> String {
    relaxed_text(a, rec)
        .map(|t| format!("<div class=\"notice relaxed\">{}</div>", xml(&t)))
        .unwrap_or_default()
}

/// Marker delimiters for a traced number inside summary prose. The summary text is shared by
/// the HTML and the PDF; the HTML turns a marker into a `<span>` carrying the trace
/// attributes, the PDF strips it to the printed text.
const MARK_OPEN: char = '\u{1}';
const MARK_SEP: char = '\u{2}';
const MARK_CLOSE: char = '\u{3}';

/// `text` printed in prose, traced as `value` obtained by `op` over `n` inputs.
pub(crate) fn tr(text: &str, value: f64, op: &str, n: usize) -> String {
    format!(
        "{MARK_OPEN}{}|{op}|{n}{MARK_SEP}{text}{MARK_CLOSE}",
        raw(value)
    )
}

/// `text` traced to a derived [`Stat`]; plain text when the stat has no value.
pub(crate) fn tr_stat(s: &Stat, text: &str) -> String {
    match s.value {
        Some(v) => tr(text, v, op_name(s.op), s.n),
        None => text.to_owned(),
    }
}

/// Turn the markers of an XML-escaped string into traced `<span>`s.
pub(crate) fn expand_markers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(MARK_OPEN) {
        out.push_str(&rest[..i]);
        let body = &rest[i + MARK_OPEN.len_utf8()..];
        let (Some(sep), Some(close)) = (body.find(MARK_SEP), body.find(MARK_CLOSE)) else {
            break;
        };
        let attrs: Vec<&str> = body[..sep].splitn(3, '|').collect();
        let _ = write!(
            out,
            "<span data-v=\"{}\" data-op=\"{}\" data-n=\"{}\">{}</span>",
            attrs[0],
            attrs[1],
            attrs[2],
            &body[sep + MARK_SEP.len_utf8()..close]
        );
        rest = &body[close + MARK_CLOSE.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// The printed text of a string with markers.
pub(crate) fn strip_markers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(MARK_OPEN) {
        out.push_str(&rest[..i]);
        let body = &rest[i + MARK_OPEN.len_utf8()..];
        let (Some(sep), Some(close)) = (body.find(MARK_SEP), body.find(MARK_CLOSE)) else {
            break;
        };
        out.push_str(&body[sep + MARK_SEP.len_utf8()..close]);
        rest = &body[close + MARK_CLOSE.len_utf8()..];
    }
    out.push_str(rest);
    out
}

/// A table cell for a derived value, with its trace attributes. `none` is the word printed
/// when there is no value (e.g. "held lock").
fn td_stat(s: &Stat, decimals: usize, unit: &str, none: &str) -> String {
    match s.value {
        Some(v) => format!(
            "<td class=\"num\" data-v=\"{}\" data-op=\"{}\" data-n=\"{}\">{:.*}{}</td>",
            raw(v),
            op_name(s.op),
            s.n,
            decimals,
            v,
            unit
        ),
        None => format!(
            "<td class=\"none\" data-op=\"{}\" data-n=\"{}\">{}</td>",
            op_name(s.op),
            s.n,
            xml(none)
        ),
    }
}

/// A table cell for a fraction copied from a cell field, printed as a percentage.
fn td_cell_pct(v: Option<f64>) -> String {
    let Some(v) = v else {
        return "<td class=\"none\" data-op=\"cell\" data-n=\"1\">no data</td>".to_owned();
    };
    format!(
        "<td class=\"num\" data-v=\"{}\" data-op=\"cell\" data-n=\"1\">{:.1}&#8239;%</td>",
        raw(v),
        100.0 * v
    )
}

/// A table cell for a value copied from a cell field.
fn td_cell(v: Option<f64>, decimals: usize, unit: &str, none: &str) -> String {
    match v {
        Some(v) => format!(
            "<td class=\"num\" data-v=\"{}\" data-op=\"cell\" data-n=\"1\">{:.*}{}</td>",
            raw(v),
            decimals,
            v,
            unit
        ),
        None => format!(
            "<td class=\"none\" data-op=\"cell\" data-n=\"1\">{}</td>",
            xml(none)
        ),
    }
}

fn pct(s: &Stat) -> String {
    match s.value {
        Some(v) => format!(
            "<td class=\"num\" data-v=\"{}\" data-op=\"{}\" data-n=\"{}\">{:.1}&#8239;%</td>",
            raw(v),
            op_name(s.op),
            s.n,
            100.0 * v
        ),
        None => format!(
            "<td class=\"none\" data-op=\"{}\" data-n=\"{}\">no data</td>",
            op_name(s.op),
            s.n
        ),
    }
}

fn short(h: &str) -> String {
    h.chars().take(12).collect()
}

fn design_colour(a: &Aggregates, design: &str) -> Option<&'static str> {
    a.designs
        .iter()
        .position(|d| d == design)
        .and_then(|i| DESIGN_COLOURS.get(i).copied())
}

fn swatch(a: &Aggregates, design: &str) -> String {
    match design_colour(a, design) {
        Some(c) => format!("<span class=\"sw\" style=\"background:{c}\"></span>"),
        None => String::new(),
    }
}

fn names(v: &[String]) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].clone(),
        _ => format!("{} and {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

// ───────────────────────────── page ─────────────────────────────

const CSS: &str = "\
.lab h2{font-family:var(--lab-head);font-size:24px;margin:34px 0 8px;border-bottom:1px solid var(--lab-line);padding-bottom:4px}\
.lab h3{font-family:var(--lab-head);font-size:18px;margin:20px 0 6px}\
.lab table{border-collapse:collapse;width:100%;margin:8px 0 14px;font-size:13px}\
.lab th,.lab td{border-bottom:1px solid var(--lab-line);padding:4px 6px;text-align:left;vertical-align:top}\
.lab th{font-weight:700;color:var(--lab-ink2)}\
.lab td.num{text-align:right;font-family:var(--lab-mono);white-space:nowrap}\
.lab td.none{text-align:right;font-style:italic;color:var(--lab-ink2)}\
.lab .tag{display:inline-block;font:700 10px/1.6 var(--lab-mono);letter-spacing:.06em;padding:0 5px;border:1px solid currentColor;border-radius:2px;margin-right:4px}\
.lab .measured{color:#1c5cab}.lab .modelled{color:#7d6636}\
.lab .banner{font-weight:700;text-align:center;padding:10px 14px;border-radius:4px;margin:16px 0;letter-spacing:.03em}\
.lab .synthetic{background:#e6eef8;color:#163f73;border:1px solid #9ec5f4}\
.lab .confidential{background:#f8e7e3;color:#7a2416;border:1px solid #d9a092}\
.lab .incomplete,.lab .relaxed{background:#fff3cd;color:#664d03;border:1px solid #e0c36a}\
.lab .notice{border:1px dashed var(--lab-line);padding:8px 12px;font-size:13px;margin:8px 0}\
.lab .sw{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:6px;vertical-align:baseline}\
.lab .mono{font-family:var(--lab-mono);font-size:12px;word-break:break-all}\
.lab figure{margin:10px 0 18px}.lab figcaption{font-size:12px;color:var(--lab-ink2);margin-top:4px}\
.lab ul.caveats li{margin:3px 0}\
@media print{.lab h2{break-after:avoid}.lab table,.lab figure{break-inside:avoid}}";

fn body_html(c: &LoadedCampaign, a: &Aggregates, o: &LabReportOptions) -> String {
    let mut s = String::with_capacity(64 * 1024);
    let class_banner = class_banner(o);
    let incomplete = (!c.is_complete()).then(|| {
        format!(
            "<div class=\"banner incomplete\">INCOMPLETE CAMPAIGN: {} of {} cells present. \
             Figures cover the cells present only.</div>",
            c.cells.len(),
            c.manifest.cells.len()
        )
    });
    let _ = write!(
        s,
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>Lab campaign report: {name}</title>\
         <style>.lab{{--lab-head:{head};--lab-mono:{mono};--lab-line:{line};--lab-ink2:{ink2}}}{css}</style>\
         </head><body style=\"background:{paper};color:{ink};font-family:{body};\
         max-width:1040px;margin:auto;padding:24px 32px 48px\"><main class=\"lab\">",
        name = xml(&c.manifest.name),
        head = brand::FONT_HEAD,
        mono = brand::FONT_MONO,
        line = brand::LINE,
        ink2 = brand::INK_2,
        css = CSS,
        paper = brand::PAPER,
        ink = brand::INK,
        body = brand::FONT_BODY,
    );
    s.push_str(&brand::cover_html(
        "Lab campaign report",
        &cover_subtitle(c, o),
    ));
    s.push_str(&class_banner);
    if let Some(b) = &incomplete {
        s.push_str(b);
    }
    s.push_str(&cover_facts(c, o));
    s.push_str(&exec_summary(c, a));
    s.push_str(&test_matrix(c, a));
    s.push_str(&scorecard(a));
    s.push_str(&events_section(a));
    s.push_str(&curves_section(a));
    s.push_str(&stability_section(a));
    s.push_str(&pass_fail_section(c, a));
    s.push_str(&method_section(c));
    s.push_str(&provenance_section(c));
    s.push_str(&limits_section(o));
    s.push_str(&appendix(c));
    s.push_str(&class_banner);
    s.push_str("</main></body></html>");
    s
}

fn class_banner(o: &LabReportOptions) -> String {
    match o.data_class {
        DataClass::Synthetic => "<div class=\"banner synthetic\">SYNTHETIC CAMPAIGN: Kshana-generated \
            scenes, not a client's recordings. The numbers illustrate the method; they are not a \
            measurement of any receiver.</div>"
            .to_owned(),
        DataClass::ClientConfidential => format!(
            "<div class=\"banner confidential\">CLIENT-CONFIDENTIAL: results on recordings supplied \
             by the client. Do not forward.</div><div class=\"notice\">{}</div>",
            xml(o.export_notice.as_deref().unwrap_or(EXPORT_NOTICE_PLACEHOLDER))
        ),
    }
}

fn cover_subtitle(c: &LoadedCampaign, o: &LabReportOptions) -> String {
    match &o.client {
        Some(client) => format!("Campaign {} · prepared for {}", c.manifest.name, client),
        None => format!("Campaign {}", c.manifest.name),
    }
}

fn cover_facts(c: &LoadedCampaign, o: &LabReportOptions) -> String {
    let mut rows = vec![
        ("Campaign", xml(&c.manifest.name)),
        ("Data class", o.data_class.label().to_owned()),
        (
            "Engine that produced the cells",
            format!("Kshana {}", xml(&c.manifest.engine_version)),
        ),
        ("Report produced with", xml(&brand::produced_with())),
        (
            "Cells",
            format!(
                "{} of {} present ({})",
                c.cells.len(),
                c.manifest.cells.len(),
                if c.is_complete() {
                    "complete"
                } else {
                    "INCOMPLETE"
                }
            ),
        ),
        (
            "DIGEST (verified)",
            format!("<span class=\"mono\">{}</span>", xml(&c.digest)),
        ),
    ];
    if let Some(client) = &o.client {
        rows.insert(1, ("Prepared for", xml(client)));
    }
    if let Some(t) = &o.prepared_utc {
        rows.push(("Prepared (UTC)", xml(t)));
    }
    let mut s = String::from("<table>");
    for (k, v) in rows {
        let _ = write!(s, "<tr><th style=\"width:28%\">{k}</th><td>{v}</td></tr>");
    }
    s.push_str("</table>");
    s
}

// ───────────────────────────── §1 summary ─────────────────────────────

/// The summary, as plain text shared by the HTML and PDF editions so they can never differ.
pub(crate) struct Summary {
    pub(crate) intro: String,
    /// `(event and chain, finding)` per event and front-end chain.
    pub(crate) items: Vec<(String, String)>,
    pub(crate) bars: Option<String>,
}

/// The three caveats under every summary: `(label, text)`.
pub(crate) const CAVEATS: [(&str, &str); 3] = [
    (
        "MEASURED",
        "loop response of Kshana's software receiver on the supplied recordings, scored against \
         the stated test conditions. It describes these loop designs, not the receiver that was \
         recorded.",
    ),
    (
        "MODELLED",
        "the C/N0-vs-J/S reference curve is an analytic spectral-separation model, shown for \
         comparison only. It is not a validation and not a simulation of any jammer.",
    ),
    (
        "",
        "\"Loss of lock\" follows each design's lock state machine and dwell times (section 8). \
         Results hold for the recordings and conditions in section 2 only.",
    ),
];

/// A count printed in prose: its own value over as many inputs.
fn count_tr(n: usize) -> String {
    tr(&n.to_string(), n as f64, "count", n)
}

/// Build the summary from fixed sentence templates over the aggregates.
pub(crate) fn summary(c: &LoadedCampaign, a: &Aggregates) -> Summary {
    let intro = format!(
        "This campaign replayed {r} recording(s) through {f} front-end chain(s) and {d} loop \
         design(s): {n} cell(s){inc}. Each cell is one recording processed by one chain and one \
         design; every figure below is read from those cells.",
        r = count_tr(c.manifest.recordings.len()),
        f = count_tr(a.frontends.len()),
        d = count_tr(a.designs.len()),
        n = count_tr(c.cells.len()),
        inc = if c.is_complete() {
            ""
        } else {
            ", campaign INCOMPLETE"
        },
    );
    let mut keys: Vec<(String, String, String)> = Vec::new();
    for e in &a.events {
        let k = (
            e.recording.clone(),
            e.event_id.clone(),
            format!("{} / {}", e.kind, e.event_type),
        );
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    let mut items = Vec::new();
    for (rec, ev, kind) in &keys {
        for fe in &a.frontends {
            let rows: Vec<&EventRow> = a
                .events
                .iter()
                .filter(|r| &r.recording == rec && &r.event_id == ev && &r.frontend == fe)
                .collect();
            if rows.is_empty() {
                continue;
            }
            let head = format!(
                "{ev} ({kind}, recording {rec}), chain {fe}{}",
                if relaxed_for(a, rec).is_empty() {
                    ""
                } else {
                    " [recording's own bar is relaxed, see section 7]"
                }
            );
            let mut body = String::new();
            // Longest time to loss; satellites that held lock count as longest.
            let ttl: Vec<(String, Option<f64>)> = rows
                .iter()
                .map(|r| {
                    let v = if r.held == r.satellites {
                        Some(f64::INFINITY)
                    } else {
                        r.time_to_loss_s.value
                    };
                    (r.design.clone(), v)
                })
                .collect();
            let best = top(&rank(&ttl, true));
            let all = best.len() == rows.len() && rows.len() > 1;
            let b0 = rows.iter().find(|r| r.design == best[0]).unwrap();
            if b0.held == b0.satellites {
                let who = if all {
                    "every design".to_owned()
                } else {
                    names(&best)
                };
                let _ = write!(body, "{who} held lock throughout");
            } else if all {
                let _ = write!(
                    body,
                    "every design lost lock after the same median time ({} s after onset)",
                    tr_stat(
                        &b0.time_to_loss_s,
                        &format!("{:.2}", b0.time_to_loss_s.value.unwrap_or(f64::NAN))
                    )
                );
            } else {
                let _ = write!(
                    body,
                    "{} held lock longest before loss (median {} s after onset{})",
                    names(&best),
                    tr_stat(
                        &b0.time_to_loss_s,
                        &format!("{:.2}", b0.time_to_loss_s.value.unwrap_or(f64::NAN))
                    ),
                    match b0.js_at_loss_db.value {
                        Some(js) => format!(
                            ", at stated J/S {} dB",
                            tr_stat(&b0.js_at_loss_db, &format!("{js:.1}"))
                        ),
                        None => String::new(),
                    }
                );
            }
            let re: Vec<(String, Option<f64>)> = rows
                .iter()
                .map(|r| (r.design.clone(), r.reacq_time_s.value))
                .collect();
            let fastest = top(&rank(&re, false));
            if let Some(r0) = rows.iter().find(|r| r.design == fastest[0]) {
                if let Some(v) = r0.reacq_time_s.value {
                    let t = tr_stat(&r0.reacq_time_s, &format!("{v:.2}"));
                    if fastest.len() == rows.len() && rows.len() > 1 {
                        let _ = write!(
                            body,
                            "; every design re-acquired after the same median time ({t} s)"
                        );
                    } else {
                        let _ = write!(
                            body,
                            "; {} re-acquired fastest (median {t} s)",
                            names(&fastest)
                        );
                    }
                }
            }
            body.push('.');
            items.push((head, body));
        }
    }
    let (passed, evaluated) = a
        .scorecard
        .iter()
        .map(|r| r.verdicts)
        .chain(a.events.iter().map(|r| r.verdicts))
        .fold((0, 0), |(p, e), (bp, be)| (p + bp, e + be));
    let bars = (evaluated > 0).then(|| {
        format!(
            "Against the client's stated bars, the engine's verdicts: {} of {} \
             checks passed (whole-run and per-event, per satellite; section 7).",
            tr(&passed.to_string(), passed as f64, "count", evaluated),
            tr(&evaluated.to_string(), evaluated as f64, "count", evaluated),
        )
    });
    Summary { intro, items, bars }
}

fn exec_summary(c: &LoadedCampaign, a: &Aggregates) -> String {
    let sm = summary(c, a);
    let mut s = format!(
        "<h2>1. Summary</h2><p>{}</p><ul>",
        expand_markers(&xml(&sm.intro))
    );
    for (head, body) in &sm.items {
        let _ = write!(
            s,
            "<li><strong>{}</strong>: {}</li>",
            xml(head),
            expand_markers(&xml(body))
        );
    }
    if let Some(b) = &sm.bars {
        let _ = write!(s, "<li>{}</li>", expand_markers(&xml(b)));
    }
    s.push_str("</ul><ul class=\"caveats\">");
    for (tag, text) in CAVEATS {
        let t = match tag {
            "MEASURED" => "<span class=\"tag measured\">MEASURED</span>",
            "MODELLED" => "<span class=\"tag modelled\">MODELLED</span>",
            _ => "",
        };
        let _ = write!(s, "<li>{t}{}</li>", xml(text));
    }
    s.push_str("</ul>");
    s
}

fn top(r: &[(String, usize)]) -> Vec<String> {
    r.iter()
        .filter(|(_, k)| *k == 1)
        .map(|(n, _)| n.clone())
        .collect()
}

// ───────────────────────────── §2 matrix ─────────────────────────────

fn js_range(ev: &serde_json::Value) -> String {
    let pts = ev.pointer("/power/points").and_then(|p| p.as_array());
    let q = ev
        .pointer("/power/quantity")
        .and_then(|q| q.as_str())
        .unwrap_or("");
    match pts {
        Some(p) if !p.is_empty() => {
            let vals: Vec<f64> = p
                .iter()
                .filter_map(|x| x.get(1).and_then(|v| v.as_f64()))
                .collect();
            let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let unit = if q == "jammer_dbm" {
                "dBm (jammer)"
            } else {
                "dB J/S"
            };
            if lo == hi {
                format!("{lo} {unit}")
            } else {
                format!("{lo} to {hi} {unit}")
            }
        }
        _ => "none stated".to_owned(),
    }
}

/// `v` without the named top-level keys (identity fields the table shows separately).
pub(crate) fn without(v: &serde_json::Value, keys: &[&str]) -> serde_json::Value {
    match v.as_object() {
        Some(m) => serde_json::Value::Object(
            m.iter()
                .filter(|(k, _)| !keys.contains(&k.as_str()))
                .map(|(k, x)| (k.clone(), x.clone()))
                .collect(),
        ),
        None => v.clone(),
    }
}

/// Flatten nested parameters to `a.b=value` pairs (objects one level deep at most per key).
fn params_text(v: &serde_json::Value) -> String {
    let mut out = Vec::new();
    flatten("", v, &mut out);
    if out.is_empty() {
        "(defaults)".to_owned()
    } else {
        out.join(", ")
    }
}

fn flatten(prefix: &str, v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, x) in m {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&p, x, out);
            }
        }
        serde_json::Value::Null => {}
        other => out.push(format!("{prefix}={}", other.to_string().trim_matches('"'))),
    }
}

fn test_matrix(c: &LoadedCampaign, a: &Aggregates) -> String {
    let mut s = String::from(
        "<h2>2. Test matrix</h2><h3>Recordings and stated test conditions</h3>\
         <table><tr><th>Recording</th><th>Receiver (as stated)</th><th>Expected</th>\
         <th>Event</th><th>Kind / type</th><th>Window (s)</th><th>Stated power</th></tr>",
    );
    for r in &c.manifest.recordings {
        let cond = &r.conditions;
        let dut = cond
            .pointer("/receiver/dut")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let expected = cond
            .get("expected")
            .and_then(|e| e.as_array())
            .map(|v| {
                v.iter()
                    .map(|x| {
                        format!(
                            "{} {}",
                            x.get("signal").and_then(|s| s.as_str()).unwrap_or("?"),
                            x.get("ids").map(|i| i.to_string()).unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
        let events: Vec<&serde_json::Value> = cond
            .get("event")
            .and_then(|e| e.as_array())
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        let span = events.len().max(1);
        let _ = write!(
            s,
            "<tr><td rowspan=\"{span}\">{}</td><td rowspan=\"{span}\">{}</td><td rowspan=\"{span}\">{}</td>",
            xml(&r.id),
            xml(dut),
            xml(&expected)
        );
        if events.is_empty() {
            s.push_str(
                "<td colspan=\"4\"><em>no stated events (whole-run metrics only)</em></td></tr>",
            );
        }
        for (i, ev) in events.iter().enumerate() {
            if i > 0 {
                s.push_str("<tr>");
            }
            let g = |k: &str| {
                ev.get(k)
                    .map(|v| v.to_string().trim_matches('"').to_owned())
                    .unwrap_or_default()
            };
            let _ = write!(
                s,
                "<td>{}</td><td>{} / {}</td><td class=\"num\">{}–{}</td><td>{}</td></tr>",
                xml(&g("id")),
                xml(&g("kind")),
                xml(&g("type")),
                xml(&g("onset_s")),
                xml(&g("offset_s")),
                xml(&js_range(ev))
            );
        }
    }
    s.push_str("</table><h3>Front-end chains</h3><table><tr><th>Chain</th><th>Parameters</th><th>Hash</th></tr>");
    for (spec, hash) in &c.manifest.frontends {
        let name = spec.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let _ = write!(
            s,
            "<tr><td>{}</td><td>{}</td><td class=\"mono\">{}</td></tr>",
            xml(name),
            xml(&params_text(&without(spec, &["name"]))),
            xml(&short(hash))
        );
    }
    s.push_str("</table><h3>Loop designs</h3><table><tr><th>Design</th><th>Parameters</th><th>Hash</th></tr>");
    for d in &c.manifest.designs {
        let name = d.get("name").and_then(|n| n.as_str()).unwrap_or("?");
        let hash = d.get("hash").and_then(|n| n.as_str()).unwrap_or("");
        let _ = write!(
            s,
            "<tr><td>{}{}</td><td>{}</td><td class=\"mono\">{}</td></tr>",
            swatch(a, name),
            xml(name),
            xml(&params_text(&without(d, &["name", "hash", "schema"]))),
            xml(&short(hash))
        );
    }
    s.push_str("</table>");
    s
}

// ───────────────────────────── §3 scorecard ─────────────────────────────

fn scorecard(a: &Aggregates) -> String {
    let mut s = String::from(
        "<h2>3. Scorecard (whole run)</h2>\
         <p><span class=\"tag measured\">MEASURED</span>Per front-end chain and loop design, over every \
         recording and satellite. Availability is time in lock over time scored; the median is over \
         satellites. False locks are episodes over locked hours, summed across satellites.</p>",
    );
    s.push_str(&heatmap(a));
    s.push_str(
        "<table><tr><th>Chain</th><th>Design</th><th>Satellites</th><th>Availability</th>\
         <th>Phase lock</th><th>Losses</th><th>False locks / locked h</th><th>PLL jitter</th>\
         <th>DLL jitter</th></tr>",
    );
    for r in &a.scorecard {
        let _ = write!(
            s,
            "<tr><td>{}</td><td>{}{}</td><td class=\"num\">{}</td>{}{}{}{}{}{}</tr>",
            xml(&r.frontend),
            swatch(a, &r.design),
            xml(&r.design),
            r.satellites,
            pct(&r.availability),
            pct(&r.phase_lock_frac),
            td_stat(&r.losses, 0, "", "no data"),
            td_stat(&r.false_lock_per_locked_hour, 2, "", "no locked time"),
            td_stat(&r.pll_jitter_deg, 2, "&#8239;°", "no data"),
            td_stat(&r.dll_jitter_chips, 3, "&#8239;chip", "no data"),
        );
    }
    s.push_str("</table>");
    for rb in &a.relaxed {
        let _ = write!(
            s,
            "<div class=\"notice relaxed\">RELAXED BAR (whole-run rows pool every recording): {}.</div>",
            xml(&rb.describe())
        );
    }
    s
}

/// A single-hue heatmap of median availability, chain (rows) by design (columns), values
/// printed in every cell.
fn heatmap(a: &Aggregates) -> String {
    let (cw, ch, lw, th) = (130.0, 34.0, 120.0, 26.0);
    let w = lw + cw * a.designs.len() as f64;
    let h = th + ch * a.frontends.len() as f64;
    let vals: Vec<f64> = a
        .scorecard
        .iter()
        .filter_map(|r| r.availability.value)
        .collect();
    let lo = vals.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut s = format!(
        "<figure><svg viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" \
         aria-label=\"Median availability by front-end chain and loop design\" \
         style=\"max-width:100%;font-family:{mono};font-size:12px\">",
        mono = brand::FONT_MONO
    );
    for (j, d) in a.designs.iter().enumerate() {
        let _ = write!(
            s,
            "<text x=\"{}\" y=\"17\" text-anchor=\"middle\" fill=\"{}\">{}</text>",
            lw + cw * (j as f64 + 0.5),
            brand::INK_2,
            xml(d)
        );
    }
    for (i, f) in a.frontends.iter().enumerate() {
        let y = th + ch * i as f64;
        let _ = write!(
            s,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"{}\">{}</text>",
            lw - 8.0,
            y + ch / 2.0 + 4.0,
            brand::INK_2,
            xml(f)
        );
        for (j, d) in a.designs.iter().enumerate() {
            let x = lw + cw * j as f64;
            let stat = a
                .scorecard
                .iter()
                .find(|r| &r.frontend == f && &r.design == d)
                .map(|r| r.availability);
            let v = stat.as_ref().and_then(|s| s.value);
            let attrs = match &stat {
                Some(st) => match st.value {
                    Some(v) => format!(
                        " data-v=\"{}\" data-op=\"{}\" data-n=\"{}\"",
                        raw(v),
                        op_name(st.op),
                        st.n
                    ),
                    None => String::new(),
                },
                None => String::new(),
            };
            let (fill, ink, txt) = match v {
                Some(v) => {
                    // Light-to-dark blue over the observed range (mid tone if all equal).
                    let t = if hi > lo {
                        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
                    } else {
                        0.5
                    };
                    let l = 92.0 - 52.0 * t;
                    (
                        format!("hsl(213,62%,{l:.0}%)"),
                        if l < 60.0 { "#ffffff" } else { brand::INK },
                        format!("{:.1} %", 100.0 * v),
                    )
                }
                None => (
                    brand::PARCHMENT.to_owned(),
                    brand::INK_2,
                    "no data".to_owned(),
                ),
            };
            let _ = write!(
                s,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"{fill}\"/>\
                 <text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{ink}\"{attrs}>{txt}</text>",
                x + 2.0,
                y + 2.0,
                cw - 4.0,
                ch - 4.0,
                x + cw / 2.0,
                y + ch / 2.0 + 4.0
            );
        }
    }
    s.push_str(
        "</svg><figcaption>Median whole-run availability (MEASURED) per chain and design; darker is \
         higher, and the shading spans the observed range only. The table below gives the same \
         values.</figcaption></figure>",
    );
    s
}

// ───────────────────────────── §4 events ─────────────────────────────

fn events_section(a: &Aggregates) -> String {
    let mut s = String::from(
        "<h2>4. Results per stated event</h2>\
         <p><span class=\"tag measured\">MEASURED</span>For each event the lab stated, per chain and \
         design, over the satellites the event affected. Time to loss is counted from the event onset; \
         re-acquisition from the end of the event (or the loss, if later). Medians are over the \
         satellites that lost lock.</p>",
    );
    let mut seen: Vec<(String, String)> = Vec::new();
    for e in &a.events {
        let k = (e.recording.clone(), e.event_id.clone());
        if seen.contains(&k) {
            continue;
        }
        seen.push(k.clone());
        let _ = write!(
            s,
            "<h3>{} ({}), recording {}</h3>{}<table><tr><th>Chain</th><th>Design</th><th>Satellites</th>\
             <th>Lost</th><th>Time to loss</th><th>Stated J/S at loss</th><th>Re-acquired</th>\
             <th>Re-acquisition time</th><th>Availability in event</th></tr>",
            xml(&e.event_id),
            xml(&format!("{} / {}", e.kind, e.event_type)),
            xml(&e.recording),
            relaxed_notice(a, &e.recording)
        );
        for r in a
            .events
            .iter()
            .filter(|r| r.recording == k.0 && r.event_id == k.1)
        {
            let _ = write!(
                s,
                "<tr><td>{}</td><td>{}{}</td><td class=\"num\">{}</td>{}{}{}{}{}{}</tr>",
                xml(&r.frontend),
                swatch(a, &r.design),
                xml(&r.design),
                r.satellites,
                td_stat(&r.lost, 0, "", "–"),
                td_stat(&r.time_to_loss_s, 2, "&#8239;s", "held lock"),
                td_stat(&r.js_at_loss_db, 1, "&#8239;dB", "no J/S stated"),
                td_stat(&r.reacquired, 0, "", "–"),
                td_stat(&r.reacq_time_s, 2, "&#8239;s", "not within grace"),
                pct(&r.availability),
            );
        }
        s.push_str("</table>");
    }
    if seen.is_empty() {
        s.push_str("<p><em>No stated events: whole-run metrics only (section 3).</em></p>");
    }
    s
}

// ───────────────────────────── §5 curves ─────────────────────────────

fn curves_section(a: &Aggregates) -> String {
    let mut s = String::from(
        "<h2>5. C/N0 degradation against stated J/S</h2>\
         <p><span class=\"tag measured\">MEASURED</span>median C/N0 degradation (baseline minus \
         measured, locked epochs only) per J/S bin, median over satellites, per design. \
         <span class=\"tag modelled\">MODELLED</span>the analytic spectral-separation reference \
         (dashed, grey), median over designs, shown for comparison only. Bins with fewer than 50 \
         locked epochs are hollow.</p>",
    );
    if a.curves.is_empty() {
        s.push_str(
            "<p><em>No event in this campaign states a J/S profile; no curve is drawn.</em></p>",
        );
        return s;
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
        let group: Vec<&Curve> = a
            .curves
            .iter()
            .filter(|c| c.recording == k.0 && c.event_id == k.1 && c.frontend == k.2)
            .collect();
        let _ = write!(
            s,
            "<h3>{} (recording {}), chain {}</h3>{}",
            xml(&k.1),
            xml(&k.0),
            xml(&k.2),
            relaxed_notice(a, &k.0)
        );
        s.push_str(&curve_svg(a, &group));
        s.push_str(&curve_table(a, &group));
    }
    s
}

fn curve_svg(a: &Aggregates, group: &[&Curve]) -> String {
    let (w, h, ml, mr, mt, mb) = (640.0, 300.0, 56.0, 16.0, 14.0, 42.0);
    let xs: Vec<f64> = group
        .iter()
        .flat_map(|c| c.bins.iter().map(|b| b.js_db))
        .collect();
    let ys: Vec<f64> = group
        .iter()
        .flat_map(|c| c.bins.iter())
        .flat_map(|b| {
            [
                b.measured_degradation_db.value,
                b.modelled_degradation_db.value,
            ]
        })
        .flatten()
        .collect();
    let (x0, x1) = (
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    );
    let y1 = ys.iter().cloned().fold(0.0, f64::max).max(1.0) * 1.1;
    let x1 = if x1 > x0 { x1 } else { x0 + 1.0 };
    let px = |x: f64| ml + (x - x0) / (x1 - x0) * (w - ml - mr);
    let py = |y: f64| mt + (1.0 - y / y1) * (h - mt - mb);
    let mut s = format!(
        "<figure><svg viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" \
         aria-label=\"C/N0 degradation against stated J/S\" style=\"max-width:100%;font-family:{};font-size:11px\">",
        brand::FONT_MONO
    );
    // Axes and recessive grid.
    for k in 0..=4 {
        let y = y1 * k as f64 / 4.0;
        let _ = write!(
            s,
            "<line x1=\"{ml}\" x2=\"{}\" y1=\"{py}\" y2=\"{py}\" stroke=\"{}\" stroke-width=\"0.6\"/>\
             <text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"{}\">{:.1}</text>",
            w - mr,
            brand::LINE,
            ml - 6.0,
            py(y) + 4.0,
            brand::INK_2,
            y,
            py = py(y)
        );
    }
    for x in xs.iter().fold(Vec::<f64>::new(), |mut v, x| {
        if !v.contains(x) {
            v.push(*x)
        }
        v
    }) {
        let _ = write!(
            s,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{}\">{}</text>",
            px(x),
            h - mb + 16.0,
            brand::INK_2,
            x
        );
    }
    let _ = write!(
        s,
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{ink2}\">stated J/S (dB)</text>\
         <text transform=\"translate(14,{}) rotate(-90)\" text-anchor=\"middle\" fill=\"{ink2}\">degradation (dB)</text>",
        ml + (w - ml - mr) / 2.0,
        h - 6.0,
        mt + (h - mt - mb) / 2.0,
        ink2 = brand::INK_2
    );
    // MODELLED reference: median over designs per bin.
    let mut bins: Vec<f64> = xs.clone();
    bins.sort_by(f64::total_cmp);
    bins.dedup();
    let model: Vec<(f64, f64)> = bins
        .iter()
        .filter_map(|x| {
            let st = crate::lab_campaign::aggregate::median(
                group
                    .iter()
                    .flat_map(|c| c.bins.iter())
                    .filter(|b| b.js_db == *x)
                    .filter_map(|b| b.modelled_degradation_db.value),
            );
            st.value.map(|v| (*x, v))
        })
        .collect();
    if model.len() > 1 {
        let pts: Vec<String> = model
            .iter()
            .map(|(x, y)| format!("{:.1},{:.1}", px(*x), py(*y)))
            .collect();
        let _ = write!(
            s,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{MODELLED_GREY}\" stroke-width=\"2\" stroke-dasharray=\"6 4\"/>",
            pts.join(" ")
        );
        let (lx, ly) = model[model.len() - 1];
        let _ = write!(
            s,
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" fill=\"{MODELLED_GREY}\">MODELLED</text>",
            px(lx) - 4.0,
            py(ly) - 6.0
        );
    }
    // MEASURED per design.
    for c in group {
        let Some(col) = design_colour(a, &c.design) else {
            continue;
        };
        let pts: Vec<(f64, f64, bool)> = c
            .bins
            .iter()
            .filter_map(|b| {
                b.measured_degradation_db
                    .value
                    .map(|y| (b.js_db, y, b.n.value.unwrap_or(0.0) >= 50.0))
            })
            .collect();
        if pts.len() > 1 {
            let line: Vec<String> = pts
                .iter()
                .map(|(x, y, _)| format!("{:.1},{:.1}", px(*x), py(*y)))
                .collect();
            let _ = write!(
                s,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{col}\" stroke-width=\"2\"/>",
                line.join(" ")
            );
        }
        for (x, y, full) in &pts {
            let _ = write!(
                s,
                "<circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"6.5\" fill=\"{paper}\"/>\
                 <circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"4.5\" fill=\"{}\" stroke=\"{col}\" stroke-width=\"2\"/>",
                if *full { col } else { brand::PAPER },
                cx = px(*x),
                cy = py(*y),
                paper = brand::PAPER
            );
        }
    }
    s.push_str("</svg><figcaption>");
    for c in group {
        let _ = write!(s, "{}{} &nbsp; ", swatch(a, &c.design), xml(&c.design));
    }
    let q = group.iter().find_map(|c| c.q);
    let formula = group.iter().find_map(|c| c.formula.clone());
    let _ = write!(
        s,
        "<span style=\"color:{MODELLED_GREY}\">- - MODELLED</span><br>MODELLED reference: {}{}. \
         Shown for comparison; it is not a validation.</figcaption></figure>",
        xml(formula.as_deref().unwrap_or("not stated")),
        q.map(|q| format!(", Q = {q}")).unwrap_or_default()
    );
    s
}

fn curve_table(a: &Aggregates, group: &[&Curve]) -> String {
    let mut s = String::from(
        "<table><tr><th>Design</th><th>Stated J/S</th><th>Locked epochs</th>\
         <th>MEASURED degradation</th><th>MODELLED degradation</th></tr>",
    );
    for c in group {
        for b in &c.bins {
            let _ = write!(
                s,
                "<tr><td>{}{}</td><td class=\"num\" data-v=\"{}\" data-op=\"cell\" data-n=\"1\">{} dB</td>{}{}{}</tr>",
                swatch(a, &c.design),
                xml(&c.design),
                raw(b.js_db),
                b.js_db,
                td_stat(&b.n, 0, "", "–"),
                td_stat(&b.measured_degradation_db, 2, "&#8239;dB", "no locked epochs"),
                td_stat(&b.modelled_degradation_db, 2, "&#8239;dB", "–"),
            );
        }
    }
    s.push_str("</table>");
    s
}

// ───────────────────────────── §6–§7 ─────────────────────────────

fn stability_section(a: &Aggregates) -> String {
    let mut s = String::from(
        "<h2>6. Tracking stability</h2><p><span class=\"tag measured\">MEASURED</span>Discriminator \
         jitter over locked epochs (median over satellites) and false-lock episodes. A false lock is the \
         lock state machine's alias detection, or, where a truth file exists, a run of locked epochs \
         whose Doppler departs from truth by more than the stated threshold.</p><table><tr><th>Chain</th>\
         <th>Design</th><th>PLL jitter</th><th>DLL jitter</th><th>False-lock episodes</th>\
         <th>per locked hour</th></tr>",
    );
    for r in &a.scorecard {
        let _ = write!(
            s,
            "<tr><td>{}</td><td>{}{}</td>{}{}{}{}</tr>",
            xml(&r.frontend),
            swatch(a, &r.design),
            xml(&r.design),
            td_stat(&r.pll_jitter_deg, 2, "&#8239;°", "no data"),
            td_stat(&r.dll_jitter_chips, 3, "&#8239;chip", "no data"),
            td_stat(&r.false_lock_episodes, 0, "", "no data"),
            td_stat(&r.false_lock_per_locked_hour, 2, "", "no locked time"),
        );
    }
    s.push_str("</table>");
    s
}

fn pass_fail_section(c: &LoadedCampaign, a: &Aggregates) -> String {
    let bars = c.manifest.scoring.get("bars").and_then(|b| b.as_object());
    let evaluated: usize = a
        .scorecard
        .iter()
        .map(|r| r.verdicts.1)
        .chain(a.events.iter().map(|r| r.verdicts.1))
        .sum();
    if evaluated == 0 {
        return "<h2>7. Pass/fail</h2><p><em>The client set no pass/fail bars for this campaign, so \
                no pass or fail is implied.</em></p>"
            .to_owned();
    }
    let mut s = String::from(
        "<h2>7. Pass/fail against the client's bars</h2><p>The verdicts are the engine's own \
         (<code>scorecard.json</code>); this report counts them. Bars as stated for the campaign: ",
    );
    s.push_str(
        &bars
            .map(|b| {
                b.iter()
                    .map(|(k, v)| format!("<code>{}</code> = {}", xml(k), xml(&v.to_string())))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "none (per-recording bars only)".to_owned()),
    );
    s.push_str(
        ". A recording's own bars, where stated in its test conditions, override these.</p>",
    );
    for rb in &a.relaxed {
        let _ = write!(
            s,
            "<div class=\"notice relaxed\">RELAXED BAR: {}.</div>",
            xml(&rb.describe())
        );
    }
    s.push_str(
        "<table><tr><th>Scope</th><th>Chain</th><th>Design</th><th>Passed</th><th>Evaluated</th>\
         <th>Failed bars</th></tr>",
    );
    let failed = |m: &std::collections::BTreeMap<String, usize>| {
        if m.is_empty() {
            "–".to_owned()
        } else {
            m.iter()
                .map(|(k, n)| format!("<code>{}</code> ×{n}", xml(k)))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let row = |s: &mut String,
               scope: &str,
               fe: &str,
               de: &str,
               (p, e): (usize, usize),
               f: String| {
        let _ = write!(
            s,
            "<tr><td>{}</td><td>{}</td><td>{}{}</td>\
             <td class=\"num\" data-v=\"{p}\" data-op=\"count\" data-n=\"{e}\">{p}</td>\
             <td class=\"num\" data-v=\"{e}\" data-op=\"count\" data-n=\"{e}\">{e}</td><td>{f}</td></tr>",
            xml(scope),
            xml(fe),
            swatch(a, de),
            xml(de),
        );
    };
    for r in a.scorecard.iter().filter(|r| r.verdicts.1 > 0) {
        row(
            &mut s,
            "whole run",
            &r.frontend,
            &r.design,
            r.verdicts,
            failed(&r.failed_bars),
        );
    }
    for r in a.events.iter().filter(|r| r.verdicts.1 > 0) {
        row(
            &mut s,
            &format!(
                "{} ({}){}",
                r.event_id,
                r.recording,
                if relaxed_for(a, &r.recording).is_empty() {
                    ""
                } else {
                    " [relaxed bar]"
                }
            ),
            &r.frontend,
            &r.design,
            r.verdicts,
            failed(&r.failed_bars),
        );
    }
    s.push_str("</table>");
    s
}

// ───────────────────────────── §8–§10 ─────────────────────────────

/// Metric definitions, verbatim from the engine's `docs/design/LAB-CAMPAIGN.md` §3
/// (`kshana.campaign-cell/1`).
pub(crate) const DEFINITIONS: &[(&str, &str)] = &[
    ("availability", "time in LOCKED / time scored (after settle_s); whole run and within each event window"),
    ("time to loss of lock", "first LOCKED → LOST transition in [onset, offset + reacq_grace] minus onset; null = held lock"),
    ("J/S at loss", "stated J/S (from [event.power]) at the moment of loss"),
    ("re-acquisition time", "first return to LOCKED at or after max(offset, t_loss) minus offset; null = not within grace"),
    ("baseline C/N0", "median NWPR C/N0 over locked epochs in [onset − baseline_window_s, onset) (else the stated nominal_cn0_dbhz)"),
    ("C/N0 degradation vs J/S", "per js_bin_db bin of stated J/S during the event: n, median measured C/N0 over locked epochs, degradation = baseline − measured"),
    ("MODELLED reference", "per bin: effective C/N0 from the engine's jamming model, (C/N0)eff = [1/(C/N0) + (J/S)/(Q·Rc)]^-1, with Q per jammer type or the event's q"),
    ("false-lock rate", "episodes per hour of locked time; an episode is the lock state machine's FALSE_LOCK alias detection or, with a truth sidecar, consecutive locked epochs with |Doppler − truth| above the threshold"),
    ("PLL / DLL jitter", "sample standard deviation of the PLL (deg) and DLL (chips) discriminator outputs over locked epochs: baseline window and per event"),
];

/// The lock sources the cells report, and what each means for the lock-based metrics.
pub(crate) fn lock_source_note(c: &LoadedCampaign) -> String {
    let mut src: Vec<&str> = c.cells.iter().map(|x| x.lock_source.as_str()).collect();
    src.sort_unstable();
    src.dedup();
    let detail = if src == ["track-session"] {
        " The lock state in these cells is the tracking session's lock state machine (LOCKED \
         against everything else), with its loss-of-lock, false-lock and re-acquisition events, as \
         set by each loop design's lock settings. Time after a channel's last update (retired, or \
         never acquired) counts as unlocked."
    } else {
        ""
    };
    format!("Lock source in these cells: {}.{detail}", src.join(", "))
}

fn method_section(c: &LoadedCampaign) -> String {
    let mut s = String::from(
        "<h2>8. Method and definitions</h2><p>Each cell replays one recording through one front-end \
         chain and one loop design in Kshana's software receiver and scores the epoch stream online \
         against the recording's stated test conditions. Definitions, as fixed by the engine for cell \
         schema <code>kshana.campaign-cell/1</code>:</p><table><tr><th>Metric</th><th>Definition</th></tr>",
    );
    let note = lock_source_note(c);
    for (k, v) in DEFINITIONS {
        let _ = write!(s, "<tr><td>{}</td><td>{}</td></tr>", xml(k), xml(v));
    }
    s.push_str(&format!("</table><p>{}</p>", xml(&note)));
    s.push_str(
        "<p>Aggregates in this report are medians, sums, counts, or ratios of sums over the \
         cells' values. Each one says so, and gives its input count in the document source \
         (<code>data-op</code>, <code>data-n</code>).</p>",
    );
    s
}

fn provenance_section(c: &LoadedCampaign) -> String {
    let mut s = String::from("<h2>9. Provenance and reproducibility</h2><p>");
    let _ = write!(
        s,
        "DIGEST <span class=\"mono\">{}</span>: <strong>verified</strong> against the {} cell file(s) \
         present. Every cell key was re-derived from its inputs. Engine {}; run hash \
         <span class=\"mono\">{}</span>; scoring hash <span class=\"mono\">{}</span>.",
        xml(&c.digest),
        c.cells.len(),
        xml(&c.manifest.engine_version),
        xml(&short(&c.manifest.run_hash)),
        xml(&short(&c.manifest.scoring_hash)),
    );
    s.push_str(
        "</p><p>Reproduce: <code>kshana iq campaign &lt;campaign.toml&gt; --out &lt;dir&gt;</code> with \
         the same engine version and inputs gives byte-identical cell files and the same DIGEST. Then \
         <code>kshana-pro lab-report &lt;dir&gt;</code> renders this report.</p>\
         <table><tr><th>Cell key</th><th>Recording</th><th>Recording SHA-256</th><th>Chain</th>\
         <th>Design</th><th>Samples</th></tr>",
    );
    for x in &c.cells {
        let _ = write!(
            s,
            "<tr><td class=\"mono\" title=\"{k}\">{}</td><td>{}</td><td class=\"mono\" title=\"{rs}\">{}</td>\
             <td>{}</td><td>{}</td><td class=\"num\" data-v=\"{n}\" data-op=\"cell\" data-n=\"1\">{n}</td></tr>",
            xml(&short(&x.key)),
            xml(&x.recording.id),
            xml(&short(&x.recording.sha256)),
            xml(&x.frontend.name),
            xml(&x.design.name),
            k = xml(&x.key),
            rs = xml(&x.recording.sha256),
            n = x.samples_processed,
        );
    }
    for k in &c.missing {
        let _ = write!(
            s,
            "<tr><td class=\"mono\">{}</td><td colspan=\"5\"><em>missing</em></td></tr>",
            xml(&short(k))
        );
    }
    s.push_str("</table>");
    s
}

fn limits_section(o: &LabReportOptions) -> String {
    let mut s = String::from(
        "<h2>10. Limits</h2><ul class=\"caveats\">\
         <li>Results describe Kshana's software receiver, with the stated loop designs and front-end \
         chains, on the supplied recordings and their stated test conditions. They make no claim about \
         the internals of the receiver that was recorded.</li>\
         <li>Kshana generated and transmitted no interference for this work. The stated jammer and \
         spoofer conditions are the lab's; Kshana only reads the recordings.</li>\
         <li>The MODELLED curve is an analytic reference. It is not a validation of the model or of the \
         loops.</li>\
         <li>No extrapolation to untested conditions is made here. Predictions with a stated hold-out \
         error come from a separate calibration step and are labelled PREDICTION.</li>",
    );
    if o.data_class == DataClass::Synthetic {
        s.push_str(
            "<li>This is a SYNTHETIC campaign. Its numbers illustrate the method only.</li>",
        );
    }
    s.push_str("</ul>");
    s
}

fn appendix(c: &LoadedCampaign) -> String {
    let mut s = String::from(
        "<h2>Appendix A. Per-satellite results</h2><table><tr><th>Recording</th><th>Chain</th>\
         <th>Design</th><th>Satellite</th><th>Availability</th><th>Losses</th><th>Re-acq.</th>\
         <th>Median C/N0</th><th>PLL jitter</th><th>DLL jitter</th></tr>",
    );
    for x in &c.cells {
        for sat in &x.satellites {
            let w = &sat.whole_run;
            let _ = write!(
                s,
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} {}</td>{}{}{}{}{}{}</tr>",
                xml(&x.recording.id),
                xml(&x.frontend.name),
                xml(&x.design.name),
                xml(&sat.signal),
                sat.id,
                td_cell_pct(w.availability),
                td_cell(Some(w.loss_count as f64), 0, "", "–"),
                td_cell(Some(w.reacq_count as f64), 0, "", "–"),
                td_cell(w.median_cn0_dbhz, 1, "&#8239;dB-Hz", "no data"),
                td_cell(w.pll_jitter_deg, 2, "&#8239;°", "no data"),
                td_cell(w.dll_jitter_chips, 3, "&#8239;chip", "no data"),
            );
        }
    }
    s.push_str(
        "</table><h2>Appendix B. Per-satellite event results</h2><table><tr><th>Recording</th>\
         <th>Chain</th><th>Design</th><th>Satellite</th><th>Event</th><th>Time to loss</th>\
         <th>J/S at loss</th><th>Re-acq. time</th><th>Availability</th></tr>",
    );
    for x in &c.cells {
        for sat in &x.satellites {
            for e in &sat.events {
                let _ = write!(
                    s,
                    "<tr><td>{}</td><td>{}</td><td>{}</td><td>{} {}</td><td>{}</td>{}{}{}{}</tr>",
                    xml(&x.recording.id),
                    xml(&x.frontend.name),
                    xml(&x.design.name),
                    xml(&sat.signal),
                    sat.id,
                    xml(&e.event_id),
                    td_cell(e.time_to_loss_s, 2, "&#8239;s", "held lock"),
                    td_cell(e.js_at_loss_db, 1, "&#8239;dB", "no J/S stated"),
                    td_cell(e.reacq_time_s, 2, "&#8239;s", "not within grace"),
                    td_cell_pct(e.availability),
                );
            }
        }
    }
    s.push_str("</table>");
    s
}
