<!--
PRIVATE — Ashforde internal. kshana-pro repo only; never publish in the open kshana repo.
Master tracker and bid decision memo for ESA AO/1-13695 (NAVISP EL1-132). Contact: contact@ashforde.org
-->

# AO/1-13695 · NAVISP EL1-132 · Flexible eLoran/GNSS signal simulator: bid decision memo

**Status (19 Nov 2026): CONDITIONAL GO.** Work the eligibility and partner gates now. The bid/no-bid
call is at the **gate review on Mon 7 Dec 2026**, with a hard stop on **Mon 14 Dec**. If a gate fails,
switch to Plan B (Kshana Pro engine licensed or subcontracted to an eligible prime). Do not write the
full proposal before gates G1–G3 are green.

This file is the single source of truth for the pursuit. Companion files in this folder:
[`compliance-matrix.md`](compliance-matrix.md) · [`technical-proposal-draft.md`](technical-proposal-draft.md) ·
[`wp-schedule-cost-skeleton.md`](wp-schedule-cost-skeleton.md) · [`risk-register.md`](risk-register.md) ·
[`clarification-questions.md`](clarification-questions.md).

## 1. The tender (what we know, and from where)

| Item | Value | Source |
|---|---|---|
| Title | NAVISP EL1-132 Development and Validation of Flexible eLoran/GNSS Signal Simulator (EXPRO+) | esa-star snapshot 15 Nov 2026, tender 194887 |
| Tender action | 1-13695 · Open Competition · ESTEC · initiating service RNC-FNI | same |
| Programme / budget line | NAVISP Element 1 · E/0365-200 | same |
| Price range | "> 500 KEURO". The NAVISP Work Plan 2026 Addendum lists **€800k, 24 months** | snapshot; research report 01 [S1][S2] |
| Open → close | 24 Sep 2026 → **Mon 4 Jan 2027, 13:00** (Brussels time per esa-star convention) | snapshot `tenders.csv` |
| Clarification deadline | **Mon 28 Dec 2026, 13:00** | snapshot `tenders.csv` |
| Last update | 11 Nov 2026, "Tender Clarification has been published" | snapshot |
| Eligible countries | AT, BE, CH, DE, FI, FR, GR, HU, IE, NL, NO, PL, PT, SE, GB. **Not Estonia. Italy opted out.** | snapshot; report 01 |
| Evaluation weights | Background and experience, incl. facilities **15%** · Understanding and programme of work **55%** (the snapshot shows "55,[redacted]%"; the scrubber mangled the figure, so read the exact value in the LoI) · Time dedication, costing, admin compliance and contract conditions **30%** | snapshot |
| IP measure / ECOS | IP measure N/A · ECOS required: True | snapshot |

> **Deadline conflict, resolve first.** Research report 01 (18 Nov) says EL1-132 "closed 18 Nov 2026
> 13:00". The esa-star snapshot (taken 15 Nov, last update 11 Nov, "Tender Clarification has been
> published") shows **4 Jan 2027**. The most likely explanation is that the 11 Nov clarification
> extended the deadline (inference). The founder must confirm the closing date on the live esa-star page
> and download the clarification before any work starts.

### Key acceptance factors (pass/fail)

1. Eligibility (EXPRO/TC §1.1), for the tenderer **and every subcontractor**.
2. Free competition (Cover Letter ¶10).
3. Non-commitment by the Agency (EXPRO/TC §1.7).
4. Non-benefits requirements (EXPRO/TC §1.8).
5. Binding price in the Cover Letter and the Detailed Proposal.
6. Price type as requested in the ITT.
7. Within the budget limit.
8. **A statement signed by the NAVISP Element 1 delegate of the prime's country (and of each
   subcontractor's country), attached to the Cover Letter, authorising ESA to fund the quoted price.**
   For a German prime, that is the German delegation (German Space Agency at DLR). **No delegate letter
   means a non-compliant bid.** This is the longest-lead item after the UG itself.

### Documents the founder must download (esa-star login needed; not in any repo)

Tender 194887 on esa-star Publication, manifest as of 15 Nov:

