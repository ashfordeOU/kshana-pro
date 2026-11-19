// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! A7 — self-calibrating simulator loop with signed lineage (gap 26; patent candidate F1:
//! do not describe this loop in public material before filing).
//!
//! # Loop
//!
//! 1. **Generate** a scenario from the current propagation parameters
//!    ([`generate_scenario`]): the template scenario with a Millington groundwave over a
//!    conductivity map whose calibrated sections carry the current σ, an ionospheric
//!    skywave at the current reflection height, and a per-station ASF bias.
//! 2. **Compare** the reference receiver's observations ([`RefObservation`]: groundwave
//!    ToA corrected for the known receiver clock, and skywave-minus-groundwave delay, e.g.
//!    from [`super::inverse::fit_skywave`]) with the simulator's prediction ([`predict`],
//!    the crate's own propagation models evaluated at every reference site). The
//!    comparison is scored with the fidelity comparator's metric rules
//!    ([`super::fidelity::Metric`], observable level, per-metric thresholds and score
//!    0–100). When a recorded capture is supplied ([`SignalCapture`]), the full comparator
//!    ([`super::fidelity::compare_samples`]: signal, observable and solution levels) also
//!    runs on the regenerated scenario at the first and last iteration (or every one).
//! 3. **Inverse modelling**: one damped Gauss–Newton step
//!    ([`super::inverse::levenberg_marquardt`], one iteration per loop pass, the
//!    Jacobian by regenerating the simulator at perturbed parameters) on the whitened
//!    residuals plus the parameter priors (ground-class priors in `log10 σ`, ASF-bias and
//!    reflection-height priors). Parameters: `log10 σ` per calibrated path section,
//!    ASF bias per station (ns), skywave virtual height (km). Uncertainty: the a-priori
//!    covariance `(JᵀJ)⁻¹` of the whitened problem, inflated by the reduced χ^½ when it
//!    exceeds 1.
//! 4. **Regenerate** and repeat until the cost stops falling (relative change below
//!    `rel_tol`), every step is below 1 % of its standard error, no downhill step exists,
//!    or `max_iter` passes have run. A step is accepted only if it lowers the cost, so the
//!    cost is monotonically non-increasing by construction.
//!
//! # Lineage
//!
//! Every pass (including the initial state, pass 0) is appended to the hash-chained
//! calibration register ([`crate::calreg`]) as one entry: quantity
//! [`SELFCAL_QUANTITY`], value = observable fidelity score, `notes` = the
//! [`IterationRecord`] as compact JSON (parameters with uncertainty, deltas, fidelity,
//! scenario hash, recording digest, estimator version) and `report_sha256` = SHA-256 of
//! that JSON. The run is closed by a [`LineageSignature`]: HMAC-SHA256 (RFC 2104, key held
//! by the calibration authority) over the hash of the run's last entry, the number of
//! entries, the calibrated scenario's hash and the recording digest.
//! [`verify_lineage`] re-checks the register chain, every record digest, the iteration
//! sequence, and the signature: editing, removing or reordering an entry, or rewriting the
//! whole chain with recomputed hashes, is detected. HMAC is symmetric: whoever holds the
//! key can sign; a public-key signature of the [`LineageSignature`] can be added at the
//! product level (kshana-pro signed dossier).
//!
//! # Identifiability (stated limits)
//!
//! * An ASF bias and a transmitter timing offset are the same thing in ToA (see
//!   [`super::inverse`]): the bias absorbs both. In the generated scenario it is applied as
//!   a transmitter clock offset (`tx_clock` time reference) of that station. Receiver
//!   clocks must be known (UTC-traceable reference receivers): observations are
//!   `toa − nominal emission − receiver clock`.
//! * One site cannot separate a section's σ from the station bias; several sites with
//!   different path fractions over each section are needed. The posterior covariance
//!   shows how well the data constrain each parameter.
//! * Time-reference offsets are taken at the scene start (drift ignored in the
//!   prediction).
//!
//! # Evidence
//!
//! MODELLED: verified by recovering the parameters of a synthetic "truth world" generated
//! by this crate's own models (tests). That checks the loop and the estimators, not the
//! propagation physics; nothing has been calibrated against a live recording yet.

use super::fidelity::{
    compare_samples, Bound, CaptureMeta, FidelityConfig, Level, Metric, ReferenceSource,
};
use super::inverse::{levenberg_marquardt, LmOptions, ParamEstimate};
use crate::calreg::{EntryBody, Register, SubjectKind};
use crate::geo::Geo;
use crate::ldc::hmac::{hmac_sha256, tags_equal};
use crate::prop::conductivity::{ConductivityConfig, GroundConst, PolygonConfig};
use crate::prop::ground::{GroundConfig, K_LORAN};
use crate::prop::sky::{Daylight, IonoConfig, SkyConfig};
use crate::prop::{self, PathEffect, PathKind};
use crate::scenario::Scenario;
use crate::timebase::{gauss, TimeRefModel};
use crate::traj::RxState;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

/// Estimator version written into every lineage record.
pub const SELFCAL_VERSION: &str = "kshana-eloran selfcal v1";
/// Calibration-register quantity of a loop pass.
pub const SELFCAL_QUANTITY: &str = "selfcal_iteration";

// ───────────────────────────── configuration ─────────────────────────────

/// A path section (conductivity-map polygon) whose σ is calibrated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SectionSpec {
    pub name: String,
    /// Vertices `[lat_deg, lon_deg]` (closed implicitly).
    pub vertices: Vec<[f64; 2]>,
    /// ε_r; default from σ (ITU-R P.527 categories).
    #[serde(default)]
    pub eps_r: Option<f64>,
    /// Prior (ground-class) σ (S/m): the start point and the prior mean.
    pub prior_sigma_s_m: f64,
    /// Prior standard deviation in log10 σ.
    #[serde(default = "d_prior_log10")]
    pub prior_log10_std: f64,
}

fn d_prior_log10() -> f64 {
    1.0
}

/// A station whose ASF bias (ns) is calibrated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BiasSpec {
    /// "chain/station".
    pub key: String,
    #[serde(default)]
    pub prior_ns: f64,
    pub prior_std_ns: f64,
}

/// The skywave reflection height is calibrated (one virtual height, day and night).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkySpec {
    pub prior_h_km: f64,
    pub prior_std_km: f64,
}

/// A reference receiver site (surveyed position).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefSite {
    pub id: String,
    pub position: Geo,
}

/// What a reference observation measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObsKind {
    /// Groundwave `toa − nominal emission − receiver clock` (ns): propagation + emission
    /// error.
    GroundToa,
    /// Skywave (one hop) carrier delay minus groundwave delay (ns).
    SkyMinusGround,
}

/// One reference-receiver observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefObservation {
    pub site: String,
    pub chain: String,
    pub station: String,
    pub kind: ObsKind,
    pub value_ns: f64,
    pub sigma_ns: f64,
}

impl RefObservation {
    fn key(&self) -> String {
        format!("{}:{}/{}", self.site, self.chain, self.station)
    }
}

