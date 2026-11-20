// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Telematics and road-charging GNSS anti-spoof checks (backlog B3002)
//!
//! Research 2026-11-20 gaps 07:I03, I05, I07, I13, I16 and 05:#66-#73: road-charging
//! on-board units, insurance telematics and fleet trackers trust the GNSS fix they are given.
//! This module checks a recorded telematics stream for the signs of jamming and spoofing
//! using only what such units record, and turns them into evidence:
//!
//! * [`check`]: per vehicle, seven indicators (a position jump faster than the vehicle can
//!   move; GNSS speed against wheel or CAN speed; a frozen position while the wheels turn; an
//!   outage while moving or with the ignition on; C/N0 below or above the vehicle's own
//!   median; an AGC drop; receiver time against the logger clock; GNSS position against a
//!   network position), merged into episodes with a verdict (jamming indicated, spoofing
//!   indicated, both, or inconsistent with the cause undetermined), the distance driven during
//!   each episode, a jammer-fingerprint class for jamming episodes, and per-trip trust for
//!   pay-as-you-drive scoring.
//! * [`evaluate_obu`]: an on-board-unit fraud evaluation over test runs with recorded attack
//!   windows (from a conducted or shielded laboratory, or a sanctioned field event): detection
//!   rate with its Wilson interval, time to detect, false episodes per hour of clean driving,
//!   and the distance the unit failed to report (the charge at risk). Signed by the command
//!   as evidence for an EETS provider's own assessment.
//! * [`Library`]: the jammer fingerprint library (`report-templates/transport/
//!   jammer-fingerprints.json`), rules on receiver-side features only. A class describes the
//!   episode as seen from the vehicle; nothing here estimates where a jammer was.
//!
//! **Privacy rule.** Every output names a vehicle only by a salted pseudonym
//! ([`super::pseudonym`]; the salt is the operator's and is never written out) and a
//! position only as a coarse cell (default 0.1°). Times stay, because evidence needs them.
//!
//! **Label: MODELLED.** Rules over the records as supplied; tested on synthetic streams.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{counts, dist_m, quantile, wilson95, Table};

/// The command feature of `road check` and `road trips`.
pub const FEATURE: &str = "pro-telematics-antispoof";
/// The command feature of `road obu-eval` (signed evaluation).
pub const FEATURE_OBU: &str = "pro-obu-fraud-eval";
pub const FORMAT: &str = "kshana-pro-telematics-check/1";
pub const OBU_FORMAT: &str = "kshana-pro-obu-fraud-eval/1";
pub const LIBRARY_FORMAT: &str = "kshana-jammer-fingerprints/1";
pub const LABEL: &str = "MODELLED: indicator rules over the telematics records as supplied; tested on synthetic streams. An episode is evidence to investigate, not proof of an attack or of who caused it. No output estimates where a jammer or spoofer was.";
pub const PRIVACY: &str = "Vehicles are named by a salted pseudonym (the salt is not written out); positions appear only as coarse grid cells.";

/// The built-in fingerprint library.
pub const LIBRARY_JSON: &str =
    include_str!("../../report-templates/transport/jammer-fingerprints.json");

/// The indicators that point to jamming.
pub const JAM_KINDS: &[&str] = &["outage_moving", "cn0_drop", "agc_drop"];
/// The indicators that point to spoofing (or a receiver fault, alone).
pub const SPOOF_KINDS: &[&str] = &[
    "position_jump",
    "speed_mismatch",
    "frozen_position",
    "cn0_rise",
    "time_offset",
    "network_disagree",
];

/// Thresholds; every one has a default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    /// Fastest plausible vehicle speed (m/s).
    pub max_speed_mps: f64,
    /// GNSS against wheel speed tolerance (m/s) ...
    pub speed_tol_mps: f64,
    /// ... sustained this long (s).
    pub speed_persist_s: f64,
    /// A fix within this distance of the run start counts as frozen (m) ...
    pub freeze_m: f64,
    /// ... for this long while moving (s).
    pub freeze_s: f64,
    /// Wheel speed above which the vehicle is moving (m/s).
    pub moving_mps: f64,
    /// An outage this long while moving or with the ignition on is flagged (s).
    pub gap_s: f64,
    /// C/N0 this far below the vehicle's median (dB).
    pub cn0_drop_db: f64,
    /// C/N0 this far above the vehicle's median (dB).
    pub cn0_rise_db: f64,
    /// AGC this far below the vehicle's median (dB).
    pub agc_drop_db: f64,
    /// Receiver time against logger time, beyond the median offset (s).
    pub time_tol_s: f64,
    /// GNSS against network position (m).
    pub network_tol_m: f64,
    /// Flags closer than this merge into one episode (s).
    pub merge_s: f64,
    /// A new trip starts after a gap this long or an ignition off (s).
    pub trip_gap_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            max_speed_mps: 70.0,
            speed_tol_mps: 3.0,
            speed_persist_s: 10.0,
            freeze_m: 1.0,
            freeze_s: 30.0,
            moving_mps: 2.0,
            gap_s: 10.0,
            cn0_drop_db: 6.0,
            cn0_rise_db: 4.0,
            agc_drop_db: 3.0,
            time_tol_s: 1.0,
            network_tol_m: 5000.0,
            merge_s: 60.0,
            trip_gap_s: 600.0,
        }
    }
}

