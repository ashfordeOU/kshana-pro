// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Scoring a receiver's output against a scenario's criteria (method [`METHOD_ID`]).
//!
//! The receiver output is the PICS device log (`kshana-pics-device-log/1`,
//! [`crate::pics::log::DeviceLog`]): per epoch the fix and its validity, the time error when
//! measured, and the receiver's own flags. Truth is the scenario's trajectory at each epoch
//! time (moving or static). Errors: horizontal = haversine distance to truth; time = |logged
//! time error|.
//!
//! An epoch is *valid* when the receiver does not withdraw its fix; *flagged* when it raises
//! a jamming, spoofing or integrity warning. Metrics, over the criterion's window:
//!
//! * `detection_latency_s`: first flagged epoch (or withdrawn fix, when
//!   `withdraw_counts_as_detection`) after onset, minus onset; none in the window: not met.
//! * `misleading_epochs`: valid, unflagged epochs with an error beyond the threshold
//!   (`[expected] misleading_*` unless the criterion gives one).
//! * `max_unflagged_h_error_m` / `max_unflagged_time_error_ns`: largest error of valid,
//!   unflagged epochs (0 when there are none).
//! * `recovery_s`: from the threat's end to the first epoch from which, for the dwell,
//!   every epoch is valid, unflagged and within the recovery threshold.
//! * `availability`: fraction of epochs valid (and within the threshold when one is given).
//! * `max_h_error_m`, `p95_h_error_m`, `max_time_error_ns`: over valid epochs, flagged or not.
//! * `false_alarms`: flagged epochs (in the baseline by default).
//!
//! A metric whose data the log lacks (no epoch in the window, no position or time error, a
//! log ending before a recovery could be seen) is **not assessed**, never met. A scenario
//! `pass`es when every required criterion is met, `fail`s when one is not met, and is
//! `incomplete` otherwise.

use kshana::interference_map::grid::haversine_m;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::fidelity::FidelityStatement;
use super::spec::{Criterion, Metric, Spec, Window};
use crate::pics::log::DeviceLog;
use crate::scorecards::levels::Verdict;

pub const METHOD_ID: &str = "kshana-pro/testkit/v1";
pub const VERDICT_FORMAT: &str = "kshana-testkit-verdict/1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CriterionResult {
    pub id: String,
    pub metric: String,
    pub window: String,
    pub from_s: f64,
    pub to_s: f64,
    pub op: String,
    pub value: f64,
    pub required: bool,
    #[serde(default)]
    pub level: Option<u8>,
    pub measured: Option<f64>,
    pub verdict: Verdict,
    pub detail: String,
}

/// One scenario's verdict for one build.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioVerdict {
    pub format: String,
    pub method: String,
    pub label: String,
    pub scenario: String,
    pub scenario_version: u32,
    pub scenario_sha256: String,
    pub title: String,
    pub build: String,
    pub receiver: String,
    pub source: String,
    pub log_sha256: String,
    pub epochs: usize,
    pub criteria: Vec<CriterionResult>,
    /// `pass`, `fail`, `incomplete` or `not-run`.
    pub verdict: String,
    /// The highest resilience level (1 to 4, modelled on the DHS RPCF) whose criteria, and
    /// every lower level's, are all met; 0 when level 1 is not.
    pub level_supported: Option<u8>,
    #[serde(default)]
    pub fidelity: Option<FidelityStatement>,
    /// The verdict's qualification by replay fidelity, in words (empty when none applies).
    #[serde(default)]
    pub qualification: String,
}

struct Ep {
    t: f64,
    valid: bool,
    flagged: bool,
    withdrawn: bool,
    h: Option<f64>,
    te: Option<f64>,
}

fn window(spec: &Spec, c: &Criterion) -> Option<(Window, f64, f64)> {
    let w = c.window.unwrap_or(c.metric.default_window());
    let d = spec.scene.duration_s;
    let tw = spec.threat_window(c.threat.as_deref());
    Some(match w {
        Window::All => (w, 0.0, d),
        Window::Baseline => (w, 0.0, tw.map(|x| x.0).unwrap_or(d)),
        Window::Threat => {
            let (a, b) = tw?;
            (w, a, b)
        }
        Window::After => {
            let (_, b) = tw?;
            (w, b, d)
        }
        Window::FromOnset => {
            let (a, _) = tw?;
            (w, a, d)
        }
    })
}

fn p95(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let k = ((v.len() as f64) * 0.95).ceil() as usize;
    Some(v[k.clamp(1, v.len()) - 1])
}

