# V2b results: incident forensics v2, power-consistent attribution (16 November 2026)

Pre-registration: `2026-11-16_V2b_PREREGISTRATION.md` (approved by the owner 16 Nov 2026, approval
commit 60c91516). Method `kshana-pro/incident-forensics/v2` (`Rules::V2` in
`src/forensics/detect.rs`) and the development table `V2b_dev.json` committed in f190b64c,
**before** the new labelled set (SplitMix64 seed 20261121, synthesiser seeds 3000 to 3047) was
generated. Scored once (`KSHANA_PRO_V2B_WRITE=1 cargo test --release --test forensics_v2 v2b_scored`);
every case is in `V2b_results.json`, and the test reproduces that file on every run. SYNTHETIC:
the open engine's NMEA synthesiser (kshana 0.41.0); not recordings.

## Verdict: every synthetic bar met. E1 is still NOT claimed

The E1 claim rule needs V2b **and** the public-event part (V2P, JammerTest 2024) to pass. V2P is not
approved for this run and has not been scored, so every E1 report keeps its "V2 not met" limit.

| Bar | Rule (unchanged from V2) | Result | Outcome |
|---|---|---|---|
| B1 | injected category indicated in >= 22 of 24 positives | 24 / 24 | **Met** |
| B1j | jamming indicated in >= 10 of 12 jamming cases | 12 / 12 | **Met** |
| B1s | spoofing indicated in >= 10 of 12 spoofing cases | 12 / 12 | **Met** |
| B2 | jamming or spoofing indicated in <= 1 of 24 negatives | 1 / 24 | **Met** |
| **B3** (the V2 miss) | the other category indicated in <= 2 of 24 positives | 0 / 24 | **Met** |
| B4 | onset in [start, start + ramp + 60 s] in >= 90 % of detected positives | 23 / 24 | **Met** |
| B5 | no high-confidence finding in any receiver-only case | 0 / 48 | **Met** |
| B6 AIS positives | AIS: spoofing indicated in >= 9 of 10 positives | 10 / 10 | **Met** |
| B6 AIS negatives | AIS: spoofing indicated in <= 1 of 10 negatives | 0 / 10 | **Met** |
| B7 ADS-B positives | ADS-B: jamming indicated in >= 9 of 10 positives | 10 / 10 | **Met** |
| B7 ADS-B negatives | ADS-B: jamming indicated in <= 1 of 10 negatives | 0 / 10 | **Met** |

B2 sits at its limit (1 of 24): the clean case S06 was indicated as spoofing by 18 epochs of the
`sea-level` monitor alone, in an episode with no jamming alarm, so neither R1 nor R2 applies
(both v1 and v2 count it). B4: 23 of 24 detected positives had their onset in the window; the miss is
S29, a time spoof of 8.5 s whose spoofing finding begins at 817 s (window 272 to 463 s).

## Development (published before scoring, `V2b_dev.json`)

v2 on V2's own labelled set (development data): B3 0 / 24 (v1: 4 / 24), B1 24 / 24, B2
0 / 24, B4 24 / 24; every bar passed. On the five training
scenarios v2 gives the same indications as v1. In the four V2 jamming cases that v1 also called
spoofing (S04, S08, S32, S40), v2 counts no signal-shape evidence; what still counts is `sea-level`
(S04 33 epochs, S40 54) below the 0.2 share. This supports the section 1 mechanism (signal-shape
and position-domain evidence in jammed epochs) without telling the two apart: v1 did not record
which monitor counted.

## Every receiver case: what counted toward each family (v2)

Epoch counts per finding (`v2_counted_<monitor>_epochs`); "jam" lists jamming-family monitors.

