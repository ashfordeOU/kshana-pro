// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Resilient-time civil interface tests (`kshana-pro altpnt time-iface`).
//!
//! A resilient-time service (for example a national resilient-time programme's civil output)
//! delivers time at a civil interface (PTP, NTP, 1PPS) and declares its state (`locked`,
//! `holdover`, `freerun`, `recovering`) and, often, the accuracy it claims at each moment. The
//! tester measures the delivered time against an independent reference and logs, once per
//! epoch, `t_s,offset_ns,state[,declared_accuracy_ns]`. This module tests that log against the
//! limits the service declares. It reads measurements; it disciplines no clock and steers
//! nothing.
//!
//! Tests (each `WITHIN-LIMIT`, `EXCEEDS-LIMIT` or `NOT-TESTED`):
//!
//! 1. `locked-accuracy`: every epoch declared `locked` has `|offset| <= locked_ns`;
//! 2. `declared-quality`: no epoch's `|offset|` exceeds the accuracy it declared (an
//!    overstated quality is misleading to every client downstream);
//! 3. `holdover-growth`: in every holdover episode `|offset|` stays under the declared
//!    holdover profile (linear between its points, flat after the last);
//! 4. `recovery`: after every holdover or free-run episode the service is `locked` and within
//!    `locked_ns` again within `max_recovery_s`;
//! 5. `continuity`: no gap over `max_gap_s` and no time running backwards;
//! 6. `mtie-locked`: MTIE of the longest locked stretch under the declared mask
//!    (`crate::timing::compliance::mtie`).
//!
//! A result is test evidence for the log supplied under the limits stated; it is not a
//! statement that the service meets any standard.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{rnd, Inputs, EVIDENCE, SCOPE};

/// The result format.
pub const FORMAT: &str = "kshana-pro-altpnt-time-iface/1";

/// The declared limits.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub locked_ns: f64,
    /// Holdover profile: `[elapsed_s, max_abs_offset_ns]` points, increasing in time.
    #[serde(default)]
    pub holdover_profile: Vec<[f64; 2]>,
    #[serde(default)]
    pub max_recovery_s: Option<f64>,
    #[serde(default = "default_gap")]
    pub max_gap_s: f64,
    /// MTIE mask in the locked state: `[tau_s, max_ns]` points.
    #[serde(default)]
    pub mtie_mask: Vec<[f64; 2]>,
}

fn default_gap() -> f64 {
    2.0
}

/// The request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub service: String,
    /// PTP, NTP, 1PPS or other.
    pub interface: String,
    /// How the reference was obtained (stated, not checked).
    #[serde(default)]
    pub reference: String,
    pub log: String,
    pub limits: Limits,
}

/// One logged epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct Epoch {
    pub t: f64,
    pub offset_ns: f64,
    pub state: State,
    pub declared_ns: Option<f64>,
}

/// The service's declared state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Locked,
    Holdover,
    Freerun,
    Recovering,
}

/// Parse a log: (epochs in time order as given, rows skipped).
pub fn parse_log(text: &str) -> Result<(Vec<Epoch>, usize), String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("empty log")?
        .split(',')
        .map(|h| h.trim().to_lowercase())
        .collect();
    let idx = |n: &str| head.iter().position(|h| h == n);
    let (Some(it), Some(io), Some(is)) = (idx("t_s"), idx("offset_ns"), idx("state")) else {
        return Err("log header needs t_s, offset_ns and state".into());
    };
    let id = idx("declared_accuracy_ns");
    let (mut out, mut bad) = (Vec::new(), 0);
    for l in lines {
        let c: Vec<&str> = l.split(',').map(str::trim).collect();
        let num = |i: usize| {
            c.get(i)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        let st = match c.get(is).map(|s| s.to_lowercase()) {
            Some(s) if s == "locked" => Some(State::Locked),
            Some(s) if s == "holdover" => Some(State::Holdover),
            Some(s) if s == "freerun" || s == "free-run" => Some(State::Freerun),
            Some(s) if s == "recovering" => Some(State::Recovering),
            _ => None,
        };
        match (num(it), num(io), st) {
            (Some(t), Some(o), Some(state)) => out.push(Epoch {
                t,
                offset_ns: o,
                state,
                declared_ns: id.and_then(num).filter(|v| *v >= 0.0),
            }),
            _ => bad += 1,
        }
    }
    if out.is_empty() {
        return Err("log has no valid epoch".into());
    }
    Ok((out, bad))
}

