// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Multi-source trust (roadmap 0.42 F3)
//!
//! Choose, epoch by epoch (second by second on a live feed), which time source to trust among
//! GNSS, OSNMA-authenticated Galileo, a LEO PNT service, eLoran, PTP and the local clock's own
//! holdover, and keep the evidence for every choice.
//!
//! The rule is the one fixed in `docs/evidence/v4-multi-source-switching/2026-11-15_PREREGISTRATION.md`
//! (section 2) before this code was written:
//!
//! * **Exclusion.** A source whose status is not `ok` is out. An authenticating source (OSNMA)
//!   that reports `failed` takes its whole group out until it has reported `ok` for
//!   [`OSNMA_RECOVERY_S`].
//! * **Qualification.** Every other group votes through its best source (smallest stated sigma);
//!   the holdover votes while its sigma is at most [`HOLDOVER_VOTE_MAX_S`]. A vote agrees when the
//!   two offsets differ by at most `K sqrt(sigma_c^2 + sigma_r^2)`. A candidate qualifies with at
//!   least one agreeing vote and no more disagreeing than agreeing votes. Its protection level is
//!   `K sqrt(sigma_c^2 + sigma_ref^2)`, `sigma_ref` the tightest agreeing vote.
//! * **Holdover.** A line through the last [`WINDOW_S`] of external outputs that are at least
//!   [`GUARD_S`] old, with an uncertainty from the fit, the clock class's noise and its drift bound.
//! * **Choice.** The best-ranked qualified source (smallest sigma, then the configured order);
//!   a worse current choice is kept until a better one has qualified for `max(2 tau0, 10 s)`; a
//!   choice that stops qualifying is left at once.
//!
//! The evidence is a decisions table (one row per epoch: the choice, its offset and protection
//! level, and every source's verdict) and an evidence document whose switch records are
//! hash-chained and which carries the decisions table's SHA-256 ([`Evidence::verify`]); verifying
//! needs no licence.
//!
//! Honest scope: the rule decides from the sources' stated noise levels and their mutual
//! consistency; it cannot see a fault smaller than the consistency bound, and a fault common to
//! every source of every group is invisible to it. The V4 bars were scored on recorded clocks
//! with modelled source errors and injected faults (see `RESULTS.md` beside the
//! pre-registration); no real multi-source recording has been scored.

pub mod v4;

use kshana::api::{ExternalPack, KshanaError, RunOutput, Scenario, ScenarioMeta};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The scenario kind (and the licence feature of `kshana-pro trust run`).
pub const KIND: &str = "pro-multi-source-trust";
/// The method tag written into every evidence document.
pub const METHOD: &str = "kshana-pro/multi-source/v1";
/// The evidence document's format tag.
pub const EVIDENCE_FORMAT: &str = "kshana-pro-trust-evidence/1";

/// Consistency and protection multiplier `K`.
pub const K: f64 = 4.0;
/// Holdover fit window, s.
pub const WINDOW_S: f64 = 3600.0;
/// Holdover guard: outputs younger than this are left out of the fit, s.
pub const GUARD_S: f64 = 600.0;
/// Largest holdover sigma that votes, s.
pub const HOLDOVER_VOTE_MAX_S: f64 = 200e-9;
/// Time an authenticating source must report `ok` before its group is used again, s.
pub const OSNMA_RECOVERY_S: f64 = 60.0;

/// A source's status at one epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Unavailable,
    /// Authentication failed (an authenticating source only).
    Failed,
}

impl Status {
    pub fn parse(s: &str) -> Option<Status> {
        match s.trim() {
            "ok" => Some(Status::Ok),
            "unavailable" | "" => Some(Status::Unavailable),
            "failed" => Some(Status::Failed),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Unavailable => "unavailable",
            Status::Failed => "failed",
        }
    }
}

/// One configured external source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDef {
    /// Short identifier (`osnma`, `gnss`, `leo`, `ptp`, `eloran`, ...): letters, digits, `-`, `_`.
    pub id: String,
    /// Sources of one group share a signal path and never vote for each other.
    pub group: String,
    /// The source's stated one-sigma noise, ns.
    pub sigma_ns: f64,
    /// An authenticating source (OSNMA): `failed` takes its whole group out.
    #[serde(default)]
    pub authenticates_group: bool,
}

