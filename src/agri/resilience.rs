// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The autosteer GNSS-loss resilience score and the outage diagnosis, from a machine log.
//!
//! **Episodes.** A degraded episode is a run of epochs below the fix the work needs
//! ([`Profile::required_fix`]), counted while steering is engaged when the log records it.
//! Runs separated by less than [`Profile::merge_s`] are one episode. An episode's
//! **recovery time** is from its start until the required fix is back and held for
//! [`Profile::hold_s`].
//!
//! **Score (0-100).** Four components, each 0-1, weighted (weights are named defaults):
//!
//! | Component | Weight | Meaning |
//! |---|---|---|
//! | availability | 40 | share of engaged time at the required fix |
//! | recovery | 20 | median recovery time: 1 at or under `recovery_good_s`, 0 at or over `recovery_bad_s`, linear between |
//! | continuity | 20 | share of episodes the machine rode through without steering dropping out |
//! | degraded accuracy | 20 | share of degraded time with the guidance cross-track error within tolerance |
//!
//! A component the log cannot inform (no cross-track error column; no episodes for recovery
//! and continuity, which then score 1) is reported as not assessed and its weight is shared
//! over the others. Grades: A at 85 or more, B 70, C 50, D below.
//!
//! **Diagnosis** (indicative; the dealer's "jamming, space weather or correction feed?"):
//!
//! * *interference-like*: the satellite count falls by [`Profile::sat_drop_frac`] or more
//!   against the minute before, or mean C/N0 falls by [`Profile::cn0_drop_db`] or more, or
//!   the position is lost outright;
//! * *correction-feed-like*: satellites and C/N0 hold, the fix falls to a non-RTK solution and
//!   the correction age exceeds [`Profile::max_corr_age_s`] (or is absent while the fix drops);
//! * *ionosphere-like*: satellites and C/N0 hold, corrections are fresh, and the fix stays in
//!   RTK float or toggles between fixed and float [`Profile::toggle_min`] times or more;
//!   a Kp at or above 5 during the episode (when a Kp series is given) is added as support;
//! * otherwise *undetermined*.
//!
//! This reads signatures in one machine's log. It does not locate any source and is not an
//! attribution; a log without satellite counts or C/N0 mostly yields *undetermined*.

use serde::{Deserialize, Serialize};

use super::log::{Epoch, Fix, MachineLog};
use super::spaceweather::KpSeries;

/// The machine class (labels the report; sets demonstration default tolerances).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineClass {
    Agriculture,
    Forestry,
    Construction,
}

/// What the work needs and the thresholds the score uses (`kshana-agri-profile/1`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub format: String,
    pub class: MachineClass,
    pub required_fix: Fix,
    /// Cross-track tolerance (m).
    pub tolerance_m: f64,
    pub merge_s: f64,
    pub hold_s: f64,
    pub recovery_good_s: f64,
    pub recovery_bad_s: f64,
    pub max_corr_age_s: f64,
    pub sat_drop_frac: f64,
    pub cn0_drop_db: f64,
    pub toggle_min: usize,
    pub weights: [f64; 4],
    /// Where the values came from.
    pub source: String,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            format: "kshana-agri-profile/1".into(),
            class: MachineClass::Agriculture,
            required_fix: Fix::RtkFixed,
            tolerance_m: 0.05,
            merge_s: 10.0,
            hold_s: 10.0,
            recovery_good_s: 10.0,
            recovery_bad_s: 300.0,
            max_corr_age_s: 10.0,
            sat_drop_frac: 0.4,
            cn0_drop_db: 6.0,
            toggle_min: 3,
            weights: [40.0, 20.0, 20.0, 20.0],
            source: "kshana-pro demonstration defaults (not a machine maker's figures)".into(),
        }
    }
}

impl Profile {
    /// Defaults for a class: forestry tolerates more (0.5 m), construction grade control
    /// less (0.03 m). Demonstration values.
    pub fn for_class(class: MachineClass) -> Self {
        let mut p = Profile {
            class,
            ..Default::default()
        };
        match class {
            MachineClass::Agriculture => {}
            MachineClass::Forestry => {
                p.tolerance_m = 0.5;
                p.required_fix = Fix::Dgnss;
            }
            MachineClass::Construction => p.tolerance_m = 0.03,
        }
        p
    }

    pub fn parse(text: &str) -> Result<Profile, String> {
        let p: Profile = toml::from_str(text).map_err(|e| format!("profile: {e}"))?;
        if p.format != "kshana-agri-profile/1" {
            return Err(format!(
                "profile format '{}' is not kshana-agri-profile/1",
                p.format
            ));
        }
        if p.recovery_bad_s <= p.recovery_good_s || p.weights.iter().any(|w| *w < 0.0) {
            return Err("profile: recovery_bad_s must exceed recovery_good_s; weights >= 0".into());
        }
        Ok(p)
    }
}

