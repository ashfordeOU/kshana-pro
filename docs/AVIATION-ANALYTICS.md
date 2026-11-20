# Aviation operational analytics

PRIVATE. This page describes Pro methods and belongs to this private repository only.

Research backlog epic E11 (items B1101 to B1104; Kshana research 05, 07, 07b and 12 in the
coordination repository, `records/ops/kshana/research-2026-11-20-gaps/`). Operators, airports
and ANSPs already hold the data these analyses need: ADS-B in the terminal area, flight-data
exports, alert logs and NOTAMs. What they lack is a written method that turns it into a record
of what interference did to approaches, receivers and crews:

* an airport's RNP approaches lost for an hour while no conventional approach was there to fall
  back on, with go-arounds during that hour;
* receivers and clocks that stay wrong after spoofing, and TAWS alerts hours later or on the
  next flight;
* crews rejecting nuisance alerts caused by interference, with nobody counting how often;
* short spoofing bursts and interference areas that no NOTAM announced.

`kshana-pro aviation` answers these with four commands. Code: `src/aviation/`, CLI
`src/cli/aviation_cmd.rs`. Tests: `tests/aviation_analytics.rs` (16 tests) and unit tests in
`src/aviation/mod.rs`. Synthetic inputs: `kshana-pro aviation synth <dir>`.

**Decision support only.** Every result says so: it is not an operational approval, not an
airworthiness or continuing-airworthiness determination, not a means of compliance and not a
substitute for the operator's, the ANSP's or the manufacturer's procedures. Every method is
**MODELLED**: its rules are exercised on synthetic inputs with known truth, not validated
against labelled operational data. Nothing here radiates and no emitter is located: areas are
the 0.5° cells where aircraft were affected, never a source position. Pre-revenue: no
operator, airport or ANSP has used it.

## Licence

| Command | Feature | Tier |
|---|---|---|
| `approach` | `pro-approach-availability` | Pro |
| `residual` | `pro-residual-effects` | Pro |
| `nuisance` | `pro-nuisance-alerts` | Pro |
| `areas` | `pro-unpublished-areas` | Enterprise |
| `check`, `synth` | none | free |

The four features have explicit entries in `license::explicit_tier`, are in
`PRO_COMMAND_FEATURES` and in the signed tier manifest. Each producing command takes
`--out <dir> --signer <name> --key-file <file> [--utc <iso>] [--license <token>]` and writes a
signed directory (signed-dossier format: the result JSON, `summary.txt`, extra files, and every
input under `inputs/` with its SHA-256). `kshana-pro aviation check <dir> --trust <key>` checks
it offline. Floats in results are rounded to 9 significant digits so the signed bytes do not
depend on a platform's last bits.

## `aviation approach` (B1101): approach availability, go-arounds, capacity

Request (TOML): `title`, `[airport]` (`id`, `lat`, `lon`, `elevation_ft`), `[[procedures]]`
(`id`, `kind`, optional `runway`, `faf`, `threshold`), `adsb` (CSV files in the documented
`timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp` format), optional `maps` (open
interference-map GeoJSON), optional `[capacity]`, optional `[oceanic]`, optional `[params]`.