/// The selector's configuration: the sources (their order breaks ranking ties) and the local
/// clock's class (its holdover noise and drift bound).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(rename = "source")]
    pub sources: Vec<SourceDef>,
    /// The local clock's class: `tcxo`, `ocxo`, `rafs`, `csac`, `uso` or `dsac`.
    pub clock_class: String,
    /// The epoch interval, s.
    pub tau0_s: f64,
}

/// The drift bound of a clock class, 1/s (pre-registration section 2.3).
pub fn class_drift_bound(class: &str) -> Option<f64> {
    Some(match class.to_ascii_lowercase().as_str() {
        "tcxo" => 1e-14,
        "ocxo" => 1e-15,
        "csac" => 1e-15,
        "rafs" | "rubidium" | "rb" => 1e-16,
        "uso" => 1e-15,
        "dsac" => 1e-17,
        _ => return None,
    })
}

impl Config {
    /// The configuration the V4 bars were pre-registered with, for a clock class and interval.
    pub fn v4(clock_class: &str, tau0_s: f64) -> Config {
        let s = |id: &str, group: &str, sigma_ns: f64, auth: bool| SourceDef {
            id: id.into(),
            group: group.into(),
            sigma_ns,
            authenticates_group: auth,
        };
        Config {
            sources: vec![
                s("osnma", "gnss", 15.0, true),
                s("gnss", "gnss", 15.0, false),
                s("leo", "leo", 40.0, false),
                s("ptp", "ptp", 50.0, false),
                s("eloran", "eloran", 60.0, false),
            ],
            clock_class: clock_class.into(),
            tau0_s,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.sources.is_empty() || self.sources.len() > 32 {
            return Err("between 1 and 32 sources are configured".into());
        }
        let ok_id = |s: &str| {
            !s.is_empty()
                && s.len() <= 32
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        };
        for (i, s) in self.sources.iter().enumerate() {
            if !ok_id(&s.id) || !ok_id(&s.group) {
                return Err(format!(
                    "source {}: id and group are 1 to 32 letters, digits, '-' or '_'",
                    i + 1
                ));
            }
            if s.id == "holdover" || s.id == "none" || s.group == "holdover" {
                return Err("\"holdover\" and \"none\" are reserved".into());
            }
            if !(s.sigma_ns.is_finite() && s.sigma_ns > 0.0) {
                return Err(format!("source {}: sigma_ns must be positive", s.id));
            }
            if self.sources[..i].iter().any(|o| o.id == s.id) {
                return Err(format!("source {} is listed twice", s.id));
            }
        }
        if crate::timing::model::class_noise(&self.clock_class).is_none() {
            return Err(format!(
                "clock_class {:?} is not one of {}",
                self.clock_class,
                crate::timing::model::CLASS_NAMES.join(", ")
            ));
        }
        if !(self.tau0_s.is_finite() && self.tau0_s > 0.0) {
            return Err("tau0_s must be positive".into());
        }
        Ok(())
    }

    /// The hold time before switching to a better source, s.
    pub fn t_up(&self) -> f64 {
        (2.0 * self.tau0_s).max(10.0)
    }
}

/// One source's reading at one epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obs {
    pub status: Status,
    /// The measured offset of the local clock against the source, s (ignored unless `ok`).
    pub offset_s: f64,
}

/// What the selector output at one epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choice {
    /// An external source, by index into [`Config::sources`].
    Source(usize),
    Holdover,
    None,
}

/// One source's verdict at one epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Unavailable,
    /// Authentication failed.
    AuthFailed,
    /// Its group is out after an authentication failure, until the authenticating source recovers.
    AuthRecovering,
    Qualified {
        agree: u8,
        disagree: u8,
    },
    NotQualified {
        agree: u8,
        disagree: u8,
    },
}

impl Verdict {
    pub fn code(&self) -> String {
        match self {
            Verdict::Unavailable => "X:unavailable".into(),
            Verdict::AuthFailed => "X:auth-failed".into(),
            Verdict::AuthRecovering => "X:auth-recovering".into(),
            Verdict::Qualified { agree, disagree } => format!("Q{agree}/{disagree}"),
            Verdict::NotQualified { agree, disagree } => format!("N{agree}/{disagree}"),
        }
    }
    pub fn qualified(&self) -> bool {
        matches!(self, Verdict::Qualified { .. })
    }
}

