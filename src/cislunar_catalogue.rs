// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Cislunar catalogue build-up: tracklet association and initial orbit determination
//!
//! A cislunar catalogue starts from short angles-only tracklets that carry no object identity.
//! This study takes such tracklets (right ascension and declination seen from the Earth's
//! centre), associates them to objects, and gives each object an initial orbit with its
//! covariance:
//!
//! 1. **Attributable** per tracklet: mean epoch, angles and angle rates from a straight-line fit.
//! 2. **Association by range hypotheses** (the admissible-region approach of Milani et al.,
//!    2004): for a pair of tracklets, the earlier one's attributable plus a hypothesised range
//!    and range rate is a full state; it is propagated in the Earth–Moon CR3BP (the open
//!    engine's [`kshana::cr3bp::propagate_cr3bp`]) to the later tracklet's epochs. The best
//!    hypothesis on a grid is refined by Gauss–Newton ([`kshana::batch_ls::gauss_newton`]); the
//!    pair is linked when the normalised RMS angle residual is within the stated gate. Linked
//!    tracklets form objects (connected components).
//! 3. **Initial orbit determination** per object: a six-state batch least-squares fit to every
//!    observation of its tracklets ([`kshana::batch_ls::gauss_newton`]), with the covariance
//!    `(HᵀWH)⁻¹` from the finite-difference Jacobian ([`kshana::batch_ls::fd_jacobian`]).
//!
//! The synthetic tracks ([`synth`]) are generated with the same CR3BP propagator and noise of the
//! stated angular sigma, so the study reports association purity and completeness and the IOD
//! error against truth. **MODELLED**: circular restricted three-body dynamics in the synodic
//! frame, observer at the Earth's centre, no light time or aberration. Planning and evidence only.

use std::collections::BTreeMap;

use kshana::batch_ls::{fd_jacobian, gauss_newton};
use kshana::cr3bp::{propagate_cr3bp, Cr3bpState, EARTH_MOON_MU};
use serde::{Deserialize, Serialize};

use crate::forensics::inputs::round;

/// Pro feature.
pub const FEATURE: &str = "pro-cislunar-catalogue";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-cislunar-catalogue/1";
/// Label.
pub const EVIDENCE: &str = "MODELLED: Earth-Moon CR3BP dynamics (open engine), geocentric \
angles-only observations without light time or aberration, range-hypothesis association and \
batch least-squares IOD. A catalogue build-up study on synthetic tracks, not an operational \
catalogue.";
/// Earth–Moon distance (km) and time unit (s) of the normalised CR3BP.
pub const LU_KM: f64 = 384_400.0;
pub const TU_S: f64 = 375_190.0;

const MU: f64 = EARTH_MOON_MU;
const STEP_TU: f64 = 0.002;

/// One observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Obs {
    /// Time (normalised CR3BP units from the study epoch; 1 TU = 375 190 s).
    pub t: f64,
    pub ra_rad: f64,
    pub dec_rad: f64,
}

/// A tracklet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tracklet {
    pub id: String,
    pub obs: Vec<Obs>,
    /// Truth label of a synthetic tracklet (used only to score the association).
    #[serde(default)]
    pub truth_object: Option<String>,
}

/// Truth of a synthetic object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TruthObject {
    pub name: String,
    /// Rotating-frame state at t = 0 (normalised).
    pub r: [f64; 3],
    pub v: [f64; 3],
}

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub data_source: String,
    pub sigma_arcsec: f64,
    /// Range hypotheses (Earth–Moon units) and range-rate hypotheses (LU/TU).
    pub range_min: f64,
    pub range_max: f64,
    pub range_rate_max: f64,
    #[serde(default = "d_nr")]
    pub n_range: usize,
    #[serde(default = "d_nrr")]
    pub n_range_rate: usize,
    /// Link gate: normalised RMS angle residual.
    #[serde(default = "d_gate")]
    pub gate: f64,
    #[serde(rename = "tracklet")]
    pub tracklets: Vec<Tracklet>,
    #[serde(default, rename = "truth")]
    pub truth: Vec<TruthObject>,
}

fn d_nr() -> usize {
    24
}
fn d_nrr() -> usize {
    16
}
fn d_gate() -> f64 {
    3.0
}

