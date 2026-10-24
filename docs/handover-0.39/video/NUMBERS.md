# Numbers in the 0.38 + 0.39 release video

Engine: kshana 0.39.0, commit `fc21ac4531fbc56c960284ff27ef58a34b39efa1` (integ/0.39.0), built from that checkout. Every command below was run by `src/collect.py`; the output is what the engine printed. Logs written by the script are synthetic and say so on screen. The Jammertest catalogue and plan are the published NPRA/jammertest-plan files at commit 75c0b2ceea10e470c3799d14a8ef18842d8375a7 (MIT; fetched and SHA-256 checked, not vendored).

## 1. `kshana jammertest score session.toml --catalog testcatalog.json --plan plan4.json --location 2 --local-offset-h 2`
- directory: `jt`; exit status 0
```
MODELLED: scored against the scheduled slots of the published test plan, not against recorded on-air times; no external oracle states when a transmitter was on
test     kind        onset s     end s outcome       latency s recover s  test name
1.1.4    jamming         960      1680 late               30.0       0.0  Jammer S2.1
1.1.8    jamming        1920      2640 late              150.0       1.0  Jammer U1.1
1.1.12   jamming        2880      3600 missed                -         -  Jammer H1.1
1.1.13   jamming        3840      4560 detected           10.0       0.0  Jammer H1.2
4 tests, 4 evaluable: 1 detected within tolerance, 2 late, 1 missed; median latency 30.0 s, median time to recover 0.0 s
false alarms outside every slot: 2 runs, 121 of 1375 clean epochs
Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.
```

## 2. `kshana jammertest score session.toml --catalog testcatalog.json --plan plan4.json --location 2 --local-offset-h 2 --json`
- directory: `jt`; exit status 0

## 3. `kshana evidence keygen --out signer.key`
- directory: `jt`; exit status 0
```
private key: signer.key  (keep it secret; never commit it)
public key:  signer.key.pub
public key (hex): ea917d3dd90968fa1cf1511023d8cffa9213d12b0613304360bda1a046348da5
fingerprint: 893be5c59a09e51d95d1839c8568e84f
Give verifiers the public key by a route they trust; they pin it with --pubkey.
```

## 4. `kshana jammertest score session.toml --catalog testcatalog.json --plan plan4.json --location 2 --local-offset-h 2 --key signer.key --out pack --created-utc 2025-09-15T15:00:00Z`
- directory: `jt`; exit status 0; signed evidence pack
```
MODELLED: scored against the scheduled slots of the published test plan, not against recorded on-air times; no external oracle states when a transmitter was on
test     kind        onset s     end s outcome       latency s recover s  test name
1.1.4    jamming         960      1680 late               30.0       0.0  Jammer S2.1
1.1.8    jamming        1920      2640 late              150.0       1.0  Jammer U1.1
1.1.12   jamming        2880      3600 missed                -         -  Jammer H1.1
1.1.13   jamming        3840      4560 detected           10.0       0.0  Jammer H1.2
4 tests, 4 evaluable: 1 detected within tolerance, 2 late, 1 missed; median latency 30.0 s, median time to recover 0.0 s
false alarms outside every slot: 2 runs, 121 of 1375 clean epochs
Advisory software, not type-approved navigation equipment (IEC 61108, IEC 61162): the operator remains responsible for the navigation of the vessel.
```

## 5. `kshana evidence verify pack --pubkey signer.key.pub --json`
- directory: `jt`; exit status 0; verifies the signed pack

## 6. `kshana evidence verify pack-tampered --pubkey signer.key.pub --json`
- directory: `jt`; exit status 1; the same pack with one byte appended to summary.html

## 7. `kshana receiver-trust session.toml`
- directory: `flags`; exit status 0; synthetic UBX log, 200 epochs: C/N0 down 20 dB at 80-90 s and 120-130 s; the receiver's spoofDetState set at 120-130 s and 160-165 s
```
scenario 4f96779dd313 | receiver-trust | ubx log, 200 epochs over 199 s | monitors 3 | first alarm at 80.0 s; untrusted for 22 and degraded for 0 of 140 epochs after calibration
```

