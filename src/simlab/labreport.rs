// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Comparable lab test reports for spoofing and interference test services.
//!
//! Test services report the same kinds of results in their own layouts and units (a J/S at
//! loss of lock in one table, a "time to detect" in milliseconds in another), so two labs'
//! reports on the same receiver cannot be put side by side. This module defines a common
//! report ([`REPORT_FORMAT`]) with a fixed metric vocabulary ([`VOCABULARY`]) and canonical
//! units, a **column mapping** that turns one lab's results CSV into it (format
//! [`RESULTS_FORMAT`]; one mapping per lab layout, written once), and a comparison across
//! labs:
//!
//! ```toml
//! format = "kshana-simlab-lab-results/1"
//! lab = "Lab A"
//! service = "spoofing"            # spoofing | interference
//! report_id = "LA-2026-014"
//! date = "2026-11-10"
//! test_object = "Receiver X, firmware 1.2"
//! results = "lab-a.csv"
//! [setup]
//! rf_path = "conducted"           # conducted | shielded-chamber (over-the-air is refused)
//! simulator = "Lab simulator"
//! reference = "gpsdo"
//! calibration_due = "2027-03-01"
//! [scenario_ids]                  # the lab's scenario names → common scenario ids
//! "SP-03 sync takeover" = "spoof-sync-takeover"
//! [scenario_sha256]               # optional: SHA-256 of each common scenario file run
//! spoof-sync-takeover = "…"
//! [columns]
//! "Scenario" = { field = "scenario" }
//! "TTD [ms]" = { metric = "time_to_detect_s", unit = "ms" }
//! "U(TTD) [ms]" = { uncertainty_of = "time_to_detect_s", unit = "ms" }   # expanded, k = 2
//! ```
//!
//! The normalised report lists every result in canonical units with its expanded
//! uncertainty when the lab gave one, the columns left unmapped, the vocabulary metrics of
//! the service the report does not carry, and an **EN 16803 topic index**: which report
//! contents bear on the topics of EN 16803-1 (definitions and performance metrics), -2
//! (basic performance by record and replay) and -3 (security performance), cited by part
//! and scope only. No accreditation scheme for EN 16803 testing exists to our knowledge; the
//! index helps a lab assemble its own evidence and is not a statement that a test, a lab or
//! a receiver conforms to the standard.
//!
//! The comparison ([`compare`]) groups results by scenario and metric across reports and,
//! when both labs stated an expanded uncertainty, gives the **En number**
//! `En = |x₁ − x₂| / √(U₁² + U₂²)` (ISO 13528; |En| ≤ 1 consistent). It names what stops two
//! results being comparable: a different scenario file hash, a different RF path, a missing
//! uncertainty.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::r;
use crate::aviation::Inputs;

pub const RESULTS_FORMAT: &str = "kshana-simlab-lab-results/1";
pub const REPORT_FORMAT: &str = "kshana-simlab-lab-report/1";
pub const METHOD: &str = "kshana-pro/simlab-lab-report/v1";

/// `(metric, canonical unit, meaning)`.
pub const VOCABULARY: &[(&str, &str, &str)] = &[
    (
        "jsr_at_first_effect_db",
        "dB",
        "J/S at which C/N0 first drops by 1 dB or more",
    ),
    (
        "jsr_at_loss_of_lock_db",
        "dB",
        "J/S at which the receiver loses lock or the fix",
    ),
    (
        "jsr_at_reacquisition_db",
        "dB",
        "J/S at which it reacquires on a falling ramp",
    ),
    (
        "cn0_drop_db",
        "dB",
        "C/N0 drop at the scenario's peak interference",
    ),
    (
        "time_to_detect_s",
        "s",
        "attack start to the receiver's first spoofing/interference flag",
    ),
    (
        "time_to_alarm_s",
        "s",
        "attack start to the alarm at the user interface",
    ),
    (
        "time_to_capture_s",
        "s",
        "attack start to the position or time being pulled",
    ),
    (
        "reacquisition_time_s",
        "s",
        "attack end to a valid fix again",
    ),
    (
        "ttff_s",
        "s",
        "time to first fix after the scenario's start",
    ),
    (
        "position_error_max_m",
        "m",
        "largest horizontal position error while a fix was output",
    ),
    (
        "position_error_95_m",
        "m",
        "95th-percentile horizontal position error",
    ),
    (
        "time_error_max_ns",
        "ns",
        "largest time error of the PPS or time output",
    ),
    (
        "availability",
        "fraction",
        "share of epochs with a valid fix",
    ),
    ("detected", "bool", "the receiver flagged the attack"),
    (
        "captured",
        "bool",
        "the receiver output followed the false solution",
    ),
];

