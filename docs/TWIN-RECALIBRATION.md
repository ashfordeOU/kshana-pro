# Digital-twin recalibration from telemetry (evidence pack on the hosted `calibrate` verdict)

Module `src/twin_recal.rs`, command `kshana-pro twin-recal` (`src/cli/twin_recal_cmd.rs`), tests
in `tests/clock_bench.rs` (module `twin_recal`, with a mock service). Feature
`pro-twin-recalibration` (Pro).

**No fitting code is in this crate.** General parametric calibration of a simulator against
telemetry is the hosted verdict service's `calibrate` method, which ships in no Pro build (owner
decision of 19 November 2026; [REMOTE-VERDICTS.md](REMOTE-VERDICTS.md)). This pack is built
around it.

`twin-recal run <request.toml> --out <dir> --signer <name> --key-file <file> [--url ...]
[--trust ...] [--license ...]`:

1. reads the flown telemetry (`t_s,value,sigma`) and splits it into windows of `window_s`;
2. per window, writes the `calibrate` request: the stated `[[parameter]]` tables (initial, prior,
   prior sigma, bounds) and the model;
3. sends it through the existing verdict client and **verifies** each signed verdict against the
   trusted service key (a verdict from another key stops the run);
4. computes locally, with the local model, the residual RMS and reduced chi-square of each window
   before (initial parameters) and after (returned posterior);
5. fits, per parameter, a weighted straight line to the posterior values against window mid-time
   (weights from the service's standard errors). That gives the **drift per day** with its
   standard error, called significant above `drift_k` (3) standard errors.

Worked models:

| `[model] kind` | observable | parameters | sent as |
|---|---|---|---|
| `clock` | time error (ns) | `x0` (ns), `y` (ns/s), `d` (ns/s²); local model `x0 + TIE(y, d, t)` with the open engine's `holdover::deterministic_tie` | expression `x0 + y*t + 0.5*d*t*t` |
| `orbit-decay` | altitude (km) | `b` = Cd·A/m (m²/kg); local model: near-circular drag decay in the open engine's US76 density from `altitude0_km` | design of runs over `b_design` (the local model's predictions per run) |

The signed pack holds `pack.json`/`pack.html`, every request (`exchanges/NN-request.json`) and
every signed verdict (`exchanges/NN-verdict.json`), and the inputs. MODELLED: calibrated
parameters explain the telemetry under the model; they are not a validation.
`twin-recal synth <dir>` writes four days of synthetic clock telemetry whose frequency grows day
by day.
