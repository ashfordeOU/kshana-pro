// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # Cross-system spoofing attribution from the eLoran/GNSS/clock disagreement geometry
//!
//! Patent candidate **E1** (research 04 §2; gap 01-3). CONFIDENTIAL: Kshana Pro only; do not
//! publish or move into the open engine before counsel decides on filing.
//!
//! Detection says *something* is wrong; attribution says *what*. With GNSS, eLoran and a
//! local clock observed together, each failure cause leaves a different pattern in the
//! disagreement between three independent solutions:
//!
//! * the **GNSS-only** fix (position `p_G`, clock `b_G`, internal χ² `q_G`) through the
//!   satellite line-of-sight geometry;
//! * the **eLoran-only** fix (horizontal position `p_E` on a height aid, clock `b_E`,
//!   internal χ² `q_E`) through the station bearing geometry, and its leave-one-station-out
//!   variants;
//! * the **local clock**'s holdover prediction `b_clk` of the receiver clock against GPS
//!   time, with `b_E = b_G + δ`, `δ` the GPS − eLoran time-scale offset (prior `δ_ref`).
//!
//! | cause | pattern |
//! |---|---|
//! | GNSS spoofed | `b_G` off the clock (time push) and/or `p_G` off `p_E` (position push); `b_E` agrees with the clock; GNSS internally consistent |
//! | GNSS jammed | GNSS lost (< 4 ranges) or its residuals inflated (`q_G` ≫ dof) with no coherent displacement |
//! | eLoran station fault | one station's residual along its bearing vector, either sign; removing it restores agreement; its propagation signature consistent (E4) |
//! | eLoran meacon / relay | one station late (never early); its propagation signature inconsistent with the claimed path (E4, [`kshana_eloran::guard::meacon`]) |
//! | local clock fault | `b_G` and `b_E` off the clock by the same amount; `δ` and positions consistent |
//!
//! Each hypothesis is scored by a generalised log-likelihood: −½ × the normalised squared
//! disagreement it cannot explain, minus a penalty `λ` per free parameter it needs (Occam
//! term), plus the log prior. Station hypotheses are evaluated per station (leave-one-out)
//! and summed per class. The softmax of the class scores is reported as posterior-like
//! scores, with the winning cause, the station (when one is implicated), the recommended
//! exclusion and a plain-language explanation of the features that drove the decision.
//!
//! The time disagreement uses the joint covariance of `[b_G − b_clk, b_E − δ_ref − b_clk]`
//! (the clock error is common to both); the position disagreement is the two-axis
//! normalised separation of `p_G` and `p_E`. A degraded-GNSS (jamming) hypothesis fits a
//! free variance inflation `s ≥ 1` of the GNSS noise.
//!
//! # Evidence and limits (honest)
//!
//! MODELLED: checked by the Monte Carlo confusion-matrix test (`tests/eloran_attrib.rs`) on
//! broadcast-ephemeris GNSS geometry and synthetic eLoran stations with stated noise, ASF
//! error and attack magnitudes; not validated against recorded attacks.
//!
//! * Snapshot attribution (one epoch); no temporal model (slow drags are caught once the
//!   accumulated disagreement exceeds the test; the δ steering envelope of D4 is not used).
//! * One fault cause at a time; a whole-band eLoran relay (all stations) and combined
//!   attacks are not modelled as hypotheses.
//! * Loss of GNSS with everything else consistent is attributed to jamming; obstruction
//!   cannot be told apart from it here.
//! * Meacon versus transmitter fault needs the E4 propagation-signature verdicts; without
//!   them a late single-station anomaly is split between the two by prior.
//! * `λ`, the priors and the σ values are tuning (keep confidential; research 04 §5).

use crate::eloran_fusion::{
    dot, enu, solve, sub, to_geo, EloranRange, GnssRange, Solved, State, C,
};
use kshana_eloran::geo::inverse;
use kshana_eloran::guard::meacon::StationVerdict;
use kshana_eloran::prop::ground::N_ATM;
use serde::{Deserialize, Serialize};

/// The attributed cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    Clean,
    GnssSpoof,
    GnssJam,
    EloranStationFault,
    EloranMeacon,
    ClockFault,
}

