# Kshana roadmap: 0.41 to 0.43

> **Status: not started. Nothing here begins (no code, branches or sessions) until the owner gives an explicit go,
> workstream by workstream.** Every workstream can start on its own; dependencies are stated.

Private planning document (kshana-pro). Revision 2: rewritten after an independent audit (§15 lists what changed).
Public material stays neutral: no company names, no actors or intent.

## 0. How to read this

- **Sizes:** S = days, M = 1–2 weeks, L = 3–6 weeks for one workstream. Nothing larger than M is started as one item:
  L items are split into named sub-items.
- **Exists** cells cite the file that proves it; **Missing** is only what no file shows. Paths starting `src/` or
  `docs/` are in the public engine repo; `pro:` means kshana-pro.
- **Done-test:** every item carries a measurable acceptance test or a pre-registered bar.
- New capabilities start MODELLED, with the bar fixed before reference data is examined. Partner data is labelled
  PARTNER. A commercial relationship never changes a verdict.

## 1. Position and goals

Kshana is the neutral, open referee and planning layer for PNT resilience. It is not a cheaper RF simulator. The moat is:

- the honesty ledger (every figure labelled, misses published);
- reproducibility (scenario, seed and engine version → the same bits);
- signed evidence;
- reach across every surface (browser, CLI, packages, MCP).

Full strategy: `pro:docs/strategy/2026-10-18-kshana-standard-strategy.md`. Move 9 of that memo (governance) is still open.

### 1a. Competitive map

| Category | Examples (class) | Kshana's stance |
| --- | --- | --- |
| RF / constellation simulators (hardware) | Lab signal generators | Complement: Kshana scores what their receivers do and models the operational consequence; import their scenario and log files (A2) |
| Interference-monitoring networks | Fixed-station and crowd networks | Complement: referee, planner, labelled data, consequence (B) |
| Flight-dynamics and astrodynamics tools | GMAT, Orekit, STK, Basilisk, TMPL-class | Complement: independent cross-check, exchange formats (C) |
| Open receivers and SDR stacks | GNSS-SDR, RTKLIB | Integrate: readers and plugins exist; scoring on top |
| Resilience frameworks and consultancies | Assessment frameworks | Supply the evidence layer: compliance mapping, self-assessment (G4) |

### 1b. Dominance KPIs (measured at each release)

| KPI | Now (0.40) | Target by 0.43 |
| --- | --- | --- |
| Verified integrations ("Works with Kshana", A7) | 0 | 10 |
| Scored public datasets (bench and referee) | Jammertest, TEXBAT and a few others | +5 |
| Ledger rows promoted (MODELLED → VALIDATED or PARTNER) | 176 VALIDATED | +25 |
| Paying design partners | 0 | 3 |
| Citations / external papers using Kshana | Read from Zenodo and Google Scholar at each release | +10 |

## 2. Workstream A: plug-and-play integration layer ("Kshana Connect")

**Goal:** a company connects with configuration only: no special development by us or them, now or later.

**Definition of done (testable):**

- There is one named **reference integration per customer shape** (§3), each replayed from a public dataset.
- An integration passes when all of these hold:
  - `kshana connect check` is green;
  - an engineer outside the project completes it from the docs alone in **≤ 4 hours**;
  - **zero code changes** are needed in either repo.
- CI repeats every reference integration at every release.

