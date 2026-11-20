// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Nuisance-alert and alert-fatigue statistics (B1103)
//!
//! Crews and bridge teams reject alerts caused by interference: TAWS/EGPWS terrain warnings
//! from a spoofed position, clock and GNSS position alerts, bridge GNSS and BNWAS alarms.
//! This module counts them from the alert logs an operator already keeps, so the rates
//! reproduce exactly from the input:
//!
//! * every alert is classed by its text ([`super::alert_class`]: `taws`, `clock`, `gnss`,
//!   `bnwas`, `ais`, `other`) and, where the log says so, by its disposition (`nuisance` /
//!   `false` / `spurious`, `genuine` / `valid`, or unknown);
//! * an alert is **interference-attributed** when its subject had an interference interval
//!   (unified events from `forensics fdm` or `reports episodes`, or a CSV
//!   `subject,start_utc,end_utc`) covering it within `margin_s`, and its class depends on
//!   GNSS (every class except `other`);
//! * an alert is **likely nuisance** when its disposition says so, or when the disposition is
//!   unknown and it is interference-attributed in a class that interference produces falsely
//!   (`taws`, `clock`, `gnss`, `bnwas`, `ais`).
//!
//! Per route, per phase and per class: counts, the stated nuisance share among dispositioned
//! alerts and the likely-nuisance share, each with a Wilson 95 % interval; with crew hours
//! per route, alerts and likely-nuisance alerts per crew-hour (95 % Poisson interval) and a
//! workload estimate (handling seconds per likely-nuisance alert, by class, per crew-hour).
//! A training extract lists the route, phase and class cells with the most likely-nuisance
//! alerts. MODELLED: the attribution rule is exercised on synthetic logs; decision support.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::*;
use crate::forensics::inputs::iso;

pub const FORMAT: &str = "kshana-pro-nuisance-alerts/1";
pub const METHOD: &str = "kshana-pro/nuisance-alerts/v1";
pub const RESULT_FILE: &str = "nuisance-alerts.json";
pub const EVIDENCE: &str =
    "MODELLED for the attribution of undispositioned alerts (rule exercised \
on synthetic logs); counts and stated shares are COMPUTED from the logs supplied and reproduce \
from them exactly.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Params {
    pub margin_s: f64,
    /// Seconds a crew spends recognising and rejecting one nuisance alert, by class.
    pub handling_s: BTreeMap<String, f64>,
    /// Cells in the training extract.
    pub training_top: usize,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            margin_s: 120.0,
            handling_s: [
                ("taws", 30.0),
                ("clock", 20.0),
                ("gnss", 20.0),
                ("bnwas", 10.0),
                ("ais", 15.0),
                ("other", 15.0),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
            training_top: 10,
        }
    }
}

/// A nuisance-alert request (TOML).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub title: String,
    /// CSV `time,subject,route,phase,alert[,disposition]`.
    pub alerts: Vec<String>,
    /// Interference intervals: unified events JSON, or CSV `subject,start_utc,end_utc`.
    #[serde(default)]
    pub exposure: Vec<String>,
    /// CSV `route,crew_hours` (lines for one route add up).
    #[serde(default)]
    pub hours: Option<String>,
    #[serde(default)]
    pub params: Params,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    pub t: f64,
    pub subject: String,
    pub route: String,
    pub phase: String,
    pub text: String,
    pub class: String,
    /// `nuisance`, `genuine` or `unknown`.
    pub disposition: String,
    pub interference_attributed: bool,
    pub likely_nuisance: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub alerts: usize,
    pub stated_nuisance: usize,
    pub stated_genuine: usize,
    pub undispositioned: usize,
    pub interference_attributed: usize,
    pub likely_nuisance: usize,
    /// Stated nuisance over dispositioned alerts.
    pub stated_nuisance_share: Option<f64>,
    pub stated_nuisance_ci95: Option<(f64, f64)>,
    /// Likely nuisance over all alerts.
    pub likely_nuisance_share: Option<f64>,
    pub likely_nuisance_ci95: Option<(f64, f64)>,
    pub crew_hours: Option<f64>,
    pub alerts_per_crew_hour: Option<f64>,
    pub alerts_per_crew_hour_ci95: Option<(f64, f64)>,
    pub likely_nuisance_per_crew_hour: Option<f64>,
    pub likely_nuisance_per_crew_hour_ci95: Option<(f64, f64)>,
    /// Minutes of crew handling of likely-nuisance alerts per crew-hour.
    pub handling_min_per_crew_hour: Option<f64>,
    #[serde(skip)]
    handling_s: f64,
}

