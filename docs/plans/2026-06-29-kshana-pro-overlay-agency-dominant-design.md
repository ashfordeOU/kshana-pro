# Design — Kshana Pro Overlay → Agency-Dominant

- **Date:** 2026-06-29
- **Status:** Approved (brainstorming complete; proceeding to implementation plan)
- **Scope:** Coordinated, field-dominant deepening of the Kshana Pro overlay's three layers
  (L1 device digital twins, L2 export-sensitive resilience, L3 trade studies & assurance) and the
  public attachment seams, grounded in real ESA/NASA usage practice.
- **Repos touched:** public `kshana` (science primitives + registry), private `kshana-pro` (packs,
  presets, evidence, gated content).

---

## 1. Context & goal

The overlay diagram (`docs/OVERLAY-ARCHITECTURE.md`, and the public web rendering) describes Kshana Pro
as "a separate private crate — a superset by construction," with three value layers attaching to the
public engine via seven traits: `Scenario`, `ExternalPack`, `ErrorModel`, `EphemerisProvider`,
`ForceModel`, `Propagator`, `LunarEnvironment`.

Current honest maturity (verified by code exploration 2026-06-29):
- **L1 (twin):** real but shallow — a **2-parameter (white-FM + flicker-FM)** least-squares ADEV fit,
  validated by regenerating ADEV; demonstrated on a published CSAC spec. Customer instance NDA-gated.
- **L2 (resilience):** deliberate **fail-closed seam guard** (returns `Unsupported`). The HMAC-SHA256
  license tier gate is real. Nothing to build in content — export law is the wall, by design.
- **L3 (trade + pack_c):** real but single-metric — dot-path FoM extraction, direction-aware ranking,
  SHA-256 hash-pinned Markdown evidence pack, regression gate.
- **Seams:** all 7 are real, open `pub trait`s, but dispatch is a **closed compile-time enum match**;
  pro attaches with **no runtime registry**.

**Goal:** make L1 + L3 + the seams field-dominant and credible at ESA/NASA usage level, coordinated as
one effort, without ever weakening the honesty firewall.

---

## 2. Research grounding (state of the art)

Three parallel web-grounded research streams (full citations in §9). Key findings that shape the design:

**L1 — clocks.** The field standard (NIST SP 1065 / Riley; IEEE Std 1139; Zucca & Tavella 2005; ESA
DAVAR/Hadamard practice; JPL DSAC) is: **all 5 power-law noise types + drift**; the **3-state clock
SDE** (Φ upper-triangular + closed-form process-noise Q); the **full statistic suite**
(MDEV/TDEV/HDEV/ThéoH, not just ADEV); and **χ²/EDF confidence intervals** (Greenhall combined-EDF) on
every point. Validation oracles that exist for free: **NBS Monograph-140 9-point known-answer set**
(ADEV(τ=1) = 91.22945), the **NBS14 1000-point set**, and **allantools** (BSD). Holdover prediction
`x(t)=x₀+y₀t+½Dt²+ε(t)` vs ITU-T G.8272.1 / GNSS clock-prediction interval. Agency credibility wrapper:
NASA-STD-7009B Credibility Assessment Scale (8 factors), ASME V&V 40 (Context of Use), Glaessgen &
Stargel digital-twin definition (AIAA 2012-1818).

**L3 — trade & assurance.** NASA SE Handbook §6.8 Decision Analysis (MOE/MOP/TPM/KPP, utility functions,
robustness assessment) and **ECSS-E-ST-10C Annex L** (a *mandatory* Trade-off Report DRD requiring a
weighted criteria table, classification, and **weight-sensitivity analysis** `<9>`). MCDA: WSM is the
operational default; **AHP for weight derivation** (Consistency Ratio < 0.10); Pareto front + knee-point
is the canonical multi-objective method (MIT MATE). Sensitivity: tornado + Monte-Carlo/SMAA P(winner)
(Saltelli; ~10⁴ weight vectors). Assurance: GSN Community Standard SCSC-141C, ECSS Verification Control
Document, W3C-PROV provenance, content-addressing/Merkle for tamper-evidence, ACM reproducibility badging.

