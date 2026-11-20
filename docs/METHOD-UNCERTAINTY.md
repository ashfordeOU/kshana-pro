# Re-scoring schedule and per-test uncertainty budgets

Research 2026-11-20 gaps, backlog B2505, Pro part only (the public register and the revocation
list are not in this crate). Module `src/method_uncertainty.rs`, command `kshana-pro method`
(`src/cli/method_cmd.rs`), tests `tests/method_uncertainty.rs`. Feature `pro-method-uncertainty`
(Pro).

## Re-scoring schedule

`method schedule <register.csv>` reads the user's register of scorings
(`product_id,method_id,scored_utc`), takes the latest scoring per product and method, and gives
the next due date after `--interval-months` (12; month ends clamped, 29 February to 28
February) with **OVERDUE**, **DUE SOON** (within `--warn-days`, 60) or **SCHEDULED**.

## Per-test uncertainty budgets

`method budget <method.toml>` gives, per test of a method (JCGM 100:2008 sections 4-6, Annex G):

* Type A from repeated runs: standard deviation of the mean, `n − 1` degrees of freedom (one run
  is refused);
* Type B contributions as standard, 95 % expanded (`/1.96`) or rectangular half-width (`/√3`),
  with sensitivity coefficients, stated or infinite degrees of freedom, and a source each;
* the combined standard uncertainty, each contribution's share, the Welch-Satterthwaite
  effective degrees of freedom and the coverage factor for 95 % from the Student t distribution;
* with a reference value and its expanded uncertainty: the bias and its normalised error `E_n`.

The report states how well tests are measured under the method; product results are "scored
under method X". It rates no product. With `--out`, a signed directory.
