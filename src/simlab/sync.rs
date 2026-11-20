// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Synchronous multi-sensor replay check (format [`FORMAT`]).
//!
//! An integrated receiver (GNSS + IMU + odometry + camera) can only be replay-tested when
//! every stream is played back on one time base. The streams were recorded on their own
//! clocks; this module aligns them through **shared events** (a tap seen by the IMU and the
//! camera, a PPS marker in the CAN log and the I/Q recording, a light flash) and says, per
//! stream, whether the alignment holds within the tolerance:
//!
//! ```toml
//! format = "kshana-simlab-replay-set/1"
//! id = "drive-1"
//! reference = "gnss-iq"          # the stream whose clock is the common time base
//! tolerance_ms = 1.0
//! [[stream]]
//! name = "gnss-iq"
//! kind = "iq"                    # iq | imu | can | camera | odometry | other
//! clock = "gnss"                 # gnss | pps-disciplined | ptp | host | free-running
//! sigmf = "drive.sigmf-meta"     # or start_s, sample_rate_hz and samples
//! start_s = 1000.0
//! [[stream]]
//! name = "imu"
//! kind = "imu"
//! clock = "free-running"
//! timestamps = "imu.csv"         # first column: time on the stream's clock, s
//! [[event]]
//! name = "tap-1"
//! at = { gnss-iq = 1010.0, imu = 10.0123 }
//! ```
//!
//! Per stream: start, end, the nominal rate (median interval), the jitter (standard
//! deviation of the intervals, gaps excluded), gaps (an interval above 2.5 × the median) and
//! non-monotonic stamps. Against the reference: with two or more shared events, a
//! least-squares line `t_ref = a + b·t_stream` (offset `a`, drift `(b − 1)·10⁶` ppm) and the
//! residuals at the events; with one event, the offset only, and the drift bounded by the
//! clock class (free-running 50 ppm, host 100 ppm, disciplined 1 ppb; representative
//! values) over the stream's duration; with none, the stream is aligned only when both
//! clocks are disciplined to GNSS time (gnss, pps-disciplined, ptp), at the declared
//! `clock_offset_s`.
//!
//! Status per stream: `aligned`, `aligned-offset-only`, `misaligned` (residual or drift
//! bound above the tolerance), `unsynchronisable` (no shared event and no common clock);
//! jitter above the tolerance is a further finding. The **replay manifest** gives the common
//! window on the reference clock and, per stream, the correction and the start (seconds, and
//! the sample index for I/Q). `sensors_synchronised` (for `testkit fidelity`) is true only
//! when every non-I/Q stream is `aligned`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{median, numeric_csv, r, std_dev};
use crate::aviation::Inputs;

pub const FORMAT: &str = "kshana-simlab-replay-set/1";
pub const METHOD: &str = "kshana-pro/simlab-replay-sync/v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamDef {
    pub name: String,
    pub kind: String,
    pub clock: String,
    #[serde(default)]
    pub sigmf: Option<String>,
    #[serde(default)]
    pub start_s: Option<f64>,
    #[serde(default)]
    pub sample_rate_hz: Option<f64>,
    #[serde(default)]
    pub samples: Option<f64>,
    #[serde(default)]
    pub timestamps: Option<String>,
    /// Known offset to the reference clock when no shared event exists (common clocks).
    #[serde(default)]
    pub clock_offset_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub name: String,
    pub at: BTreeMap<String, f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplaySet {
    pub format: String,
    pub id: String,
    pub reference: String,
    pub tolerance_ms: f64,
    #[serde(rename = "stream")]
    pub streams: Vec<StreamDef>,
    #[serde(default, rename = "event")]
    pub events: Vec<Event>,
}

const KINDS: [&str; 6] = ["iq", "imu", "can", "camera", "odometry", "other"];