| # | Item | Exists (cited) | Missing | Size | Done-test |
| --- | --- | --- | --- | --- | --- |
| A1 | **Canonical schema `kshana.obs/1`** for everything a station or system produces: PVT, C/N0 and AGC per signal, clock offset, spectrum summaries, alarms, IQ references, OSNMA status, site/antenna/receiver metadata, **and the partner-input fields** (antenna patterns, device sheets, bus constraints) so E2–E5 never fork it | Internal structs per reader; `kshana.partner-submission/1` (`docs/PARTNER-DATA.md`) | Versioned JSON Schema, validator `kshana connect validate`, schema docs, semver and deprecation policy | M | Every A2 reader's output validates; schema published under a permissive licence (A10) |
| A2a | **Mapping file `connect.toml`**: CSV/JSON/Parquet/HDF5 columns → schema fields, units, time base; no code | — | All | M | Three unrelated vendor CSVs map with no code |
| A2b | **Receiver and timing readers (delta only)** | RINEX, UBX, SBF, NovAtel, RTCM, NMEA, SigMF, ANTEX, SP3, CCSDS, SPICE; linuxptp (ptp4l, phc2sys, ts2phc) and chrony (`docs/CLOCK-LOG-FORMATS.md`) | White Rabbit and ntpd logs; Trimble/Leica logs; RF-simulator scenario and motion files (import/export) | M | Each reader round-trips its test vector |
| A2c | **Industry feeds**, in owner-ranked order (top five for 0.41) | ADS-B/AIS reports (`src/interference_map/`) | Aviation: ASTERIX CAT021/023, ARINC 424/429, GNSS-RFI report formats, NOTAM. Maritime: NMEA 2000, IEC 61162-450. Rail: ETCS/ERTMS position reports. Automotive: CAN/DBC, ROS bag, ASAM OpenSCENARIO. Power grid: IEEE C37.118 (PMU time quality), IEC 61850-9-3. Telecom/5G: G.8275 profiles, O-RAN M-plane sync alarms, SyncE ESMC. Finance: RTS 25 clock-traceability reports. Handsets: 3GPP LPP | L (split by industry) | Each feed has a public sample and a passing replay |
| A3a | **Live ingest, core** | Live NMEA over stdin/TCP/UDP (`src/live_io.rs`); MQTT publish only, no TLS (`src/telemetry/mqtt.rs`) | MQTT subscribe with TLS; WebSocket; file-drop watch; all into `kshana.obs/1` with time alignment | M | 100 stations × 1 Hz sustained on the reference box; replay from a stored stream |
| A3b | **Live ingest, scale** | — | Kafka and NATS; back-pressure; load-test harness | M | 1,000 stations × 1 Hz; latency budget met (K3) |
| A4 | **Outputs and alerts** | JSON/CSV/HTML reports, Prometheus, OTLP, Grafana (`deploy/grafana`), syslog CEF/LEEF and MQTT publish (`src/telemetry/`), CZML/KML/GeoJSON/STK/SigMF | Signed webhooks; CAP alerts; SNMP traps; stable `kshana.result/1` schema; scorecard export for insurers and regulators | M | Each sink receives a test alert end to end |
| A5a | **Service API** | OpenAPI 3.1 served by Pro (`pro:src/serve/openapi.rs`, `pro:docs/SERVICE.md`) | Generated clients (Python, TypeScript, Go, Java); a versioning and deprecation policy across all surfaces; gRPC for high-rate streams | M | Clients generated in CI and tested against the service |
| A5b | **C ABI** as a **separate crate `kshana-capi`** (the core keeps `#![forbid(unsafe_code)]`, `src/lib.rs`; this guarantee stays and is restated in SECURITY.md) | — | cbindgen header, safety review, examples for C/C++/Go/Java/MATLAB/LabVIEW | M | Fuzzed FFI boundary; examples build in CI |
| A6 | **Deployment kits** | Docker images (reference build, MCP); air-gapped compose stack (`pro:deploy/docker-compose.sovereign.yml`, `pro:deploy/DEPLOY.md`); Signal K, OpenCPN, ROS 2, GNSS-SDR integrations | **Helm chart (none exists)**; Compose for "engine + ingest + service + Grafana"; signed multi-arch images; hardware sizing guide | M | Install on a clean cluster and air-gapped host from docs only |
| A7a | **Conformance kit, files** | `kshana partner check` (format of partner submissions) | Test vectors and replay corpus per adapter; `kshana connect check` for file integrations | M | Run by CI on every reference integration |
| A7b | **Conformance kit, live** (after A3) | — | Live checks: latency, ordering, reconnect | S | As A7a for live paths |
| A8 | **Integration guides**, one per customer shape, each with a public-data worked example; translated as the docs are (L5) | Surface, MCP and deploy docs | The guides | M | Each guide is the script of a reference integration |
| A9 | **Integration security** | Bearer-token auth (`--api-token-file`, `pro:docs/SERVICE.md`; not user management) | OIDC, per-tenant isolation, audit log, data retention, PII-free crowd ingest by default | M | Pen test (K2) passes |
| A10 | **Licensing for integrators** | AGPL-3.0 + commercial licence | Schemas, conformance vectors and generated clients under MIT or CC-BY-4.0 (as was done for the bench spec); a one-page **integrator licensing FAQ** (file/network separation vs linking) | S | Counsel-reviewed FAQ published |
| A11 | **Adapter registry and community adapters** | — | Out-of-tree adapter interface (mapping file, or a WASM/Python plug-in), registry, CONTRIBUTING rules, review checklist | M | First external adapter merged into the registry without engine changes |

