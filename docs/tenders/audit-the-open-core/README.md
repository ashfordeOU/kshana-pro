<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only; NOT for the open kshana repo.
Contact: contact@ashforde.org
-->

# Audit the open core — prime-facing trust package (PRIVATE)

Internal bid-pursuit asset (**dominance-plan step A12**). A single leave-behind that lets a prime's
chief engineer audit the **open Kshana core line-by-line in an afternoon** and confirm the
machine-checked honesty discipline behind it.

**This is the prime-facing trust differentiator:** every other navigation simulator in the
competition is a black box; ours is public under Apache-2.0 and a CI-enforced invariant makes it
structurally impossible to label a model as a validation.

## Contents

| File | What it is |
|------|------------|
| [`AUDIT-THE-CORE.md`](AUDIT-THE-CORE.md) | The leave-behind. One-page index mapping each capability claim → exact open-repo module → tests → validated/modelled label → `OracleKind`; the machine-checked verification matrix and CI-enforced invariant (`Validated ⇒ ExternalDataset`); the five real validation islands with their external oracles; and the "what we deliberately do NOT claim" paragraph. |

## What it points at

The **public** open-core repo `github.com/ashfordeOU/kshana` (Apache-2.0, v0.17.0). Specifically:
`src/verification.rs` (the matrix + the invariant), `docs/VALIDATION.md`, `docs/PROVENANCE.md`,
`docs/SGP4-VALIDATION.md`, `docs/CLAIMS-VS-REALITY.md`. Every claim in the leave-behind traces to
real code that was read, not asserted from memory.

## Standing rules

- **Open core only.** No export-controlled threat libraries, no customer-calibration data, no
  proprietary-overlay internals appear in this package — only what is already public and runs
  offline with `cargo test`.
- **Honest framing, always.** Software-model validation; **TRL ~3**; no flight heritage, no
  certification, no agency endorsement. The five validation islands are **all software-model** —
  none flight or hardware — and the leave-behind says so explicitly.
- **Private.** Stays in the kshana-pro repo. Never publish in the open kshana repo.
- All external contact: **contact@ashforde.org**.
