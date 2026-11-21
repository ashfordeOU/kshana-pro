# LEO-PNT test procedure: leo-doppler-stress, version 1.0 (frozen)

Written for open publication. Machine-readable form: `leo/procedures/leo-doppler-stress.toml` in the
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

## Doppler bounds

The largest Doppler and Doppler rate for the provider's altitude and carrier come from
`kshana-pro leo-eval doppler --altitude-km <h> --carrier-hz <f>` (circular orbit, Earth rotation
neglected: largest range rate `v·R/(R+h)` at the horizon, largest range acceleration `v²/h`
overhead, scaled by `f/c`). Set the simulator's passes to reach them.

## Steps

- **static-reference** (h_rms_m): static receiver, nominal power, no added dynamics: the reference run
- **doppler-max** (availability): satellite passes at the profile's largest Doppler (horizon) for the stated altitude and carrier
- **doppler-rate-max** (availability): overhead passes at the profile's largest Doppler rate
- **doppler-rate-max-accuracy** (h_p95_m): horizontal error during the overhead passes