/// The metrics a report of each service is expected to carry.
pub fn core_metrics(service: &str) -> &'static [&'static str] {
    match service {
        "spoofing" => &[
            "detected",
            "captured",
            "time_to_detect_s",
            "position_error_max_m",
            "time_error_max_ns",
        ],
        _ => &[
            "jsr_at_loss_of_lock_db",
            "jsr_at_reacquisition_db",
            "reacquisition_time_s",
            "position_error_95_m",
            "availability",
        ],
    }
}

fn canonical(metric: &str) -> Option<&'static str> {
    VOCABULARY
        .iter()
        .find(|(m, _, _)| *m == metric)
        .map(|(_, u, _)| *u)
}

/// Convert `v` in `unit` to the canonical unit `to`.
pub fn convert(v: f64, unit: &str, to: &str) -> Result<f64, String> {
    let f = match (to, unit) {
        ("dB", "dB") => 1.0,
        ("s", "s") => 1.0,
        ("s", "ms") => 1e-3,
        ("s", "min") => 60.0,
        ("m", "m") => 1.0,
        ("m", "cm") => 1e-2,
        ("m", "km") => 1e3,
        ("ns", "ns") => 1.0,
        ("ns", "us" | "µs") => 1e3,
        ("ns", "ms") => 1e6,
        ("ns", "s") => 1e9,
        ("fraction", "fraction") => 1.0,
        ("fraction", "%") => 1e-2,
        _ => return Err(format!("cannot convert {unit} to {to}")),
    };
    Ok(v * f)
}

