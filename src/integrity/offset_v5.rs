// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Stage 2 of the AIS position-offset test, method `kshana-pro/m4-offset/v5` (roadmap 0.42 V3c),
//! as pre-registered in `docs/evidence/v3-m4-offset-successor/2026-11-16_V3c_PREREGISTRATION.md`.
//!
//! This file first carries only the development diagnostic the pre-registration asks to publish
//! before the method is built: the **recall ceiling**, from the generator's truth.

use std::collections::BTreeMap;

use kshana::m4_dark_vessel::Scene;

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
