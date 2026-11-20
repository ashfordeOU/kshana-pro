// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Multi-sensor RNP scenarios along a route, with the e-conspicuity backup test.
//!
//! Each scenario says which sensors are left (GNSS on or off, facilities out, an inertial
//! reference with its drift, an optional declared backup position source). At each sample
//! along the route the best remaining sensor gives the navigation sensor error (95 %):
//!
//! 1. GNSS, at the stated `gnss_nse_95_nm`;
//! 2. DME/DME, the best Doc 9613 pair (2drms);
//! 3. VOR/DME, from the stated VOR bearing sigma and the DME error at that range (2drms);
//! 4. inertial coast from the last sample with an external sensor, `drift × time` at the
//!    stated ground speed (a planning rule, not an inertial model).
//!
//! `TSE = √(NSE² + FTE²)` with the user's FTE; the on-board monitoring alert stands in as
//! `NSE > RNP` (the estimate of position uncertainty exceeding the RNP value). The NACp that
//! the position supports is taken from the DO-260B EPU bounds, against the user's required
//! NACp: the e-conspicuity backup test. A declared backup source (for example a mobile-network
//! position for drones) enters as its own 95 % EPU when every other sensor is gone.
//!
//! Evidence: **MODELLED**.

use serde::{Deserialize, Serialize};

use super::{best_pair, dist_bearing, dme_sigma_m, dmes_in_view, vor_dme_in_view, Station, NM};

/// DO-260B NACp categories: (category, EPU bound in metres, strictly less than).
pub const NACP: &[(u8, f64)] = &[
    (11, 3.0),
    (10, 10.0),
    (9, 30.0),
    (8, 92.6),
    (7, 185.2),
    (6, 555.6),
    (5, 926.0),
    (4, 1852.0),
    (3, 3704.0),
    (2, 7408.0),
    (1, 18520.0),
];

/// NACp for a 95 % EPU (m): the highest category whose bound exceeds it; 0 = unknown.
pub fn nacp(epu_m: f64) -> u8 {
    NACP.iter()
        .find(|(_, b)| epu_m < *b)
        .map(|(c, _)| *c)
        .unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub name: String,
    /// Waypoints `[lat, lon]`.
    pub points: Vec<[f64; 2]>,
    pub altitude_ft: f64,
    pub ground_speed_kt: f64,
    #[serde(default = "d_sample")]
    pub sample_nm: f64,
    /// Navigation specification name, for the report.
    pub nav_spec: String,
    pub rnp_nm: f64,
    /// Flight technical error (95 %, NM), stated by the user.
    pub fte_95_nm: f64,
    /// GNSS sensor error (95 %, NM), stated by the user.
    pub gnss_nse_95_nm: f64,
    /// VOR bearing one-sigma (deg), needed for VOR/DME.
    #[serde(default)]
    pub vor_sigma_deg: Option<f64>,
    /// ADS-B NACp the e-conspicuity test requires.
    #[serde(default)]
    pub required_nacp: Option<u8>,
}

fn d_sample() -> f64 {
    2.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    pub name: String,
    pub epu_95_m: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    #[serde(default = "d_true")]
    pub gnss: bool,
    /// GNSS lost only between these along-track distances (NM); whole route when absent.
    #[serde(default)]
    pub gnss_out_nm: Option<[f64; 2]>,
    #[serde(default)]
    pub dme_dme: Option<bool>,
    #[serde(default)]
    pub vor_dme: Option<bool>,
    /// Facilities out in this scenario.
    #[serde(default)]
    pub out: Vec<String>,
    /// Inertial drift (NM/h); none when absent.
    #[serde(default)]
    pub irs_drift_nm_per_h: Option<f64>,
    #[serde(default)]
    pub backup: Option<Backup>,
}