impl Params {
    fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        for (n, v) in [
            ("max_speed_mps", self.max_speed_mps),
            ("speed_tol_mps", self.speed_tol_mps),
            ("speed_persist_s", self.speed_persist_s),
            ("freeze_m", self.freeze_m),
            ("freeze_s", self.freeze_s),
            ("gap_s", self.gap_s),
            ("cn0_drop_db", self.cn0_drop_db),
            ("cn0_rise_db", self.cn0_rise_db),
            ("agc_drop_db", self.agc_drop_db),
            ("time_tol_s", self.time_tol_s),
            ("network_tol_m", self.network_tol_m),
            ("trip_gap_s", self.trip_gap_s),
        ] {
            if !super::pos(v) {
                p.push(format!("{n} must be > 0"));
            }
        }
        if !super::nonneg(self.merge_s) || !super::nonneg(self.moving_mps) {
            p.push("merge_s and moving_mps must be >= 0".into());
        }
        p
    }
}

/// One record of a stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub vehicle: String,
    pub fix: Option<(f64, f64)>,
    pub speed_mps: Option<f64>,
    pub wheel_mps: Option<f64>,
    pub sats: Option<f64>,
    pub cn0_dbhz: Option<f64>,
    pub agc_db: Option<f64>,
    pub gnss_time: Option<f64>,
    pub ignition: Option<bool>,
    pub network: Option<(f64, f64)>,
    pub truth: Option<(f64, f64)>,
}

/// Read a telematics CSV: `t` is required; `vehicle`, `lat`, `lon`, `speed_mps`,
/// `wheel_mps`, `sats`, `cn0_dbhz`, `agc_db`, `gnss_time`, `ignition`, `cell_lat`,
/// `cell_lon`, `truth_lat`, `truth_lon` are optional. An empty `lat` is no fix.
pub fn read_stream(table: &Table) -> Result<Vec<Sample>, String> {
    let c = |n: &str| table.col(n);
    let t = table.need("t")?;
    let (lat, lon) = (c("lat"), c("lon"));
    let mut out = Vec::with_capacity(table.rows.len());
    for i in 0..table.rows.len() {
        let pair = |a: Option<usize>, b: Option<usize>| -> Result<Option<(f64, f64)>, String> {
            Ok(match (table.num(i, a)?, table.num(i, b)?) {
                (Some(x), Some(y)) => Some((x, y)),
                _ => None,
            })
        };
        let ign = table.text(i, c("ignition"));
        out.push(Sample {
            t: table
                .num(i, Some(t))?
                .ok_or(format!("row {}: empty t", i + 2))?,
            vehicle: match table.text(i, c("vehicle")) {
                "" => "vehicle".into(),
                v => v.to_string(),
            },
            fix: pair(lat, lon)?,
            speed_mps: table.num(i, c("speed_mps"))?,
            wheel_mps: table.num(i, c("wheel_mps"))?,
            sats: table.num(i, c("sats"))?,
            cn0_dbhz: table.num(i, c("cn0_dbhz"))?,
            agc_db: table.num(i, c("agc_db"))?,
            gnss_time: table.num(i, c("gnss_time"))?,
            ignition: match ign {
                "" => None,
                "1" | "on" | "true" => Some(true),
                _ => Some(false),
            },
            network: pair(c("cell_lat"), c("cell_lon"))?,
            truth: pair(c("truth_lat"), c("truth_lon"))?,
        });
    }
    out.sort_by(|a, b| a.vehicle.cmp(&b.vehicle).then(a.t.total_cmp(&b.t)));
    Ok(out)
}

/// One indicator at one time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Flag {
    pub t: f64,
    pub kind: &'static str,
    pub detail: String,
}

fn moving(s: &Sample, p: &Params) -> bool {
    s.wheel_mps.is_some_and(|w| w > p.moving_mps)
}

