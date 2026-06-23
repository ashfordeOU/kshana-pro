<!-- PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE. -->

# Pack A — Ideal Customer Profile & How to Build the Target List

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

Who Pack A is for, why they buy, when they buy, and a **repeatable method** to build the
real prospect list. This document names well-known market players **only as market context** —
it contains **no invented contacts, leads, or claimed relationships**. Build the actual list
yourself with the methods below.

---

## The ideal first design partner (the bullseye)

Before segmenting, anchor on the single best first customer. The ideal Pack A design partner:

- **Has a detector.** A working GNSS spoofing / jamming / RF-impairment detector that emits a
  per-sample score (or one that can be made to).
- **Has labelled ID/OOD data.** Ground-truth labels and a sense of which data is bench
  (in-distribution) vs field/shifted (out-of-distribution) — they can produce the intake.
- **Has an internal V&V process** the study feeds into — they already care about evidence,
  acceptance, and defensibility, so the deliverable lands in an existing workflow.
- **Has budget authority** and a person who can say yes to a small paid engagement.
- **Has a near-term decision** the study informs: a go/no-go, a customer trial, a milestone
  review, a tender response, an investor or board readout.

A team with four of those five is a strong first design partner. Lead with these.

---

## Market segments

For each segment: **who they are**, **why they'd buy** (their pain), **the buying trigger**,
and **how to find them**. Named companies are illustrative of the *category*, not leads.

### 1. GNSS receiver & chipset vendors (algorithm groups)

- **Who:** R&D / algorithm teams inside receiver and chipset makers shipping interference
  monitoring or RFI/spoof detection features. *(Category context: the large GNSS
  receiver/chipset names familiar from ION proceedings and product datasheets.)*
- **Why they'd buy:** they claim interference robustness in datasheets and to customers, but
  can't quantify how a bench-tuned detector degrades on conditions they didn't train on. The
  optimism gap is exactly the number their own customers will eventually probe.
- **Buying trigger:** a new detection feature heading to release; a customer asking "how does
  this hold up in the real world?"; a competitor making a robustness claim they must answer.
- **How to find them:** ION GNSS+ and ION ITM exhibitor and author lists; product pages
  mentioning "interference monitoring / RFI detection / spoofing detection"; LinkedIn searches
  for titles like *"GNSS algorithm engineer"*, *"signal processing — GNSS"*, *"interference
  detection"* at receiver companies.

### 2. Anti-jam / anti-spoof & interference-detection startups

- **Who:** venture- or grant-funded companies whose whole product is detecting GNSS
  interference, spoofing, or jamming — including resilient-PNT and "GPS-denied" players.
- **Why they'd buy:** their entire pitch is robustness, so a credible, reproducible, honestly
  labelled third-party gap study is a *sales asset* for them, not just internal hygiene. They
  also feel the optimism gap most acutely because their differentiation lives or dies on it.
- **Buying trigger:** a funding round (diligence wants third-party evidence); a pilot with a
  defense or critical-infrastructure customer; a published competitor benchmark.
- **How to find them:** startup/scale-up directories and accelerators tagged
  *resilient PNT / assured PNT / GPS-denied*; defense-innovation programme cohorts; arXiv and
  ION authors affiliated with companies rather than universities; LinkedIn company search on
  *"anti-spoofing"*, *"anti-jam"*, *"assured PNT"*, *"resilient PNT"*.

### 3. Defense & space primes' PNT teams

- **Who:** PNT / navigation-warfare / EW groups inside large defense and space primes and
  their PNT-focused subsidiaries.
- **Why they'd buy:** rigorous, traceable evidence is the currency of their acceptance and
  V&V processes; a reproducible MODELLED gap study with a provenance block fits how they
  already document decisions. Honest "not a certification" framing is a feature, not a bug, in
  this audience — they distrust overclaiming.
- **Buying trigger:** a programme milestone or design review; a navwar / resilient-PNT
  initiative; a tender or capability statement that needs supporting evidence.
