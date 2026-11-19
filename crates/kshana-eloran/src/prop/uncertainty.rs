// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Propagation uncertainty: seeded Monte Carlo over the inputs of the groundwave and
//! skywave models — ground conductivity and permittivity (per ground type, with the
//! uncertainty of its atlas class), the effective-earth-radius factor, terrain-height
//! noise and the ionospheric reflection height — giving the distribution of the
//! groundwave delay, ASF and field (and of the skywave delay) at receiver points, with
//! percentiles, plus a one-at-a-time sensitivity ranking.
//!
//! # Sampling (MODELLED: the input distributions are assumptions)
//!
//! Per sample `s` (ChaCha8 seeded with `seed`, draws in a fixed order):
//!
//! * every ground type `g` of the map (distinct σ, ε_r pair met on any path) gets
//!   `σ_g · 10^{N(0, sd_g)}` and `ε_g · max(1/ε_g, 1 + N(0, e_g))` — the same draw on every
//!   path (a class is wrong everywhere at once); `sd_g`, `e_g` from the nearest
//!   [`super::p832`] class unless overridden by `sigma_log10_sd` / `eps_rel_sd`;
//! * `k = k₀ + N(0, k_factor_sd)` (clamped to 0.6–9), used for the model and its seawater
//!   SF reference alike;
//! * terrain (when a DEM is given): heights plus a Gaussian noise field of standard
//!   deviation `terrain_sd_m`, bilinear on a lattice of `terrain_corr_km` spacing,
//!   seeded per sample (the same field on every path);
//! * skywave: both virtual heights `+ N(0, sky_height_sd_km)`.
//!
//! The groundwave is evaluated per path by Millington over the sampled grounds (fresh,
//! uncached homogeneous-ground curves per sample, 300 √d intervals) or, with a DEM, by the
//! integral equation of [`super::terrain`] (`direct` method) over the sampled profile;
//! the path is then built exactly as the ground models do
//! ([`super::terrain::path_from_correction`]). The skywave delay is the
//! [`super::sky::Ionospheric`] one-hop delay with the sampled heights.
//!
//! With every uncertainty zero all samples equal the nominal evaluation (tested), and the
//! nominal Millington evaluation agrees with the `smooth_earth`/`millington` models within
//! 1 ns (the coarser curve tables).
//!
//! # Sensitivity
//!
//! One parameter at a time at ±1 standard deviation (others nominal); the ranking value is
//! the mean over the points of `|f(+1σ) − f(−1σ)|/2` for the delay (and reported for ASF,
//! field and skywave delay). Terrain noise has no "+1σ" state: its value is the standard
//! deviation over `terrain_realisations` (default 8) seeded noise fields.

use super::conductivity::{ConductivityMap, GroundConst};
use super::dem::Dem;
use super::ground::{Correction, F0_HZ, N_ATM};
use super::millington::{millington, sections_from_samples, Section};
use super::p832::{builtin_classes, nearest_class, GroundClass};
use super::sky::{IonoConfig, Ionospheric};
use super::smooth_earth::{Ground, GroundCurve, SEAWATER};
use super::terrain::{path_from_correction, smooth, solve_profile};
use super::PropagationModel;
use crate::chain::Station;
use crate::geo::{inverse, path_points, Geo};
use crate::time::AbsTime;
use crate::timebase::gauss;
use crate::traj::RxState;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Monte Carlo configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UncertaintyConfig {
    pub samples: usize,
    pub seed: u64,
    /// Override of every class's sd of log₁₀ σ.
    pub sigma_log10_sd: Option<f64>,
    /// Override of every class's relative sd of ε_r.
    pub eps_rel_sd: Option<f64>,
    pub k_factor_sd: f64,
    pub terrain_sd_m: f64,
    pub terrain_corr_km: f64,
    pub terrain_realisations: usize,
    pub sky_height_sd_km: f64,
    /// Percentiles reported (0–100).
    pub percentiles: Vec<f64>,
}

