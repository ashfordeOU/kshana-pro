// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! B2003: radiation-fault signatures in a spacecraft GNSS receiver's output trace, and an
//! injector that writes fixtures with ground truth.
//!
//! The trace (`kshana-space-rx-trace/1`, CSV) is what the on-board computer logs from the
//! receiver: the OBC's own receive time, the receiver's time tag, the ECEF solution, clock
//! bias, satellites used, mean C/N0, fix flag, the receiver's reset counter and the supply
//! current. The detector looks for the signatures single-event effects leave in such a trace:
//!
//! | Signature | Class | What it looks like |
//! |---|---|---|
//! | `latchup_suspect` | SEL | supply current above the trace's baseline by a set fraction for several epochs |
//! | `reset` | SEFI/SEL | output gap or loss of fix with the reset counter going up |
//! | `functional_interrupt` | SEFI | output gap without a reset |
//! | `frozen_output` | SEFI | the same solution and time tag repeated while the OBC clock advances |
//! | `output_upset` | SEU | one epoch off the orbit dynamics on one axis, the next back on it; a bit-flip test on the difference |
//! | `timetag_upset` | SEU | one time tag off the OBC clock, the next back on it |
//! | `clock_upset` | SEU | one clock-bias value off the trend, the next back on it |
//!
//! It also reports what is **not** a radiation signature but would otherwise be confused with
//! one: a time-tag or solution **step** that persists, a clock **millisecond jump** (a normal
//! steering action of many receivers) and loss of fix without a reset.
//!
//! The dynamics check propagates the previous epoch with two-body gravity in the rotating
//! Earth frame (RK4); thresholds are parameters with stated defaults. MODELLED: no flight
//! trace has been processed in this build; the fixtures are SYNTHETIC.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{round, LABEL};

pub const TRACE_FORMAT: &str = "kshana-space-rx-trace/1";
pub const INJECT_FORMAT: &str = "kshana-space-rx-inject/1";
pub const RESULT_SCHEMA: &str = "kshana-space-rx-see-result/1";
/// The trace's column header.
pub const HEADER: &str = "t_s,tow_s,x_m,y_m,z_m,vx_mps,vy_mps,vz_mps,clock_bias_m,n_sv,cn0_mean_dbhz,fix,reset_count,current_ma";

const MU: f64 = 3.986_004_418e14;
const OMEGA_E: f64 = 7.292_115e-5;
const WEEK_S: f64 = 604_800.0;
/// One millisecond of light travel, metres (receiver clock steering steps).
pub const MS_M: f64 = 299_792.458;

/// One logged epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct Epoch {
    pub t_s: f64,
    pub tow_s: f64,
    pub r: [f64; 3],
    pub v: [f64; 3],
    pub clock_bias_m: f64,
    pub n_sv: u32,
    pub cn0: f64,
    pub fix: bool,
    pub reset_count: u32,
    pub current_ma: f64,
}

/// Parse a trace (header comment line optional, then the column header).
pub fn parse_trace(text: &str) -> Result<Vec<Epoch>, String> {
    let mut out = Vec::new();
    let mut header_seen = false;
    for (ln, line) in text.lines().enumerate() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if !header_seen {
            if l != HEADER {
                return Err(format!(
                    "trace line {}: expected the header {HEADER}",
                    ln + 1
                ));
            }
            header_seen = true;
            continue;
        }
        let f: Vec<&str> = l.split(',').collect();
        if f.len() != 14 {
            return Err(format!("trace line {}: expected 14 fields", ln + 1));
        }
        let num = |i: usize| -> Result<f64, String> {
            if f[i].is_empty() {
                return Ok(f64::NAN);
            }
            f[i].parse::<f64>()
                .map_err(|_| format!("trace line {}: field {} not a number", ln + 1, i + 1))
        };
        out.push(Epoch {
            t_s: num(0)?,
            tow_s: num(1)?,
            r: [num(2)?, num(3)?, num(4)?],
            v: [num(5)?, num(6)?, num(7)?],
            clock_bias_m: num(8)?,
            n_sv: num(9)? as u32,
            cn0: num(10)?,
            fix: num(11)? > 0.0,
            reset_count: num(12)? as u32,
            current_ma: num(13)?,
        });
    }
    if !header_seen {
        return Err("trace: no header".into());
    }
    if out.windows(2).any(|w| w[1].t_s <= w[0].t_s) {
        return Err("trace: OBC time t_s must increase".into());
    }
    Ok(out)
}