/// The indicators of one vehicle's samples (time-sorted).
pub fn flags(v: &[Sample], p: &Params) -> Vec<Flag> {
    let mut out = Vec::new();
    let med = |f: &dyn Fn(&Sample) -> Option<f64>| {
        quantile(&v.iter().filter_map(f).collect::<Vec<_>>(), 0.5)
    };
    let cn0_med = med(&|s| s.cn0_dbhz);
    let agc_med = med(&|s| s.agc_db);
    let toff_med = med(&|s| s.gnss_time.map(|g| g - s.t));
    let mut last_fix: Option<(f64, (f64, f64))> = None;
    let mut mismatch_since: Option<f64> = None;
    let mut freeze: Option<(f64, (f64, f64))> = None;
    let mut outage_since: Option<f64> = None;
    let mut outage_flagged = false;
    for s in v {
        // Outage while moving or with the ignition on.
        let active = moving(s, p) || s.ignition == Some(true);
        match s.fix {
            None if active => {
                let since = *outage_since.get_or_insert(s.t);
                if !outage_flagged && s.t - since >= p.gap_s {
                    out.push(Flag {
                        t: since,
                        kind: "outage_moving",
                        detail: format!("no fix for {:.0} s while active", s.t - since),
                    });
                    outage_flagged = true;
                }
            }
            None => {}
            Some(_) => {
                outage_since = None;
                outage_flagged = false;
            }
        }
        if let (Some(c), Some(m)) = (s.cn0_dbhz, cn0_med) {
            if c < m - p.cn0_drop_db {
                out.push(Flag {
                    t: s.t,
                    kind: "cn0_drop",
                    detail: format!("C/N0 {c:.1} dB-Hz against median {m:.1}"),
                });
            } else if c > m + p.cn0_rise_db && s.fix.is_some() {
                out.push(Flag {
                    t: s.t,
                    kind: "cn0_rise",
                    detail: format!("C/N0 {c:.1} dB-Hz against median {m:.1}"),
                });
            }
        }
        if let (Some(a), Some(m)) = (s.agc_db, agc_med) {
            if a < m - p.agc_drop_db {
                out.push(Flag {
                    t: s.t,
                    kind: "agc_drop",
                    detail: format!("AGC {a:.1} dB against median {m:.1}"),
                });
            }
        }
        if let (Some(g), Some(m)) = (s.gnss_time, toff_med) {
            let d = g - s.t - m;
            if d.abs() > p.time_tol_s {
                out.push(Flag {
                    t: s.t,
                    kind: "time_offset",
                    detail: format!("receiver time {d:+.1} s from its usual offset"),
                });
            }
        }
        let Some(pos) = s.fix else {
            mismatch_since = None;
            freeze = None;
            continue;
        };
        if let Some(n) = s.network {
            let d = dist_m(pos.0, pos.1, n.0, n.1);
            if d > p.network_tol_m {
                out.push(Flag {
                    t: s.t,
                    kind: "network_disagree",
                    detail: format!("{:.1} km from the network position", d / 1000.0),
                });
            }
        }
        let mut implied = None;
        if let Some((t0, q)) = last_fix {
            let dt = s.t - t0;
            if dt > 0.0 {
                let d = dist_m(q.0, q.1, pos.0, pos.1);
                implied = Some(d / dt);
                if d / dt > p.max_speed_mps && d > 50.0 {
                    out.push(Flag {
                        t: s.t,
                        kind: "position_jump",
                        detail: format!("{:.0} m in {dt:.1} s", d),
                    });
                }
            }
        }
        last_fix = Some((s.t, pos));
        // GNSS speed (reported, else implied) against wheel speed.
        match (s.speed_mps.or(implied), s.wheel_mps) {
            (Some(g), Some(w)) if (g - w).abs() > p.speed_tol_mps => {
                let since = *mismatch_since.get_or_insert(s.t);
                if s.t - since >= p.speed_persist_s {
                    out.push(Flag {
                        t: s.t,
                        kind: "speed_mismatch",
                        detail: format!("GNSS {g:.1} m/s, wheels {w:.1} m/s"),
                    });
                    mismatch_since = Some(s.t + 1e-9);
                }
            }
            _ => mismatch_since = None,
        }
        // Frozen position while the wheels turn.
        if moving(s, p) {
            match freeze {
                Some((t0, q)) if dist_m(q.0, q.1, pos.0, pos.1) <= p.freeze_m => {
                    if s.t - t0 >= p.freeze_s {
                        out.push(Flag {
                            t: s.t,
                            kind: "frozen_position",
                            detail: format!(
                                "position unchanged for {:.0} s while moving",
                                s.t - t0
                            ),
                        });
                        freeze = Some((s.t, pos));
                    }
                }
                _ => freeze = Some((s.t, pos)),
            }
        } else {
            freeze = None;
        }
    }
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

/// Features of a jamming episode, from the vehicle's records only.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Features {
    pub duration_s: f64,
    pub wheel_km: f64,
    pub toggles_per_min: f64,
    pub cn0_drop_db: Option<f64>,
    pub agc_drop_db: Option<f64>,
}

/// One class rule (every stated bound must hold).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    pub min_duration_s: Option<f64>,
    pub max_duration_s: Option<f64>,
    pub min_wheel_km: Option<f64>,
    pub max_wheel_km: Option<f64>,
    pub min_toggles_per_min: Option<f64>,
    pub min_cn0_drop_db: Option<f64>,
}

