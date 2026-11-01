# Kshana roadmap: 0.41 to 0.43

> **Status: not started. Nothing here begins (no code, branches or sessions) until the owner gives an explicit go,
> workstream by workstream.** Every workstream below can be started on its own; dependencies are stated.

Private planning document (kshana-pro). Public material stays neutral: no company names, no actors or intent.

## 0. How to read this

- **Sizes:** S = days, M = 1–2 weeks, L = 3–6 weeks of one workstream.
- **Exists / Missing:** what the engine or Pro already has today, and what is not built.
- **Gate:** an owner decision or external party the item waits on.
- Every new capability starts MODELLED with its bar pre-registered before any reference data is examined. A partner's data
  is labelled PARTNER; a commercial relationship never changes a verdict.

## 1. Where Kshana stands and what this roadmap is for

Kshana's position is the neutral, open referee and planning layer for PNT resilience: the honesty ledger,
reproducibility, signed evidence and reach across every surface. 0.40 delivered the mission thread, per-figure evidence
labels, measured clock logs, the partner kit, the emitter region and 18 languages.

This roadmap does three things:

1. **Plug-and-play for any company (Workstream A).** A monitoring network, receiver or clock vendor, flight-dynamics
   house, operator, test event or hardware partner connects with configuration only: no special development by us or
   them. This is the first priority and the base for everything else.
2. **Build the capabilities those companies need (B, C, G):** interference-monitoring networks, navigation and
   flight-dynamics tools, network timing.
3. **Close the open evidence and partner gaps (D, E),** harden quality (H), and prepare Pro, research and market work
   (F, I, J).

### Who it is for (customer shapes)

| Shape | Example class | What they need from Kshana |
| --- | --- | --- |
| Interference-monitoring network | SkySweeper-class (SJAWS), airport/port monitoring | Referee scorecard, site planner, labelled test data, consequence model, live ingest of their station logs |
| Navigation / flight-dynamics house | KinetX / Intuitive Machines class, TMPL/GMAT/Orekit/STK users | Independent cross-check, trade studies, mission budgets, lunar service analysis, CCSDS/SPICE exchange |
| Operator of critical timing or positioning | Ports, airports, telecom, grid, finance | Holdover and consequence studies, site trust state, alerts into their tools |
| Receiver / antenna / clock vendor | Receiver makers, CRPA vendors, clock makers | Scored benchmark against public data, PARTNER/VALIDATED rows, datasheet import |
| Hardware partner (PARTNER rows) | Bus, RF payload, quantum payload, flight PA houses | A documented interface so their data drives Kshana studies directly |
| Test event / agency programme / standards | Jammertest-class events, standards working groups | Pre-registered scoring of a test campaign, reproducible evidence packs |
| Tooling and AI ecosystem | Dashboards, notebooks, AI assistants | APIs, SDKs, MCP, Grafana, notebooks |

## 2. Workstream A: plug-and-play integration layer ("Kshana Connect")

