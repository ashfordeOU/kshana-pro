// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Vector charts from a run's own columns.
//!
//! [`plan`] turns a [`ChartData`] into positions in the unit square: axis ticks with
//! their labels, one polyline per series (broken where the result has a null) or one
//! bar per series and category. The plan is independent of the output format: the PDF
//! face draws it with [`draw`] from `printpdf` lines, rectangles and text, and the HTML
//! face draws the same plan as inline SVG, so both show the same picture.
//!
//! Nothing is smoothed, fitted or interpolated: a polyline joins the result's samples in
//! order, and a chart with more than [`super::figures::MAX_CHART_POINTS`] samples was
//! thinned by a constant stride when its data was read, which its caption states.

use printpdf::{Line, LinePoint, Mm, Op, Point, Pt};

use super::figures::ChartData;
use crate::report_brand as brand;
use crate::report_pdf::{rgb, sans, Layout};

/// Series colours, chosen to stay apart on white paper and in greyscale print.
pub const PALETTE: [&str; 6] = [
    brand::NAVY_INK,
    brand::BRASS_MUTED,
    "#9a3b2e",
    "#2f6f62",
    "#5b4b8a",
    "#3d6e9e",
];

/// The colour of series `i`.
pub fn colour(i: usize) -> &'static str {
    PALETTE[i % PALETTE.len()]
}

/// Round tick positions covering `[min, max]`: steps of 1, 2 or 5 times a power of ten,
/// about `target` of them. The first tick is at or below `min` and the last at or above
/// `max`. Returns the ticks and the step.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> (Vec<f64>, f64) {
    let (mut lo, mut hi) = (min.min(max), min.max(max));
    if !min.is_finite() || !max.is_finite() {
        return (vec![0.0, 1.0], 1.0);
    }
    if lo == hi {
        let pad = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
        lo -= pad;
        hi += pad;
    }
    let raw = (hi - lo) / target.max(1) as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let step = mag
        * if norm <= 1.0 {
            1.0
        } else if norm <= 2.0 {
            2.0
        } else if norm <= 5.0 {
            5.0
        } else {
            10.0
        };
    let first = (lo / step).floor();
    let last = (hi / step).ceil();
    let n = (last - first).round() as i64;
    let ticks = (0..=n.clamp(1, 200))
        .map(|i| (first + i as f64) * step)
        .collect();
    (ticks, step)
}

/// A tick value written with the decimals its step needs.
pub fn tick_label(v: f64, step: f64) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    if v != 0.0 && (v.abs() >= 1e6 || v.abs() < 1e-4) {
        return format!("{v:.1e}");
    }
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()).clamp(0.0, 6.0) as usize
    };
    format!("{v:.decimals$}")
}

/// One polyline of a plan, in the unit square (x right, y up).
#[derive(Debug, Clone, PartialEq)]
pub struct PlanLine {
    pub series: usize,
    /// Runs of consecutive samples; a null in the result ends a run.
    pub segments: Vec<Vec<(f64, f64)>>,
}

/// One bar of a plan, in the unit square.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanBar {
    pub series: usize,
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
}

/// A chart laid out in the unit square.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub bar: bool,
    pub x_ticks: Vec<(f64, String)>,
    pub y_ticks: Vec<(f64, String)>,
    pub lines: Vec<PlanLine>,
    pub bars: Vec<PlanBar>,
    /// One entry per series: its path and unit.
    pub legend: Vec<String>,
    /// The x path and unit.
    pub x_title: String,
    /// The unit the series share, or a note that they differ.
    pub y_title: String,
    /// Number of finite samples drawn.
    pub samples: usize,
}

fn unit_text(u: &Option<String>) -> String {
    match u.as_deref() {
        Some(u) if !u.is_empty() && u != "1" => format!(" [{u}]"),
        Some("1") => " [dimensionless]".to_string(),
        _ => String::new(),
    }
}

