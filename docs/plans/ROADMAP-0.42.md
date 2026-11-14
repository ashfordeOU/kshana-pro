# Kshana 0.42: plan (draft for the owner's sign-off)

Status: **PROPOSED, 13 November 2026.** Nothing here is started except Phase 0 items the owner
already ordered. Every date is an estimate. Every evidence claim follows the pre-registration rule:
readings and oracle before code, scored once, misses published, bars never loosened.

Inputs: the 0.41 roadmap (`ROADMAP-0.41-0.43.md`) and its leftovers; three research passes run on
13 November 2026 (Kshana and Pro inventory; the commercial PNT market; defence, agencies and space);
the owner's instructions of 13 November 2026: plan 0.42 to cover every gap, fit every segment,
connect inside and beside every system, and **build the connector parts in the open edition and the
main things in Pro, so customers pay**. The page version of the research is the artifact
"Kshana 0.42 and Pro Plan" (claude.ai/artifact/Vd8UHHR5Sf1CbbzJUWx2S1, private).

---

## 0. The idea in one paragraph

Hardware vendors sell boxes; data vendors sell maps. Nobody sells **neutral, verifiable evidence**
that ties a real interference event to what a receiver or clock did and what it did to operations.
Insurers, regulators, ANSPs, monitoring programmes, receiver makers and timing owners all need that
evidence. 0.42 makes Kshana the layer that **plugs into every system** (open, free, so it spreads)
and turns what flows through it into **decisions and evidence people pay for** (Pro and Enterprise).

Kshana stays civil and commercial-first; defence is a dual-use route. Kshana is pre-revenue, with no
customers; nothing in this plan says otherwise.

---

## 1. The edition rule for 0.42 (amends the F10a matrix of 7 November 2026)

**Owner, 13 November 2026:** "build pro connector parts in free and main things in pro as we said
we wanna make them pay".

| Edition | What goes there | Why |
|---|---|---|
| **Open (AGPL, free)** | Every **reader, writer, protocol client, format exporter, schema, SDK and conformance kit**; one Station; single-run analysis; verifying any signed evidence pack; safety-relevant detection | Adoption: Kshana must connect to everything for free. A connector alone gives a customer data, not a decision |
| **Pro (licence token)** | The **products that turn data into decisions and evidence**: forensics, scorecards, threat library content, fleet timing assurance, multi-source trust, fusion across sites, studies, signed dossiers, alert routing policies, more than one Station | This is what buyers pay for (research, §3) |
| **Enterprise** | Running it at scale inside an organisation: HA service, SSO (OIDC, SAML), SCIM, audit log, multi-tenant, air-gapped support, retention policies, SLA | Large buyers pay for operations, not features |

### 1a. Fourth tier: Arksa-exclusive (owner, 14 November 2026)

Owner: Kshana Pro parts that make Arksa powerful must not be public or sold to anyone; they are scoped so they strengthen Arksa, not anyone who uses Kshana. Decisions: every item in the Kshana-in-Arksa report gets its own build row (register: claude.ai/artifact/RTiCstDuuNopBKgh7b9FgN §13, 273 rows); space-mission jobs are Arksa-exclusive; the lunar packs are Arksa-exclusive; the referee (E4) engine is Arksa-exclusive while its signed reports may be sold.

| Tier | Rule | Examples |
|---|---|---|
| **Open** | As above | formats, connectors, reference models, lunar time library, threat-scenario library |
| **Pro (sold to anyone)** | Products for markets that do not compete with Arksa | maritime/aviation forensics (E1), insurer evaluations, telecom/finance timing (T1–T3 ground), receiver scorecards (E3), trade study, uncertainty, batch, scenario CI, twin |
| **Arksa-exclusive Pro** | Any job that designs, proves, approves or operates a spacecraft. Private code; unlocked only by an `arksa` entitlement claim that only Arksa's issuer signs; never runs under a standalone Pro token; never shipped in Desktop Pro on a customer device (runs server-side or in contracted Arksa Sovereign installs) | F4 spaceborne interference exposure; E4 referee engine (space part); T1 for spacecraft clocks; PNT and time passport; covariance quality; threat-aware planning; anomaly attribution; lunar packs (time budget, beacon placement, observability, signal security, lunar OED); pro-mbse, pro-design-optimise, pro-spectrum-coexistence, pro-optical-rf-hybrid, pro-realtime-frame-eop; mission dossier, ECSS VCD, GSN, SysML/ReqIF, NAV-DT twin and NAVEX codec, MRD; M-workstream space RF survey |
| **Core** | Never shipped; Arksa servers only; patent before disclosure | P1–P5; constellation-as-sensor (B6) |
| **Enterprise** | As above (operations) | SSO, SCIM, audit, HA |

Build items this adds: token claim `arksa` and refusal test (a standalone Pro token cannot run an Arksa-exclusive kind); signed tier manifest (D-K10) listing kinds per tier; Desktop Pro build excludes Arksa-exclusive and Core code (binary symbol check in CI); Arksa MCP catalogue exposes Arksa-exclusive kinds, the public Kshana MCP does not. Anything already published stays public.

### 1b. Protection of Arksa-KPro and Core (owner, 14 November 2026)

Owner: manage Arksa-KPro, Pro and Public well; hide the methods in Pro and KPro; no way for anyone to extract them; Kshana Pro may say some high-end things are in Arksa without details. Rule follows where code runs: only code that never ships stays secret.

