// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! B2401: lunar navigation service verification beyond the monthly SLA of D7.
//!
//! ## URA overbounding (`ura`)
//! An LNSP broadcasts, with each ephemeris and clock, a user range accuracy (URA, or SISA in
//! the Galileo-derived wording) that receivers use as the 1σ of the signal-in-space error
//! (SISE). The LunaNet Interoperability Specification and LSIS V1.0 carry a URA-like field; the
//! exact bit allocation and the overbounding requirement that goes with it are **to confirm**
//! against the published documents (the research note's source [10] says the allocation is
//! still being fixed). This check therefore uses the generic paired-Gaussian overbounding test
//! of GNSS integrity practice (DeCleene, ION GPS 2000): the URA overbounds the SISE at
//! threshold `k` when the empirical two-sided tail `P(|SISE/URA| > k)` does not exceed the
//! Gaussian tail `2Q(k) = erfc(k/√2)`.
//!
//! Input `time_s,node,sise_m,ura_m`: the SISE is the broadcast product minus a precise
//! reference product, projected on the range (formed by the verifier, from its own reference).
//! Samples are correlated in time; the count used for the intervals is the number of distinct
//! `decorrelation_s` blocks per node (`n_eff`), and each tail fraction carries a Wilson 95 %
//! interval on that count. Verdict per threshold: **OVERBOUNDED** (upper bound under the
//! Gaussian tail), **NOT OVERBOUNDED** (lower bound over it) or **INCONCLUSIVE**, with the
//! number of independent samples a zero-exceedance demonstration would need. The inflation
//! factor is the smallest scale on the URA for which every threshold the data can resolve is
//! met on the point estimate.
//!
//! ## Cross-satellite time consistency (`time-consistency`)
//! Input `time_s,node,clock_ns`: each node's broadcast clock offset minus a monitor's estimate
//! of it (or any per-node clock residual with a common reference). At every epoch with three or
//! more nodes the median of the nodes not in an event is removed (so a common-mode error of
//! the monitor cancels and a faulty node does not bias the others),
//! the deviations are normalised by the pooled robust sigma (MAD), and a two-sided CUSUM per
//! node (`k`, `h`) raises an alarm; a plain threshold `threshold_ns` raises one too. Each alarm
//! is an event with an onset (the last time the CUSUM was zero), the alarm time, and a class
//! (**step** or **ramp**, from a line fitted to the hour after the onset).
//!
//! MODELLED: checked on synthetic constellations with injected steps, ramps and understated
//! URA; no lunar service exists yet.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{csv_rows, median, num, rnd, two_sided_tail, upper_verdict, wilson95, Loaded};

fn d_decor() -> f64 {
    900.0
}
fn d_thresholds() -> Vec<f64> {
    vec![1.0, 2.0, 3.0, 4.0]
}

