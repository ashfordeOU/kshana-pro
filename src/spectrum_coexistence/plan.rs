// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! A candidate signal plan: one signal design plus one constellation, and the two open
//! runs that describe it.
//!
//! The plan's signal goes to the open `leo-signal` kind unchanged (a preset signal name,
//! or an inline table in that kind's own schema), and its constellation goes to the open
//! `constellation-design` kind unchanged (its `shell` and `satellite` tables). Every
//! physical number this pack uses is read back from those runs; nothing here computes
//! a spectrum, a coefficient or a visibility.

use kshana::api::KshanaError;
use serde_json::Value;

/// The signal-preset files the open `leo-signal` kind carries. A plan that names a
/// preset signal is run against all of them and its signal is picked out by name, so
/// the open engine (not this list) decides whether the name exists. A test holds this
/// list to the one the engine reports.
pub const LEO_SIGNAL_PRESET_FILES: &[&str] = &[
    "xona-pulsar",
    "iridium-stl",
    "starlink-soo",
    "centispace",
    "generic-c-band",
    "generic-bands",
];

/// The keys a `[[plan]]` table may carry.
const PLAN_KEYS: &[&str] = &[
    "label",
    "signal",
    "mask_deg",
    "shell",
    "satellite",
    "j2",
    "received_power_dbw",
    "received_power_max_dbw",
];

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(msg.into())
}

/// Where a plan's signal design comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalSource {
    /// A signal of one of the open engine's compiled-in presets, by its name.
    Preset(String),
    /// An inline signal table in the `leo-signal` schema, passed through unchanged.
    Inline(toml::Table),
}

/// One candidate plan as the study file states it.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub label: String,
    pub signal: SignalSource,
    /// Elevation mask (degrees) of the user receiver, for both coverage runs.
    pub mask_deg: f64,
    /// `constellation-design` Walker shell tables, unchanged.
    pub shells: Vec<toml::Value>,
    /// `constellation-design` explicit element sets, unchanged.
    pub satellites: Vec<toml::Value>,
    /// Secular J2 drift in the coverage run (the open kind's `j2`).
    pub j2: Option<bool>,
    /// Reference received power (dBW) overriding the design's own, as an input.
    pub received_power_dbw: Option<f64>,
    /// Maximum received power (dBW) overriding the design's own, as an input.
    pub received_power_max_dbw: Option<f64>,
}

fn finite_opt(t: &toml::Table, key: &str, label: &str) -> Result<Option<f64>, KshanaError> {
    match t.get(key) {
        None => Ok(None),
        Some(v) => {
            let x = v
                .as_float()
                .or_else(|| v.as_integer().map(|i| i as f64))
                .ok_or_else(|| err(format!("plan '{label}': {key} must be a number")))?;
            if !x.is_finite() {
                return Err(err(format!("plan '{label}': {key} must be finite")));
            }
            Ok(Some(x))
        }
    }
}

