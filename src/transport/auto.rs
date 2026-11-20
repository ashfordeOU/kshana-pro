// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Automotive GNSS evidence (backlog B3001, first slice)
//!
//! Research 2026-11-20 gaps 07:I01, I02, I09, I12 and 08:D11-D13, E1-E3.
//!
//! * [`CATALOGUE`]: a SOTIF (ISO 21448) triggering-condition catalogue for automotive GNSS
//!   positioning: each condition with the functional insufficiency it can reveal, its effect
//!   on a protection level, and the data a validation needs. Kshana's own catalogue, written
//!   for this work; it quotes no standard text.
//! * [`validate_pl`]: protection-level validation per triggering condition: Stanford-diagram
//!   counts, the 95 % upper bound on the misleading-information rate from an effective
//!   sample size, the independent samples still needed for a target integrity risk, and
//!   catalogue coverage (which conditions have data, which have none).
//! * [`v2x`]: V2X position and time checks over a decoded CAM log with recorded attack
//!   windows: whether the transmitted 95 % position-confidence ellipse held the true
//!   position outside and inside the attacks, generation-time error, detection latency and
//!   false flags. Expected behaviours only: nothing here generates a spoofing signal.
//! * [`inject`]: a data-level fault-injection library for recorded position output (step,
//!   ramp, freeze, dropout, noise, time jump, PL scaling), to test a downstream integrity
//!   monitor. It edits a recorded file; it generates no signal.
//!
//! Not in this slice: ISO 26262 SEooC assessor tools beyond the evidence catalogue
//! (`regpack assess --catalogue iso26262-gnss-seooc`), statistical scenario-coverage
//! sampling, and correction-service ("cloud") integrity models.
//!
//! **Label: MODELLED.** Statistics of the records as supplied.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::integrity::{self, Epoch, Stanford};
use super::{en_m, offset_deg, quantile, wilson95, Table};

pub const FEATURE_PL: &str = "pro-automotive-pl";
pub const FEATURE_V2X: &str = "pro-v2x-checks";
pub const FEATURE_INJECT: &str = "pro-fault-injection";
pub const PL_FORMAT: &str = "kshana-pro-pl-validation/1";
pub const V2X_FORMAT: &str = "kshana-pro-v2x-checks/1";
pub const LABEL: &str = "MODELLED: statistics of the records as supplied (logged errors against a reference, protection levels and decoded messages). Evidence for the safety case; whether a SOTIF or ISO 26262 argument is acceptable is for the assessor.";

/// One triggering condition.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Condition {
    pub id: &'static str,
    pub condition: &'static str,
    pub insufficiency: &'static str,
    pub effect: &'static str,
    pub data: &'static str,
}