impl Cell {
    fn add(&mut self, a: &Alert, handling: f64) {
        self.alerts += 1;
        match a.disposition.as_str() {
            "nuisance" => self.stated_nuisance += 1,
            "genuine" => self.stated_genuine += 1,
            _ => self.undispositioned += 1,
        }
        self.interference_attributed += a.interference_attributed as usize;
        if a.likely_nuisance {
            self.likely_nuisance += 1;
            self.handling_s += handling;
        }
    }

    fn finish(&mut self, hours: Option<f64>) {
        let d = self.stated_nuisance + self.stated_genuine;
        self.stated_nuisance_share =
            (d > 0).then(|| rnd(self.stated_nuisance as f64 / d as f64, 4));
        self.stated_nuisance_ci95 = wilson95(self.stated_nuisance, d);
        self.likely_nuisance_share =
            (self.alerts > 0).then(|| rnd(self.likely_nuisance as f64 / self.alerts as f64, 4));
        self.likely_nuisance_ci95 = wilson95(self.likely_nuisance, self.alerts);
        if let Some(h) = hours.filter(|h| *h > 0.0) {
            let per = |n: usize| {
                let (lo, hi) = poisson95(n);
                (rnd(n as f64 / h, 4), (rnd(lo / h, 4), rnd(hi / h, 4)))
            };
            let (a, aci) = per(self.alerts);
            let (l, lci) = per(self.likely_nuisance);
            self.crew_hours = Some(rnd(h, 3));
            self.alerts_per_crew_hour = Some(a);
            self.alerts_per_crew_hour_ci95 = Some(aci);
            self.likely_nuisance_per_crew_hour = Some(l);
            self.likely_nuisance_per_crew_hour_ci95 = Some(lci);
            self.handling_min_per_crew_hour = Some(rnd(self.handling_s / 60.0 / h, 4));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrainingRow {
    pub route: String,
    pub phase: String,
    pub class: String,
    pub alerts: usize,
    pub likely_nuisance: usize,
    pub likely_nuisance_share: f64,
    pub example_alerts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NuisanceResult {
    pub format: String,
    pub method: String,
    pub evidence: String,
    pub statement: String,
    pub title: String,
    pub period_start_utc: Option<String>,
    pub period_end_utc: Option<String>,
    pub rules: Vec<String>,
    pub overall: Cell,
    pub by_route: BTreeMap<String, Cell>,
    pub by_phase: BTreeMap<String, Cell>,
    pub by_class: BTreeMap<String, Cell>,
    pub by_route_phase: BTreeMap<String, Cell>,
    pub training_extract: Vec<TrainingRow>,
    pub exposure_intervals: usize,
    pub rows_refused: usize,
    pub limits: Vec<String>,
    pub params: Params,
    pub inputs: Vec<InputRef>,
}

fn disposition(s: &str) -> &'static str {
    match s.trim().to_ascii_lowercase().as_str() {
        "nuisance" | "false" | "spurious" | "false-alert" | "false_alert" | "rejected" => {
            "nuisance"
        }
        "genuine" | "valid" | "true" | "real" => "genuine",
        _ => "unknown",
    }
}

/// Interference intervals by subject, from unified events (JSON) or CSV.
pub fn read_intervals(text: &str) -> Result<Vec<(String, f64, f64)>, String> {
    let t = text.trim_start();
    let mut out = vec![];
    if t.starts_with('[') || t.starts_with('{') {
        let v: serde_json::Value = serde_json::from_str(t).map_err(|e| e.to_string())?;
        let arr = match &v {
            serde_json::Value::Array(a) => a.clone(),
            other => other
                .get("unified_events")
                .or(other.get("events"))
                .and_then(|e| e.as_array())
                .cloned()
                .ok_or("JSON has no events array")?,
        };
        for e in arr {
            let ev: crate::ievents::Event =
                serde_json::from_value(e).map_err(|e| format!("event: {e}"))?;
            let (s, e2) = (utc(&ev.start_utc)?, utc(&ev.end_utc)?);
            for o in &ev.observers {
                let subject = o.split_once(':').map(|(_, x)| x).unwrap_or(o);
                out.push((subject.to_string(), s, e2));
            }
        }
    } else {
        for (i, l) in text.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') || l.to_ascii_lowercase().starts_with("subject") {
                continue;
            }
            let f: Vec<&str> = l.split(',').map(str::trim).collect();
            if f.len() < 3 {
                return Err(format!(
                    "intervals line {}: needs subject,start_utc,end_utc",
                    i + 1
                ));
            }
            out.push((f[0].to_string(), utc(f[1])?, utc(f[2])?));
        }
    }
    Ok(out)
}

/// One alert log row: time, subject, route, phase, alert text, disposition text.
pub type AlertRow = (f64, String, String, String, String, String);

/// Parse an alert log.
pub fn read_alerts(text: &str) -> Result<(Vec<AlertRow>, usize), String> {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let h: Vec<String> = lines
        .next()
        .ok_or("empty alert log")?
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let c = |names: &[&str]| h.iter().position(|x| names.contains(&x.as_str()));
    let ct = c(&["time", "timestamp", "utc"]).ok_or("alert log has no 'time' column")?;
    let ca = c(&["alert", "message", "text"]).ok_or("alert log has no 'alert' column")?;
    let (cs, cr, cp, cd) = (
        c(&["subject", "aircraft", "vessel", "tail"]),
        c(&["route", "sector", "voyage"]),
        c(&["phase", "flight_phase", "area"]),
        c(&["disposition", "outcome", "crew_assessment"]),
    );
    let (mut out, mut refused) = (vec![], 0);
    for l in lines {
        let f: Vec<&str> = l.split(',').map(str::trim).collect();
        let g = |i: Option<usize>| i.and_then(|i| f.get(i).copied()).unwrap_or("").to_string();
        let Some(t) = crate::forensics::inputs::parse_utc(&g(Some(ct))) else {
            refused += 1;
            continue;
        };
        let text = g(Some(ca));
        if text.is_empty() {
            refused += 1;
            continue;
        }
        let or = |s: String| {
            if s.is_empty() {
                "not stated".to_string()
            } else {
                s
            }
        };
        out.push((t, or(g(cs)), or(g(cr)), or(g(cp)), text, g(cd)));
    }
    Ok((out, refused))
}

/// Analyse alert logs, interval texts and an hours CSV.
pub fn analyse(
    req: &Request,
    alert_logs: &[(String, String)],
    exposure: &[(String, String)],
    hours_csv: Option<&str>,
    inputs: &Inputs,
) -> Result<NuisanceResult, String> {
    let p = &req.params;
    let mut intervals = vec![];
    for (name, text) in exposure {
        intervals.extend(read_intervals(text).map_err(|e| format!("{name}: {e}"))?);
    }
    let mut hours: BTreeMap<String, f64> = BTreeMap::new();
    if let Some(csv) = hours_csv {
        for (i, l) in csv.lines().enumerate() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') || l.to_ascii_lowercase().starts_with("route") {
                continue;
            }
            let (r, h) = l
                .rsplit_once(',')
                .ok_or(format!("hours line {}: needs route,crew_hours", i + 1))?;
            let h: f64 = h
                .trim()
                .parse()
                .map_err(|_| format!("hours line {}: crew_hours is not a number", i + 1))?;
            *hours.entry(r.trim().to_string()).or_default() += h;
        }
    }
    let mut alerts = vec![];
    let mut refused = 0;
    for (name, text) in alert_logs {
        let (rows, r) = read_alerts(text).map_err(|e| format!("{name}: {e}"))?;
        refused += r;
        for (t, subject, route, phase, text, disp) in rows {
            let class = alert_class(&text).to_string();
            let disposition = disposition(&disp).to_string();
            let attributed = class != "other"
                && intervals
                    .iter()
                    .any(|(s, a, b)| *s == subject && t >= a - p.margin_s && t <= b + p.margin_s);
            let likely = disposition == "nuisance" || (disposition == "unknown" && attributed);
            alerts.push(Alert {
                t,
                subject,
                route,
                phase,
                text,
                class,
                disposition,
                interference_attributed: attributed,
                likely_nuisance: likely,
            });
        }
    }
    alerts.sort_by(|a, b| a.t.total_cmp(&b.t));
    let handling = |c: &str| p.handling_s.get(c).copied().unwrap_or(0.0);
    let mut overall = Cell::default();
    let mut by_route: BTreeMap<String, Cell> = BTreeMap::new();
    let mut by_phase: BTreeMap<String, Cell> = BTreeMap::new();
    let mut by_class: BTreeMap<String, Cell> = BTreeMap::new();
    let mut by_rp: BTreeMap<String, Cell> = BTreeMap::new();
    let mut cells: BTreeMap<(String, String, String), (Cell, Vec<String>)> = BTreeMap::new();
    for a in &alerts {
        let h = handling(&a.class);
        overall.add(a, h);
        by_route.entry(a.route.clone()).or_default().add(a, h);
        by_phase.entry(a.phase.clone()).or_default().add(a, h);
        by_class.entry(a.class.clone()).or_default().add(a, h);
        by_rp
            .entry(format!("{} / {}", a.route, a.phase))
            .or_default()
            .add(a, h);
        let e = cells
            .entry((a.route.clone(), a.phase.clone(), a.class.clone()))
            .or_default();
        e.0.add(a, h);
        if a.likely_nuisance && e.1.len() < 3 && !e.1.contains(&a.text) {
            e.1.push(a.text.clone());
        }
    }
    let total_hours: f64 = hours.values().sum();
    overall.finish((total_hours > 0.0).then_some(total_hours));
    for (k, c) in by_route.iter_mut() {
        c.finish(hours.get(k).copied());
    }
    for c in by_phase
        .values_mut()
        .chain(by_class.values_mut())
        .chain(by_rp.values_mut())
    {
        c.finish(None);
    }
    let mut training: Vec<TrainingRow> = cells
        .into_iter()
        .filter(|(_, (c, _))| c.likely_nuisance > 0)
        .map(|((route, phase, class), (c, ex))| TrainingRow {
            route,
            phase,
            class,
            alerts: c.alerts,
            likely_nuisance: c.likely_nuisance,
            likely_nuisance_share: rnd(c.likely_nuisance as f64 / c.alerts as f64, 4),
            example_alerts: ex,
        })
        .collect();
    training.sort_by(|a, b| {
        b.likely_nuisance
            .cmp(&a.likely_nuisance)
            .then(b.likely_nuisance_share.total_cmp(&a.likely_nuisance_share))
            .then(a.route.cmp(&b.route))
            .then(a.phase.cmp(&b.phase))
            .then(a.class.cmp(&b.class))
    });
    training.truncate(p.training_top);
    let rules = vec![
        format!("Interference-attributed: the alert's class depends on GNSS (not 'other') and an interference interval of the same subject covers it within {} s.", p.margin_s),
        "Likely nuisance: the log says nuisance/false/spurious, or the disposition is unknown and the alert is interference-attributed.".into(),
        "Stated nuisance share: stated nuisance over dispositioned alerts (Wilson 95 %). Rates per crew-hour use the hours file (Poisson 95 %, Byar).".into(),
        "Workload: handling seconds per likely-nuisance alert by class (params.handling_s), summed and divided by crew hours.".into(),
    ];
    let limits = vec![
        "Alerts are classed by their text; a genuine terrain or traffic alert inside an interference interval counts as interference-attributed. Stated dispositions take precedence.".to_string(),
        "Handling times are stated parameters, not measurements of crew workload.".into(),
        "Per-crew-hour rates are only as complete as the alert logs and the hours supplied.".into(),
    ];
    Ok(NuisanceResult {
        format: FORMAT.into(),
        method: METHOD.into(),
        evidence: EVIDENCE.into(),
        statement: DECISION_SUPPORT.into(),
        title: req.title.clone(),
        period_start_utc: alerts.first().map(|a| iso(a.t)),
        period_end_utc: alerts.last().map(|a| iso(a.t)),
        rules,
        overall,
        by_route,
        by_phase,
        by_class,
        by_route_phase: by_rp,
        training_extract: training,
        exposure_intervals: intervals.len(),
        rows_refused: refused,
        limits,
        params: p.clone(),
        inputs: inputs.refs.clone(),
    })
}