/// `[ura]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UraReq {
    pub data: String,
    #[serde(default = "d_decor")]
    pub decorrelation_s: f64,
    #[serde(default = "d_thresholds")]
    pub thresholds: Vec<f64>,
    /// Where the URA values and the reference product come from.
    #[serde(default)]
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Threshold {
    pub k: f64,
    pub gaussian_tail: f64,
    pub exceedances: usize,
    pub fraction: f64,
    pub wilson95: Option<(f64, f64)>,
    pub verdict: String,
    /// Independent samples a zero-exceedance demonstration at this threshold needs (Wilson).
    pub n_needed_zero_exceedance: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct UraGroup {
    pub node: String,
    pub n: usize,
    pub n_eff: usize,
    pub mean_z: f64,
    pub std_z: f64,
    pub max_abs_z: f64,
    pub thresholds: Vec<Threshold>,
    /// Smallest scale on the URA that makes the point-estimate tails overbounded.
    pub inflation_needed: f64,
    pub verdict: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UraReport {
    pub schema: String,
    pub source: String,
    pub method: String,
    pub groups: Vec<UraGroup>,
    pub evidence: String,
}

fn ura_group(node: &str, z: &[f64], n_eff: usize, ks: &[f64]) -> UraGroup {
    let n = z.len();
    let (mean_z, std_z) = super::mean_std(z);
    let tail = |zs: &[f64], k: f64| zs.iter().filter(|v| v.abs() > k).count();
    let thresholds: Vec<Threshold> = ks
        .iter()
        .map(|&k| {
            let g = two_sided_tail(k);
            let e = tail(z, k);
            let e_eff = ((e as f64) * n_eff as f64 / n as f64).round() as usize;
            let w = wilson95(e_eff, n_eff);
            let verdict = match w {
                Some((lo, hi)) => upper_verdict(lo, hi, g, "OVERBOUNDED", "NOT OVERBOUNDED"),
                None => "INCONCLUSIVE".into(),
            };
            Threshold {
                k,
                gaussian_tail: g,
                exceedances: e,
                fraction: e as f64 / n as f64,
                wilson95: w,
                verdict,
                // Wilson upper bound at zero exceedances is z²/(n + z²) with z = 1.96.
                n_needed_zero_exceedance: (3.8416 * (1.0 - g) / g).ceil() as u64,
            }
        })
        .collect();
    // Inflation: smallest s with tail(|z|/s > k) <= 2Q(k) for every k the sample resolves.
    let grid: Vec<f64> = (1..=60).map(|i| i as f64 * 0.1).collect();
    let ok = |s: f64| {
        grid.iter()
            .filter(|&&k| two_sided_tail(k) * n as f64 >= 1.0)
            .all(|&k| (tail(z, k * s) as f64) <= two_sided_tail(k) * n as f64)
    };
    let (mut lo, mut hi) = (0.01, 1.0);
    while !ok(hi) && hi < 1e6 {
        hi *= 2.0;
    }
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if ok(mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let verdict = if thresholds.iter().any(|t| t.verdict == "NOT OVERBOUNDED") {
        "NOT OVERBOUNDED"
    } else if thresholds.iter().all(|t| t.verdict == "OVERBOUNDED") {
        "OVERBOUNDED"
    } else {
        "OVERBOUNDED WHERE RESOLVED; INCONCLUSIVE ELSEWHERE"
    };
    UraGroup {
        node: node.into(),
        n,
        n_eff,
        mean_z: rnd(mean_z, 4),
        std_z: rnd(std_z, 4),
        max_abs_z: rnd(z.iter().fold(0.0f64, |a, v| a.max(v.abs())), 3),
        thresholds,
        inflation_needed: rnd(hi.max(1.0), 3),
        verdict: verdict.into(),
    }
}

/// URA overbounding validation.
pub fn ura(l: &mut Loaded) -> Result<UraReport, String> {
    let r = l
        .req
        .ura
        .clone()
        .ok_or("the request has no [ura] section")?;
    if r.decorrelation_s <= 0.0 || r.thresholds.iter().any(|k| *k <= 0.0) {
        return Err("decorrelation_s and thresholds must be positive".into());
    }
    let text = l.read(&r.data, "sise-ura")?;
    let rows = csv_rows(&text, &["time_s", "node", "sise_m", "ura_m"])?;
    let mut by: BTreeMap<String, (Vec<f64>, BTreeSet<i64>)> = BTreeMap::new();
    for f in &rows {
        let t = num(&f[0], "time_s")?;
        let (e, u) = (num(&f[2], "sise_m")?, num(&f[3], "ura_m")?);
        if u <= 0.0 {
            return Err(format!("node {}: URA must be positive", f[1]));
        }
        let g = by.entry(f[1].clone()).or_default();
        g.0.push(e / u);
        g.1.insert((t / r.decorrelation_s).floor() as i64);
    }
    if by.is_empty() {
        return Err("no samples".into());
    }
    let mut groups: Vec<UraGroup> = by
        .iter()
        .map(|(n, (z, b))| ura_group(n, z, b.len(), &r.thresholds))
        .collect();
    let all: Vec<f64> = by.values().flat_map(|(z, _)| z.iter().copied()).collect();
    let n_eff: usize = by.values().map(|(_, b)| b.len()).sum();
    groups.push(ura_group("ALL", &all, n_eff, &r.thresholds));
    Ok(UraReport {
        schema: "kshana-pro/lunar-ura-overbound/v1".into(),
        source: r.source,
        method: "paired-Gaussian two-sided tail test of SISE/URA at each threshold (DeCleene 2000); \
                 Wilson 95 % on decorrelation blocks; LNSP URA definition to confirm against LNIS/LSIS"
            .into(),
        groups,
        evidence: super::EVIDENCE.into(),
    })
}

fn d_tc_threshold() -> f64 {
    15.0
}
fn d_k() -> f64 {
    0.5
}
fn d_h() -> f64 {
    10.0
}
fn d_window() -> f64 {
    3600.0
}

/// `[time_consistency]` section.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeReq {
    pub data: String,
    #[serde(default = "d_tc_threshold")]
    pub threshold_ns: f64,
    #[serde(default = "d_k")]
    pub cusum_k: f64,
    #[serde(default = "d_h")]
    pub cusum_h: f64,
    #[serde(default = "d_window")]
    pub class_window_s: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimeEvent {
    pub node: String,
    pub onset_s: f64,
    pub alarm_s: f64,
    /// `step` or `ramp`.
    pub class: String,
    /// Mean deviation from the ensemble over the classification window (ns).
    pub level_ns: f64,
    /// Fitted slope over that window (ns/s).
    pub slope_ns_per_s: f64,
    /// `cusum`, `threshold` or both.
    pub trigger: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct NodeConsistency {
    pub node: String,
    pub samples: usize,
    pub rms_dev_ns: f64,
    pub max_abs_dev_ns: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimeReport {
    pub schema: String,
    pub epochs: usize,
    pub pooled_sigma_ns: f64,
    /// 95th percentile of the across-node spread (max minus min) per epoch (ns).
    pub spread_p95_ns: f64,
    pub nodes: Vec<NodeConsistency>,
    pub events: Vec<TimeEvent>,
    pub verdict: String,
    pub evidence: String,
}

/// Least-squares line `y = a + b t`; returns `(mean y, b)`.
pub(crate) fn line_fit(t: &[f64], y: &[f64]) -> (f64, f64) {
    let n = t.len() as f64;
    let (mt, my) = (t.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let sxx: f64 = t.iter().map(|x| (x - mt) * (x - mt)).sum();
    let sxy: f64 = t.iter().zip(y).map(|(x, v)| (x - mt) * (v - my)).sum();
    (my, if sxx > 0.0 { sxy / sxx } else { 0.0 })
}

/// Cross-satellite time-consistency monitor.
pub fn time_consistency(l: &mut Loaded) -> Result<TimeReport, String> {
    let r = l
        .req
        .time_consistency
        .clone()
        .ok_or("the request has no [time_consistency] section")?;
    let text = l.read(&r.data, "clock-residuals")?;
    let rows = csv_rows(&text, &["time_s", "node", "clock_ns"])?;
    // epoch (ms key) -> node -> value
    let mut ep: BTreeMap<i64, BTreeMap<String, f64>> = BTreeMap::new();
    for f in &rows {
        let t = num(&f[0], "time_s")?;
        ep.entry((t * 1000.0).round() as i64)
            .or_default()
            .insert(f[1].clone(), num(&f[2], "clock_ns")?);
    }
    // Pass 1: the pooled robust sigma of the deviations from the plain ensemble median.
    let mut spreads = Vec::new();
    let mut abs_dev = Vec::new();
    for m in ep.values().filter(|m| m.len() >= 3) {
        let v: Vec<f64> = m.values().copied().collect();
        let med = median(&v);
        let (mn, mx) = v
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        spreads.push(mx - mn);
        abs_dev.extend(v.iter().map(|x| (x - med).abs()));
    }
    if spreads.is_empty() {
        return Err("no epoch has three or more nodes".into());
    }
    let sigma = (1.4826 * median(&abs_dev)).max(1e-9);
    // Pass 2, in time order: the ensemble is the median of the nodes not in an event (all
    // nodes when fewer than three are clean), so a faulty node does not bias the others.
    #[derive(Default)]
    struct St {
        sp: f64,
        sn: f64,
        zero_p: f64,
        zero_n: f64,
        started: bool,
        in_event: bool,
        quiet_since: Option<f64>,
        dev: Vec<(f64, f64)>,
    }
    let mut st: BTreeMap<String, St> = BTreeMap::new();
    let mut raw: Vec<(String, f64, f64, &'static str)> = Vec::new();
    for (tk, m) in ep.iter().filter(|(_, m)| m.len() >= 3) {
        let t = *tk as f64 / 1000.0;
        let clean: Vec<f64> = m
            .iter()
            .filter(|(n, _)| !st.get(*n).is_some_and(|s| s.in_event))
            .map(|(_, x)| *x)
            .collect();
        let med = if clean.len() >= 3 {
            median(&clean)
        } else {
            median(&m.values().copied().collect::<Vec<_>>())
        };
        for (n, x) in m {
            let s = st.entry(n.clone()).or_default();
            let d = x - med;
            s.dev.push((t, d));
            if !s.started {
                s.started = true;
                s.zero_p = t;
                s.zero_n = t;
            }
            // After an event the node stays in it until it has been quiet (within 3 sigma)
            // for a whole classification window: one event per excursion.
            if s.in_event {
                if d.abs() > 3.0 * sigma {
                    s.quiet_since = None;
                    continue;
                }
                let q0 = *s.quiet_since.get_or_insert(t);
                if t - q0 < r.class_window_s {
                    continue;
                }
                s.in_event = false;
                (s.sp, s.sn, s.zero_p, s.zero_n) = (0.0, 0.0, t, t);
            }
            let z = d / sigma;
            s.sp = (s.sp + z - r.cusum_k).max(0.0);
            s.sn = (s.sn - z - r.cusum_k).max(0.0);
            if s.sp == 0.0 {
                s.zero_p = t;
            }
            if s.sn == 0.0 {
                s.zero_n = t;
            }
            let cus = s.sp > r.cusum_h || s.sn > r.cusum_h;
            let thr = d.abs() > r.threshold_ns;
            if cus || thr {
                let onset = match (cus, s.sp > r.cusum_h) {
                    (true, true) => s.zero_p,
                    (true, false) => s.zero_n,
                    _ => t,
                };
                let trig = match (cus, thr) {
                    (true, true) => "cusum+threshold",
                    (true, false) => "cusum",
                    _ => "threshold",
                };
                raw.push((n.clone(), onset, t, trig));
                s.in_event = true;
                s.quiet_since = None;
            }
        }
    }
    let mut events = Vec::new();
    for (node, onset, alarm, trig) in raw {
        let (tt, yy): (Vec<f64>, Vec<f64>) = st[&node]
            .dev
            .iter()
            .filter(|p| p.0 >= onset && p.0 <= onset + r.class_window_s)
            .copied()
            .unzip();
        let (level, slope) = line_fit(&tt, &yy);
        let span = tt.last().unwrap_or(&onset) - tt.first().unwrap_or(&onset);
        events.push(TimeEvent {
            node,
            onset_s: onset,
            alarm_s: alarm,
            class: if (slope * span).abs() > 0.5 * level.abs() {
                "ramp"
            } else {
                "step"
            }
            .into(),
            level_ns: rnd(level, 3),
            slope_ns_per_s: rnd(slope, 6),
            trigger: trig.into(),
        });
    }
    let nodes: Vec<NodeConsistency> = st
        .iter()
        .map(|(node, s)| NodeConsistency {
            node: node.clone(),
            samples: s.dev.len(),
            rms_dev_ns: rnd(
                (s.dev.iter().map(|p| p.1 * p.1).sum::<f64>() / s.dev.len() as f64).sqrt(),
                3,
            ),
            max_abs_dev_ns: rnd(s.dev.iter().fold(0.0f64, |a, p| a.max(p.1.abs())), 3),
        })
        .collect();
    let mut sp = spreads.clone();
    sp.sort_by(|a, b| a.total_cmp(b));
    let p95 = sp[((sp.len() as f64 * 0.95).ceil() as usize).clamp(1, sp.len()) - 1];
    let verdict = if events.is_empty() {
        "CONSISTENT: no node departed from the ensemble".to_string()
    } else {
        let ns: BTreeSet<&str> = events.iter().map(|e| e.node.as_str()).collect();
        format!(
            "INCONSISTENT: {} event(s) on {}",
            events.len(),
            ns.into_iter().collect::<Vec<_>>().join(", ")
        )
    };
    Ok(TimeReport {
        schema: "kshana-pro/lunar-time-consistency/v1".into(),
        epochs: spreads.len(),
        pooled_sigma_ns: rnd(sigma, 4),
        spread_p95_ns: rnd(p95, 3),
        nodes,
        events,
        verdict,
        evidence: super::EVIDENCE.into(),
    })
}
