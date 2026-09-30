// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Spectrum coexistence: a whole signal plan and constellation against the open GNSS signals
//!
//! The free `leo-signal` kind answers, for ONE satellite of a signal design, how much it
//! lowers the carrier-to-noise density (C/N0) of each open global navigation satellite
//! system (GNSS) signal, and the reverse. This study answers the programme question:
//! with every satellite of my constellation that is in view at once, at its maximum
//! received power, does each candidate signal plan stay inside the compatibility limit I
//! must meet, everywhere on Earth, and which plan ranks best?
//!
//! Method (International Telecommunication Union Recommendation ITU-R M.1831; Betz
//! 2001; Kaplan and Hegarty, *Understanding GPS/GNSS*, 3rd edition, section 9.4):
//! effective C/N0 = C / (N0 + I), with I the sum over interfering satellites of received
//! power times the spectral separation coefficient (SSC). Every physical number (SSC,
//! received power, noise density, satellites in view, ranging accuracy) is READ from an
//! open-engine run of `leo-signal` or `constellation-design` through
//! [`kshana::api::run_scenario`]; this module adds only the aggregation arithmetic
//! ([`aggregate`]) and ranks the plans with [`kshana::mcda::pareto`].
//!
//! Scope: friendly-system radio compatibility of open-service navigation signals with
//! published spectra. A study that names jammers, spoofers, attacks or threats is
//! refused, only the open-service victims the open engine models are accepted, and the
//! result carries no jammer-tolerance field (see docs/SPECTRUM-COEXISTENCE.md).

pub mod aggregate;
pub mod cli;
pub mod plan;
pub mod render;

use std::collections::BTreeMap;

use kshana::api::KshanaError;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use aggregate::{aggregate_grid, as_f64, loss_db, ratio_db, scale_to_power, single_ratio};
use plan::{CoverageReading, Plan, SignalReading, VictimReading};

/// The Pro scenario kind (and licence feature) of this study.
pub const KIND: &str = "pro-spectrum-coexistence";

/// Version of the result document's layout.
pub const SCHEMA_VERSION: &str = "1";

/// The open-service GNSS signals a study may name as victims: exactly the signals the
/// open `leo-signal` kind carries a compatibility model for. There is no way to define
/// another victim, and no encrypted or authorised-service signal is among them.
pub const OPEN_VICTIMS: &[&str] = &[
    "gps-l1ca",
    "galileo-e1",
    "gps-l5",
    "galileo-e5a",
    "galileo-e5b",
    "galileo-e5-altboc",
];

/// Table names that mark an adversarial analysis. A study containing any of them, at
/// any depth, is refused.
pub const REFUSED_TABLES: &[&str] = &[
    "jammers", "jammer", "spoofer", "spoofers", "attack", "threat",
];

/// The refusal text for an adversarial table.
pub const REFUSAL: &str =
    "interference analysis of jammers, spoofers, attacks or threats is outside this pack: \
     it answers friendly-system radio compatibility of open-service navigation signals only";

/// The keys a study file may carry at the top level.
const STUDY_KEYS: &[&str] = &[
    "kind",
    "title",
    "max_degradation_db",
    "limit_source",
    "receiver",
    "plan",
    "victims",
    "grid",
];

/// The tolerance within which this pack's own single-satellite loss must reproduce the
/// open run's stated single-satellite loss, or the study stops.
pub const CONSISTENCY_TOLERANCE_DB: f64 = 1e-9;

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// The analysis grid and time span shared by every coverage run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub grid_step_deg: f64,
    pub duration_s: f64,
    pub step_s: f64,
}

/// A parsed, validated study.
#[derive(Debug, Clone, PartialEq)]
pub struct Study {
    pub title: String,
    pub max_degradation_db: f64,
    pub limit_source: String,
    pub receiver: toml::Table,
    pub plans: Vec<Plan>,
    pub victims: Vec<String>,
    pub grid: Grid,
    /// SHA-256 of the canonical JSON form of the parsed study.
    pub scenario_hash: String,
}

/// The GNSS system and open `constellation-design` preset that transmit a victim.
pub fn victim_system(victim: &str) -> (&'static str, &'static str) {
    if victim.starts_with("gps-") {
        ("GPS", "gps-baseline")
    } else {
        ("Galileo", "galileo")
    }
}

/// Refuse a study that carries an adversarial table anywhere in it.
pub fn refuse_threat_tables(t: &toml::Table) -> Result<(), KshanaError> {
    fn walk(v: &toml::Value) -> bool {
        match v {
            toml::Value::Table(t) => t.iter().any(|(k, v)| {
                REFUSED_TABLES
                    .iter()
                    .any(|r| r.eq_ignore_ascii_case(k.trim()))
                    || walk(v)
            }),
            toml::Value::Array(a) => a.iter().any(walk),
            _ => false,
        }
    }
    if walk(&toml::Value::Table(t.clone())) {
        return Err(KshanaError::Unsupported(REFUSAL.to_string()));
    }
    Ok(())
}