impl Rule {
    pub fn matches(&self, f: &Features) -> bool {
        let ge = |b: Option<f64>, x: f64| b.is_none_or(|b| x >= b);
        let le = |b: Option<f64>, x: f64| b.is_none_or(|b| x <= b);
        ge(self.min_duration_s, f.duration_s)
            && le(self.max_duration_s, f.duration_s)
            && ge(self.min_wheel_km, f.wheel_km)
            && le(self.max_wheel_km, f.wheel_km)
            && ge(self.min_toggles_per_min, f.toggles_per_min)
            && self
                .min_cn0_drop_db
                .is_none_or(|b| f.cn0_drop_db.is_some_and(|d| d >= b))
    }
}

/// One fingerprint class.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub id: String,
    pub name: String,
    pub rule: Rule,
    pub meaning: String,
}

/// The fingerprint library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub format: String,
    pub version: String,
    pub title: String,
    pub basis: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub classes: Vec<Class>,
}

impl Library {
    /// The built-in library.
    pub fn builtin() -> Library {
        serde_json::from_str(LIBRARY_JSON).expect("built-in library parses")
    }

    /// Parse a library, checking the format and that the last class matches everything.
    pub fn parse(text: &str) -> Result<Library, String> {
        let l: Library = serde_json::from_str(text).map_err(|e| format!("library: {e}"))?;
        if l.format != LIBRARY_FORMAT {
            return Err(format!("library format '{}'", l.format));
        }
        if l.classes.last().map(|c| c.rule != Rule::default()) != Some(false) {
            return Err("the last class must have an empty rule (the catch-all)".into());
        }
        Ok(l)
    }

    /// The first class whose rule matches.
    pub fn classify(&self, f: &Features) -> &Class {
        self.classes
            .iter()
            .find(|c| c.rule.matches(f))
            .unwrap_or_else(|| self.classes.last().expect("non-empty"))
    }
}

/// One episode.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Episode {
    pub start: f64,
    pub end: f64,
    pub kinds: Vec<String>,
    pub flags: usize,
    pub verdict: String,
    /// Fingerprint class id (jamming episodes only).
    pub class: Option<String>,
    pub features: Features,
    /// Distance by GNSS fixes inside the episode (km).
    pub gnss_km: f64,
    /// Coarse cell of the last fix before the episode.
    pub cell: Option<String>,
}

fn verdict_of(kinds: &BTreeSet<&str>) -> &'static str {
    let jam = kinds.iter().any(|k| JAM_KINDS.contains(k));
    let spoof = kinds.iter().filter(|k| SPOOF_KINDS.contains(k)).count();
    match (jam, spoof) {
        (true, 0) => "jamming indicated",
        (true, _) => "jamming and spoofing indicated",
        (false, n) if n >= 2 => "spoofing indicated",
        _ => "inconsistent, cause undetermined",
    }
}

/// Merge flags into episodes and describe each.
pub fn episodes(
    v: &[Sample],
    fl: &[Flag],
    p: &Params,
    lib: &Library,
    cell_deg: f64,
) -> Vec<Episode> {
    let mut groups: Vec<(f64, f64, Vec<&Flag>)> = Vec::new();
    for f in fl {
        // An outage flag stands for the whole outage: extend its end to the next fix.
        let end = if f.kind == "outage_moving" {
            v.iter()
                .find(|s| s.t > f.t && s.fix.is_some())
                .map(|s| s.t)
                .unwrap_or_else(|| v.last().map(|s| s.t).unwrap_or(f.t))
        } else {
            f.t
        };
        match groups.last_mut() {
            Some(g) if f.t - g.1 <= p.merge_s => {
                g.1 = g.1.max(end);
                g.2.push(f);
            }
            _ => groups.push((f.t, end, vec![f])),
        }
    }
    groups
        .into_iter()
        .map(|(start, end, fs)| {
            let kinds: BTreeSet<&str> = fs.iter().map(|f| f.kind).collect();
            let verdict = verdict_of(&kinds);
            let inside: Vec<&Sample> = v.iter().filter(|s| s.t >= start && s.t <= end).collect();
            let mut f = Features {
                duration_s: end - start,
                ..Features::default()
            };
            let mut gnss_m = 0.0;
            let mut toggles = 0usize;
            for w in inside.windows(2) {
                let dt = w[1].t - w[0].t;
                if let (Some(a), Some(b)) = (w[0].wheel_mps, w[1].wheel_mps) {
                    f.wheel_km += 0.5 * (a + b) * dt / 1000.0;
                }
                if let (Some(a), Some(b)) = (w[0].fix, w[1].fix) {
                    gnss_m += dist_m(a.0, a.1, b.0, b.1);
                }
                if w[0].fix.is_some() != w[1].fix.is_some() {
                    toggles += 1;
                }
            }
            if f.duration_s > 0.0 {
                f.toggles_per_min = toggles as f64 / (f.duration_s / 60.0);
            }
            let med = |g: &dyn Fn(&Sample) -> Option<f64>| {
                quantile(&v.iter().filter_map(g).collect::<Vec<_>>(), 0.5)
            };
            let low = |g: &dyn Fn(&Sample) -> Option<f64>| {
                inside
                    .iter()
                    .filter_map(|s| g(s))
                    .fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x))))
            };
            f.cn0_drop_db = med(&|s| s.cn0_dbhz)
                .zip(low(&|s| s.cn0_dbhz))
                .map(|(m, l)| (m - l).max(0.0));
            f.agc_drop_db = med(&|s| s.agc_db)
                .zip(low(&|s| s.agc_db))
                .map(|(m, l)| (m - l).max(0.0));
            let jam = verdict.starts_with("jamming");
            let class = jam.then(|| lib.classify(&f).id.clone());
            let cell = v
                .iter()
                .rev()
                .find(|s| s.t <= start && s.fix.is_some())
                .and_then(|s| s.fix)
                .map(|(a, b)| super::cell(a, b, cell_deg));
            Episode {
                start,
                end,
                kinds: kinds.iter().map(|k| k.to_string()).collect(),
                flags: fs.len(),
                verdict: verdict.into(),
                class,
                features: f,
                gnss_km: gnss_m / 1000.0,
                cell,
            }
        })
        .collect()
}