/// The training extract as CSV.
pub fn training_csv(r: &NuisanceResult) -> String {
    let mut s = String::from(
        "route,phase,class,alerts,likely_nuisance,likely_nuisance_share,example_alerts\n",
    );
    for t in &r.training_extract {
        s += &format!(
            "{},{},{},{},{},{},{}\n",
            t.route,
            t.phase,
            t.class,
            t.alerts,
            t.likely_nuisance,
            t.likely_nuisance_share,
            t.example_alerts.join(" | ")
        );
    }
    s
}

/// Read a request file and its inputs, and analyse.
pub fn run(path: &Path) -> Result<(NuisanceResult, Inputs), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let req: Request = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut inp = Inputs::default();
    inp.add_text("request.toml", "request", &text);
    let mut logs = vec![];
    for a in &req.alerts {
        logs.push((a.clone(), inp.read(base, a, "alerts")?));
    }
    let mut exp = vec![];
    for a in &req.exposure {
        exp.push((a.clone(), inp.read(base, a, "exposure")?));
    }
    let hours = match &req.hours {
        Some(h) => Some(inp.read(base, h, "crew-hours")?),
        None => None,
    };
    let r = analyse(&req, &logs, &exp, hours.as_deref(), &inp)?;
    Ok((r, inp))
}

/// The plain-text summary.
pub fn summary(r: &NuisanceResult) -> String {
    let o = &r.overall;
    let mut s = format!(
        "Nuisance alerts: {}\n{}\nEvidence: {}\n\n{} alert(s): {} stated nuisance, {} stated genuine, {} undispositioned; {} interference-attributed; {} likely nuisance\n",
        r.title, r.statement, r.evidence, o.alerts, o.stated_nuisance, o.stated_genuine, o.undispositioned, o.interference_attributed, o.likely_nuisance
    );
    for (k, c) in &r.by_route {
        s += &format!(
            "Route {k}: {} alert(s), {} likely nuisance{}\n",
            c.alerts,
            c.likely_nuisance,
            c.likely_nuisance_per_crew_hour
                .map(|v| format!(", {v} per crew-hour"))
                .unwrap_or_default()
        );
    }
    s
}
