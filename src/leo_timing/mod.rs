// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # LEO timing verification (research 2026-11-20 gaps, E21 B2102)
//!
//! A pre-registered scorecard for timing receivers that take time from a low-Earth-orbit
//! (LEO) service, scored on the receiver's own field logs (indoor, urban, sea, open sky):
//!
//! * **Pre-registration.** The plan's method part (thresholds, claims, replay window) is
//!   hashed ([`method_sha256`]) and may be stated in the plan as `prereg_sha256` before any log
//!   is taken. The scorecard says whether the hash matches; a method changed after the fact
//!   shows.
//! * **Per environment.** Availability of a timing solution, RMS, 95th percentile and maximum
//!   absolute time error against the log's reference, and the MTIE and TDEV curves of the open
//!   engine (`kshana::telecom_timing`), with the maximum absolute time error set against the
//!   ITU-T G.8272 PRTC-A limit (100 ns) as a measured comparison, never a statement of
//!   conformance.
//! * **Claims.** Each stated vendor figure (metric, environment, value, source) is set against
//!   the measured figure: `reproduced`, `not-reproduced` or `not-evaluated`.
//! * **Authentication and replay window.** From an authentication log (receive time, signed
//!   transmit time, accepted or not), the messages accepted outside the stated window (which
//!   should have been refused), negative latencies and the largest latency accepted.
//!
//! **Evidence class.** `VALIDATED` only when every log is a real field log (`synthetic =
//! false`) and the pre-registration hash matches; otherwise `MODELLED`, with the reason.
//! Results are test evidence; a scorecard never says a receiver or a service is compliant,
//! conformant or certified.

pub mod synth;

use std::path::Path;

use kshana::telecom_timing::{mask_by_id, mtie_curve_ns, tdev_curve_ns, CurvePoint};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::aviation::Inputs;

/// `leo-timing score` (Pro).
pub const FEATURE: &str = "pro-leo-timing-scorecard";
/// The format of a scorecard.
pub const FORMAT: &str = "kshana-pro-leo-timing-scorecard/1";
/// The method; the rules above are constants of this version.
pub const METHOD_ID: &str = "kshana-pro/leo-timing-scorecard/v1";
/// The telecom-timing mask the maximum time error is set against.
pub const REFERENCE_MASK: &str = "prtc-a";

/// Thresholds stated before scoring.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thresholds {
    /// Largest acceptable absolute time error (ns).
    pub max_abs_te_ns: f64,
    /// Smallest acceptable availability of a timing solution (0 to 1).
    pub availability_min: f64,
}

/// A stated claim to replicate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    pub id: String,
    /// `rms_te_ns`, `p95_abs_te_ns`, `max_abs_te_ns` or `availability`.
    pub metric: String,
    /// The environment the claim is about (as the logs label it).
    pub environment: String,
    /// The claimed value.
    pub value: f64,
    /// Where the claim was published.
    pub source: String,
}

/// One time-error log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogRef {
    /// CSV with `t_s,te_ns,valid` (time error against the reference; valid 1 when the receiver
    /// reported a timing solution).
    pub file: String,
    /// `indoor`, `urban`, `sea`, `open` or another label.
    pub environment: String,
    /// Whether the log is synthetic (made by `leo-timing synth` or a simulation).
    pub synthetic: bool,
}

/// The authentication log and the replay window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRef {
    /// CSV with `rx_s,tx_s,accepted` (receive time, authenticated transmit time, 1 accepted).
    pub file: String,
    /// Acceptance window stated by the receiver or the service (s).
    pub window_s: f64,
    /// The time scale both columns are on (e.g. `UTC`, `GPST`), as the operator states it.
    #[serde(default)]
    pub time_scale: Option<String>,
    /// When the columns are on different scales: the transmit scale minus the receive scale (s),
    /// e.g. 18 for transmit times in GPST and receive times in UTC. Subtracted from `tx_s`.
    #[serde(default)]
    pub tx_minus_rx_scale_s: f64,
    /// Whether the log is synthetic.
    pub synthetic: bool,
}

/// The plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub receiver: String,
    pub service: String,
    pub thresholds: Thresholds,
    #[serde(default)]
    pub claim: Vec<Claim>,
    #[serde(default)]
    pub replay_window_s: Option<f64>,
    /// The method hash registered before the logs were taken.
    #[serde(default)]
    pub prereg_sha256: Option<String>,
    #[serde(default)]
    pub log: Vec<LogRef>,
    #[serde(default)]
    pub replay: Option<ReplayRef>,
}

