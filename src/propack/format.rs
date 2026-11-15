// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The Pro pack file format (roadmap 0.42 C10).
//!
//! ```text
//! offset  size  content
//! 0       8     magic "KSPPACK1"
//! 8       4     n, the length of the signed manifest (big-endian u32)
//! 12      n     the signed manifest: <manifest JSON hex>.<Ed25519 signature hex>
//! 12+n    rest  the payload, sealed with ChaCha20-Poly1305 (ciphertext and 16-byte tag)
//! ```
//!
//! * **Manifest** ([`PackManifest`]): pack id, version, platform, licensee and licensee
//!   id, the token the pack was issued for, the device it is bound to, issue time, licence
//!   expiry and grace days, the SHA-256 and length of the plaintext payload, the key
//!   derivation salt, the nonce, the kinds the pack provides, the watermark id and, for
//!   macOS, the Apple team ID the native payload is signed with. Signed with the vendor's
//!   pack key (Ed25519). The manifest bytes are also the AEAD's associated data, so a
//!   payload cannot be moved under another manifest.
//! * **Keys.** The server holds one pack master secret. Per licence (token) it derives
//!   the licence secret `HKDF-SHA256(master, salt "kshana-pro-pack/licence/v1", info
//!   token id)`, which is handed to the app with the pack. The payload key is
//!   `HKDF-SHA256(licence secret, salt = the manifest's random salt, info =
//!   "kshana-pro-pack/content/v1" | device | pack id | version | platform)`: it needs the
//!   licence secret and the device's machine hash, and is fresh for every pack built (new
//!   salt), so the random 96-bit nonce is never reused under a key.
//! * **Payload**: a container of named files ([`Container`]); the builder adds the
//!   per-licensee watermark entry [`WATERMARK_ENTRY`] before sealing.

use ring::aead;
use ring::hkdf;
use ring::rand::{SecureRandom, SystemRandom};
use sha2::{Digest, Sha256};

use ed25519_dalek::{Signature, VerifyingKey};
use kshana::api::KshanaError;

use crate::license::{key_id, KEY_LEN};

/// The file's first eight bytes.
pub const MAGIC: &[u8; 8] = b"KSPPACK1";
/// The manifest's `format`.
pub const FORMAT: &str = "kshana-pro-pack/1";
/// The AEAD named in the manifest.
pub const AEAD: &str = "chacha20-poly1305";
/// The container's first four bytes.
pub const CONTAINER_MAGIC: &[u8; 4] = b"KPC1";
/// The container entry holding the per-licensee watermark.
pub const WATERMARK_ENTRY: &str = ".kshana/watermark.json";
/// Largest manifest accepted (bytes).
pub const MAX_MANIFEST: usize = 64 * 1024;
/// Largest payload accepted (bytes).
pub const MAX_PAYLOAD: usize = 1 << 30;

fn bad(m: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(format!("pro pack: {}", m.into()))
}

/// The signed manifest.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    pub format: String,
    /// Lowercase letters, digits and `-`, 1 to 64 characters.
    pub pack_id: String,
    pub version: String,
    /// `macos-aarch64`, `macos-x86_64`, `windows-x86_64`, `windows-aarch64`,
    /// `linux-x86_64`, `linux-aarch64`, or `any` for a payload without native code.
    pub platform: String,
    pub licensee: String,
    /// [`licensee_id`] of the licensee.
    pub licensee_id: String,
    /// The licence token the pack was issued for (its revocation name).
    pub token_id: String,
    /// The machine hash of the one device the pack opens on.
    pub device: String,
    pub issued_unix: i64,
    /// The licence's expiry (Unix seconds; never 0 for a pack).
    pub expires_unix: i64,
    /// Days after expiry the pack keeps opening, with a daily warning.
    pub grace_days: i64,
    /// SHA-256 of the plaintext payload (the container with the watermark), hex.
    pub payload_sha256: String,
    pub payload_len: u64,
    /// Key derivation salt (32 bytes, hex).
    pub salt: String,
    /// AEAD nonce (12 bytes, hex).
    pub nonce: String,
    pub aead: String,
    /// Kinds and command features the pack provides (each `pro-` prefixed).
    #[serde(default)]
    pub provides: Vec<String>,
    /// The first 16 bytes (hex) of the watermark mark; the full mark is in the payload.
    pub watermark_id: String,
    /// macOS: the Apple team ID the native payload is signed with (library validation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_team_id: Option<String>,
    /// Key ID of the pack signing key.
    pub kid: String,
}