fn crit(spec: &Spec, c: &Criterion, eps: &[Ep]) -> CriterionResult {
    let ex = &spec.expected;
    let mut r = CriterionResult {
        id: c.id.clone(),
        metric: c.metric.as_str().into(),
        window: c
            .window
            .unwrap_or(c.metric.default_window())
            .as_str()
            .into(),
        from_s: 0.0,
        to_s: 0.0,
        op: c.op.as_str().into(),
        value: c.value,
        required: c.required,
        level: c.level,
        measured: None,
        verdict: Verdict::NotAssessed,
        detail: String::new(),
    };
    let Some((_, a, b)) = window(spec, c) else {
        r.detail = "the scenario has no threat for this window".into();
        return r;
    };
    r.from_s = a;
    r.to_s = b;
    let inw: Vec<&Ep> = eps.iter().filter(|e| e.t >= a && e.t < b).collect();
    let last_t = eps.last().map(|e| e.t).unwrap_or(f64::NEG_INFINITY);
    if inw.is_empty() {
        r.detail = format!("no receiver epoch in {a:.0}–{b:.0} s");
        return r;
    }
    let timing = c.metric.is_timing();
    let err = |e: &Ep, dh: f64, dt: f64| -> Option<bool> {
        // Some(true) = beyond the threshold; None = no data in the metric's domain.
        if timing {
            return e.te.map(|x| x > c.threshold.unwrap_or(dt));
        }
        let hb = e.h.map(|h| h > c.threshold.unwrap_or(dh));
        let tb = e.te.map(|t| t > dt);
        match (hb, tb) {
            (None, None) => None,
            (x, y) => Some(x.unwrap_or(false) || y.unwrap_or(false)),
        }
    };
    let has_data = |s: &[&Ep]| {
        s.iter().any(|e| {
            if timing {
                e.te.is_some()
            } else {
                e.h.is_some() || e.te.is_some()
            }
        })
    };
    let measured: Option<f64> = match c.metric {
        Metric::DetectionLatencyS => {
            let hit = inw
                .iter()
                .find(|e| e.flagged || (ex.withdraw_counts_as_detection && e.withdrawn));
            match hit {
                Some(e) => {
                    r.detail = format!(
                        "detected at {:.1} s ({})",
                        e.t,
                        if e.flagged { "flag" } else { "fix withdrawn" }
                    );
                    Some(e.t - a)
                }
                None => {
                    r.detail = "not detected in the threat window".into();
                    r.verdict = Verdict::NotMet;
                    return r;
                }
            }
        }
        Metric::MisleadingEpochs => {
            if !has_data(&inw) {
                None
            } else {
                let n = inw
                    .iter()
                    .filter(|e| {
                        e.valid
                            && !e.flagged
                            && err(e, ex.misleading_h_error_m, ex.misleading_time_error_ns)
                                == Some(true)
                    })
                    .count();
                r.detail = format!("{n} valid, unflagged epochs beyond the misleading threshold");
                Some(n as f64)
            }
        }
        Metric::MaxUnflaggedHErrorM | Metric::MaxUnflaggedTimeErrorNs => {
            if !has_data(&inw) {
                None
            } else {
                let v = inw
                    .iter()
                    .filter(|e| e.valid && !e.flagged)
                    .filter_map(|e| if timing { e.te } else { e.h })
                    .fold(0.0, f64::max);
                Some(v)
            }
        }
        Metric::MaxHErrorM | Metric::MaxTimeErrorNs | Metric::P95HErrorM => {
            let v: Vec<f64> = inw
                .iter()
                .filter(|e| e.valid)
                .filter_map(|e| if timing { e.te } else { e.h })
                .collect();
            if v.is_empty() {
                None
            } else if c.metric == Metric::P95HErrorM {
                p95(v)
            } else {
                Some(v.iter().copied().fold(0.0, f64::max))
            }
        }
        Metric::Availability => {
            let ok = inw
                .iter()
                .filter(|e| {
                    e.valid
                        && match (c.threshold, e.h, e.te) {
                            (None, _, _) => true,
                            (Some(th), Some(h), _) => h <= th,
                            (Some(th), None, Some(t)) => t <= th,
                            _ => false,
                        }
                })
                .count();
            Some(ok as f64 / inw.len() as f64)
        }
        Metric::FalseAlarms => Some(inw.iter().filter(|e| e.flagged).count() as f64),
        Metric::RecoveryS => {
            if !has_data(&inw) {
                None
            } else {
                let dwell = ex.recover_dwell_s;
                let good = |e: &Ep| {
                    e.valid
                        && !e.flagged
                        && err(e, ex.recover_h_error_m, ex.recover_time_error_ns) == Some(false)
                };
                let mut found = None;
                for (i, e) in inw.iter().enumerate() {
                    if !good(e) {
                        continue;
                    }
                    let span: Vec<&&Ep> =
                        inw[i..].iter().take_while(|x| x.t <= e.t + dwell).collect();
                    let covered = span.last().is_some_and(|x| x.t >= e.t + dwell - 1e-6);
                    if covered && span.iter().all(|x| good(x)) {
                        found = Some(e.t - a);
                        break;
                    }
                }
                match found {
                    Some(v) => Some(v),
                    None if last_t >= a + c.value + dwell => {
                        r.detail = "no recovery held for the dwell".into();
                        r.verdict = Verdict::NotMet;
                        return r;
                    }
                    None => {
                        r.detail = "the log ends before a recovery could be seen".into();
                        return r;
                    }
                }
            }
        }
    };
    match measured {
        Some(m) => {
            r.measured = Some(m);
            r.verdict = Verdict::from_bool(Some(c.op.holds(m, c.value)));
            if r.detail.is_empty() {
                r.detail = format!("{m:.3} {} {}", c.op.as_str(), c.value);
            }
        }
        None => {
            r.detail = format!(
                "the log has no {} data in the window",
                if timing {
                    "time-error"
                } else {
                    "position or time-error"
                }
            );
        }
    }
    r
}

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// Score one receiver log against one (resolved) scenario.
pub fn score(
    spec: &Spec,
    log: &DeviceLog,
    build: &str,
    receiver: &str,
) -> Result<ScenarioVerdict, String> {
    log.validate()?;
    let eps: Vec<Ep> = log
        .epochs
        .iter()
        .map(|e| {
            let tr = spec.trajectory.at(e.t_s);
            Ep {
                t: e.t_s,
                valid: e.fix_valid != Some(false),
                flagged: e.flagged(),
                withdrawn: e.withdrawn(),
                h: match (e.lat_deg, e.lon_deg) {
                    (Some(la), Some(lo)) if e.fix_valid != Some(false) => {
                        Some(haversine_m(tr.lat_deg, tr.lon_deg, la, lo))
                    }
                    _ => None,
                },
                te: e.time_error_ns.map(f64::abs),
            }
        })
        .collect();
    let criteria: Vec<CriterionResult> =
        spec.criteria.iter().map(|c| crit(spec, c, &eps)).collect();
    let verdict = verdict_of(&criteria);
    Ok(ScenarioVerdict {
        format: VERDICT_FORMAT.into(),
        method: METHOD_ID.into(),
        label: super::LABEL.into(),
        scenario: spec.id.clone(),
        scenario_version: spec.version,
        scenario_sha256: spec.sha256(),
        title: spec.title.clone(),
        build: build.into(),
        receiver: receiver.into(),
        source: log.source.clone().unwrap_or_else(|| "json".into()),
        log_sha256: sha(log.to_json().as_bytes()),
        epochs: log.epochs.len(),
        level_supported: level_supported(&criteria),
        criteria,
        verdict: verdict.into(),
        fidelity: None,
        qualification: String::new(),
    })
}