| File | Type | Why |
|---|---|---|
| `eli1-13695.pdf` | Letter of Invitation | exact weights, max price, price type, submission rules, KAF wording |
| `etc1-13695.pdf` | Tender Conditions (EXPRO/TC) | §1.1 eligibility (subcontractors), §1.7, §1.8 |
| `ecc1-13695.pdf` | Contract Conditions | IP clauses, background IP, payment plan |
| `ept1-13695.docx` | Proposal Template | the structure the proposal must follow, Cover Letter ¶10 |
| `ews1-13695.pdf` | **Statement of Work** (1.5 MB) | the real requirements: tasks, deliverables, milestones, TRL target, hardware deliverables |
| **any clarification document(s)** published 11 Nov 2026 or later | Clarifications | the deadline change and any Q&A; not in the 15 Nov manifest, so check the tender page |

Save them to the private Vault copy (esa-star terms: internal bid use only). When the SoW is read, every
line marked **Unknown until ITT read** in the compliance matrix must be resolved.

## 2. Go / no-go criteria

| # | Gate | Green when | By | Owner | Status |
|---|---|---|---|---|---|
| G1 | **Eligible bidding entity** | esa-star helpdesk confirms a route (UG registered in time, or another accepted route), **and** the entity is registered/validated in esa-star | answer by Tue 1 Dec; registration by Mon 14 Dec | Founder | ▢ helpdesk question drafted ([`clarification-questions.md`](clarification-questions.md) §A) |
| G2 | **German NAVISP delegate letter obtainable** | German Space Agency at DLR confirms it will issue the funding statement for the UG's price | contact by Fri 27 Nov; written yes or "in process" by Mon 14 Dec | Founder | ▢ |
| G3 | **RF / eLoran hardware partner** signed up (teaming letter), eligible (in the country list, not Italy) and bringing named key persons (§4) | LOI or teaming agreement | Mon 14 Dec | Founder | ▢ |
| G4 | **No showstopper in the SoW** (e.g. a mandatory hardware product we cannot source, on-air transmission, an access requirement we cannot meet) | SoW read; matrix "Unknown" lines resolved; no "cannot meet" line | Fri 27 Nov | Analyst + founder | ▢ documents not yet downloaded |
| G5 | **Capacity** | founder can commit ≈ 0.86 FTE over 24 months (380 days; see the cost skeleton) or a named second engineer is available; does not collide with the seed raise (first close target 29 Jan 2027) | Mon 7 Dec | Founder | ▢ |
| G6 | **IP posture** | the proposal can describe the simulator without disclosing unfiled patent-candidate methods (see §6); background IP declared under the commercial licence | Mon 7 Dec | Founder (counsel optional) | ▢ |

**Decision rule.** All of G1–G4 green on 7 Dec: write and submit as prime (Ashforde UG). G1 or G2 red,
G3 green: Plan B, the partner primes and Ashforde supplies the engine, **if** clarification Q1 confirms
an Estonian subcontractor/supplier is allowed. G3 red on 14 Dec: no-go as prime; send the licensing
offer (Plan B) to likely primes instead.

**Why it is worth the effort (inference, from reports 01, 10 and the snapshot):**
- Report 10 ranks it the best-fitting open ESA tender (fit 5/5). The SoW themes on the esa-star page
  (signal generation, propagation, multi-chain, multi-time-reference, record and replay, receiver
  analysis and validation tools, coupling to commercial GNSS simulators) map to modules that already
  exist in `crates/kshana-eloran` (see the matrix).
- 55% of the score is understanding and programme of work. Only 15% is company background, which is
  Ashforde's weakest area.
- The same engine serves the five follow-on eLoran ITTs (EL1-133, EL1-145, EL1-151, EL1-152, EL1-153;
  1 Jan to 1 Apr 2027 per report 12). Winning EL1-132 makes Ashforde the reference simulator team for that programme.

**Why it may fail (be honest with ourselves):** no eligible entity yet; no hardware or live-signal
validation of any kind; no prior ESA contract as prime; a solo founder raising a seed round at the same
time; and incumbents with lab facilities (GNSS simulator vendors, receiver makers) can team quickly.

## 3. Eligibility steps, with dates

