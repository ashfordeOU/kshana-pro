<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
Bid framing for the two FutureNAV demonstrators now runnable from the OPEN simulator.
Contact: contact@ashforde.org
-->

# Runnable bid demonstrators — 13494 AI-eval & 13503 quantum-trade

**Tenders:** ESA AO/1-13494 (AI & Digital Payload PNT) · AO/1-13503 (Quantum-Enabled PNT)
**What changed (2026-06-16):** the two flagship demonstrators are now **first-class
scenario kinds in the open `kshana` simulator** — a prime / ESA evaluator can run and
audit them directly from the public tool (and, since Kshana Pro is a superset, via
`kshana-pro run` too). Previously they were rich, tested *library* code with no CLI
front-door; now they are reachable, reproducible, and honesty-guarded.

> **HONESTY BANNER.** Both demonstrators are **MODELLED**. The 13494 corpus is
> synthetic and parameter-grounded (composed from Kshana's jamming/AGC/SQM/parity
> models) — never field captures or raw IQ; the harness reports operating
> characteristics only, with no good/bad verdict. The 13503 trade **quantifies** a
> partner clock's benefit and never validates the device; when a clock floor is an
> assumed class default (not a measured ADEV), a floor caveat renders on the artifact.
> Neither borrows the SGP4 / Allan-Stable32 / IGS external-validation islands. TRL ≤ 3.

---

## 1. AI/ML RF-impairment evaluation testbed (13494) — `impairment-eval`

```sh
kshana scenarios/impairment-eval.toml          # open tool
kshana-pro run scenarios/impairment-eval.toml  # Pro superset (same result)
```

Generates a labelled, class-balanced synthetic corpus (nominal / jamming / spoof-time /
spoof-position / multipath), scores a detector (`energy|agc|sqm|parity|fused`) through
the **detector-agnostic** harness, and reports AUC / ROC / confusion + per-class Pd at a
target Pfa — **plus the in- vs out-of-distribution optimism gap** a hostile reviewer
cares about (`auc_in − auc_out`, flagged when it exceeds tolerance). Reference run:
AUC ≈ 0.998 in-distribution / ≈ 0.981 out, gap ≈ 0.017 (not flagged optimistic).

**Differentiator:** GMV/Qascom ship a GNSS *simulator*; none ships an open, regenerable
**referee** that scores *any* detector on a labelled, params-grounded corpus and reports
its own optimism. Covers dominance items *impairment-corpus generator* + *eval-testbed*.

## 2. Quantum-vs-classical PNT trade (13503) — `quantum-trade`

```sh
kshana scenarios/quantum-trade.toml
kshana-pro run scenarios/quantum-trade.toml
```

Computes the timing- and inertial-holdover benefit of a candidate clock over a classical
baseline, plus a GNSS-denied resilience-vs-time envelope. The candidate floor is driven
by a partner clock's **measured overlapping-ADEV curve** fitted by non-negative least
squares — the **defensibility hinge** that turns "another model" into a partner-data
budget (`floor_assumed = false`). Hand a DLR/PTB/NPL clock team their lab ADEV and watch
the holdover budget derive honestly. Covers dominance items *measured-ADEV ingestion* (A05)
+ *quantum-vs-classical trade* (A07).

## 3. Why this wins evaluation points

- **Run-it-yourself credibility.** The 13494/13503 demonstrators execute from the open
  tool with reproducible scenario hashes — an evaluator audits the method, not a claim.
- **Verification discipline on display.** Both kinds carry MODELLED labels, the optimism
  gap / floor caveat, and live verification-matrix rows; a no-overclaim regression test
  (`tests/dominance_demonstrators.rs`) fails if any of these honesty invariants slips.
- **Open funnel intact.** Capability lives in the open Apache-2.0 core (the calling
  card); the Pro overlay adds the bid framing, packaging, and licensed SKUs — never
  starving the open core.

## 4. What this is NOT

- Not validated/flight-demonstrated performance — MODELLED only.
- Not a field/IQ corpus (13494) — synthetic, parameter-grounded.
- Not a device validation (13503) — quantifies, never validates, a partner clock.

---

*Open implementation: `impairment_eval` / `quantum_trade` scenario kinds in the public
Kshana repo (`scenarios/impairment-eval.toml`, `scenarios/quantum-trade.toml`).
Bid framing: this document (kshana-pro, PRIVATE).*
