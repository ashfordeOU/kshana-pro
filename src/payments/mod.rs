// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Payments and paywall (roadmap F10g), off by default
//!
//! A merchant of record (Paddle or Lemon Squeezy) takes the payment and the tax; its
//! signed webhook reaches this service, which issues the licence token through the
//! issuance register (F10d), renews it on each subscription payment and revokes it on
//! a refund, a chargeback or the end of a subscription, rewriting the signed revocation
//! list the apps receive with their next update (F10c).
//!
//! **One switch**, `enabled` in `payments.toml`, false by default
//! (`commercial/payments.toml`). Off: the webhook service refuses to start, every
//! delivery is answered 404, and the paywall returns [`paywall::Paywall::Off`] so no
//! checkout and no price is shown. This module makes no outbound network call in any
//! state; the test `payments_code_makes_no_outbound_connection` checks the sources.
//!
//! **Prices** stay `PLACEHOLDER` until the owner sets them; live mode is refused while
//! any price or provider ID is `PLACEHOLDER` (owner gate 12: provider, prices, bank
//! account, VAT confirmation).
//!
//! **Vendor side only.** The webhook service holds a signing key, so it runs at the
//! vendor, never in a customer build or service (`docs/PAYMENTS.md`).

pub mod config;
pub mod paywall;
pub mod provider;
pub mod server;
pub mod webhook;

pub use config::{PaymentsConfig, Provider, PLACEHOLDER};
