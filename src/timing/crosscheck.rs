// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Multi-source time cross-check with non-GNSS sources (E19 B1905)
//!
//! Research 05 (#107 and the ISP/WISP voices that dropped WWVB for GPS-only), 09b (#1492) and
//! 12 (#1692, #1693, #1711, #1763, #1765): a timing node should check GNSS against sources
//! that do not share its failure modes: a long-wave time signal (DCF77, MSF, WWVB, JJY, BPC),
//! a terrestrial broadcast positioning service, eLoran, a LEO service, a fibre or PTP feed from
//! a national time laboratory, and NTP/NTS servers of independent operators.
//!
//! The check differs from the node self-check ([`super::selfcheck`]) in one respect:
//! **independence**. Every source has a traceability *root* (the system its time ultimately
//! comes from). Sources that share a root vote as one group, so three receivers of one GNSS,
//! or a PTP grandmaster disciplined by that GNSS, cannot outvote one long-wave receiver. Per
//! bin ([`arbitrate`]):
//!
//! 1. inside a group of three or more, a member beyond `k·u_i + floor` of the group's median
//!    is a **member fault** and leaves the group;
//! 2. across groups, the pairwise consistency matrix of the group values gives the verdict:
//!    `consistent`; **`isolated`** (exactly one group whose removal leaves every remaining
//!    pair consistent, at least two groups remaining, and that group inconsistent with every
//!    one of them: a single-source fault isolated); `detected-not-isolated` (two groups that
//!    disagree); `ambiguous` (more than one group could be the faulty one); `multiple-faults`;
//!    `no-independent-check` (one group);
//! 3. the consensus (weighted mean of the groups left) is the node's offset; beyond
//!    `node_limit_s` it is a **`node-off`** alarm.
//!
//! Over the record, a **triad monitor** (three-cornered hat on the group series) estimates each
//! group's own noise from the variances of the pairwise differences, and a group noisier than
//! twice its stated uncertainty is flagged. A source that stops reporting for `stale_s` is a
//! **`silent`** event (the alarm time is the last reading plus `stale_s`).
//!
//! Long-wave time signals are corrected for the ground-wave delay from the transmitter to the
//! node (great-circle distance over `c`, transmitter sites approximate); `delay_s` overrides it.
//!
//! **Label: MODELLED.** Exact arithmetic on the logs as supplied, with the operator's stated
//! uncertainties, signs and roots; tested on synthetic logs only. The default uncertainties of
//! [`CLASSES`] are typical orders of magnitude for the operator to replace.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::selfcheck::{self, Reading, SourceSpec};
use super::sig9;

pub const FEATURE: &str = "pro-timing-crosscheck";
pub const FORMAT: &str = "kshana-pro-timing-crosscheck/1";
pub const LABEL: &str = "MODELLED: a vote across independent time sources on the logs as supplied, with the operator's stated uncertainties, signs and traceability roots; tested on synthetic logs. A fault is isolated only as far as the sources are truly independent; a fault common to every root is invisible.";
/// Speed of light, m/s.
const C: f64 = 299_792_458.0;

/// Source classes: (class, default root, typical standard uncertainty of one binned offset, s).
pub const CLASSES: [(&str, &str, f64); 12] = [
    ("gnss", "gnss", 100e-9),
    ("radio-time", "<transmitter>", 2e-3),
    ("bps", "bps", 1e-6),
    ("eloran", "eloran", 100e-9),
    ("leo", "leo", 1e-6),
    ("fibre", "<name>", 100e-9),
    ("ptp", "<name>", 1e-6),
    ("ntp", "<name>", 5e-3),
    ("nts", "<name>", 5e-3),
    ("holdover", "<name>", 1e-6),
    ("local", "<name>", 1e-6),
    ("other", "<name>", 1e-3),
];

