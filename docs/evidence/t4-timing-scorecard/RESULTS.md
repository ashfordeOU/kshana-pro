# T4 timing scorecard: results of the pre-registered public sample (15 November 2026)

Scored once, after `2026-11-15_PREREGISTRATION.md` was committed (aa3d41c6), with method
`kshana-pro/timing-scorecard/v1` on `examples/timing-scorecard/request.toml`. Signed sample:
`report-templates/samples/timing-scorecard-public/` (demo key:
`report-templates/samples/timing-scorecard-public.demo-public-key.txt`). Tests:
`tests/timing_scorecard.rs` (`t4_b1_*`, `t4_b2_*`, `t4_b3_*`).

| Bar | Outcome |
|---|---|
| T4-B1 reproducible and verifiable | **Met.** |
| T4-B2 (a) bisection within one grid step of the detection time | **Met** for all four time-push pairs. |
| T4-B2 (b) denial holdover warnings agree with T1's fleet assessment | **Met** (gm-ldn-1: Internal 100 ns and PRTC-A style limits warn inside the 6 h denial; gm-fra-1: none). |
| T4-B3 every time push detected before the grandmaster's tightest limit | **Met.** Detected after 1 s (50 ns/s push) and 2 s (20 ns/s push) by the holdover envelope (about 20 ns, dominated by the 20 ns floor). |

## What the sample shows

Highest level supported for every grandmaster and scenario: **2**. Level 3 is not supported
because `gm-ldn-1` (synthetic rubidium) reaches a 5 % breach probability of the 100 ns
internal limit about 40 to 47 minutes into holdover under every scenario; `gm-fra-1` (real IGS
GPS satellite rubidium clock) holds every limit for the 6 h denial in the model.

## Caveats

* The detection is fast because the counterfeit rates (20 to 50 ns/s) are far above what a
  rubidium oscillator can explain; a slower push would take longer, and the envelope is the T1
  prediction, whose own bars (V5, V5b) missed. The 1.6 ns 95 % bound for `gm-fra-1` after 6 h is
  the model's and is optimistic for a ground grandmaster.
* No real grandmaster was attacked. The permitted sentence (T4-B3 met): "on the public sample,
  the holdover-envelope check detected every time push before the grandmaster's tightest
  user-set limit was crossed".