/// One trip (pay-as-you-drive).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Trip {
    pub start: f64,
    pub end: f64,
    pub wheel_km: Option<f64>,
    pub gnss_km: f64,
    /// Share of the trip's time inside an episode.
    pub affected_share: f64,
    /// `trusted`, `partly affected` or `untrusted`.
    pub trust: String,
}

/// Split a vehicle's samples into trips and score each.
pub fn trips(v: &[Sample], eps: &[Episode], p: &Params) -> Vec<Trip> {
    let mut bounds: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, s) in v.iter().enumerate() {
        let off = s.ignition == Some(false);
        let gap = i > 0 && s.t - v[i - 1].t > p.trip_gap_s;
        if let Some(st) = start {
            if off || gap {
                bounds.push((st, i - 1));
                start = None;
            }
        }
        if start.is_none() && !off {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        bounds.push((st, v.len() - 1));
    }
    bounds
        .into_iter()
        .filter(|(a, b)| b > a)
        .map(|(a, b)| {
            let seg = &v[a..=b];
            let (t0, t1) = (seg[0].t, seg[seg.len() - 1].t);
            let mut wheel = 0.0;
            let mut any_wheel = false;
            let mut gnss = 0.0;
            for w in seg.windows(2) {
                if let (Some(x), Some(y)) = (w[0].wheel_mps, w[1].wheel_mps) {
                    wheel += 0.5 * (x + y) * (w[1].t - w[0].t);
                    any_wheel = true;
                }
                if let (Some(x), Some(y)) = (w[0].fix, w[1].fix) {
                    gnss += dist_m(x.0, x.1, y.0, y.1);
                }
            }
            let dur = (t1 - t0).max(1e-9);
            let affected: f64 = eps
                .iter()
                .map(|e| (e.end.min(t1) - e.start.max(t0)).max(0.0))
                .sum();
            let share = (affected / dur).min(1.0);
            let ratio_bad = any_wheel && wheel > 500.0 && !(0.9..=1.1).contains(&(gnss / wheel));
            let trust = if share > 0.2 || ratio_bad {
                "untrusted"
            } else if share > 0.0 {
                "partly affected"
            } else {
                "trusted"
            };
            Trip {
                start: t0,
                end: t1,
                wheel_km: any_wheel.then_some(wheel / 1000.0),
                gnss_km: gnss / 1000.0,
                affected_share: share,
                trust: trust.into(),
            }
        })
        .collect()
}

/// The check request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckRequest {
    pub title: String,
    /// The operator's pseudonymisation salt (never written out).
    pub salt: String,
    /// Grid of the coarse cells (degrees, at least 0.05).
    #[serde(default = "default_cell")]
    pub cell_deg: f64,
    /// The telematics CSV, relative to the request.
    pub stream: String,
    #[serde(default)]
    pub params: Params,
    /// A fingerprint library to use instead of the built-in one.
    pub library: Option<String>,
}

fn default_cell() -> f64 {
    0.1
}

/// One vehicle's results.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VehicleReport {
    pub vehicle: String,
    pub samples: usize,
    pub fixes: usize,
    pub span_s: f64,
    pub flags_by_kind: BTreeMap<String, usize>,
    pub episodes: Vec<Episode>,
    pub trips: Vec<Trip>,
}

/// The check report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CheckReport {
    pub format: &'static str,
    pub label: &'static str,
    pub privacy: &'static str,
    pub title: String,
    pub library_version: String,
    pub params: Params,
    pub vehicles: Vec<VehicleReport>,
    pub episodes_by_verdict: BTreeMap<String, usize>,
    pub episodes_by_class: BTreeMap<String, usize>,
    pub verdict: String,
}

fn load_library(base: &Path, path: &Option<String>) -> Result<Library, String> {
    match path {
        Some(p) => {
            let f = base.join(p);
            Library::parse(
                &std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?,
            )
        }
        None => Ok(Library::builtin()),
    }
}