/// The stable id of a licensee: SHA-256 of a fixed prefix and the trimmed, lowercased name.
pub fn licensee_id(licensee: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"kshana-licensee-v1:");
    h.update(licensee.trim().to_lowercase().as_bytes());
    hex::encode(h.finalize())
}

/// Whether `id` is a valid pack id.
pub fn valid_pack_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
}

/// The platforms a pack may name.
pub const PLATFORMS: &[&str] = &[
    "macos-aarch64",
    "macos-x86_64",
    "windows-x86_64",
    "windows-aarch64",
    "linux-x86_64",
    "linux-aarch64",
    "any",
];

/// The platform this build runs on, in the pack's naming.
pub fn this_platform() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        _ => "linux",
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

// ─── Crypto ──────────────────────────────────────────────────────────────────────────

struct Len32;
impl hkdf::KeyType for Len32 {
    fn len(&self) -> usize {
        32
    }
}

/// HKDF-SHA256 (RFC 5869) to 32 bytes.
pub fn hkdf32(ikm: &[u8], salt: &[u8], info: &[&[u8]]) -> [u8; 32] {
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, salt).extract(ikm);
    let mut out = [0u8; 32];
    // 32 bytes is far inside HKDF's limit (255 blocks), so neither call can fail.
    if let Ok(okm) = prk.expand(info, Len32) {
        let _ = okm.fill(&mut out);
    }
    out
}

/// The licence secret for `token_id` under the server's master secret.
pub fn licence_secret(master: &[u8; 32], token_id: &str) -> [u8; 32] {
    hkdf32(
        master,
        b"kshana-pro-pack/licence/v1",
        &[token_id.as_bytes()],
    )
}

/// The watermark key under the server's master secret.
pub fn watermark_key(master: &[u8; 32]) -> [u8; 32] {
    hkdf32(master, b"kshana-pro-pack/watermark/v1", &[b"mark"])
}

/// The payload key for one pack on one device.
pub fn content_key(
    licence_secret: &[u8; 32],
    m: &PackManifest,
    device: &str,
) -> Result<[u8; 32], KshanaError> {
    let salt = hex::decode(&m.salt).map_err(|_| bad("salt is not hex"))?;
    Ok(hkdf32(
        licence_secret,
        &salt,
        &[
            b"kshana-pro-pack/content/v1",
            b"|",
            device.as_bytes(),
            b"|",
            m.pack_id.as_bytes(),
            b"|",
            m.version.as_bytes(),
            b"|",
            m.platform.as_bytes(),
        ],
    ))
}

fn random<const N: usize>() -> Result<[u8; N], KshanaError> {
    let mut b = [0u8; N];
    SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| bad("the system random source failed"))?;
    Ok(b)
}

fn aead_key(key: &[u8; 32]) -> Result<aead::LessSafeKey, KshanaError> {
    aead::UnboundKey::new(&aead::CHACHA20_POLY1305, key)
        .map(aead::LessSafeKey::new)
        .map_err(|_| bad("could not make the payload key"))
}

fn nonce_of(m: &PackManifest) -> Result<aead::Nonce, KshanaError> {
    let n: [u8; 12] = hex::decode(&m.nonce)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| bad("nonce is not 12 hex bytes"))?;
    Ok(aead::Nonce::assume_unique_for_key(n))
}

/// HMAC-SHA256 (ring).
pub fn hmac(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    let mut out = [0u8; 32];
    out.copy_from_slice(ring::hmac::sign(&k, msg).as_ref());
    out
}

// ─── Container ───────────────────────────────────────────────────────────────────────

