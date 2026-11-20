// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Long-term signatures (E25 B2503)
//!
//! A signed dossier ([`crate::signed_dossier`]) proves who signed a delivery while the
//! signer's key is trusted and its signature algorithm is sound. Evidence is kept for years
//! (insurance claims, incident inquiries, certification files), longer than a key stays in
//! service and longer than a time-stamp authority's certificate lasts. This module adds a
//! long-term record, `longterm.json` beside `signature.json`, in the manner of the archive
//! time-stamp chains of RFC 4998 (Evidence Record Syntax) and the archive time-stamps of the
//! ETSI long-term signature levels. It is its own JSON format, not an implementation of
//! either standard.
//!
//! The record ([`LongTermRecord`]) holds:
//!
//! * **a key history** ([`KeyHistory`]): the signer's first key (self-signed, with the time it
//!   became valid) and every rotation after it, each signed by both the outgoing and the
//!   incoming key and chained to the event before it. A verifier who pins any one key of the
//!   history (an old one or the current one) reaches every other through the signatures, and
//!   learns from when to when each key could sign;
//! * **an archive time-stamp chain** ([`ArchiveStamp`]): stamp 0 is an RFC 3161 time-stamp
//!   token over the exact bytes of `signature.json` (which lists every file's SHA-256 and
//!   carries the Ed25519 signature), so it proves the signed dossier existed at the stamp's
//!   time. Each later stamp covers the stamp before it (its imprint, its token and its
//!   certificates, [`renewal_preimage`]) and is added while that stamp's certificates are
//!   still valid, so the chain carries the proof past each authority's certificate expiry
//!   and across a change of authority. Each stamp keeps the authority's root certificate,
//!   so the record verifies without a network; the verifier still pins which roots it
//!   trusts (the file, or its SHA-256 fingerprint).
//!
//! Verification ([`verify`]) is offline and checks: every file and the dossier signature;
//! the key history; that the dossier key is in it and a pinned key reaches it; each token's
//! CMS signature and certificate chain to a pinned root (the open engine's RFC 3161
//! verifier, `kshana::evidence::tsa`) and its imprint over the data this format says it
//! covers; that each renewal came before the previous stamp's certificates expired; that
//! the first stamp falls inside the dossier key's validity (so a dossier signed with a key
//! later rotated out or reported compromised still verifies, and one first stamped after
//! the key was retired does not); and that the last stamp is still within its certificates'
//! validity at the verification time (`--at`), or else when it should have been renewed.
//!
//! Not checked: certificate revocation (no CRL or OCSP is fetched; a record could store
//! them in a later format), a weakening of SHA-256 (the hash is named per stamp, and only
//! SHA-256 is accepted today), name and policy constraints. A time-stamp proves when data
//! existed; it says nothing about whether the evidence inside is correct.
//!
//! Adding stamps and recording keys are Pro (`pro-longterm-signature`); building a request
//! and verifying need no licence. A network time-stamp request is optional and goes through
//! the system `curl` (`kshana-pro longterm stamp`); `request` and `add-token` work with any
//! authority's own tools, offline.

use std::path::Path;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::license::{key_id, parse_key_hex, public_key_for, KEY_LEN};
use crate::obs_provenance::parse_time;
use crate::signed_dossier::SIGNATURE_FILE;

/// The feature that unlocks adding stamps and recording keys.
pub const KIND: &str = "pro-longterm-signature";
/// The record's file name, beside `signature.json`.
pub use crate::signed_dossier::LONGTERM_FILE;
/// The record's format tag.
pub const FORMAT: &str = "kshana-longterm/1";
/// The key history's format tag.
pub const HISTORY_FORMAT: &str = "kshana-key-history/1";
/// The only archive hash this version accepts.
pub const HASH_ALG: &str = "sha256";
const ZERO_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// How long before the last stamp's certificates expire the verifier starts advising renewal.
pub const RENEW_ADVICE_S: i64 = 365 * 86_400;

// ── key history ───────────────────────────────────────────────────────────────

/// One event of a key history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyEvent {
    /// `genesis` (the first key) or `rotation` (a new key replaces the current one).
    pub event: String,
    /// Who holds the key (an organisation or a role).
    pub holder: String,
    /// The key this event brings into service (hex).
    pub public_key: String,
    pub key_id: String,
    /// The key this rotation retires (absent for genesis).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_key_id: Option<String>,
    /// Genesis: the key is valid from here. Rotation: the outgoing key is retired and the new
    /// key valid from here (ISO 8601 UTC). For a compromise this is the time the compromise
    /// may have begun, which can be earlier than the rotation itself.
    pub effective_utc: String,
    /// `scheduled` or `compromise` (rotations only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// SHA-256 of the previous event's JSON (64 zeros for genesis).
    pub prev: String,
    /// Rotation: signature by the outgoing key. Empty for genesis.
    #[serde(default)]
    pub sig_from: String,
    /// Signature by the key this event brings into service.
    pub sig_self: String,
}