impl Cause {
    pub const ALL: [Cause; 6] = [
        Cause::Clean,
        Cause::GnssSpoof,
        Cause::GnssJam,
        Cause::EloranStationFault,
        Cause::EloranMeacon,
        Cause::ClockFault,
    ];
    fn index(self) -> usize {
        Cause::ALL.iter().position(|c| *c == self).unwrap()
    }
    pub fn label(self) -> &'static str {
        match self {
            Cause::Clean => "clean",
            Cause::GnssSpoof => "GNSS spoofed",
            Cause::GnssJam => "GNSS jammed",
            Cause::EloranStationFault => "eLoran station fault",
            Cause::EloranMeacon => "eLoran meaconing/relay",
            Cause::ClockFault => "local clock fault",
        }
    }
}

/// Settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttribConfig {
    /// Prior on δ = GPS time − eLoran time scale (ns, σ ns).
    pub offset_ref_ns: (f64, f64),
    /// Height aid for the eLoran-only fix (ellipsoidal m, σ m); default: the GNSS fix's
    /// height (σ 50 m), else 0 m (σ 100 m).
    pub height_aid: Option<(f64, f64)>,
    /// Penalty per free parameter (log-likelihood units).
    pub lambda: f64,
    /// Penalty for a free propagation signature (meacon hypothesis, when E4 data exist).
    pub lambda_signature: f64,
    /// Class priors in [`Cause::ALL`] order (normalised internally).
    pub prior: [f64; 6],
    /// Posterior-like score of `Clean` below which the alarm is raised.
    pub alarm_below: f64,
    pub max_iter: usize,
}

impl Default for AttribConfig {
    fn default() -> Self {
        AttribConfig {
            offset_ref_ns: (0.0, 10.0),
            height_aid: None,
            lambda: 6.0,
            lambda_signature: 4.0,
            prior: [1.0; 6],
            alarm_below: 0.5,
            max_iter: 20,
        }
    }
}

/// One epoch's inputs.
#[derive(Clone, Debug, Default)]
pub struct AttribEpoch<'a> {
    pub gnss: &'a [GnssRange],
    pub eloran: &'a [EloranRange],
    /// Local-clock prediction of the receiver clock − GPS time (ns) and its σ (ns).
    pub clock_ns: (f64, f64),
    /// E4 verdicts per station (`station` = the [`EloranRange::id`]); optional.
    pub e4: &'a [StationVerdict],
    /// Starting point (ECEF) for the solves; default: the eLoran stations' centroid.
    pub approx_ecef: Option<[f64; 3]>,
}

/// The disagreement features.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Features {
    pub n_gnss: usize,
    pub n_eloran: usize,
    /// GNSS-only internal χ² and dof.
    pub q_gnss: Option<(f64, usize)>,
    /// eLoran-only internal χ² and dof.
    pub q_eloran: Option<(f64, usize)>,
    /// `b_G − b_clk` (ns) and its normalised value.
    pub gnss_vs_clock_ns: Option<(f64, f64)>,
    /// `b_E − δ_ref − b_clk` (ns) and its normalised value.
    pub eloran_vs_clock_ns: Option<(f64, f64)>,
    /// `p_G − p_E` east, north (m) and the two-axis normalised statistic.
    pub position_split_m: Option<(f64, f64, f64)>,
    /// Largest leave-one-out eLoran residual: station, residual (m), normalised.
    pub worst_station: Option<(String, f64, f64)>,
}

/// What to exclude or repair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Exclusion {
    Nothing,
    Gnss,
    Station(String),
    Clock,
}

/// The attribution of one epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attribution {
    /// Posterior-like scores in [`Cause::ALL`] order (sum 1).
    pub scores: Vec<(Cause, f64)>,
    pub cause: Cause,
    /// The implicated eLoran station, for station causes.
    pub station: Option<String>,
    /// Raised when the `Clean` score is below [`AttribConfig::alarm_below`].
    pub alarm: bool,
    pub exclusion: Exclusion,
    pub features: Features,
    pub explanation: Vec<String>,
}

impl Attribution {
    pub fn score(&self, c: Cause) -> f64 {
        self.scores[c.index()].1
    }
}

