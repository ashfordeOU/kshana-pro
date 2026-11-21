# Pro OSNMA scenario pack

Research 2026-11-20 gaps, backlog B1402. Module `src/osnma_scenarios.rs`, command
`kshana-pro osnma-pack` (`src/cli/osnma_pack_cmd.rs`), tests `tests/osnma_scenarios.rs`.

Builds on the OSNMA scorecard (`src/osnma_conformance.rs`): its receiver-log format is reused
as is (test `id` = scenario id), and its stimulus check is run inside the pack. That scorecard
scores bar-based tests (TTFAF, forged navigation data, replay, key and tree renewal, public-key
revocation). What this adds: scenarios with expected states per subframe, a reference model that
derives them, and scoring against them.

| scenario | expected states (subframes 0–39) |
|---|---|
| `startup-cold` | N until the public key (sf 4) and root key (sf 6) are held, then A |
| `key-renewal` | A throughout: the new root key is held before the switch |
| `chain-revocation` | N from sf 19 (its key comes from a revoked chain) to sf 23; A once chain 2's root key is verified |
| `subframe-loss` | N for the three lost subframes; sf 14 still A, its key verified across the gap |
| `replay-delayed` | N from the 60 s replay: tags arrive after their key |
| `meaconing-short` | A: OSNMA alone does not detect a 4 s relay (a receiver reporting N is noted, not counted unsafe) |
| `time-sync-loss` | N once the time uncertainty passes the bound |
| `time-sync-attack` | N once the receiver's clock is 28 s ahead: denial, not false authentication |
| `pk-revocation` | N from the revocation until a new public key and root key |

The last subframe is N in every scenario: its key is not yet disclosed.

**Reference model** (parameters illustrative: disclosure delay 30 s, `max_uncertainty_s` 15 s):
subframe *k* is A when it was received; the public key is known and not revoked; the chain's root
key is known and the chain not revoked; the time uncertainty is within the bound; delay + clock
error + uncertainty < disclosure delay; and a later subframe is received whose key is usable for
that chain (same chain, or across a renewal, never across a revocation). `osnma-pack export`
records whether the model reproduces every stated table, and the cryptographic premises checked
with the open engine (`kshana::osnma::tesla`): a key verifies across three lost subframes; a key of
a revoked chain fails against the new root; an earlier key presented as current fails; plus the
scorecard's stimulus check.

`osnma-pack score <receiver-log.json> --out <dir> ...` takes the receiver's last overall status in
each subframe, compares it with the expected states and lists **unsafe** subframes (A where N is
expected) and **missed** ones (not A where A is expected); signed, with the pack.

References: Galileo OSNMA SIS ICD Issue 1.1 and Receiver Guidelines Issue 1.3 as the open engine
implements them; the model's parameters are illustrative, not quoted from either. **Not covered**:
DO-401A avionics cases and SBAS authentication (not public). The pack describes test conditions
for a lab's own simulator and generates no signal. Test evidence under the stated model, not a
statement that a receiver meets OSNMA or any standard. MODELLED: scorer exercised on SYNTHETIC
receiver logs (`osnma-pack synth-log good|faulty`).
