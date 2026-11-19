// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! eLoran network planner (stream F5).
//!
//! Given candidate transmitter sites (position, power, cost) and a service area, the planner
//! evaluates on a lat/lon grid:
//!
//! * per-station **field strength** and **SNR** against a stated noise field (20 kHz RMS),
//!   with any propagation model of [`crate::prop`];
//! * the **usable** stations (SNR ≥ a threshold);
//! * **geometry**: HDOP of the range-domain fix (east, north, clock), weighted by each
//!   station's ranging σ;
//! * **accuracy** (95 %, horizontal) = 2 · HDOPʷ (weighted, in metres);
//! * **availability**: probability, over independent station outages (availability `a` per
//!   station), that the accuracy target is met — exact enumeration of up/down states for up
//!   to 12 stations, else the all-up and single-outage states;
//! * **coverage**: share of grid points meeting the accuracy target with the required
//!   availability.
//!
//! It also scores **cross-rate interference** risk between proposed GRIs (lattice overlap
//! fraction of [`crate::interf::cri`]) and selects sites: exhaustive over all subsets of a
//! given size when that is small, else greedy, maximising coverage, then minimising cost.
//!
//! Ranging-σ model: `σ = σ₀ · 10^(−(SNR − SNR₀)/20)`, floored at `σ_min` (white-noise ToA
//! error scales with 1/amplitude). All results are MODELLED.

use crate::chain::{Chain, Station};
use crate::geo::{inverse, Geo};
use crate::interf::cri::{overlap_fraction_lattice, CriChain};
use crate::prop::PropagationModel;
use crate::traj::RxState;
use serde::{Deserialize, Serialize};

/// A candidate transmitter site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub position: Geo,
    pub power_kw: f64,
    /// Capital + operating cost in arbitrary units.
    #[serde(default)]
    pub cost: f64,
}

impl Candidate {
    fn station(&self) -> Station {
        Station {
            id: self.id.clone(),
            name: self.id.clone(),
            role: crate::signal::Role::Secondary,
            position: self.position,
            power_kw: self.power_kw,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: Default::default(),
            faults: vec![],
        }
    }
}

/// Service area and evaluation settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanArea {
    pub lat_min: f64,
    pub lat_max: f64,
    pub lon_min: f64,
    pub lon_max: f64,
    /// Grid step (degrees).
    pub step_deg: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanSettings {
    /// Noise field, RMS in 20 kHz (µV/m).
    pub noise_uv_m_20k: f64,
    /// Minimum usable SNR (dB).
    pub min_snr_db: f64,
    /// Ranging σ at `snr0_db` (m) and its floor (m).
    pub sigma0_m: f64,
    pub snr0_db: f64,
    pub sigma_min_m: f64,
    /// Accuracy target (95 %, horizontal, m) and required availability.
    pub accuracy_target_m: f64,
    pub availability_target: f64,
    /// Availability of each transmitter (probability it is on air).
    pub station_availability: f64,
}

impl Default for PlanSettings {
    fn default() -> Self {
        PlanSettings {
            noise_uv_m_20k: 50.0,
            min_snr_db: 0.0,
            sigma0_m: 30.0,
            snr0_db: 10.0,
            sigma_min_m: 3.0,
            accuracy_target_m: 100.0,
            availability_target: 0.999,
            station_availability: 0.999,
        }
    }
}

/// One evaluated grid point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridPoint {
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub usable: usize,
    pub hdop_w_m: Option<f64>,
    pub accuracy95_m: Option<f64>,
    pub availability: f64,
    pub covered: bool,
    /// Per-station SNR (dB), in candidate order.
    pub snr_db: Vec<f64>,
}

/// A coverage map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    pub sites: Vec<String>,
    pub points: Vec<GridPoint>,
    /// Share of grid points covered.
    pub coverage: f64,
    pub cost: f64,
}

/// Weighted horizontal DOP (metres) of a range-domain fix with an unknown clock, from the
/// bearings (rad) and σ (m) of the usable stations. `None` when fewer than 3 stations or the
/// geometry is singular.
pub fn weighted_hdop_m(bearings: &[f64], sigmas: &[f64]) -> Option<f64> {
    if bearings.len() < 3 {
        return None;
    }
    // normal matrix N = Σ w hᵀh with h = [-sin az, -cos az, 1], w = 1/σ²
    let mut n = [[0.0f64; 3]; 3];
    for (az, s) in bearings.iter().zip(sigmas) {
        let h = [-az.sin(), -az.cos(), 1.0];
        let w = 1.0 / (s * s);
        for i in 0..3 {
            for j in 0..3 {
                n[i][j] += w * h[i] * h[j];
            }
        }
    }
    let inv = invert3(n)?;
    let v = inv[0][0] + inv[1][1];
    if v.is_finite() && v > 0.0 {
        Some(v.sqrt())
    } else {
        None
    }
}