| ID | Item | Done when |
|---|---|---|
| X1 | `arksa` entitlement claim in licence tokens; only Arksa's issuer key signs it; every Arksa-exclusive kind refuses without it | Test: a standalone Pro token is refused for each Arksa-exclusive kind |
| X2 | Signed tier manifest (D-K10) naming every kind's tier | Manifest verified at start; unknown kinds refused |
| X3 | Arksa-exclusive and Core code moved out of shipped crates into a server-only module (proposed separate private repo `arksa-kpro`, owner to approve) | Desktop Pro and public packages build without it |
| X4 | CI symbol check on every Desktop Pro and Pro release binary: no Arksa-exclusive or Core symbols, strings or files; stripped release builds | CI red if any appear |
| X5 | Leak gate run from the private repo against the open repo on every open-repo push: no Pro, KPro or Core module names, method terms or files | Gate in Pro CI; term list kept private |
| X6 | Results-only service API for Arksa-exclusive kinds: verdict, evidence class, signature; intermediates withheld; per-tenant query budgets; probing alerts; watermarked outputs; access log | Tests for each |
| X7 | Trade-secret register and patent-before-disclosure rule (P1, P2, P5, B6 filings; P4 secret) | Register exists; no disclosure before filing date |
| X8 | Marketing rule: outcome names only for "In Arksa" capabilities; no method, equation, parameter or diagram before filing | Copy check in CI for site and docs |

### 1c. Commercial plan: editions, prices, licences, protection (owner, 14 November 2026: "plan it all for me")

All prices are **proposals for the owner to approve (gate K4)**; nothing is public until then. USD to match Arksa's price list (FACTS.md); excl. VAT.

**Payment models**
| What | Model | Proposed price |
|---|---|---|
| Kshana Pro, individual | Subscription, monthly or yearly | $290/month or $2,900/year |
| Kshana Pro, team (min 3 seats) | Yearly per seat | $4,900/seat/year |
| Kshana Enterprise (SSO, HA, audit, 25 seats) | Yearly | from $45,000/year |
| Kshana Pro, academic | Yearly per seat; teaching free | $490/seat/year |
| Evidence pack: incident forensics | One-time per incident | $4,900 standard; $14,900 claim-grade with expert review |
| Evidence pack: receiver scorecard | One-time per receiver per campaign | $12,000 |
| Evidence pack: timing audit (RTS 25, G.8272) | Yearly per site | $5,900/site/year |
| Evidence pack: insurer qualifying-system evaluation | One-time per system | $24,000 |
| Evidence pack: framework mapping (EASA, NIST, NIS 2) | One-time | $3,900 |
| Qualification support package (per standard) | One-time + yearly updates | $9,900 + $2,500/year |
| Kshana Sovereign (on-site, air-gapped) | Yearly site licence; perpetual only if a government requires it (≈3× yearly + 20%/year maintenance) | from $90,000/year; "Contact us" in public |
| Arksa | Existing list (FACTS.md): Pro $249/mo; Team $599/seat/mo ($499 yearly); Mission $60k+/yr; Operations $500/satellite/mo; Academic labs $1.5k/yr; Sovereign "Contact us" (internal $150k+/yr, air-gapped from $250k) | unchanged |

No one-time "buy forever" for Kshana Pro: without renewal there is no way to stop a cracked or leaked copy.

**Arksa-KPro inside Arksa's plans** (never sold standalone)
| Arksa plan | Arksa-KPro jobs included |
|---|---|
| Pro | Design studies: MBSE, design optimiser, spectrum coexistence, optical/RF hybrid, frame/EOP, lunar studies |
| Team | + mission dossier, ECSS evidence (VCD, GSN), MRD, NAV-DT twin |
| Mission | + PNT and time passport, covariance quality (Space Act), referee engine, space-receiver threat scenarios |
| Operations ($/satellite) | + spaceborne interference exposure per pass, time-to-breach for onboard and ground clocks, anomaly attribution, threat-aware planning |
| Sovereign | All of the above on the customer's own servers under contract (Core verdicts included; escrow; no inspection rights) |
| Academic | Design studies only, watermarked "academic" |

**Licence mechanics**
- C1 Subscription tokens valid 35 days, renewed automatically online; 30-day offline grace; seat bound to the machine ID; revocation list checked at renewal.
- C2 Yearly prepaid offline tokens (term + 30 days) for air-gapped Pro and Sovereign; renewal by signed file.
- C3 Pro split by secrecy: methods that are patented or acceptable to be studied run on the device; trade-secret Pro methods run on a hosted Kshana Pro service (signed results only, inputs in, verdict out), with a monthly compute allowance per subscription.
- C4 Every signed result carries an invisible licence-holder mark (watermark) so leaked reports can be traced.
- C5 Terms: no reverse engineering, no benchmarking publication without consent, export and sanctions clauses, liability cap 12 months' fees, "not for safety-of-life use".

**Payments and invoices**
- C6 Self-serve (Pro individual, team, academic, evidence packs under $5k): merchant of record handles VAT worldwide (Paddle recommended; gate K3; Stripe stays deferred by the owner).
- C7 Contracts (Enterprise, Sovereign, Arksa Mission/Operations, large evidence packs, QSP): one-page order form, invoice, bank transfer, net 14/30.
- C8 Licence issuing tied to payment webhooks (S3), refunds revoke (S3 tests).

