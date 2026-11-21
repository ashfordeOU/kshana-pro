// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Broadcast single-frequency network timing (research 05 #109, 07b T04; backlog B3102)
//!
//! In a DAB or DVB-T single-frequency network (SFN) every transmitter sends the same symbol
//! at the same instant, timed from GNSS. A receiver in the overlap of two transmitters sees
//! both signals; their arrival-time difference must stay inside the guard interval (246 µs in
//! DAB mode I, 224 µs in DVB-T 8k 1/4 at 8 MHz), and the network plan already spends most of
//! it on the path differences of the overlap zones. A time error that differs between two
//! transmitters spends the rest. On 26 January 2016 (SVN-23) GPS broadcast UTC parameters
//! about 13 µs off: equipment that followed the error and equipment that rejected it diverged,
//! many elements entered holdover, and DAB SFNs were disturbed (research 07b [T69][T70]). The
//! lesson: resilience at the transmitter level is not resilience at UTC level.
//!
//! Two analyses over one network description (transmitters, the planned worst-case arrival
//! offset of each overlapping pair, each transmitter's reaction to a bad reference and its
//! oscillator):
//!
//! * **Monitor** the recorded logs: each transmitter's time error against an independent
//!   reference, binned; per bin the network's common-mode error (a UTC-level error: every
//!   transmitter moved together), each transmitter's departure from the others, and each
//!   pair's remaining guard margin.
//! * **Scenarios**: a UTC-level step (preset `svn23`, 13 µs), a GNSS loss (holdover from each
//!   transmitter's oscillator through the holdover evidence model,
//!   [`super::holdover_evidence::assess`]) or a slow ramp. Each transmitter follows the error
//!   (`accept`), holds over (`holdover`) or goes off air (`mute`); the result is each pair's
//!   margin with its GUM bound, the UTC-level error, the transmitters muted, and for a GNSS
//!   loss the time until the first pair runs out of margin.
//!
//! **Label: MODELLED.** Guard intervals from the published DAB and DVB-T parameters (to
//! confirm against ETSI EN 300 401 and EN 300 744); the planned offsets and reactions are the
//! operator's. Evidence for the operator, not a regulatory or licensing determination.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::grid::{self, nonneg, pos, ClockLogSpec};
use super::holdover_evidence::{self, HoldoverSpec, OscillatorSpec};

pub const FEATURE: &str = "pro-sfn-timing";
pub const FORMAT: &str = "kshana-pro-sfn-timing/1";
pub const LABEL: &str = "MODELLED: guard intervals from the published DAB and DVB-T parameters (to confirm against ETSI EN 300 401 and EN 300 744); the network plan's arrival offsets, each transmitter's reaction to a bad reference and its oscillator are the operator's; holdover from the holdover evidence model with a GUM budget; tested on synthetic logs. Evidence for the operator, not a regulatory or licensing determination.";

/// SVN-23 (26 Jan 2016): the UTC error broadcast by GPS.
pub const SVN23_STEP_S: f64 = 13e-6;
pub const SVN23_BASIS: &str = "SVN-23, 26 Jan 2016: GPS broadcast UTC parameters about 13 µs in error; one operator logged nearly 2,500 alarms and about 40 elements in holdover; DAB SFNs were disturbed (research 07b [T69][T70]). Duration here is the scenario's (illustrative default 12 h).";

/// DAB elementary period (s): 1 / 2.048 MHz.
const DAB_T: f64 = 1.0 / 2.048e6;

