// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Saved studies and share links (roadmap F3a)
//!
//! * **Saved studies**: a tenant keeps named job requests (`{name, op, request}`) under
//!   `<data-dir>/library/<tenant>/<name>.json` and runs one again with one call. A
//!   saved study is the request, not its result: running it is an ordinary job (the
//!   same request gives the same job, so a study already run is not run twice).
//! * **Share links**: the owner of a succeeded job makes a read-only link,
//!   `/shared/<token>`, that serves its status, result and artefacts to anyone holding
//!   the token, without a credential, until it expires (1 to 90 days) or is revoked.
//!   Only the token's SHA-256 is stored (`<data-dir>/shares/<sha256>.json`); the token
//!   is shown once.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One saved study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedStudy {
    pub name: String,
    pub op: String,
    pub request: serde_json::Value,
    pub saved_utc: String,
    pub saved_by: String,
}

/// One share link, as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Share {
    pub job_id: String,
    pub tenant: String,
    pub created_unix: i64,
    pub expires_unix: i64,
    pub by: String,
    #[serde(default)]
    pub revoked: bool,
}

fn tenant_dir(data_dir: &Path, tenant: &str) -> PathBuf {
    // Tenant names are checked names (letters, digits, '.', '_', '-'), or '*'.
    let t = if tenant == super::auth::ALL_TENANTS {
        "_all"
    } else {
        tenant
    };
    data_dir.join("library").join(t)
}

/// Save (or replace) a study.
pub fn save(data_dir: &Path, tenant: &str, s: &SavedStudy) -> std::io::Result<bool> {
    let d = tenant_dir(data_dir, tenant);
    std::fs::create_dir_all(&d)?;
    let p = d.join(format!("{}.json", s.name));
    let existed = p.exists();
    let body = serde_json::to_string_pretty(s).map_err(std::io::Error::other)?;
    super::jobs::write_atomic(&p, body.as_bytes())?;
    Ok(existed)
}

/// One saved study of `tenant`.
pub fn get(data_dir: &Path, tenant: &str, name: &str) -> Option<SavedStudy> {
    std::fs::read_to_string(tenant_dir(data_dir, tenant).join(format!("{name}.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// Every saved study of `tenant`, by name.
pub fn list(data_dir: &Path, tenant: &str) -> Vec<SavedStudy> {
    let mut out: Vec<SavedStudy> = std::fs::read_dir(tenant_dir(data_dir, tenant))
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .filter_map(|t| serde_json::from_str(&t).ok())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn share_path(data_dir: &Path, token: &str) -> PathBuf {
    data_dir.join("shares").join(format!(
        "{}.json",
        hex::encode(Sha256::digest(token.as_bytes()))
    ))
}

/// Make a share link for `job_id`; returns the token (shown once).
pub fn share(data_dir: &Path, s: &Share) -> std::io::Result<String> {
    use rand::RngCore;
    let mut b = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut b);
    let token = format!("ksh_{}", hex::encode(b));
    let p = share_path(data_dir, &token);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let body = serde_json::to_string_pretty(s).map_err(std::io::Error::other)?;
    super::jobs::write_atomic(&p, body.as_bytes())?;
    Ok(token)
}

/// The live share for `token` at `now` (not expired, not revoked).
pub fn resolve(data_dir: &Path, token: &str, now: i64) -> Option<Share> {
    if !token.starts_with("ksh_")
        || token.len() != 52
        || !token[4..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return None;
    }
    let s: Share = std::fs::read_to_string(share_path(data_dir, token))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())?;
    (!s.revoked && now < s.expires_unix).then_some(s)
}

/// Revoke `token` when it belongs to `tenant`. Returns whether it did.
pub fn revoke(data_dir: &Path, token: &str, tenant: &str) -> std::io::Result<bool> {
    let p = share_path(data_dir, token);
    let Some(mut s) = std::fs::read_to_string(&p)
        .ok()
        .and_then(|t| serde_json::from_str::<Share>(&t).ok())
    else {
        return Ok(false);
    };
    if s.tenant != tenant && tenant != super::auth::ALL_TENANTS {
        return Ok(false);
    }
    s.revoked = true;
    let body = serde_json::to_string_pretty(&s).map_err(std::io::Error::other)?;
    super::jobs::write_atomic(&p, body.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_share_expires_and_a_malformed_token_resolves_to_nothing() {
        let d = std::env::temp_dir().join(format!("kshana-pro-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let s = Share {
            job_id: "a".repeat(64),
            tenant: "t".into(),
            created_unix: 100,
            expires_unix: 200,
            by: "k".into(),
            revoked: false,
        };
        let token = share(&d, &s).unwrap();
        assert_eq!(resolve(&d, &token, 199), Some(s));
        assert_eq!(resolve(&d, &token, 200), None);
        assert_eq!(resolve(&d, "ksh_../../etc", 150), None);
        assert!(!revoke(&d, &token, "other").unwrap());
        assert!(revoke(&d, &token, "t").unwrap());
        assert_eq!(resolve(&d, &token, 150), None);
    }
}