/// An attributable.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attributable {
    pub tracklet: String,
    pub t: f64,
    pub ra: f64,
    pub dec: f64,
    pub ra_rate: f64,
    pub dec_rate: f64,
}

/// A link between tracklets.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub residual: f64,
    pub range: f64,
    pub range_rate: f64,
}

/// An object with its initial orbit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Object {
    pub id: String,
    pub tracklets: Vec<String>,
    pub epoch_tu: f64,
    /// Rotating-frame state (normalised).
    pub state: Option<[f64; 6]>,
    pub rms_arcsec: Option<f64>,
    pub pos_sigma_km: Option<f64>,
    pub vel_sigma_m_s: Option<f64>,
    /// Truth comparison of a synthetic study.
    pub truth_object: Option<String>,
    pub pos_error_km: Option<f64>,
    pub within_3_sigma: Option<bool>,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub data_source: String,
    pub attributables: Vec<Attributable>,
    pub links: Vec<Link>,
    pub objects: Vec<Object>,
    pub unassociated: Vec<String>,
    /// Synthetic scoring: share of objects whose tracklets share one truth label, and share of
    /// truth objects recovered whole.
    pub purity: Option<f64>,
    pub completeness: Option<f64>,
    pub limits: Vec<String>,
    pub verdict: String,
}

fn rot(t: f64, v: [f64; 3]) -> [f64; 3] {
    let (s, c) = t.sin_cos();
    [c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]]
}

fn cross_z(v: [f64; 3]) -> [f64; 3] {
    [-v[1], v[0], 0.0]
}

const EARTH: [f64; 3] = [-MU, 0.0, 0.0];

/// Geocentric angles (ra, dec) in the inertial frame of a rotating-frame position at time t.
pub fn angles(r: [f64; 3], t: f64) -> (f64, f64) {
    let d = rot(t, [r[0] - EARTH[0], r[1] - EARTH[1], r[2]]);
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    (
        d[1].atan2(d[0]).rem_euclid(std::f64::consts::TAU),
        (d[2] / n).asin(),
    )
}

fn wrap(a: f64, near: f64) -> f64 {
    let mut x = a;
    while x - near > std::f64::consts::PI {
        x -= std::f64::consts::TAU;
    }
    while near - x > std::f64::consts::PI {
        x += std::f64::consts::TAU;
    }
    x
}

/// Rotating-frame state from an attributable and a range hypothesis.
fn state_from(a: &Attributable, rho: f64, rho_dot: f64) -> [f64; 6] {
    let (sa, ca) = a.ra.sin_cos();
    let (sd, cd) = a.dec.sin_cos();
    let u = [cd * ca, cd * sa, sd];
    let du_da = [-cd * sa, cd * ca, 0.0];
    let du_dd = [-sd * ca, -sd * sa, cd];
    let d_in = u.map(|x| x * rho);
    let dd_in: [f64; 3] =
        [0, 1, 2].map(|k| rho_dot * u[k] + rho * (a.ra_rate * du_da[k] + a.dec_rate * du_dd[k]));
    let rel = rot(-a.t, d_in);
    let r = [rel[0] + EARTH[0], rel[1] + EARTH[1], rel[2]];
    let vr = rot(-a.t, dd_in);
    let w = cross_z(rel);
    [r[0], r[1], r[2], vr[0] - w[0], vr[1] - w[1], vr[2] - w[2]]
}

/// Propagate a rotating-frame state from `t0` and give the angles at each time (sorted).
fn predict(x0: &[f64], t0: f64, times: &[f64]) -> Vec<(f64, f64)> {
    let mut s = Cr3bpState {
        r: [x0[0], x0[1], x0[2]],
        v: [x0[3], x0[4], x0[5]],
    };
    let mut t = t0;
    let mut out = Vec::with_capacity(times.len());
    for &tt in times {
        let dt = tt - t;
        if dt.abs() > 0.0 {
            let n = ((dt.abs() / STEP_TU).ceil() as usize).max(1);
            s = propagate_cr3bp(s, MU, dt, n);
            t = tt;
        }
        out.push(angles(s.r, tt));
    }
    out
}