/// Check every vehicle of a stream.
pub fn check_samples(
    samples: &[Sample],
    req: &CheckRequest,
    lib: &Library,
) -> Result<CheckReport, String> {
    let pr = req.params.problems();
    if !pr.is_empty() {
        return Err(pr.join("; "));
    }
    if req.salt.trim().len() < 8 {
        return Err("salt must be at least 8 characters (privacy rule)".into());
    }
    if !super::nonneg(req.cell_deg - 0.05) {
        return Err("cell_deg must be at least 0.05 (privacy rule)".into());
    }
    let mut by: BTreeMap<&str, Vec<Sample>> = BTreeMap::new();
    for s in samples {
        by.entry(s.vehicle.as_str()).or_default().push(s.clone());
    }
    let mut vehicles = Vec::new();
    for (id, v) in by {
        let fl = flags(&v, &req.params);
        let eps = episodes(&v, &fl, &req.params, lib, req.cell_deg);
        let tr = trips(&v, &eps, &req.params);
        vehicles.push(VehicleReport {
            vehicle: super::pseudonym(&req.salt, id),
            samples: v.len(),
            fixes: v.iter().filter(|s| s.fix.is_some()).count(),
            span_s: v.last().map(|s| s.t).unwrap_or(0.0) - v.first().map(|s| s.t).unwrap_or(0.0),
            flags_by_kind: counts(fl.iter().map(|f| f.kind)),
            episodes: eps,
            trips: tr,
        });
    }
    vehicles.sort_by(|a, b| a.vehicle.cmp(&b.vehicle));
    let all: Vec<&Episode> = vehicles.iter().flat_map(|v| &v.episodes).collect();
    let by_verdict = counts(all.iter().map(|e| e.verdict.as_str()));
    let by_class = counts(all.iter().filter_map(|e| e.class.as_deref()));
    let verdict = if all.is_empty() {
        format!("{} vehicle(s): no episode", vehicles.len())
    } else {
        format!(
            "{} vehicle(s): {} episode(s) ({})",
            vehicles.len(),
            all.len(),
            by_verdict
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Ok(CheckReport {
        format: FORMAT,
        label: LABEL,
        privacy: PRIVACY,
        title: req.title.clone(),
        library_version: lib.version.clone(),
        params: req.params.clone(),
        vehicles,
        episodes_by_verdict: by_verdict,
        episodes_by_class: by_class,
        verdict,
    })
}

/// Run a check request (paths relative to `base`).
pub fn check(req: &CheckRequest, base: &Path) -> Result<CheckReport, String> {
    let lib = load_library(base, &req.library)?;
    let t = Table::read(&base.join(&req.stream))?;
    check_samples(&read_stream(&t)?, req, &lib)
}

// ---------------------------------------------------------------------------------------
// On-board-unit fraud evaluation.

/// One test run.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub name: String,
    pub stream: String,
    /// `none` for a clean run, otherwise the attack recorded (free text, e.g. `jamming`,
    /// `spoofing-drag`, `meaconing`).
    pub attack: String,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

/// The evaluation request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObuRequest {
    pub title: String,
    /// The unit under test (make and model, as the applicant names it).
    pub unit: String,
    pub salt: String,
    #[serde(default = "default_cell")]
    pub cell_deg: f64,
    #[serde(default)]
    pub params: Params,
    pub library: Option<String>,
    pub runs: Vec<Run>,
}

/// One run's results.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RunResult {
    pub name: String,
    pub attack: String,
    pub detected: Option<bool>,
    pub latency_s: Option<f64>,
    pub false_episodes: usize,
    pub clean_hours: f64,
    /// Reference distance: truth track if present, else wheel distance (km).
    pub reference_km: Option<f64>,
    pub reference_basis: String,
    /// Distance reported by the unit's fixes, gaps longer than `gap_s` not bridged (km).
    pub reported_km: f64,
    /// `1 − reported/reference`.
    pub unreported_share: Option<f64>,
    pub episodes: Vec<Episode>,
}

/// The evaluation report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ObuReport {
    pub format: &'static str,
    pub label: &'static str,
    pub privacy: &'static str,
    pub title: String,
    pub unit: String,
    pub utc: String,
    pub runs: Vec<RunResult>,
    pub attack_runs: usize,
    pub detected: usize,
    pub detection_wilson95: Option<(f64, f64)>,
    pub median_latency_s: Option<f64>,
    pub false_episodes_per_hour: Option<f64>,
    pub unreported_share_attack: Option<f64>,
    pub unreported_share_clean: Option<f64>,
    pub framing: &'static str,
    pub verdict: String,
}

pub const OBU_FRAMING: &str = "Evidence for the EETS provider's or toll charger's own assessment of the unit's behaviour under GNSS attack. Kshana is not a notified body; this evaluation does not state suitability for use or conformity with any EETS requirement.";

