# Space-segment GNSS receiver evidence

`kshana-pro space-rx` (module `kshana_pro::space_rx`): the receiver-side view of a GNSS
receiver flown on a spacecraft. What it meets in orbit, what its test campaign shows, and how
its faults look in its own output. Research 2026-11-20 gaps, epic E20 (items B2001, B2002,
B2003).

**Evidence label: MODELLED.** No receiver has been tested with these tools in this build. The
sample campaign, mission profile, environment spectra, orbit and traces are SYNTHETIC
(`kshana-pro space-rx sample ...`). A scorecard is evidence for a part selection or a review
against the limits a mission states; it never says a part is fit for flight or meets a
standard. Radiation and interface robustness are modelling and test planning only.

**Export-control scope.** Signal-parameter level only: nothing here locates an emitter
(areas are regions; a map cell is a region), controls an antenna, fuses navigation sensors,
handles restricted signals or produces a waveform. The test points are levels and offsets for
a simulator run in the receiver team's own lab.

## Commands

| Command | Licence feature | Tier |
|---|---|---|
| `exposure <scenario.toml> --out <dir> [--signer --key-file]` | `pro-space-rx-exposure` | Pro |
| `score --campaign --mission --out --signer --key-file` | `pro-space-rx-qual` | Pro |
| `db add --db <db.json> --scorecard <dir> [--trust]` | `pro-space-rx-qual` | Pro |
| `detect --trace [--params] [--truth] --out --signer --key-file` | `pro-space-rx-see` | Pro |
| `vectors --trace --out` | `pro-space-rx-see` | Pro |
| `tolerance --vectors --log --out --signer --key-file` | `pro-space-rx-see` | Pro |
| `rate <model.toml> [--out]` | `pro-space-rx-see` | Pro |
| `db select`, `db schema`, `inject`, `sample`, `verify` | none | free |

Signed output directories (`signed_dossier`, Ed25519) are never overwritten. `tolerance` exits
1 when a vector is not met; `detect --truth` exits 1 when an injected signature is missed.

## B2001: ground-interference envelope from orbit (`exposure`)

Input `kshana-space-rx-exposure/1`: a TLE (propagated with the open engine's SGP4, TEME to
ITRS), a start time and span, assumed EIRPs, the receiver (antenna boresight zenith or nadir,
gain pattern over the angle off boresight, gain toward GNSS, signal power, noise temperature,
tracking threshold, spectral-separation Q, and the J/S it was tested to), and interference
areas: declared (centre, radius, EIRPs, source) and/or the degraded cells of an open-engine
interference map (GeoJSON `status` degraded/anomalous or observatory `state` degraded).

Per step and area in view (spacecraft above the mask seen from the point of the area nearest
the sub-satellite point: the worst case for the receiver), the model computes J/S, the
effective C/N0 from the open engine's anti-jam equation, and the Doppler offset of a static
ground transmitter at the spacecraft. Outputs: passes per area (`passes.csv`), per-revolution
in-view and below-threshold fractions, and the **test envelope**: J/S p50/p95/max per EIRP,
time below the tracking threshold, time above the tested J/S, pass dwell, |Doppler offset|
and |Doppler rate|, with one test point per EIRP. A ground transmitter that emulates signals
for a static ground user is offset from the authentic signals by roughly the Doppler shown
(tens of kHz in LEO), which is the parameter range a spoofing-resilience test of a space
receiver needs.

On the sample (510 km sun-synchronous orbit, two illustrative areas), the qualitative pattern
reported for spaceborne receivers over interference regions is reproduced: exposure only on
passes within radio view of the areas, and loss of tracking only near closest approach and
only at the strongest assumed EIRP (`tests/space_rx.rs`). That is a qualitative check, not a
validation against flight data.

## B2002: qualification scorecards, selection database, template (`score`, `db`)

`kshana-space-rx-campaign/1` records one receiver's campaign (`data_class` synthetic, public
with a citation, or own): TID steps with navigation outputs, supply current, TTFF; SEE runs
(heavy ion or proton, LET, fluence, latch-up, SEFI, reset, upset counts); vibration per axis
with tracking; thermal and thermal-vacuum runs; tracking cases; interference runs (J/S and
whether tracking held). `kshana-space-rx-mission/1` states the mission's limits, each with a
source: dose with radiation design margin, latch-up LET threshold, SEFI cross-section,
position error, TTFF, current increase, vibration level and C/N0 drop, temperature range,
cycles, vacuum, tracking cases, J/S.

