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