**Distribution**
- Kshana Desktop (free): direct signed download for macOS (done), Windows (K5), Linux; Mac App Store and TestFlight optional later (owner: "App Store we will handle in a bit").
- Kshana Desktop Pro: direct signed download only (store rules would force in-app purchase).
- Arksa: Arksa cloud, plus Sovereign installs; Arksa-KPro never on a device.

**Build items and order**
| ID | Item | When |
|---|---|---|
| C1–C2 | Token lifetime, renewal, grace, revocation, seat binding | 0.42 phase 0 (with S1) |
| C3 | Hosted Kshana Pro service for secret methods (reuses S6 hosted trial) | 0.42 phase 1 |
| C4 | Result watermark | 0.42 phase 1 |
| C5 | Terms and order form (counsel review) | before first paid sale |
| C6–C8 | Checkout and issuing (S3–S5) | when K3 and K4 clear |
| C9 | Private price sheet for sales, then public price page (Sovereign "Contact us") | after K4 |

**Amendment to F10a:** "industry adapters beyond the top five" moves from Pro to **Open**. Every
other F10a line stands. **Owner gate K1** confirms this amendment.

**Line kept from the blueprint:** safety-relevant detection, file formats and published evidence are
never paywalled. A free user can always detect, read, write and verify; a paying user gets the
analysis, the evidence products, the fleet and the operations.

**How the line is drawn in code:** connectors live in the open crate (`kshana::connect_*`, readers
and writers under `src/`, SDKs under `sdk/`); Pro consumes them through the existing seam and adds
products under `kshana-pro/src/`. A CI check in Pro fails if a Pro module re-implements a reader or
writer that exists in the open crate (one implementation of every format).

---

## 2. Fit by segment: what each buyer gets free, and what they pay for

| Segment | Free (Open) gets them in | They pay for (Pro / Enterprise) | Main 0.42 items |
|---|---|---|---|
| Maritime: owners, P&I clubs, adjusters, flag and port states | AIS and NMEA 0183/2000 readers, Signal K, OpenCPN, IEC 61162-450, single-vessel interference check | **Incident forensics pack** (claims-grade, signed), **AIS and position integrity scoring** across fleets and areas, retainers | I1, I6, E1, E5 |
| Aviation: airlines, ANSPs, Eurocontrol | ASTERIX CAT021 (have) plus CAT010/048/062, ADS-B NIC/NACp feeds, OpenSky reader, GeoJSON/OGC layers for EFBs | **Forensics** for flights, **validated common picture** for an FIR, spoofing **training scenarios**, risk layers via API | I1, I5, E1, E2, F1 |
| Telecom, grid, finance, data centres | linuxptp, chrony, PPS, TICC, ntpd, White Rabbit, CGGTTS (have); PTP management, SNMP from grandmasters, C37.118 (have), IEC 61850 sampled-value timing, OPC UA, Prometheus | **Timing assurance**: holdover and time-to-breach for every clock, fleet view, audit export for MiFID II RTS 25 and NIST IR 8323 | I2, I7, T1–T4 |
| Receiver and chipset makers, anti-jam startups | UBX, SBF, NovAtel, Javad, Unicore, SBP, RTCM MSM, RINEX 2–4, Android raw (have), SigMF, GNSS-SDR | **Resilience test campaign and referee scorecard** mapped to IEEE P1952 and the DHS levels; **threat scenario library** | I3, I4, E2, E3 |
| Simulator makers and test ranges | Scenario export to Skydel, SimGEN/PosApp, R&S, LabSat, gps-sdr-sim | **Threat library licence** (OEM), **Jammertest scoring** service | I4, E2, G4 |
| Monitoring authorities (EU, UK, ANSPs, coast guards, MoDs) | Station (one), Connect, MQTT/NATS/Kafka, STIX/TAXII export, CAP alerts (have) | **Fusion across sites**, area trust console, geolocation (once a locator meets its gate), evidence for joint investigation, HA and multi-tenant (Enterprise) | I5, I7, F1, F2, V1 |
| Drones, robotics, automotive | MAVLink (PX4, ArduPilot), ROS 2 (have), U-space/UTM GeoJSON layers, OSNMA verification (have) | **Pre-flight interference risk** with evidence, fleet health | I6, F1 |
| LEO PNT operators (Xona, TrustPoint, Iridium) | Signal presets as published, open accuracy and coverage kinds | **Independent user-side verification** studies; **multi-source trust** | E4, F3 |
| Space operators and lunar teams | CCSDS OEM/OPM/OMM/TDM/ADM, SPK, STK, SP3, GMAT/Orekit/Basilisk/ANISE (have); Yamcs, OpenMCT, COSMOS telemetry bridges | **Mission dossier**, lunar studies, **spaceborne receiver interference** assurance (Arksa Ops pillar) | I8, E4, F4 |
| Insurers and auditors | Free verifier for any signed pack | **Scorecards**, forensics, framework evidence packs | E1, E3, E6 |
| Agencies and research groups | Everything open, Python, notebooks | Studies, trade studies, signed dossiers; grant consortia | G2, G3 |

---

## 3. Workstreams

Effort: S (days), M (one to three weeks), L (more). Every item has an acceptance test; nothing is
"done" until its test is green in CI on Linux, macOS and Windows unless stated.

