// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Operating envelope of every Pro output (research 2026-11-20 gaps, B2508)
//!
//! Every Pro output says where it may be relied on: the inputs it is valid for, the ranges its
//! method was built and exercised over, and what it does not cover. The envelope is chosen by
//! the output's `format` tag through a table of **families** ([`FAMILIES`]); every format the
//! crate writes belongs to one (`tests/operating_envelope.rs` scans the source for every format
//! tag and fails on one that falls through to the generic envelope).
//!
//! [`attach`] writes `operating-envelope.json` into an evidence directory whose top-level JSON
//! carries a Kshana format; [`crate::signed_dossier::sign`] calls it, so every signed Pro
//! evidence directory carries its envelope under the signature. A directory with no Kshana
//! format (a user's own files signed with `dossier sign`) is left as it is.
//!
//! The envelope states scope; it is not a claim that an output meets any standard.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Format tag of an envelope.
pub const FORMAT: &str = "kshana-pro-operating-envelope/1";
/// File name inside an evidence directory.
pub const FILE: &str = "operating-envelope.json";

/// A stated range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub quantity: String,
    pub range: String,
}

/// The envelope of one output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub format: String,
    /// The output this envelope belongs to.
    pub output_format: String,
    pub family: String,
    pub valid_inputs: Vec<String>,
    pub ranges: Vec<Range>,
    pub exclusions: Vec<String>,
    pub evidence: String,
}

/// One family of outputs.
pub struct Family {
    pub name: &'static str,
    /// Tokens of the format name (split at `-`) that place a format in this family.
    pub tokens: &'static [&'static str],
    pub valid_inputs: &'static [&'static str],
    pub ranges: &'static [(&'static str, &'static str)],
    pub exclusions: &'static [&'static str],
}

/// Exclusions of every Pro output.
pub const COMMON_EXCLUSIONS: &[&str] = &[
    "Does not locate a radio emitter, control an antenna, fuse sensors for navigation, or transmit.",
    "Is not an approval, licence, type approval or regulatory finding; any such decision is for the competent body.",
    "Exercised on synthetic or published inputs unless the output names a validation data set; MODELLED otherwise.",
    "Inputs are taken as supplied: errors or manipulation in them carry through.",
];

const GEO: (&str, &str) = (
    "positions",
    "latitude -90..90 deg, longitude -180..180 deg (WGS 84); declared areas must not cross the antimeridian",
);

/// The families; for a format, the earliest token of its name that a family claims decides.
pub const FAMILIES: &[Family] = &[
    Family {
        name: "interference-and-forensics",
        tokens: &["interference", "forensic", "incident", "dispute", "log", "attack", "threat", "area", "alert", "watch", "planner", "fdm", "complaint", "custody", "product", "referee", "coexistence"],
        valid_inputs: &[
            "receiver logs (NMEA, RINEX, vendor CSV as documented), Kshana Station events, interference reports as supplied",
            "thresholds pre-registered in the request",
        ],
        ranges: &[GEO, ("time", "UTC; inputs without a time reference are refused")],
        exclusions: &["Says what the data are consistent with; not a finding of intent or of who was responsible."],
    },
    Family {
        name: "maritime",
        tokens: &["maritime", "ais", "ecdis", "bridge", "voyage", "transit", "mass", "bam", "wwrns", "flag", "sanctions", "iuu", "subsea", "vms", "casualty", "departure", "survey"],
        valid_inputs: &[
            "decoded AIS positions and static data, VMS, LRIT, radar or ECDIS exports as documented; vessel NMEA logs",
            "user-declared areas, ports, zones and asset routes",
        ],
        ranges: &[GEO, ("speed", "0..102.2 kn as broadcast; implied speeds above the stated limit are jumps")],
        exclusions: &["Bundles no sanctions list, maritime boundary or asset route; makes no sanctions, fisheries or attribution determination."],
    },
    Family {
        name: "aviation-and-airspace",
        tokens: &["flight", "rnp", "notam", "nuisance", "residual", "reset", "unpublished", "approach", "avionics", "low", "utm", "ussp", "rid", "navaid", "regional", "adsb", "aviation"],
        valid_inputs: &[
            "flight-data or ADS-B exports, crew reports, procedure definitions and avionics logs as documented",
        ],
        ranges: &[GEO, ("altitude", "as reported; barometric and geometric altitudes are not mixed")],
        exclusions: &["Decision support only; not an airworthiness, operational or airspace approval."],
    },
    Family {
        name: "land-transport-agriculture-and-cellular",
        tokens: &["road", "rail", "nr", "agnss", "v2x", "telematics", "obu", "ecall", "emergency", "pl", "tolerance", "gnb", "guidance", "agri", "ue"],
        valid_inputs: &[
            "telematics, on-board-unit, eCall test, CAM, rail passage, farm-machine and device logs as documented; values transcribed by the user with their source",
        ],
        ranges: &[GEO, ("phase budget", "as stated in the request; default ±1.5 µs to a common reference, to confirm against the release in use")],
        exclusions: &["Ships no 3GPP, ETSI or CEN test value that was not read; pre-assessment only, not a test verdict."],
    },
    Family {
        name: "timing",
        tokens: &["timing", "holdover", "timebomb", "clock", "station", "sfn", "grid", "time", "calibration"],
        valid_inputs: &[
            "time-error, phase, PMU, SCADA and clock measurement series with their reference as documented",
            "oscillator classes or stated noise coefficients",
        ],
        ranges: &[("time error", "seconds; holdover predictions up to ten times the declared period"), ("averaging time", "as covered by the data; ADEV/TDEV beyond one third of the record are not reported")],
        exclusions: &["Model predictions are bounds under the stated oscillator model, not measurements."],
    },
    Family {
        name: "insurance-and-finance",
        tokens: &["accumulation", "parametric", "underwriting", "claims", "bi", "basis", "tail", "spillover", "exposure", "inspection", "payout"],
        valid_inputs: &["station C/N0 series, fleet or route exposure data and policy terms supplied by the user"],
        ranges: &[GEO, ("money", "no prices or premiums are produced; payouts follow the user's stated schedule")],
        exclusions: &["Not insurance, financial or actuarial advice; modelled figures for the user's own assessment."],
    },
    Family {
        name: "eloran-and-terrestrial",
        tokens: &["eloran", "loran", "dme", "vor", "rmode", "navaids", "wwrns", "crisis"],
        valid_inputs: &["eLoran or R-Mode receiver logs, station tables and ASF data as documented"],
        ranges: &[GEO, ("signal", "LF 100 kHz eLoran; other bands as the output states")],
        exclusions: &["Simulator outputs are for conducted use only."],
    },
    Family {
        name: "receivers-space-and-test",
        tokens: &["receiver", "osnma", "unit", "scheme", "equipment", "firmware", "pics", "testkit", "scorecard", "space", "sbas", "lugre", "lunar", "trust", "link", "verification", "dut", "procedure", "verdict", "trace", "params", "fleet", "kit", "campaign", "mission", "see", "inject", "db", "sequence", "stream", "fingerprints", "lines", "prereg", "review", "failure", "labels", "ca", "identification", "interface", "sim", "traim", "consistency", "l1", "recorder", "replay", "selftest", "tco", "equivalence", "vectors", "precision", "sensitivity", "fingerprint", "cislunar"],
        valid_inputs: &["receiver and test-bench logs, simulator exports, mission profiles and campaign files as documented"],
        ranges: &[("signals", "civil open signals only; no PRS, M-code or other restricted signals")],
        exclusions: &["Scores are under the named method and pre-registered bars; they are not type approval."],
    },
    Family {
        name: "evidence-and-governance",
        tokens: &["signed", "pack", "results", "report", "audit", "backup", "pre", "regulator", "framework", "bid", "dependency", "graph", "inventory", "dossier", "longterm", "licence", "revocations", "key", "feeder", "keyring", "provenance", "obs", "catalogue", "rating", "check", "operating", "envelope", "evidence", "feed", "ks", "meter", "method", "rescoring", "telemetry", "vault"],
        valid_inputs: &["Kshana evidence directories, signed packs and the user's declared architecture or catalogues"],
        ranges: &[("signatures", "Ed25519 over SHA-256 file digests; verification is offline")],
        exclusions: &["States which evidence speaks to a requirement and what is missing; never that a product or operator meets a document."],
    },
];

