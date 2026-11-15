// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # AWS KMS licence signer (key custody S7-a; cargo feature `kms`)
//!
//! The day-to-day issuing key lives in AWS KMS as an `ECC_NIST_EDWARDS25519` key with key
//! usage `SIGN_VERIFY`; its private half never leaves KMS. [`KmsSigner`] calls two actions
//! of the KMS JSON API:
//!
//! * `GetPublicKey` once, to learn the key (DER SubjectPublicKeyInfo; the raw Ed25519 key is
//!   its last 32 bytes, after the fixed 12-byte prefix `302a300506032b6570032100`), and
//! * `Sign` with `SigningAlgorithm: ED25519_SHA_512` and `MessageType: RAW` for every
//!   signature: pure Ed25519, the same 64 bytes a seed would give, so tokens signed through
//!   KMS verify with the unchanged `verify` code.
//!
//! Requests are signed with AWS Signature Version 4, implemented here with `ring`'s
//! HMAC-SHA256 and `sha2` (no AWS SDK, no new crate). Transport is HTTP/1.1 over `rustls`
//! (TLS 1.2/1.3, roots from `SSL_CERT_FILE`, `KSHANA_PRO_KMS_CA_FILE` or the usual system
//! bundle paths); plain `http://` is accepted only for a loopback endpoint (the tests' fake
//! KMS). KMS takes at most 4096 bytes for a RAW message; a larger document is refused with a
//! clear error (licence tokens and check answers are a few hundred bytes; a revocation list
//! that grows past that should be split by sequence).
//!
//! Credentials are read from the environment at signing time (`AWS_ACCESS_KEY_ID`,
//! `AWS_SECRET_ACCESS_KEY`, optional `AWS_SESSION_TOKEN`), so a role's rotating credentials
//! exported by the host are picked up; nothing is cached on disk or printed.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use kshana::api::KshanaError;
use sha2::{Digest, Sha256};

use super::signer::LicenceSigner;
use super::KEY_LEN;

/// The KMS limit for a `RAW` message, in bytes.
pub const MAX_RAW_MESSAGE: usize = 4096;

/// The DER prefix of an Ed25519 SubjectPublicKeyInfo.
const ED25519_SPKI_PREFIX: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

fn bad(m: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(m.into())
}

/// AWS credentials (never printed: `Debug` hides the secret).
#[derive(Clone)]
/// The region used when none is configured: Frankfurt (owner decision, 15 Nov 2026). The KMS
/// signer stays off until `KSHANA_PRO_SIGNER=kms` is set.
pub const DEFAULT_REGION: &str = "eu-central-1";

pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"(hidden)")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "(hidden)"),
            )
            .finish()
    }
}

impl Credentials {
    /// From `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and `AWS_SESSION_TOKEN`.
    pub fn from_env() -> Result<Credentials, KshanaError> {
        let id = std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_default();
        let secret = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default();
        if id.trim().is_empty() || secret.trim().is_empty() {
            return Err(bad(
                "the KMS signer needs AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY in the environment",
            ));
        }
        Ok(Credentials {
            access_key_id: id.trim().into(),
            secret_access_key: secret.trim().into(),
            session_token: std::env::var("AWS_SESSION_TOKEN")
                .ok()
                .filter(|s| !s.trim().is_empty()),
        })
    }
}

/// Where credentials come from.
#[derive(Clone)]
pub enum CredentialSource {
    /// Read the environment at every call (rotating role credentials).
    Environment,
    /// Fixed credentials (tests).
    Fixed(Credentials),
}

impl CredentialSource {
    fn get(&self) -> Result<Credentials, KshanaError> {
        match self {
            CredentialSource::Environment => Credentials::from_env(),
            CredentialSource::Fixed(c) => Ok(c.clone()),
        }
    }
}

