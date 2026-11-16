// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The user receiver of method `kshana-pro/pnt-verification/v2` (E4b pre-registration,
//! `docs/evidence/e4-pnt-verification/2026-11-16_E4b_PREREGISTRATION.md`, section 2.2): a
//! Pro-side wrapper over the open OSNMA verifier (kshana 0.41.0) with three policies.
//!
//! 1. **Session-long failure memory**: the open verifier with `failure_memory_s` at its maximum,
//!    so a failed tag keeps the satellite `failed` to the end of the session.
//! 2. **Hold until the chain is verified**: the stream is read once to find the first TESLA chain
//!    whose DSM-KROOT signature verifies (checked here again with the same public key); the whole
//!    session is then verified from its first page with that chain trusted, so data and tags
//!    received before the chain was authenticated are checked against it (no depth limit).
//! 3. **Check the bits as received**: for an ADKD 0 or 12 tag the open verifier could not
//!    check because the sub-frame's Word Types 1 to 4 carry different IODnav values (it leaves it
//!    `pending: no_nav_data`), the 549 ADKD 0 bits are assembled from the words as received and
//!    the tag is checked exactly as the open verifier checks one (the same message, the same MAC,
//!    each of the COP sub-frames in turn); no match on data that was there is a failure, blamed as
//!    the open verifier blames one.
//!
//! The readout is E4's: a satellite's status at the end of the stream. MODELLED, protocol level.

use std::collections::{BTreeMap, BTreeSet};

use kshana::osnma::bits::{read_bits, BitWriter};
use kshana::osnma::dsm::{DsmAssembler, DsmKroot};
use kshana::osnma::mac;
use kshana::osnma::page::InavPage;
use kshana::osnma::signature::{self, PublicKey};
use kshana::osnma::subframe::{MackLayout, Subframe, SubframeAssembler, SUBFRAME_S};
use kshana::osnma::tesla;
use kshana::osnma::verifier::{Chain, Config, Event, PendingReason, TagStatus, Verifier};
use kshana::osnma::{gst_pack, OsnmaStatus};
use serde::Serialize;

/// Which of the three policies run (all three for the method; subsets for the development
/// table only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Policies {
    pub session_memory: bool,
    pub hold_until_chain: bool,
    pub bits_as_received: bool,
}

impl Policies {
    /// Method v2: all three.
    pub const V2: Policies = Policies {
        session_memory: true,
        hold_until_chain: true,
        bits_as_received: true,
    };
    /// The open verifier as E4 used it.
    pub const NONE: Policies = Policies {
        session_memory: false,
        hold_until_chain: false,
        bits_as_received: false,
    };
}

/// What one verification of a session gave.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionOutcome {
    /// Satellites `failed` at the end of the stream.
    pub failed: BTreeSet<u8>,
    /// Satellites `authenticated` at the end of the stream.
    pub authenticated: BTreeSet<u8>,
    /// Sub-frame time of the first verified DSM-KROOT, when one verified.
    pub chain_verified_gst: Option<u32>,
    /// Tags the open verifier left pending for want of consistent data and policy 3 checked:
    /// (matched, failed).
    pub as_received_checked: (usize, usize),
}

/// The ADKD 0 pieces (word type, first bit, length), as the open engine's `navdata`.
const ADKD0_PIECES: [(u8, usize, usize); 5] = [
    (1, 6, 120),
    (2, 6, 120),
    (3, 6, 122),
    (4, 6, 120),
    (5, 6, 67),
];
const ADKD0_BITS: usize = 549;
const SLOW_MAC_DELAY: u32 = 10;

fn word_type(w: &[u8; 16]) -> u8 {
    (w[0] >> 2) & 0x3F
}

/// The ADKD 0 bits of a sub-frame's words as received (Word Types 1 to 5 present, IODnav not
/// required to agree).
fn adkd0_as_received(words: &[[u8; 16]]) -> Option<(Vec<u8>, usize)> {
    let mut out = BitWriter::new();
    for &(wt, off, len) in &ADKD0_PIECES {
        let w = words.iter().find(|w| word_type(w) == wt)?;
        let mut done = 0;
        while done < len {
            let n = (len - done).min(32);
            out.push(read_bits(w, off + done, n)?, n);
            done += n;
        }
    }
    Some((out.into_bytes(), ADKD0_BITS))
}