### S. Sell what is built (Phase 0, before 0.42)

| ID | Item | Edition | Effort | Done when |
|---|---|---|---|---|
| S1 | Pro release on engine 0.41 (Pro 0.3.0): tag, release smoke test (finishes F1b), changelog | Pro | S | Tag pushed; smoke test verifies a signed token on the released binary |
| S2 | Signed and notarised Desktop Pro (self-hosted Mac runner), same checks as the open Desktop release | Pro | S | `codesign`, `spctl`, `stapler` pass; `.dmg` attached to a private Pro release. **Gate K2** (Apple secrets in kshana-pro) |
| S3 | Checkout end to end in the provider's test mode: checkout, verified webhook, token issued, app unlocks, refund revokes; forged, replayed, out-of-order webhooks refused | Pro | M | CI test in test mode green. **Gate K3** (provider and test account) |
| S4 | Price list and editions page; Sovereign stays "Contact us" in public; prices from gate K4 | Pro | S | Page live; switch still off until K4 |
| S5 | Payments live | Pro | S | **Gate K4**: provider, prices, bank account, VAT advice |
| S6 | Hosted trial (F3b) deployed: 14-day trial tokens only, rate-limited, no customer data kept | Pro | M | Trial token issued and expires; abuse limits tested |
| S7 | Licence signing key custody checked against docs/LICENSING-KEYS.md (offline, backup, rotation drill) | Pro | S | Owner confirms custody; rotation drill run on a test key |

### I. Interconnect: inside and beside every system (all Open)

Rule: one implementation per format or protocol, in the open crate, each with conformance vectors
and a round-trip test. Readers never configure hardware unless the user opts in (logged).

| ID | Area | Items | Effort | Done when |
|---|---|---|---|---|
| I1 | **Maritime and aviation data** | NMEA 0183 AIVDM/AIVDO and NMEA 2000 (via canboat JSON); IEC 61162-450 UDP; ASTERIX CAT010, CAT048, CAT062 (CAT021 exists); ADS-B NIC/NACp from OpenSky and Beast/SBS; ICAO NOTAM interference notices (text to structured) | M | Each reader passes published or recorded vectors; one Signal K and one OpenSky end-to-end demo |
| I2 | **Timing systems** | PTP management (IEEE 1588 TLVs, linuxptp pmc); SNMP polling and traps for grandmasters (generic MIB plus Meinberg, Microchip TimeProvider, Oscilloquartz where MIBs are public); IEC 61850-9-2 sampled-value timing quality; NTP (ntpq/ntpd) (have chrony); ITU-T G.8275.x profile reports | M | Readers tested on captured traffic; MTIE and holdover runs from each source |
| I3 | **Receivers** | u-blox UBX (NAV-PVT, NAV-SAT, MON-RF, NAV-TIMEGPS, SEC-SIG), Septentrio SBF (via existing sbf-tools), NovAtel OEM binary logs, Javad GREIS, Unicore, Swift SBP, RTCM 3 MSM observations, RINEX 2.11 (3 and 4 exist) | M | Each with recorded logs from public datasets; trust verdicts identical across formats for the same epoch set |
| I4 | **SDR, IQ and simulators** | SigMF read and write; SoapySDR capture (user-installed drivers); scenario export to Safran Skydel, Spirent SimGEN/PosApp, R&S, LabSat and gps-sdr-sim formats where the formats are public or documented to licensees | M–L | Export round trip where a reader exists; a published scenario reproduced in one simulator by a partner (G4) |
| I5 | **Ops, IT and security systems** | Prometheus/OpenMetrics exporter; OpenTelemetry; syslog (RFC 5424) and CEF; Kafka in and out (A2 remainder); SNMP traps out; Splunk HEC, Elastic, generic webhooks (have) with templates for ServiceNow, PagerDuty, Opsgenie, Slack, Teams; STIX 2.1 objects and a TAXII 2.1 client for sharing interference indicators; CAP (have) | M | Each output validated against its schema or a reference collector; one Grafana dashboard per segment |
| I6 | **Drones, robotics, GIS** | MAVLink (GPS_RAW_INT, GPS_STATUS, GPS2) for PX4 and ArduPilot logs and live; ROS 2 (have); GeoJSON, KML, CZML (Cesium), OGC API Features and Tiles; U-space/UTM interference layer as GeoJSON | M | Layers open in QGIS and Cesium; MAVLink tested on public flight logs |
| I7 | **Industrial and grid control** | OPC UA client and server (read-only by default); Modbus TCP read; DNP3 read for timing alarms where public specs allow | M | Tested against open-source servers (open62541, pymodbus) |
| I8 | **Space operations** | CCSDS ADM and OMM where missing; SP3 write; CCSDS Space Packet and XTCE for telemetry; Yamcs, OpenMCT and OpenC3 COSMOS bridges | M | Round trip via Orekit or Yamcs in CI |
| I9 | **SDKs and languages** (A5b remainder) | Java (JNI or REST), .NET/C#, MATLAB (MEX over the C ABI), LabVIEW (via the C ABI), Julia; C, C++, Python, Go, TypeScript exist | M | Each SDK runs the same three conformance scenarios with identical bytes |
| I10 | **Deploy anywhere** | Helm chart and Kubernetes operator; Terraform module; Docker (have); Raspberry Pi and ARM edge image for Station; cloud marketplace listings (AWS, Azure) after Pro launch | M | Station installs and reports on a Pi and on Kubernetes in CI |
| I11 | **Agents and APIs** | MCP (53 tools, have) gains the new readers; OpenAPI 3.1 (Pro service) published for the open endpoints too | S | MCP tool list and OpenAPI validated in CI |
| I12 | **Connect conformance kit v2** | Every connector in I1–I11 ships vectors in the kit; `kshana connect check-live` covers them | S | Kit lists every connector; a third party can run it offline |

