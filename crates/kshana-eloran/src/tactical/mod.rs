// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! A16 — tactical / deployable transmitters and signals-of-opportunity fusion (gaps 18 and
//! 19; NAVISP-EL1-151 and EL1-152 scenario modelling).
//!
//! # Deployable transmitters
//!
//! [`DeployableTx`]: a low-power transportable transmitter, described by transmitter power,
//! mast height, top loading and antenna loss resistance, set-up time, and the uncertainty
//! of its initial ASF / timing (unknown at switch-on). The antenna is an electrically short
//! monopole (λ ≈ 3 km at 100 kHz):
//!
//! ```text
//! R_rad = 160 π² (h_eff/λ)²,  h_eff = h (top-loaded) or h/2 (plain mast)
//! η = R_rad / (R_rad + R_loss),  P_rad = η · P_tx
//! ```
//!
//! (standard short-monopole relations, e.g. Terman, *Radio Engineers' Handbook*; the loss
//! resistance of a quickly deployed ground system is an input, defaults illustrative). The
//! radiated power feeds the network planner ([`crate::plan`], groundwave field `E1·√P/d`
//! with any propagation model): [`coverage`] and [`service_radius_km`] give the coverage
//! of a deployment, and [`deployable_scenario`] writes a simulator scenario of it with the
//! unknown initial offsets as transmitter clock errors. [`templates`] gives three
//! illustrative classes (man-pack, vehicle, container); their parameters are assumptions
//! for study, not vendor data.
//!
//! # Rapid ASF calibration
//!
//! [`rapid_asf_calibration`]: a reference receiver at a surveyed site with a known clock
//! measures `toa − nominal emission − receiver clock − PF` of the new transmitter (the
//! correction beyond PF, i.e. SF + ASF + transmitter timing, the quantity of
//! [`crate::valid::inverse::estimate_asf`]); a scalar Kalman filter (random-walk state for
//! the transmitter's clock wander) converges on it from a wide prior. The result is the
//! correction users apply; [`spatial_residual_ns`] gives how far it stays valid (the ASF
//! difference between the reference site and a user, from the propagation model).
//! [`reference_observations`] synthesises the reference receiver's data (SYNTHETIC).
//!
//! # Signals of opportunity
//!
//! [`SoopTx`]: an AM/LW broadcast carrier at a known site and frequency with **unknown
//! carrier phase**. A reference station at a known position (clock known) tracks each
//! carrier's phase; a rover's phase minus the reference phase cancels the transmitter phase
//! and leaves `(R_rover − R_ref + c·δt)/λ` modulo one cycle, where `δt` is the rover clock
//! offset it shares with its eLoran pseudoranges. The whole cycles are resolved by rounding
//! against the eLoran-only fix (valid when that fix's error along the line of sight is well
//! under λ/2: LW, λ ≈ 1.1–2 km, suits this; medium-wave AM, λ ≈ 180–600 m, needs a good
//! eLoran fix). Range delays at each carrier's own frequency come from the smooth-earth
//! groundwave ([`crate::prop::smooth_earth::GroundCurve`]). [`fusion_study`] is the
//! accuracy study helper: weighted HDOP (eLoran alone and fused,
//! [`crate::plan::weighted_hdop_m`]) and a Monte Carlo of the eLoran-only and fused
//! position errors with the ambiguity success rate. A fused solution whose residuals fail a
//! 5σ check falls back to eLoran only (counted).
//!
//! # Limits
//!
//! Groundwave only (daytime; skywave contamination of AM/LW at night is not modelled);
//! homogeneous ground in the fusion study; SoOP residual propagation errors and eLoran
//! residual ASF are random per trial with stated σ.
//!
//! # Evidence
//!
//! MODELLED (internal tests: power scaling, Kalman convergence, Monte Carlo fusion gain).
//! No deployable-transmitter or SoOP field data has been compared.

use crate::geo::{direct, inverse, Geo};
use crate::plan::{weighted_hdop_m, Candidate, Coverage, PlanArea, PlanSettings, Planner};
use crate::prop::ground::{K_LORAN, N_ATM};
use crate::prop::smooth_earth::{Ground, GroundCurve};
use crate::prop::{PathKind, PropagationModel, C};
use crate::rx::dsp::solve;
use crate::scenario::Scenario;
use crate::timebase::gauss;
use crate::traj::RxState;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// LORAN carrier wavelength (m).
pub const LAMBDA_100K_M: f64 = C / 100_000.0;

// ───────────────────────────── deployable transmitters ─────────────────────────────

/// A low-power transportable transmitter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeployableTx {
    pub id: String,
    pub position: Geo,
    /// Transmitter output power (kW).
    pub tx_power_kw: f64,
    /// Mast height (m).
    pub antenna_height_m: f64,
    /// Top-loaded (h_eff = h) or plain mast (h_eff = h/2).
    #[serde(default)]
    pub top_loaded: bool,
    /// Antenna + ground-system loss resistance (Ω).
    pub loss_resistance_ohm: f64,
    /// Set-up time from arrival to on-air (min), for the record.
    #[serde(default)]
    pub setup_min: f64,
    /// 1σ of the unknown initial ASF + timing offset at switch-on (ns).
    #[serde(default)]
    pub initial_offset_sigma_ns: f64,
}