fn attributable(tr: &Tracklet) -> Result<Attributable, String> {
    let n = tr.obs.len();
    if n < 2 {
        return Err(format!("tracklet {}: at least two observations", tr.id));
    }
    let tm = tr.obs.iter().map(|o| o.t).sum::<f64>() / n as f64;
    let ra0 = tr.obs[0].ra_rad;
    let fit = |f: &dyn Fn(&Obs) -> f64| {
        let ym = tr.obs.iter().map(f).sum::<f64>() / n as f64;
        let sxx: f64 = tr.obs.iter().map(|o| (o.t - tm).powi(2)).sum();
        let sxy: f64 = tr.obs.iter().map(|o| (o.t - tm) * (f(o) - ym)).sum();
        (ym, if sxx > 0.0 { sxy / sxx } else { 0.0 })
    };
    let (ra, rar) = fit(&|o: &Obs| wrap(o.ra_rad, ra0));
    let (dec, decr) = fit(&|o: &Obs| o.dec_rad);
    Ok(Attributable {
        tracklet: tr.id.clone(),
        t: tm,
        ra,
        dec,
        ra_rate: rar,
        dec_rate: decr,
    })
}

fn flatten(obs: &[&Obs]) -> (Vec<f64>, Vec<f64>) {
    let times = obs.iter().map(|o| o.t).collect();
    let z = obs.iter().flat_map(|o| [o.ra_rad, o.dec_rad]).collect();
    (times, z)
}

fn model(x: &[f64], t0: f64, times: &[f64], z: &[f64]) -> Vec<f64> {
    predict(x, t0, times)
        .iter()
        .enumerate()
        .flat_map(|(i, (ra, dec))| [wrap(*ra, z[2 * i]) * dec.cos(), *dec])
        .collect()
}

fn scaled(z: &[f64]) -> Vec<f64> {
    // RA residuals scaled by cos(dec) so both components are angles on the sky.
    z.chunks(2)
        .flat_map(|c| [c[0] * c[1].cos(), c[1]])
        .collect()
}

fn nrms(x: &[f64], t0: f64, times: &[f64], z: &[f64], sig: f64) -> f64 {
    let h = model(x, t0, times, z);
    let zs = scaled(z);
    let ss: f64 = h.iter().zip(&zs).map(|(a, b)| (a - b).powi(2)).sum();
    (ss / h.len() as f64).sqrt() / sig
}

