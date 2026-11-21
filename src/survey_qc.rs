// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Marine survey and construction positioning QC under interference (research 2026-11-20
//! gaps, B0807)
//!
//! Hydrographic and offshore-construction positioning is accepted or re-run line by line. This
//! module reads the positioning log of a survey (`timestamp,line_id,lat,lon` plus any of
//! `fix_quality,num_sats,cn0_db,agc_db,diff_age_s,sigma_h_m`) and flags, epoch by epoch, the
//! signs that interference degraded positioning, each against a threshold the user states:
//!
//! * mean C/N0 below the line's own calibration baseline by `cn0_drop_db`;
//! * AGC below its baseline by `agc_drop_db` (a front end turning gain down for added power);
//! * satellites fewer than `min_sats`;
//! * fix quality below the line's required mode (`required_fix`, NMEA GGA quality codes:
//!   4 RTK fixed, 5 RTK float, 2 differential, 1 autonomous);
//! * correction age above `max_diff_age_s`;
//! * reported horizontal sigma above the survey's horizontal tolerance;
//! * cross-track residual from the line's fitted straight track above the tolerance.
//!
//! Flagged epochs closer than `merge_s` form intervals. Each line gets **PASS** (no interval),
//! **DEGRADED** (flagged share up to `reject_share`; the intervals to check or infill) or
//! **RE-RUN** (above it). The QC annex lists them with the indicators behind each interval; the
//! **site risk plan** groups flagged time by hour of day and 0.01-degree cell and lists the
//! windows and places where interference recurred.
//!
//! The horizontal tolerance and its source (e.g. the survey specification or an IHO S-44 order
//! the user names) are the user's; Kshana bundles no tolerance table. **MODELLED**: rules
//! exercised on a synthetic survey with injected intervals; nothing here locates an emitter.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::{iso, parse_utc};
use crate::transport::{en_m, Table};

/// Pro feature.
pub const FEATURE: &str = "pro-survey-qc";
/// Format tag.
pub const FORMAT: &str = "kshana-pro-survey-qc-annex/1";

/// The request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    pub log: String,
    /// Horizontal tolerance (m) and where it comes from.
    pub horizontal_tolerance_m: f64,
    pub tolerance_source: String,
    #[serde(default = "d_calib")]
    pub calibration_s: f64,
    #[serde(default = "d_cn0")]
    pub cn0_drop_db: f64,
    #[serde(default = "d_agc")]
    pub agc_drop_db: f64,
    #[serde(default = "d_sats")]
    pub min_sats: u32,
    /// Required fix quality (GGA code); `None` disables the check.
    #[serde(default)]
    pub required_fix: Option<u8>,
    #[serde(default = "d_age")]
    pub max_diff_age_s: f64,
    #[serde(default = "d_merge")]
    pub merge_s: f64,
    #[serde(default = "d_reject")]
    pub reject_share: f64,
}

fn d_calib() -> f64 {
    120.0
}
fn d_cn0() -> f64 {
    6.0
}
fn d_agc() -> f64 {
    3.0
}
fn d_sats() -> u32 {
    6
}
fn d_age() -> f64 {
    10.0
}
fn d_merge() -> f64 {
    10.0
}
fn d_reject() -> f64 {
    0.05
}

#[derive(Clone, Debug)]
struct Row {
    t: f64,
    lat: f64,
    lon: f64,
    fix: Option<u8>,
    sats: Option<u32>,
    cn0: Option<f64>,
    agc: Option<f64>,
    age: Option<f64>,
    sigma: Option<f64>,
}

/// One flagged interval.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Interval {
    pub start_utc: String,
    pub end_utc: String,
    pub duration_s: f64,
    pub epochs: usize,
    /// Indicator and the number of epochs it fired on.
    pub indicators: BTreeMap<&'static str, usize>,
    pub start_position: [f64; 2],
}

/// QC of one line.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LineQc {
    pub line_id: String,
    pub epochs: usize,
    pub start_utc: String,
    pub end_utc: String,
    pub baseline_cn0_db: Option<f64>,
    pub baseline_agc_db: Option<f64>,
    pub flagged_share: f64,
    /// PASS, DEGRADED or RE-RUN.
    pub qc: &'static str,
    pub intervals: Vec<Interval>,
}

/// A recurring risk window or place.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Risk {
    pub key: String,
    pub flagged_s: f64,
    pub lines: Vec<String>,
}

