<!--
PRIVATE — Ashforde internal. kshana-pro repo only. Contact: contact@ashforde.org
-->

# UK Dstl RFI: Resilient timing and positioning system for degraded GNSS environments (2026)

| Item | Value |
|---|---|
| Notice | Find a Tender **2026/S 000-107427**, UK2 preliminary market engagement; OCID ocds-h6vhtk-07b9ac; https://www.find-tender.service.gov.uk/Notice/107427-2026 |
| Buyer | Defence Science and Technology Laboratory (Dstl), Porton Down |
| What | A commercially available, proven PNT system: UTC-traceable 1PPS and 10 MHz, better than ±30 ns; static position better than 5 m; compliant through 12 h loss of all external PNT; works under jamming/spoofing, including initial acquisition; TRL 9; global; no fibre timing, no user-provided signals, no temporary references, no ITAR components |
| Value / term | ≈ £100k ex VAT (below threshold); contract 4 Feb – 26 Mar 2027 (estimated); goods, CPV 38112100; "particularly suited to SMEs" |
| Dates | Published 11 Nov 2026 · **RFI deadline Wed 9 Dec 2026 17:00** · tender expected 4 Jan 2027 |
| Contracts Finder | The RNTF copy of the notice gives no Contracts Finder link. Below-threshold UK notices are often mirrored there, but this one was not found in this pass (unverified). Search Contracts Finder for the OCID or title before sending |
| Sources | RNTF, 12 Nov 2026 (https://rntfnd.org/2026/11/12/rfi-resilient-timing-and-positioning-system-for-operation-in-degraded-gnss-environments-gov-uk/); GPS World report; vcprep research report 12 §0 item 1 and G1. Find a Tender returned HTTP 403 to automated fetches, so **read the notice yourself** |

## Recommendation: GO, as a short, honest capability response (not a bid for the system)

- The requirement is a TRL 9 hardware system. **Kshana is software for test, assurance and evidence.**
  It cannot meet the requirement as a supplier, and the response says so in its first paragraph.
- Why respond anyway:
  1. Every supplier must provide jamming/spoofing "test or operational results" and holdover evidence.
     That is the gap Kshana's referee scorecards, threat library and signed evidence packs address.
  2. An RFI response can shape the tender (expected 4 Jan 2027) toward comparable, checkable evidence.
  3. It opens a route to partner with a hardware supplier as its test layer.
- Effort: about 1 day to finalise the draft. Expected direct revenue: low (inference). Strategic value:
  a first contact with a UK defence buyer of resilient timing, as a template for later MOD buys
  (report 12, inference).

## Files

- [`RFI-RESPONSE-DRAFT.md`](RFI-RESPONSE-DRAFT.md): covering letter, item-by-item response (VALIDATED/
  MODELLED labels, "N/A as a supplier" where honest), suggested supplier-partner paragraph, three
  questions to Dstl including eligibility, and a pre-send checklist.

## The eligibility question for a non-UK SME (also in the draft, question 1)

> Ashforde is an SME established in Estonia (EU), with a German company in formation, and has no UK
> establishment. Can a non-UK supplier respond to this RFI and take part in the subsequent tender, either
> as a supplier or as a subcontractor or test partner to a supplier? Are there security, clearance or
> establishment requirements (for example UK security clearance of staff or facility, or a UK company)
> that would exclude a non-UK SME?

Background (unverified, check before relying on it):
- The Procurement Act 2023 regime does not by itself bar EU suppliers.
- Defence and security requirements can carry security conditions, such as clearance or a UK facility.
- Exports of dual-use software from the EU to the UK are normally covered by the EU general export
  authorisation EU001.
- None of this is a legal conclusion.

## Export control: read before sending anything beyond the RFI text

- Dstl is a military end user. Kshana's export classification is **not yet determined**
  (`commercial/export-control/CLASSIFICATION-REQUEST-PACK.md`, "NOT A DETERMINATION"; Pack B hold).
- The RFI response is product information and is fine to send.
- **Do not send software, attack waveforms or threat-scenario exports to Dstl or a supplier until counsel
  confirms the classification and end-use position.** The vetted-buyer rule for attack-IQ applies
  (`docs/ATTACK-IQ.md`).

## Founder actions

| Date | Action |
|---|---|
| by Fri 27 Nov | Read the full notice; copy the Dstl e-mail address from it; search Contracts Finder for a mirror |
| by Fri 4 Dec | Approve the draft (set item 14; confirm the ITAR/export line); optionally agree a supplier-partner mention in writing |
| **Wed 9 Dec, 17:00 UK** | Send (Word or PDF, ≤ 6 pages, exact subject line) |
| from 4 Jan 2027 | Watch for the tender notice; decide partner-only participation |