## 8. `kshana holdover-test-example.toml`
- directory: `run-holdover-test-example`; exit status 0
```
holdover-test: holdover 1795 s, max|TE| 299.1 ns; prtc-a PASS, eprtc-a-holdover FAIL; recovered 70 s after GNSS returned (MODELLED, measured against the mask)
wrote holdover-test-example.result.json, holdover-test-example.chart.svg, holdover-test-example.report.html, and holdover-test-example.report.json
```

## 9. `kshana standards-pack run dhs-rpcf`
- directory: `kshana-video-_1eg3xz_`; exit status 0; text summary
```
# DHS Resilient PNT Conformance Framework v2.0, test profiles for levels 1 to 4

A standards pack measures modelled scenarios against bars stated before the run. It supports evidence for the framework rows it names; it does not rate or approve any product, and a MET bar says only that the scenario's modelled output met the stated bar.

A row marked evidenced means a run in this set supports evidence for the capabilities the row names. It is not a finding that a framework is met, and it does not mean any product has been rated or approved by anyone. The gap column states what the runs do not show.

Advisory: analysis output from a research and engineering tool, not type-approved navigation equipment and not a safety-of-life service; it must not be the sole basis for any navigation or safety decision.

| Test | Profile | Status | Bars | Supports rows |
|---|---|---|---|---|
| L1-T1 A run is repeatable from its scenario, seed and engine version | level-1 | MET | `(whole result)` identical on a second run true (measured true) | DHS-S8.6 |
| L1-T2 Manual recovery: a reset returns the receiver to a trusted state | level-1 | NEEDS INPUT | A receiver log that covers a threat, an operator reset and the recovery, plus the stated reset time. Score it with `kshana receiver-trust` against events stated before the run, then run `kshana compliance-report` on the result. | DHS-L1-R2 |
| L2-T1 A position-push spoofer is identified by the combined detector | level-2 | MET | `/decision/fused/alert` eq true (measured true); `/decision/raim/alert` eq true (measured true) | DHS-L2-R4 |
| L2-T2 A nearby broadband jammer is visible in the link-budget result | level-2 | MET | `/jammer_present` eq true (measured true); `/fom/availability_under_jamming` lt 1.0 (measured 0.0) | DHS-L2-R4, DHS-S8.1-8.3 |
| L2-T3 Automatic recovery without interrupting output | level-2 | NEEDS INPUT | A receiver log in which output continues during the threat and the source is recovered without operator action, scored with `kshana receiver-trust` against events stated before the run. | DHS-L2-R5 |
| L3-T1 Optical-clock holdover stays inside the stated timing bound | level-3 | MET | `/quantum/fom/timing_p95_ns` le 20.0 (measured 0.00012048185902206077) | DHS-L3-R7 |
| L3-T2 Chip-scale atomic clock holdover stays inside the same bound | level-3 | MET | `/classical/fom/timing_p95_ns` le 20.0 (measured 19.674897980377633) | DHS-L3-R7 |
| L3-T3 Fused alternative-PNT is never worse than free inertial or its best single channel | level-3 | MET | `/combined_m` le 70658.64378832017 (measured 364.8813331964644); `/combined_m` le 464.1363582677436 (measured 364.8813331964644); `/combined_m` le 782.4300085010005 (measured 364.8813331964644); `/combined_m` le 367.62324216266774 (measured 364.8813331964644) | DHS-L3-R6, DHS-L3-R7 |
| L4-T1 Layering of PNT sources survives a common-mode denial vector | level-4 | NOT MET | `/correlation_sweep/min_ratio_over_grid` ge 2.0 (measured 1.210977701543739) | DHS-L4-R8, DHS-S5.5 |
```