/// Loop configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelfCalConfig {
    /// Calibration-register subject (the simulator instance / scenario family).
    pub subject_id: String,
    pub operator: String,
    /// Register date `YYYY-MM-DD`.
    pub date: String,
    pub sections: Vec<SectionSpec>,
    /// Ground outside the calibrated sections.
    #[serde(default = "GroundConst::seawater")]
    pub background: GroundConst,
    #[serde(default)]
    pub biases: Vec<BiasSpec>,
    #[serde(default)]
    pub sky: Option<SkySpec>,
    #[serde(default = "d_max_iter")]
    pub max_iter: usize,
    /// Convergence: relative cost decrease below this.
    #[serde(default = "d_rel_tol")]
    pub rel_tol: f64,
    /// Observable fidelity thresholds (|recorded − simulated|, ns).
    #[serde(default = "d_toa_thr")]
    pub toa_threshold_ns: f64,
    #[serde(default = "d_sky_thr")]
    pub sky_threshold_ns: f64,
    /// Search bounds of log10 σ.
    #[serde(default = "d_bounds")]
    pub log10_sigma_bounds: (f64, f64),
    /// Millington sampling step along each path (km).
    #[serde(default = "d_sample")]
    pub sample_km: f64,
}

fn d_max_iter() -> usize {
    10
}
fn d_rel_tol() -> f64 {
    1e-4
}
fn d_toa_thr() -> f64 {
    20.0
}
fn d_sky_thr() -> f64 {
    500.0
}
fn d_bounds() -> (f64, f64) {
    (-5.0, 1.0)
}
fn d_sample() -> f64 {
    2.0
}

impl SelfCalConfig {
    /// A configuration with defaults for everything but the identity and the sections.
    pub fn new(subject_id: &str, operator: &str, date: &str, sections: Vec<SectionSpec>) -> Self {
        SelfCalConfig {
            subject_id: subject_id.into(),
            operator: operator.into(),
            date: date.into(),
            sections,
            background: GroundConst::seawater(),
            biases: Vec::new(),
            sky: None,
            max_iter: d_max_iter(),
            rel_tol: d_rel_tol(),
            toa_threshold_ns: d_toa_thr(),
            sky_threshold_ns: d_sky_thr(),
            log10_sigma_bounds: d_bounds(),
            sample_km: d_sample(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.sections.is_empty() && self.biases.is_empty() && self.sky.is_none() {
            return Err("selfcal: nothing to calibrate".into());
        }
        for s in &self.sections {
            if s.vertices.len() < 3 {
                return Err(format!("selfcal: section {:?} needs ≥ 3 vertices", s.name));
            }
            if !(s.prior_sigma_s_m > 0.0) || !(s.prior_log10_std > 0.0) {
                return Err(format!(
                    "selfcal: section {:?}: prior σ and its std must be positive",
                    s.name
                ));
            }
        }
        if self.biases.iter().any(|b| !(b.prior_std_ns > 0.0)) {
            return Err("selfcal: bias prior std must be positive".into());
        }
        if let Some(s) = self.sky {
            if !(s.prior_h_km > 20.0) || !(s.prior_std_km > 0.0) {
                return Err("selfcal: sky prior height must be > 20 km, std > 0".into());
            }
        }
        if !(self.log10_sigma_bounds.1 > self.log10_sigma_bounds.0) {
            return Err("selfcal: bad log10 σ bounds".into());
        }
        Ok(())
    }

    /// Parameter names in vector order.
    pub fn param_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .sections
            .iter()
            .map(|s| format!("log10_sigma:{}", s.name))
            .collect();
        v.extend(self.biases.iter().map(|b| format!("asf_bias_ns:{}", b.key)));
        if self.sky.is_some() {
            v.push("sky_h_km".into());
        }
        v
    }

    fn prior(&self) -> (Vec<f64>, Vec<f64>) {
        let mut m: Vec<f64> = self
            .sections
            .iter()
            .map(|s| s.prior_sigma_s_m.log10())
            .collect();
        let mut s: Vec<f64> = self.sections.iter().map(|s| s.prior_log10_std).collect();
        for b in &self.biases {
            m.push(b.prior_ns);
            s.push(b.prior_std_ns);
        }
        if let Some(k) = self.sky {
            m.push(k.prior_h_km);
            s.push(k.prior_std_km);
        }
        (m, s)
    }

    fn fd_steps(&self) -> Vec<f64> {
        let mut v = vec![0.005; self.sections.len()];
        v.extend(std::iter::repeat_n(0.5, self.biases.len()));
        if self.sky.is_some() {
            v.push(0.2);
        }
        v
    }
}

/// Calibrated propagation parameters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CalParams {
    /// log10 σ (S/m) per section name.
    pub log10_sigma: BTreeMap<String, f64>,
    /// ASF bias per "chain/station" (ns, true − nominal arrival).
    pub bias_ns: BTreeMap<String, f64>,
    /// Skywave virtual height (km).
    pub sky_h_km: Option<f64>,
}

impl CalParams {
    /// The parameters of `cfg`'s vector `x`.
    pub fn from_vec(cfg: &SelfCalConfig, x: &[f64]) -> CalParams {
        let ns = cfg.sections.len();
        let nb = cfg.biases.len();
        CalParams {
            log10_sigma: cfg
                .sections
                .iter()
                .zip(x)
                .map(|(s, v)| (s.name.clone(), *v))
                .collect(),
            bias_ns: cfg
                .biases
                .iter()
                .zip(&x[ns..])
                .map(|(b, v)| (b.key.clone(), *v))
                .collect(),
            sky_h_km: cfg.sky.map(|_| x[ns + nb]),
        }
    }
    /// The vector of `cfg`'s parameters (missing entries take the prior mean).
    pub fn to_vec(&self, cfg: &SelfCalConfig) -> Vec<f64> {
        let (prior, _) = cfg.prior();
        let mut v = Vec::new();
        for (i, s) in cfg.sections.iter().enumerate() {
            v.push(self.log10_sigma.get(&s.name).copied().unwrap_or(prior[i]));
        }
        let ns = cfg.sections.len();
        for (i, b) in cfg.biases.iter().enumerate() {
            v.push(self.bias_ns.get(&b.key).copied().unwrap_or(prior[ns + i]));
        }
        if cfg.sky.is_some() {
            v.push(self.sky_h_km.unwrap_or(prior[prior.len() - 1]));
        }
        v
    }
    /// The prior means of `cfg` (the loop's start point).
    pub fn prior(cfg: &SelfCalConfig) -> CalParams {
        CalParams::from_vec(cfg, &cfg.prior().0)
    }
}

// ───────────────────────────── generation and prediction ─────────────────────────────

