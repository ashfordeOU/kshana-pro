// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # PNT-DP: a reference implementation of shipborne PNT data processing (IMO MSC.1/Circ.1575)
//!
//! W1119 C2 (research 02 gap 2). Multi-source position processing over GNSS, a second GNSS,
//! DGNSS, eLoran and an inertial/log velocity input, epoch by epoch:
//!
//! 1. **Input checks** (Circ.1575 annex paragraphs 53 to 57: completeness, correctness, update
//!    rate, plausibility): a sample is `incomplete` (missing or out-of-range position or
//!    sigma), `invalid` (the source says so), `stale` (older than `max_age_s`), `frozen` (the
//!    same position repeated for `frozen_epochs` samples: a live sensor with non-zero noise
//!    does not repeat itself exactly), or a `jump` (its displacement since its own previous
//!    sample differs from the inertial/log motion, or exceeds `max_speed_mps` without one, by
//!    more than `k` times the combined sigma). A sample that fails is excluded.
//! 2. **Dead reckoning**: the inertial/log velocity carries an independent position forward
//!    from an anchor (the first solution with integrity of at least medium), its sigma growing
//!    with `sigma_mps * dt`; it is re-anchored only when its sigma exceeds `reanchor_sigma_m`,
//!    the epoch's integrity is high and it agrees with the solution, so a slow walk-off cannot
//!    drag it along.
//! 3. **Consistency** (paragraphs 84 to 88): two candidates agree when their distance is at
//!    most `k sqrt(s1^2 + s2^2)`. The processor finds the mutually consistent sets of
//!    candidates and the set spanning the most **groups** (sources of one group share a signal
//!    path, for example every GNSS service on L-band, and never confirm each other). A unique
//!    such set wins; sources outside it are `inconsistent`. Two or more sets spanning as many
//!    groups but different ones is an unresolved `disagreement`.
//! 4. **Selection and integrity** (paragraph 47 grading: none, low, medium, high): the winning
//!    set's smallest-sigma position source is the solution. Integrity is
//!    * **high**: the set spans at least three groups (a fault of any one group is detected
//!      and excluded) and the protection level is within `alert_limit_m`;
//!    * **medium**: it spans two groups (consistency across largely uncorrelated sources), or
//!      three with the protection level beyond the alert limit;
//!    * **low**: one group only (single-source plausibility and consistency), or an
//!      unresolved disagreement whose competing sets share a group, the solution then taken
//!      from that shared group;
//!    * **none**: no candidate passed the input checks, an unresolved disagreement with no
//!      shared group, or dead reckoning alone.
//!
//!    The protection level is `min over agreeing members r of another group of
//!    (|solution - r| + k sigma_r)`: it bounds the solution's true error whenever at least one
//!    such member is fault-free with an error within `k sigma_r`.
//! 5. **Status** (paragraphs 92 and 93): `raised` when the integrity level falls from the
//!    previous epoch (redundancy lost), `ok` otherwise.
//!
//! Time is not processed here: the time-source selector with the same group logic is
//! [`crate::multi_source`].
//!
//! **Evidence label: MODELLED.** Checked against hand-specified test vectors (spoof, jump,
//! frozen sensor, disagreement; [`vectors`]) and a Monte Carlo test of the protection level on
//! synthetic Gaussian errors. No recorded ship data has been processed; the rule is a
//! reference reading of the circular's principles, not a type-approved implementation, and
//! the circular assigns integrity levels to equipment methods, not to this code. The
//! circular's paragraph references are as read by the open engine's `imo-circ-1575` standards
//! pack (read in full 2026-10-18); the limits used here are configuration, illustrative until
//! confirmed against the circular and the equipment standard.

pub mod vectors;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// The licence feature of `kshana-pro pntdp run` (the multi-source trust feature: PNT-DP is
/// its position-domain counterpart). `pntdp vectors` is free.
pub const FEATURE: &str = "pro-multi-source-trust";
/// The method tag.
pub const METHOD: &str = "kshana-pro/pntdp/v1";
pub const LABEL: &str = "MODELLED: reference PNT data processing after IMO MSC.1/Circ.1575 \
(paragraphs 47, 53 to 57, 84 to 88, 92 and 93), checked on synthetic test vectors; not \
type-approved equipment and not a statement that any equipment meets the circular.";

