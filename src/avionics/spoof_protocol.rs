// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! Spoofing-detection output verification: procedures of kind `spoof-detection` (research
//! 2026-11-20 gaps, B1208, test protocol only).
//!
//! The procedure states the stimulus timeline the bench plays (its spoofing scenarios come from
//! the open environment library and the bench; nothing here generates a signal) and the
//! equipment log's `alert` flag is checked against it:
//!
//! * **precondition**: no alert before the first event;
//! * **detection**, per event: the first alert at or after the event start, within the
//!   applicant's `detection_time_s`;
//! * **clearance**, per event, when the procedure declares `clear_time_s`: the alert cleared
//!   within it after the event ends (recorded as not assessed when the limit is not supplied);
//! * **false alerts**: alert epochs in the clean segments (outside every event and its
//!   `settle_s` after it), against the applicant's `max_false_alert_s` (seconds of alert allowed).
//!
//! The protocol verifies the equipment's output; it contains no detection or mitigation
//! algorithm. The DFMC aided-inertial test protocol is not built here (its MOPS was not read).
//! MODELLED: a test structure; limits are the applicant's.

use serde::{Deserialize, Serialize};

use super::procedure::{Check, DutLog, Params};
use crate::scorecards::levels::Verdict;

/// Stimulus labels the bench may play (the scenario content is the bench's).
pub const KINDS: [&str; 6] = [
    "meaconing",
    "time-push",
    "position-drag",
    "matched-power-takeover",
    "jamming-then-spoofing",
    "other",
];

/// One stimulus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub t_s: f64,
    pub duration_s: f64,
    /// The bench scenario that plays it (free text, e.g. a library scenario id).
    #[serde(default)]
    pub scenario: String,
}

/// The `[spoof]` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpoofSpec {
    pub duration_s: u32,
    /// Seconds after an event that are not counted as clean.
    #[serde(default = "d_settle")]
    pub settle_s: f64,
    #[serde(default, rename = "event")]
    pub events: Vec<Event>,
}

fn d_settle() -> f64 {
    30.0
}

/// The limits a spoof-detection procedure uses.
pub const PARAMS: [&str; 3] = ["detection_time_s", "clear_time_s", "max_false_alert_s"];

impl SpoofSpec {
    /// Check the timeline.
    pub fn validate(&self) -> Result<(), String> {
        if self.duration_s == 0 || self.events.is_empty() {
            return Err("spoof needs a positive duration_s and at least one event".into());
        }
        if self.settle_s.is_nan() || self.settle_s < 0.0 {
            return Err("spoof.settle_s must be zero or positive".into());
        }
        let mut ev: Vec<&Event> = self.events.iter().collect();
        ev.sort_by(|a, b| a.t_s.total_cmp(&b.t_s));
        for (i, e) in ev.iter().enumerate() {
            if !KINDS.contains(&e.kind.as_str()) {
                return Err(format!(
                    "spoof event '{}': kind '{}' (expected {})",
                    e.id,
                    e.kind,
                    KINDS.join(", ")
                ));
            }
            if !(e.t_s > 0.0
                && e.duration_s > 0.0
                && e.t_s + e.duration_s <= self.duration_s as f64)
            {
                return Err(format!("spoof event '{}': needs 0 < t_s, a positive duration and to end within duration_s", e.id));
            }
            if i > 0 && e.t_s < ev[i - 1].t_s + ev[i - 1].duration_s {
                return Err(format!("spoof event '{}' overlaps the one before", e.id));
            }
        }
        Ok(())
    }
}

fn check(
    id: String,
    subject: String,
    p: &str,
    params: &Params,
    measured: Option<f64>,
    v: Verdict,
    detail: String,
) -> Check {
    let l = params.limit(p);
    Check {
        id,
        kind: if p.is_empty() {
            "precondition".into()
        } else {
            "expectation".into()
        },
        subject,
        limit_param: p.into(),
        limit: l.value,
        limit_source: l.source,
        measured,
        verdict: v,
        detail,
    }
}

