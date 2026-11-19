// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! The PICS runner: score device logs against the pre-registered bars and write the
//! scorecard directory (signed by the caller with [`crate::signed_dossier::sign`]).
//!
//! Method `kshana-pro/pics/v1` (every rule a constant of this version). For one scenario,
//! with the threat from `onset` to `end` and the device's errors against truth (horizontal:
//! haversine distance to the true position; time: the logged time error):
//!
//! 1. **Detection**: the first epoch in the threat that carries an explicit flag (jamming,
//!    spoofing, or integrity not `ok`), or, unless the bars require an explicit flag, whose
//!    fix is withdrawn; met when it comes within `detect_within_s` of onset.
//! 2. **No misleading output** (DHS level 2 criterion): from onset to the end of the log, no
//!    epoch whose output is valid and unflagged has an error beyond `max_unflagged_*`.
//! 3. **Recovery** (level 1): the first time after `end` from which, for the suite's dwell,
//!    every epoch is valid, unflagged and within `recover_*`; met within `recover_within_s`.
//! 4. **Bounded** (level 3): every output in the threat that is not withdrawn is within
//!    `bounded_*`, flagged or not.
//! 5. **Full** (level 4): the fraction of threat epochs with a valid output within `full_*`
//!    is at least `full_availability`.
//!
//! A criterion whose data the log lacks (no time error for a timing scenario, a log ending
//! before the recovery window) is **not assessed**, never passed. A case passes the bars when
//! 1, 2 and 3 are met; 4 and 5 grade the level only.

use std::path::Path;

use kshana::api::KshanaError;
use kshana::interference_map::grid::haversine_m;
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::log::{DeviceLog, Truth};
use super::{Bars, Mapping, MappingRow, Scenario, Suite, LABEL, LOCK_V1};
use crate::scorecards::levels::{self, CaseLevels, LevelMapping, Verdict};

/// The scorecard format.
pub const FORMAT: &str = "kshana-pics-scorecard/1";
/// The scoring method.
pub const METHOD_ID: &str = "kshana-pro/pics/v1";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Criterion {
    pub verdict: Verdict,
    /// What was measured, in words with numbers.
    pub detail: String,
}