/// The kind of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Gnss,
    Dgnss,
    Eloran,
    /// Inertial or log: velocity (north, east), no position.
    Inertial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDef {
    pub id: String,
    pub kind: SourceKind,
    /// Sources of one group share a signal path and never confirm each other.
    pub group: String,
    /// Stated one-sigma horizontal error per axis, m (position sources); a sample's own sigma
    /// overrides it.
    #[serde(default)]
    pub sigma_m: f64,
    /// Inertial/log: one-sigma velocity error per axis, m/s.
    #[serde(default)]
    pub sigma_mps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Limits {
    /// Consistency and protection multiplier.
    pub k: f64,
    pub max_speed_mps: f64,
    pub max_age_s: f64,
    pub frozen_epochs: usize,
    pub reanchor_sigma_m: f64,
    /// Horizontal alert limit, m (IMO Table C-1 operational level A, 100 m, by default).
    pub alert_limit_m: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            k: 4.0,
            max_speed_mps: 25.0,
            max_age_s: 3.0,
            frozen_epochs: 5,
            reanchor_sigma_m: 5.0,
            alert_limit_m: 100.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "source")]
    pub sources: Vec<SourceDef>,
    #[serde(default)]
    pub limits: Limits,
}

impl Config {
    pub fn parse(src: &str) -> Result<Config, String> {
        let c: Config = toml::from_str(src).map_err(|e| format!("PNT-DP config: {e}"))?;
        c.validate()?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut ids = BTreeSet::new();
        let mut inertial = 0;
        for s in &self.sources {
            if s.id.is_empty()
                || !s
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(format!("PNT-DP config: bad source id '{}'", s.id));
            }
            if !ids.insert(s.id.as_str()) {
                return Err(format!("PNT-DP config: duplicate source '{}'", s.id));
            }
            if s.kind == SourceKind::Inertial {
                inertial += 1;
                if !(s.sigma_mps.is_finite() && s.sigma_mps > 0.0) {
                    return Err(format!("{}: an inertial source needs sigma_mps > 0", s.id));
                }
            } else if !(s.sigma_m.is_finite() && s.sigma_m > 0.0) {
                return Err(format!("{}: a position source needs sigma_m > 0", s.id));
            }
        }
        if inertial > 1 {
            return Err("PNT-DP config: at most one inertial/log source".into());
        }
        let l = &self.limits;
        if !(l.k > 0.0
            && l.max_speed_mps > 0.0
            && l.max_age_s > 0.0
            && l.frozen_epochs >= 2
            && l.reanchor_sigma_m > 0.0
            && l.alert_limit_m > 0.0)
        {
            return Err("PNT-DP config: limits must be positive (frozen_epochs >= 2)".into());
        }
        Ok(())
    }

    pub fn source(&self, id: &str) -> Option<&SourceDef> {
        self.sources.iter().find(|s| s.id == id)
    }
}

/// One input sample.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub t_s: f64,
    pub source: String,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    pub sigma_m: Option<f64>,
    pub valid: bool,
    pub vn_mps: Option<f64>,
    pub ve_mps: Option<f64>,
}

/// The CSV header of an input file.
pub const INPUT_HEADER: &str = "t_s,source,lat_deg,lon_deg,sigma_m,valid,vn_mps,ve_mps";

/// Parse the input CSV ([`INPUT_HEADER`]; empty cells are absent values; `valid` is 1/0 or
/// true/false). Samples must be in non-decreasing time.
pub fn parse_inputs(cfg: &Config, text: &str) -> Result<Vec<Sample>, String> {
    let mut out = Vec::new();
    let mut last = f64::NEG_INFINITY;
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("t_s") {
            continue;
        }
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        if f.len() != 8 {
            return Err(format!(
                "line {}: expected 8 fields ({INPUT_HEADER})",
                n + 1
            ));
        }
        let num = |s: &str, what: &str| -> Result<Option<f64>, String> {
            if s.is_empty() {
                Ok(None)
            } else {
                s.parse::<f64>()
                    .map(Some)
                    .map_err(|e| format!("line {}: {what}: {e}", n + 1))
            }
        };
        let t = num(f[0], "t_s")?.ok_or_else(|| format!("line {}: t_s missing", n + 1))?;
        if !t.is_finite() || t < last {
            return Err(format!("line {}: time must be non-decreasing", n + 1));
        }
        last = t;
        if cfg.source(f[1]).is_none() {
            return Err(format!("line {}: unknown source '{}'", n + 1, f[1]));
        }
        out.push(Sample {
            t_s: t,
            source: f[1].to_string(),
            lat_deg: num(f[2], "lat_deg")?,
            lon_deg: num(f[3], "lon_deg")?,
            sigma_m: num(f[4], "sigma_m")?,
            valid: matches!(f[5], "1" | "true" | ""),
            vn_mps: num(f[6], "vn_mps")?,
            ve_mps: num(f[7], "ve_mps")?,
        });
    }
    Ok(out)
}