impl DeployableTx {
    /// Radiation resistance of the short monopole (Ω).
    pub fn radiation_resistance_ohm(&self) -> f64 {
        let h_eff = if self.top_loaded {
            self.antenna_height_m
        } else {
            0.5 * self.antenna_height_m
        };
        160.0 * std::f64::consts::PI.powi(2) * (h_eff / LAMBDA_100K_M).powi(2)
    }
    /// Antenna efficiency `R_rad / (R_rad + R_loss)`.
    pub fn efficiency(&self) -> f64 {
        let r = self.radiation_resistance_ohm();
        r / (r + self.loss_resistance_ohm.max(0.0))
    }
    /// Radiated power (kW).
    pub fn radiated_kw(&self) -> f64 {
        self.tx_power_kw.max(0.0) * self.efficiency()
    }
    /// The planner candidate (radiated power).
    pub fn candidate(&self) -> Candidate {
        Candidate {
            id: self.id.clone(),
            position: self.position,
            power_kw: self.radiated_kw(),
            cost: 0.0,
        }
    }
    fn station(&self) -> crate::chain::Station {
        crate::chain::Station {
            id: self.id.clone(),
            name: self.id.clone(),
            role: crate::signal::Role::Secondary,
            position: self.position,
            power_kw: self.radiated_kw(),
            emission_delay_us: 0.0,
            time_ref: String::new(),
            tx_clock: None,
            ecd_us: 0.0,
            status: Default::default(),
            faults: vec![],
        }
    }
}

/// Illustrative deployable-transmitter classes at `position` (assumed parameters for
/// studies, not vendor data): man-pack (1 kW, 12 m plain mast, 3 Ω), vehicle (10 kW, 30 m
/// top-loaded, 2 Ω), container (50 kW, 60 m top-loaded, 1 Ω).
pub fn templates(position: Geo) -> Vec<DeployableTx> {
    let mk = |id: &str, p: f64, h: f64, top: bool, r: f64, setup: f64, u: f64| DeployableTx {
        id: id.into(),
        position,
        tx_power_kw: p,
        antenna_height_m: h,
        top_loaded: top,
        loss_resistance_ohm: r,
        setup_min: setup,
        initial_offset_sigma_ns: u,
    };
    vec![
        mk("manpack", 1.0, 12.0, false, 3.0, 30.0, 1000.0),
        mk("vehicle", 10.0, 30.0, true, 2.0, 60.0, 1000.0),
        mk("container", 50.0, 60.0, true, 1.0, 240.0, 500.0),
    ]
}

/// Coverage of a deployment (all transmitters on) over `area` with the planner.
pub fn coverage(
    txs: &[DeployableTx],
    area: PlanArea,
    settings: PlanSettings,
    prop: &dyn PropagationModel,
) -> Result<Coverage, String> {
    let cands: Vec<Candidate> = txs.iter().map(|t| t.candidate()).collect();
    let n = cands.len();
    let p = Planner::new(cands, area, settings, prop)?;
    Ok(p.evaluate(&(0..n).collect::<Vec<_>>()))
}