/// The GNSS triggering-condition catalogue.
pub const CATALOGUE: &[Condition] = &[
    Condition {
        id: "open-sky",
        condition: "Open sky, nominal constellation",
        insufficiency: "Error model tails (overbound) in the best case",
        effect: "Baseline MI rate and availability",
        data: "Long open-road drives with a reference trajectory",
    },
    Condition {
        id: "urban-canyon",
        condition: "Urban canyon, NLOS and multipath",
        insufficiency: "Measurement error model assumes line of sight",
        effect: "Errors larger than the PL with plausible geometry",
        data: "City-centre drives, dense buildings, reference INS/RTK trajectory",
    },
    Condition {
        id: "foliage",
        condition: "Tree canopy and seasonal foliage",
        insufficiency: "Attenuation and diffraction not in the noise model",
        effect: "Reduced C/N0, cycle slips, PL inflation or MI",
        data: "Forest roads in leaf and out of leaf",
    },
    Condition {
        id: "tunnel-exit",
        condition: "Tunnel and underpass exits",
        insufficiency: "Re-convergence after outage; stale ambiguities",
        effect: "Large error immediately after the outage",
        data: "Tunnel exits with timing of first fixes",
    },
    Condition {
        id: "bridge-overpass",
        condition: "Bridges, overpasses, gantries",
        insufficiency: "Short blockage and reflection from metal structures",
        effect: "Short error spikes",
        data: "Motorway gantries and stacked interchanges",
    },
    Condition {
        id: "parking-structure",
        condition: "Multi-storey car parks and depots",
        insufficiency: "Indoor/outdoor transitions",
        effect: "Position held from before entry",
        data: "Entry and exit sequences",
    },
    Condition {
        id: "iono-scintillation",
        condition: "Ionospheric scintillation and storms",
        insufficiency: "Ionospheric model and correction latency",
        effect: "Range errors beyond the broadcast bounds",
        data: "High-latitude or equatorial drives on disturbed days",
    },
    Condition {
        id: "correction-outage",
        condition: "Loss or latency of correction data",
        insufficiency: "Correction age and integrity messaging",
        effect: "Degraded accuracy with an optimistic PL",
        data: "Drives with induced correction-link loss",
    },
    Condition {
        id: "correction-fault",
        condition: "Faulty correction data from the service",
        insufficiency: "Service integrity monitoring and time to alert",
        effect: "Common-mode error across vehicles",
        data: "Replayed faulty correction streams (fault injection)",
    },
    Condition {
        id: "satellite-fault",
        condition: "Satellite clock or ephemeris fault",
        insufficiency: "Fault detection within the time to alert",
        effect: "Ramp or step on one satellite's range",
        data: "Simulated faults on recorded or simulated measurements",
    },
    Condition {
        id: "jamming",
        condition: "Radio-frequency interference and jamming (personal privacy devices)",
        insufficiency: "C/N0 loss handling and coasting",
        effect: "Outage, then degraded fixes at the edges",
        data: "Conducted or shielded laboratory runs, sanctioned field events",
    },
    Condition {
        id: "spoofing",
        condition: "Spoofing and meaconing",
        insufficiency: "No authentication or consistency check against vehicle sensors",
        effect: "Wrong position with a small PL",
        data: "Conducted or shielded laboratory runs, sanctioned field events",
    },
    Condition {
        id: "high-dynamics",
        condition: "Hard braking, sharp turns, high speed",
        insufficiency: "Tracking loop stress and motion model",
        effect: "Lag errors and loss of lock",
        data: "Proving-ground manoeuvres",
    },
    Condition {
        id: "antenna-installation",
        condition: "Antenna placement, roof racks, windscreen coatings",
        insufficiency: "Installed antenna gain pattern and phase-centre variation",
        effect: "Elevation-dependent biases",
        data: "Vehicle variants and accessory configurations",
    },
];

/// Protection-level validation request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlRequest {
    pub title: String,
    /// CSV: `t`, `err_m`, `pl_m`, optional `al_m`, `condition`.
    pub data: String,
    /// Alert limit when the CSV has no `al_m` (m).
    pub al_m: Option<f64>,
    /// Decorrelation time of the errors (s); the effective sample size is `n·Δt/τ`.
    pub decorrelation_s: f64,
    /// Target per-sample integrity risk the claim is about.
    pub target_ir: Option<f64>,
    /// Conditions that must have data; default every catalogue condition.
    pub required: Option<Vec<String>>,
}

/// One condition's row.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConditionRow {
    pub id: String,
    pub in_catalogue: bool,
    pub stats: Option<Stanford>,
    /// `no data`, `MI observed`, `supports the target`, `more data needed` or `no target`.
    pub status: String,
    pub more_samples_needed: Option<f64>,
}

/// The validation report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub dt_s: f64,
    pub decorrelation_s: f64,
    pub target_ir: Option<f64>,
    pub samples_needed: Option<f64>,
    pub overall: Stanford,
    pub conditions: Vec<ConditionRow>,
    pub coverage: f64,
    pub verdict: String,
}

fn cond_status(s: &Stanford, target: Option<f64>, need: Option<f64>) -> (String, Option<f64>) {
    if s.mi + s.hmi > 0 {
        return ("MI observed".into(), None);
    }
    match (target, s.mi_rate_upper95, need) {
        (None, _, _) => ("no target".into(), None),
        (Some(t), Some(u), _) if u <= t => ("supports the target".into(), None),
        (Some(_), _, Some(n)) => (
            "more data needed".into(),
            Some((n - s.effective_samples).max(0.0).ceil()),
        ),
        _ => ("more data needed".into(), None),
    }
}

