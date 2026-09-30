// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Charts and the report page of a study, drawn from the result document alone.
//!
//! Every mark is read from `uq.result.json`'s own fields, so a chart can never show a
//! number the document does not hold, and the files are byte-identical whenever the
//! document is. Three charts (scalable vector graphics, no script, no external
//! resource) and one self-contained page:
//!
//! * [`bands_svg`]: per time step, the 5th to 95th percentile band and the median of
//!   each series output, with the base run and the limit;
//! * [`sensitivity_svg`]: ranked bars of the sensitivity of each scalar output to each
//!   input, with their confidence intervals;
//! * [`convergence_svg`]: each statistic settling as runs accumulate;
//! * [`report_html`]: the headline probability, the tables behind every chart, the
//!   limits of the method and the reproducibility stamp.
//!
//! Colours: two categorical series colours checked for separation under the common
//! colour-vision deficiencies on the dark surface; text is always a text colour, never
//! a series colour; the limit uses a reserved warning colour and always carries a
//! label. Every mark has a `<title>`, which a browser shows on hover.

use serde_json::Value;

const SURFACE: &str = "#0c0b08";
const TEXT: &str = "#e8e2d6";
const MUTED: &str = "#8c8273";
const GRID: &str = "#342c21";
/// Categorical series colours, in fixed order.
const SERIES: [&str; 4] = ["#3987e5", "#d95926", "#199e70", "#c98500"];
/// Reserved for a limit; never a series.
const LIMIT: &str = "#fab219";
const FONT: &str = "system-ui,-apple-system,'Segoe UI',sans-serif";

const W: f64 = 920.0;
const ML: f64 = 70.0;
const MR: f64 = 200.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A number for a label: four significant figures, scientific outside 1e-3..1e5.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    if v == 0.0 {
        return "0".to_string();
    }
    let a = v.abs();
    if !(1e-3..1e5).contains(&a) {
        format!("{v:.3e}")
    } else if a >= 1000.0 {
        format!("{v:.1}")
    } else if a >= 100.0 {
        format!("{v:.2}")
    } else if a >= 1.0 {
        format!("{v:.3}")
    } else {
        format!("{v:.4}")
    }
}

/// A tick label: [`fmt_num`] without trailing zeros (`20` rather than `20.000`).
fn fmt_tick(v: f64) -> String {
    let s = fmt_num(v);
    if s.contains('e') || !s.contains('.') {
        return s;
    }
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

/// A label cut to `max` characters, with an ellipsis when it was longer.
fn clip(label: &str, max: usize) -> String {
    if label.chars().count() <= max {
        label.to_string()
    } else {
        format!("{}…", label.chars().take(max - 1).collect::<String>())
    }
}

fn unit_suffix(unit: &str) -> String {
    if unit.is_empty() || unit == "1" {
        String::new()
    } else {
        format!(" {unit}")
    }
}

/// About five round tick values covering `[lo, hi]`.
fn ticks(lo: f64, hi: f64) -> Vec<f64> {
    let span = hi - lo;
    if !(span.is_finite() && span > 0.0) {
        return vec![lo];
    }
    let raw = span / 5.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * mag);
    let mut t = (lo / step).ceil() * step;
    let mut out = Vec::new();
    while t <= hi + step * 1e-9 {
        out.push(if t.abs() < step * 1e-9 { 0.0 } else { t });
        t += step;
    }
    out
}

/// A linear map from data to pixels.
#[derive(Clone, Copy)]
struct Scale {
    d0: f64,
    d1: f64,
    p0: f64,
    p1: f64,
}

impl Scale {
    fn new(d0: f64, d1: f64, p0: f64, p1: f64) -> Scale {
        let (d0, d1) = if d1 > d0 {
            (d0, d1)
        } else {
            (d0 - 0.5, d0 + 0.5)
        };
        Scale { d0, d1, p0, p1 }
    }
    fn at(&self, v: f64) -> f64 {
        self.p0 + (v - self.d0) / (self.d1 - self.d0) * (self.p1 - self.p0)
    }
}

fn pad_range(lo: f64, hi: f64) -> (f64, f64) {
    let span = hi - lo;
    let pad = if span > 0.0 {
        0.06 * span
    } else {
        0.5 * lo.abs().max(1.0)
    };
    (lo - pad, hi + pad)
}

fn floats(v: &Value) -> Vec<Option<f64>> {
    v.as_array()
        .map(|a| a.iter().map(Value::as_f64).collect())
        .unwrap_or_default()
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn svg_open(h: f64, title: &str, desc: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{W}\" height=\"{h:.0}\" viewBox=\"0 0 {W} {h:.0}\" \
         font-family=\"{FONT}\" font-size=\"12\" role=\"img\" aria-label=\"{}\">\
         <title>{}</title><desc>{}</desc>\
         <rect width=\"100%\" height=\"100%\" fill=\"{SURFACE}\"/>",
        esc(title),
        esc(title),
        esc(desc)
    )
}

fn header(doc: &Value, what: &str) -> String {
    let runs = doc["runs"]["total"].as_u64().unwrap_or(0);
    let failed = doc["runs"]["failed"].as_u64().unwrap_or(0);
    let mut s = format!(
        "<text x=\"{ML}\" y=\"26\" fill=\"{TEXT}\" font-size=\"15\" font-weight=\"600\">{}</text>\
         <text x=\"{ML}\" y=\"44\" fill=\"{MUTED}\">{} · {} runs of `{}`{} · seed {} · MODELLED</text>",
        esc(what),
        esc(text(&doc["title"])),
        runs,
        esc(text(&doc["base"]["kind"])),
        if failed > 0 {
            format!(" ({failed} failed, recorded)")
        } else {
            String::new()
        },
        doc["seed"].as_u64().unwrap_or(0),
    );
    let assumed: Vec<&str> = doc["inputs"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|i| i["source_class"] == "ASSUMED")
                .map(|i| text(&i["name"]))
                .collect()
        })
        .unwrap_or_default();
    if !assumed.is_empty() {
        s += &format!(
            "<text x=\"{ML}\" y=\"60\" fill=\"{MUTED}\">ASSUMED input distributions: {}</text>",
            esc(&assumed.join(", "))
        );
    }
    s
}

fn footer(doc: &Value, y: f64) -> String {
    format!(
        "<text x=\"{ML}\" y=\"{y:.0}\" fill=\"{MUTED}\" font-size=\"10\">Kshana Pro {} on open engine {} · study hash {} · every figure is conditional on the stated input distributions</text>",
        esc(text(&doc["overlay_version"])),
        esc(text(&doc["engine_version"])),
        esc(&text(&doc["scenario_hash"]).chars().take(16).collect::<String>()),
    )
}

fn y_axis(scale: Scale, x0: f64, x1: f64, unit: &str) -> String {
    let mut s = String::new();
    for t in ticks(scale.d0, scale.d1) {
        let y = scale.at(t);
        s += &format!(
            "<line x1=\"{x0}\" y1=\"{y:.1}\" x2=\"{x1}\" y2=\"{y:.1}\" stroke=\"{GRID}\"/>\
             <text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"{MUTED}\">{}</text>",
            x0 - 8.0,
            y + 4.0,
            fmt_tick(t)
        );
    }
    if !unit_suffix(unit).is_empty() {
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"{MUTED}\" font-size=\"11\">{}</text>",
            x0 - 8.0,
            scale.p1 - 10.0,
            esc(unit)
        );
    }
    s
}

