// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # ADS-B phantom-target plausibility evidence (research 2026-11-20 gaps, B1002)
//!
//! Reads the ADS-B CSV of the open engine's interference map and `adsb-indicators`
//! (`timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp,source_type`, with the optional columns
//! `alt_geom_ft` and `receiver_id`), the receiving stations the user **declares** (position,
//! antenna height, range), and optionally an independent surveillance track file the user
//! supplies (radar or multilateration output, `timestamp,lat,lon`). For every target address it
//! gives a plausibility score in [0, 1] with the reasons:
//!
//! * **physics**: implied speed above the declared limit sustained over the track, or a vertical
//!   rate above the declared limit;
//! * **teleport**: a position jump no aircraft could make in the time between reports;
//! * **two places at once**: the address alternates between positions that cannot both be true;
//! * **no consistent history**: too few reports for a track, or an address that is not 24-bit hex;
//! * **unhearable**: reports from beyond what the declared receiver (or any declared receiver)
//!   could hear, by its declared range and the radio horizon of its antenna and the target;
//! * **altitude**: geometric and barometric altitude further apart than the declared limit
//!   (`altitude-gross` beyond twice the limit);
//! * **no entry**: a target first heard deep inside coverage at cruise altitude, and a
//!   **cluster** of such appearances close in time and place;
//! * **no independent track**: the user's reference source has nothing near most of its reports.
//!
//! Reasons combine as a noisy-or of their weights. The output is per-target plausibility
//! evidence for review. **No function estimates where a transmitter is**: there is no time
//! difference of arrival, no multilateration of an emitter and no direction finding; receiver
//! positions are used only to ask whether a declared station could have heard a reported
//! position. MODELLED.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::forensics::inputs::parse_utc;
use crate::transport::{dist_m, offset_deg, Table};

/// Pro feature.
pub const FEATURE: &str = "pro-adsb-phantom";
/// Report format.
pub const FORMAT: &str = "kshana-pro-adsb-phantom/1";
/// The method label.
pub const METHOD: &str = "kshana-pro-adsb-phantom/v1";

const KT: f64 = 0.514_444;
const FT: f64 = 0.3048;
const NM: f64 = 1852.0;

/// A declared receiving station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receiver {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    /// Antenna height above the ground (m).
    pub antenna_m: f64,
    /// The station's declared range (nm).
    pub range_nm: f64,
}

/// The receivers file: `[[receiver]]` tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receivers {
    pub receiver: Vec<Receiver>,
}

/// Thresholds; every one is declared in the report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub max_speed_kt: f64,
    pub max_vertical_fpm: f64,
    /// A jump counts when it exceeds the distance at the limit speed plus this (m).
    pub jump_slack_m: f64,
    /// Jumps at or above this count, and this share of the reports, read as two places at once.
    pub two_places_min_jumps: usize,
    pub min_reports: usize,
    /// Tolerance on a receiver's reach (fraction).
    pub reach_tolerance: f64,
    pub unhearable_min_reports: usize,
    pub max_geom_baro_ft: f64,
    /// First report above this altitude, inside this fraction of every reach, and later than
    /// `start_grace_s` after the data start, is an appearance with no entry.
    pub entry_min_alt_ft: f64,
    pub entry_interior_frac: f64,
    pub start_grace_s: f64,
    pub cluster_window_s: f64,
    pub cluster_radius_m: f64,
    pub cluster_min_targets: usize,
    pub reference_window_s: f64,
    pub reference_radius_m: f64,
    pub reference_min_share: f64,
    pub flag_score: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            max_speed_kt: 750.0,
            max_vertical_fpm: 20_000.0,
            jump_slack_m: 2_000.0,
            two_places_min_jumps: 4,
            min_reports: 3,
            reach_tolerance: 0.1,
            unhearable_min_reports: 2,
            max_geom_baro_ft: 3_000.0,
            entry_min_alt_ft: 10_000.0,
            entry_interior_frac: 0.7,
            start_grace_s: 120.0,
            cluster_window_s: 120.0,
            cluster_radius_m: 30_000.0,
            cluster_min_targets: 3,
            reference_window_s: 10.0,
            reference_radius_m: 3_000.0,
            reference_min_share: 0.3,
            flag_score: 0.5,
        }
    }
}

