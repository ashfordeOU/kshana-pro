# Export-Control Determination Questionnaire — Pack B (Timing-Resilience / Holdover)

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

> ⚠️ **THIS DOCUMENT IS NOT LEGAL OR EXPORT-CONTROL ADVICE, AND IT IS NOT A
> DETERMINATION.** It is a structured checklist of the questions and steps that a
> qualified export-control adviser and/or the competent national authority (for
> Ashforde OÜ, established in Estonia: probably Estonia's **Strategic Goods Commission**
> at the Ministry of Foreign Affairs, to be confirmed by counsel as question E1 in
> `commercial/export-control/CLASSIFICATION-REQUEST-PACK.md`; this draft first assumed
> Germany's BAFA, see §2.3) will use to *make* a determination. It does not classify any item, does not
> assert that anything is or is not controlled, and does not authorise any sale.
> Nothing here may be relied upon as a basis to export, disclose, package, or sell.
> A binding outcome requires qualified counsel and, where relevant, a formal
> classification opinion from the competent authority.

---

## 0. How to use this document

This questionnaire **operationalizes** the existing internal screen in
`legal/EXPORT-CONTROL-ASSESSMENT.md`. That assessment is a preliminary self-screen;
it deliberately stops at "potentially controlled / treat as controlled until
screened." This document is the **next layer**: the concrete questions the founder
must answer, and the steps to take, so that a competent adviser/authority can turn
"potentially" into a documented determination.

- Do not duplicate or re-litigate the assessment here. Answer the questions below
  with **facts about Pack B**, attach evidence, then hand the completed pack to
  counsel.
- Where a question cannot be answered factually yet, mark it **OPEN** — an open
  question is itself a gate item.
- All external correspondence uses **contact@ashforde.org** only.

### Honesty constraints carried from the program

- Everything Pack B computes is **MODELLED** simulation output. It is **not a
  certification** and **not** a measurement of, or against, real hardware.
- No TRL, heritage, or ESA-endorsement claims may be made or implied to any
  adviser, authority, or customer.
- The optimism-gap result is an **arXiv preprint (not peer-reviewed)**; describe it
  as such if it is referenced as part of the public-domain argument.

---

## 1. The question to be resolved

**Can Pack B (timing-resilience / holdover) be packaged and offered as a commercial
study, and under what conditions — given a formal export-control determination?**

Scope of this question:

- It concerns **Pack B only.** The **open core engine** and **Pack A
  (optimism-gap audit)** are **already public** and are **out of scope** of this
  determination — they are not gated by it and must not be put on hold by it.
- "Offered/packaged" includes preparing a saleable study deliverable, marketing it,
  delivering it to a customer, and any **technical assistance/consulting** wrapped
  around it.
- "Export" must be read broadly to include **intangible transfer** — disclosing
  controlled software or technology to a foreign person, by email, screen-share,
  repository access, or hosted delivery, can itself be a controlled act.

---

## 2. Regulatory frame to investigate (pose as questions, not conclusions)

The adviser/authority will work through the following. State each as a question to
be **determined**, never as a conclusion reached here.

### 2.1 EU Dual-Use Regulation (EU) 2021/821

- Does any component of Pack B fall within **Annex I** of Regulation (EU) 2021/821
  (the EU dual-use control list)?
- Which Annex I **categories** require examination? Candidates **to investigate**
  (not assertions of applicability):
  - **Category 7** — navigation and avionics (e.g. GNSS-related items, anti-jam /
    anti-spoof, timing/navigation functionality)?
  - **Category 5, Part 1** — telecommunications (timing/frequency, signal
    processing)?
  - **Category 5, Part 2** — "information security" / cryptography — relevant **only
    if** Pack B contains, uses, or requires cryptographic functions?
- Do the **general technology / software notes** and definitions in Annex I (e.g.
  "in the public domain", "basic scientific research", "required", "development",
  "production", "use") change the answer for the modelling software at issue?

### 2.2 Wassenaar Arrangement (origin of the lists)

- The relevant EU/national list entries derive from the **Wassenaar Arrangement**
  control lists. Is there Wassenaar-level guidance or interpretive note that informs
  how the candidate categories above apply to **modelling software** (as opposed to
  hardware or real performance data)?

### 2.3 National route: which authority? (revised 2026-09-26)

> **Revision.** This section first assumed the German route. Ashforde OÜ is established
> in Estonia, and BAFA's own page says it issues no goods-list opinion (AzG, Auskunft zur
> Güterliste) to a foreign company or for technology. The likely competent authority is
> Estonia's Strategic Goods Commission, under the Strategic Goods Act. Counsel confirms
> this first (question E1 in `commercial/export-control/CLASSIFICATION-REQUEST-PACK.md`).
> The German questions below stay relevant only if a German-established entity exports
> or contracts, or a German customer seeks its own classification.

