# Exposure, route risk and briefings

`kshana-pro briefing <command>` (module `src/briefings/`, research backlog E09, items B0901 to
B0905). It keeps interference exposure as history along the routes people actually travel and
reads that history back as briefings. Each briefing states its confidence and its data window.

Every figure is **MODELLED**. It is computed from the tracks, events and interference-map
layers supplied, under the method stated in each output, and has not been checked against
independent ground truth. Briefings and alerts are decision support. They are never operational
instructions, navigation warnings, NOTAMs, routing orders or forecasts.

## Commands and licence features

| Command | What it writes | Feature | Tier |
|---|---|---|---|
| `transits <req>` | Per-transit exposure records, listed-area records, fleet view (`transit-history.json`, CSVs, `fleet.html`) | `pro-route-exposure` | Pro |
| `brief <req>` | Area and route briefing with confidence, data window and provenance (`briefing.json`, `.html`, `.pdf`) | `pro-route-exposure` | Pro |
| `tails <req>` | Per-tail exposure ledger over a period (90 days by default), by altitude band | `pro-aviation-exposure` | Pro |
| `lowalt <req>` | Low-altitude cell map of degraded ADS-B integrity (no identifiers) | `pro-aviation-exposure` | Pro |
| `occ <req>` (alias `preflight`) | Flight briefings and the OCC alert list, with data latency | `pro-aviation-exposure` | Pro |
| `regional <req>` | Versioned regional dataset; rebuilds byte-identically; signing optional | `pro-regional-datasets` | Pro |
| `road <req>` | Corridor exposure index and heat map under the privacy rule | `pro-road-exposure` | Pro |
| `spillover <req>` | Footprint of declared test areas, affected users, agreement with layers | `pro-spillover-footprint` | Pro |
| `notam <file>` | NOTAM text to geometry with harmonised start/end criteria | free | - |
| `areas` | Built-in chokepoints and regions as an area file | free | - |
| `verify <dir> --trust <hex>` | Offline signature check | free | - |