/// A container of named files: `KPC1`, a big-endian u32 count, then per entry a u16 name
/// length, the UTF-8 name, a u64 data length and the data.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Container {
    pub entries: Vec<(String, Vec<u8>)>,
}

/// Whether `name` is a safe relative path inside a container (no `..`, no absolute path,
/// no backslash, no empty component).
pub fn valid_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && name
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}

impl Container {
    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d.as_slice())
    }

    /// Add or replace an entry.
    pub fn put(&mut self, name: &str, data: Vec<u8>) -> Result<(), KshanaError> {
        if !valid_entry_name(name) {
            return Err(bad(format!("bad entry name '{name}'")));
        }
        self.entries.retain(|(n, _)| n != name);
        self.entries.push((name.to_string(), data));
        Ok(())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(CONTAINER_MAGIC);
        out.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        for (n, d) in &self.entries {
            out.extend_from_slice(&(n.len() as u16).to_be_bytes());
            out.extend_from_slice(n.as_bytes());
            out.extend_from_slice(&(d.len() as u64).to_be_bytes());
            out.extend_from_slice(d);
        }
        out
    }

    pub fn from_bytes(b: &[u8]) -> Result<Container, KshanaError> {
        let mut r = Reader { b, at: 0 };
        if r.take(4)? != CONTAINER_MAGIC {
            return Err(bad("payload is not a pack container"));
        }
        let n = u32::from_be_bytes(r.take(4)?.try_into().unwrap_or([0; 4])) as usize;
        if n > 100_000 {
            return Err(bad("too many container entries"));
        }
        let mut c = Container::default();
        for _ in 0..n {
            let nl = u16::from_be_bytes(r.take(2)?.try_into().unwrap_or([0; 2])) as usize;
            let name = std::str::from_utf8(r.take(nl)?)
                .map_err(|_| bad("entry name is not UTF-8"))?
                .to_string();
            let dl = u64::from_be_bytes(r.take(8)?.try_into().unwrap_or([0; 8]));
            let dl = usize::try_from(dl).map_err(|_| bad("entry too large"))?;
            let data = r.take(dl)?.to_vec();
            if !valid_entry_name(&name) || c.get(&name).is_some() {
                return Err(bad("bad or repeated entry name"));
            }
            c.entries.push((name, data));
        }
        if r.at != b.len() {
            return Err(bad("trailing bytes after the container"));
        }
        Ok(c)
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], KshanaError> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.b.len());
        let end = end.ok_or_else(|| bad("truncated"))?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }
}

// ─── Watermark ───────────────────────────────────────────────────────────────────────

/// The watermark entry: who the payload was issued to, and a mark only the vendor can
/// make and check (HMAC under the watermark key).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Watermark {
    pub licensee_id: String,
    pub token_id: String,
    pub pack_id: String,
    pub version: String,
    pub issued_unix: i64,
    /// HMAC-SHA256 under the watermark key of the fields above, hex.
    pub mark: String,
}

fn mark_input(w: &Watermark) -> Vec<u8> {
    format!(
        "kshana-pro-pack/mark/v1|{}|{}|{}|{}|{}",
        w.licensee_id, w.token_id, w.pack_id, w.version, w.issued_unix
    )
    .into_bytes()
}

impl Watermark {
    pub fn new(
        key: &[u8; 32],
        licensee_id: &str,
        token_id: &str,
        pack_id: &str,
        version: &str,
        issued_unix: i64,
    ) -> Watermark {
        let mut w = Watermark {
            licensee_id: licensee_id.into(),
            token_id: token_id.into(),
            pack_id: pack_id.into(),
            version: version.into(),
            issued_unix,
            mark: String::new(),
        };
        w.mark = hex::encode(hmac(key, &mark_input(&w)));
        w
    }

    /// Whether the mark was made with `key` over these fields.
    pub fn verify(&self, key: &[u8; 32]) -> bool {
        let want = hmac(key, &mark_input(self));
        hex::decode(&self.mark)
            .map(|got| crate::payments::provider::ct_eq(&got, &want))
            .unwrap_or(false)
    }
}

