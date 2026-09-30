// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The study's figures and report, drawn from the result document only, so a figure
//! can never show a number the JSON does not hold.
//!
//! - `coexistence.matrix.svg`: candidate plans against the victim signals, each cell the
//!   worst-cell loss of carrier-to-noise density (C/N0) with its verdict, in both
//!   directions;
//! - `coexistence.map.svg`: the upper-bound loss per grid cell for one plan (the plan
//!   whose worst loss is largest), on a latitude-longitude grid;
//! - `coexistence.pareto.svg`: ranging accuracy against the worst loss caused, per plan,
//!   with the Pareto front marked;
//! - `coexistence.report.html`: one self-contained page holding all three.

use serde_json::Value;

/// Escape text for SVG and HTML.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// A loss in dB, printed with enough digits to be read at any size.
pub fn fmt_db(x: f64) -> String {
    if x == 0.0 {
        "0".to_string()
    } else if x.abs() >= 0.01 {
        format!("{x:.3}")
    } else {
        format!("{x:.2e}")
    }
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_default()
}

fn arr<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

const BG: &str = "#0f1115";
const INK: &str = "#e6e8ec";
const MUTED: &str = "#9aa3ae";
const RULE: &str = "#2a2f38";
const OK: &str = "#1f6f5c";
const BAD: &str = "#8f2d2d";
const NONE: &str = "#2b2f36";

fn cell_text(entry: &Value) -> (String, &'static str, String) {
    let status = entry["verdict"]["status"].as_str().unwrap_or_default();
    match entry["worst_loss_db"].as_f64() {
        Some(l) => (
            format!("{} dB", fmt_db(l)),
            if status == "compatible" { OK } else { BAD },
            if status == "compatible" {
                "within limit".to_string()
            } else {
                "over limit".to_string()
            },
        ),
        None => (
            match entry["reason"].as_str() {
                Some("no spectral overlap") => "no spectral overlap".to_string(),
                Some(r) if r.contains("in view") => "none in view".to_string(),
                _ => "no interference".to_string(),
            },
            NONE,
            "compatible".to_string(),
        ),
    }
}

