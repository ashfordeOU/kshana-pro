// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Timing-module attack recovery metric (research backlog B1304, E13; part of the timing
//! scorecards, T4)
//!
//! After a spoofing or jamming attack ends, a timing module should return to correct time:
//! how long it takes, how far off it stays, and how it leaves holdover (a phase step when it
//! re-locks is a fault for the network behind it). This module measures that from a **recorded**
//! time-error log of the module's 1PPS (or any time output) against an independent reference,
//! before, during and after a stated attack window:
//!
//! * **Baseline**: the median time error (TE) over `baseline_s` before the attack;
//! * **Attack**: the largest |TE − baseline| between onset and end;
//! * **Recovery time**: from the end of the attack to the first sample from which every sample
//!   for `hold_s` stays within `recovered_within_ns` of the baseline (`null` when it never
//!   does; "not observed" when the log ends too early to tell);
//! * **Residual offset**: the median TE − baseline over `residual_window_s` from recovery (or
//!   from the end of the log's post-attack part when it never recovered);
//! * **Holdover behaviour**, when the log carries the module's state (`locked`, `holdover`,
//!   ...): holdover entry after onset and its latency, exit, and the TE step across the exit;
//!   without states, the largest sample-to-sample step after the attack;
//! * **Multi-receiver hardening**: several modules or configurations of one site are scored
//!   side by side and ranked; with three or more, the per-sample median of the receivers (the
//!   simplest majority combination) is scored as one more configuration, so a hardening claim
//!   ("a voted set recovers faster than any single module") is checked on the logs.
//!
//! Logs: CSV (`t_s,te_ns[,state]`, header required) or any `kshana::clock_log` format (TICC,
//! TimeLab, RINEX clock; phase taken as TE). Bars are stated before scoring and hashed in.
//! Evidence: **MODELLED** until real modules are logged; checked on three synthetic fixtures with
//! closed-form answers (`tests/receiver_flag_scorecards.rs`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::scorecards::flags::sig9;
use crate::scorecards::levels::Verdict;

pub const REQUEST_FORMAT: &str = "kshana-pro-timing-recovery-request/1";
pub const FORMAT: &str = "kshana-pro-timing-recovery/1";
pub const METHOD_ID: &str = "kshana-pro/timing-recovery/v1";
pub const RESULT_FILE: &str = "timing-recovery.json";
pub const EVIDENCE: &str = "MODELLED: the metric is checked on synthetic time-error logs with \
closed-form answers; no real timing module has been logged in this release.";
pub const WORDING: &str = "Measured recovery of the logs supplied under bars stated before \
scoring. Not a statement that a module meets a standard, and not a certification.";

fn sha(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

/// The attack window, scenario seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attack {
    pub start_s: f64,
    pub end_s: f64,
}

/// Bars, stated before scoring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bars {
    pub fixed_utc: Option<String>,
    /// Recovered: |TE − baseline| within this, ns.
    pub recovered_within_ns: f64,
    /// ... held for this long, s.
    pub hold_s: f64,
    /// Baseline window before onset, s.
    pub baseline_s: f64,
    /// Residual window from recovery, s.
    pub residual_window_s: f64,
    /// Recovery bar, s.
    pub max_recovery_s: f64,
    /// Residual bar, ns (absolute residual against the baseline).
    pub max_residual_ns: f64,
    /// Largest acceptable phase step at holdover exit (or after the attack), ns.
    pub max_step_ns: f64,
}

impl Default for Bars {
    fn default() -> Self {
        Bars {
            fixed_utc: None,
            recovered_within_ns: 100.0,
            hold_s: 60.0,
            baseline_s: 300.0,
            residual_window_s: 300.0,
            max_recovery_s: 600.0,
            max_residual_ns: 50.0,
            max_step_ns: 100.0,
        }
    }
}

impl Bars {
    fn validate(&self) -> Result<(), String> {
        for (n, v) in [
            ("recovered_within_ns", self.recovered_within_ns),
            ("hold_s", self.hold_s),
            ("baseline_s", self.baseline_s),
            ("residual_window_s", self.residual_window_s),
            ("max_recovery_s", self.max_recovery_s),
            ("max_residual_ns", self.max_residual_ns),
            ("max_step_ns", self.max_step_ns),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("bars: {n} must be > 0"));
            }
        }
        Ok(())
    }
}

