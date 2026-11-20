# Receiver-test partner — gap analysis and build plan (name-free)

**Status:** ANALYSIS ONLY — nothing here is built yet. For coordinator + founder review.
**Against:** `RECEIVER-TEST-PARTNER-REQUIREMENTS.md` (R1–R9 and the binding no-synthesis line).
**Baseline:** public `kshana` tag `v0.32.0` (`e647b1e7`) plus open public PRs **#34** (MCP IQ tools,
head `a68eaf9`), **#35** (IQ ledger rows, head `08461c5`), **#38** (`iq track` acquisition default,
head `d8bf5be`). Private overlay: this repo at `main` (engine pin `=0.32.0`).
Paths are in the public repo at `v0.32.0` unless marked *pro:* or *PR #n*.

**Legend.** Status: **HAVE** (built, usable from CLI/Python) · **PARTIAL** · **GAP**.
Size: **S** ≤ 3 days · **M** ≤ 2 weeks · **L** > 2 weeks (one engineer-agent, incl. tests and docs).
Surface: **E** engine library · **CLI** · **Py** Python · **MCP** · **Pro** private overlay.
Every build item lists the test or validation basis it must carry. Nothing is planned that synthesises
jammer or spoofer waveforms or transmits; items near that line are listed separately as
**FOUNDER DECISION** (§12), not planned.

---

## 1. One-screen summary

| # | Requirement | Status at 0.32.0 (+PRs) | Biggest gap | First build item |
|---|---|---|---|---|
| R1 | Long multi-satellite raw IQ | **HAVE** for one signal per file (11 signals), streamed, deterministic | One signal per file on CLI/Py; only GPS L1 C/A tested end to end; nav data is real only for GPS LNAV | B1.1 per-signal end-to-end tests; B1.2 multi-signal scenes |
| R2 | Channel effects inside the IQ | **HAVE** (iono, tropo, Cornell scintillation, specular multipath, land-mobile, NLOS), all MODELLED | Not checked against measured channel data; single reflector; one carrier for the channel | B2.1 measured-scintillation check; B2.2 multi-reflector |
| R3 | Loops you can see inside and change | **PARTIAL** — rich in Rust, narrow on CLI/Py | Orders, discriminators, lock thresholds Rust-only; E/P/L only; no re-acquisition; E/L taps not exported | B3.1 loop-design file; B3.2 full epoch export |
| R4 | How loops respond to jammers/spoofers | **PARTIAL** — receiver side only; their recordings are the stimulus | Front-end flags silently ignored by `iq track`/`sweep`; no IQ-domain detection monitors | B4.1 front end in track/sweep; B4.2 IQ monitors |
| R5 | Their recordings × many loop designs, overnight | **PARTIAL** — `iq sweep` replays one file through a flag product | One file, single-threaded, no design file, no truth, no resume, memory grows with length | B5.1 campaign runner |
| R6 | Large recordings | **PARTIAL** — reader streams in bounded memory | Tracking keeps every epoch in RAM (measured 230 MB / 60 s / 8 ch); no GB-scale test; MCP budget 50 M samples | B6.1 streaming epoch sink |
| R7 | Calibrate to their lab data with stated error | **PARTIAL** — `iq labfit` with hold-out and PREDICTION labels | Fits receiver *logs*, not IQ-derived timelines; never run on real data with C/N0 | B7.1 labfit from IQ campaigns |
| R8 | Automated scoring vs known test conditions | **GAP** | No event/truth file consumed for scoring on IQ; no loss-of-lock / re-acq / false-lock metrics | B8.1 truth-annotated scoring |
| R9 | Their formats and signals | **PARTIAL** — ci8/ci16/cf32/2-bit/real-IF, SigMF, raw+sidecar | No 4-bit/unsigned-8/12-bit; `.sdrx` reader not wired to `iq`; no vendor header importers | B9.1 format breadth; questions in §13 |

**Measured today (this container, §2):** scene generation **≈ 3.3 M complex samples/s** with 4 threads for
8 satellites (≈ 1.4 M/s single thread), bounded at **≈ 11 MB RSS**, bit-identical across thread
counts; tracking **≈ 2.3 M samples/s for 8 channels** (≈ 17 M/s for 1 channel), RSS growing with
recording length.

---

## 2. Measured performance (fresh, 2026-10-07)

Environment: Linux x86_64 container, 4 vCPU, `cargo build --release` of `v0.32.0`, Rust 1.93.0.
Scene: GPS L1 C/A, 8 PRNs, 45 dB-Hz, 4.092 MHz complex, 60 s, cf32 output (1.96 GB) + truth CSV (41 MB).

| Job | Wall | Throughput | Peak RSS | Notes |
|---|---|---|---|---|
| `iq scene` 60 s, `--threads 1` | 174.9 s | 1.40 M samples/s (0.34× real time) | 10 MB | |
| `iq scene` 60 s, `--threads 4` | 75.2 s | 3.27 M samples/s (0.80× real time) | 11 MB | output byte-identical to threads 1 (`cmp`) |
| `iq track` 60 s, 8 PRNs (`--acq-coherent 2`) | 108.0 s | 2.27 M samples/s (0.56× real time) | 230 MB | 479 992 epoch rows; phase lock 99.5–99.7 % per PRN; mean Doppler after 30 s equals truth to 0.1 Hz |
| `iq track` 60 s, 1 PRN | 14.6 s | 16.8 M samples/s (4.1× real time) | 35 MB | |
| `iq sweep` 60 s, 1 PRN, 2×2 designs | 52.5 s | 4 designs ≈ 4.7 M samples/s·design | 55 MB | single reader, single thread |

**Extrapolations (unverified, linear):** a 1-hour 8-satellite scene at 4.092 MHz takes ≈ 75 min on 4
threads and writes ≈ 118 GB cf32 (≈ 59 GB ci16 with a gain fix, B1.4). Tracking 1 hour × 8 channels
would need ≈ 14 GB RAM today (≈ 0.48 MB per channel-second) — the R6 blocker B6.1 removes.

---

## 3. R1 — Long, multi-satellite raw IQ

**(a) Today.**
- `iq scene` / `kshana.iq_scene` generate multi-satellite IQ with a 1 ms per-satellite truth sidecar
  (CSV/JSONL; code phase, Doppler, range, C/N0) — `src/iq/cli/scene.rs`, `src/iq/scene/truth.rs:44`.
  Chunked generation in bounded memory, "independent of the duration" (`src/iq/scene/mod.rs:379-381`);
  `chunked_and_threaded_output_equals_one_shot_bit_for_bit` (`src/iq/scene/tests.rs:211`). No duration cap
  (must not cross a GPS week, `mod.rs:141-144`). Measured §2.
- 11 signals generated and accepted by acquire/track via one `build_code` (`src/iq/cli/signal.rs:11-62`):
  GPS L1 C/A, L5 I/Q, L2C; Galileo E1-B/E1-C (CBOC), E5a-I/Q; BeiDou B1I, B1C (data component);
  GLONASS L1OF (FDMA k = −7…6). Codes checked against ICDs: `tests/iq_signals.rs` (23 tests, TESTED).