impl Plan {
    /// Read one `[[plan]]` table. Unknown keys are refused so a typing slip cannot be
    /// silently ignored.
    pub fn from_table(t: &toml::Table, index: usize) -> Result<Plan, KshanaError> {
        let label = t
            .get("label")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                err(format!(
                    "plan {}: `label` (a non-empty text) is required",
                    index + 1
                ))
            })?
            .to_string();
        if let Some(k) = t.keys().find(|k| !PLAN_KEYS.contains(&k.as_str())) {
            return Err(err(format!(
                "plan '{label}': unknown key `{k}` (a plan takes {})",
                PLAN_KEYS.join(", ")
            )));
        }
        let signal = match t.get("signal") {
            Some(toml::Value::String(s)) if !s.trim().is_empty() => {
                SignalSource::Preset(s.trim().to_string())
            }
            Some(toml::Value::Table(tab)) => SignalSource::Inline(tab.clone()),
            Some(_) => {
                return Err(err(format!(
                    "plan '{label}': `signal` is a preset signal name or an inline signal table"
                )))
            }
            None => return Err(err(format!("plan '{label}': `signal` is required"))),
        };
        let mask_deg = finite_opt(t, "mask_deg", &label)?
            .ok_or_else(|| err(format!("plan '{label}': `mask_deg` is required")))?;
        if !(0.0..90.0).contains(&mask_deg) {
            return Err(err(format!(
                "plan '{label}': mask_deg must be at least 0 and below 90 degrees"
            )));
        }
        let arr = |key: &str| -> Result<Vec<toml::Value>, KshanaError> {
            match t.get(key) {
                None => Ok(vec![]),
                Some(toml::Value::Array(a)) => Ok(a.clone()),
                Some(_) => Err(err(format!(
                    "plan '{label}': `{key}` must be an array of tables ([[plan.{key}]])"
                ))),
            }
        };
        let shells = arr("shell")?;
        let satellites = arr("satellite")?;
        if shells.is_empty() && satellites.is_empty() {
            return Err(err(format!(
                "plan '{label}': the constellation needs at least one [[plan.shell]] or [[plan.satellite]]"
            )));
        }
        let j2 = match t.get("j2") {
            None => None,
            Some(toml::Value::Boolean(b)) => Some(*b),
            Some(_) => return Err(err(format!("plan '{label}': j2 must be true or false"))),
        };
        let received_power_dbw = finite_opt(t, "received_power_dbw", &label)?;
        let received_power_max_dbw = finite_opt(t, "received_power_max_dbw", &label)?;
        Ok(Plan {
            label,
            signal,
            mask_deg,
            shells,
            satellites,
            j2,
            received_power_dbw,
            received_power_max_dbw,
        })
    }

    /// The `leo-signal` scenario for this plan: the study's receiver and victims, and
    /// the plan's signal, each passed through unchanged.
    pub fn signal_scenario(&self, receiver: &toml::Table, victims: &[String]) -> String {
        let mut root = toml::Table::new();
        root.insert("kind".into(), toml::Value::String("leo-signal".into()));
        match &self.signal {
            SignalSource::Preset(_) => {
                root.insert(
                    "presets".into(),
                    toml::Value::Array(
                        LEO_SIGNAL_PRESET_FILES
                            .iter()
                            .map(|p| toml::Value::String((*p).into()))
                            .collect(),
                    ),
                );
            }
            SignalSource::Inline(t) => {
                root.insert(
                    "signals".into(),
                    toml::Value::Array(vec![toml::Value::Table(t.clone())]),
                );
            }
        }
        root.insert("receiver".into(), toml::Value::Table(receiver.clone()));
        let mut compat = toml::Table::new();
        compat.insert(
            "victims".into(),
            toml::Value::Array(
                victims
                    .iter()
                    .map(|v| toml::Value::String(v.clone()))
                    .collect(),
            ),
        );
        root.insert("compatibility".into(), toml::Value::Table(compat));
        toml::to_string(&root).expect("a table of plain values serialises")
    }

    /// The name of the signal to pick out of the `leo-signal` result.
    pub fn signal_name(&self) -> Option<String> {
        match &self.signal {
            SignalSource::Preset(n) => Some(n.clone()),
            SignalSource::Inline(t) => t.get("name").and_then(|v| v.as_str()).map(String::from),
        }
    }

    /// The `constellation-design` scenario for this plan's constellation alone.
    pub fn coverage_scenario(&self, grid: &super::Grid) -> String {
        let mut c = toml::Table::new();
        c.insert("name".into(), toml::Value::String(self.label.clone()));
        if !self.shells.is_empty() {
            c.insert("shell".into(), toml::Value::Array(self.shells.clone()));
        }
        if !self.satellites.is_empty() {
            c.insert(
                "satellite".into(),
                toml::Value::Array(self.satellites.clone()),
            );
        }
        coverage_toml(grid, self.mask_deg, self.j2, c)
    }
}

/// The `constellation-design` scenario for one open GNSS preset alone (the reverse
/// direction's numbers in view).
pub fn gnss_coverage_scenario(
    grid: &super::Grid,
    mask_deg: f64,
    system: &str,
    preset: &str,
) -> String {
    let mut c = toml::Table::new();
    c.insert("name".into(), toml::Value::String(system.into()));
    c.insert("preset".into(), toml::Value::String(preset.into()));
    coverage_toml(grid, mask_deg, None, c)
}