/// A signer's key history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyHistory {
    pub format: String,
    pub events: Vec<KeyEvent>,
}

/// When one key could sign: `[from, until)`.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyWindow {
    pub key: [u8; KEY_LEN],
    pub key_id: String,
    pub holder: String,
    pub from: i64,
    pub until: Option<i64>,
    /// Why it ended (`scheduled`, `compromise`), when it did.
    pub ended_by: Option<String>,
}

fn event_message(e: &KeyEvent) -> Result<Vec<u8>, String> {
    let mut body = e.clone();
    body.sig_from = String::new();
    body.sig_self = String::new();
    let mut out = HISTORY_FORMAT.as_bytes().to_vec();
    out.push(b'\n');
    out.extend(serde_json::to_vec(&body).map_err(|e| e.to_string())?);
    Ok(out)
}

fn event_hash(e: &KeyEvent) -> Result<String, String> {
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(e).map_err(|e| e.to_string())?,
    )))
}

fn ed_verify(pk: &[u8; KEY_LEN], msg: &[u8], sig_hex: &str) -> bool {
    let sig: Option<[u8; 64]> = hex::decode(sig_hex).ok().and_then(|b| b.try_into().ok());
    match (VerifyingKey::from_bytes(pk), sig) {
        (Ok(k), Some(s)) => k.verify_strict(msg, &Signature::from_bytes(&s)).is_ok(),
        _ => false,
    }
}

impl KeyHistory {
    /// A history whose first key is `seed`'s, valid from `valid_from`.
    pub fn genesis(
        seed: &[u8; KEY_LEN],
        holder: &str,
        valid_from: &str,
    ) -> Result<KeyHistory, String> {
        if holder.trim().is_empty() {
            return Err("a key history needs a holder".into());
        }
        parse_time(valid_from)?;
        let pk = public_key_for(seed);
        let mut e = KeyEvent {
            event: "genesis".into(),
            holder: holder.trim().into(),
            public_key: hex::encode(pk),
            key_id: key_id(&pk),
            from_key_id: None,
            effective_utc: valid_from.into(),
            reason: None,
            prev: ZERO_HASH.into(),
            sig_from: String::new(),
            sig_self: String::new(),
        };
        e.sig_self = hex::encode(
            SigningKey::from_bytes(seed)
                .sign(&event_message(&e)?)
                .to_bytes(),
        );
        Ok(KeyHistory {
            format: HISTORY_FORMAT.into(),
            events: vec![e],
        })
    }

    /// Retire the current key (`old_seed`, which must be it) in favour of `new_seed` from
    /// `effective`; both keys sign the event.
    pub fn rotate(
        &mut self,
        old_seed: &[u8; KEY_LEN],
        new_seed: &[u8; KEY_LEN],
        effective: &str,
        reason: &str,
    ) -> Result<(), String> {
        if !matches!(reason, "scheduled" | "compromise") {
            return Err("the reason is scheduled or compromise".into());
        }
        let windows = self.windows()?;
        let cur = windows.last().ok_or("an empty key history")?;
        let old_pk = public_key_for(old_seed);
        if old_pk != cur.key {
            return Err(format!(
                "the outgoing key {} is not the current key {}",
                key_id(&old_pk),
                cur.key_id
            ));
        }
        let new_pk = public_key_for(new_seed);
        if windows.iter().any(|w| w.key == new_pk) {
            return Err("the incoming key is already in the history".into());
        }
        if parse_time(effective)? < cur.from {
            return Err("a rotation cannot take effect before the current key's validity".into());
        }
        let last = self.events.last().ok_or("an empty key history")?;
        let mut e = KeyEvent {
            event: "rotation".into(),
            holder: last.holder.clone(),
            public_key: hex::encode(new_pk),
            key_id: key_id(&new_pk),
            from_key_id: Some(cur.key_id.clone()),
            effective_utc: effective.into(),
            reason: Some(reason.into()),
            prev: event_hash(last)?,
            sig_from: String::new(),
            sig_self: String::new(),
        };
        let msg = event_message(&e)?;
        e.sig_from = hex::encode(SigningKey::from_bytes(old_seed).sign(&msg).to_bytes());
        e.sig_self = hex::encode(SigningKey::from_bytes(new_seed).sign(&msg).to_bytes());
        self.events.push(e);
        self.windows().map(|_| ())
    }