/// The selector's decision at one epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub t_s: f64,
    pub choice: Choice,
    /// The output offset, s (`None` with no output).
    pub offset_s: Option<f64>,
    /// The output's protection level, s.
    pub pl_s: Option<f64>,
    /// Every external source's verdict, in configured order.
    pub verdicts: Vec<Verdict>,
    /// The holdover's sigma at this epoch, s (`None` while not anchored).
    pub holdover_sigma_s: Option<f64>,
    /// Why the choice changed at this epoch (empty when it did not).
    pub reason: &'static str,
}

impl Decision {
    pub fn state(&self) -> &'static str {
        match self.choice {
            Choice::Source(_) => "trusted",
            Choice::Holdover => "holdover",
            Choice::None => "none",
        }
    }
}

// ------------------------------------------------------------------ holdover

/// The holdover fit of pre-registration section 2.3, over a sliding window kept as running sums.
#[derive(Clone, Debug)]
pub struct Holdover {
    q_wf: f64,
    q_rw: f64,
    drift: f64,
    pts: Vec<(f64, f64, f64)>,
    lo: usize,
    hi: usize,
    origin: (f64, f64),
    sums: [f64; 6],
    since_rebase: usize,
}

/// A holdover estimate at one epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoldoverEstimate {
    pub offset_s: f64,
    pub sigma_s: f64,
}

impl Holdover {
    pub fn new(class: &str) -> Option<Holdover> {
        let n = crate::timing::model::class_noise(class)?;
        Some(Holdover {
            q_wf: n.q_wf,
            q_rw: n.q_rw,
            drift: class_drift_bound(class)?,
            pts: Vec::new(),
            lo: 0,
            hi: 0,
            origin: (0.0, 0.0),
            sums: [0.0; 6],
            since_rebase: 0,
        })
    }

    /// Record an external output (time, offset, its stated variance).
    pub fn push(&mut self, t: f64, y: f64, var: f64) {
        self.pts.push((t, y, var));
    }

    fn add(&mut self, i: usize, sign: f64) {
        let (t, y, v) = self.pts[i];
        let (t, y) = (t - self.origin.0, y - self.origin.1);
        self.sums[0] += sign;
        self.sums[1] += sign * t;
        self.sums[2] += sign * t * t;
        self.sums[3] += sign * y;
        self.sums[4] += sign * t * y;
        self.sums[5] += sign * v;
        self.since_rebase += 1;
    }

    fn rebase(&mut self) {
        self.sums = [0.0; 6];
        if self.lo < self.hi {
            self.origin = (self.pts[self.lo].0, self.pts[self.lo].1);
        }
        for i in self.lo..self.hi {
            self.add(i, 1.0);
        }
        self.since_rebase = 0;
    }

    /// The estimate at `now`, or `None` while not anchored (fewer than 3 epochs in the window).
    pub fn estimate(&mut self, now: f64) -> Option<HoldoverEstimate> {
        let mut moved = false;
        while self.hi < self.pts.len() && self.pts[self.hi].0 <= now - GUARD_S {
            if self.lo == self.hi {
                // An empty window starts afresh at this point.
                self.origin = (self.pts[self.hi].0, self.pts[self.hi].1);
                self.sums = [0.0; 6];
            }
            self.add(self.hi, 1.0);
            self.hi += 1;
            moved = true;
        }
        if self.hi == 0 {
            return None;
        }
        let t_e = self.pts[self.hi - 1].0;
        while self.lo < self.hi && self.pts[self.lo].0 < t_e - WINDOW_S {
            self.add(self.lo, -1.0);
            self.lo += 1;
            moved = true;
        }
        if moved && self.since_rebase >= 4096 {
            self.rebase();
        }
        let [_, st, stt, sy, sty, sv] = self.sums;
        let n_pts = self.hi - self.lo;
        if n_pts < 3 {
            return None;
        }
        let n = n_pts as f64;
        let tbar = st / n;
        let sxx = stt - st * st / n;
        if sxx <= 0.0 {
            return None;
        }
        let b = (sty - st * sy / n) / sxx;
        let te = t_e - self.origin.0;
        let a = sy / n + b * (te - tbar);
        let v = (sv / n).max(0.0);
        let var_a = v * (1.0 / n + (te - tbar).powi(2) / sxx);
        let var_b = v / sxx;
        let u = now - t_e;
        let drift = self.drift * (u + WINDOW_S / 2.0).powi(2) / 2.0;
        let var =
            var_a + var_b * u * u + self.q_wf * u + self.q_rw * u.powi(3) / 3.0 + drift * drift;
        Some(HoldoverEstimate {
            offset_s: self.origin.1 + a + b * u,
            sigma_s: var.max(0.0).sqrt(),
        })
    }
}

