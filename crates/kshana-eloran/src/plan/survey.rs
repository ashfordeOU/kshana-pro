// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Integrity-driven ASF survey planning (patent candidate F2; research 02 gap 31): where to
//! measure ASF so that the eLoran protection level over a service area falls fastest.
//!
//! Model:
//! * Each station's **residual ASF** over the area is a zero-mean Gaussian process with prior
//!   σ₀(x) = σ_floor + σ_scale·√(d/1000 km) (uncertainty grows with path length, as the
//!   ground-conductivity uncertainty integrates) and a squared-exponential correlation of
//!   length `corr_km`. Stations are independent.
//! * A **survey point** measures every station's ASF there with noise σ_m (a survey vehicle
//!   receives all stations). The posterior variance of station i at x given points S is
//!   σ₀² − kᵀ(K + σ_m² I)⁻¹k (exact GP conditioning).
//! * The **protection level** at x is k_PL · weighted HDOP with per-station range σ
//!   √(σ_toa² + σ_asf,i²(x)), k_PL from the integrity risk (normal quantile).
//! * **Objective**: the fraction of the area with PL ≤ target, then the mean PL. Points are
//!   chosen greedily from the candidates (survey routes, ports, roads or the grid itself).
//!
//! Evidence: **MODELLED**. The GP is a stated prior, not fitted; replace σ₀, `corr_km` and
//! σ_m with values from survey data (the ASF-grid and inverse-modelling modules estimate them).

use super::weighted_hdop_m;
use crate::geo::{inverse, Geo};
use serde::{Deserialize, Serialize};

/// Survey-planning settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurveyConfig {
    pub stations: Vec<(String, f64, f64)>,
    /// Service area box (lat_min, lat_max, lon_min, lon_max) and grid step (deg).
    pub area: [f64; 4],
    pub step_deg: f64,
    /// Candidate survey points (lat, lon); empty = the grid itself.
    pub candidates: Vec<(f64, f64)>,
    /// Number of survey points to choose.
    pub budget: usize,
    pub sigma_floor_m: f64,
    pub sigma_scale_m: f64,
    pub corr_km: f64,
    pub survey_noise_m: f64,
    pub toa_noise_m: f64,
    /// Protection multiplier (e.g. 5.33 for 1e-7 two-sided normal).
    pub k_pl: f64,
    pub target_pl_m: f64,
}

impl Default for SurveyConfig {
    fn default() -> Self {
        SurveyConfig {
            stations: vec![],
            area: [0.0; 4],
            step_deg: 0.25,
            candidates: vec![],
            budget: 10,
            sigma_floor_m: 15.0,
            sigma_scale_m: 60.0,
            corr_km: 80.0,
            survey_noise_m: 5.0,
            toa_noise_m: 10.0,
            k_pl: 5.33,
            target_pl_m: 300.0,
        }
    }
}

/// One chosen survey point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pick {
    pub order: usize,
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Area fraction meeting the target after this pick.
    pub coverage: f64,
    pub mean_pl_m: f64,
}

/// Planning result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurveyPlan {
    pub coverage_before: f64,
    pub mean_pl_before_m: f64,
    pub picks: Vec<Pick>,
    pub coverage_after: f64,
    pub mean_pl_after_m: f64,
    /// Grid: (lat, lon, PL before, PL after).
    pub grid: Vec<(f64, f64, f64, f64)>,
    pub evidence: String,
}

struct Model {
    st: Vec<Geo>,
    grid: Vec<Geo>,
    cfg: SurveyConfig,
}

fn km(a: Geo, b: Geo) -> f64 {
    inverse(a, b).distance_m / 1000.0
}