/// The canonical JSON form of a value: object keys sorted at every level, compact.
pub fn canonical_json(v: &Value) -> String {
    fn sort(v: &Value) -> Value {
        match v {
            Value::Object(m) => {
                let sorted: BTreeMap<&String, Value> =
                    m.iter().map(|(k, v)| (k, sort(v))).collect();
                let mut out = serde_json::Map::new();
                for (k, v) in sorted {
                    out.insert(k.clone(), v);
                }
                Value::Object(out)
            }
            Value::Array(a) => Value::Array(a.iter().map(sort).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_string(&sort(v)).expect("a JSON value serialises")
}

/// Lower-case hexadecimal SHA-256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn get_f64(t: &toml::Table, key: &str) -> Option<f64> {
    t.get(key)
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
}

impl Study {
    /// Parse a study file.
    pub fn parse(src: &str) -> Result<Study, KshanaError> {
        let t: toml::Table =
            toml::from_str(src).map_err(|e| err(format!("invalid {KIND} study: {e}")))?;
        Study::from_table(&t)
    }

    /// Validate a parsed study table.
    pub fn from_table(t: &toml::Table) -> Result<Study, KshanaError> {
        refuse_threat_tables(t)?;
        match t.get("kind").and_then(toml::Value::as_str) {
            Some(KIND) => {}
            other => {
                return Err(err(format!(
                    "a coexistence study must say kind = \"{KIND}\" (found {other:?})"
                )))
            }
        }
        if let Some(k) = t.keys().find(|k| !STUDY_KEYS.contains(&k.as_str())) {
            return Err(err(format!(
                "unknown key `{k}` in the study (a study takes {})",
                STUDY_KEYS.join(", ")
            )));
        }
        let title = t
            .get("title")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| err("`title` (a non-empty text) is required"))?
            .to_string();
        let max_degradation_db = get_f64(t, "max_degradation_db").ok_or_else(|| {
            err("`max_degradation_db` is required: this pack builds in no limit; state the limit you must meet, in dB")
        })?;
        if !max_degradation_db.is_finite() || max_degradation_db < 0.0 {
            return Err(err(
                "`max_degradation_db` must be a finite number of dB, zero or more",
            ));
        }
        let limit_source = t
            .get("limit_source")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                err("`limit_source` is required: say where the limit comes from (a requirement, an agreement, a recommendation)")
            })?
            .to_string();
        let receiver = match t.get("receiver") {
            None => toml::Table::new(),
            Some(toml::Value::Table(r)) => r.clone(),
            Some(_) => return Err(err("`receiver` must be a table")),
        };
        let victims: Vec<String> = match t.get("victims") {
            Some(toml::Value::Array(a)) if !a.is_empty() => a
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(String::from)
                        .ok_or_else(|| err("`victims` holds signal names (text)"))
                })
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(err(format!(
                    "`victims` is required: a list of open-service GNSS signals from {}",
                    OPEN_VICTIMS.join(", ")
                )))
            }
        };
        for (i, v) in victims.iter().enumerate() {
            if !OPEN_VICTIMS.contains(&v.as_str()) {
                return Err(KshanaError::Unsupported(format!(
                    "victim {v:?} is refused: a victim must be one of the open-service signals the open engine models ({}); \
                     no encrypted or authorised-service signal can be a victim",
                    OPEN_VICTIMS.join(", ")
                )));
            }
            if victims[..i].contains(v) {
                return Err(err(format!("victim {v:?} is listed twice")));
            }
        }
        let g = match t.get("grid") {
            Some(toml::Value::Table(g)) => g,
            _ => {
                return Err(err(
                    "`[grid]` is required, with grid_step_deg, duration_s and step_s",
                ))
            }
        };
        if let Some(k) = g
            .keys()
            .find(|k| !["grid_step_deg", "duration_s", "step_s"].contains(&k.as_str()))
        {
            return Err(err(format!(
                "unknown key `{k}` in [grid] (it takes grid_step_deg, duration_s, step_s)"
            )));
        }
        let gnum = |k: &str| -> Result<f64, KshanaError> {
            let x = get_f64(g, k).ok_or_else(|| err(format!("[grid] needs {k}")))?;
            if !(x.is_finite() && x > 0.0) {
                return Err(err(format!("[grid] {k} must be a positive number")));
            }
            Ok(x)
        };
        let grid = Grid {
            grid_step_deg: gnum("grid_step_deg")?,
            duration_s: gnum("duration_s")?,
            step_s: gnum("step_s")?,
        };
        if grid.grid_step_deg > 90.0 {
            return Err(err("[grid] grid_step_deg must be at most 90 degrees"));
        }
        let plan_tables: Vec<&toml::Table> = match t.get("plan") {
            Some(toml::Value::Array(a)) => a
                .iter()
                .map(|p| {
                    p.as_table()
                        .ok_or_else(|| err("each [[plan]] must be a table"))
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(err("`plan` must be an array of tables ([[plan]])")),
            None => vec![],
        };
        if plan_tables.is_empty() {
            return Err(err(
                "the study has no candidate plan: add at least one [[plan]]",
            ));
        }
        let plans = plan_tables
            .iter()
            .enumerate()
            .map(|(i, p)| Plan::from_table(p, i))
            .collect::<Result<Vec<_>, _>>()?;
        for (i, p) in plans.iter().enumerate() {
            if plans[..i].iter().any(|q| q.label == p.label) {
                return Err(err(format!("plan label {:?} is used twice", p.label)));
            }
        }
        let as_json = serde_json::to_value(t).map_err(|e| err(e.to_string()))?;
        let scenario_hash = sha256_hex(canonical_json(&as_json).as_bytes());
        Ok(Study {
            title,
            max_degradation_db,
            limit_source,
            receiver,
            plans,
            victims,
            grid,
            scenario_hash,
        })
    }
}

/// Every open run the study makes, with its hashes.
struct Runner {
    members: Vec<Value>,
}

impl Runner {
    fn run(&mut self, role: String, kind: &str, src: &str) -> Result<Value, KshanaError> {
        let out = kshana::api::run_scenario(src)
            .map_err(|e| err(format!("{role}: the open {kind} run failed: {e}")))?;
        let v: Value = serde_json::from_str(&out.json)
            .map_err(|e| err(format!("{role}: the open {kind} result is not JSON: {e}")))?;
        let got = v.get("kind").and_then(Value::as_str);
        if got != Some(kind) {
            return Err(err(format!(
                "{role}: expected an open {kind} result, got {got:?}"
            )));
        }
        self.members.push(json!({
            "role": role,
            "kind": kind,
            "scenario_sha256": sha256_hex(src.as_bytes()),
            "result_sha256": sha256_hex(out.json.as_bytes()),
        }));
        Ok(v)
    }
}

