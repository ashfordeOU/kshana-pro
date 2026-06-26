// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # SpaceStream — the NAVEX→NAV-DT interface (A09, ICD + codec)
//!
//! A **versioned, aggregated-metrics-only** message schema for the closed-loop
//! "SpaceStream" link a reconfigurable PNT payload (NAVEX) would use to downlink
//! per-pass performance summaries to the ground Navigation Digital Twin (NAV-DT).
//!
//! ## Hard schema rule (the honesty guard, enforced in code)
//! The message carries **only aggregated metrics and alerts** — per-pass timing /
//! availability / integrity figures and an RF-impairment *alert* (a label +
//! confidence + a few parameters). It **never** carries raw IQ, sample streams, or
//! field data. [`tests::schema_forbids_raw_sample_or_iq_fields`] fails if a raw-data
//! field is ever added.
//!
//! ## Status (honest framing)
//! This is an interface **DESIGN proposal** pending the real FPD-10030 SRD — **not**
//! an agreed or flight interface. It does not imply that NAVEX hardware exists or is
//! Ashforde's; Kshana Pro provides the ground-side schema + codec, one component of
//! a partner-led NAV-DT.
//!
//! Built on the open core's figure-of-merit types ([`kshana::fom::FoMScores`]) and
//! the connectors' [`super::EphemSample`] (for the pass epoch).

use std::collections::BTreeMap;

use kshana::api::KshanaError;
use serde::{Deserialize, Serialize};

use super::{jd_to_utc, EphemSample};

/// The message format discriminator.
pub const FORMAT: &str = "spacestream-navex-navdt";
/// The message schema version (semver). Compatibility is by **major** version.
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Aggregated ranging/timing accuracy figures (nanoseconds) — from [`kshana::fom`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RangingTimingFom {
    pub timing_rms_ns: f64,
    pub timing_p95_ns: f64,
}

/// Integrity flags (NOT a position-domain HPL/VPL — see `docs/INTEGRITY.md`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IntegrityFlags {
    /// Whether an ARAIM solution was available over the pass.
    pub araim_available: bool,
    /// Filter self-consistency fraction in `[0,1]`, if assessed.
    pub self_consistency: Option<f64>,
    /// True if any integrity alert fired over the pass.
    pub alert: bool,
}

/// Aggregated link/carrier summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkSummary {
    pub cn0_dbhz: f64,
    pub margin_db: f64,
}

/// An RF-impairment alert: an aggregated classification, never the underlying IQ.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RfImpairmentAlert {
    /// Impairment label, e.g. `"nominal"`, `"jamming"`, `"spoof-time"`, `"multipath"`.
    pub label: String,
    /// Classifier confidence in `[0,1]`.
    pub confidence: f64,
    /// A few aggregated decision parameters (e.g. `js_db`, `cn0_drop_db`). A
    /// `BTreeMap` so serialisation is deterministic (stable goldens).
    pub params: BTreeMap<String, f64>,
}

/// One SpaceStream downlink message: a per-pass aggregated performance summary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpaceStreamMessage {
    /// Always [`FORMAT`] — lets a NAV-DT consumer recognise the message.
    pub format: String,
    /// The [`SCHEMA_VERSION`] the message was produced under.
    pub schema_version: String,
    /// Opaque pass identifier.
    pub pass_id: String,
    /// Pass epoch as an ISO-8601-ish UTC string (deterministic; never wall-clock).
    pub pass_timestamp_utc: String,
    pub ranging_timing: RangingTimingFom,
    /// Fraction of the pass with an in-spec solution, `[0,1]`.
    pub availability: f64,
    pub integrity: IntegrityFlags,
    /// Aggregated RF-impairment alert, if one fired. `None` for a nominal pass.
    pub rf_impairment_alert: Option<RfImpairmentAlert>,
    pub link: LinkSummary,
}

/// Schema-version compatibility verdict for a consumer reading a foreign message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaCompat {
    /// Same major version — readable.
    Compatible,
    /// Different major version — refuse.
    Incompatible,
    /// Not a `MAJOR.MINOR.PATCH` semver of unsigned integers.
    Malformed,
}

