// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Neutral clock benchmarks under a pre-registered protocol (E33 B3302)
//!
//! Research 07b (T13), 08 (C6, C7, C10) and 12 (#1669, #1677): buyers of optical, caesium,
//! rubidium, chip-scale atomic, quartz and MEMS clocks compare them on figures each maker states
//! its own way, and quantum-clock programmes ask for validation scorecards that no maker writes.
//! This module scores several clocks on the **same stated metrics, from their own logs, under a
//! protocol fixed before the data**:
//!
//! * **Pre-registration.** The `[protocol]` table (metrics, averaging times, confidence level,
//!   measurement reference and its stability, holdover test, environmental phases) is hashed in
//!   a canonical JSON form ([`protocol_sha256`]); `kshana-pro timing clock-bench register` prints
//!   the hash and the canonical text for the owner to publish or sign before any data is taken.
//!   Scoring refuses a request whose protocol does not hash to the registered value, so the
//!   protocol cannot be changed after the results are seen. Each clock may state when its data
//!   collection began; the report says whether that is after the registration time (as stated
//!   by the operator; not checked against the logs).
//! * **Metrics per clock and phase** (from the open engine's clock-log readers and Allan-family
//!   estimators): overlapping Allan deviation with a chi-squared confidence interval
//!   (conservative degrees of freedom), time deviation (TDEV), maximum time interval error
//!   (MTIE), a holdover test (frequency estimated on the first `holdover_train_s` of the log,
//!   the prediction error over the next `holdover_window_s` and the time it first reaches the
//!   threshold), and for each environmental phase (temperature, vibration, shock, at sea, ...)
//!   the mean frequency shift from the baseline phase and the Allan deviation ratio to it.
//! * **Reference floor.** A point whose Allan deviation is below `floor_factor` times the
//!   reference's stated stability is flagged `reference_limited`: it measures the reference as
//!   much as the clock and is not reported as the clock's figure. This is what keeps an optical
//!   clock measured against a microwave reference from being under- or over-stated.
//! * **Datasheet claims.** Where a clock states datasheet Allan deviation points, the baseline
//!   measurement over the claim is reported as a ratio, as a fact about that unit and that log.
//!
//! The scorecard lists clocks in input order with the same metrics side by side. **It does not
//! rank, score against each other, or name a winner**, and it says nothing about a maker beyond
//! the document a clock's claims come from.
//!
//! **Label: COMPUTED** from the logs as supplied under the registered protocol; the holdover
//! test is a linear-frequency prediction (MODELLED). Tested on synthetic logs only. Not a type
//! approval, an endorsement or a statement that a unit meets any requirement.

use std::collections::BTreeMap;
use std::path::Path;

use kshana::allan::{conservative_edf, deviation_ci, mtie, overlapping_adev, time_deviation};
use kshana::clock_log::{read_input, ClockLogInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::sig9;

pub const FEATURE: &str = "pro-timing-clock-bench";
pub const FORMAT: &str = "kshana-pro-clock-bench/1";
pub const PROTOCOL_FORMAT: &str = "kshana-pro-clock-bench-protocol/1";
pub const LABEL: &str = "COMPUTED from the clock logs as supplied, under the pre-registered protocol named by its hash; the holdover test is a linear-frequency prediction (MODELLED). Clocks are listed in input order and compared on the stated metrics only; no ranking. Tested on synthetic logs; not a type approval or an endorsement.";
/// The phase every other phase is compared with.
pub const BASELINE: &str = "baseline";
/// The metrics a protocol may name.
pub const METRICS: [&str; 6] = [
    "adev",
    "tdev",
    "mtie",
    "holdover",
    "sensitivity",
    "datasheet",
];

/// The protocol, fixed before the data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protocol {
    /// Identifier.
    pub id: String,
    /// Title.
    pub title: String,
    /// When the protocol was registered (ISO 8601 UTC).
    pub registered_utc: String,
    /// The measurement reference, described (for example a hydrogen maser steered to UTC(k)).
    pub reference: String,
    /// The reference's stated Allan deviation `[tau_s, sigma_y]`.
    pub reference_adev: Vec<(f64, f64)>,
    /// Averaging times, s.
    pub taus: Vec<f64>,
    /// Confidence level of the intervals (default 0.95).
    #[serde(default = "d_conf")]
    pub confidence: f64,
    /// A point is reference-limited below this multiple of the reference (default 3).
    #[serde(default = "d_floor")]
    pub floor_factor: f64,
    /// Holdover test: training length, s.
    #[serde(default)]
    pub holdover_train_s: f64,
    /// Holdover test: prediction window, s.
    #[serde(default)]
    pub holdover_window_s: f64,
    /// Holdover test: time-error threshold, s.
    #[serde(default)]
    pub holdover_threshold_s: f64,
    /// The phases data will be taken in; must include `baseline`.
    pub phases: Vec<String>,
    /// The metrics reported (a subset of [`METRICS`]).
    pub metrics: Vec<String>,
}

