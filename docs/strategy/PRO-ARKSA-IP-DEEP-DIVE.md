<!-- PROPRIETARY — INTERNAL STRATEGY. Contains patent strategy: never copy into a public repository, site or post. -->

# Kshana Pro inside Arksa: value, ownership and patentable inventions

**Owner:** Ashforde OÜ · **Status:** analysis for the owner's decision · **Date:** 3 November 2026
**Not legal advice.** Every patent point below must be checked by a European patent attorney before
anything is filed or published. The purpose here is to get the decisions and the timing right.

## 1. What Kshana is today (measured, 3 November 2026)

| | |
|---|---|
| Open engine (AGPL or commercial) | ~451 000 lines of Rust; 92 scenario kinds; 194 bundled scenarios; 401 verification-matrix rows, each with its evidence and status; ~3 800 library tests and 566 integration-test files; 19 languages; CLI, Python, WebAssembly, MCP, Studio |
| Kshana Pro (private overlay) | ~72 000 lines of Rust: study packs, twin framework, batch and Monte Carlo, service and jobs, signed licence tokens, branded reports and dossiers; CI green on engine 0.40 with a release-build token smoke test |
| Arksa (proprietary, TypeScript) | ~530 000 lines; open Kshana embedded as WebAssembly (DOP, availability, ionosphere); Kshana Pro tier follows the Arksa plan (`KSHANA_TIER_FOR_PLAN`); Arksa issues the Kshana Pro token with the issuer key; desktop sidecar runs Pro on the device |
| 0.41 so far | obs/1 schema, Connect mapping files and conformance kit, all cores with identical bytes, bounded memory and a governor, ntpd / White Rabbit / RINEX 4 readers, 34 signed pre-registrations |

Nothing else in civil PNT combines, in one engine, simulation of the threat, live monitoring of
real receivers, independent scoring against signed bars, and space and lunar navigation. That
breadth is the asset. The question is what to sell on top of it, and what to own.

## 2. The market: who sells what (research)