impl Model {
    fn sigma0(&self, s: usize, x: Geo) -> f64 {
        self.cfg.sigma_floor_m + self.cfg.sigma_scale_m * (km(self.st[s], x) / 1000.0).sqrt()
    }
    fn kern(&self, s: usize, a: Geo, b: Geo) -> f64 {
        let d = km(a, b);
        self.sigma0(s, a) * self.sigma0(s, b) * (-0.5 * (d / self.cfg.corr_km).powi(2)).exp()
    }
    /// Posterior variance of station `s` at every grid point given survey points `pts`.
    fn post_var(&self, s: usize, pts: &[Geo]) -> Vec<f64> {
        let n = pts.len();
        let prior: Vec<f64> = self
            .grid
            .iter()
            .map(|x| self.sigma0(s, *x).powi(2))
            .collect();
        if n == 0 {
            return prior;
        }
        // K + σm² I, Cholesky.
        let mut k = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..n {
                k[i][j] = self.kern(s, pts[i], pts[j])
                    + if i == j {
                        self.cfg.survey_noise_m.powi(2)
                    } else {
                        0.0
                    };
            }
        }
        let l = cholesky(&k);
        self.grid
            .iter()
            .zip(prior)
            .map(|(x, p)| {
                let kv: Vec<f64> = pts.iter().map(|q| self.kern(s, *x, *q)).collect();
                let z = forward(&l, &kv);
                (p - z.iter().map(|v| v * v).sum::<f64>()).max(0.0)
            })
            .collect()
    }
    fn pl(&self, pts: &[Geo]) -> Vec<f64> {
        let vars: Vec<Vec<f64>> = (0..self.st.len()).map(|s| self.post_var(s, pts)).collect();
        self.grid
            .iter()
            .enumerate()
            .map(|(g, x)| {
                let mut bear = vec![];
                let mut sig = vec![];
                for (s, st) in self.st.iter().enumerate() {
                    bear.push(inverse(*x, *st).az1_rad);
                    sig.push((self.cfg.toa_noise_m.powi(2) + vars[s][g]).sqrt());
                }
                weighted_hdop_m(&bear, &sig)
                    .map(|h| self.cfg.k_pl * h)
                    .unwrap_or(f64::INFINITY)
            })
            .collect()
    }
    fn score(&self, pl: &[f64]) -> (f64, f64) {
        let ok = pl.iter().filter(|p| **p <= self.cfg.target_pl_m).count() as f64 / pl.len() as f64;
        let fin: Vec<f64> = pl.iter().cloned().filter(|p| p.is_finite()).collect();
        (ok, fin.iter().sum::<f64>() / fin.len().max(1) as f64)
    }
}

fn cholesky(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                l[i][j] = (a[i][i] - s).max(1e-12).sqrt();
            } else {
                l[i][j] = (a[i][j] - s) / l[j][j];
            }
        }
    }
    l
}

fn forward(l: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut z = vec![0.0; n];
    for i in 0..n {
        let s: f64 = (0..i).map(|k| l[i][k] * z[k]).sum();
        z[i] = (b[i] - s) / l[i][i];
    }
    z
}

fn build(cfg: &SurveyConfig) -> Result<Model, String> {
    if cfg.stations.len() < 3 {
        return Err("survey planning needs at least three stations".into());
    }
    let [la0, la1, lo0, lo1] = cfg.area;
    if !(la1 > la0 && lo1 > lo0 && cfg.step_deg > 0.0) {
        return Err(
            "area must be lat_min < lat_max, lon_min < lon_max with a positive step".into(),
        );
    }
    let mut grid = vec![];
    let mut la = la0;
    while la <= la1 + 1e-9 {
        let mut lo = lo0;
        while lo <= lo1 + 1e-9 {
            grid.push(Geo::new(la, lo));
            lo += cfg.step_deg;
        }
        la += cfg.step_deg;
    }
    Ok(Model {
        st: cfg
            .stations
            .iter()
            .map(|(_, a, o)| Geo::new(*a, *o))
            .collect(),
        grid,
        cfg: cfg.clone(),
    })
}

/// Score a given set of survey points (for comparisons, e.g. against a planned route).
pub fn evaluate(cfg: &SurveyConfig, points: &[(f64, f64)]) -> Result<(f64, f64), String> {
    let m = build(cfg)?;
    let pts: Vec<Geo> = points.iter().map(|(a, o)| Geo::new(*a, *o)).collect();
    Ok(m.score(&m.pl(&pts)))
}