/// The AWS KMS back end of [`LicenceSigner`].
pub struct KmsSigner {
    /// Key ID, key ARN or `alias/...`.
    pub key_id: String,
    pub region: String,
    /// `https://kms.<region>.amazonaws.com` unless overridden.
    pub endpoint: String,
    credentials: CredentialSource,
    public_key: OnceLock<[u8; KEY_LEN]>,
    /// The time source for request signing (tests pin it).
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl KmsSigner {
    /// A signer for `key_id` in `region`.
    pub fn new(
        key_id: &str,
        region: &str,
        endpoint: Option<&str>,
        credentials: CredentialSource,
    ) -> Result<KmsSigner, KshanaError> {
        let key_id = key_id.trim();
        let region = region.trim();
        if key_id.is_empty() {
            return Err(bad("the KMS signer needs a key ID (KSHANA_PRO_KMS_KEY_ID)"));
        }
        if region.is_empty()
            || !region
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(bad(
                "the KMS signer needs a region such as eu-central-1 (KSHANA_PRO_KMS_REGION)",
            ));
        }
        let endpoint = endpoint
            .map(|e| e.trim().trim_end_matches('/').to_string())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| format!("https://kms.{region}.amazonaws.com"));
        Url::parse(&endpoint)?;
        Ok(KmsSigner {
            key_id: key_id.into(),
            region: region.into(),
            endpoint,
            credentials,
            public_key: OnceLock::new(),
            clock: Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            }),
        })
    }

    /// From `KSHANA_PRO_KMS_KEY_ID`, `KSHANA_PRO_KMS_REGION` (or `AWS_REGION`,
    /// `AWS_DEFAULT_REGION`, else [`DEFAULT_REGION`]) and `KSHANA_PRO_KMS_ENDPOINT`;
    /// credentials from the environment.
    pub fn from_env() -> Result<KmsSigner, KshanaError> {
        let key = std::env::var("KSHANA_PRO_KMS_KEY_ID").unwrap_or_default();
        let region = ["KSHANA_PRO_KMS_REGION", "AWS_REGION", "AWS_DEFAULT_REGION"]
            .iter()
            .find_map(|v| std::env::var(v).ok().filter(|s| !s.trim().is_empty()))
            .unwrap_or_else(|| DEFAULT_REGION.to_string());
        let endpoint = std::env::var("KSHANA_PRO_KMS_ENDPOINT").ok();
        KmsSigner::new(
            &key,
            &region,
            endpoint.as_deref(),
            CredentialSource::Environment,
        )
    }

    /// Pin the clock used to date requests (tests).
    pub fn with_clock(mut self, clock: Arc<dyn Fn() -> i64 + Send + Sync>) -> KmsSigner {
        self.clock = clock;
        self
    }

    fn call(
        &self,
        target: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, KshanaError> {
        let url = Url::parse(&self.endpoint)?;
        let body = serde_json::to_vec(body).map_err(|e| bad(e.to_string()))?;
        let creds = self.credentials.get()?;
        let amz_date = amz_date((self.clock)());
        let host = url.host_header();
        let mut headers: Vec<(String, String)> = vec![
            ("content-type".into(), "application/x-amz-json-1.1".into()),
            ("host".into(), host.clone()),
            ("x-amz-date".into(), amz_date.clone()),
            ("x-amz-target".into(), format!("TrentService.{target}")),
        ];
        if let Some(t) = &creds.session_token {
            headers.push(("x-amz-security-token".into(), t.clone()));
        }
        let auth = sigv4_authorization(&SigV4Request {
            method: "POST",
            path: &url.path,
            query: "",
            headers: &headers,
            body: &body,
            access_key_id: &creds.access_key_id,
            secret_access_key: &creds.secret_access_key,
            region: &self.region,
            service: "kms",
            amz_date: &amz_date,
        });
        headers.push(("authorization".into(), auth));
        let (status, text) = http_post(&url, &headers, &body)?;
        let v: serde_json::Value = serde_json::from_slice(&text).map_err(|_| {
            bad(format!(
                "KMS {target}: HTTP {status}, answer is not JSON ({} bytes)",
                text.len()
            ))
        })?;
        if status != 200 {
            let kind = v["__type"].as_str().unwrap_or("error");
            let msg = v["message"]
                .as_str()
                .or_else(|| v["Message"].as_str())
                .unwrap_or("");
            return Err(bad(format!("KMS {target}: HTTP {status} {kind} {msg}")));
        }
        Ok(v)
    }
}

