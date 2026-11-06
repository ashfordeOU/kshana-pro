// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # OIDC bearer tokens, verified offline (roadmap A9)
//!
//! A JWT from the operator's identity provider is verified against a JWKS file the
//! operator places on disk (re-read when it changes); the service fetches nothing, so it
//! works air-gapped. Checked: compact JWS form, `alg` ES256 or RS256 only (`none` and the
//! HMAC algorithms are refused, as is a key whose `alg` differs), a `kid` in the set, the
//! signature, `iss`, `aud` (a string or an array), `exp` (required) and `nbf`, with a
//! leeway. The principal's id, tenant and role come from configured claims; a token
//! without them is refused, never given a default.
//!
//! RS256 is RSASSA-PKCS1-v1_5 with SHA-256 (RFC 8017, section 8.2.2), with a modulus of
//! at least 2048 bits; ES256 is ECDSA on P-256 with SHA-256, the signature as r||s
//! (RFC 7518, section 3.4). Both are checked against tokens signed by an independent
//! implementation (`tests/service_oidc.rs`).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::auth::{is_name, Principal, Role, Source, ALL_TENANTS};

/// `oidc.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcConfig {
    /// The `iss` every token must carry, exactly.
    pub issuer: String,
    /// The `aud` every token must carry (or contain).
    pub audience: String,
    /// The JWKS file, relative to the config file.
    pub jwks_file: String,
    /// The claim naming the principal (default `sub`).
    #[serde(default = "default_id_claim")]
    pub id_claim: String,
    /// The claim holding the role: `reader`, `submitter` or `admin`, or an array of
    /// strings in which the highest of those wins.
    pub role_claim: String,
    /// The claim holding the tenant.
    pub tenant_claim: String,
    /// Clock leeway for `exp` and `nbf`, seconds (default 60).
    #[serde(default = "default_leeway")]
    pub leeway_s: i64,
}

fn default_id_claim() -> String {
    "sub".into()
}

fn default_leeway() -> i64 {
    60
}

/// Why a token was refused (kept for tests and logs; the client sees only 401).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    Malformed,
    Algorithm,
    UnknownKey,
    Signature,
    Issuer,
    Audience,
    Expired,
    NotYetValid,
    Claims,
    Keys(String),
}

enum Key {
    Ec(p256::ecdsa::VerifyingKey),
    Rsa { n: Vec<u8>, e: Vec<u8> },
}

struct KeySet {
    stamp: Option<SystemTime>,
    keys: Vec<(String, Key)>,
}

/// Verifies tokens for one issuer.
pub struct Verifier {
    cfg: OidcConfig,
    jwks_path: PathBuf,
    set: Mutex<KeySet>,
}

/// URL-safe base64 without padding (RFC 4648, section 5), as JWS uses it.
pub fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32)
    };
    let b = s.as_bytes();
    if b.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() * 3 / 4);
    for chunk in b.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Some(out)
}

fn parse_jwks(text: &str) -> Result<Vec<(String, Key)>, TokenError> {
    let bad = |m: &str| TokenError::Keys(m.to_string());
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| bad(&e.to_string()))?;
    let mut out = Vec::new();
    for k in v["keys"].as_array().ok_or_else(|| bad("no keys array"))? {
        let s = |f: &str| k[f].as_str();
        let Some(kid) = s("kid") else { continue };
        if s("use").is_some_and(|u| u != "sig") {
            continue;
        }
        let field = |f: &str| {
            s(f).and_then(b64url_decode)
                .ok_or_else(|| bad(&format!("key {kid}: bad {f}")))
        };
        let key = match (s("kty"), s("alg")) {
            (Some("EC"), None | Some("ES256")) if s("crv") == Some("P-256") => {
                let (x, y) = (field("x")?, field("y")?);
                if x.len() != 32 || y.len() != 32 {
                    return Err(bad(&format!("key {kid}: P-256 coordinates are 32 bytes")));
                }
                let mut sec1 = vec![4u8];
                sec1.extend(x);
                sec1.extend(y);
                Key::Ec(
                    p256::ecdsa::VerifyingKey::from_sec1_bytes(&sec1)
                        .map_err(|_| bad(&format!("key {kid}: not a point on P-256")))?,
                )
            }
            (Some("RSA"), None | Some("RS256")) => {
                let n = field("n")?;
                let n: Vec<u8> = n.into_iter().skip_while(|&b| b == 0).collect();
                if n.len() < 256 {
                    return Err(bad(&format!(
                        "key {kid}: an RSA modulus below 2048 bits is refused"
                    )));
                }
                Key::Rsa { n, e: field("e")? }
            }
            _ => continue, // other key types and algorithms are not used
        };
        out.push((kid.to_string(), key));
    }
    Ok(out)
}