    /// Check every event and return each key's validity window, in order.
    pub fn windows(&self) -> Result<Vec<KeyWindow>, String> {
        if self.format != HISTORY_FORMAT {
            return Err(format!("key history: unknown format '{}'", self.format));
        }
        let mut out: Vec<KeyWindow> = Vec::new();
        let mut prev = ZERO_HASH.to_string();
        for (i, e) in self.events.iter().enumerate() {
            let at = format!("key history event {i}");
            if e.prev != prev {
                return Err(format!("{at}: not chained to the event before it"));
            }
            let pk = parse_key_hex(&e.public_key).map_err(|x| format!("{at}: {x}"))?;
            if key_id(&pk) != e.key_id {
                return Err(format!("{at}: key_id does not match public_key"));
            }
            let t = parse_time(&e.effective_utc).map_err(|x| format!("{at}: {x}"))?;
            let msg = event_message(e)?;
            if !ed_verify(&pk, &msg, &e.sig_self) {
                return Err(format!("{at}: the key's own signature does not verify"));
            }
            match (i, e.event.as_str()) {
                (0, "genesis") => {}
                (0, _) => return Err(format!("{at}: a history starts with genesis")),
                (_, "rotation") => {
                    let cur = out.last_mut().expect("i > 0");
                    if e.from_key_id.as_deref() != Some(cur.key_id.as_str()) {
                        return Err(format!("{at}: does not retire the current key"));
                    }
                    if !ed_verify(&cur.key, &msg, &e.sig_from) {
                        return Err(format!(
                            "{at}: the outgoing key's signature does not verify"
                        ));
                    }
                    if t < cur.from {
                        return Err(format!(
                            "{at}: takes effect before the outgoing key's validity"
                        ));
                    }
                    if !matches!(e.reason.as_deref(), Some("scheduled" | "compromise")) {
                        return Err(format!("{at}: unknown reason"));
                    }
                    cur.until = Some(t);
                    cur.ended_by = e.reason.clone();
                }
                _ => return Err(format!("{at}: unexpected event '{}'", e.event)),
            }
            if out.iter().any(|w| w.key == pk) {
                return Err(format!("{at}: a key comes back into service"));
            }
            out.push(KeyWindow {
                key: pk,
                key_id: e.key_id.clone(),
                holder: e.holder.clone(),
                from: t,
                until: None,
                ended_by: None,
            });
            prev = event_hash(e)?;
        }
        if out.is_empty() {
            return Err("an empty key history".into());
        }
        Ok(out)
    }

    /// Read and check a history file.
    pub fn from_file(path: &Path) -> Result<KeyHistory, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let h: KeyHistory =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        h.windows()?;
        Ok(h)
    }
}

// ── DER (just enough for tokens and certificate validity) ─────────────────────

/// (tag, whole element, content, rest)
type Tlv<'a> = (u8, &'a [u8], &'a [u8], &'a [u8]);

fn tlv(b: &[u8]) -> Result<Tlv<'_>, String> {
    let tag = *b.first().ok_or("truncated DER")?;
    let l0 = *b.get(1).ok_or("truncated DER")?;
    let (len, hdr) = if l0 < 0x80 {
        (usize::from(l0), 2)
    } else {
        let k = usize::from(l0 & 0x7f);
        if k == 0 || k > 4 || b.len() < 2 + k {
            return Err("bad DER length".into());
        }
        (
            b[2..2 + k]
                .iter()
                .fold(0usize, |v, x| (v << 8) | usize::from(*x)),
            2 + k,
        )
    };
    let end = hdr.checked_add(len).ok_or("bad DER length")?;
    if b.len() < end {
        return Err("truncated DER".into());
    }
    Ok((tag, &b[..end], &b[hdr..end], &b[end..]))
}

fn children(mut b: &[u8]) -> Result<Vec<Tlv<'_>>, String> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let t = tlv(b)?;
        b = t.3;
        out.push(t);
    }
    Ok(out)
}

fn enc(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut o = vec![tag];
    let n = content.len();
    if n < 0x80 {
        o.push(n as u8);
    } else {
        let bytes: Vec<u8> = n
            .to_be_bytes()
            .iter()
            .copied()
            .skip_while(|&x| x == 0)
            .collect();
        o.push(0x80 | bytes.len() as u8);
        o.extend(bytes);
    }
    o.extend(content);
    o
}

const OID_SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];

/// An RFC 3161 `TimeStampReq` over a SHA-256 digest, asking for the authority's certificate
/// in the token (`certReq TRUE`), no policy and no nonce.
pub fn timestamp_request(digest: &[u8; 32]) -> Vec<u8> {
    let alg = enc(0x30, &[enc(0x06, OID_SHA256), vec![0x05, 0x00]].concat());
    let imprint = enc(0x30, &[alg, enc(0x04, digest)].concat());
    enc(
        0x30,
        &[enc(0x02, &[1]), imprint, vec![0x01, 0x01, 0xff]].concat(),
    )
}

