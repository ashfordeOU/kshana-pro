# arXiv submission package

Everything needed to upload "A Conditional Timing Protection Level" to arXiv.
The upload artifact is `../timing-protection-level-arxiv.tar.gz`
(contents: `timing-protection-level.tex` + `figures/` only — source, no build junk).

**SUBMITTED — live on arXiv as [2606.24210](https://arxiv.org/abs/2606.24210)**
(eess.SP, announced 2026-06-24). This package is the verified source that was uploaded;
the metadata and steps below are kept as the submission record. NB: an arXiv announcement
is a public **preprint**, not peer-reviewed or journal-published.

---

## What to upload

`timing-protection-level-arxiv.tar.gz` — verified to compile standalone (8 pages,
5 figures, 2 tables) from source + figures alone, with the same TeX Live stack arXiv
uses (IEEEtran, graphicx, booktabs, amsmath, url; inline `thebibliography`, so no
BibTeX run is needed; PDF figures, so arXiv must build with pdfLaTeX, which is the
default for PDF figures).

## Metadata to enter on the arXiv form

**Title**
A Conditional Timing Protection Level: Holdover-Limited Undetected Time Error Under GNSS Spoofing

**Authors**
Chakshu Baweja (Ashforde OÜ)

**Contact**
contact@ashforde.org

**Primary category (recommended)**
eess.SP  (Signal Processing)

**Cross-list (recommended)**
cs.CR  (Cryptography and Security)
Optional second cross-list: eess.SY (Systems and Control) — for the impossibility /
sequential-detection framing.

**Comments field**
8 pages, 5 figures, 2 tables. Calibrated on the public JammerTest 2024 dataset
(Zenodo 10.5281/zenodo.15911589). Simulator, bound, and calibration example open
source (AGPL-3.0).

**License (founder decision)**
Recommended: CC BY 4.0 (most reuse-friendly, matches the open-source posture).
Alternatives: arXiv non-exclusive license to distribute; CC BY-SA 4.0; CC0.
Note: this is the license for the *paper text*; the *code* remains AGPL-3.0 (the two
are independent).

**Abstract (plain text — paste verbatim; arXiv renders inline math)**

A GNSS timing receiver under spoofing has no nominal-geometry fault to bound with
position-domain receiver autonomous integrity monitoring: the threat is a slow,
common-mode pull of served clock time that the receiver's own time-accuracy flag
need not reveal. We make three contributions of graded strength. First, a field
measurement: solving the receiver clock-solution trajectory from the raw L1
pseudoranges, the receiver's clean dual-band position, and broadcast ephemeris, we
show that a recorded over-the-air spoof in the public JammerTest 2024 campaign
pulled a survey-grade u-blox ZED-F9P by roughly 1.01 ms of served time while the
receiver reported a self-assessed time accuracy of at most 51 ns, a
claim-versus-reality gap near 2e4. Second, an impossibility observation: against an
adversary free to choose the ramp rate, no finite unconditional bound on undetected
time error exists under a single self-referential clock-aided monitor, because a ramp
slow enough to keep the disciplined reference in lock-step (equivalently, below a
sequential test's reference value) is never alarmed while the integrated error grows
without limit. A finite guarantee is therefore necessarily conditional. Third, the
conditional bound itself: the Timing Protection Level (TPL), equal to a model-free
monitor's static detectability floor plus the oscillator's coast uncertainty over the
detection latency, holds given detection by an independent cross-satellite
consistency check that a coherent spoofer does not drive in lock-step. Each term is a
closed form over a primitive verified separately in the open Kshana simulator, so the
sum is reproducible by hand. Calibrated on the recorded attack, the holdover budget
is 114 ns at a one-second recovery and 458 ns even at a 60-second coast, between 2200
and 8800 times below the 1.01 ms the receiver silently accepted; on this slow ramp a
clock-aided sequential test alone gives essentially no protection (it alarms only
after ~993 us),
whereas the model-free consistency monitor crosses its alarm during the spoofer's
ramp, minutes before the capture. We are explicit about the boundary: the bound is
calibrated on real data but not independently validated against ground-truth field
error, it carries no aviation-style integrity-risk budget, and its long-coast value
is governed by the oscillator's long-tau red-noise floor and is reported as a swept
band rather than a single scalar. The simulator, the bound, and the calibration
example are open source under AGPL-3.0.

---

## Step-by-step (founder)

1. Sign in at https://arxiv.org/ (the contact@ashforde.org account).
2. New submission -> choose license (above) -> upload `timing-protection-level-arxiv.tar.gz`.
3. arXiv AutoTeX will compile it; check the generated PDF preview matches the local
   `timing-protection-level.pdf` (8 pages). If AutoTeX picks XeLaTeX and the Times
   metrics warn, the PDF is unaffected.
4. Enter the metadata above (title, authors, abstract, categories, comments).
5. If this is the first submission in eess.SP or cs.CR, arXiv may ask for an
   endorsement; request it from a colleague already publishing in that category.
6. Submit. Note the assigned arXiv identifier.
   -> DONE: assigned **2606.24210** (https://arxiv.org/abs/2606.24210), eess.SP,
   announced 2026-06-24.

## Pre-submission reminders (project constraints)

- Dataset raw recordings are NOT included (GPL-3.0 third-party data, not
  redistributed); only the derived scalars appear in the paper, with provenance.
- No machine paths, no personal email, no AI/tool attribution in the source (checked).
- Do not claim TRL > 3, heritage, certification, or any endorsement.
- The paper is the public-facing artifact; the bid/strategy material stays private.
