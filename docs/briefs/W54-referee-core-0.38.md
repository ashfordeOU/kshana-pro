# W54 brief: Kshana 0.38 WS-B, referee core: Jammertest scoring and receiver protocols

Read first: docs/briefs/_COMMON-0.38.md (rules), docs/plans/0.38-PLAN.md section WS-B, and strategy moves 1 and 4.

## Branch you own
- kshana: feat/referee-0.38 from origin/main.

## Items (plan IDs)
B1 `kshana jammertest import` (NPRA jammertest-plan test catalogue + transmission plan JSON → receiver-trust
   [[events]], labelled "scheduled truth", catalogue IDs and year preserved). Round-trip test on the public plan
   (fetch script + pinned SHA-256; check the repo's licence first and record it).
B2 `kshana jammertest score <log>`: per test detection latency, missed and false alarms, time to recover; output a
   signed evidence pack (reuse receiver-trust evidence). Run on the JammerTest 2024 Zenodo logs via the existing
   fetch path (never vendored).
B3 Pre-register the bars in xval/jammertest-*/PREREGISTRATION.md, committed BEFORE the scoring run. Ledger rows
   stay MODELLED unless an external oracle agrees.
B4 UBX-SEC-SIG and UBX-NAV-STATUS spoofDetState in the UBX reader, plus a `receiver-self-report` monitor (the
   receiver's own flag vs Kshana's monitors), surfaced in results (Studio strip is W53's; give it the field).
B5 Septentrio SBF reader (PVT, MeasEpoch, RF status), read-only, from public reference guides only; evaluate the
   sbf-tools crate licence first (cargo deny must pass).
B6 Direct serial/USB input to `receiver-trust live`, read-only; no configuration writes unless --configure is given
   explicitly. Test with a pty loopback.
B7 SimulaMet Jammertest 2025 NMEA adapter (IEEE DataPort DOI 10.21227/a7v4-xw11) behind a fetch script ONLY if
   the licence clearly allows it; otherwise report the terms to the coordinator and skip.

## Done when
Every new command on every surface (or N/A with reason in docs/SURFACES.md); docs page(s) (docs/JAMMERTEST.md,
protocol notes); gates green; results and bars recorded; report as in the common rules.