/// The guard interval (s) and its basis for a system name.
///
/// `dab-mode-1` .. `dab-mode-4`; `dvb-t-<2k|8k>-<1/4|1/8|1/16|1/32>` with the channel
/// bandwidth (5, 6, 7 or 8 MHz).
pub fn guard_interval(system: &str, channel_mhz: f64) -> Result<(f64, String), String> {
    let s = system.to_ascii_lowercase();
    if let Some(m) = s.strip_prefix("dab-mode-") {
        let (n, mode) = match m {
            "1" | "i" => (504.0, "I"),
            "2" | "ii" => (126.0, "II"),
            "3" | "iii" => (63.0, "III"),
            "4" | "iv" => (252.0, "IV"),
            _ => return Err(format!("unknown DAB mode '{m}' (1, 2, 3 or 4)")),
        };
        return Ok((
            n * DAB_T,
            format!("DAB transmission mode {mode}: guard {n} T, T = 1/2.048 MHz (ETSI EN 300 401, to confirm)"),
        ));
    }
    if let Some(rest) = s.strip_prefix("dvb-t-") {
        let (fft, gi) = rest
            .split_once('-')
            .ok_or_else(|| format!("expected dvb-t-<2k|8k>-<1/4|1/8|1/16|1/32>, got '{system}'"))?;
        let nfft = match fft {
            "2k" => 2048.0,
            "8k" => 8192.0,
            _ => return Err(format!("unknown DVB-T FFT size '{fft}' (2k or 8k)")),
        };
        let frac = match gi {
            "1/4" => 0.25,
            "1/8" => 0.125,
            "1/16" => 0.0625,
            "1/32" => 0.03125,
            _ => return Err(format!("unknown DVB-T guard fraction '{gi}'")),
        };
        if ![5.0, 6.0, 7.0, 8.0].contains(&channel_mhz) {
            return Err("channel_mhz must be 5, 6, 7 or 8".into());
        }
        let t = 7.0 / (8.0 * channel_mhz) * 1e-6;
        return Ok((
            nfft * t * frac,
            format!(
                "DVB-T {fft} mode, guard {gi} of Tu = {nfft} T, T = 7/{} µs at {channel_mhz} MHz (ETSI EN 300 744, to confirm)",
                8.0 * channel_mhz
            ),
        ));
    }
    Err(format!(
        "unknown system '{system}' (dab-mode-1..4, dvb-t-<2k|8k>-<1/4|1/8|1/16|1/32>, or give guard_us)"
    ))
}

/// The systems `guard_interval` knows, with their guard at 8 MHz.
pub fn systems() -> Vec<(String, f64, String)> {
    let mut v = Vec::new();
    for m in 1..=4 {
        let n = format!("dab-mode-{m}");
        let (g, b) = guard_interval(&n, 8.0).expect("known");
        v.push((n, g, b));
    }
    for f in ["2k", "8k"] {
        for gi in ["1/4", "1/8", "1/16", "1/32"] {
            let n = format!("dvb-t-{f}-{gi}");
            let (g, b) = guard_interval(&n, 8.0).expect("known");
            v.push((n, g, b));
        }
    }
    v
}

fn d_accept() -> String {
    "accept".into()
}
fn d_ch() -> f64 {
    8.0
}
fn d_utc() -> f64 {
    1e-6
}
fn d_k() -> f64 {
    2.0
}
fn d_locked() -> f64 {
    50e-9
}
fn d_bin() -> f64 {
    60.0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transmitter {
    pub name: String,
    /// What the transmitter's timing does when its GNSS reference turns bad or is lost:
    /// `accept` (follows it), `holdover` (rejects it and holds over) or `mute` (off air).
    #[serde(default = "d_accept")]
    pub on_bad_reference: String,
    /// The time error against an independent reference (any clock-log format).
    #[serde(default)]
    pub log: Option<ClockLogSpec>,
    /// The oscillator, for holdover scenarios.
    #[serde(default)]
    pub oscillator: Option<OscillatorSpec>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    pub a: String,
    pub b: String,
    /// The network plan's worst-case arrival-time difference in the pair's overlap zone,
    /// static delays included (µs).
    pub planned_offset_us: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    /// `svn23` or `gnss-loss-24h`; explicit fields override the preset's.
    #[serde(default)]
    pub preset: Option<String>,
    /// `utc-step`, `gnss-loss` or `ramp`.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub step_s: Option<f64>,
    #[serde(default)]
    pub rate_s_per_s: Option<f64>,
    #[serde(default)]
    pub duration_h: Option<f64>,
    /// The transmitters whose reference is affected (default: all).
    #[serde(default)]
    pub transmitters: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SfnRequest {
    pub network: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default = "d_ch")]
    pub channel_mhz: f64,
    /// Explicit guard interval (µs), wins over `system`.
    #[serde(default)]
    pub guard_us: Option<f64>,
    /// The UTC-level limit of the network (s).
    #[serde(default = "d_utc")]
    pub utc_limit_s: f64,
    /// Guard reserved for other items (echoes, equaliser), µs.
    #[serde(default)]
    pub extra_margin_us: f64,
    #[serde(default = "d_k")]
    pub coverage_k: f64,
    /// Standard uncertainty of a locked transmitter's time error (s).
    #[serde(default = "d_locked")]
    pub locked_u_s: f64,
    #[serde(default = "d_bin")]
    pub bin_s: f64,
    pub transmitter: Vec<Transmitter>,
    /// Overlapping pairs (default: every pair, planned offset 0).
    #[serde(default)]
    pub pair: Vec<Pair>,
    #[serde(default)]
    pub scenario: Vec<Scenario>,
}