/// The template scenario with `params` applied (see the module docs).
pub fn generate_scenario(
    base: &Scenario,
    cfg: &SelfCalConfig,
    params: &CalParams,
) -> Result<Scenario, String> {
    let mut sc = base.clone();
    let (lo, hi) = cfg.log10_sigma_bounds;
    if !cfg.sections.is_empty() {
        let polygons = cfg
            .sections
            .iter()
            .map(|s| {
                let ls = params
                    .log10_sigma
                    .get(&s.name)
                    .copied()
                    .unwrap_or(s.prior_sigma_s_m.log10())
                    .clamp(lo, hi);
                PolygonConfig {
                    name: s.name.clone(),
                    sigma_s_m: 10f64.powf(ls),
                    eps_r: s.eps_r,
                    vertices: s.vertices.clone(),
                }
            })
            .collect();
        sc.propagation.ground = GroundConfig::Millington {
            conductivity: ConductivityConfig::Polygons {
                background: cfg.background,
                polygons,
            },
            k_factor: K_LORAN,
            sample_km: cfg.sample_km,
            asf_grid_deg: 0.005,
            temporal: None,
        };
    }
    if let Some(h) = params.sky_h_km {
        if !(h > 20.0 && h < 400.0) {
            return Err(format!("selfcal: skywave height {h} km out of range"));
        }
        let mut ic = match &sc.propagation.sky {
            Some(SkyConfig::Ionospheric(ic)) => ic.clone(),
            _ => IonoConfig {
                daylight: Daylight::Day,
                ..IonoConfig::default()
            },
        };
        ic.h_day_km = h;
        ic.h_night_km = h;
        sc.propagation.sky = Some(SkyConfig::Ionospheric(ic));
    }
    for (key, b) in &params.bias_ns {
        let (ci, si) = find_station(&sc, key)?;
        let id = format!("selfcal:{key}");
        let st = &mut sc.chains[ci].chain.stations[si];
        let mut model = match &st.tx_clock {
            Some(t) => base
                .time_refs
                .iter()
                .find(|r| &r.id == t)
                .cloned()
                .unwrap_or_else(|| TimeRefModel::ideal(t)),
            None => TimeRefModel::ideal(&id),
        };
        model.id = id.clone();
        // true emission = nominal − offset: a later arrival by b is an offset of −b
        model.offset_ns -= b;
        st.tx_clock = Some(id.clone());
        sc.time_refs.retain(|r| r.id != id);
        sc.time_refs.push(model);
    }
    Ok(sc)
}

fn find_station(sc: &Scenario, key: &str) -> Result<(usize, usize), String> {
    let (c, s) = key
        .split_once('/')
        .ok_or_else(|| format!("selfcal: station key {key:?} is not \"chain/station\""))?;
    for (ci, ch) in sc.chains.iter().enumerate() {
        if ch.chain.name == c {
            if let Some(si) = ch.chain.stations.iter().position(|x| x.id == s) {
                return Ok((ci, si));
            }
        }
    }
    Err(format!("selfcal: no station {key:?} in the scenario"))
}

/// The simulator's observable prediction at one site for one station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub site: String,
    pub chain: String,
    pub station: String,
    /// Groundwave propagation delay + emission error (ns).
    pub ground_ns: f64,
    /// Skywave (one hop) carrier delay − groundwave delay (ns), when a skywave is modelled.
    pub sky_minus_ground_ns: Option<f64>,
}

fn total_ns(p: &PathEffect) -> f64 {
    p.geom_ns + p.pf_ns + p.sf_ns + p.asf_ns
}

fn time_ref_offset_ns(sc: &Scenario, id: &str) -> f64 {
    sc.time_refs
        .iter()
        .find(|r| r.id == id)
        .map(|r| r.offset_ns)
        .unwrap_or(0.0)
}

/// Emission error (true − nominal, ns) of a station at the scene start.
fn emission_error_ns(sc: &Scenario, st: &crate::chain::Station) -> f64 {
    let mut x = 0.0;
    if !st.time_ref.is_empty() {
        x += time_ref_offset_ns(sc, &st.time_ref);
    }
    if let Some(t) = &st.tx_clock {
        x += time_ref_offset_ns(sc, t);
    }
    -x
}

/// Propagation part of the prediction: per (site, chain, station) the groundwave delay and
/// the skywave delay difference.
fn propagate(sc: &Scenario, sites: &[RefSite]) -> Result<Vec<(f64, Option<f64>)>, String> {
    let ctx = sc.build_context()?;
    let model = prop::build(&sc.propagation, &ctx)?;
    let mut out = Vec::new();
    for site in sites {
        let rx = RxState {
            t_s: 0.0,
            pos: site.position,
            heading_rad: 0.0,
            speed_mps: 0.0,
        };
        for ch in &sc.chains {
            for st in &ch.chain.stations {
                let paths = model.paths(st, &rx);
                let g = paths
                    .iter()
                    .find(|p| p.kind == PathKind::Ground)
                    .map(total_ns)
                    .ok_or_else(|| format!("selfcal: no groundwave {}/{}", ch.chain.name, st.id))?;
                let s = paths
                    .iter()
                    .find(|p| p.kind == PathKind::Sky { hop: 1 })
                    .map(|p| total_ns(p) - g);
                out.push((g, s));
            }
        }
    }
    Ok(out)
}

/// The simulator's prediction of every (site, station) observable of scenario `sc`.
pub fn predict(sc: &Scenario, sites: &[RefSite]) -> Result<Vec<Prediction>, String> {
    let p = propagate(sc, sites)?;
    let mut out = Vec::new();
    let mut k = 0;
    for site in sites {
        for ch in &sc.chains {
            for st in &ch.chain.stations {
                let (g, s) = p[k];
                k += 1;
                out.push(Prediction {
                    site: site.id.clone(),
                    chain: ch.chain.name.clone(),
                    station: st.id.clone(),
                    ground_ns: g + emission_error_ns(sc, st),
                    sky_minus_ground_ns: s,
                });
            }
        }
    }
    Ok(out)
}

/// Synthetic reference observations ("truth world", SYNTHETIC): the prediction of
/// `truth` plus Gaussian noise (seeded). Skywave observations only when `sigma_sky_ns` is
/// given and the truth models a skywave.
pub fn synthetic_observations(
    base: &Scenario,
    cfg: &SelfCalConfig,
    truth: &CalParams,
    sites: &[RefSite],
    sigma_toa_ns: f64,
    sigma_sky_ns: Option<f64>,
    seed: u64,
) -> Result<Vec<RefObservation>, String> {
    let sc = generate_scenario(base, cfg, truth)?;
    let pred = predict(&sc, sites)?;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut out = Vec::new();
    for p in &pred {
        out.push(RefObservation {
            site: p.site.clone(),
            chain: p.chain.clone(),
            station: p.station.clone(),
            kind: ObsKind::GroundToa,
            value_ns: p.ground_ns + sigma_toa_ns * gauss(&mut rng),
            sigma_ns: sigma_toa_ns,
        });
        if let (Some(s), Some(ss)) = (p.sky_minus_ground_ns, sigma_sky_ns) {
            out.push(RefObservation {
                site: p.site.clone(),
                chain: p.chain.clone(),
                station: p.station.clone(),
                kind: ObsKind::SkyMinusGround,
                value_ns: s + ss * gauss(&mut rng),
                sigma_ns: ss,
            });
        }
    }
    Ok(out)
}

