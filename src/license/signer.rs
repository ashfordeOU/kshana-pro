// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Licence signers (key custody S7-a)
//!
//! Everything the vendor signs for licensing (licence tokens, the issuance register's
//! tokens, renewals, online check answers, revocation lists and Pro packs) signs through
//! one interface, [`LicenceSigner`], with two back ends:
//!
//! * **a raw seed** (`[u8; 32]` implements the trait): the offline root key on the owner's
//!   air-gapped machine, the tests and the release smoke test's throwaway key;
//! * **AWS KMS** ([`super::kms::KmsSigner`], cargo feature `kms`): the day-to-day issuing
//!   key, an `ECC_NIST_EDWARDS25519` key that cannot be exported, signing with
//!   `ED25519_SHA_512` and `MessageType: RAW`. KMS returns the same pure Ed25519 signature
//!   a seed would, so every verifier in this crate accepts it unchanged.
//!
//! The back end is picked by configuration, never by code: `KSHANA_PRO_SIGNER=seed` (the
//! default; the seed comes from `KSHANA_PRO_SIGNING_KEY`) or `KSHANA_PRO_SIGNER=kms` with
//! `KSHANA_PRO_KMS_KEY_ID` and the region (see [`from_env`]). No key material or credential
//! is ever written to this repository.

use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use kshana::api::KshanaError;

use super::{key_id, KEY_LEN};

/// Something that holds a licence signing key and signs with it.
pub trait LicenceSigner {
    /// The raw 32-byte Ed25519 public key.
    fn public_key(&self) -> Result<[u8; KEY_LEN], KshanaError>;
    /// A 64-byte pure Ed25519 signature over `msg`.
    fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 64], KshanaError>;
    /// What the signer is, for logs (never key material): `seed` or `aws-kms <key id>`.
    fn describe(&self) -> String;
    /// The key ID ([`key_id`]) of the public key.
    fn kid(&self) -> Result<String, KshanaError> {
        Ok(key_id(&self.public_key()?))
    }
}

/// The raw-seed back end: the seed is the 32-byte Ed25519 private key.
impl LicenceSigner for [u8; KEY_LEN] {
    fn public_key(&self) -> Result<[u8; KEY_LEN], KshanaError> {
        Ok(SigningKey::from_bytes(self).verifying_key().to_bytes())
    }
    fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 64], KshanaError> {
        Ok(SigningKey::from_bytes(self).sign(msg).to_bytes())
    }
    fn describe(&self) -> String {
        "seed".into()
    }
}

impl<T: LicenceSigner + ?Sized> LicenceSigner for Box<T> {
    fn public_key(&self) -> Result<[u8; KEY_LEN], KshanaError> {
        (**self).public_key()
    }
    fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 64], KshanaError> {
        (**self).sign_bytes(msg)
    }
    fn describe(&self) -> String {
        (**self).describe()
    }
}

impl<T: LicenceSigner + ?Sized> LicenceSigner for std::sync::Arc<T> {
    fn public_key(&self) -> Result<[u8; KEY_LEN], KshanaError> {
        (**self).public_key()
    }
    fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 64], KshanaError> {
        (**self).sign_bytes(msg)
    }
    fn describe(&self) -> String {
        (**self).describe()
    }
}

/// A signer shared between threads (the service, the payments webhook).
pub type SharedSigner = std::sync::Arc<dyn LicenceSigner + Send + Sync>;

/// Sign `payload` and check the signature against the signer's own public key before it
/// is used (a remote signer that answers with the wrong key or a damaged signature is
/// caught here, not by a customer). Returns `payload_hex.signature_hex`.
pub fn sign_document(signer: &dyn LicenceSigner, payload: &[u8]) -> Result<String, KshanaError> {
    let sig = sign_checked(signer, payload)?;
    Ok(format!("{}.{}", hex::encode(payload), hex::encode(sig)))
}

/// Sign `payload` and verify the signature under the signer's public key.
pub fn sign_checked(signer: &dyn LicenceSigner, payload: &[u8]) -> Result<[u8; 64], KshanaError> {
    let sig = signer.sign_bytes(payload)?;
    let pk = signer.public_key()?;
    let ok = VerifyingKey::from_bytes(&pk)
        .map(|k| {
            k.verify_strict(payload, &Signature::from_bytes(&sig))
                .is_ok()
        })
        .unwrap_or(false);
    if !ok {
        return Err(KshanaError::InvalidInput(format!(
            "the signer ({}) returned a signature that does not verify under its own public key",
            signer.describe()
        )));
    }
    Ok(sig)
}

