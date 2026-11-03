# Kshana roadmap: 0.41 to 0.43

> **Status: not started. Nothing here begins (no code, branches or sessions) until the owner gives an explicit go,
> workstream by workstream.** Owner-run activities already under way (the 2 November Product Hunt launch, J1) are the
> only exception. Precondition met: the kshana.dev site-audit fixes are live (0.40).

Private planning document (kshana-pro). **Revision 4** (3 November 2026: workstreams L, native install, and M, space RF surveillance), after two independent audits against both repositories and
GitHub. §17 lists what each audit changed. Public material stays neutral: no company names, no actors or intent.

## 0. How to read this

- **IDs:** every item has an ID. §16 schedules every ID or marks it Gate (waits on an owner/external decision) or
  Parked (deliberately not done).
- **Sizes:** S = days, M = 1–2 weeks. Nothing larger than M exists here: larger work is split into named sub-items.
- **Exists** cells cite the file that proves it. Paths starting `src/`, `docs/`, `xval/` or `web/` are in the public
  engine repo; `pro:` is kshana-pro.
- **Done** = the measurable acceptance test or the pre-registered bar.
- Rules carried from every release: new capabilities start MODELLED, with the bar fixed before reference data is
  examined; partner data is labelled PARTNER (or PARTNER-input for partner-supplied inputs); no bar is ever loosened;
  misses stay published; a commercial relationship never changes a verdict.

## 1. Position, goals, KPIs

Kshana is the neutral, open referee and planning layer for PNT resilience, not a cheaper RF simulator. The moat:

- **The honesty ledger:** every figure is labelled, and misses are published.
- **Reproducibility:** the same scenario, seed and engine version give the same bits.
- **Signed evidence.**
- **Reach across every surface.**

Strategy memo: `pro:docs/strategy/2026-10-18-kshana-standard-strategy.md`. Its move 9 (governance) is open; see I.

**Focus rule ("do not spread", from the memo):** new work goes to interference monitoring (B), the integration layer (A),
flight-dynamics exchange (C) and timing (G). The other families are **maintained** (§1c).

### 1a. Competitive map

| Category | Kshana's stance |
| --- | --- |
| RF / constellation simulators (hardware) | Complement: import their scenario and motion files (A2b), score receivers under test, model the operational consequence |
| Interference-monitoring networks | Complement: referee, planner, labelled data, consequence (B) |
| Flight-dynamics / astrodynamics tools (GMAT, Orekit, STK, Basilisk, TMPL-class) | Complement: independent cross-check, exchange formats (C) |
| Open receivers / SDR (GNSS-SDR, RTKLIB, gLAB) | Integrate and score (A2b, D) |
| Resilience frameworks and consultancies | Supply the evidence layer (G4) |
| Space RF surveillance and tip-and-cue constellations | Complement: accuracy model, revisit/reaction referee, target maintenance validated on public AIS, timing budget (M) |
| Desktop GNSS tools (receiver vendor utilities, SDR suites) | Complement: one signed app that scores any receiver live and replays bit-for-bit (L) |

### 1b. KPIs (measured at each release)

| KPI | 0.40 | Target 0.43 |
| --- | --- | --- |
| Verified integrations (A7, with written consent to list: H8) | 0 | 10 |
| Scored public datasets | Read from bench manifests at 0.41 | +5 |
| Ledger rows promoted to VALIDATED or PARTNER | 176 VALIDATED | +25 |
| Paying design partners | 0 | 3 |
| External citations / papers | Read from Zenodo and Scholar at 0.41 | +10 |

### 1c. Families: focus vs maintained

- **Focus:** interference and spoofing, receiver trust, timing/holdover, integrity, CCSDS/SPICE exchange, lunar
  navigation service.
- **Maintained (bug fixes, ledger hygiene, no new scope unless a partner asks):**
  - LEO PNT (7 kinds);
  - optical (2);
  - AOCS, EO, entry, launch;
  - Mars and solar system;
  - quantum (the cold-atom error terms stay internal: VM-8112313d);
  - alternative PNT (follow-ons listed in D, Parked).

## 2. Workstream A: plug-and-play integration layer ("Kshana Connect")

**Goal:** any company connects with configuration only. That means no special development by us or them, now or later.

**Definition of done:**

- There is one named **reference integration per customer shape** (§3), replayed from public data.
- An integration passes when:
  - `kshana connect check` is green;
  - an engineer outside the project completes it from the docs alone in ≤ 4 hours;
  - **zero code changes** are needed in either repo;
  - the same integration works through every surface (CLI, Python, WASM, MCP).
- CI repeats every reference integration at every release.

