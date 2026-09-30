// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The HTML face of the mission dossier: one self-contained page.
//!
//! It shows the dossier body and nothing else, in the same order as the PDF. Styles are
//! inline, the programme's charts are inline SVG drawn from the same plan the PDF uses
//! ([`super::pdf_chart::plan`]), and the chart the engine drew for each run is embedded
//! as a data URI. The page loads nothing from the network and runs no script, and a
//! print stylesheet lays it out for paper.

use std::collections::BTreeMap;

use super::compose::{Dossier, RunSheet, BANNER_LINES};
use super::figures::{ChartData, FigureRow};
use super::pdf::num;
use super::pdf_chart::{colour, plan};
use crate::report_brand as brand;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn verdict_class(v: &str) -> &'static str {
    match v {
        "met" => "met",
        "not met" => "fail",
        _ => "nv",
    }
}

/// An SVG document as a data URI: the characters a URI cannot hold are percent-encoded,
/// so the image travels inside the page.
fn svg_data_uri(svg: &str) -> String {
    let mut out = String::from("data:image/svg+xml;charset=utf-8,");
    for b in svg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// A programme chart as inline SVG.
fn chart_svg(c: &ChartData) -> String {
    let p = plan(c);
    let (w, h) = (720.0_f64, 300.0_f64);
    let (left, right, top, bottom) = (64.0, 16.0, 22.0, 46.0);
    let px = |u: f64| left + (w - left - right) * u;
    let py = |u: f64| h - bottom - (h - top - bottom) * u;
    let legend_h = 18.0 * p.legend.len() as f64;
    let mut s = format!(
        "<svg class=\"chart\" viewBox=\"0 0 {w} {}\" role=\"img\" aria-label=\"{}\">",
        h + legend_h,
        esc(&c.title)
    );
    s += &format!(
        "<text x=\"4\" y=\"13\" class=\"c-t\">{}</text>",
        esc(&p.y_title)
    );
    for (u, label) in &p.y_ticks {
        let y = py(*u);
        s += &format!(
            "<line x1=\"{left}\" y1=\"{y:.2}\" x2=\"{:.2}\" y2=\"{y:.2}\" class=\"c-g\"/><text x=\"{:.2}\" y=\"{:.2}\" class=\"c-t\" text-anchor=\"end\">{}</text>",
            w - right,
            left - 6.0,
            y + 4.0,
            esc(label)
        );
    }
    s += &format!(
        "<path d=\"M{left} {top} L{left} {:.2} L{:.2} {:.2}\" class=\"c-a\"/>",
        h - bottom,
        w - right,
        h - bottom
    );
    for (u, label) in &p.x_ticks {
        let x = px(*u);
        s += &format!(
            "<line x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{x:.2}\" y2=\"{:.2}\" class=\"c-a\"/><text x=\"{x:.2}\" y=\"{:.2}\" class=\"c-t\" text-anchor=\"middle\">{}</text>",
            h - bottom,
            h - bottom + 5.0,
            h - bottom + 18.0,
            esc(label)
        );
    }
    s += &format!(
        "<text x=\"{left}\" y=\"{:.2}\" class=\"c-t\">{}</text>",
        h - 8.0,
        esc(&p.x_title)
    );
    if p.samples == 0 {
        s += &format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" class=\"c-t\">no finite samples in the named columns</text>",
            left + 12.0,
            h / 2.0
        );
    }
    for b in &p.bars {
        s += &format!(
            "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>",
            px(b.x0),
            py(b.y1),
            px(b.x1) - px(b.x0),
            (py(b.y0) - py(b.y1)).max(0.6),
            colour(b.series)
        );
    }
    for line in &p.lines {
        for seg in &line.segments {
            let pts: Vec<String> = if seg.len() == 1 {
                let (x, y) = seg[0];
                vec![
                    format!("{:.2},{:.2}", px(x) - 1.5, py(y)),
                    format!("{:.2},{:.2}", px(x) + 1.5, py(y)),
                ]
            } else {
                seg.iter()
                    .map(|(x, y)| format!("{:.2},{:.2}", px(*x), py(*y)))
                    .collect()
            };
            s += &format!(
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.6\"/>",
                pts.join(" "),
                colour(line.series)
            );
        }
    }
    for (i, entry) in p.legend.iter().enumerate() {
        let y = h + 12.0 + 18.0 * i as f64;
        s += &format!(
            "<rect x=\"{left}\" y=\"{:.2}\" width=\"18\" height=\"4\" fill=\"{}\"/><text x=\"{:.2}\" y=\"{:.2}\" class=\"c-l\">{}</text>",
            y - 5.0,
            colour(i),
            left + 26.0,
            y,
            esc(entry)
        );
    }
    s += "</svg>";
    s
}

