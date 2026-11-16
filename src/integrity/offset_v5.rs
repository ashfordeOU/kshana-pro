// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Stage 2 of the AIS position-offset test, method `kshana-pro/m4-offset/v5` (roadmap 0.42 V3c),
//! as pre-registered in `docs/evidence/v3-m4-offset-successor/2026-11-16_V3c_PREREGISTRATION.md`.
//!
//! Stage 1 is the open engine's committed M4 v3 decision, unchanged. Stage 2 is V3b's
//! (`offset_v4` with `b = true`: steps 1 to 3, own-position fixes excluded, seeding with the pairwise
//! gate 13.816, final support gate `G` around the refined offset, the contradiction test), with:
//!
//! 1. a **relative-motion test**: over the final support, `v(t) = o + w (t - t̄)` is fitted by least
//!    squares and the hypothesis is kept only if `|ŵ|² Stt / σ² <= G` (no drift of the displacement);
//! 2. **persistence**: at least `m` support epochs whose first and last are at least `T_span` apart;
//! 3. **exclusivity**: a fix supports the accepted hypothesis of one identity only; the identity with
//!    the smaller relative-motion statistic keeps a contested fix and the other is re-tested without it;
//! 4. the decision: POSITION-OFFSET; the `dark_fix` flags of the accepted support's fixes are cleared
//!    (they are explained by the offset), and no other fix flag changes.
//!
//! The open v3 decision gives DARK only through fix flags (an AIS identity's class is never DARK),
//! so "stage-1 class NORMAL or DARK" is, in method terms, every identity whose stage-1 class is
//! NORMAL; clearing only the support's flags is the method-observable form of the pre-registration's
//! rule that a DARK outcome may change only where it rests on fixes in the accepted support.
//! Outputs describe data consistency, never intent. MODELLED.
//!
//! Also here: the development diagnostic published before the method was built, the **recall
//! ceiling** from the generator's truth.

use std::collections::{BTreeMap, BTreeSet};

use kshana::m4_dark_vessel::{Label, Scene};
use kshana::m4_logic::Decision;
use serde::Serialize;

use super::offset_v4::{associate, en, tracks, Track, GATE, R_MAX_M, SIGMA_M};

/// The method identifier written into every output.
pub const METHOD_ID: &str = "kshana-pro/m4-offset/v5";
/// V3b's seeding gate (its chosen pair), kept.
pub const G_PAIR: f64 = 13.816;
/// Epoch tolerance, s (V3's).
const EPOCH_TOL_S: u32 = 60;
const DEG: f64 = std::f64::consts::PI / 180.0;

/// The two tuned numbers of V3c.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Params {
    /// Support epochs needed.
    pub m: usize,
    /// Shortest time from the first to the last support epoch, s.
    pub t_span_s: u32,
}

/// The development grid, in the tie-break order (smaller `m`, then smaller `T_span`).
pub const GRID: [Params; 4] = [
    Params {
        m: 3,
        t_span_s: 1200,
    },
    Params {
        m: 3,
        t_span_s: 1800,
    },
    Params {
        m: 4,
        t_span_s: 1200,
    },
    Params {
        m: 4,
        t_span_s: 1800,
    },
];

/// The pair chosen on development seeds 70 to 79 (`V3c_dev.json`): no pair had FA <= 0.005 (the
/// lowest was 0.0070, at m = 4, equal for both `T_span`), so by the pre-registered rule the
/// lowest-FA pair is taken (ties to the smaller `T_span`) and the miss is expected.
pub const V3C_CHOSEN: Params = Params {
    m: 4,
    t_span_s: 1200,
};

/// One identity's accepted offset hypothesis.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OffsetFinding {
    pub id: u64,
    pub onset_s: u32,
    pub offset_en_m: [f64; 2],
    pub offset_m: f64,
    pub support: usize,
    pub span_s: u32,
    /// `|ŵ|² Stt / σ²`.
    pub relative_motion_stat: f64,
    /// The RF fixes of the support (indices into the stream's fixes).
    #[serde(skip)]
    pub fixes: Vec<usize>,
}

/// A candidate: time, displacement from the AIS prediction (east, north, m), fix index.
type Cand = (u32, (f64, f64), usize);

fn candidate_epochs(
    trs: &[Track],
    k: usize,
    rf: &[(u32, f64, f64)],
    assoc: &[Option<usize>],
    banned: &BTreeSet<usize>,
) -> Vec<Vec<Cand>> {
    let tr = &trs[k];
    // A box around the track's reports, R_MAX_M wide (a prefilter only).
    let (mut la0, mut la1, mut lo0, mut lo1) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for r in &tr.rep {
        la0 = la0.min(r.1);
        la1 = la1.max(r.1);
        lo0 = lo0.min(r.2);
        lo1 = lo1.max(r.2);
    }
    let dlat = (R_MAX_M + 1000.0) / kshana::m4_dark_vessel::R_M / DEG;
    let dlon = dlat / (la0.abs().max(la1.abs()).min(85.0) * DEG).cos();
    let mut cands: Vec<Cand> = Vec::new();
    for (j, &(t, a, b)) in rf.iter().enumerate() {
        if a < la0 - dlat || a > la1 + dlat || b < lo0 - dlon || b > lo1 + dlon {
            continue;
        }
        if banned.contains(&j) || assoc[j].is_some_and(|x| trs[x].id != tr.id) {
            continue;
        }
        let Some(q) = tr.predict(t) else { continue };
        let v = en(q, (a, b));
        if (v.0 * v.0 + v.1 * v.1).sqrt() <= R_MAX_M {
            cands.push((t, v, j));
        }
    }
    cands.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    let mut epochs: Vec<Vec<Cand>> = Vec::new();
    for c in cands {
        match epochs.last_mut() {
            Some(e) if c.0 <= e[0].0 + EPOCH_TOL_S => e.push(c),
            _ => epochs.push(vec![c]),
        }
    }
    epochs
}