/// One test's result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub id: &'static str,
    /// WITHIN-LIMIT, EXCEEDS-LIMIT or NOT-TESTED.
    pub verdict: &'static str,
    pub epochs_tested: usize,
    pub exceedances: usize,
    pub worst: Option<f64>,
    pub detail: String,
}

fn verdict(tested: bool, exceed: usize) -> &'static str {
    match (tested, exceed) {
        (false, _) => "NOT-TESTED",
        (true, 0) => "WITHIN-LIMIT",
        _ => "EXCEEDS-LIMIT",
    }
}

/// The holdover profile's limit at `elapsed` s (None without a profile).
pub fn profile_at(p: &[[f64; 2]], elapsed: f64) -> Option<f64> {
    let first = p.first()?;
    if elapsed <= first[0] {
        return Some(first[1]);
    }
    for w in p.windows(2) {
        let ([a, va], [b, vb]) = (w[0], w[1]);
        if elapsed <= b {
            return Some(va + (vb - va) * (elapsed - a) / (b - a));
        }
    }
    p.last().map(|x| x[1])
}

/// The test report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub format: &'static str,
    pub evidence: &'static str,
    pub title: String,
    pub service: String,
    pub interface: String,
    pub reference: String,
    pub limits: Limits,
    pub epochs: usize,
    pub skipped_rows: usize,
    pub span_s: f64,
    pub state_epochs: std::collections::BTreeMap<String, usize>,
    pub tests: Vec<TestResult>,
    pub scope: &'static str,
}

fn check_limits(l: &Limits) -> Result<(), String> {
    let inc = |p: &[[f64; 2]]| {
        p.iter()
            .all(|x| x[0].is_finite() && x[1].is_finite() && x[0] >= 0.0 && x[1] >= 0.0)
            && p.windows(2).all(|w| w[1][0] > w[0][0])
    };
    if !(l.locked_ns.is_finite() && l.locked_ns > 0.0 && l.max_gap_s > 0.0) {
        return Err("limits: locked_ns and max_gap_s must be > 0".into());
    }
    if !inc(&l.holdover_profile) || !inc(&l.mtie_mask) {
        return Err(
            "limits: holdover_profile and mtie_mask need non-negative points in increasing time"
                .into(),
        );
    }
    Ok(())
}

