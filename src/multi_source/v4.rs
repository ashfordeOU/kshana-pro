// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The V4 scoring of `docs/evidence/v4-multi-source-switching/2026-11-15_PREREGISTRATION.md`:
//! the data sets (recorded IGS clocks D-R1 and D-R2, synthetic D-S), the modelled source
//! measurements and injected faults (section 3), the oracle and GNSS-only baselines, and the
//! bars B1 to B6 (section 4). `tests/multi_source_v4.rs` runs it and pins the numbers.
//!
//! Reading of section 3 where the text leaves a choice: the noise is drawn epoch by epoch, the
//! five sources in their order within each epoch; a fault acts on the epochs `t0 <= t < t0 + d`
//! (epoch times `k tau0` from the run's first epoch); "unavailable" overrides "failed"; under S1
//! `osnma` keeps its unramped measurement (the table names only its status).

use super::{Choice, Config, Holdover, Obs, Selector, Status};
use crate::timing::model::{class_noise, synth_clock, SplitMix};

/// One local clock run.
#[derive(Clone, Debug)]
pub struct Run {
    pub id: String,
    pub class: String,
    pub tau0: f64,
    /// True offset `x(t)` at each epoch, s.
    pub x: Vec<f64>,
}

/// Parse an IGS clock fixture (the V5 layout) into runs, split at gaps; runs shorter than one
/// day are dropped. Returns the runs and the number dropped.
pub fn igs_runs(text: &str) -> (Vec<Run>, usize) {
    let tau0 = 300.0;
    let mut runs = Vec::new();
    let mut dropped = 0;
    let mut flush = |id: &str, seg: &mut Vec<f64>, part: &mut usize| {
        if seg.is_empty() {
            return;
        }
        if (seg.len() as f64) * tau0 >= 86_400.0 {
            let name = if *part == 0 {
                id.to_string()
            } else {
                format!("{id}.{part}")
            };
            runs.push(Run {
                id: name,
                class: "rafs".into(),
                tau0,
                x: std::mem::take(seg),
            });
        } else {
            dropped += 1;
            seg.clear();
        }
        *part += 1;
    };
    let mut id = String::new();
    let mut seg: Vec<f64> = Vec::new();
    let mut part = 0usize;
    let mut last_k: Option<i64> = None;
    let mut acc: i64 = 0;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(h) = line.strip_prefix('@') {
            flush(&id, &mut seg, &mut part);
            id = h.split_whitespace().next().unwrap_or("").to_string();
            part = 0;
            last_k = None;
            acc = 0;
            continue;
        }
        let mut f = line.split_whitespace();
        let (Some(k), Some(inc)) = (f.next(), f.next()) else {
            continue;
        };
        let (Ok(k), Ok(inc)) = (k.parse::<i64>(), inc.parse::<i64>()) else {
            continue;
        };
        acc += inc;
        if let Some(l) = last_k {
            if k != l + 1 {
                flush(&id, &mut seg, &mut part);
            }
        }
        seg.push(acc as f64 * 1e-13);
        last_k = Some(k);
    }
    flush(&id, &mut seg, &mut part);
    (runs, dropped)
}

/// The D-S clocks: ten one-day clocks at 1 s, `ocxo` and `rafs` alternating (seed 20261117).
pub fn synthetic_runs() -> Vec<Run> {
    let mut rng = SplitMix::new(20_261_117);
    (0..10)
        .map(|i| {
            let class = if i % 2 == 0 { "ocxo" } else { "rafs" };
            let n = class_noise(class).expect("known class");
            let drift = rng.range(-1e-15, 1e-15);
            let freq0 = rng.range(-1e-10, 1e-10);
            let x = synth_clock(&mut rng, 86_400, 1.0, n.q_wf, n.q_rw, drift, freq0, 0.0);
            Run {
                id: format!("S{:02}-{class}", i + 1),
                class: class.into(),
                tau0: 1.0,
                x,
            }
        })
        .collect()
}

/// A fault type of the pre-registration's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultType {
    S1,
    S2,
    J,
    P,
    L,
    E,
    A,
}

impl FaultType {
    pub const ALL: [FaultType; 7] = [
        FaultType::S1,
        FaultType::S2,
        FaultType::J,
        FaultType::P,
        FaultType::L,
        FaultType::E,
        FaultType::A,
    ];
    fn rate(self) -> f64 {
        match self {
            FaultType::J => 2.0,
            FaultType::A => 0.5,
            _ => 1.0,
        }
    }
    fn duration(self) -> f64 {
        match self {
            FaultType::S1 | FaultType::S2 | FaultType::A => 1800.0,
            FaultType::J => 1200.0,
            _ => 3600.0,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            FaultType::S1 => "S1",
            FaultType::S2 => "S2",
            FaultType::J => "J",
            FaultType::P => "P",
            FaultType::L => "L",
            FaultType::E => "E",
            FaultType::A => "A",
        }
    }
}

