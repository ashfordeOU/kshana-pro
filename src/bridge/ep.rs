// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! ECDIS estimated-position (EP) fallback tester (research 07 B02, 07b M06).
//!
//! When GNSS is lost an ECDIS is expected to indicate the loss and carry on with an
//! estimated position from gyro and log; a manually entered position should be kept and
//! dead-reckoned from; and a position that stops changing while still marked valid is a
//! fault the crew should be told about. This tester drives those cases with recorded
//! stimuli and checks what the ECDIS did, from a table of the positions and indications it
//! showed (exported, or read off a screen recording):
//!
//! * [`vectors`] build the stimuli through the open engine's training NMEA generator
//!   ([`kshana::nmea_synth`]): a GNSS loss on a straight leg, a loss across a turn, a
//!   frozen-but-valid position, and a loss with a manual position entered part-way. Each
//!   vector carries the reference EP (dead reckoning on the stimulus's own gyro and log,
//!   [`reference`]), the checkpoints and tolerances, and the true track.
//! * [`check`] compares an observed table (`t_s,lat_deg,lon_deg[,mode][,alarm]`) with the
//!   vector: indication of the loss within the deadline, EP within tolerance at every
//!   checkpoint, the manual position kept, a frozen position flagged, and the return to
//!   GNSS. Each criterion is PASS, FAIL or NOT ASSESSED (the table lacks the column).
//!
//! The criteria are measured against the behaviour IEC 61174 and the IMO ECDIS performance
//! standard describe for loss of position input; the deadlines and tolerances are this
//! tester's parameters, written into every vector, not figures quoted from those documents.
//! MODELLED (see [`super::LABEL`]).

use serde::{Deserialize, Serialize};

use super::nmea::{self, ReaderConfig, Role};
use super::{distance_m, move_ne, KN_MPS};

/// The vector's format tag.
pub const FORMAT: &str = "kshana-pro-ecdis-ep-vector/1";
/// The check result's format tag.
pub const RESULT_FORMAT: &str = "kshana-pro-ecdis-ep-check/1";

/// What a vector exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VectorKind {
    /// GNSS lost (no valid fix) for a window.
    Loss,
    /// GNSS position frozen but still marked valid for a window.
    Frozen,
    /// GNSS lost, and a position entered by hand part-way through.
    Manual,
}

/// A manual position the operator enters during the run.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ManualInput {
    pub t_s: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
}

/// One checkpoint: where the reference EP is, and how far the ECDIS may be from it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    pub t_s: f64,
    pub ref_lat_deg: f64,
    pub ref_lon_deg: f64,
    pub tolerance_m: f64,
    /// How far the reference itself is from the true position (dead-reckoning quality).
    pub ref_truth_error_m: Option<f64>,
}

/// A test vector.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Vector {
    pub format: String,
    pub id: String,
    pub title: String,
    pub kind: VectorKind,
    /// The window in which GNSS is lost or frozen, seconds from the start.
    pub window_start_s: f64,
    pub window_end_s: f64,
    /// The loss (or the frozen position) must be indicated within this many seconds.
    pub indication_deadline_s: f64,
    /// The ECDIS should be back on GNSS this long after the window ends.
    pub recovery_deadline_s: f64,
    pub manual: Option<ManualInput>,
    pub checkpoints: Vec<Checkpoint>,
    pub tolerance_floor_m: f64,
    pub tolerance_frac_of_run: f64,
    pub label: String,
    pub instructions: String,
}

/// A built-in vector definition.
struct Def {
    id: &'static str,
    title: &'static str,
    kind: VectorKind,
    scenario: &'static str,
    window: (f64, f64),
    manual_at: Option<f64>,
}

