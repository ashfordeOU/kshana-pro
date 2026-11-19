# Referee scorecards and the receiver resilience test report (roadmap 0.42, E3)

A test campaign puts one receiver through threat cases and scores it under bars written before
scoring. Code: `src/scorecards/`; kind `pro-receiver-scorecard` (`pro` edition and licence
tier); command `kshana-pro scorecard run|verify`. Results are **test evidence mapped to** the
DHS Resilient PNT Conformance Framework levels and the IEEE P1952 draft topics; a scorecard
never says a receiver is compliant, conformant or certified.

## A campaign

```toml
kind = "pro-receiver-scorecard"
title = "Vendor X receiver, simulator campaign"
[receiver]
maker = "Vendor X"
model = "RX-1"
firmware = "4.2"
[campaign]
id = "C-2026-007"
operator = "Test lab"
facility = "Lab simulator replaying threat library exports"
[bars]                         # stated before scoring (defaults shown)
fixed_utc = "2026-11-20T09:00:00Z"
misleading_error_m = 50.0      # a valid fix farther than this from truth is wrong output
misleading_time_error_s = 0.5
max_recovery_s = 120.0         # level 1
recovery_hold_s = 10.0
bounded_error_m = 500.0        # level 3
bounded_time_error_s = 1.0
bounded_min_availability = 0.5
full_error_m = 20.0            # level 4
full_min_availability = 0.99
monitor_counts_as_warning = false

[[case]]                       # a log of the receiver under test, events from a library scenario
id = "hormuz"
library = "hormuz-position-displacement"
library_offset_s = 0.0
[case.log]
format = "ubx"                 # ubx | rinex | android | nmea (open readers)
path = "logs/hormuz.ubx"
platform = "vessel"
[case.truth]                   # the export's I4 motion CSV
motion_csv = "export/testbench/hormuz-position-displacement.motion.csv"

[[case]]                       # a library analogue alone (the public sample does this)
id = "texbat-ds4"
library = "texbat-ds4-matched-power-position-push"
```

A log case may instead give `[[case.events]]` (receiver-trust events: label, kind, onset_s,
end_s) stated before scoring.

## What is scored (method `kshana-pro/receiver-scorecard/v1`)

