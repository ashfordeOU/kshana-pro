<!--
PRIVATE — Ashforde OÜ internal bid material. kshana-pro repo only.
Honest record of a real-data validation ATTEMPT for the GNSS-security / spoof-jam
vertical (AO/1-13503, D2 validated-island expansion). Outcome: stays MODELLED, by
evidence. Contact: contact@ashforde.org
-->

# Security vertical — real-data validation attempt (honest negative result)

**PRIVATE.** Part of D2 (validated-island expansion). This documents a genuine
attempt to turn the **GNSS spoof/jam detection** capability from `MODELLED` into an
externally-`VALIDATED` island using real public attack data — and the honest reason
it **stays MODELLED**. Recording a tried-and-failed validation *is* A3 evidence:
representativeness justified, gaps to flight clearly identified.

## Goal

Build a clean, non-circular, label-backed island: a **Kshana-computed** detection
score that, on **real attack data**, reproduces a known separation between
attack and non-attack conditions, with the metric (ROC AUC) cross-checked against
an independent reference tool — exactly the pattern used for the OPS-SAT anomaly
island (`tests/opssat_ad_reference.rs`).

## Data used

**Jammertest 2024** (Norwegian government open GNSS attack campaign), the labelled
set already cached locally under `kshana/realdata-cache/jammertest2024/`:
24 scenarios across **Jamming, Spoofing, Meaconing, Jamming+Spoofing**, each with
u-blox receiver logs — `mon_rf.csv` (RF monitor: AGC, noise/ms, jamming indicator),
`nav_pvt.csv` (PVT solution: fix type, satellites, DOP, accuracy) and a
`scenario.json` carrying the **attack-type label**.

## What we tried (45,035 real epochs, pooled, labelled by attack type)

The only ground-truth label the dataset provides is the **per-scenario attack
type**. We tested whether any candidate score separates **jamming-class**
(Jamming, Jamming+Spoofing) from **non-jamming** (Spoofing, Meaconing):

| Candidate detection score | What it should catch | AUC (jamming vs not) |
|---|---|---|
| PVT health — satellites visible (numSV) | jamming starves the fix | 0.34 |
| PVT health — geometry (pDOP) | jamming degrades geometry | 0.43 |
| PVT health — horizontal accuracy (hAcc) | jamming inflates error | 0.36 |
| Raw RF — AGC count | jamming raises front-end gain | **0.62** |
| Raw RF — noise per ms | jamming raises the noise floor | 0.39 |
| (reference) receiver's own jamInd flag | — | 0.37 |

**No score exceeds ~0.62 AUC; most sit at chance (0.5) or invert.** There is no
clean, honest separation at the pooled, cross-scenario level.

## Why it does not yield a clean validated island (the honest reasons)

1. **No per-epoch onset truth.** The public data labels each *scenario* by attack
   type, not each *epoch* by attack on/off. The jammer/spoofer is **intermittent**,
   so pooled epochs are dominated by clean pre-onset periods — washing out any
   separation. Recovering onset requires **manual annotation**, which is a
   subjective judgement, **not an external-dataset oracle** → labelling it
   `Validated` would violate our own honesty rule.
2. **Spoofing deliberately looks clean.** A successful spoof *maintains* a
   plausible, high-confidence fix — so PVT-health metrics cannot separate it from
   benign operation (hence the ~0.4 AUCs).
3. **Using the receiver's own `jamInd` flag would be circular** — that is the
   receiver's detector, not Kshana's; it validates nothing about our code (and here
   it doesn't even separate).
4. **Cross-scenario confounds.** Two receivers, different locations/times and front
   ends per scenario inject AGC/noise differences unrelated to the attack.

## Honest conclusion

The GNSS spoof/jam **detection** capability stays **`MODELLED`** — physics-sound,
unit-tested (`spoof`, `spoof_detect`, `spoof_monitors`, `tpl::Cusum`), and
exercised on real data for **characterisation** (`tests/spoof_texbat_validation.rs`,
the resilience study) — but **not** externally `Validated` against a clean oracle,
because the public attack data lacks the per-epoch ground truth that an honest
external-oracle validation needs. This matches the verification-matrix status
already recorded (`status: Modelled` for the spoofing-detection row).

## The path to a real `Validated` security island (tracked follow-on)

The one public source with a **documented quantitative onset** is **TEXBAT** (UT
Austin): e.g. ds7 exhibits a **1.273 µs clock pull by t ≈ 468 s** (Humphreys et al.,
ION GNSS 2012). Validation route, when the founder fetches the raw IQ (multi-GB,
research-use):

1. `scripts` fetch of TEXBAT ds2/ds3/ds7 `.bin` into `/realdata-cache/` (git-ignored).
2. SDR front-end (existing `sdr` + `realdata::iqif`) → solved clock-offset series.
3. Run `tpl::Cusum` and assert **no false alarm** on the clean pre-onset window,
   then **detection at the documented latency** with the documented clock-pull as
   the external oracle.

That is a genuine external-oracle validation; it is gated only on fetching the
large raw recordings, not on any honesty compromise.

---

*Ashforde OÜ · contact@ashforde.org · We attempted real-data validation and report
the honest negative. MODELLED unless an external oracle exists — no exceptions.*
