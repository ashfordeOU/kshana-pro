<!--
PRIVATE — Ashforde OÜ internal. kshana-pro repo only; not for the open repo.
One-page prime handout for AO/1-13494 WP-B ("Characterisation & Evaluation of ML
Techniques"): the detector-agnostic AI-algorithm evaluation testbed.
Honest framing intact: referee, not classifier; one component, not the whole NAV-DT.
Contact: contact@ashforde.org
-->

# The AI-algorithm evaluation testbed — a neutral referee for RF-impairment detectors

**AO/1-13494 · WP-B "Characterisation & Evaluation of ML Techniques" · Ashforde OÜ (consortium SME)**
**Contact:** contact@ashforde.org · **Engine:** open Apache-2.0 Kshana v0.18.0, `src/impairment_eval.rs`

---

## In one line

A **detector-agnostic evaluation harness** that scores *any* candidate RF-impairment
detection/classification algorithm — the **referee, not a competitor**. It answers the
question an ESA reviewer actually asks of an AI claim: *"how do you know it works, and how
do you know you didn't fool yourself?"*

## What it does

- **Scores any detector through one trait interface** — energy, AGC-power, signal-quality
  (SQM), RAIM-parity, a fused detector, or a partner's own algorithm. The testbed never
  owns the classifier; it grades whatever is plugged in.
- **Emits the full operating-characteristic set**: ROC curve, **AUC**, confusion matrix at
  a chosen operating point, **P_d / P_md / P_fa**, precision / accuracy / F1, and a
  **per-impairment-class** breakdown (nominal / jamming / spoof-time / spoof-position /
  multipath).
- **Carries the honesty checks a reviewer looks for**: a stratified train/test split with
  **disjoint keys**, an **exact-duplication leakage guard**, and an explicit
  **distribution-shift optimism report** (in- vs out-of-distribution AUC gap) — so a
  flattering in-sample number cannot pass unchallenged.

## Validated vs modelled (the line that makes it credible)

| Layer | Status | Why |
|---|---|---|
| The **metrics** (AUC / ROC / confusion / P_d-P_md / precision / F1) | **VALIDATED** | Matched **exactly** to **scikit-learn 1.9.0** (Pedregosa et al., JMLR 2011) over 5 datasets × 24 thresholds — exact counts + < 1e-9 (`tests/eval_metrics_reference.rs`). The referee's ruler is independently correct. |
| The **labelled RF-impairment corpus** | **MODELLED** | Seeded, reproducible, **parameter-grounded** (TEXBAT-style parameters) — **not** raw IQ or field captures, and reported as exactly that. Seeding/reproducibility is CI-enforced. |
| A **good/bad verdict on a real fielded detector** | **NOT CLAIMED** | An AUC here is over model-derived labels from a synthetic corpus, not a field-detection figure. The testbed characterises; it does not certify a device. |

## Why a prime wants this in the bid

- It **de-risks the WP-B deliverable directly**: a turnkey, reproducible, leakage-guarded
  evaluation protocol is exactly the "Characterisation & Evaluation of ML Techniques"
  evidence the SoW asks for — handed over ready, not promised.
- It is a **neutral referee**: Ashforde scores the consortium's algorithms without competing
  to own them, which removes the conflict a classifier-owning partner would carry.
- It is **auditable line-by-line**: open Apache-2.0 core, every metric unit-tested and
  externally validated, every run reproducible from `scenario + seed + engine version`.

## Honest boundaries

- Ashforde is a **WP contributor / SME, never prime and never a mandated Key Person**; this
  is **one component of a partner-led NAV-DT, not the whole twin**.
- The corpus is **synthetic / parameter-grounded** (no field IQ); bundled detectors are
  **published-method baselines, not state-of-the-art**.
- Kshana is a **simulation / evaluation** tool at **TRL ≈ 3** — no flight heritage, no ESA
  endorsement, no certification, no hardware. The on-board / edge AI inference engine is a
  partner's role.

*All claims trace to Kshana v0.18.0 `docs/CAPABILITY.md` / `docs/VALIDATION.md` under the
validated-or-modelled discipline. Companion: [`SE-verification-plan.md`](SE-verification-plan.md)
(§2–3) and the shared [`../cross-tender/verification-evidence-fragment-ECSS-E-ST-10-02.md`](../cross-tender/verification-evidence-fragment-ECSS-E-ST-10-02.md). Contact: **contact@ashforde.org**.*
