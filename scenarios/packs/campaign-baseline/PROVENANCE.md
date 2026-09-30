# Provenance: campaign-baseline

The subscription pack for campaigns. Every scenario file comes from the open Kshana engine
repository at commit `da55ae630b16dc93dc81119627b2445396782e3d` (branch `feat/pro-engine-base`, the integrated 75-kind
engine this overlay builds against). A file marked "copied verbatim" is byte-identical to
the open file; the SHA-256 below is of the file in this directory.

| File | Source in the open repository | SHA-256 |
|---|---|---|
| `campaign-jam-spoof-holdover-integrity.toml` | `scenarios/campaign-jam-spoof-holdover-integrity.toml`, copied verbatim | `cec1e10f8eb478bae58acebe9c2f06cfccf6572f6f6895dc633fb409b8d8ee5f` |
| `campaign-monte-carlo-clock-holdover.toml` | `scenarios/campaign-monte-carlo-clock-holdover.toml`, copied verbatim | `3504af515f45a4ea2686122978cad662d1ca93c637ffe26a327ea4e282e3a004` |
| `campaign-sweep-jammer-power.toml` | `scenarios/campaign-sweep-jammer-power.toml`, copied verbatim | `c7a343ad8f0648e5910e04254346b09fcb07e96b741a982ca857d52345069e2a` |

No file in this pack uses a workshop preset or its numbers; `tests/campaign_watch.rs`
scans every pack for the preset's name and fails if it appears.
