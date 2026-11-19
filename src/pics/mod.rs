// PROPRIETARY — Kshana Pro. Not part of the open-source engine. See LICENSE.
//
//! # PICS: PNT Interference Conformance Suite (W1119 C1; research 02 gaps 1, 7, 8, 14; play P1)
//!
//! A versioned suite of interference scenarios (`pics/suite-v1.toml`): jamming (CW, chirp,
//! broadband) and spoofing (simplistic, intermediate, time push, position drag, meaconing),
//! each with its threat parameters, the expected device behaviour in words, and pass bars
//! written down before any device is scored.
//!
//! * **Pre-registration and tamper detection.** Every scenario's bars are hashed (SHA-256 of
//!   their canonical JSON) into `pics/suite-v1.lock`, compiled into the binary. A suite whose
//!   bars do not hash to the lock is refused ([`Suite::check_lock`]), so a bar cannot be
//!   loosened after the fact without a new suite version; the scorecard carries each bar
//!   hash and the suite hash, and is signed with the dossier signature
//!   ([`crate::signed_dossier`]), so a changed scorecard fails verification.
//! * **Clause mapping** (`pics/mapping-v1.toml`, [`Mapping`]): MSC.401(95), MSC.1/Circ.1575,
//!   IEC 61108-1, ED-259A App. W, DO-401A (draft), IEEE P1952 (draft), DHS RPCF levels,
//!   ITU-T G.8272.1 / G.8273.2 and IEEE C37.238. A clause number is cited only where the
//!   research or a source the open engine transcribed gives one; every other row says
//!   "mapping to be confirmed" ([`UNCONFIRMED`]).
//! * **Runner** ([`score`]): scores a device's logged response (the common JSON log
//!   [`log::DeviceLog`], or NMEA or a timing log converted to it) against the bars and maps
//!   the result to the DHS resilience levels with the receiver scorecards' level logic
//!   ([`crate::scorecards::levels`]).
//! * **Simulator export** ([`export`]): each scenario as a GNSS degradation timeline rendered
//!   for the Skydel, Spirent, LabSat and R&S SMBV command profiles of `kshana-eloran::sync`.
//!
//! **Evidence label: MODELLED.** The scoring is checked by internal tests on synthetic device
//! logs; no device has been scored against PICS in this build, and no external dataset
//! validates the bars. A scorecard is test evidence **mapped to** the provisions above; it
//! never says a device is compliant, conformant or certified. The bars are illustrative until
//! confirmed against the standards.

pub mod export;
pub mod log;
pub mod score;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The licence feature of `kshana-pro pics score` (the receiver-scorecard feature: PICS is a
/// receiver test campaign under a fixed, versioned suite). `list`, `show` and `export` are free.
pub const FEATURE: &str = "pro-receiver-scorecard";
/// The suite file this build carries.
pub const SUITE_V1: &str = include_str!("../../pics/suite-v1.toml");
/// The pre-registered bar hashes of [`SUITE_V1`].
pub const LOCK_V1: &str = include_str!("../../pics/suite-v1.lock");
/// The clause mapping of suite v1.
pub const MAPPING_V1: &str = include_str!("../../pics/mapping-v1.toml");
/// The clause text of a row whose clause the research does not give.
pub const UNCONFIRMED: &str = "mapping to be confirmed";
/// The evidence label of the suite and every scorecard.
pub const LABEL: &str = "MODELLED: PICS verdicts computed from the device's logged \
response under bars pre-registered in the suite (hash-locked). Threat parameters are \
representative, not read off a recording; bars are illustrative until confirmed against the \
standards they are mapped to. Test evidence for the scenarios run, mapped to the provisions \
listed; not a statement that any device meets a standard, and not a certification.";

/// The suite's header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteInfo {
    pub id: String,
    pub version: String,
    pub title: String,
    pub truth_lat_deg: f64,
    pub truth_lon_deg: f64,
    pub truth_height_m: f64,
    pub baseline_s: f64,
    pub threat_s: f64,
    pub recovery_s: f64,
    pub recovery_dwell_s: f64,
}