// ------------------------------------------------------------------ the selector

/// The multi-source trust selector: feed it one epoch at a time ([`Selector::step`]).
#[derive(Clone, Debug)]
pub struct Selector {
    cfg: Config,
    sigma: Vec<f64>,
    group_of: Vec<usize>,
    n_groups: usize,
    current: Choice,
    qualified_since: Vec<Option<f64>>,
    /// Per group: blocked by an authentication failure.
    blocked: Vec<bool>,
    /// Per source: the time since which it has reported `ok` (authenticating sources).
    ok_since: Vec<Option<f64>>,
    holdover: Holdover,
}

impl Selector {
    pub fn new(cfg: Config) -> Result<Selector, String> {
        cfg.validate()?;
        let mut groups: Vec<String> = Vec::new();
        let group_of = cfg
            .sources
            .iter()
            .map(|s| match groups.iter().position(|g| *g == s.group) {
                Some(i) => i,
                None => {
                    groups.push(s.group.clone());
                    groups.len() - 1
                }
            })
            .collect();
        let n = cfg.sources.len();
        Ok(Selector {
            sigma: cfg.sources.iter().map(|s| s.sigma_ns * 1e-9).collect(),
            group_of,
            n_groups: groups.len(),
            current: Choice::None,
            qualified_since: vec![None; n],
            blocked: vec![false; groups.len()],
            ok_since: vec![None; n],
            holdover: Holdover::new(&cfg.clock_class).ok_or("unknown clock class")?,
            cfg,
        })
    }

    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Ranking key: smaller is better (sigma, then configured order).
    fn better(&self, a: usize, b: usize) -> bool {
        (self.sigma[a], a) < (self.sigma[b], b)
    }