/// Validate protection levels by triggering condition.
pub fn validate_pl(req: &PlRequest, base: &Path) -> Result<PlReport, String> {
    if !super::pos(req.decorrelation_s) {
        return Err("decorrelation_s must be > 0".into());
    }
    let t = Table::read(&base.join(&req.data))?;
    let (ct, ce, cp) = (t.need("t")?, t.need("err_m")?, t.need("pl_m")?);
    let (ca, cc) = (t.col("al_m"), t.col("condition"));
    let mut times = Vec::new();
    let mut by: BTreeMap<String, Vec<Epoch>> = BTreeMap::new();
    let mut all = Vec::new();
    for i in 0..t.rows.len() {
        let (Some(tt), Some(e), Some(p)) = (
            t.num(i, Some(ct))?,
            t.num(i, Some(ce))?,
            t.num(i, Some(cp))?,
        ) else {
            continue;
        };
        let al = t
            .num(i, ca)?
            .or(req.al_m)
            .ok_or("no alert limit: give al_m or an al_m column")?;
        let ep = Epoch {
            err_m: e,
            pl_m: p,
            al_m: al,
        };
        times.push(tt);
        all.push(ep);
        let c = match t.text(i, cc) {
            "" => "unlabelled",
            c => c,
        };
        by.entry(c.to_string()).or_default().push(ep);
    }
    if all.is_empty() {
        return Err("no epochs".into());
    }
    times.sort_by(f64::total_cmp);
    let dts: Vec<f64> = times
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|d| *d > 0.0)
        .collect();
    let dt = quantile(&dts, 0.5).unwrap_or(1.0);
    let need = req.target_ir.and_then(integrity::samples_needed);
    let required: Vec<String> = req
        .required
        .clone()
        .unwrap_or_else(|| CATALOGUE.iter().map(|c| c.id.to_string()).collect());
    let mut rows = Vec::new();
    for id in &required {
        let st = by
            .get(id)
            .map(|v| integrity::stanford(v, dt, req.decorrelation_s));
        let (status, more) = match &st {
            None => ("no data".to_string(), need),
            Some(s) => cond_status(s, req.target_ir, need),
        };
        rows.push(ConditionRow {
            in_catalogue: CATALOGUE.iter().any(|c| c.id == id),
            id: id.clone(),
            stats: st,
            status,
            more_samples_needed: more,
        });
    }
    for (id, v) in &by {
        if !required.contains(id) {
            let s = integrity::stanford(v, dt, req.decorrelation_s);
            let (status, more) = cond_status(&s, req.target_ir, need);
            rows.push(ConditionRow {
                in_catalogue: CATALOGUE.iter().any(|c| c.id == id),
                id: id.clone(),
                stats: Some(s),
                status,
                more_samples_needed: more,
            });
        }
    }
    let covered = required.iter().filter(|r| by.contains_key(*r)).count();
    let coverage = covered as f64 / required.len().max(1) as f64;
    let overall = integrity::stanford(&all, dt, req.decorrelation_s);
    let mi_conds: Vec<&str> = rows
        .iter()
        .filter(|r| r.status == "MI observed")
        .map(|r| r.id.as_str())
        .collect();
    let verdict = format!(
        "{} epochs, availability {:.3}; MI {} and HMI {}{}; {covered} of {} required conditions have data{}",
        overall.epochs,
        overall.availability,
        overall.mi,
        overall.hmi,
        if mi_conds.is_empty() { String::new() } else { format!(" (in {})", mi_conds.join(", ")) },
        required.len(),
        match (req.target_ir, overall.mi_rate_upper95) {
            (Some(t), Some(u)) => format!(
                "; overall MI-rate 95 % bound {u:.2e} against target {t:.1e} ({})",
                if u <= t && overall.mi + overall.hmi == 0 { "data support it" } else { "data do not support it yet" }
            ),
            _ => String::new(),
        }
    );
    Ok(PlReport {
        format: PL_FORMAT,
        label: LABEL,
        title: req.title.clone(),
        dt_s: dt,
        decorrelation_s: req.decorrelation_s,
        target_ir: req.target_ir,
        samples_needed: need,
        overall,
        conditions: rows,
        coverage,
        verdict,
    })
}