/// Write a trace.
pub fn write_trace(e: &[Epoch]) -> String {
    let mut s = format!("# {TRACE_FORMAT}\n{HEADER}\n");
    for x in e {
        s.push_str(&format!(
            "{:.3},{:.3},{:.2},{:.2},{:.2},{:.3},{:.3},{:.3},{:.2},{},{:.1},{},{},{}\n",
            x.t_s,
            x.tow_s,
            x.r[0],
            x.r[1],
            x.r[2],
            x.v[0],
            x.v[1],
            x.v[2],
            x.clock_bias_m,
            x.n_sv,
            x.cn0,
            u8::from(x.fix),
            x.reset_count,
            if x.current_ma.is_finite() {
                format!("{:.1}", x.current_ma)
            } else {
                String::new()
            }
        ));
    }
    s
}

fn accel(r: [f64; 3], v: [f64; 3]) -> [f64; 3] {
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let k = -MU / (n * n * n);
    let w2 = OMEGA_E * OMEGA_E;
    [
        k * r[0] + 2.0 * OMEGA_E * v[1] + w2 * r[0],
        k * r[1] - 2.0 * OMEGA_E * v[0] + w2 * r[1],
        k * r[2],
    ]
}

/// Propagate an ECEF state by `dt` seconds (two-body, rotating frame, RK4 in 1 s substeps).
pub fn propagate(r: [f64; 3], v: [f64; 3], dt: f64) -> ([f64; 3], [f64; 3]) {
    let n = dt.abs().ceil().max(1.0) as usize;
    let h = dt / n as f64;
    let (mut r, mut v) = (r, v);
    let add =
        |a: [f64; 3], b: [f64; 3], s: f64| [a[0] + s * b[0], a[1] + s * b[1], a[2] + s * b[2]];
    for _ in 0..n {
        let k1v = accel(r, v);
        let k1r = v;
        let k2v = accel(add(r, k1r, h / 2.0), add(v, k1v, h / 2.0));
        let k2r = add(v, k1v, h / 2.0);
        let k3v = accel(add(r, k2r, h / 2.0), add(v, k2v, h / 2.0));
        let k3r = add(v, k2v, h / 2.0);
        let k4v = accel(add(r, k3r, h), add(v, k3v, h));
        let k4r = add(v, k3v, h);
        for i in 0..3 {
            r[i] += h / 6.0 * (k1r[i] + 2.0 * k2r[i] + 2.0 * k3r[i] + k4r[i]);
            v[i] += h / 6.0 * (k1v[i] + 2.0 * k2v[i] + 2.0 * k3v[i] + k4v[i]);
        }
    }
    (r, v)
}

fn d_pos() -> f64 {
    30.0
}
fn d_tt() -> f64 {
    0.5
}
fn d_clock() -> f64 {
    100.0
}
fn d_freeze() -> usize {
    3
}
fn d_gap() -> f64 {
    3.0
}
fn d_latch() -> f64 {
    30.0
}
fn d_latch_n() -> usize {
    2
}
fn d_res() -> f64 {
    0.01
}
fn d_flip_tol() -> f64 {
    5.0
}
fn d_max_dt() -> f64 {
    30.0
}

/// Detector thresholds (defaults stated in every result).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// Position residual against the dynamics that marks an epoch off, metres.
    #[serde(default = "d_pos")]
    pub pos_threshold_m: f64,
    /// Time-tag disagreement with the OBC clock, seconds.
    #[serde(default = "d_tt")]
    pub timetag_tolerance_s: f64,
    /// Clock-bias residual against the trend, metres.
    #[serde(default = "d_clock")]
    pub clock_threshold_m: f64,
    /// Repeated identical epochs that count as frozen output.
    #[serde(default = "d_freeze")]
    pub freeze_epochs: usize,
    /// A gap longer than this many nominal intervals is an output interruption.
    #[serde(default = "d_gap")]
    pub gap_factor: f64,
    /// Supply-current rise over the baseline median, percent.
    #[serde(default = "d_latch")]
    pub latchup_rise_pct: f64,
    #[serde(default = "d_latch_n")]
    pub latchup_epochs: usize,
    /// Output resolution of the position fields (for the bit-flip test), metres.
    #[serde(default = "d_res")]
    pub resolution_m: f64,
    /// How close a difference must be to a power of two of the resolution, metres.
    #[serde(default = "d_flip_tol")]
    pub bitflip_tolerance_m: f64,
    /// The dynamics check is skipped across gaps longer than this, seconds.
    #[serde(default = "d_max_dt")]
    pub max_dynamics_dt_s: f64,
}

