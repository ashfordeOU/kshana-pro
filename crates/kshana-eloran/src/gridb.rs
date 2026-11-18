// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! The preloaded GRI / chain database and the UTC(k) laboratory list.
//!
//! Two JSON files ship inside the crate (`data/gri_db.json`, `data/utc_k.json`, embedded with
//! `include_str!`); `data/README.md` documents their sources and how to update them.
//!
//! * [`GriDb`]: LORAN-C, eLoran and Chayka chains worldwide, current and historical. Every
//!   chain and station carries its `sources` and a `confidence` ([`Confidence`]). Values that
//!   no source gave are `null`; such a chain is **incomplete** and [`DbChain::to_chain`]
//!   refuses it unless the caller supplies the missing values ([`Overrides`]).
//! * [`GriDb::merge`]: apply a user file of updates and additions (e.g. future deployments)
//!   on top of the embedded database.
//! * [`scenario_snippet`]: a TOML `[[chains]]` block for a database chain, to paste into a
//!   scenario.
//! * [`UtcKList`]: the laboratories publishing a UTC(k) in BIPM Circular T, and
//!   [`default_time_ref`] / [`illustrative_time_ref`] to get a [`TimeRefModel`] for one.

use crate::chain::{Chain, Station, StationStatus};
use crate::geo::Geo;
use crate::signal::{PulseSignal, Role};
use crate::timebase::TimeRefModel;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The embedded chain database (JSON).
pub const GRI_DB_JSON: &str = include_str!("../data/gri_db.json");
/// The embedded UTC(k) list (JSON).
pub const UTC_K_JSON: &str = include_str!("../data/utc_k.json");

/// Format tag of the chain database (and of user merge files).
pub const GRI_DB_FORMAT: &str = "kshana-eloran-gri-db";
/// Format tag of the UTC(k) list.
pub const UTC_K_FORMAT: &str = "kshana-eloran-utc-k";
/// Highest database format version this code reads.
pub const GRI_DB_VERSION: u32 = 1;

/// How well a value is supported by its sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Confidence {
    /// Two independent sources agree, or one authoritative primary source.
    #[serde(rename = "verified")]
    Verified,
    /// One secondary source.
    #[serde(rename = "single-source")]
    SingleSource,
    /// Doubtful, conflicting or inferred.
    #[serde(rename = "unverified")]
    Unverified,
}

/// Operational status of a chain (or of a station, when it differs from its chain's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DbStatus {
    Operational,
    Testbed,
    Planned,
    Terminated,
    NeverBuilt,
    Unknown,
}

/// A station in the database. Field names follow [`Station`]; values a source did not give
/// are `None`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DbStation {
    /// Designator, e.g. "M", "W" … "Z" (unique in the chain).
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub role: Role,
    #[serde(default)]
    pub position: Option<Geo>,
    /// Nominal radiated peak power (kW).
    #[serde(default)]
    pub power_kw: Option<f64>,
    /// Emission delay after the master (µs); 0 for the master.
    #[serde(default)]
    pub emission_delay_us: Option<f64>,
    /// Secondary coding delay (µs), informative only.
    #[serde(default)]
    pub coding_delay_us: Option<f64>,
    /// Time reference, when it differs from the chain's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DbStatus>,
    #[serde(default)]
    pub sources: Vec<String>,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// A chain in the database.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DbChain {
    /// Unique key, e.g. "9960-northeast-us". Several entries may share a GRI (different eras).
    pub id: String,
    /// Group repetition interval (µs).
    pub gri_us: u32,
    pub name: String,
    #[serde(default)]
    pub region: String,
    /// ISO 3166-1 alpha-2 codes of the countries hosting stations.
    #[serde(default)]
    pub countries: Vec<String>,
    /// "loran-c", "eloran", "chayka", "loran-c-timing", …
    #[serde(default)]
    pub system: String,
    pub status: DbStatus,
    /// Start of operation (ISO date, possibly partial: "1995", "1997-09").
    #[serde(default)]
    pub operational_from: Option<String>,
    /// End of operation (ISO date, possibly partial).
    #[serde(default)]
    pub operational_to: Option<String>,
    /// Time reference the chain is steered to, e.g. "UTC(KRIS)".
    #[serde(default)]
    pub time_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub stations: Vec<DbStation>,
}

/// The chain database.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GriDb {
    pub format: String,
    pub version: u32,
    pub updated: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub chains: Vec<DbChain>,
}

/// Values supplied by the user for one station when converting a chain.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StationOverride {
    #[serde(default)]
    pub position: Option<Geo>,
    #[serde(default)]
    pub power_kw: Option<f64>,
    #[serde(default)]
    pub emission_delay_us: Option<f64>,
    #[serde(default)]
    pub time_ref: Option<String>,
}

/// Values supplied by the user when converting a chain ([`DbChain::to_chain`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Overrides {
    /// Per station id. A supplied value replaces the database value.
    #[serde(default)]
    pub stations: BTreeMap<String, StationOverride>,
    /// Time reference for every station without its own (replaces the chain's).
    #[serde(default)]
    pub time_ref: Option<String>,
    /// Drop stations that lack required values instead of failing (the chain must keep its
    /// master and at least one secondary).
    #[serde(default)]
    pub skip_incomplete_stations: bool,
}

/// Partial date → comparable "YYYY-MM-DD" bound.
fn date_bound(d: &str, end: bool) -> String {
    let parts: Vec<&str> = d.split('-').collect();
    match parts.len() {
        1 => format!("{}-{}", parts[0], if end { "12-31" } else { "01-01" }),
        2 => format!(
            "{}-{}-{}",
            parts[0],
            parts[1],
            if end { "31" } else { "01" }
        ),
        _ => d.to_string(),
    }
}