/// A receiver's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogRef {
    /// `csv` (`t_s,te_ns[,state]`) or a `kshana::clock_log` format (`ticc`, `timelab`,
    /// `rinex-clk`).
    pub format: String,
    pub path: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub clock: Option<String>,
}

/// One receiver (or configuration) under test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiverSpec {
    pub id: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub firmware: Option<String>,
    pub log: LogRef,
    /// Scenario time of the log's first sample (clock-log formats) or added to `t_s` (CSV), s.
    #[serde(default)]
    pub t0_s: f64,
}

/// A recovery request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub format: String,
    pub title: String,
    /// What the attack was (a test-kit scenario id, or words), for the record.
    #[serde(default)]
    pub attack_scenario: Option<String>,
    pub attack: Attack,
    #[serde(default)]
    pub bars: Bars,
    #[serde(rename = "receiver")]
    pub receivers: Vec<ReceiverSpec>,
    /// Score the per-sample median of the receivers too (needs three or more).
    #[serde(default)]
    pub ensemble_median: bool,
    #[serde(skip)]
    pub base_dir: Option<PathBuf>,
}

impl Request {
    pub fn parse(src: &str) -> Result<Request, String> {
        let r: Request =
            toml::from_str(src).map_err(|e| format!("invalid timing-recovery request: {e}"))?;
        r.validate()?;
        Ok(r)
    }

    pub fn from_file(path: &Path) -> Result<(Request, String), String> {
        let src = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r = Request::parse(&src)?;
        r.base_dir = path.parent().map(Path::to_path_buf);
        Ok((r, src))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.format != REQUEST_FORMAT {
            return Err(format!("format must be '{REQUEST_FORMAT}'"));
        }
        self.bars.validate()?;
        let a = self.attack;
        if !(a.start_s.is_finite() && a.end_s.is_finite() && a.end_s > a.start_s) {
            return Err("attack: end_s must be after start_s".into());
        }
        if self.receivers.is_empty() {
            return Err("a request has at least one [[receiver]]".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for r in &self.receivers {
            if r.id.trim().is_empty() || !ids.insert(r.id.as_str()) || r.id == ENSEMBLE {
                return Err(format!(
                    "receiver id '{}' is empty, repeated or reserved",
                    r.id
                ));
            }
        }
        if self.ensemble_median && self.receivers.len() < 3 {
            return Err("ensemble_median needs three or more receivers".into());
        }
        Ok(())
    }

    fn resolve(&self, p: &str) -> PathBuf {
        let pb = PathBuf::from(p);
        match (&self.base_dir, pb.is_absolute()) {
            (Some(b), false) => b.join(pb),
            _ => pb,
        }
    }
}

/// The id of the median-of-receivers configuration.
pub const ENSEMBLE: &str = "ensemble-median";

/// One sample of a time-error log.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub t_s: f64,
    pub te_ns: f64,
    pub state: Option<String>,
}

/// Read `t_s,te_ns[,state]` CSV (header required; `#` comments allowed).
pub fn read_csv(text: &str, t0_s: f64) -> Result<Vec<Sample>, String> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let head: Vec<String> = lines
        .next()
        .ok_or("time-error CSV is empty")?
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| head.iter().position(|h| h == n);
    let (Some(ct), Some(ce)) = (col("t_s"), col("te_ns")) else {
        return Err("time-error CSV needs columns t_s and te_ns".into());
    };
    let cs = col("state");
    let mut out = vec![];
    for (i, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let num = |c: usize| -> Result<f64, String> {
            f.get(c)
                .ok_or_else(|| format!("row {}: missing column", i + 2))?
                .parse::<f64>()
                .map_err(|e| format!("row {}: {e}", i + 2))
        };
        let (t, te) = (num(ct)? + t0_s, num(ce)?);
        if !t.is_finite() || !te.is_finite() {
            return Err(format!("row {}: values must be finite", i + 2));
        }
        if out.last().is_some_and(|s: &Sample| t < s.t_s) {
            return Err(format!("row {}: times must be non-decreasing", i + 2));
        }
        out.push(Sample {
            t_s: t,
            te_ns: te,
            state: cs
                .and_then(|c| f.get(c))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_ascii_lowercase()),
        });
    }
    if out.is_empty() {
        return Err("time-error CSV has no samples".into());
    }
    Ok(out)
}