/// The certificates a token carries (DER), from its `SignedData`.
pub fn token_certificates(token: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let (tag, _, body, _) = tlv(token)?;
    if tag != 0x30 {
        return Err("the token is not a DER sequence".into());
    }
    let kids = children(body)?;
    // TimeStampResp { PKIStatusInfo, ContentInfo } or ContentInfo { OID, [0] SignedData }.
    let ci_body = match kids.first().map(|k| k.0) {
        Some(0x30) => kids.get(1).ok_or("no token in the response")?.2,
        Some(0x06) => body,
        _ => return Err("not a time-stamp response or token".into()),
    };
    let ci = children(ci_body)?;
    let wrapped = ci.get(1).ok_or("no SignedData")?;
    let (_, _, sd_body, _) = tlv(wrapped.2)?;
    let mut out = Vec::new();
    for k in children(sd_body)? {
        if k.0 == 0xa0 {
            for c in children(k.2)? {
                if c.0 == 0x30 {
                    out.push(c.1.to_vec());
                }
            }
        }
    }
    Ok(out)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn der_time(tag: u8, s: &[u8]) -> Result<i64, String> {
    let digits = |b: &[u8]| -> Result<i64, String> {
        std::str::from_utf8(b)
            .ok()
            .filter(|t| t.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| "bad certificate time".to_string())
    };
    if s.last() != Some(&b'Z') {
        return Err("certificate time not in UTC".into());
    }
    let (y, r) = match tag {
        0x17 if s.len() >= 13 => {
            let yy = digits(&s[0..2])?;
            (if yy < 50 { 2000 + yy } else { 1900 + yy }, &s[2..])
        }
        0x18 if s.len() >= 15 => (digits(&s[0..4])?, &s[4..]),
        _ => return Err("bad certificate time".into()),
    };
    let f = |i: usize| digits(&r[i..i + 2]);
    Ok(days_from_civil(y, f(0)?, f(2)?) * 86_400 + f(4)? * 3600 + f(6)? * 60 + f(8)?)
}

/// A certificate's validity `(notBefore, notAfter)`, Unix seconds.
pub fn certificate_validity(der: &[u8]) -> Result<(i64, i64), String> {
    let (_, _, cert, _) = tlv(der)?;
    let (_, _, tbs, _) = tlv(cert)?;
    let mut k = children(tbs)?.into_iter();
    let mut first = k.next().ok_or("short certificate")?;
    if first.0 == 0xa0 {
        first = k.next().ok_or("short certificate")?; // version, then the serial
    }
    let _ = first;
    let _sig_alg = k.next().ok_or("short certificate")?;
    let _issuer = k.next().ok_or("short certificate")?;
    let validity = k.next().ok_or("short certificate")?;
    let t = children(validity.2)?;
    if t.len() != 2 {
        return Err("bad certificate validity".into());
    }
    Ok((der_time(t[0].0, t[0].2)?, der_time(t[1].0, t[1].2)?))
}

/// Read certificates from PEM (one or more `CERTIFICATE` blocks) or a single DER file.
pub fn read_certificates(bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::CertificateDer;
    if bytes.starts_with(b"-----") || bytes.windows(11).any(|w| w == b"-----BEGIN ") {
        let certs: Vec<Vec<u8>> = CertificateDer::pem_slice_iter(bytes)
            .map(|c| c.map(|c| c.as_ref().to_vec()))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("PEM: {e:?}"))?;
        if certs.is_empty() {
            return Err("no CERTIFICATE block in the PEM file".into());
        }
        Ok(certs)
    } else {
        certificate_validity(bytes)?;
        Ok(vec![bytes.to_vec()])
    }
}

// ── the record ────────────────────────────────────────────────────────────────

/// What the record protects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    /// The signed file whose exact bytes stamp 0 covers (`signature.json`).
    pub signature_file: String,
    pub signature_sha256: String,
}

/// One archive time-stamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveStamp {
    pub seq: u32,
    pub hash_alg: String,
    /// SHA-256 (hex) of the data this stamp covers: `signature.json` for stamp 0,
    /// [`renewal_preimage`] of the stamp before it otherwise.
    pub imprint: String,
    /// A name for the authority, for people.
    pub tsa: String,
    /// The authority's time as the token states it (informational; verification reads the
    /// token).
    pub gen_time: String,
    /// The token (DER, hex): a `TimeStampResp` or a bare `TimeStampToken`.
    pub token: String,
    /// Further certificates (DER, hex) the chain to the root needs, beyond those in the token.
    #[serde(default)]
    pub chain: Vec<String>,
    /// The authority's root certificate (DER, hex), kept so the record verifies offline. It
    /// is trusted only when the verifier pins it.
    pub anchor: String,
}

