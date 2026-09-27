<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Export-control classification request pack

**Owner:** Ashforde OÜ · **Status:** ready for export-control counsel · **Date:** 2026-09-25

> ## NOT A DETERMINATION
>
> **This document does not classify anything and is not legal or export-control
> advice.** It gives a factual description of the software for a classification
> request, lists control-list entries that an adviser or authority should **assess**,
> with the reason each is on the list, and sets out the questions for the competent
> authority. Being listed below does **not** mean an item is controlled; being left off
> does **not** mean it is not. Until a documented determination exists, the Pack B hold
> in `legal/EXPORT-CONTROL-DETERMINATION.md` section 5 stands.

Abbreviations: PNT (positioning, navigation and timing), GNSS (Global Navigation
Satellite System), GPS (Global Positioning System), API (application programming interface), TEXBAT (Texas Spoofing Test Battery, a public spoofing data set), INS (inertial navigation system), CRPA (controlled reception pattern
antenna), MVDR (minimum-variance distortionless response, a beamforming method), OSNMA
(Galileo Open Service Navigation Message Authentication), TESLA (Timed Efficient Stream
Loss-tolerant Authentication), RAIM (receiver autonomous integrity monitoring), MTIE
(maximum time interval error), AGPL (GNU Affero General Public License, version 3), HMAC
(hash-based message authentication code), Ed25519 (the Edwards-curve digital signature
algorithm on Curve25519), SHA-256 (Secure Hash Algorithm, 256-bit),
AES (Advanced Encryption Standard), BAFA (Bundesamt für Wirtschaft und Ausfuhrkontrolle,
Germany's Federal Office for Economic Affairs and Export Control), AzG (Auskunft zur
Güterliste, BAFA's goods-list classification opinion), ELAN-K2 (BAFA's electronic
application system), AWV (Außenwirtschaftsverordnung, the German Foreign Trade and
Payments Ordinance), ESA (European Space Agency), EU (European Union), TOML (Tom's
Obvious Minimal Language, a configuration file format), JSON (JavaScript Object
Notation), PDF (Portable Document Format), HTML (HyperText Markup Language).

This pack builds on, and does not replace, `legal/EXPORT-CONTROL-ASSESSMENT.md`
(the internal self-screen) and `legal/EXPORT-CONTROL-DETERMINATION.md` (the Pack B
questionnaire). It answers most of that questionnaire's section 3A and 3B from the code.

---

## 1. The finding that changes the plan: which authority?

The existing drafts assume **BAFA** because ESA recorded the company's operative
nationality as German. Two facts, checked 2026-09-25, cut against that:

1. **Ashforde OÜ is established in Estonia** (registry code 17321180; registered office
   in Tallinn). Source: the company's published legal notice,
   <https://ashforde.org/legal>. The e-Business Register page could not be read by a
   script that day; re-check by hand.
2. **BAFA does not issue an AzG to a foreign company, or for technology.** BAFA's AzG
   page lists, among the cases where no AzG is issued: "Der Antragsteller ist eine
   Bank, Spedition, ausländische Firma/Institution" (the applicant is a bank, freight
   forwarder, or foreign company or institution) and "Es handelt sich um Technologie"
   (it concerns technology). The same page says an AzG is issued on a form in ELAN-K2
   and "ist zwei Jahre gültig" (is valid for two years).
   Source: <https://www.bafa.de/DE/Aussenwirtschaft/Ausfuhrkontrolle/Antragsarten/Auskuenfte_zur_Gueterliste/auskuenfte_zur_gueterliste_node.html>,
   read 2026-09-25.

