<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
B03 — cited cold-atom-interferometer (CAI) error-model parameter sheet for AO/1-13503.
For a prime's technical volume / Phase 0/A trade pack. Contact: contact@ashforde.org
-->

# CAI parameter sheet — cited cold-atom-interferometer error model

**Tender:** ESA AO/1-13503/26/NL/RR — *Quantum-Enabled PNT Demonstrator* (FutureNAV C3), Phases 0/A/B1
**Step:** B03 — cited CAI accelerometer/gyro parameter sheet → bracketed error model
**Author / owner:** Ashforde OÜ — navigation-performance & quantum-vs-classical trade-study SME (consortium WP contributor)
**Engine of record:** Kshana PNT-performance simulator, v0.20.0 — module `inertial::cai_params` (open core, dual-licensed: GNU AGPL-3.0-only + commercial licence)
**Contact:** contact@ashforde.org

> **HONESTY BANNER (load-bearing — keep it on every face of this artifact).**
> Every value below is a **MODELLED literature-survey bracket**, not a validated
> figure and not a quote. Each bracket is an **order-of-magnitude range** distilled
> from the published atom-interferometry literature and is flagged
> `needs_source_confirmation = true`: **confirm the exact value against the named
> primary source before any bid use.** This sheet **quantifies** what a
> navigation-grade CAI *delivers* at performance level; it **does not model any
> partner's device, claim flight heritage, or validate anyone's instrument.** The
> CAI/optical-clock hardware — and its mandated Key Person — are the **quantum-
> hardware partner's**, not Ashforde's. No TRL > 3, no certification, no ESA
> endorsement is claimed. The sheet does **not** borrow Kshana's external-oracle
> validation islands (SGP4 / Allan–Stable32 / IGS).

---

## 1. Why this exists

A quantum-PNT consortium needs a ground-side analysis partner who can take a cold-
atom inertial sensor's *performance* and turn it into a navigation budget — without
pretending to own or validate the hardware. This sheet is that bridge. It curates the
metrics the sensor-physics community uses (bias instability, velocity/angle random
walk, scale-factor stability, interrogation-limited sample rate, fringe-ambiguity
dynamic range), brackets each one (best / nominal / conservative), attaches a
citation to every bracket, and **encodes it so the Kshana navigation budget
(`QuantumNavBudget`) and the quantum-vs-classical trade engine can consume CAI
performance directly** — no device model required.

It is implemented and unit-tested in the open repo (`inertial::cai_params`), so a
reviewer can read the brackets, the citations, and the consistency checks in code.

## 2. The cited parameter brackets

Each row is `best / nominal / conservative` with the convention that **smaller is
better** for error metrics and **larger is better** for sample rate and dynamic
range. SI is canonical; convenience units are shown in parentheses. Every row is
flagged for source-confirmation.

| Metric | Best | Nominal | Conservative | Unit (convenience) | Primary source(s) — confirm before use |
|---|---|---|---|---|---|
| Bias instability | 1.0e-7 | 1.0e-6 | 1.0e-5 | m/s² (≈0.01 / 0.1 / 1 µg) | Bongs et al., *Nat. Rev. Phys.* 1, 731 (2019); Geiger et al., *Nat. Commun.* 2, 474 (2011) (airborne CAI ~few µg); Exail/Muquans AQG class |
| Velocity random walk (accel. ASD) | 1.0e-8 | 1.0e-7 | 1.0e-6 | m/s²/√Hz (≈1 / 10 / 100 ng/√Hz) | Bongs et al. (2019); Geiger et al. (2011); shot-noise-limited CAI sensitivity |
| Scale-factor stability | 1.0e-3 | 1.0e-2 | 1.0e-1 | ppm (≈1 / 10 / 100 ppb) | Scale factor `k_eff·T²` set by the atomic transition / optical reference; Le Gouët et al., *Appl. Phys. B* 92, 133 (2008) |
| Sample rate (interrogation-limited) | 10 | 2 | 0.5 | Hz | Cycle-time limit `T_c`; interleaved/joint interrogation, e.g. Savoie et al., *Sci. Adv.* 4, eaau7948 (2018) |
| Dynamic range (fringe-limited, hybrid-extended) | 50 | 1.0 | 1.0e-3 | m/s² (≈5.1 / 0.1 / 1e-4 g) | Raw = fringe ambiguity (computed, §4); hybrid CAI+classical: Lautier et al., *APL* 105, 144102 (2014); Cheiney et al., *PRApplied* 10, 034030 (2018) |
| Angle random walk (Sagnac gyro) | 1.0e-9 | 1.0e-8 | 1.0e-7 | rad/s/√Hz | Gauguet et al., *PRA* 80, 063604 (2009); Stockton et al., *PRL* 107, 133001 (2011); Durfee et al., *PRL* 97, 240801 (2006) |