const STRAIGHT: &str = r#"
[scenario]
name = "ep-loss-straight"
description = "Straight leg at 12 kn; GNSS lost for ten minutes."
trainer_note = "ECDIS EP fallback stimulus. Synthetic; never feed it to live navigation systems."
seed = 41
start_utc = "2026-03-02T10:00:00Z"
duration_s = 1200
rate_hz = 1
[vessel]
lat_deg = 57.60
lon_deg = 11.20
speed_kn = 12
[[waypoint]]
lat_deg = 57.75
lon_deg = 11.60
speed_kn = 12
[environment]
current_speed_kn = 0.6
current_set_deg = 120
[output]
log_interval_s = 1
[[event]]
kind = "jamming"
label = "GNSS lost"
start_s = 300
duration_s = 600
ramp_s = 0
recovery_s = 0
cn0_drop_db = 45
"#;

const TURN: &str = r#"
[scenario]
name = "ep-loss-turn"
description = "GNSS lost across a 60 degree alteration at 10 kn."
trainer_note = "ECDIS EP fallback stimulus. Synthetic; never feed it to live navigation systems."
seed = 42
start_utc = "2026-03-02T11:00:00Z"
duration_s = 1500
rate_hz = 1
[vessel]
lat_deg = 57.60
lon_deg = 11.20
speed_kn = 10
[[waypoint]]
lat_deg = 57.62
lon_deg = 11.32
speed_kn = 10
[[waypoint]]
lat_deg = 57.70
lon_deg = 11.36
speed_kn = 10
[output]
log_interval_s = 1
[[event]]
kind = "jamming"
label = "GNSS lost"
start_s = 200
duration_s = 900
ramp_s = 0
recovery_s = 0
cn0_drop_db = 45
"#;

const DEFS: &[Def] = &[
    Def {
        id: "ep-loss-straight",
        title: "GNSS lost for ten minutes on a straight leg",
        kind: VectorKind::Loss,
        scenario: STRAIGHT,
        window: (300.0, 900.0),
        manual_at: None,
    },
    Def {
        id: "ep-loss-turn",
        title: "GNSS lost across an alteration of course",
        kind: VectorKind::Loss,
        scenario: TURN,
        window: (200.0, 1100.0),
        manual_at: None,
    },
    Def {
        id: "ep-frozen-position",
        title: "GNSS position frozen while still marked valid",
        kind: VectorKind::Frozen,
        scenario: STRAIGHT,
        window: (300.0, 900.0),
        manual_at: None,
    },
    Def {
        id: "ep-manual-position",
        title: "GNSS lost; a manual position entered after five minutes",
        kind: VectorKind::Manual,
        scenario: STRAIGHT,
        window: (300.0, 900.0),
        manual_at: Some(600.0),
    },
];

/// The ids of the built-in vectors.
pub fn ids() -> Vec<&'static str> {
    DEFS.iter().map(|d| d.id).collect()
}

/// Recompute the checksum of a sentence after editing its fields.
fn reframe(body: &str) -> String {
    format!(
        "${body}*{:02X}",
        kshana::interop::testbench::nmea_checksum(body)
    )
}

/// Replace the position fields of RMC and GGA sentences between `t0` and `t1` with those
/// of the epoch at `t0`, keeping them valid: a frozen receiver output.
fn freeze(epochs: &mut [kshana::nmea_synth::gen::Epoch], t0: f64, t1: f64) {
    let mut held: Vec<(String, Vec<String>)> = Vec::new();
    for e in epochs.iter_mut() {
        for l in e.lines.iter_mut() {
            let Some(body) = l.strip_prefix('$').and_then(|b| b.split('*').next()) else {
                continue;
            };
            let mut f: Vec<String> = body.split(',').map(str::to_string).collect();
            let kind = f[0]
                .get(f[0].len().saturating_sub(3)..)
                .unwrap_or("")
                .to_string();
            let range = match kind.as_str() {
                "RMC" => 2..10,
                "GGA" | "GNS" => 2..6,
                "VTG" => 1..10,
                _ => continue,
            };
            if (e.t_s - t0).abs() < 1e-9 {
                held.retain(|(k, _)| *k != kind);
                held.push((kind.clone(), f[range.clone()].to_vec()));
            } else if e.t_s > t0 && e.t_s < t1 {
                if let Some((_, h)) = held.iter().find(|(k, _)| *k == kind) {
                    // RMC keeps its own date (the last field of the held range).
                    let keep_date = kind == "RMC";
                    for (i, v) in range.clone().zip(h.iter()) {
                        if keep_date && i == 9 {
                            continue;
                        }
                        f[i] = v.clone();
                    }
                    *l = reframe(&f.join(","));
                }
            }
        }
    }
}