- Broadcast-ephemeris scenes from RINEX 3 GPS nav (`iq scene --nav --rx-pos`, `iq_scene_broadcast`);
  pseudoranges solve back through the engine SPP to < 1 mm (`tests/iq_scene_receiver.rs::broadcast_scene_pseudoranges_solve_back_to_the_receiver`).
- *PR #35*: IQ scene geometry row **VALIDATED** against gps-sdr-sim (worst 1.6 mm, 5.5e-6 chip, 7.4e-5 Hz;
  GPS L1 C/A, static receiver, no channel/clock).

**(b) Gaps.**
1. **Only GPS L1 C/A is tested end to end** (scene → acquire → track); every `tests/iq_cli.rs` test uses `gps-l1ca`.
   The other 10 signals are ICD-checked codes only.
2. **One signal per file on CLI/Python** (`cli/scene.rs:70-74`). The Rust `Scene` can mix codes per satellite
   (`scene/mod.rs:483`) but nothing tests it; multi-frequency (L1+L5) needs separate files or a very wide fs.
3. **Navigation data**: real message only for GPS LNAV (subframes 4–5 empty, `scene/nav.rs:31-39`). `--data`
   applies 50 bit/s seeded bits to **every** signal, including pilots and Galileo/BeiDou at the wrong symbol
   rate (`LNAV_BIT_S`, `scene/mod.rs:757-758`). No I/NAV, F/NAV, CNAV, D1/D2, B-CNAV1, GLONASS meander.
4. **Absent signals**: E5b, E5 AltBOC, E6, L1C, B2a, B3I, GLONASS L2/CDMA (`docs/design/iq-notes/signals.md:209-212`);
   no E1-B+C or L5 I+Q composite; B1C pilot not exposed.
5. **Integer scene output collapses**: noise normalised to unit power and `IqWriter` scale 1, so ci8/ci16 output is
   ≈ {−1, 0, 1}; saturation count not reported (`io/stream.rs:208-210`, `scene/mod.rs:104-106`). No gain flag.
6. Receiver dynamics, receiver clock drift, per-satellite C/N0 and elevation C/N0 exist in Rust
   (`geometry.rs:21-34,123-136,276-303`) but not on CLI/Python; no clock phase-noise model.
7. Python returns the whole scene as float lists in memory (`python.rs:337-358`) — no file-writing path.
8. No check that fs covers the signal bandwidth or the FDMA spread (`SceneConfig::validate`, `scene/mod.rs:198`).

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B1.1 | End-to-end scene → acquire → track test per signal (11), incl. tiered-period hand-off | M | E, CLI | injected truth per signal; Doppler/code within stated bounds | PR #38 |
| B1.2 | Multi-signal, multi-constellation scenes: repeatable `--signal NAME:prns[:dopplers]` groups, explicit centre/IF, fs-bandwidth and FDMA-span validation | M | E, CLI, Py, MCP | truth recovery per group; refusal tests | B1.1 |
| B1.3 | Data fidelity: no data on pilots; correct symbol rate per signal; option to leave data off; document LNAV-only real message | S | E | per-signal bit-period test; pilot carries no transitions | — |
| B1.4 | Scene output gain/scale + saturation report for integer formats | S | E, CLI, Py | clipping count ≈ closed-form Gaussian tail; ci16 round-trip SNR | — |
| B1.5 | Expose per-satellite C/N0, elevation C/N0, receiver trajectory and clock drift on CLI/Py (scene TOML) | M | CLI, Py, MCP | existing Rust tests + CLI round-trip | — |
| B1.6 | Python file path: `iq_scene(..., out=path, format=...)` streaming to disk | S | Py | file == CLI output byte for byte | — |
| B1.7 | Composite signals (E1 B+C, L5 I+Q) and new codes (E5b, B2a, L1C) | L | E | ICD chip tables (as `iq_signals.rs`); PSD vs published BOC closed forms | B1.1; partner signal list (R9) |
| B1.8 | Galileo I/NAV and GPS CNAV message encoders (only if partner needs data demod) | L | E | ICD + RTKLIB/GNSS-SDR decode oracle (as `gps_lnav_rtklib_integer_oracle.rs`) | partner answer R9 |

---

## 4. R2 — Channel effects inside the IQ

**(a) Today.** All applied per satellite at 1 kHz knots through `SceneChannelAdapter`
(`src/iq/channel/scene_bridge.rs:105-145`), CLI flags and Python kwargs (`iq_scene` only):
first-order ionosphere with code-carrier divergence (`--iono-stec|--iono-vtec|--iono-klobuchar`;
`iono.rs:132-175` tests), Saastamoinen/Niell troposphere (`--tropo`; `tropo.rs:72`), Cornell
scintillation amplitude + phase, time-correlated per satellite (`--s4 --scint-tau0 --sigma-phi`;
S4 within 4 %, τ₀ within 5 %, `scint.rs:251-350`; in-scene S4 0.5 ± 0.12, `scene_bridge.rs:229`),
specular ground multipath with Fresnel coefficients (`--multipath-height --multipath-ground`;
`multipath.rs:240-307`), three-state land-mobile with Loo fading (`--land-mobile`), NLOS (`--nlos`).
All **MODELLED**: "None of it is checked against measured channel data" (`iq-notes/channel.md:3-5`).

**(b) Gaps.** (1) No comparison with measured scintillation/multipath; (2) one reflector on CLI, no
roughness/diffraction; land-mobile parameters "illustrative" and redrawn per step with no Doppler
spectrum (`channel.md:35-36,69`); (3) channel uses the first satellite's carrier for all satellites
(`cli/scene.rs:328-332`) — wrong for multi-frequency scenes (B1.2); (4) no frequency scaling of
scintillation; (5) channel kwargs missing from `iq_scene_broadcast`; (6) ionospheric rate not added to
Doppler (`channel.md:62-63`); (7) doc inconsistency (SplitMix64 vs ChaCha8, `scint.rs:42-44` vs `channel.md:27`).

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B2.1 | Scintillation check against a public measured dataset (S4/σφ time series → scene → tracked S4/σφ) | M | E | pre-registered bar, ledger row (MODELLED→VALIDATED candidate) | data availability |
| B2.2 | N-reflector multipath on CLI/TOML (delay, amplitude, phase, Doppler per path) + per-path truth | M | E, CLI, Py | closed-form correlation-triangle distortion; DLL bias vs the textbook multipath error envelope | — |
| B2.3 | Per-satellite carrier in the channel; frequency-scaled scintillation (S4 ∝ f^−1.5 option) | S | E | iono scaling test at L1/L5 | B1.2 |
| B2.4 | Channel kwargs on `iq_scene_broadcast`; iono rate into Doppler | S | Py, E | truth Doppler vs d/dt of iono delay | — |
| B2.5 | Land-mobile Doppler spectrum (isotropic-scattering) and ITU-R P.681 parameter sets | M | E | autocorrelation vs Bessel J0 closed form | — |

---

## 5. R3 — Acquisition and tracking loops they can see inside and change