#[derive(Serialize)]
struct MethodPart<'a> {
    method: &'a str,
    thresholds: &'a Thresholds,
    claims: &'a [Claim],
    replay_window_s: Option<f64>,
}

/// SHA-256 (hex) of the method part of a plan: the method id, thresholds, claims and replay
/// window, as canonical JSON. Logs are not part of it.
pub fn method_sha256(p: &Plan) -> String {
    let m = MethodPart {
        method: METHOD_ID,
        thresholds: &p.thresholds,
        claims: &p.claim,
        replay_window_s: p.replay_window_s,
    };
    let bytes = serde_json::to_vec(&m).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

/// One parsed time-error record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeSample {
    pub t_s: f64,
    pub te_ns: f64,
    pub valid: bool,
}

fn header_pos(head: &str, names: &[&str]) -> Result<Vec<usize>, String> {
    let cols: Vec<String> = head.split(',').map(|c| c.trim().to_lowercase()).collect();
    names
        .iter()
        .map(|n| {
            cols.iter()
                .position(|c| c == n)
                .ok_or_else(|| format!("CSV header lacks column {n}"))
        })
        .collect()
}

fn flag01(s: &str) -> Option<bool> {
    match s.trim() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

/// Parse a `t_s,te_ns,valid` log; rows that do not parse are counted, not used.
pub fn parse_te(text: &str) -> Result<(Vec<TeSample>, usize), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head = lines.next().ok_or("empty time-error log")?;
    let ix = header_pos(head, &["t_s", "te_ns", "valid"])?;
    let (mut out, mut bad) = (Vec::new(), 0usize);
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let get = |i: usize| f.get(i).map(|s| s.trim()).unwrap_or("");
        match (
            get(ix[0]).parse::<f64>(),
            get(ix[1]).parse::<f64>(),
            flag01(get(ix[2])),
        ) {
            (Ok(t), Ok(te), Some(v)) if t.is_finite() && te.is_finite() => out.push(TeSample {
                t_s: t,
                te_ns: te,
                valid: v,
            }),
            _ => bad += 1,
        }
    }
    if out.len() < 2 {
        return Err("a time-error log needs at least two rows".into());
    }
    if out.windows(2).any(|w| w[1].t_s <= w[0].t_s) {
        return Err("time-error log times must increase".into());
    }
    Ok((out, bad))
}

/// Measured figures of one environment.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EnvResult {
    pub environment: String,
    pub logs: usize,
    pub samples: usize,
    pub rows_rejected: usize,
    pub availability: f64,
    pub rms_te_ns: Option<f64>,
    pub p95_abs_te_ns: Option<f64>,
    pub max_abs_te_ns: Option<f64>,
    /// `within` or `exceeds` the stated threshold; `no-solution` without a valid sample.
    pub threshold_check: &'static str,
    pub availability_check: &'static str,
    /// The PRTC-A maximum absolute time error limit (ns) and whether the measured maximum is
    /// within it (a measured comparison, not a conformance statement).
    pub prtc_a_limit_ns: Option<f64>,
    pub within_prtc_a_limit: Option<bool>,
    /// MTIE and TDEV of the first log's valid samples, when they are uniformly spaced.
    pub mtie_ns: Vec<CurvePoint>,
    pub tdev_ns: Vec<CurvePoint>,
}

/// One claim's outcome.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClaimResult {
    pub id: String,
    pub metric: String,
    pub environment: String,
    pub claimed: f64,
    pub measured: Option<f64>,
    /// `reproduced`, `not-reproduced` or `not-evaluated`.
    pub outcome: &'static str,
    pub source: String,
}

/// The replay-window check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ReplayResult {
    pub window_s: f64,
    pub messages: usize,
    pub accepted: usize,
    /// Accepted although older than the window: each one is a replay the receiver let in.
    pub accepted_outside_window: usize,
    /// Accepted with a receive time before the transmit time.
    pub accepted_negative_latency: usize,
    /// Refused although inside the window.
    pub refused_inside_window: usize,
    pub max_accepted_latency_s: Option<f64>,
    /// `held` when nothing was accepted outside the window, else `breached`.
    pub outcome: &'static str,
    pub synthetic: bool,
    /// The time scales of the two columns: declared, or assumed common.
    pub time_scale: String,
    /// Transmit scale minus receive scale applied (s).
    pub tx_minus_rx_scale_s: f64,
    /// Set when the latencies sit near a whole leap-second scale offset (18, 19 or 37 s), the
    /// mark of receive and transmit times on different scales.
    pub scale_warning: Option<String>,
}

