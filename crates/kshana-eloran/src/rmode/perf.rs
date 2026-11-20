// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! Draft R-Mode receiver performance tests (research 2026-11-20 gaps, B3701).
//!
//! NCSR 13 (June 2026) agreed a draft MSC resolution on R-Mode receiver performance
//! standards. Its text is not adopted and was not read for this module, so **no limit is
//! built in**: every limit comes from a profile the user supplies (TOML), the profile states
//! where its numbers come from (`source`) and how far they were checked (`source_status`),
//! and a test whose limit is missing is reported `not_assessed`. What is built is the
//! measurement side, which does not depend on the adopted numbers: how each performance
//! parameter is estimated from receiver output records, with a one-sided confidence bound and
//! a check that the record set is large enough to show the limit at all.
//!
//! | id | parameter | statistic | decision |
//! |---|---|---|---|
//! | RMP-01 | horizontal accuracy (95 %) | 95th percentile of the error of valid fixes, distribution-free order-statistic bounds | pass when the upper bound ≤ limit, fail when the lower bound > limit |
//! | RMP-02 | availability | fraction of epochs with a valid fix (and at least `min_beacons`), one-sided Wilson bounds | pass when the lower bound ≥ limit, fail when the upper bound < limit |
//! | RMP-03 | continuity over an interval T | loss events (valid → not valid) per valid hour, Poisson upper bound, `exp(−λ_u T)` | pass when ≥ limit |
//! | RMP-04 | integrity: misleading information | valid, unalarmed epochs with error > alert limit (and > stated HPL), zero-failure bound | pass with no event and enough epochs |
//! | RMP-05 | time to alarm | from the first epoch over the alert limit to the first alarm or invalid flag | pass when every episode is within the limit; `not_exercised` with no episode |
//! | RMP-06 | update interval | largest spacing of consecutive output records | pass when ≤ limit |
//! | RMP-07 | acquisition | first record to the first valid fix within the accuracy limit | pass when ≤ limit |
//! | RMP-08 | tone-cycle ambiguity | fraction of beacon measurements with the fine cycle resolved (simulated records only) | pass when the lower bound ≥ limit |
//!
//! Records can be a receiver's own log (CSV with named columns, or JSON) or come from the MF
//! R-Mode simulator ([`simulate`]), which repeats [`super::run`] with new noise per epoch and an
//! optional night period with skywave; its alarm is a residual threshold standing in for the
//! receiver's own integrity monitor. Epochs are treated as independent unless the profile sets
//! `independence_s`, which thins the records to one per interval first.
//!
//! Evidence: **MODELLED** for simulated records. The output is a pre-assessment against the
//! limits in the supplied profile, never a statement that a receiver meets the standard.

use super::{run, RModeConfig, Skywave};
use crate::geo::{inverse, Geo};
use crate::stats::{mean_std, zero_failure_trials};
use crate::stats::{poisson_rate_upper, quantile_bounds, quantile_sorted, wilson_one_sided};
use serde::{Deserialize, Serialize};

pub const FORMAT: &str = "kshana-rmode-perf/1";

/// One receiver output record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Epoch {
    /// Time of the record (s, any origin).
    pub t_s: f64,
    /// The receiver declares the fix valid (usable).
    pub valid: bool,
    #[serde(default)]
    pub lat_deg: Option<f64>,
    #[serde(default)]
    pub lon_deg: Option<f64>,
    pub truth_lat_deg: f64,
    pub truth_lon_deg: f64,
    /// The receiver's integrity alarm.
    #[serde(default)]
    pub alarm: bool,
    /// The receiver's stated horizontal protection level (m), if it gives one.
    #[serde(default)]
    pub hpl_m: Option<f64>,
    #[serde(default)]
    pub beacons_tracked: Option<u32>,
    /// Beacon measurements with the fine tone cycle resolved, out of `beacons_tracked`
    /// (known only for simulated records).
    #[serde(default)]
    pub ambiguities_ok: Option<u32>,
}

impl Epoch {
    /// Horizontal error (m) of the fix, when there is one.
    pub fn error_m(&self) -> Option<f64> {
        match (self.lat_deg, self.lon_deg) {
            (Some(a), Some(o)) => Some(
                inverse(
                    Geo::new(a, o),
                    Geo::new(self.truth_lat_deg, self.truth_lon_deg),
                )
                .distance_m,
            ),
            _ => None,
        }
    }
}

