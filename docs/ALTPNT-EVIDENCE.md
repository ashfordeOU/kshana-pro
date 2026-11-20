# Alternative-PNT evaluation and civil test-campaign evidence

Backlog E40 B4004 (research 2026-11-20 gaps: 07:G02, 07b:D02, D04, D05, D08, D10).
Module `kshana_pro::altpnt`; command `kshana-pro altpnt`; tests `tests/altpnt_evidence.rs`.

Four evidence products for an alternative-PNT (A-PNT) system someone else built, run by a
tester, a buyer or the supplier on data they already hold. Every result is a signed directory
(`signature.json`, verified offline with `kshana-pro altpnt check` or `kshana-pro dossier
verify`), carries its inputs under `inputs/` with their SHA-256, and is labelled **MODELLED**:
the rules are exercised on synthetic fixtures with known answers, not validated against a real
campaign. A result is test evidence for the data supplied, never a certification, a
qualification or an approval.

| Command | Feature (tier) | What it produces |
|---|---|---|
| `failure-modes <architecture.toml>` | `pro-altpnt-evidence` (Pro) | `failure-modes.json`, `failure-modes.md` |
| `labels <claims.toml>` | `pro-altpnt-evidence` (Pro) | `labels.json`, `labels.md` |
| `campaign <campaign.toml>` | `pro-altpnt-campaign` (Pro) | `campaign.json`, `campaign.md` |
| `time-iface <request.toml>` | `pro-altpnt-evidence` (Pro) | `time-iface.json`, `time-iface.md` |
| `check <dir> --trust <hex>` | free | signature and file check |
| `synth <dir>` | free | synthetic inputs for every command |

Producing commands take `--out <dir> --signer <name> --key-file <file> [--utc <iso>]
[--license <token>] [--dev]`; `failure-modes` also takes `--trust <hex>[,...]` to pin the
signers of the evidence it cites.

## Scope and export control

Evaluation and test evidence only. Nothing in this module computes a position or time
solution, fuses sensors, matches a map (terrain, gravity or magnetic), selects or switches
sources, locates an emitter, steers an antenna or radiates. The campaign reads outputs the
system under test already produced and compares them with truth the tester supplies; the
failure-mode pack reads declarations; the time-interface tests read a measured offset log.

The campaign's civil-releasability screen runs before any file is read. It refuses a campaign
whose end use names a weapon, munition, missile, targeting, fire control or warhead; a dataset
that is not `unclassified`; a release other than `public` or `civil-restricted`; and a dataset
naming a military or restricted signal (PRS, M-code, P(Y), P-code, SAASM). The declarations are
the tester's; the screen does not inspect the data itself.

## Evidence labels (twin versus flight)

Where a result was obtained, weakest first: `twin`, `simulator`, `hil`, `lab-rf`, `field`,
`flight`. A claim, a failure mode or a campaign condition takes the label of its strongest
environment:

| Label | Meaning |
|---|---|
| `VALIDATED` | (claims only) an operational measurement exists and the model (twin or simulator) prediction agrees with it within the claim's relative tolerance (default 0.2) |
| `MEASURED-OPERATIONAL` | measured in the field or in flight |
| `MEASURED-LAB` | measured on the unit under test in HIL or an RF lab, not in operation |
| `MODELLED` | twin or simulator only: an architectural target, not an empirical result |
| `CLAIMED` | no result |

`labels` decides "met" from the least favourable value of the strongest environment (a twin
value is never averaged into a flight result) and reports the model's gap to the operational
mean as `agrees`, `model-optimistic` or `model-pessimistic`.

## Architecture failure modes

The request lists `[[source]]` (id, kind, `depends_on` resources), `[[failure_mode]]` (against
one `source` or one `resource`; `detection`, `mitigation`, `fallback` sources) and `[[test]]`
(failure mode, environment, outcome `pass|fail|inconclusive`, optional `evidence` directory).
Findings, each a fixed rule: `single-point-of-failure` (a resource every source depends on),
`shared-resource-undeclared`, `source-undocumented`, `no-detection`, `no-mitigation`,
`common-cause-fallback` (a fallback the same failure takes out), `no-independent-fallback`,
`evidence-not-verified`, `test-failed`, `untested`. Each mode's status is `TESTED`,
`TEST-FAILED`, `MODELLED-ONLY` or `UNTESTED`.

Cited evidence is verified with `signed_dossier::verify`: `verified` (pinned signer),
`signature-ok-untrusted` (no `--trust` given; counted and marked), or `failed: ...` (not
counted).

## Civil test campaign

Each `[[run]]` names a dataset, a condition, an environment, the system's `output` CSV
(`t_s,lat_deg,lon_deg` and optional `alt_m,hpl_m,alert,time_error_ns`) and the tester's `truth`
CSV (`t_s,lat_deg,lon_deg[,alt_m]`, interpolated linearly at each output time). Scores per run:
availability (truth epochs with an output within `match_window_s`, Wilson 95 % interval),
horizontal error 50th/95th percentile (nearest rank) and maximum (great-circle distance),
vertical 95th percentile, time-error 95th percentile, misleading epochs (error above the stated
protection level) and hazardously misleading epochs (misleading without the alert flag) with a
Wilson 95 % interval on the rate. Runs of one condition are pooled from the strongest
environment only. Each `[[claim]]` (condition, metric `availability|h50_m|h95_m|hmax_m|v95_m|
time95_ns|hmi_rate`, `max` or `min`) is `MET`, `NOT-MET` or `NOT-TESTED`; a rate claim also says
whether the bound of its interval meets the limit.

## Resilient-time civil interface tests

For a resilient-time service's civil output (PTP, NTP, 1PPS; for example the civil interface of
a national resilient-time programme), measured against an independent reference and logged as
`t_s,offset_ns,state[,declared_accuracy_ns]` with state `locked|holdover|freerun|recovering`.
Tests, each `WITHIN-LIMIT`, `EXCEEDS-LIMIT` or `NOT-TESTED` against the limits the service
declares: `locked-accuracy`, `declared-quality` (the offset never exceeds the accuracy the
service declared at that epoch), `holdover-growth` (against the declared profile from each
episode's start), `recovery` (locked and within the locked limit within `max_recovery_s`),
`continuity` (gaps, time running backwards) and `mtie-locked` (MTIE of the longest locked
stretch against the declared mask, `timing::compliance::mtie`).

## Not built here

* B4001 quantum-sensor benchmark protocol and cold-atom error models, and B4003 terrestrial
  SoOP (DVB-T/LTE) clock-offset library: open engine items.
* B4002 perception-fusion and lidar-landmark integrity: hosted service item.
* An open cascade-failover harness that switches sources (07b:D03) and magnetic-anomaly map
  quality assessment (07b:D12): out of scope here; the first becomes navigation software, the
  second is map-matching navigation territory.