/// The validation page.
pub fn pl_html(r: &PlReport) -> String {
    use super::{f3, fo};
    let rows = r
        .conditions
        .iter()
        .map(|c| {
            let s = c.stats.as_ref();
            vec![
                c.id.clone(),
                if c.in_catalogue { "yes" } else { "no" }.into(),
                s.map(|s| s.epochs.to_string())
                    .unwrap_or_else(|| "0".into()),
                s.map(|s| f3(s.availability)).unwrap_or_else(|| "-".into()),
                s.map(|s| format!("{} / {}", s.mi, s.hmi))
                    .unwrap_or_else(|| "-".into()),
                fo(s.and_then(|s| s.mi_rate_upper95)),
                c.status.clone(),
                fo(c.more_samples_needed),
            ]
        })
        .collect();
    super::page(
        &format!("Protection-level validation: {}", r.title),
        r.label,
        &r.verdict,
        &[format!(
            "Effective samples use Δt {} s and decorrelation {} s.",
            f3(r.dt_s),
            f3(r.decorrelation_s)
        )],
        &[(
            "Triggering conditions".into(),
            vec![
                "Condition",
                "In catalogue",
                "Epochs",
                "Availability",
                "MI / HMI",
                "MI rate 95 % bound",
                "Status",
                "More samples needed",
            ],
            rows,
        )],
    )
}

// ---------------------------------------------------------------------------------------
// V2X checks.

/// One recorded attack window.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Attack {
    pub kind: String,
    pub start: f64,
    pub end: f64,
}

/// The V2X request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V2xRequest {
    pub title: String,
    pub unit: String,
    /// CSV: `t`, `gen_time`, `lat`, `lon`, `truth_lat`, `truth_lon`, `conf_major_m`,
    /// `conf_minor_m`, `conf_orient_deg`, optional `spoof_flag`.
    pub log: String,
    #[serde(default)]
    pub attack: Vec<Attack>,
    /// Generation-time tolerance (s), stated by the applicant.
    pub time_tol_s: f64,
    /// Detection limit after an attack starts (s), stated by the applicant.
    pub detect_within_s: Option<f64>,
}

/// One check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct V2xCheck {
    pub id: String,
    pub title: String,
    pub messages: usize,
    pub value: Option<f64>,
    pub interval95: Option<(f64, f64)>,
    pub expected: String,
    pub status: String,
}

/// The V2X report.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct V2xReport {
    pub format: &'static str,
    pub label: &'static str,
    pub title: String,
    pub unit: String,
    pub ellipse_basis: &'static str,
    pub checks: Vec<V2xCheck>,
    pub verdict: String,
}

pub const ELLIPSE_BASIS: &str = "CAM position confidence ellipse read as a 95 % confidence region (ETSI TS 102 894-2 common data dictionary, PosConfidenceEllipse; to confirm against the version the unit implements).";

/// Whether the true position lies within the confidence ellipse around the reported one.
pub fn in_ellipse(err_e: f64, err_n: f64, major: f64, minor: f64, orient_deg: f64) -> bool {
    let th = orient_deg.to_radians();
    let a = err_e * th.sin() + err_n * th.cos();
    let b = err_e * th.cos() - err_n * th.sin();
    if major <= 0.0 || minor <= 0.0 {
        return false;
    }
    (a / major).powi(2) + (b / minor).powi(2) <= 1.0
}

