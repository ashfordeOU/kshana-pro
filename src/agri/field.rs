// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The field planner: parallel swath lines over a field boundary.
//!
//! The boundary (a GeoJSON `Polygon`, the first ring, or `lat,lon` rows) is put in a local
//! plane and rotated so the working direction is "up". Centre lines are laid at
//! `w/2, 3w/2, ...` across the field from its first edge and clipped to the polygon (every
//! inside segment of a concave field becomes its own line). The plan reports the passes, the
//! path length, the field area and the covered area, and a working time at a speed with a
//! fixed time per turn. Headland passes are not planned.
//!
//! With an hourly advisory (from [`super::spaceweather::advise`]), the planner also finds the
//! earliest run of consecutive `go` hours long enough for the work (the interference- and
//! space-weather-aware part); `caution` hours count only when allowed.

use serde::Serialize;

use super::geo::{area, LocalFrame};
use super::lines::{GuidanceLine, LineKind, LineSet};

/// Read a boundary: GeoJSON Polygon / Feature / FeatureCollection (first polygon), or
/// `lat,lon` rows. Returns `[lat, lon]` points.
pub fn read_boundary(text: &str) -> Result<Vec<[f64; 2]>, String> {
    let t = text.trim_start();
    let pts: Vec<[f64; 2]> = if t.starts_with('{') {
        let v: serde_json::Value =
            serde_json::from_str(t).map_err(|e| format!("boundary GeoJSON: {e}"))?;
        fn find(v: &serde_json::Value) -> Option<&serde_json::Value> {
            match v.get("type")?.as_str()? {
                "Polygon" => v.get("coordinates")?.get(0),
                "MultiPolygon" => v.get("coordinates")?.get(0)?.get(0),
                "Feature" => find(v.get("geometry")?),
                "FeatureCollection" => v.get("features")?.as_array()?.iter().find_map(find),
                _ => None,
            }
        }
        find(&v)
            .and_then(|r| r.as_array())
            .ok_or("boundary GeoJSON: no polygon")?
            .iter()
            .map(|p| Some([p.get(1)?.as_f64()?, p.get(0)?.as_f64()?]))
            .collect::<Option<_>>()
            .ok_or("boundary GeoJSON: bad coordinate")?
    } else {
        t.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("lat"))
            .map(|l| super::parse_latlon(l).map(|(a, b)| [a, b]))
            .collect::<Result<_, _>>()?
    };
    let mut pts = pts;
    if pts.len() > 1 && pts[0] == pts[pts.len() - 1] {
        pts.pop();
    }
    if pts.len() < 3 {
        return Err("boundary needs at least three points".into());
    }
    Ok(pts)
}

/// Planner inputs.
#[derive(Clone, Debug)]
pub struct PlanParams {
    pub heading_deg: f64,
    pub width_m: f64,
    pub speed_kmh: f64,
    pub turn_s: f64,
    /// Shortest line kept (m).
    pub min_line_m: f64,
}

/// A working window found in an advisory.
#[derive(Clone, Debug, Serialize)]
pub struct Window {
    pub start_utc: String,
    pub hours: usize,
    pub caution_hours: usize,
}

/// The plan.
#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub label: &'static str,
    pub heading_deg: f64,
    pub width_m: f64,
    pub passes: usize,
    pub path_m: f64,
    pub field_ha: f64,
    pub covered_ha: f64,
    pub work_h: f64,
    pub window: Option<Window>,
    pub notes: Vec<String>,
    #[serde(skip)]
    pub lines: LineSet,
}

