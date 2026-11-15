// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Result watermark (roadmap 0.42 C4)
//!
//! Every signed result Kshana Pro hands out carries a licence-holder mark, so a leaked
//! report can be traced to the licence it was made under. The mark is
//!
//! ```text
//! mark = HMAC-SHA256(watermark key, "kshana-pro-result-mark/v1|" licensee_id "|" nonce "|" content_sha256)[..16]
//! ```
//!
//! * **Per licensee:** `licensee_id` is the same salted hash of the licensee name the Pro
//!   pack watermark uses ([`crate::propack::format::licensee_id`]); the name itself is never
//!   in the result.
//! * **Invisible:** without the vendor's watermark key the mark is 16 random-looking bytes;
//!   it says nothing about who holds the licence. In text outputs (Markdown, HTML, CSV) the
//!   same bytes can also ride as zero-width characters ([`embed_text`]), and in 16-bit IQ
//!   files in the sample least significant bits ([`embed_lsb_i16`]), so a copy that loses its
//!   JSON envelope still carries the mark.
//! * **Bound to the content:** the content digest is inside the HMAC, so a mark copied onto
//!   another result does not verify; inside a signed envelope the signature covers it too.
//! * **Verifiable:** the vendor, holding the key and the licensee list (the issuance
//!   register), recovers the licensee with [`trace`]; [`verify`] checks one candidate.
//!
//! The watermark key is a vendor secret (`KSHANA_PRO_WATERMARK_KEY`, 64 hex characters, in
//! the server's secret store; never in a repository). Tests use throwaway keys.

use sha2::{Digest, Sha256};

/// The format tag inside the HMAC input.
pub const MARK_FORMAT: &str = "kshana-pro-result-mark/v1";

/// Length of a mark, in bytes.
pub const MARK_LEN: usize = 16;

/// A result watermark as it appears in a result envelope.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResultMark {
    /// [`MARK_FORMAT`].
    pub format: String,
    /// Random per result (hex, 16 bytes), so two results of one licensee carry unrelated marks.
    pub nonce: String,
    /// The mark (hex, [`MARK_LEN`] bytes).
    pub mark: String,
}

fn hmac(key: &[u8; 32], msg: &[u8]) -> [u8; 32] {
    crate::propack::format::hmac(key, msg)
}

fn mark_bytes(key: &[u8; 32], licensee_id: &str, nonce: &str, content: &[u8]) -> [u8; MARK_LEN] {
    let msg = format!(
        "{MARK_FORMAT}|{licensee_id}|{nonce}|{}",
        hex::encode(Sha256::digest(content))
    );
    let full = hmac(key, msg.as_bytes());
    let mut m = [0u8; MARK_LEN];
    m.copy_from_slice(&full[..MARK_LEN]);
    m
}

/// Mark `content` for `licensee` with a fresh random nonce.
pub fn mark(key: &[u8; 32], licensee: &str, content: &[u8]) -> ResultMark {
    use rand::RngCore;
    let mut n = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut n);
    mark_with_nonce(key, licensee, &hex::encode(n), content)
}

/// [`mark`] with a given nonce (reproducible outputs and tests).
pub fn mark_with_nonce(key: &[u8; 32], licensee: &str, nonce: &str, content: &[u8]) -> ResultMark {
    let lid = crate::propack::format::licensee_id(licensee);
    ResultMark {
        format: MARK_FORMAT.into(),
        nonce: nonce.into(),
        mark: hex::encode(mark_bytes(key, &lid, nonce, content)),
    }
}

/// Whether `m` was made for `licensee` over `content` with `key`.
pub fn verify(key: &[u8; 32], licensee: &str, content: &[u8], m: &ResultMark) -> bool {
    if m.format != MARK_FORMAT {
        return false;
    }
    let lid = crate::propack::format::licensee_id(licensee);
    let want = mark_bytes(key, &lid, &m.nonce, content);
    let Ok(got) = hex::decode(&m.mark) else {
        return false;
    };
    // Constant-time compare.
    got.len() == MARK_LEN
        && got
            .iter()
            .zip(want.iter())
            .fold(0u8, |a, (x, y)| a | (x ^ y))
            == 0
}

