// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! # Registration hook for a server-side extension (roadmap 0.42, X3)
//!
//! Kinds and commands that do not ship in Kshana Pro live in a separate, private crate
//! that depends on this one (never the reverse). That crate's server binary calls
//! [`register`] once, before anything else runs, and then drives this crate's own
//! command line ([`crate::cli::main_with_args`]) or service. From then on:
//!
//! * the dispatcher ([`crate::registry::run`], [`crate::registry::run_gated`]) builds and
//!   runs the extension's kinds, and [`crate::registry::pro_kinds`] and
//!   [`crate::registry::pro_registry`] list and register them;
//! * the command line runs the extension's commands and prints their usage lines;
//! * the keyring trusts the extension's issuer keys for the hidden-tier claim.
//!
//! Registration does not unlock anything. Every extension kind and command feature is
//! hidden-tier: the signed tier manifest must already list it, by hash, at a hidden tier,
//! or [`crate::license::tiers::startup_check`] fails and the kind is refused. Running one
//! still goes through [`crate::license::check`], which needs a verified token carrying the
//! claim. A build that registers nothing (every shipped Kshana Pro build) has none of
//! this code and refuses the hidden kinds by hash with a neutral message.

use std::process::ExitCode;
use std::sync::OnceLock;

use kshana::api::{ExternalPack, KshanaError, ScenarioMeta};
use kshana::registry::PackRegistry;

use crate::license::{Tier, KEY_LEN};

/// Build an extension pack from its scenario TOML.
pub type BuildFn = fn(&str) -> Result<Box<dyn ExternalPack>, KshanaError>;

/// One scenario kind added by an extension.
#[derive(Clone)]
pub struct ExtensionKind {
    /// The kind's metadata (its `name` carries the `pro-` prefix).
    pub meta: ScenarioMeta,
    /// The licence tier recorded for the kind (informational: the edition gate decides).
    pub min_tier: Tier,
    /// Parse the scenario TOML into the pack.
    pub build: BuildFn,
    /// Add the kind's factory to a registry (the pack's `ExternalPack::register_into`).
    pub register_into: fn(&mut PackRegistry),
}

/// One command feature added by an extension (gated by a command, not a scenario kind).
#[derive(Clone, Copy)]
pub struct ExtensionFeature {
    pub id: &'static str,
    /// The licence tier recorded for the feature (informational).
    pub min_tier: Tier,
}

/// One command added by an extension to the command line.
#[derive(Clone, Copy)]
pub struct ExtensionCommand {
    /// The first argument that selects it.
    pub name: &'static str,
    /// Its usage line(s), printed with the command line's usage.
    pub usage: &'static str,
    /// Run it with the full argument list (the command name first).
    pub run: fn(&[String]) -> ExitCode,
}

/// Everything one extension adds.
#[derive(Clone, Default)]
pub struct Extension {
    /// A short name, for error messages.
    pub name: &'static str,
    pub kinds: Vec<ExtensionKind>,
    pub features: Vec<ExtensionFeature>,
    pub commands: Vec<ExtensionCommand>,
    /// Issuer public keys trusted for the hidden-tier claim (`Keyring::add_arksa_issuer`).
    pub issuer_keys: Vec<[u8; KEY_LEN]>,
}

static EXTENSION: OnceLock<Extension> = OnceLock::new();

/// Register the extension. Call it once, first thing in the server binary. A second call
/// is refused. A kind or feature id that is not `pro-` prefixed, that collides with a Pro
/// kind, feature or command, or that is listed twice is refused.
pub fn register(ext: Extension) -> Result<(), KshanaError> {
    let bad = |m: String| KshanaError::InvalidInput(format!("extension {}: {m}", ext.name));
    let own_kinds: Vec<&str> = crate::registry::builtin_pro_kinds()
        .iter()
        .map(|m| m.name)
        .collect();
    // A kind and a command feature may share an id (one entitlement covers both); the
    // same id twice among the kinds, or twice among the features, is a mistake.
    let kind_ids: Vec<&str> = ext.kinds.iter().map(|k| k.meta.name).collect();
    let feature_ids: Vec<&str> = ext.features.iter().map(|f| f.id).collect();
    for ids in [&kind_ids, &feature_ids] {
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err(bad("an id is listed twice".into()));
        }
    }
    for id in kind_ids.iter().chain(&feature_ids) {
        if !id.starts_with(crate::registry::PRO_KIND_PREFIX) {
            return Err(bad("an id lacks the pro- prefix".into()));
        }
        if own_kinds.contains(id) || crate::license::PRO_COMMAND_FEATURES.contains(id) {
            return Err(bad("an id collides with a Pro kind or feature".into()));
        }
    }
    for c in &ext.commands {
        if crate::cli::BUILTIN_COMMANDS.contains(&c.name) {
            return Err(bad(format!(
                "command '{}' collides with a Pro command",
                c.name
            )));
        }
    }
    EXTENSION
        .set(ext)
        .map_err(|_| KshanaError::InvalidInput("an extension is already registered".into()))
}

/// The registered extension, if any.
pub fn get() -> Option<&'static Extension> {
    EXTENSION.get()
}

/// The registered kinds (empty without an extension).
pub fn kinds() -> &'static [ExtensionKind] {
    get().map(|e| e.kinds.as_slice()).unwrap_or(&[])
}

/// The registered command features (empty without an extension).
pub fn features() -> &'static [ExtensionFeature] {
    get().map(|e| e.features.as_slice()).unwrap_or(&[])
}

/// The registered commands (empty without an extension).
pub fn commands() -> &'static [ExtensionCommand] {
    get().map(|e| e.commands.as_slice()).unwrap_or(&[])
}

/// The registered issuer keys (empty without an extension).
pub fn issuer_keys() -> Vec<[u8; KEY_LEN]> {
    get().map(|e| e.issuer_keys.clone()).unwrap_or_default()
}

/// Whether `id` is a kind or feature of the registered extension.
pub fn owns(id: &str) -> bool {
    kinds().iter().any(|k| k.meta.name == id) || features().iter().any(|f| f.id == id)
}

/// The recorded licence tier of an extension kind or feature.
pub fn min_tier(id: &str) -> Option<Tier> {
    kinds()
        .iter()
        .find(|k| k.meta.name == id)
        .map(|k| k.min_tier)
        .or_else(|| features().iter().find(|f| f.id == id).map(|f| f.min_tier))
}

/// Build an extension pack for `kind`, or `None` when the extension has no such kind.
pub fn build(kind: &str, src: &str) -> Option<Result<Box<dyn ExternalPack>, KshanaError>> {
    kinds()
        .iter()
        .find(|k| k.meta.name == kind)
        .map(|k| (k.build)(src))
}

/// Parse a pack's scenario TOML into a boxed [`ExternalPack`] with the dispatcher's error
/// text. An extension's [`ExtensionKind::build`] is usually `parse_pack::<ItsScenario>`.
pub fn parse_pack<P>(src: &str) -> Result<Box<dyn ExternalPack>, KshanaError>
where
    P: ExternalPack + serde::de::DeserializeOwned + 'static,
{
    toml::from_str::<P>(src)
        .map(|p| Box::new(p) as Box<dyn ExternalPack>)
        .map_err(|e| KshanaError::InvalidInput(format!("invalid scenario: {e}")))
}