fn verdict(worst: Option<f64>, reason: Option<&str>, limit: f64) -> Value {
    match (worst, reason) {
        (Some(w), _) if w <= limit => json!({
            "status": "compatible",
            "basis": "the worst cell is at or below the limit",
            "margin_db": limit - w,
        }),
        (Some(w), _) => json!({
            "status": "not compatible",
            "basis": "the worst cell is above the limit",
            "margin_db": limit - w,
        }),
        (None, Some(r)) => json!({
            "status": "compatible",
            "basis": format!("{r}: no interference to compare with the limit"),
            "margin_db": Value::Null,
        }),
        (None, None) => json!({
            "status": "compatible",
            "basis": "no interference to compare with the limit",
            "margin_db": Value::Null,
        }),
    }
}

const NO_OVERLAP: &str = "no spectral overlap";
const NEVER_IN_VIEW: &str = "no satellite is ever in view above the mask";

fn cell(cov: &CoverageReading, w: &aggregate::Worst) -> Value {
    json!({
        "lat_deg": cov.lat_deg[w.lat_index],
        "lon_deg": cov.lon_deg[w.lon_index],
        "satellites_in_view": w.in_view,
    })
}

/// One direction of one victim for one plan, aggregated over a coverage grid.
struct Aggregated {
    worst_loss: Option<f64>,
    json: Value,
    grid_loss: Option<Vec<Vec<Option<f64>>>>,
}

#[allow(clippy::too_many_arguments)]
fn aggregate_direction(
    victim: &str,
    ssc: Option<f64>,
    single_ref: Option<f64>,
    single_bound: Option<f64>,
    cov: &CoverageReading,
    limit: f64,
    engine_single: Option<f64>,
    consistency: String,
    extra: serde_json::Map<String, Value>,
) -> Aggregated {
    let mut j = serde_json::Map::new();
    j.insert("gnss".into(), json!(victim));
    for (k, v) in extra {
        j.insert(k, v);
    }
    j.insert("spectral_overlap".into(), json!(ssc.is_some()));
    j.insert("ssc_db_per_hz".into(), json!(ssc));
    j.insert(
        "single_satellite_inr_db".into(),
        json!(single_ref.map(ratio_db)),
    );
    j.insert(
        "single_satellite_loss_db".into(),
        json!(single_ref.map(loss_db)),
    );
    j.insert(
        "engine_single_satellite_loss_db".into(),
        json!(engine_single),
    );
    j.insert("consistency".into(), json!(consistency));
    let (worst_loss, grid_loss, reason) = match (single_ref, single_bound) {
        (Some(r_ref), Some(r_bound)) => {
            let (g, worst) = aggregate_grid(&as_f64(&cov.max_visible), r_bound);
            let (_, worst_mean) = aggregate_grid(&cov.mean_visible, r_ref);
            match worst {
                Some(w) => {
                    j.insert("worst_loss_db".into(), json!(w.loss_db));
                    j.insert("worst_inr_db".into(), json!(ratio_db(w.ratio)));
                    j.insert("worst_cell".into(), cell(cov, &w));
                    j.insert(
                        "worst_mean_case_loss_db".into(),
                        json!(worst_mean.map(|m| m.loss_db)),
                    );
                    j.insert("reason".into(), Value::Null);
                    (Some(w.loss_db), Some(g), None)
                }
                None => {
                    j.insert("worst_loss_db".into(), Value::Null);
                    j.insert("worst_inr_db".into(), Value::Null);
                    j.insert("worst_cell".into(), Value::Null);
                    j.insert("worst_mean_case_loss_db".into(), Value::Null);
                    j.insert("reason".into(), json!(NEVER_IN_VIEW));
                    (None, Some(g), Some(NEVER_IN_VIEW))
                }
            }
        }
        _ => {
            j.insert("worst_loss_db".into(), Value::Null);
            j.insert("worst_inr_db".into(), Value::Null);
            j.insert("worst_cell".into(), Value::Null);
            j.insert("worst_mean_case_loss_db".into(), Value::Null);
            j.insert("reason".into(), json!(NO_OVERLAP));
            (None, None, Some(NO_OVERLAP))
        }
    };
    j.insert("verdict".into(), verdict(worst_loss, reason, limit));
    Aggregated {
        worst_loss,
        json: Value::Object(j),
        grid_loss,
    }
}

fn combine_verdicts(entries: &[Value], what: &str) -> Value {
    let failing: Vec<&str> = entries
        .iter()
        .filter(|e| e["verdict"]["status"] == "not compatible")
        .filter_map(|e| e["gnss"].as_str())
        .collect();
    if failing.is_empty() {
        json!({
            "status": "compatible",
            "basis": format!("every victim signal is compatible {what}"),
            "failing": [],
        })
    } else {
        json!({
            "status": "not compatible",
            "basis": format!("the worst cell is above the limit {what} for {}", failing.join(", ")),
            "failing": failing,
        })
    }
}

/// Run a study given as TOML text: the result document.
pub fn run_study(src: &str) -> Result<Value, KshanaError> {
    run(&Study::parse(src)?)
}

/// Run a study given as a parsed TOML table: the result document.
pub fn run_study_table(t: &toml::Table) -> Result<Value, KshanaError> {
    run(&Study::from_table(t)?)
}

