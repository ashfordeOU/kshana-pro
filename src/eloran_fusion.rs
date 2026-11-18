// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Hybrid eLoran/GNSS positioning and integrity
//!
//! One estimator for both systems, in the range domain:
//!
//! * **GNSS**: code pseudoranges already corrected for the satellite clock, ionosphere and
//!   troposphere (from the open engine's `pvt::assemble_epoch` on a RINEX file, or from any
//!   simulator), modelled as `ρ = |r_sat − r| + b_G`, with `b_G = c·(receiver clock − GPS time)`;
//! * **eLoran**: pseudo-delays from the software receiver (`kshana_eloran::rx::Observation`:
//!   arrival minus nominal emission) less the user's propagation corrections (ASF…), modelled
//!   exactly as the receiver's own solver does, `c·τ = η·d_geodesic(r, station) + b_E`, with
//!   `η = 1.000338` (the Primary Factor) and heights ignored (LF groundwave on the ellipsoid);
//! * the two clock biases are **separate**: `b_E = b_G + δ`, where `δ = c·(GPS time − eLoran
//!   time scale)` is the **time-scale offset** between the systems (UTC(k) steering of the
//!   eLoran stations against GPS time). It can be given a prior (from the broadcast GPS–UTC
//!   parameters and the eLoran operator's UTC(k) offsets) or learnt while both are available.
//!
//! State `x = [E, N, U, b_G, δ]` (position increments in the local geodetic frame, metres).
//! Each epoch is an iterated maximum-a-posteriori solve (a Gauss–Newton weighted least squares
//! with the predicted state as a prior, i.e. an iterated extended Kalman filter update for a
//! random-walk state); [`HybridFilter`] propagates it with process noise between epochs. With
//! no GNSS the solution continues on eLoran (horizontal position and `b_E`; height and `δ`
//! carried by the filter, or a height aid), and `b_G = b_E − δ` keeps GNSS time.
//!
//! ## Integrity
//!
//! Three residual tests, each against the χ² quantile for the configured false-alarm
//! probability ([`kshana::raim::chi2_quantile`]):
//!
//! 1. **Global** consistency of the hybrid solve: the minimum of the MAP cost (normalised
//!    measurement residuals plus the prior's normalised innovation) is χ² with
//!    `m + n_prior − 5` degrees of freedom.
//! 2. **GNSS snapshot RAIM** (≥ 5 satellites): the open engine's [`kshana::raim::snapshot_raim`]
//!    on the residuals of a GNSS-only fix ([`kshana::pvt::solve_spp`]).
//! 3. **Cross-system** test against eLoran (≥ 3 stations): the GNSS-only and the eLoran-only
//!    fixes are compared horizontally (two-axis separation statistic,
//!    [`kshana::cross_raim::separation_statistic_axis`]) and in time (the eLoran-minus-GNSS
//!    clock difference against the learnt or stated time-scale offset). A self-consistent
//!    GNSS spoof (all pseudoranges from a false position or a false clock) passes RAIM but
//!    fails this test.
//!
//! When the cross-system test or the global test flags and GNSS RAIM cannot clear GNSS alone,
//! the filter (with [`FusionConfig::fallback_to_eloran`]) excludes GNSS for the epoch and
//! reports the eLoran solution, so a spoofer cannot drag the state.
//!
//! An inertial sensor is not modelled here (the state has no velocity or attitude); a
//! moving platform is handled through the position process noise.
//!
//! MODELLED: measurement models are the receivers' own; noise levels are the user's stated
//! σ; the tests are snapshot tests with Gaussian assumptions (no fault priors, no protection
//! levels beyond the engine RAIM's own).