/// The first chain whose DSM-KROOT signature verifies with one of `keys`, and the sub-frame
/// time at which it was complete.
fn first_chain(subframes: &[Subframe], keys: &[PublicKey]) -> Option<(Chain, u32)> {
    let mut dsm = DsmAssembler::new();
    let mut nma: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    for sf in subframes.iter().filter(|s| s.has_osnma) {
        let dh = sf.dsm_header();
        let seen = nma.entry(dh.dsm_id).or_default();
        if !seen.contains(&sf.hkroot[0]) && seen.len() < 4 {
            seen.push(sf.hkroot[0]);
        }
        let Some(d) = dsm.push(dh.dsm_id, dh.block_id, sf.dsm_block()) else {
            continue;
        };
        let headers = nma.remove(&d.dsm_id).unwrap_or_default();
        if d.dsm_id >= 12 {
            continue;
        }
        let pkid = d.bytes[0] & 0x0F;
        let Some(pk) = keys.iter().find(|k| k.pkid == pkid) else {
            continue;
        };
        let Ok(k) = DsmKroot::parse(&d.bytes, pk.key_type.signature_bits()) else {
            continue;
        };
        let ok = headers
            .iter()
            .filter(|n| k.padding_matches(**n) != Some(false))
            .any(|n| signature::verify(pk, &k.signed_message(*n), &k.signature).is_ok());
        if ok {
            return Some((Chain::from_kroot(&k), sf.gst_sf));
        }
    }
    None
}

/// Verify one session (pages in time order) under `pol`.
pub fn verify_session(
    pages: &[InavPage],
    keys: &[PublicKey],
    merkle_root: Option<[u8; 32]>,
    pol: Policies,
) -> SessionOutcome {
    let mut asm = SubframeAssembler::new();
    let subframes: Vec<Subframe> = pages.iter().filter_map(|p| asm.push(p)).collect();
    let found = first_chain(&subframes, keys);
    let trusted = if pol.hold_until_chain {
        found.as_ref().map(|c| c.0.clone())
    } else {
        None
    };
    let mut v = Verifier::new(Config {
        merkle_root,
        public_keys: keys.to_vec(),
        trusted_chain: trusted.clone(),
        failure_memory_s: pol.session_memory.then_some(u32::MAX),
        ..Config::default()
    });
    let mut pending_tags = Vec::new();
    let mut key_ok: BTreeSet<u32> = BTreeSet::new();
    for p in pages {
        for e in v.push_page(p) {
            match e {
                Event::Tag(t)
                    if t.status == TagStatus::Pending(PendingReason::NoNavData)
                        && matches!(t.adkd, 0 | 12) =>
                {
                    pending_tags.push(t)
                }
                Event::KeyVerified { gst } => {
                    key_ok.insert(gst);
                }
                _ => {}
            }
        }
    }
    let mut failed: BTreeSet<u8> = BTreeSet::new();
    let mut authenticated: BTreeSet<u8> = BTreeSet::new();
    for (s, st) in v.sat_status() {
        let prn: u8 = s[1..].parse().unwrap_or(0);
        match st {
            OsnmaStatus::Failed => {
                failed.insert(prn);
            }
            OsnmaStatus::Authenticated => {
                authenticated.insert(prn);
            }
            _ => {}
        }
    }
    let mut checked = (0usize, 0usize);
    let chain = trusted.or_else(|| found.as_ref().map(|c| c.0.clone()));
    if pol.bits_as_received {
        if let Some(chain) = chain {
            let by: BTreeMap<(u8, u32), &Subframe> =
                subframes.iter().map(|s| ((s.svid, s.gst_sf), s)).collect();
            let layout = MackLayout {
                key_bits: chain.key_bits,
                tag_bits: chain.tag_bits,
            };
            // A verified key of a slot, read from any sub-frame of that slot that carries it.
            let key_at = |slot: u32| -> Option<Vec<u8>> {
                if !key_ok.contains(&slot) {
                    return None;
                }
                let anchor = chain.gst0.checked_sub(SUBFRAME_S)?;
                subframes
                    .iter()
                    .filter(|s| s.gst_sf == slot && s.has_osnma)
                    .filter_map(|s| layout.parse(&s.mack).map(|m| m.key))
                    .find(|k| {
                        tesla::verify_key(
                            chain.hash,
                            k,
                            slot,
                            &chain.root_key,
                            anchor,
                            &chain.alpha,
                        )
                        .is_ok()
                    })
            };
            for t in &pending_tags {
                let Some(sf) = by.get(&(t.prna, t.tag_gst)) else {
                    continue;
                };
                let Some(mack) = layout.parse(&sf.mack) else {
                    continue;
                };
                let (tag, cop) = if t.ctr == 1 {
                    (mack.tag0, mack.cop0)
                } else {
                    match mack.tags.get(usize::from(t.ctr) - 2) {
                        Some(x) => (x.tag, x.cop),
                        None => continue,
                    }
                };
                let slot = if t.adkd == 12 {
                    t.tag_gst + SUBFRAME_S * (1 + SLOW_MAC_DELAY)
                } else {
                    t.tag_gst + SUBFRAME_S
                };
                let Some(key) = key_at(slot) else { continue };
                let mut saw = false;
                let mut matched = false;
                for k in 1..=u32::from(cop) {
                    let Some(g) = t.tag_gst.checked_sub(SUBFRAME_S * k) else {
                        break;
                    };
                    let Some(dsf) = by.get(&(t.prnd, g)) else {
                        continue;
                    };
                    let Some((nav, bits)) = adkd0_as_received(&dsf.words) else {
                        continue;
                    };
                    saw = true;
                    let mut m = BitWriter::new();
                    if t.ctr != 1 {
                        m.push(u64::from(t.prnd), 8);
                    }
                    m.push(u64::from(t.prna), 8);
                    m.push(u64::from(gst_pack(t.tag_gst)), 32);
                    m.push(u64::from(t.ctr), 8);
                    m.push(u64::from(t.nmas), 2);
                    for chunk in 0..bits.div_ceil(32) {
                        let n = (bits - chunk * 32).min(32);
                        m.push(read_bits(&nav, chunk * 32, n).unwrap_or_default(), n);
                    }
                    let Ok(full) = mac::compute(chain.mac, &key, &m.into_bytes()) else {
                        saw = false;
                        break;
                    };
                    if read_bits(&full, 0, chain.tag_bits) == Some(tag) {
                        matched = true;
                        break;
                    }
                }
                if matched {
                    checked.0 += 1;
                } else if saw {
                    checked.1 += 1;
                    failed.insert(t.prnd);
                    failed.insert(t.prna);
                    authenticated.remove(&t.prnd);
                    authenticated.remove(&t.prna);
                }
            }
        }
    }
    SessionOutcome {
        failed,
        authenticated,
        chain_verified_gst: found.map(|c| c.1),
        as_received_checked: checked,
    }
}

