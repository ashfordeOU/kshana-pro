// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Pass-to-pass drift from a machine log against an A-B line.
//!
//! Every epoch is put in the line's frame: along-track `s` and cross-track `x` (left of the
//! A-to-B direction positive). With swath width `w`, the pass an epoch belongs to is
//! `k = round(x / w)` and its deviation from that pass's centre is `x - k w`. A pass is a run
//! of epochs on one `k`, driving one way, at least [`DriftParams::min_pass_m`] long.
//!
//! * **Within-pass drift**: a straight-line fit of the deviation against time over the pass
//!   (the pass that "starts correctly then slowly starts missing"): start and end deviation,
//!   rate (m/min) and the largest deviation.
//! * **Adjacent-pass spacing error**: for passes `k` and `k+1`, sampled every metre over the
//!   along-track range both cover, `e(s) = dev_{k+1}(s) - dev_k(s)`: positive is a wider gap
//!   than the swath. Pairs driven within [`DriftParams::max_gap_min`] minutes of each other
//!   count as pass-to-pass (the usual convention is 15 minutes); pairs further apart are
//!   reported as repeatability, not pass-to-pass.
//! * **Overlap and miss**: the actual spacing is `w + e`; with implement width `W`, the miss
//!   strip is `max(0, w + e - W)` and the overlap `max(0, W - w - e)`, integrated along the
//!   pair into areas (m², ha).
//!
//! Positions come from the log; the method measures what the log says the machine did, so a
//! receiver that reports a wrong position consistently shows no drift here.

use serde::Serialize;

use super::geo::LocalFrame;
use super::lines::GuidanceLine;
use super::log::{Fix, MachineLog};

/// Parameters of the drift analysis.
#[derive(Clone, Copy, Debug)]
pub struct DriftParams {
    /// Swath (pass) spacing `w` (m).
    pub width_m: f64,
    /// Implement working width `W` (m); defaults to `w`.
    pub implement_m: f64,
    /// Operator tolerance for spacing error (m). Demonstration default 0.076 m (3 in).
    pub tolerance_m: f64,
    /// Pairs further apart than this (minutes) are repeatability, not pass-to-pass.
    pub max_gap_min: f64,
    /// Shortest pass (m).
    pub min_pass_m: f64,
    /// Lowest fix quality used.
    pub min_fix: Fix,
    /// Use only epochs with steering engaged, when the log records it.
    pub engaged_only: bool,
}

impl DriftParams {
    pub fn new(width_m: f64) -> Self {
        DriftParams {
            width_m,
            implement_m: width_m,
            tolerance_m: 0.076,
            max_gap_min: 15.0,
            min_pass_m: 30.0,
            min_fix: Fix::Autonomous,
            engaged_only: true,
        }
    }
}

/// One pass.
#[derive(Clone, Debug, Serialize)]
pub struct Pass {
    pub index: i64,
    pub direction: i8,
    pub t_start_s: f64,
    pub t_end_s: f64,
    pub length_m: f64,
    pub epochs: usize,
    pub start_dev_m: f64,
    pub end_dev_m: f64,
    pub drift_m_per_min: f64,
    pub max_abs_dev_m: f64,
    pub rms_dev_m: f64,
    #[serde(skip)]
    track: Vec<(f64, f64)>, // (s, dev) sorted by s
}

/// One adjacent pair.
#[derive(Clone, Debug, Serialize)]
pub struct PairStat {
    pub passes: [i64; 2],
    pub gap_min: f64,
    /// `pass_to_pass` or `repeatability`.
    pub class: String,
    pub overlap_len_m: f64,
    pub mean_error_m: f64,
    pub rms_error_m: f64,
    pub p95_abs_error_m: f64,
    pub max_abs_error_m: f64,
    pub miss_m2: f64,
    pub overlap_m2: f64,
    pub over_tolerance_m: f64,
}

