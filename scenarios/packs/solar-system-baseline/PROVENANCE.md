# Provenance: solar-system-baseline

The subscription pack for the solar system. Every scenario file comes from the open Kshana engine
repository at commit `da55ae630b16dc93dc81119627b2445396782e3d` (branch `feat/pro-engine-base`, the integrated 75-kind
engine this overlay builds against). A file marked "copied verbatim" is byte-identical to
the open file; the SHA-256 below is of the file in this directory.

| File | Source in the open repository | SHA-256 |
|---|---|---|
| `europa-surface-pnt.toml` | `scenarios/europa-surface-pnt.toml`, copied verbatim | `ae914ffba332ad400debbbc2134746af084b7e87a92ac17af0be15db221e2825` |
| `mars-orbit-pnt.toml` | `scenarios/mars-orbit-pnt.toml`, copied verbatim | `813ff12e5f8119636bd43bf164c08f061b97b6bb5e5da9362bec98f1e1d2f8e8` |
| `solar-system-tour.toml` | `scenarios/solar-system-tour.toml`, copied verbatim | `fae395f4bb88699439c66d9669010f430cd10430f5d442af7849e7399226504b` |

No file in this pack uses a workshop preset or its numbers; `tests/campaign_watch.rs`
scans every pack for the preset's name and fails if it appears.