fn track_km(points: impl Iterator<Item = (f64, Option<(f64, f64)>)>, max_gap_s: f64) -> f64 {
    let mut last: Option<(f64, (f64, f64))> = None;
    let mut d = 0.0;
    for (t, p) in points {
        if let Some(p) = p {
            if let Some((t0, q)) = last {
                if t - t0 <= max_gap_s {
                    d += dist_m(q.0, q.1, p.0, p.1);
                }
            }
            last = Some((t, p));
        }
    }
    d / 1000.0
}

/// Evaluate a unit over its runs (paths relative to `base`).
pub fn evaluate_obu(req: &ObuRequest, base: &Path, utc: &str) -> Result<ObuReport, String> {
    if req.runs.is_empty() {
        return Err("no runs".into());
    }
    let lib = load_library(base, &req.library)?;
    let mut runs = Vec::new();
    for r in &req.runs {
        let attack = r.attack.trim().to_ascii_lowercase();
        let window = match (attack.as_str(), r.start, r.end) {
            ("none", _, _) => None,
            (_, Some(a), Some(b)) if b > a => Some((a, b)),
            _ => return Err(format!("run {}: an attack run needs start < end", r.name)),
        };
        let mut samples = read_stream(&Table::read(&base.join(&r.stream))?)?;
        for s in &mut samples {
            s.vehicle = "unit".into();
        }
        let creq = CheckRequest {
            title: r.name.clone(),
            salt: req.salt.clone(),
            cell_deg: req.cell_deg,
            stream: r.stream.clone(),
            params: req.params.clone(),
            library: None,
        };
        let rep = check_samples(&samples, &creq, &lib)?;
        let eps = rep
            .vehicles
            .into_iter()
            .next()
            .map(|v| v.episodes)
            .unwrap_or_default();
        let span = samples.last().map(|s| s.t).unwrap_or(0.0)
            - samples.first().map(|s| s.t).unwrap_or(0.0);
        let hit = |e: &Episode, (a, b): (f64, f64)| e.end >= a && e.start <= b + req.params.merge_s;
        let (detected, latency, false_eps, clean_s) = match window {
            Some(w) => {
                let first = eps
                    .iter()
                    .filter(|e| hit(e, w))
                    .map(|e| e.start)
                    .reduce(f64::min);
                (
                    Some(first.is_some()),
                    first.map(|t| (t - w.0).max(0.0)),
                    eps.iter().filter(|e| !hit(e, w)).count(),
                    (span - (w.1 - w.0)).max(0.0),
                )
            }
            None => (None, None, eps.len(), span),
        };
        let has_truth = samples.iter().any(|s| s.truth.is_some());
        let has_wheel = samples.iter().any(|s| s.wheel_mps.is_some());
        let (reference_km, basis) = if has_truth {
            (
                Some(track_km(
                    samples.iter().map(|s| (s.t, s.truth)),
                    f64::INFINITY,
                )),
                "truth track",
            )
        } else if has_wheel {
            let mut d = 0.0;
            for w in samples.windows(2) {
                if let (Some(a), Some(b)) = (w[0].wheel_mps, w[1].wheel_mps) {
                    d += 0.5 * (a + b) * (w[1].t - w[0].t);
                }
            }
            (Some(d / 1000.0), "wheel distance")
        } else {
            (None, "none supplied")
        };
        let reported = track_km(samples.iter().map(|s| (s.t, s.fix)), req.params.gap_s);
        runs.push(RunResult {
            name: r.name.clone(),
            attack: attack.clone(),
            detected,
            latency_s: latency,
            false_episodes: false_eps,
            clean_hours: clean_s / 3600.0,
            reference_km,
            reference_basis: basis.into(),
            reported_km: reported,
            unreported_share: reference_km
                .filter(|r| *r > 0.0)
                .map(|r| 1.0 - reported / r),
            episodes: eps,
        });
    }
    let att: Vec<&RunResult> = runs.iter().filter(|r| r.detected.is_some()).collect();
    let det = att.iter().filter(|r| r.detected == Some(true)).count();
    let lat: Vec<f64> = att.iter().filter_map(|r| r.latency_s).collect();
    let clean_h: f64 = runs.iter().map(|r| r.clean_hours).sum();
    let fe: usize = runs.iter().map(|r| r.false_episodes).sum();
    let share = |sel: &dyn Fn(&RunResult) -> bool| -> Option<f64> {
        let (mut rep, mut refk) = (0.0, 0.0);
        for r in runs.iter().filter(|r| sel(r)) {
            if let Some(k) = r.reference_km {
                rep += r.reported_km;
                refk += k;
            }
        }
        (refk > 0.0).then(|| 1.0 - rep / refk)
    };
    let ua = share(&|r| r.detected.is_some());
    let uc = share(&|r| r.detected.is_none());
    let verdict = format!(
        "{}: {det} of {} attack run(s) detected{}; {fe} false episode(s) over {:.2} h of clean driving; unreported distance under attack {}",
        req.unit,
        att.len(),
        quantile(&lat, 0.5)
            .map(|m| format!(" (median time to detect {m:.0} s)"))
            .unwrap_or_default(),
        clean_h,
        ua.map(|u| format!("{:.1} %", 100.0 * u)).unwrap_or_else(|| "not measured".into())
    );
    Ok(ObuReport {
        format: OBU_FORMAT,
        label: LABEL,
        privacy: PRIVACY,
        title: req.title.clone(),
        unit: req.unit.clone(),
        utc: utc.to_string(),
        attack_runs: att.len(),
        detected: det,
        detection_wilson95: wilson95(det, att.len()),
        median_latency_s: quantile(&lat, 0.5),
        false_episodes_per_hour: (clean_h > 0.0).then(|| fe as f64 / clean_h),
        unreported_share_attack: ua,
        unreported_share_clean: uc,
        framing: OBU_FRAMING,
        runs,
        verdict,
    })
}

