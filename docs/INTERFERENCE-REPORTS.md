# Interference reports: from crew reports to verified, signed evidence

PRIVATE. This page describes Pro methods and belongs to this private repository only.

Authorities count only what crews report, and every authority asks for it in another shape.
India's DGCA wants a report within 10 minutes, typed as jamming, spoofing, signal loss or
integrity error, with FMS screenshots and preserved logs. Lithuania logged 1,022 pilot reports
in one month (June 2025). Norway's Nkom stopped asking to be told about Finnmark jamming. The
EASA/EUROCONTROL action plan (February 2026) asks for a common definition of when an
interference episode starts and ends. IATA counted about 1.5 million GPS signal losses in
ADS-B data. Receivers can stay wrong for hours after spoofing, and airlines have no
per-aircraft record of exposure. Mariners (RIN survey: 36 % not confident) cannot tell GNSS
spoofing from AIS spoofing. (Sources: Kshana research 05, 07, 07b and 12,
`records/ops/kshana/research-2026-11-20-gaps/` in the coordination repository; figures as
summarised there.)

`kshana-pro reports` turns those reports into one record, groups them into episodes under
written rules, checks each report against the data, keeps an exposure ledger per aircraft or
vessel, and flags receivers that may still be wrong after an episode. The commands are also
reachable as `kshana-pro forensics reports ...`. Code: `src/ireports/`, CLI
`src/cli/reports_cmd.rs`. Tests: `tests/interference_reports.rs`. Sample inputs:
`tests/data/interference-reports/`, all synthetic.

Nothing here radiates, and no emitter is located. Pre-revenue: no customer, authority or
airline has used it.

## Licence

| Command | Feature | Tier |
|---|---|---|
| `normalise`, `validate`, `episodes` | `pro-interference-reports` | Pro |
| `verify`, `reset-check` | `pro-report-verification` | Pro |
| `ledger` | `pro-exposure-ledger` | Enterprise |
| `check`, `params`, `regimes`, `synth` | none | free |

The three features have explicit entries in `license::explicit_tier`, are in
`PRO_COMMAND_FEATURES` and in the signed tier manifest (`commercial/tier-manifest.json`).
Every producing command takes `--license <token>` (or the activated licence); `--dev` works in
debug builds only. Signed outputs use the signed-dossier format (Ed25519 over every file's
SHA-256); `kshana-pro reports check <dir> --trust <key>` or `kshana-pro dossier verify`
checks them offline with no licence.

## Quick start (synthetic data)

```bash
D=tests/data/interference-reports
kshana-pro reports synth work/                      # observations + request files
kshana-pro reports normalise $D/dgca_forms.txt,$D/easa_ecr.txt,$D/asrs_records.txt,$D/notams.txt,$D/ship_log.txt,$D/finnmark_export.csv \
  --out work/reports.json --dev
kshana-pro reports validate work/reports.json --regime dgca --dev
kshana-pro reports episodes work/reports.json --adsb work/adsb_punjab.csv \
  --out work/episodes --signer "Ops" --key-file issuer.key --dev
kshana-pro reports verify work/verify.toml --out work/verification --signer "Ops" --key-file issuer.key --dev
kshana-pro reports reset-check work/reset-abc.toml --out work/reset-abc --signer "Ops" --key-file issuer.key --dev
kshana-pro reports ledger work/ledger.toml --out work/ledger --signer "Ops" --key-file issuer.key \
  --trust <issuer public key hex> --dev
kshana-pro reports check work/ledger --trust <issuer public key hex>
```

`issuer.key` comes from `kshana-pro dossier keygen`.

## 1. Normaliser (`kshana-pro/interference-report-normaliser/v1`)

One record per report, `kshana-pro-interference-report/1` (schema version 1), in a document
`kshana-pro-interference-reports/1`:

| Field | Meaning |
|---|---|
| `id` | `rep-` + 12 hex of SHA-256(source file hash, record number): stable across runs |
| `source` | shape, file, file SHA-256, record number |
| `reporter_role`, `reporter` | pilot, crew, ATC, mariner, authority, NOTAM |
| `platform`, `subject_id`, `subject_id_type` | aircraft (registration, ICAO 24-bit), vessel (IMO, MMSI), area (NOTAM number) |
| `flight_or_voyage`, `platform_type` | as reported |
| `occurred_start_utc`, `occurred_end_utc`, `time_uncertainty_s` | when; how precisely (60 s for a time to the minute, a day without a time, half a month for an ASRS-like record) |
| `reported_utc` | when it was reported (for deadline rules) |
| `position` (`lat_deg`, `lon_deg`, `radius_m`, `text`), `altitude` (`lower_ft`, `upper_ft`, `reference`) | where |
| `kind` | `jamming`, `spoofing` or `unknown`: only an explicit jamming or spoofing statement sets one |
| `reported_type` | `jamming`, `spoofing`, `signal_loss`, `integrity_error`, `interference_unspecified`, `announced_outage`, `not_stated` |
| `spoofed_layer` | for spoofing: `gnss`, `ais`, `both`, `not_stated` |
| `systems_affected` | normalised names: GNSS receiver, FMS, ADS-B, TAWS/EGPWS, Autopilot, Inertial (IRS/INS), RNP/RNAV, approach guidance, Clock/time, EFB, AIS, ECDIS, Radar, Gyro, Dynamic positioning, VDR |
| `recovery_action`, `time_to_recover_min` | as reported, or read from the narrative ("restored after 18 minutes", "GPS normal 04:41Z") |
| `fms_screenshot`, `logs_preserved` | yes/no answers |
| `narrative`, `extra`, `notes` | the text; source fields with no schema place; every inference the normaliser made |

Shapes (auto-detected, or `--format`):

| Shape | Modelled on | Notes |
|---|---|---|
| `dgca` | the content the DGCA circular of 10 Nov 2025 asks for (as summarised) | `key: value`, records split by `---` |
| `eccairs` | EASA ECR / ECCAIRS-like field names (Reg. (EU) 376/2014 content, as summarised) | not the ECCAIRS taxonomy attribute codes |
| `asrs` | ASRS-like de-identified records (`ACN`, month, six-hour local-time bin, `Narrative 1`) | time too coarse to cluster or cross-check; marked so |
| `notam` | ICAO NOTAM Q) to G) items | Q-line centre and radius, B)/C) window, F)/G) band; context only |
| `shiplog` | deck log lines `YYYY-MM-DD HHMMZ <position> <text>` under an `IMO`/`MMSI` header | type, systems and recovery read from the text |
| `csv` | any export with a header row | headers matched through the same alias table as the forms |

Coordinates are read in the forms crews write: decimal, degrees and minutes (`59°20.5'N
024°05.1'E`, `31 42N 074 48E`), with seconds, and compact ICAO groups (`3142N07448E`).

## 2. Validation per regime

`kshana-pro reports validate <file> --regime <id>`: each failed rule is `mandatory` or
`recommended`; a report is `complete` when no mandatory rule fails. `--strict` exits 3 on any
incomplete report. The rules paraphrase each regime as summarised in Kshana research; check
them against the authority's text before relying on them for a filing.

| Regime | Mandatory (paraphrased) | Deadline |
|---|---|---|
| `dgca` | time to the minute, time of report, type in {jamming, spoofing, signal loss, integrity error}, coordinates, altitude (aircraft), affected equipment, logs preserved, FMS screenshots (aircraft), registration or flight number | report within **10 minutes** |
| `easa` | UTC date and time, registration, location, event type, narrative, reporter category | report within **72 hours** (Reg. 376/2014) |
| `asrs` | narrative, date | filing within 10 days (recommended; FAA AC 00-46) |
| `maritime` | IMO or MMSI, time, position, type; for spoofing, whether the GNSS receiver or the AIS picture was affected | none; systems affected recommended (UK MGN 719 PNT dependencies) |
| `generic` | time to five minutes, position, type | none |

## 3. Episodes (`kshana-pro/interference-episodes/v1`)

`kshana-pro reports episodes <reports.json> [--adsb <csv>...] [--params <toml>]`. Every
threshold is in `[params]` (`kshana-pro reports params` prints the defaults), written into the
output with its SHA-256, and the rules are written out in words in `definition`:

* **Report member**: active from its start to its stated end, else start + time to recover,
  else start + `report_default_duration_min` (5). NOTAMs are context only (listed as
  `context_notams` of the episodes they overlap). Reports whose time is uncertain by more
  than `max_time_uncertainty_min` (60) are listed as not clustered.
* **ADS-B member** (CSV `timestamp,aircraft_id,lat,lon,alt_baro_ft,nic,nacp`): NIC below
  `adsb_nic_min` (7) or NACp below `adsb_nacp_min` (8) after the aircraft reported at or
  above them; samples within `adsb_run_gap_s` (120) form one run; runs shorter than
  `adsb_min_run_s` (30) dropped; located at the last clean position. A jump of at least
  `adsb_jump_km` (5) above `adsb_jump_kn` (900) is a spoofing-like member.
* **Linking**: two members are linked when their active intervals are no more than
  `quiet_gap_min` (15) apart and they are within `join_radius_km` (150) of each other, or
  belong to the same subject (a moving aircraft or vessel). A candidate episode is a connected
  group of linked members; the result does not depend on input order. The centroid and
  radius are reported, not used for linking.
* **Start** = the earliest member start. **End** = the latest member end; the episode is
  closed because nothing was active within `quiet_gap_min` after it, or marked
  `open_at_data_end` when the input data stops inside that gap.
* **Threshold**: at least `min_reports` (1) reports or `adsb_min_aircraft` (2) degraded
  aircraft; other candidates are listed as below threshold.
* **Kind**: jamming (jamming reports, NIC/NACp drops), spoofing (spoofing reports, ADS-B
  jumps), mixed with both, unknown otherwise.

Signed directory: `episodes.json`, `episodes.csv`, `events.json` (the episodes as unified
`kshana-pnt-interference-event/1` events, for the area console and the voyage risk score),
`inputs/`, `signature.json`. The unified events' confidence is the count rule 1 − 0.5^observers
(capped at 0.95), stated as not a probability.

## 4. Verification and the signed record (`kshana-pro/report-verification/v1`)

`kshana-pro reports verify <request.toml>`:

```toml
reports = "reports.json"
report_ids = []                       # all reports when empty
[[receiver]]                          # any open-engine log format, or csv
label = "VT-ABC GNSS (FMS export)"
path = "vt-abc_gnss.csv"
format = "csv"                        # ubx, rinex, android, sbf, nmea, rtklib-pos, novatel, rtcm3, gnss-sdr, csv
role = "subject"                      # or "independent" (a Station, another receiver)
# static = true / static_position = [lat, lon] / max_speed_kn = 40
[[adsb]]
path = "adsb.csv"
subject_id = "800ABC"                 # when the file uses another id than the report
[[ais]]
path = "ais.csv"
[params]                              # every threshold; defaults shown by the record
```

Window: the report's start − `window_pad_min` (10) to its end + the pad (plus the time
uncertainty). Indicators per source:

* receiver (against the `baseline_min` (30) before the window): C/N0 drop ≥ 6 dB,
  ≥ 4 satellites lost, loss of fix (jamming-like); C/N0 up ≥ 3 dB with spread under half the
  baseline's, position off a fixed site by > 50 m or a jump above the speed limit, the
  receiver's own spoofing state (spoofing-like); the receiver's own jamming state;
* ADS-B: the subject's NIC/NACp drop after good values (jamming-like) and jumps
  (spoofing-like); aircraft within `nearby_radius_km` (100) doing the same;
* AIS: the subject's jumps (≥ 500 m above 50 kn) and speeds over 50 kn; vessels nearby.

Labels:

* **CORROBORATED**: at least one source shows an anomaly in the window. `kind_agreement:
  false` when the observed kind differs from the reported one (crews' jamming/spoofing calls
  are often wrong).
* **CONTRADICTED**: no anomaly anywhere, and a direct source covers at least `coverage_min`
  (80 %) of the window: the subject's own receiver with a baseline, or, for a jamming or
  unknown report, the subject's ADS-B track with an integrity baseline. ADS-B integrity never
  contradicts a spoofing report (a spoofed position can carry good NIC); AIS never
  contradicts alone.
