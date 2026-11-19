<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
DRAFT response to the Dstl RFI. Nothing has been sent. Founder to review, convert to Word/PDF and send.
-->

# Draft RFI response: Dstl "Resilient Timing and Positioning System for Operation in Degraded GNSS Environments"

**Notice:** Find a Tender 2026/S 000-107427 (UK2 preliminary market engagement), OCID
ocds-h6vhtk-07b9ac, published 11 Nov 2026, edited 12 Nov 2026. Buyer: Defence Science and Technology
Laboratory (Dstl), Porton Down. https://www.find-tender.service.gov.uk/Notice/107427-2026

**RFI deadline: Wed 9 Dec 2026, 17:00 (UK time).** The notice also shows an "engagement deadline" of
10 Dec. Use 9 Dec.

**How to send:**
- One Word or PDF file, by e-mail to the Dstl address printed in the notice. **The founder copies the
  address from the notice itself; do not use any other address.**
- Subject line: *RFI Response – Resilient Timing and Positioning System for Operation in Degraded GNSS
  Environments*.
- Covering letter with company details first, then the proposed solution against each information item.

**Source of the requirement text:** RNTF summary of the notice (12 Nov 2026) and GPS World. The Find a
Tender page returned HTTP 403 to automated fetches. **Read the notice in full before sending** and fix
any wording that differs.

---

## Positioning (read before editing)

- Dstl asks for a **commercially available, proven PNT system** (goods, CPV 38112100), **TRL 9**.
- **Kshana is not that.** It is software for test, assurance and evidence. It has no antenna and no
  oscillator, and it outputs no 1PPS or 10 MHz.
- An honest response therefore does **not** offer a system. It does three things:
  1. tells Dstl plainly that Ashforde does not supply a timing/positioning unit;
  2. offers what Ashforde does supply: an independent, pre-registered **test-and-evidence layer** that
     checks any candidate system against the four measurable requirements (±30 ns, < 5 m, 12 h holdover,
     jamming/spoofing including initial acquisition), with signed and re-checkable results;
  3. offers to act as the **evidence partner of a hardware supplier** that bids the system.
- The aim is to put "independently verifiable evidence" into Dstl's tender specification (expected
  4 Jan 2027). Winning the £100k goods contract is not the aim.

---

## Covering letter (draft)

> **RFI Response – Resilient Timing and Positioning System for Operation in Degraded GNSS Environments**
> Reference: Find a Tender 2026/S 000-107427
>
> Dear Sir or Madam,
>
> Ashforde welcomes this preliminary market engagement and responds with information on independent
> test and evidence for resilient timing and positioning systems.
>
> **We do not offer a timing and positioning unit, and we are not a TRL 9 hardware supplier.** Ashforde
> develops Kshana, software for testing PNT equipment and producing evidence about it. We respond
> because the requirement asks every supplier for evidence:
> - timing and position accuracy;
> - holdover over 12 hours with all external sources lost;
> - performance under jamming and spoofing, including during initial acquisition.
>
> We believe Dstl will want that evidence to be comparable across suppliers and checkable by Dstl itself.
> Below we describe how we could provide that, either to Dstl directly or as the test partner of a
> hardware supplier.
>
> **Company details**
> - Ashforde OÜ, private limited company, Estonia. Registry code 17321180. Ahtri tn 12, 15551 Tallinn.
>   SME.
> - Operations in Frankfurt, Germany. A German company, Ashforde UG (haftungsbeschränkt), is in formation.
> - Contact: Chakshu Baweja, founder · contact@ashforde.org · ashforde.org.
> - Not VAT-registered.
> - No US-origin controlled components in our software *[founder: confirm, see item 15]*.
>
> Yours faithfully,
> Chakshu Baweja, Founder, Ashforde

---

## Response against the information items

The table follows the information list in the notice (as summarised by RNTF). "N/A" means the item
describes a hardware unit Ashforde does not supply.