| ID | Item | Exists (cited) | Missing | Size | Done |
| --- | --- | --- | --- | --- | --- |
| A1 | **Schemas**: `kshana.obs/1` (station/system observations, alarms, IQ refs, OSNMA, site/antenna/receiver metadata, partner-input fields for E2–E5) and **scenario-schema versioning** (version field, migration tool `kshana scenario migrate`, deprecation window) | Readers share one `Timeline` (`docs/MACHINE-INPUTS.md`); result envelope with `schema_version` and a compatibility contract (`docs/SCHEMA.md`); partner submissions are TOML (`docs/PARTNER-DATA.md`) | JSON Schema for obs/1; validator `kshana connect validate`; scenario versioning and migrator; change policy | M | Every reader's output validates; old scenarios migrate; schemas published under A10 |
| A2a | **Mapping file `connect.toml`** (CSV/JSON/Parquet/HDF5 → schema; units; time base) | — | All | M | Three unrelated vendor CSVs map with no code |
| A2b | **Receiver, timing and test-bench formats (delta)** | RINEX, UBX, SBF, NovAtel, RTCM3, NMEA, SigMF, ANTEX, SP3, CCSDS, SPICE; linuxptp and chrony (`docs/CLOCK-LOG-FORMATS.md`); motion/test-bench **export** (`src/interop/testbench.rs`) | White Rabbit, ntpd; Trimble/Leica; RF-simulator scenario and motion **import**; RINEX 4.00 observations tested (`ROADMAP.md`) | M | Each reader round-trips its vector |
| A2c1–8 | **Industry feeds**, one ID per industry, owner ranks the top five for 0.41 | ADS-B/AIS reports (`src/interference_map/`) | A2c1 aviation (ASTERIX CAT021/023, ARINC 424/429, GNSS-RFI reports aligned with the aviation incident-report form, NOTAM); A2c2 maritime (NMEA 2000, IEC 61162-450); A2c3 rail (ETCS/ERTMS position reports); A2c4 automotive (CAN/DBC, ROS bag, ASAM OpenSCENARIO); A2c5 power grid (IEEE C37.118, IEC 61850-9-3); A2c6 telecom/5G (G.8275 profiles, O-RAN M-plane sync alarms, SyncE ESMC); A2c7 finance (RTS 25 clock-traceability report); A2c8 handsets and home (3GPP LPP; Home Assistant and NTP-appliance exporters) | S–M each | Each feed: public sample + passing replay |
| A3a | **Live ingest, core** | Live NMEA stdin/TCP/UDP (`src/live_io.rs`); `receiver-trust live` follows a file, serial, `--listen`; `iq monitor --follow` (growing SigMF); MQTT publish without TLS (`src/telemetry/mqtt.rs`) | MQTT subscribe with TLS; WebSocket; generic directory watch; all into obs/1 with time alignment | M | 100 stations × 1 Hz on the reference box; stored-stream replay |
| A3b | **Live ingest, scale** | — | Kafka, NATS, back-pressure, load-test harness | M | 1,000 stations × 1 Hz within K3 |
| A4 | **Outputs and alerts** | Reports (JSON/CSV/HTML); Prometheus, OTLP (feature-gated, off by default, no TLS), syslog CEF/LEEF, MQTT (`src/telemetry/`): **receiver-trust stream only**; Grafana (`deploy/grafana`); CZML/KML/GeoJSON/STK/SigMF exports | Telemetry for **all** scenario results and B-stream alerts; OTLP TLS and default-on build; signed webhooks; CAP; SNMP traps; scorecard export for insurers/regulators | M | Every sink receives a test alert end to end, over TLS |
| A5a | **Service API and clients** | OpenAPI 3.1 in Pro (`pro:src/serve/openapi.rs`) | Generated clients (Python, TypeScript, Go, Java); cross-surface versioning policy; gRPC for streams | M | Clients generated and tested in CI against the service (needs F1) |
| A5b | **C ABI** as a separate crate `kshana-capi` (the core keeps `#![forbid(unsafe_code)]`, `src/lib.rs`; restated in SECURITY.md) | — | cbindgen header, safety review, examples (C/C++/Go/Java/MATLAB/LabVIEW) | M | FFI fuzzed; examples build in CI |
| A6 | **Deployment kits** | Docker images; signed SLSA-attested multi-arch images (`.github/workflows/reference-image.yml`, `mcp-publish.yml`); air-gapped compose (`pro:deploy/docker-compose.sovereign.yml`); Signal K, OpenCPN, ROS 2, GNSS-SDR | **Helm chart (none exists)**; Compose "engine + ingest + service + Grafana"; sizing guide | M | Clean-cluster and air-gapped installs from docs only |
| A7a | **Conformance kit, files** | `kshana partner check` (partner-submission format only) | Vectors and replay corpus per adapter; `kshana connect check`; surface-parity check | M | CI runs every reference integration |
| A7b | **Conformance kit, live** | — | Latency, ordering, reconnect checks | S | As A7a for live paths |
| A8a / A8b | **Integration guides**, one per customer shape; A8a for shapes whose items ship by 0.42, A8b for test-event and port/airport shapes (need B4, B6) | Surface, MCP, deploy docs | Guides, translated per K8 | M + S | Each guide is the script of its reference integration |
| A9 | **Integration security** | Bearer token (`--api-token-file`, `pro:docs/SERVICE.md`) | OIDC, tenant isolation, audit log, retention, PII-free crowd ingest | M | Pen test (K2) passes |
| A10 | **Licensing for integrators**: schemas, vectors and clients under MIT or CC-BY-4.0; integrator licensing FAQ (file/network separation vs linking) | Licence split still a draft (`docs/drafts/LICENSES-SPLIT.md`; `bench/SPEC.md` pending) | Decision and FAQ | S | **Gate: owner + counsel** |
| A11 | **Adapter registry and community adapters**, built on the existing seam | `ExternalPack` (`src/api.rs`) and `PackRegistry` (`src/registry.rs`): Rust, compile-time only | Runtime plug-ins (mapping file; WASM or Python), registry, CONTRIBUTING rules, integration-request and adapter-submission issue templates, CLA/DCO decision | M | First external adapter merged with no engine change (**CLA/DCO: Gate**) |
| A12 | **Surface parity backlog** | `docs/SURFACES.md` | Emitter regions and OSNMA on MCP; Connect commands on Python/WASM/MCP/Studio | S | Parity check in A7a green |

**Order:** A1 → A2a, A7a, A10, A12 → A2b, A2c (top five), A4, A5a → A3a → A7b, A6, A9, A8a → A3b, A5b, A11, A8b.

## 3. Reference integrations (one per customer shape)

| Shape | Reference integration (public data) | Needs |
| --- | --- | --- |
| Interference-monitoring network (SJAWS-class) | Station logs replayed from Jammertest; referee scorecard, area trust, alerts to Grafana and webhook | A1–A4, B2, B5 |
| Flight-dynamics house (KinetX / IM, TMPL-class users) | Public trajectory as OEM/OPM/SPK loaded **in GMAT, Orekit and STK themselves**; difference report | C1, C7 |
| Port, airport, telecom, grid operator | ptp4l/chrony log + scheduled jamming; holdover consequence; CAP alert | A2b, A4, B6, G1 |
| Receiver / antenna / clock vendor | Vendor log on a public campaign; bench track; listing | A7, E6 |
| Test lab / RF-simulator house | Simulator scenario imported, receiver scored, evidence pack | A2b, E8 |
| Hardware partner | Antenna pattern or device sheet driving a study (PARTNER-input) | E2–E5 |
| Test event / programme | Pre-registered scoring of a campaign; signed pack | B2, B4 |
| Tooling / AI ecosystem | MCP + generated clients in a notebook | A5a |

## 4. Workstream B: interference-monitoring networks

Detail: `0.41-BACKLOG.md` §1.

| ID | Item | Size | Depends | Done |
| --- | --- | --- | --- | --- |
| B1a | Planner: detection and location-accuracy maps, station count and placement | M | — | Reproduces a published multi-station TDOA campaign's geometry within a pre-registered bound |
| B1b | Planner Pareto front with a **cost model** (the optimiser has none today, `pro:docs/DESIGN-OPTIMISER.md`) | S | B1a, F1 | Front includes station-cost axis; reproducible |
| B2 | Referee location metrics (error, region coverage, time to locate) | M | — | Matches hand-computed values on a synthetic truth set |
| B3a | Emitter locator, power-difference (C/N0 + AGC) | M | A1, A2a | Pre-registered bar |
| B3b | Emitter locator, TDOA from IQ snapshots | M | B3a | Pre-registered bar |
| B3-gate | **Pre-registration before the reference positions are seen**: check whether Jammertest 2026 transmitter positions are already published; if yes, pre-register against the next campaign instead | — | — | **Gate (dated at go)** |
| B4a | Benchmark: jam / spoof / meaconing classes (TEXBAT, OAKBAT, Jammertest) | M | — | Bars set in advance |
| B4b | Benchmark: natural confounders (scintillation, solar bursts, multipath, obstruction) | M | B4a | Bars set in advance |
| B5 | Area trust state from several reference stations | M | A1, A3a | Alert latency ≤ 5 s end to end on replay (K3) |
| B6 | Port and airport scenario pack | M | — | Consequences reproduce from scenario and seed |
| B7 | Synthetic labelled data packs (IQ, logs) | M | — | Same seed, same file; exact labels |
| B8 | Environmental baseline per site and time | S | — | Compared with documented event days |
| B9 | Two-way data with the maritime trust score | M | A3a, A4 | Round trip on replay |
| B10 | IQ signal breadth for receiver-test partners: E5b, E6, B2a, B3I, L1C, GLONASS CDMA (`docs/design/iq-notes/signals.md`) | M | — | Codes checked against the ICDs |

