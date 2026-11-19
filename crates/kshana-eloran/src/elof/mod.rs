// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//! # ELOF v0.1: eLoran Observation & Scenario Format
//!
//! A versioned, self-describing exchange format for eLoran observations (what a receiver
//! measured), simulator truth (what really happened) and the scenarios behind them. The
//! normative description is `docs/ELOF.md` of this crate; this module is its reference
//! reader, writer and validator.
//!
//! * **JSON lines** (`.elof.jsonl`): line 1 is the header record, every following line one
//!   record (`obs`, `truth`, `timing`, `ldc`), each tagged by `"record"`.
//! * **Compact CSV profile** (`.elof.csv`, [`csv`]): the header as a JSON comment line,
//!   then `obs` rows only.
//! * **Converters** ([`convert`]): simulator truth table, software receiver output
//!   (measurements, time solution, data channel) and imported receiver logs
//!   ([`crate::field::importers::ObsRecord`]), both ways where the information allows.
//!
//! The observable is `toa_ns`: arrival of the group's pulse-0 time origin minus the
//! station's **nominal** emission of that group on the Loran schedule (group · GRI from the
//! Loran epoch plus the emission delay), on the file's time scale. It is a pseudo-delay:
//! propagation plus every clock offset. Times of records (`t_s`) are seconds since the
//! header's `epoch_utc` and must not decrease through the file.
//!
//! # Evidence label: MODELLED
//!
//! The format, its validator and converters are checked by internal round-trip and
//! rejection tests only. No external receiver has yet written ELOF; no standards body has
//! reviewed it. Version 0.1 is a proposal.

pub mod convert;
pub mod csv;
#[cfg(test)]
mod tests;

use crate::signal::{Parity, Role};
use crate::time::{AbsTime, Ps, PS_PER_US};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Format tag of the header.
pub const FORMAT: &str = "ELOF";
/// Version this implementation writes.
pub const VERSION: &str = "0.1";
/// Highest version this implementation reads (same major, minor ≤ this).
pub const SUPPORTED: (u32, u32) = (0, 1);

/// Units of every numeric field (normative; the header repeats them and the validator
/// refuses a file whose header states different units).
pub const UNITS: &[(&str, &str)] = &[
    ("t_s", "s since epoch_utc"),
    ("toa_ns", "ns"),
    ("ecd_us", "us"),
    ("snr_db", "dB (20 kHz)"),
    ("field_uv_m", "uV/m"),
    ("bearing_deg", "deg"),
    ("delay_ns", "ns"),
    ("asf_ns", "ns"),
    ("sky_delay_us", "us"),
    ("sky_ratio_db", "dB"),
    ("time_ref_offset_ns", "ns"),
    ("emission_error_ns", "ns"),
    ("ref_minus_rx_ns", "ns"),
    ("lat_deg", "deg (WGS84)"),
    ("distance_km", "km"),
];

/// The spec units as a map (what a writer puts in the header).
pub fn spec_units() -> BTreeMap<String, String> {
    UNITS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Flags of an observation (controlled vocabulary; shared with the receiver log
/// importers, [`crate::field::importers::flags`]).
pub const FLAGS: &[&str] = &[
    "cycle_slip",
    "cycle_warning",
    "snr_warning",
    "blink",
    "invalid",
    "no_fix",
];

/// Time scale of `t_s` and of the observables.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeScale {
    /// UTC (or a UTC(k), named in `time_reference`).
    Utc,
    Tai,
    Gps,
    /// The receiver's own clock (free running or disciplined; see `time_reference`).
    Receiver,
    /// The simulator's truth timeline.
    Scene,
}

/// Where the observations come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// Simulator truth only.
    Simulated,
    /// A software receiver run on simulated or recorded samples.
    SoftwareReceiver,
    /// A hardware receiver on a bench (conducted) stimulus.
    HardwareReceiver,
    /// A receiver in the field (live signals).
    Field,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub kind: SourceKind,
    /// Producing software and version.
    #[serde(default)]
    pub software: String,
    #[serde(default)]
    pub description: String,
}