/// Run the V2X checks.
pub fn v2x(req: &V2xRequest, base: &Path) -> Result<V2xReport, String> {
    if !super::pos(req.time_tol_s) {
        return Err("time_tol_s must be > 0".into());
    }
    let t = Table::read(&base.join(&req.log))?;
    let c: Vec<usize> = [
        "t",
        "gen_time",
        "lat",
        "lon",
        "truth_lat",
        "truth_lon",
        "conf_major_m",
        "conf_minor_m",
        "conf_orient_deg",
    ]
    .iter()
    .map(|n| t.need(n))
    .collect::<Result<_, _>>()?;
    let cf = t.col("spoof_flag");
    let under = |x: f64| req.attack.iter().any(|a| x >= a.start && x <= a.end);
    let (mut in_c, mut n_c, mut in_a, mut n_a) = (0, 0, 0, 0);
    let (mut tok_c, mut tok_a) = (0, 0);
    let mut false_flags = 0;
    let mut first_flag: Vec<Option<f64>> = vec![None; req.attack.len()];
    let (mut t0, mut t1) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..t.rows.len() {
        let g = |k: usize| t.num(i, Some(c[k]));
        let (
            Some(tt),
            Some(gt),
            Some(la),
            Some(lo),
            Some(tla),
            Some(tlo),
            Some(ma),
            Some(mi),
            Some(or),
        ) = (
            g(0)?,
            g(1)?,
            g(2)?,
            g(3)?,
            g(4)?,
            g(5)?,
            g(6)?,
            g(7)?,
            g(8)?,
        )
        else {
            continue;
        };
        t0 = t0.min(tt);
        t1 = t1.max(tt);
        let (e, n) = en_m(la, lo, tla, tlo);
        let inside = in_ellipse(e, n, ma, mi, or);
        let time_ok = (gt - tt).abs() <= req.time_tol_s;
        let flag = matches!(t.text(i, cf), "1" | "true" | "yes");
        if under(tt) {
            n_a += 1;
            in_a += usize::from(inside);
            tok_a += usize::from(time_ok);
            for (k, a) in req.attack.iter().enumerate() {
                if flag && tt >= a.start && tt <= a.end && first_flag[k].is_none() {
                    first_flag[k] = Some(tt - a.start);
                }
            }
        } else {
            n_c += 1;
            in_c += usize::from(inside);
            tok_c += usize::from(time_ok);
            false_flags += usize::from(flag);
        }
    }
    if n_c + n_a == 0 {
        return Err("no complete messages in the log".into());
    }
    let share = |k: usize, n: usize| (n > 0).then(|| k as f64 / n as f64);
    let mut checks = Vec::new();
    let w_c = wilson95(in_c, n_c);
    checks.push(V2xCheck {
        id: "V1".into(),
        title: "True position inside the transmitted confidence ellipse, outside attacks".into(),
        messages: n_c,
        value: share(in_c, n_c),
        interval95: w_c,
        expected: "at least 0.95 (the ellipse's stated confidence)".into(),
        status: match w_c {
            None => "not assessed: no messages".into(),
            Some((_, hi)) if hi < 0.95 => {
                "ellipse optimistic: the 95 % interval lies below 0.95".into()
            }
            Some(_) => "consistent with 95 % confidence".into(),
        },
    });
    checks.push(V2xCheck {
        id: "V2".into(),
        title: "True position outside the transmitted ellipse during attacks (misleading confidence)".into(),
        messages: n_a,
        value: share(n_a - in_a, n_a),
        interval95: wilson95(n_a - in_a, n_a),
        expected: "low; a unit that knows it is attacked should enlarge its ellipse or stop claiming confidence".into(),
        status: if n_a == 0 { "not assessed: no attack messages".into() } else { "reported".into() },
    });
    checks.push(V2xCheck {
        id: "V3".into(),
        title: format!(
            "Generation time within {} s of true time (outside / during attacks)",
            req.time_tol_s
        ),
        messages: n_c + n_a,
        value: share(tok_c + tok_a, n_c + n_a),
        interval95: None,
        expected: format!(
            "outside attacks {} ; during attacks {}",
            super::fo(share(tok_c, n_c)),
            super::fo(share(tok_a, n_a))
        ),
        status: if tok_a < n_a {
            "time misled during an attack".into()
        } else {
            "time held".into()
        },
    });
    let lat: Vec<f64> = first_flag.iter().flatten().copied().collect();
    let det = lat.len();
    checks.push(V2xCheck {
        id: "V4".into(),
        title: "Attacks flagged by the unit, and time to flag".into(),
        messages: req.attack.len(),
        value: quantile(&lat, 0.5),
        interval95: None,
        expected: match req.detect_within_s {
            Some(l) => format!("every attack flagged within {l} s"),
            None => "every attack flagged (no time limit stated)".into(),
        },
        status: if req.attack.is_empty() {
            "not assessed: no attacks recorded".into()
        } else if det < req.attack.len() {
            format!("{det} of {} attacks flagged", req.attack.len())
        } else if req
            .detect_within_s
            .is_some_and(|l| lat.iter().any(|x| *x > l))
        {
            "every attack flagged, some late".into()
        } else {
            "every attack flagged in time".into()
        },
    });
    let clean_h =
        ((t1 - t0) - req.attack.iter().map(|a| a.end - a.start).sum::<f64>()).max(0.0) / 3600.0;
    checks.push(V2xCheck {
        id: "V5".into(),
        title: "Spoofing flags outside attacks (per hour of clean messages)".into(),
        messages: n_c,
        value: (clean_h > 0.0).then(|| false_flags as f64 / clean_h),
        interval95: None,
        expected: "none".into(),
        status: format!("{false_flags} false flag(s)"),
    });
    let verdict = format!(
        "{}: V1 {}; V2 misleading share {}; V4 {}; V5 {}",
        req.unit,
        checks[0].status,
        super::fo(checks[1].value),
        checks[3].status,
        checks[4].status
    );
    Ok(V2xReport {
        format: V2X_FORMAT,
        label: LABEL,
        title: req.title.clone(),
        unit: req.unit.clone(),
        ellipse_basis: ELLIPSE_BASIS,
        checks,
        verdict,
    })
}

