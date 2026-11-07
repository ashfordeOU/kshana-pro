# Spectrum coexistence: a whole signal plan and constellation against the open GNSS signals

`kshana-pro coexist` tests one or more candidate signal plans, each a signal design plus a
constellation, against every open-service satellite navigation signal the open Kshana
engine models, in both directions and everywhere on Earth, and gives a verdict against the
compatibility limit the study states. The free `leo-signal` kind answers the same question
for ONE satellite of a design; this study answers it for the whole system: every satellite
of the plan that is in view at once.

Every physical number is read from a run of the open engine (`leo-signal` and
`constellation-design`, through `kshana::api::run_scenario`). The study adds only the
aggregation arithmetic and ranks the plans with the open engine's Pareto routine
(`kshana::mcda::pareto`).

Abbreviations used on this page, in full:

| Abbreviation | Meaning |
|---|---|
| AltBOC | alternative binary offset carrier (the Galileo E5 wideband signal) |
| BPSK | binary phase-shift keying |
| C/A | coarse/acquisition (the GPS L1 C/A signal) |
| C/N0 | carrier-to-noise density ratio, in decibel-hertz (dB-Hz) |
| dB, dBW | decibel; decibels relative to one watt |
| GNSS | global navigation satellite system |
| GPS | Global Positioning System |
| HTML | HyperText Markup Language |
| INR | interference-to-noise ratio |
| ITU-R | International Telecommunication Union, Radiocommunication Sector |
| JSON | JavaScript Object Notation |
| LEO | low Earth orbit |
| MCDA | multi-criteria decision analysis |
| SHA-256 | Secure Hash Algorithm, 256-bit |
| SSC | spectral separation coefficient, in decibels per hertz (dB/Hz) |
| SVG | Scalable Vector Graphics |
| TOML | Tom's Obvious Minimal Language (the study file format) |

## What the study answers, and what it does not

It answers:

- For each candidate plan and each open GNSS signal, the largest loss of that signal's
  C/N0 anywhere on the grid when every satellite of the plan in view is received at the
  plan's maximum power (an upper bound), and the mean-case loss (mean number in view at
  the reference power).
- The reverse: the loss of the plan's own C/N0 from the open GPS and Galileo satellites in
  view.
- A verdict per plan and signal against `max_degradation_db`, with the margin.
- A world map of the upper-bound loss for the plan whose worst loss is largest.
- A Pareto front over (ranging accuracy, worst loss caused, satellite count).
- With `[[other_system]]` (roadmap F8b): other systems already in the band, defined
  like plans. Per plan and victim, their upper-bound interference is added to the
  plan's in every cell, and the study gives the worst cell of the sum, the verdict
  against the limit, and the plan's share of the interference there. The other
  systems are not candidates: they are not ranked and get no verdict of their own
  beyond their own worst loss.

It does not answer, and refuses to be used for:

- Anything about jammers, spoofers, attacks or threats. A study containing a table named
  `jammers`, `jammer`, `spoofer`, `spoofers`, `attack` or `threat`, at any depth, is
  refused with the message that such analysis is outside this pack. The result carries no
  jammer-tolerance or jammer-to-signal field, although the open `leo-signal` result has
  them; they are not copied through.
- Any victim other than the open-service signals the open engine carries a compatibility
  model for: `gps-l1ca`, `galileo-e1`, `gps-l5`, `galileo-e5a`, `galileo-e5b` and
  `galileo-e5-altboc` with engine 0.40 (the list is read from the engine, so a victim
  the engine adds, such as GPS L2C, follows the engine pin). There is no way to define
  another victim; a signal the pinned engine has no compatibility model for is refused
  rather than answered from a guess.
- A coordination filing or a regulatory finding.

## Method

Effective carrier-to-noise density at a victim receiver is

    C/N0,eff = C / (N0 + I),    I = sum over interfering satellites of P_i x kappa

with `P_i` the received power of satellite `i` and `kappa` the SSC of the interfering
spectrum into the victim receiver's band. The loss of C/N0 is `10 log10(1 + I/N0)`. This is
the published method for radionavigation-satellite compatibility (ITU-R Recommendation
M.1831; J. W. Betz, "Binary Offset Carrier Modulations for Radionavigation", NAVIGATION
48(4), 2001; E. D. Kaplan and C. J. Hegarty, *Understanding GPS/GNSS*, 3rd edition, Artech
House 2017, section 9.4).

