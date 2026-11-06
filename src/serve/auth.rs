// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Principals, roles and API keys (roadmap A9)
//!
//! Every request is made by a [`Principal`]: an id, a tenant and a [`Role`]. A principal
//! comes from a named API key (`--keys-file`), from the single legacy token
//! (`--api-token-file`, an `admin` of tenant `default` with id `token`), or, when the
//! server has neither, is the open server's `anonymous` admin of tenant `default`, as
//! before keys existed.
//!
//! A keys file holds only the SHA-256 of each key. Comparison hashes the presented key
//! and compares it with every entry in constant time, so neither the result nor the
//! time taken says which entry came closest. Design: `docs/design/A9-INTEGRATION-SECURITY.md`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// What a principal may do. Ordered: each role may do everything the ones below it may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Read routes (GET).
    Reader,
    /// Also submit runs, studies and jobs.
    Submitter,
    /// Also the ledger and the audit log.
    Admin,
}

impl Role {
    /// The lower-case name.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Submitter => "submitter",
            Role::Admin => "admin",
        }
    }
}

/// Where a principal came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The open server (no keys configured).
    Open,
    /// An API key or the legacy token.
    Key,
    /// An OIDC bearer token.
    Oidc,
}

impl Source {
    /// The name written to the audit log.
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Open => "open",
            Source::Key => "key",
            Source::Oidc => "oidc",
        }
    }
}

/// The tenant every job belonged to before tenants existed, and the open server's.
pub const DEFAULT_TENANT: &str = "default";
/// The tenant of an operator key that sees every tenant's jobs.
pub const ALL_TENANTS: &str = "*";

/// Who made a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub id: String,
    pub tenant: String,
    pub role: Role,
    pub source: Source,
}

impl Principal {
    /// The open server's caller.
    pub fn open() -> Principal {
        Principal {
            id: "anonymous".into(),
            tenant: DEFAULT_TENANT.into(),
            role: Role::Admin,
            source: Source::Open,
        }
    }

    /// True when this principal may see a job of `tenant`.
    pub fn sees(&self, tenant: &str) -> bool {
        self.tenant == tenant || (self.tenant == ALL_TENANTS && self.role == Role::Admin)
    }
}

/// True when `s` is a usable id or tenant name: 1 to 64 characters from
/// `A-Z a-z 0-9 . _ -` (and `*` alone, for a tenant).
pub fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// One entry of a keys file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyEntry {
    pub id: String,
    pub tenant: String,
    pub role: Role,
    /// SHA-256 of the key, lower-case hexadecimal.
    pub sha256: String,
    /// A revoked key is refused like an unknown one.
    #[serde(default)]
    pub revoked: bool,
}

/// A keys file: `[[key]]` tables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeysFile {
    #[serde(default, rename = "key")]
    pub keys: Vec<KeyEntry>,
}

impl KeysFile {
    /// Parse and check a keys file: well-formed names and hashes, unique ids.
    pub fn parse(text: &str) -> Result<KeysFile, String> {
        let f: KeysFile = toml::from_str(text).map_err(|e| format!("keys file: {e}"))?;
        let mut seen = std::collections::BTreeSet::new();
        for k in &f.keys {
            if !is_name(&k.id) || k.id == "token" || k.id == "anonymous" {
                return Err(format!(
                    "keys file: '{}' is not a usable key id (1-64 of A-Z a-z 0-9 . _ -; not 'token' or 'anonymous')",
                    k.id
                ));
            }
            if !(is_name(&k.tenant) || k.tenant == ALL_TENANTS) {
                return Err(format!(
                    "keys file: key '{}': bad tenant '{}'",
                    k.id, k.tenant
                ));
            }
            if k.tenant == ALL_TENANTS && k.role != Role::Admin {
                return Err(format!(
                    "keys file: key '{}': only an admin key may have tenant '*'",
                    k.id
                ));
            }
            if k.sha256.len() != 64
                || hex::decode(&k.sha256).is_err()
                || k.sha256 != k.sha256.to_lowercase()
            {
                return Err(format!(
                    "keys file: key '{}': sha256 must be 64 lower-case hexadecimal characters",
                    k.id
                ));
            }
            if !seen.insert(k.id.clone()) {
                return Err(format!("keys file: key id '{}' appears twice", k.id));
            }
        }
        Ok(f)
    }

    /// The file as TOML.
    pub fn to_toml(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }
}

/// A new random key (shown once) and the entry that stores its hash.
pub fn new_key(id: &str, tenant: &str, role: Role) -> (String, KeyEntry) {
    let bytes: [u8; 32] = rand::random();
    let key = format!("kpk_{}", hex::encode(bytes));
    let entry = KeyEntry {
        id: id.to_string(),
        tenant: tenant.to_string(),
        role,
        sha256: hex::encode(Sha256::digest(key.as_bytes())),
        revoked: false,
    };
    (key, entry)
}

struct Stored {
    digest: [u8; 32],
    principal: Principal,
    revoked: bool,
}

/// Why a request has no principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No `Authorization: Bearer` header.
    Missing,
    /// A bearer credential that matches nothing (or a revoked key, or a bad token).
    Invalid,
}

/// Turns an `Authorization` header into a principal.
#[derive(Default)]
pub struct Authenticator {
    keys: Vec<Stored>,
    oidc: Option<super::oidc::Verifier>,
}

fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// The credential of an `Authorization: Bearer <credential>` header.
pub fn bearer(authorization: Option<&str>) -> Option<&str> {
    let (scheme, rest) = authorization?.trim().split_once(' ')?;
    let c = rest.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !c.is_empty()).then_some(c)
}

impl Authenticator {
    /// From the legacy token and a keys file (either may be absent).
    pub fn new(api_token: Option<&str>, keys: Option<&KeysFile>) -> Authenticator {
        let mut a = Authenticator::default();
        if let Some(t) = api_token {
            a.keys.push(Stored {
                digest: Sha256::digest(t.as_bytes()).into(),
                principal: Principal {
                    id: "token".into(),
                    tenant: DEFAULT_TENANT.into(),
                    role: Role::Admin,
                    source: Source::Key,
                },
                revoked: false,
            });
        }
        for k in keys.map(|f| f.keys.as_slice()).unwrap_or_default() {
            let mut digest = [0u8; 32];
            if hex::decode_to_slice(&k.sha256, &mut digest).is_err() {
                continue; // KeysFile::parse refuses these
            }
            a.keys.push(Stored {
                digest,
                principal: Principal {
                    id: k.id.clone(),
                    tenant: k.tenant.clone(),
                    role: k.role,
                    source: Source::Key,
                },
                revoked: k.revoked,
            });
        }
        a
    }

    /// Also accept OIDC bearer tokens.
    pub fn with_oidc(mut self, v: super::oidc::Verifier) -> Authenticator {
        self.oidc = Some(v);
        self
    }

    /// True when requests must carry a credential.
    pub fn required(&self) -> bool {
        !self.keys.is_empty() || self.oidc.is_some()
    }

    /// The principal of a request, at server time `now` (Unix seconds).
    pub fn authenticate(
        &self,
        authorization: Option<&str>,
        now: i64,
    ) -> Result<Principal, Refusal> {
        if !self.required() {
            return Ok(Principal::open());
        }
        let cred = bearer(authorization).ok_or(Refusal::Missing)?;
        // A JWT has two dots; an API key has none. Each kind is checked only by its own path.
        if cred.matches('.').count() == 2 {
            return match &self.oidc {
                Some(v) => v.principal(cred, now).map_err(|_| Refusal::Invalid),
                None => Err(Refusal::Invalid),
            };
        }
        let got: [u8; 32] = Sha256::digest(cred.as_bytes()).into();
        let mut found: Option<&Stored> = None;
        for s in &self.keys {
            if ct_eq(&got, &s.digest) {
                found = Some(s);
            }
        }
        match found {
            Some(s) if !s.revoked => Ok(s.principal.clone()),
            _ => Err(Refusal::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_roles_revocation_and_the_legacy_token() {
        let (k1, e1) = new_key("ingest-a", "site-a", Role::Submitter);
        let (k2, mut e2) = new_key("old", "site-a", Role::Reader);
        e2.revoked = true;
        let (k3, e3) = new_key("ops", ALL_TENANTS, Role::Admin);
        let f = KeysFile {
            keys: vec![e1, e2, e3],
        };
        let f = KeysFile::parse(&f.to_toml()).unwrap();
        let a = Authenticator::new(Some("legacy"), Some(&f));
        let p = a.authenticate(Some(&format!("Bearer {k1}")), 0).unwrap();
        assert_eq!(
            (p.id.as_str(), p.tenant.as_str(), p.role),
            ("ingest-a", "site-a", Role::Submitter)
        );
        assert!(!p.sees("site-b"));
        assert_eq!(
            a.authenticate(Some(&format!("Bearer {k2}")), 0),
            Err(Refusal::Invalid)
        );
        assert!(a
            .authenticate(Some(&format!("bearer {k3}")), 0)
            .unwrap()
            .sees("anyone"));
        let t = a.authenticate(Some("Bearer legacy"), 0).unwrap();
        assert_eq!((t.id.as_str(), t.role), ("token", Role::Admin));
        assert_eq!(a.authenticate(None, 0), Err(Refusal::Missing));
        assert_eq!(a.authenticate(Some(&k1), 0), Err(Refusal::Missing));
        assert_eq!(
            a.authenticate(Some("Bearer kpk_00"), 0),
            Err(Refusal::Invalid)
        );
        assert_eq!(
            a.authenticate(Some("Bearer a.b.c"), 0),
            Err(Refusal::Invalid)
        );
        let open = Authenticator::new(None, None);
        assert_eq!(open.authenticate(None, 0), Ok(Principal::open()));
    }

    #[test]
    fn bad_keys_files_are_refused() {
        let h = "a".repeat(64);
        for bad in [
            format!("[[key]]\nid = \"token\"\ntenant = \"t\"\nrole = \"reader\"\nsha256 = \"{h}\"\n"),
            format!("[[key]]\nid = \"x\"\ntenant = \"*\"\nrole = \"reader\"\nsha256 = \"{h}\"\n"),
            "[[key]]\nid = \"x\"\ntenant = \"t\"\nrole = \"reader\"\nsha256 = \"AB\"\n".to_string(),
            format!("[[key]]\nid = \"x\"\ntenant = \"t\"\nrole = \"root\"\nsha256 = \"{h}\"\n"),
            format!("[[key]]\nid = \"x\"\ntenant = \"t\"\nrole = \"reader\"\nsha256 = \"{h}\"\n[[key]]\nid = \"x\"\ntenant = \"u\"\nrole = \"reader\"\nsha256 = \"{h}\"\n"),
        ] {
            assert!(KeysFile::parse(&bad).is_err(), "{bad}");
        }
    }
}