use kshana::frames::{ecef_to_geodetic, geodetic_to_ecef, Geodetic};
use kshana::pvt::{solve_spp, SppMeasurement};
use kshana::raim::{chi2_quantile, snapshot_raim, RaimResult};
use kshana::rinex::RinexEphemeris;
use kshana_eloran::chain::Chain;
use kshana_eloran::geo::{inverse, Geo};
use kshana_eloran::prop::ground::N_ATM;
use kshana_eloran::rx::{Corrections, Observation};
use serde::{Deserialize, Serialize};

/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;
const NS: usize = 5;
/// A prior variance above this (m²) is treated as uninformative.
const WEAK_VAR: f64 = 1e10;

/// One GNSS pseudorange, corrected (satellite clock, ionosphere, troposphere removed).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GnssRange {
    pub id: String,
    /// Satellite position at transmission, rotated to the reception frame (ECEF, m).
    pub sat_ecef: [f64; 3],
    /// `ρ + c·δt_sv − I − T` (m).
    pub range_m: f64,
    pub sigma_m: f64,
}

impl GnssRange {
    /// From the open engine's SPP measurement (`pvt::assemble_epoch`).
    pub fn from_spp(id: &str, m: &SppMeasurement, sigma_m: f64) -> GnssRange {
        GnssRange {
            id: id.into(),
            sat_ecef: m.sat_ecef,
            range_m: m.pseudorange_m + m.sat_clock_m - m.iono_m - m.tropo_m,
            sigma_m,
        }
    }
}

/// One eLoran station range: `c·(pseudo-delay − correction)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EloranRange {
    /// `chain/station`.
    pub id: String,
    pub station: Geo,
    /// `c·(arrival − nominal emission) − c·correction` (m).
    pub range_m: f64,
    pub sigma_m: f64,
}

impl EloranRange {
    /// From the software receiver's observations, with the user's corrections (ASF, ns).
    /// σ is `max(c·std/√n, sigma_floor_m)`.
    pub fn from_observations(
        obs: &[Observation],
        chains: &[Chain],
        corr: &Corrections,
        sigma_floor_m: f64,
    ) -> Vec<EloranRange> {
        obs.iter()
            .map(|o| {
                let st = &chains[o.chain_index].stations[o.station_index];
                let s = if o.n > 1 {
                    C * o.std_s / (o.n as f64).sqrt()
                } else {
                    0.0
                };
                EloranRange {
                    id: Corrections::key(&o.chain, &o.station),
                    station: st.position,
                    range_m: C * (o.pseudo_delay_s - corr.get(&o.chain, &o.station) * 1e-9),
                    sigma_m: s.max(sigma_floor_m),
                }
            })
            .collect()
    }
}

/// Estimator settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FusionConfig {
    /// Starting point (ECEF) when the filter has no state; default: the eLoran stations'
    /// centroid on the ellipsoid.
    pub initial_ecef: Option<[f64; 3]>,
    /// Prior on `δ` (GPS time − eLoran time scale, ns) and its σ (ns).
    pub offset_prior_ns: Option<(f64, f64)>,
    /// Height aid (ellipsoidal m, σ m), used when the GNSS cannot fix height.
    pub height_aid: Option<(f64, f64)>,
    /// Process noise spectral densities (m²/s): horizontal, vertical, GNSS clock, offset δ.
    pub q_horizontal: f64,
    pub q_vertical: f64,
    pub q_clock: f64,
    pub q_offset: f64,
    /// False-alarm and missed-detection probabilities of the tests.
    pub p_fa: f64,
    pub p_md: f64,
    /// Exclude GNSS for an epoch when the cross-system or global test flags.
    pub fallback_to_eloran: bool,
    pub max_iter: usize,
}

impl Default for FusionConfig {
    fn default() -> Self {
        FusionConfig {
            initial_ecef: None,
            offset_prior_ns: None,
            height_aid: None,
            q_horizontal: 1.0,
            q_vertical: 1.0,
            q_clock: 100.0,
            q_offset: 1e-4,
            p_fa: 1e-5,
            p_md: 1e-3,
            fallback_to_eloran: true,
            max_iter: 20,
        }
    }
}

