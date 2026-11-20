# Power-grid and broadcast SFN timing evidence (backlog E31: B3101, B3102)

Two commands built on the timing assurance layer (`docs/TIMING-ASSURANCE.md`): `kshana-pro
grid` (feature `pro-grid-timing`, Pro) and `kshana-pro sfn` (feature `pro-sfn-timing`, Pro;
`sfn systems` is free). Code: `src/timing/grid.rs`, `src/timing/sfn.rs`, CLI
`src/cli/grid_cmd.rs`; tests `tests/grid_sfn_timing.rs` (synthetic records only).

They reuse what is on `main`: the self-check's clock-log readers
(`timing::selfcheck::read_source`: `csv`, `ptp4l`, `phc2sys`, `ts2phc`, `chrony-tracking`,
`chrony-statistics`, `pps-assert`, `ntp-peerstats`), the holdover evidence model with its GUM
budget (`timing::holdover_evidence::assess`), the signed delivery directory
(`signed_dossier`, verified free with `kshana-pro dossier verify`), and the open engine's
C37.118.2 reader (`kshana::c37118`, kshana 0.41.0), which reads each PMU's time flags.

**Every result is MODELLED**: arithmetic on the operator's records as supplied, tested on
synthetic records. A finding is evidence for the operator to investigate. NERC, IEEE and ETSI
figures are quoted as the criteria the evidence is measured against (each "to confirm against
the current text"); the output never states that a standard or regulation is met. That is for
the operator, its auditor and its regulator. Floats in signed packs are rounded to 9
significant digits.

## Power grid (`kshana-pro grid`)

A time error `Δt` at a PMU is a phase-angle error `Δθ = 360°·f0·Δt` (0.0216°/µs at 60 Hz,
0.018°/µs at 50 Hz) and a total vector error `2·|sin(Δθ/2)|`. The IEEE C37.118.1 steady-state
TVE criterion of 1 % is used up by a pure phase error at 26.53 µs (60 Hz) or 31.83 µs (50 Hz):
that is the default timing budget, scaled by `timing_share` when the operator allots timing
only part of the TVE.

The clock log of a device is its time error against an **independent** reference (a second
GNSS-independent clock, PTP from another grandmaster, a portable reference): `csv` is
`unix_time,offset_s[,locked]`, device minus reference.

### `grid pmu <request.toml> [--json <out>]`

```toml
nominal_hz = 60.0
tve_budget = 0.01        # default
timing_share = 1.0       # default: timing alone
match_tolerance_s = 0.5  # clock reading to C37.118 report
[[pmu]]
name = "SUB-A PMU 1"
id = 7                   # IDCODE in the capture (needed when it holds several PMUs)
capture = "sub-a.c37"    # concatenated C37.118.2 frames (CFG-2 first)
clock = { format = "csv", path = "pmu7-te.csv" }
[[event]]
name = "line trip"
start_unix = 1795092600
end_unix = 1795092660
```

Per PMU: max |TE|, phase-angle error and TVE from timing alone, the runs beyond the budget,
and the capture's own view (reports, sync-error reports and intervals, worst claimed time
quality, clock-fault and leap-pending reports, frames with a bad CRC). Two joins of the clock
log with the capture: readings whose error exceeded the time quality the PMU claimed at that
moment, and readings beyond the budget while the PMU reported itself in sync (status
`beyond-budget-flags-silent`: the case of a gradual spoof the receiver follows). Per event
window: each PMU's median error and phase error, and the angle-difference error of every PMU
pair, `360°·f0·(Δt_a − Δt_b)`, which is what a post-event angle comparison inherits.

### `grid drift <request.toml> [--json <out>] [--csv <trace.csv>]`

Gradual-spoof detection on a recorded series:

```toml
nominal_hz = 60.0                               # optional for a clock; needed for an angle pair
clock = { format = "csv", path = "pmu7-te.csv" } # or:
# angle_pair = { path = "bus-3.csv" }            # unix_time,angle_a_deg,angle_b_deg
[detector]
noise_s = 50e-9        # standard uncertainty of one reading
allowed_rate = 1e-9    # the clock's largest plausible rate (s/s)
window_s = 300         # rate-test window
k = 5                  # rate-test coverage factor
persist = 3            # consecutive exceedances
cusum_kappa = 0.5      # CUSUM reference, in noise_s
cusum_h = 20           # CUSUM threshold, in noise_s
step_k = 8             # a single increment beyond step_k·√2·noise_s is a step
baseline_s = 600       # the first seconds set the baseline
```