## 5. Workstream C: navigation and flight-dynamics tools

| ID | Item | Exists (cited) | Missing | Size | Done |
| --- | --- | --- | --- | --- | --- |
| C1 | **Exchange formats**: correct the overstated claim and close it | OPM/OMM/OCM (`src/odm.rs`), AEM/APM (`src/adm.rs`), SPK writer (`src/spk_writer.rs`), TDM (`src/ccsds_tdm.rs`), Orekit round trip (`docs/INTEROP.md`). **OEM is checked against the spec and an independent parser only; never loaded into GMAT or STK** (`docs/INTEROP.md:243`, VM-81fb8429) | Load-tests in GMAT and STK (and CesiumJS for CZML); CDM; SPICE CK; OCM PERT/OD blocks; **rename ledger row VM-81fb8429** to what was checked | M | Files open in GMAT 2025a and STK; row title matches evidence |
| C2 | Lunar orientation vs NASA SVS Moon phase and libration tables; the new row cites NASA SVS | DE440 lunar orientation | Comparison, bars, row | S | Pre-registered bars |
| C3 | Artemis I manoeuvre-aware second run (pre-registration draft `docs/mission-thread/PREREGISTRATION-C2-DRAFT.md`) and NRHO force model for CAPSTONE | Both NOT MET | As stated | M | New pre-registered runs |
| C4 | Real DSN tracking comparison | TRK-2-18 reader, Mars ramped Doppler | Partner data | M | **Gate: partner** |
| C5a / C5b | Optical-navigation measurement model / small-body gravity (or partner study first) | B-plane, light time | Models | M + M | Bars set in advance |
| C6 | Lander navigation vs flown data | Descent/surface/ascent (MODELLED) | Partner data | M | **Gate: partner** |
| C7 | Adapters/notebooks for GMAT, Orekit, Basilisk, TMPL-class | MCP, Python, Rust | Adapters | M | Reference integration (§3) passes |
| C8 | LunaNet AFS message layer and closed-loop decodability vs a lab or flight receiver | Codes and coding VALIDATED | Receiver comparison | M | **Gate: partner** |
| C9 | OD and frames follow-ons (`ROADMAP.md`): gravity above degree 70; OD light time, Earth rotation, sequential filter for range-rate and az/el; selenocentric transform and family continuation; carrier phase + clock tight coupling; gLAB parity | Listed in ROADMAP.md | As listed | S–M each (C9a–e) | Each against its existing oracle |

## 6. Workstream D: open evidence gaps (each stays NOT MET until a new pre-registered run meets it)

| ID | Gap | Path | Size |
| --- | --- | --- | --- |
| D1 | Artemis I, CAPSTONE | C3 | — |
| D2 | Clock-log F1(a), F2, D2 | Noise identification; measured vs self-predicted holdover | M |
| D3 | Emitter region NOT ASSESSABLE | B3 | — |
| D4 | kshana-bench A2, D1, R1; task (b) baseline re-run; task (a) event-level metrics; second OOD track for task (d) | Baselines, re-runs | M |
| D5 | Space-user service volume S1–S5, S7 | GLONASS reference angles; held-out reception campaign | M |
| D6 | Lunar infrastructure C3/Q3/G3 (orbit determination, carrier phase, geodesy; `xval/lunar-infra/RESULTS-v2.md`); post-hoc C3 sweep (`pro:docs/handover-0.40/.../W57-H2-handover.md`) | Partner/measured data; sweep | M, Gate |
| D7 | LunaNet AFS printed column | Recheck at spec's next issue | S |
| D8a / D8b | IQ layer NOT MET rows (D8a: tracking and C/N0 rows; D8b: timing and campaign rows): VM-77d4bf93, VM-56e6f4f4, VM-f5e2ce4e, VM-e6a6348d, VM-bdfec2e4, VM-35b27a4f, VM-59bb0091, VM-6254a75d, VM-c9318c63 | IQ re-campaign with pre-registered bars | M + M |
| D9 | Alternative PNT: real-EGM2008 one-hour run 3106 m vs < 500 m (VM-6a54cfeb) | Full EGM2008 / crustal magnetic map / map-error Monte Carlo / SLAM / EIGEN loader | Parked (maintained family) |
| D10 | UTC(k) prospective round 3 (VM-cce187eb), needs monthly issues to ~468 | Scheduled scoring | S, **Gate: date ~early 2027** |
| D11 | DE440 Sun/Moon bar (`xval/propagator-de440-orekit/`) | Re-run with fixed frame chain | S |
| D12 | Transfer optimiser (VM-c80a768c); LEO Doppler positioning (VM-c4520dfd) | Re-run / parked per §1c | S each |
| D13 | Campaign Monte Carlo 183/200 (VM-badb561f); campaign optimise round 1 (VM-5664a533) | Re-run | S |
| D14 | Lunar frame datum (VM-55f1a043); spectrum bands (VM-23e52907) | Re-run | S each |
| D15 | ARAIM reproduction 4 of 12 NOT MET (`docs/INTEGRITY.md`) | Reference reconciliation | M |
| D16 | Lunar reference-station prediction "not yet scored" (`docs/VALIDATION.md`) | Watch; score when data arrives | S, Gate |
| D17 | Defect D11c (fusion flicker bound) open; D12 fixed in engine but `pro:docs/defects/D12.md` still "open" | Fix D11c; close D12 doc | S |
| D18 | W57 leftovers: almanac-HDOP scorer; pin TEXBAT hash | Small fixes | S |

## 7. Workstream E: partners and partner areas

**Rules:**

- The four PARTNER rows stay "Not provided" (`docs/VERIFICATION-MATRIX.md`): bus engineering, RF payload and antenna
  hardware, quantum payload hardware, and flight-hardware product assurance.
- The interfaces below let a partner's own data drive studies, labelled **PARTNER-input**. No row changes label
  because an interface exists.