| Step | What | Target date | Notes |
|---|---|---|---|
| E1 | Confirm the closing date and download all documents (§1) | **Fri 20 Nov** | esa-star login |
| E2 | Send the esa-star helpdesk question on the UG in formation ([`clarification-questions.md`](clarification-questions.md) §A). Use the helpdesk/contact route **shown inside esa-star**; do not guess an address | **Fri 20 Nov** | (unverified) whether a UG i.G. can be registered or bid |
| E3 | Complete the UG: pay in the €500 capital, notary filing, entry in the Handelsregister (Amtsgericht Darmstadt) | ASAP; needed by **Fri 11 Dec** for esa-star registration in time | FACTS.md 18 Nov: Commerzbank account opened; capital and notary filing pending |
| E4 | Register Ashforde UG on esa-star (entity registration and validation); link it to the founder's user | start the day the HR extract exists; validated by **Mon 14 Dec** | validation lead time (unverified); ask in E2 |
| E5 | Ask the German NAVISP delegation (German Space Agency at DLR) for the funding authorisation statement. Use the contact route published on DLR's own site, or ESA's NAVISP pages | first contact **Fri 27 Nov**; letter in hand by **Tue 22 Dec** | Also brief them on the follow-on EL1-133/145/151/152/153 ITTs |
| E6 | Partner's delegate letter (if the partner is in another country, it needs its own country's statement) | **Tue 22 Dec** | Partner's job |
| E7 | Submit the ESA clarification questions ([`clarification-questions.md`](clarification-questions.md) §B) | **Fri 4 Dec** (hard limit Mon 28 Dec 13:00) | Asking early gets answers before the gate review |
| E8 | Intellectual property: Arksa IP goes to the UG (FACTS 18 Nov). For this bid, decide whether **kshana-eloran** background IP is held or licensed by the UG (the OÜ → UG licence or assignment) | **Mon 7 Dec** | Background IP must sit with, or be licensed to, the tenderer |

**Other eligibility notes**
- **The OÜ cannot be the tenderer:** Estonia is not in the list. Earlier dossiers record the OÜ's esa-star entity (83208)
  as validated with **German nationality** (13494 gap audit, Jun 2026), while another note lists
  entity 1000051523 (`research/ESA_FUNDING_AND_PARTNERSHIP_PLAN.md`). Both are **(unverified)**: check which record is
  live. The helpdesk question also asks whether the DE-validated record could tender for a DE-eligible
  NAVISP ITT. Do not rely on it. The 13494/13503 audits rated an Estonian registered office a
  contract-award risk.
- **Subcontractors must also be eligible** (KAF 1). An Estonian OÜ subcontract is therefore doubtful
  without a clarification (Q1 in §B).
- **The founder's ESA relationship.** FACTS.md lists the founder as "Space Systems and DevOps Engineer at
  ESA/ESOC from 2021". If the founder still works at or for ESA (staff or contractor), check the
  conflict-of-interest and non-benefits rules (KAF 4) before tendering. **(Unverified: founder to confirm.)**

## 4. Partner needs

Ashforde brings the software simulator, propagation physics, receiver analysis, evidence chain and
programme management. Ashforde does not have the following, so a partner must bring them:

| Need | Why | What "good" looks like |
|---|---|---|
| **LF/eLoran hardware heritage**: eLoran transmitter, timing or receiver equipment, or an LF signal generator/simulator they have built | The simulator needs a real 90–110 kHz output chain (DAC, LF amplifier, antenna or H-field simulator) and someone who has measured it. Kshana's hardware drivers compile but have **never run on hardware** | Has delivered eLoran or Loran-C equipment, LF test benches, or NAVISP/ESA navigation hardware |
| **Lab facilities**: a commercial GNSS simulator (Spirent/Keysight, Safran Skydel, or R&S), a 10 MHz/1PPS reference, a time-interval counter, a reference eLoran receiver | 15% weight includes "adequacy of the proposed facilities"; the sync with commercial GNSS simulators must be shown | Named lab, equipment list, availability over 24 months |
| **Live-signal access** for fidelity validation | The validation task uses live signals; Europe's only active transmitter at present is Anthorn (UK) (report 01) | A site in Anthorn's coverage area, or a partnership with GLA/Trinity House, a national operator or a test transmitter |
| **Named key persons** | ESA scores staff experience | (1) RF/LF hardware lead with eLoran or LF experience; (2) validation/test lead who has run receiver test campaigns; (3) Ashforde: Chakshu Baweja, project manager and software/systems lead (8+ years; ESA/ESOC Space Systems and DevOps Engineer from 2021; author of six 2026 arXiv papers on PNT and timing) |
| **TRL evidence** | NAVISP asks for a TRL path from EL1 toward EL2 equipment | Partner hardware at TRL ≥ 5–6. Kshana software is self-assessed at **TRL 3–4**: components checked against published references in software; nothing on hardware or live signals |
| Eligibility | KAF 1 and 8 | In the eligible list (not Italy; Austria fine for EL1-132); can get its own delegate letter |

