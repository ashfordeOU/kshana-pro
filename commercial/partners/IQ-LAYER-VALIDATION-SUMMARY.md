# Kshana GNSS IQ layer — validation summary (release 0.32.0)

*Generic, reusable partner brief. Grounded only in the public `ashfordeOU/kshana` repository at tag
**`v0.32.0`** (commit `f570219e`, released 2026-10-05; `pip install kshana==0.32.0`). File paths below
are in that tree (sources: `CHANGELOG.md`, `docs/design/GNSS-IQ-PLAN.md`, `docs/VERIFICATION-MATRIX.md`,
`kshana.pyi`, and the tests named below). Marks: **[BUILT]** code ships in 0.32.0 · **[TESTED]** checked in the 0.32.0 test
suite against an independent reference (ICD table, closed form, third-party program, or injected
truth) · **[PLANNED]** not in 0.32.0. "TESTED" is not the engine's own ledger grade: the
verification matrix (`docs/VERIFICATION-MATRIX.md`) grades each row MODELLED or VALIDATED, and is
quoted where a row exists.*

## What it is
A **software-only** GNSS signal-level layer (`src/iq/`): it writes and reads IQ files for software
receivers and processes them, end to end, from the CLI (`kshana iq …`) and Python. No hardware,
no transmit path.

| Capability | Surface (0.32.0) | Mark |
| --- | --- | --- |
| **Scene** — long multi-satellite IQ with a per-epoch truth sidecar (code phase, Doppler, range, C/N0) | `iq scene` · `kshana.iq_scene` | [BUILT] [TESTED] |
| **Broadcast-ephemeris scene** — RINEX nav + receiver position → true per-satellite geometry | `iq scene --nav` · `iq_scene_broadcast` | [BUILT] [TESTED] |
| **Channel** — iono (STEC/VTEC/Klobuchar), Saastamoinen/Niell tropo, Cornell scintillation, specular multipath, 3-state land-mobile, NLOS | `iq scene --iono-* --tropo --s4 …` | [BUILT] [TESTED] (models MODELLED) |
| **Acquire** — FFT search, chi-square threshold for a search-wide P<sub>fa</sub> | `iq acquire` · `iq_acquire` | [BUILT] [TESTED] |
| **Track** — multi-channel DLL/PLL/FLL bank, NWPR + Beaulieu C/N0, bit sync; replay one file through many loop designs | `iq track` / `iq sweep` · `iq_track` | [BUILT] [TESTED] |
| **Front end** — band-pass FIR, adaptive notch, pulse blanking, FD excision, AGC, 1/2/3/8/14-bit quantiser | `iq frontend` · `iq_frontend` | [BUILT] [TESTED] |
| **Lab fit** — fit loss-of-lock model to a real receiver's trust timeline, leave-one-run-out hold-out, extrapolation labelled | `iq labfit` · `iq_labfit` | [BUILT] [TESTED] on synthetic runs |
| **Data / SigMF** — int8/int16/float32/2-bit/real-IF, multi-file SigMF, resampling, SHA-256 inventory, batch runner | `iq inventory/convert/extract/decimate`, `--format sigmf` | [BUILT] [TESTED] |
| **Signals** — GPS L1 C/A, L5, L2C; Galileo E1 B/C, E5a; BeiDou B1I, B1C; GLONASS L1OF | `iq_signals()` | [BUILT] [TESTED] vs ICDs |

## Validation basis (all at `v0.32.0`)
*Re-run 2026-10-07 at the tag (Linux x86_64, Rust 1.93.0, `cargo test --release` on the ten test files named below): **99 passed, 0 failed, 1 ignored** (the strict gps-sdr-sim test, ignored by design).*

