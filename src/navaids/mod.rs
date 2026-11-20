// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Terrestrial navaids: DME/VOR network twin, RNP scenarios and flight-inspection truth
//!
//! Research 2026-11-20 gaps, E37 B3703 (07 F15, 07b S19, 08 D15): when GNSS is denied, aircraft
//! fall back on DME/DME, VOR/DME and inertial coasting. This module answers, for a declared
//! network of facilities:
//!
//! * **Coverage twin** ([`coverage`]): at a flight level over an area, which facilities are
//!   usable (radio line of sight over a 4/3 earth, service volume, DME range and elevation
//!   limits), whether a DME/DME pair meets the geometry criteria, the DME/DME 2drms, and which
//!   single DME outage removes DME/DME coverage from the most of the area (critical DMEs).
//! * **RNP scenarios** ([`rnp`]): along a route, with GNSS on or off and facilities out, the
//!   navigation sensor error, the total system error against the RNP value, the on-board
//!   monitoring alert, the inertial coast where nothing else is left, and the ADS-B NACp that
//!   the backup position supports (the e-conspicuity backup test, also for a declared
//!   non-aviation backup such as a mobile-network position).
//! * **Flight-inspection truth assurance** ([`inspection`]): whether the inspection aircraft's
//!   position truth agrees with the facilities it is measuring, so a GNSS-corrupted truth is
//!   not mistaken for a facility error.
//!
//! Published values used (each checked in the tests):
//!
//! * radio horizon `d = 4.12 (√h₁ + √h₂)` km (h in metres; 4/3 earth), the same as the FAA AIM
//!   rule `1.23 √h` NM (h in feet);
//! * VOR standard service volumes T, L and H of FAA AIM 1-1-8 Table 1-1-1 (classic SSVs, as
//!   transcribed; check against the edition in force); any facility can instead state its own
//!   designated operational coverage;
//! * DME/DME criteria of ICAO Doc 9613 (PBN manual) for RNAV 1/2: DMEs from 3 to 160 NM, below
//!   40° elevation, pair included angle 30° to 150°; DME error `σ_air = max(0.085 NM, 0.125 % of
//!   the distance)` and `σ_sis = 0.05 NM`;
//! * the ADS-B NACp categories of RTCA DO-260B (EPU bounds 10 NM down to 3 m).
//!
//! Evidence: **MODELLED** (geometry and published error models; no terrain masking, no
//! multipath, no measured facility performance). Decision support and planning only, never
//! navigation: no position is computed for an aircraft to fly.

pub mod inspection;
pub mod rnp;

use serde::{Deserialize, Serialize};

pub const FEATURE: &str = "pro-navaids";

pub const NM: f64 = 1852.0;
pub const FT: f64 = 0.3048;
const R_EARTH_M: f64 = 6_371_008.8;

/// Facility kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Dme,
    Vor,
    VorDme,
}

impl Kind {
    pub fn has_dme(self) -> bool {
        matches!(self, Kind::Dme | Kind::VorDme)
    }
    pub fn has_vor(self) -> bool {
        matches!(self, Kind::Vor | Kind::VorDme)
    }
}

/// One facility.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Station {
    pub id: String,
    pub kind: Kind,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Antenna elevation (ft above mean sea level).
    #[serde(default)]
    pub elev_ft: f64,
    /// Standard service volume class `T`, `L` or `H` (FAA AIM 1-1-8).
    #[serde(default)]
    pub ssv: Option<String>,
    /// Designated operational coverage: radius (NM) and ceiling (ft); overrides the SSV.
    #[serde(default)]
    pub doc_nm: Option<f64>,
    #[serde(default)]
    pub doc_ft: Option<f64>,
    /// Out of service (NOTAM).
    #[serde(default)]
    pub out: bool,
}

/// Great-circle distance (m) and initial bearing (rad) on the mean sphere.
pub fn dist_bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> (f64, f64) {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let h = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    let d = 2.0 * R_EARTH_M * h.sqrt().min(1.0).asin();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    (d, y.atan2(x))
}

/// Radio horizon (m) between antennas at `h1_m` and `h2_m` above the surface, 4/3 earth.
pub fn radio_horizon_m(h1_m: f64, h2_m: f64) -> f64 {
    4120.0 * (h1_m.max(0.0).sqrt() + h2_m.max(0.0).sqrt())
}