/// The reference EP: dead reckoning on the stimulus's gyro and log from the anchor (the
/// last valid GNSS position before `from_s`, or a given position), at every epoch from
/// `from_s`. Returns `(t_s, lat, lon, distance run)`.
pub fn reference(
    stimulus: &[u8],
    from_s: f64,
    anchor: Option<(f64, f64)>,
    to_s: f64,
) -> Result<Vec<(f64, f64, f64, f64)>, String> {
    let (epochs, sum) = nmea::read(stimulus, &ReaderConfig::default())?;
    let primary = sum.primary_gnss.unwrap_or_default();
    // The anchor: the given position at `from_s`, or the last valid GNSS fix at or before it.
    let mut anchor_t = from_s;
    let mut pos = anchor;
    if pos.is_none() {
        for e in epochs.iter().filter(|e| e.t_s <= from_s + 1e-9) {
            if let Some(g) = e.gnss.get(&primary).filter(|g| g.valid) {
                if let (Some(la), Some(lo)) = (g.lat_deg, g.lon_deg) {
                    pos = Some((la, lo));
                    anchor_t = e.t_s;
                }
            }
        }
    }
    let mut pos = pos.ok_or("no GNSS position before the window to start the reference from")?;
    let mut out = Vec::new();
    let mut last = anchor_t;
    let mut run = 0.0;
    for e in epochs
        .iter()
        .filter(|e| e.t_s > anchor_t + 1e-9 && e.t_s <= to_s + 1e-9)
    {
        let dt = e.t_s - last;
        last = e.t_s;
        let hdg = e
            .heading
            .values()
            .find(|(r, _)| *r == Role::Gyro)
            .map(|(_, h)| *h);
        let speed = e
            .ground
            .values()
            .next()
            .copied()
            .or_else(|| e.water.values().next().map(|(l, t)| (*l, t.unwrap_or(0.0))));
        if let (Some(h), Some((l, tr))) = (hdg, speed) {
            let (s, c) = h.to_radians().sin_cos();
            let (vl, vt) = (l * KN_MPS, tr * KN_MPS);
            let (vn, ve) = (vl * c - vt * s, vl * s + vt * c);
            pos = move_ne(pos.0, pos.1, vn * dt, ve * dt);
            run += vn.hypot(ve) * dt;
        }
        if e.t_s >= from_s - 1e-9 {
            out.push((e.t_s, pos.0, pos.1, run));
        }
    }
    Ok(out)
}

/// A generated vector: the vector, its stimulus and the generator's instructor log (truth).
pub struct Built {
    pub vector: Vector,
    pub stimulus: String,
    pub truth_json: String,
    pub reference_csv: String,
}