/// Weights of the reasons (noisy-or).
pub const WEIGHTS: &[(&str, f64)] = &[
    ("physics", 0.7),
    ("teleport", 0.55),
    ("two-places", 0.8),
    ("no-history", 0.15),
    ("bad-address", 0.3),
    ("unhearable", 0.8),
    ("altitude", 0.3),
    ("altitude-gross", 0.6),
    ("no-entry", 0.15),
    ("cluster", 0.5),
    ("no-independent-track", 0.6),
];

fn weight(code: &str) -> f64 {
    WEIGHTS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|x| x.1)
        .unwrap_or(0.0)
}

#[derive(Clone, Debug)]
struct Rep {
    t: f64,
    lat: f64,
    lon: f64,
    baro_ft: Option<f64>,
    geom_ft: Option<f64>,
    rx: Option<String>,
}

/// One reason.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Reason {
    pub code: &'static str,
    pub weight: f64,
    pub detail: String,
}

/// One target.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Target {
    /// The address as given, or its pseudonym.
    pub address: String,
    pub reports: usize,
    pub first_utc: String,
    pub last_utc: String,
    pub score: f64,
    pub flagged: bool,
    pub reasons: Vec<Reason>,
}

/// Input statistics.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Stats {
    pub rows: usize,
    pub used: usize,
    pub excluded_non_adsb: usize,
    pub excluded_unparsable: usize,
    pub reference_points: usize,
}

/// The report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    pub format: &'static str,
    pub method: &'static str,
    pub label: &'static str,
    pub params: Params,
    pub receivers: Vec<Receiver>,
    pub stats: Stats,
    pub targets: Vec<Target>,
    pub flagged: usize,
    pub reason_counts: BTreeMap<String, usize>,
    pub pseudonymised: bool,
    pub not_done: &'static str,
    pub verdict: String,
}

fn ts(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().or_else(|| parse_utc(s))
}

/// Reach of a receiver to a target at `alt_m` (m): the declared range or the radio horizon
/// (4/3 Earth), whichever is shorter.
pub fn reach_m(r: &Receiver, alt_m: f64) -> f64 {
    let horizon = 4_124.0 * (r.antenna_m.max(0.0).sqrt() + alt_m.max(0.0).sqrt());
    (r.range_nm * NM).min(horizon)
}

fn alt_m(p: &Rep) -> f64 {
    p.geom_ft.or(p.baro_ft).unwrap_or(0.0) * FT
}

fn read_reports(csv: &str, st: &mut Stats) -> Result<BTreeMap<String, Vec<Rep>>, String> {
    let t = Table::parse(csv)?;
    let (ct, ci, cla, clo, cb) = (
        t.need("timestamp")?,
        t.need("aircraft_id")?,
        t.need("lat")?,
        t.need("lon")?,
        t.need("alt_baro_ft")?,
    );
    let (cg, crx, csrc) = (
        t.col("alt_geom_ft"),
        t.col("receiver_id"),
        t.col("source_type"),
    );
    let mut by: BTreeMap<String, Vec<Rep>> = BTreeMap::new();
    for i in 0..t.rows.len() {
        st.rows += 1;
        if csrc.is_some_and(|c| {
            let s = t.text(i, Some(c)).trim();
            !s.is_empty() && !s.to_ascii_lowercase().starts_with("adsb")
        }) {
            st.excluded_non_adsb += 1;
            continue;
        }
        let id = t.text(i, Some(ci)).trim().to_ascii_lowercase();
        let num = |c: usize| t.text(i, Some(c)).trim().parse::<f64>().ok();
        let (Some(tt), Some(lat), Some(lon)) = (ts(t.text(i, Some(ct))), num(cla), num(clo)) else {
            st.excluded_unparsable += 1;
            continue;
        };
        if id.is_empty() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            st.excluded_unparsable += 1;
            continue;
        }
        st.used += 1;
        by.entry(id).or_default().push(Rep {
            t: tt,
            lat,
            lon,
            baro_ft: num(cb),
            geom_ft: cg.and_then(num),
            rx: crx
                .map(|c| t.text(i, Some(c)).trim().to_string())
                .filter(|s| !s.is_empty()),
        });
    }
    for v in by.values_mut() {
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
    }
    Ok(by)
}