### E. Evidence products (Pro): what people pay for

Each product has a pre-registered quality bar before it is claimed in public, and a public sample
built only from public data.

| ID | Product | Buyer | Effort | Done when |
|---|---|---|---|---|
| E1 | **Incident forensics pack**: for a vessel or flight and a time window, whether there was jamming or spoofing, where, how strong, how the receiver and AIS or ADS-B track behaved, with a signed, re-runnable report and chain of custody | P&I clubs, insurers, adjusters, flag states, owners, airlines, ANSPs | M | Public sample on one public event; pre-registered detection bars met on a labelled set (V2); report verifies offline with the free verifier |
| E2 | **Threat scenario library** (F10h): curated real events (Baltic, Hormuz, Red Sea, Jammertest, TEXBAT) as reproducible scenarios, monthly updates, exports via I4 | Receiver makers, simulator users, crew training, test ranges | M | First 12 scenarios with provenance and licence notes; subscription feed signed |
| E3 | **Referee scorecards and receiver resilience test report** (F10a scorecards): campaigns mapped to IEEE P1952 and the DHS resilience levels; described as test evidence only | Receiver and chipset makers, integrators, buyers comparing vendors | M | Scorecard format frozen; one campaign run on public data and published as a sample |
| E4 | **Independent verification studies** for new PNT services (LEO, OSNMA, lunar): user-side coverage, accuracy and integrity with pre-registered bars | Xona, TrustPoint, Iridium users, Moonlight and lunar teams, agencies | S per study | Study template; one public sample on published signal data |
| E5 | **AIS and position integrity scoring** at fleet and area scale (builds on M4) | Ports, coastal states, charterers, sanctions teams | M | Scoring API; M4 position-offset bar re-run as a new pre-registration (V3) |
| E6 | **Framework evidence packs** (F10i): mapped to EASA SIB 2022-02R4, NIST IR 8323, DHS RPCF, IMO CL 5037, MiFID II RTS 25, EU Space Act and NIS 2 duties. Wording: "mapped to" only | Operators preparing audits | S | Six existing mappings extended; template per framework |
| E7 | **Insurer scorecard**: a periodic, signed exposure summary for a fleet or route | Insurers, brokers | S (after E1, E5) | Template and one sample |

### F. Fusion and decisions (Pro and Enterprise)

| ID | Item | Edition | Effort | Done when |
|---|---|---|---|---|
| F1 | **Area console**: fuse Stations, receivers and third-party feeds (customer-supplied HawkEye 360, Spire, SkAI or national network data) into an area trust picture with event timelines | Pro (multi-tenant: Enterprise) | L | Two-site demo from public data; HA failover tested |
| F2 | **Interference geolocation** in the console, using a locator only after it meets its gate (V1); until then the console shows detections and areas, never positions | Pro | (V1) | Locator meets its pre-registered gate |
| F3 | **Multi-source trust**: choose which source to trust second by second (GNSS, OSNMA, LEO, eLoran, PTP, holdover) with evidence | Pro | L | Pre-registered switching bars (V4) on recorded data |
| F4 | **Spaceborne receiver interference assurance** (Arksa Ops pillar): exposure of a spacecraft's GNSS receiver along its orbit | Pro | M | Study on public interference maps and TLEs |
| F5 | **Alert policy engine**: who is told what, when, through which I5 output; quiet hours, escalation, de-duplication | Pro | S | Policies tested; free edition keeps raw alerts |
| F6 | **Signed planned items from F10a**: planner Pareto front with cost (B1b), space tip-and-cue study (M9), Station fleet (have), HA (have) | Pro | M | B1b and M9 pass their tests |
| F7 | **Synthetic attack IQ** (B7), licence-gated, within counsel's export approval (owner, 7 Nov) | Pro | M | Gate K6 confirmed; generator behind the licence; never in the open crate |
| F8 | **Enterprise operations**: SSO (OIDC, SAML), SCIM, audit log export, retention, tenant isolation tests | Enterprise | M | Each with tests; tenant isolation test green |

### T. Timing assurance (Pro; Kshana's best-validated family)

| ID | Item | Edition | Effort | Done when |
|---|---|---|---|---|
| T1 | Holdover and **time-to-breach** for every clock in a mixed-vendor fleet, from I2 sources | Pro | M | Prediction bars pre-registered (V5) and scored on recorded holdover runs |
| T2 | Fleet view in the Station console and Grafana; per-clock evidence history | Pro | S | Demo on recorded data |
| T3 | Audit export for MiFID II RTS 25 traceability and NIST IR 8323 profiles | Pro | S | Template; sample report |
| T4 | Timing scorecards for grandmasters and PTP networks under spoofing scenarios (with E3) | Pro | M | One public sample |

### U. Desktop apps look like apps (owner, 13 November 2026: "go with A plus C palette and D as Pro home")

