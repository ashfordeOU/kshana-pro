// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Explainable position trust, reconstruction of dark periods, gap cause and anchor drag
//!
//! * [`trust_scores`] (B0703): a position-trust score per vessel and period, 0-100, from five
//!   named components with **stated weights** ([`WEIGHTS`], not fitted): reception continuity,
//!   kinematic plausibility, agreement of broadcast SOG with the implied speed, identity
//!   stability and position availability. Each component lists the observations (UTC times)
//!   that lowered it.
//! * [`port_calls`] (B0703): for each reception gap and each declared port, whether a call of
//!   at least the stated minimum stay fits in the gap at the stated service speed, with the
//!   slack: a concealed port-call **candidate**, not a finding that a call happened.
//! * [`dark_sts`] (B0703): for two vessels whose gaps overlap by at least the encounter
//!   duration, whether a common meeting point exists that both could reach, stay at for the
//!   overlap, and leave from to their next reported positions (a grid search over the
//!   region of the four gap ends).
//! * [`classify_gap`] (B0704): what the data say about the cause of a gap: reports received
//!   without a position fix (the transponder was on, its GNSS fix was lost: consistent with
//!   interference), most neighbours losing reports or fixes over the same time (area-wide:
//!   consistent with interference or a coverage loss), the vessel's VMS reporting with a
//!   plausible track through the gap (the AIS alone went silent: consistent with AIS switched
//!   off), or undetermined.
//! * [`anchor_drag`] (B0705): a running anchor-drag likelihood from speed and heading
//!   signatures over a sliding window (slow-band share, heading-to-course divergence,
//!   straightness, anchor status while moving) through a logistic link with **stated
//!   coefficients** ([`DRAG_COEFFS`], not fitted), and the alert time against the first entry
//!   into a declared asset corridor.
//!
//! **MODELLED.** Heuristics with stated weights, exercised on synthetic fixtures with known
//! truth; not trained or validated on labelled incidents.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::packs::subsea::Asset;
use super::{gaps, interp, jumps, r, Area, Gap, Params, Report, Tracks, KN};
use crate::forensics::inputs::iso;
use crate::transport::dist_m as dist;

const MAX_OBS: usize = 20;

/// Weights of the trust components (sum 1); stated, not fitted.
pub const WEIGHTS: [(&str, f64); 5] = [
    ("continuity", 0.30),
    ("kinematics", 0.25),
    ("sog-agreement", 0.15),
    ("identity", 0.15),
    ("fix-availability", 0.15),
];

/// One trust component.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Component {
    pub name: &'static str,
    pub weight: f64,
    /// 0..1; `None` when the period gives nothing to judge it on.
    pub value: Option<f64>,
    pub rule: &'static str,
    /// UTC times of the observations that lowered it (first 20).
    pub observations: Vec<String>,
}

/// Trust of one vessel over one period.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Trust {
    pub mmsi: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub reports: usize,
    /// 0..100, weighted mean of the available components.
    pub score: Option<f64>,
    pub components: Vec<Component>,
}

fn cap(v: Vec<String>) -> Vec<String> {
    v.into_iter().take(MAX_OBS).collect()
}

