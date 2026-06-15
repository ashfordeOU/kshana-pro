<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; never publish.
Contact: contact@ashforde.org
-->

# Reproducing every headline number (13494 SE note)

A reviewer can independently reproduce every headline figure in this bid with a
handful of one-line commands. **All of these pin our own deterministic outputs —
that is _reproducibility_, not external validation.** A passing CI gate here is
**not** ESA acceptance and **not** TRL maturation; it only proves the numbers are
reproducible and the honesty labels hold.

## One-command reruns

From the `kshana-pro` repo:

```bash
# Twin reproducibility goldens + SpaceStream golden messages + the metric gate:
cargo test --test reproducibility_twin

# The no-overclaim CI guard for the mission-analysis artifacts (benefits + MRD):
cargo test --test no_overclaims_mission_analysis

# The full overlay test suite (dispatcher, packs, twin, suite, report, connectors):
cargo test
```

From the open `kshana` repo (the engine all numbers derive from):

```bash
cargo test                         # full suite, including the validation islands
./scripts/check-reproducible.sh    # reference scenario is byte-identical across runs
```

## What each golden pins

| Golden | What it pins | Cross-platform? |
|--------|--------------|-----------------|
| `GOLDEN_SCENARIO_HASH` (`reproducibility_twin.rs`) | SHA-256 over the canonical scenario **inputs** | **Yes** — hashes inputs, not float outputs. |
| Twin bundle `to_json` | The NAV-DT bundle is byte-stable across runs | In-process determinism; cross-platform rests on the open core's reproducibility invariant. |
| Consolidated-report fingerprint (FNV-1a) | The whole consolidated artifact reproduces | In-process determinism. |
| `scenarios/spacestream-*.json` | The four SpaceStream golden messages match the canonical schema | **Yes** — structural. |

## The metric gate (CI)

`ConsolidatedReport::gate(&[GateCheck { metric, op, threshold }])` reuses the
connectors' `GateOp` to fail a pipeline when a headline FoM crosses a threshold or a
**required** RTM row is non-compliant. Example: fail if `availability < 0.5` or any
required requirement-traceability row fails. The gate's own outcome carries the
honest note: *"A passing gate pins reproducible thresholds on our own MODELLED
outputs. It is NOT external validation, ESA acceptance, or TRL maturation."*

## The honest distinction (read this)

- **Golden vectors / a passing gate = reproducibility** of *our own* modelled
  outputs. Anyone re-running the commands gets the same bits.
- **External validation** is a separate, smaller set — the five validation islands
  (SGP4 vs AIAA, Allan vs NIST SP-1065, IGS real data, IAU/SOFA frames, Mars-LMO +
  DE440), all of which are **software-model** validations, none flight or hardware.
- A reproducible modelled number is still a **modelled** number. The verification
  matrix (`kshana::verification`) is the source of truth for which is which, and the
  no-overclaim guard fails CI if anything is mislabelled.

_Last updated 2026-06-15. PRIVATE — do not publish._
