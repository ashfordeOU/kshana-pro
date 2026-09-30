# Provenance of the engine scenarios under `engine/`

The three files in `engine/` are copies of scenario files of the open Kshana engine,
taken unchanged from its `scenarios/` directory. The studies in this directory use them
as their base scenarios: each study's `base_scenario` text is the engine file with its
comment lines removed (for the campaign, the `[monte_carlo.scenario]` table written as a
stand-alone scenario). `tests/uncertainty.rs` checks the SHA-256 (Secure Hash Algorithm,
256-bit) values below against the files, checks each file against the embedded engine's
own copy, and checks that every study's base scenario parses to the same content as its
engine file.

- Open engine repository commit: `da55ae630b16dc93dc81119627b2445396782e3d`
  (2026-09-29, branch `feat/pro-engine-base`, version 0.28.0 with 75 scenario kinds;
  the working tree was clean when the files were copied).
- Copied on 2026-09-30.

| File | Used by | SHA-256 |
|---|---|---|
| `engine/campaign-monte-carlo-clock-holdover.toml` | `uq-clock-holdover-seeds.toml`, `uq-clock-holdover-qwf.toml` | `3504af515f45a4ea2686122978cad662d1ca93c637ffe26a327ea4e282e3a004` |
| `engine/leo-pnt-end-to-end.toml` | `uq-leo-pnt-chain.toml`, `uq-leo-pnt-chain-sobol.toml` | `4a0286b53161bff2257bb693b77f73c20632df077f0eb84a459d5a32ef21ebce` |
| `engine/link-budget.toml` | `uq-link-budget-margin.toml` | `1c75d05b9fc5e9374a896c944d6495cf972cd373fd6922ac1acbef0d3588df7f` |

No file of the engine's optional in-orbit-demonstration preset is used, and no study here
depends on it.
