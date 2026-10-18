# W55 brief: Kshana 0.38 WS-C timing, standards and hardware safety, plus WS-D fixes D1, D3, D4

Read first: docs/briefs/_COMMON-0.38.md (rules), docs/plans/0.38-PLAN.md sections WS-C and WS-D, strategy moves
3, 5 and 6.

## Branch you own
- kshana: feat/timing-standards-0.38 from origin/main.

## Items (plan IDs)
C1 clock_log formats for linuxptp (ptp4l, phc2sys, ts2phc) and chrony (tracking/statistics); parser tests on
   public sample logs (licence recorded) or self-written fixtures.
C2 `holdover-test` scenario kind (baseline → GNSS removed by the operator → holdover → recovery), scored against
   ITU-T masks from PUBLIC tables only; docs/TIMING-HIL.md; wording "measured against the mask".
C3 NIST IR 8323r1 crosswalk rows in compliance-report (paraphrased; "supports evidence for").
C4 ION GNSS SDR metadata writer for iq scene output; round trip with the existing reader.
C5 `iq monitor --follow` on a growing SigMF file (receive only); test with a file that grows during the run.
C6 docs/HARDWARE-SAFETY.md: HIL stages 0–4 from the strategy memo, legal notes, a `lab-attestation` field.
   Mark it "owner legal review pending" in your report.
D1 README: remove the VALIDATED wording from items that have no row in the verification matrix (check each claim
   against web/studio/data/verification-matrix.json / the generated matrix).
D3 docs/CAPABILITY.md still says there is no PPP/RTK: correct it from the matrix (0.37 has RTK with integer
   ambiguity fixing).
D4 e2e-live "Marine integrations" TCP race: make the test wait for the full stream before comparing (it failed once
   on main run 39122070323 and passed on re-run). Fix robustness; never skip or loosen it.

## Done when
Gates green; every new command/kind on every surface (or N/A with reason); docs written; report as in the common
rules.