/// Trust scores of `mmsi` over consecutive periods of `period_s` from its first report.
pub fn trust_scores(t: &Tracks, mmsi: &str, period_s: f64, p: &Params) -> Vec<Trust> {
    let Some(v) = t.by_mmsi.get(mmsi) else {
        return Vec::new();
    };
    let (Some(f), Some(l)) = (v.first(), v.last()) else {
        return Vec::new();
    };
    let ids: Vec<(f64, String)> = t
        .statics
        .get(mmsi)
        .map(|s| {
            super::identity_changes(s)
                .into_iter()
                .filter_map(|c| crate::forensics::inputs::parse_utc(&c.utc).map(|x| (x, c.utc)))
                .collect()
        })
        .unwrap_or_default();
    let unavail = t.unavailable.get(mmsi).cloned().unwrap_or_default();
    let all_jumps = jumps(v, p);
    let coll = super::collision(v, p);
    let mut out = Vec::new();
    let mut a = f.t;
    while a <= l.t {
        let b = a + period_s;
        let inp: Vec<&Report> = v.iter().filter(|x| x.t >= a && x.t < b).collect();
        // Continuity: time between consecutive reports (both in or straddling the period)
        // no longer than the gap threshold, over the span of the track inside the period.
        let span = (b.min(l.t) - a.max(f.t)).max(0.0);
        let mut covered = 0.0;
        let mut gap_obs = Vec::new();
        for w in v.windows(2) {
            let (s, e) = (w[0].t.max(a), w[1].t.min(b));
            if e <= s {
                continue;
            }
            if w[1].t - w[0].t <= p.gap_s {
                covered += e - s;
            } else {
                gap_obs.push(format!("gap {} to {}", iso(w[0].t), iso(w[1].t)));
            }
        }
        let continuity = (span > 0.0).then(|| (covered / span).min(1.0));
        let pj: Vec<String> = all_jumps
            .iter()
            .filter(|j| {
                crate::forensics::inputs::parse_utc(&j.utc).is_some_and(|x| x >= a && x < b)
            })
            .map(|j| format!("jump at {} ({:.0} kn)", j.utc, j.implied_speed_kn))
            .collect();
        let kin = (inp.len() >= 2).then(|| 0.5f64.powi(pj.len() as i32));
        // SOG agreement on short intervals with SOG on both ends.
        let (mut n_sog, mut bad) = (0usize, Vec::new());
        for w in v.windows(2) {
            if w[0].t < a || w[1].t >= b || w[1].t - w[0].t > 600.0 || w[1].t <= w[0].t {
                continue;
            }
            let (Some(s0), Some(s1)) = (w[0].sog_kn, w[1].sog_kn) else {
                continue;
            };
            n_sog += 1;
            let imp = dist(w[0].lat, w[0].lon, w[1].lat, w[1].lon) / (w[1].t - w[0].t) / KN;
            let m = 0.5 * (s0 + s1);
            if (imp - m).abs() > (0.5 * m).max(3.0) {
                bad.push(format!(
                    "{}: implied {imp:.1} kn, broadcast {m:.1} kn",
                    iso(w[1].t)
                ));
            }
        }
        let sog = (n_sog > 0).then(|| 1.0 - bad.len() as f64 / n_sog as f64);
        let mut id_obs: Vec<String> = ids
            .iter()
            .filter(|(x, _)| *x >= a && *x < b)
            .map(|(_, u)| format!("identity change at {u}"))
            .collect();
        let mut id_pen = id_obs.len() as i32;
        if let Some(c) = &coll {
            if crate::forensics::inputs::parse_utc(&c.first_utc).is_some_and(|x| x >= a && x < b) {
                id_pen += 2;
                id_obs.push(format!("one MMSI on two transmitters from {}", c.first_utc));
            }
        }
        let identity = (!inp.is_empty()).then(|| 0.5f64.powi(id_pen));
        let un: Vec<f64> = unavail
            .iter()
            .copied()
            .filter(|x| *x >= a && *x < b)
            .collect();
        let fix =
            (inp.len() + un.len() > 0).then(|| inp.len() as f64 / (inp.len() + un.len()) as f64);
        let comps = vec![
            Component {
                name: WEIGHTS[0].0,
                weight: WEIGHTS[0].1,
                value: continuity,
                rule: "share of the track's span in the period covered by report intervals no longer than params.gap_s",
                observations: cap(gap_obs),
            },
            Component {
                name: WEIGHTS[1].0,
                weight: WEIGHTS[1].1,
                value: kin,
                rule: "halved for each implied-speed jump above params.max_speed_kn",
                observations: cap(pj),
            },
            Component {
                name: WEIGHTS[2].0,
                weight: WEIGHTS[2].1,
                value: sog,
                rule: "share of intervals up to 600 s whose implied speed is within max(3 kn, 50 %) of the broadcast SOG",
                observations: cap(bad),
            },
            Component {
                name: WEIGHTS[3].0,
                weight: WEIGHTS[3].1,
                value: identity,
                rule: "halved for each static-data identity change; quartered when one MMSI is on two transmitters",
                observations: cap(id_obs),
            },
            Component {
                name: WEIGHTS[4].0,
                weight: WEIGHTS[4].1,
                value: fix,
                rule: "share of reports received with a position (91/181 'not available' reports count against)",
                observations: cap(un.iter().map(|x| format!("no fix at {}", iso(*x))).collect()),
            },
        ];
        let (sw, sv) = comps.iter().fold((0.0, 0.0), |(sw, sv), c| match c.value {
            Some(x) => (sw + c.weight, sv + c.weight * x),
            None => (sw, sv),
        });
        out.push(Trust {
            mmsi: mmsi.into(),
            period_start_utc: iso(a),
            period_end_utc: iso(b),
            reports: inp.len(),
            score: (sw > 0.0).then(|| r(100.0 * sv / sw, 1)),
            components: comps,
        });
        a = b;
    }
    out
}