**Order:** A1 → A2a, A7a, A10 → A2b/A2c, A4, A5a → A3a → A7b, A6, A9 → A3b, A5b, A11.

## 3. Reference integrations (one per customer shape)

| Shape | Reference integration (public data) | Uses |
| --- | --- | --- |
| Interference-monitoring network (SJAWS-class) | Station logs replayed from Jammertest; referee scorecard, area trust state, alerts to Grafana and a webhook | A1–A4, B2, B5 |
| Flight-dynamics house (KinetX / IM class, TMPL-class users) | Public trajectory exchanged as CCSDS OEM/OPM and SPK; difference report | C1, C7 |
| Port / airport / telecom / grid operator | Timing log (ptp4l/chrony) and a scheduled jamming event; holdover consequence and CAP alert | A2b, A4, B6, G1 |
| Receiver / antenna / clock vendor | Vendor log on a public test campaign; bench track and "Works with Kshana" listing | A7, E6 |
| Hardware partner | Antenna pattern or device sheet driving a study, labelled PARTNER-input | E2–E5 |
| Test event / programme | Pre-registered scoring of a campaign; signed evidence pack | B2, B4 |
| Tooling / AI ecosystem | MCP and generated clients in a notebook | A5a |

## 4. Workstream B: interference-monitoring networks (SJAWS-adjacent)

Detail: `0.41-BACKLOG.md` §1.

| # | Item | Size | Depends on | Done-test / bar |
| --- | --- | --- | --- | --- |
| B1 | Monitoring-network planner (detection and location-accuracy maps, station count and placement); its Pareto front uses the existing Pro optimiser | M (+S for Pro Pareto) | — | Reproduces the published geometry and accuracy of a public multi-station TDOA campaign within a pre-registered bound |
| B2 | Location metrics in the referee (error, confidence-region coverage, time to locate) | M | — | Metrics match hand-computed values on a synthetic truth set |
| B3 | Fixed-station emitter locator (C/N0 + AGC power-difference, TDOA with IQ) | M + M | A1, A2a | **Pre-registration filed before Jammertest 2026 data is published (dated gate)** |
| B4 | Jam / spoof / meaconing / natural benchmark | M + M | — | Bars set in advance per class; results published met or not |
| B5 | Area trust state from several reference stations | M | A1, A3a | Alert latency ≤ 5 s end to end (K3) on replay |
| B6 | Port and airport scenario pack | M | — | Consequence figures reproduce from scenario and seed |
| B7 | Synthetic labelled data packs (IQ and logs) | M | — | Same seed, same file; labels exact |
| B8 | Environmental baseline (expected natural degradation per site and time) | S | — | Compared with documented event days |
| B9 | Two-way data with the maritime trust score | M | A3a, A4 | Round trip on replay |

## 5. Workstream C: navigation and flight-dynamics tools