// ------------------------------------------------------------------ helpers

struct Sub {
    pos: [f64; 3],
    b_m: f64,
    var_b: f64,
    cov_h: [[f64; 2]; 2],
    q: f64,
    dof: usize,
}

fn sub_of(s: &Solved, dof: usize) -> Sub {
    let c = &s.state.cov;
    Sub {
        pos: s.state.pos,
        b_m: s.state.b_gnss_m,
        var_b: c[3][3],
        cov_h: [[c[0][0], c[0][1]], [c[1][0], c[1][1]]],
        q: s.cost,
        dof,
    }
}

fn gnss_only(g: &[GnssRange], approx: [f64; 3], it: usize) -> Option<Sub> {
    if g.len() < 4 {
        return None;
    }
    let mut p = State::weak(approx);
    p.cov[4][4] = 1e-6;
    let s = solve(&p, g, &[], None, it)?;
    Some(sub_of(&s, g.len() - 4))
}

fn eloran_only(e: &[EloranRange], start: [f64; 3], aid: (f64, f64), it: usize) -> Option<Sub> {
    if e.len() < 3 {
        return None;
    }
    let mut p = State::weak(start);
    p.cov[4][4] = 1e-6;
    let s = solve(&p, &[], e, Some(aid), it)?;
    Some(sub_of(&s, e.len() - 3))
}

fn inv2(m: [[f64; 2]; 2]) -> Option<[[f64; 2]; 2]> {
    let d = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    (d.abs() > 1e-300).then(|| [[m[1][1] / d, -m[0][1] / d], [-m[1][0] / d, m[0][0] / d]])
}

fn quad2(m: [[f64; 2]; 2], u: [f64; 2]) -> f64 {
    u[0] * (m[0][0] * u[0] + m[0][1] * u[1]) + u[1] * (m[1][0] * u[0] + m[1][1] * u[1])
}

/// Time and position terms between a GNSS sub-solution (noise scaled by `s`) and an
/// eLoran sub-solution.
struct Terms {
    /// Full time χ² (2 dof), the clock-fault residual (1 dof), the eLoran-vs-clock marginal
    /// (spoof: GNSS time free), and the position χ² (2 dof).
    q_time: f64,
    q_clock: f64,
    q_e_only: f64,
    q_pos: f64,
    u_ns: [f64; 2],
    z: [f64; 2],
    split: (f64, f64),
}

fn terms(
    g: &Sub,
    s: f64,
    e: &Sub,
    clk_m: f64,
    var_clk: f64,
    d_m: f64,
    var_d: f64,
) -> Option<Terms> {
    let u = [g.b_m - clk_m, e.b_m - d_m - clk_m];
    let c = [
        [s * g.var_b + var_clk, var_clk],
        [var_clk, e.var_b + var_d + var_clk],
    ];
    let ci = inv2(c)?;
    let q_time = quad2(ci, u);
    let one_ci_u = ci[0][0] * u[0] + ci[0][1] * u[1] + ci[1][0] * u[0] + ci[1][1] * u[1];
    let one_ci_one = ci[0][0] + ci[0][1] + ci[1][0] + ci[1][1];
    let q_clock = q_time - one_ci_u * one_ci_u / one_ci_one;
    let q_e_only = u[1] * u[1] / c[1][1];
    let (ee, nn, _) = enu(g.pos);
    let d = sub(g.pos, e.pos);
    let dv = [dot(d, ee), dot(d, nn)];
    let cp = [
        [
            s * g.cov_h[0][0] + e.cov_h[0][0],
            s * g.cov_h[0][1] + e.cov_h[0][1],
        ],
        [
            s * g.cov_h[1][0] + e.cov_h[1][0],
            s * g.cov_h[1][1] + e.cov_h[1][1],
        ],
    ];
    let q_pos = quad2(inv2(cp)?, dv);
    Some(Terms {
        q_time,
        q_clock,
        q_e_only,
        q_pos,
        u_ns: [u[0] / C * 1e9, u[1] / C * 1e9],
        z: [u[0] / c[0][0].sqrt(), u[1] / c[1][1].sqrt()],
        split: (dv[0], dv[1]),
    })
}

