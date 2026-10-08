# Mutual NDA, standard (plain) variant

Prepared 2026-10-08 at a prospective partner's request for an NDA "without all the
security" that they could pass straight to their attorney. It is the plain, standard
counterpart to the longer branded draft in `../nda-mutual-branded/`.

- `Ashforde-Mutual-NDA-Standard.docx`: the document as sent, editable so the other side's
  attorney can redline it.
- `Ashforde-Mutual-NDA-Standard.md`: a text copy for diffs and review.

**No counterparty name appears in any of these files** (placeholders only). Keep it that way;
who it was sent to is not recorded in git.

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

Not legal advice. Have counsel review before signing.