/// Build one built-in vector.
pub fn build(id: &str) -> Result<Built, String> {
    let d = DEFS
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("unknown EP vector '{id}' (known: {})", ids().join(", ")))?;
    let mut scn_text = d.scenario.to_string();
    if d.kind == VectorKind::Frozen {
        // No jamming: the receiver keeps a valid fix that the tester freezes.
        scn_text = scn_text
            .split("[[event]]")
            .next()
            .unwrap_or_default()
            .replace("ep-loss-straight", "ep-frozen-position");
    }
    let scn = kshana::nmea_synth::TrainingScenario::parse(&scn_text)?;
    let mut g = kshana::nmea_synth::generate(&scn)?;
    if d.kind == VectorKind::Frozen {
        freeze(&mut g.epochs, d.window.0, d.window.1);
    }
    let stimulus = g.nmea_text();
    let truth = &g.log.track;
    let truth_at = |t: f64| {
        truth
            .iter()
            .find(|r| (r.t_s - t).abs() < 1e-6)
            .map(|r| (r.true_lat_deg, r.true_lon_deg))
    };
    let (floor, frac): (f64, f64) = (50.0, 0.02);
    let mut checkpoints = Vec::new();
    let mut ref_rows = Vec::new();
    let mut add = |rows: Vec<(f64, f64, f64, f64)>, from: f64| {
        for (t, la, lo, run) in rows {
            ref_rows.push((t, la, lo));
            if t > from + 1e-9 && ((t - from) % 60.0).abs() < 1e-6 && t < d.window.1 {
                checkpoints.push(Checkpoint {
                    t_s: t,
                    ref_lat_deg: super::sig9(la),
                    ref_lon_deg: super::sig9(lo),
                    tolerance_m: super::sig9(floor.max(frac * run)),
                    ref_truth_error_m: truth_at(t)
                        .map(|(a, b)| super::sig9(distance_m(la, lo, a, b))),
                });
            }
        }
    };
    let mut manual = None;
    match (d.kind, d.manual_at) {
        (VectorKind::Manual, Some(tm)) => {
            add(
                reference(stimulus.as_bytes(), d.window.0, None, tm)?,
                d.window.0,
            );
            let (la, lo) = truth_at(tm).ok_or("no truth row at the manual entry time")?;
            manual = Some(ManualInput {
                t_s: tm,
                lat_deg: la,
                lon_deg: lo,
            });
            let mut after = reference(stimulus.as_bytes(), tm, Some((la, lo)), d.window.1)?;
            after.retain(|r| r.0 > tm);
            add(after, tm);
        }
        (VectorKind::Loss, _) => add(
            reference(stimulus.as_bytes(), d.window.0, None, d.window.1)?,
            d.window.0,
        ),
        _ => {}
    }
    let mut reference_csv = String::from("t_s,lat_deg,lon_deg,mode,alarm\n");
    // A table that passes every criterion: what a reference ECDIS would show.
    for e in &g.epochs {
        let t = e.t_s;
        let in_win = t >= d.window.0 && t < d.window.1;
        let (la, lo, mode, alarm) = if in_win && d.kind != VectorKind::Frozen {
            match ref_rows.iter().find(|r| (r.0 - t).abs() < 1e-6) {
                Some(r) => (r.1, r.2, "EP", 1),
                None => continue,
            }
        } else {
            let (la, lo) = truth_at(t).unwrap_or((f64::NAN, f64::NAN));
            if in_win {
                (la, lo, "GNSS", 1)
            } else {
                (la, lo, "GNSS", 0)
            }
        };
        if la.is_nan() {
            continue;
        }
        reference_csv.push_str(&format!("{t},{la:.7},{lo:.7},{mode},{alarm}\n"));
    }
    let vector = Vector {
        format: FORMAT.into(),
        id: d.id.into(),
        title: d.title.into(),
        kind: d.kind,
        window_start_s: d.window.0,
        window_end_s: d.window.1,
        indication_deadline_s: if d.kind == VectorKind::Frozen {
            60.0
        } else {
            10.0
        },
        recovery_deadline_s: 60.0,
        manual,
        checkpoints,
        tolerance_floor_m: floor,
        tolerance_frac_of_run: frac,
        label: super::LABEL.into(),
        instructions: format!(
            "Play stimulus.nmea into the ECDIS under test (a training or test installation, \
             never a live bridge) at 1 Hz from t = 0. {} Record the ECDIS own-ship position, \
             its position mode (GNSS, EP, DR, MANUAL) and whether a position alarm or \
             indication is shown, once a second, as CSV: t_s,lat_deg,lon_deg,mode,alarm. \
             Check it with `kshana-pro bridge ep-check <vector-dir> --observed <file>`.",
            match d.kind {
                VectorKind::Manual =>
                    "At the manual entry time in vector.json, enter that position by hand.",
                VectorKind::Frozen =>
                    "The GNSS position stops changing in the window while still marked valid.",
                VectorKind::Loss => "GNSS has no valid fix in the window.",
            }
        ),
    };
    Ok(Built {
        vector,
        stimulus,
        truth_json: g.log.to_json(),
        reference_csv,
    })
}