/// Groundwave SNR (dB, noise RMS in 20 kHz) of `tx` at `at`.
pub fn snr_db(tx: &DeployableTx, at: Geo, prop: &dyn PropagationModel, noise_uv_m: f64) -> f64 {
    let rx = RxState {
        t_s: 0.0,
        pos: at,
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    let f = prop
        .paths(&tx.station(), &rx)
        .into_iter()
        .find(|p| p.kind == PathKind::Ground)
        .map(|p| p.field_uv_m)
        .unwrap_or(0.0);
    20.0 * (f.max(1e-30) / noise_uv_m).log10()
}

/// Distance (km) along `bearing_deg` at which the SNR falls to `min_snr_db` (bisection to
/// 0.1 km; `max_km` when it never does, 0 when it is below at 1 km).
pub fn service_radius_km(
    tx: &DeployableTx,
    prop: &dyn PropagationModel,
    noise_uv_m: f64,
    min_snr_db: f64,
    bearing_deg: f64,
    max_km: f64,
) -> f64 {
    let at = |km: f64| direct(tx.position, bearing_deg.to_radians(), km * 1000.0);
    let ok = |km: f64| snr_db(tx, at(km), prop, noise_uv_m) >= min_snr_db;
    if !ok(1.0) {
        return 0.0;
    }
    if ok(max_km) {
        return max_km;
    }
    let (mut lo, mut hi) = (1.0, max_km);
    while hi - lo > 0.1 {
        let m = 0.5 * (lo + hi);
        if ok(m) {
            lo = m;
        } else {
            hi = m;
        }
    }
    0.5 * (lo + hi)
}

/// A simulator scenario of a deployment: one chain (the first transmitter master, the
/// others secondaries 13 ms apart) at radiated power over homogeneous ground of
/// `ground_sigma_s_m`, receiver static at `receiver`; `offsets_ns[i]` is transmitter `i`'s
/// unknown initial offset (true − nominal arrival), as a transmitter clock error.
pub fn deployable_scenario(
    txs: &[DeployableTx],
    offsets_ns: &[f64],
    receiver: Geo,
    start_utc: &str,
    duration_s: f64,
    ground_sigma_s_m: f64,
) -> Result<Scenario, String> {
    if txs.is_empty() || txs.len() > 6 {
        return Err("deployable scenario: 1 to 6 transmitters".into());
    }
    let mut s = format!(
        "[scene]\nstart_utc = \"{start_utc}\"\nduration_s = {duration_s}\nfs_hz = 1000000\nseed = 1\n\
         output_scale_uv_m = 1000000.0\n[receiver]\ntype = \"static\"\nlat_deg = {}\nlon_deg = {}\n\
         [[outputs]]\ntype = \"e_field\"\n[propagation.ground]\nmodel = \"smooth_earth\"\nsigma_s_m = {ground_sigma_s_m}\n",
        receiver.lat_deg, receiver.lon_deg
    );
    for (i, t) in txs.iter().enumerate() {
        let off = offsets_ns.get(i).copied().unwrap_or(0.0);
        s += &format!(
            "[[time_refs]]\nid = \"tx:{}\"\noffset_ns = {}\n",
            t.id, -off
        );
    }
    s += "[[chains]]\nname = \"tactical\"\ngri_us = 89700\n";
    for (i, t) in txs.iter().enumerate() {
        s += &format!(
            "[[chains.stations]]\nid = \"{}\"\nrole = \"{}\"\nposition = {{ lat_deg = {}, lon_deg = {} }}\n\
             power_kw = {}\nemission_delay_us = {}\ntx_clock = \"tx:{}\"\n",
            t.id,
            if i == 0 { "master" } else { "secondary" },
            t.position.lat_deg,
            t.position.lon_deg,
            t.radiated_kw(),
            if i == 0 { 0.0 } else { 13_000.0 * i as f64 },
            t.id
        );
    }
    Scenario::from_toml(&s)
}

// ───────────────────────────── rapid ASF calibration ─────────────────────────────

/// One reference-receiver observation of the correction beyond PF (ns).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefObs {
    pub t_s: f64,
    pub value_ns: f64,
    pub sigma_ns: f64,
}

/// Rapid calibration filter settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RapidCalConfig {
    /// Prior mean and σ of the correction (ns): the planning model's value and its doubt.
    pub prior_ns: f64,
    pub prior_sigma_ns: f64,
    /// Random-walk process noise of the transmitter timing (ns²/s); 0 for a steered
    /// transmitter.
    pub process_ns2_per_s: f64,
    /// Accuracy at which the correction is declared usable (1σ, ns).
    pub target_sigma_ns: f64,
}

/// Filter state after one observation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalStep {
    pub t_s: f64,
    pub estimate_ns: f64,
    pub sigma_ns: f64,
    pub innovation_ns: f64,
}

/// Result of a rapid calibration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RapidCalResult {
    pub steps: Vec<CalStep>,
    /// First time the σ reached the target (s), if it did.
    pub usable_at_s: Option<f64>,
    pub estimate_ns: f64,
    pub sigma_ns: f64,
}

/// Scalar Kalman filter on the reference observations (see the module docs).
pub fn rapid_asf_calibration(
    obs: &[RefObs],
    cfg: &RapidCalConfig,
) -> Result<RapidCalResult, String> {
    if !(cfg.prior_sigma_ns > 0.0) || cfg.process_ns2_per_s < 0.0 {
        return Err("rapid calibration: prior σ > 0 and process noise ≥ 0 required".into());
    }
    let mut x = cfg.prior_ns;
    let mut p = cfg.prior_sigma_ns.powi(2);
    let mut t_prev = obs.first().map(|o| o.t_s).unwrap_or(0.0);
    let mut steps = Vec::with_capacity(obs.len());
    let mut usable_at = None;
    for o in obs {
        if !(o.sigma_ns > 0.0) || !o.value_ns.is_finite() || o.t_s < t_prev {
            return Err(
                "rapid calibration: observations need σ > 0, finite values, time order".into(),
            );
        }
        p += cfg.process_ns2_per_s * (o.t_s - t_prev);
        t_prev = o.t_s;
        let r = o.sigma_ns.powi(2);
        let k = p / (p + r);
        let innov = o.value_ns - x;
        x += k * innov;
        p *= 1.0 - k;
        let s = p.sqrt();
        if usable_at.is_none() && s <= cfg.target_sigma_ns {
            usable_at = Some(o.t_s);
        }
        steps.push(CalStep {
            t_s: o.t_s,
            estimate_ns: x,
            sigma_ns: s,
            innovation_ns: innov,
        });
    }
    Ok(RapidCalResult {
        steps,
        usable_at_s: usable_at,
        estimate_ns: x,
        sigma_ns: p.sqrt(),
    })
}

/// Correction beyond PF (ns) of `tx` at `at` from the propagation model (SF + ASF).
pub fn beyond_pf_ns(tx: &DeployableTx, at: Geo, prop: &dyn PropagationModel) -> f64 {
    let rx = RxState {
        t_s: 0.0,
        pos: at,
        heading_rad: 0.0,
        speed_mps: 0.0,
    };
    prop.paths(&tx.station(), &rx)
        .into_iter()
        .find(|p| p.kind == PathKind::Ground)
        .map(|p| p.sf_ns + p.asf_ns)
        .unwrap_or(f64::NAN)
}