/// One injected fault.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fault {
    pub kind: FaultType,
    pub t0: f64,
    pub dur: f64,
}

impl Fault {
    fn active(&self, t: f64) -> bool {
        t >= self.t0 && t < self.t0 + self.dur
    }
}

// Source indices in the V4 configuration.
const OSNMA: usize = 0;
const GNSS: usize = 1;
const LEO: usize = 2;
const PTP: usize = 3;
const ELORAN: usize = 4;
const N_SRC: usize = 5;

/// The generated measurements of one run.
#[derive(Clone, Debug)]
pub struct Generated {
    pub obs: Vec<[Obs; N_SRC]>,
    /// The fault error `f_s` per epoch and source, s.
    pub fault_err: Vec<[f64; N_SRC]>,
    /// The truth label: faulty per epoch and source.
    pub faulty: Vec<[bool; N_SRC]>,
    pub faults: Vec<Fault>,
}

/// Generate the sources of a run (section 3, "Source noise and faults").
#[allow(clippy::needless_range_loop)]
pub fn generate(run: &Run, cfg: &Config, seed: u64) -> Generated {
    let mut rng = SplitMix::new(seed);
    let n = run.x.len();
    let sig: Vec<f64> = cfg.sources.iter().map(|s| s.sigma_ns * 1e-9).collect();
    let mut noise = vec![[0.0; N_SRC]; n];
    for row in noise.iter_mut() {
        for (s, v) in row.iter_mut().enumerate() {
            *v = sig[s] * rng.normal();
        }
    }
    let leo_phase = rng.range(0.0, 1800.0);
    let t_total = n as f64 * run.tau0;
    let days = t_total / 86_400.0;
    let mut faults = Vec::new();
    for ft in FaultType::ALL {
        let u = rng.uniform();
        let count = (ft.rate() * days + u).floor() as usize;
        for _ in 0..count {
            let t0 = rng.range(0.0, (t_total - ft.duration()).max(0.0));
            faults.push(Fault {
                kind: ft,
                t0,
                dur: ft.duration(),
            });
        }
    }
    let mut obs = Vec::with_capacity(n);
    let mut fault_err = Vec::with_capacity(n);
    let mut faulty = Vec::with_capacity(n);
    for k in 0..n {
        let t = k as f64 * run.tau0;
        let mut st = [Status::Ok; N_SRC];
        let mut f = [0.0; N_SRC];
        let mut lab = [false; N_SRC];
        if (t - leo_phase).rem_euclid(1800.0) >= 600.0 {
            st[LEO] = Status::Unavailable;
        }
        let mut failed_osnma = false;
        let mut unavailable = [false; N_SRC];
        for ft in faults.iter().filter(|ft| ft.active(t)) {
            let ramp = 2e-9 * (t - ft.t0);
            match ft.kind {
                FaultType::S1 => {
                    f[GNSS] += ramp;
                    lab[GNSS] = true;
                    failed_osnma = true;
                }
                FaultType::S2 => {
                    f[GNSS] += ramp;
                    f[OSNMA] += ramp;
                    lab[GNSS] = true;
                    lab[OSNMA] = true;
                }
                FaultType::J => {
                    unavailable[GNSS] = true;
                    unavailable[OSNMA] = true;
                }
                FaultType::P => {
                    f[PTP] += 500e-9;
                    lab[PTP] = true;
                }
                FaultType::L => {
                    f[LEO] += 300e-9;
                    lab[LEO] = true;
                }
                FaultType::E => unavailable[ELORAN] = true,
                FaultType::A => unavailable = [true; N_SRC],
            }
        }
        if failed_osnma {
            st[OSNMA] = Status::Failed;
        }
        for s in 0..N_SRC {
            if unavailable[s] {
                st[s] = Status::Unavailable;
            }
        }
        let mut row = [Obs {
            status: Status::Unavailable,
            offset_s: 0.0,
        }; N_SRC];
        for s in 0..N_SRC {
            row[s] = Obs {
                status: st[s],
                offset_s: if st[s] == Status::Ok {
                    run.x[k] + noise[k][s] + f[s]
                } else {
                    0.0
                },
            };
        }
        obs.push(row);
        fault_err.push(f);
        faulty.push(lab);
    }
    Generated {
        obs,
        fault_err,
        faulty,
        faults,
    }
}

