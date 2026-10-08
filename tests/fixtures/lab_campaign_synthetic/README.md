# Synthetic lab-campaign fixture

`basic/` is **real output of the engine's own `kshana iq campaign`**, run on two
Kshana-generated recordings. It is not a lab recording, and no number in it is a
measurement of any receiver.

| Item | Value |
|---|---|
| Engine | public `kshana`, branch `feat/034-campaign` (PR #46) @ `acbdce08aad59f466521b8640397bc89fe9d20c4`, which carries #45 final `f3a8ca8b` (release build; reports engine version `0.32.0` until 0.34.0 is cut) |
| Generator | `make_fixture.py` in this folder (`KSHANA=<engine binary> python3 make_fixture.py <scratch-dir>`) |
| Recordings | `synthetic-ramp`: 16 s, GPS L1 C/A PRNs 3 and 11, 4.092 MS/s ci16. The stated broadband J/S steps 20 → 30 dB; the signal's C/N0 follows the engine's analytic value for that J/S. `synthetic-gap`: 10 s, PRN 7, signal absent 4.0–5.5 s |
| Stimulus | an `iq scene --no-noise` signal × a C/N0 (signal-power) profile, plus seeded unit-power noise. No jammer or spoofer waveform is synthesised |
| Campaign | `data_class = "synthetic"`; 2 front-end chains (`raw`, `agc-3bit`) × 2 loop designs (`baseline`, `narrow-pll`) × 2 recordings = 8 cells; bars `max_reacq_s = 3.0`, `min_availability = 0.5`; lock source `track-session`; DIGEST `0d3be425…`; default design hash `33261cd1…` (the `baseline` design) |
| Files kept | `campaign.json` (`kshana.campaign-plan/1`), `cells/*.json` (`kshana.campaign-cell/1`), `scorecard.json` (`kshana.campaign-scorecard/1`), `DIGEST`. The recordings themselves are not kept |

Visible engine behaviour at this head, kept as it is: after the 1.5 s outage only the
`baseline` design on the `agc-3bit` chain re-locks (0.56 s after the signal returns). The other
three `synthetic-gap` cells do not re-lock (`reacq_time_s` null, `max_reacq_s` FAIL, whole-run
availability about 0.34 for that satellite). The `synthetic-ramp` event now holds lock in every
cell. This is reported to the engine owners; the fixture is not tuned to look better.

Pinning: the cell keys contain the design hash and the data class. When the engine's default
design changes (and so its hash), or when 0.34.0 is released, regenerate with the new engine
and update the engine row above.

`tests/lab_campaign_no_client_data.rs` keeps every campaign document in this folder and
nowhere else (export-control item N6).
