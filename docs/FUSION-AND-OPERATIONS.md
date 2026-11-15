# Fusion and operations (roadmap 0.42 F1, F2, F3, F5, F6)

PRIVATE. Kshana Pro products that turn what Stations, receivers, feeds and time sources report
into an area picture, a trusted time choice and routed alerts. Every output is MODELLED: these
products fuse and decide from their inputs; they add no detection of their own. Kshana is
pre-revenue; nothing here describes a customer deployment.

| Row | What | Kind / feature | Command | Tier |
|---|---|---|---|---|
| F1 | Area console | `pro-area-picture` / `pro-area-console` | `kshana-pro area picture`, `GET /area/console`, `GET /area/picture` | Pro; more than one tenant per console: Enterprise |
| F2 | No emitter positions in the console | hard switch `area::EMITTER_POSITIONS` | (none) | (none) |
| F3 | Multi-source trust | `pro-multi-source-trust` | `kshana-pro trust run`, `trust verify` (free) | Pro |
| F5 | Alert policy engine | `pro-alert-policy` | `kshana-pro alerts route`, `alerts raw` (free) | Pro |
| F6 | Planner Pareto front with cost (B1b) | `pro-planner-front` | `kshana-pro run <front.toml>` | Pro |
| F6 | Station fleet, HA service | `pro-station-fleet`, `pro-service-ha` | unchanged (0.41) | Pro, Enterprise |

## F1: area console

A console file (`kshana-pro-area-console/1`) lists monitored areas and their sources:

* `station` and `receiver` sources: a state per epoch (`t_s,state`, states `nominal`,
  `degraded`, `untrusted`: the open engine's per-station rule, as a Station or a receiver-trust
  run reports it);
* `feed` sources: customer-supplied detections (`t_start_s,t_end_s,kind,confidence` and,
  optionally, an emitter estimate `lat_deg,lon_deg`, which is discarded: F2). A feed needs a
  `provider` and has a `min_confidence` (default 0.5).

Per area the stations and receivers go through the open engine's pre-registered area rule
(`kshana::area_trust`, version 1, 2 (default) or 3's aggregation) unchanged. The fusion around
it:

* a feed detection alone raises the area to `DEGRADED` at most; the event is
  **uncorroborated**; it is **corroborated** when the area's own network is degraded or
  untrusted at the same second;
* **events** are maximal runs of `DEGRADED`/`UNTRUSTED` seconds, with the stations and
  receivers that reported degraded or worse, the overlapping feed detections, the basis
  (`network`, `feed`, `network+feed`) and every state change inside;
* events of different areas that overlap within 60 s form a **wide-area event**.

**Tenancy.** Each area has a tenant (default `default`). `Picture::for_tenant` gives one
tenant's view (its areas, events and its part of wide-area events). A console with more than one
tenant needs an Enterprise licence (`area::multi_tenant_allowed`); the service reads each
tenant's own console at `<data-dir>/area/<tenant>/console.toml`. Full tenant isolation tests
belong to F8 (Enterprise operations).

**Two-site demo** (`examples/area-console/`, written by `generate.py`): Gulf of Finland
approaches (three Stations, one receiver, one feed) and Tartu approach (three Stations, one feed
with emitter estimates). SYNTHETIC: a MODELLED analogue of the published Baltic jamming
descriptions in the threat library; no recording, station or feed exists. Result
(`tests/fusion_operations.rs`, `f1_two_site_demo_*`): three events, one wide-area event linking
both areas' simultaneous jamming (both corroborated by their feeds), one uncorroborated
feed-only event; the receiver's own five-minute untrusted spell is one source of four and does
not raise the area under the v2 rule; two emitter estimates withheld.

**HA.** Two Enterprise service nodes on one data directory serve the same picture; after the
lease holder fails, the other node takes the lease and serves the same bytes
(`f1_service_serves_the_console_under_the_licence_and_survives_failover`).

## F2: detections and areas, never emitter positions

`area::EMITTER_POSITIONS` is an enum with a single value, `Off`. There is no configuration key
(unknown keys are refused), no environment variable and no code path for "on": turning it on
takes a code change, which the roadmap allows only after the open locator successor (B3c, V1)
meets its pre-registered gate on the **blind JammerTest 2027 real-data run**. B3c has met only
its wideband synthetic gate. A feed's emitter estimate is validated as a field and dropped; the
picture, the page and the alerts carry the count withheld and no coordinate (tests
`f2_emitter_positions_are_a_hard_switch_that_is_off` and
`f2_feed_emitter_estimates_are_withheld_from_every_output`). Area geometry is not part of the
console file either.

## F3: multi-source trust

The rule is fixed in `docs/evidence/v4-multi-source-switching/2026-11-15_PREREGISTRATION.md`
(committed alone, 8cf2b77b, before any selector code): exclusion by status (an OSNMA failure
takes the whole GNSS group out until OSNMA has been `ok` for 60 s); qualification by votes of
every other group's best source and of the holdover (agreement within
`4 sqrt(sigma_c^2 + sigma_r^2)`; at least one agreeing vote and no more disagreeing than
agreeing); a protection level `4 sqrt(sigma_c^2 + sigma_ref^2)`; a holdover line through the
last hour of external outputs older than 10 minutes, with fit, class-noise and drift-bound
uncertainty; best-ranked choice with a hold before switching up and an immediate switch away.

