# E4b results: admissible-design geometry and session-long tamper evidence (16 November 2026)

Pre-registration: `2026-11-16_E4b_PREREGISTRATION.md` (approved by the owner 16 Nov 2026, approval
commit 60c91516). Method `kshana-pro/pnt-verification/v2`. Order: the receiver wrapper
(`src/pnt_study/osnma_v2.rs`) and the development table `E4b_dev.json` (a495ce15) before the test
vectors were downloaded; the study file `examples/pnt-verification-v2/study.toml` (it names this
pre-registration's SHA-256), the stream selection `E4b_streams.json` and the scoring test
(d4e6dca3) before any flip. Scored once (`KSHANA_PRO_OSNMA_TV=<dir> cargo test --release --test
evidence_products e4b_scored -- --ignored`): `E4b_results.json`. MODELLED.

**Gate status (owner, 16 Nov 2026):** E4-S1 and E4-S2 are **not 0.42 gates**; they measure a
third-party design. They are published here either way. S5 (integrity) measures Kshana's
verification method and is a gate.

## Verdict: E4b NOT MET (S5 missed; S1, S2, S3 missed in at least one configuration; S4 met)

| Bar | Bar | e4-split-delta | e4-split-star | preset-split-delta | preset-split-star | Met in all four? |
|---|---|---|---|---|---|---|
| E4-S1 (not a 0.42 gate) | LEO availability >= 0.99 at 0-60 deg (worst) | 0.9698 at 10.0 deg (missed) | 0.9898 at 60.0 deg (missed) | 0.8989 at 0.0 deg (missed) | 0.9804 at 0.0 deg (missed) | **MISSED** |
| E4-S2 (not a 0.42 gate) | LEO availability >= 0.99 at 70-90 deg (worst) | 0.0000 at 89.9 deg (missed) | 0.3815 at 80.0 deg (missed) | 0.8808 at 89.9 deg (missed) | 0.6459 at 89.9 deg (missed) | **MISSED** |
| E4-S3 | median LEO HDOP <= 2.0 at 0-90 deg (worst) | 1.9601 at 89.9 deg (met) | 3.2560 at 89.9 deg (missed) | 1.4631 at 89.9 deg (met) | 1.0051 at 0.0 deg (met) | **MISSED** |
| E4-S4 | GPS + Galileo + LEO availability >= 0.999 (worst) | 1.0000 at 0.0 deg (met) | 1.0000 at 0.0 deg (met) | 1.0000 at 0.0 deg (met) | 1.0000 at 0.0 deg (met) | **Met** |

Configurations: satellite split 192 / 66 (E4) or 168 / 90 (open preset) x polar-shell Walker
pattern delta or star; 12 planes at 53 deg (delta, phasing 1), 6 at 97 deg (phasing 1), 1 080 km.
Grid: latitudes 0 to 90 deg every 10 deg (the pole row is the engine's 89.9 deg), longitudes every
15 deg, 24 h every 120 s, mask 10 deg, PDOP threshold 6. Per-latitude figures for every
configuration are in `E4b_results.json`. S3, met in E4, misses here on the e4-split-star
configuration (median HDOP 3.26 at the pole).

## S5: OSNMA tamper detection on the GSC test vectors

Archive `Test_vectors.zip`, 10 319 251 bytes as expected, SHA-256
`ef9b9afc6ef9e1c57393415cf2a1dc80c4035e7c06123907ea0bb97dd3ccf370`, held in `/workspace/.data-osnma/`
(outside the repository, never committed, deleted after scoring). 200 flips per stream, ChaCha8
seed 20261123, E4's eligibility rule.

| Stream (tree) | Detected | Share | Bar >= 0.99 | Failed satellites, untampered | Bar 0 |
|---|---|---|---|---|---|
| configuration_1 (Merkle_tree_1) | 5 / 200 | 0.025 | **MISSED** | 0 | Met |
| configuration_2 (Merkle_tree_2) | 4 / 200 | 0.020 | **MISSED** | 0 | Met |

**Stream selection (disclosed).** The archive's only documentation, `readme.txt`, lists no stream
as nominal; it says the data are an annex to the Galileo OSNMA Receiver Guidelines v1.3, which is
not in the archive and was not an approved download. The two streams the archive names as service
configurations (`configuration_1`, `configuration_2`) were selected; the sixteen event streams
(chain revocation, end of chain, new Merkle tree, new public key, alert message, public key
revocation) were not. Each stream used the archive's Merkle tree whose root and keys authenticate
the most tags of the untampered stream (the open engine's opt-in rule). Committed before any flip.

## Development (published before the vectors were opened, `E4b_dev.json`)

On E4's synthetic sample stream with E4's seed 4242:

| Receiver | Detected of 200 | Undetected in the first six sub-frames | Undetected IODnav flips |
|---|---|---|---|
| open verifier (E4) | 136 | 52 | 17 |
| 1 session-long memory | 136 | 52 | 17 |
| 1 + 2 hold until the chain verifies | 183 | 5 | 17 |
| 1 + 2 + 3 (method v2) | 200 | 0 | 0 |

Session-long memory alone changed nothing, so **H1 (failure memory expired) is refuted**;
holding data until the chain verifies recovered 47 of the 52 early misses, so **H2 held**. The
bits-as-received check removed the IODnav misses. No untampered failure in any row.

## Reading (post hoc, not a re-score)

The development stream did not show what the real vectors do. A throwaway diagnostic on six
undetected flips of `configuration_1` (not committed, no result changed) showed the same pattern
each time: the ADKD 0 tags sent after the tampered sub-frame authenticate against the **previous**
sub-frame, whose data are identical (the tags carry a COP above 1, and the open verifier, like the
Receiver Guidelines, accepts a tag that matches any of the last COP sub-frames); the next
untampered sub-frame then repeats the genuine data, and the satellite ends `authenticated`. The
tampered copy is never authenticated, but nothing is recorded as a failure, and E4's detection
rule counts only `failed`. Method v2 does not address this: its policies cover failure memory,
the chain start and IODnav assembly. Counting it would need a new rule (for example, a received
ADKD 0 data set inside a tag's COP window that differs from the data the tag authenticates is a
failure) and so a **new pre-registration**, not a re-score. E4b's policies stay as they are, and
this miss is published.

The E4 product claim stays about the study method and template, never that a service is approved
or fit for a use. E4's own result (`RESULTS.md`) stays published.

