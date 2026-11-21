# Timing assurance (roadmap 0.42, family T)

Holdover and time to breach for every clock in a mixed-vendor fleet (T1), the fleet view with
per-clock evidence history (T2), and signed audit reports mapped to MiFID II RTS 25
traceability and to the NIST IR 8323 PNT profile (T3). Code: `src/timing/`; kind
`pro-timing-holdover`; features `pro-timing-fleet` and `pro-timing-audit`; all at the `pro`
edition in the signed tier manifest and the `pro` licence tier.

**Evidence status.** The first prediction method was pre-registered as evidence row V5 and
scored on recorded clocks: **V5 missed** (`docs/evidence/v5-holdover-time-to-breach/RESULTS.md`).
Its successor (a Kalman-filter handover state, used by the product) was pre-registered as V5b
and scored on IGS data V5 did not use: **V5b missed too** (`V5b_RESULTS.md`: all bars passed on
synthetic clocks; on real rubidium clocks the coverage was 0.894 against 0.90 and the
false-alarm bar missed). The time to breach is therefore **not a validated claim**, and every
result says so in its label.

**V5c (16 November 2026).** A third predictor, `kshana-pro/holdover/v3` (V5b's handover with the
GPS orbit-period harmonics removed and carried into the prediction, and a flicker-frequency
floor), was pre-registered and **passed every bar** on new IGS clocks (D-R3, 2025-10-05 to 18) and
a new synthetic fleet (`V5c_RESULTS.md`). The product's fleet assessment still runs the V5b
predictor, and its results keep the "not validated" label until it is switched to the V5c
predictor in a release; only then may its time to breach be presented as validated (T1).

## T1: a fleet file

```toml
kind = "pro-timing-holdover"
title = "Trading floor clocks"
fleet = "ldn"
window_h = 48      # training window (default 48)
horizon_h = 48     # prediction horizon (default 48)

[[limits]]                       # values the operator sets; no limit is built in
name = "RTS 25 HFT style (user-set)"
value_s = 100e-6
[[limits]]
name = "PRTC-A style (user-set)"
value_s = 1.1e-6

[[clocks]]
id = "gm-1"                      # letters, digits, - _ .
site = "LD4"
vendor = "Vendor A"
model = "GM-R"
role = "grandmaster"
[clocks.source]                  # the open engine's clock-log input, unchanged
format = "ticc"                  # rinex-clk timelab ticc ptp4l phc2sys ts2phc chrony-tracking
path = "logs/gm-1.ticc"          # chrony-statistics ntp-loopstats ntp-peerstats wr-stat cggtts pps-assert
channel = "chA"

[[clocks]]
id = "bc-2"
class = "ocxo"                   # prior for a servo record: tcxo ocxo rafs csac uso dsac
# adev_prior = [[1, 5e-12], [100, 2e-12], [10000, 3e-12]]   # or datasheet points (wins)
limits = ["RTS 25 HFT style (user-set)"]                    # optional subset
[clocks.source]
format = "ptp4l"
path = "logs/bc-2.ptp4l.log"
```

Every log is read by the open engine's readers (`kshana::clock_log`); Pro re-implements no
reader. Gaps default to `longest` (the longest gap-free run). `holdover_start_s` moves the
handover from the record's end to a chosen time.

**Two kinds of record.**

* *Free-running* (TICC, TimeLab, RINEX clock, CGGTTS by default): the record shows the
  oscillator against a better reference. The model is fitted to it (the V5b predictor): the
  drift from a quadratic over the window, the noise (white phase, white frequency, random-walk
  frequency) from the Allan variance of the window with the drift term held fixed, and the
  handover state with its full covariance from the open engine's three-state clock Kalman
  filter (`kshana::clock_state::ClockState3`) run over the window.
* *Disciplined* (linuxptp, chrony, ntpd, White Rabbit, PPS by default): the record is the offset
  left under servo control and does not show the oscillator's free-running noise. The noise
  comes from the stated `class` or `adev_prior`, and the record gives the present offset and
  its spread. A servo record without a prior is refused, with that reason.

**Outputs per clock:** the model, the predicted holdover error (mean and 1-sigma) at 10 min,
1 h, 4 h, 12 h, 24 h and 48 h, and per limit the **warning time** (breach probability first
reaches 5 %) and the **median time** (50 %). Status: `act` (a warning within 4 h), `watch`,
`ok` (no warning within the horizon), `breached` (a servo record's present offset already
beyond a limit). The fleet is ordered by earliest warning.

```sh
kshana-pro timing assess fleet.toml --evidence evidence/ --json fleet.json   # appends one entry per clock
kshana-pro run fleet.toml                                                    # same, through the dispatcher
```

## T2: fleet view and evidence history

* **Evidence history:** `--evidence <dir>` appends `<dir>/<clock>.jsonl`, one entry per
  assessment with the record's SHA-256, the model, the breach times and the status, each entry
  carrying the SHA-256 of the one before. `kshana-pro timing history verify <dir>` (no licence)
  reports a changed, dropped or reordered entry; appending to a broken history is refused.
* **Station console page:** the Pro service serves `GET /timing/fleet` (HTML) and
  `GET /timing/metrics` (Prometheus) for the tenant's fleet file
  `<data-dir>/timing/<tenant>/fleet.toml` (log paths relative to it) and its history in
  `<data-dir>/timing/<tenant>/evidence/`. A request only reads; schedule `timing assess
  --evidence` to add history. `kshana-pro timing console fleet.toml --evidence <dir> --out
  page.html` writes the same page.