/// Which measurements a fix used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Hybrid,
    GnssOnly,
    EloranOnly,
    /// Prediction only (no measurement).
    Coast,
}

/// A χ² test outcome.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chi2Test {
    pub statistic: f64,
    pub dof: f64,
    pub threshold: f64,
    pub flagged: bool,
}

impl Chi2Test {
    fn new(statistic: f64, dof: f64, p_fa: f64) -> Option<Chi2Test> {
        if dof < 1.0 {
            return None;
        }
        let threshold = chi2_quantile(1.0 - p_fa, dof);
        Some(Chi2Test {
            statistic,
            dof,
            threshold,
            flagged: statistic > threshold,
        })
    }
}

/// GNSS against eLoran.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrossCheck {
    /// GNSS-only minus eLoran-only fix, east and north (m).
    pub d_east_m: f64,
    pub d_north_m: f64,
    pub horizontal: Chi2Test,
    /// `(b_E − b_G) − δ_ref` (ns), where `δ_ref` is the learnt or stated offset.
    pub time_innovation_ns: f64,
    pub time: Option<Chi2Test>,
    pub flagged: bool,
}

/// The integrity outcome of one epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Integrity {
    pub global: Option<Chi2Test>,
    #[serde(skip)]
    pub gnss_raim: Option<RaimResult>,
    pub cross: Option<CrossCheck>,
    /// Any test flagged.
    pub flagged: bool,
    /// GNSS was excluded for this epoch.
    pub gnss_excluded: bool,
    pub reasons: Vec<String>,
}

/// One epoch's solution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FusionFix {
    pub t_s: f64,
    pub mode: Mode,
    pub ecef: [f64; 3],
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub h_m: f64,
    /// Receiver clock − GPS time (ns).
    pub gnss_clock_ns: f64,
    /// Receiver clock − eLoran time scale (ns).
    pub eloran_clock_ns: f64,
    /// GPS time − eLoran time scale (ns).
    pub offset_ns: f64,
    /// One-sigma: east, north, up (m), GNSS clock, offset (ns).
    pub sigma: [f64; 5],
    pub n_gnss: usize,
    pub n_eloran: usize,
    /// Post-fit residuals (id, m).
    pub residuals_m: Vec<(String, f64)>,
    pub integrity: Integrity,
}

// ------------------------------------------------------------------ small linear algebra

type Mat = Vec<Vec<f64>>;

fn invert(a: &Mat) -> Option<Mat> {
    let n = a.len();
    let mut m = a.clone();
    let mut inv: Mat = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[p][c].abs() < 1e-300 {
            return None;
        }
        m.swap(c, p);
        inv.swap(c, p);
        let d = m[c][c];
        for j in 0..n {
            m[c][j] /= d;
            inv[c][j] /= d;
        }
        for i in 0..n {
            if i != c {
                let f = m[i][c];
                if f != 0.0 {
                    for j in 0..n {
                        m[i][j] -= f * m[c][j];
                        inv[i][j] -= f * inv[c][j];
                    }
                }
            }
        }
    }
    Some(inv)
}

fn zeros(n: usize) -> Mat {
    vec![vec![0.0; n]; n]
}

/// Geodetic east, north, up unit vectors (ECEF) at `r`.
fn enu(r: [f64; 3]) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let g = ecef_to_geodetic(r);
    let (sla, cla) = g.lat_rad.sin_cos();
    let (slo, clo) = g.lon_rad.sin_cos();
    (
        [-slo, clo, 0.0],
        [-sla * clo, -sla * slo, cla],
        [cla * clo, cla * slo, sla],
    )
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn to_geo(r: [f64; 3]) -> Geo {
    let g = ecef_to_geodetic(r);
    Geo {
        lat_deg: g.lat_rad.to_degrees(),
        lon_deg: g.lon_rad.to_degrees(),
        h_m: g.alt_m,
    }
}

