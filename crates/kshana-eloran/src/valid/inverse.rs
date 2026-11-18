// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! D3 — inverse modelling: propagation and timing parameters estimated from recordings and
//! measurements, each with an uncertainty, and turned into updated configuration.
//!
//! # Estimators
//!
//! * **Per-station ASF** ([`estimate_asf`]) from ToA residuals at known positions over one
//!   or more sites and epochs. Observation model (ns), with the conventions of
//!   [`crate::rx::solve`]:
//!
//!   ```text
//!   pd = toa − nominal emission = PF(d) + C + e_tx + c_rx
//!   ```
//!
//!   `PF = η·d/c` (η = 1.000338), `C` the propagation correction beyond PF (SF + ASF in the
//!   RTCM decomposition — exactly what [`crate::rx::Corrections`] holds), `e_tx` the
//!   station's emission error (true − nominal emission: time-scale offset, transmitter
//!   timing) and `c_rx` the receiver clock error. `C` per (site, station) is the weighted
//!   mean of `pd − PF − e_tx − c_rx` over epochs; its standard error is the larger of the
//!   formal `1/√Σw` and the scatter `/√n`. **`e_tx` and `c_rx` must be known** (monitor-site
//!   data and a calibrated, UTC-traceable receiver clock): `C`, `e_tx` and `c_rx` are not
//!   separable from ToA alone. Optionally the seawater SF of the smooth-earth model is
//!   subtracted to report the ASF proper.
//! * **Transmitter timing offsets** ([`estimate_tx_timing`]) from multi-site data with the
//!   propagation correction `C` known per (site, station) (from a model or an earlier
//!   calibration): unknowns `e_tx` per station and, when site clocks are not known, `c_rx`
//!   per site — then one reference station's `e_tx` is held at 0 and the others are relative
//!   to it. Weighted least squares by Levenberg–Marquardt (the problem is linear, it
//!   converges in one step); covariance from the Jacobian.
//! * **Effective conductivity of a path segment** ([`fit_segment_conductivity`]): along one
//!   radial profile of known sections plus one section of unknown σ, measured ASF (relative
//!   to all-seawater, the crate's definition, see [`crate::prop::ground`]) at several
//!   distances is fitted with Millington's method over smooth-earth curves
//!   ([`crate::prop::millington`], [`crate::prop::smooth_earth`]) by a grid + golden-section
//!   search in `log10 σ`; the 1σ interval comes from the curvature of χ² (`Δχ² = 1`), inflated
//!   by the reduced χ² when it exceeds 1.
//! * **Skywave delay and ratio** ([`fit_skywave`]) from an averaged received pulse
//!   ([`super::fidelity::averaged_pulse`]): two-path model through the receiver's front end
//!   ([`crate::rx::track`] template `Q`),
//!
//!   ```text
//!   y(τ) = Im{W e^{jωτ} Q(τ − t_e)} + Im{W_s e^{jωτ} Q(τ − t_e − D)}
//!   ```
//!
//!   linear in `W`, `W_s` (variable projection on a (t_e, D) grid), then all six parameters
//!   by Levenberg–Marquardt. Ratio `|W_s/W|`, skywave carrier phase `arg(W_s/W) + ωD`
//!   (the `phase_deg` of the `fixed` skywave model).
//!
//! # Outputs
//!
//! [`corrections_for_site`] (a [`crate::rx::Corrections`] for the receiver),
//! [`asf_table_csv`], [`ground_with_fitted_sigma`] (an updated groundwave configuration),
//! [`SkywaveFit::to_sky_config`], and [`InverseSummary`] (JSON of everything).
//!
//! # Evidence
//!
//! MODELLED: the estimators are checked by recovering known synthetic parameters (tests);
//! the synthetic data come from this crate's own models, so the tests verify the
//! estimators, not the propagation physics.

use super::fidelity::AveragedPulse;
use super::stats;
use crate::chain::Chain;
use crate::geo::{inverse, Geo};
use crate::prop::conductivity::{eps_for_sigma, ConductivityConfig};
use crate::prop::ground::{GroundConfig, F0_HZ, N_ATM};
use crate::prop::millington::{millington, Section};
use crate::prop::sky::SkyConfig;
use crate::prop::smooth_earth::{Ground, GroundCurve, SEAWATER};
use crate::prop::C;
use crate::rx::dsp::{invert, solve};
use crate::rx::track::Template;
use crate::rx::{nominal_emission_s, Corrections, Measurement, C64};
use crate::time::AbsTime;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ───────────────────────────── numerics ─────────────────────────────

/// Options of [`levenberg_marquardt`].
#[derive(Clone, Debug, PartialEq)]
pub struct LmOptions {
    pub max_iter: usize,
    /// Initial damping (relative to diag(JᵀJ)).
    pub lambda0: f64,
    /// Plain Gauss–Newton (no damping, every step accepted).
    pub gauss_newton: bool,
    /// Convergence: every |δ_j| ≤ tol·(|x_j| + tol).
    pub tol: f64,
    /// Absolute finite-difference steps per parameter (empty: 1e-6·max(|x_j|, 1e-3)).
    pub fd_step: Vec<f64>,
}

impl Default for LmOptions {
    fn default() -> Self {
        LmOptions {
            max_iter: 100,
            lambda0: 1e-3,
            gauss_newton: false,
            tol: 1e-10,
            fd_step: Vec::new(),
        }
    }
}

/// Result of a least-squares fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LmResult {
    pub x: Vec<f64>,
    /// Standard errors from `s²(JᵀJ)⁻¹`, `s² = RSS/(m − n)` (a posteriori).
    pub std_err: Vec<f64>,
    /// Standard errors from `(JᵀJ)⁻¹` (residuals already weighted by their σ: a priori).
    pub std_err_apriori: Vec<f64>,
    /// A-posteriori covariance.
    pub cov: Vec<Vec<f64>>,
    pub rss: f64,
    pub dof: usize,
    /// `√(RSS/dof)`: the reduced χ^½ for weighted residuals.
    pub sigma0: f64,
    pub iterations: usize,
    pub converged: bool,
}

fn rss(r: &[f64]) -> f64 {
    r.iter().map(|x| x * x).sum()
}

fn jacobian<F: FnMut(&[f64]) -> Vec<f64>>(f: &mut F, x: &[f64], steps: &[f64], m: usize) -> Vec<Vec<f64>> {
    let n = x.len();
    let mut j = vec![vec![0.0; n]; m];
    for k in 0..n {
        let h = steps[k];
        let mut xp = x.to_vec();
        let mut xm = x.to_vec();
        xp[k] += h;
        xm[k] -= h;
        let (rp, rm) = (f(&xp), f(&xm));
        for i in 0..m {
            j[i][k] = (rp[i] - rm[i]) / (2.0 * h);
        }
    }
    j
}

fn normal(j: &[Vec<f64>], r: &[f64]) -> (Vec<Vec<f64>>, Vec<f64>) {
    let n = j.first().map(|x| x.len()).unwrap_or(0);
    let mut a = vec![vec![0.0; n]; n];
    let mut g = vec![0.0; n];
    for (row, ri) in j.iter().zip(r) {
        for p in 0..n {
            g[p] += row[p] * ri;
            for q in 0..n {
                a[p][q] += row[p] * row[q];
            }
        }
    }
    (a, g)
}

