# Synthetic lab-campaign fixture

`basic/` is **real output of the engine's own `kshana iq campaign`**, run on two
Kshana-generated recordings. It is not a lab recording, and no number in it is a
measurement of any receiver.

| Item | Value |
|---|---|
| Engine | public `kshana`, branch `feat/034-campaign` (PR #46) @ `acbdce08aad59f466521b8640397bc89fe9d20c4`, which carries #45 final `f3a8ca8b` (release build; reports engine version `0.32.0` until 0.34.0 is cut) |
| Generator | `make_fixture.py` in this folder (`KSHANA=<engine binary> python3 make_fixture.py <scratch-dir>`) |
| Recordings | `synthetic-ramp`: 16 s, GPS L1 C/A PRNs 3 and 11, 2.5 MS/s ci16 (non-commensurate with the 1.023 MChip/s code; 4.092 MS/s would give 4 samples/chip, a staircase DLL, and the engine warns about it). The stated broadband J/S steps 20 → 30 dB; the signal's C/N0 follows the engine's analytic value for that J/S. `synthetic-gap`: 10 s, PRN 7, signal absent 4.0–5.5 s |
| Stimulus | an `iq scene --no-noise` signal × a C/N0 (signal-power) profile, plus seeded unit-power noise. No jammer or spoofer waveform is synthesised |
| Campaign | `data_class = "synthetic"`; 2 front-end chains (`raw`, `agc-3bit`) × 3 loop designs (`baseline`, `narrow-pll`, `reacquire`) × 2 recordings = 12 cells; bars `max_reacq_s = 3.0`, `min_availability = 0.5`; lock source `track-session`; DIGEST `df11fe24…`; default design hash `33261cd1…` (the `baseline` design) |
| Files kept | `campaign.json` (`kshana.campaign-plan/1`), `cells/*.json` (`kshana.campaign-cell/1`), `scorecard.json` (`kshana.campaign-scorecard/1`), `DIGEST`. The recordings themselves are not kept |

Designs. `baseline` and `narrow-pll` set no lock table, so `lock.reacquire` is at its default
(`false`): the track session only observes, and a channel that lost lock can only come back by
its own loops (`recovered`). With the FLL assist at its default (pull-in only), the PLL is left
alone through the noise gap; whether a channel is back after the outage then depends on where
its loops wandered. `reacquire` extends `baseline` with `lock.reacquire = true` at the other
defaults, which is what a lab would run for outages.

Visible engine behaviour at this head, kept as it is (nothing here is tuned): after the 1.5 s
outage on `synthetic-gap`, the observe-only designs come back on some chains (0.54 s after the
signal returns) and not on others (`reacq_time_s` null, `max_reacq_s` FAIL), while the
`reacquire` design is back on both chains after about 1.28 s. The `synthetic-ramp` event holds
lock in every cell.

Pinning: the cell keys contain the design hash and the data class. When the engine's default
design changes (and so its hash), or when 0.34.0 is released, regenerate with the new engine
and update the engine row above.

`tests/lab_campaign_no_client_data.rs` keeps every campaign document in this folder and
nowhere else (export-control item N6).