/// The family of a format tag (`None` when no family claims it).
pub fn family_of(format: &str) -> Option<&'static Family> {
    let name = format.split('/').next().unwrap_or(format);
    let name = name
        .strip_prefix("kshana-pro-")
        .or_else(|| name.strip_prefix("kshana-"))
        .or_else(|| name.strip_prefix("kshana."))
        .unwrap_or(name);
    // The earliest token any family claims decides (`space-rx-exposure` is a receiver output,
    // `pnt-interference-event` an interference one).
    name.split('-')
        .find_map(|t| FAMILIES.iter().find(|f| f.tokens.contains(&t)))
}

/// The envelope of an output format; a format no family claims gets the common envelope only.
pub fn for_format(format: &str) -> Envelope {
    let f = family_of(format);
    let mut exclusions: Vec<String> = f
        .map(|f| f.exclusions.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default();
    exclusions.extend(COMMON_EXCLUSIONS.iter().map(|s| s.to_string()));
    Envelope {
        format: FORMAT.into(),
        output_format: format.into(),
        family: f.map(|f| f.name).unwrap_or("general").into(),
        valid_inputs: f
            .map(|f| f.valid_inputs.iter().map(|s| s.to_string()).collect())
            .unwrap_or_else(|| {
                vec!["inputs as documented for the command that wrote this output".into()]
            }),
        ranges: f
            .map(|f| {
                f.ranges
                    .iter()
                    .map(|(q, r)| Range {
                        quantity: q.to_string(),
                        range: r.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        exclusions,
        evidence: "MODELLED unless the output itself names a validation data set.".into(),
    }
}

/// The Kshana format of an evidence directory: the `format` of the first top-level JSON file
/// (sorted by name) that carries one starting with `kshana`.
pub fn detect(dir: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.ends_with(".json")
                && n != FILE
                && n != crate::signed_dossier::SIGNATURE_FILE
                && n != crate::signed_dossier::LONGTERM_FILE
        })
        .collect();
    names.sort();
    names.into_iter().find_map(|n| {
        let text = std::fs::read_to_string(dir.join(&n)).ok()?;
        let v: serde_json::Value = serde_json::from_str(&text).ok()?;
        v.get("format")?
            .as_str()
            .filter(|f| f.starts_with("kshana"))
            .map(str::to_string)
    })
}

/// Write `operating-envelope.json` into `dir` when it carries a Kshana format and has none
/// yet; returns whether one is present afterwards.
pub fn attach(dir: &Path) -> Result<bool, String> {
    let path = dir.join(FILE);
    if path.exists() {
        return Ok(true);
    }
    let Some(f) = detect(dir) else {
        return Ok(false);
    };
    let text = serde_json::to_string_pretty(&for_format(&f)).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(true)
}
