# Provenance: leo-pnt-baseline

The subscription pack for low-Earth-orbit positioning, navigation and timing. Every scenario file comes from the open Kshana engine
repository at commit `da55ae630b16dc93dc81119627b2445396782e3d` (branch `feat/pro-engine-base`, the integrated 75-kind
engine this overlay builds against). A file marked "copied verbatim" is byte-identical to
the open file; the SHA-256 below is of the file in this directory.

| File | Source in the open repository | SHA-256 |
|---|---|---|
| `leo-pnt-end-to-end.toml` | `scenarios/leo-pnt-end-to-end.toml`, copied verbatim | `4a0286b53161bff2257bb693b77f73c20632df077f0eb84a459d5a32ef21ebce` |
| `xona-pulsar-end-to-end.toml` | `scenarios/xona-pulsar-end-to-end.toml`, copied verbatim | `69aaf2a92478e3cabf27d4700e68f1ad231b1caf987d33d7e6a7313380971c0f` |

No file in this pack uses a workshop preset or its numbers; `tests/campaign_watch.rs`
scans every pack for the preset's name and fails if it appears.
