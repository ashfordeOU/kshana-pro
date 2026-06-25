<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; never publish in the open kshana repo.
Audit of ESA Clarifications 1-3 to AO/1-13503/26/NL/RR and their impact on the Ashforde pursuit.
Source PDFs (founder's machine): ~/Downloads/1-13503_25062026023528/clar13503e{1,2,3}.pdf
Contact: contact@ashforde.org
-->

# Clarifications 1–3 audit — AO/1-13503/26/NL/RR (Quantum-Enabled PNT Demonstrator)

**Bottom line: net-positive for us, on every count.** Three clarifications, all of which either
*widen* what a software/simulation SME can compliantly contribute, *lower* the documentation burden, or
*relax* the timeline. One of them (Clar 1, Answer 3) is a material upgrade to our win theory — ESA has
now stated, in writing, that a **simulation environment** is a compliant way to demonstrate the
feasibility, performance and operational benefits of the proposed concepts. That is, almost verbatim,
what Kshana is.

Cross-reference: [`TENDER-13503-PURSUIT.md`](TENDER-13503-PURSUIT.md) (tracker, now updated),
[`technical-contribution.md`](technical-contribution.md) (§1 trade method), [`software-PA-evidence.md`](software-PA-evidence.md)
(internal PA process), [`D1.1-gnss-denied-resilience-benefits.md`](D1.1-gnss-denied-resilience-benefits.md).

---

## Clarification 2 (19 Jun 2026) — DEADLINE EXTENDED ⏰

**9 Jul 2026 → 17 Aug 2026, 13:00 Amsterdam.** +39 days (~5.5 extra weeks).

- **Impact:** the single biggest practical change. The old tracker flagged the schedule as ~6 days
  *shorter* than the audit plan and forced a "seat by ~29 Jun or pivot to OSIP" compression. **That
  compression is gone.** The hard gate (T7, secure a consortium seat) now has a realistic runway, and
  there is time to run a proper open-competition selection if that is the route in (see Clar 1/A2).
- **⚠ Check:** this extension is for **13503 only**. The sister tender **13494** (AI/Digital Payload PNT)
  is, as far as we know, **still 9 Jul 2026** — verify on esa-star. If 13494 holds at 9 Jul, the two
  tenders are now ~6 weeks apart, which *helps* (sequence effort: 13494 first, then 13503), but keep them
  firewalled (combined proposals forbidden).
- **Action:** founder to re-confirm 13494's deadline; re-baseline both trackers' dates.

---

## Clarification 1 (12 Jun 2026) — three substantive Q&As

### A1 — a SINGLE application area is compliant ✅ (focus, don't sprawl)

ESA: *"A mission concept focused on a single application area would be considered compliant, provided it
is clearly justified … and demonstrates a credible path towards a future quantum-enabled PNT in-orbit
demonstrator."* Multi-area concepts are "welcome and encouraged … but not at the cost of depth,
feasibility and demonstrator relevance." The candidate areas (SoW §1.5.2): quantum sensors for
spacecraft navigation; **optical timing generation and distribution**; **quantum time transfer**;
**fault/anomaly detection for quantum systems**.

- **Impact:** we do **not** need to span the whole design space. We can anchor the bid on the **one area
  where Kshana is strongest and the barrier-to-entry for a software SME is lowest** — i.e. an
  analysis/simulation-led area rather than a hardware-led one. Two natural fits, both software-heavy:
  1. **Quantum time transfer / optical timing distribution / secure time dissemination** — Kshana owns
     ADEV/MDEV/TDEV (validated vs Stable32/NIST), van-Loan Kalman clock + holdover FoM, two-way time
     transfer with reciprocal-delay cancellation, and the Timing Protection Level (arXiv:2606.24210).
  2. **Fault / anomaly detection for quantum systems** — Kshana owns the resilience-scoring engine, the
     fused spoof/jam/SQM detectors, the TPL CUSUM change-detector, and the AI-evaluation /
     optimism-gap methodology. This area is almost entirely software/analysis — an ideal SME WP.
- **Action (founder decision):** pick the focused application area. Recommendation: lead with **secure
  time dissemination / quantum time transfer** (our deepest validated stack) **or** **anomaly detection
  for quantum systems** (lowest hardware dependency, highest SME ownership), and treat the
  quantum-vs-classical trade as the cross-cutting method that substantiates the gains. Either keeps the
  bid deep and feasible, which A1 explicitly rewards.
- Note A1 cites **ESA Tender Conditions Part 3.2** — tenderers must "substantiate how the selected
  concept delivers the targeted PNT performance, robustness, autonomy and/or resilience gains." **That
  substantiation is exactly Kshana's quantum-vs-classical trade method** (technical-contribution §1).

### A2 — open competition for critical pre-developments must COMPLETE before submission ⚠

ESA: open competition is required for subcontractors providing **critical pre-developments**; this must
total **≥ €500,000 per contract** awarded by open competition; and the process (ESA Tender Conditions
Part 2.3.1), **including selection of the subcontractors forming part of the consortium, must be
completed before the proposal is submitted.**

- **Impact — two routes in, know which one is ours:**
  - **Route A (preferred): named consortium partner from the outset.** Ashforde's WP is
    *navigation-performance analysis + the quantum-vs-classical trade + the simulation-based
    feasibility/benefit demonstration* — this is **mission-analysis / SE / software**, not a hardware
    "critical pre-development." As a named consortium member, the open-competition-before-submission
    constraint does not gate us; we are part of the team the prime forms.
  - **Route B: critical-pre-development subcontractor via open competition.** If a prime instead scopes
    our work as a critical pre-development, they must run an open competition and **select us before
    submission** — a merit route that *favours* an SME with public, reproducible evidence, but with a
    hard pre-submission deadline (now eased by the extension).
- **Action:** when engaging a prime, state plainly that Kshana is offered as a **named consortium WP
  (Route A)**; confirm with them whether any part is treated as a critical pre-development (Route B). The
  €500k open-competition floor is the *prime's* obligation to satisfy across the consortium — useful to
  understand, not ours to carry. Keep the consortium-seat work (T5–T8) as the hard gate; the extension
  buys the time to do it properly.

### A3 — simulation environments are a COMPLIANT maturation/demonstration method ✅✅ (the big one)

ESA: Phase B1 is **not** full space qualification or a flight model. Critical tech: TRL 3/4 → 4/5 (end
Phase A) → target 5/6 (end B1). For low-TRL quantum tech, *"it would be compliant to demonstrate
feasibility, performance and operational benefits through laboratory or ground-based representative
demonstrations using existing hardware, engineering models **and/or simulations environments**, provided
that appropriate representativeness is justified and remaining gaps towards flight are clearly
identified."*

- **Impact — this elevates Kshana from "heritage/credibility" to a requirement-compliant demonstration
  vehicle.** ESA has explicitly named *simulation environments* as a compliant way to demonstrate
  **feasibility, performance and operational benefits** — the exact verbs in the objective — and has
  required exactly the discipline Kshana is built around: **"appropriate representativeness justified"**
  (our validated/modelled labelling) and **"remaining gaps towards flight clearly identified"** (our
  honesty matrix / `verification.rs`). Our differentiator (honest, reproducible, validated-or-modelled
  simulation) is now *the named compliance mechanism*, not a nice-to-have.
- **What we can do with it:**
  - Reframe Ashforde's WP from "trade-study support" to **"the simulation-based feasibility-,
    performance- and operational-benefit demonstration for the selected application area"** — a
    load-bearing contribution to the Phase 0/A/B1 objective, paired with the partner's lab/EM work for
    device-level TRL. Update `technical-contribution.md` and the PBF-7 WP descriptor accordingly.
  - Make **representativeness + gap-to-flight** a first-class, explicit section of our method (it already
    is, implicitly, in the validated/modelled labelling — now name it against A3's wording). This turns
    our honesty moat into a direct compliance argument the reviewer is told to look for.
  - The **TRL-maturation dossier** (T23) should cite A3: Kshana supplies the
    feasibility/performance/operational-benefit evidence at TRL 3/4→4/5→5/6, with gaps-to-flight
    enumerated, while space qualification defers to B2+ (A3 explicitly permits this with a credible
    roadmap).
- **Action:** thread A3's wording through `technical-contribution.md` (§1), `TRL-maturation-dossier.md`
  (T23), and the WP descriptor; add a "representativeness & gaps-to-flight" sub-section.

---

## Clarification 3 (24 Jun 2026) — FPD-10031 PA&S is Reference, no SoC required ✅ (lower burden)

ESA: the Product Assurance & Safety Requirements doc (FPD-10031) is a **Reference**, not an Applicable,
document; **no separate Statement of Compliance is required**; PA&S is addressed "at a level
commensurate with the Phase 0/A/B1 nature"; for pre-developments the tenderer must "have an internal
Product Assurance process in place and apply it"; full compliance not expected, "progressive approach …
beginning in Phase B1."

- **Impact — favourable for an SME, and a direct fit for what we already have.** No heavyweight PA&S
  compliance statement to produce. The bar is "an internal PA process, applied appropriately" — and
  Kshana's software engineering hygiene **is** a demonstrable internal PA process: CI gate, full test
  suite, machine-checked `verification.rs` matrix, CycloneDX SBOM, SLSA build provenance,
  reproducible-by-construction outputs. This is exactly what `software-PA-evidence.md` (T21, ECSS-Q-ST-80
  mapping) already documents.
- **What we can do:** position `software-PA-evidence.md` as evidence of our **internal PA process** (per
  Clar 3), explicitly scoped to the software half; flight-hardware PA remains a partner's. Add one line
  to that doc tying it to Clar 3 and the "progressive approach from Phase B1" expectation.
- **Action:** annotate `software-PA-evidence.md` with the Clar 3 framing; **do not** produce an
  FPD-10031 SoC (confirmed not required).

---

## Consolidated action list

**Founder-gated decisions (strategic):**
1. **Pick the focused application area** (A1) — recommend secure time dissemination / quantum time
   transfer, *or* anomaly detection for quantum systems (lowest hardware dependency). One area, deep.
2. **Confirm 13494's deadline** (still 9 Jul?) and re-sequence the two pursuits (13503 now 17 Aug).
3. Proceed (when ready) with the **consortium-seat** work (T5–T8) — now with a realistic runway; state
   Kshana as a **named consortium WP (Route A)**.

**Document updates (can do now, no founder gate — bid material in this folder):**
4. ✅ Tracker re-baselined to the **17 Aug 2026** deadline (done — see `TENDER-13503-PURSUIT.md`).
5. Thread **A3** (simulation = compliant demonstration; representativeness + gaps-to-flight) through
   `technical-contribution.md` §1, the PBF-7 WP descriptor, and `TRL-maturation-dossier.md` (T23).
6. Annotate `software-PA-evidence.md` as our **internal PA process** per **Clar 3**; note no FPD-10031
   SoC required.
7. Update deadline references in `../README.md` and `../cross-tender/README.md` for 13503 (done where
   13503-specific; 13494 left at 9 Jul pending confirmation).

**Do NOT:**
- Produce an FPD-10031 Statement of Compliance (Clar 3: not required).
- Start prime outreach — remains founder-gated/parked.
- Overclaim: A3 legitimises *simulation-based demonstration with justified representativeness and stated
  gaps-to-flight* — it does **not** licence TRL/flight/space-qualification claims. Keep the
  validated/modelled discipline; that is now also the compliance argument.

---

*Last updated 2026-06-25. Sources: Clarifications 1 (12 Jun), 2 (19 Jun), 3 (24 Jun) to AO/1-13503/26/NL/RR;
existing 13503 pursuit pack. Ashforde OÜ · contact@ashforde.org · Kshana TRL ~3, validated/modelled, no
flight heritage / certification / ESA endorsement.*