/// One flip campaign on a session: E4's eligibility rule and draws (ChaCha8 `seed`; ADKD 0
/// bits of Word Types 1 to 5; the CRC recomputed; a sub-frame followed by at least two more of
/// the same satellite), read out with [`verify_session`] under `pol`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Campaign {
    pub pages: usize,
    pub eligible_pages: usize,
    pub clean_failed: BTreeSet<u8>,
    pub clean_authenticated: BTreeSet<u8>,
    pub clean_chain_verified_gst: Option<u32>,
    pub flips: usize,
    pub detected: usize,
    pub detection_share: f64,
    /// (svid, GST, word type, word bit, sub-frames before the stream end of that satellite).
    pub undetected: Vec<(u8, u32, u8, usize, u32)>,
}

/// Run a flip campaign.
pub fn campaign(
    pages: &[InavPage],
    keys: &[PublicKey],
    merkle_root: Option<[u8; 32]>,
    flips: usize,
    seed: u64,
    pol: Policies,
) -> Result<Campaign, String> {
    use rand::{Rng, SeedableRng};
    let clean = verify_session(pages, keys, merkle_root, pol);
    let wt = |p: &InavPage| (p.data_word()[0] >> 2) & 0x3F;
    let sf = |p: &InavPage| p.gst / 30;
    let mut last_sf: BTreeMap<u8, u32> = BTreeMap::new();
    for p in pages {
        let e = last_sf.entry(p.svid).or_insert(0);
        *e = (*e).max(sf(p));
    }
    let eligible: Vec<usize> = (0..pages.len())
        .filter(|&k| {
            let p = &pages[k];
            super::ADKD0_PIECES.iter().any(|x| x.0 == wt(p))
                && sf(p) + 2 <= last_sf.get(&p.svid).copied().unwrap_or(0)
        })
        .collect();
    if eligible.is_empty() {
        return Err("no page is eligible for tampering".into());
    }
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let mut detected = 0usize;
    let mut undetected = Vec::new();
    for _ in 0..flips {
        let k = eligible[rng.gen_range(0..eligible.len())];
        let p = &pages[k];
        let w = wt(p);
        let (_, first, len) = super::ADKD0_PIECES
            .iter()
            .copied()
            .find(|x| x.0 == w)
            .expect("eligible");
        let wb = first + rng.gen_range(0..len);
        let pb = super::page_bit_of_word_bit(wb);
        let mut bytes = [0u8; 30];
        bytes.copy_from_slice(p.bytes());
        bytes[pb / 8] ^= 1 << (7 - pb % 8);
        let mut t = pages.to_vec();
        t[k] = InavPage::from_bytes(p.svid, p.gst, &bytes).with_valid_crc();
        let r = verify_session(&t, keys, merkle_root, pol);
        if r.failed.contains(&p.svid) {
            detected += 1;
        } else {
            let before = last_sf.get(&p.svid).copied().unwrap_or(0) - sf(p);
            undetected.push((p.svid, p.gst, w, wb, before));
        }
    }
    Ok(Campaign {
        pages: pages.len(),
        eligible_pages: eligible.len(),
        clean_failed: clean.failed,
        clean_authenticated: clean.authenticated,
        clean_chain_verified_gst: clean.chain_verified_gst,
        flips,
        detected,
        detection_share: if flips == 0 {
            0.0
        } else {
            detected as f64 / flips as f64
        },
        undetected,
    })
}