/// Long-wave time-signal transmitters: (id, site, latitude, longitude). Approximate site
/// coordinates; one kilometre is 3.3 µs, small against the receivers' jitter.
pub const TRANSMITTERS: [(&str, &str, f64, f64); 6] = [
    ("dcf77", "Mainflingen (DE)", 50.0157, 9.0109),
    ("msf", "Anthorn (UK)", 54.9116, -3.2783),
    ("wwvb", "Fort Collins (US)", 40.6779, -105.0469),
    ("jjy40", "Ohtakadoya-yama (JP)", 37.3725, 140.8489),
    ("jjy60", "Hagane-yama (JP)", 33.4653, 130.1752),
    ("bpc", "Shangqiu (CN)", 34.4567, 115.8372),
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub lat_deg: f64,
    pub lon_deg: f64,
}

/// One source.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub name: String,
    /// One of [`CLASSES`].
    pub class: String,
    /// The traceability root; default by class (GNSS sources share `gnss`, a long-wave signal
    /// is its transmitter, other classes are their own root unless `depends_on` says more).
    #[serde(default)]
    pub root: Option<String>,
    /// A source this one is derived from (e.g. a PTP grandmaster disciplined by a GNSS
    /// receiver): it takes that source's root.
    #[serde(default)]
    pub depends_on: Option<String>,
    /// For `radio-time`: one of [`TRANSMITTERS`].
    #[serde(default)]
    pub transmitter: Option<String>,
    /// Delay subtracted from the offset (propagation and receiver), s; for `radio-time` the
    /// default is the ground-wave delay from the transmitter.
    #[serde(default)]
    pub delay_s: Option<f64>,
    /// As the self-check: `csv`, `ptp4l`, `phc2sys`, `ts2phc`, `chrony-tracking`,
    /// `chrony-statistics`, `pps-assert`, `ntp-peerstats`.
    pub format: String,
    pub path: String,
    /// Standard uncertainty of one binned offset, s (default by class).
    #[serde(default)]
    pub u_s: Option<f64>,
    #[serde(default)]
    pub sign: Option<f64>,
    #[serde(default)]
    pub time_offset_s: f64,
    #[serde(default)]
    pub select: Option<String>,
}

fn d_bin() -> f64 {
    60.0
}
fn d_k() -> f64 {
    3.0
}

/// A cross-check request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub node: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default = "d_bin")]
    pub bin_s: f64,
    #[serde(default = "d_k")]
    pub k: f64,
    #[serde(default)]
    pub floor_s: f64,
    /// The node's own offset (the consensus) beyond this is `node-off`, s.
    #[serde(default)]
    pub node_limit_s: Option<f64>,
    /// A source silent this long is a `silent` event, s (default 3 bins).
    #[serde(default)]
    pub stale_s: Option<f64>,
    #[serde(default)]
    pub location: Option<Location>,
    pub sources: Vec<Source>,
}

/// Great-circle distance on a sphere of radius 6371 km, m.
pub fn distance_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = p2 - p1;
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * a.sqrt().min(1.0).asin()
}

/// The ground-wave delay from a transmitter to a location, s.
pub fn ground_wave_delay_s(transmitter: &str, loc: &Location) -> Result<f64, String> {
    let (_, _, la, lo) = TRANSMITTERS
        .iter()
        .find(|t| t.0.eq_ignore_ascii_case(transmitter))
        .ok_or_else(|| {
            format!(
                "unknown transmitter '{transmitter}' (one of {})",
                TRANSMITTERS.map(|t| t.0).join(", ")
            )
        })?;
    Ok(distance_m(*la, *lo, loc.lat_deg, loc.lon_deg) / C)
}

/// A source resolved: its root, uncertainty and delay.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Resolved {
    pub name: String,
    pub class: String,
    pub root: String,
    /// The root was taken from the class default, not stated.
    pub root_stated: bool,
    pub u_s: f64,
    pub delay_s: f64,
    pub delay_basis: String,
}

