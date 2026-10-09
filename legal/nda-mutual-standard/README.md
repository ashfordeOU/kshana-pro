# Mutual NDA, standard (plain) variant

Prepared 2026-10-08 at a prospective partner's request for an NDA "without all the
security" that they could pass straight to their attorney. It is the plain, standard
counterpart to the longer branded draft in `../nda-mutual-branded/`.

- `Ashforde-Mutual-NDA-Standard.docx`: the document as sent, editable so the other side's
  attorney can redline it.
- `Ashforde-Mutual-NDA-Standard.md`: a text copy for diffs and review.

The template itself names no counterparty. From round 1 the negotiated copies name the
counterparty company (SJAWS Technologies Inc.), as partner specifics may live in `legal/`.
Individuals' names and personal email addresses still never appear: redact them from any
markup before it is committed.

## What changed against the branded draft

The original file was not password- or edit-protected, so "security" was read as the
security-heavy clauses and markings. The draft was about twice the length of a standard mutual NDA.

Removed:
- the CONFIDENTIAL banner, the "High Assurance" header tagline and "Confidential" in the
  footer;
- no-reverse-engineering, the non-compete use restriction and "use at your own risk" (§2);
- no-residuals and the one-sided feedback licence (§5);
- §6 Evaluation software: the evaluation licence and the ban on publishing benchmarks. This
  belongs in a short evaluation licence sent with any build;
- §7 Third-party and ESA restrictions;
- §8 Export control and sanctions;
- §9 Data protection / GDPR;
- the internal drafting notes ("template, not legal advice" and the governing-law note).

Kept:
- Purpose, definition, obligations, exclusions and compelled disclosure.
- Ownership, no licence beyond the Purpose, and independent development.
- The AGPL-3.0 carve-out, so the public Kshana engine is never Confidential Information.
- Term (2 years, confidentiality 3 years), return or destruction, no warranty, no obligation
  to proceed, publicity, remedies and the general clauses.

Added or decided:
- **Governing law: the Republic of Estonia.** Venue, exclusive/non-exclusive, the counterparty
  details, the effective date and the signatory are left blank for the parties.
- **5(c) Feedback**, neutral and mutual: *"Either Party may freely use general comments or
  suggestions the other offers about its products or technology ("Feedback"), provided that
  doing so does not disclose the other Party's Confidential Information."*
  - Reason: Kshana is developed in public under AGPL-3.0, and improvements suggested in
    discussion flow into it.
  - Without this clause, general suggestions received under the NDA would be usable "solely
    for the Purpose".
  - The clause keeps the other side's data, recordings and specifics protected.
- Sub-items are lettered (a), (b), … within each section.
- Document properties are set to Ashforde OÜ, with no comments and no tracked changes.

## Still to handle outside this NDA

- **Export control:** do not share export-controlled material until counsel answers N6
  (`../EXPORT-CONTROL-DETERMINATION.md`).
- **Evaluation builds:** send any build with a short evaluation licence covering licence
  scope, no redistribution, no published benchmarks without consent, and return or
  destruction.
- **The other side's third-party (ESA / European partner) obligations:** their attorney may
  add a clause back. That is acceptable.

## Negotiation history

### Round 1, 2026-10-09

The counterparty's counsel returned a markup. `Ashforde-Mutual-NDA-Standard-reply-1.docx` is
Ashforde's reply, with tracked changes and comments (`.md` is its text copy).
`Ashforde-Mutual-NDA-Standard-reply-1-clean.docx` is the same text with every change accepted
and no comments. It is redacted for git: the counsel's name and email
address are replaced. The copy sent is otherwise identical.

Their changes, all accepted:
- §1: their methods and software code for their product added to what is confidential.
- §5(a): their ownership list made to match Ashforde's.
- §6: the bracketed defaults confirmed: 2-year term, 30 days' notice, 3 years' confidentiality.

Their comments and Ashforde's answers (tracked changes plus threaded replies):
- §11: they asked for Ontario law and courts. Accepted, with **non-exclusive** jurisdiction and
  an express right for either Party to seek an injunction (§10) in any competent court.
- §12(a): they asked for notice emails now, with a copy to counsel. Accepted: Ashforde
  contact@ashforde.org; the counterparty's address and counsel's address left for them to fill;
  added that the copy to counsel is a courtesy and is not itself notice.

SJAWS's details filled from its Corporations Canada record (read 2026-10-09): SJAWS
Technologies Inc., a CBCA corporation, number 1464649-5, active, registered office 3200 - 650
West Georgia Street, Vancouver BC V6B 4P7. Notice email admin@sjaws.com (the address on its
website), with the copy to the counsel who sent the markup; both left for them to confirm.

Also filled or fixed by Ashforde: registry code 17321180 and registered office (from
`commercial/payment-details.toml`; check against the e-Business Register), the Effective Date
defined as the date of the last signature, and a missing comma in §5(a).

Still open: SJAWS's signatory and who notices should be marked for, and their confirmation of
the details above. Ashforde's counsel to confirm the Ontario choice before signing.

Not legal advice. Have counsel review before signing.