/// The plan-by-victim matrix, both directions.
pub fn matrix_svg(r: &Value) -> String {
    let plans = arr(r, "plans");
    let victims = arr(r, "victims");
    let (ml, cw, ch, top) = (190.0_f64, 118.0_f64, 44.0_f64, 116.0_f64);
    // Column labels, the rows, then room for the next block's heading.
    let block_h = 24.0 + plans.len() as f64 * ch + 44.0;
    let w = ml + victims.len() as f64 * cw + 16.0;
    let h = top + 2.0 * block_h + 40.0;
    let limit = r["limit"]["max_degradation_db"]
        .as_f64()
        .unwrap_or(f64::NAN);
    let mut o = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"system-ui,-apple-system,Segoe UI,sans-serif\" font-size=\"11\">"
    );
    o += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{BG}\"/>");
    o += &format!(
        "<text x=\"16\" y=\"26\" fill=\"{INK}\" font-size=\"15\" font-weight=\"600\">Spectrum coexistence: worst-cell loss of carrier-to-noise density, per plan and signal</text>"
    );
    o += &format!(
        "<text x=\"16\" y=\"44\" fill=\"{MUTED}\">{} · C/N0: carrier-to-noise density; GNSS: global navigation satellite system</text>",
        esc(s(r, "title"))
    );
    o += &format!(
        "<text x=\"16\" y=\"62\" fill=\"{MUTED}\">Limit {} dB. Source of the limit: {}</text>",
        fmt_db(limit),
        esc(r["limit"]["limit_source"].as_str().unwrap_or_default())
    );
    o += &format!(
        "<text x=\"16\" y=\"80\" fill=\"{MUTED}\">MODELLED upper bound: every satellite in view at the maximum received power, no elevation-dependent power, no antenna pattern.</text>"
    );
    let blocks = [
        ("caused", "Interference the plan causes to each GNSS signal (upper bound)"),
        ("received", "Interference the GNSS constellations cause to the plan (most in view, minimum GNSS power)"),
    ];
    for (b, (key, heading)) in blocks.iter().enumerate() {
        let y0 = top + b as f64 * block_h;
        o += &format!(
            "<text x=\"16\" y=\"{:.0}\" fill=\"{INK}\" font-weight=\"600\">{}</text>",
            y0 - 8.0,
            esc(heading)
        );
        for (j, v) in victims.iter().enumerate() {
            o += &format!(
                "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{MUTED}\" text-anchor=\"middle\">{}</text>",
                ml + j as f64 * cw + cw / 2.0,
                y0 + 14.0,
                esc(v.as_str().unwrap_or_default())
            );
        }
        for (i, p) in plans.iter().enumerate() {
            let y = y0 + 24.0 + i as f64 * ch;
            o += &format!(
                "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{INK}\" text-anchor=\"end\">{}</text>",
                ml - 10.0,
                y + ch / 2.0 + 4.0,
                esc(s(p, "label"))
            );
            for (j, e) in arr(p, key).iter().enumerate() {
                let x = ml + j as f64 * cw;
                let (t, fill, sub) = cell_text(e);
                o += &format!(
                    "<rect x=\"{:.0}\" y=\"{y:.0}\" width=\"{:.0}\" height=\"{:.0}\" rx=\"3\" fill=\"{fill}\" stroke=\"{BG}\" stroke-width=\"2\"/>",
                    x + 1.0,
                    cw - 2.0,
                    ch - 2.0
                );
                o += &format!(
                    "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{INK}\" text-anchor=\"middle\" font-weight=\"600\">{}</text>",
                    x + cw / 2.0,
                    y + 18.0,
                    esc(&t)
                );
                o += &format!(
                    "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{INK}\" fill-opacity=\"0.75\" text-anchor=\"middle\" font-size=\"10\">{}</text>",
                    x + cw / 2.0,
                    y + 33.0,
                    esc(&sub)
                );
            }
        }
    }
    o += &format!(
        "<text x=\"16\" y=\"{:.0}\" fill=\"{MUTED}\" font-size=\"10\">Each cell: the worst grid cell's loss of carrier-to-noise density (C/N0) for that global navigation satellite system (GNSS) signal. Spectral separation</text>",
        h - 34.0
    );
    o += &format!(
        "<text x=\"16\" y=\"{:.0}\" fill=\"{MUTED}\" font-size=\"10\">coefficients read from the open leo-signal kind; satellites in view from constellation-design. Engine {} · scenario {}</text>",
        h - 20.0,
        esc(r["engine_version"].as_str().unwrap_or("?")),
        esc(&s(r, "scenario_hash").chars().take(12).collect::<String>())
    );
    o += "</svg>";
    o
}

