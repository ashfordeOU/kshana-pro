# E3 receiver scorecard: results of the pre-registered public sample (15 November 2026)

Scored once, after `2026-11-15_PREREGISTRATION.md` was committed (d5898f62), with method
`kshana-pro/receiver-scorecard/v1` on `examples/scorecards/public-analogue-campaign.toml`.
Signed sample: `report-templates/samples/receiver-scorecard-public/` (demo key:
`report-templates/samples/receiver-scorecard-public.demo-public-key.txt`). Tests:
`tests/receiver_scorecard.rs` (`e3_b1_*`, `e3_b2_*`, `e3_b3_*`) keep these outcomes equal to the
code.

| Bar | Outcome |
|---|---|
| E3-B1 reproducible and verifiable | **Met.** Two scorings are byte-identical; the signed sample verifies with its demo key; a one-byte change fails. |
| E3-B2 (a) max error equals the generator's within 1.0 m | **MISSED** on 5 of 15 events (below). |
| E3-B2 (b) misleading count equals the generator's within 2 epochs | **Met.** Worst difference 1 epoch. |
| E3-B3 unmonitored misleading epochs at most half the unwarned ones (spoofing events) | **Met.** 1,696 of 4,091 (41.5 %). |

## The E3-B2 (a) miss

* **Meaconing (`hormuz-meaconing`).** The scorecard reports a largest error of 6.4 m where the
  generator's true-time error is 136 m. The re-broadcast delays the receiver's time as well as
  its position, and the scorecard aligns truth by the receiver's own epoch time, so a
  consistent delay of both looks almost correct in position. The misleading output was still
  counted (through the 20 s time error), and the counts agree. **Method limitation:** v1 cannot
  measure the position error of a combined position-and-time delay from the receiver's own
  timestamps. A v2 must align truth by an independent clock (arrival time or a reference
  clock) and be pre-registered before it is used.
* **Four drag-offs of 0.6 to 8 km.** The two distance computations differ by up to 0.4 %
  (8,022.5 m against 8,006 m; 1,995.4 m against 2,003.5 m), more than the 1.0 m tolerance that
  was written for metre-scale errors. The bar was ill-specified for kilometre offsets; it stays
  missed, and no level verdict depends on the difference (every one is far beyond 500 m).

## What the sample shows (test evidence for the modelled receiver only)

Eleven cases, fifteen events: the open monitor detected 6, was late on 7 and missed 2 (the two
time spoofs). Every event recovered within the bar. Jamming cases support level 2 (the fix is
lost, never wrong) and miss level 3 (availability 14 to 34 %); every spoofing case misses level 2.
Highest level supported by the whole campaign: **1**. These are the behaviour of the open
engine's synthetic NMEA receiver model on MODELLED analogues, not of any product.

The permitted sentence (E3-B3 met): "on the public sample campaign, the open monitor warned
during at least half of the epochs in which the receiver model gave wrong output without its
own warning".