fn read_reference(csv: &str) -> Result<Vec<(f64, f64, f64)>, String> {
    let t = Table::parse(csv)?;
    let (ct, cla, clo) = (t.need("timestamp")?, t.need("lat")?, t.need("lon")?);
    let mut v = Vec::new();
    for i in 0..t.rows.len() {
        let num = |c: usize| t.text(i, Some(c)).trim().parse::<f64>().ok();
        if let (Some(tt), Some(a), Some(b)) = (ts(t.text(i, Some(ct))), num(cla), num(clo)) {
            v.push((tt, a, b));
        }
    }
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(v)
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn push(out: &mut Vec<Reason>, code: &'static str, detail: String) {
    out.push(Reason {
        code,
        weight: weight(code),
        detail,
    });
}

/// Run the check. `salt`: pseudonymise addresses with it (the open map never writes them).
pub fn check(
    csv: &str,
    receivers: &[Receiver],
    reference: Option<&str>,
    p: &Params,
    salt: Option<&str>,
) -> Result<Report, String> {
    if receivers.is_empty() {
        return Err("at least one declared receiver is needed".into());
    }
    for r in receivers {
        if !(r.range_nm > 0.0 && r.antenna_m >= 0.0) {
            return Err(format!(
                "receiver {}: range_nm > 0 and antenna_m >= 0 are needed",
                r.id
            ));
        }
    }
    let mut st = Stats::default();
    let by = read_reports(csv, &mut st)?;
    if by.is_empty() {
        return Err("no usable ADS-B report".into());
    }
    let refs = match reference {
        Some(r) => Some(read_reference(r)?),
        None => None,
    };
    st.reference_points = refs.as_ref().map_or(0, Vec::len);
    let t0 = by
        .values()
        .flat_map(|v| v.first())
        .map(|r| r.t)
        .fold(f64::INFINITY, f64::min);
    let vmax = p.max_speed_kt * KT;
    let rx_of = |id: &str| receivers.iter().find(|r| r.id == id);
    // Could any (or the named) receiver hear this report?
    let hearable = |r: &Rep| -> bool {
        let a = alt_m(r);
        let ok = |x: &Receiver| {
            dist_m(x.lat, x.lon, r.lat, r.lon) <= reach_m(x, a) * (1.0 + p.reach_tolerance)
        };
        match r.rx.as_deref().and_then(rx_of) {
            Some(x) => ok(x),
            None => receivers.iter().any(ok),
        }
    };

    let mut targets = Vec::new();
    let mut appear: Vec<(usize, f64, f64, f64)> = Vec::new();
    for (id, v) in &by {
        let mut rs = Vec::new();
        // Address and history.
        if id.len() != 6 || !id.chars().all(|c| c.is_ascii_hexdigit()) {
            push(
                &mut rs,
                "bad-address",
                format!(
                    "'{}' is not a 24-bit hex address",
                    if salt.is_some() {
                        "(pseudonymised)"
                    } else {
                        id
                    }
                ),
            );
        }
        if v.len() < p.min_reports {
            push(
                &mut rs,
                "no-history",
                format!("{} report(s), fewer than {}", v.len(), p.min_reports),
            );
        }
        // Jumps and rates.
        let lim = |dt: f64| vmax * dt + p.jump_slack_m;
        let jump: Vec<bool> = v
            .windows(2)
            .map(|w| {
                dist_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon) > lim((w[1].t - w[0].t).max(0.0))
            })
            .collect();
        let mut speeds = Vec::new();
        let (mut biggest, mut vert) = (0.0f64, 0.0f64);
        for (k, w) in v.windows(2).enumerate() {
            let dt = (w[1].t - w[0].t).max(0.0);
            let d = dist_m(w[0].lat, w[0].lon, w[1].lat, w[1].lon);
            if jump[k] {
                biggest = biggest.max(d);
            }
            if dt >= 5.0 {
                speeds.push(d / dt);
                if let (Some(a), Some(b)) = (w[0].baro_ft, w[1].baro_ft) {
                    if (b - a).abs() > 1_000.0 {
                        vert = vert.max((b - a).abs() / dt * 60.0);
                    }
                }
            }
        }
        let jumps = jump.iter().filter(|j| **j).count();
        // Alternation: out and back, with the reports either side consistent with each other.
        let alternations = (1..jump.len())
            .filter(|&k| {
                jump[k - 1]
                    && jump[k]
                    && dist_m(v[k - 1].lat, v[k - 1].lon, v[k + 1].lat, v[k + 1].lon)
                        <= lim(v[k + 1].t - v[k - 1].t)
            })
            .count();
        let fast = median(speeds).filter(|m| *m > vmax);
        if alternations >= p.two_places_min_jumps && jumps as f64 >= 0.2 * v.len() as f64 {
            push(&mut rs, "two-places", format!("{alternations} out-and-back jumps in {} reports (largest {:.1} km): the address alternates between positions that cannot both be true", v.len(), biggest / 1000.0));
        } else if let Some(m) = fast {
            push(
                &mut rs,
                "physics",
                format!(
                    "median implied speed {:.0} kt above the limit {:.0} kt",
                    m / KT,
                    p.max_speed_kt
                ),
            );
        } else if jumps > 0 {
            push(
                &mut rs,
                "teleport",
                format!(
                    "{jumps} jump(s) beyond {:.0} kt plus {:.1} km (largest {:.1} km)",
                    p.max_speed_kt,
                    p.jump_slack_m / 1000.0,
                    biggest / 1000.0
                ),
            );
        }
        if vert > p.max_vertical_fpm {
            push(
                &mut rs,
                "physics",
                format!(
                    "vertical rate {vert:.0} ft/min above the limit {:.0} ft/min",
                    p.max_vertical_fpm
                ),
            );
        }
        // Coverage.
        let unheard = v.iter().filter(|r| !hearable(r)).count();
        if unheard >= p.unhearable_min_reports || (unheard > 0 && unheard * 2 >= v.len()) {
            push(
                &mut rs,
                "unhearable",
                format!(
                    "{unheard} of {} report(s) beyond the reach of {} declared receiver",
                    v.len(),
                    if v.iter().any(|r| r.rx.is_some()) {
                        "the"
                    } else {
                        "any"
                    }
                ),
            );
        }
        // Altitude.
        let diffs: Vec<f64> = v
            .iter()
            .filter_map(|r| Some((r.geom_ft? - r.baro_ft?).abs()))
            .collect();
        if let Some(m) = median(diffs) {
            if m > p.max_geom_baro_ft {
                let code = if m > 2.0 * p.max_geom_baro_ft {
                    "altitude-gross"
                } else {
                    "altitude"
                };
                push(
                    &mut rs,
                    code,
                    format!(
                        "median |geometric - barometric| {m:.0} ft above {:.0} ft",
                        p.max_geom_baro_ft
                    ),
                );
            }
        }
        // Entry.
        let f = &v[0];
        let a = alt_m(f);
        let interior = receivers.iter().all(|x| {
            let reach = reach_m(x, a);
            let d = dist_m(x.lat, x.lon, f.lat, f.lon);
            d > reach * (1.0 + p.reach_tolerance) || d < p.entry_interior_frac * reach
        }) && hearable(f);
        if interior && f.baro_ft.unwrap_or(0.0) >= p.entry_min_alt_ft && f.t - t0 > p.start_grace_s
        {
            push(
                &mut rs,
                "no-entry",
                format!(
                    "first heard deep inside coverage at {:.0} ft, {:.0} s after the data start",
                    f.baro_ft.unwrap_or(0.0),
                    f.t - t0
                ),
            );
            appear.push((targets.len(), f.t, f.lat, f.lon));
        }
        // Independent source.
        if let Some(refs) = &refs {
            if v.len() >= 5 {
                let matched = v
                    .iter()
                    .filter(|r| {
                        let lo = refs.partition_point(|x| x.0 < r.t - p.reference_window_s);
                        refs[lo..]
                            .iter()
                            .take_while(|x| x.0 <= r.t + p.reference_window_s)
                            .any(|x| dist_m(x.1, x.2, r.lat, r.lon) <= p.reference_radius_m)
                    })
                    .count();
                let share = matched as f64 / v.len() as f64;
                if share < p.reference_min_share {
                    push(&mut rs, "no-independent-track", format!("{:.0}% of reports have a reference track point within {:.0} s and {:.1} km", share * 100.0, p.reference_window_s, p.reference_radius_m / 1000.0));
                }
            }
        }
        targets.push(Target {
            address: match salt {
                Some(s) => crate::transport::pseudonym(s, id),
                None => id.clone(),
            },
            reports: v.len(),
            first_utc: crate::forensics::inputs::iso(v[0].t),
            last_utc: crate::forensics::inputs::iso(v[v.len() - 1].t),
            score: 0.0,
            flagged: false,
            reasons: rs,
        });
    }
    // Clusters of appearances with no entry.
    for (i, &(k, t, la, lo)) in appear.iter().enumerate() {
        let near: Vec<usize> = appear
            .iter()
            .enumerate()
            .filter(|(j, x)| {
                *j != i
                    && (x.1 - t).abs() <= p.cluster_window_s
                    && dist_m(x.2, x.3, la, lo) <= p.cluster_radius_m
            })
            .map(|(_, x)| x.0)
            .collect();
        if near.len() + 1 >= p.cluster_min_targets {
            push(&mut targets[k].reasons, "cluster", format!("one of {} targets first heard inside coverage within {:.0} s and {:.0} km of each other", near.len() + 1, p.cluster_window_s, p.cluster_radius_m / 1000.0));
        }
    }
    let mut counts = BTreeMap::new();
    for t in &mut targets {
        let keep: f64 = t.reasons.iter().map(|r| 1.0 - r.weight).product();
        t.score = 1.0 - keep;
        t.flagged = t.score >= p.flag_score;
        for r in &t.reasons {
            *counts.entry(r.code.to_string()).or_insert(0) += 1;
        }
    }
    targets.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.address.cmp(&b.address)));
    let flagged = targets.iter().filter(|t| t.flagged).count();
    Ok(Report {
        format: FORMAT,
        method: METHOD,
        label: "MODELLED: plausibility evidence from the reports, declared receivers and an optional user-supplied reference",
        params: p.clone(),
        receivers: receivers.to_vec(),
        verdict: format!(
            "{flagged} of {} target(s) at or above the plausibility score {:.2} for review; {} report(s) used. No transmitter position is estimated.",
            targets.len(),
            p.flag_score,
            st.used
        ),
        stats: st,
        targets,
        flagged,
        reason_counts: counts,
        pseudonymised: salt.is_some(),
        not_done: "No emitter location: no time difference of arrival, no multilateration of a transmitter, no direction finding.",
    })
}

