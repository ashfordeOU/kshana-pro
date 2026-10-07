# Synthetic lab-campaign fixture

`basic/` is **real output of the engine's own `kshana iq campaign`**, run on two
Kshana-generated recordings. It is not a lab recording, and no number in it is a
measurement of any receiver.

| Item | Value |
|---|---|
| Engine | public `kshana`, branch `feat/034-campaign` @ `da95b9c` (release build; reports engine version `0.32.0` until 0.34.0 is cut) |
| Generator | `make_fixture.py` in this folder (`KSHANA=<engine binary> python3 make_fixture.py <scratch-dir>`) |
| Recordings | `synthetic-ramp`: 16 s, GPS L1 C/A PRNs 3 and 11, 4.092 MS/s ci16. The stated broadband J/S steps 20 → 30 dB; the signal's C/N0 follows the engine's analytic value for that J/S. `synthetic-gap`: 10 s, PRN 7, signal absent 4.0–5.5 s |
| Stimulus | an `iq scene --no-noise` signal × a C/N0 (signal-power) profile, plus seeded unit-power noise. No jammer or spoofer waveform is synthesised |
| Campaign | 2 front-end chains (`raw`, `agc-3bit`) × 2 loop designs (`baseline`, `narrow-pll`) × 2 recordings = 8 cells; bars `max_reacq_s = 3.0`, `min_availability = 0.5` |
| Files kept | `campaign.json` (`kshana.campaign-plan/1`), `cells/*.json` (`kshana.campaign-cell/1`), `scorecard.json` (`kshana.campaign-scorecard/1`), `DIGEST`. The recordings themselves are not kept |

Visible engine behaviour on this branch, kept as it is: after the outage, two narrow-PLL
channels never re-lock (`reacq_time_s` null). The campaign's lock tracker has no
re-acquisition step yet (`RECEIVER-TEST-PARTNER-GAP-ANALYSIS.md` §16.3 item 1).

Pinning: the cell keys contain the design hash. When the engine makes that hash canonical
(key-sorted), or when 0.34.0 is released, regenerate with the new engine and update the
engine row above.

`tests/lab_campaign_no_client_data.rs` keeps every campaign document in this folder and
nowhere else (export-control item N6).