fn logsumexp(v: &[f64]) -> f64 {
    let m = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !m.is_finite() {
        return m;
    }
    m + v.iter().map(|x| (x - m).exp()).sum::<f64>().ln()
}

fn centroid(e: &[EloranRange]) -> Option<[f64; 3]> {
    if e.is_empty() {
        return None;
    }
    let (la, lo) = e.iter().fold((0.0, 0.0), |a, s| {
        (a.0 + s.station.lat_deg, a.1 + s.station.lon_deg)
    });
    let n = e.len() as f64;
    Some(crate::eloran_fusion::geo_to_ecef(
        kshana_eloran::geo::Geo::new(la / n, lo / n),
    ))
}

/// Leave-one-out residual of station `s` against a sub-solution, and its variance (m²).
fn loo_residual(s: &EloranRange, sol: &Solved) -> (f64, f64) {
    let here = to_geo(sol.state.pos);
    let inv = inverse(here, s.station);
    let r = s.range_m - (N_ATM * inv.distance_m + sol.state.b_gnss_m);
    let h = [
        -N_ATM * inv.az1_rad.sin(),
        -N_ATM * inv.az1_rad.cos(),
        0.0,
        1.0,
    ];
    let c = &sol.state.cov;
    let mut v = s.sigma_m * s.sigma_m;
    for i in 0..4 {
        for j in 0..4 {
            v += h[i] * c[i][j] * h[j];
        }
    }
    (r, v)
}

// ------------------------------------------------------------------ attribution