/// The evaluation page.
pub fn obu_html(r: &ObuReport) -> String {
    use super::{f3, fo};
    let rows = r
        .runs
        .iter()
        .map(|x| {
            vec![
                x.name.clone(),
                x.attack.clone(),
                match x.detected {
                    Some(true) => "detected".into(),
                    Some(false) => "not detected".into(),
                    None => "clean run".into(),
                },
                fo(x.latency_s),
                x.false_episodes.to_string(),
                fo(x.reference_km),
                f3(x.reported_km),
                x.unreported_share
                    .map(|u| format!("{:.1} %", 100.0 * u))
                    .unwrap_or_else(|| "-".into()),
            ]
        })
        .collect();
    super::page(
        &format!("On-board unit GNSS attack evaluation: {}", r.title),
        r.label,
        &r.verdict,
        &[r.framing.to_string(), r.privacy.to_string()],
        &[(
            "Runs".into(),
            vec![
                "Run",
                "Attack",
                "Result",
                "Time to detect (s)",
                "False episodes",
                "Reference (km)",
                "Reported (km)",
                "Unreported",
            ],
            rows,
        )],
    )
}

/// The check page.
pub fn check_html(r: &CheckReport) -> String {
    use super::f3;
    let rows = r
        .vehicles
        .iter()
        .flat_map(|v| {
            v.episodes.iter().map(move |e| {
                vec![
                    v.vehicle.clone(),
                    format!("{:.0}", e.start),
                    f3(e.features.duration_s),
                    e.verdict.clone(),
                    e.class.clone().unwrap_or_else(|| "-".into()),
                    e.kinds.join(" "),
                    f3(e.features.wheel_km),
                    e.cell.clone().unwrap_or_else(|| "-".into()),
                ]
            })
        })
        .collect();
    super::page(
        &format!("Telematics GNSS check: {}", r.title),
        r.label,
        &r.verdict,
        &[r.privacy.to_string()],
        &[(
            "Episodes".into(),
            vec![
                "Vehicle",
                "Start (unix s)",
                "Duration (s)",
                "Verdict",
                "Fingerprint",
                "Indicators",
                "Driven (km)",
                "Cell",
            ],
            rows,
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drive(n: usize, f: impl Fn(usize, &mut Sample)) -> Vec<Sample> {
        (0..n)
            .map(|i| {
                let (lat, lon) = super::super::offset_deg(52.0, 13.0, 0.0, 20.0 * i as f64);
                let mut s = Sample {
                    t: i as f64,
                    vehicle: "v1".into(),
                    fix: Some((lat, lon)),
                    speed_mps: Some(20.0),
                    wheel_mps: Some(20.0),
                    cn0_dbhz: Some(42.0),
                    ignition: Some(true),
                    ..Sample::default()
                };
                f(i, &mut s);
                s
            })
            .collect()
    }

    #[test]
    fn clean_drive_has_no_flags() {
        let v = drive(600, |_, _| {});
        assert!(flags(&v, &Params::default()).is_empty());
    }

    #[test]
    fn jamming_while_moving_is_classified() {
        let v = drive(1200, |i, s| {
            if (300..700).contains(&i) {
                s.fix = None;
                s.speed_mps = None;
                s.cn0_dbhz = Some(25.0);
            }
        });
        let p = Params::default();
        let fl = flags(&v, &p);
        let eps = episodes(&v, &fl, &p, &Library::builtin(), 0.1);
        assert_eq!(eps.len(), 1, "{eps:?}");
        assert_eq!(eps[0].verdict, "jamming indicated");
        assert_eq!(eps[0].class.as_deref(), Some("travels-with-vehicle"));
        assert!(eps[0].features.wheel_km > 7.0);
    }

    #[test]
    fn library_parses_and_needs_catch_all() {
        let l = Library::parse(LIBRARY_JSON).unwrap();
        assert!(l.classes.len() >= 4);
        let bad = LIBRARY_JSON.replace("\"rule\": {}", "\"rule\": {\"min_duration_s\": 1.0}");
        assert!(Library::parse(&bad).is_err());
    }
}