fn valid_partial_date(d: &str) -> bool {
    let parts: Vec<&str> = d.split('-').collect();
    let lens = [4, 2, 2];
    !parts.is_empty()
        && parts.len() <= 3
        && parts
            .iter()
            .zip(lens)
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_digit()))
}

/// GRI range and step check (40 000 to 99 990 µs, steps of 10 µs).
pub fn valid_gri(gri_us: u32) -> bool {
    (40_000..=99_990).contains(&gri_us) && gri_us.is_multiple_of(10)
}

impl DbStation {
    /// Names of the values [`DbChain::to_chain`] needs and this station lacks.
    fn missing(&self, ov: Option<&StationOverride>) -> Vec<&'static str> {
        let mut m = Vec::new();
        if self.position.is_none() && ov.and_then(|o| o.position).is_none() {
            m.push("position");
        }
        if self.power_kw.is_none() && ov.and_then(|o| o.power_kw).is_none() {
            m.push("power_kw");
        }
        if self.role == Role::Secondary
            && self.emission_delay_us.is_none()
            && ov.and_then(|o| o.emission_delay_us).is_none()
        {
            m.push("emission_delay_us");
        }
        m
    }
}

impl DbChain {
    /// Missing values, as "station id: field" strings (empty when complete).
    pub fn missing(&self) -> Vec<String> {
        self.missing_with(&Overrides::default())
    }

    fn missing_with(&self, ov: &Overrides) -> Vec<String> {
        self.stations
            .iter()
            .flat_map(|s| {
                s.missing(ov.stations.get(&s.id))
                    .into_iter()
                    .map(move |f| format!("{}: {f}", s.id))
            })
            .collect()
    }

    /// Whether every station has position, power and (secondaries) emission delay.
    pub fn is_complete(&self) -> bool {
        self.missing().is_empty()
    }

    /// Whether the chain was operating on `date` ("YYYY[-MM[-DD]]"). Unknown bounds are open.
    pub fn active_on(&self, date: &str) -> bool {
        let d = date_bound(date, false);
        let after_start = self
            .operational_from
            .as_deref()
            .is_none_or(|f| date_bound(f, false) <= d);
        let before_end = self
            .operational_to
            .as_deref()
            .is_none_or(|t| d <= date_bound(t, true));
        after_start && before_end
    }

    /// Convert to a simulator [`Chain`] (LORAN-C signal), refusing an incomplete chain.
    pub fn to_chain_strict(&self) -> Result<Chain, String> {
        self.to_chain(&Overrides::default())
    }

    /// Convert to a simulator [`Chain`] (LORAN-C signal). Values missing from the database
    /// must come from `ov`; otherwise this fails listing what is missing. The result is
    /// checked with [`Chain::validate`].
    pub fn to_chain(&self, ov: &Overrides) -> Result<Chain, String> {
        for k in ov.stations.keys() {
            if !self.stations.iter().any(|s| &s.id == k) {
                return Err(format!(
                    "chain {}: override for unknown station {k:?}",
                    self.id
                ));
            }
        }
        let missing = self.missing_with(ov);
        let mut stations = Vec::new();
        for s in &self.stations {
            let o = ov.stations.get(&s.id);
            if !s.missing(o).is_empty() {
                if ov.skip_incomplete_stations && s.role == Role::Secondary {
                    continue;
                }
                return Err(format!(
                    "chain {} ({}) is incomplete: missing {}. Supply the values with gridb::Overrides \
                     (or skip_incomplete_stations for secondaries); see the chain's sources and notes.",
                    self.id,
                    self.name,
                    missing.join(", ")
                ));
            }
            let time_ref = o
                .and_then(|o| o.time_ref.clone())
                .or_else(|| s.time_ref.clone())
                .or_else(|| ov.time_ref.clone())
                .or_else(|| self.time_ref.clone())
                .unwrap_or_default();
            let station_off = matches!(s.status, Some(DbStatus::Terminated | DbStatus::NeverBuilt));
            stations.push(Station {
                id: s.id.clone(),
                name: s.name.clone(),
                role: s.role,
                position: o.and_then(|o| o.position).or(s.position).expect("checked"),
                power_kw: o.and_then(|o| o.power_kw).or(s.power_kw).expect("checked"),
                emission_delay_us: if s.role == Role::Master {
                    0.0
                } else {
                    o.and_then(|o| o.emission_delay_us)
                        .or(s.emission_delay_us)
                        .expect("checked")
                },
                time_ref,
                tx_clock: None,
                ecd_us: 0.0,
                status: if station_off {
                    StationStatus::Off
                } else {
                    StationStatus::On
                },
                faults: vec![],
            });
        }
        if !stations.iter().any(|s| s.role == Role::Master) || stations.len() < 2 {
            return Err(format!(
                "chain {}: after skipping incomplete stations no master + secondary remain (missing {})",
                self.id,
                missing.join(", ")
            ));
        }
        let chain = Chain {
            name: self.name.clone(),
            gri_us: self.gri_us,
            signal: PulseSignal::loran_c(),
            stations,
        };
        chain.validate()?;
        Ok(chain)
    }