| # | Item | Exists (cited) | Missing | Size |
| --- | --- | --- | --- | --- |
| C1 | Exchange formats (delta) | OPM/OMM/OCM KVN+XML (`src/odm.rs`), AEM/APM (`src/adm.rs`), SPK types 9/13/3/21 (`src/spk_writer.rs`), TDM read/write (`src/ccsds_tdm.rs`), OEM checked with GMAT/Orekit/STK, Orekit round trip (`docs/INTEROP.md`) | CDM; SPICE CK; round-trip tests with GMAT and STK | S |
| C2 | Lunar orientation check against the NASA SVS Moon phase and libration tables (the ledger cites NASA SVS only) | DE440 lunar orientation | Comparison, pre-registered bars, ledger row | S |
| C3 | Close the cislunar misses: manoeuvre-aware second run (draft awaits the owner), NRHO force model | Artemis I, CAPSTONE comparisons (NOT MET) | As stated | M |
| C4 | Real tracking data (DSN passes) | TRK-2-18 reader, Mars ramped Doppler | Joint comparison (partner) | M, gate |
| C5 | Optical navigation and small-body gravity, or a partner study first | B-plane, light-time | Measurement models | M + M |
| C6 | Lander navigation against flown data | Descent/surface/ascent kinds (MODELLED) | Partner data | M, gate |
| C7 | Thin adapters and notebooks for GMAT, Orekit, Basilisk, TMPL-class libraries | MCP, Python, Rust | Adapters | M |

## 6. Workstream D: close the open evidence gaps

No bar is loosened; each stays NOT MET until a new pre-registered run meets it.

| Gap (0.40) | Path | Where |
| --- | --- | --- |
| Artemis I (45.3 km vs 5 km), CAPSTONE (136.8 km vs 25 km) | Manoeuvre-aware run; NRHO force model | C3 |
| Clock-log F1(a), F2, D2 | Better noise identification; measured holdover vs one predicted from its own earlier data | D, M |
| Emitter region NOT ASSESSABLE | Fixed stations, Jammertest 2026 | B3 |
| kshana-bench A2, D1, R1 | Stronger baselines; re-run R1; second OOD track for task (d) | D, M |
| Service volume S1–S5, S7 | Resolve the two GLONASS reference angles; held-out reception campaign | D, M |
| Lunar geodesy (3 bars) | Measured lunar quantities or partner data | D, gate |
| LunaNet AFS (one printed column) | Recheck at the specification's next issue | D, S |
| W57 leftovers | Almanac-HDOP scorer, pin TEXBAT hash, Studio view for C3 | D, S |

## 7. Workstream E: partners and partner areas

**Rules:**

- The four PARTNER rows (bus engineering, RF payload and antenna hardware, quantum payload hardware, flight-hardware
  product assurance; `docs/VERIFICATION-MATRIX.md`) stay "Not provided". The interfaces below let a partner's own data
  drive studies.
- That data is labelled **PARTNER-input**. No row changes label because an interface exists.

| # | Partner area | Exists | Interface (no development for the partner) | Size |
| --- | --- | --- | --- | --- |
| E1 | Data partners (9 MODELLED rows; holdover floors first) | Partner kit (`docs/PARTNER-DATA.md`), `kshana partner check` | Data agreement template, signed upload, pre-registration form, scoring report | M |
| E2 | RF payload and antenna | ANTEX reader, CRPA model, link budgets | Measured-pattern CSV and **CST/HFSS pattern export import**, CRPA array file, payload tables (EIRP, G/T, masks) | M |
| E3 | Quantum payload | Quantum sensor/clock models, clock twin | Device performance sheet (bias stability, Allan profile, bandwidth, sensitivities) | S |
| E4 | Spacecraft bus | Mission thread | Navigation-relevant bus constraints from **SysML v2 / Capella** exports | S |
| E5 | Flight-hardware product assurance | Pro traceability (CSV, ReqIF, SysML v2), dossier | PA evidence links by requirement id (**ECSS-Q / Polarion ids**), no copying of their data | S |
| E6 | Clock and receiver vendors | Clock twin, receiver trust, readers | Datasheet import; vendor bench track; listing via A7 | M |
| E7 | **Partner programme and certification** | Outreach drafts (not sent), timing-partner shortlist | Partner page; tiers (data, technology, channel) with entry criteria; certification lifecycle: apply → conformance kit → publish → **recertify every minor release** → revoke on failure; fees in F6; badge says "format conformance, not performance"; conflict-of-interest line on any scorecard of a commercial partner | S |