**Seams.** The field's two *operational* engines both use a **runtime registry of factories** to absorb
extensions from parties they don't compile with: **Orekit** (`DataProvidersManager.addProvider`) and
**NASA GMAT** (`FactoryManager.RegisterFactory`). This is exactly the kshana↔kshana-pro relationship
(expression problem: an open type axis needs a registry, not a closed `match`). Rust embodiment: a
**hand-rolled explicit `PackRegistry` of trait-object factories**, WASM/`no_std`-safe. `linkme` rejected
(no WASM), `inventory` relegated to optional native sugar (life-before-main + unspecified order hurts
auditability), `libloading`/cdylib rejected (no stable Rust ABI, UB-on-panic, no WASM). "Superset by
construction" enforced via the GitLab CE/EE `FOSS_ONLY` pattern (public builds standalone) + a shared
conformance suite both crates run.

---

## 3. Invariants (rules every phase obeys)

1. **Honesty firewall.** No `Validated` tier without an `ExternalDataset` oracle in the verification
   matrix (`verification.rs`). Everything else is `Modelled` and labelled so. Extend, never weaken.
2. **Public/private split.** Science primitives → public `kshana` (open, verifiable, heritage-building,
   can earn `Validated`). Productized packs, real-datasheet presets, customer instances, bid evidence →
   private `kshana-pro` (Foreground IP, licensed).
3. **Superset by construction.** Pro = public science + private packaging. CI-enforced.
4. **WASM/`no_std`-safe.** Every new public primitive compiles to `wasm32-unknown-unknown`.
5. **Every emitted number carries `{tier, provenance}`.** The provenance spine and the honesty tier are
   one object both verticals reuse.

---

## 4. Architecture — two substrates, two verticals, one firewall

```
                    ┌─────────────────────────────────────────────┐
                    │   HONESTY FIREWALL (existing ethos)          │
                    │   never Validated without an external oracle │
                    └─────────────────────────────────────────────┘
   SHARED SUBSTRATE A                          SHARED SUBSTRATE B
   ┌───────────────────────────┐               ┌──────────────────────────┐
   │ Assurance / Provenance    │               │ Registry / Seams          │
   │ • W3C-PROV evidence packs │               │ • PackRegistry (factories)│
   │ • χ²/EDF uncertainty       │               │ • with_builtins()+additive│
   │ • NBS/allantools oracles  │               │ • CI superset enforcement │
   │ • GSN/VCD traceability     │               │ • ExternalPack→register   │
   └───────────────────────────┘               └──────────────────────────┘
            ▲              ▲                          ▲            ▲
   ┌────────┴───────┐  ┌───┴────────────────┐  ┌──────┴───────┐
   │ VERTICAL L1    │  │ VERTICAL L3        │  │ L2 = legal    │
   │ Clock twins    │  │ Trade & assurance  │  │ wall — gate   │
   └────────────────┘  └────────────────────┘  │ only, no      │
                                                │ content build │
                                                └──────────────┘
```

L1's "V&V credibility dossier" and L3's "audit-grade evidence pack" are the **same artifact** — both are
expressions of `ValidationTier {Validated, Modelled, Partial}` (`fom_label.rs:37`) + the verification
matrix. Build the substrates once; both verticals inherit credibility.

---

## 5. Phase 0 — The Spine (public `kshana`)

Gating prerequisite for the cleanest end-state.

### 5.1 Substrate B — `PackRegistry` seam (`kshana::registry`)
Runtime registry replacing closed enum-match dispatch (Orekit/GMAT model in idiomatic Rust):

