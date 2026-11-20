# Road, rail, automotive, eCall and 3GPP evidence

Research 2026-11-20 gaps, epic E30 (backlog B3001-B3005). Module `src/transport/`, commands
`kshana-pro road|ecall|auto|nr|rail` (`src/cli/transport_cmd.rs`), tests
`tests/road_rail_auto.rs` (synthetic records only).

**Evidence label: MODELLED.** Every output is arithmetic on the records or parameters the user
supplies. Outputs say "checked against <document> §x", "pre-assessment" or "evidence for";
none states that a product meets a regulation or standard. Nothing here locates an emitter,
controls an antenna, fuses sensors for navigation, or generates a signal.

## Commands and features

| Command | What it does | Feature (tier) |
|---|---|---|
| `road check <request.toml>` | Telematics or on-board-unit stream: indicators, episodes, verdict, fingerprint class, per-trip trust | `pro-telematics-antispoof` (Pro) |
| `road obu-eval <request.toml> --out ...` | On-board-unit fraud evaluation over test runs with recorded attack windows; signed | `pro-obu-fraud-eval` (Pro) |
| `road library` | The jammer fingerprint library | free |
| `ecall suite`, `ecall mapping` | 2017/79 Annex VI tests and the requirement-to-test mapping | free |
| `ecall score <results.toml> --out ...` | Laboratory results against the Annex VI limits; signed pre-assessment | `pro-ecall-tests` (Pro) |
| `ecall location <calls.csv>` | AML/E112 and PSAP location reliability | `pro-emergency-location` (Pro) |
| `ecall duty <log.csv>`, `ecall battery <params.toml>` | Duty-cycle detection; tracker battery trade-off | free |
| `auto catalogue` | SOTIF triggering-condition catalogue for GNSS | free |
| `auto pl <request.toml> [--out ...]` | PL validation per triggering condition, coverage | `pro-automotive-pl` (Pro) |
| `auto v2x <request.toml> [--out ...]` | V2X confidence-ellipse, time, detection, false-flag checks | `pro-v2x-checks` (Pro) |
| `auto inject <request.toml> --csv <out>` | Data-level fault injection on recorded output | `pro-fault-injection` (Pro) |
| `nr tdd <request.toml>`, `nr prs <request.toml>` | 5G TDD timing under GNSS loss; PRS accuracy model | `pro-nr-timing` (Pro) |
| `nr vectors`, `nr check <vectors.json> <device.csv>` | GNSS integrity reference vectors; device check | free; `pro-agnss-integrity` (Pro) |
| `rail thr <request.toml> [--out ...]`, `rail balise <request.toml> [--out ...]` | THR position-hazard model; virtual-balise passage test | `pro-rail-evidence` (Pro) |
| `regpack assess rail-gnss-sil4 \| iso26262-gnss-seooc \| eu-2017-79-ecall-gnss` | Pre-assessment over the signed packs above | `pro-regulator-pack` (Pro) |

Signed packs (`report.json`, `report.html`, the request under `inputs/`, `signature.json`)
verify with `kshana-pro dossier verify <dir> --trust <key>`.

## B3002 Telematics and road-charging anti-spoof (built)

Stream CSV: `t` (Unix s) and any of `vehicle`, `lat`, `lon` (empty = no fix), `speed_mps`,
`wheel_mps`, `sats`, `cn0_dbhz`, `agc_db`, `gnss_time`, `ignition`, `cell_lat`, `cell_lon`,
`truth_lat`, `truth_lon`. Request: `title`, `salt` (at least 8 characters), `stream`,
optional `cell_deg` (at least 0.05, default 0.1), `[params]`, `library`.