/// Minimise `Σ rᵢ(x)²` from `x0` (Levenberg–Marquardt with Marquardt's diagonal scaling, or
/// Gauss–Newton). `f` returns the residual vector (weight it by 1/σ for meaningful a-priori
/// errors). The Jacobian is by central differences.
pub fn levenberg_marquardt<F: FnMut(&[f64]) -> Vec<f64>>(
    mut f: F,
    x0: &[f64],
    o: &LmOptions,
) -> Result<LmResult, String> {
    let n = x0.len();
    let mut x = x0.to_vec();
    let mut r = f(&x);
    let m = r.len();
    if m < n || n == 0 {
        return Err(format!("least squares: {m} residuals for {n} parameters"));
    }
    if r.iter().any(|v| !v.is_finite()) {
        return Err("least squares: non-finite residual at the start point".into());
    }
    let steps = |x: &[f64]| -> Vec<f64> {
        (0..n)
            .map(|k| {
                o.fd_step
                    .get(k)
                    .copied()
                    .unwrap_or(1e-6 * x[k].abs().max(1e-3))
            })
            .collect()
    };
    let mut lambda = if o.gauss_newton { 0.0 } else { o.lambda0 };
    let mut converged = false;
    let mut it = 0;
    while it < o.max_iter {
        it += 1;
        let j = jacobian(&mut f, &x, &steps(&x), m);
        let (a, g) = normal(&j, &r);
        let cur = rss(&r);
        let mut accepted = None;
        for _ in 0..30 {
            let mut ad = a.clone();
            for k in 0..n {
                ad[k][k] += lambda * a[k][k].max(1e-300);
            }
            let ng: Vec<f64> = g.iter().map(|v| -v).collect();
            let d = match solve(&ad, &ng) {
                Some(d) => d,
                None if !o.gauss_newton => {
                    lambda = (lambda * 10.0).max(1e-6);
                    continue;
                }
                None => return Err("least squares: singular normal matrix".into()),
            };
            let xn: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + b).collect();
            let rn = f(&xn);
            let ok = rn.iter().all(|v| v.is_finite());
            if o.gauss_newton || (ok && rss(&rn) <= cur) {
                accepted = Some((xn, rn, d));
                lambda /= 10.0;
                break;
            }
            lambda = (lambda * 10.0).max(1e-6);
        }
        let Some((xn, rn, d)) = accepted else {
            converged = true; // no downhill step: at the minimum to working precision
            break;
        };
        let small = d
            .iter()
            .zip(&xn)
            .all(|(di, xi)| di.abs() <= o.tol * (xi.abs() + o.tol));
        let rel = (cur - rss(&rn)).abs() <= 1e-15 * cur.max(1e-300);
        x = xn;
        r = rn;
        if small || rel {
            converged = true;
            break;
        }
    }
    let j = jacobian(&mut f, &x, &steps(&x), m);
    let (a, _) = normal(&j, &r);
    let ca = invert(&a).ok_or("least squares: singular normal matrix at the solution")?;
    let s = rss(&r);
    let dof = m - n;
    let s2 = if dof > 0 { s / dof as f64 } else { f64::NAN };
    let cov: Vec<Vec<f64>> = ca
        .iter()
        .map(|row| row.iter().map(|v| v * s2).collect())
        .collect();
    Ok(LmResult {
        std_err: (0..n).map(|k| cov[k][k].max(0.0).sqrt()).collect(),
        std_err_apriori: (0..n).map(|k| ca[k][k].max(0.0).sqrt()).collect(),
        cov,
        x,
        rss: s,
        dof,
        sigma0: s2.sqrt(),
        iterations: it,
        converged,
    })
}

/// Result of a one-dimensional search.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Search1d {
    pub x: f64,
    pub fx: f64,
    pub evals: usize,
}

/// Golden-section minimisation of a unimodal `f` on `[a, b]` to interval width `tol`.
pub fn golden_section<F: FnMut(f64) -> f64>(mut f: F, a: f64, b: f64, tol: f64) -> Search1d {
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (a.min(b), a.max(b));
    let mut c = b - g * (b - a);
    let mut d = a + g * (b - a);
    let (mut fc, mut fd) = (f(c), f(d));
    let mut evals = 2;
    while b - a > tol {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - g * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + g * (b - a);
            fd = f(d);
        }
        evals += 1;
    }
    let x = 0.5 * (a + b);
    let fx = f(x);
    Search1d {
        x,
        fx,
        evals: evals + 1,
    }
}

/// Coarse grid of `n` points on `[a, b]`, then golden section around the best point
/// (guards against a non-unimodal objective at grid scale).
pub fn grid_golden<F: FnMut(f64) -> f64>(mut f: F, a: f64, b: f64, n: usize, tol: f64) -> Search1d {
    let n = n.max(3);
    let h = (b - a) / (n - 1) as f64;
    let mut best = (f64::INFINITY, 0usize);
    for i in 0..n {
        let v = f(a + i as f64 * h);
        if v < best.0 {
            best = (v, i);
        }
    }
    let lo = a + (best.1.saturating_sub(1)) as f64 * h;
    let hi = a + ((best.1 + 1).min(n - 1)) as f64 * h;
    let mut r = golden_section(&mut f, lo, hi, tol);
    r.evals += n;
    r
}

/// One-sigma half-width of the minimum of a χ² objective at `x` from its curvature
/// (`Δχ² = 1`): `√(2 / f'')`; NaN when the curvature is not positive.
pub fn curvature_sigma<F: FnMut(f64) -> f64>(mut f: F, x: f64, h: f64) -> f64 {
    let (fm, f0, fp) = (f(x - h), f(x), f(x + h));
    let c = (fp - 2.0 * f0 + fm) / (h * h);
    if c > 0.0 {
        (2.0 / c).sqrt()
    } else {
        f64::NAN
    }
}

/// A named estimate with its standard error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamEstimate {
    pub name: String,
    pub value: f64,
    pub std_err: f64,
}

// ───────────────────────────── ASF and timing ─────────────────────────────

/// One ToA observation at a known site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToaObservation {
    pub site: String,
    pub site_pos: Geo,
    pub chain: String,
    pub station: String,
    pub station_pos: Geo,
    /// Epoch (any consistent time scale, s), for the record.
    pub epoch_s: f64,
    /// `toa − nominal emission` (ns).
    pub pseudo_delay_ns: f64,
    /// Standard uncertainty of the observation (ns).
    pub sigma_ns: f64,
}

impl ToaObservation {
    /// "chain/station".
    pub fn key(&self) -> String {
        Corrections::key(&self.chain, &self.station)
    }
    /// Primary Factor delay (ns) over the geodesic.
    pub fn pf_ns(&self) -> f64 {
        N_ATM * inverse(self.station_pos, self.site_pos).distance_m / C * 1e9
    }
}

/// Observations from receiver measurements at a known site (`epoch_offset_s` + scene time
/// as the epoch; every observation gets `sigma_ns`).
pub fn observations_from_measurements(
    site: &str,
    site_pos: Geo,
    meas: &[Measurement],
    chains: &[Chain],
    start: AbsTime,
    epoch_offset_s: f64,
    sigma_ns: f64,
) -> Vec<ToaObservation> {
    meas.iter()
        .map(|m| {
            let ch = &chains[m.chain_index];
            ToaObservation {
                site: site.into(),
                site_pos,
                chain: m.chain.clone(),
                station: m.station.clone(),
                station_pos: ch.stations[m.station_index].position,
                epoch_s: epoch_offset_s + m.toa_s,
                pseudo_delay_ns: (m.toa_s
                    - nominal_emission_s(ch, m.station_index, m.group, start))
                    * 1e9,
                sigma_ns,
            }
        })
        .collect()
}

