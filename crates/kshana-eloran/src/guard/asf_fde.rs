// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! LDC/ASF fault detection and exclusion on eLoran TOA residuals (research 2026-11-20 gaps,
//! B3702; the user-level integrity work NAVISP EL1-153 asks for).
//!
//! CONFIDENTIAL patent candidate: compiled only with the `unfiled-methods` feature.
//!
//! A user corrects each station's pseudorange with the published ASF map value at its
//! position plus, when the LORAN Data Channel carries one, a differential (temporal)
//! correction from a reference site (LDC message type 5, [`crate::ldc::msg`]). A fault can sit
//! in the **signal** (a station timing error, a relay) or in the **correction** (a corrupted
//! or stale differential value). Both put the same bias on one residual, so ordinary residual
//! FDE can only drop the station. This module separates them:
//!
//! 1. **Screen** each correction: an LDC warning marks the station unhealthy; a correction
//!    older than `max_age_s` is dropped (map only); a correction larger than
//!    `K·σ_temporal` (the temporal ASF variation the map leaves) is flagged implausible.
//! 2. **Detect**: weighted least squares (north, east, clock) on all corrected stations;
//!    `q = rᵀWr` against `χ²(1 − p_fa, n − 3)`.
//! 3. **Separate**: for each station `j`, solve without it and predict its residual twice,
//!    with its correction and with the map alone (variance includes the prediction's own
//!    uncertainty). A **correction fault** is the case where the corrected residual is out
//!    and the map-only residual is in: the station is kept, on the map alone. A **station
//!    fault** is out both ways: the station is excluded. The hypothesis whose re-solve passes
//!    the detection threshold with the smallest normalised statistic is taken; when none
//!    does, the epoch is **unresolved** and alarms.
//! 4. **Protect**: the horizontal protection level of the final geometry, Brown's slope form:
//!    `HPL = max_j slope_j · √T + K_md · σ_major`, with `T` the detection threshold and
//!    `σ_major` the major semi-axis of the horizontal covariance. An epoch alarms when it is
//!    unresolved or `HPL > HAL`.
//!
//! Evidence: **MODELLED** (synthetic epochs in the tests below). The thresholds follow from
//! `p_fa` and `p_md` given by the user; no integrity requirement is built in.

use super::{chi2_quantile, normal_quantile};
use crate::geo::{inverse, Geo};
use serde::{Deserialize, Serialize};

const C_M_PER_NS: f64 = 0.299_792_458;
const R_EARTH: f64 = 6_371_000.0;

/// One station observed at one epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StationObs {
    pub id: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Arrival minus nominal emission (ns), including the receiver clock.
    pub pseudo_delay_ns: f64,
    /// One-sigma measurement noise (ns).
    pub sigma_ns: f64,
    /// Published ASF map value at the user position (ns).
    #[serde(default)]
    pub asf_map_ns: f64,
    /// One-sigma temporal ASF variation the map does not capture (ns).
    pub asf_temporal_sigma_ns: f64,
    /// Differential correction received over the LDC (ns), if any.
    #[serde(default)]
    pub ldc_correction_ns: Option<f64>,
    #[serde(default)]
    pub ldc_age_s: Option<f64>,
    /// One-sigma noise of the differential correction (ns).
    #[serde(default)]
    pub ldc_sigma_ns: f64,
    /// The LDC carried an unhealthy-station warning.
    #[serde(default)]
    pub ldc_unhealthy: bool,
}