impl LicenceSigner for KmsSigner {
    fn public_key(&self) -> Result<[u8; KEY_LEN], KshanaError> {
        if let Some(k) = self.public_key.get() {
            return Ok(*k);
        }
        let v = self.call("GetPublicKey", &serde_json::json!({ "KeyId": self.key_id }))?;
        if let Some(spec) = v["KeySpec"].as_str() {
            if spec != "ECC_NIST_EDWARDS25519" {
                return Err(bad(format!(
                    "KMS key {} has key spec {spec}; licence signing needs ECC_NIST_EDWARDS25519",
                    self.key_id
                )));
            }
        }
        if let Some(usage) = v["KeyUsage"].as_str() {
            if usage != "SIGN_VERIFY" {
                return Err(bad(format!(
                    "KMS key {} has key usage {usage}; licence signing needs SIGN_VERIFY",
                    self.key_id
                )));
            }
        }
        let der = base64_decode(v["PublicKey"].as_str().unwrap_or(""))
            .ok_or_else(|| bad("KMS GetPublicKey: PublicKey is not base64"))?;
        if der.len() != ED25519_SPKI_PREFIX.len() + KEY_LEN || der[..12] != ED25519_SPKI_PREFIX {
            return Err(bad(
                "KMS GetPublicKey: not an Ed25519 SubjectPublicKeyInfo (44 bytes)",
            ));
        }
        let mut k = [0u8; KEY_LEN];
        k.copy_from_slice(&der[12..]);
        Ok(*self.public_key.get_or_init(|| k))
    }

    fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 64], KshanaError> {
        if msg.len() > MAX_RAW_MESSAGE {
            return Err(bad(format!(
                "KMS signs RAW messages of at most {MAX_RAW_MESSAGE} bytes; this document has {}",
                msg.len()
            )));
        }
        let v = self.call(
            "Sign",
            &serde_json::json!({
                "KeyId": self.key_id,
                "Message": base64_encode(msg),
                "MessageType": "RAW",
                "SigningAlgorithm": "ED25519_SHA_512",
            }),
        )?;
        if let Some(alg) = v["SigningAlgorithm"].as_str() {
            if alg != "ED25519_SHA_512" {
                return Err(bad(format!("KMS Sign answered with algorithm {alg}")));
            }
        }
        let sig = base64_decode(v["Signature"].as_str().unwrap_or(""))
            .ok_or_else(|| bad("KMS Sign: Signature is not base64"))?;
        sig.try_into()
            .map_err(|s: Vec<u8>| bad(format!("KMS Sign: signature is {} bytes, not 64", s.len())))
    }

    fn describe(&self) -> String {
        format!("aws-kms {} ({})", self.key_id, self.region)
    }
}

// ---------------------------------------------------------------------------------
// AWS Signature Version 4
// ---------------------------------------------------------------------------------

/// One request to sign with SigV4.
pub struct SigV4Request<'a> {
    pub method: &'a str,
    /// The URI path, already in canonical (encoded) form.
    pub path: &'a str,
    /// The canonical query string (`""` for none).
    pub query: &'a str,
    /// Every header that is signed (names in any case; all are signed).
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
    pub access_key_id: &'a str,
    pub secret_access_key: &'a str,
    pub region: &'a str,
    pub service: &'a str,
    /// `YYYYMMDDTHHMMSSZ`.
    pub amz_date: &'a str,
}

fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    ring::hmac::sign(&k, msg).as_ref().to_vec()
}

/// The SigV4 signing key for a date (`YYYYMMDD`), region and service.
pub fn sigv4_signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k = hmac(&k, region.as_bytes());
    let k = hmac(&k, service.as_bytes());
    hmac(&k, b"aws4_request")
}

/// The canonical request, the string to sign and the signature (hex) of `r`.
pub fn sigv4_parts(r: &SigV4Request<'_>) -> (String, String, String, String) {
    let mut hs: Vec<(String, String)> = r
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.trim().to_ascii_lowercase(),
                v.split_whitespace().collect::<Vec<_>>().join(" "),
            )
        })
        .collect();
    hs.sort();
    let canonical_headers: String = hs.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed: String = hs
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        r.method,
        r.path,
        r.query,
        canonical_headers,
        signed,
        hex::encode(Sha256::digest(r.body))
    );
    let date = &r.amz_date[..8.min(r.amz_date.len())];
    let scope = format!("{date}/{}/{}/aws4_request", r.region, r.service);
    let sts = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        r.amz_date,
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );
    let key = sigv4_signing_key(r.secret_access_key, date, r.region, r.service);
    let sig = hex::encode(hmac(&key, sts.as_bytes()));
    (canonical, sts, signed, sig)
}

/// The `Authorization` header value for `r`.
pub fn sigv4_authorization(r: &SigV4Request<'_>) -> String {
    let (_, _, signed, sig) = sigv4_parts(r);
    let date = &r.amz_date[..8.min(r.amz_date.len())];
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{date}/{}/{}/aws4_request, SignedHeaders={signed}, Signature={sig}",
        r.access_key_id, r.region, r.service
    )
}