/// The limits profile (user supplied; no default numbers).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    /// Profile name.
    pub profile: String,
    /// Where the numbers come from (document and paragraph).
    pub source: String,
    /// `placeholder`, `unverified` (from a summary) or `checked` (read in the source text).
    pub source_status: String,
    pub accuracy_95_m: Option<f64>,
    pub availability_min: Option<f64>,
    pub continuity_min: Option<f64>,
    pub continuity_interval_s: Option<f64>,
    pub alert_limit_m: Option<f64>,
    pub time_to_alarm_s: Option<f64>,
    pub max_update_interval_s: Option<f64>,
    pub acquisition_max_s: Option<f64>,
    pub min_beacons: Option<u32>,
    pub ambiguity_success_min: Option<f64>,
    /// One-sided confidence of every bound (default 0.95).
    pub confidence: Option<f64>,
    /// Keep one record per this interval before the statistics (s; 0 = all records).
    pub independence_s: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
    /// The confidence interval straddles the limit.
    Inconclusive,
    /// Too few records to show the limit even with a perfect result.
    InsufficientData,
    /// The profile gives no limit for this test.
    NotAssessed,
    /// The records contain no event that exercises the test.
    NotExercised,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub id: String,
    pub parameter: String,
    pub outcome: Outcome,
    pub limit: Option<f64>,
    /// Point estimate.
    pub estimate: Option<f64>,
    /// One-sided bounds at the profile confidence.
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
    pub samples: usize,
    /// Records needed to show the limit with a perfect result, when that applies.
    pub samples_needed: Option<usize>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerfReport {
    pub format: String,
    pub profile: String,
    pub limits_source: String,
    pub limits_source_status: String,
    pub confidence: f64,
    pub records: usize,
    pub records_used: usize,
    pub tests: Vec<TestResult>,
    pub summary: String,
    pub statement: String,
    pub evidence: String,
}

/// The test library: id and parameter.
pub const LIBRARY: &[(&str, &str)] = &[
    ("RMP-01", "horizontal accuracy (95 %)"),
    ("RMP-02", "availability"),
    ("RMP-03", "continuity"),
    ("RMP-04", "integrity: misleading information"),
    ("RMP-05", "time to alarm"),
    ("RMP-06", "update interval"),
    ("RMP-07", "acquisition"),
    ("RMP-08", "tone-cycle ambiguity resolution"),
];

fn result(
    id: &str,
    outcome: Outcome,
    limit: Option<f64>,
    samples: usize,
    note: &str,
) -> TestResult {
    TestResult {
        id: id.into(),
        parameter: LIBRARY
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, p)| p.to_string())
            .unwrap_or_default(),
        outcome,
        limit,
        estimate: None,
        lower_bound: None,
        upper_bound: None,
        samples,
        samples_needed: None,
        note: note.into(),
    }
}

/// Parse records from CSV (header with the [`Epoch`] field names; `valid`/`alarm` as
/// 1/0/true/false; empty cells are missing) or JSON (an array of records).
pub fn parse_records(text: &str) -> Result<Vec<Epoch>, String> {
    let t = text.trim_start();
    if t.starts_with('[') {
        return serde_json::from_str(t).map_err(|e| format!("records JSON: {e}"));
    }
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines
        .next()
        .ok_or("records: empty")?
        .split(',')
        .map(|h| h.trim().to_string())
        .collect();
    let col = |name: &str| header.iter().position(|h| h == name);
    let need = |name: &str| col(name).ok_or(format!("records CSV: no column {name:?}"));
    let (ct, cv, cla, clo) = (need("t_s")?, need("valid")?, col("lat_deg"), col("lon_deg"));
    let (cta, cto) = (need("truth_lat_deg")?, need("truth_lon_deg")?);
    let (cal, chpl, cb, cam) = (
        col("alarm"),
        col("hpl_m"),
        col("beacons_tracked"),
        col("ambiguities_ok"),
    );
    let mut out = vec![];
    for (i, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let cell = |c: Option<usize>| c.and_then(|c| f.get(c).copied()).filter(|v| !v.is_empty());
        let num = |c: Option<usize>| -> Result<Option<f64>, String> {
            cell(c)
                .map(|v| {
                    v.parse::<f64>()
                        .map_err(|_| format!("records line {}: {v:?} is not a number", i + 2))
                })
                .transpose()
        };
        let boolean = |c: Option<usize>| -> Result<bool, String> {
            match cell(c) {
                None | Some("0") | Some("false") | Some("no") => Ok(false),
                Some("1") | Some("true") | Some("yes") => Ok(true),
                Some(v) => Err(format!("records line {}: {v:?} is not 0/1", i + 2)),
            }
        };
        out.push(Epoch {
            t_s: num(Some(ct))?.ok_or(format!("records line {}: t_s missing", i + 2))?,
            valid: boolean(Some(cv))?,
            lat_deg: num(cla)?,
            lon_deg: num(clo)?,
            truth_lat_deg: num(Some(cta))?.ok_or(format!("records line {}: truth", i + 2))?,
            truth_lon_deg: num(Some(cto))?.ok_or(format!("records line {}: truth", i + 2))?,
            alarm: boolean(cal)?,
            hpl_m: num(chpl)?,
            beacons_tracked: num(cb)?.map(|v| v as u32),
            ambiguities_ok: num(cam)?.map(|v| v as u32),
        });
    }
    Ok(out)
}