/// One epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FdeEpoch {
    pub t_s: f64,
    /// Approximate user position to start the solution.
    pub approx_lat_deg: f64,
    pub approx_lon_deg: f64,
    pub stations: Vec<StationObs>,
    /// Truth, for test records only (horizontal error and bounding checks).
    #[serde(default)]
    pub truth_lat_deg: Option<f64>,
    #[serde(default)]
    pub truth_lon_deg: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FdeConfig {
    /// False-alert probability per epoch.
    pub p_fa: f64,
    /// Missed-detection probability.
    pub p_md: f64,
    /// Horizontal alert limit (m).
    pub hal_m: f64,
    /// Oldest differential correction used (s).
    #[serde(default = "d_age")]
    pub max_age_s: f64,
    /// Plausibility multiplier on the temporal ASF sigma.
    #[serde(default = "d_k")]
    pub k_plausible: f64,
    /// Primary factor (refractive index of the atmosphere at the surface).
    #[serde(default = "d_eta")]
    pub eta: f64,
}

fn d_age() -> f64 {
    60.0
}
fn d_k() -> f64 {
    5.0
}
fn d_eta() -> f64 {
    1.000_338
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Consistent with every correction applied.
    Nominal,
    /// One correction excluded; the station kept on the map alone.
    CorrectionExcluded,
    /// One station excluded.
    StationExcluded,
    /// Detected and not isolated: alarm.
    Unresolved,
    /// Too few stations to detect (fewer than four after screening).
    NoRedundancy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FdeEpochResult {
    pub t_s: f64,
    pub decision: Decision,
    pub excluded: Option<String>,
    /// Stations screened before the solution, with the reason.
    pub screened: Vec<(String, String)>,
    pub test_statistic: Option<f64>,
    pub threshold: Option<f64>,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    pub hpl_m: Option<f64>,
    pub alarm: bool,
    pub horizontal_error_m: Option<f64>,
    /// Truth outside the HPL with no alarm (test records only).
    pub misleading: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FdeReport {
    pub format: String,
    pub epochs: Vec<FdeEpochResult>,
    pub nominal: usize,
    pub corrections_excluded: usize,
    pub stations_excluded: usize,
    pub unresolved: usize,
    pub alarms: usize,
    pub misleading: usize,
    pub evidence: String,
}

pub const FORMAT: &str = "kshana-eloran-asf-fde/1";

/// A measurement row ready for the solver (metres).
#[derive(Clone, Debug)]
struct Row {
    id: String,
    at: Geo,
    /// Pseudorange with the applied ASF removed (m).
    rho_m: f64,
    var_m2: f64,
    /// Pseudorange with the map alone (m), and its variance.
    rho_map_m: f64,
    var_map_m2: f64,
}

struct Solution {
    lat: f64,
    lon: f64,
    bias_m: f64,
    q: f64,
    /// (H^T W H)^-1 (north, east, clock).
    cov: [[f64; 3]; 3],
    /// Unit design rows at the solution.
    h: Vec<[f64; 3]>,
}

fn inv3(a: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);
    if det.abs() < 1e-30 {
        return None;
    }
    let mut m = [[0.0; 3]; 3];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let (r0, r1) = ((j + 1) % 3, (j + 2) % 3);
            let (c0, c1) = ((i + 1) % 3, (i + 2) % 3);
            *v = (a[r0][c0] * a[r1][c1] - a[r0][c1] * a[r1][c0]) / det;
        }
    }
    Some(m)
}

fn predicted(eta: f64, user: Geo, s: Geo) -> (f64, f64) {
    let inv = inverse(user, s);
    (eta * inv.distance_m, inv.az1_rad)
}