/// A concealed port-call candidate.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PortCall {
    pub mmsi: String,
    pub port: String,
    pub gap_start_utc: String,
    pub gap_end_utc: String,
    pub gap_h: f64,
    /// Sailing time to the port and on to the next report at the service speed (h).
    pub sailing_h: f64,
    /// Gap time left after sailing and the minimum stay (h).
    pub slack_h: f64,
}

/// Port calls that fit inside each gap.
pub fn port_calls(g: &[Gap], ports: &[Area], service_kn: f64, min_stay_h: f64) -> Vec<PortCall> {
    let mut out = Vec::new();
    for x in g {
        for pt in ports {
            let d = pt.distance_m(x.start[0], x.start[1]) + pt.distance_m(x.end[0], x.end[1]);
            let sail = d / (service_kn * KN) / 3600.0;
            let slack = x.duration_h - sail - min_stay_h;
            if slack >= 0.0 {
                out.push(PortCall {
                    mmsi: x.mmsi.clone(),
                    port: pt.name.clone(),
                    gap_start_utc: x.start_utc.clone(),
                    gap_end_utc: x.end_utc.clone(),
                    gap_h: r(x.duration_h, 2),
                    sailing_h: r(sail, 2),
                    slack_h: r(slack, 2),
                });
            }
        }
    }
    out
}

/// A dark ship-to-ship candidate: overlapping gaps with a feasible meeting point.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DarkSts {
    pub a: String,
    pub b: String,
    pub overlap_start_utc: String,
    pub overlap_end_utc: String,
    pub overlap_h: f64,
    /// A meeting point both could reach and leave in time, with the smaller slack (h).
    pub point: [f64; 2],
    pub slack_h: f64,
    pub observations: Vec<String>,
}