/// Lay a chart out in the unit square.
pub fn plan(c: &ChartData) -> Plan {
    let mut p = Plan {
        bar: c.style == "bar",
        x_title: format!("{}{}", c.x_path, unit_text(&c.x_unit)),
        ..Default::default()
    };
    p.legend = c
        .series
        .iter()
        .map(|s| format!("{}{}", s.path, unit_text(&s.unit)))
        .collect();
    let units: Vec<&Option<String>> = c.series.iter().map(|s| &s.unit).collect();
    p.y_title = if units.windows(2).all(|w| w[0] == w[1]) {
        units
            .first()
            .map(|u| unit_text(u).trim().to_string())
            .unwrap_or_default()
    } else {
        "[units differ, see the legend]".to_string()
    };

    let ys: Vec<f64> = c
        .series
        .iter()
        .flat_map(|s| s.values.iter().flatten().copied())
        .filter(|v| v.is_finite())
        .collect();
    if ys.is_empty() {
        return p;
    }
    let (mut y_min, mut y_max) = ys
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(*v), b.max(*v))
        });
    if p.bar {
        y_min = y_min.min(0.0);
        y_max = y_max.max(0.0);
    }
    let (yt, y_step) = nice_ticks(y_min, y_max, 5);
    let (y_lo, y_hi) = (yt[0], yt[yt.len() - 1]);
    let ny = |v: f64| (v - y_lo) / (y_hi - y_lo);
    p.y_ticks = yt
        .iter()
        .map(|t| (ny(*t), tick_label(*t, y_step)))
        .collect();

    if p.bar {
        let n = c.labels.len().max(1);
        let k = c.series.len().max(1);
        let slot = 1.0 / n as f64;
        let bar_w = slot * 0.72 / k as f64;
        for (i, label) in c.labels.iter().enumerate() {
            p.x_ticks.push(((i as f64 + 0.5) * slot, label.clone()));
            for (si, s) in c.series.iter().enumerate() {
                if let Some(Some(v)) = s.values.get(i) {
                    let x0 = i as f64 * slot + slot * 0.14 + si as f64 * bar_w;
                    let (a, b) = (ny(0.0), ny(*v));
                    p.bars.push(PlanBar {
                        series: si,
                        x0,
                        x1: x0 + bar_w,
                        y0: a.min(b),
                        y1: a.max(b),
                    });
                    p.samples += 1;
                }
            }
        }
        return p;
    }

    let xs: Vec<f64> = c.x.iter().flatten().copied().collect();
    if xs.is_empty() {
        return p;
    }
    let (x_lo, x_hi) = xs
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(*v), b.max(*v))
        });
    let span = if x_hi > x_lo { x_hi - x_lo } else { 1.0 };
    let nx = |v: f64| (v - x_lo) / span;
    let (xt, x_step) = nice_ticks(x_lo, x_hi, 6);
    p.x_ticks = xt
        .iter()
        .filter(|t| **t >= x_lo - span * 1e-9 && **t <= x_hi + span * 1e-9)
        .map(|t| (nx(*t), tick_label(*t, x_step)))
        .collect();
    for (si, s) in c.series.iter().enumerate() {
        let mut segments: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut cur: Vec<(f64, f64)> = Vec::new();
        for (x, y) in c.x.iter().zip(s.values.iter()) {
            match (x, y) {
                (Some(x), Some(y)) => {
                    cur.push((nx(*x), ny(*y)));
                    p.samples += 1;
                }
                _ => {
                    if !cur.is_empty() {
                        segments.push(std::mem::take(&mut cur));
                    }
                }
            }
        }
        if !cur.is_empty() {
            segments.push(cur);
        }
        p.lines.push(PlanLine {
            series: si,
            segments,
        });
    }
    p
}

// --- PDF drawing -------------------------------------------------------------------

/// Millimetres of legend per series.
const LEGEND_ROW_MM: f32 = 3.8;
/// Height of the plot area in a PDF chart block.
const PLOT_H_MM: f32 = 46.0;