**(a) Today.** FFT PCPS acquisition for every signal (`src/iq/acq.rs:130-255`), coherent/non-coherent,
χ² threshold for a search-wide P_fa, `second_peak`/`peak_ratio` reported (`acq.rs:86-117`); P_fa/P_d vs
square-law/Marcum-Q TESTED (`tests/iq_receiver.rs`). Tracking bank (`src/iq/track/`): PLL order 1–3,
FLL 1–2, FLL-assisted PLL, DLL 1–2 with carrier aiding; discriminators Costas atan / atan2 / decision-
directed, FLL cross-product / atan2 / pilot, DLL EML power / dot product / envelope (`discrim.rs`);
NWPR and Beaulieu C/N0; PLI lock; histogram bit sync. Closed-form tests: B_n within 6 %, PLL jitter 12 %,
DLL jitter 15 %, C/N0 0.3 dB (`tests/iq_receiver.rs`). CLI `iq track` flags `--pll-bw --fll-bw --dll-bw
--spacing --coherent --acq-coherent …`; Python `iq_track` kwargs similar. *PR #38* makes the hand-off use
≈ 4 ms of acquisition (≈ 167 Hz bins): false locks 27 → 1 and misses 102 → 11 of 180 seeded channels.

**(b) Gaps.**
1. **Rust-only knobs**: loop orders, pure PLL/FLL, discriminator choice, carrier aiding off, PLI/code-lock
   thresholds, C/N0 windows, bit-sync parameters; setting any bandwidth forces PLL 2nd / FLL 1st order
   (`cli/track.rs:116-123`, `python.rs:559-566`).
2. **E/P/L only** (`track/channel.rs:266-271`); no VE/VL, no multi-correlator bank (needed for SQM, B4.2).
3. **No loss-of-lock state machine and no re-acquisition**: PLI and code lock are flags only; a lost channel
   keeps running. No false-lock detector (the 500 Hz false lock reported a clean track).
4. Export drops E/L correlators, carrier phase, bit edges and bits (`cli/track.rs:177-180`, `python.rs:588-607`).
5. Full acquisition grid computed (`AcqGrid`, `acq.rs:120-126`) but never exported; no fine-Doppler step in `iq::acq`.
6. GLONASS: FDMA offset not applied in acquisition (user must set IF, `acq.rs:179`).
7. No vector tracking, no Kalman-filter tracking (`track/mod.rs:36-37`).
8. Default design not tied to a published reference receiver; `iq::track` has no external-oracle row
   (GNSS-SDR oracle exists only for the analytic `tracking_loop.rs`).
9. *PR #38 caveat*: "4 periods for a 1 ms code (L5)" — `period_s()` includes NH overlays, so L5-I gets 1 period (10 ms),
   L2C/E1-C one 1.5 s / 100 ms period. Worth a follow-up before 0.33.0 ships.

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B3.1 | **Loop-design file** (TOML): every `LoopConfig` field (orders, discriminators, aiding, thresholds, C/N0/bit-sync windows); accepted by `track`, `sweep`, Py, MCP; design hash in outputs | M | E, CLI, Py, MCP | round-trip; each discriminator vs closed form already in `iq_receiver.rs` | — |
| B3.2 | Full per-epoch export (E/P/L, discriminators, carrier phase, bit edges/bits, lock state) | S | CLI, Py | schema test; values equal in-memory `EpochOutput` | — |
| B3.3 | Lock state machine: loss-of-lock declaration (dwell), re-acquisition (narrow search around last Doppler), false-lock check (FLL/PLL ±k/(2T) test, bit-sync consistency) | M | E | seeded scenes with signal gaps → re-acq time vs truth; false-lock rate on PR #38's 180-channel set | PR #38 |
| B3.4 | Acquisition grid export (npy/CSV) + fine Doppler refinement in `iq::acq` | S | E, CLI, Py | grid peak == result; refinement vs injected Doppler (0.2 Hz RMS bar as `acquisition.rs`) | — |
| B3.5 | Multi-correlator bank (N taps, arbitrary spacing), VE/VL | M | E | correlation-triangle closed form incl. BOC; jitter tests unchanged | — |
| B3.6 | GLONASS FDMA offset applied automatically in acquire/track | S | E | k ≠ 0 scene acquired without manual IF | — |
| B3.7 | Tiered-period hand-off fix for L5/L2C/E1-C (primary-period acquisition with secondary handled after) | S | E | per-signal false-lock/miss sweep as PR #38 | PR #38, B1.1 |
| B3.8 | `iq::track` external-oracle row: same IQ through GNSS-SDR, compare jitter/C/N0/lock (pre-registered) | M | E | ledger row MODELLED→VALIDATED candidate | B3.2 |
| B3.9 | Vector tracking / EKF tracking (optional, research) | L | E | position/clock truth from broadcast scenes | B1.5, B3.3; partner interest |

---

## 6. R4 — How the loops respond to jammers and spoofers (receiver side only)

The stimulus is **their** RF recordings (their simulator generates the jammers/spoofers). Our job is the
instrumented receiver, mitigation front end, and detection/response metrics.

**(a) Today.** Front-end mitigation `iq::frontend`: band-pass FIR, adaptive notch (normalised-LMS), pulse
blanking, frequency-domain excision, AGC, 1–16-bit quantiser (`src/iq/cli/frontend.rs:155-197`;
19 closed-form tests, `tests/iq_frontend.rs`). Usable via `iq frontend` (file → file) and `iq acquire`.
Receiver-side detection exists **on logs** (`receiver_trust/monitors.rs`: C/N0 drop, AGC, jamming
indicator, loss of lock, position jump, RAIM, clock; JammerTest pre-registered pipeline reproduced on 8
onsets) and **on scalars** (`spoof_monitors.rs`: AGC, SQM ratio, parity RAIM, combined). An older C/A-only
IQ→SQM path exists in Rust (`realdata/iqif.rs:113-171`), no CLI.

**(b) Gaps.**
1. **Defect: `iq track` and `iq sweep` ignore the front-end flags** the usage text says they take
   (`cli/mod.rs:56`; `track.rs` and `sweep.rs` never build the chain) — no error is raised. Python `iq_track`
   has no front-end kwargs. So "loop response with notch/blanking on vs off" is not runnable in one step today.
2. **No IQ-domain monitors**: AGC gain time series, front-end counters (blanking duty, excised bins, notch
   frequency) are Rust final-state getters only (`agc.rs:61-63`, `blank.rs:39`, `fdaf.rs:78`, `notch.rs:52`);
   no SQM metrics from the tracking bank; no C/N0-drop detector on IQ; no multi-peak acquisition alarm; no
   spectral (Welch) monitor wired as a detector; no Doppler/code-phase consistency check.
