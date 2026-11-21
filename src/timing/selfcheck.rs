// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Clock self-check and cross-site correlation (W1120 TA2)
//!
//! Research 05 (gaps G8, G10): a Turkish operator's cell ran 6,250 µs off with "no reference
//! to validate the timing"; after spoofing a receiver can look healthy and still be wrong;
//! gpsd and chrony fail silently (research 08b finding 6). Two checks:
//!
//! * **Node** ([`check_node`]): several time sources at one node (GNSS PPS, PTP with its path
//!   delay, NTP peers, any CSV), each giving the node's offset from that source with a stated
//!   standard uncertainty, binned on a common grid. Per bin, a weighted vote with outlier
//!   rejection (`|o_i − c| > k·sqrt(u_i² + u_c²) + floor`) finds the consensus offset `c` of
//!   the node and the sources that disagree. The headline status is **`gnss-outlier`**: the
//!   source the node is disciplined to reads healthy (its offset within `k·u` of zero, its
//!   servo locked) while at least two independent sources agree that the node is off: the
//!   node's time is wrong although its GNSS still looks healthy. One source alone is reported
//!   as `single-source` (the time cannot be validated), two as `pair-agree` / `pair-disagree`
//!   (a disagreement is seen but not attributed). A PTP path-delay step beyond a threshold is
//!   an event (route change, asymmetry, delay attack).
//! * **Sites** ([`check_sites`]): one series per site (the site's time against a common
//!   reference or monitor), median and MAD across sites per bin; a site beyond
//!   `k·sqrt(u_i² + σ²) + floor` is flagged, and a two-sided CUSUM catches slow divergence
//!   below that threshold. Each flagged bin gets a scope: `single-site`, `operator-wide` (all
//!   flagged sites of one operator), `regional` (several operators in one region) or
//!   `common-mode` (the median itself moved against its baseline: a system-wide event such as
//!   the SVN-23 13 µs UTC error, visible only against a non-GNSS reference).
//!
//! **Label: MODELLED.** The vote is exact arithmetic on the logs as given; the stated source
//! uncertainties and signs are the operator's. Tested on synthetic logs only.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const FEATURE: &str = "pro-timing-selfcheck";
/// Cross-site and cross-operator correlation.
pub const CORRELATION_FEATURE: &str = "pro-timing-correlation";
pub const NODE_FORMAT: &str = "kshana-pro-timing-selfcheck/1";
pub const SITES_FORMAT: &str = "kshana-pro-timing-correlation/1";
pub const LABEL: &str = "MODELLED: a weighted vote on the logs as supplied, with the operator's stated source uncertainties and offset signs; tested on synthetic logs. A disagreement is evidence to investigate, not a diagnosis.";

/// Unix seconds of 1980-01-06, where the engine's chrony and ntpd readers count from.
const GPS_EPOCH_UNIX: f64 = 315_964_800.0;

/// One time source of a node, or one site's series.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpec {
    pub name: String,
    /// `gnss`, `ptp`, `ntp`, `pps` or `other` (informative).
    #[serde(default)]
    pub kind: Option<String>,
    /// `csv` (`unix_time,offset_s[,locked]`), `ptp4l`, `phc2sys`, `ts2phc`,
    /// `chrony-tracking`, `chrony-statistics`, `pps-assert` or `ntp-peerstats`.
    pub format: String,
    pub path: String,
    /// Standard uncertainty of one binned offset of this source, s.
    pub u_s: f64,
    /// The node is disciplined to this source.
    #[serde(default)]
    pub disciplining: bool,
    /// Multiplier that makes the offset "node minus source" (default by format: +1, and −1
    /// for ntpd peerstats, whose offset is the server's minus the local clock's).
    #[serde(default)]
    pub sign: Option<f64>,
    /// Added to the log's time stamps to make them Unix seconds (for logs stamped with a
    /// monotonic clock).
    #[serde(default)]
    pub time_offset_s: f64,
    /// Keep only lines of this source/peer (chrony statistics, ntpd peerstats, linuxptp).
    #[serde(default)]
    pub select: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoldoverLogSpec {
    /// CSV `unix_time,state` (`locked`, `holdover`, `freerun`, ...).
    pub path: String,
}

fn d_bin() -> f64 {
    60.0
}
fn d_k() -> f64 {
    3.0
}
fn d_delay() -> f64 {
    100e-9
}

/// A node self-check request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeSpec {
    pub node: String,
    #[serde(default = "d_bin")]
    pub bin_s: f64,
    #[serde(default = "d_k")]
    pub k: f64,
    #[serde(default)]
    pub floor_s: f64,
    /// A PTP path-delay change between bins beyond this is an event, s.
    #[serde(default = "d_delay")]
    pub path_delay_step_s: f64,
    pub sources: Vec<SourceSpec>,
    #[serde(default)]
    pub holdover: Option<HoldoverLogSpec>,
}