/// The result document as written to `coexistence.result.json` and printed by `run`.
pub fn to_json_string(v: &Value) -> String {
    serde_json::to_string_pretty(v).expect("a JSON value serialises") + "\n"
}

/// Run a validated study.
pub fn run(study: &Study) -> Result<Value, KshanaError> {
    let mut runner = Runner { members: vec![] };
    let limit = study.max_degradation_db;
    let mut engine_version: Option<String> = None;
    let mut grid_axes: Option<(Vec<f64>, Vec<f64>)> = None;
    let mut gnss_cov: BTreeMap<(String, u64), CoverageReading> = BTreeMap::new();
    let mut gnss_json: Vec<Value> = vec![];
    let mut plans_json: Vec<Value> = vec![];
    let mut pareto_rows: Vec<(String, Option<f64>, Option<f64>, u64)> = vec![];

    for plan in &study.plans {
        let label = &plan.label;
        // (1) The plan's signal: SSCs both ways, single-satellite losses, powers.
        let leo = runner.run(
            format!("plan '{label}': signal"),
            "leo-signal",
            &plan.signal_scenario(&study.receiver, &study.victims),
        )?;
        let sig = SignalReading::read(&leo, plan.signal_name().as_deref(), label)?;
        if engine_version.is_none() {
            engine_version = sig.engine_version.clone();
        }
        let n0 = sig.noise_density_dbw_per_hz;
        let (p_ref, p_ref_basis) = match (plan.received_power_dbw, sig.received_power_dbw) {
            (Some(p), _) => (p, "input: the plan's received_power_dbw".to_string()),
            (None, Some(p)) => (
                p,
                "read: the signal design's received_power_dbw in the open leo-signal run".to_string(),
            ),
            (None, None) => (
                sig.reference_cn0_dbhz + n0,
                format!(
                    "read: the design states no received power, so the open leo-signal run's reference C/N0 ({}) plus its noise density",
                    sig.reference_cn0_source.as_deref().unwrap_or("reference_cn0_dbhz")
                ),
            ),
        };
        let (p_max, p_max_basis) = match (plan.received_power_max_dbw, sig.received_power_max_dbw) {
            (Some(p), _) => (p, "input: the plan's received_power_max_dbw".to_string()),
            (None, Some(p)) => (
                p,
                "read: the signal design's received_power_max_dbw in the open leo-signal run"
                    .to_string(),
            ),
            (None, None) => (
                p_ref,
                "no maximum received power stated: the bound uses the reference power".to_string(),
            ),
        };
        // A satellite signal at a ground receiver is far below 1 W. A value at or above
        // 0 dBW is an EIRP or a value in the wrong unit, and would otherwise run to a
        // meaningless loss (or overflow to an infinite ratio).
        for (what, p, basis) in [
            ("reference", p_ref, &p_ref_basis),
            ("maximum", p_max, &p_max_basis),
        ] {
            if p >= 0.0 {
                return Err(err(format!(
                    "plan '{label}': the {what} received power {p} dBW ({basis}) is 0 dBW (1 W) or more; \
                     a received power at the user antenna is far weaker: check it is a received power \
                     in dBW, not an EIRP or a value in another unit"
                )));
            }
        }
        if p_max < p_ref {
            return Err(err(format!(
                "plan '{label}': the maximum received power {p_max} dBW is below the reference power {p_ref} dBW"
            )));
        }

        // (2) The plan's constellation: numbers in view per cell.
        let cov = CoverageReading::read(
            &runner.run(
                format!("plan '{label}': constellation"),
                "constellation-design",
                &plan.coverage_scenario(&study.grid),
            )?,
            &format!("plan '{label}'"),
        )?;
        match &grid_axes {
            None => grid_axes = Some((cov.lat_deg.clone(), cov.lon_deg.clone())),
            Some((la, lo)) if *la == cov.lat_deg && *lo == cov.lon_deg => {}
            Some(_) => {
                return Err(err(format!(
                    "plan '{label}': the coverage grid differs from the first plan's"
                )))
            }
        }

        // (3) and (5): the interference this plan causes to each victim.
        let reading = |v: &str| -> Result<&VictimReading, KshanaError> {
            sig.victims.iter().find(|r| r.gnss == v).ok_or_else(|| {
                err(format!(
                    "plan '{label}': the open leo-signal run has no compatibility row for {v}"
                ))
            })
        };
        let mut caused = vec![];
        let mut map: Option<Vec<Vec<Option<f64>>>> = None;
        let mut worst_caused: Option<f64> = None;
        for v in &study.victims {
            let r = reading(v)?;
            let single_ref = r
                .ssc_into_gnss_db_per_hz
                .map(|k| single_ratio(p_ref, k, n0));
            let single_max = single_ref.map(|x| scale_to_power(x, p_ref, p_max));
            // The pack's own single-satellite loss at the design's power must reproduce
            // the open run's to within the tolerance, or nothing is reported.
            let consistency = match (sig.received_power_dbw, r.ssc_into_gnss_db_per_hz, r.engine_gnss_loss_db) {
                (Some(pd), Some(k), Some(e)) => {
                    let mine = loss_db(single_ratio(pd, k, n0));
                    if (mine - e).abs() > CONSISTENCY_TOLERANCE_DB {
                        return Err(err(format!(
                            "plan '{label}', {v}: the single-satellite loss rebuilt from the open run's SSC ({mine} dB) \
                             does not reproduce the run's own {e} dB"
                        )));
                    }
                    format!("reproduces the open run's single-satellite loss at the design's received power to {CONSISTENCY_TOLERANCE_DB:e} dB")
                }
                (Some(_), Some(_), None) => {
                    return Err(err(format!(
                        "plan '{label}', {v}: the open run states an SSC and a received power but no single-satellite loss"
                    )))
                }
                (_, None, _) => "not applicable: no spectral overlap".to_string(),
                (None, Some(_), _) => "not applicable: the design states no received power, so the open run states no single-satellite loss".to_string(),
            };
            let a = aggregate_direction(
                v,
                r.ssc_into_gnss_db_per_hz,
                single_ref,
                single_max,
                &cov,
                limit,
                r.engine_gnss_loss_db,
                consistency,
                serde_json::Map::new(),
            );
            if let Some(w) = a.worst_loss {
                worst_caused = Some(worst_caused.map_or(w, |c: f64| c.max(w)));
            }
            if let Some(g) = a.grid_loss {
                map = Some(match map {
                    None => g,
                    Some(m) => m
                        .iter()
                        .zip(&g)
                        .map(|(a, b)| {
                            a.iter()
                                .zip(b)
                                .map(|(x, y)| match (x, y) {
                                    (Some(x), Some(y)) => Some(x.max(*y)),
                                    (x, y) => x.or(*y),
                                })
                                .collect()
                        })
                        .collect(),
                });
            }
            caused.push(a.json);
        }

        // (4): the interference the open GNSS constellations cause to this plan.
        let mut received = vec![];
        for v in &study.victims {
            let r = reading(v)?;
            let (system, preset) = victim_system(v);
            let key = (system.to_string(), plan.mask_deg.to_bits());
            if !gnss_cov.contains_key(&key) {
                let c = CoverageReading::read(
                    &runner.run(
                        format!("{system} ({preset}) at a {} deg mask", plan.mask_deg),
                        "constellation-design",
                        &plan::gnss_coverage_scenario(&study.grid, plan.mask_deg, system, preset),
                    )?,
                    system,
                )?;
                gnss_json.push(json!({
                    "system": system,
                    "preset": preset,
                    "mask_deg": plan.mask_deg,
                    "total_satellites": c.total_satellites,
                    "most_in_view": c.max_visible.iter().flatten().max().copied().unwrap_or(0),
                    "grid_max_in_view": c.max_visible,
                    "grid_mean_in_view": c.mean_visible,
                }));
                gnss_cov.insert(key.clone(), c);
            }
            let gc = &gnss_cov[&key];
            let single = r
                .ssc_from_gnss_db_per_hz
                .map(|k| single_ratio(r.gnss_received_power_dbw, k, n0));
            let consistency = match (single, r.engine_plan_loss_db) {
                (Some(s), Some(e)) => {
                    let mine = loss_db(s);
                    if (mine - e).abs() > CONSISTENCY_TOLERANCE_DB {
                        return Err(err(format!(
                            "plan '{label}', {v} into the plan: the rebuilt single-satellite loss ({mine} dB) \
                             does not reproduce the run's own {e} dB"
                        )));
                    }
                    format!("reproduces the open run's single-satellite loss to {CONSISTENCY_TOLERANCE_DB:e} dB")
                }
                (Some(_), None) => {
                    return Err(err(format!(
                        "plan '{label}', {v} into the plan: the open run states an SSC but no single-satellite loss"
                    )))
                }
                (None, _) => "not applicable: no spectral overlap".to_string(),
            };
            let mut extra = serde_json::Map::new();
            extra.insert("gnss_system".into(), json!(system));
            extra.insert("gnss_preset".into(), json!(preset));
            extra.insert(
                "gnss_received_power_dbw".into(),
                json!(r.gnss_received_power_dbw),
            );
            let a = aggregate_direction(
                v,
                r.ssc_from_gnss_db_per_hz,
                single,
                single,
                gc,
                limit,
                r.engine_plan_loss_db,
                consistency,
                extra,
            );
            received.push(a.json);
        }

        let verdict_caused = combine_verdicts(&caused, "with the interference this plan causes");
        let verdict_received =
            combine_verdicts(&received, "with the interference it causes to this plan");
        pareto_rows.push((
            label.clone(),
            sig.ranging_accuracy_m,
            worst_caused,
            cov.total_satellites,
        ));
        let empty_map: Vec<Vec<Option<f64>>> = cov
            .max_visible
            .iter()
            .map(|r| vec![None; r.len()])
            .collect();
        plans_json.push(json!({
            "label": label,
            "signal": {
                "name": sig.name,
                "source": sig.source,
                "centre_hz": sig.centre_hz,
                "tx_bandwidth_hz": sig.tx_bandwidth_hz,
                "ranging_accuracy_m": sig.ranging_accuracy_m,
                "reference_cn0_dbhz": sig.reference_cn0_dbhz,
                "noise_density_dbw_per_hz": n0,
                "reference_power_dbw": p_ref,
                "reference_power_basis": p_ref_basis,
                "max_power_dbw": p_max,
                "max_power_basis": p_max_basis,
            },
            "constellation": {
                "total_satellites": cov.total_satellites,
                "mask_deg": plan.mask_deg,
                "most_in_view": cov.max_visible.iter().flatten().max().copied().unwrap_or(0),
                "grid_max_in_view": cov.max_visible,
                "grid_mean_in_view": cov.mean_visible,
            },
            "caused": caused,
            "received": received,
            "worst_loss_caused_db": worst_caused,
            "map_loss_db": map.unwrap_or(empty_map),
            "verdict_caused": verdict_caused,
            "verdict_received": verdict_received,
        }));
    }

    // (6) The Pareto front of (ranging accuracy, worst loss caused, satellite count).
    let pareto = pareto_json(&pareto_rows);
    let (lat, lon) = grid_axes.unwrap_or_default();
    let map_plan = plans_json
        .iter()
        .max_by(|a, b| {
            let fa = a["worst_loss_caused_db"]
                .as_f64()
                .unwrap_or(f64::NEG_INFINITY);
            let fb = b["worst_loss_caused_db"]
                .as_f64()
                .unwrap_or(f64::NEG_INFINITY);
            // Ties keep the earlier plan.
            fa.partial_cmp(&fb)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(std::cmp::Ordering::Greater)
        })
        .and_then(|p| p["label"].as_str())
        .unwrap_or_default()
        .to_string();
    let compatible = plans_json
        .iter()
        .filter(|p| p["verdict_caused"]["status"] == "compatible")
        .count();

    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "kind": KIND,
        "overlay": "kshana-pro",
        "overlay_version": crate::version(),
        "engine_version": engine_version,
        "scenario_hash": study.scenario_hash,
        "title": study.title,
        "label": LABEL,
        "bound": BOUND_STATEMENT,
        "limit": {
            "max_degradation_db": limit,
            "limit_source": study.limit_source,
            "applies_to": "the worst cell of each victim signal, in each direction",
        },
        "victims": study.victims,
        "grid": {
            "grid_step_deg": study.grid.grid_step_deg,
            "duration_s": study.grid.duration_s,
            "step_s": study.grid.step_s,
            "lat_deg": lat,
            "lon_deg": lon,
        },
        "summary": {
            "plans": plans_json.len(),
            "plans_compatible": compatible,
            "map_plan": map_plan,
        },
        "plans": plans_json,
        "gnss_constellations": gnss_json,
        "pareto": pareto,
        "figure_tiers": {"figures": figure_tiers()},
        "not_modelled": NOT_MODELLED,
        "members": runner.members,
        "units": units(),
    }))
}