/// Fast forward model of the loop: propagation memoised on (σ, h); biases and emission
/// errors added analytically (identical to [`predict`] of [`generate_scenario`]).
struct Forward<'a> {
    base: &'a Scenario,
    cfg: &'a SelfCalConfig,
    sites: &'a [RefSite],
    /// Observation index → prediction index.
    map: Vec<usize>,
    /// Per prediction index: base emission error (ns) and the bias parameter index.
    etx: Vec<(f64, Option<usize>)>,
    cache: HashMap<Vec<u64>, Vec<(f64, Option<f64>)>>,
    evals: usize,
}

impl<'a> Forward<'a> {
    fn new(
        base: &'a Scenario,
        cfg: &'a SelfCalConfig,
        sites: &'a [RefSite],
        obs: &[RefObservation],
    ) -> Result<Forward<'a>, String> {
        let mut index = BTreeMap::new();
        let mut etx = Vec::new();
        let mut k = 0usize;
        for site in sites {
            for ch in &base.chains {
                for st in &ch.chain.stations {
                    let key = format!("{}/{}", ch.chain.name, st.id);
                    index.insert(format!("{}:{key}", site.id), k);
                    let bi = cfg.biases.iter().position(|b| b.key == key);
                    etx.push((emission_error_ns(base, st), bi));
                    k += 1;
                }
            }
        }
        let map = obs
            .iter()
            .map(|o| {
                index.get(&o.key()).copied().ok_or_else(|| {
                    format!(
                        "selfcal: observation {} has no site/station in the scenario",
                        o.key()
                    )
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        for b in &cfg.biases {
            find_station(base, &b.key)?;
        }
        Ok(Forward {
            base,
            cfg,
            sites,
            map,
            etx,
            cache: HashMap::new(),
            evals: 0,
        })
    }

    /// Predicted value of every observation at `x`.
    fn eval(&mut self, x: &[f64], obs: &[RefObservation]) -> Result<Vec<f64>, String> {
        let ns = self.cfg.sections.len();
        let nb = self.cfg.biases.len();
        let mut key: Vec<u64> = x[..ns].iter().map(|v| v.to_bits()).collect();
        if self.cfg.sky.is_some() {
            key.push(x[ns + nb].to_bits());
        }
        if !self.cache.contains_key(&key) {
            let mut p = CalParams::from_vec(self.cfg, x);
            p.bias_ns.clear();
            let sc = generate_scenario(self.base, self.cfg, &p)?;
            let v = propagate(&sc, self.sites)?;
            self.evals += 1;
            if self.cache.len() > 256 {
                self.cache.clear();
            }
            self.cache.insert(key.clone(), v);
        }
        let prop = &self.cache[&key];
        obs.iter()
            .zip(&self.map)
            .map(|(o, &k)| {
                let (g, s) = prop[k];
                match o.kind {
                    ObsKind::GroundToa => {
                        let (e, bi) = self.etx[k];
                        Ok(g + e + bi.map(|i| x[ns + i]).unwrap_or(0.0))
                    }
                    ObsKind::SkyMinusGround => s.ok_or_else(|| {
                        format!(
                            "selfcal: skywave observation {} but no skywave is modelled",
                            o.key()
                        )
                    }),
                }
            })
            .collect()
    }
}

// ───────────────────────────── fidelity ─────────────────────────────

/// Observable-level fidelity of the reference observations against a prediction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservableFidelity {
    /// Per observation: recorded − simulated, with the comparator's pass/score rules.
    pub metrics: Vec<Metric>,
    /// 0–100 (100 × mean metric score).
    pub score: f64,
    pub pass: bool,
    /// Σ ((recorded − simulated)/σ)².
    pub chi2: f64,
    pub n: usize,
    pub toa_rms_ns: f64,
    pub sky_rms_ns: Option<f64>,
}

/// Score `obs` against predicted values `pred` (same order).
pub fn observable_fidelity(
    obs: &[RefObservation],
    pred: &[f64],
    cfg: &SelfCalConfig,
) -> ObservableFidelity {
    let mut metrics = Vec::new();
    let mut chi2 = 0.0;
    let (mut t2, mut nt, mut s2, mut nsk) = (0.0, 0usize, 0.0, 0usize);
    for (o, p) in obs.iter().zip(pred) {
        let d = o.value_ns - p;
        chi2 += (d / o.sigma_ns).powi(2);
        let (name, thr) = match o.kind {
            ObsKind::GroundToa => {
                t2 += d * d;
                nt += 1;
                ("toa_bias", cfg.toa_threshold_ns)
            }
            ObsKind::SkyMinusGround => {
                s2 += d * d;
                nsk += 1;
                ("sky_delay_diff", cfg.sky_threshold_ns)
            }
        };
        metrics.push(Metric::new(
            Level::Observable,
            Some(o.key()),
            name,
            d,
            "ns",
            thr,
            Bound::MaxAbs,
        ));
    }
    let n = metrics.len();
    let score = if n == 0 {
        0.0
    } else {
        100.0 * metrics.iter().map(|m| m.score).sum::<f64>() / n as f64
    };
    ObservableFidelity {
        pass: n > 0 && metrics.iter().all(|m| m.pass),
        metrics,
        score,
        chi2,
        n,
        toa_rms_ns: if nt > 0 { (t2 / nt as f64).sqrt() } else { 0.0 },
        sky_rms_ns: (nsk > 0).then(|| (s2 / nsk as f64).sqrt()),
    }
}

/// A recorded capture at one reference site for the full (signal-level) comparator.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalCapture {
    pub channels: Vec<Vec<f64>>,
    pub fs_hz: f64,
    pub meta: CaptureMeta,
    pub config: FidelityConfig,
    /// Run the comparator at every pass (otherwise the first and the last only).
    pub every_iteration: bool,
}

impl SignalCapture {
    fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(serde_json::to_vec(&self.meta).expect("serialisable"));
        h.update(self.fs_hz.to_le_bytes());
        for c in &self.channels {
            h.update((c.len() as u64).to_le_bytes());
            for v in c {
                h.update(v.to_le_bytes());
            }
        }
        hex::encode(h.finalize())
    }
}

/// SHA-256 of the observations (canonical JSON) and of the capture, if any.
pub fn recording_digest(obs: &[RefObservation], capture: Option<&SignalCapture>) -> String {
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(obs).expect("serialisable"));
    if let Some(c) = capture {
        h.update(c.digest().as_bytes());
    }
    hex::encode(h.finalize())
}

// ───────────────────────────── lineage ─────────────────────────────

/// One loop pass as recorded in the calibration register.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IterationRecord {
    pub version: String,
    /// 0 = initial state (priors), then 1, 2, …
    pub iteration: usize,
    pub recording_digest: String,
    /// Hash of the scenario generated from the parameters of this pass.
    pub scenario_hash: String,
    /// Parameters after this pass, with standard errors.
    pub params: Vec<ParamEstimate>,
    /// Parameter change of this pass.
    pub deltas: BTreeMap<String, f64>,
    pub fidelity_score: f64,
    pub fidelity_pass: bool,
    pub chi2: f64,
    pub n_obs: usize,
    /// Cost = χ² + prior penalty (non-increasing).
    pub cost: f64,
    pub toa_rms_ns: f64,
    pub sky_rms_ns: Option<f64>,
    /// Full comparator score and pass on the recorded capture, when run.
    pub signal_score: Option<f64>,
    pub signal_pass: Option<bool>,
    /// A downhill step was found and applied.
    pub accepted_step: bool,
}

