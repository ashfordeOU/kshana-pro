// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! The Pro pack registry and the **superset dispatcher**.
//!
//! Dispatch is the concrete proof of the superset model: a `pro-*` scenario runs
//! through a Pro [`ExternalPack`]; anything else is delegated **unchanged** to the
//! open engine via [`kshana::api::run_scenario`]. So a single `kshana-pro` binary
//! runs every open scenario *and* every Pro pack — open ∪ proprietary.

use std::marker::PhantomData;

use kshana::api::{run_scenario, ExternalPack, KshanaError, RunOutput, ScenarioMeta};
use serde::de::DeserializeOwned;

use crate::packs::batch_pack::BatchScenario;
use crate::packs::beacon_dop_pack::{BeaconDopScenario, SurfaceBeaconDopStudyScenario};
use crate::packs::beacon_placement_pack::BeaconPlacementScenario;
use crate::packs::cislunar_observability_study_pack::CislunarObservabilityStudyScenario;
use crate::packs::conflict_resilience_study_pack::ConflictResilienceStudyScenario;
use crate::packs::design_optimise_pack::DesignOptimiseScenario;
use crate::packs::lunar_signal_security_pack::LunarSignalSecurityScenario;
use crate::packs::lunar_time_budget_study_pack::LunarTimeBudgetStudyScenario;
use crate::packs::mbse_pack::MbseScenario;
use crate::packs::optical_rf_hybrid_study_pack::OpticalRfHybridStudyScenario;
use crate::packs::optimism_gap_audit::OptimismGapAuditScenario;
use crate::packs::quantum_systematics_pack::QuantumSystematicsScenario;
use crate::packs::realtime_frame_eop_study_pack::RealtimeFrameEopStudyScenario;
use crate::packs::seam_check::SeamCheckPack;
use crate::packs::spectrum_coexistence_pack::SpectrumCoexistenceScenario;
use crate::packs::trade_study_pack::TradeStudyScenario;
use crate::packs::twin_pack::TwinScenario;
use crate::packs::uncertainty_pack::UncertaintyScenario;

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
        UncertaintyScenario::KIND => Some(parse::<UncertaintyScenario>(kind, src)),
        TradeStudyScenario::KIND => Some(parse::<TradeStudyScenario>(kind, src)),
        BatchScenario::KIND => Some(parse::<BatchScenario>(kind, src)),
        QuantumSystematicsScenario::KIND => Some(parse::<QuantumSystematicsScenario>(kind, src)),
        MbseScenario::KIND => Some(parse::<MbseScenario>(kind, src)),
        OptimismGapAuditScenario::KIND => Some(parse::<OptimismGapAuditScenario>(kind, src)),
        DesignOptimiseScenario::KIND => Some(parse::<DesignOptimiseScenario>(kind, src)),
        LunarSignalSecurityScenario::KIND => Some(parse::<LunarSignalSecurityScenario>(kind, src)),
        BeaconDopScenario::KIND => Some(parse::<BeaconDopScenario>(kind, src)),
        SurfaceBeaconDopStudyScenario::KIND => {
            Some(parse::<SurfaceBeaconDopStudyScenario>(kind, src))
        }
        BeaconPlacementScenario::KIND => Some(parse::<BeaconPlacementScenario>(kind, src)),
        LunarTimeBudgetStudyScenario::KIND => {
            Some(parse::<LunarTimeBudgetStudyScenario>(kind, src))
        }
        RealtimeFrameEopStudyScenario::KIND => {
            Some(parse::<RealtimeFrameEopStudyScenario>(kind, src))
        }
        OpticalRfHybridStudyScenario::KIND => {
            Some(parse::<OpticalRfHybridStudyScenario>(kind, src))
        }
        CislunarObservabilityStudyScenario::KIND => {
            Some(parse::<CislunarObservabilityStudyScenario>(kind, src))
        }
        ConflictResilienceStudyScenario::KIND => {
            Some(parse::<ConflictResilienceStudyScenario>(kind, src))
        }
        SpectrumCoexistenceScenario::KIND => Some(parse::<SpectrumCoexistenceScenario>(kind, src)),
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
        UncertaintyScenario::meta_static(),
        TradeStudyScenario::meta_static(),
        BatchScenario::meta_static(),
        QuantumSystematicsScenario::meta_static(),
        MbseScenario::meta_static(),
        OptimismGapAuditScenario::meta_static(),
        DesignOptimiseScenario::meta_static(),
        LunarSignalSecurityScenario::meta_static(),
        BeaconDopScenario::meta_static(),
        SurfaceBeaconDopStudyScenario::meta_static(),
        BeaconPlacementScenario::meta_static(),
        LunarTimeBudgetStudyScenario::meta_static(),
        RealtimeFrameEopStudyScenario::meta_static(),
        OpticalRfHybridStudyScenario::meta_static(),
        CislunarObservabilityStudyScenario::meta_static(),
        ConflictResilienceStudyScenario::meta_static(),
        SpectrumCoexistenceScenario::meta_static(),
    ]
}