fn thin(recs: &[Epoch], every_s: f64) -> Vec<Epoch> {
    if every_s <= 0.0 {
        return recs.to_vec();
    }
    let mut out: Vec<Epoch> = vec![];
    for r in recs {
        if out.last().is_none_or(|l| r.t_s - l.t_s >= every_s - 1e-9) {
            out.push(r.clone());
        }
    }
    out
}

/// Run the test library on the records against the profile.
pub fn assess(records: &[Epoch], lim: &Limits) -> Result<PerfReport, String> {
    if records.is_empty() {
        return Err("no records".into());
    }
    if records.windows(2).any(|w| w[1].t_s < w[0].t_s) {
        return Err("records must be in time order".into());
    }
    let conf = lim.confidence.unwrap_or(0.95);
    if !(conf > 0.5 && conf < 1.0) {
        return Err("confidence must be in (0.5, 1)".into());
    }
    let all = records;
    let recs = thin(records, lim.independence_s.unwrap_or(0.0));
    let n = recs.len();
    let mut tests = vec![];

    // RMP-01 accuracy.
    let mut errs: Vec<f64> = recs
        .iter()
        .filter(|r| r.valid)
        .filter_map(Epoch::error_m)
        .collect();
    errs.sort_by(f64::total_cmp);
    tests.push(match lim.accuracy_95_m {
        None => result("RMP-01", Outcome::NotAssessed, None, errs.len(), "profile gives no accuracy_95_m"),
        Some(l) => {
            let (lo, up) = quantile_bounds(errs.len(), 0.95, conf);
            let mut r = result("RMP-01", Outcome::InsufficientData, Some(l), errs.len(), "");
            r.estimate = quantile_sorted(&errs, 0.95);
            r.lower_bound = lo.map(|i| errs[i]);
            r.upper_bound = up.map(|i| errs[i]);
            r.samples_needed = zero_failure_trials(0.95, conf);
            r.outcome = match (r.lower_bound, r.upper_bound) {
                (_, Some(u)) if u <= l => Outcome::Pass,
                (Some(lb), _) if lb > l => Outcome::Fail,
                (_, Some(_)) => Outcome::Inconclusive,
                _ => Outcome::InsufficientData,
            };
            r.note = "distribution-free order-statistic bounds on the 95th percentile of valid-fix errors".into();
            r
        }
    });

    // RMP-02 availability.
    let min_b = lim.min_beacons.unwrap_or(0);
    let avail = recs
        .iter()
        .filter(|r| r.valid && r.beacons_tracked.unwrap_or(u32::MAX) >= min_b)
        .count();
    tests.push(bounded_fraction(
        "RMP-02",
        avail,
        n,
        lim.availability_min,
        conf,
        "valid fixes over all records",
    ));

    // RMP-03 continuity: loss events per valid time (over all records, not thinned).
    tests.push(match (lim.continuity_min, lim.continuity_interval_s) {
        (Some(cmin), Some(ti)) => {
            let mut valid_time = 0.0;
            let mut losses = 0usize;
            for w in all.windows(2) {
                if w[0].valid {
                    valid_time += w[1].t_s - w[0].t_s;
                    if !w[1].valid {
                        losses += 1;
                    }
                }
            }
            let lam_u = poisson_rate_upper(losses, valid_time, conf);
            let mut r = result("RMP-03", Outcome::Fail, Some(cmin), all.len(), "");
            r.estimate = Some((-(losses as f64) / valid_time.max(1e-9) * ti).exp());
            r.lower_bound = Some((-lam_u * ti).exp());
            // Exposure that shows the limit with no loss.
            let need_t = poisson_rate_upper(0, 1.0, conf) * ti / -(cmin.ln());
            r.samples_needed = Some(need_t.ceil() as usize);
            r.outcome = if r.lower_bound.unwrap() >= cmin {
                Outcome::Pass
            } else if r.estimate.unwrap() < cmin && losses > 0 {
                Outcome::Fail
            } else if losses == 0 {
                Outcome::InsufficientData
            } else {
                Outcome::Inconclusive
            };
            r.note = format!(
                "{losses} loss events in {valid_time:.0} s of valid output; samples_needed is the loss-free valid time (s) that shows the limit"
            );
            r
        }
        _ => result("RMP-03", Outcome::NotAssessed, None, all.len(), "profile gives no continuity_min and continuity_interval_s"),
    });

    // RMP-04 misleading information.
    tests.push(match lim.alert_limit_m {
        None => result("RMP-04", Outcome::NotAssessed, None, n, "profile gives no alert_limit_m"),
        Some(al) => {
            let used: Vec<&Epoch> = recs.iter().filter(|r| r.valid && !r.alarm).collect();
            let mi = used
                .iter()
                .filter(|r| {
                    r.error_m().is_some_and(|e| e > al && r.hpl_m.is_none_or(|h| e > h))
                })
                .count();
            let mut r = result("RMP-04", Outcome::Pass, Some(0.0), used.len(), "");
            r.estimate = Some(mi as f64);
            // Upper bound on the MI probability per epoch.
            r.upper_bound = wilson_one_sided(mi, used.len(), conf).map(|b| b.1);
            if mi == 0 && !used.is_empty() {
                r.upper_bound = Some(1.0 - (1.0 - conf).powf(1.0 / used.len() as f64));
            }
            r.outcome = if mi > 0 { Outcome::Fail } else if used.is_empty() { Outcome::InsufficientData } else { Outcome::Pass };
            r.note = format!(
                "{mi} valid, unalarmed epochs with error over the alert limit{}; upper_bound is the per-epoch probability bound (the draft sets the integrity risk the data must show)",
                if used.iter().any(|r| r.hpl_m.is_some()) { " and the stated HPL" } else { "" }
            );
            r
        }
    });

    // RMP-05 time to alarm (all records, not thinned).
    tests.push(match (lim.alert_limit_m, lim.time_to_alarm_s) {
        (Some(al), Some(tta)) => {
            let mut worst: Option<f64> = None;
            let mut episodes = 0usize;
            let mut unalarmed = 0usize;
            let mut i = 0;
            while i < all.len() {
                let over = |r: &Epoch| r.valid && r.error_m().is_some_and(|e| e > al);
                if over(&all[i]) && (i == 0 || !over(&all[i - 1])) {
                    episodes += 1;
                    let t0 = all[i].t_s;
                    match all[i..].iter().find(|r| r.alarm || !r.valid) {
                        Some(a) => worst = Some(worst.unwrap_or(0.0).max(a.t_s - t0)),
                        None => unalarmed += 1,
                    }
                }
                i += 1;
            }
            let mut r = result("RMP-05", Outcome::NotExercised, Some(tta), episodes, "");
            r.estimate = worst;
            r.outcome = if episodes == 0 {
                Outcome::NotExercised
            } else if unalarmed > 0 || worst.is_some_and(|w| w > tta) {
                Outcome::Fail
            } else {
                Outcome::Pass
            };
            r.note = format!("{episodes} episodes over the alert limit, {unalarmed} never alarmed; estimate is the worst time to alarm (s)");
            r
        }
        _ => result("RMP-05", Outcome::NotAssessed, None, 0, "profile gives no alert_limit_m and time_to_alarm_s"),
    });

    // RMP-06 update interval.
    let gaps: Vec<f64> = all.windows(2).map(|w| w[1].t_s - w[0].t_s).collect();
    let max_gap = gaps.iter().copied().fold(f64::NAN, f64::max);
    tests.push(match lim.max_update_interval_s {
        None => result(
            "RMP-06",
            Outcome::NotAssessed,
            None,
            gaps.len(),
            "profile gives no max_update_interval_s",
        ),
        Some(l) if gaps.is_empty() => result(
            "RMP-06",
            Outcome::InsufficientData,
            Some(l),
            0,
            "one record",
        ),
        Some(l) => {
            let mut r = result(
                "RMP-06",
                if max_gap <= l {
                    Outcome::Pass
                } else {
                    Outcome::Fail
                },
                Some(l),
                gaps.len(),
                "largest spacing of consecutive records (s)",
            );
            r.estimate = Some(max_gap);
            r.upper_bound = Some(max_gap);
            r.lower_bound = Some(mean_std(&gaps).0);
            r.note =
                "estimate and upper_bound: largest spacing; lower_bound: mean spacing (s)".into();
            r
        }
    });

    // RMP-07 acquisition.
    tests.push(match (lim.acquisition_max_s, lim.accuracy_95_m) {
        (Some(l), acc) => {
            let t0 = all[0].t_s;
            let first = all
                .iter()
                .find(|r| r.valid && r.error_m().is_some_and(|e| acc.is_none_or(|a| e <= a)));
            let mut r = result(
                "RMP-07",
                Outcome::Fail,
                Some(l),
                all.len(),
                "first record to the first valid fix within the accuracy limit (s)",
            );
            r.estimate = first.map(|f| f.t_s - t0);
            r.outcome = match r.estimate {
                Some(t) if t <= l => Outcome::Pass,
                _ => Outcome::Fail,
            };
            r
        }
        (None, _) => result(
            "RMP-07",
            Outcome::NotAssessed,
            None,
            all.len(),
            "profile gives no acquisition_max_s",
        ),
    });

    // RMP-08 ambiguity.
    let (ok, tot) = recs.iter().fold((0usize, 0usize), |(a, b), r| {
        match (r.ambiguities_ok, r.beacons_tracked) {
            (Some(o), Some(t)) => (a + o as usize, b + t as usize),
            _ => (a, b),
        }
    });
    tests.push(if tot == 0 {
        result(
            "RMP-08",
            Outcome::NotExercised,
            lim.ambiguity_success_min,
            0,
            "records carry no ambiguity truth (simulated records only)",
        )
    } else {
        bounded_fraction(
            "RMP-08",
            ok,
            tot,
            lim.ambiguity_success_min,
            conf,
            "beacon measurements with the fine tone cycle resolved",
        )
    });

    let count = |o: Outcome| tests.iter().filter(|t| t.outcome == o).count();
    let summary = format!(
        "{} pass, {} fail, {} inconclusive, {} insufficient data, {} not exercised, {} not assessed",
        count(Outcome::Pass),
        count(Outcome::Fail),
        count(Outcome::Inconclusive),
        count(Outcome::InsufficientData),
        count(Outcome::NotExercised),
        count(Outcome::NotAssessed)
    );
    Ok(PerfReport {
        format: FORMAT.into(),
        profile: lim.profile.clone(),
        limits_source: lim.source.clone(),
        limits_source_status: if lim.source_status.is_empty() { "placeholder".into() } else { lim.source_status.clone() },
        confidence: conf,
        records: records.len(),
        records_used: n,
        tests,
        summary,
        statement: "Pre-assessment against the limits of the supplied profile; the draft R-Mode receiver performance standard (NCSR 13, June 2026) is not adopted and its limits are not built in. Not a statement that the receiver meets any standard.".into(),
        evidence: "MODELLED for simulated records; for a receiver log, the statistics are only as good as the truth reference recorded with it".into(),
    })
}