3. No response metrics vs a stated stimulus (loss-of-lock time, re-acq time, C/N0 degradation vs J/S) — see R8.

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B4.1 | **Front end in `track`/`sweep`/Py/MCP** (and refuse unknown flags) — mitigation becomes a sweep axis | S | E, CLI, Py, MCP | track with chain == track on `iq frontend` output, byte for byte | — |
| B4.2 | **IQ detection monitors** emitted as time series: AGC gain, input power, blanking duty, excised bins, notch frequency, per-channel C/N0 drop, PLI, SQM (ratio, delta, asymmetry from B3.5 taps), acquisition `peak_ratio`/second-peak alarm, Welch PSD excess-power monitor | M | E, CLI, Py | each monitor vs a closed form on clean IQ (false-alarm rate at stated threshold, as `iq_receiver.rs` P_fa test); detection on their recordings scored in B8.1 | B3.5 (for SQM), B6.1 |
| B4.3 | Detection report: first-alarm epoch, kind, per-monitor ROC across a campaign (reuse `impairment_eval` ROC/AUC harness) | S | E, Pro | ROC harness already tested (matrix line 70) | B4.2, B8.1 |

---

## 7. R5 — Their recordings × many loop designs (overnight sweeps)

**(a) Today.** `iq sweep` replays one recording once through the Cartesian product of `--pll-bw
--dll-bw --spacing --coherent` (`src/iq/cli/sweep.rs:29-65`), reporting phase/code jitter (discriminator
std over the second half), lock fractions and mean C/N0 (`sweep.rs:89-108`). Measured: 4 designs over
60 s in 52.5 s (§2). Test `tests/iq_cli.rs::sweep_orders_lock_fraction_with_carrier_bandwidth`.
Batch runner exists (`iq/io/batch.rs`: bounded in-flight work, ordered results, per-item panic capture),
used only by `iq inventory`. Pro has resumable, checkpointed batch infrastructure (*pro:* `src/batch/uq`,
`runs.jsonl` checkpoint with study hash) but for scenario kinds only — `iq` is not a scenario kind.

**(b) Gaps.** one recording per run; single-threaded; no design file (FLL fixed at 10 Hz, orders fixed);
no front end (B4.1); no truth/event input; metrics are second-half averages, not time-resolved; no
resume/checkpoint; no provenance hashes or version stamp in sweep output; not in Python/MCP; memory grows
with length × designs × PRNs (R6).

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B5.1 | **Campaign runner** `kshana iq campaign <campaign.toml>`: recordings (glob/inventory) × loop designs (B3.1) × front-end chains × PRN sets; parallel across work items via `iq::io::batch`; checkpoint/resume (`runs.jsonl`); per-run SHA-256 of recording + design + engine version | M | E, CLI, Py | resume yields identical results to an uninterrupted run; results independent of worker count (as `batch_runner_orders_results_deterministically`) | B3.1, B4.1, B6.1 |
| B5.2 | Sweep/campaign in Python and MCP (MCP: path-only, budgeted, async job handle) | S | Py, MCP | as PR #34 round-trip tests | B5.1, PR #34 |
| B5.3 | Pro: overnight campaign orchestration + watch (reuse *pro:* `batch/uq` checkpointing, `pack_c` hash chain) and the lab report (§11) | M | Pro | campaign_watch-style byte-stable report test | B5.1 |

---

## 8. R6 — Large recordings

**(a) Today.** Reader streams in bounded memory (64 KiB default, `io/stream.rs:15`), header skip and
seek (`stream.rs:128,419-428`), multi-file SigMF collections with hash checks, resampling/decimation,
`iq inventory` with streamed SHA-256 and parallel workers; tests use a 4 MiB file
(`tests/iq_io.rs::chunked_read_of_a_multi_megabyte_file_equals_one_shot_in_bounded_memory`).
Scene writing is bounded (≈ 11 MB RSS for a 1.96 GB output, §2).

**(b) Gaps.** (1) **Tracking memory grows with length**: `TrackingBank::run` keeps every epoch
(`track/bank.rs:48-50`); measured 230 MB for 60 s × 8 channels → ≈ 14 GB/hour extrapolated; (2) no
GB-scale test; (3) tracking throughput 0.56× real time for 8 channels at 4 MHz on 4 vCPU — overnight
sweeps need parallelism across channels/designs (B5.1); (4) MCP default budget 50 M complex samples per
call (*PR #34*, ≈ 10 s at 5 MHz) blocks multi-GB work over MCP; (5) Python passes samples as lists (≈ 1.5 GB
RSS for 8 M samples in the notebook) — no zero-copy/NumPy or file path.

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B6.1 | **Streaming epoch sink** (CSV/Parquet-free JSONL or binary) + online metric accumulators; bounded RAM regardless of length | M | E, CLI, Py | RSS flat across 10 s vs 600 s test; output identical to in-memory path | — |
| B6.2 | Per-channel parallel tracking (channels independent after hand-off) | S | E | bit-identical to serial | B6.1 |
| B6.3 | GB-scale CI test (synthetic 4 GB file, `#[ignore]` + nightly) with throughput budget | S | E | RSS < bound; throughput logged | B6.1 |
| B6.4 | Python file-path APIs (`iq_track(path=...)`, NumPy buffer protocol in/out) | S | Py | equality with CLI | — |
| B6.5 | MCP long-job contract (job id, progress, result file; budget per job not per call) | M | MCP | as PR #34 tests | PR #34, B5.1 |

---

## 9. R7 — Calibrate models against their lab data, with stated prediction error

**(a) Today.** `iq labfit` / `kshana.iq_labfit(toml)` fits the tracking-loop loss-of-lock model (PLL
bandwidth, pull-in ratio, dwells) and an empirical model to receiver-trust timelines from UBX, RINEX 3,
Android, NMEA logs, with a level-calibration offset and J/S via `jamming::effective_cn0_dbhz`; grid +
Nelder-Mead + seeded bootstrap; leave-one-run-out / k-fold hold-out with RMS split into interpolation vs
extrapolation; predictions labelled `PREDICTION` with nearest tested run and distance
(`src/iq/labfit/fit.rs:226-238,300-334`; `tests/iq_labfit.rs` 11 tests on synthetic runs). JammerTest 2024
fixture is reported **unusable** (no C/N0) with its SHA-256, never fitted. Not yet validated on any real
receiver (`labfit/mod.rs:31-33`).

**(b) Gaps.** (1) inputs are receiver **logs**; it cannot fit to timelines **measured from their IQ**
through our own loops (the natural path once they hand us recordings + test conditions); (2) no real-data
fit yet; (3) UBX reader lacks RXM-RAWX/SFRBX; no Septentrio SBF / NovAtel / Trimble / RTCM ingest
(`receiver_trust/ingest.rs:476-481`) — matters if their DUT logs are proprietary; (4) jammer-type Q factors
used by the J/S path look inconsistent (see §11b defect D5).

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B7.1 | labfit input from campaign outputs (IQ-derived C/N0/lock timelines + test conditions from B8.1 truth) | M | E, CLI, Py | synthetic: known parameters recovered (as existing tests); hold-out error reported | B5.1, B8.1 |
| B7.2 | Real-data calibration study on partner data (pre-registered bars, hold-out error published to them) | M | Pro (study) → E ledger "Partner" row | pre-registration before data is opened | NDA, data |
| B7.3 | Receiver-log ingest breadth: UBX RXM-RAWX/SFRBX; SBF/NovAtel/RTCM MSM as the partner's DUT requires | M each | E | vendor sample files + RTKLIB convbin oracle | partner answer R9 |
| B7.4 | Resolve the jammer Q-factor table (D5) before any J/S-based calibration is shown | S | E | textbook §9.4 / published SSC closed forms (already VALIDATED in `spectrum`) | — |