fn d_conf() -> f64 {
    0.95
}
fn d_floor() -> f64 {
    3.0
}

/// One phase's log of one clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    /// The phase (one of the protocol's).
    pub phase: String,
    /// The log, read by the open engine's clock-log readers; a `path` is relative to the request.
    pub log: ClockLogInput,
}

/// One clock under test.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockSpec {
    /// Identifier.
    pub id: String,
    /// Technology: optical, caesium, rubidium, hydrogen-maser, csac, ocxo, tcxo, mems, quartz or
    /// other.
    pub technology: String,
    /// The document the datasheet claims come from.
    #[serde(default)]
    pub datasheet: String,
    /// Datasheet Allan deviation claims `[tau_s, sigma_y]`.
    #[serde(default)]
    pub datasheet_adev: Vec<(f64, f64)>,
    /// When data collection began, as stated by the operator (ISO 8601 UTC).
    #[serde(default)]
    pub collected_from_utc: Option<String>,
    /// The logs, one per phase.
    pub segment: Vec<Segment>,
}

/// A benchmark request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchSpec {
    /// The protocol.
    pub protocol: Protocol,
    /// The protocol hash recorded at registration.
    pub protocol_sha256: String,
    /// The clocks.
    pub clock: Vec<ClockSpec>,
}

/// The canonical JSON of a protocol: its fields in declaration order, numbers as serde writes
/// them.
pub fn canonical_protocol(p: &Protocol) -> String {
    let mut v = serde_json::to_value(p).expect("serialises");
    if let Some(o) = v.as_object_mut() {
        o.insert("format".into(), PROTOCOL_FORMAT.into());
    }
    // The field order is fixed by the struct (sorted by key, or in declaration order when
    // serde_json keeps insertion order), so the text is the same for the same protocol.
    serde_json::to_string(&v).expect("serialises")
}

/// SHA-256 of [`canonical_protocol`], hex.
pub fn protocol_sha256(p: &Protocol) -> String {
    hex::encode(Sha256::digest(canonical_protocol(p).as_bytes()))
}

