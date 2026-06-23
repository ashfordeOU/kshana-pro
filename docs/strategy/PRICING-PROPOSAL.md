> **INTERNAL — PRICING PROPOSAL FOR FOUNDER DECISION.** The figures below are
> **recommendations, not committed prices.** The binding [`legal/PRICING.md`](../../legal/PRICING.md)
> retains its `[FOUNDER-SET PLACEHOLDER]` values until the founder approves numbers
> here and ports them over. Nothing in this document is an offer, a quote, or a
> commitment to any customer.

# Kshana Pro — Pricing Proposal (Recommendation)

**Owner:** Ashforde OÜ · **Status:** proposal for founder decision · **Date:** 2026-06-24

---

## 0. How to read this document

This is a **recommendation document**, not a rate card. It exists so the founder can
make a small number of explicit pricing decisions quickly and then port approved
numbers into the binding [`legal/PRICING.md`](../../legal/PRICING.md). Every range below
is a **starting point** for that decision, chosen to be defensible and easy to adjust —
not a fixed figure.

What is being priced is **MODELLED decision-support evidence**: reproducible studies that
feed the customer's own verification and validation. A study is **not a certification**,
not a warranty of field performance, and not a peer-reviewed result. The underlying
optimism-gap method is published as an **arXiv preprint
([arXiv:2606.22054](https://arxiv.org/abs/2606.22054)) — a preprint, not peer-reviewed.**
There are no TRL, heritage, or endorsement claims attached to any price here, and none
should be added when these numbers are ported over.

All currency below is shown in EUR as a working convention only; **currency is a founder
decision** (see §8).

---

## 1. Pricing philosophy

Four principles drive the recommended numbers:

1. **Value-based, not cost-plus.** A Pack A study is priced against *the cost of an
   optimism-gap surprise in production* — a detector that scored well on the bench and
   then degraded in the field — plus the **V&V effort the customer saves** by getting a
   reproducible distribution-shift estimate instead of building the harness themselves.
   The price anchor is "what is it worth to know the uncomfortable number before you
   ship," not "how many days did the study take."

2. **Open-core as funnel, not revenue.** The open Kshana engine and the thin reproducible
   report are **lead generation**. They establish credibility (anyone can reproduce the
   baseline) and lower the trust barrier. We do **not** try to monetize the open layer;
   revenue comes from the branded study, the curated scenario subscription, and support.
   The open layer's job is to bring qualified detector-R&D teams to `contact@ashforde.org`.

3. **Design-partner discounting for early references.** The first customers are worth more
   as *references* than as *revenue*. Discount the first cohort deliberately and in
   exchange for a nameable reference, a logo, a quote, or a co-authored result — never as
   an unstructured giveaway. The discount is a one-time cohort policy, not a list price.

4. **Land-and-expand up the ladder.** The motion is: land with a one-off study (Pack A),
   prove value, then expand into recurring scenario subscription (Pack C) and, where
   clearance permits, partner-led timing work (Pack B). Price each rung so the *next*
   rung is the obvious next purchase, and so a study fee can be partially credited toward
   a subsequent subscription (see §7).

---

## 2. The monetization ladder (recommended structure)

The ladder mirrors [`PRODUCTIZATION-ROADMAP.md`](PRODUCTIZATION-ROADMAP.md): paid
design-partner study → productized study → recurring scenario subscription, with Pack B
as a later, gated, partner-led rung. Each rung maps onto an Eval / Pro / Enterprise tier
from [`COMMERCIAL-LICENSE.md`](../../legal/COMMERCIAL-LICENSE.md).

| # | Offering | Motion | Recurrence | Maps to tier |
|---|---|---|---|---|
| 0 | Evaluation / scoping | qualify, scope, sample report | one-off, often free/time-boxed | **Evaluation** |
| 1 | Pack A — design-partner study | services-led, discounted for reference | one-off | **Pro** (engagement) |
| 2 | Pack A — productized study | services-led, standard list | one-off, repeatable | **Pro** (engagement) |
| 3 | Pack C — scenario subscription | product-led, recurring | annual | **Pro → Enterprise** |
| 4 | Pack B — timing resilience | partner-led, clearance-gated | TBD | **Enterprise** (gated) |

### Rung 0 — Evaluation / scoping (the on-ramp)

- **Recommended price:** **€0, time-boxed**, exactly as the Evaluation tier in
  `COMMERCIAL-LICENSE.md` contemplates.
- **What it includes:** a scoping call, the public **sample study report**
  ([`docs/packs/sample-optimism-gap-study.pdf`](../packs/sample-optimism-gap-study.pdf)),
  and confirmation that the customer's detector outputs fit the intake format
  ([`DETECTOR-INTAKE-FORMAT.md`](../packs/DETECTOR-INTAKE-FORMAT.md)).
- **Rationale:** removes friction at the top of the funnel; costs almost nothing; converts
  interest into a scoped Pack A. Keep it explicitly time-boxed so it does not become free
  consulting.
- **Tier mapping:** **Evaluation** — community/best-effort support, no licence fee.

### Rung 1 — Pack A paid design-partner study (one-off, discounted for a reference)

The first paying engagements. Discounted **in exchange for a reference** (named logo,
testimonial quote, or a co-presentable anonymized result), not discounted by default.

- **Recommended range:** **€6,000 – €15,000 per study** (design-partner rate), i.e. the
  standard list price (Rung 2) **less the design-partner discount** in §7. As a sanity
  check, this is roughly **40–60% of the productized list price**.
- **Cohort cap:** recommend limiting the design-partner rate to the **first 3–5
  reference customers**, then retiring it. Scarcity protects the list price.
- **Rationale:** low enough that an R&D team can approve it without a procurement cycle,
  high enough that the customer treats the engagement seriously and the reference is
  earned. The reference value (credibility for the next sale) is the real "discount
  recovery."
- **Tier mapping:** **Pro** — delivered under a Pro-tier commercial engagement + order
  form; one-off study fee, not an annual licence.
- **Binding-list target:** sets `Design-partner pilot (discounted…)` in `PRICING.md`.

### Rung 2 — Pack A productized study (standard list price, once repeatable)

The standing list price once the study is repeatable and no longer needs a reference
discount.

- **Recommended range:** **€12,000 – €25,000 per study** (standard), with **€18,000** a
  reasonable single-anchor if the founder wants one headline number.
- **Add-on — additional detector / scenario set:** **€2,500 – €6,000 per additional unit**
  (a second detector, a different impairment family, or an extra scenario set analyzed in
  the same engagement). Marginal effort is low, so this is mostly margin and a natural
  expansion lever.
- **Optional bespoke / custom basis:** for non-standard Pack A work or early Pack B/C
  scoping, a **day-rate of €1,200 – €2,000/day**. The founder may anchor this to the
  private tender cost-volume rate **if** consistent — but that figure is tender-specific
  and **must not** become a public list price.
- **Rationale:** the value anchor is the avoided cost of a field-degradation surprise and
  the saved V&V build-out, which sits comfortably in the low five figures for a serious
  detector program. Pricing below that under-signals quality; pricing far above it stalls
  the land motion before any reference exists.
- **Tier mapping:** **Pro** — Pro-tier engagement + order form, Pro SLA.
- **Binding-list target:** sets `Pack A — Optimism-Gap Audit (standard)`, `additional
  detector / scenario set`, and the `Custom study … day-rate` rows in `PRICING.md`.

### Rung 3 — Pack C scenario subscription (annual recurring)

Recurring revenue: a versioned, curated scenario library plus a CI harness the customer
pins each build against (per [`PRODUCTIZATION-ROADMAP.md`](PRODUCTIZATION-ROADMAP.md)
Phase 2). This is the expansion rung after a Pack A reference exists.

**Proposed tiering axis.** Recommend a **primary axis of curated scenario-pack
entitlement + CI run volume**, with **seats** as a secondary modifier. This bills for the
thing that grows with customer value (how much of the curated library they rely on and how
hard they exercise it in CI), rather than for raw usage that is hard to meter cleanly.

| Tier | Who it fits | Scenario entitlement | CI usage | Seats | **Recommended annual range** | Maps to |
|---|---|---|---|---|---|---|
| **C-Starter** | one detector team, regress one product line | 1 curated pack, version-pinned | light (e.g. ≤ daily CI) | 1–3 | **€8,000 – €15,000 / yr** | Pro |
| **C-Team** | a product group, several lines | several curated packs | standard CI cadence | up to ~10 | **€18,000 – €40,000 / yr** | Pro → Enterprise |
| **C-Org / Enterprise** | org-wide / multi-team / prime | full curated library + new releases | high-volume CI | org-wide | **€45,000 – €90,000+ / yr** | Enterprise (+ named contact, Enterprise SLA) |

- **Rationale:** subscription value scales with how central the curated scenarios are to
  the customer's release gate. Scenario-pack count + CI cadence is **inspectable and
  defensible**; pure per-run metering invites gaming and billing disputes. Seats as a
  secondary modifier keeps small teams cheap and large orgs paying for breadth.
- **Tier mapping:** **Pro** at the lower tiers, **Enterprise** at C-Org (org-wide /
  multi-team / primes), matching the Enterprise row and named-contact SLA in
  `COMMERCIAL-LICENSE.md` / `PRICING.md`.
- **Binding-list target:** sets the `Scenario-pack subscription (Pack C, future)` add-on
  and informs the `Pro` / `Enterprise` annual-licence rows in `PRICING.md`.

### Rung 4 — Pack B timing resilience (model only — NO firm numbers)

Pack B (timing-resilience / holdover risk for critical infrastructure) is
**export-control-gated and partner-led**. Per
[`PRODUCTIZATION-ROADMAP.md`](PRODUCTIZATION-ROADMAP.md) Phase 3 and
[`EXPORT-CONTROL-ASSESSMENT.md`](../../legal/EXPORT-CONTROL-ASSESSMENT.md), delivery is
blocked on an export-control determination and customer screening.

- **Pricing recommendation:** **none yet — do not publish or quote a number.** Pricing is
  **TBD after the export-control determination** and the first partner pilot establish the
  delivery model, clearance overhead, and partner revenue split.
- **Recommended model (structure only):** Enterprise-tier, **partner-led engagement**
  (telecom / grid / finance via a partner), priced as a **bespoke clearance-gated study or
  pilot** under NDA + screening, likely at or above the top of the Pack A custom day-rate
  band to reflect clearance and compliance overhead — **but the figure is deliberately
  left open here.**
- **Tier mapping:** **Enterprise**, gated behind clearance and the customer-screening form.

---

## 3. How the rungs map to the COMMERCIAL-LICENSE tiers (summary)

| Tier (`license` module) | Rungs it carries | What the customer gets |
|---|---|---|
| **Evaluation** | Rung 0 | Time-boxed scoping, sample report, community/best-effort support, no fee |
| **Pro** | Rungs 1–2; lower Pack C tiers | One-off studies and/or a single-team scenario subscription; Pro SLA |
| **Enterprise** | Top Pack C tier; Pack B | Org-wide / multi-team / prime access; Enterprise SLA + named contact; gated Pack B |

This keeps the proposal fully consistent with the Eval / Pro / Enterprise structure the
EULA already enforces — no new tiers are introduced.

---

## 4. Comparables and justification (MARKET CONTEXT — estimates, not quotes)

> **Read this section as general market context, not as facts about any named competitor's
> current prices.** The figures below are **rough market estimates** used only to sanity-check
> the ranges above. They are not sourced quotes and should not be cited as such.

- **Professional GNSS simulation and test tooling** from established vendors **typically runs
  into the five- to six-figure range** (perpetual or annual), as a general market
  observation. Against that backdrop, a **low-five-figure one-off study** (Rung 2) and a
  **mid-four-to-five-figure annual subscription** (Rung 3) read as accessible, services-led
  entry points rather than premium platform purchases.
- **Specialist R&D / V&V consulting** is commonly **estimated in the four-figures-per-day
  range**, which is why the recommended **€1,200–€2,000/day** bespoke rate sits within a
  plausible, defensible band.
- **The value anchor** for Pack A is the **avoided cost of a field-degradation surprise**
  (re-work, delayed deployment, a detector that fails in conditions it was never tested
  against) plus the **internally-built distribution-shift V&V harness the customer does not
  have to build and maintain.** For a serious detector program, that avoided cost is
  generally well into five figures — which is what justifies a low-five-figure study price
  without overclaiming.

Honesty note for every comparable above: the deliverable remains **MODELLED**, reproducible
evidence — **not** a certification — so the pricing is positioned as decision-support, not as
a substitute for the customer's own validation.

---

## 5. Discounting and terms (recommendations)

These are **recommended policies**, to be confirmed by the founder and reviewed by counsel
where they touch contract terms.

- **Design-partner discount:** recommend **40–60% off the Rung 2 list price**, granted only
  in exchange for a documented reference (logo + named quote, or a co-presentable anonymized
  result), and **capped at the first 3–5 customers**. Sunset the discount after the cohort.
- **Annual-prepay discount (Pack C):** recommend **~10–15%** for annual prepayment vs.
  any installment option, to pull forward cash and reduce churn risk.
- **Multi-year (Pack C):** recommend **~15–20%** off for a 2–3 year commitment, with
  pricing locked for the term — optional, founder's call on whether to offer it this early.
- **Multi-pack bundle:** recommend **crediting a paid Pack A study (up to ~50% of its fee)
  toward the first year of a Pack C subscription** if the customer subscribes within, say,
  90 days. This makes the land-and-expand path explicit and rewards moving up the ladder.
- **Volume (multiple studies / detectors):** recommend tiered breaks on the
  additional-detector add-on (e.g. lower per-unit price beyond 3 units) rather than
  discounting the base study.
- **Standard commercial terms (recommended defaults, counsel to confirm):**
  - **Currency:** EUR (founder to confirm — see §8).
  - **Payment:** **net 30** (matches the `[net 30]` placeholder in `PRICING.md`).
  - **Taxes:** exclusive of VAT / applicable taxes.
  - **Quote validity:** **30 days**.
  - **Subscription billing:** **annual, in advance.**
  - **Liability / governing law:** per `COMMERCIAL-LICENSE.md` — **counsel to set the
    liability cap and EE-vs-DE governing law**; out of scope for this pricing proposal.

---

## 6. Suggested binding-list values (illustrative single anchors)

If the founder wants one concrete number per line to drop into `legal/PRICING.md`, the
following are **illustrative midpoints of the ranges above** — still recommendations, still
the founder's to overwrite:

| `PRICING.md` line | Illustrative anchor (founder to confirm) |
|---|---|
| Pack A — Optimism-Gap Audit (standard) | **€18,000** |
| — additional detector / scenario set | **€4,000 / unit** |
| Design-partner pilot (discounted) | **€9,000** (≈ 50% of standard) |
| Custom study (bespoke) day-rate | **€1,500 / day** |
| Pack C subscription — entry (C-Starter) | **€12,000 / yr** |
| Pack C subscription — Enterprise (C-Org) | **€60,000+ / yr** |
| Pro annual licence (with study/subscription) | per chosen Pack C tier |
| Enterprise annual licence | per chosen Pack C tier + named contact |

These are **anchors for discussion**, not commitments. The binding file keeps its
`[FOUNDER-SET PLACEHOLDER]` values until the founder writes the chosen numbers in.

---

## 7. Land-and-expand example (illustrative)

1. **Evaluation (€0):** scoping call + sample report; confirm intake fit.
2. **Pack A design-partner study (~€9,000):** first reference customer, discounted for a
   logo + quote.
3. **Pack A productized study (~€18,000):** subsequent customers at list.
4. **Pack C C-Starter (~€12,000/yr):** the same customer pins CI to a versioned scenario
   set; up to ~50% of their Pack A fee credited toward year one.
5. **Pack C C-Org / Enterprise (€60,000+/yr):** expansion to org-wide / multi-team; named
   contact + Enterprise SLA.
6. **Pack B:** only once export-control clearance and a partner are in place — pricing TBD.

---

## 8. Decisions the founder must make (checklist)

Turnkey list to convert this proposal into `legal/PRICING.md`:

**Numbers to set**
- [ ] Pack A **standard** study price (recommended €12,000–€25,000; anchor €18,000).
- [ ] Pack A **additional detector / scenario set** unit price (recommended €2,500–€6,000).
- [ ] Pack A **design-partner** rate (recommended 40–60% off standard; anchor €9,000).
- [ ] Bespoke **day-rate** (recommended €1,200–€2,000/day; reconcile with private tender
      cost-volume rate, keep that rate non-public).
- [ ] Pack C **C-Starter** annual price (recommended €8,000–€15,000).
- [ ] Pack C **C-Team** annual price (recommended €18,000–€40,000).
- [ ] Pack C **C-Org / Enterprise** annual price (recommended €45,000–€90,000+).
- [ ] Pro vs. Enterprise **annual licence** figures (align to chosen Pack C tiers).

**Policy to set**
- [ ] **Design-partner discount %** and **cohort cap** (recommended 40–60%, first 3–5).
- [ ] **Annual-prepay discount %** (recommended ~10–15%).
- [ ] **Multi-year discount %** — offer now or later? (recommended ~15–20% if offered).
- [ ] **Pack A → Pack C credit** — offer it, and at what % / window? (recommended ~50%, 90 days).
- [ ] **Volume break** on additional detectors — yes/no and thresholds.
- [ ] **Public-anchor vs. quote-only** — publish a list price or keep deal-gated?

**Commercial terms to confirm**
- [ ] **Currency** (EUR assumed here — confirm).
- [ ] **Payment terms** (net 30 assumed — confirm).
- [ ] **Quote validity** (30 days assumed — confirm).
- [ ] **Subscription billing cadence** (annual-in-advance assumed — confirm).
- [ ] **Pack B model + numbers** — defer until export-control determination + first partner.

**Counsel-gated (out of scope here, flagged for completeness)**
- [ ] Liability cap, governing law (EE vs. DE), B2B carve-outs — per `COMMERCIAL-LICENSE.md`.

---

## 9. Standing reminder

**Nothing in this document is binding.** These are **recommendations and starting points**
for the founder's decision. The founder sets the final figures; the binding
[`legal/PRICING.md`](../../legal/PRICING.md) retains its `[FOUNDER-SET PLACEHOLDER]` values
until the approved numbers are ported over. **Counsel reviews all contractual terms**
(liability, governing law, export control) before any signed order. All deliverables remain
**MODELLED** decision-support evidence — **not a certification**, not a guarantee of field
performance, and not peer-reviewed.

---

**Contact:** [contact@ashforde.org](mailto:contact@ashforde.org) · Ashforde OÜ