- **gps-sdr-sim cross-generator** — `tests/gps_l1ca_gpssdrsim_cross_generator.rs`, pre-registered with
  a bit-exact bar, gps-sdr-sim (MIT, commit `28ca29a6`) run as a separate program on its `brdc0010.22n`
  (32 satellites). **0 of 32 736 C/A chips differ; parity of 0 of 1 920 words differs** [TESTED]. 230 of
  1 152 computed LNAV fields differ by one LSB because gps-sdr-sim truncates where IS-GPS-200 needs
  rounding — pinned as a finding that runs in CI; the strict all-fields test is `#[ignore]`d with the
  gap. The ledger row exists ("GPS L1 C/A cross-generator conformance") and is **MODELLED** because the
  pre-registered bit-exact bar (`f88f2969`) failed on those fields; promotion is [PLANNED].
  RTKLIB decodes the same encoder's integers 608/608 (`tests/gps_lnav_rtklib_integer_oracle.rs`).
- **Receiver vs closed forms** — `tests/iq_receiver.rs` (16 tests) [TESTED]: FFT vs direct DFT to 1e-9;
  empirical P<sub>fa</sub> and P<sub>d</sub> vs square-law / Marcum-Q within 4 binomial σ; loop B<sub>n</sub> vs
  Kaplan & Hegarty Table 5.6 within 6 %; PLL jitter vs thermal-noise closed form within 12 %; 2nd-order
  ramp error within 1 %; DLL jitter within 15 %; NWPR/Beaulieu C/N0 unbiased to 0.3 dB at 35/45 dB-Hz.
- **Scene vs independent receiver code** — `tests/iq_scene_receiver.rs` [TESTED]: the pre-existing
  `sdr::acquire/track` finds each injected satellite (refined to 0.05 chip, 2 Hz); correlation power
  matches injected C/N0 (tolerance 0.3 dB at 45, 0.6 dB at 38 dB-Hz); a broadcast scene's pseudoranges
  solve back through the engine's SPP to < 1 mm.
- **Signals vs ICDs** — `tests/iq_signals.rs` (23 tests) [TESTED]: chips quoted from IS-GPS-200N,
  IS-GPS-705J, Galileo OS SIS ICD Annex C, BDS SIS ICD B1C/B1I, GLONASS ICD — never regenerated.
- **Streaming / large data** — `tests/iq_io.rs` (12) and `tests/iq_frontend.rs` (19) [TESTED]: every
  format round-trips bit-exact; chunked processing (any chunk size) is bit-identical to one-shot; a
  multi-MiB file read in 4 KiB chunks equals one-shot in bounded memory; resampler images/aliases
  more than 75 dB down; Kaiser/Butterworth stages vs their closed forms.
- **SigMF + Welch** — `tests/sigmf_welch_oracle.rs` [TESTED], matrix row **VALIDATED**: Welch PSD vs
  SciPy 1.18.1 within 1e-12 relative over 27 cases (observed 6.0e-14); sigmf-python 1.13.0 recordings
  read bit-identically and vice-versa; metadata valid against SigMF v1.2.6 schema.
- **CLI/Python end to end** — `tests/iq_cli.rs` (10) [TESTED]: acquire/track recover the scene's
  injected truth, SigMF round-trip, broadcast scene, front-end flags, sweep ordering; Python stub
  checked against bindings by `mypy.stubtest` in CI.
- **Worked example on the published wheel** — `examples/iq-notebook/` in this repository [TESTED] (synthetic scene):
  three PRNs acquired within 0.12 chip; tracked Doppler error ≈ 0.01 Hz (σ ≈ 1.3 Hz), C/N0 within
  0.8 dB of the injected 45 dB-Hz.

## Explicit limits
- **Software only; no transmit**; no output drives radio hardware. **No interference or spoofer
  waveform synthesis** — channel effects act on the legitimate signal only; jamming/spoofing remain
  analytic models elsewhere in the engine.
- **Not in WASM/browser or MCP in 0.32.0** (`iq::cli` is `cfg(not(wasm32))`; no `iq_*` in `src/wasm.rs`
  or the MCP server). MCP exposure and a WASM demo are **[PLANNED]**.
- **Tracking hand-off default:** `iq_track` with `acq_coherent=1` (≈ 667 Hz bins) can false-lock ≈ 500 Hz
  off; use `acq_coherent=2` or finer Doppler bins.
- **No new VALIDATED ledger rows** for acquisition/tracking/channel in 0.32.0; channel and loop
  physics stay MODELLED. Real-capture evidence (LuGRE lunar IQ vs
  GNSS-SDR) is recorded as findings, not validations.
