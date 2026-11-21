# NavIC test kit and integrity evidence

Research 2026-11-20 gaps, backlog B1605. Module `src/navic.rs`, command `kshana-pro navic`
(`src/cli/navic_cmd.rs`), feature `pro-navic-evidence` (Pro; `scaffold` free), tests
`tests/navic_evidence.rs` (SYNTHETIC logs).

* **L1 test-vector scaffold** (`navic scaffold`): cases for acquisition and tracking
  sensitivity, navigation-data decoding, time to first fix and health handling, every value
  `null` with a `source` to fill from the NavIC L1 SPS interface specification the user holds.
  No signal parameter or limit is shipped.
* **Health-aware receiver checks** (`navic health <sats.csv> <health.csv> <fixes.csv>`):
  satellites used while the user's health record marks them unhealthy, and NavIC-only fixes at
  epochs with fewer than four healthy NavIC satellites tracked (the 3-versus-4 case), with the
  number of such epochs.
* **NavIC, GPS and Galileo consistency** (`navic consistency <solutions.csv>`): per-epoch
  solutions of each system compared pairwise against `k·√(σa² + σb²)` (`--gate-k`, default 5);
  inconsistent episodes per pair. An episode on every pair with one system points at that system.
* **Timing evidence** (`navic timing <request.toml>`): the receiver's offsets against NavIC and
  GPS time; the NavIC-minus-GPS mean, RMS and peak-to-peak, MTIE of the NavIC offset, and the
  traceability chain as the user states it (each link to confirm against the operators'
  publications); signed.

MODELLED: checks on logs as supplied; nothing generates a signal or locates an emitter.