/// Read a receiver's log.
pub fn read_log(r: &Request, rx: &ReceiverSpec) -> Result<(Vec<Sample>, Vec<u8>), String> {
    let p = r.resolve(&rx.log.path);
    let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let s = if rx.log.format == "csv" {
        read_csv(&text, rx.t0_s)?
    } else {
        let inp = kshana::clock_log::ClockLogInput {
            format: rx.log.format.clone(),
            path: None,
            text: Some(text),
            clock: rx.log.clock.clone(),
            channel: rx.log.channel.clone(),
            ..Default::default()
        };
        let rec = kshana::clock_log::read_input(&inp)?;
        rec.time_s
            .iter()
            .zip(&rec.phase_s)
            .map(|(t, x)| Sample {
                t_s: rx.t0_s + t,
                te_ns: x * 1e9,
                state: None,
            })
            .collect()
    };
    Ok((s, bytes))
}

/// A recovery criterion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    pub id: String,
    pub bar: String,
    pub measured: Option<f64>,
    pub verdict: Verdict,
}

/// One receiver's recovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceiverResult {
    pub id: String,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub log_sha256: String,
    pub samples: usize,
    pub baseline_ns: Option<f64>,
    pub attack_max_dev_ns: Option<f64>,
    /// `recovered`, `not recovered` (the log covers long enough after the attack), or
    /// `not observed` (the log ends too early).
    pub recovery: String,
    pub recovery_s: Option<f64>,
    pub residual_ns: Option<f64>,
    pub residual_abs_te_ns: Option<f64>,
    pub holdover_entry_latency_s: Option<f64>,
    pub holdover_exit_s: Option<f64>,
    pub holdover_exit_step_ns: Option<f64>,
    pub max_step_after_ns: Option<f64>,
    pub criteria: Vec<Criterion>,
}

/// A recovery report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub wording: String,
    pub title: String,
    pub attack: Attack,
    pub bars: Bars,
    pub bars_sha256: String,
    pub request_sha256: String,
    pub receivers: Vec<ReceiverResult>,
    /// Receiver ids from best to worst (recovery time, then residual).
    pub ranking: Vec<String>,
    pub hardening: Vec<String>,
    pub limits: Vec<String>,
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

fn is_holdover(s: &Sample) -> bool {
    s.state
        .as_deref()
        .is_some_and(|x| x.contains("holdover") || x == "freerun" || x == "free-run")
}

