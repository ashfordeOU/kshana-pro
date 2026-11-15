// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! A timing fleet and its assessment (roadmap 0.42 T1): every clock of a mixed-vendor fleet,
//! each read from its own I2 source through the open engine's clock-log readers
//! ([`kshana::clock_log`]: TICC, TimeLab, RINEX clock, CGGTTS, linuxptp `ptp4l` / `phc2sys` /
//! `ts2phc`, chrony, ntpd, White Rabbit, Linux PPS), modelled one by one
//! ([`super::model::HoldoverModel`]) and given a time to breach for every user-set limit.
//!
//! No limit is built in: each is a value the operator states (for example 100 us in the style
//! of an RTS 25 limit, or 1.1 us in the style of a PRTC-A limit). Using one asserts nothing
//! about a standard.

use std::path::Path;

use kshana::api::KshanaError;
use kshana::clock_log::{ClockLogInput, NormalisedPhase};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::model::{
    class_noise, fit_noise, HoldoverModel, Noise, NoiseSource, Prediction, CLASS_NAMES,
};

/// The format tag of an assessment document.
pub const ASSESSMENT_FORMAT: &str = "kshana-pro-timing-fleet/1";

/// A limit the operator sets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limit {
    pub name: String,
    /// The largest acceptable |time error| (s).
    pub value_s: f64,
}

/// One clock of the fleet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockSpec {
    pub id: String,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// `grandmaster`, `boundary`, `server`, `client`, `reference`, ... (free text).
    #[serde(default)]
    pub role: Option<String>,
    /// A clock class prior (`tcxo`, `ocxo`, `rafs`, `csac`, `uso`, `dsac`).
    #[serde(default)]
    pub class: Option<String>,
    /// Datasheet Allan deviation points `[[tau_s, adev], ...]`, a prior that wins over `class`.
    #[serde(default)]
    pub adev_prior: Option<Vec<[f64; 2]>>,
    /// `free-running` or `disciplined`; the default follows the source format.
    #[serde(default)]
    pub record: Option<String>,
    /// Names of the fleet limits that apply to this clock (default: all).
    #[serde(default)]
    pub limits: Option<Vec<String>>,
    /// Handover time, seconds from the first sample of the record (default: its last sample).
    #[serde(default)]
    pub holdover_start_s: Option<f64>,
    /// The clock's I2 source (the open engine's clock-log input).
    pub source: ClockLogInput,
}

/// A fleet scenario (`kind = "pro-timing-holdover"`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FleetSpec {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub fleet: Option<String>,
    /// Training window (h), default 48.
    #[serde(default)]
    pub window_h: Option<f64>,
    /// Prediction horizon (h), default 48.
    #[serde(default)]
    pub horizon_h: Option<f64>,
    #[serde(default)]
    pub limits: Vec<Limit>,
    #[serde(default)]
    pub clocks: Vec<ClockSpec>,
}

/// Formats whose record is a closed-loop servo offset (not the oscillator free-running).
pub const DISCIPLINED_FORMATS: [&str; 9] = [
    "ptp4l",
    "phc2sys",
    "ts2phc",
    "chrony-tracking",
    "chrony-statistics",
    "ntp-loopstats",
    "ntp-peerstats",
    "wr-stat",
    "pps-assert",
];

impl FleetSpec {
    pub fn parse(src: &str) -> Result<FleetSpec, KshanaError> {
        let spec: FleetSpec = toml::from_str(src)
            .map_err(|e| KshanaError::InvalidInput(format!("invalid timing fleet: {e}")))?;
        spec.validate()?;
        Ok(spec)
    }