/// `YYYYMMDDTHHMMSSZ` for Unix seconds (UTC).
pub fn amz_date(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

// ---------------------------------------------------------------------------------
// Base64 (standard alphabet, padded), for the KMS JSON fields.
// ---------------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard padded base64.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Decode standard base64 (padding optional, whitespace ignored); `None` if invalid.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    let trimmed: &[u8] = {
        let mut end = clean.len();
        while end > 0 && clean[end - 1] == b'=' {
            end -= 1;
        }
        &clean[..end]
    };
    if clean.len() - trimmed.len() > 2 || trimmed.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in trimmed {
        let v = B64.iter().position(|&x| x == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------------
// A small HTTP/1.1 client: one POST per connection.
// ---------------------------------------------------------------------------------

struct Url {
    tls: bool,
    host: String,
    port: u16,
    path: String,
}

impl Url {
    fn parse(s: &str) -> Result<Url, KshanaError> {
        let (tls, rest) = if let Some(r) = s.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = s.strip_prefix("http://") {
            (false, r)
        } else {
            return Err(bad(format!(
                "KMS endpoint must be an https:// URL, got '{s}'"
            )));
        };
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) if !h.ends_with(']') || h.starts_with('[') => (
                h.to_string(),
                p.parse::<u16>()
                    .map_err(|_| bad(format!("bad port in KMS endpoint '{s}'")))?,
            ),
            _ => (hostport.to_string(), if tls { 443 } else { 80 }),
        };
        if host.is_empty() {
            return Err(bad(format!("no host in KMS endpoint '{s}'")));
        }
        if !tls && !matches!(host.as_str(), "127.0.0.1" | "localhost" | "[::1]") {
            return Err(bad(
                "a plain http:// KMS endpoint is accepted only on the loopback address (tests)",
            ));
        }
        Ok(Url {
            tls,
            host,
            port,
            path: path.to_string(),
        })
    }

    fn host_header(&self) -> String {
        let default = if self.tls { 443 } else { 80 };
        if self.port == default {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn http_post(
    url: &Url,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<(u16, Vec<u8>), KshanaError> {
    let io = |e: std::io::Error| bad(format!("KMS request to {}: {e}", url.host));
    let host = url.host.trim_start_matches('[').trim_end_matches(']');
    let tcp = TcpStream::connect((host, url.port)).map_err(io)?;
    tcp.set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(io)?;
    tcp.set_write_timeout(Some(Duration::from_secs(30)))
        .map_err(io)?;
    let mut req = format!("POST {} HTTP/1.1\r\n", url.path);
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str(&format!(
        "content-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    ));
    let mut raw = Vec::new();
    if url.tls {
        let cfg = tls_client_config()?;
        let name = rustls::pki_types::ServerName::try_from(url.host.clone())
            .map_err(|_| bad(format!("bad KMS host name '{}'", url.host)))?;
        let conn = rustls::ClientConnection::new(cfg, name).map_err(|e| bad(e.to_string()))?;
        let mut s = rustls::StreamOwned::new(conn, tcp);
        s.write_all(req.as_bytes()).map_err(io)?;
        s.write_all(body).map_err(io)?;
        s.flush().map_err(io)?;
        read_all_tolerant(&mut s, &mut raw).map_err(io)?;
    } else {
        let mut s = tcp;
        s.write_all(req.as_bytes()).map_err(io)?;
        s.write_all(body).map_err(io)?;
        s.read_to_end(&mut raw).map_err(io)?;
    }
    parse_response(&raw)
}

/// Read to the end; a TLS peer that closes without close_notify after a complete answer is
/// tolerated (the body length is checked by the parser).
fn read_all_tolerant(s: &mut impl Read, out: &mut Vec<u8>) -> std::io::Result<()> {
    match s.read_to_end(out) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && !out.is_empty() => Ok(()),
        Err(e) => Err(e),
    }
}

fn parse_response(raw: &[u8]) -> Result<(u16, Vec<u8>), KshanaError> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| bad("KMS answer has no HTTP header"))?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut body = raw[split + 4..].to_vec();
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| bad("KMS answer has no status line"))?;
    let header = |name: &str| {
        head.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case(name)
                .then(|| v.trim().to_string())
        })
    };
    if header("transfer-encoding").is_some_and(|v| v.eq_ignore_ascii_case("chunked")) {
        body = dechunk(&body).ok_or_else(|| bad("KMS answer: bad chunked body"))?;
    } else if let Some(n) = header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        if body.len() < n {
            return Err(bad("KMS answer was cut short"));
        }
        body.truncate(n);
    }
    Ok((status, body))
}