/// The drift report.
#[derive(Clone, Debug, Serialize)]
pub struct DriftReport {
    pub label: &'static str,
    pub line: String,
    pub width_m: f64,
    pub implement_m: f64,
    pub tolerance_m: f64,
    pub epochs_used: usize,
    pub passes: Vec<Pass>,
    pub pairs: Vec<PairStat>,
    pub pass_to_pass_rms_m: Option<f64>,
    pub pass_to_pass_p95_m: Option<f64>,
    pub repeatability_rms_m: Option<f64>,
    pub worst_within_pass_drift_m_per_min: Option<f64>,
    pub miss_ha: f64,
    pub overlap_ha: f64,
    pub notes: Vec<String>,
}

fn interp(track: &[(f64, f64)], s: f64) -> Option<f64> {
    let i = track.partition_point(|p| p.0 < s);
    if i == 0 || i >= track.len() {
        return track.get(i).filter(|p| (p.0 - s).abs() < 1e-9).map(|p| p.1);
    }
    let (a, b) = (track[i - 1], track[i]);
    if b.0 - a.0 > 10.0 {
        return None; // a hole in the pass
    }
    Some(a.1 + (b.1 - a.1) * (s - a.0) / (b.0 - a.0))
}

/// Analyse `log` against the A-B `line`.
pub fn analyse(
    log: &MachineLog,
    line: &GuidanceLine,
    p: &DriftParams,
) -> Result<DriftReport, String> {
    if p.width_m <= 0.0 || p.implement_m <= 0.0 {
        return Err("width and implement width must be positive".into());
    }
    let a = line.points[0];
    let b = line.points[line.points.len() - 1];
    let f = LocalFrame::new(a[0], a[1], 0.0);
    let bb = f.en(b[0], b[1]);
    let l = (bb[0] * bb[0] + bb[1] * bb[1]).sqrt();
    if l < 1.0 {
        return Err("A and B are less than 1 m apart".into());
    }
    let u = [bb[0] / l, bb[1] / l];
    let n = [-u[1], u[0]];
    let engaged = p.engaged_only && log.has_steer();
    let mut pts: Vec<(f64, f64, f64)> = Vec::new(); // (t, s, x)
    for e in &log.epochs {
        if e.fix < p.min_fix || !e.fix.has_position() || (engaged && e.steer != Some(true)) {
            continue;
        }
        let q = f.en(e.lat, e.lon);
        pts.push((e.t_s, q[0] * u[0] + q[1] * u[1], q[0] * n[0] + q[1] * n[1]));
    }
    // Segment into passes.
    let mut passes: Vec<Pass> = Vec::new();
    let mut cur: Vec<(f64, f64, f64)> = Vec::new();
    let mut cur_k = i64::MIN;
    let mut cur_dir = 0i8;
    let flush = |cur: &mut Vec<(f64, f64, f64)>, k: i64, dir: i8, passes: &mut Vec<Pass>| {
        if cur.len() >= 3 && dir != 0 {
            let s0 = cur.iter().map(|c| c.1).fold(f64::INFINITY, f64::min);
            let s1 = cur.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
            if s1 - s0 >= p.min_pass_m {
                let devs: Vec<(f64, f64)> = cur
                    .iter()
                    .map(|c| (c.0, c.2 - k as f64 * p.width_m))
                    .collect();
                let m = devs.len() as f64;
                let (mt, md) = (
                    devs.iter().map(|d| d.0).sum::<f64>() / m,
                    devs.iter().map(|d| d.1).sum::<f64>() / m,
                );
                let sxx: f64 = devs.iter().map(|d| (d.0 - mt).powi(2)).sum();
                let sxy: f64 = devs.iter().map(|d| (d.0 - mt) * (d.1 - md)).sum();
                let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
                let (t0, t1) = (devs[0].0, devs[devs.len() - 1].0);
                let dv: Vec<f64> = devs.iter().map(|d| d.1).collect();
                let mut track: Vec<(f64, f64)> = cur
                    .iter()
                    .map(|c| (c.1, c.2 - k as f64 * p.width_m))
                    .collect();
                track.sort_by(|x, y| x.0.total_cmp(&y.0));
                track.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
                passes.push(Pass {
                    index: k,
                    direction: dir,
                    t_start_s: t0,
                    t_end_s: t1,
                    length_m: super::round(s1 - s0, 2),
                    epochs: cur.len(),
                    start_dev_m: super::round(md + slope * (t0 - mt), 4),
                    end_dev_m: super::round(md + slope * (t1 - mt), 4),
                    drift_m_per_min: super::round(slope * 60.0, 5),
                    max_abs_dev_m: super::round(dv.iter().fold(0.0f64, |a, b| a.max(b.abs())), 4),
                    rms_dev_m: super::round(super::rms(&dv), 4),
                    track,
                });
            }
        }
        cur.clear();
    };
    for w in 0..pts.len() {
        let (t, s, x) = pts[w];
        let k = (x / p.width_m).round() as i64;
        let dir = if w > 0 {
            let ds = s - pts[w - 1].1;
            if ds.abs() < 0.05 {
                cur_dir
            } else {
                ds.signum() as i8
            }
        } else {
            0
        };
        let gap = w > 0 && t - pts[w - 1].0 > 30.0;
        if k != cur_k || (cur_dir != 0 && dir != 0 && dir != cur_dir) || gap {
            flush(&mut cur, cur_k, cur_dir, &mut passes);
            cur_k = k;
            cur_dir = 0;
        }
        if cur_dir == 0 && !cur.is_empty() {
            cur_dir = dir;
        }
        cur.push((t, s, x));
    }
    flush(&mut cur, cur_k, cur_dir, &mut passes);
    // Adjacent pairs: for each k, the pass on k+1 nearest in time.
    let mut pairs = Vec::new();
    let (mut miss, mut over) = (0.0, 0.0);
    for pa in &passes {
        let Some(pb) = passes
            .iter()
            .filter(|q| q.index == pa.index + 1)
            .min_by(|x, y| {
                (x.t_start_s - pa.t_start_s)
                    .abs()
                    .total_cmp(&(y.t_start_s - pa.t_start_s).abs())
            })
        else {
            continue;
        };
        let lo = pa.track[0].0.max(pb.track[0].0);
        let hi = pa.track[pa.track.len() - 1]
            .0
            .min(pb.track[pb.track.len() - 1].0);
        if hi - lo < 1.0 {
            continue;
        }
        let mut errs = Vec::new();
        let (mut m2, mut o2, mut over_tol) = (0.0, 0.0, 0.0);
        let mut s = lo;
        while s <= hi {
            if let (Some(da), Some(db)) = (interp(&pa.track, s), interp(&pb.track, s)) {
                let e = db - da;
                errs.push(e);
                let spacing = p.width_m + e;
                m2 += (spacing - p.implement_m).max(0.0);
                o2 += (p.implement_m - spacing).max(0.0);
                if e.abs() > p.tolerance_m {
                    over_tol += 1.0;
                }
            }
            s += 1.0;
        }
        if errs.is_empty() {
            continue;
        }
        let ta = (pa.t_start_s + pa.t_end_s) / 2.0;
        let tb = (pb.t_start_s + pb.t_end_s) / 2.0;
        let gap_min = (tb - ta).abs() / 60.0;
        miss += m2;
        over += o2;
        let abs: Vec<f64> = errs.iter().map(|e| e.abs()).collect();
        pairs.push(PairStat {
            passes: [pa.index, pb.index],
            gap_min: super::round(gap_min, 2),
            class: if gap_min <= p.max_gap_min {
                "pass_to_pass".into()
            } else {
                "repeatability".into()
            },
            overlap_len_m: super::round(hi - lo, 1),
            mean_error_m: super::round(errs.iter().sum::<f64>() / errs.len() as f64, 4),
            rms_error_m: super::round(super::rms(&errs), 4),
            p95_abs_error_m: super::round(super::percentile(&abs, 95.0).unwrap_or(0.0), 4),
            max_abs_error_m: super::round(abs.iter().fold(0.0f64, |a, b| a.max(*b)), 4),
            miss_m2: super::round(m2, 2),
            overlap_m2: super::round(o2, 2),
            over_tolerance_m: over_tol,
        });
    }
    let collect = |class: &str| -> Vec<f64> {
        pairs
            .iter()
            .filter(|q| q.class == class)
            .map(|q| q.rms_error_m)
            .collect()
    };
    let ptp = collect("pass_to_pass");
    let rep = collect("repeatability");
    let ptp_p95: Vec<f64> = pairs
        .iter()
        .filter(|q| q.class == "pass_to_pass")
        .map(|q| q.p95_abs_error_m)
        .collect();
    let mut notes = Vec::new();
    if passes.is_empty() {
        notes.push("No pass found: check the line, the width and the fix filter.".into());
    }
    if engaged {
        notes.push("Only epochs with steering engaged were used.".into());
    }
    if !log.has_steer() {
        notes.push("The log does not record steering engagement; every epoch was used.".into());
    }
    Ok(DriftReport {
        label: super::LABEL,
        line: line.name.clone(),
        width_m: p.width_m,
        implement_m: p.implement_m,
        tolerance_m: p.tolerance_m,
        epochs_used: pts.len(),
        pass_to_pass_rms_m: (!ptp.is_empty()).then(|| super::round(super::rms(&ptp), 4)),
        pass_to_pass_p95_m: ptp_p95
            .iter()
            .cloned()
            .reduce(f64::max)
            .map(|v| super::round(v, 4)),
        repeatability_rms_m: (!rep.is_empty()).then(|| super::round(super::rms(&rep), 4)),
        worst_within_pass_drift_m_per_min: passes
            .iter()
            .map(|q| q.drift_m_per_min.abs())
            .reduce(f64::max),
        passes,
        pairs,
        miss_ha: super::round(miss / 10_000.0, 4),
        overlap_ha: super::round(over / 10_000.0, 4),
        notes,
    })
}

