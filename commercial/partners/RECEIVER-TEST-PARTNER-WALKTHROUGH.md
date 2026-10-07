# Receiver-test partner: walkthrough on your own data (name-free)

**Status:** DRAFT runbook for coordinator and founder review. It is not a release note.
**Purpose:** this is the "build you can run on your own data" sequence. It goes from a raw lab capture
to a signed-off campaign report:
format sidecar → `iq frontend` → `iq track` with two loop designs → epochs and summary →
`iq campaign` → `kshana-pro lab-report` (HTML and PDF).
**Against:** `RECEIVER-TEST-PARTNER-REQUIREMENTS.md` (R1–R9) and the traceability in
`RECEIVER-TEST-PARTNER-GAP-ANALYSIS.md` §16.
**Run on (2026-10-07):** every command below was run in a clean container. The recording was
synthetic (Appendix A), and each command ran on the engine branch that ships it:

| Label | Engine source the command was run on | Ships in |
|---|---|---|
| **0.32.0** | released `v0.32.0` behaviour, present on every branch below | released |
| **0.33.0** | public PR #44 @ `15107fa` (front end in `track`/`sweep`). PR #38 (≈4 ms auto acquisition default) is *not* in its base | 0.33.0 |
| **0.34.0-formats** | public PR #43 @ `18d3b42` (4/12-bit, `cu8`, multi-stream, `.sdrx`); only the multi-stream path was run here | 0.34.0 |
| **0.34.0-track** | public PR #45 @ `a60ad59` (loop-design file, epochs/events/summary, re-acquisition) | 0.34.0 |
| **0.34.0-campaign** | public `feat/034-campaign` @ `da95b9c` (`iq campaign`, scorecard, DIGEST) | 0.34.0 |
| **Pro** | this repo, PR 17 `pro/lab-campaign-report-plan` (`kshana-pro lab-report`) | Pro release on engine 0.34.0 |

No step needs network access. No step synthesises a jammer or spoofer waveform. The only stimulus is
your own recording; in this dry run it was a synthetic stand-in with a signal gap.

> **Export control (item N6).** Until counsel answers, no partner recording, result or anti-jam
> number goes into either Kshana repository. Run this on your own machine. The outputs of
> steps 5–7 (cells, scorecard, report) are yours and stay with you.

---

## 0. What you need

- The `kshana` binary for 0.34.0, or for the dry run: the branch builds listed above.
- `kshana-pro` for step 7 only.
- One raw capture. Here it is `lab-run-017.bin`: 20 s of GPS L1 C/A, 4.092 MS/s complex int16
  little-endian, with a 512-byte vendor header. Three satellites (PRN 3, 11, 22) at 45 dB-Hz, and
  the signal absent from 8 s to 10 s (noise only, same noise power). Its size is 327 MB.
- A working directory laid out as:

```text
lab-run-017.bin          # your capture
lab-run-017.toml         # step 1: how to read it
designs.toml             # step 3: your loop designs
conds/lab-run-017.toml   # step 5: what happened during the capture
campaign.toml            # step 5: recordings × front ends × designs
```

---

## 1. Describe the recording: format sidecar — **0.32.0**, multi-stream keys **0.34.0-formats**

A raw file gets a sidecar next to it, named `<file>.toml` (or `.json`). Every `iq` command reads
it, so no format flags are needed after this step.

```toml
# lab-run-017.toml — how to read lab-run-017.bin
format = "ci16_le"
sample_rate_hz = 4.092e6
center_hz = 1575.42e6
if_hz = 0.0
header_bytes = 512
datetime = "2026-10-07T12:00:00Z"
description = "synthetic stand-in for a lab capture"
```

```sh
kshana iq info lab-run-017.bin
```

`iq info` prints JSON: the resolved format, rate, `n_samples` (81 840 000), `duration_s` (20.0)
and the file's SHA-256 (the campaign stamps the same hash into every cell). A wrong
`header_bytes` or `format` shows up here as a wrong duration or as a power level far from the
expected one.

**0.34.0-formats** adds the following.
- More formats: `ci4_msb`/`ci4_lsb`, `cu4_*`, `cu8` (RTL-SDR), `ci12r_le`/`ci12l_be`/…, and
  `c2sm_byte`.
- Sample-interleaved multi-stream files: put `channels = <n>` and `channel = <k>` in the sidecar,
  or pass `--channels <n> --channel <k>`.
- Multi-channel SigMF.
- ION GNSS SDR `.sdrx` recordings.

