# Kshana Pro — Pack A Detector-Intake Format

> PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.

The format a customer uses to submit **their own detector's** scored outputs so we
compute their *field-vs-bench* (ID-vs-OOD) **optimism gap** with the same engine the
published study uses.

This is part of Pack A — the Optimism-Gap Audit. The published proof of the
phenomenon and the ID-only predictor is **arXiv:2606.22054**, an arXiv **preprint**
(NOT peer-reviewed, NOT a certification).

---

## What you submit

A flat list of **scored, labelled samples**. Each sample is one detector decision on
one case:

| Field          | Type                | Meaning                                                                 |
|----------------|---------------------|-------------------------------------------------------------------------|
| `score`        | number (`f64`)      | Your detector's scalar decision statistic. **Higher = more likely the impairment is present.** |
| `label`        | boolean             | Ground truth: `true` = impairment genuinely present (a positive); `false` = clean (a negative). |
| `distribution` | `"id"` or `"ood"`   | Which regime the sample came from: `id` = in-distribution / **bench** (tuned, nominal conditions); `ood` = out-of-distribution / **field** (shifted, subtler conditions). |

### Orientation (load-bearing)

`score` **must** be oriented so a higher value means *more impaired*. If your
detector outputs a "health" or "confidence-it-is-clean" score, negate it (or
otherwise flip it) before submitting. AUC is computed under the "higher ⇒ positive"
convention, exactly as in the open engine.

---

## What we compute

We split your samples by `distribution`, compute the AUC over each subset
(positives vs negatives) using the open engine's **exact Mann–Whitney AUC routine**
(`kshana::impairment_eval::auc` — the same routine the published baseline uses), and
return:

| Field          | Meaning                                                          |
|----------------|-----------------------------------------------------------------|
| `auc_id`       | AUC over the in-distribution (bench) subset.                    |
| `auc_ood`      | AUC over the out-of-distribution (field) subset.                |
| `optimism_gap` | `auc_id − auc_ood`. **Positive ⇒ your bench number is optimistic** — it over-states how the detector does in the field regime. |
| `n_id`         | Number of in-distribution samples used.                         |
| `n_ood`        | Number of out-of-distribution samples used.                     |

No math is re-derived for your submission: it is the open engine's AUC, so your
number is computed identically to ours.

---

## Validation / rejection

The intake is rejected with a clear error when the data cannot support an honest
AUC:

- **Empty** input (no samples at all).
- A **missing distribution** (no `id` samples, or no `ood` samples).
- A **single-class subset** (all positives or all negatives within `id` or within
  `ood`) — AUC needs both classes.
- A **non-finite score** (`NaN` / `±∞`).

---

## Worked example (JSON)

```json
{
  "samples": [
    { "score": 9.0, "label": true,  "distribution": "id" },
    { "score": 8.0, "label": true,  "distribution": "id" },
    { "score": 2.0, "label": false, "distribution": "id" },
    { "score": 1.0, "label": false, "distribution": "id" },

    { "score": 7.0, "label": true,  "distribution": "ood" },
    { "score": 3.0, "label": true,  "distribution": "ood" },
    { "score": 4.0, "label": false, "distribution": "ood" },
    { "score": 2.0, "label": false, "distribution": "ood" }
  ]
}
```

On the bench (`id`) subset the two positives (9.0, 8.0) sit cleanly above the two
negatives (2.0, 1.0), so `auc_id = 1.0`. In the field (`ood`) subset one positive
(3.0) drops into the negative range (4.0, 2.0), so `auc_ood = 0.75` and the
**optimism gap is 0.25** — the bench AUC over-stated field performance by a quarter
of the AUC range.

Result:

```json
{
  "auc_id": 1.0,
  "auc_ood": 0.75,
  "optimism_gap": 0.25,
  "n_id": 4,
  "n_ood": 4
}
```

---

## Honest scope

This is a MODELLED engineering measurement over your own labelled data. It reports
operating characteristics only — it is **not a certification**, not ESA acceptance,
and not a TRL claim. The optimism gap measured here is between *your* two submitted
regimes; the published study's gap is a controlled synthetic→synthetic severity
shift (see arXiv:2606.22054, an arXiv preprint).