For each plan:

1. **Signal.** One `leo-signal` run with the study's receiver and victims. Read, per victim:
   the SSC of the plan's signal into the victim (`ssc_leo_into_gnss_db_per_hz`), the SSC of
   the victim into the plan's tracked component (`ssc_gnss_into_leo_db_per_hz`), the run's
   own single-satellite losses in both directions, the GNSS minimum received power, the
   receiver noise density `N0`, the design's received powers and the ranging accuracy.
2. **Constellation.** One `constellation-design` run of the plan's constellation alone over
   the study grid: per cell, the most satellites above the mask at any epoch
   (`grid.max_visible`) and the mean over the epochs (`grid.mean_visible`).
3. **Aggregation, the direction the plan causes.** The single-satellite ratio at the
   reference power is `r = P_ref x kappa / N0`. Per cell and victim, the upper bound is
   `(most in view) x r x P_max/P_ref`, the mean case `(mean in view) x r`, and each loss is
   `10 log10(1 + ratio)`. Before anything is reported, the study rebuilds the open run's own
   single-satellite loss from the SSC it read and stops if the two differ by more than
   1e-9 dB.
4. **Aggregation, the direction the plan receives.** One `constellation-design` run each of
   the open `gps-baseline` and `galileo` presets at the plan's mask gives the GNSS
   satellites in view per cell; each is taken at the GNSS minimum received power the open
   engine states. GPS victims use the GPS preset and Galileo victims the Galileo preset.
5. **Verdict.** Per plan and victim, the worst cell against `max_degradation_db`: at or
   below the limit is compatible, above it is not, and the margin is limit minus worst.
6. **Ranking.** `kshana::mcda::pareto::pareto_front` over (ranging accuracy, worst loss
   caused over the victims, satellite count), all minimised.

**Reference power.** The plan's `received_power_dbw` if given (an input); else the design's
own received power; else, when the design states none (the generic designs), the open
run's reference C/N0 plus its noise density, which is the power the open run itself uses
for that design. **Maximum power**: the plan's `received_power_max_dbw`, else the design's,
else the reference power. Each basis is stated beside the number in the result. A reference
or maximum power of 0 dBW (1 W) or more is refused: no satellite signal reaches a user
antenna that strongly, so such a value is an equivalent isotropically radiated power (EIRP)
or a number in another unit. (The open `leo-signal` kind does not currently return for an
inline design that itself states a received power above about -125 dBW; that is an
open-engine limit, not something this pack can catch before the run.)

**No spectral overlap.** A null SSC in the open result means the bands do not overlap. It is
carried as "no spectral overlap" with a null loss, never as zero, and the verdict says
"compatible" because there is no overlap, not because a loss of zero was computed. A cell
with no satellite in view carries a null loss too, and a constellation never in view gives
a null worst loss with that reason.

**This is a bound.** In the direction the plan causes, every visible satellite is taken at
the maximum received power, with no elevation-dependent power and no receive-antenna
pattern, and the count is the most in view at any epoch. The result and every figure say
so.

## Study file reference

```toml
kind = "pro-spectrum-coexistence"
title = "..."                       # required
max_degradation_db = 0.1            # required: no limit is built in (dB, finite, >= 0)
limit_source = "..."                # required: where the limit comes from
victims = ["gps-l1ca", "gps-l5"]    # required: from the open list above, no repeats

[receiver]                          # optional: passed unchanged to leo-signal
antenna_temp_k = 290.0              # the same keys the open leo-signal and spectrum
noise_figure_db = 2.0               # kinds take
reference_cn0_dbhz = 45.0

[grid]                              # required
grid_step_deg = 5.0                 # cell size, at most 90 deg
duration_s = 86400.0                # time span of every coverage run
step_s = 300.0                      # time step

[[plan]]                            # one or more
label = "..."                       # required, unique
signal = "generic-l"                # a preset signal name of the open engine, or an
                                    # inline table in the leo-signal `[[signals]]` schema
mask_deg = 10.0                     # required, 0 <= mask < 90
received_power_dbw = -150.0         # optional input, overrides the design's
received_power_max_dbw = -145.0     # optional input, overrides the design's
j2 = false                          # optional, passed to constellation-design

[[plan.shell]]                      # constellation-design Walker shell, unchanged
pattern = "delta"
total = 288
planes = 12
phasing = 1
altitude_km = 1000.0
inclination_deg = 55.0

[[plan.satellite]]                  # or explicit element sets, unchanged
altitude_km = 1000.0
inclination_deg = 53.0

[[other_system]]                    # optional, zero or more: systems already in the band
label = "..."                       # the same keys as a [[plan]]; labels are one set
signal = "generic-l"                # with the plans'
mask_deg = 10.0
[[other_system.shell]]
pattern = "delta"
total = 96
planes = 8
phasing = 1
altitude_km = 1000.0
inclination_deg = 55.0
```