impl SfnRequest {
    pub fn parse(src: &str) -> Result<SfnRequest, String> {
        toml::from_str(src).map_err(|e| format!("sfn request: {e}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MonitorEvent {
    /// `utc-level`, `transmitter-offset` or `pair-margin`.
    pub kind: String,
    pub subject: String,
    pub start_unix: f64,
    pub end_unix: f64,
    pub bins: usize,
    /// Worst value over the run: |common-mode| (s), |departure| (s) or the lowest margin (s).
    pub worst_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PairMonitor {
    pub a: String,
    pub b: String,
    pub planned_offset_s: f64,
    pub available_s: f64,
    pub max_abs_relative_s: f64,
    pub min_margin_s: f64,
    pub bins_beyond: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Monitor {
    pub bins: usize,
    pub first_unix: f64,
    pub last_unix: f64,
    pub max_abs_common_mode_s: f64,
    pub pairs: Vec<PairMonitor>,
    pub events: Vec<MonitorEvent>,
    /// Rows of the transmitter logs that could not be read.
    pub skipped_rows: Vec<super::selfcheck::SkippedRows>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TxOutcome {
    pub name: String,
    /// `locked`, `accepted-error`, `holdover` or `muted`.
    pub state: String,
    pub mean_s: Option<f64>,
    pub u_s: Option<f64>,
    /// |mean| + k·u against UTC.
    pub utc_bound_s: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PairOutcome {
    pub a: String,
    pub b: String,
    pub available_s: f64,
    pub relative_mean_s: Option<f64>,
    pub relative_bound_s: Option<f64>,
    pub margin_s: Option<f64>,
    /// `ok`, `self-interference-risk`, `one-muted` or `both-muted`.
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScenarioOutcome {
    pub name: String,
    pub kind: String,
    pub basis: String,
    pub step_s: Option<f64>,
    pub rate_s_per_s: Option<f64>,
    pub duration_s: f64,
    pub transmitters: Vec<TxOutcome>,
    pub pairs: Vec<PairOutcome>,
    /// Transmitters that noticed the bad reference (holdover or mute).
    pub alarms: usize,
    pub holdover_entries: usize,
    pub muted: usize,
    /// `intact` or `self-interference-risk`.
    pub sfn_status: String,
    /// `within` or `utc-level-error`.
    pub utc_status: String,
    /// `full` or `reduced`.
    pub coverage: String,
    /// GNSS loss: the holdover after which the first pair runs out of margin.
    pub first_pair_breach_s: Option<f64>,
    /// GNSS loss: the holdover after which the first transmitter passes the UTC limit.
    pub first_utc_breach_s: Option<f64>,
    pub verdict: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SfnReport {
    pub format: &'static str,
    pub label: &'static str,
    pub network: String,
    pub title: Option<String>,
    pub utc: String,
    pub guard_s: f64,
    pub guard_basis: String,
    pub extra_margin_s: f64,
    pub utc_limit_s: f64,
    pub coverage_k: f64,
    pub monitor: Option<Monitor>,
    pub scenarios: Vec<ScenarioOutcome>,
    pub verdict: String,
}

fn h(x: f64) -> String {
    super::fleet::human_time(x)
}

fn dur(s: f64) -> String {
    super::fleet::human_duration(Some(s))
}

struct Net<'a> {
    req: &'a SfnRequest,
    guard: f64,
    pairs: Vec<(usize, usize, f64)>,
}

impl Net<'_> {
    fn available(&self, planned: f64) -> f64 {
        self.guard - planned.abs() - self.req.extra_margin_us * 1e-6
    }

    /// (mean, u) of a transmitter after `d` seconds of holdover.
    fn holdover(&self, i: usize, d: f64, utc: &str) -> Result<(f64, f64), String> {
        let t = &self.req.transmitter[i];
        let osc = t.oscillator.clone().ok_or_else(|| {
            format!(
                "transmitter {}: a holdover needs its oscillator (class, adev or tdev)",
                t.name
            )
        })?;
        let hs = HoldoverSpec {
            site: t.name.clone(),
            title: None,
            preset: None,
            period_h: Some(d.max(1.0) / 3600.0),
            limit_s: Some(self.req.utc_limit_s),
            coverage_k: Some(self.req.coverage_k),
            equipment_class: None,
            initial_te_s: 0.0,
            u_initial_te_s: self.req.locked_u_s,
            oscillator: osc,
        };
        let r = holdover_evidence::assess(&hs, utc)?;
        Ok((r.mean_at_period_s, r.u_c_at_period_s))
    }
}

fn first_breach(
    horizon: f64,
    mut breached: impl FnMut(f64) -> Result<bool, String>,
) -> Result<Option<f64>, String> {
    let n = 60;
    let mut prev = 0.0;
    for i in 0..=n {
        let t = 60.0 * (horizon / 60.0).powf(i as f64 / n as f64);
        if breached(t)? {
            let (mut lo, mut hi) = (prev, t);
            for _ in 0..40 {
                let mid = (lo + hi) / 2.0;
                if breached(mid)? {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Ok(Some(hi));
        }
        prev = t;
    }
    Ok(None)
}

fn run_scenario(net: &Net, sc: &Scenario, utc: &str) -> Result<ScenarioOutcome, String> {
    let req = net.req;
    let (pkind, pstep, pdur, pbasis) = match sc.preset.as_deref() {
        None => (None, None, None, String::from("the operator's scenario")),
        Some("svn23") => (
            Some("utc-step"),
            Some(SVN23_STEP_S),
            Some(12.0),
            SVN23_BASIS.to_string(),
        ),
        Some("gnss-loss-24h") => (
            Some("gnss-loss"),
            None,
            Some(24.0),
            "illustrative: GNSS lost at every affected transmitter for 24 h".to_string(),
        ),
        Some(p) => return Err(format!("unknown preset '{p}' (svn23, gnss-loss-24h)")),
    };
    let kind = sc
        .kind
        .as_deref()
        .or(pkind)
        .ok_or_else(|| format!("scenario {}: give a kind or a preset", sc.name))?
        .to_string();
    let duration_h = sc.duration_h.or(pdur).unwrap_or(1.0);
    if !pos(duration_h) {
        return Err(format!("scenario {}: duration_h must be > 0", sc.name));
    }
    let d = duration_h * 3600.0;
    let step = sc.step_s.or(pstep);
    let rate = sc.rate_s_per_s;
    match kind.as_str() {
        "utc-step" if step.is_none() => {
            return Err(format!("scenario {}: utc-step needs step_s", sc.name))
        }
        "ramp" if rate.is_none() => {
            return Err(format!("scenario {}: ramp needs rate_s_per_s", sc.name))
        }
        "utc-step" | "ramp" | "gnss-loss" => {}
        k => {
            return Err(format!(
                "scenario {}: unknown kind '{k}' (utc-step, gnss-loss, ramp)",
                sc.name
            ))
        }
    }
    let n = req.transmitter.len();
    let affected: Vec<bool> = match &sc.transmitters {
        None => vec![true; n],
        Some(list) => {
            for x in list {
                if !req.transmitter.iter().any(|t| &t.name == x) {
                    return Err(format!("scenario {}: unknown transmitter '{x}'", sc.name));
                }
            }
            req.transmitter
                .iter()
                .map(|t| list.contains(&t.name))
                .collect()
        }
    };
    let k = req.coverage_k;
    // State of each transmitter at the end of the scenario: None = off air.
    let mut states: Vec<(String, Option<(f64, f64)>)> = Vec::with_capacity(n);
    for (i, t) in req.transmitter.iter().enumerate() {
        let locked = ("locked".to_string(), Some((0.0, req.locked_u_s)));
        let st = if !affected[i] {
            locked
        } else {
            match (kind.as_str(), t.on_bad_reference.as_str()) {
                (_, "mute") if kind != "ramp" => ("muted".to_string(), None),
                ("ramp", _) => (
                    "accepted-error".to_string(),
                    Some((rate.unwrap_or(0.0) * d, req.locked_u_s)),
                ),
                ("utc-step", "accept") => (
                    "accepted-error".to_string(),
                    Some((step.unwrap_or(0.0), req.locked_u_s)),
                ),
                (_, "holdover") | ("gnss-loss", "accept") => {
                    ("holdover".to_string(), Some(net.holdover(i, d, utc)?))
                }
                (_, r) => {
                    return Err(format!(
                        "transmitter {}: on_bad_reference '{r}' (accept, holdover or mute)",
                        t.name
                    ))
                }
            }
        };
        states.push(st);
    }
    let transmitters: Vec<TxOutcome> = req
        .transmitter
        .iter()
        .zip(&states)
        .map(|(t, (s, v))| TxOutcome {
            name: t.name.clone(),
            state: s.clone(),
            mean_s: v.map(|x| x.0),
            u_s: v.map(|x| x.1),
            utc_bound_s: v.map(|x| x.0.abs() + k * x.1),
        })
        .collect();
    let pairs: Vec<PairOutcome> = net
        .pairs
        .iter()
        .map(|&(a, b, planned)| {
            let avail = net.available(planned);
            let (sa, sb) = (&states[a].1, &states[b].1);
            let (rm, rb, m, status) = match (sa, sb) {
                (Some(x), Some(y)) => {
                    let rm = x.0 - y.0;
                    let rb = rm.abs() + k * (x.1 * x.1 + y.1 * y.1).sqrt();
                    let m = avail - rb;
                    (
                        Some(rm),
                        Some(rb),
                        Some(m),
                        if m >= 0.0 {
                            "ok"
                        } else {
                            "self-interference-risk"
                        },
                    )
                }
                (None, None) => (None, None, None, "both-muted"),
                _ => (None, None, None, "one-muted"),
            };
            PairOutcome {
                a: req.transmitter[a].name.clone(),
                b: req.transmitter[b].name.clone(),
                available_s: avail,
                relative_mean_s: rm,
                relative_bound_s: rb,
                margin_s: m,
                status: status.into(),
            }
        })
        .collect();
    let holdover_entries = transmitters
        .iter()
        .filter(|t| t.state == "holdover")
        .count();
    let muted = transmitters.iter().filter(|t| t.state == "muted").count();
    let alarms = if kind == "ramp" {
        0
    } else {
        holdover_entries + muted
    };
    let risk: Vec<String> = pairs
        .iter()
        .filter(|p| p.status == "self-interference-risk")
        .map(|p| format!("{}–{}", p.a, p.b))
        .collect();
    let utc_bad: Vec<&str> = transmitters
        .iter()
        .filter(|t| t.utc_bound_s.is_some_and(|b| b > req.utc_limit_s))
        .map(|t| t.name.as_str())
        .collect();
    // GNSS loss: how long until the first pair, and the first transmitter, runs out.
    let (first_pair, first_utc) = if kind == "gnss-loss" {
        let horizon = (30.0 * 86_400.0f64).max(d);
        let at = |dd: f64| -> Result<Vec<Option<(f64, f64)>>, String> {
            (0..n)
                .map(|i| {
                    if !affected[i] {
                        Ok(Some((0.0, req.locked_u_s)))
                    } else if req.transmitter[i].on_bad_reference == "mute" {
                        Ok(None)
                    } else {
                        net.holdover(i, dd, utc).map(Some)
                    }
                })
                .collect()
        };
        let fp = first_breach(horizon, |dd| {
            let s = at(dd)?;
            Ok(net.pairs.iter().any(|&(a, b, planned)| match (s[a], s[b]) {
                (Some(x), Some(y)) => {
                    (x.0 - y.0).abs() + k * (x.1 * x.1 + y.1 * y.1).sqrt() > net.available(planned)
                }
                _ => false,
            }))
        })?;
        let fu = first_breach(horizon, |dd| {
            let s = at(dd)?;
            Ok(s.iter()
                .flatten()
                .any(|x| x.0.abs() + k * x.1 > req.utc_limit_s))
        })?;
        (fp, fu)
    } else {
        (None, None)
    };
    let sfn_status = if risk.is_empty() {
        "intact"
    } else {
        "self-interference-risk"
    };
    let utc_status = if utc_bad.is_empty() {
        "within"
    } else {
        "utc-level-error"
    };
    let coverage = if muted > 0 { "reduced" } else { "full" };
    let mut verdict = match (sfn_status, utc_status) {
        ("intact", "utc-level-error") => format!(
            "SFN self-consistent but off UTC: {} beyond the UTC limit of {} (resilience at the transmitter level, not at UTC level)",
            utc_bad.join(", "),
            h(req.utc_limit_s)
        ),
        ("intact", _) => "SFN self-consistent and within the UTC limit".to_string(),
        (_, u) => format!(
            "pair margin exhausted (self-interference risk in the overlap zone): {}{}",
            risk.join(", "),
            if u == "utc-level-error" {
                format!("; beyond the UTC limit: {}", utc_bad.join(", "))
            } else {
                String::new()
            }
        ),
    };
    if muted > 0 {
        verdict.push_str(&format!(
            "; {muted} transmitter(s) off air (coverage reduced)"
        ));
    }
    if alarms > 0 {
        verdict.push_str(&format!(
            "; {alarms} transmitter(s) raised a reference alarm ({holdover_entries} in holdover)"
        ));
    }
    if let Some(f) = first_pair {
        verdict.push_str(&format!(
            "; first pair out of margin after {} of holdover",
            dur(f)
        ));
    }
    Ok(ScenarioOutcome {
        name: sc.name.clone(),
        kind,
        basis: pbasis,
        step_s: step,
        rate_s_per_s: rate,
        duration_s: d,
        transmitters,
        pairs,
        alarms,
        holdover_entries,
        muted,
        sfn_status: sfn_status.into(),
        utc_status: utc_status.into(),
        coverage: coverage.into(),
        first_pair_breach_s: first_pair,
        first_utc_breach_s: first_utc,
        verdict,
    })
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn push_event(
    open: &mut BTreeMap<(String, String), MonitorEvent>,
    done: &mut Vec<MonitorEvent>,
    (kind, subject, t): (&str, &str, f64),
    flagged: bool,
    worst: f64,
    lower_is_worse: bool,
) {
    let key = (kind.to_string(), subject.to_string());
    if flagged {
        let e = open.entry(key).or_insert(MonitorEvent {
            kind: kind.into(),
            subject: subject.into(),
            start_unix: t,
            end_unix: t,
            bins: 0,
            worst_s: worst,
        });
        e.end_unix = t;
        e.bins += 1;
        e.worst_s = if lower_is_worse {
            e.worst_s.min(worst)
        } else {
            e.worst_s.max(worst)
        };
    } else if let Some(e) = open.remove(&key) {
        done.push(e);
    }
}

fn monitor(net: &Net, base: &Path) -> Result<Option<Monitor>, String> {
    let req = net.req;
    let mut series: Vec<Option<Vec<super::selfcheck::Reading>>> = Vec::new();
    let mut skipped = Vec::new();
    for t in &req.transmitter {
        series.push(match &t.log {
            Some(l) => {
                let (r, sk) = grid::read_clock(&t.name, l, base)?;
                skipped.extend(sk.nonzero());
                Some(r)
            }
            None => None,
        });
    }
    if series.iter().all(Option::is_none) {
        return Ok(None);
    }
    let t0 = series
        .iter()
        .flatten()
        .filter_map(|s| s.first().map(|r| r.t))
        .fold(f64::INFINITY, f64::min);
    let n = req.transmitter.len();
    let mut bins: BTreeMap<i64, Vec<Vec<f64>>> = BTreeMap::new();
    for (i, s) in series.iter().enumerate() {
        for r in s.iter().flatten() {
            let b = ((r.t - t0) / req.bin_s).floor() as i64;
            bins.entry(b).or_insert_with(|| vec![Vec::new(); n])[i].push(r.offset);
        }
    }
    let mut open = BTreeMap::new();
    let mut events = Vec::new();
    let mut pm: Vec<PairMonitor> = net
        .pairs
        .iter()
        .map(|&(a, b, p)| PairMonitor {
            a: req.transmitter[a].name.clone(),
            b: req.transmitter[b].name.clone(),
            planned_offset_s: p,
            available_s: net.available(p),
            max_abs_relative_s: 0.0,
            min_margin_s: f64::INFINITY,
            bins_beyond: 0,
        })
        .collect();
    let mut max_common: f64 = 0.0;
    let lim = req.utc_limit_s;
    for (b, cells) in &bins {
        let t = t0 + *b as f64 * req.bin_s;
        let te: Vec<Option<f64>> = cells
            .iter()
            .map(|c| {
                let mut c = c.clone();
                (!c.is_empty()).then(|| median(&mut c))
            })
            .collect();
        let mut present: Vec<f64> = te.iter().flatten().copied().collect();
        if present.len() >= 2 {
            let common = median(&mut present);
            max_common = max_common.max(common.abs());
            push_event(
                &mut open,
                &mut events,
                ("utc-level", "network", t),
                common.abs() > lim,
                common.abs(),
                false,
            );
            for (i, x) in te.iter().enumerate() {
                let dep = x.map(|x| (x - common).abs()).unwrap_or(0.0);
                push_event(
                    &mut open,
                    &mut events,
                    ("transmitter-offset", &req.transmitter[i].name, t),
                    dep > lim,
                    dep,
                    false,
                );
            }
        }
        for (j, &(a, bb, _)) in net.pairs.iter().enumerate() {
            let (Some(x), Some(y)) = (te[a], te[bb]) else {
                continue;
            };
            // A pair's relative offset cancels the reference only if both transmitters' logs are
            // against one common reference on one time scale, with consistent signs: the
            // operator's statement; nothing here can check it.
            let rel = (x - y).abs();
            let p = &mut pm[j];
            p.max_abs_relative_s = p.max_abs_relative_s.max(rel);
            let m = p.available_s - rel;
            p.min_margin_s = p.min_margin_s.min(m);
            if m < 0.0 {
                p.bins_beyond += 1;
            }
            let subj = format!("{}–{}", p.a, p.b);
            push_event(
                &mut open,
                &mut events,
                ("pair-margin", &subj, t),
                m < 0.0,
                m,
                true,
            );
        }
    }
    events.extend(open.into_values());
    events.sort_by(|a, b| {
        a.start_unix
            .total_cmp(&b.start_unix)
            .then(a.kind.cmp(&b.kind))
    });
    let first = *bins.keys().next().expect("non-empty");
    let last = *bins.keys().last().expect("non-empty");
    Ok(Some(Monitor {
        bins: bins.len(),
        first_unix: t0 + first as f64 * req.bin_s,
        last_unix: t0 + last as f64 * req.bin_s,
        max_abs_common_mode_s: max_common,
        pairs: pm,
        events,
        skipped_rows: skipped,
    }))
}

/// Assess a network (log paths relative to `base`).
pub fn assess(req: &SfnRequest, base: &Path, utc: &str) -> Result<SfnReport, String> {
    if req.transmitter.len() < 2 {
        return Err("an SFN needs at least two transmitters".into());
    }
    let (guard, guard_basis) = match (req.guard_us, &req.system) {
        (Some(g), _) => (g * 1e-6, "the operator's guard interval".to_string()),
        (None, Some(s)) => guard_interval(s, req.channel_mhz)?,
        (None, None) => return Err("give system (e.g. dab-mode-1) or guard_us".into()),
    };
    if !pos(guard)
        || !pos(req.utc_limit_s)
        || !pos(req.coverage_k)
        || !nonneg(req.locked_u_s)
        || !pos(req.bin_s)
        || !nonneg(req.extra_margin_us)
    {
        return Err("guard, utc_limit_s, coverage_k and bin_s must be > 0; locked_u_s and extra_margin_us >= 0".into());
    }
    let idx = |n: &str| {
        req.transmitter
            .iter()
            .position(|t| t.name == n)
            .ok_or_else(|| format!("pair: unknown transmitter '{n}'"))
    };
    let mut names: Vec<&str> = req.transmitter.iter().map(|t| t.name.as_str()).collect();
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        return Err("transmitter names must be unique".into());
    }
    let pairs = if req.pair.is_empty() {
        let n = req.transmitter.len();
        (0..n)
            .flat_map(|a| (a + 1..n).map(move |b| (a, b, 0.0)))
            .collect()
    } else {
        req.pair
            .iter()
            .map(|p| Ok((idx(&p.a)?, idx(&p.b)?, p.planned_offset_us * 1e-6)))
            .collect::<Result<Vec<_>, String>>()?
    };
    let net = Net { req, guard, pairs };
    let mon = monitor(&net, base)?;
    let scenarios = req
        .scenario
        .iter()
        .map(|s| run_scenario(&net, s, utc))
        .collect::<Result<Vec<_>, _>>()?;
    let mut parts = Vec::new();
    if let Some(m) = &mon {
        let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
        for e in &m.events {
            *kinds.entry(e.kind.as_str()).or_insert(0) += 1;
        }
        parts.push(if m.events.is_empty() {
            format!(
                "recorded logs ({} bins): no UTC-level, transmitter or pair-margin event",
                m.bins
            )
        } else {
            format!(
                "recorded logs ({} bins): {}",
                m.bins,
                kinds
                    .iter()
                    .map(|(k, n)| format!("{n} {k} event(s)"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    }
    for s in &scenarios {
        parts.push(format!("{}: {}", s.name, s.verdict));
    }
    if parts.is_empty() {
        parts.push("no log and no scenario given: guard interval and pair budget only".into());
    }
    let note =
        super::selfcheck::skipped_note(mon.as_ref().map_or(&[][..], |m| &m.skipped_rows[..]));
    Ok(SfnReport {
        format: FORMAT,
        label: LABEL,
        network: req.network.clone(),
        title: req.title.clone(),
        utc: utc.into(),
        guard_s: guard,
        guard_basis,
        extra_margin_s: req.extra_margin_us * 1e-6,
        utc_limit_s: req.utc_limit_s,
        coverage_k: req.coverage_k,
        monitor: mon,
        scenarios,
        verdict: format!("{}{note}", parts.join(" | ")),
    })
}

pub fn html(r: &SfnReport) -> String {
    let mut sections = Vec::new();
    if let Some(m) = &r.monitor {
        sections.push((
            "Recorded logs: pairs".to_string(),
            vec![
                "Pair",
                "Available",
                "max |relative|",
                "min margin",
                "Bins beyond",
            ],
            m.pairs
                .iter()
                .map(|p| {
                    vec![
                        format!("{}–{}", p.a, p.b),
                        h(p.available_s),
                        h(p.max_abs_relative_s),
                        if p.min_margin_s.is_finite() {
                            h(p.min_margin_s)
                        } else {
                            "-".into()
                        },
                        p.bins_beyond.to_string(),
                    ]
                })
                .collect(),
        ));
        sections.push((
            "Recorded logs: events".to_string(),
            vec![
                "Kind",
                "Subject",
                "Start (unix)",
                "End (unix)",
                "Bins",
                "Worst",
            ],
            m.events
                .iter()
                .map(|e| {
                    vec![
                        e.kind.clone(),
                        e.subject.clone(),
                        format!("{:.0}", e.start_unix),
                        format!("{:.0}", e.end_unix),
                        e.bins.to_string(),
                        h(e.worst_s),
                    ]
                })
                .collect(),
        ));
    }
    for s in &r.scenarios {
        let mut rows: Vec<Vec<String>> = s
            .transmitters
            .iter()
            .map(|t| {
                vec![
                    t.name.clone(),
                    t.state.clone(),
                    t.mean_s.map(h).unwrap_or_else(|| "-".into()),
                    t.utc_bound_s.map(h).unwrap_or_else(|| "-".into()),
                    String::new(),
                ]
            })
            .collect();
        rows.extend(s.pairs.iter().map(|p| {
            vec![
                format!("{}–{}", p.a, p.b),
                p.status.clone(),
                p.relative_mean_s.map(h).unwrap_or_else(|| "-".into()),
                p.relative_bound_s.map(h).unwrap_or_else(|| "-".into()),
                p.margin_s.map(h).unwrap_or_else(|| "-".into()),
            ]
        }));
        sections.push((
            format!("Scenario {} ({}): {}", s.name, s.kind, s.verdict),
            vec!["Transmitter / pair", "State", "Mean", "Bound", "Margin"],
            rows,
        ));
    }
    grid::page(
        &format!("SFN timing evidence: {}", r.network),
        r.label,
        &r.verdict,
        &[format!(
            "Guard {} ({}); other items {}; UTC limit {}; k = {}; {}",
            h(r.guard_s),
            r.guard_basis,
            h(r.extra_margin_s),
            h(r.utc_limit_s),
            r.coverage_k,
            r.utc
        )],
        &sections,
    )
}

/// Write `report.json`, `report.html` and `inputs/sfn.toml`.
pub fn write_dir(out: &Path, r: &SfnReport, src: &str) -> Result<(), String> {
    grid::write_pack(out, r, &html(r), "sfn.toml", src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_values() {
        let (g, _) = guard_interval("dab-mode-1", 8.0).unwrap();
        assert!((g - 246.09375e-6).abs() < 1e-12);
        let (g, _) = guard_interval("dvb-t-8k-1/4", 8.0).unwrap();
        assert!((g - 224e-6).abs() < 1e-12);
        let (g, _) = guard_interval("dvb-t-2k-1/32", 7.0).unwrap();
        assert!((g - 8e-6).abs() < 1e-12);
        assert!(guard_interval("dvb-t2-32k", 8.0).is_err());
        assert_eq!(systems().len(), 12);
    }

    #[test]
    fn first_breach_bisects() {
        let b = first_breach(1e6, |t| Ok(t > 5000.0)).unwrap().unwrap();
        assert!((b - 5000.0).abs() < 1.0, "{b}");
        assert_eq!(first_breach(1e4, |_| Ok(false)).unwrap(), None);
    }
}