Three tests run together: a two-sided level CUSUM against the series' own baseline (catches a
departure of a few `noise_s` quickly), a sliding-window least-squares rate test against the
plausible rate (tells a drift from a level shift), and a step test. The report gives the first
alarm, its kind (`gradual-drift`, `step`, `level-shift`, `none`), the estimated onset (where
the CUSUM last left zero), the rate fitted from the onset to one window past the alarm, the
time error at the alarm, the time the error used up the budget (default: the TVE budget when
`nominal_hz` is given) and the warning between the two.

An **angle pair** (two PMUs on the same bus, or electrically very close, one of them timed
from an independent source) gives detection without a clock log at the PMU under test: the
standing angle difference is removed (median of the baseline) and the change is read as a time
error. Only valid where the load flow cannot move the angle difference by the amounts at
stake; the operator chooses the pair.

### `grid scada <request.toml> [--json <out>]`

```toml
path = "scada.csv"            # device,source_unix,receive_unix[,seq]
ahead_tolerance_s = 0.001
max_latency_s = 10
step_s = 0.5                  # change of the latency floor that counts as a clock step
window_s = 3600               # window of the latency floor
required_resolution_s = 0.001 # optional
```

Per device, in arrival order (by sequence number when every record has one): stamps going
backwards, repeated stamps, stamps ahead of the master's receive time (device clock ahead),
stale records, the latency floor per window (its steps are clock steps; its slope, without
steps, is the device clock's drift in s/day) and the apparent resolution (the coarsest
decimal step every stamp sits on). The latency includes transport, so this checks consistency;
it does not measure the device's offset from UTC.

### `grid prc002 <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>]`

```toml
site = "Substation North"
gap_s = 300                   # a longer gap between readings is a monitoring gap
# limit_s = 0.002             # default: ±2 ms (NERC PRC-002 R10, to confirm)
# period_start_unix / period_end_unix: default the span of the logs
[[device]]
name = "DFR-1"
kind = "dfr"
clock = { format = "csv", path = "dfr1.csv" }
[device.resilience]           # optional: the holdover evidence model
period_h = 24
oscillator = { class = "ocxo" }
u_initial_te_s = 1e-6
```

Per device: readings, max |TE|, share within the limit, runs beyond it, monitoring gaps and
coverage, status (`within`, `within-with-gaps`, `beyond`, `no-data`), and resilience: how long
the device stays within the limit after losing its reference (largest supported holdover, the
bound at the stated period), from its oscillator through the holdover evidence model. Writes
`report.json`, `report.html`, `inputs/prc002.toml`, signed.

### `grid event <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>]`

Was timing a cause of a grid event?

```toml
name = "line trip"
start_unix = 1795092600
end_unix = 1795092620
pre_s = 3600              # searched for a drift onset before the window
nominal_hz = 60.0
coverage_gap_s = 60       # the clock log must reach this close to both ends of the window
[[device]]
name = "PMU-7"
kind = "pmu"              # a PMU's budget is the TVE budget, any other device's ±2 ms
id = 7
capture = "sub-a.c37"
clock = { format = "csv", path = "pmu7-te.csv" }
[[device]]
name = "SER-2"
kind = "ser"
clock = { format = "csv", path = "ser2.csv" }
# log_name = "SER2"       # its name in the SCADA and sequence-of-events logs
[scada]                   # optional, as for `grid scada`
path = "scada.csv"
[soe]                     # optional
path = "soe.csv"          # device,unix_time,label
[[soe.order]]
before = "relay 21 trip"
after = "breaker 52A open"
min_gap_s = 0.0
```

Per device: `timing-implicated` (measured error beyond its budget in the window, C37.118
reports flagged not in sync, or a recorded event order that the device's measured clock error
explains), `timing-suspect` (a drift alarm in the look-back, or SCADA clock findings, within
budget), `timing-not-implicated` (clock evidence covering the window, within budget, no flag),
or `insufficient` (no covering clock evidence). Each expected order is re-checked after
correcting each stamp by its device's measured error at that time: `consistent`,
`inverted-explained-by-timing`, `inverted-not-explained`, `inverted-clock-unknown`,
`consistent-only-before-correction` or `missing`. A device is implicated by an order only when
its own correction carries at least half of the change. Writes `report.json`, `report.html`,
`inputs/event.toml`, signed.

## Broadcast SFN (`kshana-pro sfn`)

