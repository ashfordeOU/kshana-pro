<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
DRAFT written before the SoW was read. Re-map every section to the ITT proposal template (ept1-13695.docx).
-->

# AO/1-13695 (EL1-132): technical proposal outline and draft text

**Status: pre-ITT draft.** The section numbering below is provisional. When `ept1-13695.docx` (Proposal
Template) and `ews1-13695.pdf` (SoW) are read, move the text into the template's structure and add SoW
requirement IDs. Text in *[brackets]* is a placeholder for the partner or for facts not yet known.

**Writing rules (do not break):**
- Label every quantitative claim **VALIDATED** or **MODELLED**, as in `crates/kshana-eloran/docs/VALIDATION.md`.
- Say that **nothing has yet run on hardware or on a live eLoran signal**.
- No "certified", "compliant", "proven", "field-proven". No patents claimed (none filed).
- Describe unfiled patent-candidate methods as outcomes only (see pursuit tracker §6).
- Compare other simulators on published facts only. Name no funded start-up competitor.

## Outline (maps to the evaluation criteria)

| Part / section | Content | Criterion | Draft below? |
|---|---|---|---|
| 1 Executive summary | One page: objective, approach, team, what exists today, what the contract adds | all | ✔ (§1) |
| 2 Understanding of the objectives and problem areas | Why hybrid GNSS/eLoran testing needs a new simulator; the problem areas ESA names; our reading of them | 55% | ✔ (§2) |
| 3 State of the art | Earlier simulators, commercial GNSS simulators, receivers; facts only | 55% | ✔ (§3) |
| 4 Proposed technical approach | 4.1 architecture · 4.2 signal definitions and data channel · 4.3 propagation · 4.4 multi-chain / multi-time-reference · 4.5 GNSS coupling and hardware chain · 4.6 receiver analysis and validation tools · 4.7 record and replay · 4.8 fidelity validation against live signals | 55% | ✔ 4.1, 4.3, 4.4, 4.6, 4.8; outline 4.2, 4.5, 4.7 |
| 5 Verification and validation philosophy | Evidence labels, pre-registered bars, signed campaign records | 55% | ✔ (§5) |
| 6 Programme of work | WPs, deliverables, milestones, schedule (see `wp-schedule-cost-skeleton.md`) | 55% / 30% | skeleton |
| 7 Risks | from `risk-register.md` | 55% | register |
| 8 Background and experience; facilities | Ashforde + *[partner]*; key persons; lab equipment | 15% | ✔ outline (§8) |
| 9 Management | organisation, reporting, PA/QA, configuration management | 30% | outline |
| 10 Background IP and licences | kshana-eloran as background IP, commercial licence; open core AGPL-3.0 (not used in delivered items unless agreed) | 30% (contract conditions) | outline |
| Financial / contractual parts | FFP price, PSS forms, delegate statement(s), cover letter | 30% | template only |

---

## §1 Executive summary (draft)

Hybrid GNSS/eLoran receivers and timing systems are being specified now, for the UK national eLoran
programme, the ROK–UK–France standards work and maritime use, but there is no flexible tool to test them
end to end. This activity delivers that tool: a software-defined eLoran/GNSS signal simulator that
synchronises with the commercial GNSS simulators laboratories already own, plus the receiver analysis and
validation tools to judge the results.

Ashforde does not start from zero. The simulator core already exists as Ashforde's proprietary
*Kshana Pro eLoran* engine, brought in as background IP. It covers:
- LORAN-C pulse synthesis on an exact integer-picosecond timeline;
- groundwave and skywave propagation with ASF over conductivity maps;
- any number of chains, each steered to its own UTC(k);
- the LORAN data channel (Eurofix, 9th pulse and configurable formats);
- a software receiver and a validation campaign runner.