Input: a table `t_s,<id>_status,<id>_offset_ns,...` and a configuration (sources with group,
stated sigma and the authenticating flag; the local clock class; the epoch interval). Output:
`trust-decisions.csv` (one row per epoch: choice, state, offset, protection level, holdover
sigma, every source's verdict, the switch reason) and `trust-evidence.json` (counts, the
decisions table's SHA-256, hash-chained switch records). `kshana-pro trust verify` checks the
hash, the chain and that the switch records are exactly the table's changes of choice; it needs
no licence.

**V4 result: met on every data set** (`docs/evidence/v4-multi-source-switching/RESULTS.md`). F3 is
claimed in the scope the pre-registration states: recorded clocks with modelled source errors
and injected faults; no real multi-source recording has been scored.

**Known limitation (found on the demo, for a v2):** a group whose own candidate fails the
cross-check still votes against the others. During a GNSS time push the pushed group's vote and
a holdover fitted partly on the start of the push can together outvote a healthy PTP source,
so the choice moves between PTP and eLoran a few times (`examples/multi-source/`, minutes 21 to
24). Nothing misleading is output (both are healthy), but a v2 rule should drop the vote of a
group that is itself inconsistent, under a new pre-registration.

## F5: alert policy engine

A policy (`kshana-pro-alert-policy/1`) has recipients (each with default I5 outputs) and routes:
match by kind, minimum severity, area and source; notify recipients through named I5 outputs
(webhook, ServiceNow, PagerDuty, Opsgenie, Slack, Teams, syslog, CEF, Kafka, SNMP trap, Splunk
HEC, Elastic, STIX/TAXII, CAP, MQTT, NATS, Prometheus, OpenTelemetry; anything else is refused);
quiet hours in local time with an `allow` severity (held alerts go out as one digest when quiet
hours end); escalation steps after `after_s` without an acknowledgement (per alert key, also for
earlier keys whose window closed); de-duplication per key (`kind`, `area`, `source`) inside a
window, where a worse alert is always sent (`worse`) and the next notification reports how many
repeats were folded. Routes stop at the first match unless `stop = false`.

The engine plans and sends nothing; delivery goes through the open I5 outputs. **Raw alerts stay
free:** `kshana-pro alerts raw` prints every alert unchanged without a licence, `route` without a
licence says so, and every plan accounts for every input alert (notified, folded, held or
unrouted). `alerts::from_picture` and `alerts::from_trust` turn console events and selector
switches into raw alerts (no emitter position). Demo: `examples/alerts/`.

## F6: signed planned items of F10a

* **B1b planner Pareto front with cost** (`pro-planner-front`): candidate costs (capital plus
  running cost per year over `years`), every subset of `m_min` to `m_max` stations (exhaustive up
  to 20 000), objectives from the open planner unchanged, the exact non-dominated front, the best
  network per `m` (equal to the open planner's own exhaustive placement, tested), and the
  cheapest network meeting a target. Demo: `examples/planner-front/` (illustrative sites and
  costs). Note: the open planner's B1a cross-platform reproducibility half (P5 on macOS and
  Windows) is still pending in the open repository; the front inherits that status.
* **Station fleet and HA**: already built (0.41); checked unchanged in the signed tier manifest
  (`pro-station-fleet` Pro, `pro-service-ha` Enterprise); the area console runs under HA.
* **Space tip-and-cue study (M9): not built here.** M9 is part of the M workstream (space RF
  survey, tip-and-cue, target maintenance), which the owner's rule of 14 November 2026 (roadmap
  0.42 section 1a) makes Arksa-exclusive; it belongs in the private server-side crate, not in
  Kshana Pro. Open for the owner: confirm, or re-scope M9 as a ground-only maritime study for Pro.
