# Receiver-test partner — technical requirements (name-free)

**Source:** correspondence with a prospective partner, 2026-10-03 to 2026-10-06 (kept outside git).
The counterparty runs real GNSS receivers against a lab RF simulator from a European vendor, in
jammer and spoofer configurations, with work tied to that vendor and to ESA. An NDA is in review.
This file records **what they need**, in their technical terms, so every build decision can be traced
to it. It names no person and no company; keep it that way (handover §2 rule 3).

## What they asked for (their words, paraphrased)

| # | Requirement | Why it matters to them |
|---|---|---|
| R1 | **Raw IQ generation beyond short single epochs**: long, multi-satellite IQ, "the fundamental GPS observable" | They don't care about perturbed pseudorange/carrier phase alone; they test acquisition and tracking |
| R2 | **Channel effects inside the IQ**, not only on observables: ionospheric scintillation (amplitude and phase), troposphere delay, multipath, NLOS | They will "drill into" these effects at the loop level |
| R3 | **Acquisition and tracking loops they can see inside and change**: FFT acquisition, configurable PLL/FLL/DLL (bandwidth, integration time, correlator spacing) | They want to know *how the loops respond*, which a black-box receiver can't show |
| R4 | **How the loops respond to jammers** (and spoofers): the core value they will judge a meeting on | Their lab has RF recordings of the exact receiver vs. the vendor simulator across "every jammer type" |
| R5 | **Use their own recordings**: replay one recording through many loop designs (overnight sweeps) | They bring the stimulus; we bring the instrumented receiver |
| R6 | **Large recordings** | Lab captures are long and big |
| R7 | **Calibrate models against their lab data** so predictions for untested cases carry a stated error | Go beyond what was tested, honestly |
| R8 | **Automated, repeatable scoring against known test conditions** | Their tests have a known truth (jammer type, power, timing) |
| R9 | **Signals and formats** still to be confirmed by them: receiver log format (RINEX / NMEA / proprietary / raw IQ), IQ sample rate, bit depth, centre frequency, single or multi-frequency, which signals (L1 C/A, L5, L2C, Galileo E1/E5, …) | We asked; answers pending. Build for breadth until they answer |

## What we told them we will NOT do (binding)

- **No jammer or spoofer RF generation, no over-the-air transmit.** Their simulator does that. Our value
  is the receiving and analysis side. This matches the engine's existing policy (software only, no
  transmit, no interference/spoofer waveform synthesis). Any proposal that touches this line goes to
  the founder first.

## What we told them is coming (we must deliver it)

From the 2026-10-04 reply: long multi-satellite raw IQ; scintillation (amplitude + phase),
troposphere, multipath and NLOS in the IQ itself; a fuller open receiver with FFT acquisition and
configurable tracking loops; replay of one recording through many loop designs; handling of large
recordings; calibration of the models against their lab data. Then either a build they can run on
their own data, or a joint walkthrough on a call.

## Status

Gap analysis against kshana 0.32.0 and the open PRs, with a prioritised build plan: see
`RECEIVER-TEST-PARTNER-GAP-ANALYSIS.md` (to be written next to this file).