```rust
pub struct ScenarioId(Cow<'static, str>);              // open id; built-ins &'static, alloc-free
pub trait ScenarioFactory: Send + Sync {               // object-safe
    fn id(&self) -> &ScenarioId;
    fn build(&self, cfg: &ScenarioConfig) -> Result<Box<dyn Scenario>, BuildError>;
    fn metadata(&self) -> ScenarioMeta { ScenarioMeta::default() }   // defaulted → semver-minor growth
}
pub struct PackRegistry { scenarios: BTreeMap<ScenarioId, Box<dyn ScenarioFactory>> }  // no_std + deterministic
impl PackRegistry { pub fn with_builtins() -> Self { /* seed every current kind */ } }
```
`ExternalPack` shifts to `fn register_into(&mut PackRegistry)`; pro entry point:
`let mut r = kshana::PackRegistry::with_builtins(); ProPack::register_into(&mut r);` — additive by
construction. **Migration is additive & semver-safe** (kshana pre-1.0): introduce registry alongside the
enum, re-implement enum dispatch *in terms of* the registry, no signature breaks.

### 5.2 Substrate A — Assurance / Provenance / Uncertainty (`kshana::assurance`)
- **`provenance`** — W3C-PROV record per number: engine **commit SHA** (not tag), input hashes, seed +
  nondeterminism caveats, `Cargo.lock` hash, timestamp, **Merkle/hash-chain root**, reproduction
  tolerance.
- **`uncertainty`** — **χ²/EDF confidence intervals** via **Greenhall combined-EDF**, with per-τ noise
  identification. Reused by L1 (Allan stats) and L3 (Monte-Carlo P(winner)).
- **`oracle` harness** — first-class registration of an `ExternalDataset` oracle + known-answer vectors,
  so `Modelled→Validated` is a structured act. **Hook into the active validation campaign.**

### 5.3 CI gates added
Public-standalone build (`FOSS_ONLY` analog) · shared conformance suite · registry-additivity assertion
(`pro.ids() ⊇ public.ids()`).

---

## 6. Phase 1 — L1 Clock Twin (science public, packs private)

| Tier | Capability | Where | Oracle → tier |
|---|---|---|---|
| T1 | Full **5-noise power-law + drift** fit; **3-state clock SDE** (Φ + closed-form Q); clock-class switch (Cs 2-state / Rb·maser·optical 3-state) | public `kshana::clock` | allantools + NBS-140 → **Validated** |
| T1 | Statistic suite: **MDEV, TDEV, overlapping HDEV, ThéoH/TOTVAR** | public | NBS-140 9-pt KAT → **Validated** |
| T1 | **χ²/EDF confidence intervals** on every point (Substrate A) | public | Greenhall ref → **Validated** |
| T2 | Known-answer regression vectors (NBS-140; NBS14 1000-pt) + allantools cross-val to 1e-3 | public tests | this *is* the oracle |
| T3 | **Device-class presets** keyed to real datasheets (CSAC SA.45s, OCXO, Rb PRS10, Galileo RAFS/PHM, Sr/Yb optical) | private `kshana-pro` | datasheet = published budget |
| T3 | Optical **systematic-shift budget** (lattice AC-Stark scalar/vector/tensor/hyperpolarizability, DC-Stark, density, micromotion+quadrupole, Dick-effect+QPN) | private (formulas could be public) | published budgets |
| T4 | **Holdover prediction** `x(t)=x₀+y₀t+½Dt²+ε(t)` with UQ band; report vs G.8272.1 / 1.5µs-24h / GNSS interval | private pack | Modelled |
| T5 | Customer twin instance (NDA) + **V&V dossier** (NASA-STD-7009B CAS + ASME V&V 40) | private (gated) | customer ref data |

**T1+T2** = decisive jump past the 2-parameter baseline; independently shippable.

---

## 7. Phase 2 — L3 Trade & Assurance (math public, packs private)

