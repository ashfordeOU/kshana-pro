// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Smart tachograph and road-charging OSNMA field verification (research 2026-11-20 gaps,
//! B1403)
//!
//! Commission Implementing Regulation (EU) 2021/1228 amends the smart-tachograph rules so that
//! the vehicle unit (VU) uses Galileo OSNMA-authenticated positions: positions carry an
//! authentication flag, the VU's clock may only be re-adjusted from authenticated GNSS time, a
//! discrepancy with authenticated GNSS time triggers a time-conflict event, and an attack or a
//! failed authentication triggers a GNSS anomaly event (requirements of Annex IC as read on
//! EUR-Lex, 21 Nov 2026; the GNS_xx requirements of Appendix 12 were not in the text read and are
//! to confirm). The catalogue `eu-2021-1228-tachograph-osnma` maps them to this evidence.
//!
//! [`verify`] reads the user's VU or test-receiver log
//! (`timestamp,position_auth,gnss_time,vu_time,event`; `position_auth` is `authenticated`,
//! `not-authenticated` or `none`; `event` is empty, `gnss-anomaly`, `time-conflict` or
//! `time-adjust`) and reports, each checked against the requirement as read:
//!
//! * authenticated share of positions and time to the first authenticated position;
//! * **time conflict** (Annex IC req. 86, ±3 s): records where VU time and authenticated GNSS
//!   time differ by more than the threshold, and whether a `time-conflict` event was logged
//!   at or before the next re-adjustment;
//! * **time adjustment** (req. 211 and 212): adjustments made without an authenticated position,
//!   and intervals between automatic adjustments outside 72 h to 168 h;
//! * **GNSS anomaly events** (req. 88a): events closer than 10 minutes to the previous one;
//! * **fallback**: time spent with positions recorded but not authenticated, and with no
//!   position, so the behaviour while OSNMA is unavailable is on record.
//!
//! Evidence for the operator or manufacturer, signed; MODELLED (synthetic logs in tests). The
//! thresholds are those of the requirements as read and are written into every result. It does
//! not state that a vehicle unit meets the regulation.

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};
use crate::transport::Table;

/// Pro feature.
pub const FEATURE: &str = "pro-tachograph-osnma";
/// Format.
pub const FORMAT: &str = "kshana-pro-tachograph-osnma-field/1";

/// Thresholds as read from Annex IC (written into every result).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Thresholds {
    /// Req. 86: time discrepancy above this (s).
    pub time_conflict_s: f64,
    /// Req. 211: automatic re-adjustment window (h).
    pub readjust_min_h: f64,
    pub readjust_max_h: f64,
    /// Req. 88a: no further GNSS anomaly event within this (s).
    pub anomaly_quiet_s: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            time_conflict_s: 3.0,
            readjust_min_h: 72.0,
            readjust_max_h: 168.0,
            anomaly_quiet_s: 600.0,
        }
    }
}

/// A finding with the records behind it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    pub check: &'static str,
    pub requirement: &'static str,
    pub ok: bool,
    pub detail: String,
    pub observations: Vec<String>,
}

/// The field-verification result.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Result_ {
    pub format: &'static str,
    pub label: &'static str,
    pub source: &'static str,
    pub thresholds: Thresholds,
    pub records: usize,
    pub authenticated_share: f64,
    pub first_authenticated_after_s: Option<f64>,
    pub not_authenticated_s: f64,
    pub no_position_s: f64,
    pub findings: Vec<Finding>,
    pub verdict: String,
}

const SOURCE: &str = "Commission Implementing Regulation (EU) 2021/1228, Annex IC requirements 86, 88a, 110, 211 and 212, read on EUR-Lex (CELEX 32021R1228) on 21 Nov 2026; Appendix 12 GNS_xx requirements not read (to confirm).";

struct Rec {
    t: f64,
    auth: String,
    gnss: Option<f64>,
    vu: Option<f64>,
    event: String,
}

fn cap(v: Vec<String>) -> Vec<String> {
    v.into_iter().take(20).collect()
}