/// Attribute one epoch.
pub fn attribute(ep: &AttribEpoch, cfg: &AttribConfig) -> Result<Attribution, String> {
    let it = cfg.max_iter;
    let approx = ep
        .approx_ecef
        .or_else(|| centroid(ep.eloran))
        .ok_or("attribution: no eLoran stations and no approximate position")?;
    if ep.eloran.len() < 3 {
        return Err("attribution: at least 3 eLoran stations are needed".into());
    }
    let clk_m = ep.clock_ns.0 * 1e-9 * C;
    let var_clk = (ep.clock_ns.1 * 1e-9 * C).powi(2);
    let d_m = cfg.offset_ref_ns.0 * 1e-9 * C;
    let var_d = (cfg.offset_ref_ns.1 * 1e-9 * C).powi(2);
    let lam = cfg.lambda;
    let lp: Vec<f64> = {
        let t: f64 = cfg.prior.iter().sum();
        cfg.prior.iter().map(|p| (p / t).max(1e-300).ln()).collect()
    };

    let g = gnss_only(ep.gnss, approx, it);
    let aid = cfg.height_aid.unwrap_or_else(|| match &g {
        Some(g) => (to_geo(g.pos).h_m, 50.0),
        None => (0.0, 100.0),
    });
    let start = g.as_ref().map(|g| g.pos).unwrap_or(approx);
    let e = eloran_only(ep.eloran, start, aid, it).ok_or("attribution: eLoran solve failed")?;

    // E4 signature terms per station
    let sig = |id: &str| -> Option<(f64, usize)> {
        ep.e4
            .iter()
            .find(|v| v.station == id)
            .filter(|v| v.signature_dof > 0)
            .map(|v| (v.signature_stat, v.signature_dof))
    };
    let s_all: f64 = ep
        .eloran
        .iter()
        .filter_map(|s| sig(&s.id))
        .map(|x| x.0)
        .sum();

    // leave-one-out eLoran sub-solutions
    struct Loo {
        id: String,
        sub: Sub,
        r: f64,
        z: f64,
    }
    let mut loos: Vec<Loo> = Vec::new();
    if ep.eloran.len() >= 4 {
        for (i, s) in ep.eloran.iter().enumerate() {
            let rest: Vec<EloranRange> = ep
                .eloran
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, x)| x.clone())
                .collect();
            let mut p = State::weak(start);
            p.cov[4][4] = 1e-6;
            if let Some(sol) = solve(&p, &[], &rest, Some(aid), it) {
                let (r, v) = loo_residual(s, &sol);
                loos.push(Loo {
                    id: s.id.clone(),
                    sub: sub_of(&sol, rest.len() - 3),
                    r,
                    z: r / v.sqrt(),
                });
            }
        }
    }

    let mut feats = Features {
        n_gnss: ep.gnss.len(),
        n_eloran: ep.eloran.len(),
        q_eloran: Some((e.q, e.dof)),
        ..Default::default()
    };
    if let Some(w) = loos.iter().max_by(|a, b| a.z.abs().total_cmp(&b.z.abs())) {
        feats.worst_station = Some((w.id.clone(), w.r, w.z));
    }
    // class log-likelihood variants: (class, station, value)
    let mut v: Vec<(Cause, Option<String>, f64)> = Vec::new();
    let e_vs_clk = |sub: &Sub| -> (f64, f64) {
        let u = sub.b_m - d_m - clk_m;
        (u, u * u / (sub.var_b + var_d + var_clk))
    };

    match &g {
        None => {
            // GNSS lost: jamming (eLoran consistent with the clock) or a clock fault
            let (u, q) = e_vs_clk(&e);
            feats.eloran_vs_clock_ns = Some((u / C * 1e9, q.sqrt() * u.signum()));
            let base = -0.5 * (e.q + s_all);
            v.push((Cause::GnssJam, None, base - 0.5 * q + lp[2]));
            v.push((Cause::ClockFault, None, base - lam + lp[5]));
        }
        Some(gs) => {
            feats.q_gnss = Some((gs.q, gs.dof));
            let t = terms(gs, 1.0, &e, clk_m, var_clk, d_m, var_d)
                .ok_or("attribution: singular time/position covariance")?;
            feats.gnss_vs_clock_ns = Some((t.u_ns[0], t.z[0]));
            feats.eloran_vs_clock_ns = Some((t.u_ns[1], t.z[1]));
            feats.position_split_m = Some((t.split.0, t.split.1, t.q_pos));
            let common = gs.q + e.q + s_all;
            // clean
            v.push((
                Cause::Clean,
                None,
                -0.5 * (common + t.q_time + t.q_pos) + lp[0],
            ));
            // GNSS spoofed: time push, position push, or both
            for (q, k) in [
                (t.q_e_only + t.q_pos, 1.0),
                (t.q_time, 2.0),
                (t.q_e_only, 3.0),
            ] {
                v.push((
                    Cause::GnssSpoof,
                    None,
                    -0.5 * (common + q) - k * lam + lp[1] - 3f64.ln(),
                ));
            }
            // local clock fault: one common offset
            v.push((
                Cause::ClockFault,
                None,
                -0.5 * (common + t.q_clock + t.q_pos) - lam + lp[5],
            ));
            // GNSS jammed (degraded): free noise inflation s ≥ 1
            if gs.dof >= 1 {
                let s = (gs.q / gs.dof as f64).max(1.0);
                if let Some(tj) = terms(gs, s, &e, clk_m, var_clk, d_m, var_d) {
                    let ll = -0.5
                        * (gs.q / s + gs.dof as f64 * s.ln() + e.q + s_all + tj.q_time + tj.q_pos);
                    v.push((Cause::GnssJam, None, ll - lam + lp[2]));
                }
            }
            // eLoran station fault / meacon (leave one out)
            let n = loos.len().max(1) as f64;
            for l in &loos {
                let Some(tl) = terms(gs, 1.0, &l.sub, clk_m, var_clk, d_m, var_d) else {
                    continue;
                };
                let ll_fault = -0.5 * (gs.q + l.sub.q + s_all + tl.q_time + tl.q_pos);
                v.push((
                    Cause::EloranStationFault,
                    Some(l.id.clone()),
                    ll_fault - lam + lp[3] - n.ln(),
                ));
                // meacon: the bias must be late; the station's signature is free
                let ll_late = if l.r >= 0.0 {
                    ll_fault
                } else {
                    -0.5 * (common + t.q_time + t.q_pos)
                };
                let (s_i, pen) = match sig(&l.id) {
                    Some((s_i, _)) => (s_i, cfg.lambda_signature),
                    None => (0.0, 0.0),
                };
                v.push((
                    Cause::EloranMeacon,
                    Some(l.id.clone()),
                    ll_late + 0.5 * s_i - lam - pen + lp[4] - n.ln(),
                ));
            }
        }
    }

    // class scores
    let mut class_ll = [f64::NEG_INFINITY; 6];
    for c in Cause::ALL {
        let xs: Vec<f64> = v.iter().filter(|x| x.0 == c).map(|x| x.2).collect();
        if !xs.is_empty() {
            class_ll[c.index()] = logsumexp(&xs);
        }
    }
    let z = logsumexp(&class_ll);
    let scores: Vec<(Cause, f64)> = Cause::ALL
        .iter()
        .map(|c| (*c, (class_ll[c.index()] - z).exp()))
        .collect();
    let cause = scores
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
        .unwrap();
    let station = matches!(cause, Cause::EloranStationFault | Cause::EloranMeacon)
        .then(|| {
            v.iter()
                .filter(|x| x.0 == cause)
                .max_by(|a, b| a.2.total_cmp(&b.2))
                .and_then(|x| x.1.clone())
        })
        .flatten();
    let exclusion = match cause {
        Cause::Clean => Exclusion::Nothing,
        Cause::GnssSpoof | Cause::GnssJam => Exclusion::Gnss,
        Cause::EloranStationFault | Cause::EloranMeacon => {
            Exclusion::Station(station.clone().unwrap_or_default())
        }
        Cause::ClockFault => Exclusion::Clock,
    };
    let alarm = scores[0].1 < cfg.alarm_below;
    let explanation = explain(cause, station.as_deref(), &feats, ep, &scores);
    Ok(Attribution {
        scores,
        cause,
        station,
        alarm,
        exclusion,
        features: feats,
        explanation,
    })
}