    /// Decide one epoch. `obs` has one entry per configured source, in order.
    #[allow(clippy::needless_range_loop)]
    pub fn step(&mut self, t: f64, obs: &[Obs]) -> Decision {
        let n = self.cfg.sources.len();
        assert_eq!(obs.len(), n, "one reading per configured source");
        // 2.1: authentication state.
        for i in 0..n {
            if !self.cfg.sources[i].authenticates_group {
                continue;
            }
            let g = self.group_of[i];
            match obs[i].status {
                Status::Failed => {
                    self.blocked[g] = true;
                    self.ok_since[i] = None;
                }
                Status::Ok => {
                    let since = *self.ok_since[i].get_or_insert(t);
                    if self.blocked[g] && t - since >= OSNMA_RECOVERY_S {
                        self.blocked[g] = false;
                    }
                }
                Status::Unavailable => self.ok_since[i] = None,
            }
        }
        let usable: Vec<bool> = (0..n)
            .map(|i| obs[i].status == Status::Ok && !self.blocked[self.group_of[i]])
            .collect();
        // Representatives of each group.
        let mut rep: Vec<Option<usize>> = vec![None; self.n_groups];
        for i in 0..n {
            if usable[i] {
                let g = self.group_of[i];
                if rep[g].is_none_or(|r| self.better(i, r)) {
                    rep[g] = Some(i);
                }
            }
        }
        let ho = self.holdover.estimate(t);
        let ho_votes = ho.filter(|h| h.sigma_s <= HOLDOVER_VOTE_MAX_S);
        // 2.2: qualification.
        let mut verdicts = Vec::with_capacity(n);
        let mut pl = vec![f64::INFINITY; n];
        for c in 0..n {
            let v = if !usable[c] {
                match obs[c].status {
                    Status::Unavailable => Verdict::Unavailable,
                    Status::Failed => Verdict::AuthFailed,
                    Status::Ok => Verdict::AuthRecovering,
                }
            } else {
                let (mut agree, mut disagree) = (0u8, 0u8);
                let mut sref = f64::INFINITY;
                let mut vote = |o: f64, s: f64| {
                    let sc = self.sigma[c];
                    if (obs[c].offset_s - o).abs() <= K * (sc * sc + s * s).sqrt() {
                        agree += 1;
                        sref = sref.min(s);
                    } else {
                        disagree += 1;
                    }
                };
                for (g, r) in rep.iter().enumerate() {
                    if g == self.group_of[c] {
                        continue;
                    }
                    if let Some(r) = r {
                        vote(obs[*r].offset_s, self.sigma[*r]);
                    }
                }
                if let Some(h) = ho_votes {
                    vote(h.offset_s, h.sigma_s);
                }
                if agree >= 1 && agree >= disagree {
                    let sc = self.sigma[c];
                    pl[c] = K * (sc * sc + sref * sref).sqrt();
                    Verdict::Qualified { agree, disagree }
                } else {
                    Verdict::NotQualified { agree, disagree }
                }
            };
            if v.qualified() {
                self.qualified_since[c].get_or_insert(t);
            } else {
                self.qualified_since[c] = None;
            }
            verdicts.push(v);
        }
        // 2.4: choice.
        let best = (0..n)
            .filter(|&i| verdicts[i].qualified())
            .min_by(|&a, &b| (self.sigma[a], a).partial_cmp(&(self.sigma[b], b)).unwrap());
        let t_up = self.cfg.t_up();
        let (next, reason) = match self.current {
            Choice::Source(c) if verdicts[c].qualified() => {
                // A better-ranked candidate that has qualified long enough.
                let up = (0..n)
                    .filter(|&i| {
                        self.better(i, c) && self.qualified_since[i].is_some_and(|s| t - s >= t_up)
                    })
                    .min_by(|&a, &b| (self.sigma[a], a).partial_cmp(&(self.sigma[b], b)).unwrap());
                match up {
                    Some(i) => (Choice::Source(i), "better source held"),
                    None => (Choice::Source(c), ""),
                }
            }
            Choice::Source(c) => match best {
                Some(i) => (
                    Choice::Source(i),
                    if usable[c] {
                        "current source failed the cross-check"
                    } else {
                        "current source excluded"
                    },
                ),
                None if ho.is_some() => (Choice::Holdover, "no source qualifies"),
                None => (Choice::None, "no source qualifies; no holdover"),
            },
            Choice::Holdover | Choice::None => match best {
                Some(i) => (Choice::Source(i), "a source qualifies"),
                None if ho.is_some() => (Choice::Holdover, ""),
                None => (Choice::None, ""),
            },
        };
        let reason = if next == self.current { "" } else { reason };
        self.current = next;
        let (offset_s, pl_s) = match next {
            Choice::Source(i) => {
                self.holdover
                    .push(t, obs[i].offset_s, self.sigma[i] * self.sigma[i]);
                (Some(obs[i].offset_s), Some(pl[i]))
            }
            Choice::Holdover => {
                let h = ho.expect("holdover is chosen only when anchored");
                (Some(h.offset_s), Some(K * h.sigma_s))
            }
            Choice::None => (None, None),
        };
        Decision {
            t_s: t,
            choice: next,
            offset_s,
            pl_s,
            verdicts,
            holdover_sigma_s: ho.map(|h| h.sigma_s),
            reason,
        }
    }
}

// ------------------------------------------------------------------ input table