/// The scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Scorecard {
    pub format: &'static str,
    pub method: &'static str,
    pub receiver: String,
    pub service: String,
    pub method_sha256: String,
    pub prereg_sha256: Option<String>,
    pub preregistered: bool,
    pub all_logs_real: bool,
    /// `VALIDATED` or `MODELLED`.
    pub evidence_class: &'static str,
    pub evidence_reason: String,
    pub thresholds: Thresholds,
    pub environments: Vec<EnvResult>,
    pub claims: Vec<ClaimResult>,
    pub replay: Option<ReplayResult>,
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
    let k = ((sorted.len() as f64 * q).ceil() as usize).clamp(1, sorted.len());
    sorted[k - 1]
}

fn uniform_dt(s: &[TeSample]) -> Option<f64> {
    let dt = s.get(1)?.t_s - s.first()?.t_s;
    let ok = s
        .windows(2)
        .all(|w| ((w[1].t_s - w[0].t_s) - dt).abs() <= 1e-6 * dt.max(1.0));
    (ok && dt > 0.0).then_some(dt)
}

/// Score one environment from its logs.
pub fn score_env(env: &str, logs: &[(Vec<TeSample>, usize)], th: &Thresholds) -> EnvResult {
    let all: Vec<&TeSample> = logs.iter().flat_map(|(s, _)| s.iter()).collect();
    let valid: Vec<f64> = all.iter().filter(|s| s.valid).map(|s| s.te_ns).collect();
    let mut abs: Vec<f64> = valid.iter().map(|v| v.abs()).collect();
    abs.sort_by(|a, b| a.total_cmp(b));
    let availability = valid.len() as f64 / all.len().max(1) as f64;
    let rms = (!valid.is_empty())
        .then(|| (valid.iter().map(|v| v * v).sum::<f64>() / valid.len() as f64).sqrt());
    let p95 = (!abs.is_empty()).then(|| percentile(&abs, 0.95));
    let max = abs.last().copied();
    let limit = mask_by_id(REFERENCE_MASK)
        .and_then(|m| m.max_abs_te)
        .map(|l| l.value_ns);
    let (mtie, tdev) = match logs.first() {
        Some((s, _)) => {
            let v: Vec<TeSample> = s.iter().copied().filter(|x| x.valid).collect();
            match uniform_dt(&v) {
                Some(dt) if v.len() == s.len() => {
                    let te: Vec<f64> = v.iter().map(|x| x.te_ns).collect();
                    (mtie_curve_ns(&te, dt), tdev_curve_ns(&te, dt))
                }
                _ => (Vec::new(), Vec::new()),
            }
        }
        None => (Vec::new(), Vec::new()),
    };
    EnvResult {
        environment: env.to_string(),
        logs: logs.len(),
        samples: all.len(),
        rows_rejected: logs.iter().map(|(_, b)| b).sum(),
        availability,
        rms_te_ns: rms,
        p95_abs_te_ns: p95,
        max_abs_te_ns: max,
        threshold_check: match max {
            None => "no-solution",
            Some(m) if m <= th.max_abs_te_ns => "within",
            Some(_) => "exceeds",
        },
        availability_check: if availability >= th.availability_min {
            "within"
        } else {
            "below"
        },
        prtc_a_limit_ns: limit,
        within_prtc_a_limit: match (max, limit) {
            (Some(m), Some(l)) => Some(m <= l),
            _ => None,
        },
        mtie_ns: mtie,
        tdev_ns: tdev,
    }
}

fn claim_outcome(c: &Claim, envs: &[EnvResult]) -> ClaimResult {
    let e = envs.iter().find(|e| e.environment == c.environment);
    let (measured, lower_is_better) = match (c.metric.as_str(), e) {
        ("rms_te_ns", Some(e)) => (e.rms_te_ns, true),
        ("p95_abs_te_ns", Some(e)) => (e.p95_abs_te_ns, true),
        ("max_abs_te_ns", Some(e)) => (e.max_abs_te_ns, true),
        ("availability", Some(e)) => (Some(e.availability), false),
        _ => (None, true),
    };
    let outcome = match measured {
        None => "not-evaluated",
        Some(m) if (lower_is_better && m <= c.value) || (!lower_is_better && m >= c.value) => {
            "reproduced"
        }
        Some(_) => "not-reproduced",
    };
    ClaimResult {
        id: c.id.clone(),
        metric: c.metric.clone(),
        environment: c.environment.clone(),
        claimed: c.value,
        measured,
        outcome,
        source: c.source.clone(),
    }
}

/// Check an authentication log against a replay window, both columns taken as one time scale
/// (not declared).
pub fn replay_check(text: &str, window_s: f64, synthetic: bool) -> Result<ReplayResult, String> {
    replay_check_scaled(text, window_s, synthetic, None, 0.0)
}