/// The V2X page.
pub fn v2x_html(r: &V2xReport) -> String {
    let rows = r
        .checks
        .iter()
        .map(|c| {
            vec![
                c.id.clone(),
                c.title.clone(),
                c.messages.to_string(),
                super::fo(c.value),
                c.interval95
                    .map(|(a, b)| format!("{a:.3}-{b:.3}"))
                    .unwrap_or_else(|| "-".into()),
                c.expected.clone(),
                c.status.clone(),
            ]
        })
        .collect();
    super::page(
        &format!("V2X position and time checks: {}", r.title),
        r.label,
        &r.verdict,
        &[r.ellipse_basis.into()],
        &[(
            "Checks".into(),
            vec![
                "Check",
                "Title",
                "Messages",
                "Value",
                "95 % interval",
                "Expected",
                "Status",
            ],
            rows,
        )],
    )
}

// ---------------------------------------------------------------------------------------
// Fault injection.

/// One fault.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    /// `step`, `ramp`, `freeze`, `dropout`, `noise`, `time_jump` or `pl_scale`.
    pub kind: String,
    pub start: f64,
    pub end: Option<f64>,
    #[serde(default)]
    pub east_m: f64,
    #[serde(default)]
    pub north_m: f64,
    #[serde(default)]
    pub rate_mps: f64,
    #[serde(default)]
    pub bearing_deg: f64,
    #[serde(default)]
    pub sigma_m: f64,
    #[serde(default)]
    pub dt_s: f64,
    #[serde(default = "one")]
    pub factor: f64,
}

fn one() -> f64 {
    1.0
}

/// The injection request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InjectRequest {
    /// CSV with `t`, `lat`, `lon`, optional `pl_m`.
    pub input: String,
    #[serde(default)]
    pub seed: u64,
    pub fault: Vec<Fault>,
}

pub const FAULT_KINDS: &[&str] = &[
    "step",
    "ramp",
    "freeze",
    "dropout",
    "noise",
    "time_jump",
    "pl_scale",
];

