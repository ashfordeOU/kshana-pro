# Sealed incident capture and chain of custody

Research 2026-11-20 gaps, backlog B0604. Module `src/custody/`, command `kshana-pro custody`
(`src/cli/custody_cmd.rs`), tests `tests/custody.rs` (SYNTHETIC recordings).

| Command | Feature | What it does |
|---|---|---|
| `custody capture <request.toml> --out <dir> --key-file <file>` | `pro-custody-capture` (Pro) | Freeze a ring buffer around a trigger; sealed SigMF capture; custody form |
| `custody handover <dir> --from --to --purpose --key-file` | free | Append a signed, hash-chained handover |
| `custody verify <dir> [--trust <key>]` | free | Check data, seal, form and chain |
| `custody export <dir> [--json <out>]` | free | The whole chain as one JSON document |

## Capture

The source SigMF recording (`cf32_le`, `ci16_le` or `ci8`) is streamed through a ring buffer of
`pre_s` seconds. At the trigger, a stated `sample` or the first block whose power rises
`threshold_db` over the mean of the previous `baseline_blocks` blocks, the ring is frozen and
`post_s` more seconds are taken. Sample bytes are copied as they are, never re-encoded.

## Seal and custody form

`capture.sigmf-meta` carries, in the `kshana_custody` extension namespace: the calibration ID,
the source data's SHA-256 and first sample, the trigger, the capture data's SHA-256, the sealing
time, the sealer's public key and key id, and an Ed25519 signature over the canonical metadata
without the signature. `custody.json` binds the collector, organisation, location, reason and
time to the data and metadata hashes, signed by the collector.

## Handover log

`handover.json`: each entry (sequence, time, from, to, purpose, capture id) is chained to the
previous one by SHA-256 (the first to the custody form) and signed by the party handing over.
A handover is refused when the directory no longer verifies.

## Verification

Any changed byte of the data, metadata, form or log, a re-ordered or dropped handover, or a
form bound to other data fails. With `--trust`, the sealing key must be a trusted one.

Not built: a PDF of the chain (the export is JSON); a hardware-anchored seal (it would sit behind
the `unfiled-methods` feature). Nothing here locates an emitter or transmits.
