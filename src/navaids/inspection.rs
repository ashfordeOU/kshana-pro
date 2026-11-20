// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Flight-inspection truth assurance.
//!
//! A flight-inspection aircraft compares what a facility radiates (DME range, VOR radial)
//! with the range and radial predicted from its own position truth, today usually GNSS based.
//! When the truth itself is disturbed (interference near the facility), a correct facility can
//! look out of tolerance. This check uses the redundancy of several facilities measured at the
//! same epoch:
//!
//! * every residual `measured − predicted` within `K` sigmas: **consistent**;
//! * one facility out and the rest consistent without it: **facility deviation** (the normal
//!   inspection finding);
//! * several out, and one common horizontal offset of the truth explains them all: **truth
//!   suspect**, with the size of that offset (never a corrected track);
//! * otherwise **unresolved** (too few facilities, or more than one cause).
//!
//! The residual model: a truth wrong by `δ` (north, east) changes a predicted DME range by
//! `−u·δ` (u the unit vector to the facility) and a predicted VOR radial by `(t·δ)/d` (t the
//! direction of increasing radial, d the distance). Sigmas: the stated measurement sigma and
//! the truth's stated sigma projected on each direction. Radials are true bearings (convert
//! magnetic radials before input).
//!
//! Evidence: **MODELLED**; assurance of the inspection data, not navigation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{dist_bearing, Station, FT, NM};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionRequest {
    #[serde(rename = "station")]
    pub stations: Vec<Station>,
    /// Track CSV `t_s,lat_deg,lon_deg,alt_ft,truth_sigma_m` (relative to the request).
    pub track: String,
    /// Measurements CSV `t_s,station,kind,value` (kind `dme` in NM slant, `vor` in degrees true).
    pub measurements: String,
    pub dme_sigma_m: f64,
    #[serde(default)]
    pub vor_sigma_deg: Option<f64>,
    #[serde(default = "d_k")]
    pub k_sigma: f64,
}

