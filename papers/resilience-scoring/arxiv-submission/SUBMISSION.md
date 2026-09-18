# arXiv submission package -- resilience-scoring

Everything needed to upload "How Stable Is a PNT Resilience Score?" to arXiv.
Upload artifact: `../resilience-scoring-arxiv.tar.gz`
(contents: `resilience-scoring.tex` + `figures/` only -- source, no build junk).

Submission is the founder's manual step (account + endorsement); this package is
prepared and verified, not submitted.

## What to upload

`resilience-scoring-arxiv.tar.gz` -- verified to compile standalone (6 pages, 5
figures, 1 table) from a pristine extract using the same TeX Live stack arXiv uses
(IEEEtran, graphicx, amsmath, booktabs, url; inline `thebibliography`, so no BibTeX;
PDF figures, so arXiv builds with pdfLaTeX, the default for PDF figures).

## Metadata for the arXiv form

**Title**
How Stable Is a PNT Resilience Score? Decision-Instability of Single-Number Resilience Ratings under Framework-Aligned Weighting

**Author**
Chakshu Baweja (Ashforde OÜ, Tallinn, Estonia)

**Contact**
contact@ashforde.org

**Primary category (recommended)**
cs.CR (Cryptography and Security) -- the paper is about resilience scoring, a threat
ensemble (jamming/spoofing/meaconing), and the gameability of self-attestation.

**Cross-list (recommended)**
eess.SP (Signal Processing) -- PNT/GNSS. Optional second cross-list: stat.AP
(composite-indicator sensitivity / decision-stability methodology).

**Comments field**
6 pages, 5 figures, 1 table. Open-source scoring engine, instability study, artifact
generator, and figure pipeline (AGPL-3.0); study artifact is byte-deterministic
(fixed seed). Aligned to DHS RPCF v2.0; a simulation-derived self-assessment, not a
certification.

**License (founder decision)**
Recommended: CC BY 4.0 (paper text). The code remains AGPL-3.0 (independent of the
text license). Alternatives: arXiv non-exclusive license; CC BY-SA; CC0.

**Abstract (plain text; arXiv renders inline math)**

Every authoritative positioning, navigation, and timing (PNT) resilience framework,
from the DHS Resilient PNT Conformance Framework (RPCF) to the
Resist-Detect-Respond-Recover model and Yang's resilient-PNT criteria, defines what
resilience means but supplies only self-attestation: a checklist or a maturity Level,
with no engine, no measurement, and no evidence. We build the missing measurement
layer as an open, deterministic scoring engine over a PNT simulator, emitting
per-dimension sub-scores each traceable to a scenario and an oracle and each tagged
with its honest validation status. We then ask whether a single composite score, or a
single maturity Level, is a stable basis for a decision. Across a reference panel of
seven architectures built to span genuine cross-dimension tradeoffs, a Dirichlet
weighting simplex over the seven RPCF categories, and a five-threat ensemble, the
answer separates into two regimes. The composite winner is stable under active denial
and under near-equal weightings, flipping in about 1 percent of draws, so a single
number is safe precisely where one design dominates; but re-weighting alone flips the
winner in up to 22 percent of draws under nominal conditions, where designs genuinely
contend, and that instability is a domain application of known composite-indicator
sensitivity. The sharper and weighting-invariant failure is categorical: a
weakest-link maturity Level (our minimum-over-categories operationalization of the
RPCF ladder, not the framework's own rule) is a function of the threat assumed rather
than of the architecture, changing for one architecture in seven across the ensemble.
A constructed example shows that, because the composite rewards declared techniques, a
single-band receiver declaring all seven techniques can outscore a genuinely more
resilient system: self-attestation can be gamed by declaration. And apparent fourfold
GNSS redundancy reduces, by the definition of a shared common-mode failure domain, to
an effective diversity of one. The conclusions hold under a plus-or-minus 20 percent
perturbation of every driver within the modelled reduction. We argue for reporting
per-dimension sub-scores with provenance and a rank range, not a phantom single
number. This is a simulation-derived self-assessment aligned to RPCF v2.0, not a
certification.

## Step-by-step (founder)

1. Sign in at https://arxiv.org/ (the contact@ashforde.org account).
2. New submission -> choose license -> upload `resilience-scoring-arxiv.tar.gz`.
3. arXiv AutoTeX compiles it; check the PDF preview matches the local 6-page build.
4. Enter the metadata above.
5. First submission in cs.CR/eess.SP may need an endorsement -- request from a
   colleague already publishing there.
6. Submit; note the arXiv identifier.

## Provenance / quality record

- Numbers reproduce exactly from `kshana/paper-artifacts/resilience-study.json`
  (byte-deterministic, fixed seed 12345, engine 0.19.0); 35 resilience-module tests
  pass; full engine suite green. (Independently fact-checked: FULLY REPRODUCES.)
- 4-reviewer adversarial pass run 2026-06-22; the MAJOR findings (RPCF-Level
  mislabel, H1 near-refutation, H3 definitional framing, H2 over-generalization,
  invalid binomial SE, overstated novelty) were all addressed by honest reframing,
  no numbers changed. Fig 5 caption corrected to match the plot; Fig 5 title overlap
  fixed; kshana software citation + 5 further references added.

## Constraints (carry over)

- No raw third-party data shipped; figures derive from the in-repo deterministic
  artifact. No machine paths / personal email / AI attribution in source (checked).
- Do not claim certification, accreditation, TRL>3, heritage, or endorsement.
- The paper explicitly disclaims certification; keep that intact in any edit.