/// A sequential colour for `t` in 0..=1 (dark blue to yellow).
fn ramp(t: f64) -> String {
    const STOPS: [(f64, [f64; 3]); 5] = [
        (0.0, [38.0, 50.0, 110.0]),
        (0.25, [33.0, 110.0, 142.0]),
        (0.5, [34.0, 160.0, 130.0]),
        (0.75, [140.0, 200.0, 70.0]),
        (1.0, [250.0, 225.0, 50.0]),
    ];
    let t = t.clamp(0.0, 1.0);
    let k = STOPS
        .windows(2)
        .position(|w| t <= w[1].0)
        .unwrap_or(STOPS.len() - 2);
    let (a, b) = (STOPS[k], STOPS[k + 1]);
    let f = if b.0 > a.0 {
        (t - a.0) / (b.0 - a.0)
    } else {
        0.0
    };
    let c: Vec<u8> = (0..3)
        .map(|i| (a.1[i] + f * (b.1[i] - a.1[i])).round() as u8)
        .collect();
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// The upper-bound loss per grid cell for the study's map plan.
pub fn map_svg(r: &Value) -> String {
    let map_plan = r["summary"]["map_plan"].as_str().unwrap_or_default();
    let plan = arr(r, "plans")
        .iter()
        .find(|p| s(p, "label") == map_plan)
        .cloned()
        .unwrap_or(Value::Null);
    let lat: Vec<f64> = arr(&r["grid"], "lat_deg")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let lon: Vec<f64> = arr(&r["grid"], "lon_deg")
        .iter()
        .filter_map(Value::as_f64)
        .collect();
    let step = r["grid"]["grid_step_deg"].as_f64().unwrap_or(10.0);
    let (ml, mt, pw, ph) = (46.0_f64, 70.0_f64, 720.0_f64, 360.0_f64);
    let w = ml + pw + 24.0;
    let h = mt + ph + 92.0;
    let values: Vec<f64> = arr(&plan, "map_loss_db")
        .iter()
        .flat_map(|row| row.as_array().into_iter().flatten())
        .filter_map(Value::as_f64)
        .filter(|x| *x > 0.0)
        .collect();
    let (lo, hi) = values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
            (a.min(x), b.max(x))
        });
    let t_of = |x: f64| {
        if values.is_empty() || hi <= lo {
            1.0
        } else {
            (x.log10() - lo.log10()) / (hi.log10() - lo.log10())
        }
    };
    let xpx = |lon: f64| ml + (lon + 180.0) / 360.0 * pw;
    let ypx = |lat: f64| mt + (90.0 - lat) / 180.0 * ph;
    let mut o = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"system-ui,-apple-system,Segoe UI,sans-serif\" font-size=\"11\">"
    );
    o += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{BG}\"/>");
    o += &format!(
        "<text x=\"16\" y=\"26\" fill=\"{INK}\" font-size=\"15\" font-weight=\"600\">Upper-bound loss of carrier-to-noise density caused by plan \u{201c}{}\u{201d}</text>",
        esc(map_plan)
    );
    o += &format!(
        "<text x=\"16\" y=\"44\" fill=\"{MUTED}\">C/N0: carrier-to-noise density. Per cell, the largest loss over the global navigation satellite system (GNSS) signals it overlaps.</text>"
    );
    o += &format!(
        "<text x=\"16\" y=\"60\" fill=\"{MUTED}\">MODELLED upper bound: most satellites in view, each at the maximum received power. {} deg cells, no coastlines drawn; grey: no interference.</text>",
        step
    );
    o += &format!("<rect x=\"{ml}\" y=\"{mt}\" width=\"{pw}\" height=\"{ph}\" fill=\"{NONE}\"/>");
    for (i, row) in arr(&plan, "map_loss_db").iter().enumerate() {
        for (j, v) in row.as_array().into_iter().flatten().enumerate() {
            let (Some(&la), Some(&lo_)) = (lat.get(i), lon.get(j)) else {
                continue;
            };
            let fill = match v.as_f64() {
                Some(x) if x > 0.0 => ramp(t_of(x)),
                _ => continue,
            };
            o += &format!(
                "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{fill}\"><title>{la} deg, {lo_} deg: {} dB</title></rect>",
                xpx(lo_ - step / 2.0),
                ypx(la + step / 2.0),
                step / 360.0 * pw,
                step / 180.0 * ph,
                fmt_db(v.as_f64().unwrap_or_default())
            );
        }
    }
    for k in (-180..=180).step_by(30) {
        let x = xpx(k as f64);
        o += &format!(
            "<line x1=\"{x:.1}\" y1=\"{mt}\" x2=\"{x:.1}\" y2=\"{:.0}\" stroke=\"{BG}\" stroke-opacity=\"0.55\"/>",
            mt + ph
        );
        o += &format!(
            "<text x=\"{x:.1}\" y=\"{:.0}\" fill=\"{MUTED}\" text-anchor=\"middle\" font-size=\"10\">{k}</text>",
            mt + ph + 14.0
        );
    }
    for k in (-90..=90).step_by(30) {
        let y = ypx(k as f64);
        o += &format!(
            "<line x1=\"{ml}\" y1=\"{y:.1}\" x2=\"{:.0}\" y2=\"{y:.1}\" stroke=\"{BG}\" stroke-opacity=\"0.55\"/>",
            ml + pw
        );
        o += &format!(
            "<text x=\"{:.0}\" y=\"{:.1}\" fill=\"{MUTED}\" text-anchor=\"end\" font-size=\"10\">{k}</text>",
            ml - 6.0,
            y + 3.0
        );
    }
    o += &format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{MUTED}\" text-anchor=\"middle\" font-size=\"10\">longitude (deg)</text>",
        ml + pw / 2.0,
        mt + ph + 28.0
    );
    // Legend.
    let ly = mt + ph + 44.0;
    let lw = 260.0;
    for k in 0..52 {
        let t = k as f64 / 51.0;
        o += &format!(
            "<rect x=\"{:.1}\" y=\"{ly:.0}\" width=\"{:.2}\" height=\"10\" fill=\"{}\"/>",
            ml + t * lw,
            lw / 51.0 + 0.4,
            ramp(t)
        );
    }
    let legend = if values.is_empty() {
        "no cell has interference".to_string()
    } else {
        format!(
            "{} dB to {} dB (log scale); limit {} dB",
            fmt_db(lo),
            fmt_db(hi),
            fmt_db(
                r["limit"]["max_degradation_db"]
                    .as_f64()
                    .unwrap_or(f64::NAN)
            )
        )
    };
    o += &format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{INK}\" font-size=\"11\">{}</text>",
        ml + lw + 12.0,
        ly + 9.0,
        esc(&legend)
    );
    o += &format!(
        "<text x=\"16\" y=\"{:.0}\" fill=\"{MUTED}\" font-size=\"10\">Engine {} · scenario {} · Kshana Pro spectrum coexistence</text>",
        h - 12.0,
        esc(r["engine_version"].as_str().unwrap_or("?")),
        esc(&s(r, "scenario_hash").chars().take(12).collect::<String>())
    );
    o += "</svg>";
    o
}

