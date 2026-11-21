# LEO-PNT test procedure: leo-signal-profiles, version 1.0 (frozen)

Written for open publication. Machine-readable form: `leo/procedures/leo-signal-profiles.toml` in the
Kshana Pro repository; its method hash is printed by `kshana-pro leo-eval sim-hash`.

## Status

Version 1.0 is frozen: any change to the steps, metrics, bars or tolerances is a new version
with a new method hash.

## How to run it

1. Copy the TOML. Add, per step, the bar you hold the receiver to (`bar`, `bar_source`) and
   the tolerance within which two simulators count as equivalent (`equivalence_tolerance`).
   These change the method hash.
2. Register the hash (`kshana-pro leo-eval sim-hash <your.toml>`) before taking any run.
3. Run every step on each simulator with the same receiver(s), logging
   `simulator,step,t_s,err_e_m,err_n_m,err_u_m,clock_err_s,fix,receiver`.
4. Score (`sim-score`) and check simulator equivalence (`sim-equivalence`), giving the
   registered hash.

Simulators are reported in input order and never ranked. Equivalence verdicts use the simulator
self-test vocabulary: `consistent`, `simulator-deviation`, `receiver-deviation`,
`unattributed` (one receiver) and `inconclusive`.

## Steps

- **profile-nominal** (h_rms_m): the signal profile as the provider describes it, nominal power
- **profile-low-power** (availability): the same profile at the lowest power the profile states
- **profile-few-satellites** (h_p95_m): the same profile with the fewest satellites in view the profile allows
- **profile-with-gnss** (h_rms_m): the LEO profile together with GNSS