impl Default for UncertaintyConfig {
    fn default() -> Self {
        UncertaintyConfig {
            samples: 200,
            seed: 1,
            sigma_log10_sd: None,
            eps_rel_sd: None,
            k_factor_sd: 0.05,
            terrain_sd_m: 0.0,
            terrain_corr_km: 5.0,
            terrain_realisations: 8,
            sky_height_sd_km: 3.0,
            percentiles: vec![5.0, 50.0, 95.0],
        }
    }
}

impl UncertaintyConfig {
    /// All uncertainties zero (the nominal model in every sample).
    pub fn zero(samples: usize) -> UncertaintyConfig {
        UncertaintyConfig {
            samples,
            sigma_log10_sd: Some(0.0),
            eps_rel_sd: Some(0.0),
            k_factor_sd: 0.0,
            terrain_sd_m: 0.0,
            sky_height_sd_km: 0.0,
            ..Default::default()
        }
    }
}

/// Terrain settings when a DEM is used.
pub struct TerrainSetup<'a> {
    pub dem: &'a Dem,
    pub step_m: f64,
    pub smooth_m: f64,
    pub max_slope: f64,
}

/// What is evaluated.
pub struct Problem<'a> {
    pub stations: Vec<Station>,
    pub points: Vec<Geo>,
    pub map: &'a ConductivityMap,
    /// Classes for the per-type uncertainty (default: the built-in P.527 classes).
    pub classes: Vec<GroundClass>,
    pub k_factor: f64,
    /// Millington sample spacing (km).
    pub sample_km: f64,
    pub terrain: Option<TerrainSetup<'a>>,
    /// One-hop skywave (the delay is reported) and the scene start for `daylight = auto`.
    pub sky: Option<IonoConfig>,
    pub start: Option<AbsTime>,
}

impl<'a> Problem<'a> {
    /// A problem with the built-in classes, no terrain and no skywave.
    pub fn new(
        stations: Vec<Station>,
        points: Vec<Geo>,
        map: &'a ConductivityMap,
        k_factor: f64,
    ) -> Problem<'a> {
        Problem {
            stations,
            points,
            map,
            classes: builtin_classes(),
            k_factor,
            sample_km: 2.0,
            terrain: None,
            sky: None,
            start: None,
        }
    }
}

/// Summary statistics of one quantity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    pub nominal: f64,
    pub mean: f64,
    pub sd: f64,
    pub min: f64,
    pub max: f64,
    /// (percentile, value).
    pub percentiles: Vec<(f64, f64)>,
}

fn stats(nominal: f64, v: &[f64], pcts: &[f64]) -> Stats {
    let n = v.len().max(1) as f64;
    let mean = v.iter().sum::<f64>() / n;
    let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let pct = |p: f64| -> f64 {
        if s.is_empty() {
            return f64::NAN;
        }
        let x = (p / 100.0).clamp(0.0, 1.0) * (s.len() - 1) as f64;
        let i = x.floor() as usize;
        let j = (i + 1).min(s.len() - 1);
        s[i] + (s[j] - s[i]) * (x - i as f64)
    };
    Stats {
        nominal,
        mean,
        sd,
        min: s.first().copied().unwrap_or(f64::NAN),
        max: s.last().copied().unwrap_or(f64::NAN),
        percentiles: pcts.iter().map(|&p| (p, pct(p))).collect(),
    }
}

/// Distributions at one (station, point).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointResult {
    pub station: String,
    pub point: Geo,
    pub delay_ns: Stats,
    pub asf_ns: Stats,
    pub field_dbuv_m: Stats,
    pub sky_delay_us: Option<Stats>,
}

/// One row of the sensitivity ranking.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sensitivity {
    pub parameter: String,
    pub delay_ns: f64,
    pub asf_ns: f64,
    pub field_db: f64,
    pub sky_delay_us: f64,
}

