# Ship bridge consistency, dead reckoning, drills and PNT dependency audits (research 2026-11-20, epic E08)

Two command families. `kshana-pro bridge` works from what a ship's bridge already records:
a cross-sensor consistency aid, a dead-reckoning error budget, an ECDIS estimated-position
(EP) fallback tester and GNSS-denied drill packs. `kshana-pro dependency` maps what an
operator runs onto the PNT sources it depends on, audits the graph, answers the usual
government questions on GNSS reliance and aggregates signed audits into a register.

Code: `src/bridge/` (`nmea.rs`, `consistency.rs`, `dr.rs`, `ep.rs`, `drills.rs`),
`src/dependency/` (`mod.rs`, `templates.rs`, `register.rs`), CLI `src/cli/bridge_cmd.rs`.
Tests: `tests/bridge.rs`, `tests/dependency_audit.rs` and the unit tests in each module.

**Evidence label: MODELLED.** The bridge rules are exercised on synthetic logs with known
truth (the open engine's training NMEA generator and hand-made logs); they have not been
validated against a labelled set of real bridge recordings. The dependency audit is exact over
the graph as the operator lists it; whether the listing matches the installation is the
operator's statement.

**Decision support, never navigation.** Nothing here is navigation equipment or
type-approved. The consistency aid names a disagreement between sensors and the evidence for
it; it never outputs a position to steer by or an instruction to the crew. The dead reckoning
inside it is a reference for the comparison, not a navigation solution, and it does no
map-matching and no GNSS/inertial fusion. No signal is generated: the drills and EP vectors
are NMEA text for simulators and test installations, marked not for navigation by the
generator. IMO and IEC documents are named only as the criteria a result is measured against.

## Licence tiers

| Command | Feature | Tier |
|---|---|---|
| `bridge dr`, `bridge ep-vectors`, `bridge drills list|show`, `bridge verify` | none | free |
| `bridge check` | `pro-bridge-consistency` | Pro |
| `bridge ep-check` | `pro-ecdis-ep-test` | Pro |
| `bridge drills build|score` | `pro-bridge-drills` | Pro |
| `dependency templates|template`, `dependency verify` | none | free |
| `dependency audit`, `dependency questionnaire` | `pro-dependency-audit` | Pro |
| `dependency register` | `pro-dependency-register` | Enterprise |

Every feature has an explicit entry in `license::explicit_tier` and in the signed tier
manifest (`commercial/tier-manifest.json`).

## 1. Bridge consistency aid (`bridge check`, B0801)

```
kshana-pro bridge check voyage.nmea [--config bridge.toml] [--json]
kshana-pro bridge check voyage.nmea --out result/ --signer "Ship operator" --key-file op.key
```

**Input.** A recorded IEC 61162-1 (NMEA 0183) log, or a capture of IEC 61162-450 datagrams
(TAG blocks give the source `s:` and the time `c:`). Sentences read: `RMC`, `GGA`, `GNS`,
`VTG`, `ZDA` (GNSS), `HDT`, `THS` (gyro or GNSS compass), `VHW`, `VBW` (log, water and bottom
track), `DPT`, `DBT` (echo-sounder), `TTM` (radar tracked targets). A bad checksum is skipped
and counted. Roles follow the talker (GP/GN/GL/GA/GB: GNSS, HE/HN: gyro, VD/VW/VM: log,
SD/SS: echo-sounder, RA: radar; an `HDT` from a GNSS talker is a GNSS compass) and a
`[reader.sources]` table overrides them. Epochs follow the primary GNSS receiver's `RMC`
(or `GGA`/`GNS`) on the bridge cadence (`rate_hz`) or the TAG block time, so the GNSS time is
a measurement to check, not the time base.

**Checks**, each between named sensors and each a ratio of residual to threshold:

| Check | Compares | Default threshold |
|---|---|---|
| `heading` | every pair of heading sources | 3 deg (10 s mean) |
| `velocity` | GNSS speed and course with gyro heading and log | 1 kn, plus 1.5 kn on water track |
| `position` | GNSS position with dead reckoning on gyro and log from the last consistent anchor | 50 m plus 3 sigma of the DR budget (section 2) |
| `gnss-pair` | the primary receiver with each other receiver | 50 m |
| `time` | GNSS time with the bridge cadence or TAG time | rate 0.02 s/s, jump 2 s |
| `depth` | echo-sounder with the depth band of a configured zone at the GNSS position | the band |
| `radar` | radar targets of configured fixed marks with range and bearing predicted from GNSS | 50 m + 1 % of range, 2 deg |

A check fails when its ratio stays above 1 for `persist_s` (5 s). The **consistency score**
per epoch is 100 times the mean of `clamp(2 - ratio, 0, 1)` over the checks that ran. A GNSS
outage (no valid fix) is reported as an outage, not a disagreement: jamming shows as
unavailability, spoofing as disagreement. The DR anchor moves to GNSS only while every check
holds, so a slow pull is measured against an independent reference.

**Alerts and the sensor named.** Consecutive epochs with a failing check form an alert. It
lists the checks in the order they failed (a heading check first is the heading-before-position
warning) and names a sensor by a single-fault rule at the alert's onset: the sensor
quantities common to every failing check, less those cleared by an instantaneous check that
holds at that epoch (heading, velocity, GNSS pair, time, radar; the integrating position check
and the coarse depth band never clear). When more than one candidate remains the alert says
the data cannot separate them and names the GNSS-family source first, as a stated prior (it is
the source exposed to radio interference), not as a finding. Each alert carries neutral
cross-checks the crew may make (radar ranges, visual bearings, sounder against chart, heading
sources against the magnetic compass, GNSS time against the ship's clock).

**Configuration** (`bridge.toml`, all optional):

```toml
[reader]
primary_gnss = "GP"
rate_hz = 1
[reader.sources]
II = "gyro"            # an integrated talker carrying the gyro heading

[thresholds]
heading_deg = 3.0
velocity_kn = 1.0
persist_s = 5

[dr]                   # the DR budget behind the position check (section 2)
gyro_bias_deg = 0.5

[[depth_zone]]
name = "fairway"
polygon = [[57.60, 11.70], [57.60, 11.90], [57.70, 11.90], [57.70, 11.70]]
min_m = 18
max_m = 40

[[radar_mark]]
name = "racon buoy"
target = 7
lat_deg = 57.66
lon_deg = 11.80
```

**Output** (with `--out`, signed with `crate::signed_dossier`): `consistency.json` (format
`kshana-pro-bridge-consistency/1`, every threshold, input counts, availability per check,
outages, alerts), `epochs.csv` (score and ratio per check per epoch), `summary.txt`.

**Acceptance (tests/bridge.rs).** A drag-off from the open engine's training generator is
flagged before a 0.1 NM position alarm would trip;
GNSS is named with the honest "not separable" note. A GNSS compass that departs before the
position is flagged first and named alone (the gyro is cleared by the holding velocity check).
A position step is caught by radar marks, the DR position check and the depth band, and
GNSS position is named. Jamming is an outage with no alert; a time spoof names the GNSS time;
an IEC 61162-450 capture of the same log gives the same alerts; a clean passage raises none.

**Not built here.** The open light version for leisure and fishing apps (backlog: open
engine); DSC distress-position checks (no DSC sentence reader yet); a live feed (the aid reads
recordings; a live tap would reuse the same reader).

## 2. Dead-reckoning quality (`bridge dr`, B0802)

```
kshana-pro bridge dr --speed-kn 12 --depth-m 300 --outage-s 7200 --limit-m 1852 --monte-carlo 2000
```

Along and across the track, for a straight leg at constant speed:

```
along^2(T) = s0^2 + (b_v^2 + (v k_v)^2 + c^2) T^2 + q_v T
cross^2(T) = s0^2 + v^2 (b_h^2 T^2 + d_h^2 T^4 / 4) + c^2 T^2 + q_v T
```

with the initial error `s0`, log bias `b_v` and scale `k_v`, gyro bias `b_h` and drift `d_h`,
unknown current `c` and log white noise `q_v`. The 95 % radius integrates the elliptical
normal exactly; `--monte-carlo` draws the same errors and integrates a DR track second by
second (deterministic seed) to check the closed form. A Doppler log tracks the bottom only to
its depth limit (default 200 m): deeper, the current term enters (`--depth-m`). Output: the
error-growth table, the time the 95 % radius reaches `--limit-m`, the Monte Carlo figures.
Defaults are typical class values, not those of named equipment.

## 3. ECDIS EP fallback tester (`bridge ep-vectors`, `bridge ep-check`, B0802)

`ep-vectors --out dir` writes four vectors, each with `stimulus.nmea` (from the open engine's
training generator), `vector.json` (window, deadlines, checkpoints with the reference EP and
tolerance, the reference's own error against truth), `truth.json` and
`reference-observed.csv` (a table that passes):

* `ep-loss-straight`: GNSS lost for ten minutes on a straight leg;
* `ep-loss-turn`: GNSS lost across an alteration;
* `ep-frozen-position`: the position stops changing while still marked valid;
* `ep-manual-position`: GNSS lost, a manual position entered after five minutes.

Play the stimulus into the ECDIS on a test installation, record `t_s,lat_deg,lon_deg,mode,alarm`
once a second, then `ep-check <vector-dir> --observed ecdis.csv`. Criteria: loss (or frozen
position) indicated within the deadline (10 s; 60 s for the frozen case), EP within tolerance
at every checkpoint (50 m or 2 % of the distance run), the manual position kept and
dead-reckoned from, and back on GNSS after the window. A criterion the table cannot show is
NOT ASSESSED. The deadlines and tolerances are this tester's parameters, written into each
vector, measured against the loss-of-position behaviour IEC 61174 and the IMO ECDIS
performance standard describe; they are not figures quoted from those documents.

## 4. Drill packs (`bridge drills`, B0805)

`drills list` shows the six packs: coastal drag-off, open-sea jamming, port time spoof,
replay delay (meaconing), natural degradation in a fjord (the negative control: a spoofing
report loses marks) and a GNSS-denied readiness checklist (paper charts, sextant and tables,
sight and DR practice, the company procedure, the list of GNSS-dependent equipment).

`drills build <id> --out dir` writes the stimulus, the scenario, the generator's instructor
log (JSON and text), `rubric.csv`, `drill.json` and `aid-reference.txt` (when the consistency
aid saw the evidence: a reference for the instructor, not a model answer). `drills score <id>
--actions actions.csv` scores a crew action log (`t_s,action[,by][,note]`, action = rubric id)
against the rubric: full marks inside the window after the onset, half marks in the late
allowance, `must_not` items scored when absent. The signed directory (`score.json`,
`score.txt`, the rubric and the actions) is a training record for the safety management
system; it records what was observed and does not attest competence. Competence themes are
named as what the drill is measured against (STCW tables of competence, the IMO theme of
navigating with GNSS affected, the company procedure).

## 5. PNT dependency audit (`dependency`, B0803)

```
kshana-pro dependency template ship > vessel.toml     # edit to match the installation
kshana-pro dependency audit vessel.toml --out audit/ --signer "Operator" --key-file op.key
```

**Inventory** (`kshana-pro-pnt-inventory/1`, TOML or JSON): `asset`, `operator`, `sector` (a
template id), optional `required_autonomy_s`, and `[[node]]`s. A node is a source (`gnss`,
`gnss-compass`, `gyro`, `log`, `radar`, `eloran`, `inertial`, `ptp-grandmaster`,
`atomic-clock`, `network-time`, `mobile-network`, `terrestrial-radio`, `manual`, `antenna`,
`power`) or a `system`. Systems list `[[node.needs]]` with `pnt` (position, velocity, heading,
time), `from` (providers in priority order), `fallback` and `cross_check`; any node lists
`depends_on`, `critical`, `holdover_s` (GNSS-disciplined sources) and `isolation_switch`. YAML
is not read (no YAML parser in the dependency graph); TOML and JSON are.

Every GNSS source depends on one implicit node, the GNSS signal in space, so redundant
receivers are redundant against equipment faults, not against a jammer or spoofer in range.

**Audit.** GNSS denied (each critical function: independent, carries on degraded on a
fallback, or stops; at the onset and after the holdover of GNSS-disciplined sources runs out,
with the time it changes); GNSS spoofed (critical functions on GNSS-derived data with no
independent cross-check: silently wrong); single node loss for every node (single points of
failure); isolation switches; required autonomy; template coverage. Status: `L1` (a critical
function stops without GNSS, or a single point of failure), `L2` (none stops, some degraded or
silent under spoofing), `L3` (none of those). Output, signed: `audit.json`, `findings.csv`,
`graph.graphml`, `graph.json`, `inventory.txt`, `summary.txt`.

**Sector templates** (`dependency templates`): ship (structured after IACS UR E26, UK MCA
MGN 719 and IMO MSC.1/Circ.1575), port, aircraft architecture, water, pipeline, emergency
calls, TETRA, rescue. A template lists the functions a sector usually has with the PNT kinds
they need; the audit reports those the inventory leaves out. Findings are measured against the
template; they never say an operator meets a rule.

## 6. Questionnaire kit and national register (`dependency questionnaire`, `register`, B0804)

`questionnaire <audit-dir>` answers seven standard questions on GNSS reliance (which functions
depend on GNSS, how long they last without it, single points of failure, how spoofing would be
noticed, isolation, template coverage, past events) from a signed audit, citing its hash; a
question the inventory cannot answer says so.

`register <dir>,<dir>,... --pseudonym-key-file k` verifies each signed audit (pin signers with
`--trust`) and keeps per operator only the status under a keyed pseudonym (HMAC-SHA256 of
operator and asset; the key stays with the register keeper), with the audit hash and signer
key id. Per sector it aggregates the level distribution, totals and the minimum and median
autonomy, and withholds any sector with fewer operators than `--min-group` (default 3). No
inventory, node or system name enters the register. Output, signed: `register.json`,
`sectors.csv`.

## Not built in this epic

* **B0806** (departure go/no-go with verified zones): depends on the verified zone layer of
  B0901, which is not built.
* **B0807** (survey and construction positioning QC): depends on B1503; not started.
* The open light consistency check for leisure apps belongs in the open engine.