/// ECEF of a geodetic point.
pub fn geo_to_ecef(g: Geo) -> [f64; 3] {
    geodetic_to_ecef(Geodetic {
        lat_rad: g.lat_deg.to_radians(),
        lon_rad: g.lon_deg.to_radians(),
        alt_m: g.h_m,
    })
}

// ------------------------------------------------------------------ the core solve

/// A state estimate: position, GNSS clock bias and offset (m), covariance in the local
/// geodetic frame at `pos`.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub pos: [f64; 3],
    pub b_gnss_m: f64,
    pub offset_m: f64,
    pub cov: [[f64; NS]; NS],
}

impl State {
    fn weak(pos: [f64; 3]) -> State {
        let mut cov = [[0.0; NS]; NS];
        for (i, row) in cov.iter_mut().enumerate() {
            row[i] = 1e12;
        }
        State {
            pos,
            b_gnss_m: 0.0,
            offset_m: 0.0,
            cov,
        }
    }
}

struct Solved {
    state: State,
    cost: f64,
    m: usize,
    n_prior: usize,
    residuals: Vec<(String, f64)>,
}

/// Iterated MAP solve of `gnss`, `eloran` and the optional height aid against `prior`.
fn solve(
    prior: &State,
    gnss: &[GnssRange],
    eloran: &[EloranRange],
    height_aid: Option<(f64, f64)>,
    max_iter: usize,
) -> Option<Solved> {
    let pinv = invert(&prior.cov.iter().map(|r| r.to_vec()).collect())?;
    let n_prior = (0..NS).filter(|&i| prior.cov[i][i] < WEAK_VAR).count();
    let mut x = (prior.pos, prior.b_gnss_m, prior.offset_m);
    let eval = |x: &([f64; 3], f64, f64)| {
        // rows (h, residual, weight, id)
        let (e, n, u) = enu(x.0);
        let mut rows: Vec<([f64; NS], f64, f64, String)> = Vec::new();
        for g in gnss {
            let d = sub(g.sat_ecef, x.0);
            let rho = norm(d);
            let l = [d[0] / rho, d[1] / rho, d[2] / rho];
            rows.push((
                [-dot(l, e), -dot(l, n), -dot(l, u), 1.0, 0.0],
                g.range_m - (rho + x.1),
                1.0 / (g.sigma_m * g.sigma_m),
                g.id.clone(),
            ));
        }
        if !eloran.is_empty() {
            let here = to_geo(x.0);
            for s in eloran {
                let inv = inverse(here, s.station);
                rows.push((
                    [
                        -N_ATM * inv.az1_rad.sin(),
                        -N_ATM * inv.az1_rad.cos(),
                        0.0,
                        1.0,
                        1.0,
                    ],
                    s.range_m - (N_ATM * inv.distance_m + x.1 + x.2),
                    1.0 / (s.sigma_m * s.sigma_m),
                    s.id.clone(),
                ));
            }
        }
        if let Some((h, sh)) = height_aid {
            let here = ecef_to_geodetic(x.0);
            rows.push((
                [0.0, 0.0, 1.0, 0.0, 0.0],
                h - here.alt_m,
                1.0 / (sh * sh),
                "height-aid".into(),
            ));
        }
        // prior innovation: x̄ − x in the local frame
        let dp = sub(prior.pos, x.0);
        let delta = [
            dot(dp, e),
            dot(dp, n),
            dot(dp, u),
            prior.b_gnss_m - x.1,
            prior.offset_m - x.2,
        ];
        (rows, delta, (e, n, u))
    };
    for _ in 0..max_iter {
        let (rows, delta, (e, n, u)) = eval(&x);
        let mut a = zeros(NS);
        let mut b = [0.0; NS];
        for (h, r, w, _) in &rows {
            for i in 0..NS {
                b[i] += w * h[i] * r;
                for j in 0..NS {
                    a[i][j] += w * h[i] * h[j];
                }
            }
        }
        for i in 0..NS {
            for j in 0..NS {
                a[i][j] += pinv[i][j];
                b[i] += pinv[i][j] * delta[j];
            }
        }
        let ai = invert(&a)?;
        let mut dx = [0.0; NS];
        for i in 0..NS {
            for j in 0..NS {
                dx[i] += ai[i][j] * b[j];
            }
        }
        for k in 0..3 {
            x.0[k] += dx[0] * e[k] + dx[1] * n[k] + dx[2] * u[k];
        }
        x.1 += dx[3];
        x.2 += dx[4];
        if dx[..3].iter().map(|v| v * v).sum::<f64>().sqrt() < 1e-4
            && dx[3].abs() < 1e-4
            && dx[4].abs() < 1e-4
        {
            break;
        }
    }
    // final covariance and cost at the solution
    let (rows, delta, _) = eval(&x);
    let mut a = zeros(NS);
    let mut cost = 0.0;
    for (h, r, w, _) in &rows {
        cost += w * r * r;
        for i in 0..NS {
            for j in 0..NS {
                a[i][j] += w * h[i] * h[j];
            }
        }
    }
    for i in 0..NS {
        for j in 0..NS {
            a[i][j] += pinv[i][j];
            if prior.cov[i][i] < WEAK_VAR && prior.cov[j][j] < WEAK_VAR {
                cost += delta[i] * pinv[i][j] * delta[j];
            }
        }
    }
    let p = invert(&a)?;
    let mut cov = [[0.0; NS]; NS];
    for i in 0..NS {
        for j in 0..NS {
            cov[i][j] = p[i][j];
        }
    }
    Some(Solved {
        state: State {
            pos: x.0,
            b_gnss_m: x.1,
            offset_m: x.2,
            cov,
        },
        cost,
        m: rows.len(),
        n_prior,
        residuals: rows.into_iter().map(|(_, r, _, id)| (id, r)).collect(),
    })
}