/// Verify a log.
pub fn verify(csv: &str, th: &Thresholds) -> Result<Result_, String> {
    let t = Table::parse(csv)?;
    let (ct, ca) = (t.need("timestamp")?, t.need("position_auth")?);
    let (cg, cv, ce) = (t.col("gnss_time"), t.col("vu_time"), t.col("event"));
    let mut r = Vec::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp '{ts}'", i + 2))?;
        let auth = t.text(i, Some(ca)).to_string();
        if !["authenticated", "not-authenticated", "none"].contains(&auth.as_str()) {
            return Err(format!("row {}: position_auth '{auth}'", i + 2));
        }
        let ev = t.text(i, ce).to_string();
        if !["", "gnss-anomaly", "time-conflict", "time-adjust"].contains(&ev.as_str()) {
            return Err(format!("row {}: event '{ev}'", i + 2));
        }
        r.push(Rec {
            t: tt,
            auth,
            gnss: parse_utc(t.text(i, cg)),
            vu: parse_utc(t.text(i, cv)),
            event: ev,
        });
    }
    if r.len() < 2 {
        return Err("the log needs at least two records".into());
    }
    r.sort_by(|a, b| a.t.total_cmp(&b.t));
    let span = |pred: &dyn Fn(&Rec) -> bool| -> f64 {
        r.windows(2)
            .filter(|w| pred(&w[0]))
            .map(|w| w[1].t - w[0].t)
            .sum()
    };
    let n_auth = r.iter().filter(|x| x.auth == "authenticated").count();
    let first = r
        .iter()
        .find(|x| x.auth == "authenticated")
        .map(|x| x.t - r[0].t);
    let mut findings = Vec::new();
    // Time conflict.
    let conflicts: Vec<&Rec> = r
        .iter()
        .filter(|x| {
            x.auth == "authenticated"
                && matches!((x.gnss, x.vu), (Some(g), Some(v)) if (v - g).abs() > th.time_conflict_s)
        })
        .collect();
    let unlogged: Vec<String> = conflicts
        .iter()
        .filter(|c| {
            // A time-conflict event at or after the discrepancy, before the next adjustment.
            let next_adj = r.iter().find(|x| x.t > c.t && x.event == "time-adjust").map(|x| x.t);
            !r.iter().any(|x| {
                x.event == "time-conflict" && x.t >= c.t && next_adj.is_none_or(|a| x.t <= a)
            })
        })
        .map(|c| format!("{}: VU time differs from authenticated GNSS time by {:.1} s, no time-conflict event", iso(c.t), c.vu.unwrap_or(0.0) - c.gnss.unwrap_or(0.0)))
        .collect();
    findings.push(Finding {
        check: "time-conflict-event",
        requirement: "Annex IC req. 86",
        ok: unlogged.is_empty(),
        detail: format!(
            "{} record(s) with a discrepancy above {} s against authenticated GNSS time; {} without a time-conflict event",
            conflicts.len(),
            th.time_conflict_s,
            unlogged.len()
        ),
        observations: cap(unlogged),
    });
    // Time adjustment only from authenticated positions; interval window.
    let adj: Vec<&Rec> = r.iter().filter(|x| x.event == "time-adjust").collect();
    let unauth: Vec<String> = adj
        .iter()
        .filter(|x| x.auth != "authenticated")
        .map(|x| format!("{}: time adjusted with position '{}'", iso(x.t), x.auth))
        .collect();
    findings.push(Finding {
        check: "adjust-from-authenticated-time",
        requirement: "Annex IC req. 211, 212",
        ok: unauth.is_empty(),
        detail: format!(
            "{} adjustment(s); {} without an authenticated position",
            adj.len(),
            unauth.len()
        ),
        observations: cap(unauth),
    });
    let out_window: Vec<String> = adj
        .windows(2)
        .filter_map(|w| {
            let h = (w[1].t - w[0].t) / 3600.0;
            (h < th.readjust_min_h || h > th.readjust_max_h).then(|| {
                format!(
                    "{} to {}: {h:.1} h between adjustments",
                    iso(w[0].t),
                    iso(w[1].t)
                )
            })
        })
        .collect();
    findings.push(Finding {
        check: "readjustment-interval",
        requirement: "Annex IC req. 211",
        ok: out_window.is_empty(),
        detail: format!(
            "{} interval(s) outside {}-{} h (manual workshop adjustments, if any, appear here too and need the workshop record)",
            out_window.len(),
            th.readjust_min_h,
            th.readjust_max_h
        ),
        observations: cap(out_window),
    });
    // GNSS anomaly spacing.
    let an: Vec<&Rec> = r.iter().filter(|x| x.event == "gnss-anomaly").collect();
    let close: Vec<String> = an
        .windows(2)
        .filter(|w| w[1].t - w[0].t < th.anomaly_quiet_s)
        .map(|w| {
            format!(
                "{} after {}: {:.0} s apart",
                iso(w[1].t),
                iso(w[0].t),
                w[1].t - w[0].t
            )
        })
        .collect();
    findings.push(Finding {
        check: "anomaly-event-spacing",
        requirement: "Annex IC req. 88a",
        ok: close.is_empty(),
        detail: format!(
            "{} GNSS anomaly event(s); {} within {} s of the previous",
            an.len(),
            close.len(),
            th.anomaly_quiet_s
        ),
        observations: cap(close),
    });
    let not_auth = span(&|x: &Rec| x.auth == "not-authenticated");
    let none = span(&|x: &Rec| x.auth == "none");
    let bad = findings.iter().filter(|f| !f.ok).count();
    let verdict = format!(
        "{} record(s), {:.1} % authenticated; {:.0} s recorded not authenticated, {:.0} s without position; {} of {} checks against the requirements as read show a departure. Field evidence; not a statement that the unit meets the regulation.",
        r.len(),
        100.0 * n_auth as f64 / r.len() as f64,
        not_auth,
        none,
        bad,
        findings.len()
    );
    Ok(Result_ {
        format: FORMAT,
        label: "MODELLED: checks of the log as supplied against the requirements as read; the unit's own conformity assessment is the type-approval authority's.",
        source: SOURCE,
        thresholds: th.clone(),
        records: r.len(),
        authenticated_share: n_auth as f64 / r.len() as f64,
        first_authenticated_after_s: first,
        not_authenticated_s: not_auth,
        no_position_s: none,
        findings,
        verdict,
    })
}