/// Pass bars of one scenario (absent: the criterion does not apply to the scenario's domain).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bars {
    /// Detection needs an explicit flag (jamming, spoofing or integrity); a withdrawn fix
    /// does not count.
    pub explicit_flag_required: bool,
    pub detect_within_s: f64,
    #[serde(default)]
    pub max_unflagged_h_error_m: Option<f64>,
    #[serde(default)]
    pub max_unflagged_time_error_ns: Option<f64>,
    pub recover_within_s: f64,
    #[serde(default)]
    pub recover_h_error_m: Option<f64>,
    #[serde(default)]
    pub recover_time_error_ns: Option<f64>,
    #[serde(default)]
    pub bounded_h_error_m: Option<f64>,
    #[serde(default)]
    pub bounded_time_error_ns: Option<f64>,
    pub full_availability: f64,
    #[serde(default)]
    pub full_h_error_m: Option<f64>,
    #[serde(default)]
    pub full_time_error_ns: Option<f64>,
}

impl Bars {
    /// SHA-256 of the canonical JSON of the bars (fields in declaration order, no white space).
    pub fn sha256(&self) -> String {
        hex::encode(Sha256::digest(
            serde_json::to_vec(self).expect("bars serialise"),
        ))
    }
}

/// One scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    /// `jamming` or `spoofing`.
    pub class: String,
    /// `cw`, `chirp`, `broadband`, `simplistic`, `intermediate`, `time-push`,
    /// `position-drag` or `meaconing`.
    pub technique: String,
    /// `position` and/or `timing`.
    pub domains: Vec<String>,
    pub title: String,
    pub expected: String,
    pub params: BTreeMap<String, f64>,
    pub bars: Bars,
}

impl Scenario {
    pub fn has_domain(&self, d: &str) -> bool {
        self.domains.iter().any(|x| x == d)
    }
}

/// The suite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub suite: SuiteInfo,
    #[serde(rename = "scenario")]
    pub scenarios: Vec<Scenario>,
}

/// The techniques a v1 suite may use, by class.
pub const TECHNIQUES: [(&str, &str); 8] = [
    ("jamming", "cw"),
    ("jamming", "chirp"),
    ("jamming", "broadband"),
    ("spoofing", "simplistic"),
    ("spoofing", "intermediate"),
    ("spoofing", "time-push"),
    ("spoofing", "position-drag"),
    ("spoofing", "meaconing"),
];

impl Suite {
    /// Parse and validate a suite file.
    pub fn parse(src: &str) -> Result<Suite, String> {
        let s: Suite = toml::from_str(src).map_err(|e| format!("PICS suite: {e}"))?;
        s.validate()?;
        Ok(s)
    }

    /// The suite this build carries (v1), checked against its lock.
    pub fn builtin() -> Suite {
        let s = Suite::parse(SUITE_V1).expect("the built-in PICS suite parses");
        s.check_lock(LOCK_V1)
            .expect("the built-in PICS suite matches its lock");
        s
    }