fn polyline(xs: &[f64], ys: &[Option<f64>], sx: Scale, sy: Scale) -> Vec<String> {
    // One path per unbroken run of values.
    let mut paths = Vec::new();
    let mut cur = String::new();
    for (x, y) in xs.iter().zip(ys) {
        match y {
            Some(y) => {
                cur += &format!(
                    "{}{:.2},{:.2}",
                    if cur.is_empty() { "M" } else { " L" },
                    sx.at(*x),
                    sy.at(*y)
                );
            }
            None => {
                if !cur.is_empty() {
                    paths.push(std::mem::take(&mut cur));
                }
            }
        }
    }
    if !cur.is_empty() {
        paths.push(cur);
    }
    paths
}

/// An index for a label: three decimals, never a signed zero.
fn fmt_index(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    let s = format!("{v:.3}");
    if s == "-0.000" {
        "0.000".to_string()
    } else {
        s
    }
}

fn legend(items: &[(&str, &str, &str)], x: f64, y: f64) -> String {
    // (colour, kind, label); kind is "line", "area", "dash" or "bar".
    let mut s = String::new();
    for (i, (colour, kind, label)) in items.iter().enumerate() {
        let yy = y + i as f64 * 18.0;
        s += &match *kind {
            "area" => format!(
                "<rect x=\"{x}\" y=\"{:.1}\" width=\"18\" height=\"10\" fill=\"{colour}\" fill-opacity=\"0.28\"/>",
                yy - 9.0
            ),
            "bar" => format!(
                "<rect x=\"{x}\" y=\"{:.1}\" width=\"18\" height=\"8\" rx=\"2\" fill=\"{colour}\"/>",
                yy - 8.0
            ),
            "dash" => format!(
                "<line x1=\"{x}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{colour}\" stroke-width=\"2\" stroke-dasharray=\"5 4\"/>",
                yy - 4.0,
                x + 18.0,
                yy - 4.0
            ),
            _ => format!(
                "<line x1=\"{x}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{colour}\" stroke-width=\"2\"/>",
                yy - 4.0,
                x + 18.0,
                yy - 4.0
            ),
        };
        s += &format!(
            "<text x=\"{:.1}\" y=\"{yy:.1}\" fill=\"{TEXT}\" font-size=\"11\">{}</text>",
            x + 24.0,
            esc(label)
        );
    }
    s
}

fn limit_lines(op: &str, value: f64) -> Vec<f64> {
    if op.starts_with("abs_") {
        vec![value, -value]
    } else {
        vec![value]
    }
}

fn limit_phrase(op: &str, value: f64, unit: &str) -> String {
    let u = unit_suffix(unit);
    match op {
        "lt" => format!("limit: below {}{u}", fmt_tick(value)),
        "le" => format!("limit: at most {}{u}", fmt_tick(value)),
        "gt" => format!("limit: above {}{u}", fmt_tick(value)),
        "ge" => format!("limit: at least {}{u}", fmt_tick(value)),
        _ => format!("limit: within ±{}{u}", fmt_tick(value)),
    }
}

fn output_names<'a>(doc: &'a Value, section: &str) -> Vec<&'a str> {
    doc["output_order"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter(|n| doc[section].get(*n).is_some())
                .collect()
        })
        .unwrap_or_default()
}

/// The outputs that have a sensitivity entry, in declared order.
fn sensitivity_names(doc: &Value) -> Vec<&str> {
    doc["output_order"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter(|n| doc["sensitivity"]["outputs"].get(*n).is_some())
                .collect()
        })
        .unwrap_or_default()
}

const TOP: f64 = 78.0;

/// The percentile bands of the series outputs. A study with no series output gets the
/// 5th to 95th percentile interval of each scalar output instead.
pub fn bands_svg(doc: &Value) -> String {
    let names = output_names(doc, "bands");
    if names.is_empty() {
        return intervals_svg(doc);
    }
    let panel_h = 300.0;
    let h = TOP + names.len() as f64 * (panel_h + 56.0) + 24.0;
    let mut s = svg_open(
        h,
        "Percentile band over the ensemble",
        "Per time step: the 5th to 95th percentile band and the median of each series output over the realisations, with the base run and the limit.",
    );
    s += &header(doc, "5th to 95th percentile band and median, per time step");
    for (pi, name) in names.iter().enumerate() {
        let b = &doc["bands"][name];
        let unit = text(&b["unit"]);
        let y0 = TOP + pi as f64 * (panel_h + 56.0) + 22.0;
        let t: Vec<f64> = floats(&b["t"]).into_iter().flatten().collect();
        let (p05, p50, p95, nominal) = (
            floats(&b["p05"]),
            floats(&b["p50"]),
            floats(&b["p95"]),
            floats(&b["nominal"]),
        );
        let limit = b["limit"]["value"].as_f64();
        let op = text(&b["limit"]["op"]);
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for v in p05.iter().chain(&p95).chain(&nominal).flatten() {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
        if let Some(l) = limit {
            for v in limit_lines(op, l) {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if !lo.is_finite() {
            (lo, hi) = (0.0, 1.0);
        }
        let (lo, hi) = pad_range(lo, hi);
        let sx = Scale::new(
            t.first().copied().unwrap_or(0.0),
            t.last().copied().unwrap_or(1.0),
            ML,
            W - MR,
        );
        let sy = Scale::new(lo, hi, y0 + panel_h, y0);
        s += &format!(
            "<text x=\"{ML}\" y=\"{:.1}\" fill=\"{TEXT}\" font-weight=\"600\">{} <tspan fill=\"{MUTED}\" font-weight=\"400\">({}, {} realisations)</tspan></text>",
            y0 - 24.0,
            esc(name),
            esc(text(&b["series_path"])),
            b["n"].as_u64().unwrap_or(0)
        );
        s += &y_axis(sy, ML, W - MR, unit);
        for tick in ticks(sx.d0, sx.d1) {
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{MUTED}\">{}</text>",
                sx.at(tick),
                y0 + panel_h + 16.0,
                fmt_tick(tick)
            );
        }
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"{MUTED}\" font-size=\"11\">time ({})</text>",
            W - MR,
            y0 + panel_h + 30.0,
            esc(text(&b["time_unit"]))
        );
        // The band: forward along the 95th percentile, back along the 5th.
        let mut area = String::new();
        let pts: Vec<(f64, f64, f64)> = t
            .iter()
            .zip(p05.iter().zip(&p95))
            .filter_map(|(x, (a, c))| Some((*x, (*a)?, (*c)?)))
            .collect();
        for (i, (x, _, c)) in pts.iter().enumerate() {
            area += &format!(
                "{}{:.2},{:.2}",
                if i == 0 { "M" } else { " L" },
                sx.at(*x),
                sy.at(*c)
            );
        }
        for (x, a, _) in pts.iter().rev() {
            area += &format!(" L{:.2},{:.2}", sx.at(*x), sy.at(*a));
        }
        if !pts.is_empty() {
            s += &format!(
                "<path d=\"{area} Z\" fill=\"{}\" fill-opacity=\"0.28\"><title>5th to 95th percentile of {} over {} realisations</title></path>",
                SERIES[0],
                esc(name),
                b["n"].as_u64().unwrap_or(0)
            );
        }
        for d in polyline(&t, &nominal, sx, sy) {
            s += &format!(
                "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"><title>base run: the scenario as written, one realisation</title></path>",
                SERIES[1]
            );
        }
        for d in polyline(&t, &p50, sx, sy) {
            s += &format!(
                "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"><title>median of {} over the realisations</title></path>",
                SERIES[0],
                esc(name)
            );
        }
        if let Some(l) = limit {
            for (i, v) in limit_lines(op, l).into_iter().enumerate() {
                let y = sy.at(v);
                s += &format!(
                    "<line x1=\"{ML}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{y:.1}\" stroke=\"{LIMIT}\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\"><title>{}</title></line>",
                    W - MR,
                    esc(&limit_phrase(op, l, unit))
                );
                if i == 0 {
                    s += &format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{TEXT}\" font-size=\"11\">{}</text>",
                        W - MR + 8.0,
                        y + 4.0,
                        esc(&limit_phrase(op, l, unit))
                    );
                }
            }
        }
        // Direct labels at the right edge for the last band values.
        if let Some((_, a, c)) = pts.last() {
            let last_med = p50.iter().rev().flatten().next().copied();
            for (v, lab) in [
                (Some(*c), "95th percentile"),
                (last_med, "median"),
                (Some(*a), "5th percentile"),
            ] {
                if let Some(v) = v {
                    s += &format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{MUTED}\" font-size=\"10\">{lab} {}</text>",
                        W - MR + 8.0,
                        sy.at(v) + 3.0 + if lab == "median" { 0.0 } else if lab.starts_with("95") { -8.0 } else { 10.0 },
                        fmt_num(v)
                    );
                }
            }
        }
        s += &legend(
            &[
                (SERIES[0], "area", "5th to 95th percentile"),
                (SERIES[0], "line", "median"),
                (SERIES[1], "line", "base run (one realisation)"),
            ],
            ML + 12.0,
            y0 + 18.0,
        );
    }
    s += &footer(doc, h - 10.0);
    s += "</svg>";
    s
}