/// The annex.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Annex {
    pub format: &'static str,
    pub title: String,
    pub evidence: &'static str,
    pub request: Request,
    pub log_sha256: String,
    pub lines: Vec<LineQc>,
    /// Flagged time by UTC hour of day.
    pub risk_hours: Vec<Risk>,
    /// Flagged time by 0.01-degree cell (south-west corner).
    pub risk_cells: Vec<Risk>,
    pub plan: Vec<String>,
    pub verdict: String,
}

fn parse(text: &str) -> Result<BTreeMap<String, Vec<Row>>, String> {
    let t = Table::parse(text)?;
    let (ct, cl, cla, clo) = (
        t.need("timestamp")?,
        t.need("line_id")?,
        t.need("lat")?,
        t.need("lon")?,
    );
    let c = |n: &str| t.col(n);
    let (cf, cs, cc, ca, cg, cx) = (
        c("fix_quality"),
        c("num_sats"),
        c("cn0_db"),
        c("agc_db"),
        c("diff_age_s"),
        c("sigma_h_m"),
    );
    let mut out: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for i in 0..t.rows.len() {
        let ts = t.text(i, Some(ct));
        let tt = parse_utc(ts).ok_or_else(|| format!("row {}: bad timestamp '{ts}'", i + 2))?;
        let need = |c| {
            t.num(i, Some(c))?
                .ok_or_else(|| format!("row {}: empty coordinate", i + 2))
        };
        out.entry(t.text(i, Some(cl)).to_string())
            .or_default()
            .push(Row {
                t: tt,
                lat: need(cla)?,
                lon: need(clo)?,
                fix: t.num(i, cf)?.map(|v| v as u8),
                sats: t.num(i, cs)?.map(|v| v as u32),
                cn0: t.num(i, cc)?,
                agc: t.num(i, ca)?,
                age: t.num(i, cg)?,
                sigma: t.num(i, cx)?,
            });
    }
    for v in out.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    Ok(out)
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    v.retain(|x| x.is_finite());
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

/// Fix quality rank: higher is better (4 RTK fixed > 5 float > 2 differential > 1 autonomous).
fn rank(q: u8) -> u8 {
    match q {
        4 => 4,
        5 => 3,
        2 | 9 => 2,
        1 => 1,
        _ => 0,
    }
}

fn validate(q: &Request) -> Result<(), String> {
    for (n, v) in [
        ("horizontal_tolerance_m", q.horizontal_tolerance_m),
        ("calibration_s", q.calibration_s),
        ("cn0_drop_db", q.cn0_drop_db),
        ("agc_drop_db", q.agc_drop_db),
        ("max_diff_age_s", q.max_diff_age_s),
        ("merge_s", q.merge_s),
    ] {
        if v.is_nan() || v <= 0.0 {
            return Err(format!("{n} must be positive"));
        }
    }
    if !(0.0..=1.0).contains(&q.reject_share) {
        return Err("reject_share must be in [0, 1]".into());
    }
    if q.tolerance_source.trim().is_empty() {
        return Err("tolerance_source must name where the tolerance comes from".into());
    }
    Ok(())
}

fn qc_line(id: &str, v: &[Row], q: &Request) -> LineQc {
    let t0 = v[0].t;
    let calib: Vec<&Row> = v.iter().filter(|r| r.t - t0 <= q.calibration_s).collect();
    let b_cn0 = median(calib.iter().filter_map(|r| r.cn0).collect());
    let b_agc = median(calib.iter().filter_map(|r| r.agc).collect());
    // Straight-line fit of the line (first to last epoch).
    let (a, b) = (&v[0], &v[v.len() - 1]);
    let (ex, ny) = en_m(a.lat, a.lon, b.lat, b.lon);
    let len = ex.hypot(ny);
    let flags: Vec<Vec<&'static str>> = v
        .iter()
        .map(|r| {
            let mut f = Vec::new();
            if let (Some(c), Some(bc)) = (r.cn0, b_cn0) {
                if bc - c >= q.cn0_drop_db {
                    f.push("cn0-drop");
                }
            }
            if let (Some(g), Some(bg)) = (r.agc, b_agc) {
                if bg - g >= q.agc_drop_db {
                    f.push("agc-drop");
                }
            }
            if r.sats.is_some_and(|s| s < q.min_sats) {
                f.push("few-satellites");
            }
            if let (Some(req), Some(fx)) = (q.required_fix, r.fix) {
                if rank(fx) < rank(req) {
                    f.push("fix-degraded");
                }
            }
            if r.age.is_some_and(|x| x > q.max_diff_age_s) {
                f.push("correction-age");
            }
            if r.sigma.is_some_and(|s| s > q.horizontal_tolerance_m) {
                f.push("sigma-over-tolerance");
            }
            if len > 0.0 {
                let (x, y) = en_m(a.lat, a.lon, r.lat, r.lon);
                let xt = (x * ny - y * ex).abs() / len;
                if xt > q.horizontal_tolerance_m {
                    f.push("cross-track");
                }
            }
            f
        })
        .collect();
    let mut intervals: Vec<Interval> = Vec::new();
    let mut last_t = f64::NEG_INFINITY;
    for (r, f) in v.iter().zip(&flags) {
        if f.is_empty() {
            continue;
        }
        let join = r.t - last_t <= q.merge_s && !intervals.is_empty();
        if !join {
            intervals.push(Interval {
                start_utc: iso(r.t),
                end_utc: iso(r.t),
                duration_s: 0.0,
                epochs: 0,
                indicators: BTreeMap::new(),
                start_position: [r.lat, r.lon],
            });
        }
        let iv = intervals.last_mut().expect("pushed");
        iv.end_utc = iso(r.t);
        iv.duration_s = r.t - parse_utc(&iv.start_utc).unwrap_or(r.t);
        iv.epochs += 1;
        for k in f {
            *iv.indicators.entry(k).or_insert(0) += 1;
        }
        last_t = r.t;
    }
    let share = flags.iter().filter(|f| !f.is_empty()).count() as f64 / v.len() as f64;
    LineQc {
        line_id: id.into(),
        epochs: v.len(),
        start_utc: iso(t0),
        end_utc: iso(b.t),
        baseline_cn0_db: b_cn0,
        baseline_agc_db: b_agc,
        flagged_share: crate::forensics::inputs::round(share, 4),
        qc: if intervals.is_empty() {
            "PASS"
        } else if share <= q.reject_share {
            "DEGRADED"
        } else {
            "RE-RUN"
        },
        intervals,
    }
}

/// Run the QC on a request whose log is relative to `base`.
pub fn run(q: &Request, base: &std::path::Path) -> Result<Annex, String> {
    validate(q)?;
    let text = crate::timing::grid::read_text(base, &q.log)?;
    let lines = parse(&text).map_err(|e| format!("{}: {e}", q.log))?;
    if lines.is_empty() {
        return Err(format!("{}: no epochs", q.log));
    }
    let qcs: Vec<LineQc> = lines.iter().map(|(id, v)| qc_line(id, v, q)).collect();
    // Risk by hour and by cell: flagged epochs weighted by their spacing.
    let mut hours: BTreeMap<String, (f64, std::collections::BTreeSet<String>)> = BTreeMap::new();
    let mut cells: BTreeMap<String, (f64, std::collections::BTreeSet<String>)> = BTreeMap::new();
    for l in &qcs {
        let v = &lines[&l.line_id];
        let dt = if v.len() > 1 {
            (v[v.len() - 1].t - v[0].t) / (v.len() - 1) as f64
        } else {
            1.0
        };
        for iv in &l.intervals {
            let (Some(a), Some(b)) = (parse_utc(&iv.start_utc), parse_utc(&iv.end_utc)) else {
                continue;
            };
            for r in v.iter().filter(|r| r.t >= a && r.t <= b) {
                let h = ((r.t.rem_euclid(86_400.0)) / 3600.0).floor() as u32;
                let e = hours.entry(format!("{h:02}:00-{h:02}:59 UTC")).or_default();
                e.0 += dt;
                e.1.insert(l.line_id.clone());
                let c = crate::transport::cell(r.lat, r.lon, 0.01);
                let e = cells.entry(c).or_default();
                e.0 += dt;
                e.1.insert(l.line_id.clone());
            }
        }
    }
    let to_risk = |m: BTreeMap<String, (f64, std::collections::BTreeSet<String>)>| {
        let mut v: Vec<Risk> = m
            .into_iter()
            .map(|(k, (s, l))| Risk {
                key: k,
                flagged_s: crate::forensics::inputs::round(s, 1),
                lines: l.into_iter().collect(),
            })
            .collect();
        v.sort_by(|a, b| b.flagged_s.total_cmp(&a.flagged_s).then(a.key.cmp(&b.key)));
        v
    };
    let (risk_hours, risk_cells) = (to_risk(hours), to_risk(cells));
    let mut plan = Vec::new();
    for r in risk_hours.iter().filter(|r| r.lines.len() >= 2).take(5) {
        plan.push(format!(
            "Interference indicators recurred {} on {} line(s): schedule lines that need the tolerance outside this window or with a second positioning source.",
            r.key,
            r.lines.len()
        ));
    }
    for r in risk_cells.iter().filter(|r| r.lines.len() >= 2).take(5) {
        plan.push(format!(
            "Cell {} saw indicators on {} line(s): plan infill there with monitoring of C/N0 and AGC.",
            r.key,
            r.lines.len()
        ));
    }
    if plan.is_empty() {
        plan.push(
            "No recurring window or place: monitor C/N0, AGC and fix quality as usual.".into(),
        );
    }
    let n = |s| qcs.iter().filter(|l| l.qc == s).count();
    let verdict = format!(
        "{} line(s): {} PASS, {} DEGRADED, {} RE-RUN against {} m ({}). Engineering QC for the survey record; acceptance is the client's and the surveyor's.",
        qcs.len(),
        n("PASS"),
        n("DEGRADED"),
        n("RE-RUN"),
        q.horizontal_tolerance_m,
        q.tolerance_source
    );
    Ok(Annex {
        format: FORMAT,
        title: q.title.clone(),
        evidence: "MODELLED: rules exercised on a synthetic survey with injected interference intervals; thresholds stated in the request.",
        request: q.clone(),
        log_sha256: crate::forensics::inputs::sha256_hex(text.as_bytes()),
        lines: qcs,
        risk_hours,
        risk_cells,
        plan,
        verdict,
    })
}

/// HTML page.
pub fn html(a: &Annex) -> String {
    let rows = a
        .lines
        .iter()
        .flat_map(|l| {
            let mut v = vec![vec![
                l.line_id.clone(),
                l.qc.to_string(),
                format!("{:.2} %", 100.0 * l.flagged_share),
                String::new(),
                String::new(),
            ]];
            v.extend(l.intervals.iter().map(|i| {
                vec![
                    String::new(),
                    String::new(),
                    String::new(),
                    format!("{} to {}", i.start_utc, i.end_utc),
                    i.indicators
                        .iter()
                        .map(|(k, n)| format!("{k} ({n})"))
                        .collect::<Vec<_>>()
                        .join(", "),
                ]
            }));
            v
        })
        .collect();
    let prow = a.plan.iter().map(|p| vec![p.clone()]).collect();
    crate::transport::page(
        &a.title,
        a.evidence,
        &a.verdict,
        &[],
        &[
            (
                "Lines".into(),
                vec!["line", "QC", "flagged", "interval", "indicators (epochs)"],
                rows,
            ),
            ("Site risk plan".into(), vec!["action"], prow),
        ],
    )
}

/// A SYNTHETIC survey: four parallel 1 Hz lines with RTK fixes; interference injected on
/// lines L2 (300 s at 120 s) and L4 (20 s at 400 s), both at the same hour.
pub fn synth() -> (String, Vec<(String, f64, f64)>) {
    let t0 = 1_780_308_000.0; // 2026-06-01T10:00:00Z
    let mut s = String::from(
        "timestamp,line_id,lat,lon,fix_quality,num_sats,cn0_db,agc_db,diff_age_s,sigma_h_m\n",
    );
    let mut truth = Vec::new();
    for (li, (id, inj)) in [
        ("L1", None),
        ("L2", Some((120.0, 300.0))),
        ("L3", None),
        ("L4", Some((400.0, 20.0))),
    ]
    .iter()
    .enumerate()
    {
        let start = t0 + li as f64 * 900.0;
        if let Some((a, d)) = inj {
            truth.push((id.to_string(), start + a, start + a + d));
        }
        for k in 0..600 {
            let t = start + k as f64;
            let (la, lo) =
                crate::transport::offset_deg(51.0 + li as f64 * 0.001, 2.0, 2.0 * k as f64, 0.0);
            let jam = inj.is_some_and(|(a, d)| (k as f64) >= a && (k as f64) < a + d);
            let w = ((k * 7919 + li * 31) % 11) as f64 * 0.1 - 0.5;
            let (fix, sats, cn0, agc, sig) = if jam {
                (5, 5, 33.0 + w, 38.0, 0.6)
            } else {
                (4, 14, 45.0 + w, 42.0 + 0.1 * w, 0.03)
            };
            s.push_str(&format!(
                "{},{id},{la:.8},{lo:.8},{fix},{sats},{cn0:.1},{agc:.1},1.0,{sig}\n",
                iso(t)
            ));
        }
    }
    (s, truth)
}