---

## 10. R8 — Automated, repeatable scoring against known test conditions

**(a) Today.** Truth exists only for our own scenes (truth sidecar; SigMF annotations written and counted
by `inventory`, `inventory.rs:179`, never consumed). Event scoring exists only on logs: receiver-trust
`[[events]]` (onset, end, kind, predicted C/N0 drop) + `[compare]` tolerances (`docs/RECEIVER-TRUST.md:66-85`).
`iq sweep` metrics are second-half averages; no time-resolved score.

**(b) Gaps.** No test-condition file for recordings; no loss-of-lock time, re-acquisition time, C/N0
degradation vs stated J/S, false-lock rate, time-to-detect, false-alarm rate; no pass/fail against a
stated bar; no repeatability evidence (same inputs → same report hash).

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B8.1 | **Test-condition schema** per recording (TOML/JSON, or SigMF annotations): jammer/spoofer type, on/off times, power/J/S profile, PRNs affected, DUT; imported from their simulator logs where possible | S | E, CLI | schema round-trip; SigMF annotation import test | partner sample (R9) |
| B8.2 | **Scoring engine**: per channel and per event — time to loss of lock, time to re-acquire, C/N0 degradation vs stated J/S (with predicted curve as a *reference line* only, see §12), false-lock rate (Doppler vs truth or vs nominal pre-event), monitor time-to-detect and false-alarm rate; tolerances and pass/fail | M | E, CLI, Py | synthetic scenes with signal gaps and known truth (no interference synthesis): each metric vs injected truth | B3.3, B4.2, B6.1 |
| B8.3 | Repeatability: report digest over (recording SHA, design hash, test-condition hash, engine version); same inputs → same digest across OS | S | E, Pro | cross-platform golden (as `tests/cross_platform_golden.rs`) | B5.1 |

---

## 11. R9 — Signals and formats (answers pending)

**(a) Today.** Streaming formats `c|r` × `i8 | i16_le/be | f32_le/be | 2-bit {tc,sm,ob}×{msb,lsb}` with
`_qi` swap (`io/format.rs:253-290`); real IF → complex baseband (`RealIfToBaseband`); SigMF read/write
incl. collections (`io/sigmf_stream.rs`; Welch+SigMF **VALIDATED** vs SciPy/sigmf-python); raw files with a
JSON/TOML sidecar (`format, sample_rate_hz, center_hz, if_hz, header_bytes, …`, `inventory.rs:26-47`) or
CLI flags (`--format --rate --center --if --header`). ION GNSS SDR `.sdrx` parser handles any bit width,
incl. 4-bit (`realdata/ion_sdr.rs:391`) — but only for LuGRE. Receiver logs: UBX (NAV-PVT/TIMEGPS/SAT,
MON-RF), RINEX 3, Android, NMEA.

**(b) Gaps.** No 4-bit / 12-bit / unsigned 8-bit (RTL-style) / one-2-bit-sample-per-byte in `iq::io`;
`.sdrx` not wired to `iq`; multi-channel SigMF refused (`sigmf_stream.rs:87-90`) — a multi-frequency lab
recorder writes several streams; no importers for common lab-recorder or simulator-export headers (not
found: GNU Radio meta, UHD, HDF5, vendor formats); RINEX 4 refused (`rinex.rs:228-235`).

**(c) Build items.**
| Id | Item | Size | Surface | Test / validation basis | Deps |
|---|---|---|---|---|---|
| B9.1 | Format breadth: 4/12-bit, unsigned 8-bit, byte-per-sample 2-bit, offset-binary for all widths; `.sdrx` → `open_recording` | S | E, CLI | hand-built byte vectors (as 2-bit tests); ION SDR sample files | — |
| B9.2 | Multi-stream recordings (multi-channel SigMF / `.sdrx` multi-lane) → one stream per band | M | E | sigmf-python multi-channel fixture | B1.2 for multi-frequency |
| B9.3 | Importers for the partner's recorder/simulator metadata (decide after their answer) | S–M | E | their sample files | partner answer |
| B9.4 | RINEX 4 nav for broadcast scenes | S | E | IGS RINEX 4 file vs RINEX 3 of same day | — |

---

## 11b. Defects found during this analysis (fix regardless of the partner)

| Id | Defect | Evidence | Fix |
|---|---|---|---|
| D1 | `iq track`/`iq sweep` silently ignore front-end flags the usage advertises | `cli/mod.rs:56`; no chain in `track.rs`/`sweep.rs` (verified by grep) | B4.1 (or refuse the flags until then) |
| D2 | `--data` puts 50 bit/s bits on pilots and on Galileo/BeiDou at the LNAV rate | `scene/mod.rs:757-758` | B1.3 |
| D3 | Integer scene output quantises unit-power noise to ≈ {−1,0,1}; clipping unreported | `io/stream.rs:208-210`, `scene/mod.rs:104-106` | B1.4 |
| D4 | PR #38 auto hand-off is 1 period for tiered codes (L5-I 10 ms, L2C 1.5 s, E1-C 100 ms), contrary to its doc | `iq/mod.rs:87-89` vs PR #38 doc | B3.7 |
| D5 | Jammer Q factors: `jamming.rs:178-182` has broadband 1.0, CW 1.5; `docs/SPECTRUM.md` evidence cites Q = 1 (CW) and 1.5 (matched) from the standard textbook (Understanding GPS/GNSS, 3rd ed., §9.4) | verified both lines | B7.4 — reconcile against the VALIDATED SSC values |
| D6 | *PR #35 finding*: all 21 absent PRNs cross the acquisition threshold on gps-sdr-sim's noise-free 2-bit input | PR #35 pinned test | already recorded as a finding; acquisition threshold on quantised/noise-free input needs a CFAR option (part of B3.4) |
| D7 | GLONASS acquisition does not apply the FDMA offset | `acq.rs:179` | B3.6 |

---

## 12. FOUNDER DECISION — items at or near the no-synthesis line (NOT planned)

The binding line: **no jammer or spoofer RF generation, no over-the-air transmit**. Everything in §§3–11 stays on
the legitimate-signal / receiving / analysis side. The following would touch the line; each needs a founder decision.