/// The 5th to 95th percentile interval of each scalar output (used when a study has no
/// series output, so there is no band over time to draw).
fn intervals_svg(doc: &Value) -> String {
    let names = output_names(doc, "outputs");
    let row_h = 74.0;
    let h = TOP + names.len() as f64 * row_h + 50.0;
    let mut s = svg_open(
        h,
        "Percentile interval of each output",
        "This study declares no series output, so there is no band over time. Each row shows one scalar output's 5th to 95th percentile interval, its median, the base run and the limit, on the output's own axis.",
    );
    s += &header(
        doc,
        "5th to 95th percentile interval of each output (no series output declared)",
    );
    for (i, name) in names.iter().enumerate() {
        let o = &doc["outputs"][name];
        let unit = text(&o["unit"]);
        let y = TOP + i as f64 * row_h + 30.0;
        let (Some(p05), Some(p50), Some(p95), Some(min), Some(max)) = (
            o["p05"].as_f64(),
            o["p50"].as_f64(),
            o["p95"].as_f64(),
            o["min"].as_f64(),
            o["max"].as_f64(),
        ) else {
            s += &format!(
                "<text x=\"{ML}\" y=\"{y:.1}\" fill=\"{TEXT}\">{}: no value</text>",
                esc(name)
            );
            continue;
        };
        let limit = o["limit"]["value"].as_f64();
        let op = text(&o["limit"]["op"]);
        let (mut lo, mut hi) = (min, max);
        if let Some(n) = o["nominal"].as_f64() {
            lo = lo.min(n);
            hi = hi.max(n);
        }
        if let Some(l) = limit {
            for v in limit_lines(op, l) {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        let (lo, hi) = pad_range(lo, hi);
        let sx = Scale::new(lo, hi, ML, W - MR);
        s += &format!(
            "<text x=\"{ML}\" y=\"{:.1}\" fill=\"{TEXT}\" font-weight=\"600\">{} <tspan fill=\"{MUTED}\" font-weight=\"400\">({}{}) median {}, 5th to 95th percentile {} to {}</tspan></text>",
            y - 14.0,
            esc(name),
            esc(text(&o["path"])),
            esc(&unit_suffix(unit)),
            fmt_num(p50),
            fmt_num(p05),
            fmt_num(p95)
        );
        s += &format!(
            "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{MUTED}\"><title>smallest to largest value: {} to {}</title></line>",
            sx.at(min), y + 8.0, sx.at(max), y + 8.0, fmt_num(min), fmt_num(max)
        );
        s += &format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"12\" rx=\"4\" fill=\"{}\" fill-opacity=\"0.55\"><title>5th to 95th percentile: {} to {}{}</title></rect>",
            sx.at(p05), y + 2.0, (sx.at(p95) - sx.at(p05)).max(1.0), SERIES[0],
            fmt_num(p05), fmt_num(p95), esc(&unit_suffix(unit))
        );
        s += &format!(
            "<line x1=\"{0:.1}\" y1=\"{1:.1}\" x2=\"{0:.1}\" y2=\"{2:.1}\" stroke=\"{TEXT}\" stroke-width=\"2\"><title>median {3}</title></line>",
            sx.at(p50), y - 2.0, y + 18.0, fmt_num(p50)
        );
        if let Some(n) = o["nominal"].as_f64() {
            s += &format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{}\" stroke=\"{SURFACE}\" stroke-width=\"2\"><title>base run {}</title></circle>",
                sx.at(n), y + 8.0, SERIES[1], fmt_num(n)
            );
        }
        if let Some(l) = limit {
            for v in limit_lines(op, l) {
                s += &format!(
                    "<line x1=\"{0:.1}\" y1=\"{1:.1}\" x2=\"{0:.1}\" y2=\"{2:.1}\" stroke=\"{LIMIT}\" stroke-width=\"1.5\" stroke-dasharray=\"4 3\"><title>{3}</title></line>",
                    sx.at(v), y - 6.0, y + 22.0, esc(&limit_phrase(op, l, unit))
                );
            }
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{TEXT}\" font-size=\"11\">{}</text>",
                W - MR + 8.0,
                y + 12.0,
                esc(&limit_phrase(op, l, unit))
            );
        }
        for tick in ticks(lo, hi) {
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{MUTED}\" font-size=\"10\">{}</text>",
                sx.at(tick), y + 36.0, fmt_tick(tick)
            );
        }
    }
    s += &legend(
        &[
            (SERIES[0], "bar", "5th to 95th percentile"),
            (SERIES[1], "bar", "base run"),
        ],
        W - MR + 8.0,
        TOP + 4.0,
    );
    s += &footer(doc, h - 10.0);
    s += "</svg>";
    s
}

fn input_label(doc: &Value, name: &str) -> String {
    let assumed = doc["inputs"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["name"] == name))
        .is_some_and(|i| i["source_class"] == "ASSUMED");
    if assumed {
        format!("{name} (ASSUMED)")
    } else {
        name.to_string()
    }
}