fn figure_table(figs: &[FigureRow], with_class: bool) -> String {
    let mut h = String::from(
        "<div class=\"scroll\"><table><thead><tr><th>Field</th><th>Value</th><th>Unit</th>",
    );
    if with_class {
        h += "<th>Provenance</th>";
    }
    h += "<th>What it is</th></tr></thead><tbody>";
    for f in figs {
        h += &format!(
            "<tr><td><code>{}</code></td><td class=\"n\">{}</td><td>{}</td>",
            esc(&f.path),
            esc(&num(f.value)),
            esc(f.unit.as_deref().unwrap_or("-"))
        );
        if with_class {
            h += &format!("<td>{}</td>", esc(&f.provenance));
        }
        h += &format!(
            "<td>{}{}</td></tr>",
            f.element
                .as_ref()
                .map(|e| format!("<b>{}</b> ", esc(e)))
                .unwrap_or_default(),
            esc(f.note.as_deref().unwrap_or(""))
        );
    }
    h += "</tbody></table></div>";
    h
}

fn sheet_html(s: &RunSheet, engine_chart: Option<&String>) -> String {
    let mut h = format!(
        "<section class=\"sheet\" id=\"run-{id}\"><h2>Run {id}: {title}</h2>",
        id = esc(&s.id),
        title = esc(&s.title)
    );
    if let Some(d) = &s.description {
        h += &format!("<p>{}</p>", esc(d));
    }
    h += &format!(
        "<p class=\"small\">Kind <code>{}</code> · seed {} · engine {} · scenario hash <code>{}</code><br>Result SHA-256 <code>{}</code><br>{}</p>",
        esc(&s.kind),
        s.seed.map(|x| x.to_string()).unwrap_or_else(|| "not recorded".into()),
        esc(s.engine_version.as_deref().unwrap_or("not recorded")),
        esc(s.scenario_hash.as_deref().unwrap_or("not stamped by this kind")),
        s.result.sha256,
        esc(&s.summary)
    );
    if let Some(l) = &s.label {
        h += &format!("<p class=\"label\">Run label: {}</p>", esc(l));
    }
    if !s.verdicts.is_empty() {
        h += "<h3>Requirements verified on this run</h3><p>";
        for v in &s.verdicts {
            h += &format!(
                "<span class=\"chip {}\">{} {}</span> <span class=\"small\">{} basis</span> &nbsp; ",
                verdict_class(&v.verdict),
                esc(&v.requirement),
                esc(&v.verdict),
                esc(&v.basis)
            );
        }
        h += "</p>";
    }
    h += "<h3>Headline figures: scalars the result marks as computed</h3>";
    if s.headline.is_empty() {
        h += if s.no_provenance {
            "<p class=\"small\">The result document carries no units block with provenance, so nothing in it can be called a figure.</p>"
        } else {
            "<p class=\"small\">The result marks no scalar as computed.</p>"
        };
    } else {
        h += &figure_table(&s.headline, false);
    }
    for c in &s.charts {
        h += &format!("<h3>{}</h3>{}", esc(&c.title), chart_svg(c));
        if c.stride > 1 {
            h += &format!(
                "<p class=\"small\">Every {}th of the result's {} samples is drawn.</p>",
                c.stride, c.points
            );
        }
    }
    if let Some(svg) = engine_chart {
        h += &format!(
            "<details><summary>The chart the engine drew for this run (<code>{}</code>)</summary><img class=\"engine-chart\" alt=\"The chart the open engine drew for run {}\" src=\"{}\"></details>",
            esc(&s.chart_file.path),
            esc(&s.id),
            svg_data_uri(svg)
        );
    }
    if s.figures.len() > s.headline.len() {
        h += &format!(
            "<details><summary>All {} computed scalars</summary>{}</details>",
            s.figures.len(),
            figure_table(&s.figures, false)
        );
    }
    if !s.series.is_empty() {
        h += &format!(
            "<details><summary>{} computed columns (count, minimum, maximum)</summary><div class=\"scroll\"><table><thead><tr><th>Column</th><th>Count</th><th>Minimum</th><th>Maximum</th><th>Unit</th></tr></thead><tbody>",
            s.series.len()
        );
        for c in &s.series {
            h += &format!(
                "<tr><td><code>{}</code></td><td class=\"n\">{}</td><td class=\"n\">{}</td><td class=\"n\">{}</td><td>{}</td></tr>",
                esc(&c.path),
                c.count,
                esc(&num(c.min)),
                esc(&num(c.max)),
                esc(c.unit.as_deref().unwrap_or("-"))
            );
        }
        h += "</tbody></table></div></details>";
    }
    if !s.other_outputs.is_empty() {
        h += &format!(
            "<details><summary>{} outputs of another provenance class</summary>{}</details>",
            s.other_outputs.len(),
            figure_table(&s.other_outputs, true)
        );
    }
    if !s.inputs.is_empty() {
        h += &format!(
            "<details><summary>{} inputs and constants the run echoes (not figures)</summary>{}</details>",
            s.inputs.len(),
            figure_table(&s.inputs, true)
        );
    }
    if s.unlabelled_numeric_fields > 0 {
        h += &format!(
            "<p class=\"small\">{} numeric fields of the result have no units entry and are not shown.</p>",
            s.unlabelled_numeric_fields
        );
    }
    h += "<h3>Capability labels, from the run's report.json</h3>";
    if s.capabilities.is_empty() {
        h += &format!("<p class=\"small\">{}</p>", esc(&s.integrity));
    } else {
        h += "<div class=\"scroll\"><table><thead><tr><th>Capability</th><th>Label</th><th>Role</th></tr></thead><tbody>";
        for c in &s.capabilities {
            h += &format!(
                "<tr><td>{}</td><td><span class=\"chip {}\">{}</span></td><td>{}</td></tr>",
                esc(&c.requirement),
                if c.label == "VALIDATED" { "met" } else { "mod" },
                esc(&c.label),
                esc(&c.role)
            );
        }
        h += "</tbody></table></div>";
    }
    if !s.not_modelled.is_empty() {
        h += "<h3>Not modelled, from the run's report.json</h3><ul class=\"items\">";
        for n in &s.not_modelled {
            h += &format!(
                "<li>{} <span class=\"small\">[{}]</span></li>",
                esc(&n.text),
                esc(&n.source)
            );
        }
        h += "</ul>";
    }
    h += "</section>";
    h
}