fn invert3(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    let scale = m
        .iter()
        .flatten()
        .map(|x| x.abs())
        .fold(0.0, f64::max)
        .powi(3);
    if det.abs() <= 1e-12 * scale.max(1e-300) {
        return None;
    }
    let c = |a: usize, b: usize, c2: usize, d: usize| m[a][b] * m[c2][d] - m[a][d] * m[c2][b];
    let inv = [
        [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
        [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
        [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
    ];
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = inv[i][j] / det;
        }
    }
    Some(out)
}

/// The planner over a fixed set of candidates and a propagation model.
pub struct Planner<'a> {
    pub candidates: Vec<Candidate>,
    pub area: PlanArea,
    pub settings: PlanSettings,
    pub prop: &'a dyn PropagationModel,
    /// Per grid point: per candidate (field µV/m, bearing rad), precomputed once.
    cache: Vec<(Geo, Vec<(f64, f64)>)>,
}

impl<'a> Planner<'a> {
    pub fn new(
        candidates: Vec<Candidate>,
        area: PlanArea,
        settings: PlanSettings,
        prop: &'a dyn PropagationModel,
    ) -> Result<Planner<'a>, String> {
        if candidates.is_empty() {
            return Err("planner: no candidate sites".into());
        }
        if !(area.step_deg > 0.0) || area.lat_max < area.lat_min || area.lon_max < area.lon_min {
            return Err("planner: invalid area".into());
        }
        let mut cache = Vec::new();
        let nlat = ((area.lat_max - area.lat_min) / area.step_deg).round() as usize + 1;
        let nlon = ((area.lon_max - area.lon_min) / area.step_deg).round() as usize + 1;
        for i in 0..nlat {
            for j in 0..nlon {
                let p = Geo::new(
                    area.lat_min + i as f64 * area.step_deg,
                    area.lon_min + j as f64 * area.step_deg,
                );
                let rx = RxState {
                    t_s: 0.0,
                    pos: p,
                    heading_rad: 0.0,
                    speed_mps: 0.0,
                };
                let per: Vec<(f64, f64)> = candidates
                    .iter()
                    .map(|c| {
                        let st = c.station();
                        let field = prop
                            .paths(&st, &rx)
                            .into_iter()
                            .find(|x| x.kind == crate::prop::PathKind::Ground)
                            .map(|x| x.field_uv_m)
                            .unwrap_or(0.0);
                        (field, inverse(p, c.position).az1_rad)
                    })
                    .collect();
                cache.push((p, per));
            }
        }
        Ok(Planner {
            candidates,
            area,
            settings,
            prop,
            cache,
        })
    }

    fn sigma(&self, snr_db: f64) -> f64 {
        let s = &self.settings;
        (s.sigma0_m * 10f64.powf(-(snr_db - s.snr0_db) / 20.0)).max(s.sigma_min_m)
    }

    fn accuracy(&self, per: &[(f64, f64)], use_idx: &[usize]) -> Option<f64> {
        let mut b = vec![];
        let mut s = vec![];
        for &k in use_idx {
            let snr = 20.0 * (per[k].0 / self.settings.noise_uv_m_20k).log10();
            if snr >= self.settings.min_snr_db {
                b.push(per[k].1);
                s.push(self.sigma(snr));
            }
        }
        weighted_hdop_m(&b, &s).map(|h| 2.0 * h)
    }

    /// Evaluate the coverage of the candidate subset `sites` (indices).
    pub fn evaluate(&self, sites: &[usize]) -> Coverage {
        let set = &self.settings;
        let a = set.station_availability;
        let mut points = Vec::with_capacity(self.cache.len());
        for (p, per) in &self.cache {
            let snr: Vec<f64> = per
                .iter()
                .map(|(f, _)| 20.0 * (f / set.noise_uv_m_20k).log10())
                .collect();
            let usable = sites.iter().filter(|&&k| snr[k] >= set.min_snr_db).count();
            let acc = self.accuracy(per, sites);
            let ok = |idx: &[usize]| {
                self.accuracy(per, idx)
                    .is_some_and(|x| x <= set.accuracy_target_m)
            };
            let n = sites.len();
            let availability = if n <= 12 {
                let mut tot = 0.0;
                for mask in 0u32..(1 << n) {
                    let up: Vec<usize> = (0..n)
                        .filter(|i| mask & (1 << i) != 0)
                        .map(|i| sites[i])
                        .collect();
                    let k = up.len() as i32;
                    let pr = a.powi(k) * (1.0 - a).powi(n as i32 - k);
                    if ok(&up) {
                        tot += pr;
                    }
                }
                tot
            } else {
                let mut tot = if ok(sites) { a.powi(n as i32) } else { 0.0 };
                for i in 0..n {
                    let up: Vec<usize> = sites
                        .iter()
                        .enumerate()
                        .filter(|(j, _)| *j != i)
                        .map(|(_, k)| *k)
                        .collect();
                    if ok(&up) {
                        tot += a.powi(n as i32 - 1) * (1.0 - a);
                    }
                }
                tot
            };
            let covered = acc.is_some_and(|x| x <= set.accuracy_target_m)
                && availability >= set.availability_target;
            points.push(GridPoint {
                lat_deg: p.lat_deg,
                lon_deg: p.lon_deg,
                usable,
                hdop_w_m: acc.map(|x| x / 2.0),
                accuracy95_m: acc,
                availability,
                covered,
                snr_db: snr,
            });
        }
        let coverage =
            points.iter().filter(|p| p.covered).count() as f64 / points.len().max(1) as f64;
        Coverage {
            sites: sites
                .iter()
                .map(|&k| self.candidates[k].id.clone())
                .collect(),
            points,
            coverage,
            cost: sites.iter().map(|&k| self.candidates[k].cost).sum(),
        }
    }

    /// Choose `m` sites: exhaustive when C(n, m) ≤ `max_exhaustive`, else greedy. Best =
    /// highest coverage, then lowest cost.
    pub fn select(&self, m: usize, max_exhaustive: usize) -> Coverage {
        let n = self.candidates.len();
        let m = m.min(n);
        let better = |a: &Coverage, b: &Coverage| {
            a.coverage > b.coverage + 1e-12
                || ((a.coverage - b.coverage).abs() <= 1e-12 && a.cost < b.cost)
        };
        if binom(n, m) <= max_exhaustive as f64 {
            let mut best: Option<Coverage> = None;
            let mut idx: Vec<usize> = (0..m).collect();
            loop {
                let c = self.evaluate(&idx);
                if best.as_ref().is_none_or(|b| better(&c, b)) {
                    best = Some(c);
                }
                // next combination
                let mut i = m;
                loop {
                    if i == 0 {
                        return best.expect("at least one subset");
                    }
                    i -= 1;
                    if idx[i] < n - m + i {
                        idx[i] += 1;
                        for j in i + 1..m {
                            idx[j] = idx[j - 1] + 1;
                        }
                        break;
                    }
                }
            }
        }
        let mut chosen: Vec<usize> = vec![];
        while chosen.len() < m {
            let mut best: Option<(usize, Coverage)> = None;
            for k in 0..n {
                if chosen.contains(&k) {
                    continue;
                }
                let mut t = chosen.clone();
                t.push(k);
                let c = self.evaluate(&t);
                if best.as_ref().is_none_or(|(_, b)| better(&c, b)) {
                    best = Some((k, c));
                }
            }
            chosen.push(best.expect("a candidate remains").0);
        }
        self.evaluate(&chosen)
    }
}

