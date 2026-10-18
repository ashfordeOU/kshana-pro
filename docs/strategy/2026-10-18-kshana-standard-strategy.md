# Kshana: becoming the open standard for PNT resilience research, simulation and HIL

Strategy memo, 2026-10-18. Baseline: Kshana v0.37.0 (`/workspace/kshana`), the Pro overlay (`/workspace/kshana-pro`) and the 0.36 scope audit. External facts are cited inline. Everything proposed follows the house rules: VALIDATED only with an external oracle, no invented partners or data, non-transmitting by default, advisory and not certified.

## Thesis

The paid PNT test market sells RF hardware: signal generators such as Spirent GSS7000 (up to 256 channels, modular options, no public price; [datasheet](https://www.gnss.ca/spirent/positioning_navigation_timing/GSS7000.html)), Safran Skydel (GPU plus SDR, [datasheet](https://safran-navigation-timing.com/wp-content/uploads/2024/07/SAFRAN_Skydel_Datasheet_7-17-2024_US.pdf)) and record/replay boxes such as LabSat (from £5,950 in an earlier generation; [Inside GNSS](https://insidegnss.com/racelogic-launches-gps-data-logger-replay-system/)). What nobody sells is a neutral, open, reproducible way to say whether a receiver, a detector or a timing chain passed. That means the event truth, the scoring, the evidence and the mapping to frameworks.

Open receivers (GNSS-SDR, GPL-3, v0.0.21 in April 2026; [manpage](https://manpages.opensuse.org/Tumbleweed/gnss-sdr/gnss-sdr.1)) and open signal generators (gps-sdr-sim) already exist. The open test-procedure layer does not. Meanwhile the procedures themselves are being written in public: the Jammertest test catalogue is MIT-licensed JSON ([NPRA/jammertest-plan](https://github.com/NPRA/jammertest-plan)); IEEE P1952 is turning the DHS Resilient PNT Conformance Framework into a user-equipment standard, with a conformity-assessment steering committee ([IEEE ICAP](https://standards.ieee.org/products-programs/icap/programs/pnt-user-equipment), [sagroups P1952](https://sagroups.ieee.org/p1952)); and EASA/EUROCONTROL published an interference action plan in March 2026 ([EASA](https://www.easa.europa.eu/en/node/143866)).

Kshana should not become a cheaper Skydel. It should become the place where those procedures turn into runnable, versioned, citable test suites, and where any receiver log, IQ recording or detector output gets scored against stated truth. It then sits beside every simulator and receiver instead of competing with them. Its moat is what it already has: the honesty ledger, bit-for-bit reproducibility, signed evidence packs, and reach into every surface people already use.

## Where Kshana already leads (do not rebuild)

- **An honesty ledger with no peer.** 364 machine-checked rows: 175 VALIDATED against an external oracle, 185 MODELLED, 4 PARTNER (`docs/VERIFICATION-MATRIX.md`). No commercial simulator or open tool publishes requirement → test → oracle → status this way.
- **Timing first.** Allan/MDEV/TDEV/MTIE are matched to Stable32/NIST SP 1065. Kshana ingests real clock logs (TICC, TimeLab, RINEX CLK), runs ITU-T telecom masks and holdover Kalman filters, and carries a Conditional Timing Protection Level calibrated on a recorded spoof.
- **The receiver-trust chain.** Real UBX, RINEX 3, Android and NMEA logs give an epoch-by-epoch trust score with reasons. A live process reads stdin/TCP/UDP with a gate. Evidence packs are signed (RFC 3161). Trust telemetry goes to Prometheus/SIEM/OTel, with Signal K and OpenCPN plugins (`docs/RECEIVER-TRUST.md`, `docs/MARITIME-TRUST.md`).
- **An open software receiver and IQ layer** (`kshana iq`): scenes, channel, acquisition and tracking, front-end DSP, monitors, labfit, campaigns. It reads and writes SigMF and reads ION SDR metadata (`src/iq/io`). It is VALIDATED against gps-sdr-sim (bit-exact chips) and against GNSS-SDR on real LuGRE lunar IQ. Real-data adapters cover JammerTest 2024, LuGRE, GnssLogger, UBX, Yunnan and SatGrid (`src/realdata/`).
- **Integrity algorithms:** ARAIM ADD v4.2, RAIM FDE and DO-229E SBAS protection levels.
- **Galileo OSNMA verification,** open and receiver-side.
- **Lab bridges:** `bench-export` (motion, NMEA and waypoints for lab simulators, no signal) and `compliance-report` (DHS RPCF v2.0, IMO, EASA, NIS2 Art. 21, EN 16803).
- **An interference map** built from licence-checked ADS-B (adsb.lol, ODbL) and AIS (NOAA, Kystverket).
- **Detector evaluation:** `impairment-eval` measures the optimism gap with a 13-detector panel. Its ROC/AUC metrics match scikit-learn exactly.
- **Distribution:** CLI, crate, PyPI, npm/WASM (kshana.dev Studio), MCP server, JetBrains, notebooks, Zenodo DOI.
- **Lunar and LEO:** a LunaNet interop writer, a lunar reference station with a frozen prediction, and a LEO PNT chain.

What is already planned in the 0.36 audit, so not new here: PPP/RTK on real data (WP-07), CCSDS OPM/OCM and an SPK writer (WP-08), MagNav/EMAG2 maps (WP-10), CRPA into jamming J/S, a WASM IQ surface, BOC and vector tracking (WP-11), calibrating trust thresholds on JammerTest logs, RFC 3161 chain verification, an official OSNMA vector run (WP-12), and a Starlink Doppler proxy (WP-14).

**Gaps found in the code (none present today):**
- no UBX-SEC-SIG parse;
- no Septentrio SBF or NovAtel reader;
- no RTCM3 MSM reader;
- no ION SDR metadata *writer*;
- no direct serial/USB receiver ingest;
- no PTP/chrony/linuxptp log ingest;
- no Jammertest test-catalogue import;
- no public benchmark or leaderboard.

## Top 10 moves, ranked by impact × feasibility for a tiny team

### 1. Jammertest Conformance Suite: the catalogue as executable truth

- **What:** `kshana jammertest import testcatalog.json transmissionplan-*.json` turns each catalogue test (type, power, timing) into `receiver-trust` `[[events]]` with the catalogue ID and year. `kshana jammertest score <log>` then gives per-test detection latency, missed and false alarms, and time to recover, as a signed evidence pack.
- **Why it makes Kshana the standard:** Jammertest is the largest open interference test week, with 360 participants from 120 organisations in 2025 ([GPS World](https://www.gpsworld.com/esa-fights-interference-in-arctic-tests/)). It runs from a standardised test catalogue ([FFI paper](https://www.ffi.no/publikasjoner/arkiv/jammertest-an-open-gnss-interference-test-arena-to-accelerate-the-development-of-resilient-gnss-applications)), and that catalogue is MIT-licensed JSON. Every participant leaves with logs and no common scorer. Be that scorer.
- **First deliverable:** the importer, plus a run over the public JammerTest 2024 Zenodo logs (GPL-3, already adapted) and the SimulaMet Jammertest 2025 NMEA set ([IEEE DataPort, DOI 10.21227/a7v4-xw11](https://ieee-dataport.org/documents/simulamet-jammertest-2025-nmea-derived-dataset)). Publish results under pre-registered bars.
- **Effort:** S (importer), M (scored pages).
- **Dependencies:** none for the import. Contacting NPRA/Nkom to have the scorer listed is a later step; do not claim a relationship before one exists.
- **Risks:** dataset terms (DataPort licence to check; never vendor GPL-3 data into the repo, use fetch scripts as today). Catalogue times are planned, not measured, so label them "scheduled truth". Scores describe logs, not products.

### 2. An open PNT resilience benchmark and leaderboard ("the GSDC of interference")

- **What:** a versioned, DOI'd benchmark of tasks with fixed splits and fixed metrics:
  - (a) jamming/spoofing detection on receiver logs;
  - (b) detection on IQ;
  - (c) timing holdover prediction from measured clock logs;
  - (d) out-of-distribution optimism gap.

  Sources are open sets fetched by script: JammerTest 2024/2025, OAKBAT (ORNL DOI, registration; [ORNL](https://doi.ccs.ornl.gov/dataset/d21dfe58-3af9-5ed8-9c97-693c12045aee)), TEXBAT ([UT Radionav](https://radionavlab.ae.utexas.edu/research/public-datasets-and-code/)), LuGRE, and Kshana synthetic corpora. Kshana's `impairment-eval` is the scorer, and submissions are result files.
- **Why:** the Google Smartphone Decimeter Challenge made a dataset plus a metric the yardstick for smartphone positioning. Researchers now use it "to measure their location algorithm improvements in a standard way" ([GPS World](https://www.gpsworld.com/smart-ways-to-improve-smartphone-location-accuracy/), [ION](https://www.ion.org/gnss/googlecompetition.cfm)). ML interference papers still use self-made, non-comparable datasets: ACSNet is simulated ([arXiv 2504.10806](https://arxiv.org/pdf/2504.10806)) and RealRFI is closed ([review arXiv 2501.02352](https://arxiv.org/pdf/2501.02352)). Fraunhofer benchmarked 128 models but on its own data ([arXiv 2507.14167](https://arxiv.org/abs/2507.14167v1)). Nobody owns the common split.
- **First deliverable:** `kshana-bench v1` covering tasks (a) and (d) only. Spec, metric code and baseline scores go out under **CC-BY-4.0 for the spec and CC0 for the split manifests**, so vendors can implement against it without AGPL friction. Static leaderboard page on kshana.dev.
- **Effort:** M.
- **Dependencies:** dataset licences (per-set review, like `docs/data/INTERFERENCE-DATA-SOURCES.md`). A competition track at ION GNSS+ or ENC needs an organiser partner; pitch it, don't announce it.
- **Risks:** TEXBAT/OAKBAT redistribution limits, so manifests reference the data and never copy it. Leaderboards invite overfitting, so keep a sealed holdout set.

### 3. Timing HIL harness: "pull the antenna, score the holdover"

- **What:** passive ingest of live timing instruments, then scoring of a holdover test against ITU-T masks and the CTPL:
  - linuxptp `ptp4l`/`phc2sys`/`ts2phc` logs, `pmc` dumps, chrony `tracking`/`statistics` logs and kernel PPS offsets;
  - White Rabbit (IEEE 1588-2019 High Accuracy profile; [OHWR](https://ohwr.org/project/wr-std/wikis/wrin1588)) status exports;
  - the existing TICC logs.

  A scripted procedure: baseline, GNSS removed (by the operator, e.g. antenna cable or a simulator), holdover, recovery.
- **Why:** timing is Kshana's best-validated domain. NIST IR 8323r1 asks operators to "detect disturbances and manipulation" in PNT-dependent systems ([NIST](https://csrc.nist.gov/News/2023/nist-ir-8323-revision-1-pnt-is-now-available)). Telecom and power operators do these unplug tests by hand today. An open, reproducible holdover-test procedure with signed evidence is a natural reference.
- **First deliverable:** `clock_log` formats `linuxptp` and `chrony`, plus a `holdover-test` scenario kind and a write-up of the procedure (`docs/TIMING-HIL.md`).
- **Effort:** S–M.
- **Dependencies:** none for parsers. A timing-lab partner is needed to promote per-clock-class floors to VALIDATED (audit RM-05).
- **Risks:** low. Purely passive. No claims of conformance to G.8272/G.8273; use "measured against the mask".

### 4. Receiver protocol coverage: read-only, multi-vendor, "Wireshark for GNSS trust"

- **What:**
  - add UBX-SEC-SIG (jamming and spoofing states; [field layout](https://docs.ros.org/en/jazzy/p/ublox_ubx_msgs/msg/UBXSecSig.html)) and UBX-NAV-STATUS `spoofDetState` to the UBX reader;
  - add a Septentrio SBF reader (PVT, MeasEpoch, RF status), evaluating the Rust `sbf-tools` crate first ([docs.rs](https://docs.rs/crate/sbf-tools/0.4.0)), licence permitting;
  - add NovAtel ASCII/binary basics and RTCM3 MSM4/7 observations (read-only);
  - add direct serial/USB input to `receiver-trust live` (read-only, with no configuration writes unless `--configure` is given explicitly).
- **Why:** standards follow ubiquity. Wireshark won by decoding everything neutrally. Today receiver-trust covers u-blox, Android and NMEA; survey-grade and timing users run Septentrio, NovAtel and Trimble. Comparing a receiver's *own* spoof flag with Kshana's independent monitors is a unique, citable output.
- **First deliverable:** UBX-SEC-SIG plus a monitor named `receiver-self-report`, then SBF.
- **Effort:** S (UBX), M (SBF/RTCM).
- **Dependencies:** public interface docs only. Vendor sample logs need permission (audit DS-LAB-06 is already flagged confidential).
- **Risks:** proprietary protocol documents may restrict reuse, so implement from public reference guides. Never claim to replicate a vendor's detector.

### 5. Open reference implementations of public test procedures, with crosswalks

- **What:** for each public procedure, a scenario pack plus scoring plus a compliance-report row.
  - IMO MSC.1/Circ.1575 (shipborne PNT data processing, integrity and multi-system RAIM; [IMO](https://wwwcdn.imo.org/localresources/en/OurWork/Safety/Documents/IMO%20Documents%20related%20to/MSC.1-Circ.1575.pdf)).
  - The ICAO GNSS Manual Doc 9849 Appendix F RFI mitigation cycle (monitor, assess, mitigate; [ICAO APAC WP](https://www.icao.int/sites/default/files/APAC/Meetings/2026/2026%20SRWG10/3-Working%20Papers/WP13_ICAO-AI.6.2-Updates-on-GNSS-RFI-matters-and-its-future-management.pdf)).
  - DHS RPCF levels 1–4 as test profiles (PDF v1.0 [DHS](https://dhs.gov/sites/default/files/publications/2020_12_resilient_pnt_conformance_framework.pdf)).
  - NIST IR 8323 functions.
  - The OSNMA Receiver Guidelines v1.3 Annex B vectors ([EUSPA](https://www.euspa.europa.eu/sites/default/files/2024-03/Galileo_OSNMA_Receiver_Guidelines_v1.3.pdf)), once the GSC registration is done (audit FUT-25).
- **Why:** an open reference that bodies can point to is how SOFA/ERFA and the AIAA SGP4 vectors became canonical. P1952 is drafting now. A working, open test harness offered to the working group is the most leverage per hour.
- **First deliverable:** `docs/compliance/dhs-rpcf-test-profiles.md` plus four scenario packs (one per RPCF level) that exercise the recovery, holdover and verification behaviours, scored by receiver-trust.
- **Effort:** M.
- **Dependencies:** P1952 participation (IEEE-SA individual membership, a founder action).
- **Risks:** paraphrase only (as `docs/compliance` already does). Never say "conformant" or "certified": keep "supports evidence for".

### 6. Receive-only SDR capture and metadata (the safe half of "drive hardware")

- **What:**
  - `kshana iq capture` wraps receive-only capture from UHD (USRP), bladeRF, HackRF, LimeSDR, PlutoSDR and RTL-SDR (by shelling out to vendor tools or SoapySDR) into SigMF *plus* ION GNSS SDR metadata (ION Council-approved 2020; [sdr.ion.org](https://sdr.ion.org/));
  - add an ION metadata writer;
  - feed captures into `iq monitor` live and in near-real time.
- **Why:** a receiver is legal almost everywhere, while transmitting in GNSS bands is not. In the US, wilful interference is barred under 47 U.S.C. §333, and the FCC enforces against jammers ([FCC](https://www.fcc.gov/general/jammer-enforcement)); gps-sdr-sim's own community warns that transmission is "prohibited by law" in most countries. Capture turns Kshana into a field interference monitor and a dataset generator for move 2.
- **First deliverable:** the ION `.sdrx` writer, plus a capture recipe doc, plus `iq monitor --follow` on a growing file.
- **Effort:** S (writer), M (capture wrapper).
- **Dependencies:** none. Hardware is the user's.
- **Risks:** privacy and legality of recording in some jurisdictions (document it). The code must contain no TX path.

### 7. Neutral simulator round-trip: export → lab → log → score → evidence

- **What:** generalise `bench-export` into a vendor-neutral, documented **test-conditions spec**. `kshana.test-conditions/1` already exists in the IQ campaign; publish it as CC-BY. Add importers for the *logs* simulators emit (truth trajectories, event logs) when samples are shared. Close the loop: one command produces an evidence pack linking scenario hash, lab conditions hash, receiver log hash and score.
- **Why:** simulator vendors own signal fidelity, and receiver makers need neutral acceptance evidence. An open procedure that works on Spirent, Skydel, R&S, Keysight, IFEN or LabSat makes Kshana the referee, not the rival. Skydel's software-defined, bring-your-own-SDR model ([Safran](https://safran-navigation-timing.com/building-your-own-gnss-simulator/)) shows labs already mix vendors.
- **First deliverable:** the spec document, plus a worked example using gps-sdr-sim as the open "lab simulator" (file only, cabled-replay instructions marked operator responsibility).
- **Effort:** M.
- **Dependencies:** a real simulator lab to validate. Partner needed, none claimed.
- **Risks:** never write vendor scenario formats from reverse engineering. Keep the event "labelled interval, not interference recipe" rule (`docs/TEST-BENCH.md`).

### 8. Open interference observatory with a published methodology

- **What:** turn `interference-map` into a reproducible daily product: method paper, versioned code, daily GeoJSON and PNG under ODbL/NLOD-compatible terms, and a standard incident-report JSON. Use the NACp-vs-almanac-HDOP method ([Aeronautical Journal 2025](https://www.cambridge.org/core/journals/aeronautical-journal/article/gnss-interference-detection-methodology-utilising-adsb-nacp-indicator-and-gps-almanac-data/72FEBF7BFE1541D7BF146B14BFAA4AB8)) as an alternative scorer, with the ICAO "report → confirm → NOTAM" cycle in mind.
- **Why:** gpsjam.org is popular but personal and opaque about method ([coverage](https://newspaceeconomy.ca/2026/03/20/gps-jamming-and-the-war-over-navigation-what-gpsjam-org-reveals-about-the-middle-east-conflict/)). Thirteen EU states asked for coordinated action ([NAVISP](https://navisp.esa.int/news/article/13%20EU%20Member%20States%20call%20for%20common%20actions%20in%20response%20to%20GNSS%20jamming%20and%20spoofing%20threats)). An auditable, reproducible map with "not observed ≠ clean" semantics is citable where a hobby map is not.
- **First deliverable:** a method note plus a 30-day back-catalogue for one region (Baltic) from adsb.lol, with a sensitivity study.
- **Effort:** M.
- **Dependencies:** compute and storage (cheap). An AIS licence per region.
- **Risks:** high political sensitivity. Attribution of causes is forbidden. OpenSky's licence bars commercial use, already excluded. Publish with a 24 h or greater delay and coarse cells.

### 9. Governance and citability that outlast the founder

- **What:**
  - a JOSS paper (audit DS-PA-11);
  - a Technical Steering Committee charter with external seats, modelled on Orekit's meritocratic PMC (Airbus, NRL, ESA and universities sit on it; [Orekit](https://orekit.org/));
  - an **oracle registry** where third parties submit oracle runs that can promote rows, reviewed in public;
  - spec and test-vector artefacts under permissive licences, with the engine staying AGPL plus commercial;
  - a CITATION policy for benchmark results.
- **Why:** licences shape adoption. Orekit (Apache-2.0) became CNES's and Airbus's flight-dynamics base. RTKLIB (BSD-2) is everywhere, including its demo5 fork ([ecosyste.ms](https://opencollective.ecosyste.ms/projects/3294)). AGPL engines can be standards when the *tests and formats* are free to implement. Agencies adopt projects that have governance, not ones that hinge on a single company.
- **First deliverable:** `GOVERNANCE.md` draft, `LICENSES/` for spec/vectors (CC-BY-4.0/CC0), and the JOSS submission.
- **Effort:** S–M.
- **Dependencies:** an external academic co-author and real TSC members (people, not placeholders).
- **Risks:** none legal. The reputational risk is announcing a TSC with no members.

### 10. LunaNet AFS / LANS as the first open "compliance-check" receiver

- **What:** implement the LNIS v5 Augmented Forward Signal (AD1 Vol. A) in the IQ layer: codes, message format, SISE checks. Add a `lunanet-afs-check` that verifies a recorded or generated AFS stream against the published spec, and extend the frozen lunar-station predictions.
- **Why:** LNIS v5 was baselined and released by NASA for "open, international use" on 2025-02-14 ([IOAG](https://ioag.org/announcements/entry/238/)), with TBDs to close in v6. Few open implementations exist, so the first validated open one gets cited by every lunar receiver team. Kshana already writes LunaNet interop and flew a LuGRE validation.
- **First deliverable:** AFS code generation, VALIDATED against the spec's tables if they include test values, otherwise MODELLED.
- **Effort:** M.
- **Dependencies:** public spec only. ESA Moonlight elements remain partner-gated (audit LRS-06).
- **Risks:** spec churn to v6, so pin versions. Smaller market than terrestrial.

**Deliberately unranked:**
- **RTCM SC-134 integrity messages:** the standard was "expected late 2025" with no confirmation found ([GPS World](https://editions.mydigitalpublication.com/article/GNSS+High-Integrity+for+High+Accuracy+Applications/5023046/850790/article.html)), and RTCM standards are paid. Watch it, and prepare protection-level hooks.
- **Xona Pulsar:** no public ICD was found; only an [arXiv overview](https://arxiv.org/pdf/2509.19551) exists. Use the generic LEO chain, not "Pulsar".
- **Starlink signals of opportunity:** already WP-14. Reported results of 3.6 m 2D ([Satellite Navigation 2025](https://satellite-navigation.springeropen.com/articles/10.1186/s43020-025-00163-y)) give a published oracle.

## Hardware-in-the-loop roadmap (staged, safety-first)

| Stage | Kshana's role | Hardware | RF emission | Gate |
|---|---|---|---|---|
| **0 (today)** | files: replay logs, IQ, bench-export, live NMEA/UBX over stdin/TCP/UDP | none | none | — |
| **1 (0.38–0.39)** passive receive | serial/USB receivers (UBX, SBF, NMEA, RTCM3) read-only; timing logs (PTP, chrony, PPS, TICC, White Rabbit); IMU CSV; receive-only SDR capture to SigMF+ION | user's receivers, GNSSDO, PTP grandmaster, SDR in RX | **none** | config writes need explicit `--configure`; default read-only |
| **2 (0.40)** lab orchestration, no RF from Kshana | test-conditions spec; operator steps (antenna pull, simulator start) prompted with timestamps; round-trip scoring and evidence | lab-owned simulators, shielded boxes | by the **lab's** simulator only, conducted | a signed `lab-attestation` field (operator, setup: conducted/shielded, attenuation) recorded in the evidence pack; Kshana refuses to score a lab run without it |
| **3 (Pro, with counsel)** conducted replay of *legitimate* GNSS IQ scenes | Kshana writes scene files; documented replay via the user's own tool (UHD `tx_samples_from_file`) into a cable with attenuators to a DUT in an RF enclosure | user's SDR, attenuators, enclosure | conducted only, user's responsibility | no TX code in the open engine; interference/spoof waveforms stay out of scope, as GNSS-IQ-PLAN already states; export-control review (`kshana-pro/commercial/export-control`) |
| **4 (not planned)** real-time closed-loop RF HIL | partner territory (Spirent, Safran, R&S, IFEN) | — | — | integrate via the stage 2 spec instead |

Principles:
1. Receive is the product, and transmitting is never in the open code.
2. Every hardware session yields an evidence pack with hashes of inputs, conditions and thresholds.
3. A conducted or shielded attestation is recorded, never inferred.
4. Hardware-derived numbers stay MODELLED until an external oracle agrees, such as a lab truth trajectory or a reference clock.
5. Treat CRPA, IMU and CSAC as data sources (patterns, logs, published Allan curves), not devices to drive.

## Standards and conformance roadmap

| Body / document | Status (cited) | What Kshana publishes | When |
|---|---|---|---|
| Jammertest test catalogue (NPRA, MIT) | public JSON, 2025 and 2026 plans ([repo](https://github.com/NPRA/jammertest-plan)) | importer, scoring, results on public logs | 0.38 |
| DHS RPCF v2.0 → **IEEE P1952** | in development; CA steering committee ([IEEE](https://standards.ieee.org/products-programs/icap/programs/pnt-user-equipment)) | open test profiles per resilience level, offered to the WG as an open harness | 0.39, then ongoing |
| NIST IR 8323r1 Foundational PNT Profile | published 2023 ([NIST](https://csrc.nist.gov/News/2023/nist-ir-8323-revision-1-pnt-is-now-available)) | crosswalk rows (Identify/Protect/Detect/Respond/Recover) in `compliance-report` | 0.38 |
| IMO MSC.401(95) / MSC.1/Circ.1575; IEC 61108 / 61162 | public circular; IEC paid | integrity and PNT-DP scenario pack; NMEA 0183/61162-1 sentence coverage note; no IEC claims | 0.39 |
| ICAO Doc 9849 App. F; EASA/EUROCONTROL action plan 2026 | public | RFI monitor-assess-mitigate pack; incident-report JSON aligned to the planned ICAO reporting form | 0.40 |
| RTCA DO-229E / ARAIM ADD v4.2 / EU ARAIM | already implemented | EU-ARAIM Table A-3 reproduction (planned WP-12) as a public vector set | 0.39 |
| Galileo OSNMA Receiver Guidelines v1.3 Annex B | vectors behind GSC registration ([EUSPA](https://www.euspa.europa.eu/sites/default/files/2024-03/Galileo_OSNMA_Receiver_Guidelines_v1.3.pdf)) | recorded run against official vectors (row promotion) | after registration |
| LunaNet LNIS v5 / AFS | baselined Feb 2025 ([IOAG](https://ioag.org/announcements/entry/238/)) | AFS check tool, versioned to v5 | 0.40 |
| ION GNSS SDR metadata; SigMF | ION-approved 2020 | full read/write, conformance fixtures donated upstream | 0.38 |
| ITU-T G.8272 / G.8273 masks | partly paywalled (TIES) | mask overlays from public tables only | ongoing |
| RTCM SC-104 / SC-134; 3GPP LPP integrity | SC-134 status unconfirmed; 3GPP public | hooks only; 3GPP LPP integrity parameters as a later study | watch |

The publication format for every row is the same:
- a scenario pack;
- pre-registered bars;
- a results JSON;
- a ledger row.

Spec and vector files are CC-BY or CC0 so vendors can embed them.

## Ecosystem, community and adoption playbook

**What the precedents teach:**
- **Canonical reference values:** SPICE/NAIF and SOFA, and the AIAA SGP4 vectors Kshana already passes.
- **Permissive licence plus governance:** Orekit (Apache-2.0, meritocratic PMC, CNES/Airbus adoption).
- **Free plus pragmatic plus forks:** RTKLIB (BSD-2, demo5 for u-blox users, taught in courses such as [U-Tokyo GNSS training](https://home.csis.u-tokyo.ac.jp/~dinesh/GNSS_Train_files/202501/PresentationMaterials/20_Installation_Setup_Instructions.pdf)).
- **Universal decoding plus foundation:** Wireshark.
- **A real-time receiver plus academic papers:** GNSS-SDR.
- **Dataset plus leaderboard:** GSDC.

The playbook below applies them.

1. **Benchmarks:** `kshana-bench` (move 2) and the Jammertest suite (move 1). Every result is a citable JSON with a DOI via Zenodo.
2. **Datasets:** never host third-party data. Publish manifests with SHA-256 and fetch scripts. Publish Kshana's *own* synthetic corpora and receive-only captures (with consent) under CC-BY.
3. **Interop:** donate fixtures upstream (SigMF GNSS extension examples, ION metadata samples, GNSS-SDR configuration recipes). Add a `gnss-sdr` → receiver-trust adapter so GNSS-SDR users get trust scoring free. Add a gnss_lib_py bridge (Python) and an RTKLIB `.pos` reader.
4. **Governance:** a TSC with external seats only when real people accept; the oracle registry; public RFCs for schema changes (KIF, test-conditions).
5. **Education:** a "PNT resilience lab in a browser" course of 6 notebooks on Studio (jam, spoof, holdover, integrity, OSNMA, map), offered to universities and ION tutorials. MCP makes Kshana the PNT tool AI assistants reach for, so keep the MCP tool descriptions sharp.
6. **Integrations:** Grafana dashboard (exists), Signal K/OpenCPN (exist), ROS 2 node (receive and score; `ublox_ubx_msgs` already exists in ROS), Home Assistant / NTP-appliance exporters for timing users.
7. **Presence:** submit to ION GNSS+ and ENC (benchmark paper, Jammertest scoring results). Bring a scoring booth to Jammertest as a participant, not a transmitter.
8. **Commercial alignment:** the open suite creates demand for Pro services, such as private benchmark runs, lab-attested evidence packs and tender support (`kshana-pro/docs/gtm`). Pack A's "optimism gap" ICP maps directly onto move 2.

## Quick wins for 0.38 (days each)

1. **Jammertest catalogue importer** (`testcatalog*.json` → `events.toml`): about 1–2 days.
2. **UBX-SEC-SIG and NAV-STATUS spoof flags** in the UBX reader, plus a `receiver-self-report` monitor: about 1 day.
3. **ION GNSS SDR metadata writer** for `iq scene` output (the reader exists): about 1 day.
4. **linuxptp and chrony clock-log formats** for `clock_log`: about 2 days.
5. **SimulaMet Jammertest 2025 NMEA adapter** behind a fetch script, with a pre-registered evaluation stub, after a licence check: about 1–2 days.
6. **NIST IR 8323 crosswalk** added to `compliance-report`: about 1 day.
7. **Licence split:** `LICENSES/CC-BY-4.0` for `docs/compliance`, schemas and future bench specs, as an owner decision: about 0.5 day.
8. **`iq monitor --follow`** on a growing SigMF file (receive-only live monitoring): about 2 days.
9. **`docs/HARDWARE-SAFETY.md`:** the stage table above, with the legal notes and the attestation field: about 0.5 day.
10. **JOSS paper draft and `GOVERNANCE.md` draft:** about 2 days of writing.

## What NOT to do

- **Do not ship any transmit path** (no TX to UHD/HackRF, and no jamming or spoofing waveform synthesis to files meant for SDR playback). Transmission is illegal in most places, a reputational and export-control risk, and unnecessary for the "referee" position.
- **Do not chase signal-generator fidelity** against Spirent/Skydel/R&S. They sell certified RF hardware with calibration chains, and a tiny team loses that race.
- **Do not claim conformance,** certification, "IEEE P1952 compliant", "IMO-approved" or "DO-229 certified". Keep "supports evidence for" and advisory wording.
- **Do not promote rows on lab or partner numbers** you cannot show to an external oracle. Do not name partners, NPRA, EUSPA or test events as endorsers before they agree in writing.
- **Do not redistribute TEXBAT, OAKBAT, JammerTest (GPL-3) or DataPort data** in the repo or site. Use manifests and fetch scripts only.
- **Do not build Xona- or Starlink-specific signal models** from non-public information. Do not implement paid standards (RTCM, IEC, EN, ITU TIES) from unofficial copies.
- **Do not attribute interference** to states or actors on the observatory. Publish only "degraded / not degraded / not observed".
- **Do not spread into the remaining 0.36-audit new domains** (AOCS, re-entry, scheduling) ahead of moves 1–4. Breadth is already high, and the standard-maker's edge is depth of evidence in PNT resilience.
