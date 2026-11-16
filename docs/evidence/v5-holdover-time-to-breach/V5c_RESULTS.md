# V5c results: holdover time to breach with orbit-period and flicker terms

Pre-registration: `2026-11-16_V5c_PREREGISTRATION.md` (approved by the owner 16 Nov 2026, approval
commit 60c91516). Method `kshana-pro/holdover/v3` (`HoldoverModel::fit_harmonic`) and the
development table `V5c_dev.json` committed in 78bd30fd, before D-R3 was downloaded.

## Data (recorded before scoring)

* **D-R3** (real): `tests/fixtures/timing_v5c_igs/clocks.txt`, SHA-256
  `a76a398df30297c47199e5bd615ac3b7fb6d7f6678ed066229336aa6955bad48` (IGS final clocks
  2025-10-05 to 2025-10-18, eleven GPS IIF clocks, all complete; source files and their SHA-256 in
  `tests/fixtures/timing_v5c_igs/NOTICE.md`).
* **D-S3** (SYNTHETIC): V5b's D-S2 design, seed 20261120.

Scored once (`KSHANA_PRO_V5C_SCORE=1 cargo test --release --test timing_v5 v5c_scored_as_pre_registered_and_published`)
after the fixture and its SHA-256 were committed (25a0c109); numbers in `V5c_results.json`.

## Verdict: V5c PASSED (B1 to B5 on D-R3 and D-S3)

| Bar | Pass if | D-R3 (real) | D-S3 (synthetic) |
|---|---|---|---|
| B1 95 % interval coverage | 0.90 to 0.99 | 0.9013 (pass) | 0.9217 (pass) |
| B2 early warning share | >= 0.90 | 0.9294 (pass) | 0.9706 (pass) |
| B3 median T_warn / T_act | >= 0.20 | 0.7131 (pass) | 0.282 (pass) |
| B4 T_50 within a factor 2 | >= 0.60 | 0.9343 (pass) | 0.7108 (pass) |
| B5 false-alarm share | <= 0.25 | 0.1285 (pass) | 0.0208 (pass) |

D-R3: 220 runs (none excluded), 411 breach pairs, 249 non-breach pairs. D-S3: 100 runs,
204 and 96. **T1 may now be called claimed for the V5c predictor**; V5 and V5b stay
published misses. The product's fleet assessment still runs the V5b predictor, so product results keep
their "not validated" label until it is switched (`docs/TIMING-ASSURANCE.md`).

B1 passes with little margin (0.9013 against 0.90). Coverage is still uneven clock by clock (G03
0.71 and G30 0.77 under-covered); B1 is a fleet-level bar and is met as written.

## Per clock (D-R3)

| Clock | Runs | Coverage | Early warning | False alarms |
|---|---|---|---|---|
| G03 | 20 | 0.7071 | 0.775 | 3 / 20 |
| G06 | 20 | 0.95 | 0.9464 | 4 / 4 |
| G08 | 20 | 0.9429 | 1.0 | 1 / 21 |
| G09 | 20 | 0.95 | 0.925 | 0 / 20 |
| G10 | 20 | 0.9571 | 0.9 | 0 / 20 |
| G24 | 20 | 0.9357 | 0.975 | 0 / 20 |
| G25 | 20 | 0.8929 | 0.9231 | 7 / 47 |
| G26 | 20 | 0.9429 | 1.0 | 7 / 16 |
| G27 | 20 | 0.9571 | 1.0 | 0 / 20 |
| G30 | 20 | 0.7714 | 0.6364 | 7 / 38 |
| G32 | 20 | 0.9071 | 1.0 | 3 / 23 |

## Development (committed before D-R3 was downloaded, `V5c_dev.json`)

| Data set | B1 | B2 | B3 | B4 | B5 | All |
|---|---|---|---|---|---|---|
| D-R (real, IGS GPS IIF rubidium, emulated holdover) | 0.8857 | 0.8776 | 0.7347 | 0.9365 | 0.1064 | miss |
| D-R2 (real, IGS GPS IIF rubidium 2025-08-17..30, emulated holdover) | 0.9224 | 0.9093 | 0.6899 | 0.9345 | 0.1366 | pass |
| D-S (SYNTHETIC, four classes) | 0.9133 | 0.9602 | 0.3049 | 0.7811 | 0.101 | pass |
| D-S2 (SYNTHETIC, four classes, seed 20261116) | 0.91 | 0.9635 | 0.2926 | 0.724 | 0.1204 | pass |

D-R (V5's period) still missed B1 and B2 in development; the method was not changed.

## Notes

* The `F u^2` term is the `x_rms(u) ~ u sigma_y(u)` approximation for flicker FM, stated as an
  approximation, as pre-registered.
* The fixture was cut by a re-implementation of the open engine's cutter (no numpy, scipy or
  allantools here); its header noise values are written as 0 and are not read by the predictor
  (`tests/fixtures/timing_v5c_igs/NOTICE.md`).
* Holdover is emulated on free-running satellite clocks; no GNSS removal took place.