fn explain(
    cause: Cause,
    station: Option<&str>,
    f: &Features,
    ep: &AttribEpoch,
    scores: &[(Cause, f64)],
) -> Vec<String> {
    let mut out = Vec::new();
    let best = scores.iter().map(|s| s.1).fold(0.0, f64::max);
    out.push(format!("attributed: {} (score {:.3})", cause.label(), best));
    if let Some((u, z)) = f.gnss_vs_clock_ns {
        out.push(format!("GNSS clock − local clock: {u:+.1} ns ({z:+.1}σ)"));
    }
    if let Some((u, z)) = f.eloran_vs_clock_ns {
        out.push(format!(
            "eLoran clock − δ − local clock: {u:+.1} ns ({z:+.1}σ)"
        ));
    }
    if let Some((de, dn, q)) = f.position_split_m {
        out.push(format!(
            "GNSS − eLoran position: E {de:+.0} m, N {dn:+.0} m (χ²₂ = {q:.1})"
        ));
    }
    if let Some((q, d)) = f.q_gnss {
        out.push(format!("GNSS internal χ² = {q:.1} on {d} dof"));
    }
    if f.n_gnss < 4 {
        out.push(format!("GNSS ranges available: {} (< 4)", f.n_gnss));
    }
    if let Some((id, r, z)) = &f.worst_station {
        out.push(format!(
            "largest leave-one-out eLoran residual: {id} {:+.0} ns ({z:+.1}σ)",
            r / C * 1e9
        ));
    }
    if let Some(id) = station {
        if let Some(v) = ep.e4.iter().find(|v| v.station == id) {
            out.push(format!(
                "{id} propagation signature (E4): S = {:.1} on {} features, class {:?}",
                v.signature_stat, v.signature_dof, v.class
            ));
        }
    }
    out.push(
        match cause {
            Cause::Clean => "all three systems agree within their stated uncertainty",
            Cause::GnssSpoof => {
                "GNSS disagrees with both eLoran and the local clock, which agree with each other"
            }
            Cause::GnssJam => "GNSS lost or its noise inflated, with no coherent displacement",
            Cause::EloranStationFault => {
                "one eLoran station disagrees along its bearing; its propagation signature is consistent"
            }
            Cause::EloranMeacon => {
                "one eLoran station is late and its propagation signature does not fit the claimed path"
            }
            Cause::ClockFault => {
                "GNSS and eLoran agree with each other but both disagree with the local clock by the same amount"
            }
        }
        .to_string(),
    );
    out
}