/// Antenna type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AntennaKind {
    EField,
    HLoop,
    CrossedLoop,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AntennaMeta {
    #[serde(default)]
    pub kind: AntennaKind,
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    /// Antenna + cable delay (ns), when calibrated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cable_delay_ns: Option<f64>,
}

/// WGS84 position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub alt_m: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReceiverMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub firmware: String,
    /// Receiver internal delay (ns), when calibrated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal_delay_ns: Option<f64>,
    /// Surveyed (static) position, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(default)]
    pub antenna: AntennaMeta,
}

/// Kind of the receiver's time reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeRefKind {
    /// A national realisation UTC(k).
    UtcK,
    Gnss,
    Loran,
    FreeRunning,
    /// The simulator's truth timeline.
    Scene,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeReference {
    /// e.g. "UTC(NPL)", "GPST", "scene".
    pub id: String,
    pub kind: TimeRefKind,
    #[serde(default)]
    pub description: String,
}

/// A transmitting station the records may name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationMeta {
    pub chain: String,
    pub gri_us: u32,
    pub station: String,
    pub role: Role,
    pub lat_deg: f64,
    pub lon_deg: f64,
    #[serde(default)]
    pub emission_delay_us: f64,
    /// Time reference the station is steered to (e.g. "UTC(A)").
    #[serde(default)]
    pub time_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ldc_scheme: Option<String>,
}

/// One station's ASF as used by the receiver (or the scenario's model).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsfStation {
    pub chain: String,
    pub station: String,
    pub asf_ns: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sigma_ns: Option<f64>,
}

/// ASF metadata.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AsfMeta {
    /// ASF corrections are applied in the observables (false: raw pseudo-delays).
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_sha256: Option<String>,
    #[serde(default)]
    pub stations: Vec<AsfStation>,
}

/// A scenario the file relates to, referenced by SHA-256.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRef {
    pub sha256: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// The header record (line 1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub format: String,
    pub version: String,
    pub units: BTreeMap<String, String>,
    pub time_scale: TimeScale,
    /// UTC instant `t_s = 0` (`YYYY-MM-DDTHH:MM:SS[.f]Z`).
    pub epoch_utc: String,
    pub source: Source,
    #[serde(default)]
    pub receiver: ReceiverMeta,
    pub time_reference: TimeReference,
    pub stations: Vec<StationMeta>,
    #[serde(default)]
    pub asf: AsfMeta,
    #[serde(default)]
    pub scenarios: Vec<ScenarioRef>,
    /// `truth` records are present (simulated stimulus).
    #[serde(default)]
    pub truth_included: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<String>,
}

impl Header {
    /// A header with the spec's format, version and units.
    pub fn new(
        epoch_utc: &str,
        time_scale: TimeScale,
        source: Source,
        time_reference: TimeReference,
        stations: Vec<StationMeta>,
    ) -> Header {
        Header {
            format: FORMAT.into(),
            version: VERSION.into(),
            units: spec_units(),
            time_scale,
            epoch_utc: epoch_utc.into(),
            source,
            receiver: ReceiverMeta::default(),
            time_reference,
            stations,
            asf: AsfMeta::default(),
            scenarios: vec![],
            truth_included: false,
            comments: vec![],
        }
    }

    /// The station metadata of (chain, station).
    pub fn station(&self, chain: &str, station: &str) -> Option<&StationMeta> {
        self.stations
            .iter()
            .find(|s| s.chain == chain && s.station == station)
    }

    /// Absolute instant of `t_s = 0`.
    pub fn epoch(&self) -> Result<AbsTime, String> {
        AbsTime::from_utc_iso(&self.epoch_utc).map_err(|e| format!("epoch_utc: {e}"))
    }
}