fn d_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RnpRequest {
    #[serde(rename = "station")]
    pub stations: Vec<Station>,
    pub route: Route,
    #[serde(rename = "scenario")]
    pub scenarios: Vec<Scenario>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub along_nm: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// `gnss`, `dme_dme`, `vor_dme`, `irs`, `backup:<name>` or `none`.
    pub sensor: String,
    pub nse_95_nm: Option<f64>,
    pub tse_95_nm: Option<f64>,
    pub meets_rnp: bool,
    pub alert: bool,
    pub nacp: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub samples: Vec<Sample>,
    pub meets_rnp_fraction: f64,
    pub alert_fraction: f64,
    pub first_alert_nm: Option<f64>,
    pub longest_irs_coast_nm: f64,
    pub sensor_share: Vec<(String, f64)>,
    pub nacp_min: u8,
    pub meets_nacp_fraction: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RnpReport {
    pub format: String,
    pub route: String,
    pub nav_spec: String,
    pub rnp_nm: f64,
    pub scenarios: Vec<ScenarioResult>,
    pub evidence: String,
}

/// Sample the route every `step_nm` (great-circle legs, linear in lat/lon within a leg).
fn sample_route(r: &Route) -> Result<Vec<(f64, f64, f64)>, String> {
    if r.points.len() < 2 || r.sample_nm.is_nan() || r.sample_nm <= 0.0 {
        return Err("route: at least two points and sample_nm > 0".into());
    }
    let mut out = vec![];
    let mut along = 0.0;
    for w in r.points.windows(2) {
        let (d, _) = dist_bearing(w[0][0], w[0][1], w[1][0], w[1][1]);
        let leg = d / NM;
        let n = (leg / r.sample_nm).ceil().max(1.0) as usize;
        for k in 0..n {
            let f = k as f64 / n as f64;
            out.push((
                along + f * leg,
                w[0][0] + f * (w[1][0] - w[0][0]),
                w[0][1] + f * (w[1][1] - w[0][1]),
            ));
        }
        along += leg;
    }
    let l = r.points[r.points.len() - 1];
    out.push((along, l[0], l[1]));
    Ok(out)
}

pub fn run(req: &RnpRequest) -> Result<RnpReport, String> {
    let r = &req.route;
    if !(r.rnp_nm > 0.0 && r.fte_95_nm >= 0.0 && r.gnss_nse_95_nm > 0.0 && r.ground_speed_kt > 0.0)
    {
        return Err("route: rnp_nm, gnss_nse_95_nm and ground_speed_kt must be > 0".into());
    }
    let pts = sample_route(r)?;
    let mut results = vec![];
    for sc in &req.scenarios {
        for id in &sc.out {
            if !req.stations.iter().any(|s| &s.id == id) {
                return Err(format!("scenario {}: unknown facility {id}", sc.name));
            }
        }
        let st: Vec<Station> = req
            .stations
            .iter()
            .map(|s| {
                let mut s = s.clone();
                s.out |= sc.out.contains(&s.id);
                s
            })
            .collect();
        let mut samples = vec![];
        let mut last_ext: Option<(f64, f64)> = None; // (along, nse at that point)
        let mut longest = 0.0f64;
        for &(along, lat, lon) in &pts {
            let gnss_on = sc.gnss || sc.gnss_out_nm.is_some_and(|[a, b]| along < a || along > b);
            let mut pick: Option<(String, f64)> = None;
            if gnss_on {
                pick = Some(("gnss".into(), r.gnss_nse_95_nm));
            }
            if pick.is_none() && sc.dme_dme.unwrap_or(true) {
                let v = dmes_in_view(&st, lat, lon, r.altitude_ft, None);
                if let Some((e, _)) = best_pair(&st, &v) {
                    pick = Some(("dme_dme".into(), e / NM));
                }
            }
            if pick.is_none() && sc.vor_dme.unwrap_or(true) {
                if let Some(sig) = r.vor_sigma_deg {
                    let best = vor_dme_in_view(&st, lat, lon, r.altitude_ft)
                        .into_iter()
                        .map(|(_, d, _)| {
                            let cross = d * sig.to_radians();
                            let range = dme_sigma_m(d);
                            2.0 * (cross * cross + range * range).sqrt() / NM
                        })
                        .min_by(f64::total_cmp);
                    if let Some(e) = best {
                        pick = Some(("vor_dme".into(), e));
                    }
                }
            }
            if let Some((_, e)) = &pick {
                last_ext = Some((along, *e));
            } else if let (Some(drift), Some((a0, e0))) = (sc.irs_drift_nm_per_h, last_ext) {
                let hours = (along - a0) / r.ground_speed_kt;
                longest = longest.max(along - a0);
                pick = Some(("irs".into(), e0 + drift * hours));
            }
            if let Some(b) = &sc.backup {
                let be = b.epu_95_m / NM;
                if pick.as_ref().is_none_or(|p| p.0 == "irs" && p.1 > be) {
                    pick = Some((format!("backup:{}", b.name), be));
                }
            }
            let (sensor, nse) = match pick {
                Some((s, e)) => (s, Some(e)),
                None => ("none".into(), None),
            };
            let tse = nse.map(|e| (e * e + r.fte_95_nm * r.fte_95_nm).sqrt());
            samples.push(Sample {
                along_nm: along,
                lat_deg: lat,
                lon_deg: lon,
                sensor,
                nse_95_nm: nse,
                tse_95_nm: tse,
                meets_rnp: tse.is_some_and(|t| t <= r.rnp_nm),
                alert: nse.is_none_or(|e| e > r.rnp_nm),
                nacp: nse.map(|e| nacp(e * NM)).unwrap_or(0),
            });
        }
        let n = samples.len() as f64;
        let mut share: Vec<(String, f64)> = vec![];
        for s in &samples {
            match share.iter_mut().find(|x| x.0 == s.sensor) {
                Some(x) => x.1 += 1.0 / n,
                None => share.push((s.sensor.clone(), 1.0 / n)),
            }
        }
        results.push(ScenarioResult {
            name: sc.name.clone(),
            meets_rnp_fraction: samples.iter().filter(|s| s.meets_rnp).count() as f64 / n,
            alert_fraction: samples.iter().filter(|s| s.alert).count() as f64 / n,
            first_alert_nm: samples.iter().find(|s| s.alert).map(|s| s.along_nm),
            longest_irs_coast_nm: longest,
            sensor_share: share,
            nacp_min: samples.iter().map(|s| s.nacp).min().unwrap_or(0),
            meets_nacp_fraction: r
                .required_nacp
                .map(|q| samples.iter().filter(|s| s.nacp >= q).count() as f64 / n),
            samples,
        });
    }
    Ok(RnpReport {
        format: "kshana-pro-rnp-scenarios/1".into(),
        route: r.name.clone(),
        nav_spec: r.nav_spec.clone(),
        rnp_nm: r.rnp_nm,
        scenarios: results,
        evidence: "MODELLED: Doc 9613 DME/DME error model and geometry, stated GNSS/VOR/FTE/inertial figures, DO-260B NACp bounds; planning and assessment only".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nacp_bounds() {
        assert_eq!(nacp(2.0), 11);
        assert_eq!(nacp(50.0), 8);
        assert_eq!(nacp(92.6), 7);
        assert_eq!(nacp(1000.0), 4);
        assert_eq!(nacp(30_000.0), 0);
    }
}
