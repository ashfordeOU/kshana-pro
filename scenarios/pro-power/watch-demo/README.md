# Campaign watch: demonstration sequence

A demonstration sequence, labelled as one wherever it is shown. Six revisions of the open
chained-mission campaign (jamming, spoofing, holdover, integrity) run in order through one
campaign watch (`pack.toml`). Each run is gated against the last passing run and appended
to one hash-chained history. Every run is a real open-engine run; the revisions are
written for the demonstration.

| Revision | File | Change against the previous revision | Expected result |
|---|---|---|---|
| 1 | `revisions/r1-published.toml` | the open campaign as published; only a header is added | baseline, exit 0 |
| 2 | `revisions/r2-documentation.toml` | a comment only | nothing moves, exit 0 |
| 3 | `revisions/r3-weaker-jammer.toml` | jamming-phase jammer -30 dBW becomes -33 dBW | the carrier-to-noise density ratio (C/N0) improves, exit 0 |
| 4 | `revisions/r4-stronger-jammer.toml` | jamming-phase jammer -33 dBW becomes -24 dBW | C/N0 falls below the 25 dB-Hz floor, every satellite is lost: regression, exit 1 |
| 5 | `revisions/r5-withdrawn.toml` | revision 4 withdrawn: the jammer is back at -33 dBW | exit 0 against revision 3 |
| 6 | `revisions/r6-faster-spoofer.toml` | the spoofer ramps at 0.12 ns/s instead of 0.1 ns/s | detected 60 s earlier, exit 0 |

Run it from the repository root after `cargo build`:

```bash
scenarios/pro-power/watch-demo/run-demo.sh <output-dir>
```

The script writes `SHOWCASE.md` (the exact command, revision SHA-256 (Secure Hash
Algorithm, 256-bit), scenario hash, seed and engine version of every run), the reports,
the history, the trend page and two diffs. It refuses an output directory that is not
empty unless it is an earlier output of the script. MODELLED scenarios: decision support,
not a certification. See `docs/CAMPAIGN-WATCH.md`.