- Ashforde OÜ's operative EU nationality is **German (DE)**; the EU Regulation is
  implemented nationally via the **Außenwirtschaftsverordnung (AWV)** and
  administered by **BAFA**.
- Is a **Güterlisteneinstufung / Auskunft zur Güterliste** (BAFA classification
  enquiry) the appropriate instrument to obtain a documented classification for the
  uncertain components?
- Are there **national (AWV) controls** beyond the EU Annex I list that must be
  checked?

### 2.4 Carve-outs to test (not to assume)

- **"Publicly available" / "in the public domain":** does the published open core
  qualify, such that those components are outside control? Confirm **in writing**,
  per the open assessment's existing checklist item.
- **"Basic scientific research":** does any part of Pack B's method qualify?
- **The arXiv preprint:** does a publicly posted, non-peer-reviewed preprint count
  toward the public-domain argument for the method it describes — and for which
  components, exactly?
- **Crucial distinction to resolve:** even if a *method* is public, is the **specific
  Pack B software build, parameters, scenarios, or accompanying technology**
  non-public and therefore separately classifiable?

### 2.5 Software vs technology vs technical assistance

- Distinguish three control surfaces and ask the question for each:
  1. **Controlled software** — the Pack B build/artifact itself.
  2. **Controlled technology** — the technical know-how "required" for development,
     production, or use of a controlled item (can be controlled even when the
     software is not shipped).
  3. **Controlled technical assistance** — instruction, consulting, training, or
     working services relating to a controlled item, including across borders.

---

## 3. Questionnaire — the founder must answer these (with evidence)

Group A–E below. Answer factually; attach the build/specs as the evidence pack for
counsel. Mark anything unresolved **OPEN**.

### A. Product characterization (what Pack B actually is)

- **A1.** What does Pack B compute, precisely? State each output: **timing
  holdover** behaviour, **protection-level / TPL bounds**, **time-to-breach**,
  **CUSUM (or other) detector** logic, holdover curves, requirement-traceability
  output.
- **A2.** Is Pack B **pure modelling software** with **no real hardware, no
  firmware, and no real device/threat data shipped**? Confirm the open/closed
  boundary described in the assessment holds (no real adversary waveforms, no real
  anti-jam/anti-spoof performance parameters bundled).
- **A3.** Does Pack B **contain, embed, call, or require cryptographic functions**
  (encryption, authentication, key management, signal authentication such as OSNMA
  handling)? This determines whether **Category 5 Part 2** is even in scope. If yes,
  describe exactly what and why.
- **A4.** Does Pack B **model or enable anti-jam / anti-spoof techniques** at a level
  of detail that could itself be controlled (e.g. parameters, algorithms, or
  performance figures tied to defeating real threats), or does it operate only on
  **representative/published** parameters with **MODELLED** output?
- **A5.** Does Pack B's output describe **performance against real, named threats**,
  or only **generic, parameterised** scenarios? (The assessment flags real-threat
  performance as the higher-scrutiny case.)
- **A6.** What is the delivery form — a **report/study** (output only), a **software
  licence** (customer runs the build), or a **hosted service**? Each may classify
  differently.

### B. The public-domain question (what is already public vs not)

- **B1.** List the components that are **already public**: the **open core engine**,
  the **arXiv preprint (non-peer-reviewed)**, any published scenarios/parameters.
- **B2.** For each public component, does the **"publicly available / in the public
  domain"** carve-out remove it from control? (Question for counsel; do not assume.)
- **B3.** What, specifically, in **Pack B is NOT public** — the commercial build,
  curated scenarios (e.g. 4 h / 24 h / 1-week holdover, denial + spoof profiles),
  parameters, report templates, or know-how? These are the items that need a
  determination.
- **B4.** Could combining individually-public elements into the Pack B deliverable
  create something separately classifiable (aggregation question)?