* **Grafana:** `deploy/grafana/kshana-pro-timing-fleet.json` (Prometheus data source; scrape
  `/timing/metrics`, or write `kshana-pro timing metrics fleet.toml --out x.prom` for the node
  exporter's text-file collector). Panels: counts by status, earliest warning, the time-to-breach
  table, status timeline, 95 % bound at 1 h, offsets, inventory.

**Demo on recorded data:** `examples/timing-fleet/` (eight clocks, seven source formats). Two
are REAL public data: IGS GPS satellite rubidium clocks G03 and G25 standing in for site
references. The others are SYNTHETIC logs written by `examples/timing-fleet/generate.py`.

## T3: audit export

```sh
kshana-pro dossier keygen > signer.key          # once; publish the public key to recipients
kshana-pro timing audit fleet.toml --profile rts25-traceability \
    --evidence evidence/ --operator operator.toml \
    --signer "Example operator" --key-file signer.key --out report/
kshana-pro dossier verify report/ --trust <public key>   # free edition, offline
```

Profiles (`report-templates/timing/`): `rts25-traceability` (Commission Delegated Regulation
(EU) 2017/574: Article 1 reference time, Articles 2 and 3 with the Annex, Article 4
traceability, plus the operator's continuity evidence) and `nist-ir-8323-profile` (CSF 1.1
subcategories as the profile applies them: ID.AM-1, ID.RA-1, DE.CM-1, DE.AE-5, DE.AE-3,
RC.RP-1). Each item is **mapped to** a provision and lists the evidence blocks the report
carries; the operator's statements (UTC source, traceability chain, timestamp point, yearly
review) come from `--operator`:

```toml
organisation = "Example operator"
utc_source = "UTC(k) via GNSS-disciplined grandmasters"
traceability_chain = "..."
timestamp_point = "..."
review = "2026-10-01"
activity = "high-frequency"     # kshana::rts25 activity names
```

The RTS 25 divergence block uses the open engine's `kshana::rts25::assess` on servo records
(free-running records are against a reference, so they are marked "not assessed"). The report
never says compliant, conformant or certified (a test scans for it). The directory
(`report.html`, `report.json`, `fleet-assessment.json`, `history/`, `template.json`,
`README.txt`) is signed as a whole (`signature.json`, Ed25519). Sample:
`report-templates/samples/timing-audit-rts25/`, signed with a published demo key
(`report-templates/samples/timing-audit-rts25.demo-public-key.txt`).

## T4: timing scorecards under spoofing scenarios

`kshana-pro timing scorecard <request.toml> --out <dir> --signer <name> --key-file <key>` (kind
`pro-timing-scorecard`, `src/timing/scorecard.rs`). A request names a T1 fleet file, the
grandmasters, the threat library timing scenarios (E2 `[timing]`: time push, time step,
denial), optional site pairs for a cross-check and the `[[network]]` links behind each
grandmaster, and a detector stated before scoring (`k_sigma` 3, `floor_s` 20 ns, `cross_check`,
`full_fraction` 0.1). For each grandmaster and scenario: detection when the counterfeit offset
leaves the T1 holdover envelope (`k sigma(u) + |mean(u)| + floor`), or exceeds the pair's
combined residual by the same rule where the event reaches one receiver; a denial is seen at
once. Each clock of the network gets the error delivered before detection (plus its own servo
offset), per limit breached-before-detection and holdover warning before the threat ends, and
the levels of the E3 scoring. Public sample: `examples/timing-scorecard/request.toml`,
pre-registered (`docs/evidence/t4-timing-scorecard/`), all three claim bars met; signed sample
`report-templates/samples/timing-scorecard-public/`. It inherits T1's unvalidated holdover model.

### T4 recovery metric: timing modules after an attack (research backlog B1304, E13)

`kshana-pro timing recovery <request.toml> --out <dir> --signer <name> --key-file <file>`
(licence: `pro-timing-scorecard`; `src/timing/recovery.rs`; method
`kshana-pro/timing-recovery/v1`). Unlike the modelled T4 scorecard above, it reads **recorded**
time-error logs of one or more modules' 1PPS against an independent reference (CSV
`t_s,te_ns[,state]`, or TICC, TimeLab and RINEX clock through the open engine's `clock_log`)
and a stated `[attack]` window. Per module: baseline (median TE before onset), the largest
deviation during the attack, the **recovery time** (end of attack to the first sample from
which every sample for `hold_s` stays within `recovered_within_ns` of baseline; `not observed`
when the log ends too early), the **residual offset** after recovery, and holdover behaviour
when the log carries states (entry latency, exit time and the phase step across the exit; without
states, the largest step after the attack). Bars (defaults 100 ns, 60 s hold, 600 s recovery,
50 ns residual, 100 ns step) are hashed into the report. Several modules or configurations of
one site are ranked; with `ensemble_median = true` and three or more, the per-sample median of
the modules is scored too, so a multi-receiver hardening claim is checked on the logs.
`kshana-pro timing recovery-synth <dir>` writes the three fixtures of the tests (fast slew,
slow slew, holdover with a 250 ns exit step). Evidence: **MODELLED** (closed-form fixtures in
`tests/receiver_flag_scorecards.rs`; no real module has been logged).

## C4: timing evidence packs for regulation (W1119)

`kshana-pro timing compliance <framework> inputs.toml --out <dir> --signer <name> --key-file <key>`
(Pro, feature `pro-timing-audit`) writes one signed annual pack. Each pack is a table of
requirement, evidence and verdict. `kshana-pro timing compliance list` names the frameworks:

| Framework | Source | Limits |
|---|---|---|
| `mifid2-rts25` | Delegated Regulation (EU) 2017/574, Articles 1 to 4 and the Annex | Annex divergence and granularity by activity, as `kshana::rts25` transcribes and pins them |
| `dora-rts-2024-1774` | Delegated Regulation (EU) 2024/1774, Article 12 | none in the text per the research; the operator's tolerance, marked "to confirm with the regulation text" |
| `eu-cer` | Directive (EU) 2022/2557 | no named GNSS rule found; article numbers to confirm |
| `india-ist-draft` | draft Legal Metrology (IST) Rules, 2025 | a draft as found; every provision to confirm |

The inputs file names these items:

- the clock's time-error record against its local reference (`[clock]`, any clock-log format);
- BIPM Circular T section 1 and the laboratory code (`[circular_t]`);
- an open-engine `holdover-test` result (`[holdover]`);
- a spoof test: a T4 timing scorecard or a PICS scorecard (`[spoof]`);
- a T1 fleet assessment (`[fleet]`);
- the operator's statements (`[operator]`).

For RTS 25, the bound on the divergence from UTC is max |offset from UTC(k)| + max |UTC − UTC(k)|
+ 2u from Circular T, compared with the Annex limit.

Every requirement gets one of five verdicts:

- `supported by the evidence`;
- `not supported by the evidence`;
- `insufficient evidence`;
- `operator statement (not verified)`;
- `to be completed by the operator`.

A pack never states that a requirement is met, and the T3 forbidden-words rule is tested on
every framework. Every input file is copied under `inputs/` and hashed before the pack is
signed. Verify a pack with `kshana-pro dossier verify` (free).

Evidence label: MODELLED. The packs are tested on synthetic inputs (`tests/timing_compliance.rs`),
and no operator's records have been packed. Kshana's reading of what DORA "reliable", the CER
duties and the India draft ask is marked as such, to be reviewed by the operator.

## Not done here

* PTP management (`pmc`), SNMP and NTP mode-6 status snapshots: the open engine reads them in its
  `connect` commands on its main branch, which the engine release this crate pins (0.41.0) does
  not carry; they will add status context, not phase records, when the pin moves.
* The product's holdover prediction (the V5b predictor) is not validated (V5 and V5b missed).
  The V5c predictor passed its pre-registered bars (`V5c_RESULTS.md`) but is not yet the one the
  fleet assessment runs; until it is, do not present the time to breach as a validated figure.
* A PDF rendering of the audit report; the HTML is the deliverable.

## Holdover under attack: PRTC/ePRTC and substation PTP (W1119 C8)

`kshana-pro timing attack <scenario.toml | prtc | eprtc | substation> --out <dir> --signer <name> --key-file <file>`
(licence: `pro-timing-scorecard`) runs a GNSS-disciplined clock through a spoof that drags GNSS
time slowly and then steps it, and reports the time error against masks. The oscillator is the
eLoran crate's `sync::edo` model (presets `ocxo`, `rubidium`, `cesium`, or explicit ADEV terms)
and the discipline is the same type-2 loop. Detectors: `none`; `rate_guard` (the loop's phase
error over a window: catches steps, not the drags a type-2 loop tracks); `cross_check` (GNSS
against eLoran after a calibration window: catches a drag once it passes the threshold). On
alarm the clock goes to `holdover` (steering rolled back `rollback_s` before the alarm, so the
drag's frequency stays out of holdover) or to `eloran` (its uncalibrated bias carried into the
output). The report gives the alarm time and latency, TE at alarm, max |TE| before and after
the attack, a result per mask, MTIE, and a 10 s TE trace.

Default masks are single max |TE| bars citing ITU-T G.8272 (PRTC-A 100 ns, PRTC-B 40 ns),
G.8272.1 (ePRTC 30 ns locked; 100 ns holdover in the `eprtc` preset), a G.8271-class
end-application budget (1.5 µs) and, for `substation`, the IEEE C37.238 example ±1 µs budget less
a 200 ns network allowance. All are illustrative defaults, configurable, to be confirmed against
the Recommendations and the standard; the full MTIE/TDEV masks are not applied. The substation
case also checks the C37.238 time-inaccuracy view: the inaccuracy the grandmaster would advertise
(locked value, plus a holdover growth model) against its actual TE; an advertised value below the
actual TE is reported as misleading. `kshana-pro timing attack-template <preset>` prints a preset.

Evidence: **MODELLED** (closed-form checks in `tests/holdover_attack.rs`); not run on a real clock.

## Timing assurance v2: time bombs, self-check, holdover evidence, network packs (W1120)

Research 05, 07b, 08b and 12 (20 Nov 2026) found four gaps around fielded timing: firmware
"time bombs" (Telstra's national outage of 8 Jul 2026 was reported as a GPS rollover-type flaw
after a vendor warning was not applied; secondary sources, to verify against ACMA/Telstra),
nodes whose time is wrong while GNSS looks healthy (a Turkish operator's cell ran 6,250 µs off
with "no reference to validate the timing"), holdover evidence for Finland's Regulation 54
(mobile networks withstand at least two weeks without GNSS), and audit findings such as a
grandmaster cable delay "not set at all" (about 500 ns). Code: `src/timing/timebomb.rs`,
`selfcheck.rs`, `holdover_evidence.rs`, and the new items of `compliance.rs`. Demo inputs (all
SYNTHETIC, written by `examples/timing-assurance-v2/generate.py`):
`examples/timing-assurance-v2/`.