fn solve(rows: &[Row], start: Geo, eta: f64) -> Option<Solution> {
    if rows.len() < 3 {
        return None;
    }
    let (mut lat, mut lon) = (start.lat_deg, start.lon_deg);
    let mut bias = rows[0].rho_m - predicted(eta, start, rows[0].at).0;
    for _ in 0..20 {
        let p = Geo::new(lat, lon);
        let mut a = [[0.0; 3]; 3];
        let mut g = [0.0; 3];
        for r in rows {
            let (d, az) = predicted(eta, p, r.at);
            let h = [-eta * az.cos(), -eta * az.sin(), 1.0];
            let w = 1.0 / r.var_m2;
            let e = r.rho_m - d - bias;
            for i in 0..3 {
                g[i] += w * h[i] * e;
                for j in 0..3 {
                    a[i][j] += w * h[i] * h[j];
                }
            }
        }
        let ai = inv3(a)?;
        let dx: Vec<f64> = (0..3)
            .map(|i| (0..3).map(|j| ai[i][j] * g[j]).sum())
            .collect();
        lat += (dx[0] / R_EARTH).to_degrees();
        lon += (dx[1] / (R_EARTH * lat.to_radians().cos())).to_degrees();
        bias += dx[2];
        if dx[0].abs() < 1e-4 && dx[1].abs() < 1e-4 {
            break;
        }
    }
    let p = Geo::new(lat, lon);
    let mut a = [[0.0; 3]; 3];
    let mut q = 0.0;
    let mut hs = vec![];
    for r in rows {
        let (d, az) = predicted(eta, p, r.at);
        let h = [-eta * az.cos(), -eta * az.sin(), 1.0];
        let w = 1.0 / r.var_m2;
        q += w * (r.rho_m - d - bias).powi(2);
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += w * h[i] * h[j];
            }
        }
        hs.push(h);
    }
    Some(Solution {
        lat,
        lon,
        bias_m: bias,
        q,
        cov: inv3(a)?,
        h: hs,
    })
}

/// Residual of a row predicted from a solution that excluded it, normalised by the
/// combined sigma (measurement + prediction).
fn predicted_residual(sol: &Solution, eta: f64, at: Geo, rho: f64, var: f64) -> f64 {
    let p = Geo::new(sol.lat, sol.lon);
    let (d, az) = predicted(eta, p, at);
    let h = [-eta * az.cos(), -eta * az.sin(), 1.0];
    let mut pv = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            pv += h[i] * sol.cov[i][j] * h[j];
        }
    }
    (rho - d - sol.bias_m) / (var + pv).sqrt()
}

/// Brown's slope HPL for the geometry of `sol` with `rows`.
fn hpl(sol: &Solution, rows: &[Row], t: f64, k_md: f64) -> f64 {
    let n = rows.len();
    // A = (H^T W H)^-1 H^T W  (3 × n); S = I − H A.
    let a: Vec<[f64; 3]> = (0..n)
        .map(|k| {
            let w = 1.0 / rows[k].var_m2;
            let mut col = [0.0; 3];
            for (i, c) in col.iter_mut().enumerate() {
                *c = (0..3).map(|j| sol.cov[i][j] * sol.h[k][j]).sum::<f64>() * w;
            }
            col
        })
        .collect();
    let mut slope_max = 0.0f64;
    for k in 0..n {
        let hk = sol.h[k];
        let s_kk = 1.0 - (0..3).map(|i| hk[i] * a[k][i]).sum::<f64>();
        if s_kk <= 1e-12 {
            continue;
        }
        let horiz = (a[k][0].powi(2) + a[k][1].powi(2)).sqrt();
        let slope = horiz * rows[k].var_m2.sqrt() / s_kk.sqrt();
        slope_max = slope_max.max(slope);
    }
    let (cnn, cee, cne) = (sol.cov[0][0], sol.cov[1][1], sol.cov[0][1]);
    let major = ((cnn + cee) / 2.0 + (((cnn - cee) / 2.0).powi(2) + cne * cne).sqrt()).sqrt();
    slope_max * t.sqrt() + k_md * major
}