/// The inputs of one output's sensitivity entry, ranked first, unranked last.
fn ranked(entry: &Value) -> Vec<&Value> {
    let mut v: Vec<&Value> = entry["inputs"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    v.sort_by_key(|e| e["rank"].as_u64().unwrap_or(u64::MAX));
    v
}

/// One bar of the sensitivity chart: (value key, colour, interval low key, interval
/// high key, what the value is).
type BarSpec = (
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    &'static str,
);

/// Ranked bars of each scalar output's sensitivity to each input.
pub fn sensitivity_svg(doc: &Value) -> String {
    let method = text(&doc["sensitivity"]["method"]);
    let names = sensitivity_names(doc);
    if method == "none" || names.is_empty() {
        let h = 150.0;
        let mut s = svg_open(
            h,
            "Sensitivity",
            "No sensitivity analysis was requested in this study.",
        );
        s += &header(doc, "Sensitivity of the outputs to the inputs");
        s += &format!(
            "<text x=\"{ML}\" y=\"100\" fill=\"{TEXT}\">No sensitivity analysis was requested (sensitivity method \"none\").</text>\
             <text x=\"{ML}\" y=\"120\" fill=\"{MUTED}\">Set [sensitivity] method = \"src\" (no extra runs) or \"sobol\" (n × (k + 2) runs) in the study file.</text>"
        );
        s += "</svg>";
        return s;
    }
    let sobol = method == "sobol";
    let row_h = if sobol { 34.0 } else { 26.0 };
    let label_w = 210.0;
    let x0 = ML + label_w;
    let x1 = W - MR;
    let panel_heights: Vec<f64> = names
        .iter()
        .map(|n| {
            let rows = doc["sensitivity"]["outputs"][n]["inputs"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0);
            74.0 + rows as f64 * row_h + 30.0
        })
        .collect();
    let h = TOP + panel_heights.iter().sum::<f64>() + 30.0 + if sobol { 64.0 } else { 0.0 };
    let mut s = svg_open(
        h,
        "Sensitivity of each output to each input",
        if sobol {
            "Per output: the first-order and total Sobol' sensitivity index of each input with 95% bootstrap confidence intervals, ranked by total index."
        } else {
            "Per output: the standardised regression coefficient of each input, ranked by magnitude, with the R-squared of the linear fit."
        },
    );
    s += &header(
        doc,
        if sobol {
            "What drives each output: first-order and total sensitivity index per input"
        } else {
            "What drives each output: standardised regression coefficient per input"
        },
    );
    let legend_h = if sobol { 64.0 } else { 0.0 };
    let mut y = TOP + 8.0 + legend_h;
    for (pi, name) in names.iter().enumerate() {
        let e = &doc["sensitivity"]["outputs"][name];
        let rows = ranked(e);
        // Axis range.
        let (mut lo, mut hi) = if sobol {
            (0.0f64, 1.0f64)
        } else {
            (-1.0f64, 1.0f64)
        };
        for r in &rows {
            for key in [
                "first_order_ci95_low",
                "first_order_ci95_high",
                "total_ci95_low",
                "total_ci95_high",
                "first_order",
                "total",
                "src",
            ] {
                if let Some(v) = r[key].as_f64() {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
        let sx = Scale::new(lo, hi, x0, x1);
        s += &format!(
            "<text x=\"{ML}\" y=\"{:.1}\" fill=\"{TEXT}\" font-weight=\"600\">{} <tspan fill=\"{MUTED}\" font-weight=\"400\">({})</tspan></text>",
            y + 14.0,
            esc(name),
            esc(text(&doc["outputs"][name]["path"]))
        );
        let sub = if sobol {
            match e["interaction_share"].as_f64() {
                Some(v) => format!(
                    "{} complete design rows · share not explained by any single input alone: {}",
                    e["n_rows"].as_u64().unwrap_or(0),
                    fmt_num(v)
                ),
                None => text(&e["reason"]).to_string(),
            }
        } else {
            // The chart has room for one line; the full statement is in the result
            // document and the report.
            match e["r_squared"].as_f64() {
                Some(r2) if e["reliable"] == true => format!(
                    "R-squared {}: the linear fit explains {:.1} % of the output variance",
                    fmt_num(r2),
                    100.0 * r2
                ),
                Some(r2) => format!(
                    "R-squared {}: the response is not linear in the inputs, so this ranking is NOT reliable (use method \"sobol\")",
                    fmt_num(r2)
                ),
                None => text(&e["reason"]).to_string(),
            }
        };
        let unreliable = !sobol && e["reliable"] == false && e["r_squared"].is_number();
        s += &format!(
            "<text x=\"{ML}\" y=\"{:.1}\" fill=\"{}\" font-size=\"11\">{}{}</text>",
            y + 32.0,
            if unreliable { TEXT } else { MUTED },
            if unreliable { "⚠ " } else { "" },
            esc(&sub)
        );
        let top = y + 52.0;
        let bottom = top + rows.len() as f64 * row_h;
        for t in ticks(lo, hi) {
            let x = sx.at(t);
            s += &format!(
                "<line x1=\"{x:.1}\" y1=\"{top:.1}\" x2=\"{x:.1}\" y2=\"{bottom:.1}\" stroke=\"{GRID}\"/>\
                 <text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{MUTED}\" font-size=\"10\">{}</text>",
                bottom + 14.0,
                fmt_tick(t)
            );
        }
        let zero = sx.at(0.0);
        s += &format!(
            "<line x1=\"{zero:.1}\" y1=\"{top:.1}\" x2=\"{zero:.1}\" y2=\"{bottom:.1}\" stroke=\"{MUTED}\"/>"
        );
        for (ri, r) in rows.iter().enumerate() {
            let ry = top + ri as f64 * row_h;
            let inp = text(&r["input"]);
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"{TEXT}\">{}</text>",
                x0 - 10.0,
                ry + row_h / 2.0 + 4.0,
                esc(&input_label(doc, inp))
            );
            if let Some(why) = r["reason"].as_str() {
                s += &format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{MUTED}\" font-size=\"11\">no value: {}</text>",
                    x0 + 6.0,
                    ry + row_h / 2.0 + 4.0,
                    esc(why)
                );
                continue;
            }
            let bars: Vec<BarSpec> = if sobol {
                vec![
                    (
                        "first_order",
                        SERIES[0],
                        Some("first_order_ci95_low"),
                        Some("first_order_ci95_high"),
                        "first-order index",
                    ),
                    (
                        "total",
                        SERIES[1],
                        Some("total_ci95_low"),
                        Some("total_ci95_high"),
                        "total index",
                    ),
                ]
            } else {
                vec![(
                    "src",
                    SERIES[0],
                    None,
                    None,
                    "standardised regression coefficient",
                )]
            };
            let bar_h = if sobol { 10.0 } else { 12.0 };
            for (bi, (key, colour, lo_key, hi_key, what)) in bars.iter().enumerate() {
                let Some(v) = r[*key].as_f64() else { continue };
                let by = ry + 5.0 + bi as f64 * (bar_h + 2.0);
                let (xa, xb) = (zero.min(sx.at(v)), zero.max(sx.at(v)));
                let ci = match (lo_key, hi_key) {
                    (Some(l), Some(u)) => r[*l].as_f64().zip(r[*u].as_f64()),
                    _ => None,
                };
                let tip = match ci {
                    Some((l, u)) => format!(
                        "{inp}: {what} {} (95% confidence interval {} to {})",
                        fmt_num(v),
                        fmt_num(l),
                        fmt_num(u)
                    ),
                    None => format!(
                        "{inp}: {what} {}; slope {} {}",
                        fmt_num(v),
                        fmt_num(r["coefficient"].as_f64().unwrap_or(f64::NAN)),
                        text(
                            &doc["units"]
                                [format!("sensitivity.outputs.{name}.inputs[].coefficient")]
                                ["unit"]
                        )
                    ),
                };
                s += &format!(
                    "<rect x=\"{xa:.1}\" y=\"{by:.1}\" width=\"{:.1}\" height=\"{bar_h}\" rx=\"2\" fill=\"{colour}\"><title>{}</title></rect>",
                    (xb - xa).max(1.0),
                    esc(&tip)
                );
                if let Some((l, u)) = ci {
                    let cy = by + bar_h / 2.0;
                    s += &format!(
                        "<path d=\"M{0:.1},{1:.1} L{2:.1},{1:.1} M{0:.1},{3:.1} L{0:.1},{4:.1} M{2:.1},{3:.1} L{2:.1},{4:.1}\" stroke=\"{TEXT}\" stroke-width=\"1.2\" fill=\"none\"><title>{5}</title></path>",
                        sx.at(l), cy, sx.at(u), cy - 3.0, cy + 3.0, esc(&tip)
                    );
                }
            }
            let label = if sobol {
                format!(
                    "{} · {}",
                    fmt_index(r["first_order"].as_f64().unwrap_or(f64::NAN)),
                    fmt_index(r["total"].as_f64().unwrap_or(f64::NAN))
                )
            } else {
                format!("{:+.3}", r["src"].as_f64().unwrap_or(f64::NAN))
            };
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{TEXT}\" font-size=\"11\">{}</text>",
                x1 + 8.0,
                ry + row_h / 2.0 + 4.0,
                esc(&label)
            );
        }
        if pi == 0 {
            if sobol {
                s += &legend(
                    &[
                        (SERIES[0], "bar", "first-order index"),
                        (SERIES[1], "bar", "total index"),
                        (TEXT, "line", "95% interval"),
                    ],
                    ML,
                    TOP + 6.0,
                );
            } else {
                s += &format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{MUTED}\" font-size=\"10\">coefficient</text>",
                    x1 + 8.0,
                    top - 6.0
                );
            }
        }
        if sobol {
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{MUTED}\" font-size=\"10\">first · total</text>",
                x1 + 8.0,
                top - 6.0
            );
        }
        y += panel_heights[pi];
    }
    s += &footer(doc, h - 10.0);
    s += "</svg>";
    s
}