/// One reading.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub t: f64,
    /// Node minus source, s.
    pub offset: f64,
    pub locked: Option<bool>,
    pub path_delay: Option<f64>,
}

fn ptp_path_delay(line: &str) -> Option<f64> {
    let i = line.find("path delay")?;
    line[i + 10..]
        .split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
        .map(|ns| ns * 1e-9)
}

/// Rows of one input that were dropped, with the first few reasons (shown in the report so a
/// reader sees "N rows skipped" rather than a silent loss).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SkippedRows {
    pub source: String,
    pub rows: usize,
    pub first: Vec<String>,
}

/// The verdict suffix for skipped rows ("" when none).
pub fn skipped_note(v: &[SkippedRows]) -> String {
    if v.is_empty() {
        return String::new();
    }
    let total: usize = v.iter().map(|s| s.rows).sum();
    format!(
        "; {total} row(s) skipped ({}), see skipped_rows",
        v.iter()
            .map(|s| format!("{} {}", s.source, s.rows))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

impl SkippedRows {
    /// The first reasons kept.
    pub const KEEP: usize = 5;
    pub fn new(source: &str) -> Self {
        SkippedRows {
            source: source.into(),
            ..Default::default()
        }
    }
    pub fn add(&mut self, line: usize, reason: &str) {
        self.rows += 1;
        if self.first.len() < Self::KEEP {
            self.first.push(format!("line {line}: {reason}"));
        }
    }
    /// `Some` only when something was skipped.
    pub fn nonzero(self) -> Option<Self> {
        (self.rows > 0).then_some(self)
    }
}

/// Read one source's log into readings (node minus source, Unix time).
pub fn read_source(s: &SourceSpec, text: &str) -> Result<Vec<Reading>, String> {
    read_source_counted(s, text).map(|(r, _)| r)
}

/// [`read_source`], with the rows of a `csv` or `ntp-peerstats` log that could not be read
/// counted (a `csv` log's first unreadable line is its header, not a skip). Daemon logs (ptp4l,
/// chrony and the like) mix measurement lines with other messages, so their other lines are not
/// counted.
pub fn read_source_counted(
    s: &SourceSpec,
    text: &str,
) -> Result<(Vec<Reading>, SkippedRows), String> {
    let mut skipped = SkippedRows::new(&s.name);
    use kshana::clock_log::{read_line, ClockLogFormat};
    let sign = s.sign.unwrap_or(if s.format == "ntp-peerstats" {
        -1.0
    } else {
        1.0
    });
    let mut out = Vec::new();
    match s.format.as_str() {
        "csv" => {
            let mut first = true;
            for (n, l) in text.lines().enumerate() {
                let l = l.trim();
                if l.is_empty() || l.starts_with('#') {
                    continue;
                }
                let header = std::mem::replace(&mut first, false);
                let c: Vec<&str> = l.split([',', ';']).map(str::trim).collect();
                let (Some(t), Some(o)) = (
                    c.first().and_then(|x| x.parse::<f64>().ok()),
                    c.get(1).and_then(|x| x.parse::<f64>().ok()),
                ) else {
                    if !header {
                        skipped.add(n + 1, "time or offset not a number");
                    }
                    continue;
                };
                let locked = c.get(2).map(|x| {
                    matches!(
                        x.to_ascii_lowercase().as_str(),
                        "1" | "true" | "locked" | "s2"
                    )
                });
                out.push(Reading {
                    t: t + s.time_offset_s,
                    offset: sign * o,
                    locked,
                    path_delay: None,
                });
            }
        }
        "ntp-peerstats" => {
            for (n, l) in text.lines().enumerate() {
                let f: Vec<&str> = l.split_whitespace().collect();
                if f.is_empty() || f[0].starts_with('#') {
                    continue;
                }
                if f.len() < 8 {
                    skipped.add(n + 1, "fewer than 8 fields");
                    continue;
                }
                if s.select.as_deref().is_some_and(|p| p != f[2]) {
                    continue;
                }
                let (Ok(mjd), Ok(sec), Ok(off), Ok(delay)) = (
                    f[0].parse::<i64>(),
                    f[1].parse::<f64>(),
                    f[4].parse::<f64>(),
                    f[5].parse::<f64>(),
                ) else {
                    skipped.add(n + 1, "a field is not a number");
                    continue;
                };
                let t = (mjd - 40_587) as f64 * 86_400.0 + sec;
                out.push(Reading {
                    t: t + s.time_offset_s,
                    offset: sign * off,
                    locked: None,
                    path_delay: Some(delay / 2.0),
                });
            }
        }
        f => {
            let fmt = ClockLogFormat::from_name(f)?;
            let chrony = matches!(
                fmt,
                ClockLogFormat::ChronyTracking | ClockLogFormat::ChronyStatistics
            );
            let linuxptp = matches!(
                fmt,
                ClockLogFormat::Ptp4l | ClockLogFormat::Phc2sys | ClockLogFormat::Ts2phc
            );
            if !(chrony || linuxptp || fmt == ClockLogFormat::PpsAssert) {
                return Err(format!(
                    "source {}: format '{f}' is not read line by line (use csv, ptp4l, phc2sys, ts2phc, chrony-tracking, chrony-statistics, pps-assert or ntp-peerstats)",
                    s.name
                ));
            }
            for l in text.lines() {
                let Some(r) = read_line(fmt, l) else { continue };
                if s.select.as_deref().is_some_and(|p| p != r.source) {
                    continue;
                }
                let t = if chrony {
                    r.t_s + GPS_EPOCH_UNIX
                } else {
                    r.t_s
                };
                out.push(Reading {
                    t: t + s.time_offset_s,
                    offset: sign * r.offset_s,
                    locked: r.locked,
                    path_delay: if linuxptp { ptp_path_delay(l) } else { None },
                });
            }
        }
    }
    // A NaN or infinite time or offset (a logger's "nan" while unlocked, say) would compare false
    // against every limit and pass as "within"; refuse it so it can never reach a verdict.
    if let Some((i, r)) = out
        .iter()
        .enumerate()
        .find(|(_, r)| !(r.t.is_finite() && r.offset.is_finite()))
    {
        return Err(format!(
            "source {}: reading {} has a time or offset that is not a finite number (t {}, offset {}); remove such rows or mark the source unlocked",
            s.name,
            i + 1,
            r.t,
            r.offset
        ));
    }
    if out.is_empty() {
        return Err(format!("source {}: no reading in the log", s.name));
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    Ok((out, skipped))
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// One source in one bin.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    o: f64,
    locked: Option<bool>,
    delay: Option<f64>,
}

fn bin_all(series: &[Vec<Reading>], t0: f64, bin: f64) -> BTreeMap<i64, Vec<Option<Cell>>> {
    let mut acc: BTreeMap<i64, Vec<Vec<Reading>>> = BTreeMap::new();
    for (i, s) in series.iter().enumerate() {
        for r in s {
            let b = ((r.t - t0) / bin).floor() as i64;
            acc.entry(b)
                .or_insert_with(|| vec![Vec::new(); series.len()])[i]
                .push(*r);
        }
    }
    acc.into_iter()
        .map(|(b, per)| {
            let cells = per
                .into_iter()
                .map(|rs| {
                    if rs.is_empty() {
                        return None;
                    }
                    let mut o: Vec<f64> = rs.iter().map(|r| r.offset).collect();
                    let mut d: Vec<f64> = rs.iter().filter_map(|r| r.path_delay).collect();
                    let locked = if rs.iter().any(|r| r.locked == Some(false)) {
                        Some(false)
                    } else if rs.iter().any(|r| r.locked == Some(true)) {
                        Some(true)
                    } else {
                        None
                    };
                    Some(Cell {
                        o: median(&mut o),
                        locked,
                        delay: (!d.is_empty()).then(|| median(&mut d)),
                    })
                })
                .collect();
            (b, cells)
        })
        .collect()
}

/// The vote of one bin.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vote {
    /// `agree`, `node-off`, `gnss-outlier`, `disciplining-outlier`, `source-outlier`,
    /// `no-majority`, `pair-agree`, `pair-disagree`, `single-source`.
    pub status: String,
    /// The consensus node-minus-reference offset and its standard uncertainty, s.
    pub consensus_s: Option<f64>,
    pub u_consensus_s: Option<f64>,
    pub outliers: Vec<String>,
}

/// The weighted vote: `(name, offset, u, disciplining, locked)` per present source.
pub fn vote(cells: &[(String, f64, f64, bool, Option<bool>)], k: f64, floor: f64) -> Vote {
    let n = cells.len();
    let mk = |status: &str, c: Option<f64>, u: Option<f64>, out: Vec<String>| Vote {
        status: status.into(),
        consensus_s: c,
        u_consensus_s: u,
        outliers: out,
    };
    match n {
        0 => return mk("single-source", None, None, vec![]),
        1 => return mk("single-source", Some(cells[0].1), Some(cells[0].2), vec![]),
        2 => {
            let (a, b) = (&cells[0], &cells[1]);
            let lim = k * (a.2 * a.2 + b.2 * b.2).sqrt() + floor;
            let (wa, wb) = (1.0 / (a.2 * a.2), 1.0 / (b.2 * b.2));
            let c = (wa * a.1 + wb * b.1) / (wa + wb);
            let u = (1.0 / (wa + wb)).sqrt();
            return if (a.1 - b.1).abs() > lim {
                mk("pair-disagree", None, None, vec![])
            } else {
                mk("pair-agree", Some(c), Some(u), vec![])
            };
        }
        _ => {}
    }
    // One source, one vote: the plain median and the MAD start the vote, so a precise source
    // cannot outvote the others on its weight alone.
    let mut xs: Vec<f64> = cells.iter().map(|c| c.1).collect();
    let m = median(&mut xs);
    let mut dev: Vec<f64> = cells.iter().map(|c| (c.1 - m).abs()).collect();
    let mad = 1.4826 * median(&mut dev);
    let mut inl: Vec<bool> = cells
        .iter()
        .map(|c| (c.1 - m).abs() <= k * (c.2 * c.2 + mad * mad).sqrt() + floor)
        .collect();
    let mut c = m;
    let mut uc = 0.0;
    for _ in 0..3 {
        let w: f64 = cells
            .iter()
            .zip(&inl)
            .filter(|(_, i)| **i)
            .map(|(c, _)| 1.0 / (c.2 * c.2))
            .sum();
        if w <= 0.0 {
            break;
        }
        c = cells
            .iter()
            .zip(&inl)
            .filter(|(_, i)| **i)
            .map(|(x, _)| x.1 / (x.2 * x.2))
            .sum::<f64>()
            / w;
        uc = (1.0 / w).sqrt();
        let next: Vec<bool> = cells
            .iter()
            .map(|x| (x.1 - c).abs() <= k * (x.2 * x.2 + uc * uc).sqrt() + floor)
            .collect();
        if next == inl {
            break;
        }
        inl = next;
    }
    let n_in = inl.iter().filter(|x| **x).count();
    if n_in < 2 || n_in * 2 <= n {
        return mk("no-majority", None, None, vec![]);
    }
    let outliers: Vec<String> = cells
        .iter()
        .zip(&inl)
        .filter(|(_, i)| !**i)
        .map(|(x, _)| x.0.clone())
        .collect();
    let disc = cells.iter().position(|x| x.3);
    let status = match disc {
        Some(d) if !inl[d] => {
            let x = &cells[d];
            if x.1.abs() <= k * x.2 + floor && x.4 != Some(false) {
                "gnss-outlier"
            } else {
                "disciplining-outlier"
            }
        }
        _ if !outliers.is_empty() => "source-outlier",
        _ if c.abs() > k * uc + floor => "node-off",
        _ => "agree",
    };
    mk(status, Some(c), Some(uc), outliers)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Bin {
    pub t_unix: f64,
    pub sources: usize,
    pub vote: Vote,
    pub holdover_state: Option<String>,
}

/// A run of bins with one status (or a path-delay step).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Event {
    pub status: String,
    pub start_unix: f64,
    pub end_unix: f64,
    pub bins: usize,
    pub max_abs_consensus_s: Option<f64>,
    pub sources: Vec<String>,
    pub holdover_state: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SourceSummary {
    pub name: String,
    pub kind: Option<String>,
    pub format: String,
    pub readings: usize,
    pub u_s: f64,
    pub disciplining: bool,
    pub outlier_bins: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NodeReport {
    pub format: &'static str,
    pub label: &'static str,
    pub node: String,
    pub bin_s: f64,
    pub k: f64,
    pub floor_s: f64,
    pub sources: Vec<SourceSummary>,
    pub counts: BTreeMap<String, usize>,
    pub events: Vec<Event>,
    pub verdict: String,
    /// Rows of the source logs that could not be read.
    pub skipped_rows: Vec<SkippedRows>,
    #[serde(skip)]
    pub bins: Vec<Bin>,
}

fn read_text(base: &Path, p: &str) -> Result<String, String> {
    let path = if Path::new(p).is_absolute() {
        p.into()
    } else {
        base.join(p)
    };
    std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_states(text: &str) -> Vec<(f64, String)> {
    let mut v: Vec<(f64, String)> = text
        .lines()
        .filter_map(|l| {
            let (a, b) = l.trim().split_once([',', ';'])?;
            Some((a.trim().parse::<f64>().ok()?, b.trim().to_ascii_lowercase()))
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

fn state_at(states: &[(f64, String)], t: f64) -> Option<String> {
    states
        .iter()
        .rev()
        .find(|(ts, _)| *ts <= t)
        .map(|x| x.1.clone())
}

fn human(x: f64) -> String {
    super::fleet::human_time(x)
}

/// Run the node self-check (log paths relative to `base`).
pub fn check_node(spec: &NodeSpec, base: &Path) -> Result<NodeReport, String> {
    if spec.sources.is_empty() {
        return Err("a node needs at least one source".into());
    }
    if spec.bin_s.is_nan() || spec.bin_s <= 0.0 || spec.k.is_nan() || spec.k <= 0.0 {
        return Err("bin_s and k must be > 0".into());
    }
    if spec.sources.iter().filter(|s| s.disciplining).count() > 1 {
        return Err("at most one source can be the disciplining one".into());
    }
    let mut series = Vec::new();
    let mut skipped = Vec::new();
    for s in &spec.sources {
        if !(s.u_s.is_finite() && s.u_s > 0.0) {
            return Err(format!("source {}: u_s must be > 0", s.name));
        }
        let (r, sk) = read_source_counted(s, &read_text(base, &s.path)?)?;
        series.push(r);
        skipped.extend(sk.nonzero());
    }
    let states = match &spec.holdover {
        Some(h) => read_states(&read_text(base, &h.path)?),
        None => Vec::new(),
    };
    let t0 = series
        .iter()
        .filter_map(|s| s.first())
        .map(|r| r.t)
        .fold(f64::INFINITY, f64::min);
    let binned = bin_all(&series, t0, spec.bin_s);
    let mut bins = Vec::new();
    let mut outlier_bins = vec![0usize; spec.sources.len()];
    let mut events: Vec<Event> = Vec::new();
    let mut prev_delay: Vec<Option<f64>> = vec![None; spec.sources.len()];
    for (b, cells) in &binned {
        let t = t0 + *b as f64 * spec.bin_s;
        let present: Vec<(String, f64, f64, bool, Option<bool>)> = cells
            .iter()
            .zip(&spec.sources)
            .filter_map(|(c, s)| c.map(|c| (s.name.clone(), c.o, s.u_s, s.disciplining, c.locked)))
            .collect();
        for (i, c) in cells.iter().enumerate() {
            if let Some(d) = c.and_then(|c| c.delay) {
                if let Some(p) = prev_delay[i] {
                    if (d - p).abs() > spec.path_delay_step_s {
                        events.push(Event {
                            status: "path-delay-step".into(),
                            start_unix: t,
                            end_unix: t + spec.bin_s,
                            bins: 1,
                            max_abs_consensus_s: None,
                            sources: vec![spec.sources[i].name.clone()],
                            holdover_state: state_at(&states, t + spec.bin_s),
                            detail: format!(
                                "path delay of {} moved from {} to {} (route change, asymmetry or delay attack)",
                                spec.sources[i].name,
                                human(p),
                                human(d)
                            ),
                        });
                    }
                }
                prev_delay[i] = Some(d);
            }
        }
        let v = vote(&present, spec.k, spec.floor_s);
        for (i, s) in spec.sources.iter().enumerate() {
            if v.outliers.contains(&s.name) {
                outlier_bins[i] += 1;
            }
        }
        bins.push(Bin {
            t_unix: t,
            sources: present.len(),
            vote: v,
            holdover_state: state_at(&states, t + spec.bin_s),
        });
    }
    // Merge runs.
    let mut runs: Vec<Event> = Vec::new();
    for b in &bins {
        if b.vote.status == "agree" || b.vote.status == "pair-agree" {
            continue;
        }
        let ext = runs.last_mut().filter(|e| {
            e.status == b.vote.status
                && (b.t_unix - e.end_unix).abs() < 1e-6
                && e.holdover_state == b.holdover_state
        });
        match ext {
            Some(e) => {
                e.end_unix = b.t_unix + spec.bin_s;
                e.bins += 1;
                if let Some(c) = b.vote.consensus_s {
                    e.max_abs_consensus_s = Some(e.max_abs_consensus_s.unwrap_or(0.0).max(c.abs()));
                }
                for o in &b.vote.outliers {
                    if !e.sources.contains(o) {
                        e.sources.push(o.clone());
                    }
                }
            }
            None => runs.push(Event {
                status: b.vote.status.clone(),
                start_unix: b.t_unix,
                end_unix: b.t_unix + spec.bin_s,
                bins: 1,
                max_abs_consensus_s: b.vote.consensus_s.map(f64::abs),
                sources: b.vote.outliers.clone(),
                holdover_state: b.holdover_state.clone(),
                detail: String::new(),
            }),
        }
    }
    for e in &mut runs {
        e.detail = match e.status.as_str() {
            "gnss-outlier" => format!(
                "the node's time is off by up to {} against the independent sources while the disciplining source reads healthy",
                human(e.max_abs_consensus_s.unwrap_or(0.0))
            ),
            "disciplining-outlier" => "the disciplining source itself shows the disagreement (its offset or lock state is not healthy)".into(),
            "source-outlier" => format!("{} disagree(s) with the other sources", e.sources.join(", ")),
            "node-off" => format!(
                "every source agrees the node is off, by up to {}",
                human(e.max_abs_consensus_s.unwrap_or(0.0))
            ),
            "no-majority" => "the sources split with no majority".into(),
            "pair-disagree" => "two sources disagree; with two the faulty one cannot be named".into(),
            "single-source" => "one source only: the node's time cannot be validated".into(),
            _ => String::new(),
        };
    }
    events.extend(runs);
    events.sort_by(|a, b| a.start_unix.total_cmp(&b.start_unix));
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for b in &bins {
        *counts.entry(b.vote.status.clone()).or_default() += 1;
    }
    let n = bins.len().max(1);
    let gnss: Vec<&Event> = events
        .iter()
        .filter(|e| e.status == "gnss-outlier")
        .collect();
    let verdict = if !gnss.is_empty() {
        let worst = gnss
            .iter()
            .filter_map(|e| e.max_abs_consensus_s)
            .fold(0.0, f64::max);
        let mins: f64 = gnss.iter().map(|e| e.end_unix - e.start_unix).sum::<f64>() / 60.0;
        format!(
            "the node's time disagreed with its independent sources by up to {} for {:.0} min in {} event(s) while its GNSS looked healthy",
            human(worst),
            mins,
            gnss.len()
        )
    } else if counts.get("single-source").copied().unwrap_or(0) == n {
        "one time source only: the node's time cannot be validated (no independent reference)"
            .into()
    } else if counts.contains_key("node-off") || counts.contains_key("disciplining-outlier") {
        "the node's time was off with the disagreement visible at its disciplining source".into()
    } else if events.is_empty() {
        format!("the sources agreed in every one of {} bins", bins.len())
    } else {
        format!(
            "{} event(s); no time where GNSS looked healthy while the node was off",
            events.len()
        )
    };
    Ok(NodeReport {
        format: NODE_FORMAT,
        label: LABEL,
        node: spec.node.clone(),
        bin_s: spec.bin_s,
        k: spec.k,
        floor_s: spec.floor_s,
        sources: spec
            .sources
            .iter()
            .zip(&series)
            .zip(&outlier_bins)
            .map(|((s, r), o)| SourceSummary {
                name: s.name.clone(),
                kind: s.kind.clone(),
                format: s.format.clone(),
                readings: r.len(),
                u_s: s.u_s,
                disciplining: s.disciplining,
                outlier_bins: *o,
            })
            .collect(),
        counts,
        events,
        verdict: format!("{verdict}{}", skipped_note(&skipped)),
        skipped_rows: skipped,
        bins,
    })
}

/// The bins as CSV.
pub fn node_bins_csv(r: &NodeReport) -> String {
    let mut o =
        String::from("t_unix,sources,status,consensus_s,u_consensus_s,outliers,holdover_state\n");
    for b in &r.bins {
        o.push_str(&format!(
            "{},{},{},{},{},{},{}\n",
            b.t_unix,
            b.sources,
            b.vote.status,
            b.vote
                .consensus_s
                .map(|x| format!("{x:e}"))
                .unwrap_or_default(),
            b.vote
                .u_consensus_s
                .map(|x| format!("{x:e}"))
                .unwrap_or_default(),
            b.vote.outliers.join(" "),
            b.holdover_state.as_deref().unwrap_or("")
        ));
    }
    o
}

// ---------------------------------------------------------------------------------------
// Cross-site and cross-operator correlation
// ---------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteSpec {
    pub id: String,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    pub format: String,
    pub path: String,
    pub u_s: f64,
    #[serde(default)]
    pub sign: Option<f64>,
    #[serde(default)]
    pub time_offset_s: f64,
    #[serde(default)]
    pub select: Option<String>,
}

fn d_cusum_h() -> f64 {
    5.0
}
fn d_min_sites() -> usize {
    3
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SitesSpec {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "d_bin")]
    pub bin_s: f64,
    #[serde(default = "d_k")]
    pub k: f64,
    #[serde(default)]
    pub floor_s: f64,
    /// CUSUM decision interval (in units of the combined sigma; slack 0.5).
    #[serde(default = "d_cusum_h")]
    pub cusum_h: f64,
    #[serde(default = "d_min_sites")]
    pub min_sites: usize,
    /// Bins at the start taken as the common-mode baseline (default: the first 10 %, at least 5).
    #[serde(default)]
    pub baseline_bins: Option<usize>,
    pub sites: Vec<SiteSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SiteEvent {
    /// `diverged` (beyond the threshold), `slow-divergence` (CUSUM) or `common-mode`.
    pub kind: String,
    pub sites: Vec<String>,
    pub operators: Vec<String>,
    /// `single-site`, `operator-wide`, `regional`, `multi-operator` or `common-mode`.
    pub scope: String,
    pub start_unix: f64,
    pub end_unix: f64,
    pub max_abs_divergence_s: f64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SiteSummary {
    pub id: String,
    pub operator: Option<String>,
    pub region: Option<String>,
    pub readings: usize,
    pub flagged_bins: usize,
    pub first_flag_unix: Option<f64>,
    pub cusum_alarm_unix: Option<f64>,
    pub max_abs_divergence_s: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SitesReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: Option<String>,
    pub bins: usize,
    pub bins_with_quorum: usize,
    pub sites: Vec<SiteSummary>,
    pub events: Vec<SiteEvent>,
    pub verdict: String,
    /// Rows of the site logs that could not be read.
    pub skipped_rows: Vec<SkippedRows>,
}

fn scope_of(flagged: &[usize], spec: &SitesSpec) -> String {
    let ops: std::collections::BTreeSet<String> = flagged
        .iter()
        .map(|&i| {
            spec.sites[i]
                .operator
                .clone()
                .unwrap_or_else(|| "(unstated)".into())
        })
        .collect();
    let regions: std::collections::BTreeSet<Option<String>> = flagged
        .iter()
        .map(|&i| spec.sites[i].region.clone())
        .collect();
    if flagged.len() == 1 {
        "single-site".into()
    } else if ops.len() == 1 {
        "operator-wide".into()
    } else if regions.len() == 1 && regions.iter().next().is_some_and(|r| r.is_some()) {
        "regional".into()
    } else {
        "multi-operator".into()
    }
}

/// Run the cross-site correlation (log paths relative to `base`).
pub fn check_sites(spec: &SitesSpec, base: &Path) -> Result<SitesReport, String> {
    if spec.sites.len() < spec.min_sites.max(3) {
        return Err(format!(
            "the correlation needs at least {} sites",
            spec.min_sites.max(3)
        ));
    }
    let mut series = Vec::new();
    let mut skipped = Vec::new();
    for s in &spec.sites {
        if !(s.u_s.is_finite() && s.u_s > 0.0) {
            return Err(format!("site {}: u_s must be > 0", s.id));
        }
        let src = SourceSpec {
            name: s.id.clone(),
            format: s.format.clone(),
            path: s.path.clone(),
            u_s: s.u_s,
            sign: s.sign,
            time_offset_s: s.time_offset_s,
            select: s.select.clone(),
            ..Default::default()
        };
        let (r, sk) = read_source_counted(&src, &read_text(base, &s.path)?)?;
        series.push(r);
        skipped.extend(sk.nonzero());
    }
    let t0 = series
        .iter()
        .filter_map(|s| s.first())
        .map(|r| r.t)
        .fold(f64::INFINITY, f64::min);
    let binned = bin_all(&series, t0, spec.bin_s);
    let n = spec.sites.len();
    let mut flagged_bins = vec![0usize; n];
    let mut first_flag = vec![None; n];
    let mut alarm = vec![None; n];
    let mut maxdiv = vec![0.0f64; n];
    let (mut sp, mut sm) = (vec![0.0f64; n], vec![0.0f64; n]);
    let mut medians: Vec<(f64, f64)> = Vec::new();
    let mut flag_runs: Vec<(f64, Vec<usize>, f64)> = Vec::new();
    let mut quorum = 0;
    for (b, cells) in &binned {
        let t = t0 + *b as f64 * spec.bin_s;
        let present: Vec<(usize, f64)> = cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.map(|c| (i, c.o)))
            .collect();
        if present.len() < spec.min_sites.max(3) {
            continue;
        }
        quorum += 1;
        let mut xs: Vec<f64> = present.iter().map(|x| x.1).collect();
        let m = median(&mut xs);
        let mut dev: Vec<f64> = present.iter().map(|x| (x.1 - m).abs()).collect();
        let sigma = 1.4826 * median(&mut dev);
        medians.push((t, m));
        let mut fl = Vec::new();
        let mut worst = 0.0f64;
        for (i, x) in &present {
            let u = spec.sites[*i].u_s;
            let s_c = (u * u + sigma * sigma).sqrt();
            let d = x - m;
            maxdiv[*i] = maxdiv[*i].max(d.abs());
            if d.abs() > spec.k * s_c + spec.floor_s {
                fl.push(*i);
                flagged_bins[*i] += 1;
                first_flag[*i].get_or_insert(t);
                worst = worst.max(d.abs());
            }
            let z = d / s_c.max(f64::MIN_POSITIVE);
            sp[*i] = (sp[*i] + z - 0.5).max(0.0);
            sm[*i] = (sm[*i] - z - 0.5).max(0.0);
            if alarm[*i].is_none() && (sp[*i] > spec.cusum_h || sm[*i] > spec.cusum_h) {
                alarm[*i] = Some(t);
            }
        }
        if !fl.is_empty() {
            flag_runs.push((t, fl, worst));
        }
    }
    let mut events: Vec<SiteEvent> = Vec::new();
    for (t, fl, worst) in flag_runs {
        let scope = scope_of(&fl, spec);
        let sites: Vec<String> = fl.iter().map(|&i| spec.sites[i].id.clone()).collect();
        let ext = events
            .last_mut()
            .filter(|e| e.kind == "diverged" && e.sites == sites && (t - e.end_unix).abs() < 1e-6);
        match ext {
            Some(e) => {
                e.end_unix = t + spec.bin_s;
                e.max_abs_divergence_s = e.max_abs_divergence_s.max(worst);
            }
            None => {
                let mut ops: Vec<String> = fl
                    .iter()
                    .filter_map(|&i| spec.sites[i].operator.clone())
                    .collect();
                ops.sort();
                ops.dedup();
                events.push(SiteEvent {
                    kind: "diverged".into(),
                    sites,
                    operators: ops,
                    scope,
                    start_unix: t,
                    end_unix: t + spec.bin_s,
                    max_abs_divergence_s: worst,
                    detail: String::new(),
                });
            }
        }
    }
    for e in &mut events {
        e.detail = format!(
            "{} beyond k·sqrt(u² + σ²) + floor of the cross-site median, by up to {} ({})",
            e.sites.join(", "),
            human(e.max_abs_divergence_s),
            e.scope
        );
    }
    for i in 0..n {
        if let Some(a) = alarm[i] {
            if first_flag[i].is_none_or(|f| a < f) {
                events.push(SiteEvent {
                    kind: "slow-divergence".into(),
                    sites: vec![spec.sites[i].id.clone()],
                    operators: spec.sites[i].operator.clone().into_iter().collect(),
                    scope: "single-site".into(),
                    start_unix: a,
                    end_unix: a + spec.bin_s,
                    max_abs_divergence_s: maxdiv[i],
                    detail: format!(
                        "CUSUM alarm for {} before (or without) a threshold crossing: a slow drift away from the other sites",
                        spec.sites[i].id
                    ),
                });
            }
        }
    }
    // Common mode: the median against its baseline.
    let nb = spec
        .baseline_bins
        .unwrap_or((medians.len() / 10).max(5))
        .min(medians.len());
    if nb >= 3 && medians.len() > nb {
        let mut base: Vec<f64> = medians[..nb].iter().map(|x| x.1).collect();
        let mb = median(&mut base);
        let mut us: Vec<f64> = spec.sites.iter().map(|s| s.u_s).collect();
        let u = median(&mut us) / (n as f64).sqrt();
        let lim = spec.k * u * 1.2533 + spec.floor_s;
        let mut cur: Option<SiteEvent> = None;
        for (t, m) in &medians[nb..] {
            let d = (m - mb).abs();
            if d > lim {
                match cur.as_mut() {
                    Some(e) => {
                        e.end_unix = t + spec.bin_s;
                        e.max_abs_divergence_s = e.max_abs_divergence_s.max(d);
                    }
                    None => {
                        cur = Some(SiteEvent {
                            kind: "common-mode".into(),
                            sites: spec.sites.iter().map(|s| s.id.clone()).collect(),
                            operators: Vec::new(),
                            scope: "common-mode".into(),
                            start_unix: *t,
                            end_unix: t + spec.bin_s,
                            max_abs_divergence_s: d,
                            detail: String::new(),
                        })
                    }
                }
            } else if let Some(mut e) = cur.take() {
                e.detail = format!("the cross-site median moved by up to {} from its baseline: every site moved together (a system-wide event; seen only against a reference independent of the common source)", human(e.max_abs_divergence_s));
                events.push(e);
            }
        }
        if let Some(mut e) = cur.take() {
            e.detail = format!("the cross-site median moved by up to {} from its baseline: every site moved together (a system-wide event; seen only against a reference independent of the common source)", human(e.max_abs_divergence_s));
            events.push(e);
        }
    }
    events.sort_by(|a, b| a.start_unix.total_cmp(&b.start_unix));
    let mut by_scope: BTreeMap<&str, usize> = BTreeMap::new();
    for e in &events {
        *by_scope.entry(e.scope.as_str()).or_default() += 1;
    }
    let verdict = if events.is_empty() {
        format!("no site diverged from the others in {quorum} bins with a quorum")
    } else {
        format!(
            "{} event(s): {}",
            events.len(),
            by_scope
                .iter()
                .map(|(k, v)| format!("{k} x{v}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Ok(SitesReport {
        format: SITES_FORMAT,
        label: LABEL,
        title: spec.title.clone(),
        bins: binned.len(),
        bins_with_quorum: quorum,
        sites: spec
            .sites
            .iter()
            .enumerate()
            .map(|(i, s)| SiteSummary {
                id: s.id.clone(),
                operator: s.operator.clone(),
                region: s.region.clone(),
                readings: series[i].len(),
                flagged_bins: flagged_bins[i],
                first_flag_unix: first_flag[i],
                cusum_alarm_unix: alarm[i],
                max_abs_divergence_s: maxdiv[i],
            })
            .collect(),
        events,
        verdict: format!("{verdict}{}", skipped_note(&skipped)),
        skipped_rows: skipped,
    })
}
