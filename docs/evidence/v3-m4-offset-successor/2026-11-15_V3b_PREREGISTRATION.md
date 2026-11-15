# V3b pre-registration: the M4 position-offset bar, corrected stage-2 method (roadmap 0.42 V3)

Written and committed on **2026-11-15**, alone, after the V3 development run and **before** any
V3b code and before any seed named here is generated. V3 (`2026-11-15_PREREGISTRATION.md`) stands
and is scored and published as registered, whatever V3b shows. A bar may be tightened, never
loosened. Results go in `V3b_RESULTS.md` next to this file, met or missed.

## 0. What was seen first (disclosure)

The V3 development run (seeds 50 to 59, `dev.json`): every grid pair had a false-alarm rate far
above 0.005 (0.140 to 0.549), with offset recall at d ≥ 5 km of 0.879 to 0.988. Diagnosis from
the method text, not from scored data: in V3's step 4, a NORMAL vessel's own fix, which lies where
the AIS track says the vessel is (displacement near zero), can support any hypothesis `o` with
`|o|` up to the pairwise radius (5.26 km at 13.816), so a single stray fix 3.7 to 5 km away plus
the vessel's own fixes passes steps 5 to 7. V3's scored run on seeds 5000 to 5009 is run with the
pair V3's rule chose (m = 3, 13.816), as registered, and is published as it comes out.

## 1. Data

As V3 (the open M4 generator, kshana 0.41.0), with new seeds: **development 60 to 69; scored 6000
to 6009**, used by no one.

## 2. The method (V3's, with three changes; `kshana-pro/m4-offset/v4b`)

Stage 1 and V3's steps 1 to 3 and 8 unchanged. Changes:

1. **Support excludes fixes consistent with the AIS position.** A candidate with
   `|v|² / σ² ≤ G` is never part of any support (step 4) and never seeds a hypothesis.
2. **Final support gate.** After the refinement, the support is recomputed with the gate
   `|v − ô|² / σ² ≤ G` (a 3.717 km radius around the refined offset) in place of the pairwise
   gate; steps 5 and 6 use that support. The pairwise gate `G_pair` is used only for the first
   support (to form `ô`).
3. **Contradiction (step 7)** as V3, with "support" meaning the support of change 2.

Tuning on seeds 60 to 69 only: `m ∈ {2, 3}`, `G_pair ∈ {13.816, 18.421}`, by V3's rule (highest
recall at d ≥ 5 km subject to FA ≤ 0.005; ties to the smaller m, then the smaller G_pair; none
eligible: lowest FA, miss expected). The chosen pair is committed before seed 6000 is generated.

## 3. Bars

V3's bars, unchanged: V3-B4 (offset recall ≥ 0.85 at 5 km, ≥ 0.98 at 10, 20 and 50 km) and the
guards V3-G1 to V3-G6 (FA ≤ 0.01; DARK recall ≥ 0.95 and precision ≥ 0.80; GAP recall ≥ 0.85;
IDENTITY-DUPLICATE recall ≥ 0.95; determinism; stage 1 unchanged). V3b is MET only if all hold.

## 4. Process

1. This file, committed alone.
2. Code, development run (`V3b_dev.json`), chosen pair committed.
3. One run on seeds 6000 to 6009 (`V3b_results.json`, `V3b_RESULTS.md`).