## 8. Workstream F: Kshana Pro

| # | Item | Status (cited) | Size / gate |
| --- | --- | --- | --- |
| F1 | Release readiness: re-pin to the published engine, `--locked`, signed licence token end to end (`pro:docs/PRO-POWER.md` §7) | Not done | S |
| F2 | Connect in Pro: A-layer service with tenancy, audit log, SLA metrics | Missing | M + M (after A) |
| F3 | Hosted / on-prem platform: multi-tenant jobs, saved studies, team sharing | Missing | M + M |
| F4 | Pack B (timing study): a draft export questionnaire exists (`pro:legal/EXPORT-CONTROL-DETERMINATION.md`); next step: counsel completes the determination | Gate: counsel | — |
| F5 | Pack C (subscription, CI guide) | Partial | M |
| F6 | Commercial: templates exist (DPA, NDA, SLA/SOW drafts, `pro:legal/`); gates are counsel review, pricing (`pro:docs/strategy/PRICING-PROPOSAL.md`), trademark, payment go-live; **connector packaging and pricing** | Gate: owner/counsel | — |

## 9. Workstream G: missing domain features

| # | Item | Exists | Missing (delta) | Size |
| --- | --- | --- | --- | --- |
| G1 | Network timing resilience | `telecom-timing` kind, G.8273.2 T-BC/T-TSC (`docs/TELECOM-TIMING.md`) | Multi-hop PTP/White Rabbit distribution, failover, holdover across a network | M |
| G2 | Time-transfer authentication (NTS, PTP auth TLV), spoofed-time detection | — | All; attaches to G1 | M (after G1) |
| G3 | Outage cost and ROI | ROI layer (`src/frugal.rs`) | Operational-impact model reusing it | S |
| G4 | Guided self-assessment mapped to resilience frameworks | Compliance report (`docs/compliance/`) | Questionnaire | M |
| G5 | Fleet and route risk dashboards | Interference maps | Route scoring and dashboards | M |
| G6 | Gap-remediation planner (weakest link → ranked mitigations) | Mission thread weakest link | Planner | M |
| G7 | Underwater acoustic PNT | — | Parked | — |

## 10. Workstream H: quality, platform, i18n

| # | Item | Size |
| --- | --- | --- |
| H1 | SonarCloud new-code coverage ≥ 80% (about 810 lines; CLI paths first, by in-process tests) | M |
| H2 | CI: shard the suite; the release `verify` keeps CI's settings and limits (fixed in 0.40; keep it tested) | S |
| H3 | Studio English strings on translated pages; native-speaker review programme; one-pass rendering | M |
| H4 | CHANGELOG correction (RUSTSEC-2026-0387, MSRV 1.88 under 0.40.0); README MCP table cells | S |
| H5 | Translated READMEs regenerated at each release | S |
| H6 | Release dry-run before every tag | S |
| H7 | The 0.40 browser audit (every page × 18 languages × widths) as a CI job | M |
| H8 | A CI check that no company name appears in the public repository or the "Works with Kshana" list without recorded written consent | S |

## 11. Workstream K: adoption readiness (non-functional)

| # | Item | Size |
| --- | --- | --- |
| K1 | **Support and LTS:** a 1.0 stability criterion; an LTS branch with a 12-month patch window (SECURITY.md now supports only the latest release) | S + ongoing |
| K2 | **Security:** IEC 62443-4-1 process readiness, ISO 27001 readiness for the hosted service, pen test of the service, TLS on all ingest and sinks, signed adapters | M |
| K3 | **Real-time targets:** alert latency ≤ 5 s end to end (B5); ingest throughput per A3a/A3b; a load-test harness in CI | S |
| K4 | **Observability of Kshana itself:** metrics and traces for ingest and the job service | S |
| K5 | **Accessibility:** WCAG 2.2 AA audit and fixes for Studio and the site | M |
| K6 | **Data protection:** GDPR DPIA template for crowd and partner data; per-tenant data residency | S |
| K7 | **Export control** classification for Connect, the C ABI and the air-gapped bundle (not only Pack B) | Gate: counsel |
| K8 | **Documentation:** docs versioned per release; translation plan for Connect guides | S |