/// Write every built-in vector under `out/<id>/`.
pub fn write_all(out: &std::path::Path) -> Result<usize, String> {
    for id in ids() {
        let b = build(id)?;
        let d = out.join(id);
        std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        let w = |n: &str, t: &str| std::fs::write(d.join(n), t).map_err(|e| format!("{n}: {e}"));
        w(
            "vector.json",
            &(serde_json::to_string_pretty(&b.vector).map_err(|e| e.to_string())? + "\n"),
        )?;
        w("stimulus.nmea", &b.stimulus)?;
        w("truth.json", &b.truth_json)?;
        w("reference-observed.csv", &b.reference_csv)?;
    }
    Ok(ids().len())
}

/// One observed row.
#[derive(Clone, Debug, PartialEq)]
pub struct Observed {
    pub t_s: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub mode: Option<String>,
    pub alarm: Option<bool>,
}

/// Parse the observed table.
pub fn parse_observed(text: &str) -> Result<Vec<Observed>, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let head: Vec<String> = lines
        .next()
        .ok_or("empty observed table")?
        .split(',')
        .map(|h| h.trim().to_ascii_lowercase())
        .collect();
    let col = |n: &str| head.iter().position(|h| h == n);
    let (Some(ct), Some(ca), Some(co)) = (col("t_s"), col("lat_deg"), col("lon_deg")) else {
        return Err("the observed table needs columns t_s, lat_deg and lon_deg".into());
    };
    let (cm, cl) = (col("mode"), col("alarm"));
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let num = |c: usize| -> Result<f64, String> {
            f.get(c)
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("row {}: column {} is not a number", i + 2, head[c]))
        };
        out.push(Observed {
            t_s: num(ct)?,
            lat_deg: num(ca)?,
            lon_deg: num(co)?,
            mode: cm
                .and_then(|c| f.get(c))
                .filter(|v| !v.is_empty())
                .map(|v| v.to_ascii_uppercase()),
            alarm: cl.and_then(|c| f.get(c)).and_then(|v| match *v {
                "1" | "true" | "yes" | "TRUE" | "YES" => Some(true),
                "0" | "false" | "no" | "FALSE" | "NO" => Some(false),
                _ => None,
            }),
        });
    }
    out.sort_by(|a, b| a.t_s.total_cmp(&b.t_s));
    Ok(out)
}

/// The outcome of one criterion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Pass,
    Fail,
    NotAssessed,
}

/// One criterion's result.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Criterion {
    pub id: String,
    pub outcome: Outcome,
    pub detail: String,
}

/// A check result.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CheckResult {
    pub format: String,
    pub vector: String,
    pub kind: VectorKind,
    pub label: String,
    pub criteria: Vec<Criterion>,
    pub passed: bool,
}

fn indicated(o: &Observed) -> Option<bool> {
    match (&o.alarm, &o.mode) {
        (Some(true), _) => Some(true),
        (_, Some(m)) if m != "GNSS" => Some(true),
        (Some(false), _) | (_, Some(_)) => Some(false),
        (None, None) => None,
    }
}

fn nearest(obs: &[Observed], t: f64) -> Option<&Observed> {
    obs.iter()
        .filter(|o| (o.t_s - t).abs() <= 2.0)
        .min_by(|a, b| (a.t_s - t).abs().total_cmp(&(b.t_s - t).abs()))
}