/// Every kind a Pro build can run, as one JSON document (`kshana-pro kinds --json`).
///
/// `open` is the embedded open engine's own catalogue
/// ([`kshana::api::list_scenario_kinds`]), never re-listed here, so it cannot drift
/// from the engine the build embeds. `pro` is [`pro_kinds`], each entry carrying the
/// minimum licence tier that runs it. `open_count` and `pro_count` are the lengths of
/// those two arrays, so a reader can count without parsing the entries.
pub fn kinds_json() -> String {
    let open = kshana::api::list_scenario_kinds();
    let pro: Vec<serde_json::Value> = pro_kinds()
        .iter()
        .map(|m| {
            serde_json::json!({
                "name": m.name,
                "min_tier": crate::license::feature_min_tier(m.name).as_str(),
                "description": m.description,
                "required_fields": m.required_fields,
                "optional_fields": m.optional_fields,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "open_count": open.len(),
        "pro_count": pro.len(),
        "open": open,
        "pro": pro,
    });
    // Static strings and integers only: serialisation cannot fail.
    serde_json::to_string_pretty(&doc).expect("the kind catalogue is plain static data")
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

// ─── Registry attachment: the open ∪ Pro superset as a `PackRegistry` ────────────
//
// The `run`/`run_gated` dispatchers above route on a `peek_kind` match table. This
// section exposes the SAME superset as the open core's object-safe
// [`kshana::registry::PackRegistry`], so a Pro build can be driven by id through the
// open registry seam (build-by-id) rather than only by the string dispatcher. It
// reuses the open core's `with_builtins()` for the open half and drives each Pro
// pack through the public [`ExternalPack::register_into`] seam for the Pro half — no
// private back-doors, no duplicated open table.

/// A registry adapter that builds the Pro [`ExternalPack`] `P` from its scenario
/// TOML. It owns no `P` — only the knowledge of how to *parse* one — so
/// `PhantomData<fn() -> P>` keeps `ProFactory<P>` unconditionally `Send + Sync`
/// (required by the object-safe [`kshana::registry::ScenarioFactory`] trait)
/// regardless of whether `P` itself is.
struct ProFactory<P> {
    id: kshana::registry::ScenarioId,
    _marker: PhantomData<fn() -> P>,
}

impl<P> kshana::registry::ScenarioFactory for ProFactory<P>
where
    P: ExternalPack + DeserializeOwned + 'static,
{
    fn id(&self) -> &kshana::registry::ScenarioId {
        &self.id
    }

    fn build(&self, src: &str) -> Result<Box<dyn kshana::api::Scenario>, String> {
        toml::from_str::<P>(src)
            .map(|p| Box::new(p) as Box<dyn kshana::api::Scenario>)
            .map_err(|e| format!("invalid {} scenario: {e}", self.id))
    }

    // metadata(): inherit the trait default (an empty placeholder). A Pro pack's
    // richer `ScenarioMeta` lives on its inherent `meta_static()` and is surfaced
    // via [`pro_kinds()`]; it is not reachable generically from `P: ExternalPack`
    // without an instance (the trait's `meta` takes `&self`), so we do NOT fabricate
    // one here rather than overclaim.
}

/// Build the boxed [`kshana::registry::ScenarioFactory`] that constructs Pro pack
/// `P` for `kind`. Each Pro pack's [`ExternalPack::register_into`] override calls
/// this with its own `Self` / `Self::KIND`.
pub fn pro_factory<P>(kind: &'static str) -> Box<dyn kshana::registry::ScenarioFactory>
where
    P: ExternalPack + DeserializeOwned + 'static,
{
    Box::new(ProFactory::<P> {
        id: kshana::registry::ScenarioId::from_static(kind),
        _marker: PhantomData,
    })
}

/// The single dispatch table for the full **open ∪ Pro** superset, as a
/// [`kshana::registry::PackRegistry`].
///
/// It starts from every open built-in
/// ([`kshana::registry::PackRegistry::with_builtins`]) and then drives each of the 17
/// Pro packs through the public [`ExternalPack::register_into`] seam. The union is
/// therefore assembled entirely through stable public API: the open half is the open
/// core's own table (never re-listed here, so it cannot drift), and the Pro half is
/// each pack registering its own factory. The result builds — and, via the returned
/// [`kshana::api::Scenario`], runs — every open scenario *and* every Pro pack by id.
///
/// Disjointness (Pro `KIND`s carry the [`PRO_KIND_PREFIX`] and never collide with an
/// open id) means the union is exactly additive; the registry's own duplicate-id
/// `debug_assert` is the runtime guard.
pub fn pro_registry() -> kshana::registry::PackRegistry {
    let mut reg = kshana::registry::PackRegistry::with_builtins();
    SeamCheckPack::register_into(&mut reg);
    TwinScenario::register_into(&mut reg);
    UncertaintyScenario::register_into(&mut reg);
    TradeStudyScenario::register_into(&mut reg);
    BatchScenario::register_into(&mut reg);
    QuantumSystematicsScenario::register_into(&mut reg);
    MbseScenario::register_into(&mut reg);
    OptimismGapAuditScenario::register_into(&mut reg);
    DesignOptimiseScenario::register_into(&mut reg);
    LunarSignalSecurityScenario::register_into(&mut reg);
    BeaconDopScenario::register_into(&mut reg);
    SurfaceBeaconDopStudyScenario::register_into(&mut reg);
    BeaconPlacementScenario::register_into(&mut reg);
    LunarTimeBudgetStudyScenario::register_into(&mut reg);
    RealtimeFrameEopStudyScenario::register_into(&mut reg);
    OpticalRfHybridStudyScenario::register_into(&mut reg);
    CislunarObservabilityStudyScenario::register_into(&mut reg);
    ConflictResilienceStudyScenario::register_into(&mut reg);
    SpectrumCoexistenceScenario::register_into(&mut reg);
    reg
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
    fn kinds_json_counts_match_the_engine_and_the_pro_registry() {
        // `kshana-pro kinds --json` is how the open kind count is read from a Pro
        // build, so its counts must be the lengths of the real catalogues.
        let doc: serde_json::Value =
            serde_json::from_str(&kinds_json()).expect("kinds_json prints JSON");
        let open = doc["open"].as_array().expect("open is an array");
        let pro = doc["pro"].as_array().expect("pro is an array");
        assert_eq!(open.len(), kshana::api::list_scenario_kinds().len());
        assert_eq!(doc["open_count"].as_u64(), Some(open.len() as u64));
        assert_eq!(pro.len(), pro_kinds().len());
        assert_eq!(doc["pro_count"].as_u64(), Some(pro.len() as u64));
        for (entry, meta) in pro.iter().zip(pro_kinds()) {
            assert_eq!(entry["name"].as_str(), Some(meta.name));
            assert_eq!(
                entry["min_tier"].as_str(),
                Some(crate::license::feature_min_tier(meta.name).as_str())
            );
        }
        for entry in open {
            let name = entry["name"].as_str().expect("an open kind has a name");
            assert!(
                !name.starts_with(PRO_KIND_PREFIX),
                "open kind '{name}' carries the Pro prefix"
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

    // ─── Superset-additivity: pro_registry() = open ⊎ pro (disjoint union) ───────

    /// Minimal, valid TOML for each Pro kind — deliberately small/fast configs (NOT
    /// the published baselines) so `build` + `run` exercise the real pack code
    /// end-to-end without heavy compute. Each carries its `kind` line for realism;
    /// the pack structs have no `deny_unknown_fields`, so the extra `kind` key is
    /// ignored on parse (exactly as the string dispatcher's `peek_kind` + full parse
    /// already relies on).
    fn pro_src(kind: &str) -> &'static str {
        match kind {
            SeamCheckPack::KIND => "kind = \"pro-seam-check\"\nlabel = \"additivity-test\"\n",
            TwinScenario::KIND => {
                "kind = \"pro-twin\"\n\
                 device = \"CSAC (test)\"\n\
                 adev_points = [[1.0, 3.0e-10], [10.0, 1.0e-10], [100.0, 3.2e-11]]\n\
                 tolerance = 0.25\n\
                 tau0_s = 1.0\n\
                 n_samples = 4096\n\
                 seed = 7\n"
            }
            UncertaintyScenario::KIND => {
                // Eight runs of a closed-form link budget with one uncertain input.
                r#"kind = "pro-uncertainty"
title = "test uncertainty"
seed = 3

base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""

[[input]]
name = "eirp"
key = "eirp_dbw"
dist = "normal"
mean = 55.0
std = 1.0
source = "assumed"

[[output]]
name = "margin"
path = "margin_db"

[sampling]
n = 8
"#
            }
            TradeStudyScenario::KIND => {
                r#"kind = "pro-trade-study"
title = "test trade"
metric = "quantum.fom.holdover_s"
lower_better = false

[[variant]]
label = "optical"
scenario = """
kind = "clock"
seed = 42
threshold_ns = 20.0
[time]
step_s = 10.0
duration_s = 1200.0
[gnss]
windows = [
  { t0 = 0.0,   t1 = 600.0,  state = "nominal" },
  { t0 = 600.0, t1 = 1200.0, state = "denied" },
]
[clock_quantum]
id = "optical-sr-lattice"
provenance = "test"
y0 = 0.0
q_wf = 1.0e-30
q_rw = 0.0
[clock_classical]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
"""

[[variant]]
label = "csac"
scenario = """
kind = "clock"
seed = 42
threshold_ns = 20.0
[time]
step_s = 10.0
duration_s = 1200.0
[gnss]
windows = [
  { t0 = 0.0,   t1 = 600.0,  state = "nominal" },
  { t0 = 600.0, t1 = 1200.0, state = "denied" },
]
[clock_quantum]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
[clock_classical]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
"""
"#
            }
            BatchScenario::KIND => {
                r#"kind = "pro-batch"
title = "test batch"
metric = "quantum.fom.timing_p95_ns"
seed_count = 4
seed_start = 1

base_scenario = """
kind = "clock"
seed = 1
threshold_ns = 20.0
[time]
step_s = 10.0
duration_s = 1200.0
[gnss]
windows = [
  { t0 = 0.0,   t1 = 600.0,  state = "nominal" },
  { t0 = 600.0, t1 = 1200.0, state = "denied" },
]
[clock_quantum]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
[clock_classical]
id = "csac-sa45s"
provenance = "test"
y0 = 0.0
q_wf = 9.0e-20
q_rw = 0.0
"""
"#
            }
            QuantumSystematicsScenario::KIND => {
                "kind = \"pro-quantum-systematics\"\n\
                 species = \"Sr (test)\"\n\
                 bbr_coeff_300k = -2.35e-14\n\
                 temperature_k = 295.0\n"
            }
            MbseScenario::KIND => {
                r#"kind = "pro-mbse"
title = "test srd"

[[requirement]]
id = "SYS-001"
text = "mass <= 12 kg"
target = 12.0
achieved = 10.5
lower_is_better = true
"#
            }
            OptimismGapAuditScenario::KIND => {
                // A tiny, fast config (the published baseline is 400/class × 5 seeds
                // × 1000 perms — far too heavy for a unit test). Still exercises the
                // real open optimism-gap engine through the pack.
                "kind = \"pro-optimism-gap-audit\"\n\
                 n_per_class = 20\n\
                 frac_train = 0.7\n\
                 severities = [0.4, 0.8]\n\
                 seeds = [1, 2]\n\
                 target_pfa = 0.05\n\
                 ridge_lambda = 0.1\n\
                 n_perms = 20\n\
                 perm_seed = 7\n"
            }
            DesignOptimiseScenario::KIND => {
                // A six-design link-budget study: the closed-form kind keeps it fast.
                r#"kind = "pro-design-optimise"
title = "test design study"
seed = 1
base_scenario = """
kind = "link-budget"
band = "x"
eirp_dbw = 55.0
g_over_t_db = 53.0
range_km = 2000.0
data_rate_bps = 1.0e6
other_losses_db = 3.0
required_eb_n0_db = 4.5
"""

[[variable]]
name = "eirp_dbw"
key = "eirp_dbw"
type = "real"
min = 40.0
max = 50.0
step = 5.0

[[variable]]
name = "data_rate_bps"
key = "data_rate_bps"
type = "choice"
values = [1.0e6, 1.0e7]

[[objective]]
name = "margin_db"
sense = "max"
path = "margin_db"

[[objective]]
name = "eirp_dbw"
sense = "min"
variable = "eirp_dbw"
"#
            }
            LunarSignalSecurityScenario::KIND => "kind = \"pro-lunar-signal-security\"\n",
            BeaconDopScenario::KIND => {
                // Tiny sweep (a single high-latitude band, small fleet, few epochs and
                // bootstrap resamples): the headline default reproduces the paper's full
                // sweep and is far too heavy for a unit test.
                "kind = \"pro-beacon-dop\"\n\
                 lat_min_deg = -85.0\n\
                 lat_max_deg = -85.0\n\
                 lat_step_deg = 1.0\n\
                 n_fleet = 4\n\
                 headline_beacons = 3\n\
                 max_beacons = 4\n\
                 n_epochs = 1\n\
                 bootstrap_resamples = 8\n"
            }
            SurfaceBeaconDopStudyScenario::KIND => {
                // Tiny N-sweep + coarse GDOP map; the headline default is a heavy study.
                "kind = \"pro-surface-beacon-dop-study\"\n\
                 n_min = 3\n\
                 n_max = 4\n\
                 map_n_relays = 3\n\
                 map_n_lat = 2\n\
                 map_n_lon = 2\n\
                 n_epochs = 1\n"
            }
            BeaconPlacementScenario::KIND => {
                // 1×1 user grid, 2×2 candidate grid, one epoch, one beacon — the headline
                // default runs the full terrain-aware optimizer over a dense grid.
                "kind = \"pro-beacon-placement\"\n\
                 user_n_lat = 1\n\
                 user_n_lon = 1\n\
                 cand_n_lat = 2\n\
                 cand_n_lon = 2\n\
                 n_epochs = 1\n\
                 k_beacons = 1\n"
            }
            LunarTimeBudgetStudyScenario::KIND => {
                // A tiny τ grid (1 s .. 1000 s, 2 points/decade) — the headline default
                // sweeps 1 s .. 1e7 s at 8 points/decade. Still assembles the real
                // 7-term budget through the open engine.
                "kind = \"pro-lunar-time-budget-study\"\n\
                 clock = \"passive-h-maser\"\n\
                 tau_min_s = 1.0\n\
                 tau_max_s = 1000.0\n\
                 points_per_decade = 2\n"
            }
            RealtimeFrameEopStudyScenario::KIND => {
                // Just the final floor + a 1-day horizon over the bundled real fixture —
                // the headline default runs 1/2/3-day horizons.
                "kind = \"pro-realtime-frame-eop-study\"\n\
                 horizons_days = [1]\n"
            }
            OpticalRfHybridStudyScenario::KIND => {
                // The default is already O(1) (single link budgets + a 5-site availability),
                // so the tiny config is just the kind line — all cited defaults, fast.
                "kind = \"pro-optical-rf-hybrid-study\"\n"
            }
            CislunarObservabilityStudyScenario::KIND => {
                // Two DROs (chief + one reference), a short 4-epoch range+range-rate arc at
                // reduced integration resolution — the headline default differential-corrects
                // four DROs over a 24-epoch range-only arc, far too heavy for a unit test.
                "kind = \"pro-cislunar-observability-study\"\n\
                 perilune_km = [20000.0, 32000.0]\n\
                 ref_phase_frac = [0.0, 0.25]\n\
                 arc = 0.05\n\
                 epochs = 4\n\
                 steps = 800\n\
                 with_range_rate = true\n"
            }
            ConflictResilienceStudyScenario::KIND => {
                // A tiny Monte-Carlo (few trials / samples, coarse intensity grid) over the
                // sourced four-layer baseline; the headline default runs 4000 trials / 2000
                // sensitivity samples over an 11-point grid.
                "kind = \"pro-conflict-resilience-study\"\n\
                 intensity_steps = 4\n\
                 rho_values = [0.0, 0.8]\n\
                 trials = 300\n\
                 sensitivity_samples = 100\n"
            }
            SpectrumCoexistenceScenario::KIND => {
                // One satellite, a coarse grid and a short span: the real study through
                // two open kinds, fast enough for a unit test.
                r#"kind = "pro-spectrum-coexistence"
title = "registry test"
max_degradation_db = 1.0
limit_source = "test limit"
victims = ["gps-l5"]

[grid]
grid_step_deg = 45.0
duration_s = 600.0
step_s = 300.0

[[plan]]
label = "one"
signal = "generic-l"
mask_deg = 10.0

[[plan.satellite]]
altitude_km = 1000.0
inclination_deg = 53.0
"#
            }
            other => panic!("no test source defined for Pro kind '{other}'"),
        }
    }

    #[test]
    fn pro_registry_is_an_additive_disjoint_union_of_open_and_pro() {
        // Additivity = disjoint union with no collisions and no loss. Compute the
        // open count DYNAMICALLY (never hardcoded) so this can't drift as the open
        // core adds built-ins.
        let open = kshana::registry::PackRegistry::with_builtins()
            .ids()
            .count();
        let pro = pro_kinds().len();
        let union = pro_registry().ids().count();
        assert_eq!(
            union,
            open + pro,
            "superset must be exactly additive: open {open} + pro {pro} = {} but registry had {union}",
            open + pro
        );
    }

    #[test]
    fn pro_registry_contains_every_open_builtin() {
        // Faithful superset: nothing the open engine offers is lost in the Pro build.
        let builtins = kshana::registry::PackRegistry::with_builtins();
        let reg = pro_registry();
        for id in builtins.ids() {
            assert!(
                reg.contains(id),
                "open built-in id '{id}' missing from pro_registry()"
            );
        }
    }

    #[test]
    fn pro_registry_contains_every_pro_kind() {
        let reg = pro_registry();
        for m in pro_kinds() {
            let id = kshana::registry::ScenarioId::from_static(m.name);
            assert!(
                reg.contains(&id),
                "Pro kind '{}' missing from pro_registry()",
                m.name
            );
        }
    }

    #[test]
    fn pro_kinds_are_disjoint_from_open_ids() {
        // The union is exactly additive ONLY because no Pro KIND equals any open id.
        let builtins = kshana::registry::PackRegistry::with_builtins();
        let open_ids: Vec<String> = builtins.ids().map(|i| i.as_str().to_string()).collect();
        for m in pro_kinds() {
            assert!(
                !open_ids.iter().any(|o| o == m.name),
                "Pro kind '{}' collides with an open built-in id — union would not be additive",
                m.name
            );
        }
    }

    #[test]
    fn every_pro_kind_builds_and_runs_through_the_registry() {
        // Build + run EVERY Pro kind by id through pro_registry(). No silent skips:
        // each registered kind has a minimal valid source in `pro_src` and must succeed.
        let reg = pro_registry();
        for m in pro_kinds() {
            let id = kshana::registry::ScenarioId::from_static(m.name);
            let scenario = reg.build(&id, pro_src(m.name)).unwrap_or_else(|e| {
                panic!("Pro kind '{}' must build through the registry: {e}", m.name)
            });
            let out = scenario.run().unwrap_or_else(|e| {
                panic!("Pro kind '{}' must run through the registry: {e}", m.name)
            });
            assert!(
                !out.summary.is_empty(),
                "Pro kind '{}' produced an empty summary",
                m.name
            );
        }
    }

    #[test]
    fn registry_drives_both_halves_open_and_pro() {
        // Faithfulness: the SAME registry builds + runs an OPEN kind AND a PRO kind
        // to a valid RunOutput — proving pro_registry() drives the open engine half
        // (via the inherited BuiltinFactory) and the Pro overlay half (via the
        // ProFactory we attached), not just that ids are present.
        let reg = pro_registry();

        // OPEN: ephemeris (TLE source identical to the run_gated open-scenario test).
        let ephem = "kind = \"ephemeris\"\n\
            tle = \"\"\"\n\
            1 25544U 98067A   20045.18587073  .00000950  00000-0  25302-4 0  9990\n\
            2 25544  51.6443 242.0161 0004885 264.6060 207.3845 15.49165514212791\n\
            \"\"\"\n\
            step_s = 60.0\n\
            duration_s = 120.0\n";
        let open_out = reg
            .build(
                &kshana::registry::ScenarioId::from_static("ephemeris"),
                ephem,
            )
            .expect("open ephemeris must build through the registry")
            .run()
            .expect("open ephemeris must run through the registry");
        assert!(
            !open_out.json.is_empty(),
            "open ephemeris run yielded empty JSON"
        );

        // PRO: seam-check via the attached overlay factory.
        let pro_out = reg
            .build(
                &kshana::registry::ScenarioId::from_static(SeamCheckPack::KIND),
                pro_src(SeamCheckPack::KIND),
            )
            .expect("pro seam-check must build through the registry")
            .run()
            .expect("pro seam-check must run through the registry");
        assert!(
            pro_out.summary.contains("seam check OK"),
            "pro seam-check run did not confirm the seam: {}",
            pro_out.summary
        );
    }
}