/// Known timing for [`estimate_asf`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AsfConfig {
    /// Receiver clock error per site (ns, receiver clock − truth/UTC). Required per site.
    pub site_clock_ns: BTreeMap<String, f64>,
    /// Emission error per station (ns, true − nominal emission), key "chain/station";
    /// missing stations are taken as 0 (an ideally UTC-steered transmitter).
    pub tx_offset_ns: BTreeMap<String, f64>,
    /// Also report ASF proper = correction − smooth-earth seawater SF (crate's model).
    #[serde(default)]
    pub subtract_seawater_sf: bool,
}

/// The propagation correction beyond PF of one station at one site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsfEstimate {
    pub site: String,
    pub site_pos: Geo,
    pub chain: String,
    pub station: String,
    /// Correction beyond PF (SF + ASF), ns: what [`Corrections`] holds.
    pub beyond_pf_ns: f64,
    pub std_err_ns: f64,
    /// Weighted scatter of the epochs (ns).
    pub scatter_ns: f64,
    pub n: usize,
    /// Seawater SF of the smooth-earth model and ASF = beyond_pf − SF (ns), if requested.
    pub sf_ns: Option<f64>,
    pub asf_ns: Option<f64>,
}

/// Seawater Secondary Factor (ns) of the crate's smooth-earth model (k = `K_LORAN`),
/// including the induction terms, at distance `d_m`.
pub fn seawater_sf_ns(d_m: f64) -> f64 {
    let gw = crate::prop::ground::Groundwave::new(
        crate::prop::conductivity::ConductivityMap::build(&ConductivityConfig::Uniform {
            sigma_s_m: 5.0,
            eps_r: Some(80.0),
        })
        .expect("seawater map"),
        crate::prop::ground::K_LORAN,
        2.0,
        0.005,
        None,
        "seawater",
    )
    .expect("seawater groundwave");
    let tx = Geo::new(0.0, 0.0);
    let rx = crate::geo::direct(tx, 0.0, d_m);
    gw.evaluate(tx, 1.0, rx, 0.0).sf_ns
}

/// Per (site, station) propagation correction beyond PF (see the module docs).
pub fn estimate_asf(obs: &[ToaObservation], cfg: &AsfConfig) -> Result<Vec<AsfEstimate>, String> {
    let mut groups: BTreeMap<(String, String), Vec<&ToaObservation>> = BTreeMap::new();
    for o in obs {
        if !(o.sigma_ns > 0.0) {
            return Err(format!("observation {}/{}: sigma_ns must be positive", o.site, o.key()));
        }
        groups.entry((o.site.clone(), o.key())).or_default().push(o);
    }
    let mut out = Vec::new();
    for ((site, key), v) in groups {
        let clk = *cfg.site_clock_ns.get(&site).ok_or_else(|| {
            format!(
                "site {site}: receiver clock offset unknown (ASF, transmitter timing and receiver \
                 clock are not separable from ToA alone; supply AsfConfig::site_clock_ns)"
            )
        })?;
        let tx = cfg.tx_offset_ns.get(&key).copied().unwrap_or(0.0);
        let vals: Vec<(f64, f64)> = v
            .iter()
            .map(|o| (o.pseudo_delay_ns - o.pf_ns() - tx - clk, 1.0 / (o.sigma_ns * o.sigma_ns)))
            .collect();
        let sw: f64 = vals.iter().map(|x| x.1).sum();
        let mean = vals.iter().map(|x| x.0 * x.1).sum::<f64>() / sw;
        let n = vals.len();
        let scatter = if n > 1 {
            (vals.iter().map(|x| x.1 * (x.0 - mean).powi(2)).sum::<f64>() / sw * n as f64
                / (n - 1) as f64)
                .sqrt()
        } else {
            0.0
        };
        let se = (1.0 / sw).sqrt().max(scatter / (n as f64).sqrt());
        let o0 = v[0];
        let sf = if cfg.subtract_seawater_sf {
            Some(seawater_sf_ns(inverse(o0.station_pos, o0.site_pos).distance_m))
        } else {
            None
        };
        out.push(AsfEstimate {
            site,
            site_pos: o0.site_pos,
            chain: o0.chain.clone(),
            station: o0.station.clone(),
            beyond_pf_ns: mean,
            std_err_ns: se,
            scatter_ns: scatter,
            n,
            sf_ns: sf,
            asf_ns: sf.map(|s| mean - s),
        });
    }
    Ok(out)
}

/// The receiver corrections (beyond PF) of one site.
pub fn corrections_for_site(est: &[AsfEstimate], site: &str) -> Corrections {
    Corrections {
        asf_ns: est
            .iter()
            .filter(|e| e.site == site)
            .map(|e| (Corrections::key(&e.chain, &e.station), e.beyond_pf_ns))
            .collect(),
    }
}

/// ASF correction table (CSV).
pub fn asf_table_csv(est: &[AsfEstimate]) -> String {
    let mut s = String::from(
        "site,lat_deg,lon_deg,chain,station,beyond_pf_ns,std_err_ns,scatter_ns,n,sf_ns,asf_ns\n",
    );
    let o = |v: Option<f64>| v.map(|x| format!("{x:.3}")).unwrap_or_default();
    for e in est {
        s.push_str(&format!(
            "{},{:.8},{:.8},{},{},{:.3},{:.3},{:.3},{},{},{}\n",
            e.site,
            e.site_pos.lat_deg,
            e.site_pos.lon_deg,
            e.chain,
            e.station,
            e.beyond_pf_ns,
            e.std_err_ns,
            e.scatter_ns,
            e.n,
            o(e.sf_ns),
            o(e.asf_ns)
        ));
    }
    s
}

/// Key of a (site, station) propagation correction: `"site|chain/station"`.
pub fn site_station_key(site: &str, chain: &str, station: &str) -> String {
    format!("{site}|{chain}/{station}")
}

/// Inputs of [`estimate_tx_timing`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimingConfig {
    /// Propagation correction beyond PF per (site, station), key [`site_station_key`].
    pub correction_ns: BTreeMap<String, f64>,
    /// Known receiver clocks per site (ns); `None` estimates one per site.
    pub site_clock_ns: Option<BTreeMap<String, f64>>,
    /// Station ("chain/station") held at e_tx = 0 when the site clocks are estimated
    /// (default: the first station in sort order).
    pub reference_station: Option<String>,
}

/// Transmitter timing estimate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimingEstimate {
    /// Emission error per station (ns, true − nominal); relative to the reference station
    /// when the site clocks were estimated.
    pub tx_offset_ns: Vec<ParamEstimate>,
    pub site_clock_ns: Vec<ParamEstimate>,
    pub reference_station: Option<String>,
    pub residual_rms_ns: f64,
    /// Reduced χ² of the weighted fit.
    pub reduced_chi2: f64,
    pub n_obs: usize,
    pub iterations: usize,
}

