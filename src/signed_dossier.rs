// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Signed dossiers (roadmap F7)
//!
//! A mission dossier, a study dossier or any delivery directory is signed as a whole:
//! `signature.json` lists every file under the directory with its SHA-256, the signer's
//! name, the time and the signer's Ed25519 public key, and carries an Ed25519 signature
//! over the canonical JSON of that list. Verification needs nothing but the directory
//! and, to know *who* signed, the signer's public key obtained by another route (pinned
//! with `--trust`): it recomputes every hash, reports a changed, missing or added file,
//! and checks the signature. No network is used.
//!
//! The manifest and ledger a dossier already has show that its files agree with each
//! other; the signature adds who issued them. Signing is a Pro feature
//! (`pro-signed-dossier`); verifying is not gated, so a recipient without a licence
//! checks a delivery with `kshana-pro dossier verify`, or with
//! `scripts/verify-dossier-signature.py` (Python and its `cryptography` package).
//!
//! ## Formats
//!
//! * `kshana-dossier-signature/1`: the statement above.
//! * `kshana-dossier-signature/2` (B1404): the same, plus an OSNMA status block (`osnma`,
//!   [`crate::osnma_evidence::OsnmaBlock`]) and, for a pack migrated from format 1, the SHA-256
//!   of the signature file it replaces (`supersedes`). Both are inside the signed statement.
//!   A signature is written in format 2 only when it carries a block; without one it stays
//!   format 1, byte for byte as before, so every existing pack and verifier keeps working.
//!   Verification reads both; a format-1 pack reads as "OSNMA not recorded".
//!   [`migrate`] re-signs a verified format-1 pack as format 2.

use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use kshana::api::KshanaError;
use sha2::{Digest, Sha256};

use crate::license::{key_id, public_key_for, KEY_LEN};
use crate::osnma_evidence::OsnmaBlock;

/// The signature file's name.
pub const SIGNATURE_FILE: &str = "signature.json";
/// Its format tag (format 1: no OSNMA block).
pub const FORMAT: &str = "kshana-dossier-signature/1";
/// The format tag of a statement that carries an OSNMA status block.
pub const FORMAT_V2: &str = "kshana-dossier-signature/2";

/// One signed file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SignedFile {
    pub path: String,
    pub sha256: String,
}

/// The signed statement (everything in `signature.json` except the signature).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Statement {
    pub format: String,
    pub signer: String,
    pub utc: String,
    pub public_key: String,
    pub key_id: String,
    pub files: Vec<SignedFile>,
    /// Format 2 only: the OSNMA status block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub osnma: Option<OsnmaBlock>,
    /// Format 2 only, for a migrated pack: SHA-256 of the format-1 signature file it replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct SignatureFile {
    #[serde(flatten)]
    statement: Statement,
    signature: String,
}

/// The result of a verification.
#[derive(Debug, Clone, Default)]
pub struct Verification {
    pub signer: String,
    pub utc: String,
    pub key_id: String,
    pub files: usize,
    /// The signature holds over the statement.
    pub signature_ok: bool,
    /// The signing key is one of the `--trust` keys (`None`: no key was pinned).
    pub trusted: Option<bool>,
    pub problems: Vec<String>,
    /// The statement's format number (1 or 2).
    pub format: u32,
    /// The OSNMA status block; a format-1 pack has none ("not recorded").
    pub osnma: Option<OsnmaBlock>,
}

impl Verification {
    /// Signature holds, every file matches, and the key is pinned and trusted.
    pub fn passed(&self) -> bool {
        self.signature_ok && self.problems.is_empty() && self.trusted == Some(true)
    }
}

