# V4 multi-source switching: results of the pre-registered scoring (15 November 2026)

Scored once, after `2026-11-15_PREREGISTRATION.md` was committed alone (8cf2b77b, 11:20 UTC) and
before any selector code existed. Code scored: method `kshana-pro/multi-source/v1`
(`src/multi_source/mod.rs`, scoring `src/multi_source/v4.rs`), commit d8bf2772 (scored on the working tree that became that commit; the edits made after
the scoring run and before the commit changed only the evidence label text, added a `kind` field to
the evidence document and added lint attributes, none of which touches a decision). The test
`tests/multi_source_v4.rs` prints this table and pins every number below.

**V4 passes: every bar B1 to B6 met on D-R1, D-R2 and D-S.** F3 is claimed in the scope section 0
of the pre-registration states: on recorded clocks with modelled source errors and injected
faults, the selector met the pre-registered switching bars. It says nothing about any real
receiver, network or service; no real multi-source recording has been scored.

## Counts

| Data set | Runs (dropped) | Epochs | With output | Holdover | None |
|---|---|---|---|---|---|
| D-R1 (IGS clocks, 2025-09-01 to 09-14, 300 s) | 12 (0) | 44,349 | 44,349 | 477 | 0 |
| D-R2 (IGS clocks, 2025-08-17 to 08-30, 300 s) | 12 (0) | 44,064 | 44,064 | 465 | 0 |
| D-S (synthetic, 10 clocks, 1 s) | 10 (0) | 864,000 | 864,000 | 9,002 | 0 |

Each recorded set has 12 runs from 11 clocks: one clock's gap splits it into two runs, both
longer than a day (G24 in D-R1, G03 in D-R2).

## Bars

| Bar | Pass | D-R1 | D-R2 | D-S |
|---|---|---|---|---|
| B1 misleading share (`|TE| > PL`) | `<= 0.1 %` | 7 / 44,349 = 0.016 % **met** | 8 / 44,064 = 0.018 % **met** | 256 / 864,000 = 0.030 % **met** |
| B2 deselection (assessable episodes) | `>= 95 %`, at least 30 | 764 / 767 = 99.6 % **met** | 754 / 754 = 100 % **met** | 49 / 50 = 98.0 % **met** |
| B3a p95 `|TE|` against the oracle | `<= 1.25` | 39.43 / 34.50 ns = 1.143 **met** | 38.73 / 34.96 ns = 1.108 **met** | 35.04 / 34.85 ns = 1.006 **met** |
| B3b p99.9 `|TE|` against GNSS-only | strictly below | 139.3 against 3,559.7 ns **met** | 136.9 against 3,518.8 ns **met** | 120.0 against 3,511.7 ns **met** |
| B4 output changes | `<= 2 x transitions + 2 x runs` | 1,430 <= 32,160 **met** | 1,451 <= 32,114 **met** | 259 <= 2,216 **met** |
| B5 external output when two groups are healthy | `>= 99 %` | 43,685 / 43,685 = 100 % **met** | 43,420 / 43,420 = 100 % **met** | 852,507 / 852,509 = 99.9998 % **met** |
| B6 evidence and reproducibility | all four | **met** | **met** | **met** |

B2 by fault type (assessable, passed): D-R1 S1 152/152, S2 308/308, P 153/153, L 154/151;
D-R2 S1 150/150, S2 298/298, P 153/153, L 153/153; D-S S1 10/10, S2 20/20, P 10/10, L 10/9.
The four episodes that failed are all LEO time-error steps (+300 ns on a 40 ns source): at
some assessable epoch the faulty LEO source was still the output. The per-episode causes were
not investigated before publishing; they stayed within B2's 95 %.

B6, on the first run of each data set: two runs byte-identical (decisions table and evidence),
the evidence verifies, one row per epoch, a one-byte change in the decisions table and a changed
switch record are both detected.

## Readings the pre-registration left open

Recorded in `src/multi_source/v4.rs` before scoring: the noise is drawn epoch by epoch, the five
sources in order within each epoch; a fault acts on epochs `t0 <= t < t0 + d` (epoch times
`k tau0` from the run's first epoch); "unavailable" overrides "failed"; under S1 `osnma` keeps an
unramped measurement (the table names only its status); B2 counts one episode per fault and
source (S2 gives two); percentiles are nearest-rank.

## What this does not show

* Real source behaviour: every source's noise is white at exactly its stated level, so the test
  cannot show what happens when a source's real noise or bias exceeds what it states.
* Faults smaller than the consistency bound (about 4 sqrt(sigma_c^2 + sigma_r^2)) are not
  detectable by construction; the protection level is set so that they stay inside it.
* A fault common to every group is invisible to cross-checks.
* A weakness seen on the demo (`docs/FUSION-AND-OPERATIONS.md`, F3): an inconsistent group keeps
  its vote, which can make the choice move between two healthy sources during a GNSS time push.
  A v2 rule would need a new pre-registration.