fn semver(v: &str) -> Option<(u32, u32, u32)> {
    let mut p = v.split('.');
    let major = p.next()?.parse().ok()?;
    let minor = p.next()?.parse().ok()?;
    let patch = p.next()?.parse().ok()?;
    if p.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Compatibility of `version` against [`SCHEMA_VERSION`] (same-major rule).
pub fn compatibility(version: &str) -> SchemaCompat {
    let cur = semver(SCHEMA_VERSION).expect("SCHEMA_VERSION is valid semver");
    match semver(version) {
        None => SchemaCompat::Malformed,
        Some((maj, _, _)) if maj == cur.0 => SchemaCompat::Compatible,
        Some(_) => SchemaCompat::Incompatible,
    }
}

impl SpaceStreamMessage {
    /// Build a message from the open core's per-pass figures of merit plus the pass
    /// epoch (taken from an ephemeris sample's `jd_utc`). This is the genuine
    /// composition over the open FoM + connector types.
    pub fn from_pass(
        pass_id: &str,
        epoch: &EphemSample,
        fom: &kshana::fom::FoMScores,
        araim_available: bool,
        link: LinkSummary,
        rf_impairment_alert: Option<RfImpairmentAlert>,
    ) -> SpaceStreamMessage {
        let (y, mo, d, h, mi, s) = jd_to_utc(epoch.jd_utc);
        SpaceStreamMessage {
            format: FORMAT.into(),
            schema_version: SCHEMA_VERSION.into(),
            pass_id: pass_id.into(),
            pass_timestamp_utc: format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:06.3}Z"),
            ranging_timing: RangingTimingFom {
                timing_rms_ns: fom.timing_rms_ns,
                timing_p95_ns: fom.timing_p95_ns,
            },
            availability: fom.availability,
            integrity: IntegrityFlags {
                araim_available,
                self_consistency: fom.integrity,
                alert: rf_impairment_alert.is_some()
                    || fom.integrity.map(|c| c < 0.9).unwrap_or(false),
            },
            rf_impairment_alert,
            link,
        }
    }

    /// Compact canonical JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("SpaceStreamMessage serializes")
    }

    /// Indented JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).expect("SpaceStreamMessage serializes")
    }

    /// Parse + validate: must be a SpaceStream message of a compatible major version.
    pub fn parse(json: &str) -> Result<SpaceStreamMessage, KshanaError> {
        let msg: SpaceStreamMessage = serde_json::from_str(json)
            .map_err(|e| KshanaError::InvalidInput(format!("not a SpaceStream message: {e}")))?;
        if msg.format != FORMAT {
            return Err(KshanaError::InvalidInput(format!(
                "wrong format '{}', expected '{FORMAT}'",
                msg.format
            )));
        }
        match compatibility(&msg.schema_version) {
            SchemaCompat::Compatible => Ok(msg),
            other => Err(KshanaError::InvalidInput(format!(
                "incompatible SpaceStream schema version '{}' ({other:?}); this build speaks '{SCHEMA_VERSION}'",
                msg.schema_version
            ))),
        }
    }
}

/// Emit a JSON Schema (draft 2020-12) describing the message — the machine-readable
/// half of the ICD. `additionalProperties:false` plus the absence of any IQ/sample
/// field is the schema-level guarantee that only aggregated metrics flow.
pub fn json_schema() -> String {
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "SpaceStream NAVEX→NAV-DT message",
        "type": "object",
        "additionalProperties": false,
        "required": ["format","schema_version","pass_id","pass_timestamp_utc","ranging_timing","availability","integrity","link"],
        "properties": {
            "format": {"const": FORMAT},
            "schema_version": {"type":"string","description":"semver; compatibility by major"},
            "pass_id": {"type":"string"},
            "pass_timestamp_utc": {"type":"string","description":"ISO-8601 UTC; deterministic, never wall-clock"},
            "ranging_timing": {
                "type":"object","additionalProperties": false,
                "required":["timing_rms_ns","timing_p95_ns"],
                "properties":{"timing_rms_ns":{"type":"number"},"timing_p95_ns":{"type":"number"}}
            },
            "availability": {"type":"number","minimum":0.0,"maximum":1.0},
            "integrity": {
                "type":"object","additionalProperties": false,
                "required":["araim_available","alert"],
                "properties":{"araim_available":{"type":"boolean"},"self_consistency":{"type":["number","null"]},"alert":{"type":"boolean"}}
            },
            "rf_impairment_alert": {
                "type":["object","null"],"additionalProperties": false,
                "required":["label","confidence","params"],
                "properties":{"label":{"type":"string"},"confidence":{"type":"number","minimum":0.0,"maximum":1.0},"params":{"type":"object","additionalProperties":{"type":"number"}}}
            },
            "link": {
                "type":"object","additionalProperties": false,
                "required":["cn0_dbhz","margin_db"],
                "properties":{"cn0_dbhz":{"type":"number"},"margin_db":{"type":"number"}}
            }
        }
    });
    serde_json::to_string_pretty(&schema).expect("schema serializes")
}