fn d_k() -> f64 {
    4.0
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackPoint {
    pub t_s: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub alt_ft: f64,
    pub truth_sigma_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Meas {
    pub t_s: f64,
    pub station: String,
    pub vor: bool,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Residual {
    pub station: String,
    pub kind: String,
    /// Metres for DME, degrees for VOR.
    pub residual: f64,
    pub z: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpochVerdict {
    pub t_s: f64,
    /// `consistent`, `facility_deviation`, `truth_suspect`, `unresolved`.
    pub verdict: String,
    pub station: Option<String>,
    pub truth_offset_m: Option<f64>,
    pub residuals: Vec<Residual>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InspectionReport {
    pub format: String,
    pub epochs: usize,
    pub consistent: usize,
    pub facility_deviation: BTreeMap<String, usize>,
    pub truth_suspect: usize,
    /// Truth-suspect episodes `[start, end]` (s).
    pub truth_suspect_episodes: Vec<[f64; 2]>,
    pub max_truth_offset_m: Option<f64>,
    pub unresolved: usize,
    pub verdicts: Vec<EpochVerdict>,
    pub evidence: String,
}

fn rows(text: &str, n: usize, what: &str) -> Result<Vec<Vec<String>>, String> {
    let mut out = vec![];
    for (i, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("t_s") {
            continue;
        }
        let f: Vec<String> = l.split(',').map(|s| s.trim().to_string()).collect();
        if f.len() < n {
            return Err(format!("{what} line {}: need {n} columns", i + 1));
        }
        out.push(f);
    }
    Ok(out)
}

fn num(s: &str, what: &str) -> Result<f64, String> {
    s.parse()
        .map_err(|_| format!("{what}: {s:?} is not a number"))
}

pub fn parse_track(text: &str) -> Result<Vec<TrackPoint>, String> {
    rows(text, 5, "track")?
        .into_iter()
        .map(|f| {
            Ok(TrackPoint {
                t_s: num(&f[0], "track t_s")?,
                lat_deg: num(&f[1], "track lat")?,
                lon_deg: num(&f[2], "track lon")?,
                alt_ft: num(&f[3], "track alt")?,
                truth_sigma_m: num(&f[4], "track truth_sigma_m")?,
            })
        })
        .collect()
}

pub fn parse_measurements(text: &str) -> Result<Vec<Meas>, String> {
    rows(text, 4, "measurements")?
        .into_iter()
        .map(|f| {
            let vor = match f[2].as_str() {
                "dme" => false,
                "vor" => true,
                k => return Err(format!("measurements: kind {k:?} (dme or vor)")),
            };
            Ok(Meas {
                t_s: num(&f[0], "measurements t_s")?,
                station: f[1].clone(),
                vor,
                value: num(&f[3], "measurements value")?,
            })
        })
        .collect()
}

/// One usable measurement: residual, sigma, and the residual's sensitivity to a truth offset
/// (north, east) in the same unit.
struct Row {
    station: String,
    vor: bool,
    r: f64,
    s: f64,
    g: [f64; 2],
}

fn solve_offset(rs: &[&Row]) -> Option<([f64; 2], f64)> {
    let (mut a, mut b, mut c, mut y0, mut y1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for r in rs {
        let w = 1.0 / (r.s * r.s);
        a += w * r.g[0] * r.g[0];
        b += w * r.g[0] * r.g[1];
        c += w * r.g[1] * r.g[1];
        y0 += w * r.g[0] * r.r;
        y1 += w * r.g[1] * r.r;
    }
    let det = a * c - b * b;
    if det <= 1e-12 * (a + c).powi(2) {
        return None;
    }
    let d = [(c * y0 - b * y1) / det, (a * y1 - b * y0) / det];
    let worst = rs
        .iter()
        .map(|r| ((r.r - r.g[0] * d[0] - r.g[1] * d[1]) / r.s).abs())
        .fold(0.0, f64::max);
    Some((d, worst))
}

pub fn assess(
    req: &InspectionRequest,
    track: &[TrackPoint],
    meas: &[Meas],
) -> Result<InspectionReport, String> {
    if !(req.dme_sigma_m > 0.0 && req.k_sigma > 0.0) {
        return Err("dme_sigma_m and k_sigma must be > 0".into());
    }
    let by_id: BTreeMap<&str, &Station> = req.stations.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut by_t: BTreeMap<i64, Vec<&Meas>> = BTreeMap::new();
    for m in meas {
        if !by_id.contains_key(m.station.as_str()) {
            return Err(format!(
                "measurement at {} s: unknown station {}",
                m.t_s, m.station
            ));
        }
        if m.vor && req.vor_sigma_deg.is_none() {
            return Err("VOR measurements need vor_sigma_deg".into());
        }
        by_t.entry((m.t_s * 1000.0).round() as i64)
            .or_default()
            .push(m);
    }
    let k = req.k_sigma;
    let mut verdicts = vec![];
    for tp in track {
        let Some(ms) = by_t.get(&((tp.t_s * 1000.0).round() as i64)) else {
            continue;
        };
        let mut rs = vec![];
        for m in ms {
            let s = by_id[m.station.as_str()];
            // Aircraft → station bearing and distance.
            let (d, brg) = dist_bearing(tp.lat_deg, tp.lon_deg, s.lat_deg, s.lon_deg);
            if m.vor {
                // Radial: bearing station → aircraft.
                let (_, radial) = dist_bearing(s.lat_deg, s.lon_deg, tp.lat_deg, tp.lon_deg);
                let pred = radial.to_degrees().rem_euclid(360.0);
                let mut r = (m.value - pred).rem_euclid(360.0);
                if r > 180.0 {
                    r -= 360.0;
                }
                let sv = req.vor_sigma_deg.unwrap();
                let st = (tp.truth_sigma_m / d.max(1.0)).to_degrees();
                // d(radial)/dδ = (−sin θ, cos θ)/d (rad/m); residual = meas − pred.
                let g = [
                    radial.sin() / d.max(1.0) * 180.0 / std::f64::consts::PI,
                    -radial.cos() / d.max(1.0) * 180.0 / std::f64::consts::PI,
                ];
                rs.push(Row {
                    station: m.station.clone(),
                    vor: true,
                    r,
                    s: (sv * sv + st * st).sqrt(),
                    g,
                });
            } else {
                let dh = (tp.alt_ft - s.elev_ft) * FT;
                let pred = (d * d + dh * dh).sqrt();
                let r = m.value * NM - pred;
                let hfrac = d / pred.max(1.0);
                rs.push(Row {
                    station: m.station.clone(),
                    vor: false,
                    r,
                    s: (req.dme_sigma_m.powi(2) + (tp.truth_sigma_m * hfrac).powi(2)).sqrt(),
                    // Truth wrong by δ: predicted range changes by −u·δ, residual by +u·δ.
                    g: [brg.cos() * hfrac, brg.sin() * hfrac],
                });
            }
        }
        let out: Vec<usize> = (0..rs.len())
            .filter(|&i| (rs[i].r / rs[i].s).abs() > k)
            .collect();
        let (verdict, station, offset) = if out.is_empty() {
            ("consistent", None, None)
        } else {
            let single = (out.len() == 1 && rs.len() >= 2).then(|| out[0]);
            // A single facility out with the rest consistent and the common-offset model
            // not explaining all rows better.
            let all: Vec<&Row> = rs.iter().collect();
            let common = (rs.len() >= 3)
                .then(|| solve_offset(&all))
                .flatten()
                .filter(|(_, worst)| *worst <= k);
            match (single, common) {
                (_, Some((d, _))) if out.len() >= 2 => {
                    ("truth_suspect", None, Some(d[0].hypot(d[1])))
                }
                (Some(i), _) => ("facility_deviation", Some(rs[i].station.clone()), None),
                _ => ("unresolved", None, None),
            }
        };
        verdicts.push(EpochVerdict {
            t_s: tp.t_s,
            verdict: verdict.into(),
            station,
            truth_offset_m: offset,
            residuals: rs
                .iter()
                .map(|r| Residual {
                    station: r.station.clone(),
                    kind: if r.vor { "vor" } else { "dme" }.into(),
                    residual: r.r,
                    z: r.r / r.s,
                })
                .collect(),
        });
    }
    let mut facility = BTreeMap::new();
    for v in &verdicts {
        if let Some(s) = &v.station {
            *facility.entry(s.clone()).or_insert(0) += 1;
        }
    }
    let mut episodes: Vec<[f64; 2]> = vec![];
    let mut prev_suspect = false;
    for v in &verdicts {
        let s = v.verdict == "truth_suspect";
        if s && prev_suspect {
            episodes.last_mut().unwrap()[1] = v.t_s;
        } else if s {
            episodes.push([v.t_s, v.t_s]);
        }
        prev_suspect = s;
    }
    let c = |w: &str| verdicts.iter().filter(|v| v.verdict == w).count();
    Ok(InspectionReport {
        format: "kshana-pro-inspection-truth/1".into(),
        epochs: verdicts.len(),
        consistent: c("consistent"),
        truth_suspect: c("truth_suspect"),
        unresolved: c("unresolved"),
        facility_deviation: facility,
        truth_suspect_episodes: episodes,
        max_truth_offset_m: verdicts
            .iter()
            .filter_map(|v| v.truth_offset_m)
            .max_by(f64::total_cmp),
        verdicts,
        evidence: "MODELLED: residual consistency across facilities on stated sigmas; the truth offset size is reported, never a corrected track".into(),
    })
}
