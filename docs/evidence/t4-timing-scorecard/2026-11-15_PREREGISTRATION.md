# T4 timing scorecard: pre-registration of the public sample (15 November 2026)

Written and committed **before** the first scoring of `examples/timing-scorecard/request.toml`.
Results go in `RESULTS.md` next to this file, misses included.

## What is scored

* **Fleet:** `examples/timing-fleet/fleet.toml` (T1/T2 demo fleet: `gm-fra-1` and `ref-fra-0`
  are REAL public IGS rubidium clocks; every other record is SYNTHETIC), unchanged.
* **Grandmasters:** `gm-ldn-1` and `gm-fra-1`; cross-check pair `gm-fra-1` / `ref-fra-0`;
  network links as in the request file.
* **Scenarios:** threat library (E2, edition 2026-11) `jammertest-time-spoofing` (time push
  50 ns/s to 10 µs, all receivers), `texbat-ds3-matched-power-time-push` (20 ns/s to 2 µs, one
  receiver), `baltic-timing-holdover-under-jamming` (6 h denial).
* **Method:** `kshana-pro/timing-scorecard/v1` (`src/timing/scorecard.rs`): envelope 3 sigma of
  the T1 prediction plus |mean| plus 20 ns; cross-check `3 sqrt(r1^2 + r2^2) + 20 ns`;
  detection grid 1 s; level 4 within 10 % of a clock's tightest limit. Limits are the fleet's
  user-set limits.

## Disclosure

The T1 assessment of this fleet (models, warning times) was produced and published with T1-T3.
The scorer was developed on a synthetic two-grandmaster fleet written by
`tests/timing_scorecard.rs`, not on this fleet. No T4 quantity (detection time, delivered
error, holdover warning under a scenario, level) was computed for this fleet before this file
was committed.

## Claim bars

* **T4-B1, reproducible and verifiable.** Two scorings give byte-identical
  `timing-scorecard.json`; the signed report verifies with the free verifier, and a one-byte
  change makes it fail.
* **T4-B2, independent route.** (a) For every grandmaster and time-push scenario detected by
  its own envelope, a bisection on the continuous function `|offset(u)| - envelope(u)` finds
  the first crossing within one grid step (1 s) of the reported detection time. (b) For the
  denial scenario, the grandmaster's holdover warning time for each limit agrees with the T1
  fleet assessment's warning time (`fleet::assess`, its own grid) within the coarser of the two
  grid steps, wherever both fall inside the 6 h denial.
* **T4-B3, detection before breach.** For both grandmasters and both time-push scenarios, the
  detector fires before the grandmaster's tightest user-set limit is breached (no
  `breached_before_detection` at the grandmaster for its tightest limit). If met, the sample may
  say: "on the public sample, the holdover-envelope check detected every time push before the
  grandmaster's tightest user-set limit was crossed". If missed, that sentence is not used and
  the miss is published.

These bars say nothing about real attacks on real grandmasters: the oscillator path is the T1
model, and T1's own prediction bars (V5, V5b) missed.
