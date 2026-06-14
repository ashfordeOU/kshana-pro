// PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE.
//
//! The Pro pack registry and the **superset dispatcher**.
//!
//! Dispatch is the concrete proof of the superset model: a `pro-*` scenario runs
//! through a Pro [`ExternalPack`]; anything else is delegated **unchanged** to the
//! open engine via [`kshana::api::run_scenario`]. So a single `kshana-pro` binary
//! runs every open scenario *and* every Pro pack — open ∪ proprietary.

use kshana::api::{run_scenario, ExternalPack, KshanaError, RunOutput, ScenarioMeta};

use crate::packs::seam_check::SeamCheckPack;

/// Prefix every Pro-only scenario kind carries. The dispatcher routes on it, and
/// it makes Pro kinds visually unmistakable in a scenario file and in logs.
pub const PRO_KIND_PREFIX: &str = "pro-";

/// Minimal TOML peek: read the top-level `kind` field without committing to any
/// pack's full schema. Mirrors how the open engine classifies (`api::classify`),
/// but here it only decides *open vs Pro routing*.
pub fn peek_kind(src: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Kind {
        kind: Option<String>,
    }
    toml::from_str::<Kind>(src).ok().and_then(|k| k.kind)
}

/// Is this scenario answered by a Pro pack (rather than the open engine)?
pub fn is_pro_kind(kind: &str) -> bool {
    kind.starts_with(PRO_KIND_PREFIX) && pro_kinds().iter().any(|m| m.name == kind)
}

/// Construct the registered Pro pack for a kind, deserializing its fields from the
/// scenario TOML. Returns `None` if no Pro pack answers to `kind`.
fn build_pro_pack(kind: &str, src: &str) -> Option<Result<Box<dyn ExternalPack>, KshanaError>> {
    match kind {
        SeamCheckPack::KIND => Some(
            toml::from_str::<SeamCheckPack>(src)
                .map(|p| Box::new(p) as Box<dyn ExternalPack>)
                .map_err(|e| KshanaError::InvalidInput(format!("invalid {kind} scenario: {e}"))),
        ),
        _ => None,
    }
}

/// Introspection metadata for every registered Pro pack — the Pro counterpart to
/// [`kshana::api::list_scenario_kinds`]. The union of the two is the full set of
/// kinds a Pro build can run.
pub fn pro_kinds() -> Vec<ScenarioMeta> {
    vec![SeamCheckPack::meta_static()]
}

/// The superset dispatcher. Route `pro-*` kinds to their Pro pack; delegate every
/// other scenario unchanged to the open engine. This is the single entry point a
/// Pro caller (CLI, future binding) uses.
pub fn run(src: &str) -> Result<RunOutput, KshanaError> {
    match peek_kind(src) {
        Some(kind) if is_pro_kind(&kind) => match build_pro_pack(&kind, src) {
            Some(Ok(pack)) => pack.run(),
            Some(Err(e)) => Err(e),
            // is_pro_kind said yes but no constructor matched — registry desync.
            None => Err(KshanaError::Unsupported(format!(
                "Pro kind '{kind}' is registered but has no constructor"
            ))),
        },
        // Open scenario (or no/unknown kind) → the open engine, unchanged.
        _ => run_scenario(src),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_kind_reads_top_level_kind() {
        assert_eq!(
            peek_kind("kind = \"pro-seam-check\"\n").as_deref(),
            Some("pro-seam-check")
        );
        assert_eq!(
            peek_kind("kind = \"ephemeris\"\n").as_deref(),
            Some("ephemeris")
        );
        assert_eq!(peek_kind("threshold_ns = 50\n"), None);
    }

    #[test]
    fn is_pro_kind_only_for_registered_pro_packs() {
        assert!(is_pro_kind("pro-seam-check"));
        assert!(!is_pro_kind("pro-not-real")); // pro- prefix but unregistered
        assert!(!is_pro_kind("ephemeris")); // open kind
    }

    #[test]
    fn pro_kinds_are_all_pro_prefixed() {
        for m in pro_kinds() {
            assert!(
                m.name.starts_with(PRO_KIND_PREFIX),
                "Pro pack '{}' must carry the {PRO_KIND_PREFIX} prefix",
                m.name
            );
        }
    }
}
