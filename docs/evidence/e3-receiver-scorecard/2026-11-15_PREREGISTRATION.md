# E3 receiver scorecard: pre-registration of the public sample campaign (15 November 2026)

Written and committed **before** the first scoring of
`examples/scorecards/public-analogue-campaign.toml`. The results go in `RESULTS.md` next to
this file, misses included.

## What is scored

* **Campaign:** `examples/scorecards/public-analogue-campaign.toml`: the eleven threat library
  (E2, edition 2026-11) scenarios that carry an NMEA analogue. The unit under test is the open
  engine's synthetic NMEA receiver model (`kshana::nmea_synth`, kshana 0.41.0); truth is the
  generator's own true track and true time. Public data only.
* **Method:** `kshana-pro/receiver-scorecard/v1` (`src/scorecards/mod.rs`), format
  `kshana-pro-receiver-scorecard/1`.
* **Bars of the campaign** (in the file, `[bars]`): wrong output is a fix more than 50 m or a
  time more than 0.5 s from the truth; level 1 needs recovery within 120 s (held 10 s); level 3
  needs every fix within 500 m and 1 s and a fix in at least 50 % of the threat epochs; level 4
  every fix within 20 m and a fix in at least 99 %; the open monitor's alarm does not count as
  the receiver's warning.

## Disclosure

While the threat library was written, its analogues were run once through the open
receiver-trust monitors to check they generate and score (per event: outcome, latency, first
monitors). That run was seen. No receiver-level measure of this scorecard (fix availability,
error, misleading epochs, recovery, levels) and none of the claim-bar quantities below were
computed on these cases before this file was committed. The scorer was developed on other
inputs (the open engine's training scenarios `coastal-drag-off` and `open-sea-jamming`).

## Claim bars

* **E3-B1, reproducible and verifiable.** Two scorings of the campaign give byte-identical
  `scorecard.json`; the signed report directory verifies with the free verifier
  (`kshana-pro dossier verify`), and a one-byte change to any file makes it fail.
* **E3-B2, independent oracle.** For every case and event, by a different route (the
  generator's instructor log: its own `position_error_m`, `time_offset_s` and `fix_valid`
  per row, not the NMEA read back): (a) the event's `max_error_m` equals the largest
  `position_error_m` of the rows in `[onset, end]` with a valid fix, within 1.0 m; (b)
  `misleading_unwarned` equals the number of rows in `[onset, onset + (end - onset) +
  recovery_s)` (or to the next event or the log end when there is no recovery) with a valid
  fix and `position_error_m > 50` or `|time_offset_s| > 0.5`, within 2 epochs per event.
* **E3-B3, what the open monitor adds.** Over the spoofing-class events of the campaign
  (receiver-trust kind `spoofing`), the sum of `misleading_unmonitored` is at most half the sum
  of `misleading_unwarned`. If met, the sample may say: "on the public sample campaign, the open
  monitor warned during at least half of the epochs in which the receiver model gave wrong
  output without its own warning". If missed, that sentence is not used and the miss is
  published.

No other claim is made from this campaign. Levels are reported as test evidence for the
modelled receiver only.