/// Transmitter timing offsets (and site clocks) from multi-site data (see the module docs).
pub fn estimate_tx_timing(obs: &[ToaObservation], cfg: &TimingConfig) -> Result<TimingEstimate, String> {
    let mut stations: Vec<String> = obs.iter().map(|o| o.key()).collect();
    stations.sort();
    stations.dedup();
    let mut sites: Vec<String> = obs.iter().map(|o| o.site.clone()).collect();
    sites.sort();
    sites.dedup();
    let est_clocks = cfg.site_clock_ns.is_none();
    let reference = if est_clocks {
        let r = cfg
            .reference_station
            .clone()
            .unwrap_or_else(|| stations[0].clone());
        if !stations.contains(&r) {
            return Err(format!("reference station {r} has no observations"));
        }
        Some(r)
    } else {
        None
    };
    let tx_free: Vec<&String> = stations
        .iter()
        .filter(|s| Some(*s) != reference.as_ref())
        .collect();
    // fixed part of each observation: pd − PF − correction (− known clock)
    let mut rows = Vec::with_capacity(obs.len());
    for o in obs {
        let k = site_station_key(&o.site, &o.chain, &o.station);
        let c = *cfg
            .correction_ns
            .get(&k)
            .ok_or_else(|| format!("no propagation correction for {k}"))?;
        let clk = match &cfg.site_clock_ns {
            Some(m) => *m
                .get(&o.site)
                .ok_or_else(|| format!("no clock for site {}", o.site))?,
            None => 0.0,
        };
        let tx_i = tx_free.iter().position(|s| **s == o.key());
        let site_i = sites.iter().position(|s| *s == o.site).unwrap();
        rows.push((o.pseudo_delay_ns - o.pf_ns() - c - clk, o.sigma_ns, tx_i, site_i));
    }
    let n_tx = tx_free.len();
    let n_par = n_tx + if est_clocks { sites.len() } else { 0 };
    let resid = |x: &[f64]| -> Vec<f64> {
        rows.iter()
            .map(|(y, s, ti, si)| {
                let mut m = 0.0;
                if let Some(t) = ti {
                    m += x[*t];
                }
                if est_clocks {
                    m += x[n_tx + si];
                }
                (y - m) / s
            })
            .collect()
    };
    let lm = levenberg_marquardt(
        resid,
        &vec![0.0; n_par],
        &LmOptions {
            fd_step: vec![1.0; n_par],
            ..Default::default()
        },
    )?;
    let raw: Vec<f64> = {
        let r = |x: &[f64]| -> Vec<f64> {
            rows.iter()
                .map(|(y, _, ti, si)| {
                    let mut m = 0.0;
                    if let Some(t) = ti {
                        m += x[*t];
                    }
                    if est_clocks {
                        m += x[n_tx + si];
                    }
                    y - m
                })
                .collect()
        };
        r(&lm.x)
    };
    // a-priori errors (σ given) inflated by the reduced χ when it exceeds 1
    let infl = lm.sigma0.max(1.0);
    let mut tx_offset_ns: Vec<ParamEstimate> = tx_free
        .iter()
        .enumerate()
        .map(|(i, s)| ParamEstimate {
            name: (*s).clone(),
            value: lm.x[i],
            std_err: lm.std_err_apriori[i] * infl,
        })
        .collect();
    if let Some(r) = &reference {
        tx_offset_ns.push(ParamEstimate {
            name: r.clone(),
            value: 0.0,
            std_err: 0.0,
        });
        tx_offset_ns.sort_by(|a, b| a.name.cmp(&b.name));
    }
    let site_clock_ns = if est_clocks {
        sites
            .iter()
            .enumerate()
            .map(|(i, s)| ParamEstimate {
                name: s.clone(),
                value: lm.x[n_tx + i],
                std_err: lm.std_err_apriori[n_tx + i] * infl,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(TimingEstimate {
        tx_offset_ns,
        site_clock_ns,
        reference_station: reference,
        residual_rms_ns: stats(&raw).rms,
        reduced_chi2: lm.sigma0 * lm.sigma0,
        n_obs: rows.len(),
        iterations: lm.iterations,
    })
}

// ───────────────────────────── conductivity ─────────────────────────────

/// Ground of a profile section.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SegmentGround {
    Known {
        sigma_s_m: f64,
        #[serde(default)]
        eps_r: Option<f64>,
    },
    /// The segment whose σ is estimated (ε_r from σ by default, ITU-R P.527 categories).
    Unknown {
        #[serde(default)]
        eps_r: Option<f64>,
    },
}

/// One section of a radial profile, ending `end_km` from the transmitter.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileSection {
    pub ground: SegmentGround,
    pub end_km: f64,
}

/// A radial profile from a transmitter: contiguous sections (the last one extends to any
/// farther point), exactly one of them `Unknown`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathProfile {
    pub sections: Vec<ProfileSection>,
    pub k_factor: f64,
}

/// A measured ASF (relative to all-seawater, ns) at a distance along the profile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsfPoint {
    pub distance_km: f64,
    pub asf_ns: f64,
    pub sigma_ns: f64,
}

/// Options of [`fit_segment_conductivity`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConductivityFitOptions {
    pub sigma_min_s_m: f64,
    pub sigma_max_s_m: f64,
    pub n_grid: usize,
    /// Golden-section tolerance in log10 σ.
    pub tol_log10: f64,
    /// Curve table intervals (in √d) of each ground.
    pub curve_n: usize,
}

impl Default for ConductivityFitOptions {
    fn default() -> Self {
        ConductivityFitOptions {
            sigma_min_s_m: 1e-4,
            sigma_max_s_m: 0.1,
            n_grid: 13,
            tol_log10: 1e-3,
            curve_n: 400,
        }
    }
}

/// Result of a conductivity fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConductivityFit {
    pub sigma_s_m: f64,
    pub log10_sigma: f64,
    /// 1σ in log10 σ (curvature of χ², inflated by √(reduced χ²) when > 1).
    pub log10_std: f64,
    /// 1σ interval of σ.
    pub sigma_lo_s_m: f64,
    pub sigma_hi_s_m: f64,
    pub chi2: f64,
    pub reduced_chi2: f64,
    pub n: usize,
    /// Measured − model (ns) at the solution.
    pub residuals_ns: Vec<f64>,
    pub evals: usize,
    /// The estimate sits at a search bound (the data do not constrain σ there).
    pub at_bound: bool,
}

struct ProfileModel {
    secs: Vec<(Option<Ground>, f64)>,
    known: Vec<Option<GroundCurve>>,
    sea: GroundCurve,
    k: f64,
    d_max: f64,
    n: usize,
}

impl ProfileModel {
    fn new(p: &PathProfile, d_max_km: f64, n: usize) -> Result<ProfileModel, String> {
        if p.sections.is_empty() {
            return Err("profile: no sections".into());
        }
        if p
            .sections
            .iter()
            .filter(|s| matches!(s.ground, SegmentGround::Unknown { .. }))
            .count()
            != 1
        {
            return Err("profile: exactly one section must be Unknown".into());
        }
        if p.sections.windows(2).any(|w| w[1].end_km <= w[0].end_km) || p.sections[0].end_km <= 0.0 {
            return Err("profile: section ends must be positive and increasing".into());
        }
        let d_max = (d_max_km.max(p.sections.last().unwrap().end_km) * 1.02 + 1.0) * 1000.0;
        let secs: Vec<(Option<Ground>, f64)> = p
            .sections
            .iter()
            .map(|s| {
                let g = match s.ground {
                    SegmentGround::Known { sigma_s_m, eps_r } => Some(Ground {
                        sigma: sigma_s_m,
                        eps_r: eps_r.unwrap_or_else(|| eps_for_sigma(sigma_s_m)),
                    }),
                    SegmentGround::Unknown { .. } => None,
                };
                (g, s.end_km * 1000.0)
            })
            .collect();
        let known = secs
            .iter()
            .map(|(g, _)| g.map(|g| GroundCurve::new(g, F0_HZ, p.k_factor, N_ATM, d_max, n)))
            .collect();
        Ok(ProfileModel {
            secs,
            known,
            sea: GroundCurve::new(SEAWATER, F0_HZ, p.k_factor, N_ATM, d_max, n),
            k: p.k_factor,
            d_max,
            n,
        })
    }