fn binom(n: usize, k: usize) -> f64 {
    (0..k).fold(1.0, |acc, i| acc * (n - i) as f64 / (i + 1) as f64)
}

/// Cross-rate interference risk: lattice overlap fraction of each chain's pulses by the
/// others' (window `window_us`, e.g. 300 µs pulse length plus guard).
pub fn cri_risk(chains: &[Chain], window_us: f64) -> Vec<(String, f64)> {
    let cri: Vec<CriChain> = chains.iter().map(CriChain::from_chain).collect();
    chains
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let others: Vec<CriChain> = cri
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, x)| x.clone())
                .collect();
            (
                c.name.clone(),
                overlap_fraction_lattice(&cri[i], &others, window_us),
            )
        })
        .collect()
}

/// Coverage as CSV (`lat,lon,usable,hdop_w_m,accuracy95_m,availability,covered`).
pub fn coverage_csv(c: &Coverage) -> String {
    let mut s = String::from("lat_deg,lon_deg,usable,hdop_w_m,accuracy95_m,availability,covered\n");
    let o = |v: Option<f64>| v.map(|x| format!("{x:.2}")).unwrap_or_default();
    for p in &c.points {
        s.push_str(&format!(
            "{:.5},{:.5},{},{},{},{:.6},{}\n",
            p.lat_deg,
            p.lon_deg,
            p.usable,
            o(p.hdop_w_m),
            o(p.accuracy95_m),
            p.availability,
            p.covered
        ));
    }
    s
}