/// Check a protocol is usable.
pub fn validate_protocol(p: &Protocol) -> Result<(), String> {
    if p.taus.is_empty() || p.taus.len() > 64 {
        return Err("protocol.taus: give 1 to 64 averaging times".into());
    }
    if p.taus.iter().any(|t| !(t.is_finite() && *t > 0.0)) {
        return Err("protocol.taus: every averaging time must be positive".into());
    }
    if p.reference_adev.is_empty()
        || p.reference_adev
            .iter()
            .any(|(t, s)| !(t.is_finite() && *t > 0.0 && s.is_finite() && *s > 0.0))
    {
        return Err("protocol.reference_adev: give the reference's stability as positive [tau_s, sigma_y] points".into());
    }
    if !(p.confidence > 0.0 && p.confidence < 1.0) {
        return Err("protocol.confidence must be between 0 and 1".into());
    }
    if !(p.floor_factor.is_finite() && p.floor_factor >= 1.0) {
        return Err("protocol.floor_factor must be at least 1".into());
    }
    if !p.phases.iter().any(|x| x == BASELINE) {
        return Err("protocol.phases must include baseline".into());
    }
    if p.metrics.is_empty() {
        return Err("protocol.metrics: name at least one metric".into());
    }
    for m in &p.metrics {
        if !METRICS.contains(&m.as_str()) {
            return Err(format!(
                "protocol.metrics: unknown metric {m} (one of {})",
                METRICS.join(", ")
            ));
        }
    }
    if p.metrics.iter().any(|m| m == "holdover")
        && !(p.holdover_train_s > 0.0 && p.holdover_window_s > 0.0 && p.holdover_threshold_s > 0.0)
    {
        return Err("the holdover metric needs holdover_train_s, holdover_window_s and holdover_threshold_s above zero".into());
    }
    Ok(())
}

/// Log-log interpolation of `[tau, value]` points; flat beyond the ends.
pub fn interp_loglog(points: &[(f64, f64)], tau: f64) -> f64 {
    let mut p = points.to_vec();
    p.sort_by(|a, b| a.0.total_cmp(&b.0));
    if tau <= p[0].0 {
        return p[0].1;
    }
    for w in p.windows(2) {
        if tau <= w[1].0 {
            let f = (tau / w[0].0).ln() / (w[1].0 / w[0].0).ln();
            return (w[0].1.ln() + f * (w[1].1 / w[0].1).ln()).exp();
        }
    }
    p[p.len() - 1].1
}

/// An Allan deviation with its chi-squared confidence interval.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AdevCi {
    /// The estimate.
    pub dev: f64,
    /// Lower bound.
    pub lo: f64,
    /// Upper bound.
    pub hi: f64,
    /// Effective degrees of freedom.
    pub edf: f64,
}

/// One averaging time of one phase.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TauRow {
    /// Averaging time, s.
    pub tau_s: f64,
    /// Overlapping Allan deviation and its interval (`null`: the log is too short).
    pub adev: Option<AdevCi>,
    /// Time deviation, s.
    pub tdev_s: Option<f64>,
    /// MTIE, s.
    pub mtie_s: Option<f64>,
    /// The reference's stated Allan deviation at this averaging time.
    pub reference_adev: f64,
    /// Whether the clock's Allan deviation is below `floor_factor` times the reference's.
    pub reference_limited: bool,
}

/// The holdover test of one phase.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HoldoverTest {
    /// Fractional frequency estimated on the training span.
    pub trained_freq: f64,
    /// Largest |prediction error| over the window, s.
    pub max_error_s: f64,
    /// First time after the training span the error reaches the threshold, s (`null`: never).
    pub time_to_threshold_s: Option<f64>,
}

/// One phase of one clock.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PhaseResult {
    /// The phase.
    pub phase: String,
    /// Samples used.
    pub samples: usize,
    /// Sample interval, s.
    pub tau0_s: f64,
    /// Gaps in the log.
    pub gaps: usize,
    /// SHA-256 of the normalised phase samples.
    pub phase_sha256: String,
    /// Mean fractional frequency over the log.
    pub mean_freq: f64,
    /// The averaging-time rows.
    pub taus: Vec<TauRow>,
    /// The holdover test (`null`: not in the protocol, or the log is shorter than training plus
    /// window).
    pub holdover: Option<HoldoverTest>,
    /// Mean frequency shift from the baseline phase (`null` for the baseline).
    pub freq_shift_from_baseline: Option<f64>,
    /// Allan deviation ratio to the baseline phase at each averaging time (`null` where either
    /// is missing).
    pub adev_ratio_to_baseline: Vec<Option<f64>>,
}