    fn asf_ns(&self, unknown: Ground, d_km: &[f64]) -> Vec<f64> {
        let uc = GroundCurve::new(unknown, F0_HZ, self.k, N_ATM, self.d_max, self.n);
        d_km.iter()
            .map(|&dk| {
                let d = dk * 1000.0;
                let mut secs = Vec::new();
                for (i, (_, end)) in self.secs.iter().enumerate() {
                    let last = i + 1 == self.secs.len();
                    let e = if last { d } else { end.min(d) };
                    secs.push(Section { ground: i, end_m: e });
                    if *end >= d {
                        break;
                    }
                }
                let m = millington(&secs, |g, x| match &self.known[g] {
                    Some(c) => c.at(x),
                    None => uc.at(x),
                });
                (m.phase_delay_s - self.sea.at(d).phase_delay_s) * 1e9
            })
            .collect()
    }
}

/// Model ASF (ns, Millington − seawater) along `profile` for an unknown-section σ.
pub fn profile_asf_ns(profile: &PathProfile, sigma_s_m: f64, d_km: &[f64], curve_n: usize) -> Result<Vec<f64>, String> {
    let dmax = d_km.iter().fold(0.0f64, |a, b| a.max(*b));
    let m = ProfileModel::new(profile, dmax, curve_n)?;
    Ok(m.asf_ns(unknown_ground(profile, sigma_s_m), d_km))
}

fn unknown_ground(p: &PathProfile, sigma: f64) -> Ground {
    let eps = p
        .sections
        .iter()
        .find_map(|s| match s.ground {
            SegmentGround::Unknown { eps_r } => Some(eps_r),
            _ => None,
        })
        .flatten();
    Ground {
        sigma,
        eps_r: eps.unwrap_or_else(|| eps_for_sigma(sigma)),
    }
}

/// Effective conductivity of the profile's unknown section (see the module docs).
pub fn fit_segment_conductivity(
    profile: &PathProfile,
    points: &[AsfPoint],
    opts: &ConductivityFitOptions,
) -> Result<ConductivityFit, String> {
    if points.len() < 2 {
        return Err("conductivity fit: need at least 2 ASF points".into());
    }
    if points.iter().any(|p| !(p.sigma_ns > 0.0) || !(p.distance_km > 0.0)) {
        return Err("conductivity fit: distances and sigma_ns must be positive".into());
    }
    if !(opts.sigma_min_s_m > 0.0 && opts.sigma_max_s_m > opts.sigma_min_s_m) {
        return Err("conductivity fit: need 0 < sigma_min < sigma_max".into());
    }
    let d: Vec<f64> = points.iter().map(|p| p.distance_km).collect();
    let dmax = d.iter().fold(0.0f64, |a, b| a.max(*b));
    let model = ProfileModel::new(profile, dmax, opts.curve_n)?;
    let mut evals = 0usize;
    let mut chi2 = |ls: f64| -> f64 {
        evals += 1;
        let a = model.asf_ns(unknown_ground(profile, 10f64.powf(ls)), &d);
        points
            .iter()
            .zip(&a)
            .map(|(p, m)| ((p.asf_ns - m) / p.sigma_ns).powi(2))
            .sum()
    };
    let (lo, hi) = (opts.sigma_min_s_m.log10(), opts.sigma_max_s_m.log10());
    let r = grid_golden(&mut chi2, lo, hi, opts.n_grid, opts.tol_log10);
    let n = points.len();
    let red = if n > 1 { r.fx / (n - 1) as f64 } else { f64::NAN };
    let h = 0.02;
    let s = curvature_sigma(&mut chi2, r.x, h) * red.max(1.0).sqrt();
    let a = model.asf_ns(unknown_ground(profile, 10f64.powf(r.x)), &d);
    let at_bound = (r.x - lo).abs() < 2.0 * opts.tol_log10 || (hi - r.x).abs() < 2.0 * opts.tol_log10;
    Ok(ConductivityFit {
        sigma_s_m: 10f64.powf(r.x),
        log10_sigma: r.x,
        log10_std: s,
        sigma_lo_s_m: 10f64.powf(r.x - s),
        sigma_hi_s_m: 10f64.powf(r.x + s),
        chi2: r.fx,
        reduced_chi2: red,
        n,
        residuals_ns: points.iter().zip(&a).map(|(p, m)| p.asf_ns - m).collect(),
        evals: evals + r.evals,
        at_bound,
    })
}

/// An updated groundwave configuration with a fitted σ: for `smooth_earth` the uniform
/// conductivity; for `millington` over polygons the polygon named `polygon` (its ε_r reset
/// to the default from σ unless it had one); a uniform map's σ. Rasters are refused (fit
/// per cell is not what a 1-D profile gives).
pub fn ground_with_fitted_sigma(cfg: &GroundConfig, polygon: Option<&str>, sigma_s_m: f64) -> Result<GroundConfig, String> {
    if !(sigma_s_m > 0.0 && sigma_s_m.is_finite()) {
        return Err("fitted sigma must be positive".into());
    }
    let mut out = cfg.clone();
    match &mut out {
        GroundConfig::Simple { .. } => {
            return Err("the simple groundwave model has no conductivity".into())
        }
        GroundConfig::SmoothEarth { sigma_s_m: s, .. } => *s = sigma_s_m,
        GroundConfig::Millington { conductivity, .. } => match conductivity {
            ConductivityConfig::Uniform { sigma_s_m: s, .. } => *s = sigma_s_m,
            ConductivityConfig::Polygons { polygons, .. } => {
                let name = polygon.ok_or("polygon map: name the polygon to update")?;
                let p = polygons
                    .iter_mut()
                    .find(|p| p.name == name)
                    .ok_or_else(|| format!("no polygon named {name:?}"))?;
                p.sigma_s_m = sigma_s_m;
            }
            _ => return Err("raster maps are not updated from a profile fit".into()),
        },
    }
    Ok(out)
}

// ───────────────────────────── skywave ─────────────────────────────

/// Options of [`fit_skywave`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkywaveFitOptions {
    /// Skywave delay search range and grid step (µs).
    pub delay_range_us: (f64, f64),
    pub delay_step_us: f64,
    /// Groundwave envelope origin search range relative to the reference ToA and step (µs).
    pub t_env_range_us: (f64, f64),
    pub t_env_step_us: f64,
    /// White-noise power gain `Σh²` of the front end the pulse was averaged through
    /// (`Receiver::fir.noise_gain`). Band-passed noise is correlated over ≈ 1/`Σh²`
    /// samples, so the least-squares standard errors (which assume independent samples)
    /// are inflated by `1/√Σh²`. `None`: no inflation (independent samples assumed).
    #[serde(default)]
    pub noise_gain: Option<f64>,
}