/// The height a chart block needs in the PDF, legend included.
pub fn height_mm(c: &ChartData) -> f32 {
    6.0 + PLOT_H_MM + 9.0 + c.series.len() as f32 * LEGEND_ROW_MM + 3.0
}

fn text(l: &mut Layout, x_mm: f32, y_mm: f32, s: &str, pt: f32, col: &str) {
    let keep = l.y_mm;
    l.y_mm = y_mm;
    l.text_at(x_mm, s, sans(), pt, rgb(col));
    l.y_mm = keep;
}

fn polyline(l: &mut Layout, pts: &[(f32, f32)], col: &str, thickness_pt: f32) {
    if pts.len() < 2 {
        return;
    }
    l.ops.push(Op::SetOutlineColor { col: rgb(col) });
    l.ops.push(Op::SetOutlineThickness {
        pt: Pt(thickness_pt),
    });
    l.ops.push(Op::DrawLine {
        line: Line {
            points: pts
                .iter()
                .map(|(x, y)| LinePoint {
                    p: Point::new(Mm(*x), Mm(*y)),
                    bezier: false,
                })
                .collect(),
            is_closed: false,
        },
    });
}

/// Draw a chart block whose top-left corner is `(x_mm, top_mm)` and whose width is
/// `w_mm`. `ascii` maps text to what the built-in PDF fonts can show. The block is
/// [`height_mm`] tall.
pub(crate) fn draw(
    l: &mut Layout,
    x_mm: f32,
    top_mm: f32,
    w_mm: f32,
    c: &ChartData,
    ascii: &dyn Fn(&str) -> String,
) {
    let p = plan(c);
    let grid = "#d9d3c7";
    let axis = "#6b645a";
    let left = x_mm + 17.0;
    let right = x_mm + w_mm - 3.0;
    let plot_top = top_mm - 6.0;
    let plot_bottom = plot_top - PLOT_H_MM;
    let px = |u: f64| left + (right - left) * u as f32;
    let py = |u: f64| plot_bottom + (plot_top - plot_bottom) * u as f32;

    // The unit of the y axis, above its top end.
    text(l, x_mm, top_mm - 3.2, &ascii(&p.y_title), 7.0, axis);

    // Grid and y tick labels.
    for (u, label) in &p.y_ticks {
        polyline(l, &[(left, py(*u)), (right, py(*u))], grid, 0.3);
        text(l, x_mm, py(*u) - 0.9, &ascii(label), 6.5, axis);
    }
    // Frame: left and bottom axes.
    polyline(
        l,
        &[(left, plot_top), (left, plot_bottom), (right, plot_bottom)],
        axis,
        0.6,
    );
    // x ticks.
    let n_x = p.x_ticks.len().max(1);
    for (i, (u, label)) in p.x_ticks.iter().enumerate() {
        let x = px(*u);
        polyline(l, &[(x, plot_bottom), (x, plot_bottom - 1.2)], axis, 0.5);
        let shown = ascii(label);
        // A bar chart centres its category under the slot; keep long names short.
        let max_chars = ((right - left) / n_x as f32 / 1.25) as usize;
        let shown: String = if shown.chars().count() > max_chars.max(4) {
            shown.chars().take(max_chars.max(4)).collect()
        } else {
            shown
        };
        let half = shown.chars().count() as f32 * 0.62;
        let tx = if p.bar || i > 0 { x - half } else { x };
        text(l, tx.max(x_mm), plot_bottom - 4.2, &shown, 6.5, axis);
    }
    text(l, left, plot_bottom - 8.0, &ascii(&p.x_title), 7.0, axis);

    if p.samples == 0 {
        text(
            l,
            left + 4.0,
            (plot_top + plot_bottom) / 2.0,
            "no finite samples in the named columns",
            8.0,
            axis,
        );
    }
    for b in &p.bars {
        l.fill_rect(
            px(b.x0),
            py(b.y0),
            px(b.x1) - px(b.x0),
            (py(b.y1) - py(b.y0)).max(0.15),
            rgb(colour(b.series)),
        );
    }
    for line in &p.lines {
        for seg in &line.segments {
            let pts: Vec<(f32, f32)> = seg.iter().map(|(x, y)| (px(*x), py(*y))).collect();
            if pts.len() == 1 {
                // A lone sample between nulls: a short tick so it is visible.
                let (x, y) = pts[0];
                polyline(l, &[(x - 0.4, y), (x + 0.4, y)], colour(line.series), 0.9);
            } else {
                polyline(l, &pts, colour(line.series), 0.9);
            }
        }
    }
    // Legend.
    let mut y = plot_bottom - 12.0;
    for (i, entry) in p.legend.iter().enumerate() {
        l.fill_rect(left, y + 0.3, 5.0, 1.2, rgb(colour(i)));
        text(l, left + 7.0, y, &ascii(entry), 7.0, brand::INK);
        y -= LEGEND_ROW_MM;
    }
}