    /// Check the fleet: clocks and limits present, ids unique and safe, names known.
    pub fn validate(&self) -> Result<(), KshanaError> {
        let bad = |m: String| Err(KshanaError::InvalidInput(format!("timing fleet: {m}")));
        if self.clocks.is_empty() {
            return bad("no [[clocks]]".into());
        }
        if self.limits.is_empty() {
            return bad("no [[limits]]: state at least one limit (name and value_s)".into());
        }
        for l in &self.limits {
            if !(l.value_s.is_finite() && l.value_s > 0.0) {
                return bad(format!("limit '{}' must be a positive value_s", l.name));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for c in &self.clocks {
            if c.id.is_empty()
                || !c
                    .id
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || "-_.".contains(ch))
            {
                return bad(format!(
                    "clock id '{}' must be letters, digits, '-', '_' or '.'",
                    c.id
                ));
            }
            if !ids.insert(c.id.clone()) {
                return bad(format!("clock id '{}' is listed twice", c.id));
            }
            if let Some(r) = &c.record {
                if r != "free-running" && r != "disciplined" {
                    return bad(format!(
                        "clock '{}': record must be free-running or disciplined",
                        c.id
                    ));
                }
            }
            if let Some(k) = &c.class {
                if class_noise(k).is_none() {
                    return bad(format!(
                        "clock '{}': unknown class '{k}' (expected one of {})",
                        c.id,
                        CLASS_NAMES.join(", ")
                    ));
                }
            }
            for n in c.limits.iter().flatten() {
                if !self.limits.iter().any(|l| &l.name == n) {
                    return bad(format!("clock '{}' names an unknown limit '{n}'", c.id));
                }
            }
        }
        for (v, what) in [(self.window_h, "window_h"), (self.horizon_h, "horizon_h")] {
            if let Some(v) = v {
                if !(v.is_finite() && v > 0.0) {
                    return bad(format!("{what} must be positive"));
                }
            }
        }
        Ok(())
    }

    pub fn window_s(&self) -> f64 {
        self.window_h.unwrap_or(48.0) * 3600.0
    }
    pub fn horizon_s(&self) -> f64 {
        self.horizon_h.unwrap_or(48.0) * 3600.0
    }

    /// Make every relative source `path` relative to `base` (the fleet file's directory).
    pub fn resolve_paths(&mut self, base: &Path) {
        for c in &mut self.clocks {
            if let Some(p) = &c.source.path {
                let pb = Path::new(p);
                if pb.is_relative() {
                    c.source.path = Some(base.join(pb).to_string_lossy().into_owned());
                }
            }
        }
    }
}

/// The predicted times to breach one named limit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LimitBreach {
    pub name: String,
    pub value_s: f64,
    pub t_warn_s: Option<f64>,
    pub t_50_s: Option<f64>,
}

/// The assessment of one clock.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClockAssessment {
    pub id: String,
    pub site: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub role: Option<String>,
    pub source_format: String,
    /// `free-running` or `disciplined`.
    pub record: String,
    /// `measured` (noise from the record) or `prior` (noise from `class` or `adev_prior`).
    pub noise_source: NoiseSource,
    /// The prior used, when any (`class:rafs`, `datasheet`).
    pub prior: Option<String>,
    pub n_used: usize,
    pub tau0_s: f64,
    pub span_s: f64,
    pub handover_s: f64,
    /// SHA-256 of the phase samples used (the open engine's record hash).
    pub record_sha256: String,
    /// Largest |phase| in the record (s): for a servo record, the largest offset seen.
    pub max_abs_phase_s: f64,
    pub holdover: HoldoverModel,
    pub predictions: Vec<Prediction>,
    pub breaches: Vec<LimitBreach>,
    /// `ok` (no warning within the horizon), `watch`, `act` (a warning within 4 h) or
    /// `breached` (a servo record's present offset already beyond a limit).
    pub status: String,
}

impl ClockAssessment {
    /// The earliest warning time over this clock's limits.
    pub fn worst_warn_s(&self) -> Option<f64> {
        self.breaches
            .iter()
            .filter_map(|b| b.t_warn_s)
            .min_by(f64::total_cmp)
    }
}

/// The assessment of a fleet.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FleetAssessment {
    pub format: &'static str,
    pub kind: &'static str,
    pub label: String,
    pub title: String,
    pub fleet: String,
    pub window_s: f64,
    pub horizon_s: f64,
    pub limits: Vec<Limit>,
    pub clocks: Vec<ClockAssessment>,
    /// Clock ids by earliest warning first (clocks with no warning last).
    pub order: Vec<String>,
    pub counts: StatusCounts,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StatusCounts {
    pub clocks: usize,
    pub ok: usize,
    pub watch: usize,
    pub act: usize,
    pub breached: usize,
}

/// Status thresholds: a warning within this many seconds is `act`.
pub const ACT_WITHIN_S: f64 = 4.0 * 3600.0;