fn bounded_fraction(
    id: &str,
    k: usize,
    n: usize,
    limit: Option<f64>,
    conf: f64,
    what: &str,
) -> TestResult {
    let Some(l) = limit else {
        return result(id, Outcome::NotAssessed, None, n, "profile gives no limit");
    };
    let mut r = result(id, Outcome::InsufficientData, Some(l), n, what);
    if n == 0 {
        return r;
    }
    r.estimate = Some(k as f64 / n as f64);
    let (lo, up) = wilson_one_sided(k, n, conf).unwrap();
    r.lower_bound = Some(lo);
    r.upper_bound = Some(up);
    r.samples_needed = if l < 1.0 {
        zero_failure_trials(l, conf)
    } else {
        None
    };
    r.outcome = if lo >= l {
        Outcome::Pass
    } else if up < l {
        Outcome::Fail
    } else if r.samples_needed.is_some_and(|m| n < m) {
        Outcome::InsufficientData
    } else {
        Outcome::Inconclusive
    };
    r
}

/// Simulation plan: repeat the MF R-Mode bench with new noise per epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimPlan {
    pub bench: RModeConfig,
    pub epochs: usize,
    #[serde(default = "d_interval")]
    pub interval_s: f64,
    /// Epochs from this index on are at night (skywave on all paths).
    #[serde(default)]
    pub night_from: Option<usize>,
    #[serde(default)]
    pub night_skywave: Option<Skywave>,
    /// The receiver's integrity monitor stand-in: alarm when the fix residual RMS exceeds this (m).
    #[serde(default)]
    pub alarm_residual_m: Option<f64>,
}