| Command | Licence feature | Tier |
|---|---|---|
| `timing timebomb vectors / analyse / fleet` | `pro-timing-timebomb` | Pro |
| `timing timebomb advisories` | none | free |
| `timing selfcheck node` | `pro-timing-selfcheck` | Pro |
| `timing selfcheck sites` | `pro-timing-correlation` | Enterprise |
| `timing holdover-evidence` | `pro-timing-holdover-evidence` | Pro |
| `timing compliance itu-g8271-1`, RTS 25 items 6 and 7 | `pro-timing-audit` | Pro |

All four features are at the `pro` edition in the signed tier manifest.

### TA1: time-bomb auditor

```sh
kshana-pro timing timebomb vectors --out vectors/ --pivot 2019-03-14 --at 2027-03-14T00:00:00Z
kshana-pro timing timebomb analyse vectors/gps-wnro-2038 capture.nmea --format nmea --json a.json [--strict]
kshana-pro timing timebomb fleet inventory.toml --advisories ours.toml --csv fleet.csv
```

**Vectors.** Each vector covers `--pre` (120 s) before to `--post` (180 s) after a hazard
epoch, at 1 Hz: `gps-wnro-2038` (10-bit week 3072, 2038-11-20T23:59:42Z), `gps-wn8192`
(13-bit, 2137-01-05), `ntp-era-2036` (2036-02-07T06:28:16Z), `unix-2038`
(2038-01-19T03:14:08Z), `rmc-century-2080` (two-digit RMC year), `leap-insert` and
`leap-delete` (a synthetic positive and negative leap second at the end of `--leap-date`,
default 2027-06-30: no leap second is announced, the date is hypothetical), `lsf-ambiguity`
(the next 128-week multiple since the last leap second's week 1929, where an 8-bit WN_LSF
becomes ambiguous; the 2025 bug of research 05 [81] triggered after 256 weeks), one
`pivot-rollover-<date>` per `--pivot` (1024 weeks after the GPS week of a firmware date) and
one `custom-<epoch>` per `--at`. Files per vector:

* `replay.nmea`: `$GPZDA` and `$GPRMC` per second (CRLF), for serial replay into a consumer of
  receiver output (an NTP server's refclock, gpsd/chrony, a GPSDO or grandmaster with an
  external receiver input);
* `replay.ubx`: UBX NAV-TIMEGPS, NAV-TIMEUTC (second 60 during a leap second) and NAV-TIMELS
  (the leap-second announcement and countdown) per second, field layout of the u-blox M8
  protocol description (check the device's protocol version);
* `simulator.json`: the start in UTC and GPS time (full week, week mod 1024 and mod 8192),
  the LNAV UTC parameters to broadcast (ΔtLS, ΔtLSF, WN_LSF full and 8-bit, DN), and the
  gps-sdr-sim command line from the eLoran crate's adapter (`-T` moves any RINEX navigation
  file to the epoch; gps-sdr-sim takes its UTC parameters from the RINEX header and cannot set
  WN_LSF or DN, so the leap vectors need a simulator that can);
* `expected.csv` (the correct output) and `vector.json`; `manifest.json` lists every file's
  SHA-256.

**Analyser.** Reads the device's output during the replay: NMEA (ZDA, else RMC read as 20yy;
a line may start with a lab time stamp), UBX (NAV-TIMEUTC, else NAV-TIMEGPS minus leapS) or
CSV (`[lab_time,]device_time`, ISO 8601 with second 60, or Unix seconds). Records are paired
with the vector's epochs by lab time when every record has one, else in order; the capture is
taken to start with the replay (`--start-index`, `--replay-start <lab time>` otherwise). The
device's own time never sets the pairing, so a constant error shows from the first record.
Faults: `week-rollover` (a multiple of 1024 weeks), `century` (about 100 years), `wrap-32`
(2^32 or 2^31 s), `gps-utc-offset` (17 to 19 s), `leap-not-applied` (±1 s at the leap event),
`leap-mistimed` (±1 s elsewhere in a leap vector), `unexpected-leap` (±1 s with none due),
`time-jump`, `stuck-time` (three or more records with the same time), `no-output` (a gap over
5 s). A leap second shown as a repeated 23:59:59 that returns to the right time is a note.