/// The long-term record, `longterm.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LongTermRecord {
    pub format: String,
    pub subject: Subject,
    pub key_history: KeyHistory,
    #[serde(default)]
    pub archive: Vec<ArchiveStamp>,
}

fn sha_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn unhex(s: &str, what: &str) -> Result<Vec<u8>, String> {
    hex::decode(s).map_err(|_| format!("{what}: not hex"))
}

/// The data stamp `prev.seq + 1` covers: a short text naming the stamp before it by its
/// imprint, the SHA-256 of its token and the SHA-256 of each chain certificate, one per line.
pub fn renewal_preimage(prev: &ArchiveStamp) -> Result<Vec<u8>, String> {
    let token = unhex(&prev.token, "token")?;
    let chain = prev
        .chain
        .iter()
        .map(|c| unhex(c, "chain certificate").map(|d| sha_hex(&d)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "{FORMAT} renewal\nseq={}\nhash_alg={HASH_ALG}\nprev_imprint={}\nprev_token_sha256={}\nprev_chain_sha256={}\n",
        prev.seq + 1,
        prev.imprint,
        sha_hex(&token),
        chain.join(",")
    )
    .into_bytes())
}

/// Read `longterm.json` from `dir`, if there is one.
pub fn read_record(dir: &Path) -> Result<Option<LongTermRecord>, String> {
    let p = dir.join(LONGTERM_FILE);
    if !p.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let r: LongTermRecord =
        serde_json::from_str(&text).map_err(|e| format!("{LONGTERM_FILE}: {e}"))?;
    if r.format != FORMAT {
        return Err(format!("{LONGTERM_FILE}: unknown format '{}'", r.format));
    }
    Ok(Some(r))
}

fn write_record(dir: &Path, r: &LongTermRecord) -> Result<(), String> {
    let p = dir.join(LONGTERM_FILE);
    let text = serde_json::to_string_pretty(r).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
}

fn signature_bytes(dir: &Path) -> Result<Vec<u8>, String> {
    let p = dir.join(SIGNATURE_FILE);
    std::fs::read(&p).map_err(|e| format!("{}: {e} (sign the directory first)", p.display()))
}

/// The data the next stamp must cover and its SHA-256.
pub fn next_preimage(dir: &Path) -> Result<(Vec<u8>, [u8; 32]), String> {
    let pre = match read_record(dir)?.and_then(|r| r.archive.last().cloned()) {
        None => signature_bytes(dir)?,
        Some(last) => renewal_preimage(&last)?,
    };
    let d: [u8; 32] = Sha256::digest(&pre).into();
    Ok((pre, d))
}

/// Add a time-stamp token as the next archive stamp. The first stamp needs the signer's
/// key history. The token must stamp exactly the next preimage and verify against `anchor`
/// (with `chain` as further certificates) before it is recorded.
pub fn add_stamp(
    dir: &Path,
    history: Option<KeyHistory>,
    token: &[u8],
    chain: &[Vec<u8>],
    anchor: &[u8],
    tsa: &str,
) -> Result<ArchiveStamp, String> {
    let sig = signature_bytes(dir)?;
    let mut rec = match read_record(dir)? {
        Some(mut r) => {
            if r.subject.signature_sha256 != sha_hex(&sig) {
                return Err(format!(
                    "{SIGNATURE_FILE} changed since the record was started; a long-term record covers one signature"
                ));
            }
            if let Some(h) = history {
                replace_history(&mut r, h)?;
            }
            r
        }
        None => {
            let h = history.ok_or(
                "the first archive stamp needs the signer's key history (--history <file>)",
            )?;
            h.windows()?;
            LongTermRecord {
                format: FORMAT.into(),
                subject: Subject {
                    signature_file: SIGNATURE_FILE.into(),
                    signature_sha256: sha_hex(&sig),
                },
                key_history: h,
                archive: Vec::new(),
            }
        }
    };
    let (pre, digest) = next_preimage(dir)?;
    let info = kshana::evidence::tsr::parse_token(token)?;
    if info.hash_alg != HASH_ALG || info.imprint != hex::encode(digest) {
        return Err(
            "the token does not stamp the data this record needs next (request it with `longterm request`)"
                .into(),
        );
    }
    certificate_validity(anchor).map_err(|e| format!("anchor: {e}"))?;
    let v = kshana::evidence::tsa::verify_token(token, &pre, &[anchor.to_vec()], chain);
    if !v.pass {
        return Err(format!(
            "the token does not verify against the given root: {}",
            v.reason
        ));
    }
    let stamp = ArchiveStamp {
        seq: rec.archive.len() as u32,
        hash_alg: HASH_ALG.into(),
        imprint: hex::encode(digest),
        tsa: if tsa.trim().is_empty() {
            "tsa".into()
        } else {
            tsa.trim().into()
        },
        gen_time: info.gen_time,
        token: hex::encode(token),
        chain: chain.iter().map(hex::encode).collect(),
        anchor: hex::encode(anchor),
    };
    rec.archive.push(stamp.clone());
    write_record(dir, &rec)?;
    Ok(stamp)
}