Design canvas: claude.ai/artifact/XekCZay7m5Gcmy6pdM9MD9 (concepts A, C, D chosen; B kept as the reference for
the Pro fleet screen). Same Studio tokens (ground, surfaces, the six domain colours, Bricolage Grotesque, Geist,
Geist Mono); the change is structure: an app frame, not a scrolling web page.

| ID | Item | Edition | Effort | Done when |
|---|---|---|---|---|
| U1 | **App shell (concept A)**: sidebar (Home, Runs, Scenario library, Devices, Evidence packs, Files; Pro section), document tabs in the title bar, centre view switch (map, timeline, table, JSON) with a result strip, inspector (inputs, result, export, the reproduce command), status bar (engine, devices, cores, edition, offline) | Open (free Desktop) | L | Every Studio scenario opens in a tab and runs natively; the shell is used only inside the Desktop apps, the website stays as it is |
| U2 | **Command palette and run history (concept C)**: ⌘K for scenarios and actions (run, compare two runs, export an evidence pack, connect a receiver), Pro actions tagged; Runs table with compare | Open | M | Keyboard-only run, compare and export work; Pro actions open the upgrade screen in the free app |
| U3 | **Desktop Pro home and licence (concept D)**: "What do you need to prove today?" with the four jobs (forensics, timing assurance, scorecard, study), recent work, licence panel (edition, seats, Stations, renewal, checked offline); replaces the plain licence page | Pro | M | Pro home opens first in Desktop Pro; licence states (none, active, expiring, expired, revoked) all drawn and tested |
| U4 | **Desktop wording**: the apps never say "runs in your browser"; they say what runs natively and offline | Open | S | Copy check in CI for the Desktop frontend |
| U5 | **Free-to-Pro upgrade screen** in the free Desktop (what Pro adds, link to the editions page; no checkout until S5) | Open | S | Reached from every Pro-tagged item |
| U6 | **Pro fleet screen (concept B)** on the U1 shell, after U1 to U3 | Pro | M | Built on the Station fleet service |

**Status 14 Nov:** U1, U2 and U5 on kshana main (f52f4e10c); extension hook `window.KshanaShell` (66a537b60); U3 on kshana-pro main (7c008b33, hook path 7c2c7512); signed test build ui-88321a33b given to the owner. U4 open: the classic Studio tool pages inside the app still say "runs in your browser".

#### U, continued: the instrument (owner, 13–14 Nov: "we keep all"; look and real-world maps added)

Vision: claude.ai/artifact/K3KuWpfUwjX2isPouDjhbM. Look and maps: claude.ai/artifact/DZwzwqAzPrhrwLjHqY3j4N. Edition rule as in section 1: connectors, formats and SDK open; evidence and decision products Pro; operations Enterprise.

**In 0.42**

| ID | Item | Edition | Effort | Done when |
|---|---|---|---|---|
| U7 | **Time cursor and linked views**: one clock across map, sky plot, plots, inspector and log; range select narrows every view; bookmarks; J/K/L, space, M keys | Open | L | Scrubbing 24 h of 1 Hz data at 60 fps in a Playwright frame test; every view follows the cursor |
| U8 | **Runs kept on disk and project files** (`.kshana` folder: scenarios, data links, runs, notebooks, layouts); search, tags, pins, notes | Open | M | Close and reopen restores tabs, runs and selection; recipe replay gives bit-identical results |
| U9 | **Drop anything in**: RINEX, UBX, NMEA, SP3, clock logs, IQ, folders; format detective shows format, span, constellations and gaps before loading | Open | M | Every format the engine reads opens by drag and drop; unknown files get a clear message |
| U10 | **Code mirror console**: every GUI action writes its Python or CLI line; "copy the script that makes this" on every figure | Open | M | Pasting the console history into Python reproduces the session's runs |
| U11 | **Explain this number** for headline results: model, inputs with hashes, assumptions, sensitivity, evidence class and oracle | Open | L | Available on every result that has a capability row; links to the method page |
| U12 | **Markers and delta markers** on all plots, with readouts in the inspector | Open | S | Keyboard and mouse; markers show on the map at the same instant |
| U13 | **Figure export** in journal styles (IEEE, ION, AIAA) as SVG, PDF, PNG | Open | S | Exported figures match the on-screen data exactly |
| U14 | **Look**: colour roles (domain, evidence, constellation, trust scale), symbol set (12 symbols), Geist Mono numbers with units and uncertainty, Comfortable and Compact density | Open | M | Visual regression screenshots per view; colour-blind check passes |
| U15 | **2D map on the time cursor**: Night ops and Chart styles, Natural Earth offline base, scale bar and projection always shown | Open | L | Opens with no network; map, plots and sky plot agree at every instant |
| U16 | **Sky plot with trails**, masks and per-satellite health colours | Open | S | Matches engine geometry at sampled epochs |
| U17 | **Ship and aircraft connectors** (AIS and ADS-B files; reported vs estimated tracks) | Open | M | Public sample files load; licence of each sample checked |
| U18 | **Shell in 18 languages**, same i18n checks as the Studio | Open | M | i18n check covers apps/desktop/shell |
| U30 | **Interactive charts, 2D and 3D** (owner, 14 Nov: "not just visual but interactable, checking values, zoomable, 2D, 3D and everything possible"): hover values with units, pinned readouts, delta markers, wheel/box/axis zoom, pan, linked axes and brushing, legend toggles, CSV/SVG/PNG export; line, scatter/Stanford, bar, heatmap, sky, log–log ADEV/MTIE, spectrum/waterfall; 3D trajectory, scatter with ellipsoids, surface, orbit camera, picking; every chart follows the time cursor | Open | L | 1M-point line ≥ 30 fps, 100k at 60 fps; unit tests for decimation, zoom maths, hit-testing, exports; every result view uses it |
| U19 | **Signed evidence packs** and **incident replay v1** on the map (reported vs estimated, trust shading, forensics pack) | Pro | L | V2 bars met before E1 is claimed; pack verifies with the public verifier |