/// Check an authentication log against a replay window. Latency is `rx − (tx − d)` with `d`
/// the transmit scale minus the receive scale: both columns must be on one time scale, or `d`
/// must be stated, or a leap-second offset (18 s GPST−UTC, 37 s TAI−UTC) turns into false
/// breaches or hides real ones.
pub fn replay_check_scaled(
    text: &str,
    window_s: f64,
    synthetic: bool,
    time_scale: Option<&str>,
    tx_minus_rx_scale_s: f64,
) -> Result<ReplayResult, String> {
    if !tx_minus_rx_scale_s.is_finite() {
        return Err("tx_minus_rx_scale_s must be a number".into());
    }
    if !(window_s.is_finite() && window_s > 0.0) {
        return Err("the replay window must be a positive number of seconds".into());
    }
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let head = lines.next().ok_or("empty authentication log")?;
    let ix = header_pos(head, &["rx_s", "tx_s", "accepted"])?;
    let mut r = ReplayResult {
        window_s,
        messages: 0,
        accepted: 0,
        accepted_outside_window: 0,
        accepted_negative_latency: 0,
        refused_inside_window: 0,
        max_accepted_latency_s: None,
        outcome: "held",
        synthetic,
        time_scale: match (time_scale, tx_minus_rx_scale_s != 0.0) {
            (Some(s), false) => format!("{s} (declared, both columns)"),
            (Some(s), true) => {
                format!("{s} (declared), transmit times moved by {tx_minus_rx_scale_s} s")
            }
            (None, false) => "not declared: both columns ASSUMED on one time scale".into(),
            (None, true) => {
                format!("not declared; transmit times moved by {tx_minus_rx_scale_s} s")
            }
        },
        tx_minus_rx_scale_s,
        scale_warning: None,
    };
    let mut lats = Vec::new();
    for l in lines {
        let f: Vec<&str> = l.split(',').collect();
        let get = |i: usize| f.get(i).map(|s| s.trim()).unwrap_or("");
        let (Ok(rx), Ok(tx), Some(acc)) = (
            get(ix[0]).parse::<f64>(),
            get(ix[1]).parse::<f64>(),
            flag01(get(ix[2])),
        ) else {
            continue;
        };
        if !(rx.is_finite() && tx.is_finite()) {
            continue;
        }
        r.messages += 1;
        let lat = rx - (tx - tx_minus_rx_scale_s);
        lats.push(lat);
        let inside = (0.0..=window_s).contains(&lat);
        if acc {
            r.accepted += 1;
            if lat > window_s {
                r.accepted_outside_window += 1;
            }
            if lat < 0.0 {
                r.accepted_negative_latency += 1;
            }
            r.max_accepted_latency_s = Some(r.max_accepted_latency_s.map_or(lat, |m| m.max(lat)));
        } else if inside {
            r.refused_inside_window += 1;
        }
    }
    if r.accepted_outside_window > 0 || r.accepted_negative_latency > 0 {
        r.outcome = "breached";
    }
    if !lats.is_empty() {
        lats.sort_by(f64::total_cmp);
        let med = lats[lats.len() / 2];
        if let Some(k) = [18.0, -18.0, 19.0, -19.0, 37.0, -37.0]
            .iter()
            .find(|k| (med - *k).abs() < 1.0 && window_s < 15.0)
        {
            r.scale_warning = Some(format!(
                "the median latency is {med:.3} s, within 1 s of {k} s: receive and transmit times look like they are on different time scales; state tx_minus_rx_scale_s"
            ));
        }
    }
    Ok(r)
}

/// A plan with its logs read.
pub struct Loaded {
    pub plan: Plan,
    pub logs: Vec<(String, Vec<TeSample>, usize)>,
    pub replay_text: Option<String>,
    pub inputs: Inputs,
}

/// Read a plan and every file it names (relative to the plan's directory).
pub fn load(path: &Path) -> Result<Loaded, String> {
    let mut inputs = Inputs::default();
    let base = path.parent().unwrap_or(Path::new("."));
    let text = inputs.read(
        Path::new("."),
        path.to_str().ok_or("plan path is not UTF-8")?,
        "plan",
    )?;
    let plan: Plan = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    validate(&plan)?;
    let mut logs = Vec::new();
    for l in &plan.log {
        let t = inputs.read(base, &l.file, "time-error-log")?;
        let (s, bad) = parse_te(&t).map_err(|e| format!("{}: {e}", l.file))?;
        logs.push((l.environment.clone(), s, bad));
    }
    let replay_text = match &plan.replay {
        Some(r) => Some(inputs.read(base, &r.file, "authentication-log")?),
        None => None,
    };
    Ok(Loaded {
        plan,
        logs,
        replay_text,
        inputs,
    })
}