/// Check an observed table against a vector.
pub fn check(v: &Vector, obs: &[Observed]) -> CheckResult {
    let mut crit = Vec::new();
    let has_ind = obs.iter().any(|o| indicated(o).is_some());
    // Indication of the loss (or the frozen position).
    let id = if v.kind == VectorKind::Frozen {
        "frozen-position-indicated"
    } else {
        "loss-indicated"
    };
    crit.push(if !has_ind {
        Criterion {
            id: id.into(),
            outcome: Outcome::NotAssessed,
            detail: "the observed table has no mode or alarm column".into(),
        }
    } else {
        let first = obs
            .iter()
            .filter(|o| o.t_s >= v.window_start_s && o.t_s < v.window_end_s)
            .find(|o| indicated(o) == Some(true));
        match first {
            Some(o) if o.t_s - v.window_start_s <= v.indication_deadline_s + 1e-9 => Criterion {
                id: id.into(),
                outcome: Outcome::Pass,
                detail: format!(
                    "indicated {:.0} s after the window opened (deadline {:.0} s)",
                    o.t_s - v.window_start_s,
                    v.indication_deadline_s
                ),
            },
            Some(o) => Criterion {
                id: id.into(),
                outcome: Outcome::Fail,
                detail: format!(
                    "indicated only {:.0} s after the window opened (deadline {:.0} s)",
                    o.t_s - v.window_start_s,
                    v.indication_deadline_s
                ),
            },
            None => Criterion {
                id: id.into(),
                outcome: Outcome::Fail,
                detail: if v.kind == VectorKind::Frozen {
                    "the frozen GNSS position was accepted with no indication".into()
                } else {
                    "no indication of the loss in the window".into()
                },
            },
        }
    });
    // EP at the checkpoints (manual: before and after the entry).
    if v.kind != VectorKind::Frozen {
        let split_at = v.manual.as_ref().map(|m| m.t_s).unwrap_or(f64::INFINITY);
        for (cid, sel) in [
            ("ep-within-tolerance", true),
            ("manual-position-retained", false),
        ] {
            if !sel && v.manual.is_none() {
                continue;
            }
            let cps: Vec<&Checkpoint> = v
                .checkpoints
                .iter()
                .filter(|c| (c.t_s <= split_at) == sel)
                .collect();
            let mut worst: Option<(f64, f64)> = None;
            let mut missing = 0;
            let mut over = 0;
            for c in &cps {
                match nearest(obs, c.t_s) {
                    Some(o) => {
                        let d = distance_m(o.lat_deg, o.lon_deg, c.ref_lat_deg, c.ref_lon_deg);
                        if d > c.tolerance_m {
                            over += 1;
                        }
                        if worst.is_none_or(|w| d / c.tolerance_m > w.0 / w.1) {
                            worst = Some((d, c.tolerance_m));
                        }
                    }
                    None => missing += 1,
                }
            }
            let outcome = if cps.is_empty() {
                Outcome::NotAssessed
            } else if over == 0 && missing == 0 {
                Outcome::Pass
            } else {
                Outcome::Fail
            };
            crit.push(Criterion {
                id: cid.into(),
                outcome,
                detail: format!(
                    "{} checkpoints, {over} beyond tolerance, {missing} without an observation; worst {}",
                    cps.len(),
                    worst
                        .map(|(d, t)| format!("{d:.0} m against {t:.0} m"))
                        .unwrap_or_else(|| "-".into())
                ),
            });
        }
    }
    // Back on GNSS after the window.
    crit.push(if !has_ind {
        Criterion {
            id: "recovered".into(),
            outcome: Outcome::NotAssessed,
            detail: "the observed table has no mode or alarm column".into(),
        }
    } else {
        let after = obs
            .iter()
            .filter(|o| o.t_s >= v.window_end_s)
            .find(|o| indicated(o) == Some(false));
        match after {
            Some(o) if o.t_s - v.window_end_s <= v.recovery_deadline_s + 1e-9 => Criterion {
                id: "recovered".into(),
                outcome: Outcome::Pass,
                detail: format!(
                    "back on GNSS {:.0} s after the window",
                    o.t_s - v.window_end_s
                ),
            },
            _ => Criterion {
                id: "recovered".into(),
                outcome: Outcome::Fail,
                detail: format!(
                    "not back on GNSS within {:.0} s of the window",
                    v.recovery_deadline_s
                ),
            },
        }
    });
    let passed = crit.iter().all(|c| c.outcome != Outcome::Fail)
        && crit.iter().any(|c| c.outcome == Outcome::Pass);
    CheckResult {
        format: RESULT_FORMAT.into(),
        vector: v.id.clone(),
        kind: v.kind,
        label: super::LABEL.into(),
        criteria: crit,
        passed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_table_passes_and_a_late_or_wrong_one_fails() {
        for id in ids() {
            let b = build(id).unwrap();
            let obs = parse_observed(&b.reference_csv).unwrap();
            let r = check(&b.vector, &obs);
            assert!(r.passed, "{id}: {:?}", r.criteria);
        }
        let b = build("ep-loss-straight").unwrap();
        assert!(b.vector.checkpoints.len() >= 9);
        // The reference EP stays close to truth on a ten-minute loss (ground-tracking log).
        for c in &b.vector.checkpoints {
            assert!(c.ref_truth_error_m.unwrap() < 60.0, "{c:?}");
        }
        // An ECDIS that freezes its last GNSS position instead of dead-reckoning.
        let mut obs = parse_observed(&b.reference_csv).unwrap();
        let hold = obs.iter().find(|o| o.t_s == 299.0).cloned().unwrap();
        for o in obs.iter_mut().filter(|o| o.t_s >= 300.0 && o.t_s < 900.0) {
            o.lat_deg = hold.lat_deg;
            o.lon_deg = hold.lon_deg;
        }
        let r = check(&b.vector, &obs);
        assert!(!r.passed);
        assert_eq!(
            r.criteria
                .iter()
                .find(|c| c.id == "ep-within-tolerance")
                .unwrap()
                .outcome,
            Outcome::Fail
        );
    }

    #[test]
    fn a_frozen_position_with_no_indication_fails() {
        let b = build("ep-frozen-position").unwrap();
        // The stimulus really freezes: the position at 600 s equals that at 300 s.
        let (ep, _) = nmea::read(b.stimulus.as_bytes(), &ReaderConfig::default()).unwrap();
        let p = |t: f64| {
            let g = &ep.iter().find(|e| e.t_s == t).unwrap().gnss["GN"];
            (g.lat_deg.unwrap(), g.lon_deg.unwrap(), g.valid)
        };
        assert_eq!(p(300.0), p(600.0));
        assert!(p(600.0).2);
        assert_ne!(p(300.0), p(1000.0));
        let obs: Vec<Observed> = parse_observed(&b.reference_csv)
            .unwrap()
            .into_iter()
            .map(|mut o| {
                o.alarm = Some(false);
                o
            })
            .collect();
        let r = check(&b.vector, &obs);
        assert!(!r.passed);
        assert!(r.criteria[0].detail.contains("accepted"));
    }

    #[test]
    fn missing_columns_are_not_assessed() {
        let b = build("ep-loss-straight").unwrap();
        let text: String = b
            .reference_csv
            .lines()
            .map(|l| l.splitn(4, ',').take(3).collect::<Vec<_>>().join(",") + "\n")
            .collect();
        let r = check(&b.vector, &parse_observed(&text).unwrap());
        assert_eq!(r.criteria[0].outcome, Outcome::NotAssessed);
        assert!(r.passed, "EP still checked: {:?}", r.criteria);
        assert!(parse_observed("a,b\n1,2").is_err());
    }
}