| ID | Partner area | Exists | Interface | Size | Done |
| --- | --- | --- | --- | --- | --- |
| E1 | Data partners (9 MODELLED rows; holdover floors first) | Partner kit, `kshana partner check` | Data agreement, signed upload, pre-registration form, scoring report | M | First submission scored end to end |
| E2 | RF payload and antenna | ANTEX, CRPA model | Measured-pattern CSV, CST/HFSS export import, CRPA array file, payload tables | M | A vendor pattern drives a study |
| E3 | Quantum payload | Clock twin, quantum models | Device performance sheet | S | Sheet drives clock twin |
| E4 | Spacecraft bus | Mission thread | SysML v2 / Capella export of navigation-relevant constraints | S | Constraint file drives a thread |
| E5 | Flight-hardware product assurance | Pro traceability (CSV, ReqIF, SysML v2), dossier | PA evidence links by requirement id (ECSS-Q / Polarion); **ReqIF export validated against the ReqIF schema** (`pro:docs/MBSE.md`) | S | ReqIF validates; links resolve |
| E6 | Clock and receiver vendors | Clock twin, receiver trust | Datasheet import; vendor bench track | M | Listing via A7 |
| E7 | Programme and certification | Outreach drafts; timing shortlist (`pro:commercial/partners/`) | Partner page; tiers with entry criteria; lifecycle (apply → conformance → publish → recertify each minor → revoke); badge "format conformance, not performance"; conflict-of-interest line | S | **Gate: owner sends outreach** |
| E8 | **Test labs and RF-simulator houses** | Commitments R1–R9 (`pro:main:commercial/partners/RECEIVER-TEST-PARTNER-REQUIREMENTS.md`) | Track R1–R9 here; simulator import (A2b); evidence pack per test | M | R1–R9 each met or re-scoped |
| E9 | Other partner shapes: uncrewed-aircraft navigation (export-gated), positioning-chip makers (`BASEBAND-KSHANA-BRIEF.md`), certification bodies, insurers, cloud marketplaces | Briefs | One-page integration path each (via §3) | S | **Gate: owner** |

## 8. Workstream F: Kshana Pro