// ------------------------------------------------------------------ sub-solutions and tests

/// GNSS-only snapshot fix through the open engine's SPP solver, and its RAIM.
fn gnss_raim(gnss: &[GnssRange], apriori: [f64; 3], cfg: &FusionConfig) -> Option<RaimResult> {
    if gnss.len() < 5 {
        return None;
    }
    let meas: Vec<SppMeasurement> = gnss
        .iter()
        .map(|g| SppMeasurement {
            sat_ecef: g.sat_ecef,
            pseudorange_m: g.range_m,
            sat_clock_m: 0.0,
            iono_m: 0.0,
            tropo_m: 0.0,
            weight: 1.0 / (g.sigma_m * g.sigma_m),
        })
        .collect();
    let fix = solve_spp(&meas, apriori)?;
    let sats: Vec<[f64; 3]> = gnss.iter().map(|g| g.sat_ecef).collect();
    let resid: Vec<f64> = gnss
        .iter()
        .map(|g| g.range_m - (norm(sub(g.sat_ecef, fix.ecef)) + fix.clock_bias_m))
        .collect();
    let sigma = (gnss.iter().map(|g| g.sigma_m * g.sigma_m).sum::<f64>() / gnss.len() as f64)
        .sqrt();
    snapshot_raim(fix.ecef, &sats, &resid, sigma, cfg.p_fa, cfg.p_md)
}