/// Each statistic settling as runs accumulate.
pub fn convergence_svg(doc: &Value) -> String {
    let names = output_names(doc, "outputs");
    let panel_h = 170.0;
    let gap = 66.0;
    // Panels: per scalar output the statistic; with a limit also the probability; with
    // Sobol' indices also the total indices.
    struct Panel<'a> {
        name: &'a str,
        kind: &'static str,
    }
    let mut panels = Vec::new();
    for n in &names {
        let o = &doc["outputs"][n];
        if o["convergence"].as_array().is_some_and(|a| !a.is_empty()) {
            panels.push(Panel {
                name: n,
                kind: "stat",
            });
            if o["limit"].is_object() {
                panels.push(Panel {
                    name: n,
                    kind: "prob",
                });
            }
        }
        if doc["sensitivity"]["outputs"][n]["convergence"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
        {
            panels.push(Panel {
                name: n,
                kind: "sobol",
            });
        }
    }
    let h = TOP + panels.len().max(1) as f64 * (panel_h + gap) + 24.0;
    let mut s = svg_open(
        h,
        "Convergence as runs accumulate",
        "Per output: the running mean and 5th to 95th percentile over the first n realisations, the running probability of meeting the limit, and the running total sensitivity indices.",
    );
    s += &header(doc, "How the estimates settle as runs accumulate");
    if panels.is_empty() {
        s += &format!(
            "<text x=\"{ML}\" y=\"110\" fill=\"{TEXT}\">No scalar output gave a value, so there is no estimate to follow.</text>"
        );
    }
    let log2 = |n: f64| n.max(1.0).log2();
    for (pi, p) in panels.iter().enumerate() {
        let y0 = TOP + pi as f64 * (panel_h + gap) + 30.0;
        let o = &doc["outputs"][p.name];
        let unit = text(&o["unit"]);
        let conv = if p.kind == "sobol" {
            &doc["sensitivity"]["outputs"][p.name]["convergence"]
        } else {
            &o["convergence"]
        };
        let pts: Vec<&Value> = conv
            .as_array()
            .map(|a| a.iter().collect())
            .unwrap_or_default();
        let ns: Vec<f64> = pts.iter().map(|c| c["n"].as_f64().unwrap_or(1.0)).collect();
        let sx = Scale::new(
            log2(ns.first().copied().unwrap_or(1.0)),
            log2(ns.last().copied().unwrap_or(2.0)),
            ML + 10.0,
            W - MR - 10.0,
        );
        let xs: Vec<f64> = ns.iter().map(|n| log2(*n)).collect();
        let col = |key: &str| -> Vec<Option<f64>> { pts.iter().map(|c| c[key].as_f64()).collect() };
        let title = match p.kind {
            "stat" => format!("{}: running mean and 5th to 95th percentile", p.name),
            "prob" => format!("{}: running probability of meeting the limit", p.name),
            _ => format!("{}: running total sensitivity index per input", p.name),
        };
        s += &format!(
            "<text x=\"{ML}\" y=\"{:.1}\" fill=\"{TEXT}\" font-weight=\"600\">{}</text>",
            y0 - 24.0,
            esc(&title)
        );
        // x axis
        for (n, x) in ns.iter().zip(&xs) {
            s += &format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"{MUTED}\" font-size=\"10\">{}</text>",
                sx.at(*x),
                y0 + panel_h + 15.0,
                *n as u64
            );
        }
        s += &format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"end\" fill=\"{MUTED}\" font-size=\"11\">{}</text>",
            W - MR,
            y0 + panel_h + 30.0,
            if p.kind == "sobol" { "rows of the design used (n)" } else { "realisations used (n)" }
        );
        match p.kind {
            "stat" => {
                let (mean, p05, p95) = (col("mean"), col("p05"), col("p95"));
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for v in mean.iter().chain(&p05).chain(&p95).flatten() {
                    lo = lo.min(*v);
                    hi = hi.max(*v);
                }
                let (lo, hi) = pad_range(lo, hi);
                let sy = Scale::new(lo, hi, y0 + panel_h, y0);
                s += &y_axis(sy, ML, W - MR, unit);
                let both: Vec<(f64, f64, f64)> = xs
                    .iter()
                    .zip(p05.iter().zip(&p95))
                    .filter_map(|(x, (a, c))| Some((*x, (*a)?, (*c)?)))
                    .collect();
                let mut area = String::new();
                for (i, (x, _, c)) in both.iter().enumerate() {
                    area += &format!(
                        "{}{:.2},{:.2}",
                        if i == 0 { "M" } else { " L" },
                        sx.at(*x),
                        sy.at(*c)
                    );
                }
                for (x, a, _) in both.iter().rev() {
                    area += &format!(" L{:.2},{:.2}", sx.at(*x), sy.at(*a));
                }
                if both.len() > 1 {
                    s += &format!(
                        "<path d=\"{area} Z\" fill=\"{}\" fill-opacity=\"0.28\"><title>running 5th to 95th percentile</title></path>",
                        SERIES[0]
                    );
                }
                for d in polyline(&xs, &mean, sx, sy) {
                    s += &format!(
                        "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>",
                        SERIES[0]
                    );
                }
                for (i, c) in pts.iter().enumerate() {
                    if let Some(m) = mean[i] {
                        s += &format!(
                            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{}\" stroke=\"{SURFACE}\" stroke-width=\"2\"><title>n = {}: mean {}{u}, 5th percentile {}, 95th percentile {}</title></circle>",
                            sx.at(xs[i]), sy.at(m), SERIES[0], c["n"].as_u64().unwrap_or(0),
                            fmt_num(m),
                            fmt_num(p05[i].unwrap_or(f64::NAN)),
                            fmt_num(p95[i].unwrap_or(f64::NAN)),
                            u = esc(&unit_suffix(unit)),
                        );
                    }
                }
                // The final bootstrap interval on the mean.
                if let (Some(l), Some(u), Some(x)) =
                    (o["ci95_low"].as_f64(), o["ci95_high"].as_f64(), xs.last())
                {
                    let x = sx.at(*x) + 9.0;
                    s += &format!(
                        "<path d=\"M{x:.1},{:.1} L{x:.1},{:.1} M{:.1},{:.1} L{:.1},{:.1} M{:.1},{:.1} L{:.1},{:.1}\" stroke=\"{TEXT}\" stroke-width=\"1.2\" fill=\"none\"><title>95% bootstrap confidence interval on the mean: {} to {}</title></path>",
                        sy.at(l), sy.at(u), x - 3.0, sy.at(l), x + 3.0, sy.at(l), x - 3.0, sy.at(u), x + 3.0, sy.at(u),
                        fmt_num(l), fmt_num(u)
                    );
                }
                if let Some(m) = o["mean"].as_f64() {
                    s += &format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{TEXT}\" font-size=\"11\">mean {}{}</text>",
                        W - MR + 16.0, sy.at(m) + 4.0, fmt_num(m), esc(&unit_suffix(unit))
                    );
                }
                s += &legend(
                    &[
                        (SERIES[0], "line", "running mean"),
                        (SERIES[0], "area", "5th to 95th percentile"),
                        (TEXT, "line", "95% interval on mean"),
                    ],
                    W - MR + 16.0,
                    y0 + panel_h - 42.0,
                );
            }
            "prob" => {
                let pr = col("probability_among_evaluated");
                let sy = Scale::new(0.0, 1.0, y0 + panel_h, y0);
                s += &y_axis(sy, ML, W - MR, "");
                for d in polyline(&xs, &pr, sx, sy) {
                    s += &format!(
                        "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>",
                        SERIES[0]
                    );
                }
                for (i, c) in pts.iter().enumerate() {
                    if let Some(v) = pr[i] {
                        s += &format!(
                            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{}\" stroke=\"{SURFACE}\" stroke-width=\"2\"><title>n = {}: share meeting the limit {}</title></circle>",
                            sx.at(xs[i]), sy.at(v), SERIES[0], c["n"].as_u64().unwrap_or(0), fmt_num(v)
                        );
                    }
                }
                let l = &o["limit"];
                if let (Some(p), Some(lo), Some(hi)) = (
                    l["probability"].as_f64(),
                    l["probability_ci95_low"].as_f64(),
                    l["probability_ci95_high"].as_f64(),
                ) {
                    s += &format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{TEXT}\" font-size=\"11\">{} ({} to {})</text>\
                         <text x=\"{:.1}\" y=\"{:.1}\" fill=\"{MUTED}\" font-size=\"10\">{}</text>",
                        W - MR + 16.0, sy.at(p) + 4.0, fmt_num(p), fmt_num(lo), fmt_num(hi),
                        W - MR + 16.0, sy.at(p) + 18.0,
                        esc(&limit_phrase(text(&l["op"]), l["value"].as_f64().unwrap_or(f64::NAN), unit))
                    );
                }
            }
            _ => {
                let inputs: Vec<&str> = doc["inputs"]
                    .as_array()
                    .map(|a| a.iter().map(|i| text(&i["name"])).collect())
                    .unwrap_or_default();
                // Draw the four highest-ranked inputs; colour follows the input's
                // position in the study file, not its rank.
                let entry = &doc["sensitivity"]["outputs"][p.name];
                let mut drawn: Vec<usize> = (0..inputs.len())
                    .filter(|i| entry["inputs"][*i]["rank"].as_u64().is_some_and(|r| r <= 4))
                    .collect();
                drawn.truncate(4);
                let series: Vec<(usize, Vec<Option<f64>>)> = drawn
                    .iter()
                    .map(|i| (*i, pts.iter().map(|c| c["total"][*i].as_f64()).collect()))
                    .collect();
                let (mut lo, mut hi) = (0.0f64, 1.0f64);
                for (_, ys) in &series {
                    for v in ys.iter().flatten() {
                        lo = lo.min(*v);
                        hi = hi.max(*v);
                    }
                }
                let sy = Scale::new(lo, hi, y0 + panel_h, y0);
                s += &y_axis(sy, ML, W - MR, "");
                let mut items = Vec::new();
                for (slot, (i, ys)) in series.iter().enumerate() {
                    let colour = SERIES[slot % SERIES.len()];
                    for d in polyline(&xs, ys, sx, sy) {
                        s += &format!(
                            "<path d=\"{d}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"2\"/>"
                        );
                    }
                    for (j, c) in pts.iter().enumerate() {
                        if let Some(v) = ys[j] {
                            s += &format!(
                                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"{colour}\" stroke=\"{SURFACE}\" stroke-width=\"2\"><title>{}: total index {} from the first {} rows</title></circle>",
                                sx.at(xs[j]), sy.at(v), esc(inputs[*i]), fmt_num(v), c["n"].as_u64().unwrap_or(0)
                            );
                        }
                    }
                    items.push((colour, "line", clip(&input_label(doc, inputs[*i]), 24)));
                }
                let refs: Vec<(&str, &str, &str)> =
                    items.iter().map(|(c, k, l)| (*c, *k, l.as_str())).collect();
                s += &legend(&refs, W - MR + 16.0, y0 + 12.0);
                if inputs.len() > drawn.len() {
                    s += &format!(
                        "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{MUTED}\" font-size=\"10\">top {} of {} inputs</text>",
                        W - MR + 16.0, y0 + 12.0 + drawn.len() as f64 * 18.0, drawn.len(), inputs.len()
                    );
                }
            }
        }
    }
    s += &footer(doc, h - 10.0);
    s += "</svg>";
    s
}