/// Prediction horizons reported per clock (those within the fleet horizon).
pub const REPORT_HORIZONS_S: [f64; 6] = [
    600.0,
    3600.0,
    4.0 * 3600.0,
    12.0 * 3600.0,
    86_400.0,
    172_800.0,
];

/// The breach search grid never has more than this many points (the step grows past it).
pub const MAX_GRID: f64 = 20_000.0;

fn read_record(c: &ClockSpec) -> Result<NormalisedPhase, KshanaError> {
    let mut inp = c.source.clone();
    if inp.gaps.is_none() {
        inp.gaps = Some("longest".into());
    }
    kshana::clock_log::read_input(&inp)
        .map_err(|e| KshanaError::InvalidInput(format!("clock '{}': {e}", c.id)))
}

fn prior_noise(c: &ClockSpec) -> Option<(Noise, String)> {
    if let Some(pts) = &c.adev_prior {
        let n = fit_noise(&pts.iter().map(|p| (p[0], p[1] * p[1])).collect::<Vec<_>>());
        return Some((n, "datasheet".into()));
    }
    let k = c.class.as_deref()?;
    Some((class_noise(k)?, format!("class:{}", k.to_ascii_lowercase())))
}

/// Assess one clock against the fleet's limits.
pub fn assess_clock(spec: &FleetSpec, c: &ClockSpec) -> Result<ClockAssessment, KshanaError> {
    let rec = read_record(c)?;
    let bad = |m: String| KshanaError::InvalidInput(format!("clock '{}': {m}", c.id));
    let tau0 = rec.tau0_s;
    let mut x: &[f64] = &rec.phase_s;
    let mut handover = rec.time_s.last().copied().unwrap_or(0.0);
    if let Some(h) = c.holdover_start_s {
        let k = (h / tau0).round();
        if k.is_nan() || k < 1.0 || k as usize >= x.len() {
            return Err(bad(format!(
                "holdover_start_s {h} is outside the record (0 .. {handover})"
            )));
        }
        x = &x[..=k as usize];
        handover = k * tau0;
    }
    let record = c.record.clone().unwrap_or_else(|| {
        if DISCIPLINED_FORMATS.contains(&c.source.format.as_str()) {
            "disciplined".into()
        } else {
            "free-running".into()
        }
    });
    let window = spec.window_s();
    let horizon = spec.horizon_s();
    let (model, prior) = if record == "disciplined" {
        let (n, name) = prior_noise(c).ok_or_else(|| {
            bad(format!(
                "its {} record is a servo offset, which does not show the oscillator's free-running noise: give `class` or `adev_prior`",
                c.source.format
            ))
        })?;
        (
            HoldoverModel::from_prior(x, tau0, window, n).map_err(bad)?,
            Some(name),
        )
    } else {
        (
            HoldoverModel::fit_kalman(x, tau0, window).map_err(bad)?,
            None,
        )
    };
    let step = tau0.max(horizon / MAX_GRID);
    let mut breaches = Vec::new();
    for l in &spec.limits {
        if let Some(names) = &c.limits {
            if !names.contains(&l.name) {
                continue;
            }
        }
        let b = model.time_to_breach(l.value_s, step, horizon);
        breaches.push(LimitBreach {
            name: l.name.clone(),
            value_s: l.value_s,
            t_warn_s: b.t_warn_s,
            t_50_s: b.t_50_s,
        });
    }
    let predictions = REPORT_HORIZONS_S
        .iter()
        .filter(|u| **u <= horizon)
        .map(|u| model.predict(*u))
        .collect();
    let max_abs = x.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let breached =
        record == "disciplined" && breaches.iter().any(|b| model.phase_s.abs() >= b.value_s);
    let warn = breaches
        .iter()
        .filter_map(|b| b.t_warn_s)
        .min_by(f64::total_cmp);
    let status = if breached {
        "breached"
    } else {
        match warn {
            None => "ok",
            Some(t) if t <= ACT_WITHIN_S => "act",
            Some(_) => "watch",
        }
    };
    let mut h = Sha256::new();
    for v in x {
        h.update(v.to_le_bytes());
    }
    Ok(ClockAssessment {
        id: c.id.clone(),
        site: c.site.clone(),
        vendor: c.vendor.clone(),
        model: c.model.clone(),
        role: c.role.clone(),
        source_format: c.source.format.clone(),
        record,
        noise_source: model.noise_source,
        prior,
        n_used: x.len(),
        tau0_s: tau0,
        span_s: (x.len().saturating_sub(1)) as f64 * tau0,
        handover_s: handover,
        record_sha256: hex::encode(h.finalize()),
        max_abs_phase_s: max_abs,
        holdover: model,
        predictions,
        breaches,
        status: status.into(),
    })
}