| Pri | Capability | Where | Standard |
|---|---|---|---|
| P0 | **Weighted decision matrix** (WSM) + **value-function normalization** + **knock-out** (M=1 = today's single-metric, backward-compatible) | public `kshana::mcda` | NASA §6.8; ECSS Annex L `<7><8>` |
| P0 | **AHP-for-weights** with **CR < 0.10 gate** | public | NASA-endorsed; Saaty |
| P1 | **Pareto front** + knee-point selection (MATE) | public | aerospace canonical |
| P2 | **Weight-sensitivity**: tornado + Monte-Carlo/SMAA **P(winner)** + min-weight-change-to-flip | public | ECSS Annex L `<9>` *requires it* |
| P3 | Utility/value functions w/ risk attitude (MAUT) | public | NASA §6.8.2.2.5 |
| P4 | **PROV-grade evidence pack** (Substrate A) — extends hash table to full provenance + Merkle chain | public emitter / private content | FAIR R1.2; ISO 27037 |
| P5 | **GSN assurance case** + ECSS **Verification-Control-Document** traceability | public structure / private instance | GSN SCSC-141C; ECSS-E-ST-10-02C |
| P6 | Output shaping to **ECSS Annex L** + **NASA Table 6.8-1**; **TRL per variant** (ISO 16290) w/ TRL-6 knock-out | private packs | submittable as PDR/CDR evidence |
| P7 | Regression gate → **multi-criteria assurance gate** (winner-stability, Pareto-membership, CR<0.10, provenance-completeness, P(winner), hash-chain integrity) | private | CI-enforced honesty |

**P0+P2** = "ECSS-Annex-L-conformant weighted trade-off with mandatory sensitivity"; biggest credibility
jump for least code; independently shippable.

---

## 8. Phase 3 — Agency-credibility wrapper & surfacing
- **V&V dossier generator** (NASA-STD-7009B CAS + ASME V&V 40) consuming Substrate A.
- **Update `OVERLAY-ARCHITECTURE.md` + the web diagram** to reflect new depth and any
  `Modelled→Validated` flips — **only after** the flips are real (honesty firewall).
- **Feed the validation campaign** (`project_kshana_validation_campaign`): each new public primitive
  checked against allantools/NBS/published vectors becomes a campaign flip.

---

## 9. Cross-cutting

- **Testing:** TDD throughout. Known-answer vectors are the L1 oracle; property tests for registry
  additivity; the shared conformance suite is the superset proof. **Battle-test end to end:** each
  vertical must run from scenario → registry → pack → evidence pack → tier-labelled report, with the
  honesty gate green.
- **Semver/migration:** all new public API additive; trait growth only via defaulted methods;
  `#[non_exhaustive]` on new public enums/structs from day one.
- **Scale reality:** large (multi-thousand-line, weeks). Ramps designed so T1+T2 (L1) and P0+P2 (L3) are
  independently shippable. Phase 0 gates the cleanest end-state.
- **Execution:** strong candidate for multi-agent workflow (per-module parallel TDD) at execution time;
  decided in the plan, not here.

---

## 10. Decisions resolved
- Sequencing: **substrate-first, do all of it** (user: "all").
- Public/private split: science public, product private (as in §3.2).
- Registry refactor: **in Phase 0** (accepted churn for structural superset enforcement).
- L2: no content build — gate only (export law).

---

## 11. Key sources
NIST SP 1065 (Riley, Handbook of Frequency Stability Analysis); IEEE Std 1139-2008; Zucca & Tavella 2005
(3-state clock model, arXiv:1506.01080); Greenhall & Riley combined-EDF (35th PTTI 2003); allantools
(github.com/aewallin/allantools); NBS Monograph 140; Burt et al. *Nature* 595 (DSAC); NASA-STD-7009B;
AIAA 2012-1818 (Glaessgen & Stargel). NASA SE Handbook SP-2016-6105 Rev2 §6.8; ECSS-E-ST-10C Rev.1
Annex L; ECSS-E-ST-10-02C; ISO 16290 (TRL); Saaty AHP; MIT MATE (Ross & Hastings); Saltelli GSA;
GSN Community Standard SCSC-141C; W3C PROV-DM; FAIR (Wilkinson 2016). Orekit
(DataProvidersManager/ForceModel/Propagator); NASA GMAT (Factory/FactoryManager, NTRS 20180000083);
Nyx/ANISE (Rust trait-generic precedent); Fowler PoEAA (Plugin/Registry/Separated Interface); Rust
Cargo SemVer reference + API Guidelines; GitLab CE/EE `FOSS_ONLY` open-core pattern.