/// The signer the vendor tools use, from the environment:
///
/// * `KSHANA_PRO_SIGNER` unset or `seed`: the raw seed in `KSHANA_PRO_SIGNING_KEY`
///   (64 hex characters). Debug builds without it fall back to the demo key (with a
///   warning), so a developer can run the commands; release builds refuse.
/// * `KSHANA_PRO_SIGNER=kms`: AWS KMS (needs the `kms` cargo feature). `KSHANA_PRO_KMS_KEY_ID`
///   (key ID, ARN or `alias/...`), `KSHANA_PRO_KMS_REGION` or `AWS_REGION`, credentials from
///   `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` and optionally `AWS_SESSION_TOKEN`;
///   `KSHANA_PRO_KMS_ENDPOINT` overrides the endpoint (tests, VPC endpoints);
///   `KSHANA_PRO_KMS_EXPECT_KID` pins the key ID the KMS key must have.
pub fn from_env(demo_seed: Option<[u8; KEY_LEN]>) -> Result<SharedSigner, KshanaError> {
    let which = std::env::var("KSHANA_PRO_SIGNER").unwrap_or_default();
    match which.trim().to_ascii_lowercase().as_str() {
        "" | "seed" => match std::env::var("KSHANA_PRO_SIGNING_KEY")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .ok_or(())
        {
            Ok(h) => Ok(std::sync::Arc::new(super::parse_key_hex(&h)?)),
            Err(_) => match demo_seed {
                Some(s) => {
                    eprintln!("[warning] KSHANA_PRO_SIGNING_KEY not set: signing with the DEMO key (debug builds only)");
                    Ok(std::sync::Arc::new(s))
                }
                None => Err(KshanaError::InvalidInput(
                    "set KSHANA_PRO_SIGNING_KEY to the vendor signing key, or KSHANA_PRO_SIGNER=kms"
                        .into(),
                )),
            },
        },
        "kms" => kms_from_env(),
        other => Err(KshanaError::InvalidInput(format!(
            "unknown KSHANA_PRO_SIGNER '{other}' (expected seed or kms)"
        ))),
    }
}

/// The Pro pack signer: AWS KMS when `KSHANA_PRO_PACK_SIGNER=kms` (key from
/// `KSHANA_PRO_PACK_KMS_KEY_ID`, the rest as in [`from_env`]), else the seed `seed` (read by
/// the caller from `KSHANA_PRO_PACK_SIGNING_KEY`).
pub fn pack_signer_from_env(
    seed: Result<[u8; KEY_LEN], KshanaError>,
) -> Result<SharedSigner, KshanaError> {
    if std::env::var("KSHANA_PRO_PACK_SIGNER").is_ok_and(|v| v.trim().eq_ignore_ascii_case("kms")) {
        return pack_kms_from_env();
    }
    Ok(std::sync::Arc::new(seed?))
}

#[cfg(feature = "kms")]
fn pack_kms_from_env() -> Result<SharedSigner, KshanaError> {
    let mut s = super::kms::KmsSigner::from_env()?;
    s.key_id = std::env::var("KSHANA_PRO_PACK_KMS_KEY_ID")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| {
            KshanaError::InvalidInput(
                "KSHANA_PRO_PACK_SIGNER=kms needs KSHANA_PRO_PACK_KMS_KEY_ID".into(),
            )
        })?;
    Ok(std::sync::Arc::new(s))
}

#[cfg(not(feature = "kms"))]
fn pack_kms_from_env() -> Result<SharedSigner, KshanaError> {
    kms_from_env()
}

#[cfg(feature = "kms")]
fn kms_from_env() -> Result<SharedSigner, KshanaError> {
    let s = super::kms::KmsSigner::from_env()?;
    if let Ok(want) = std::env::var("KSHANA_PRO_KMS_EXPECT_KID") {
        let got = s.kid()?;
        if !got.eq_ignore_ascii_case(want.trim()) {
            return Err(KshanaError::InvalidInput(format!(
                "the KMS key has key ID {got}, not the expected {}",
                want.trim()
            )));
        }
    }
    Ok(std::sync::Arc::new(s))
}

#[cfg(not(feature = "kms"))]
fn kms_from_env() -> Result<SharedSigner, KshanaError> {
    Err(KshanaError::Unsupported(
        "KSHANA_PRO_SIGNER=kms needs a build with the `kms` cargo feature".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Liar([u8; KEY_LEN]);
    impl LicenceSigner for Liar {
        fn public_key(&self) -> Result<[u8; KEY_LEN], KshanaError> {
            [9u8; KEY_LEN].public_key()
        }
        fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 64], KshanaError> {
            self.0.sign_bytes(msg)
        }
        fn describe(&self) -> String {
            "liar".into()
        }
    }

    #[test]
    fn seed_signer_matches_ed25519_and_a_wrong_key_is_caught() {
        let seed = [5u8; KEY_LEN];
        let doc = sign_document(&seed, b"hello").unwrap();
        let direct = SigningKey::from_bytes(&seed).sign(b"hello").to_bytes();
        assert_eq!(
            doc,
            format!("{}.{}", hex::encode(b"hello"), hex::encode(direct))
        );
        assert_eq!(seed.kid().unwrap(), key_id(&seed.public_key().unwrap()));
        assert!(sign_document(&Liar(seed), b"hello").is_err());
    }
}