fn d_interval() -> f64 {
    1.0
}

/// Run the simulator per epoch and return the records (MODELLED).
pub fn simulate(plan: &SimPlan) -> Result<Vec<Epoch>, String> {
    if plan.epochs == 0 || plan.epochs > 100_000 {
        return Err("epochs must be 1..=100000".into());
    }
    let mut out = Vec::with_capacity(plan.epochs);
    for i in 0..plan.epochs {
        let mut c = plan.bench.clone();
        c.seed = plan.bench.seed.wrapping_add(i as u64 * 7919);
        if plan.night_from.is_some_and(|k| i >= k) {
            c.skywave = plan.night_skywave.clone().or(c.skywave);
        }
        let r = run(&c)?;
        let alarm = plan.alarm_residual_m.is_some_and(|a| r.residual_rms_m > a);
        out.push(Epoch {
            t_s: i as f64 * plan.interval_s,
            valid: !alarm,
            lat_deg: Some(r.fix_lat_deg),
            lon_deg: Some(r.fix_lon_deg),
            truth_lat_deg: c.rx_lat_deg,
            truth_lon_deg: c.rx_lon_deg,
            alarm,
            hpl_m: None,
            beacons_tracked: Some(r.beacons.len() as u32),
            ambiguities_ok: Some(r.beacons.iter().filter(|b| b.ambiguity_ok).count() as u32),
        });
    }
    Ok(out)
}