### C. End-use / end-user screening

- **C1.** Who are the **intended customers**? Characterise the ICP: **critical-infra
  timing** buyers — **5G / telecom, electricity grid, financial-market timing**.
- **C2.** Is there any **military, defence, intelligence, or dual-use end use**
  contemplated, directly or via a prime/integrator?
- **C3.** What **sanctioned-party / denied-party screening** will be run on every
  customer and its beneficial owners (EU consolidated sanctions list and national
  lists) before any disclosure?
- **C4.** Do the **"catch-all" (end-use) controls** apply — i.e. controls triggered
  by knowledge/suspicion of **military end use** in an arms-embargoed destination, or
  **WMD / missile** end use — even for items not on the list? What is the procedure
  to check this per engagement?

### D. Geography

- **D1.** Where is each customer **located / established**? Distinguish **intra-EU**
  delivery from **export to a third country**.
- **D2.** For third-country delivery, which **destination** and does that destination
  attract additional restrictions (e.g. embargo, country-specific measures)?
- **D3.** For **intangible transfer**, where will the **recipient persons** be (a
  foreign national receiving access changes the analysis even within the EU)? Where
  will any **hosting / data** sit?
- **D4.** How does the answer to D1–D3 change the **authorisation obligation** (e.g.
  whether an individual or general licence could be required)?

### E. The technical-assistance angle

- **E1.** Will delivery involve **consulting, interpretation, tuning, training, or
  bespoke study work** for the customer — beyond shipping a static artifact?
- **E2.** If yes, could that consulting constitute controlled **"technical
  assistance"** relating to a controlled item, especially to a foreign customer or
  recipient?
- **E3.** What scoping of the engagement (artifact-only vs assistance-included) would
  change the control posture, and is that a deliberate design choice to record?

---

## 4. Steps to a determination (action list)

1. **Assemble the product description / evidence pack.** A precise, written Pack B
   description answering Section 3A–E: outputs, build form, crypto status,
   scenario/parameter provenance, public-vs-non-public component list, intended ICP,
   and delivery model. This is the document counsel and the competent authority will classify.
2. **Engage export-control counsel** experienced with **EU dual-use** and the practice of
   the competent authority (Regulation (EU) 2021/821; for an Estonian company, the
   Strategic Goods Act and the Strategic Goods Commission). Have them review the assessment
   and this completed questionnaire.
3. **File a classification request with the competent authority where uncertain.** Where counsel cannot give
   a confident classification (especially for any non-public Pack B build,
   scenarios, or anti-jam/anti-spoof-adjacent content), pursue a documented classification from the
   authority counsel confirms under E1 (the German Auskunft zur Güterliste is not open
   to a foreign company).
4. **Document the determination.** Record per-component outcomes (Annex I
   category / "not listed", carve-out applicability, any licence requirement) in the
   **Record of determinations** table in `legal/EXPORT-CONTROL-ASSESSMENT.md` (that
   assessment owns the canonical log; this questionnaire feeds it).
5. **Implement screening + recordkeeping before any Pack B sale.** Stand up, and be
   able to evidence: customer end-use/end-user screening (sanctions + catch-all),
   intra-EU vs export determination per deal, access control on any non-public
   components, intangible-transfer controls, and retention of records — consistent
   with the operational gates already in the assessment.

---

## 5. GATE statement

> **Pack B packaging and sale remain ON HOLD** until a **documented export-control
> determination exists** (counsel opinion and/or a classification opinion from the competent authority),
> and the screening/recordkeeping in Step 5 is in place.
>
> The **open core engine** and **Pack A (optimism-gap audit)** are **already public
> and are UNAFFECTED** by this gate. Nothing in this document pauses, withdraws, or
> re-screens them.
>
> Until the determination is documented, do not market, price, deliver, or disclose
> any non-public Pack B component, and do not provide Pack B technical assistance, to
> any customer.

---

> ⚠️ **REMINDER — THIS IS NOT LEGAL OR EXPORT-CONTROL ADVICE AND NOT A
> DETERMINATION.** It is a checklist to drive one. Do not act on it as a
> classification. Obtain qualified export-control counsel and, where relevant, a
> formal classification opinion from the **competent authority** (see §2.3) before packaging, offering, exporting, or
> disclosing any Pack B component. Contact for this matter: **contact@ashforde.org**.