impl Default for SkywaveFitOptions {
    fn default() -> Self {
        SkywaveFitOptions {
            delay_range_us: (20.0, 250.0),
            delay_step_us: 2.0,
            t_env_range_us: (-15.0, 15.0),
            t_env_step_us: 1.0,
            noise_gain: None,
        }
    }
}

/// Result of a two-path fit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkywaveFit {
    pub chain: String,
    pub station: String,
    /// Skywave delay after the groundwave (µs) and its standard error.
    pub delay_us: f64,
    pub delay_std_us: f64,
    /// Sky/ground amplitude ratio (dB) and its standard error.
    pub ratio_db: f64,
    pub ratio_std_db: f64,
    /// Skywave carrier phase relative to the groundwave (deg, (−180, 180]).
    pub sky_phase_deg: f64,
    /// Groundwave envelope origin relative to the reference carrier ToA (µs).
    pub t_env_us: f64,
    pub t_env_std_us: f64,
    /// Groundwave peak amplitude (input units of the averaged pulse).
    pub ground_amplitude: f64,
    /// RMS residual (input units) and RMS of the data.
    pub rms_residual: f64,
    pub rms_data: f64,
    pub iterations: usize,
    pub converged: bool,
}

impl SkywaveFit {
    /// A `fixed` skywave configuration reproducing the fit.
    pub fn to_sky_config(&self) -> SkyConfig {
        SkyConfig::Fixed {
            delay_us: self.delay_us,
            ratio_db: self.ratio_db,
            phase_deg: self.sky_phase_deg,
        }
    }
}

fn wrap_deg(x: f64) -> f64 {
    let mut y = x.rem_euclid(360.0);
    if y > 180.0 {
        y -= 360.0;
    }
    y
}

/// Two-path (groundwave + one skywave) fit of an averaged pulse (see the module docs).
/// `tpl` is the receiver template of the station's chain (`Receiver::templates`).
pub fn fit_skywave(pulse: &AveragedPulse, tpl: &Template, opts: &SkywaveFitOptions) -> Result<SkywaveFit, String> {
    let m = pulse.samples.len();
    if m < 16 {
        return Err("skywave fit: averaged pulse too short".into());
    }
    let w = tpl.omega;
    let tau: Vec<f64> = (0..m).map(|j| pulse.tau(j)).collect();
    let ph: Vec<C64> = tau.iter().map(|t| C64::cis(w * t)).collect();
    let y = &pulse.samples;
    let yy: f64 = y.iter().map(|v| v * v).sum();
    // basis for (te, D) in seconds
    let basis = |te: f64, d: f64| -> [Vec<f64>; 4] {
        let mut b: [Vec<f64>; 4] = Default::default();
        for v in b.iter_mut() {
            v.reserve(m);
        }
        for j in 0..m {
            let z1 = ph[j] * tpl.at(tau[j] - te);
            let z2 = ph[j] * tpl.at(tau[j] - te - d);
            b[0].push(z1.im);
            b[1].push(z1.re);
            b[2].push(z2.im);
            b[3].push(z2.re);
        }
        b
    };
    let vp = |te: f64, d: f64| -> Option<(f64, Vec<f64>)> {
        let b = basis(te, d);
        let mut a = vec![vec![0.0; 4]; 4];
        let mut r = vec![0.0; 4];
        for p in 0..4 {
            r[p] = b[p].iter().zip(y).map(|(x, v)| x * v).sum();
            for q in p..4 {
                let s: f64 = b[p].iter().zip(&b[q]).map(|(x, z)| x * z).sum();
                a[p][q] = s;
                a[q][p] = s;
            }
        }
        let c = solve(&a, &r)?;
        let expl: f64 = c.iter().zip(&r).map(|(x, z)| x * z).sum();
        Some((yy - expl, c))
    };
    let mut best: Option<(f64, f64, f64, Vec<f64>)> = None;
    let nt = ((opts.t_env_range_us.1 - opts.t_env_range_us.0) / opts.t_env_step_us).round() as usize;
    let nd = ((opts.delay_range_us.1 - opts.delay_range_us.0) / opts.delay_step_us).round() as usize;
    for it in 0..=nt {
        let te = (opts.t_env_range_us.0 + it as f64 * opts.t_env_step_us) * 1e-6;
        for id in 0..=nd {
            let d = (opts.delay_range_us.0 + id as f64 * opts.delay_step_us) * 1e-6;
            if let Some((r, c)) = vp(te, d) {
                if best.as_ref().map(|b| r < b.0).unwrap_or(true) {
                    best = Some((r, te, d, c));
                }
            }
        }
    }
    let (_, te0, d0, c0) = best.ok_or("skywave fit: no solvable grid point")?;
    // full nonlinear refinement: x = (te µs, D µs, Re W, Im W, Re Ws, Im Ws)
    let amp = c0.iter().fold(0.0f64, |a, b| a.max(b.abs())).max(1e-30);
    let model = |x: &[f64]| -> Vec<f64> {
        let (te, d) = (x[0] * 1e-6, x[1] * 1e-6);
        (0..m)
            .map(|j| {
                let z1 = ph[j] * tpl.at(tau[j] - te);
                let z2 = ph[j] * tpl.at(tau[j] - te - d);
                y[j] - (x[2] * z1.im + x[3] * z1.re + x[4] * z2.im + x[5] * z2.re)
            })
            .collect()
    };
    let x0 = [te0 * 1e6, d0 * 1e6, c0[0], c0[1], c0[2], c0[3]];
    let lm = levenberg_marquardt(
        model,
        &x0,
        &LmOptions {
            fd_step: vec![1e-4, 1e-4, 1e-6 * amp, 1e-6 * amp, 1e-6 * amp, 1e-6 * amp],
            tol: 1e-12,
            ..Default::default()
        },
    )?;
    let x = &lm.x;
    let wg = C64::new(x[2], x[3]);
    let ws = C64::new(x[4], x[5]);
    let (ag, as_) = (wg.norm_sqr(), ws.norm_sqr());
    let k = 20.0 / std::f64::consts::LN_10;
    let grad = [0.0, 0.0, -k * x[2] / ag, -k * x[3] / ag, k * x[4] / as_, k * x[5] / as_];
    let infl = match opts.noise_gain {
        Some(g) if g > 0.0 && g <= 1.0 => 1.0 / g.sqrt(),
        _ => 1.0,
    };
    let mut var = 0.0;
    for p in 0..6 {
        for q in 0..6 {
            var += grad[p] * lm.cov[p][q] * grad[q];
        }
    }
    let rel = ws * wg.conj();
    let q_peak = tpl
        .q
        .iter()
        .fold(0.0f64, |a, q| a.max(q.abs()));
    Ok(SkywaveFit {
        chain: pulse.chain.clone(),
        station: pulse.station.clone(),
        delay_us: x[1],
        delay_std_us: lm.std_err[1] * infl,
        ratio_db: 10.0 * (as_ / ag).log10(),
        ratio_std_db: var.max(0.0).sqrt() * infl,
        sky_phase_deg: wrap_deg((rel.arg() + w * x[1] * 1e-6).to_degrees()),
        t_env_us: x[0],
        t_env_std_us: lm.std_err[0] * infl,
        ground_amplitude: ag.sqrt() * q_peak,
        rms_residual: (lm.rss / m as f64).sqrt(),
        rms_data: (yy / m as f64).sqrt(),
        iterations: lm.iterations,
        converged: lm.converged,
    })
}