/// A recorded or live source table: `t_s,<id>_status,<id>_offset_ns,...` for every configured
/// source (columns in any order; unknown columns are refused).
pub fn parse_table(cfg: &Config, text: &str) -> Result<Vec<(f64, Vec<Obs>)>, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'));
    let (_, header) = lines.next().ok_or("the source table is empty")?;
    let cols: Vec<&str> = header.split(',').map(str::trim).collect();
    if cols.first() != Some(&"t_s") {
        return Err("the first column is t_s".into());
    }
    let mut idx = Vec::new();
    for s in &cfg.sources {
        let st = cols
            .iter()
            .position(|c| *c == format!("{}_status", s.id))
            .ok_or(format!("no column {}_status", s.id))?;
        let of = cols
            .iter()
            .position(|c| *c == format!("{}_offset_ns", s.id))
            .ok_or(format!("no column {}_offset_ns", s.id))?;
        idx.push((st, of));
    }
    if cols.len() != 1 + 2 * cfg.sources.len() {
        return Err("the table has columns for sources that are not configured".into());
    }
    let mut out: Vec<(f64, Vec<Obs>)> = Vec::new();
    for (ln, line) in lines {
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        if f.len() != cols.len() {
            return Err(format!(
                "line {}: {} fields, expected {}",
                ln + 1,
                f.len(),
                cols.len()
            ));
        }
        let t: f64 = f[0]
            .parse()
            .map_err(|_| format!("line {}: t_s is not a number", ln + 1))?;
        if !t.is_finite() || out.last().is_some_and(|p| p.0 >= t) {
            return Err(format!("line {}: t_s must increase", ln + 1));
        }
        let mut obs = Vec::with_capacity(idx.len());
        for (k, &(st, of)) in idx.iter().enumerate() {
            let status = Status::parse(f[st]).ok_or(format!(
                "line {}: status {:?} (ok, unavailable, failed)",
                ln + 1,
                f[st]
            ))?;
            if status == Status::Failed && !cfg.sources[k].authenticates_group {
                return Err(format!(
                    "line {}: only an authenticating source reports failed",
                    ln + 1
                ));
            }
            let offset_s = if status == Status::Ok {
                let v: f64 = f[of].parse().map_err(|_| {
                    format!(
                        "line {}: {} offset is not a number",
                        ln + 1,
                        cfg.sources[k].id
                    )
                })?;
                if !v.is_finite() {
                    return Err(format!("line {}: offset not finite", ln + 1));
                }
                v * 1e-9
            } else {
                0.0
            };
            obs.push(Obs { status, offset_s });
        }
        out.push((t, obs));
    }
    if out.is_empty() {
        return Err("the source table has no rows".into());
    }
    Ok(out)
}

// ------------------------------------------------------------------ evidence

fn fmt_ns(v: Option<f64>) -> String {
    v.map(|x| format!("{:.3}", x * 1e9)).unwrap_or_default()
}

/// The choice's name.
pub fn choice_name(cfg: &Config, c: Choice) -> &str {
    match c {
        Choice::Source(i) => &cfg.sources[i].id,
        Choice::Holdover => "holdover",
        Choice::None => "none",
    }
}

/// The decisions table header.
pub fn decisions_header(cfg: &Config) -> String {
    let mut h = "t_s,choice,state,offset_ns,pl_ns,holdover_sigma_ns".to_string();
    for s in &cfg.sources {
        h.push(',');
        h.push_str(&s.id);
    }
    h.push_str(",reason\n");
    h
}

/// One decisions table row.
pub fn decision_row(cfg: &Config, d: &Decision) -> String {
    let mut r = format!(
        "{},{},{},{},{},{}",
        d.t_s,
        choice_name(cfg, d.choice),
        d.state(),
        fmt_ns(d.offset_s),
        fmt_ns(d.pl_s),
        fmt_ns(d.holdover_sigma_s)
    );
    for v in &d.verdicts {
        r.push(',');
        r.push_str(&v.code());
    }
    r.push(',');
    r.push_str(d.reason);
    r.push('\n');
    r
}

/// One switch record of the evidence chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SwitchRecord {
    pub index: usize,
    pub t_s: f64,
    pub from: String,
    pub to: String,
    pub reason: String,
    /// SHA-256 of the previous record's `hash` (64 zeros for the first).
    pub prev: String,
    /// SHA-256 over `index|t_s|from|to|reason|prev`.
    pub hash: String,
}

impl SwitchRecord {
    fn digest(index: usize, t_s: f64, from: &str, to: &str, reason: &str, prev: &str) -> String {
        let mut h = Sha256::new();
        h.update(format!("{index}|{t_s}|{from}|{to}|{reason}|{prev}").as_bytes());
        hex::encode(h.finalize())
    }
}

