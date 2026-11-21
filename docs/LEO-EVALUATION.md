# LEO-PNT evaluation

Research 2026-11-20 gaps, epic E21: B2101, B2107 (evaluation only), B2108. Module
`src/leo_eval/`, command `kshana-pro leo-eval` (`src/cli/leo_eval_cmd.rs`), feature
`pro-leo-evaluation` (Pro; `sim-hash`, `claims-template` and `claims-check` free), tests
`tests/leo_evaluation.rs`. The LEO timing scorecard (B2102) is separate (`leo-timing`).

## Neutral multi-simulator procedures and scorecards (B2101)

The procedures are published documents, frozen at version 1.0: `docs/leo-procedures/`
(Doppler stress, acquisition, signal profiles), machine-readable in `leo/procedures/*.toml`
(`leo-eval sim-procedure <id>`, free). A procedure (`kshana-pro-leo-sim-procedure/1`) names
steps, each with a metric (`h_rms_m`, `h_p95_m`, `availability`, `ttff_s`, `clock_rms_s`), and in
the user's copy a bar with its source and an `equivalence_tolerance`.

As in the LEO timing scorecard, the method hash (SHA-256 of the canonical method part: id,
version, steps) is registered before the runs (`sim-hash`, free); the scorecard and the
equivalence report state whether the registered hash matches, and the label is MODELLED
otherwise. Runs: `simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix[,receiver]`.

* `sim-score`: simulators in input order, never ranked; each step's median and scaled MAD across
  simulators; a simulator more than three scaled MADs from the median (three or more
  simulators) "differs from the others".
* `sim-equivalence`: every simulator against the first listed (by input order only), per step
  with a tolerance, per receiver; verdicts in the simulator self-test vocabulary: `consistent`,
  `simulator-deviation` (at least two receivers differ beyond the tolerance with the same
  sign), `receiver-deviation` (some receivers differ, others agree), `unattributed` (one
  receiver differs), `inconclusive`. A consistent result is not a calibration of either
  simulator.
* `doppler`: the largest Doppler and Doppler rate for an altitude and carrier (circular orbit,
  Earth rotation neglected).

No provider is named, and none is compared as a competitor.

### Requirements cross-walk for user equipment, L-band and C-band (B2101)

`leo-eval crosswalk [--json <out>] [--markdown]` (`pro-leo-ue-crosswalk`, Pro) maps every step of
the published procedures to the public requirement documents read for it: ETSI EN 303 413
V1.2.1 (L-band GNSS receivers: receiver blocking, spurious emissions), Recommendation ITU-R
M.2031 (C-band RNSS receiving earth stations, 5 010-5 030 MHz: interference protection criteria),
the DHS RPCF and NIST IR 8323r1 (diversity and holdover expectations). Each row states how the
step relates to the clause (`measures-same-quantity`, `related`, `evidence-for`,
`no-requirement-in-source`, `none-found`, `gap`); rows with no read source say **to confirm**, and
clauses no procedure exercises are listed as gaps. RTCA, EUROCAE and IEC standards are not public
and are not cited. The table is published as
[`docs/leo-procedures/UE-REQUIREMENTS-CROSSWALK.md`](leo-procedures/UE-REQUIREMENTS-CROSSWALK.md),
kept in step with the code by a test. A cross-walk, never a statement that a receiver meets a
document.

## Orbit, clock and DCB evaluation and PPP dependency risk (B2107, evaluation only)

* `sisre`: broadcast-minus-precise differences `t,sat,dR_m,dA_m,dC_m,dclk_m[,sigma_m]`;
  `SISRE² = E[u_z²]R² − 2E[u_z]RT + T² + E[1 − u_z²](A² + C²)/2`, with the expectations over the
  users that see the satellite above the mask, computed numerically for the stated altitude
  (checked against the published GPS weights 0.98 and 1/49); RMS, 95th percentile, maximum,
  orbit-only RMS, and the Gaussian overbound of `SISRE/σ` when a broadcast sigma is given.
* `dcb`: day-to-day stability and jumps larger than `k·√(σ₁² + σ₂²)` (k = 3).
* `deps`: a PPP dependency register rated critical (single source, no fallback, convergence
  lost), high, moderate or low, with the reason.

* `traim`: a T-RAIM fault-injection study on the open engine's snapshot RAIM with single-fault
  exclusion (position and clock together): per number of satellites in view, the false-alarm
  rate, and per injected fault (single, or several together) the detection rate, correct
  exclusion rate and recovery time after the fault ends. It shows the low-redundancy and
  multi-fault limits of the published algorithm on stated fixtures.

Onboard orbit-integrity methods are not built here.

## Investor due-diligence template (B2108, Pro part)

The open claim datasets and benchmarks are built in the open engine. `leo-eval dd-template
<claims.json> [--measured <claim,value csv>]` writes an investor due-diligence report template from
a claim dataset (`kshana-pro-leo-claims/1`: quantity, unit, direction, value, conditions, source
URL, date and quote). A value without its URL, date and quote is refused; every verdict
(`REPRODUCED`, `NOT REPRODUCED`, `NOT TESTED`) cites the statement it tests and carries its label
(MODELLED unless the measurement names its validation data). `claims-check` (free) checks a dataset.