fn parse_bool(s: &str) -> Option<f64> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "yes" | "y" | "true" | "detected" | "captured" => Some(1.0),
        "0" | "no" | "n" | "false" | "not detected" | "not captured" => Some(0.0),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnMap {
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub uncertainty_of: Option<String>,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub rf_path: String,
    #[serde(default)]
    pub simulator: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub calibration_due: Option<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabResults {
    pub format: String,
    pub lab: String,
    pub service: String,
    pub report_id: String,
    pub date: String,
    pub test_object: String,
    pub results: String,
    pub setup: Setup,
    #[serde(default)]
    pub scenario_ids: BTreeMap<String, String>,
    #[serde(default)]
    pub scenario_sha256: BTreeMap<String, String>,
    pub columns: BTreeMap<String, ColumnMap>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReportResult {
    pub scenario: String,
    pub scenario_sha256: Option<String>,
    pub metric: String,
    pub value: f64,
    pub unit: String,
    /// Expanded uncertainty (k = 2) in the canonical unit, when the lab stated one.
    pub expanded_u: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Topic {
    pub part: String,
    pub topic: String,
    /// `carried` or `not-carried`.
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabReport {
    pub format: String,
    pub method: String,
    pub label: String,
    pub evidence: String,
    pub lab: String,
    pub service: String,
    pub report_id: String,
    pub date: String,
    pub test_object: String,
    pub setup: Setup,
    pub results: Vec<ReportResult>,
    pub unmapped_columns: Vec<String>,
    pub skipped_cells: Vec<String>,
    pub missing_core_metrics: Vec<String>,
    pub en16803_topics: Vec<Topic>,
}

pub struct Loaded {
    pub spec: LabResults,
    pub csv: String,
    pub inputs: Inputs,
}

pub fn load(path: &Path) -> Result<Loaded, String> {
    let (src, mut inputs) = super::read_request(path, "lab-results")?;
    let spec: LabResults = toml::from_str(&src).map_err(|e| format!("lab results: {e}"))?;
    if spec.format != RESULTS_FORMAT {
        return Err(format!(
            "lab results: format '{}' (expected {RESULTS_FORMAT})",
            spec.format
        ));
    }
    if spec.service != "spoofing" && spec.service != "interference" {
        return Err("lab results: service must be spoofing or interference".into());
    }
    if spec.setup.rf_path != "conducted" && spec.setup.rf_path != "shielded-chamber" {
        return Err(
            "lab results: setup.rf_path must be conducted or shielded-chamber (over-the-air test results are not taken)".into(),
        );
    }
    kshana_eloran::calreg::day_number(&spec.date).map_err(|e| format!("lab results: {e}"))?;
    let mut scen = 0;
    for (col, m) in &spec.columns {
        let set = [
            m.field.is_some(),
            m.metric.is_some(),
            m.uncertainty_of.is_some(),
        ];
        if set.iter().filter(|x| **x).count() != 1 {
            return Err(format!(
                "column '{col}': give exactly one of field, metric, uncertainty_of"
            ));
        }
        if let Some(f) = &m.field {
            if f != "scenario" {
                return Err(format!("column '{col}': unknown field '{f}'"));
            }
            scen += 1;
        }
        for x in [&m.metric, &m.uncertainty_of].into_iter().flatten() {
            let to = canonical(x).ok_or_else(|| format!("column '{col}': unknown metric '{x}'"))?;
            let unit = m.unit.as_deref().unwrap_or(to);
            if to != "bool" {
                convert(1.0, unit, to).map_err(|e| format!("column '{col}': {e}"))?;
            }
        }
    }
    if scen != 1 {
        return Err("lab results: exactly one column must map to field = \"scenario\"".into());
    }
    let csv = inputs.read(super::base_of(path), &spec.results, "results")?;
    Ok(Loaded { spec, csv, inputs })
}

fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut q = false;
    for ch in line.chars() {
        match ch {
            '"' => q = !q,
            ',' if !q => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(ch),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// Normalise one lab's results.
pub fn normalise(l: &Loaded) -> Result<LabReport, String> {
    let s = &l.spec;
    let mut lines = l.csv.lines().filter(|x| !x.trim().is_empty());
    let header = split_csv(lines.next().ok_or("results: empty file")?);
    for c in s.columns.keys() {
        if !header.contains(c) {
            return Err(format!("results: mapped column '{c}' is not in the header"));
        }
    }
    let unmapped: Vec<String> = header
        .iter()
        .filter(|h| !s.columns.contains_key(*h))
        .cloned()
        .collect();
    let scol = header
        .iter()
        .position(|h| s.columns.get(h).is_some_and(|m| m.field.is_some()))
        .unwrap();
    let mut results: Vec<ReportResult> = Vec::new();
    let mut skipped = Vec::new();
    let mut uncert: Vec<(String, String, f64)> = Vec::new();
    for (row, line) in lines.enumerate() {
        let f = split_csv(line);
        let raw = f.get(scol).cloned().unwrap_or_default();
        let scenario = s.scenario_ids.get(&raw).cloned().unwrap_or(raw);
        for (ci, h) in header.iter().enumerate() {
            let Some(m) = s.columns.get(h) else { continue };
            let cell = f.get(ci).map(String::as_str).unwrap_or("");
            if cell.is_empty() || m.field.is_some() {
                continue;
            }
            let metric = m.metric.as_ref().or(m.uncertainty_of.as_ref()).unwrap();
            let to = canonical(metric).unwrap();
            let v = if to == "bool" {
                parse_bool(cell)
            } else {
                cell.parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .and_then(|x| convert(x, m.unit.as_deref().unwrap_or(to), to).ok())
            };
            let Some(v) = v else {
                skipped.push(format!("row {} column '{h}': '{cell}'", row + 2));
                continue;
            };
            if m.uncertainty_of.is_some() {
                uncert.push((scenario.clone(), metric.clone(), v.abs()));
            } else {
                results.push(ReportResult {
                    scenario: scenario.clone(),
                    scenario_sha256: s.scenario_sha256.get(&scenario).cloned(),
                    metric: metric.clone(),
                    value: r(v, 9),
                    unit: to.into(),
                    expanded_u: None,
                });
            }
        }
    }
    for (sc, m, u) in uncert {
        if let Some(x) = results
            .iter_mut()
            .find(|x| x.scenario == sc && x.metric == m)
        {
            x.expanded_u = Some(r(u, 9));
        }
    }
    let have = |m: &str| results.iter().any(|x| x.metric == m);
    let missing: Vec<String> = core_metrics(&s.service)
        .iter()
        .filter(|m| !have(m))
        .map(|m| m.to_string())
        .collect();
    let topics = topics(s, &results);
    Ok(LabReport {
        format: REPORT_FORMAT.into(),
        method: METHOD.into(),
        label: super::LABEL.into(),
        evidence: super::EVIDENCE.into(),
        lab: s.lab.clone(),
        service: s.service.clone(),
        report_id: s.report_id.clone(),
        date: s.date.clone(),
        test_object: s.test_object.clone(),
        setup: s.setup.clone(),
        results,
        unmapped_columns: unmapped,
        skipped_cells: skipped,
        missing_core_metrics: missing,
        en16803_topics: topics,
    })
}

fn topics(s: &LabResults, res: &[ReportResult]) -> Vec<Topic> {
    let t = |part: &str, topic: &str, ok: bool, detail: String| Topic {
        part: part.into(),
        topic: topic.into(),
        status: if ok { "carried" } else { "not-carried" }.into(),
        detail,
    };
    let scen: std::collections::BTreeSet<&str> = res.iter().map(|x| x.scenario.as_str()).collect();
    let hashed = scen
        .iter()
        .filter(|x| s.scenario_sha256.contains_key(**x))
        .count();
    let with_u = res.iter().filter(|x| x.expanded_u.is_some()).count();
    let has = |ms: &[&str]| res.iter().any(|x| ms.contains(&x.metric.as_str()));
    let cal_ok = s.setup.calibration_due.as_deref().is_some_and(|d| {
        matches!(
            (kshana_eloran::calreg::day_number(d), kshana_eloran::calreg::day_number(&s.date)),
            (Ok(a), Ok(b)) if a >= b
        )
    });
    vec![
        t(
            "EN 16803-1",
            "use case and performance metrics defined",
            !scen.is_empty(),
            format!(
                "{} scenario(s) in the common vocabulary; {} with a scenario file hash",
                scen.len(),
                hashed
            ),
        ),
        t(
            "EN 16803-2",
            "record-and-replay bench described",
            !s.setup.simulator.is_empty() && !s.setup.reference.is_empty(),
            format!(
                "RF path {}, simulator '{}', reference '{}'",
                s.setup.rf_path, s.setup.simulator, s.setup.reference
            ),
        ),
        t(
            "EN 16803-2",
            "accuracy and availability metrics",
            has(&[
                "position_error_95_m",
                "position_error_max_m",
                "availability",
                "ttff_s",
            ]),
            "position error, availability or TTFF results".into(),
        ),
        t(
            "EN 16803-3",
            "security performance under jamming or spoofing",
            has(&[
                "jsr_at_loss_of_lock_db",
                "jsr_at_first_effect_db",
                "detected",
                "captured",
                "time_to_detect_s",
            ]),
            "J/S thresholds, detection or capture results".into(),
        ),
        t(
            "all parts",
            "uncertainty of the results",
            with_u > 0,
            format!(
                "{with_u} of {} result(s) with an expanded uncertainty",
                res.len()
            ),
        ),
        t(
            "all parts",
            "bench reference within its calibration interval",
            cal_ok,
            match &s.setup.calibration_due {
                Some(d) => format!("calibration due {d}, test date {}", s.date),
                None => "no calibration due date stated".into(),
            },
        ),
    ]
}

pub fn markdown(rep: &LabReport) -> String {
    let mut s = format!(
        "# Lab test report (common format): {} {}\n\n{} service; test object: {}; {}; RF path {}.\n\n| scenario | metric | value | unit | U (k=2) |\n|---|---|---|---|---|\n",
        rep.lab, rep.report_id, rep.service, rep.test_object, rep.date, rep.setup.rf_path
    );
    for x in &rep.results {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            x.scenario,
            x.metric,
            x.value,
            x.unit,
            x.expanded_u
                .map(|u| u.to_string())
                .unwrap_or_else(|| "-".into())
        ));
    }
    if !rep.missing_core_metrics.is_empty() {
        s.push_str(&format!(
            "\nCore metrics of the {} service not in this report: {}.\n",
            rep.service,
            rep.missing_core_metrics.join(", ")
        ));
    }
    s.push_str("\n## EN 16803 topic index\n\nWhich report contents bear on the topics of each part (cited by part and scope only; not a statement of conformity, and no accreditation scheme is implied).\n\n");
    for t in &rep.en16803_topics {
        s.push_str(&format!(
            "- {} — {}: **{}** ({})\n",
            t.part, t.topic, t.status, t.detail
        ));
    }
    s.push_str(&format!("\n{}\n", rep.label));
    s
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabValue {
    pub lab: String,
    pub report_id: String,
    pub value: f64,
    pub expanded_u: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairCheck {
    pub a: String,
    pub b: String,
    pub difference: f64,
    pub en: Option<f64>,
    /// `consistent`, `inconsistent`, `agree`, `disagree` (booleans) or `not-comparable`.
    pub outcome: String,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub scenario: String,
    pub metric: String,
    pub unit: String,
    pub values: Vec<LabValue>,
    pub pairs: Vec<PairCheck>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub method: &'static str,
    pub label: &'static str,
    pub evidence: &'static str,
    pub reports: Vec<String>,
    pub test_objects: Vec<String>,
    pub rows: Vec<Row>,
    pub consistent: usize,
    pub inconsistent: usize,
    pub not_comparable: usize,
    pub notes: Vec<String>,
}

/// Compare normalised reports.
pub fn compare(reps: &[LabReport]) -> Result<Comparison, String> {
    if reps.len() < 2 {
        return Err("compare needs at least two reports".into());
    }
    let mut notes = Vec::new();
    let objs: std::collections::BTreeSet<&str> =
        reps.iter().map(|x| x.test_object.as_str()).collect();
    if objs.len() > 1 {
        notes.push(
            "the reports test different objects: differences include the objects' own".into(),
        );
    }
    let mut keys: Vec<(String, String, String)> = reps
        .iter()
        .flat_map(|x| {
            x.results
                .iter()
                .map(|y| (y.scenario.clone(), y.metric.clone(), y.unit.clone()))
        })
        .collect();
    keys.sort();
    keys.dedup();
    let (mut ok, mut bad, mut nc) = (0, 0, 0);
    let mut rows = Vec::new();
    for (sc, m, unit) in keys {
        let got: Vec<(&LabReport, &ReportResult)> = reps
            .iter()
            .filter_map(|rp| {
                Some((
                    rp,
                    rp.results
                        .iter()
                        .find(|y| y.scenario == sc && y.metric == m)?,
                ))
            })
            .collect();
        let mut pairs = Vec::new();
        for i in 0..got.len() {
            for j in i + 1..got.len() {
                let ((ra, a), (rb, b)) = (got[i], got[j]);
                let mut reasons = Vec::new();
                if let (Some(x), Some(y)) = (&a.scenario_sha256, &b.scenario_sha256) {
                    if x != y {
                        reasons.push("different scenario files (hash differs)".to_string());
                    }
                } else {
                    reasons.push("scenario file hash not stated by both labs".into());
                }
                if ra.setup.rf_path != rb.setup.rf_path {
                    reasons.push(format!(
                        "RF path differs ({} vs {})",
                        ra.setup.rf_path, rb.setup.rf_path
                    ));
                }
                let diff = b.value - a.value;
                let hard = reasons.iter().any(|x| x.starts_with("different scenario"));
                let (en, outcome) = if hard {
                    (None, "not-comparable")
                } else if unit == "bool" {
                    (None, if diff == 0.0 { "agree" } else { "disagree" })
                } else {
                    match (a.expanded_u, b.expanded_u) {
                        (Some(ua), Some(ub)) if ua.hypot(ub) > 0.0 => {
                            let en = diff.abs() / ua.hypot(ub);
                            (
                                Some(r(en, 3)),
                                if en <= 1.0 {
                                    "consistent"
                                } else {
                                    "inconsistent"
                                },
                            )
                        }
                        _ => {
                            reasons.push("no En: an expanded uncertainty is missing".into());
                            (None, "not-comparable")
                        }
                    }
                };
                match outcome {
                    "consistent" | "agree" => ok += 1,
                    "inconsistent" | "disagree" => bad += 1,
                    _ => nc += 1,
                }
                pairs.push(PairCheck {
                    a: ra.lab.clone(),
                    b: rb.lab.clone(),
                    difference: r(diff, 9),
                    en,
                    outcome: outcome.into(),
                    reasons,
                });
            }
        }
        rows.push(Row {
            scenario: sc,
            metric: m,
            unit,
            values: got
                .iter()
                .map(|(rp, x)| LabValue {
                    lab: rp.lab.clone(),
                    report_id: rp.report_id.clone(),
                    value: x.value,
                    expanded_u: x.expanded_u,
                })
                .collect(),
            pairs,
        });
    }
    Ok(Comparison {
        method: METHOD,
        label: super::LABEL,
        evidence: super::EVIDENCE,
        reports: reps
            .iter()
            .map(|x| format!("{} {}", x.lab, x.report_id))
            .collect(),
        test_objects: objs.into_iter().map(String::from).collect(),
        rows,
        consistent: ok,
        inconsistent: bad,
        not_comparable: nc,
        notes,
    })
}

pub fn compare_markdown(c: &Comparison) -> String {
    let mut s = format!(
        "# Lab comparison\n\nReports: {}. {} consistent or agreeing, {} inconsistent or disagreeing, {} not comparable.\n\n| scenario | metric | lab values | outcome |\n|---|---|---|---|\n",
        c.reports.join("; "),
        c.consistent,
        c.inconsistent,
        c.not_comparable
    );
    for row in &c.rows {
        let vals = row
            .values
            .iter()
            .map(|v| match v.expanded_u {
                Some(u) => format!("{}: {} ± {} {}", v.lab, v.value, u, row.unit),
                None => format!("{}: {} {}", v.lab, v.value, row.unit),
            })
            .collect::<Vec<_>>()
            .join("; ");
        let out = row
            .pairs
            .iter()
            .map(|p| {
                let mut o = p.outcome.clone();
                if let Some(en) = p.en {
                    o.push_str(&format!(" (En {en})"));
                }
                if !p.reasons.is_empty() {
                    o.push_str(&format!(": {}", p.reasons.join("; ")));
                }
                o
            })
            .collect::<Vec<_>>()
            .join(" / ");
        s.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.scenario,
            row.metric,
            vals,
            if out.is_empty() {
                "one lab only".into()
            } else {
                out
            }
        ));
    }
    for n in &c.notes {
        s.push_str(&format!("\nNote: {n}\n"));
    }
    s.push_str(&format!("\n{}\n", c.label));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_booleans() {
        assert_eq!(convert(250.0, "ms", "s").unwrap(), 0.25);
        assert_eq!(convert(1.5, "us", "ns").unwrap(), 1500.0);
        assert_eq!(convert(97.0, "%", "fraction").unwrap(), 0.97);
        assert!(convert(1.0, "dBm", "dB").is_err());
        assert_eq!(parse_bool("Yes"), Some(1.0));
        assert_eq!(parse_bool("maybe"), None);
        assert_eq!(split_csv("a,\"b, c\",d"), vec!["a", "b, c", "d"]);
    }
}
