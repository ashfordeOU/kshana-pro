# ADS-B phantom-target plausibility evidence

Research 2026-11-20 gaps, B1002 (Pro part). Feature `pro-adsb-phantom` (Pro). Command
`kshana-pro adsb-phantom`.

The open engine's `adsb-indicators` gives per-cell indicators beyond NIC (exposed aircraft,
update-rate collapse, track inconsistency, low-accuracy share). This command works per target:
it reads the same ADS-B CSV and asks, for every address, whether its reports are physically
plausible, and why not. The output is evidence for an ANSP's or an ADS-B In user's review.
It is labelled MODELLED.

**It never locates a transmitter.** There is no time difference of arrival, no
multilateration of an emitter and no direction finding. Receiver positions are used only to
ask whether a declared station could have heard a reported position.

## Inputs

- The ADS-B CSV of the interference map: `timestamp,aircraft_id,lat,lon,alt_baro_ft` with
  `nic`/`nacp`/`source_type` as there. Two columns are optional: `alt_geom_ft` and
  `receiver_id`. Rows whose `source_type` is not ADS-B are excluded, as in the open map.
- `receivers.toml`: `[[receiver]]` entries with `id`, `lat`, `lon`, `antenna_m` and `range_nm`,
  as the user declares them.
- Optional: `--reference <tracks.csv>` (`timestamp,lat,lon`), an independent surveillance
  source the user supplies, such as radar or multilateration output. This is a data comparison
  only; clip it to the area that source covers.
- Optional: `--params <params.toml>` to override any threshold. Every threshold is written into
  the report.

## Reasons and weights

Each reason has a weight. They combine as a noisy-or, `score = 1 − Π(1 − w)`, and a target with
`score ≥ 0.5` is marked for review.

| Reason | Rule (defaults) | Weight |
|---|---|---|
| `physics` | median implied speed over pairs ≥ 5 s apart above 750 kt, or vertical rate above 20 000 ft/min | 0.7 |
| `teleport` | a jump longer than 750 kt × Δt + 2 km | 0.55 |
| `two-places` | ≥ 4 out-and-back jumps (the reports either side agree), jumps ≥ 20 % of reports | 0.8 |
| `no-history` | fewer than 3 reports | 0.15 |
| `bad-address` | not a 24-bit hex address | 0.3 |
| `unhearable` | ≥ 2 reports (or half of them) beyond the reach of the named receiver, or of every declared receiver. Reach = min(declared range, 4/3-Earth radio horizon 4.124(√h_rx + √h_tgt) km), +10 % | 0.8 |
| `altitude` / `altitude-gross` | median \|geometric − barometric\| above 3 000 ft / above 6 000 ft | 0.3 / 0.6 |
| `no-entry` | first heard above 10 000 ft inside 70 % of every reach, more than 120 s after the data start | 0.15 |
| `cluster` | ≥ 3 such appearances within 120 s and 30 km of each other | 0.5 |
| `no-independent-track` | with a reference: under 30 % of the target's reports (≥ 5) have a reference point within 10 s and 3 km | 0.6 |

## Acceptance on the synthetic scene

`adsb-phantom synth <dir>` writes a SYNTHETIC scene. It has two declared receivers and 45
genuine aircraft: 40 cross coverage from outside, and 5 depart and climb. The genuine traffic
has 5 % dropouts, ±300 ft geometric-barometric offsets and a noisy reference. It also has nine
injected addresses: a ghost cluster of four, a teleporting address, an address in two places,
an unhearable target 420 km out, an 8 000 ft altitude mismatch and a Mach-4 track.

In `tests/adsb_phantom.rs`, all nine injected addresses are flagged with the expected reason.
**0 of the 45 genuine aircraft** are flagged, with or without the reference. That rate is for
this synthetic scene. On real traffic the rate depends on the receivers declared and the
thresholds, and has not been measured.

## Privacy

`--pseudonymise <salt>` replaces each address with a salted hash in the report. The pack then
keeps the SHA-256 of the input CSV in place of the CSV itself.

## Not built

- Any estimate of where a phantom's transmitter is.
- Reading raw Mode S, Beast or readsb trace formats directly. The open map's CSV is the input.
- A false-flag rate on recorded real traffic: none is bundled, and the rate above is synthetic.