/// Synthetic reference-receiver observations (SYNTHETIC): truth = model correction at the
/// site + `tx_offset_ns` (+ a random walk of `walk_ns2_per_s`), noise from the SNR at the
/// site through the planner's ranging-σ model; one observation every `interval_s`.
#[allow(clippy::too_many_arguments)]
pub fn reference_observations(
    tx: &DeployableTx,
    site: Geo,
    prop: &dyn PropagationModel,
    settings: &PlanSettings,
    tx_offset_ns: f64,
    walk_ns2_per_s: f64,
    n: usize,
    interval_s: f64,
    seed: u64,
) -> (Vec<RefObs>, Vec<f64>) {
    let base = beyond_pf_ns(tx, site, prop) + tx_offset_ns;
    let snr = snr_db(tx, site, prop, settings.noise_uv_m_20k);
    let sigma_ns = settings.ranging_sigma_m(snr) / C * 1e9;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut walk = 0.0;
    let mut obs = Vec::with_capacity(n);
    let mut truth = Vec::with_capacity(n);
    for i in 0..n {
        if i > 0 {
            walk += (walk_ns2_per_s * interval_s).sqrt() * gauss(&mut rng);
        }
        let t = base + walk;
        truth.push(t);
        obs.push(RefObs {
            t_s: i as f64 * interval_s,
            value_ns: t + sigma_ns * gauss(&mut rng),
            sigma_ns,
        });
    }
    (obs, truth)
}

/// Error (ns) of applying the reference site's correction at `user`: the difference of
/// the model corrections beyond PF (spatial decorrelation of SF + ASF).
pub fn spatial_residual_ns(
    tx: &DeployableTx,
    reference: Geo,
    user: Geo,
    prop: &dyn PropagationModel,
) -> f64 {
    beyond_pf_ns(tx, user, prop) - beyond_pf_ns(tx, reference, prop)
}

// ───────────────────────────── signals of opportunity ─────────────────────────────

/// A broadcast carrier used as a signal of opportunity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoopTx {
    pub id: String,
    pub position: Geo,
    pub freq_hz: f64,
    /// Radiated power (kW).
    pub power_kw: f64,
}

/// A fusion study set-up.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FusionScenario {
    pub eloran: Vec<Candidate>,
    pub soop: Vec<SoopTx>,
    /// SoOP reference station (known position and clock).
    pub reference: Geo,
    pub rover: Geo,
    /// Homogeneous ground (σ S/m, ε_r).
    pub ground_sigma_s_m: f64,
    pub ground_eps_r: f64,
    /// eLoran noise and ranging-σ model.
    pub settings: PlanSettings,
    /// SoOP noise field in the carrier-tracking bandwidth (µV/m) and phase noise per
    /// receiver at `soop_snr0_db` (deg), scaling like the eLoran σ, floored at 0.2°.
    pub soop_noise_uv_m: f64,
    pub soop_phase_noise_deg: f64,
    pub soop_snr0_db: f64,
    /// Residual eLoran ASF after calibration (1σ, ns, random per station per trial).
    pub eloran_residual_ns: f64,
    /// Residual differential SoOP propagation error (1σ, ns, per carrier per trial).
    pub soop_residual_ns: f64,
}

/// Fusion study result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FusionStudy {
    pub trials: usize,
    /// Weighted HDOP-based 1σ horizontal (m), from the stated σ.
    pub hdop_eloran_m: Option<f64>,
    pub hdop_fused_m: Option<f64>,
    pub rms_eloran_m: f64,
    pub rms_fused_m: f64,
    pub p95_eloran_m: f64,
    pub p95_fused_m: f64,
    /// Share of carriers whose cycle ambiguity was resolved correctly.
    pub ambiguity_success: f64,
    /// Trials whose fused fix failed the residual check (eLoran-only used).
    pub fallbacks: usize,
    /// Per-signal stated σ (m): eLoran stations then SoOP carriers.
    pub sigma_m: Vec<f64>,
}

struct Ranger {
    curves: Vec<GroundCurve>,
}

impl Ranger {
    /// Range-equivalent phase delay (m) on curve `k` over distance `d`.
    fn range_m(&self, k: usize, d: f64) -> f64 {
        N_ATM * d + C * self.curves[k].at(d).phase_delay_s
    }
    fn field_uv_m(&self, k: usize, d: f64, power_kw: f64) -> f64 {
        crate::prop::ground::E1_UV_M_PER_SQRT_KW_AT_1KM * power_kw.max(0.0).sqrt()
            / (d / 1000.0).max(1e-3)
            * self.curves[k].at(d).ln_amp.exp()
    }
}

/// One ranging observation `y = R(p) + clock + e` (m).
#[derive(Clone, Copy)]
struct RangeObs {
    tx: Geo,
    curve: usize,
    y_m: f64,
    sigma_m: f64,
}