With other systems the result adds `other_systems` (each one's signal, constellation and
own worst losses) and, per plan, `with_other_systems`: per victim the worst cell of the
summed interference, its loss, the plan's share there (`plan_share_at_worst`, 0 to 1) and
a verdict, and one combined verdict.

Unknown keys at the top level, in `[grid]` or in a plan are refused. A preset signal name is
looked up by running `leo-signal` over all the open engine's signal presets
(`xona-pulsar`, `iridium-stl`, `starlink-soo`, `centispace`, `generic-c-band`,
`generic-bands`; a test holds this list to the one the engine reports).

## Commands

```bash
kshana-pro coexist <study.toml> --out <dir> --license <token>
kshana-pro run <study.toml> --license <token>      # prints the result JSON
```

A debug build also accepts `--dev` in place of a licence. `coexist` writes:

| File | What it holds |
|---|---|
| `coexistence.result.json` | the full result (below) |
| `coexistence.matrix.svg` | plans against victims, each cell the worst-cell loss with its verdict, both directions |
| `coexistence.map.svg` | the upper-bound loss per grid cell for the plan whose worst loss is largest |
| `coexistence.pareto.svg` | ranging accuracy against the worst loss caused, with the Pareto front |
| `coexistence.report.html` | one self-contained page with all of the above |

The feature is `pro-spectrum-coexistence`; it is refused at the Evaluation tier.

## The result document

`schema_version`, `kind`, `overlay` (`kshana-pro`), `overlay_version`, `engine_version`
(read from the open run), `scenario_hash` (SHA-256 of the canonical JSON form of the parsed
study: keys sorted, compact), `label` (starts with MODELLED), `bound`, `limit`, `victims`,
`grid`, `summary`, `plans[]` (per plan: `signal`, `constellation` with the per-cell counts,
`caused[]` and `received[]` per victim, `worst_loss_caused_db`, `map_loss_db`, and the two
verdicts), `gnss_constellations[]`, `pareto`, `figure_tiers.figures[]` (every aggregate
figure MODELLED), `not_modelled[]`, `members[]` (every open run made: its kind, role and the
SHA-256 of its scenario and of its result) and `units` (for every numeric field, keyed by
result path: `{unit, provenance, note}`, provenance `read`, `input` or `aggregate`). No key
contains `jam`, `js_` or `spoof`; a test walks the document to hold that. Two runs of one
study are byte-identical.

## Worked example

`scenarios/pro-power/coexistence/generic-walker-coexistence.toml`: four plans on the open
engine's generic REPRESENTATIVE designs (no operator's system), illustrative Walker delta
shells at 1000 km and 55 degrees, a 5 degree grid over one day at 300 s steps, a 10 degree
mask, all six victims, and an ILLUSTRATIVE limit of 0.1 dB (not a regulatory, coordination
or protection criterion). The generic designs state no received power, so the reference and
maximum power are the open run's reference C/N0 of 45 dB-Hz plus its noise density of
-201.975 dBW/Hz: -156.975 dBW. Numbers from the run with scenario hash
`479364bde54004a2300562d7468c9accb53b87ac7408982de86d810aadf985d4` (first run on engine 0.28.0
at the development pin, then again with Kshana Pro 0.2.0 on engine 0.29.0, with the same
figures; a debug build, with the engine compiled optimised):

