# Provenance: spectrum-baseline

The subscription pack for spectrum. Every scenario file comes from the open Kshana engine
repository at commit `39afbd712df32c0252f82763358f19c91f725d0f` (branch `feat/pro-engine-base`, the integrated 75-kind
engine this overlay builds against). A file marked "copied verbatim" is byte-identical to
the open file; the SHA-256 below is of the file in this directory.

| File | Source in the open repository | SHA-256 |
|---|---|---|
| `l-band-waterfall-jamming.toml` | `scenarios/l-band-waterfall-jamming.toml`, copied verbatim | `204830758faac62c51e3b396ff425442fee7d781378790c9c4ca7173f6c4973d` |
| `multi-band-jamming-waterfall.toml` | `scenarios/multi-band-jamming-waterfall.toml`, copied verbatim | `698a3ffd6da94878d98913acb7c65bd732047e42910382aa273aae64548b0dc5` |

No file in this pack uses a workshop preset or its numbers; `tests/campaign_watch.rs`
scans every pack for the preset's name and fails if it appears.
