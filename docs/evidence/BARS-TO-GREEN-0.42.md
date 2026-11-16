# Bars to green before Kshana Pro 0.42 (phase 1: diagnosis and new pre-registrations)

Written 2026-11-15 (22:10 UTC). Owner decision of 15 November 2026: 0.42 is not released until
every pre-registered evidence bar that missed is met by a new, honestly pre-registered method.
This page is phase 1 only: each miss diagnosed from its **published** results, and a new
pre-registration proposed. Nothing was built, run or scored, and no new data were read. Every
bar below is the original bar, unchanged; no published miss is altered. Diagnoses marked
(unverified) are hypotheses that the development step of each pre-registration settles before
any held-out data are read.

Status values: **PROPOSED** (pre-registration committed, awaiting the owner's approval to build
and run), **NO METHOD** (no honest method can meet the bar as defined; owner decision needed),
**NOT FILED** (a method exists in writing, but a complete pre-registration cannot be written yet).

| # | Miss (original bar) | Published result | Diagnosis (one line) | New method | Data (new) | Effort | Risk | Status |
|---|---|---|---|---|---|---|---|---|
| 1 | V5 D-R **B5** false alarms <= 0.25 (T1) | 0.5289 | Interval too wide on some clocks (G09 flags 20/20 non-breach pairs at coverage 1.000), so median time to breach falls inside 48 h | V5c: V5b's Kalman handover plus orbit-period harmonics (P = 43 082 s, k = 1, 2) and a flicker-FM term | D-R3: IGS final clocks 2025-10-05..18, about 43 MB (BKG); D-S3 seed 20261120 | 2-3 d | **High** (B5 may be unreachable for any calibrated predictor on these limits) | APPROVED 16 Nov (`v5-holdover-time-to-breach/2026-11-16_V5c_PREREGISTRATION.md`) |
| 2 | V5 D-S **B1, B2, B4** (T1) | 0.6733, 0.4730, 0.2928 | 48 h least-squares handover state with white-residual covariance under-states its error on random-walk clocks (fixed by V5b, which passed D-S2) | V5c (same; re-tests the synthetic bars on a new seed) | D-S3 seed 20261120 (generated) | in 1 | Low | APPROVED 16 Nov (V5c) |
| 3 | V5b D-R2 **B1** coverage 0.90..0.99, **B5** <= 0.25 (T1) | 0.8935; 0.4897 | Three noise terms wrong both ways: G25/G26/G24 under-covered (0.65-0.79), G09/G32 over-covered; periodic orbit terms and flicker FM not modelled (unverified) | V5c | D-R3, D-S3 | in 1 | B1 moderate; B5 **high** | APPROVED 16 Nov (V5c) |
| 4 | V2 **B3** cross-attribution <= 2/24 (E1) | 4/24 | Moderate jamming (24.9-27.9 dB) lets C/N0-spread collapse at the tracking floor (and noisy state monitors) count as spoofing-like (unverified which monitor) | V2b: forensics v2, power-consistency veto on signal-shape evidence and masking of position-domain alarms in jammed epochs | New synthetic set, seed 20261121, synth seeds 3000-3047 (generated) | 1 d | Moderate | **MET** 16 Nov (V2b: B3 0/24; all synthetic bars met; E1 still needs V2P; `v2-forensics-detection/V2b_RESULTS.md`) |
| 5 | V2 part **P** public event, must be scored and pass (E1 claim rule) | not scored (no data) | No openly licensed public-event recording with a transmission schedule was in hand | V2P: forensics v2 on JammerTest 2024 stationary ZED-F9P recordings, labels from the published plan; bars equal to or tighter than V2's | JammerTest 2024, Zenodo 15911589, 358 MB, GPL-3.0 (owner's computer already holds a copy; scored locally, never committed) | 2-3 d | **High** (too few baseline/negative windows may make P not assessable) | PROPOSED (`v2-forensics-detection/2026-11-16_V2P_PREREGISTRATION.md`) |
| 6 | V3 **V3-B4** offset recall (0.85 at 5 km; 0.98 at 10/20/50 km) and **V3-G1** FA <= 0.01 (E5) | 0.861/0.879/0.871/0.871; FA 0.132 | Vessel's own fixes supported offset hypotheses (fixed in V3b); recall flat in distance | V3c (below) | seeds dev 70-79, scored 7000-7009 (generated) | 3-5 d | **High** | **MISSED** 16 Nov (V3c: recall 0.650/0.775/0.796/0.761; FA 0.0059 met; `v3-m4-offset-successor/V3c_RESULTS.md`) |
| 7 | V3b **V3-B4** and **V3-G1** (E5) | 0.829/0.857/0.829/0.896; FA 0.072 | FA: unrelated fixes stay inside the 3.7 km gate for 10 min (no test that the displacement is constant); recall: 4 % of offset vessels decided DARK plus a distance-independent loss (too few post-onset fixes or dense-traffic exclusion, unverified) | V3c: relative-motion (zero relative velocity) test, exclusivity of fixes, DARK identities reconsidered, recall ceiling published from dev seeds before scoring | as 6 | in 6 | **High** (a ceiling below 0.98 would make B4 unreachable on this generator) | **MISSED** 16 Nov (V3c: B4 0.650/0.775/0.796/0.761, G1 FA 0.0059 met; dev ceiling < 0.98 at 50 km for every pair; `v3-m4-offset-successor/V3c_RESULTS.md`) |
| 8 | E4 **S1** LEO availability >= 0.99 at 0°-60° | worst 0.969 at 10° | Service-design property (PDOP > 6 on a few % of samples); also two unstated design choices (satellite split, polar-shell pattern) were fixed silently | E4b: every admissible configuration (2 splits x 2 patterns), denser grid; met only if met in all | none (published design) | in 10 | **Very high**: not reachable by any Kshana method if the design lacks the availability | **MISSED** 16 Nov (E4b, all four configurations: worst 0.899 at 0 deg); **not a 0.42 gate (owner, 16 Nov)**; `e4-pnt-verification/E4b_RESULTS.md` |
| 9 | E4 **S2** LEO availability >= 0.99 at 70°/80°/90° | 0.960 / 0.599 / 0.000 | Only the 97° shell is visible above ~75°; its satellites sit at similar elevations (VDOP 3.8-5.8) | E4b (as 8) | none | in 10 | **Very high** (expected to miss again) | **MISSED** 16 Nov (E4b: 70-90 deg worst 0.000 to 0.881 by configuration); **not a 0.42 gate (owner, 16 Nov)** |
| 10 | E4 **S5a** OSNMA tamper detection >= 0.99 | 136/200 | 52 of 64 misses are flips in the first 6 of 24 sub-frames (status read once at stream end; 600 s failure memory or pre-chain data, unverified which); all 12 later misses are IODnav flips the verifier never assembles or checks | E4b: session-long failure memory, hold data until the chain verifies, check ADKD 0 tags on the bits as received | GSC OSNMA test vectors, `Test_vectors.zip`, 10.3 MB (scored locally, never committed); flip seed 20261123 | 1-2 d | Moderate | **MISSED** 16 Nov (E4b S5: 5/200 and 4/200 on the GSC vectors; tags authenticate against the previous identical sub-frame (COP > 1); a fix needs a new pre-registration; `e4-pnt-verification/E4b_RESULTS.md`) |
| 11 | E3 **B2 (a)** max error equals the generator's within 1.0 m | missed on 5/15 events | Meaconing: truth aligned by the receiver's own (delayed) clock; drag-offs: sphere (scorecard) against WGS-84 radii (generator), 0.2-0.4 % at km | E3b: scorecard v2, WGS-84 geodesic error, truth at arrival time on an independent clock | Held-out synthetic campaign, seeds 31001-31016 (generated) | 0.5-1 d | Low-moderate | **MET** 16 Nov (E3b held-out: B2 (a) 20/20 within 1.0 m, worst 0.2 m; B1, B2 (b), B3 met; `e3-receiver-scorecard/E3b_RESULTS.md`) |
| 12 | Open engine **M4 v1-v3 B4** offset recall (open repository) | NOT MET (v3: 0.489 at 5 km; 0.646-0.675 at 10-50 km) | As 7 (stage 1 alone) | Covered by V3c as the Pro successor; the open rows stay NOT MET | as 6 | in 6 | High | V3c **MISSED** 16 Nov (the open rows stay NOT MET) |
| 13 | Open engine **E5 v2 B4a** spoofing AUC >= 0.70 (TEXBAT) | 0.6941 (E5-PD met, 0.7410) | All ten TEXBAT recordings already scored; no blind re-score possible on them | E5 v3 (CUSUM on the E5-PD window score), written in `v6-spoofing-and-area-trust/2026-11-15_DECISION.md` | Candidate blind set: Tuni2025 GPS L1 spoofing "Selected Scenarios", Zenodo 17249727, CC BY 4.0, about 40 GB raw I/Q (owner's computer only; whether any Kshana code has read it is unverified) | 1-2 w | Unknown | **NOT FILED**: E5 v2's exact definitions (units, onset exclusion, minimum-data rule) live in the open repository (`xval/bench-e`) and must be copied in verbatim, and the owner must sign the method, before a complete pre-registration can be committed |
| 14 | Open engine **B5 v3 T2** area trust (100 % of single-satellite faults flagged and every alarmed station cleared) | NOT MET (4/127 geometry misses; 2/67 two-satellite epochs) | Stations whose fix without the faulty satellite still exceeds `H_s` cannot be cleared by any screen without changing what T2 measures | none | — | — | **Unreachable as defined** | **NO METHOD**; **not a 0.42 gate (owner, 16 Nov)**; the miss stays published |

Not listed as misses: V4 (all bars met), T4 (all met), E3-B1, E3-B2 (b), E3-B3, E4-S3, E4-S4,
E4-S5b (met). The mission-dossier worked example's "4 not met" requirements
(`docs/PRO-POWER.md`) are illustrative requirements of a sample programme, not pre-registered
evidence bars. The Ishigami interval misses in `docs/UNCERTAINTY.md` are the expected 5 % of a
95 % interval, not a bar.

## Owner decisions and approvals, 16 November 2026

Recorded on 2026-11-16, in the first commit of the phase-2 work, **before any new evaluation data
were generated, downloaded or read**.

* **Release rule.** Kshana 0.42 ships only when every bar that measures Kshana itself is met.
  **E4-S1 and E4-S2** (availability of a third-party LEO design) and the open engine's
  **B5 v3 T2** (unreachable as defined) are **not 0.42 gates**; their misses stay published.
* **Approved to build and run:** E3b, V2b, V3c, E4b (the S5 integrity part is a gate; S1/S2 are
  re-run on the four admissible configurations and published as "not a 0.42 gate (owner,
  16 Nov)") and V5c. Each pre-registration's status now reads "APPROVED by the owner,
  16 Nov 2026". Bars, metrics, thresholds, seeds and named data are unchanged.
* **Approved downloads:** the GSC OSNMA `Test_vectors.zip` (10 319 251 bytes expected; kept
  outside the repository, never committed, SHA-256 recorded in `E4b_RESULTS.md`) and the 14 IGS
  final clock files of GPS weeks 2387 and 2388 from the BKG mirror (only the decimated fixture the
  V5c plan names is committed, with IGS attribution).
* **Not approved here:** V2P (stays PROPOSED; runs on the owner's computer) and E5 v3 (NOT FILED).
* **Order:** E3b, V2b, V3c (development recall ceiling committed first), E4b (S5), V5c.

## Which pre-registrations need new data the owner must approve

| Pre-registration | Source | Size | Licence and handling |
|---|---|---|---|
| V5c (D-R3) | `https://igs.bkg.bund.de/root_ftp/IGS/products/2387/` and `/2388/`, `IGS0OPSFIN_2025278..2910000_01D_30S_CLK.CLK.gz` | 14 files, about 43 MB (about 0.5 MB committed after decimation) | IGS: open use with attribution; fixture committed like V5/V5b |
| E4b (S5) | `https://www.gsc-europa.eu/sites/default/files/sites/all/files/Test_vectors.zip` | 10 319 251 bytes | GSC site terms (redistribution not assumed); scored locally, never committed |
| V2P | Zenodo record 15911589 (DOI 10.5281/zenodo.15910563), `GNSS_DATASET_JAMMING_SPOOFING.tar.gz`; plan PDF from gpspatron.com; IGS BRDC 2024 | 358 MB (+ a few MB) | GPL-3.0-or-later; JammerTest data never redistributed; the owner's existing copy is used if its hash matches; runs on the owner's computer (extracted size unknown, more than the container's free disk) |
| E5 v3 (not filed) | Zenodo record 17249727 (Tuni2025 GPS "Selected Scenarios") | about 40 GB | CC BY 4.0; owner's computer only |

V2b, V3c and E3b use only generated synthetic data (new seeds); no download.

## Recommended order (fastest real win first)

1. **E3b**: half a day to a day, no download, low-moderate risk; the root causes are visible in
   the code and the published numbers.
2. **V2b**: about a day, no download; fixes the only synthetic E1 miss (E1 still needs V2P).
3. **E4b, integrity part (S5)**: 1-2 days, 10 MB; the IODnav mechanism is clear from published data.
4. **V5c**: 2-3 days, 43 MB; likely to fix B1, B5 uncertain.
5. **V2P**: 2-3 days on the owner's computer; high assessability risk, but it is the only route to
   an E1 claim.
6. **V3c**: 3-5 days; run the development ceiling first and decide with the owner whether the
   scored run is worth it.
7. **E5 v3**: needs the open repository's definitions, the owner's signature and 40 GB on the
   owner's computer.

**Plainly:** under the decision as written, 0.42 cannot be released while E4-S1/S2 (a measured
property of a third-party design) and, if the open-engine rows count, B5 v3 T2 (unreachable as
defined) are gates. Neither can be met by a better Kshana method. Which rows gate 0.42 is the
owner's decision; no bar was changed here.

## Where this lives

`docs/evidence/` is in this private repository only: no workflow or script mirrors it to a public
repository (searched `.github/` and `scripts/` for mirror, publish and rsync steps, 2026-11-15),
and the open-repository leak gate scans the open repository, not this one. The pre-registrations
name only open-engine and Pro methods; no Arksa-exclusive method detail is in them.