/// Run the FDE on every epoch.
pub fn run(cfg: &FdeConfig, epochs: &[FdeEpoch]) -> Result<FdeReport, String> {
    if !(cfg.p_fa > 0.0 && cfg.p_fa < 0.5 && cfg.p_md > 0.0 && cfg.p_md < 0.5) {
        return Err("p_fa and p_md must be in (0, 0.5)".into());
    }
    if !(cfg.hal_m > 0.0) {
        return Err("hal_m must be > 0".into());
    }
    let k_md = normal_quantile(1.0 - cfg.p_md);
    let mut out = vec![];
    for e in epochs {
        out.push(one(cfg, e, k_md)?);
    }
    let c = |d: Decision| out.iter().filter(|r| r.decision == d).count();
    Ok(FdeReport {
        format: FORMAT.into(),
        nominal: c(Decision::Nominal),
        corrections_excluded: c(Decision::CorrectionExcluded),
        stations_excluded: c(Decision::StationExcluded),
        unresolved: c(Decision::Unresolved),
        alarms: out.iter().filter(|r| r.alarm).count(),
        misleading: out.iter().filter(|r| r.misleading).count(),
        epochs: out,
        evidence: "MODELLED: weighted least squares with chi-square detection, correction-versus-signal separation by leave-one-out prediction, slope HPL; thresholds from the stated p_fa and p_md".into(),
    })
}