Producing commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]`. Their
output is signed with `crate::signed_dossier`.

## Inputs

- **Layers**: the open engine's per-day maps (`kshana-interference-map/v1`, from
  `kshana interference-map adsb|ais`). They are read with the open engine's own
  `route::load_map`. A file or a directory of `.geojson` files can be named. A cell missing from
  a map was not observed, and is never read as clear. A layer finer than 0.5 degrees is refused.
  The open engine's `route::exposure` gives the route shares.
  `scripts/exposure/build-regional-inputs.sh` builds layers from raw data the user fetched.
- **Events**: unified interference events (`kshana-pnt-interference-event/1`, `crate::ievents`),
  validated on load.
- **Tracks**: the documented AIS (`vessel_id`) and ADS-B (`aircraft_id`, `alt_baro_ft`, `nic`)
  CSVs (`crate::forensics::inputs::parse_tracks`).
- **Areas**: built-in boxes on a 0.5 degree raster (chokepoints: Hormuz, Bab el-Mandeb, Dover,
  Bosphorus, Suez, Gibraltar, Singapore, Panama, Danish straits; regions: Baltic, the Gulf, Red
  Sea off Jeddah, north-western Black Sea, eastern Mediterranean, Caribbean, Canary Islands,
  India north-western border, Delhi terminal area, Korea west, Korea DMZ corridor, Finnmark).
  These boxes are for accounting only. They are **not** legal, listed or charted boundaries.
  Market listed areas are not shipped because of their owners' terms. Users supply them in an
  area file with `kind = "listed"` and a `source`, and every listed-area record names that
  source.

## Methods

- **Transits (B0901).** Each vessel's track is split into transits at gaps longer than `gap_h`
  (6 h by default). Each report holds until the next one, for at most 30 minutes. That time is
  attributed to the named areas, to the layer state of that day, and to every active event
  within `reach_km` plus the event's radius. Exposure time is the time spent in a degraded cell
  or within reach of an event. Record ids are SHA-256 hashes of the content, with floats rounded
  to 9 significant digits, so the same input always gives the same id.
- **Briefings (B0901).** For each area, the briefing reports:
  - events in the window, by kind, and how many were corroborated;
  - observed days and degraded days;
  - the share of observed cell-days that were degraded;
  - persistent cells (cells degraded on at least half of their observed days).

  For a route, it reports the open engine's route exposure for each layer day, plus the
  historic-event exposure of `exposure::voyage`.
- **Confidence** (`CONFIDENCE_METHOD`):
  - **insufficient**: no observed day and no event in the window;
  - **high**: coverage of at least 70 %, data on at least 80 % of the window's days, and at
    least half of the events corroborated;
  - **medium**: coverage of at least 40 % and data on at least half of the days;
  - **low**: anything else.

  A low-altitude flight with no low-altitude observations is lowered by one level.
- **Tail ledger (B0902).** Reports are grouped by tail, with aliases merging ICAO address and
  registration. Flights are split at 30-minute gaps. The ledger records exposure minutes by band
  (low below 2,000 ft, mid up to 10,000 ft, high above), the minutes in which the aircraft's own
  NIC was below 7, and the events encountered. A tail is flagged `review` above the configured
  thresholds. It is a planning aid for maintenance. It complements the report-based
  `reports ledger`.
- **Low-altitude map (B0902).** Cells (at least 0.5 degrees) are split into bands below and
  above the ceiling. A cell is published only when at least 5 distinct aircraft were seen in it.
  The map contains no identifiers.
- **OCC alerts (B0902).** These use only what was known at `as_of`: layers whose day had ended
  and events that had started. The level is `advisory` when there was past degradation or an
  event on or near the route, and `information` otherwise. Data latency is `as_of` minus the
  latest observation used. Open observatory layers are published at least 24 h after their day
  ends.
- **Regional datasets (B0903).** For each region and cell, the dataset gives the state on every
  layer day and a class: persistent, recurrent, episodic or none. Pattern separation works as
  follows:
  - each day, degraded cells that touch are grouped into clusters (`observatory::cluster`);
  - each cluster is linked to the earlier pattern it shares the most cells with, within
    `link_gap_days`.

  Sources that degrade the same cells on the same days stay one pattern. The airport pattern
  library lists the patterns and cells within a radius of each airport. A dataset has no
  generation time and lists its files' SHA-256 hashes in `MANIFEST.sha256`. The same request
  and inputs always rebuild the same bytes.
- **Road index (B0904).** Each corridor is cut into segments of `segment_km`. Each detection
  counts for the nearest segment within the corridor's half-width. The rate is distinct
  device-days per 100 km per day, and the index is `100 × (1 − exp(−rate / reference_rate))`.
  The heat map uses cells of at least 0.5 degrees. **Privacy rule (V22):**
  - device ids are counted in memory and never written;
  - a segment or cell is withheld, without counts, below `min_distinct` devices (at least 5);
  - times are kept only to the date and the hour of day.

  Patterns that point to an individual are for the competent authority to act on, not for
  publication.
- **Spillover (B0905).**
  - `notam` reads the Q) circle, the E) polygon or circle, B)/C) with EST and PERM, the D)
    schedule (`HHMM-HHMM`, `DAILY HHMM-HHMM`, `H24`) and F)/G). It applies the harmonised
    criteria in `HARMONISED_CRITERIA` to every NOTAM.
  - `spillover` starts from the **declared** area. Its ranges come from the radio horizon,
    `4.12 × (√h_source + √h_receiver)` km on a 4/3 Earth. The inner ring uses the lower assumed
    source height and the outer ring the higher one, both capped at `max_range_km`. The earth
    is treated as smooth (no terrain).
  - Affected users are the track reports in footprint cells, at their altitude, during active
    windows.
  - Agreement compares the footprint at the agreement altitude with the degraded layer cells
    within twice the outer range.

## Export-control boundary

Nothing here estimates where an emitter is. The spillover model starts from the area an
authority declared, never from observations. Every map, footprint and area output is either a
named or declared area or a grid of cells of at least 0.5 degrees, the open engine's
publication granularity. Finer requests are refused.

## Not done (and why)

- **Agreement for 3 public events (B0905 acceptance).** Not run here. It needs public ADS-B
  data for the event days and the NOTAMs, which cannot be fetched in this environment. The code
  path is tested on synthetic layers.
- **Terrain** in the footprint. It is not modelled (smooth Earth), so terrain shielding can only
  make the footprint smaller than stated.
- **Real regional releases (B0903).** The code, the build script and the method note are
  shipped. No real data is shipped, and a release must check the data licence first.
- **Stop-buzzer real-time feed (B0905).** The affected-user list works on replayed or
  post-event tracks. There is no live feed.
- **Briefing latency.** The latency of each alert's data is stated. The time taken to generate
  a briefing has not been benchmarked.