/// Nominal emission (s since `epoch`) of pulse 0 of `st`'s group `group`: group · GRI from
/// the Loran epoch plus the emission delay (identical to
/// [`crate::rx::solve::nominal_emission_s`]).
pub fn nominal_emission_s(st: &StationMeta, group: i64, epoch: AbsTime) -> f64 {
    let gri = st.gri_us as i128 * PS_PER_US as i128;
    let abs = group as i128 * gri;
    (Ps((abs - epoch.0) as i64) + Ps::from_us(st.emission_delay_us)).as_s()
}

/// An observation of one station (one measurement / tracking window).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Obs {
    pub t_s: f64,
    pub chain: String,
    pub station: String,
    /// Group number since the Loran epoch the observable refers to.
    pub group: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<usize>,
    pub toa_ns: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ecd_us: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snr_db: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_uv_m: Option<f64>,
    /// Phase-code interval of the group (must agree with the group number's parity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_code: Option<Parity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bearing_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub flags: BTreeSet<String>,
}

/// Simulator truth of one station's group (as [`crate::TruthRecord`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Truth {
    pub t_s: f64,
    /// True emission of pulse 0 (s since `epoch_utc`).
    pub t_emit_s: f64,
    pub chain: String,
    pub station: String,
    pub group: i64,
    pub toa_ns: f64,
    pub delay_ns: f64,
    pub geom_ns: f64,
    pub pf_ns: f64,
    pub sf_ns: f64,
    pub asf_ns: f64,
    pub ecd_us: f64,
    pub field_uv_m: f64,
    /// SNR in 20 kHz; absent for a noise-free stimulus (infinite SNR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snr_db: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky_delay_us: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sky_ratio_db: Option<f64>,
    pub time_ref_offset_ns: f64,
    pub emission_error_ns: f64,
    pub rx_lat_deg: f64,
    pub rx_lon_deg: f64,
    pub rx_heading_deg: f64,
    pub bearing_deg: f64,
    pub distance_km: f64,
    /// Data-channel symbol transmitted in this group, when the chain carries LDC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ldc_symbol: Option<u32>,
}

/// A receiver time solution: a time reference (UTC(k)) minus the receiver clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    pub t_s: f64,
    pub time_ref: String,
    /// Per-station estimate when both are set; per time reference otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station: Option<String>,
    pub ref_minus_rx_ns: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub std_ns: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<usize>,
}

/// One decided data-channel symbol.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ldc {
    pub t_s: f64,
    pub chain: String,
    pub station: String,
    pub group: i64,
    pub scheme: String,
    pub symbol: u32,
    /// The group's pulse timing could not be measured (`symbol` is a placeholder).
    #[serde(default)]
    pub missing: bool,
}

/// A record after the header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
pub enum Record {
    Obs(Obs),
    Truth(Truth),
    Timing(Timing),
    Ldc(Ldc),
}

impl Record {
    pub fn t_s(&self) -> f64 {
        match self {
            Record::Obs(r) => r.t_s,
            Record::Truth(r) => r.t_s,
            Record::Timing(r) => r.t_s,
            Record::Ldc(r) => r.t_s,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Record::Obs(_) => "obs",
            Record::Truth(_) => "truth",
            Record::Timing(_) => "timing",
            Record::Ldc(_) => "ldc",
        }
    }
}

/// Serialised header line (the header carries `"record": "header"`).
#[derive(Serialize, Deserialize)]
#[serde(tag = "record", rename_all = "snake_case")]
enum HeaderLine {
    Header(Header),
}

/// An ELOF file in memory.
#[derive(Clone, Debug, PartialEq)]
pub struct ElofFile {
    pub header: Header,
    pub records: Vec<Record>,
}

impl ElofFile {
    pub fn new(header: Header) -> ElofFile {
        ElofFile {
            header,
            records: vec![],
        }
    }

