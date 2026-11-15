// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Pro pack delivery (roadmap 0.42 C10, owner decision 10, 15 November 2026)
//!
//! The free, notarised Kshana Desktop app downloads the Pro pack from our hosting after
//! purchase. A pack is signed by our pack key, encrypted for one licence and one device,
//! and watermarked for its licensee; the same format serves macOS, Windows and Linux.
//!
//! * [`format`]: the file format, the keys, sealing and opening, the watermark.
//! * [`issuer`]: the vendor side: the catalogue of built payloads, and issuing a pack for
//!   a valid licence token and device.
//! * [`server`]: the HTTP endpoints of the licence and pack service (activate a seat,
//!   the automatic licence check, the revocation list, fetch a pack).
//! * [`loader`]: the app side: install, verify, decrypt, load through the extension
//!   hook ([`crate::extension::register_pack`]), the expiry notices, the lock at expiry
//!   and the deletion at the end of the 30-day renewal window.
//!
//! Docs: `docs/PRO-PACKS.md`.

pub mod format;
pub mod issuer;
pub mod loader;
pub mod server;