/// Assess every clock of the fleet.
pub fn assess(spec: &FleetSpec) -> Result<FleetAssessment, KshanaError> {
    let mut clocks = Vec::new();
    for c in &spec.clocks {
        clocks.push(assess_clock(spec, c)?);
    }
    let mut order: Vec<(Option<f64>, String)> = clocks
        .iter()
        .map(|c| {
            let w = if c.status == "breached" {
                Some(0.0)
            } else {
                c.worst_warn_s()
            };
            (w, c.id.clone())
        })
        .collect();
    order.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => x.total_cmp(&y).then(a.1.cmp(&b.1)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.1.cmp(&b.1),
    });
    let mut counts = StatusCounts {
        clocks: clocks.len(),
        ..Default::default()
    };
    for c in &clocks {
        match c.status.as_str() {
            "ok" => counts.ok += 1,
            "watch" => counts.watch += 1,
            "act" => counts.act += 1,
            _ => counts.breached += 1,
        }
    }
    Ok(FleetAssessment {
        format: ASSESSMENT_FORMAT,
        kind: super::KIND,
        label: super::label(),
        title: spec
            .title
            .clone()
            .unwrap_or_else(|| "Timing fleet holdover".into()),
        fleet: spec.fleet.clone().unwrap_or_else(|| "fleet".into()),
        window_s: spec.window_s(),
        horizon_s: spec.horizon_s(),
        limits: spec.limits.clone(),
        clocks,
        order: order.into_iter().map(|o| o.1).collect(),
        counts,
    })
}

impl FleetAssessment {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the assessment is plain data")
    }

    pub fn clock(&self, id: &str) -> Option<&ClockAssessment> {
        self.clocks.iter().find(|c| c.id == id)
    }

    /// One CSV row per (clock, limit).
    pub fn to_csv(&self) -> String {
        let mut s = String::from(
            "clock,site,vendor,model,role,source,record,noise,limit,limit_s,t_warn_s,t_50_s,status\n",
        );
        let q = |v: &Option<String>| v.clone().unwrap_or_default().replace(',', ";");
        let t = |v: Option<f64>| v.map(|x| format!("{x}")).unwrap_or_else(|| "beyond".into());
        for c in &self.clocks {
            for b in &c.breaches {
                s.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    c.id,
                    q(&c.site),
                    q(&c.vendor),
                    q(&c.model),
                    q(&c.role),
                    c.source_format,
                    c.record,
                    match c.noise_source {
                        NoiseSource::Measured => "measured",
                        NoiseSource::Prior => "prior",
                    },
                    b.name.replace(',', ";"),
                    b.value_s,
                    t(b.t_warn_s),
                    t(b.t_50_s),
                    c.status
                ));
            }
        }
        s
    }
}

/// A duration for people: `beyond` for none, else s, min, h or d.
pub fn human_duration(t: Option<f64>) -> String {
    match t {
        None => "beyond horizon".into(),
        Some(s) if s < 120.0 => format!("{s:.0} s"),
        Some(s) if s < 7200.0 => format!("{:.0} min", s / 60.0),
        Some(s) if s < 172_800.0 => format!("{:.1} h", s / 3600.0),
        Some(s) => format!("{:.1} d", s / 86_400.0),
    }
}

/// A time error for people (s, ms, us, ns or ps).
pub fn human_time(x: f64) -> String {
    let a = x.abs();
    if a >= 1.0 {
        format!("{x:.3} s")
    } else if a >= 1e-3 {
        format!("{:.3} ms", x * 1e3)
    } else if a >= 1e-6 {
        format!("{:.3} µs", x * 1e6)
    } else if a >= 1e-9 {
        format!("{:.2} ns", x * 1e9)
    } else {
        format!("{:.1} ps", x * 1e12)
    }
}