fn pareto_json(rows: &[(String, Option<f64>, Option<f64>, u64)]) -> Value {
    use kshana::mcda::Objective;
    let eligible: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].1.is_some()).collect();
    let points: Vec<Vec<f64>> = eligible
        .iter()
        .map(|&i| {
            let (_, ranging, worst, n) = &rows[i];
            vec![ranging.unwrap_or_default(), worst.unwrap_or(0.0), *n as f64]
        })
        .collect();
    let front_local = kshana::mcda::pareto::pareto_front(
        &points,
        &[Objective::Min, Objective::Min, Objective::Min],
    );
    let on_front: Vec<usize> = front_local.iter().map(|&k| eligible[k]).collect();
    json!({
        "objectives": [
            {"name": "ranging_accuracy_m", "sense": "minimise", "source": "the plan's leo-signal run, tracking.ranging_accuracy_m"},
            {"name": "loss_for_ranking_db", "sense": "minimise", "source": "the plan's worst upper-bound loss caused to any victim"},
            {"name": "total_satellites", "sense": "minimise", "source": "the plan's constellation-design run"},
        ],
        "method": "kshana::mcda::pareto::pareto_front (non-dominated set, every objective minimised)",
        "note": "A plan with no spectral overlap with any victim ranks at 0 dB loss (it causes no interference); its reported loss stays null. A plan with no ranging accuracy (a Doppler-only signal) is left out of the ranking.",
        "points": rows.iter().enumerate().map(|(i, (label, ranging, worst, n))| json!({
            "plan": label,
            "ranging_accuracy_m": ranging,
            "loss_for_ranking_db": if ranging.is_some() { json!(worst.unwrap_or(0.0)) } else { Value::Null },
            "total_satellites": n,
            "on_front": on_front.contains(&i),
            "excluded_reason": if ranging.is_some() { Value::Null } else { json!("the signal states no ranging accuracy (Doppler-only)") },
        })).collect::<Vec<_>>(),
        "front": on_front.iter().map(|&i| rows[i].0.clone()).collect::<Vec<_>>(),
    })
}