Every model carries an evidence label. A small set is **VALIDATED** against independent published
references: the transmitted pulse shape against the USCG specification, the seawater secondary factor
against the Loran-C User Handbook polynomial, WGS-84 geodesy against Vincenty's test line, and CRC and
HMAC test vectors. The rest is **MODELLED**. **None of it has yet been run on hardware or compared
with a live eLoran signal.** Closing that gap is the purpose of this contract.

*[Partner]* brings *[LF/eLoran hardware heritage, a GNSS-simulator laboratory and access to live
signals]*. Together we will build and characterise a reference hardware chain, prove synchronisation with
*[two]* commercial GNSS simulators via 1PPS/10 MHz, and run a live-signal fidelity campaign with
pre-registered pass/fail bars. The contract ends with a validated simulator, open datasets for the
NAVISP EL3-036bis PNT Digital Twin, and a roadmap to EL2 equipment.

## §2 Understanding of the objectives and problem areas (draft)

ESA's Work Plan states why earlier integrated GNSS–eLoran simulators fell short. They were "hardware
centric and monolithic, region-specific (e.g. CONUS-focused), limited test scenarios (e.g. only static
mode), not easily adaptable to evolving eLoran standards, modern digital architectures, or modular GNSS
simulator coupling". We read this as six problem areas, and each one shapes our design.

1. **Hardware-centric and monolithic → software-defined core, thin hardware.** All signal, propagation,
   timing and data-channel physics run in software and are evaluated analytically at each sample instant.
   The hardware is a replaceable output stage behind one device interface (DAC/AWG/SDR, LF amplifier,
   antenna or H-field simulator). A lab can change the output hardware without touching the physics.
2. **Region-specific → global geometry and data.** Chains are data. A sourced GRI database (187 entries
   with a confidence value per entry, from the USCG Loran-C User Handbook, NGA Pub. 117 and others) and
   conductivity maps (polygons, rasters, ITU-R P.832 atlases) place any chain anywhere. Scenario packs
   copy real chain geometry unchanged and label everything else ILLUSTRATIVE.
3. **Static only → moving receivers.** Waypoint trajectories with heading (ship, train, aircraft), with
   crossed-loop H-field antennas that respond to heading. *[Add drone/high-dynamic profiles if the SoW
   asks.]*
4. **Fixed standards → signal definitions as configuration.** Carrier, envelope, pulse offsets and phase
   codes are data. New definitions (SAE AS9990-family, 9th/10th-pulse data channels, Eurofix) are added
   without code changes in the synthesis path. The first task reviews present and future definitions with
   ESA and records the chosen set.
5. **No modular GNSS coupling → standard timing interfaces plus verification.** Coupling uses 1PPS and
   10 MHz with a defined start-epoch handshake and per-vendor command profiles. The coupling is then
   *measured*: time-interval-counter readings give an alignment report with uncertainty (k = 2), so a lab
   knows the eLoran-to-GNSS offset rather than assuming it.
6. **Fidelity → measured against live signals.** A simulator is only useful if its output matches the
   real signal well enough for the test at hand. We define the fidelity metrics and their pass bars
   **before** the campaign (signal, observable and solution level), then record live signals and score
   the simulator against them.

Two further problem areas we expect the SoW to stress *[confirm against SoW]*:
- **Multi-time-reference**: international eLoran will run chains on different UTC(k) (UTC(NPL), UTC(OP),
  UTC(KRIS)) next to GNSS system times. Timing receivers must be tested against that mix.
- **Record and replay**: the activity must produce coherent LF and L-band recordings that others can
  replay, including the EL3-036bis Digital Twin.

## §3 State of the art (draft; published facts only)