In a DAB or DVB-T single-frequency network the arrival-time difference of two transmitters in
their overlap zone must stay inside the guard interval; the network plan spends most of it on
path differences and static delays. A time error that differs between two transmitters spends
the rest. SVN-23 (26 Jan 2016): GPS broadcast UTC parameters about 13 µs in error; equipment
that followed the error and equipment that rejected it diverged, about 40 elements of one
operator entered holdover, and DAB SFNs were disturbed (research 07b [T69][T70]): resilience
at the transmitter level, not at UTC level.

`sfn systems` lists the guard intervals known: DAB modes I–IV (504, 126, 63, 252 T with
T = 1/2.048 MHz: 246.09, 61.52, 30.76, 123.05 µs) and DVB-T 2k/8k with guard 1/4 to 1/32 at
5–8 MHz (T = 7/(8·BW) µs; 8k 1/4 at 8 MHz = 224 µs). DVB-T2 and others: give `guard_us`.

### `sfn assess <request.toml> --out <dir> --signer <name> --key-file <file> [--utc <iso>]`

```toml
network = "Region DAB SFN"
system = "dab-mode-1"         # or guard_us = 246.09
utc_limit_s = 1e-6            # the network's UTC-level limit
extra_margin_us = 0           # guard reserved for other items
coverage_k = 2
locked_u_s = 50e-9            # u of a locked transmitter's time error
bin_s = 60
[[transmitter]]
name = "TX-A"
on_bad_reference = "accept"   # accept | holdover | mute
log = { format = "csv", path = "tx-a.csv" }   # TE against an independent reference
oscillator = { class = "ocxo" }               # for holdover
[[pair]]                      # overlapping pairs (default: every pair, planned 0)
a = "TX-A"
b = "TX-B"
planned_offset_us = 150       # the plan's worst-case arrival difference in the overlap zone
[[scenario]]
name = "SVN-23"
preset = "svn23"              # utc-step of 13 µs, 12 h (illustrative)
[[scenario]]
name = "GNSS lost"
kind = "gnss-loss"            # utc-step | gnss-loss | ramp
duration_h = 24
# transmitters = ["TX-A"]     # the affected ones (default: all)
# step_s / rate_s_per_s
```

**Recorded logs.** Binned medians per transmitter; per bin the common-mode error (median
across transmitters; beyond the UTC limit = `utc-level` event), each transmitter's departure
from it (`transmitter-offset`), and each pair's margin, guard − |planned| − extra −
|TE_a − TE_b| (below zero = `pair-margin` event: self-interference risk in the overlap zone).

**Scenarios.** Each affected transmitter follows the error (`accept`), holds over on its
oscillator (`holdover`, through the holdover evidence model) or goes off air (`mute`); a ramp
is followed by all affected transmitters (a slow drag raises no alarm). Per pair: relative
mean, bound `|m_a − m_b| + k·sqrt(u_a² + u_b²)`, margin and status (`ok`,
`self-interference-risk`, `one-muted`, `both-muted`); per network: `intact` or
`self-interference-risk`, `within` or `utc-level-error`, coverage `full` or `reduced`, the
alarms and holdover entries, and for a GNSS loss the holdover after which the first pair, and
the first transmitter, runs out. Writes `report.json`, `report.html`, `inputs/sfn.toml`,
signed.

The test reproduces SVN-23 both ways: four DAB mode I transmitters' logs where three follow a
13 µs step and one rejects it (UTC-level event, the odd transmitter's departure, the pair
planned at 240 µs of 246 µs runs out of margin, the pair planned at 150 µs does not), and the
`svn23` preset (all follow: intact but off UTC; mixed: self-interference risk; mute: coverage
reduced).

## Not built

* No live C37.118 or SCADA protocol client (IEC 60870-5-104, DNP3, IEC 61850): recorded
  captures and exports only. The phasor values themselves are not decoded from C37.118
  frames (the open reader steps over them); angle pairs come from the operator's CSV export.
* No DVB-T MIP/ETI/EDI time-stamp parsing (the SFN's own distribution-network timing); the
  transmitter's emitted time error comes from the operator's monitoring log.
* No transmitter or receiver positions: the overlap-zone path differences are the operator's
  planned offsets. No direction finding or location of any emitter.
* No signal generation of any kind: the drift fixtures are recorded-style time-error and angle
  series, not RF.
* Nothing here has been run against a real PMU, substation, wind farm or broadcast network.