/// Read the watermark of a decrypted payload and check its mark (vendor side, to trace a
/// leaked payload to its licensee).
pub fn trace(payload: &[u8], watermark_key: &[u8; 32]) -> Result<Watermark, KshanaError> {
    let c = Container::from_bytes(payload)?;
    let w: Watermark = serde_json::from_slice(
        c.get(WATERMARK_ENTRY)
            .ok_or_else(|| bad("the payload carries no watermark"))?,
    )
    .map_err(|e| bad(format!("watermark unreadable: {e}")))?;
    if !w.verify(watermark_key) {
        return Err(bad("the watermark's mark does not verify"));
    }
    Ok(w)
}

// ─── Seal and open ───────────────────────────────────────────────────────────────────

/// What the builder needs to seal one pack.
pub struct SealInput<'a> {
    pub pack_id: &'a str,
    pub version: &'a str,
    pub platform: &'a str,
    pub licensee: &'a str,
    pub token_id: &'a str,
    pub device: &'a str,
    pub issued_unix: i64,
    pub expires_unix: i64,
    pub grace_days: i64,
    pub provides: Vec<String>,
    pub apple_team_id: Option<String>,
}

/// Seal `base` (the unwatermarked container) for one licence and device: add the
/// watermark, encrypt, sign. Returns the pack file bytes and its manifest.
pub fn seal(
    base: &Container,
    input: &SealInput<'_>,
    master: &[u8; 32],
    signer: &dyn crate::license::LicenceSigner,
) -> Result<(Vec<u8>, PackManifest), KshanaError> {
    if !valid_pack_id(input.pack_id) {
        return Err(bad("pack id must be 1 to 64 of a-z, 0-9 and -"));
    }
    if !PLATFORMS.contains(&input.platform) {
        return Err(bad(format!("unknown platform '{}'", input.platform)));
    }
    if input.device.len() != 64 || hex::decode(input.device).is_err() {
        return Err(bad("device must be a 64-hex machine hash"));
    }
    if input.expires_unix <= 0 {
        return Err(bad("a pack needs a licence with an expiry"));
    }
    if input.version.is_empty() || input.version.len() > 64 {
        return Err(bad("version must be 1 to 64 characters"));
    }
    for p in &input.provides {
        if !p.starts_with(crate::registry::PRO_KIND_PREFIX) {
            return Err(bad("every provided id carries the pro- prefix"));
        }
    }
    let lid = licensee_id(input.licensee);
    let wm = Watermark::new(
        &watermark_key(master),
        &lid,
        input.token_id,
        input.pack_id,
        input.version,
        input.issued_unix,
    );
    let mut payload = base.clone();
    payload.put(
        WATERMARK_ENTRY,
        serde_json::to_vec(&wm).map_err(|e| bad(e.to_string()))?,
    )?;
    let mut buf = payload.to_bytes();
    if buf.len() > MAX_PAYLOAD {
        return Err(bad("payload too large"));
    }
    let manifest = PackManifest {
        format: FORMAT.into(),
        pack_id: input.pack_id.into(),
        version: input.version.into(),
        platform: input.platform.into(),
        licensee: input.licensee.into(),
        licensee_id: lid,
        token_id: input.token_id.into(),
        device: input.device.to_ascii_lowercase(),
        issued_unix: input.issued_unix,
        expires_unix: input.expires_unix,
        grace_days: input.grace_days,
        payload_sha256: hex::encode(Sha256::digest(&buf)),
        payload_len: buf.len() as u64,
        salt: hex::encode(random::<32>()?),
        nonce: hex::encode(random::<12>()?),
        aead: AEAD.into(),
        provides: input.provides.clone(),
        watermark_id: wm.mark[..32].to_string(),
        apple_team_id: input.apple_team_id.clone(),
        kid: signer.kid()?,
    };
    let json = serde_json::to_vec(&manifest).map_err(|e| bad(e.to_string()))?;
    let signed = crate::license::signer::sign_document(signer, &json)?;
    let key = content_key(
        &licence_secret(master, input.token_id),
        &manifest,
        &manifest.device,
    )?;
    aead_key(&key)?
        .seal_in_place_append_tag(
            nonce_of(&manifest)?,
            aead::Aad::from(signed.as_bytes()),
            &mut buf,
        )
        .map_err(|_| bad("sealing failed"))?;
    let mut out = Vec::with_capacity(12 + signed.len() + buf.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(signed.len() as u32).to_be_bytes());
    out.extend_from_slice(signed.as_bytes());
    out.extend_from_slice(&buf);
    Ok((out, manifest))
}