/// Choose `budget` survey points greedily.
pub fn plan(cfg: &SurveyConfig) -> Result<SurveyPlan, String> {
    let m = build(cfg)?;
    let cands: Vec<Geo> = if cfg.candidates.is_empty() {
        m.grid.clone()
    } else {
        cfg.candidates
            .iter()
            .map(|(a, o)| Geo::new(*a, *o))
            .collect()
    };
    let pl0 = m.pl(&[]);
    let (c0, mp0) = m.score(&pl0);
    let mut chosen: Vec<Geo> = vec![];
    let mut picks = vec![];
    for order in 0..cfg.budget.min(cands.len()) {
        let mut best: Option<(usize, (f64, f64))> = None;
        for (i, c) in cands.iter().enumerate() {
            if chosen.iter().any(|q| km(*q, *c) < 1e-3) {
                continue;
            }
            let mut trial = chosen.clone();
            trial.push(*c);
            let sc = m.score(&m.pl(&trial));
            let better = match best {
                None => true,
                Some((_, b)) => sc.0 > b.0 + 1e-12 || ((sc.0 - b.0).abs() <= 1e-12 && sc.1 < b.1),
            };
            if better {
                best = Some((i, sc));
            }
        }
        let Some((i, sc)) = best else { break };
        chosen.push(cands[i]);
        picks.push(Pick {
            order: order + 1,
            lat_deg: cands[i].lat_deg,
            lon_deg: cands[i].lon_deg,
            coverage: sc.0,
            mean_pl_m: sc.1,
        });
    }
    let pl1 = m.pl(&chosen);
    let (c1, mp1) = m.score(&pl1);
    Ok(SurveyPlan {
        coverage_before: c0,
        mean_pl_before_m: mp0,
        picks,
        coverage_after: c1,
        mean_pl_after_m: mp1,
        grid: m.grid.iter().zip(pl0.iter().zip(&pl1)).map(|(g, (a, b))| (g.lat_deg, g.lon_deg, *a, *b)).collect(),
        evidence: "MODELLED: Gaussian-process ASF prior (stated σ₀, correlation length, survey noise); greedy selection".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> SurveyConfig {
        SurveyConfig {
            stations: vec![
                ("A".into(), 57.5, 10.0),
                ("B".into(), 52.5, 8.5),
                ("C".into(), 54.5, 16.0),
            ],
            area: [54.0, 56.0, 10.0, 13.0],
            step_deg: 0.5,
            budget: 4,
            target_pl_m: 200.0,
            ..Default::default()
        }
    }

    #[test]
    fn surveys_reduce_protection_levels_and_greedy_beats_corner_points() {
        let c = cfg();
        let p = plan(&c).unwrap();
        assert!(p.coverage_after >= p.coverage_before);
        assert!(
            p.mean_pl_after_m < 0.8 * p.mean_pl_before_m,
            "{} -> {}",
            p.mean_pl_before_m,
            p.mean_pl_after_m
        );
        // Coverage never decreases as points are added.
        assert!(p
            .picks
            .windows(2)
            .all(|w| w[1].coverage + 1e-12 >= w[0].coverage));
        // Four points at the area's corners do worse than the planned four.
        let corners = [(54.0, 10.0), (54.0, 13.0), (56.0, 10.0), (56.0, 13.0)];
        let (cc, cm) = evaluate(&c, &corners).unwrap();
        assert!(
            p.coverage_after > cc || (p.coverage_after == cc && p.mean_pl_after_m < cm),
            "plan {:?} vs corners {cc} {cm}",
            (p.coverage_after, p.mean_pl_after_m)
        );
    }

    #[test]
    fn posterior_variance_falls_near_a_survey_point_only_within_the_correlation_length() {
        let c = SurveyConfig {
            corr_km: 30.0,
            ..cfg()
        };
        let m = build(&c).unwrap();
        let pt = Geo::new(55.0, 11.5);
        let before = m.post_var(0, &[]);
        let after = m.post_var(0, &[pt]);
        for (g, (b, a)) in m.grid.iter().zip(before.iter().zip(&after)) {
            let d = km(*g, pt);
            if d < 1.0 {
                assert!(*a < 0.1 * b, "at the point: {a} vs {b}");
            }
            if d > 150.0 {
                assert!(
                    (b - a) / b < 1e-3,
                    "far away ({d} km) unchanged: {a} vs {b}"
                );
            }
        }
    }
}