impl Default for Params {
    fn default() -> Self {
        toml::from_str("").expect("defaults")
    }
}

/// One detected event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// The signature name.
    pub kind: String,
    /// `SEU`, `SEFI`, `SEL`, `SEFI/SEL` or `other` (not a radiation signature).
    pub class: String,
    pub epoch: usize,
    pub t_s: f64,
    pub detail: String,
}

fn ev(kind: &str, class: &str, i: usize, t: f64, detail: String) -> Event {
    Event {
        kind: kind.into(),
        class: class.into(),
        epoch: i,
        t_s: t,
        detail,
    }
}

/// Whether `d` (metres) is a single bit flip at resolution `res`: the bit index.
pub fn bitflip_bit(d: f64, res: f64, tol: f64) -> Option<u32> {
    let q = d.abs() / res;
    if q < 1.0 {
        return None;
    }
    let k = q.log2().round();
    if !(0.0..63.0).contains(&k) {
        return None;
    }
    ((2f64.powf(k) * res - d.abs()).abs() <= tol).then_some(k as u32)
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    if v.is_empty() {
        f64::NAN
    } else {
        v[v.len() / 2]
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn tow_diff(a: f64, b: f64) -> f64 {
    let mut d = b - a;
    if d < -WEEK_S / 2.0 {
        d += WEEK_S;
    }
    d
}

/// Run the detector.
pub fn detect(e: &[Epoch], p: &Params) -> Vec<Event> {
    let mut out = Vec::new();
    if e.len() < 3 {
        return out;
    }
    let mut dts: Vec<f64> = e.windows(2).map(|w| w[1].t_s - w[0].t_s).collect();
    let nominal = median(&mut dts);
    let mut cur: Vec<f64> = e
        .iter()
        .map(|x| x.current_ma)
        .filter(|x| x.is_finite())
        .collect();
    let base_i = median(&mut cur);
    let ok_fix = |i: usize| e[i].fix && e[i].r.iter().all(|x| x.is_finite());
    let same = |a: &Epoch, b: &Epoch| a.r == b.r && a.tow_s == b.tow_s;

    // Latch-up suspects.
    if base_i.is_finite() && base_i > 0.0 {
        let lim = base_i * (1.0 + p.latchup_rise_pct / 100.0);
        let mut i = 0;
        while i < e.len() {
            if e[i].current_ma > lim {
                let mut j = i;
                while j + 1 < e.len() && e[j + 1].current_ma > lim {
                    j += 1;
                }
                if j + 1 - i >= p.latchup_epochs {
                    let peak = e[i..=j]
                        .iter()
                        .map(|x| x.current_ma)
                        .fold(f64::MIN, f64::max);
                    let reset_after = e[j..e.len().min(j + 5)]
                        .iter()
                        .any(|x| x.reset_count > e[i].reset_count);
                    out.push(ev(
                        "latchup_suspect",
                        "SEL",
                        i,
                        e[i].t_s,
                        format!(
                            "supply current {} mA over {} epochs (baseline {} mA, +{}%){}",
                            round(peak, 1),
                            j + 1 - i,
                            round(base_i, 1),
                            round((peak / base_i - 1.0) * 100.0, 0),
                            if reset_after {
                                "; followed by a reset or power cycle"
                            } else {
                                ""
                            }
                        ),
                    ));
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
    }

    // Gaps, resets, loss of fix.
    for i in 1..e.len() {
        let dt = e[i].t_s - e[i - 1].t_s;
        let reset = e[i].reset_count > e[i - 1].reset_count;
        if dt > p.gap_factor * nominal {
            out.push(if reset {
                ev(
                    "reset",
                    "SEFI/SEL",
                    i,
                    e[i].t_s,
                    format!(
                        "output gap {} s; reset counter {} -> {}",
                        round(dt, 1),
                        e[i - 1].reset_count,
                        e[i].reset_count
                    ),
                )
            } else {
                ev(
                    "functional_interrupt",
                    "SEFI",
                    i,
                    e[i].t_s,
                    format!("output gap {} s without a reset", round(dt, 1)),
                )
            });
        } else if reset {
            out.push(ev(
                "reset",
                "SEFI/SEL",
                i,
                e[i].t_s,
                format!(
                    "reset counter {} -> {} without an output gap",
                    e[i - 1].reset_count,
                    e[i].reset_count
                ),
            ));
        }
    }
    let mut i = 1;
    while i < e.len() {
        if e[i - 1].fix && !e[i].fix {
            let mut j = i;
            while j + 1 < e.len() && !e[j + 1].fix {
                j += 1;
            }
            let reset = e[i..=j.min(e.len() - 1)]
                .iter()
                .any(|x| x.reset_count > e[i - 1].reset_count)
                || e.get(j + 1)
                    .is_some_and(|x| x.reset_count > e[i - 1].reset_count);
            let covered = out
                .iter()
                .any(|x| x.kind == "reset" && x.epoch >= i && x.epoch <= j + 1);
            if !reset {
                out.push(ev(
                    "loss_of_fix",
                    "other",
                    i,
                    e[i].t_s,
                    format!("no fix for {} epochs, no reset", j + 1 - i),
                ));
            } else if !covered {
                out.push(ev(
                    "reset",
                    "SEFI/SEL",
                    i,
                    e[i].t_s,
                    format!("no fix for {} epochs; reset counter rose", j + 1 - i),
                ));
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }

    // Frozen output. `skip` marks the frozen epochs and the one after (its predecessor is
    // stale), which the time-tag, dynamics and clock checks leave out.
    let mut skip = vec![false; e.len()];
    let mut i = 1;
    while i < e.len() {
        if ok_fix(i) && same(&e[i], &e[i - 1]) {
            let mut j = i;
            while j + 1 < e.len() && same(&e[j + 1], &e[i - 1]) {
                j += 1;
            }
            let n = j + 2 - i;
            if n >= p.freeze_epochs {
                out.push(ev(
                    "frozen_output",
                    "SEFI",
                    i - 1,
                    e[i - 1].t_s,
                    format!(
                        "the same solution and time tag {n} times over {} s of OBC time",
                        round(e[j].t_s - e[i - 1].t_s, 1)
                    ),
                ));
            }
            for s in skip.iter_mut().take((j + 2).min(e.len())).skip(i) {
                *s = true;
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    let frozen = |k: usize| skip[k];

    // Time tags.
    let mut extra = Vec::new();
    let tt_bad = |a: usize, b: usize| {
        (tow_diff(e[a].tow_s, e[b].tow_s) - (e[b].t_s - e[a].t_s)).abs() > p.timetag_tolerance_s
    };
    let mut i = 1;
    while i < e.len() {
        let reset = e[i].reset_count > e[i - 1].reset_count;
        if !reset && e[i].fix && e[i - 1].fix && !frozen(i) && tt_bad(i - 1, i) {
            let d = tow_diff(e[i - 1].tow_s, e[i].tow_s) - (e[i].t_s - e[i - 1].t_s);
            if i + 1 < e.len() && !tt_bad(i - 1, i + 1) {
                extra.push(ev(
                    "timetag_upset",
                    "SEU",
                    i,
                    e[i].t_s,
                    format!(
                        "time tag off the OBC clock by {} s for one epoch",
                        round(d, 3)
                    ),
                ));
                i += 2;
                continue;
            }
            extra.push(ev(
                "timetag_step",
                "other",
                i,
                e[i].t_s,
                format!(
                    "time tag stepped by {} s against the OBC clock and stayed",
                    round(d, 3)
                ),
            ));
        }
        i += 1;
    }

    // Dynamics: one-epoch upsets on the solution.
    let resid = |a: usize, b: usize| -> Option<[f64; 3]> {
        let dt = e[b].t_s - e[a].t_s;
        if dt > p.max_dynamics_dt_s || !ok_fix(a) || !ok_fix(b) {
            return None;
        }
        let (r, _) = propagate(e[a].r, e[a].v, dt);
        Some(dist(e[b].r, r))
    };
    let big = |d: [f64; 3]| d.iter().any(|x| x.abs() > p.pos_threshold_m);
    let mut i = 1;
    while i + 1 < e.len() {
        if e[i].reset_count != e[i - 1].reset_count || frozen(i) {
            i += 1;
            continue;
        }
        let Some(d) = resid(i - 1, i) else {
            i += 1;
            continue;
        };
        if big(d) {
            let back = resid(i - 1, i + 1).is_some_and(|d2| !big(d2));
            let axes: Vec<usize> = (0..3).filter(|k| d[*k].abs() > p.pos_threshold_m).collect();
            if back {
                let flip = if axes.len() == 1 {
                    bitflip_bit(d[axes[0]], p.resolution_m, p.bitflip_tolerance_m)
                } else {
                    None
                };
                extra.push(ev(
                    "output_upset",
                    "SEU",
                    i,
                    e[i].t_s,
                    format!(
                        "one epoch off the dynamics by ({}, {}, {}) m{}",
                        round(d[0], 2),
                        round(d[1], 2),
                        round(d[2], 2),
                        match flip {
                            Some(k) => format!(
                                "; {} axis consistent with a flip of bit {k} at {} m resolution",
                                ["x", "y", "z"][axes[0]],
                                p.resolution_m
                            ),
                            None => String::new(),
                        }
                    ),
                ));
                i += 2;
                continue;
            }
            extra.push(ev(
                "solution_step",
                "other",
                i,
                e[i].t_s,
                format!(
                    "solution left the dynamics by ({}, {}, {}) m and did not return",
                    round(d[0], 1),
                    round(d[1], 1),
                    round(d[2], 1)
                ),
            ));
        }
        i += 1;
    }

    // Clock bias: predicted from the last two epochs accepted as clean.
    let mut good: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < e.len() {
        let usable = e[i].fix && !frozen(i) && e[i].clock_bias_m.is_finite();
        if !usable {
            i += 1;
            continue;
        }
        if let Some(&g) = good.last() {
            if e[g].reset_count != e[i].reset_count {
                good.clear();
            }
        }
        if good.len() < 2 || i + 1 >= e.len() {
            good.push(i);
            i += 1;
            continue;
        }
        let (a, b) = (good[good.len() - 2], good[good.len() - 1]);
        let rate = (e[b].clock_bias_m - e[a].clock_bias_m) / (e[b].t_s - e[a].t_s);
        let pred = |k: usize| e[b].clock_bias_m + rate * (e[k].t_s - e[b].t_s);
        let d = e[i].clock_bias_m - pred(i);
        if d.abs() <= p.clock_threshold_m {
            good.push(i);
            i += 1;
            continue;
        }
        let back = (e[i + 1].clock_bias_m - pred(i + 1)).abs() <= p.clock_threshold_m;
        let ms = d / MS_M;
        if back {
            extra.push(ev(
                "clock_upset",
                "SEU",
                i,
                e[i].t_s,
                format!(
                    "clock bias off its trend by {} m for one epoch",
                    round(d, 1)
                ),
            ));
            i += 1;
            continue;
        }
        if (ms - ms.round()).abs() * MS_M < p.clock_threshold_m && ms.round() != 0.0 {
            extra.push(ev(
                "clock_ms_jump",
                "other",
                i,
                e[i].t_s,
                format!(
                    "clock bias stepped by {} ms (receiver clock steering)",
                    ms.round()
                ),
            ));
        } else {
            extra.push(ev(
                "clock_step",
                "other",
                i,
                e[i].t_s,
                format!("clock bias stepped by {} m and stayed", round(d, 1)),
            ));
        }
        good.clear();
        good.push(i);
        i += 1;
    }
    out.extend(extra);
    out.sort_by(|a, b| a.epoch.cmp(&b.epoch).then(a.kind.cmp(&b.kind)));
    out
}

/// The detector result document.
pub fn result(e: &[Epoch], p: &Params, events: &[Event], trace_sha256: &str) -> Value {
    let count = |c: &str| events.iter().filter(|x| x.class.contains(c)).count();
    json!({
        "schema": RESULT_SCHEMA,
        "label": LABEL,
        "evidence": "MODELLED",
        "trace_sha256": trace_sha256,
        "epochs": e.len(),
        "span_s": e.last().map_or(0.0, |l| round(l.t_s - e[0].t_s, 3)),
        "params": p,
        "counts": {
            "SEU": count("SEU"),
            "SEFI": count("SEFI"),
            "SEL": count("SEL"),
            "other": count("other"),
        },
        "events": events,
        "note": "A signature is consistent with a single-event effect; it is not proof of one. Class `other` marks behaviour that is not a radiation signature.",
    })
}

// ── Synthetic traces and injection ─────────────────────────────────────────────────────

/// A deterministic clean trace: circular orbit, `n` epochs every `dt` s, small noise.
pub fn synth_trace(n: usize, dt: f64, alt_km: f64, inc_deg: f64, seed: u64) -> Vec<Epoch> {
    let a = 6_378_137.0 + alt_km * 1e3;
    let w = (MU / (a * a * a)).sqrt();
    let inc = inc_deg.to_radians();
    let mut s = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let mut noise = move || {
        s = s
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((s >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 2.0
    };
    (0..n)
        .map(|k| {
            let t = k as f64 * dt;
            let u = w * t;
            let ri = [
                a * u.cos(),
                a * u.sin() * inc.cos(),
                a * u.sin() * inc.sin(),
            ];
            let vi = [
                -a * w * u.sin(),
                a * w * u.cos() * inc.cos(),
                a * w * u.cos() * inc.sin(),
            ];
            let th = OMEGA_E * t;
            let (c, sn) = (th.cos(), th.sin());
            let r = [c * ri[0] + sn * ri[1], -sn * ri[0] + c * ri[1], ri[2]];
            let vr = [vi[0] + OMEGA_E * ri[1], vi[1] - OMEGA_E * ri[0], vi[2]];
            let v = [c * vr[0] + sn * vr[1], -sn * vr[0] + c * vr[1], vr[2]];
            Epoch {
                t_s: 1000.0 + t + 0.002 * noise(),
                tow_s: 345_600.0 + t,
                r: [r[0] + noise(), r[1] + noise(), r[2] + noise()],
                v: [
                    v[0] + 0.01 * noise(),
                    v[1] + 0.01 * noise(),
                    v[2] + 0.01 * noise(),
                ],
                clock_bias_m: 1500.0 + 0.3 * t + noise(),
                n_sv: 9,
                cn0: 44.0 + noise(),
                fix: true,
                reset_count: 0,
                current_ma: 120.0 + noise(),
            }
        })
        .collect()
}

/// One event to inject.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inject {
    /// `bitflip`, `timetag`, `clock`, `freeze`, `sefi`, `reset` or `latchup`.
    pub kind: String,
    pub epoch: usize,
    /// Axis for `bitflip` (`x`, `y`, `z`).
    #[serde(default)]
    pub axis: String,
    /// Bit for `bitflip` (of the position at the stated resolution).
    #[serde(default)]
    pub bit: u32,
    /// Epochs affected (`freeze`, `sefi`, `reset`, `latchup`).
    #[serde(default)]
    pub length: usize,
    /// Size for `timetag` (s) and `clock` (m).
    #[serde(default)]
    pub magnitude: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InjectFile {
    pub format: String,
    #[serde(default = "d_res")]
    pub resolution_m: f64,
    #[serde(rename = "event")]
    pub events: Vec<Inject>,
}

impl InjectFile {
    pub fn parse(src: &str) -> Result<Self, String> {
        let f: InjectFile = toml::from_str(src).map_err(|e| format!("inject: {e}"))?;
        if f.format != INJECT_FORMAT {
            return Err(format!("inject: format must be \"{INJECT_FORMAT}\""));
        }
        Ok(f)
    }
}

/// The signature each injected kind should produce.
pub fn expected_kind(kind: &str) -> &'static str {
    match kind {
        "bitflip" => "output_upset",
        "timetag" => "timetag_upset",
        "clock" => "clock_upset",
        "freeze" => "frozen_output",
        "sefi" => "functional_interrupt",
        "reset" => "reset",
        "latchup" => "latchup_suspect",
        _ => "unknown",
    }
}

/// Inject events into a clean trace; returns the new trace and the ground truth (the epoch
/// index in the new trace where each signature should be found).
pub fn inject(clean: &[Epoch], f: &InjectFile) -> Result<(Vec<Epoch>, Value), String> {
    let mut e = clean.to_vec();
    let mut drop = vec![false; e.len()];
    let mut truth = Vec::new();
    let mut evs = f.events.clone();
    evs.sort_by_key(|x| x.epoch);
    for x in &evs {
        let i = x.epoch;
        if i == 0 || i + x.length.max(1) + 2 >= e.len() {
            return Err(format!("inject: epoch {i} out of range"));
        }
        match x.kind.as_str() {
            "bitflip" => {
                let k = match x.axis.as_str() {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    _ => return Err("inject: bitflip needs axis x, y or z".into()),
                };
                if x.bit > 40 {
                    return Err("inject: bit must be at most 40".into());
                }
                let q = (e[i].r[k] / f.resolution_m).round() as i64;
                e[i].r[k] = (q ^ (1i64 << x.bit)) as f64 * f.resolution_m;
            }
            "timetag" => e[i].tow_s += if x.magnitude == 0.0 { 1.0 } else { x.magnitude },
            "clock" => {
                e[i].clock_bias_m += if x.magnitude == 0.0 {
                    1000.0
                } else {
                    x.magnitude
                }
            }
            "freeze" => {
                for k in i + 1..=i + x.length.max(2) {
                    let t = e[k].t_s;
                    e[k] = e[i].clone();
                    e[k].t_s = t;
                }
            }
            "sefi" | "reset" => {
                for d in drop.iter_mut().skip(i).take(x.length.max(5)) {
                    *d = true;
                }
                if x.kind == "reset" {
                    let after = i + x.length.max(5);
                    for y in e.iter_mut().skip(after) {
                        y.reset_count += 1;
                    }
                }
            }
            "latchup" => {
                for y in e.iter_mut().skip(i).take(x.length.max(3)) {
                    y.current_ma *= 1.8;
                }
                let after = i + x.length.max(3);
                for d in drop.iter_mut().skip(after).take(5) {
                    *d = true;
                }
                for y in e.iter_mut().skip(after + 5) {
                    y.reset_count += 1;
                }
            }
            other => return Err(format!("inject: unknown kind '{other}'")),
        }
    }
    // Index map after dropping epochs.
    let mut map = vec![0usize; e.len() + 1];
    let mut n = 0;
    for (k, d) in drop.iter().enumerate() {
        map[k] = n;
        if !d {
            n += 1;
        }
    }
    map[e.len()] = n;
    for x in &evs {
        let i = x.epoch;
        let at = match x.kind.as_str() {
            "freeze" => map[i],
            "sefi" | "reset" => map[i + x.length.max(5)],
            _ => map[i],
        };
        truth.push(json!({"kind": x.kind, "expect": expected_kind(&x.kind), "epoch": at}));
        if x.kind == "latchup" {
            truth.push(json!({"kind": "latchup", "expect": "reset", "epoch": map[i + x.length.max(3) + 5]}));
        }
    }
    let kept: Vec<Epoch> = e
        .into_iter()
        .zip(drop)
        .filter(|(_, d)| !d)
        .map(|(x, _)| x)
        .collect();
    Ok((kept, json!({"format": INJECT_FORMAT, "truth": truth})))
}

/// Score detected events against ground truth: (found, missed, unexpected).
pub fn match_truth(truth: &Value, events: &[Event]) -> (usize, Vec<String>, Vec<String>) {
    let mut used = vec![false; events.len()];
    let mut found = 0;
    let mut missed = Vec::new();
    for t in truth["truth"].as_array().into_iter().flatten() {
        let want = t["expect"].as_str().unwrap_or("");
        let at = t["epoch"].as_u64().unwrap_or(0) as usize;
        match events
            .iter()
            .enumerate()
            .find(|(k, x)| !used[*k] && x.kind == want && x.epoch.abs_diff(at) <= 1)
        {
            Some((k, _)) => {
                used[k] = true;
                found += 1;
            }
            None => missed.push(format!("{want}@{at}")),
        }
    }
    let unexpected = events
        .iter()
        .zip(&used)
        .filter(|(_, u)| !**u)
        .map(|(x, _)| format!("{}@{}", x.kind, x.epoch))
        .collect();
    (found, missed, unexpected)
}

/// A SYNTHETIC sample injection file (one of each kind).
pub const SAMPLE_INJECT: &str = r#"format = "kshana-space-rx-inject/1"
resolution_m = 0.01
[[event]]
kind = "bitflip"
epoch = 60
axis = "y"
bit = 16
[[event]]
kind = "timetag"
epoch = 120
magnitude = 2.0
[[event]]
kind = "clock"
epoch = 180
magnitude = 900.0
[[event]]
kind = "freeze"
epoch = 240
length = 4
[[event]]
kind = "sefi"
epoch = 300
length = 8
[[event]]
kind = "reset"
epoch = 380
length = 20
[[event]]
kind = "latchup"
epoch = 470
length = 4
"#;
