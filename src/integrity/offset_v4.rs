// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Stage 2 of the AIS position-offset test, method `kshana-pro/m4-offset/v4` (roadmap 0.42 V3),
//! exactly as pre-registered in
//! `docs/evidence/v3-m4-offset-successor/2026-11-15_PREREGISTRATION.md` (section 2).
//!
//! Stage 1 is the open engine's committed M4 v3 decision (`kshana::m4_logic::decide` with
//! `M4Params::v3()`), unchanged. Stage 2 looks, for every identity stage 1 left NORMAL, for a
//! persistent RF track that moves parallel to the AIS track at a constant displacement (the
//! vessel's own fixes when its broadcast positions are offset). It may change only NORMAL to
//! POSITION-OFFSET; fix flags are never changed. Outputs describe data consistency ("AIS position
//! inconsistent with a persistent, parallel RF track"), never intent. MODELLED.

use std::collections::BTreeMap;

use kshana::m4_dark_vessel::{Label, R_M};
use kshana::m4_logic::Decision;
use kshana::portable_math::PortableFloat;
use serde::Serialize;

const DEG: f64 = std::f64::consts::PI / 180.0;

/// The method identifier written into every output.
pub const METHOD_ID: &str = "kshana-pro/m4-offset/v4";
/// Fix noise per axis, m (the generator's, as in M4).
pub const SIGMA_M: f64 = 1000.0;
/// Chi-square gate, 2 degrees of freedom, 0.999.
pub const GATE: f64 = 13.816;
/// Largest candidate displacement, m.
pub const R_MAX_M: f64 = 60_000.0;
/// Largest AIS report spacing interpolated over, s.
pub const INTERP_MAX_S: u32 = 60;
/// Epoch tolerance, s.
pub const EPOCH_TOL_S: u32 = 60;
/// Shortest span of a supporting RF track, s.
pub const MIN_SPAN_S: u32 = 600;
/// Speed above which consecutive reports show two vessels on one identity, kn (M4's).
pub const MAX_SPEED_KN: f64 = 70.0;

/// The two tuned numbers of stage 2.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Params {
    /// Supporting epochs needed.
    pub m: usize,
    /// Pairwise consistency gate (chi-square, 2 degrees of freedom).
    pub g_pair: f64,
    /// The V3b corrections (`2026-11-15_V3b_PREREGISTRATION.md`): support never includes a fix
    /// consistent with the AIS position, and the final support uses the gate `G` around the
    /// refined offset. `false` is V3 as registered.
    pub b: bool,
}

/// The development grid of the pre-registration.
pub const GRID: [Params; 4] = [
    Params { m: 2, g_pair: 13.816, b: false },
    Params { m: 2, g_pair: 18.421, b: false },
    Params { m: 3, g_pair: 13.816, b: false },
    Params { m: 3, g_pair: 18.421, b: false },
];

/// The pair chosen on development seeds 50 to 59 (`docs/evidence/v3-m4-offset-successor/dev.json`):
/// no pair had FA <= 0.005 (the lowest was 0.140, at m = 3 and 13.816), so by the
/// pre-registered rule the lowest-FA pair is taken and the miss is expected.
pub const V3_CHOSEN: Params = Params {
    m: 3,
    g_pair: 13.816,
    b: false,
};

/// The V3b development grid (seeds 60 to 69).
pub const GRID_B: [Params; 4] = [
    Params { m: 2, g_pair: 13.816, b: true },
    Params { m: 2, g_pair: 18.421, b: true },
    Params { m: 3, g_pair: 13.816, b: true },
    Params { m: 3, g_pair: 18.421, b: true },
];

/// One identity's offset finding.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OffsetFinding {
    /// The AIS identity.
    pub id: u64,
    /// First supporting fix time, s.
    pub onset_s: u32,
    /// Estimated displacement of the RF track from the AIS track, east and north, m.
    pub offset_en_m: [f64; 2],
    /// Its length, m.
    pub offset_m: f64,
    /// Supporting epochs.
    pub support: usize,
    /// Time from the first to the last supporting fix, s.
    pub span_s: u32,
}

/// Local east and north metres of `b` from `a` (M4's formula).
pub fn en(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (
        (b.1 - a.1) * DEG * R_M * (a.0 * DEG).pcos(),
        (b.0 - a.0) * DEG * R_M,
    )
}

fn implied_kn(a: (u32, f64, f64), b: (u32, f64, f64)) -> f64 {
    let (e, n) = en((a.1, a.2), (b.1, b.2));
    let dt = f64::from(b.0.saturating_sub(a.0).max(1));
    (e * e + n * n).sqrt() / dt * 3600.0 / 1852.0
}

/// One AIS track: an identity's reports `(t, lat, lon)` sorted by time.
#[derive(Clone, Debug)]
pub struct Track {
    /// The identity.
    pub id: u64,
    /// Reports.
    pub rep: Vec<(u32, f64, f64)>,
    bbox: [f64; 4],
}