/// Everything estimated, for a JSON record next to the updated configuration.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InverseSummary {
    pub asf: Vec<AsfEstimate>,
    pub timing: Option<TimingEstimate>,
    /// (profile / segment name, fit).
    pub conductivity: Vec<(String, ConductivityFit)>,
    pub skywave: Vec<SkywaveFit>,
}

impl InverseSummary {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rx::{self, RxConfig, RxInput};
    use crate::scenario::Scenario;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn gauss(r: &mut ChaCha8Rng) -> f64 {
        crate::timebase::gauss(r)
    }

    #[test]
    fn lm_and_gauss_newton_recover_an_exponential() {
        let (a, k) = (3.0, 0.7);
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let t: Vec<f64> = (0..60).map(|i| i as f64 * 0.1).collect();
        let s = 0.01;
        let y: Vec<f64> = t.iter().map(|t| a * (-k * t).exp() + s * gauss(&mut rng)).collect();
        for gn in [false, true] {
            let r = levenberg_marquardt(
                |x| t.iter().zip(&y).map(|(t, y)| (y - x[0] * (-x[1] * t).exp()) / s).collect(),
                &[2.0, 0.5],
                &LmOptions { gauss_newton: gn, ..Default::default() },
            )
            .unwrap();
            assert!(r.converged);
            assert!((r.x[0] - a).abs() < 3.0 * r.std_err[0], "{r:?}");
            assert!((r.x[1] - k).abs() < 3.0 * r.std_err[1], "{r:?}");
            assert!((r.sigma0 - 1.0).abs() < 0.3);
        }
        let g = golden_section(|x| (x - 1.234).powi(2), -5.0, 5.0, 1e-9);
        assert!((g.x - 1.234).abs() < 1e-8);
        let g = grid_golden(|x| (x * 3.0).cos() + 0.1 * x * x, -3.0, 3.0, 25, 1e-9);
        assert!(g.fx < -0.85 && (g.x.abs() - 1.03).abs() < 0.05, "{g:?}");
        assert!((curvature_sigma(|x| (x - 2.0).powi(2) / 0.25, 2.0, 1e-3) - 0.5).abs() < 1e-6);
    }

    fn sites() -> Vec<(&'static str, Geo)> {
        vec![("S1", Geo::new(55.0, 3.0)), ("S2", Geo::new(54.2, 4.5)), ("S3", Geo::new(56.0, 4.0))]
    }
    fn stations() -> Vec<(&'static str, Geo)> {
        vec![
            ("M", Geo::new(56.5, 1.0)),
            ("X", Geo::new(53.5, 1.5)),
            ("Y", Geo::new(55.5, 6.5)),
            ("Z", Geo::new(53.8, 5.5)),
        ]
    }

    /// Synthetic observations: correction c(site, station), tx offsets, site clocks, noise.
    fn synth(
        corr: &dyn Fn(usize, usize) -> f64,
        tx: &[f64],
        clk: &[f64],
        sigma: f64,
        epochs: usize,
    ) -> Vec<ToaObservation> {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let mut v = Vec::new();
        for (i, (s, sp)) in sites().into_iter().enumerate() {
            for (j, (st, tp)) in stations().into_iter().enumerate() {
                for e in 0..epochs {
                    let pf = N_ATM * inverse(tp, sp).distance_m / C * 1e9;
                    v.push(ToaObservation {
                        site: s.into(),
                        site_pos: sp,
                        chain: "c".into(),
                        station: st.into(),
                        station_pos: tp,
                        epoch_s: e as f64,
                        pseudo_delay_ns: pf + corr(i, j) + tx[j] + clk[i] + sigma * gauss(&mut rng),
                        sigma_ns: sigma,
                    });
                }
            }
        }
        v
    }

    #[test]
    fn asf_per_site_and_station_is_recovered() {
        let corr = |i: usize, j: usize| 300.0 + 150.0 * i as f64 - 70.0 * j as f64;
        let tx = [10.0, -5.0, 0.0, 25.0];
        let clk = [40.0, -12.0, 3.0];
        let obs = synth(&corr, &tx, &clk, 5.0, 20);
        let cfg = AsfConfig {
            site_clock_ns: sites().iter().zip(clk).map(|((s, _), c)| (s.to_string(), c)).collect(),
            tx_offset_ns: stations().iter().zip(tx).map(|((s, _), t)| (format!("c/{s}"), t)).collect(),
            subtract_seawater_sf: false,
        };
        let est = estimate_asf(&obs, &cfg).unwrap();
        assert_eq!(est.len(), 12);
        for e in &est {
            let i = sites().iter().position(|s| s.0 == e.site).unwrap();
            let j = stations().iter().position(|s| s.0 == e.station).unwrap();
            let want = corr(i, j);
            assert!((e.std_err_ns - 5.0 / 20f64.sqrt()).abs() < 0.6, "{}", e.std_err_ns);
            assert!((e.beyond_pf_ns - want).abs() < 3.5 * e.std_err_ns, "{e:?} want {want}");
        }
        let c = corrections_for_site(&est, "S2");
        assert_eq!(c.asf_ns.len(), 4);
        assert!(asf_table_csv(&est).lines().count() == 13);
        // the clock is required
        assert!(estimate_asf(&obs, &AsfConfig::default()).is_err());
    }

    #[test]
    fn transmitter_timing_from_multi_site_data() {
        let corr = |i: usize, j: usize| 200.0 + 40.0 * i as f64 + 90.0 * j as f64;
        let tx = [0.0, 37.0, -22.0, 15.0];
        let clk = [40.0, -12.0, 3.0];
        let obs = synth(&corr, &tx, &clk, 4.0, 10);
        let mut cmap = BTreeMap::new();
        for (i, (s, _)) in sites().iter().enumerate() {
            for (j, (st, _)) in stations().iter().enumerate() {
                cmap.insert(site_station_key(s, "c", st), corr(i, j));
            }
        }
        // unknown clocks: relative to the reference station c/M (true offset 0)
        let est = estimate_tx_timing(
            &obs,
            &TimingConfig { correction_ns: cmap.clone(), site_clock_ns: None, reference_station: Some("c/M".into()) },
        )
        .unwrap();
        for (j, (st, _)) in stations().iter().enumerate() {
            let p = est.tx_offset_ns.iter().find(|p| p.name == format!("c/{st}")).unwrap();
            if j > 0 {
                assert!(p.std_err > 0.3 && p.std_err < 2.0, "{p:?}");
            }
            assert!((p.value - tx[j]).abs() < 3.5 * p.std_err.max(1e-9), "{p:?}");
        }
        for (i, (s, _)) in sites().iter().enumerate() {
            let p = est.site_clock_ns.iter().find(|p| p.name == *s).unwrap();
            assert!((p.value - clk[i]).abs() < 3.5 * p.std_err, "{p:?}");
        }
        assert!((est.reduced_chi2 - 1.0).abs() < 0.4);
        // known clocks: absolute offsets for every station
        let est = estimate_tx_timing(
            &obs,
            &TimingConfig {
                correction_ns: cmap,
                site_clock_ns: Some(sites().iter().zip(clk).map(|((s, _), c)| (s.to_string(), c)).collect()),
                reference_station: None,
            },
        )
        .unwrap();
        assert!(est.reference_station.is_none() && est.site_clock_ns.is_empty());
        for (j, (st, _)) in stations().iter().enumerate() {
            let p = est.tx_offset_ns.iter().find(|p| p.name == format!("c/{st}")).unwrap();
            assert!((p.value - tx[j]).abs() < 3.5 * p.std_err, "{p:?}");
        }
    }