fn crit(verdict: Verdict, detail: impl Into<String>) -> Criterion {
    Criterion {
        verdict,
        detail: detail.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CaseResult {
    pub scenario: String,
    pub title: String,
    pub class: String,
    pub technique: String,
    pub expected: String,
    pub bars: Bars,
    pub bars_sha256: String,
    pub device: String,
    pub log_source: String,
    pub log_sha256: String,
    pub epochs: usize,
    pub truth: Truth,
    /// Seconds from onset to detection; `None` when not detected in the threat.
    pub detection_s: Option<f64>,
    /// `flag` or `fix withdrawn`.
    pub detected_by: Option<String>,
    /// Flagged epochs before onset (false alarms; reported, not a bar).
    pub baseline_flags: usize,
    pub misleading_epochs: usize,
    pub max_unflagged_h_error_m: Option<f64>,
    pub max_unflagged_time_error_ns: Option<f64>,
    pub recovery_s: Option<f64>,
    pub threat_availability: Option<f64>,
    pub detection: Criterion,
    pub no_misleading: Criterion,
    pub recovery: Criterion,
    pub bounded: Criterion,
    pub full: Criterion,
    /// `pass` (detection, no misleading output and recovery met), `fail` (one not met) or
    /// `incomplete` (none failed, one not assessed).
    pub verdict: String,
    pub levels: CaseLevels,
    /// The mapping rows that name this scenario.
    pub mapped_to: Vec<MappingRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SuiteRef {
    pub id: String,
    pub version: String,
    pub file_sha256: String,
    pub lock_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Totals {
    pub cases: usize,
    pub pass: usize,
    pub fail: usize,
    pub incomplete: usize,
}

/// A PICS scorecard.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Scorecard {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub wording: &'static str,
    pub suite: SuiteRef,
    pub cases: Vec<CaseResult>,
    pub totals: Totals,
    pub levels: LevelMapping,
    pub limits: Vec<String>,
}

/// The wording rule of every scorecard.
pub const WORDING: &str = "Each case is test evidence for one PICS scenario under bars \
pre-registered in the suite, mapped to the provisions listed with it. It is not a statement \
that the device meets any standard or framework; the reader applies the standard.";

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

struct Errs {
    t: f64,
    valid: bool,
    flagged: bool,
    withdrawn: bool,
    h: Option<f64>,
    te: Option<f64>,
}

fn over(v: Option<f64>, bar: Option<f64>) -> Option<bool> {
    match (v, bar) {
        (Some(x), Some(b)) => Some(x > b),
        _ => None,
    }
}

/// Score one log against one scenario.
pub fn score_case(
    suite: &Suite,
    sc: &Scenario,
    log: &DeviceLog,
    mapping: &Mapping,
) -> Result<CaseResult, String> {
    if log.scenario != sc.id {
        return Err(format!(
            "log is for scenario {} but is scored against {}",
            log.scenario, sc.id
        ));
    }
    log.validate()?;
    let truth = log.truth.unwrap_or(Truth {
        lat_deg: suite.suite.truth_lat_deg,
        lon_deg: suite.suite.truth_lon_deg,
        height_m: suite.suite.truth_height_m,
    });
    let pos = sc.has_domain("position");
    let tim = sc.has_domain("timing");
    let b = &sc.bars;
    let (onset, end) = suite.threat_window();
    let dwell = suite.suite.recovery_dwell_s;
    let errs: Vec<Errs> = log
        .epochs
        .iter()
        .map(|e| Errs {
            t: e.t_s,
            valid: e.fix_valid != Some(false),
            flagged: e.flagged(),
            withdrawn: e.withdrawn(),
            h: if pos {
                match (e.lat_deg, e.lon_deg) {
                    (Some(la), Some(lo)) => Some(haversine_m(truth.lat_deg, truth.lon_deg, la, lo)),
                    _ => None,
                }
            } else {
                None
            },
            te: if tim {
                e.time_error_ns.map(f64::abs)
            } else {
                None
            },
        })
        .collect();
    let threat: Vec<&Errs> = errs.iter().filter(|e| e.t >= onset && e.t < end).collect();
    let after_onset: Vec<&Errs> = errs.iter().filter(|e| e.t >= onset).collect();
    let has_h = |s: &[&Errs]| s.iter().any(|e| e.h.is_some());
    let has_t = |s: &[&Errs]| s.iter().any(|e| e.te.is_some());
    // A domain the scenario tests but the log never measured makes its criteria not assessed.
    let domain_gap = |s: &[&Errs]| (pos && !has_h(s)) || (tim && !has_t(s));

    // 1. Detection.
    let baseline_flags = errs.iter().filter(|e| e.t < onset && e.flagged).count();
    let det = threat.iter().find_map(|e| {
        if e.flagged {
            Some((e.t - onset, "flag"))
        } else if e.withdrawn && !b.explicit_flag_required {
            Some((e.t - onset, "fix withdrawn"))
        } else {
            None
        }
    });
    let detection = if threat.is_empty() {
        crit(
            Verdict::NotAssessed,
            "the log has no epoch inside the threat",
        )
    } else {
        match det {
            Some((s, by)) if s <= b.detect_within_s => crit(
                Verdict::Met,
                format!(
                    "detected by {by} {s:.1} s after onset (bar {} s)",
                    b.detect_within_s
                ),
            ),
            Some((s, by)) => crit(
                Verdict::NotMet,
                format!(
                    "detected by {by} {s:.1} s after onset, later than the bar of {} s",
                    b.detect_within_s
                ),
            ),
            None => crit(
                Verdict::NotMet,
                if b.explicit_flag_required {
                    "no explicit flag during the threat".to_string()
                } else {
                    "no flag and no withdrawn fix during the threat".to_string()
                },
            ),
        }
    };

    // 2. No misleading output.
    let mut misleading = 0usize;
    let (mut max_h, mut max_t): (Option<f64>, Option<f64>) = (None, None);
    for e in &after_onset {
        if !e.valid || e.flagged {
            continue;
        }
        if let Some(h) = e.h {
            max_h = Some(max_h.map_or(h, |m: f64| m.max(h)));
        }
        if let Some(t) = e.te {
            max_t = Some(max_t.map_or(t, |m: f64| m.max(t)));
        }
        if over(e.h, b.max_unflagged_h_error_m) == Some(true)
            || over(e.te, b.max_unflagged_time_error_ns) == Some(true)
        {
            misleading += 1;
        }
    }
    let no_misleading = if after_onset.is_empty() {
        crit(Verdict::NotAssessed, "the log ends before onset")
    } else if misleading > 0 {
        crit(
            Verdict::NotMet,
            format!(
                "{misleading} valid, unflagged epochs beyond the bars (largest unflagged errors: {} m, {} ns)",
                fmt_opt(max_h),
                fmt_opt(max_t)
            ),
        )
    } else if domain_gap(&after_onset) {
        crit(
            Verdict::NotAssessed,
            "no misleading epoch in the data given, but a tested domain has no error data",
        )
    } else {
        crit(
            Verdict::Met,
            format!(
                "no valid, unflagged epoch beyond the bars (largest unflagged errors: {} m, {} ns)",
                fmt_opt(max_h),
                fmt_opt(max_t)
            ),
        )
    };

    // 3. Recovery.
    let post: Vec<&Errs> = errs.iter().filter(|e| e.t >= end).collect();
    let good = |e: &Errs| {
        e.valid
            && !e.flagged
            && (!pos
                || e.h
                    .is_some_and(|h| h <= b.recover_h_error_m.unwrap_or(f64::INFINITY)))
            && (!tim
                || e.te
                    .is_some_and(|t| t <= b.recover_time_error_ns.unwrap_or(f64::INFINITY)))
    };
    let last_t = errs.last().map(|e| e.t).unwrap_or(f64::NEG_INFINITY);
    let mut rec_at = None;
    for (k, e) in post.iter().enumerate() {
        if !good(e) {
            continue;
        }
        if e.t + dwell > last_t {
            break;
        }
        if post[k..]
            .iter()
            .take_while(|x| x.t <= e.t + dwell)
            .all(|x| good(x))
        {
            rec_at = Some(e.t - end);
            break;
        }
    }
    let covered = last_t >= end + b.recover_within_s + dwell;
    let recovery = match rec_at {
        Some(s) if s <= b.recover_within_s => crit(
            Verdict::Met,
            format!("correct and unflagged from {s:.1} s after the threat ended, held {dwell} s (bar {} s)", b.recover_within_s),
        ),
        Some(s) => crit(
            Verdict::NotMet,
            format!("correct only from {s:.1} s after the threat ended (bar {} s)", b.recover_within_s),
        ),
        None if domain_gap(&post) => crit(
            Verdict::NotAssessed,
            "a tested domain has no error data after the threat",
        ),
        None if covered => crit(
            Verdict::NotMet,
            format!("not correct and unflagged for {dwell} s within {} s of the threat ending", b.recover_within_s),
        ),
        None => crit(
            Verdict::NotAssessed,
            "the log ends before the recovery window and its dwell",
        ),
    };

    // 4. Bounded.
    let outputs: Vec<&&Errs> = threat.iter().filter(|e| !e.withdrawn).collect();
    let beyond = outputs
        .iter()
        .filter(|e| {
            over(e.h, b.bounded_h_error_m) == Some(true)
                || over(e.te, b.bounded_time_error_ns) == Some(true)
        })
        .count();
    let bounded = if threat.is_empty() {
        crit(Verdict::NotAssessed, "no epoch inside the threat")
    } else if beyond > 0 {
        crit(
            Verdict::NotMet,
            format!("{beyond} outputs during the threat beyond the bound"),
        )
    } else if domain_gap(&threat) && !outputs.is_empty() {
        crit(
            Verdict::NotAssessed,
            "a tested domain has no error data during the threat",
        )
    } else {
        crit(
            Verdict::Met,
            format!(
                "every output during the threat within the bound ({} outputs, the rest withdrawn)",
                outputs.len()
            ),
        )
    };

    // 5. Full performance.
    let full_ok = threat
        .iter()
        .filter(|e| {
            e.valid
                && (!pos
                    || e.h
                        .is_some_and(|h| h <= b.full_h_error_m.unwrap_or(f64::INFINITY)))
                && (!tim
                    || e.te
                        .is_some_and(|t| t <= b.full_time_error_ns.unwrap_or(f64::INFINITY)))
        })
        .count();
    let avail = (!threat.is_empty()).then(|| full_ok as f64 / threat.len() as f64);
    let full = match avail {
        None => crit(Verdict::NotAssessed, "no epoch inside the threat"),
        Some(a) if domain_gap(&threat) && a < b.full_availability => crit(
            Verdict::NotAssessed,
            "a tested domain has no error data during the threat",
        ),
        Some(a) if a >= b.full_availability => crit(
            Verdict::Met,
            format!(
                "{:.1} % of threat epochs at full performance (bar {:.1} %)",
                100.0 * a,
                100.0 * b.full_availability
            ),
        ),
        Some(a) => crit(
            Verdict::NotMet,
            format!(
                "{:.1} % of threat epochs at full performance (bar {:.1} %)",
                100.0 * a,
                100.0 * b.full_availability
            ),
        ),
    };

    let levels = CaseLevels::from_criteria(
        recovery.verdict,
        no_misleading.verdict,
        bounded.verdict,
        full.verdict,
    );
    let core = detection
        .verdict
        .and(no_misleading.verdict)
        .and(recovery.verdict);
    let verdict = match core {
        Verdict::Met => "pass",
        Verdict::NotMet => "fail",
        Verdict::NotAssessed => "incomplete",
    };
    Ok(CaseResult {
        scenario: sc.id.clone(),
        title: sc.title.clone(),
        class: sc.class.clone(),
        technique: sc.technique.clone(),
        expected: sc.expected.clone(),
        bars: sc.bars.clone(),
        bars_sha256: sc.bars.sha256(),
        device: log.device.name.clone(),
        log_source: log.source.clone().unwrap_or_else(|| "json".into()),
        log_sha256: sha(log.to_json().as_bytes()),
        epochs: log.epochs.len(),
        truth,
        detection_s: det.map(|d| d.0),
        detected_by: det.map(|d| d.1.to_string()),
        baseline_flags,
        misleading_epochs: misleading,
        max_unflagged_h_error_m: max_h,
        max_unflagged_time_error_ns: max_t,
        recovery_s: rec_at,
        threat_availability: avail,
        detection,
        no_misleading,
        recovery,
        bounded,
        full,
        verdict: verdict.into(),
        levels,
        mapped_to: mapping.rows_for(&sc.id).into_iter().cloned().collect(),
    })
}

fn fmt_opt(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.1}")).unwrap_or_else(|| "n/a".into())
}

/// Score logs (one or more, each naming its scenario) against a suite source, checked first
/// against `lock`. A suite whose bars were changed after pre-registration is refused.
pub fn score(
    suite_src: &str,
    lock: &str,
    mapping: &Mapping,
    logs: &[DeviceLog],
) -> Result<Scorecard, KshanaError> {
    let bad = |m: String| KshanaError::InvalidInput(m);
    let suite = Suite::parse(suite_src).map_err(bad)?;
    suite.check_lock(lock).map_err(bad)?;
    if logs.is_empty() {
        return Err(bad("PICS score needs at least one device log".into()));
    }
    let mut cases = Vec::new();
    for l in logs {
        let sc = suite
            .scenario(&l.scenario)
            .ok_or_else(|| bad(format!("unknown PICS scenario '{}'", l.scenario)))?;
        cases.push(score_case(&suite, sc, l, mapping).map_err(bad)?);
    }
    let lv: Vec<CaseLevels> = cases.iter().map(|c| c.levels.clone()).collect();
    let count = |v: &str| cases.iter().filter(|c| c.verdict == v).count();
    Ok(Scorecard {
        format: FORMAT,
        method: METHOD_ID,
        label: LABEL,
        wording: WORDING,
        suite: SuiteRef {
            id: suite.suite.id.clone(),
            version: suite.suite.version.clone(),
            file_sha256: Suite::file_sha256(suite_src),
            lock_sha256: sha(lock.as_bytes()),
        },
        totals: Totals {
            cases: cases.len(),
            pass: count("pass"),
            fail: count("fail"),
            incomplete: count("incomplete"),
        },
        cases,
        levels: levels::map(&lv),
        limits: vec![
            "Scored from the device's own log as supplied; Kshana did not run the test and cannot see what the log leaves out.".into(),
            "Threat parameters and bars are representative and pre-registered; they are illustrative until confirmed against the standards they are mapped to.".into(),
            "Truth is static (the suite's or the log's stated position); a moving-platform test needs a truth track, which v1 does not take.".into(),
            "A scenario not run is not covered by this scorecard.".into(),
        ],
    })
}

/// Problems with a scorecard's bars against the lock this build carries (a scorecard made
/// with loosened bars, or by a build with another lock).
pub fn check_bars(s: &Scorecard) -> Vec<String> {
    check_bars_json(&serde_json::to_value(s).unwrap_or_default())
}

/// [`check_bars`] on a scorecard read back from JSON.
pub fn check_bars_json(v: &serde_json::Value) -> Vec<String> {
    let mut want = std::collections::BTreeMap::new();
    for l in LOCK_V1.lines().filter(|l| !l.starts_with('#')) {
        if let Some((id, h)) = l.split_once(' ') {
            want.insert(id.to_string(), h.trim().to_string());
        }
    }
    let mut p = Vec::new();
    let Some(cases) = v.get("cases").and_then(|c| c.as_array()) else {
        return vec!["scorecard has no cases".into()];
    };
    for c in cases {
        let id = c.get("scenario").and_then(|x| x.as_str()).unwrap_or("?");
        let stated = c.get("bars_sha256").and_then(|x| x.as_str()).unwrap_or("");
        let recomputed = c
            .get("bars")
            .and_then(|b| serde_json::from_value::<Bars>(b.clone()).ok())
            .map(|b| b.sha256())
            .unwrap_or_default();
        if stated != recomputed {
            p.push(format!("{id}: bars do not hash to their stated sha256"));
        }
        match want.get(id) {
            Some(h) if *h == recomputed => {}
            Some(_) => p.push(format!("{id}: bars differ from the pre-registered lock")),
            None => p.push(format!("{id}: not a pre-registered scenario of this build")),
        }
    }
    p
}

/// `cases.csv`.
pub fn cases_csv(s: &Scorecard) -> String {
    let mut o = String::from(
        "scenario,class,technique,verdict,detection,no_misleading,recovery,bounded,full,detection_s,recovery_s,misleading_epochs,bars_sha256\n",
    );
    for c in &s.cases {
        o.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            c.scenario,
            c.class,
            c.technique,
            c.verdict,
            c.detection.verdict.as_str(),
            c.no_misleading.verdict.as_str(),
            c.recovery.verdict.as_str(),
            c.bounded.verdict.as_str(),
            c.full.verdict.as_str(),
            c.detection_s.map(|v| format!("{v:.1}")).unwrap_or_default(),
            c.recovery_s.map(|v| format!("{v:.1}")).unwrap_or_default(),
            c.misleading_epochs,
            c.bars_sha256
        ));
    }
    o
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `report.html`: a plain, self-contained page.
pub fn html(s: &Scorecard) -> String {
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>PICS scorecard</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:.3rem;text-align:left;vertical-align:top}}\
.met{{color:#065f46}}.not-met{{color:#991b1b}}.not-assessed{{color:#92400e}}</style></head><body>\
<h1>PICS scorecard</h1><p><strong>{}</strong> {} · method {} · suite sha256 <code>{}</code></p><p>{}</p><p>{}</p>",
        esc(&s.suite.id),
        esc(&s.suite.version),
        s.method,
        &s.suite.file_sha256[..16],
        esc(s.label),
        esc(s.wording)
    );
    o.push_str(&format!(
        "<p>{} cases: {} pass, {} fail, {} incomplete. Highest DHS RPCF level supported by every case: {}.</p>",
        s.totals.cases, s.totals.pass, s.totals.fail, s.totals.incomplete, s.levels.highest_supported
    ));
    o.push_str("<table><tr><th>Scenario</th><th>Verdict</th><th>Detection</th><th>No misleading output</th><th>Recovery</th><th>Bounded</th><th>Full</th></tr>");
    for c in &s.cases {
        let cell = |k: &Criterion| {
            let cls = k.verdict.as_str().replace(' ', "-");
            format!(
                "<td class=\"{cls}\">{}<br><small>{}</small></td>",
                k.verdict.as_str(),
                esc(&k.detail)
            )
        };
        o.push_str(&format!(
            "<tr><td><strong>{}</strong><br><small>{}</small></td><td>{}</td>{}{}{}{}{}</tr>",
            esc(&c.scenario),
            esc(&c.title),
            c.verdict,
            cell(&c.detection),
            cell(&c.no_misleading),
            cell(&c.recovery),
            cell(&c.bounded),
            cell(&c.full)
        ));
    }
    o.push_str("</table><h2>Mapped provisions</h2><table><tr><th>Scenario</th><th>Standard</th><th>Clause</th><th>Topic</th></tr>");
    for c in &s.cases {
        for r in &c.mapped_to {
            o.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                esc(&c.scenario),
                esc(&r.standard),
                esc(&r.clause),
                esc(&r.topic)
            ));
        }
    }
    o.push_str("</table><h2>Limits</h2><ul>");
    for l in &s.limits {
        o.push_str(&format!("<li>{}</li>", esc(l)));
    }
    o.push_str("</ul></body></html>\n");
    o
}

/// Write the scorecard directory: `scorecard.json`, `cases.csv`, `report.html`, the suite
/// file scored against, and each normalised device log under `logs/`.
pub fn write_dir(
    out: &Path,
    s: &Scorecard,
    suite_src: &str,
    logs: &[DeviceLog],
) -> Result<(), KshanaError> {
    let io = |e: std::io::Error| KshanaError::InvalidInput(format!("{}: {e}", out.display()));
    std::fs::create_dir_all(out.join("logs")).map_err(io)?;
    std::fs::write(
        out.join("scorecard.json"),
        serde_json::to_string_pretty(s).expect("scorecard serialises"),
    )
    .map_err(io)?;
    std::fs::write(out.join("cases.csv"), cases_csv(s)).map_err(io)?;
    std::fs::write(out.join("report.html"), html(s)).map_err(io)?;
    std::fs::write(out.join("suite.toml"), suite_src).map_err(io)?;
    for (i, l) in logs.iter().enumerate() {
        std::fs::write(
            out.join("logs")
                .join(format!("{:02}-{}.json", i + 1, l.scenario)),
            l.to_json(),
        )
        .map_err(io)?;
    }
    Ok(())
}