/// Dark-STS candidates involving the `focus` vessels.
pub fn dark_sts(t: &Tracks, focus: &BTreeSet<String>, p: &Params, service_kn: f64) -> Vec<DarkSts> {
    let all: BTreeMap<&String, Vec<Gap>> = t.by_mmsi.iter().map(|(m, v)| (m, gaps(v, p))).collect();
    let v = service_kn * KN;
    let mut out = Vec::new();
    for (ma, ga) in &all {
        for (mb, gb) in &all {
            if ma >= mb || !(focus.contains(*ma) || focus.contains(*mb)) {
                continue;
            }
            for x in ga {
                for y in gb {
                    let (s, e) = (x.t0.max(y.t0), x.t1.min(y.t1));
                    let d = e - s;
                    if d < p.encounter_min_s {
                        continue;
                    }
                    let pts = [x.start, x.end, y.start, y.end];
                    let (la0, la1) = pts
                        .iter()
                        .fold((90.0f64, -90.0f64), |(a, b), q| (a.min(q[0]), b.max(q[0])));
                    let (lo0, lo1) = pts.iter().fold((180.0f64, -180.0f64), |(a, b), q| {
                        (a.min(q[1]), b.max(q[1]))
                    });
                    let mut best: Option<([f64; 2], f64)> = None;
                    // The gap ends themselves, then a 15 x 15 grid over their region.
                    let cands = pts.iter().map(|q| (q[0], q[1])).chain((0..225).map(|k| {
                        let (i, j) = (k / 15, k % 15);
                        (
                            la0 + (la1 - la0) * i as f64 / 14.0,
                            lo0 + (lo1 - lo0) * j as f64 / 14.0,
                        )
                    }));
                    {
                        for (pl, pn) in cands {
                            let slack = |g: &Gap| {
                                // Reach the point by the overlap start and leave at its end.
                                let to = dist(g.start[0], g.start[1], pl, pn) / v;
                                let from = dist(pl, pn, g.end[0], g.end[1]) / v;
                                ((s - g.t0) - to).min((g.t1 - e) - from)
                            };
                            let sl = slack(x).min(slack(y));
                            if sl >= 0.0 && best.is_none_or(|b| sl > b.1) {
                                best = Some(([pl, pn], sl));
                            }
                        }
                    }
                    if let Some((pt, sl)) = best {
                        out.push(DarkSts {
                            a: (*ma).clone(),
                            b: (*mb).clone(),
                            overlap_start_utc: iso(s),
                            overlap_end_utc: iso(e),
                            overlap_h: r(d / 3600.0, 2),
                            point: [r(pt[0], 4), r(pt[1], 4)],
                            slack_h: r(sl / 3600.0, 2),
                            observations: vec![
                                format!(
                                    "{ma} last report {} at {:.4},{:.4}",
                                    x.start_utc, x.start[0], x.start[1]
                                ),
                                format!(
                                    "{ma} next report {} at {:.4},{:.4}",
                                    x.end_utc, x.end[0], x.end[1]
                                ),
                                format!(
                                    "{mb} last report {} at {:.4},{:.4}",
                                    y.start_utc, y.start[0], y.start[1]
                                ),
                                format!(
                                    "{mb} next report {} at {:.4},{:.4}",
                                    y.end_utc, y.end[0], y.end[1]
                                ),
                            ],
                        });
                    }
                }
            }
        }
    }
    out
}

/// What the data say about a gap's cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GapCause {
    /// Reports kept arriving without a position fix.
    TransponderOnNoFix,
    /// Most neighbours lost reports or fixes over the same time.
    AreaWide,
    /// VMS kept reporting a plausible track: AIS alone went silent.
    AisOnlySilent,
    Undetermined,
}

/// Classification of one gap with its reasons.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GapClass {
    pub mmsi: String,
    pub start_utc: String,
    pub end_utc: String,
    pub cause: GapCause,
    pub meaning: &'static str,
    pub observations: Vec<String>,
}

/// One VMS position (from [`super::vms`]).
pub type VmsPoint = (f64, f64, f64);