fn cross_check(
    gnss: &[GnssRange],
    eloran: &[EloranRange],
    prior: &State,
    offset_ref: Option<(f64, f64)>,
    cfg: &FusionConfig,
) -> Option<CrossCheck> {
    if gnss.len() < 4 || eloran.len() < 3 {
        return None;
    }
    // GNSS alone: δ pinned (unobservable), everything else weak.
    let mut pg = State::weak(prior.pos);
    pg.b_gnss_m = prior.b_gnss_m;
    pg.cov[4][4] = 1e-6;
    let g = solve(&pg, gnss, &[], None, cfg.max_iter)?;
    // eLoran alone at the GNSS height: δ pinned at 0, so b_G carries b_E.
    let mut pe = State::weak(g.state.pos);
    pe.b_gnss_m = g.state.b_gnss_m;
    pe.cov[2][2] = 1.0;
    pe.cov[4][4] = 1e-6;
    let e = solve(&pe, &[], eloran, None, cfg.max_iter)?;
    // horizontal separation in the GNSS fix's frame
    let (ee, nn, _) = enu(g.state.pos);
    let d = sub(g.state.pos, e.state.pos);
    let (de, dn) = (dot(d, ee), dot(d, nn));
    let stat = kshana::cross_raim::separation_statistic_axis(
        de,
        0.0,
        g.state.cov[0][0].sqrt(),
        e.state.cov[0][0].sqrt(),
    ) + kshana::cross_raim::separation_statistic_axis(
        dn,
        0.0,
        g.state.cov[1][1].sqrt(),
        e.state.cov[1][1].sqrt(),
    );
    let horizontal = Chi2Test::new(stat, 2.0, cfg.p_fa)?;
    let diff = e.state.b_gnss_m - g.state.b_gnss_m;
    let (time_innovation_ns, time) = match offset_ref {
        Some((d_ref, var_ref)) => {
            let var = var_ref + g.state.cov[3][3] + e.state.cov[3][3];
            let inn = diff - d_ref;
            (inn / C * 1e9, Chi2Test::new(inn * inn / var, 1.0, cfg.p_fa))
        }
        None => (f64::NAN, None),
    };
    let flagged = horizontal.flagged || time.is_some_and(|t| t.flagged);
    Some(CrossCheck {
        d_east_m: de,
        d_north_m: dn,
        horizontal,
        time_innovation_ns,
        time,
        flagged,
    })
}

// ------------------------------------------------------------------ the filter

/// The hybrid filter: feed it one epoch at a time.
#[derive(Clone, Debug)]
pub struct HybridFilter {
    pub cfg: FusionConfig,
    state: Option<State>,
    last_t: Option<f64>,
}

fn centroid(eloran: &[EloranRange]) -> Option<[f64; 3]> {
    if eloran.is_empty() {
        return None;
    }
    let mut s = [0.0; 3];
    for e in eloran {
        let r = geo_to_ecef(Geo {
            h_m: 0.0,
            ..e.station
        });
        for k in 0..3 {
            s[k] += r[k];
        }
    }
    let n = eloran.len() as f64;
    let c = [s[0] / n, s[1] / n, s[2] / n];
    let g = to_geo(c);
    Some(geo_to_ecef(Geo { h_m: 0.0, ..g }))
}

impl HybridFilter {
    pub fn new(cfg: FusionConfig) -> HybridFilter {
        HybridFilter {
            cfg,
            state: None,
            last_t: None,
        }
    }

    /// The current state, if any.
    pub fn state(&self) -> Option<&State> {
        self.state.as_ref()
    }

