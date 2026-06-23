<!-- PROPRIETARY — Kshana Pro overlay. NOT Apache-2.0. See LICENSE. -->

# Pack A — Outreach Playbook

**Owner:** Ashforde OÜ · **Status:** working draft · **Date:** 2026-06-24

The repeatable outreach motion to land the first Pack A design partner. All outreach is sent
from **[contact@ashforde.org](mailto:contact@ashforde.org)**. Lead with the published
preprint as credibility, offer a discounted **design-partner** study in exchange for a
reference / case study, and keep every claim honest: the deliverable is **MODELLED** and
**not a certification**.

Use the prospect list from [`ICP-AND-TARGET-LIST.md`](ICP-AND-TARGET-LIST.md) and qualify with
[`QUALIFICATION-AND-OBJECTIONS.md`](QUALIFICATION-AND-OBJECTIONS.md).

---

## The principles

- **Credibility first.** Open with the published method (arXiv:2606.22054) and an open,
  auditable engine — not with a sales pitch.
- **Be useful before asking.** Offer the sample study and the open engine; the recipient gets
  value even if they never buy.
- **Stay honest, always.** MODELLED, not a certification, an arXiv preprint (not peer-reviewed),
  no TRL/heritage/endorsement claims. Honesty is why technical buyers trust us.
- **One specific ask.** A 20-minute call to see if a design-partner study fits.
- **No pricing in cold outreach.** Pricing is available on request / per the order form.

---

## Subject-line options (cold email)

Pick one; keep it specific and non-hyped:

- *How much does your GNSS detector degrade out-of-distribution?*
- *The optimism gap in interference detectors — a reproducible study*
- *Bench-to-field AUC drop for your detector (design-partner study)*
- *Reproducible field-vs-bench evidence for your PNT detector*
- *arXiv:2606.22054 → a study on your own detector*

---

## Cold email template

> **From:** contact@ashforde.org
> **Subject:** *The optimism gap in interference detectors — a reproducible study*
>
> Hi `[first name]`,
>
> I came across `[specific, true signal — e.g. your ION GNSS+ paper on …, your work on …,
> your team's interference-detection product]`, so this may be relevant.
>
> Most GNSS interference detectors score beautifully on the bench and then quietly degrade on
> conditions they weren't trained on. We call that drop the **optimism gap** — the difference
> between in-distribution (bench) and out-of-distribution (field) AUC. We published the
> phenomenon and an in-distribution-only predictor for it as a preprint,
> **arXiv:2606.22054** (it's a preprint — not peer-reviewed).
>
> We turn that method into a study on **your** detector: you send per-sample scores, labels,
> and an `id` / `ood` tag (no model, no raw RF), and you get a branded, reproducible
> **PNT-Resilience Study Report** — your bench-vs-field AUC, a per-class breakdown, and a
> provenance block so you (or anyone) can reproduce it from the scenario + seed + engine
> version. It runs on our **open** engine, so you can audit the maths yourself.
>
> To be clear about what it is: a **MODELLED** engineering study to feed your own V&V —
> **not a certification**, not a guarantee of field performance.
>
> We're taking on a small number of **design partners** at preferential terms in exchange for
> a reference / short case study. Worth a 20-minute call to see if your detector is a fit?
>
> Happy to send the sample report and the open engine either way.
>
> Best,
> `[name]`
> Ashforde OÜ · contact@ashforde.org · ashforde.org

*Keep it to roughly this length. The one personalised, true line at the top is what earns the
read — never fabricate it.*

---

## LinkedIn connection request (short)

> Hi `[first name]` — I work on reproducible GNSS interference-detector evaluation (we
> published the "optimism gap" as arXiv:2606.22054). Would value connecting with people
> working on detection robustness.

## LinkedIn follow-up message (after they connect)

> Thanks for connecting, `[first name]`. We measure the **optimism gap** — how much a GNSS
> interference detector's AUC drops from bench (in-distribution) to field (out-of-distribution)
> — and deliver it as a reproducible, **MODELLED** study report (explicitly not a
> certification) on an open engine you can audit. Method's published as arXiv:2606.22054 (a
> preprint). We're onboarding a few **design partners** at preferential terms for a reference.
> Open to a short call, or shall I send the sample report? Either way — contact@ashforde.org.

---

## 3-touch follow-up cadence

All from contact@ashforde.org. Stop the moment they reply (then handle the reply, below).

| Touch | Timing | Channel | Goal | Gist |
|------|--------|---------|------|------|
| **1** | Day 0 | Email | First value + soft ask | The cold email above. |
| **2** | Day 3–4 | LinkedIn | Multi-channel, low-friction | Connection request + (on accept) the follow-up message. |
| **3** | Day 5–7 | Email reply (same thread) | Give value, lower the bar | "Sending the **sample report** + a link to the **open engine** so you can see the method even if now's not the time. If useful, I'll scope a quick design-partner study." Attach/link the sample. |
| **4** | Day 12–14 | Email reply (same thread) | Polite close-out | "I'll leave it here so I'm not crowding your inbox — if the optimism gap ever moves up your list, reply and I'll pick it straight back up. The preprint's arXiv:2606.22054 if you want the method meanwhile." |

After touch 4 with no reply: move to a low-frequency nurture (e.g. share when a relevant
result or update is published). Never imply urgency that isn't real.

---

## The design-partner offer (what you're actually pitching)

- A **discounted Pack A study** at preferential design-partner terms (figures **available on
  request** / per the order form — never quoted in cold outreach).
- In exchange for a **reference and a short case study** (named, or well-anonymised if they
  prefer).
- Delivered under the standard engagement: NDA → SOW → study → readout (see
  [`../../legal/SOW-TEMPLATE.md`](../../legal/SOW-TEMPLATE.md)). Their inputs and the delivered
  report instance are theirs; we retain the engine and methods.

---

## Reply-handling — "what good looks like"

- **Interested → book the call.** Don't re-sell over email. Confirm a 20-minute slot, send the
  sample report and the intake format ([`../packs/DETECTOR-INTAKE-FORMAT.md`](../packs/DETECTOR-INTAKE-FORMAT.md))
  beforehand, and qualify on the call.
- **"Send me more."** Send the [sales one-pager](PACK-A-SALES-ONEPAGER.md), the
  [sample report](../packs/sample-optimism-gap-study.html), and the open engine link. Propose a
  call as the next step.
- **A real objection** (certified? build it ourselves? TRL? peer-reviewed? trust the numbers?)
  → answer straight from [`QUALIFICATION-AND-OBJECTIONS.md`](QUALIFICATION-AND-OBJECTIONS.md).
  Never overclaim to win the point.
- **"No budget / not now."** Ask to stay in touch, offer the sample + open engine as a goodwill
  drop, and note the trigger to circle back (a release, a trial, a milestone).
- **Wrong person.** Ask for a warm intro to the algorithm lead / PNT-resilience owner.

**A great reply** is one where the prospect names a real detector and real labelled ID/OOD
data and a real decision the number would inform — that's a design partner. Route them to the
call and the SOW.

---

*All outreach from **[contact@ashforde.org](mailto:contact@ashforde.org)**. MODELLED study,
not a certification. arXiv:2606.22054 is a preprint, not peer-reviewed.*
