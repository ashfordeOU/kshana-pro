<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

> **INTERNAL GATE — delete this box before the page goes to anyone outside Ashforde OÜ.**
>
> This offer may not be sent, quoted or delivered yet. Four gates stand in front of it:
>
> 1. **Export control.** A timing-resilience and holdover study is the "Pack B" line in
>    `legal/EXPORT-CONTROL-DETERMINATION.md`, whose section 5 says Pack B packaging and
>    sale stay **on hold** until a documented export-control determination exists. The
>    pack to obtain one is `commercial/export-control/CLASSIFICATION-REQUEST-PACK.md`.
> 2. **Price.** Every price field below is a founder-set placeholder (pricing is
>    deferred to the end by founder decision; see `legal/PRICING.md`).
> 3. **Contract.** The commercial licence and the order form are working drafts awaiting
>    counsel (`commercial/counsel/COMMERCIAL-LICENCE-FINALISATION.md`).
> 4. **Payment.** No business bank account exists yet (`commercial/GO-LIVE-CHECKLIST.md`).
>
> Capability check (read from this repository on 2026-09-25, see "How the study is
> computed" below): the holdover prediction, the time-error and maximum time interval
> error mask checks, and the device calibration exist in the Kshana Pro code. Two things
> the scope promises are **study-team work, not a packaged command yet**: turning a
> customer's phase or time-interval-error log into the calibration input, and the
> long-duration aging and temperature behaviour for a named oscillator. The sensitivity
> table and the oscillator-class comparison are also assembled by the study team from
> repeated runs (the device presets exist; a one-command report does not). Two more
> deliverable gaps: the dossier has **no PDF face yet** (HTML and JSON only; a PDF is made
> by printing the HTML), and the Pro MTIE check compares against **flat time-error
> ceilings**, not the observation-interval-dependent ITU-T MTIE breakpoint tables. The
> breakpoint tables ship in the open engine's `telecom-timing` kind from the release after
> 0.27.2; until Pro is rebuilt on that release, run the mask check with that kind. Do not
> promise a turnaround until the study team has run one end-to-end on public data.

# Timing holdover sizing study for critical infrastructure

**A fixed-scope study from Ashforde OÜ, computed on the open Kshana engine.**

How long can your site keep correct time when the satellite signal goes away, and is
the oscillator you have (or plan to buy) good enough for the requirement you are held
to? This study answers that for one class of site, in the units your specification is
written in, with a report anyone can re-run.

*Abbreviations, spelled out once:* GNSS (Global Navigation Satellite System, for example
the Global Positioning System (GPS) and Galileo), PNT (positioning, navigation and timing), TIE (time interval error),
MTIE (maximum time interval error), ITU-T (International
Telecommunication Union Telecommunication Standardization Sector), PRTC (primary
reference time clock), ePRTC (enhanced primary reference time clock), OCXO
(oven-controlled crystal oscillator), CSAC (chip-scale atomic clock), UTC (Coordinated
Universal Time), PDF (Portable Document Format), HTML (HyperText Markup Language), JSON
(JavaScript Object Notation).

---

## Who it is for

Operators of critical national infrastructure that take time from GNSS: mobile and fixed
telecom networks, electricity grids, and financial trading and settlement systems; and
the test houses and timing laboratories that advise them.

## The question it answers

For **one site class** (a group of sites that share the same timing architecture and
requirement):

1. Starting from the moment GNSS is lost, **how long does the local clock stay inside
   your time-error budget**, with an uncertainty band, not a single best-case number?
2. Does the clock hold over for **the outage you must survive** (the GNSS-reacquisition
   interval or outage duration your policy names)?
3. Does the coasting clock stay inside the **MTIE mask** of the specification you name,
   at every observation interval?
4. If not, **what oscillator class** would, and what does the answer depend on most?

## What you provide

| Input | Detail |
|---|---|
| Phase or TIE logs | Time error of the site's clock against a reference (for example UTC, a GNSS receiver or a primary reference), exported as comma-separated values with timestamps. Several days is better than hours, because long-term drift is what dominates holdover. |
| Oscillator datasheets | For each oscillator in scope (for example OCXO, rubidium, caesium or CSAC): the published frequency-stability (Allan deviation) curve, aging rate and temperature sensitivity. |
| Site class definition | The timing architecture, the time-error budget the site must meet, the specification clause it comes from, and the outage duration to survive. |
| Environment | The temperature range the oscillator actually sees at the site, if known. |
| A named contact | Someone who can answer questions about the logs within the study timeline. |

Your data stays yours. It is handled under the data processing terms in the
engagement contract and is not placed in any public repository.

## What you receive

A **Kshana Pro study dossier**, as HTML (printable to PDF) and machine-readable JSON, containing:

- the **holdover curve**: time error against time since GNSS loss, with a stated
  coverage band;
- the **in-specification coast time** against your time-error budget, and a clear
  **holds / does not hold** answer for your outage duration;
- the **MTIE of the coasting clock** checked against your named mask, point by point,
  with **PASS or FAIL** at each observation interval, and each mask breakpoint cited to
  the clause of the standard it comes from (for example ITU-T G.8272 PRTC or G.8272.1
  ePRTC classes);
- the **calibration** of the clock model to your logs and datasheets, showing how well
  the model reproduces the measured stability;
- a **sensitivity table**: which input moves the answer most (typically the long-term
  drift and aging terms, not the short-term stability);
- an **oscillator-class comparison** for the site class;
- a **provenance record** on every figure: whether it is **VALIDATED** (reproduces a
  published reference) or **MODELLED**, plus the engine version, scenario fingerprint
  and random seed, so anyone with the same inputs gets the same numbers.

The related method, bounding the time error a clock can accumulate undetected during
holdover when GNSS is spoofed, is published as an arXiv preprint,
*A Conditional Timing Protection Level: Holdover-Limited Undetected Time Error Under GNSS
Spoofing* ([arXiv:2606.24210](https://arxiv.org/abs/2606.24210)). It is a preprint and
has not been peer-reviewed.

## Timeline

| Step | Duration |
|---|---|
| Kick-off call and input checklist | `[FOUNDER-SET]` |
| Input review; questions back to you | `[FOUNDER-SET]` working days from receiving complete inputs |
| Analysis and draft dossier | `[FOUNDER-SET]` working days |
| Review call and final dossier | `[FOUNDER-SET]` working days after your comments |

The clock starts when complete inputs arrive. Missing or unusable logs move every date.

## Price

| Item | Price |
|---|---|
| Study, one site class | `[€PLACEHOLDER — founder-set]` |
| Each additional site class, same engagement | `[€PLACEHOLDER — founder-set]` |
| Review of the dossier with your auditor or test house (optional) | `[€PLACEHOLDER — founder-set]` |

Payment terms `[FOUNDER-SET]`. Prices exclude any applicable taxes. This page is not a
quotation; a signed order form under the Kshana Pro Commercial Licence Agreement is.

## Assumptions

- The logs are genuine records of the site's clock and are long enough to show drift.
- Oscillator figures come from the manufacturer's published datasheets unless you
  supply measured data.
- The requirement, mask and outage duration are yours to name; we cite the clause you
  give us and do not choose your compliance target for you.
- The coasting model is the open Kshana holdover model: white and random-walk
  frequency noise and deterministic drift. It has no flicker-noise term; where flicker
  matters, the dossier says so and shows its effect separately.

## What the study does not do

- It is **not a certification**, a compliance test or a type approval. It is modelled
  decision-support evidence that feeds your own verification.
- It does **not measure hardware**. There is no laboratory or field test in scope;
  where you need one, we can recommend a timing laboratory or test house.
- It does **not** model or supply real jamming or spoofing threat signals, adversary
  waveforms or anti-jam performance data.
- It does **not** redesign your network, choose a vendor, or guarantee field
  performance.
- It does **not** cover sites outside the named site class.

## How the study is computed

On the open **Kshana** engine (licensed under the GNU Affero General Public License,
version 3, so the core method can be inspected and re-run), with the private **Kshana
Pro** overlay adding the study workflow: device calibration from a published or measured
Allan-deviation budget, the banded holdover prediction, the time-error and MTIE mask
checks, and the dossier. Each figure in the dossier carries its provenance label.

## Next step

Write to **contact@ashforde.org** with the site class you have in mind. We reply with
the input checklist and a scoping call.

---

*Ashforde OÜ · registry code 17321180 · Tallinn, Estonia · contact@ashforde.org*