/// The Monte Carlo result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyReport {
    pub samples: usize,
    pub seed: u64,
    pub results: Vec<PointResult>,
    /// Sorted by decreasing delay sensitivity.
    pub sensitivity: Vec<Sensitivity>,
}

/// Inputs of one evaluation.
#[derive(Clone, Debug)]
struct Draw {
    grounds: Vec<Ground>,
    k: f64,
    terrain: Option<(u64, f64)>, // (noise seed, sd)
    dh_sky_m: f64,
}

/// One evaluation's outputs per path: delay ns, ASF ns, field dB, sky delay µs.
type Out = Vec<[f64; 4]>;

struct PathGeom {
    st: usize,
    pt: usize,
    d: f64,
    /// Millington sections over ground-type indices.
    sections: Vec<Section>,
    /// Terrain profile: (spacing, raw heights, positions, ground index per node).
    profile: Option<(f64, Vec<f64>, Vec<Geo>, Vec<usize>)>,
}

/// Gaussian lattice noise, deterministic in (seed, i, j).
fn lattice_gauss(seed: u64, i: i64, j: i64) -> f64 {
    let mut h = seed ^ 0x9E37_79B9_7F4A_7C15;
    for v in [i as u64, j as u64] {
        h ^= v.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 31)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^= h >> 29;
    }
    let mut r = ChaCha8Rng::seed_from_u64(h);
    gauss(&mut r)
}

fn noise_at(seed: u64, corr_km: f64, p: Geo) -> f64 {
    let q = (corr_km / 111.2).max(1e-4);
    let (fi, fj) = (p.lat_deg / q, p.lon_deg / q);
    let (i0, j0) = (fi.floor() as i64, fj.floor() as i64);
    let (u, v) = (fi - i0 as f64, fj - j0 as f64);
    (1.0 - u) * (1.0 - v) * lattice_gauss(seed, i0, j0)
        + u * (1.0 - v) * lattice_gauss(seed, i0 + 1, j0)
        + (1.0 - u) * v * lattice_gauss(seed, i0, j0 + 1)
        + u * v * lattice_gauss(seed, i0 + 1, j0 + 1)
}

/// The Monte Carlo engine.
pub struct Engine<'a> {
    pub problem: Problem<'a>,
    pub cfg: UncertaintyConfig,
    kinds: Vec<GroundConst>,
    paths: Vec<PathGeom>,
    d_max: f64,
}

fn kind_index(kinds: &mut Vec<GroundConst>, g: GroundConst) -> usize {
    match kinds.iter().position(|k| *k == g) {
        Some(i) => i,
        None => {
            kinds.push(g);
            kinds.len() - 1
        }
    }
}