/// Indicative cause of an episode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    InterferenceLike,
    CorrectionFeedLike,
    IonosphereLike,
    Undetermined,
}

/// One degraded episode.
#[derive(Clone, Debug, Serialize)]
pub struct Episode {
    pub start_s: f64,
    pub end_s: f64,
    pub duration_s: f64,
    pub worst_fix: Fix,
    /// Time to regain and hold the required fix (s); `None` if the log ends first.
    pub recovery_s: Option<f64>,
    pub steering_dropped: bool,
    pub max_abs_xte_m: Option<f64>,
    pub cause: Cause,
    pub evidence: Vec<String>,
}

/// One score component.
#[derive(Clone, Debug, Serialize)]
pub struct Component {
    pub name: &'static str,
    pub weight: f64,
    pub value: Option<f64>,
    pub detail: String,
}

/// The resilience report.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub label: &'static str,
    pub class: MachineClass,
    pub required_fix: Fix,
    pub profile_source: String,
    pub span_s: f64,
    pub engaged_s: f64,
    pub score: f64,
    pub grade: char,
    pub components: Vec<Component>,
    pub episodes: Vec<Episode>,
    pub causes: std::collections::BTreeMap<String, usize>,
    pub notes: Vec<String>,
}

fn counted(e: &Epoch, has_steer: bool) -> bool {
    !has_steer || e.steer == Some(true)
}

fn median(v: &[f64]) -> Option<f64> {
    super::percentile(v, 50.0)
}

/// Diagnose one episode (indices `a..b` of `log.epochs`).
fn diagnose(
    log: &MachineLog,
    a: usize,
    b: usize,
    p: &Profile,
    kp: Option<&KpSeries>,
) -> (Cause, Vec<String>) {
    let ep = &log.epochs;
    let t0 = ep[a].t_s;
    let before: Vec<&Epoch> = ep[..a]
        .iter()
        .rev()
        .take_while(|e| t0 - e.t_s <= 60.0)
        .collect();
    let during = &ep[a..b];
    let mean = |v: Vec<f64>| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
    let sats_b = mean(
        before
            .iter()
            .filter_map(|e| e.sats.map(f64::from))
            .collect(),
    );
    let sats_d = during
        .iter()
        .filter_map(|e| e.sats.map(f64::from))
        .reduce(f64::min);
    let cn0_b = mean(before.iter().filter_map(|e| e.cn0).collect());
    let cn0_d = mean(during.iter().filter_map(|e| e.cn0).collect());
    let age_d = during.iter().filter_map(|e| e.corr_age).reduce(f64::max);
    let mut ev = Vec::new();
    let sat_drop = match (sats_b, sats_d) {
        (Some(x), Some(y)) if x > 0.0 => {
            ev.push(format!("satellites {x:.1} before, minimum {y:.0} during"));
            Some((x - y) / x >= p.sat_drop_frac)
        }
        _ => None,
    };
    let cn0_drop = match (cn0_b, cn0_d) {
        (Some(x), Some(y)) => {
            ev.push(format!("mean C/N0 {x:.1} dB-Hz before, {y:.1} during"));
            Some(x - y >= p.cn0_drop_db)
        }
        _ => None,
    };
    let lost = during.iter().any(|e| !e.fix.has_position());
    if lost {
        ev.push("position lost during the episode".into());
    }
    if sat_drop == Some(true) || cn0_drop == Some(true) || (lost && sat_drop != Some(false)) {
        return (Cause::InterferenceLike, ev);
    }
    let signal_held = sat_drop == Some(false) || cn0_drop == Some(false);
    let non_rtk = during
        .iter()
        .any(|e| e.fix < Fix::RtkFloat && e.fix.has_position());
    if let Some(age) = age_d {
        ev.push(format!("correction age up to {age:.1} s"));
    }
    let stale = age_d.map(|a| a > p.max_corr_age_s);
    if signal_held && non_rtk && stale != Some(false) {
        return (Cause::CorrectionFeedLike, ev);
    }
    // Fixed/float toggles across the episode and its surroundings.
    let lo = a.saturating_sub(5);
    let hi = (b + 5).min(ep.len());
    let toggles = ep[lo..hi]
        .windows(2)
        .filter(|w| {
            (w[0].fix == Fix::RtkFixed && w[1].fix == Fix::RtkFloat)
                || (w[0].fix == Fix::RtkFloat && w[1].fix == Fix::RtkFixed)
        })
        .count();
    let float_only = during.iter().all(|e| e.fix == Fix::RtkFloat);
    if signal_held && stale != Some(true) && (toggles >= p.toggle_min || float_only) {
        ev.push(format!("{toggles} fixed/float toggles; corrections fresh"));
        if let Some(k) = kp.and_then(|k| k.max_between(t0, ep[b - 1].t_s)) {
            ev.push(format!("Kp up to {k:.2} during the episode"));
        }
        return (Cause::IonosphereLike, ev);
    }
    if let Some(k) = kp.and_then(|k| k.max_between(t0, ep[b - 1].t_s)) {
        ev.push(format!("Kp up to {k:.2} during the episode"));
    }
    (Cause::Undetermined, ev)
}