/// The checks of a spoof-detection procedure against an equipment log.
pub fn checks(spec: &SpoofSpec, params: &Params, log: Option<&DutLog>) -> Vec<Check> {
    let Some(log) = log else {
        return spec
            .events
            .iter()
            .map(|e| {
                check(
                    format!("{}-detected", e.id),
                    e.kind.clone(),
                    "detection_time_s",
                    params,
                    None,
                    Verdict::NotAssessed,
                    "no equipment log supplied".into(),
                )
            })
            .collect();
    };
    let alert_at = |t0: f64, t1: f64, want: bool| {
        log.epochs
            .iter()
            .filter(|e| e.t_s >= t0 && e.t_s < t1)
            .find(|e| e.alert == Some(want))
            .map(|e| e.t_s)
    };
    let mut out = Vec::new();
    let first = spec
        .events
        .iter()
        .map(|e| e.t_s)
        .fold(f64::INFINITY, f64::min);
    let pre: Vec<_> = log
        .epochs
        .iter()
        .filter(|e| e.t_s < first && e.alert.is_some())
        .collect();
    let pre_ok = (!pre.is_empty()).then(|| pre.iter().all(|e| e.alert == Some(false)));
    out.push(check(
        "precondition-no-alert".into(),
        "no alert before the first event".into(),
        "",
        params,
        None,
        Verdict::from_bool(pre_ok),
        match pre_ok {
            None => "no alert flag logged before the first event".into(),
            Some(true) => "no alert before the first event".into(),
            Some(false) => "an alert was raised before any stimulus".into(),
        },
    ));
    for e in &spec.events {
        let end = e.t_s + e.duration_s;
        let lim = params.value("detection_time_s");
        let first_alert = alert_at(e.t_s, end, true);
        let lat = first_alert.map(|t| t - e.t_s);
        let v = match (lim, lat) {
            (None, _) => Verdict::NotAssessed,
            (Some(_), None) => Verdict::NotMet,
            (Some(l), Some(x)) => Verdict::from_bool(Some(x <= l)),
        };
        out.push(check(
            format!("{}-detected", e.id),
            format!("{} ({})", e.kind, e.scenario),
            "detection_time_s",
            params,
            lat,
            v,
            match lat {
                Some(x) => format!("first alert {x:.1} s after the stimulus started"),
                None => "no alert while the stimulus lasted".into(),
            },
        ));
        let cl = params.value("clear_time_s");
        let next = spec
            .events
            .iter()
            .filter(|o| o.t_s >= end)
            .map(|o| o.t_s)
            .fold(spec.duration_s as f64, f64::min);
        let cleared = alert_at(end, next, false).map(|t| t - end);
        out.push(check(
            format!("{}-cleared", e.id),
            format!("{} ({})", e.kind, e.scenario),
            "clear_time_s",
            params,
            cleared,
            match (cl, cleared) {
                (None, _) => Verdict::NotAssessed,
                (Some(_), None) => Verdict::NotMet,
                (Some(l), Some(x)) => Verdict::from_bool(Some(x <= l)),
            },
            match cleared {
                Some(x) => format!("alert cleared {x:.1} s after the stimulus ended"),
                None => "the alert did not clear before the next stimulus or the end".into(),
            },
        ));
    }
    // False alerts in clean segments.
    let clean = |t: f64| {
        t >= first
            && !spec
                .events
                .iter()
                .any(|e| t >= e.t_s && t < e.t_s + e.duration_s + spec.settle_s)
    };
    let dt = if log.epochs.len() > 1 {
        (log.epochs[log.epochs.len() - 1].t_s - log.epochs[0].t_s) / (log.epochs.len() - 1) as f64
    } else {
        1.0
    };
    let fa: Vec<f64> = log
        .epochs
        .iter()
        .filter(|e| clean(e.t_s) && e.alert == Some(true))
        .map(|e| e.t_s)
        .collect();
    let fa_s = fa.len() as f64 * dt;
    let lim = params.value("max_false_alert_s");
    out.push(check(
        "false-alerts".into(),
        "alert in clean segments".into(),
        "max_false_alert_s",
        params,
        Some(fa_s),
        match lim {
            None => Verdict::NotAssessed,
            Some(l) => Verdict::from_bool(Some(fa_s <= l)),
        },
        if fa.is_empty() {
            "no alert in the clean segments".into()
        } else {
            format!(
                "{fa_s:.0} s of alert in clean segments, first at t = {}",
                fa[0]
            )
        },
    ));
    out
}

/// A SYNTHETIC log: `good` alerts 2 s into each event and clears 2 s after it; otherwise it
/// never alerts during events and alerts once in a clean segment.
pub fn synth_epochs(spec: &SpoofSpec, good: bool) -> Vec<super::procedure::DutEpoch> {
    (0..spec.duration_s)
        .map(|t| {
            let t = t as f64;
            let in_ev = spec
                .events
                .iter()
                .any(|e| t >= e.t_s + 2.0 && t < e.t_s + e.duration_s + 2.0);
            let alert = if good {
                in_ev
            } else {
                t == spec.duration_s as f64 - 1.0
            };
            super::procedure::DutEpoch {
                t_s: t,
                mode: Some("gnss-only".into()),
                used_prns: None,
                hpl_m: None,
                vpl_m: None,
                alert: Some(alert),
            }
        })
        .collect()
}