/// The run-level label.
pub const LABEL: &str = "MODELLED: aggregate radio-frequency compatibility of candidate signal plans with open-service global navigation satellite system (GNSS) signals. \
The spectral separation coefficients and single-satellite losses underneath are read from the open engine's leo-signal kind, \
which computes them from its VALIDATED closed forms (Betz 2001 spectral separation, band-limited power spectral densities); \
the engine labels those compatibility figures MODELLED. The aggregation over the satellites in view is a MODELLED upper bound \
(every visible satellite at the maximum received power). Not a coordination filing and not a regulatory finding.";

/// The statement of what the bound assumes.
pub const BOUND_STATEMENT: &str = "An upper bound, not a prediction: in the direction the plan causes, every satellite in view at a cell \
is taken at the plan's maximum received power, with no elevation-dependent power and no receive-antenna pattern, and the count is the \
most satellites in view at that cell at any epoch. The mean case uses the mean number in view at the reference power. In the direction \
the plan receives, the open GNSS presets' satellites in view are each taken at the GNSS minimum received power the open engine states \
(the interface-specification minimum), so that direction is a most-in-view figure at minimum GNSS power, not a power bound.";

/// What the study does not model.
pub const NOT_MODELLED: &[&str] = &[
    "Elevation-dependent received power: every visible satellite is taken at the maximum received power.",
    "The receive-antenna gain pattern of the victim receiver.",
    "Spreading-code line structure: the spectra are the smooth power spectral densities of the open engine.",
    "Out-of-band emission beyond the open engine's ideal band edge.",
    "Which satellites of an open GNSS preset transmit which signal: every satellite of the preset is taken to transmit every signal of its system.",
    "GNSS received power above the interface-specification minimum, in the direction the plan receives.",
    "The sum over several GNSS signals at one receiver of the plan: each victim signal is reported on its own (Galileo E5a, E5b and the E5 AltBOC signal are one transmission).",
    "Interference from any other system (other GNSS, other low Earth orbit systems, terrestrial services).",
    "Time correlation: the count per cell is the most in view at any epoch of the run.",
    "Coordination, filing or regulatory status: the result is not a coordination filing and not a regulatory finding.",
];