impl<'a> Engine<'a> {
    pub fn new(problem: Problem<'a>, cfg: UncertaintyConfig) -> Result<Engine<'a>, String> {
        if cfg.samples == 0 || cfg.samples > 100_000 {
            return Err("uncertainty: samples in 1..=100000".into());
        }
        if !(problem.k_factor > 0.6 && problem.k_factor < 9.0) || !(problem.sample_km > 0.0) {
            return Err("uncertainty: k_factor in (0.6, 9), sample_km > 0".into());
        }
        if problem.classes.is_empty() {
            return Err("uncertainty: no ground classes".into());
        }
        for v in [cfg.k_factor_sd, cfg.terrain_sd_m, cfg.sky_height_sd_km]
            .into_iter()
            .chain(cfg.sigma_log10_sd)
            .chain(cfg.eps_rel_sd)
        {
            if !(v >= 0.0 && v.is_finite()) {
                return Err("uncertainty: standard deviations must be finite and ≥ 0".into());
            }
        }
        if let Some(t) = &problem.terrain {
            if !(t.step_m >= 10.0) || !(t.max_slope > 0.0) {
                return Err("uncertainty: terrain step_m ≥ 10 m and max_slope > 0".into());
            }
        }
        let mut kinds = Vec::new();
        let mut paths = Vec::new();
        let mut d_max: f64 = 1000.0;
        for (si, st) in problem.stations.iter().enumerate() {
            for (pi, &p) in problem.points.iter().enumerate() {
                let d = inverse(st.position, p).distance_m;
                d_max = d_max.max(d);
                let n = ((d / (problem.sample_km * 1000.0)).ceil() as usize).clamp(2, 20_000);
                let idx: Vec<usize> = path_points(st.position, p, n)
                    .iter()
                    .map(|(_, q)| kind_index(&mut kinds, problem.map.at(*q)))
                    .collect();
                let sections = sections_from_samples(&idx, d);
                let profile = problem.terrain.as_ref().map(|t| {
                    let n = ((d / t.step_m).ceil() as usize).max(2);
                    let pp = path_points(st.position, p, n);
                    let pos: Vec<Geo> = pp.iter().map(|x| x.1).collect();
                    let h: Vec<f64> = pos.iter().map(|q| t.dem.height_m(*q)).collect();
                    let gi: Vec<usize> = pos
                        .iter()
                        .map(|q| kind_index(&mut kinds, problem.map.at(*q)))
                        .collect();
                    (d / n as f64, h, pos, gi)
                });
                paths.push(PathGeom {
                    st: si,
                    pt: pi,
                    d,
                    sections,
                    profile,
                });
            }
        }
        Ok(Engine {
            problem,
            cfg,
            kinds,
            paths,
            d_max: d_max * 1.05,
        })
    }

    /// Ground types met on the paths.
    pub fn ground_types(&self) -> &[GroundConst] {
        &self.kinds
    }

    fn class_sd(&self, g: GroundConst) -> (f64, f64) {
        let c = nearest_class(&self.problem.classes, g);
        (
            self.cfg.sigma_log10_sd.unwrap_or(c.sigma_log10_sd),
            self.cfg.eps_rel_sd.unwrap_or(c.eps_rel_sd),
        )
    }

    fn nominal_draw(&self) -> Draw {
        Draw {
            grounds: self
                .kinds
                .iter()
                .map(|g| Ground {
                    sigma: g.sigma_s_m,
                    eps_r: g.eps(),
                })
                .collect(),
            k: self.problem.k_factor,
            terrain: None,
            dh_sky_m: 0.0,
        }
    }

    fn random_draw(&self, rng: &mut ChaCha8Rng, sample: usize) -> Draw {
        let mut d = self.nominal_draw();
        for (i, g) in self.kinds.iter().enumerate() {
            let (ss, es) = self.class_sd(*g);
            let (zs, ze) = (gauss(rng), gauss(rng));
            d.grounds[i].sigma *= 10f64.powf(ss * zs);
            d.grounds[i].eps_r = (d.grounds[i].eps_r * (1.0 + es * ze)).max(1.0);
        }
        let zk = gauss(rng);
        d.k = (self.problem.k_factor + self.cfg.k_factor_sd * zk).clamp(0.6, 9.0);
        let zh = gauss(rng);
        d.dh_sky_m = self.cfg.sky_height_sd_km * 1000.0 * zh;
        if self.cfg.terrain_sd_m > 0.0 {
            d.terrain = Some((
                self.cfg.seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ sample as u64,
                self.cfg.terrain_sd_m,
            ));
        }
        d
    }

    fn evaluate(&self, draw: &Draw, curves: &mut HashMap<(u64, u64, u64), GroundCurve>) -> Out {
        let mut curve = |g: Ground, k: f64| -> GroundCurve {
            let key = (g.sigma.to_bits(), g.eps_r.to_bits(), k.to_bits());
            curves
                .entry(key)
                .or_insert_with(|| GroundCurve::new(g, F0_HZ, k, N_ATM, self.d_max, 300))
                .clone()
        };
        let sea = curve(SEAWATER, draw.k);
        let gcurves: Vec<GroundCurve> = draw.grounds.iter().map(|g| curve(*g, draw.k)).collect();
        let mut out = Vec::with_capacity(self.paths.len());
        for p in &self.paths {
            let st = &self.problem.stations[p.st];
            let rx = self.problem.points[p.pt];
            let s = sea.at(p.d);
            let corr = match (&p.profile, &self.problem.terrain) {
                (Some((step, h, pos, gi)), Some(t)) if p.d >= 2.0 * t.step_m => {
                    let hn: Vec<f64> = match draw.terrain {
                        Some((seed, sd)) => h
                            .iter()
                            .zip(pos)
                            .map(|(x, q)| x + sd * noise_at(seed, self.cfg.terrain_corr_km, *q))
                            .collect(),
                        None => h.clone(),
                    };
                    let hs = smooth(&hn, (t.smooth_m / step).round() as usize);
                    let grounds: Vec<Ground> = gi.iter().map(|i| draw.grounds[*i]).collect();
                    let ie = solve_profile(&hs, *step, &grounds, draw.k, t.max_slope);
                    Correction {
                        phase_s: ie.phase_delay_s - s.phase_delay_s,
                        ln_amp: ie.ln_amp - s.ln_amp,
                        group_s: ie.group_delay_s - s.group_delay_s,
                        land: 0.0,
                    }
                }
                _ => {
                    let m = millington(&p.sections, |g, x| gcurves[g].at(x));
                    Correction {
                        phase_s: m.phase_delay_s - s.phase_delay_s,
                        ln_amp: m.ln_amp - s.ln_amp,
                        group_s: m.group_delay_s - s.group_delay_s,
                        land: 0.0,
                    }
                }
            };
            let g = path_from_correction(&sea, draw.k, st.position, st.power_kw, rx, corr, 0.0);
            let sky = match &self.problem.sky {
                Some(c) => {
                    let mut c = c.clone();
                    c.h_day_km += draw.dh_sky_m / 1000.0;
                    c.h_night_km += draw.dh_sky_m / 1000.0;
                    c.fading = None;
                    match Ionospheric::with_start(c, self.problem.start) {
                        Ok(m) => {
                            let r = RxState {
                                t_s: 0.0,
                                pos: rx,
                                heading_rad: 0.0,
                                speed_mps: 0.0,
                            };
                            m.paths(st, &r)
                                .first()
                                .map(|q| q.delay.as_us() + q.ecd_s * 1e6)
                                .unwrap_or(f64::NAN)
                        }
                        Err(_) => f64::NAN,
                    }
                }
                None => f64::NAN,
            };
            out.push([
                g.delay.as_ns(),
                g.asf_ns,
                20.0 * g.field_uv_m.max(1e-30).log10(),
                sky,
            ]);
        }
        out
    }

    /// The nominal evaluation (no perturbation).
    pub fn nominal(&self) -> Vec<[f64; 4]> {
        self.evaluate(&self.nominal_draw(), &mut HashMap::new())
    }

    /// The individual Monte Carlo samples: per sample, per path (station-major, then point)
    /// `[delay ns, ASF ns, field dBµV/m, skywave delay µs]` (the draws [`Engine::run`]
    /// summarises). For callers that test a decision rule over the propagation draws.
    pub fn samples(&self) -> Vec<Vec<[f64; 4]>> {
        self.samples_with(&mut HashMap::new())
    }

    fn samples_with(&self, curves: &mut HashMap<(u64, u64, u64), GroundCurve>) -> Vec<Out> {
        let mut rng = ChaCha8Rng::seed_from_u64(self.cfg.seed);
        let mut samples: Vec<Out> = Vec::with_capacity(self.cfg.samples);
        for s in 0..self.cfg.samples {
            let d = self.random_draw(&mut rng, s);
            samples.push(self.evaluate(&d, curves));
            // the per-sample curves are not reused: keep the cache bounded
            if curves.len() > 64 {
                curves.clear();
            }
        }
        samples
    }

    /// Run the Monte Carlo and the sensitivity analysis.
    pub fn run(&self) -> Result<UncertaintyReport, String> {
        let mut curves = HashMap::new();
        let nominal = self.evaluate(&self.nominal_draw(), &mut curves);
        let samples = self.samples_with(&mut curves);
        let pcts = &self.cfg.percentiles;
        let results = self
            .paths
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let col = |k: usize| -> Vec<f64> { samples.iter().map(|s| s[i][k]).collect() };
                PointResult {
                    station: self.problem.stations[p.st].id.clone(),
                    point: self.problem.points[p.pt],
                    delay_ns: stats(nominal[i][0], &col(0), pcts),
                    asf_ns: stats(nominal[i][1], &col(1), pcts),
                    field_dbuv_m: stats(nominal[i][2], &col(2), pcts),
                    sky_delay_us: self
                        .problem
                        .sky
                        .as_ref()
                        .map(|_| stats(nominal[i][3], &col(3), pcts)),
                }
            })
            .collect();
        Ok(UncertaintyReport {
            samples: self.cfg.samples,
            seed: self.cfg.seed,
            results,
            sensitivity: self.sensitivity(&mut curves),
        })
    }

    fn sensitivity(&self, curves: &mut HashMap<(u64, u64, u64), GroundCurve>) -> Vec<Sensitivity> {
        let base = self.nominal_draw();
        let mean_half = |a: &Out, b: &Out| -> [f64; 4] {
            let mut r = [0.0; 4];
            for (x, y) in a.iter().zip(b) {
                for k in 0..4 {
                    let v = ((x[k] - y[k]) / 2.0).abs();
                    if v.is_finite() {
                        r[k] += v / a.len() as f64;
                    }
                }
            }
            r
        };
        let mut rows = Vec::new();
        let mut push = |name: String, r: [f64; 4]| {
            rows.push(Sensitivity {
                parameter: name,
                delay_ns: r[0],
                asf_ns: r[1],
                field_db: r[2],
                sky_delay_us: r[3],
            })
        };
        for (i, g) in self.kinds.iter().enumerate() {
            let (ss, es) = self.class_sd(*g);
            let cname = nearest_class(&self.problem.classes, *g).name.clone();
            let label = format!("{cname} (σ={} S/m, ε_r={})", g.sigma_s_m, g.eps());
            if ss > 0.0 {
                let (mut a, mut b) = (base.clone(), base.clone());
                a.grounds[i].sigma *= 10f64.powf(ss);
                b.grounds[i].sigma *= 10f64.powf(-ss);
                let r = mean_half(&self.evaluate(&a, curves), &self.evaluate(&b, curves));
                push(format!("sigma {label}"), r);
            }
            if es > 0.0 {
                let (mut a, mut b) = (base.clone(), base.clone());
                a.grounds[i].eps_r *= 1.0 + es;
                b.grounds[i].eps_r = (b.grounds[i].eps_r * (1.0 - es)).max(1.0);
                let r = mean_half(&self.evaluate(&a, curves), &self.evaluate(&b, curves));
                push(format!("eps_r {label}"), r);
            }
        }
        if self.cfg.k_factor_sd > 0.0 {
            let (mut a, mut b) = (base.clone(), base.clone());
            a.k = (a.k + self.cfg.k_factor_sd).clamp(0.6, 9.0);
            b.k = (b.k - self.cfg.k_factor_sd).clamp(0.6, 9.0);
            let r = mean_half(&self.evaluate(&a, curves), &self.evaluate(&b, curves));
            push("k_factor".into(), r);
        }
        if self.cfg.sky_height_sd_km > 0.0 && self.problem.sky.is_some() {
            let (mut a, mut b) = (base.clone(), base.clone());
            a.dh_sky_m = self.cfg.sky_height_sd_km * 1000.0;
            b.dh_sky_m = -a.dh_sky_m;
            let r = mean_half(&self.evaluate(&a, curves), &self.evaluate(&b, curves));
            push("sky_height".into(), r);
        }
        if self.cfg.terrain_sd_m > 0.0 && self.problem.terrain.is_some() {
            let nom = self.evaluate(&base, curves);
            let n = self.cfg.terrain_realisations.max(2);
            let runs: Vec<Out> = (0..n)
                .map(|r| {
                    let mut d = base.clone();
                    d.terrain = Some((
                        0xA5A5_0000 ^ r as u64 ^ self.cfg.seed,
                        self.cfg.terrain_sd_m,
                    ));
                    self.evaluate(&d, curves)
                })
                .collect();
            let mut acc = [0.0; 4];
            for (i, _) in nom.iter().enumerate() {
                for k in 0..4 {
                    let v: Vec<f64> = runs.iter().map(|r| r[i][k]).collect();
                    let s = stats(nom[i][k], &v, &[]).sd;
                    if s.is_finite() {
                        acc[k] += s / nom.len() as f64;
                    }
                }
            }
            push("terrain_height".into(), acc);
        }
        rows.sort_by(|a, b| b.delay_ns.total_cmp(&a.delay_ns));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::super::conductivity::{ConductivityConfig, PolygonConfig};
    use super::super::dem::{Hill, Hills};
    use super::super::ground::{Groundwave, K_LORAN};
    use super::super::sky::Daylight;
    use super::*;

    fn station() -> Station {
        Station {
            id: "M".into(),
            name: "M".into(),
            role: crate::signal::Role::Master,
            position: Geo::new(49.0, 0.0),
            power_kw: 400.0,
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: crate::chain::StationStatus::On,
            faults: vec![],
        }
    }
    fn coast() -> ConductivityMap {
        ConductivityMap::build(&ConductivityConfig::Polygons {
            background: GroundConst::seawater(),
            polygons: vec![PolygonConfig {
                name: "land".into(),
                sigma_s_m: 0.003,
                eps_r: Some(22.0),
                vertices: vec![[50.0, -5.0], [55.0, -5.0], [55.0, 5.0], [50.0, 5.0]],
            }],
        })
        .unwrap()
    }
    fn points() -> Vec<Geo> {
        vec![Geo::new(50.5, 0.3), Geo::new(51.2, -0.4)]
    }

    #[test]
    fn zero_uncertainty_reproduces_the_nominal_model() {
        let map = coast();
        let e = Engine::new(
            Problem::new(vec![station()], points(), &map, K_LORAN),
            UncertaintyConfig::zero(5),
        )
        .unwrap();
        let r = e.run().unwrap();
        let gw = Groundwave::new(map.clone(), K_LORAN, 2.0, 0.005, None, "m").unwrap();
        for p in &r.results {
            for s in [&p.delay_ns, &p.asf_ns, &p.field_dbuv_m] {
                assert_eq!(s.mean, s.nominal);
                assert_eq!(s.sd, 0.0);
                assert!(s.percentiles.iter().all(|(_, v)| *v == s.nominal));
            }
            // and the nominal agrees with the millington model (coarser curve tables)
            let m = gw.correction_exact(station().position, p.point);
            let full = gw.evaluate(station().position, 400.0, p.point, 0.0);
            // ~0.4 % of a ~600 ns ASF from the coarser curve tables used here
            assert!(
                (p.asf_ns.nominal - m.phase_s * 1e9).abs() < 3.0,
                "{} vs {}",
                p.asf_ns.nominal,
                m.phase_s * 1e9
            );
            assert!(
                (p.delay_ns.nominal - full.delay.as_ns()).abs() < 3.5,
                "{} vs {}",
                p.delay_ns.nominal,
                full.delay.as_ns()
            );
            assert!((p.field_dbuv_m.nominal - 20.0 * full.field_uv_m.log10()).abs() < 0.02);
        }
        assert!(r.sensitivity.is_empty());
        assert_eq!(e.ground_types().len(), 2);
    }

    #[test]
    fn monte_carlo_is_deterministic_and_ranks_conductivity() {
        let map = coast();
        let sky = IonoConfig {
            daylight: Daylight::Night,
            ..Default::default()
        };
        let mk = |seed: u64| {
            let mut p = Problem::new(vec![station()], points(), &map, K_LORAN);
            p.sky = Some(sky.clone());
            Engine::new(
                p,
                UncertaintyConfig {
                    samples: 12,
                    seed,
                    ..Default::default()
                },
            )
            .unwrap()
            .run()
            .unwrap()
        };
        let a = mk(7);
        let b = mk(7);
        let c = mk(8);
        assert_eq!(a, b, "same seed, same result");
        assert_ne!(a.results[0].asf_ns.mean, c.results[0].asf_ns.mean);
        let r = &a.results[0];
        assert!(
            r.asf_ns.sd > 1.0,
            "land σ uncertainty spreads the ASF: {}",
            r.asf_ns.sd
        );
        let p = &r.asf_ns.percentiles;
        assert!(p[0].1 <= p[1].1 && p[1].1 <= p[2].1);
        let sky_s = r.sky_delay_us.as_ref().unwrap();
        assert!(sky_s.sd > 0.1 && sky_s.sd < 50.0, "{}", sky_s.sd);
        // ranking: the land conductivity dominates the groundwave delay; the sky height
        // moves only the skywave
        let top = &a.sensitivity[0];
        assert!(
            top.parameter.starts_with("sigma land"),
            "{:?}",
            a.sensitivity
        );
        let sh = a
            .sensitivity
            .iter()
            .find(|s| s.parameter == "sky_height")
            .unwrap();
        assert_eq!(sh.delay_ns, 0.0);
        assert!(sh.sky_delay_us > 0.5);
        assert!(a
            .sensitivity
            .windows(2)
            .all(|w| w[0].delay_ns >= w[1].delay_ns));
        // JSON round trip of the report
        let j = serde_json::to_string(&a).unwrap();
        let back: UncertaintyReport = serde_json::from_str(&j).unwrap();
        assert_eq!(back.samples, 12);
    }

    #[test]
    fn terrain_noise_spreads_the_delay() {
        let map = ConductivityMap::build(&ConductivityConfig::Uniform {
            sigma_s_m: 0.005,
            eps_r: Some(15.0),
        })
        .unwrap();
        let dem = Dem::new(
            Box::new(Hills {
                base_m: 50.0,
                hills: vec![Hill {
                    lat_deg: 49.3,
                    lon_deg: 0.0,
                    height_m: 300.0,
                    radius_km: 5.0,
                }],
            }),
            0.0,
        );
        let mk = |sd: f64| {
            let mut p = Problem::new(vec![station()], vec![Geo::new(49.6, 0.0)], &map, K_LORAN);
            p.terrain = Some(TerrainSetup {
                dem: &dem,
                step_m: 1000.0,
                smooth_m: 1000.0,
                max_slope: 0.3,
            });
            Engine::new(
                p,
                UncertaintyConfig {
                    samples: 6,
                    terrain_sd_m: sd,
                    terrain_realisations: 3,
                    ..UncertaintyConfig::zero(6)
                },
            )
            .unwrap()
            .run()
            .unwrap()
        };
        let z = mk(0.0);
        // identical samples: zero up to floating-point rounding
        assert!(
            z.results[0].delay_ns.sd < 1e-9,
            "{}",
            z.results[0].delay_ns.sd
        );
        let n = mk(100.0);
        assert!(
            n.results[0].delay_ns.sd > 0.01,
            "{}",
            n.results[0].delay_ns.sd
        );
        assert_eq!(n.sensitivity[0].parameter, "terrain_height");
        assert!(Engine::new(
            Problem::new(vec![station()], points(), &map, K_LORAN),
            UncertaintyConfig {
                samples: 0,
                ..Default::default()
            }
        )
        .is_err());
    }
}