/// The signature closing a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineageSignature {
    pub version: String,
    pub subject_id: String,
    /// Hash of the run's last register entry.
    pub head: String,
    /// Entries in the run.
    pub n_entries: usize,
    pub scenario_hash: String,
    pub recording_digest: String,
    /// Identifier of the signing key (not the key).
    pub key_id: String,
    /// HMAC-SHA256 tag (hex).
    pub tag: String,
}

impl LineageSignature {
    fn message(&self) -> String {
        format!(
            "kshana-selfcal-lineage\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            self.version,
            self.subject_id,
            self.head,
            self.n_entries,
            self.scenario_hash,
            self.recording_digest,
            self.key_id
        )
    }
    fn sign(mut self, key: &[u8]) -> LineageSignature {
        self.tag = hex::encode(hmac_sha256(key, self.message().as_bytes()));
        self
    }
    fn tag_ok(&self, key: &[u8]) -> bool {
        let want = hmac_sha256(key, self.message().as_bytes());
        hex::decode(&self.tag)
            .map(|t| tags_equal(&t, &want))
            .unwrap_or(false)
    }
}

/// Result of [`verify_lineage`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineageCheck {
    pub entries: usize,
    pub problems: Vec<String>,
}

impl LineageCheck {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn sha_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Verify a run's lineage in `register` against its signature (see the module docs).
pub fn verify_lineage(
    register: &Register,
    sig: &LineageSignature,
    key: &[u8],
) -> Result<LineageCheck, String> {
    let mut problems = Vec::new();
    let v = register.verify()?;
    problems.extend(v.problems.iter().map(|p| format!("register: {p}")));
    if !sig.tag_ok(key) {
        problems.push("signature: HMAC tag does not match (wrong key or altered fields)".into());
    }
    let entries = register.entries().unwrap_or_default();
    let Some(end) = entries.iter().position(|e| e.hash == sig.head) else {
        problems.push("signature: the signed head entry is not in the register".into());
        return Ok(LineageCheck {
            entries: 0,
            problems,
        });
    };
    // the run: entries of the subject ending at the head, back to iteration 0
    let mut run = Vec::new();
    for e in entries[..=end].iter().rev() {
        if e.body.subject_id != sig.subject_id || e.body.quantity != SELFCAL_QUANTITY {
            continue;
        }
        let rec: Result<IterationRecord, _> = serde_json::from_str(&e.body.notes);
        let first = rec.as_ref().map(|r| r.iteration == 0).unwrap_or(false);
        run.push((e, rec));
        if first {
            break;
        }
    }
    run.reverse();
    if run.len() != sig.n_entries {
        problems.push(format!(
            "run has {} entries, the signature covers {}",
            run.len(),
            sig.n_entries
        ));
    }
    for (i, (e, rec)) in run.iter().enumerate() {
        let seq = e.body.seq;
        if sha_hex(&e.body.notes) != e.body.report_sha256 {
            problems.push(format!("entry {seq}: record digest mismatch"));
        }
        match rec {
            Err(err) => problems.push(format!("entry {seq}: not an iteration record: {err}")),
            Ok(r) => {
                if r.iteration != i {
                    problems.push(format!(
                        "entry {seq}: iteration {} where {i} expected",
                        r.iteration
                    ));
                }
                if r.recording_digest != sig.recording_digest {
                    problems.push(format!("entry {seq}: recording digest differs"));
                }
                if (r.fidelity_score - e.body.value).abs() > 1e-9 {
                    problems.push(format!("entry {seq}: value differs from its record"));
                }
                if i + 1 == run.len() && r.scenario_hash != sig.scenario_hash {
                    problems.push("last entry: calibrated scenario hash differs".into());
                }
            }
        }
    }
    // a later run of the subject must start again at iteration 0
    let next = entries[end + 1..]
        .iter()
        .find(|e| e.body.subject_id == sig.subject_id && e.body.quantity == SELFCAL_QUANTITY);
    if let Some(next) = next {
        if let Ok(r) = serde_json::from_str::<IterationRecord>(&next.body.notes) {
            if r.iteration != 0 {
                problems.push(format!(
                    "entry {}: continues the signed run after its signature",
                    next.body.seq
                ));
            }
        }
    }
    Ok(LineageCheck {
        entries: run.len(),
        problems,
    })
}

// ───────────────────────────── the loop ─────────────────────────────

/// Result of a self-calibration run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SelfCalResult {
    pub records: Vec<IterationRecord>,
    /// Posterior (final parameters with standard errors).
    pub posterior: Vec<ParamEstimate>,
    pub params: CalParams,
    /// The calibrated scenario.
    pub scenario: Scenario,
    pub converged: bool,
    pub stop_reason: String,
    pub recording_digest: String,
    pub signature: LineageSignature,
    /// Forward-model evaluations (simulator regenerations).
    pub regenerations: usize,
}

impl SelfCalResult {
    /// The posterior estimate `name` (see [`SelfCalConfig::param_names`]).
    pub fn estimate(&self, name: &str) -> Option<&ParamEstimate> {
        self.posterior.iter().find(|p| p.name == name)
    }
    /// Observable fidelity score per pass.
    pub fn scores(&self) -> Vec<f64> {
        self.records.iter().map(|r| r.fidelity_score).collect()
    }
}