/// One datasheet claim against the baseline measurement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClaimCheck {
    /// Averaging time, s.
    pub tau_s: f64,
    /// The claim.
    pub claimed: f64,
    /// The baseline measurement at this averaging time (`null`: too short).
    pub measured: Option<f64>,
    /// measured / claimed.
    pub ratio: Option<f64>,
    /// Whether the measurement's interval is reference-limited here.
    pub reference_limited: bool,
}

/// One clock's results.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClockResult {
    /// Identifier.
    pub id: String,
    /// Technology.
    pub technology: String,
    /// The datasheet document.
    pub datasheet: String,
    /// Whether data collection began after registration, as stated (`null`: not stated).
    pub collected_after_registration: Option<bool>,
    /// Per phase.
    pub phases: Vec<PhaseResult>,
    /// Datasheet claims against the baseline.
    pub claims: Vec<ClaimCheck>,
}

/// The side-by-side table: one row per metric, phase and averaging time, one column per clock.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SideBySide {
    /// Metric.
    pub metric: String,
    /// Phase.
    pub phase: String,
    /// Averaging time, s (`null` for whole-phase metrics).
    pub tau_s: Option<f64>,
    /// Value per clock id, in input order (`null`: not available or reference-limited).
    pub values: Vec<(String, Option<f64>)>,
}

/// The scorecard.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BenchReport {
    /// Format tag.
    pub format: &'static str,
    /// Label.
    pub label: &'static str,
    /// When the scorecard was made (ISO 8601 UTC).
    pub utc: String,
    /// The protocol.
    pub protocol: Protocol,
    /// Its registered hash (checked).
    pub protocol_sha256: String,
    /// Per clock.
    pub clocks: Vec<ClockResult>,
    /// The side-by-side table.
    pub table: Vec<SideBySide>,
}

fn has(p: &Protocol, m: &str) -> bool {
    p.metrics.iter().any(|x| x == m)
}

fn holdover_test(x: &[f64], tau0: f64, p: &Protocol) -> Option<HoldoverTest> {
    let nt = (p.holdover_train_s / tau0).round() as usize;
    let nw = (p.holdover_window_s / tau0).round() as usize;
    if nt < 2 || nw < 1 || x.len() < nt + nw + 1 {
        return None;
    }
    // Least-squares line on the training span.
    let n = nt as f64;
    let (mut st, mut sx, mut stt, mut stx) = (0.0, 0.0, 0.0, 0.0);
    for (i, &xi) in x[..nt].iter().enumerate() {
        let t = i as f64 * tau0;
        st += t;
        sx += xi;
        stt += t * t;
        stx += t * xi;
    }
    let den = n * stt - st * st;
    if den == 0.0 {
        return None;
    }
    let y = (n * stx - st * sx) / den;
    let c = (sx - y * st) / n;
    let mut max_e: f64 = 0.0;
    let mut first = None;
    for (k, &xk) in x.iter().enumerate().take(nt + nw + 1).skip(nt) {
        let t = k as f64 * tau0;
        let e = (xk - (c + y * t)).abs();
        max_e = max_e.max(e);
        if first.is_none() && e >= p.holdover_threshold_s {
            first = Some((k - nt) as f64 * tau0);
        }
    }
    Some(HoldoverTest {
        trained_freq: sig9(y),
        max_error_s: sig9(max_e),
        time_to_threshold_s: first,
    })
}

