# LEO-PNT evaluation

Research 2026-11-20 gaps, epic E21: B2101, B2107 (evaluation only), B2108. Module
`src/leo_eval/`, command `kshana-pro leo-eval` (`src/cli/leo_eval_cmd.rs`), feature
`pro-leo-evaluation` (Pro; `sim-hash`, `claims-template` and `claims-check` free), tests
`tests/leo_evaluation.rs`. The LEO timing scorecard (B2102) is separate (`leo-timing`).

## Neutral multi-simulator procedures and scorecards (B2101)

A procedure (`kshana-pro-leo-sim-procedure/1`) names steps, each with a metric (`h_rms_m`,
`h_p95_m`, `availability`, `ttff_s`, `clock_rms_s`) and optionally a bar with its source.
`sim-hash` gives the SHA-256 to register before the runs; `sim-score` refuses a procedure that
changed. Runs: `simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix`. The scorecard lists
simulators in input order, never ranked, with each step's median and scaled MAD across
simulators; a simulator more than three scaled MADs from the median (with at least three
simulators) "differs from the others", which says they disagree, not which is right.

## Orbit, clock and DCB evaluation and PPP dependency risk (B2107, evaluation only)

* `sisre`: broadcast-minus-precise differences `t,sat,dR_m,dA_m,dC_m,dclk_m[,sigma_m]`;
  `SISRE² = E[u_z²]R² − 2E[u_z]RT + T² + E[1 − u_z²](A² + C²)/2`, with the expectations over the
  users that see the satellite above the mask, computed numerically for the stated altitude
  (checked against the published GPS weights 0.98 and 1/49); RMS, 95th percentile, maximum,
  orbit-only RMS, and the Gaussian overbound of `SISRE/σ` when a broadcast sigma is given.
* `dcb`: day-to-day stability and jumps larger than `k·√(σ₁² + σ₂²)` (k = 3).
* `deps`: a PPP dependency register rated critical (single source, no fallback, convergence
  lost), high, moderate or low, with the reason.

Onboard orbit-integrity methods are not built here.

## Constellation-claim datasets (B2108)

`kshana-pro-leo-claims/1`: per claim the quantity, unit, direction, value, conditions and the
source URL, date and quoted words. A value without all three is refused. `claims-template`
writes Celeste, CentiSpace and Geespace templates whose claims are placeholders: Kshana cites
nothing itself. `claims-compare` puts filled claims beside measured values and says whether each
was reproduced under its own direction.