    #[test]
    fn segment_conductivity_is_recovered() {
        let profile = PathProfile {
            sections: vec![
                ProfileSection { ground: SegmentGround::Known { sigma_s_m: 5.0, eps_r: Some(80.0) }, end_km: 40.0 },
                ProfileSection { ground: SegmentGround::Unknown { eps_r: None }, end_km: 240.0 },
                ProfileSection { ground: SegmentGround::Known { sigma_s_m: 5.0, eps_r: Some(80.0) }, end_km: 400.0 },
            ],
            k_factor: crate::prop::ground::K_LORAN,
        };
        let d: Vec<f64> = (0..10).map(|i| 80.0 + 35.0 * i as f64).collect();
        let truth = 0.004;
        let clean = profile_asf_ns(&profile, truth, &d, 400).unwrap();
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let sigma = 5.0;
        let pts: Vec<AsfPoint> = d
            .iter()
            .zip(&clean)
            .map(|(d, a)| AsfPoint { distance_km: *d, asf_ns: a + sigma * gauss(&mut rng), sigma_ns: sigma })
            .collect();
        let fit = fit_segment_conductivity(&profile, &pts, &Default::default()).unwrap();
        eprintln!("conductivity fit {fit:?} clean asf {clean:?}");
        assert!(!fit.at_bound);
        assert!(fit.log10_std > 0.0 && fit.log10_std < 0.1, "{}", fit.log10_std);
        assert!(((fit.log10_sigma - truth.log10()) / fit.log10_std).abs() < 3.5, "{fit:?}");
        assert!((fit.sigma_s_m / truth - 1.0).abs() < 0.2);
        // updated configuration
        let g: GroundConfig = toml::from_str("model = \"smooth_earth\"\nsigma_s_m = 0.01").unwrap();
        match ground_with_fitted_sigma(&g, None, fit.sigma_s_m).unwrap() {
            GroundConfig::SmoothEarth { sigma_s_m, .. } => assert_eq!(sigma_s_m, fit.sigma_s_m),
            _ => unreachable!(),
        }
        let m: GroundConfig = toml::from_str(
            "model = \"millington\"\n[conductivity]\ntype = \"polygons\"\nbackground = { sigma_s_m = 5.0 }\n\
             [[conductivity.polygons]]\nname = \"land\"\nsigma_s_m = 0.01\nvertices = [[0.0,0.0],[1.0,0.0],[1.0,1.0]]",
        )
        .unwrap();
        let u = ground_with_fitted_sigma(&m, Some("land"), 0.004).unwrap();
        assert!(toml::to_string(&u).unwrap().contains("0.004"));
        assert!(ground_with_fitted_sigma(&m, Some("sea"), 0.004).is_err());
    }

    #[test]
    fn skywave_delay_ratio_and_phase_from_averaged_pulses() {
        let src = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.5
fs_hz = 1000000
seed = 1
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[propagation.ground]
model = "simple"
[propagation.sky]
model = "fixed"
delay_us = 47.0
ratio_db = -3.0
phase_deg = 40.0
[[interference]]
type = "wgn"
rms_uv_m_20khz = 100.0
seed = 5
[[chains]]
name = "sky"
gri_us = 79800
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
"#;
        let sc = Scenario::from_toml(src).unwrap();
        let (sink, _truth, _) = crate::run_to_memory(src).unwrap();
        let start = sc.start().unwrap();
        let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
        let input = RxInput::new(sink.channels, 1e6, start);
        let (rx, out) = rx::process(&input, &chains, RxConfig::default()).unwrap();
        for si in 0..2 {
            let refs: Vec<(i64, f64)> = out
                .measurements
                .iter()
                .filter(|m| m.station_index == si)
                .map(|m| (m.group, m.toa_s))
                .collect();
            let p = super::super::fidelity::averaged_pulse(
                &rx.filtered[0], 1e6, rx.n_valid, &chains[0], si, &refs, (-30.0, 330.0), 2,
            )
            .unwrap();
            let opts = SkywaveFitOptions { noise_gain: Some(rx.fir.noise_gain), ..Default::default() };
            let fit = fit_skywave(&p, &rx.templates[0], &opts).unwrap();
            eprintln!("skywave fit {fit:?}");
            assert!(fit.converged);
            assert!((fit.delay_us - 47.0).abs() < 0.3 && fit.delay_std_us < 0.2, "{fit:?}");
            assert!((fit.ratio_db + 3.0).abs() < 0.3 && fit.ratio_std_db < 0.2, "{fit:?}");
            assert!(wrap_deg(fit.sky_phase_deg - 40.0).abs() < 5.0, "{fit:?}");
            assert!((fit.delay_us - 47.0).abs() < 4.0 * fit.delay_std_us.max(0.02));
            match fit.to_sky_config() {
                SkyConfig::Fixed { delay_us, .. } => assert_eq!(delay_us, fit.delay_us),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn asf_end_to_end_from_receiver_measurements() {
        // smooth-earth scene, known site, ideal clocks: the estimated correction beyond PF
        // equals the truth SF + ASF
        let src = r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.6
fs_hz = 1000000
seed = 1
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 30.0
[propagation.ground]
model = "smooth_earth"
sigma_s_m = 0.01
[[chains]]
name = "e2e"
gri_us = 79800
[[chains.stations]]
id = "M"
role = "master"
position = { lat_deg = 56.5, lon_deg = 1.0 }
time_ref = "UTC(A)"
[[chains.stations]]
id = "X"
role = "secondary"
position = { lat_deg = 53.5, lon_deg = 1.5 }
emission_delay_us = 13000.0
time_ref = "UTC(A)"
"#;
        let sc = Scenario::from_toml(src).unwrap();
        let (sink, truth, _) = crate::run_to_memory(src).unwrap();
        let start = sc.start().unwrap();
        let chains: Vec<Chain> = sc.chains.iter().map(|c| c.chain.clone()).collect();
        let (_rx, out) =
            rx::process(&RxInput::new(sink.channels, 1e6, start), &chains, RxConfig::default()).unwrap();
        let site = Geo::new(55.0, 3.0);
        let obs = observations_from_measurements("site", site, &out.measurements, &chains, start, 0.0, 3.0);
        let mut cfg = AsfConfig { site_clock_ns: [("site".to_string(), 0.0)].into(), ..Default::default() };
        for t in &truth {
            cfg.tx_offset_ns.insert(format!("{}/{}", t.chain, t.station), t.emission_error_ns);
        }
        let est = estimate_asf(&obs, &cfg).unwrap();
        assert_eq!(est.len(), 2);
        for e in &est {
            let t = truth.iter().find(|t| t.station == e.station).unwrap();
            let want = t.sf_ns + t.asf_ns;
            assert!((e.beyond_pf_ns - want).abs() < 10.0, "{e:?} want {want}");
        }
    }
}