/// Drift bound (fractional) of a clock class; `None` for an unknown class.
pub fn drift_bound(clock: &str) -> Option<f64> {
    Some(match clock {
        "gnss" | "pps-disciplined" | "ptp" => 1e-9,
        "host" => 100e-6,
        "free-running" => 50e-6,
        _ => return None,
    })
}

fn disciplined(clock: &str) -> bool {
    matches!(clock, "gnss" | "pps-disciplined" | "ptp")
}

/// One stream's time line on its own clock.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub start: f64,
    pub end: f64,
    pub rate_hz: f64,
    pub jitter_s: f64,
    pub gaps: usize,
    pub max_gap_s: f64,
    pub non_monotonic: usize,
    pub samples: f64,
}

/// Bytes per sample of a SigMF `core:datatype` (`cf32_le`, `ci16_le`, `ru8`, …).
pub fn sigmf_bytes_per_sample(dt: &str) -> Option<u64> {
    let t = dt.split('_').next()?;
    let complex = match t.chars().next()? {
        'c' => 2,
        'r' => 1,
        _ => return None,
    };
    let bits: u64 = t.get(2..)?.parse().ok()?;
    if !matches!(bits, 8 | 16 | 32 | 64) {
        return None;
    }
    Some(complex * bits / 8)
}

/// Unix seconds of `YYYY-MM-DDTHH:MM:SS[.f]Z`.
pub fn iso_to_unix(s: &str) -> Result<f64, String> {
    let day = kshana_eloran::calreg::day_number(s)?;
    let t = s
        .get(11..)
        .ok_or_else(|| format!("bad time {s:?}"))?
        .trim_end_matches('Z');
    let p: Vec<&str> = t.split(':').collect();
    if p.len() != 3 {
        return Err(format!("bad time {s:?}"));
    }
    let n = |x: &str| x.parse::<f64>().map_err(|_| format!("bad time {s:?}"));
    Ok(day as f64 * 86_400.0 + n(p[0])? * 3600.0 + n(p[1])? * 60.0 + n(p[2])?)
}

fn uniform(start: f64, rate: f64, samples: f64) -> Timeline {
    Timeline {
        start,
        end: start + samples / rate,
        rate_hz: rate,
        jitter_s: 0.0,
        gaps: 0,
        max_gap_s: 0.0,
        non_monotonic: 0,
        samples,
    }
}

/// The time line of a timestamp column.
pub fn timeline_of(times: &[f64]) -> Result<Timeline, String> {
    if times.len() < 3 {
        return Err("fewer than 3 timestamps".into());
    }
    let dts: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let non_monotonic = dts.iter().filter(|d| **d <= 0.0).count();
    let pos: Vec<f64> = dts.iter().copied().filter(|d| *d > 0.0).collect();
    let med = median(&pos);
    if med.is_nan() || med <= 0.0 {
        return Err("no increasing timestamps".into());
    }
    let gaps: Vec<f64> = pos.iter().copied().filter(|d| *d > 2.5 * med).collect();
    let regular: Vec<f64> = pos.iter().copied().filter(|d| *d <= 2.5 * med).collect();
    let lo = times.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = times.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(Timeline {
        start: lo,
        end: hi,
        rate_hz: 1.0 / med,
        jitter_s: std_dev(&regular),
        gaps: gaps.len(),
        max_gap_s: gaps.iter().copied().fold(0.0, f64::max),
        non_monotonic,
        samples: times.len() as f64,
    })
}

/// A loaded set: definitions and time lines.
pub struct Loaded {
    pub set: ReplaySet,
    pub timelines: Vec<Timeline>,
    pub inputs: Inputs,
}