/// EMSA-PKCS1-v1_5 DigestInfo prefix for SHA-256 (RFC 8017, section 9.2, note 1).
const SHA256_DIGEST_INFO: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05,
    0x00, 0x04, 0x20,
];

/// RSASSA-PKCS1-v1_5 verification with SHA-256. Every value here is public, so the
/// variable-time arithmetic leaks nothing.
pub fn rs256_verify(n: &[u8], e: &[u8], msg: &[u8], sig: &[u8]) -> bool {
    use crypto_bigint::modular::{BoxedMontyForm, BoxedMontyParams};
    use crypto_bigint::{BoxedUint, Odd, Resize};
    let k = n.len();
    if sig.len() != k || e.is_empty() || e.len() > 8 {
        return false;
    }
    let bits = (k * 8) as u32;
    let modulus = BoxedUint::from_be_slice_vartime(n).resize(bits);
    let Some(odd) = Option::<Odd<BoxedUint>>::from(Odd::new(modulus.clone())) else {
        return false;
    };
    let s = BoxedUint::from_be_slice_vartime(sig).resize(bits);
    if s >= modulus {
        return false;
    }
    let params = BoxedMontyParams::new_vartime(odd);
    let exp = BoxedUint::from_be_slice_vartime(e);
    let m = BoxedMontyForm::new(s, &params).pow(&exp).retrieve();
    let em = m.to_be_bytes();
    let em = &em[em.len() - k..];
    let h = Sha256::digest(msg);
    let t_len = SHA256_DIGEST_INFO.len() + h.len();
    if k < t_len + 11 {
        return false;
    }
    let mut want = vec![0x00, 0x01];
    want.extend(std::iter::repeat_n(0xFF, k - t_len - 3));
    want.push(0x00);
    want.extend(SHA256_DIGEST_INFO);
    want.extend(h);
    em == want.as_slice()
}

fn es256_verify(key: &p256::ecdsa::VerifyingKey, msg: &[u8], sig: &[u8]) -> bool {
    use p256::ecdsa::signature::Verifier as _;
    let Ok(sig) = p256::ecdsa::Signature::from_slice(sig) else {
        return false;
    };
    key.verify(msg, &sig).is_ok()
}

