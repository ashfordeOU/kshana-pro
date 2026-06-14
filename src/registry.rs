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
use crate::packs::trade_study_pack::TradeStudyScenario;
use crate::packs::twin_pack::TwinScenario;

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
    // Helper: parse a pack's TOML into a boxed ExternalPack with a uniform error.
    fn parse<P>(kind: &str, src: &str) -> Result<Box<dyn ExternalPack>, KshanaError>
    where
        P: ExternalPack + serde::de::DeserializeOwned + 'static,
    {
        toml::from_str::<P>(src)
            .map(|p| Box::new(p) as Box<dyn ExternalPack>)
            .map_err(|e| KshanaError::InvalidInput(format!("invalid {kind} scenario: {e}")))
    }
    match kind {
        SeamCheckPack::KIND => Some(parse::<SeamCheckPack>(kind, src)),
        TwinScenario::KIND => Some(parse::<TwinScenario>(kind, src)),
        TradeStudyScenario::KIND => Some(parse::<TradeStudyScenario>(kind, src)),
        _ => None,
    }
}

/// Introspection metadata for every registered Pro pack — the Pro counterpart to
/// [`kshana::api::list_scenario_kinds`]. The union of the two is the full set of
/// kinds a Pro build can run.
pub fn pro_kinds() -> Vec<ScenarioMeta> {
    vec![
        SeamCheckPack::meta_static(),
        TwinScenario::meta_static(),
        TradeStudyScenario::meta_static(),
    ]
}

/// The superset dispatcher. Route `pro-*` kinds to their Pro pack; delegate every
/// other scenario unchanged to the open engine. This is the single entry point a
/// Pro caller (CLI, future binding) uses.
pub fn run(src: &str) -> Result<RunOutput, KshanaError> {
    match peek_kind(src) {
        // Any `pro-` prefixed kind is OWNED by the overlay: build + run it, or fail
        // closed with an honest error. It must NEVER silently fall through to the
        // open engine (whose classifier maps an unknown kind to its clock default)
        // — that would let a mistyped or not-yet-registered Pro kind (e.g. a future
        // `pro-twin` in a scenario file before the registry is updated) appear to
        // succeed as a clock run.
        Some(kind) if kind.starts_with(PRO_KIND_PREFIX) => match build_pro_pack(&kind, src) {
            Some(Ok(pack)) => pack.run(),
            Some(Err(e)) => Err(e),
            None => Err(KshanaError::Unsupported(format!(
                "'{kind}' carries the {PRO_KIND_PREFIX} prefix but is not a registered Pro pack"
            ))),
        },
        // Open scenario (or no/unknown non-Pro kind) → the open engine, unchanged.
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
