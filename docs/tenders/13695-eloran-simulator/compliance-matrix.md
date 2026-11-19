<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
-->

# AO/1-13695 (EL1-132): requirements → capability compliance matrix (pre-ITT draft)

**This matrix was built without the Statement of Work.** The requirements come from what is public or
in our snapshot:

- **[D]** the esa-star tender description (snapshot 15 Nov 2026, tender 194887);
- **[W]** the EL1-132 task list in the NAVISP Work Plan 2026 Addendum, as quoted in vcprep research
  report 01 §1.1 [S2];
- **[C]** ESA's own critique of earlier simulators in the same Work Plan text (report 01 §0 item 4);
- **[K]** the key acceptance factors.

When `ews1-13695.pdf` is read, replace each line with the SoW requirement ID and wording, and add the
lines the SoW adds.

**Status key**
- **Met (code)**: implemented and tested in `crates/kshana-eloran` (kshana-pro main 6c222bc). "Met" means the
  function exists. It says nothing about meeting an ESA performance figure, which we do not have yet.
- **Partly**: part exists; the gap is named.
- **To build**: absent today; work-package effort in the cost skeleton.
- **Unknown until ITT read**: cannot be judged without the SoW.

**Evidence label** (from `crates/kshana-eloran/docs/VALIDATION.md`):
- **V** VALIDATED: reproduced against an independent published reference by an automated test.
- **M** MODELLED: internal tests only.
- **HW-U** untested on hardware.

