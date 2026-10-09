<!-- PROPRIETARY — Kshana Pro overlay. Internal commercial record. Not part of the open-source engine. -->

# Independent review of the receiver trust-score method: scope

**Prepared:** 2026-10-09 · **Status:** draft for founder decision. Nothing sent. Companion document: [`AUDIT-RFP.md`](AUDIT-RFP.md).
**Basis:** the public Kshana repository and its `docs/RECEIVER-TRUST.md`, read 2026-10-09 (https://github.com/ashfordeOU/kshana/blob/main/docs/RECEIVER-TRUST.md). Where this scope restates the method, check it against the repository at the commit the reviewer is given.

## Purpose

Obtain an independent, publishable technical opinion on whether the receiver trust-score method is sound, honestly described and adequately tested, so that funders, test houses and users can rely on it, or see exactly where they cannot. It is a review of method and evidence, not a certification of a receiver or a product.

## The method in one page (as publicly documented)

- **Input.** A receiver log (u-blox UBX, RINEX 3, Android GnssLogger CSV, NMEA 0183), optionally with a scenario file.
- **Pre-registration.** Events (label, kind, onset, optional end, optional predicted C/N0 drop) and tolerances are written into the scenario before the run; the scenario is part of the result's hash.
- **Monitors.** Each runs only if its input exists: C/N0 drop, loss of lock, AGC departure, u-blox jamming indicator, position jump, and, on RINEX logs with a navigation file, RAIM, solve failure and clock checks. The first `calibration_s` seconds (default 60) set a baseline and cannot alarm. Thresholds are inputs with documented defaults.
- **Score mapping.** Per epoch: untrusted (RAIM, clock, solve failure or position-jump alarm; or C/N0 drop reaching twice its threshold), degraded (any other alarm), nominal (no alarm).
- **Event scoring.** False alarms before onset, first alarm, latency, outcome (detected, late, missed, not-evaluable); predicted versus observed C/N0 drop (agree, disagree, no-prediction, not-evaluable).
- **Stated limitations.** Monitors are labelled MODELLED; thresholds are inputs; one agreeing prediction is "one data point, not a validation"; the bundled example log is synthetic.
- **Existing test evidence.** A test file checks the engine-path monitors against a public interference-trial spoofing oracle (eight published onsets); a verification-matrix row holds the numbers.

## Questions the reviewer is asked to answer

### A. Pre-registration
1. Does the pre-registration mechanism do what it claims? Can a tolerance or event be changed after seeing the log without the result's hash revealing it?
2. What does the hash cover and not cover (log, scenario, engine version, thresholds)? Is the audit trail sufficient for a third party to reproduce a result?
3. Are there ways to pre-register in a manner that makes later detection claims easy to satisfy (for example loose tolerances)? Should the method enforce limits?

### B. Monitor logic
4. Is each monitor's rule implemented as documented? Check against code and tests.
5. Are the defaults (for example calibration length, thresholds) justified, and is their sensitivity analysed? What happens when the calibration window itself contains an event?
6. Are failure modes handled: missing data, clock jumps, constellation changes, receiver restarts, multi-constellation logs?
7. Are the monitors statistically independent enough to be combined, and is false-alarm behaviour on clean data characterised?

### C. Score mapping
8. Is the three-state mapping defensible? Is the "twice the threshold" rule for C/N0 justified, and are the ordering and asymmetry between monitors reasoned?
9. Is "trust" an appropriate word for what is computed? Is there a risk users read the state as a probability or a safety assurance?
10. Could two receivers or two scenarios with equal states hide materially different risk?

### D. Test evidence
11. Is the existing evidence (including the public trial oracle) adequate for the claims made? Is the oracle independent of the method's development?
12. Is the sample size, diversity of receivers, interference types and environments sufficient? What is the smallest additional evidence that would change the conclusion?
13. Can the evidence be reproduced from the public repository alone?

### E. Limitations and claims
14. Does the documentation state the limits accurately and prominently? List any claim that exceeds the evidence.
15. Which uses should be explicitly discouraged (for example safety-of-life decisions, certification evidence)?

## Out of scope

Certification of any receiver; RF measurements; development work on Kshana; legal or export-control opinion; review of unrelated Kshana capabilities.

## Materials provided

- Public repository at a named commit: source, tests, scenario examples, documentation, validation matrix.
- The synthetic example log and scenario.
- Public interference-trial data and oracle references already used by the tests (as listed in the repository).
- A written method summary and a list of known weaknesses prepared by Ashforde OÜ (to be disclosed in full to the reviewer).
- Reasonable written answers to technical questions during the review; all questions and answers are logged and included in the report appendix.
- No partner data. If the reviewer wants additional recordings, they obtain or generate them independently.

## Deliverables

1. Written report answering A to E, with a finding register: each finding rated (blocking, significant, minor, observation), with evidence and recommended action.
2. Conclusion statement of a fixed, plain form that can be quoted: what the review did, what it found, what it did not examine.
3. A reproducibility log: commands run, versions, outputs.
4. Review of Ashforde OÜ's response and of the corrected version, if any (second round).
5. Optional: a short public summary written by the reviewer.

## Independence requirements

- No financial interest in Ashforde OÜ or in any Kshana commercial licensee, and no equity, employment, consultancy or other paid work for Ashforde OÜ in the previous 24 months (period is a proposal; founder to confirm).
- Written declaration of conflicts, including any relationships with receiver manufacturers or test-equipment vendors whose products the method touches.
- No contribution to Kshana source code before or during the review.
- Payment is for time and not contingent on the conclusion. Ashforde OÜ does not edit findings or conclusions.
- Reviewer may stop and say so publicly if access or conditions compromise independence.

## Publication terms (proposed)

- The reviewer may publish the full report, and the review is publishable regardless of outcome.
- Ashforde OÜ gets a factual-accuracy read of a draft for a fixed short period `[PLACEHOLDER days]`, limited to correcting misstatements of fact. Disagreements on conclusions are recorded in a response annexed to the report, not edited out.
- Ashforde OÜ may publish the report in full and may link it from the repository and site. It may not publish excerpts that change the meaning.
- Report licensing: `[FOUNDER/COUNSEL: choose, e.g. Creative Commons Attribution]`.
- Material findings that affect users are fixed or disclosed publicly within `[PLACEHOLDER]` of the final report.

## Evaluation of the review's usefulness for funders

The report and response can be cited in grant applications as independent evidence of method review. It must not be described as a validation of detection performance unless the report itself says so.