| Plan | Most in view | Worst loss caused (upper bound) | Verdict | Ranging accuracy |
|---|---|---|---|---|
| L band (`generic-l`), 288 satellites | 17 | 0.00544 dB, into the E5 AltBOC signal | compatible, margin 0.0946 dB | 0.1105 m |
| L band (`generic-l`), 144 satellites | 9 | 0.00288 dB, into the E5 AltBOC signal | compatible | 0.1105 m |
| C band (`generic-c-band-leo`), 288 | 17 | no spectral overlap with any victim | compatible (no overlap) | 0.1105 m |
| Wide C band (`generic-c-wide`), 288 | 17 | no spectral overlap with any victim | compatible (no overlap) | 0.0218 m |

For the 288-satellite L-band plan the read SSC into the AltBOC signal is -86.325 dB/Hz, so
one satellite gives an INR of -41.33 dB (a loss of 0.00032 dB); 17 in view at the worst cell
(47.5 S, 177.5 W) give an INR of -29.02 dB and the 0.00544 dB above. Into GPS L5, Galileo
E5a and E5b the SSC is -96.45 dB/Hz and the worst loss 0.00053 dB. The largest mean-case
loss over the cells is 0.00472 dB into AltBOC (at the upper-bound worst cell, with 14.43
satellites in view on average, the mean case is 0.00462 dB). In the other direction, at most 10 GPS or 10 Galileo
satellites are in view; at their minimum powers they lower the L-band plan's C/N0 by at most
0.0091 dB (AltBOC, -152 dBW). The Pareto front is the 144-satellite L-band plan (fewest
satellites) and the wide C-band plan (finest ranging, no overlap).

The C-band plans show a small non-null coefficient in the direction they receive (for
example -126.2 dB/Hz from AltBOC): the open engine does not band-limit the GNSS spectrum in
that direction, so the far sidelobes of the GNSS modulation reach the C band. The study
reports that coefficient as read.

An independent check: the same leo-signal and constellation-design scenarios run with the
separate open-engine binary reproduce every SSC, ranging accuracy and per-cell count of the
result exactly, and the worst losses to 1e-12 dB.

## Limitations

- **An upper bound.** Every visible satellite at the maximum received power; no
  elevation-dependent power; the count is the most in view at any epoch.
- **Open-service signals only**, and only those the open engine models (not GPS L2C).
- **Smooth spectra.** The open engine's power spectral densities have no spreading-code
  lines; the open `leo-signal` kind states the same limit.
- **No receive-antenna pattern** at the victim receiver: the coverage runs give counts in
  view, not the elevation of each satellite, so an elevation-dependent gain cannot be
  applied without changing the bound.
- **The reverse direction uses the GNSS minimum received power** and takes every satellite
  of a preset as transmitting every signal of its system; it is a most-in-view figure, not
  a power bound. Each victim is reported on its own, not summed (Galileo E5a, E5b and
  AltBOC are one transmission).
- **Other systems are other satellite systems only**, entered as `[[other_system]]`
  with a signal design and a constellation, at their maximum powers (an upper bound,
  like the plan's). The most-in-view counts of the systems are added cell by cell,
  even when their maxima fall at different epochs, so the sum is a bound too.
  Terrestrial services, and GNSS systems beyond the open GPS and Galileo presets, are
  not in the sum.
- **GPS L2C arrives with the engine.** The victims are the open engine's own list
  (`kshana::leo_signal::GNSS_VICTIM_NAMES`). The engine 0.40 this build pins has no L2C
  compatibility model; the open engine has added one (MODELLED), so `gps-l2c` becomes a
  victim with the engine release that carries it, with no change here.
- **Not a coordination filing and not a regulatory finding.**

## Tests

`tests/spectrum_coexistence.rs` reads every expected value from a direct open-engine run in
the test: one satellite reproduces the free single-satellite loss for every generic design
(to 1e-9 dB); the Xona Pulsar X5 coefficients are the ones the open document states (test
only, never in the showcase); `10 log10(1 + N r)`; doubling the satellites or adding
3.0103 dB of power adds 3.0103 dB of INR; the verdict flips across the limit; the C-band
design gives "no spectral overlap"; the counts equal the `constellation-design` grids; the
exclusions; determinism; the licence gate; empty, NaN and never-in-view inputs; the result
contract; and that no file of this feature mentions the open engine's isolated workshop
preset.