| ID | Item | Status (cited) | Size | Done |
| --- | --- | --- | --- | --- |
| F1a | **Port Pro from engine `=0.32.0` to 0.40** (`pro:Cargo.toml`; PRO-POWER §7 still says a 75-kind path build) | Not done | M | Pro builds `--locked` on published 0.40; all Pro tests green |
| F1b | Release build and signed licence token end to end; release-smoke script (WIP on `pro/backlog-0.36`) | Not done | S | Signed token verified in CI |
| F2a | Service hardening: TLS in the binary, streaming, cancellation/timeout, priority, HA (no single-directory lock), backup (`pro:docs/SERVICE.md` limits) | Missing | M | Load and failover tests pass |
| F2b | Connect in Pro: tenancy, audit log, SLA metrics (after A) | Missing | M | Tenant isolation test |
| F3a | Hosted / on-prem platform: multi-tenant jobs, saved studies, sharing | Missing | M | Pilot tenant runs a study |
| F3b | **Hosted trial sandbox** (the engine hosts no endpoint today) | Missing | S | Public trial with quotas |
| F3c | **Structured study intake** that runs the pack (`POST /study` only acknowledges today, `pro:src/serve/study_intake.rs`; the site uses a mailto) | Partial | S | A request produces a queued study and an instrumented J metric |
| F4 | Pack B; export-control: questionnaire draft (`pro:legal/EXPORT-CONTROL-DETERMINATION.md`); **PR #14 factual correction** (jammer-waveform IQ export, in-memory spoofer) and branch `legal/classification-pack-iq` held for counsel | Gate | — | **Gate: counsel** |
| F5 | Pack C (subscription, CI guide) | Partial | M | First subscriber flow end to end |
| F6 | Commercial: templates exist (DPA, NDA, SLA/SOW drafts, `pro:legal/`); **the MSA does not exist** (`COUNSEL-REVIEW-PACKAGE.md` §4.7); pricing; trademark; payment go-live checklist (`pro:commercial/GO-LIVE-CHECKLIST.md`, open boxes); connector packaging and pricing | Gate | — | **Gate: owner + counsel** |
| F7 | **Signed dossier**: the mission dossier, ledger and manifest are unsigned today (`pro:docs/MISSION-DOSSIER.md`), against the "signed evidence" moat | Missing | S | Signature verifies offline |
| F8 | Pro feature limits: uncertainty with correlated inputs; spectrum coexistence beyond current bands (L2C, antenna pattern, other systems); dossier PDF with Unicode fonts (translated customers); fix the stale STK note in `pro:src/connectors/mod.rs` | Missing | S–M each (F8a–d) | Each with a test |
| F9 | Grants programme (`pro` PR #27) | Draft | — | **Gate: owner** |

## 9. Workstream G: missing domain features

| ID | Item | Exists | Missing (delta) | Size | Done |
| --- | --- | --- | --- | --- | --- |
| G1a | Telecom timing completeness | `telecom-timing` kind (`docs/TELECOM-TIMING.md`) | G.8273.2 Tables 7-6/7-7/7-10, relative time error, class D, 100-sample PTP filter | M | Against the recommendation's tables |
| G1b | Network timing: multi-hop PTP/White Rabbit, failover, holdover across a network | — | All | M | Bars set in advance |
| G2 | Time-transfer authentication (NTS, PTP auth TLV), spoofed-time detection | — | All (after G1b) | M | Detection bars |
| G3 | Outage cost / ROI | `src/frugal.rs` | Operational-impact model | S | Worked example reproduces |
| G4 | Self-assessment questionnaire on the compliance mapping; resolve the open compliance items (`docs/compliance/dhs-rpcf-test-profiles.md`, `docs/compliance/imo-circ-1575.md`) | Compliance report | Questionnaire; open tests | M | Each open test met, re-scoped or kept NOT MET |
| G5 | Fleet and route risk dashboards | `kshana route-exposure` (`docs/INTERFERENCE-MAP.md`) | Fleet view and dashboards | S | Dashboard from a public day |
| G6 | Gap-remediation planner (weakest link → ranked mitigations) | Mission-thread weakest link | Planner | M | Worked example |
| G7 | Underwater acoustic PNT | — | — | — | Parked |
| G8 | 3GPP LPP integrity study | — | Study | M | Bars set in advance |

## 10. Workstream H: quality, platform, i18n, site

| ID | Item | Size | Done |
| --- | --- | --- | --- |
| H1 | SonarCloud new-code coverage ≥ 80% (~810 lines; CLI paths via in-process tests) | M | Gate green |
| H2 | CI sharding; keep release `verify` aligned with CI (fixed in 0.40); triage the 8 open Dependabot PRs (#85–#100) | S | CI ≤ 2 h; PRs merged or closed |
| H3a | i18n 0.40 leftovers: the English strings left on translated Studio pages (area cards, evidence legend, chart captions); leftover Rust-built English words (W55) | S | No English string on a translated page in the H7 audit |
| H3b | i18n: **whole-sentence messages for gendered languages** (promised for 0.41, `pro:docs/plans/0.40-INTEGRATION-BRIEF.md`); native-speaker review programme; one-pass rendering | M | No fragment-assembled sentence; review sign-offs per language |
| H4 | CHANGELOG (RUSTSEC-2026-0387, MSRV 1.88 under 0.40.0); README MCP table cells; `docs/SURFACES.md` drift (Studio views now exist) | S | Docs tests green |
| H5 | Translated READMEs regenerated each release | S | Release job |
| H6 | 0.40.0 release leftovers: the MCP server ghcr image (cancelled at the tag workflow's 90-minute limit, run 41201309954), and the two jobs it blocked (MCP registry entry, channel parity); build arm64 on a native arm64 runner instead of QEMU so an image fits well inside the limit; republish and verify the 0.40.0 images (needs the owner to re-tag, or ship as 0.41.0 images); release dry-run before every tag | S | Both images and the registry entry published and verified; channel parity green; image job < 60 min |
| H7 | The 0.40 browser audit (every page × 18 languages × widths) as a CI job | M | Job green |
| H8 | CI check: no company name in the public repo or the integration list without recorded consent | S | Job green |
| H9 | Studio gaps: maritime-trust field, self-report strip, Jammertest Tools card and notebook, clock logs; **bring your own logs into Studio** | M | Each view ships with a test |
| H10 | Issue #36 (macOS oracle failures); W55 SBOM verdict re-run against the final lock; CI labfit follow-up tests | S | Closed |

## 11. Workstream K: adoption readiness

| ID | Item | Size | Done |
| --- | --- | --- | --- |
| K1 | 1.0 stability criterion; LTS branch with a 12-month patch window (SECURITY.md is latest-only) | S | Policy published |
| K2 | Security: IEC 62443-4-1 process readiness; ISO 27001 readiness for hosted; pen test; TLS everywhere; signed adapters; **PyPI Trusted Publishing** (attestations are off under token auth, `.github/workflows/publish.yml`) | M | Pen test passed; attestations on |
| K3a / K3b | Real-time targets: load-test harness (with A3a) / alert latency ≤ 5 s (with B5) and 1,000 stations (with A3b) | S + S | Harness in CI |
| K4 | Observability of Kshana itself (ingest, job service) | S | Dashboards exist |
| K5 | WCAG 2.2 AA audit and fixes (Studio, site) | M | Audit report, fixes merged |
| K6 | GDPR DPIA template; per-tenant data residency | S | Template published |
| K7 | Export classification for Connect, C ABI, air-gapped bundle | — | **Gate: counsel** |
| K8 | Docs versioned per release; translation plan for Connect guides | S | Versioned docs live |
| K9 | **Procurement pack**: security questionnaire answers, published SBOM and VEX, vulnerability-response SLA, LTS terms, company fact sheet, export statement per artifact | M | Pack published |

## 12. Workstream I: research, standards, governance (owner-gated)

| ID | Item |
| --- | --- |
| I1 | JOSS tools paper (author decisions, external co-author) |
| I2 | AI/ML impairment-evaluation testbed paper (IEEE/ION PLANS 2027) |
| I3 | ION GNSS+ and ENC submissions |
| I4 | Standards bodies: RTCM SC-134, EUROCAE WG-62, ITU-T Q13/15, **IEEE P1952** |
| I5 | Governance: TSC charter, oracle registry, licence split (links A10), CLA/DCO (links A11) |
| I6 | Upstream contributions: fixtures to SigMF and ION repositories; a Zenodo DOI per bench result |

## 13. Workstream J: market and launch

| ID | Item | Done |
| --- | --- | --- |
| J1 | Product Hunt (2 November, owner-run): first comment, replies; Show HN, r/GNSS, r/rust | Launch done; replies answered |
| J2 | Metrics per release: Studio sessions, installs (crates.io, PyPI, npm), stars, studies requested (F3c instrument), verified integrations, design-partner conversations | Metrics table each release |
| J3 | Listings: MCP registry (owner variable), winget moderation (the 0.40.0 pull request is open; the tag's workflow still fails on the duplicate, fixed on main), OpenCPN catalogue PR (`packaging/channels.json`) | Listed |
| J4 | Publish a **neutral public roadmap** (`ROADMAP.md` does not reflect 0.41–0.43) | Public file updated |
| J5 | Pack A outreach (first paying design partner); Jammertest scoring presence; browser course for universities | **Gate: owner** |
| J6 | Correct the outreach material: the flight-dynamics fit document claims OEM exchange with GMAT/STK as VALIDATED (see C1) | Corrected copies issued |

## 13a. Workstream L: native install (Kshana Desktop and Kshana Station)

**Goal:** Kshana installs on a Mac, Windows or Linux machine (Mac first), is signed and notarized as Ashforde OÜ, and connects to the sensors the browser cannot reach. What runs in kshana.dev stays the free entry; the installed app adds live hardware, long runs and the Pro features. Nothing here forks the engine: the app links the same Rust crate, and the window reuses the Studio front end (`web/studio/`), so there is one engine and one UI.

**What the browser cannot do today, and the installed app will** (from `docs/SURFACES.md`, "Why the N/A cells are N/A"):

| Capability | Today | Why not in the browser |
| --- | --- | --- |
| `receiver-trust live` as a running process: serial/USB NMEA (`--serial`, read only), TCP, UDP, followed file, `--gate`, `--listen` | CLI only (`src/live_io.rs`, `open_serial`) | A page cannot hold a socket or a serial port open for others (Web Serial is Chromium-only and per-tab) |
| Trust telemetry from a running stream (Prometheus scrape, syslog CEF/LEEF, MQTT, OTLP) | CLI (`src/telemetry/`) | A page cannot serve a scrape endpoint or hold a broker connection |
| GNSS IQ: `iq scene/acquire/track/monitor --follow`, `extract/convert/decimate` | CLI, Python, MCP (bounded) | Recordings are gigabytes; WASM has no IQ functions |
| Training NMEA streamed to TCP/UDP, Signal K and OpenCPN feeds, ROS 2, GNSS-SDR relay | CLI, `integrations/` | Consumers of a running process |
| Clock logs from the machine itself (linuxptp `ptp4l`/`phc2sys`, chrony) | Files only (`docs/CLOCK-LOG-FORMATS.md`) | A page cannot read the system's PTP/chrony state |
| Long campaigns, Monte Carlo, Pro optimiser and service | CLI, Pro (`pro:src/serve/`) | Single-threaded WASM, tab memory limits, licence |

**Shapes (one engine, three deliverables):**

1. **Kshana Desktop**: a windowed app (Tauri 2: Rust backend is the engine itself, the webview shows the Studio UI) with a Devices panel, live views, local files of any size, and Pro unlocked by the signed licence token (F1b).
2. **`kshana` and `kshana-mcp` binaries, signed**: the five targets already built per release (`.github/workflows/release.yml`, matrix `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, two Linux) become signed/notarized, with a Homebrew cask/formula, winget, Scoop, `.deb`/`.rpm` (MCP already ships via `brew install ashfordeOU/tap/kshana-mcp`, Scoop and winget: `packaging/channels.json`).
3. **Kshana Station**: a headless service (launchd, systemd, Windows service) on a site computer wired to receivers, SDRs and clocks. It publishes `kshana.obs/1` (A1) and alerts (A4), is managed from Desktop or remotely (A9), and is the node of a monitoring network (B1, B3, B5). The existing marine reference build (`deploy/reference-build/`) is its first, container-only form.

| ID | Item | Exists (cited) | Missing | Size | Done |
| --- | --- | --- | --- | --- | --- |
| L1 | **macOS signing and notarization of the CLI and MCP binaries** (Developer ID Application + Installer certificates, hardened runtime, `notarytool` in CI (Apple ID + app-specific password, as the owner's other macOS products use; repository secrets `APPLE_CERTIFICATE(_PASSWORD)`, `APPLE_INSTALLER_CERTIFICATE(_PASSWORD)`, `APPLE_ID`, `APPLE_APP_PASSWORD`, `APPLE_TEAM_ID`), stapled `.pkg`; universal arm64+x86_64) and a Homebrew **formula/cask** for `kshana` | Unsigned macOS binaries per release; MCP tap | Signing step, secrets, universal build, `kshana` tap entry | S | `spctl --assess` passes on a clean Mac; `brew install` works; Gatekeeper shows Ashforde OÜ |
| L2a | **Kshana Desktop for macOS, core**: Tauri 2 shell, Studio UI in the webview, engine calls native (no WASM), open/save any file size, long runs off the UI thread, signed + notarized `.dmg`, signed auto-update (ed25519) | Studio UI (`web/studio/`), engine crate | The shell, update channel, file bridge | M | Every Studio view renders natively; a 1 GB IQ file opens; update verifies its signature; notarized |
| L2b | **Kshana Desktop: Devices panel** (lists serial/USB ports, network feeds and SDRs; start/stop `receiver-trust live`, telemetry, IQ capture; read-only by default; `--configure` stays refused) | `open_serial`, live sources, gate, telemetry encoders | UI, device discovery, permissions prompts | M | A USB receiver streams to the Maritime trust view within 60 s of plugging in, from the docs alone |
| L3 | **Kshana Station service** (launchd/systemd/Windows service; config file; local-only API by default, TLS + token for remote per A9; obs/1 out; health endpoint; log rotation) | Reference build (container), `--listen`, telemetry | Native service packaging, obs/1 producer, remote management | M | Station runs 7 days unattended on macOS and Linux arm64 (Raspberry Pi class) with no data loss on replay comparison |
| L4a | **Device connectors, receivers**: live UBX (u-blox) and SBF (Septentrio) binary over USB/serial, NovAtel, NTRIP client (RTCM3 in), C/N0 + AGC + jamming indicators per epoch (feeds B3a) | UBX/SBF/NovAtel/RTCM3 **log** readers (A2b) | Live framing, NTRIP, AGC/jam fields into obs/1 | M | Live and replayed runs of the same recorded bytes give identical results (bit-for-bit) |
| L4b | **Device connectors, SDR**: live IQ capture to SigMF via SoapySDR (RTL-SDR, HackRF, Airspy, LimeSDR, USRP) feeding `iq monitor` | `iq monitor --follow` on a growing SigMF file | Capture, ring buffer, gain/frequency control (receive only) | M | A 30-minute capture at 4 MS/s with no dropped samples on the reference Mac and Pi |
| L4c | **Device connectors, time and traffic**: live linuxptp/chrony state, PPS on Linux, local ADS-B (readsb/dump1090 JSON) and AIS (AIS-catcher) feeds into the emitter region and the maritime trust score | Clock-log and ADS-B/AIS **file** readers (`src/interference_map/adsb.rs`) | Live readers | S | Each feed shows live and matches its replay |
| L5 | **Windows**: signed MSI/MSIX, winget, Windows service for Station, USB-serial on inbox drivers | Unsigned Windows binary, MCP on winget/Scoop | Signing (Azure Artifact Signing if Ashforde validates as an EU organisation; otherwise an OV certificate from a CA, valid ~460 days since March 2026), installer | M | SmartScreen-clean install on a fresh Windows 11; service survives reboot |
| L6 | **Linux**: AppImage, `.deb`/`.rpm`, systemd unit, udev rules for SDRs, `dialout` guidance, arm64 | Unsigned binaries (x86-64, arm64) | Packages, signed repo | S | Installs and runs on Ubuntu LTS, Fedora and Raspberry Pi OS |
| L7 | **Air-gapped install bundle** (all installers, SBOM, checksums, signatures, offline licence) | Air-gapped compose (`pro:deploy/docker-compose.sovereign.yml`) | Desktop/Station bundle | S | Installs with no network; signatures verify offline |
| L8 | **Hardware-in-the-loop evidence**: a bench of one u-blox F9-class receiver, one RTL-SDR v4, one PTP-capable NIC; nightly replay-equals-live test; ledger rows for the live paths | Replay engine, ledger | Bench, CI runner on the bench, rows | S | Live-path rows filed MODELLED with a pre-registered equality bar, then VALIDATED when it holds 30 nights |
| L4d | **More sensors, live**: IMU over serial/USB (log reader exists), NMEA 2000 / CAN (SocketCAN on Linux, USB-CAN gateways), ROS 2 topics in, GPSDO and time-interval counters live (TICC log reader exists), PPS, barometer and wheel odometry for fusion, scintillation monitors (ISMR) | IMU, TICC, NovAtel, RTCM3, RTKLIB, GNSS-SDR **log** readers (`src/receiver_trust/machines/`, clock-log readers) | Live readers, CAN, ROS 2 subscribe | M | Each sensor live equals its replay; fusion kinds accept them unchanged |
| L4e | **Multi-sensor timing and health**: one time base for all inputs (host clock disciplined by PTP/chrony or PPS), per-device latency calibration, hot-plug and reconnect, a watchdog per sensor (stale, frozen, out of range), calibration records kept with the run | Timeline shared by readers (`docs/MACHINE-INPUTS.md`) | Live alignment, latency calibration, watchdog | M | Two sensors on one PPS agree within a pre-registered skew; unplug/replug recovers with the gap marked, never filled |
| L10a | **Use every CPU core, deterministically**: a shared worker pool for Monte Carlo, sweeps, campaigns, the optimiser and IQ correlation (`--threads` defaults to all cores instead of 1); progress and cancel in Desktop | Threads only in `src/sweep.rs` and `iq track --threads` (default 1); no thread pool | Pool, parallel kinds, fixed-order reductions, per-run seeded streams | M | **Same bytes for 1 and N threads** on every parallel kind (CI); ≥ 0.7 × N speed-up on an 8-core Mac for Monte Carlo |
| L10b | **Memory: bounded and streaming**: chunked and memory-mapped reading for IQ and long logs, bounded ring buffers with back-pressure for live sensors, a resource governor (RAM and CPU cap per job; Station budget), no growth over long runs | No memory mapping or governor today | Streaming paths, caps, back-pressure, soak test | M | A 50 GB recording processes within a 2 GB cap; Station RSS flat (±5 %) over 7 days at full sensor load |
| L10c | **Performance budget and sizing**: benchmarks per platform (Apple silicon, Windows x86-64, Raspberry Pi 5) in CI; a Station sizing table (sensors and sample rates per core and per GB); native arm64 builds tuned; GPU (Metal, Vulkan) for IQ acquisition only as a later gate | Bench harness (kshana-bench is accuracy, not speed) | Speed benchmarks, sizing table | S | Published sizing table; regressions over 10 % fail CI |
| L11 | **Sensor plug-in kit**: a new device type added by configuration (framing, fields, units into obs/1) or a sandboxed plug-in, with a conformance test, so a partner adds their sensor with no change to Kshana | `ExternalPack`/`PackRegistry` (compile-time), A2a mapping file (planned) | Runtime device plug-ins on A11, device conformance in A7 | M | A partner-style sensor added from the docs alone in ≤ 4 h (the A definition of done) |
| L9 | **Mac App Store edition** (optional, later): sandboxed build with device entitlements | — | Sandbox audit; serial/USB entitlement check (unverified in this plan; prototype first) | S | Gate: owner, after L2 |

**Rules:** results never depend on the machine: the same inputs give the same bytes on any core count or platform (L10a), and live runs equal their replay (L8). Read-only to hardware by default (the receiver is never configured without an explicit, logged opt-in that stays out of the free build); local-only network binding by default; no telemetry from the app itself; every installer signed, with SBOM and checksums (K2, K9); Pro features unlock by token in the same binary, so there is one download.

**Open checks before L2 starts:** SDR drivers are never bundled: Kshana links only SoapySDR (Boost licence, no notice needed in binaries) and loads the driver the user installed (Homebrew, apt, the vendor installer) at run time, so no GPL code ships in either build (counsel to confirm once); macOS entitlements for serial/USB under the hardened runtime (prototype on a clean Mac); export classification of a desktop that geolocates emitters (K7).

## 13b. Workstream M: space-based RF surveillance, tip-and-cue and target maintenance

**Why:** a smallsat maker has published a two-layer maritime concept: a wide-area RF survey satellite detects suspicious emissions and cues an optical satellite to confirm the vessel, and a "Target Information Maintenance" step keeps re-observing the target so the camera points where the ship *will* be ([vendor article](https://space-inventor.com/news/hunting-dark-ships-in-the-baltic-sea/)). Published figures, all from simulation at 550 km sun-synchronous over the Baltic: revisit 20 min with 16 RF satellites (10 min with 32), full Baltic refresh about 39 min (18 min), RF-to-optical reaction about 15 min with 16 optical satellites (7 min with 32), target speed 16 to 22 kn. Not stated: geolocation method or accuracy, signal classes, optical resolution, flight results. Those gaps are exactly where an independent model and referee helps.

**Kshana's role:** the independent planner, accuracy model and referee for such a system, plus the PNT/timing layer it depends on. It does not build satellites or classify imagery. The strongest tie to Kshana's core: in the Baltic, the "suspicious RF" includes GNSS jammers, which Kshana already models and locates on the ground.

**Exists:** passive TDOA/FDOA geolocation with moving receivers and its Cramér–Rao bound (`src/geolocation.rs`, MODELLED); orbit propagation and constellations (`src/propagator.rs`, `src/constellation.rs`, `src/leo_pass.rs`); Kalman filtering (`src/kalman.rs`); ADS-B and AIS readers and the emitter region (`src/interference_map/`); the maritime trust score; the IQ layer.

| ID | Item | Missing | Size | Done |
| --- | --- | --- | --- | --- |
| M1 | **Space RF geolocation accuracy**: single-satellite Doppler over a pass, formation TDOA/FDOA (2 to 3 satellites), multi-pass fusion; error ellipses over an area of interest per emitter class (AIS VHF, marine radar S/X band, GNSS L1 jammer, satellite-phone L band) | Space receiver geometry on top of `geolocation.rs`; emitter classes; maps | M | CRLB maps reproduce a published space-based geolocation case within a pre-registered bound |
| M2 | **Revisit and tip-and-cue latency**: given planes, satellites, altitude, swath/field of regard and agility, the distribution of revisit and RF-to-optical reaction time over an area | Access/swath model, cueing logic, scenario kind `tip-and-cue` | M | Reproduces the vendor's published Baltic figures (revisit, refresh, reaction) within bars fixed before running; result labelled a cross-check of vendor simulation, not VALIDATED |
| M3 | **Target information maintenance**: track filter from sparse RF fixes (constant velocity and manoeuvring models), prediction error growth with latency, probability the vessel is inside the optical footprint at image time, required pointing | Vessel motion models, capture probability | M | **VALIDATED against public AIS**: on held-out Danish Maritime Authority historical AIS tracks ([DMA AIS data](https://www.dma.dk/safety-at-sea/navigational-information/ais-data)), predicted position after 7, 15 and 39 min meets a pre-registered error bar |
| M4 | **Dark vessels and AIS integrity**: an RF fix with no matching AIS (dark), AIS position inconsistent with the RF fix (spoofed), identity jumps and gaps | Association logic over the existing AIS reader and trust score | S | Scored on a synthetic truth set with known dark and spoofed cases; confusion matrix published |
| M5 | **GNSS jammers seen from orbit**: detectability and location of L1 jammers from LEO (power, FDOA), and a **space + ground hybrid** with fixed stations (B3a, L3) | Space-side link budget for jammers; hybrid estimator | M | Hybrid beats either layer alone on a synthetic Baltic scenario; bar pre-registered |
| M6 | **Timing for the space layer**: inter-satellite clock sync and orbit knowledge needed for a target TDOA/FDOA accuracy, and holdover when the satellites' own GNSS is degraded | Budget built on the timing and holdover kinds | S | Budget table per accuracy class; MODELLED |
| M7 | **Synthetic RF from orbit**: IQ scenes with orbital Doppler and path loss for AIS (GMSK), pulsed marine radar and jammers, exact labels, for classifier training | Signals (with B10), orbital channel | M | Same seed, same file; labels exact |
| M8 | **Exchange and referee**: detections and cues in via Connect (A2a); outputs as CZML/KML, CCSDS OEM, obs/1; referee metrics (detection latency, location error, capture rate) per B2 | Mappings, metrics for cue chains | S | A partner's detection CSV maps with no code and is scored |
| M9 | **Pro pack: maritime tip-and-cue study**: constellation trade (pairs per plane vs revisit, reaction and capture rate), with the design optimiser (B1b cost axis) | Pack on top of M1 to M3 | S | Study reproduces from scenario and seed |

**Guardrails:** public repository wording stays neutral ("space RF survey", "optical cue", no company or programme names, no actors or intent); vendor figures are labelled as the vendor's simulation, and Kshana's own results start MODELLED; dual-use: export classification before any partner delivery (K7); outreach to the vendor is the owner's call (gate).

## 14. Owner gates

1. Go per workstream.
2. Ranking of the A2c top five.
3. A10 licence decision and FAQ; A11 CLA/DCO.
4. B3 pre-registration timing (check Jammertest 2026 publication first).
5. Partner outreach, data agreements, consent before any public listing (E7, E9, J5).
6. Pricing, MSA, trademark, counsel reviews, payment go-live, export control (F4, F6, K7, F9).
7. The Artemis I manoeuvre-aware pre-registration (C3).
8. Research authorship and governance (I).
9. **Native install (L):** Apple Developer ID certificates and an App Store Connect API key stored as CI secrets; the Windows signing route (Azure Artifact Signing organisation validation, or an OV certificate); counsel on SDR driver licences; whether to pursue the Mac App Store (L9).
10. **Space surveillance (M):** outreach to the vendor **after the 0.41 release** (owner decision, 3 November 2026), presenting M1 to M4 and M6 as shipped and M5, M7 to M9 as the plan; data access; export classification (K7) before any delivery.

## 15. Pre-start checklist for any go

When the owner says go on a workstream:

1. Re-read this plan.
2. Re-verify every Exists cell it touches.
3. File the pre-registrations before any data is examined.
4. Branch from current main.
5. Commit as Ashforde OÜ, with no attribution lines, neutral public wording and no force-push.

## 16. Schedule (every ID appears once)

| Release | IDs |
| --- | --- |
| **0.41** | L1; M1, M2, M3, M4, M6; F1a, F1b; A1, A2a, A2b, A2c (top five), A4, A5a, A7a, A12; B1a, B1b, B2, B3-gate filed, B8; C1, C2; D11, D17, D18; H1, H2, H3a, H3b, H4, H5, H6, H8, H10; K1, K3a, K8; J2, J3, J4, J6 |
| **0.42** | L2a, L2b, L3, L4a, L4c, L4e, L8, L10a, L10b, L10c; A3a, A6, A7b, A8a, A9; B3a, B3b, B5, B7, B10; C3, C7, C9a–c; D1 (with C3), D2, D3 (with B3), D4, D8a, D13, D14; E1–E6, E8; G1a, G1b, G3, G5; H9; K2, K3b, K4, K6, K9 |
| **0.43** | L4b, L4d, L5, L6, L7, L11; M5, M7, M8, M9; A2c (remaining), A3b, A5b, A8b, A11; B4a, B4b, B6, B9; C5a, C5b, C9d–e; D5, D7, D8b, D12, D15; F2a, F2b, F3a, F3b, F3c, F5, F7, F8a–d; G2, G4, G6, G8; H7; K5 |
| **Gate** | L9, A10, B3-gate decision, C4, C6, C8, D6, D10 (~early 2027), D16, E7, E9, F4, F6, F9, I1–I6, J5, K7 |
| **Parked** | D9, G7 |
| **Owner-run, under way** | J1 |

## 17. What the audits changed

**Audit 1 (revision 1 → 2):**

- Cited Exists cells (CCSDS ODM/ADM/SPK/TDM, linuxptp/chrony, syslog/MQTT, live NMEA, Pro OpenAPI, air-gapped stack,
  token auth, telecom-timing, ROI, legal templates).
- Removed the false Helm claim.
- C ABI as a separate crate.
- New items: integrator licensing; a testable plug-and-play definition of done with reference integrations; industry
  adapters; adapter registry.
- New sections: positioning, competitive map and KPIs; adoption readiness (K).
- Partners: PARTNER-input labelling, partner-tool links and certification.
- Ordering fixes.

**Audit 2 (revision 2 → 3):**

- **Exists cells corrected:**
  - OEM was never loaded into GMAT/STK (C1, J6);
  - bench licence pending (A10 is now a gate);
  - `ExternalPack`/`PackRegistry` exist (A11 builds on them);
  - signed multi-arch images and the result envelope exist;
  - telemetry carries the receiver-trust stream only;
  - OTLP is off by default and has no TLS;
  - file following, motion export and route scoring exist;
  - readers share `Timeline`.
- **F1 resized:** a 0.32 → 0.40 port, scheduled first.
- **New D rows:**
  - the IQ NOT MET rows;
  - alternative PNT (parked);
  - UTC(k) round 3;
  - DE440 Sun/Moon;
  - transfer optimiser, LEO Doppler;
  - campaign Monte Carlo and optimise;
  - lunar frame datum, spectrum bands;
  - ARAIM 4/12;
  - lunar reference station;
  - D11c/D12;
  - W57 corrected to W58 lunar-infrastructure leftovers.
- **New items:**
  - telecom-timing tables (G1a);
  - compliance open tests (G4);
  - OD and frames follow-ons (C9);
  - IQ signal breadth (B10);
  - LunaNet receiver comparison (C8);
  - surface parity (A12);
  - scenario-schema versioning (A1);
  - procurement pack (K9);
  - hosted trial and structured study intake (F3b, F3c);
  - signed dossier (F7);
  - Pro limits (F2a, F8);
  - MSA missing (F6);
  - PR #14 export correction (F4);
  - grants (F9);
  - test-lab partners R1–R9 (E8) and other partner shapes (E9);
  - IEEE P1952 and upstream contributions (I);
  - gendered-language messages (H3);
  - Studio gaps and bring-your-own-logs (H9);
  - Dependabot triage, issue #36, SBOM re-run (H2, H10);
  - PyPI Trusted Publishing (K2);
  - arm64 image republish (H6);
  - a public roadmap (J4);
  - the cost model for the planner (B1b).
- **Consistency:**
  - every ID scheduled, gated or parked;
  - K3 split to follow its dependencies;
  - F1 before A5a/B1b;
  - guides split by shape;
  - L/M+M items split;
  - every table has a Done column;
  - gate names fixed;
  - the launch marked owner-run;
  - the precondition carried.

**After the 0.40.0 release (owner instruction: every 0.40 leftover goes in 0.41):**

- H6 widened to the MCP server image, the MCP registry entry and channel parity that the release did not publish, plus native arm64 builds.
- H3 split: H3a (Studio English strings and W55 leftovers) and H3b (gendered-language messages, already promised for 0.41) both in 0.41.
- H5 (translated READMEs at each release) moved to 0.41.
- J3 notes the open winget pull request.

**After the owner's request of 3 November 2026 (native install; the published space RF tip-and-cue concept):**

- New workstream L (§13a): signed and notarized macOS first (L1 in 0.41), Kshana Desktop on Tauri reusing the Studio UI, Kshana Station as the network node for B, live receiver/SDR/clock/traffic connectors, Windows and Linux in 0.43, hardware-in-the-loop evidence.
- New workstream M (§13b): space RF geolocation accuracy, revisit and tip-and-cue latency (cross-checking the vendor's published figures), target maintenance validated on public Danish AIS, dark-vessel and AIS-integrity logic, GNSS jammers from orbit with a space + ground hybrid, timing budget, synthetic RF, exchange and a Pro study.
- Gates 9 and 10 added; competitive map extended; every new ID scheduled.
- Owner decision: approach the vendor after 0.41, so M1, M2, M3, M4 and M6 move into 0.41 (the accuracy model, revisit and reaction cross-check, target maintenance validated on public AIS, dark-vessel and AIS-integrity logic, timing budget); M5, M7 to M9 stay in 0.43 as the presented plan. Windows signing route: Azure Artifact Signing first, an OV certificate on a cloud signing service as fallback. SDR drivers: none bundled; SoapySDR (Boost) loads the user-installed driver at run time.
- Same day, on the owner's question about CPU, RAM and sensors: L10a (all cores, same bytes for 1 and N threads), L10b (bounded memory, streaming, resource governor, 7-day soak), L10c (speed benchmarks and a Station sizing table), L4d (IMU, NMEA 2000/CAN, ROS 2, GPSDO/TICC, PPS, odometry, scintillation monitors live), L4e (one time base, latency calibration, hot-plug, watchdog), L11 (sensor plug-in kit).