| Class | What is published | Relevance |
|---|---|---|
| Earlier integrated GNSS–eLoran simulators | 2009 Integrated GPS-eLoran Simulator (Alion / Univ. Rhode Island / USCG Academy), cited by ESA; an experimental eLoran signal simulator by CTU Prague with GLA (2011), validated on a commercial receiver under Gaussian noise; Chinese real-time simulators (IET RSN 2023, a 2024 high-dynamic paper, an NTSC patent on UTC(NTSC) synchronisation) | Basis of ESA's critique (§2). None is available as a European product |
| Commercial GNSS simulators | Keysight (incl. Spirent PNT), Safran Skydel, R&S SMW/SMBV, CAST, Racelogic LabSat: GNSS, LEO PNT, INS and I/Q replay. No published eLoran signal generation found (report 01, Nov 2026) | These are the simulators the new eLoran simulator must couple to. The design treats them as partners, not rivals |
| eLoran receivers and antennas | NAVISP EL1-065 (H-field handheld antenna) and EL1-080 (miniaturised GNSS/LowRF receiver) by a UK company; Dutch and UK eLoran timing receivers | Devices under test for the simulator |
| Coverage tools | LCAST (Stanford/FAA), Korean accuracy simulators using land-cover conductivity maps | Coverage prediction, not signal simulation |

Sources: vcprep research report 01 §2 and its source list [S2][S5][S6][S9]–[S19]. Re-check each fact
before submission.

## §4 Proposed technical approach

### 4.1 Architecture (draft)

```
scenario (TOML, hashed) ─▶ Scene ─┬─▶ sample sinks: SigMF file │ LF hardware chain │ memory
                                  └─▶ truth (per station per group)
Scene = chains (signal definition, stations, faults, data channel)
      + timebase (TAI truth timeline, UTC(k) models, GNSS system times)
      + propagation (groundwave PF/SF/ASF + skywave behind one trait)
      + receiver trajectory + antenna channels (E-field, crossed H-loops)
      + interference and noise
GNSS side: commercial GNSS simulator ◀─ 1PPS/10 MHz + start-epoch handshake + command profile
Analysis: software receiver ─▶ observations (open format) ─▶ fidelity comparator / campaign scorer
```

- **Exact timing.** Time is an integer picosecond count. Each pulse is evaluated analytically at every
  sample instant, so delay resolution does not depend on the sample rate. MODELLED: a 10 ns reference
  offset moves the output by 10.00 ± 0.05 ns against the simulator's own truth. At the hardware connector
  this is not yet measured (WP4).
- **Deterministic.** The same scenario and seed give identical samples, independent of block size. Every
  output carries the scenario's SHA-256, so a test can be re-run and checked bit for bit.
- **Hardware abstraction.** One device interface for DACs/AWGs/SDRs (SoapySDR, UHD, Red Pitaya, Digilent,
  SCPI/LXI generators), plus a virtual bench for continuous integration. The drivers are built but
  **untested on hardware**; WP4 selects one reference chain and characterises it.
- **Conducted output only** (cables, antenna simulators). No over-the-air transmission.

### 4.2 Signal definitions and data channel (outline)
- Review the present and future definitions (LORAN-C spec, Eurofix, 9th pulse, SAE AS9990-family *[if
  available, Q6]*, the 10th pulse).
- Configuration schema; a conformance test per definition.
- Close the known gaps: an automated phase-code table check against the specification; confirm the
  Eurofix CRC-14 and RS parameters against the source texts.
- Status today: CRC-24Q, CRC-16 and HMAC-SHA256 VALIDATED (published test vectors); on-air formats MODELLED.

### 4.3 Propagation (draft)

The groundwave model computes the primary factor (PF), the secondary factor (SF) and the additional
secondary factor (ASF):
- **Smooth earth:** Sommerfeld–Norton–Wait at short range, Wait/Bremmer residue series beyond.
- **Mixed paths:** the Millington method (with Pressey phase recovery) over conductivity maps given as
  polygons, rasters or ITU-R P.832 atlases.
- **Terrain:** an integral-equation option over DEMs.
- **Urban:** urban corrections.