/// Records as CSV (the format [`parse_records`] reads).
pub fn records_csv(recs: &[Epoch]) -> String {
    let o = |v: Option<f64>| v.map(|x| format!("{x}")).unwrap_or_default();
    let mut s = String::from("t_s,valid,lat_deg,lon_deg,truth_lat_deg,truth_lon_deg,alarm,hpl_m,beacons_tracked,ambiguities_ok\n");
    for r in recs {
        s += &format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            r.t_s,
            r.valid as u8,
            o(r.lat_deg),
            o(r.lon_deg),
            r.truth_lat_deg,
            r.truth_lon_deg,
            r.alarm as u8,
            o(r.hpl_m),
            r.beacons_tracked.map(|v| v.to_string()).unwrap_or_default(),
            r.ambiguities_ok.map(|v| v.to_string()).unwrap_or_default()
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(t: f64, valid: bool, err_m: f64, alarm: bool) -> Epoch {
        // 1e-5 deg of latitude ≈ 1.106 m.
        Epoch {
            t_s: t,
            valid,
            lat_deg: Some(55.0 + err_m / 111_000.0),
            lon_deg: Some(13.0),
            truth_lat_deg: 55.0,
            truth_lon_deg: 13.0,
            alarm,
            hpl_m: None,
            beacons_tracked: Some(4),
            ambiguities_ok: None,
        }
    }

    fn lim() -> Limits {
        Limits {
            profile: "test".into(),
            accuracy_95_m: Some(10.0),
            availability_min: Some(0.95),
            alert_limit_m: Some(25.0),
            time_to_alarm_s: Some(10.0),
            max_update_interval_s: Some(2.0),
            acquisition_max_s: Some(60.0),
            ..Default::default()
        }
    }

    fn get(r: &PerfReport, id: &str) -> TestResult {
        r.tests.iter().find(|t| t.id == id).unwrap().clone()
    }

    #[test]
    fn clean_records_pass_and_small_sets_are_insufficient() {
        let recs: Vec<Epoch> = (0..300)
            .map(|i| rec(i as f64, true, (i % 7) as f64, false))
            .collect();
        let r = assess(&recs, &lim()).unwrap();
        assert_eq!(
            get(&r, "RMP-01").outcome,
            Outcome::Pass,
            "{:?}",
            get(&r, "RMP-01")
        );
        assert_eq!(get(&r, "RMP-02").outcome, Outcome::Pass);
        assert_eq!(get(&r, "RMP-04").outcome, Outcome::Pass);
        assert_eq!(get(&r, "RMP-05").outcome, Outcome::NotExercised);
        assert_eq!(get(&r, "RMP-03").outcome, Outcome::NotAssessed);
        let small = assess(&recs[..30], &lim()).unwrap();
        assert_eq!(get(&small, "RMP-01").outcome, Outcome::InsufficientData);
        assert!(r.statement.contains("Pre-assessment"));
    }

    #[test]
    fn misleading_information_and_late_alarm_fail() {
        let mut recs: Vec<Epoch> = (0..200).map(|i| rec(i as f64, true, 2.0, false)).collect();
        // Error 40 m from t = 100, alarm only at t = 115.
        for r in recs.iter_mut().skip(100).take(20) {
            *r = rec(r.t_s, true, 40.0, r.t_s >= 115.0);
        }
        let r = assess(&recs, &lim()).unwrap();
        assert_eq!(get(&r, "RMP-04").outcome, Outcome::Fail);
        let tta = get(&r, "RMP-05");
        assert_eq!(tta.outcome, Outcome::Fail);
        assert!((tta.estimate.unwrap() - 15.0).abs() < 1e-9);
    }

    #[test]
    fn continuity_bound_and_csv_round_trip() {
        let mut recs: Vec<Epoch> = (0..3600).map(|i| rec(i as f64, true, 1.0, false)).collect();
        recs[1800].valid = false;
        let mut l = lim();
        l.continuity_min = Some(0.9);
        l.continuity_interval_s = Some(60.0);
        let r = assess(&recs, &l).unwrap();
        let c = get(&r, "RMP-03");
        assert!(c.estimate.unwrap() > 0.98 && c.lower_bound.unwrap() < c.estimate.unwrap());
        let back = parse_records(&records_csv(&recs)).unwrap();
        assert_eq!(back.len(), recs.len());
        assert!(!back[1800].valid);
    }

    #[test]
    fn simulated_records_degrade_at_night() {
        let bk = |id: &str, lat: f64, lon: f64, f: f64| super::super::Beacon {
            id: id.into(),
            lat_deg: lat,
            lon_deg: lon,
            carrier_hz: f,
            sync_error_ns: 0.0,
            tone_amp: 0.3,
            ground_sigma: None,
            ground_eps_r: None,
        };
        let plan = SimPlan {
            bench: RModeConfig {
                beacons: vec![
                    bk("A", 55.6, 12.6, 296.5e3),
                    bk("B", 54.2, 12.1, 298.5e3),
                    bk("C", 54.7, 14.9, 303.5e3),
                    bk("D", 56.1, 15.6, 309.5e3),
                ],
                rx_lat_deg: 55.0,
                rx_lon_deg: 13.8,
                prior_lat_deg: Some(55.0015),
                prior_lon_deg: Some(13.8015),
                integration_s: 1.0,
                ..Default::default()
            },
            epochs: 4,
            interval_s: 10.0,
            night_from: Some(2),
            night_skywave: Some(Skywave {
                excess_delay_us: 60.0,
                rel_amp: 0.5,
                phase_rad: 1.0,
            }),
            alarm_residual_m: None,
        };
        let recs = simulate(&plan).unwrap();
        let day: f64 = recs[..2].iter().filter_map(Epoch::error_m).sum::<f64>() / 2.0;
        let night: f64 = recs[2..].iter().filter_map(Epoch::error_m).sum::<f64>() / 2.0;
        assert!(night > day, "day {day} night {night}");
        let r = assess(&recs, &lim()).unwrap();
        assert_ne!(get(&r, "RMP-08").outcome, Outcome::NotExercised);
    }
}