## 10. `kshana standards-pack run dhs-rpcf --json`
- directory: `kshana-video-_1eg3xz_`; exit status 0

## 11. `kshana standards-pack run imo-circ-1575`
- directory: `kshana-video-_1eg3xz_`; exit status 0; text summary
```
# IMO MSC.1/Circ.1575 shipborne PNT data processing: accuracy levels and integrity evaluation

A standards pack measures modelled scenarios against bars stated before the run. It supports evidence for the framework rows it names; it does not rate or approve any product, and a MET bar says only that the scenario's modelled output met the stated bar.

A row marked evidenced means a run in this set supports evidence for the capabilities the row names. It is not a finding that a framework is met, and it does not mean any product has been rated or approved by anyone. The gap column states what the runs do not show.

Advisory: analysis output from a research and engineering tool, not type-approved navigation equipment and not a safety-of-life service; it must not be the sole basis for any navigation or safety decision.

| Test | Profile | Status | Bars | Supports rows |
|---|---|---|---|---|
| ACC-A Level A horizontal and vertical limits held on open sea, 95 % of epochs | accuracy-level-a | NOT MET | `/samples_available` ge 0.95 (measured 0.27146814404432135) | IMO-1575-ACC |
| ACC-B Level B limits held on open sea with one constellation, 95 % of epochs | accuracy-level-b | NOT MET | `/samples_available` ge 0.95 (measured 0.0) | IMO-1575-ACC |
| ACC-B-MULTI Level B limits held on open sea with GPS plus Galileo, 95 % of epochs | accuracy-level-b | NOT MET | `/samples_available` ge 0.95 (measured 0.0) | IMO-1575-ACC, IMO-1575-INT |
| ACC-B-HARBOUR Level B limits held in a harbour approach with a representative skyline, 95 % of epochs | accuracy-level-b | NOT MET | `/samples_available` ge 0.95 (measured 0.0) | IMO-1575-ACC, IMO-1575-INT |
| INT-CONSIST Consistency checks across measurements flag a satellite-subset position push | integrity-evaluation | MET | `/decision/raim/alert` eq true (measured true) | IMO-1575-INT |
| STATUS-1 Status is raised when redundancy is lost (for example RAIM FDE replaced by no RAIM) | status-data | NEEDS INPUT | A receiver or PNT-DP output log, or its interface documentation, showing the status or integrity indication change when the number of usable satellites falls from the over-determined case to four. Paragraph 93.2.3 asks for this status to be raised; Kshana scores a log against events stated before the run with `kshana receiver-trust`. | IMO-1575-STATUS |
| INPUT-1 Implausible or inconsistent input is marked and excluded | status-data | NEEDS INPUT | A receiver log or PNT-DP input trace containing data that violates range, rate or consistency limits, with the exclusion visible in the output. Paragraphs 53 to 57. | IMO-1575-INPUT |
```

## 12. `kshana standards-pack run imo-circ-1575 --json`
- directory: `kshana-video-_1eg3xz_`; exit status 0