/// Score a log. `kp` (optional) adds geomagnetic context to the diagnosis; its times must be
/// on the same clock as the log (unix seconds).
pub fn score(log: &MachineLog, p: &Profile, kp: Option<&KpSeries>) -> Report {
    let ep = &log.epochs;
    let has_steer = log.has_steer();
    let dt = log.interval_s();
    // Epoch weights: the time each epoch stands for (capped at 5 intervals).
    let w: Vec<f64> = (0..ep.len())
        .map(|i| {
            ep.get(i + 1)
                .map(|n| (n.t_s - ep[i].t_s).min(5.0 * dt))
                .unwrap_or(dt)
        })
        .collect();
    let mut engaged = 0.0;
    let mut good = 0.0;
    for (i, e) in ep.iter().enumerate() {
        if counted(e, has_steer) {
            engaged += w[i];
            if e.fix >= p.required_fix {
                good += w[i];
            }
        }
    }
    // Episodes: runs of degraded epochs (counted, or steering dropped from a counted run).
    let mut raw: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < ep.len() {
        let degraded = |e: &Epoch| e.fix < p.required_fix;
        if degraded(&ep[i])
            && (counted(&ep[i], has_steer) || (i > 0 && counted(&ep[i - 1], has_steer)))
        {
            let a = i;
            while i < ep.len() && degraded(&ep[i]) {
                i += 1;
            }
            raw.push((a, i));
        } else {
            i += 1;
        }
    }
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for r in raw {
        match runs.last_mut() {
            Some(last) if ep[r.0].t_s - ep[last.1 - 1].t_s < p.merge_s => last.1 = r.1,
            _ => runs.push(r),
        }
    }
    let mut episodes = Vec::new();
    let mut degraded_t = 0.0;
    let mut degraded_ok = 0.0;
    let mut xte_seen = false;
    for (a, b) in runs {
        let start = ep[a].t_s;
        let end = ep[b - 1].t_s + w[b - 1];
        // Recovery: first time from b on with the required fix held for hold_s.
        let mut rec = None;
        let mut j = b;
        while j < ep.len() {
            if ep[j].fix >= p.required_fix {
                let t = ep[j].t_s;
                let mut k = j;
                while k < ep.len() && ep[k].fix >= p.required_fix && ep[k].t_s - t < p.hold_s {
                    k += 1;
                }
                if k < ep.len() && ep[k].fix >= p.required_fix {
                    rec = Some(ep[k].t_s - start);
                    break;
                }
                if k >= ep.len() {
                    break;
                }
                j = k;
            } else {
                j += 1;
            }
        }
        let dropped = has_steer && ep[a..b].iter().any(|e| e.steer == Some(false));
        let mut max_x: Option<f64> = None;
        for (k, e) in ep[a..b].iter().enumerate() {
            if let Some(x) = e.xte {
                xte_seen = true;
                max_x = Some(max_x.unwrap_or(0.0).max(x.abs()));
                if counted(e, has_steer) {
                    degraded_t += w[a + k];
                    if x.abs() <= p.tolerance_m {
                        degraded_ok += w[a + k];
                    }
                }
            }
        }
        let worst = ep[a..b].iter().map(|e| e.fix).min().unwrap_or(Fix::None);
        let (cause, evidence) = diagnose(log, a, b, p, kp);
        episodes.push(Episode {
            start_s: start,
            end_s: end,
            duration_s: super::round(end - start, 2),
            worst_fix: worst,
            recovery_s: rec.map(|r| super::round(r, 2)),
            steering_dropped: dropped,
            max_abs_xte_m: max_x.map(|x| super::round(x, 4)),
            cause,
            evidence,
        });
    }
    let mut notes = Vec::new();
    let avail = (engaged > 0.0).then(|| good / engaged);
    let recs: Vec<f64> = episodes
        .iter()
        .map(|e| e.recovery_s.unwrap_or(p.recovery_bad_s))
        .collect();
    let rec_v = if episodes.is_empty() {
        Some(1.0)
    } else {
        median(&recs).map(|m| {
            ((p.recovery_bad_s - m) / (p.recovery_bad_s - p.recovery_good_s)).clamp(0.0, 1.0)
        })
    };
    let cont_v = if !has_steer {
        None
    } else if episodes.is_empty() {
        Some(1.0)
    } else {
        Some(episodes.iter().filter(|e| !e.steering_dropped).count() as f64 / episodes.len() as f64)
    };
    let acc_v = if !xte_seen {
        if episodes.is_empty() {
            None
        } else {
            notes.push(
                "The log has no cross-track error while degraded: accuracy not assessed.".into(),
            );
            None
        }
    } else {
        (degraded_t > 0.0).then(|| degraded_ok / degraded_t)
    };
    if !has_steer {
        notes.push("The log does not record steering engagement: every epoch counted and continuity not assessed.".into());
    }
    let comps = vec![
        Component {
            name: "availability",
            weight: p.weights[0],
            value: avail,
            detail: format!(
                "{:.1} % of {:.0} s at {} or better",
                avail.unwrap_or(0.0) * 100.0,
                engaged,
                p.required_fix.as_str()
            ),
        },
        Component {
            name: "recovery",
            weight: p.weights[1],
            value: rec_v,
            detail: match median(&recs) {
                Some(m) => format!("median recovery {m:.1} s over {} episodes", episodes.len()),
                None => "no episodes".into(),
            },
        },
        Component {
            name: "continuity",
            weight: p.weights[2],
            value: cont_v,
            detail: format!(
                "{} of {} episodes without steering dropping out",
                episodes.iter().filter(|e| !e.steering_dropped).count(),
                episodes.len()
            ),
        },
        Component {
            name: "degraded_accuracy",
            weight: p.weights[3],
            value: acc_v,
            detail: format!(
                "{:.0} s of {:.0} s degraded time within {} m",
                degraded_ok, degraded_t, p.tolerance_m
            ),
        },
    ];
    let wsum: f64 = comps
        .iter()
        .filter(|c| c.value.is_some())
        .map(|c| c.weight)
        .sum();
    let score = if wsum > 0.0 {
        comps
            .iter()
            .filter_map(|c| c.value.map(|v| v * c.weight))
            .sum::<f64>()
            / wsum
            * 100.0
    } else {
        0.0
    };
    let grade = match score {
        s if s >= 85.0 => 'A',
        s if s >= 70.0 => 'B',
        s if s >= 50.0 => 'C',
        _ => 'D',
    };
    let mut causes = std::collections::BTreeMap::new();
    for e in &episodes {
        *causes
            .entry(
                serde_json::to_value(e.cause)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
            .or_insert(0) += 1;
    }
    Report {
        label: super::LABEL,
        class: p.class,
        required_fix: p.required_fix,
        profile_source: p.source.clone(),
        span_s: log.span_s(),
        engaged_s: super::round(engaged, 1),
        score: super::round(score, 1),
        grade,
        components: comps,
        episodes,
        causes,
        notes,
    }
}

/// A Markdown summary.
pub fn markdown(r: &Report) -> String {
    let mut m = format!(
        "# Autosteer GNSS-loss resilience ({})\n\nClass {:?}, required fix `{}`; {:.0} s engaged of {:.0} s logged.\n\n**Score {:.1} / 100 (grade {})**\n\n| Component | Weight | Value | Detail |\n|---|---|---|---|\n",
        r.label,
        r.class,
        r.required_fix.as_str(),
        r.engaged_s,
        r.span_s,
        r.score,
        r.grade
    );
    for c in &r.components {
        m.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            c.name,
            c.weight,
            c.value
                .map(|v| format!("{v:.3}"))
                .unwrap_or_else(|| "not assessed".into()),
            c.detail
        ));
    }
    m.push_str("\n| Start s | Duration s | Worst fix | Recovery s | Steering dropped | Max XTE m | Indicative cause |\n|---|---|---|---|---|---|---|\n");
    for e in &r.episodes {
        m.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            e.start_s,
            e.duration_s,
            e.worst_fix.as_str(),
            e.recovery_s
                .map(|x| x.to_string())
                .unwrap_or_else(|| "-".into()),
            e.steering_dropped,
            e.max_abs_xte_m
                .map(|x| x.to_string())
                .unwrap_or_else(|| "-".into()),
            serde_json::to_value(e.cause).unwrap().as_str().unwrap()
        ));
    }
    m.push_str("\nThe cause column reads signatures in this log only; it is not an attribution and locates no source.\n");
    for n in &r.notes {
        m.push_str(&format!("\n- {n}"));
    }
    m.push_str(&format!("\n\nProfile values: {}.\n", r.profile_source));
    m
}