/// Weighted least squares (east, north, clock) by Gauss–Newton from `p0`.
fn wls_fix(ranger: &Ranger, obs: &[RangeObs], p0: Geo, c0: f64) -> Option<(Geo, f64, Vec<f64>)> {
    if obs.len() < 3 {
        return None;
    }
    let (mut p, mut c) = (p0, c0);
    for _ in 0..20 {
        let mut a = vec![vec![0.0; 3]; 3];
        let mut b = vec![0.0; 3];
        for o in obs {
            let inv = inverse(p, o.tx);
            let h = [-inv.az1_rad.sin(), -inv.az1_rad.cos(), 1.0];
            let r = o.y_m - (ranger.range_m(o.curve, inv.distance_m) + c);
            let w = 1.0 / (o.sigma_m * o.sigma_m);
            for i in 0..3 {
                b[i] += w * h[i] * r;
                for j in 0..3 {
                    a[i][j] += w * h[i] * h[j];
                }
            }
        }
        let dx = solve(&a, &b)?;
        let step = dx[0].hypot(dx[1]);
        if step > 0.0 {
            p = direct(p, dx[0].atan2(dx[1]), step);
        }
        c += dx[2];
        if step < 1e-4 && dx[2].abs() < 1e-4 {
            break;
        }
    }
    let res = obs
        .iter()
        .map(|o| {
            let d = inverse(p, o.tx).distance_m;
            (o.y_m - ranger.range_m(o.curve, d) - c) / o.sigma_m
        })
        .collect();
    Some((p, c, res))
}

fn percentile(v: &mut [f64], q: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let i = ((v.len() as f64 - 1.0) * q).round() as usize;
    v[i]
}

/// The SoOP fusion accuracy study (see the module docs).
pub fn fusion_study(sc: &FusionScenario, trials: usize, seed: u64) -> Result<FusionStudy, String> {
    if sc.eloran.len() < 3 {
        return Err("fusion study: at least 3 eLoran stations".into());
    }
    if trials == 0 {
        return Err("fusion study: trials must be > 0".into());
    }
    let ground = Ground {
        sigma: sc.ground_sigma_s_m,
        eps_r: sc.ground_eps_r,
    };
    let all: Vec<Geo> = sc
        .eloran
        .iter()
        .map(|c| c.position)
        .chain(sc.soop.iter().map(|s| s.position))
        .collect();
    let d_max = all
        .iter()
        .flat_map(|t| {
            [
                inverse(*t, sc.rover).distance_m,
                inverse(*t, sc.reference).distance_m,
            ]
        })
        .fold(0.0f64, f64::max)
        * 1.3
        + 100_000.0;
    let mut freqs = vec![100_000.0];
    freqs.extend(sc.soop.iter().map(|s| s.freq_hz));
    let ranger = Ranger {
        curves: freqs
            .iter()
            .map(|f| GroundCurve::new(ground, *f, K_LORAN, N_ATM, d_max, 400))
            .collect(),
    };
    // stated σ per signal
    let mut sigma_m = Vec::new();
    let mut bearings = Vec::new();
    for c in &sc.eloran {
        let inv = inverse(sc.rover, c.position);
        let snr = 20.0
            * (ranger.field_uv_m(0, inv.distance_m, c.power_kw) / sc.settings.noise_uv_m_20k)
                .log10();
        let s = sc.settings.ranging_sigma_m(snr);
        sigma_m.push((s * s + (sc.eloran_residual_ns * 1e-9 * C).powi(2)).sqrt());
        bearings.push(inv.az1_rad);
    }
    let ne = sc.eloran.len();
    let mut lambdas = Vec::new();
    for (k, s) in sc.soop.iter().enumerate() {
        let lam = C / s.freq_hz;
        lambdas.push(lam);
        let snr_at = |p: Geo| {
            let d = inverse(p, s.position).distance_m;
            20.0 * (ranger.field_uv_m(k + 1, d, s.power_kw) / sc.soop_noise_uv_m).log10()
        };
        let ph = |snr: f64| {
            (sc.soop_phase_noise_deg * 10f64.powf(-(snr - sc.soop_snr0_db) / 20.0)).max(0.2)
        };
        let ph_rov = ph(snr_at(sc.rover));
        let ph_ref = ph(snr_at(sc.reference));
        let s_phase = lam * (ph_rov.powi(2) + ph_ref.powi(2)).sqrt() / 360.0;
        sigma_m.push((s_phase * s_phase + (sc.soop_residual_ns * 1e-9 * C).powi(2)).sqrt());
        bearings.push(inverse(sc.rover, s.position).az1_rad);
    }
    let hdop_eloran = weighted_hdop_m(&bearings[..ne], &sigma_m[..ne]);
    let hdop_fused = weighted_hdop_m(&bearings, &sigma_m);

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let (mut err_e, mut err_f) = (Vec::new(), Vec::new());
    let (mut amb_ok, mut amb_n, mut fallbacks) = (0usize, 0usize, 0usize);
    for _ in 0..trials {
        let clock_m: f64 = rng.gen_range(-3000.0..3000.0);
        // eLoran pseudoranges (residual ASF + noise, components drawn separately)
        let mut obs = Vec::new();
        for (i, c) in sc.eloran.iter().enumerate() {
            let d = inverse(sc.rover, c.position).distance_m;
            let noise_only = (sigma_m[i].powi(2) - (sc.eloran_residual_ns * 1e-9 * C).powi(2))
                .max(0.0)
                .sqrt();
            let e =
                noise_only * gauss(&mut rng) + sc.eloran_residual_ns * 1e-9 * C * gauss(&mut rng);
            obs.push(RangeObs {
                tx: c.position,
                curve: 0,
                y_m: ranger.range_m(0, d) + clock_m + e,
                sigma_m: sigma_m[i],
            });
        }
        let start = sc.reference; // a known nearby point
        let Some((pe, ce, _)) = wls_fix(&ranger, &obs, start, 0.0) else {
            continue;
        };
        err_e.push(inverse(pe, sc.rover).distance_m);
        // SoOP: differential phase (transmitter phase cancels), cycles resolved against
        // the eLoran-only fix
        let mut fused = obs.clone();
        for (k, s) in sc.soop.iter().enumerate() {
            let lam = lambdas[k];
            let d_rov = inverse(sc.rover, s.position).distance_m;
            let d_ref = inverse(sc.reference, s.position).distance_m;
            let r_ref = ranger.range_m(k + 1, d_ref);
            let true_m = ranger.range_m(k + 1, d_rov) - r_ref + clock_m;
            let e = sigma_m[ne + k] * gauss(&mut rng);
            let phase = (true_m + e) / lam;
            let frac = phase.rem_euclid(1.0);
            let n_true = (phase - frac).round();
            let pred =
                (ranger.range_m(k + 1, inverse(pe, s.position).distance_m) - r_ref + ce) / lam;
            let n = (pred - frac).round();
            amb_n += 1;
            if n == n_true {
                amb_ok += 1;
            }
            fused.push(RangeObs {
                tx: s.position,
                curve: k + 1,
                y_m: (frac + n) * lam + r_ref,
                sigma_m: sigma_m[ne + k],
            });
        }
        let pick = match wls_fix(&ranger, &fused, pe, ce) {
            Some((pf, _, res)) if res.iter().all(|r| r.abs() <= 5.0) => pf,
            _ => {
                fallbacks += 1;
                pe
            }
        };
        err_f.push(inverse(pick, sc.rover).distance_m);
    }
    let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len().max(1) as f64).sqrt();
    Ok(FusionStudy {
        trials: err_e.len(),
        hdop_eloran_m: hdop_eloran,
        hdop_fused_m: hdop_fused,
        rms_eloran_m: rms(&err_e),
        rms_fused_m: rms(&err_f),
        p95_eloran_m: percentile(&mut err_e.clone(), 0.95),
        p95_fused_m: percentile(&mut err_f.clone(), 0.95),
        ambiguity_success: if amb_n > 0 {
            amb_ok as f64 / amb_n as f64
        } else {
            f64::NAN
        },
        fallbacks,
        sigma_m,
    })
}