/// Classify a gap; `vms` is the vessel's VMS track (time, lat, lon), if any.
pub fn classify_gap(
    g: &Gap,
    t: &Tracks,
    vms: Option<&[VmsPoint]>,
    neighbour_radius_m: f64,
    p: &Params,
) -> GapClass {
    let mut obs = Vec::new();
    let nofix: Vec<f64> = t
        .unavailable
        .get(&g.mmsi)
        .map(|u| {
            u.iter()
                .copied()
                .filter(|x| *x > g.t0 && *x < g.t1)
                .collect()
        })
        .unwrap_or_default();
    let mk = |cause, meaning, obs| GapClass {
        mmsi: g.mmsi.clone(),
        start_utc: g.start_utc.clone(),
        end_utc: g.end_utc.clone(),
        cause,
        meaning,
        observations: obs,
    };
    if nofix.len() >= 2 {
        obs.extend(
            nofix
                .iter()
                .take(MAX_OBS)
                .map(|x| format!("report without position at {}", iso(*x))),
        );
        return mk(
            GapCause::TransponderOnNoFix,
            "the transponder kept transmitting without a GNSS fix: consistent with GNSS interference or a receiver fault, not with AIS switched off",
            obs,
        );
    }
    // Neighbours present at the gap start.
    let mut n = 0;
    let mut affected = 0;
    for (m, v) in &t.by_mmsi {
        if m == &g.mmsi {
            continue;
        }
        let Some((la, lo, _)) = interp(v, g.t0, p.interp_max_s) else {
            continue;
        };
        if dist(la, lo, g.start[0], g.start[1]) > neighbour_radius_m {
            continue;
        }
        n += 1;
        let lost_reports = gaps(v, p)
            .iter()
            .any(|h| (h.t1.min(g.t1) - h.t0.max(g.t0)) >= 0.5 * (g.t1 - g.t0));
        let lost_fix = t
            .unavailable
            .get(m)
            .is_some_and(|u| u.iter().filter(|x| **x > g.t0 && **x < g.t1).count() >= 2);
        if lost_reports || lost_fix {
            affected += 1;
            obs.push(format!(
                "neighbour {m} also lost {} over the same time",
                if lost_fix { "its fix" } else { "reports" }
            ));
        }
    }
    if n >= 2 && affected * 2 >= n {
        obs.push(format!(
            "{affected} of {n} neighbours within {:.0} km affected",
            neighbour_radius_m / 1000.0
        ));
        return mk(
            GapCause::AreaWide,
            "most neighbours lost reports or fixes over the same time: consistent with interference or a coverage loss in the area",
            obs,
        );
    }
    if let Some(vm) = vms {
        let inside: Vec<&VmsPoint> = vm.iter().filter(|x| x.0 > g.t0 && x.0 < g.t1).collect();
        if inside.len() >= 2 {
            let plausible = inside.windows(2).all(|w| {
                let dt = (w[1].0 - w[0].0).max(1.0);
                dist(w[0].1, w[0].2, w[1].1, w[1].2) / dt / KN <= p.max_speed_kn
            });
            if plausible {
                obs.extend(
                    inside
                        .iter()
                        .take(MAX_OBS)
                        .map(|x| format!("VMS position at {} ({:.4},{:.4})", iso(x.0), x.1, x.2)),
                );
                return mk(
                    GapCause::AisOnlySilent,
                    "the vessel's VMS kept reporting a plausible track while AIS was silent: consistent with AIS switched off or silenced",
                    obs,
                );
            }
        }
    }
    obs.push(format!(
        "{n} neighbour(s) within {:.0} km, {affected} affected; no VMS through the gap",
        neighbour_radius_m / 1000.0
    ));
    mk(
        GapCause::Undetermined,
        "the data do not separate the causes",
        obs,
    )
}

/// Coefficients of the anchor-drag logistic link (stated, not fitted): intercept, slow-band
/// share, heading-course divergence, straightness × slow share, anchor status while moving.
pub const DRAG_COEFFS: [f64; 5] = [-4.0, 3.0, 2.5, 1.5, 2.0];

/// Anchor-drag result for one vessel.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Drag {
    pub mmsi: String,
    pub max_likelihood: f64,
    /// First time the likelihood reached the threshold within `alert_distance_m` of an asset.
    pub alert_utc: Option<String>,
    /// First entry into an asset corridor at or below the slow speed.
    pub corridor_entry_utc: Option<String>,
    pub corridor: Option<String>,
    /// Corridor entry minus alert (h): positive means the alert came first.
    pub lead_h: Option<f64>,
    /// Features at the maximum.
    pub features: BTreeMap<&'static str, f64>,
    pub observations: Vec<String>,
}