/// A SYNTHETIC scene: `(adsb.csv, receivers.toml, reference.csv, injected addresses)`.
/// 45 genuine aircraft (40 crossing coverage from outside, 5 departures) and six injected
/// cases: a ghost cluster of four, a teleporting address, an address in two places, an
/// unhearable target, a geometric/barometric mismatch and a Mach-4 track.
pub fn synth() -> (String, String, String, Vec<String>) {
    let rxs = [
        Receiver {
            id: "RX-N".into(),
            lat: 52.0,
            lon: 5.0,
            antenna_m: 30.0,
            range_nm: 150.0,
        },
        Receiver {
            id: "RX-S".into(),
            lat: 51.5,
            lon: 5.4,
            antenna_m: 20.0,
            range_nm: 150.0,
        },
    ];
    let (clat, clon) = (51.75, 5.2);
    let mut s: u64 = 99;
    let mut rnd = || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let t0 = 1_763_700_000.0;
    let mut rows: Vec<(f64, String)> = Vec::new();
    let mut refs: Vec<(f64, f64, f64)> = Vec::new();
    let heard = |lat: f64, lon: f64, alt_ft: f64| -> Option<&Receiver> {
        rxs.iter()
            .find(|x| dist_m(x.lat, x.lon, lat, lon) <= reach_m(x, alt_ft * FT))
    };
    let row = |rows: &mut Vec<(f64, String)>,
               t: f64,
               id: &str,
               lat: f64,
               lon: f64,
               baro: f64,
               geom: f64,
               rx: &str| {
        rows.push((
            t,
            format!("{t:.1},{id},{lat:.5},{lon:.5},{baro:.0},{geom:.0},8,9,adsb_icao,{rx}"),
        ));
    };
    let next_id = |k: u32| format!("{:06x}", 0x400000 + k * 7919);
    // Genuine crossings.
    for k in 0..40u32 {
        let id = next_id(k);
        let hdg = rnd() * std::f64::consts::TAU;
        let off = (rnd() - 0.5) * 200_000.0;
        let spd = (380.0 + rnd() * 100.0) * KT;
        let alt = 25_000.0 + (rnd() * 14.0).floor() * 1_000.0;
        let geom = alt + (rnd() - 0.5) * 600.0;
        let start = -1_800.0 + rnd() * 2_400.0;
        let (e0, n0) = (
            -hdg.sin() * 450_000.0 + hdg.cos() * off,
            -hdg.cos() * 450_000.0 - hdg.sin() * off,
        );
        let mut t = 0.0;
        while t < 3_600.0 {
            let tt = start + t;
            t += 10.0;
            if !(0.0..1_800.0).contains(&tt) || rnd() < 0.05 {
                continue;
            }
            let (e, n) = (
                e0 + hdg.sin() * spd * (t - 10.0),
                n0 + hdg.cos() * spd * (t - 10.0),
            );
            let (lat, lon) = offset_deg(clat, clon, e, n);
            if let Some(x) = heard(lat, lon, alt) {
                let rx = x.id.clone();
                row(&mut rows, t0 + tt, &id, lat, lon, alt, geom, &rx);
                let (rl, ro) = offset_deg(lat, lon, (rnd() - 0.5) * 400.0, (rnd() - 0.5) * 400.0);
                refs.push((t0 + tt, rl, ro));
            }
        }
    }
    // Departures from near RX-S, climbing.
    for k in 40..45u32 {
        let id = next_id(k);
        let hdg = rnd() * std::f64::consts::TAU;
        let begin = 200.0 + rnd() * 1_200.0;
        for i in 0..60 {
            let tt = begin + i as f64 * 10.0;
            if tt >= 1_800.0 {
                break;
            }
            let d = 120.0 * (i as f64 * 10.0) + 0.5 * 0.3 * (i as f64 * 10.0).powi(2);
            let (lat, lon) = offset_deg(51.52, 5.38, hdg.sin() * d, hdg.cos() * d);
            let alt = (500.0 + 2_500.0 * i as f64 / 6.0).min(30_000.0);
            row(&mut rows, t0 + tt, &id, lat, lon, alt, alt + 150.0, "RX-S");
            refs.push((t0 + tt, lat, lon));
        }
    }
    // Injected: a ghost cluster of four, appearing together mid-coverage.
    let mut inj = Vec::new();
    for g in 0..4u32 {
        let id = format!("{:06x}", 0xa0b000 + g);
        inj.push(id.clone());
        for i in 0..30 {
            let tt = 900.0 + g as f64 * 15.0 + i as f64 * 10.0;
            let (lat, lon) = offset_deg(
                clat,
                clon,
                g as f64 * 5_000.0 + 2_300.0 * i as f64,
                3_000.0 * g as f64,
            );
            row(
                &mut rows,
                t0 + tt,
                &id,
                lat,
                lon,
                34_000.0,
                34_200.0,
                "RX-N",
            );
        }
    }
    // A teleporting address: a genuine-looking track that jumps 60 km once.
    let id = "a0c001".to_string();
    inj.push(id.clone());
    for i in 0..80 {
        let tt = 100.0 + i as f64 * 10.0;
        let jump = if i >= 40 { 60_000.0 } else { 0.0 };
        let (lat, lon) = offset_deg(
            clat,
            clon,
            -260_000.0 + 2_300.0 * i as f64 + jump,
            -40_000.0,
        );
        row(
            &mut rows,
            t0 + tt,
            &id,
            lat,
            lon,
            31_000.0,
            31_100.0,
            "RX-N",
        );
        if i < 40 {
            refs.push((t0 + tt, lat, lon));
        }
    }
    // The same address in two places: a genuine track and a false one interleaved.
    let id = "a0c002".to_string();
    inj.push(id.clone());
    for i in 0..60 {
        let tt = 300.0 + i as f64 * 5.0;
        let (lat, lon) = if i % 2 == 0 {
            offset_deg(clat, clon, 40_000.0 + 1_150.0 * i as f64, 20_000.0)
        } else {
            offset_deg(clat, clon, -60_000.0, -70_000.0 + 1_150.0 * i as f64)
        };
        row(
            &mut rows,
            t0 + tt,
            &id,
            lat,
            lon,
            29_000.0,
            29_100.0,
            "RX-S",
        );
        if i % 2 == 0 {
            refs.push((t0 + tt, lat, lon));
        }
    }
    // An unhearable target: 420 km from both receivers, reported by RX-N.
    let id = "a0c003".to_string();
    inj.push(id.clone());
    for i in 0..20 {
        let tt = 600.0 + i as f64 * 10.0;
        let (lat, lon) = offset_deg(clat, clon, 420_000.0, 2_300.0 * i as f64);
        row(
            &mut rows,
            t0 + tt,
            &id,
            lat,
            lon,
            35_000.0,
            35_100.0,
            "RX-N",
        );
    }
    // Geometric and barometric altitude 8000 ft apart, on a track entering from outside.
    let id = "a0c004".to_string();
    inj.push(id.clone());
    for i in 0..60 {
        let tt = 400.0 + i as f64 * 10.0;
        let (lat, lon) = offset_deg(clat, clon, -270_000.0 + 2_300.0 * i as f64, 10_000.0);
        if heard(lat, lon, 33_000.0).is_some() {
            row(
                &mut rows,
                t0 + tt,
                &id,
                lat,
                lon,
                33_000.0,
                41_000.0,
                "RX-N",
            );
            refs.push((t0 + tt, lat, lon));
        }
    }
    // A Mach-4 track entering from outside.
    let id = "a0c005".to_string();
    inj.push(id.clone());
    for i in 0..30 {
        let tt = 1_000.0 + i as f64 * 10.0;
        let (lat, lon) = offset_deg(clat, clon, 10_000.0, -270_000.0 + 13_600.0 * i as f64);
        if heard(lat, lon, 40_000.0).is_some() {
            row(
                &mut rows,
                t0 + tt,
                &id,
                lat,
                lon,
                40_000.0,
                40_100.0,
                "RX-S",
            );
        }
    }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut csv = String::from(
        "timestamp,aircraft_id,lat,lon,alt_baro_ft,alt_geom_ft,nic,nacp,source_type,receiver_id\n",
    );
    for (_, r) in rows {
        csv.push_str(&r);
        csv.push('\n');
    }
    let mut rtoml = String::new();
    for x in &rxs {
        rtoml.push_str(&format!(
            "[[receiver]]\nid = \"{}\"\nlat = {}\nlon = {}\nantenna_m = {}\nrange_nm = {}\n\n",
            x.id, x.lat, x.lon, x.antenna_m, x.range_nm
        ));
    }
    refs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut rcsv = String::from("timestamp,lat,lon\n");
    for (t, a, b) in refs {
        rcsv.push_str(&format!("{t:.1},{a:.5},{b:.5}\n"));
    }
    (csv, rtoml, rcsv, inj)
}