/// A simple selector of the baselines: a fixed per-epoch pick with the section 2.3 holdover.
fn baseline(run: &Run, gen: &Generated, pick: impl Fn(usize) -> Option<usize>) -> Vec<Option<f64>> {
    let cfg = Config::v4(&run.class, run.tau0);
    let mut ho = Holdover::new(&run.class).expect("known class");
    let mut out = Vec::with_capacity(run.x.len());
    for k in 0..run.x.len() {
        let t = k as f64 * run.tau0;
        let h = ho.estimate(t);
        match pick(k) {
            Some(s) => {
                let o = gen.obs[k][s].offset_s;
                let sg = cfg.sources[s].sigma_ns * 1e-9;
                ho.push(t, o, sg * sg);
                out.push(Some(o - run.x[k]));
            }
            None => out.push(h.map(|h| h.offset_s - run.x[k])),
        }
    }
    out
}

/// Per-data-set totals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    pub runs: usize,
    pub dropped: usize,
    pub epochs: usize,
    pub output_epochs: usize,
    pub misleading: usize,
    pub episodes_assessable: usize,
    pub episodes_passed: usize,
    /// Per fault type (S1, S2, P, L): (assessable, passed).
    pub by_type: Vec<(String, usize, usize)>,
    pub te_sel: Vec<f64>,
    pub te_oracle: Vec<f64>,
    pub te_gnss: Vec<f64>,
    pub changes: usize,
    pub transitions: usize,
    pub b5_eligible: usize,
    pub b5_external: usize,
    pub holdover_epochs: usize,
    pub none_epochs: usize,
}

/// Nearest-rank percentile (`p` in (0, 1]).
pub fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let r = ((p * v.len() as f64).ceil() as usize).clamp(1, v.len());
    v[r - 1]
}

/// Score one run into `t`.
pub fn score_run(run: &Run, seed: u64, t: &mut Tally) {
    let cfg = Config::v4(&run.class, run.tau0);
    let gen = generate(run, &cfg, seed);
    let sig: Vec<f64> = cfg.sources.iter().map(|s| s.sigma_ns * 1e-9).collect();
    let group = [0usize, 0, 1, 2, 3];
    let n = run.x.len();
    let mut sel = Selector::new(cfg.clone()).expect("the V4 configuration is valid");
    let mut choice = Vec::with_capacity(n);
    for k in 0..n {
        let tt = k as f64 * run.tau0;
        let d = sel.step(tt, &gen.obs[k]);
        t.epochs += 1;
        match d.choice {
            Choice::Holdover => t.holdover_epochs += 1,
            Choice::None => t.none_epochs += 1,
            _ => {}
        }
        if let (Some(o), Some(pl)) = (d.offset_s, d.pl_s) {
            t.output_epochs += 1;
            let te = o - run.x[k];
            if te.abs() > pl {
                t.misleading += 1;
            }
            t.te_sel.push(te.abs());
        }
        if k > 0 && choice.last() != Some(&d.choice) {
            t.changes += 1;
        }
        // Transitions: any status or faulty label change.
        if k > 0 {
            let changed = (0..N_SRC).any(|s| {
                gen.obs[k][s].status != gen.obs[k - 1][s].status
                    || gen.faulty[k][s] != gen.faulty[k - 1][s]
            });
            if changed {
                t.transitions += 1;
            }
        }
        // B5.
        let mut groups_ok = [false; 4];
        for s in 0..N_SRC {
            if gen.obs[k][s].status == Status::Ok && !gen.faulty[k][s] {
                groups_ok[group[s]] = true;
            }
        }
        if groups_ok.iter().filter(|g| **g).count() >= 2 {
            t.b5_eligible += 1;
            if matches!(d.choice, Choice::Source(_)) {
                t.b5_external += 1;
            }
        }
        choice.push(d.choice);
    }
    // Oracle and GNSS-only.
    let oracle_pick = |k: usize| {
        (0..N_SRC)
            .filter(|&s| gen.obs[k][s].status == Status::Ok && !gen.faulty[k][s])
            .min_by(|&a, &b| sig[a].total_cmp(&sig[b]).then(a.cmp(&b)))
    };
    for te in baseline(run, &gen, oracle_pick).into_iter().flatten() {
        t.te_oracle.push(te.abs());
    }
    let gnss_pick = |k: usize| (gen.obs[k][GNSS].status == Status::Ok).then_some(GNSS);
    for te in baseline(run, &gen, gnss_pick).into_iter().flatten() {
        t.te_gnss.push(te.abs());
    }
    // B2: episodes per (fault, source).
    for f in &gen.faults {
        let srcs: &[usize] = match f.kind {
            FaultType::S1 => &[GNSS],
            FaultType::S2 => &[GNSS, OSNMA],
            FaultType::P => &[PTP],
            FaultType::L => &[LEO],
            _ => &[],
        };
        for &s in srcs {
            let ref_sigma = |k: usize| {
                (0..N_SRC)
                    .filter(|&r| {
                        group[r] != group[s]
                            && gen.obs[k][r].status == Status::Ok
                            && !gen.faulty[k][r]
                    })
                    .map(|r| sig[r])
                    .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))))
            };
            let epochs: Vec<usize> = (0..n).filter(|&k| f.active(k as f64 * run.tau0)).collect();
            let first = epochs.iter().copied().find(|&k| {
                ref_sigma(k).is_some_and(|sr| {
                    gen.fault_err[k][s].abs() > 5.0 * (sig[s] * sig[s] + sr * sr).sqrt()
                })
            });
            let Some(first) = first else { continue };
            let assessable: Vec<usize> = epochs
                .iter()
                .copied()
                .filter(|&k| k >= first + 3 && ref_sigma(k).is_some())
                .collect();
            if assessable.is_empty() {
                continue;
            }
            let passed = assessable.iter().all(|&k| choice[k] != Choice::Source(s));
            t.episodes_assessable += 1;
            if passed {
                t.episodes_passed += 1;
            }
            let name = f.kind.name();
            match t.by_type.iter_mut().find(|e| e.0 == name) {
                Some(e) => {
                    e.1 += 1;
                    e.2 += passed as usize;
                }
                None => t.by_type.push((name.into(), 1, passed as usize)),
            }
        }
    }
    t.runs += 1;
}