/// The canonical golden messages — the source of truth the on-disk samples under
/// `scenarios/spacestream-*.json` must match. Keeping them in code lets a test
/// detect drift between the schema and the shipped samples.
pub fn golden_messages() -> Vec<(&'static str, SpaceStreamMessage)> {
    let p = |pairs: &[(&str, f64)]| -> BTreeMap<String, f64> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    };
    vec![
        (
            "spacestream-nominal",
            SpaceStreamMessage {
                format: FORMAT.into(),
                schema_version: SCHEMA_VERSION.into(),
                pass_id: "PASS-0001".into(),
                pass_timestamp_utc: "2026-06-15T00:00:00.000Z".into(),
                ranging_timing: RangingTimingFom {
                    timing_rms_ns: 2.5,
                    timing_p95_ns: 4.1,
                },
                availability: 0.999,
                integrity: IntegrityFlags {
                    araim_available: true,
                    self_consistency: Some(0.98),
                    alert: false,
                },
                rf_impairment_alert: None,
                link: LinkSummary {
                    cn0_dbhz: 45.0,
                    margin_db: 8.0,
                },
            },
        ),
        (
            "spacestream-jamming-alert",
            SpaceStreamMessage {
                format: FORMAT.into(),
                schema_version: SCHEMA_VERSION.into(),
                pass_id: "PASS-0002".into(),
                pass_timestamp_utc: "2026-06-15T01:30:00.000Z".into(),
                ranging_timing: RangingTimingFom {
                    timing_rms_ns: 30.0,
                    timing_p95_ns: 62.0,
                },
                availability: 0.82,
                integrity: IntegrityFlags {
                    araim_available: true,
                    self_consistency: Some(0.71),
                    alert: true,
                },
                rf_impairment_alert: Some(RfImpairmentAlert {
                    label: "jamming".into(),
                    confidence: 0.93,
                    params: p(&[("js_db", 35.0), ("cn0_drop_db", 12.0)]),
                }),
                link: LinkSummary {
                    cn0_dbhz: 33.0,
                    margin_db: -2.0,
                },
            },
        ),
        (
            "spacestream-spoof-time-alert",
            SpaceStreamMessage {
                format: FORMAT.into(),
                schema_version: SCHEMA_VERSION.into(),
                pass_id: "PASS-0003".into(),
                pass_timestamp_utc: "2026-06-15T03:00:00.000Z".into(),
                ranging_timing: RangingTimingFom {
                    timing_rms_ns: 12.0,
                    timing_p95_ns: 25.0,
                },
                availability: 0.95,
                integrity: IntegrityFlags {
                    araim_available: true,
                    self_consistency: Some(0.6),
                    alert: true,
                },
                rf_impairment_alert: Some(RfImpairmentAlert {
                    label: "spoof-time".into(),
                    confidence: 0.88,
                    params: p(&[("agc_excess_db", 6.0), ("sqm_metric", 0.42)]),
                }),
                link: LinkSummary {
                    cn0_dbhz: 44.0,
                    margin_db: 7.0,
                },
            },
        ),
        (
            "spacestream-degraded-no-alert",
            SpaceStreamMessage {
                format: FORMAT.into(),
                schema_version: SCHEMA_VERSION.into(),
                pass_id: "PASS-0004".into(),
                pass_timestamp_utc: "2026-06-15T04:30:00.000Z".into(),
                ranging_timing: RangingTimingFom {
                    timing_rms_ns: 18.0,
                    timing_p95_ns: 40.0,
                },
                availability: 0.6,
                integrity: IntegrityFlags {
                    araim_available: false,
                    self_consistency: Some(0.82),
                    alert: true,
                },
                rf_impairment_alert: None,
                link: LinkSummary {
                    cn0_dbhz: 40.0,
                    margin_db: 3.0,
                },
            },
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epoch_sample() -> EphemSample {
        EphemSample {
            t_s: 0.0,
            jd_utc: 2_451_545.0, // 2000-01-01 12:00:00 UTC
            gcrs_r_m: [1.0e6, 2.0e6, 3.0e6],
            gcrs_v_m_s: [10.0, 20.0, 30.0],
            ecef_r_m: [4.0e6, 5.0e6, 6.0e6],
            lat_deg: 45.0,
            lon_deg: -120.0,
            alt_km: 400.0,
            speed_m_s: 7600.0,
        }
    }

    #[test]
    fn round_trips_every_golden_in_memory() {
        for (name, msg) in golden_messages() {
            let back = SpaceStreamMessage::parse(&msg.to_json())
                .unwrap_or_else(|e| panic!("{name} failed to round-trip: {e}"));
            assert_eq!(back, msg, "{name} changed across the codec");
            // Pretty form round-trips identically too.
            let back2 = SpaceStreamMessage::parse(&msg.to_json_pretty()).unwrap();
            assert_eq!(back2, msg);
        }
    }

    #[test]
    fn parse_rejects_wrong_format() {
        let mut m = golden_messages()[0].1.clone();
        m.format = "some-other-tool".into();
        assert!(SpaceStreamMessage::parse(&m.to_json()).is_err());
    }

    #[test]
    fn schema_version_compatibility_is_by_major() {
        assert_eq!(compatibility("1.0.0"), SchemaCompat::Compatible);
        assert_eq!(compatibility("1.4.2"), SchemaCompat::Compatible); // newer minor/patch, same major
        assert_eq!(compatibility("2.0.0"), SchemaCompat::Incompatible);
        assert_eq!(compatibility("0.9.0"), SchemaCompat::Incompatible);
        assert_eq!(compatibility("1.0"), SchemaCompat::Malformed); // not 3-part
        assert_eq!(compatibility("x.y.z"), SchemaCompat::Malformed);
    }

    #[test]
    fn parse_refuses_incompatible_major() {
        let mut m = golden_messages()[0].1.clone();
        m.schema_version = "2.0.0".into();
        assert!(SpaceStreamMessage::parse(&m.to_json()).is_err());
    }

    #[test]
    fn schema_forbids_raw_sample_or_iq_fields() {
        // The aggregated-only guarantee: no message field carries raw IQ / samples.
        let schema = json_schema();
        for forbidden in [
            "\"iq\"",
            "\"samples\"",
            "\"raw\"",
            "\"waveform\"",
            "\"i_q\"",
        ] {
            assert!(
                !schema.contains(forbidden),
                "schema must not expose a raw-data field ({forbidden})"
            );
        }
        // And the schema is closed (additionalProperties:false) so none can be added
        // ad hoc by a producer.
        assert!(schema.contains("\"additionalProperties\": false"));
        // A serialized message likewise has no such field.
        let json = golden_messages()[1].1.to_json();
        assert!(!json.contains("\"iq\"") && !json.contains("\"samples\""));
    }

    #[test]
    fn from_pass_composes_fom_and_epoch() {
        let fom = kshana::fom::FoMScores {
            timing_rms_ns: 5.0,
            timing_p95_ns: 9.0,
            holdover_s: 1000.0,
            resilience_slope_ns_per_s: 0.1,
            availability: 0.97,
            integrity: Some(0.85),
            security: None,
        };
        let msg = SpaceStreamMessage::from_pass(
            "PASS-X",
            &epoch_sample(),
            &fom,
            true,
            LinkSummary {
                cn0_dbhz: 42.0,
                margin_db: 5.0,
            },
            None,
        );
        assert_eq!(msg.ranging_timing.timing_rms_ns, 5.0);
        assert_eq!(msg.availability, 0.97);
        assert_eq!(msg.integrity.self_consistency, Some(0.85));
        // self_consistency 0.85 < 0.9 ⇒ integrity alert flagged.
        assert!(msg.integrity.alert);
        // Epoch formatted from the J2000 JD.
        assert!(msg.pass_timestamp_utc.starts_with("2000-01-01T12:00:00"));
        // Round-trips.
        assert_eq!(SpaceStreamMessage::parse(&msg.to_json()).unwrap(), msg);
    }

    #[test]
    fn json_schema_is_valid_json_and_closed() {
        let schema: serde_json::Value = serde_json::from_str(&json_schema()).unwrap();
        assert_eq!(schema["properties"]["format"]["const"], FORMAT);
        assert_eq!(schema["additionalProperties"], false);
    }

    #[test]
    fn shipped_golden_files_match_the_canonical_messages() {
        // The on-disk samples under scenarios/spacestream-*.json must parse to the
        // exact canonical messages — so the shipped goldens cannot drift from the
        // schema. (Set KSHANA_REGEN_SPACESTREAM=1 to (re)write them.)
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/scenarios");
        let regen = std::env::var("KSHANA_REGEN_SPACESTREAM").is_ok();
        for (name, msg) in golden_messages() {
            let path = format!("{dir}/{name}.json");
            if regen {
                std::fs::write(&path, msg.to_json_pretty() + "\n").unwrap();
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("missing golden {path}: {e}"));
            let parsed = SpaceStreamMessage::parse(&text)
                .unwrap_or_else(|e| panic!("golden {name} failed to parse: {e}"));
            assert_eq!(
                parsed, msg,
                "golden file {name} drifted from the canonical message"
            );
        }
    }
}