The scorecard gives each check met, not met or not assessed (no data, no limit, or a campaign
that stopped short), with the figures. The selection database `kshana-space-rx-db/1` keeps the
test-backed figures per campaign (dose functional to, latch-up LET seen, highest LET tested,
SEFI cross-section, vibration and temperature functional, J/S held, cases) and the scorecards
run; `db schema` prints its JSON Schema, `db select` filters on dose and J/S. The template
(`template.md`) is a qualification process outline in the shape of published GNSS payload
campaigns (requirements, part selection, TID, SEE, mechanical, thermal, functional,
interference, EMC, acceptance), each evidence slot filled from the campaign or marked OPEN.
ECSS and ESCC documents are cited by number only. Inputs that use claim words are refused.

The J/S held in the campaign is the `tested_js_db` of the exposure scenario, so the envelope
reports the time in orbit beyond what the receiver was tested to.

## B2003: radiation-fault signatures, interface vectors, SEE rates

**`detect`** reads `kshana-space-rx-trace/1` (CSV: OBC time, receiver time of week, ECEF
position and velocity, clock bias, satellites, mean C/N0, fix, reset counter, supply current)
and reports signatures with a class:

| Signature | Class |
|---|---|
| `latchup_suspect` (current above baseline for several epochs; notes a following reset) | SEL |
| `reset` (gap or loss of fix with the reset counter rising) | SEFI/SEL |
| `functional_interrupt` (output gap without reset) | SEFI |
| `frozen_output` (same solution and time tag while the OBC clock runs) | SEFI |
| `output_upset` (one epoch off the two-body dynamics, then back; bit-flip test on the difference at the output resolution) | SEU |
| `timetag_upset`, `clock_upset` (one epoch off, then back) | SEU |
| `timetag_step`, `solution_step`, `clock_step`, `clock_ms_jump`, `loss_of_fix` | other |

Class `other` is behaviour that persists or is normal receiver behaviour (a millisecond clock
steering step), reported so it is not mistaken for a radiation effect. Thresholds are
parameters with defaults written into every result. **`inject`** writes a SYNTHETIC fixture
with ground truth (`bitflip`, `timetag`, `clock`, `freeze`, `sefi`, `reset`, `latchup`);
`detect --truth` scores against it.

**`vectors`** builds an NMEA 0183 replay stream from a trace: nominal GGA and RMC per epoch
with fifteen corrupt classes interleaved (checksum bit flip, invalid checksum characters,
truncation, missing terminator, over 82 characters, control bytes, garbage, empty fields,
latitude out of range, no fix, duplicate, time going backwards, a position jump with a
recomputed checksum, implausible altitude, unknown sentence), each with the actions an OBC may
take. A reference parser applies those rules and must agree before vectors are written
(`reference-actions.csv`). **`tolerance`** scores an OBC log (`id,action`) per vector and
class: met, not met, or not assessed when missing.

**`rate`** (`kshana-space-rx-see-rate/1`) folds a Weibull cross-section per effect (heavy ion,
optional proton) with integral LET and proton spectra per environment (for example from a
CREME96 or OMERE run) into events per day, per year and mean days between. Simplifications are
written into the result: omnidirectional flux, normal-incidence cross-section, no chord-length
treatment. A planning figure, not a predicted on-orbit rate.

## Not built here

- B2004 (receiver timestamp-fault detector and onboard clock behaviour) and B2005 (GEO/HEO
  and high-dynamics scenarios, upper-SSV benchmark) belong to the open engine per the backlog;
  `detect` covers only time-tag and clock signatures of single-event effects.
- Validation against flight traces or a published campaign: none in this build.

## Resilience evaluation under degradation scenarios

`space-rx resilience <request.toml> --out <dir> --signer <name> --key-file <file>`
(`pro-space-rx-resilience`, Pro; module `src/space_rx/resilience.rs`, tests in
`tests/space_rx.rs`). Scores a space receiver's **logged behaviour** against a reference
trajectory and clock over the degradation windows a campaign applied: `outage`, `scintillation`,
`ephemeris-fault`, `above-constellation` (weak, sparse signals at high altitude).

The log has the space-receiver trace columns (`t_s,x_m,y_m,z_m,clock_bias_m,...,fix`) plus an
optional `integrity_flag`. The reference has `t_s,x_m,y_m,z_m,clock_bias_m`. The mission limits
(position, time, minimum availability, maximum recovery, maximum time to flag a fault, settle
time) are stated with their source.

Per scenario it reports:

* availability, and the 95th percentile position error;
* epochs with a fix beyond the position limit and no flag (misleading, unflagged);
* the time to the first flag;
* the time to recover (a fix within the limit held for the settle time);
* holdover: the clock's time error during no-fix epochs, against the last fixed epoch's offset.

Each check is met or not met against the limits. The clean baseline is reported beside them.
`space-rx resilience-synth <good|faulty> <dir>` writes synthetic logs.

**Evaluation only**: it measures behaviour already logged. It implements no interference
mitigation or authentication technique. Evidence for a review; not a statement that a part is fit
for flight.