Under the EU Dual-Use Regulation, export authorisations are granted by the competent
authority of the Member State where the exporter is resident or established (Regulation
(EU) 2021/821; article wording to be confirmed on
<https://eur-lex.europa.eu/eli/reg/2021/821/oj/eng>, which could not be read by a script
on 2026-09-25). For an Estonian company that points to Estonia's **Strategic Goods
Commission**, the inter-agency licensing body at the Ministry of Foreign Affairs
(<https://www.vm.ee/en/activity/control-strategic-goods>; licence applications at
<https://www.vm.ee/en/activity/control-strategic-goods/application-license-strategic-goods>).
The legal basis is Estonia's Strategic Goods Act
(<https://www.riigiteataja.ee/en/eli/511072014011/consolide>).

**So the first question for counsel is question E1 below**, before anything is filed
anywhere. BAFA stays relevant if a German-established entity later exports or
contracts, or if a German customer asks for its own classification.

## 2. What the software is

### 2.1 The open engine: Kshana

- **What it is.** A deterministic simulation and analysis library and command-line
  program for the resilience of positioning, navigation and timing. It is written in
  Rust. A user describes a scenario in a TOML file (threat, sensor or clock set,
  budget); the engine computes figures of merit and writes JSON, HTML or charts. Each
  result is reproducible from scenario + random seed + engine version.
- **Version and licence.** 0.28.0, licensed AGPL-3.0-only with a commercial option.
  *(Revised 2026-09-27: this pack first described 0.27.2. Between the two releases the
  engine added the telecom timing, slot timing and orbital timing modules listed below and
  no new dependency; no cryptographic code was added.)*
- **Where it is public (checked 2026-09-27).** Public GitHub repository
  `ashfordeOU/kshana` (GitHub API: not private, licence AGPL-3.0); crates.io crate
  `kshana`, latest 0.28.0; Python Package Index project `kshana`, 0.28.0; npm package
  `kshana`, latest 0.28.0, licence AGPL-3.0-only. Anyone may download, copy and
  redistribute it without asking.
- **What it computes (module names from the 0.28.0 source):**
  - clock and holdover models: Allan deviation, MTIE, holdover time to an error
    budget (`allan`, `holdover`, `clock_state`, `clock_specs`);
  - timing against a requirement: time error, MTIE and TDEV against published ITU-T
    telecom masks (`telecom_timing`); how long a clock stays inside a scheduled slot's
    guard interval after losing its reference (`slot_timing`); and a timing protection
    level for a satellite cross-checked by ground contacts or crosslinks
    (`orbital_timing`);
  - GNSS measurement simulation, position solutions and dilution of precision
    (`gnss_sim`, `pvt`, `rinex`, `sp3`);
  - integrity monitoring: RAIM variants and protection levels (`raim`, `integrity`,
    `hybrid_integrity`, `tpl`);
  - inertial coasting and GNSS/INS fusion in simulation (`inertial`, `fusion`,
    `hybrid`, `kalman`, `particle_filter`);
  - **interference effects as link budgets**: jammer-to-signal ratio to effective
    carrier-to-noise density to loss of lock, following a textbook chain
    (Kaplan & Hegarty, *Understanding GPS/GNSS*, 3rd ed.) (`jamming`, `linkbudget`);
  - **spoofing detection and capture** models (`spoof`, `spoof_detect`,
    `spoof_monitors`, `spoof_capture`);
  - **CRPA anti-jam beamforming weights**: deterministic null-steering and MVDR weights
    for an N-element antenna array (`crpa`);
  - **navigation message authentication budget sizing** for an OSNMA/TESLA-style
    scheme: overhead, latency and forgery-resistance figures (`nma_budget`);
  - a threat-parameter catalogue in which every input is labelled MODELLED and traced
    to a public source, including the public JammerTest 2024 campaign and the TEXBAT
    spoofing data set (`conflict_threat_params`);
  - quantum-sensor versus classical trade studies from published device figures
    (`quantum_trade`, `quantum_devices`);
  - orbit determination, lunar and cislunar navigation and time-transfer analysis.
- **What it does not do:**
  - it does **not** transmit, receive or process radio signals; it contains no
    receiver, no signal generator, no jammer and no spoofer;
  - it does **not** run on or control any hardware, and it is not firmware;
  - it does **not** decrypt any GNSS signal and has no access to any encrypted or
    governmental service (for example Galileo's Public Regulated Service or GPS
    military codes);
  - it does **not** contain real adversary waveforms or non-public threat data; threat
    parameters are representative values from cited public sources;
  - it does **not** encrypt data. Cryptographic primitives present: SHA-256 hashing
    (dependency `sha2`) used to fingerprint scenarios and results, and a ChaCha-based
    pseudo-random number generator (dependency `rand_chacha`) used for reproducible
    seeded simulation. The OSNMA module only sizes a scheme's budget: in the 0.28.0
    source it imports no cryptographic code.

### 2.2 The private overlay: Kshana Pro

- **What it is.** A proprietary Rust program and library that depends on the published
  engine and adds study workflows and report generation. It is **not public** and is
  not distributed today.
- **What it adds (from `src/` in this repository):**
  - device "twins": calibration of a clock model to a published or measured Allan
    deviation budget, with presets for oscillator and atomic-clock classes built from
    published datasheets (`twin`, `twin/presets`);
  - banded holdover prediction and checks against time-error and MTIE masks
    (`twin/holdover_uq`);
  - study packs (for example the optimism-gap audit, trade studies, conflict
    resilience, lunar time budget) and branded HTML/PDF study dossiers with a
    tamper-evident, hash-chained study ledger (`packs`, `report*`, `study_ledger`);
  - a licence-entitlement check using Ed25519-signed tokens (`license`; HMAC-SHA256
    until 2026-09-26);
  - an "L2" resilience layer, which today is a stub. **Real threat libraries,
    adversary waveforms and anti-jam or anti-spoof performance data are not in the
    code and must not enter this repository** (`legal/EXPORT-CONTROL-ASSESSMENT.md`
    section 3).
- **Cryptographic primitives present:** SHA-256 (ledger and fingerprints), Ed25519
  digital signatures (licence tokens: authentication and integrity only; they replaced
  HMAC-SHA256 on 2026-09-26). A PDF library dependency
  (`lopdf`) pulls in AES and cipher-block-chaining code, which PDF uses for document
  encryption; whether any Pro code path calls it has not been established (question A3).

### 2.3 The deliverables

- **Study dossiers** (HTML, PDF, JSON) for a named customer: modelled results, with a
  VALIDATED or MODELLED label and provenance on each figure. Example: the timing holdover
  sizing study (`commercial/offers/cni-timing-holdover-study.md`), whose inputs are the
  customer's clock logs and published oscillator datasheets.
- **Technical assistance**: scoping calls, interpretation of results, and review of the
  dossier with the customer or its test house.

### 2.4 The public-domain position, stated as facts

- The engine source, build and documentation are published on the four channels in
  section 2.1 without any restriction on further dissemination beyond the copyright
  licence (AGPL).
- The methods behind two study lines are published as arXiv preprints, not
  peer-reviewed: arXiv:2606.22054 (optimism gap in interference detectors) and
  arXiv:2606.24210 (conditional timing protection level under spoofing).
- Kshana Pro, its presets, report templates, study know-how and any customer
  deliverable are **not** public.

## 3. Candidate control-list entries to assess

The list to test against is **Annex I of Regulation (EU) 2021/821** as in force on the
filing date. The Commission announced its 2025 update on 8 September 2025
(<https://policy.trade.ec.europa.eu/news/2025-update-eu-control-list-dual-use-items-2025-09-08_en>);
a law-firm note reports that it came into effect on 15 November 2025 after publication
in the Official Journal
(<https://www.cooley.com/news/insight/2025/2025-12-05-eu--issues-2025-update-to-dual-use-control-list>),
and search summaries give its number as Commission Delegated Regulation (EU) 2025/2003
(not confirmed on a primary page). Counsel must confirm both on EUR-Lex and check for
any later update. Entry descriptions
below are summaries **to be verified against the list in force**; they are not
quotations.

| # | Entry to assess | Why it is on this list | What would take it off |
|---|---|---|---|
| 1 | **General Software Note and General Technology Note; definition of "in the public domain"** | Decides whether the published engine is outside control whatever else applies. The engine is AGPL-licensed; the question is whether copyright licence terms affect "public domain" status. | A written confirmation that the published engine is "in the public domain" for Annex I purposes. |
| 2 | **Category 7, software (7D) for navigation equipment** | The engine and Pro include inertial coasting, GNSS/INS fusion and integrity monitoring. Entries in 7D cover software specially designed or modified for controlled navigation equipment, or to improve its performance to controlled levels. | Confirmation that simulation and analysis software that does not run in, or improve, a navigation system is not "specially designed" for one. |
| 3 | **Category 7, GNSS receiving equipment with adaptive antennas (7A), and related software (7D) and technology (7E)** | The `crpa` module computes null-steering and MVDR weights for an antenna array, the kind of processing adaptive-antenna anti-jam receivers use. | Confirmation that a textbook beamforming-weight calculator used in simulation is not software for the development, production or use of such equipment. |
| 4 | **Category 7 technology (7E) and technical assistance** | Study dossiers and consulting explain navigation and timing performance. Technology "required" for controlled items is controlled even when no software is shipped. | Confirmation that modelled figures of merit from public-parameter scenarios are not "required" technology. |
| 5 | **Category 3, atomic frequency standards (3A) and related software (3D) and technology (3E)** | Pro calibrates twins of atomic clocks and oscillators from datasheets and predicts their holdover; the engine carries clock presets for atomic standards. | Confirmation that modelling published clock performance is not development or production technology for a controlled frequency standard. |
| 6 | **Entries added in the 2025 update for quantum technologies** | The engine and Pro compare quantum sensors and clocks with classical ones. The Commission's notice says the update adds "controls related to quantum technology", giving as examples quantum computers, electronic components designed to work at cryogenic temperatures, parametric signal amplifiers and cryogenic cooling systems. | Confirmation that no added entry covers trade-study software for quantum PNT sensors. |
| 7 | **Category 5 Part 2, information security (5A002, 5D002)** | Cryptographic primitives are present (section 2). Digital signatures and hashing are for authentication and integrity; a PDF library carries AES code. | Confirmation that the primitives present fall outside the confidentiality-cryptography controls, and whether an unused library routine matters. |
| 8 | **Category 5 Part 1, telecommunications** | The first commercial study targets telecom timing and synchronisation. | Confirmation that timing analysis software for network operators is not telecom equipment software under Part 1. |
| 9 | **EU Common Military List: military electronic equipment including GNSS jamming (ML11), software (ML21) and technology (ML22)** | The engine models jamming and spoofing effects and anti-jam processing. Military-list items are licensed nationally. | Confirmation that the software is not "specially designed" for military use and that public-parameter effect models are not military technology. |
| 10 | **National lists**: Estonia's list under the Strategic Goods Act; Germany's Export List Part I Section B (national dual-use entries), if a German entity is involved | National entries can go beyond Annex I. | A statement that no national entry applies. |
| 11 | **End-use ("catch-all") controls and technical-assistance controls in Regulation (EU) 2021/821** | Apply to unlisted items when the exporter knows or is informed of a military end use in an embargoed destination, or a weapons-of-mass-destruction or missile end use, and to technical assistance. | Not removable by classification; handled per deal by screening (section 5). |

## 4. Questions for the authority and for counsel

Questions are grouped so they can be lifted into a letter. Letters refer to the
existing questionnaire in `legal/EXPORT-CONTROL-DETERMINATION.md` where they overlap.

**E. Which authority**

- **E1.** For Ashforde OÜ, established in Estonia with an ESA-recorded operative
  nationality of Germany, which Member State's competent authority grants export
  authorisations and gives classification opinions: Estonia's Strategic Goods
  Commission, BAFA, or both for different transactions?
- **E2.** Does the Strategic Goods Commission give a written classification opinion (an
  equivalent of BAFA's AzG) on request, for software and for technology? In what
  form, how long does it take, and how long is it valid? (Its licence page states a
  processing time for licence applications; that is not the same instrument.)
- **E3.** If BAFA is relevant for a German-established entity, is an AzG the right
  instrument given that it is not issued for technology, and how should the
  technology and technical-assistance elements be handled instead?

**A. The product (answers above; confirm)**

- **A1.** Is the description in section 2 accurate and complete enough to classify?
  What further evidence (source listing, module documentation, sample outputs) is
  needed?
- **A2.** Confirm the software contains no hardware, firmware, signal processing of
  live radio data, real threat data or decryption of GNSS services.
- **A3.** Do the cryptographic elements in section 2 (hashing, HMAC tokens, an AES
  routine inside a PDF library) bring the software within
  Category 5 Part 2 or its notes?

**B. Public domain**

- **B1.** Is the published AGPL engine "in the public domain" for Annex I purposes
  despite its copyright licence, and does that cover every module listed in section
  2.1, including `crpa`, `jamming`, `spoof*` and `nma_budget`?
- **B2.** Do the arXiv preprints put the corresponding methods in the public domain,
  and for which elements exactly?
- **B3.** Is Kshana Pro, which is not public, separately classifiable, and do any of
  entries 2 to 9 in section 3 apply to it?
- **B4.** Could combining public elements into a non-public study dossier create
  something separately controlled?

**C. Deliverables and assistance**

- **C1.** Are study dossiers built from public-parameter scenarios and a customer's own
  clock logs "technology" under Annex I? Does the answer change when the customer's
  inputs describe an atomic clock or a network timing architecture?
- **C2.** Does scoping, interpretation or review with a customer, or its test house,
  outside the EU amount to controlled technical assistance?

**D. Destinations and screening**

- **D1.** For deliveries inside the EU, is any authorisation needed for listed items,
  and which (for example for items listed in Annex IV)?
- **D2.** For the United Kingdom, Switzerland and other likely destinations, is an EU
  general export authorisation available if an item is listed?
- **D3.** What screening and record-keeping should be in place before the first sale?
  (Section 5 is the proposed minimum.)

## 5. Operating rules until a determination exists

These restate, and do not loosen, `legal/EXPORT-CONTROL-ASSESSMENT.md` section 3.

1. The Pack B hold stands: no marketing, pricing, delivery or technical assistance for
   the timing holdover study until a documented determination exists.
2. The public engine stays as it is; nothing in this pack pauses it.
3. No real threat data, adversary waveforms, or anti-jam or anti-spoof performance
   data enter any repository.
4. Every prospective customer is screened against the EU consolidated sanctions list
   before any non-public material is shared, and the check is recorded.
5. Record each determination in the "Record of determinations" table in
   `legal/EXPORT-CONTROL-ASSESSMENT.md`.

## 6. What to send

1. This pack, with the NOT A DETERMINATION box kept.
2. `legal/EXPORT-CONTROL-ASSESSMENT.md` and `legal/EXPORT-CONTROL-DETERMINATION.md`.
3. The public engine's `README.md` and `LICENSING.md` (public repository).
4. The module documentation headers of the modules named in section 2 (the source is
   public for the engine; for Pro, send headers only, not source, unless counsel asks
   under confidentiality).
5. A sample dossier generated from public data only.

## 7. Record

| Date | Step | By | Outcome |
|---|---|---|---|
| 2026-09-25 | Pack prepared; BAFA AzG exclusion for foreign companies found | Ashforde OÜ | Authority question E1 opened |