fn gauss(state: &mut u64) -> f64 {
    let mut next = || {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    };
    let (u1, u2) = (next(), next());
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// Apply the faults; returns the faulted CSV (with the original position as `truth_lat`,
/// `truth_lon`, the active faults and the injected offset) and the epochs touched.
pub fn inject(req: &InjectRequest, base: &Path) -> Result<(String, usize), String> {
    for f in &req.fault {
        if !FAULT_KINDS.contains(&f.kind.as_str()) {
            return Err(format!(
                "unknown fault kind '{}'; known: {}",
                f.kind,
                FAULT_KINDS.join(", ")
            ));
        }
        if f.end.is_some_and(|e| e <= f.start) {
            return Err(format!("fault {}: end must be after start", f.kind));
        }
    }
    let t = Table::read(&base.join(&req.input))?;
    let (ct, cla, clo) = (t.need("t")?, t.need("lat")?, t.need("lon")?);
    let cpl = t.col("pl_m");
    let mut out = String::from("# kshana-pro auto inject: data-level faults on recorded output; no signal is generated\nt,lat,lon,pl_m,truth_lat,truth_lon,offset_e_m,offset_n_m,faults\n");
    let mut rng = req.seed ^ 0x6b73_6861_6e61;
    let mut frozen: Vec<Option<(f64, f64)>> = vec![None; req.fault.len()];
    let mut touched = 0;
    for i in 0..t.rows.len() {
        let tt = t
            .num(i, Some(ct))?
            .ok_or(format!("row {}: empty t", i + 2))?;
        let (Some(la), Some(lo)) = (t.num(i, Some(cla))?, t.num(i, Some(clo))?) else {
            continue;
        };
        let mut pl = t.num(i, cpl)?;
        let (mut lat, mut lon, mut t_out) = (la, lo, tt);
        let mut active = Vec::new();
        let mut drop = false;
        for (k, f) in req.fault.iter().enumerate() {
            let on = tt >= f.start && f.end.is_none_or(|e| tt < e);
            if !on {
                continue;
            }
            active.push(f.kind.as_str());
            match f.kind.as_str() {
                "step" => (lat, lon) = offset_deg(lat, lon, f.east_m, f.north_m),
                "ramp" => {
                    let d = f.rate_mps * (tt - f.start);
                    let b = f.bearing_deg.to_radians();
                    (lat, lon) = offset_deg(lat, lon, d * b.sin(), d * b.cos());
                }
                "freeze" => {
                    let fz = *frozen[k].get_or_insert((lat, lon));
                    (lat, lon) = fz;
                }
                "dropout" => drop = true,
                "noise" => {
                    let (e, n) = (f.sigma_m * gauss(&mut rng), f.sigma_m * gauss(&mut rng));
                    (lat, lon) = offset_deg(lat, lon, e, n);
                }
                "time_jump" => t_out += f.dt_s,
                _ => pl = pl.map(|p| p * f.factor),
            }
        }
        if !active.is_empty() {
            touched += 1;
        }
        let (e, n) = en_m(la, lo, lat, lon);
        let (sl, so) = if drop {
            (String::new(), String::new())
        } else {
            (format!("{lat:.9}"), format!("{lon:.9}"))
        };
        out.push_str(&format!(
            "{t_out},{sl},{so},{},{la:.9},{lo:.9},{e:.3},{n:.3},{}\n",
            pl.map(|p| format!("{p}")).unwrap_or_default(),
            active.join("+")
        ));
    }
    Ok((out, touched))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipse_orientation() {
        // Major axis east-west (orientation 90 degrees): 9 m east is inside a 10 x 2 ellipse.
        assert!(in_ellipse(9.0, 0.0, 10.0, 2.0, 90.0));
        assert!(!in_ellipse(0.0, 9.0, 10.0, 2.0, 90.0));
        assert!(in_ellipse(0.0, 9.0, 10.0, 2.0, 0.0));
    }

    #[test]
    fn catalogue_ids_unique() {
        let mut ids: Vec<&str> = CATALOGUE.iter().map(|c| c.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), CATALOGUE.len());
        assert!(CATALOGUE.len() >= 12);
    }
}