/// Run every test on a log.
pub fn test(req: &Request, log: &[Epoch], skipped: usize) -> Result<Report, String> {
    check_limits(&req.limits)?;
    let l = &req.limits;
    let mut tests = Vec::new();

    // 1. Locked accuracy.
    let locked: Vec<&Epoch> = log.iter().filter(|e| e.state == State::Locked).collect();
    let ex = locked
        .iter()
        .filter(|e| e.offset_ns.abs() > l.locked_ns)
        .count();
    let worst = locked
        .iter()
        .map(|e| e.offset_ns.abs())
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v))));
    tests.push(TestResult {
        id: "locked-accuracy",
        verdict: verdict(!locked.is_empty(), ex),
        epochs_tested: locked.len(),
        exceedances: ex,
        worst: worst.map(|w| rnd(w, 3)),
        detail: format!("|offset| <= {} ns in the locked state", l.locked_ns),
    });

    // 2. Declared quality.
    let decl: Vec<&Epoch> = log.iter().filter(|e| e.declared_ns.is_some()).collect();
    let over: Vec<f64> = decl
        .iter()
        .filter_map(|e| {
            let d = e.declared_ns.unwrap_or(f64::INFINITY);
            (e.offset_ns.abs() > d).then_some(e.offset_ns.abs() - d)
        })
        .collect();
    tests.push(TestResult {
        id: "declared-quality",
        verdict: verdict(!decl.is_empty(), over.len()),
        epochs_tested: decl.len(),
        exceedances: over.len(),
        worst: over.iter().cloned().fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.max(v)))).map(|w| rnd(w, 3)),
        detail: "|offset| <= the accuracy the service declared at that epoch (worst: excess over the declaration, ns)".into(),
    });

    // Episodes of holdover or free-run: (start index, end index exclusive).
    let mut episodes = Vec::new();
    let mut i = 0;
    while i < log.len() {
        if matches!(log[i].state, State::Holdover | State::Freerun) {
            let s = i;
            while i < log.len() && matches!(log[i].state, State::Holdover | State::Freerun) {
                i += 1;
            }
            episodes.push((s, i));
        } else {
            i += 1;
        }
    }

    // 3. Holdover growth (holdover epochs only, against the profile from the episode start).
    let mut tested = 0;
    let mut ex = 0;
    let mut worst: Option<f64> = None;
    if !l.holdover_profile.is_empty() {
        for &(s, e) in &episodes {
            let t0 = log[s].t;
            for ep in &log[s..e] {
                if ep.state != State::Holdover {
                    continue;
                }
                tested += 1;
                let lim = profile_at(&l.holdover_profile, ep.t - t0).unwrap_or(f64::INFINITY);
                let m = ep.offset_ns.abs() / lim.max(1e-12);
                worst = Some(worst.map_or(m, |w| w.max(m)));
                if ep.offset_ns.abs() > lim {
                    ex += 1;
                }
            }
        }
    }
    tests.push(TestResult {
        id: "holdover-growth",
        verdict: verdict(tested > 0, ex),
        epochs_tested: tested,
        exceedances: ex,
        worst: worst.map(|w| rnd(w, 4)),
        detail: format!(
            "|offset| under the declared holdover profile from each episode's start over {} episodes (worst: ratio to the limit)",
            episodes.len()
        ),
    });

    // 4. Recovery.
    let mut rec_tested = 0;
    let mut rec_ex = 0;
    let mut rec_worst: Option<f64> = None;
    let mut unrecovered = 0;
    if let Some(max_r) = l.max_recovery_s {
        for &(_, e) in &episodes {
            if e >= log.len() {
                continue; // the log ends in the episode: nothing to recover from yet
            }
            rec_tested += 1;
            let t_end = log[e].t;
            match log[e..]
                .iter()
                .find(|x| x.state == State::Locked && x.offset_ns.abs() <= l.locked_ns)
            {
                Some(x) => {
                    let r = x.t - t_end;
                    rec_worst = Some(rec_worst.map_or(r, |w| w.max(r)));
                    if r > max_r {
                        rec_ex += 1;
                    }
                }
                None => {
                    unrecovered += 1;
                    rec_ex += 1;
                }
            }
        }
    }
    tests.push(TestResult {
        id: "recovery",
        verdict: verdict(rec_tested > 0, rec_ex),
        epochs_tested: rec_tested,
        exceedances: rec_ex,
        worst: rec_worst.map(|w| rnd(w, 3)),
        detail: format!(
            "locked and within {} ns within {} s of each episode's end; {unrecovered} never recovered (worst: recovery time, s)",
            l.locked_ns,
            l.max_recovery_s.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
        ),
    });

    // 5. Continuity.
    let (mut gaps, mut back) = (0, 0);
    let mut worst_gap: f64 = 0.0;
    for w in log.windows(2) {
        let d = w[1].t - w[0].t;
        if d <= 0.0 {
            back += 1;
        } else {
            worst_gap = worst_gap.max(d);
            if d > l.max_gap_s {
                gaps += 1;
            }
        }
    }
    tests.push(TestResult {
        id: "continuity",
        verdict: verdict(log.len() > 1, gaps + back),
        epochs_tested: log.len(),
        exceedances: gaps + back,
        worst: Some(rnd(worst_gap, 3)),
        detail: format!(
            "{gaps} gaps over {} s, {back} steps backwards in time (worst: largest step, s)",
            l.max_gap_s
        ),
    });

    // 6. MTIE over the longest locked stretch.
    let mut best = (0, 0);
    let mut i = 0;
    while i < log.len() {
        if log[i].state == State::Locked {
            let s = i;
            while i < log.len()
                && log[i].state == State::Locked
                && (i == s || log[i].t - log[i - 1].t <= l.max_gap_s)
            {
                i += 1;
            }
            if i - s > best.1 - best.0 {
                best = (s, i);
            }
        } else {
            i += 1;
        }
    }
    let seg = &log[best.0..best.1];
    let (mut m_tested, mut m_ex, mut m_worst, mut parts) = (0, 0, None::<f64>, Vec::new());
    if seg.len() >= 3 && !l.mtie_mask.is_empty() {
        let mut steps: Vec<f64> = seg.windows(2).map(|w| w[1].t - w[0].t).collect();
        steps.sort_by(f64::total_cmp);
        let dt = steps[steps.len() / 2];
        let x: Vec<f64> = seg.iter().map(|e| e.offset_ns).collect();
        for &[tau, lim] in &l.mtie_mask {
            let w = (tau / dt).round() as usize;
            if w == 0 || w >= x.len() {
                continue;
            }
            m_tested += 1;
            let m = crate::timing::compliance::mtie(&x, w);
            let r = m / lim.max(1e-12);
            m_worst = Some(m_worst.map_or(r, |v| v.max(r)));
            if m > lim {
                m_ex += 1;
            }
            parts.push(format!("tau {tau} s: {} ns (mask {lim})", rnd(m, 3)));
        }
    }
    tests.push(TestResult {
        id: "mtie-locked",
        verdict: verdict(m_tested > 0, m_ex),
        epochs_tested: seg.len(),
        exceedances: m_ex,
        worst: m_worst.map(|w| rnd(w, 4)),
        detail: if parts.is_empty() {
            "no mask point fits the longest locked stretch".into()
        } else {
            format!("{} (worst: ratio to the mask)", parts.join("; "))
        },
    });

    let mut state_epochs = std::collections::BTreeMap::new();
    for e in log {
        let k = serde_json::to_value(e.state)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        *state_epochs.entry(k).or_insert(0) += 1;
    }
    Ok(Report {
        format: FORMAT,
        evidence: EVIDENCE,
        title: req.title.clone(),
        service: req.service.clone(),
        interface: req.interface.clone(),
        reference: req.reference.clone(),
        limits: req.limits.clone(),
        epochs: log.len(),
        skipped_rows: skipped,
        span_s: rnd(log.last().map(|e| e.t).unwrap_or(0.0) - log[0].t, 3),
        state_epochs,
        tests,
        scope: SCOPE,
    })
}