* **Outage windows.** Bins of `bin_s` (300 s) in which at least `min_aircraft` (2) distinct
  aircraft inside `terminal_radius_nm` (30 NM) and below `terminal_max_agl_ft` (10,000 ft)
  were mostly degraded (NIC < 7 or NACp < 8), being at least `min_share` (0.5) of the aircraft
  sampled there. Only aircraft that reported good accuracy somewhere in the data count (the open
  map's equipment baseline guard). Bins closer than `merge_gap_s` (600 s) merge.
* **Availability.** GNSS-dependent kinds (`rnp`, `rnp-ar`, `lpv`, `lnav`, `lnav-vnav`, `lp`,
  `gls`) are unavailable during outage windows; conventional kinds (`ils`, `loc`, `vor`, `ndb`,
  `dme`) are not affected in this model, and their own aids are not assessed. Availability is
  the share of observed minutes (bins with assessable terminal traffic). With `faf` and
  `threshold`, the final segment's exposure on each map day comes from the open engine's route
  exposure (`kshana::interference_map::route`).
* **Fallback.** A GNSS-only airport (no conventional procedure configured) is a stated risk
  factor; outage minutes with a conventional fallback are counted.
* **Go-around exposure.** Inside `approach_radius_nm` (12 NM), an approach is a descent of
  `descent_ft` (1,000 ft) to below `approach_floor_agl_ft` (1,500 ft); a go-around is a climb of
  `go_around_climb_ft` (500 ft) after the lowest point. Approaches count as under degradation
  when the lowest point falls in an outage window or the aircraft itself was degraded in the
  `lookback_s` (600 s) before it. Rates with Wilson 95 % intervals and their ratio. Go-arounds
  are inferred from altitude only: weather, traffic or an unstable approach look the same, so
  the ratio is an association, not a cause.
* **Capacity (modelled).** Lost arrivals = outage hours × (nominal − fallback rate). The
  fallback rate is stated, or assumed nominal when a conventional procedure exists and zero at
  a GNSS-only airport. Holding, delay and diversion costs are not modelled.
* **Oceanic RNP support.** In a named box, the minutes in which an aircraft's broadcast accuracy
  does not support the stated RNP (NACp EPU bound above RNP, or the NIC containment bound,
  largest over versions, above twice RNP; both tables from the open engine), the longest run,
  spoofing-like jumps (≥ 5 km at > 900 kn), and the minutes not supported after a jump. It
  shows what the broadcast supports, not whether a flight met its airspace requirement.

Fixture: a GNSS-only airport, 48 arrivals and 4 departures over four hours, a one-hour outage
within 25 NM, three go-arounds inside it and one outside; the window is found within 10 minutes
of truth, both RNP procedures lose the same minutes, and about 12 arrivals are lost.

## `aviation residual` (B1102): residual effects and clean-or-reset

Request: `title`, `aircraft`, `[[flights]]` (`id`, `csv` in the `forensics fdm` format), optional
`alerts` (CSV `time,flight,alert`), `resets` (CSV `reset_utc[,action]`), `areas`
(`[south, west, north, east]` boxes, e.g. from `aviation areas` or NOTAMs) and `[params]`.

Each flight goes through the FDM detector (`crate::fdm`, W1119 C5). After its exposures:

| Indicator | Rule |
|---|---|
| `clock_offset_after_exposure` | median GNSS UTC − recorder time on clean rows, `settle_s` (600 s) after the last event or on a later flight, differs from the aircraft's pre-exposure reference by more than `clock_tol_s` (1 s) |
| `degradation_after_area_exit` | an event continues more than `exit_persist_s` (120 s) after the aircraft left every area; the position during the event is carried from the last clean fix by inertial ground speed and track, coarsely, for area membership only |
| `degraded_from_flight_start` | a later flight has an event within `start_window_s` (120 s) of its first record, outside every area |
| `alert_without_interference` | a TAWS/EGPWS or clock alert after an exposure, with no detected event within `alert_margin_s` (60 s) |

Outcome (MODELLED, a recommendation only): `RESET RECOMMENDED` (an indicator after the last
exposure and no reset since), `LRU CHECK RECOMMENDED` (an indicator after a recorded reset that
followed the exposure: the effect persisted through it), `NO RESIDUAL EFFECT FOUND`,
`NO EXPOSURE FOUND`, `NOT ASSESSABLE`. The position-against-reference check for one receiver is
`kshana-pro reports reset-check`; the manufacturer's and operator's procedures decide.

Fixture: F1 spoofed with a clock that stays 30 s wrong; F2 (next day) with the clock still wrong
and a TAWS alert with no interference; a receiver power cycle; F3 clean. Without F3 and the
reset: `RESET RECOMMENDED`; with them: `NO RESIDUAL EFFECT FOUND`; with F3 still wrong after the
reset: `LRU CHECK RECOMMENDED`.

## `aviation nuisance` (B1103): nuisance-alert statistics

Request: `title`, `alerts` (CSV `time,subject,route,phase,alert[,disposition]`; maritime column
names `vessel`, `voyage`, `area` are accepted), optional `exposure` (unified events JSON from
`forensics fdm` or `reports episodes`, or CSV `subject,start_utc,end_utc`), optional `hours`
(CSV `route,crew_hours`), `[params]`.

* Each alert is classed by its text: `taws`, `clock`, `gnss`, `bnwas`, `ais` or `other`; the
  disposition column, where present, says `nuisance` (false, spurious, rejected) or `genuine`
  (valid).
* **Interference-attributed**: GNSS-dependent class (not `other`) and an interference interval
  of the same subject covers it within `margin_s` (120 s).
* **Likely nuisance**: stated nuisance, or undispositioned and interference-attributed.
* Per route, phase, class and route/phase: counts, the stated nuisance share among dispositioned
  alerts and the likely-nuisance share (Wilson 95 %); with crew hours, alerts and likely-nuisance
  alerts per crew-hour (Poisson 95 %, Byar) and handling minutes per crew-hour (stated seconds
  per alert by class in `params.handling_s`; not a measurement of workload).
* `training-extract.csv`: the route, phase and class cells with the most likely-nuisance alerts,
  with example alert texts.

Counts and stated shares are computed from the logs and reproduce exactly (the test works them
by hand). The bridge-alarm variant (BNWAS and GNSS alarms on a ship's bridge) uses the same
classes and rules.

## `aviation areas` (B1104): unpublished areas and short bursts

Request: `title`, `adsb` (fleet CSVs), optional `fdm_events` (unified events JSON), `notams`
(ICAO NOTAM text or normalised reports JSON from `reports normalise`), `[[advisories]]`
(`name`, `box` or `center` + `radius_nm`, optional `start_utc`/`end_utc`), `[params]`.

* An aircraft with a good report somewhere in the data is **affected** in a cell during a
  `window_s` (300 s) window when it reported NIC < 7 or NACp < 8 there at or above `min_alt_ft`
  (1,000 ft), a spoofing-like jump (located at the position before the jump; a jump back within
  `merge_gap_s` is located at the position after it, so the false position is never used), or an FDM event
  whose last clean position is in the cell (an event's `source_estimate`, if any, is never read).
* **Event**: windows with at least `min_aircraft` distinct affected aircraft, merged per cell
  when within `merge_gap_s` (600 s); a **short burst** lasts at most `short_burst_max_s` (900 s).
* **Coverage**: `COVERED` when an advisory's area intersects the cell and its validity overlaps
  the event; otherwise `UNPUBLISHED`.
* **Minimum evidence and granularity**: `cell_deg` may not be finer than the open engine's
  pre-registered 0.5°, and `min_aircraft` may not be below its publication minimum (5 distinct
  aircraft); smaller groups are counted (`windows_below_minimum`), never located. Aircraft
  identities are counted, not listed.
* Outputs: `unpublished-areas.json`, `areas.geojson` (event cells) and `alerts.csv` in the alert
  policy engine's raw table (`kshana-pro alerts route` routes it), one alert per unpublished
  event (critical for spoofing or mixed, warning for jamming).

Fixture: eight aircraft spoofed for two minutes in one cell with no advisory (a short-burst
domestic event) is found as an unpublished short burst; six jammed aircraft under a covering
NOTAM are `COVERED`; four aircraft stay below the minimum and are not located.

## Not built

* Avionics availability (RAIM/FDE, SBAS) prediction per procedure: the analyses use the
  broadcast ADS-B integrity as the proxy.
* A simulation of the arrival flow (sequencing, holding, diversions); capacity is a stated-rate
  model.
* An open method note for the bridge-alarm variant in the open engine (kshana): the open repo is
  outside this change.
* Validation on labelled operator, airport or ANSP data: every method stays MODELLED until then.