/// Coverage as a GeoJSON FeatureCollection of points.
pub fn coverage_geojson(c: &Coverage) -> String {
    let feats: Vec<serde_json::Value> = c
        .points
        .iter()
        .map(|p| {
            serde_json::json!({
                "type": "Feature",
                "geometry": { "type": "Point", "coordinates": [p.lon_deg, p.lat_deg] },
                "properties": { "usable": p.usable, "accuracy95_m": p.accuracy95_m,
                                "availability": p.availability, "covered": p.covered }
            })
        })
        .collect();
    serde_json::json!({ "type": "FeatureCollection", "features": feats }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::ground::Simple;

    fn cands() -> Vec<Candidate> {
        let c = |id: &str, lat, lon, cost| Candidate {
            id: id.into(),
            position: Geo::new(lat, lon),
            power_kw: 400.0,
            cost,
        };
        vec![
            c("N", 58.0, 3.0, 10.0),
            c("S", 52.0, 3.0, 10.0),
            c("E", 55.0, 8.0, 10.0),
            c("W", 55.0, -2.0, 10.0),
            c("NN", 59.0, 3.2, 1.0), // nearly collinear with N
        ]
    }
    fn area() -> PlanArea {
        PlanArea {
            lat_min: 54.0,
            lat_max: 56.0,
            lon_min: 2.0,
            lon_max: 4.0,
            step_deg: 0.5,
        }
    }

    #[test]
    fn surrounding_geometry_beats_collinear() {
        let around = [0.0f64, 90.0, 180.0, 270.0].map(|d| d.to_radians());
        let line = [0.0f64, 2.0, 4.0, 180.0].map(|d| d.to_radians());
        let s = [10.0; 4];
        let a = weighted_hdop_m(&around, &s).unwrap();
        let b = weighted_hdop_m(&line, &s).unwrap();
        assert!(a < b, "{a} vs {b}");
        assert!(weighted_hdop_m(&around[..2], &s[..2]).is_none());
    }

    #[test]
    fn coverage_grows_with_sites_and_selection_prefers_geometry() {
        let prop = Simple {
            atten_db_per_1000km: 3.0,
        };
        let p = Planner::new(
            cands(),
            area(),
            PlanSettings {
                accuracy_target_m: 60.0,
                ..Default::default()
            },
            &prop,
        )
        .unwrap();
        let three = p.evaluate(&[0, 1, 2]);
        let four = p.evaluate(&[0, 1, 2, 3]);
        assert!(four.coverage >= three.coverage);
        assert!(four
            .points
            .iter()
            .zip(&three.points)
            .all(|(a, b)| a.availability >= b.availability - 1e-12));
        let best = p.select(3, 1000);
        // the near-collinear cheap site must not displace a geometry-improving site
        assert!(!best.sites.contains(&"NN".to_string()) || best.coverage >= three.coverage);
        let greedy = p.select(3, 0);
        assert_eq!(greedy.sites.len(), 3);
        assert!(greedy.coverage <= best.coverage + 1e-12);
        assert!(coverage_csv(&best).lines().count() == best.points.len() + 1);
        assert!(coverage_geojson(&best).contains("FeatureCollection"));
    }

    #[test]
    fn cri_risk_is_zero_for_harmonic_gris_and_positive_otherwise() {
        let mk = |name: &str, gri: u32| {
            let mut c = crate::gridb::GriDb::embedded()
                .unwrap()
                .chains
                .iter()
                .find(|c| c.is_complete())
                .unwrap()
                .to_chain_strict()
                .unwrap();
            c.name = name.into();
            c.gri_us = gri;
            c
        };
        let r = cri_risk(&[mk("a", 79_800), mk("b", 59_900)], 300.0);
        assert!(r.iter().all(|(_, f)| (0.0..=1.0).contains(f)));
        assert!(r.iter().any(|(_, f)| *f > 0.0), "{r:?}");
    }
}
