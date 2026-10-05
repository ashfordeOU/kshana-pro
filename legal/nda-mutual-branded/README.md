# Branded mutual NDA (polished draft + .docx builder)

Preserved from a working session on 2026-10-04 (previously local-only; now in git).

- `Ashforde-Mutual-NDA.md` — a richer, ready-to-send mutual NDA draft that protects Ashforde's
  IP/roadmap/pre-release builds, accounts for a counterparty's simulator/receiver/recordings
  and third-party/ESA constraints, covers export control and GDPR, and carves out the public
  AGPL-3.0 engine as explicitly non-confidential. Bracketed `[fields]` are fill-ins; it is a
  starting point, not legal advice — have counsel review before signing.
- `Ashforde-Mutual-NDA.docx` — the same, rendered to Word with Ashforde branding (Georgia/gold
  `#B6985A`, wordmark `A S H F O R D E ®` with a superscript ®, a CONFIDENTIAL header
  watermark, gold rule).
- `build_nda_docx.py` — the `python-docx` generator that produces the `.docx` from the Markdown
  (`pip install python-docx`, then `python build_nda_docx.py`).

Relationship to `legal/NDA-TEMPLATE-MUTUAL.md`: that file is the concise canonical template;
this folder is the longer branded variant produced for a specific prospective partner
engagement. Reconcile into one canonical form when counsel reviews. **No counterparty name
appears in any of these files** (placeholders only) — keep it that way.
