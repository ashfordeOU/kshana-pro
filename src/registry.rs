// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The Pro pack registry and the **superset dispatcher**.
//!
//! Dispatch is the concrete proof of the superset model: a `pro-*` scenario runs
//! through a Pro [`ExternalPack`]; anything else is delegated **unchanged** to the
//! open engine via [`kshana::api::run_scenario`]. So a single `kshana-pro` binary
//! runs every open scenario *and* every Pro pack — open ∪ proprietary.

use kshana::api::{run_scenario, ExternalPack, KshanaError, RunOutput, ScenarioMeta};

use crate::packs::batch_pack::BatchScenario;
use crate::packs::beacon_dop_pack::{BeaconDopScenario, SurfaceBeaconDopStudyScenario};
use crate::packs::lunar_signal_security_pack::LunarSignalSecurityScenario;
use crate::packs::mbse_pack::MbseScenario;
use crate::packs::optimism_gap_audit::OptimismGapAuditScenario;
use crate::packs::quantum_systematics_pack::QuantumSystematicsScenario;
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
        BatchScenario::KIND => Some(parse::<BatchScenario>(kind, src)),
        QuantumSystematicsScenario::KIND => Some(parse::<QuantumSystematicsScenario>(kind, src)),
        MbseScenario::KIND => Some(parse::<MbseScenario>(kind, src)),
        OptimismGapAuditScenario::KIND => Some(parse::<OptimismGapAuditScenario>(kind, src)),
        LunarSignalSecurityScenario::KIND => {
            Some(parse::<LunarSignalSecurityScenario>(kind, src))
        }
        BeaconDopScenario::KIND => Some(parse::<BeaconDopScenario>(kind, src)),
        SurfaceBeaconDopStudyScenario::KIND => {
            Some(parse::<SurfaceBeaconDopStudyScenario>(kind, src))
        }
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
        BatchScenario::meta_static(),
        QuantumSystematicsScenario::meta_static(),
        MbseScenario::meta_static(),
        OptimismGapAuditScenario::meta_static(),
        LunarSignalSecurityScenario::meta_static(),
        BeaconDopScenario::meta_static(),
        SurfaceBeaconDopStudyScenario::meta_static(),
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

/// Licensed dispatch: enforce a Pro [`Entitlement`] before running a `pro-*` kind.
/// Open scenarios are always free (Pro embeds the open AGPL-3.0 engine). A denied Pro
/// kind fails closed with an honest, actionable error naming the required tier.
pub fn run_gated(
    src: &str,
    entitlement: &crate::license::Entitlement,
    now_unix: i64,
) -> Result<RunOutput, KshanaError> {
    if let Some(kind) = peek_kind(src) {
        if kind.starts_with(PRO_KIND_PREFIX) && !entitlement.allows(&kind, now_unix) {
            // Do NOT echo the licensee name here — this message can reach an HTTP
            // 403 body seen by an unauthorised caller.
            return Err(KshanaError::Unsupported(format!(
                "'{kind}' requires the '{}' tier; current license tier is '{}'{}",
                crate::license::feature_min_tier(&kind).as_str(),
                entitlement.tier.as_str(),
                if entitlement.is_expired(now_unix) {
                    " (and it has expired)"
                } else {
                    ""
                },
            )));
        }
    }
    run(src)
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

    #[test]
    fn run_gated_denies_above_tier_feature_without_running_it() {
        use crate::license::{Entitlement, Tier};
        let pro = Entitlement {
            licensee: "ACME".into(),
            tier: Tier::Pro,
            expires_unix: 0,
            features: vec![],
        };
        // pro-batch is Enterprise-only → denied for a Pro license, fails closed
        // BEFORE any (expensive) run.
        let err = run_gated("kind = \"pro-batch\"\n", &pro, 0).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("requires the 'enterprise' tier"), "got: {msg}");
        assert!(msg.contains("tier is 'pro'"), "got: {msg}");
        assert!(
            !msg.contains("licensee"),
            "must not leak the licensee name: {msg}"
        );
    }

    #[test]
    fn run_gated_allows_entitled_feature_and_open_scenarios() {
        use crate::license::Entitlement;
        // Eval tier: the free seam-check is allowed and runs.
        let eval = Entitlement::evaluation();
        assert!(run_gated("kind = \"pro-seam-check\"\n", &eval, 0).is_ok());
        // Open scenarios are always free, even on Eval.
        let ephem = "kind = \"ephemeris\"\n\
            tle = \"\"\"\n\
            1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
            2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
            \"\"\"\n\
            step_s = 60.0\n\
            duration_s = 120.0\n";
        assert!(run_gated(ephem, &eval, 0).is_ok());
        // But a value pack is denied on Eval.
        assert!(run_gated("kind = \"pro-twin\"\n", &eval, 0).is_err());
    }
}