    /// Sort records by `t_s` (stable: equal times keep their order).
    pub fn sort(&mut self) {
        self.records.sort_by(|a, b| a.t_s().total_cmp(&b.t_s()));
    }

    pub fn obs(&self) -> impl Iterator<Item = &Obs> {
        self.records.iter().filter_map(|r| match r {
            Record::Obs(o) => Some(o),
            _ => None,
        })
    }
    pub fn truth(&self) -> impl Iterator<Item = &Truth> {
        self.records.iter().filter_map(|r| match r {
            Record::Truth(o) => Some(o),
            _ => None,
        })
    }
    pub fn timing(&self) -> impl Iterator<Item = &Timing> {
        self.records.iter().filter_map(|r| match r {
            Record::Timing(o) => Some(o),
            _ => None,
        })
    }
    pub fn ldc(&self) -> impl Iterator<Item = &Ldc> {
        self.records.iter().filter_map(|r| match r {
            Record::Ldc(o) => Some(o),
            _ => None,
        })
    }

    /// JSON-lines text (header line, then one line per record).
    pub fn to_jsonl(&self) -> Result<String, String> {
        let mut s = serde_json::to_string(&HeaderLine::Header(self.header.clone()))
            .map_err(|e| e.to_string())?;
        s.push('\n');
        for r in &self.records {
            s.push_str(&serde_json::to_string(r).map_err(|e| e.to_string())?);
            s.push('\n');
        }
        Ok(s)
    }

    /// Parse JSON-lines text (structure only; see [`validate`] for the semantic checks).
    /// Blank lines are ignored.
    pub fn from_jsonl(text: &str) -> Result<ElofFile, String> {
        let mut lines = text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty());
        let (_, first) = lines.next().ok_or("ELOF: empty file")?;
        let v: serde_json::Value =
            serde_json::from_str(first).map_err(|e| format!("line 1: not JSON: {e}"))?;
        if v.get("record").and_then(|r| r.as_str()) != Some("header") {
            return Err(
                "line 1: the first record must be the header (\"record\": \"header\")".into(),
            );
        }
        check_version(&v)?;
        let HeaderLine::Header(header) =
            serde_json::from_value(v).map_err(|e| format!("line 1: header: {e}"))?;
        let mut records = Vec::new();
        for (i, l) in lines {
            let r: Record = serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1))?;
            records.push(r);
        }
        Ok(ElofFile { header, records })
    }
}

/// Parse "major.minor".
pub fn parse_version(v: &str) -> Option<(u32, u32)> {
    let (a, b) = v.split_once('.')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

/// Check the format tag and version before the rest of the header is interpreted (a
/// newer major version may change any field).
fn check_version(v: &serde_json::Value) -> Result<(), String> {
    let f = v.get("format").and_then(|x| x.as_str()).unwrap_or("");
    if f != FORMAT {
        return Err(format!(
            "line 1: format {f:?} is not {FORMAT:?} (not an ELOF file)"
        ));
    }
    let ver = v.get("version").and_then(|x| x.as_str()).unwrap_or("");
    let Some((maj, min)) = parse_version(ver) else {
        return Err(format!("line 1: version {ver:?} is not major.minor"));
    };
    if maj != SUPPORTED.0 || min > SUPPORTED.1 {
        return Err(format!(
            "line 1: ELOF version {ver} is not supported by this reader (reads {}.0 to {}.{})",
            SUPPORTED.0, SUPPORTED.0, SUPPORTED.1
        ));
    }
    Ok(())
}

/// One finding of the validator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    /// Record index (0-based, after the header); `None` for the header.
    pub record: Option<usize>,
    pub message: String,
}