**Fleet.** An inventory (`[[devices]]`: id, site, vendor, model, firmware, firmware_date,
role, `week_resolution` = pivot | full | unknown, `ntp`, `time_t_32bit`, `nmea_input`) is
checked against the epoch hazards (with dates and days from `--utc`) and the advisory table:
the seed (`src/timing/timebomb-advisories.toml`, printed by `timebomb advisories`) plus the
operator's `--advisories` files. A match is `affected` (firmware below `fixed_in`),
`not-affected`, or `review` (no version range in the table, or no firmware in the inventory).
Priority: `act` (affected, or a hazard within a year or past), `plan` (within five years),
`watch`, `info`. The seed carries only entries traceable to the research: six Microchip
TimeProvider 4100 CVEs (CVE-2024-9054, -43684, -43685, CVE-2025-47900, -47901, -47904, as
research 08b summarised the vendor and NVD pages; fixed version 2.5 only where the research
states it), Meinberg MBGSA-2025.07 (fixed version read from the advisory page's title) and the
other 2025 LANTIME advisories by count, the rollover report behind the July 2026 outage (a
community report; product not named) and a time-nuts report of a GPSDO's 2004 rollover. Every
entry gives its source and how it was checked; examples are labelled `EXAMPLE-` and a CVE id
is accepted only on a vendor security advisory.

Labels: vectors are COMPUTED; an analysis is a LOG ANALYSIS of one capture, not a
certification of the device; a fleet match is a prompt to read the vendor's advisory.

### TA2: clock self-check and cross-site correlation

```sh
kshana-pro timing selfcheck node examples/timing-assurance-v2/node/node.toml --json n.json --csv bins.csv
kshana-pro timing selfcheck sites examples/timing-assurance-v2/sites/sites.toml --json s.json
```

**Node.** Each `[[sources]]` gives the node's offset from that source (node minus source) with
its standard uncertainty `u_s`: `csv` (`unix_time,offset_s[,locked]`), the open engine's line
readers for `ptp4l`, `phc2sys`, `ts2phc` (with the PTP path delay), `chrony-tracking`,
`chrony-statistics` and `pps-assert`, or ntpd `ntp-peerstats` (sign −1 by default: ntpd's
offset is the server's minus the local clock's). `select` keeps one peer; `sign` and
`time_offset_s` fix a log's sign or a monotonic time base; at most one source is
`disciplining`. Per bin (`bin_s`, default 60 s), the median and MAD of the sources start an
iterated weighted vote; a source is an outlier when `|o_i − c| > k·sqrt(u_i² + u_c²) + floor`
(`k` 3). Statuses: `agree`, `node-off` (every source agrees the node is off),
**`gnss-outlier`** (the disciplining source reads within `k·u` of zero and locked, while at
least two independent sources agree the node is off: the node's time is wrong although its
GNSS looks healthy), `disciplining-outlier`, `source-outlier`, `no-majority`, `pair-agree` /
`pair-disagree` (two sources: a disagreement is seen but not attributed) and `single-source`
(the time cannot be validated). A PTP path-delay change above `path_delay_step_s` (100 ns)
between bins is an event. An optional holdover state log (`unix_time,state`) is carried into
each bin and event. On the demo node (GNSS PPS, PTP from a regional grandmaster, two NTP
servers) the two-hour 6.25 ms error behind a healthy PPS is one `gnss-outlier` event.

**Sites.** One series per site (the site's time against a common monitor, any reader above),
with `operator` and `region`. Per bin with at least `min_sites` (3) sites, a site beyond
`k·sqrt(u_i² + σ²) + floor` of the median (σ = 1.4826 MAD) is flagged; a two-sided CUSUM
(slack 0.5, decision interval `cusum_h` 5) reports a `slow-divergence` before or without a
threshold crossing; the median moving from its baseline is a `common-mode` event (every site
moved: a system-wide event such as SVN-23, seen only against a reference that does not share
the common source). Each flagged run has a scope: `single-site`, `operator-wide`, `regional`
(several operators in one region) or `multi-operator`.

Label: MODELLED (a vote on the logs as supplied, with the operator's stated uncertainties and
signs); tested on synthetic logs.

### TA3: holdover evidence

```sh
kshana-pro timing holdover-evidence examples/timing-assurance-v2/holdover/cesium-reg54.toml \
    --out hold/ --signer "Example operator" --key-file signer.key
kshana-pro timing holdover-evidence presets
```

`TE(t) = x0 + y0·t + D·t²/2` with the standard uncertainties of the time error at the loss
(stated, and an ITU-T equipment class as a rectangular ± bound: `prtc-a` 100 ns, `prtc-b`
40 ns, `eprtc` 30 ns, `t-bc-class-a/b/c` 50/20/10 ns cTE, as transcribed and to confirm with
G.8272, G.8272.1 and G.8273.2), the residual frequency offset (`t·u(y0)`), ageing
(`t²/2·u(D)`), temperature (`t·c_T·ΔT/√3`) and the oscillator noise (the open engine's coast
variance `q_wf·t + q_rw·t³/3`, from a clock class, datasheet ADEV points or TDEV points
converted under a white-FM assumption), combined by the GUM and expanded with `k` (2). The
report gives the bound `|TE| + U` against the limit over the period, the budget table at the
end of the period, the largest supported holdover and P(|TE| > limit); it lists every default
it applied. Presets: `finland-reg54` (336 h, ±1.5 µs; the limit is the ITU-T G.8271
end-application budget, the operator's choice, not a figure of the Regulation) and
`g8271-1-day`. The directory (`report.json`, `report.html`, `budget.csv`, `curve.csv`,
`inputs/holdover.toml`) is signed; verify with `kshana-pro dossier verify` (free). On the demo
inputs the rubidium site falls short of two weeks (between one and fourteen days, as the tests pin) and the caesium reference holds them.

Label: MODELLED: a prediction from the stated characterisation, not a measured holdover.

### TA4: evidence-pack items

* `mifid2-rts25` gains **RTS25-6** (the antenna cable delay configured: expected delay
  `L / (v·c)` plus fixed delays, with its GUM uncertainty from the length and the velocity
  factor, against `configured_s`; not set or off by more than `2u + tolerance` is "not
  supported by the evidence") and **RTS25-7** (the holdover behaviour documented: the
  operator's `holdover_procedure`, a holdover evidence report covering `required_holdover_h`
  at a limit no looser than the activity's). Inputs `[cable_delay]` and
  `holdover_evidence = { path = ... }`.
* New framework **`itu-g8271-1`**: G8271-1 PRTC max|TE| at reference point A (`[prtc]`
  record; PRTC-A 100 ns or PRTC-B 40 ns), G8271-2 max|TE| at reference point C (`[clock]`
  record plus `network.u_measurement_s`, against 1100 ns or `network.max_te_s`), G8271-3 dTE
  MTIE against the operator's transcription of the mask (`network.mtie_mask`), G8271-4 the
  cable delay, G8271-5 holdover for the required period, G8271-6 independent validation (a
  self-check report; supporting evidence, not a provision of the Recommendation) and G8271-7
  GNSS loss and spoofing. Limits are as transcribed and to confirm with the Recommendations.

Every pack keeps the C4 verdicts and the wording rule (tested on every framework).

### Not built in v2

* No radiating test signal: the vectors are files for a lab's own simulator or serial replay;
  the RF route goes through the operator's simulator.
* No CVE feed: the advisory table is curated by hand; its seed carries only what the research
  reports name, and several vendor pages behind them were not read directly.
* No PTP management-plane (`pmc`) or SNMP reader for the self-check; logs only.
* None of TA1 to TA4 has been run against a real device, network or operator record.

## Timing networks (E19: B1902, B1904, B1905, B1907)

Research 05, 06, 06b, 07b, 08b, 09b and 12 (20 Nov 2026) found four more gaps around the
networks that carry time: the security posture of the appliances themselves (six CVEs for one
grandmaster line in 2024-2026, one of them upgrade images that are not signed,
CVE-2025-47904; most 2025 fixes of one time-server line were bundled OpenSSH, curl and
libxml2), one statement of a distribution chain from UTC(k) to an end point, a cross-check
against sources that do not share GNSS's failure modes, and timing and pass-quality evidence
for ground stations and teleports. Code: `src/timing/posture.rs`, `chain.rs`, `crosscheck.rs`,
`station.rs`, CLI `src/cli/timing_net_cmd.rs`. Demo inputs (all SYNTHETIC, written by
`examples/timing-networks/generate.py`): `examples/timing-networks/`. Tests:
`tests/timing_networks.rs`.

| Command | Licence feature | Tier |
|---|---|---|
| `timing posture check / firmware / sbom station` | `pro-timing-posture` | Pro |
| `timing posture sbom check` | none | free |
| `timing chain` | `pro-timing-chain` | Pro |
| `timing chain template` | none | free |
| `timing crosscheck` | `pro-timing-crosscheck` | Pro |
| `timing crosscheck profile` | none | free |
| `timing station` | `pro-timing-station` | Pro |

All four features are at the `pro` edition in the signed tier manifest.

### B1902: appliance security posture

```sh
kshana-pro timing posture firmware posture/firmware.img --sha256 <vendor digest> --version 2.5 --json posture/gm-2.fwcheck.json
kshana-pro timing posture check posture/inventory.toml --components posture/components.toml --json p.json --csv p.csv
kshana-pro timing posture sbom station posture/station-firmware.toml --out station.cdx.json
kshana-pro timing posture sbom check station.cdx.json
```

**Exposure report.** An inventory (`[[appliances]]`: id, site, vendor, model, firmware, role,
`management` = isolated | internal | internet | unknown, `services`, `upgrade_signing` =
device-verifies | unsigned | unknown, `firmware_check` (a record from `posture firmware`),
`sbom` (the vendor's CycloneDX JSON)) is checked against:

* the **appliance advisory table** of the time-bomb auditor (the same seed and `--advisories`
  files, matched by the same code: `affected`, `not-affected`, `review`); a `review` on an
  appliance whose management is reachable from the internet becomes `act`;
* **firmware integrity**: an advisory tagged `unsigned-firmware` (the seed tags
  CVE-2025-47904) or `upgrade_signing = "unsigned"` needs an out-of-band check of the installed
  image; without a matching record it is `exposed` (`act`), with one whose version matches the
  installed firmware it is `mitigated`; an unknown signing practice is `review`;
* **management reach and services**: management from the internet is `act`; clear-text or
  amplifying services (telnet, http, ftp, tftp, SNMPv1/v2c, NTP mode 6 and 7) are `plan`, or
  `act` from the internet;
* the **vendor SBOM's components** (nested ones included) against a component advisory table
  (`[[component_advisory]]`: id, component, `affected_from`, `fixed_in`, hazard, source,
  verification; `--components`, repeatable). No component table ships: the operator brings its
  own (the demo's entries are labelled EXAMPLE-).

**Firmware check.** The SHA-256 of an image against the vendor's published digest
(`--sha256`), a `SHA256SUMS` file looked up by file name (`--sums`), or an Ed25519 detached
signature (raw, hex or base64) with the vendor's public key (`--sig`, `--pubkey`). The record
(`kshana-pro-firmware-check/1`) carries the digest, method, reference and verdict (`match`,
`mismatch`, `not-listed`, `signature-valid`, `signature-invalid`); the command fails on
anything but a match. It is the operator's control for appliances that do not verify their own
images; it does not show that the vendor's digest or key is genuine.

**SBOM.** `sbom station` writes a CycloneDX 1.5 JSON BOM of a station's firmware from a
component list (name, version, type, purl, supplier, SPDX licences, SHA-256 or a file to hash,
`depends_on`), with a reproducible `urn:uuid` serial. The Pro release's own BOM is
`sbom.cdx.json` of the offline bundle (`scripts/sbom.py`, unchanged). `sbom check` checks any
CycloneDX JSON (1.4 to 1.6) for the required fields, the component-type and hash-algorithm
enumerations, hash lengths, unique `bom-ref`s, purl form and dependency references; it is a
structural check, not a full JSON-schema validation.

Label: COMPUTED from the inventory and tables as supplied; a match is a prompt to read the
advisory, not a finding that the appliance is exploitable.

### B1904: time-distribution chain verifier

```sh
kshana-pro timing chain examples/timing-networks/chain/chain.toml --out stmt/ --signer "Example operator" --key-file signer.key
kshana-pro timing chain template
```

A request gives the date, the reference laboratory with **UTC − UTC(k)** (the operator's
transcription of BIPM Circular T rows `mjd,value_ns,u_ns`, interpolated to the date; after the
last row it is held for at most `max_extrapolation_days` (45) with `extrapolation_u_s_per_day`
(0.5 ns) added in quadrature, and marked provisional; or a stated value), and the hops:

* `wr`: the White Rabbit link model, round trip `δMM = Δtxm + Δrxm + Δtxs + Δrxs + δms + δsm`,
  fibre asymmetry `α = δms/δsm − 1`, so `δms = (1+α)/(2+α)·(δMM − ΣΔ)`. `α` is stated or
  **estimated from a calibration measurement** (`[hops.alpha_calibration]`: the time interval
  from the master's PPS to the slave's over a spool, with the `α` configured during the
  measurement; GUM by finite differences over the round trip, the four fixed delays and the
  PPS reading). A configured `α` that differs from the calibrated one is a bias (the slave reads
  behind by the delay it underestimates). Components: `α`, the fixed delays and the round trip.
  The spool's `α` applies to the deployed fibre only if it is the same fibre type and
  wavelength pair;
* `ptp`: a stated asymmetry (bias) with its uncertainty and the cTE of the equipment class as a
  rectangular bound (class A 50 ns, B 20 ns, C 10 ns, as transcribed from ITU-T G.8273.2);
* `fibre-twtt`, `gnss-cv` (a common-view comparison, type A) and `stated`.

Every hop needs a calibration record (`id`, `date`, `valid_days`, `by`); the chain is
`unbroken`, `provisional` (Circular T extrapolated) or `broken` (an expired, missing or
not-yet-valid calibration, or a reference too old), naming the break. The end point's offset
from UTC is every hop's bias (output minus input) plus the laboratory's offset UTC(k) − UTC,
which is **minus** the Circular T value: `End − UTC = Σ bias − (UTC − UTC(k))`, with the combined and expanded (`k`)
uncertainty, against an optional `limit_s`. An **onboarding test** (`[onboarding]`) for a
national time service compares the delivered time against the operator's reference over a
window: its mean must agree with the chain's predicted offset within
`k·sqrt(u_c² + u_ref² + s²/n)` (`consistent`, `inconsistent`, or `wander-exceeded` against
`max_wander_s`). The directory (`statement.json`, `statement.html`, `budget.csv`, `inputs/`
with the Circular T and onboarding files) is signed; verify with `kshana-pro dossier verify`
(free). The tests recover a synthetic `α` within its uncertainty and the deployed link's
asymmetry within its budget.

Label: MODELLED: the arithmetic of the stated measurements and records; a traceability
statement for the operator's own records, to be reviewed with the laboratory that calibrated
the chain.

### B1905: multi-source cross-check with non-GNSS sources

```sh
kshana-pro timing crosscheck examples/timing-networks/crosscheck/pop.toml --json cc.json --csv bins.csv
kshana-pro timing crosscheck profile wisp
```

Sources as in the self-check (any of its log formats), each with a class (`gnss`,
`radio-time`, `bps`, `eloran`, `leo`, `fibre`, `ptp`, `ntp`, `nts`, `holdover`, `local`,
`other`) and a **traceability root**: GNSS receivers share `gnss`; a long-wave signal is its
transmitter (`dcf77`, `msf`, `wwvb`, `jjy40`, `jjy60`, `bpc`); `depends_on` gives a derived
source its upstream's root (a PTP grandmaster disciplined by the same GNSS is not
independent); other classes are their own root unless `root` says otherwise, and are listed as
`unstated_independence`. Long-wave offsets are corrected for the ground-wave delay
(great-circle distance from the transmitter over `c`; approximate sites; `delay_s` overrides).

Per bin: a member beyond `k·u + floor` of its root group's median is a `member-fault`; the
groups' values give `consistent`, **`isolated`** (exactly one root inconsistent with every
other while the rest agree: a single-source fault isolated), `detected-not-isolated` (two roots
disagree), `ambiguous`, `multiple-faults` or `no-independent-check`; the consensus beyond
`node_limit_s` is `node-off`. A source silent for `stale_s` is a `silent` event (alarm time =
last reading + `stale_s`). A **triad monitor** (three-cornered hat over every trio of roots,
leaving out bins where a root was isolated) estimates each root's own noise and flags one
noisier than twice its stated uncertainty. Profiles: `wisp` (GNSS PPS, a long-wave receiver,
NTS servers of two operators: a low-cost multi-source appliance for a wireless ISP) and
`timing-node` (GNSS, PTP disciplined by it, fibre from UTC(k), eLoran). The default
uncertainties per class are typical orders of magnitude for the operator to replace.

Label: MODELLED: a vote across independent sources on the logs as supplied; a fault is isolated
only as far as the sources are truly independent, and a fault common to every root is
invisible. Tested on synthetic logs.

### B1907: ground-station timing and pass-quality assurance

```sh
kshana-pro timing station examples/timing-networks/station/station.toml --json st.json --csv passes.csv \
    --out st/ --signer "Example teleport" --key-file signer.key
```

A pass log (CSV with a header: `pass_id, satellite, scheduled_aos, scheduled_los` and
optionally `antenna, provider, customer, actual_aos, actual_los, bytes_planned,
bytes_delivered, frames_bad, frames_total, status`; ISO 8601 or Unix times) and the station's
clock log (station minus UTC, any self-check format). Per pass: `delivered`, `degraded` (the
criteria missed: contact fraction, AOS delay, data delivered, frame error rate, station time
error), `missed` (no contact, `failed` or `cancelled-station`) or `excluded`
(`cancelled-customer`). The station time error is the largest |offset| of the clock log in the
pass window, against a stated `limit_s` or a limit derived from the antenna (MODELLED:
`pointing_fraction × beamwidth / ω`, `ω = v/h` the zenith angular rate of a circular orbit at
`altitude_km`, Earth rotation neglected; 2° and 10 % at 500 km give about 0.23 s). Aggregates
per station, provider (federated booking), customer, satellite and antenna, with the success
rate against `target_success_rate`. With `--out`, a signed directory (`report.json`,
`report.html`, `passes.csv`, `inputs/`) for customer reports and the operator's periodic
reporting; it follows no regulator's template.

Label: COMPUTED for the SLA figures from the logs as supplied; MODELLED for the derived timing
limit. Tested on a synthetic station.

### Not built in E19

* B1903 (cross-grandmaster and cross-operator correlation as a hosted verdict) and B1908 (hosted
  delay-calibration verdict) belong to the hosted service; the in-crate cross-site check stays
  `timing selfcheck sites`.
* No CVE or SBOM feed: the advisory and component tables are curated by the operator; the
  seed carries only what the research reports name.
* The firmware check verifies an image file before an upgrade; it does not read firmware back
  from a device.
* None of B1902, B1904, B1905 or B1907 has been run against a real appliance, link, node or
  station.

## Power grid and broadcast SFN (E31)

PMU phase-angle evidence, gradual-spoof detection, SCADA time stamps, PRC-002 evidence with
holdover resilience, grid event forensics and DAB/DVB-T SFN timing build on this layer: see
[`GRID-SFN-TIMING.md`](GRID-SFN-TIMING.md).

## Clock benchmarks (E33: B3302)

Several clocks scored on the same stated metrics, from their own logs, under a protocol fixed
before the data: optical against caesium, rubidium and chip-scale atomic clocks, MEMS against
quartz, a clock at sea or in a temperature chamber against its own baseline. Code:
`src/timing/clock_bench.rs`, CLI `src/cli/clock_bench_cmd.rs`. Demo (all SYNTHETIC, written by
`examples/clock-bench/generate.py`): `examples/clock-bench/`. Tests: `tests/clock_bench.rs`.

| Command | Licence feature | Tier |
|---|---|---|
| `timing clock-bench register` | none | free |
| `timing clock-bench score` | `pro-timing-clock-bench` | Pro |

```sh
kshana-pro timing clock-bench register examples/clock-bench/bench.toml --json protocol.json
kshana-pro timing clock-bench score examples/clock-bench/bench.toml --csv table.csv \
    --out bench-evidence --signer lab --key-file lab.key
```

**Pre-registration.** The `[protocol]` table names the metrics (`adev`, `tdev`, `mtie`,
`holdover`, `sensitivity`, `datasheet`), the averaging times, the confidence level, the
measurement reference and its stated Allan deviation, the floor factor, the holdover test
(training span, window, threshold) and the phases data will be taken in (`baseline` and any of
temperature, vibration, shock, at sea, ...). `register` prints the SHA-256 of its canonical JSON
(`protocol.json` with `--json`). Publish or sign that text before any data is taken. The request
records the hash as `protocol_sha256`, and `score` refuses a protocol that no longer hashes to it.
So the protocol cannot be changed once the results are seen. A clock may state
`collected_from_utc`. The report says whether that is after `registered_utc`, as stated by the
operator. The logs themselves are not checked for it.

**Metrics.** Each clock gives one log per phase (`[[clock.segment]]`), read by the open engine's
clock-log readers (TimeLab, TICC, RINEX clock and the other formats of
`docs/CLOCK-LOG-FORMATS.md` in the open repository). Per phase and averaging time the report gives:

* the overlapping Allan deviation with a chi-squared interval (conservative degrees of freedom);
* the TDEV and the MTIE;
* a holdover test: the frequency is fitted on the training span, and the report gives the largest
  prediction error over the window and the first time it reaches the threshold;
* for each phase other than the baseline, the mean frequency shift from the baseline and the
  Allan deviation ratio to it.

Datasheet Allan deviation claims, when a clock states them, are reported as the measured/claimed
ratio on the baseline, at the protocol's averaging times.

**Reference floor.** A point below `floor_factor` times the reference's stated Allan deviation is
flagged `reference_limited`. It is left blank in the side-by-side table, because it measures the
reference as much as the clock. In the demo the synthetic optical unit is reference-limited at
every averaging time, which is the correct result against that reference.

**Neutrality.** Clocks are listed in input order. The side-by-side table (`table.csv`, and the
HTML scorecard) puts the same metric for every clock on one row. Nothing is ranked or summed into
a score, and there is no winner. A maker appears only as the document a clock's claims come from.
With `--out`, a signed directory (`scorecard.json`, `scorecard.html`, `table.csv`,
`protocol.json`, `inputs/bench.toml`); verify with `kshana-pro dossier verify` (free).

Label: COMPUTED from the logs as supplied under the registered protocol; the holdover test is a
linear-frequency prediction (MODELLED). Tested on synthetic logs only. Not a type approval or an
endorsement.

### Not built in E33

* No run on real clocks: the demo and the tests use synthetic white-FM logs.
* The holdover test is not flagged against the reference floor (only the Allan deviation is).
* The environmental phases are compared as whole logs. There is no per-event analysis inside a
  phase (a shock's step is seen only in the phase's mean frequency and stability).
* The open engine's oscillator models (`kshana oscillator`, open B3301) are not called from here.
  This crate pins the published engine (0.41.0), which does not carry them yet.
