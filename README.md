# Kshana Pro

**Proprietary overlay on the open [Kshana](https://github.com/AshfordeOU/kshana) PNT-resilience engine.**
Private — All rights reserved (see [`LICENSE`](LICENSE)). **Not open source. Do not distribute.**

---

## What this is

Kshana Pro is a **strict superset** of open Kshana. It does **not** fork the open
engine — it **depends on it** as a Cargo dependency and extends it through the open
core's public, semver-stable trait seams. Every open feature is present *by
construction*; Pro adds the value that must stay closed.

```
        ┌──────────────────────────────────────────────────────────────┐
        │                     kshana-pro  (PRIVATE)                      │
        │                                                                │
        │  OVERLAY (code)            KNOWLEDGE LAYER (never leaves)       │
        │  ┌──────────────────┐      ┌──────────────────────────────┐    │
        │  │ L1 twin          │      │ docs/tenders/  13503·13494·   │    │
        │  │ L2 resilience 🔒 │      │   lunar  (bid evidence, decks)│    │
        │  │ L3 suite (gated) │      │ papers/        (arXiv-track)  │    │
        │  │ + 33 Pro packs   │      │ docs/gtm · docs/strategy      │    │
        │  │   via ExternalPack│     │ legal/  (licence·NDA·export·  │    │
        │  │   / ErrorModel   │      │   BIPR·trademark)             │    │
        │  └────────┬─────────┘      │ docs/kshana-architecture.html │    │
        │           │                └───────────────┬──────────────┘    │
        └───────────┼────────────────────────────────┼───────────────────┘
                    │ depends on (never forks)        │ cites (never copies code)
        ┌───────────▼─────────────────────────────────▼───────────────────┐
        │                  kshana  (OPEN, AGPL-3.0-only)                   │
        │   the whole validated engine + every surface                    │
        │   · 3 quantum-PNT verticals     · 7 lunar-PNT modules           │
        │   · representativeness + gaps-to-flight ledger                  │
        │   · validated-island matrix (verification.rs, honesty gate)     │
        └──────────────────────────────────────────────────────────────────┘

   Capability flows OUT → open repo.   Evidence · strategy · money stay IN.
   Every claim, both sides, is underwritten by the VALIDATED / MODELLED
   honesty labels + the representativeness ledger.
```

See [`docs/OVERLAY-ARCHITECTURE.md`](docs/OVERLAY-ARCHITECTURE.md) for the seam map
and [`legal/`](legal) for IP/NDA/export-control/BIPR/trademark material.

**Tender / bid material** lives under [`docs/tenders/`](docs/tenders/) — the FutureNAV
pursuit dossier covering the flagship **AO/1-13503 Quantum-Enabled PNT** (deadline
17 Aug 2026), **AO/1-13494 AI & Digital Payload PNT** (9 Jul 2026) and the anticipatory
**lunar-PNT** assets, plus per-tender briefings, the ECOS/FFP cost volume, the dominance
deck, and cross-tender deliverables. Start at [`docs/tenders/INDEX.html`](docs/tenders/INDEX.html)
(or [`docs/tenders/README.md`](docs/tenders/README.md)), and read
[`docs/kshana-architecture.html`](docs/kshana-architecture.html) first if you're new.
PRIVATE — never publish any of it to the open kshana repo.

## Repository contents — where to find what

Kshana Pro is two things in one private repo: **(a)** the proprietary Rust **overlay**
(the `src/` packs, documented below), and **(b)** the **private knowledge layer** — bid
evidence, research, commercial and strategy material — that turns the open engine into
won tenders and revenue. The map:

| Area | Path | What's there |
|------|------|--------------|
| **Field guide** | [`docs/kshana-architecture.html`](docs/kshana-architecture.html) | From-zero explainer: PNT, the engine, the honesty/validated-island model, the open ⟷ private split, the three quantum-PNT areas, and how we win ESA tenders. Start here if you're new. |
| **Tender pursuits** | [`docs/tenders/`](docs/tenders/) | All ESA bid material — open [`INDEX.html`](docs/tenders/INDEX.html) / [`README.md`](docs/tenders/README.md). |
| → Quantum-PNT (flagship) | [`docs/tenders/13503-quantum-pnt/`](docs/tenders/13503-quantum-pnt/) | AO/1-13503 (deadline 17 Aug 2026): pursuit tracker, demonstrator capability across all 3 SoW areas, clarifications audit, SE-verification & software-PA evidence, TRL dossier, the validation-dataset catalog, and the honest security-validation negative. |
| → AI / digital-payload PNT | [`docs/tenders/13494-ai-payload-pnt/`](docs/tenders/13494-ai-payload-pnt/) | AO/1-13494 (deadline 9 Jul 2026): NAV-DT + AI-eval-testbed work packages. |
| → Lunar PNT | [`docs/tenders/lunar-pnt/`](docs/tenders/lunar-pnt/) | Anticipatory shelf assets vs Moonlight/LCNS (capability statement + fit map). Outreach PARKED pending founder go. |
| → Cross-tender + cost | [`docs/tenders/cross-tender/`](docs/tenders/cross-tender/), [`cost-volume/`](docs/tenders/cost-volume/), [`briefings/`](docs/tenders/briefings/) | BIPR/IP posture, D1.1 benefits & D1.4 MRD, the ECOS/FFP cost volume, per-tender briefings, the dominance deck. |
| **Research papers** | [`papers/`](papers/) | arXiv-track papers (LaTeX + figures + reproducible data) + the open-engine technical report. **arXiv-first** — nothing public until the founder announces. |
| **Commercial / GTM** | [`docs/gtm/`](docs/gtm/) | ICP & target list, outreach sequence, sales one-pager, qualification/objections. |
| **Strategy & roadmaps** | [`docs/strategy/`](docs/strategy/), [`docs/roadmaps/`](docs/roadmaps/) | Pricing proposal, productization roadmap, phase plans, the quantum-PNT-13503 and lunar-PNT build roadmaps. |
| **Legal** | [`legal/`](legal/) | Commercial licence, NDA/SOW/DPA templates, IP policy, export-control determination, BIPR & trademark briefs, counsel-review package. |
| **The overlay (code)** | [`src/`](src/), [`scenarios/`](scenarios/), [`tests/`](tests/) | The Pro packs + deliverable/deployment modules ([Status](#status--what-is-built-vs-gated)); seam map in [`docs/OVERLAY-ARCHITECTURE.md`](docs/OVERLAY-ARCHITECTURE.md). |
| **User guides (code features)** | [`docs/PRO-POWER.md`](docs/PRO-POWER.md), [`docs/DESIGN-OPTIMISER.md`](docs/DESIGN-OPTIMISER.md), [`docs/UNCERTAINTY.md`](docs/UNCERTAINTY.md), [`docs/MISSION-DOSSIER.md`](docs/MISSION-DOSSIER.md), [`docs/CAMPAIGN-WATCH.md`](docs/CAMPAIGN-WATCH.md), [`docs/SPECTRUM-COEXISTENCE.md`](docs/SPECTRUM-COEXISTENCE.md), [`docs/SERVICE.md`](docs/SERVICE.md), [`docs/MBSE.md`](docs/MBSE.md) | The summary of the six Pro power features, then one guide per feature: what it does, the command, inputs and outputs, worked examples with measured numbers, and what is not done. |

**Visibility rule (non-negotiable):** everything in this repo is **PRIVATE**. Capability
*code* is mirrored to the open [`kshana`](https://github.com/AshfordeOU/kshana) repo;
**bid evidence, papers, strategy, commercial and legal material never leave this repo.**

## Status — what is built vs gated

The overlay is well past its P0 seam: **thirty-three Pro packs** (plus the **Pack C**
scenario-CI subscription product) are built and tested on public data across the
P0 / L1 / L3 value layers, plus the deliverable and deployment modules listed below. What stays **gated** is never a *framework* — it is the
*instance* that needs private data, export clearance, or a signed customer: a real
customer-device twin, the export-controlled L2 threat models, and the productised
ECSS/MBSE SKU. The honest line is **framework (built) vs gated instance (not built
on spec)**.

### Built Pro packs

Each runs on public data; the tier shown is the licence required to *run* it (see
[Licensing](#build--run)). The table covers the framework packs; the full list of the
thirty-three registered in the signed tier manifest (`commercial/tier-manifest.json`): the seventeen
this crate contains are in [`src/registry.rs`](src/registry.rs), incident forensics
(`pro-incident-forensics`, roadmap 0.42 E1) arrives as a Pro pack, all listed with their tiers by
`cargo run -- kinds`. `tests/readme_counts.rs` fails the build if the pack count written
here stops matching the registry.

**Editions (roadmap 0.42, section 1a; X3 done).** Fifteen of the thirty-three are
**Arksa-exclusive**, and so are the `mbse`, `design`, `coexist`, `mission`, `navdt`,
`spacestream` and `mrd` commands: their code is no longer in this repository. It moved to the
private server-side crate (Arksa servers only), which depends on this one and registers
through `kshana_pro::extension`; this crate keeps their manifest entries by hash and refuses
them with a neutral message. The edition of every kind is in a signed tier manifest verified
at start-up. Details, checks and what remains: [docs/PROTECTION.md](docs/PROTECTION.md).

| Pack (`kind`) | Layer | What it does | Min tier | State |
|---------------|-------|--------------|----------|-------|
| `pro-seam-check` | P0 | Smallest real `ExternalPack` — exercises the whole overlay contract end-to-end | Evaluation (free) | ✅ built + tested |
| `pro-twin` | L1 | Calibrates a clock twin to a device's *published* Allan budget; validates reproduction with the open-core Allan estimator; adds holdover-UQ **MTIE(τ)-mask compliance** (reuses the validated `kshana::allan::mtie` estimator with a conservative ±kσ band) against caller-supplied ITU-T-style masks | Pro | ✅ framework demonstrator (customer-device instance gated) |
| `pro-quantum-systematics` | L1 | Optical-clock systematic frequency-shift budget (BBR, gravitational redshift, Zeeman, extras) + uncertainty floor — formulas built, species coefficients are *cited inputs*, never baked in | Pro | ✅ framework demonstrator |
| `pro-trade-study` | L3 | Runs architecture variants through the embedded open engine, ranks them on a figure of merit, and stress-tests the winner with an **8-method MCDA concordance panel** (WSM/WPM/WASPAS/MOORA/TOPSIS/VIKOR/PROMETHEE II + COPRAS; Kendall-τ concordance + rank-reversal count, folded into the ECSS Annex L / NASA-SE-6.8 reports); emits an audit-grade evidence pack with reproducible scenario hashes | Pro | ✅ framework demonstrator (licensed SKU gated) |
| `pro-optimism-gap-audit` | Pack A | The in- vs out-of-distribution **optimism-gap audit** packaged as a sold study: ingests a customer detector through the open-core eval testbed, scores the ID→OOD degradation, and emits the branded Pack A report (`report_pack_a` + `packs::detector_intake`) | Pro | ✅ built + tested |
| `pro-batch` | scale | Parallel Monte-Carlo runner: fans one base scenario across many seeds concurrently, aggregates a figure of merit (mean/std/min/max/p05/p95) | Enterprise | ✅ built + tested |
| `pro-scenario-ci` | Pack C | The **scenario-subscription CI** product (run via `kshana-pro ci`): a manifest of scenarios + metric thresholds gated against a rolling baseline (the JSON output doubles as the next run's baseline), emitting the branded Pack C regression report (`pack_c::{manifest,metrics,gate,run}` + `report_pack_c`). **Campaign watch**: `[[watch]]` tables name the figures that matter (result paths, aggregates, named campaign figures, sweep crossings), what worse means for each and optional unit-checked requirements; a Monte Carlo figure is gated by a Welch t-test on the mean and an F-test on the spread; `--history` keeps a hash-chained history (`ci history verify`), `--trend` draws it, `--exact` reports every changed member result, `ci diff` lists what moved. Five subscription packs in `scenarios/packs/`. See [`docs/CAMPAIGN-WATCH.md`](docs/CAMPAIGN-WATCH.md) | Pro | ✅ built + tested |
| `pro-timing-holdover` | T | **Timing assurance** (`kshana-pro timing`): holdover model and time to breach of operator-set limits for every clock of a mixed-vendor fleet, each read from its own source (TICC, TimeLab, RINEX clock, CGGTTS, linuxptp, chrony, ntpd, White Rabbit, PPS); per-clock hash-chained evidence history, a fleet page in the Station console and a Grafana dashboard (`pro-timing-fleet`), and signed audit reports mapped to MiFID II RTS 25 traceability or the NIST IR 8323 profile (`pro-timing-audit`). Prediction bars pre-registered as V5 and, for the successor predictor, V5b: **both missed and published**, so the time to breach is not a validated claim. See [`docs/TIMING-ASSURANCE.md`](docs/TIMING-ASSURANCE.md) | Pro | ✅ built + tested; V5 and V5b missed |
| `pro-threat-scenario` | E | **Threat scenario library** (`kshana-pro threats`, feature `pro-threat-library`): twelve curated real events (Baltic, Hormuz, Red Sea, JammerTest, TEXBAT) with provenance and licence notes and **no source data**; each has a reproducible analogue (open NMEA generator, scored by the open receiver-trust monitors) and/or a timing profile; exports through the open I4 test-bench writers; a signed monthly feed that verifies for free. See [`docs/THREAT-LIBRARY.md`](docs/THREAT-LIBRARY.md) | Pro | ✅ built + tested; analogues are MODELLED |
| `pro-receiver-scorecard` | E | **Receiver resilience scorecard and test report** (`kshana-pro scorecard`): a test campaign (library analogues or the receiver's own logs under stated events, optional I4 truth), scored for detection, availability, error, misleading output without warning and recovery under bars stated before scoring; mapped to the DHS resilience levels and the IEEE P1952 draft topics as test evidence; frozen format `kshana-pro-receiver-scorecard/1`; signed report verifies for free. Public sample pre-registered (E3-B1 met, E3-B2 part (a) **missed and published**, E3-B3 met). See [`docs/RECEIVER-SCORECARDS.md`](docs/RECEIVER-SCORECARDS.md) | Pro | ✅ built + tested; sample published |
| `pro-timing-scorecard` | T | **Timing scorecards** (`kshana-pro timing scorecard`): grandmasters and the PTP network behind them under threat library timing profiles (time push, step, denial); detection against the T1 holdover envelope or a site cross-check, delivered error per clock and limit, holdover after detection; levels via the E3 scoring. Public sample pre-registered (T4-B1..B3 met); inherits T1's unvalidated holdover model. See [`docs/TIMING-ASSURANCE.md`](docs/TIMING-ASSURANCE.md) | Pro | ✅ built + tested; MODELLED |
| `pro-area-picture` | F | **Area console** (`kshana-pro area picture`, feature `pro-area-console`; Station console page `GET /area/console`): fuses Kshana Stations, receivers and customer-supplied third-party feeds into one area trust picture per monitored area through the open engine's pre-registered area rule, with event timelines (sources, basis, corroboration) and wide-area events across areas; tenancy hooks (more than one tenant in one console is Enterprise); HA failover tested. **F2: detections and areas only, never emitter positions** (a hard switch with no "on" value, until the locator successor meets its gate on the blind JammerTest 2027 run). Two-site demo on a synthetic analogue of published Baltic events. See [`docs/FUSION-AND-OPERATIONS.md`](docs/FUSION-AND-OPERATIONS.md) | Pro (multi-tenant: Enterprise) | ✅ built + tested; MODELLED |
| `pro-multi-source-trust` | F | **Multi-source trust** (`kshana-pro trust`): chooses epoch by epoch among GNSS, OSNMA-authenticated Galileo, LEO, eLoran, PTP and holdover under a rule fixed before the code (exclusion, cross-group consistency votes, protection levels, holdover with class noise and drift bound, hysteresis), with a hash-chained evidence record of every choice that verifies for free. Switching bars pre-registered as V4 and **met on all three data sets** (recorded IGS clocks with modelled sources and injected faults, and a synthetic set); no real multi-source recording scored. See [`docs/evidence/v4-multi-source-switching/`](docs/evidence/v4-multi-source-switching/) | Pro | ✅ built + tested; V4 met |
| `pro-planner-front` | F6 / B1b | **Planner Pareto front with cost**: every network of `m_min` to `m_max` stations from the open planner's candidates, capital plus years of running cost against the open planner's objective, exact (exhaustive) front, best per `m` equal to the open planner's own placement, cheapest network meeting a target. MODELLED (free-space maps) | Pro | ✅ built + tested; MODELLED |
| `pro-ais-integrity` | E5 | **AIS and position integrity scoring** (`kshana-pro integrity score`): per AIS identity, gaps, one identity in two places, and AIS positions inconsistent with independent RF fixes (open M4 v3 decision); integrity scores per fleet and per area with exact 95 % intervals; RF fixes with no matching AIS per area; signed. The pre-registered stage-2 offset test (V3, then V3b) **missed both times and is published**; its hits are listed as offset candidates and never change a class. Describes data consistency, never intent. See [`docs/EVIDENCE-PRODUCTS.md`](docs/EVIDENCE-PRODUCTS.md) | Pro | ✅ built + tested; V3, V3b missed; MODELLED |
| `pro-pnt-verification` | E4 | **Independent verification study of a new PNT service, user side** (`kshana-pro verify-service`): coverage and geometry accuracy of a LEO service from its published design (alone and with GNSS), OSNMA authentication integrity under seeded tampering; bars from a named pre-registration whose SHA-256 is checked at run time; signed. Public sample (Xona Pulsar published design, OSNMA synthetic stream): 3 of 6 bars met, published. Ground users only; lunar and spacecraft studies are not in this crate | Pro | ✅ built + tested; sample does not pass; MODELLED |
| `pro-uncertainty` | scale | **Uncertainty and sensitivity** (`kshana-pro uncertainty`): stated input distributions over any open scenario; per output the mean, spread, percentiles with a bootstrap interval, the probability of meeting a limit (Wilson interval), time-series bands, and Sobol' or standardised-regression sensitivity; seeded, thread-count independent, stop and resume. See [`docs/UNCERTAINTY.md`](docs/UNCERTAINTY.md) | Enterprise | ✅ built + tested |

**eLoran, R-Mode and terrestrial navaids (research 2026-11-20 gaps E37; features `pro-rmode-perf`, `pro-wwrns-trust`, `pro-eloran-claims`, `pro-navaids` (Pro) and `pro-eloran-sol-evidence` (Enterprise); `kshana-pro eloran rmode-perf|wwrns-trust|claim|interop|sol-evidence|ldc-fde`, `kshana-pro navaids`).** Draft R-Mode receiver performance tests (accuracy, availability, continuity, misleading information, time to alarm, update interval, acquisition, tone-cycle ambiguity) with confidence bounds and sample-size checks against a limits profile the user supplies (the NCSR 13 draft is not adopted and no limit is built in); GNSS + R-Mode + eLoran trust decisions per epoch with GNSS-suspect flags; timing-claim verification (for example 20 ns to UTC) with a GUM budget and BIPM Circular T, an interoperability test across stations steered to different UTC(k) (normalised error per pair), and a signed safety-of-life evidence index whose categories are Kshana's own; LDC/ASF fault detection and exclusion that separates correction faults from signal faults (patent-pending, on-prem builds only); a DME/VOR network twin (service volumes, ICAO Doc 9613 DME/DME criteria, critical DMEs), multi-sensor RNP scenarios with the e-conspicuity NACp backup test, and flight-inspection truth assurance. Pre-assessment and planning only, MODELLED. See [`docs/ELORAN-RMODE-NAVAIDS.md`](docs/ELORAN-RMODE-NAVAIDS.md).

**Regulator, type-approval and framework pre-assessment packs (research 2026-11-20 gaps E29; features `pro-regulator-pack`, `pro-interference-complaint`, `pro-bid-attestation`, `kshana-pro regpack`).** Requirement catalogues (IMO MSC.401(95)/MSC.432(98), MSC.1/Circ.1575, bridge alert management of PNT alerts, IACS UR E27, ETSI EN 303 413, the MASS Code, the NIST CSF 2.0 PNT profile draft, EU Space Act cybersecurity) state per requirement which signed Kshana evidence speaks to it and what is missing, with clauses cited only where the research read them. Checks behind them: a BAM alert-map check replayed over PNT data processing output, MASS override independence over a declared architecture, link-security evidence from a declared link configuration. Also an interference report annex under the headings of ITU RR Appendix 10 (effects and 0.5-degree areas only, never an emitter position) and a signed bill-of-materials export-status attestation with its test evidence chain for bids. Pre-assessment only: no pack states that a product or operator meets a document. See [`docs/REGULATOR-PACKS.md`](docs/REGULATOR-PACKS.md).

**Maritime AIS integrity evidence (research 2026-11-20 gaps E07, Pro parts; features `pro-ais-consistency`, `pro-ais-sanctions-pack`, `pro-ais-iuu-pack`, `pro-ais-subsea-pack`; `kshana-pro ais`).** On the user's own decoded AIS: cross-source consistency against radar, VTS, LRIT, satellite AIS or imagery detections with declared accuracies (residual against a stated gate, inconsistency episodes, AIS vessels unconfirmed inside a source's coverage, targets without AIS); a sanctions-review pack for vessels the user names (reception gaps, ship-to-ship encounter candidates, identity changes, implied-speed jumps, one MMSI on two transmitters; no list bundled, no determination); an IUU-review pack against declared zones (time inside, fishing-like speed band and broadcast fishing status, gaps near zones, encounters at sea); and a subsea-asset pack for declared cable and pipeline corridors (dwell, slow speed, anchor status while moving, vessels around a reported fault: presence, not attribution). Each is a signed evidence directory. Positions are the vessels' own broadcasts: nothing locates a radio emitter. MODELLED (synthetic scenario, `tests/ais_evidence.rs`). See [`docs/AIS-EVIDENCE.md`](docs/AIS-EVIDENCE.md).

**Lunar surface operations evidence, second slice (research 2026-11-20 gaps E24 B2404; features `pro-lunar-plume-visibility`, `pro-lunar-traverse-safety`, `pro-lunar-rf-forensics`; `kshana-pro lunar-surface`).** Dust-plume visibility as a MODELLED parametric ejecta-sheet model with every assumption stated (optical and Rayleigh-limit RF extinction along declared lines of sight, obscuration onset height); LTV/EVA navigation safety evidence from a traverse log (sigma honesty, hazard keep-out margins with the protection level, walk-back margin against declared consumables); RF self-interference forensics from a SigMF recording, analysis only (spurs, harmonic match to declared on-board sources, time correlation, in-band C/N0 loss). Signed evidence directories. MODELLED (synthetic inputs, `tests/lunar_surface.rs`). See [`docs/LUNAR-SURFACE.md`](docs/LUNAR-SURFACE.md).
**LEO-PNT evaluation (research 2026-11-20 gaps E21: B2101, B2107, B2108; feature `pro-leo-evaluation`; `kshana-pro leo-eval`).** Published multi-simulator test procedures frozen at 1.0, scorecards under registered method hashes (never ranked) and simulator-equivalence verdicts; SISRE, DCB, a T-RAIM fault-injection study and a PPP dependency register (evaluation only); an investor due-diligence template in which every verdict cites its source. See [`docs/LEO-EVALUATION.md`](docs/LEO-EVALUATION.md).

**Road, rail, automotive, eCall and 3GPP evidence (research 2026-11-20 gaps E30; features `pro-telematics-antispoof`, `pro-obu-fraud-eval`, `pro-ecall-tests`, `pro-emergency-location`, `pro-automotive-pl`, `pro-v2x-checks`, `pro-fault-injection`, `pro-nr-timing`, `pro-agnss-integrity`, `pro-rail-evidence`; `kshana-pro road|ecall|auto|nr|rail`).** Telematics and road-charging anti-spoof indicators with episodes, a jammer fingerprint library on receiver-side features (never an emitter position) and per-trip trust, pseudonymised by rule; a signed on-board-unit fraud evaluation with the distance a unit failed to report. The eCall GNSS tests of Delegated Regulation (EU) 2017/79 Annex VI (read on legislation.gov.uk) with every requirement mapped to its test, signed scoring of laboratory results, AML/E112 and PSAP location reliability against the declared confidence, duty-cycle detection and a tracker battery model. A SOTIF triggering-condition catalogue with protection-level validation per condition, V2X confidence-ellipse and time checks, and data-level fault injection. 5G TDD frame timing under GNSS loss, a PRS accuracy model and GNSS integrity reference vectors (not 3GPP conformance cases). A rail position hazard model including jamming and spoofing against the apportioned THR and a virtual-balise passage test with an overbound check; the EN 50129, ISO 26262/SOTIF and 2017/79 catalogues take their signed packs. MODELLED; pre-assessment wording only. See [`docs/ROAD-RAIL-AUTO.md`](docs/ROAD-RAIL-AUTO.md).

**Space-segment GNSS receiver evidence (research 2026-11-20 gaps E20; features `pro-space-rx-exposure`, `pro-space-rx-qual`, `pro-space-rx-see`, `kshana-pro space-rx`).** The receiver-side view of a GNSS receiver on a spacecraft: the ground-interference envelope it meets in orbit (TLE on the open engine's SGP4 over declared areas or degraded map cells; J/S, effective C/N0, time beyond the J/S it was tested to, Doppler offsets of ground transmitters seen from orbit, simulator test points at signal-parameter level), campaign scorecards against a mission profile with a selection database and a qualification process template, radiation-fault signatures in its output trace with injected fixtures, receiver-to-OBC corrupt-data vectors with OBC log scoring, and a SEE rate forecast. MODELLED; scorecards are evidence for a selection or review, not a statement that a part is fit for flight. See [`docs/SPACE-RECEIVER.md`](docs/SPACE-RECEIVER.md).

**Framework evidence packs (roadmap 0.42 E6; feature `pro-framework-pack`, `kshana-pro frameworks pack`)** and **insurer scorecard (E7; feature `pro-insurer-scorecard`, `kshana-pro exposure run`).** Seven framework templates (EASA SIB 2022-02R4, NIST IR 8323r1, DHS RPCF, IMO Circular Letter No. 5037 as a draft until its text is read, MiFID II RTS 25, the EU Space Act proposal, NIS 2) map signed Kshana Pro evidence to each duty, "mapped to" only, with the gap; the exposure summary counts signed forensics and integrity evidence for a fleet or route over a period, adds no detection, and is signed. Samples under `report-templates/samples/`. See [`docs/EVIDENCE-PRODUCTS.md`](docs/EVIDENCE-PRODUCTS.md).

**Log time integrity and dispute evidence (research 2026-11-20 gaps E06; features
`pro-log-time-integrity` and `pro-dispute-evidence`, `kshana-pro forensics log-time|dispute`).**
The time stamps of VDR/ECDIS NMEA, ACARS/CPDLC message logs and CSV/JSON exports (FDM, SATCOM,
GMDSS) checked against each record's reference and against the other clocks: steps, backward
time, frozen clocks, drift, GPS week rollover, leap seconds, ordering and cross-log
disagreement, with a corrected timeline. A charter-party dispute report (off-hire,
delay/laycan, DP downtime, deviation, safe port) says when GNSS position or timing integrity
was lost, for how long and which sources agree, citing the maritime pack, the forensics bundle
and the log check; signed. MODELLED, synthetic tests only, not a legal finding. See
[`docs/INCIDENT-EVIDENCE.md`](docs/INCIDENT-EVIDENCE.md).

**Alert policy engine (roadmap 0.42 F5; feature `pro-alert-policy`, `kshana-pro alerts route`).**
Routes decide who is told what, when and through which of the open engine's I5 outputs, with quiet
hours (held into a digest), escalation when nobody acknowledges, and de-duplication that never
hides a worse alert. It plans and sends nothing itself; the free edition keeps raw alerts
(`kshana-pro alerts raw`, no licence), and every plan accounts for every input alert. The area
console and the multi-source selector feed it. See [`docs/FUSION-AND-OPERATIONS.md`](docs/FUSION-AND-OPERATIONS.md).

### Layer attach points & gates

| Layer | Modules | Attaches via | Gate / trigger | State |
|-------|---------|--------------|----------------|-------|
| P0 | `packs::seam_check`, `registry` | `ExternalPack` | on-spec | ✅ built + tested |
| L1 | `twin`, `premium`, `packs::{twin,quantum_systematics}_pack` | `kshana::models::ClockModel`/`ErrorModel` + `allan` | `twin::customer_instance_gated` (NDA partner + contract) | ✅ frameworks built; customer instance gated |
| L2 | `resilience` | `kshana::models::ErrorModel` / `ExternalPack` | export clearance + customer | 🔒 fail-closed stub (legal wall) |
| L3 | `suite`, `packs::trade_study_pack` | `ExternalPack` + `kshana::api::run_scenario` | `suite::product_gated` (productised SKU) | ✅ frameworks built; SKU gated |

### Supporting modules (built)

Beyond the packs, the overlay ships the deliverable and deployment surface a
study/bid needs:

- **L3 assurance stack** (`suite::{decision,multi_method,provenance,gate,vv_dossier}`)
  — the trade study upgraded from a single figure of merit to a defensible, agency-shaped
  decision: an **8-method MCDA concordance panel** (Kendall-τ agreement + rank-reversal
  count), a **W3C-PROV** tamper-evident provenance record, a **fail-closed** multi-criteria
  assurance gate, and the capstone agency **V&V credibility dossier**. (The GSN argument and
  the VCD moved to the Arksa server crate with X3.)
- **L1 twin depth** (`twin::{holdover_uq,presets,optical_systematics}`) — holdover-UQ
  **MTIE(τ)-mask compliance** over the validated `kshana::allan::mtie` estimator (with a
  conservative ±kσ band), a named clock **device-preset** library, and the optical-lattice
  **systematic-shift** budget.
- **`report`** (A11) — one renderer (Markdown / HTML / SVG / JSON) over the figures of
  merit of one open run + trade-study evidence + RF-impairment evaluation,
  MODELLED/VALIDATED labels carried through.
- **`pro_study` · `study_ledger` · `report_pdf` / `report_brand` / `report_pack_a` /
  `report_pack_c`** — the study → delivery pipeline. `pro_study` composes one branded
  cover + a single MODELLED banner + N pack sections (with the `report_brand` furniture);
  `report_pdf` renders a pure-Rust PDF; `study_ledger` appends a hash-chained,
  tamper-evident delivery record (request → run → delivered; `verify()` catches any
  alteration); `report_pack_a` / `report_pack_c` render the Pack A / Pack C study faces.
- **`benefits`** (A02 / D1.1) — benefit-vs-baseline ratio engine over the trade-study
  core, with dual MODELLED/VALIDATED provenance labels and no auto-upgrade to VALIDATED.
- **`connectors`** (PR21) — STK `.e` (km, J2000/GCRS) + universal CSV exporters and a CI
  metric gate (dot-path metric, non-zero exit).
- **`serve`** (PR22) — a dependency-light, std-only blocking HTTP/1.1 service
  (`GET /health` · `/version` · `/kinds`; `POST /run` · `POST /study`) for on-prem /
  air-gapped / sovereign / hosted deployment, entitlement-gated and **fail-closed**
  (open engine + free seam-check at Evaluation until a license token is supplied).
  With `--data-dir` it is also an **on-premises job service**: a job queue on disk,
  a bounded worker pool (`--workers`), the operations `run`, `dossier` and `ci`,
  results and artefacts stored per job, a hash-chained delivery ledger
  (`GET /ledger/verify`), unfinished jobs queued again after a restart, an OpenAPI
  document generated from the route table, a job board page and an optional bearer
  token (`--api-token-file`). See [`docs/SERVICE.md`](docs/SERVICE.md).
- **Pro power summary** — what each of the six Pro power features (design optimiser,
  uncertainty, mission dossier, campaign watch, spectrum coexistence, job service) does, its inputs and
  outputs, and what is honestly not done: [`docs/PRO-POWER.md`](docs/PRO-POWER.md).
- **`license`** (PR7, F10b–F10f) — the tier mechanics: Ed25519-signed tokens (public key only in the build), fail-closed Evaluation
  default, an explicit tier for every Pro kind and command (a test enforces it); token v2
  (products, seats, Stations, machine binding, key rotation, not-before), offline
  activation with grace period and signed revocation lists, and the vendor issuance
  register. See [`docs/LICENSING.md`](docs/LICENSING.md).
- **`payments`** (F10g) — merchant-of-record webhooks (Paddle, Lemon Squeezy) that issue,
  renew and revoke tokens, and the paywall; switched off by default, prices
  `PLACEHOLDER`. See [`docs/PAYMENTS.md`](docs/PAYMENTS.md).
- **`signed_dossier`** (F7) — `kshana-pro dossier sign|verify`: every file of a dossier
  under one Ed25519 signature, verified offline.

### What is NOT built on spec (gated)

- A real **customer-device clock twin** — the framework swaps *published* Allan points
  for NDA characterisation data; `twin::customer_instance_gated` guards it.
- All of **L2 resilience** — real threat data is export-controlled; the module stays a
  fail-closed seam guard until the export assessment clears.
- The productised **ECSS/MBSE SKU** — the MBSE framework is Arksa-exclusive since X3 and
  is not in this crate.

No gated *capability* is built on spec — only the *frameworks* (runnable on public
data) and the L2 guard.

### What the embedded open engine now carries (depended-on, not duplicated)

Because Pro is a superset *by construction*, every advance in the open core is present
here too. Recent open-core capability the bid evidence relies on:

- **The three quantum-PNT demonstrator verticals** (AO/1-13503) — trusted quantum
  **timing** (optical-clock + entanglement time-transfer), **GNSS-free quantum
  navigation** (cold-atom interferometer vs nav-grade INS), and **fault/anomaly
  detection** — each a runnable scenario, bundled into one reproducible study. Capability
  is open; the bid framing is private (`docs/tenders/13503-quantum-pnt/QUANTUM-DEMONSTRATOR-CAPABILITY.md`).
- **The representativeness + gaps-to-flight ledger** (`representativeness.rs`) — a
  machine-checked record on every demonstration (external anchors · modelled assumptions ·
  gaps-to-flight · representative TRL band). This *is* the Clar 1/A3 compliance mechanism.
- **The validated-island matrix** (`verification.rs`, honesty-gated), expanded under D2
  with four new real-data islands: a **real caesium clock** and **Stable32 PHASE.DAT**
  (Allan estimators), **real ESA OPS-SAT telemetry** (anomaly ROC AUC vs scikit-learn),
  and **ICGEM/GRS80 gravity-functional synthesis** (gravity-aided nav map). Everything
  else stays honestly **MODELLED**.
- **The seven lunar-PNT modules** (lunar time, VLBI, joint OD+clock, frame realisation,
  nav service, differential PNT, interop export) backing the lunar shelf assets.

No TRL > 3, no flight heritage, no certification, no ESA endorsement — the same honesty
posture the open core enforces.

## Build & run

A **release** build depends on the **published open-core crate** (`kshana = "<version>"`),
resolved to an exact version in `Cargo.lock` and advanced with `cargo update -p kshana` as
each open-core release lands.

> **Kshana Pro 0.2.0 on open engine 0.29.0. Development pin on branch `feat/pro-power`
> (not a release pin).** Open engine 0.29.0 (75 scenario kinds, counted with
> `cargo run -- kinds --json`) is not on crates.io yet, so this branch builds against a
> dedicated, read-only engine worktree by path:
> `kshana = { path = "../kshana-wt-pro-engine", version = "0.29" }`, which is branch
> `feat/pro-engine-base` of the open repository, fast-forwarded to the 0.29.0 release tree.
> **Release step, not done:** after the crates.io publish, replace the path with
> `kshana = "0.29.0"`, run `cargo update -p kshana`, and re-run every check below with
> `--locked`. A path pin builds only on a machine that has the worktree; a release must
> not depend on it.

It used to track a **git dependency on the open core's `main` branch**, pinned to
`version = "0.24"`. That is why it no longer does: a branch pin resolves to whatever the
branch points at, so when the open core's history was rewritten the pinned commit stopped
being reachable from `refs/heads/main` — proven with a clean clone, which did not receive
it. GitHub still served the object by raw SHA, so nothing failed loudly, but the manifest
demanded a version the branch could no longer supply. A published version is immutable and
cannot drift that way.

For local co-development, override with a **path dep** to a sibling `../kshana` checkout
(see the comments in [`Cargo.toml`](Cargo.toml)). Pro never forks the engine — it embeds
it, so the two versions move together.

```bash
cargo test                       # unit + integration tests (proves the overlay)
cargo run -- kinds               # list open kinds (inherited) + Pro kinds + their tier
cargo run -- kinds --json        # the same as JSON, with open_count and pro_count
cargo run -- version
```

The suite is unit tests across every module plus **one hundred and thirteen integration tests** in
[`tests/`](tests) that prove the contract: `overlay.rs` (superset by construction),
`open_engine_reachable.rs` (every open kind the embedded engine lists is run from one of
the engine's own scenario files through the Pro dispatcher and the open ∪ Pro registry,
and must give the open engine's result byte for byte), the study/report/pack path
(`pro_study.rs`, `report_cover.rs`, `report_pdf.rs`, `study_ledger.rs`,
`study_intake.rs`, `pack_c.rs`, `optimism_gap_scenarios.rs`, `study_dossier.rs`),
the Pro power features that stay in this crate (`uncertainty.rs`, `campaign_watch.rs`,
`service_jobs.rs`, each checked against independent open-engine runs, closed forms or
real sockets; the design optimiser, mission dossier and spectrum coexistence tests moved
with their code), the `*_in_sync.rs` guards (each committed deliverable matches its generator output, so
docs never drift), `modelled_label.rs` (every Pro kind's result and every command's
result document carries a top-level `label` that starts with MODELLED, the open engine's
own form), `ashforde_brand.rs` and `ashforde_sample.rs` (every document is an Ashforde
document: the seal watermark on every PDF page and in every HTML report, the embedded
type, the footer and page numbers, nothing fetched; the SAMPLE mark only with
`--sample`), `readme_counts.rs` (the pack and integration-test counts in this
README match the registry and the `tests/` directory),
`changelog_tracks_engine_pin.rs` (the changelog names the pinned open-engine version),
`licensing.rs` (licence tokens v2, offline activation, grace period, revocation lists
and the issuance register end to end), `payments.rs` (the payments webhook and paywall
in test mode: forged, replayed and out-of-order webhooks refused; off by default),
`signed_dossier.rs` (a signed dossier verified offline, also by an independent Python
verifier), `service_hardening.rs` (cancellation, time limits, priority, the event stream,
TLS, failover between two nodes, backup and restore, a concurrent load),
`free_build_check.rs` (the free-build check finds Pro code in a Pro binary),
`offline_bundle.rs` (the air-gapped bundle signed, checked and installed offline; the
Debian package), `station_fleet.rs` (the Pro Station fleet: one Station without the fleet feature, the
licence's Station count with it), `service_trial.rs` (the open trial sandbox: Evaluation tier, a tenant per caller,
quotas, 14-day trial tokens, trial jobs deleted after their retention),
`licence_kms.rs` (licence, revocation-list and pack signing through AWS KMS against a fake
KMS endpoint, byte for byte equal to the seed signer; runs with `--features kms`),
`hosted_service.rs` (the results-only API and the hosted service for secret methods: signed
verdicts only, watermark, monthly allowances, probing alerts, chained access log),
`price_list.rs` (the editions page with the public price switch off and the private price
sheet), `attack_iq.rs` (synthetic attack IQ guardrails: licence, vetted buyer, attestation,
files only, watermark and log), `pack_c_subscription.rs` (a Pack C subscriber from the purchase webhook to a gated CI run,
a renewal and a refund),
`lab_campaign_load.rs` (the lab-campaign loader verifies cell keys, the DIGEST and the
schema on a synthetic campaign, and refuses tampered input), and
`lab_campaign_report.rs` (the lab campaign report traces every printed number to the
cells, labels MEASURED and MODELLED, and enforces the data class),
`lab_campaign_no_client_data.rs` (no client campaign data or SigMF recording anywhere in
the tree; export-control item N6).

**Licensing (PR7).** `run` is **fail-closed**: with no `--license` it enforces the
**Evaluation** tier (open scenarios + the free `pro-seam-check` only). A value pack
needs a license token (or `--dev` for local runs in a debug build; a release build
refuses `--dev`). Open scenarios are always free.

```bash
# Open scenario — always free, no license needed:
cargo run -- run ../kshana/scenarios/ephemeris.toml         # [routed to: open engine]

# Local exploration of Pro packs without a license:
cargo run -- run scenarios/twin-csac.toml --dev            # L1 twin (PASS)
cargo run -- run scenarios/trade-clock.toml --dev          # L3 trade + evidence pack

# The real licensed flow — issue, then run under that tier:
TOK=$(cargo run -q -- license issue --tier pro --licensee "ACME Space" --days 90)
cargo run -- license verify "$TOK"                          # shows entitled Pro kinds
cargo run -- run scenarios/twin-csac.toml --license "$TOK"  # allowed (Pro tier)
cargo run -- run scenarios/batch-clock-mc.toml --license "$TOK"  # DENIED (Enterprise-only)
```

Tiers: **Evaluation** (free `pro-seam-check`) → **Pro** (`pro-twin`,
`pro-trade-study`, `pro-quantum-systematics`, `pro-optimism-gap-audit`,
`pro-scenario-ci`) → **Enterprise** (+ `pro-batch`, `pro-uncertainty`). Tokens are
Ed25519-signed: `license issue` signs with `$KSHANA_PRO_SIGNING_KEY` (vendor only),
and a build verifies with the public key compiled in from `$KSHANA_PRO_LICENSE_PUBKEY`
(`license keygen` makes the pair). Debug builds fall back to a demo key pair so the
commands above work locally; release builds contain no demo key, and one built
without a public key runs every Pro kind at Evaluation tier.

**Connectors & CI (PR21).** Export an ephemeris to enterprise tools, or gate a
metric in CI. The open core emits CCSDS OEM/SP3 (GMAT/Orekit) and, since 0.29, STK `.e`
for any scene (`kshana --export stk`); Pro keeps its one-step STK `.e` from an `ephemeris`
result (km, J2000/GCRS), adds universal CSV, and a metric gate with a CI exit code.

```bash
cargo run -- export ../kshana/scenarios/ephemeris.toml --format stk > orbit.e
cargo run -- export ../kshana/scenarios/ephemeris.toml --format csv > orbit.csv
# CI gate: nonzero exit fails the pipeline (dot-path supports array indices):
cargo run -- gate ../kshana/scenarios/ephemeris.toml --metric samples.0.alt_km --op gt --threshold 400
```

**Deliverable & deployment subcommands.** Beyond `run` / `export` / `gate`, the
overlay exposes the study-deliverable and service surface (full list: `kshana-pro`
with no args, or see [`src/cli/mod.rs`](src/cli/mod.rs) `usage()`):

```bash
cargo run -- study --dossier scenarios/dossier-holdover-csac.toml --out out/ --dev  # study dossier: HTML + JSON technical note (Pro)
cargo run -- report scenarios/trade-clock.toml --format html # A11 consolidated report (md|json|html|svg)
cargo run -- report scenarios/trade-clock.toml --pdf out.pdf # A11 pure-Rust PDF export (report_pdf)
cargo run -- ci --pack <manifest.toml> --baseline ci.json --out report.html --json ci.json  # Pack C scenario-CI regression gate (Pro)
cargo run -- benefits --json                                # A02/D1.1 benefit-vs-baseline table (dual provenance labels)
cargo run -- serve --addr 127.0.0.1:8088                    # PR22 HTTP service — GET /health /version /kinds · POST /run /study, entitlement-gated
```

**Study dossier (the paid unit).** `study --dossier <scenario.toml> --out <dir>` turns
one scenario into a reproducible evidence dossier laid out like a European Space Agency
(ESA) technical note: identification, scope, applicable and reference documents, method,
results, limitations. It writes `dossier.json`, `dossier.html` and `scenario.run.toml`
(the exact text run) and prints the dossier's SHA-256 result hash; the same scenario
gives the same hash and, without `--utc`, byte-identical files. The results carry the engine and Pro
versions, the engine's scenario hash, a VALIDATED or MODELLED tier on every figure (read
from the open verification matrix), a one-at-a-time sensitivity sweep, a holdover band
and maximum time interval error (MTIE) check for `clock` scenarios, and the Goal
Structuring Notation (GSN) argument, Verification Control Document (VCD) and W3C-PROV
provenance record from the L3 stack. It also maps the run's behaviour to the four levels
of the US Department of Homeland Security (DHS) Resilient PNT Conformance Framework (RPCF)
v2.0 and gives Prevent / Respond / Recover readings for later mapping to IEEE P1952, all
labelled **tentative, simulation-derived, not a certified level**. The optional
`[dossier]` block in the scenario sets the title, subject, figures, sweep and holdover
settings; `scenarios/dossier-holdover-csac.toml` is the worked example. There is no PDF
face for the dossier yet. Pro tier.

The Pro packs (runnable examples in [`scenarios/`](scenarios)):
- `scenarios/twin-csac.toml` — `pro-twin`: calibrates a clock twin to a representative
  CSAC ADEV budget; validates it reproduces the curve within tolerance.
- `scenarios/trade-clock.toml` — `pro-trade-study`: trades an optical-lattice vs a CSAC
  architecture by timing holdover; emits the ranked evidence pack (`evidence_pack_markdown`).
- `scenarios/quantum-systematics-sr.toml` — `pro-quantum-systematics`: an Sr optical-clock
  systematic-shift budget (coefficients supplied in the scenario, never baked in).
- `scenarios/batch-clock-mc.toml` — `pro-batch`: a Monte-Carlo ensemble of a clock scenario
  across many seeds (Enterprise tier).
- `scenarios/dossier-holdover-csac.toml` — `study --dossier`: an open `clock` scenario
  with a `[dossier]` block; the chip-scale atomic clock's holdover through a 110-minute
  GNSS denial, written up as a study dossier.
- `scenarios/pro-power/uncertainty/uq-*.toml` — `kshana-pro uncertainty`: five worked
  studies (link margin, clock holdover over seeds and over an uncertain noise level, the
  low-Earth-orbit positioning chain with regression and with Sobol' sensitivity).
- `scenarios/packs/*/pack.toml` — `kshana-pro ci`: five campaign-watch subscription packs
  (low Earth orbit, constellations, spectrum, campaigns, the solar system), each with a
  `PROVENANCE.md`; `scenarios/pro-power/watch-demo/` holds a six-revision demonstration
  sequence and its script `run-demo.sh`.
- `tests/fixtures/service/*.toml` — public open-engine scenarios, with provenance, that the
  job service's acceptance test submits through `POST /jobs`.

## Deployment

The overlay ships a container and two compose topologies for on-prem / air-gapped /
sovereign delivery — full notes in [`deploy/DEPLOY.md`](deploy/DEPLOY.md).

```bash
# Build the image — the context MUST be the PARENT dir so the `../kshana` path
# dependency resolves (the Dockerfile COPYs BOTH the open core and the overlay):
docker build -f kshana-pro/Dockerfile -t kshana-pro ..

# Run it — fail-closed by default (open engine + free seam-check at Evaluation):
docker run -p 8088:8088 kshana-pro
# Unlock Pro/Enterprise with a license token:
docker run -p 8088:8088 kshana-pro serve --addr 0.0.0.0:8088 --license <token>
```

The runtime image is a non-root `debian-slim` that `EXPOSE`s **8088** and carries a
`HEALTHCHECK` calling `kshana-pro healthcheck` against `/health`, so a hung server is
reported unhealthy. Two compose files cover the modes —
[`deploy/docker-compose.product.yml`](deploy/docker-compose.product.yml) (hosted
product) and [`deploy/docker-compose.sovereign.yml`](deploy/docker-compose.sovereign.yml)
(air-gapped / sovereign). Because `serve` is fail-closed, an image started with no
license still answers for the open engine and `pro-seam-check` only — never a paid pack.

## Discipline (from the open-core strategy)

- **Never fork** the open core — depend on it, extend via seams.
- **Never starve** the open core to upsell Pro (it is the funnel).
- **Never open** the moat: export-controlled data, customer calibrations, Pro IP.
- **Don't build gated *capability* on spec** — the L1/L3 *frameworks* are built and
  run on public data (a BD/bid calling card), but the gated *instances* (a customer
  device twin, a sold SKU) and **all of L2** (legal wall) wait on their triggers.
- **No GPL/copyleft third-party deps** (the open core is AGPL-3.0-only / commercial
  dual-licensed by design; a copyleft *dependency* would break the dual-licence model + ESA terms).
- **Never commit** NDA / customer / export-controlled material (see `.gitignore`).
- **Push discipline:** capability *code* mirrors to the open `kshana` repo; **bid evidence,
  papers, strategy, commercial & legal material stay in this private repo**. Nothing —
  public or private — is pushed without explicit founder go.

## Trademark & contact

"Kshana" is a trademark of Ashforde OÜ. A fork of the open engine may use the
AGPL-3.0-only code; it may not use the name or the canonical distribution.

---

*Ashforde OÜ · all external/business contact: **contact@ashforde.org** · Kshana —
open AGPL-3.0-only, commercial licence available · PRIVATE overlay: do not distribute.*