/// Validator report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
    /// Records per kind.
    pub counts: BTreeMap<String, usize>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
    fn err(&mut self, record: Option<usize>, m: impl Into<String>) {
        self.errors.push(Issue {
            record,
            message: m.into(),
        });
    }
    fn warn(&mut self, record: Option<usize>, m: impl Into<String>) {
        self.warnings.push(Issue {
            record,
            message: m.into(),
        });
    }
    /// One-line summary.
    pub fn summary(&self) -> String {
        let c: Vec<String> = self
            .counts
            .iter()
            .map(|(k, v)| format!("{v} {k}"))
            .collect();
        format!(
            "{}: {} error(s), {} warning(s); records: {}",
            if self.ok() { "valid" } else { "INVALID" },
            self.errors.len(),
            self.warnings.len(),
            if c.is_empty() {
                "none".into()
            } else {
                c.join(", ")
            }
        )
    }
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Semantic validation: version, units, header completeness, station references, finite
/// values and ranges, phase code vs group parity, monotonic time, truth only when
/// declared.
pub fn validate(f: &ElofFile) -> Report {
    let mut r = Report::default();
    let h = &f.header;
    // version / format
    if h.format != FORMAT {
        r.err(None, format!("format {:?} is not {FORMAT:?}", h.format));
    }
    match parse_version(&h.version) {
        Some((maj, min)) if maj == SUPPORTED.0 && min <= SUPPORTED.1 => {}
        _ => r.err(
            None,
            format!("version {:?} not supported (reads 0.0 to 0.1)", h.version),
        ),
    }
    // units: every spec unit stated exactly
    for (k, v) in UNITS {
        match h.units.get(*k) {
            None => r.err(None, format!("units: {k} missing (spec: {v:?})")),
            Some(u) if u != v => r.err(
                None,
                format!("units: {k} is {u:?} but ELOF {VERSION} requires {v:?}"),
            ),
            _ => {}
        }
    }
    for k in h.units.keys() {
        if !UNITS.iter().any(|(s, _)| s == k) {
            r.warn(None, format!("units: unknown field {k:?} ignored"));
        }
    }
    let epoch = match h.epoch() {
        Ok(e) => Some(e),
        Err(e) => {
            r.err(None, e);
            None
        }
    };
    if h.time_reference.id.trim().is_empty() {
        r.err(None, "time_reference.id is required");
    }
    if h.stations.is_empty() {
        r.err(None, "stations: at least one station is required");
    }
    let mut seen = BTreeSet::new();
    for s in &h.stations {
        if s.chain.is_empty() || s.station.is_empty() {
            r.err(None, "stations: chain and station ids are required");
        }
        if !seen.insert((s.chain.as_str(), s.station.as_str())) {
            r.err(
                None,
                format!("stations: {}/{} declared twice", s.chain, s.station),
            );
        }
        if !(40_000..=99_990).contains(&s.gri_us) || s.gri_us % 10 != 0 {
            r.err(
                None,
                format!(
                    "stations: {}/{} GRI {} µs outside 40000..99990 in 10 µs steps",
                    s.chain, s.station, s.gri_us
                ),
            );
        }
        if !(s.lat_deg.abs() <= 90.0) || !(s.lon_deg.abs() <= 180.0) {
            r.err(
                None,
                format!("stations: {}/{} position out of range", s.chain, s.station),
            );
        }
        if !(s.emission_delay_us >= 0.0 && s.emission_delay_us < s.gri_us as f64) {
            r.err(
                None,
                format!(
                    "stations: {}/{} emission delay outside the GRI",
                    s.chain, s.station
                ),
            );
        }
    }
    if let Some(g) = &h.asf.grid_sha256 {
        if !is_sha256(g) {
            r.err(
                None,
                "asf.grid_sha256 is not a lower-case SHA-256 hex digest",
            );
        }
    }
    for a in &h.asf.stations {
        if h.station(&a.chain, &a.station).is_none() {
            r.err(
                None,
                format!("asf: {}/{} is not a declared station", a.chain, a.station),
            );
        }
        if !a.asf_ns.is_finite() || a.sigma_ns.is_some_and(|s| !(s >= 0.0)) {
            r.err(
                None,
                format!(
                    "asf: {}/{} values must be finite, σ ≥ 0",
                    a.chain, a.station
                ),
            );
        }
    }
    for s in &h.scenarios {
        if !is_sha256(&s.sha256) {
            r.err(
                None,
                format!(
                    "scenarios: {:?} is not a lower-case SHA-256 hex digest",
                    s.sha256
                ),
            );
        }
    }
    if h.source.kind == SourceKind::Simulated && !h.truth_included {
        r.warn(None, "source is simulated but truth_included is false");
    }
    // records
    let mut last_t = f64::NEG_INFINITY;
    for (i, rec) in f.records.iter().enumerate() {
        *r.counts.entry(rec.kind().into()).or_default() += 1;
        let t = rec.t_s();
        if !t.is_finite() {
            r.err(Some(i), "t_s is not finite");
        } else {
            if t < last_t {
                r.err(
                    Some(i),
                    format!("t_s {t} decreases (previous record {last_t}): time must be monotonic"),
                );
            }
            last_t = last_t.max(t);
        }
        let station = |chain: &str, st: &str, r: &mut Report| -> Option<StationMeta> {
            let m = h.station(chain, st).cloned();
            if m.is_none() {
                r.err(
                    Some(i),
                    format!("{chain}/{st} is not declared in the header's stations"),
                );
            }
            m
        };
        let fin = |name: &str, v: f64, r: &mut Report| {
            if !v.is_finite() {
                r.err(Some(i), format!("{name} is not finite"));
            }
        };
        let toa_ok = |m: &StationMeta, toa_ns: f64, r: &mut Report| {
            let gri_ns = m.gri_us as f64 * 1e3;
            if toa_ns.is_finite() && !(toa_ns.abs() < gri_ns) {
                r.err(
                    Some(i),
                    format!("toa_ns {toa_ns} beyond ± one GRI ({gri_ns} ns): wrong unit or group"),
                );
            }
        };
        match rec {
            Record::Obs(o) => {
                let m = station(&o.chain, &o.station, &mut r);
                fin("toa_ns", o.toa_ns, &mut r);
                if let Some(m) = &m {
                    toa_ok(m, o.toa_ns, &mut r);
                    if let Some(e) = epoch {
                        let nom = nominal_emission_s(m, o.group, e);
                        if t.is_finite() && (t - nom).abs() > 2.0 * m.gri_us as f64 * 1e-6 + 1.0 {
                            r.warn(
                                Some(i),
                                format!(
                                    "t_s {t} is far from group {}'s emission ({nom:.6} s)",
                                    o.group
                                ),
                            );
                        }
                    }
                }
                if let Some(v) = o.ecd_us {
                    if !(v.abs() <= 10.0) {
                        r.err(Some(i), format!("ecd_us {v} outside ±10 µs"));
                    }
                }
                if let Some(v) = o.snr_db {
                    if !(-60.0..=120.0).contains(&v) {
                        r.err(Some(i), format!("snr_db {v} outside −60..120 dB"));
                    }
                }
                if let Some(v) = o.field_uv_m {
                    if !(v >= 0.0 && v.is_finite()) {
                        r.err(Some(i), format!("field_uv_m {v} must be ≥ 0"));
                    }
                }
                if let Some(v) = o.bearing_deg {
                    if !(0.0..360.0).contains(&v) {
                        r.err(Some(i), format!("bearing_deg {v} outside [0, 360)"));
                    }
                }
                if let Some(p) = o.phase_code {
                    if p != Parity::of_group(o.group) {
                        r.err(
                            Some(i),
                            format!(
                                "phase_code {p:?} disagrees with group {} (parity {:?})",
                                o.group,
                                Parity::of_group(o.group)
                            ),
                        );
                    }
                }
                for fl in &o.flags {
                    if !FLAGS.contains(&fl.as_str()) {
                        r.warn(Some(i), format!("unknown flag {fl:?}"));
                    }
                }
            }
            Record::Truth(x) => {
                if !h.truth_included {
                    r.err(
                        Some(i),
                        "truth record but the header does not declare truth_included",
                    );
                }
                if let Some(m) = station(&x.chain, &x.station, &mut r) {
                    toa_ok(&m, x.toa_ns, &mut r);
                }
                for (n, v) in [
                    ("toa_ns", x.toa_ns),
                    ("delay_ns", x.delay_ns),
                    ("asf_ns", x.asf_ns),
                    ("ecd_us", x.ecd_us),
                    ("field_uv_m", x.field_uv_m),
                    ("snr_db", x.snr_db.unwrap_or(0.0)),
                    ("time_ref_offset_ns", x.time_ref_offset_ns),
                    ("emission_error_ns", x.emission_error_ns),
                ] {
                    fin(n, v, &mut r);
                }
                if (x.geom_ns + x.pf_ns + x.sf_ns + x.asf_ns - x.delay_ns).abs()
                    > 1e-6 * x.delay_ns.abs().max(1.0)
                {
                    // the simulator's delay is the sum of its parts
                    r.warn(Some(i), "delay_ns differs from geom + pf + sf + asf");
                }
                if !(x.rx_lat_deg.abs() <= 90.0) || !(x.rx_lon_deg.abs() <= 180.0) {
                    r.err(Some(i), "receiver position out of range");
                }
            }
            Record::Timing(x) => {
                fin("ref_minus_rx_ns", x.ref_minus_rx_ns, &mut r);
                if x.time_ref.is_empty() {
                    r.err(Some(i), "timing: time_ref is required");
                }
                match (&x.chain, &x.station) {
                    (Some(c), Some(s)) => {
                        station(c, s, &mut r);
                    }
                    (None, None) => {}
                    _ => r.err(Some(i), "timing: chain and station go together"),
                }
                if x.std_ns.is_some_and(|s| !(s >= 0.0)) {
                    r.err(Some(i), "timing: std_ns must be ≥ 0");
                }
            }
            Record::Ldc(x) => {
                if let Some(m) = station(&x.chain, &x.station, &mut r) {
                    if m.ldc_scheme.as_deref().is_some_and(|s| s != x.scheme) {
                        r.err(
                            Some(i),
                            format!(
                                "ldc scheme {:?} differs from the station's {:?}",
                                x.scheme, m.ldc_scheme
                            ),
                        );
                    }
                }
                if x.scheme.is_empty() {
                    r.err(Some(i), "ldc: scheme is required");
                }
            }
        }
    }
    if f.records.is_empty() {
        r.warn(None, "no records");
    }
    r
}

/// Parse and validate text in either profile (JSON lines, or CSV when the first line is
/// the CSV magic). A parse failure is reported as an error.
pub fn validate_text(text: &str) -> (Option<ElofFile>, Report) {
    let parsed = if csv::is_csv(text) {
        csv::from_csv(text)
    } else {
        ElofFile::from_jsonl(text)
    };
    match parsed {
        Ok(f) => {
            let r = validate(&f);
            (Some(f), r)
        }
        Err(e) => {
            let mut r = Report::default();
            r.err(None, e);
            (None, r)
        }
    }
}

/// Read a file in either profile, refusing an invalid one.
pub fn read_valid(text: &str) -> Result<ElofFile, String> {
    let (f, r) = validate_text(text);
    if !r.ok() {
        let first: Vec<String> = r
            .errors
            .iter()
            .take(5)
            .map(|e| match e.record {
                Some(i) => format!("record {i}: {}", e.message),
                None => e.message.clone(),
            })
            .collect();
        return Err(format!("ELOF invalid: {}", first.join("; ")));
    }
    f.ok_or_else(|| "ELOF: unreadable".into())
}