/// Service-volume radius (NM) of a VOR class at an altitude (ft above the facility), FAA AIM
/// 1-1-8 Table 1-1-1 (classic T/L/H). `None` outside the volume.
pub fn ssv_radius_nm(class: &str, alt_ft: f64) -> Option<f64> {
    if alt_ft < 1000.0 {
        return None;
    }
    match class {
        "T" => (alt_ft <= 12_000.0).then_some(25.0),
        "L" => (alt_ft <= 18_000.0).then_some(40.0),
        "H" => {
            if alt_ft <= 14_500.0 {
                Some(40.0)
            } else if (18_000.0..=45_000.0).contains(&alt_ft) {
                Some(130.0)
            } else if alt_ft <= 60_000.0 {
                Some(100.0)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Usable coverage radius (NM) of a facility at an aircraft altitude (ft MSL): the stated DOC
/// or SSV, never beyond line of sight (antenna 30 ft above the site). `None`: not usable
/// at this altitude.
pub fn coverage_radius_nm(s: &Station, alt_ft: f64) -> Option<f64> {
    let above = alt_ft - s.elev_ft;
    let los = radio_horizon_m(30.0 * FT, above.max(0.0) * FT) / NM;
    let vol = match (s.doc_nm, s.doc_ft, s.ssv.as_deref()) {
        (Some(r), c, _) => c.is_none_or(|c| alt_ft <= c).then_some(r),
        (None, _, Some(cl)) => ssv_radius_nm(cl, above),
        // No stated volume: line of sight only (DME 160 NM limit applies separately).
        (None, _, None) => Some(f64::INFINITY),
    }?;
    Some(vol.min(los))
}

/// Doc 9613 DME error model: one-sigma (m) at a slant range (m).
pub fn dme_sigma_m(range_m: f64) -> f64 {
    let air = (0.085 * NM).max(0.00125 * range_m);
    let sis = 0.05 * NM;
    (air * air + sis * sis).sqrt()
}

/// A DME usable for DME/DME at a point: (bearing from the aircraft to the DME, rad; slant
/// range m).
#[derive(Clone, Copy, Debug)]
pub struct DmeView {
    pub idx: usize,
    pub bearing_rad: f64,
    pub slant_m: f64,
}

/// The DMEs usable for DME/DME at a point and altitude (Doc 9613 range and elevation limits,
/// coverage radius), skipping `excluded`.
pub fn dmes_in_view(
    st: &[Station],
    lat: f64,
    lon: f64,
    alt_ft: f64,
    excluded: Option<usize>,
) -> Vec<DmeView> {
    let mut v = vec![];
    for (i, s) in st.iter().enumerate() {
        if !s.kind.has_dme() || s.out || Some(i) == excluded {
            continue;
        }
        let (d, brg) = dist_bearing(lat, lon, s.lat_deg, s.lon_deg);
        let dh = (alt_ft - s.elev_ft) * FT;
        let slant = (d * d + dh * dh).sqrt();
        let elev = dh.atan2(d).to_degrees();
        let Some(r) = coverage_radius_nm(s, alt_ft) else {
            continue;
        };
        let d_nm = d / NM;
        if slant / NM < 3.0 || d_nm > 160.0 || d_nm > r || elev >= 40.0 {
            continue;
        }
        v.push(DmeView {
            idx: i,
            bearing_rad: brg,
            slant_m: slant,
        });
    }
    v
}

/// Included angle (deg, 0..180) between two bearings.
pub fn included_deg(a: f64, b: f64) -> f64 {
    let d = (a - b).to_degrees().rem_euclid(360.0);
    if d > 180.0 {
        360.0 - d
    } else {
        d
    }
}

/// 2drms (m) of a horizontal fix from range measurements at the given bearings and sigmas.
pub fn range_fix_2drms_m(views: &[(f64, f64)]) -> Option<f64> {
    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    for &(brg, s) in views {
        let (n, e) = (brg.cos(), brg.sin());
        let w = 1.0 / (s * s);
        a += w * n * n;
        b += w * n * e;
        c += w * e * e;
    }
    let det = a * c - b * b;
    if det <= 1e-12 * (a + c).powi(2) {
        return None;
    }
    Some(2.0 * ((a + c) / det).sqrt())
}

/// The best DME pair meeting the 30–150° criterion: (2drms m, pair ids).
pub fn best_pair(st: &[Station], views: &[DmeView]) -> Option<(f64, (String, String))> {
    let mut best: Option<(f64, (String, String))> = None;
    for i in 0..views.len() {
        for j in i + 1..views.len() {
            let ang = included_deg(views[i].bearing_rad, views[j].bearing_rad);
            if !(30.0..=150.0).contains(&ang) {
                continue;
            }
            let Some(e) = range_fix_2drms_m(&[
                (views[i].bearing_rad, dme_sigma_m(views[i].slant_m)),
                (views[j].bearing_rad, dme_sigma_m(views[j].slant_m)),
            ]) else {
                continue;
            };
            if best.as_ref().is_none_or(|b| e < b.0) {
                best = Some((
                    e,
                    (st[views[i].idx].id.clone(), st[views[j].idx].id.clone()),
                ));
            }
        }
    }
    best
}

/// VOR/DME availability at a point: a co-located VOR/DME in coverage.
pub fn vor_dme_in_view(st: &[Station], lat: f64, lon: f64, alt_ft: f64) -> Vec<(usize, f64, f64)> {
    st.iter()
        .enumerate()
        .filter(|(_, s)| s.kind == Kind::VorDme && !s.out)
        .filter_map(|(i, s)| {
            let (d, brg) = dist_bearing(lat, lon, s.lat_deg, s.lon_deg);
            let r = coverage_radius_nm(s, alt_ft)?;
            (d / NM <= r).then_some((i, d, brg))
        })
        .collect()
}

/// Coverage request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageRequest {
    pub name: String,
    #[serde(rename = "station")]
    pub stations: Vec<Station>,
    /// Altitude (ft MSL).
    pub altitude_ft: f64,
    /// `[lat_min, lat_max, lon_min, lon_max]`.
    pub area: [f64; 4],
    pub step_deg: f64,
    /// 2drms above which a DME/DME fix does not count (NM), e.g. the navigation
    /// specification's sensor allowance; stated by the user.
    #[serde(default)]
    pub max_2drms_nm: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoveragePoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub dmes: usize,
    pub vors: usize,
    pub dme_dme: bool,
    pub dme_dme_2drms_nm: Option<f64>,
    pub pair: Option<(String, String)>,
    pub vor_dme: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Critical {
    pub id: String,
    /// Fraction of the area that loses DME/DME when this DME is out.
    pub loss_fraction: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub format: String,
    pub name: String,
    pub altitude_ft: f64,
    pub points: usize,
    pub dme_dme_fraction: f64,
    pub vor_dme_fraction: f64,
    pub any_fraction: f64,
    pub median_2drms_nm: Option<f64>,
    pub critical: Vec<Critical>,
    pub grid: Vec<CoveragePoint>,
    pub evidence: String,
}

fn dme_dme_at(
    req: &CoverageRequest,
    lat: f64,
    lon: f64,
    excl: Option<usize>,
) -> Option<(f64, (String, String))> {
    let v = dmes_in_view(&req.stations, lat, lon, req.altitude_ft, excl);
    best_pair(&req.stations, &v).filter(|(e, _)| req.max_2drms_nm.is_none_or(|m| e / NM <= m))
}

/// Coverage twin over the area.
pub fn coverage(req: &CoverageRequest) -> Result<CoverageReport, String> {
    let [la0, la1, lo0, lo1] = req.area;
    if !(la1 > la0 && lo1 > lo0 && req.step_deg > 0.0) {
        return Err("area must be [lat_min, lat_max, lon_min, lon_max] with step_deg > 0".into());
    }
    let nla = ((la1 - la0) / req.step_deg).round() as usize + 1;
    let nlo = ((lo1 - lo0) / req.step_deg).round() as usize + 1;
    if nla * nlo > 250_000 {
        return Err("grid too large (over 250,000 points)".into());
    }
    let mut grid = vec![];
    for i in 0..nla {
        for j in 0..nlo {
            let (lat, lon) = (la0 + i as f64 * req.step_deg, lo0 + j as f64 * req.step_deg);
            let v = dmes_in_view(&req.stations, lat, lon, req.altitude_ft, None);
            let bp = dme_dme_at(req, lat, lon, None);
            let vors = req
                .stations
                .iter()
                .filter(|s| s.kind.has_vor() && !s.out)
                .filter(|s| {
                    coverage_radius_nm(s, req.altitude_ft)
                        .is_some_and(|r| dist_bearing(lat, lon, s.lat_deg, s.lon_deg).0 / NM <= r)
                })
                .count();
            grid.push(CoveragePoint {
                lat_deg: lat,
                lon_deg: lon,
                dmes: v.len(),
                vors,
                dme_dme: bp.is_some(),
                dme_dme_2drms_nm: bp.as_ref().map(|b| b.0 / NM),
                pair: bp.map(|b| b.1),
                vor_dme: !vor_dme_in_view(&req.stations, lat, lon, req.altitude_ft).is_empty(),
            });
        }
    }
    let n = grid.len() as f64;
    let covered: Vec<usize> = (0..grid.len()).filter(|&k| grid[k].dme_dme).collect();
    let mut critical = vec![];
    for (si, s) in req.stations.iter().enumerate() {
        if !s.kind.has_dme() || s.out {
            continue;
        }
        let lost = covered
            .iter()
            .filter(|&&k| dme_dme_at(req, grid[k].lat_deg, grid[k].lon_deg, Some(si)).is_none())
            .count();
        critical.push(Critical {
            id: s.id.clone(),
            loss_fraction: lost as f64 / n,
        });
    }
    critical.sort_by(|a, b| {
        b.loss_fraction
            .total_cmp(&a.loss_fraction)
            .then(a.id.cmp(&b.id))
    });
    let mut e: Vec<f64> = grid.iter().filter_map(|g| g.dme_dme_2drms_nm).collect();
    e.sort_by(f64::total_cmp);
    Ok(CoverageReport {
        format: "kshana-pro-navaid-coverage/1".into(),
        name: req.name.clone(),
        altitude_ft: req.altitude_ft,
        points: grid.len(),
        dme_dme_fraction: covered.len() as f64 / n,
        vor_dme_fraction: grid.iter().filter(|g| g.vor_dme).count() as f64 / n,
        any_fraction: grid.iter().filter(|g| g.dme_dme || g.vor_dme).count() as f64 / n,
        median_2drms_nm: (!e.is_empty()).then(|| e[e.len() / 2]),
        critical,
        grid,
        evidence: "MODELLED: 4/3-earth line of sight, AIM service volumes or stated DOC, ICAO Doc 9613 DME/DME criteria and error model; no terrain masking".into(),
    })
}

/// The grid as CSV.
pub fn coverage_csv(r: &CoverageReport) -> String {
    let mut s = String::from("lat_deg,lon_deg,dmes,vors,dme_dme,dme_dme_2drms_nm,vor_dme\n");
    for g in &r.grid {
        s += &format!(
            "{:.4},{:.4},{},{},{},{},{}\n",
            g.lat_deg,
            g.lon_deg,
            g.dmes,
            g.vors,
            g.dme_dme as u8,
            g.dme_dme_2drms_nm
                .map(|v| format!("{v:.3}"))
                .unwrap_or_default(),
            g.vor_dme as u8
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_horizon_matches_the_aim_rule() {
        // FAA AIM: d (NM) = 1.23 √h (ft). At 10,000 ft: 123 NM.
        let d = radio_horizon_m(0.0, 10_000.0 * FT) / NM;
        assert!((d - 123.0).abs() < 1.0, "{d}");
    }

    #[test]
    fn service_volumes() {
        assert_eq!(ssv_radius_nm("T", 5_000.0), Some(25.0));
        assert_eq!(ssv_radius_nm("T", 13_000.0), None);
        assert_eq!(ssv_radius_nm("L", 18_000.0), Some(40.0));
        assert_eq!(ssv_radius_nm("H", 10_000.0), Some(40.0));
        assert_eq!(ssv_radius_nm("H", 16_000.0), Some(100.0));
        assert_eq!(ssv_radius_nm("H", 30_000.0), Some(130.0));
        assert_eq!(ssv_radius_nm("H", 50_000.0), Some(100.0));
    }

    #[test]
    fn dme_sigma_and_pair_geometry() {
        // Short range: 0.085 NM floor; 100 NM: 0.125 % = 0.125 NM.
        assert!(
            (dme_sigma_m(10.0 * NM) / NM - (0.085f64.powi(2) + 0.05f64.powi(2)).sqrt()).abs()
                < 1e-9
        );
        assert!(
            (dme_sigma_m(100.0 * NM) / NM - (0.125f64.powi(2) + 0.05f64.powi(2)).sqrt()).abs()
                < 1e-9
        );
        assert!((included_deg(0.0, 350f64.to_radians()) - 10.0).abs() < 1e-9);
        // Perpendicular pair of equal sigma s: 2drms = 2·√2·s.
        let e = range_fix_2drms_m(&[(0.0, 100.0), (std::f64::consts::FRAC_PI_2, 100.0)]).unwrap();
        assert!((e - 2.0 * 2f64.sqrt() * 100.0).abs() < 1e-6);
        // Collinear: no fix.
        assert!(range_fix_2drms_m(&[(0.0, 100.0), (std::f64::consts::PI, 100.0)]).is_none());
    }
}
