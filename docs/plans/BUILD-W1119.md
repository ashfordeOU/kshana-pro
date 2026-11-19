# Build programme W1119 (CONFIDENTIAL, Kshana Pro only)

Founder, 18 Nov 2026: "Once you have the gaps and info from research, start building and testing it all. All of it."
Research: VCprep `records/ops/kshana/research-2026-11-19/` (01 eLoran market, 02 maritime/aviation/timing,
03 lunar/deep space, 04 patent candidates). Integration branch: `build/w1119`; work branches `ws/w1119-*`.

Rules:
- Everything here is **Kshana Pro (private)**. Nothing goes into the open AGPL engine: AGPL §11 licenses
  contributors' patents to everyone, and public disclosure destroys novelty (04 §3). Patent-candidate IDs (E1,
  F1, ...) are noted; counsel decides filings.
- Every item: code + tests (+ CLI where useful) + evidence label (MODELLED / VALIDATED) + docs line. Commit as
  ashfordeOU, no AI attribution. Each item is committed when green, so work resumes from the last commit.
- Public wording compares on facts; "dominate" is internal only.

Status: [ ] todo, [~] in progress, [x] done (commit sha).

## A. eLoran programme (NAVISP EL1-133/145/151/152/153, UK eLoran IOC 2028)
- [ ] A1 eLoran Observation & Scenario Format (ELOF v0.1): spec, reader/writer/validator, converters (gap 01-20)
- [ ] A2 eLoran-Bench: reference scenarios + exact truth + pre-registered metrics + scorer (01-21, move 2)
- [ ] A3 ASF-aware timing protection level with Monte Carlo coverage test (01-1/2, E2)
- [x] A4 Cross-system spoofing attribution (E1) and meacon detection from propagation signature (E4) (01-3)
- [x] A5 LDC delay-bounded authentication (E7) (01-27)
- [x] A6 Spoof-safe online ASF learning gated by the protection level (E3) (01-12)
- [ ] A7 Self-calibrating simulator loop with signed lineage (F1) (01-26)
- [ ] A8 Integrity-driven ASF survey planner (F2) (02-31)
- [x] A9 Sync-Verify: LF<->GNSS alignment certificate, any simulator vendor (D3) (01-29, move 4)
- [x] A10 eLoran-disciplined oscillator bench + GNSS->eLoran switchover transients (01-17/25)
- [x] A11 Transmitter acceptance automation: ECD, zero-crossing jitter, phase code, PCI (01-22)
- [x] A12 Impulsive atmospheric noise validated against receiver behaviour (01-10)
- [~] A13 R-Mode (MF R-Mode done: synthesis, ranging, fix; VDES R-Mode and G1187 conformance pending): MF (≈300 kHz) and VDES R-Mode signals + propagation + IALA G1187 conformance (01-11, 02-3)
- [x] A14 Unified LF+GNSS interference event schema and report (01-23)
- [ ] A15 Coherent LF + L-band record and replay on one timeline (01-9)
- [ ] A16 Tactical/deployable transmitter and signals-of-opportunity scenarios (01-18/19)

## B. Hardware (plug in and test)
- [~] B1 Vendor command profiles (Keysight 33500B/33600A, Rigol DG1000Z AWGs; Keysight 53230A, CNT-91 counters done; Siglent pending byte order; Spirent/LabSat manuals unreachable) from programming manuals (AWGs, counters, GNSS simulators), cited
- [x] B2 First-plug conformance against spec-twin prediction intervals (C2)
- [x] B3 Hardware-anchored evidence of the emitted waveform: loopback digest + fidelity bound in the signed record (B2)

## C. Maritime, aviation, timing
- [ ] C1 PICS: PNT Interference Conformance Suite, clause-mapped (MSC.401/Circ.1575, IEC 61108, ED-259A, IEEE P1952, DHS RPCF, G.8272.1, C37.238), signed scorecards (02-1/7/8/14, P1)
- [ ] C2 PNT-DP (Circ.1575) reference implementation + test vectors (02-2)
- [ ] C3 Maritime incident evidence pack: NMEA/AIS/VDR ingest, actor-vs-victim analysis (02-5/24/34)
- [ ] C4 Timing compliance packs: MiFID RTS 25, DORA Art. 12, CER, India IST (02-15/16/22)
- [ ] C5 FDM spoofing event detector for flight data (02-9)
- [ ] C6 OSNMA receiver conformance scorecard (02-21)
- [ ] C7 Insurer equipment-resilience scorecard and voyage risk score (02-25/26)
- [ ] C8 Telecom/grid holdover-under-attack scenarios (G.8272.1 PRTC/ePRTC, C37.238) (02-17/18/19)

## D. Lunar and deep space (Pro, on top of the open lunar modules)
- [x] D1 Lunar time with GUM uncertainty, TCL/TL switchable, W0 sensitivity (03-1/8)
- [x] D2 "Circular T for the Moon": TL(k)−UTC processing chain with uncertainty (03-2)
- [x] D3 Earth–Moon TWTT holdover calculator (03-4)
- [x] D4 TL–UTC broadcast offset encoding + conformance vectors (03-5)
- [x] D5 LUNEX prototype reader/writer/validator + LuGRE converter (03-30)
- [ ] D6 LuGRE C/N0 reproduction harness, pre-registered (03-14)
- [ ] D7 LNSP SLA verification: SISE/availability/continuity signed monthly reports (03-16/25)
- [ ] D8 Lunar mission-review evidence pack (03-24)
- [ ] D9 Mars coordinate time + MTC/MSD (03-9)

## Resume
If a session stops: read this file on `build/w1119`, `git log`, pick the first [ ] or [~] item, continue.
