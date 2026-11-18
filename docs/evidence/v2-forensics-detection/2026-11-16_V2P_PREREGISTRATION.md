# V2P pre-registration: incident forensics on a public interference event (roadmap 0.42 E1, part P)

**Status: PROPOSED.** Written on 2026-11-15 (21:50 UTC) and committed alone, before any
forensics code has been run on the data below and before the case list is built. It takes effect
on the owner's approval (dated 2026-11-16). V2 section 0 left part P empty and said it would be
pre-registered separately with bars equal to or tighter than the synthetic ones; this is that
registration. The result goes in `V2P_RESULTS.md` next to this file, met or missed.

## 0. What was seen first (disclosure)

* The data below were read by the **open engine's** validation rows M009 to M011 (October 2026,
  `handover/open-core-0.30/site-next/research/validation-0.30/records/`): the stationary u-blox
  ZED-F9P `rinex.csv` of spoofing sub-scenarios 2.1.x and 2.3.x (observable-level RAIM and
  clock-aided monitors) and the stepped-power jamming scenario 1.6.4. No forensics method (v1 or
  v2) has been run on them: nothing under `src/forensics/` or `tests/forensics*` names the data
  set (searched 2026-11-15) (unverified beyond that search).
* The forensics method scored here is `kshana-pro/incident-forensics/v2`, fixed in
  `2026-11-16_V2b_PREREGISTRATION.md`, and nothing else. If V2b's method text is changed before
  V2b is scored, this registration is void and must be re-filed.

## 1. Data (public; scored locally, never redistributed)

* **Recordings:** JammerTest 2024 dataset, Sayyaf, Ortiz and Renaudin (Université Gustave
  Eiffel), Zenodo record 15911589, DOI 10.5281/zenodo.15910563, GPL-3.0-or-later,
  `GNSS_DATASET_JAMMING_SPOOFING.tar.gz` (358 MB; SHA-256 begins `73e78934aa00d01d`, as recorded
  by the open validation manifest). The owner's computer already holds a copy
  (`~/Code/kshana-oracles/data/jammertest2024/`, per M009); the hash is checked before use. The
  recordings and anything derived from them sample by sample stay on the owner's computer and
  are never committed (JammerTest data are never redistributed); only per-case verdicts, onsets
  and the bar numbers are published.
* **Labels:** the published transmission plan, Jammertest Consortium, dated 2024-09-13,
  `https://gpspatron.com/wp-content/uploads/2024/10/Transmissionplan-Jammertest-2024.pdf`
  (SHA-256 begins `680282b0937f6d34`). Local CEST times are converted to GPS time as M010 does
  (local - 2 h + 18 s).
* **Navigation data:** IGS merged broadcast navigation for each recording day, BKG mirror
  `https://igs.bkg.bund.de/root_ftp/IGS/BRDC/2024/<doy>/` (day 255 already hashed in M010).
* **Scope:** stationary ZED-F9P folders only (dynamic folders out); platform `static`.
  Sub-scenarios that spoof Galileo only are out (the engine's fix uses GPS here, as M010).

## 2. Conversion and case list (committed before any analysis)

1. A converter (`tools/jammertest2024_to_rinex.py`, committed before use) writes each recording's
   `rinex.csv` to RINEX 3 observation text (pseudorange and `S` C/N0 codes, values unchanged).
   The only data read before the case list is committed are each recording's first and last epoch
   times.
2. **Positive cases:** every plan slot (jamming tests, category jamming; spoofing and meaconing
   tests, category spoofing) whose stationary recording covers
   `[slot start - 300 s, slot start + 120 s]`. A slot the plan lists as both jamming and spoofing
   is positive for both and left out of B3. Incident window `[slot start - 60 s, slot end]`; the
   300 s before it are the baseline.
3. **Negative cases:** windows of 300 s inside a recording that start at least 300 s after the
   end of the previous slot (or the recording start) and end at least 60 s before the next slot,
   non-overlapping, taken in time order; each with its own 300 s baseline before it inside the
   same quiet stretch.
4. The case list (`V2P_cases.json`: recording, slot id, category, window, baseline) is committed
   before the first analysis.

## 3. Bars (equal to or tighter than V2's)

| Bar | Pass when |
|---|---|
| P-B1 | The planned category is indicated in >= 92 % of positive cases overall, and in >= 90 % of jamming and >= 90 % of spoofing cases separately (V2: 22/24 = 91.7 % overall, 10/12 = 83 % per category) |
| P-B2 | Jamming or spoofing indicated in <= 5 % of negative cases, and in none when there are fewer than 20 |
| P-B3 | Among single-category positives, the other category indicated in <= 10 % |
| P-B4 | Onset (`onset.latest_utc` of the earliest finding of the planned category) within `[slot start, slot start + ramp + 60 s]` in >= 90 % of detected positives; `ramp` is the plan's time to its final power step for a stepped-power slot, 0 otherwise |
| P-B5 | No finding at `high` confidence (one receiver per case) |

**Assessability:** P-B1 needs at least 10 jamming and 10 spoofing positives; P-B2 at least 10
negatives; P-B3 at least 10 single-category positives. A bar with fewer is **not assessable and
counts as not passed** (E1 cannot be claimed on too little data).

## 4. Scoring

```
KSHANA_PRO_V2P_DATA=<local dir> KSHANA_PRO_V2P_WRITE=1 cargo test --release --test forensics_v2 v2p_scored -- --ignored --nocapture
```

run once on the owner's computer (the test is `#[ignore]` and skips without the data; nothing is
read from the network). It writes `V2P_results.json` (per-case verdicts and onsets only).

## 5. Miss and publication

Any bar failing or not assessable is a miss, published in `V2P_RESULTS.md` with its numbers and
the case list. E1 stays unclaimed unless V2b and V2P both pass.

## 6. Feasibility (for the owner; not part of the method)

* Effort about 2 to 3 days (converter, case builder from the plan, scorer). No new download if
  the owner's copy matches the hash (else 358 MB, plus a few MB of broadcast navigation); the
  extracted size is not known (unverified) and needs more than the ~3 GB free on the cloud
  container, so it runs on the owner's computer.
* **Risk high, mainly assessability:** M010 found pre-onset stretches of only 15 to 240 s in
  several stationary recordings, so the 300 s baseline and the negative windows may leave too few
  cases. If so, P is not assessable and E1 stays unclaimed: no other openly licensed recording of
  a public event with a published transmission schedule is known to us. A shorter baseline would
  be a new registration with its reason stated, not a change here.