    /// Structural checks of one entry (not completeness).
    pub fn validate(&self) -> Result<(), String> {
        let id = &self.id;
        if id.is_empty() {
            return Err("chain with empty id".into());
        }
        if !valid_gri(self.gri_us) {
            return Err(format!(
                "chain {id}: GRI {} µs must be 40 000 to 99 990 µs in steps of 10 µs",
                self.gri_us
            ));
        }
        for d in [&self.operational_from, &self.operational_to]
            .into_iter()
            .flatten()
        {
            if !valid_partial_date(d) {
                return Err(format!("chain {id}: date {d:?} is not YYYY[-MM[-DD]]"));
            }
        }
        if let (Some(f), Some(t)) = (&self.operational_from, &self.operational_to) {
            if date_bound(f, false) > date_bound(t, true) {
                return Err(format!(
                    "chain {id}: operational_from {f} is after operational_to {t}"
                ));
            }
        }
        if self.stations.is_empty() {
            return Err(format!("chain {id}: no stations"));
        }
        if self
            .stations
            .iter()
            .filter(|s| s.role == Role::Master)
            .count()
            != 1
        {
            return Err(format!("chain {id}: needs exactly one master"));
        }
        let mut seen = BTreeSet::new();
        for s in &self.stations {
            if !seen.insert(s.id.as_str()) {
                return Err(format!("chain {id}: duplicate station id {:?}", s.id));
            }
            if let Some(p) = s.position {
                if !(-90.0..=90.0).contains(&p.lat_deg) || !(-180.0..=180.0).contains(&p.lon_deg) {
                    return Err(format!(
                        "chain {id} station {}: position out of range",
                        s.id
                    ));
                }
            }
            if let Some(p) = s.power_kw {
                if p.is_nan() || p <= 0.0 {
                    return Err(format!(
                        "chain {id} station {}: power must be positive",
                        s.id
                    ));
                }
            }
            match (s.role, s.emission_delay_us) {
                (Role::Master, Some(d)) if d != 0.0 => {
                    return Err(format!("chain {id}: master emission delay must be 0"))
                }
                (Role::Secondary, Some(d)) if d.is_nan() || d <= 0.0 || d >= self.gri_us as f64 => {
                    return Err(format!(
                        "chain {id} station {}: emission delay {d} outside (0, GRI)",
                        s.id
                    ))
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl GriDb {
    /// The embedded database (parsed and validated).
    pub fn embedded() -> Result<GriDb, String> {
        GriDb::from_json(GRI_DB_JSON)
    }

    /// Parse and validate a database.
    pub fn from_json(src: &str) -> Result<GriDb, String> {
        let db: GriDb = serde_json::from_str(src).map_err(|e| format!("GRI database: {e}"))?;
        db.validate()?;
        Ok(db)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("serialisable")
    }

    /// Validate the whole database: header, every entry, unique ids, GRIs unique per active
    /// era (two entries with the same GRI must have non-overlapping, known operating periods),
    /// and every complete chain converts and passes [`Chain::validate`].
    pub fn validate(&self) -> Result<(), String> {
        if self.format != GRI_DB_FORMAT {
            return Err(format!(
                "GRI database: format {:?}, expected {GRI_DB_FORMAT:?}",
                self.format
            ));
        }
        if self.version == 0 || self.version > GRI_DB_VERSION {
            return Err(format!(
                "GRI database: unsupported version {}",
                self.version
            ));
        }
        let mut ids = BTreeSet::new();
        for c in &self.chains {
            c.validate()?;
            if !ids.insert(c.id.as_str()) {
                return Err(format!("GRI database: duplicate chain id {:?}", c.id));
            }
            if c.is_complete() {
                c.to_chain_strict()
                    .map_err(|e| format!("GRI database: complete chain {}: {e}", c.id))?;
            }
        }
        for (i, a) in self.chains.iter().enumerate() {
            for b in &self.chains[i + 1..] {
                if a.gri_us == b.gri_us && eras_may_overlap(a, b) {
                    return Err(format!(
                        "GRI database: chains {} and {} share GRI {} with overlapping (or open) operating periods",
                        a.id, b.id, a.gri_us
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&DbChain> {
        self.chains.iter().find(|c| c.id == id)
    }

    /// All entries with GRI `gri`, given either in µs (99600) or as the 4-digit designator
    /// (9960).
    pub fn by_gri(&self, gri: u32) -> Vec<&DbChain> {
        let us = if gri < 10_000 { gri * 10 } else { gri };
        self.chains.iter().filter(|c| c.gri_us == us).collect()
    }

    /// Entries whose region contains `region` (case-insensitive).
    pub fn by_region(&self, region: &str) -> Vec<&DbChain> {
        let q = region.to_lowercase();
        self.chains
            .iter()
            .filter(|c| c.region.to_lowercase().contains(&q))
            .collect()
    }

    /// Entries operating on `date` (unknown bounds count as open).
    pub fn active_on(&self, date: &str) -> Vec<&DbChain> {
        self.chains.iter().filter(|c| c.active_on(date)).collect()
    }

    /// Free-text search: a number matches the GRI (µs or designator); text matches (case-
    /// insensitive substring) id, name, region, country codes, system or station names.
    pub fn search(&self, query: &str) -> Vec<&DbChain> {
        let q = query.trim().to_lowercase();
        if let Ok(n) = q.parse::<u32>() {
            return self.by_gri(n);
        }
        self.chains
            .iter()
            .filter(|c| {
                c.id.to_lowercase().contains(&q)
                    || c.name.to_lowercase().contains(&q)
                    || c.region.to_lowercase().contains(&q)
                    || c.system.to_lowercase() == q
                    || c.countries.iter().any(|k| k.to_lowercase() == q)
                    || c.stations
                        .iter()
                        .any(|s| s.name.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Apply a user file (same format; see [`UserDb`]): entries with a known id are updated
    /// field by field (stations merged by id), unknown ids are added (and must then be full
    /// entries). The merged database is validated; on error `self` is unchanged.
    pub fn merge(&mut self, user: &UserDb) -> Result<MergeReport, String> {
        if user.format != GRI_DB_FORMAT {
            return Err(format!(
                "user GRI database: format {:?}, expected {GRI_DB_FORMAT:?}",
                user.format
            ));
        }
        if user.version == 0 || user.version > GRI_DB_VERSION {
            return Err(format!(
                "user GRI database: unsupported version {}",
                user.version
            ));
        }
        let mut next = self.clone();
        let mut report = MergeReport::default();
        for p in &user.chains {
            if let Some(c) = next.chains.iter_mut().find(|c| c.id == p.id) {
                p.apply(c)?;
                report.updated.push(p.id.clone());
            } else {
                next.chains.push(p.to_new()?);
                report.added.push(p.id.clone());
            }
        }
        if !user.updated.is_empty() {
            next.updated = user.updated.clone();
        }
        next.validate()?;
        *self = next;
        Ok(report)
    }

    /// Parse a user file and merge it ([`GriDb::merge`]).
    pub fn merge_json(&mut self, src: &str) -> Result<MergeReport, String> {
        let user: UserDb =
            serde_json::from_str(src).map_err(|e| format!("user GRI database: {e}"))?;
        self.merge(&user)
    }
}

/// Whether two entries' operating periods may overlap (open/unknown bounds may).
fn eras_may_overlap(a: &DbChain, b: &DbChain) -> bool {
    let a_ends_before_b = matches!((&a.operational_to, &b.operational_from),
        (Some(t), Some(f)) if date_bound(t, true) < date_bound(f, false));
    let b_ends_before_a = matches!((&b.operational_to, &a.operational_from),
        (Some(t), Some(f)) if date_bound(t, true) < date_bound(f, false));
    !(a_ends_before_b || b_ends_before_a)
}

/// Result of [`GriDb::merge`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MergeReport {
    pub added: Vec<String>,
    pub updated: Vec<String>,
}

/// A user database file: same header as [`GriDb`], entries as partial patches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserDb {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub updated: String,
    pub chains: Vec<ChainPatch>,
}

/// A partial chain entry: `id` plus any fields to set.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChainPatch {
    pub id: String,
    #[serde(default)]
    pub gri_us: Option<u32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub countries: Option<Vec<String>>,
    #[serde(default)]
    pub system: Option<String>,
    #[serde(default)]
    pub status: Option<DbStatus>,
    #[serde(default)]
    pub operational_from: Option<String>,
    #[serde(default)]
    pub operational_to: Option<String>,
    #[serde(default)]
    pub time_ref: Option<String>,
    #[serde(default)]
    pub operator: Option<String>,
    /// Appended to the entry's sources.
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub confidence: Option<Confidence>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Stations merged by id.
    #[serde(default)]
    pub stations: Vec<StationPatch>,
    /// Station ids to remove.
    #[serde(default)]
    pub remove_stations: Vec<String>,
}

/// A partial station entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StationPatch {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub role: Option<Role>,
    #[serde(default)]
    pub position: Option<Geo>,
    #[serde(default)]
    pub power_kw: Option<f64>,
    #[serde(default)]
    pub emission_delay_us: Option<f64>,
    #[serde(default)]
    pub coding_delay_us: Option<f64>,
    #[serde(default)]
    pub time_ref: Option<String>,
    #[serde(default)]
    pub status: Option<DbStatus>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub confidence: Option<Confidence>,
    #[serde(default)]
    pub notes: Option<String>,
}

macro_rules! set_if {
    ($dst:expr, $src:expr) => {
        if let Some(v) = $src.clone() {
            $dst = v;
        }
    };
}
macro_rules! set_opt_if {
    ($dst:expr, $src:expr) => {
        if $src.is_some() {
            $dst = $src.clone();
        }
    };
}

impl StationPatch {
    fn apply(&self, s: &mut DbStation) {
        set_if!(s.name, self.name);
        set_if!(s.role, self.role);
        set_opt_if!(s.position, self.position);
        set_opt_if!(s.power_kw, self.power_kw);
        set_opt_if!(s.emission_delay_us, self.emission_delay_us);
        set_opt_if!(s.coding_delay_us, self.coding_delay_us);
        set_opt_if!(s.time_ref, self.time_ref);
        set_opt_if!(s.status, self.status);
        set_if!(s.confidence, self.confidence);
        set_opt_if!(s.notes, self.notes);
        s.sources.extend(self.sources.iter().cloned());
    }

    fn to_new(&self, chain_id: &str) -> Result<DbStation, String> {
        let role = self.role.ok_or_else(|| {
            format!(
                "user GRI database: new station {chain_id}/{} needs a role",
                self.id
            )
        })?;
        let mut s = DbStation {
            id: self.id.clone(),
            name: String::new(),
            role,
            position: None,
            power_kw: None,
            emission_delay_us: if role == Role::Master {
                Some(0.0)
            } else {
                None
            },
            coding_delay_us: None,
            time_ref: None,
            status: None,
            sources: vec![],
            confidence: Confidence::Unverified,
            notes: None,
        };
        self.apply(&mut s);
        Ok(s)
    }
}

impl ChainPatch {
    fn apply(&self, c: &mut DbChain) -> Result<(), String> {
        set_if!(c.gri_us, self.gri_us);
        set_if!(c.name, self.name);
        set_if!(c.region, self.region);
        set_if!(c.countries, self.countries);
        set_if!(c.system, self.system);
        set_if!(c.status, self.status);
        set_opt_if!(c.operational_from, self.operational_from);
        set_opt_if!(c.operational_to, self.operational_to);
        set_opt_if!(c.time_ref, self.time_ref);
        set_opt_if!(c.operator, self.operator);
        set_if!(c.confidence, self.confidence);
        set_opt_if!(c.notes, self.notes);
        c.sources.extend(self.sources.iter().cloned());
        c.stations.retain(|s| !self.remove_stations.contains(&s.id));
        for sp in &self.stations {
            if let Some(s) = c.stations.iter_mut().find(|s| s.id == sp.id) {
                sp.apply(s);
            } else {
                c.stations.push(sp.to_new(&c.id)?);
            }
        }
        Ok(())
    }

    fn to_new(&self) -> Result<DbChain, String> {
        let need = |what: &str| format!("user GRI database: new chain {:?} needs {what}", self.id);
        let mut c = DbChain {
            id: self.id.clone(),
            gri_us: self.gri_us.ok_or_else(|| need("gri_us"))?,
            name: self.name.clone().ok_or_else(|| need("name"))?,
            region: String::new(),
            countries: vec![],
            system: "loran-c".into(),
            status: self.status.ok_or_else(|| need("status"))?,
            operational_from: None,
            operational_to: None,
            time_ref: None,
            operator: None,
            sources: vec![],
            confidence: Confidence::Unverified,
            notes: None,
            stations: vec![],
        };
        if self.stations.is_empty() {
            return Err(need("stations"));
        }
        self.apply(&mut c)?;
        Ok(c)
    }
}

fn toml_str(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// A TOML `[[chains]]` block for the complete database chain with GRI `gri` (µs or 4-digit
/// designator). Fails when no complete entry has that GRI, or when several do (then use
/// [`scenario_snippet_for`] with the entry id).
pub fn scenario_snippet(gri: u32) -> Result<String, String> {
    let db = GriDb::embedded()?;
    let all = db.by_gri(gri);
    let complete: Vec<_> = all.iter().filter(|c| c.is_complete()).collect();
    match complete.as_slice() {
        [c] => scenario_snippet_for(c, &Overrides::default()),
        [] if all.is_empty() => Err(format!("no chain with GRI {gri} in the database")),
        [] => Err(format!(
            "no complete chain with GRI {gri}: {}",
            all.iter()
                .map(|c| format!("{} (missing {})", c.id, c.missing().join(", ")))
                .collect::<Vec<_>>()
                .join("; ")
        )),
        many => Err(format!(
            "several complete chains with GRI {gri}: {}; use scenario_snippet_for",
            many.iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// A TOML `[[chains]]` block for `chain` converted with `ov`. Comments carry the entry's
/// id, status, sources and confidence. The time references the stations use must be defined
/// in the scenario's `[[time_refs]]` (see [`time_refs_snippet`]).
pub fn scenario_snippet_for(chain: &DbChain, ov: &Overrides) -> Result<String, String> {
    let c = chain.to_chain(ov)?;
    let mut out = String::new();
    out.push_str(&format!(
        "# {} — kshana-eloran GRI database entry {:?} (status {:?}, confidence {:?})\n",
        chain.name, chain.id, chain.status, chain.confidence
    ));
    for s in &chain.sources {
        out.push_str(&format!("# source: {}\n", s.replace('\n', " ")));
    }
    out.push_str("[[chains]]\n");
    out.push_str(&format!("name = {}\n", toml_str(&c.name)));
    out.push_str(&format!("gri_us = {}\n", c.gri_us));
    for (s, db) in c.stations.iter().zip(
        chain
            .stations
            .iter()
            .filter(|d| c.stations.iter().any(|s| s.id == d.id)),
    ) {
        out.push_str(&format!(
            "[[chains.stations]]\n# confidence {:?}\n",
            db.confidence
        ));
        out.push_str(&format!("id = {}\n", toml_str(&s.id)));
        out.push_str(&format!("name = {}\n", toml_str(&s.name)));
        out.push_str(&format!(
            "role = \"{}\"\n",
            if s.role == Role::Master {
                "master"
            } else {
                "secondary"
            }
        ));
        out.push_str(&format!(
            "position = {{ lat_deg = {:?}, lon_deg = {:?} }}\n",
            s.position.lat_deg, s.position.lon_deg
        ));
        out.push_str(&format!("power_kw = {:?}\n", s.power_kw));
        out.push_str(&format!("emission_delay_us = {:?}\n", s.emission_delay_us));
        if !s.time_ref.is_empty() {
            out.push_str(&format!("time_ref = {}\n", toml_str(&s.time_ref)));
        }
        if s.status == StationStatus::Off {
            out.push_str("status = \"off\"\n");
        }
    }
    Ok(out)
}

/// `[[time_refs]]` blocks (ideal models, [`default_time_ref`]) for every time reference the
/// converted chain uses.
pub fn time_refs_snippet(chain: &Chain) -> Result<String, String> {
    let refs: BTreeSet<&str> = chain
        .stations
        .iter()
        .map(|s| s.time_ref.as_str())
        .filter(|r| !r.is_empty())
        .collect();
    let mut out = String::new();
    for r in refs {
        let m = default_time_ref(r)?;
        out.push_str(&format!(
            "[[time_refs]]\nid = {}\noffset_ns = {:?}\n",
            toml_str(&m.id),
            m.offset_ns
        ));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// UTC(k) laboratories

/// A laboratory publishing a UTC(k) in BIPM Circular T.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UtcLab {
    /// Circular T laboratory code, e.g. "NPL".
    pub code: String,
    /// "UTC(NPL)".
    pub utc_k: String,
    pub city: String,
    /// ISO 3166-1 alpha-2.
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub institution: Option<String>,
    /// "general-knowledge" or "unverified" (Circular T gives code and city only).
    #[serde(default)]
    pub institution_confidence: String,
    /// UTC − UTC(k) (ns) at the last date of Circular T 466 (illustrative snapshot).
    #[serde(default)]
    pub ct466_last_utc_minus_utck_ns: Option<f64>,
    /// Largest |UTC − UTC(k)| (ns) over Circular T 466 (illustrative snapshot).
    #[serde(default)]
    pub ct466_max_abs_ns: Option<f64>,
    pub source: String,
    pub confidence: Confidence,
}

/// The UTC(k) list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UtcKList {
    pub format: String,
    pub version: u32,
    pub updated: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    pub labs: Vec<UtcLab>,
}

/// "UTC(NPL)", "utc(npl)" or "NPL" → "NPL".
fn lab_code(id: &str) -> String {
    let t = id.trim();
    let u = t.to_uppercase();
    let inner = u
        .strip_prefix("UTC(")
        .and_then(|r| r.strip_suffix(')'))
        .unwrap_or(&u);
    inner.trim().to_string()
}

impl UtcKList {
    pub fn embedded() -> Result<UtcKList, String> {
        let l: UtcKList =
            serde_json::from_str(UTC_K_JSON).map_err(|e| format!("UTC(k) list: {e}"))?;
        if l.format != UTC_K_FORMAT {
            return Err(format!("UTC(k) list: format {:?}", l.format));
        }
        Ok(l)
    }

    /// Look up by code or UTC(k) id, case-insensitive.
    pub fn get(&self, id: &str) -> Option<&UtcLab> {
        let code = lab_code(id);
        self.labs.iter().find(|l| l.code == code)
    }
}

/// The default model of a time reference: ideal (UTC(k) = UTC), with the canonical id
/// "UTC(<code>)". Fails for a laboratory not in the UTC(k) list. Use
/// [`TimeRefModel::ideal`] directly for references outside it (e.g. "tx:Anthorn").
pub fn default_time_ref(id: &str) -> Result<TimeRefModel, String> {
    let list = UtcKList::embedded()?;
    let lab = list.get(id).ok_or_else(|| {
        format!("{id:?} is not a laboratory of the UTC(k) list (BIPM Circular T)")
    })?;
    Ok(TimeRefModel::ideal(&lab.utc_k))
}

/// An ILLUSTRATIVE model: offset set from the laboratory's last Circular T 466 value
/// (`x = UTC(k) − UTC = −(UTC − UTC(k))`), no drift or noise. A snapshot for realistic
/// magnitudes in demonstrations, not a model of the laboratory. Labs without a value give
/// the ideal model.
pub fn illustrative_time_ref(id: &str) -> Result<TimeRefModel, String> {
    let list = UtcKList::embedded()?;
    let lab = list.get(id).ok_or_else(|| {
        format!("{id:?} is not a laboratory of the UTC(k) list (BIPM Circular T)")
    })?;
    let mut m = TimeRefModel::ideal(&lab.utc_k);
    if let Some(v) = lab.ct466_last_utc_minus_utck_ns {
        m.offset_ns = -v;
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> GriDb {
        GriDb::embedded().expect("embedded GRI database parses and validates")
    }

    #[test]
    fn embedded_db_parses_and_validates() {
        let d = db();
        assert_eq!(d.format, GRI_DB_FORMAT);
        assert_eq!(d.version, 1);
        assert!(d.chains.len() >= 30, "{} chains", d.chains.len());
        for c in &d.chains {
            assert!(!c.sources.is_empty(), "{} has no source", c.id);
            for s in &c.stations {
                assert!(!s.sources.is_empty(), "{}/{} has no source", c.id, s.id);
            }
        }
    }

    #[test]
    fn every_complete_chain_converts_and_validates() {
        let d = db();
        let complete: Vec<_> = d.chains.iter().filter(|c| c.is_complete()).collect();
        assert!(
            complete.len() >= 15,
            "only {} complete chains",
            complete.len()
        );
        for c in complete {
            let ch = c
                .to_chain_strict()
                .unwrap_or_else(|e| panic!("{}: {e}", c.id));
            ch.validate().unwrap();
            assert_eq!(ch.gri_us, c.gri_us);
            assert_eq!(ch.stations.len(), c.stations.len());
        }
    }

    #[test]
    fn incomplete_chain_is_refused_with_clear_error_then_accepted_with_overrides() {
        let d = db();
        let c = d.get("6731-lessay").unwrap();
        assert!(!c.is_complete());
        let e = c.to_chain_strict().unwrap_err();
        assert!(e.contains("incomplete") && e.contains("Y: power_kw"), "{e}");
        let mut ov = Overrides::default();
        ov.stations.insert(
            "Y".into(),
            StationOverride {
                power_kw: Some(250.0),
                ..Default::default()
            },
        );
        ov.time_ref = Some("UTC(NPL)".into());
        let ch = c.to_chain(&ov).unwrap();
        assert_eq!(ch.stations[2].emission_delay_us, 27300.0);
        assert_eq!(ch.stations[2].power_kw, 250.0);
        assert_eq!(ch.stations[2].time_ref, "UTC(NPL)");
        // skipping incomplete secondaries
        let k = d.get("9930-korea").unwrap();
        let mut ov = Overrides {
            skip_incomplete_stations: true,
            ..Default::default()
        };
        assert!(k.to_chain(&ov).is_err(), "master power unknown");
        for id in ["M", "W"] {
            ov.stations.insert(
                id.into(),
                StationOverride {
                    power_kw: Some(50.0),
                    ..Default::default()
                },
            );
        }
        let ch = k.to_chain(&ov).unwrap();
        assert_eq!(ch.stations.len(), 2);
        assert_eq!(ch.stations[0].time_ref, "UTC(KRIS)");
        // an override for an unknown station is an error
        let mut bad = Overrides::default();
        bad.stations.insert("Q".into(), StationOverride::default());
        assert!(k.to_chain(&bad).unwrap_err().contains("unknown station"));
    }

    #[test]
    fn gris_are_valid_and_unique_per_active_era() {
        let d = db();
        for c in &d.chains {
            assert!(valid_gri(c.gri_us), "{}", c.id);
        }
        for (i, a) in d.chains.iter().enumerate() {
            for b in &d.chains[i + 1..] {
                if a.gri_us == b.gri_us {
                    assert!(!eras_may_overlap(a, b), "{} / {}", a.id, b.id);
                }
            }
        }
        // the shared GRIs are distinct eras
        assert_eq!(d.by_gri(9930).len(), 2);
        assert_eq!(d.by_gri(6731).len(), 2);
        assert_eq!(
            d.active_on("2010-01-01")
                .iter()
                .filter(|c| c.gri_us == 99300)
                .count(),
            1
        );
        assert_eq!(
            d.active_on("2020-01-01")
                .iter()
                .filter(|c| c.gri_us == 99300)
                .count(),
            1
        );
        assert_eq!(
            d.active_on("2020-01-01")
                .iter()
                .find(|c| c.gri_us == 99300)
                .unwrap()
                .id,
            "9930-korea"
        );
        // validation rejects a GRI clash in the same era and a bad GRI
        let mut bad = d.clone();
        let mut dup = bad.get("9960-northeast-us").unwrap().clone();
        dup.id = "dup".into();
        bad.chains.push(dup);
        assert!(bad.validate().unwrap_err().contains("share GRI"));
        let mut bad = d.clone();
        bad.chains[0].gri_us = 59_305;
        assert!(bad.validate().is_err());
    }

    /// Spot checks transcribed from primary / independent sources.
    #[test]
    fn spot_checks() {
        let d = db();
        // USCG Loran-C User Handbook 1992, Table B-1 (p. B-3): 9960 W Caribou ED 13797.20 µs,
        // coding delay 11000, 800 kW; Seneca 42°42'50.716"N 76°49'33.308"W. PRC68 gives the
        // same Seneca position; a USNO calibration report uses ED 13797.2.
        let c = d.get("9960-northeast-us").unwrap();
        let w = c.stations.iter().find(|s| s.id == "W").unwrap();
        assert_eq!(w.emission_delay_us, Some(13797.20));
        assert_eq!(w.coding_delay_us, Some(11000.0));
        assert_eq!(w.power_kw, Some(800.0));
        let m = c.stations[0].position.unwrap();
        assert!((m.lat_deg - (42.0 + 42.0 / 60.0 + 50.716 / 3600.0)).abs() < 1e-6);
        assert!((m.lon_deg + (76.0 + 49.0 / 60.0 + 33.308 / 3600.0)).abs() < 1e-6);
        assert_eq!(c.operational_to.as_deref(), Some("2010-02-08"));
        // Handbook 1992 (p. B-3): 9940 Y Searchlight ED 41967.30; 9610 Z Grangeville 69304.00.
        let ed = |id: &str, st: &str| {
            d.get(id)
                .unwrap()
                .stations
                .iter()
                .find(|s| s.id == st)
                .unwrap()
                .emission_delay_us
                .unwrap()
        };
        assert_eq!(ed("9940-us-west-coast", "Y"), 41967.30);
        assert_eq!(ed("9610-south-central-us", "Z"), 69304.00);
        // Handbook 1992 (p. B-2): 7980 Y Jupiter 45201.88.
        assert_eq!(ed("7980-southeast-us", "Y"), 45201.88);
        // Anthorn 6731-Y ED 27300 µs: Irish Lights NtM 13/2012 and the SHOM 6731 table agree.
        assert_eq!(ed("6731-lessay", "Y"), 27300.0);
        // SHOM 6731 table: X Soustons 13000, Z Sylt 42100.
        assert_eq!(ed("6731-lessay", "X"), 13000.0);
        assert_eq!(ed("6731-lessay", "Z"), 42100.0);
        // Handbook 1992 (p. B-3, Chayka 8000): Syzran ED 67941.60, coding 65000, 700 kW.
        assert_eq!(ed("8000-western-russia-chayka", "Z"), 67941.60);
        // Pohang (9930 M): Wikipedia 36°11'5.33"N 129°20'27.4"E; Pub 117 (2005) 36 11 05N 129 20 27E.
        let p = d.get("9930-korea").unwrap().stations[0].position.unwrap();
        assert!((p.lat_deg - 36.1848).abs() < 1e-3 && (p.lon_deg - 129.3409).abs() < 1e-3);
    }

    #[test]
    fn searching_works() {
        let d = db();
        let by = |q: &str| d.search(q).iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(by("9960"), vec!["9960-northeast-us"]);
        assert_eq!(by("99600"), vec!["9960-northeast-us"]);
        assert!(by("anthorn").contains(&"6731-anthorn-uk-eloran".to_string()));
        assert!(by("anthorn").contains(&"6731-lessay".to_string()));
        assert!(by("KR").contains(&"9930-korea".to_string()));
        assert!(by("chayka").contains(&"8000-western-russia-chayka".to_string()));
        assert!(d.by_region("east asia").len() >= 5);
        assert!(d.by_region("north america").len() >= 8);
        assert!(by("no such chain").is_empty());
        let op = d.active_on("2026-11-18");
        assert!(op.iter().any(|c| c.id == "9930-korea"));
        assert!(!op.iter().any(|c| c.id == "9960-northeast-us"));
    }

    #[test]
    fn user_merge_updates_and_adds() {
        let mut d = db();
        let n = d.chains.len();
        let user = r#"{
          "format": "kshana-eloran-gri-db", "version": 1, "updated": "2027-01-01",
          "chains": [
            { "id": "6731-lessay",
              "stations": [ { "id": "Y", "power_kw": 250.0, "sources": ["user: test value"] } ] },
            { "id": "9930-korea", "notes": "user note",
              "stations": [
                { "id": "M", "power_kw": 100.0 }, { "id": "W", "power_kw": 50.0 },
                { "id": "V", "position": { "lat_deg": 37.5, "lon_deg": 126.0 }, "power_kw": 50.0,
                  "emission_delay_us": 30000.0 } ] },
            { "id": "4321-future", "gri_us": 43210, "name": "4321 Future eLoran", "status": "planned",
              "region": "Testland", "operational_from": "2030",
              "stations": [
                { "id": "M", "role": "master", "position": { "lat_deg": 10.0, "lon_deg": 20.0 }, "power_kw": 100.0 },
                { "id": "X", "role": "secondary", "position": { "lat_deg": 11.0, "lon_deg": 20.0 },
                  "power_kw": 100.0, "emission_delay_us": 12000.0 } ] }
          ] }"#;
        let r = d.merge_json(user).unwrap();
        assert_eq!(r.updated, vec!["6731-lessay", "9930-korea"]);
        assert_eq!(r.added, vec!["4321-future"]);
        assert_eq!(d.chains.len(), n + 1);
        assert_eq!(d.updated, "2027-01-01");
        let l = d.get("6731-lessay").unwrap();
        assert!(l.is_complete());
        assert!(l.stations[2]
            .sources
            .iter()
            .any(|s| s == "user: test value"));
        assert_eq!(
            l.stations[2].emission_delay_us,
            Some(27300.0),
            "untouched field kept"
        );
        let k = d.get("9930-korea").unwrap();
        assert!(k.is_complete());
        k.to_chain_strict().unwrap();
        let f = d.get("4321-future").unwrap();
        assert_eq!(f.confidence, Confidence::Unverified);
        f.to_chain_strict().unwrap();
        assert_eq!(d.search("4321").len(), 1);

        // a bad merge leaves the database unchanged
        let before = d.clone();
        let clash = r#"{ "format": "kshana-eloran-gri-db", "version": 1,
          "chains": [ { "id": "x", "gri_us": 99600, "name": "clash", "status": "operational",
            "stations": [ { "id": "M", "role": "master" } ] } ] }"#;
        assert!(d.merge_json(clash).unwrap_err().contains("share GRI"));
        assert_eq!(d, before);
        let wrong = r#"{ "format": "other", "version": 1, "chains": [] }"#;
        assert!(d.merge_json(wrong).is_err());
        let new_without_gri = r#"{ "format": "kshana-eloran-gri-db", "version": 1,
          "chains": [ { "id": "y", "name": "n", "status": "planned", "stations": [] } ] }"#;
        assert!(d
            .merge_json(new_without_gri)
            .unwrap_err()
            .contains("gri_us"));
    }

    #[test]
    fn snippet_parses_and_runs_in_a_scenario() {
        let snip = scenario_snippet(9960).unwrap();
        assert!(snip.contains("[[chains]]") && snip.contains("gri_us = 99600"));
        let chain = GriDb::embedded()
            .unwrap()
            .get("9960-northeast-us")
            .unwrap()
            .to_chain_strict()
            .unwrap();
        let refs = time_refs_snippet(&chain).unwrap();
        assert!(refs.contains("UTC(USNO)"));
        let src = format!(
            r#"
[scene]
start_utc = "2026-11-18T00:00:00Z"
duration_s = 0.25
fs_hz = 400000
seed = 3
[receiver]
type = "static"
lat_deg = 41.0
lon_deg = -72.0
[[outputs]]
type = "e_field"
{refs}
{snip}"#
        );
        let sc = crate::Scenario::from_toml(&src).unwrap();
        assert_eq!(sc.chains[0].chain, chain);
        let (sink, truth, stats) = crate::run_to_memory(&src).unwrap();
        assert!(stats.pulses > 0);
        assert!(!truth.is_empty());
        assert_eq!(sink.channels.len(), 1);
        // incomplete / ambiguous / unknown GRIs are refused clearly
        assert!(scenario_snippet(9930)
            .unwrap_err()
            .contains("no complete chain"));
        assert!(scenario_snippet(4000).unwrap_err().contains("no chain"));
        // non-ASCII names survive TOML quoting
        let bo = GriDb::embedded().unwrap();
        let s = scenario_snippet_for(bo.get("7970-norwegian-sea").unwrap(), &Overrides::default())
            .unwrap();
        let parsed: toml::Value = toml::from_str(&s).unwrap();
        assert_eq!(
            parsed["chains"][0]["stations"][1]["name"].as_str(),
            Some("Bø, Norway")
        );
    }

    #[test]
    fn utc_k_list_and_time_refs() {
        let l = UtcKList::embedded().unwrap();
        assert!(l.labs.len() >= 80);
        for code in [
            "NPL", "PTB", "OP", "USNO", "KRIS", "NTSC", "NICT", "SU", "VSL", "NIST",
        ] {
            let lab = l.get(code).unwrap_or_else(|| panic!("{code}"));
            assert_eq!(lab.confidence, Confidence::Verified);
            assert!(lab.source.contains("Circular T"));
        }
        // Circular T 466: KRIS (Daejeon), NPL (Teddington).
        assert_eq!(l.get("utc(kris)").unwrap().city, "Daejeon");
        assert_eq!(l.get("NPL").unwrap().country.as_deref(), Some("GB"));
        let m = default_time_ref("UTC(NPL)").unwrap();
        assert_eq!(m, TimeRefModel::ideal("UTC(NPL)"));
        assert_eq!(default_time_ref("kris").unwrap().id, "UTC(KRIS)");
        assert!(default_time_ref("UTC(NOPE)").is_err());
        // illustrative: Circular T 466, UTC − UTC(USNO) = −0.2 ns on 2026-10-31 → x = +0.2 ns
        let i = illustrative_time_ref("USNO").unwrap();
        assert!((i.offset_ns - 0.2).abs() < 1e-9);
        // every chain time_ref in the database is a known laboratory
        for c in &db().chains {
            if let Some(r) = &c.time_ref {
                assert!(l.get(r).is_some(), "{}: {r}", c.id);
            }
        }
    }
}