fn replace_history(rec: &mut LongTermRecord, h: KeyHistory) -> Result<(), String> {
    h.windows()?;
    let old = &rec.key_history.events;
    if h.events.len() < old.len() || h.events[..old.len()] != old[..] {
        return Err(
            "the new key history must extend the recorded one (same events, then new ones)".into(),
        );
    }
    rec.key_history = h;
    Ok(())
}

/// Replace the record's key history with an extension of it (after a key rotation).
pub fn update_history(dir: &Path, h: KeyHistory) -> Result<(), String> {
    let mut rec = read_record(dir)?.ok_or("no long-term record in this directory")?;
    replace_history(&mut rec, h)?;
    write_record(dir, &rec)
}

// ── verification ──────────────────────────────────────────────────────────────

/// What the verifier trusts and when it verifies.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// Signer keys pinned by the verifier (any key of the history will do).
    pub trust_keys: Vec<[u8; KEY_LEN]>,
    /// Time-stamp authority roots the verifier trusts (DER).
    pub tsa_anchors: Vec<Vec<u8>>,
    /// SHA-256 fingerprints (hex) of roots the verifier trusts; a stamp's stored root is
    /// used when its fingerprint is pinned.
    pub tsa_fingerprints: Vec<String>,
    /// The verification time, Unix seconds.
    pub at_unix: i64,
}

/// One stamp's outcome.
#[derive(Debug, Clone, Serialize)]
pub struct StampCheck {
    pub seq: u32,
    pub tsa: String,
    pub gen_time_unix: Option<i64>,
    /// The earliest certificate expiry among the token's and the chain's certificates.
    pub not_after_unix: Option<i64>,
    pub pass: bool,
    pub reason: String,
}

/// The outcome of [`verify`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct LongTermVerification {
    pub signer: String,
    pub key_id: String,
    pub files: usize,
    /// The dossier signature holds and every file matches.
    pub dossier_ok: bool,
    pub key_in_history: bool,
    /// A pinned key reaches the dossier key through the history.
    pub key_trusted: bool,
    /// The dossier key could sign at the first stamp's time.
    pub key_valid_at_first_stamp: Option<bool>,
    /// The first stamp's time: the dossier existed by then.
    pub existed_by_unix: Option<i64>,
    /// The last stamp's certificates expire then: add a stamp before it.
    pub renew_by_unix: Option<i64>,
    pub renew_soon: bool,
    pub stamps: Vec<StampCheck>,
    pub problems: Vec<String>,
}

impl LongTermVerification {
    pub fn passed(&self) -> bool {
        self.problems.is_empty()
            && self.dossier_ok
            && self.key_trusted
            && self.key_valid_at_first_stamp == Some(true)
            && !self.stamps.is_empty()
            && self.stamps.iter().all(|s| s.pass)
    }
}

fn earliest_not_after(token: &[u8], chain: &[Vec<u8>]) -> Option<i64> {
    token_certificates(token)
        .ok()?
        .iter()
        .chain(chain)
        .filter_map(|c| certificate_validity(c).ok().map(|v| v.1))
        .min()
}

