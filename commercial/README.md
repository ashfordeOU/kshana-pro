<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Commercial readiness pack

**Owner:** Ashforde OÜ · **Status:** ready, waiting on three founder-side events · **Date:** 2026-09-25

Everything needed to take money for Kshana work, prepared ahead so that each blocker,
when it clears, is a matter of filling in values rather than writing documents. This
folder is internal; this repository is private and must stay private.

Abbreviations: ESA (European Space Agency), esa-star (ESA's supplier registration
system), IBAN (International Bank Account Number), VAT (value added tax), BAFA
(Germany's Federal Office for Economic Affairs and Export Control), CNI (critical
national infrastructure), HTML (HyperText Markup Language), TOML (Tom's Obvious Minimal
Language, a configuration file format).

## Founder decisions this pack follows

| Topic | Decision (2026-09-25) | What the pack does about it |
|---|---|---|
| Business bank account | Comes later; everything must be plug-and-play the day it exists | One file to fill (`payment-details.toml`); every document refuses to render until it is filled; a checklist of every other place the details go |
| Pricing | Deferred to the end | Every price is `PLACEHOLDER` or `[€PLACEHOLDER]`; the renderer refuses a final document while any remains. No price is invented anywhere |
| ESA Business Applications and Space Solutions (BASS) Kick-start application | Deferred | Not prepared here. The esa-star bank step in the go-live checklist is needed for any ESA contract, not only that one |
| Partner and continuity research | Delegated: research and prepare | Researched partner list and draft emails. **Nothing sent** |

## Contents

| # | File | What it is | Waiting on |
|---|---|---|---|
| 1 | [`payment-details.toml`](payment-details.toml) | The single source of seller identity, bank account, payment reference and VAT status. Bank, reference and tax fields are empty on purpose. | Bank account |
| 1 | [`tools/render.py`](tools/render.py) | Renders invoice, quotation and order form to print-ready HTML from that file plus one deal file. Standard library only (Python 3.11 or later). Refuses, naming every problem, while payment details are empty or invalid or a deal still has placeholders. | Bank account, prices |
| 1 | [`tools/test_render.py`](tools/test_render.py) | Unit tests for the renderer. | — |
| 1 | [`deals/example-deal.toml`](deals/example-deal.toml) | Example deal. The buyer is fictional; prices are placeholders. | — |
| 2 | [`GO-LIVE-CHECKLIST.md`](GO-LIVE-CHECKLIST.md) | Every place bank and payment details must be entered once the account exists, with the exact field: esa-star Bank Details, invoicing, order form, website, tax portal. | Bank account |
| 3 | [`offers/cni-timing-holdover-study.md`](offers/cni-timing-holdover-study.md) | One-page fixed-scope offer: timing holdover sizing study for telecom and other CNI. Carries an internal gate box to delete before sending. | Export-control determination, prices, counsel, bank |
| 4 | [`counsel/COMMERCIAL-LICENCE-FINALISATION.md`](counsel/COMMERCIAL-LICENCE-FINALISATION.md) | For counsel: where the licence drafts are, 15 issues found, and 16 questions to answer. Changes no licence text. | Counsel |
| 5 | [`export-control/CLASSIFICATION-REQUEST-PACK.md`](export-control/CLASSIFICATION-REQUEST-PACK.md) | Factual software description, candidate control-list entries with reasons, and questions for the authority. **Not a determination.** | Export-control counsel |
| 6 | [`partners/TIMING-PARTNERS.md`](partners/TIMING-PARTNERS.md) | Researched list of timing laboratories, test houses and national metrology institutes in the European Union and United Kingdom, each with its public contact route and sources. | Founder approval to contact |
| 6 | [`partners/OUTREACH-DRAFTS.md`](partners/OUTREACH-DRAFTS.md) | Draft emails. **Drafts only; never sent.** | Founder approval |

## Three findings the founder should see first

1. **The export-control authority is probably Estonian, not German.** The existing
   drafts plan a BAFA classification opinion. BAFA's own page says it does not issue
   one to a foreign company or for technology, and Ashforde OÜ is established in
   Estonia. See section 1 of the classification pack.
2. **No text yet grants the commercial licence for the open engine** that the public
   `LICENSING.md` advertises; the draft licence covers only Kshana Pro and study
   reports. See issue 1 of the counsel pack. **Update 2026-09-26:** a draft grant is now
   `legal/COMMERCIAL-LICENSE.md` §2A, for counsel to mark up; the legal drafts now name
   the Estonian Strategic Goods Commission as the likely authority, pending question E1.
3. **The timing holdover study is "Pack B"**, which `legal/EXPORT-CONTROL-DETERMINATION.md`
   puts on hold until a determination exists. The offer page is ready but must not go
   out before then.

## Using the renderer

```sh
# Is payment-details.toml ready? (exit 0 when ready, 3 when not)
python3 commercial/tools/render.py check

# A watermarked preview, allowed today:
python3 commercial/tools/render.py render invoice --deal commercial/deals/example-deal.toml --draft

# The real document, once the bank account and prices exist:
python3 commercial/tools/render.py render invoice --deal commercial/deals/<deal>.toml

# Tests:
python3 -m unittest discover -s commercial/tools -p 'test_*.py'
```

Output goes to `commercial/out/`, which is not tracked. Open the HTML in a browser and
print to PDF (A4). Exit codes: 0 success, 2 bad arguments, 3 payment details not ready,
4 deal not ready.

Copy `deals/example-deal.toml` to start a new deal. Name deal files by deal identifier
(for example `deals/D0001.toml`); avoid putting "customer" in a file name, because the
repository's `.gitignore` hides `*-customer-*` files.

## Change record

- 2026-09-25: pack created (items 1 to 6 above).