impl Verifier {
    /// Read `oidc.toml` at `path` and the JWKS file it names.
    pub fn from_file(path: &std::path::Path) -> Result<Verifier, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let cfg: OidcConfig =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let jwks_path = path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join(&cfg.jwks_file);
        let v = Verifier::new(cfg, jwks_path);
        v.refresh()
            .map_err(|e| format!("{}: {e:?}", v.jwks_path.display()))?;
        Ok(v)
    }

    /// A verifier for `cfg`, reading keys from `jwks_path`.
    pub fn new(cfg: OidcConfig, jwks_path: PathBuf) -> Verifier {
        Verifier {
            cfg,
            jwks_path,
            set: Mutex::new(KeySet {
                stamp: None,
                keys: Vec::new(),
            }),
        }
    }

    /// Re-read the JWKS file when its modification time changed. A file that has become
    /// unreadable or invalid keeps the last good set.
    fn refresh(&self) -> Result<(), TokenError> {
        let stamp = std::fs::metadata(&self.jwks_path)
            .and_then(|m| m.modified())
            .ok();
        let mut set = self.set.lock().unwrap_or_else(|p| p.into_inner());
        if stamp.is_some() && stamp == set.stamp {
            return Ok(());
        }
        let text = std::fs::read_to_string(&self.jwks_path)
            .map_err(|e| TokenError::Keys(e.to_string()))?;
        let keys = parse_jwks(&text)?;
        *set = KeySet { stamp, keys };
        Ok(())
    }

    /// Verify `token` at server time `now` and return its principal.
    pub fn principal(&self, token: &str, now: i64) -> Result<Principal, TokenError> {
        let mut parts = token.split('.');
        let (Some(h64), Some(p64), Some(s64), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(TokenError::Malformed);
        };
        let header: serde_json::Value = b64url_decode(h64)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or(TokenError::Malformed)?;
        let alg = header["alg"].as_str().ok_or(TokenError::Algorithm)?;
        if alg != "ES256" && alg != "RS256" {
            return Err(TokenError::Algorithm);
        }
        if header.get("crit").is_some() {
            return Err(TokenError::Malformed);
        }
        let kid = header["kid"].as_str().ok_or(TokenError::UnknownKey)?;
        let sig = b64url_decode(s64).ok_or(TokenError::Malformed)?;
        let signing_input = &token.as_bytes()[..h64.len() + 1 + p64.len()];
        let _ = self.refresh();
        {
            let set = self.set.lock().unwrap_or_else(|p| p.into_inner());
            let key = set
                .keys
                .iter()
                .find(|(k, _)| k == kid)
                .map(|(_, k)| k)
                .ok_or(TokenError::UnknownKey)?;
            let ok = match (alg, key) {
                ("ES256", Key::Ec(k)) => es256_verify(k, signing_input, &sig),
                ("RS256", Key::Rsa { n, e }) => rs256_verify(n, e, signing_input, &sig),
                _ => return Err(TokenError::Algorithm),
            };
            if !ok {
                return Err(TokenError::Signature);
            }
        }
        let claims: serde_json::Value = b64url_decode(p64)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or(TokenError::Malformed)?;
        if claims["iss"].as_str() != Some(self.cfg.issuer.as_str()) {
            return Err(TokenError::Issuer);
        }
        let aud_ok = match &claims["aud"] {
            serde_json::Value::String(a) => *a == self.cfg.audience,
            serde_json::Value::Array(a) => a
                .iter()
                .any(|x| x.as_str() == Some(self.cfg.audience.as_str())),
            _ => false,
        };
        if !aud_ok {
            return Err(TokenError::Audience);
        }
        let leeway = self.cfg.leeway_s.max(0);
        let exp = claims["exp"].as_i64().ok_or(TokenError::Expired)?;
        if now > exp + leeway {
            return Err(TokenError::Expired);
        }
        if claims["nbf"].as_i64().is_some_and(|nbf| now + leeway < nbf) {
            return Err(TokenError::NotYetValid);
        }
        let id = claims[self.cfg.id_claim.as_str()]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256);
        let tenant = claims[self.cfg.tenant_claim.as_str()]
            .as_str()
            .filter(|t| is_name(t) && *t != ALL_TENANTS);
        let role_of = |s: &str| match s {
            "reader" => Some(Role::Reader),
            "submitter" => Some(Role::Submitter),
            "admin" => Some(Role::Admin),
            _ => None,
        };
        let role = match &claims[self.cfg.role_claim.as_str()] {
            serde_json::Value::String(s) => role_of(s),
            serde_json::Value::Array(a) => {
                a.iter().filter_map(|x| x.as_str().and_then(role_of)).max()
            }
            _ => None,
        };
        let (Some(id), Some(tenant), Some(role)) = (id, tenant, role) else {
            return Err(TokenError::Claims);
        };
        Ok(Principal {
            id: id.to_string(),
            tenant: tenant.to_string(),
            role,
            source: Source::Oidc,
        })
    }
}
