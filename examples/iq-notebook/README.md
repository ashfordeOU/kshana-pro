# GNSS IQ worked example — scene → acquire → track (kshana 0.32.0)

`iq_acquire_track.ipynb` drives the GNSS IQ layer of the **published** `kshana` wheel from
Python, with no Rust toolchain: `kshana.iq_scene` synthesises 2 s of GPS L1 C/A IQ for three
satellites, `kshana.iq_acquire` finds them (and rejects an absent PRN), the PRN 3 search grid
is plotted, and `kshana.iq_track` tracks all three — Doppler error, C/N0 and phase-lock against
the scene's injected truth. A last section sweeps the acquisition margin against C/N0. Every
cell asserts what it shows, so a run that drifts from the committed outputs fails loudly.

The committed notebook **carries real outputs** from an executed run (figures and tables).

## Run it

```bash
python3 -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt          # pins kshana==0.32.0 from PyPI
jupyter nbconvert --to notebook --execute --inplace iq_acquire_track.ipynb
# or open it interactively:  pip install jupyterlab && jupyter lab iq_acquire_track.ipynb
```

Runtime ≈ 20 s on 4 cores; peak memory ≈ 1.5 GB (the 2 s scene is 8.2 M complex samples held
as Python float lists — the 0.32.0 Python API passes samples as lists, not NumPy buffers).

## Environment of the committed outputs

| Item | Value |
| --- | --- |
| kshana | 0.32.0 (`kshana-0.32.0-cp39-abi3-manylinux_2_28_x86_64.whl`, PyPI) |
| Python | CPython 3.13.16 |
| OS | Linux x86_64, glibc 2.39 |
| numpy / matplotlib | 2.5.3 / 3.11.2 |
| nbconvert / nbformat / ipykernel | 7.17.1 / 5.11.1 / 7.4.0 |
| Executed | 2026-10-07 |

All scenes are seeded, so the numbers reproduce on any platform the wheel supports; plot
rasterisation may differ in the last pixel.

## What the run shows (from the committed outputs)

- Acquisition (20 ms, default search): PRNs 3, 11, 22 acquired, code phase within 0.12 chip
  of truth, Doppler within one 667 Hz bin; absent PRN 5 rejected (statistic 23.4 vs threshold 35.9).
- Tracking (`acq_coherent=2`): final-0.5 s Doppler error +0.01/+0.01/+0.02 Hz (σ ≈ 1.3 Hz),
  NWPR C/N0 44.2–44.4 dB-Hz for an injected 45 dB-Hz, phase and code lock 100 %.
- **Finding, reported in the notebook:** with the 0.32.0 default `acq_coherent=1` the tracking
  hand-off bin is ≈ 667 Hz, so the initial carrier error can exceed a 1 ms FLL's ≈ 250 Hz
  pull-in; on this scene two of three channels false-lock ≈ 500 Hz off. `acq_coherent=2`
  fixes it. Candidate engine follow-up: a finer default hand-off search in `iq_track`.
- Acquisition margin: the default 1 ms search detects down to ≈ 44 dB-Hz; 1 ms × 10
  non-coherent down to ≈ 34 dB-Hz.

## Files

- `iq_acquire_track.ipynb` — the executed notebook.
- `build_notebook.py` — generates the (unexecuted) notebook from source; edit this, then
  re-run it and the `nbconvert --execute` line above.
- `requirements.txt` — the pinned environment.

Scope: synthetic scene, software only (no transmit, no interference/spoofing synthesis). The
validation basis of the IQ layer is summarised in
[`commercial/partners/IQ-LAYER-VALIDATION-SUMMARY.md`](../../commercial/partners/IQ-LAYER-VALIDATION-SUMMARY.md).