/// Resolve every source's root, uncertainty and delay.
pub fn resolve(spec: &Spec) -> Result<Vec<Resolved>, String> {
    let mut names = BTreeSet::new();
    for s in &spec.sources {
        if s.name.trim().is_empty() || !names.insert(s.name.as_str()) {
            return Err(format!("source name '{}' is empty or repeated", s.name));
        }
        if !CLASSES.iter().any(|c| c.0 == s.class) {
            return Err(format!(
                "source {}: unknown class '{}' (one of {})",
                s.name,
                s.class,
                CLASSES.map(|c| c.0).join(", ")
            ));
        }
    }
    let own_root = |s: &Source| -> (String, bool) {
        if let Some(r) = &s.root {
            return (r.clone(), true);
        }
        match s.class.as_str() {
            "radio-time" => (
                s.transmitter
                    .clone()
                    .unwrap_or_else(|| format!("radio-time:{}", s.name)),
                true,
            ),
            "gnss" | "bps" | "eloran" | "leo" => (s.class.clone(), true),
            _ => (format!("{}:{}", s.class, s.name), false),
        }
    };
    let mut out = Vec::new();
    for s in &spec.sources {
        // Follow depends_on to the first source with no dependency.
        let mut cur = s;
        let mut seen = BTreeSet::new();
        while let Some(d) = &cur.depends_on {
            if !seen.insert(cur.name.as_str()) {
                return Err(format!("source {}: depends_on forms a cycle", s.name));
            }
            cur = spec
                .sources
                .iter()
                .find(|x| &x.name == d)
                .ok_or_else(|| format!("source {}: depends_on unknown '{d}'", cur.name))?;
        }
        let (root, stated) = if std::ptr::eq(cur, s) {
            own_root(s)
        } else {
            (own_root(cur).0, true)
        };
        let u = s.u_s.unwrap_or_else(|| {
            CLASSES
                .iter()
                .find(|c| c.0 == s.class)
                .map(|c| c.2)
                .unwrap_or(1e-3)
        });
        if !(u > 0.0 && u.is_finite()) {
            return Err(format!("source {}: u_s must be positive", s.name));
        }
        let (delay, basis) = match (s.delay_s, s.class.as_str(), &s.transmitter) {
            (Some(d), _, _) => (d, "stated".to_string()),
            (None, "radio-time", Some(t)) => {
                let loc = spec.location.as_ref().ok_or_else(|| {
                    format!(
                        "source {}: a radio-time source needs [location] or delay_s",
                        s.name
                    )
                })?;
                let d = ground_wave_delay_s(t, loc)?;
                let site = TRANSMITTERS
                    .iter()
                    .find(|x| x.0.eq_ignore_ascii_case(t))
                    .map(|x| x.1)
                    .unwrap_or("");
                (
                    d,
                    format!(
                        "ground wave from {site}: {:.0} km great circle / c (approximate site)",
                        d * C / 1000.0
                    ),
                )
            }
            (None, "radio-time", None) => {
                return Err(format!(
                    "source {}: a radio-time source needs transmitter or delay_s",
                    s.name
                ))
            }
            _ => (0.0, "none".to_string()),
        };
        out.push(Resolved {
            name: s.name.clone(),
            class: s.class.clone(),
            root,
            root_stated: stated,
            u_s: u,
            delay_s: sig9(delay),
            delay_basis: basis,
        });
    }
    Ok(out)
}

/// One present source in a bin: (index, offset after delay, u).
type Cell = (usize, f64, f64);

/// The verdict of one bin.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BinVerdict {
    pub t_s: f64,
    /// `consistent`, `isolated`, `detected-not-isolated`, `ambiguous`, `multiple-faults`,
    /// `no-independent-check`, `no-data`.
    pub status: String,
    /// The isolated root (status `isolated`).
    pub isolated: Option<String>,
    /// Sources that disagreed with the rest of their own root group.
    pub member_faults: Vec<String>,
    pub groups: usize,
    pub consensus_s: Option<f64>,
    pub u_consensus_s: Option<f64>,
    pub node_off: bool,
}

fn wmean(v: &[(f64, f64)]) -> (f64, f64) {
    let w: f64 = v.iter().map(|x| 1.0 / (x.1 * x.1)).sum();
    let m = v.iter().map(|x| x.0 / (x.1 * x.1)).sum::<f64>() / w;
    (m, (1.0 / w).sqrt())
}