impl Track {
    fn new(id: u64, rep: Vec<(u32, f64, f64)>) -> Self {
        let mut b = [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY];
        for r in &rep {
            b[0] = b[0].min(r.1);
            b[1] = b[1].max(r.1);
            b[2] = b[2].min(r.2);
            b[3] = b[3].max(r.2);
        }
        Self { id, rep, bbox: b }
    }

    /// The AIS position at `t`: a report at `t`, or the linear interpolation between the
    /// reports around `t` when they are at most [`INTERP_MAX_S`] apart (M4's rule).
    pub fn predict(&self, t: u32) -> Option<(f64, f64)> {
        let k = self.rep.partition_point(|r| r.0 < t);
        if k < self.rep.len() && self.rep[k].0 == t {
            return Some((self.rep[k].1, self.rep[k].2));
        }
        if k == 0 || k == self.rep.len() {
            return None;
        }
        let (a, b) = (self.rep[k - 1], self.rep[k]);
        if b.0 - a.0 > INTERP_MAX_S {
            return None;
        }
        let w = f64::from(t - a.0) / f64::from(b.0 - a.0);
        Some((a.1 + w * (b.1 - a.1), a.2 + w * (b.2 - a.2)))
    }

    /// Whether `(lat, lon)` can be within `margin_m` of the track at all.
    fn near_box(&self, lat: f64, lon: f64, margin_m: f64) -> bool {
        let dlat = margin_m / R_M / DEG;
        let dlon = margin_m / (R_M * (lat.abs().min(85.0) * DEG).pcos()) / DEG;
        lat >= self.bbox[0] - dlat
            && lat <= self.bbox[1] + dlat
            && lon >= self.bbox[2] - dlon
            && lon <= self.bbox[3] + dlon
    }
}

/// The AIS tracks of a stream: one per identity, or, for an identity stage 1 decided
/// IDENTITY-DUPLICATE, the parts M4 splits it into by the speed rule.
pub fn tracks(ais: &[(u32, u64, f64, f64)], stage1: &BTreeMap<u64, Label>) -> Vec<Track> {
    let mut by_id: BTreeMap<u64, Vec<(u32, f64, f64)>> = BTreeMap::new();
    for &(t, id, a, b) in ais {
        by_id.entry(id).or_default().push((t, a, b));
    }
    let mut out = Vec::new();
    for (id, mut rep) in by_id {
        rep.sort_by(|a, b| a.0.cmp(&b.0));
        if stage1.get(&id) == Some(&Label::IdentityDuplicate) {
            let mut parts: Vec<Vec<(u32, f64, f64)>> = Vec::new();
            for &r in &rep {
                let pick = parts
                    .iter()
                    .position(|v| implied_kn(*v.last().unwrap_or(&r), r) <= MAX_SPEED_KN);
                match pick {
                    Some(k) => parts[k].push(r),
                    None => parts.push(vec![r]),
                }
            }
            out.extend(parts.into_iter().map(|v| Track::new(id, v)));
        } else {
            out.push(Track::new(id, rep));
        }
    }
    out
}

/// Each fix's best AIS track (index into `tracks`) within the gate, or `None` (step 1).
pub fn associate(tracks: &[Track], rf: &[(u32, f64, f64)]) -> Vec<Option<usize>> {
    let s2 = SIGMA_M * SIGMA_M;
    let margin = (GATE * s2).sqrt() + 1000.0;
    rf.iter()
        .map(|&(t, a, b)| {
            let mut best: Option<(f64, usize)> = None;
            for (k, tr) in tracks.iter().enumerate() {
                if !tr.near_box(a, b, margin) {
                    continue;
                }
                if let Some(q) = tr.predict(t) {
                    let (e, n) = en(q, (a, b));
                    let d2 = (e * e + n * n) / s2;
                    if d2 <= GATE && best.is_none_or(|x| d2 < x.0) {
                        best = Some((d2, k));
                    }
                }
            }
            best.map(|x| x.1)
        })
        .collect()
}

/// A candidate: time and displacement from the AIS prediction (east, north, m).
type Cand = (u32, (f64, f64));

