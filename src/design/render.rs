// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The chart (`design.pareto.svg`) and the report (`design.report.html`) of a study.
//!
//! Both are drawn from the result document alone, so they can show nothing the
//! document does not state. The chart uses the open engine's chart surface and inks.
//! Identity never rests on colour alone: evaluated designs are small grey dots, front
//! designs are ringed blue discs joined by a staircase, the knee carries a ring and a
//! label, and the reference design is an orange diamond with a label. The report is one
//! self-contained file: no script, no font, no image and no link is fetched.

use serde_json::Value;

const SURFACE: &str = "#0c0b08";
const INK: &str = "#bcb3a3";
const INK_MUTED: &str = "#8c8273";
const GRID: &str = "#262019";
const AXIS: &str = "#342c21";
const CONTEXT: &str = "#6f6450";
const FRONT: &str = "#3987e5";
const REFERENCE: &str = "#d95926";
/// One-hue ramp for a third objective, worst to best.
const RAMP: [&str; 5] = ["#184f95", "#256abf", "#3987e5", "#6da7ec", "#b7d3f6"];

/// Escape text for SVG and HTML.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A number as a person writes it: whole numbers without a point, others with up to
/// four decimals and no trailing zeros.
pub fn num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        return format!("{x:.0}");
    }
    let s = format!("{x:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(num).unwrap_or_else(|| n.to_string()),
        Value::Null => "—".into(),
        other => other.to_string(),
    }
}

struct Axis {
    name: String,
    unit: String,
    minimise: bool,
    lo: f64,
    hi: f64,
    ticks: Vec<f64>,
}

fn nice_step(span: f64, target: f64) -> f64 {
    let raw = (span / target).max(f64::MIN_POSITIVE);
    let mag = 10f64.powf(raw.log10().floor());
    let r = raw / mag;
    let m = if r <= 1.0 {
        1.0
    } else if r <= 2.0 {
        2.0
    } else if r <= 5.0 {
        5.0
    } else {
        10.0
    };
    m * mag
}

fn axis(objective: &Value, values: &[f64]) -> Axis {
    let mut lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mut hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !lo.is_finite() || !hi.is_finite() {
        lo = 0.0;
        hi = 1.0;
    }
    if hi <= lo {
        let pad = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
        lo -= pad;
        hi += pad;
    }
    let step = nice_step(hi - lo, 5.0);
    let lo_t = (lo / step).floor() * step;
    let hi_t = (hi / step).ceil() * step;
    let n = ((hi_t - lo_t) / step).round() as usize;
    let ticks = (0..=n).map(|k| lo_t + k as f64 * step).collect();
    let unit = objective["unit"].as_str().unwrap_or("").to_string();
    Axis {
        name: objective["name"].as_str().unwrap_or("").to_string(),
        unit: if unit.starts_with("unstated") || unit == "1" {
            String::new()
        } else {
            unit
        },
        minimise: objective["sense"].as_str() == Some("min"),
        lo: lo_t,
        hi: hi_t,
        ticks,
    }
}

impl Axis {
    fn title(&self) -> String {
        let unit = if self.unit.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.unit)
        };
        format!(
            "{}{unit} · {} is better",
            self.name,
            if self.minimise { "lower" } else { "higher" }
        )
    }
    fn frac(&self, x: f64) -> f64 {
        (x - self.lo) / (self.hi - self.lo)
    }
}

fn objective_values(item: &Value, names: &[String]) -> Option<Vec<f64>> {
    names
        .iter()
        .map(|n| item["objectives"][n.as_str()].as_f64())
        .collect()
}