Per event: the open monitor's outcome and latency on the receiver's log (receiver-trust), the
receiver's own first flag (UBX-NAV-STATUS, UBX-SEC-SIG), fix availability, largest and 95th
percentile error against truth, largest time error (library cases), **misleading epochs**
(wrong output with no warning from the receiver; and with neither the receiver's warning nor
the monitor's alarm), and recovery (correct output held for the hold time after the event).
Per case and level: level 1 every event recovered within the bar; level 2 also no misleading
epoch; level 3 also every fix within the bound and enough availability; level 4 the full bound.
Without truth, levels 2 to 4 are "not assessed".

## Frozen format

`receiver-scorecard.json` is `kshana-pro-receiver-scorecard/1`, top-level fields `SCORECARD_FIELDS`, case
fields `CASE_FIELDS` (`src/scorecards/mod.rs`); `tests/receiver_scorecard.rs` fails if they
drift. The report directory: `receiver-scorecard.json`, `report.html`, `campaign.toml` (as run; its
SHA-256 and the bars' SHA-256 are in the scorecard), `cases.csv`, `mapping/` (the two templates
in `report-templates/scorecards/`), `README.txt`, `signature.json`. Verify for free:
`kshana-pro dossier verify <dir> --trust <key>` or `kshana-pro scorecard verify`.

## Public sample

`examples/scorecards/public-analogue-campaign.toml`: the eleven library analogues, the open
engine's synthetic NMEA receiver model as the unit under test. Pre-registered in
`docs/evidence/e3-receiver-scorecard/2026-11-15_PREREGISTRATION.md`; results in `RESULTS.md`
(E3-B1 met, E3-B2 (a) missed and published, E3-B2 (b) met, E3-B3 met). Signed sample:
`report-templates/samples/receiver-scorecard-public/`.

## Limits

* v1 aligns truth by the receiver's own epoch times: a combined position-and-time delay
  (meaconing) hides its position error (E3-B2 (a) miss). Use the time error and the misleading
  count, which still see it.
* No campaign on a real receiver has been run yet; the sample is a modelled receiver.
* P1952 is a draft: topics are mapped by subject, without clause numbers.

## PICS: PNT Interference Conformance Suite (W1119 C1)

A fixed, versioned suite of eight interference scenarios (`pics/suite-v1.toml`, suite
`kshana-pics` 1.0.0): jamming (CW, chirp, broadband) and spoofing (simplistic, intermediate,
time push, position drag, meaconing). Each has its threat parameters, the expected device
behaviour in words and pass bars. The bars are pre-registered: their SHA-256 hashes are in
`pics/suite-v1.lock`, compiled in, and a suite whose bars differ is refused. A bar change is a
new suite version.

```
kshana-pro pics list | show <id> | mapping
kshana-pro pics sample <id> --behaviour good|bad --out log.json     # SYNTHETIC log, to try the runner
kshana-pro pics score log.json [more logs] --out <dir> --signer <name> --key-file <key>   # Pro
kshana-pro pics score cap.nmea --input nmea --scenario PICS-JAM-CW --t0 0 ...            # NMEA log
kshana-pro pics score te.log --input timing --clock-format ticc --scenario PICS-SPF-TPUSH ...
kshana-pro pics export <id> --profile skydel|spirent|labsat|rs-smbv
kshana-pro pics verify <dir> --trust <public key>                   # free: signature, files, bars
```

Scoring (method `kshana-pro/pics/v1`): detection within the bar (spoofing needs an explicit
flag, jamming also accepts a withdrawn fix), no valid unflagged output beyond the bars, and
recovery within the bar held for 30 s. These three decide `pass`. The bounded-error and
full-performance criteria grade the DHS RPCF level with the scorecard level logic above. Data
a log lacks makes a criterion `not assessed`, never passed.

The clause mapping (`pics/mapping-v1.toml`) covers MSC.401(95), MSC.1/Circ.1575, IEC 61108-1,
ED-259A App. W, DO-401A (draft), IEEE P1952 (draft), DHS RPCF levels, ITU-T G.8272.1 and
G.8273.2, and IEEE C37.238. Clause numbers are cited only for Circ.1575 (paragraphs, as read
by the open engine's standards pack), G.8272.1 and G.8273.2 (as transcribed in the open
engine's telecom-timing masks), ED-259A Appendix W and the RPCF levels. Every other row says
"mapping to be confirmed".

Evidence label: MODELLED. The scoring is checked on synthetic logs (`tests/pics.rs`). No device
has been scored, and the threat parameters and bars are illustrative until they are confirmed
against the standards. The simulator export carries jammers as commands; a spoofer is a
schedule marker plus its per-second offsets, and the operator sets it up in the simulator's
own spoofing scenario.

## OSNMA receiver conformance scorecard (W1119 C6)

`kshana-pro scorecard osnma <receiver-log.json> --out <dir> --signer <name> --key-file <file> [--bars <bars.toml>]`
(licence: `pro-receiver-scorecard`) scores an OSNMA-capable receiver from a log in the format
`kshana-osnma-receiver-log/1` (JSON: receiver identity, and per test the kind, the event time,
the forged satellites, and per epoch the overall authentication status A/F/N, the fix, and per
satellite its status and whether it was used). Test kinds and their pre-registered bars
(illustrative defaults, all configurable, hashed into the scorecard):

| Kind | Bar (default) |
|---|---|
| `ttfaf` | first authenticated fix within 300 s cold, 120 s warm, 60 s hot |
| `forged_nav` | forged satellites rejected (F/N, not used) within 60 s and never used authenticated afterwards; ≥ 90 % of genuine-satellite epochs stay authenticated |
| `replay` | no authenticated fix from replayed signals later than 60 s after the event |
| `chain_renewal`, `pk_renewal`, `merkle_renewal` | authentication resumes with an outage ≤ 60 s and no failure on genuine signals |
| `pk_revocation` | no authenticated status later than 60 s after the revocation |

Each scorecard carries a stimulus self-check made with the open engine's OSNMA verifier
(`kshana::osnma::tesla` and `merkle`): a genuine TESLA key and public key verify, the forged
and replayed ones fail. References: Galileo OSNMA SIS ICD Issue 1.1 and Receiver Guidelines
Issue 1.3; the bars are not taken from them (illustrative until confirmed). A scorecard is test
evidence, never a statement of conformance. `kshana-pro scorecard osnma-synth <good|faulty> <file>`
writes a synthetic log.

Evidence: **MODELLED** (synthetic receiver logs, `tests/osnma_conformance.rs`).