/// Ranging accuracy against the worst loss caused, per plan, with the front marked.
pub fn pareto_svg(r: &Value) -> String {
    let pts = arr(&r["pareto"], "points");
    let (ml, mt, pw, ph) = (72.0_f64, 70.0_f64, 560.0_f64, 300.0_f64);
    let w = ml + pw + 200.0;
    let h = mt + ph + 70.0;
    let xs: Vec<f64> = pts
        .iter()
        .filter_map(|p| p["ranging_accuracy_m"].as_f64())
        .filter(|x| *x > 0.0)
        .collect();
    let ys: Vec<f64> = pts
        .iter()
        .filter_map(|p| p["loss_for_ranking_db"].as_f64())
        .collect();
    let (xlo, xhi) = xs
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
            (a.min(x), b.max(x))
        });
    let (xlo, xhi) = if xs.is_empty() {
        (0.01, 1.0)
    } else {
        (
            10f64.powf(xlo.log10().floor()),
            10f64.powf((xhi.log10() + 1e-9).ceil()),
        )
    };
    let yhi = ys.iter().fold(0.0_f64, |a, &b| a.max(b));
    let yhi = if yhi > 0.0 { yhi * 1.15 } else { 1.0 };
    let xpx = |x: f64| ml + (x.log10() - xlo.log10()) / (xhi.log10() - xlo.log10()) * pw;
    let ypx = |y: f64| mt + ph - y / yhi * ph;
    let nmax = pts
        .iter()
        .filter_map(|p| p["total_satellites"].as_f64())
        .fold(1.0_f64, f64::max);
    let mut o = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"system-ui,-apple-system,Segoe UI,sans-serif\" font-size=\"11\">"
    );
    o += &format!("<rect width=\"100%\" height=\"100%\" fill=\"{BG}\"/>");
    o += &format!(
        "<text x=\"16\" y=\"26\" fill=\"{INK}\" font-size=\"15\" font-weight=\"600\">Ranging accuracy against the worst loss caused, per plan</text>"
    );
    o += &format!(
        "<text x=\"16\" y=\"44\" fill=\"{MUTED}\">Filled: on the Pareto front of (ranging accuracy, worst loss caused, satellite count), all minimised. Marker area: satellite count.</text>"
    );
    o += &format!(
        "<rect x=\"{ml}\" y=\"{mt}\" width=\"{pw}\" height=\"{ph}\" fill=\"none\" stroke=\"{RULE}\"/>"
    );
    let mut d = xlo;
    while d <= xhi * 1.0001 {
        let x = xpx(d);
        o += &format!(
            "<line x1=\"{x:.1}\" y1=\"{mt}\" x2=\"{x:.1}\" y2=\"{:.0}\" stroke=\"{RULE}\"/>",
            mt + ph
        );
        o += &format!(
            "<text x=\"{x:.1}\" y=\"{:.0}\" fill=\"{MUTED}\" text-anchor=\"middle\" font-size=\"10\">{d}</text>",
            mt + ph + 14.0
        );
        d *= 10.0;
    }
    for k in 0..=4 {
        let yv = yhi * k as f64 / 4.0;
        let y = ypx(yv);
        o += &format!(
            "<line x1=\"{ml}\" y1=\"{y:.1}\" x2=\"{:.0}\" y2=\"{y:.1}\" stroke=\"{RULE}\"/>",
            ml + pw
        );
        o += &format!(
            "<text x=\"{:.0}\" y=\"{:.1}\" fill=\"{MUTED}\" text-anchor=\"end\" font-size=\"10\">{}</text>",
            ml - 6.0,
            y + 3.0,
            fmt_db(yv)
        );
    }
    o += &format!(
        "<text x=\"{:.0}\" y=\"{:.0}\" fill=\"{MUTED}\" text-anchor=\"middle\">ranging accuracy (m, log scale; smaller is better)</text>",
        ml + pw / 2.0,
        mt + ph + 32.0
    );
    o += &format!(
        "<text x=\"18\" y=\"{:.0}\" fill=\"{MUTED}\" transform=\"rotate(-90 18 {:.0})\" text-anchor=\"middle\">worst loss caused (dB)</text>",
        mt + ph / 2.0,
        mt + ph / 2.0
    );
    let mut legend_y = mt + 8.0;
    for p in pts {
        let label = s(p, "plan");
        match (
            p["ranging_accuracy_m"].as_f64(),
            p["loss_for_ranking_db"].as_f64(),
        ) {
            (Some(x), Some(y)) if x > 0.0 => {
                let rad =
                    4.0 + 10.0 * (p["total_satellites"].as_f64().unwrap_or(1.0) / nmax).sqrt();
                let front = p["on_front"].as_bool().unwrap_or(false);
                let (fill, stroke) = if front {
                    ("#f2c94c", "#f2c94c")
                } else {
                    ("none", MUTED)
                };
                o += &format!(
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{rad:.1}\" fill=\"{fill}\" fill-opacity=\"0.85\" stroke=\"{stroke}\" stroke-width=\"1.5\"><title>{}: {x} m, {} dB, {} satellites</title></circle>",
                    xpx(x),
                    ypx(y),
                    esc(label),
                    fmt_db(y),
                    p["total_satellites"]
                );
                o += &format!(
                    "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"{INK}\" font-size=\"10\">{}</text>",
                    xpx(x) + rad + 3.0,
                    ypx(y) - rad - 2.0,
                    esc(label)
                );
            }
            _ => {
                o += &format!(
                    "<text x=\"{:.0}\" y=\"{legend_y:.0}\" fill=\"{MUTED}\" font-size=\"10\">{}: not ranked (no ranging accuracy)</text>",
                    ml + pw + 12.0,
                    esc(label)
                );
                legend_y += 14.0;
            }
        }
    }
    o += &format!(
        "<text x=\"16\" y=\"{:.0}\" fill=\"{MUTED}\" font-size=\"10\">A plan with no spectral overlap ranks at 0 dB (it causes no interference). Engine {} · scenario {}</text>",
        h - 12.0,
        esc(r["engine_version"].as_str().unwrap_or("?")),
        esc(&s(r, "scenario_hash").chars().take(12).collect::<String>())
    );
    o += "</svg>";
    o
}