Dry run of the multi-stream case on PR #43 @ 18d3b42. The test file was a 3 s two-stream file,
`two-ant.bin`: stream 0 was noise only, and stream 1 was the first 3 s of `lab-run-017`,
sample-interleaved `ci16_le` with no header.

```toml
# two-ant.toml
format = "ci16_le"
sample_rate_hz = 4.092e6
center_hz = 1575.42e6
channels = 2      # two sample-interleaved streams
channel = 1       # read stream 1
```

```sh
kshana iq info two-ant.bin             # "channels": 2, n_samples 12 276 000, duration_s 3.0
kshana iq track two-ant.bin --signal gps-l1ca --prn 3,11,22 --acq-coherent 4 --max-seconds 2
kshana iq track two-ant.bin --channel 0 --signal gps-l1ca --prn 3 --acq-coherent 4   # the other stream
```

On stream 1 all three satellites tracked at about 44 dB-Hz. On stream 0, PRN 3 was correctly
reported as not acquired (statistic 25.2 against threshold 38.7). Without a sidecar the same file
reads with `--format ci16_le --rate 4.092e6 --channels 2 --channel 1`.

## 2. Condition the samples: front end — **0.32.0** (standalone), **0.33.0** (inside `track`)

The standalone filter writes a conditioned copy:

```sh
kshana iq frontend lab-run-017.bin fe-notch-2bit.bin --notch --bits 2 --out-format ci8
```

The stages are `--bandpass lo,hi`, `--notch`, `--blank <thr>`, `--excise`, `--agc` and `--bits <n>`,
applied in that order. Without `--out-format` the copy is written as `cf32`. For this file that is
650 MB, twice the input, so name a compact format.

From **0.33.0** the same flags work directly on `iq acquire`, `iq track` and `iq sweep`, and no
copy is written:

```sh
kshana iq track lab-run-017.bin --signal gps-l1ca --prn 3,11,22 \
  --notch --bits 2 --acq-coherent 4 --max-seconds 6
```

The front end is applied to both the acquisition pass and the tracking pass. PR #44's tests check
that the output equals `iq track` run on the cf32 copy `iq frontend` writes. On the dry run all
three satellites tracked at about 44 dB-Hz through the 2-bit front end (phase lock 0.96–0.97 over
6 s, 7 s wall time). `--acq-coherent 4` is needed on the PR #44 build; see §8 item 4.

## 3. Track with two loop designs — **0.34.0-track**

A loop-design file names designs. Each one either states every loop parameter or `extends`
another design and changes only what differs:

```toml
# designs.toml
schema = "kshana.loop-design/1"

[[design]]
name = "baseline"     # the engine default: FLL-assisted 2nd-order PLL, 1st-order carrier-aided DLL

[[design]]
name = "narrow-pll"
extends = "baseline"
[design.carrier]
pll_bw_hz = 8.0
[design.code]
bw_hz = 1.0
```

Run one design per track run:

```sh
for d in baseline narrow-pll; do
  kshana iq track lab-run-017.bin --signal gps-l1ca --prn 3,11,22 \
    --design designs.toml --design-name $d --reacquire \
    --epochs epochs-$d.jsonl --events events-$d.jsonl --summary summary-$d.json
done
```

Each run took about 11 s (one core) for 20 s × 3 channels. The resolved design is printed in the
summary with its hash. `baseline` resolves to c93993d4…, and `narrow-pll` resolves to c2a408ed….

To score a whole design file against one recording in a single pass, use the sweep:

```sh
kshana iq sweep lab-run-017.bin --signal gps-l1ca --prn 3,11,22 \
  --design designs.toml --max-seconds 8 --csv sweep.csv
```

`sweep.csv` has one row per (design, satellite). Its columns are `design, code, epochs,
phase_jitter_deg, code_jitter_chips, phase_lock_frac, code_lock_frac, mean_cn0_dbhz,
design_hash`.

## 4. Look inside the loops: epochs, events, summary — **0.34.0-track**

| File | Schema | One record per | Contents |
|---|---|---|---|
| `epochs-<d>.jsonl` (`--epochs-format csv\|jsonl\|bin`) | `kshana.track-epoch/1` | channel × coherent epoch | E/P/L I and Q, the PLL/FLL/DLL discriminators, Doppler, carrier phase, code rate and phase, PLI, phase/code lock flags, C/N0 by NWPR and by Beaulieu, bit edge and bit, and the lock state. The first line is a header with the field list, the record size for `bin`, and the design hashes. |
| `events-<d>.jsonl` | — | lock-state transition | `from` → `to` with a reason, at the epoch and the sample index. The states are PULL_IN, LOCKED, LOST, REACQ and RETIRED. |
| `summary-<d>.json` | `kshana.track-summary/1` | channel, plus the run | Locked fraction, phase/code lock fractions, jitter, mean and final C/N0, re-acquisitions, false locks, and the final state. It also has the fully resolved design(s) and which parameters a command-line flag overrode (`overridden_by_flags`). |