/// Verify a directory's dossier signature and long-term record, offline.
pub fn verify(dir: &Path, opts: &VerifyOptions) -> Result<LongTermVerification, String> {
    let rec = read_record(dir)?.ok_or("no long-term record (longterm.json) in this directory")?;
    let sig_bytes = signature_bytes(dir)?;
    let dv = crate::signed_dossier::verify(dir, &[]).map_err(|e| e.to_string())?;
    let mut v = LongTermVerification {
        signer: dv.signer.clone(),
        key_id: dv.key_id.clone(),
        files: dv.files,
        dossier_ok: dv.signature_ok && dv.problems.is_empty(),
        ..Default::default()
    };
    v.problems.extend(dv.problems.iter().cloned());
    if rec.subject.signature_file != SIGNATURE_FILE
        || rec.subject.signature_sha256 != sha_hex(&sig_bytes)
    {
        v.problems
            .push(format!("{SIGNATURE_FILE} is not the one the record covers"));
    }
    let dossier_key = serde_json::from_slice::<serde_json::Value>(&sig_bytes)
        .ok()
        .and_then(|j| j.get("public_key")?.as_str().map(str::to_string))
        .and_then(|k| parse_key_hex(&k).ok());

    // Keys.
    let windows = match rec.key_history.windows() {
        Ok(w) => w,
        Err(e) => {
            v.problems.push(e);
            Vec::new()
        }
    };
    let window = dossier_key.and_then(|k| windows.iter().find(|w| w.key == k));
    v.key_in_history = window.is_some();
    if !v.key_in_history && !windows.is_empty() {
        v.problems
            .push("the dossier was signed with a key that is not in the key history".into());
    }
    v.key_trusted = v.key_in_history && windows.iter().any(|w| opts.trust_keys.contains(&w.key));
    if opts.trust_keys.is_empty() {
        v.problems
            .push("no signer key pinned: pass --trust with a key of the signer's history".into());
    } else if v.key_in_history && !v.key_trusted {
        v.problems
            .push("no pinned key is in the signer's key history".into());
    }

    // Stamps.
    if rec.archive.is_empty() {
        v.problems.push("the record has no archive stamp".into());
    }
    let mut prev: Option<(&ArchiveStamp, Option<i64>, Option<i64>)> = None;
    for (i, s) in rec.archive.iter().enumerate() {
        let mut c = StampCheck {
            seq: s.seq,
            tsa: s.tsa.clone(),
            gen_time_unix: None,
            not_after_unix: None,
            pass: false,
            reason: "-".into(),
        };
        let r = (|| -> Result<(), String> {
            if s.seq as usize != i {
                return Err("out of sequence".into());
            }
            if s.hash_alg != HASH_ALG {
                return Err(format!("hash '{}' is not accepted", s.hash_alg));
            }
            let token = unhex(&s.token, "token")?;
            let chain = s
                .chain
                .iter()
                .map(|x| unhex(x, "chain certificate"))
                .collect::<Result<Vec<_>, _>>()?;
            let pre = match prev {
                None => sig_bytes.clone(),
                Some((p, _, _)) => renewal_preimage(p)?,
            };
            if s.imprint != sha_hex(&pre) {
                return Err("the recorded imprint is not the hash of the data it covers".into());
            }
            let mut anchors = opts.tsa_anchors.clone();
            let stored = unhex(&s.anchor, "anchor")?;
            if !anchors.contains(&stored)
                && opts
                    .tsa_fingerprints
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&sha_hex(&stored)))
            {
                anchors.push(stored);
            }
            if anchors.is_empty() {
                return Err("no trusted root pinned (--tsa-anchor or --tsa-fingerprint)".into());
            }
            let tv = kshana::evidence::tsa::verify_token(&token, &pre, &anchors, &chain);
            c.gen_time_unix = tv.gen_time_unix;
            c.not_after_unix = earliest_not_after(&token, &chain);
            if !tv.pass {
                return Err(format!("the token does not verify: {}", tv.reason));
            }
            let g = tv.gen_time_unix.ok_or("no time in the token")?;
            if let Some((_, pg, pna)) = prev {
                if pg.is_some_and(|pg| g < pg) {
                    return Err("stamped before the stamp it renews".into());
                }
                match pna {
                    Some(na) if g < na => {}
                    Some(_) => {
                        return Err(
                            "added after the previous stamp's certificates had expired".into()
                        )
                    }
                    None => return Err("the previous stamp's expiry is unknown".into()),
                }
            }
            Ok(())
        })();
        match r {
            Ok(()) => c.pass = true,
            Err(e) => {
                v.problems.push(format!("archive stamp {i}: {e}"));
                c.reason = e;
            }
        }
        prev = Some((s, c.gen_time_unix, c.not_after_unix));
        v.stamps.push(c);
    }

    // The key at the first stamp, and the chain's end at the verification time.
    if let Some(first) = v.stamps.first().filter(|s| s.pass) {
        let g = first.gen_time_unix.unwrap_or(i64::MIN);
        v.existed_by_unix = Some(g);
        if let Some(w) = window {
            let ok = g >= w.from && w.until.is_none_or(|u| g < u);
            v.key_valid_at_first_stamp = Some(ok);
            if !ok {
                v.problems.push(if g < w.from {
                    "the dossier was first stamped before its key became valid".to_string()
                } else {
                    format!(
                        "the dossier was first stamped after its key was retired ({})",
                        w.ended_by.as_deref().unwrap_or("rotation")
                    )
                });
            }
        }
    }
    if v.stamps.iter().all(|s| s.pass) {
        if let Some(last) = v.stamps.last() {
            v.renew_by_unix = last.not_after_unix;
            match last.not_after_unix {
                Some(na) if opts.at_unix < na => {
                    v.renew_soon = na - opts.at_unix < RENEW_ADVICE_S;
                }
                Some(_) => v.problems.push(
                    "the last archive stamp's certificates have expired at the verification time; a stamp should have been added before"
                        .into(),
                ),
                None => v
                    .problems
                    .push("the last archive stamp's expiry is unknown".into()),
            }
        }
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const K1: [u8; KEY_LEN] = [21; KEY_LEN];
    const K2: [u8; KEY_LEN] = [22; KEY_LEN];
    const K3: [u8; KEY_LEN] = [23; KEY_LEN];

    #[test]
    fn key_history_rotations_and_windows() {
        let mut h = KeyHistory::genesis(&K1, "Ashforde OÜ", "2026-01-01T00:00:00Z").unwrap();
        h.rotate(&K1, &K2, "2027-01-01T00:00:00Z", "scheduled")
            .unwrap();
        h.rotate(&K2, &K3, "2027-06-01T00:00:00Z", "compromise")
            .unwrap();
        let w = h.windows().unwrap();
        assert_eq!(w.len(), 3);
        assert_eq!(
            w[0].until,
            Some(parse_time("2027-01-01T00:00:00Z").unwrap())
        );
        assert_eq!(w[1].ended_by.as_deref(), Some("compromise"));
        assert!(w[2].until.is_none());
        // Only the current key can be rotated out; a key cannot come back.
        let mut g = h.clone();
        assert!(g
            .rotate(&K1, &[24; KEY_LEN], "2028-01-01T00:00:00Z", "scheduled")
            .is_err());
        assert!(g
            .rotate(&K3, &K1, "2028-01-01T00:00:00Z", "scheduled")
            .is_err());
        // Tampering with any event, or dropping one, breaks the history.
        let mut t = h.clone();
        t.events[1].effective_utc = "2028-01-01T00:00:00Z".into();
        assert!(t.windows().is_err());
        let mut t = h.clone();
        t.events.remove(1);
        assert!(t.windows().is_err());
        // A rotation signed by the new key only (the outgoing signature replaced).
        let mut t = h.clone();
        t.events[1].sig_from = t.events[1].sig_self.clone();
        assert!(t.windows().is_err());
    }

    #[test]
    fn timestamp_request_der() {
        let d = [7u8; 32];
        let r = timestamp_request(&d);
        // SEQUENCE { INTEGER 1, SEQUENCE { SEQUENCE { OID sha256, NULL }, OCTET STRING }, BOOLEAN TRUE }
        let (tag, whole, body, rest) = tlv(&r).unwrap();
        assert_eq!((tag, whole.len(), rest.len()), (0x30, r.len(), 0));
        let k = children(body).unwrap();
        assert_eq!(k.len(), 3);
        assert_eq!((k[0].0, k[0].2), (0x02, &[1u8][..]));
        let imp = children(k[1].2).unwrap();
        assert_eq!(imp[1].2, &d[..]);
        assert_eq!((k[2].0, k[2].2), (0x01, &[0xffu8][..]));
    }

    #[test]
    fn der_times_and_lengths() {
        assert_eq!(der_time(0x17, b"700101000000Z").unwrap(), 0);
        assert_eq!(der_time(0x18, b"20500101000000Z").unwrap(), 2_524_608_000);
        assert_eq!(der_time(0x17, b"491231235959Z").unwrap(), 2_524_607_999);
        assert!(der_time(0x17, b"700101000000+").is_err());
        let long = enc(0x04, &[0u8; 300]);
        assert_eq!(&long[..4], &[0x04, 0x82, 0x01, 0x2c]);
        assert_eq!(tlv(&long).unwrap().2.len(), 300);
    }

    #[test]
    fn renewal_preimage_is_stable_text() {
        let s = ArchiveStamp {
            seq: 0,
            hash_alg: HASH_ALG.into(),
            imprint: "ab".repeat(32),
            tsa: "A".into(),
            gen_time: String::new(),
            token: "00".into(),
            chain: vec!["01".into()],
            anchor: String::new(),
        };
        let p = String::from_utf8(renewal_preimage(&s).unwrap()).unwrap();
        assert_eq!(
            p,
            format!(
                "kshana-longterm/1 renewal\nseq=1\nhash_alg=sha256\nprev_imprint={}\nprev_token_sha256={}\nprev_chain_sha256={}\n",
                "ab".repeat(32),
                sha_hex(&[0]),
                sha_hex(&[1])
            )
        );
    }
}
