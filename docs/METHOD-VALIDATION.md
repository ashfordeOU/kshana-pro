# Interlaboratory precision study (method validation)

Research 2026-11-20 gaps, B2505 (Pro part). Feature `pro-method-validation` (Pro). Commands:
`kshana-pro method validate` and `kshana-pro method synth-study`. The second is free.

The study estimates how precise a scoring method is when several laboratories run it, in the
form of the ISO 5725-2 basic method. It is implemented from the standard's published formulae.
The standard's text has not been read for this build: the critical values are cross-checked
against published table entries (below), and the rest is **to confirm**. The report gives
statistics of the results supplied. It is labelled MODELLED and states precision under the
method as run, nothing more.

## Input

A TOML study:

```toml
title = "Interlaboratory study"
method_id = "KM-1"
quantity = "time-to-detect"
unit = "s"
# alpha_straggler = 0.05, alpha_outlier = 0.01 (defaults)
# exclude = [{ lab = "L7", reason = "calibration error found on investigation" }]

[[level]]
name = "A"
reference_value = 12.0          # optional
[level.results]
L1 = [12.1, 11.8, 12.3]
L2 = [12.6, 12.4, 12.9]
L3 = [11.5, 11.9, 11.7]
```

At least three laboratories are needed per level. Replicate counts may differ between
laboratories.

## Output

For each level:

- **Cells:** each laboratory's n, mean and standard deviation, Mandel's h and k, and whether
  it was excluded.
- **Precision:**
  - repeatability `s_r² = Σ(n_i − 1)s_i² / Σ(n_i − 1)`;
  - between-laboratory `s_L² = max(0, (s_d² − s_r²)/n̄)`, where
    `s_d² = Σ n_i(ȳ_i − ȳ)²/(p − 1)` and `n̄ = (Σn_i − Σn_i²/Σn_i)/(p − 1)`;
  - reproducibility `s_R² = s_L² + s_r²`;
  - limits `r = 2.8 s_r` and `R = 2.8 s_R`.
- **Consistency tests:** Cochran's C on the largest variance, and Grubbs' statistic on the
  largest and smallest cell means. Each is classed `ok`, `straggler` (above the 5 % critical
  value) or `outlier` (above 1 %).
  - Cochran's critical value is computed from F as
    `1/(1 + (p − 1)/F(α/p; n − 1, (p − 1)(n − 1)))`.
  - Grubbs' critical value is computed from t as
    `((p − 1)/√p)·√(t²/(p − 2 + t²))`, with `t = t(α/(2p); p − 2)`.
  - When replicate counts differ, Cochran uses the most common n (to confirm).
- **Bias** (with a reference value): the mean of the cell means minus the reference, against
  the bound `1.96 √((s_R² − (1 − 1/n̄)s_r²)/p)`.

Laboratories are excluded only when the request lists them with a reason. The precision is
then reported both with all laboratories and after the exclusions. The report never removes
a laboratory on its own.

## Checks

`tests/method_validation.rs` covers:

- the critical values against published table entries:
  - Cochran p = 10, n = 2 (0.602 / 0.718) and p = 3, n = 2 (0.967 / 0.993);
  - Grubbs p = 10 (2.290 / 2.482) and p = 20 (2.709 / 3.001);
- `s_r`, `s_L`, `s_R`, the bias bound and Mandel h/k on a case worked by hand;
- the synthetic study, where the biased laboratory (L7, Grubbs) and the scattered one
  (L3 at level B, Cochran) are found, and an exclusion with a reason lowers R and the bias;
- the signed pack behind a licence.

## Not built

- The double-Grubbs test, the iterative removal procedure and the robust methods of
  ISO 5725-5.
- Trueness by ISO 5725-4 beyond the single-reference bias bound.
- Any statement that a method has been accepted by a body.
