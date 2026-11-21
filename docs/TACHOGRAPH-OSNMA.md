# Smart tachograph and road-charging OSNMA field verification

Research 2026-11-20 gaps, backlog B1403. Module `src/tacho_osnma.rs`, command `kshana-pro tacho`
(`src/cli/tacho_cmd.rs`), feature `pro-tachograph-osnma` (Pro; `synth` free), catalogue
`eu-2021-1228-tachograph-osnma` (`kshana-pro regpack assess --catalogue ...`), tests
`tests/tachograph_osnma.rs`.

## What was read

Commission Implementing Regulation (EU) 2021/1228 on EUR-Lex (CELEX 32021R1228), read on
21 Nov 2026: Annex IC requirements 41, 41b, 86, 88a, 110, 135a, 211 and 212 and Appendix 1 data
definitions. Appendix 12 (GNSS, requirements GNS_xx) was not in the text read; its rows are
marked "clause to be confirmed". No road-charging text on OSNMA was read: that row applies the
same field check to any unit that logs the authentication flag.

## Field verification (`tacho verify <log.csv>`)

Log: `timestamp,position_auth,gnss_time,vu_time,event` (`authenticated`, `not-authenticated`,
`none`; events `gnss-anomaly`, `time-conflict`, `time-adjust`). Checks against the requirements
as read, thresholds written into the result:

| check | requirement | rule |
|---|---|---|
| time-conflict-event | req. 86 | VU time and authenticated GNSS time differing by more than 3 s have a time-conflict event before the next adjustment |
| adjust-from-authenticated-time | req. 211, 212 | time adjustments only with an authenticated position |
| readjustment-interval | req. 211 | automatic adjustments 72 h to 168 h apart (workshop adjustments show here too) |
| anomaly-event-spacing | req. 88a | no GNSS anomaly event within 10 minutes of the previous |

Also: authenticated share, time to the first authenticated position, and the time recorded
without authentication and without position (the fallback behaviour while OSNMA is
unavailable). Every departure lists the records behind it. The pack is signed.

Field evidence from the user's log, MODELLED; it does not state that a vehicle unit meets the
regulation. Whether the receiver itself detects an attack is a laboratory question (OSNMA
receiver scorecard).