#[cfg(test)]
mod tests {
    use super::super::figures::ChartSeries;
    use super::*;

    #[test]
    fn ticks_are_round_and_cover_the_range() {
        let (t, step) = nice_ticks(3.2, 46.9, 5);
        assert_eq!(step, 10.0);
        assert_eq!(t, vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0]);
        let (t, step) = nice_ticks(0.0, 0.43, 5);
        assert_eq!(step, 0.1);
        assert!((t[t.len() - 1] - 0.5).abs() < 1e-12);
        assert_eq!(tick_label(0.30000000000000004, 0.1), "0.3");
        // A flat series still gets an axis.
        let (t, _) = nice_ticks(50.0, 50.0, 5);
        assert!(t[0] < 50.0 && t[t.len() - 1] > 50.0);
        let (t, _) = nice_ticks(f64::NAN, 1.0, 5);
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn a_null_breaks_the_polyline() {
        let c = ChartData {
            title: "t".into(),
            style: "line".into(),
            x_path: "t_s".into(),
            x_unit: Some("s".into()),
            x: vec![Some(0.0), Some(1.0), Some(2.0), Some(3.0)],
            labels: vec![],
            series: vec![ChartSeries {
                path: "v".into(),
                unit: Some("ns".into()),
                note: None,
                values: vec![Some(1.0), Some(2.0), None, Some(4.0)],
            }],
            points: 4,
            stride: 1,
        };
        let p = plan(&c);
        assert_eq!(p.lines[0].segments.len(), 2);
        assert_eq!(p.lines[0].segments[0].len(), 2);
        assert_eq!(p.samples, 3);
        assert_eq!(p.y_title, "[ns]");
        assert_eq!(p.legend, vec!["v [ns]"]);
        // Every point lies in the unit square.
        for seg in &p.lines[0].segments {
            for (x, y) in seg {
                assert!((0.0..=1.0).contains(x) && (0.0..=1.0).contains(y));
            }
        }
    }

    #[test]
    fn bars_start_at_zero() {
        let c = ChartData {
            title: "t".into(),
            style: "bar".into(),
            x_path: "bands[].name".into(),
            x_unit: None,
            x: vec![],
            labels: vec!["a".into(), "b".into()],
            series: vec![ChartSeries {
                path: "bands[].v".into(),
                unit: Some("dB-Hz".into()),
                note: None,
                values: vec![Some(3.2), Some(44.1)],
            }],
            points: 2,
            stride: 1,
        };
        let p = plan(&c);
        assert!(p.bar);
        assert_eq!(p.bars.len(), 2);
        assert_eq!(p.bars[0].y0, 0.0);
        assert!(p.bars[1].y1 > p.bars[0].y1);
        assert_eq!(p.x_ticks.len(), 2);
    }
}
