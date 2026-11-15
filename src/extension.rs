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
        if own_kinds.contains(id)
            || crate::license::PRO_COMMAND_FEATURES.contains(id)
            || crate::license::tiers::PACK_DELIVERED_KINDS.contains(id)
        {
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

/// The recorded licence tier of an extension kind or feature (the server-side extension
/// first, then a loaded Pro pack).
pub fn min_tier(id: &str) -> Option<Tier> {
    kinds()
        .iter()
        .find(|k| k.meta.name == id)
        .map(|k| k.min_tier)
        .or_else(|| features().iter().find(|f| f.id == id).map(|f| f.min_tier))
        .or_else(|| {
            pack_kinds()
                .into_iter()
                .find(|k| k.meta.name == id)
                .map(|k| k.min_tier)
        })
        .or_else(|| {
            pack_features()
                .into_iter()
                .find(|f| f.id == id)
                .map(|f| f.min_tier)
        })
}

/// Build an extension pack for `kind` (the server-side extension, then a loaded Pro pack),
/// or `None` when neither has such a kind.
pub fn build(kind: &str, src: &str) -> Option<Result<Box<dyn ExternalPack>, KshanaError>> {
    kinds()
        .iter()
        .find(|k| k.meta.name == kind)
        .or_else(|| pack_kinds().into_iter().find(|k| k.meta.name == kind))
        .map(|k| (k.build)(src))
}

// ─── Pro packs (roadmap 0.42 C10) ──────────────────────────────────────────────────────
//
// A Pro pack is downloaded by the free Desktop app after purchase, verified and decrypted
// by [`crate::propack::loader`], and then registered here. Its kinds and features are at
// the `pro` edition: they run only under a licence whose tier unlocks them
// ([`crate::license::check`]), like the Pro kinds compiled into this crate. A pack can
// never add a kind at a hidden tier: an id the signed tier manifest lists at any tier
// other than `pro`, a built-in Pro kind or feature, or a kind of the server-side extension
// is refused.

/// What one verified Pro pack adds.
#[derive(Clone, Default)]
pub struct PackExtension {
    /// The pack's id (from its signed manifest).
    pub pack_id: String,
    pub kinds: Vec<ExtensionKind>,
    pub features: Vec<ExtensionFeature>,
}

static PACKS: std::sync::Mutex<Vec<&'static PackExtension>> = std::sync::Mutex::new(Vec::new());

fn packs() -> Vec<&'static PackExtension> {
    PACKS.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// The kinds of every loaded Pro pack.
pub fn pack_kinds() -> Vec<&'static ExtensionKind> {
    packs().into_iter().flat_map(|p| p.kinds.iter()).collect()
}

/// The command features of every loaded Pro pack.
pub fn pack_features() -> Vec<&'static ExtensionFeature> {
    packs()
        .into_iter()
        .flat_map(|p| p.features.iter())
        .collect()
}

/// Whether `id` is a kind or feature of a loaded Pro pack.
pub fn pack_owns(id: &str) -> bool {
    pack_kinds().iter().any(|k| k.meta.name == id) || pack_features().iter().any(|f| f.id == id)
}

/// Register a verified Pro pack. Only [`crate::propack::loader`] can call this: `proof`
/// is made there, after the signature, the device binding, the licence state and the
/// payload hash have all been checked, and it names the kinds and features the signed
/// pack manifest says the pack provides. Refused: an id the manifest does not list, an id
/// without the `pro-` prefix, an id that collides with a built-in Pro kind or feature, a
/// server-side extension id or an already loaded pack, an id the tier manifest lists at
/// any tier other than `pro`, and a pack id loaded twice.
pub fn register_pack(
    pack: PackExtension,
    proof: &crate::propack::loader::VerifiedPack,
) -> Result<(), KshanaError> {
    let bad = |m: String| KshanaError::InvalidInput(format!("pack {}: {m}", pack.pack_id));
    if pack.pack_id != proof.pack_id() {
        return Err(bad("the pack id does not match the verified pack".into()));
    }
    let builtin: Vec<&str> = crate::registry::builtin_pro_kinds()
        .iter()
        .map(|m| m.name)
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let ids = pack
        .kinds
        .iter()
        .map(|k| k.meta.name)
        .chain(pack.features.iter().map(|f| f.id));
    for id in ids {
        if !proof.provides(id) {
            return Err(bad(format!("'{id}' is not in the signed pack manifest")));
        }
        if !id.starts_with(crate::registry::PRO_KIND_PREFIX) {
            return Err(bad("an id lacks the pro- prefix".into()));
        }
        if builtin.contains(&id)
            || crate::license::PRO_COMMAND_FEATURES.contains(&id)
            || owns(id)
            || pack_owns(id)
        {
            return Err(bad(format!("'{id}' collides with a kind already present")));
        }
        match crate::license::tiers::edition_of(id) {
            None | Some(crate::license::tiers::Edition::Pro) => {}
            Some(_) => return Err(bad("an id is not at the pro edition".into())),
        }
        if !seen.insert(id) {
            return Err(bad(format!("'{id}' is listed twice")));
        }
    }
    let mut all = PACKS.lock().unwrap_or_else(|p| p.into_inner());
    if all.iter().any(|p| p.pack_id == pack.pack_id) {
        return Err(bad("already loaded".into()));
    }
    all.push(Box::leak(Box::new(pack)));
    Ok(())
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
