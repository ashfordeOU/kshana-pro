# Provenance: constellation-baseline

The subscription pack for constellations. Every scenario file comes from the open Kshana engine
repository at commit `da55ae630b16dc93dc81119627b2445396782e3d` (branch `feat/pro-engine-base`, the integrated 75-kind
engine this overlay builds against). A file marked "copied verbatim" is byte-identical to
the open file; the SHA-256 below is of the file in this directory.

| File | Source in the open repository | SHA-256 |
|---|---|---|
| `constellation-multi-gnss-coverage.toml` | `scenarios/constellation-multi-gnss-coverage.toml`, copied verbatim | `ab1e8d8d48fdfba293faeacac0ec15cbd0ef8a0a7658663055c5a3e7eb7233f0` |
| `gps-baseline-sps.toml` | derived (see below) | `769f1532b8d12298066577a315abdf7467c7254f1deee87f77446c79a61d0237` |
| `lunar-relay-constellation.toml` | `scenarios/lunar-relay-constellation.toml`, copied verbatim | `d626dcb56d0749b58bae19d446a5d869cecc3126ce499d0862c2be2a2665dff3` |

## Derived file

`gps-baseline-sps.toml` is derived from `scenarios/constellation-multi-gnss-coverage.toml`
(SHA-256 `ab1e8d8d48fdfba293faeacac0ec15cbd0ef8a0a7658663055c5a3e7eb7233f0`) to reproduce the conditions of the Global Positioning System (GPS) Standard Positioning Service
Performance Standard (SPS PS), 5th edition (2020), Appendix B, as the open engine's own
test `constellation::tests::gps_baseline_global_dop_matches_the_sps_performance_standard`
sets them. The edits:

- only the `gps-baseline` preset (Galileo, BeiDou and GLONASS removed);
- `duration_s = 86164.0` (one sidereal day) and `step_s = 300.0` (287 five-minute epochs);
- `mask_deg = 5.0`, `grid_step_deg = 4.0`, `clock = "common"` (one receiver clock);
- `max_tracks = 24` (one ground track per satellite); the comment header is replaced by one
  that states the derivation.

The open document `docs/CONSTELLATION-DESIGN.md` states what this run reproduces: global
horizontal dilution of precision (HDOP) median 0.940, 95 % 1.255 and mean 0.965 against the published 0.94, 1.25 and 0.96
(bar 0.03), and 100 % availability globally and at the worst site against Table 3.8-1's
98 % and 88 %.

No file in this pack uses a workshop preset or its numbers; `tests/campaign_watch.rs`
scans every pack for the preset's name and fails if it appears.