**Umbrella survey for the whole sheet:** Bongs, Holynski, Vovrosh, Bouyer, Condon,
Rasel, Schubert, Schleich & Roura, *"Taking atom interferometric quantum sensors
from the laboratory to real-world applications"*, **Nature Reviews Physics 1,
731–739 (2019)**.

## 3. How a consumer uses it

`CaiParameterSheet::to_nav_budget(level, ref_accel_m_s2, tau_stability_s)` builds a
`QuantumNavBudget` that consumes the brackets at a chosen level:

- `bias_m_s2` ← bias-instability bracket at `level`;
- `scale_factor_ppm` ← scale-factor-stability bracket at `level`;
- the white-noise floor ← the reference CAI physics for that level (§4);
- `ref_accel_m_s2`, `tau_stability_s` ← caller (the platform's sustained specific
  force and the long-term stability time).

The budget then yields dead-reckoning position-error 1σ vs coast time and the
inertial-holdover figure that feeds the GNSS-denied resilience envelope. A consortium
partner can therefore drop **their** measured numbers into the same bracket shape and
get a directly comparable budget.

## 4. Cross-checks (consistency, not assertion)

Two of the brackets are checked against first principles rather than simply stated:

1. **Velocity random walk** — the cited VRW bracket is cross-checked against the VRW
   the *existing* Kshana CAI interferometer physics (`CaiAccelerometer::accel_asd`,
   shot-noise-limited `σ_a·√T_c`) produces for a documented reference configuration
   (Rb87 D2 line, `T = 50 ms`, `N = 1e6`, contrast 0.5, cycle time `1/rate`). The
   unit test `physics_vrw_lands_within_the_cited_vrw_bracket` asserts the physics
   value falls **inside** the cited bracket at all three levels — an internal-
   consistency oracle.
2. **Dynamic range** — the raw single-sensor limit is **computed** from the fringe-
   ambiguity condition `Δa = 2π/(k_eff·T²)`. For the reference configuration this is
   ≈ **1.56e-4 m/s² (≈16 µg) per fringe**, far below the navigation dynamic range,
   which is why a bare CAI must be **hybridised** with a classical accelerometer to
   reach the ~several-g bracket. The test
   `raw_fringe_ambiguity_is_computed_and_far_below_the_hybrid_range` pins this.

A third check is behavioural: the conservative budget must drift more than the best
budget at a fixed coast time (every term is worse) — `conservative_budget_drifts_more_than_best_budget`.

## 5. Verification status

In the Kshana verification matrix (`src/verification.rs`) this capability is recorded
as:

- **Status:** `MODELLED` (not Validated — there is no redistributable numeric
  external oracle for navigation-grade CAI hardware that Kshana could recompute
  against; claiming Validated here would be an overclaim).
- **Oracle kind:** `InternalConsistency` — the VRW physics cross-check plus the
  computed fringe-ambiguity limit.
- **Invariant:** the matrix CI test enforces that nothing is labelled Validated
  without an `ExternalDataset` oracle, so this row cannot silently inherit the
  validation halo of the SGP4 / Allan / IGS islands.

## 6. What this is NOT

- **Not a device model.** No specific instrument is simulated; no hardware is owned.
- **Not validated performance.** The numbers are literature brackets pending source
  confirmation; they are illustrative until a partner supplies measured values.
- **Not a flight-heritage or certification claim.** TRL ≤ 3, no ESA endorsement.
- **Not the mandated quantum-nav Key Person.** That role is partner-provided; Ashforde
  is the performance-analysis WP contributor only.

---

*Open implementation: `inertial::cai_params` in the public Kshana repo. Bid framing
and partner positioning: this document and the 13503 pursuit pack (kshana-pro,
PRIVATE).*
