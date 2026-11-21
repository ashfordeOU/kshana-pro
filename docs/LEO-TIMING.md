# LEO timing verification

`kshana-pro leo-timing` scores a timing receiver that takes time from a low-Earth-orbit (LEO)
service on its own field logs, under a method registered before the logs were taken (research
2026-11-20 gaps, backlog E21 B2102). Source: `src/leo_timing/`. Tests: `tests/leo_timing.rs`.

```
kshana-pro leo-timing synth demo                       # synthetic logs and plan.toml (free)
kshana-pro leo-timing hash demo/plan.toml              # the method hash to register (free)
kshana-pro leo-timing score demo/plan.toml --out result --signer lab --key-file lab.key   # Pro
kshana-pro leo-timing check result --trust <public key hex>                              # free
```

Licence: `score` needs `pro-leo-timing-scorecard` (Pro). `hash`, `check` and `synth` are free.

## Pre-registration

The method part of a plan is the method id, `[thresholds]` (`max_abs_te_ns`, `availability_min`),
every `[[claim]]` and `replay_window_s`. `leo-timing hash` prints its SHA-256 over canonical JSON.
Register that hash (publish it, time-stamp it, give it to the counterparty) before taking logs and
write it into the plan as `prereg_sha256`. The scorecard recomputes the hash: a threshold or a claim
changed after registration no longer matches and the scorecard says so. The logs are not part of
the hash, so adding logs after registration is allowed.

## What is scored

* **Per environment** (`[[log]]` entries share a label: `indoor`, `urban`, `sea`, `open`, ...):
  availability of a timing solution, RMS, 95th percentile and maximum absolute time error over the
  valid samples, against the stated thresholds; the maximum against the ITU-T G.8272 PRTC-A
  maximum absolute time error (100 ns, from the open engine's mask table) as a measured comparison;
  MTIE and TDEV of the first log when its samples are evenly spaced and all valid.
* **Claims** (`[[claim]]`: `metric` one of `rms_te_ns`, `p95_abs_te_ns`, `max_abs_te_ns`,
  `availability`; the environment; the value; the published source): `reproduced` when the
  measured figure is at least as good, `not-reproduced` otherwise, `not-evaluated` without logs
  for that environment.
* **Authentication and replay window** (`[replay]`: CSV `rx_s,tx_s,accepted`): messages accepted
  later than the window after their authenticated transmit time (each one a replay the receiver
  let in), accepted with a negative latency, refused although inside the window, the largest
  latency accepted; `held` or `breached`. Both columns must be on one time scale (state it as
  `time_scale`), or the request must state `tx_minus_rx_scale_s`, the transmit scale minus the
  receive scale (18 s for GPST against UTC), which is taken off the transmit times. The result
  records which applied. A median latency within 1 s of 18, 19 or 37 s with a window under 15 s
  raises `scale_warning`.

Log formats: time error `t_s,te_ns,valid` (time error against the lab or field reference, `valid`
1 when the receiver reported a timing solution); authentication `rx_s,tx_s,accepted`. Rows that do
not parse are counted in `rows_rejected`.

## Evidence class

`VALIDATED` only when every log is marked `synthetic = false` and the plan's `prereg_sha256`
matches the method hash; otherwise `MODELLED`, with `evidence_reason`. The result is a signed
directory (`scorecard.json`, `summary.txt`, the inputs and their hashes). It is test evidence: it
never says a receiver or a service is compliant, conformant or certified.

## Not built here

* The receiver-integration test kit for LEO timing modules (backlog B2102, "ASIC integration kit"):
  needs a hardware interface definition per module.
* The neutral multi-simulator LEO-PNT procedures and scorecards (B2101) and LEO orbit determination
  and time synchronisation integrity (B2107).