| # | Candidate | What it is | Pros | Cons / risk |
|---|---|---|---|---|
| F1 | **Analytic interference models in scoring** — spectral-separation C/N0 degradation (`jamming::effective_cn0_dbhz`, `spectrum` SSC κ, already in the engine) used to draw a *predicted* C/N0-vs-J/S reference curve next to the measured one | Closed-form, no waveform; already shipped for the observable-level kinds | Gives the partner "measured vs predicted" per jammer type; feeds labfit (R7); SSC values already VALIDATED | It is a model *of* interference; must be labelled MODELLED; D5 must be fixed first; partner might read it as a jammer simulator claim |
| F2 | **Equivalent-noise C/N0 degradation inside our scenes** (raise the noise floor or scale signal power over time to emulate wideband-jammer-equivalent C/N0 loss) | Not a jammer waveform, but a stand-in for one | Lets us test B3.3/B8.2 scoring end to end without their recordings | Blurs the line ("Kshana simulates jamming"); synthetic C/N0 ramps can be made with `--cn0` profiles that are explicitly not interference — prefer that wording |
| F3 | **Duplicate-PRN / time-offset copies of a satellite in a scene** (a spoofer-shaped stimulus) | Synthesis of a spoofing signal | Would exercise spoof detectors (SQM, multi-peak) without lab data | **Crosses the line.** Recommend NO; use partner recordings only |
| F4 | **CW/chirp/pulsed waveforms added to IQ** | Jammer synthesis | Front-end mitigation tests without lab data | **Crosses the line.** Recommend NO; the frontend tests already use tones and noise as generic DSP test signals (`tests/iq_frontend.rs` header) — keep it there, never as a "jammer" product feature. **Exists already:** file-only analytic jammer IQ snapshots (CW, chirp, noise) via the `spectrum` kind's `[iq]` / `--export sigmf` since 0.29.0. The founder decided on 2026-10-07 to keep that existing feature; F4 as a NEW capability stays held pending counsel |
| F5 | **Exporting scenes in a format that feeds an RF simulator / SDR transmitter** | A path to transmit | Partner might ask to load our scenes into their simulator | Policy says no format that drives radio hardware (`docs/design/GNSS-IQ-PLAN.md:24-29`); decide explicitly |

**Correction (2026-10-07):** F3 and F4 are not purely hypothetical. The public engine already
synthesises interference and spoofing signals in software in two places, outside the IQ layer.
The `spectrum` kind's `[iq]` section, shipped since 0.29.0, draws CW, chirp and noise jammers
into an IQ snapshot that `--export sigmf` writes to a file. `spoof_capture` adds an in-memory
spoofer copy of GPS L1 C/A for its loop-capture model. Neither transmits. The facts and the
counsel questions are in `commercial/export-control/CLASSIFICATION-REQUEST-PACK.md` §2.1a (items
S4, S5, N3, N4). Founder decision (2026-10-07): keep these existing features and send the corrected pack to export-control counsel. F3, F4 and F5 as NEW capabilities stay held pending counsel.

Recommendation: approve **F1** (with D5 fixed and MODELLED labels) and the non-interference wording of **F2**
(C/N0 profiles only); decline **F3–F5**.

---

## 13. Prioritised release plan