    pub fn validate(&self) -> Result<(), String> {
        let i = &self.suite;
        for (n, v) in [
            ("baseline_s", i.baseline_s),
            ("threat_s", i.threat_s),
            ("recovery_s", i.recovery_s),
            ("recovery_dwell_s", i.recovery_dwell_s),
        ] {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("PICS suite: {n} must be > 0"));
            }
        }
        if i.recovery_dwell_s >= i.recovery_s {
            return Err("PICS suite: recovery_dwell_s must be shorter than recovery_s".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.scenarios {
            if !seen.insert(s.id.as_str()) {
                return Err(format!("PICS suite: duplicate scenario {}", s.id));
            }
            if !TECHNIQUES.contains(&(s.class.as_str(), s.technique.as_str())) {
                return Err(format!(
                    "{}: unknown class/technique {}/{}",
                    s.id, s.class, s.technique
                ));
            }
            if s.domains.is_empty() || s.domains.iter().any(|d| d != "position" && d != "timing") {
                return Err(format!("{}: domains must be position and/or timing", s.id));
            }
            let b = &s.bars;
            let pos = s.has_domain("position");
            let tim = s.has_domain("timing");
            let need = |name: &str, v: Option<f64>, want: bool| -> Result<(), String> {
                match (v, want) {
                    (Some(x), true) if x.is_finite() && x > 0.0 => Ok(()),
                    (None, false) => Ok(()),
                    (Some(_), true) => Err(format!("{}: bars.{name} must be > 0", s.id)),
                    (None, true) => Err(format!("{}: bars.{name} is required", s.id)),
                    (Some(_), false) => Err(format!(
                        "{}: bars.{name} given for a domain the scenario does not test",
                        s.id
                    )),
                }
            };
            need("max_unflagged_h_error_m", b.max_unflagged_h_error_m, pos)?;
            need("recover_h_error_m", b.recover_h_error_m, pos)?;
            need("bounded_h_error_m", b.bounded_h_error_m, pos)?;
            need("full_h_error_m", b.full_h_error_m, pos)?;
            need(
                "max_unflagged_time_error_ns",
                b.max_unflagged_time_error_ns,
                tim,
            )?;
            need("recover_time_error_ns", b.recover_time_error_ns, tim)?;
            need("bounded_time_error_ns", b.bounded_time_error_ns, tim)?;
            need("full_time_error_ns", b.full_time_error_ns, tim)?;
            if !(b.detect_within_s > 0.0 && b.detect_within_s < i.threat_s) {
                return Err(format!("{}: bars.detect_within_s out of range", s.id));
            }
            if !(b.recover_within_s >= 0.0
                && b.recover_within_s + i.recovery_dwell_s <= i.recovery_s)
            {
                return Err(format!(
                    "{}: bars.recover_within_s plus the dwell must fit the recovery period",
                    s.id
                ));
            }
            if !(0.0..=1.0).contains(&b.full_availability) {
                return Err(format!(
                    "{}: bars.full_availability must be in [0, 1]",
                    s.id
                ));
            }
        }
        Ok(())
    }

    pub fn scenario(&self, id: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }

    /// Onset and end of the threat, s from the scenario start.
    pub fn threat_window(&self) -> (f64, f64) {
        (
            self.suite.baseline_s,
            self.suite.baseline_s + self.suite.threat_s,
        )
    }

    /// Total scenario length, s.
    pub fn duration_s(&self) -> f64 {
        self.suite.baseline_s + self.suite.threat_s + self.suite.recovery_s
    }

    /// The lock text of this suite: one `<scenario id> <bars sha256>` line per scenario.
    pub fn lock_text(&self) -> String {
        let mut s = format!(
            "# PICS {} {} pre-registered bar hashes (SHA-256 of each scenario's canonical bars JSON).\n",
            self.suite.id, self.suite.version
        );
        for sc in &self.scenarios {
            s.push_str(&format!("{} {}\n", sc.id, sc.bars.sha256()));
        }
        s
    }

    /// Check every scenario's bars against a lock; a scenario added, removed or changed is
    /// reported by id.
    pub fn check_lock(&self, lock: &str) -> Result<(), String> {
        let mut want: BTreeMap<&str, &str> = BTreeMap::new();
        for l in lock.lines() {
            let l = l.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            let (id, h) = l
                .split_once(' ')
                .ok_or_else(|| format!("PICS lock: bad line '{l}'"))?;
            want.insert(id, h.trim());
        }
        let mut problems = Vec::new();
        for sc in &self.scenarios {
            match want.remove(sc.id.as_str()) {
                None => problems.push(format!("{}: not in the lock", sc.id)),
                Some(h) if h != sc.bars.sha256() => {
                    problems.push(format!("{}: bars changed since pre-registration", sc.id))
                }
                Some(_) => {}
            }
        }
        for id in want.keys() {
            problems.push(format!("{id}: pre-registered but missing from the suite"));
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "PICS suite does not match its pre-registered lock: {}",
                problems.join("; ")
            ))
        }
    }

    /// SHA-256 of the suite file's bytes.
    pub fn file_sha256(src: &str) -> String {
        hex::encode(Sha256::digest(src.as_bytes()))
    }
}