/// Read a replay set and every file it names.
pub fn load(path: &Path) -> Result<Loaded, String> {
    let (src, mut inputs) = super::read_request(path, "replay-set")?;
    let set: ReplaySet = toml::from_str(&src).map_err(|e| format!("replay set: {e}"))?;
    if set.format != FORMAT {
        return Err(format!(
            "replay set: format '{}' (expected {FORMAT})",
            set.format
        ));
    }
    if set.tolerance_ms.is_nan() || set.tolerance_ms <= 0.0 {
        return Err("replay set: tolerance_ms must be > 0".into());
    }
    if !set.streams.iter().any(|s| s.name == set.reference) {
        return Err(format!(
            "replay set: reference '{}' is not a stream",
            set.reference
        ));
    }
    let base = super::base_of(path);
    let mut timelines = Vec::new();
    for s in &set.streams {
        if !KINDS.contains(&s.kind.as_str()) {
            return Err(format!("stream {}: unknown kind '{}'", s.name, s.kind));
        }
        if drift_bound(&s.clock).is_none() {
            return Err(format!("stream {}: unknown clock '{}'", s.name, s.clock));
        }
        if set.streams.iter().filter(|x| x.name == s.name).count() > 1 {
            return Err(format!("stream {}: name used twice", s.name));
        }
        let tl = if let Some(ts) = &s.timestamps {
            let text = inputs.read(base, ts, &format!("timestamps:{}", s.name))?;
            let (rows, _) = numeric_csv(&text, 1);
            let t: Vec<f64> = rows.iter().map(|r| r[0]).collect();
            timeline_of(&t).map_err(|e| format!("stream {}: {e}", s.name))?
        } else if let Some(meta) = &s.sigmf {
            let text = inputs.read(base, meta, &format!("sigmf-meta:{}", s.name))?;
            let v: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("{meta}: {e}"))?;
            let rate = v["global"]["core:sample_rate"]
                .as_f64()
                .ok_or_else(|| format!("{meta}: no core:sample_rate"))?;
            let dt = v["global"]["core:datatype"].as_str().unwrap_or("");
            let bps = sigmf_bytes_per_sample(dt)
                .ok_or_else(|| format!("{meta}: unsupported core:datatype '{dt}'"))?;
            let data = base.join(meta.replace(".sigmf-meta", ".sigmf-data"));
            let bytes = std::fs::metadata(&data)
                .map_err(|e| format!("{}: {e}", data.display()))?
                .len();
            let start = match s.start_s {
                Some(t) => t,
                None => iso_to_unix(
                    v["captures"][0]["core:datetime"]
                        .as_str()
                        .ok_or_else(|| format!("{meta}: no start_s and no core:datetime"))?,
                )?,
            };
            uniform(start, rate, (bytes / bps) as f64)
        } else {
            match (s.start_s, s.sample_rate_hz, s.samples) {
                (Some(t), Some(rt), Some(n)) if rt > 0.0 && n > 0.0 => uniform(t, rt, n),
                _ => {
                    return Err(format!(
                        "stream {}: needs timestamps, sigmf, or start_s + sample_rate_hz + samples",
                        s.name
                    ))
                }
            }
        };
        timelines.push(tl);
    }
    for e in &set.events {
        for k in e.at.keys() {
            if !set.streams.iter().any(|s| &s.name == k) {
                return Err(format!("event {}: unknown stream '{k}'", e.name));
            }
        }
    }
    Ok(Loaded {
        set,
        timelines,
        inputs,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamResult {
    pub name: String,
    pub kind: String,
    pub clock: String,
    pub start_s: f64,
    pub end_s: f64,
    pub rate_hz: f64,
    pub jitter_ms: f64,
    pub gaps: usize,
    pub max_gap_s: f64,
    pub non_monotonic: usize,
    pub shared_events: usize,
    /// Correction to the reference clock: `t_ref = offset_s + (1 + drift_ppm·1e-6)·t`.
    pub offset_s: f64,
    pub drift_ppm: f64,
    /// Largest residual at the shared events (two or more events), ms.
    pub max_residual_ms: Option<f64>,
    /// Bound of the uncorrected drift over the stream (one event or none), ms.
    pub drift_bound_ms: Option<f64>,
    pub status: String,
    pub findings: Vec<String>,
    /// Replay start on the stream's own clock (window start mapped back), s.
    pub replay_start_s: Option<f64>,
    /// I/Q only: the sample index of the replay start.
    pub replay_start_sample: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncReport {
    pub method: &'static str,
    pub label: &'static str,
    pub evidence: &'static str,
    pub id: String,
    pub reference: String,
    pub tolerance_ms: f64,
    /// `synchronised`, `limited` or `not-synchronised`.
    pub verdict: String,
    pub sensors_synchronised: bool,
    /// Common window on the reference clock, `[start, end]` s; `None` when the streams do
    /// not overlap.
    pub window_s: Option<[f64; 2]>,
    pub streams: Vec<StreamResult>,
    pub inputs: Vec<crate::aviation::InputRef>,
}

fn fit(pairs: &[(f64, f64)]) -> (f64, f64) {
    let n = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let b = if sxx > 0.0 { sxy / sxx } else { 1.0 };
    (my - b * mx, b)
}

/// Check the alignment and write the manifest.
pub fn check(l: &Loaded) -> SyncReport {
    let set = &l.set;
    let tol = set.tolerance_ms / 1e3;
    let rdef = set
        .streams
        .iter()
        .find(|s| s.name == set.reference)
        .unwrap();
    let mut out = Vec::new();
    for (s, tl) in set.streams.iter().zip(&l.timelines) {
        let pairs: Vec<(f64, f64)> = set
            .events
            .iter()
            .filter_map(|e| Some((*e.at.get(&s.name)?, *e.at.get(&set.reference)?)))
            .collect();
        let dur = tl.end - tl.start;
        let mut findings = Vec::new();
        let (a, b, max_res, bound, mut status) = if s.name == set.reference {
            (0.0, 1.0, None, None, "aligned".to_string())
        } else if pairs.len() >= 2 {
            let (a, b) = fit(&pairs);
            let mr = pairs
                .iter()
                .map(|(x, y)| (y - (a + b * x)).abs())
                .fold(0.0, f64::max);
            if pairs.len() == 2 {
                findings.push(
                    "two shared events: the line passes through both, so the residual is zero by construction; add a third to check it".into(),
                );
            }
            let st = if mr <= tol { "aligned" } else { "misaligned" };
            (a, b, Some(mr), None, st.to_string())
        } else if pairs.len() == 1 {
            let a = pairs[0].1 - pairs[0].0;
            let db = drift_bound(&s.clock)
                .unwrap()
                .max(drift_bound(&rdef.clock).unwrap())
                * dur;
            let st = if db <= tol {
                "aligned-offset-only"
            } else {
                findings.push(format!(
                    "one shared event and a {} clock: the drift over {:.1} s may reach {:.3} ms; add a second event near the end",
                    s.clock,
                    dur,
                    db * 1e3
                ));
                "misaligned"
            };
            (a, 1.0, None, Some(db), st.to_string())
        } else if disciplined(&s.clock) && disciplined(&rdef.clock) {
            findings.push(format!(
                "no shared event: aligned through the common {} time base at the declared clock_offset_s",
                s.clock
            ));
            let db = 2e-9 * dur;
            (
                s.clock_offset_s,
                1.0,
                None,
                Some(db),
                "aligned-offset-only".into(),
            )
        } else {
            findings.push(
                "no shared event and no common disciplined clock: the stream cannot be placed on the reference time base".into(),
            );
            (0.0, 1.0, None, None, "unsynchronisable".into())
        };
        if tl.jitter_s > tol {
            findings.push(format!(
                "timestamp jitter {:.3} ms exceeds the tolerance",
                tl.jitter_s * 1e3
            ));
            if status == "aligned" {
                status = "aligned-offset-only".into();
            }
        }
        if tl.gaps > 0 {
            findings.push(format!(
                "{} gap(s), longest {:.3} s: the replay holds the last value through them",
                tl.gaps, tl.max_gap_s
            ));
        }
        if tl.non_monotonic > 0 {
            findings.push(format!(
                "{} non-increasing timestamp(s): the recorder's clock stepped or the log is out of order",
                tl.non_monotonic
            ));
        }
        out.push(StreamResult {
            name: s.name.clone(),
            kind: s.kind.clone(),
            clock: s.clock.clone(),
            start_s: tl.start,
            end_s: tl.end,
            rate_hz: r(tl.rate_hz, 6),
            jitter_ms: r(tl.jitter_s * 1e3, 6),
            gaps: tl.gaps,
            max_gap_s: r(tl.max_gap_s, 6),
            non_monotonic: tl.non_monotonic,
            shared_events: pairs.len(),
            offset_s: a,
            drift_ppm: r((b - 1.0) * 1e6, 4),
            max_residual_ms: max_res.map(|x| r(x * 1e3, 6)),
            drift_bound_ms: bound.map(|x| r(x * 1e3, 6)),
            status,
            findings,
            replay_start_s: None,
            replay_start_sample: None,
        });
    }
    // Common window on the reference clock over the placeable streams.
    let placed: Vec<(usize, f64, f64)> = out
        .iter()
        .enumerate()
        .filter(|(_, s)| s.status != "unsynchronisable")
        .map(|(i, s)| {
            let b = 1.0 + s.drift_ppm * 1e-6;
            (i, s.offset_s + b * s.start_s, s.offset_s + b * s.end_s)
        })
        .collect();
    let w0 = placed.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let w1 = placed.iter().map(|p| p.2).fold(f64::INFINITY, f64::min);
    let window = (w1 > w0).then_some([w0, w1]);
    if let Some([w0, _]) = window {
        for (i, _, _) in &placed {
            let s = &mut out[*i];
            let b = 1.0 + s.drift_ppm * 1e-6;
            let t = (w0 - s.offset_s) / b;
            s.replay_start_s = Some(t);
            if s.kind == "iq" {
                s.replay_start_sample = Some(((t - s.start_s) * s.rate_hz).round().max(0.0) as u64);
            }
        }
    }
    let sensors_ok = out
        .iter()
        .filter(|s| s.kind != "iq")
        .all(|s| s.status == "aligned");
    let any_bad = out
        .iter()
        .any(|s| s.status == "misaligned" || s.status == "unsynchronisable");
    let verdict = if window.is_none() || any_bad {
        "not-synchronised"
    } else if out.iter().all(|s| s.status == "aligned") {
        "synchronised"
    } else {
        "limited"
    };
    SyncReport {
        method: METHOD,
        label: super::LABEL,
        evidence: super::EVIDENCE,
        id: set.id.clone(),
        reference: set.reference.clone(),
        tolerance_ms: set.tolerance_ms,
        verdict: verdict.into(),
        sensors_synchronised: sensors_ok && window.is_some(),
        window_s: window,
        streams: out,
        inputs: l.inputs.refs.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datatypes_and_iso_times() {
        assert_eq!(sigmf_bytes_per_sample("cf32_le"), Some(8));
        assert_eq!(sigmf_bytes_per_sample("ci16_le"), Some(4));
        assert_eq!(sigmf_bytes_per_sample("ru8"), Some(1));
        assert_eq!(sigmf_bytes_per_sample("xq7"), None);
        assert_eq!(iso_to_unix("1970-01-02T00:00:01.5Z").unwrap(), 86_401.5);
    }

    #[test]
    fn timeline_finds_gaps_and_jitter() {
        let mut t: Vec<f64> = (0..100).map(|i| i as f64 * 0.01).collect();
        t.retain(|x| !(0.5..0.6).contains(x));
        let tl = timeline_of(&t).unwrap();
        assert!((tl.rate_hz - 100.0).abs() < 1e-6);
        assert_eq!(tl.gaps, 1);
        assert!((tl.max_gap_s - 0.11).abs() < 1e-9);
        assert!(tl.jitter_s < 1e-9);
    }
}