fn phase_result(
    seg: &Segment,
    base: &Path,
    p: &Protocol,
) -> Result<(PhaseResult, Vec<Option<AdevCi>>), String> {
    let mut log = seg.log.clone();
    if let Some(path) = &log.path {
        log.path = Some(base.join(path).to_string_lossy().into_owned());
    }
    let np = read_input(&log)?;
    let x = &np.phase_s;
    let tau0 = np.tau0_s;
    if x.len() < 3 {
        return Err(format!("phase {}: fewer than three samples", seg.phase));
    }
    let span = (x.len() - 1) as f64 * tau0;
    let mean_freq = (x[x.len() - 1] - x[0]) / span;
    let mut rows = Vec::new();
    let mut adevs = Vec::new();
    for &tau in &p.taus {
        let m = (tau / tau0).round() as usize;
        let ok = m >= 1 && x.len() > 2 * m + 1;
        let adev = if ok {
            let d = overlapping_adev(x, tau0, m);
            let edf = conservative_edf(x.len(), m);
            (d.is_finite() && edf > 0.0).then(|| {
                let ci = deviation_ci(d, edf, p.confidence);
                AdevCi {
                    dev: sig9(ci.dev),
                    lo: sig9(ci.lo),
                    hi: sig9(ci.hi),
                    edf: sig9(ci.edf),
                }
            })
        } else {
            None
        };
        let r = interp_loglog(&p.reference_adev, tau);
        let limited = adev.as_ref().is_some_and(|a| a.dev < p.floor_factor * r);
        let tdev =
            (has(p, "tdev") && m >= 1 && x.len() > 3 * m).then(|| sig9(time_deviation(x, tau0, m)));
        let mt = (has(p, "mtie") && m >= 1 && x.len() > m).then(|| sig9(mtie(x, m)));
        adevs.push(adev.clone());
        rows.push(TauRow {
            tau_s: tau,
            adev: if has(p, "adev") || has(p, "datasheet") || has(p, "sensitivity") {
                adev
            } else {
                None
            },
            tdev_s: tdev,
            mtie_s: mt,
            reference_adev: r,
            reference_limited: limited,
        });
    }
    let holdover = if has(p, "holdover") {
        holdover_test(x, tau0, p)
    } else {
        None
    };
    Ok((
        PhaseResult {
            phase: seg.phase.clone(),
            samples: x.len(),
            tau0_s: tau0,
            gaps: np.n_gaps,
            phase_sha256: np.phase_sha256(),
            mean_freq: sig9(mean_freq),
            taus: rows,
            holdover,
            freq_shift_from_baseline: None,
            adev_ratio_to_baseline: vec![],
        },
        adevs,
    ))
}

