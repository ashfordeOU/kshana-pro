# Remote verdicts

Owner decision, 19 November 2026. Some methods ship in **no** Pro build. They run only on
Ashforde's hosted verdict service, so the method never reaches a customer machine and cannot
be extracted. A licensed customer still gets the result:

```
kshana-pro verdict survey request.json --url https://<service> --out verdict.json
kshana-pro verdict verify verdict.json
```

1. The client sends the method's inputs with the customer's licence token (from `--license`
   or the token activated on this machine). The token goes to `curl` on stdin, never on the
   command line.
2. The service checks the licence (valid, unexpired, not revoked, Pro tier or above), applies
   per-licence limits, runs the method and returns a **signed verdict**: the outcome (numbers
   rounded), its evidence class, and an Ed25519 signature. Intermediate values, traces,
   tuning and full grids are never returned (the full survey grid only to Enterprise).
3. The client accepts the answer only if the signature verifies against a trusted service
   key: `--trust`, else `KSHANA_VERDICT_KEY` at run time, else the key built into the
   release (`KSHANA_VERDICT_KEY` at build time). Several keys may be given, comma separated,
   during a key rotation. A saved verdict can be re-verified any time, offline.

Methods served: `selfcal` (self-calibrating simulator loop), `survey` (integrity-driven ASF
survey planner), `cause` (what caused a vessel or aircraft position episode; below). Endpoint:
`--url` or `KSHANA_VERDICT_URL`; HTTPS only (a loopback `http://` address is allowed for a
local test service).

## `cause`: what caused a position episode

```
kshana-pro verdict cause episode.json --url https://<service> --out verdict.json
```

For marine insurers (a claim that rests only on the AIS track of a spoofed period is weak),
coastal states (enforcement needs an evidence standard), sanctions screening (a ship that
falsified its own AIS versus one that was the victim of interference) and LRIT/ECDIS position
disputes. The request (`episode.json`, times ISO-8601 UTC or Unix seconds):

| Field | Meaning |
|---|---|
| `platform` | `vessel` or `aircraft` |
| `subject_id` | MMSI, IMO or ICAO address |
| `window` | optional `{start_utc, end_utc}` (at most 7 days); default the reported track's extent |
| `reported` | the AIS/ADS-B track `[{t, lat_deg, lon_deg, sog_kn?}]`; a report without a position has the coordinates omitted |
| `independent` | optional `[{source, id, positions, sigma_m?}]`; `source`: `radar`, `coastal_radar`, `sar`, `optical`, `mlat`, `other`, `lrit`, `secondary_gnss`, `sat_ais`, `coastal_ais` (for AIS relays, `receiver_lat_deg`, `receiver_lon_deg` and `range_km` allow a reception-range check) |
| `receiver` | optional onboard GNSS log `{epochs: [{t, fix?, cn0_dbhz[]? or cn0_mean_dbhz?, n_sats?, agc_db?, lat_deg?, lon_deg?, clock_bias_ns?}], baseline_cn0_dbhz?, baseline_agc_db?}` |
| `regional` | optional `kshana-pnt-interference-event/1` reports (`kshana-pro` interference events); an empty list states that monitoring covered the window and saw nothing |

The verdict (`method` `cause`, evidence class MODELLED) carries `classification`
(`external_interference_jamming`, `external_spoofing`, `self_reported_track_falsification`,
`equipment_fault` or `insufficient_evidence`), `confidence` in steps of 0.05 with a band, the
`window` of the episode, `questions` (`reported_track`: false / consistent / undetermined;
`rf_interference`: indicated / not indicated / undetermined; `origin`: external, own ship or
aircraft, equipment, undetermined), the `evidence` findings described at outcome level (what
was found, its strength in words, which causes it supports or weighs against, the sources and
time window) and `limits`. When the data cannot separate the causes the answer is
`insufficient_evidence` with the leading hypothesis, not a guess. It is an engineering
reconstruction of the data supplied, not a legal finding, and does not assess intent. The
method has been checked on synthetic episodes only; it is not validated on real incidents.

The client (`src/remote_verdict.rs`) holds no method code and names no service. The service
lives in a separate private repository and is never linked into this crate (X3); its
identifiers are registered `[core]` terms, so the source guard and the binary check fail if
one ever appears here.

Customers who cannot reach a network use the hardened on-prem build instead, and only under
a signed on-premises contract: the licence must carry the explicit `onprem-contract` grant as
well as being Enterprise and machine-bound (`docs/OFFLINE-PROTECTION.md`).