## 12. Workstream I: research, standards, governance (owner-gated)

- JOSS tools paper (author decisions, external co-author).
- AI/ML impairment-evaluation testbed paper, IEEE/ION PLANS 2027.
- ION GNSS+ and ENC submissions.
- **Standards bodies** where Connect formats and the referee could become reference material: RTCM SC-134, EUROCAE WG-62, ITU-T Q13/15.
- Governance: TSC charter, oracle registry, licence split.

## 13. Workstream J: market and launch

**Metrics per release:**

- Studio sessions;
- installs across crates.io, PyPI and npm;
- GitHub stars;
- studies requested;
- verified integrations;
- design-partner conversations.

**Items:**

- Product Hunt (2 November): first comment, replies, Show HN, r/GNSS, r/rust.
- Listings: MCP registry publish (owner variable), winget moderation.
- "Request a study" call to action live; partner page (E7).
- Pack A outreach to the first paying design partner; Jammertest scoring presence; the browser course for universities.

## 14. Owner gates

1. Go per workstream.
2. Ranking of the A2c top five industries for 0.41.
3. Partner outreach and data agreements; any contact with a named company or test-event organiser; written consent before any company appears on a public list.
4. Pricing, trademark, counsel reviews (licensing FAQ, export control, DPA), payment go-live.
5. Draft C2 (Artemis I manoeuvre-aware run).
6. Research authorship and governance.

## 15. Sequencing

| Release | Contents |
| --- | --- |
| **0.41** | A1, A2a, A2b, A2c (top five), A4, A5a, A7a, A10; B1, B2, B3 pre-registration filed; C1, C2; H1, H2, H4; K1, K3 |
| **0.42** | A3a, A6, A7b, A8, A9, A11; B3, B5, B7, B8; C3, C7; E1–E7; G1, G3; K2, K4, K6, K8 |
| **0.43** | A3b, A5b; B4, B6, B9; C4–C6; D remaining; F2, F3; G2, G4–G6; H3, H7; K5 |

## 16. What the audit changed (revision 1 → 2)

- **Corrected "Exists" cells, each now cited:**
  - CCSDS OPM/OMM/OCM/AEM/APM and SPK writing exist, so C1 shrinks to S;
  - linuxptp and chrony readers;
  - syslog CEF/LEEF and MQTT output;
  - live NMEA input;
  - Pro's OpenAPI;
  - the air-gapped stack;
  - token authentication;
  - the telecom-timing kind;
  - the ROI layer;
  - the existing legal templates.
- **Removed:** the false "Helm references" claim.
- **C ABI:** now a separate crate, so the core's no-unsafe guarantee stands.
- **New items:**
  - integrator licensing (A10);
  - a testable plug-and-play definition of done, with reference integrations;
  - industry adapters (aviation, maritime, rail, automotive, grid, telecom, finance, handsets, academia);
  - an adapter registry (A11).
- **New sections:** positioning, the competitive map and dominance KPIs (§1a, §1b); adoption readiness (K: LTS, security, latency, accessibility, data protection, export control, docs).
- **Partner areas:** PARTNER-input labelling, partner-tool links (CST/HFSS, SysML v2/Capella, ECSS-Q/Polarion), and a certification lifecycle.
- **Ordering fixes:**
  - conformance split into file and live parts;
  - the B3 pre-registration dated before Jammertest 2026;
  - partner fields placed in A1;
  - G2 after G1;
  - L items split.
- **Also added:** acceptance tests on every item; market metrics; standards bodies; NASA SVS cited directly for C2.