/// Write samples as input CSV.
pub fn inputs_csv(samples: &[Sample]) -> String {
    let o = |v: Option<f64>, dp: usize| v.map(|x| format!("{x:.dp$}")).unwrap_or_default();
    let mut s = format!("{INPUT_HEADER}\n");
    for x in samples {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            x.t_s,
            x.source,
            o(x.lat_deg, 9),
            o(x.lon_deg, 9),
            o(x.sigma_m, 3),
            if x.valid { 1 } else { 0 },
            o(x.vn_mps, 4),
            o(x.ve_mps, 4)
        ));
    }
    s
}

/// Integrity levels after Circ.1575 paragraph 47, in increasing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Integrity {
    None,
    Low,
    Medium,
    High,
}

impl Integrity {
    pub fn as_str(self) -> &'static str {
        match self {
            Integrity::None => "none",
            Integrity::Low => "low",
            Integrity::Medium => "medium",
            Integrity::High => "high",
        }
    }
    pub fn parse(s: &str) -> Option<Integrity> {
        Some(match s {
            "none" => Integrity::None,
            "low" => Integrity::Low,
            "medium" => Integrity::Medium,
            "high" => Integrity::High,
            _ => return None,
        })
    }
}

/// The output of one epoch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Output {
    pub t_s: f64,
    /// The selected source (`ins` style id of the inertial source when dead reckoning alone),
    /// `None` when there is no solution.
    pub selected: Option<String>,
    pub lat_deg: Option<f64>,
    pub lon_deg: Option<f64>,
    pub sigma_m: Option<f64>,
    pub protection_level_m: Option<f64>,
    pub integrity: Integrity,
    /// Groups spanned by the winning consistent set.
    pub groups: usize,
    /// Per source, its flags (sorted).
    pub flags: BTreeMap<String, Vec<String>>,
    /// `ok` or `raised` (the integrity level fell since the previous epoch).
    pub status: &'static str,
}

impl Output {
    /// Flags as `source:flag[+flag];...` (sorted), empty when none.
    pub fn flags_text(&self) -> String {
        self.flags
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| format!("{k}:{}", v.join("+")))
            .collect::<Vec<_>>()
            .join(";")
    }
}

/// Local east/north metres about a reference point (spherical, adequate over tens of km).
#[derive(Debug, Clone, Copy)]
struct Local {
    lat0: f64,
    lon0: f64,
    mlat: f64,
    mlon: f64,
}

