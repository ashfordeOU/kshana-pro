<!-- PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE. -->

# Pack A — Optimism-Gap Audit

**Know how much your GNSS interference detector will degrade in the field — before you ship it.**

A reproducible, MODELLED study that quantifies your detector's *optimism gap*: the drop
from bench performance (in-distribution) to field performance (out-of-distribution). Built
on the open [Kshana](https://github.com/AshfordeOU/kshana) PNT engine; the methodology is
published as an arXiv preprint, [arXiv:2606.22054](https://arxiv.org/abs/2606.22054)
(a preprint — not peer-reviewed). Delivered by [Ashforde OÜ](https://ashforde.org).

---

## Who it's for

R&D teams building GNSS **spoofing / jamming / RF-impairment detectors**:
- receiver and chipset vendors' algorithm groups,
- defence and space R&D teams,
- university spin-outs taking a detector toward deployment.

## The job it does

You trained a detector and it scores beautifully on your test set. The open question every
serious team has — and few can answer with evidence — is: **how much of that score survives
contact with conditions you didn't train on?** Pack A answers it with a number and a method
you can reproduce, not a vendor's assurance.

## How it works

1. **You send your detector's outputs** — per-sample scores, ground-truth labels, and a
   distribution tag (`id` / `ood`). Schema + worked example:
   [`DETECTOR-INTAKE-FORMAT.md`](DETECTOR-INTAKE-FORMAT.md). (No need to ship us your model
   or your raw RF — scores and labels are enough.)
2. **We compute the optimism gap** — AUC on the in-distribution subset, AUC on the
   out-of-distribution subset, and the gap between them, using the open engine's scoring
   (the same code, public and reproducible).
3. **You get a Study Report** — see below.

## What you get

A branded, reproducible **PNT-Resilience Study Report** (HTML + PDF):
- the **predicted field-vs-bench degradation** (the optimism gap) as the headline,
- in-distribution AUC, out-of-distribution AUC, per-class breakdown,
- every figure labelled **VALIDATED** or **MODELLED** inline,
- an audit-grade **provenance block** (engine version, scenario hash, study id, date),
- reproducible from the scenario + seed + engine version.

Sample: [`sample-optimism-gap-study.html`](sample-optimism-gap-study.html) ·
[`sample-optimism-gap-study.pdf`](sample-optimism-gap-study.pdf) — generated from the
published baseline, with no customer data.

## What it is — and is not

- It **is** decision-support evidence that feeds *your* verification & validation: a
  defensible, reproducible estimate of distribution-shift degradation.
- It is **MODELLED**, and it is **not a certification**, not a guarantee of field
  performance, and not a peer-reviewed result. The underlying method is an arXiv preprint.
- We tell you the uncomfortable number. That honesty is the point of the audit.

## Start a study

Contact **[contact@ashforde.org](mailto:contact@ashforde.org)** · [ashforde.org](https://ashforde.org).
Pack A studies are delivered under a commercial engagement (Pro tier). Scope, turnaround,
and pricing are set per engagement — see the rate card (figures provided on request).

---

*Kshana is a performance simulator for studies and trade-offs, not a navigation product.
"Kshana" and associated marks are trademarks of Ashforde OÜ.*