**Candidate partner profiles** (inference from report 01; **no outreach without the founder's go**):
- eLoran receiver/antenna incumbent in the UK. Its country is eligible, and NAVISP EL1-065/EL1-080 heritage was found.
- eLoran receiver maker in the Netherlands (NL eligible).
- Transmitter/timing equipment houses with European entities.
- A GNSS simulator vendor's European entity: L-band side only, no LF stack published.
- German test labs or universities with LF/navigation labs (keeps the work in Germany, under one delegation).

Pick the partner whose delegate is easiest to obtain. A **German partner** means one delegation (DLR)
for both. A UK partner needs a UK delegate statement as well.

## 5. What we bid (shape)

- **Prime:** Ashforde UG (when G1 and G2 are green). **Partner:** RF/LF hardware and lab (subcontractor).
- **Offer:** a software-defined eLoran/GNSS simulator core (Kshana Pro `kshana-eloran`, brought in as
  background IP under the commercial licence), extended and qualified under the contract; a reference
  hardware chain built with the partner; receiver analysis and validation tools; a live-signal fidelity
  campaign; open datasets in a documented format for the EL3-036bis PNT Digital Twin; an EL1→EL2 roadmap.
- **Plan B:** a non-exclusive, fixed-price engine licence plus engineering support to whichever eligible
  prime bids, if ESA's answer allows an Estonian (or not-yet-registered German) supplier.

## 6. Honesty and IP rules for this bid

- Every performance figure carries **VALIDATED** (reproduced against an independent published reference by an
  automated test) or **MODELLED** (internal tests only). The labels come from
  `crates/kshana-eloran/docs/VALIDATION.md`. Never "flight-proven", "field-proven", "certified" or "compliant".
- **Nothing has run on hardware or on a live eLoran signal.** Say it plainly.
- Pre-revenue; no customers, LOIs or patents. "Patent candidates" exist but **none are filed**. Methods
  behind the `unfiled-methods` feature (attribution guard E1/E4, LDC delay authentication E7, spoof-safe
  ASF learning E3, self-calibration F1, survey planning F2, first-plug conformance C2/B2) are described in
  the proposal **only as outcomes, never as methods**, until filed. A proposal is confidential to ESA, but
  this rule follows `docs/ELORAN.md` and keeps novelty safe.
- Competitors are compared on published facts only (report 01 §2). Never "kill", "obsolete" or "dominate".
- Hardware output is conducted only (cables or antenna simulators). No over-the-air transmission.

## 7. Founder action list

| Date | Action |
|---|---|
| **Fri 20 Nov** | Confirm the closing date on esa-star; download the 5 documents plus clarifications (§1). Send the helpdesk question (§A). |
| **by Fri 27 Nov** | Push the UG through capital, notary filing and Handelsregister. Make first contact with the German NAVISP delegation. Confirm your ESA/ESOC status (conflict check). Hand the SoW to the analyst to close the "Unknown" lines. |
| **by Fri 4 Dec** | Approve and submit the ESA clarification questions (§B). Shortlist and approach 2–3 hardware partners (founder's go needed). |
| **Mon 7 Dec** | Gate review G1–G6: go, Plan B or no-go. |
| **by Mon 14 Dec** | Partner teaming letter; UG validated on esa-star; delegate letter request in process. Hard stop if still red. |
| **Tue 22 Dec** | Delegate letter(s) in hand; draft proposal frozen. |
| **Mon 28 Dec 13:00** | Last chance for clarifications. |
| **Wed 30 Dec** | Submit (do not leave it to 4 Jan; it falls over the holidays). |
| **Mon 4 Jan 2027 13:00** | Closing. |

_Last updated 19 Nov 2026. Sources: esa-star snapshot 2026-11-15 (summary 194887, `document_manifests/194887.json`,
`tenders.csv`), vcprep research reports 01 (18 Nov), 10 and 12 (20 Nov), `docs/ELORAN.md`,
`crates/kshana-eloran/docs/VALIDATION.md` at kshana-pro main 6c222bc. The SoW has not been read._