fn dechunk(mut b: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let eol = b.windows(2).position(|w| w == b"\r\n")?;
        let size_txt = std::str::from_utf8(&b[..eol]).ok()?;
        let size = usize::from_str_radix(size_txt.split(';').next()?.trim(), 16).ok()?;
        b = &b[eol + 2..];
        if size == 0 {
            return Some(out);
        }
        if b.len() < size + 2 {
            return None;
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..];
    }
}

fn tls_client_config() -> Result<Arc<rustls::ClientConfig>, KshanaError> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::CertificateDer;
    static CFG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    if let Some(c) = CFG.get() {
        return Ok(c.clone());
    }
    let candidates: Vec<String> = ["KSHANA_PRO_KMS_CA_FILE", "SSL_CERT_FILE"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .chain(
            [
                "/etc/ssl/certs/ca-certificates.crt",
                "/etc/pki/tls/certs/ca-bundle.crt",
                "/etc/ssl/cert.pem",
                "/usr/local/etc/openssl/cert.pem",
            ]
            .iter()
            .map(|s| s.to_string()),
        )
        .collect();
    let mut roots = rustls::RootCertStore::empty();
    for path in &candidates {
        if let Ok(iter) = CertificateDer::pem_file_iter(path) {
            for c in iter.flatten() {
                let _ = roots.add(c);
            }
            if !roots.is_empty() {
                break;
            }
        }
    }
    if roots.is_empty() {
        return Err(bad(
            "no CA certificates found for the KMS connection; set KSHANA_PRO_KMS_CA_FILE",
        ));
    }
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| bad(e.to_string()))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(CFG.get_or_init(|| Arc::new(cfg)).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigv4_matches_the_aws_published_vectors() {
        // AWS "Examples of how to derive a signing key for Signature Version 4".
        assert_eq!(
            hex::encode(sigv4_signing_key(
                "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
                "20120215",
                "us-east-1",
                "iam"
            )),
            "f4780e2d9f65fa895f9c67b32ce1baf0b0d8a43505a000a1a9e090d414db404d"
        );
        // The SigV4 test suite case "get-vanilla".
        let headers = vec![
            ("Host".to_string(), "example.amazonaws.com".to_string()),
            ("X-Amz-Date".to_string(), "20150830T123600Z".to_string()),
        ];
        let r = SigV4Request {
            method: "GET",
            path: "/",
            query: "",
            headers: &headers,
            body: b"",
            access_key_id: "AKIDEXAMPLE",
            secret_access_key: "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            region: "us-east-1",
            service: "service",
            amz_date: "20150830T123600Z",
        };
        assert_eq!(
            sigv4_authorization(&r),
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, \
             SignedHeaders=host;x-amz-date, \
             Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
    }

    #[test]
    fn dates_base64_and_urls() {
        assert_eq!(amz_date(0), "19700101T000000Z");
        assert_eq!(amz_date(1_440_938_160), "20150830T123600Z");
        assert_eq!(amz_date(1_771_200_000), "20260216T000000Z");
        for n in 0..70usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base64_decode(&base64_encode(&data)).unwrap(), data);
        }
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert!(base64_decode("Zm9v!").is_none());
        assert!(Url::parse("http://example.com").is_err());
        assert!(Url::parse("ftp://x").is_err());
        let u = Url::parse("https://kms.eu-central-1.amazonaws.com").unwrap();
        assert_eq!((u.port, u.path.as_str()), (443, "/"));
        assert_eq!(u.host_header(), "kms.eu-central-1.amazonaws.com");
        let u = Url::parse("http://127.0.0.1:4567").unwrap();
        assert_eq!(u.host_header(), "127.0.0.1:4567");
        assert!(format!(
            "{:?}",
            Credentials {
                access_key_id: "A".into(),
                secret_access_key: "SECRET".into(),
                session_token: Some("TOK".into())
            }
        )
        .find("SECRET")
        .is_none());
    }

    #[test]
    fn oversized_messages_are_refused_before_any_request() {
        let s = KmsSigner::new(
            "alias/x",
            "eu-central-1",
            Some("http://127.0.0.1:1"),
            CredentialSource::Fixed(Credentials {
                access_key_id: "A".into(),
                secret_access_key: "B".into(),
                session_token: None,
            }),
        )
        .unwrap();
        let e = s.sign_bytes(&vec![0u8; MAX_RAW_MESSAGE + 1]).unwrap_err();
        assert!(e.to_string().contains("4096"));
    }
}