**Moved into 0.42 by the owner on 14 Nov** (were planned for 0.43)

| ID | Item | Edition |
|---|---|---|
| U20 | Dockable panes, saved layouts per job, pop-out windows, presentation and focus modes | Open |
| U21 | Sweep studio (two-axis sweeps, Monte Carlo on all cores, failure map, open any cell as a run) | Open |
| U22 | Run diff (recipe and results aligned on one time axis) | Open |
| U23 | Embedded Python console bound to the project; lab notebook with PDF, HTML and Jupyter export; cite button | Open |
| U24 | Globe with orbits, stations and coverage; imagery and terrain; offline region packs; follow mode; Bridge (red) mode | Open |
| U25 | Spectrum and waterfall for IQ; Allan deviation, TIE/MTIE and Stanford plots | Open |
| U26 | Live AIS and ADS-B connectors | Open |
| U27 | Station fleet (U6), receiver scorecards, review mode, report builder, area trust on the map | Pro |
| U28 | Team library, single sign-on, roles, audit log, always-on watch walls | Enterprise |
| U29 | Course mode, paper reproduction kits, assistant (planned) | Open |

Map building blocks (proposed): MapLibre GL (BSD-3), deck.gl (MIT), CesiumJS (Apache-2.0) with our own terrain and imagery sources, PMTiles offline packs; positions always from the engine. Data-source licences (AIS, ADS-B, imagery) are checked before anything ships.

### Q. Quality and platform (carried from 0.41)

| ID | Item | Effort | Done when |
|---|---|---|---|
| Q1 | Windows: signing route (Azure Artifact Signing first; OV certificate on a cloud signing service as fallback), MSI run (L5) | M | Signed MSI installs on windows-2022 in CI. **Gate K5** (Windows signing account, runner billing) |
| Q2 | Linux packages installed and tested on Ubuntu, Fedora, Raspberry Pi OS (L6) | S | Install tests green |
| Q3 | Coverage job under four hours without dropping any test (split into shards and merge the reports) | M | Two consecutive runs under 4 h; line floor unchanged |
| Q4 | Galileo and BeiDou health bits read in the broadcast-health rule | S | Tests on real navigation files |
| Q5 | Arksa moves to engine 0.41 (D-K5), with Alpha | M | Arksa CI green on the pinned 0.41 package |
| Q6 | Public roadmap (J4), neutral wording, no Pro method detail | S | `ROADMAP.md` updated |
| Q7 | Pro planning docs corrected: Apple account exists and signing works (open repo, 12 Nov) | S | MACOS-NOTARISATION.md and the 0.41 roadmap line updated |
| Q8 | Homebrew, Scoop and winget at 0.42 (noreply identity fixed in d1c954408) | S | Parity shows all three |
| Q9 | Release readiness: the i18n and pages checks run before Auto-tag (so a release cannot ship with a red site check, as happened at 0.41.0) | S | Auto-tag waits on the i18n workflow too |

### V. Evidence rows (each pre-registered and signed before any data)

| ID | Row | Note |
|---|---|---|
| V1 | Locator successor (B3c): matched correlation refinement and a σ that includes it; periodic-waveform handling; bounded solver step. Gate on synthetic data, then Jammertest 2027 real data as already pre-registered for B3a/B3b | Unlocks F2 |
| V2 | E1 forensics detection bars on a labelled set (public events plus synthetic) | Before E1 is claimed |
| V3 | M4 position-offset bar, new method | v3 miss stays published |
| V4 | Multi-source switching bars | Before F3 is claimed |
| V5 | Holdover time-to-breach prediction bars on recorded clocks | Before T1 is claimed |
| V6 | E5 v3 spoofing benchmark and B5 v4 area trust, only where a new method is written down first | Misses stay published |
| V7 | C2 and C3 Artemis rows: re-scope or close with a written reason | No silent drop |

### G. Go-to-market and funding (owner-led; drafts by the Kshana seat)

| ID | Item | Route | Timing |
|---|---|---|---|
| G1 | First paying design partner for E1 or E3 (Pack A, gate J5) | Direct | Dec–Jan |
| G2 | ESA BIC Estonia application (≈€60k) and FutureNAV call for ideas | ESA | From Nov 2026 |
| G3 | Consortium for Horizon Europe CL5-2027-06-D6-11 (PNT resilience and e-conspicuity, €5–7M, closes 7 Oct 2027) | EU | Partner search from Jan |
| G4 | One simulator OEM (licence E2 scenarios through I4 exports) and one LEO operator (E4) | Direct | Q1 2027 |
| G5 | Two Baltic pilots: an ANSP and a maritime authority (E1, E5, F1) | National tenders / pilots | Q1 2027 |
| G6 | One P&I club or average adjuster conversation (E1, E7) | Direct | Q1 2027 |
| G7 | EDF 2027 SME call and NATO DIANA: only after the legal checks in K7 | EU / NATO | 2027 |
| G8 | Eurocontrol and EU interference-monitoring subcontracts (watch tenders; offer F1 analytics) | Tender / subcontract | Rolling |