/// The bars of one data set.
#[derive(Clone, Debug, PartialEq)]
pub struct Bars {
    pub name: String,
    pub b1_share: f64,
    pub b1: bool,
    pub b2_share: f64,
    pub b2_assessable: bool,
    pub b2: bool,
    pub b3a_ratio: f64,
    pub b3a: bool,
    pub b3b_sel: f64,
    pub b3b_gnss: f64,
    pub b3b: bool,
    pub b4_changes: usize,
    pub b4_limit: usize,
    pub b4: bool,
    pub b5_share: f64,
    pub b5: bool,
    pub p95_sel: f64,
    pub p95_oracle: f64,
}

impl Bars {
    /// B1 to B5 (B6 is checked separately).
    pub fn passed(&self) -> bool {
        self.b1 && self.b2 && self.b3a && self.b3b && self.b4 && self.b5
    }
}

/// Compute the bars from a tally.
pub fn bars(name: &str, t: &mut Tally) -> Bars {
    let b1_share = t.misleading as f64 / t.output_epochs.max(1) as f64;
    let b2_share = t.episodes_passed as f64 / t.episodes_assessable.max(1) as f64;
    let b2_assessable = t.episodes_assessable >= 30;
    let p95_sel = percentile(&mut t.te_sel, 0.95);
    let p95_oracle = percentile(&mut t.te_oracle, 0.95);
    let b3b_sel = percentile(&mut t.te_sel, 0.999);
    let b3b_gnss = percentile(&mut t.te_gnss, 0.999);
    let b3a_ratio = p95_sel / p95_oracle;
    let b4_limit = 2 * t.transitions + 2 * t.runs;
    let b5_share = t.b5_external as f64 / t.b5_eligible.max(1) as f64;
    Bars {
        name: name.into(),
        b1_share,
        b1: b1_share <= 0.001,
        b2_share,
        b2_assessable,
        b2: b2_assessable && b2_share >= 0.95,
        b3a_ratio,
        b3a: b3a_ratio <= 1.25,
        b3b_sel,
        b3b_gnss,
        b3b: b3b_sel < b3b_gnss,
        b4_changes: t.changes,
        b4_limit,
        b4: t.changes <= b4_limit,
        b5_share,
        b5: b5_share >= 0.99,
        p95_sel,
        p95_oracle,
    }
}

/// The seed of run `index` of data set `set` (R1 = 1, R2 = 2, S = 3).
pub fn seed(set: u64, index: usize) -> u64 {
    20_261_115 + 1000 * set + index as u64
}

/// Score a whole data set.
pub fn score_set(name: &str, set: u64, runs: &[Run], dropped: usize) -> (Bars, Tally) {
    let mut t = Tally {
        dropped,
        ..Tally::default()
    };
    for (i, r) in runs.iter().enumerate() {
        score_run(r, seed(set, i), &mut t);
    }
    t.by_type.sort();
    let b = bars(name, &mut t);
    (b, t)
}

/// The source table of one generated run, in the `kshana-pro trust run` input format (B6).
pub fn table(run: &Run, gen: &Generated) -> String {
    let cfg = Config::v4(&run.class, run.tau0);
    let mut s = String::from("t_s");
    for src in &cfg.sources {
        s.push_str(&format!(",{0}_status,{0}_offset_ns", src.id));
    }
    s.push('\n');
    for (k, row) in gen.obs.iter().enumerate() {
        s.push_str(&format!("{}", k as f64 * run.tau0));
        for o in row {
            if o.status == Status::Ok {
                s.push_str(&format!(",ok,{:.6}", o.offset_s * 1e9));
            } else {
                s.push_str(&format!(",{},", o.status.as_str()));
            }
        }
        s.push('\n');
    }
    s
}