    /// The predicted state at `t_s` (prior of the next update).
    fn predict(&self, t_s: f64, gnss: &[GnssRange], eloran: &[EloranRange]) -> Option<State> {
        let mut s = match &self.state {
            Some(s) => s.clone(),
            None => {
                let pos = self
                    .cfg
                    .initial_ecef
                    .or_else(|| centroid(eloran))
                    .or_else(|| {
                        // under the satellites, on the ellipsoid
                        let mut m = [0.0; 3];
                        for g in gnss {
                            for k in 0..3 {
                                m[k] += g.sat_ecef[k];
                            }
                        }
                        let n = norm(m);
                        (n > 0.0).then(|| [m[0] / n * 6.37e6, m[1] / n * 6.37e6, m[2] / n * 6.37e6])
                    })?;
                let mut s = State::weak(pos);
                if let Some((d, sd)) = self.cfg.offset_prior_ns {
                    s.offset_m = d * 1e-9 * C;
                    s.cov[4][4] = (sd * 1e-9 * C).powi(2);
                }
                return Some(s);
            }
        };
        let dt = self.last_t.map(|t0| (t_s - t0).max(0.0)).unwrap_or(0.0);
        let q = [
            self.cfg.q_horizontal,
            self.cfg.q_horizontal,
            self.cfg.q_vertical,
            self.cfg.q_clock,
            self.cfg.q_offset,
        ];
        for (i, qi) in q.iter().enumerate() {
            s.cov[i][i] += qi * dt;
        }
        // keep the stated offset prior as a floor of knowledge
        if let Some((d, sd)) = self.cfg.offset_prior_ns {
            let v = (sd * 1e-9 * C).powi(2);
            if s.cov[4][4] > v {
                // fuse the stated prior into the propagated offset
                let w1 = 1.0 / s.cov[4][4];
                let w2 = 1.0 / v;
                s.offset_m = (s.offset_m * w1 + d * 1e-9 * C * w2) / (w1 + w2);
                s.cov[4][4] = 1.0 / (w1 + w2);
            }
        }
        Some(s)
    }

    /// Process one epoch at `t_s`: GNSS ranges (empty when jammed) and eLoran ranges.
    pub fn step(
        &mut self,
        t_s: f64,
        gnss: &[GnssRange],
        eloran: &[EloranRange],
    ) -> Result<FusionFix, String> {
        let prior = self
            .predict(t_s, gnss, eloran)
            .ok_or("no initial position: give FusionConfig::initial_ecef")?;
        let cfg = self.cfg.clone();
        let use_gnss = gnss.len() >= 4 || (self.state.is_some() && !gnss.is_empty());
        let g_in: &[GnssRange] = if use_gnss { gnss } else { &[] };
        // height aid when the GNSS cannot carry height and the prior does not
        let height_aid = if g_in.len() < 4 && prior.cov[2][2] > 1e4 {
            cfg.height_aid.or(Some((to_geo(prior.pos).h_m.max(0.0), 50.0)))
        } else {
            None
        };
        let mut solved = solve(&prior, g_in, eloran, height_aid, cfg.max_iter)
            .ok_or("hybrid solve: singular geometry")?;
        let mut reasons = Vec::new();
        let global = Chi2Test::new(
            solved.cost,
            (solved.m + solved.n_prior) as f64 - NS as f64,
            cfg.p_fa,
        );
        if global.is_some_and(|t| t.flagged) {
            reasons.push("global residual test".to_string());
        }
        let raim = gnss_raim(g_in, prior.pos, &cfg);
        if raim.is_some_and(|r| r.fault_detected) {
            reasons.push("GNSS RAIM fault".to_string());
        }
        let offset_ref = (prior.cov[4][4] < WEAK_VAR).then_some((prior.offset_m, prior.cov[4][4]));
        let cross = cross_check(g_in, eloran, &prior, offset_ref, &cfg);
        if let Some(c) = &cross {
            if c.horizontal.flagged {
                reasons.push("GNSS/eLoran horizontal inconsistency".to_string());
            }
            if c.time.is_some_and(|t| t.flagged) {
                reasons.push("GNSS/eLoran time-scale inconsistency".to_string());
            }
        }
        let flagged = !reasons.is_empty();
        let mut excluded = false;
        if flagged && cfg.fallback_to_eloran && !g_in.is_empty() && eloran.len() >= 3 {
            let aid = if prior.cov[2][2] > 1e4 {
                cfg.height_aid.or(Some((to_geo(prior.pos).h_m, 50.0)))
            } else {
                None
            };
            if let Some(s) = solve(&prior, &[], eloran, aid, cfg.max_iter) {
                solved = s;
                excluded = true;
            }
        }
        let n_gnss = if excluded { 0 } else { g_in.len() };
        let mode = match (n_gnss > 0, !eloran.is_empty()) {
            (true, true) => Mode::Hybrid,
            (true, false) => Mode::GnssOnly,
            (false, true) => Mode::EloranOnly,
            (false, false) => Mode::Coast,
        };
        let s = &solved.state;
        let g = to_geo(s.pos);
        let fix = FusionFix {
            t_s,
            mode,
            ecef: s.pos,
            lat_deg: g.lat_deg,
            lon_deg: g.lon_deg,
            h_m: g.h_m,
            gnss_clock_ns: s.b_gnss_m / C * 1e9,
            eloran_clock_ns: (s.b_gnss_m + s.offset_m) / C * 1e9,
            offset_ns: s.offset_m / C * 1e9,
            sigma: [
                s.cov[0][0].sqrt(),
                s.cov[1][1].sqrt(),
                s.cov[2][2].sqrt(),
                s.cov[3][3].sqrt() / C * 1e9,
                s.cov[4][4].sqrt() / C * 1e9,
            ],
            n_gnss,
            n_eloran: eloran.len(),
            residuals_m: solved.residuals.clone(),
            integrity: Integrity {
                global,
                gnss_raim: raim,
                cross,
                flagged,
                gnss_excluded: excluded,
                reasons,
            },
        };
        self.state = Some(solved.state);
        self.last_t = Some(t_s);
        Ok(fix)
    }
}

