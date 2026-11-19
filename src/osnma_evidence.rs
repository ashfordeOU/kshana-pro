// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # OSNMA status block for signed packs (B1404)
//!
//! Every Pro pack is signed through [`crate::signed_dossier`]. This module adds an optional
//! OSNMA status block to that signed statement, so any pack (scorecards, forensics, timing,
//! frameworks, exposure, studies, dossiers) can record whether the Galileo navigation data
//! behind it was authenticated by OSNMA when it was produced.
//!
//! The block holds six fields and nothing more:
//!
//! | Field | Meaning |
//! |---|---|
//! | `status` | `authenticated`, `failed` or `unavailable` (the open verifier's overall status), or `not_recorded` (a migrated older pack) |
//! | `key_id` | public key id (PKID) of the last DSM-KROOT that verified, if any |
//! | `chain_id` | TESLA chain id (CID) of that KROOT, if any |
//! | `merkle_root` | the Merkle tree root (a SHA-256 hash, hex) the verifier was given, if any |
//! | `verifier` | the open verifier that produced the status (`kshana <version>`) |
//! | `freshness_checked` | whether the verifier enforced freshness against an independent reference time |
//!
//! What the block is not: it is not a time authentication, a position authentication or a
//! comparison of receiver time with any trusted time; it records the open verifier's status
//! of the navigation data and the trust anchors it used. `freshness_checked: false` means the
//! status says "authentic, not necessarily fresh" (`docs/OSNMA.md` of the open engine). The
//! advisory of the open verifier applies: not type-approved navigation equipment, not a
//! navigation integrity service.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The status values a block may carry.
pub const STATUSES: [&str; 4] = ["authenticated", "failed", "unavailable", "not_recorded"];

/// The OSNMA status block of a signed pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsnmaBlock {
    pub status: String,
    pub key_id: Option<u8>,
    pub chain_id: Option<u8>,
    pub merkle_root: Option<String>,
    pub verifier: String,
    pub freshness_checked: bool,
}

impl OsnmaBlock {
    /// The block a migrated pack signed before the block existed carries.
    pub fn not_recorded() -> Self {
        Self {
            status: "not_recorded".into(),
            key_id: None,
            chain_id: None,
            merkle_root: None,
            verifier: String::new(),
            freshness_checked: false,
        }
    }

    /// Check the field values (a block read from a file is checked before it is signed or
    /// trusted).
    pub fn validate(&self) -> Result<(), String> {
        if !STATUSES.contains(&self.status.as_str()) {
            return Err(format!(
                "osnma.status '{}' is not one of {STATUSES:?}",
                self.status
            ));
        }
        if let Some(c) = self.chain_id {
            if c > 3 {
                return Err(format!("osnma.chain_id {c} is not a 2-bit chain id"));
            }
        }
        if let Some(k) = self.key_id {
            if k > 15 {
                return Err(format!("osnma.key_id {k} is not a 4-bit key id"));
            }
        }
        if let Some(r) = &self.merkle_root {
            if r.len() != 64
                || !r
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err("osnma.merkle_root must be 64 lowercase hex digits".into());
            }
        }
        if self.status == "not_recorded" {
            if *self != Self::not_recorded() {
                return Err(
                    "an osnma block with status not_recorded carries no other value".into(),
                );
            }
        } else if !self.verifier.starts_with("kshana ") {
            return Err("osnma.verifier must name the open verifier (kshana <version>)".into());
        }
        Ok(())
    }

    /// The block for a report of the open verifier (`kshana osnma verify --json`, or
    /// [`kshana::osnma::cli::verify_report`]), run as `verifier` with the Merkle root it was given.
    pub fn from_report(
        report: &Value,
        merkle_root: Option<[u8; 32]>,
        verifier: &str,
    ) -> Result<Self, String> {
        let status = report["overall"]
            .as_str()
            .ok_or("the OSNMA report has no overall status")?
            .to_string();
        let last_kroot = report["events"]
            .as_array()
            .and_then(|v| v.iter().rfind(|e| e["kind"] == "kroot_verified"));
        let num = |k: &str| {
            last_kroot
                .and_then(|e| e[k].as_u64())
                .and_then(|n| u8::try_from(n).ok())
        };
        let b = Self {
            status,
            key_id: num("pkid"),
            chain_id: num("cid"),
            merkle_root: merkle_root.map(hex::encode),
            verifier: verifier.to_string(),
            freshness_checked: report["freshness_enforced"].as_bool().unwrap_or(false),
        };
        b.validate()?;
        Ok(b)
    }

    /// Run the open verifier embedded in this build on `input` (I/NAV pages or a u-blox
    /// stream) with the given trust anchors and reference time, and make the block.
    pub fn verify_input(
        input: Vec<u8>,
        label: &str,
        merkle_root: Option<[u8; 32]>,
        public_keys: &[String],
        reference_time: Option<u32>,
    ) -> Result<Self, String> {
        let keys = public_keys
            .iter()
            .map(|k| kshana::osnma::cli::parse_public_key(k))
            .collect::<Result<Vec<_>, _>>()?;
        let p = kshana::osnma::cli::VerifyParams {
            label,
            format: None,
            start_gst: None,
            merkle_root,
            keys,
            reference: reference_time,
            max_err: None,
        };
        let (report, _) = kshana::osnma::cli::verify_report(input, &p)?;
        Self::from_report(&report, merkle_root, &verifier_name())
    }

    /// Read a block file written by `kshana-pro osnma-status`.
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let b: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        b.validate()?;
        Ok(b)
    }
}