/// The licensee among `candidates` whose mark `m` is, if any.
pub fn trace<'a>(
    key: &[u8; 32],
    candidates: &'a [String],
    content: &[u8],
    m: &ResultMark,
) -> Option<&'a str> {
    candidates
        .iter()
        .find(|c| verify(key, c, content, m))
        .map(String::as_str)
}

/// The watermark key from `KSHANA_PRO_WATERMARK_KEY` (64 hex characters).
pub fn key_from_env() -> Result<[u8; 32], kshana::api::KshanaError> {
    let h = std::env::var("KSHANA_PRO_WATERMARK_KEY").map_err(|_| {
        kshana::api::KshanaError::InvalidInput(
            "set KSHANA_PRO_WATERMARK_KEY (64 hex characters, the vendor's watermark key)".into(),
        )
    })?;
    crate::license::parse_key_hex(&h)
}

// ---------------------------------------------------------------------------------
// Carriers: zero-width text and 16-bit sample least significant bits.
// ---------------------------------------------------------------------------------

const ZW0: char = '\u{200B}'; // zero width space: bit 0
const ZW1: char = '\u{200C}'; // zero width non-joiner: bit 1
const ZW_START: char = '\u{2060}'; // word joiner: frame start and end

/// `text` with the bytes of `m` (nonce then mark) hidden as zero-width characters after the
/// first line. Readers see no change; [`extract_text`] recovers them.
pub fn embed_text(text: &str, m: &ResultMark) -> String {
    let mut bits = String::new();
    bits.push(ZW_START);
    let bytes = [
        hex::decode(&m.nonce).unwrap_or_default(),
        hex::decode(&m.mark).unwrap_or_default(),
    ]
    .concat();
    for b in bytes {
        for i in (0..8).rev() {
            bits.push(if (b >> i) & 1 == 1 { ZW1 } else { ZW0 });
        }
    }
    bits.push(ZW_START);
    match text.find('\n') {
        Some(i) => format!("{}{}{}", &text[..i], bits, &text[i..]),
        None => format!("{text}{bits}"),
    }
}

/// The mark hidden by [`embed_text`], and the text without it.
pub fn extract_text(text: &str) -> Option<(ResultMark, String)> {
    let start = text.find(ZW_START)?;
    let rest = &text[start + ZW_START.len_utf8()..];
    let end = rest.find(ZW_START)?;
    let frame = &rest[..end];
    let bits: Vec<u8> = frame
        .chars()
        .map(|c| match c {
            ZW0 => Some(0u8),
            ZW1 => Some(1u8),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    if bits.len() != 8 * (16 + MARK_LEN) {
        return None;
    }
    let bytes: Vec<u8> = bits
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |a, b| (a << 1) | b))
        .collect();
    let clean = format!("{}{}", &text[..start], &rest[end + ZW_START.len_utf8()..]);
    Some((
        ResultMark {
            format: MARK_FORMAT.into(),
            nonce: hex::encode(&bytes[..16]),
            mark: hex::encode(&bytes[16..]),
        },
        clean,
    ))
}

/// Number of samples [`embed_lsb_i16`] uses: one bit per sample, a 32-bit sync word, the
/// nonce and the mark.
pub const LSB_SAMPLES: usize = 32 + 8 * (16 + MARK_LEN);
const LSB_SYNC: u32 = 0x4B53_574D; // "KSWM"