/// A SYNTHETIC log over ten days, a record every ten minutes: OSNMA unavailable for half of
/// day 3; VU time 4 s off on day 5; one automatic time adjustment on day 6; two GNSS anomaly
/// events. `good` logs the time-conflict events; otherwise it also adjusts time on day 3 without
/// authentication, logs no time-conflict event, and repeats an anomaly event two minutes later.
pub fn synth(good: bool) -> String {
    let t0 = 1_780_272_000.0;
    let mut s = String::from("timestamp,position_auth,gnss_time,vu_time,event\n");
    let mut push = |t: f64, auth: &str, drift: f64, ev: &str| {
        s.push_str(&format!(
            "{},{auth},{},{},{ev}\n",
            iso(t0 + t),
            iso(t0 + t),
            iso(t0 + t + drift)
        ));
    };
    for k in 0..(10 * 144) {
        let t = k as f64 * 600.0;
        let day = t / 86_400.0;
        let auth = if k < 2 {
            "none"
        } else if (3.0..3.5).contains(&day) {
            "not-authenticated"
        } else {
            "authenticated"
        };
        let conflict = (720..722).contains(&k);
        let drift = if conflict { 4.0 } else { 0.0 };
        let ev = match k {
            864 => "time-adjust",
            435 if !good => "time-adjust",
            460 | 470 => "gnss-anomaly",
            _ if conflict && good => "time-conflict",
            _ => "",
        };
        push(t, auth, drift, ev);
        if k == 460 && !good {
            push(t + 120.0, auth, 0.0, "gnss-anomaly");
        }
    }
    s
}