/// Score one receiver's samples.
pub fn score_samples(id: &str, samples: &[Sample], attack: Attack, bars: &Bars) -> ReceiverResult {
    let base = median(
        samples
            .iter()
            .filter(|s| s.t_s >= attack.start_s - bars.baseline_s && s.t_s < attack.start_s)
            .map(|s| s.te_ns)
            .collect(),
    );
    let dev = |s: &Sample| base.map(|b| s.te_ns - b);
    let attack_max = samples
        .iter()
        .filter(|s| s.t_s >= attack.start_s && s.t_s <= attack.end_s)
        .filter_map(|s| dev(s).map(f64::abs))
        .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
    let after: Vec<&Sample> = samples.iter().filter(|s| s.t_s >= attack.end_s).collect();
    let last_t = samples.last().map(|s| s.t_s).unwrap_or(f64::NEG_INFINITY);
    // Recovery: the first post-attack sample from which every sample over hold_s is within.
    let mut rec_t = None;
    if base.is_some() {
        for (i, s) in after.iter().enumerate() {
            if s.t_s + bars.hold_s > last_t {
                break;
            }
            let ok = after[i..]
                .iter()
                .take_while(|x| x.t_s <= s.t_s + bars.hold_s)
                .all(|x| dev(x).is_some_and(|d| d.abs() <= bars.recovered_within_ns));
            if ok {
                rec_t = Some(s.t_s);
                break;
            }
        }
    }
    let recovery = match (rec_t, base) {
        (Some(_), _) => "recovered",
        (None, Some(_)) if last_t >= attack.end_s + bars.max_recovery_s + bars.hold_s => {
            "not recovered"
        }
        _ => "not observed",
    };
    let res_from = rec_t.unwrap_or(attack.end_s + bars.max_recovery_s);
    let res: Vec<&&Sample> = after
        .iter()
        .filter(|s| s.t_s >= res_from && s.t_s <= res_from + bars.residual_window_s)
        .collect();
    let residual = median(res.iter().filter_map(|s| dev(s)).collect());
    let residual_abs = median(res.iter().map(|s| s.te_ns).collect());
    // Holdover entry and exit.
    let has_state = samples.iter().any(|s| s.state.is_some());
    let entry = samples
        .iter()
        .find(|s| s.t_s >= attack.start_s && is_holdover(s))
        .map(|s| s.t_s);
    let mut exit = None;
    let mut exit_step = None;
    if let Some(e) = entry {
        for w in samples.windows(2) {
            if w[0].t_s >= e && is_holdover(&w[0]) && !is_holdover(&w[1]) && w[1].state.is_some() {
                exit = Some(w[1].t_s);
                exit_step = Some((w[1].te_ns - w[0].te_ns).abs());
                break;
            }
        }
    }
    let step_until = rec_t.map_or(last_t, |t| t + bars.hold_s);
    let max_step_after = after
        .windows(2)
        .filter(|w| w[1].t_s <= step_until)
        .map(|w| (w[1].te_ns - w[0].te_ns).abs())
        .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
    let rec_s = rec_t.map(|t| (t - attack.end_s).max(0.0));
    let mut criteria = vec![
        Criterion {
            id: "recovery".into(),
            bar: format!(
                "within {} ns of baseline, held {} s, no later than {} s after the attack",
                bars.recovered_within_ns, bars.hold_s, bars.max_recovery_s
            ),
            measured: rec_s.map(sig9),
            verdict: match recovery {
                "recovered" => Verdict::from_bool(rec_s.map(|r| r <= bars.max_recovery_s)),
                "not recovered" => Verdict::NotMet,
                _ => Verdict::NotAssessed,
            },
        },
        Criterion {
            id: "residual".into(),
            bar: format!("|median TE − baseline| ≤ {} ns", bars.max_residual_ns),
            measured: residual.map(sig9),
            verdict: Verdict::from_bool(residual.map(|r| r.abs() <= bars.max_residual_ns)),
        },
    ];
    let step = if has_state { exit_step } else { max_step_after };
    criteria.push(Criterion {
        id: if has_state {
            "holdover-exit-step"
        } else {
            "post-attack-step"
        }
        .into(),
        bar: format!("phase step ≤ {} ns", bars.max_step_ns),
        measured: step.map(sig9),
        verdict: if has_state && entry.is_some() && exit.is_none() {
            Verdict::NotAssessed
        } else {
            Verdict::from_bool(step.map(|s| s <= bars.max_step_ns))
        },
    });
    ReceiverResult {
        id: id.into(),
        model: None,
        firmware: None,
        log_sha256: String::new(),
        samples: samples.len(),
        baseline_ns: base.map(sig9),
        attack_max_dev_ns: attack_max.map(sig9),
        recovery: recovery.into(),
        recovery_s: rec_s.map(sig9),
        residual_ns: residual.map(sig9),
        residual_abs_te_ns: residual_abs.map(sig9),
        holdover_entry_latency_s: entry.map(|e| sig9(e - attack.start_s)),
        holdover_exit_s: exit.map(sig9),
        holdover_exit_step_ns: exit_step.map(sig9),
        max_step_after_ns: max_step_after.map(sig9),
        criteria,
    }
}