/// Hide `m` in the least significant bits of the first [`LSB_SAMPLES`] samples (changes
/// each by at most one count, far below any receiver's noise floor for synthetic data).
/// Returns false when there are too few samples.
pub fn embed_lsb_i16(samples: &mut [i16], m: &ResultMark) -> bool {
    if samples.len() < LSB_SAMPLES {
        return false;
    }
    let mut bits = Vec::with_capacity(LSB_SAMPLES);
    for i in (0..32).rev() {
        bits.push(((LSB_SYNC >> i) & 1) as i16);
    }
    for b in [
        hex::decode(&m.nonce).unwrap_or_default(),
        hex::decode(&m.mark).unwrap_or_default(),
    ]
    .concat()
    {
        for i in (0..8).rev() {
            bits.push(((b >> i) & 1) as i16);
        }
    }
    if bits.len() != LSB_SAMPLES {
        return false;
    }
    for (s, b) in samples.iter_mut().zip(bits) {
        *s = (*s & !1) | b;
    }
    true
}

/// The mark hidden by [`embed_lsb_i16`], if the sync word is there.
pub fn extract_lsb_i16(samples: &[i16]) -> Option<ResultMark> {
    if samples.len() < LSB_SAMPLES {
        return None;
    }
    let bits: Vec<u8> = samples[..LSB_SAMPLES]
        .iter()
        .map(|s| (*s & 1) as u8)
        .collect();
    let sync = bits[..32]
        .iter()
        .fold(0u32, |a, b| (a << 1) | u32::from(*b));
    if sync != LSB_SYNC {
        return None;
    }
    let bytes: Vec<u8> = bits[32..]
        .chunks(8)
        .map(|c| c.iter().fold(0u8, |a, b| (a << 1) | b))
        .collect();
    Some(ResultMark {
        format: MARK_FORMAT.into(),
        nonce: hex::encode(&bytes[..16]),
        mark: hex::encode(&bytes[16..]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [8; 32];

    #[test]
    fn marks_are_per_licensee_bound_to_content_and_traceable() {
        let content = b"{\"verdict\":\"pass\"}";
        let m = mark(&KEY, "Acme Ltd", content);
        assert!(verify(&KEY, "Acme Ltd", content, &m));
        assert!(verify(&KEY, "  acme ltd ", content, &m), "name normalised");
        assert!(!verify(&KEY, "Other GmbH", content, &m));
        assert!(!verify(&KEY, "Acme Ltd", b"{\"verdict\":\"fail\"}", &m));
        assert!(!verify(&[9; 32], "Acme Ltd", content, &m));
        let who = vec!["Other GmbH".to_string(), "Acme Ltd".to_string()];
        assert_eq!(trace(&KEY, &who, content, &m), Some("Acme Ltd"));
        // Two results of one licensee carry unrelated marks.
        let m2 = mark(&KEY, "Acme Ltd", content);
        assert_ne!(m.mark, m2.mark);
        // The mark names nobody.
        let json = serde_json::to_string(&m).unwrap();
        assert!(!json.to_lowercase().contains("acme"));
    }

    #[test]
    fn text_and_sample_carriers_round_trip() {
        let m = mark_with_nonce(&KEY, "Acme Ltd", &"ab".repeat(16), b"x");
        let text = "# Report\n\nVerdict: pass\n";
        let marked = embed_text(text, &m);
        assert_ne!(marked, text);
        assert_eq!(
            marked.replace(['\u{200B}', '\u{200C}', '\u{2060}'], ""),
            text
        );
        let (back, clean) = extract_text(&marked).unwrap();
        assert_eq!(back, m);
        assert_eq!(clean, text);
        assert!(extract_text(text).is_none());

        let mut s: Vec<i16> = (0..1000).map(|i| ((i * 97) % 2000 - 1000) as i16).collect();
        let orig = s.clone();
        assert!(embed_lsb_i16(&mut s, &m));
        assert!(s.iter().zip(&orig).all(|(a, b)| (a - b).abs() <= 1));
        assert_eq!(extract_lsb_i16(&s).unwrap(), m);
        assert!(extract_lsb_i16(&orig).is_none());
        assert!(!embed_lsb_i16(&mut [0i16; 10], &m));
    }
}