| # | Requirement (source) | Kshana today (code reference) | Status | Evidence | Gap / action |
|---|---|---|---|---|---|
| R01 | Flexible, modular, future-proof simulator architecture [D][W] | Data-driven scene: chains, timebase, propagation behind one trait, outputs, interference (`scene.rs`, `scenario.rs`); TOML scenarios with SHA-256 on every output | Met (code) | M | Show modularity against the SoW's interface list; write the ICD |
| R02 | Review of present and future eLoran signal definitions; framework adaptable to evolving standards [W][C] | Signal definition is data (carrier, envelope, offsets, phase codes); LORAN-C per COMDTINST M16562.4A default (`signal.rs`) | Partly | V (pulse shape, ECD) · M (phase-code tables) | No automated test compares the phase codes with the spec table (known gap). Add SAE AS9990-family and 10th-pulse definitions once the documents are obtained (paid SAE standard; **unverified** whether ESA provides it, see Q6) |
| R03 | High-fidelity signal generation [D] | Analytic pulse evaluation on an integer-picosecond timeline; delay resolution independent of sample rate (`time.rs`, `scene.rs`) | Met (code) | V (pulse shape) · M (synthesis: 10 ns offset reproduced to ± 0.05 ns vs own truth) | Fidelity at a hardware connector not measured (R14, R22) |
| R04 | LORAN data channel (Eurofix, 9th pulse, future formats) [W] | Eurofix, 9th-pulse, generic PPM/tri-state/phase mappers; RS, CRC-24Q/16/14, HMAC; message scheduler; receive decoder (`ldc/`) | Partly | V (CRC-24Q, CRC-16, HMAC RFC 4231) · M (on-air formats) | Eurofix CRC-14 polynomial and 9th-pulse RS parameters NEED CONFIRMATION against source texts; message layouts are Kshana's own, not a standard's |
| R05 | Software-defined core plus hardware architecture [W] | Software core complete; `Device` trait with drivers for SoapySDR, UHD, Red Pitaya, Digilent, SCPI/LXI AWGs, file and virtual bench (`hw/`) | Partly | HW-U | Real drivers compile but **have never run on hardware**. Build and characterise the reference LF chain with the partner (WP4) |
| R06 | Synchronisation with commercial GNSS simulators via standard timing interfaces (1PPS / 10 MHz) [D][W] | Adapters with configurable command profiles for Skydel, Spirent, R&S SMBV/SMW, LabSat, gps-sdr-sim (`sync/`); start-epoch handshake; sync budget; Sync-Verify certificate from TIC readings (`sync/certify.rs`) | Partly | V (GPS calendar) · M (rest) | Not exercised against any real GNSS simulator; vendor command profiles to be confirmed against each manual; needs a partner lab (WP4) |
| R07 | Propagation: groundwave [D][W] | PF/SF/ASF; smooth earth (Sommerfeld–Norton–Wait + Wait/Bremmer residue series); Millington mixed paths over polygon/raster conductivity; terrain integral equation; P.832 atlases (`prop/`) | Met (code) | V (short-range field 300 mV/m; seawater SF vs Harris polynomial ≤ 20 ns, with one stated calibration) · M (land paths, long-range field) | Land-path ASF never compared with measurements (WP6) |
| R08 | Propagation: skywave and groundwave/skywave interference [W] | One/two-hop geometry, P.684-style field, day/night transition, Rayleigh/Rice fading, superposition per pulse (`prop/sky.rs`, `sky_fading.rs`) | Met (code) | M | Reflection-coefficient fit not read from P.684 curves; validate against recorded night signals |
| R09 | Environments: urban, rural, maritime, mountainous, desert, Arctic; coastal boundaries [W] | Urban effects (`prop/urban.rs`), terrain (`prop/terrain.rs`, `dem.rs`), conductivity maps and coastlines; scenario packs for maritime, rail, aviation, timing | Partly | M | No desert or Arctic pack yet; urban model unmeasured. Write packs per the SoW list (WP3) |
| R10 | Time-of-day and temporal variation [W] | Deterministic diurnal/seasonal ΔASF, sunrise transition, fading, clock noise | Partly | M | No stochastic or weather-driven ASF |
| R11 | Multi-chain [D][W] | Any number of chains on the Loran epoch, true superposition, cross-rate interference; GRI database (187 entries, sourced) | Met (code) | V (GRI range/resolution) · M (CRI) | Dual-rated transmitter blanking not modelled |
| R12 | Multi-time-reference: each chain on its own UTC(k); multiple GNSS time bases [D][W] | UTC(k) models (offset, drift, steps, noise), transmitter clocks, GPST/GST/BDT/GLONASST; BIPM Circular T import (`timebase.rs`, `timeref/`) | Met (code) | V (leap second, GNSS time constants) · M (clock models) | Sub-second clock noise not modelled |
| R13 | Dynamic receivers (not only static mode) [C] | Waypoint trajectories with heading; ship, rail and ≈ 70 m/s aviation packs (`traj.rs`) | Partly | M | Receiver state is constant within each block (default 10 ms); height not used by propagation. Add high-dynamic and drone profiles if the SoW requires them |
| R14 | Record and replay [D][W] | SigMF record/replay, segments, decimation, looped replay (`replay/`); coherent LF + L-band capture sets on one timeline with PPS markers (`capset/`) | Partly | M | Capture from real hardware not verified; capture-set marker check works on simulated streams only (WP5) |
| R15 | Receiver Analysis and Validation Tools [D][W] | Software receiver: acquisition, tracking, cycle ID, ECD, SNR, position/time/TDoA, LDC hook (`rx/`); real-time receiver (`rt/`); fidelity comparator (`valid/fidelity.rs`); pre-registered campaigns with hash-chained manifests (`campaign/`); ELOF observation format and eLoran-Bench scoring (`elof/`, `refbench/`) | Met (code) | M | Receiver not compared with a reference receiver or live data; batch processing |
| R16 | End-to-end validation of hybrid / resilient PNT solutions (timing, maritime, other) [D] | Hybrid eLoran/GNSS fusion with cross-system integrity (`src/eloran_fusion.rs`), GNSS bridge on the same trajectory and time base, timing protection level (`tpl/`) | Partly | M | Demonstrate with a third-party hybrid receiver (partner) |
| R17 | Fidelity validation against live signals in representative environments [W] | Fidelity comparator at signal/observable/solution level; field recording station (`field/`); cross-check harnesses that can promote a model to VALIDATED (`valid/crosscheck.rs`) | To build (campaign) | — | **No live eLoran capture exists.** Plan the campaign with the partner (WP6); site in Anthorn coverage or another transmitter (Q5) |
| R18 | Interference and noise (CW, CRI, atmospheric, man-made) | CW, WGN, pulsed, band-limited, impulsive (incl. railway), P.372 atmospheric with Middleton Class A (`interf/`) | Met (code) | M | Atmospheric table and presets are representative, not measured |
| R19 | Calibration of the simulator output (delay, gain) | Calibration train, delay characterisation, calibrated sink, TIC/GUM receiver-delay budget (`hw/calib.rs`, `accept/calibrate.rs`) | Partly | M | Verified only on synthetic loopbacks |
| R20 | Repeatability / configuration management | Bit-for-bit deterministic from scenario + seed; scenario SHA-256 in every output; signed evidence (`kshana-pro dossier sign`) | Met (code) | M | Cross-platform last-bit differences possible |
| R21 | User interface | CLI, HTTP service (`src/eloran_service.rs`), Desktop Pro eLoran page (`apps/desktop-pro/pro/desktop/eloran.mjs`) | Partly | — | The validation ledger still lists "GUI: Not built" (it predates the Desktop page). Confirm what the SoW requires (operator GUI, remote control, scripting) |
| R22 | Performance figures at the RF connector (timing accuracy, amplitude, spectral purity, real time) | Real-time factor reported per run; propagation < 10% of real time for 10 stations in release builds | Unknown until ITT read | — | Hardware figures need measurement. The SoW will set numbers; check each against the partner's chain |
| R23 | Hardware deliverable (unit to ESA?) and target TRL | — | Unknown until ITT read | — | Q3. Cost depends on it |
| R24 | Interoperability with the UK PNT Digital Twin (NAVISP EL3-036bis): datasets "could support future validation steps" [W] | ELOF v0.1 observation/scenario format (`elof/`, spec `crates/kshana-eloran/docs/ELOF.md`); SigMF data | Partly | M | Agree the format with ESA (Q7); EL3-036bis prime not identified |
| R25 | Roadmap from EL1 to higher-TRL equipment (EL2) [W] | Network planner, national digital twin, bench-readiness tools exist as starting points | To build (document) | — | WP7 deliverable |
| R26 | Product assurance / software standards (ECSS-E-ST-40, ECSS-Q-ST-80 tailoring) | CI: format, strict clippy, full test suite on every push; nightly virtual-bench/acceptance/campaign run (`.github/workflows/eloran.yml`); about 400 test functions in the crate | Partly | — | Tailoring unknown until ITT read; write the SDP/SVVP/SPAP per the SoW |
| R27 | Deliverable documentation (requirements, design, test, user manuals, final report, executive summary) | `docs/ELORAN.md`, `crates/kshana-eloran/docs/VALIDATION.md`, `ELOF.md`, scenario-pack headers | Partly | — | ESA document list unknown until ITT read |
| R28 | IP / licence of delivered software; source delivery | Proprietary crate (`publish = false`); commercial licence; open core AGPL-3.0 dual-licensed | Unknown until ITT read | — | Read `ecc1-13695.pdf` IP clauses; declare background IP; Q2 |
| R29 | Threat scenarios (LF jamming, spoofing, meaconing, forged data messages) | `threat/` models; Enterprise vetted-buyer gating required before exposing them | Unknown until ITT read | M | Offer only if the SoW asks (Q9); export-control review first |
| R30 | Eligibility, delegate statement, binding FFP price, within budget [K] | — | To build (admin) | — | Gates G1–G2 in the pursuit tracker |

## Summary (pre-ITT)

| Status | Count | Lines |
|---|---|---|
| Met (code) | 9 | R01, R03, R07, R08, R11, R12, R15, R18, R20 |
| Partly | 14 | R02, R04, R05, R06, R09, R10, R13, R14, R16, R19, R21, R24, R26, R27 |
| To build | 3 | R17 (live campaign), R25 (roadmap), R30 (admin) |
| Unknown until ITT read | 4 | R22, R23, R28, R29 |
| **Total** | **30** | |

**Read-across.** The software core is the strong part (≈ 76k lines of Rust in the crate). The weak parts
are everything that touches the physical world: hardware output, GNSS-simulator sync on a real bench,
and live-signal fidelity. That is exactly what the partner must bring and what WP4 and WP6 fund.