**0.33.0 (already holds):** MCP IQ tools (#34), IQ ledger rows (#35), `iq track` hand-off default (#38).
Add if cheap: **D1 guard** (refuse front-end flags on track/sweep until B4.1) and **D4 doc/behaviour note**.

**0.34.0 — "Lab replay" (makes R4/R5/R6/R8 real on their recordings).**
B4.1 front end in track/sweep · B6.1 streaming epoch sink · B6.2 parallel channels · B3.1 loop-design file ·
B3.2 full epoch export · B3.3 lock state machine + re-acquisition + false-lock check · B8.1 test-condition schema ·
B8.2 scoring engine · B5.1 campaign runner (resume, hashes) · B4.2 IQ monitors (non-SQM part) ·
B9.1 format breadth · B6.3 GB-scale test · B8.3 repeatability digest. *(≈ 6–8 weeks.)*

**0.35.0 — "Breadth" (R1/R2/R3/R9 for multi-GNSS, multi-frequency).**
B1.1 per-signal end-to-end tests · B3.7 tiered hand-off · B3.6 GLONASS FDMA · B1.2 multi-signal scenes ·
B1.3 data fidelity · B1.4 integer gain · B1.5 scene TOML (dynamics, clock, per-sat C/N0) · B1.6/B6.4 Python
file paths · B2.2–B2.4 channel · B3.4 grid export + CFAR · B3.5 multi-correlator + SQM monitors · B9.2 multi-stream ·
B5.2/B6.5 Python + MCP campaigns. *(≈ 6–8 weeks.)*

**0.36.0 — "Calibrated" (R7, credibility).**
B7.1 labfit from IQ campaigns · B7.4 Q-factor reconcile · B3.8 `iq::track` vs GNSS-SDR oracle row · B2.1 measured
scintillation row · B7.3 receiver-log breadth as needed · B4.3 detection ROC report. *(≈ 4–6 weeks.)*

**Pro (parallel to 0.34–0.36):** B5.3 overnight orchestration + **lab campaign report** (HTML/PDF with provenance
digest, per-design scorecards, detection ROC, labfit predictions with hold-out error) built on *pro:*
`study --dossier` / `report_pdf.rs`; B7.2 partner calibration study under NDA.

**Later / on request:** B1.7 composite + new codes, B1.8 nav-message encoders, B2.5 land-mobile spectrum,
B3.9 vector/EKF tracking.

---

## 14. Demo to offer the partner

**Now (0.32.0 + PR #38 behaviour via `--acq-coherent 2`), 30 minutes, synthetic only:**
1. 60 s, 8-satellite GPS L1 C/A scene with scintillation (S4 0.5) and ground multipath; truth sidecar.
2. Acquisition grid + `iq track` on all 8: Doppler/C/N0/PLI vs truth (the notebook in `examples/iq-notebook/`).
3. `iq sweep` over PLL/DLL bandwidths on the scintillated scene: lock fraction vs bandwidth.
4. `iq frontend` notch/blanking/excision on a file, then `iq acquire` with the same chain.
5. Honest limits slide (this document's §11b and §12).

**After 0.34.0, on one of THEIR recordings (under NDA):** inventory + hashes → campaign of 3 front-end chains ×
6 loop designs × their PRNs overnight → scorecard per jammer event (time to lose lock, time to re-acquire, C/N0
drop vs their stated J/S, monitor time-to-detect) → repeatability digest. This is the meeting-winning demo for R4/R5/R8.

---

## 15. Questions for the partner (closes R9; drives B1.7, B9.3, B7.3)

1. Recorder: sample format (bits, signed/unsigned/offset, I/Q order, packing), sample rate, centre frequency, IF,
   single or multiple RF channels per file, file sizes and lengths, metadata/header format.
2. Signals and bands they need us to track (L1 C/A only? L5, L2C, E1, E5a/b, B1C, GLONASS?).
3. Test-condition records from their simulator (jammer/spoofer type, power profile, timing): what format can they export?
4. DUT receiver logs: RINEX, NMEA, or a proprietary binary (which)? With C/N0 and AGC?
5. Metrics they already use to grade receivers (to match B8.2 definitions), and pass/fail bars.
6. Compute: can they run a CLI/Python build on their own machines, or do they need a hosted run?

---

## 16. Traceability and status (as of 2026-10-07 21:10 UTC)

This section maps every requirement (R1–R9) and defect (D1–D7) to its deliverable, where that
deliverable lives (PR/branch @ sha) and its state. The status comes from reading the code on each
branch, not only the PR descriptions. Public `kshana` heads used:

| Ref | Head | Content |
|---|---|---|
| `main` | `096d07d` | 0.32.0 + #34 (MCP IQ tools, merged `b6d31a1`), #35, #37, #39 (`a02c136`), #40 (D3, `409eb45`), #41 (`c5747cc`), #42 (`096d07d`) |
| PR #38 | `48d382c` | auto hand-off ≈ 4 ms (D4 documented); MCP IQ tools are on `main` via #34 |
| PR #40 | merged (`409eb45`) | integer scene output scaled to the integer range (D3) |
| PR #43 | `b515174` | lab-recording formats, multi-stream, `.sdrx` (B9.1, B9.2) |
| PR #44 | `1c00e94` (critic: merge) | front-end flags applied in `iq track` / `iq sweep` (D1) |
| PR #45 | `3a82a8e` | track engine: loop-design TOML, epoch stream, lock state machine (contains #38) |
| PR #46 / branch `feat/034-campaign` | `aea8284a` (later `13c84c9f`) | test conditions, campaign runner on the track session, scorer, `data_class` required |
| kshana PR #51 | `4d4d9b87` | public signal matrix, D8–D10 findings (holds until after 0.33.0) |
| branch `feat/034-iq-monitors` | `85ce5e1` | IQ-path monitors, Rust library only (WIP) |
| kshana-pro PR 17 | `5039fe0`; re-pin on `a0853a8` | Pro lab campaign report (L1–L6), Mac CI green; pins once to the final engine |

States: **merged** · **open** (PR open) · **branch** (pushed, no PR) · **not started**.
Owners follow the coordinator's routing of 2026-10-07.

### 16.1 Requirements

| R | Deliverable | Where @ sha | State | Owner | Remaining gap (concrete) |
|---|---|---|---|---|---|
| R1 long multi-sat IQ | scene generator, 11 signals, streamed | `main` (0.31/0.32) | merged | — | B1.1 per-signal scene→acquire→track tests; B1.2 multi-signal/multi-constellation scenes; B1.5 dynamics/clock/per-sat C/N0 on CLI; B1.7 composites/new codes: **not started** |
| | integer output scaling (B1.4 / D3) | PR #40 (merged, `409eb45`) | merged | W5 | clipping count is free text, not written into SigMF meta (routed to W5 after D7) |
| | data fidelity (B1.3 / D2) | PR #48 @ fb27f6f | open | W5 | `--data` still puts 50 bit/s on pilots and on Galileo/BeiDou |
| R2 channel in IQ | iono, tropo, Cornell scintillation, multipath, land-mobile, NLOS | `main` | merged (MODELLED) | — | B2.1 measured-scintillation check, B2.2 N-reflector, B2.3 per-satellite carrier, B2.4 broadcast-scene channel kwargs, B2.5 land-mobile spectrum: **not started** |
| R3 loops visible + changeable | loop-design TOML (`kshana.loop-design/1`, every `LoopConfig` field), overrides recorded, design hash in outputs (B3.1) | PR #45 @ 50a494e | open | W1 | design hash fixed: canonical key-sorted JSON, pinned by a test (default design `0773c7ee…94ff`, @ 3a82a8e); the default design changes again with the `fll_assist` change |
| | full per-epoch export `kshana.track-epoch/1` (csv/jsonl/184-byte binary) + events + summary (B3.2) | PR #45 | open | W1 | — |
| | lock state machine PULL_IN/LOCKED/LOST/REACQ/RETIRED, false-lock alias check, re-acquisition (B3.3) | PR #45 | open | W1 | — |
| | tiered hand-off (B3.7 / D4) | PR #38 @ 48d382c | open | W1 | see D4 |
| | Python: `iq_track(design=…)`, `iq_loop_designs`, `iq_read_epochs`; MCP `iq_track` with design/epochs | PR #45 | open | W1 | **no Python or MCP `iq_sweep`**; no front-end options on Python/MCP `iq_track` (0.34, W1) |
| | acquisition grid export / fine Doppler (B3.4), multi-correlator / VE-VL taps (B3.5), GNSS-SDR oracle row for `iq::track` (B3.8), vector/EKF (B3.9) | — | **not started** | — | B3.5 also blocks the SQM pair monitor (below) |
| R4 response to jammers/spoofers (receive side) | front end in `track`/`sweep` (B4.1 / D1) | PR #44 @ 1c00e94 | open | W5 | CLI only (Python/MCP: W1, 0.34) |
| | IQ-path monitors: power/AGC, PSD excess, kurtosis, pulses, C/N0 CUSUM, SQM delta/ratio, lock (B4.2) | branch `feat/034-iq-monitors` @ 85ce5e1 | branch (WIP) | W8 | **Rust library only**: no CLI/Python/MCP; not fed by the campaign; no time-to-detect / false-alarm scoring; `cn0.rs:20` refers to a `kshana iq monitor` command that does not exist; SQM pair test gets no extra taps (needs B3.5); no closed-form Pfa for PSD excess and SQM thresholds; blanking duty / excised bins / notch frequency and acquisition multi-peak alarm not wired |
| | detection ROC report (B4.3) | — | not started | — | needs scored monitors |
| R5 recordings × designs, overnight | campaign runner `kshana iq campaign` (cells, resume, `runs.jsonl`, `hashes.json`, DIGEST), Python `iq_campaign*`, MCP `iq_campaign` + `iq_campaign_status` (B5.1, B5.2) | branch `feat/034-campaign` @ aea8284a (PR #46) | open | W7 | now scores on the track session (PR #45's lock machine); epochs per cell via `run.epochs` (csv/jsonl/bin, hashed into the cell); `--dry-run` prints counts, not keys; the last re-acquisition fixes (`fll_assist`, time-based window) arrive with W1's change |
| | Pro overnight orchestration + campaign watch (B5.3 / L8) | — | not started | W3 | waits for 0.34.0 |
| R6 large recordings | streaming epoch sink, bounded memory (B6.1) | PR #45 | open | W1 | — |
| | multi-GB streamed **reading** test (1 GiB, VmHWM < 64 MB, `#[ignore]`) | PR #43 @ b515174 | open | W8 | — |
| | per-channel parallel tracking (B6.2); GB-scale **tracking/campaign** perf test (B6.3) | — | **not started** | — | tracking was 0.56× real time for 8 channels (§2) |
| | Python file-path / NumPy APIs (B6.4); MCP long-job contract (B6.5) | partly: MCP `iq_campaign` is incremental within a sample budget | branch (WIP) | W7 | B6.4 not started |
| R7 calibration with stated error | `iq labfit` on receiver logs | `main` | merged (MODELLED) | — | **B7.1 labfit from IQ campaign timelines: not started**; B7.2 real-data study: blocked on NDA and counsel item N6; B7.3 log-format breadth: not started |
| R8 automated scoring | test-condition schema `kshana.test-conditions/1` + `kshana iq conditions` (B8.1) | branch `feat/034-campaign` | branch (WIP) | W7 | **no SigMF annotation import**; no importer for the lab's simulator logs (waits for their sample files) |
| | scorer: availability, time to loss, J/S at loss, re-acq time, C/N0 per J/S bin + MODELLED curve, false locks per locked hour, PLL/DLL jitter (B8.2) | branch `feat/034-campaign` | branch (WIP) | W7 | MODELLED curve reads Q from `jamming::q_factor` (single source; follows D5); false lock only with a truth file; no monitor time-to-detect |
| | repeatability DIGEST (B8.3); Pro report verifies it | branch `feat/034-campaign`; kshana-pro PR 17 | branch / open | W7, W3 | PR 17 mirrors the real `kshana.campaign-plan/1`, cell key and DIGEST byte form; `data_class` is now required in the plan, every cell and the scorecard (#46), and Pro cross-checks all three |
| R9 formats & signals | ci4/cu4/cu8/ci12, 2-bit per byte, multi-stream raw + multi-channel SigMF, `.sdrx` (B9.1, B9.2) | PR #43 | open | W8 | `.sdrx` subset (one lane, stream, block); no unsigned 12/16-bit; **no per-vendor header importers** (wait for the partner's answers, §15); RINEX 4 nav (B9.4) not started |

### 16.2 Defects

| D | Fix | Where @ sha | State | Owner | Note |
|---|---|---|---|---|---|
| D1 front-end flags ignored by `track`/`sweep` | applied to acquisition and tracking passes; byte-identical to `iq frontend` then `track` | PR #44 @ 1c00e94 | open | W5 | CLI fixed; Python/MCP `iq_track` have no front-end option (W1, 0.34) |
| D2 `--data` bits on pilots / wrong symbol rate | per-signal symbol timing; pilots and unmodelled signals refuse data | PR #48 @ fb27f6f | open | W5 | stacked on #44 |
| D3 integer scene output ≈ ±1, clipping unreported | scaled to ¼ full scale (ci8 31.75 LSB, ci16 8191.75 LSB, 2-bit thresholds at 1σ); clipping reported | PR #40 (merged, `409eb45`) | merged | W5 | clipping not in SigMF meta (W5, after D7) |
| D4 tiered codes get one acquisition period | rule stated and pinned per signal family (4 periods for 1 ms codes; 1 full tiered period for L5-I 10 ms, E1-C 100 ms, L2C 1.5 s) | PR #38 @ 48d382c | open | W1 | **documented, not changed** for tiered codes; whether a 1.5 s L2C hand-off is acceptable is a design decision still open |
| D5 jammer Q table vs `SPECTRUM.md` | decision (a2): CW/narrowband Q 1.5 → 1.0 in 0.33; broadband 2.0 and the lunar re-tune in 0.34 | branch `fix/iq-d5-jammer-q` (WIP) | branch | W5 | the campaign scorer's MODELLED curve reads Q from `jamming::q_factor` (single source), so the fix flows through |
| D6 absent PRNs cross threshold on noise-free 2-bit input | recorded as a pinned finding | `main` (#35) | merged (finding) | — | CFAR option (B3.4) not started |
| D7 GLONASS FDMA offset in acquisition | `SampleSpec::baseband_hz(carrier)` used by acquisition and tracking | PR #50 @ 2c4c038 | open | W5 | GLONASS `sat_id` (u32 wrap) is a documented 0.33 limitation; 0.34 plan: slot-number id plus a signed `freq_channel` field |
| D8 default acquisition step 2/(3T) vs FLL pull-in ±1/(4T): Galileo E1-B/C false-lock at ±1/(2T) | none yet | `tests/iq_signal_matrix.rs`, kshana PR #51 @ 4d4d9b87 (ignored tests `galileo_e1b`, `galileo_e1c`) | finding; fix not started | W1 | residual +83.3 Hz tracked at +125.02 Hz; M2M4 ≈40.2 dB-Hz; E1-C secondary agreement 51.87 %; the zero-residual control passes |
| D9 L2C CM (20 ms) cannot hold lock with the default loop (Bn·T 0.3 PLL / 0.2 FLL) | none yet | PR #51 (ignored test `gps_l2c`) | finding; fix not started | W1 | PLI −0.084, Doppler −99.71 Hz, M2M4 15.77 dB-Hz |
| D10 B1C C/N0 below nominal | none yet | PR #51 (ignored test `beidou_b1c`) | finding; cause not established | W1 | NWPR 42.55 dB-Hz (−2.45), M2M4 43.68 (−1.32); lock, code and Doppler pass |

Issue #36 (macOS-local failures of `cargo test --all`): the Mac probe found 10 failures out of 3823, all bit-exact or 1e-15 fixture pins that differ in the last bit between libm builds; no oracle disagreement. The fix is a tolerance change on the fixture pins only (W2, `fix/macos-fixture-tolerance`, WIP). One case (#6, solver iterate) is not last-bit and is handled as a separate, reviewed amendment. No pre-registered bar changes.

### 16.3 The real remaining gaps (nothing covers these yet), in priority order

1. **Campaign ↔ track-engine integration: done in #46.** The campaign now scores lock on the track
   session (PR #45's lock machine), so time to re-acquire and false-lock rate are the design note's
   metrics. Remaining: W1's `fll_assist` pull-in gate and time-based re-acquisition window, after
   which narrow-PLL channels should re-lock inside the grace period. Owners: W1 + W7.
2. **D5 before any J/S figure is shown.** The MODELLED reference curve in every campaign report reads
   the Q table, which D5 changes: CW/narrowband in 0.33, broadband and the lunar re-tune in 0.34.
3. **Detection on IQ is not reachable or scored (R4).** The monitors exist as a library. No CLI, no
   Python, no campaign hook, and no time-to-detect / false-alarm metric per stated event. Owner: W8.
4. **Epoch streams from campaigns** are wired in #46 (`run.epochs`: csv/jsonl/bin per cell, hashed into the
   cell); Pro verifies the files. Remaining: none beyond the final pin.
5. **Stating test conditions from lab artefacts (R8/R9).** There is no SigMF annotation import and no
   simulator-log importer. Every recording needs a hand-written conditions file.
6. **Breadth (R1/R9).** No per-signal end-to-end test beyond GPS L1 C/A, no multi-signal scenes, and D2
   and D7 not yet merged; the public signal matrix (#51) adds D8–D10 as findings (Galileo E1 false-lock,
   L2C, B1C C/N0). A multi-frequency or Galileo partner test would rest on untested paths.
7. **Calibration from IQ (R7, B7.1)** is not started; real-data calibration is also blocked on counsel (N6).
8. **Throughput (R6).** No parallel channels and no GB-scale tracking test. 8 channels run at about half
   real time on 4 vCPU (§2), so overnight sweeps rely on campaign-level parallelism only.
9. **Smaller items.**
   - No multi-correlator taps, so the SQM pair test is idle.
   - `data_class` is now required in #46 and cross-checked by Pro (done).
   - No Python sweep.
   - The design hash is canonical key-sorted JSON (fixed in #45).
   - `LAB-CAMPAIGN.md` is implemented in #46; recheck its Q-table text after D5.