fn figure_tiers() -> Vec<Value> {
    let f = |path: &str, requirement: Option<&str>, tier: &str| json!({"path": path, "requirement": requirement, "tier": tier});
    vec![
        f(
            "plans[].caused[].worst_loss_db",
            Some("max_degradation_db"),
            "MODELLED",
        ),
        f("plans[].caused[].worst_inr_db", None, "MODELLED"),
        f("plans[].caused[].worst_mean_case_loss_db", None, "MODELLED"),
        f(
            "plans[].caused[].single_satellite_loss_db",
            None,
            "MODELLED",
        ),
        f("plans[].caused[].ssc_db_per_hz", None, "MODELLED"),
        f(
            "plans[].received[].worst_loss_db",
            Some("max_degradation_db"),
            "MODELLED",
        ),
        f(
            "plans[].received[].worst_mean_case_loss_db",
            None,
            "MODELLED",
        ),
        f(
            "plans[].received[].single_satellite_loss_db",
            None,
            "MODELLED",
        ),
        f("plans[].received[].ssc_db_per_hz", None, "MODELLED"),
        f(
            "plans[].worst_loss_caused_db",
            Some("max_degradation_db"),
            "MODELLED",
        ),
        f("plans[].map_loss_db[][]", None, "MODELLED"),
        f("plans[].signal.ranging_accuracy_m", None, "MODELLED"),
        f(
            "plans[].constellation.grid_max_in_view[][]",
            None,
            "MODELLED",
        ),
        f("pareto.front[]", None, "MODELLED"),
    ]
}

/// `{unit, provenance, note}` for every numeric field of the result, keyed by path.
pub fn units() -> Value {
    let rows: &[(&str, &str, &str, &str)] = &[
        ("limit.max_degradation_db", "dB", "input", "the compatibility limit the study must meet, with its stated source"),
        ("grid.grid_step_deg", "deg", "input", "latitude and longitude step of the coverage grid"),
        ("grid.duration_s", "s", "input", "time span of every coverage run"),
        ("grid.step_s", "s", "input", "time step of every coverage run"),
        ("grid.lat_deg[]", "deg", "read", "cell-centre latitudes, from the constellation-design runs"),
        ("grid.lon_deg[]", "deg", "read", "cell-centre longitudes, from the constellation-design runs"),
        ("summary.plans", "count", "aggregate", "candidate plans in the study"),
        ("summary.plans_compatible", "count", "aggregate", "plans compatible with every victim in the direction they cause"),
        ("plans[].signal.centre_hz", "Hz", "read", "carrier of the plan's signal (leo-signal run)"),
        ("plans[].signal.tx_bandwidth_hz", "Hz", "read", "transmit bandwidth of the plan's signal (leo-signal run)"),
        ("plans[].signal.ranging_accuracy_m", "m", "read", "code-tracking ranging accuracy at the reference C/N0 (leo-signal run, tracking.ranging_accuracy_m); null for a Doppler-only signal"),
        ("plans[].signal.reference_cn0_dbhz", "dB-Hz", "read", "the signal's reference C/N0 in the leo-signal run"),
        ("plans[].signal.noise_density_dbw_per_hz", "dBW/Hz", "read", "receiver thermal noise density N0 (leo-signal run)"),
        ("plans[].signal.reference_power_dbw", "dBW", "read or input", "received power of one satellite for the mean case; its basis is stated beside it"),
        ("plans[].signal.max_power_dbw", "dBW", "read or input", "received power of every satellite in view for the upper bound; its basis is stated beside it"),
        ("plans[].constellation.total_satellites", "count", "read", "satellites in the plan's constellation (constellation-design run)"),
        ("plans[].constellation.mask_deg", "deg", "input", "elevation mask of the user receiver"),
        ("plans[].constellation.most_in_view", "count", "read", "most satellites of the plan in view at any cell and epoch"),
        ("plans[].constellation.grid_max_in_view[][]", "count", "read", "most satellites of the plan in view per cell, [lat][lon] (constellation-design grid.max_visible)"),
        ("plans[].constellation.grid_mean_in_view[][]", "count", "read", "mean satellites of the plan in view per cell, [lat][lon] (constellation-design grid.mean_visible)"),
        ("plans[].caused[].ssc_db_per_hz", "dB/Hz", "read", "SSC of the plan's signal into the victim over the victim's receiver band (leo-signal run); null when the bands do not overlap"),
        ("plans[].caused[].single_satellite_inr_db", "dB", "aggregate", "interference-to-noise ratio of one satellite at the reference power, P x SSC / N0"),
        ("plans[].caused[].single_satellite_loss_db", "dB", "aggregate", "loss of the victim's C/N0 from one satellite at the reference power, 10 log10(1 + P x SSC / N0)"),
        ("plans[].caused[].engine_single_satellite_loss_db", "dB", "read", "the leo-signal run's own single-satellite loss, when the design states a received power"),
        ("plans[].caused[].worst_loss_db", "dB", "aggregate", "upper bound on the victim's C/N0 loss at the worst cell: 10 log10(1 + most in view x single ratio at the maximum power)"),
        ("plans[].caused[].worst_inr_db", "dB", "aggregate", "aggregate interference-to-noise ratio at the worst cell (upper bound)"),
        ("plans[].caused[].worst_mean_case_loss_db", "dB", "aggregate", "largest mean-case loss over the cells: mean number in view at the reference power"),
        ("plans[].caused[].worst_cell.lat_deg", "deg", "read", "latitude of the worst cell"),
        ("plans[].caused[].worst_cell.lon_deg", "deg", "read", "longitude of the worst cell"),
        ("plans[].caused[].worst_cell.satellites_in_view", "count", "read", "most satellites in view at the worst cell"),
        ("plans[].caused[].verdict.margin_db", "dB", "aggregate", "limit minus the worst-cell loss; null when there is no interference"),
        ("plans[].received[].gnss_received_power_dbw", "dBW", "read", "GNSS minimum received power the leo-signal run states (interface specification)"),
        ("plans[].received[].ssc_db_per_hz", "dB/Hz", "read", "SSC of the GNSS signal into the plan's tracked component (leo-signal run); null when the bands do not overlap"),
        ("plans[].received[].single_satellite_inr_db", "dB", "aggregate", "interference-to-noise ratio of one GNSS satellite at its minimum power"),
        ("plans[].received[].single_satellite_loss_db", "dB", "aggregate", "loss of the plan's C/N0 from one GNSS satellite at its minimum power"),
        ("plans[].received[].engine_single_satellite_loss_db", "dB", "read", "the leo-signal run's own single-satellite loss of the plan's C/N0"),
        ("plans[].received[].worst_loss_db", "dB", "aggregate", "loss of the plan's C/N0 at the worst cell: most GNSS satellites of that system in view, each at the GNSS minimum power"),
        ("plans[].received[].worst_inr_db", "dB", "aggregate", "aggregate interference-to-noise ratio at the worst cell"),
        ("plans[].received[].worst_mean_case_loss_db", "dB", "aggregate", "largest mean-case loss over the cells: mean GNSS satellites in view at the minimum power"),
        ("plans[].received[].worst_cell.lat_deg", "deg", "read", "latitude of the worst cell"),
        ("plans[].received[].worst_cell.lon_deg", "deg", "read", "longitude of the worst cell"),
        ("plans[].received[].worst_cell.satellites_in_view", "count", "read", "most GNSS satellites in view at the worst cell"),
        ("plans[].received[].verdict.margin_db", "dB", "aggregate", "limit minus the worst-cell loss; null when there is no interference"),
        ("plans[].worst_loss_caused_db", "dB", "aggregate", "largest worst-cell upper bound over the victims; null when no victim overlaps or nothing is in view"),
        ("plans[].map_loss_db[][]", "dB", "aggregate", "per cell, the largest upper-bound loss over the overlapping victims, [lat][lon]; null where nothing overlaps or nothing is in view"),
        ("gnss_constellations[].mask_deg", "deg", "input", "elevation mask of the GNSS coverage run (the plan's mask)"),
        ("gnss_constellations[].total_satellites", "count", "read", "satellites in the open GNSS preset"),
        ("gnss_constellations[].most_in_view", "count", "read", "most satellites of the preset in view at any cell and epoch"),
        ("gnss_constellations[].grid_max_in_view[][]", "count", "read", "most satellites of the preset in view per cell, [lat][lon]"),
        ("gnss_constellations[].grid_mean_in_view[][]", "count", "read", "mean satellites of the preset in view per cell, [lat][lon]"),
        ("pareto.points[].ranging_accuracy_m", "m", "read", "ranging accuracy of the plan's signal"),
        ("pareto.points[].loss_for_ranking_db", "dB", "aggregate", "worst loss caused, as ranked (0 dB for a plan with no spectral overlap)"),
        ("pareto.points[].total_satellites", "count", "read", "satellites in the plan's constellation"),
    ];
    let mut m = serde_json::Map::new();
    for (path, unit, provenance, note) in rows {
        m.insert(
            (*path).to_string(),
            json!({"unit": unit, "provenance": provenance, "note": note}),
        );
    }
    Value::Object(m)
}