/// A standard or framework of the mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// One mapping row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappingRow {
    pub standard: String,
    /// A cited clause, or exactly [`UNCONFIRMED`].
    pub clause: String,
    pub topic: String,
    pub scenarios: Vec<String>,
    /// Where the clause (or the absence of one) comes from.
    pub basis: String,
}

impl MappingRow {
    pub fn confirmed(&self) -> bool {
        self.clause != UNCONFIRMED
    }
}

/// The clause mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    #[serde(rename = "standard")]
    pub standards: Vec<Standard>,
    #[serde(rename = "row")]
    pub rows: Vec<MappingRow>,
}

/// The standards whose clause numbers PICS may cite: the research or an open-engine
/// transcription gives them. Rows of any other standard must say [`UNCONFIRMED`].
pub const CITABLE: [&str; 5] = [
    "imo-msc1-circ-1575",
    "eurocae-ed-259a",
    "dhs-rpcf",
    "itu-t-g8272-1",
    "itu-t-g8273-2",
];

/// The standards the v1 mapping must cover.
pub const REQUIRED_STANDARDS: [&str; 10] = [
    "imo-msc-401-95",
    "imo-msc1-circ-1575",
    "iec-61108-1",
    "eurocae-ed-259a",
    "rtca-do-401a",
    "ieee-p1952",
    "dhs-rpcf",
    "itu-t-g8272-1",
    "itu-t-g8273-2",
    "ieee-c37-238",
];

impl Mapping {
    pub fn parse(src: &str) -> Result<Mapping, String> {
        toml::from_str(src).map_err(|e| format!("PICS mapping: {e}"))
    }

    pub fn builtin() -> Mapping {
        Mapping::parse(MAPPING_V1).expect("the built-in PICS mapping parses")
    }

    /// Coverage problems against a suite: an unknown standard or scenario, a required
    /// standard without a row, a scenario without a row, a clause cited for a standard whose
    /// clauses are not citable, a row without a basis.
    pub fn coverage_problems(&self, suite: &Suite) -> Vec<String> {
        let mut p = Vec::new();
        let std_ids: Vec<&str> = self.standards.iter().map(|s| s.id.as_str()).collect();
        for req in REQUIRED_STANDARDS {
            if !std_ids.contains(&req) {
                p.push(format!("standard {req} is not declared"));
            }
            if !self.rows.iter().any(|r| r.standard == req) {
                p.push(format!("standard {req} has no mapping row"));
            }
        }
        for r in &self.rows {
            if !std_ids.contains(&r.standard.as_str()) {
                p.push(format!("row of undeclared standard {}", r.standard));
            }
            if r.confirmed() && !CITABLE.contains(&r.standard.as_str()) {
                p.push(format!(
                    "row '{}' of {} cites clause '{}' but that standard's clauses are not in the research",
                    r.topic, r.standard, r.clause
                ));
            }
            if r.basis.trim().is_empty() {
                p.push(format!("row '{}' has no basis", r.topic));
            }
            if r.scenarios.is_empty() {
                p.push(format!("row '{}' maps no scenario", r.topic));
            }
            for s in &r.scenarios {
                if suite.scenario(s).is_none() {
                    p.push(format!("row '{}' names unknown scenario {s}", r.topic));
                }
            }
        }
        for sc in &suite.scenarios {
            if !self
                .rows
                .iter()
                .any(|r| r.scenarios.iter().any(|x| x == &sc.id))
            {
                p.push(format!("scenario {} is mapped to nothing", sc.id));
            }
        }
        p
    }

    /// The rows that name a scenario.
    pub fn rows_for(&self, scenario: &str) -> Vec<&MappingRow> {
        self.rows
            .iter()
            .filter(|r| r.scenarios.iter().any(|s| s == scenario))
            .collect()
    }

    pub fn standard(&self, id: &str) -> Option<&Standard> {
        self.standards.iter().find(|s| s.id == id)
    }
}
