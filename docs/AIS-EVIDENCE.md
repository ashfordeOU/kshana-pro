# Maritime AIS integrity evidence

Research 2026-11-20 gaps, epic E07 (Pro parts; backlog item ids to confirm against the build
backlog). The open engine carries the AIS integrity detectors; this overlay adds the paid
evidence on top. Module `src/ais_evidence/`, command `kshana-pro ais` (`src/cli/ais_cmd.rs`),
tests `tests/ais_evidence.rs` (synthetic scenario only).

**Evidence label: MODELLED.** Rules are exercised on a synthetic scenario with known truth;
thresholds are pre-registered in each request and written into each pack; nothing has been
validated against labelled real cases. Packs are engineering reconstructions of the data
supplied, for review: not legal opinions, findings of fact or determinations of intent.
Positions are the vessels' own AIS broadcasts or the user's independent sources: nothing here
locates a radio emitter, and nothing transmits.

## Commands and features

| Command | Feature | Tier | What it does |
|---|---|---|---|
| `ais consistency <request.toml>` | `pro-ais-consistency` | Pro | AIS against independent sources with declared accuracies; signed |
| `ais sanctions <request.toml>` | `pro-ais-sanctions-pack` | Pro | Dark-activity review of named vessels; signed |
| `ais iuu <request.toml>` | `pro-ais-iuu-pack` | Pro | Fishing-activity review against declared zones; signed |
| `ais subsea <request.toml>` | `pro-ais-subsea-pack` | Pro | Cable and pipeline corridor activity and presence around a fault; signed |
| `ais synth <dir>` | none | free | Writes the SYNTHETIC scenario and a request for each command |

Every pack command takes `--out <dir> --signer <name> --key-file <file> [--utc <iso>]` and
writes `report.json`, `report.html`, `inputs/request.toml` and `signature.json`. Verify with
`kshana-pro dossier verify <dir> --trust <key>` (free).

## Inputs

* **Positions** (`ais = "..."`): CSV `timestamp,mmsi,lat,lon[,sog_kn,cog_deg,heading_deg,nav_status,source]`,
  decoded by the user's AIS decoder or supplier. Rows with a bad time, a position out of range
  (including 91/181 "not available") or no MMSI are refused and counted. SOG 102.3 and
  navigational status 15 are read as "not available".
* **Static data** (`static = "..."`, optional): CSV `timestamp,mmsi[,imo,name,callsign,flag,ship_type]`.
* **Independent sources** (consistency only): CSV `timestamp,track_id,lat,lon[,mmsi]`, one file
  per `[[sources]]` entry with `sigma_m` (declared 1-sigma accuracy) and an optional coverage
  polygon.
* **Areas, zones, assets**: polygons and polylines of `[lat, lon]` as the user declares them.
  Kshana bundles no boundaries, no asset routes and no sanctions list. Areas must not cross
  the antimeridian.

## Track engine (`[params]`, written into every pack)

| Parameter | Default | Meaning |
|---|---|---|
| `gap_s` | 10 800 | No report for longer than this is a reception gap |
| `max_speed_kn` | 50 | Implied speed above this between consecutive reports is a jump |
| `encounter_dist_m` | 500 | Encounter: both vessels within this distance... |
| `encounter_max_sog_kn` | 3 | ...both at or below this speed... |
| `encounter_min_s` | 7 200 | ...for at least this long |
| `grid_s` | 600 | Time grid for encounters and dwell |
| `interp_max_s` | 1 800 | Longest span between reports bridged by interpolation |

A **gap** is "no report received": coverage, reception, the supplier or a transponder switched
off all produce one, and the packs say so. **One MMSI on two transmitters** is flagged when at
least two away-and-back alternations occur whose legs are both jumps.

## Cross-source consistency

Per independent report with a declared `mmsi`, the AIS position of that vessel interpolated to
the report time gives a residual against the gate `k·√(σ_ais² + σ_src²) + SOG·τ` (`gate_k`,
`ais_sigma_m`, `time_tolerance_s`): consistent, inconsistent, or AIS absent. Without a declared
`mmsi` the report is associated with the nearest AIS vessel inside the gate, or is a target
without AIS. Where a source declares coverage, an AIS vessel inside it at a scan time with no
independent report within the gate is unconfirmed: what a broadcast position that is not the
vessel's produces, and also what a missed detection produces. Consecutive inconsistent reports
(closer than `episode_join_s`) form episodes. Per vessel: counts, the consistent share with a
Wilson 95 % interval, the largest residual.

## Sanctions review

For the vessels the user names (or all): gaps (flagged when they start or end within
`near_area_m` of a declared area of interest), encounter candidates with any vessel, identity
changes in static data (IMO, name, call sign, flag, ship type), IMO numbers broadcast under
more than one MMSI, jumps and shared-MMSI alternations. Each indicator is labelled REPORTED (as
broadcast) or DERIVED (computed). No sanctions list is bundled, no vessel is screened, and the
pack makes no sanctions determination.

## IUU review

Against declared zones: hours inside, hours inside within the fishing-like speed band
`fishing_band_kn` (default 0.5-5 kn, a stated heuristic), hours inside with the broadcast status
"engaged in fishing", gaps starting or ending within `near_zone_m`, and encounter candidates at
sea with the zone they are near. Not a finding of illegal fishing.

## Subsea assets

Against declared cable or pipeline routes with a corridor half-width `buffer_m`: entries, hours
inside, hours at or below `slow_kn`, reports with status "at anchor" or "moored" at or above
`moving_kn` inside the corridor (an anchor-drag indicator, or a status left unchanged), and gaps
inside. With `[incident]` (`utc`, `lat`, `lon`, `window_h`, `radius_m`): every vessel within the
radius in the window before the fault time, closest first, with its closest approach, time
within the radius, slow time, anchor-status reports and whether a gap covers the fault time.
Presence is not attribution; vessels without AIS are not in the pack.

## Limits

* Gaussian gate on declared accuracies; heavier-tailed sources raise inconsistent counts.
* Nearest-neighbour association can mis-associate in dense traffic.
* Encounters show proximity at low speed, not a transfer; broadcast status is often left unchanged.
* Positions inherit any error or manipulation of the broadcasts.
* No land mask, no port database, no bundled boundaries.