fn td(v: &Value) -> String {
    match v.as_f64() {
        Some(x) => format!("<td class=\"num\">{}</td>", fmt_num(x)),
        None => "<td class=\"num\">n/a</td>".to_string(),
    }
}

/// The self-contained report page: no script, no external resource.
pub fn report_html(doc: &Value) -> String {
    // Every Kshana Pro document is an Ashforde document: the shared furniture of
    // crate::report_brand goes around the renderer's own page, which is unchanged.
    let body = report_html_unbranded(doc);
    let id = crate::report_brand::content_id(&body);
    crate::report_brand::finish_html(
        &body,
        &crate::report_brand::DocMark::new("Uncertainty study", id),
    )
}

fn report_html_unbranded(doc: &Value) -> String {
    let mut h = String::new();
    h += "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
          <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">";
    h += &format!(
        "<title>{} · Kshana Pro uncertainty study</title>",
        esc(text(&doc["title"]))
    );
    h += &format!(
        "<style>\
         :root{{--surface:{SURFACE};--panel:#15130e;--text:{TEXT};--muted:{MUTED};--grid:{GRID};--warn:{LIMIT}}}\
         *{{box-sizing:border-box}}\
         body{{margin:0;background:var(--surface);color:var(--text);font:15px/1.55 {FONT}}}\
         main{{max-width:940px;margin:0 auto;padding:28px 16px 64px}}\
         h1{{font-size:24px;line-height:1.25;margin:0 0 6px}}\
         h2{{font-size:17px;margin:36px 0 10px;padding-top:18px;border-top:1px solid var(--grid)}}\
         p{{margin:8px 0}} .muted{{color:var(--muted)}} code{{font:13px ui-monospace,Menlo,monospace;overflow-wrap:anywhere}}\
         .tag{{display:inline-block;border:1px solid var(--muted);border-radius:4px;padding:0 6px;font-size:12px;letter-spacing:.04em}}\
         .tag.warn{{border-color:var(--warn)}}\
         .tiles{{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:12px;margin:14px 0}}\
         .tile{{background:var(--panel);border:1px solid var(--grid);border-radius:8px;padding:14px 16px}}\
         .tile .big{{font-size:34px;font-weight:650;line-height:1.1;font-variant-numeric:tabular-nums}}\
         .scroll{{overflow-x:auto}} table{{border-collapse:collapse;width:100%;font-size:13px}}\
         th,td{{text-align:left;padding:6px 10px;border-bottom:1px solid var(--grid);vertical-align:top}}\
         th{{color:var(--muted);font-weight:500}} td.num,th.num{{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}}\
         figure{{margin:14px 0}} figure svg{{max-width:100%;height:auto;border:1px solid var(--grid);border-radius:8px}}\
         ul{{padding-left:20px}} li{{margin:4px 0}}\
         </style></head><body><main>"
    );
    let runs = &doc["runs"];
    h += &format!(
        "<p class=\"muted\">Kshana Pro · uncertainty and sensitivity study</p><h1>{}</h1>\
         <p><span class=\"tag\">MODELLED</span> <span class=\"muted\">{}</span></p>",
        esc(text(&doc["title"])),
        esc(text(&doc["label"]).trim_start_matches("MODELLED: "))
    );
    h += &format!(
        "<p>{} runs of the open engine's <code>{}</code> scenario kind ({} finished, {} failed), sampled by {} with n = {}; sensitivity method: {}.</p>",
        runs["total"],
        esc(text(&doc["base"]["kind"])),
        runs["ok"],
        runs["failed"],
        if doc["sampling"]["method"] == "lhs" { "Latin hypercube sampling" } else { "plain random sampling" },
        doc["sampling"]["n"],
        match text(&doc["sensitivity"]["method"]) {
            "sobol" => "Sobol' variance-based indices",
            "src" => "standardised regression coefficients",
            _ => "none",
        },
    );
    if runs["failed"].as_u64().unwrap_or(0) > 0 {
        h += &format!(
            "<p><span class=\"tag warn\">⚠ {} FAILED RUNS</span> Every statistic below is over the successful realisations only. A failed run is recorded, never dropped, and never counted as meeting a limit.</p>",
            runs["failed"]
        );
    }

    // Headline tiles: the probability of meeting each limit.
    let scalar_names = output_names(doc, "outputs");
    let mut tiles = String::new();
    for n in &scalar_names {
        let o = &doc["outputs"][n];
        let l = &o["limit"];
        if let Some(p) = l["probability"].as_f64() {
            tiles += &format!(
                "<div class=\"tile\"><div class=\"muted\">{}</div><div class=\"big\">{:.1} %</div>\
                 <div>probability of meeting the limit, given the stated input uncertainty</div>\
                 <div class=\"muted\">{} · 95 % confidence interval {:.1} % to {:.1} % · {} of {} realisations</div></div>",
                esc(n),
                100.0 * p,
                esc(&limit_phrase(text(&l["op"]), l["value"].as_f64().unwrap_or(f64::NAN), text(&o["unit"]))),
                100.0 * l["probability_ci95_low"].as_f64().unwrap_or(f64::NAN),
                100.0 * l["probability_ci95_high"].as_f64().unwrap_or(f64::NAN),
                l["n_met"],
                l["n_realisations"],
            );
        }
    }
    if !tiles.is_empty() {
        h += &format!("<div class=\"tiles\">{tiles}</div>");
    }

    // Inputs.
    h += "<h2>What was assumed about the inputs</h2>";
    let inputs = doc["inputs"].as_array().cloned().unwrap_or_default();
    if inputs.is_empty() {
        h += "<p>No input is varied; only the engine seed changes from one realisation to the next.</p>";
    } else {
        h += "<div class=\"scroll\"><table><tr><th>Input</th><th>Scenario key</th><th>Distribution</th><th class=\"num\">Base value</th><th>Where the uncertainty comes from</th></tr>";
        for i in &inputs {
            let assumed = i["source_class"] == "ASSUMED";
            h += &format!(
                "<tr><td>{}</td><td><code>{}</code></td><td>{}</td>{}<td>{}{}</td></tr>",
                esc(text(&i["name"])),
                esc(text(&i["key"])),
                esc(text(&i["distribution"])),
                td(&i["nominal"]),
                if assumed {
                    "<span class=\"tag warn\">ASSUMED</span> "
                } else {
                    ""
                },
                esc(text(&i["source"])),
            );
        }
        h += "</table></div><p class=\"muted\">Inputs are sampled independently of each other. Every figure on this page is conditional on these distributions.</p>";
    }
    if doc["sampling"]["vary_seed"] == true {
        h += &format!(
            "<p class=\"muted\">Engine seed: {}.</p>",
            esc(text(&doc["sampling"]["seed_rule"]))
        );
    }

    // Outputs.
    h += "<h2>How sure are we: each output</h2><div class=\"scroll\"><table><tr><th>Output</th><th class=\"num\">Base run</th><th class=\"num\">Mean</th><th class=\"num\">95 % interval on the mean</th><th class=\"num\">Standard deviation</th><th class=\"num\">5th percentile · median · 95th percentile</th><th>Tier</th></tr>";
    for n in &scalar_names {
        let o = &doc["outputs"][n];
        let tier = doc["figure_tiers"]["figures"]
            .as_array()
            .and_then(|f| f.iter().find(|f| f["output"] == *n))
            .map(|f| text(&f["tier"]))
            .unwrap_or("MODELLED");
        let ci = match (o["ci95_low"].as_f64(), o["ci95_high"].as_f64()) {
            (Some(l), Some(u)) => format!("{} to {}", fmt_num(l), fmt_num(u)),
            _ => "n/a".to_string(),
        };
        h += &format!(
            "<tr><td>{} ({})<br><code>{}</code><br><span class=\"muted\">{} values</span></td>{}{}<td class=\"num\">{}</td>{}<td class=\"num\">{} · {} · {}</td><td>{}</td></tr>",
            esc(n),
            esc(text(&o["unit"])),
            esc(text(&o["path"])),
            o["n"],
            td(&o["nominal"]),
            td(&o["mean"]),
            ci,
            td(&o["std"]),
            fmt_num(o["p05"].as_f64().unwrap_or(f64::NAN)),
            fmt_num(o["p50"].as_f64().unwrap_or(f64::NAN)),
            fmt_num(o["p95"].as_f64().unwrap_or(f64::NAN)),
            esc(tier),
        );
    }
    h += "</table></div>";
    h += &format!("<figure>{}</figure>", bands_svg(doc));
    for n in output_names(doc, "bands") {
        let b = &doc["bands"][n];
        let t = floats(&b["t"]);
        let last = t.len().saturating_sub(1);
        h += &format!(
            "<p class=\"muted\">Band <code>{}</code>: {} time steps, {} realisations. At the last step ({} {}): 5th percentile {}, median {}, 95th percentile {} {}.</p>",
            esc(n),
            t.len(),
            b["n"],
            fmt_num(t.get(last).copied().flatten().unwrap_or(f64::NAN)),
            esc(text(&b["time_unit"])),
            fmt_num(b["p05"][last].as_f64().unwrap_or(f64::NAN)),
            fmt_num(b["p50"][last].as_f64().unwrap_or(f64::NAN)),
            fmt_num(b["p95"][last].as_f64().unwrap_or(f64::NAN)),
            esc(text(&b["unit"])),
        );
    }

    // Sensitivity.
    h += "<h2>What drives it: sensitivity to each input</h2>";
    let method = text(&doc["sensitivity"]["method"]);
    h += &format!(
        "<p class=\"muted\">Cost: {}.</p>",
        esc(text(&doc["sensitivity"]["cost"]))
    );
    h += &format!("<figure>{}</figure>", sensitivity_svg(doc));
    for n in sensitivity_names(doc) {
        let e = &doc["sensitivity"]["outputs"][n];
        h += &format!("<h3>{}</h3>", esc(n));
        if method == "sobol" {
            h += "<div class=\"scroll\"><table><tr><th class=\"num\">Rank</th><th>Input</th><th class=\"num\">First-order index</th><th class=\"num\">95 % interval</th><th class=\"num\">Total index</th><th class=\"num\">95 % interval</th></tr>";
            for r in ranked(e) {
                if let Some(why) = r["reason"].as_str() {
                    h += &format!(
                        "<tr><td class=\"num\">n/a</td><td>{}</td><td colspan=\"4\">no value: {}</td></tr>",
                        esc(&input_label(doc, text(&r["input"]))),
                        esc(why)
                    );
                    continue;
                }
                h += &format!(
                    "<tr><td class=\"num\">{}</td><td>{}</td>{}<td class=\"num\">{} to {}</td>{}<td class=\"num\">{} to {}</td></tr>",
                    r["rank"],
                    esc(&input_label(doc, text(&r["input"]))),
                    td(&r["first_order"]),
                    fmt_num(r["first_order_ci95_low"].as_f64().unwrap_or(f64::NAN)),
                    fmt_num(r["first_order_ci95_high"].as_f64().unwrap_or(f64::NAN)),
                    td(&r["total"]),
                    fmt_num(r["total_ci95_low"].as_f64().unwrap_or(f64::NAN)),
                    fmt_num(r["total_ci95_high"].as_f64().unwrap_or(f64::NAN)),
                );
            }
            h += "</table></div>";
            if let Some(v) = e["interaction_share"].as_f64() {
                h += &format!(
                    "<p>Share not explained by any single input alone: <strong>{}</strong>. <span class=\"muted\">{}.</span></p>",
                    fmt_num(v),
                    esc(text(&e["interaction_share_meaning"]))
                );
            }
        } else {
            h += "<div class=\"scroll\"><table><tr><th class=\"num\">Rank</th><th>Input</th><th class=\"num\">Standardised coefficient</th><th class=\"num\">Slope (output per unit of input)</th></tr>";
            for r in ranked(e) {
                if let Some(why) = r["reason"].as_str() {
                    h += &format!(
                        "<tr><td class=\"num\">n/a</td><td>{}</td><td colspan=\"2\">no value: {}</td></tr>",
                        esc(&input_label(doc, text(&r["input"]))),
                        esc(why)
                    );
                    continue;
                }
                h += &format!(
                    "<tr><td class=\"num\">{}</td><td>{}</td>{}{}</tr>",
                    r["rank"],
                    esc(&input_label(doc, text(&r["input"]))),
                    td(&r["src"]),
                    td(&r["coefficient"]),
                );
            }
            h += "</table></div>";
            h += &format!(
                "<p>{}{}</p>",
                if e["reliable"] == false {
                    "<span class=\"tag warn\">⚠ NOT RELIABLE</span> "
                } else {
                    ""
                },
                esc(text(&e["statement"]))
            );
        }
        if let Some(why) = e["reason"].as_str() {
            h += &format!("<p>No indices for this output: {}.</p>", esc(why));
        }
    }

    h += "<h2>Has it settled: convergence</h2>";
    h += &format!("<figure>{}</figure>", convergence_svg(doc));

    h += "<h2>What this study does not model</h2><ul>";
    for x in doc["not_modelled"].as_array().cloned().unwrap_or_default() {
        h += &format!("<li>{}</li>", esc(text(&x)));
    }
    h += "</ul>";

    let rep = &doc["reproducibility"];
    h += "<h2>Reproducibility</h2><div class=\"scroll\"><table>";
    for (k, v) in [
        (
            "Study hash (SHA-256, Secure Hash Algorithm 256-bit, of the canonical study)",
            text(&doc["scenario_hash"]).to_string(),
        ),
        ("Study seed", doc["seed"].to_string()),
        (
            "Open engine version",
            text(&doc["engine_version"]).to_string(),
        ),
        (
            "Kshana Pro version",
            text(&doc["overlay_version"]).to_string(),
        ),
        ("Runs", rep["runs_total"].to_string()),
        ("Run digest", text(&rep["run_digest"]).to_string()),
        (
            "Base run result hash",
            text(&rep["base_result_sha256"]).to_string(),
        ),
    ] {
        h += &format!(
            "<tr><th>{}</th><td><code>{}</code></td></tr>",
            esc(k),
            esc(&v)
        );
    }
    h += &format!(
        "</table></div><p class=\"muted\">{}. {}.</p>",
        esc(text(&rep["rebuild"])),
        esc(text(&rep["run_digest_rule"]))
    );
    h += "</main></body></html>\n";
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_labelled_with_four_significant_figures() {
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(92.124_475), "92.124");
        assert_eq!(fmt_num(0.820_66), "0.8207");
        assert_eq!(fmt_num(-3.5), "-3.500");
        assert_eq!(fmt_num(1234.56), "1234.6");
        assert_eq!(fmt_num(1.575e-19), "1.575e-19");
        assert_eq!(fmt_num(f64::NAN), "n/a");
        assert_eq!(fmt_index(9.372e-4), "0.001");
        assert_eq!(fmt_index(-1.5e-4), "0.000");
        assert_eq!(fmt_index(0.98803), "0.988");
        assert_eq!(fmt_tick(20.0), "20");
        assert_eq!(fmt_tick(-0.5), "-0.5");
        assert_eq!(fmt_tick(1000.0), "1000");
        assert_eq!(fmt_tick(0.0), "0");
        assert_eq!(clip("other_losses (ASSUMED)", 10), "other_los…");
    }

    #[test]
    fn ticks_are_round_and_inside_the_range() {
        let unit = ticks(0.0, 1.0);
        assert_eq!(unit.len(), 6);
        for (got, want) in unit.iter().zip([0.0, 0.2, 0.4, 0.6, 0.8, 1.0]) {
            assert!((got - want).abs() < 1e-12);
        }
        let t = ticks(-23.0, 47.0);
        assert!(t
            .iter()
            .all(|v| *v >= -23.0 && *v <= 47.0 && (v / 20.0).fract() == 0.0));
        assert_eq!(ticks(5.0, 5.0), vec![5.0]);
    }

    #[test]
    fn markup_characters_in_a_title_are_escaped() {
        assert_eq!(esc("a<b & \"c\">"), "a&lt;b &amp; &quot;c&quot;&gt;");
    }
}
