# UTM / U-space positioning integrity (`kshana-pro utm`)

Research backlog E18, item B1804. Source: `src/utm/`, `src/cli/utm_cmd.rs`. Tests:
`tests/utm_integrity.rs` (synthetic inputs only).

Four products for drone operators, U-space service providers (USSPs) and the authorities that
review their risk assessments. Every result is a signed directory (`summary.txt`, the JSON result,
the inputs under `inputs/`, the signature), labelled **MODELLED**, and is **decision support**:
never a flight authorisation, an operational authorisation, a SORA approval or a means of
compliance. Interference maps show where aircraft reported degraded navigation; nothing here
locates an emitter.

| Command | Feature | Tier | Output |
|---|---|---|---|
| `utm sora <request.toml>` | `pro-utm-integrity` | Pro | `sora.json`, `sora-input.md` |
| `utm feed <request.toml>` | `pro-ussp-feed` | Enterprise | `feed.json`, `feed.jsonl` |
| `utm rid-check <request.toml>` | `pro-utm-integrity` | Pro | `rid-check.json` |
| `utm attest <request.toml>` | `pro-utm-integrity` | Pro | `attestation.json` |
| `utm check <dir> --trust <key>` | none | free | verification |
| `utm synth <dir>` | none | free | synthetic inputs and requests |

Producing commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]` and a
licence (`--license <token>`, the stored licence, or `--dev` in debug builds).

## Request

```toml
title = "Survey north field"
maps = ["map-2026-06-01.geojson", "map-2026-06-02.geojson"]  # kshana-interference-map/v1, one per day
rid = "rid.csv"              # Remote ID records (rid-check, attest)
telemetry = "flight.csv"     # flight telemetry (attest)

[operation]
id = "SURVEY-1"              # also the Remote ID identity attest looks for
uas_class = "multirotor"     # multirotor | fixed-wing | vtol
speed_m_s = 10.0
height_agl_m = 60.0
waypoints = [[52.10, 5.10], [52.10, 5.188]]
contingency_m = 50.0
ground_risk_buffer_m = 100.0
# [operation.dead_reckoning]  heading_err_deg, scale_err, velocity_bias_m_s,
#                             velocity_noise_m_s, noise_corr_s (override the class terms)

[feed]                       # feed only
area = [52.0, 5.0, 52.2, 5.3]  # south, west, north, east
valid_hours = 24.0

[rid_params]                 # optional; defaults shown
max_speed_m_s = 50.0
max_climb_m_s = 20.0
speed_tol_abs_m_s = 5.0
speed_tol_rel = 0.5
max_operator_m = 15000.0
clone_window_s = 1.0
```

The maps are the open engine's per-day interference maps (`kshana interference-map adsb|ais`),
read with the open engine's own route reader.

## `sora`: GNSS-interference input for the SORA

For each day map the route's length is split by cell state (degraded, not degraded, unassessed,
not observed) with the open engine's `route::exposure`. The result states the days with any
degraded route length and their rate with a Wilson 95 % interval, the mean share not observed,
the longest route length in degraded cells on one day and the time to fly it, and the
dead-reckoning error after it:

```
σ(t)² = (ψ·d)² + (s·d)² + (w·t)² + 2·σ_v²·τ·t        d = v·t
```

Class terms (bands, not any product's datasheet): multirotor (optical flow) ψ 3°, s 3 %, w 0,
σ_v 0.1 m/s, τ 1 s; fixed-wing and VTOL (airspeed and heading) ψ 2°, s 5 %, w 1 m/s, σ_v 0.5 m/s,
τ 10 s. The containment margin is the contingency buffer minus that error. Interference class:
`insufficient-data` (no maps, or half or more of the route not observed), `none-observed`, `high`
(half or more of the days affected, or a negative containment margin), `medium` (a tenth or more),
`low`.

`sora-input.md` fills a fixed template (`utm::SORA_TEMPLATE`): the operation, the per-day exposure
table, the containment paragraph and the inputs it offers to the operator's assessment (GNSS as an
external service the operation relies on; the margin as an input to sizing the contingency volume,
not a sizing rule; unobserved route length is not clear airspace). It is an input to the
operator's own SORA, not a SORA.

## `feed`: USSP integrity feed

For every map cell that overlaps the area: the state on the latest day that has the cell, the
degraded days among the days the cell was observed, and an advisory class: `persistent`
(degraded on half or more of the observed days and on the latest), `recurring` (two or more days),
`single` (one day) or `none`. Each record is valid from the end of the latest map day for
`valid_hours`. `feed.jsonl` is one header line, then one record per line, for a USSP to ingest. This
is a persistence summary of past days; it forecasts nothing.

## `rid-check`: Remote ID position authenticity

From the broadcast records alone (CSV `t_utc, uas_id, lat, lon, alt_m[, speed_m_s, op_lat, op_lon]`,
time ISO-8601 UTC or Unix seconds), per identity: reports out of time order or repeated, pairs whose
reported speed disagrees with the speed between the reports, pairs implying more than the plausible
speed or climb, reports with the operator farther than the limit, and pairs that put one identity in
two places within `clone_window_s`. `INCONSISTENT` for a clone, an implied speed above the limit or a
time-order fault; `SUSPECT` for soft findings; else `CONSISTENT`. These are consistency findings about
the data received, not proof of intent; no receiver measurement is used and no emitter is located.

## `attest`: per-flight positioning-integrity attestation

From the flight's telemetry (CSV `t_utc, lat, lon, fix_type[, eph_m, jamming_state, spoofing_state]`):
the share of epochs without a 3-D fix, the longest stretch without one and the dead-reckoning error
over it at the planned speed, epochs with a receiver jamming or spoofing state of 2 or more, position
steps faster than twice the planned speed plus 10 m/s, the largest reported accuracy, the share of the
flown track in cells degraded on the flight's day map (when that map is given), and the Remote ID
check of the operation's identity (when Remote ID is given). Statement:

* `INTEGRITY-SUSPECT`: a spoofing flag, a position jump, a dead-reckoning error over the longest gap
  beyond the contingency buffer, or an inconsistent Remote ID;
* `DEGRADED`: jamming flags, fix gaps within the buffer, or a track through degraded cells;
* `NOMINAL`: none of these.

The directory is signed with the operator's or the USSP's dossier key and verifies offline with
`kshana-pro dossier verify <dir> --trust <public key>` or `kshana-pro utm check`. A hosted per-flight
verdict (a service-signed attestation) is not part of this release.

## Sources

The SORA is JARUS's Specific Operations Risk Assessment (JARUS guidelines on SORA, versions 2.0 and
2.5); the operational volume, contingency volume and ground-risk buffer, and the objectives on external
services, are the operator's to assess. Remote ID fields follow the broadcast content of ASTM F3411 and
ASD-STAN EN 4709-002 (identity, position, altitude, speed, operator position). The interference maps
are the open engine's `kshana-interference-map/v1` documents.