/// The name of the open verifier this build embeds.
pub fn verifier_name() -> String {
    format!("kshana {}", crate::report_brand::engine_version())
}

/// The block every signature made by this process carries, set once from the command line
/// (`--osnma-status <block.json>`, accepted by every signing command, like `--sample`).
static AMBIENT: Mutex<Option<OsnmaBlock>> = Mutex::new(None);

/// Set (or clear) the block that signatures made by this process carry.
pub fn set_ambient(b: Option<OsnmaBlock>) {
    if let Ok(mut g) = AMBIENT.lock() {
        *g = b;
    }
}

/// The block set for this process, if any.
pub fn ambient() -> Option<OsnmaBlock> {
    AMBIENT.lock().ok().and_then(|g| g.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_report_becomes_a_block_with_the_last_verified_kroot() {
        let r = json!({
            "overall": "authenticated",
            "freshness_enforced": true,
            "events": [
                {"kind": "kroot_verified", "cid": 3, "pkid": 1},
                {"kind": "key_verified"},
                {"kind": "kroot_verified", "cid": 1, "pkid": 2}
            ]
        });
        let b = OsnmaBlock::from_report(&r, Some([0xab; 32]), "kshana 0.41.0").unwrap();
        assert_eq!(b.status, "authenticated");
        assert_eq!((b.key_id, b.chain_id), (Some(2), Some(1)));
        assert_eq!(b.merkle_root.as_deref(), Some("ab".repeat(32).as_str()));
        assert!(b.freshness_checked);
        let r = json!({"overall": "unavailable", "freshness_enforced": false, "events": []});
        let b = OsnmaBlock::from_report(&r, None, "kshana 0.41.0").unwrap();
        assert_eq!((b.key_id, b.chain_id, b.merkle_root), (None, None, None));
        assert!(!b.freshness_checked);
    }

    #[test]
    fn invalid_blocks_are_refused() {
        let ok = OsnmaBlock {
            status: "failed".into(),
            key_id: Some(1),
            chain_id: Some(3),
            merkle_root: None,
            verifier: "kshana 0.41.0".into(),
            freshness_checked: false,
        };
        assert!(ok.validate().is_ok());
        for bad in [
            OsnmaBlock {
                status: "time-authenticated".into(),
                ..ok.clone()
            },
            OsnmaBlock {
                chain_id: Some(4),
                ..ok.clone()
            },
            OsnmaBlock {
                key_id: Some(16),
                ..ok.clone()
            },
            OsnmaBlock {
                merkle_root: Some("AB".repeat(32)),
                ..ok.clone()
            },
            OsnmaBlock {
                verifier: "other 1.0".into(),
                ..ok.clone()
            },
            OsnmaBlock {
                status: "not_recorded".into(),
                ..ok.clone()
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?}");
        }
        assert!(OsnmaBlock::not_recorded().validate().is_ok());
        // No field beyond the six.
        let extra = r#"{"status":"failed","key_id":1,"chain_id":3,"merkle_root":null,"verifier":"kshana 0.41.0","freshness_checked":false,"time_authenticated":true}"#;
        assert!(serde_json::from_str::<OsnmaBlock>(extra).is_err());
    }
}