- **How to find them:** ION ITM / ION GNSS+ government-and-defense sessions; programme
  announcements and RFIs from defense/space agencies; LinkedIn titles like *"PNT systems
  engineer"*, *"navigation warfare"*, *"GNSS resilience lead"* at primes.

### 4. GNSS test houses, metrology labs & simulators

- **Who:** independent test houses, calibration/metrology labs, and GNSS
  simulator/record-and-replay vendors that already run detector and receiver characterisation.
- **Why they'd buy:** the optimism-gap study is a natural **add-on service** to what they sell,
  and a reproducible methodology with provenance complements their measurement credibility.
  They may resell or co-deliver it (subject to licence terms).
- **Buying trigger:** a client asking for distribution-shift / robustness characterisation
  beyond standard test points; building out a resilience-testing service line.
- **How to find them:** ION GNSS+ exhibitor lists (test & simulation vendors); accreditation
  registers for GNSS/EMC test labs; standards-body participant lists (see "Standards bodies").

### 5. Academic & government PNT research groups

- **Who:** university navigation labs and government/national-lab PNT groups building or
  evaluating interference detectors.
- **Why they'd buy:** they value reproducibility and open methods, can produce clean labelled
  ID/OOD data, and may co-author or cite — turning a paid study into shared credibility. Often
  the *fastest* first design partner because the alignment on open, honest, reproducible
  evidence is total.
- **Buying trigger:** a grant deliverable; a paper needing an external reproducible gap
  measurement; a benchmark or dataset release.
- **How to find them:** authors of GNSS interference/spoofing detection papers on arXiv and in
  ION/IEEE/*NAVIGATION* venues; **citations of and around arXiv:2606.22054** (forward and
  backward) are the single highest-signal source — anyone working in this exact space; lab
  websites; government PNT programme pages.

---

## How to build the real prospect list — a repeatable method

Run this loop weekly; it produces named, real, non-fabricated prospects:

1. **Mine the literature.** Pull authors and affiliations from recent GNSS
   interference/spoofing/jamming detection papers on **arXiv** and in **ION**, **IEEE**, and
   *NAVIGATION* venues. Track **citations of arXiv:2606.22054** specifically — they are
   already thinking about distribution shift and the optimism gap.
2. **Work the conferences.** **ION GNSS+** and **ION ITM** author lists, session programmes,
   and exhibitor directories are the densest concentration of the ICP. Also scan EW / navwar
   and resilient-PNT industry events.
3. **Map the standards bodies.** Participant and working-group lists for GNSS interference,
   resilient-PNT, and receiver-testing standards surface both vendors and labs who care about
   characterisation evidence.
4. **Search LinkedIn by role + keyword.** Title patterns: *"GNSS algorithm"*, *"interference
   detection"*, *"anti-spoofing"*, *"anti-jam"*, *"PNT resilience"*, *"navigation warfare"*,
   *"signal processing GNSS"*. Filter to companies/labs that fit a segment above.
5. **Scan product & funding signals.** Datasheets claiming "interference / spoofing detection";
   funding announcements for assured/resilient-PNT startups (diligence-driven buyers).
6. **Score against the bullseye.** For each prospect, mark how many of the five design-partner
   criteria they plausibly meet (detector / labelled ID/OOD data / V&V process / budget /
   near-term decision). Prioritise 4–5 matches.
7. **Find the right human.** Identify the algorithm lead or PNT-resilience owner — the person
   who feels the optimism gap — not a generic sales/marketing inbox.
8. **Log it.** Keep one tracker (company, segment, contact, role, signal, bullseye score, next
   touch) so the [outreach cadence](OUTREACH-SEQUENCE.md) runs cleanly.

Outputs of this method feed directly into [`OUTREACH-SEQUENCE.md`](OUTREACH-SEQUENCE.md);
qualify each prospect with [`QUALIFICATION-AND-OBJECTIONS.md`](QUALIFICATION-AND-OBJECTIONS.md).

---

*All contact via **[contact@ashforde.org](mailto:contact@ashforde.org)**. Named companies above
are market context only — Ashforde OÜ claims no relationship with any of them. Pack A is a
MODELLED study, not a certification.*