| # | Information item | Ashforde response |
|---|---|---|
| 1 | System description | **Not a PNT system.** Kshana is software for PNT test and evidence. It runs on a standard computer and works from recorded receiver/clock logs and from scenario files, either alongside a laboratory's GNSS simulator or on recordings taken in the field. It scores the device under test against bars written before the test, and it signs the result so others can check it offline. |
| 2 | Timing accuracy and evidence (±30 ns to UTC, 1PPS and 10 MHz) | N/A as a supplier. **As the test layer:** Kshana ingests time-interval-counter readings (1PPS of the device under test against a UTC-traceable reference) and clock logs in common formats (TICC, RINEX clock, CGGTTS, ptp4l, chrony and others). It reports offset, TDEV/MTIE and drift, with stated uncertainty, against a user-set limit such as ±30 ns. MODELLED: the method uses the GUM (combined and expanded uncertainty), and the arithmetic is checked by internal tests. Accuracy depends on the counter and reference used. |
| 3 | Position accuracy (< 5 m static vs surveyed truth) | N/A as a supplier. **As the test layer:** scores logged fixes (UBX, RINEX, NMEA, Android) against surveyed truth with pass/fail bars written before the test. |
| 4 | Holdover (12 h, all external PNT lost) | N/A as a supplier. **As the test layer:** a scripted 12 h all-source denial in the laboratory scenario, with the time error measured against the reference and the time of any breach recorded. **Holdover *prediction* is not a validated claim today.** Two earlier predictors missed their pre-registered bars on real clocks. A third passed every bar on new data (16 Nov 2026), but the product still runs the older predictor and labels its results "not validated". Measurement of an actual 12 h test does not depend on prediction. |
| 5 | Signal sources | N/A. Kshana simulates or analyses GNSS, eLoran (LF) and other sources. It is a test tool, not a source. |
| 6 | UTC traceability | N/A as a supplier. For tests, traceability comes from the laboratory's reference (for example a UTC(k)-steered reference). Kshana records the reference chain and its uncertainty in the evidence pack. |
| 7 | Sensor fusion | N/A as a supplier. For eLoran-capable candidates, Kshana Pro includes an eLoran/GNSS simulator and hybrid-fusion integrity tools. MODELLED; **not yet run on hardware or against live eLoran signals.** |
| 8 | Anti-jam / anti-spoof measures and evidence (incl. initial acquisition) | N/A as a supplier. **As the test layer:** a library of 12 curated real interference events from published descriptions (Baltic, Strait of Hormuz, Red Sea, JammerTest). Each scenario carries its provenance; no third-party recordings are redistributed. Includes a 6 h timing-denial profile and meaconing/displacement cases. Referee scorecards score a receiver's logs under these cases against pre-registered bars. A cold-start case (initial acquisition under attack) can be scored the same way when the campaign includes one; it is not yet a ready-made library case. Results are test evidence **mapped to** the DHS Resilient PNT Conformance Framework levels and IEEE P1952 draft topics. They never state that a receiver is compliant or certified. Synthetic attack waveforms are produced only as files (never radiated), for vetted buyers, under licence. |
| 9 | Acquisition capability | As item 8. |
| 10 | Timing and data outputs | N/A. |
| 11 | Oscillator technology | N/A. Kshana's timing tools accept oscillator classes (TCXO, OCXO, Rb, CSAC, Cs) and datasheet Allan-deviation points as priors. |
| 12 | External dependencies (no fibre, no user-provided signals, no temporary references) | Kshana has **no dependency on fibre timing networks** or user-generated signals; it is software. A test campaign needs a laboratory reference, as any test does. That reference is part of the test bench, not of the fielded system. |
| 13 | Environmental constraints, SWaP | N/A (software; runs on a standard laptop or server). |
| 14 | Indicative unit cost | N/A for a unit. For independent evidence on one candidate system, a fixed-price study scoped to the four headline requirements: *[founder to set a figure, or write "on request"]*. |
| 15 | Operational use; TRL evidence | **Kshana is not TRL 9 and is not a fielded PNT system.** TRL is the wrong scale for a test tool. The relevant evidence: an open-source engine (AGPL-3.0, dual-licensed) whose validated parts are checked against independent published references in continuous integration, and pre-registered results with misses published. Pre-revenue: **no customers yet**, and no deployment with any defence user. **ITAR:** the software is developed in the EU by Ashforde and contains no ITAR-controlled components *[founder: confirm with the export-control record in `commercial/export-control/` before sending]*. |

## Possible arrangement with a hardware supplier (paragraph for the response)

> Where a supplier proposes a TRL 9 system, Ashforde can act as its independent test partner. Before any
> testing, we would agree with Dstl and the supplier the test cases and pass bars for the four headline
> requirements: timing ±30 ns, static position < 5 m, 12 h holdover with all external sources lost, and
> jamming/spoofing including initial acquisition. We would then run or witness the tests in the
> supplier's or a third-party laboratory and deliver a signed evidence pack that Dstl can re-check offline.
> The supplier keeps the system responsibility; Ashforde provides evidence only, never a compliance
> statement.

## Questions to Dstl (include at the end of the response)

1. **Eligibility of a non-UK SME.** Ashforde is an SME established in Estonia (EU), with a German
   company in formation, and has no UK establishment. Can a non-UK supplier respond to this RFI and take
   part in the subsequent tender, either as a supplier or as a subcontractor or test partner to a supplier?
   Are there security, clearance or establishment requirements (for example UK security clearance of
   staff or facility, or a UK company) that would exclude a non-UK SME?
2. **Scope for independent test and evidence.** Will the tender allow, or ask for, independent
   verification of the jamming, spoofing and holdover evidence submitted by suppliers? Would an
   evidence-only role (test partner to a supplier, or a separate service to Dstl) be in scope, given that
   the requirement is classed as goods?
3. **Test conditions.** Will Dstl define the jamming/spoofing test conditions (threat types, power
   levels, the initial-acquisition case) in the tender, so that suppliers' evidence is comparable?

---

## Before sending (founder checklist)

- [ ] Read the full notice on Find a Tender; copy the Dstl e-mail address from the notice; check the
      information list matches the table above.
- [ ] Decide the sender entity: Ashforde OÜ (exists) is the safe choice. Mention the UG only as "in
      formation".
- [ ] Confirm the export statement (item 15) against `commercial/export-control/`. Spoofing scenarios go
      to vetted buyers only.
- [ ] Item 14: set a figure or write "on request".
- [ ] Optional: name a hardware supplier only with its written agreement.
- [ ] Convert to Word or PDF (≤ 6 pages). Send by **Wed 9 Dec 2026, 17:00 UK time**.
