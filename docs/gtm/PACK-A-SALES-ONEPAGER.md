<!-- PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE. -->

# Pack A — Optimism-Gap Audit

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

### Know how much your GNSS interference detector will degrade in the field — before you ship it. With a number you can reproduce.

*Buyer-facing one-pager. For the technical spec, see [`../packs/PACK-A.md`](../packs/PACK-A.md).*

---

## The problem: the optimism gap

Your detector scores beautifully on the bench. On your test set it looks finished.

Then it meets conditions it wasn't trained on — a subtler spoof, an unfamiliar jammer, a
multipath environment from a different site — and the score quietly drops. The gap between
**bench performance (in-distribution)** and **field performance (out-of-distribution)** is
the *optimism gap*, and most teams discover its size the expensive way: in production, in a
trial that fails, or in a customer's deployment.

Every serious detector team knows the question — *how much of my bench number survives
contact with the field?* — and few can answer it with evidence rather than a hope. That
unanswered question stalls go/no-go decisions, weakens your story to your own customers, and
turns a shippable detector into a risk nobody will sign off on.

## What you get

A branded, reproducible **PNT-Resilience Study Report** (HTML + PDF) that puts a defensible
number on your optimism gap:

- **The headline:** your predicted field-vs-bench degradation (`AUC_id − AUC_ood`) as the
  top-line figure — the optimism gap itself.
- In-distribution AUC, out-of-distribution AUC, and a **per-impairment-class breakdown**
  (e.g. jamming, spoof, multipath) so you see *where* the gap lives.
- **Every figure labelled inline as VALIDATED or MODELLED** — no ambiguity about what is a
  measured engine output versus a simulation result.
- An audit-grade **provenance block**: engine version, scenario hash, study id, date — so the
  study is reproducible from the **scenario + seed + engine version**, by you or by us.
- A clear **MODELLED / "not a certification"** banner, because that is exactly what it is.

See the real deliverable: [`../packs/sample-optimism-gap-study.html`](../packs/sample-optimism-gap-study.html)
(generated from the published baseline, with no customer data).

## Why Ashforde

- **An open engine you can audit.** The study runs on the open
  [Kshana](https://github.com/AshfordeOU/kshana) PNT engine — the same public, reproducible
  AUC routine for everyone. You don't have to trust a black box; you can read it.
- **A published method behind it.** The optimism-gap phenomenon and the in-distribution-only
  gap predictor are established in **[arXiv:2606.22054](https://arxiv.org/abs/2606.22054)** —
  an arXiv preprint (not peer-reviewed). The method is in the open, not invented for the sale.
- **Honest labelling, by design.** We tell you the uncomfortable number, label every claim
  MODELLED or VALIDATED, and never dress up a simulation as a certification. That honesty is
  the reason the evidence is usable inside your own V&V.

## How it works — 4 steps

1. **Intake.** You send your detector's scored outputs in our simple format — per-sample
   scores, ground-truth labels, and a distribution tag (`id` / `ood`). No model, no raw RF.
2. **Run.** We split by distribution and compute AUC on each subset and the gap between them,
   using the open engine's exact AUC routine — the same code the published study uses.
3. **Report.** You receive the branded PNT-Resilience Study Report (HTML + PDF), the
   machine-readable result, and the scenario hash + seed + engine version.
4. **Readout.** A short call to walk through the number, the per-class breakdown, and how it
   feeds your verification & validation.

## What we need from you

Just your detector's outputs, per the intake format
([`../packs/DETECTOR-INTAKE-FORMAT.md`](../packs/DETECTOR-INTAKE-FORMAT.md)): a flat list of
samples, each with a **score** (oriented so higher = more impaired), a ground-truth **label**,
and a **distribution** tag (`id` = bench, `ood` = field). That's it — scores and labels are
enough. We never need your model weights or your raw IQ.

## What it is — and is not

- It **is** reproducible decision-support evidence that feeds *your* V&V: a defensible,
  auditable estimate of distribution-shift degradation.
- It is **MODELLED**, and it is **not a certification**, not a guarantee of field performance,
  not peer-reviewed, and not a TRL or flight-heritage claim.

---

## Start a study

Email **[contact@ashforde.org](mailto:contact@ashforde.org)** — tell us you have a detector
and labelled ID/OOD data, and we'll scope a study. Design-partner terms are available for the
first reference engagements. Scope, turnaround, and **pricing are available on request** (per
the order form).

[ashforde.org](https://ashforde.org) · *Kshana is a performance simulator for studies and
trade-offs, not a navigation product. Marks are trademarks of Ashforde OÜ.*