/// Candidates of track `k` grouped into epochs (steps 2 and 3).
fn candidate_epochs(
    tracks: &[Track],
    k: usize,
    rf: &[(u32, f64, f64)],
    assoc: &[Option<usize>],
) -> Vec<Vec<Cand>> {
    let tr = &tracks[k];
    let mut cands: Vec<Cand> = Vec::new();
    for (j, &(t, a, b)) in rf.iter().enumerate() {
        if assoc[j].is_some_and(|x| tracks[x].id != tr.id) {
            continue;
        }
        if !tr.near_box(a, b, R_MAX_M) {
            continue;
        }
        let Some(q) = tr.predict(t) else { continue };
        let v = en(q, (a, b));
        if (v.0 * v.0 + v.1 * v.1).sqrt() <= R_MAX_M {
            cands.push((t, v));
        }
    }
    cands.sort_by(|a, b| a.0.cmp(&b.0));
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

/// The support of offset `o` from epoch `from` on: per epoch, its candidate closest to `o`
/// when within the pairwise gate (step 4). Returns (epoch index, candidate).
fn support(
    epochs: &[Vec<Cand>],
    from: usize,
    o: (f64, f64),
    lim: f64,
    exclude_zero: bool,
) -> Vec<(usize, Cand)> {
    let zero = GATE * SIGMA_M * SIGMA_M;
    let mut out = Vec::new();
    for (i, e) in epochs.iter().enumerate().skip(from) {
        let best = e
            .iter()
            .filter(|c| !exclude_zero || c.1 .0 * c.1 .0 + c.1 .1 * c.1 .1 > zero)
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

/// Steps 4 to 7 for one track's epochs: the first passing hypothesis, in time order.
fn test_track(epochs: &[Vec<Cand>], p: Params) -> Option<(u32, (f64, f64), usize, u32)> {
    let s2 = SIGMA_M * SIGMA_M;
    let r_min2 = GATE * s2;
    for (ec, e) in epochs.iter().enumerate() {
        for c in e {
            if c.1 .0 * c.1 .0 + c.1 .1 * c.1 .1 < r_min2 {
                continue;
            }
            let pair = p.g_pair * 2.0 * s2;
            let s0 = support(epochs, ec, c.1, pair, p.b);
            if s0.is_empty() {
                continue;
            }
            let o_hat = mean(&s0);
            let fin = if p.b { GATE * s2 } else { pair };
            let s = support(epochs, ec, o_hat, fin, p.b);
            if s.len() < p.m {
                continue;
            }
            let t_first = s.iter().map(|x| x.1 .0).min().unwrap_or(0);
            let t_last = s.iter().map(|x| x.1 .0).max().unwrap_or(0);
            if t_last - t_first < MIN_SPAN_S {
                continue;
            }
            if o_hat.0 * o_hat.0 + o_hat.1 * o_hat.1 < r_min2 {
                continue;
            }
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
            return Some((t_first, o_hat, s.len(), t_last - t_first));
        }
    }
    None
}

/// Stage 2 over a whole stream: the new classes (only NORMAL may become POSITION-OFFSET) and
/// the findings, in identity order.
pub fn stage2(
    ais: &[(u32, u64, f64, f64)],
    rf: &[(u32, f64, f64)],
    stage1: &BTreeMap<u64, Label>,
    p: Params,
) -> (BTreeMap<u64, Label>, Vec<OffsetFinding>) {
    let trs = tracks(ais, stage1);
    let assoc = associate(&trs, rf);
    let mut class = stage1.clone();
    let mut found = Vec::new();
    for k in 0..trs.len() {
        let id = trs[k].id;
        if stage1.get(&id) != Some(&Label::Normal) {
            continue;
        }
        let epochs = candidate_epochs(&trs, k, rf, &assoc);
        if let Some((t0, o, n, span)) = test_track(&epochs, p) {
            class.insert(id, Label::PositionOffset);
            found.push(OffsetFinding {
                id,
                onset_s: t0,
                offset_en_m: [o.0, o.1],
                offset_m: (o.0 * o.0 + o.1 * o.1).sqrt(),
                support: n,
                span_s: span,
            });
        }
    }
    (class, found)
}

/// Stage 2 for each of several parameter pairs, sharing the tracks and the association.
pub fn stage2_grid(
    ais: &[(u32, u64, f64, f64)],
    rf: &[(u32, f64, f64)],
    stage1: &BTreeMap<u64, Label>,
    grid: &[Params],
) -> Vec<BTreeMap<u64, Label>> {
    let trs = tracks(ais, stage1);
    let assoc = associate(&trs, rf);
    let mut out: Vec<BTreeMap<u64, Label>> = grid.iter().map(|_| stage1.clone()).collect();
    for k in 0..trs.len() {
        let id = trs[k].id;
        if stage1.get(&id) != Some(&Label::Normal) {
            continue;
        }
        let epochs = candidate_epochs(&trs, k, rf, &assoc);
        for (g, p) in grid.iter().enumerate() {
            if test_track(&epochs, *p).is_some() {
                out[g].insert(id, Label::PositionOffset);
            }
        }
    }
    out
}

/// The stage-1 decision with stage 2's classes (fix flags unchanged).
pub fn with_classes(d: &Decision, class: BTreeMap<u64, Label>) -> Decision {
    Decision {
        class,
        dark_fix: d.dark_fix.clone(),
        taken: d.taken.clone(),
    }
}