fn wrap(a: f64) -> f64 {
    let x = (a + 180.0).rem_euclid(360.0) - 180.0;
    x.abs()
}

/// Running anchor-drag likelihood of one track against declared corridors.
pub fn anchor_drag(
    v: &[Report],
    corridors: &[Asset],
    window_s: f64,
    threshold: f64,
    alert_distance_m: f64,
    slow_kn: f64,
) -> Drag {
    let [c0, c1, c2, c3, c4] = DRAG_COEFFS;
    let mut best = (0.0, BTreeMap::new());
    let mut alert: Option<f64> = None;
    let mut entry: Option<(f64, String)> = None;
    let mut obs = Vec::new();
    for (i, x) in v.iter().enumerate() {
        // Corridor entry (slow, inside).
        if entry.is_none() && x.sog_kn.is_some_and(|s| s <= slow_kn) {
            if let Some(a) = corridors.iter().find(|a| a.contains(x.lat, x.lon)) {
                let n = &a.name;
                entry = Some((x.t, n.clone()));
                obs.push(format!("slow inside corridor '{n}' at {}", iso(x.t)));
            }
        }
        let w: Vec<&Report> = v[..=i]
            .iter()
            .rev()
            .take_while(|y| x.t - y.t <= window_s)
            .collect();
        if w.len() < 3 || x.t - w[w.len() - 1].t < 0.5 * window_s {
            continue;
        }
        let slow = w
            .iter()
            .filter(|y| y.sog_kn.is_some_and(|s| (0.3..=4.0).contains(&s)))
            .count() as f64
            / w.len() as f64;
        let div: Vec<f64> = w
            .iter()
            .filter_map(|y| Some(wrap(y.heading_deg? - y.cog_deg?)))
            .collect();
        let divergence = if div.is_empty() {
            0.0
        } else {
            (div.iter().sum::<f64>() / div.len() as f64 / 90.0).min(1.0)
        };
        let path: f64 = w
            .windows(2)
            .map(|p| dist(p[0].lat, p[0].lon, p[1].lat, p[1].lon))
            .sum();
        let disp = dist(w[0].lat, w[0].lon, w[w.len() - 1].lat, w[w.len() - 1].lon);
        let straight = if path > 0.0 { disp / path } else { 0.0 };
        let anch = w
            .iter()
            .filter(|y| matches!(y.nav_status, Some(1 | 5)) && y.sog_kn.is_some_and(|s| s >= 0.5))
            .count() as f64
            / w.len() as f64;
        let z = c0 + c1 * slow + c2 * divergence + c3 * straight * slow + c4 * anch;
        let lik = 1.0 / (1.0 + (-z).exp());
        if lik > best.0 {
            best = (
                lik,
                BTreeMap::from([
                    ("slow_share", r(slow, 3)),
                    ("heading_course_divergence", r(divergence, 3)),
                    ("straightness", r(straight, 3)),
                    ("anchor_status_moving_share", r(anch, 3)),
                ]),
            );
        }
        if alert.is_none()
            && lik >= threshold
            && corridors
                .iter()
                .any(|a| a.distance_m(x.lat, x.lon) <= a.buffer_m + alert_distance_m)
        {
            alert = Some(x.t);
            obs.push(format!(
                "likelihood {lik:.2} at {} within {:.0} km of an asset",
                iso(x.t),
                alert_distance_m / 1000.0
            ));
        }
    }
    Drag {
        mmsi: v.first().map(|x| x.mmsi.clone()).unwrap_or_default(),
        max_likelihood: r(best.0, 3),
        alert_utc: alert.map(iso),
        corridor_entry_utc: entry.as_ref().map(|e| iso(e.0)),
        corridor: entry.as_ref().map(|e| e.1.clone()),
        lead_h: match (alert, &entry) {
            (Some(a), Some((e, _))) => Some(r((e - a) / 3600.0, 2)),
            _ => None,
        },
        features: best.1,
        observations: obs,
    }
}