fn inv(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut m: Vec<Vec<f64>> = a
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut v = r.clone();
            v.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            v
        })
        .collect();
    for c in 0..n {
        let p = (c..n).max_by(|&x, &y| m[x][c].abs().total_cmp(&m[y][c].abs()))?;
        if m[p][c].abs() < 1e-300 {
            return None;
        }
        m.swap(c, p);
        let d = m[c][c];
        m[c].iter_mut().for_each(|v| *v /= d);
        for r in 0..n {
            if r != c {
                let f = m[r][c];
                if f != 0.0 {
                    let row = m[c].clone();
                    m[r].iter_mut().zip(&row).for_each(|(v, w)| *v -= f * w);
                }
            }
        }
    }
    Some(m.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// Run the study.
pub fn run(q: &Request) -> Result<Report, String> {
    if q.data_source.trim().is_empty() {
        return Err("data_source must be stated".into());
    }
    if !(q.sigma_arcsec > 0.0
        && q.range_max > q.range_min
        && q.range_min > 0.0
        && q.range_rate_max >= 0.0)
    {
        return Err("sigma_arcsec > 0 and 0 < range_min < range_max are needed".into());
    }
    let sig = (q.sigma_arcsec / 3600.0).to_radians();
    let mut trs = q.tracklets.clone();
    trs.sort_by(|a, b| a.obs[0].t.total_cmp(&b.obs[0].t));
    let atts: Vec<Attributable> = trs.iter().map(attributable).collect::<Result<_, _>>()?;
    let n = trs.len();

    // Pairwise association.
    let mut links = vec![];
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            let a = &atts[i];
            let obs: Vec<&Obs> = trs[j].obs.iter().collect();
            let (times, z) = flatten(&obs);
            let cost = |rho: f64, rr: f64| nrms(&state_from(a, rho, rr), a.t, &times, &z, sig);
            let mut best = (f64::INFINITY, 0.0, 0.0);
            for ki in 0..q.n_range {
                let rho = q.range_min
                    + (q.range_max - q.range_min) * ki as f64 / (q.n_range - 1).max(1) as f64;
                for kj in 0..q.n_range_rate {
                    let rr = -q.range_rate_max
                        + 2.0 * q.range_rate_max * kj as f64 / (q.n_range_rate - 1).max(1) as f64;
                    let c = cost(rho, rr);
                    if c < best.0 {
                        best = (c, rho, rr);
                    }
                }
            }
            // Refine the full state on both tracklets together (a pairwise initial orbit).
            let mut both: Vec<&Obs> = trs[i].obs.iter().chain(trs[j].obs.iter()).collect();
            both.sort_by(|x, y| x.t.total_cmp(&y.t));
            let (tb, zb) = flatten(&both);
            let zs = scaled(&zb);
            let w = vec![1.0 / (sig * sig); zs.len()];
            let x0 = state_from(a, best.1, best.2);
            let h = |p: &[f64]| model(p, a.t, &tb, &zb);
            let joint = gauss_newton(h, &zs, &w, &x0, 30, 1e-12)
                .map(|res| nrms(&res.x, a.t, &tb, &zb, sig))
                .filter(|c| c.is_finite())
                .unwrap_or(f64::INFINITY);
            best.0 = joint;
            if best.0 <= q.gate {
                links.push(Link {
                    from: trs[i].id.clone(),
                    to: trs[j].id.clone(),
                    residual: round(best.0, 3),
                    range: round(best.1, 5),
                    range_rate: round(best.2, 5),
                });
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut objects = vec![];
    let mut unassociated = vec![];
    for (k, (_, mem)) in groups.into_iter().enumerate() {
        if mem.len() < 2 {
            unassociated.push(trs[mem[0]].id.clone());
            continue;
        }
        let first = &atts[mem[0]];
        let link = links
            .iter()
            .find(|l| l.from == trs[mem[0]].id && mem.iter().any(|&m| trs[m].id == l.to));
        let x0 = link
            .map(|l| state_from(first, l.range, l.range_rate))
            .unwrap_or_else(|| state_from(first, (q.range_min + q.range_max) / 2.0, 0.0));
        let mut obs: Vec<&Obs> = mem.iter().flat_map(|&m| trs[m].obs.iter()).collect();
        obs.sort_by(|a, b| a.t.total_cmp(&b.t));
        let (times, z) = flatten(&obs);
        let zs = scaled(&z);
        let w = vec![1.0 / (sig * sig); zs.len()];
        let h = |p: &[f64]| model(p, first.t, &times, &z);
        let fit = gauss_newton(h, &zs, &w, &x0, 30, 1e-12);
        let (state, rms, ps, vs) = match fit {
            Some(f) if f.converged || f.rms_residual.is_finite() => {
                let jac = fd_jacobian(&h, &f.x, zs.len());
                let mut a = vec![vec![0.0; 6]; 6];
                for (i, row) in jac.iter().enumerate() {
                    for p in 0..6 {
                        for qq in 0..6 {
                            a[p][qq] += row[p] * w[i] * row[qq];
                        }
                    }
                }
                let cov = inv(&a);
                let st: [f64; 6] = [f.x[0], f.x[1], f.x[2], f.x[3], f.x[4], f.x[5]];
                let ps = cov
                    .as_ref()
                    .map(|c| (c[0][0] + c[1][1] + c[2][2]).max(0.0).sqrt() * LU_KM);
                let vs = cov
                    .as_ref()
                    .map(|c| (c[3][3] + c[4][4] + c[5][5]).max(0.0).sqrt() * LU_KM * 1e3 / TU_S);
                (Some(st), Some(f.rms_residual.to_degrees() * 3600.0), ps, vs)
            }
            _ => (None, None, None, None),
        };
        let labels: Vec<Option<String>> =
            mem.iter().map(|&m| trs[m].truth_object.clone()).collect();
        let truth_label = labels[0]
            .clone()
            .filter(|l| labels.iter().all(|x| x.as_deref() == Some(l)));
        let (pe, w3) = match (
            &truth_label,
            state,
            q.truth
                .iter()
                .find(|t| Some(&t.name) == truth_label.as_ref()),
        ) {
            (Some(_), Some(st), Some(tr)) => {
                let s = propagate_cr3bp(
                    Cr3bpState { r: tr.r, v: tr.v },
                    MU,
                    first.t,
                    ((first.t / STEP_TU).ceil() as usize).max(1),
                );
                let e = ((st[0] - s.r[0]).powi(2)
                    + (st[1] - s.r[1]).powi(2)
                    + (st[2] - s.r[2]).powi(2))
                .sqrt()
                    * LU_KM;
                (Some(round(e, 1)), ps.map(|p| e <= 3.0 * p))
            }
            _ => (None, None),
        };
        objects.push(Object {
            id: format!("OBJ-{}", k + 1),
            tracklets: mem.iter().map(|&m| trs[m].id.clone()).collect(),
            epoch_tu: round(first.t, 6),
            state: state.map(|s| s.map(|x| round(x, 9))),
            rms_arcsec: rms.map(|x| round(x, 3)),
            pos_sigma_km: ps.map(|x| round(x, 1)),
            vel_sigma_m_s: vs.map(|x| round(x, 3)),
            truth_object: truth_label,
            pos_error_km: pe,
            within_3_sigma: w3,
        });
    }
    let scored = trs.iter().all(|t| t.truth_object.is_some());
    let (purity, completeness) = if scored && !objects.is_empty() {
        let pure = objects.iter().filter(|o| o.truth_object.is_some()).count() as f64
            / objects.len() as f64;
        let mut truth_sets: BTreeMap<String, usize> = BTreeMap::new();
        for t in &trs {
            *truth_sets
                .entry(t.truth_object.clone().unwrap_or_default())
                .or_insert(0) += 1;
        }
        let whole = truth_sets
            .iter()
            .filter(|(name, cnt)| {
                objects.iter().any(|o| {
                    o.truth_object.as_deref() == Some(name.as_str()) && o.tracklets.len() == **cnt
                })
            })
            .count() as f64
            / truth_sets.len() as f64;
        (Some(round(pure, 3)), Some(round(whole, 3)))
    } else {
        (None, None)
    };
    let verdict = format!(
        "{} tracklet(s) → {} object(s), {} unassociated; {} link(s){} (MODELLED)",
        n,
        objects.len(),
        unassociated.len(),
        links.len(),
        match (purity, completeness) {
            (Some(p), Some(c)) => format!(
                "; purity {:.0} %, completeness {:.0} %",
                100.0 * p,
                100.0 * c
            ),
            _ => String::new(),
        }
    );
    Ok(Report {
        format: FORMAT,
        evidence: EVIDENCE,
        title: q.title.clone(),
        data_source: q.data_source.clone(),
        attributables: atts
            .into_iter()
            .map(|a| Attributable {
                ra: round(a.ra, 9),
                dec: round(a.dec, 9),
                ra_rate: round(a.ra_rate, 9),
                dec_rate: round(a.dec_rate, 9),
                t: round(a.t, 6),
                ..a
            })
            .collect(),
        links,
        objects,
        unassociated,
        purity,
        completeness,
        limits: vec![
            "Circular restricted three-body dynamics in the Earth-Moon synodic frame: no solar gravity, solar radiation pressure or lunar eccentricity.".into(),
            "Observer at the Earth's centre; no light time, aberration, refraction or site geometry.".into(),
            "Association links pairs within the gate and joins connected tracklets; a wrong link merges objects (reported through purity on synthetic data).".into(),
            "Covariance from the finite-difference normal matrix at the solution: a formal, linearised bound.".into(),
        ],
        verdict,
    })
}

/// HTML page.
pub fn html(r: &Report) -> String {
    let o = |x: Option<f64>| x.map(|v| format!("{v}")).unwrap_or_else(|| "-".into());
    crate::transport::page(
        &r.title,
        r.evidence,
        &r.verdict,
        &[format!("Data source: {}.", r.data_source)],
        &[
            (
                "Objects".into(),
                vec![
                    "object",
                    "tracklets",
                    "RMS (arcsec)",
                    "position σ (km)",
                    "velocity σ (m/s)",
                    "truth",
                    "error (km)",
                    "within 3σ",
                ],
                r.objects
                    .iter()
                    .map(|x| {
                        vec![
                            x.id.clone(),
                            x.tracklets.join(", "),
                            o(x.rms_arcsec),
                            o(x.pos_sigma_km),
                            o(x.vel_sigma_m_s),
                            x.truth_object.clone().unwrap_or_default(),
                            o(x.pos_error_km),
                            x.within_3_sigma.map(|b| b.to_string()).unwrap_or_default(),
                        ]
                    })
                    .collect(),
            ),
            (
                "Links".into(),
                vec![
                    "from",
                    "to",
                    "normalised residual",
                    "range (LU)",
                    "range rate (LU/TU)",
                ],
                r.links
                    .iter()
                    .map(|l| {
                        vec![
                            l.from.clone(),
                            l.to.clone(),
                            format!("{}", l.residual),
                            format!("{}", l.range),
                            format!("{}", l.range_rate),
                        ]
                    })
                    .collect(),
            ),
            (
                "Limits".into(),
                vec!["limit"],
                r.limits.iter().map(|l| vec![l.clone()]).collect(),
            ),
        ],
    )
}

/// A SYNTHETIC study: three cislunar objects (near L4, near L5, and a lunar-vicinity orbit)
/// propagated in the CR3BP, each seen in three tracklets of four observations over about two
/// hours, one night apart, with 1 arcsec noise; tracklets shuffled and unlabelled for the
/// method (labels kept for scoring).
pub fn synth() -> Request {
    let truth = vec![
        TruthObject {
            name: "SYN-L4".into(),
            r: [0.5 - MU + 0.02, 0.866_025 + 0.01, 0.01],
            v: [0.0, 0.005, 0.0],
        },
        TruthObject {
            name: "SYN-L5".into(),
            r: [0.5 - MU - 0.01, -0.866_025 + 0.02, -0.02],
            v: [0.004, 0.0, 0.002],
        },
        TruthObject {
            name: "SYN-LUNAR".into(),
            r: [1.0 - MU - 0.15, 0.0, 0.05],
            v: [0.0, 0.45, 0.0],
        },
    ];
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut noise = || {
        let mut u = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64
        };
        (u() + u() + u() + u() - 2.0) * 1.732
    };
    let sig = (1.0f64 / 3600.0).to_radians();
    let night = 86_400.0 / TU_S;
    let mut tracklets = vec![];
    for (k, o) in truth.iter().enumerate() {
        for n in 0..3 {
            let t0 = 0.05 + n as f64 * night + k as f64 * 0.003;
            let times: Vec<f64> = (0..4).map(|i| t0 + i as f64 * 2400.0 / TU_S).collect();
            let s0 = [o.r[0], o.r[1], o.r[2], o.v[0], o.v[1], o.v[2]];
            let ang = predict(&s0, 0.0, &times);
            tracklets.push(Tracklet {
                id: format!("TRK-{:02}", tracklets.len() + 1),
                obs: times
                    .iter()
                    .zip(ang)
                    .map(|(t, (ra, dec))| Obs {
                        t: *t,
                        ra_rad: (ra + noise() * sig / dec.cos()).rem_euclid(std::f64::consts::TAU),
                        dec_rad: dec + noise() * sig,
                    })
                    .collect(),
                truth_object: Some(o.name.clone()),
            });
        }
    }
    // Interleave so the order says nothing about identity.
    tracklets.sort_by(|a, b| a.obs[0].t.total_cmp(&b.obs[0].t));
    for (i, t) in tracklets.iter_mut().enumerate() {
        t.id = format!("TRK-{:02}", i + 1);
    }
    Request {
        title: "SYNTHETIC cislunar catalogue build-up".into(),
        data_source: "SYNTHETIC (kshana-pro cislunar-catalogue synth): CR3BP truth, 1 arcsec noise"
            .into(),
        sigma_arcsec: 1.0,
        range_min: 0.3,
        range_max: 1.3,
        range_rate_max: 0.6,
        n_range: d_nr(),
        n_range_rate: d_nrr(),
        gate: d_gate(),
        tracklets,
        truth,
    }
}