/// A pack file split into its parts, the manifest signature checked.
#[derive(Debug, Clone)]
pub struct SignedPack {
    pub manifest: PackManifest,
    /// The signed manifest text (the AEAD's associated data).
    pub signed: String,
    /// The kid of the key that verified it.
    pub kid: String,
    sealed_at: usize,
}

/// Split a pack file and verify its manifest signature against `keys` (the pack signing
/// public keys this build trusts). Does not decrypt.
pub fn verify_signed(file: &[u8], keys: &[[u8; KEY_LEN]]) -> Result<SignedPack, KshanaError> {
    if file.len() < 12 || &file[..8] != MAGIC {
        return Err(bad("not a Kshana Pro pack"));
    }
    let n = u32::from_be_bytes(file[8..12].try_into().unwrap_or([0; 4])) as usize;
    if n > MAX_MANIFEST || 12 + n > file.len() {
        return Err(bad("manifest length out of range"));
    }
    let signed = std::str::from_utf8(&file[12..12 + n])
        .map_err(|_| bad("manifest is not text"))?
        .to_string();
    let (jh, sh) = signed
        .split_once('.')
        .ok_or_else(|| bad("manifest is not payload.signature"))?;
    let json = hex::decode(jh).map_err(|_| bad("manifest is not hex"))?;
    let sig: [u8; 64] = hex::decode(sh)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| bad("signature is not 64 hex bytes"))?;
    let sig = Signature::from_bytes(&sig);
    let mut kid = None;
    for k in keys {
        if let Ok(vk) = VerifyingKey::from_bytes(k) {
            if vk.verify_strict(&json, &sig).is_ok() {
                kid = Some(key_id(k));
                break;
            }
        }
    }
    let kid = kid.ok_or_else(|| bad("bad signature (tampered, or not signed by the vendor)"))?;
    let manifest: PackManifest =
        serde_json::from_slice(&json).map_err(|e| bad(format!("manifest unreadable: {e}")))?;
    if manifest.format != FORMAT || manifest.aead != AEAD {
        return Err(bad("unknown pack format"));
    }
    if !valid_pack_id(&manifest.pack_id) {
        return Err(bad("bad pack id"));
    }
    Ok(SignedPack {
        manifest,
        signed,
        kid,
        sealed_at: 12 + n,
    })
}

/// Decrypt a verified pack with the licence secret on the device `device`, and check the
/// payload hash and length. Fails on a wrong device or secret (the tag does not verify).
pub fn open_sealed(
    file: &[u8],
    sp: &SignedPack,
    licence_secret: &[u8; 32],
    device: &str,
) -> Result<Vec<u8>, KshanaError> {
    let key = content_key(licence_secret, &sp.manifest, &device.to_ascii_lowercase())?;
    let mut buf = file[sp.sealed_at..].to_vec();
    let plain = aead_key(&key)?
        .open_in_place(
            nonce_of(&sp.manifest)?,
            aead::Aad::from(sp.signed.as_bytes()),
            &mut buf,
        )
        .map_err(|_| bad("the payload does not decrypt on this device with this licence"))?;
    let plain = plain.to_vec();
    if plain.len() as u64 != sp.manifest.payload_len
        || hex::encode(Sha256::digest(&plain)) != sp.manifest.payload_sha256
    {
        return Err(bad("the payload hash does not match the signed manifest"));
    }
    Ok(plain)
}