* **UNCORROBORATED**: anything else, with the reasons (no data, coverage, independent
  sources only, time too coarse).

For a vessel's spoofing report, `observed_layer` is `gnss` when the vessel's receiver shows
spoofing, and `ais-only` when only its AIS track does while its receiver covers the window
cleanly (the GNSS-vs-AIS question mariners cannot answer from the bridge).

Signed directory: `verification.json` (each record with the full report, window, label,
reasons, every source check and indicator, inputs with hashes, parameters and their hash),
`verification.csv`, `inputs/` (every input byte for byte), `signature.json`. A label states how
the data supplied relates to the report, not whether the report is true.

## 5. Exposure ledger (`kshana-pro/exposure-ledger/v1`)

`kshana-pro reports ledger <ledger.toml> [--trust <key>]`:

```toml
title = "Fleet, November 2026"
episodes = ["episodes"]               # signed episode directories (or episodes.json files)
resets = "resets.csv"                 # subject_id,reset_utc[,action]
period_start_utc = "2026-11-01T00:00:00Z"   # optional
spoofing_events_flag = 1
exposure_minutes_flag = 120
[aliases]
"800ABC" = "VT-ABC"                   # ICAO address → registration, MMSI → IMO
```

A signed episode directory is verified first; one whose signature fails, or whose signer is
not among `--trust`, is refused and listed. Per subject: exposure minutes (union of its
member intervals, by kind), episodes, spoofing events, first and last exposure, last
spoofing, last reset, and since that reset the spoofing events and minutes. Flag `review`
at `spoofing_events_flag` spoofing events or `exposure_minutes_flag` minutes since the last
reset. Output: `ledger.json`, `ledger.csv`, `inputs/`, signed. A planning aid for
maintenance, not an airworthiness or seaworthiness determination; it is as complete as the
reports and ADS-B behind the episodes.

## 6. Post-spoofing reset check (`kshana-pro/reset-check/v1`)

`kshana-pro reports reset-check <reset.toml>`:

```toml
subject = "VT-ABC"
episode_end_utc = "2026-11-12T06:00:00Z"
reference_track = "vt-abc_irs.csv"    # or reference_position = [lat, lon] for a fixed site
reference_label = "IRS position"
[receiver]
path = "vt-abc_gnss.csv"
format = "csv"                        # or any open-engine log format
[params]                              # settle_min 5, max_disagreement_m 500, min_share 0.25, min_samples 20, frozen_m 5, frozen_min_s 60, reference_moved_m 1000
```

From `settle_min` after the episode: the share of fixes more than `max_disagreement_m` from the
reference, whether the end of the log still disagrees, and a frozen position while the
reference moves. Outcome **RESET RECOMMENDED** (a finding), **NO RESET INDICATED** (enough
samples, no finding) or **NOT ASSESSABLE** (too few samples with a fix and a reference; most
epochs without a fix suggest the interference may still be present). Evidence **MODELLED**:
a recommendation only; it is not validated against receiver manufacturers' reset criteria,
and the manufacturer's and operator's procedures decide. The reference must be independent of
the GNSS receiver under test (inertial, a second receiver on another antenna, a surveyed
site); ADS-B is not independent of the aircraft's GNSS.

## 7. Evidence status and limits

* Every output is **MODELLED**: rules and thresholds are stated engineering choices, checked on
  synthetic data with known truth (`tests/interference_reports.rs`) and the synthetic sample
  forms. None is validated on labelled real reports, real ADS-B or real receiver logs.
* The regime rules paraphrase the regimes as summarised in Kshana research; they are not the
  authorities' templates. The ECCAIRS export (taxonomy attribute codes, E5X files) is not
  built.
* No automatic capture from the EFB, QAR or FMS; reports come in as files.
* Free-text reading is keyword-based: a narrative that never names the type stays
  `not_stated`, and every inference is written to `notes`.
* ADS-B ids and registrations are linked only through the ledger's `aliases` (no aircraft
  registry is bundled).
* Not built: emitter location (never), a public-event sample (no openly licensed report set
  yet), a regulator-facing web form.