/// A one-line summary of a result document.
pub fn summary_line(result: &Value) -> String {
    format!(
        "Kshana Pro spectrum coexistence | {} | {}/{} plans compatible in the direction they cause, against {} dB ({}) | MODELLED upper bound",
        result["title"].as_str().unwrap_or_default(),
        result["summary"]["plans_compatible"],
        result["summary"]["plans"],
        result["limit"]["max_degradation_db"],
        result["limit"]["limit_source"].as_str().unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threat_tables_are_refused_at_any_depth() {
        let t: toml::Table = toml::from_str("[[plan]]\n[[plan.jammers]]\nx = 1\n").unwrap();
        assert!(refuse_threat_tables(&t).is_err());
        let t: toml::Table = toml::from_str("[Threat]\nx = 1\n").unwrap();
        assert!(refuse_threat_tables(&t).is_err());
        let t: toml::Table = toml::from_str("[grid]\nx = 1\n").unwrap();
        assert!(refuse_threat_tables(&t).is_ok());
    }

    #[test]
    fn canonical_json_sorts_keys() {
        let v = json!({"b": 1, "a": {"d": 2, "c": [ {"z": 1, "y": 2} ]}});
        assert_eq!(
            canonical_json(&v),
            r#"{"a":{"c":[{"y":2,"z":1}],"d":2},"b":1}"#
        );
    }

    #[test]
    fn verdict_edges() {
        assert_eq!(verdict(Some(1.0), None, 1.0)["status"], "compatible");
        assert_eq!(
            verdict(Some(1.0 + 1e-12), None, 1.0)["status"],
            "not compatible"
        );
        let v = verdict(None, Some(NO_OVERLAP), 1.0);
        assert_eq!(v["status"], "compatible");
        assert!(v["basis"].as_str().unwrap().contains("no spectral overlap"));
    }

    #[test]
    fn units_cover_the_figure_tier_paths() {
        let u = units();
        for f in figure_tiers() {
            let p = f["path"].as_str().unwrap();
            if p.starts_with("pareto.front") {
                continue;
            }
            assert!(u.get(p).is_some(), "no units row for {p}");
        }
    }
}