/// A weak-geometry fusion template (synthetic sites, not real stations): three eLoran
/// stations within ~50° of azimuth north of the rover, two LW carriers to the south-east
/// and south-west, reference station 20 km from the rover.
pub fn weak_geometry_template() -> FusionScenario {
    let rover = Geo::new(52.0, 5.0);
    let cand = |id: &str, az: f64, km: f64, p: f64| Candidate {
        id: id.into(),
        position: direct(rover, az.to_radians(), km * 1000.0),
        power_kw: p,
        cost: 0.0,
    };
    let soop = |id: &str, az: f64, km: f64, f: f64, p: f64| SoopTx {
        id: id.into(),
        position: direct(rover, az.to_radians(), km * 1000.0),
        freq_hz: f,
        power_kw: p,
    };
    FusionScenario {
        eloran: vec![
            cand("A", -25.0, 400.0, 400.0),
            cand("B", 0.0, 600.0, 400.0),
            cand("C", 25.0, 450.0, 400.0),
        ],
        soop: vec![
            soop("LW1", 135.0, 300.0, 183_000.0, 500.0),
            soop("LW2", 225.0, 350.0, 234_000.0, 300.0),
        ],
        reference: direct(rover, 90f64.to_radians(), 20_000.0),
        rover,
        ground_sigma_s_m: 0.01,
        ground_eps_r: 15.0,
        settings: PlanSettings::default(),
        soop_noise_uv_m: 5.0,
        soop_phase_noise_deg: 2.0,
        soop_snr0_db: 30.0,
        eloran_residual_ns: 20.0,
        soop_residual_ns: 10.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prop::ground::GroundConfig;
    use crate::prop::BuildContext;
    use crate::time::AbsTime;

    fn land() -> Box<dyn PropagationModel> {
        let cfg = GroundConfig::SmoothEarth {
            sigma_s_m: 0.01,
            eps_r: None,
            k_factor: K_LORAN,
            temporal: None,
        };
        let ctx = BuildContext::new(
            AbsTime::from_utc_iso("2026-11-19T12:00:00Z").unwrap(),
            1.0,
            0,
        );
        crate::prop::ground::build(&cfg, &ctx).unwrap()
    }

    fn trio(power_kw: f64) -> Vec<DeployableTx> {
        let c = Geo::new(50.0, 10.0);
        [0.0, 120.0, 240.0]
            .iter()
            .enumerate()
            .map(|(i, az)| DeployableTx {
                id: format!("T{i}"),
                position: direct(c, f64::to_radians(*az), 80_000.0),
                tx_power_kw: power_kw,
                antenna_height_m: 30.0,
                top_loaded: true,
                loss_resistance_ohm: 2.0,
                setup_min: 60.0,
                initial_offset_sigma_ns: 1000.0,
            })
            .collect()
    }

    #[test]
    fn short_monopole_efficiency_and_power() {
        let t = &templates(Geo::new(50.0, 10.0));
        // 30 m top-loaded: R_rad = 160 π² (30/2998)² ≈ 0.158 Ω
        let v = &t[1];
        assert!(
            (v.radiation_resistance_ohm() - 0.1581).abs() < 1e-3,
            "{}",
            v.radiation_resistance_ohm()
        );
        assert!((v.efficiency() - 0.1581 / 2.1581).abs() < 1e-3);
        // a plain mast has a quarter of the radiation resistance of a top-loaded one
        let mut plain = v.clone();
        plain.top_loaded = false;
        assert!(
            (plain.radiation_resistance_ohm() * 4.0 - v.radiation_resistance_ohm()).abs() < 1e-12
        );
        // classes ordered by radiated power
        assert!(t[0].radiated_kw() < t[1].radiated_kw() && t[1].radiated_kw() < t[2].radiated_kw());
        assert!(t[0].radiated_kw() < 0.01, "{}", t[0].radiated_kw());
    }

    #[test]
    fn coverage_shrinks_with_power_as_expected() {
        let prop = land();
        let noise = 50.0;
        let tx = &trio(10.0)[0];
        // SNR scales with √P: 10× less power is exactly −10 dB everywhere
        let mut low = tx.clone();
        low.tx_power_kw = 1.0;
        for km in [20.0, 100.0, 300.0] {
            let p = direct(tx.position, 1.0, km * 1000.0);
            let d = snr_db(tx, p, prop.as_ref(), noise) - snr_db(&low, p, prop.as_ref(), noise);
            assert!((d - 10.0).abs() < 1e-9, "{d}");
        }
        // service radius strictly shrinks with power and with a shorter mast
        let r = |t: &DeployableTx| service_radius_km(t, prop.as_ref(), noise, 0.0, 45.0, 2000.0);
        let powers = [50.0, 10.0, 2.0, 0.5];
        let radii: Vec<f64> = powers
            .iter()
            .map(|p| {
                let mut t = tx.clone();
                t.tx_power_kw = *p;
                r(&t)
            })
            .collect();
        assert!(radii.windows(2).all(|w| w[1] < w[0] - 1.0), "{radii:?}");
        assert!(radii[0] < 2000.0 && radii[3] > 1.0, "{radii:?}");
        let mut short = tx.clone();
        short.antenna_height_m = 10.0;
        assert!(r(&short) < r(tx));
        // area coverage of a three-transmitter deployment does not grow as power drops,
        // and drops substantially between the extremes
        let area = PlanArea {
            lat_min: 48.5,
            lat_max: 51.5,
            lon_min: 7.5,
            lon_max: 12.5,
            step_deg: 0.25,
        };
        let mut settings = PlanSettings::default();
        settings.accuracy_target_m = 250.0;
        settings.availability_target = 0.0;
        let cov: Vec<f64> = [50.0, 10.0, 2.0, 0.5]
            .iter()
            .map(|p| {
                coverage(&trio(*p), area.clone(), settings.clone(), prop.as_ref())
                    .unwrap()
                    .coverage
            })
            .collect();
        assert!(cov.windows(2).all(|w| w[1] <= w[0]), "{cov:?}");
        assert!(cov[0] > 0.5 && cov[3] < 0.5 * cov[0], "{cov:?}");
    }

    #[test]
    fn rapid_asf_calibration_converges() {
        let prop = land();
        let tx = trio(10.0).remove(0);
        let site = direct(tx.position, 2.0, 40_000.0);
        let settings = PlanSettings::default();
        let offset = 730.0; // unknown at switch-on
        let (obs, truth) = reference_observations(
            &tx,
            site,
            prop.as_ref(),
            &settings,
            offset,
            0.0,
            200,
            1.0,
            11,
        );
        let cfg = RapidCalConfig {
            prior_ns: beyond_pf_ns(&tx, site, prop.as_ref()),
            prior_sigma_ns: tx.initial_offset_sigma_ns,
            process_ns2_per_s: 0.0,
            target_sigma_ns: 5.0,
        };
        let r = rapid_asf_calibration(&obs, &cfg).unwrap();
        let s_meas = obs[0].sigma_ns;
        // converges to the truth within its uncertainty; σ ≈ σ_meas/√n
        assert!(
            (r.estimate_ns - truth[0]).abs() < 3.0 * r.sigma_ns,
            "{} vs {}",
            r.estimate_ns,
            truth[0]
        );
        assert!((r.sigma_ns - s_meas / 200f64.sqrt()).abs() < 0.01 * r.sigma_ns + 1e-9);
        // σ never grows without process noise, and the error shrinks with time
        assert!(r.steps.windows(2).all(|w| w[1].sigma_ns <= w[0].sigma_ns));
        let e10 = (r.steps[9].estimate_ns - truth[0]).abs();
        assert!(e10 < 3.0 * r.steps[9].sigma_ns);
        // usable at the first n with 1/(1/σ_prior² + n/σ_meas²) ≤ target²
        let n_need = (s_meas.powi(2) * (1.0 / 25.0 - 1.0 / cfg.prior_sigma_ns.powi(2)))
            .ceil()
            .max(1.0) as usize;
        assert_eq!(r.usable_at_s, Some((n_need - 1) as f64), "σ_meas {s_meas}");
        // the batch estimator agrees (no process noise, flat prior negligible)
        let tob: Vec<crate::valid::inverse::ToaObservation> = obs
            .iter()
            .map(|o| crate::valid::inverse::ToaObservation {
                site: "ref".into(),
                site_pos: site,
                chain: "tactical".into(),
                station: tx.id.clone(),
                station_pos: tx.position,
                epoch_s: o.t_s,
                pseudo_delay_ns: o.value_ns
                    + N_ATM * inverse(tx.position, site).distance_m / C * 1e9,
                sigma_ns: o.sigma_ns,
            })
            .collect();
        let mut acfg = crate::valid::inverse::AsfConfig::default();
        acfg.site_clock_ns.insert("ref".into(), 0.0);
        let batch = crate::valid::inverse::estimate_asf(&tob, &acfg).unwrap();
        assert!(
            (batch[0].beyond_pf_ns - r.estimate_ns).abs() < 0.1,
            "{} {}",
            batch[0].beyond_pf_ns,
            r.estimate_ns
        );
        // with transmitter clock wander the filter keeps a floor and still tracks
        let (obs_w, truth_w) = reference_observations(
            &tx,
            site,
            prop.as_ref(),
            &settings,
            offset,
            0.5,
            600,
            1.0,
            12,
        );
        let mut cw = cfg;
        cw.process_ns2_per_s = 0.5;
        let rw = rapid_asf_calibration(&obs_w, &cw).unwrap();
        let last = rw.steps.last().unwrap();
        assert!(last.sigma_ns > r.sigma_ns);
        assert!((last.estimate_ns - truth_w.last().unwrap()).abs() < 4.0 * last.sigma_ns);
        // the correction's validity shrinks with distance from the reference site
        let near = spatial_residual_ns(&tx, site, direct(site, 0.5, 5_000.0), prop.as_ref());
        let far = spatial_residual_ns(&tx, site, direct(site, 0.5, 80_000.0), prop.as_ref());
        assert!(near.abs() < far.abs(), "{near} {far}");
    }

    #[test]
    fn soop_fusion_improves_accuracy_in_weak_geometry() {
        let sc = weak_geometry_template();
        let st = fusion_study(&sc, 400, 5).unwrap();
        assert_eq!(st.trials, 400);
        let (he, hf) = (st.hdop_eloran_m.unwrap(), st.hdop_fused_m.unwrap());
        assert!(hf < 0.5 * he, "HDOP-σ {he} → {hf} m");
        assert!(st.ambiguity_success > 0.98, "{}", st.ambiguity_success);
        assert!(
            st.rms_fused_m < 0.5 * st.rms_eloran_m,
            "{} → {}",
            st.rms_eloran_m,
            st.rms_fused_m
        );
        assert!(st.p95_fused_m < st.p95_eloran_m);
        // the Monte Carlo agrees with the stated geometry to within ±40 %
        assert!(
            (st.rms_eloran_m / he - 1.0).abs() < 0.4,
            "{} vs {he}",
            st.rms_eloran_m
        );
        assert!(
            (st.rms_fused_m / hf - 1.0).abs() < 0.4,
            "{} vs {hf}",
            st.rms_fused_m
        );
        // without SoOP carriers there is nothing to gain
        let mut none = sc.clone();
        none.soop.clear();
        let s0 = fusion_study(&none, 100, 5).unwrap();
        assert!((s0.rms_fused_m - s0.rms_eloran_m).abs() < 1e-9);
    }

    #[test]
    fn deployable_scenario_runs_in_the_simulator() {
        let txs = trio(10.0);
        let rx = Geo::new(50.0, 10.0);
        let sc = deployable_scenario(
            &txs,
            &[0.0, 250.0, -400.0],
            rx,
            "2026-11-19T12:00:00Z",
            0.3,
            0.01,
        )
        .unwrap();
        let mut scene = sc.build().unwrap();
        let mut sink = crate::scene::MemSink::default();
        let mut truth = Vec::new();
        scene.run(&mut sink, &mut truth).unwrap();
        assert!(!truth.is_empty());
        assert_eq!(sc.chains[0].chain.stations.len(), 3);
        assert!((sc.chains[0].chain.stations[1].power_kw - txs[1].radiated_kw()).abs() < 1e-12);
        // the unknown offset makes the true emission later by 250 ns for T1
        let t1 = sc.time_refs.iter().find(|t| t.id == "tx:T1").unwrap();
        assert_eq!(t1.offset_ns, -250.0);
    }
}