Size for 20 s × 3 channels at 1 ms: about 17 MB as JSONL, 4.6 MB as `--epochs-format bin`
(184 bytes per record). Use `bin` for long captures.

On the dry-run capture the events show every channel LOCKED at 1.2 s and LOST at 8.2 s. That is
0.2 s of loss dwell after the 8.0 s gap onset, which is correct. After that the channel goes into
re-acquisition. **Known issue at a60ad59:** see §8 items 2–3.

## 5. Run the campaign: recordings × front ends × designs — **0.34.0-campaign**

The test-condition file says what the lab did during the capture. The engine scores against it.

```toml
# conds/lab-run-017.toml
schema = "kshana.test-conditions/1"

[recording]
id = "lab-run-017"
path = "../lab-run-017.bin"   # relative to this file; read through ../lab-run-017.toml
settle_s = 2.0

[receiver]
dut = "receiver under test, as the lab names it"
front_end = "ideal"

[[expected]]
signal = "gps-l1ca"
ids = [3, 11, 22]
nominal_cn0_dbhz = 45.0

[[event]]
id = "gap-1"
kind = "outage"          # interference | spoofing | outage | other
type = "unknown"         # the lab's own label
onset_s = 8.0
offset_s = 10.0
```

```toml
# campaign.toml
schema = "kshana.campaign/1"
name = "synthetic-walkthrough"
data_class = "synthetic"      # required; a partner writes "client-confidential"

[inputs]
conditions = ["conds/*.toml"]
designs = "designs.toml"

[[frontend]]
name = "raw"

[[frontend]]
name = "agc-3bit"
bits = 3

[scoring]
baseline_window_s = 2.0

[scoring.bars]
max_reacq_s = 3.0
```

```sh
kshana iq conditions conds/lab-run-017.toml        # validate the conditions file only
kshana iq campaign campaign.toml --out out --dry-run   # count the cells, run nothing
kshana iq campaign campaign.toml --out out --workers 4
kshana iq campaign report out                      # rebuild scorecard + report from the cells
```

1 recording × 2 front ends × 2 designs = 4 cells, which took 26 s with 4 workers. The run can be
stopped and restarted. Cells already on disk are kept, unless `--no-resume` is passed. `out/`
holds the following:

- `campaign.json` (`kshana.campaign-plan/1`): the fully resolved plan, with input hashes.
- `cells/<key>.json` (`kshana.campaign-cell/1`): one file per cell. The file name is the content
  key over the recording, conditions, front end, design, scoring and engine hashes.
- `scorecard.json`/`scorecard.csv`: one row per (cell, satellite) for the whole run, and one per
  (cell, satellite, event). Each row carries the engine's PASS/FAIL against your bars, and the
  bar that failed.
- `report.html`: the engine's own plain report.
- `DIGEST`: SHA-256 over the sorted `<key> <sha256(cell)>` lines. It is written only when every
  cell is present.

Result on the dry run: 11 of 12 event rows PASS `max_reacq_s = 3.0`, with re-acquisition
0.35–0.41 s after the signal returns. The exception is raw/baseline on PRN 11. It never relocked
after the gap: event FAIL `max_reacq_s`, run availability 0.384 against about 0.92 elsewhere.
That is the kind of design × satellite difference the campaign exists to surface. See §8 item 1
for why this branch can differ from the `iq track --reacquire` path.

## 6. (Optional) Re-check the campaign's integrity

```sh
cat out/DIGEST                 # d1b51341… on the dry run
```

`kshana-pro lab-report` (step 7) does this check itself and refuses a directory that fails it.
It re-derives every cell key from the cell's own inputs, checks that each cell is a planned cell
of this campaign at this engine version, recomputes the DIGEST, and checks that the scorecard
belongs to the same cells.

## 7. Sign-off report: HTML and PDF — **Pro**

```sh
kshana-pro lab-report out --data-class client-confidential \
  --html lab-run-017-report.html --pdf lab-run-017-report.pdf
```

The dry run used `--data-class synthetic` and took under 0.1 s. The report contains the following:

- **Every page:** the data class.
- **Method:** the lock-source note, which says how lock was decided and what the engine did
  *not* do.
- **Tables:** per-design × per-front-end tables of the engine's own PASS/FAIL verdicts against
  your bars, the per-event table (time to loss, re-acquisition time, outage), and the C/N0
  degradation curves where an interference event states a J/S profile. The analytic reference is
  labelled **MODELLED**.
- **Appendix:** the cell keys and the DIGEST.

The report does not recompute pass/fail; it prints the engine's.

---

## 8. Known limitations on the branches as run (2026-10-07)

Each one is reported to the engine owner. None of them blocks the sequence.

1. **The campaign and `iq track` disagree on re-acquisition.** This is `feat/034-campaign` @
   da95b9c. The campaign's own lock state has no REACQ state. A channel that loses lock in a
   long outage can stay lost (PRN 11, raw/baseline above). Public PR #46 @ d696bc46 has since moved
   the campaign runner onto the track engine's session (`lock_source = "track-session"`); this
   walkthrough has not been re-run on it yet. Cells, keys and the DIGEST change with that move, so
   re-run a campaign rather than mixing old and new cells.
2. **`--reacquire` gives up too early by default.** This is PR #45 @ a60ad59. With the default
   `max_reacq_attempts = 3`, every channel uses its three attempts within about 0.1 s of the
   loss and is RETIRED. It does not come back when the signal returns at 10 s. The workaround is
   a design with `[design.lock] max_reacq_attempts = 200`. With that, all three re-acquire at
   10.03 s and are LOCKED at 11.2 s. A time budget or back-off has been proposed upstream.
3. **The summary disagrees with the events.** This is PR #45 @ a60ad59. In the case in item 2,
   `final_state` reads REACQ while the events end in RETIRED. Trust `events-*.jsonl`.
4. **Front end plus a coarse quantiser needs a longer acquisition.** This is PR #44 @ 15107fa,
   whose base predates PR #38. With `--notch --bits 2`, the 1 ms acquisition missed PRN 3
   (statistic 32.70 against threshold 35.86). `--acq-coherent 4` finds it. PR #38 makes about
   4 ms the default for 1 ms codes, so after both merge the flag should not be needed. Re-check
   this on the 0.33.0 tag.
5. **The cell key will change once more.** The design hash is moving to canonical JSON, and the
   campaign carries `data_class` (required) in every cell and in its key. Re-run campaigns made before
   0.34.0 is tagged.
6. **Engine version string.** These branch builds report `engine_version = "0.32.0"`. The
   released 0.34.0 reports its own version, and `lab-report` checks that every cell matches the
   campaign.

---

## Appendix A. The synthetic stand-in used for this dry run

This appendix is for reproducing the dry run only. A partner skips it and uses their capture.
`iq scene` (0.32.0) makes the satellites. The 8–10 s stretch, effectively noise only (a 0 dB-Hz satellite plus noise), and the int16 header file
are spliced in with a few lines of NumPy, because the scene CLI has no per-interval C/N0 profile.
That is a signal-power change on the legitimate signal, not an interference waveform.

```sh
kshana iq scene full.cf32 --rate 4.092e6 --duration 20 --signal gps-l1ca --prn 3,11,22 \
  --doppler 1250,-2300,400 --cn0 45 --seed 7 --truth lab-run-017.truth.csv
kshana iq scene gap.cf32 --rate 4.092e6 --duration 2 --signal gps-l1ca --prn 3 --cn0 0 --seed 99
```

```python
import numpy as np
fs = 4_092_000
x = np.memmap("full.cf32", dtype=np.complex64, mode="r+")
x[8*fs:10*fs] = np.fromfile("gap.cf32", dtype=np.complex64)[:2*fs]  # effectively noise only (0 dB-Hz satellite + noise), same noise power
s = np.clip(np.round(x.view(np.float32) * 4096), -32768, 32767).astype("<i2")
with open("lab-run-017.bin", "wb") as f:
    f.write(b"\0" * 512)          # stand-in vendor header
    f.write(s.tobytes())
```

## Appendix B. Step → requirement

| Step | R | D (from the gap analysis §16.2) |
|---|---|---|
| 1 sidecar and formats | R9, R6 | — |
| 2 front end | R4 | D1 |
| 3 loop designs, sweep | R3, R5 | — |
| 4 epochs/events/summary | R3, R6 | — |
| 5 campaign, scorecard | R5, R8 | D5 (the MODELLED curve's Q) |
| 6 DIGEST | R8 | — |
| 7 Pro report | R8 | — |