fn d2(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

/// Per epoch from `from` on, the candidate closest to `o` (never one consistent with the AIS
/// position) when within `lim` (m²).
fn support(epochs: &[Vec<Cand>], from: usize, o: (f64, f64), lim: f64) -> Vec<(usize, Cand)> {
    let zero = GATE * SIGMA_M * SIGMA_M;
    let mut out = Vec::new();
    for (i, e) in epochs.iter().enumerate().skip(from) {
        let best = e
            .iter()
            .filter(|c| c.1 .0 * c.1 .0 + c.1 .1 * c.1 .1 > zero)
            .min_by(|a, b| d2(a.1, o).total_cmp(&d2(b.1, o)))
            .copied();
        if let Some(c) = best {
            if d2(c.1, o) <= lim {
                out.push((i, c));
            }
        }
    }
    out
}

fn mean(s: &[(usize, Cand)]) -> (f64, f64) {
    let n = s.len().max(1) as f64;
    (
        s.iter().map(|x| x.1 .1 .0).sum::<f64>() / n,
        s.iter().map(|x| x.1 .1 .1).sum::<f64>() / n,
    )
}

/// `|ŵ|² Stt / σ²` of the least-squares drift of the displacements over the support.
pub fn relative_motion_stat(pts: &[(u32, (f64, f64))]) -> f64 {
    let n = pts.len() as f64;
    if pts.len() < 2 {
        return f64::INFINITY;
    }
    let tb = pts.iter().map(|p| f64::from(p.0)).sum::<f64>() / n;
    let eb = pts.iter().map(|p| p.1 .0).sum::<f64>() / n;
    let nb = pts.iter().map(|p| p.1 .1).sum::<f64>() / n;
    let stt: f64 = pts.iter().map(|p| (f64::from(p.0) - tb).powi(2)).sum();
    if stt <= 0.0 {
        return f64::INFINITY;
    }
    let we = pts
        .iter()
        .map(|p| (f64::from(p.0) - tb) * (p.1 .0 - eb))
        .sum::<f64>()
        / stt;
    let wn = pts
        .iter()
        .map(|p| (f64::from(p.0) - tb) * (p.1 .1 - nb))
        .sum::<f64>()
        / stt;
    (we * we + wn * wn) * stt / (SIGMA_M * SIGMA_M)
}

/// The first passing hypothesis of one track's epochs, in time order.
fn test_track(epochs: &[Vec<Cand>], p: Params, id: u64) -> Option<OffsetFinding> {
    let s2 = SIGMA_M * SIGMA_M;
    let r_min2 = GATE * s2;
    for (ec, e) in epochs.iter().enumerate() {
        for c in e {
            if c.1 .0 * c.1 .0 + c.1 .1 * c.1 .1 < r_min2 {
                continue;
            }
            let s0 = support(epochs, ec, c.1, G_PAIR * 2.0 * s2);
            if s0.is_empty() {
                continue;
            }
            let o_hat = mean(&s0);
            let s = support(epochs, ec, o_hat, GATE * s2);
            // Persistence.
            if s.len() < p.m {
                continue;
            }
            let t_first = s.iter().map(|x| x.1 .0).min().unwrap_or(0);
            let t_last = s.iter().map(|x| x.1 .0).max().unwrap_or(0);
            if t_last - t_first < p.t_span_s {
                continue;
            }
            if o_hat.0 * o_hat.0 + o_hat.1 * o_hat.1 < r_min2 {
                continue;
            }
            // Relative motion: the displacement must not drift.
            let pts: Vec<(u32, (f64, f64))> = s.iter().map(|x| (x.1 .0, x.1 .1)).collect();
            let stat = relative_motion_stat(&pts);
            if stat > GATE {
                continue;
            }
            // No contradiction after onset (V3b's change 3).
            let supported: Vec<usize> = s.iter().map(|x| x.0).collect();
            let contradicted = epochs.iter().enumerate().skip(ec).any(|(i, ep)| {
                !supported.contains(&i)
                    && ep.iter().any(|x| {
                        let z = x.1 .0 * x.1 .0 + x.1 .1 * x.1 .1;
                        z / s2 <= GATE && z < d2(x.1, o_hat)
                    })
            });
            if contradicted {
                continue;
            }
            return Some(OffsetFinding {
                id,
                onset_s: t_first,
                offset_en_m: [o_hat.0, o_hat.1],
                offset_m: (o_hat.0 * o_hat.0 + o_hat.1 * o_hat.1).sqrt(),
                support: s.len(),
                span_s: t_last - t_first,
                relative_motion_stat: stat,
                fixes: s.iter().map(|x| x.1 .2).collect(),
            });
        }
    }
    None
}

/// Stage 2 over a stream for one parameter pair: the accepted findings, one per track at most,
/// after exclusivity (keyed by track index).
fn accepted(
    trs: &[Track],
    rf: &[(u32, f64, f64)],
    assoc: &[Option<usize>],
    stage1: &BTreeMap<u64, Label>,
    p: Params,
) -> BTreeMap<usize, OffsetFinding> {
    let mut banned: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut found: BTreeMap<usize, OffsetFinding> = BTreeMap::new();
    let eligible: Vec<usize> = (0..trs.len())
        .filter(|&k| stage1.get(&trs[k].id) == Some(&Label::Normal))
        .collect();
    let mut todo: BTreeSet<usize> = eligible.iter().copied().collect();
    loop {
        for &k in &todo {
            let empty = BTreeSet::new();
            let b = banned.get(&k).unwrap_or(&empty);
            let epochs = candidate_epochs(trs, k, rf, assoc, b);
            match test_track(&epochs, p, trs[k].id) {
                Some(f) => {
                    found.insert(k, f);
                }
                None => {
                    found.remove(&k);
                }
            }
        }
        todo.clear();
        // Exclusivity: every contested fix goes to the smallest relative-motion statistic.
        let mut owner: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (&k, f) in &found {
            for &j in &f.fixes {
                owner.entry(j).or_default().push(k);
            }
        }
        for (j, ks) in owner {
            if ks.len() < 2 {
                continue;
            }
            let keep = *ks
                .iter()
                .min_by(|a, b| {
                    found[a]
                        .relative_motion_stat
                        .total_cmp(&found[b].relative_motion_stat)
                        .then(a.cmp(b))
                })
                .unwrap();
            for &k in ks.iter().filter(|&&k| k != keep) {
                banned.entry(k).or_default().insert(j);
                todo.insert(k);
            }
        }
        if todo.is_empty() {
            return found;
        }
    }
}

/// Stage 2 for each parameter pair, sharing the tracks and the association: per pair, the
/// decision (classes and fix flags) and the findings.
pub fn stage2_grid(
    scene: &Scene,
    d1: &Decision,
    grid: &[Params],
) -> Vec<(Decision, Vec<OffsetFinding>)> {
    let trs = tracks(&scene.ais, &d1.class);
    let assoc = associate(&trs, &scene.rf);
    grid.iter()
        .map(|&p| {
            let f = accepted(&trs, &scene.rf, &assoc, &d1.class, p);
            let mut class = d1.class.clone();
            let mut dark_fix = d1.dark_fix.clone();
            for g in f.values() {
                class.insert(g.id, Label::PositionOffset);
                for &j in &g.fixes {
                    dark_fix[j] = false;
                }
            }
            (
                Decision {
                    class,
                    dark_fix,
                    taken: d1.taken.clone(),
                },
                f.into_values().collect(),
            )
        })
        .collect()
}

/// The development grid of `m` (support epochs) and `T_span` (s).
pub const M_GRID: [usize; 2] = [3, 4];
/// The development grid of `T_span`, s.
pub const T_SPAN_GRID: [u32; 2] = [1200, 1800];

/// Per offset distance (km), the offset vessels of a scene and those with at least `m`
/// post-onset RF fixes of their own whose first and last are at least `t_span_s` apart (from the
/// generator's truth: `rf_owner`, `onset_s`). Returns `km -> (reachable, all)`.
pub fn recall_ceiling(scene: &Scene, m: usize, t_span_s: u32) -> BTreeMap<u32, (usize, usize)> {
    let mut times: BTreeMap<i64, Vec<u32>> = BTreeMap::new();
    for (f, &o) in scene.rf.iter().zip(&scene.rf_owner) {
        if o >= 0 {
            times.entry(o).or_default().push(f.0);
        }
    }
    let mut out: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    let vessels = scene.truth["vessels"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for (i, v) in vessels.iter().enumerate() {
        if v["label"] != "POSITION-OFFSET" {
            continue;
        }
        let km = v["offset_km"].as_f64().unwrap_or(0.0).round() as u32;
        let on = v["onset_s"].as_u64().unwrap_or(0) as u32;
        let post: Vec<u32> = times
            .get(&(i as i64))
            .map(|t| t.iter().copied().filter(|&t| t >= on).collect())
            .unwrap_or_default();
        let ok = post.len() >= m
            && post.iter().max().copied().unwrap_or(0) - post.iter().min().copied().unwrap_or(0)
                >= t_span_s;
        let e = out.entry(km).or_insert((0, 0));
        e.1 += 1;
        if ok {
            e.0 += 1;
        }
    }
    out
}
