// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! National PNT dependency registers from signed operator audits (research 05 #46, 07 N03,
//! N11, N12, 07b R12, 12 #1730).
//!
//! A national PNT office, a regulator or a sector body wants to know how reliant a sector is
//! on GNSS without collecting every operator's system inventory. Each operator runs the
//! dependency audit ([`super::audit`]) and sends the signed directory; the register:
//!
//! * verifies every audit's signature (and, with trusted keys, its signer), and leaves out
//!   those that do not verify, saying so;
//! * keeps per operator only the **resilience status** ([`super::Status`]: level, counts,
//!   shortest autonomy) under a **pseudonym** (a keyed hash of the operator and asset names;
//!   the key stays with the register keeper), with the audit's hash as the evidence pointer;
//! * aggregates per sector (level distribution, totals, median and minimum autonomy) and
//!   **suppresses** any sector with fewer operators than the threshold, so no single
//!   operator's figures can be read off an aggregate.
//!
//! No node, system name or graph leaves the operator's audit. MODELLED: the levels are this
//! register's rule over the audit's figures, not a regulatory grading.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Audit, Status};

/// The register's format tag.
pub const FORMAT: &str = "kshana-pro-pnt-dependency-register/1";
/// The register's main file.
pub const RESULT_FILE: &str = "register.json";

/// One operator's entry.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub pseudonym: String,
    pub sector: String,
    pub status: Status,
    /// The SHA-256 of the signed audit.json, and the key id that signed it.
    pub audit_sha256: String,
    pub signer_key_id: String,
}

/// One sector's aggregate.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SectorAggregate {
    pub sector: String,
    pub operators: usize,
    /// Fewer operators than the threshold: figures withheld.
    pub suppressed: bool,
    pub levels: BTreeMap<String, usize>,
    pub critical_functions: usize,
    pub gnss_dependent: usize,
    pub silent_under_spoofing: usize,
    pub single_points: usize,
    pub min_autonomy_s: Option<f64>,
    pub median_autonomy_s: Option<f64>,
}

/// A source the register left out.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rejected {
    pub dir: String,
    pub reason: String,
}

/// The register.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Register {
    pub format: String,
    pub title: String,
    pub min_group: usize,
    pub statement: String,
    pub entries: Vec<Entry>,
    pub sectors: Vec<SectorAggregate>,
    pub rejected: Vec<Rejected>,
}

/// The keyed pseudonym of an operator's asset.
pub fn pseudonym(key: &[u8], operator: &str, asset: &str) -> String {
    use ring::hmac;
    let k = hmac::Key::new(hmac::HMAC_SHA256, key);
    let tag = hmac::sign(&k, format!("{operator}\u{1f}{asset}").as_bytes());
    format!("op-{}", &hex::encode(tag.as_ref())[..16])
}

/// Read one signed audit directory: the audit, its hash and the signer's key id.
pub fn read_audit(dir: &Path, trust: &[[u8; 32]]) -> Result<(Audit, String, String), String> {
    let v = crate::signed_dossier::verify(dir, trust).map_err(|e| e.to_string())?;
    if !v.passed() {
        return Err(format!("signature check failed: {}", v.problems.join("; ")));
    }
    let bytes = std::fs::read(dir.join(super::RESULT_FILE))
        .map_err(|e| format!("{}: {e}", super::RESULT_FILE))?;
    let a: Audit =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", super::RESULT_FILE))?;
    if a.format != super::AUDIT_FORMAT {
        return Err(format!("not a dependency audit ({})", a.format));
    }
    use sha2::{Digest, Sha256};
    Ok((a, hex::encode(Sha256::digest(&bytes)), v.key_id))
}

