<!-- PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE. -->

# Pack A — Qualification & Objection Handling

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

How to tell a real Pack A design partner from a not-yet, and how to answer the hard questions
honestly. Every answer here stays inside what we can defend: the study is **MODELLED** and
**not a certification**; the method is an **arXiv preprint** ([arXiv:2606.22054](https://arxiv.org/abs/2606.22054)),
**not peer-reviewed**; we make **no** TRL, flight-heritage, ESA-endorsement, or regulatory
claims. Honesty is the moat — never trade it for a yes.

Pair this with [`ICP-AND-TARGET-LIST.md`](ICP-AND-TARGET-LIST.md) (who) and
[`OUTREACH-SEQUENCE.md`](OUTREACH-SEQUENCE.md) (how you reached them).

---

## Qualifying questions

Ask these on the first call. They map to the five design-partner criteria.

1. **Detector.** Do you have a working GNSS spoofing / jamming / RF-impairment detector that
   produces a per-sample score (or that we can get a score out of)?
2. **Labelled ID/OOD data.** Do you have ground-truth labels, and can you separate your data
   into bench (in-distribution) and field/shifted (out-of-distribution) so you can produce the
   intake ([`../packs/DETECTOR-INTAKE-FORMAT.md`](../packs/DETECTOR-INTAKE-FORMAT.md))?
3. **V&V need.** Do you have an internal verification & validation (or acceptance / diligence)
   process this evidence would feed into?
4. **Budget authority.** Is there a budget and a person who can approve a small paid
   engagement — or get it approved?
5. **Near-term decision.** Is there a decision in the next `[1–2 quarters]` this number would
   inform — a go/no-go, a customer trial, a milestone review, a tender, a board/investor
   readout?
6. **(Bonus) Reference.** If the study is useful, would you be open to a reference or short
   case study (named or anonymised)? That's the heart of the design-partner deal.

---

## Qualification rubric

| | **Good fit (pursue now)** | **Not yet (nurture)** |
|---|---|---|
| Detector | Working detector that emits / can emit scores | Idea or pre-prototype; no scoreable output |
| Data | Has labelled ID/OOD data, can produce the intake | No labels, or can't separate ID vs OOD |
| V&V | Real V&V / acceptance / diligence process | No place for the evidence to land |
| Budget | Budget + an approver in reach | No budget and no path to one |
| Timing | Decision within ~1–2 quarters | No decision the number informs |
| Reference | Open to a reference / case study | Won't be referenceable under any terms |

**Read:** **4–5 "good fit"** rows (especially detector + data + V&V) → this is a design-partner
candidate; move to SOW. **2–3** → nurture; send the sample report and the open engine, set a
trigger to revisit. **0–1** → not the ICP; be polite, leave the door open, don't over-invest.

---

## Objection-handling table

| Objection | Honest answer |
|---|---|
| **"Is this certified?"** | No — and we're deliberate about that. Pack A is **MODELLED decision-support** for **your own** V&V, with a MODELLED / "not a certification" banner on every report. It gives you a defensible, reproducible number to feed your certification and acceptance work — it does not replace it. Anyone selling you a "certified" robustness number from simulation is overclaiming. |
| **"Why not just build it ourselves?"** | You can — and you can read exactly how, because the engine is **open and auditable** and the methodology is **published** (arXiv:2606.22054). What you get from us is the **reproducible method already built and validated**, branded provenance (scenario hash + seed + engine version), per-figure VALIDATED/MODELLED labelling, and the result **faster** than standing it up internally. Many partners audit our open engine *and* have us run the study. |
| **"What's your TRL / flight heritage?"** | We don't claim either, and we won't. This is **modelled, research-grade tooling — TRL-3-class** — for engineering studies and trade-offs, not a flown or fielded system. There is **no flight heritage** to claim. What we offer is reproducible evidence, not maturity theatre. The honesty is the point. |
| **"Is the result peer-reviewed?"** | No. The underlying method is an **arXiv preprint** (arXiv:2606.22054) — a preprint, **not peer-reviewed**. We state that on the deliverable and everywhere else. The strength isn't a peer-review stamp; it's that the method is **open and the result is reproducible** from the scenario + seed + engine version. |
| **"How do we trust the numbers?"** | Two ways, both verifiable. First, the engine is **open core you can audit** — it's the same public AUC routine for the published study and for yours. Second, every report carries a **provenance block** (engine version, scenario hash, study id) so the result is **reproducible** from scenario + seed + engine version, by you, independently. You don't have to trust us; you can re-run it. |
| **"Can't we just use our bench AUC?"** | Your bench AUC is the **optimistic** number — that's the whole problem. The optimism gap is precisely how much it over-states field performance, and you can't see it from the bench number alone. Pack A quantifies the drop, per impairment class, so you know what you're actually shipping. |
| **"We can't share our model / raw RF."** | You don't need to. The intake is **scores + labels + an `id`/`ood` tag** — no model weights, no raw IQ. That's enough to compute the gap, and it keeps your IP with you. |
| **"What about our data / IP?"** | You own your inputs and the delivered report instance; we retain the engine, methods, and templates. Work runs under NDA and the standard engagement terms ([`../../legal/COMMERCIAL-LICENSE.md`](../../legal/COMMERCIAL-LICENSE.md), [`../../legal/SOW-TEMPLATE.md`](../../legal/SOW-TEMPLATE.md)). |
| **"How much does it cost?"** | **Available on request**, per the order form — and we have preferential **design-partner** terms for the first reference engagements. Let's confirm fit first, then scope and price. |
| **"Is this endorsed by ESA / an agency?"** | No. We make **no** endorsement, certification, or regulatory-approval claims of any kind. It's an independent engineering study you can audit and reproduce. |

---

## The design-partner offer structure

- **What they get:** a discounted Pack A study at preferential design-partner terms (figures
  **available on request** / per the order form), delivered as the branded PNT-Resilience Study
  Report (HTML + PDF) plus the machine-readable result and reproducibility provenance, with a
  readout call.
- **What we get:** a **reference and a short case study** — named, or well-anonymised if they
  prefer — and the learning from a first real customer engagement.
- **How it runs:** NDA → SOW ([`../../legal/SOW-TEMPLATE.md`](../../legal/SOW-TEMPLATE.md)) →
  intake → study → report → readout. Inputs and the delivered report are theirs; Ashforde
  retains the engine and methods.
- **The honest frame, held throughout:** MODELLED, not a certification, an arXiv preprint (not
  peer-reviewed), no TRL/heritage/endorsement claims. That frame is exactly what makes the
  evidence usable inside a serious V&V process — and it's non-negotiable.

---

*All contact via **[contact@ashforde.org](mailto:contact@ashforde.org)**. Pack A is a MODELLED
study, not a certification; arXiv:2606.22054 is a preprint, not peer-reviewed.*