impl Local {
    fn new(lat0: f64, lon0: f64) -> Local {
        let (mlat, mlon) = crate::pics::log::metres_per_degree(lat0);
        Local {
            lat0,
            lon0,
            mlat,
            mlon,
        }
    }
    fn en(&self, lat: f64, lon: f64) -> (f64, f64) {
        ((lon - self.lon0) * self.mlon, (lat - self.lat0) * self.mlat)
    }
    fn latlon(&self, e: f64, n: f64) -> (f64, f64) {
        (self.lat0 + n / self.mlat, self.lon0 + e / self.mlon)
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[derive(Debug, Clone, Default)]
struct History {
    last_t: Option<f64>,
    last_pos: Option<(f64, f64)>,
    last_raw: Option<(f64, f64)>,
    repeats: usize,
}

#[derive(Debug, Clone)]
struct Candidate {
    id: String,
    group: String,
    pos: (f64, f64),
    sigma: f64,
    is_dr: bool,
}

/// Dead reckoning: the position carried forward from an anchor, the anchor's sigma and time.
/// The velocity error is treated as a bias over the coast (fully correlated), so the
/// position sigma grows linearly: `sqrt(sigma0^2 + (sigma_mps (t - t0))^2)`.
#[derive(Debug, Clone, Copy)]
struct Dr {
    pos: (f64, f64),
    sigma0: f64,
    t0: f64,
}

impl Dr {
    fn sigma(&self, t: f64, sigma_mps: f64) -> f64 {
        (self.sigma0 * self.sigma0 + (sigma_mps * (t - self.t0)).powi(2)).sqrt()
    }
}

/// The processor's state across epochs.
pub struct Processor {
    cfg: Config,
    local: Option<Local>,
    hist: BTreeMap<String, History>,
    /// Dead reckoning, once anchored.
    dr: Option<Dr>,
    last_t: Option<f64>,
    last_integrity: Option<Integrity>,
}

impl Processor {
    pub fn new(cfg: Config) -> Result<Processor, String> {
        cfg.validate()?;
        Ok(Processor {
            cfg,
            local: None,
            hist: BTreeMap::new(),
            dr: None,
            last_t: None,
            last_integrity: None,
        })
    }

    fn inertial(&self) -> Option<&SourceDef> {
        self.cfg
            .sources
            .iter()
            .find(|s| s.kind == SourceKind::Inertial)
    }

    /// Process the samples of one epoch (all with time `t`).
    pub fn step(&mut self, t: f64, samples: &[&Sample]) -> Output {
        let k = self.cfg.limits.k;
        let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let add = |f: &mut BTreeMap<String, Vec<String>>, id: &str, what: &str| {
            let v = f.entry(id.to_string()).or_default();
            if !v.iter().any(|x| x == what) {
                v.push(what.to_string());
                v.sort();
            }
        };
        if self.local.is_none() {
            if let Some(s) = samples
                .iter()
                .find(|s| s.lat_deg.is_some() && s.lon_deg.is_some())
            {
                self.local = Some(Local::new(s.lat_deg.unwrap(), s.lon_deg.unwrap()));
            }
        }
        let dt_epoch = self.last_t.map(|l| t - l).unwrap_or(0.0);
        // Inertial/log velocity of this epoch.
        let ins_def = self.inertial().cloned();
        let vel = ins_def.as_ref().and_then(|d| {
            samples
                .iter()
                .find(|s| s.source == d.id && s.valid)
                .and_then(|s| match (s.vn_mps, s.ve_mps) {
                    (Some(n), Some(e)) if n.is_finite() && e.is_finite() => Some((e, n)),
                    _ => None,
                })
        });
        if let (Some(d), None) = (&ins_def, vel) {
            if samples.iter().any(|s| s.source == d.id) {
                add(&mut flags, &d.id, "invalid");
            }
        }
        // Propagate dead reckoning.
        if let (Some(dr), Some((ve, vn))) = (self.dr.as_mut(), vel) {
            dr.pos = (dr.pos.0 + ve * dt_epoch, dr.pos.1 + vn * dt_epoch);
        } else if vel.is_none() {
            self.dr = None;
        }
        // Input checks.
        let mut cands: Vec<Candidate> = Vec::new();
        for def in self
            .cfg
            .sources
            .iter()
            .filter(|s| s.kind != SourceKind::Inertial)
        {
            let Some(s) = samples.iter().find(|s| s.source == def.id) else {
                if let Some(h) = self.hist.get(&def.id) {
                    if h.last_t
                        .is_some_and(|lt| t - lt > self.cfg.limits.max_age_s)
                    {
                        add(&mut flags, &def.id, "stale");
                    }
                }
                continue;
            };
            let sigma = s.sigma_m.unwrap_or(def.sigma_m);
            let (Some(lat), Some(lon)) = (s.lat_deg, s.lon_deg) else {
                add(&mut flags, &def.id, "incomplete");
                continue;
            };
            if !(lat.is_finite()
                && lon.is_finite()
                && lat.abs() <= 90.0
                && lon.abs() <= 180.0
                && sigma.is_finite()
                && sigma > 0.0)
            {
                add(&mut flags, &def.id, "incomplete");
                continue;
            }
            if !s.valid {
                add(&mut flags, &def.id, "invalid");
                continue;
            }
            let local = self.local.expect("set from the first position");
            let pos = local.en(lat, lon);
            let h = self.hist.entry(def.id.clone()).or_default();
            // Frozen: the same raw position repeated.
            h.repeats = if h.last_raw == Some((lat, lon)) {
                h.repeats + 1
            } else {
                1
            };
            h.last_raw = Some((lat, lon));
            let frozen = h.repeats >= self.cfg.limits.frozen_epochs;
            // Rate / jump against the source's own previous sample.
            let mut jump = false;
            if let (Some(lt), Some(lp)) = (h.last_t, h.last_pos) {
                let dt = t - lt;
                if dt > 0.0 {
                    let d = (pos.0 - lp.0, pos.1 - lp.1);
                    let noise = k * (2.0f64).sqrt() * sigma;
                    jump = match (vel, &ins_def) {
                        (Some((ve, vn)), Some(id)) => {
                            let r = ((d.0 - ve * dt).powi(2) + (d.1 - vn * dt).powi(2)).sqrt();
                            r > noise + k * id.sigma_mps * dt
                        }
                        _ => {
                            (d.0 * d.0 + d.1 * d.1).sqrt()
                                > self.cfg.limits.max_speed_mps * dt + noise
                        }
                    };
                }
            }
            h.last_t = Some(t);
            h.last_pos = Some(pos);
            if frozen {
                add(&mut flags, &def.id, "frozen");
            }
            if jump {
                add(&mut flags, &def.id, "jump");
            }
            if frozen || jump {
                continue;
            }
            cands.push(Candidate {
                id: def.id.clone(),
                group: def.group.clone(),
                pos,
                sigma,
                is_dr: false,
            });
        }
        if let (Some(dr), Some(d)) = (self.dr, &ins_def) {
            cands.push(Candidate {
                id: d.id.clone(),
                group: d.group.clone(),
                pos: dr.pos,
                sigma: dr.sigma(t, d.sigma_mps),
                is_dr: true,
            });
        }
        // Consistency: mutually consistent subsets, by the groups they span.
        let n = cands.len();
        let agree = |a: &Candidate, b: &Candidate| {
            dist(a.pos, b.pos) <= k * (a.sigma * a.sigma + b.sigma * b.sigma).sqrt()
        };
        let mut best_groups = 0usize;
        let mut best: Vec<u32> = Vec::new();
        for mask in 1u32..(1u32 << n) {
            let idx: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
            if !idx
                .iter()
                .all(|&i| idx.iter().all(|&j| i >= j || agree(&cands[i], &cands[j])))
            {
                continue;
            }
            // Only sets with at least one position source count.
            if idx.iter().all(|&i| cands[i].is_dr) {
                continue;
            }
            let g: BTreeSet<&str> = idx.iter().map(|&i| cands[i].group.as_str()).collect();
            if g.len() > best_groups {
                best_groups = g.len();
                best = vec![mask];
            } else if g.len() == best_groups {
                best.push(mask);
            }
        }
        // Keep only maximal sets (not contained in another of the best).
        let maximal: Vec<u32> = best
            .iter()
            .copied()
            .filter(|&m| !best.iter().any(|&o| o != m && o & m == m))
            .collect();
        let mut integrity;
        let mut chosen: Option<usize> = None;
        let mut pl = None;
        let mut groups = 0;
        if cands.iter().all(|c| c.is_dr) {
            // Dead reckoning alone (or nothing at all).
            integrity = Integrity::None;
            if let Some(i) = cands.iter().position(|c| c.is_dr) {
                chosen = Some(i);
            }
        } else if maximal.len() == 1 {
            let m = maximal[0];
            groups = best_groups;
            let members: Vec<usize> = (0..n).filter(|i| m & (1 << i) != 0).collect();
            for (i, c) in cands.iter().enumerate() {
                if m & (1 << i) == 0 && best_groups >= 2 {
                    add(&mut flags, &c.id, "inconsistent");
                }
            }
            let sel = members
                .iter()
                .copied()
                .filter(|&i| !cands[i].is_dr)
                .min_by(|&a, &b| cands[a].sigma.total_cmp(&cands[b].sigma).then(a.cmp(&b)))
                .expect("a best set holds a position source");
            chosen = Some(sel);
            pl = members
                .iter()
                .filter(|&&r| cands[r].group != cands[sel].group)
                .map(|&r| dist(cands[sel].pos, cands[r].pos) + k * cands[r].sigma)
                .min_by(f64::total_cmp);
            integrity = match best_groups {
                0 | 1 => Integrity::Low,
                2 => Integrity::Medium,
                _ if pl.is_some_and(|p| p <= self.cfg.limits.alert_limit_m) => Integrity::High,
                _ => Integrity::Medium,
            };
        } else {
            // Unresolved: several sets span as many groups. Take the solution from what they
            // share, if anything.
            let common = maximal.iter().fold(u32::MAX, |a, &m| a & m);
            for (i, c) in cands.iter().enumerate() {
                if common & (1 << i) == 0 {
                    add(&mut flags, &c.id, "disagreement");
                }
            }
            let sel = (0..n)
                .filter(|&i| common & (1 << i) != 0 && !cands[i].is_dr)
                .min_by(|&a, &b| cands[a].sigma.total_cmp(&cands[b].sigma).then(a.cmp(&b)));
            match sel {
                Some(s) => {
                    integrity = Integrity::Low;
                    chosen = Some(s);
                    groups = 1;
                }
                None => {
                    integrity = Integrity::None;
                    chosen = (0..n)
                        .filter(|&i| !cands[i].is_dr)
                        .min_by(|&a, &b| cands[a].sigma.total_cmp(&cands[b].sigma).then(a.cmp(&b)));
                }
            }
        }
        if chosen.is_none() {
            integrity = Integrity::None;
        }
        // Dead reckoning: anchor on the first solution of medium integrity or better; re-anchor
        // only on a high-integrity solution it agrees with, once its sigma has grown.
        if let (Some(c), Some(_), Some(id)) = (chosen, vel, &ins_def) {
            let sel = &cands[c];
            if !sel.is_dr {
                let cur = self.dr.map(|d| (d.pos, d.sigma(t, id.sigma_mps)));
                match cur {
                    None if integrity >= Integrity::Medium => {
                        self.dr = Some(Dr {
                            pos: sel.pos,
                            sigma0: sel.sigma,
                            t0: t,
                        });
                    }
                    Some((p, s))
                        if integrity == Integrity::High
                            && s > self.cfg.limits.reanchor_sigma_m
                            && dist(p, sel.pos) <= k * (s * s + sel.sigma * sel.sigma).sqrt() =>
                    {
                        self.dr = Some(Dr {
                            pos: sel.pos,
                            sigma0: sel.sigma,
                            t0: t,
                        });
                    }
                    _ => {}
                }
            }
        }
        let status = match self.last_integrity {
            Some(prev) if integrity < prev => "raised",
            _ => "ok",
        };
        self.last_integrity = Some(integrity);
        self.last_t = Some(t);
        let local = self.local;
        let (lat, lon) = match (chosen, local) {
            (Some(c), Some(l)) => {
                let (la, lo) = l.latlon(cands[c].pos.0, cands[c].pos.1);
                (Some(la), Some(lo))
            }
            _ => (None, None),
        };
        Output {
            t_s: t,
            selected: chosen.map(|c| cands[c].id.clone()),
            lat_deg: lat,
            lon_deg: lon,
            sigma_m: chosen.map(|c| cands[c].sigma),
            protection_level_m: pl,
            integrity,
            groups,
            flags,
            status,
        }
    }
}

/// Run a whole input sequence (samples grouped by identical time).
pub fn run(cfg: &Config, samples: &[Sample]) -> Result<Vec<Output>, String> {
    let mut p = Processor::new(cfg.clone())?;
    let mut out = Vec::new();
    let mut i = 0;
    while i < samples.len() {
        let t = samples[i].t_s;
        let mut j = i;
        while j < samples.len() && samples[j].t_s == t {
            j += 1;
        }
        let ep: Vec<&Sample> = samples[i..j].iter().collect();
        out.push(p.step(t, &ep));
        i = j;
    }
    Ok(out)
}

/// The output CSV header.
pub const OUTPUT_HEADER: &str =
    "t_s,selected,lat_deg,lon_deg,sigma_m,protection_level_m,integrity,groups,status,flags";

pub fn outputs_csv(out: &[Output]) -> String {
    let o = |v: Option<f64>, dp: usize| v.map(|x| format!("{x:.dp$}")).unwrap_or_default();
    let mut s = format!("{OUTPUT_HEADER}\n");
    for x in out {
        s.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            x.t_s,
            x.selected.as_deref().unwrap_or(""),
            o(x.lat_deg, 9),
            o(x.lon_deg, 9),
            o(x.sigma_m, 3),
            o(x.protection_level_m, 3),
            x.integrity.as_str(),
            x.groups,
            x.status,
            x.flags_text()
        ));
    }
    s
}