/// Whether two values agree within `k` combined standard uncertainties plus a floor. Every
/// source's value must already be against one reference on one time scale with the operator's
/// sign convention; the vote does not convert scales.
fn agree(a: (f64, f64), b: (f64, f64), k: f64, floor: f64) -> bool {
    (a.0 - b.0).abs() <= k * (a.1 * a.1 + b.1 * b.1).sqrt() + floor
}

/// The arbitration of one bin: the group values `(root, value, u)`, the member faults, the
/// status and the isolated root.
pub type Arbitration = (Vec<(String, f64, f64)>, Vec<String>, String, Option<String>);

/// The arbitration of one bin: `cells` the present sources, `roots[i]` the root of source `i`.
pub fn arbitrate(
    cells: &[Cell],
    roots: &[String],
    names: &[String],
    k: f64,
    floor: f64,
) -> Arbitration {
    // 1. Group by root; drop members that disagree with the rest of their group.
    let mut groups: BTreeMap<&str, Vec<(usize, f64, f64)>> = BTreeMap::new();
    for c in cells {
        groups.entry(roots[c.0].as_str()).or_default().push(*c);
    }
    let mut member_faults = Vec::new();
    let mut gv: Vec<(String, f64, f64)> = Vec::new();
    for (root, mem) in groups {
        let mut keep = mem.clone();
        if mem.len() >= 3 {
            let mut vals: Vec<f64> = mem.iter().map(|x| x.1).collect();
            let med = median(&mut vals).unwrap_or(0.0);
            keep = mem
                .iter()
                .filter(|m| {
                    let ok = agree((m.1, m.2), (med, 0.0), k, floor);
                    if !ok {
                        member_faults.push(names[m.0].clone());
                    }
                    ok
                })
                .copied()
                .collect();
        }
        if keep.is_empty() {
            continue;
        }
        let (m, u) = wmean(&keep.iter().map(|x| (x.1, x.2)).collect::<Vec<_>>());
        gv.push((root.to_string(), m, u));
    }
    let n = gv.len();
    let ok = |i: usize, j: usize| agree((gv[i].1, gv[i].2), (gv[j].1, gv[j].2), k, floor);
    let all_ok = |skip: Option<usize>| {
        (0..n).all(|i| Some(i) == skip || (i + 1..n).all(|j| Some(j) == skip || ok(i, j)))
    };
    let (status, isolated) = match n {
        0 => ("no-data".to_string(), None),
        1 => ("no-independent-check".to_string(), None),
        _ if all_ok(None) => ("consistent".to_string(), None),
        2 => ("detected-not-isolated".to_string(), None),
        _ => {
            let cands: Vec<usize> = (0..n)
                .filter(|&g| all_ok(Some(g)) && (0..n).filter(|&j| j != g).all(|j| !ok(g, j)))
                .collect();
            match cands.len() {
                1 => ("isolated".to_string(), Some(gv[cands[0]].0.clone())),
                0 if (0..n).any(|g| all_ok(Some(g))) => ("ambiguous".to_string(), None),
                0 => ("multiple-faults".to_string(), None),
                _ => ("ambiguous".to_string(), None),
            }
        }
    };
    (gv, member_faults, status, isolated)
}

/// One event (a run of bins with the same finding).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Event {
    /// `isolated`, `member-fault`, `node-off`, `silent`, `detected-not-isolated`,
    /// `ambiguous`, `multiple-faults`.
    pub kind: String,
    pub subject: String,
    pub start_s: f64,
    pub end_s: f64,
    pub bins: usize,
    /// For a fault: the median deviation of the subject from the consensus, s.
    pub magnitude_s: Option<f64>,
    pub detail: String,
}

/// The triad monitor's estimate for one root group.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TriadEstimate {
    pub root: String,
    pub stated_u_s: f64,
    /// Median over the trios that contain the group; `None` when no trio resolved it.
    pub estimated_sigma_s: Option<f64>,
    pub trios: usize,
    /// `as-stated`, `noisier-than-stated`, `not-resolved`.
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub label: &'static str,
    pub node: String,
    pub profile: Option<String>,
    pub sources: Vec<Resolved>,
    pub roots: Vec<String>,
    /// Sources whose independence rests on a class default (no root or dependency stated).
    pub unstated_independence: Vec<String>,
    pub bins: Vec<BinVerdict>,
    pub counts: BTreeMap<String, usize>,
    pub events: Vec<Event>,
    pub triad: Vec<TriadEstimate>,
    pub verdict: String,
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