/// The evidence document of one selector run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub format: String,
    pub method: String,
    pub config: Config,
    /// SHA-256 of the input table (empty when the input was not a file).
    pub input_sha256: String,
    /// SHA-256 of the decisions table.
    pub decisions_sha256: String,
    pub epochs: usize,
    pub counts: Counts,
    pub switches: Vec<SwitchRecord>,
    pub label: String,
}

/// Epoch counts by state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    pub trusted: usize,
    pub holdover: usize,
    pub none: usize,
    /// Epochs per chosen source, in configured order.
    pub by_source: Vec<(String, usize)>,
}

/// The honesty label of every evidence document.
pub const LABEL: &str =
    "MODELLED: each choice follows the pre-registered rule (V4) from the sources' stated \
noise levels and their mutual consistency; it cannot see a fault smaller than the consistency \
bound or one common to every group. Evidence of the selector's decisions, not of any source's \
accuracy.";

/// A run's decisions table and evidence document.
#[derive(Clone, Debug)]
pub struct RunEvidence {
    pub decisions_csv: String,
    pub evidence: Evidence,
}

/// Run the selector over a table and build the evidence.
pub fn run_table(
    cfg: &Config,
    rows: &[(f64, Vec<Obs>)],
    input_sha256: &str,
) -> Result<RunEvidence, String> {
    let mut sel = Selector::new(cfg.clone())?;
    let mut csv = decisions_header(cfg);
    let mut switches = Vec::new();
    let mut counts = Counts {
        by_source: cfg.sources.iter().map(|s| (s.id.clone(), 0)).collect(),
        ..Counts::default()
    };
    let mut prev_choice = Choice::None;
    for (k, (t, obs)) in rows.iter().enumerate() {
        let d = sel.step(*t, obs);
        csv.push_str(&decision_row(cfg, &d));
        match d.choice {
            Choice::Source(i) => {
                counts.trusted += 1;
                counts.by_source[i].1 += 1;
            }
            Choice::Holdover => counts.holdover += 1,
            Choice::None => counts.none += 1,
        }
        if k > 0 && d.choice != prev_choice || k == 0 && d.choice != Choice::None {
            let prev = switches
                .last()
                .map(|s: &SwitchRecord| s.hash.clone())
                .unwrap_or_else(|| "0".repeat(64));
            let (from, to) = (choice_name(cfg, prev_choice), choice_name(cfg, d.choice));
            let reason = if d.reason.is_empty() {
                "start"
            } else {
                d.reason
            };
            let index = switches.len();
            switches.push(SwitchRecord {
                index,
                t_s: *t,
                from: from.into(),
                to: to.into(),
                reason: reason.into(),
                hash: SwitchRecord::digest(index, *t, from, to, reason, &prev),
                prev,
            });
        }
        prev_choice = d.choice;
    }
    let decisions_sha256 = hex::encode(Sha256::digest(csv.as_bytes()));
    Ok(RunEvidence {
        evidence: Evidence {
            kind: KIND.into(),
            format: EVIDENCE_FORMAT.into(),
            method: METHOD.into(),
            config: cfg.clone(),
            input_sha256: input_sha256.into(),
            decisions_sha256,
            epochs: rows.len(),
            counts,
            switches,
            label: LABEL.into(),
        },
        decisions_csv: csv,
    })
}