fn opt_db(v: &Value) -> String {
    v.as_f64()
        .map(|x| format!("{} dB", fmt_db(x)))
        .unwrap_or_else(|| "null".into())
}

fn direction_table(p: &Value, key: &str, heading: &str) -> String {
    let mut t = format!(
        "<h4>{}</h4><table><thead><tr><th>GNSS signal</th><th>Spectral separation coefficient</th><th>One satellite</th><th>Worst cell</th><th>Where (lat, lon; in view)</th><th>Mean case, worst</th><th>Verdict</th></tr></thead><tbody>",
        esc(heading)
    );
    for e in arr(p, key) {
        let ssc = e["ssc_db_per_hz"]
            .as_f64()
            .map(|x| format!("{x:.2} dB/Hz"))
            .unwrap_or_else(|| "no spectral overlap".into());
        let whr = if e["worst_cell"].is_object() {
            format!(
                "{}, {}; {}",
                e["worst_cell"]["lat_deg"],
                e["worst_cell"]["lon_deg"],
                e["worst_cell"]["satellites_in_view"]
            )
        } else {
            esc(e["reason"].as_str().unwrap_or("-"))
        };
        let st = e["verdict"]["status"].as_str().unwrap_or_default();
        t += &format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td class=\"{}\">{}<br><small>{}</small></td></tr>",
            esc(s(e, "gnss")),
            ssc,
            opt_db(&e["single_satellite_loss_db"]),
            opt_db(&e["worst_loss_db"]),
            whr,
            opt_db(&e["worst_mean_case_loss_db"]),
            if st == "compatible" { "ok" } else { "bad" },
            esc(st),
            esc(e["verdict"]["basis"].as_str().unwrap_or_default())
        );
    }
    t + "</tbody></table>"
}