## 13. `kshana standards-pack run icao-doc9849-appf`
- directory: `kshana-video-_1eg3xz_`; exit status 0; text summary
```
# ICAO Doc 9849 Appendix F: monitor, assess and mitigate GNSS radio-frequency interference

A standards pack measures modelled scenarios against bars stated before the run. It supports evidence for the framework rows it names; it does not rate or approve any product, and a MET bar says only that the scenario's modelled output met the stated bar.

A row marked evidenced means a run in this set supports evidence for the capabilities the row names. It is not a finding that a framework is met, and it does not mean any product has been rated or approved by anyone. The gap column states what the runs do not show.

Advisory: analysis output from a research and engineering tool, not type-approved navigation equipment and not a safety-of-life service; it must not be the sole basis for any navigation or safety decision.

| Test | Profile | Status | Bars | Supports rows |
|---|---|---|---|---|
| MON-1 Detect the start and end of an interference event at the receiver | monitor | NEEDS INPUT | A receiver log (UBX, RINEX, Android or NMEA) from a flight or test that includes an interference interval, and the interval's stated start and end, scored with `kshana receiver-trust` against events stated before the run. | ICAO-9849-MON |
| ASSESS-1 Scenario variation: a 10 W broadband jammer 1 km away denies GNSS | assess | MET | `/fom/availability_under_jamming` le 0.05 (measured 0.0) | ICAO-9849-ASSESS |
| ASSESS-2 Scenario variation: the same jammer 10 km away still denies GNSS | assess | MET | `/fom/availability_under_jamming` le 0.05 (measured 0.0) | ICAO-9849-ASSESS |
| ASSESS-3 Scenario variation: the same jammer 100 km away degrades but does not deny | assess | MET | `/fom/availability_under_jamming` ge 0.95 (measured 1.0) | ICAO-9849-ASSESS |
| MIT-1 On-board detection: a spoofer pushing three satellites is flagged | mitigate | MET | `/decision/fused/alert` eq true (measured true) | ICAO-9849-MIT |
| MIT-2 Alternative PNT limits the severity of a GNSS outage | mitigate | MET | `/combined_m` le 70658.64378832017 (measured 364.8813331964644) | ICAO-9849-MIT |
| MIT-3 A timing holdover keeps the stated bound while GNSS is denied | mitigate | MET | `/quantum/fom/timing_p95_ns` le 20.0 (measured 0.00012048185902206077) | ICAO-9849-MIT |
| REPORT-1 Reporting and closed-loop awareness of interference events | monitor | NEEDS INPUT | Aggregated reports for a region (flight reports, ADS-B based interference cells). `kshana interference-map` builds a map from local ADS-B or AIS files; it is a view of past reports and does not locate a source. | ICAO-9849-MON |
```

## 14. `kshana standards-pack run icao-doc9849-appf --json`
- directory: `kshana-video-_1eg3xz_`; exit status 0

## 15. `kshana bench baseline --task d --split test --out bench`
- directory: `kshana-video-_1eg3xz_`; exit status 0
```
d-test-energy-cn0-drop
d-test-agc-excess
d-test-sqm-imbalance
d-test-raim-parity
d-test-fused-max-z
```

## 16. `kshana gnss-ssv-shell.toml`
- directory: `run-gnss-ssv-shell`; exit status 0
```
gnss-ssv shell 36000 km: l1 combined 1+ 99.4% 4+ 73.2%; l5 combined 1+ 100.0% 4+ 99.9%
wrote gnss-ssv-shell.result.json, gnss-ssv-shell.chart.svg, gnss-ssv-shell.report.html, and gnss-ssv-shell.report.json
```

## 17. `kshana gnss-ssv-heo.toml`
- directory: `run-gnss-ssv-heo`; exit status 0
```
gnss-ssv trajectory 497 to 58622 km altitude: l1 combined 1+ 100.0% 4+ 97.4% (max outage 0 min); l5 combined 1+ 100.0% 4+ 100.0% (max outage 0 min)
wrote gnss-ssv-heo.result.json, gnss-ssv-heo.chart.svg, gnss-ssv-heo.report.html, gnss-ssv-heo.report.json, and gnss-ssv-heo.table.csv
```

## 18. `kshana gnss-ssv-nav-lunar.toml`
- directory: `run-gnss-ssv-nav-lunar`; exit status 0
```
gnss-ssv navigation: position RMS 87.4 m overall, 1.4 m inside the GNSS orbits, 89.7 m above; longest gap 0 min
wrote gnss-ssv-nav-lunar.result.json, gnss-ssv-nav-lunar.chart.svg, gnss-ssv-nav-lunar.report.html, and gnss-ssv-nav-lunar.report.json
```