/// Run the self-calibration loop (see the module docs); every pass is appended to
/// `register` and the run is signed with `key` (identified as `key_id`).
#[allow(clippy::too_many_arguments)]
pub fn run_selfcal(
    base: &Scenario,
    cfg: &SelfCalConfig,
    sites: &[RefSite],
    obs: &[RefObservation],
    capture: Option<&SignalCapture>,
    register: &Register,
    key: &[u8],
    key_id: &str,
) -> Result<SelfCalResult, String> {
    cfg.validate()?;
    if key.is_empty() {
        return Err("selfcal: empty signing key".into());
    }
    if obs
        .iter()
        .any(|o| !(o.sigma_ns > 0.0) || !o.value_ns.is_finite())
    {
        return Err("selfcal: observations need finite values and σ > 0".into());
    }
    let names = cfg.param_names();
    let n = names.len();
    if obs.len() < n {
        return Err(format!(
            "selfcal: {} observations for {n} parameters",
            obs.len()
        ));
    }
    let (prior_m, prior_s) = cfg.prior();
    let digest = recording_digest(obs, capture);
    let mut fwd = Forward::new(base, cfg, sites, obs)?;

    let residuals = |fwd: &mut Forward, x: &[f64]| -> Vec<f64> {
        match fwd.eval(x, obs) {
            Ok(p) => {
                let mut r: Vec<f64> = obs
                    .iter()
                    .zip(&p)
                    .map(|(o, v)| (o.value_ns - v) / o.sigma_ns)
                    .collect();
                r.extend((0..n).map(|k| (x[k] - prior_m[k]) / prior_s[k]));
                r
            }
            Err(_) => vec![f64::NAN; obs.len() + n],
        }
    };
    let cost_of = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>();

    let mut x = prior_m.clone();
    let mut std = prior_s.clone();
    let mut records: Vec<IterationRecord> = Vec::new();
    let mut converged = false;
    let mut stop_reason = format!("max_iter ({}) reached", cfg.max_iter);

    let make_record = |fwd: &mut Forward,
                       it: usize,
                       x: &[f64],
                       std: &[f64],
                       dx: &[f64],
                       accepted: bool,
                       with_signal: bool|
     -> Result<IterationRecord, String> {
        let pred = fwd.eval(x, obs)?;
        let fid = observable_fidelity(obs, &pred, cfg);
        let params = CalParams::from_vec(cfg, x);
        let sc = generate_scenario(base, cfg, &params)?;
        let (signal_score, signal_pass) = match capture {
            Some(c) if with_signal => {
                let rep = compare_samples(
                    &c.channels,
                    c.fs_hz,
                    &c.meta,
                    &ReferenceSource::Scenario(Box::new(sc.clone())),
                    &c.config,
                )?;
                (Some(rep.score), Some(rep.pass))
            }
            _ => (None, None),
        };
        let prior_pen: f64 = (0..n)
            .map(|k| ((x[k] - prior_m[k]) / prior_s[k]).powi(2))
            .sum();
        Ok(IterationRecord {
            version: SELFCAL_VERSION.into(),
            iteration: it,
            recording_digest: digest.clone(),
            scenario_hash: sc.hash(),
            params: names
                .iter()
                .zip(x.iter().zip(std))
                .map(|(nm, (v, s))| ParamEstimate {
                    name: nm.clone(),
                    value: *v,
                    std_err: *s,
                })
                .collect(),
            deltas: names.iter().cloned().zip(dx.iter().copied()).collect(),
            fidelity_score: fid.score,
            fidelity_pass: fid.pass,
            chi2: fid.chi2,
            n_obs: fid.n,
            cost: fid.chi2 + prior_pen,
            toa_rms_ns: fid.toa_rms_ns,
            sky_rms_ns: fid.sky_rms_ns,
            signal_score,
            signal_pass,
            accepted_step: accepted,
        })
    };

    let mut heads = Vec::new();
    let append = |rec: &IterationRecord, heads: &mut Vec<String>| -> Result<(), String> {
        let notes = serde_json::to_string(rec).map_err(|e| format!("selfcal record: {e}"))?;
        let e = register.append(EntryBody {
            seq: 0,
            subject_id: cfg.subject_id.clone(),
            subject_kind: SubjectKind::Simulator,
            date: cfg.date.clone(),
            quantity: SELFCAL_QUANTITY.into(),
            value: rec.fidelity_score,
            unit: "fidelity_score".into(),
            expanded_u: 0.0,
            k: 2.0,
            operator: cfg.operator.clone(),
            report_sha256: sha_hex(&notes),
            interval_days: None,
            notes,
        })?;
        heads.push(e.hash);
        Ok(())
    };

    let r0 = make_record(&mut fwd, 0, &x, &std, &vec![0.0; n], false, true)?;
    append(&r0, &mut heads)?;
    let mut cost = r0.cost;
    records.push(r0);

    let opts = LmOptions {
        max_iter: 1,
        fd_step: cfg.fd_steps(),
        ..LmOptions::default()
    };
    for it in 1..=cfg.max_iter {
        let lm = levenberg_marquardt(|p| residuals(&mut fwd, p), &x, &opts)?;
        let dx: Vec<f64> = lm.x.iter().zip(&x).map(|(a, b)| a - b).collect();
        let accepted = dx.iter().any(|d| *d != 0.0);
        let infl = lm.sigma0.max(1.0);
        let new_std: Vec<f64> = lm.std_err_apriori.iter().map(|s| s * infl).collect();
        let new_cost = cost_of(&residuals(&mut fwd, &lm.x));
        let rel = if cost > 0.0 {
            (cost - new_cost) / cost
        } else {
            0.0
        };
        let small = dx
            .iter()
            .zip(&new_std)
            .all(|(d, s)| d.abs() <= 0.01 * s.max(1e-12));
        let done = if !accepted {
            stop_reason = "no downhill step: at the minimum".into();
            true
        } else if rel < cfg.rel_tol {
            stop_reason = format!("relative cost change {rel:.2e} < {:.1e}", cfg.rel_tol);
            true
        } else if small {
            stop_reason = "every step below 1 % of its standard error".into();
            true
        } else {
            false
        };
        if done {
            converged = true;
        }
        let last = done || it == cfg.max_iter;
        let with_signal = capture.is_some_and(|c| c.every_iteration) || last;
        x = lm.x.clone();
        std = new_std;
        let rec = make_record(&mut fwd, it, &x, &std, &dx, accepted, with_signal)?;
        append(&rec, &mut heads)?;
        cost = rec.cost;
        records.push(rec);
        if done {
            break;
        }
    }

    let params = CalParams::from_vec(cfg, &x);
    let scenario = generate_scenario(base, cfg, &params)?;
    let last = records.last().expect("at least one record");
    let signature = LineageSignature {
        version: SELFCAL_VERSION.into(),
        subject_id: cfg.subject_id.clone(),
        head: heads.last().cloned().expect("appended"),
        n_entries: records.len(),
        scenario_hash: last.scenario_hash.clone(),
        recording_digest: digest.clone(),
        key_id: key_id.into(),
        tag: String::new(),
    }
    .sign(key);
    Ok(SelfCalResult {
        posterior: last.params.clone(),
        records,
        params,
        scenario,
        converged,
        stop_reason,
        recording_digest: digest,
        signature,
        regenerations: fwd.evals,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::MemSink;
    use crate::traj::Trajectory;

    /// Synthetic four-station geometry over a North-Sea-like box (not real stations).
    const BASE: &str = r#"
[scene]
start_utc = "2026-11-18T12:00:00Z"
duration_s = 0.4
fs_hz = 1000000
seed = 3
output_scale_uv_m = 1000000.0
[receiver]
type = "static"
lat_deg = 55.0
lon_deg = 3.0
[[outputs]]
type = "e_field"
[[time_refs]]
id = "UTC(A)"
offset_ns = 20.0
[[chains]]
name = "sc"
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
[[chains.stations]]
id = "Y"
role = "secondary"
position = { lat_deg = 55.5, lon_deg = 6.5 }
emission_delay_us = 29000.0
time_ref = "UTC(A)"
[[chains.stations]]
id = "Z"
role = "secondary"
position = { lat_deg = 53.8, lon_deg = 5.5 }
emission_delay_us = 46000.0
time_ref = "UTC(A)"
"#;

    fn base() -> Scenario {
        Scenario::from_toml(BASE).unwrap()
    }

    fn sites() -> Vec<RefSite> {
        [
            ("S1", 55.0, 3.0),
            ("S2", 54.2, 2.2),
            ("S3", 55.8, 4.5),
            ("S4", 54.5, 5.0),
        ]
        .iter()
        .map(|(id, la, lo)| RefSite {
            id: (*id).into(),
            position: Geo::new(*la, *lo),
        })
        .collect()
    }

    fn sections() -> Vec<SectionSpec> {
        vec![
            SectionSpec {
                name: "west".into(),
                vertices: vec![[54.0, 0.0], [57.0, 0.0], [57.0, 2.6], [54.0, 2.6]],
                eps_r: None,
                prior_sigma_s_m: 0.01,
                prior_log10_std: 1.0,
            },
            SectionSpec {
                name: "east".into(),
                vertices: vec![[53.0, 4.2], [56.2, 4.2], [56.2, 7.5], [53.0, 7.5]],
                eps_r: None,
                prior_sigma_s_m: 0.01,
                prior_log10_std: 1.0,
            },
        ]
    }

    fn config(sky: bool) -> SelfCalConfig {
        let mut c = SelfCalConfig::new("SIM-TEST", "test", "2026-11-19", sections());
        c.biases = ["M", "X", "Y", "Z"]
            .iter()
            .map(|s| BiasSpec {
                key: format!("sc/{s}"),
                prior_ns: 0.0,
                prior_std_ns: 200.0,
            })
            .collect();
        if sky {
            c.sky = Some(SkySpec {
                prior_h_km: 70.0,
                prior_std_km: 10.0,
            });
        }
        c
    }

    fn truth(sky: bool) -> CalParams {
        CalParams {
            log10_sigma: [
                ("west".into(), 0.002f64.log10()),
                ("east".into(), 0.03f64.log10()),
            ]
            .into_iter()
            .collect(),
            bias_ns: [
                ("sc/M", 15.0),
                ("sc/X", 40.0),
                ("sc/Y", -25.0),
                ("sc/Z", 0.0),
            ]
            .iter()
            .map(|(k, v)| ((*k).into(), *v))
            .collect(),
            sky_h_km: sky.then_some(76.0),
        }
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "kshana-selfcal-{}-{name}.jsonl",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&p);
        p
    }

    const KEY: &[u8] = b"test calibration authority key";

    #[test]
    fn fast_forward_model_matches_the_generated_scenario() {
        let cfg = config(true);
        let t = truth(true);
        let obs = synthetic_observations(&base(), &cfg, &t, &sites(), 1.0, Some(1.0), 1).unwrap();
        let s = sites();
        let b = base();
        let mut f = Forward::new(&b, &cfg, &s, &obs).unwrap();
        let fast = f.eval(&t.to_vec(&cfg), &obs).unwrap();
        let pred = predict(&generate_scenario(&base(), &cfg, &t).unwrap(), &sites()).unwrap();
        for (o, v) in obs.iter().zip(&fast) {
            let p = pred
                .iter()
                .find(|p| p.site == o.site && p.station == o.station)
                .unwrap();
            let want = match o.kind {
                ObsKind::GroundToa => p.ground_ns,
                ObsKind::SkyMinusGround => p.sky_minus_ground_ns.unwrap(),
            };
            assert!((v - want).abs() < 1e-9, "{v} vs {want}");
        }
        // the bias enters as a later arrival and the time reference offset as an earlier one
        let mut t0 = t.clone();
        t0.bias_ns.insert("sc/X".into(), 0.0);
        let p0 = predict(&generate_scenario(&base(), &cfg, &t0).unwrap(), &sites()).unwrap();
        let px = |p: &[Prediction]| p.iter().find(|q| q.station == "X").unwrap().ground_ns;
        assert!((px(&pred) - px(&p0) - 40.0).abs() < 1e-9);
        // land sections slow the groundwave: lower σ, larger delay
        let mut lo = t.clone();
        lo.log10_sigma.insert("west".into(), -3.5);
        let pl = predict(&generate_scenario(&base(), &cfg, &lo).unwrap(), &sites()).unwrap();
        let pm = |p: &[Prediction]| p.iter().find(|q| q.station == "M").unwrap().ground_ns;
        assert!(pm(&pl) > pm(&pred) + 50.0, "{} {}", pm(&pl), pm(&pred));
    }

    #[test]
    fn loop_recovers_truth_world_and_fidelity_improves_monotonically() {
        let cfg = config(true);
        let t = truth(true);
        let obs =
            synthetic_observations(&base(), &cfg, &t, &sites(), 5.0, Some(100.0), 42).unwrap();
        let path = tmp("recover");
        let reg = Register::open(&path);
        let r = run_selfcal(&base(), &cfg, &sites(), &obs, None, &reg, KEY, "k1").unwrap();
        assert!(r.converged, "{}", r.stop_reason);
        assert!(r.records.len() >= 3, "{} passes", r.records.len());
        // the initial (prior) world is clearly wrong, the calibrated one fits within noise
        let first = &r.records[0];
        let last = r.records.last().unwrap();
        assert!(first.toa_rms_ns > 100.0, "{}", first.toa_rms_ns);
        assert!(!first.fidelity_pass);
        assert!(last.fidelity_pass, "{last:?}");
        assert!(last.toa_rms_ns < 10.0, "{}", last.toa_rms_ns);
        let red = last.chi2 / (last.n_obs - cfg.param_names().len()) as f64;
        assert!(red < 2.0, "reduced χ² {red}");
        // monotonic: the cost never rises; the score never falls (up to noise)
        for w in r.records.windows(2) {
            assert!(
                w[1].cost <= w[0].cost * (1.0 + 1e-12),
                "{} → {}",
                w[0].cost,
                w[1].cost
            );
            assert!(w[1].fidelity_score >= w[0].fidelity_score - 0.5);
        }
        assert!(last.fidelity_score > first.fidelity_score + 20.0);
        // parameters recovered within 3σ, with useful uncertainty
        let tv = t.to_vec(&cfg);
        for (k, name) in cfg.param_names().iter().enumerate() {
            let e = r.estimate(name).unwrap();
            assert!(e.std_err > 0.0 && e.std_err.is_finite());
            assert!(
                (e.value - tv[k]).abs() < 3.0 * e.std_err,
                "{name}: {} ± {} vs truth {}",
                e.value,
                e.std_err,
                tv[k]
            );
        }
        assert!(r.estimate("log10_sigma:west").unwrap().std_err < 0.3);
        assert!(r.estimate("log10_sigma:east").unwrap().std_err < 0.3);
        assert!(r.estimate("sky_h_km").unwrap().std_err < 1.0);
        assert!(r.estimate("asf_bias_ns:sc/X").unwrap().std_err < 30.0);
        // lineage intact and signed; the calibrated scenario is the signed one
        let chk = verify_lineage(&reg, &r.signature, KEY).unwrap();
        assert!(chk.ok(), "{:?}", chk.problems);
        assert_eq!(chk.entries, r.records.len());
        assert_eq!(r.scenario.hash(), r.signature.scenario_hash);
        assert!(r.regenerations > r.records.len());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn posterior_uncertainty_covers_truth_over_noise_realisations() {
        // Monte Carlo: the 2σ interval of each section's log10 σ must contain the truth in
        // about 95 % of realisations; with 12 trials allow at most 3 misses per parameter.
        let mut cfg = config(false);
        cfg.max_iter = 8;
        let t = truth(false);
        let tv = t.to_vec(&cfg);
        let mut misses = vec![0usize; 2];
        for seed in 0..12u64 {
            let obs =
                synthetic_observations(&base(), &cfg, &t, &sites(), 5.0, None, 100 + seed).unwrap();
            let path = tmp(&format!("mc{seed}"));
            let reg = Register::open(&path);
            let r = run_selfcal(&base(), &cfg, &sites(), &obs, None, &reg, KEY, "k").unwrap();
            for k in 0..2 {
                let e = &r.posterior[k];
                if (e.value - tv[k]).abs() > 2.0 * e.std_err {
                    misses[k] += 1;
                }
            }
            let _ = std::fs::remove_file(&path);
        }
        assert!(misses.iter().all(|m| *m <= 3), "{misses:?}");
    }

    #[test]
    fn signal_level_comparator_confirms_the_calibration() {
        // the "recording": the truth world simulated at site S1 (synthetic capture)
        let cfg = config(false);
        let t = truth(false);
        let mut b = base();
        b.receiver = Trajectory::Static {
            lat_deg: 55.0,
            lon_deg: 3.0,
            h_m: 0.0,
            heading_deg: 0.0,
        };
        let truth_sc = generate_scenario(&b, &cfg, &t).unwrap();
        let mut scene = truth_sc.build().unwrap();
        let mut sink = MemSink::default();
        let mut tr = Vec::new();
        scene.run(&mut sink, &mut tr).unwrap();
        let capture = SignalCapture {
            channels: sink.channels,
            fs_hz: 1e6,
            meta: CaptureMeta {
                position: Geo::new(55.0, 3.0),
                antenna: super::super::fidelity::CaptureAntenna::EField,
                start_utc: "2026-11-18T12:00:00Z".into(),
                scale_uv_m: 1e6,
                gain_db: 0.0,
                label: "synthetic truth capture".into(),
            },
            config: FidelityConfig::default(),
            every_iteration: false,
        };
        let obs = synthetic_observations(&b, &cfg, &t, &sites(), 5.0, None, 7).unwrap();
        let path = tmp("signal");
        let reg = Register::open(&path);
        let r = run_selfcal(&b, &cfg, &sites(), &obs, Some(&capture), &reg, KEY, "k").unwrap();
        let s0 = r.records[0].signal_score.unwrap();
        let s1 = r.records.last().unwrap().signal_score.unwrap();
        assert_eq!(r.records[0].signal_pass, Some(false));
        assert!(s1 > s0, "signal score {s0} → {s1}");
        assert_eq!(r.records.last().unwrap().signal_pass, Some(true), "{s1}");
        // intermediate passes skip the expensive comparator
        if r.records.len() > 2 {
            assert!(r.records[1].signal_score.is_none());
        }
        // the capture is part of the signed recording digest
        assert_eq!(r.recording_digest, recording_digest(&obs, Some(&capture)));
        assert_ne!(r.recording_digest, recording_digest(&obs, None));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn tampering_with_the_lineage_is_detected() {
        let mut cfg = config(false);
        cfg.max_iter = 4;
        let t = truth(false);
        let obs = synthetic_observations(&base(), &cfg, &t, &sites(), 5.0, None, 3).unwrap();
        let path = tmp("tamper");
        let reg = Register::open(&path);
        let r = run_selfcal(&base(), &cfg, &sites(), &obs, None, &reg, KEY, "k").unwrap();
        assert!(verify_lineage(&reg, &r.signature, KEY).unwrap().ok());
        let orig = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = orig.lines().collect();
        assert!(lines.len() >= 2);

        // 1. edit a recorded parameter delta in place: the chain breaks
        let edited = lines[1].replacen(
            "\\\"accepted_step\\\":true",
            "\\\"accepted_step\\\":false",
            1,
        );
        assert_ne!(edited, lines[1]);
        let mut l2 = lines.clone();
        l2[1] = &edited;
        std::fs::write(&path, l2.join("\n") + "\n").unwrap();
        let c = verify_lineage(&reg, &r.signature, KEY).unwrap();
        assert!(!c.ok());
        assert!(
            c.problems.iter().any(|p| p.contains("edited")),
            "{:?}",
            c.problems
        );

        // 2. rewrite the whole chain with recomputed hashes (forger without the key)
        std::fs::write(&path, "").unwrap();
        let forged_reg = Register::open(&path);
        for (i, l) in lines.iter().enumerate() {
            let e: crate::calreg::Entry = serde_json::from_str(l).unwrap();
            let mut b = e.body.clone();
            if i == 1 {
                let mut rec: IterationRecord = serde_json::from_str(&b.notes).unwrap();
                rec.fidelity_score = 99.9;
                b.value = 99.9;
                b.notes = serde_json::to_string(&rec).unwrap();
                b.report_sha256 = sha_hex(&b.notes);
            }
            forged_reg.append(b).unwrap();
        }
        assert!(
            forged_reg.verify().unwrap().ok(),
            "chain itself re-hashes cleanly"
        );
        let c = verify_lineage(&forged_reg, &r.signature, KEY).unwrap();
        assert!(!c.ok());
        assert!(
            c.problems
                .iter()
                .any(|p| p.contains("head entry is not in the register")),
            "{:?}",
            c.problems
        );
        // the forger re-signs with a wrong key: the tag fails
        let mut fake = r.signature.clone();
        fake.head = forged_reg.verify().unwrap().head;
        let fake = fake.sign(b"guessed key");
        let c = verify_lineage(&forged_reg, &fake, KEY).unwrap();
        assert!(
            c.problems.iter().any(|p| p.contains("HMAC")),
            "{:?}",
            c.problems
        );

        // 3. drop the last entry (truncate the run): the signed head is gone
        std::fs::write(&path, lines[..lines.len() - 1].join("\n") + "\n").unwrap();
        let c = verify_lineage(&reg, &r.signature, KEY).unwrap();
        assert!(!c.ok());

        // 4. a record whose digest field was not updated
        std::fs::write(&path, "").unwrap();
        for (i, l) in lines.iter().enumerate() {
            let e: crate::calreg::Entry = serde_json::from_str(l).unwrap();
            let mut b = e.body.clone();
            if i == 0 {
                b.notes = b.notes.replace("\"iteration\":0", "\"iteration\":0 ");
            }
            reg.append(b).unwrap();
        }
        // even re-signed by the key holder, the record no longer matches its digest
        let mut resigned = r.signature.clone();
        resigned.head = reg.verify().unwrap().head;
        let resigned = resigned.sign(KEY);
        let c = verify_lineage(&reg, &resigned, KEY).unwrap();
        assert!(
            c.problems.iter().any(|p| p.contains("record digest")),
            "{:?}",
            c.problems
        );

        // the original file verifies again
        std::fs::write(&path, &orig).unwrap();
        assert!(verify_lineage(&reg, &r.signature, KEY).unwrap().ok());
        assert!(!verify_lineage(&reg, &r.signature, b"other").unwrap().ok());
        let _ = std::fs::remove_file(&path);
    }
}