/// Score a request. `base` resolves log paths; `utc` stamps the report.
pub fn score(spec: &BenchSpec, base: &Path, utc: &str) -> Result<BenchReport, String> {
    let p = &spec.protocol;
    validate_protocol(p)?;
    let h = protocol_sha256(p);
    if !h.eq_ignore_ascii_case(spec.protocol_sha256.trim()) {
        return Err(format!(
            "the protocol does not hash to the registered value (registered {}, now {h}): it was changed after registration",
            spec.protocol_sha256.trim()
        ));
    }
    if spec.clock.is_empty() || spec.clock.len() > 64 {
        return Err("give 1 to 64 clocks".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut clocks = Vec::new();
    for c in &spec.clock {
        if !ids.insert(c.id.clone()) {
            return Err(format!("clock {} is listed twice", c.id));
        }
        let mut seen = std::collections::BTreeSet::new();
        for s in &c.segment {
            if !p.phases.contains(&s.phase) {
                return Err(format!(
                    "clock {}: phase {} is not in the registered protocol",
                    c.id, s.phase
                ));
            }
            if !seen.insert(s.phase.clone()) {
                return Err(format!("clock {}: phase {} is given twice", c.id, s.phase));
            }
        }
        let mut phases = Vec::new();
        let mut adevs: BTreeMap<String, Vec<Option<AdevCi>>> = BTreeMap::new();
        for s in &c.segment {
            let (r, a) = phase_result(s, base, p).map_err(|e| format!("clock {}: {e}", c.id))?;
            adevs.insert(s.phase.clone(), a);
            phases.push(r);
        }
        let base_freq = phases
            .iter()
            .find(|r| r.phase == BASELINE)
            .map(|r| r.mean_freq);
        let base_adev = adevs.get(BASELINE).cloned();
        if has(p, "sensitivity") {
            for r in phases.iter_mut().filter(|r| r.phase != BASELINE) {
                r.freq_shift_from_baseline = base_freq.map(|b| sig9(r.mean_freq - b));
                let mine = &adevs[&r.phase];
                r.adev_ratio_to_baseline = (0..p.taus.len())
                    .map(|i| {
                        let a = mine[i].as_ref()?;
                        let b = base_adev.as_ref()?[i].as_ref()?;
                        Some(sig9(a.dev / b.dev))
                    })
                    .collect();
            }
        }
        let claims = if has(p, "datasheet") {
            c.datasheet_adev
                .iter()
                .map(|&(tau, claimed)| {
                    let i = p.taus.iter().position(|t| (t - tau).abs() <= 1e-9 * tau);
                    let meas = i.and_then(|i| base_adev.as_ref()?[i].clone());
                    let r = interp_loglog(&p.reference_adev, tau);
                    ClaimCheck {
                        tau_s: tau,
                        claimed,
                        measured: meas.as_ref().map(|m| m.dev),
                        ratio: meas.as_ref().map(|m| sig9(m.dev / claimed)),
                        reference_limited: meas.is_some_and(|m| m.dev < p.floor_factor * r),
                    }
                })
                .collect()
        } else {
            vec![]
        };
        let after = c
            .collected_from_utc
            .as_deref()
            .map(|t| t.trim() >= p.registered_utc.trim());
        clocks.push(ClockResult {
            id: c.id.clone(),
            technology: c.technology.clone(),
            datasheet: c.datasheet.clone(),
            collected_after_registration: after,
            phases,
            claims,
        });
    }
    let table = side_by_side(p, &clocks);
    Ok(BenchReport {
        format: FORMAT,
        label: LABEL,
        utc: utc.to_string(),
        protocol: p.clone(),
        protocol_sha256: h,
        clocks,
        table,
    })
}

fn side_by_side(p: &Protocol, clocks: &[ClockResult]) -> Vec<SideBySide> {
    let mut out = Vec::new();
    for ph in &p.phases {
        let get = |c: &ClockResult| c.phases.iter().find(|r| &r.phase == ph).cloned();
        for (i, &tau) in p.taus.iter().enumerate() {
            for (metric, wanted) in [("adev", "adev"), ("tdev_s", "tdev"), ("mtie_s", "mtie")] {
                if !has(p, wanted) {
                    continue;
                }
                let values = clocks
                    .iter()
                    .map(|c| {
                        let v = get(c).and_then(|r| {
                            let row = &r.taus[i];
                            if row.reference_limited {
                                return None;
                            }
                            match metric {
                                "adev" => row.adev.as_ref().map(|a| a.dev),
                                "tdev_s" => row.tdev_s,
                                _ => row.mtie_s,
                            }
                        });
                        (c.id.clone(), v)
                    })
                    .collect();
                out.push(SideBySide {
                    metric: metric.into(),
                    phase: ph.clone(),
                    tau_s: Some(tau),
                    values,
                });
            }
        }
        if has(p, "holdover") {
            out.push(SideBySide {
                metric: "holdover_max_error_s".into(),
                phase: ph.clone(),
                tau_s: None,
                values: clocks
                    .iter()
                    .map(|c| {
                        let v = get(c).and_then(|r| r.holdover.map(|h| h.max_error_s));
                        (c.id.clone(), v)
                    })
                    .collect(),
            });
        }
        if has(p, "sensitivity") && ph != BASELINE {
            out.push(SideBySide {
                metric: "freq_shift_from_baseline".into(),
                phase: ph.clone(),
                tau_s: None,
                values: clocks
                    .iter()
                    .map(|c| {
                        (
                            c.id.clone(),
                            get(c).and_then(|r| r.freq_shift_from_baseline),
                        )
                    })
                    .collect(),
            });
        }
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fmt(v: Option<f64>) -> String {
    v.map(|x| format!("{x:.3e}")).unwrap_or_else(|| "-".into())
}

/// The side-by-side table as CSV: `metric,phase,tau_s,<clock ids...>`.
pub fn csv(r: &BenchReport) -> String {
    let mut o = String::from("metric,phase,tau_s");
    for c in &r.clocks {
        o.push(',');
        o.push_str(&c.id.replace([',', '\n'], " "));
    }
    o.push('\n');
    for row in &r.table {
        o.push_str(&format!(
            "{},{},{}",
            row.metric,
            row.phase,
            row.tau_s.map(|t| t.to_string()).unwrap_or_default()
        ));
        for (_, v) in &row.values {
            o.push(',');
            if let Some(v) = v {
                o.push_str(&format!("{v:e}"));
            }
        }
        o.push('\n');
    }
    o
}

/// The scorecard as HTML.
pub fn html(r: &BenchReport) -> String {
    let mut o = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Clock benchmark: {}</title>\
<style>body{{font-family:system-ui,sans-serif;max-width:64rem;margin:2rem auto;padding:0 1rem;color:#111;background:#fff}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #ccc;padding:.3rem;text-align:left}}small{{color:#555}}</style></head><body>\
<h1>Clock benchmark: {}</h1><p>{}</p><p>Protocol <code>{}</code>, registered {}, SHA-256 <code>{}</code>. Reference: {}.</p>\
<p>Clocks are listed in input order; a dash is a value the log cannot give or a point limited by the reference.</p>",
        esc(&r.protocol.title),
        esc(&r.protocol.title),
        esc(r.label),
        esc(&r.protocol.id),
        esc(&r.protocol.registered_utc),
        esc(&r.protocol_sha256),
        esc(&r.protocol.reference),
    );
    o.push_str("<table><tr><th>Metric</th><th>Phase</th><th>τ (s)</th>");
    for c in &r.clocks {
        o.push_str(&format!(
            "<th>{}<br><small>{}</small></th>",
            esc(&c.id),
            esc(&c.technology)
        ));
    }
    o.push_str("</tr>");
    for row in &r.table {
        o.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td>",
            esc(&row.metric),
            esc(&row.phase),
            row.tau_s.map(|t| t.to_string()).unwrap_or_default()
        ));
        for (_, v) in &row.values {
            o.push_str(&format!("<td>{}</td>", fmt(*v)));
        }
        o.push_str("</tr>");
    }
    o.push_str("</table>");
    for c in &r.clocks {
        if c.claims.is_empty() {
            continue;
        }
        o.push_str(&format!(
            "<h2>{}: datasheet claims</h2><p><small>{}</small></p><table><tr><th>τ (s)</th><th>Claimed</th><th>Measured</th><th>Ratio</th><th>Reference-limited</th></tr>",
            esc(&c.id),
            esc(&c.datasheet)
        ));
        for k in &c.claims {
            o.push_str(&format!(
                "<tr><td>{}</td><td>{:.3e}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                k.tau_s,
                k.claimed,
                fmt(k.measured),
                k.ratio
                    .map(|x| format!("{x:.3}"))
                    .unwrap_or_else(|| "-".into()),
                if k.reference_limited { "yes" } else { "no" }
            ));
        }
        o.push_str("</table>");
    }
    o.push_str("</body></html>\n");
    o
}

/// Write the evidence directory: `scorecard.json`, `scorecard.html`, `table.csv`,
/// `protocol.json` (the canonical registered text) and `inputs/bench.toml`.
pub fn write_dir(out: &Path, r: &BenchReport, src: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", out.display());
    std::fs::create_dir_all(out.join("inputs")).map_err(io)?;
    std::fs::write(
        out.join("scorecard.json"),
        serde_json::to_string_pretty(r).expect("serialises"),
    )
    .map_err(io)?;
    std::fs::write(out.join("scorecard.html"), html(r)).map_err(io)?;
    std::fs::write(out.join("table.csv"), csv(r)).map_err(io)?;
    std::fs::write(out.join("protocol.json"), canonical_protocol(&r.protocol)).map_err(io)?;
    std::fs::write(out.join("inputs").join("bench.toml"), src).map_err(io)?;
    Ok(())
}