/// The self-contained report page.
pub fn report_html(r: &Value, matrix: &str, map: &str, pareto: &str) -> String {
    let mut plans_html = String::new();
    for p in arr(r, "plans") {
        let sig = &p["signal"];
        let con = &p["constellation"];
        plans_html += &format!(
            "<section class=\"plan\"><h3>{}</h3><p>Signal <b>{}</b> ({}), centre {} MHz, transmit bandwidth {} MHz, ranging accuracy {}. \
             Constellation of {} satellites, at most {} in view above a {} deg mask. Reference power {} dBW ({}); maximum power {} dBW ({}).</p>{}{}</section>",
            esc(s(p, "label")),
            esc(s(sig, "name")),
            esc(sig["source"].as_str().unwrap_or("-")),
            sig["centre_hz"].as_f64().map(|x| format!("{:.4}", x / 1e6)).unwrap_or_default(),
            sig["tx_bandwidth_hz"].as_f64().map(|x| format!("{:.3}", x / 1e6)).unwrap_or_default(),
            sig["ranging_accuracy_m"].as_f64().map(|x| format!("{x:.4} m")).unwrap_or_else(|| "none (Doppler-only)".into()),
            con["total_satellites"],
            con["most_in_view"],
            con["mask_deg"],
            sig["reference_power_dbw"].as_f64().map(|x| format!("{x:.3}")).unwrap_or_default(),
            esc(s(sig, "reference_power_basis")),
            sig["max_power_dbw"].as_f64().map(|x| format!("{x:.3}")).unwrap_or_default(),
            esc(s(sig, "max_power_basis")),
            direction_table(p, "caused", "Interference this plan causes (upper bound)"),
            direction_table(p, "received", "Interference the GNSS constellations cause to this plan (most in view, minimum GNSS power)"),
        );
    }
    let verdict_rows: String = arr(r, "plans")
        .iter()
        .map(|p| {
            let c = p["verdict_caused"]["status"].as_str().unwrap_or_default();
            let v = p["verdict_received"]["status"].as_str().unwrap_or_default();
            format!(
                "<tr><td>{}</td><td>{}</td><td class=\"{}\">{}</td><td class=\"{}\">{}</td></tr>",
                esc(s(p, "label")),
                opt_db(&p["worst_loss_caused_db"]),
                if c == "compatible" { "ok" } else { "bad" },
                esc(c),
                if v == "compatible" { "ok" } else { "bad" },
                esc(v)
            )
        })
        .collect();
    let members: String = arr(r, "members")
        .iter()
        .map(|m| {
            format!(
                "<tr><td>{}</td><td>{}</td><td><code>{}</code></td><td><code>{}</code></td></tr>",
                esc(s(m, "role")),
                esc(s(m, "kind")),
                esc(s(m, "scenario_sha256")),
                esc(s(m, "result_sha256"))
            )
        })
        .collect();
    let not_modelled: String = arr(r, "not_modelled")
        .iter()
        .map(|n| format!("<li>{}</li>", esc(n.as_str().unwrap_or_default())))
        .collect();
    let front: Vec<&str> = arr(&r["pareto"], "front")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Spectrum coexistence study</title>
<style>
:root {{ --bg:#ffffff; --ink:#15181d; --muted:#5b6470; --rule:#d9dde3; --ok:#1f6f5c; --bad:#a33a3a; --panel:#f5f6f8; }}
@media (prefers-color-scheme: dark) {{ :root {{ --bg:#0f1115; --ink:#e6e8ec; --muted:#9aa3ae; --rule:#2a2f38; --ok:#4fbf9f; --bad:#e07a7a; --panel:#161a20; }} }}
body {{ background:var(--bg); color:var(--ink); font:15px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif; margin:0; padding:24px 16px; }}
main {{ max-width:1040px; margin:0 auto; }}
h1 {{ font-size:24px; margin:0 0 4px; }} h2 {{ font-size:19px; margin:32px 0 8px; }} h3 {{ font-size:16px; margin:24px 0 6px; }} h4 {{ font-size:14px; margin:14px 0 6px; color:var(--muted); }}
.label {{ background:var(--panel); border-left:3px solid var(--muted); padding:10px 14px; font-size:13px; }}
.fig {{ overflow-x:auto; margin:12px 0; }} .fig svg {{ max-width:100%; height:auto; }}
table {{ border-collapse:collapse; width:100%; font-size:13px; margin:6px 0 12px; display:block; overflow-x:auto; }}
th, td {{ border-bottom:1px solid var(--rule); padding:6px 8px; text-align:left; vertical-align:top; }}
th {{ color:var(--muted); font-weight:600; }} td.ok {{ color:var(--ok); font-weight:600; }} td.bad {{ color:var(--bad); font-weight:600; }}
code {{ font-size:11px; word-break:break-all; }} small {{ color:var(--muted); font-weight:400; }}
</style></head><body><main>
<h1>{title}</h1>
<p class="label">{label}</p>
<p><b>Limit:</b> {limit} dB on the worst grid cell, in each direction. <b>Source of the limit:</b> {limit_source}.</p>
<p><b>What the bound assumes.</b> {bound}</p>
<h2>Verdicts</h2>
<table><thead><tr><th>Plan</th><th>Worst loss caused</th><th>Interference it causes</th><th>Interference it receives</th></tr></thead><tbody>{verdict_rows}</tbody></table>
<h2>Plans against GNSS signals</h2><div class="fig">{matrix}</div>
<h2>Where the loss is largest</h2><div class="fig">{map}</div>
<h2>Ranking the plans</h2><div class="fig">{pareto}</div>
<p>Pareto front (ranging accuracy, worst loss caused, satellite count, all minimised): {front}. {pareto_note}</p>
<h2>Per plan</h2>{plans_html}
<h2>Not modelled</h2><ul>{not_modelled}</ul>
<h2>Abbreviations</h2>
<table><tbody>
<tr><td>C/N0</td><td>carrier-to-noise density ratio, in decibel-hertz (dB-Hz)</td></tr>
<tr><td>GNSS</td><td>global navigation satellite system</td></tr>
<tr><td>GPS</td><td>Global Positioning System; L1 C/A is its L1 coarse/acquisition signal</td></tr>
<tr><td>AltBOC</td><td>alternative binary offset carrier, the Galileo E5 wideband signal</td></tr>
<tr><td>SSC</td><td>spectral separation coefficient, in decibels per hertz (dB/Hz)</td></tr>
<tr><td>INR</td><td>interference-to-noise ratio</td></tr>
<tr><td>dBW</td><td>decibels relative to one watt</td></tr>
<tr><td>ITU-R</td><td>International Telecommunication Union, Radiocommunication Sector</td></tr>
</tbody></table>
<h2>Reproducibility</h2>
<p>Scenario hash (SHA-256 of the canonical JSON form of the study) <code>{hash}</code>; open engine {engine}; overlay {overlay}. No random numbers are drawn: the study has no seed.</p>
<table><thead><tr><th>Open run</th><th>Kind</th><th>Scenario SHA-256</th><th>Result SHA-256</th></tr></thead><tbody>{members}</tbody></table>
</main></body></html>
"#,
        title = esc(s(r, "title")),
        label = esc(s(r, "label")),
        limit = fmt_db(
            r["limit"]["max_degradation_db"]
                .as_f64()
                .unwrap_or(f64::NAN)
        ),
        limit_source = esc(r["limit"]["limit_source"].as_str().unwrap_or_default()),
        bound = esc(s(r, "bound")),
        front = esc(&if front.is_empty() {
            "none".to_string()
        } else {
            front.join(", ")
        }),
        pareto_note = esc(s(&r["pareto"], "note")),
        hash = esc(s(r, "scenario_hash")),
        engine = esc(r["engine_version"].as_str().unwrap_or("?")),
        overlay = esc(s(r, "overlay_version")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_db_keeps_small_numbers_readable() {
        assert_eq!(fmt_db(0.0), "0");
        assert_eq!(fmt_db(0.25), "0.250");
        assert_eq!(fmt_db(0.00123), "1.23e-3");
    }

    #[test]
    fn ramp_ends() {
        assert_eq!(ramp(0.0), "#26326e");
        assert_eq!(ramp(1.0), "#fae132");
    }
}