| Category | Examples | What they charge for | What they do not do |
|---|---|---|---|
| Monitoring probes | GPSPatron GP-Probe and GP-Cloud | Free live view; paid automatic detection, PPS monitoring, API, notifications, retention (7 days raw, 3 months events), on-device detection and scripted response; subscription or perpetual, per probe ([licences](https://gpspatron.com/gp-probe-din-l1-licenses/)) | Needs their hardware; no simulation, no independent scoring, no signed reproducible verdicts |
| Receiver and timing firewalls | Receiver makers' anti-jam and anti-spoof features, timing appliances | Hardware with protection inside | Vendor-specific; nothing assures the fleet as a whole |
| Space RF intelligence | Commercial RF-geolocation constellations offering GNSS interference detection products ([example announcement](https://sensorsandsystems.com/?p=97725)) | Data subscriptions, government contracts | No ground truth, no customer-side receivers, no assurance of the customer's own systems |
| Interference maps | Public maps from aircraft ADS-B integrity values | Free or advertising | Descriptive; no evidence a customer can rely on |
| Test equipment | Simulators, sync testers ([portable sync tester](https://electrorent.com/us/products/telecommunication-transmission-testers/transmission-tester-portable-up-to-10gbit/calnex/sentinel/01t5Y00000EcWfCQAV)) | Hardware plus licensable measurement channels | Lab only; no field assurance |
| Research | Crowdsourced jammer localisation from phone C/N0 and AGC ([2023](https://arxiv.org/pdf/2305.02038), [2022](https://arxiv.org/pdf/2204.13974)); power-difference localisation with commercial receivers ([2026](https://web.stanford.edu/group/scpnt/gpslab/pubs/papers/Kriezis_ION_PPNT_2026_LCM_PDOA.pdf)); a granted patent on networked receivers locating an interfering source ([US9766343B2](https://patents.google.com/patent/US9766343)) | — | Methods, not an assurance product |

**Demand.** The US framework is voluntary: the Foundational PNT Profile (NIST IR 8323) asks
organisations to identify systems that depend on PNT, select sources, *detect disturbances and
manipulation* and manage the risk ([NIST IR 8323 r1](https://csrc.nist.gov/pubs/ir/8323/r1/ipd)).
In the EU there is policy direction (resilient timing, a terrestrial timing backbone) but no binding
requirement for GNSS backup was found in NIS2 or the critical-entities directive
([InsideGNSS on resilient timing](https://insidegnss.com/europes-jrc-makes-resilient-timing-a-priority/)).
So the paying need is **evidence that an auditor, insurer or customer accepts**, not a legal mandate.

**What open-core businesses gate.** The buyer, not the user, decides the tier: individual
contributors get the free product, managers and directors pay for scale, governance and compliance
([GitLab pricing model](https://handbook.gitlab.com/handbook/company/pricing),
[tiering guidance](https://handbook.gitlab.com/handbook/product/product-processes/tiering-guidance-for-features/));
features that exist free elsewhere cannot be gated (GitLab moved its scanners to the free tier).

## 3. The gap nobody fills: assurance you can prove

Every product above answers "is there interference now?". None answers the questions a director,
regulator or insurer asks:

1. *Was my system's position and time trustworthy at 03:12 last Tuesday, and can a third party
   check that answer without trusting me?*
2. *How long will my timing stay inside its budget if GNSS goes now, at this site, with this clock,
   and how sure are we of that number?*
3. *Which of my sites, routes, satellites or operations are exposed to the threat environment of the
   next 24 hours, and what is the cheapest change that keeps them inside their budgets?*
4. *Does this receiver behave as its maker says under attack, measured against bars nobody could tune to?*

Kshana already has the parts for all four: byte-identical replay on any platform, signed evidence
and raw-slice hashes, digital twins of clocks and receivers with pre-registered validation, threat
and propagation models, an independent referee and benchmark, and (through Arksa) the mission and
orbit engine. **The Pro product is to join them into one continuously running, provable assurance
record: the PNT assurance ledger.** That is the thing that does not exist in PNT today.

## 4. Pro, re-centred on the assurance ledger

| Who pays | Their question | Pro gives | Charged |
|---|---|---|---|
| Operations manager (telecom, grid, port, airport, operator of satellites) | One view of every site, alerts, a monthly report | Station fleet, area trust across sites, alert routing, history, SLA reports | Per Station a year; in Arksa, per satellite in Arksa Ops |
| Head of resilience, compliance or risk | Prove what happened and that we detect it | **The assurance ledger**: signed, third-party-verifiable verdicts with the raw evidence, audit packs mapped to NIST IR 8323 functions and the sector timing standards, insurer scorecards | Pro core; Enterprise for audit and retention |
| Timing owner | How long will we hold, and how sure are we | **Predictive assurance**: the twin's predicted time-to-breach per site, recalibrated against what each site actually does, with an alarm before the budget is breached | Pro |
| Mission and network planner (Arksa) | What is exposed, and what is the cheapest fix | **Threat-aware planning**: routes, station placement, satellite operations and holdover schedules optimised against the forecast threat field and each receiver's measured vulnerability | Pro and Enterprise inside Arksa |
| Anyone without a GNSS specialist | Keep me current | **Threat library subscription**: signed monthly detector calibrations and scenarios from public datasets and incidents | Annual subscription: the renewal engine |
| Receiver and antenna makers, integrators, buyers | Independent proof under attack | **Signed, pre-registered referee scorecards** with sealed holdouts | Per test or per product line |
| OEM | Embed Kshana in a closed product | Commercial engine licence | Royalty or flat |

Free stays free: every reader and format, mapping files, the conformance kit, schemas, all kinds,
single-site detection and alerts, the free Desktop, one Station, and all published evidence.

## 5. Arksa owns everything Kshana is: how, legally and commercially

1. **Ownership is already in place.** Ashforde OÜ holds the copyright in Kshana; every commit is
   Ashforde's own and outside contributions carry the inbound grant that permits the commercial
   edition (`docs/SUITE.md` in Arksa). Arksa and Kshana Pro are Ashforde products.
2. **Sign the internal licence note now** (an open founder decision in Arksa's `docs/SUITE.md`):
   Ashforde OÜ licenses Kshana (all versions, perpetual, transferable with either product) to
   Arksa under the commercial edition. Without it, a sale, spin-out or investor review of Arksa
   depends on "same company". With it, Arksa may embed the engine under the commercial licence
   rather than only call it.
3. **If Arksa becomes its own company** (investment, sale), assign or exclusively license
   Kshana's copyright and the patents below to it, keeping the open project open. This is a
   corporate-counsel decision; the point is to make it consciously before an investor asks.
4. **One commercial system.** Arksa already has plans, online checkout and the token issuer.
   F10g (payments) should **reuse Arksa's billing and issuer** for standalone Kshana Pro too,
   instead of a second merchant integration: one price list, one token format, one revocation list.
   Kshana Pro stays sellable alone (telecom, grid, maritime buyers who never need Arksa), but it is
   bought through the same system.
5. **Inside Arksa, Kshana Pro becomes the PNT pillar of Arksa Ops:** spaceborne-receiver
   interference and timing assurance for flying satellites (the planned $500 per satellite a month
   operations line), and threat-aware mission design in the studio. That is value no mission tool
   offers, and it rises with Arksa's plan, not as an add-on.

## 6. Patents: what can be protected, and the timing that decides it

### The rules that matter (to be confirmed by counsel)

- **Europe:** computer-implemented inventions, simulations included, are patentable when they
  have a further technical effect beyond running on a computer (EPO Enlarged Board, G 1/19:
  [summary](https://www.carpmaels.com/g1-19-computer-implemented-simulation-inventions-are-patentable-at-the-epo/),
  [EPO case law](https://www.epo.org/en/legal/case-law/2025/clr_i_a_6_5_6_a.html)). Controlling a
  receiver, a timing source, a sensor's tasking or a vehicle's route is the kind of technical use
  that qualifies; a report alone is weaker.
- **Novelty is absolute in Europe:** anything Ashforde has made public (the open repository, docs,
  the 34 pre-registrations, the preprint) is prior art against Ashforde's own later application.
  The US gives a 12-month grace period for an inventor's own disclosure.
- **AGPL and patents:** the AGPL carries a contributor patent licence (section 11) to everyone who
  receives the program ([commentary](https://dev.to/praveenlavu/the-gpl-v3-patent-trap-nobody-checks-until-a-lawyer-walks-your-requirementstxt-1hkm)).
  **A patented method must never go into open Kshana**; it lives only in Kshana Pro and Arksa.
- **Prior art already exists** for crowdsourced C/N0/AGC localisation and power-difference
  localisation (section 2), so B3a as such is not a patent candidate. Value is in combinations
  with a technical effect that nobody has published.

### Consequence for what was published today

The 34 pre-registrations (public since 3 November 2026) describe methods: area trust across
stations (B5), the space and ground hybrid locator (M5), tip-and-cue latency (M2), the
replay-equals-live bench protocol (L8). At the level of detail they disclose, these are prior art in
Europe from today. In the US, Ashforde could still file on its own disclosed content until
**3 November 2027**. Details not in those files (specific estimators, calibrations,
architectures) remain undisclosed.

**Change of practice from now on (recommended):** for any Pro method, publish only the
**SHA-256 hash of the pre-registration** (a timestamped commitment that the bars came first) and
release its text after the patent filing. This keeps pre-registration credible and keeps novelty.
Open-engine pre-registrations stay public as today.

### Invention candidates (none disclosed in detail yet; for counsel's prior-art search)

| # | Candidate | Technical effect (the G 1/19 test) | Why it may be new |
|---|---|---|---|
| P1 | **Verifiable PNT verdict**: a signed record binding the raw observation bytes, the engine build hash, the configuration and the verdict, from which an independent party re-executes on any platform and must obtain the identical verdict hash; disagreement localises the step that differs | Reproducible control of an assurance decision across heterogeneous hardware (portable arithmetic, deterministic parallelism) | Tamper-evident logs exist generally; deterministic, platform-independent re-execution of a PNT integrity decision with step localisation was not found |
| P2 | **Predictive time-to-breach with online twin recalibration**: a clock and receiver twin predicts each site's holdover budget, is recalibrated from the site's own measured behaviour with a coverage guarantee, and switches the timing source or raises the alarm before the predicted breach | Controls the timing source and the alarm | Holdover monitoring exists; a recalibrated predictive switch with calibrated coverage was not found |
| P3 | **Joint self-calibration and localisation across heterogeneous receivers**: each receiver's AGC and C/N0 response is estimated together with the interference source positions from co-observation and sky geometry, so mixed fleets need no lab calibration | Improves the accuracy of a physical localisation from existing receivers | Published methods avoid calibration or assume one receiver type; joint estimation across vendors was not found |
| P4 | **Cross-layer cueing**: a detection in one layer (orbit, ground Stations, aircraft and ship reports) re-tasks another (raises a Station's IQ capture rate, schedules a pass, narrows a search region), optimising the sensors' resources | Controls sensor tasking and data capture | Space-to-ground cueing for GNSS interference with receiver-side capture control was not found (the Space Inventor tip-and-cue concerns their own satellites, so keep the claims ground-side and multi-layer) |
| P5 | **Threat-aware mission and route planning**: routes, satellite operations and holdover schedules optimised against a forecast interference field and each receiver's measured vulnerability profile from the referee | Controls a vehicle's route or a spacecraft's operations | Planning around forecast PNT threat with receiver-specific vulnerability was not found; strongest inside Arksa |
| P6 | **Sealed-holdout scoring**: test vectors committed by hash before a device is submitted, revealed after scoring, with signed scorecards | Weak alone (close to a business method); combine with P1 | Supporting claim, not a lead |

### What to do, in order

1. **This week:** stop publishing new Pro method detail; adopt hash-only pre-registration for Pro.
2. **Engage a European patent attorney** for a novelty search on P1 to P5 (start with P1, P2 and P5,
   the strongest technical effects and the most product-relevant).
3. **File US provisional applications** for the candidates that survive the search. They are cheap,
   fast, and set a priority date that a European or PCT application can claim within 12 months.
4. **Then a PCT or EPO application** for the ones that matter commercially.
5. **Decide on the 3 November disclosures** (B5, M5, M2, L8): file in the US before 3 November 2027
   if counsel sees value; otherwise treat them as Ashforde's published prior art, which also stops a
   competitor patenting them.
6. **Record inventorship and assignment** to Ashforde OÜ (or Arksa's company) for each candidate.

## 7. Changes to the 0.41 plan this implies

- **F10a** becomes the matrix in section 4 (one Station free, then per Station; adapters free;
  Pro = fleet, assurance ledger, predictive assurance, threat library, signed, pre-registered scorecards,
  cost planner; Enterprise = high availability, single sign-on, audit, multi-tenant, air-gapped).
- **F10g** reuses Arksa's plans, checkout and issuer; no second merchant.
- **New:** F10h threat library subscription; F10i compliance evidence packs; **P1 to P5 built only
  in Kshana Pro** (and surfaced in Arksa), never in the open engine.
- **New gate:** patent counsel engaged before any Pro method is described publicly.
- **Arksa:** sign the internal Kshana licence note; Kshana Pro becomes the PNT pillar of Arksa Ops.

## 8. Decisions for the owner

1. Adopt the assurance-ledger positioning and the section 4 matrix (replaces the earlier F10a draft).
2. Payments through Arksa's billing and issuer for both products.
3. Sign the internal licence note (Kshana to Arksa, commercial edition).
4. Engage patent counsel; hash-only pre-registration for Pro methods from today.
5. Whether Arksa will become a separate company, so assignment is planned rather than rushed.

## Addendum, 3 November 2026: decisions with Arksa's Alpha (round 2)

These supersede the sections above where they differ (VCprep `records/ops/ARKSA_KSHANA_DISCUSSION.md`, D-K11 to D-K14).

- **Three layers.** Open (AGPL), Pro (sold, runs on customer machines), Core (Kshana + Arksa only, never shipped, never unlocked by a token).
- **Placement.** P1 verifiable verdict and P2 predictive time to breach stay in Core until filed, then ship in Pro. P3 joint self-calibration, P4 cross-layer cueing and P5 threat-aware planning stay in Core as trade secrets.
- **kshana-core.** Private repository owned by the Kshana seat. It is reached as an authenticated service by Arksa's server and Ashforde's hosted service. It is never linked into the Pro build (CI check), never compiled to WebAssembly, the desktop sidecar or the CLI, and never written by Arksa teams.
- **Sovereign.** By default through an Ashforde-hosted endpoint. On-premises Core only under a founder-approved contract: confidentiality, no reverse engineering, audit rights, a signed server image only, no source.
- **Model register.** Generated from the verification matrix into `register_slice.json`; Core entries appear by id and SHA-256 only. First 20 entries by 10 November.
- **One physics source.** A pre-registered head-to-head decides it. Order: ionosphere, then clock and holdover, then troposphere, then receiver and interference propagation.
- **Wording.** "Own the market" is an internal goal. Public copy compares on facts, with no "certified".