Evidence:
- **VALIDATED:** short-range field strength against the ITU-R P.368 reference (within 0.5 dB of
  300 mV/m at 1 km for 1 kW). Seawater SF against the Loran-C User Handbook (Harris) polynomial, within
  20 ns from 50 to 2,000 km, with one stated calibration of the effective earth-radius factor (k = 1.24).
  The special functions behind the residue series against DLMF/A&S tables.
- **MODELLED:** land-path ASF, the long-range field, propagation ECD, terrain and urban effects.

The skywave model covers one- and two-hop geometry with group and phase heights, P.684-style field
strength with a day/night transition, and Rayleigh/Rice fading. Groundwave and skywave are superposed
pulse by pulse. All of it is MODELLED; the reflection-coefficient fit is not yet read from the P.684
curves.

Noise and interference cover ITU-R P.372 atmospheric noise with Middleton Class A impulsiveness,
man-made impulsive noise (including railway traction), CW, pulsed and band-limited interferers, and
exact cross-rate interference by superposing all chains. The presets are representative, not measured.

The contract work in this area (WP3, WP6):
1. Compare land-path ASF and night skywave with the live recordings.
2. Fit or replace the parameters that fail.
3. Promote a model to VALIDATED only where an independent measurement backs it.

### 4.4 Multi-chain and multi-time-reference (draft)

Any number of chains run at once, each with its own GRI (40,000–99,990 µs in 10 µs steps, checked on
build) on the Loran epoch. Cross-rate interference comes from true superposition, not from a
statistical model.

Each station is steered to a UTC(k) model with offset, drift, steps and seeded clock noise, and can
have its own transmitter clock. GNSS system times are explicit: GPST = TAI − 19 s and BDT = TAI − 33 s
are checked by tests, and GST and GLONASST are defined the same way. UTC(k) offsets can be fitted to
BIPM Circular T values.

So one scenario can run, for example, a chain on UTC(NPL) and a chain on UTC(OP) next to a GNSS
simulator on GPST. The truth table gives each station's emission error against its own reference. The
test is whether a timing receiver can tell them apart. MODELLED (internal tests: two chains on two
references give per-reference time solutions within ± 5 ns of truth under cross-rate interference).

### 4.5 GNSS coupling and the hardware chain (outline; partner to co-write)
- Reference chain: *[partner AWG/DAC]* → LF amplifier → E-field/H-field antenna simulator → device under test.
- Clocking from a common 10 MHz/1PPS. Start-epoch handshake with the GNSS simulator.
- Per-vendor command profiles confirmed against the manuals of *[two]* GNSS simulators in *[partner lab]*.
- Coupling measured with a time-interval counter; the report gives offset ± U (k = 2), TDEV per decade,
  drift and a guard-banded decision.
- Bench readiness: the bench is checked against datasheets before purchase. The catalogue flags RF SDRs
  that cannot output 100 kHz (lowest tuning frequency in the tens of MHz; AC-coupled outputs).

### 4.6 Receiver analysis and validation tools (draft)

The tools judge receivers and the simulator against exact truth:
- **Software reference receiver:** acquisition, tracking, cycle identification, ECD and SNR estimation,
  position, time and TDoA solutions, and an LDC decoder hook. It runs on simulated or recorded samples.
  MODELLED, against simulator truth:
  - high SNR: ToA RMS < 5 ns and 0 cycle errors;
  - 20 dB SNR: ToA RMS < 30 ns;
  - two-chain cross-rate interference: ToA RMS < 10 ns and position < 10 m.

  It has not yet been compared with a reference receiver or live data.
- **Common observation format:** receiver logs (CSV profiles, NMEA, vendor formats) are imported into
  one documented, versioned format. It carries ToA, ECD, SNR, field strength, phase code, cycle flags,
  decided data-channel symbols and the time solution, plus the truth when the stimulus was simulated.
- **Pre-registered campaigns:** each test case has its pass bars fixed and hashed before it runs. The
  results are written to a hash-chained manifest that anyone can verify offline. A requirements
  verification matrix (RVM) is produced per campaign.