**Goal:** a company connects its data and tools to Kshana with a configuration file and the documentation, no code.
**Definition of done:** a new integration (for example a monitoring network's station logs into the referee and planner)
is completed by an outside engineer from the docs alone, passes the conformance kit, and needs no change in Kshana.

| # | Item | Exists today | Missing (to build) | Size |
| --- | --- | --- | --- | --- |
| A1 | **Canonical observation schema**, versioned JSON Schema for everything a station or system produces: PVT, C/N0 and AGC per signal, clock offset, spectrum summaries, alarms, IQ file references, OSNMA status, metadata (site, antenna, receiver) | Format readers write internal structs; `kshana.partner-submission/1` for partner data | A public, versioned `kshana.obs/1` schema and its validator (`kshana connect validate`); schema docs; change policy (semver, deprecation) | M |
| A2 | **Input adapters** | RINEX obs/nav, u-blox UBX, Septentrio SBF, NovAtel, RTCM, NMEA, SigMF IQ, ADS-B/AIS reports, ANTEX, CCSDS OEM/TDM, SP3, SPICE kernels, clock-log formats (8) | ASTERIX CAT021/CAT023 (aviation surveillance feeds); linuxptp/ptp4l, chrony/ntpd and White Rabbit logs; SNMP/syslog alarm feeds; Trimble/Leica receiver logs; generic CSV/JSON/Parquet **no-code mapping file** (`connect.toml`: column → schema field, units, time base) so any vendor format maps without code | L |
| A3 | **Live ingest** | MQTT and WebSocket in places (marine plugins); file readers | A streaming ingest service: MQTT, Kafka, NATS, WebSocket and file-drop watch, all into `kshana.obs/1`; back-pressure, replay from a stored stream, time alignment | L |
| A4 | **Outputs and alerts** | JSON/CSV/HTML reports, Prometheus metrics, OpenTelemetry (OTLP), Grafana dashboard, CZML/KML/GeoJSON/STK/SigMF exports | Webhooks with signed payloads; alert formats: CAP (Common Alerting Protocol), syslog/CEF for SOC tools, SNMP traps; a stable result schema (`kshana.result/1`) documented alongside A1 | M |
| A5 | **APIs and SDKs** | Rust crate, Python (PyPI), npm/WebAssembly, CLI, MCP server (49 tools); Pro `serve` HTTP job service | OpenAPI 3.1 description of the job service and generated clients; gRPC option for high-rate streams; C ABI (`cdylib` + header) so C/C++/Go/Java/MATLAB and LabVIEW can call the engine; versioning and deprecation policy across all surfaces | L |
| A6 | **Deployment kits** | Docker images (reference build, MCP), Helm references, Signal K, OpenCPN, ROS 2, GNSS-SDR, JetBrains | One supported Helm chart and Docker Compose for "engine + ingest + job service + Grafana"; an **air-gapped / on-prem bundle** (signed images, SBOM, offline licence); hardware sizing guide; multi-arch images | M |
| A7 | **Conformance kit ("Works with Kshana")** | `kshana partner check` (format only) | Test vectors and a replay corpus per adapter; `kshana connect check` that proves an integration end to end; a public list of verified integrations; a badge with clear meaning (format conformance, not a verdict on the partner's system) | M |
| A8 | **Integration guides** | Surface docs, MCP install docs, deploy docs | One guide per customer shape (table in §1), each with a worked example on public data (e.g. a monitoring network replayed from Jammertest logs; a flight-dynamics cross-check from a public trajectory) | M |
| A9 | **Security and trust of the integration** | Signed evidence packs, SBOM, attestations | AuthN/AuthZ for the service (API keys, OIDC), per-tenant isolation in Pro, audit log, data-retention settings, PII-free ingest of crowd data by default | M |

**Dependencies:** A1 first; A2/A3/A4 build on it; A7 needs A1–A4; A9 is needed before any hosted or on-prem customer.

## 3. Workstream B: interference-monitoring networks (SJAWS-adjacent)

Detail in `0.41-BACKLOG.md` §1. Summary, in the agreed order:

| # | Item | Size | Depends on |
| --- | --- | --- | --- |
| B1 | Monitoring-network planner (site, candidate stations, threat set → detection and location-accuracy maps, station count and placement; Pro optimiser for the Pareto front) | L | — |
| B2 | Location metrics in the referee (location error, confidence-region coverage, time to locate) | M | — |
| B3 | Fixed-station emitter locator (C/N0 + AGC power-difference fit, TDOA with IQ snapshots), pre-registered on Jammertest 2026 | L | A1, A2 |
| B4 | Jam / spoof / meaconing / natural benchmark (TEXBAT, OAKBAT, Jammertest, event days) | L | — |
| B5 | Area trust state from several reference stations, with alert latency | M | A1, A3 |
| B6 | Port and airport scenario pack (terminal timing holdover, cranes, pilotage, AIS; airport approach and surface ops) | M | — |
| B7 | Synthetic data with known truth: labelled IQ and receiver-log packs for classifier training and regression | M | — |
| B8 | Environmental baseline service: expected natural degradation for a site and time | S | — |
| B9 | Two-way data: network alerts into the maritime trust score; ship trust scores as extra observations | M | A3, A4 |

**Plug-and-play outcome:** a monitoring network streams its station logs (A3) in `kshana.obs/1` (A1), receives referee
scores, area trust state and consequence alerts (A4) in its own dashboard, and plans new sites with B1, with no code.

## 4. Workstream C: navigation and flight-dynamics tools (KinetX / Intuitive Machines class, and others)

| # | Item | Exists | Missing | Size |
| --- | --- | --- | --- | --- |
| C1 | **Exchange formats** | CCSDS OEM (checked against GMAT, Orekit, STK), SP3, SPICE kernel reading, TDM reader, TRK-2-18 reader | CCSDS OPM/OMM/AEM/CDM read and write; SPICE kernel **writing** (SPK/CK) so results load in their tools; TDM writer; a round-trip test with each tool | M |
| C2 | **Lunar orientation check** against NASA SVS Moon phase and libration tables (from the Turion post) | DE440 lunar orientation | The comparison, pre-registered bars, a ledger row | S |
| C3 | **Close the cislunar misses** | Artemis I and CAPSTONE comparisons (NOT MET) | Manoeuvre-aware comparison (a second pre-registered run, draft C2 awaits the owner); higher-fidelity force model for NRHO | M |
| C4 | **Real tracking data** | TRK-2-18 reader, Mars ramped Doppler (MODELLED) | Joint comparison on real DSN passes (partner data), promotes rows to VALIDATED or PARTNER | M, gate: partner |
| C5 | **Optical navigation and small bodies** (gap named in the KinetX fit document) | B-plane targeting, light-time solver | Optical navigation measurement model and small-body gravity; or a joint study with a partner first | L |
| C6 | **Lander navigation with flown data** | Descent, surface, ascent kinds (MODELLED) | Comparison with real descent navigation data from a flown lander (partner) | M, gate: partner |
| C7 | **Tool adapters** | MCP, Python, Rust | Thin adapters/notebooks for GMAT, Orekit, Basilisk and TMPL-class libraries (export a scenario, import their trajectory, score the difference) | M |

## 5. Workstream D: close the open evidence gaps

Each stays NOT MET until a new pre-registered run meets its bar. No bar is loosened.

| Gap (0.40) | Path to close | Workstream | Size |
| --- | --- | --- | --- |
| Artemis I cislunar arc (45.3 km against 5 km) | Manoeuvre-aware second run (C3) | C3 | M |
| CAPSTONE NRHO (136.8 km against 25 km) | Higher-fidelity force model and manoeuvre history (C3) | C3 | M |
| Clock-log fit F1(a), F2 / D2 | Better noise identification; measured holdover compared with one predicted from its own earlier data (public roadmap "next") | D | M |
| Emitter region NOT ASSESSABLE (Jammertest 2025) | Fixed stations (B3) and Jammertest 2026 data | B3 | — |
| kshana-bench A2, D1, R1 | Stronger baselines; re-run R1; second out-of-distribution track for task (d) | D | M |
| Space-user service volume S1–S5, S7 (GLONASS reference angles) | Resolve the booklet's two GLONASS angles; held-out reception campaign | D | M |
| Lunar geodesy (3 bars) | Measured lunar quantities or a partner dataset | D | M, gate |
| LunaNet AFS (one printed tertiary column) | Wait for the specification's next issue; recheck | D | S |
| W57 leftovers | Almanac-HDOP scorer; pin the TEXBAT hash; Studio view and SVG for C3 | D | S |

## 6. Workstream E: partners and partner areas

**Principle:** partner areas plug in through published interfaces, so a partner's data drives Kshana studies with no
special development.

| # | Partner area | Exists | Interface to publish (no-code for the partner) | Size |
| --- | --- | --- | --- | --- |
| E1 | Data partners (9 MODELLED rows that partner data can promote; holdover floors first) | Partner kit, `kshana.partner-submission/1`, `kshana partner check`, PARTNER-DATA.md | Data agreement template; submission portal or signed upload; pre-registration form; scoring report template | M |
| E2 | Navigation RF payload and antenna hardware (PARTNER row) | ANTEX reader, CRPA model, link budgets | Antenna pattern import (ANTEX and measured-pattern CSV), CRPA array description file, payload performance tables (EIRP, G/T, filter masks) mapped into the link and interference models | M |
| E3 | Quantum payload hardware (PARTNER row) | Quantum sensor and clock performance models, clock twin | Device performance sheet format (bias stability, Allan profile, bandwidth, environmental sensitivities) feeding the clock twin and quantum-vs-classical trade | S |
| E4 | Spacecraft bus engineering (PARTNER row) | Mission thread, attitude-independent nav | Interface for bus constraints that affect navigation (pointing, power/duty cycles for receivers, thermal effect on clocks) as scenario inputs | S |
| E5 | Flight-hardware product assurance (PARTNER row) | Pro traceability (CSV, ReqIF, SysML v2), dossier, evidence packs | A PA evidence link: their PA records referenced by requirement id in the Pro dossier, no copying of their data | S |
| E6 | Clock and receiver vendors | Clock twin, receiver-trust, readers | Datasheet import; vendor benchmark track in kshana-bench; "Works with Kshana" listing (A7) | M |
| E7 | Partner programme operations | Outreach drafts (not sent), timing-partner shortlist | Partner page on kshana.dev; tiers (data, technology, channel); independence policy published; contact path. **Gate: owner sends outreach** | S |

## 7. Workstream F: Kshana Pro

| # | Item | Status | Size / gate |
| --- | --- | --- | --- |
| F1 | Release readiness: re-pin to the published engine, `--locked`, signed licence token exercised end to end | Not done | S |
| F2 | Connectors in Pro: the A-layer service with auth, tenancy, audit log, SLA metrics | Missing | L (after A) |
| F3 | Hosted / on-prem service ("Resilience Platform"): multi-tenant job service, saved studies, team sharing | Missing | L |
| F4 | Pack B (timing resilience study) | Blocked on export-control determination | Gate: counsel |
| F5 | Pack C (subscription, CI guide) | Partial | M |
| F6 | Pricing, trademark, EULA/MSA/DPA/SOW counsel review, payment go-live | Unset | Gate: owner |
| F7 | Network-planner Pareto (B1) and referee reports (B2) as Pro features | Missing | with B |

## 8. Workstream G: missing domain features (from the domination research)

| # | Item | Size |
| --- | --- | --- |
| G1 | Network timing resilience: PTP / White Rabbit distribution, holdover across a timing network, failover | L |
| G2 | Time-transfer authentication: NTS and PTP authentication TLV, spoofed-time detection | M |
| G3 | Loss and ROI model: operational impact and cost of an outage, for business cases | M |
| G4 | Guided self-assessment questionnaire mapped to resilience frameworks (builds on the compliance report) | M |
| G5 | Fleet and route risk dashboards (maritime and aviation routes against interference maps) | M |
| G6 | Gap-remediation planner: from a study's weakest link to ranked mitigations | M |
| G7 | Underwater acoustic PNT | Parked |

## 9. Workstream H: quality, platform and i18n

| # | Item | Size |
| --- | --- | --- |
| H1 | SonarCloud new-code coverage to ≥ 80% (about 810 lines; CLI paths first, by in-process tests) | M |
| H2 | CI speed: shard the test suite across jobs; keep every test; keep the release's verify within limits | S |
| H3 | Studio strings still in English on translated pages; native-speaker review programme; one-pass rendering (non-English runs cost 2×) | M |
| H4 | CHANGELOG correction (RUSTSEC-2026-0387 and MSRV 1.88 under 0.40.0); README MCP table cells cut off | S |
| H5 | Translated READMEs regenerated at each release (automated job, machine-translation note kept) | S |
| H6 | Release robustness follow-ups already landed (opt-level 1, verify disk, winget, image timeouts); add a release dry-run before every tag | S |
| H7 | Site: keep the 0.40 browser audit (every page × 18 languages × widths) as a CI job | M |

## 10. Workstream I: research, standards and governance (owner-gated)

- JOSS tools paper (needs author decisions and an external co-author).
- AI/ML impairment-evaluation testbed paper, target IEEE/ION PLANS 2027.
- ION GNSS+ and ENC submissions (benchmark, Jammertest scoring).
- Governance: TSC charter, oracle registry, licence split; standards participation.

## 11. Workstream J: market and launch

- Product Hunt launch (2 November): first comment, replies, Show HN, r/GNSS, r/rust.
- Listings: MCP registry publish (owner variable), winget moderation.
- "Request a study" call to action live on kshana.dev; partner page (E7).
- Pack A outreach to the first paying design partner; Jammertest scoring presence; the browser course for universities.

## 12. Owner gates (decisions only the owner can make)

1. Go per workstream.
2. Partner outreach (send), data agreements, any contact with a named company or test-event organiser.
3. Pricing, trademark, counsel reviews, payment go-live, export-control determination.
4. Draft C2 (Artemis I manoeuvre-aware run) approval.
5. Research authorship and governance.

## 13. Suggested sequencing

| Release | Contents | Why |
| --- | --- | --- |
| **0.41** | A1, A2 (mapping file + ASTERIX + PTP/NTP logs), A4 (webhooks, result schema), A7 (first conformance kit); B1, B2; C2; H1, H2, H4 | The plug-in base plus the first two monitoring-network capabilities; cheap, reuse-heavy items |
| **0.42** | A3, A5 (OpenAPI, C ABI), A6, A8, A9; B3, B5, B7, B8; C1, C3; E2–E5 interfaces; G2 | Live operation, deployability, partner interfaces, closing the cislunar misses |
| **0.43** | B4, B6, B9; C4–C7; D remaining; F2, F3; G1, G3–G6 | Benchmarks, partner-data validations, the hosted platform and the remaining domain features |

Each release keeps the usual rules: every count read from the engine, every figure labelled, misses published, no bar
loosened, the public repository neutral.