/// Plan swaths over `boundary`.
pub fn plan(boundary: &[[f64; 2]], p: &PlanParams) -> Result<Plan, String> {
    if p.width_m <= 0.0 || p.speed_kmh <= 0.0 {
        return Err("width and speed must be positive".into());
    }
    let f = LocalFrame::new(boundary[0][0], boundary[0][1], 0.0);
    let poly: Vec<[f64; 2]> = boundary.iter().map(|q| f.en(q[0], q[1])).collect();
    let a = p.heading_deg.to_radians();
    let u = [a.sin(), a.cos()]; // along
    let n = [u[1], -u[0]]; // across (to the right of the heading)
    let rot: Vec<[f64; 2]> = poly
        .iter()
        .map(|q| [q[0] * n[0] + q[1] * n[1], q[0] * u[0] + q[1] * u[1]])
        .collect();
    let xmin = rot.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min);
    let xmax = rot.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max);
    let mut lines = Vec::new();
    let mut path = 0.0;
    let mut k = 0usize;
    loop {
        let x = xmin + p.width_m * (k as f64 + 0.5);
        if x > xmax {
            break;
        }
        // Intersections of the vertical x with the polygon edges.
        let m = rot.len();
        let mut ys = Vec::new();
        for i in 0..m {
            let (a, b) = (rot[i], rot[(i + 1) % m]);
            if (a[0] <= x) != (b[0] <= x) {
                ys.push(a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0]));
            }
        }
        ys.sort_by(f64::total_cmp);
        for (j, seg) in ys.chunks(2).enumerate() {
            if seg.len() < 2 || seg[1] - seg[0] < p.min_line_m {
                continue;
            }
            // Alternate direction pass by pass (boustrophedon).
            let (y0, y1) = if k.is_multiple_of(2) {
                (seg[0], seg[1])
            } else {
                (seg[1], seg[0])
            };
            let to_ll = |y: f64| {
                let e = x * n[0] + y * u[0];
                let nn = x * n[1] + y * u[1];
                let (la, lo) = f.latlon(e, nn);
                [la, lo]
            };
            lines.push(GuidanceLine {
                name: if j == 0 {
                    format!("pass-{:03}", k + 1)
                } else {
                    format!("pass-{:03}-{}", k + 1, j + 1)
                },
                kind: LineKind::Ab,
                points: vec![to_ll(y0), to_ll(y1)],
                width_m: Some(p.width_m),
            });
            path += seg[1] - seg[0];
        }
        k += 1;
    }
    if lines.is_empty() {
        return Err("no swath fits the field at this width".into());
    }
    let turns = lines.len().saturating_sub(1) as f64;
    let work_h = path / (p.speed_kmh * 1000.0) + turns * p.turn_s / 3600.0;
    let field = area(&poly);
    Ok(Plan {
        label: super::LABEL,
        heading_deg: p.heading_deg,
        width_m: p.width_m,
        passes: lines.len(),
        path_m: super::round(path, 1),
        field_ha: super::round(field / 10_000.0, 4),
        covered_ha: super::round(path * p.width_m / 10_000.0, 4),
        work_h: super::round(work_h, 2),
        window: None,
        notes: vec!["Headland passes are not planned; covered area counts the swaths only.".into()],
        lines: LineSet::new(lines),
    })
}

/// The earliest run of consecutive hours (status `go`, and `caution` when allowed) of at
/// least `need_h`, from an advisory JSON (`hours[].utc`, `hours[].status`).
pub fn window(
    advisory_json: &str,
    need_h: f64,
    allow_caution: bool,
) -> Result<Option<Window>, String> {
    let v: serde_json::Value =
        serde_json::from_str(advisory_json).map_err(|e| format!("advisory: {e}"))?;
    let hours = v
        .get("hours")
        .and_then(|h| h.as_array())
        .ok_or("advisory: no hours")?;
    let need = need_h.ceil().max(1.0) as usize;
    let mut start: Option<usize> = None;
    let mut caution = 0;
    for (i, h) in hours.iter().enumerate() {
        let st = h.get("status").and_then(|s| s.as_str()).unwrap_or("no-go");
        let ok = st == "go" || (allow_caution && st == "caution");
        if ok {
            let s0 = *start.get_or_insert(i);
            if st == "caution" {
                caution += 1;
            }
            if i + 1 - s0 >= need {
                return Ok(Some(Window {
                    start_utc: hours[s0]
                        .get("utc")
                        .and_then(|u| u.as_str())
                        .unwrap_or("")
                        .into(),
                    hours: need,
                    caution_hours: caution,
                }));
            }
        } else {
            start = None;
            caution = 0;
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_plan() {
        // A field about 400 m north-south by 120 m east-west.
        let f = LocalFrame::new(49.9, -97.1, 0.0);
        let c = |e: f64, n: f64| {
            let (a, b) = f.latlon(e, n);
            [a, b]
        };
        let b = vec![c(0.0, 0.0), c(120.0, 0.0), c(120.0, 400.0), c(0.0, 400.0)];
        let p = plan(
            &b,
            &PlanParams {
                heading_deg: 0.0,
                width_m: 12.0,
                speed_kmh: 10.0,
                turn_s: 20.0,
                min_line_m: 5.0,
            },
        )
        .unwrap();
        assert_eq!(p.passes, 10);
        assert!((p.path_m - 4000.0).abs() < 1.0, "{}", p.path_m);
        assert!((p.field_ha - 4.8).abs() < 0.01);
        assert!((p.covered_ha - 4.8).abs() < 0.01);
        // 4 km at 10 km/h plus 9 turns of 20 s.
        assert!((p.work_h - (0.4 + 0.05)).abs() < 0.01);
        assert!((p.lines.lines[0].heading_deg() - 0.0).abs() < 0.01);
        assert!((p.lines.lines[1].heading_deg() - 180.0).abs() < 0.01);
    }

    #[test]
    fn window_search() {
        let j = r#"{"hours":[{"utc":"a","status":"go"},{"utc":"b","status":"no-go"},{"utc":"c","status":"go"},{"utc":"d","status":"caution"},{"utc":"e","status":"go"}]}"#;
        assert!(window(j, 2.0, false).unwrap().is_none());
        let w = window(j, 2.5, true).unwrap().unwrap();
        assert_eq!(
            (w.start_utc.as_str(), w.hours, w.caution_hours),
            ("c", 3, 1)
        );
    }
}