/// The per-sample median of several logs on the first log's time axis (each log's nearest
/// sample within half its spacing; times where fewer than three logs have one are skipped).
pub fn median_ensemble(logs: &[Vec<Sample>]) -> Vec<Sample> {
    let Some(axis) = logs.first() else {
        return vec![];
    };
    let tol: Vec<f64> = logs
        .iter()
        .map(|l| {
            0.5 * median(
                l.windows(2)
                    .map(|w| w[1].t_s - w[0].t_s)
                    .filter(|d| *d > 0.0)
                    .collect(),
            )
            .unwrap_or(1.0)
        })
        .collect();
    axis.iter()
        .filter_map(|a| {
            let vals: Vec<f64> = logs
                .iter()
                .zip(&tol)
                .filter_map(|(l, tl)| {
                    let i = l.partition_point(|s| s.t_s < a.t_s);
                    [i.checked_sub(1), Some(i)]
                        .into_iter()
                        .flatten()
                        .filter_map(|j| l.get(j))
                        .filter(|s| (s.t_s - a.t_s).abs() <= *tl + 1e-9)
                        .min_by(|x, y| (x.t_s - a.t_s).abs().total_cmp(&(y.t_s - a.t_s).abs()))
                        .map(|s| s.te_ns)
                })
                .collect();
            (vals.len() >= 3).then(|| Sample {
                t_s: a.t_s,
                te_ns: median(vals).unwrap_or(0.0),
                state: None,
            })
        })
        .collect()
}

/// Score a request from its files.
pub fn score(r: &Request, src: &str) -> Result<Report, String> {
    let mut logs = vec![];
    for rx in &r.receivers {
        logs.push(read_log(r, rx).map_err(|e| format!("receiver {}: {e}", rx.id))?);
    }
    score_logs(r, src, &logs)
}

/// Score loaded logs: `(samples, raw bytes)` in the order of `r.receivers`.
pub fn score_logs(
    r: &Request,
    src: &str,
    logs: &[(Vec<Sample>, Vec<u8>)],
) -> Result<Report, String> {
    r.validate()?;
    if logs.len() != r.receivers.len() {
        return Err("one log per receiver".into());
    }
    let mut out = vec![];
    for (rx, (s, b)) in r.receivers.iter().zip(logs) {
        let mut x = score_samples(&rx.id, s, r.attack, &r.bars);
        x.model = rx.model.clone();
        x.firmware = rx.firmware.clone();
        x.log_sha256 = sha(b);
        out.push(x);
    }
    let mut hardening = vec![];
    if r.ensemble_median {
        let ens = median_ensemble(&logs.iter().map(|l| l.0.clone()).collect::<Vec<_>>());
        if ens.is_empty() {
            return Err("ensemble: the logs share no common sample times".into());
        }
        let mut x = score_samples(ENSEMBLE, &ens, r.attack, &r.bars);
        x.model = Some("per-sample median of the receivers".into());
        out.push(x.clone());
        let singles: Vec<&ReceiverResult> = out.iter().filter(|y| y.id != ENSEMBLE).collect();
        let worst_single = singles
            .iter()
            .map(|y| y.recovery_s.unwrap_or(f64::INFINITY))
            .fold(f64::NEG_INFINITY, f64::max);
        let best_single = singles
            .iter()
            .map(|y| y.recovery_s.unwrap_or(f64::INFINITY))
            .fold(f64::INFINITY, f64::min);
        let e = x.recovery_s.unwrap_or(f64::INFINITY);
        let f = |v: f64| {
            if v.is_finite() {
                format!("{v} s")
            } else {
                "not recovered".into()
            }
        };
        hardening.push(format!(
            "Median of {} receivers: recovery {}; single receivers from {} to {}; largest attack deviation {} ns (single receivers up to {} ns).",
            singles.len(),
            f(e),
            f(best_single),
            f(worst_single),
            x.attack_max_dev_ns.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
            singles
                .iter()
                .filter_map(|y| y.attack_max_dev_ns)
                .fold(0.0, f64::max)
        ));
        hardening.push(if e < worst_single {
            "The median configuration recovered faster than the slowest single receiver on these logs.".into()
        } else {
            "The median configuration did not recover faster than the slowest single receiver on these logs.".into()
        });
    }
    let mut ranking: Vec<&ReceiverResult> = out.iter().collect();
    ranking.sort_by(|a, b| {
        a.recovery_s
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b.recovery_s.unwrap_or(f64::INFINITY))
            .then(
                a.residual_ns
                    .map(f64::abs)
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&b.residual_ns.map(f64::abs).unwrap_or(f64::INFINITY)),
            )
            .then(a.id.cmp(&b.id))
    });
    let ranking = ranking.iter().map(|x| x.id.clone()).collect();
    Ok(Report {
        format: FORMAT.into(),
        method: METHOD_ID.into(),
        evidence: EVIDENCE.into(),
        wording: WORDING.into(),
        title: r.title.clone(),
        attack: r.attack,
        bars: r.bars.clone(),
        bars_sha256: sha(&serde_json::to_vec(&r.bars).unwrap_or_default()),
        request_sha256: sha(src.as_bytes()),
        receivers: out,
        ranking,
        hardening,
        limits: vec![
            "TE is the log's measurement against its own reference; the reference's own error is not removed.".into(),
            "The baseline is the median before onset: a module already offset before the attack recovers to its own baseline, not to zero (residual_abs_te_ns shows the absolute value).".into(),
            "Holdover states are the module's own reports where the log carries them.".into(),
            "MODELLED: checked on synthetic logs; no real timing module is scored in this release.".into(),
        ],
    })
}

