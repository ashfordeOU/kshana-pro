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
//! **Only the root retires keys (key custody S7-b).** A build told its root key ID(s)
//! (`KSHANA_PRO_ROOT_KID`, [`Keyring::add_root`]) accepts a list that retires any key (the
//! root included) only when a root signed it. A list signed by the day-to-day issuing key
//! may revoke tokens, nothing else: if it names a retired key it is refused as a whole, so
//! a stolen issuing key cannot retire the root or the other keys. No list may retire the
//! key that signed it (an old root is retired by the new root during a root rotation,
//! with both declared as roots in the overlap release). A build with no root
//! configured keeps the earlier rule (any trusted key) and the vendor tools warn about it.
//!
//! Format: `payload_hex.signature_hex`, like a token, where the payload is the JSON of
//! [`RevocationList`]. `sequence` only grows: a store refuses a list older than the
//! one it holds, so an old list cannot be replayed to un-revoke a token.

use kshana::api::KshanaError;

use super::signer::{sign_document, LicenceSigner};
use super::{check_signature, split_signed, Keyring};

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

/// Sign a list with a vendor signer (a seed or KMS; sets `format` and `kid`).
pub fn sign(signer: &dyn LicenceSigner, list: &RevocationList) -> Result<String, KshanaError> {
    let mut l = list.clone();
    l.format = FORMAT.to_string();
    l.kid = Some(signer.kid()?);
    let payload = serde_json::to_vec(&l)
        .map_err(|e| KshanaError::InvalidInput(format!("could not serialise list: {e}")))?;
    sign_document(signer, &payload)
}

/// Verify a signed list against `ring`.
pub fn verify(ring: &Keyring, text: &str) -> Result<RevocationList, KshanaError> {
    let (payload, sig) = split_signed(text)
        .map_err(|e| KshanaError::InvalidInput(format!("revocation list: {e}")))?;
    let list: RevocationList = serde_json::from_slice(&payload)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid revocation list: {e}")))?;
    let signed_by = check_signature(ring, list.kid.as_deref(), &payload, &sig)
        .map_err(|e| KshanaError::InvalidInput(format!("revocation list: {e}")))?;
    check_key_authority(ring, &signed_by, &list)?;
    if list.format != FORMAT {
        return Err(KshanaError::InvalidInput(format!(
            "unknown revocation list format '{}'",
            list.format
        )));
    }
    Ok(list)
}

/// Key custody S7-b: may the key `signed_by` retire the keys this list names?
fn check_key_authority(
    ring: &Keyring,
    signed_by: &str,
    list: &RevocationList,
) -> Result<(), KshanaError> {
    if list.retired_kids.is_empty() {
        return Ok(());
    }
    if !ring.root_ids().is_empty() && !ring.is_root(signed_by) {
        return Err(KshanaError::InvalidInput(format!(
            "revocation list signed by key {signed_by}, which is not a root key, names {} retired \
             key(s); only the root key may retire keys (the issuing key may revoke tokens only)",
            list.retired_kids.len()
        )));
    }
    if list
        .retired_kids
        .iter()
        .any(|k| k.eq_ignore_ascii_case(signed_by))
    {
        return Err(KshanaError::InvalidInput(format!(
            "revocation list retires the key that signed it ({signed_by}); a new key must sign it"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::{key_id, public_key_for, KEY_LEN};

    const SEED: [u8; KEY_LEN] = [3; KEY_LEN];

    #[test]
    fn only_the_root_may_retire_keys() {
        let root = [10u8; KEY_LEN];
        let issuing = [11u8; KEY_LEN];
        let other = [12u8; KEY_LEN];
        let (rk, ik, ok) = (
            key_id(&public_key_for(&root)),
            key_id(&public_key_for(&issuing)),
            key_id(&public_key_for(&other)),
        );
        let mut ring = Keyring::single(public_key_for(&root));
        ring.add(public_key_for(&issuing));
        ring.add(public_key_for(&other));
        ring.add_root(&rk);
        let tokens_only = RevocationList {
            sequence: 1,
            revoked: vec!["kt_x".into()],
            ..Default::default()
        };
        // The issuing key may revoke tokens.
        assert!(verify(&ring, &sign(&issuing, &tokens_only).unwrap()).is_ok());
        // It may not retire the root, another key, or itself.
        for target in [&rk, &ok, &ik] {
            let l = RevocationList {
                sequence: 2,
                revoked: vec!["kt_y".into()],
                retired_kids: vec![target.clone()],
                ..Default::default()
            };
            let e = verify(&ring, &sign(&issuing, &l).unwrap()).unwrap_err();
            assert!(
                e.to_string().contains("only the root key may retire keys"),
                "{e}"
            );
            // The root may retire the other keys, never itself.
            let r = verify(&ring, &sign(&root, &l).unwrap());
            if target == &rk {
                assert!(r.is_err());
            } else {
                assert_eq!(r.unwrap().retired_kids, vec![target.clone()]);
            }
        }
        // Two roots during a root rotation: the new root retires the old one.
        let new_root = [13u8; KEY_LEN];
        let nk = key_id(&public_key_for(&new_root));
        ring.add(public_key_for(&new_root));
        ring.add_root(&nk);
        let l = RevocationList {
            sequence: 3,
            retired_kids: vec![rk.clone()],
            ..Default::default()
        };
        let mut applied = ring.clone();
        verify(&ring, &sign(&new_root, &l).unwrap())
            .unwrap()
            .apply_to(&mut applied);
        assert!(applied.is_retired(&rk));
        assert!(!applied.is_retired(&nk));
    }

    #[test]
    fn without_a_root_the_earlier_rule_holds_but_no_key_retires_itself() {
        let a = [20u8; KEY_LEN];
        let b = [21u8; KEY_LEN];
        let mut ring = Keyring::single(public_key_for(&a));
        ring.add(public_key_for(&b));
        let retire_b = RevocationList {
            sequence: 1,
            retired_kids: vec![key_id(&public_key_for(&b))],
            ..Default::default()
        };
        assert!(verify(&ring, &sign(&a, &retire_b).unwrap()).is_ok());
        assert!(verify(&ring, &sign(&b, &retire_b).unwrap()).is_err());
    }

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