## 19. `kshana gnss-ssv-formation-heo.toml`
- directory: `run-gnss-ssv-formation-heo`; exit status 0
```
gnss-ssv navigation: position RMS 41.4 m overall, 7.7 m inside the GNSS orbits, 46.4 m above; longest gap 0 min; formation: deputy at 0.02 deg (1 km): relative RMS 1.2 m filtered vs 5.1 m from absolute fixes; deputy at 0.05 deg (2 km): relative RMS 2.1 m filtered vs 4.1 m from absolute fixes
wrote gnss-ssv-formation-heo.result.json, gnss-ssv-formation-heo.chart.svg, gnss-ssv-formation-heo.report.html, and gnss-ssv-formation-heo.report.json
```

## 20. `kshana lunar-infrastructure-od-consistency.toml`
- directory: `run-lunar-infrastructure-od-consistency`; exit status 0
```
lunar-infrastructure od-consistency: GNSS 351.5 m vs ground 23.0 m position sigma, mean q 5.81 (chi-square 6 expects 6)
wrote lunar-infrastructure-od-consistency.result.json, lunar-infrastructure-od-consistency.chart.svg, lunar-infrastructure-od-consistency.report.html, and lunar-infrastructure-od-consistency.report.json
```

## 21. `kshana lunar-infrastructure-od-consistency-model-error.toml`
- directory: `run-lunar-infrastructure-od-consistency-model-error`; exit status 0
```
lunar-infrastructure od-consistency: GNSS 351.5 m vs ground 23.0 m position sigma, mean q 5.95 (chi-square 6 expects 6)
wrote lunar-infrastructure-od-consistency-model-error.result.json, lunar-infrastructure-od-consistency-model-error.chart.svg, lunar-infrastructure-od-consistency-model-error.report.html, and lunar-infrastructure-od-consistency-model-error.report.json
```

## 22. `kshana lunar-infrastructure-frame-coestimation.toml`
- directory: `run-lunar-infrastructure-frame-coestimation`; exit status 0
```
lunar-infrastructure frame-coestimation: smallest information eigenvalue x111.01 with orbiter links, CRLB trace x0.010
wrote lunar-infrastructure-frame-coestimation.result.json, lunar-infrastructure-frame-coestimation.chart.svg, lunar-infrastructure-frame-coestimation.report.html, and lunar-infrastructure-frame-coestimation.report.json
```

## 23. `kshana lunar-infrastructure-space-tie.toml`
- directory: `run-lunar-infrastructure-space-tie`; exit status 0
```
lunar-infrastructure space-tie: 4 sweep points on 1 arcs; laser-vs-GNSS x translation sigma 1.0363 m at the registered local tie
wrote lunar-infrastructure-space-tie.result.json, lunar-infrastructure-space-tie.chart.svg, lunar-infrastructure-space-tie.report.html, and lunar-infrastructure-space-tie.report.json
```

## 24. `kshana lunar-infrastructure-geodesy-sensitivity.toml`
- directory: `run-lunar-infrastructure-geodesy-sensitivity`; exit status 0
```
lunar-infrastructure geodesy-sensitivity: CRLB sigma h2 1.52e-4, k2 1.25e-4, l2 1.23e-4
wrote lunar-infrastructure-geodesy-sensitivity.result.json, lunar-infrastructure-geodesy-sensitivity.chart.svg, lunar-infrastructure-geodesy-sensitivity.report.html, and lunar-infrastructure-geodesy-sensitivity.report.json
```

## 25. `kshana lunar-infrastructure-carrier-phase.toml`
- directory: `run-lunar-infrastructure-carrier-phase`; exit status 0
```
lunar-infrastructure carrier-phase: 69 ambiguities, ILS success 0.000 (bootstrap 0.003) on the longest arc
wrote lunar-infrastructure-carrier-phase.result.json, lunar-infrastructure-carrier-phase.chart.svg, lunar-infrastructure-carrier-phase.report.html, and lunar-infrastructure-carrier-phase.report.json
```