/// Write `timing-recovery.json`, `request.toml` and `README.txt` (sign afterwards).
pub fn write_dir(dir: &Path, rep: &Report, src: &str) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{}: {e}", dir.display());
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(rep).unwrap_or_default() + "\n",
    )
    .map_err(io)?;
    std::fs::write(dir.join("request.toml"), src).map_err(io)?;
    let mut t = format!(
        "Timing recovery ({FORMAT}, method {METHOD_ID})\n{}\nAttack {} s to {} s\n\n{WORDING}\n\n",
        rep.title, rep.attack.start_s, rep.attack.end_s
    );
    for x in &rep.receivers {
        t += &format!(
            "{}: {} {}; residual {} ns\n",
            x.id,
            x.recovery,
            x.recovery_s
                .map(|v| format!("after {v} s"))
                .unwrap_or_default(),
            x.residual_ns
                .map(|v| v.to_string())
                .unwrap_or_else(|| "n/a".into())
        );
    }
    for h in &rep.hardening {
        t += &format!("{h}\n");
    }
    t += &format!("\nEvidence: {EVIDENCE}\n");
    std::fs::write(dir.join("README.txt"), t).map_err(io)?;
    Ok(())
}

/// Synthetic time-error logs with closed-form answers (MODELLED fixtures).
pub mod synth {
    use super::Sample;

    /// A module that follows a push of `rate` ns/s during the attack and, after it, slews back
    /// at `slew` ns/s to its baseline `b`, with an optional holdover from onset to `exit_s`.
    pub fn push_and_slew(
        b: f64,
        start: f64,
        end: f64,
        rate: f64,
        slew: f64,
        duration: f64,
        holdover_exit: Option<(f64, f64)>,
    ) -> Vec<Sample> {
        let peak = rate * (end - start);
        (0..=duration as usize)
            .map(|i| {
                let t = i as f64;
                let (te, st) = match holdover_exit {
                    // Holdover: TE frozen near its last locked value, then steps by `step` when
                    // the module re-locks at `exit`.
                    Some((exit, step)) if t >= start => {
                        if t < exit {
                            (b, "holdover")
                        } else {
                            (b + if t < exit + 1.0 { step } else { 0.0 }, "locked")
                        }
                    }
                    Some(_) => (b, "locked"),
                    None if t < start => (b, ""),
                    None if t <= end => (b + rate * (t - start), ""),
                    None => (b + (peak - slew * (t - end)).max(0.0), ""),
                };
                Sample {
                    t_s: t,
                    te_ns: te,
                    state: (!st.is_empty()).then(|| st.to_string()),
                }
            })
            .collect()
    }

    /// The samples as CSV.
    pub fn csv(s: &[Sample]) -> String {
        let mut o = String::from("t_s,te_ns,state\n");
        for x in s {
            o.push_str(&format!(
                "{},{},{}\n",
                x.t_s,
                x.te_ns,
                x.state.clone().unwrap_or_default()
            ));
        }
        o
    }
}