/// A scenario that could not be run (no log), recorded so the gap is visible.
pub fn not_run(spec: &Spec, build: &str, receiver: &str, why: &str) -> ScenarioVerdict {
    ScenarioVerdict {
        format: VERDICT_FORMAT.into(),
        method: METHOD_ID.into(),
        label: super::LABEL.into(),
        scenario: spec.id.clone(),
        scenario_version: spec.version,
        scenario_sha256: spec.sha256(),
        title: spec.title.clone(),
        build: build.into(),
        receiver: receiver.into(),
        source: why.into(),
        log_sha256: String::new(),
        epochs: 0,
        criteria: vec![],
        verdict: "not-run".into(),
        level_supported: None,
        fidelity: None,
        qualification: String::new(),
    }
}

fn verdict_of(c: &[CriterionResult]) -> &'static str {
    let req: Vec<&CriterionResult> = c.iter().filter(|x| x.required).collect();
    if req.iter().any(|x| x.verdict == Verdict::NotMet) {
        "fail"
    } else if req.is_empty() || req.iter().any(|x| x.verdict == Verdict::NotAssessed) {
        "incomplete"
    } else {
        "pass"
    }
}

fn level_supported(c: &[CriterionResult]) -> Option<u8> {
    if !c.iter().any(|x| x.level.is_some()) {
        return None;
    }
    let mut lvl = 0;
    for l in 1..=4u8 {
        let at: Vec<&CriterionResult> = c.iter().filter(|x| x.level == Some(l)).collect();
        if at.is_empty() {
            // a level with no criterion of its own is carried by the levels below
            if c.iter().any(|x| x.level.is_some_and(|y| y > l)) {
                lvl = l;
                continue;
            }
            break;
        }
        if at.iter().all(|x| x.verdict == Verdict::Met) {
            lvl = l;
        } else {
            break;
        }
    }
    Some(lvl)
}