## Numbers used on screen, and where each comes from

- Jammertest: 4 tests (published plan, Test Area 2, 1.1.4, 1.1.8, 1.1.12, 1.1.13); outcomes 1.1.4 late, 1.1.8 late, 1.1.12 missed, 1.1.13 detected; latencies 30.0, 150.0, -, 10.0 s; median latency 30.0 s; false-alarm runs 2, 121 of 1375 clean epochs. Source: commands 1 and 2 above (`--json`). The log is synthetic.
- Evidence pack: files config.json, epochs.json, log-slice.bin, manifest.json, manifest.sig, summary.html; `kshana evidence verify` ok=True, checks manifest pass, signature pass, files pass, no-extra-files pass, hash-chain pass, slice-and-epochs pass, full-log skipped, timestamp skipped.
- Tampered copy (one byte appended to summary.html): `kshana evidence verify` exit status 1; failures [{'code': 'file-size-mismatch', 'name': 'summary.html', 'expected': 203788, 'actual': 203789}, {'code': 'file-hash-mismatch', 'name': 'summary.html', 'expected': '005fa2017908eb06be3efd02a0233d113d8d7bc528931081c4679371f349f6b0', 'actual': 'cc69b39ae67027a157eead48a1c042196c535ecc518119d6d95cd8890fa50181'}].
- Receiver self-report (synthetic UBX log, 140 epochs): receiver flag epochs 17, both 11, receiver only 6, Kshana only 11; first Kshana alarm 80.0 s, first receiver flag 120.0 s.
- Holdover test (`holdover-test-example.toml`, a synthetic time-error record): verdicts {'baseline_masks': 'PASS', 'holdover_envelope': 'FAIL', 'holdover_masks': 'FAIL', 'recovery': 'RECOVERED'}; masks ['prtc-a', 'eprtc-a-holdover']; max |TE| per step [('baseline', 4.39), ('holdover', 299.07), ('recovery', 299.94)] ns.
- Standards packs: dhs-rpcf 6 met, 1 not met, 2 needs input; imo-circ-1575 1 met, 4 not met, 2 needs input; icao-doc9849-appf 6 met, 0 not met, 2 needs input.
- kshana-bench task D baselines on the test split (live run at the version above): agc-excess AUC 0.848 in-distribution, 0.819 shifted; energy(cn0-drop) AUC 0.595 in-distribution, 0.565 shifted; fused(max-z) AUC 0.998 in-distribution, 0.983 shifted; raim-parity AUC 0.682 in-distribution, 0.672 shifted; sqm-imbalance AUC 0.882 in-distribution, 0.882 shifted. Recorded verdicts (committed `xval/bench/RESULTS-v2.md`, not re-run for the video): A2 AUC 0.766, NOT MET; D1 largest gap 0.0301, NOT MET (unchanged from 1.0.0, still pinned); task (b) R1 NOT MET (carried, not re-run), per `docs/BENCH.md`.
- Space-user GNSS, HEO run (`gnss-ssv-heo.toml`): combined-constellation availability {"l1": {"at_least_1": 100.0, "at_least_4": 97.36842105263158, "max_outage_4_min": 30.0}, "l5": {"at_least_1": 100.0, "at_least_4": 100.0, "max_outage_4_min": 0.0}}; nav run final position error 184 m; formation 88 m. All MODELLED.
- Lunar infrastructure: OD consistency mean q 5.81 against 6 expected (400 trials); with a model error 3e-10 m/s2, mean q 5.95, fraction above the 95 % point 0.052; carrier-phase fixing success per arc [(1.0, 0.01), (2.0, 0.005), (3.0, 0.0275)] (committed full run `tests/fixtures/lunar_infra/phase_full_run.json`). All MODELLED.
- Ledger: 388 rows: 176 VALIDATED, 208 MODELLED, 4 PARTNER (`web/data/verification-matrix.json` at the commit above).