/// A Markdown summary of the report.
pub fn markdown(r: &DriftReport) -> String {
    let o = |v: Option<f64>| v.map(|x| format!("{x:.3}")).unwrap_or_else(|| "-".into());
    let mut m = format!(
        "# Pass-to-pass drift ({})\n\nLine `{}`, swath {} m, implement {} m, tolerance {} m, {} epochs used.\n\n\
         - Pass-to-pass RMS: {} m; worst pair 95th percentile: {} m\n- Repeatability RMS (pairs over the time gap): {} m\n\
         - Worst within-pass drift: {} m/min\n- Miss area: {:.4} ha; overlap area: {:.4} ha\n\n\
         | Pass | Dir | Length m | Start dev m | End dev m | Drift m/min | Max m |\n|---|---|---|---|---|---|---|\n",
        r.label,
        r.line,
        r.width_m,
        r.implement_m,
        r.tolerance_m,
        r.epochs_used,
        o(r.pass_to_pass_rms_m),
        o(r.pass_to_pass_p95_m),
        o(r.repeatability_rms_m),
        o(r.worst_within_pass_drift_m_per_min),
        r.miss_ha,
        r.overlap_ha
    );
    for q in &r.passes {
        m.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            q.index,
            q.direction,
            q.length_m,
            q.start_dev_m,
            q.end_dev_m,
            q.drift_m_per_min,
            q.max_abs_dev_m
        ));
    }
    m.push_str("\n| Pair | Gap min | Class | RMS m | p95 m | Miss m² | Overlap m² |\n|---|---|---|---|---|---|---|\n");
    for q in &r.pairs {
        m.push_str(&format!(
            "| {}-{} | {} | {} | {} | {} | {} | {} |\n",
            q.passes[0],
            q.passes[1],
            q.gap_min,
            q.class,
            q.rms_error_m,
            q.p95_abs_error_m,
            q.miss_m2,
            q.overlap_m2
        ));
    }
    for n in &r.notes {
        m.push_str(&format!("\n- {n}"));
    }
    m.push('\n');
    m
}