| Case | Class | Injected | Jam. ind. | Spoof. ind. | Onset ok | Counted (finding category: monitor epochs) |
|---|---|---|---|---|---|---|
| S00 | jamming | jamming 20.731 dB | yes | no | yes | jamming: cn0-drop 259, loss-of-lock 190, sea-level 17 of 259 |
| S01 | spoofing | drag-off 2456 m | no | yes | yes | spoofing: cn0-rise 757, cn0-spread 757, heading-course 129, kinematic 205, speed-log 125 of 817 |
| S02 | clean | none | no | no |  | no finding |
| S03 | confounder | natural (fjord) | no | no |  | no finding |
| S04 | jamming | jamming 32.884 dB | yes | no | yes | jamming: cn0-drop 119, loss-of-lock 302 of 351 |
| S05 | spoofing | time spoof 29.218 s | no | yes | yes | spoofing: kinematic 144 of 144; spoofing: kinematic 30, time-consistency 29 of 30 |
| S06 | clean | none | no | yes |  | spoofing: sea-level 18 of 18 |
| S07 | confounder | natural (container-terminal) | no | no |  | no finding |
| S08 | jamming | jamming 39.234 dB | yes | no | yes | jamming: cn0-drop 71, loss-of-lock 320, sea-level 47 of 377 |
| S09 | spoofing | drag-off 707 m | no | yes | yes | spoofing: cn0-spread 689, heading-course 308, kinematic 60, speed-log 151 of 749 |
| S10 | clean | none | no | no |  | no finding |
| S11 | confounder | natural (fjord) | no | no |  | degradation-not-attributed: loss-of-lock 31 of 31 |
| S12 | jamming | jamming 36.471 dB | yes | no | yes | jamming: cn0-drop 87, loss-of-lock 348 of 376 |
| S13 | spoofing | time spoof 30.91 s | no | yes | yes | spoofing: kinematic 95 of 95; spoofing: kinematic 30, time-consistency 30 of 30 |
| S14 | clean | none | no | no |  | no finding |
| S15 | confounder | natural (container-terminal) | no | no |  | degradation-not-attributed: loss-of-lock 184 of 184 |
| S16 | jamming | jamming 37.954 dB | yes | no | yes | jamming: cn0-drop 65, loss-of-lock 477 of 504 |
| S17 | spoofing | drag-off 2196 m | no | yes | yes | spoofing: cn0-rise 513, cn0-spread 513, heading-course 199, kinematic 60, speed-log 155 of 573 |
| S18 | clean | none | no | no |  | no finding |
| S19 | confounder | natural (fjord) | no | no |  | degradation-not-attributed: loss-of-lock 371 of 371 |
| S20 | jamming | jamming 36.157 dB | yes | no | yes | jamming: cn0-drop 88, loss-of-lock 328, sea-level 42 of 398 |
| S21 | spoofing | time spoof 56.387 s | no | yes | yes | spoofing: kinematic 100 of 100; spoofing: kinematic 60, time-consistency 56 of 60 |
| S22 | clean | none | no | no |  | no finding |
| S23 | confounder | natural (container-terminal) | no | no |  | degradation-not-attributed: loss-of-lock 470 of 470 |
| S24 | jamming | jamming 30.845 dB | yes | no | yes | jamming: cn0-drop 129, loss-of-lock 476 of 530 |
| S25 | spoofing | drag-off 2142 m | no | yes | yes | spoofing: cn0-rise 700, cn0-spread 700, heading-course 240, speed-log 238 of 700 |
| S26 | clean | none | no | no |  | no finding |
| S27 | confounder | natural (fjord) | no | no |  | degradation-not-attributed: loss-of-lock 204 of 204 |
| S28 | jamming | jamming 27.839 dB | yes | no | yes | jamming: cn0-drop 87, kinematic 1, loss-of-lock 245 of 285 |
| S29 | spoofing | time spoof 8.513 s | no | yes | no | spoofing: kinematic 30, time-consistency 8 of 30 |
| S30 | clean | none | no | no |  | no finding |
| S31 | confounder | natural (container-terminal) | no | no |  | degradation-not-attributed: loss-of-lock 499 of 499 |
| S32 | jamming | jamming 38.316 dB | yes | no | yes | jamming: cn0-drop 93, loss-of-lock 396 of 430 |
| S33 | spoofing | drag-off 1435 m | no | yes | yes | spoofing: cn0-rise 532, cn0-spread 576, heading-course 170, kinematic 127, speed-log 196 of 636 |
| S34 | clean | none | no | no |  | no finding |
| S35 | confounder | natural (fjord) | no | no |  | degradation-not-attributed: loss-of-lock 572 of 572 |
| S36 | jamming | jamming 38.165 dB | yes | no | yes | jamming: cn0-drop 79, loss-of-lock 251 of 287 |
| S37 | spoofing | time spoof 9.06 s | no | yes | yes | spoofing: kinematic 100 of 100; spoofing: kinematic 30, time-consistency 9 of 30 |
| S38 | clean | none | no | no |  | no finding |
| S39 | confounder | natural (container-terminal) | no | no |  | no finding |
| S40 | jamming | jamming 24.127 dB | yes | no | yes | jamming: cn0-drop 506, loss-of-lock 462, sea-level 16 of 506 |
| S41 | spoofing | drag-off 1739 m | no | yes | yes | spoofing: cn0-rise 5, cn0-spread 735, heading-course 183, kinematic 111, speed-log 165 of 795 |
| S42 | clean | none | no | no |  | no finding |
| S43 | confounder | natural (fjord) | no | no |  | degradation-not-attributed: loss-of-lock 197 of 197 |
| S44 | jamming | jamming 36.817 dB | yes | no | yes | jamming: cn0-drop 85, loss-of-lock 439 of 475 |
| S45 | spoofing | time spoof 35.792 s | no | yes | yes | spoofing: kinematic 177 of 177; degradation-not-attributed: kinematic 4, loss-of-lock 226, time-consistency 35 of 230 |
| S46 | clean | none | no | no |  | no finding |
| S47 | confounder | natural (container-terminal) | no | no |  | no finding |

## Notes

* R1's baseline median is the median, over the calibration window, of each epoch's median C/N0
  of the tracked satellites (fixed in the code before the development run; the pre-registration
  says "the baseline median"). An epoch whose median cannot be formed does not count under R1.
* The method parameters (`MethodParams::V1`) are unchanged; v2 changes only the counting rules,
  and reports name `kshana-pro/incident-forensics/v2`. The product's default method stays v1
  until a release decision; V2's published miss (`RESULTS.md`, B3 4 / 24) stays published.
* Known cost (stated in the pre-registration): an attack that spoofs only while it also jams
  loses position-domain epochs under R2. This set has no such class.

