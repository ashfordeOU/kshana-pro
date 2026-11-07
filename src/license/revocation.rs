// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Signed revocation list (roadmap F10c).
//!
//! The vendor builds the list from the issuance register ([`super::register`]) and
//! signs it with the licence signing key; it is shipped inside every update (Desktop,
//! Station, the CLI packages) and can be imported by hand on an air-gapped machine.
//! An installation never asks a server whether a token is revoked: a revoked token
//! stops working at the next update that carries the list.
//!
//! The list also carries retired key IDs, so a key retired after a release (key
//! rotation, [`super::Keyring`]) is refused by installed builds at their next update
//! without a rebuild.
//!
//! Format: `payload_hex.signature_hex`, like a token, where the payload is the JSON of
//! [`RevocationList`]. `sequence` only grows: a store refuses a list older than the
//! one it holds, so an old list cannot be replayed to un-revoke a token.

use ed25519_dalek::{Signer, SigningKey};
use kshana::api::KshanaError;

use super::{check_signature, key_id, public_key_for, split_signed, Keyring, KEY_LEN};

/// The `format` value of a revocation list.
pub const FORMAT: &str = "kshana-revocations/1";

/// A revocation list.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RevocationList {
    pub format: String,
    /// Grows with every list the vendor publishes.
    pub sequence: u64,
    pub issued_unix: i64,
    /// Revoked token IDs ([`super::token_id`]).
    #[serde(default)]
    pub revoked: Vec<String>,
    /// Retired signing-key IDs ([`super::key_id`]).
    #[serde(default)]
    pub retired_kids: Vec<String>,
    /// ID of the key that signed the list.
    #[serde(default)]
    pub kid: Option<String>,
}

impl RevocationList {
    /// Whether `token_id` is on the list.
    pub fn is_revoked(&self, token_id: &str) -> bool {
        self.revoked.iter().any(|r| r == token_id)
    }

    /// Retire the listed keys in `ring`.
    pub fn apply_to(&self, ring: &mut Keyring) {
        for k in &self.retired_kids {
            ring.retire(k);
        }
    }
}

/// Sign a list with the vendor seed (sets `format` and `kid`).
pub fn sign(seed: &[u8; KEY_LEN], list: &RevocationList) -> Result<String, KshanaError> {
    let mut l = list.clone();
    l.format = FORMAT.to_string();
    l.kid = Some(key_id(&public_key_for(seed)));
    let payload = serde_json::to_vec(&l)
        .map_err(|e| KshanaError::InvalidInput(format!("could not serialise list: {e}")))?;
    let sig = SigningKey::from_bytes(seed).sign(&payload);
    Ok(format!(
        "{}.{}",
        hex::encode(&payload),
        hex::encode(sig.to_bytes())
    ))
}

/// Verify a signed list against `ring`.
pub fn verify(ring: &Keyring, text: &str) -> Result<RevocationList, KshanaError> {
    let (payload, sig) = split_signed(text)
        .map_err(|e| KshanaError::InvalidInput(format!("revocation list: {e}")))?;
    let list: RevocationList = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid revocation list: {e}")))?;
    check_signature(ring, list.kid.as_deref(), &payload, &sig)
        .map_err(|e| KshanaError::InvalidInput(format!("revocation list: {e}")))?;
    if list.format != FORMAT {
        return Err(KshanaError::InvalidInput(format!(
            "unknown revocation list format '{}'",
            list.format
        )));
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; KEY_LEN] = [3; KEY_LEN];

    #[test]
    fn signed_list_round_trips_and_tamper_fails() {
        let ring = Keyring::single(public_key_for(&SEED));
        let list = RevocationList {
            sequence: 4,
            issued_unix: 100,
            revoked: vec!["kt_a".into()],
            ..Default::default()
        };
        let text = sign(&SEED, &list).unwrap();
        let back = verify(&ring, &text).unwrap();
        assert!(back.is_revoked("kt_a"));
        assert!(!back.is_revoked("kt_b"));
        assert_eq!(back.sequence, 4);

        // Change one payload byte: the signature no longer holds.
        let mut bad = text.clone();
        let c = if bad.starts_with('7') { "8" } else { "7" };
        bad.replace_range(0..1, c);
        assert!(verify(&ring, &bad).is_err());

        // A list signed by an unknown key is refused.
        let other = sign(&[4; KEY_LEN], &list).unwrap();
        assert!(verify(&ring, &other).is_err());
    }
}
