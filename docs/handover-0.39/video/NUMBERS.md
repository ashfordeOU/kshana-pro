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
public key (hex): a8c93425f00b9349f8f7c7d033323b533a4f4b4c9837501eee251ef7c238b336
fingerprint: 4724bd98b46278538ec75301826943b2
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
- directory: `flags`; exit status 0; synthetic UBX log, 200 epochs
```
(writes session.result.json)
```

## 8. `kshana holdover-test-example.toml`
- directory: `run`; exit status 0
```
(writes holdover-test-example.result.json)
```

## 9. `kshana standards-pack run dhs-rpcf --json`
- directory: `run`; exit status 0

## 10. `kshana standards-pack run imo-circ-1575 --json`
- directory: `run`; exit status 0

## 11. `kshana standards-pack run icao-doc9849-appf --json`
- directory: `run`; exit status 0

## 12. `kshana bench baseline --task d --split test --out bench`
- directory: `run`; exit status 0

## 13. `kshana gnss-ssv-shell.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 14. `kshana gnss-ssv-heo.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 15. `kshana gnss-ssv-nav-lunar.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 16. `kshana gnss-ssv-formation-heo.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 17. `kshana lunar-infrastructure-od-consistency.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 18. `kshana lunar-infrastructure-od-consistency-model-error.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 19. `kshana lunar-infrastructure-frame-coestimation.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 20. `kshana lunar-infrastructure-space-tie.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 21. `kshana lunar-infrastructure-geodesy-sensitivity.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## 22. `kshana lunar-infrastructure-carrier-phase.toml`
- directory: `run`; exit status 0
```
(writes result.json)
```

## Numbers used on screen, and where each comes from

- Jammertest: 4 tests (published plan, Test Area 2, 1.1.4, 1.1.8, 1.1.12, 1.1.13); outcomes 1.1.4 late, 1.1.8 late, 1.1.12 missed, 1.1.13 detected; latencies 30.0, 150.0, None, 10.0 s; median latency 30.0 s; false-alarm runs 2, 121 of 1375 clean epochs. Source: commands 1 and 2 above (`--json`). The log is synthetic.
- Evidence pack: files config.json, epochs.json, log-slice.bin, manifest.json, manifest.sig, summary.html; `kshana evidence verify` ok=True, checks manifest pass, signature pass, files pass, no-extra-files pass, hash-chain pass, slice-and-epochs pass, full-log skipped, timestamp skipped.
- Tampered copy (one byte appended to summary.html): `kshana evidence verify` exit status 1; failures [{'code': 'file-size-mismatch', 'name': 'summary.html', 'expected': 203788, 'actual': 203789}, {'code': 'file-hash-mismatch', 'name': 'summary.html', 'expected': '7072b40bc57d28abc8ba8898da862b790b637ae2c0322f5ca2f4131158a05c82', 'actual': '184c2d0a703e6dfc15190b089936764979ab2d9d1c14a934960fb95429c6fc10'}].
- Receiver self-report (synthetic UBX log, 140 epochs): receiver flag epochs 17, both 11, receiver only 6, Kshana only 11; first Kshana alarm 80.0 s, first receiver flag 120.0 s.
- Holdover test (`holdover-test-example.toml`, a synthetic time-error record): verdicts {'baseline_masks': 'PASS', 'holdover_envelope': 'FAIL', 'holdover_masks': 'FAIL', 'recovery': 'RECOVERED'}; masks ['prtc-a', 'eprtc-a-holdover']; max |TE| per step [('baseline', 4.39), ('holdover', 299.07), ('recovery', 299.94)] ns.
- Standards packs: dhs-rpcf 6 met, 1 not met, 2 needs input; imo-circ-1575 1 met, 4 not met, 2 needs input; icao-doc9849-appf 6 met, 0 not met, 2 needs input.
- kshana-bench task D baselines on the test split (live run at the version above): agc-excess AUC 0.848 in-distribution, 0.819 shifted; energy(cn0-drop) AUC 0.595 in-distribution, 0.565 shifted; fused(max-z) AUC 0.998 in-distribution, 0.983 shifted; raim-parity AUC 0.682 in-distribution, 0.672 shifted; sqm-imbalance AUC 0.882 in-distribution, 0.882 shifted. Recorded verdicts (committed `xval/bench/RESULTS-v2.md`, not re-run for the video): A2 ['0.766', 'NOT MET']; D1 ['largest gap 0.0301', 'NOT MET** (unchanged from 1.0.0, still pinned)']; task (b) R1 NOT MET (carried, not re-run), per `docs/BENCH.md`.
- Space-user GNSS, HEO run (`gnss-ssv-heo.toml`): combined-constellation availability {"l1": {"at_least_1": 100.0, "at_least_4": 97.36842105263158, "max_outage_4_min": 30.0}, "l5": {"at_least_1": 100.0, "at_least_4": 100.0, "max_outage_4_min": 0.0}}; nav run final position error 184 m; formation 88 m. All MODELLED.
- Lunar infrastructure: OD consistency mean q 5.81 against 6 expected (400 trials); with a model error 3e-10 m/s2, mean q 5.95, fraction above the 95 % point 0.052; carrier-phase fixing success per arc [(1.0, 0.01), (2.0, 0.005), (3.0, 0.0275)] (committed full run `tests/fixtures/lunar_infra/phase_full_run.json`). All MODELLED.
- Ledger: 388 rows: 176 VALIDATED, 208 MODELLED, 4 PARTNER (`web/data/verification-matrix.json` at the commit above).