---

## 3a. Owner decision, 14 November 2026: 0.42 carries everything

Owner: "Release everything in 0.42. 0.43 or anything else gets built and delivered in 0.42."

- No 0.43 is planned. U20–U29 (listed above as 0.43) move into 0.42, with every other workstream in this plan (S, I, E, F, T, U, Q). This is the same rule as the 0.41 decision of 3 November.
- Phases 1 to 4 run in parallel ("all together"), each item on its own branch, merged to main when its acceptance test is green.
- The owner's go for the 0.42 release is given; the release ships when the release checklist and every acceptance test pass. Items blocked only by an owner gate (K3–K8) ship switched off or in test mode, and are named as such in the changelog.
- Evidence rows V1–V7 are still never on the critical path: a product claim waits for its bar; the code ships.
- Items that need outside parties (G1–G8, partner data, hardware) are not "built" by the team; their software side ships, and the outside step stays on the owner's list.

## 4. Phases and dates (estimates)

| Phase | Content | Target |
|---|---|---|
| 0. Paid launch (0.41.x, Pro 0.3.0) | S1–S7, Q7, Q9 | 18 Dec 2026 (S5 when K4 clears) |
| 1. Connect everything | I1–I12 (connectors are independent and run in parallel) | 22 Jan 2027 |
| 2. Evidence products | E1–E3 first, then E4–E7, with V2 before E1 is claimed | 5 Feb 2027 |
| 3. Timing and fusion | T1–T4, F1, F3–F8 | 12 Feb 2027 |
| 4. Platform and Desktop design | Q1–Q6, Q8; U1–U6 | 12 Feb 2027 |
| 5. Release 0.42 | Code freeze, release checklist, parity, signed Desktop and Desktop Pro | **26 Feb 2027** |
| Evidence V1–V7 | Scheduled one by one as each pre-registration is signed; never on the release's critical path | Rolling |

If the owner wants one release as in 0.41, Phases 1 to 4 run in parallel and merge to main when
green (build order "all together" of 7 November still applies).

---

## 5. Owner gates

| Gate | Decision | Default if no answer |
|---|---|---|
| K1 | Confirm the 0.42 edition rule (all connectors open; products in Pro; operations in Enterprise), amending F10a | Apply it to new connectors; leave existing code as is until confirmed |
| K2 | Apple secrets into kshana-pro (steps given 12 Nov) | Desktop Pro stays unsigned |
| K3 | Payment provider (Paddle recommended) and a test account | Checkout stays untested |
| K4 | Prices (June proposal as a starting list), bank account, VAT advice | Payments stay off |
| K5 | Windows signing account and runner billing | Windows builds stay unsigned and unrun |
| K6 | B7 attack IQ export position (counsel's approval stands as stated 7 Nov) | B7 not built |
| K7 | Legal checks: EDF third-country-control test with an individual non-EU shareholder; NAVISP eligibility via the Estonian Space Office; whether to form an Indian entity for iDEX and IN-SPACe | No EDF, NAVISP or Indian bids |
| K8 | Lead product: E1 forensics (recommended) or E3 scorecards | E1 first |
| K9 | Sign this plan and its dates | Phase 0 only |

---

## 6. KPIs for 0.42 (measured at release)

| KPI | 0.41 | Target 0.42 |
|---|---|---|
| Connectors with conformance vectors | ~25 (est.; count at Phase 1 start) | +40 |
| Systems demonstrated end to end (Signal K, OpenSky, Grafana, Yamcs, QGIS, PX4, a grandmaster, a simulator) | 4 (est.) | 12 |
| Pro products sellable with a public sample | 0 | 4 (E1, E2, E3, T1) |
| Paying customers or design partners | 0 | 1–3 |
| Rows promoted to VALIDATED or PARTNER | 176 | +10, only through signed pre-registrations |
| Signed, notarised downloads | Desktop (macOS) | Desktop and Desktop Pro (macOS); Windows if K5 clears |

---

## 7. What stays out of 0.42

- Avionics or maritime type-approval work, and any claim beyond test evidence.
- Galileo PRS, US SBIR, UK iX and ESA Celeste routes (eligibility, research of 13 Nov).
- Reverse-engineering closed vendor formats; only public or licensee-documented formats.
- Anything that configures or transmits on a radio without an explicit, logged opt-in.
- Defence-only products; Kshana stays civil and commercial-first.

---

## 8. Sources

Internal: `ROADMAP-0.41-0.43.md`; `docs/strategy/PRO-ARKSA-IP-DEEP-DIVE.md`;
`docs/strategy/PRICING-PROPOSAL.md`; `docs/gtm/ICP-AND-TARGET-LIST.md`; `src/license/mod.rs`;
open engine `docs/CAPABILITY.md`, `docs/CONNECT.md`, `CHANGELOG.md` [0.41.0].
External figures and links: the research page (artifact Vd8UHHR5Sf1CbbzJUWx2S1) and
`records/ops/kshana/plan-0.42/` in VCprep.