fn describe(item: &Value, doc: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(vars) = item["variables"].as_object() {
        let order: Vec<String> = doc["space"]["variables"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        for name in &order {
            if let Some(v) = vars.get(name) {
                parts.push(format!("{name} {}", value_text(v)));
            }
        }
    }
    if let Some(objs) = doc["objectives"].as_array() {
        for o in objs {
            let name = o["name"].as_str().unwrap_or("");
            parts.push(format!("{name} {}", value_text(&item["objectives"][name])));
        }
    }
    parts.join(" · ")
}

/// `design.pareto.svg`: every scored design, the front, the knee and the reference.
pub fn pareto_svg(doc: &Value) -> String {
    let objectives = doc["objectives"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = objectives
        .iter()
        .map(|o| o["name"].as_str().unwrap_or("").to_string())
        .collect();
    let empty = Vec::new();
    let evaluated = doc["evaluated"].as_array().unwrap_or(&empty);
    let front = doc["front"].as_array().unwrap_or(&empty);
    let reference = &doc["reference"];
    let three = names.len() == 3;

    let scored: Vec<(&Value, Vec<f64>)> = evaluated
        .iter()
        .filter(|e| e["status"].as_str() == Some("ok"))
        .filter_map(|e| objective_values(e, &names).map(|o| (e, o)))
        .collect();
    let front_pts: Vec<(&Value, Vec<f64>)> = front
        .iter()
        .filter_map(|f| objective_values(f, &names).map(|o| (f, o)))
        .collect();
    let reference_pt = objective_values(reference, &names);

    let mut xs: Vec<f64> = scored.iter().map(|(_, o)| o[0]).collect();
    let mut ys: Vec<f64> = scored.iter().map(|(_, o)| o[1]).collect();
    if let Some(r) = &reference_pt {
        xs.push(r[0]);
        ys.push(r[1]);
    }
    let ax = axis(objectives.first().unwrap_or(&Value::Null), &xs);
    let ay = axis(objectives.get(1).unwrap_or(&Value::Null), &ys);

    let (w, h) = (880.0_f64, 560.0_f64);
    let (ml, mr, mt, mb) = (72.0_f64, 28.0_f64, 96.0_f64, 96.0_f64);
    let (pw, ph) = (w - ml - mr, h - mt - mb);
    let px = |x: f64| ml + ax.frac(x) * pw;
    let py = |y: f64| mt + (1.0 - ay.frac(y)) * ph;

    let title = doc["title"].as_str().unwrap_or("");
    let statement = doc["front_statement"].as_str().unwrap_or("");
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"sans-serif\" font-size=\"12\" fill=\"{INK}\" role=\"img\" aria-label=\"{}\">",
        esc(&format!("Pareto front: {title}"))
    );
    s += &format!("<rect width=\"{w:.0}\" height=\"{h:.0}\" fill=\"{SURFACE}\"/>");
    s += &format!(
        "<text x=\"24\" y=\"30\" font-size=\"16\" font-weight=\"bold\">{}</text>",
        esc(title)
    );
    s += &format!(
        "<text x=\"24\" y=\"50\" font-size=\"12\" fill=\"{INK_MUTED}\">{} · {} front designs at {} distinct points</text>",
        esc(&capitalise(statement)),
        doc["front_summary"]["designs"].as_u64().unwrap_or(0),
        doc["front_summary"]["distinct_objective_points"]
            .as_u64()
            .unwrap_or(0),
    );

    // Grid and axes: hairlines one step off the surface.
    for &t in &ax.ticks {
        let x = px(t);
        s += &format!(
            "<line x1=\"{x:.1}\" y1=\"{mt:.0}\" x2=\"{x:.1}\" y2=\"{:.0}\" stroke=\"{GRID}\"/>",
            mt + ph
        );
        s += &format!(
            "<text x=\"{x:.1}\" y=\"{:.0}\" text-anchor=\"middle\" font-size=\"11\" fill=\"{INK_MUTED}\">{}</text>",
            mt + ph + 18.0,
            num(t)
        );
    }
    for &t in &ay.ticks {
        let y = py(t);
        s += &format!(
            "<line x1=\"{ml:.0}\" y1=\"{y:.1}\" x2=\"{:.0}\" y2=\"{y:.1}\" stroke=\"{GRID}\"/>",
            ml + pw
        );
        s += &format!(
            "<text x=\"{:.0}\" y=\"{:.1}\" text-anchor=\"end\" font-size=\"11\" fill=\"{INK_MUTED}\">{}</text>",
            ml - 8.0,
            y + 4.0,
            num(t)
        );
    }
    s += &format!(
        "<line x1=\"{ml:.0}\" y1=\"{:.0}\" x2=\"{:.0}\" y2=\"{:.0}\" stroke=\"{AXIS}\"/>",
        mt + ph,
        ml + pw,
        mt + ph
    );
    s += &format!(
        "<line x1=\"{ml:.0}\" y1=\"{mt:.0}\" x2=\"{ml:.0}\" y2=\"{:.0}\" stroke=\"{AXIS}\"/>",
        mt + ph
    );
    s += &format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" text-anchor=\"middle\" fill=\"{INK_MUTED}\">{}</text>",
        ml + pw / 2.0,
        mt + ph + 40.0,
        esc(&ax.title())
    );
    let yc = mt + ph / 2.0;
    s += &format!(
        "<text x=\"20\" y=\"{yc:.0}\" text-anchor=\"middle\" fill=\"{INK_MUTED}\" transform=\"rotate(-90 20 {yc:.0})\">{}</text>",
        esc(&ay.title())
    );

    // Every scored design that is not on the front: small grey context dots.
    let with_titles = scored.len() <= 4000;
    for (e, o) in &scored {
        if e["on_front"].as_bool() == Some(true) {
            continue;
        }
        s += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"2.5\" fill=\"{CONTEXT}\" fill-opacity=\"0.6\">",
            px(o[0]),
            py(o[1])
        );
        if with_titles {
            s += &format!("<title>{}</title>", esc(&describe(e, doc)));
        }
        s += "</circle>";
    }

    // The front line: a staircase, because no design exists between two neighbours.
    // With three objectives the line joins the designs that are also non-dominated on
    // the first two objectives alone.
    let mut line: Vec<&Vec<f64>> = front_pts.iter().map(|(_, o)| o).collect();
    if three {
        let sx = if ax.minimise { 1.0 } else { -1.0 };
        let sy = if ay.minimise { 1.0 } else { -1.0 };
        let all = line.clone();
        line.retain(|p| {
            !all.iter().any(|q| {
                sx * q[0] <= sx * p[0]
                    && sy * q[1] <= sy * p[1]
                    && (sx * q[0] < sx * p[0] || sy * q[1] < sy * p[1])
            })
        });
    }
    line.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    line.dedup_by(|a, b| a[0] == b[0] && a[1] == b[1]);
    if line.len() >= 2 {
        let mut d = String::new();
        for (i, p) in line.iter().enumerate() {
            if i == 0 {
                d += &format!("M{:.1} {:.1}", px(p[0]), py(p[1]));
            } else if ax.minimise {
                // The earlier design holds until the next one is reached.
                d += &format!(" H{:.1} V{:.1}", px(p[0]), py(p[1]));
            } else {
                d += &format!(" V{:.1} H{:.1}", py(p[1]), px(p[0]));
            }
        }
        s += &format!(
            "<path d=\"{d}\" fill=\"none\" stroke=\"{FRONT}\" stroke-width=\"2\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/>"
        );
    }

    // Front designs: discs with a surface ring. With three objectives the fill is a
    // one-hue ramp of the third.
    let third: Option<(f64, f64, bool)> = three.then(|| {
        let zs: Vec<f64> = front_pts.iter().map(|(_, o)| o[2]).collect();
        (
            zs.iter().copied().fold(f64::INFINITY, f64::min),
            zs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            objectives[2]["sense"].as_str() == Some("min"),
        )
    });
    let ramp_of = |z: f64| -> &'static str {
        match third {
            Some((lo, hi, minimise)) if hi > lo => {
                let mut f = (z - lo) / (hi - lo);
                if minimise {
                    f = 1.0 - f;
                }
                RAMP[((f * 5.0).floor() as usize).min(4)]
            }
            Some(_) => RAMP[2],
            None => FRONT,
        }
    };
    let knee_id = doc["knee"]["front_id"].as_str();
    let show_knee = doc["knee"]["exists"].as_bool() == Some(true);
    for (f, o) in &front_pts {
        s += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"5\" fill=\"{}\" stroke=\"{SURFACE}\" stroke-width=\"2\"><title>front {} · {}</title></circle>",
            px(o[0]),
            py(o[1]),
            ramp_of(if three { o[2] } else { 0.0 }),
            esc(f["id"].as_str().unwrap_or("")),
            esc(&describe(f, doc))
        );
    }
    // Direct labels: the knee and the reference only.
    let label_at = |x: f64, y: f64, text: &str| -> String {
        // Sans-serif at 12 px averages about 6.4 px a character; keep the label inside
        // the chart, to the right of the mark when it fits and to the left when not.
        let width = 6.4 * text.chars().count() as f64;
        let (tx, anchor) = if x + 12.0 + width <= w - 8.0 {
            (x + 12.0, "start")
        } else if x - 12.0 - width >= 8.0 {
            (x - 12.0, "end")
        } else {
            (w - 8.0, "end")
        };
        let ty = (y + 4.0).clamp(mt + 12.0, mt + ph - 6.0);
        format!(
            "<text x=\"{tx:.1}\" y=\"{ty:.1}\" text-anchor=\"{anchor}\" font-size=\"12\" stroke=\"{SURFACE}\" stroke-width=\"4\" paint-order=\"stroke\" stroke-linejoin=\"round\">{}</text>",
            esc(text)
        )
    };
    let short = |item: &Value| -> String {
        objectives
            .iter()
            .map(|o| {
                let name = o["name"].as_str().unwrap_or("");
                format!("{name} {}", value_text(&item["objectives"][name]))
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    if show_knee {
        if let Some((f, o)) = front_pts
            .iter()
            .find(|(f, _)| f["id"].as_str() == knee_id && knee_id.is_some())
        {
            let (x, y) = (px(o[0]), py(o[1]));
            s += &format!(
                "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"10\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"2\"><title>knee · {}</title></circle>",
                esc(&describe(f, doc))
            );
            s += &label_at(x, y + 18.0, &format!("knee: {}", short(f)));
        }
    }
    if let Some(r) = &reference_pt {
        let (x, y) = (px(r[0]), py(r[1]));
        let label = reference["label"].as_str().unwrap_or("reference");
        s += &format!(
            "<path d=\"M{x:.1} {:.1} L{:.1} {y:.1} L{x:.1} {:.1} L{:.1} {y:.1} Z\" fill=\"{REFERENCE}\" stroke=\"{SURFACE}\" stroke-width=\"2\"><title>{} · {}</title></path>",
            y - 8.0,
            x + 8.0,
            y + 8.0,
            x - 8.0,
            esc(label),
            esc(&short(reference))
        );
        s += &label_at(x, y - 14.0, &format!("{label}: {}", short(reference)));
    }

    // Legend: always present, marks beside plain-ink text.
    let ly = 74.0;
    let mut lx = ml;
    let entry = |mark: String, text: &str, lx: &mut f64| -> String {
        let out = format!(
            "{mark}<text x=\"{:.0}\" y=\"{:.0}\" font-size=\"11\" fill=\"{INK_MUTED}\">{}</text>",
            *lx + 18.0,
            ly + 4.0,
            esc(text)
        );
        *lx += 30.0 + 6.2 * text.chars().count() as f64;
        out
    };
    let m = format!(
        "<circle cx=\"{:.0}\" cy=\"{ly:.0}\" r=\"2.5\" fill=\"{CONTEXT}\" fill-opacity=\"0.6\"/>",
        lx + 7.0
    );
    s += &entry(m, "evaluated design", &mut lx);
    let m = format!(
        "<line x1=\"{:.0}\" y1=\"{ly:.0}\" x2=\"{:.0}\" y2=\"{ly:.0}\" stroke=\"{FRONT}\" stroke-width=\"2\"/><circle cx=\"{:.0}\" cy=\"{ly:.0}\" r=\"5\" fill=\"{FRONT}\" stroke=\"{SURFACE}\" stroke-width=\"2\"/>",
        lx,
        lx + 14.0,
        lx + 7.0
    );
    s += &entry(
        m,
        if three {
            "front design (line: front on the two plotted objectives)"
        } else {
            "front design"
        },
        &mut lx,
    );
    if show_knee {
        let m = format!(
            "<circle cx=\"{:.0}\" cy=\"{ly:.0}\" r=\"6\" fill=\"none\" stroke=\"{INK}\" stroke-width=\"2\"/>",
            lx + 7.0
        );
        s += &entry(m, "knee", &mut lx);
    }
    if reference_pt.is_some() {
        let c = lx + 7.0;
        let m = format!(
            "<path d=\"M{c:.0} {:.0} L{:.0} {ly:.0} L{c:.0} {:.0} L{:.0} {ly:.0} Z\" fill=\"{REFERENCE}\"/>",
            ly - 6.0,
            c + 6.0,
            ly + 6.0,
            c - 6.0
        );
        s += &entry(m, "reference design", &mut lx);
    }
    if let Some((lo, hi, minimise)) = third {
        // The third objective's ramp, worst to best, with its two ends labelled.
        let o = &objectives[2];
        let unit = o["unit"].as_str().unwrap_or("");
        let (worst, best) = if minimise { (hi, lo) } else { (lo, hi) };
        let y = h - 38.0;
        s += &format!(
            "<text x=\"{ml:.0}\" y=\"{:.0}\" font-size=\"11\" fill=\"{INK_MUTED}\">front disc shade: {} {} </text>",
            y + 4.0,
            esc(o["name"].as_str().unwrap_or("")),
            esc(&num(worst)),
        );
        let start = ml
            + 6.4
                * (format!(
                    "front disc shade: {} {} ",
                    o["name"].as_str().unwrap_or(""),
                    num(worst)
                )
                .chars()
                .count() as f64);
        for (i, c) in RAMP.iter().enumerate() {
            s += &format!(
                "<rect x=\"{:.0}\" y=\"{:.0}\" width=\"18\" height=\"10\" fill=\"{c}\"/>",
                start + i as f64 * 20.0,
                y - 5.0
            );
        }
        s += &format!(
            "<text x=\"{:.0}\" y=\"{:.0}\" font-size=\"11\" fill=\"{INK_MUTED}\">{}{} (best)</text>",
            start + 106.0,
            y + 4.0,
            esc(&num(best)),
            if unit.is_empty() || unit == "1" || unit.starts_with("unstated") {
                String::new()
            } else {
                format!(" {}", esc(unit))
            }
        );
    }

    // Footer: what made the chart.
    let hash = doc["scenario_hash"].as_str().unwrap_or("");
    s += &format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" text-anchor=\"end\" font-size=\"10\" fill=\"#62594b\">Kshana Pro {} · engine {} · study {} · seed {} · MODELLED</text>",
        w - 8.0,
        h - 8.0,
        esc(doc["overlay_version"].as_str().unwrap_or("")),
        esc(doc["engine_version"].as_str().unwrap_or("")),
        esc(&hash[..hash.len().min(12)]),
        doc["seed"].as_u64().unwrap_or(0),
    );
    s += "</svg>";
    s
}

fn table(head: &[String], rows: &[Vec<String>], numeric_from: usize) -> String {
    let mut t = String::from("<div class=\"scroll\"><table><thead><tr>");
    for (i, h) in head.iter().enumerate() {
        t += &format!(
            "<th{}>{}</th>",
            if i >= numeric_from {
                " class=\"n\""
            } else {
                ""
            },
            esc(h)
        );
    }
    t += "</tr></thead><tbody>";
    for r in rows {
        t += "<tr>";
        for (i, c) in r.iter().enumerate() {
            t += &format!(
                "<td{}>{}</td>",
                if i >= numeric_from {
                    " class=\"n\""
                } else {
                    ""
                },
                c
            );
        }
        t += "</tr>";
    }
    t += "</tbody></table></div>";
    t
}

/// `design.report.html`: one self-contained page.
pub fn report_html(doc: &Value, svg: &str) -> String {
    let empty = Vec::new();
    let title = doc["title"].as_str().unwrap_or("Design study");
    let variables: Vec<&Value> = doc["space"]["variables"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .collect();
    let objectives: Vec<&Value> = doc["objectives"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .collect();
    let with_unit = |v: &Value| -> String {
        let unit = v["unit"].as_str().unwrap_or("");
        let name = v["name"].as_str().unwrap_or("");
        if unit.is_empty() || unit == "1" || unit.starts_with("unstated") {
            name.to_string()
        } else {
            format!("{name} ({unit})")
        }
    };

    // Front table.
    let mut head = vec!["Design".to_string(), "Scenario file".to_string()];
    head.extend(variables.iter().map(|v| with_unit(v)));
    head.extend(objectives.iter().map(|o| with_unit(o)));
    head.push("Result SHA-256".into());
    let front = doc["front"].as_array().unwrap_or(&empty);
    let shown = front.len().min(200);
    let rows: Vec<Vec<String>> = front[..shown]
        .iter()
        .map(|f| {
            let mut r = vec![
                format!(
                    "{}{}",
                    esc(f["id"].as_str().unwrap_or("")),
                    if f["knee"].as_bool() == Some(true) {
                        " <span class=\"tag\">knee</span>"
                    } else {
                        ""
                    }
                ),
                format!("<code>{}</code>", esc(f["file"].as_str().unwrap_or(""))),
            ];
            for v in &variables {
                r.push(esc(&value_text(
                    &f["variables"][v["name"].as_str().unwrap_or("")],
                )));
            }
            for o in &objectives {
                r.push(esc(&value_text(
                    &f["objectives"][o["name"].as_str().unwrap_or("")],
                )));
            }
            let sha = f["result_sha256"].as_str().unwrap_or("");
            r.push(format!("<code>{}…</code>", esc(&sha[..sha.len().min(16)])));
            r
        })
        .collect();
    let front_table = table(&head, &rows, 2);

    // Objectives and variables.
    let obj_rows: Vec<Vec<String>> = objectives
        .iter()
        .map(|o| {
            vec![
                esc(o["name"].as_str().unwrap_or("")),
                esc(if o["sense"].as_str() == Some("min") {
                    "lower is better"
                } else {
                    "higher is better"
                }),
                match o["source"].as_str() {
                    Some("variable") => format!(
                        "design variable <code>{}</code>",
                        esc(o["variable"].as_str().unwrap_or(""))
                    ),
                    _ => format!(
                        "engine result <code>{}</code>",
                        esc(o["path"].as_str().unwrap_or(""))
                    ),
                },
                esc(o["unit"].as_str().unwrap_or("")),
            ]
        })
        .collect();
    let var_rows: Vec<Vec<String>> = variables
        .iter()
        .map(|v| {
            let levels = match v["values"].as_array() {
                Some(a) => a.iter().map(value_text).collect::<Vec<_>>().join(", "),
                None => format!(
                    "{} to {} ({} levels)",
                    value_text(&v["first"]),
                    value_text(&v["last"]),
                    value_text(&v["level_count"])
                ),
            };
            vec![
                esc(v["name"].as_str().unwrap_or("")),
                format!("<code>{}</code>", esc(v["key"].as_str().unwrap_or(""))),
                esc(&levels),
                esc(v["unit"].as_str().unwrap_or("")),
            ]
        })
        .collect();

    // Reference.
    let reference = &doc["reference"];
    let reference_html = if reference.is_null() {
        String::new()
    } else {
        let mut rows = vec![vec![
            "Given as".to_string(),
            esc(reference["given_as"].as_str().unwrap_or("")),
        ]];
        for o in &objectives {
            rows.push(vec![
                esc(&with_unit(o)),
                esc(&value_text(
                    &reference["objectives"][o["name"].as_str().unwrap_or("")],
                )),
            ]);
        }
        let list = |v: &Value| -> String {
            match v.as_array() {
                Some(a) if !a.is_empty() => a.iter().map(value_text).collect::<Vec<_>>().join(", "),
                _ => "none".into(),
            }
        };
        rows.push(vec![
            "Front designs that dominate it".into(),
            esc(&list(&reference["dominated_by"])),
        ]);
        rows.push(vec![
            "Front designs it dominates".into(),
            esc(&list(&reference["dominates"])),
        ]);
        rows.push(vec![
            "Reading".into(),
            esc(reference["statement"]
                .as_str()
                .or(reference["error"].as_str())
                .unwrap_or("")),
        ]);
        format!(
            "<h2>Reference design: {}</h2><p>The reference is scored by the same engine run as every other design. \"Dominates\" means at least as good on every objective of this study and better on one; it says nothing about objectives the study does not state.</p>{}",
            esc(reference["label"].as_str().unwrap_or("")),
            table(&["".into(), "".into()], &rows, 9)
        )
    };

    // Labels.
    let tier_rows: Vec<Vec<String>> = doc["figure_tiers"]["figures"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|f| {
            vec![
                format!("<code>{}</code>", esc(f["path"].as_str().unwrap_or(""))),
                format!(
                    "<span class=\"tag\">{}</span>",
                    esc(f["tier"].as_str().unwrap_or(""))
                ),
                esc(f["requirement"].as_str().unwrap_or("")),
            ]
        })
        .collect();
    let untiered_rows: Vec<Vec<String>> = doc["figure_tiers"]["untiered"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|f| {
            vec![
                format!("<code>{}</code>", esc(f["path"].as_str().unwrap_or(""))),
                "<span class=\"tag\">no label</span>".to_string(),
                esc(f["reason"].as_str().unwrap_or("")),
            ]
        })
        .collect();
    let mut all_tiers = tier_rows;
    all_tiers.extend(untiered_rows);
    let not_modelled: String = doc["not_modelled"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|n| format!("<li>{}</li>", esc(n.as_str().unwrap_or(""))))
        .collect();

    // Convergence.
    let convergence = doc["convergence"].as_array().unwrap_or(&empty);
    let convergence_html = if convergence.is_empty() {
        String::new()
    } else {
        let stride = convergence.len().div_ceil(25).max(1);
        let rows: Vec<Vec<String>> = convergence
            .iter()
            .enumerate()
            .filter(|(i, _)| i % stride == 0 || *i == convergence.len() - 1)
            .map(|(_, c)| {
                vec![
                    value_text(&c["generation"]),
                    value_text(&c["evaluations"]),
                    value_text(&c["front_size"]),
                    value_text(&c["hypervolume"]),
                ]
            })
            .collect();
        format!(
            "<h2>Search progress</h2><p>The front found so far after each generation of the search. Hypervolume is the share of the box between the best and the worst value of each objective that the front dominates; it flattens when more runs stop improving the front.</p>{}",
            table(
                &[
                    "Generation".into(),
                    "Designs run".into(),
                    "Front size".into(),
                    "Hypervolume".into()
                ],
                &rows,
                0
            )
        )
    };

    let work = &doc["work"];
    let space = &doc["space"];
    let facts = [
        ("Designs run", value_text(&work["evaluations"])),
        (
            "Feasible designs in the space",
            if space["feasible"].is_null() {
                format!("not counted (grid of {})", value_text(&space["grid_size"]))
            } else {
                value_text(&space["feasible"])
            },
        ),
        (
            "Rejected by a design constraint, never run",
            value_text(&work["infeasible"]),
        ),
        ("Runs the engine rejected", value_text(&work["failed"])),
        (
            "Runs with no value for an objective",
            value_text(&work["no_value"]),
        ),
        (
            "Runs that broke a result constraint",
            value_text(&work["constraint_violated"]),
        ),
        (
            "Designs on the front",
            value_text(&doc["front_summary"]["designs"]),
        ),
        (
            "Distinct points on the front",
            value_text(&doc["front_summary"]["distinct_objective_points"]),
        ),
        ("Search", value_text(&doc["search"]["algorithm"])),
    ];
    let facts_html: String = facts
        .iter()
        .map(|(k, v)| format!("<div><dt>{}</dt><dd>{}</dd></div>", esc(k), esc(v)))
        .collect();
    let rep = &doc["reproducibility"];
    let rep_rows = vec![
        vec![
            "Study hash (SHA-256 of the canonical study)".to_string(),
            format!(
                "<code>{}</code>",
                esc(doc["scenario_hash"].as_str().unwrap_or(""))
            ),
        ],
        vec!["Seed".to_string(), value_text(&doc["seed"])],
        vec![
            "Open engine version".to_string(),
            esc(doc["engine_version"].as_str().unwrap_or("")),
        ],
        vec![
            "Kshana Pro version".to_string(),
            esc(doc["overlay_version"].as_str().unwrap_or("")),
        ],
        vec!["Engine runs".to_string(), value_text(&rep["engine_runs"])],
        vec![
            "Run digest".to_string(),
            format!(
                "<code>{}</code>",
                esc(rep["run_digest"].as_str().unwrap_or(""))
            ),
        ],
        vec![
            "Run digest rule".to_string(),
            esc(rep["run_digest_rule"].as_str().unwrap_or("")),
        ],
        vec![
            "Determinism".to_string(),
            esc(rep["deterministic"].as_str().unwrap_or("")),
        ],
        vec![
            "How to check a front design".to_string(),
            esc(rep["check"].as_str().unwrap_or("")),
        ],
    ];

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Kshana Pro design study</title>
<style>
:root {{ color-scheme: light dark; --bg:#fbfaf7; --fg:#1b1813; --muted:#5f574a; --line:#ddd6c8; --card:#f3efe6; --accent:#1c5cab; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg:#0c0b08; --fg:#e6dfd2; --muted:#a2988a; --line:#342c21; --card:#15130f; --accent:#6da7ec; }} }}
* {{ box-sizing: border-box; }}
body {{ margin:0; background:var(--bg); color:var(--fg); font:15px/1.55 system-ui, -apple-system, "Segoe UI", sans-serif; }}
main {{ max-width:1040px; margin:0 auto; padding:32px 16px 64px; }}
h1 {{ font-size:26px; line-height:1.25; margin:0 0 6px; text-wrap:balance; }}
h2 {{ font-size:18px; margin:40px 0 10px; }}
p {{ margin:0 0 12px; max-width:75ch; }}
.kicker {{ color:var(--muted); font-size:13px; letter-spacing:.04em; text-transform:uppercase; margin:0 0 8px; }}
.label {{ border:1px solid var(--line); border-left:4px solid var(--accent); background:var(--card); padding:12px 14px; border-radius:4px; max-width:none; }}
.chart {{ margin:20px 0 8px; border:1px solid var(--line); border-radius:6px; overflow:hidden; background:#0c0b08; }}
.chart svg {{ display:block; width:100%; height:auto; }}
.caption {{ color:var(--muted); font-size:13px; }}
dl.facts {{ display:grid; grid-template-columns:repeat(auto-fit,minmax(200px,1fr)); gap:10px; margin:16px 0; }}
dl.facts div {{ border:1px solid var(--line); border-radius:4px; padding:10px 12px; background:var(--card); }}
dt {{ color:var(--muted); font-size:12px; }}
dd {{ margin:2px 0 0; font-size:18px; font-variant-numeric:tabular-nums; }}
.scroll {{ overflow-x:auto; border:1px solid var(--line); border-radius:4px; }}
table {{ border-collapse:collapse; width:100%; font-size:13px; }}
th, td {{ text-align:left; padding:7px 10px; border-bottom:1px solid var(--line); vertical-align:top; }}
th {{ color:var(--muted); font-weight:600; white-space:nowrap; background:var(--card); }}
tr:last-child td {{ border-bottom:0; }}
.n {{ text-align:right; font-variant-numeric:tabular-nums; white-space:nowrap; }}
code {{ font:12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace; overflow-wrap:anywhere; }}
.tag {{ display:inline-block; border:1px solid var(--line); border-radius:3px; padding:0 6px; font-size:11px; letter-spacing:.03em; color:var(--muted); white-space:nowrap; }}
ul {{ padding-left:20px; max-width:75ch; }}
footer {{ margin-top:48px; color:var(--muted); font-size:12px; }}
</style>
</head>
<body>
<main>
<p class="kicker">Kshana Pro · design study</p>
<h1>{title}</h1>
<p class="label"><strong>{statement_cap}.</strong> {label}</p>
<div class="chart">{svg}</div>
<p class="caption">Every dot is one run of the open engine. The line is a staircase because no design exists between two neighbours on the front. Hover a mark for its values; the table below lists every front design.</p>
<dl class="facts">{facts_html}</dl>
<h2>The front</h2>
<p>Each row is a design no other evaluated design beats on every objective at once. Each has a scenario file the free Kshana engine runs to the same result document, identified by its SHA-256.{truncated}</p>
{front_table}
{reference_html}
<h2>What was searched</h2>
{var_table}
<h2>Objectives</h2>
<p>An objective is a number the open engine wrote in the run's result, or a design variable. There is no cost model.</p>
{obj_table}
{convergence_html}
<h2>What the labels mean</h2>
<p>Each figure takes the label of the weakest open capability it rests on and is never raised. VALIDATED: checked against an independent external reference. MODELLED: a first-principles or published-formula model, tested internally. PARTNER: a partner's discipline.</p>
{tier_table}
<h2>Not modelled</h2>
<ul>{not_modelled}</ul>
<h2>Reproducibility</h2>
{rep_table}
<footer>Generated by <code>kshana-pro design</code>. This page is one file and fetches nothing.</footer>
</main>
</body>
</html>
"#,
        title = esc(title),
        statement_cap = esc(&capitalise(doc["front_statement"].as_str().unwrap_or(""))),
        label = esc(doc["label"].as_str().unwrap_or("")),
        svg = svg,
        facts_html = facts_html,
        truncated = if front.len() > shown {
            format!(
                " The first {shown} of {} are shown; front/index.csv lists them all.",
                front.len()
            )
        } else {
            String::new()
        },
        front_table = front_table,
        reference_html = reference_html,
        var_table = table(
            &[
                "Variable".into(),
                "Scenario key".into(),
                "Levels".into(),
                "Unit".into()
            ],
            &var_rows,
            9
        ),
        obj_table = table(
            &[
                "Objective".into(),
                "Sense".into(),
                "Source".into(),
                "Unit".into()
            ],
            &obj_rows,
            9
        ),
        convergence_html = convergence_html,
        tier_table = table(
            &[
                "Figure".into(),
                "Label".into(),
                "Open capability it rests on".into()
            ],
            &all_tiers,
            9
        ),
        not_modelled = not_modelled,
        rep_table = table(&["".into(), "".into()], &rep_rows, 9),
    )
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_like_numbers() {
        assert_eq!(num(24.0), "24");
        assert_eq!(num(85.0404), "85.0404");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(-3.10), "-3.1");
    }

    #[test]
    fn ticks_cover_the_data_with_round_steps() {
        let o = serde_json::json!({"name": "n", "unit": "count", "sense": "min"});
        let a = axis(&o, &[8.0, 24.0]);
        assert!(a.lo <= 8.0 && a.hi >= 24.0);
        assert!(a.ticks.len() >= 3 && a.ticks.len() <= 12);
        let flat = axis(&o, &[5.0, 5.0]);
        assert!(flat.hi > flat.lo);
    }
}