/// A loaded request with its log text.
pub struct Loaded {
    pub req: Request,
    pub log: String,
    pub inputs: Inputs,
}

/// Load a request and its log.
pub fn load(path: &Path) -> Result<Loaded, String> {
    let (req, mut inputs): (Request, Inputs) = super::read_request(path)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let log = inputs.read(base, &req.log, "time-log")?;
    Ok(Loaded { req, log, inputs })
}

/// The report as Markdown.
pub fn markdown(r: &Report) -> String {
    let mut s = format!(
        "# Resilient-time civil interface tests: {}\n\nService: {} ({} interface). Reference: {}. \
         {} epochs over {} s. Evidence class: {}.\n\n| Test | Verdict | Tested | Exceedances | Worst | Detail |\n|---|---|---|---|---|---|\n",
        r.title,
        r.service,
        r.interface,
        if r.reference.is_empty() { "not stated" } else { &r.reference },
        r.epochs,
        r.span_s,
        r.evidence
    );
    for t in &r.tests {
        s += &format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            t.id,
            t.verdict,
            t.epochs_tested,
            t.exceedances,
            t.worst.map(|w| w.to_string()).unwrap_or_else(|| "-".into()),
            t.detail
        );
    }
    s += "\nWITHIN-LIMIT means within the limit the service declared, for this log only.\n\n";
    s += r.scope;
    s += "\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_interpolates_and_holds_flat() {
        let p = [[0.0, 100.0], [3600.0, 1000.0]];
        assert_eq!(profile_at(&p, 0.0), Some(100.0));
        assert_eq!(profile_at(&p, 1800.0), Some(550.0));
        assert_eq!(profile_at(&p, 7200.0), Some(1000.0));
        assert_eq!(profile_at(&[], 1.0), None);
    }
}