fn canonical(st: &Statement) -> Result<Vec<u8>, KshanaError> {
    // serde_json writes struct fields in declaration order and no white space, and
    // `files` is sorted, so the bytes are canonical for a given statement.
    serde_json::to_vec(st).map_err(|e| KshanaError::InvalidInput(format!("statement: {e}")))
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

/// Every regular file under `dir` (sorted, `/`-separated), except the signature file.
/// Symbolic links are refused: a signature covers bytes in the directory only.
pub fn list_files(dir: &Path) -> Result<Vec<SignedFile>, KshanaError> {
    fn walk(root: &Path, d: &Path, out: &mut Vec<PathBuf>) -> Result<(), KshanaError> {
        let rd = std::fs::read_dir(d)
            .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", d.display())))?;
        for ent in rd {
            let ent = ent.map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
            let p = ent.path();
            let ft = ent
                .file_type()
                .map_err(|e| KshanaError::InvalidInput(e.to_string()))?;
            if ft.is_symlink() {
                return Err(KshanaError::InvalidInput(format!(
                    "{}: symbolic links cannot be signed",
                    rel(root, &p)
                )));
            } else if ft.is_dir() {
                walk(root, &p, out)?;
            } else if ft.is_file() {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    walk(dir, dir, &mut paths)?;
    let mut files = Vec::new();
    for p in paths {
        let r = rel(dir, &p);
        if r == SIGNATURE_FILE {
            continue;
        }
        let bytes =
            std::fs::read(&p).map_err(|e| KshanaError::InvalidInput(format!("{r}: {e}")))?;
        files.push(SignedFile {
            path: r,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
    }
    files.sort();
    Ok(files)
}

/// Sign `dir` with `seed` as `signer` at `utc`; writes and returns `signature.json`. The
/// signature carries the OSNMA status block set for this process (`--osnma-status`, see
/// [`crate::osnma_evidence::set_ambient`]), if any.
pub fn sign(
    dir: &Path,
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
) -> Result<PathBuf, KshanaError> {
    sign_with(
        dir,
        seed,
        signer,
        utc,
        crate::osnma_evidence::ambient().as_ref(),
    )
}

/// As [`sign`], with an explicit OSNMA status block (`None`: format 1, no block).
pub fn sign_with(
    dir: &Path,
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
    osnma: Option<&OsnmaBlock>,
) -> Result<PathBuf, KshanaError> {
    write_signature(dir, seed, signer, utc, osnma, None)
}

fn write_signature(
    dir: &Path,
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
    osnma: Option<&OsnmaBlock>,
    supersedes: Option<String>,
) -> Result<PathBuf, KshanaError> {
    if let Some(b) = osnma {
        b.validate().map_err(KshanaError::InvalidInput)?;
    }
    if signer.trim().is_empty() {
        return Err(KshanaError::InvalidInput(
            "a signature needs a signer name".into(),
        ));
    }
    let files = list_files(dir)?;
    if files.is_empty() {
        return Err(KshanaError::InvalidInput(format!(
            "{}: nothing to sign",
            dir.display()
        )));
    }
    let pk = public_key_for(seed);
    let statement = Statement {
        format: if osnma.is_some() { FORMAT_V2 } else { FORMAT }.into(),
        signer: signer.trim().into(),
        utc: utc.into(),
        public_key: hex::encode(pk),
        key_id: key_id(&pk),
        files,
        osnma: osnma.cloned(),
        supersedes,
    };
    let sig = SigningKey::from_bytes(seed).sign(&canonical(&statement)?);
    let out = SignatureFile {
        statement,
        signature: hex::encode(sig.to_bytes()),
    };
    let path = dir.join(SIGNATURE_FILE);
    let text = serde_json::to_string_pretty(&out)
        .map_err(|e| KshanaError::InvalidInput(e.to_string()))?
        + "\n";
    std::fs::write(&path, text)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
    Ok(path)
}

/// Verify `dir` against its `signature.json`, trusting the public keys in `trust`
/// (empty: report the key, trust nothing).
pub fn verify(dir: &Path, trust: &[[u8; KEY_LEN]]) -> Result<Verification, KshanaError> {
    let path = dir.join(SIGNATURE_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
    let sf: SignatureFile = serde_json::from_str(&text)
        .map_err(|e| KshanaError::InvalidInput(format!("{SIGNATURE_FILE}: {e}")))?;
    let st = sf.statement;
    let format = match st.format.as_str() {
        FORMAT => 1,
        FORMAT_V2 => 2,
        other => {
            return Err(KshanaError::InvalidInput(format!(
                "{SIGNATURE_FILE}: unknown format '{other}'"
            )))
        }
    };
    let pk = crate::license::parse_key_hex(&st.public_key)?;
    let mut v = Verification {
        signer: st.signer.clone(),
        utc: st.utc.clone(),
        key_id: key_id(&pk),
        files: st.files.len(),
        format,
        osnma: st.osnma.clone(),
        ..Default::default()
    };
    match (format, &st.osnma) {
        (1, Some(_)) => v
            .problems
            .push("a format-1 statement carries an OSNMA block".into()),
        (1, None) if st.supersedes.is_some() => v
            .problems
            .push("a format-1 statement carries a supersedes field".into()),
        (2, None) => v
            .problems
            .push("a format-2 statement has no OSNMA block".into()),
        (2, Some(b)) => {
            if let Err(e) = b.validate() {
                v.problems.push(e);
            }
        }
        _ => {}
    }
    if v.key_id != st.key_id {
        v.problems.push("key_id does not match public_key".into());
    }
    let sig: Option<[u8; 64]> = hex::decode(&sf.signature)
        .ok()
        .and_then(|b| b.try_into().ok());
    v.signature_ok = match (VerifyingKey::from_bytes(&pk), sig) {
        (Ok(k), Some(s)) => k
            .verify_strict(&canonical(&st)?, &Signature::from_bytes(&s))
            .is_ok(),
        _ => false,
    };
    if !v.signature_ok {
        v.problems.push("signature does not verify".into());
    }
    if !trust.is_empty() {
        v.trusted = Some(trust.contains(&pk));
        if v.trusted == Some(false) {
            v.problems.push(format!(
                "signed by key {}, which is not a trusted key",
                v.key_id
            ));
        }
    }
    let now = list_files(dir)?;
    for f in &st.files {
        match now.iter().find(|n| n.path == f.path) {
            None => v.problems.push(format!("{}: missing", f.path)),
            Some(n) if n.sha256 != f.sha256 => v.problems.push(format!("{}: changed", f.path)),
            _ => {}
        }
    }
    for n in &now {
        if !st.files.iter().any(|f| f.path == n.path) {
            v.problems
                .push(format!("{}: not covered by the signature", n.path));
        }
    }
    Ok(v)
}

/// Re-sign a format-1 pack as format 2 with `osnma` (default: "not recorded"). The pack must
/// first verify against `trust` (its old signer pinned): a pack whose files changed, or whose
/// signature does not hold, is never migrated. The new statement records the SHA-256 of the
/// old signature file in `supersedes`. The file set is unchanged. A format-2 pack is refused.
pub fn migrate(
    dir: &Path,
    trust: &[[u8; KEY_LEN]],
    seed: &[u8; KEY_LEN],
    signer: &str,
    utc: &str,
    osnma: Option<&OsnmaBlock>,
) -> Result<PathBuf, KshanaError> {
    let old = verify(dir, trust)?;
    if old.format != 1 {
        return Err(KshanaError::InvalidInput(format!(
            "{}: already format {}",
            dir.display(),
            old.format
        )));
    }
    if !old.passed() {
        return Err(KshanaError::InvalidInput(format!(
            "{}: the format-1 signature does not verify against a trusted key ({}); not migrated",
            dir.display(),
            if old.problems.is_empty() {
                "no --trust key given".to_string()
            } else {
                old.problems.join("; ")
            }
        )));
    }
    let path = dir.join(SIGNATURE_FILE);
    let bytes = std::fs::read(&path)
        .map_err(|e| KshanaError::InvalidInput(format!("{}: {e}", path.display())))?;
    let block = osnma.cloned().unwrap_or_else(OsnmaBlock::not_recorded);
    write_signature(
        dir,
        seed,
        signer,
        utc,
        Some(&block),
        Some(hex::encode(Sha256::digest(&bytes))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "kshana-pro-signed-dossier-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("runs")).unwrap();
        std::fs::write(d.join("dossier.json"), "{\"a\":1}\n").unwrap();
        std::fs::write(d.join("runs/r1.json"), "{}\n").unwrap();
        d
    }

    const SEED: [u8; KEY_LEN] = [5; KEY_LEN];

    #[test]
    fn sign_then_verify_and_every_tamper_is_caught() {
        let d = tmp("all");
        sign(&d, &SEED, "Ashforde OÜ", "2026-11-07T10:00:00Z").unwrap();
        let me = public_key_for(&SEED);
        let v = verify(&d, &[me]).unwrap();
        assert!(v.passed(), "{:?}", v.problems);
        assert_eq!(v.files, 2);
        // Not pinned: signature holds, but trust is unknown, so not passed.
        let v = verify(&d, &[]).unwrap();
        assert!(v.signature_ok && v.trusted.is_none() && !v.passed());
        // Another trusted key: refused.
        assert!(!verify(&d, &[public_key_for(&[6; KEY_LEN])])
            .unwrap()
            .passed());
        // A changed file, an added file, a removed file.
        std::fs::write(d.join("runs/r1.json"), "{\"x\":2}\n").unwrap();
        let v = verify(&d, &[me]).unwrap();
        assert!(
            v.problems.iter().any(|p| p == "runs/r1.json: changed"),
            "{:?}",
            v.problems
        );
        std::fs::write(d.join("runs/r1.json"), "{}\n").unwrap();
        std::fs::write(d.join("extra.txt"), "x").unwrap();
        assert!(!verify(&d, &[me]).unwrap().passed());
        std::fs::remove_file(d.join("extra.txt")).unwrap();
        std::fs::remove_file(d.join("dossier.json")).unwrap();
        assert!(!verify(&d, &[me]).unwrap().passed());
        std::fs::write(d.join("dossier.json"), "{\"a\":1}\n").unwrap();
        assert!(verify(&d, &[me]).unwrap().passed());
        // An edited statement (signer name) breaks the signature.
        let p = d.join(SIGNATURE_FILE);
        let t = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, t.replace("Ashforde OÜ", "Someone Else")).unwrap();
        let v = verify(&d, &[me]).unwrap();
        assert!(!v.signature_ok);
    }
}