Indicators (thresholds in `[params]`, all with defaults): `position_jump` (implied speed above
`max_speed_mps`), `speed_mismatch` (GNSS against wheel speed beyond `speed_tol_mps` for
`speed_persist_s`), `frozen_position`, `outage_moving` (no fix for `gap_s` while moving or with
the ignition on), `cn0_drop` and `cn0_rise` (against the vehicle's own median), `agc_drop`,
`time_offset` (receiver time against logger time beyond the median offset), `network_disagree`.
Flags within `merge_s` form an episode: jamming indicated (outage, C/N0 or AGC drop), spoofing
indicated (two or more spoof indicators), both, or inconsistent with the cause undetermined (a
single spoof indicator, which a receiver fault also explains).

Fingerprint library (`report-templates/transport/jammer-fingerprints.json`): classes by rule
on duration, distance driven, fix toggling and C/N0 drop depth: intermittent cycling, parked
vehicle, travels with the vehicle, passing source, unclassified. A class describes the episode
from the vehicle's side; it never says where a jammer was or who used it. The thresholds are
Kshana's starting values, to tune on labelled episodes.

Trips split on ignition off or `trip_gap_s`; a trip is untrusted when more than 20 % of its time
is inside episodes or its GNSS distance is outside 0.9-1.1 of the wheel distance.

**Privacy rule:** a vehicle appears only as `SHA-256(salt ‖ 0x1f ‖ id)[:12]`; the salt is never
written; positions appear only as cells of `cell_deg`.

OBU evaluation: runs with `attack = "none"` or an attack name and `start`/`end`. Per run:
detected (an episode overlapping the window), time to detect, false episodes, reference
distance (truth track, else wheel distance) against the distance the unit's fixes report
(gaps over `gap_s` not bridged). Framed as evidence for the EETS provider's or toll charger's
own assessment; Kshana is not a notified body.

## B3003 eCall and emergency location (built)

Source: Commission Delegated Regulation (EU) 2017/79, Annex VI, read on the legislation.gov.uk
consolidated text, 20 Nov 2026 (the EUR-Lex text is authoritative). The suite:

| Test | Annex VI | Limit | Statistic |
|---|---|---|---|
| `nmea` | 2.2.1 (1.2.1, 1.2.10) | RMC, GGA, VTG, GSA, GSV; GGA every ≤ 1 s | per sample |
| `static-combined`, `static-galileo`, `static-gps` | 2.2.2 (1.2.2, 1.2.3, 1.2.5) | 15 m | 95th percentile per sample |
| `dynamic` | 2.2.3 (1.2.5, 1.2.6) | 15 m | 95th percentile per sample |
| `shadow` | 2.2.4 (1.2.5) | 40 m | 95th percentile per sample |
| `ttff-130`, `ttff-140` | 2.2.5.7, 2.2.5.9 (1.2.7) | 60 s, 300 s | mean, ≥ 10 repeats per sample |
| `reacq` | 2.2.6.7 (1.2.8) | 20 s | mean, ≥ 10 repeats |
| `sens-cold-144`, `sens-track-155`, `sens-reacq-150` | 2.2.7.13 (1.2.9) | 3600 s, ≥ 600 s, 60 s | each sample |
| `ks-jam-recovery`, `ks-trusted-error` | Kshana extension | applicant's | not in the regulation |

At least three samples (2.1). A test with too few samples or repeats is "not assessed". 1.2.4
(WGS-84) is a declaration Kshana does not test; the 2 g bound of 1.2.6 is not exercised by the
2.2.3 method. UN R144 Annex 8 carries aligned test methods; its paragraph numbers were not read
and are to be confirmed. ERA-GLONASS needs GLONASS and has its own limits: not assessed;
sanctions screening comes first.

Location reliability: `source`, `radius_m`, `confidence` (%), optional `err_m`,
`interference`, `ttl_s`; per source and interference flag, the share of verified errors within
the reported radius with its Wilson interval against the declared confidence (at least 10
verified calls): consistent, over-confident or under-confident. Caller identifiers are ignored.

## B3001 Automotive (first slice)

Built: the 14-condition triggering-condition catalogue; PL validation (`t`, `err_m`, `pl_m`,
optional `al_m`, `condition`) with Stanford counts, the 95 % upper bound of the MI rate on
`n·Δt/τ` effective samples, samples needed for `target_ir`, and coverage of the required
conditions; V2X checks over a decoded CAM log (the confidence ellipse read as 95 % per ETSI
TS 102 894-2, to confirm); fault injection (step, ramp, freeze, dropout, noise, time jump, PL
scale) on a recorded file. Not built: ISO 26262 assessor tooling beyond the
`iso26262-gnss-seooc` catalogue, statistical scenario-coverage sampling, correction-service
integrity models, lane-level map matching (out of scope: export-control line).

## B3004 3GPP and 5G (first slice)

Built: TDD timing (guard period from numerology and symbols, propagation to the interferer;
the holdover evidence model's crossing of the ±1.5 µs budget and of half the remaining guard
period); PRS DL-TDOA accuracy (Cramér-Rao ranging for a flat spectrum, gNB synchronisation
error, TDOA geometry with a correlated reference); 48 GNSS integrity reference vectors from a
Gaussian-overbound HPL formula written into the file, and a checker that flags optimistic
device HPLs. The vectors are **not** the TS 37.571-5 or TS 38.171 conformance cases. Not built:
those vector sets, the TS 37.571-1 test-tolerance calculator, PRS carrier phase and PRU
calibration, a gNB timing-alarm adapter.

## B3005 Rail (first slice)

Built: THR model (nominal Gaussian overbound per independent sample, undetected faults,
jamming and spoofing episodes as position error) with the share of the apportioned THR per
contribution, margin in decades and the sigma that would use a tenth of the THR; virtual-balise
passage test (mean, standard deviation, expanded uncertainty of the mean with k = 2,
exceedances with an exact bound, overbound check of errors normalised by the claimed sigma);
the `rail-gnss-sil4` catalogue, the SIL 4 evidence template populated by `regpack assess`. Not
built: carrier-phase VB integrity, the EGNOS pre-existing-item argument, rail datasets with
ground truth under jamming, freight-train and level-crossing checks.
