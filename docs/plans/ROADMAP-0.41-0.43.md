# Kshana roadmap: 0.41 to 0.43

> **Status: not started. Nothing here begins (no code, branches or sessions) until the owner gives an explicit go,
> workstream by workstream.** Owner-run activities already under way (the 2 November Product Hunt launch, J1) are the
> only exception. Precondition met: the kshana.dev site-audit fixes are live (0.40).

Private planning document (kshana-pro). **Revision 3**, after two independent audits against both repositories and
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
| H3 | i18n: English strings on translated Studio pages; **whole-sentence messages for gendered languages** (promised for 0.41, `pro:docs/plans/0.40-INTEGRATION-BRIEF.md`); leftover Rust-built English words (W55); native-speaker review programme; one-pass rendering | M | No fragment-assembled sentence; review sign-offs per language |
| H4 | CHANGELOG (RUSTSEC-2026-0387, MSRV 1.88 under 0.40.0); README MCP table cells; `docs/SURFACES.md` drift (Studio views now exist) | S | Docs tests green |
| H5 | Translated READMEs regenerated each release | S | Release job |
| H6 | Release dry-run before every tag; republish and verify the 0.40.0 arm64 images | S | Dry-run job; images verified |
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
| J3 | Listings: MCP registry (owner variable), winget moderation, OpenCPN catalogue PR (`packaging/channels.json`) | Listed |
| J4 | Publish a **neutral public roadmap** (`ROADMAP.md` does not reflect 0.41–0.43) | Public file updated |
| J5 | Pack A outreach (first paying design partner); Jammertest scoring presence; browser course for universities | **Gate: owner** |
| J6 | Correct the outreach material: the flight-dynamics fit document claims OEM exchange with GMAT/STK as VALIDATED (see C1) | Corrected copies issued |

## 14. Owner gates

1. Go per workstream.
2. Ranking of the A2c top five.
3. A10 licence decision and FAQ; A11 CLA/DCO.
4. B3 pre-registration timing (check Jammertest 2026 publication first).
5. Partner outreach, data agreements, consent before any public listing (E7, E9, J5).
6. Pricing, MSA, trademark, counsel reviews, payment go-live, export control (F4, F6, K7, F9).
7. The Artemis I manoeuvre-aware pre-registration (C3).
8. Research authorship and governance (I).

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
| **0.41** | F1a, F1b; A1, A2a, A2b, A2c (top five), A4, A5a, A7a, A12; B1a, B1b, B2, B3-gate filed, B8; C1, C2; D11, D17, D18; H1, H2, H4, H6, H8, H10; K1, K3a, K8; J2, J3, J4, J6 |
| **0.42** | A3a, A6, A7b, A8a, A9; B3a, B3b, B5, B7, B10; C3, C7, C9a–c; D1 (with C3), D2, D3 (with B3), D4, D8a, D13, D14; E1–E6, E8; G1a, G1b, G3, G5; H3, H5, H9; K2, K3b, K4, K6, K9 |
| **0.43** | A2c (remaining), A3b, A5b, A8b, A11; B4a, B4b, B6, B9; C5a, C5b, C9d–e; D5, D7, D8b, D12, D15; F2a, F2b, F3a, F3b, F3c, F5, F7, F8a–d; G2, G4, G6, G8; H7; K5 |
| **Gate** | A10, B3-gate decision, C4, C6, C8, D6, D10 (~early 2027), D16, E7, E9, F4, F6, F9, I1–I6, J5, K7 |
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