fn coverage_toml(grid: &super::Grid, mask_deg: f64, j2: Option<bool>, c: toml::Table) -> String {
    let mut root = toml::Table::new();
    root.insert(
        "kind".into(),
        toml::Value::String("constellation-design".into()),
    );
    root.insert("body".into(), toml::Value::String("earth".into()));
    root.insert("duration_s".into(), toml::Value::Float(grid.duration_s));
    root.insert("step_s".into(), toml::Value::Float(grid.step_s));
    root.insert("mask_deg".into(), toml::Value::Float(mask_deg));
    root.insert(
        "grid_step_deg".into(),
        toml::Value::Float(grid.grid_step_deg),
    );
    if let Some(b) = j2 {
        root.insert("j2".into(), toml::Value::Boolean(b));
    }
    // The ground tracks are not used here; keep the run's document small.
    root.insert("track_points".into(), toml::Value::Integer(2));
    root.insert("max_tracks".into(), toml::Value::Integer(1));
    root.insert(
        "constellation".into(),
        toml::Value::Array(vec![toml::Value::Table(c)]),
    );
    toml::to_string(&root).expect("a table of plain values serialises")
}

// ─── What is read back from the open runs ────────────────────────────────────────

/// One victim row of the `leo-signal` compatibility table, as read.
#[derive(Debug, Clone, PartialEq)]
pub struct VictimReading {
    pub gnss: String,
    /// SSC of the plan's signal into the GNSS signal (dB/Hz); `None` = no overlap.
    pub ssc_into_gnss_db_per_hz: Option<f64>,
    /// SSC of the GNSS signal into the plan's tracked component (dB/Hz).
    pub ssc_from_gnss_db_per_hz: Option<f64>,
    /// The open run's own single-satellite GNSS C/N0 loss (dB), when it states one.
    pub engine_gnss_loss_db: Option<f64>,
    /// The open run's own single-satellite loss of the plan's C/N0 (dB).
    pub engine_plan_loss_db: Option<f64>,
    /// The GNSS minimum received power the open run states (dBW).
    pub gnss_received_power_dbw: f64,
}

/// What this pack reads from the plan's `leo-signal` run.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalReading {
    pub engine_version: Option<String>,
    pub name: String,
    pub source: Option<String>,
    pub centre_hz: f64,
    pub tx_bandwidth_hz: f64,
    pub ranging_accuracy_m: Option<f64>,
    pub received_power_dbw: Option<f64>,
    pub received_power_max_dbw: Option<f64>,
    pub reference_cn0_dbhz: f64,
    pub reference_cn0_source: Option<String>,
    pub noise_density_dbw_per_hz: f64,
    pub victims: Vec<VictimReading>,
}

fn num(v: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64().filter(|x| x.is_finite())
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(String::from)
}

impl SignalReading {
    /// Read the named signal out of a `leo-signal` result document.
    pub fn read(
        result: &Value,
        name: Option<&str>,
        label: &str,
    ) -> Result<SignalReading, KshanaError> {
        let signals = result
            .get("signals")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                err(format!(
                    "plan '{label}': the leo-signal run returned no signals"
                ))
            })?;
        let s = match name {
            Some(n) => signals
                .iter()
                .find(|s| s.get("name").and_then(Value::as_str) == Some(n))
                .ok_or_else(|| {
                    let known: Vec<&str> = signals
                        .iter()
                        .filter_map(|s| s.get("name").and_then(Value::as_str))
                        .collect();
                    err(format!(
                        "plan '{label}': the open engine has no signal named {n:?} (its presets hold {})",
                        known.join(", ")
                    ))
                })?,
            None => signals
                .first()
                .ok_or_else(|| err(format!("plan '{label}': the leo-signal run returned no signals")))?,
        };
        let need = |v: Option<f64>, what: &str| {
            v.ok_or_else(|| {
                err(format!(
                    "plan '{label}': the leo-signal run states no {what}"
                ))
            })
        };
        let noise = need(
            num(result, &["receiver", "noise_density_dbw_per_hz"]),
            "receiver noise density",
        )?;
        let mut victims = Vec::new();
        for c in s
            .get("compatibility")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            victims.push(VictimReading {
                gnss: text(c, "gnss").unwrap_or_default(),
                ssc_into_gnss_db_per_hz: num(c, &["ssc_leo_into_gnss_db_per_hz"]),
                ssc_from_gnss_db_per_hz: num(c, &["ssc_gnss_into_leo_db_per_hz"]),
                engine_gnss_loss_db: num(c, &["gnss_cn0_degradation_db"]),
                engine_plan_loss_db: num(c, &["leo_cn0_degradation_db"]),
                gnss_received_power_dbw: need(
                    num(c, &["gnss_received_power_dbw"]),
                    "GNSS received power",
                )?,
            });
        }
        Ok(SignalReading {
            engine_version: text(result, "engine_version"),
            name: text(s, "name").unwrap_or_default(),
            source: text(s, "source"),
            centre_hz: need(num(s, &["centre_hz"]), "centre frequency")?,
            tx_bandwidth_hz: need(num(s, &["tx_bandwidth_hz"]), "transmit bandwidth")?,
            ranging_accuracy_m: num(s, &["tracking", "ranging_accuracy_m"]),
            received_power_dbw: num(s, &["received_power_dbw"]),
            received_power_max_dbw: num(s, &["received_power_max_dbw"]),
            reference_cn0_dbhz: need(num(s, &["reference_cn0_dbhz"]), "reference C/N0")?,
            reference_cn0_source: text(s, "reference_cn0_source"),
            noise_density_dbw_per_hz: noise,
            victims,
        })
    }
}