impl Evidence {
    /// Check the evidence against a decisions table: the table's hash, one row per epoch, the
    /// switch chain, and that the switches are exactly the table's changes of choice. Returns the
    /// problems found (empty: verified).
    pub fn verify(&self, decisions_csv: &str) -> Vec<String> {
        let mut p = Vec::new();
        if self.format != EVIDENCE_FORMAT {
            p.push(format!("format {:?} is not {EVIDENCE_FORMAT}", self.format));
        }
        let h = hex::encode(Sha256::digest(decisions_csv.as_bytes()));
        if h != self.decisions_sha256 {
            p.push("the decisions table does not match its recorded SHA-256".into());
        }
        let rows: Vec<&str> = decisions_csv.lines().skip(1).collect();
        if rows.len() != self.epochs {
            p.push(format!("{} rows for {} epochs", rows.len(), self.epochs));
        }
        let mut prev = "0".repeat(64);
        for (i, s) in self.switches.iter().enumerate() {
            if s.index != i || s.prev != prev {
                p.push(format!("switch {i}: the chain is broken"));
            }
            if SwitchRecord::digest(s.index, s.t_s, &s.from, &s.to, &s.reason, &s.prev) != s.hash {
                p.push(format!("switch {i}: the record does not match its hash"));
            }
            prev = s.hash.clone();
        }
        // The switches are the table's changes of choice.
        let mut last = "none".to_string();
        let mut derived = Vec::new();
        for r in &rows {
            let mut f = r.split(',');
            let (t, c) = (f.next().unwrap_or(""), f.next().unwrap_or(""));
            if c != last {
                derived.push((t.to_string(), last.clone(), c.to_string()));
                last = c.to_string();
            }
        }
        let recorded: Vec<(String, String, String)> = self
            .switches
            .iter()
            .map(|s| (s.t_s.to_string(), s.from.clone(), s.to.clone()))
            .collect();
        if derived != recorded {
            p.push("the switch records are not the decisions table's changes of choice".into());
        }
        p
    }
}

// ------------------------------------------------------------------ scenario kind

/// A `pro-multi-source-trust` scenario: the configuration and the path of a source table.
#[derive(Clone, Debug, Deserialize)]
pub struct MultiSourceScenario {
    pub kind: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Path of the source table (relative to the working directory).
    pub input: String,
    #[serde(flatten)]
    pub config: Config,
}

impl MultiSourceScenario {
    pub const KIND: &'static str = KIND;

    pub fn meta_static() -> ScenarioMeta {
        ScenarioMeta {
            name: KIND,
            description: "Multi-source trust: choose epoch by epoch which time source to trust (GNSS, OSNMA, LEO, eLoran, PTP, holdover) under the pre-registered V4 rule, with a hash-chained evidence record of every choice.",
            required_fields: &["input", "source", "clock_class", "tau0_s"],
            optional_fields: &["title"],
        }
    }
}

impl Scenario for MultiSourceScenario {
    fn run(&self) -> Result<RunOutput, KshanaError> {
        self.config.validate().map_err(KshanaError::InvalidInput)?;
        let text = std::fs::read_to_string(&self.input)
            .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", self.input)))?;
        let rows = parse_table(&self.config, &text).map_err(KshanaError::InvalidInput)?;
        let r = run_table(
            &self.config,
            &rows,
            &hex::encode(Sha256::digest(text.as_bytes())),
        )
        .map_err(KshanaError::InvalidInput)?;
        let c = &r.evidence.counts;
        let summary = format!(
            "Kshana Pro multi-source trust | {} epochs: {} trusted, {} holdover, {} none | {} switches | method {METHOD}",
            r.evidence.epochs,
            c.trusted,
            c.holdover,
            c.none,
            r.evidence.switches.len()
        );
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"480\" height=\"72\" font-family=\"system-ui,sans-serif\"><rect width=\"480\" height=\"72\" fill=\"#0f1216\"/><text x=\"16\" y=\"30\" fill=\"#8cc4f5\" font-size=\"15\" font-weight=\"bold\">Multi-source trust · {} epochs</text><text x=\"16\" y=\"54\" fill=\"#9aa3af\" font-size=\"12\">{} trusted · {} holdover · {} none · {} switches</text></svg>",
            r.evidence.epochs,
            c.trusted,
            c.holdover,
            c.none,
            r.evidence.switches.len()
        );
        Ok(RunOutput {
            json: serde_json::to_string(&r.evidence).expect("the evidence is plain data"),
            svg,
            summary,
            csv: Some(r.decisions_csv),
        })
    }
}

impl ExternalPack for MultiSourceScenario {
    fn kind_name(&self) -> &'static str {
        KIND
    }
    fn meta(&self) -> ScenarioMeta {
        Self::meta_static()
    }
    fn register_into(reg: &mut kshana::registry::PackRegistry) {
        reg.register(crate::registry::pro_factory::<Self>(KIND));
    }
}