fn one(cfg: &FdeConfig, e: &FdeEpoch, k_md: f64) -> Result<FdeEpochResult, String> {
    let mut screened = vec![];
    let mut rows: Vec<Row> = vec![];
    for s in &e.stations {
        if !(s.sigma_ns > 0.0 && s.asf_temporal_sigma_ns >= 0.0) {
            return Err(format!("station {}: sigma_ns must be > 0", s.id));
        }
        if s.ldc_unhealthy {
            screened.push((s.id.clone(), "LDC unhealthy warning".into()));
            continue;
        }
        let v_meas = (s.sigma_ns * C_M_PER_NS).powi(2);
        let v_temp = (s.asf_temporal_sigma_ns * C_M_PER_NS).powi(2);
        let rho_map = (s.pseudo_delay_ns - s.asf_map_ns) * C_M_PER_NS;
        let mut row = Row {
            id: s.id.clone(),
            at: Geo::new(s.lat_deg, s.lon_deg),
            rho_m: rho_map,
            var_m2: v_meas + v_temp,
            rho_map_m: rho_map,
            var_map_m2: v_meas + v_temp,
        };
        if let Some(corr) = s.ldc_correction_ns {
            if s.ldc_age_s.is_some_and(|a| a > cfg.max_age_s) {
                screened.push((
                    s.id.clone(),
                    "differential correction too old: map only".into(),
                ));
            } else if s.asf_temporal_sigma_ns > 0.0
                && corr.abs() > cfg.k_plausible * s.asf_temporal_sigma_ns
            {
                screened.push((
                    s.id.clone(),
                    "differential correction implausible (larger than the temporal ASF bound): map only".into(),
                ));
            } else {
                row.rho_m = rho_map - corr * C_M_PER_NS;
                row.var_m2 = v_meas + (s.ldc_sigma_ns * C_M_PER_NS).powi(2);
            }
        }
        rows.push(row);
    }
    let start = Geo::new(e.approx_lat_deg, e.approx_lon_deg);
    let truth = match (e.truth_lat_deg, e.truth_lon_deg) {
        (Some(a), Some(o)) => Some(Geo::new(a, o)),
        _ => None,
    };
    let finish = |decision: Decision,
                  excluded: Option<String>,
                  sol: Option<(&Solution, &[Row])>,
                  q: Option<f64>,
                  t: Option<f64>,
                  screened: Vec<(String, String)>| {
        let (lat, lon, h) = match sol {
            Some((s, r)) => {
                let thr = chi2_quantile(1.0 - cfg.p_fa, r.len().saturating_sub(3).max(1));
                (Some(s.lat), Some(s.lon), Some(hpl(s, r, thr, k_md)))
            }
            None => (None, None, None),
        };
        let alarm = matches!(decision, Decision::Unresolved | Decision::NoRedundancy)
            || h.is_none_or(|h| h > cfg.hal_m);
        let err = match (truth, lat, lon) {
            (Some(tr), Some(a), Some(o)) => Some(inverse(tr, Geo::new(a, o)).distance_m),
            _ => None,
        };
        FdeEpochResult {
            t_s: e.t_s,
            decision,
            excluded,
            screened,
            test_statistic: q,
            threshold: t,
            lat_deg: lat,
            lon_deg: lon,
            hpl_m: h,
            alarm,
            horizontal_error_m: err,
            misleading: !alarm && err.zip(h).is_some_and(|(e, h)| e > h),
        }
    };
    if rows.len() < 4 {
        let s = solve(&rows, start, cfg.eta);
        return Ok(finish(
            Decision::NoRedundancy,
            None,
            s.as_ref().map(|s| (s, rows.as_slice())),
            None,
            None,
            screened,
        ));
    }
    let sol = solve(&rows, start, cfg.eta).ok_or("singular geometry")?;
    let t_all = chi2_quantile(1.0 - cfg.p_fa, rows.len() - 3);
    if sol.q <= t_all {
        return Ok(finish(
            Decision::Nominal,
            None,
            Some((&sol, &rows)),
            Some(sol.q),
            Some(t_all),
            screened,
        ));
    }
    // Separation: leave-one-out per station.
    let k_sep = normal_quantile(1.0 - cfg.p_fa / 2.0);
    let mut best: Option<(f64, Decision, usize, Vec<Row>)> = None;
    for j in 0..rows.len() {
        let others: Vec<Row> = rows
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != j)
            .map(|(_, r)| r.clone())
            .collect();
        let Some(s) = solve(&others, start, cfg.eta) else {
            continue;
        };
        let t_o = chi2_quantile(1.0 - cfg.p_fa, others.len().saturating_sub(3).max(1));
        if others.len() > 3 && s.q > t_o {
            continue;
        }
        let r = &rows[j];
        let z_corr = predicted_residual(&s, cfg.eta, r.at, r.rho_m, r.var_m2);
        let z_map = predicted_residual(&s, cfg.eta, r.at, r.rho_map_m, r.var_map_m2);
        let has_corr = (r.rho_m - r.rho_map_m).abs() > 0.0;
        let (dec, set) = if has_corr && z_corr.abs() > k_sep && z_map.abs() <= k_sep {
            let mut set = rows.clone();
            set[j].rho_m = set[j].rho_map_m;
            set[j].var_m2 = set[j].var_map_m2;
            (Decision::CorrectionExcluded, set)
        } else if z_corr.abs() > k_sep {
            (Decision::StationExcluded, others)
        } else {
            continue;
        };
        let Some(s2) = solve(&set, start, cfg.eta) else {
            continue;
        };
        let t2 = chi2_quantile(1.0 - cfg.p_fa, set.len().saturating_sub(3).max(1));
        if s2.q > t2 {
            continue;
        }
        let score = s2.q / t2;
        if best.as_ref().is_none_or(|b| score < b.0) {
            best = Some((score, dec, j, set));
        }
    }
    match best {
        Some((_, dec, j, set)) => {
            let s2 = solve(&set, start, cfg.eta).ok_or("singular geometry")?;
            let t2 = chi2_quantile(1.0 - cfg.p_fa, set.len().saturating_sub(3).max(1));
            Ok(finish(
                dec,
                Some(rows[j].id.clone()),
                Some((&s2, &set)),
                Some(s2.q),
                Some(t2),
                screened,
            ))
        }
        None => Ok(finish(
            Decision::Unresolved,
            None,
            Some((&sol, &rows)),
            Some(sol.q),
            Some(t_all),
            screened,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(i: u64) -> f64 {
        let x = i
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }

    /// Six stations around (55, 10); pseudo-delays from the truth with map ASF, a temporal
    /// ASF offset carried exactly by the LDC correction, a clock bias and small noise.
    fn epoch(seed: u64) -> FdeEpoch {
        let truth = Geo::new(55.0, 10.0);
        let sites = [
            (57.0, 9.0),
            (56.5, 13.0),
            (54.0, 13.5),
            (53.0, 9.5),
            (54.5, 6.5),
            (56.8, 6.8),
        ];
        let stations = sites
            .iter()
            .enumerate()
            .map(|(i, &(la, lo))| {
                let d = inverse(truth, Geo::new(la, lo)).distance_m * 1.000_338;
                let map = 300.0 + 50.0 * i as f64;
                let temporal = 40.0 * noise(seed * 31 + i as u64);
                let clock_ns = 5_000.0;
                StationObs {
                    id: format!("S{i}"),
                    lat_deg: la,
                    lon_deg: lo,
                    pseudo_delay_ns: d / C_M_PER_NS
                        + map
                        + temporal
                        + clock_ns
                        + 5.0 * noise(seed * 977 + i as u64),
                    sigma_ns: 10.0,
                    asf_map_ns: map,
                    asf_temporal_sigma_ns: 30.0,
                    ldc_correction_ns: Some(temporal),
                    ldc_age_s: Some(10.0),
                    ldc_sigma_ns: 5.0,
                    ldc_unhealthy: false,
                }
            })
            .collect();
        FdeEpoch {
            t_s: seed as f64,
            approx_lat_deg: 55.05,
            approx_lon_deg: 10.05,
            stations,
            truth_lat_deg: Some(55.0),
            truth_lon_deg: Some(10.0),
        }
    }

    fn cfg() -> FdeConfig {
        FdeConfig {
            p_fa: 1e-4,
            p_md: 1e-3,
            hal_m: 500.0,
            max_age_s: 60.0,
            k_plausible: 5.0,
            eta: 1.000_338,
        }
    }

    #[test]
    fn nominal_epochs_pass_and_bound_the_error() {
        let eps: Vec<FdeEpoch> = (1..20).map(epoch).collect();
        let r = run(&cfg(), &eps).unwrap();
        assert_eq!(r.nominal, eps.len(), "{:?}", r.epochs[0]);
        assert_eq!(r.misleading, 0);
        assert!(r
            .epochs
            .iter()
            .all(|e| e.horizontal_error_m.unwrap() < 30.0));
    }

    #[test]
    fn a_corrupted_correction_is_excluded_and_the_station_kept() {
        let mut e = epoch(3);
        // The LDC delivers +120 ns on S2 (within the plausibility bound); the signal is fine.
        *e.stations[2].ldc_correction_ns.as_mut().unwrap() += 120.0;
        let r = run(&cfg(), &[e]).unwrap();
        let x = &r.epochs[0];
        assert_eq!(x.decision, Decision::CorrectionExcluded, "{x:?}");
        assert_eq!(x.excluded.as_deref(), Some("S2"));
        assert!(!x.misleading);
    }

    #[test]
    fn a_station_signal_fault_excludes_the_station() {
        let mut e = epoch(4);
        e.stations[1].pseudo_delay_ns += 400.0;
        let r = run(&cfg(), &[e]).unwrap();
        let x = &r.epochs[0];
        assert_eq!(x.decision, Decision::StationExcluded, "{x:?}");
        assert_eq!(x.excluded.as_deref(), Some("S1"));
        assert!(x.horizontal_error_m.unwrap() < 50.0);
    }

    #[test]
    fn screening_stale_implausible_and_unhealthy() {
        let mut e = epoch(5);
        e.stations[0].ldc_age_s = Some(600.0);
        *e.stations[1].ldc_correction_ns.as_mut().unwrap() = 900.0;
        e.stations[2].ldc_unhealthy = true;
        let r = run(&cfg(), &[e]).unwrap();
        let x = &r.epochs[0];
        assert_eq!(x.screened.len(), 3, "{:?}", x.screened);
        assert!(x.lat_deg.is_some());
        // Two faults among few stations: never silently misleading.
        assert!(!x.misleading);
    }

    #[test]
    fn too_few_stations_alarm() {
        let mut e = epoch(6);
        e.stations.truncate(3);
        let r = run(&cfg(), &[e]).unwrap();
        assert_eq!(r.epochs[0].decision, Decision::NoRedundancy);
        assert!(r.epochs[0].alarm);
    }
}