impl Dossier {
    /// The dossier as one self-contained HTML page. `engine_charts` maps a run
    /// identifier to the chart the engine drew for it.
    pub fn to_html(&self, engine_charts: &BTreeMap<String, String>) -> String {
        // Every Kshana Pro document is an Ashforde document: the shared furniture of
        // crate::report_brand goes around the renderer's own page, which is unchanged.
        let body = self.to_html_unbranded(engine_charts);
        let id = format!(
            "SHA-256 {}",
            &self.dossier_sha256[..self.dossier_sha256.len().min(16)]
        );
        crate::report_brand::finish_html(
            &body,
            &crate::report_brand::DocMark::new("Mission dossier", id),
        )
    }

    fn to_html_unbranded(&self, engine_charts: &BTreeMap<String, String>) -> String {
        let b = &self.body;
        let id = &b.identification;
        let sm = &b.summary;
        let mut h = String::new();
        h += "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">";
        h += &format!("<title>{} | mission dossier</title>", esc(&id.title));
        h += &format!(
            "<style>
:root{{--navy:{navy};--brass:{brass};--brassm:{brassm};--ivory:{ivory};--paper:#fbf9f4;--fg:{ink};--muted:#6b645a;--line:#d9d3c7;--card:#ffffff;--met:#1f7a52;--fail:#b3261e;--nv:#8a6d00;--mod:#6a3fb5;--node:#f1ece0}}
*{{box-sizing:border-box}}body{{margin:0;background:var(--paper);color:var(--fg);font:15px/1.5 Georgia,'Times New Roman',serif}}
header.cover{{background:var(--ivory);color:var(--fg);border-bottom:3px solid var(--brass);padding:40px 16px 32px}}
header.cover .in,main{{max-width:1080px;margin:0 auto}}main{{padding:8px 16px 72px}}
.brand{{font:700 14px/1.2 system-ui,sans-serif;letter-spacing:.42em;color:var(--brass)}}.brand2{{font:11px/1.4 system-ui,sans-serif;letter-spacing:.2em;text-transform:uppercase;color:var(--brassm)}}
h1{{font-size:36px;line-height:1.15;margin:26px 0 6px;color:var(--navy)}}header .sub{{font-style:italic;font-size:17px}}
h2{{font-size:22px;margin:42px 0 10px;border-bottom:1px solid var(--brassm);padding-bottom:6px;color:var(--navy)}}h3{{font-size:16px;margin:24px 0 8px}}
.banner{{background:{brass_seal};color:var(--fg);font:700 14px/1.45 system-ui,sans-serif;padding:12px 16px;border-radius:4px;margin:22px 0}}
.meta{{display:grid;grid-template-columns:max-content 1fr;gap:4px 18px;font:13px/1.5 system-ui,sans-serif}}.meta dt{{color:var(--brassm);font-weight:700}}.meta dd{{margin:0;word-break:break-word}}
.tiles{{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:10px;margin:18px 0}}.tile{{background:var(--card);border:1px solid var(--line);border-top:3px solid currentColor;border-radius:6px;padding:10px 12px;font:13px/1.4 system-ui,sans-serif}}.tile b{{display:block;font:700 30px/1.1 Georgia,serif}}.tile span{{color:var(--muted)}}
.scroll{{overflow-x:auto;border:1px solid var(--line);border-radius:6px;background:var(--card);margin:8px 0}}table{{border-collapse:collapse;width:100%;font:13px/1.45 system-ui,sans-serif}}
th,td{{text-align:left;vertical-align:top;padding:7px 9px;border-bottom:1px solid var(--line)}}th{{background:var(--node);font-weight:600}}td.n{{font-variant-numeric:tabular-nums;font-weight:600;white-space:nowrap}}
code{{font:12px/1.4 ui-monospace,Menlo,Consolas,monospace;word-break:break-word}}.small{{font:12.5px/1.5 system-ui,sans-serif;color:var(--muted)}}.label{{font:13px/1.5 system-ui,sans-serif;border-left:3px solid var(--brass);padding-left:10px}}
.chip{{display:inline-block;padding:1px 8px;border-radius:10px;font:600 12px/1.5 system-ui,sans-serif;border:1px solid currentColor;white-space:nowrap}}.met{{color:var(--met)}}.fail{{color:var(--fail)}}.nv{{color:var(--nv)}}.mod{{color:var(--mod)}}
.chart{{width:100%;height:auto;background:var(--card);border:1px solid var(--line);border-radius:6px}}.c-t{{font:11px system-ui,sans-serif;fill:var(--muted)}}.c-l{{font:12px system-ui,sans-serif;fill:var(--fg)}}.c-g{{stroke:var(--line);stroke-width:1}}.c-a{{stroke:var(--muted);stroke-width:1.2;fill:none}}
.engine-chart{{display:block;max-width:100%;height:auto;border:1px solid var(--line);border-radius:6px;background:#fff;margin-top:8px}}details{{margin:10px 0;font:13px/1.5 system-ui,sans-serif}}summary{{cursor:pointer;color:var(--navy);font-weight:600}}
ul.items{{padding-left:20px;font:13.5px/1.5 system-ui,sans-serif}}ul.items li{{margin:5px 0}}.sig td{{height:46px}}nav a{{color:var(--navy);margin-right:14px;font:13px system-ui,sans-serif}}
@media print{{body{{background:#fff;font-size:11pt}}header.cover{{-webkit-print-color-adjust:exact;print-color-adjust:exact;break-after:avoid}}.banner{{-webkit-print-color-adjust:exact;print-color-adjust:exact}}nav{{display:none}}.scroll{{overflow:visible;border:0}}section.sheet,h2.break{{break-before:page}}tr,svg,.tile{{break-inside:avoid}}thead{{display:table-header-group}}details>summary{{list-style:none}}main{{max-width:none;padding:0}}@page{{size:A4;margin:18mm}}}}
</style></head><body>",
            navy = brand::NAVY,
            brass = brand::BRASS,
            brass_seal = brand::BRASS_SEAL,
            brassm = brand::BRASS_MUTED,
            ivory = brand::IVORY,
            ink = brand::INK
        );
        h += &format!(
            "<header class=\"cover\"><div class=\"in\"><div class=\"brand\">ASHFORDE</div><div class=\"brand2\">Ashforde O&Uuml; &middot; Quantitative PNT Resilience</div><h1>{}</h1><div class=\"sub\">Mission dossier: requirements, runs and verification matrix</div></div></header><main>",
            esc(&id.title)
        );
        h += &format!(
            "<div class=\"banner\">{}</div>",
            esc(&BANNER_LINES.join(" "))
        );
        h += "<dl class=\"meta\">";
        for (k, v) in [
            (
                "Reference",
                if id.reference.is_empty() {
                    "-".to_string()
                } else {
                    esc(&id.reference)
                },
            ),
            ("Issue", esc(&id.issue)),
            ("Classification", esc(&id.classification)),
            (
                "Issued",
                esc(self.issued_utc.as_deref().unwrap_or("not stamped")),
            ),
            (
                "Engine",
                format!(
                    "open Kshana {}, {} scenario kinds",
                    esc(&id.engine_versions.join(", ")),
                    id.open_engine_kinds
                ),
            ),
            (
                "Overlay",
                format!(
                    "Kshana Pro {}, {} build",
                    esc(&id.overlay_version),
                    esc(&id.build_profile)
                ),
            ),
            (
                "Dossier SHA-256",
                format!("<code>{}</code>", self.dossier_sha256),
            ),
            (
                "Evidence pack SHA-256",
                format!("<code>{}</code>", esc(&id.evidence_pack_sha256)),
            ),
        ] {
            h += &format!("<dt>{k}</dt><dd>{v}</dd>");
        }
        h += "</dl>";
        h += "<nav><p><a href=\"#summary\">Verdict summary</a><a href=\"#matrix\">Verification matrix</a>";
        for s in &b.sheets {
            h += &format!("<a href=\"#run-{0}\">{0}</a>", esc(&s.id));
        }
        if b.trade.is_some() {
            h += "<a href=\"#trade\">Trade study</a>";
        }
        h += "<a href=\"#open\">Open items</a><a href=\"#repro\">Reproducibility</a><a href=\"#signoff\">Sign-off</a></p></nav>";

        h += "<h2 id=\"summary\">Verdict summary</h2><div class=\"tiles\">";
        for (label, n, cls) in [
            ("Requirements", sm.requirements, ""),
            ("Met", sm.met, "met"),
            ("Not met", sm.not_met, "fail"),
            ("Not verified", sm.not_verified, "nv"),
            (
                "Met on a MODELLED or PARTNER basis",
                sm.met_on_modelled_basis,
                "mod",
            ),
            ("Open items", sm.open_items, ""),
        ] {
            h += &format!("<div class=\"tile {cls}\"><b>{n}</b><span>{label}</span></div>");
        }
        h += "</div>";
        h += &format!(
            "<p>{} scenario runs{}. {} verdicts, one per requirement and linked run: {} on a VALIDATED basis, {} on a MODELLED basis, {} on a PARTNER basis. Requirements with no run traced to them: {}.</p>",
            sm.runs,
            if sm.trade_studies > 0 { " and one trade study" } else { "" },
            sm.verdicts,
            sm.verdicts_by_basis.validated,
            sm.verdicts_by_basis.modelled,
            sm.verdicts_by_basis.partner,
            sm.without_evidence
        );
        h += "<p class=\"small\">Met: the criterion holds on every linked run. Not met: it fails on at least one. Not verified: a field is missing, a unit does not match, an array is not reduced, the run's files fail an integrity check, or no run is linked. VALIDATED, MODELLED and PARTNER are the open engine's own labels for the capability behind a number; a verdict carries the weakest label it rests on.</p>";

        h += "<h2 id=\"matrix\">Verification matrix</h2><div class=\"scroll\"><table><thead><tr><th>Requirement</th><th>Method</th><th>Acceptance criterion</th><th>Run</th><th>Verdict</th><th>Basis</th><th>Value read</th></tr></thead><tbody>";
        for r in &b.matrix {
            let span = r.cells.len().max(1);
            let head = format!(
                "<td rowspan=\"{span}\"><b>{}</b><div class=\"small\">{}</div></td><td rowspan=\"{span}\">{}</td><td rowspan=\"{span}\"><code>{}</code></td>",
                esc(&r.requirement),
                esc(&r.text),
                esc(&r.method),
                esc(r.criterion.as_deref().unwrap_or("none"))
            );
            if r.cells.is_empty() {
                h += &format!("<tr>{head}<td>-</td><td><span class=\"chip nv\">not verified</span></td><td>-</td><td>no run traced</td></tr>");
            }
            for (i, c) in r.cells.iter().enumerate() {
                let values = if c.values.is_empty() {
                    esc(c.reason.as_deref().unwrap_or(""))
                } else {
                    c.values
                        .iter()
                        .map(|v| {
                            format!(
                                "<div><code>{}</code> = <b>{}</b> {}<br><span class=\"small\">{}</span></div>",
                                esc(&v.path),
                                esc(&v.value.as_f64().map(num).unwrap_or_else(|| v.value.to_string())),
                                esc(v.unit.as_deref().filter(|u| *u != "1").unwrap_or("")),
                                esc(&v.detail)
                            )
                        })
                        .collect::<String>()
                };
                h += &format!(
                    "<tr>{}<td><a href=\"#run-{run}\">{run}</a></td><td><span class=\"chip {}\">{}</span></td><td><span class=\"chip {}\">{}{}</span></td><td>{values}</td></tr>",
                    if i == 0 { head.clone() } else { String::new() },
                    verdict_class(&c.verdict),
                    esc(&c.verdict),
                    if c.flagged { "mod" } else { "met" },
                    esc(&c.basis),
                    if c.flagged { " (flagged)" } else { "" },
                    run = esc(&c.run)
                );
            }
        }
        h += "</tbody></table></div>";

        for s in &b.sheets {
            h += &sheet_html(s, engine_charts.get(&s.id));
        }

        if let Some(t) = &b.trade {
            h += &format!(
                "<h2 id=\"trade\" class=\"break\">Trade study</h2><p><b>{}</b></p><p>Metric <code>{}</code> ({} is better), read from each variant's own run.</p>",
                esc(&t.title),
                esc(&t.metric),
                if t.lower_better { "lower" } else { "higher" }
            );
            h += "<div class=\"scroll\"><table><thead><tr><th>Rank</th><th>Variant</th><th>Metric value</th><th>Scenario hash</th></tr></thead><tbody>";
            for v in &t.variants {
                h += &format!(
                    "<tr><td>{}</td><td><b>{}</b></td><td class=\"n\">{}</td><td><code>{}</code></td></tr>",
                    v.rank.map(|r| r.to_string()).unwrap_or_else(|| "-".into()),
                    esc(&v.label),
                    match (&v.metric_value, &v.error) {
                        (Some(x), _) => esc(&num(*x)),
                        (None, Some(e)) => format!("error: {}", esc(e)),
                        (None, None) => "-".into(),
                    },
                    esc(v.scenario_hash
                        .as_deref()
                        .unwrap_or("not stamped by this kind"))
                );
            }
            h += "</tbody></table></div>";
            if let Some(r) = &t.recommendation {
                h += &format!("<p>{}</p>", esc(r));
            }
            if let (Some(o), Some(s)) = (&t.gate_outcome, &t.gate_summary) {
                h += &format!(
                    "<p class=\"small\">The trade's own assurance gate: {}. {}</p>",
                    esc(o),
                    esc(s)
                );
            }
            h += "<p class=\"small\">The trade study is a Kshana Pro kind. Each variant is an open-engine scenario run unchanged; the trade itself is not reproduced by the free engine.</p>";
        }

        h += &format!(
            "<h2 id=\"open\" class=\"break\">Open items ({})</h2>",
            b.open_items.len()
        );
        if b.open_items.is_empty() {
            h += "<p>None.</p>";
        } else {
            h += "<ul class=\"items\">";
            for o in &b.open_items {
                h += &format!(
                    "<li><b>{}</b>{}{}: {}</li>",
                    esc(&o.category),
                    o.requirement
                        .as_ref()
                        .map(|r| format!(" · {}", esc(r)))
                        .unwrap_or_default(),
                    o.run
                        .as_ref()
                        .map(|r| format!(" · {}", esc(r)))
                        .unwrap_or_default(),
                    esc(&o.text)
                );
            }
            h += "</ul>";
        }

        h += &format!(
            "<h2 id=\"repro\">Reproducibility record</h2><p class=\"small\">{}</p>",
            esc(&b.reproducibility_note)
        );
        h += "<div class=\"scroll\"><table><thead><tr><th>Run</th><th>Command</th><th>Files (SHA-256)</th><th>Stamp</th></tr></thead><tbody>";
        for r in &b.reproducibility {
            h += &format!(
                "<tr><td><b>{}</b><div class=\"small\">{}</div></td><td><code>{}</code><div class=\"small\">in a copy of <code>{}</code></div></td><td>scenario <code>{}</code><br>result <code>{}</code><br>report {}</td><td>scenario_hash <code>{}</code><br>seed {}<br>engine {}<br>platform {}</td></tr><tr><td colspan=\"4\" class=\"small\">{}</td></tr>",
                esc(&r.run),
                esc(&r.kind),
                esc(&r.command),
                esc(&r.directory),
                r.scenario.sha256,
                r.result.sha256,
                r.report
                    .as_ref()
                    .map(|f| format!("<code>{}</code>", f.sha256))
                    .unwrap_or_else(|| "none".into()),
                esc(r.scenario_hash.as_deref().unwrap_or("not stamped")),
                r.seed.map(|s| s.to_string()).unwrap_or_else(|| "not recorded".into()),
                esc(r.engine_version.as_deref().unwrap_or("not recorded")),
                esc(r.platform.as_deref().unwrap_or("not recorded")),
                esc(&r.statement)
            );
        }
        h += "</tbody></table></div>";

        h += "<h2>Inputs</h2><div class=\"scroll\"><table><thead><tr><th>File</th><th>SHA-256</th></tr></thead><tbody>";
        for f in &id.inputs {
            h += &format!(
                "<tr><td>{}</td><td><code>{}</code></td></tr>",
                esc(&f.path),
                f.sha256
            );
        }
        h += "</tbody></table></div>";

        h += "<h2 id=\"signoff\">Sign-off</h2><div class=\"scroll\"><table class=\"sig\"><thead><tr><th>Role</th><th>Name</th><th>Signature</th><th>Date</th></tr></thead><tbody>";
        for s in &b.sign_off {
            h += &format!(
                "<tr><td>{}</td><td>{}</td><td></td><td></td></tr>",
                esc(&s.role),
                esc(&s.name)
            );
        }
        h += "</tbody></table></div>";

        h += "<h2>Limitations</h2><ul class=\"items\">";
        for l in &b.limitations {
            h += &format!("<li>{}</li>", esc(l));
        }
        h += "</ul><p class=\"small\">Abbreviations: ";
        h += &b
            .terms
            .iter()
            .map(|t| format!("<b>{}</b> {}", esc(&t.term), esc(&t.meaning)))
            .collect::<Vec<_>>()
            .join("; ");
        h += ".</p>";
        h += &format!(
            "<p class=\"small\">{} · MODELLED engineering study, not a certification.</p>",
            esc(&b.generated_by)
        );
        h += "</main></body></html>\n";
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_data_uri_holds_no_raw_markup() {
        let u = svg_data_uri("<svg xmlns=\"http://www.w3.org/2000/svg\"><text>a&b</text></svg>");
        assert!(u.starts_with("data:image/svg+xml;charset=utf-8,%3Csvg"));
        assert!(!u.contains('<') && !u.contains('"') && !u.contains("http://"));
    }
}