- **Reference benchmark:** eight reference scenarios: static, ship, drone, night skywave, cross-rate
  interference, impulsive noise, data channel, and two UTC(k). Each has registered bars, and a receiver's
  observations get a signed scorecard.

These tools give ESA and later EL1 activities (EL1-133, EL1-145, EL1-151 to 153) one consistent way to
compare receivers.

### 4.7 Record and replay (outline)
- SigMF capture and replay with segments, decimation and looped replay.
- Coherent LF + L-band capture sets on one timeline, using PPS markers of known shape injected into
  both streams and a checker that fails a mis-stamped start, wrong cable delay or wrong sample rate.
  MODELLED on simulated streams (default tolerance 5 ns); the hardware version needs the injection delay
  calibrated (WP5).
- Datasets for EL3-036bis in the agreed format *[Q7]*.

### 4.8 Fidelity validation against live signals (draft)

There is no European live eLoran dataset with ground truth that we know of, so the campaign is part of
the work, not an afterthought.

1. **Pre-register.** Before any recording, fix the fidelity metrics and bars:
   - signal level: pulse shape, ECD and SNR;
   - observable level: ToA residual against predicted PF+SF+ASF, and skywave delay and ratio at night;
   - solution level: position and time error against a surveyed reference and UTC traceability.

   Agree them with ESA at the design review.
2. **Record** at *[site(s) in the coverage of Anthorn or another transmitter, Q5]* with *[partner's]*
   reference receiver and an LF recording station, disciplined to a GNSS-traceable reference, over at
   least one full day/night cycle *[and seasons if the schedule allows]*.
3. **Simulate** the same geometry and time with the simulator. Do not tune it to the recording.
4. **Score** with the fidelity comparator, pass/fail per metric.
5. **Calibrate and re-score.** Then fit the conductivity and skywave parameters by inverse modelling on
   part of the data and re-score on the withheld part. Report both scores.
6. **Promote** a model to VALIDATED only where it passes on withheld data.

The output is a fidelity report that states what the simulator reproduces, to what tolerance and in
which conditions. It also says plainly what it does not reproduce.

## §5 Verification and validation philosophy (draft)

- Two labels only: **VALIDATED** (an independent published reference or measurement reproduced by an
  automated test within a stated tolerance) and **MODELLED** (internal tests only). Code that drives
  hardware and has only been compiled is labelled **UNTESTED ON HARDWARE**.
- A model is never VALIDATED by a test against data it was tuned to, except where the calibration is
  stated.
- Bars are written before results. A miss is reported as a miss.
- Every run is reproducible from the hashed scenario and seed. Campaign records are hash-chained and can
  be signed.
- CI runs format, strict lint and the full test suite on every change, with a nightly virtual-bench,
  acceptance and campaign run.

## §8 Background and experience (outline; 15%)
- **Ashforde** (Ashforde UG (haftungsbeschränkt), Germany, with Ashforde OÜ, Estonia):
  - a software company building validated engineering tools for PNT and space missions;
  - pre-revenue, no customers;
  - the open-core PNT engine Kshana (AGPL-3.0, dual-licensed) and the proprietary Kshana Pro eLoran
    engine described here.
- **Key person: Chakshu Baweja.** Founder; project manager and software/systems lead.
  - 8+ years of professional engineering.
  - Space Systems and DevOps Engineer at ESA/ESOC from 2021.
  - Co-author of the SpaceOps 2023 paper on the data systems of the ESA Space Safety programme.
  - Six 2026 arXiv papers, including "Conditional Timing Protection Level".
- **[Partner]:** *[heritage, NAVISP/ESA activities, lab, key persons: RF/LF lead, validation lead]*.
- **Facilities:** Ashforde provides the software, compute and CI. *[Partner]* provides the GNSS simulators,
  reference clocks, counters, LF chain and receivers.