fn human(x: f64) -> String {
    super::fleet::human_time(x)
}

/// Run the cross-check. `base` resolves the source paths.
pub fn check(spec: &Spec, base: &Path) -> Result<Report, String> {
    if spec.bin_s.is_nan() || spec.bin_s <= 0.0 || spec.k.is_nan() || spec.k <= 0.0 {
        return Err("bin_s and k must be positive".into());
    }
    let res = resolve(spec)?;
    let mut series: Vec<Vec<Reading>> = Vec::new();
    for s in &spec.sources {
        let p = base.join(&s.path);
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let ss = SourceSpec {
            name: s.name.clone(),
            kind: Some(s.class.clone()),
            format: s.format.clone(),
            path: s.path.clone(),
            u_s: 1.0,
            disciplining: false,
            sign: s.sign,
            time_offset_s: s.time_offset_s,
            select: s.select.clone(),
        };
        series.push(selfcheck::read_source(&ss, &text)?);
    }
    let roots: Vec<String> = res.iter().map(|r| r.root.clone()).collect();
    let names: Vec<String> = res.iter().map(|r| r.name.clone()).collect();
    let t0 = series
        .iter()
        .filter_map(|s| s.first().map(|r| r.t))
        .fold(f64::INFINITY, f64::min);
    let t_end = series
        .iter()
        .filter_map(|s| s.last().map(|r| r.t))
        .fold(f64::NEG_INFINITY, f64::max);
    let mut acc: BTreeMap<i64, Vec<Vec<f64>>> = BTreeMap::new();
    for (i, s) in series.iter().enumerate() {
        for r in s {
            let b = ((r.t - t0) / spec.bin_s).floor() as i64;
            acc.entry(b)
                .or_insert_with(|| vec![Vec::new(); series.len()])[i]
                .push(r.offset - res[i].delay_s);
        }
    }
    let mut bins = Vec::new();
    let mut group_series: BTreeMap<String, BTreeMap<i64, f64>> = BTreeMap::new();
    // Per bin and per source: deviation from the consensus (for magnitudes).
    let mut dev: Vec<BTreeMap<i64, f64>> = vec![BTreeMap::new(); series.len()];
    let mut gdev: BTreeMap<(i64, String), f64> = BTreeMap::new();
    for (b, per) in &acc {
        let cells: Vec<Cell> = per
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let mut v = v.clone();
                median(&mut v).map(|m| (i, m, res[i].u_s))
            })
            .collect();
        let (gv, mf, status, iso) = arbitrate(&cells, &roots, &names, spec.k, spec.floor_s);
        let kept: Vec<(f64, f64)> = gv
            .iter()
            .filter(|g| {
                Some(&g.0) != iso.as_ref()
                    && !matches!(
                        status.as_str(),
                        "detected-not-isolated" | "ambiguous" | "multiple-faults"
                    )
            })
            .map(|g| (g.1, g.2))
            .collect();
        let cons = (!kept.is_empty()).then(|| wmean(&kept));
        for g in &gv {
            group_series.entry(g.0.clone()).or_default().insert(*b, g.1);
            if let Some((c, _)) = cons {
                gdev.insert((*b, g.0.clone()), g.1 - c);
            }
        }
        if let Some((c, _)) = cons {
            for cell in &cells {
                dev[cell.0].insert(*b, cell.1 - c);
            }
        }
        let node_off = matches!((cons, spec.node_limit_s), (Some((c, _)), Some(l)) if c.abs() > l);
        bins.push(BinVerdict {
            t_s: sig9(t0 + *b as f64 * spec.bin_s),
            status,
            isolated: iso,
            member_faults: mf,
            groups: gv.len(),
            consensus_s: cons.map(|c| sig9(c.0)),
            u_consensus_s: cons.map(|c| sig9(c.1)),
            node_off,
        });
    }
    // Events: runs of consecutive bins with the same subject.
    let mut events: Vec<Event> = Vec::new();
    let bin_index = |t: f64| ((t - t0) / spec.bin_s).round() as i64;
    let mut push_runs = |kind: &str,
                         flagged: Vec<(f64, String)>,
                         mag: &dyn Fn(i64, &str) -> Option<f64>,
                         detail: &dyn Fn(&str) -> String| {
        let mut by: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for (t, s) in flagged {
            by.entry(s).or_default().push(t);
        }
        for (subj, ts) in by {
            let mut start = 0;
            for i in 1..=ts.len() {
                if i == ts.len() || bin_index(ts[i]) != bin_index(ts[i - 1]) + 1 {
                    let run = &ts[start..i];
                    let mut m: Vec<f64> = run
                        .iter()
                        .filter_map(|t| mag(bin_index(*t), &subj))
                        .collect();
                    events.push(Event {
                        kind: kind.into(),
                        subject: subj.clone(),
                        start_s: run[0],
                        end_s: sig9(run[run.len() - 1] + spec.bin_s),
                        bins: run.len(),
                        magnitude_s: median(&mut m).map(sig9),
                        detail: detail(&subj),
                    });
                    start = i;
                }
            }
        }
    };
    let iso: Vec<(f64, String)> = bins
        .iter()
        .filter_map(|b| b.isolated.clone().map(|r| (b.t_s, r)))
        .collect();
    let members_of = |root: &str| -> Vec<String> {
        res.iter()
            .filter(|r| r.root == root)
            .map(|r| r.name.clone())
            .collect()
    };
    push_runs(
        "isolated",
        iso,
        &|b, s| gdev.get(&(b, s.to_string())).copied(),
        &|s| {
            format!(
                "root '{s}' ({}) disagrees with every other independent root, which agree with each other: a single-source fault isolated",
                members_of(s).join(", ")
            )
        },
    );
    let mfl: Vec<(f64, String)> = bins
        .iter()
        .flat_map(|b| b.member_faults.iter().map(move |m| (b.t_s, m.clone())))
        .collect();
    push_runs(
        "member-fault",
        mfl,
        &|b, s| {
            names
                .iter()
                .position(|n| n == s)
                .and_then(|i| dev[i].get(&b).copied())
        },
        &|s| format!("source '{s}' disagrees with the other sources of its own root"),
    );
    let off: Vec<(f64, String)> = bins
        .iter()
        .filter(|b| b.node_off)
        .map(|b| (b.t_s, spec.node.clone()))
        .collect();
    let cons_by_bin: BTreeMap<i64, f64> = bins
        .iter()
        .filter_map(|b| b.consensus_s.map(|c| (bin_index(b.t_s), c)))
        .collect();
    push_runs(
        "node-off",
        off,
        &|b, _| cons_by_bin.get(&b).copied(),
        &|_| {
            format!(
                "the independent sources agree that the node is off by more than {}",
                human(spec.node_limit_s.unwrap_or(0.0))
            )
        },
    );
    for kind in ["detected-not-isolated", "ambiguous", "multiple-faults"] {
        let fl: Vec<(f64, String)> = bins
            .iter()
            .filter(|b| b.status == kind)
            .map(|b| (b.t_s, "sources".to_string()))
            .collect();
        push_runs(kind, fl, &|_, _| None, &|_| match kind {
            "detected-not-isolated" => {
                "two independent roots disagree; a third independent source is needed to say which"
                    .into()
            }
            "ambiguous" => "the roots disagree and more than one could be the faulty one".into(),
            _ => "no single root explains the disagreement".into(),
        });
    }
    // Silent sources.
    let stale = spec.stale_s.unwrap_or(3.0 * spec.bin_s);
    for (i, s) in series.iter().enumerate() {
        let mut last = s[0].t;
        for r in s.iter().skip(1).chain(std::iter::once(&Reading {
            t: t_end + stale + 1.0,
            offset: 0.0,
            locked: None,
            path_delay: None,
        })) {
            if r.t - last > stale {
                let end = r.t.min(t_end);
                if end - last > stale {
                    events.push(Event {
                        kind: "silent".into(),
                        subject: names[i].clone(),
                        start_s: sig9(last),
                        end_s: sig9(end),
                        bins: (((end - last) / spec.bin_s).floor() as usize).max(1),
                        magnitude_s: None,
                        detail: format!(
                            "no reading for more than {} (alarm at {}); a time chain that stops silently still serves time",
                            human(stale),
                            sig9(last + stale)
                        ),
                    });
                }
            }
            last = r.t;
        }
    }
    events.sort_by(|a, b| a.start_s.total_cmp(&b.start_s).then(a.kind.cmp(&b.kind)));
    // Triad monitor.
    let groups: Vec<String> = group_series.keys().cloned().collect();
    let stated: BTreeMap<String, f64> = groups
        .iter()
        .map(|g| {
            let us: Vec<(f64, f64)> = res
                .iter()
                .filter(|r| &r.root == g)
                .map(|r| (0.0, r.u_s))
                .collect();
            (g.clone(), wmean(&us).1)
        })
        .collect();
    // Bins where a root was isolated are left out of its triads (the fault is not noise).
    let iso_bins: BTreeSet<(i64, &str)> = bins
        .iter()
        .filter_map(|x| x.isolated.as_deref().map(|r| (bin_index(x.t_s), r)))
        .collect();
    let var_diff = |a: &str, b: &str| -> Option<(f64, usize)> {
        let (sa, sb) = (&group_series[a], &group_series[b]);
        let d: Vec<f64> = sa
            .iter()
            .filter(|(k, _)| !iso_bins.contains(&(**k, a)) && !iso_bins.contains(&(**k, b)))
            .filter_map(|(k, v)| sb.get(k).map(|w| v - w))
            .collect();
        if d.len() < 10 {
            return None;
        }
        let m = d.iter().sum::<f64>() / d.len() as f64;
        Some((
            d.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (d.len() - 1) as f64,
            d.len(),
        ))
    };
    let mut est: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut trios: BTreeMap<String, usize> = BTreeMap::new();
    for i in 0..groups.len() {
        for j in i + 1..groups.len() {
            for l in j + 1..groups.len() {
                let (a, b, c) = (&groups[i], &groups[j], &groups[l]);
                let (Some(ab), Some(ac), Some(bc)) =
                    (var_diff(a, b), var_diff(a, c), var_diff(b, c))
                else {
                    continue;
                };
                for (g, v) in [
                    (a, (ab.0 + ac.0 - bc.0) / 2.0),
                    (b, (ab.0 + bc.0 - ac.0) / 2.0),
                    (c, (ac.0 + bc.0 - ab.0) / 2.0),
                ] {
                    *trios.entry(g.clone()).or_default() += 1;
                    if v > 0.0 {
                        est.entry(g.clone()).or_default().push(v.sqrt());
                    }
                }
            }
        }
    }
    let triad: Vec<TriadEstimate> = groups
        .iter()
        .filter(|g| trios.contains_key(*g))
        .map(|g| {
            let e = est.get_mut(g).and_then(|v| median(v));
            let s = stated[g];
            TriadEstimate {
                root: g.clone(),
                stated_u_s: sig9(s),
                estimated_sigma_s: e.map(sig9),
                trios: trios[g],
                status: match e {
                    None => "not-resolved",
                    Some(x) if x > 2.0 * s => "noisier-than-stated",
                    _ => "as-stated",
                }
                .into(),
            }
        })
        .collect();
    let mut counts = BTreeMap::new();
    for b in &bins {
        *counts.entry(b.status.clone()).or_insert(0) += 1;
    }
    let distinct: BTreeSet<&String> = roots.iter().collect();
    let unstated: Vec<String> = res
        .iter()
        .filter(|r| !r.root_stated)
        .map(|r| r.name.clone())
        .collect();
    let n_iso = events.iter().filter(|e| e.kind == "isolated").count();
    let n_off = events.iter().filter(|e| e.kind == "node-off").count();
    let n_sil = events.iter().filter(|e| e.kind == "silent").count();
    let verdict = if distinct.len() < 2 {
        format!(
            "{}: only one independent root ({}): the node's time cannot be cross-checked",
            spec.node,
            distinct.iter().next().map(|s| s.as_str()).unwrap_or("none")
        )
    } else if n_iso + n_off + n_sil == 0
        && counts.get("consistent").copied().unwrap_or(0)
            == bins.iter().filter(|b| b.groups >= 2).count()
    {
        format!(
            "{}: {} independent roots consistent in every bin with two or more",
            spec.node,
            distinct.len()
        )
    } else {
        format!(
            "{}: {} isolated fault(s), {} node-off, {} silent source(s), {} bin(s) not consistent across {} roots",
            spec.node,
            n_iso,
            n_off,
            n_sil,
            bins.iter()
                .filter(|b| b.groups >= 2 && b.status != "consistent")
                .count(),
            distinct.len()
        )
    };
    Ok(Report {
        format: FORMAT,
        label: LABEL,
        node: spec.node.clone(),
        profile: spec.profile.clone(),
        sources: res,
        roots: distinct.into_iter().cloned().collect(),
        unstated_independence: unstated,
        bins,
        counts,
        events,
        triad,
        verdict,
    })
}