/// What this pack reads from a `constellation-design` run of one constellation.
#[derive(Debug, Clone, PartialEq)]
pub struct CoverageReading {
    pub lat_deg: Vec<f64>,
    pub lon_deg: Vec<f64>,
    /// Most satellites above the mask at any epoch, per cell `[lat][lon]`.
    pub max_visible: Vec<Vec<u64>>,
    /// Mean satellites above the mask over the epochs, per cell `[lat][lon]`.
    pub mean_visible: Vec<Vec<f64>>,
    pub total_satellites: u64,
}

impl CoverageReading {
    pub fn read(result: &Value, what: &str) -> Result<CoverageReading, KshanaError> {
        let g = result.get("grid").ok_or_else(|| {
            err(format!(
                "{what}: the constellation-design run returned no grid"
            ))
        })?;
        let floats = |k: &str| -> Result<Vec<f64>, KshanaError> {
            g.get(k)
                .and_then(Value::as_array)
                .ok_or_else(|| err(format!("{what}: the grid has no {k}")))?
                .iter()
                .map(|x| {
                    x.as_f64()
                        .ok_or_else(|| err(format!("{what}: a non-number in grid.{k}")))
                })
                .collect()
        };
        let lat_deg = floats("lat_deg")?;
        let lon_deg = floats("lon_deg")?;
        let rows = |k: &str| -> Result<Vec<Vec<Value>>, KshanaError> {
            let a = g
                .get(k)
                .and_then(Value::as_array)
                .ok_or_else(|| err(format!("{what}: the grid has no {k}")))?;
            if a.len() != lat_deg.len() {
                return Err(err(format!(
                    "{what}: grid.{k} has the wrong number of rows"
                )));
            }
            a.iter()
                .map(|r| {
                    let r = r
                        .as_array()
                        .ok_or_else(|| err(format!("{what}: grid.{k} row is not an array")))?;
                    if r.len() != lon_deg.len() {
                        return Err(err(format!("{what}: grid.{k} row has the wrong length")));
                    }
                    Ok(r.clone())
                })
                .collect()
        };
        let max_visible = rows("max_visible")?
            .into_iter()
            .map(|r| {
                r.iter()
                    .map(|x| {
                        x.as_u64()
                            .ok_or_else(|| err(format!("{what}: grid.max_visible is not a count")))
                    })
                    .collect::<Result<Vec<u64>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mean_visible = rows("mean_visible")?
            .into_iter()
            .map(|r| {
                r.iter()
                    .map(|x| {
                        x.as_f64()
                            .filter(|v| v.is_finite() && *v >= 0.0)
                            .ok_or_else(|| err(format!("{what}: grid.mean_visible is not a count")))
                    })
                    .collect::<Result<Vec<f64>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let total_satellites = result
            .get("total_satellites")
            .and_then(Value::as_u64)
            .ok_or_else(|| err(format!("{what}: the run states no total_satellites")))?;
        Ok(CoverageReading {
            lat_deg,
            lon_deg,
            max_visible,
            mean_visible,
            total_satellites,
        })
    }
}
