# Export-Control (Dual-Use) Assessment — Kshana / Kshana Pro

**Owner:** Ashforde OÜ · **Status:** working self-assessment (PR5) · **Date:** 2026-06-14
**Decision gate:** this assessment is the **mandatory gate before any L2 resilience
work (PR12–PR14)**. ⚠️ It is a preliminary internal screen, **not a legal
classification**. A binding classification requires counsel and, where relevant, a
formal ruling from the competent authority (Germany: **BAFA**) before exporting or
disclosing controlled items.

---

## 1. Why this matters

Kshana Pro's **L2 layer** (real jam/spoof threat libraries, adversary waveforms,
anti-jam/anti-spoof performance) is the deliberately export-controlled moat — it
*legally cannot* be open. Before any such material is created, ingested, or shared,
its control status must be screened so Ashforde does not commit an export-control
breach.

Per the ESA-STAR determination, Ashforde's operative nationality is **German (DE)**;
the governing regime is therefore **EU Dual-Use Regulation (EU) 2021/821** as
implemented in Germany via the **Außenwirtschaftsverordnung (AWV)** and administered
by **BAFA**. ("Export" includes intangible technology transfer — i.e. sending
controlled software/technology or disclosing it to a foreign national, including
electronically.)

## 2. Item-by-item screen (preliminary)

| Item | Layer | Likely status (to confirm) | Rationale |
|------|-------|----------------------------|-----------|
| Open Kshana engine (representative parameters, published-coefficient models, public anchors) | Open | **Not controlled** (publicly available / basic-scientific-research carve-out) | Built from published references; the Dual-Use Reg generally exempts information "in the public domain" and basic scientific research. Confirm the carve-out applies as published. |
| GNSS jamming link-budget / spoof-detection **method** (open) | Open (O6) | **Likely not controlled** | Generic, textbook method with representative parameters; no real threat data. |
| **Real adversary waveform libraries** | L2 | **Potentially controlled** | Could engage Category 5 (telecom/info-security) and military-adjacent controls; treat as controlled until screened. |
| **Anti-jam / anti-spoof performance models with real parameters** | L2 | **Potentially controlled** | Performance against real threats may be specially designed for military use or meet dual-use thresholds. |
| **Restricted/classified resilience scenarios** | L2 | **Controlled / national-security** | Anything customer- or government-supplied at a classification level is handled under that customer's regime, not this assessment. |
| Device twin (L1) built from NDA hardware data | L1 | **Depends on the device** | A quantum-PNT payload may itself be controlled; the twin inherits scrutiny. Screen per engagement. |

## 3. Control gates (operational rules until classification is final)

1. **Default-controlled for L2.** Treat all L2 inputs and outputs as controlled
   until a documented screen says otherwise. Do not place them in any repo (see
   `.gitignore`); do not transmit them across borders or to foreign nationals
   without an authorisation check.
2. **No L2 work starts** (PR12–PR14) until: (a) this assessment is reviewed by
   counsel, (b) a per-item classification is recorded, and (c) the customer's
   eligibility/end-use is screened.
3. **Open core stays clean.** Never move real threat data, waveforms, or controlled
   performance figures into the open repo. The open/closed boundary is also the
   controlled/uncontrolled boundary for resilience.
4. **Intangible transfer awareness.** Disclosing controlled technology to a
   non-cleared person — including by email, screen-share, or repo access — can be a
   controlled export. Access control on L2 is a compliance control, not just a
   product feature.
5. **End-use / end-user screening.** Screen customers against EU/national sanctions
   and denied-party lists before any controlled disclosure.

## 4. Actions before L2 (checklist)

- [ ] Counsel review of this assessment (DE/EU dual-use specialist).
- [ ] Per-item classification (ECCN-equivalent / EU Annex I category) for each L2
      artifact, recorded in a control log.
- [ ] BAFA ruling where classification is uncertain (esp. waveforms / performance).
- [ ] Customer end-use/end-user screening procedure documented.
- [ ] Access-control + audit mechanism for L2 artifacts specified (need-to-know).
- [ ] Confirm the "publicly available / basic scientific research" carve-out for the
      open core in writing.

## 5. Record of determinations

| Date | Item | Determination | Basis | Reviewer |
|------|------|---------------|-------|----------|
| — | — | (none yet — L2 not started) | — | — |