/// The bins as CSV.
pub fn bins_csv(r: &Report) -> String {
    let mut o = String::from(
        "t_s,status,isolated,member_faults,groups,consensus_s,u_consensus_s,node_off\n",
    );
    for b in &r.bins {
        o.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            b.t_s,
            b.status,
            b.isolated.as_deref().unwrap_or(""),
            b.member_faults.join(";"),
            b.groups,
            b.consensus_s.map(|x| format!("{x:e}")).unwrap_or_default(),
            b.u_consensus_s
                .map(|x| format!("{x:e}"))
                .unwrap_or_default(),
            b.node_off
        ));
    }
    o
}

/// Request templates: (name, description, TOML).
pub const PROFILES: [(&str, &str, &str); 2] = [
    (
        "wisp",
        "a low-cost multi-source appliance for a wireless ISP point of presence: GNSS PPS, a long-wave time receiver, NTS servers of two independent operators",
        r#"node = "pop-1"
profile = "wisp"
bin_s = 60
k = 3
node_limit_s = 0.010      # the operator's own limit for the services it carries
stale_s = 180
[location]                # the node's position (for the long-wave delay)
lat_deg = 52.0
lon_deg = 5.0

[[sources]]
name = "gnss-pps"
class = "gnss"
format = "pps-assert"
path = "pps.assert"
u_s = 1e-6

[[sources]]
name = "longwave"
class = "radio-time"
transmitter = "dcf77"     # dcf77 msf wwvb jjy40 jjy60 bpc: the one received at the site
format = "csv"
path = "longwave.csv"     # unix_time,offset_s from the receiver's time marks
u_s = 2e-3

[[sources]]
name = "nts-a"
class = "nts"
root = "operator-a"       # a server traceable to a time laboratory, not to GNSS
format = "chrony-statistics"
path = "statistics.log"
select = "nts-a.example"
u_s = 5e-3

[[sources]]
name = "nts-b"
class = "nts"
root = "operator-b"
format = "chrony-statistics"
path = "statistics.log"
select = "nts-b.example"
u_s = 5e-3
"#,
    ),
    (
        "timing-node",
        "a telecom or utility timing node: GNSS, a PTP feed from a national time laboratory over fibre, eLoran where available",
        r#"node = "tn-1"
profile = "timing-node"
bin_s = 10
k = 3
node_limit_s = 1.1e-6

[[sources]]
name = "gnss"
class = "gnss"
format = "ts2phc"
path = "ts2phc.log"
u_s = 30e-9

[[sources]]
name = "ptp-gm"
class = "ptp"
depends_on = "gnss"       # the local grandmaster is disciplined by the same GNSS: not independent
format = "ptp4l"
path = "ptp4l.log"
u_s = 50e-9

[[sources]]
name = "fibre-utck"
class = "fibre"
root = "utc-k"
format = "csv"
path = "fibre.csv"
u_s = 50e-9

[[sources]]
name = "eloran"
class = "eloran"
format = "csv"
path = "eloran.csv"
u_s = 100e-9
"#,
    ),
];