/// Build the register from audit directories.
pub fn build(
    title: &str,
    dirs: &[PathBuf],
    trust: &[[u8; 32]],
    key: &[u8],
    min_group: usize,
) -> Result<Register, String> {
    if key.len() < 16 {
        return Err("the pseudonym key must be at least 16 bytes".into());
    }
    let mut entries = Vec::new();
    let mut rejected = Vec::new();
    for d in dirs {
        match read_audit(d, trust) {
            Ok((a, sha, kid)) => entries.push(Entry {
                pseudonym: pseudonym(key, &a.operator, &a.asset),
                sector: a.sector.clone(),
                status: a.status.clone(),
                audit_sha256: sha,
                signer_key_id: kid,
            }),
            Err(e) => rejected.push(Rejected {
                dir: d
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                reason: e,
            }),
        }
    }
    entries.sort_by(|a, b| a.sector.cmp(&b.sector).then(a.pseudonym.cmp(&b.pseudonym)));
    // One entry per pseudonym: a repeated asset keeps its first audit.
    entries.dedup_by(|a, b| a.pseudonym == b.pseudonym);
    let mut by: BTreeMap<String, Vec<&Entry>> = BTreeMap::new();
    for e in &entries {
        by.entry(e.sector.clone()).or_default().push(e);
    }
    let min_group = min_group.max(1);
    let sectors = by
        .into_iter()
        .map(|(sector, es)| {
            let n = es.len();
            if n < min_group {
                return SectorAggregate {
                    sector,
                    operators: n,
                    suppressed: true,
                    levels: BTreeMap::new(),
                    critical_functions: 0,
                    gnss_dependent: 0,
                    silent_under_spoofing: 0,
                    single_points: 0,
                    min_autonomy_s: None,
                    median_autonomy_s: None,
                };
            }
            let mut levels = BTreeMap::new();
            for e in &es {
                *levels.entry(e.status.level.clone()).or_insert(0) += 1;
            }
            let mut aut: Vec<f64> = es.iter().filter_map(|e| e.status.min_autonomy_s).collect();
            aut.sort_by(|a, b| a.total_cmp(b));
            SectorAggregate {
                sector,
                operators: n,
                suppressed: false,
                levels,
                critical_functions: es.iter().map(|e| e.status.critical).sum(),
                gnss_dependent: es.iter().map(|e| e.status.gnss_dependent).sum(),
                silent_under_spoofing: es.iter().map(|e| e.status.silent_under_spoofing).sum(),
                single_points: es.iter().map(|e| e.status.single_points).sum(),
                min_autonomy_s: aut.first().copied(),
                median_autonomy_s: (!aut.is_empty()).then(|| aut[aut.len() / 2]),
            }
        })
        .collect();
    Ok(Register {
        format: FORMAT.into(),
        title: title.into(),
        min_group,
        statement: "Per-operator resilience status under keyed pseudonyms and per-sector \
                    aggregates from signed dependency audits. No inventory, node or system \
                    name is held. Sectors with fewer operators than min_group are withheld. \
                    Levels are this register's rule over the audit figures (MODELLED), not a \
                    regulatory grading."
            .into(),
        entries,
        sectors,
        rejected,
    })
}

/// Write the register directory: register.json and sectors.csv.
pub fn write_dir(dir: &Path, r: &Register) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::write(
        dir.join(RESULT_FILE),
        serde_json::to_string_pretty(r).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| format!("{RESULT_FILE}: {e}"))?;
    let mut csv = String::from(
        "sector,operators,suppressed,L1,L2,L3,critical_functions,gnss_dependent,silent_under_spoofing,single_points,min_autonomy_s,median_autonomy_s\n",
    );
    for s in &r.sectors {
        let l = |k: &str| s.levels.get(k).copied().unwrap_or(0);
        let o = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
        if s.suppressed {
            csv.push_str(&format!("{},{},true,,,,,,,,,\n", s.sector, s.operators));
        } else {
            csv.push_str(&format!(
                "{},{},false,{},{},{},{},{},{},{},{},{}\n",
                s.sector,
                s.operators,
                l("L1"),
                l("L2"),
                l("L3"),
                s.critical_functions,
                s.gnss_dependent,
                s.silent_under_spoofing,
                s.single_points,
                o(s.min_autonomy_s),
                o(s.median_autonomy_s)
            ));
        }
    }
    std::fs::write(dir.join("sectors.csv"), csv).map_err(|e| format!("sectors.csv: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pseudonyms_are_keyed_and_stable() {
        let a = pseudonym(b"0123456789abcdef", "Op", "Asset");
        assert_eq!(a, pseudonym(b"0123456789abcdef", "Op", "Asset"));
        assert_ne!(a, pseudonym(b"fedcba9876543210", "Op", "Asset"));
        assert_ne!(a, pseudonym(b"0123456789abcdef", "Op", "Asset2"));
        assert!(a.starts_with("op-") && a.len() == 19);
    }
}