fn validate(p: &Plan) -> Result<(), String> {
    let t = &p.thresholds;
    if !(t.max_abs_te_ns.is_finite() && t.max_abs_te_ns > 0.0) {
        return Err("thresholds.max_abs_te_ns must be positive".into());
    }
    if !(0.0..=1.0).contains(&t.availability_min) {
        return Err("thresholds.availability_min must be in [0, 1]".into());
    }
    for c in &p.claim {
        if !matches!(
            c.metric.as_str(),
            "rms_te_ns" | "p95_abs_te_ns" | "max_abs_te_ns" | "availability"
        ) {
            return Err(format!("claim {}: unknown metric {}", c.id, c.metric));
        }
        if !c.value.is_finite() {
            return Err(format!("claim {}: value must be finite", c.id));
        }
    }
    if p.log.is_empty() {
        return Err("the plan names no [[log]]".into());
    }
    if let (Some(r), Some(w)) = (&p.replay, p.replay_window_s) {
        if (r.window_s - w).abs() > 1e-12 {
            return Err("replay.window_s differs from the registered replay_window_s".into());
        }
    }
    Ok(())
}

/// Score a loaded plan.
pub fn score(l: &Loaded) -> Result<Scorecard, String> {
    let p = &l.plan;
    let mut envs: Vec<String> = Vec::new();
    for (e, _, _) in &l.logs {
        if !envs.contains(e) {
            envs.push(e.clone());
        }
    }
    let environments: Vec<EnvResult> = envs
        .iter()
        .map(|e| {
            let logs: Vec<(Vec<TeSample>, usize)> = l
                .logs
                .iter()
                .filter(|(x, _, _)| x == e)
                .map(|(_, s, b)| (s.clone(), *b))
                .collect();
            score_env(e, &logs, &p.thresholds)
        })
        .collect();
    let claims = p
        .claim
        .iter()
        .map(|c| claim_outcome(c, &environments))
        .collect();
    let replay = match (&p.replay, &l.replay_text) {
        (Some(r), Some(t)) => Some(replay_check_scaled(
            t,
            r.window_s,
            r.synthetic,
            r.time_scale.as_deref(),
            r.tx_minus_rx_scale_s,
        )?),
        _ => None,
    };
    let hash = method_sha256(p);
    let preregistered = p
        .prereg_sha256
        .as_deref()
        .is_some_and(|h| h.eq_ignore_ascii_case(&hash));
    let all_real =
        p.log.iter().all(|x| !x.synthetic) && p.replay.as_ref().is_none_or(|r| !r.synthetic);
    let (class, reason) = match (all_real, preregistered) {
        (true, true) => (
            "VALIDATED",
            "real field logs scored by a pre-registered method".to_string(),
        ),
        (false, _) => ("MODELLED", "at least one log is synthetic".to_string()),
        (true, false) => (
            "MODELLED",
            "the method hash was not registered before scoring, or does not match".to_string(),
        ),
    };
    Ok(Scorecard {
        format: FORMAT,
        method: METHOD_ID,
        receiver: p.receiver.clone(),
        service: p.service.clone(),
        method_sha256: hash,
        prereg_sha256: p.prereg_sha256.clone(),
        preregistered,
        all_logs_real: all_real,
        evidence_class: class,
        evidence_reason: reason,
        thresholds: p.thresholds.clone(),
        environments,
        claims,
        replay,
    })
}

/// The `summary.txt` of a result.
pub fn summary(s: &Scorecard) -> String {
    let mut out = format!(
        "[leo-timing score] {} on {}: evidence class {} ({})\n",
        s.receiver, s.service, s.evidence_class, s.evidence_reason
    );
    for e in &s.environments {
        out += &format!(
            "  {}: availability {:.4}, max |TE| {} ns ({}), PRTC-A limit {}\n",
            e.environment,
            e.availability,
            e.max_abs_te_ns.map_or("-".into(), |v| format!("{v:.1}")),
            e.threshold_check,
            match e.within_prtc_a_limit {
                Some(true) => "within",
                Some(false) => "exceeded",
                None => "not evaluated",
            }
        );
    }
    for c in &s.claims {
        out += &format!("  claim {}: {}\n", c.id, c.outcome);
    }
    if let Some(r) = &s.replay {
        out += &format!(
            "  replay window {} s: {} ({} accepted outside)\n",
            r.window_s, r.outcome, r.accepted_outside_window
        );
    }
    out += "Test evidence only: not a statement that a receiver or service is compliant, conformant or certified.\n";
    out
}