// ------------------------------------------------------------------ scenario helpers

/// Ideal corrected GNSS ranges from broadcast ephemerides at GPS time of week `tow_s` for a
/// receiver at `rx` (ECEF) with clock bias `b_gnss_m`: satellite at transmit time (Sagnac
/// rotated), above `mask_deg`. Noise is the caller's.
pub fn broadcast_ranges(
    ephs: &[RinexEphemeris],
    tow_s: f64,
    rx: [f64; 3],
    b_gnss_m: f64,
    mask_deg: f64,
    sigma_m: f64,
) -> Vec<GnssRange> {
    let rxg = ecef_to_geodetic(rx);
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for e in ephs {
        if e.system != 'G' || !seen.insert(e.prn) {
            continue;
        }
        let Some(eph) = kshana::pvt::select_ephemeris(ephs, 'G', e.prn, tow_s) else {
            continue;
        };
        let mut tau = 0.07;
        let mut sat = eph.sv_position_ecef(tow_s - tau);
        for _ in 0..4 {
            sat = kshana::pvt::sagnac_rotate(eph.sv_position_ecef(tow_s - tau), tau);
            tau = norm(sub(sat, rx)) / C;
        }
        let el = kshana::frames::look_angles(rxg, sat).el_rad.to_degrees();
        if el < mask_deg {
            continue;
        }
        out.push(GnssRange {
            id: format!("G{:02}", e.prn),
            sat_ecef: sat,
            range_m: norm(sub(sat, rx)) + b_gnss_m,
            sigma_m,
        });
    }
    out
}

/// Ideal eLoran ranges for a receiver at `rx` with eLoran clock bias `b_eloran_m`.
pub fn model_eloran_ranges(
    stations: &[(String, Geo)],
    rx: Geo,
    b_eloran_m: f64,
    sigma_m: f64,
) -> Vec<EloranRange> {
    stations
        .iter()
        .map(|(id, st)| EloranRange {
            id: id.clone(),
            station: *st,
            range_m: N_ATM * inverse(rx, *st).distance_m + b_eloran_m,
            sigma_m,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_of_a_small_matrix() {
        let a = vec![vec![4.0, 1.0], vec![2.0, 3.0]];
        let i = invert(&a).unwrap();
        assert!((i[0][0] - 0.3).abs() < 1e-12 && (i[1][0] + 0.2).abs() < 1e-12);
    }
}
